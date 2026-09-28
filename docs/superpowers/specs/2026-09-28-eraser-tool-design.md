# Eraser tool — design

Date: 2026-09-28. Status: approved in brainstorming, not yet built.

## Problem

A screenshot or photo often carries marks the pipeline rightly keeps as ink but
the user does not want: a cursor, a UI line, a neighbouring word, a smudge.
Today the only recourse is the sliders, which act on the whole image. The user
needs to remove those marks by hand, with a brush whose size and softness they
control, and have the removal hold in everything the app outputs.

## Decisions

- **Scope:** an eraser with adjustable size and softness, per-stroke undo
  (Ctrl+Z) and a Clear button. No restore brush, no zoom — both can follow.
- **Architecture:** the stroke list lives in Rust, beside the source image, and
  is applied to every result. The page draws strokes live for feedback only.
  Rejected: a page-painted mask uploaded with every call (large IPC, loses
  precision when painted on a downscaled view, undo only in JS); erasing only
  on screen (Save and Copy re-extract in Rust, so the marks would return in
  exactly the outputs that matter).
- **Product boundary:** `design/ux-architecture.md` says inklift is "not an
  image editor". The eraser is a bounded exception — removing unwanted ink from
  the result. It never adds ink, and there is no painting, layering or
  retouching. The doc is updated to say so.

## 1. Brush model — `inklift-core/src/erase.rs`

No new dependencies (core has none, by policy).

```rust
pub struct Stroke {
    pub points: Vec<[f32; 2]>, // source-image pixels, pixel centres at +0.5
    pub radius: f32,           // source-image pixels
    pub softness: f32,         // 0.0 ..= 1.0
}

/// Per-pixel factor in [0, 1] that multiplies alpha: 1 keeps, 0 erases.
pub fn keep_mask(width: usize, height: usize, strokes: &[Stroke], scale: f32) -> Grid;
```

- **Distance** of a pixel centre to a stroke is its distance to the nearest
  segment of the polyline (a single point is a zero-length segment), so a fast
  drag that yields sparse points leaves no gaps.
- **Profile:** with `inner = radius * (1 - softness)`, coverage is 1 for
  `d <= inner` and falls to 0 at `d = radius` along a smoothstep. At softness 0
  the edge is still antialiased over one pixel: `inner = max(0, radius - 1)`
  in that case, so a hard brush does not alias.
- **Within one stroke** coverage is the maximum over its segments — a stroke
  crossing itself does not erase twice.
- **Across strokes** the factors multiply: `keep *= 1 - coverage`. Two soft
  passes erase more than one, as a physical eraser does.
- **Scale:** points and radius are multiplied by `scale` before rasterising, so
  the proxy preview (scale < 1) uses the same strokes. The mask is sized to the
  grid it is applied to.
- **Cost:** each segment rasterises only within its bounding box grown by the
  radius.

```rust
impl Extraction {
    /// Multiply alpha by `keep` (same shape). Ink colour is left as extracted:
    /// erasing a stray mark must not shift the colour of the rest.
    pub fn erased(self, keep: &Grid) -> Extraction;
}
```

`coverage()` reflects the erased alpha. `to_rgba8` already zeroes fully
transparent pixels, so erased areas export clean.

## 2. App backend — `inklift-gui/src/main.rs`

- `Source` gains `strokes: Vec<Stroke>`. `adopt()` builds a fresh `Source`, so
  loading a file or a capture clears the erasing with no extra code path.
- New commands, registered in `generate_handler!`:
  - `erase_stroke(stroke: StrokeIn) -> Result<usize, String>` — appends,
    returns the count. Rejects: no source loaded; empty points; any non-finite
    coordinate; radius outside 1..=400; softness outside 0..=1.
  - `undo_erase() -> Result<usize, String>` — pops one, returns the count left.
  - `clear_erase() -> Result<(), String>`.
- `render` applies `keep_mask(.., scale)` to the proxy or full extraction.
- `finished()` applies it at scale 1. Save, Copy and the tray's "Copy again"
  all go through `finished()`, so none can skip it.
- `Params` is unchanged; the sliders' contract is untouched.

## 3. Interface — `inklift-gui/ui/index.html`, `app.js`

- **Sidebar section "Clean up"** after the existing controls:
  - **Eraser** toggle (`aria-pressed`), key **E**.
  - **Size** slider, 2–200 image px, default 20, readout "20 px".
  - **Softness** slider, 0–100 %, default 25, readout "25 %".
  - **Undo** (Ctrl+Z) and **Clear** buttons.
  - All disabled until an image is loaded; Undo and Clear disabled while the
    stroke count is 0. No dead controls.
- **Eraser mode:**
  - The stage cursor is a circle outline at the brush's on-screen size
    (image px × display scale), following the pointer.
  - Pointer drag erases; pointer capture keeps the stroke going if the pointer
    leaves the image.
  - Pointer hold-to-compare is off while erasing (the pointer is busy); Space
    still compares. Esc or E leaves eraser mode. Enter still copies.
- **Display:** the ink result moves from `<img id="ink">` to a `<canvas>` that
  draws the returned PNG, keeping the same sizing and drop-shadow filters.
  While dragging, the page cuts the stroke out of the canvas with
  `destination-out` using the same profile as Rust. On pointer-up it sends the
  stroke (in source pixels) and runs the normal preview → full refresh, so the
  canvas always converges on what Rust will export.
- **Coordinates:** pointer → canvas box → source pixels using the source size
  from `Loaded`, not the proxy size in `Rendered`.
- **Undo naming:** the design doc reserves Ctrl+Z for a settings history that
  was never built. Here Ctrl+Z undoes erase strokes; if settings undo is built
  later the two should share one history.
- **Docs:** `design/ux-architecture.md` non-goal line gains the bounded
  exception; the README's desktop-app section describes the eraser.

## 4. Errors

- Every failed invoke reaches the user as a toast; no catch only logs (the
  existing `frontend_contract` check enforces this).
- A rejected stroke toasts and triggers a re-render, so the canvas snaps back
  to Rust's truth rather than showing an erasure that will not export.

## 5. Testing

Test first, per `docs/porting-capture.md` §10: each test is seen failing for
the right reason before the code it guards is written.

- **`inklift-core/tests/erase.rs`:**
  - A hard brush erases fully inside the radius and leaves pixels beyond it
    untouched.
  - Softness gives a monotonic fall-off that reaches 0 at the radius.
  - A two-point fast drag leaves no gap along the segment.
  - Two soft passes erase more than one.
  - The mask at scale 0.5 matches the full-scale mask sampled at the same
    points within tolerance.
  - `erased()` multiplies alpha, keeps ink colour, lowers coverage.
  - An empty stroke list yields an all-ones mask.
- **`inklift-gui/tests` contract checks:** the three commands are registered
  and invoked; `render` and `finished` both apply the stroke mask; `adopt`
  resets strokes; each eraser control has a handler.
- **Validation before done:** `cargo test --workspace` and
  `cargo test --workspace --features inklift-cli/shot`; clippy adds no
  warnings; run the real app, erase a stray mark on a sample, Save, and check
  the saved PNG's pixels — the mark gone, adjacent ink intact; Undo, Clear and
  loading a new image behave as specified.

## Out of scope

Restore brush, zoom and pan, pressure sensitivity, erasing in the CLI, saving
strokes across sessions.
