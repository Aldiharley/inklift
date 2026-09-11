mod common;

use common::{PageSpec, make_page};
use inklift_core::estimate_background;

/// Under a stroke, the estimate must report the paper, not the ink.
#[test]
fn background_estimate_sees_through_strokes() {
    let page = make_page(&PageSpec::default());

    let bg = estimate_background(&page.gray, None);

    let idx = page.ink_pixels();
    assert!(!idx.is_empty(), "fixture drew no ink");
    let err: f32 = idx
        .iter()
        .map(|&i| (bg.data()[i] - page.paper.data()[i]).abs())
        .sum::<f32>()
        / idx.len() as f32;
    assert!(err < 0.05, "mean error under strokes was {err:.4}");
}

/// And on bare paper it must track the lighting gradient closely.
#[test]
fn background_estimate_follows_the_lighting_gradient() {
    let page = make_page(&PageSpec { gradient: 0.5, ..Default::default() });

    let bg = estimate_background(&page.gray, None);

    let idx = page.paper_pixels();
    let err: f32 = idx
        .iter()
        .map(|&i| (bg.data()[i] - page.paper.data()[i]).abs())
        .sum::<f32>()
        / idx.len() as f32;
    assert!(err < 0.03, "mean error on paper was {err:.4}");
}
