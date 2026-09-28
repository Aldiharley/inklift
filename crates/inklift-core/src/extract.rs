use crate::alpha::ink_opacity;
use crate::background::{estimate_background, normalize_illumination};
use crate::binarize::sauvola;
use crate::cleanup::despeckle;
use crate::color::{estimate_ink_color, luma};
use crate::grid::Grid;

/// Tuning for the classical extraction path. The defaults target a phone photo
/// of a page at roughly 150-300 DPI.
#[derive(Clone, Debug)]
pub struct Options {
    /// Closing radius for the paper estimate. `None` derives it from the image
    /// size. Must exceed the half-width of the thickest stroke.
    pub background_radius: Option<usize>,
    /// Sauvola window radius. A few times the stroke width, and never less than
    /// its half-width, or the middle of the stroke comes out transparent. See
    /// [`Options::for_thick_strokes`].
    pub sauvola_radius: usize,
    /// Sauvola `k`. Higher is more conservative and drops faint ink.
    pub sauvola_k: f32,
    /// Connected components smaller than this are discarded as dust.
    pub min_area: usize,
    /// How far the soft gate reaches past the thresholded stroke, in pixels.
    pub feather: usize,
    /// Erosion applied before sampling the pen colour.
    pub core_radius: usize,
    /// The ink is *lighter* than what it sits on - a screenshot of a
    /// dark-themed application, or chalk on a blackboard.
    ///
    /// The image is inverted before anything else runs, so the extracted ink
    /// comes out dark. That keeps both exports usable: a light ink composited
    /// onto white would be invisible. Colours invert with it, so white becomes
    /// black and a coloured foreground shifts hue.
    pub invert: bool,
}

impl Options {
    /// Rescale for a downscaled preview.
    ///
    /// A live retune loop runs on a proxy so it can keep up with a dragged
    /// slider, and every pixel-denominated setting has to come with it. Radii
    /// are lengths and scale by `s`; `min_area` is an area and scales by `s²`.
    /// Getting that exponent wrong yields a preview that is confidently wrong
    /// rather than visibly broken — much the worse failure for a tuning UI.
    ///
    /// Nothing is allowed to reach zero: a zero radius is a no-op filter, and
    /// the preview would silently show an unprocessed image.
    pub fn scaled_for_proxy(mut self, s: f32) -> Self {
        let len = |v: usize| ((v as f32 * s).round() as usize).max(1);
        self.sauvola_radius = len(self.sauvola_radius);
        self.feather = len(self.feather);
        self.background_radius = self.background_radius.map(len);
        self.min_area = ((self.min_area as f32 * s * s).round() as usize).max(1);
        // sauvola_k, invert and core_radius describe the image, not its size.
        self
    }

    /// Settings for strokes up to `half_width` pixels from centre line to edge.
    ///
    /// Two stages have to see past a thick stroke, and failing either leaves it
    /// hollow: the paper estimate, whose closing must bridge the stroke, and the
    /// Sauvola window, which must reach paper from the stroke's middle or finds
    /// no contrast there and calls it paper. Both need the half-width, so one
    /// number sets both.
    ///
    /// It was the window that failed in practice. On a 2K photo of 40 px
    /// calligraphy, a paper radius of 40 changed nothing and a window of 30
    /// cleared every pinhole; synthetic 40, 60 and 80 px strokes come out solid
    /// at exactly 20, 30 and 40 (`tests/thick_strokes.rs`). The window only ever
    /// widens here: too narrow is holes, whereas too wide only picks up more
    /// dark clutter around the page.
    pub fn for_thick_strokes(mut self, half_width: usize) -> Self {
        self.background_radius = Some(half_width);
        self.sauvola_radius = self.sauvola_radius.max(half_width);
        self
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            background_radius: None,
            sauvola_radius: 12,
            sauvola_k: 0.2,
            min_area: 8,
            feather: 1,
            core_radius: 1,
            invert: false,
        }
    }
}

/// Extracted ink: an opacity field plus the pen colour it should be painted in.
///
/// Deliberately *not* a finished image. Keeping opacity and colour separate is
/// what lets the same result be written as a transparent PNG or as ink on white
/// without either export losing information.
#[derive(Clone, Debug)]
pub struct Extraction {
    alpha: Grid,
    ink_color: [f32; 3],
}

impl Extraction {
    pub fn alpha(&self) -> &Grid {
        &self.alpha
    }

    pub fn ink_color(&self) -> [f32; 3] {
        self.ink_color
    }

    /// Paint the same ink in a different colour.
    ///
    /// Only the pen colour changes; the opacity field is untouched, so this
    /// costs nothing and loses nothing. It exists because the extracted pen is
    /// the *real* one, which is usually dark — and dark ink is invisible on a
    /// dark slide. Keeping opacity and colour apart is what makes that a swap
    /// rather than a re-extraction.
    pub fn with_ink_color(mut self, rgb: [f32; 3]) -> Self {
        self.ink_color = [
            rgb[0].clamp(0.0, 1.0),
            rgb[1].clamp(0.0, 1.0),
            rgb[2].clamp(0.0, 1.0),
        ];
        self
    }

