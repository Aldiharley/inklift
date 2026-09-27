# inklift — Visual System Spec v1

Design system for a tray-resident desktop app that lifts handwriting off
photographs onto a real alpha channel. Tauri / HTML+CSS, Linux-first.

## 1. Design direction

**The feeling: a conservator's light table.** Cold, precise, optical apparatus —
ground glass, hairline-etched edges, a numeric readout — placed over something
warm, organic and pigmented. The instrument is glass; the specimen is ink. The
two never blend: **glass is only ever chrome, ink is only ever content**, and the
one moment ink touches the chrome (a single serif face, one warm accent) is
deliberate and rationed.

**Avoiding:** the frosted-card-over-purple-gradient dashboard, and iOS cosplay.
No pill tab bars, no 51×31 switches, no SF Pro, no rubber-banding scroll, no
sheet grabbers, no 17px body text. This is a 13px-dense desktop instrument that
happens to be made of glass — not a phone app in a window.

**Principle 0, which everything below obeys: glass is edges, not blur.** A glass
surface reads as glass because of four things — a *gradient* fill, an *outer
hairline*, an *inner highlight*, and a *cast shadow*. Live backdrop blur is the
fifth ingredient and the only optional one. This is not a fallback bolted on; it
is the reason the system survives XFCE.

## 2. Material system

### 2.1 Glass tiers (runtime capability, not a media query)

Tauri probes for a compositor at startup and stamps the root element. CSS never
guesses.

```css
/* html[data-glass="optical"]  compositor + usable backdrop-filter  */
/* html[data-glass="tinted"]   window transparency, no/slow blur    */
/* html[data-glass="opaque"]   no compositor at all                 */
```

| Tier | Window | Blur | Material alpha | Shadow | Corners |
|---|---|---|---|---|---|
| `optical` | transparent | `blur(24px) saturate(150%)` | 0.86 – 0.92 | real, cast on desktop | 14px |
| `tinted` | transparent | none | **0.94 – 0.97** + noise | real | 14px |
| `opaque` | solid | none | 1.0 (pre-composited) | **drawn bevel** replaces it | **0px** |

`data-glass` forces to `opaque` under `prefers-reduced-transparency: reduce` or
`prefers-contrast: more`.

**One blur radius, everywhere: 24px.** Elevation is *never* expressed by
increasing blur — it costs GPU on Linux and looks like a rendering bug. Depth
comes from shadow and edge only.

### 2.2 The edge recipe

Three coincident lines at every glass boundary. This is the whole trick; do not
ship a material missing any of them.

```css
/* light theme */
--edge-hairline:   0 0 0 0.5px rgba(30, 27, 24, 0.16);
--edge-highlight:  inset 0 1px 0 rgba(255, 255, 255, 0.62);
--edge-shade:      inset 0 -1px 0 rgba(30, 27, 24, 0.055);
--edge-rimlight:   inset 0 -0.5px 0 rgba(255, 255, 255, 0.20);

/* dark theme — note the DOUBLE outer hairline */
--edge-hairline:   0 0 0 0.5px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.075);
--edge-highlight:  inset 0 1px 0 rgba(255, 255, 255, 0.11);
--edge-shade:      inset 0 -1px 0 rgba(0, 0, 0, 0.30);
--edge-rimlight:   inset 0 -0.5px 0 rgba(255, 255, 255, 0.055);
```

Dark theme needs the double outer hairline because a single light hairline
disappears against a light desktop and a single dark one disappears against a
dark one. Two stacked 0.5px rings are always visible against anything.

The fill is **never flat** — a sheet catches light across its face:

```css
--glass-sheen-light: linear-gradient(180deg,
  rgba(255,255,255,0.14) 0%, rgba(255,255,255,0.03) 38%, rgba(30,27,24,0.035) 100%);
--glass-sheen-dark: linear-gradient(180deg,
  rgba(255,255,255,0.075) 0%, rgba(255,255,255,0.012) 42%, rgba(0,0,0,0.10) 100%);
```

