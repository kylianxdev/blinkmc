"""Blink behind a proxy: one BungeeCord (or Velocity) with a lobby and four "personal worlds", each
backend server behind its own Blink. Bots log in for real and switch servers with /server.

    python3 tools/network-test.py <proxy port> <backend dir> [<backend dir> ...]

The first backend is the lobby, the rest are named after their folder (as in the proxy's config).
Each backend runs Blink in tmux socket "net-<name>", session "<name>". Prints a table of results.
"""
import asyncio
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from netbot import Bot

PROXY = int(sys.argv[1])
DIRS = [os.path.abspath(d) for d in sys.argv[2:]]
NAMES = [os.path.basename(d) for d in DIRS]
LOBBY, WORLDS = NAMES[0], NAMES[1:]
results = []


def note(test, value):
    results.append((test, value))
    print(f"  {test}: {value}", flush=True)


def pane(name):
    return subprocess.run(["tmux", "-L", f"net-{name}", "capture-pane", "-p", "-t", name, "-S", "-20000"],
                          capture_output=True, text=True).stdout


def frozen(name):
    """Is the server frozen (and its memory moved out) right now?"""
    text = pane(name)
    return text.rfind("Frozen server's memory") > max(text.rfind("Starting server for"), text.rfind("Server is now online"))


def ram_mb(name):
    """RAM of the backend: Blink and the server (their cgroup)."""
    pid = subprocess.run(["tmux", "-L", f"net-{name}", "display-message", "-p", "-t", name, "#{pane_pid}"],
                         capture_output=True, text=True).stdout.strip()
    try:
        cgroup = open(f"/proc/{pid}/cgroup").read().split("::")[1].strip()
        return int(open(f"/sys/fs/cgroup{cgroup}/memory.current").read()) >> 20
    except (OSError, IndexError, ValueError):
        return 0


async def wait_all_frozen(names, timeout=180):
    end = time.time() + timeout
    while time.time() < end:
        if all(frozen(n) for n in names):
            return True
        await asyncio.sleep(1)
    return False


async def join(name):
    bot = Bot(name, "127.0.0.1", PROXY)
    t = time.time()
    await bot.connect()
    return bot, await bot.wait_join(t, 60)


async def switch(bot, server):
    t = await bot.command(f"server {server}")
    return await bot.wait_join(t, 60)


def fmt(seconds):
    return "failed" if seconds is None else f"{seconds:.2f} s"


async def main():
    print("waiting until every backend is frozen...", flush=True)
    await wait_all_frozen(NAMES)
    note("RAM of all 5 backends, frozen", f"{sum(ram_mb(n) for n in NAMES)} MB")

    print("\n1. a player joins the network while everything sleeps", flush=True)
    alice, t = await join("Alice")
    note("join (lobby thaws)", fmt(t))

    print("\n2. personal worlds: four players, each to their own sleeping world, at the same time", flush=True)
    others = [await join(n) for n in ["Bob", "Carol", "Dave", "Eve"]]
    for (_, t), name in zip(others, ["Bob", "Carol", "Dave", "Eve"]):
        note(f"{name} joins the lobby", fmt(t))
    bots = [b for b, _ in others]
    times = await asyncio.gather(*(switch(b, w) for b, w in zip(bots, WORLDS)))
    for w, t in zip(WORLDS, times):
        note(f"switch to own world {w} (thaws)", fmt(t))
    await asyncio.sleep(3)
    note("RAM of all 5 backends, all awake with players", f"{sum(ram_mb(n) for n in NAMES)} MB")

    print("\n3. visiting each other: switch between awake worlds", flush=True)
    awake = await asyncio.gather(*(switch(b, WORLDS[(i + 1) % len(WORLDS)]) for i, b in enumerate(bots)))
    note("switch between awake worlds", ", ".join(fmt(t) for t in awake))

    print("\n4. everyone leaves; the worlds save and freeze", flush=True)
    for b in bots + [alice]:
        await b.close()
    left = time.time()
    ok = await wait_all_frozen(NAMES)
    note("all backends frozen again", f"after {time.time() - left:.0f} s" if ok else "no")
    note("RAM of all 5 backends, frozen again", f"{sum(ram_mb(n) for n in NAMES)} MB")

    print("\n5. coming back later: join and go to a frozen world", flush=True)
    alice, t = await join("Alice")
    note("join (lobby thaws)", fmt(t))
    note(f"switch to {WORLDS[0]} (thaws)", fmt(await switch(alice, WORLDS[0])))
    note(f"back to {LOBBY} (awake)", fmt(await switch(alice, LOBBY)))
    await alice.close()

    print("\n6. the next evening: four players to their own frozen worlds at once (servers warmed up)", flush=True)
    ok = await wait_all_frozen(NAMES)
    joined = [await join(n) for n in ["Bob", "Carol", "Dave", "Eve"]]
    bots = [b for b, _ in joined]
    times = await asyncio.gather(*(switch(b, w) for b, w in zip(bots, WORLDS)))
    for w, t in zip(WORLDS, times):
        note(f"switch to own world {w} (thaws, 4 at once)", fmt(t))
    for b in bots:
        await b.close()

    print("\nResults")
    for test, value in results:
        print(f"| {test} | {value} |")


asyncio.run(main())
