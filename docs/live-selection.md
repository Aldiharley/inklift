# Live-screen region selection

## The problem this replaces

Until now, "Lift from screen" grabbed the whole display, then opened a
full-screen WebView showing that frozen screenshot as a base64 PNG, and let the
user drag a box over the picture.

On the target machine that window rendered **solid black**. Not the screenshot,
not the help text, nothing — the page was correct, the IPC worked, the image
decoded at 2560×1080, and WebKitGTK's DMABUF renderer composited none of it.
Measured on the overlay's own region:

| | near-black pixels |
|---|---|
| DMABUF renderer on | 88.8 % |
| DMABUF renderer off | 0.0 % |

The immediate cause is fixed (`WEBKIT_DISABLE_DMABUF_RENDERER`, see `main()`),
but the shape of the design is what made it catastrophic: a full-screen opaque
surface that must be composited correctly or the user loses their entire display
with nothing to act on. The user dragged blind and the capture still succeeded
(`picked 698,188,1019,410`) — the pixels were never the problem, the *painting*
was.

## The decision

**Select on the real screen. Do not paint a copy of it.**

The selector draws nothing but a thin outline: four override-redirect windows,
a few pixels thick, positioned around the selection. Everything the user sees
during selection is their actual desktop, live, because nothing covers it.

This does not merely patch the black-window bug, it removes the conditions for
it. There is no full-screen surface, so there is nothing that can fail to
composite. The worst case for a bar that fails to paint is an invisible
*outline*, not an invisible *screen*.

### Verified on the target before committing to it

A probe against this machine's X server (2560×1080, single output `Virtual1`,
depth 24, no compositor):

```
cursor: created                       (crosshair, glyph 34 of the cursor font)
bar: mapped 300x2 at 400,400          (override-redirect, then repositioned)
grab_pointer:  SUCCESS
grab_keyboard: SUCCESS
bar region: mean 112.4/255, near-white 33.3 %
released and torn down cleanly
```

The bar occupies exactly 2 of the 6 sampled rows — 33.3 % — and those rows are
pure white. The mechanism paints, in the same environment where the WebView did
not.

## Non-goals

- **Wayland.** X11 only, like `X11Capturer` already is. Native Wayland has no
  client-side pointer grab; that path needs the desktop portal and is out of
  scope here.
- **Dimming the unselected area.** The old overlay dimmed the screen to 45 %.
  Dimming requires covering the screen, which is the thing being removed. The
  outline alone shows the selection.
- **Re-deriving any selection rule.** See below.

## Contracts reused verbatim

Nothing about what a drag *means* is rewritten. `inklift-shot` already owns
those rules and they are unit-tested:

| Behaviour | Owner | Tests |
|---|---|---|
| Any drag direction gives the same rect | `Rect::from_corners` | `tests/from_corners.rs` |
| A drag off the edge is trimmed, not moved | `Rect::clamped_to` | `tests/geometry.rs` |
| A sliver is a misclick, not a selection | `SelectionState::release` (min 8×8) | `tests/selection.rs` |
| Events after settling are ignored | `SelectionState` | `tests/selection.rs` |
| Negative origins (monitor left of primary) | `Rect` | `tests/from_corners.rs` |

The live selector feeds pointer events into `SelectionState` and reads
`current()` to place the bars. A second implementation of any of these rules
would be a second version of the truth.

## Architecture

```
begin_pick                      (command; returns immediately)
  └─ worker thread
       hide "main"  ─────────── so the window is not inside the capture
       settle (round-trip + brief pause)
       pick_live_region(bounds, min)   ← blocking: grab, drag, outline, release
       show "main"  ─────────── on every exit path, always
       Selected(rect) → grab(rect) → adopt → emit "picked"
       Cancelled      → log, emit nothing
       Err(e)         → surface to the window; never swallow
```

### Why a worker thread

A `#[tauri::command]` without `async` runs **inline on the GTK main thread**.
A blocking grab-and-drag loop there would freeze the event loop for the whole
drag — no repaint, no IPC, and `hide()`/`show()` would never be processed,
because they dispatch *through* that loop. The grab must be off it.

`GrabMode::ASYNC` is required for the same family of reasons: `SYNC` freezes
event delivery to every client on the display, including this app.

### Inverted invariant

The old design's rule was **capture first, show second** — the overlay could not
photograph itself because the frame predated it. The new rule is the mirror:

> **Hide first, capture last.**

Two obligations follow, and both are load-bearing:

1. **The outline is drawn outside the selection**, offset by the border width,
   so the region handed to the extractor never contains pixels this tool drew.
   The old design shipped that bug once — the overlay's own border was measured
   inside a capture.
2. **Everything is torn down before the grab**: ungrab pointer and keyboard,
   destroy all four bars, then force a round-trip *and* a short settle so the
   server and any compositor have actually repainted. Skipping the settle
   reproduces the same defect by a different route.

### Failure modes

| Failure | Handling |
|---|---|
| A leaked pointer grab | Worst case in the whole design — it makes the desktop unclickable. Ungrab on every exit path including errors and panics; a hard timeout bounds it; process exit releases grabs regardless. |
| Main window left hidden | `show()` on every path. A worker that dies with the window hidden looks like the app vanished. |
| Grab refused (another client holds it) | Report it. Do not fall back to a silent no-op. |
| A bar fails to paint | Selection still works; only the outline is missing. Degraded, not fatal — which is the point of the design. |

## What goes away

`ui/overlay.html`, `ui/overlay.js`, the `Overlay` payload, `overlay_frame`,
`close_overlay`, `finish_pick` and `cancel_pick` as commands, `App.pending`, the
`INKLIFT_DEBUG_AUTOPICK` / `INKLIFT_DEBUG_SMALL_OVERLAY` harness, and
`tests/overlay_paints.rs`.

The command name `begin_pick` and the `picked` event **stay**, so the frontend
and the capability file need no change.

`WEBKIT_DISABLE_DMABUF_RENDERER` **stays** — the main window is a WebKitGTK
surface too, and removing it re-opens a non-painting main window on software GL.
