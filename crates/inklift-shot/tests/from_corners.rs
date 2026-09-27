//! The GUI overlay reports two raw corners and lets Rust do the thinking, so
//! the rules live in one tested place rather than being reimplemented in JS.
use inklift_shot::{Rect, resolve_pick};

const SCREEN: Rect = Rect { x: 0, y: 0, width: 1920, height: 1080 };

#[test]
fn a_forward_drag_becomes_the_obvious_rectangle() {
    assert_eq!(resolve_pick(100, 100, 400, 300, &SCREEN, 8), Some(Rect::new(100, 100, 300, 200)));
}

#[test]
fn every_drag_direction_gives_the_same_rectangle() {
    let want = Some(Rect::new(100, 100, 300, 200));
    assert_eq!(resolve_pick(400, 300, 100, 100, &SCREEN, 8), want, "reversed");
    assert_eq!(resolve_pick(400, 100, 100, 300, &SCREEN, 8), want, "mixed x");
    assert_eq!(resolve_pick(100, 300, 400, 100, &SCREEN, 8), want, "mixed y");
}

#[test]
fn a_misclick_resolves_to_nothing() {
    assert_eq!(resolve_pick(500, 500, 503, 502, &SCREEN, 8), None);
    assert_eq!(resolve_pick(500, 500, 500, 500, &SCREEN, 8), None);
}

#[test]
fn a_sliver_counts_as_a_misclick_too() {
    assert_eq!(resolve_pick(100, 100, 900, 104, &SCREEN, 8), None, "800 wide but 4 tall");
}

#[test]
fn a_drag_past_the_edge_is_trimmed_to_the_screen() {
    assert_eq!(resolve_pick(1800, 1000, 2500, 1500, &SCREEN, 8),
               Some(Rect::new(1800, 1000, 120, 80)));
}

#[test]
fn a_monitor_at_a_negative_origin_works_unchanged() {
    let left = Rect::new(-1920, 0, 1920, 1080);
    assert_eq!(resolve_pick(-1800, 100, -1500, 300, &left, 8),
               Some(Rect::new(-1800, 100, 300, 200)));
}
