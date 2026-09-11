//! The DIBCO evaluation measures.
//!
//! `f_measure`, `psnr` and `drd` follow the definitions printed in the
//! competition reports and are directly comparable with published numbers.
//! See [`pseudo_f_measure`] for the one caveat.

use crate::mask::Mask;

/// Pixel counts with ink as the positive class.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Confusion {
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
}

impl Confusion {
    pub fn of(predicted: &Mask, truth: &Mask) -> Self {
        debug_assert_eq!(predicted.len(), truth.len(), "shape mismatch");
        let mut c = Self::default();
        for i in 0..truth.len() {
            match (predicted.data()[i], truth.data()[i]) {
                (true, true) => c.true_positive += 1,
                (true, false) => c.false_positive += 1,
                (false, true) => c.false_negative += 1,
                (false, false) => c.true_negative += 1,
            }
        }
        c
    }

    pub fn recall(&self) -> f32 {
        let denominator = self.true_positive + self.false_negative;
        if denominator == 0 {
            return 0.0;
        }
        self.true_positive as f32 / denominator as f32
    }

    pub fn precision(&self) -> f32 {
        let denominator = self.true_positive + self.false_positive;
        if denominator == 0 {
            return 0.0;
        }
        self.true_positive as f32 / denominator as f32
    }
}

/// Harmonic mean of recall and precision, as a percentage.
pub fn f_measure(confusion: &Confusion) -> f32 {
    harmonic(confusion.recall(), confusion.precision())
}

fn harmonic(recall: f32, precision: f32) -> f32 {
    if recall + precision <= 0.0 {
        return 0.0;
    }
    100.0 * 2.0 * recall * precision / (recall + precision)
}

/// Peak signal-to-noise ratio in dB.
///
/// `C` is 1 for a binary image, so this reduces to `10 log10(pixels / errors)`.
/// Returns infinity for an exact match, which callers must handle before
/// averaging across a set.
pub fn psnr(predicted: &Mask, truth: &Mask) -> f32 {
    debug_assert_eq!(predicted.len(), truth.len(), "shape mismatch");
    if truth.is_empty() {
        return f32::INFINITY;
    }
    let errors = (0..truth.len())
        .filter(|&i| predicted.data()[i] != truth.data()[i])
        .count();
    if errors == 0 {
        return f32::INFINITY;
    }
    10.0 * (truth.len() as f32 / errors as f32).log10()
}

/// Reciprocal-distance weights over a 5x5 window, normalised to sum to 1.
fn drd_weights() -> [[f32; 5]; 5] {
    let mut w = [[0.0f32; 5]; 5];
    let mut total = 0.0;
    for (i, row) in w.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            let (di, dj) = (i as f32 - 2.0, j as f32 - 2.0);
            if di != 0.0 || dj != 0.0 {
                *cell = 1.0 / (di * di + dj * dj).sqrt();
                total += *cell;
            }
        }
    }
    for row in w.iter_mut() {
        for cell in row.iter_mut() {
            *cell /= total;
        }
    }
    w
}

/// Count 8x8 blocks of the ground truth that are not entirely one class.
///
/// Uniform blocks carry no text, so normalising by this rather than by total
/// area stops a mostly-blank page from flattering a result.
fn non_uniform_blocks(truth: &Mask) -> usize {
    let (w, h) = (truth.width(), truth.height());
    let mut count = 0;
    let mut by = 0;
    while by < h {
        let mut bx = 0;
        while bx < w {
            let first = truth.get(bx, by);
            let mut uniform = true;
            'block: for y in by..(by + 8).min(h) {
                for x in bx..(bx + 8).min(w) {
                    if truth.get(x, y) != first {
                        uniform = false;
                        break 'block;
                    }
                }
            }
            if !uniform {
                count += 1;
            }
            bx += 8;
        }
        by += 8;
    }
    count
}

/// Distance Reciprocal Distortion: visual damage per non-uniform block.
///
/// Each flipped pixel is charged by how much its 5x5 neighbourhood disagrees
/// with it, weighted by reciprocal distance. An error surrounded by matching
/// pixels is cheap; one standing alone in open paper costs the full weight.
/// Lower is better, and 0 is exact.
pub fn drd(predicted: &Mask, truth: &Mask) -> f32 {
    debug_assert_eq!(predicted.len(), truth.len(), "shape mismatch");
    let blocks = non_uniform_blocks(truth);
    if blocks == 0 {
        return 0.0;
    }
    let weights = drd_weights();
    let (w, h) = (truth.width(), truth.height());
    let mut total = 0.0f32;

    for y in 0..h {
        for x in 0..w {
            if predicted.get(x, y) == truth.get(x, y) {
                continue;
            }
            let flipped: f32 = if predicted.get(x, y) { 1.0 } else { 0.0 };
            for i in 0..5usize {
                for j in 0..5usize {
                    // Outside the image, the ground truth is taken to continue
                    // as background, matching the reference implementation.
                    let ny = y as isize + i as isize - 2;
                    let nx = x as isize + j as isize - 2;
                    let neighbour: f32 = if ny < 0 || nx < 0 || ny >= h as isize || nx >= w as isize {
                        0.0
                    } else if truth.get(nx as usize, ny as usize) {
                        1.0
                    } else {
                        0.0
                    };
                    total += (neighbour - flipped).abs() * weights[i][j];
                }
            }
        }
    }
    total / blocks as f32
}

