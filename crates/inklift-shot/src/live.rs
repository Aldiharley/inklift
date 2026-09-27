//! Selecting a region on the live screen.
//!
//! Nothing here paints a copy of the desktop. The user drags over their actual
//! screen and the only thing this module puts on it is a thin outline made of
//! four windows a couple of pixels thick.
//!
//! That is a deliberate reversal. The previous design raised a full-screen
//! window showing a frozen screenshot, which meant the whole display depended on
//! that one surface compositing correctly — and when it did not, the user got a
//! black rectangle over their entire screen with nothing to act on. An outline
//! has no such failure: if a bar does not paint, the selection still works and
//! only the outline is missing.
//!
//! Every rule about what a drag *means* lives in [`SelectionState`], which is
//! tested without a display. This module translates X11 events into that state
//! machine and draws what it reports. It decides nothing itself.

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;

use crate::geometry::Rect;
use crate::selection::{Outcome, SelectionState};

/// Outline colour, matching the app's accent.
const ACCENT: u32 = 0x004D_8FBF;
/// Outline thickness in pixels.
const THICKNESS: u32 = 2;
/// XC_crosshair in the standard cursor font.
const XC_CROSSHAIR: u16 = 34;
/// XK_Escape.
const XK_ESCAPE: u32 = 0xff1b;
/// Nobody drags for two minutes. A bound guarantees the grab is released even
/// if the session is abandoned, because a leaked pointer grab makes the whole
/// desktop unclickable.
const DEADLINE: Duration = Duration::from_secs(120);
/// Time for the server and any compositor to repaint once the bars are gone.
const SETTLE: Duration = Duration::from_millis(90);

/// The four bars that enclose `sel`, all of them strictly outside it.
///
/// Drawn outside on purpose: the capture is taken from the live screen after
/// this outline is torn down, so a bar one pixel inside would be photographed
/// as ink. The old overlay shipped precisely that bug.
pub fn outline_bars(sel: &Rect, thickness: u32) -> [Rect; 4] {
    let t = thickness.max(1);
    let ti = t as i32;
    // A zero-sized window is an X11 error rather than an invisible line.
    let w = sel.width.max(1);
    let h = sel.height.max(1);
    [
        Rect::new(sel.x - ti, sel.y - ti, w + 2 * t, t),
        Rect::new(sel.x - ti, sel.y + h as i32, w + 2 * t, t),
        Rect::new(sel.x - ti, sel.y, t, h),
        Rect::new(sel.x + w as i32, sel.y, t, h),
    ]
}

/// Let the user drag a region on the real screen.
///
/// Returns once they finish, cancel, or the deadline passes. The pointer and
/// keyboard grabs and all four bars are released on every exit path.
pub fn pick_live_region(bounds: Rect, min: u32) -> Result<Outcome, String> {
    pick_live_region_ready(bounds, min, || {})
}

/// As [`pick_live_region`], but calls `on_ready` the moment the selector is
/// armed — pointer grabbed, outline built, waiting for the first press.
///
/// There is no way to observe that moment from outside: checking whether the
/// pointer is grabbed means grabbing it yourself, which blocks the very grab
/// you are waiting for. Anything driving this selector rather than a human
/// needs the signal to come from in here.
pub fn pick_live_region_ready<F: FnOnce()>(
    bounds: Rect,
    min: u32,
    on_ready: F,
) -> Result<Outcome, String> {
    let (conn, screen_num) = x11rb::connect(None).map_err(|e| format!("cannot reach the X server: {e}"))?;
    let root = conn.setup().roots[screen_num].root;

    let cursor = crosshair(&conn).unwrap_or(x11rb::NONE);
    let escape = escape_keycode(&conn);
    let bars = create_bars(&conn, root)?;

    // Run the drag, then tear down unconditionally: an error must not leave the
    // desktop with a live pointer grab on it.
    let result = drag_loop(&conn, root, cursor, escape, &bars, bounds, min, on_ready);
    teardown(&conn, &bars);
    result
}