And a static grain, load-bearing in `tinted`/`opaque` where the fill is nearly
solid: a **baked 128×128 PNG** at `opacity: 0.030`, `mix-blend-mode: overlay`.
Two jobs — kills gradient banding on 8-bit panels, and makes a 0.96-alpha fill
read as *ground glass* rather than beige plastic. Never `feTurbulence` at window
size; that is a frame-rate bug waiting to happen.

### 2.3 Named materials

**`--mat-loupe` — tray popover.** Thinnest, clearest glass. Sits directly on the
desktop with nothing to protect it, so it carries the strongest edge. Alpha 0.88,
radius 14, three-layer shadow.

**`--mat-plate` — window chrome** (titlebar, sidebar, toolbars). Larger area,
therefore heavier and quieter. Alpha **0.92**, sheen range halved, shadow limited
to a single `0 1px 0` seam. No rim light — it is the body of the instrument, not
a floating slab.

**`--mat-slide` — elevated card, menu, dropdown, dialog, toast.** Alpha **0.94**,
radius 12 (14 for dialogs), full edge recipe. Always sits on `--mat-plate` or a
scrim, never straight on the desktop.

**`--mat-wash` — overlay scrim.** Not glass; it is what *makes* glass legible.
`rgba(20,18,16,0.42)` + `blur(8px) brightness(0.86)`. When blur is gone the scrim
gets **denser** (0.58), because blur was doing part of the separation work.

**`--mat-stage` — the result surface. NOT GLASS. Solid, always.** Rule: *any
surface whose job is to tell the truth about pixels is opaque.*

**Optional `--prism`** — a 0.5px chromatic split on the outer hairline, warm top
/ cool bottom, mimicking dispersion through a real edge. **Risk:** at 1× DPI the
0.5px snaps to 1px and reads as a chromatic-aberration bug. Gate behind
`min-resolution: 1.5dppx` and a setting. Never in `opaque`.

## 3. Colour

The palette is a **pigment box**; the form is glass. Every colour is named after
a real pigment. Shape and light come from the instrument, hue comes from the paint.

### 3.1 Neutrals — deliberately not grey, and inverted between themes

Light neutrals are **warm** (cotton rag, paper). Dark neutrals are **cool**
(graphite, glass). The inversion is the point: in light mode you look at paper
through glass; in dark mode you look at glass with ink behind it.

```css
/* Rag — warm, light theme. hue ≈ 40°, chroma ≈ 0.008 */
--rag-00:#FFFDFA; --rag-50:#FAF7F2; --rag-100:#F2EEE7; --rag-200:#E5DFD6;
--rag-300:#D2CABD; --rag-400:#ABA298; --rag-500:#8A8279; --rag-600:#6B645C;
--rag-700:#4E4842; --rag-800:#332F2B; --rag-900:#1E1B18;

/* Graphite — cool, dark theme. hue ≈ 215°, chroma ≈ 0.010 */
--gra-900:#0E1113; --gra-800:#14181B; --gra-700:#1B2024; --gra-600:#242A2F;
--gra-500:#333A40; --gra-400:#4C555C; --gra-300:#7A848B; --gra-200:#A6AFB5;
--gra-100:#CED5D9; --gra-50:#E7ECEF;
```

### 3.2 Accent — Prussian (cool ink)

**`--prussian-500: #2B6486`.** Hue 202°, saturation 66%, lightness 35%.

**Why not system blue.** iOS blue (`#007AFF`) is hue 211° at 100% saturation — a
*light-source* blue, a screen colour. Prussian blue is a *pigment*: synthesised
1706, the first modern artificial pigment, the standard wash of architectural
draughtsmen for two centuries, the ink of countless ledgers. It is 9° greener and
34 points less saturated, so it reads as dense and absorptive rather than
emissive — correct for a product whose subject is a substance that sits *on*
something. It is also the one blue the AI-dashboard cohort is not using, because
it is too dark to glow.

```css
--prussian-50:#EEF3F8;  --prussian-100:#D8E4EE; --prussian-200:#B4CADD;
--prussian-300:#79B4DC; --prussian-400:#4D87AD; --prussian-500:#2B6486;
--prussian-600:#1F4E73; --prussian-700:#163C5B; --prussian-800:#102C44; --prussian-900:#0B1E2F;
```

