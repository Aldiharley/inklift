# Eraser Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the desktop app's user erase leftover marks from an extraction result with a brush of adjustable size and softness, with the erasing honoured by every preview, Save, Copy and tray "Copy again".

**Architecture:** Brush strokes (points in source-image pixels, radius, softness) are validated and rasterised in `inklift-core` (`erase.rs`) into a per-pixel keep factor that multiplies the extraction's alpha. The GUI backend keeps the stroke list beside the source image and applies it in `render` (at proxy scale) and `finished` (at full scale, used by Save/Copy). The page shows the result on a `<canvas>`, cuts strokes out live while dragging, sends each finished stroke to Rust, then re-renders so the view converges on what exports.

**Tech Stack:** Rust (std-only core), Tauri 2, vanilla JS + inline CSS (no bundler, no npm).

**Spec:** `docs/superpowers/specs/2026-09-28-eraser-tool-design.md`

## Global Constraints

- Work only in the worktree `M:\Projects\inklift-eraser` on branch `eraser-tool`. Never touch `M:\Projects\inklift-clone` — another session is committing there.
- `inklift-core` has zero dependencies. Add none.
- Test first: write the test, run it, see it fail for the right reason, then implement (`docs/porting-capture.md` §10).
- Comments explain why, not what; match the surrounding register.
- Do not reformat: never run `cargo fmt`. Keep diffs to your work.
- Clippy is advisory, but add no new warnings.
- No silent failures: every failed `invoke` reaches the user via `toast(…, true)`; no `catch` only logs.
- No dead controls: a control is disabled whenever it cannot act.
- No `innerHTML` in `app.js`.
- Brush radius accepted by the backend: 1..=400 source px. Softness: 0..=1. UI Size slider (a diameter): 2–200 px, default 20. UI Softness slider: 0–100 %, default 25.
- Commit messages: imperative plain-English subject, no prefix, no trailing period; prose body; end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Files are checked out with CRLF; git normalises on commit. Do not convert line endings.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/inklift-core/src/erase.rs` | Create | `Stroke` (validated), `keep_mask` rasteriser |
| `crates/inklift-core/src/extract.rs` | Modify | `Extraction::erased` |
| `crates/inklift-core/src/lib.rs` | Modify | `mod erase;` and re-exports |
| `crates/inklift-core/tests/erase.rs` | Create | Brush behaviour tests |
| `crates/inklift-gui/src/main.rs` | Modify | Stroke state, three commands, `apply_erasing` in `render`/`finished` |
| `crates/inklift-gui/tests/eraser_contract.rs` | Create | Source checks that every output erases and every control is wired |
| `crates/inklift-gui/ui/index.html` | Modify | Canvas result, brush ring, "Clean up" controls, CSS |
| `crates/inklift-gui/ui/app.js` | Modify | Canvas painting, eraser mode, live strokes, undo/clear, keys |
| `design/ux-architecture.md` | Modify | Bounded exception to "not an image editor" |
| `README.md` | Modify | Desktop-app section mentions the eraser |

---

### Task 1: Brush model in inklift-core

**Files:**
- Create: `crates/inklift-core/src/erase.rs`
- Modify: `crates/inklift-core/src/extract.rs` (add a method to `impl Extraction`, after `with_ink_color`)
- Modify: `crates/inklift-core/src/lib.rs`
- Test: `crates/inklift-core/tests/erase.rs`

**Interfaces:**
- Consumes: `inklift_core::Grid` (`Grid::filled(w, h, v)`, `get(x, y)`, `set(x, y, v)`, `data()`, `data_mut()`, `same_shape(&Grid)`, `width()`, `height()`), `Extraction` (private fields `alpha: Grid`, `ink_color: [f32; 3]`).
- Produces:
  - `pub struct Stroke` (Clone, Debug, PartialEq; private fields) with `pub fn new(points: Vec<[f32; 2]>, radius: f32, softness: f32) -> Result<Stroke, String>`
  - `pub fn keep_mask(width: usize, height: usize, strokes: &[Stroke], scale: f32) -> Grid`
  - `impl Extraction { pub fn erased(self, keep: &Grid) -> Extraction }` — panics with a message containing `"erase mask"` when shapes differ.
  - Re-exported from the crate root: `inklift_core::{Stroke, keep_mask}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/inklift-core/tests/erase.rs`:

```rust
//! Hand erasing: the brush a user paints over marks the pipeline rightly kept
//! as ink but they do not want. What matters is where it erases, how its edge
//! falls off, and that the proxy preview and the full export agree — the app
//! previews on a downscaled proxy and exports at full size from the same list.

mod common;

use common::{PageSpec, make_page};
use inklift_core::{Grid, Options, Stroke, extract, keep_mask};

fn stroke(points: &[[f32; 2]], radius: f32, softness: f32) -> Stroke {
    Stroke::new(points.to_vec(), radius, softness).expect("a valid stroke")
}

