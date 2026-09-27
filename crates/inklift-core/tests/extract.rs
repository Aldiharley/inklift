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

/// Invert a page in place: what a dark-mode screen capture looks like, where
/// the ink is lighter than what it sits on.
fn inverted(rgb: &[inklift_core::Grid; 3]) -> [inklift_core::Grid; 3] {
    [
        rgb[0].map(|v| 1.0 - v),
        rgb[1].map(|v| 1.0 - v),
        rgb[2].map(|v| 1.0 - v),
    ]
}

#[test]
fn inversion_is_off_by_default() {
    assert!(!Options::default().invert);
}

/// Light ink on a dark ground is the normal case for a screenshot of a
/// dark-themed application, and the plain pipeline cannot see it at all.
#[test]
fn light_ink_on_a_dark_ground_needs_the_invert_flag() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });
    let dark = inverted(&page.rgb);

    let without = extract(&dark, &Options::default());
    let with = extract(&dark, &Options { invert: true, ..Default::default() });

    let ink = page.ink_pixels();
    let mean = |r: &inklift_core::Extraction| {
        ink.iter().map(|&i| r.alpha().data()[i]).sum::<f32>() / ink.len() as f32
    };
    assert!(mean(&without) < 0.05, "the plain pipeline should find no ink here");
    assert!(mean(&with) > 0.60, "with --invert the strokes should come back");
}

#[test]
fn inverting_a_normal_page_finds_nothing() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });

    let result = extract(&page.rgb, &Options { invert: true, ..Default::default() });

    assert!(result.coverage() < 0.02, "inverting an ordinary page should find no ink");
}

/// Inverting a dark-ground capture should recover the same strokes the plain
/// pipeline finds on the equivalent light-ground page.
#[test]
fn inverted_extraction_matches_the_equivalent_upright_page() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });
    let dark = inverted(&page.rgb);

    let upright = extract(&page.rgb, &Options::default());
    let flipped = extract(&dark, &Options { invert: true, ..Default::default() });

    let worst = (0..upright.alpha().len())
        .map(|i| (upright.alpha().data()[i] - flipped.alpha().data()[i]).abs())
        .fold(0.0f32, f32::max);
    assert!(worst < 0.02, "worst alpha difference was {worst:.4}");
}

/// The ink is rendered dark so that both exports stay usable; on a white
/// background a light ink would simply be invisible.
#[test]
fn inverted_ink_is_reported_as_a_dark_colour() {
    let page = make_page(&PageSpec { stroke_width: 6.0, ink_rgb: [0.9, 0.9, 0.9], ..Default::default() });
    let dark = inverted(&page.rgb);

    let c = extract(&dark, &Options { invert: true, ..Default::default() }).ink_color();

    assert!(
        inklift_core::luma(c) < 0.35,
        "expected a dark pen for the white-on-dark case, got luma {:.3}",
        inklift_core::luma(c)
    );
}

#[test]
fn a_dark_ground_is_detectable_so_the_flag_can_be_suggested() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });

    assert!(!inklift_core::looks_inverted(&page.rgb), "ordinary paper is not inverted");
    assert!(inklift_core::looks_inverted(&inverted(&page.rgb)), "a dark ground is");
}

#[test]
fn detection_uses_the_bulk_of_the_image_not_a_few_bright_pixels() {
    // Mostly dark with a small very bright patch: still a dark ground.
    let mut rgb = [
        inklift_core::Grid::filled(100, 100, 0.08),
        inklift_core::Grid::filled(100, 100, 0.08),
        inklift_core::Grid::filled(100, 100, 0.08),
    ];
    for y in 0..20 {
        for x in 0..20 {
            for c in 0..3 {
                rgb[c].set(x, y, 1.0);
            }
        }
    }
    assert!(inklift_core::looks_inverted(&rgb));
}
