use crate::filters::{grey_close, smooth};
use crate::grid::Grid;

/// Default closing radius: comfortably wider than a pen stroke at any sane
/// capture resolution, but small enough to track real lighting variation.
pub fn default_radius(width: usize, height: usize) -> usize {
    (width.min(height) / 24).max(7)
}

/// Estimate the paper luminance field, with the ink removed.
///
/// Pass `radius = None` to derive one from the image size. It must exceed the
/// half-width of the thickest stroke, or that stroke will be treated as paper.
pub fn estimate_background(gray: &Grid, radius: Option<usize>) -> Grid {
    if gray.is_empty() {
        return gray.clone();
    }
    let radius = radius.unwrap_or_else(|| default_radius(gray.width(), gray.height()));
    smooth(&grey_close(gray, radius), radius)
}

/// Divide the lighting out: bare paper becomes 1.0, solid ink approaches 0.0.
///
/// Values above 1.0 are preserved rather than clipped, so callers can still
/// distinguish "brighter than the estimated paper" from "exactly paper".
pub fn normalize_illumination(gray: &Grid, background: &Grid) -> Grid {
    debug_assert!(gray.same_shape(background), "shape mismatch");
    let mut out = Grid::new(gray.width(), gray.height());
    for i in 0..gray.len() {
        out.data_mut()[i] = gray.data()[i] / background.data()[i].max(1e-3);
    }
    out
}