### 3.3 Secondary — Iron Gall (warm ink)

Used **only where actual ink is the subject**: the extracted-ink indicator, the
pipeline rail on completion, the one serif headline, the "lifted" badge. Iron
gall is the historical manuscript ink that oxidises from blue-black to warm
sepia on the page — literally ink aging. Never used for interactive affordances
(warm brown reads as disabled).

```css
--gall-300:#CFAE87; --gall-400:#A87A4A; --gall-500:#8B5E34; --gall-600:#6A4526; --gall-dark:#C99A66;
```

### 3.4 Semantics

| Role | Pigment | Light | Dark | Contrast |
|---|---|---|---|---|
| Success | Verdigris | `#1F7A5C` | `#4FC79E` | 5.25 / 8.75 |
| Warning | Ochre | `#9A6410` | `#E0A94E` | 4.99 / 9.1 |
| Danger | Vermilion | `#B03A2A` | `#F07D6B` | 6.03 / 6.89 |
| Info / focus | Prussian | `#2B6486` | `#79B4DC` | 6.42 / 8.15 |

### 3.5 Where glass is sacrificed for legibility

Contrast on a translucent surface is computed against the **worst-case
composite**: the material's fill over a *pure white* desktop (dark theme) or
*pure black* (light theme).

- Dark popover floor = `#14181B @ 0.88` over white = `#303436`.
  `gra-50` 5.86:1 ✅ · `gra-200` 5.65:1 ✅ · `gra-300` **3.30:1 ❌**
- Light popover floor = `#FFFDFA @ 0.88` over black = `#E0DEDC`.
  `rag-900` 12.83:1 ✅ · `rag-700` 6.74:1 ✅ · `rag-600` **4.36:1 ❌**

Six hard rules follow:

1. **On glass, secondary text is exactly ONE ramp step below primary, never
   two.** The genuinely muted values exist **only on solid surfaces**. This is
   the largest visual concession in the system: popover hierarchy is carried by
   weight, size and case rather than a big lightness gap.
2. **No text below weight 450 on glass, ever.** 400-weight stems shimmer against
   a busy backdrop.
3. **Minimum material alpha for any surface bearing body text is 0.86, and 0.94
   without blur.** This is why `--mat-loupe` is 0.88 and not the prettier 0.72.
4. **Every dialog gets `--mat-wash`.** Legibility must never depend on the
   desktop behind it.
5. **Disabled on glass is `opacity: 0.45`**, and the *icon* dims further than the
   *label*. A disabled item you cannot read is a bug report, not a state.
6. **`prefers-contrast: more`** → `opaque` tier, hairlines to 0.30/0.26, focus
   ring 3px, sheen flattened, grain to 0.

## 4. Typography

Three faces, three jobs, all Google Fonts. No Inter, no Space Grotesk.

**`--font-ui`: Archivo** (variable 400–700). A *grotesque drawn for labels* —
high x-height, tight apertures, slightly narrow, flat engineered terminals of
equipment silkscreen and transit signage. Reads as apparatus, not startup. Holds
shape at 11–13px on a translucent surface where a softer humanist face goes
mushy.

**`--font-readout`: IBM Plex Mono** (400, 500). Every number the app shows.
True tabular alignment, and a drafting-instrument warmth that bridges toward ink
without going soft. A distinct mono voice makes a number instantly identifiable
as a *measurement* rather than a label.

**`--font-display`: Fraunces** (variable; `opsz` auto, `SOFT 40`, `WONK 0`).
**Five places only**: window empty state, onboarding headline, About, the
one-line result summary, error titles. Its `SOFT` axis literally controls
terminal softness — ink bleed as a type axis. The single point where pigment
intrudes into the glass instrument, always in `--gall-500`. **Risk:** small or
heavy it turns precious. Hard cap ≥ 24px, weight ≤ 500, `WONK 0`.

### 4.1 Scale — base 13px

