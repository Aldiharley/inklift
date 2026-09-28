//! Hand erasing: brush strokes the user paints over marks the pipeline rightly
//! kept as ink but they do not want — a cursor, a UI rule, a neighbour's word.
//!
//! Strokes are stored in source-image pixels and rasterised on demand into a
//! keep factor per pixel. One list then serves both the downscaled proxy the
//! app previews on and the full-resolution export, so the two cannot drift.

use crate::grid::Grid;

/// One press-drag-release of the eraser.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    points: Vec<[f32; 2]>,
    radius: f32,
    softness: f32,
}

impl Stroke {
    /// Points are in source-image pixels, where pixel `(x, y)` covers
    /// `x..x+1, y..y+1`; `radius` is in the same units; `softness` runs from a
    /// crisp edge at 0 to a fall-off starting at the centre line at 1.
    pub fn new(points: Vec<[f32; 2]>, radius: f32, softness: f32) -> Result<Stroke, String> {
        if points.is_empty() {
            return Err("an eraser stroke needs at least one point".into());
        }
        if points.iter().flatten().any(|v| !v.is_finite()) {
            return Err("an eraser stroke has a point that is not a number".into());
        }
        if !(1.0..=400.0).contains(&radius) {
            return Err(format!("eraser radius {radius} is outside 1 to 400 pixels"));
        }
        if !(0.0..=1.0).contains(&softness) {
            return Err(format!("eraser softness {softness} is outside 0 to 1"));
        }
        Ok(Stroke { points, radius, softness })
    }
}

/// Erase strength at distance `d` from a stroke's centre line.
fn coverage(d: f32, radius: f32, softness: f32) -> f32 {
    // Even a "hard" brush keeps one pixel of fall-off: a binary edge aliases,
    // and its stair-steps show against the smooth edges the pipeline produces.
    let inner = (radius * (1.0 - softness)).min(radius - 1.0).max(0.0);
    if d <= inner {
        1.0
    } else if d >= radius {
        0.0
    } else {
        let t = (radius - d) / (radius - inner);
        t * t * (3.0 - 2.0 * t)
    }
}

fn distance_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    (cx * cx + cy * cy).sqrt()
}

/// Per-pixel factor in [0, 1] to multiply alpha by: 1 keeps, 0 erases.
///
/// `scale` maps the strokes' source pixels onto this grid — 1 for the full
/// image, the proxy's factor for a preview.
pub fn keep_mask(width: usize, height: usize, strokes: &[Stroke], scale: f32) -> Grid {
    let mut keep = Grid::filled(width, height, 1.0);
    for stroke in strokes {
        let pts: Vec<[f32; 2]> = stroke.points.iter().map(|p| [p[0] * scale, p[1] * scale]).collect();
        let radius = stroke.radius * scale;

        // Only the stroke's own neighbourhood is visited, so a long session on
        // a large capture stays proportional to what was painted.
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &pts {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        // Float-to-usize casts saturate, so a stroke wholly off the image
        // yields an empty box rather than a wrap-around.
        let bx0 = (x0 - radius).floor().max(0.0) as usize;
        let by0 = (y0 - radius).floor().max(0.0) as usize;
        let bx1 = (x1 + radius).ceil().min(width as f32) as usize;
        let by1 = (y1 + radius).ceil().min(height as f32) as usize;
        if bx0 >= bx1 || by0 >= by1 {
            continue;
        }
        let bw = bx1 - bx0;

        // Within one stroke take the strongest segment, so a stroke that
        // crosses itself does not erase twice where it overlaps.
        let mut cov = vec![0.0f32; bw * (by1 - by0)];
        let segments: Vec<([f32; 2], [f32; 2])> = if pts.len() == 1 {
            vec![(pts[0], pts[0])]
        } else {
            pts.windows(2).map(|w| (w[0], w[1])).collect()
        };
        for (a, b) in segments {
            let sx0 = (a[0].min(b[0]) - radius).floor().max(bx0 as f32) as usize;
            let sy0 = (a[1].min(b[1]) - radius).floor().max(by0 as f32) as usize;
            let sx1 = (a[0].max(b[0]) + radius).ceil().min(bx1 as f32) as usize;
            let sy1 = (a[1].max(b[1]) + radius).ceil().min(by1 as f32) as usize;
            for y in sy0..sy1 {
                for x in sx0..sx1 {
                    let d = distance_to_segment([x as f32 + 0.5, y as f32 + 0.5], a, b);
                    let c = coverage(d, radius, stroke.softness);
                    let i = (y - by0) * bw + (x - bx0);
                    if c > cov[i] {
                        cov[i] = c;
                    }
                }
            }
        }

        // Across strokes the factors multiply: a second soft pass erases more,
        // as a physical eraser does.
        for y in by0..by1 {
            for x in bx0..bx1 {
                let c = cov[(y - by0) * bw + (x - bx0)];
                if c > 0.0 {
                    let k = keep.get(x, y);
                    keep.set(x, y, k * (1.0 - c));
                }
            }
        }
    }
    keep
}
