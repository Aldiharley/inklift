use crate::filters::{box_blur, max_filter};
use crate::grid::Grid;
use crate::mask::Mask;

/// Build a soft 0..1 gate from a binary mask.
///
/// The binary decision is dilated by `feather` and then blurred by the same
/// amount, so the antialiased rim just outside the thresholded stroke is still
/// allowed to contribute. Without this the output has stair-stepped edges no
/// matter how good the underlying opacity estimate is.
fn soft_gate(mask: &Mask, feather: usize) -> Grid {
    let mut g = Grid::new(mask.width(), mask.height());
    for i in 0..mask.len() {
        g.data_mut()[i] = if mask.data()[i] { 1.0 } else { 0.0 };
    }
    if feather == 0 {
        return g;
    }
    box_blur(&max_filter(&g, feather), feather)
}

/// Ink opacity per pixel: how much of the paper this pixel's ink hides.
///
/// From `observed = alpha * ink + (1 - alpha) * paper`, dividing through by the
/// estimated paper gives `normalized = alpha * ink/paper + (1 - alpha)`, so
/// `alpha = 1 - normalized` once the ink is much darker than the paper. The
/// mask decides *which* pixels may carry opacity; this division decides *how
/// much*, and it is what keeps stroke edges smooth.
pub fn ink_opacity(normalized: &Grid, mask: &Mask, feather: usize) -> Grid {
    debug_assert_eq!(normalized.len(), mask.len(), "shape mismatch");
    let gate = soft_gate(mask, feather);
    let mut out = Grid::new(normalized.width(), normalized.height());
    for i in 0..normalized.len() {
        let raw = (1.0 - normalized.data()[i]).clamp(0.0, 1.0);
        out.data_mut()[i] = (raw * gate.data()[i]).clamp(0.0, 1.0);
    }
    out
}