Desktop-dense, matching GTK's 10pt default, not iOS's 17px.

| Token | Face | Size / Leading | Weight | Tracking | Use |
|---|---|---|---|---|---|
| `micro` | ui | 10.5 / 14 | 600 | **+0.07em**, uppercase | section headers |
| `caption` | ui | 11.5 / 16 | 500 | +0.005em | helper text |
| `label` | ui | **13 / 18** | 450 | −0.003em | default UI text, menu rows |
| `label-strong` | ui | 13 / 18 | 600 | −0.006em | buttons, toast titles |
| `body` | ui | 14 / 21 | 400 | −0.002em | settings prose — **solid only** |
| `title` | ui | 16 / 22 | 600 | −0.010em | panel titles |
| `heading` | ui | 20 / 26 | 600 | −0.014em | window section heads |
| `display` | display | 28 / 32 | 450 | −0.018em | empty state, onboarding |
| `readout-sm` | readout | 11 / 14 | 450 | +0.010em | ticks, dimensions |
| `readout` | readout | 13 / 16 | 500 | +0.005em | slider chip, elapsed |
| `readout-lg` | readout | 22 / 24 | 450 | −0.008em | coverage percentage |

**Tracking is size-specific, never one global value.** Mono never gets negative
tracking — tabular figures need the air. Leading tightens as size grows. **On
glass, add +0.004em and nudge weight one step** (450→500): vibrancy costs
apparent stroke contrast, letterspacing buys it back.

## 5. The alpha motif — beating the checkerboard

The Photoshop checker is a 30-year-old engineering artifact: two greys at ~1.35:1
in 8px squares, loud enough to fight the artwork and make soft ink edges
unjudgeable. Exactly the wrong tool for a product whose value is *soft* edges.
Four mechanisms replace it, in ascending order of how expensive they feel.

### 5.1 The Void ground — texture, not pattern

A near-flat field with a directional lattice whose luminance delta is **≤ 3%**.

```css
.ground-void {
  background-color: #EDEAE4;                      /* light; dark: #171B1E */
  background-image:
    repeating-linear-gradient(45deg,  rgba(30,27,24,0.026) 0 1px, transparent 1px 12px),
    repeating-linear-gradient(-45deg, rgba(255,255,255,0.40) 0 1px, transparent 1px 12px),
    radial-gradient(120% 100% at 50% 0%, rgba(255,255,255,0.30), transparent 60%);
  background-size: 17px 17px, 17px 17px, 100% 100%;
  box-shadow: inset 0 1px 3px rgba(30,27,24,0.13), inset 0 0 0 1px var(--hairline-soft);
}
```

Two crossed hairline grids — one dark, one light, at ±45° — produce a *woven*
surface reading as warp and weft: cloth, or the ground glass of a view camera.
**17px is prime** against typical image scale factors, so it never beats against
the ink's own pixel grid. The inner shadow makes the void **recessed** — a hole
in the app, not a texture painted on it.

### 5.2 The contact shadow — the cutout tell

The real proof of alpha is not the background; it is that **the ink casts a
shadow onto whatever is behind it.**

```css
.ink-layer { filter: drop-shadow(0 1px 1.5px rgba(30,27,24,0.28))
                     drop-shadow(0 4px 10px rgba(30,27,24,0.14)); }
```

`drop-shadow` follows the alpha channel, so the shadow traces every stroke's
actual silhouette, per-pixel, including feathered edges. The single most
convincing two lines of CSS in the product: the handwriting becomes a *sheet of
pigment hovering a millimetre above the ground*. On White ground the shadow
reduces to the first layer only — real ink on real paper barely casts.

### 5.3 Peek — hold to verify

Press-and-hold anywhere on the Stage and the ground cycles **Void → White →
Black → Ink-blue** at 90ms per step while held, snapping back on release. This is
literally how professionals verify an alpha channel, and making it a gesture
instead of four clicks is the moment the app feels like a tool built by someone
who knows what they are doing. Keyboard: hold `Space`. Dragging the ink layer
also nudges it with 2px of independent travel against the ground — motion proves
separation more cheaply than any pattern can.