/// Distance from pixel (x, y)'s centre to a point.
fn dist(x: usize, y: usize, p: [f32; 2]) -> f32 {
    let (dx, dy) = (x as f32 + 0.5 - p[0], y as f32 + 0.5 - p[1]);
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn a_hard_brush_erases_inside_its_radius_and_nothing_beyond() {
    let c = [50.5, 50.5];
    let keep = keep_mask(100, 100, &[stroke(&[c], 10.0, 0.0)], 1.0);
    for y in 0..100 {
        for x in 0..100 {
            let d = dist(x, y, c);
            let k = keep.get(x, y);
            if d <= 9.0 {
                assert_eq!(k, 0.0, "({x},{y}) at {d:.2} px should be erased");
            }
            if d >= 10.0 {
                assert_eq!(k, 1.0, "({x},{y}) at {d:.2} px should be untouched");
            }
        }
    }
}

#[test]
fn softness_fades_the_edge_smoothly_out_to_the_radius() {
    let c = [50.5, 50.5];
    let keep = keep_mask(100, 100, &[stroke(&[c], 20.0, 1.0)], 1.0);
    let row: Vec<f32> = (50..80).map(|x| keep.get(x, 50)).collect();
    assert_eq!(row[0], 0.0, "the centre of the brush erases fully");
    for w in row.windows(2) {
        assert!(w[1] >= w[0], "keep must not fall moving outward: {row:?}");
    }
    let mid = keep.get(60, 50);
    assert!(mid > 0.05 && mid < 0.95, "halfway out a fully soft brush erases partly, got {mid}");
    assert_eq!(keep.get(70, 50), 1.0, "at the radius the brush has faded out");
}

#[test]
fn a_fast_drag_leaves_no_gap_between_its_points() {
    // Two points 80 px apart, as a quick flick of the mouse reports them.
    let keep = keep_mask(100, 100, &[stroke(&[[10.5, 50.5], [90.5, 50.5]], 3.0, 0.0)], 1.0);
    for x in 10..=90 {
        assert_eq!(keep.get(x, 50), 0.0, "gap at x = {x}");
    }
}

#[test]
fn a_second_soft_pass_erases_more_than_the_first() {
    let s = stroke(&[[50.5, 50.5]], 20.0, 1.0);
    let once = keep_mask(100, 100, &[s.clone()], 1.0).get(62, 50);
    let twice = keep_mask(100, 100, &[s.clone(), s], 1.0).get(62, 50);
    assert!(once > 0.0 && once < 1.0, "the test point must sit in the soft edge, got {once}");
    assert!((twice - once * once).abs() < 1e-6, "passes compound: {once} then {twice}");
}

#[test]
fn a_stroke_crossing_itself_does_not_erase_twice() {
    let there = keep_mask(100, 100, &[stroke(&[[40.5, 50.5], [60.5, 50.5]], 12.0, 1.0)], 1.0);
    let back = keep_mask(
        100,
        100,
        &[stroke(&[[40.5, 50.5], [60.5, 50.5], [40.5, 50.5]], 12.0, 1.0)],
        1.0,
    );
    assert_eq!(there.data(), back.data());
}

#[test]
fn the_proxy_mask_agrees_with_the_full_one() {
    let s = stroke(&[[40.0, 100.0], [160.0, 110.0]], 20.0, 0.5);
    let full = keep_mask(200, 200, &[s.clone()], 1.0);
    let half = keep_mask(100, 100, &[s], 0.5);
    let mut worst = 0.0f32;
    for y in 0..100 {
        for x in 0..100 {
            let avg = (full.get(2 * x, 2 * y)
                + full.get(2 * x + 1, 2 * y)
                + full.get(2 * x, 2 * y + 1)
                + full.get(2 * x + 1, 2 * y + 1))
                / 4.0;
            worst = worst.max((half.get(x, y) - avg).abs());
        }
    }
    assert!(worst < 0.1, "proxy and full masks differ by up to {worst}");
}

#[test]
fn no_strokes_keep_everything() {
    let keep = keep_mask(40, 30, &[], 1.0);
    assert!(keep.data().iter().all(|&k| k == 1.0));
}

#[test]
fn strokes_off_the_image_are_ignored() {
    let keep = keep_mask(40, 30, &[stroke(&[[-100.0, -100.0], [-50.0, -60.0]], 10.0, 0.0)], 1.0);
    assert!(keep.data().iter().all(|&k| k == 1.0));
}

#[test]
fn a_stroke_is_refused_when_it_cannot_mean_anything() {
    assert!(Stroke::new(vec![], 10.0, 0.5).is_err(), "no points");
    assert!(Stroke::new(vec![[f32::NAN, 1.0]], 10.0, 0.5).is_err(), "NaN point");
    assert!(Stroke::new(vec![[1.0, f32::INFINITY]], 10.0, 0.5).is_err(), "infinite point");
    for r in [0.5, 400.5, f32::NAN] {
        assert!(Stroke::new(vec![[1.0, 1.0]], r, 0.5).is_err(), "radius {r}");
    }
    for s in [-0.1, 1.1, f32::NAN] {
        assert!(Stroke::new(vec![[1.0, 1.0]], 10.0, s).is_err(), "softness {s}");
    }
    for (r, s) in [(1.0, 0.0), (400.0, 1.0)] {
        assert!(Stroke::new(vec![[1.0, 1.0]], r, s).is_ok(), "radius {r} softness {s}");
    }
}

#[test]
fn erasing_removes_ink_and_leaves_the_rest_as_it_was() {
    let page = make_page(&PageSpec::default());
    let before = extract(&page.rgb, &Options::default());
    let (w, h) = (before.width(), before.height());
    let mut keep = Grid::filled(w, h, 1.0);
    for y in 0..h {
        for x in 0..w / 2 {
            keep.set(x, y, 0.0);
        }
    }
    let left_ink: f32 = (0..h)
        .flat_map(|y| (0..w / 2).map(move |x| (x, y)))
        .map(|(x, y)| before.alpha().get(x, y))
        .sum();
    assert!(left_ink > 10.0, "the fixture must have ink on the left to erase");

    let after = before.clone().erased(&keep);
    for y in 0..h {
        for x in 0..w {
            let want = if x < w / 2 { 0.0 } else { before.alpha().get(x, y) };
            assert_eq!(after.alpha().get(x, y), want, "({x},{y})");
        }
    }
    assert_eq!(after.ink_color(), before.ink_color(), "erasing must not shift the pen colour");
    assert!(after.coverage() < before.coverage());
}

#[test]
#[should_panic(expected = "erase mask")]
fn a_mask_of_the_wrong_size_is_a_bug_not_a_silent_no_op() {
    let page = make_page(&PageSpec::default());
    let result = extract(&page.rgb, &Options::default());
    let _ = result.erased(&Grid::filled(3, 3, 1.0));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `M:\Projects\inklift-eraser`): `cargo test -p inklift-core --test erase`
Expected: compile FAIL — `unresolved imports inklift_core::Stroke, inklift_core::keep_mask` and `no method named erased`.

- [ ] **Step 3: Write the brush model**

Create `crates/inklift-core/src/erase.rs`:

```rust
//! Hand erasing: brush strokes the user paints over marks the pipeline rightly
//! kept as ink but they do not want — a cursor, a UI rule, a neighbour's word.
//!
//! Strokes are stored in source-image pixels and rasterised on demand into a
//! keep factor per pixel. One list then serves both the downscaled proxy the
//! app previews on and the full-resolution export, so the two cannot drift.

use crate::grid::Grid;

/// One press-drag-release of the eraser.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    points: Vec<[f32; 2]>,
    radius: f32,
    softness: f32,
}

impl Stroke {
    /// Points are in source-image pixels, where pixel `(x, y)` covers
    /// `x..x+1, y..y+1`; `radius` is in the same units; `softness` runs from a
    /// crisp edge at 0 to a fall-off starting at the centre line at 1.
    pub fn new(points: Vec<[f32; 2]>, radius: f32, softness: f32) -> Result<Stroke, String> {
        if points.is_empty() {
            return Err("an eraser stroke needs at least one point".into());
        }
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return Err("an eraser stroke has a point that is not a number".into());
        }
        if !(1.0..=400.0).contains(&radius) {
            return Err(format!("eraser radius {radius} is outside 1 to 400 pixels"));
        }
        if !(0.0..=1.0).contains(&softness) {
            return Err(format!("eraser softness {softness} is outside 0 to 1"));
        }
        Ok(Stroke { points, radius, softness })
    }
}

/// Erase strength at distance `d` from a stroke's centre line.
fn coverage(d: f32, radius: f32, softness: f32) -> f32 {
    // Even a "hard" brush keeps one pixel of fall-off: a binary edge aliases,
    // and its stair-steps show against the smooth edges the pipeline produces.
    let inner = (radius * (1.0 - softness)).min(radius - 1.0).max(0.0);
    if d <= inner {
        1.0
    } else if d >= radius {
        0.0
    } else {
        let t = (radius - d) / (radius - inner);
        t * t * (3.0 - 2.0 * t)
    }
}

fn distance_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    (cx * cx + cy * cy).sqrt()
}

/// Per-pixel factor in [0, 1] to multiply alpha by: 1 keeps, 0 erases.
///
/// `scale` maps the strokes' source pixels onto this grid — 1 for the full
/// image, the proxy's factor for a preview.
pub fn keep_mask(width: usize, height: usize, strokes: &[Stroke], scale: f32) -> Grid {
    let mut keep = Grid::filled(width, height, 1.0);
    for stroke in strokes {
        let pts: Vec<[f32; 2]> = stroke.points.iter().map(|p| [p[0] * scale, p[1] * scale]).collect();
        let radius = stroke.radius * scale;

        // Only the stroke's own neighbourhood is visited, so a long session on
        // a large capture stays proportional to what was painted.
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &pts {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        // Float-to-usize casts saturate, so a stroke wholly off the image
        // yields an empty box rather than a wrap-around.
        let bx0 = (x0 - radius).floor().max(0.0) as usize;
        let by0 = (y0 - radius).floor().max(0.0) as usize;
        let bx1 = (x1 + radius).ceil().min(width as f32) as usize;
        let by1 = (y1 + radius).ceil().min(height as f32) as usize;
        if bx0 >= bx1 || by0 >= by1 {
            continue;
        }
        let bw = bx1 - bx0;

        // Within one stroke take the strongest segment, so a stroke that
        // crosses itself does not erase twice where it overlaps.
        let mut cov = vec![0.0f32; bw * (by1 - by0)];
        let segments: Vec<([f32; 2], [f32; 2])> = if pts.len() == 1 {
            vec![(pts[0], pts[0])]
        } else {
            pts.windows(2).map(|w| (w[0], w[1])).collect()
        };
        for (a, b) in segments {
            let sx0 = (a[0].min(b[0]) - radius).floor().max(bx0 as f32) as usize;
            let sy0 = (a[1].min(b[1]) - radius).floor().max(by0 as f32) as usize;
            let sx1 = (a[0].max(b[0]) + radius).ceil().min(bx1 as f32) as usize;
            let sy1 = (a[1].max(b[1]) + radius).ceil().min(by1 as f32) as usize;
            for y in sy0..sy1 {
                for x in sx0..sx1 {
                    let d = distance_to_segment([x as f32 + 0.5, y as f32 + 0.5], a, b);
                    let c = coverage(d, radius, stroke.softness);
                    let i = (y - by0) * bw + (x - bx0);
                    if c > cov[i] {
                        cov[i] = c;
                    }
                }
            }
        }

        // Across strokes the factors multiply: a second soft pass erases more,
        // as a physical eraser does.
        for y in by0..by1 {
            for x in bx0..bx1 {
                let c = cov[(y - by0) * bw + (x - bx0)];
                if c > 0.0 {
                    let k = keep.get(x, y);
                    keep.set(x, y, k * (1.0 - c));
                }
            }
        }
    }
    keep
}
```

In `crates/inklift-core/src/extract.rs`, inside `impl Extraction`, directly after the `with_ink_color` method, add:

```rust
    /// Remove ink by hand: multiply opacity by `keep` (1 keeps, 0 erases).
    ///
    /// The pen colour is left as extracted. It was estimated from all the ink,
    /// and erasing a stray mark must not shift the colour of what remains.
    pub fn erased(mut self, keep: &Grid) -> Self {
        assert!(
            self.alpha.same_shape(keep),
            "erase mask is {}x{} but the result is {}x{}",
            keep.width(),
            keep.height(),
            self.alpha.width(),
            self.alpha.height()
        );
        for (a, k) in self.alpha.data_mut().iter_mut().zip(keep.data()) {
            *a *= k.clamp(0.0, 1.0);
        }
        self
    }
```

In `crates/inklift-core/src/lib.rs`, add `mod erase;` after `mod color;`, and add `pub use erase::{Stroke, keep_mask};` after the `pub use color::…` line.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p inklift-core --test erase`
Expected: 11 passed, 0 failed.

Then: `cargo test -p inklift-core` — Expected: all pass. And `cargo clippy -p inklift-core --tests 2>&1 | grep -A5 "erase"` — Expected: no warnings pointing at `erase.rs`, `tests/erase.rs` or the new method.

- [ ] **Step 5: Commit**

```bash
git add crates/inklift-core/src/erase.rs crates/inklift-core/src/extract.rs crates/inklift-core/src/lib.rs crates/inklift-core/tests/erase.rs
git commit -F - <<'EOF'
Add brush strokes that erase ink from an extraction

A Stroke is a validated brush path in source-image pixels with a radius and a
softness; keep_mask rasterises a list of them into a keep factor per pixel and
Extraction::erased multiplies the alpha by it. Distance is to each segment, so
a fast drag leaves no gaps; a stroke crossing itself erases once, while
separate passes compound. The mask takes a scale so the proxy preview and the
full export apply the same strokes. The pen colour is left as extracted.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 2: Stroke state and commands in the GUI backend

**Files:**
- Modify: `crates/inklift-gui/src/main.rs` (imports line 14; `struct Source` ~line 24; `adopt` ~line 126; `render` ~line 301; `finished` ~line 346; `generate_handler!` ~line 632)
- Test: `crates/inklift-gui/tests/eraser_contract.rs` (create)

**Interfaces:**
- Consumes (Task 1): `inklift_core::{Stroke, keep_mask, Extraction}`; `Stroke::new(Vec<[f32;2]>, f32, f32) -> Result<Stroke, String>`; `keep_mask(usize, usize, &[Stroke], f32) -> Grid`; `Extraction::erased(self, &Grid) -> Extraction`; `Extraction::width()/height() -> usize`.
- Produces (for Task 3):
  - Tauri command `erase_stroke` — JS: `invoke("erase_stroke", { stroke: { points: [[x, y], …], radius, softness } })` → resolves to the stroke count (number).
  - Tauri command `undo_erase` — JS: `invoke("undo_erase")` → resolves to the count left.
  - Tauri command `clear_erase` — JS: `invoke("clear_erase")` → resolves to null.
  - All three reject with `"nothing loaded yet"` when no image is loaded.
  - `render`, `save`, `copy` output already erased; `Params` unchanged.

- [ ] **Step 1: Write the failing contract tests**

Create `crates/inklift-gui/tests/eraser_contract.rs`:

```rust
//! The eraser only works if every output applies it. Save, Copy and the tray's
//! "Copy again" re-extract in Rust rather than exporting what the window shows,
//! so an output path that skipped the strokes would quietly bring the erased
//! marks back in exactly the file the user keeps. These are source checks: the
//! GUI is a binary with no library for a test to call into.

use std::fs;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The body of `fn name(` in main.rs, up to its closing brace at column 0.
fn body_of(src: &str, name: &str) -> String {
    src.split(&format!("fn {name}("))
        .nth(1)
        .and_then(|s| s.split("\n}").next())
        .unwrap_or_else(|| panic!("main.rs must define fn {name}"))
        .to_string()
}

#[test]
fn every_output_applies_the_erasing() {
    let main_rs = read("src/main.rs");
    assert!(
        body_of(&main_rs, "apply_erasing").contains("keep_mask("),
        "apply_erasing must rasterise the strokes"
    );
    for f in ["render", "finished"] {
        assert!(
            body_of(&main_rs, f).contains("apply_erasing("),
            "fn {f} does not apply the erasing, so its output would bring erased marks back"
        );
    }
    for f in ["save", "copy"] {
        assert!(
            body_of(&main_rs, f).contains("finished("),
            "fn {f} must export through finished(), which applies the erasing"
        );
    }
}

#[test]
fn loading_a_new_image_starts_with_no_erasing() {
    let body = body_of(&read("src/main.rs"), "adopt");
    assert!(
        body.contains("strokes: Vec::new()"),
        "adopt must reset the strokes: they are in the old image's pixels"
    );
}

#[test]
fn the_eraser_commands_are_registered() {
    let main_rs = read("src/main.rs");
    let handler = main_rs
        .split("generate_handler![")
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("main.rs must register a command handler");
    for cmd in ["erase_stroke", "undo_erase", "clear_erase"] {
        assert!(handler.contains(cmd), "{cmd} is not registered");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p inklift-gui --test eraser_contract`
Expected: 3 FAIL — "main.rs must define fn apply_erasing", "adopt must reset the strokes", "erase_stroke is not registered".

- [ ] **Step 3: Implement the backend**

In `crates/inklift-gui/src/main.rs`:

a) Replace the core import line
`use inklift_core::{Grid, Options, extract, looks_inverted, parse_ink_color};`
with
`use inklift_core::{Extraction, Grid, Options, Stroke, extract, keep_mask, looks_inverted, parse_ink_color};`

b) Replace
```rust
/// The source image, held for the life of a result so retuning is free.
struct Source {
    image: image::RgbaImage,
}
```
with
```rust
/// The source image, held for the life of a result so retuning is free.
struct Source {
    image: image::RgbaImage,
    /// The user's erasing, in this image's pixels. It belongs to this image
    /// and is dropped with it: the same points mean nothing on another one.
    strokes: Vec<Stroke>,
}

/// One eraser stroke as the page reports it: points in source-image pixels.
#[derive(Debug, Deserialize)]
struct StrokeIn {
    points: Vec<[f32; 2]>,
    radius: f32,
    softness: f32,
}
```

