use inklift_core::{Confusion, Mask, drd, f_measure, psnr};

fn mask(w: usize, h: usize, ink: &[(usize, usize)]) -> Mask {
    let mut m = Mask::new(w, h);
    for &(x, y) in ink {
        m.set(x, y, true);
    }
    m
}

#[test]
fn confusion_counts_the_four_cases() {
    let truth = mask(4, 4, &[(0, 0), (1, 0), (2, 0)]);
    let predicted = mask(4, 4, &[(0, 0), (1, 0), (3, 3)]);

    let c = Confusion::of(&predicted, &truth);

    assert_eq!((c.true_positive, c.false_positive, c.false_negative), (2, 1, 1));
    assert_eq!(c.true_negative, 16 - 4);
}

#[test]
fn f_measure_is_a_percentage_and_perfect_agreement_scores_100() {
    let truth = mask(8, 8, &[(1, 1), (2, 2), (3, 3)]);

    assert!((f_measure(&Confusion::of(&truth, &truth)) - 100.0).abs() < 1e-4);
}

#[test]
fn f_measure_matches_a_hand_computed_case() {
    // 8 hits, 2 misses, 2 false alarms: recall = precision = 0.8, so FM = 80%.
    let truth_px: Vec<(usize, usize)> = (0..10).map(|i| (i, 0)).collect();
    let mut pred_px: Vec<(usize, usize)> = (0..8).map(|i| (i, 0)).collect();
    pred_px.push((0, 5));
    pred_px.push((1, 5));

    let c = Confusion::of(&mask(16, 16, &pred_px), &mask(16, 16, &truth_px));

    assert_eq!((c.true_positive, c.false_positive, c.false_negative), (8, 2, 2));
    assert!((f_measure(&c) - 80.0).abs() < 1e-4, "got {}", f_measure(&c));
}

#[test]
fn f_measure_of_a_total_miss_is_zero() {
    let truth = mask(8, 8, &[(1, 1)]);
    let predicted = mask(8, 8, &[(7, 7)]);

    assert_eq!(f_measure(&Confusion::of(&predicted, &truth)), 0.0);
}

#[test]
fn psnr_matches_a_hand_computed_case() {
    // One wrong pixel out of 256: MSE = 1/256, so PSNR = 10*log10(256).
    let truth = Mask::new(16, 16);
    let predicted = mask(16, 16, &[(4, 4)]);

    let value = psnr(&predicted, &truth);

    assert!((value - 24.0824).abs() < 1e-3, "got {value}");
}

#[test]
fn a_perfect_result_has_no_distortion_and_unbounded_psnr() {
    let truth = mask(16, 16, &[(2, 2), (3, 2), (4, 2)]);

    assert_eq!(drd(&truth, &truth), 0.0);
    assert!(psnr(&truth, &truth).is_infinite());
}

#[test]
fn drd_of_one_isolated_false_positive_is_one_per_non_uniform_block() {
    // GT: a 4x4 square inside the first 8x8 block, so NUBN = 1.
    let mut ink = Vec::new();
    for y in 2..6 {
        for x in 2..6 {
            ink.push((x, y));
        }
    }
    let truth = mask(16, 16, &ink);

    // One extra pixel far from any text: its whole 5x5 neighbourhood is
    // background, so the normalised weights sum to exactly 1.
    let mut pred_ink = ink.clone();
    pred_ink.push((12, 12));
    let predicted = mask(16, 16, &pred_ink);

    assert!((drd(&predicted, &truth) - 1.0).abs() < 1e-4, "got {}", drd(&predicted, &truth));
}

/// A mistake next to real text is less visually damaging than one in open
/// space, and DRD is supposed to say so.
#[test]
fn drd_punishes_isolated_errors_more_than_adjacent_ones() {
    let mut ink = Vec::new();
    for y in 2..6 {
        for x in 2..6 {
            ink.push((x, y));
        }
    }
    let truth = mask(24, 24, &ink);

    let mut touching = ink.clone();
    touching.push((6, 4));
    let mut isolated = ink.clone();
    isolated.push((18, 18));

    assert!(
        drd(&mask(24, 24, &touching), &truth) < drd(&mask(24, 24, &isolated), &truth),
        "an error beside a stroke should cost less than one in open space"
    );
}
