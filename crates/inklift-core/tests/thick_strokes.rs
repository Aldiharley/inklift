//! Broad nib strokes, as in pointed-pen calligraphy: solid ink far wider than
//! anything the defaults were tuned on.
//!
//! Measured on a 2K photograph whose shades are ~40 px wide: the defaults left
//! a line of transparent pinholes down the middle of every thick stroke. Raising
//! the paper-estimate radius changed nothing (the default is already wider than
//! the stroke at that size); raising the Sauvola window to 30 removed them. The
//! desktop app could do neither: its "Thickest stroke" slider moved only the
//! paper radius, and the window was pinned at 12.

mod common;

use common::{PageSpec, make_page};
use inklift_core::{Options, extract, min_filter};

/// A calligraphy-sized page: 40 px strokes on a lit, lightly textured ground.
/// Large enough that the derived paper radius (30) exceeds the stroke
/// half-width (20), so only the thresholding can be at fault.
fn calligraphy() -> common::Page {
    make_page(&PageSpec {
        width: 960,
        height: 720,
        stroke_width: 40.0,
        noise: 0.01,
        ink_rgb: [0.08, 0.08, 0.10],
        ..Default::default()
    })
}

/// Pixels well inside a stroke: fully covered, and at least 4 px from any edge,
/// so the antialiased rim and the soft gate cannot blur the question.
fn interior(page: &common::Page) -> Vec<usize> {
    let core = min_filter(&page.ink, 4);
    (0..core.len()).filter(|&i| core.data()[i] > 0.99).collect()
}

fn holes(page: &common::Page, options: &Options) -> usize {
    let inside = interior(page);
    assert!(inside.len() > 10_000, "fixture has no stroke interior to test");
    let result = extract(&page.rgb, options);
    inside.iter().filter(|&&i| result.alpha().data()[i] < 0.5).count()
}

/// The reach problem, pinned so nobody "fixes" it by widening the default:
/// a window of 12 spans a stroke's middle only while the stroke is under ~24 px.
/// Past that, a window centred inside the stroke sees nothing but ink, its
/// local contrast collapses, and Sauvola calls the middle paper.
///
/// The default stays at 12 anyway. A wider window also fills in anything dark
/// that is not ink: on `design/assets/hero.jpg` coverage rose from 5.1% at 12
/// to 6.9% at 20 and 9.0% at 40, all of it desk clutter. So width is something
/// the user states, not something assumed.
#[test]
fn the_default_window_cannot_span_a_broad_stroke() {
    let page = calligraphy();
    assert!(holes(&page, &Options::default()) > 0, "the fixture no longer exercises the defect");
}

/// Stating the stroke fixes it, at a value the app's slider can reach (0-40).
///
/// Measured on this fixture, with both radii set to the stated half-width:
/// 16 left 12,533 of 77,680 interior pixels transparent, 18 left 2,554, and 20
/// — exactly the half-width — left none. The same held at 60 px (30) and 80 px
/// (40). Raising only the paper radius, which is all the slider used to do,
/// changed nothing: the paper estimate was never the stage at fault.
#[test]
fn a_thick_stroke_comes_out_solid_once_its_half_width_is_stated() {
    let page = calligraphy();
    let holes = holes(&page, &Options::default().for_thick_strokes(20));
    assert_eq!(holes, 0, "{holes} stroke-interior pixels came out transparent: the stroke is hollow");
}

/// The app previews on a downscaled proxy. If the window did not travel with
/// it, the preview and the saved file would disagree about the holes.
#[test]
fn the_proxy_preview_of_a_thick_stroke_is_solid_too() {
    let half = make_page(&PageSpec {
        width: 480,
        height: 360,
        stroke_width: 20.0,
        noise: 0.01,
        ink_rgb: [0.08, 0.08, 0.10],
        ..Default::default()
    });
    let options = Options::default().for_thick_strokes(20).scaled_for_proxy(0.5);
    assert_eq!(holes(&half, &options), 0, "the preview shows holes the full render does not");
}

/// Stating a thin stroke must not shrink a window that was already wide enough.
#[test]
fn stating_a_thin_stroke_never_narrows_the_window() {
    let options = Options::default().for_thick_strokes(4);
    assert_eq!(options.sauvola_radius, Options::default().sauvola_radius);
}