c) In `adopt`, replace `*state.source.lock().unwrap() = Some(Source { image });` with
`*state.source.lock().unwrap() = Some(Source { image, strokes: Vec::new() });`

d) Directly above `/// Extract and hand back a picture. `full` skips the proxy.` add:
```rust
/// Apply the user's erasing. Every output goes through here — the preview, and
/// through `finished` the saved file and the clipboard — or an erased mark would
/// come back in whichever one skipped it. `scale` maps the strokes' source
/// pixels onto a proxy.
fn apply_erasing(result: Extraction, strokes: &[Stroke], scale: f32) -> Extraction {
    if strokes.is_empty() {
        return result;
    }
    let keep = keep_mask(result.width(), result.height(), strokes, scale);
    result.erased(&keep)
}
```

e) In `render`, after the block
```rust
    if let Some(text) = &params.ink {
        result = result.with_ink_color(parse_ink_color(text)?);
    }
```
add
```rust
    result = apply_erasing(result, &src.strokes, scale);
```

f) In `finished`, after its identical `if let Some(text) = &params.ink { … }` block, add
```rust
    result = apply_erasing(result, &src.strokes, 1.0);
```

g) Directly after the `copy` command function, add:
```rust
/// Add one eraser stroke to the current image; returns how many it now has.
#[tauri::command]
fn erase_stroke(stroke: StrokeIn, state: State<App>) -> Result<usize, String> {
    let mut guard = state.source.lock().unwrap();
    let src = guard.as_mut().ok_or("nothing loaded yet")?;
    src.strokes.push(Stroke::new(stroke.points, stroke.radius, stroke.softness)?);
    Ok(src.strokes.len())
}

/// Drop the most recent stroke; returns how many are left.
#[tauri::command]
fn undo_erase(state: State<App>) -> Result<usize, String> {
    let mut guard = state.source.lock().unwrap();
    let src = guard.as_mut().ok_or("nothing loaded yet")?;
    src.strokes.pop();
    Ok(src.strokes.len())
}

#[tauri::command]
fn clear_erase(state: State<App>) -> Result<(), String> {
    let mut guard = state.source.lock().unwrap();
    let src = guard.as_mut().ok_or("nothing loaded yet")?;
    src.strokes.clear();
    Ok(())
}
```

