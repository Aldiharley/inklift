//! Where the selection outline goes, independent of any windowing system.
//!
//! Every platform's selector draws the same four bars, so where they sit is
//! decided once, here, and tested without a display.

use crate::geometry::Rect;

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
