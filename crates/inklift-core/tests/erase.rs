//! Hand erasing: the brush a user paints over marks the pipeline rightly kept
//! as ink but they do not want. What matters is where it erases, how its edge
//! falls off, and that the proxy preview and the full export agree — the app
//! previews on a downscaled proxy and exports at full size from the same list.

mod common;

use common::{PageSpec, make_page};
use inklift_core::{Grid, Options, Stroke, extract, keep_mask};

fn stroke(points: &[[f32; 2]], radius: f32, softness: f32) -> Stroke {
    Stroke::new(points.to_vec(), radius, softness).expect("a valid stroke")
}

/// Distance from pixel (x, y)'s centre to a point.
fn dist(x: usize, y: usize, p: [f32; 2]) -> f32 {
    let (dx, dy) = (x as f32 + 0.5 - p[0], y as f32 + 0.5 - p[1]);
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn a_hard_brush_erases_inside_its_radius_and_nothing_beyond() {
    let c = [50.5, 50.5];
    let keep = keep_mask(100, 100, &[stroke(&[c], 10.0, 0.0)], 1.0);
    for y in 0..100 {
        for x in 0..100 {
            let d = dist(x, y, c);
            let k = keep.get(x, y);
            if d <= 9.0 {
                assert_eq!(k, 0.0, "({x},{y}) at {d:.2} px should be erased");
            }
            if d >= 10.0 {
                assert_eq!(k, 1.0, "({x},{y}) at {d:.2} px should be untouched");
            }
        }
    }
}

#[test]
fn softness_fades_the_edge_smoothly_out_to_the_radius() {
    let c = [50.5, 50.5];
    let keep = keep_mask(100, 100, &[stroke(&[c], 20.0, 1.0)], 1.0);
    let row: Vec<f32> = (50..80).map(|x| keep.get(x, 50)).collect();
    assert_eq!(row[0], 0.0, "the centre of the brush erases fully");
    for w in row.windows(2) {
        assert!(w[1] >= w[0], "keep must not fall moving outward: {row:?}");
    }
    let mid = keep.get(60, 50);
    assert!(mid > 0.05 && mid < 0.95, "halfway out a fully soft brush erases partly, got {mid}");
    assert_eq!(keep.get(70, 50), 1.0, "at the radius the brush has faded out");
}

#[test]
fn a_fast_drag_leaves_no_gap_between_its_points() {
    // Two points 80 px apart, as a quick flick of the mouse reports them.
    let keep = keep_mask(100, 100, &[stroke(&[[10.5, 50.5], [90.5, 50.5]], 3.0, 0.0)], 1.0);
    for x in 10..=90 {
        assert_eq!(keep.get(x, 50), 0.0, "gap at x = {x}");
    }
}

#[test]
fn a_second_soft_pass_erases_more_than_the_first() {
    let s = stroke(&[[50.5, 50.5]], 20.0, 1.0);
    let once = keep_mask(100, 100, std::slice::from_ref(&s), 1.0).get(62, 50);
    let twice = keep_mask(100, 100, &[s.clone(), s], 1.0).get(62, 50);
    assert!(once > 0.0 && once < 1.0, "the test point must sit in the soft edge, got {once}");
    assert!((twice - once * once).abs() < 1e-6, "passes compound: {once} then {twice}");
}

#[test]
fn a_stroke_crossing_itself_does_not_erase_twice() {
    let there = keep_mask(100, 100, &[stroke(&[[40.5, 50.5], [60.5, 50.5]], 12.0, 1.0)], 1.0);
    let back = keep_mask(
        100,
        100,
        &[stroke(&[[40.5, 50.5], [60.5, 50.5], [40.5, 50.5]], 12.0, 1.0)],
        1.0,
    );
    assert_eq!(there.data(), back.data());
}

#[test]
fn the_proxy_mask_agrees_with_the_full_one() {
    let s = stroke(&[[40.0, 100.0], [160.0, 110.0]], 20.0, 0.5);
    let full = keep_mask(200, 200, std::slice::from_ref(&s), 1.0);
    let half = keep_mask(100, 100, std::slice::from_ref(&s), 0.5);
    let mut worst = 0.0f32;
    for y in 0..100 {
        for x in 0..100 {
            let avg = (full.get(2 * x, 2 * y)
                + full.get(2 * x + 1, 2 * y)
                + full.get(2 * x, 2 * y + 1)
                + full.get(2 * x + 1, 2 * y + 1))
                / 4.0;
            worst = worst.max((half.get(x, y) - avg).abs());
        }
    }
    assert!(worst < 0.1, "proxy and full masks differ by up to {worst}");
}

#[test]
fn no_strokes_keep_everything() {
    let keep = keep_mask(40, 30, &[], 1.0);
    assert!(keep.data().iter().all(|&k| k == 1.0));
}

#[test]
fn strokes_off_the_image_are_ignored() {
    let keep = keep_mask(40, 30, &[stroke(&[[-100.0, -100.0], [-50.0, -60.0]], 10.0, 0.0)], 1.0);
    assert!(keep.data().iter().all(|&k| k == 1.0));
}

#[test]
fn a_stroke_is_refused_when_it_cannot_mean_anything() {
    assert!(Stroke::new(vec![], 10.0, 0.5).is_err(), "no points");
    assert!(Stroke::new(vec![[f32::NAN, 1.0]], 10.0, 0.5).is_err(), "NaN point");
    assert!(Stroke::new(vec![[1.0, f32::INFINITY]], 10.0, 0.5).is_err(), "infinite point");
    for r in [0.5, 400.5, f32::NAN] {
        assert!(Stroke::new(vec![[1.0, 1.0]], r, 0.5).is_err(), "radius {r}");
    }
    for s in [-0.1, 1.1, f32::NAN] {
        assert!(Stroke::new(vec![[1.0, 1.0]], 10.0, s).is_err(), "softness {s}");
    }
    for (r, s) in [(1.0, 0.0), (400.0, 1.0)] {
        assert!(Stroke::new(vec![[1.0, 1.0]], r, s).is_ok(), "radius {r} softness {s}");
    }
}

#[test]
fn erasing_removes_ink_and_leaves_the_rest_as_it_was() {
    let page = make_page(&PageSpec::default());
    let before = extract(&page.rgb, &Options::default());
    let (w, h) = (before.width(), before.height());
    let mut keep = Grid::filled(w, h, 1.0);
    for y in 0..h {
        for x in 0..w / 2 {
            keep.set(x, y, 0.0);
        }
    }
    let left_ink: f32 = (0..h)
        .flat_map(|y| (0..w / 2).map(move |x| (x, y)))
        .map(|(x, y)| before.alpha().get(x, y))
        .sum();
    assert!(left_ink > 10.0, "the fixture must have ink on the left to erase");

    let after = before.clone().erased(&keep);
    for y in 0..h {
        for x in 0..w {
            let want = if x < w / 2 { 0.0 } else { before.alpha().get(x, y) };
            assert_eq!(after.alpha().get(x, y), want, "({x},{y})");
        }
    }
    assert_eq!(after.ink_color(), before.ink_color(), "erasing must not shift the pen colour");
    assert!(after.coverage() < before.coverage());
}

#[test]
#[should_panic(expected = "erase mask")]
fn a_mask_of_the_wrong_size_is_a_bug_not_a_silent_no_op() {
    let page = make_page(&PageSpec::default());
    let result = extract(&page.rgb, &Options::default());
    let _ = result.erased(&Grid::filled(3, 3, 1.0));
}
