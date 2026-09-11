use crate::mask::Mask;

/// Drop connected components smaller than `min_area` pixels.
///
/// Uses eight-connectivity, so a diagonal hairline counts as one stroke rather
/// than a row of unrelated specks. Iterative flood fill — no recursion, so a
/// page-sized blob cannot blow the stack.
pub fn despeckle(mask: &Mask, min_area: usize) -> Mask {
    if min_area <= 1 || mask.is_empty() {
        return mask.clone();
    }
    let (w, h) = (mask.width(), mask.height());
    let mut out = Mask::new(w, h);
    let mut seen = vec![false; mask.len()];
    let mut component: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for start in 0..mask.len() {
        if seen[start] || !mask.data()[start] {
            continue;
        }
        component.clear();
        stack.clear();
        stack.push(start);
        seen[start] = true;

        while let Some(i) = stack.pop() {
            component.push(i);
            let (x, y) = (i % w, i / w);
            let x0 = x.saturating_sub(1);
            let x1 = (x + 1).min(w - 1);
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(h - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    let j = ny * w + nx;
                    if !seen[j] && mask.data()[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }

        if component.len() >= min_area {
            for &i in &component {
                out.data_mut()[i] = true;
            }
        }
    }
    out
}
