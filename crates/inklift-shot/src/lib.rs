//! Screen capture and interactive region selection.
//!
//! The logic that can be wrong - normalising a backwards drag, clamping to
//! screen bounds, mapping global coordinates into a monitor's frame - lives in
//! pure types that run headless. The parts that genuinely need a display are
//! kept as thin as possible around them.
//!
//! Each platform provides the same three things: a [`Capturer`], exported under
//! its own name and as `NativeCapturer`, plus `pick_live_region` and
//! `pick_live_region_ready`. Callers use those and never name a platform.

mod capture;
mod clipboard;
mod frame;
mod geometry;
mod outline;
mod selection;
#[cfg(target_os = "linux")]
mod live;
#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
mod win_live;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod unsupported;

pub use clipboard::{hold_image, needs_holder, put_image};
pub use capture::{Capturer, Monitor, monitor_at, monitor_for, virtual_bounds};
pub use frame::{Frame, crop_global};
pub use geometry::Rect;
pub use outline::outline_bars;
pub use selection::{Outcome, SelectionState, resolve_pick};

#[cfg(target_os = "linux")]
pub use live::{pick_live_region, pick_live_region_ready};
#[cfg(target_os = "linux")]
pub use x11::{X11Capturer, X11Capturer as NativeCapturer};

#[cfg(target_os = "windows")]
pub use win::{WindowsCapturer, WindowsCapturer as NativeCapturer};
#[cfg(target_os = "windows")]
pub use win_live::{pick_live_region, pick_live_region_ready};

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use unsupported::{NativeCapturer, pick_live_region, pick_live_region_ready};

/// Whether this build has a capture backend at all.
///
/// Read by anything that offers capture to a user, so a control that could
/// only fail is greyed out rather than shown.
pub const CAPTURE_SUPPORTED: bool = cfg!(any(target_os = "linux", target_os = "windows"));
