mod common;

use common::{PageSpec, Rng, iou, make_page};
use inklift_core::{Grid, estimate_background, normalize_illumination, sauvola};

fn normalized(page: &common::Page) -> Grid {
    let bg = estimate_background(&page.gray, None);
    normalize_illumination(&page.gray, &bg)
}

#[test]
fn sauvola_recovers_strokes_under_a_lighting_gradient() {
    let page = make_page(&PageSpec { gradient: 0.5, ..Default::default() });

    let mask = sauvola(&normalized(&page), 12, 0.2);

    let score = iou(mask.data(), &page.ink, 0.5);
    assert!(score > 0.70, "IoU against ground truth was only {score:.3}");
}

#[test]
fn sauvola_survives_sensor_noise() {
    let page = make_page(&PageSpec { noise: 0.02, ..Default::default() });

    let mask = sauvola(&normalized(&page), 12, 0.2);

    let score = iou(mask.data(), &page.ink, 0.5);
    assert!(score > 0.60, "IoU with noise was only {score:.3}");
}

/// The classic local-thresholding failure: inventing ink in blank regions,
/// where the local standard deviation is pure noise.
#[test]
fn sauvola_does_not_invent_ink_on_blank_paper() {
    let (w, h) = (192usize, 144usize);
    let mut blank = Grid::filled(w, h, 1.0);
    let mut rng = Rng::new(3);
    blank.map_in_place(|v| v + rng.normal() * 0.01);

    let mask = sauvola(&blank, 12, 0.2);

    let fired = mask.data().iter().filter(|&&b| b).count();
    let frac = fired as f32 / (w * h) as f32;
    assert!(frac < 0.01, "{:.2}% of blank paper was marked as ink", frac * 100.0);
}
