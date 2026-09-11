mod common;

use common::{PageSpec, iou, make_page};
use inklift_core::{
    Mask, despeckle, estimate_background, normalize_illumination, sauvola,
};

fn mask_with(width: usize, height: usize, cells: &[(usize, usize)]) -> Mask {
    let mut m = Mask::new(width, height);
    for &(x, y) in cells {
        m.set(x, y, true);
    }
    m
}

#[test]
fn despeckle_removes_specks_below_the_area_threshold() {
    let mut m = Mask::new(40, 40);
    // one solid 6x6 blob = 36 px
    for y in 5..11 {
        for x in 5..11 {
            m.set(x, y, true);
        }
    }
    // plus scattered dust
    for &(x, y) in &[(20, 20), (25, 30), (30, 5), (35, 35)] {
        m.set(x, y, true);
    }
    assert_eq!(m.count(), 40);

    let out = despeckle(&m, 8);

    assert_eq!(out.count(), 36, "only the blob should remain");
    assert!(out.get(7, 7));
    assert!(!out.get(20, 20));
}

#[test]
fn despeckle_joins_diagonally_touching_pixels() {
    // Eight-connectivity: this diagonal chain is one component of 4 px,
    // so a threshold of 4 must keep it and a threshold of 5 must drop it.
    let m = mask_with(20, 20, &[(2, 2), (3, 3), (4, 4), (5, 5)]);

    assert_eq!(despeckle(&m, 4).count(), 4);
    assert_eq!(despeckle(&m, 5).count(), 0);
}

#[test]
fn despeckle_leaves_a_clean_page_untouched() {
    let page = make_page(&PageSpec::default());
    let bg = estimate_background(&page.gray, None);
    let mask = sauvola(&normalize_illumination(&page.gray, &bg), 12, 0.2);
    let before = iou(mask.data(), &page.ink, 0.5);

    let out = despeckle(&mask, 8);

    let after = iou(out.data(), &page.ink, 0.5);
    assert!(after >= before - 0.02, "despeckle cost {:.3} IoU", before - after);
}

#[test]
fn despeckle_with_zero_threshold_changes_nothing() {
    let m = mask_with(10, 10, &[(1, 1), (5, 5)]);
    assert_eq!(despeckle(&m, 0), m);
}
