use crate::grid::Grid;

/// Separable rank filter over a `2 * radius + 1` square window, clamping at
/// the border. `pick` folds two samples into the surviving one.
fn separable(grid: &Grid, radius: usize, init: f32, pick: fn(f32, f32) -> f32) -> Grid {
    if radius == 0 || grid.is_empty() {
        return grid.clone();
    }
    let (w, h) = (grid.width(), grid.height());

    let mut horizontal = Grid::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let lo = x.saturating_sub(radius);
            let hi = (x + radius).min(w - 1);
            let mut acc = init;
            for xx in lo..=hi {
                acc = pick(acc, grid.get(xx, y));
            }
            horizontal.set(x, y, acc);
        }
    }

    let mut out = Grid::new(w, h);
    for y in 0..h {
        let lo = y.saturating_sub(radius);
        let hi = (y + radius).min(h - 1);
        for x in 0..w {
            let mut acc = init;
            for yy in lo..=hi {
                acc = pick(acc, horizontal.get(x, yy));
            }
            out.set(x, y, acc);
        }
    }
    out
}

/// Grey dilation. On a light ground this swallows dark detail narrower than the window.
pub fn max_filter(grid: &Grid, radius: usize) -> Grid {
    separable(grid, radius, f32::NEG_INFINITY, f32::max)
}

/// Grey erosion, the dual of [`max_filter`].
pub fn min_filter(grid: &Grid, radius: usize) -> Grid {
    separable(grid, radius, f32::INFINITY, f32::min)
}

/// Separable box blur with a clamped border, via prefix sums.
pub fn box_blur(grid: &Grid, radius: usize) -> Grid {
    if radius == 0 || grid.is_empty() {
        return grid.clone();
    }
    let (w, h) = (grid.width(), grid.height());

    let mut horizontal = Grid::new(w, h);
    let mut prefix = vec![0.0f32; w + 1];
    for y in 0..h {
        for x in 0..w {
            prefix[x + 1] = prefix[x] + grid.get(x, y);
        }
        for x in 0..w {
            let lo = x.saturating_sub(radius);
            let hi = (x + radius).min(w - 1);
            horizontal.set(x, y, (prefix[hi + 1] - prefix[lo]) / (hi - lo + 1) as f32);
        }
    }

    let mut out = Grid::new(w, h);
    let mut column = vec![0.0f32; h + 1];
    for x in 0..w {
        for y in 0..h {
            column[y + 1] = column[y] + horizontal.get(x, y);
        }
        for y in 0..h {
            let lo = y.saturating_sub(radius);
            let hi = (y + radius).min(h - 1);
            out.set(x, y, (column[hi + 1] - column[lo]) / (hi - lo + 1) as f32);
        }
    }
    out
}

/// Three box passes, which approximates a Gaussian closely enough for an
/// illumination field and stays O(n) per pass.
pub fn smooth(grid: &Grid, radius: usize) -> Grid {
    let a = box_blur(grid, radius);
    let b = box_blur(&a, radius);
    box_blur(&b, radius)
}

/// Grey closing: dilate then erode. Removes dark structures narrower than the
/// window while leaving larger shapes and smooth gradients essentially intact.
pub fn grey_close(grid: &Grid, radius: usize) -> Grid {
    min_filter(&max_filter(grid, radius), radius)
}
