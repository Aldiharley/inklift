use inklift_core::{Grid, max_filter, min_filter};

/// A max filter (dilation on a light ground) should swallow any dark structure
/// narrower than its window. This is the primitive background estimation rests on.
#[test]
fn max_filter_erases_a_line_thinner_than_its_window() {
    let mut g = Grid::filled(21, 21, 1.0);
    for y in 0..21 {
        for x in 9..12 {
            g.set(x, y, 0.1);
        }
    }

    let out = max_filter(&g, 4); // 9px window against a 3px line

    assert!(
        out.data().iter().all(|&v| v > 0.99),
        "darkest surviving pixel was {}",
        out.data().iter().cloned().fold(f32::INFINITY, f32::min)
    );
}

#[test]
fn max_filter_keeps_a_line_wider_than_its_window() {
    let mut g = Grid::filled(21, 21, 1.0);
    for y in 0..21 {
        for x in 4..17 {
            g.set(x, y, 0.1);
        }
    }

    let out = max_filter(&g, 2); // 5px window against a 13px line

    assert!(out.get(10, 10) < 0.2, "centre of a wide line should survive");
}

#[test]
fn min_filter_is_the_dual_of_max_filter() {
    let mut g = Grid::filled(21, 21, 0.0);
    for y in 0..21 {
        for x in 9..12 {
            g.set(x, y, 0.9);
        }
    }

    let out = min_filter(&g, 4);

    assert!(out.data().iter().all(|&v| v < 0.01));
}