h) In `generate_handler![…]`, change
```rust
            open_file, capture, screens, render, save, copy,
            begin_pick, pick_open, pick_save, capture_supported, ui_ready
```
to
```rust
            open_file, capture, screens, render, save, copy,
            erase_stroke, undo_erase, clear_erase,
            begin_pick, pick_open, pick_save, capture_supported, ui_ready
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p inklift-gui --test eraser_contract` — Expected: 3 passed.
Run: `cargo test -p inklift-gui` — Expected: all pass, including `frontend_contract` (no JS changed yet, so `every_invoked_command_is_registered` is unaffected).
Run: `cargo clippy -p inklift-gui --tests 2>&1 | grep -B2 -A8 "erase\|StrokeIn\|apply_erasing"` — Expected: no warnings on the new code. (Note: `erase_stroke`/`undo_erase`/`clear_erase` are used via the macro, so no dead-code warning is expected.)

- [ ] **Step 5: Commit**

```bash
git add crates/inklift-gui/src/main.rs crates/inklift-gui/tests/eraser_contract.rs
git commit -F - <<'EOF'
Keep eraser strokes beside the source and apply them to every output

The backend now holds the user's strokes with the source image, dropped with
it when a new image is adopted, and exposes erase_stroke, undo_erase and
clear_erase. render applies them at the proxy's scale and finished at full
size, and Save, Copy and the tray's Copy again all export through finished,
so an erased mark cannot come back in the file the user keeps. A contract
test pins each of those paths.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 3: Eraser interface, and the docs that describe it

**Files:**
- Modify: `crates/inklift-gui/ui/index.html` (CSS block; `<img id="ink">` line 214; ground contents; sidebar after the `#inv` label)
- Modify: `crates/inklift-gui/ui/app.js` (element map; state; `render`; `adopt`; peek/keyboard section)
- Modify: `crates/inklift-gui/tests/eraser_contract.rs` (add two tests)
- Modify: `design/ux-architecture.md:28`, `README.md:255-259`

