use inklift_shot::{Outcome, Rect, SelectionState};

fn fresh() -> SelectionState {
    SelectionState::new(Rect::new(0, 0, 1920, 1080), 8, 8)
}

#[test]
fn a_fresh_selection_has_nothing_to_draw_and_is_not_finished() {
    let s = fresh();
    assert!(s.current().is_none());
    assert_eq!(s.outcome(), Outcome::Pending);
}

#[test]
fn dragging_produces_a_live_rectangle_to_draw() {
    let mut s = fresh();
    s.press(100, 100);
    s.drag(300, 250);

    assert_eq!(s.current(), Some(Rect::new(100, 100, 200, 150)));
    assert_eq!(s.outcome(), Outcome::Pending, "still mid-drag");
}

#[test]
fn releasing_finishes_the_selection() {
    let mut s = fresh();
    s.press(100, 100);
    s.drag(300, 250);
    s.release();

    assert_eq!(s.outcome(), Outcome::Selected(Rect::new(100, 100, 200, 150)));
}

#[test]
fn dragging_backwards_selects_the_same_region() {
    let mut a = fresh();
    a.press(300, 250);
    a.drag(100, 100);
    a.release();

    let mut b = fresh();
    b.press(100, 100);
    b.drag(300, 250);
    b.release();

    assert_eq!(a.outcome(), b.outcome());
}

/// A stray click must not capture a 1x1 pixel. It is a cancellation.
#[test]
fn a_click_without_a_real_drag_cancels() {
    let mut s = fresh();
    s.press(500, 500);
    s.drag(502, 501);
    s.release();

    assert_eq!(s.outcome(), Outcome::Cancelled);
}

#[test]
fn a_selection_too_thin_in_one_axis_also_cancels() {
    let mut s = fresh();
    s.press(100, 100);
    s.drag(900, 104); // 800 wide but only 4 tall
    s.release();

    assert_eq!(s.outcome(), Outcome::Cancelled);
}

#[test]
fn escape_cancels_mid_drag() {
    let mut s = fresh();
    s.press(100, 100);
    s.drag(400, 400);
    s.cancel();

    assert_eq!(s.outcome(), Outcome::Cancelled);
    assert!(s.current().is_none(), "nothing should still be drawn");
}

#[test]
fn dragging_past_the_screen_edge_trims_to_the_screen() {
    let mut s = fresh();
    s.press(1800, 1000);
    s.drag(2500, 1500);
    s.release();

    assert_eq!(s.outcome(), Outcome::Selected(Rect::new(1800, 1000, 120, 80)));
}

/// Pointer motion arrives constantly; without a press it means nothing.
#[test]
fn motion_before_any_press_is_ignored() {
    let mut s = fresh();
    s.drag(400, 400);
    assert!(s.current().is_none());
    s.release();
    assert_eq!(s.outcome(), Outcome::Pending, "a stray release must not finish anything");
}

#[test]
fn a_finished_selection_ignores_further_events() {
    let mut s = fresh();
    s.press(100, 100);
    s.drag(300, 250);
    s.release();
    let settled = s.outcome();

    s.press(0, 0);
    s.drag(50, 50);
    s.release();

    assert_eq!(s.outcome(), settled, "outcome must not change after it is decided");
}

#[test]
fn a_cancelled_selection_stays_cancelled() {
    let mut s = fresh();
    s.cancel();
    s.press(10, 10);
    s.drag(500, 500);
    s.release();
    assert_eq!(s.outcome(), Outcome::Cancelled);
}

/// Monitors left of the primary have negative coordinates.
#[test]
fn selection_works_on_a_monitor_at_a_negative_offset() {
    let mut s = SelectionState::new(Rect::new(-1920, 0, 1920, 1080), 8, 8);
    s.press(-1800, 100);
    s.drag(-1500, 300);
    s.release();

    assert_eq!(s.outcome(), Outcome::Selected(Rect::new(-1800, 100, 300, 200)));
}
