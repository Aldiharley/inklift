//! Drive a real drag against the live selector.
//!
//! The previous selection UI passed every file-reading test in the repository
//! while being a solid black rectangle on screen. The lesson is that this path
//! is only proven by actually operating it, so this test grabs no shortcuts: it
//! runs the real selector, moves the real pointer with XTEST, and checks the
//! rectangle that comes back.
//!
//! Safe to run on a live desktop: while the selector holds the pointer grab
//! (owner_events = false) the synthetic clicks are delivered only to it, never
//! to whatever happens to be under the cursor.

use std::sync::{Mutex, mpsc};
use std::time::Duration;

use inklift_shot::{Outcome, Rect, pick_live_region_ready};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest::ConnectionExt as _;

fn skip() -> bool {
    if std::env::var_os("DISPLAY").is_none() {
        eprintln!("no DISPLAY; skipping");
        return true;
    }
    false
}

fn motion<C: Connection>(conn: &C, root: u32, x: i16, y: i16) {
    let _ = conn.xtest_fake_input(MOTION_NOTIFY_EVENT, 0, 0, root, x, y, 0);
    let _ = conn.flush();
    std::thread::sleep(Duration::from_millis(12));
}

fn button<C: Connection>(conn: &C, root: u32, press: bool, x: i16, y: i16) {
    let kind = if press { BUTTON_PRESS_EVENT } else { BUTTON_RELEASE_EVENT };
    let _ = conn.xtest_fake_input(kind, 1, 0, root, x, y, 0);
    let _ = conn.flush();
    std::thread::sleep(Duration::from_millis(30));
}

/// The pointer is a single global resource: two selectors cannot hold it at
/// once, and cargo runs tests in a binary concurrently. Without this lock the
/// readiness check above sees *another* test's grab and drives the wrong
/// selector — which is exactly how these three first failed.
static POINTER: Mutex<()> = Mutex::new(());

/// Run the real selector, drive the real pointer through `steps`, return what
/// it decided. `steps` are (x, y, button_action) where the action is applied
/// after moving.
fn drag(steps: &[(i16, i16, Option<bool>)]) -> Option<Outcome> {
    let _guard = POINTER.lock().unwrap_or_else(|e| e.into_inner());
    if skip() {
        return None;
    }
    let bounds = Rect::new(0, 0, 2560, 1080);
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(pick_live_region_ready(bounds, 8, move || {
            let _ = ready_tx.send(());
        }));
    });

    // Wait for the selector to say it is armed. Probing for the grab from out
    // here would mean taking the pointer ourselves, which blocks the grab we
    // are waiting for — that race is exactly how these tests first flaked.
    if ready_rx.recv_timeout(Duration::from_secs(10)).is_err() {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Err(e)) if e.contains("holding the pointer") => {
                eprintln!("something else holds the pointer; skipping: {e}");
                return None;
            }
            other => panic!("the selector never armed: {other:?}"),
        }
    }

    let (conn, num) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[num].root;

    for &(x, y, press) in steps {
        motion(&conn, root, x, y);
        if let Some(down) = press {
            button(&conn, root, down, x, y);
        }
    }

    Some(
        rx.recv_timeout(Duration::from_secs(20))
            .expect("the selector never returned")
            .expect("the selector failed"),
    )
}

#[test]
fn a_dragged_rectangle_comes_back_as_the_selection() {
    let Some(outcome) = drag(&[
        (500, 300, Some(true)),
        (650, 400, None),
        (800, 500, Some(false)),
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Selected(Rect::new(500, 300, 300, 200)));
}

#[test]
fn a_backwards_drag_gives_the_same_rectangle() {
    // bottom-right to top-left: users do this constantly
    let Some(outcome) = drag(&[
        (800, 500, Some(true)),
        (600, 380, None),
        (500, 300, Some(false)),
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Selected(Rect::new(500, 300, 300, 200)));
}

#[test]
fn a_click_without_a_drag_is_a_misclick_not_a_capture() {
    let Some(outcome) = drag(&[(700, 400, Some(true)), (700, 400, Some(false))]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Cancelled);
}

/// The selector must not photograph its own outline.
///
/// The previous design shipped exactly this bug: the overlay's border was
/// measured *inside* a capture, 660 pixels of it. The outline here is drawn
/// strictly outside the selection, which should make it impossible — but the
/// capture is taken moments after the bars are destroyed, and a server that has
/// not finished repainting can still hand back stale pixels. That is an
/// empirical question, so this measures it rather than reasoning about it.
#[test]
fn the_capture_contains_no_pixel_of_the_selectors_own_outline() {
    use inklift_shot::{Capturer, X11Capturer};

    // The accent the bars are painted with.
    const ACCENT: [u8; 3] = [0x4D, 0x8F, 0xBF];

    let sel = Rect::new(500, 300, 300, 200);
    let Some(outcome) = drag(&[
        (sel.x as i16, sel.y as i16, Some(true)),
        (650, 400, None),
        (sel.right() as i16, sel.bottom() as i16, Some(false)),
    ]) else {
        return;
    };
    assert_eq!(outcome, Outcome::Selected(sel));

    let Ok(cap) = X11Capturer::new() else { return };
    let frame = cap.grab(&sel).expect("capture the region just selected");
    let rgba = frame.to_rgba8();
    let w = frame.width() as usize;
    let h = frame.height() as usize;
    let at = |x: usize, y: usize| {
        let i = (y * w + x) * 4;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    };

    // A bar that leaked inward paints essentially a whole edge. Anything under
    // half an edge is the desktop happening to contain that blue.
    let mut worst = (0.0f64, "");
    for (name, pts) in [
        ("top", (0..w).map(|x| (x, 0)).collect::<Vec<_>>()),
        ("bottom", (0..w).map(|x| (x, h - 1)).collect()),
        ("left", (0..h).map(|y| (0, y)).collect()),
        ("right", (0..h).map(|y| (w - 1, y)).collect()),
    ] {
        let hits = pts.iter().filter(|&&(x, y)| at(x, y) == ACCENT).count();
        let pct = hits as f64 * 100.0 / pts.len() as f64;
        if pct > worst.0 {
            worst = (pct, name);
        }
    }
    assert!(
        worst.0 < 50.0,
        "the {} edge of the capture is {:.1}% outline colour — the selector \
         photographed its own border",
        worst.1, worst.0
    );
    eprintln!("outline residue: worst edge {:.1}% ({})", worst.0, worst.1);
}
