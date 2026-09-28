#!/usr/bin/env python3
"""Render the tray icon from the project's logo mark.

Two notes on choices made here, both settled by looking rather than guessing.

**Source.** A pixel grid drawn for 16px (once `design/logo/tray-16.svg`) was tried. Rendered
and compared against the smooth mark at a real panel size of 24px, the pixel
version was muddy and ambiguous while the smooth mark stayed legible, so the
mark wins. The full mark is used rather than the bare glyph because its baseline
bar is the "ink lifting off the line" idea the whole identity rests on.

**Colour.** The mark is monochrome light. A PNG cannot follow the panel theme
the way `fill="currentColor"` would, and the panel this was built against
measured 42-58/255 mean luminance — dark, like most Linux panels. A light icon
on a light panel would disappear, so a faint dark halo gives it an edge there.
"""
import pathlib
from PIL import Image, ImageFilter

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "design/logo/mark-1024.png"
OUT = ROOT / "crates/inklift-gui/icons/tray.png"
SIZE = 64

INK = (0xE7, 0xEC, 0xEF)   # --gra-50
HALO = (0x0E, 0x11, 0x13)  # --gra-900

mark = Image.open(SRC).convert("RGBA")
mark = mark.crop(mark.getbbox())

# square it without distorting, then work at the icon size
side = max(mark.size)
square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
square.alpha_composite(mark, ((side - mark.width) // 2, (side - mark.height) // 2))
alpha = square.resize((SIZE, SIZE), Image.LANCZOS).getchannel("A")

glyph = Image.new("RGBA", (SIZE, SIZE), INK + (0,))
glyph.putalpha(alpha)

# a soft dark edge so the icon survives a light panel too
spread = alpha.filter(ImageFilter.MaxFilter(3)).filter(ImageFilter.GaussianBlur(1.1))
halo = Image.new("RGBA", (SIZE, SIZE), HALO + (0,))
halo.putalpha(spread.point(lambda v: int(v * 0.45)))

icon = Image.alpha_composite(halo, glyph)
OUT.parent.mkdir(parents=True, exist_ok=True)
icon.save(OUT)
print(f"wrote {OUT.relative_to(ROOT)}  {icon.size[0]}x{icon.size[1]}")
