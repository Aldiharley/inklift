use crate::frame::Frame;
use crate::geometry::Rect;

/// One screen in the virtual desktop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    /// Position and size in global coordinates. May start negative.
    pub bounds: Rect,
    pub primary: bool,
}

/// The smallest rectangle covering every monitor.
pub fn virtual_bounds(monitors: &[Monitor]) -> Option<Rect> {
    let first = monitors.first()?.bounds;
    let (mut x0, mut y0) = (first.x, first.y);
    let (mut x1, mut y1) = (first.right(), first.bottom());
    for m in &monitors[1..] {
        x0 = x0.min(m.bounds.x);
        y0 = y0.min(m.bounds.y);
        x1 = x1.max(m.bounds.right());
        y1 = y1.max(m.bounds.bottom());
    }
    Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// The monitor under a point.
pub fn monitor_at(monitors: &[Monitor], x: i32, y: i32) -> Option<&Monitor> {
    monitors
        .iter()
        .find(|m| x >= m.bounds.x && x < m.bounds.right() && y >= m.bounds.y && y < m.bounds.bottom())
}

/// The monitor holding the largest share of a region.
///
/// A selection dragged across a seam has to come from somewhere; taking the
/// screen with the most of it keeps the capture a single frame rather than a
/// stitch of two, which would have to reconcile differing scale factors.
pub fn monitor_for<'a>(monitors: &'a [Monitor], region: &Rect) -> Option<&'a Monitor> {
    monitors
        .iter()
        .filter_map(|m| region.clamped_to(&m.bounds).map(|overlap| (overlap.area(), m)))
        .max_by_key(|(area, _)| *area)
        .map(|(_, m)| m)
}

/// A source of screen pixels.
///
/// Implemented for X11 today. macOS and Windows slot in behind the same two
/// methods without anything above this line changing.
pub trait Capturer {
    fn monitors(&self) -> Result<Vec<Monitor>, String>;
    /// Grab a region given in global coordinates.
    fn grab(&self, region: &Rect) -> Result<Frame, String>;
}
