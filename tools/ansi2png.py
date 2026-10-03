#!/usr/bin/env python3
"""Render ANSI truecolor terminal text (from --dump with POB_ANSI=1) to PNG.

Usage: tools/ansi2png.py dump.txt out_prefix [cell_w cell_h]
Each '== ' header starts a new frame -> out_prefix-NNN.png
"""
import re, sys
from PIL import Image, ImageDraw, ImageFont

FONTS = ["/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
         "/usr/share/fonts/TTF/DejaVuSansMono.ttf"]

def main():
    src, prefix = sys.argv[1], sys.argv[2]
    cw = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    chh = int(sys.argv[4]) if len(sys.argv) > 4 else 16
    font = None
    for f in FONTS:
        try:
            font = ImageFont.truetype(f, chh - 2)
            break
        except OSError:
            pass
    frames, cur = [], None
    for line in open(src, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("== "):
            cur = [];  frames.append((line, cur)); continue
        if cur is not None:
            cur.append(line)
    tok = re.compile(r"\x1b\[38;2;(\d+);(\d+);(\d+)m\x1b\[48;2;(\d+);(\d+);(\d+)m(.)|\x1b\[0m")
    for n, (hdr, lines) in enumerate(frames):
        w = max(len(tok.findall(l)) for l in lines) or 1
        img = Image.new("RGB", (w * cw, len(lines) * chh), (0, 0, 0))
        d = ImageDraw.Draw(img)
        for y, l in enumerate(lines):
            x = 0
            for m in tok.finditer(l):
                if m.group(7) is None:
                    continue
                fg = tuple(int(m.group(i)) for i in (1, 2, 3))
                bg = tuple(int(m.group(i)) for i in (4, 5, 6))
                ch = m.group(7)
                d.rectangle([x * cw, y * chh, (x + 1) * cw - 1, (y + 1) * chh - 1], fill=bg)
                o = ord(ch)
                if o == 0x2588 or 0x2580 <= o <= 0x259f:
                    draw_block(d, o, x * cw, y * chh, cw, chh, fg)
                elif 0x2800 <= o <= 0x28ff:
                    draw_braille(d, o - 0x2800, x * cw, y * chh, cw, chh, fg)
                elif ch != " ":
                    d.text((x * cw, y * chh), ch, font=font, fill=fg)
                x += 1
        img.save(f"{prefix}-{n:03}.png")
        print(f"{prefix}-{n:03}.png  {hdr}")

def draw_braille(d, v, x, y, cw, ch, fg):
    # dots in a 2x4 grid, like a terminal font draws them
    bits = [(0, 0, 0x01), (0, 1, 0x02), (0, 2, 0x04), (1, 0, 0x08), (1, 1, 0x10), (1, 2, 0x20), (0, 3, 0x40), (1, 3, 0x80)]
    r = max(1, cw // 4)
    for cx, cy, b in bits:
        if v & b:
            px = x + cw * (cx * 2 + 1) // 4
            py = y + ch * (cy * 2 + 1) // 8
            d.ellipse([px - r, py - r, px + r - 1, py + r - 1], fill=fg)

def draw_block(d, o, x, y, cw, ch, fg):
    h2, w2 = ch // 2, cw // 2
    q = {  # quadrants: (tl, tr, bl, br)
        0x2598: (1,0,0,0), 0x259d: (0,1,0,0), 0x2596: (0,0,1,0), 0x2597: (0,0,0,1),
        0x259a: (1,0,0,1), 0x259e: (0,1,1,0), 0x2599: (1,0,1,1), 0x259b: (1,1,1,0),
        0x259c: (1,1,0,1), 0x259f: (0,1,1,1), 0x2588: (1,1,1,1), 0x2580: (1,1,0,0),
        0x2584: (0,0,1,1), 0x258c: (1,0,1,0), 0x2590: (0,1,0,1)}
    if o in q:
        tl, tr, bl, br = q[o]
        for on, (a, b) in zip((tl, tr, bl, br), ((0, 0), (1, 0), (0, 1), (1, 1))):
            if on:
                d.rectangle([x + a * w2, y + b * h2, x + (a + 1) * w2 - 1 if a == 0 else x + cw - 1, y + (b + 1) * h2 - 1 if b == 0 else y + ch - 1], fill=fg)
        return
    if o == 0x2582:  # lower quarter
        d.rectangle([x, y + ch * 3 // 4, x + cw - 1, y + ch - 1], fill=fg)
    elif o == 0x2586:  # lower three quarters
        d.rectangle([x, y + ch // 4, x + cw - 1, y + ch - 1], fill=fg)
    elif o == 0x2594:  # upper eighth
        d.rectangle([x, y, x + cw - 1, y + max(1, ch // 8) - 1], fill=fg)

if __name__ == "__main__":
    main()
