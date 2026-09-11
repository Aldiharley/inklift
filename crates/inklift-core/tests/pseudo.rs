use inklift_core::{Confusion, Mask, f_measure, pseudo_f_measure, score, skeleton};

fn bar(w: usize, h: usize, x0: usize, x1: usize, y0: usize, y1: usize) -> Mask {
    let mut m = Mask::new(w, h);
    for y in y0..y1 {
        for x in x0..x1 {
            m.set(x, y, true);
        }
    }
    m
}

#[test]
fn skeleton_thins_a_bar_to_a_hairline() {
    let thick = bar(40, 40, 4, 36, 10, 19); // 32 x 9 solid bar

    let thin = skeleton(&thick);

    assert!(thin.count() < thick.count() / 4, "skeleton kept {} px", thin.count());
    assert!(thin.count() >= 20, "skeleton collapsed to {} px", thin.count());
    for i in 0..thin.len() {
        assert!(!thin.data()[i] || thick.data()[i], "skeleton escaped the shape");
    }
}

#[test]
fn skeleton_of_an_empty_mask_is_empty() {
    assert_eq!(skeleton(&Mask::new(12, 12)).count(), 0);
}

#[test]
fn skeleton_is_idempotent() {
    let once = skeleton(&bar(40, 40, 4, 36, 10, 19));
    assert_eq!(skeleton(&once), once);
}

#[test]
fn pseudo_f_measure_of_a_perfect_result_is_100() {
    let truth = bar(40, 40, 4, 36, 10, 19);
    assert!((pseudo_f_measure(&truth, &truth) - 100.0).abs() < 1e-3);
}

/// The point of the measure: thinning a stroke without breaking it is a small
/// sin. Plain FM punishes it hard; pseudo-FM, which asks only whether the
/// stroke's medial axis survived, should not.
#[test]
fn pseudo_f_measure_forgives_thinning_that_f_measure_punishes() {
    let truth = bar(40, 40, 4, 36, 10, 19); // 9 px tall
    let eroded = bar(40, 40, 4, 36, 13, 16); // central 3 px only

    let fm = f_measure(&Confusion::of(&eroded, &truth));
    let p_fm = pseudo_f_measure(&eroded, &truth);

    assert!(fm < 70.0, "FM should punish the thinning, got {fm:.1}");
    assert!(p_fm > 95.0, "pseudo-FM should forgive it, got {p_fm:.1}");
}

/// But breaking a stroke in half is a real failure, and both must say so.
#[test]
fn pseudo_f_measure_still_punishes_a_broken_stroke() {
    let truth = bar(40, 40, 4, 36, 10, 19);
    let broken = bar(40, 40, 4, 18, 10, 19); // left half only

    assert!(pseudo_f_measure(&broken, &truth) < 75.0);
}

#[test]
fn score_reports_all_four_measures_together() {
    let truth = bar(40, 40, 4, 36, 10, 19);
    let mut predicted = truth.clone();
    predicted.set(30, 30, true); // one false alarm in open space

    let s = score(&predicted, &truth);

    assert!(s.fm > 98.0 && s.fm < 100.0);
    assert!(s.p_fm > 98.0);
    assert!(s.psnr.is_finite() && s.psnr > 20.0);
    assert!(s.drd > 0.0);
}
