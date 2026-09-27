//! Clipboard handoff.
//!
//! On X11 and Wayland the clipboard is not storage: the owning process serves
//! the data whenever another application asks for it. A command that sets the
//! clipboard and exits therefore copies nothing at all - the selection dies
//! with it, unless a clipboard manager happens to be running to inherit it.
//!
//! The fix is the one arboard prescribes: hand the work to a process that
//! outlives the command and serves requests until the clipboard is replaced.

use std::time::Duration;

/// Whether this platform needs a process to stay alive to own the clipboard.
///
/// macOS and Windows copy into a system-owned buffer, so setting and exiting
/// is enough there.
pub const fn needs_holder() -> bool {
    cfg!(all(unix, not(target_os = "macos")))
}

fn check(width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!("clipboard buffer is {} bytes, expected {expected}", rgba.len()));
    }
    Ok(())
}

fn image(width: u32, height: u32, rgba: &[u8]) -> arboard::ImageData<'_> {
    arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: std::borrow::Cow::Borrowed(rgba),
    }
}

/// Put an image on the clipboard and return immediately.
///
/// Correct on macOS and Windows. On X11 and Wayland the content only lasts as
/// long as this process, so use [`hold_image`] in a process that sticks around.
pub fn put_image(width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    check(width, height, rgba)?;
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("no clipboard available: {e}"))?;
    clipboard
        .set_image(image(width, height, rgba))
        .map_err(|e| format!("could not set the clipboard: {e}"))
}

/// Put an image on the clipboard and keep serving it until something else is
/// copied, or `hold` elapses.
///
/// Blocks. Intended for a detached holder process. Returning early when the
/// clipboard is replaced means each new capture retires the previous holder,
/// so these do not accumulate.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn hold_image(width: u32, height: u32, rgba: &[u8], hold: Duration) -> Result<(), String> {
    use arboard::SetExtLinux;

    check(width, height, rgba)?;
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("no clipboard available: {e}"))?;
    clipboard
        .set()
        .wait_until(std::time::Instant::now() + hold)
        .image(image(width, height, rgba))
        .map_err(|e| format!("could not hold the clipboard: {e}"))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn hold_image(width: u32, height: u32, rgba: &[u8], _hold: Duration) -> Result<(), String> {
    put_image(width, height, rgba)
}
