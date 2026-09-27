# inklift — Information Architecture and Interaction Spec

Source: UX architecture pass, 2026-09-27. Grounded in the shipped CLI behaviour
(README, `docs/screenshot-feature.md`, the invert detector) and in the Greenshot
tray menu supplied as a structural reference (`design/ref/reference.jpg`).

## 1. Product framing

**What it is.** inklift is a cutout tool for handwriting. You point it at
anything with writing in it — a photo of your notebook, a region of your screen,
an image on your clipboard — and it hands back just the ink, with the paper
gone. The output is a PNG with a real alpha channel, already on your clipboard,
that you drop onto a slide, a dark-themed note, a coloured page, a photo, and it
looks like it was written there. Runs entirely on your machine, about a fifth of
a second for a typical region.

**What it is not.** Not a screenshot tool — capture is one of four ways to feed
it an image, in the menu for the same reason "Open file" is. Not a scanner or
document cleaner; "clean scan on white" is a commodity and CamScanner won it.
Not OCR — nothing turns ink into text, and the value is that the strokes stay
yours. Not an image editor, annotation tool, or a place to store things.

One sentence to hold: **background removal, but for handwriting, and the removal
is the product.**

## 2. Primary flows, ranked

### Flow 1 — Lift from screen (highest frequency)

The hotkey makes this the cheapest path to a result, so it becomes the default
habit even for users who started with files.

