mod common;

use common::{PageSpec, make_page};
use inklift_core::{estimate_background, normalize_illumination};

fn mean(values: impl Iterator<Item = f32>) -> f32 {
    let v: Vec<f32> = values.collect();
    v.iter().sum::<f32>() / v.len() as f32
}

fn std_dev(values: &[f32]) -> f32 {
    let m = values.iter().sum::<f32>() / values.len() as f32;
    (values.iter().map(|v| (v - m).powi(2)).sum::<f32>() / values.len() as f32).sqrt()
}

#[test]
fn normalization_flattens_lighting_across_the_page() {
    let page = make_page(&PageSpec { gradient: 0.5, ..Default::default() });
    let paper_idx = page.paper_pixels();
    let before: Vec<f32> = paper_idx.iter().map(|&i| page.gray.data()[i]).collect();

    let bg = estimate_background(&page.gray, None);
    let flat = normalize_illumination(&page.gray, &bg);

    let after: Vec<f32> = paper_idx.iter().map(|&i| flat.data()[i]).collect();
    let m = mean(after.iter().copied());
    assert!((m - 1.0).abs() < 0.02, "paper settled at {m:.4}, not 1.0");
    assert!(
        std_dev(&after) < std_dev(&before) / 5.0,
        "spread {:.4} was not much tighter than {:.4}",
        std_dev(&after),
        std_dev(&before)
    );
}

#[test]
fn normalization_leaves_ink_dark() {
    let page = make_page(&PageSpec::default());

    let bg = estimate_background(&page.gray, None);
    let flat = normalize_illumination(&page.gray, &bg);

    let m = mean(page.ink_pixels().iter().map(|&i| flat.data()[i]));
    assert!(m < 0.5, "ink normalised to {m:.4}, too bright");
}

/// Dividing by a perfectly flat background must not disturb the image.
#[test]
fn normalization_against_flat_paper_is_a_no_op() {
    let page = make_page(&PageSpec { gradient: 0.0, ..Default::default() });
    let flat_bg = inklift_core::Grid::filled(page.gray.width(), page.gray.height(), 1.0);

    let out = normalize_illumination(&page.gray, &flat_bg);

    for i in 0..out.len() {
        assert!((out.data()[i] - page.gray.data()[i]).abs() < 1e-6);
    }
}