### 5.4 True Void (tier `optical` only, opt-in)

The Stage becomes an actual hole: the window is transparent in that rectangle and
the extracted ink composites over **the user's real desktop**. **Risk:** needs a
compositor, per-region window transparency, and correct hit-testing; impossible
on XFCE without compositing, and click-through misbehaves on some compositors.
Settings toggle, default **off**, auto-hidden outside `optical`. §5.1–5.3 must
stand alone — True Void is a party trick on top, not the plan.

### 5.5 The motif elsewhere

**Output-format picker.** Four rows, each with a 22×22 tile that is **not a
generic swatch — it is a live 22px crop of the user's actual extracted ink**,
rendered as that format would render it, with the projected byte size in
`readout-sm`. The picker shows *consequences*, not labels.

**Segmented control.** Each of Transparent / White / Both carries a 12px ground
swatch: Void weave, flat white with a hairline ring, hard 50/50 split. The
control demonstrates its own options.

**Coverage readout.** `ink 12.4% · soft edge 2.1px · α 8-bit · 1712 × 984`.
Measurement as luxury. Numbers snap — they never count up.

## 6. Component specs

### 6.1 Tray popover

- **Width 340px**, max height `min(560px, 70vh)` then scrolls with a 24px fade
  mask at overflowing edges only.
- `--mat-loupe`, radius 14, padding 6px, 8px from the tray edge,
  `transform-origin` at the tray anchor.
- **Item row:** height **30px**, radius 8, grid `16px icon / 10px / label 1fr /
  10px / shortcut auto`. Shortcut in `readout-sm` at 0.75 opacity.
- **Engraved separator** — what keeps it from looking like a web dropdown:
  ```css
  .sep { height:1px; margin:5px 8px; background:var(--hairline-soft);
         box-shadow: 0 1px 0 var(--hairline-carve); }
  ```
  1px dark + 1px light below = a rule *carved into* the glass.
- **Hover:** tinted glass inlay, not a grey fill —
  `color-mix(in oklab, var(--accent) 10%, transparent)` plus an inner highlight.
- **Active:** tint to 18% and **move the inner highlight to the bottom edge** —
  flipping the light source is how a physical surface reads as depressed. 90ms,
  fires on `pointerdown`.
- **Keyboard focus — double ring, mandatory:** 2px accent + `0 0 0 4px
  rgba(0,0,0,0.30)`. A single-colour ring on translucency over an unpredictable
  desktop *will* vanish against some wallpaper.
- **Hit target: 30px rows, 24×24 minimum for icon-only.** WCAG 2.2 SC 2.5.8 (AA)
  requires 24×24; the 44px figure is touch/AAA and would make a desktop tray menu
  absurd. Stated so nobody "fixes" it later.

### 6.2 Buttons

Heights 28 / 32 / 38, radii 8 / 9 / 10, label `label-strong`, min-width 76px.

**Primary — solid, not glass.** *Rule: anything you press with consequence is
opaque.* Confidence is the affordance; a translucent commit button is a design
that does not believe in itself. Vertical gradient `#34719A → #2B6486 → #24566F`,
inner highlight + inner shade, hairline, drop shadow. White on `#2B6486` = 6.42:1.

**Secondary** — glass (`--mat-slide` at 0.94). **Ghost** — transparent until
hover. **Destructive** — primary construction in Vermilion. **Icon button** —
28×28, radius 8, ghost.

### 6.3 Segmented control

Container height 30, radius 9, padding 2, **recessed well** (inverted bevel).
Thumb: solid raised slab, radius 7. Recessed container + raised thumb = one
glance tells you which is which. Thumb travel spring, **damping 1.0 / response
0.24s, no overshoot** — a discrete selection, not a thrown object. Focus ring on
the **container**, never the thumb.

### 6.4 Slider with live readout

