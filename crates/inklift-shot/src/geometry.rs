use std::fmt;
use std::str::FromStr;

/// An axis-aligned rectangle in screen coordinates.
///
/// The origin may be negative: a monitor placed to the left of the primary one
/// starts at a negative `x`. Width and height are always positive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Build from two opposite corners in any order, which is what a drag
    /// gives you: people select bottom-right to top-left as often as not.
    pub fn from_corners(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self {
            x: x0.min(x1),
            y: y0.min(y1),
            width: x0.abs_diff(x1),
            height: y0.abs_diff(y1),
        }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height as i32
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Whether both dimensions meet a floor. Used to treat a stray click as a
    /// cancellation rather than a one-pixel capture.
    pub fn is_at_least(&self, width: u32, height: u32) -> bool {
        self.width >= width && self.height >= height
    }

    /// Trim to `bounds`, returning `None` when nothing overlaps. Trims rather
    /// than moves: a selection running off the edge should lose the overhang,
    /// not slide back into view.
    pub fn clamped_to(&self, bounds: &Rect) -> Option<Rect> {
        let x0 = self.x.max(bounds.x);
        let y0 = self.y.max(bounds.y);
        let x1 = self.right().min(bounds.right());
        let y1 = self.bottom().min(bounds.bottom());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
    }

    /// Re-express in coordinates local to `origin`, for indexing into a frame
    /// captured from one monitor.
    pub fn relative_to(&self, origin: &Rect) -> Rect {
        Rect::new(self.x - origin.x, self.y - origin.y, self.width, self.height)
    }

    /// Whether this rect fully contains `other`.
    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.x, self.y, self.width, self.height)
    }
}

impl FromStr for Rect {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(format!("expected X,Y,W,H but got {} value(s)", parts.len()));
        }
        let x = parts[0].parse::<i32>().map_err(|e| format!("x: {e}"))?;
        let y = parts[1].parse::<i32>().map_err(|e| format!("y: {e}"))?;
        let width = parts[2].parse::<i32>().map_err(|e| format!("width: {e}"))?;
        let height = parts[3].parse::<i32>().map_err(|e| format!("height: {e}"))?;
        if width <= 0 || height <= 0 {
            return Err(format!("width and height must be positive, got {width}x{height}"));
        }
        Ok(Rect::new(x, y, width as u32, height as u32))
    }
}
