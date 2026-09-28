#!/usr/bin/env python3
"""Slice the action-icon sheet into the seven PNGs the desktop app shows.

The sheet comes from one Higgsfield generation (prompt in icons-prompt.txt):
one image rather than seven keeps lighting, clay texture and palette identical
across the family, which separate generations measurably did not.

Two notes, both from looking at real sheets:

**Finding the icons.** The model lays them out 4 over 3 but not on an exact
grid, so they are found from their opaque pixels — row bands first, then
column spans within each band, in reading order — not from fixed cells.

**The halo.** Each shape comes with a soft coloured glow in its alpha channel.
On the app's cream ground it reads as a smudge, so alpha below 110 is dropped
and 110-170 is ramped to full: the solid body and its antialiased edge stay.

Usage: python tools/make_icons.py path/to/sheet.png
"""
import pathlib
import sys
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/inklift-gui/ui/icons"
NAMES = ["open", "lift", "eraser", "undo", "clear", "save", "copy"]
SIZE = 54  # 3x the 18 px the buttons show them at
SOLID = 200


def spans(occupied, gap, min_len):
    """Runs of True in `occupied`, bridging holes shorter than `gap`."""
    out, start, last = [], None, None
    for i, v in enumerate(occupied):
        if v:
            if start is None:
                start = i
            last = i
        elif start is not None and i - last > gap:
            if last - start >= min_len:
                out.append((start, last))
            start = None
    if start is not None and last - start >= min_len:
        out.append((start, last))
    return out


def dehalo(v):
    if v < 110:
        return 0
    if v <= 170:
        return (v - 110) * 255 // 60
    return v


def main(sheet_path):
    sheet = Image.open(sheet_path).convert("RGBA")
    w, h = sheet.size
    solid = sheet.getchannel("A").point(lambda v: 255 if v > SOLID else 0)
    gap, min_len = max(8, w // 40), max(16, w // 25)

    boxes = []
    for y0, y1 in spans([bool(solid.crop((0, y, w, y + 1)).getbbox()) for y in range(h)], gap, min_len):
        band = solid.crop((0, y0, w, y1 + 1))
        for x0, x1 in spans([bool(band.crop((x, 0, x + 1, y1 - y0 + 1)).getbbox()) for x in range(w)], gap, min_len):
            bx = solid.crop((x0, y0, x1 + 1, y1 + 1)).getbbox()
            boxes.append((x0 + bx[0], y0 + bx[1], x0 + bx[2], y0 + bx[3]))

    if len(boxes) != len(NAMES):
        sys.exit(f"found {len(boxes)} icons in {sheet_path}, expected {len(NAMES)}: {boxes}")

    OUT.mkdir(parents=True, exist_ok=True)
    for name, (x0, y0, x1, y1) in zip(NAMES, boxes):
        side = max(x1 - x0, y1 - y0)
        side += 2 * int(side * 0.06)  # padding keeps the corners transparent
        cx, cy = (x0 + x1) // 2, (y0 + y1) // 2
        icon = sheet.crop((cx - side // 2, cy - side // 2, cx - side // 2 + side, cy - side // 2 + side))
        r, g, b, a = icon.split()
        icon = Image.merge("RGBA", (r, g, b, a.point(dehalo)))
        icon.resize((SIZE, SIZE), Image.LANCZOS).save(OUT / f"{name}.png", optimize=True)
        print(f"{name}: box {x1 - x0}x{y1 - y0} -> {OUT / (name + '.png')}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
