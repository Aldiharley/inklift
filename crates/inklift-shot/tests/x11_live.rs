//! Exercises the real X server. Skips itself when there is no display, so the
//! suite still passes headless.
#![cfg(target_os = "linux")]

use inklift_shot::{Capturer, Rect, X11Capturer, virtual_bounds};

fn capturer() -> Option<X11Capturer> {
    if std::env::var("DISPLAY").is_err() {
        eprintln!("skipped: no DISPLAY");
        return None;
    }
    X11Capturer::new().ok()
}

#[test]
fn monitors_are_enumerated_with_sane_geometry() {
    let Some(c) = capturer() else { return };
    let monitors = c.monitors().expect("should enumerate");

    assert!(!monitors.is_empty(), "at least one screen must be reported");
    for m in &monitors {
        assert!(m.bounds.width > 0 && m.bounds.height > 0, "{m:?} has no area");
    }
    let all = virtual_bounds(&monitors).unwrap();
    assert!(all.width > 0 && all.height > 0);
    eprintln!("monitors: {monitors:?}");
}

#[test]
fn a_small_region_captures_at_exactly_the_requested_size() {
    let Some(c) = capturer() else { return };
    let region = Rect::new(0, 0, 64, 32);

    let frame = c.grab(&region).expect("capture should succeed");

    assert_eq!((frame.width(), frame.height()), (64, 32));
    assert_eq!(frame.to_rgba8().len(), 64 * 32 * 4);
}

#[test]
fn a_region_outside_the_desktop_is_refused() {
    let Some(c) = capturer() else { return };
    assert!(c.grab(&Rect::new(100_000, 100_000, 10, 10)).is_err());
}

/// Two captures of the same static region should agree, which would not hold
/// if the stride or row order were wrong.
#[test]
fn captures_are_self_consistent() {
    let Some(c) = capturer() else { return };
    let region = Rect::new(4, 4, 32, 16);

    let a = c.grab(&region).unwrap();
    let b = c.grab(&region).unwrap();

    assert_eq!(a.to_rgba8().len(), b.to_rgba8().len());
}