- **Entry:** global hotkey, or tray "Lift from screen…"
- **Steps:** screen grabbed first → frozen frame shown dimmed to ~45% → drag →
  selection undimmed with a live `W × H` readout at the cursor → release →
  overlay vanishes → progress chip appears *at the selection's screen position*
  (not a new window — keeps the user's eyes where they were) → result lands on
  the clipboard → result window opens.
- **Decision points:**
  1. *Where to draw the box.* The paper estimator needs background around the
     strokes. First-run hint in the readout: "Include a little blank paper
     around the writing." A box cropped hard to the glyphs gives the estimator
     nothing to estimate.
  2. *How big.* Above ~1.5 MP the readout appends a cost estimate
     ("640 × 220 · instant" / "2560 × 1080 · about 20 s"). Put the cost where
     the decision is made, not in a dialog after.
- **Exit:** Esc closes the window, result stays on the clipboard. Or tab away
  and Ctrl+V in the destination.
- **Cancel:** Esc or right-click during drag; a selection under 8×8 px is a
  misclick and cancels silently. No files, ever.

### Flow 2 — Retune the result you just got

Follows roughly a third of lifts, and decides whether people keep the app.
Specified in full in §5.

- **Entry:** the result window *is* the retune surface — no mode switch, no
  second window. Also from the tray ("Retune…") and the toast.
- **Exit:** Copy (rewrites the clipboard), Save…, or Esc.

### Flow 3 — Lift from clipboard

Cheap, and slots into an existing habit: images already arrive on the clipboard
from browsers, Slack, other capture tools.

- **Entry:** hotkey or tray item. Disabled when the clipboard holds no image
  (re-checked on menu open).
- **Edge case:** if the incoming image already has an alpha channel with >50%
  fully transparent pixels, it is probably an inklift result being fed back in.
  Warn inline: "This already looks like extracted ink. Re-extracting will thin
  the strokes." Offer "Retune the original" when that source is still in Recent.

### Flow 4 — Lift from a file

Lower frequency, higher stakes: photos of real notebooks, the case the algorithm
was built for and the one most likely to need retuning.

- **Entry:** tray item, drag-and-drop onto tray icon or main window, or OS
  "Open with".
- **Steps:** decode → cost estimate if large → extract → window, with the
  "Photo of paper" preset pre-selected.
- **Multiple files:** processed sequentially into the Recent strip, with "Apply
  these settings to all" and "Save all…". No batch configuration screen beyond
  that — resist it.

## 3. Tray menu spec

The reference menu is nine verbs of capture plus upload destinations. Ours needs
four verbs of acquisition, then spends its remaining real estate on the two
things Greenshot has no concept of: **what shape the output takes** and **what
kind of source this is**. Both visible as state, at a glance, without opening
anything.

```
  Lift from screen…                     Ctrl+Alt+I
  Lift the last region again            Ctrl+Alt+R
  Lift from clipboard                   Ctrl+Alt+V
  Lift from a file…                     Ctrl+Alt+O
 ───────────────────────────────────────────────────
  Output
    ● Transparent PNG
    ○ Grey ink on white
    ○ Both files
 ───────────────────────────────────────────────────
  Ink: As written                                ▸
  Source: Screen text                            ▸
 ───────────────────────────────────────────────────
  Last result  ·  640 × 220  ·  2.9% ink
  Copy again                            Ctrl+Alt+C
  Save as…
  Retune…                               Ctrl+Alt+T
 ───────────────────────────────────────────────────
  Open inklift
  Settings…
 ───────────────────────────────────────────────────
  About inklift
  Quit inklift
```

**Ink submenu:**

```
    ● As written           the pen that was actually found
    ○ Black
    ○ White                for pasting onto a dark slide
    ○ Custom…              #RRGGBB
```

Sits beside Output rather than under Source, because it is a property of what
comes *out*, not of what went in. The default is never overridden silently: "As
written" is the honest answer and stays selected until someone chooses otherwise.

**Source submenu:**

```
    ● Screen text          k 0.20 · specks 8
    ○ Photo of paper       k 0.20 · specks 30
    ○ Faint pencil         k 0.12 · specks 8
    ○ Blurry or upscaled   k 0.12 · specks 30
    ○ Custom…
   ────────────────────────────────────────
    Light ink on a dark background        ▸
      ● Detect automatically
      ○ Always flip
      ○ Never flip
```

**Grouping rationale:**

- **Group 1 (acquisition)** — four means to one end, ordered by frequency.
  "Lift" as the verb throughout, never "capture": the verb names the product,
  and using "capture" in the top item hands the framing away in the first line.
- **Group 2 (Output)** — inline radios, not a submenu, because it is the
  differentiator and its state must be legible without a hover. This is the
  real-estate trade the reference cannot make.
- **Group 3 (Source)** — submenu with the current value on the parent row. Five
  items is too many inline, but the state still has to show. Preset values are
  printed so a user can learn the mapping and later reach for the CLI.
- **Group 4 (last result)** — replaces "Open last capture location". The header
  carries the two numbers the core already computes (coverage, size): the
  cheapest possible "did that work?".
- **Groups 5–6** — app-level, conventional.

**Accelerators.** All globals on `Ctrl+Alt+` deliberately. `Ctrl+Shift+<letter>`
is claimed app-side by every browser and editor, and a global grab steals it
system-wide — binding `Ctrl+Shift+V` would break paste-without-formatting
everywhere. `PrtScn` and `Shift+PrtScn` ship as equal-rank alternates for region
and last-region. All rebindable with live conflict detection.

**Disabled conditions:**

| Item | Disabled when |
|---|---|
| Lift from screen… | Overlay or extraction in flight; or Wayland session (replaced by an explanatory non-clickable row — §6) |
| Lift the last region again | No region captured since install / cache purge |
| Lift from clipboard | Clipboard holds no image (re-checked every menu open) |
| Lift from a file… | Extraction in flight |
| Output radios | Never — `--white` is a reversible greyscale view of the same data |
| Source preset / light-on-dark | Never; changing them with a result open re-runs it live |
| Last result header, Copy again, Save as…, Retune… | No result this session and none recoverable from cache |
| Quit | Never — but see the clipboard-ownership warning in §6 |

**Removed from the reference, and why.** "Capture window" / "from list": a
window boundary has no relationship to where ink is, and region covers the one
real case. "Imgur / external commands / Box": upload destinations reframe this
as a sharing tool and would add a network surface to a binary that currently
proves, by symbol inspection, that it has none. If wanted later, make it
"Send to…" with OS share sheets, not hosted destinations.

## 4. Main window spec

**Position: a single-result review-and-retune surface, not a library.**

A library implies inklift owns your files. It does not. The output's half-life is
about thirty seconds — created, put on the clipboard, landed in someone else's
document, and its real home is that document. A gallery adds storage policy,
naming, search, and a second place to look, for objects that have already left.
Review and retune are unavoidable: the failure modes (dark source, over-
aggressive speck removal, hollow thick strokes) are invisible in a notification
and fatal in a deck.

The one genuine recurring need a library would serve is "I lifted three lines and
pasted the wrong one." That is served by a **shallow Recent strip** — last 10,
purged at 10 items or 7 days, collapsed behind a "Recent" button. No naming, no
folders, no search, no export-all. Keep it structurally incapable of becoming a
library.

**When it appears.** After every lift, by default. The clipboard write is *not*
gated on it — the result is on the clipboard the moment it exists, so the window
is confirmation plus escape hatch, never a required step. Esc dismisses. A
preference "Don't show the window after a lift" (off by default) replaces it
with a 4-second corner toast carrying a 120px thumbnail *on checkerboard* and a
"Retune" link.

**Layout.** Default 1040 × 700, min 900 × 620, resizable. Below 780px the
sidebar collapses behind a button.

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [Screen region 640×220]        Recent ▸    ⌂ fit  25%—400%    [hold: ⎵]  │ 48px
├──────────────────────────────────────────────────┬───────────────────────┤
│                                                  │ SOURCE                │
│                                                  │ ( Screen text      ▾) │
│                                                  │                       │
│                  [ result on                     │ Pick up               │
│                    checkerboard ]                │  Bold only ──●── Faint│
│                                                  │  2.9% ink             │
│                                                  │                       │
│                                                  │ Remove specks         │
│                                                  │  Keep all ──●──  More │
│                                                  │  412 specks removed   │
│  ┌────────────────────────────────┐              │                       │
│  │ Preview on: ▦ ■white ■black ■◆ │              │ Thickest stroke       │
│  └────────────────────────────────┘              │  [ 6 px ] [Measure…]  │
│                                                  │                       │
│                                                  │ ☐ Light ink on a dark │
│                                                  │   background          │
│                                                  │                       │
│                                                  │ ▸ Advanced            │
│                                                  │ ─────────────────────│
│                                                  │ pen ■ #1E266B         │
│                                                  │ stroke ≈ 6.2 px       │
│                                                  │ region 512,340,640,220│
│                                                  │ Copy as a command ⧉   │
├──────────────────────────────────────────────────┴───────────────────────┤
│ [Reset]                        Output (●Transparent ○White ○Both)        │ 64px
│                                              [ Save… ]   [ Copy  ⏎ ]     │
└──────────────────────────────────────────────────────────────────────────┘
```

**Region hierarchy:**

1. **Canvas (dominant).** Checkerboard by default — 8px squares,
   `#fafafa`/`#e9e9e9` light, `#333`/`#2a2a2a` dark. The one moment the
   differentiator is visible, so never a plain white canvas. Fit-to-window,
   max 100% on open.
2. **"Preview on" swatches**, docked to the canvas, *not* the sidebar.
   Checkerboard / white / black / slide-blue / custom / drop-an-image. **A
   viewing control that must never merge with the Output radios** — merging
   makes "white" read as destructive and hides that `--white` is a reversible
   greyscale encoding of the same alpha.
3. **Sidebar (320px fixed):** preset → four controls → Advanced disclosure →
   diagnostics. Ordered by how often a user touches it.
4. **Bottom bar:** Output radios (mirroring the tray) plus the two exits. Copy
   is the default action, bound to Enter. "Close after copy" defaults on.

**Drag-out is first-class.** Dragging the image off the canvas drops the
transparent PNG directly into a slide or note app — shorter than Copy for the
primary use case, and it demonstrates transparency in the same gesture.

**"Copy as a command"** puts
`inklift shot --region 512,340,640,220 --k 0.14 --min-area 8` on the clipboard.
Makes the GUI a teaching surface for the CLI rather than a replacement that
strands power users.

**Empty state** (opened from the tray with no result): canvas becomes a drop
target carrying the three entry actions, plus a bundled sample result sitting on
the live "Preview on" swatches, so the pitch runs when nothing has been lifted.

## 5. The retune loop

### The precondition

**The raw source is captured and kept, always, automatically.** Every
acquisition writes the untouched full-resolution source into a session cache
(memory + disk, last 10, purged with Recent). This is `--keep-raw` promoted from
an option to an invariant. **Do not expose it as a GUI setting** — it is the
mechanism that makes retune-without-recapture possible, and offering to turn it
off offers to break the best feature in the app.

### Live preview

- On any control change, cancel the in-flight preview and start a new one on a
  **proxy** with long edge ≤ 900px (the measured 150 ms / 900×500 budget).
  Debounce 60 ms while dragging; commit a full-resolution pass 200 ms after the
  last input.
- **Correctness requirement:** every pixel-denominated parameter must be scaled
  by the proxy factor `s`, or the preview does not predict the result. `radius`
  and `window` scale by `s`, `feather` by `s`, `min-area` by `s²` (floor 1).
  Getting this wrong produces a preview that is confidently, subtly wrong — the
  worst possible outcome for a tuning UI.
- Full-screen cost is superlinear in area (2.76 MP ≈ 20 s against 0.45 MP at
  150 ms), so the proxy buys more than proportional time. If a proxy pass
  measures over 150 ms, halve again for the next one, adaptively.
- While the full-resolution pass is pending, show the upscaled proxy with a
  small "Refining…" chip. Never blank the canvas, never freeze a slider.
- On completion, swap silently and rewrite the clipboard. The chip flashes
  "Updated on your clipboard" for 1.5 s.

### Before and after

- **Hold Space** (or hold the header button) shows the original source, in
  place, at the same zoom and pan. Release returns.
- Arguing *against* a split/wipe slider. A wipe compares two similar images;
  here "before" is a photograph and "after" is a cutout on a checkerboard, and a
  moving seam between those reads as damage. Hold-to-compare puts both states in
  the same pixels a quarter-second apart, which is what actually reveals a
  missing i-dot.
- **Backslash** toggles the previous committed settings, so you can A/B your own
  change and not just your change against the photo.
- **Ctrl+Z** steps the settings stack back with a toast naming what moved:
  "Pick up 0.20 → 0.14".

### The four controls, in plain language

| CLI | Label | Control | Copy and feedback |
|---|---|---|---|
| `--k` | **Pick up** | Slider, "Bold ink only" ↔ "Every faint mark" | **Polarity is reversed** — lower k means more pickup, so the raw value must be inverted before it reaches the slider or the control fights the user. Readout: "2.9% ink" with a delta arrow. |
| `--min-area` | **Remove specks** | Slider, "Keep everything" ↔ "Remove printed noise" | Readout: "412 specks removed". When the source is a screen capture and the value exceeds ~15: "At this level, i-dots and full stops start to go." The documented trap deserves a named guard, not a tooltip. |
| `--radius` | **Thickest stroke** | Numeric px field + **[Measure…]** | Reframed from "paper-estimate radius" to the physical thing it must exceed. Measure puts a crosshair on the canvas; drag a short line across the fattest stroke and the app sets `radius = ceil(len/2) + 2`. Helper: "Raise this if thick strokes come out hollow." |
| `--invert` | **Light ink on a dark background** | Checkbox + banner (§6) | Named after the source, not the operation. A user can look at their screenshot and answer it; nobody can answer "invert?". |
| `--ink` | **Ink colour** | Swatch row: As written / Black / White / custom | The extracted pen is the *real* one, which is usually dark — and dark ink is invisible on a dark slide. Because opacity and colour are stored separately, this is a swap with **no re-extraction**, so the preview is instant and lossless. Put it directly under the preview swatches: the user discovers it at the exact moment they drop their ink on a dark ground and cannot see it. |

**Advanced** (collapsed): **Edge softness** (`--feather`), **Lighting detail**
(`--window`), and the raw numeric value of every parameter next to its real flag
name — the bridge back to the CLI.

**Presets** sit above the sliders, auto-selected from the source: a screen region
defaults to "Screen text", a decoded camera file to "Photo of paper". Touching
any slider flips the chip to "Custom" without discarding values.

**A v1.1 nudge worth planning for:** ring-shaped connected components (outline
recovered, interior missing) are a reliable signature of `radius` being too
small. Surface "Thick strokes look hollow — try raising Thickest stroke" with a
one-click fix. It is the one failure a user cannot diagnose from the picture.

## 6. Empty, loading, error and edge states

**Loading.** Two phases. Capture is instant; extraction is 0.15 s to 20 s. Show
the pipeline's own stages, because they are honest and because they are the
vocabulary a later failure message will need: "Estimating the paper…" →
"Dividing out the lighting…" → "Finding the ink…" → "Cleaning up specks…" →
"Colouring the pen…". Cancellable. Under 400 ms, show nothing — no flash of
progress.

**The invert hint.** The detector is a median-luminance test deliberately made
advisory, on the reasoning that guessing wrong *silently* is worse than not
guessing. Diverging narrowly: in a CLI a wrong guess produces a wrong file; in a
GUI it produces one visible click of undo. So **auto-apply the flip, and say so
loudly and reversibly.**

> ⟲ **Treated as light ink on a dark background.** Your source looked
> light-on-dark, so inklift flipped it before extracting. `[Undo the flip]`

Persistent banner above the canvas, not a modal, not a toast. If "Never flip" is
set, the banner instead offers `[Flip it]` and the smudge stays on screen as its
own evidence.

**Almost no ink** (coverage < 0.3%). Never show an empty checkerboard — it is
indistinguishable from a rendering bug. Show the *source* at 50% opacity behind
the message, causes ranked by likelihood, each actionable:

> **Almost nothing came out.** 0.1% of this region was read as ink.
> · The writing may be light on a dark background — `[Flip it]`
> · Pick up may be too conservative — `[Show me at "Faint"]`
> · Remove specks may be eating it — `[Keep everything]`
> · There may not be ink in this region — `[Lift a different region]`

**Invisible on the chosen ground.** When the preview ground is dark and the ink
is dark (or the reverse), the canvas looks broken rather than wrong. Detect the
collision — ink luminance within ~0.25 of the ground's — and offer the fix
inline, since it costs nothing:

> **This ink is almost the same tone as the background.** `[Paint it white]`
> `[Paint it black]`

Never auto-recolour. The extracted pen is a fact about the source, and
overwriting it silently would make the tool untrustworthy about the one thing it
is for.

**Too-low resolution.** Estimate median stroke width from the raw *before*
spending the extraction. Below 2 px:

> **These strokes are about 1.4 px wide.** Below roughly 2 px the information
> isn't in the image, and the result will have a washed-out pen colour rather
> than a crisp one. `[Extract anyway]` `[Capture it larger]`

"Capture it larger" is the real fix and should be the primary button.

**Wayland / no capture backend.** The three capture items become a single
non-clickable row: "Screen capture needs X11 on this system — Wayland support is
not built yet." File and clipboard paths stay fully functional. Never a silent
failure, never a disabled item with no explanation.

**Clipboard ownership.** On X11 and Wayland the clipboard is served by the owning
process, which is why the CLI had to spawn a detached holder. **The tray app
being resident solves this for free — it owns the selection itself.** That is the
single strongest argument for the tray-resident architecture. Consequence:
quitting while inklift owns the clipboard loses the result. On Quit, if no
clipboard manager is detected and a result is held:

> **The extraction on your clipboard will be lost when inklift quits.**
> `[Save it first]` `[Quit anyway]`

**Faults must never pose as cancellations.** This bit the CLI once already.
GUI rule: an overlay session that ends without a user action is a fault and
shows its cause. "Cancelled" is reserved for Esc, right-click, and sub-8×8
selections.

**Other states:** unsupported/corrupt file → name the file and the reason;
unwritable save path → keep the result live and re-prompt, never discard; a
replayed region now off-screen → "That region isn't on any current screen" plus
`[Pick a new one]`; already-transparent input → the warning in Flow 3.

## 7. Onboarding

Three screens, skippable, under thirty seconds. The only job is that
transparency lands before anything else does.

**Screen 1 — the demonstration, with no paragraph attached.** A bundled sample of
real handwriting, already extracted, on a checkerboard, above a row of four
backdrops: white slide, dark slide, yellow legal pad, a photograph. Clicking a
swatch slides the ink onto it. That interaction *is* the pitch; the only text is
"Lift handwriting off anything and put it anywhere." No feature list.

**Screen 2 — the hotkey.** One pre-filled field with live conflict detection,
because a tray app whose hotkey is unset is a tray app nobody opens. On macOS
this is also where Screen Recording permission is requested, with the reason
stated plainly — a hard blocker, and burying it guarantees a dead first run.

**Screen 3 — "Try it now"**, launching the region overlay on the user's own
screen. The first real result opens with a single one-time coachmark over the
bottom bar: **"It's already on your clipboard. Press Ctrl+V in your slides."**
Nothing else annotated. The "Preview on" swatches are the second thing they
should discover, and screen 1 has already taught them to look.

No account, no tour, no tips carousel, no sample-file browser.

## 8. Judgement calls and flagged risks

- **No library.** Recent capped at 10 items / 7 days, no naming or search,
  deliberately, so it cannot grow into one.
- **The window appears every time, but the clipboard never waits for it.**
  Toast-only exists as a preference, off by default, because the two worst
  failures (smudge, empty result) are invisible in a toast.
- **Invert auto-applies** with a persistent undo banner, diverging from the
  core's "suggest, don't switch". The objection there is to *silence*, and a
  banner is not silent.
- **`--keep-raw` is never exposed.** It is the retune precondition.
- **Canvas background ≠ export format.** Two controls, two places, deliberately.
- **Globals live on `Ctrl+Alt+`**, never `Ctrl+Shift+<letter>`.
- **`inklift-score` stays out of the product.** A Developer tab in Settings, or
  nowhere. It is a benchmark harness, not a feature.
- **Biggest UX risk: full-screen extraction takes about 20 seconds.** "Lift the
  whole screen" is a trap item at that cost — part of why it is not in the
  top-level tray group. The real fix is progressive refinement: proxy first,
  usable result in about a second, refine at full resolution in the background.
  Same machinery the retune preview needs — one implementation, two features.
  Build it before shipping a full-screen item at all.
- **Subtlest implementation hazard:** proxy parameter scaling (`s` for radii,
  `s²` for min-area). Get it wrong and the tuning UI lies convincingly.
- **Found by building the onboarding art, not by reasoning:** the extracted ink
  carries its own colour, which is usually dark, so "drop it anywhere" was false
  as built — on a dark slide it nearly vanished and on a photographic backdrop it
  was entirely invisible. Hence the ink-colour control above. Worth noting that
  the architecture already supported the fix (opacity and colour are stored
  apart); nothing *exposed* it. Recolouring cures the polarity problem but not a
  partial-alpha one: on a maximally busy ground, ink whose opacity peaks near
  0.65 stays faint whatever colour it is, and inflating the alpha to compensate
  would be lying about the measurement.
