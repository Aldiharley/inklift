# Action Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the desktop app's seven action buttons a soft clay 3D icon family generated with the Higgsfield CLI.

**Architecture:** One Higgsfield (GPT Image 2.5) generation produces a transparent 2K sheet of all seven icons; `tools/make_icons.py` slices it into 54×54 RGBA PNGs under `crates/inklift-gui/ui/icons/`. `index.html` puts one decorative `<img class="ico">` in each button, 18 px, dimmed further than the label when disabled. Rust contract tests pin the assets and the markup.

**Tech Stack:** Higgsfield CLI 1.1.18, Python 3.13 + Pillow, Tauri 2 vanilla HTML/CSS, Rust tests with the `image` crate (already a dev-dependency of inklift-gui).

**Spec:** `docs/superpowers/specs/2026-09-29-action-icons-design.md`

## Global Constraints

- Work only in the worktree `M:\Projects\inklift-icons` on branch `icons`. Never touch `M:\Projects\inklift-clone` — another session works there.
- The seven buttons, ids and files, exactly: `openBtn`→`open.png`, `grabBtn`→`lift.png`, `eraserBtn`→`eraser.png`, `undoBtn`→`undo.png`, `clearBtn`→`clear.png`, `saveBtn`→`save.png`, `copyBtn`→`copy.png`.
- Icons are 54×54 RGBA PNGs (3× the 18 px display size) in `crates/inklift-gui/ui/icons/`, with transparent corners.
- Markup per button: `<img class="ico" src="icons/<file>" alt="" aria-hidden="true">` as the button's first child; the visible label stays the accessible name.
- Disabled: the icon dims further than the label — `opacity:.6; filter:grayscale(.5)` on top of the button's existing `opacity:.45`.
- The 2K source sheet is NOT committed; the prompt is, verbatim, in `tools/icons-prompt.txt`.
- Test first: write the test, run it, see it fail for the right reason, then implement.
- Comments explain why, not what; match the surrounding register. Never run `cargo fmt`. Add no clippy warnings. Files are CRLF in the checkout — edit in place, do not convert.
- Commit messages: imperative plain-English subject, no prefix, no trailing period; prose body; the last line exactly `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `tools/icons-prompt.txt` | Create | The exact generation prompt |
| `tools/make_icons.py` | Create | Slice a sheet into the seven icon PNGs |
| `crates/inklift-gui/ui/icons/*.png` | Create (7) | The shipped icons |
| `crates/inklift-gui/tests/icons_contract.rs` | Create | Asset and markup contract |
| `crates/inklift-gui/ui/index.html` | Modify | Icons in seven buttons; `.ico` CSS |
| `design/visual-system.md` | Modify | New §10 "Action icons" |

---

### Task 1: Generate and slice the icon set

**Files:**
- Create: `tools/icons-prompt.txt`, `tools/make_icons.py`, `crates/inklift-gui/ui/icons/{open,lift,eraser,undo,clear,save,copy}.png`
- Test: `crates/inklift-gui/tests/icons_contract.rs` (create)

**Interfaces:**
- Produces (for Task 2): the seven files named in Global Constraints; `icons_contract.rs` with `const ICONS: [(&str, &str); 7]` (button id, file name) and `fn ui() -> PathBuf`, which Task 2 extends.

- [ ] **Step 1: Write the failing test**

Create `crates/inklift-gui/tests/icons_contract.rs`:

```rust
//! The action buttons' icons: seven decorative PNGs generated as one sheet and
//! sliced by tools/make_icons.py. They are shown at 18 px and stored at 54 so
//! they stay sharp at 3x; a missing or opaque-cornered file shows up here
//! rather than as a broken image or a white box on the dark theme.

use std::fs;
use std::path::PathBuf;

/// Button id → icon file, in the order the sheet lays them out.
const ICONS: [(&str, &str); 7] = [
    ("openBtn", "open.png"),
    ("grabBtn", "lift.png"),
    ("eraserBtn", "eraser.png"),
    ("undoBtn", "undo.png"),
    ("clearBtn", "clear.png"),
    ("saveBtn", "save.png"),
    ("copyBtn", "copy.png"),
];

fn ui() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

#[test]
fn every_action_icon_is_a_square_transparent_png() {
    for (_, file) in ICONS {
        let path = ui().join("icons").join(file);
        let img = image::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(img.color().has_alpha(), "{file} has no alpha channel");
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        assert_eq!(w, h, "{file} is {w}x{h}, not square");
        assert!(w >= 54, "{file} is {w} px; it needs 54 to stay sharp at 3x its 18 px size");
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert_eq!(rgba.get_pixel(x, y)[3], 0, "{file} corner ({x},{y}) is not transparent");
        }
        let solid = rgba.pixels().filter(|p| p[3] > 200).count() as f32 / (w * h) as f32;
        assert!(solid > 0.15, "{file} is only {:.0}% solid: is anything drawn?", solid * 100.0);
    }
}

#[test]
fn the_generation_prompt_is_kept_with_the_slicer() {
    // The 2K sheet is not committed, so the prompt is what makes the set
    // repeatable; the slicer must exist beside it.
    let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools");
    let prompt = fs::read_to_string(tools.join("icons-prompt.txt")).expect("tools/icons-prompt.txt");
    assert!(prompt.contains("clay"), "the prompt should name the clay style");
    assert!(tools.join("make_icons.py").is_file(), "tools/make_icons.py is missing");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `M:\Projects\inklift-icons`): `cargo test -p inklift-gui --test icons_contract`
Expected: both FAIL — `…ui\icons\open.png: The system cannot find the file…` and `tools/icons-prompt.txt`.

- [ ] **Step 3: Write the prompt**

Create `tools/icons-prompt.txt` with exactly this single paragraph (one line):

```
App UI icon set in a soft pastel 3D clay style: rounded, puffy, matte clay shapes with gentle soft shading and subtle highlights, like the reference image. Seven icons arranged in a neat grid, 4 on the top row and 3 on the bottom row, evenly spaced with generous gaps, each centred in its cell, no text, no labels, no background, no drop shadows or glows around the shapes: (1) open a file: a folder with a picture card (a small landscape photo) peeking out of it; (2) lift from screen: a computer monitor with a dashed selection rectangle on its screen and an upward arrow inside it; (3) eraser: a rubber eraser block, tilted, with a vermilion sleeve and a cream tip; (4) undo: a thick curved arrow turning back to the left; (5) clear erasing: a tilted eraser with three sweeping swoosh motion lines beside it; (6) save: a downward arrow into an open tray; (7) copy: two overlapping sheets of paper. Colour palette, softened to pastel tints: Prussian blue #2B6486, ochre #9A6410, vermilion #B03A2A, verdigris #1F7A5C, rag cream #F2EEE7. Simple, bold silhouettes that stay readable when shown very small. Transparent background.
```

- [ ] **Step 4: Write the slicer**

Create `tools/make_icons.py`:

```python
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
    if v < 170:
        return min(255, (v - 110) * 255 // 60)
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
```

- [ ] **Step 5: Generate the sheet**

The source sheet stays outside the repo. Run (Bash; ~1.5 credits):

```bash
cd /m/Projects/inklift-icons
REF="C:/Users/DENNIS~1/AppData/Local/Temp/claude/M--Projects-inklift/f5e215fa-0f59-448b-8496-578e0e84e94f/images/1.jpg"
OUTDIR="C:/Users/DENNIS~1/AppData/Local/Temp/claude/M--Projects-inklift/f5e215fa-0f59-448b-8496-578e0e84e94f/scratchpad/icons-build"
mkdir -p "$OUTDIR"
higgsfield generate create gpt_image_2_5 --prompt "$(cat tools/icons-prompt.txt)" --image-references "$REF" \
  --quality high --resolution 2k --background transparent --aspect_ratio 3:2 --wait --json > "$OUTDIR/job.json"
URL=$(grep -o '"https[^"]*[0-9a-f]\.png"' "$OUTDIR/job.json" | tr -d '"' | head -1)
curl -s -o "$OUTDIR/sheet.png" "$URL"
```

Expected: `$OUTDIR/sheet.png` is an RGBA PNG about 2K wide. If the CLI returns HTTP 503, retry the same command once. If the result's `.png` URL is missing, stop and report NEEDS_CONTEXT with `job.json`'s contents.

- [ ] **Step 6: Slice it**

Run: `python tools/make_icons.py "$OUTDIR/sheet.png"`
Expected: seven lines `open: box …`, `lift: …` … `copy: …`, and seven files in `crates/inklift-gui/ui/icons/`. If it reports a count other than 7, report NEEDS_CONTEXT with the output — do not regenerate on your own (the controller judges the artwork).

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p inklift-gui --test icons_contract` — Expected: 2 passed.

- [ ] **Step 8: Commit**

```bash
git add tools/icons-prompt.txt tools/make_icons.py crates/inklift-gui/ui/icons crates/inklift-gui/tests/icons_contract.rs
git commit -F - <<'EOF'
Generate the action icons as one clay sheet and slice it

One Higgsfield generation draws all seven action icons in the soft clay
style, so lighting and palette match across the set; tools/make_icons.py
finds them by their opaque pixels, drops the soft glow the model paints
around each shape, and writes 54 px PNGs for 18 px display. The prompt is
kept verbatim beside the slicer since the 2K sheet itself is not committed.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

In the report, list the absolute paths of `sheet.png` and the seven icons so the controller can look at them.

---

### Task 2: Put the icons in the buttons, and document them

**Files:**
- Modify: `crates/inklift-gui/ui/index.html` (CSS after line 72 `.btn:focus-visible{…}`; buttons at lines 194-195, 266-267, 282-283, 296-297)
- Modify: `crates/inklift-gui/tests/icons_contract.rs` (append two tests)
- Modify: `design/visual-system.md` (new §10 before `## Risk register`)

**Interfaces:**
- Consumes (Task 1): the seven PNGs in `crates/inklift-gui/ui/icons/`; `ICONS` and `ui()` in `icons_contract.rs`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/inklift-gui/tests/icons_contract.rs`:

```rust
/// A button's markup from its opening `<button` to its `</button>`.
fn button(html: &str, id: &str) -> String {
    let at = html.find(&format!("id=\"{id}\"")).unwrap_or_else(|| panic!("index.html has no #{id}"));
    let open = html[..at].rfind("<button").unwrap_or_else(|| panic!("#{id} is not a button"));
    let close = at + html[at..].find("</button>").unwrap_or_else(|| panic!("#{id} is never closed"));
    html[open..close].to_string()
}

#[test]
fn every_action_button_carries_its_icon_decoratively() {
    let html = fs::read_to_string(ui().join("index.html")).expect("index.html");
    for (id, file) in ICONS {
        let b = button(&html, id);
        assert_eq!(b.matches("<img").count(), 1, "#{id} should hold exactly one icon: {b}");
        assert!(b.contains(&format!("src=\"icons/{file}\"")), "#{id} should show icons/{file}: {b}");
        assert!(b.contains("class=\"ico\""), "#{id}'s icon needs class=\"ico\" for its size: {b}");
        // The visible label is the accessible name; an alt text would make a
        // screen reader say "Eraser, Eraser".
        assert!(
            b.contains("alt=\"\"") && b.contains("aria-hidden=\"true\""),
            "#{id}'s icon must be decorative: {b}"
        );
    }
}

#[test]
fn every_icon_the_page_references_exists() {
    let html = fs::read_to_string(ui().join("index.html")).expect("index.html");
    let mut missing = Vec::new();
    let mut rest = html.as_str();
    while let Some(i) = rest.find("src=\"icons/") {
        rest = &rest[i + 5..];
        let path = rest.split('"').next().unwrap_or("");
        if !ui().join(path).is_file() {
            missing.push(path.to_string());
        }
    }
    assert!(missing.is_empty(), "index.html shows icons that do not exist: {missing:?}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p inklift-gui --test icons_contract`
Expected: `every_action_button_carries_its_icon_decoratively` FAILS ("#openBtn should hold exactly one icon"); the other three pass (`every_icon_the_page_references_exists` passes vacuously until markup exists).

- [ ] **Step 3: Add the CSS**

In `crates/inklift-gui/ui/index.html`, directly after the line
`.btn:focus-visible{outline:2px solid var(--accent);outline-offset:2px;box-shadow:0 0 0 4px rgba(0,0,0,.30)}`
add:

```css
/* action icons are decorative; the label stays the button's name. Disabled
   dims the icon further than the label, as visual-system.md §3 rule 5 asks */
