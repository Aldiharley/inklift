use crate::filters::box_blur;
use crate::grid::Grid;
use crate::mask::Mask;

/// Dynamic range of the local standard deviation for data scaled to `[0, 1]`.
/// This is Sauvola's `R = 128` rewritten for unit-range images.
const R: f32 = 0.5;

/// Sauvola local thresholding.
///
/// `t = m * (1 + k * (s / R - 1))`, where `m` and `s` are the local mean and
/// standard deviation. The `s` term is what stops the threshold from chasing
/// noise across blank paper: with no local contrast, `t` falls well below the
/// paper value and nothing fires.
///
/// Expects an illumination-normalized image where bare paper sits near 1.0.
/// `k` around 0.2 is a good default; larger values are more conservative.
pub fn sauvola(normalized: &Grid, radius: usize, k: f32) -> Mask {
    let (w, h) = (normalized.width(), normalized.height());
    if normalized.is_empty() {
        return Mask::new(w, h);
    }
    let mean = box_blur(normalized, radius);
    let mean_of_squares = box_blur(&normalized.map(|v| v * v), radius);

    let mut out = Mask::new(w, h);
    for i in 0..normalized.len() {
        let m = mean.data()[i];
        let variance = (mean_of_squares.data()[i] - m * m).max(0.0);
        let threshold = m * (1.0 + k * (variance.sqrt() / R - 1.0));
        out.data_mut()[i] = normalized.data()[i] < threshold;
    }
    out
}