Track 4px, recessed well. Thumb 16px disc; hover `scale(1.08)`, press
`scale(0.96)` + highlight flip. Ticks at 0/50/100, **no tick numbers** — the
readout is the number. **Readout chip** right: Plex Mono 13/500 in a recessed
well, `min-width: 5ch`, tabular. Dragging turns it `--accent-text`.
**Double-click makes it an editable input.** 1:1 tracking via Pointer Events +
`setPointerCapture`, respecting grab offset; updates every `pointermove`, never
animate the thumb toward the pointer.

### 6.5 Toggle

**34 × 20**, deliberately *not* iOS's 51×31 — that proportion is unmistakably a
phone control and would undo everything else. The only pill in the system.

### 6.6 Result preview surface — the Stage

**Solid, always.** `--bg-content`, radius 12, inner shadow + hairline, padding
20px. **Ink is never displayed on glass** — putting the product's output on a
translucent surface would be lying about its alpha, and that is the one lie this
app cannot tell. Text never overlaps ink: overlays sit in the 20px gutter or in a
`--mat-slide` chip ≥ 12px from the ink bounds.

### 6.7 Toast

340 × auto, `--mat-slide`, anchored bottom-right, stacks upward, max 3.
**Dismiss timer is a 2px hairline along the bottom edge** shrinking
right-to-left over 4.5s — not a spinner, not a ring. Pauses on hover, visibly.
Enter 160ms with `blur 0→24px` in `optical` — the material *arrives* rather than
fades in.

### 6.8 Pipeline progress

Stages: `Detect → Separate → Refine edges → Compose alpha → Write`.

**Stage rail:** 3px tall, N segments with 2px gaps. Complete = solid accent.
Pending = accent at 0.12. **Current = an indeterminate shimmer confined to its
own segment** — 1100ms sweep, so you always know *which* stage without reading.
If a stage exceeds 2.5s and reports real progress, its segment goes determinate.
**Never show a percentage that is not real.**

**On completion, all segments transition from `--accent` (Prussian, cool) to
`--gall-500` (Iron Gall, warm) over 400ms, hold 500ms, then fade.** Cool while it
is an instrument working; warm the instant it is ink. The only place in the
system where the accent changes temperature, and the whole thesis paying off in
900ms.

## 7. Motion

```css
--dur-press:90ms;   --dur-fade:140ms;  --dur-state:180ms;
--dur-enter:240ms;  --dur-window:320ms; --dur-settle:420ms; /* hard ceiling */
--ease-out:      cubic-bezier(0.2, 0.8, 0.2, 1);
--ease-in:       cubic-bezier(0.4, 0, 1, 1);
--ease-standard: cubic-bezier(0.32, 0.72, 0, 1);
```

Springs for **anything a pointer can grab**. House default damping 1.0 / response
0.30–0.40s, no overshoot. Bounce (damping 0.8) *only* when the gesture carried
momentum. Always animate from the **live presentation value**, never the target.

**Three moments worth animating:**

1. **Popover open — 240ms.** `scale 0.96→1` + opacity, origin at the tray anchor,
   and in `optical` `blur(0)→blur(24px)` on the same curve. The glass *sets*.
2. **The extraction reveal — 380ms.** The one animation allowed to be beautiful,
   because it *is* the product. Source cross-dissolves to extracted ink while the
   ground cross-fades to Void, and the contact shadow grows from 0 over 220ms
   **delayed 100ms**. The delay is the whole point: for a beat the ink is flat on
   the paper, then it *lifts off*.
3. **Pipeline completion cool→warm.**

**What NOT to animate.** Menu rows beyond a 120ms fill fade. Panel switches
(instant swap + 100ms opacity; a 300ms slide between settings tabs is theatre).
**Numbers — readouts snap, always; a number that eases toward its value is lying
about the measurement.** Any looping ambient motion, animated gradient, drifting
background, blur-on-scroll, parallax, skeleton shimmer under 400ms, elevation
change on hover.

**Reduced motion** removes transforms, keeps opacity (fades aid orientation and
are not vestibular), and keeps press states, hover fills and focus rings —
because they are the only thing telling the user the app heard them.

## 8. Tray icon

