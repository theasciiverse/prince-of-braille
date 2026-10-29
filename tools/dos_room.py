#!/usr/bin/env python3
"""Extract the DOS princess's room from PrinceJS (Unlicense).

Reads assets/gfx/cutscene.png and its atlas from the PrinceJS archive and
writes assets/dos/room.bin: the room with the bed laid on it as PrinceJS's
Scene does (bed at 0,142), the front pillar, the hourglass in its seven
states and the three frames of falling sand.

Format: palette count n, n RGB triples (index 0 is transparent), then each
image as width and height (u16, little-endian) and one index per pixel,
row by row from the top.

Usage: tools/dos_room.py [path-to-PrinceJS-zip]
"""

import io
import json
import os
import struct
import sys
import zipfile

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
DEFAULT_REF = os.path.join(HERE, "..", "..", "..", "references", "Prince of Persia", "PrinceJS-master.zip")
PREFIX = "PrinceJS-master/assets/gfx/"
NAMES = ["room_pillar"] + [f"clock0{i}" for i in range(1, 8)] + [f"clocksand0{i}" for i in range(1, 4)]


def main():
    z = zipfile.ZipFile(sys.argv[1] if len(sys.argv) > 1 else DEFAULT_REF)
    sheet = Image.open(io.BytesIO(z.read(PREFIX + "cutscene.png"))).convert("RGBA")
    atlas = json.loads(z.read(PREFIX + "cutscene.json"))["frames"]
    frames = atlas if isinstance(atlas, dict) else {f["filename"]: f for f in atlas}

    def crop(name):
        f = frames[name]["frame"]
        return sheet.crop((f["x"], f["y"], f["x"] + f["w"], f["y"] + f["h"]))

    room = crop("room")
    bed = crop("room_bed")
    room.alpha_composite(bed, (0, 142))
    images = [room] + [crop(n) for n in NAMES]

    palette = [None]
    index = {}
    out = []
    for im in images:
        px = []
        for p in im.getdata():
            if p[3] < 128:
                px.append(0)
                continue
            c = p[:3]
            if c not in index:
                index[c] = len(palette)
                palette.append(c)
            px.append(index[c])
        out.append(struct.pack("<HH", *im.size) + bytes(px))
    assert len(palette) <= 256
    data = bytes([len(palette) - 1]) + b"".join(bytes(c) for c in palette[1:]) + b"".join(out)
    os.makedirs(os.path.join(ROOT, "assets", "dos"), exist_ok=True)
    with open(os.path.join(ROOT, "assets", "dos", "room.bin"), "wb") as f:
        f.write(data)
    print(f"room.bin: {len(images)} images, {len(palette) - 1} colours, {len(data)} bytes")


if __name__ == "__main__":
    main()
