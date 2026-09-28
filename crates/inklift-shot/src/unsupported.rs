//! What a platform with no capture backend gets: a refusal that explains itself.
//!
//! This exists so callers can be written once against `NativeCapturer` and
//! `pick_live_region` with no platform cfgs of their own. It is the one stub,
//! and it goes when the last platform gets a real backend — macOS is the one
//! left; see `docs/porting-capture.md`.

use crate::capture::{Capturer, Monitor};
use crate::frame::Frame;
use crate::geometry::Rect;
use crate::selection::Outcome;

const NO_CAPTURE: &str = "Lifting from the screen is not available on this \
platform yet: inklift has capture backends for Linux (X11) and Windows only. \
Open a file instead — extraction itself works everywhere.";

/// Uninhabited: `new` is the only way to get one and it always refuses, so the
/// trait methods below cannot be reached and need no behaviour of their own.
pub enum NativeCapturer {}

impl NativeCapturer {
    pub fn new() -> Result<Self, String> {
        Err(NO_CAPTURE.into())
    }
}

impl Capturer for NativeCapturer {
    fn monitors(&self) -> Result<Vec<Monitor>, String> {
        match *self {}
    }

    fn grab(&self, _region: &Rect) -> Result<Frame, String> {
        match *self {}
    }
}

pub fn pick_live_region(_bounds: Rect, _min: u32) -> Result<Outcome, String> {
    Err(NO_CAPTURE.into())
}

pub fn pick_live_region_ready<F: FnOnce()>(
    _bounds: Rect,
    _min: u32,
    _on_ready: F,
) -> Result<Outcome, String> {
    Err(NO_CAPTURE.into())
}