**Interfaces:**
- Consumes (Task 2): `invoke("erase_stroke", { stroke: { points, radius, softness } }) -> number`, `invoke("undo_erase") -> number`, `invoke("clear_erase") -> null`; `render` returns `{ png, sourcePng, width, height, … }` where `width/height` may be the proxy's; `Loaded` (from `open_file`, `capture`, the `picked` event) carries the source `width`/`height`.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing contract tests**

Append to `crates/inklift-gui/tests/eraser_contract.rs`:

```rust
#[test]
fn the_ui_calls_every_eraser_command() {
    let app = read("ui/app.js");
    for cmd in ["erase_stroke", "undo_erase", "clear_erase"] {
        assert!(
            app.contains(&format!("invoke(\"{cmd}\"")),
            "{cmd} is registered but the UI never calls it"
        );
    }
}

#[test]
fn every_eraser_control_exists_and_is_wired() {
    let html = read("ui/index.html");
    let app = read("ui/app.js");
    for id in ["eraserBtn", "eSize", "eSoft", "undoBtn", "clearBtn", "brush"] {
        assert!(html.contains(&format!("id=\"{id}\"")), "index.html has no #{id}");
        assert!(
            app.contains(&format!("$(\"{id}\")")),
            "app.js never looks up #{id}, so the control does nothing"
        );
    }
    assert!(
        html.contains("<canvas id=\"ink\""),
        "the result must be a canvas: an <img> cannot show a stroke while it is being dragged"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p inklift-gui --test eraser_contract`