/// Zhang-Suen thinning: reduce each stroke to a one-pixel-wide medial axis
/// while preserving connectivity.
pub fn skeleton(mask: &Mask) -> Mask {
    let (w, h) = (mask.width(), mask.height());
    let mut current = mask.clone();
    if w < 3 || h < 3 {
        return current;
    }

    loop {
        let mut changed = false;
        for step in 0..2 {
            let mut doomed: Vec<usize> = Vec::new();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    if !current.get(x, y) {
                        continue;
                    }
                    // P2..P9, clockwise from north.
                    let n = [
                        current.get(x, y - 1),
                        current.get(x + 1, y - 1),
                        current.get(x + 1, y),
                        current.get(x + 1, y + 1),
                        current.get(x, y + 1),
                        current.get(x - 1, y + 1),
                        current.get(x - 1, y),
                        current.get(x - 1, y - 1),
                    ];
                    let filled = n.iter().filter(|&&b| b).count();
                    if !(2..=6).contains(&filled) {
                        continue;
                    }
                    let transitions = (0..8)
                        .filter(|&i| !n[i] && n[(i + 1) % 8])
                        .count();
                    if transitions != 1 {
                        continue;
                    }
                    // n[0]=P2 n[2]=P4 n[4]=P6 n[6]=P8
                    let (a, b) = if step == 0 {
                        (n[0] && n[2] && n[4], n[2] && n[4] && n[6])
                    } else {
                        (n[0] && n[2] && n[6], n[0] && n[4] && n[6])
                    };
                    if !a && !b {
                        doomed.push(y * w + x);
                    }
                }
            }
            if !doomed.is_empty() {
                changed = true;
                for i in doomed {
                    current.data_mut()[i] = false;
                }
            }
        }
        if !changed {
            return current;
        }
    }
}

/// Pseudo-F-Measure, as defined for H-DIBCO 2010 and 2012.
///
/// Combines ordinary precision with a *pseudo-recall* measured against the
/// skeletonised ground truth: a stroke counts as found when its medial axis was
/// found, so thinning a stroke is forgiven while breaking one is not.
///
/// Two honest caveats before comparing against a published table:
///
/// 1. DIBCO 2011 and 2013 onward report a *different* measure, `Fps`, which
///    also uses a pseudo-*precision* with contour distance weights normalised
///    by local stroke width. The competition papers describe it only
///    qualitatively, so it is not reimplemented here.
/// 2. The competitions used a semi-manually corrected skeleton; this derives
///    one automatically. Numbers are therefore comparable across your own runs,
///    not against a published leaderboard.
pub fn pseudo_f_measure(predicted: &Mask, truth: &Mask) -> f32 {
    debug_assert_eq!(predicted.len(), truth.len(), "shape mismatch");
    let sg = skeleton(truth);
    let total = sg.count();
    if total == 0 {
        return 0.0;
    }
    let found = (0..sg.len())
        .filter(|&i| sg.data()[i] && predicted.data()[i])
        .count();
    let pseudo_recall = found as f32 / total as f32;
    harmonic(pseudo_recall, Confusion::of(predicted, truth).precision())
}

/// Every DIBCO measure for one image.
#[derive(Clone, Copy, Debug)]
pub struct Scores {
    /// F-Measure, percent. Higher is better.
    pub fm: f32,
    /// Pseudo-F-Measure, percent. Higher is better. See [`pseudo_f_measure`].
    pub p_fm: f32,
    /// Peak signal-to-noise ratio, dB. Higher is better; infinite if exact.
    pub psnr: f32,
    /// Distance Reciprocal Distortion. Lower is better; 0 is exact.
    pub drd: f32,
}

pub fn score(predicted: &Mask, truth: &Mask) -> Scores {
    Scores {
        fm: f_measure(&Confusion::of(predicted, truth)),
        p_fm: pseudo_f_measure(predicted, truth),
        psnr: psnr(predicted, truth),
        drd: drd(predicted, truth),
    }
}
