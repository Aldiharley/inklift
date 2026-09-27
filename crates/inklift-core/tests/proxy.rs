mod common;

use common::{PageSpec, make_page};
use inklift_core::{Options, extract};

/// Live retune previews run on a downscaled proxy. Every pixel-denominated
/// parameter has to be scaled with it or the preview stops predicting the
/// result — the worst possible failure for a tuning UI, because it is
/// confidently wrong rather than visibly broken.
#[test]
fn radii_scale_linearly_and_area_scales_by_the_square() {
    let full = Options { sauvola_radius: 12, feather: 2, min_area: 40,
                         background_radius: Some(16), ..Default::default() };

    let half = full.scaled_for_proxy(0.5);

    assert_eq!(half.sauvola_radius, 6, "a radius is a length");
    assert_eq!(half.feather, 1, "so is a feather");
    assert_eq!(half.background_radius, Some(8));
    assert_eq!(half.min_area, 10, "an area scales by s squared, not s");
}

#[test]
fn a_scale_of_one_changes_nothing() {
    let o = Options { sauvola_radius: 12, feather: 2, min_area: 40,
                      background_radius: Some(16), sauvola_k: 0.17,
                      invert: true, ..Default::default() };
    let same = o.clone().scaled_for_proxy(1.0);
    assert_eq!(same.sauvola_radius, o.sauvola_radius);
    assert_eq!(same.min_area, o.min_area);
    assert_eq!(same.background_radius, o.background_radius);
}

/// k and invert describe the image, not its size.
#[test]
fn dimensionless_settings_are_left_alone() {
    let o = Options { sauvola_k: 0.17, invert: true, ..Default::default() };
    let s = o.scaled_for_proxy(0.25);
    assert!((s.sauvola_k - 0.17).abs() < 1e-6);
    assert!(s.invert);
}

/// Scaling must never produce a zero radius: a zero-radius filter is a no-op
/// and the preview would show an unfiltered image.
#[test]
fn radii_never_collapse_to_zero_however_small_the_proxy() {
    let o = Options { sauvola_radius: 3, feather: 1, min_area: 8,
                      background_radius: Some(7), ..Default::default() };
    let tiny = o.scaled_for_proxy(0.05);
    assert!(tiny.sauvola_radius >= 1);
    assert!(tiny.min_area >= 1);
    assert!(tiny.background_radius.unwrap() >= 1);
}

#[test]
fn an_absent_background_radius_stays_absent() {
    let o = Options { background_radius: None, ..Default::default() };
    assert_eq!(o.scaled_for_proxy(0.5).background_radius, None);
}

/// The point of all of it: a proxy preview should predict the full-resolution
/// result. Same page at half size with scaled options should report closely
/// matching ink coverage.
#[test]
fn a_scaled_proxy_predicts_the_full_resolution_coverage() {
    let page = make_page(&PageSpec { width: 512, height: 384, stroke_width: 6.0,
                                     ..Default::default() });
    let opts = Options { sauvola_radius: 12, min_area: 24, ..Default::default() };
    let full = extract(&page.rgb, &opts);

    let half_page = make_page(&PageSpec { width: 256, height: 192, stroke_width: 3.0,
                                          ..Default::default() });
    let proxy = extract(&half_page.rgb, &opts.clone().scaled_for_proxy(0.5));

    let delta = (full.coverage() - proxy.coverage()).abs();
    assert!(delta < 0.02,
        "proxy said {:.3}% and full said {:.3}% — the preview would lie",
        proxy.coverage()*100.0, full.coverage()*100.0);
}
