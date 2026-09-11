//! Synthetic "photograph of handwriting" fixtures with known ground truth.
//!
//! A page is built by explicitly compositing ink over lit paper, so tests can
//! assert against the exact quantities the pipeline is trying to recover.
#![allow(dead_code)]

use inklift_core::Grid;

pub const PAPER: f32 = 0.93;

/// Deterministic xorshift, so fixtures are reproducible across platforms.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// Uniform in `[-n, n]`.
    pub fn jitter(&mut self, n: i64) -> i64 {
        if n == 0 { return 0; }
        (self.next_u64() % (2 * n as u64 + 1)) as i64 - n
    }
    /// Roughly standard normal, via a sum of uniforms.
    pub fn normal(&mut self) -> f32 {
        let mut acc = 0.0f32;
        for _ in 0..6 {
            acc += (self.next_u64() % 10_000) as f32 / 10_000.0;
        }
        (acc - 3.0) / 0.707
    }
}

pub struct Page {
    pub rgb: [Grid; 3],
    pub gray: Grid,
    /// Ground-truth ink coverage per pixel, 0.0..=1.0.
    pub ink: Grid,
    /// The lit paper luminance a perfect background estimator would recover.
    pub paper: Grid,
}

impl Page {
    pub fn ink_pixels(&self) -> Vec<usize> {
        (0..self.ink.len()).filter(|&i| self.ink.data()[i] > 0.5).collect()
    }
    pub fn paper_pixels(&self) -> Vec<usize> {
        (0..self.ink.len()).filter(|&i| self.ink.data()[i] < 0.01).collect()
    }
}

pub struct PageSpec {
    pub width: usize,
    pub height: usize,
    /// How much darker the far corner is, 0.0..1.0.
    pub gradient: f32,
    pub stroke_width: f32,
    pub noise: f32,
    pub ink_rgb: [f32; 3],
    pub seed: u64,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self {
            width: 256,
            height: 192,
            gradient: 0.45,
            stroke_width: 3.0,
            noise: 0.0,
            ink_rgb: [0.10, 0.13, 0.42],
            seed: 7,
        }
    }
}

fn illumination(width: usize, height: usize, strength: f32) -> Grid {
    let mut g = Grid::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let fy = y as f32 / (height.max(2) - 1) as f32;
            let fx = x as f32 / (width.max(2) - 1) as f32;
            g.set(x, y, 1.0 - strength * (fx + fy) / 2.0);
        }
    }
    g
}

/// Stamp an antialiased line segment of the given half-width into `cov`.
fn stamp_segment(cov: &mut Grid, a: (f32, f32), b: (f32, f32), half: f32) {
    let (w, h) = (cov.width(), cov.height());
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    let pad = half + 2.0;
    let x0 = (a.0.min(b.0) - pad).max(0.0) as usize;
    let x1 = ((a.0.max(b.0) + pad) as usize).min(w - 1);
    let y0 = (a.1.min(b.1) - pad).max(0.0) as usize;
    let y1 = ((a.1.max(b.1) + pad) as usize).min(h - 1);

    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32, y as f32);
            let t = (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0);
            let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
            let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            // 1px linear ramp at the edge, so strokes are antialiased like real ink.
            let c = ((half + 0.5 - dist).clamp(0.0, 1.0)).max(0.0);
            if c > cov.get(x, y) {
                cov.set(x, y, c);
            }
        }
    }
}

/// Three rows of handwriting-like squiggles, returned as fractional coverage.
pub fn ink_coverage(width: usize, height: usize, stroke_width: f32, seed: u64) -> Grid {
    let mut cov = Grid::new(width, height);
    let mut rng = Rng::new(seed);
    let half = stroke_width / 2.0;
    for row in 0..3 {
        let base = height as f32 * (0.22 + 0.27 * row as f32);
        let wobble = (height / 22).max(2) as i64;
        let mut prev: Option<(f32, f32)> = None;
        for i in 0..=10 {
            let x = width as f32 * (0.08 + 0.84 * i as f32 / 10.0);
            let y = base + rng.jitter(wobble) as f32;
            if let Some(p) = prev {
                stamp_segment(&mut cov, p, (x, y), half);
            }
            prev = Some((x, y));
        }
    }
    cov
}

pub fn make_page(spec: &PageSpec) -> Page {
    let (w, h) = (spec.width, spec.height);
    let field = illumination(w, h, spec.gradient);
    let paper = field.map(|v| PAPER * v);
    let ink = ink_coverage(w, h, spec.stroke_width, spec.seed);

    let mut rng = Rng::new(spec.seed ^ 0x9E37_79B9);
    let mut rgb = [Grid::new(w, h), Grid::new(w, h), Grid::new(w, h)];
    let mut gray = Grid::new(w, h);

    for i in 0..w * h {
        let a = ink.data()[i];
        let lit = field.data()[i];
        let bg = paper.data()[i];
        let mut sum = 0.0;
        for c in 0..3 {
            let fg = spec.ink_rgb[c] * lit;
            let mut v = a * fg + (1.0 - a) * bg;
            if spec.noise > 0.0 {
                v += rng.normal() * spec.noise;
            }
            let v = v.clamp(0.0, 1.0);
            rgb[c].data_mut()[i] = v;
            sum += v;
        }
        gray.data_mut()[i] = sum / 3.0;
    }

    Page { rgb, gray, ink, paper }
}

/// Intersection-over-union between a predicted mask and ground-truth coverage.
pub fn iou(predicted: &[bool], truth: &Grid, truth_at: f32) -> f32 {
    let mut inter = 0usize;
    let mut union = 0usize;
    for i in 0..predicted.len() {
        let t = truth.data()[i] > truth_at;
        let p = predicted[i];
        if p && t { inter += 1; }
        if p || t { union += 1; }
    }
    if union == 0 { return 1.0; }
    inter as f32 / union as f32
}
