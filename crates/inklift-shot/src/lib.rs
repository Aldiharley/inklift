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
mod live;
mod selection;
#[cfg(target_os = "linux")]
mod x11;

pub use clipboard::{hold_image, needs_holder, put_image};
pub use capture::{Capturer, Monitor, monitor_at, monitor_for, virtual_bounds};
pub use frame::{Frame, crop_global};
pub use geometry::Rect;
pub use live::{outline_bars, pick_live_region, pick_live_region_ready};
pub use selection::{Outcome, SelectionState, resolve_pick};
#[cfg(target_os = "linux")]
pub use x11::X11Capturer;