Expected: the two new tests FAIL ("erase_stroke is registered but the UI never calls it", "index.html has no #eraserBtn"); the three from Task 2 still pass.

- [ ] **Step 3: Change index.html**

a) In the `<style>` block, directly after the line `.ground.peek #ink{opacity:0} .ground.peek #src{opacity:1}`, add:
```css
/* eraser: the ring is the cursor, so the system one is hidden over the stage */
.ground.erasing{cursor:none}
.brush{position:absolute;border-radius:50%;pointer-events:none;
  box-shadow:0 0 0 1px rgba(255,255,255,.9),inset 0 0 0 1px rgba(30,27,24,.75)}
```

b) Directly after the line `.btn:focus-visible{…}` (the last `.btn` rule), add:
```css
.btn.tool[aria-pressed="true"]{background:color-mix(in oklab,var(--accent) 16%,transparent);color:var(--accent-text);
  box-shadow:inset 0 0 0 1px color-mix(in oklab,var(--accent) 45%,transparent)}
```

c) Directly after the line `input[type=range]:focus-visible{…}`, add:
```css
input[type=range]:disabled{opacity:.45;cursor:default}
```

d) Replace `<img id="ink" alt="Extracted handwriting" hidden>` with
```html
        <canvas id="ink" role="img" aria-label="Extracted handwriting" hidden></canvas>
```
and directly after the `<span class="chip" id="chip"></span>` line add
```html
        <div class="brush" id="brush" hidden></div>
```

e) Directly after `<label class="check"><input type="checkbox" id="inv"><span>Light ink on a dark background</span></label>`, add:
```html

      <div class="field">
        <div class="shead">Clean up</div>
        <button class="btn ghost tool" id="eraserBtn" aria-pressed="false" disabled
                title="Erase leftover marks (E)">Eraser</button>
      </div>

      <div class="field">
        <div class="flabel"><span class="n">Eraser size</span><span class="val" id="eSizeVal">20 px</span></div>
        <input type="range" id="eSize" min="2" max="200" value="20" aria-label="Eraser size" disabled>
      </div>

      <div class="field">
        <div class="flabel"><span class="n">Softness</span><span class="val" id="eSoftVal">25 %</span></div>
        <input type="range" id="eSoft" min="0" max="100" value="25" aria-label="Eraser softness" disabled>
        <div class="ends"><span>Crisp edge</span><span>Feathered</span></div>
      </div>

      <div style="display:flex;gap:8px">
        <button class="btn ghost" id="undoBtn" disabled title="Undo the last stroke (Ctrl+Z)">Undo</button>
        <button class="btn ghost" id="clearBtn" disabled>Clear erasing</button>
      </div>
```

- [ ] **Step 4: Change app.js**

a) In the `el` map, after the line `kVal: $("kVal"), mVal: $("mVal"), rVal: $("rVal"),` add:
```js
  eraser: $("eraserBtn"), eSize: $("eSize"), eSoft: $("eSoft"),
  eSizeVal: $("eSizeVal"), eSoftVal: $("eSoftVal"),
  undo: $("undoBtn"), clear: $("clearBtn"), brush: $("brush"),
```

b) After the line `let previewTimer = null, fullTimer = null, inFlight = false, queued = false;` add:
```js
let source = null;      // the loaded image's size; strokes are in its pixels
let erasing = false;
let strokeCount = 0;    // how many strokes Rust holds, as it last said
let stroke = null;      // the stroke being dragged, in source pixels
```

