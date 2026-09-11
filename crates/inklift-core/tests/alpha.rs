mod common;

use common::{Page, PageSpec, make_page};
use inklift_core::{
    Grid, despeckle, estimate_background, ink_opacity, normalize_illumination, sauvola,
};

fn alpha_for(page: &Page) -> Grid {
    let bg = estimate_background(&page.gray, None);
    let flat = normalize_illumination(&page.gray, &bg);
    let mask = despeckle(&sauvola(&flat, 12, 0.2), 8);
    ink_opacity(&flat, &mask, 1)
}

fn mean_where(alpha: &Grid, truth: &Grid, lo: f32, hi: f32) -> f32 {
    let picked: Vec<f32> = (0..alpha.len())
        .filter(|&i| truth.data()[i] >= lo && truth.data()[i] < hi)
        .map(|i| alpha.data()[i])
        .collect();
    assert!(!picked.is_empty(), "no pixels with coverage in {lo}..{hi}");
    picked.iter().sum::<f32>() / picked.len() as f32
}

#[test]
fn opacity_stays_within_the_unit_range() {
    let alpha = alpha_for(&make_page(&PageSpec { noise: 0.02, ..Default::default() }));

    assert!(
        alpha.data().iter().all(|&v| (0.0..=1.0).contains(&v)),
        "alpha escaped [0,1]"
    );
}

#[test]
fn bare_paper_is_fully_transparent() {
    let page = make_page(&PageSpec::default());

    let alpha = alpha_for(&page);

    let m = mean_where(&alpha, &page.ink, 0.0, 0.01);
    assert!(m < 0.02, "paper carried {m:.4} opacity");
}

#[test]
fn stroke_cores_are_nearly_opaque() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });

    let alpha = alpha_for(&page);

    let m = mean_where(&alpha, &page.ink, 0.99, 1.01);
    assert!(m > 0.70, "stroke cores only reached {m:.4} opacity");
}

/// The property that separates this from a binary mask: partially covered
/// pixels at the edge of a stroke must come out partially opaque.
#[test]
fn opacity_tracks_partial_stroke_coverage() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });

    let alpha = alpha_for(&page);

    let empty = mean_where(&alpha, &page.ink, 0.0, 0.01);
    let edge = mean_where(&alpha, &page.ink, 0.3, 0.7);
    let core = mean_where(&alpha, &page.ink, 0.99, 1.01);

    assert!(
        empty < edge && edge < core,
        "expected a gradient, got paper={empty:.3} edge={edge:.3} core={core:.3}"
    );
    assert!(
        (0.1..0.9).contains(&edge),
        "edge pixels came out effectively binary at {edge:.3}"
    );
}
