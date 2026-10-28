"""End to end test of Blink with a real Paper server, on Linux, macOS and Windows (used by CI).

    python3 tools/smoke-test.py <path to blink> <server folder with paper.jar>

Writes blink.toml with `blink init`, starts Blink, waits until the server is frozen, wakes it with a
join, waits until it is frozen again and stops Blink. Fails (exit 1) when a step does not happen.

    SMOKE_FRESH=1          a new server: no server.properties yet (Blink writes it)
    SMOKE_STOP=console     stop by typing "stop" in Blink's console instead of Ctrl+C
"""
import os
import re
import signal
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
BLINK, DIR = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2])
PORT = int(os.environ.get("SMOKE_PORT", "25565"))
WINDOWS = os.name == "nt"
lines = []


def step(text):
    print(f"\n=== {text}", flush=True)


def stop_blink():
    """Stop Blink the normal way (it stops the server too), or kill it."""
    try:
        if os.environ.get("SMOKE_STOP") == "console":
            blink.stdin.write("stop\n")
            blink.stdin.flush()
        else:
            blink.send_signal(signal.CTRL_BREAK_EVENT if WINDOWS else signal.SIGINT)
        blink.wait(timeout=90)
        return True
    except subprocess.TimeoutExpired:
        blink.kill()
        return False


def fail(text):
    print(f"\nFAILED: {text}", flush=True)
    if "blink" in globals() and blink.poll() is None:
        stop_blink()
    sys.exit(1)


def wait_for(pattern, count=1, timeout=300):
    end = time.time() + timeout
    while time.time() < end:
        if sum(1 for l in lines if re.search(pattern, l)) >= count:
            return
        if blink.poll() is not None:
            fail(f"blink quit while waiting for {pattern!r}")
        time.sleep(0.5)
    fail(f"timed out waiting for {pattern!r}")


def join():
    out = subprocess.run([sys.executable, os.path.join(HERE, "status-ping.py"), f"127.0.0.1:{PORT}", "--join", "--name", "Smoke"],
                         capture_output=True, text=True, timeout=90).stdout.strip()
    print(out, flush=True)
    if "on the real server" not in out:
        fail("the join did not reach the server")
    return float(re.search(r"after ([0-9.]+) s", out).group(1))


step("blink init")
with open(os.path.join(DIR, "eula.txt"), "w") as f:
    f.write("eula=true\n")
if os.environ.get("SMOKE_FRESH") == "1":
    if os.path.exists(os.path.join(DIR, "server.properties")):
        os.remove(os.path.join(DIR, "server.properties"))
elif not os.path.exists(os.path.join(DIR, "server.properties")):
    with open(os.path.join(DIR, "server.properties"), "w") as f:
        f.write(f"server-port={PORT}\nwhite-list=false\nlevel-type=minecraft\\:flat\nview-distance=4\n")
subprocess.run([BLINK, "init", "--heap", "2G", "--sleep", "10", "--force"], cwd=DIR, check=True)

step("blink start (first start: unpacking and a training run)")
env = dict(os.environ, DO_NOT_TRACK="1")
blink = subprocess.Popen([BLINK, "start"], cwd=DIR, env=env, stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace",
                         creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if WINDOWS else 0)


def read():
    for line in blink.stdout:
        lines.append(line.rstrip())
        print(line.rstrip(), flush=True)


threading.Thread(target=read, daemon=True).start()

wait_for(r"Done \(", timeout=600)
step("waiting for save and freeze")
wait_for(r"Saved the game")
wait_for(r"Server is now sleeping")
time.sleep(5)

step("wake up with a join")
first = join()
time.sleep(3)
if blink.poll() is not None:
    fail("blink quit after the wake up")

step("waiting for the second freeze")
wait_for(r"Server is now sleeping", count=2)
time.sleep(5)
second = join()

step("stopping blink")
if not stop_blink():
    fail("blink did not stop within 90 s")
log = open(os.path.join(DIR, "logs", "latest.log"), errors="replace").read()
if "Stopping server" not in log:
    fail("the server did not stop properly")

step("blink restore")
subprocess.run([BLINK, "restore"], cwd=DIR, check=True)
props = open(os.path.join(DIR, "server.properties"), errors="replace").read()
if f"server-port={PORT}" not in props:
    fail("blink restore did not put the server's own port back")

print(f"\nOK: woke up in {first:.1f} s and {second:.1f} s, stopped cleanly", flush=True)
