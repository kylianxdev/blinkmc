"""Draws the website's pixel art (our own, in Minecraft's style; no Mojang textures).

    python3 docs/art.py      # writes docs/assets/*.png

Tiny palette PNGs that the browser scales up with image-rendering: pixelated, like Minecraft does.
Needs Pillow.
"""
import math
import os
import random

from PIL import Image

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "assets")


def save(img, name, colors=32):
    img = img.convert("RGB").quantize(colors=colors, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.NONE)
    img.save(os.path.join(OUT, name), optimize=True)
    print(f"{name}: {img.size[0]}x{img.size[1]}, {os.path.getsize(os.path.join(OUT, name))} bytes")


def noise(rnd, base, spread):
    """One of four fixed shades of a color, like Minecraft's textures (not random noise)."""
    f = rnd.choice((0.86, 0.94, 1.0, 1.07)) if spread else 1.0
    return tuple(max(0, min(255, int(c * f))) for c in base)


def landscape():
    """A night scene: the server sleeps. Sky, stars, moon, hills of grass, dirt and stone, trees, a cabin."""
    W, H = 320, 96
    rnd = random.Random(7)
    img = Image.new("RGB", (W, H))
    px = img.load()
    # sky: banded gradient, like Minecraft's sky at night
    for y in range(H):
        t = y / H
        band = int(t * 8) / 8
        c = (int(11 + 22 * band), int(16 + 28 * band), int(38 + 52 * band))
        for x in range(W):
            px[x, y] = c
    # stars
    for _ in range(90):
        x, y = rnd.randrange(W), rnd.randrange(int(H * 0.55))
        px[x, y] = (230, 236, 255) if rnd.random() < 0.6 else (150, 160, 210)
    # moon: a square with craters
    mx, my = 252, 10
    for y in range(8):
        for x in range(8):
            px[mx + x, my + y] = (226, 230, 214) if (x, y) not in {(2, 2), (5, 4), (3, 6), (6, 1)} else (184, 188, 170)
    # clouds: flat blocky slabs
    for cx, cy, w in [(30, 14, 26), (120, 8, 34), (190, 20, 22), (290, 30, 18)]:
        for x in range(w):
            for y in range(3):
                if 0 < x < w - 1 or y == 1:
                    px[(cx + x) % W, cy + y] = (52, 64, 112)
    # terrain: two layers of hills
    def ground(seed, base, amp, colors, ore=True):
        r = random.Random(seed)
        phase = [r.random() * 6 for _ in range(3)]
        heights = []
        for x in range(W):
            h = base + amp * (0.55 * math.sin(x / 37 + phase[0]) + 0.3 * math.sin(x / 13 + phase[1]) + 0.15 * math.sin(x / 5 + phase[2]))
            heights.append(int(round(h)))
        # flatten in steps, so it looks like blocks
        heights = [heights[x - x % 2] for x in range(W)]
        grass, dirt, stone = colors
        for x in range(W):
            top = heights[x]
            for y in range(max(0, top), H):
                d = y - top
                if d == 0:
                    c = noise(r, grass, 10)
                elif d == 1 and r.random() < 0.6:
                    c = noise(r, grass, 8)
                elif d < 5:
                    c = noise(r, dirt, 9)
                else:
                    c = noise(r, stone, 8)
                    if ore and (x // 2 * 7 + y // 2 * 13) % 97 == 0 and d > 7:
                        c = (176, 120, 46)          # a bit of ore
                px[x, y] = c
        return heights
    ground(3, 56, 8, ((40, 66, 54), (40, 42, 58), (38, 44, 64)), ore=False)  # far hills, in shadow
    near = ground(11, 76, 6, ((62, 120, 52), (96, 66, 44), (78, 80, 92)))  # near hills
    # trees on the near hills
    for tx in [22, 61, 98, 171, 214, 268, 301]:
        top = near[tx]
        for y in range(1, 8):
            px[tx, top - y] = noise(rnd, (96, 70, 42), 6)
        for dy in range(-13, -6):
            for dx in range(-4, 5):
                corner = abs(dx) == 4 and dy in (-13, -7)
                if not corner and rnd.random() < 0.94:
                    px[tx + dx, top + dy] = noise(rnd, (40, 96, 44), 12)
    # a small cabin with a lit window
    cx = 138
    top = near[cx]
    for y in range(top - 9, top):
        for x in range(cx - 7, cx + 8):
            px[x, y] = noise(rnd, (124, 92, 56), 8)
    for i in range(9):
        for x in range(cx - 9 + i, cx + 10 - i):
            px[x, top - 9 - i // 2] = noise(rnd, (86, 52, 40), 6)
    for y in range(top - 6, top - 3):
        for x in range(cx + 2, cx + 5):
            px[x, y] = (250, 214, 110)
    for y in range(top - 5, top):
        for x in range(cx - 4, cx - 1):
            px[x, y] = (70, 46, 30)
    # torches
    for tx in [116, 158, 236]:
        top = near[tx]
        for y in (1, 2):
            px[tx, top - y] = (110, 80, 50)
        px[tx, top - 3] = (255, 200, 80)
    save(img, "landscape.png", 64)


def logo():
    eye = ["....XXXXXXXX....",
           "..XXooooooooXX..",
           ".XoowwwwwwwwooX.",
           "XoowwwbbbbwwwooX",
           "XowwwbbkkbbwwwoX",
           "XowwwbbkkbbwwwoX",
           "XoowwwbbbbwwwooX",
           ".XoowwwwwwwwooX.",
           "..XXooooooooXX..",
           "....XXXXXXXX...."]
    col = {"X": (27, 31, 59), "o": (61, 79, 168), "w": (232, 236, 255), "b": (54, 196, 107), "k": (11, 16, 38)}
    img = Image.new("RGBA", (16, 10), (0, 0, 0, 0))
    for y, row in enumerate(eye):
        for x, c in enumerate(row):
            if c in col:
                img.putpixel((x, y), col[c] + (255,))
    img.save(os.path.join(OUT, "eye.png"), optimize=True)
    print(f"eye.png: {os.path.getsize(os.path.join(OUT, 'eye.png'))} bytes")


def blocks():
    """16x16 block faces in one strip: grass, ice, stone, gold."""
    rnd = random.Random(5)
    img = Image.new("RGB", (64, 16))
    px = img.load()
    for x in range(16):
        edge = 3 + (rnd.random() < 0.5) + (rnd.random() < 0.3)
        for y in range(16):
            px[x, y] = noise(rnd, (86, 160, 60), 14) if y < edge else noise(rnd, (134, 96, 62), 14)
    for x in range(16):
        for y in range(16):
            c = noise(rnd, (140, 186, 246), 10)
            if (x + y) % 9 == 0 or (x - y) % 11 == 0:
                c = (214, 232, 255)
            px[16 + x, y] = c
    for x in range(16):
        for y in range(16):
            px[32 + x, y] = noise(rnd, (124, 124, 128), 18)
    for x in range(16):
        for y in range(16):
            c = noise(rnd, (236, 196, 64), 16)
            if x in (0, 15) or y in (0, 15):
                c = (188, 140, 30)
            px[48 + x, y] = c
    save(img, "blocks.png", 32)
    save(img.crop((0, 0, 16, 16)), "grass.png", 16)


def tree():
    """A big oak on a small grass island, for the "greener" part of the page."""
    W, H = 48, 56
    rnd = random.Random(21)
    img = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    px = img.load()
    def put(x, y, c):
        if 0 <= x < W and 0 <= y < H:
            px[x, y] = c + (255,)
    # leaves: overlapping blobs, lit from the top left, darker at the bottom
    greens = [(26, 74, 30), (38, 98, 40), (52, 124, 50), (74, 152, 62), (104, 182, 78)]
    blobs = [(24, 14, 13), (14, 20, 9), (34, 20, 9), (19, 9, 7), (30, 8, 7), (24, 24, 10), (9, 25, 6), (39, 26, 6)]
    for y in range(H):
        for x in range(W):
            inside = [b for b in blobs if (x - b[0]) ** 2 + ((y - b[1]) * 1.15) ** 2 <= b[2] ** 2]
            if not inside:
                continue
            # light: higher and more to the left is brighter, the bottom edge is in shadow
            b = min(inside, key=lambda b: (x - b[0]) ** 2 + (y - b[1]) ** 2)
            light = ((b[0] - x) * 0.6 + (b[1] - y)) / b[2]
            shade = 2 + round(light * 1.6) + rnd.choice((-1, 0, 0, 0, 1))
            if y > 26:
                shade -= 1
            put(x, y, greens[max(0, min(4, shade))])
    # a few apples
    for x, y in [(17, 15), (31, 12), (26, 22), (12, 24), (37, 22)]:
        put(x, y, (200, 44, 40)); put(x, y - 1, (90, 60, 30))
    # trunk with bark, and two branches into the leaves
    bark = [(70, 48, 30), (92, 64, 40), (112, 80, 50)]
    for y in range(26, 46):
        for x in range(21, 27):
            c = bark[1]
            if x == 21 or (x + y * 3) % 7 == 0:
                c = bark[0]
            elif x == 25 and y % 4:
                c = bark[2]
            put(x, y, c)
    for i in range(6):
        put(20 - i, 30 - i, bark[0]); put(27 + i, 31 - i, bark[0]); put(27 + i, 30 - i, bark[1])
    # roots
    for x, y in [(19, 45), (20, 45), (27, 45), (28, 45)]:
        put(x, y, bark[0])
    # the island: grass on top, dirt below, getting smaller towards the bottom
    for y in range(46, H):
        d = y - 46
        half = 21 - d * 2
        for x in range(24 - half, 24 + half):
            if d == 0:
                c = noise(rnd, (86, 160, 60), 1)
            elif d == 1 and rnd.random() < 0.5:
                c = noise(rnd, (70, 136, 50), 1)
            elif d < 7:
                c = noise(rnd, (134, 96, 62), 1)
            else:
                c = noise(rnd, (110, 110, 116), 1)
            put(x, y, c)
    # grass tufts and flowers on the island
    for x in (6, 9, 15, 31, 36, 41):
        put(x, 45, (96, 170, 64))
    for x, c in [(8, (240, 214, 60)), (13, (220, 70, 70)), (34, (240, 214, 60)), (39, (170, 120, 230))]:
        put(x, 44, c); put(x, 45, (60, 120, 44))
    img.save(os.path.join(OUT, "tree.png"), optimize=True)
    print(f"tree.png: {W}x{H}, {os.path.getsize(os.path.join(OUT, 'tree.png'))} bytes")


# 16x16 icons, drawn as text: one letter per pixel, "." is transparent
ICON_COLORS = {
    "y": (250, 214, 64), "Y": (255, 242, 150), "o": (226, 150, 40), "O": (180, 100, 30),
    "w": (236, 236, 222), "g": (170, 170, 158), "G": (120, 120, 112),
    "b": (122, 82, 48), "B": (86, 56, 32), "l": (160, 112, 64),
    "s": (150, 150, 156), "S": (100, 100, 108), "c": (110, 210, 230), "C": (60, 150, 190),
    "r": (200, 50, 46), "R": (140, 30, 30), "p": (236, 220, 196), "k": (30, 30, 36),
    "e": (74, 160, 60), "E": (40, 110, 40), "m": (222, 226, 206), "M": (180, 184, 166),
    "f": (255, 170, 40), "F": (255, 230, 120), "v": (150, 90, 200), "u": (60, 90, 200),
}
ICONS = {
    "sun": [
        "................", "...oooooooooo...", "..oyyyyyyyyyyo..", ".oyYYyyyyyyyyyo.",
        ".oyYyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.",
        ".oyyyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.",
        ".oyyyyyyyyyyyOo.", "..oyyyyyyyyyOo..", "...oooooooooo...", "................"],
    "moon": [
        "................", "...MMMMMMMMMM...", "..MmmmmmmmmmmM..", ".MmmmGmmmmmmmmM.",
        ".MmmGGmmmmmmmmM.", ".MmmmmmmmmGmmmM.", ".MmmmmmmmmmmmmM.", ".MmmmmmmmmmmmmM.",
        ".MmmmmmmGGmmmmM.", ".MmGmmmmGGmmmmM.", ".MmmmmmmmmmmmmM.", ".MmmmmmmmmmmGmM.",
        ".MmmmmmmmmmmmmM.", "..MmmmmmmmmmmM..", "...MMMMMMMMMM...", "................"],
    "bed": [
        "................", "................", "................", "................",
        "................", "..ww............", ".wwwwrrrrrrrrrr.", ".wwwwrrrrrrrrrr.",
        ".wwwgRRRRRRRRRR.", ".bbbbbbbbbbbbbb.", ".llllllllllllll.", ".b............b.",
        ".b............b.", "................", "................", "................"],
    "pick": [
        "................", "...ccccccc......", "..cCCCCCCCcc....", ".cC.......Ccc...",
        ".c.......BCcc...", ".........bBCc...", "........bB.Cc...", ".......bB..cc...",
        "......bB...c....", ".....bB.........", "....bB..........", "...bB...........",
        "..bB............", ".bB.............", "................", "................"],
    "book": [
        "................", "..RRRRRRRRRRR...", "..RrrrrrrrrrRp..", "..RryyyyyyyrRp..",
        "..RrrrrrrrrrRp..", "..RrrrrrrrrrRp..", "..RrrrrrrrrrRp..", "..RrrrrrrrrrRp..",
        "..RrrrrrrrrrRp..", "..RrrrrrrrrrRp..", "..RrrrrrrrrrRp..", "..RrrrrrrrrrRp..",
        "..RRRRRRRRRRRp..", "...ppppppppppp..", "................", "................"],
    "clock": [
        "................", ".....oooooo.....", "...ooyyyyyyoo...", "..oyyyyGyyyyyo..",
        "..oyyyyGyyyyyo..", ".oyyyyyGyyyyyyo.", ".oyyyyyGyyyyyyo.", ".oyyyyyGGGGyyyo.",
        ".oyyyyyyyyyyyyo.", ".oyyyyyyyyyyyyo.", "..oyyyyyyyyyyo..", "..oyyyyyyyyyyo..",
        "...ooyyyyyyoo...", ".....oooooo.....", "................", "................"],
    "torch": [
        "................", ".......F........", "......FfF.......", "......fff.......",
        "......fFf.......", ".......b........", ".......b........", ".......B........",
        ".......b........", ".......B........", ".......b........", ".......B........",
        ".......b........", ".......B........", "................", "................"],
    "chest": [
        "................", "................", "..BBBBBBBBBBBB..", "..BllllllllllB..",
        "..BllllllllllB..", "..BbbbbsSbbbbB..", "..BBBBBsSBBBBB..", "..BllllllllllB..",
        "..BllllllllllB..", "..BllllllllllB..", "..BllllllllllB..", "..BbbbbbbbbbbB..",
        "..BBBBBBBBBBBB..", "................", "................", "................"],
    "sapling": [
        "................", "......ee........", ".....eEee.......", "....eeEeee..e...",
        "...eeeeeE..eEe..", "....eEee..eeee..", ".......b.eeE....", ".......bee......",
        "......eb........", ".....eEb........", "......eb........", ".......b........",
        ".......B........", "....llllllll....", "....bbbbbbbb....", "................"],
}


def icons():
    names = list(ICONS)
    img = Image.new("RGBA", (16 * len(names), 16), (0, 0, 0, 0))
    for i, name in enumerate(names):
        for y, row in enumerate(ICONS[name]):
            for x, ch in enumerate(row):
                if ch in ICON_COLORS:
                    img.putpixel((i * 16 + x, y), ICON_COLORS[ch] + (255,))
    img.save(os.path.join(OUT, "icons.png"), optimize=True)
    print(f"icons.png: {' '.join(names)}, {os.path.getsize(os.path.join(OUT, 'icons.png'))} bytes")


def figures():
    """32x32 pictures: two players (our own, not Steve), a PC and a server rack."""
    rnd = random.Random(9)
    img = Image.new("RGBA", (128, 32), (0, 0, 0, 0))
    px = img.load()
    def rect(x0, y0, w, h, c):
        for y in range(y0, y0 + h):
            for x in range(x0, x0 + w):
                px[x, y] = c + (255,)
    def player(ox, hair, skin, shirt, pants, cap=None):
        rect(ox + 11, 2, 10, 10, skin)                      # head
        rect(ox + 11, 2, 10, 3, hair)
        rect(ox + 11, 5, 2, 2, hair); rect(ox + 19, 5, 2, 2, hair)
        rect(ox + 13, 7, 2, 2, (255, 255, 255)); rect(ox + 17, 7, 2, 2, (255, 255, 255))
        rect(ox + 14, 7, 1, 2, (40, 60, 120)); rect(ox + 18, 7, 1, 2, (40, 60, 120))
        rect(ox + 14, 10, 4, 1, (150, 80, 60))
        if cap:
            rect(ox + 10, 1, 12, 2, cap); rect(ox + 10, 3, 14, 1, cap)
        rect(ox + 11, 12, 10, 10, shirt)                    # body
        rect(ox + 8, 12, 3, 9, shirt); rect(ox + 21, 12, 3, 9, shirt)
        rect(ox + 8, 20, 3, 2, skin); rect(ox + 21, 20, 3, 2, skin)
        rect(ox + 11, 22, 10, 8, pants)                     # legs
        rect(ox + 15, 24, 2, 6, tuple(max(0, c - 30) for c in pants))
        rect(ox + 11, 29, 4, 2, (60, 60, 64)); rect(ox + 17, 29, 4, 2, (60, 60, 64))
    player(0, (90, 56, 30), (232, 180, 140), (56, 150, 90), (60, 70, 140))
    player(32, (230, 190, 90), (240, 196, 160), (190, 70, 70), (70, 60, 50), cap=(60, 90, 200))
    # a PC: monitor with a little world on it, keyboard
    rect(66, 3, 28, 20, (40, 40, 46))
    rect(68, 5, 24, 16, (110, 160, 230))
    for x in range(68, 92):
        h = 14 + (x * 7 % 5 == 0)
        rect(x, 5 + h - 1, 1, 16 - h + 1, (86, 160, 60))
        rect(x, 5 + h + 1, 1, max(0, 16 - h - 1), (134, 96, 62))
    rect(86, 8, 3, 3, (250, 214, 64))
    rect(78, 23, 4, 3, (60, 60, 66)); rect(72, 26, 16, 2, (60, 60, 66))
    rect(66, 29, 28, 3, (80, 80, 88))
    for x in range(67, 93, 2):
        px[x, 30] = (140, 140, 150, 255)
    # a server rack with blinking lights: one row awake (green), the rest asleep (blue)
    rect(100, 1, 24, 30, (30, 32, 40))
    for i, y in enumerate(range(3, 29, 5)):
        rect(102, y, 20, 4, (64, 66, 78))
        rect(104, y + 1, 2, 2, (93, 201, 63) if i in (1, 3) else (124, 156, 255))
        for x in range(108, 120, 2):
            px[x, y + 2] = (40, 42, 50, 255)
    img.save(os.path.join(OUT, "figures.png"), optimize=True)
    print(f"figures.png: {os.path.getsize(os.path.join(OUT, 'figures.png'))} bytes")


def clouds():
    """Flat Minecraft clouds for the night sky, drifting slowly."""
    img = Image.new("RGBA", (160, 24), (0, 0, 0, 0))
    for cx, cy, w, h in [(6, 6, 34, 6), (14, 3, 16, 3), (70, 12, 46, 6), (82, 9, 22, 3), (128, 4, 26, 5)]:
        for y in range(cy, cy + h):
            for x in range(cx, cx + w):
                img.putpixel((x, y), (70, 84, 140, 150))
    img.save(os.path.join(OUT, "clouds.png"), optimize=True)
    print(f"clouds.png: {os.path.getsize(os.path.join(OUT, 'clouds.png'))} bytes")


def ground():
    """A 64x48 strip for the footer: grass blocks, dirt and stone with a little ore, four blocks wide
    so it doesn't repeat every block."""
    rnd = random.Random(13)
    img = Image.new("RGB", (64, 48))
    px = img.load()
    for x in range(64):
        edge = 3 + (rnd.random() < 0.5) + (rnd.random() < 0.3)
        for y in range(16):
            px[x, y] = noise(rnd, (86, 160, 60), 1) if y < edge else noise(rnd, (134, 96, 62), 1)
        for y in range(16, 32):
            px[x, y] = noise(rnd, (134, 96, 62), 1)
        for y in range(32, 48):
            px[x, y] = noise(rnd, (118, 118, 124), 1)
    # ore: gold in one stone block, coal in another, in small clusters
    for cx, cy, color in [(5, 37, (236, 196, 64)), (38, 41, (40, 40, 44)), (54, 35, (40, 40, 44))]:
        for dx, dy in [(0, 0), (1, 0), (1, 1), (2, 1), (0, 2)]:
            px[cx + dx, cy + dy] = color
    save(img, "ground.png", 24)

if __name__ == "__main__":
    landscape()
    logo()
    blocks()
    tree()
    icons()
    figures()
    clouds()
    ground()
