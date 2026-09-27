use crate::geometry::Rect;

/// What the selection has settled on, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Still waiting for the user.
    Pending,
    /// A region was chosen.
    Selected(Rect),
    /// The user backed out, or the selection was too small to mean anything.
    Cancelled,
}

/// The drag state machine, free of any windowing library.
///
/// The overlay's only job is to translate input events into `press`, `drag`,
/// `release` and `cancel`. Every rule that could be wrong lives here, where it
/// can be tested without a display.
#[derive(Clone, Debug)]
pub struct SelectionState {
    bounds: Rect,
    min_width: u32,
    min_height: u32,
    anchor: Option<(i32, i32)>,
    cursor: (i32, i32),
    outcome: Outcome,
}

impl SelectionState {
    /// `min_width` and `min_height` are the floor below which a selection is
    /// read as a misclick rather than an intentional capture.
    pub fn new(bounds: Rect, min_width: u32, min_height: u32) -> Self {
        Self {
            bounds,
            min_width,
            min_height,
            anchor: None,
            cursor: (0, 0),
            outcome: Outcome::Pending,
        }
    }

    fn settled(&self) -> bool {
        !matches!(self.outcome, Outcome::Pending)
    }

    pub fn press(&mut self, x: i32, y: i32) {
        if self.settled() {
            return;
        }
        self.anchor = Some((x, y));
        self.cursor = (x, y);
    }

    pub fn drag(&mut self, x: i32, y: i32) {
        if self.settled() || self.anchor.is_none() {
            return;
        }
        self.cursor = (x, y);
    }

    /// The rectangle to draw right now, already trimmed to the screen.
    pub fn current(&self) -> Option<Rect> {
        if self.settled() {
            return None;
        }
        let (ax, ay) = self.anchor?;
        Rect::from_corners(ax, ay, self.cursor.0, self.cursor.1).clamped_to(&self.bounds)
    }

    /// Finish. A selection below the minimum size counts as a cancellation,
    /// so a stray click never captures a sliver.
    pub fn release(&mut self) -> Outcome {
        if self.settled() || self.anchor.is_none() {
            return self.outcome;
        }
        self.outcome = match self.current() {
            Some(rect) if rect.is_at_least(self.min_width, self.min_height) => {
                Outcome::Selected(rect)
            }
            _ => Outcome::Cancelled,
        };
        self.anchor = None;
        self.outcome
    }

    pub fn cancel(&mut self) {
        if self.settled() {
            return;
        }
        self.anchor = None;
        self.outcome = Outcome::Cancelled;
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn bounds(&self) -> Rect {
        self.bounds
    }
}