**The mark is a stroke that has left the page.** A baseline — the paper — and a
single pressure-tapered stroke rising off it, **with a visible gap between the
stroke's foot and the baseline.** The gap is the entire idea: ink no longer
touching the surface it was written on. That gap is the alpha channel, drawn. It
also spells the name: *ink, lifted.* Not a pen — a pen is an input device, and
this product is about output.

**16px monochrome, 1px grid:**
- Baseline: rect `x 2→14`, `y 13→15`, flat ends.
- Stroke: tapered quadratic from `(4,10)` to `(13,3)`, **3px at the foot tapering
  to 1.5px** at the tip, slight leftward bow — a real upstroke.
- Gap between stroke foot and baseline: **3px.** Never below 2px or it closes
  into a checkmark at low DPI.
- Two solid filled shapes, one colour. Works in macOS template mode, GNOME
  symbolic, and XFCE's 1-bit-ish rendering unchanged.

**22px:** same mark, plus the baseline's right end lifting into a **shallow 2px
fold** — the page corner peeling where the ink left.

**States**, all monochrome-safe: **Idle** the mark · **Working** the stroke
becomes three 2px dashes cycling upward, 3 frames / 900ms (reads at 16px where a
rotating arc smears) · **Done** a 2px dot at the tip for 2s · **Error** the
baseline breaks into two pieces with a 2px gap. The page tore — legible at 16px
in one bit.

## 9. Elevation and layering

| L | Layer | Material |
|---|---|---|
| 0 | Desktop | not ours |
| 1 | Window chrome | `--mat-plate` (glass) |
| 2 | **Content** — Stage, tables, lists | **SOLID** |
| 3 | Inline card over L2 | `--mat-slide` |
| 4 | Popover, menu, tooltip | `--mat-slide` · `--mat-loupe` over desktop |
| 5 | Dialog | `--mat-slide` + `--mat-wash` |
| 6 | Toast | `--mat-slide` |
| 7 | Drag ghost | `--mat-slide` at 0.90, `scale(1.02)` |

### The seven anti-mush rules

1. **Content is always solid.** Data does not float.
2. **Never stack two glass materials directly.** One glass-over-glass transition
   is the maximum anywhere; two is banned.
3. **Every glass layer needs an opaque seat** — a scrim, a solid parent, or a
   shadow dark enough to separate it. Glass with no seat is a translucent
   rectangle.
4. **Alpha budget: at most ONE layer of transparency between the desktop and any
   text.** Two layers deep, everything is ≥ 0.96.
5. **Blur radius does not scale with elevation.** Depth = shadow spread + edge
   strength.
6. **12px minimum from text to any glass edge.** The inner highlight competes
   with glyph stems inside that margin.
7. **Icons on glass get `drop-shadow(0 0.5px 0 rgba(30,27,24,0.10))`** in light
   theme only — without it, monochrome glyphs detach and float.

**Shadows are context-aware:** heavier over the Stage or dense content, lighter
over flat chrome. Separation should cost only what the background demands.

## Risk register

| Risk | Mitigation |
|---|---|
| `backdrop-filter` absent or ugly on XFCE | Principle 0: glass is edges. Three tiers, blur optional, alpha floors raised without it. |
| No compositor → no shadow, no rounded corners | `opaque` tier: square corners, pre-composited fills, drawn bevel. Reads native, not broken. |
| Secondary text dies over a white desktop in dark theme | One-step secondary rule, measured at worst-case composite. Costs hierarchy depth; non-negotiable. |
| Focus ring vanishes on some wallpaper | Double ring, accent + dark halo. |
| True Void needs per-region window transparency | Opt-in, default off, auto-hidden outside `optical`. |
| `--prism` reads as a rendering bug at 1× | Gated behind `min-resolution: 1.5dppx` + a setting. |
| Fraunces goes precious | ≥24px, weight ≤500, `WONK 0`, five locations only. |
| Grain via live SVG filter tanks frame rate | Baked 128px PNG only. |
| Void lattice moirés against image pixels | 17px prime pitch, ≤3% luminance delta. |
| Three webfonts × variable axes = slow cold start | Subset to latin + `wght`, ship WOFF2 in the bundle, no network fetch. |
