#!/usr/bin/env python3
"""Regenerate the sample images and the synthetic benchmark.

    python3 tools/make_fixtures.py            # samples/ and bench/
    python3 tools/make_fixtures.py bench      # bench/ only

Requires numpy and pillow. These are development fixtures, not part of the
build; the Rust crates have no Python dependency.

The benchmark writes DIBCO-style ground truth (black ink on white, named
<page>_GT.png) so `inklift-score` can be run against it:

    for f in bench/page*.png; do
        ./target/release/inklift "$f" --white -o "bench/results/$(basename $f)" -q
    done
    ./target/release/inklift-score bench/results bench/gt
"""
import sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

PAPER = np.array([236, 231, 221], np.float32)
PEN = np.array([26, 33, 96], np.float32)


def squiggles(w, h, rng, rows=4, margin=30, spacing=None):
    """Handwriting-like strokes, returned as an 8-bit coverage mask."""
    img = Image.new("L", (w, h), 0)
    draw = ImageDraw.Draw(img)
    spacing = spacing or (h - 2 * margin) // rows
    for r in range(rows):
        y0 = margin + 30 + r * spacing
        step = (w - 2 * margin) // 20
        pts = [(margin + i * step, y0 + int(rng.integers(-20, 21))) for i in range(21)]
        draw.line(pts, fill=255, width=int(rng.integers(3, 6)), joint="curve")
    return img


def degrade(cov, rng, w, h, blur=0.7, noise=6.0):
    """Composite ink over lit paper, then add the things a phone camera adds."""
    a = (np.asarray(cov, np.float32) / 255.0)[:, :, None]
    img = a * PEN + (1 - a) * PAPER
    yy, xx = np.mgrid[0:h, 0:w]
    shade = 1.0 - float(rng.uniform(0.25, 0.45)) * (xx / w) \
                - float(rng.uniform(0.10, 0.30)) * (yy / h)
    cx, cy = rng.integers(0, w), rng.integers(0, h)
    blob = 1.0 - float(rng.uniform(0.15, 0.35)) * np.exp(
        -(((xx - cx) ** 2 + (yy - cy) ** 2) / (2 * 170.0 ** 2))
    )
    img *= (shade * blob)[:, :, None]
    img = np.asarray(
        Image.fromarray(np.clip(img, 0, 255).astype(np.uint8))
        .filter(ImageFilter.GaussianBlur(blur)),
        np.float32,
    )
    return np.clip(img + rng.normal(0, noise, img.shape), 0, 255).astype(np.uint8)


def make_samples(out="samples"):
    import os
    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(11)
    w, h = 900, 500
    cov = squiggles(w, h, rng, rows=4, margin=40, spacing=100)
    page = degrade(cov, rng, w, h, blur=0.6, noise=7.0)
    Image.fromarray(page).save(f"{out}/messy.png")
    # A third-scale round trip, to show what happens below ~2px strokes.
    Image.fromarray(page).resize((w // 3, h // 3)).resize((w, h)).save(f"{out}/messy_lowres.png")
    print(f"wrote {out}/messy.png and {out}/messy_lowres.png")


def make_bench(out="bench", pages=6):
    import os
    os.makedirs(f"{out}/gt", exist_ok=True)
    os.makedirs(f"{out}/results", exist_ok=True)
    w, h = 640, 400
    for n in range(pages):
        rng = np.random.default_rng(100 + n)
        cov = squiggles(w, h, rng, rows=4, margin=30, spacing=85)
        gt = np.asarray(cov) > 127
        Image.fromarray(np.where(gt, 0, 255).astype(np.uint8)).save(f"{out}/gt/page{n}_GT.png")
        Image.fromarray(degrade(cov, rng, w, h)).save(f"{out}/page{n}.png")
    print(f"wrote {pages} pages and ground truth to {out}/")


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    if which in ("all", "samples"):
        make_samples()
    if which in ("all", "bench"):
        make_bench()
