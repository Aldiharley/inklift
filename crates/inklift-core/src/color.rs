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

/// Fraction of core samples taken as representative of undiluted ink.
///
/// Not the median. At low resolution, or through lens softness and JPEG
/// ringing, a thin stroke has almost no fully covered pixels: nearly every one
/// is part ink, part paper. A median over those lands halfway to the paper and
/// reports a pen far lighter than it is, which then caps how dark the white
/// composite can ever go. Taking a low percentile asks instead "what colour is
/// this pen where it is least diluted", while staying robust to the handful of
/// dark outliers a plain minimum would seize on.
const INK_PERCENTILE: f32 = 0.10;

/// Estimate the pen colour, expressed relative to the paper.
///
/// Sampled from the eroded core of the mask, never the feathered rim, where
/// every pixel is a blend of ink and paper and would wash the colour out.
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
        // Values are darkest-first once sorted, so a low index is the least
        // diluted ink.
        let index = ((values.len() as f32 * INK_PERCENTILE) as usize).min(values.len() - 1);
        out[ch] = values[index].clamp(0.0, 1.0);
    }
    out
}

/// Parse a pen colour: `#RRGGBB`, `#RGB`, either without the hash, or one of a
/// few names. Returned as `[0,1]` channels.
///
/// Deliberately a short list of names. A full CSS colour table invites
/// "chartreuse" and returns nothing useful for ink.
pub fn parse_ink_color(text: &str) -> Result<[f32; 3], String> {
    let t = text.trim();
    match t.to_ascii_lowercase().as_str() {
        "black" => return Ok([0.0, 0.0, 0.0]),
        "white" => return Ok([1.0, 1.0, 1.0]),
        _ => {}
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    let expand = |c: u8| -> u8 {
        let v = (c as char).to_digit(16).unwrap_or(0) as u8;
        v * 17 // #abc -> #aabbcc
    };
    let bytes = hex.as_bytes();
    let rgb = match bytes.len() {
        3 if bytes.iter().all(|b| (*b as char).is_ascii_hexdigit()) => {
            [expand(bytes[0]), expand(bytes[1]), expand(bytes[2])]
        }
        6 if bytes.iter().all(|b| (*b as char).is_ascii_hexdigit()) => {
            let p = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
            [p(0), p(2), p(4)]
        }
        _ => {
            return Err(format!(
                "could not read {text:?} as a colour. Use #RRGGBB, #RGB, black or white."
            ));
        }
    };
    Ok([
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    ])
}
