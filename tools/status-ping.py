"""Server list ping, or a fake join that wakes a server behind Blink.

    python3 tools/status-ping.py example.org            # what the server list shows
    python3 tools/status-ping.py example.org --join     # wakes the server, prints how long it took
    python3 tools/status-ping.py example.org --join --name Steve   # a name on whitelist.json

The test join stops at the encryption request, so it never logs in and never shows up in the game.

A join is answered with an encryption request (packet 0x01) once the real server has the player,
or with a disconnect message (0x00) when Blink kicks instead.
"""
import json
import socket
import struct
import sys
import time
import uuid


def varint(n):
    out = b""
    while True:
        b, n = n & 0x7F, n >> 7
        out += bytes([b | (0x80 if n else 0)])
        if not n:
            return out


def read_varint(s):
    n = shift = 0
    while True:
        b = s.recv(1)[0]
        n |= (b & 0x7F) << shift
        shift += 7
        if not b & 0x80:
            return n


def packet(data):
    return varint(len(data)) + data


def connect(host, port, state, protocol):
    s = socket.create_connection((host, port), timeout=60)
    name = host.encode()
    s.send(packet(varint(0) + varint(protocol) + varint(len(name)) + name + struct.pack(">H", port) + varint(state)))
    return s


def status(host, port):
    s = connect(host, port, 1, 0)
    s.send(packet(b"\x00"))
    read_varint(s), read_varint(s)
    size = read_varint(s)
    data = b""
    while len(data) < size:
        data += s.recv(size - len(data))
    return json.loads(data)


def main():
    host, port = sys.argv[1], 25565
    if ":" in host:
        host, port = host.rsplit(":", 1)
        port = int(port)
    info = status(host, port)
    if "--join" not in sys.argv:
        info["favicon"] = "yes" if "favicon" in info else "no"
        print(json.dumps(info, indent=1, ensure_ascii=False))
        return
    # join with the protocol the server list reports, or the real server kicks us as outdated
    s = connect(host, port, 2, info["version"]["protocol"])
    started = time.time()
    name = (sys.argv[sys.argv.index("--name") + 1] if "--name" in sys.argv else "Test").encode()
    s.send(packet(varint(0) + varint(len(name)) + name + uuid.uuid4().bytes))
    read_varint(s)
    kind = read_varint(s)
    took = time.time() - started
    if kind == 1:
        print(f"on the real server after {took:.1f} s (encryption request)")
    else:
        print(f"kicked after {took:.1f} s: " + s.recv(300).decode(errors="replace"))


if __name__ == "__main__":
    main()
