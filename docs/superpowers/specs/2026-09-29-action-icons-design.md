# Action icons — design

Date: 2026-09-29. Status: built on branch `icons`.

## Problem

Every action button in the desktop app is text only. The eraser shipped as a
bare word, and next to it the rest of the chrome gives no visual anchor for
what a button does. The owner wants an icon family, generated with the
Higgsfield CLI, in the style of a reference set of soft pastel 3D clay icons.

## Decisions

- **Scope:** the seven action buttons — Open a file…, Lift from screen,
  Eraser, Undo, Clear erasing, Save…, Copy. The ground swatches and the ☀/☾
  theme toggle already carry visuals and are unchanged. Tray menu items are out
  of scope.
- **Style: soft clay 3D** (the owner's first reference), tinted toward
  inklift's pigment palette. Chosen over a flat bold outline (its navy outline
  vanished on the dark theme at 18 px) and glossy 3D (highlights turned to
  noise at 18 px) after seeing all three generated and shown at real size in
  both themes.
- **Production:** one high-quality sheet of all seven icons from a single
  generation, sliced by a script — one image keeps lighting, texture and
  palette identical across the family. Rejected: one generation per icon
  (lighting and colour drift between siblings); shipping the medium-quality
  sample sheet (≈200 px per icon, and two glyphs too busy).

## 1. The icon set

| Control | Button id | File | Glyph |
|---|---|---|---|
| Open a file… | `openBtn` | `open.png` | a folder with a picture card peeking out |
| Lift from screen | `grabBtn` | `lift.png` | a monitor with a dashed selection rectangle and an upward arrow |
| Eraser | `eraserBtn` | `eraser.png` | a tilted eraser, vermilion sleeve and cream tip |
| Undo | `undoBtn` | `undo.png` | a curved arrow turning back to the left |
| Clear erasing | `clearBtn` | `clear.png` | an eraser with a sweeping swoosh of motion lines |
| Save… | `saveBtn` | `save.png` | a downward arrow into a tray |
| Copy | `copyBtn` | `copy.png` | two overlapping sheets |

Palette in the prompt: Prussian blue `#2B6486`, ochre `#9A6410`, vermilion
`#B03A2A`, verdigris `#1F7A5C`, rag cream `#F2EEE7`, softened to pastel tints.

## 2. Asset pipeline

- **Generate** (by hand, once): `higgsfield generate create gpt_image_2_5`
  with `--quality high --resolution 2k --background transparent
  --aspect_ratio 3:2`, the owner's clay reference as `--image-references`, and
  the prompt stored verbatim in `tools/icons-prompt.txt`. The sheet lays out
  the seven icons 4 on the top row and 3 on the bottom, in the table's order.
- **Slice:** `tools/make_icons.py <sheet.png>` finds the seven icons by their
  opaque pixels (row bands, then column spans within each band, in reading
  order), crops each to a centred square with 6 % padding, removes the soft
  coloured glow the model paints around each shape (alpha below 110 → 0,
  110–170 ramped to full, above kept), and writes
  `crates/inklift-gui/ui/icons/<name>.png` at **54×54 RGBA** — 3× the 18 px
  display size. It fails loudly if it does not find exactly seven icons.
- **Not committed:** the 2K source sheet (a few MB; the owner has removed large
  images before). Regenerating means re-running the command above and the
  script; the prompt file makes that repeatable in wording, not pixel-exact.

## 3. Interface

- Each of the seven buttons gets, as its first child,
  `<img class="ico" src="icons/<name>.png" alt="" aria-hidden="true">`. The
  icon is decorative: the visible label stays the accessible name.
- CSS: `.btn .ico{width:18px;height:18px;flex:none}`; the existing 7 px gap in
  `.btn` spaces it from the label.
- Disabled: `design/visual-system.md` §3 rule 5 — the icon dims further than
  the label. The button is already at `opacity:.45`; the icon additionally
  gets `opacity:.6; filter:grayscale(.5)`.
- No per-theme variants: the clay set was checked on both themes and reads on
  each. The CSP already permits `img-src 'self'`.
- `app.js` never rewrites these buttons' contents, so no script changes.

## 4. Testing

Test first (`docs/porting-capture.md` §10). A new
`crates/inklift-gui/tests/icons_contract.rs`:

- Every `icons/*.png` referenced from `ui/index.html` exists, decodes, is
  square, at least 54 px, and has an alpha channel with transparent corners.
- Each of the seven button ids contains exactly one `<img class="ico" …>` with
  `alt=""`, and the file it names is the one in §1's table.

Validation before done: `cargo test --workspace` and the capture-feature run;
run the real app and screenshot it in light and dark themes with the eraser
disabled (nothing loaded) and active.

## 5. Docs

`design/visual-system.md` gains a short "Action icons" section: the style, the
18 px size, the disabled rule, and how to regenerate with
`tools/icons-prompt.txt` and `tools/make_icons.py`.

## Out of scope

Tray menu icons, the app/installer icon, an empty-state illustration, icons
for the segmented controls.
