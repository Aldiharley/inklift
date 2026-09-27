# inklift

Lift handwriting off a photograph and onto a transparent or white background.

Phase 0 of the plan in `research-brief.html`: the classical path. No model, no
weights, no network, no fine-tuning. It ships today and stays in the codebase
permanently as the fallback whenever the learned path (Phase 1) is unsure.

```
$ inklift photo.jpg --both
ink 2.9% of page, pen rgb(30, 38, 107)
wrote photo.ink.png
wrote photo.white.png
```

## What it does

```
photo ─► estimate the paper ─► divide the lighting out ─► threshold locally
      ─► drop dust ─► soft opacity + pen colour ─► RGBA / grey-on-white
```

The part that is not in the literature is the last step. Published document
binarizers emit a 1-bit mask; setting `alpha = mask` gives stair-stepped strokes
that composite badly onto anything. Here the binary decision only chooses *which*
pixels may carry ink, while `alpha = 1 − normalized` decides *how much*, which is
what keeps stroke edges smooth. See `research-brief.html` §04.

## Layout

| Crate | Dependencies | Role |
|---|---|---|
| `inklift-core` | **none** | The whole algorithm. Plain `std`, so it drops into a Tauri backend, a WASM bundle or a mobile app unchanged. |
| `inklift-cli` | `image` | PNG/JPEG decoding and the command line. |
| `inklift-api` | `ureq`, `serde_json` | Optional hosted-model path, for measuring the local one against it. Behind the `api` feature, so a default build does not link it. |

Keeping the core dependency-free is deliberate — it is what makes Phase 2 (the
Tauri desktop app) a wiring job rather than a port.

## Usage

```
inklift <IMAGE> [OPTIONS]

-o, --output <PATH>   Where to write the result   [default: <IMAGE>.ink.png]
    --white           Ink on white instead of transparent
    --both            Write both exports
    --k <FLOAT>       Sauvola k; raise it to keep less faint ink  [default: 0.20]
    --window <PX>     Sauvola window radius                       [default: 12]
    --min-area <PX>   Discard connected components below this     [default: 8]
    --radius <PX>     Paper-estimate radius; must exceed the stroke half-width
    --feather <PX>    How far soft edges reach past the stroke    [default: 1]
-q, --quiet           Suppress the summary line
```

Faint pencil needs a lower `--k`. A blurry or upscaled photo needs a lower `--k`
and a larger `--min-area`. If thick strokes come out hollow, raise `--radius`:
it must exceed the half-width of the thickest stroke or that stroke is read as
paper.

## Repainting the ink

The extracted pen colour is the *real* one, which is usually dark — so the
result is invisible on a dark slide. `--ink` repaints it:

```bash
inklift photo.jpg --ink white          # for pasting onto a dark background
inklift photo.jpg --ink "#1E266B"      # a specific pen
inklift shot --ink white
```

Accepts `#RRGGBB`, `#RGB`, `black` or `white`. This is a **colour swap, not a
re-extraction**: opacity and pen colour are stored separately, so nothing is
recomputed and no quality is lost. A test pins that the alpha channel comes back
bit-identical.

It cures the polarity problem, not a faint one. Ink whose opacity peaks around
0.65 — which is what a low-resolution source gives you — stays faint on a busy
background whatever colour it is wearing. Inflating the alpha to compensate
would be lying about the measurement, so it doesn't.

A light `--ink` with `--white` is a contradiction, and the tool says so rather
than writing a file that looks empty.

## Two exports, one decision

`--white` writes **greyscale** ink on white, never a hard binary. The opacity is
still in there, so `alpha_from_gray_on_white` recovers it to within quantisation
error — a test pins this. Shipping the white-background version first therefore
costs nothing and closes no doors. Thresholding to 1-bit is the single
irreversible step available in this pipeline, and nothing here takes it.

## Build and test

```bash
cargo test                              # 104 tests, offline build
cargo test --features inklift-cli/api   # 106 tests
# Neither run touches the network.
cargo build --release
```

Every function was written against a failing test first. The core's tests run
against synthetic pages with known ground truth (`tests/common/mod.rs`), so
assertions are about recovered quantities — IoU against the true ink, opacity
against true stroke coverage — rather than golden images.

Roughly 150 ms for a 900×500 page on one core, single-threaded and unoptimised.

## Screen capture

Behind the `shot` feature, off by default for the same reason as the hosted
path: a stock build links no windowing, X11 or clipboard code at all.