c) In `render`, replace the line `el.ink.src = r.png;` with `await paint(r.png);`.

d) Directly above `/** Debounced so a dragged slider does not queue a job per pixel. */` add:
```js
/** Draw a rendered PNG into the result canvas. A canvas rather than an <img>
 *  because a stroke has to show while it is being dragged, before Rust has it. */
function paint(uri) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      el.ink.width = img.naturalWidth;
      el.ink.height = img.naturalHeight;
      el.ink.getContext("2d").drawImage(img, 0, 0);
      // a render that lands mid-drag must not wipe the stroke off the screen
      if (stroke) redrawStroke();
      resolve();
    };
    img.onerror = () => reject(new Error("the rendered result could not be decoded"));
    img.src = uri;
  });
}
```

e) In `adopt`, directly after `loaded = true;` add:
```js
  source = { width: info.width, height: info.height };
  strokeCount = 0;   // Rust dropped the old image's strokes with it
  stroke = null;
  [el.eraser, el.eSize, el.eSoft].forEach((c) => { c.disabled = false; });
  syncEraser();
```

f) Replace the whole peek block — from the line `/* ── peek: hold to compare, which is how you actually verify an alpha ──── */` through the line `document.addEventListener("keyup", (e) => { if (e.code === "Space") peek(false); });` — with:
```js
/* ── eraser ────────────────────────────────────────────────────────────── */
//
// The strokes live in Rust, in source pixels, and every render, save and copy
// applies them. The canvas here is only feedback while the pointer is down: it
// cuts the stroke out with the same fall-off as erase.rs, then the normal
// preview → full refresh replaces it with what Rust will actually export.

/** A radial gradient with erase.rs's profile: full strength to the inner
 *  radius, then a smoothstep to nothing at r. */
function brushGradient(ctx, x, y, r, soft) {
  const inner = Math.max(0, Math.min(r * (1 - soft), r - 1));
  const g = ctx.createRadialGradient(x, y, 0, x, y, r);
  for (let i = 0; i <= 8; i++) {
    const t = 1 - i / 8;
    g.addColorStop((inner + (r - inner) * (i / 8)) / r, `rgba(0,0,0,${t * t * (3 - 2 * t)})`);
  }
  return g;
}

/** Cut one brush dab out of the canvas at source pixel (x, y). */
function dab(x, y) {
  const s = el.ink.width / source.width;   // the canvas may hold the proxy
  const r = stroke.radius * s;
  const ctx = el.ink.getContext("2d");
  ctx.save();
  ctx.globalCompositeOperation = "destination-out";
  ctx.fillStyle = brushGradient(ctx, x * s, y * s, r, stroke.softness);
  ctx.beginPath();
  ctx.arc(x * s, y * s, r, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function dabSegment(a, b) {
  const s = el.ink.width / source.width;
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]) * s;
  const n = Math.max(1, Math.ceil(len / Math.max(0.5, stroke.radius * s / 4)));
  for (let i = 1; i <= n; i++) dab(a[0] + (b[0] - a[0]) * i / n, a[1] + (b[1] - a[1]) * i / n);
}

function redrawStroke() {
  const p = stroke.points;
  dab(p[0][0], p[0][1]);
  for (let i = 1; i < p.length; i++) dabSegment(p[i - 1], p[i]);
}

/** Pointer → source-image pixels, whatever size the canvas is shown at. */
function toSource(e) {
  const b = el.ink.getBoundingClientRect();
  return [(e.clientX - b.left) / b.width * source.width,
          (e.clientY - b.top) / b.height * source.height];
}

function moveBrush(e) {
  if (!erasing || !source) { el.brush.hidden = true; return; }
  const b = el.ink.getBoundingClientRect();
  const g = el.ground.getBoundingClientRect();
  const d = Number(el.eSize.value) * b.width / source.width;
  el.brush.style.width = el.brush.style.height = d + "px";
  el.brush.style.left = (e.clientX - g.left - d / 2) + "px";
  el.brush.style.top = (e.clientY - g.top - d / 2) + "px";
  el.brush.hidden = false;
}

function syncEraser() {
  el.eraser.setAttribute("aria-pressed", String(erasing));
  el.ground.classList.toggle("erasing", erasing);
  if (!erasing) el.brush.hidden = true;
  el.peekHint.textContent = erasing ? "drag to erase · hold Space to compare" : "hold to compare";
  el.undo.disabled = strokeCount === 0;
  el.clear.disabled = strokeCount === 0;
}

function setErasing(on) { erasing = on && loaded; syncEraser(); }

async function finishStroke() {
  const s = stroke;
  stroke = null;
  try {
    strokeCount = await invoke("erase_stroke", { stroke: s });
  } catch (e) {
    toast(String(e), true);
  }
  syncEraser();
  // Either way, redraw from Rust: on success the canvas converges on what will
  // export; on failure it drops an erasure that never happened.
  schedulePreview();
}

async function undoStroke() {
  if (!loaded || strokeCount === 0) return;
  try {
    strokeCount = await invoke("undo_erase");
    toast("Stroke undone");
    schedulePreview();
  } catch (e) { toast(String(e), true); }
  syncEraser();
}

async function clearStrokes() {
  if (!loaded || strokeCount === 0) return;
  try {
    await invoke("clear_erase");
    strokeCount = 0;
    toast("Erasing cleared");
    schedulePreview();
  } catch (e) { toast(String(e), true); }
  syncEraser();
}

el.eraser.addEventListener("click", (e) => {
  setErasing(!erasing);
  // A mouse click leaves focus on the button, where Space would toggle it
  // instead of peeking. Keyboard users keep their focus.
  if (e.detail > 0) el.eraser.blur();
});
el.eSize.addEventListener("input", () => { el.eSizeVal.textContent = el.eSize.value + " px"; });
el.eSoft.addEventListener("input", () => { el.eSoftVal.textContent = el.eSoft.value + " %"; });
el.undo.addEventListener("click", undoStroke);
el.clear.addEventListener("click", clearStrokes);

/* ── peek: hold to compare, which is how you actually verify an alpha ──── */
// While erasing, the pointer is busy painting, so comparing moves to Space.
const peek = (on) => loaded && el.ground.classList.toggle("peek", on);
el.ground.addEventListener("pointerdown", (e) => {
  if (!erasing) { peek(true); return; }
  if (e.button !== 0 || !source) return;
  el.ground.setPointerCapture(e.pointerId);
  stroke = { points: [toSource(e)], radius: Number(el.eSize.value) / 2,
             softness: Number(el.eSoft.value) / 100 };
  dab(stroke.points[0][0], stroke.points[0][1]);
});
el.ground.addEventListener("pointermove", (e) => {
  moveBrush(e);
  if (!stroke) return;
  const p = toSource(e);
  const last = stroke.points[stroke.points.length - 1];
  // Thin the path: Rust measures distance to each segment, so sparse points
  // lose nothing in accuracy and save a great deal of rasterising.
  if (Math.hypot(p[0] - last[0], p[1] - last[1]) < Math.max(1, stroke.radius / 4)) return;
  stroke.points.push(p);
  dabSegment(last, p);
});
["pointerup", "pointercancel"].forEach((ev) => el.ground.addEventListener(ev, () => {
  if (stroke) finishStroke();
  peek(false);
}));
el.ground.addEventListener("pointerleave", () => { el.brush.hidden = true; peek(false); });
document.addEventListener("keydown", (e) => {
  if (e.code === "Space" && !e.repeat && e.target === document.body) { e.preventDefault(); peek(true); }
  if (e.key === "Enter" && loaded && !e.ctrlKey) el.copy.click();
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "z") { e.preventDefault(); undoStroke(); }
  if (!e.ctrlKey && !e.metaKey && !e.altKey && e.key.toLowerCase() === "e" && loaded) setErasing(!erasing);
  if (e.key === "Escape" && erasing) setErasing(false);
});
document.addEventListener("keyup", (e) => { if (e.code === "Space") peek(false); });
```

