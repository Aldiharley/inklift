/// Put an RGBA image on the system clipboard.
///
/// Failure here is reported but is never fatal to a capture: the files are
/// already on disk by the time this runs, and a headless or locked-down
/// session should not turn a successful extraction into an error.
pub fn put_image(width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!("clipboard buffer is {} bytes, expected {expected}", rgba.len()));
    }
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("no clipboard available: {e}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Borrowed(rgba),
        })
        .map_err(|e| format!("could not set the clipboard: {e}"))
}