```bash
cargo build --release --features inklift-cli/shot
inklift shot                              # drag a box, Esc cancels
inklift shot --region 200,150,500,300     # skip the overlay
inklift shot --full --screen 1
```

The screen is captured *first* and the overlay drawn over that frozen frame,
so the overlay never appears in its own screenshot. The result is written,
copied to the clipboard, and the captured region printed to stdout as
`X,Y,W,H` so it can be replayed with `--region`. Everything else goes to
stderr, so the command pipes cleanly.

`--keep-raw` also saves the untouched capture, which is what you want while
still hunting for good `--k` and `--min-area` values.

### Dark themes

Screenshots of dark-themed applications are light ink on a dark ground, the
opposite of what the pipeline assumes. Without `--invert` the background is
read as ink and the result is an unreadable smudge:

```bash
inklift shot --invert
```

The image is flipped before anything else runs, so the extracted ink comes out
dark and both exports stay usable — a light ink composited onto white would be
invisible. Colours invert with it, so white becomes black and coloured
foregrounds shift hue.

When a source looks light-on-dark and `--invert` was not given, both commands
say so on stderr rather than silently returning a smudge.

Leave `--min-area` at its default for screen text: raising it to 30, which
suits a photographed page, eats i-dots and punctuation at typical font sizes.

Capture is `x11rb` — pure Rust, no C libraries, no build script. `xcap` was
the obvious choice and was rejected for Linux after reading its manifest: it
needs the `libpipewire-0.3` system library, pulls two dependencies from git
branches, and carries a `patch.crates-io` override for a security advisory.

| Build | Size |
|---|---|
| `cargo build --release` | 1.5 MB |
| `--features inklift-cli/shot` | 6.4 MB |

Full spec, implementation plan and validation results:
[`docs/screenshot-feature.md`](docs/screenshot-feature.md).

## Scoring

`inklift-score` runs the DIBCO measures over a directory of results against a
directory of ground truth. Files pair by name, ignoring case and a trailing
`_GT` / `-gt` suffix, so a DIBCO benchmark folder works unchanged.

```
$ inklift-score results/ groundtruth/
image                              FM     p-FM     PSNR      DRD
----------------------------------------------------------------
page0                           92.01    99.68    21.98   1.5285
page1                           91.02    99.77    21.95   1.6438
...
----------------------------------------------------------------
mean of 6                       89.35    99.42    21.70   1.7460
```

```
inklift-score <RESULTS_DIR> <GROUND_TRUTH_DIR> [OPTIONS]

    --csv <PATH>          Also write per-image scores as CSV
    --threshold <0-255>   Grey level below which a pixel counts as ink  [default: 128]
    --invert              Treat light pixels as ink instead of dark
```

Higher is better for FM, p-FM and PSNR; lower for DRD. An exactly reproduced
image has infinite PSNR and is excluded from that column's mean, with a count
printed underneath.

### How far to trust each number

`f_measure`, `psnr` and `drd` follow the definitions printed in the competition
reports, and the unit tests pin them against hand-computed cases — a single
isolated false positive scores exactly DRD 1.0 per non-uniform block, one wrong
pixel in 256 scores exactly 10·log10(256) dB. These are comparable with
published tables.

**The pseudo-F-measure is not.** There are two of them, and DIBCO says so
explicitly:

- **p-FM** (H-DIBCO 2010, 2012) — pseudo-recall against a skeletonised ground
  truth, combined with ordinary precision. Fully specified. This is what is
  implemented here.
- **Fps** (DIBCO 2011, 2013 onward) — also uses a pseudo-*precision* with
  contour distance weights normalised by local stroke width. The competition
  papers describe it only qualitatively; the construction lives in Ntirogiannis,
  Gatos & Pratikakis, *IEEE TIP* 22(2):595–609, and is not reimplemented here.

A second gap: the competitions used a semi-manually corrected skeleton, while
this derives one automatically by Zhang-Suen thinning. So treat the p-FM column
as comparable across your own runs, and never against a leaderboard.

It is still the most diagnostic column. p-FM stays high while FM falls when
strokes are recovered but too thin or too fat; both fall together when strokes
are actually broken or missed. That distinction tells you which knob to reach
for.

## Phase 0 baseline

On six synthetic pages with lighting gradients, cast shadows, blur and sensor
noise, against a single global threshold on the same pages:

