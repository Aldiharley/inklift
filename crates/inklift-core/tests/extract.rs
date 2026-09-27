mod common;

use common::{PageSpec, make_page};
use inklift_core::{Options, extract, luma};

#[test]
fn estimated_ink_colour_matches_the_pen() {
    let page = make_page(&PageSpec { stroke_width: 6.0, ink_rgb: [0.10, 0.13, 0.42], ..Default::default() });

    let result = extract(&page.rgb, &Options::default());

    let c = result.ink_color();
    // Colour is reported relative to the paper, so undo the fixture's paper value.
    let expected = [0.10 / common::PAPER, 0.13 / common::PAPER, 0.42 / common::PAPER];
    for ch in 0..3 {
        assert!(
            (c[ch] - expected[ch]).abs() < 0.10,
            "channel {ch} came back {:.3}, expected about {:.3}",
            c[ch],
            expected[ch]
        );
    }
    assert!(c[2] > c[1] && c[1] > c[0], "blue pen did not stay blue: {c:?}");
}

#[test]
fn extraction_is_transparent_on_paper_and_opaque_on_ink() {
    let page = make_page(&PageSpec { stroke_width: 5.0, noise: 0.015, ..Default::default() });

    let result = extract(&page.rgb, &Options::default());

    let alpha = result.alpha();
    let paper: Vec<f32> = page.paper_pixels().iter().map(|&i| alpha.data()[i]).collect();
    let ink: Vec<f32> = page.ink_pixels().iter().map(|&i| alpha.data()[i]).collect();
    let pm = paper.iter().sum::<f32>() / paper.len() as f32;
    let im = ink.iter().sum::<f32>() / ink.len() as f32;

    assert!(pm < 0.03, "paper averaged {pm:.4} opacity");
    assert!(im > 0.60, "ink averaged only {im:.4} opacity");
}

#[test]
fn rgba_export_has_four_bytes_per_pixel_and_clear_paper() {
    let page = make_page(&PageSpec::default());
    let result = extract(&page.rgb, &Options::default());

    let rgba = result.to_rgba8();

    assert_eq!(rgba.len(), page.gray.len() * 4);
    for &i in page.paper_pixels().iter().take(500) {
        assert!(rgba[i * 4 + 3] < 8, "paper pixel was not transparent");
    }
}

/// The invariant the whole "ship white now, add transparency later" plan rests
/// on: the greyscale-on-white export must still carry the opacity, recoverable
/// to within quantisation error. Writing a hard binary would destroy this.
#[test]
fn alpha_survives_a_round_trip_through_the_white_background_export() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });
    let result = extract(&page.rgb, &Options::default());
    let ink_luma = luma(result.ink_color());

    let grey = result.to_gray_on_white8();
    let recovered = inklift_core::alpha_from_gray_on_white(&grey, ink_luma);

    let worst = (0..recovered.len())
        .map(|i| (recovered[i] - result.alpha().data()[i]).abs())
        .fold(0.0f32, f32::max);
    assert!(worst < 0.02, "worst round-trip error was {worst:.4}");
}

#[test]
fn a_blank_page_extracts_to_nothing() {
    let blank = [
        inklift_core::Grid::filled(64, 64, 0.9),
        inklift_core::Grid::filled(64, 64, 0.9),
        inklift_core::Grid::filled(64, 64, 0.9),
    ];

    let result = extract(&blank, &Options::default());

    assert!(result.coverage() < 0.01, "invented ink on a blank page");
}

/// Thin, softened strokes have very few fully-covered pixels: nearly every one
/// is part ink, part paper. A median over them lands halfway to the paper and
/// reports a pen far lighter than it is, which then caps how dark the white
/// composite can ever go. Real low-resolution captures look like this.
///
/// The stroke width here matters: at 2px with this much blur the true ink
/// colour is not present anywhere in the image and no estimator could recover
/// it. At 3px it is present, so failing is a real defect rather than physics.
#[test]
fn ink_colour_survives_thin_strokes_with_no_solid_core() {
    let page = make_page(&PageSpec {
        stroke_width: 3.0,
        blur: 1,
        ink_rgb: [0.08, 0.10, 0.12],
        ..Default::default()
    });

    let c = extract(&page.rgb, &Options::default()).ink_color();

    let expected = [0.08 / common::PAPER, 0.10 / common::PAPER, 0.12 / common::PAPER];
    for ch in 0..3 {
        assert!(
            (c[ch] - expected[ch]).abs() < 0.12,
            "channel {ch} came back {:.3}, expected about {:.3} - washed out toward paper",
            c[ch],
            expected[ch]
        );
    }
}
