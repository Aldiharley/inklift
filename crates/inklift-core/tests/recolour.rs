mod common;

use common::{PageSpec, make_page};
use inklift_core::{Options, extract, parse_ink_color};

#[test]
fn hex_colours_parse_in_the_forms_people_actually_type() {
    for (text, expect) in [
        ("#1E266B", [0x1E, 0x26, 0x6B]),
        ("1E266B", [0x1E, 0x26, 0x6B]),
        ("#1e266b", [0x1E, 0x26, 0x6B]),
        ("#fff", [0xFF, 0xFF, 0xFF]),
        ("#08F", [0x00, 0x88, 0xFF]),
        ("  #1E266B  ", [0x1E, 0x26, 0x6B]),
    ] {
        let c = parse_ink_color(text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
        for ch in 0..3 {
            assert!(
                (c[ch] - expect[ch] as f32 / 255.0).abs() < 1e-5,
                "{text:?} channel {ch} came back {:.4}",
                c[ch]
            );
        }
    }
}

#[test]
fn a_few_names_are_accepted_because_nobody_remembers_hex_for_black() {
    assert_eq!(parse_ink_color("black").unwrap(), [0.0, 0.0, 0.0]);
    assert_eq!(parse_ink_color("white").unwrap(), [1.0, 1.0, 1.0]);
    assert!(parse_ink_color("WHITE").is_ok(), "case should not matter");
}

#[test]
fn nonsense_is_rejected_with_the_expected_format_named() {
    for bad in ["", "#", "#12", "#12345", "#gggggg", "chartreuse", "0,0,0"] {
        let err = parse_ink_color(bad)
            .unwrap_err()
            .to_lowercase();
        assert!(err.contains("#rrggbb"), "{bad:?} error should name the format: {err}");
    }
}

/// The whole point: recolouring is a colour swap, never a re-extraction.
/// Every alpha value must survive untouched.
#[test]
fn recolouring_preserves_the_alpha_channel_exactly() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });
    let original = extract(&page.rgb, &Options::default());
    let before: Vec<f32> = original.alpha().data().to_vec();

    let recoloured = original.with_ink_color([1.0, 1.0, 1.0]);

    assert_eq!(recoloured.ink_color(), [1.0, 1.0, 1.0]);
    assert_eq!(
        recoloured.alpha().data(),
        &before[..],
        "alpha must be bit-identical after a recolour"
    );
}

#[test]
fn recolouring_changes_what_the_rgba_export_paints() {
    let page = make_page(&PageSpec { stroke_width: 5.0, ..Default::default() });
    let original = extract(&page.rgb, &Options::default());
    let white = original.clone().with_ink_color([1.0, 1.0, 1.0]);

    let a = original.to_rgba8();
    let b = white.to_rgba8();

    // the most solidly inked pixel, whatever its absolute opacity
    let i = (0..original.alpha().len())
        .max_by(|&a, &b| {
            original.alpha().data()[a]
                .partial_cmp(&original.alpha().data()[b])
                .unwrap_or(core::cmp::Ordering::Equal)
        })
        .expect("fixture should have pixels");
    assert!(original.alpha().data()[i] > 0.5, "fixture should have real ink");
    assert_eq!(&b[i * 4..i * 4 + 3], &[255, 255, 255], "should now paint white");
    assert_ne!(&a[i * 4..i * 4 + 3], &b[i * 4..i * 4 + 3]);
    assert_eq!(a[i * 4 + 3], b[i * 4 + 3], "alpha byte unchanged");
}

#[test]
fn a_colour_out_of_range_is_clamped_rather_than_wrapping() {
    let page = make_page(&PageSpec::default());
    let r = extract(&page.rgb, &Options::default()).with_ink_color([2.0, -1.0, 0.5]);
    assert_eq!(r.ink_color(), [1.0, 0.0, 0.5]);
}
