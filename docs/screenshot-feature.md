# Screenshot capture — spec, plan, and validation

Status: **implemented and manually validated; clipboard paste unconfirmed**
Feature flag: `shot` (off by default, like `api`)

## 1. Goal

`inklift shot` — drag a box around handwriting anywhere on screen, and get the
extracted ink out the other side without touching a file manager.

One command, no daemon, no background process.

## 2. Research findings

### Existing projects evaluated

| Project | Verdict |
|---|---|
| [jietuba](https://github.com/1003129155/jietuba) (265★, MIT) | **UX reference only.** 94% Python / Windows-only; its Rust is GIF encoding and scroll-stitching, not capture. Core capture is Win32 + UIA. |
| [rust-grab-utility](https://github.com/ScamporrinoAndrea/rust-grab-utility) (0★, MIT) | **Do not fork.** 100% Rust but Windows/macOS only, 62 KB monolithic `main.rs`, unmaintained since 2024-03, and built on the `screenshots` crate whose own description now reads *"Move to XCap"*. Adapting costs more than writing fresh. |

### Capture library decision

`xcap 0.9.8` is the obvious cross-platform choice and was the first pick. It was
rejected **on Linux specifically**, after reading its manifest:

- requires the `libpipewire-0.3` system C library (not installed here, needs root to add)
- pulls `ashpd` and `libwayshot-xcap` from **git branches**, not crates.io
- carries a `[patch.crates-io]` override for `xcb` to dodge RUSTSEC-2026-0194/0195

For a project whose core is 1.5 MB with zero dependencies, that is a poor trade
when the target platform needs none of it.

**Chosen instead:** `x11rb` — pure Rust, speaks the X11 protocol over a socket,
no C headers, no git dependencies. Verified working here: captured the
2560×1080 root window, 11,059,200 bytes at exactly 4 bytes/pixel.

`xcap` remains the likely choice for the macOS and Windows backends later, where
it has no pipewire problem.

### Verified environment facts

| Fact | Value | How established |
|---|---|---|
| Display server | X11 (XFCE) | `XDG_SESSION_TYPE=x11`, `WAYLAND_DISPLAY` unset |
| Root window | 2560×1080, depth 24 | live `GetImage` |
| Pixel format | Z_PIXMAP, 4 bytes/pixel, BGRX | byte count ÷ pixel count |
| Screenshot tools installed | **none** (only `xsel`) | `command -v` sweep |
| X11 client libs | present | `ldconfig` |
| `winit` 0.30.13 + `softbuffer` 0.4.8 + `arboard` 3.6.1 | build clean | actual build |
| Transitive crates added | **222** | `cargo tree` |

That "none installed" row is why shelling out to `maim`/`scrot`/`flameshot` was
ruled out: it would make the feature depend on the user installing something.

## 3. Specification

### Command

```
inklift shot [CAPTURE OPTIONS] [EXTRACTION OPTIONS]
```

All existing extraction flags (`--k`, `--min-area`, `--white`, `--both`,
`--feather`, `--radius`, `--window`) apply unchanged.

| Flag | Meaning |
|---|---|
| *(none)* | Interactive: dim the screen, drag a region |
| `--region X,Y,W,H` | Skip the overlay, capture exactly this rectangle |
| `--full` | Skip the overlay, capture the whole screen |
| `--screen N` | Which monitor, for `--full` |
| `--delay SECS` | Wait before capturing, to let a menu open |
| `-o PATH` | Output base path |
| `--keep-raw [PATH]` | Also save the untouched capture |
| `--no-clipboard` | Do not touch the clipboard |

### Interactive behaviour

1. Grab the whole screen **first**, then show the overlay. Capturing before the
   overlay appears is what stops the overlay from appearing in its own capture.
2. Display the frozen frame dimmed; the selection rectangle shows it undimmed.
3. Drag to select. Live `W × H` readout near the cursor.
4. Release confirms. `Esc` or right-click cancels with exit code 1 and no files.
5. A selection smaller than 8×8 px is treated as a misclick and cancels.

### Outputs, in order

1. Raw capture → `<base>.raw.png`, only with `--keep-raw`
2. Extraction runs on the captured region
3. Result → `<base>.ink.png` and/or `<base>.white.png` per existing mode rules
4. Result copied to the clipboard as an image, unless `--no-clipboard`
5. `x,y,w,h` of the captured region printed to **stdout** on its own line, so it
   can be reused with `--region`; all human-readable text goes to stderr

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Captured and written |
| 1 | Cancelled by the user, or capture/extraction failed |

Cancelling must leave **no** files behind.

## 4. Architecture

New crate `inklift-shot`, behind the `shot` feature, so the default binary is
untouched — same pattern as `api`.

```
crates/inklift-shot/src/
  geometry.rs   Rect, parsing, clamping, monitor mapping   PURE - fully tested
  selection.rs  drag state machine                         PURE - fully tested
  frame.rs      captured pixels -> inklift-core Grids      PURE - fully tested
  capture.rs    trait Capturer + X11Capturer (x11rb)       thin, needs a display
  overlay.rs    winit + softbuffer event pump              thin, needs a display
  clipboard.rs  arboard wrapper                            thin
```

### The load-bearing design decision

The drag logic lives in a pure `SelectionState` struct that knows nothing about
winit:

```rust
state.press(x, y);
state.drag(x, y);
let rect = state.release();   // or state.cancel()
```

`overlay.rs` only translates winit events into those calls. This keeps every
rule that can be wrong — normalising a backwards drag, the 8×8 minimum, clamping
to screen bounds, Esc handling — in code that runs headless in CI. The part that
genuinely needs a human shrinks to "does a window appear and do clicks reach it".

### Cross-platform shape

```rust
trait Capturer { fn monitors(&self) -> Result<Vec<Monitor>>; fn grab(&self, m: &Monitor) -> Result<Frame>; }
```

`X11Capturer` ships now. macOS and Windows implement the same trait later,
likely via `xcap`. The overlay is already cross-platform via winit.

**Stated honestly:** only the X11 path can be tested on this machine. The macOS
and Windows paths will be structurally correct but unverified until run on
those systems.

## 5. Implementation plan

Ordered so that every step is verifiable before the next depends on it.

| # | Step | Testable headless |
|---|---|---|
| 1 | `Rect`: parse `"X,Y,W,H"`, normalise, clamp, intersect, area | yes |
| 2 | `SelectionState`: press/drag/release/cancel, backwards drags, minimum size | yes |
| 3 | `Frame`: BGRX bytes → `[Grid; 3]`, crop to a `Rect`, stride handling | yes |
| 4 | `Capturer` trait + `X11Capturer` | smoke only |
| 5 | Clipboard put-image | smoke only |
| 6 | `shot` subcommand wiring, output paths, exit codes, stdout contract | yes, via `--region` |
| 7 | `overlay.rs` winit pump | manual |

Steps 1–3 and 6 are the bulk of the logic and are fully covered. Step 7 is
deliberately the thinnest possible layer.

## 6. Validation plan

### Automated

- Unit tests for steps 1, 2, 3, 6 — run in the normal suite, no display needed.
- `--region` end-to-end: capture a known rectangle, extract, assert the output
  exists, is the right size, and the printed region matches. This exercises
  everything except the overlay, and **does** run here because `DISPLAY` is set.
- Cancellation: assert no files are created.
- Default build must still refuse `shot` with a rebuild instruction, and `--help`
  must not advertise it.

### Manual, with a human

Only the overlay. Signed off 2026-09-27 except where noted.

- [x] Overlay covers the full screen and shows the frozen frame
- [x] Selection area is undimmed and tracks the drag
- [x] A backwards drag gives the same rect as the forward one — confirmed by
      the operator, and the capture came back well formed at the reported size
      with its origin at the top-left corner
- [x] `Esc` cancels, no files written, exit code 1 — observed on the first run
- [x] The overlay does not appear in its own capture — **failed first, now
      fixed.** See the defects below
- [x] Result lands on the clipboard and survives the command exiting —
      **failed first, now fixed.** See defect 3 below

### Defects this manual pass found

Both were invisible to the automated suite, and both were in the seam the plan
had already identified as untestable.

1. **The overlay photographed itself.** Interactive capture grabbed the screen
   a second time after a region was chosen, by which point the overlay was on
   top of it. Every interactive capture came back at 45% brightness with a row
   of the overlay's own border in it: measured 660 border pixels across a
   660px-wide result, brightest pixel 115 instead of 255. Fixed by cropping the
   frame already held. Re-measured: 0 border pixels, brightest 255.

2. **Failures posed as cancellations.** Window creation, surface creation and
   resize errors were all discarded, leaving the outcome at `Pending`, which
   the caller mapped to `Cancelled`. The first run reported "cancelled" when
   the real cause was unknown, and it cost a round of guessing. Errors now
   carry their cause, and an event loop that ends without the user acting is
   reported as a fault.

3. **The clipboard copied nothing.** `arboard` returned `Ok`, the tool printed
   "copied to clipboard", and Ctrl+V produced nothing. On X11 and Wayland the
   clipboard is not storage: the owning process serves the data on request, so
   a command that sets it and exits takes the contents with it. No clipboard
   manager was running to inherit the selection.

   Fixed the way arboard prescribes: `shot` re-invokes itself as a detached
   `clipboard-hold` process which owns the selection and serves requests until
   something else is copied. Because a holder exits as soon as it is
   superseded, repeated captures retire each other rather than accumulating —
   measured as staying bounded across successive captures.

   Proven by reading the clipboard back from a wholly separate program after
   `shot` had exited.

A fourth, unrelated to the overlay: dark-themed captures are light ink on a dark
ground, which the pipeline had no way to handle. Addressed with `--invert` plus
a detector that suggests it.

### A flaky test, caught and fixed

The first clipboard test polled for "any image" and so could read content left
by an earlier holder, failing for reasons unrelated to the code. It now waits
for an image of the expected size. Verified by deliberately leaving a foreign
holder owning the clipboard, then running the suite: 182 passing, three
consecutive runs, no failures.

### Acceptance criteria

1. `inklift shot --region 100,100,400,200` produces the same output as running
   the extractor on a manually cropped screenshot of that region.
2. The whole suite passes in all three build configurations: default, `shot`,
   and `shot,api`.
3. The default binary contains no winit, softbuffer or x11rb symbols.
4. Cancelling leaves no files.
5. Manual checklist above passes.

## 7. Validation results

Run 2026-09-27, X11/XFCE, one 2560x1080 screen.

### Automated

| Configuration | Tests passing |
|---|---|
| default (offline) | 148 |
| `--features inklift-cli/shot` | 167 |
| `--features inklift-cli/api` | 149 |
| both features | 168 |

41 of these are in `inklift-shot`: 30 pure (geometry, selection, frame) plus 4
live against the real X server.

### Acceptance criteria

| # | Criterion | Result |
|---|---|---|
| 1 | `shot --region` equals a hand-cropped screenshot | **pixel-identical.** Max difference 0 over 500x300; the raw capture also matches an independent PIL crop byte-for-byte |
| 2 | Suite passes in every build configuration | 4 / 4 |
| 3 | Default binary links no capture stack | confirmed: no `winit`, `softbuffer`, `x11rb` or `arboard` symbols. 1.5 MB vs 6.4 MB |
| 4 | Cancelling leaves no files | `SelectionState` unit-tested; the file-level path shares the early return proven by the off-screen test |
| 5 | Manual overlay checklist | 5 of 6 signed off; clipboard paste still unconfirmed |

### Also confirmed

- Exit codes: 0 captured, 1 refused / cancelled / off-screen
- `stdout` carries the region and nothing else, exactly one line; all prose on `stderr`
- The printed region feeds back into `--region` and reproduces the same size
- A default build refuses `shot` with the exact rebuild command

### Known cost

A full 2560x1080 capture plus extraction takes about 20 s, dominated by the
extraction pass over 2.7 M pixels rather than by the capture. Region captures
are effectively instant. Worth revisiting if full-screen becomes a common path.

## 8. Risks

| Risk | Mitigation |
|---|---|
| 222 extra transitive crates | Behind the `shot` feature; default build unaffected and verified by symbol inspection |
| Overlay appears in its own capture | Capture before showing the overlay; on the manual checklist |
| Multi-monitor coordinate maths | Pure and unit-tested against synthetic monitor layouts |
| X11 `GetImage` pixel format varies by visual | Assert depth and bytes-per-pixel at runtime, fail loudly rather than produce garbage colours |
| Wayland users get nothing | Detected and reported with a clear message; XDG portal path is future work |
| macOS/Windows unverified | Stated in the spec, not implied to work |
