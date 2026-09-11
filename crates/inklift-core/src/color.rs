use crate::filters::min_filter;
use crate::grid::Grid;
use crate::mask::Mask;

/// Rec. 709 relative luminance.
pub fn luma(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// Shrink a mask by `radius`, leaving only its confident interior.
fn erode(mask: &Mask, radius: usize) -> Mask {
    if radius == 0 {
        return mask.clone();
    }
    let mut g = Grid::new(mask.width(), mask.height());
    for i in 0..mask.len() {
        g.data_mut()[i] = if mask.data()[i] { 1.0 } else { 0.0 };
    }
    let shrunk = min_filter(&g, radius);
    let mut out = Mask::new(mask.width(), mask.height());
    for i in 0..mask.len() {
        out.data_mut()[i] = shrunk.data()[i] > 0.5;
    }
    out
}

/// Estimate the pen colour, expressed relative to the paper.
///
/// Sampled from the eroded core of the mask, never the feathered rim, where
/// every pixel is a blend of ink and paper and would wash the colour out. The
/// median rather than the mean, so a few stray pixels cannot drag it.
///
/// Assumes a roughly neutral paper: each channel is normalized by the shared
/// luminance background rather than its own, which costs one closing instead of
/// three. Strongly tinted paper would need per-channel estimation.
pub fn estimate_ink_color(normalized_rgb: &[Grid; 3], mask: &Mask, core_radius: usize) -> [f32; 3] {
    let core = erode(mask, core_radius);
    let source = if core.count() >= 16 { core } else { mask.clone() };

    let mut out = [0.0f32; 3];
    for ch in 0..3 {
        let mut values: Vec<f32> = (0..source.len())
            .filter(|&i| source.data()[i])
            .map(|i| normalized_rgb[ch].data()[i])
            .collect();
        if values.is_empty() {
            continue;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        out[ch] = values[values.len() / 2].clamp(0.0, 1.0);
    }
    out
}
