//! Screen capture and interactive region selection.
//!
//! The logic that can be wrong - normalising a backwards drag, clamping to
//! screen bounds, mapping global coordinates into a monitor's frame - lives in
//! pure types that run headless. The parts that genuinely need a display are
//! kept as thin as possible around them.

mod capture;
mod clipboard;
mod frame;
mod geometry;
mod overlay;
mod selection;
#[cfg(target_os = "linux")]
mod x11;

pub use clipboard::{hold_image, needs_holder, put_image};
pub use capture::{Capturer, Monitor, monitor_at, monitor_for, virtual_bounds};
pub use frame::{Frame, crop_global};
pub use geometry::Rect;
pub use overlay::pick_region;
pub use selection::{Outcome, SelectionState};
#[cfg(target_os = "linux")]
pub use x11::X11Capturer;