- [ ] **Step 5: Update the docs**

In `design/ux-architecture.md`, replace the sentence
`Not an image editor, annotation tool, or a place to store things.`
with
`Not an image editor, annotation tool, or a place to store things — the one
hand tool is an eraser for leftover marks, and it only ever removes ink.`

In `README.md`, replace
```
than the colour you cut it from; hold-to-compare against the source; and
Save / Copy.
```
with
```
than the colour you cut it from; hold-to-compare against the source; an
eraser with adjustable size and softness for the stray marks no slider should
have to fix, which holds through retuning, Save and Copy (Ctrl+Z undoes a
stroke); and Save / Copy.
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p inklift-gui` — Expected: all pass, including all 5 `eraser_contract` tests and every `frontend_contract` test (`every_invoked_command_is_registered`, `no_ui_catch_block_only_writes_to_the_console`, `every_required_extraction_param_is_sent_by_the_ui`).
Run: `cargo test --workspace --exclude inklift-shot` — Expected: all pass. (`inklift-shot`'s desktop tests drive the real mouse; they run in final validation.)

- [ ] **Step 7: Commit**

```bash
git add crates/inklift-gui/ui/index.html crates/inklift-gui/ui/app.js crates/inklift-gui/tests/eraser_contract.rs design/ux-architecture.md README.md
git commit -F - <<'EOF'
Add an eraser to the desktop app for leftover marks

A Clean up section gives an Eraser toggle (E), size and softness sliders, and
Undo (Ctrl+Z) and Clear. The result is now drawn on a canvas so a stroke shows
while it is dragged; on release it goes to Rust, which owns the strokes, and
the usual preview-then-full refresh replaces the live cut with what will
export. While erasing, hold-to-compare moves to Space. The design doc's
"not an image editor" line gains the one bounded exception.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 4: Validation (controller, not a subagent)

- [ ] `cargo test --workspace` and `cargo test --workspace --features inklift-cli/shot` — all pass (desktop-bound suites skip when locked; if `win_pick` trips its "someone moved the mouse" guard, rerun it alone without touching the mouse).
- [ ] `cargo clippy --workspace --tests` — no warnings in any file this plan touched.
- [ ] Run the real app (`cargo run -p inklift-gui`) with WebView2 remote debugging (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`) and drive it over the DevTools protocol: open a sample, save `before.png`, toggle the eraser, drag a stroke across known ink with real pointer events, save `after.png`. Check in Rust or PowerShell that pixels within `radius × (1 − softness)` of the stroke have alpha 0 in `after.png`, pixels beyond the radius are identical in both files, and pixels elsewhere with ink still have it. Confirm Undo restores `before.png` exactly on a third save, Clear likewise, and that opening another file disables Undo/Clear.
- [ ] Screenshot the window while erasing (ring visible, controls enabled) for the report.