| | FM | p-FM | PSNR | DRD |
|---|---|---|---|---|
| inklift Phase 0 | **89.35** | **99.42** | **21.70** | **1.75** |
| global threshold | 61.37 | 61.37 | 14.94 | 36.97 |

The mean matters less than the spread. inklift ranges 85–92 across the six;
the global threshold ranges 25.8–96.4, collapsing on exactly the pages with
strong shadows. On the mildest page it beats inklift outright (96.4 to 91.0).
That is the whole argument of the research brief in one table: consistency under
unfamiliar degradation is the thing worth optimising, not the average.

p-FM at 99.4 says the stroke medial axes are essentially all recovered, so the
FM shortfall is edge thickness rather than missing ink — a `--feather` and `--k`
question, not a fundamental one.

These are synthetic pages, which is why they are a sanity check on the harness
and not a result. Real captures will score worse.

## Comparing against a hosted model

**The default build cannot reach the network.** The hosted path sits behind a
Cargo feature that is off unless asked for, so a stock `cargo build --release`
links no HTTP client at all:

| Build | Size | Contains |
|---|---|---|
| `cargo build --release` | 1.5 MB | no HTTP client, no TLS, no provider URLs |
| `cargo build --release --features inklift-cli/api` | 4.2 MB | adds `ureq` + `rustls` |

Verified by inspecting the binaries, not just by intent: the default one
contains no provider hostname and no `rustls` symbol anywhere.

In the default build the hosted flags are *refused* with instructions, never
silently ignored, and `--via` is not even listed in `--help`. Build with the
feature, then `--via` opts into sending the page to an image model so the two
can be measured on the same images with the same scorer.

```bash
cp .env.example .env               # then fill in a key
# or: export GEMINI_API_KEY=...
inklift page.jpg --via gemini --white -o api/page.png

./compare.sh pages/ groundtruth/ gemini
```

`compare.sh` runs both paths over a folder and prints two score tables plus
per-image CSVs.

| | `--via gemini` | `--via openai` |
|---|---|---|
| Default model | `gemini-3-pro-image` (Nano Banana Pro) | `gpt-image-2` |
| Endpoint | `generateContent` | `/v1/images/edits` |
| Real alpha channel | no | yes, via `background=transparent` |
| Key variable | `GEMINI_API_KEY` | `OPENAI_API_KEY` |

Keys come from the environment, or from a `.env` beside the project — copy
`.env.example` and fill it in. A real exported variable always wins over the
file, and an empty entry is ignored rather than shadowing one that is set.
`.env` is gitignored.

Override with `--model`, `--prompt`, `--api-key`, `--timeout`. Passing `--model`
or `--prompt` without `--via` is an error rather than a no-op, so you can never
believe you called a model you did not.

The instruction sent with the image is written to fight the documented failure
mode — asked to "clean up" a page, these models re-render the text in their own
hand — by telling the model in as many ways as possible to preserve the exact
strokes. Whether that is enough is the thing being measured. Override it with
`--prompt` and try your own.

Hosted results come back at whatever size the model chooses, so score them with
`--resize`, which resamples to the ground-truth dimensions first. That the flag
is needed at all is itself a finding: you cannot overlay the output on the
original to see what changed.

### What is and is not verified here

Request construction, response parsing, both providers' error shapes, base64
against the RFC vectors, and key resolution are all covered by tests. **The
network call itself is not** — no live request has been made from this code. The
first real call may still surface an auth, quota or schema surprise. Everything
around it is built so that when it does, the error says what happened.

## Known limits

- Strokes under ~2 px cannot be recovered; the information is not there. The
  low-resolution sample shows this as a washed-out pen colour, which is honest
  behaviour rather than a bug.
- Assumes roughly neutral paper. Channels are normalized by a shared luminance
  background, which costs one closing instead of three; strongly tinted paper
  would need per-channel estimation.
- Pencil on textured paper and highlighter are the known hard cases, as flagged
  in the brief.
- Pixels are processed in the space the file stores them in, with no gamma
  conversion. Division cancels a multiplicative light field either way, and
  viewers composite alpha on sRGB values, so opacity derived here is correct
  where it is used. `tests/io.rs` pins this decision.

## Next

Phase 0 is only half done until there is a **golden set**: 200+ of your own real
captures, scored with the DIBCO metrics (FM, p-FM, PSNR, DRD). That set is what
tells you whether Phase 1's learned model is actually an improvement. Without it
you are tuning blind.