fn crosshair<C: Connection>(conn: &C) -> Result<u32, String> {
    let font = conn.generate_id().map_err(|e| e.to_string())?;
    conn.open_font(font, b"cursor").map_err(|e| e.to_string())?;
    let cursor = conn.generate_id().map_err(|e| e.to_string())?;
    conn.create_glyph_cursor(
        cursor, font, font, XC_CROSSHAIR, XC_CROSSHAIR + 1,
        0, 0, 0, u16::MAX, u16::MAX, u16::MAX,
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.close_font(font);
    Ok(cursor)
}

/// Escape's keycode on this keyboard. `None` simply means Escape will not
/// cancel; right-click still will.
fn escape_keycode<C: Connection>(conn: &C) -> Option<u8> {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let count = setup.max_keycode.checked_sub(min)?.saturating_add(1);
    let map = conn.get_keyboard_mapping(min, count).ok()?.reply().ok()?;
    let per = map.keysyms_per_keycode as usize;
    if per == 0 {
        return None;
    }
    map.keysyms
        .chunks(per)
        .position(|k| k.contains(&XK_ESCAPE))
        .map(|i| min + i as u8)
}

fn create_bars<C: Connection>(conn: &C, root: u32) -> Result<[u32; 4], String> {
    let mut bars = [0u32; 4];
    for bar in bars.iter_mut() {
        let id = conn.generate_id().map_err(|e| e.to_string())?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT, id, root,
            0, 0, 1, 1, 0,
            WindowClass::INPUT_OUTPUT, x11rb::COPY_FROM_PARENT,
            // override_redirect keeps the window manager from framing, moving
            // or tabbing these; they are decoration, not windows.
            &CreateWindowAux::new()
                .background_pixel(ACCENT)
                .override_redirect(1u32)
                .event_mask(EventMask::NO_EVENT),
        )
        .map_err(|e| format!("could not create the outline: {e}"))?;
        *bar = id;
    }
    Ok(bars)
}

fn show_outline<C: Connection>(conn: &C, bars: &[u32; 4], sel: Option<Rect>) {
    match sel {
        Some(rect) => {
            for (id, bar) in bars.iter().zip(outline_bars(&rect, THICKNESS)) {
                let _ = conn.configure_window(
                    *id,
                    &ConfigureWindowAux::new()
                        .x(bar.x)
                        .y(bar.y)
                        .width(bar.width)
                        .height(bar.height)
                        .stack_mode(StackMode::ABOVE),
                );
                let _ = conn.map_window(*id);
            }
        }
        None => {
            for id in bars {
                let _ = conn.unmap_window(*id);
            }
        }
    }
    let _ = conn.flush();
}

#[allow(clippy::too_many_arguments)]
fn drag_loop<C: Connection, F: FnOnce()>(
    conn: &C,
    root: u32,
    cursor: u32,
    escape: Option<u8>,
    bars: &[u32; 4],
    bounds: Rect,
    min: u32,
    on_ready: F,
) -> Result<Outcome, String> {
    // ASYNC on both: SYNC would freeze event delivery for every client on the
    // display, this process included.
    let grab = conn
        .grab_pointer(
            false, root,
            EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION,
            GrabMode::ASYNC, GrabMode::ASYNC,
            x11rb::NONE, cursor, x11rb::CURRENT_TIME,
        )
        .map_err(|e| format!("could not take the pointer: {e}"))?
        .reply()
        .map_err(|e| format!("could not take the pointer: {e}"))?;
    if grab.status != GrabStatus::SUCCESS {
        return Err(format!(
            "another program is holding the pointer ({:?}); close it and try again",
            grab.status
        ));
    }
    // Best effort: without it Escape will not cancel, which is a smaller
    // problem than refusing to start.
    let _ = conn.grab_keyboard(false, root, x11rb::CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC);

    // Armed: the grab is live and the outline exists.
    on_ready();

    let mut state = SelectionState::new(bounds, min, min);
    let started = Instant::now();

    while started.elapsed() < DEADLINE {
        let event = conn
            .poll_for_event()
            .map_err(|e| format!("lost the connection to the X server: {e}"))?;
        let Some(event) = event else {
            std::thread::sleep(Duration::from_millis(4));
            continue;
        };
        match event {
            Event::ButtonPress(e) => match e.detail {
                1 => state.press(e.root_x as i32, e.root_y as i32),
                // Right-click is the universal "back out of this".
                3 => state.cancel(),
                _ => {}
            },
            Event::MotionNotify(e) => {
                state.drag(e.root_x as i32, e.root_y as i32);
                show_outline(conn, bars, state.current());
            }
            Event::ButtonRelease(e) if e.detail == 1 => {
                state.release();
            }
            Event::KeyPress(e) if Some(e.detail) == escape => state.cancel(),
            _ => {}
        }
        match state.outcome() {
            Outcome::Pending => {}
            settled => return Ok(settled),
        }
    }
    Err("the selection timed out".into())
}

/// Release everything, then wait for the screen to be the screen again.
///
/// The capture happens straight after this returns, so the bars must be gone
/// from the server's point of view — not merely asked to go.
fn teardown<C: Connection>(conn: &C, bars: &[u32; 4]) {
    let _ = conn.ungrab_pointer(x11rb::CURRENT_TIME);
    let _ = conn.ungrab_keyboard(x11rb::CURRENT_TIME);
    for id in bars {
        let _ = conn.unmap_window(*id);
        let _ = conn.destroy_window(*id);
    }
    let _ = conn.sync();
    std::thread::sleep(SETTLE);
}
