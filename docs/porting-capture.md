# Porting screen capture to Windows and macOS

**Read this whole document before writing code.** It is written for someone
joining the project cold. Everything you need to know is here or linked from
here.

> **Status: Windows is done; macOS (Task C) remains.** The Windows port is
> described in [`live-selection.md`](live-selection.md#on-windows). It moved the
> platform choice out of the GUI and into `inklift-shot/src/lib.rs`, so §8 is
> now simpler than written below: `main.rs` and the CLI use `NativeCapturer`
> and `pick_live_region` and name no platform, and a platform without a
> backend gets `src/unsupported.rs`, whose refusal explains itself. `outline_bars`
> now lives in `src/outline.rs`, shared by every selector.
>
> For macOS, the wiring is: add `mac.rs`, export it from `lib.rs` as
> `NativeCapturer` plus `pick_live_region{,_ready}` the way `win.rs` and
> `win_live.rs` are, add `target_os = "macos"` to `CAPTURE_SUPPORTED`, and delete
> `unsupported.rs` — nothing in the GUI changes. `tests/win_pick.rs` shows how
> to drive the selector safely on a live desktop: a sink window under the drag,
> no press unless the selector's own window is under the cursor, and a mid-drag
> photograph proving the outline is up before claiming it is gone.

---

## 1. What this project is

**inklift** extracts handwriting from a photo or screenshot and puts it on a
transparent background — real soft alpha, not a 1-bit mask. It is a Rust
workspace with a CLI and a Tauri 2 desktop app.

Repository: <https://github.com/Aldiharley/inklift>

```
crates/
  inklift-core    the whole extraction algorithm. ZERO dependencies, pure std.
  inklift-cli     image decoding, the `inklift` command, the DIBCO scorer.
  inklift-shot    screen capture, region selection, clipboard.   <- your work
  inklift-api     optional hosted-model path (off by default).
  inklift-gui     the Tauri desktop app.                         <- your work
```

Build and test:

```bash
cargo test --workspace                              # 200+ tests
cargo build --release -p inklift-gui                # the desktop app
cd crates/inklift-gui && cargo tauri build          # installers
```

The desktop app needs the [Tauri v2
prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

## 2. The problem you are solving

Screen capture and interactive region selection are **X11 only**. On Windows
and macOS the app builds, installs, runs, and extracts from files perfectly —
but "Lift from screen" is greyed out and reports:

> Lifting from the screen needs X11, and this build has no capture backend for
> your platform yet.

**Your job: make "Lift from screen" work natively on Windows and macOS.**

Everything above the platform layer already exists and is tested. You are
implementing two traits' worth of behaviour, not designing a feature.

## 3. The contracts you implement against

Do not change these. They are used by the CLI and the GUI and are covered by
tests that must keep passing.

### `Rect` — `crates/inklift-shot/src/geometry.rs`

```rust
pub struct Rect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
```

**`x` and `y` are global desktop coordinates and may be negative** — a monitor
placed left of or above the primary one starts negative. Width and height are
always positive. Several tests pin this; do not assume an origin at (0, 0).

### `Monitor` and `Capturer` — `crates/inklift-shot/src/capture.rs`

```rust
pub struct Monitor {
    pub name: String,
    pub bounds: Rect,     // global coordinates, may start negative
    pub primary: bool,
}

pub trait Capturer {
    fn monitors(&self) -> Result<Vec<Monitor>, String>;
    /// Grab a region given in global coordinates.
    fn grab(&self, region: &Rect) -> Result<Frame, String>;
}
```

Errors are `String`, and they are shown to the user. Write them as sentences a
person can act on, not as API codes.

### `Frame` — `crates/inklift-shot/src/frame.rs`

Pixels are **BGRX, 4 bytes per pixel**, optionally with row padding:

```rust
Frame::from_bgrx(width, height, data)              // tightly packed
Frame::with_stride(width, height, stride, data)    // rows every `stride` bytes
```

Both validate their inputs and return `Result<Frame, String>`. Windows DIBs and
macOS `CGImage` both hand you a row-stride, so `with_stride` is usually what you
want — pass the stride rather than repacking.

### `SelectionState` and `Outcome` — `crates/inklift-shot/src/selection.rs`

**Every rule about what a drag means already exists here and is unit-tested
without a display. Do not reimplement any of it.**

```rust
pub enum Outcome { Pending, Selected(Rect), Cancelled }

impl SelectionState {
    pub fn new(bounds: Rect, min_width: u32, min_height: u32) -> Self;
    pub fn press(&mut self, x: i32, y: i32);
    pub fn drag(&mut self, x: i32, y: i32);
    pub fn current(&self) -> Option<Rect>;   // what to draw right now
    pub fn release(&mut self) -> Outcome;
    pub fn cancel(&mut self);
}
```

It already handles: drags in any of the four directions giving the same
rectangle; trimming a drag that runs off the screen; treating anything under
8×8 as a misclick rather than a capture; ignoring events after the selection
settles; and negative origins.

Your platform selector's only job is to translate native input events into
`press` / `drag` / `release` / `cancel`, and to draw what `current()` returns.
A second implementation of these rules would be a second version of the truth.

## 4. What exists today

### `crates/inklift-shot/src/lib.rs`

```rust
#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
pub use x11::X11Capturer;
```

`mod live;` (the X11 region selector) is **not** currently gated. It compiles
everywhere because `x11rb` is pure Rust, but it can only work under X11.

### `crates/inklift-shot/src/live.rs` — the Linux selector, as a model

Read it. It is short and it documents *why* it is shaped the way it is. The
design you should mirror in spirit:

- The user drags on their **real screen**. Nothing paints a copy of the desktop.
- The only thing drawn is a thin outline — four 2px override-redirect windows —
  positioned **strictly outside** the selection.
- Everything is torn down before the capture is taken.

Public API to match:

```rust
pub fn pick_live_region(bounds: Rect, min: u32) -> Result<Outcome, String>;
pub fn pick_live_region_ready<F: FnOnce()>(bounds: Rect, min: u32, on_ready: F)
    -> Result<Outcome, String>;
pub fn outline_bars(sel: &Rect, thickness: u32) -> [Rect; 4];
```

`pick_live_region_ready` calls `on_ready` the instant the selector is armed.
That exists because there is no way to observe "the grab is live" from outside,
and the tests drive the selector with synthetic input — keep it.

### `crates/inklift-gui/src/main.rs`

Four stubs are gated behind `#[cfg(not(target_os = "linux"))]` and return a
`NO_CAPTURE` message: `capture`, `screens`, `run_pick`, and the constant itself.
`capture_supported()` returns `cfg!(target_os = "linux")` and the frontend uses
it to grey out the button.

**When your backend works, these gates and that constant come out.** Leaving a
stub behind a working implementation is how a feature silently stays broken.

## 5. Task A — Windows capture

Implement `WindowsCapturer` in a new `crates/inklift-shot/src/win.rs`, gated
`#[cfg(target_os = "windows")]`, exported from `lib.rs` the same way
`X11Capturer` is.

**Suggested approach:** GDI. `GetDC(NULL)` for the virtual screen,
`CreateCompatibleDC` / `CreateCompatibleBitmap`, `BitBlt` with `SRCCOPY`, then
`GetDIBits` with a `BITMAPINFOHEADER` whose `biHeight` is **negative** to get
top-down rows (otherwise you get a bottom-up DIB and the image is flipped).
`biBitCount = 32` gives you BGRX directly, which is exactly what `Frame` wants.

Monitors: `EnumDisplayMonitors` + `GetMonitorInfoW`. `MONITORINFOEXW.rcMonitor`
is already in global virtual-desktop coordinates, including negative ones.
`dwFlags & MONITORINFOF_PRIMARY` gives you `primary`.

**The trap that will get you: DPI.** Without per-monitor DPI awareness Windows
lies to you about coordinates and sizes, and captures come back the wrong size
or offset on any scaled display. Tauri sets DPI awareness via its manifest —
verify what it actually is at runtime rather than assuming, and make
`monitors()` report *physical* pixels consistent with what `grab()` returns.
A mixed-DPI multi-monitor setup is the case that exposes this; test it if you
have one, and say plainly in your report if you could not.

Dependencies: the official `windows` crate, with only the feature groups you
need (`Win32_Graphics_Gdi`, `Win32_UI_WindowsAndMessaging`,
`Win32_Foundation`, …). **Add it as a target-specific dependency** so Linux and
macOS builds do not pull it:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = ["Win32_Graphics_Gdi", ...] }
```

This project keeps dependencies deliberately small — `inklift-core` has none at
all. Justify anything beyond `windows` in your report.

## 6. Task B — Windows region selection

Implement the same public API as `live.rs`, in `win.rs` or a sibling module,
gated to Windows.

**Suggested approach:** a full-screen layered window (`WS_EX_LAYERED |
WS_EX_TOPMOST | WS_EX_TOOLWINDOW`, `WS_POPUP`) spanning the virtual desktop,
with `SetLayeredWindowAttributes` using a colour key so the window is
click-through-transparent except for the outline you draw. Capture the mouse
with `SetCapture`, feed `WM_LBUTTONDOWN` / `WM_MOUSEMOVE` / `WM_LBUTTONUP` into
`SelectionState`, and cancel on `WM_RBUTTONDOWN` or `VK_ESCAPE`.

Alternatively draw four thin borderless windows as the outline, exactly as the
X11 version does. That avoids transparency entirely and is the more robust
option if the layered window gives you trouble.

Two rules that are not negotiable, because this project has already shipped
both bugs and does not intend to again:

1. **The outline is drawn strictly outside the selection.** `outline_bars()`
   already computes this and is unit-tested. The capture is taken from the live
   screen after the outline is destroyed, so an outline one pixel inside gets
   photographed as ink.
2. **Everything is torn down and the screen has repainted before the grab.**
   Destroy the windows, release the capture, then let the compositor settle. On
   X11 this needed 90 ms; measure it on Windows rather than copying that number.

## 7. Task C — macOS

Same two pieces, in `crates/inklift-shot/src/mac.rs`, gated
`#[cfg(target_os = "macos")]`.

**Capture.** `ScreenCaptureKit` is the modern API; `CGDisplayCreateImage` is
simpler but deprecated from macOS 14. Either is acceptable — say which you
chose and why. `CGImage` gives you a stride via `CGImageGetBytesPerRow`; pass it
to `Frame::with_stride`. Check the byte order you actually get and convert to
BGRX if needed — do not assume.

**Permissions — this is the big one.** Screen capture requires the user to
grant Screen Recording in System Settings → Privacy & Security. A denied or
not-yet-granted permission must produce a clear, actionable error, not an empty
frame or a silent black image. Detect it (`CGPreflightScreenCaptureAccess`) and
say what the user has to do. A black capture that looks like a bug is worse
than a refusal that explains itself.

The app also needs the right `Info.plist` entries and entitlements; set them in
`crates/inklift-gui/tauri.conf.json` under `bundle.macOS`.

**Selection.** A borderless `NSWindow` at screen level covering the desktop, or
four thin windows as the outline. The same two non-negotiable rules from §6
apply.

## 8. Wiring it up

In `crates/inklift-gui/src/main.rs`:

- Delete the `NO_CAPTURE` constant and the four
  `#[cfg(not(target_os = "linux"))]` stubs.
- Make `capture`, `screens` and `run_pick` select the right capturer per
  platform. Prefer one `fn capturer() -> Result<impl Capturer, String>` behind
  a cfg over scattering cfgs through the command bodies.
- `capture_supported()` returns `true` on every platform you have implemented.
- In `build_tray`, `lift_screen` is currently enabled by
  `cfg!(target_os = "linux")` — widen it.

Leave `run_pick`'s structure alone. Its **hide first, capture last** ordering —
hide the window, wait for it to actually be gone, select, capture, then show the
window again on *every* path out including errors — is load-bearing and was
arrived at the hard way.

## 9. Testing — read this properly

This project is written test-first and the tests are unusually load-bearing.
A previous round shipped a window that rendered perfectly and ignored every
click while the entire suite passed, because every test read files instead of
looking at the screen. The response was tests that photograph the screen and
drive real input. Hold to that standard.

**Existing tests that must stay green** (`cargo test --workspace`):

| File | What it pins |
|---|---|
| `inklift-shot/tests/selection.rs` | the drag state machine, 13 tests |
| `inklift-shot/tests/from_corners.rs` | `resolve_pick` and drag direction |
| `inklift-shot/tests/geometry.rs` | clamping, negative origins, minimum size |
| `inklift-shot/tests/frame.rs` | BGRX handling, stride, cropping |
| `inklift-shot/tests/outline.rs` | the outline never overlaps the selection |
| `inklift-shot/tests/layout.rs` | multi-monitor maths |
| `inklift-gui/tests/*.rs` | the frontend and tray contracts |

`inklift-shot/tests/live_pick.rs` drives the **real** X11 selector with
synthetic pointer events via XTEST and asserts the rectangle that comes back.
It is gated to Linux. **Write the equivalent for your platform** — synthesise
input (`SendInput` on Windows, `CGEvent` on macOS) and assert the outcome.
That test is the only thing that proves the selector actually works.

**New tests you owe:**

1. A drag test as above: forward drag, backwards drag, and a click with no drag
   correctly rejected as a misclick.
2. A capture test: grab a known region and assert the frame's dimensions and
   that it is not uniformly black. `inklift-shot/tests/x11_live.rs` is the
   model; it skips gracefully when there is no display.
3. The outline-residue test: after a drag, capture the selected region and
   assert it contains no pixel of the outline colour. This is the regression
   that matters most — see `the_capture_contains_no_pixel_of_the_selectors_own_outline`
   in `live_pick.rs`.
4. Tests that need a desktop session must **skip, not fail**, when there is
   none, so CI stays usable.

Add your platform to `.github/workflows/ci.yml`. There is already a
`portability` job that cross-checks the GUI compiles for Windows; extend that
thinking rather than replacing it.

## 10. Conventions that matter here

- **Test first.** Write the failing test, watch it fail for the right reason,
  then implement. If you did not see it fail, you do not know it tests anything.
- **No silent failures.** Every error path must reach the user or the log. This
  project has been bitten repeatedly: a webview that never painted, a tray item
  wired to nothing, a CI job that uploaded no artifacts and reported success.
  Anything swallowed into a `console.error` or `/dev/null` is a defect.
- **No dead controls.** Do not add a menu item, button, or option that cannot do
  what it says on the platform it appears on.
- **Comments explain why, not what.** Read a few files before writing; match the
  register. Document the traps you hit, with the measurement that revealed them.
- **Dependencies are deliberate.** `inklift-core` has zero. Justify additions.
- **Do not reformat.** `cargo fmt` would currently rewrite 241 files; that is a
  separate decision the owner has not taken. Keep your diff to your work.
- Clippy is advisory in CI, not enforced. Do not introduce new warnings anyway.

## 11. Done means

- [ ] `cargo test --workspace` passes on your platform **and** still passes on
      Linux (CI proves the second).
- [ ] The new drag test passes, driven by synthetic input.
- [ ] `NO_CAPTURE` and all four `cfg(not(target_os = "linux"))` stubs are gone.
- [ ] `capture_supported()` returns true on your platform and the button is live.
- [ ] The tray's "Lift from screen…" is enabled.
- [ ] You have run the app, dragged a region, and confirmed the extracted result
      is correct — not merely that no error appeared.
- [ ] A capture taken next to the outline contains none of it.
- [ ] `cargo tauri build` produces an installer that runs on a clean machine.
- [ ] `README.md`'s limitations section and `docs/live-selection.md` are updated
      — both currently state that capture is X11-only.

## 12. Report back

State plainly what you verified by running versus what you only compiled, which
API you chose and why, what the DPI or permission behaviour actually was rather
than what the documentation claims, and anything you could not test (a
mixed-DPI setup, an older OS version). An honest gap is useful; a silent one
costs someone a debugging round.
