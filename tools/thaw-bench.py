"""How long a player waits when switching (through the proxy) to a frozen server, until in game.

    python3 tools/thaw-bench.py <proxy port> <server name> <tmux socket of that server's blink> [runs]

A bot waits in the lobby; every run waits until the server is frozen and its memory moved out,
switches to it, measures until the new server's "login" packet arrives, and switches back.
"""
import asyncio
import subprocess
import sys
import time

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from netbot import Bot

PORT, SERVER, SOCKET = int(sys.argv[1]), sys.argv[2], sys.argv[3]
RUNS = int(sys.argv[4]) if len(sys.argv) > 4 else 3


def reclaims():
    out = subprocess.run(["tmux", "-L", SOCKET, "capture-pane", "-p", "-t", SOCKET.split("-", 1)[1], "-S", "-5000"],
                         capture_output=True, text=True).stdout
    return out.count("Frozen server's memory")


async def main():
    bot = Bot("Bench", "127.0.0.1", PORT)
    t = time.time()
    await bot.connect()
    await bot.wait_join(t, 60)
    times = []
    for _ in range(RUNS):
        before = reclaims()
        while reclaims() == before:          # wait for the next freeze + reclaim
            await asyncio.sleep(1)
        await asyncio.sleep(1)
        bot.trace = []
        t = await bot.command(f"server {SERVER}")
        d = await bot.wait_join(t, 60)
        times.append(None if d is None else round(d, 2))
        if d and d > 2 and "--trace" in sys.argv:
            last = t
            print(f"slow run ({d:.2f} s), gaps over 0.3 s:")
            for (when, state, pid) in bot.trace:
                if when - last > 0.3:
                    print(f"  +{when - t:5.2f} s  {state} packet {pid} after {when - last:.2f} s quiet")
                last = when
        await asyncio.sleep(1)
        t = await bot.command("server lobby")
        await bot.wait_join(t, 60)
    print(f"{SERVER}: thaw switch times {times}")
    await bot.close()


asyncio.run(main())