.btn .ico{width:18px;height:18px;flex:none;pointer-events:none;-webkit-user-drag:none}
.btn:disabled .ico{opacity:.6;filter:grayscale(.5)}
```

- [ ] **Step 4: Add the icons to the buttons**

In `crates/inklift-gui/ui/index.html` make exactly these replacements (keep indentation):

- `<button class="btn ghost" id="openBtn">Open a file…</button>` →
  `<button class="btn ghost" id="openBtn"><img class="ico" src="icons/open.png" alt="" aria-hidden="true">Open a file…</button>`
- `<button class="btn ghost" id="grabBtn">Lift from screen</button>` →
  `<button class="btn ghost" id="grabBtn"><img class="ico" src="icons/lift.png" alt="" aria-hidden="true">Lift from screen</button>`
- `title="Erase leftover marks (E)">Eraser</button>` →
  `title="Erase leftover marks (E)"><img class="ico" src="icons/eraser.png" alt="" aria-hidden="true">Eraser</button>`
- `title="Undo the last stroke (Ctrl+Z)">Undo</button>` →
  `title="Undo the last stroke (Ctrl+Z)"><img class="ico" src="icons/undo.png" alt="" aria-hidden="true">Undo</button>`
- `<button class="btn ghost" id="clearBtn" disabled>Clear erasing</button>` →
  `<button class="btn ghost" id="clearBtn" disabled><img class="ico" src="icons/clear.png" alt="" aria-hidden="true">Clear erasing</button>`
- `<button class="btn ghost"  id="saveBtn" disabled>Save…</button>` (note: two spaces before `id`) →
  `<button class="btn ghost"  id="saveBtn" disabled><img class="ico" src="icons/save.png" alt="" aria-hidden="true">Save…</button>`
- `<button class="btn primary" id="copyBtn" disabled>Copy</button>` →
  `<button class="btn primary" id="copyBtn" disabled><img class="ico" src="icons/copy.png" alt="" aria-hidden="true">Copy</button>`

- [ ] **Step 5: Document the icons**

In `design/visual-system.md`, directly before the line `## Risk register`, add:

