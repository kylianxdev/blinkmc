"""A minimal Minecraft 26.3 (protocol 777) client for testing Blink behind BungeeCord or Velocity.

It logs in (offline mode), goes through the configuration phase, answers keep-alives, can type
commands like /server and follows server switches. Packet ids come from the server's own data
generator (java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports).
"""
import asyncio
import re
import struct
import time
import uuid
import zlib

PROTOCOL = 777
# clientbound / serverbound packet ids for protocol 777
LOGIN_IN = {"disconnect": 0, "finished": 2, "compression": 3, "custom_query": 4}
LOGIN_OUT = {"hello": 0, "custom_query_answer": 2, "acknowledged": 3}
CONFIG_IN = {"disconnect": 2, "finish": 3, "keep_alive": 4, "ping": 5, "known_packs": 15}
CONFIG_OUT = {"known_packs": 7, "finish": 3, "keep_alive": 4, "pong": 5}
PLAY_IN = {"disconnect": 32, "keep_alive": 45, "login": 50, "ping": 62, "position": 73, "start_configuration": 120, "system_chat": 124}
PLAY_OUT = {"accept_teleportation": 0, "chat_command": 7, "configuration_acknowledged": 16, "keep_alive": 28, "pong": 45}


def varint(n):
    n &= 0xFFFFFFFF
    out = b""
    while True:
        b, n = n & 0x7F, n >> 7
        out += bytes([b | (0x80 if n else 0)])
        if not n:
            return out


def read_varint(data, i=0):
    n = shift = 0
    while True:
        b = data[i]
        i += 1
        n |= (b & 0x7F) << shift
        shift += 7
        if not b & 0x80:
            return (n - (1 << 32) if n & (1 << 31) else n), i


def string(s):
    b = s.encode()
    return varint(len(b)) + b


def text_of(data):
    """The readable text in a chat component (NBT or JSON), roughly."""
    return " ".join(re.findall(r"[ -~]{3,}", data.decode("latin-1"))).replace("text", "").strip()


class Bot:
    def __init__(self, name, host, port, log=print):
        self.name, self.host, self.port, self.log = name, host, port, log
        self.state = "login"
        self.threshold = -1
        self.joins = []        # times a play "login" arrived (joined a server)
        self.chat = []
        self.disconnected = None
        self.joined = asyncio.Event()
        self.trace = []

    async def connect(self):
        self.started = time.time()
        self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
        host = self.host.encode()
        self.send_raw(varint(0) + varint(PROTOCOL) + varint(len(host)) + host + struct.pack(">H", self.port) + varint(2))
        self.send_raw(varint(LOGIN_OUT["hello"]) + string(self.name) + uuid.uuid3(uuid.NAMESPACE_OID, self.name).bytes)
        self.task = asyncio.create_task(self.loop())

    def send_raw(self, packet):
        if self.threshold >= 0:
            packet = varint(0) + packet          # below the threshold: not compressed
        self.writer.write(varint(len(packet)) + packet)

    def send(self, packet_id, data=b""):
        self.send_raw(varint(packet_id) + data)

    async def read_packet(self):
        length = 0
        shift = 0
        while True:
            b = (await self.reader.readexactly(1))[0]
            length |= (b & 0x7F) << shift
            shift += 7
            if not b & 0x80:
                break
        data = await self.reader.readexactly(length)
        if self.threshold >= 0:
            size, i = read_varint(data)
            data = zlib.decompress(data[i:]) if size else data[i:]
        packet_id, i = read_varint(data)
        return packet_id, data[i:]

    async def loop(self):
        try:
            while True:
                pid, data = await self.read_packet()
                self.handle(pid, data)
                await self.writer.drain()
        except (asyncio.IncompleteReadError, ConnectionError) as e:
            if self.disconnected is None:
                self.disconnected = f"connection closed ({type(e).__name__})"
        finally:
            self.joined.set()

    def handle(self, pid, data):
        self.trace.append((time.time(), self.state, pid))
        if self.state == "login":
            if pid == LOGIN_IN["compression"]:
                self.threshold, _ = read_varint(data)
            elif pid == LOGIN_IN["custom_query"]:
                message_id, _ = read_varint(data)
                self.send(LOGIN_OUT["custom_query_answer"], varint(message_id) + b"\x00")
            elif pid == LOGIN_IN["finished"]:
                self.send(LOGIN_OUT["acknowledged"])
                self.state = "config"
            elif pid == LOGIN_IN["disconnect"]:
                self.disconnected = text_of(data)
        elif self.state == "config":
            if pid == CONFIG_IN["known_packs"]:
                self.send(CONFIG_OUT["known_packs"], varint(0))
            elif pid == CONFIG_IN["keep_alive"]:
                self.send(CONFIG_OUT["keep_alive"], data[:8])
            elif pid == CONFIG_IN["ping"]:
                self.send(CONFIG_OUT["pong"], data[:4])
            elif pid == CONFIG_IN["finish"]:
                self.send(CONFIG_OUT["finish"])
                self.state = "play"
            elif pid == CONFIG_IN["disconnect"]:
                self.disconnected = text_of(data)
        else:
            if pid == PLAY_IN["login"]:
                self.joins.append(time.time())
                self.joined.set()
            elif pid == PLAY_IN["keep_alive"]:
                self.send(PLAY_OUT["keep_alive"], data[:8])
            elif pid == PLAY_IN["ping"]:
                self.send(PLAY_OUT["pong"], data[:4])
            elif pid == PLAY_IN["start_configuration"]:
                self.send(PLAY_OUT["configuration_acknowledged"])
                self.state = "config"
            elif pid == PLAY_IN["system_chat"]:
                self.chat.append((time.time(), text_of(data)))
            elif pid == PLAY_IN["disconnect"]:
                self.disconnected = text_of(data)

    async def wait_join(self, after, timeout):
        """Seconds from `after` until the next server join, or None (and why) on timeout."""
        end = time.time() + timeout
        while time.time() < end:
            fresh = [t for t in self.joins if t >= after]
            if fresh:
                return fresh[0] - after
            if self.disconnected:
                return None
            await asyncio.sleep(0.02)
        return None

    async def command(self, command):
        t = time.time()
        self.send(PLAY_OUT["chat_command"], string(command))
        await self.writer.drain()
        return t

    async def close(self):
        self.writer.close()
        try:
            await self.writer.wait_closed()
        except Exception:
            pass
