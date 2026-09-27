//! The selection outline must never sit inside the selection.
//!
//! The old overlay shipped exactly this bug: its own border was measured inside
//! a capture. The new selector draws on the live screen, so a bar that strays
//! one pixel inward is photographed as ink.

use inklift_shot::{Rect, outline_bars};

#[test]
fn no_bar_overlaps_the_selected_region() {
    let sel = Rect::new(100, 80, 300, 200);
    for t in [1u32, 2, 4] {
        for bar in outline_bars(&sel, t) {
            assert!(
                bar.width > 0 && bar.height > 0,
                "a zero-sized window is an X11 error, not a thin line"
            );
            for (px, py) in [
                (bar.x, bar.y),
                (bar.right() - 1, bar.y),
                (bar.x, bar.bottom() - 1),
                (bar.right() - 1, bar.bottom() - 1),
            ] {
                assert!(
                    !sel.contains(&Rect::new(px, py, 1, 1)),
                    "thickness {t}: bar {bar} has a corner at ({px},{py}) inside \
                     the selection {sel} — that pixel would be captured as ink"
                );
            }
        }
    }
}

#[test]
fn the_outline_encloses_the_selection_on_all_four_sides() {
    let sel = Rect::new(100, 80, 300, 200);
    let bars = outline_bars(&sel, 2);
    assert_eq!(bars.len(), 4);
    // above, below, left, right
    assert!(bars.iter().any(|b| b.bottom() == sel.y), "no bar sits above");
    assert!(bars.iter().any(|b| b.y == sel.bottom()), "no bar sits below");
    assert!(bars.iter().any(|b| b.right() == sel.x), "no bar sits left");
    assert!(bars.iter().any(|b| b.x == sel.right()), "no bar sits right");
}

#[test]
fn a_selection_at_the_screen_origin_still_yields_drawable_bars() {
    // Bars go outside the selection, so at (0,0) they land at negative
    // coordinates. That is legal for an X11 window and must not be clamped
    // inward, which would put them on top of the capture.
    let bars = outline_bars(&Rect::new(0, 0, 50, 40), 2);
    assert!(bars.iter().any(|b| b.x < 0 || b.y < 0));
    for b in bars {
        assert!(b.width > 0 && b.height > 0);
    }
}

#[test]
fn a_one_pixel_selection_does_not_produce_a_zero_sized_bar() {
    for b in outline_bars(&Rect::new(10, 10, 1, 1), 1) {
        assert!(b.width > 0 && b.height > 0, "{b} would be rejected by X11");
    }
}
