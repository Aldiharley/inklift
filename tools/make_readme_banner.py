#!/usr/bin/env python3
"""Compose the README banner.

Built from the project's own photography rather than generated, and assembled
here so it can be changed without re-running an image service.

The source is `hero-plate.jpg`, the clean plate of the same scene as
`hero.jpg` — the sheet is blank in it. The earlier banner used `hero.jpg`,
whose handwriting reads 证件样本 ("ID document sample"), which is not what a
banner for this project should say. Removing it by inpainting made a mess of
the paper; using the plate that was shot for exactly this purpose does not.

The handwriting is set in La Belle Aurore, chosen over Caveat and Zeyada by
rendering all three on the paper at final size: Caveat is too upright to read
as a signature and Zeyada too loose to read at all. Its ink colour is measured
from the pen in the original photograph, not picked.

The soft shadow under the strokes is the point of the whole image: ink lifted
off the page, casting a contact shadow onto the paper it came from.

Fonts are fetched at build time into a cache directory; nothing here is
committed except the finished JPEG.
"""
import pathlib
import re
import sys
import urllib.request
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLATE = ROOT / "design/assets/hero-plate.jpg"
HERO = ROOT / "design/assets/hero.jpg"
GLYPH = ROOT / "design/logo/glyph-1024.png"
OUT = ROOT / "design/assets/readme-banner.jpg"
CACHE = pathlib.Path(
    __import__("tempfile").gettempdir()
) / "inklift-banner-fonts"

W, H = 2560, 640
PRUSSIAN = (0x2B, 0x64, 0x86)
RAG700 = (0x4E, 0x48, 0x42)
# the paper recedes to the right; its rules run at this angle
PAPER_TILT = 9.6


def font(family: str, size: int) -> ImageFont.FreeTypeFont:
    """Fetch a Google font once, then reuse it."""
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / f"{family.replace(' ', '')}.ttf"
    if not path.exists():
        css_url = f"https://fonts.googleapis.com/css?family={family.replace(' ', '+')}"
        req = urllib.request.Request(css_url, headers={"User-Agent": "Mozilla/4.0"})
        css = urllib.request.urlopen(req, timeout=40).read().decode()
        m = re.search(r"https://[^)]+\.ttf", css)
        if not m:
            sys.exit(f"no TTF found for {family}")
        path.write_bytes(urllib.request.urlopen(m.group(0), timeout=40).read())
    return ImageFont.truetype(str(path), size)


def pen_colour() -> tuple:
    """The real ink from the original photograph, averaged over its darkest core."""
    hero = Image.open(HERO).convert("RGB")
    px = [hero.getpixel((x, y)) for y in range(560, 960, 3) for x in range(1100, 1800, 3)]
    px.sort(key=sum)
    core = px[:400]
    return tuple(sum(c[i] for c in core) // len(core) for i in range(3))


def handwriting(text: str, ink: tuple) -> Image.Image:
    """The word, its contact shadow, and the tilt of the page it sits on."""
    f = font("La Belle Aurore", 172)
    pad = 120
    box = Image.new("RGBA", (1500, 460), (0, 0, 0, 0))
    ImageDraw.Draw(box).text((pad, pad), text, font=f, fill=ink + (255,))

    # the shadow is the same strokes, dropped and softened — cast by ink that
    # has left the paper rather than printed on it
    shadow = Image.new("RGBA", box.size, (0, 0, 0, 0))
    ImageDraw.Draw(shadow).text((pad - 8, pad + 34), text, font=f, fill=(74, 78, 92, 255))
    shadow = shadow.filter(ImageFilter.GaussianBlur(11))
    shadow.putalpha(shadow.getchannel("A").point(lambda v: int(v * 0.42)))

    plate = Image.alpha_composite(shadow, box)
    return plate.rotate(PAPER_TILT, resample=Image.BICUBIC, expand=True)


def main() -> None:
    ink = pen_colour()
    plate = Image.open(PLATE).convert("RGB")
    # the same framing the hero crop had, rescaled for the plate's smaller frame
    band = plate.crop((0, 417, 2000, 917)).resize((W, H), Image.LANCZOS).convert("RGBA")

    hand = handwriting("Inklift", ink)
    # sits wholly on the sheet: further right and it runs off the edge onto
    # the bench, which reads as a mistake rather than a photograph
    band.alpha_composite(hand, (1010, -46))

    band = band.convert("RGB")
    d = ImageDraw.Draw(band)

    g = Image.open(GLYPH).convert("RGBA")
    g = g.crop(g.getbbox())
    gh = 198
    g = g.resize((int(g.width * gh / g.height), gh), Image.LANCZOS)
    stroke = Image.new("RGBA", g.size, PRUSSIAN + (0,))
    stroke.putalpha(g.getchannel("A"))

    wordmark = font("Archivo", 122)
    tagline = font("Archivo", 34)

    left, top = 296, 224
    band.paste(stroke, (left, top), stroke)
    tx = left + stroke.width + 54
    wb = d.textbbox((0, 0), "inklift", font=wordmark)
    cap_top = top + 22
    d.text((tx, cap_top - wb[1]), "inklift", font=wordmark, fill=PRUSSIAN)
    d.text((tx + 3, cap_top + (wb[3] - wb[1]) + 30),
           "Lift handwriting off any image.", font=tagline, fill=RAG700)

    # JPEG, not PNG: this is a photograph. Measured at q92 the difference over
    # the type is a mean of 1.21/255 while the file is about a quarter the size.
    band.save(OUT, quality=92, subsampling=0, optimize=True)
    print(f"wrote {OUT.relative_to(ROOT)}  {band.size[0]}x{band.size[1]}  pen {ink}")


if __name__ == "__main__":
    main()