```markdown
## 10. Action icons

The seven action buttons — Open, Lift from screen, Eraser, Undo, Clear
erasing, Save, Copy — carry one icon family in a **soft clay 3D** style, tinted
toward the pigment palette. Chosen over a flat bold outline, whose dark outline
vanished on the dark theme at 18 px, and glossy 3D, whose highlights turned to
noise at 18 px — both were generated and compared at real size in both themes.

- **Size:** shown at 18 px, stored at 54 px (3×) in
  `crates/inklift-gui/ui/icons/`.
- **Decorative:** `alt=""` and `aria-hidden="true"`; the visible label is the
  accessible name.
- **Disabled:** the icon dims further than the label (§3 rule 5):
  `opacity:.6; filter:grayscale(.5)` on top of the button's 0.45.
- **Regenerating:** one Higgsfield `gpt_image_2_5` generation (2K, high,
  transparent, the clay reference image) with the prompt in
  `tools/icons-prompt.txt`, then `python tools/make_icons.py sheet.png`. One
  sheet, never seven generations: siblings drawn separately drift in lighting
  and colour. The 2K sheet is not committed.

```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p inklift-gui` — Expected: all pass, including the 4 `icons_contract` tests and every `frontend_contract` / `eraser_contract` test (the eraser contract checks `id="eraserBtn"` etc. still exist in index.html).

- [ ] **Step 7: Commit**

```bash
git add crates/inklift-gui/ui/index.html crates/inklift-gui/tests/icons_contract.rs design/visual-system.md
git commit -F - <<'EOF'
Show the clay icons on the app's action buttons

Each action button now leads with its icon at 18 px. The icons are
decorative, so the visible label stays the accessible name, and a disabled
button dims its icon further than its label as the visual system asks. A
contract test pins every button to its file and every referenced file to
the disk; the visual system gains a section on the family and how to
regenerate it.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
```

---

### Task 3: Validation (controller, not a subagent)

- [ ] `cargo test --workspace` and `cargo test --workspace --features inklift-cli/shot` — all pass.
- [ ] `cargo clippy -p inklift-gui --tests` — no warnings in `icons_contract.rs`.
- [ ] Build and run the real app (`cargo build -p inklift-gui`, launched with its own `WEBVIEW2_USER_DATA_FOLDER` and a DevTools port). Screenshot it: light theme with nothing loaded (eraser, undo, clear, save, copy disabled), light theme with a sample loaded and the eraser active, and the same in dark theme. Check the icons are crisp, aligned with their labels, and that disabled icons are visibly dimmer than their labels.