    pub fn width(&self) -> usize {
        self.alpha.width()
    }

    pub fn height(&self) -> usize {
        self.alpha.height()
    }

    /// Fraction of the page carrying meaningful ink.
    pub fn coverage(&self) -> f32 {
        if self.alpha.is_empty() {
            return 0.0;
        }
        let n = self.alpha.data().iter().filter(|&&a| a > 0.5).count();
        n as f32 / self.alpha.len() as f32
    }

    /// Straight (non-premultiplied) RGBA. Fully transparent pixels are zeroed
    /// so that naive scaling cannot drag paper colour into the stroke edges.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.alpha.len() * 4];
        for i in 0..self.alpha.len() {
            let a = self.alpha.data()[i].clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            for ch in 0..3 {
                out[i * 4 + ch] = to_u8(self.ink_color[ch]);
            }
            out[i * 4 + 3] = to_u8(a);
        }
        out
    }

    /// The ink composited onto white, in colour. One byte per channel.
    pub fn to_rgb_on_white8(&self) -> Vec<u8> {
        let mut out = vec![255u8; self.alpha.len() * 3];
        for i in 0..self.alpha.len() {
            let a = self.alpha.data()[i].clamp(0.0, 1.0);
            for ch in 0..3 {
                out[i * 3 + ch] = to_u8(a * self.ink_color[ch] + (1.0 - a));
            }
        }
        out
    }

    /// The ink composited onto white, in greyscale.
    ///
    /// Greyscale rather than a hard binary on purpose: the opacity is still in
    /// there, so [`alpha_from_gray_on_white`] can recover it later. Thresholding
    /// at this point would be the one irreversible step in the pipeline.
    pub fn to_gray_on_white8(&self) -> Vec<u8> {
        let ink = luma(self.ink_color);
        let mut out = vec![255u8; self.alpha.len()];
        for i in 0..self.alpha.len() {
            let a = self.alpha.data()[i].clamp(0.0, 1.0);
            out[i] = to_u8(a * ink + (1.0 - a));
        }
        out
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Recover opacity from a greyscale-on-white export.
///
/// The inverse of [`Extraction::to_gray_on_white8`], accurate to quantisation.
/// This is what makes shipping the white-background version first a reversible
/// decision rather than a dead end.
pub fn alpha_from_gray_on_white(gray: &[u8], ink_luma: f32) -> Vec<f32> {
    let span = (1.0 - ink_luma).max(1e-3);
    gray.iter()
        .map(|&g| ((1.0 - g as f32 / 255.0) / span).clamp(0.0, 1.0))
        .collect()
}

/// Whether an image looks like light ink on a dark ground.
///
/// Uses the median rather than the mean, so a small blaze of brightness - a
/// selected line, a white dialog over a dark desktop - does not outvote the
/// bulk of the image. Intended for suggesting `invert`, not for switching it
/// on automatically: guessing wrong silently would be worse than doing nothing.
pub fn looks_inverted(rgb: &[Grid; 3]) -> bool {
    if rgb[0].is_empty() {
        return false;
    }
    let mut luminance: Vec<f32> = (0..rgb[0].len())
        .map(|i| (rgb[0].data()[i] + rgb[1].data()[i] + rgb[2].data()[i]) / 3.0)
        .collect();
    luminance.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    luminance[luminance.len() / 2] < 0.5
}

/// Run the classical extraction pipeline over a linear RGB image.
///
/// Channels are expected in `[0, 1]`. Stages, in order: estimate the paper,
/// divide the lighting out, threshold locally, drop dust, then turn the binary
/// decision into a soft opacity and sample the pen colour.
pub fn extract(rgb: &[Grid; 3], options: &Options) -> Extraction {
    let (w, h) = (rgb[0].width(), rgb[0].height());
    debug_assert!(rgb[1].same_shape(&rgb[0]) && rgb[2].same_shape(&rgb[0]));

    // Everything downstream assumes dark ink on light paper. Rather than
    // teach each stage about the other polarity, flip the input once and let
    // the rest of the pipeline stay exactly as it is.
    let flipped;
    let rgb = if options.invert {
        flipped = [
            rgb[0].map(|v| 1.0 - v),
            rgb[1].map(|v| 1.0 - v),
            rgb[2].map(|v| 1.0 - v),
        ];
        &flipped
    } else {
        rgb
    };

    let mut gray = Grid::new(w, h);
    for i in 0..gray.len() {
        gray.data_mut()[i] = (rgb[0].data()[i] + rgb[1].data()[i] + rgb[2].data()[i]) / 3.0;
    }

    let background = estimate_background(&gray, options.background_radius);
    let flat = normalize_illumination(&gray, &background);
    let mask = despeckle(
        &sauvola(&flat, options.sauvola_radius, options.sauvola_k),
        options.min_area,
    );
    let alpha = ink_opacity(&flat, &mask, options.feather);

    let normalized_rgb = [
        normalize_illumination(&rgb[0], &background),
        normalize_illumination(&rgb[1], &background),
        normalize_illumination(&rgb[2], &background),
    ];
    let ink_color = estimate_ink_color(&normalized_rgb, &mask, options.core_radius);

    Extraction { alpha, ink_color }
}
