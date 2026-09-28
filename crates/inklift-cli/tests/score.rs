use inklift_cli::{ScoreConfig, load_mask, normalize_stem, parse_score_args, run_score};
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("inklift-score-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// DIBCO ships ground truth beside results with a _GT suffix; pairing has to
/// see through that, and through case.
#[test]
fn stems_pair_across_ground_truth_suffixes() {
    assert_eq!(normalize_stem("H01_GT"), "h01");
    assert_eq!(normalize_stem("H01"), "h01");
    assert_eq!(normalize_stem("P02-gt"), "p02");
    assert_eq!(normalize_stem("img_gt"), "img");
    assert_eq!(normalize_stem("plain"), "plain");
}

#[test]
fn stems_that_merely_end_in_those_letters_are_left_alone() {
    assert_eq!(normalize_stem("target"), "target");
    assert_eq!(normalize_stem("night"), "night");
}

#[test]
fn a_mask_reads_dark_pixels_as_ink() {
    let d = dir("mask");
    let path = d.join("a.png");
    let (w, h) = (8usize, 8usize);
    let mut rgba = vec![255u8; w * h * 4];
    for c in 0..3 {
        rgba[(2 * w + 2) * 4 + c] = 0;
    }
    inklift_cli::save_rgba(&path, w, h, &rgba).unwrap();

    let mask = load_mask(&path, 128, false).unwrap();

    assert!(mask.get(2, 2), "the black pixel should be ink");
    assert!(!mask.get(5, 5), "white should be background");
    assert_eq!(mask.count(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn scoring_a_directory_pair_reports_one_row_per_image() {
    let results = dir("res");
    let truth = dir("gt");
    let (w, h) = (32usize, 32usize);

    for (name, shift) in [("a", 0usize), ("b", 1usize)] {
        let mut gt = vec![255u8; w * h * 4];
        let mut rs = vec![255u8; w * h * 4];
        for y in 10..20 {
            for x in 5..25 {
                for c in 0..3 {
                    gt[(y * w + x) * 4 + c] = 0;
                    rs[(y * w + x + shift) * 4 + c] = 0;
                }
            }
        }
        inklift_cli::save_rgba(&truth.join(format!("{name}_GT.png")), w, h, &gt).unwrap();
        inklift_cli::save_rgba(&results.join(format!("{name}.png")), w, h, &rs).unwrap();
    }

    let config = ScoreConfig {
        results: results.clone(),
        truth: truth.clone(),
        threshold: 128,
        invert: false,
        csv: None,
        resize: false,
    };
    let summary = run_score(&config).expect("scoring should succeed");

    assert_eq!(summary.rows.len(), 2, "both pairs should be scored");
    assert!(summary.rows[0].scores.fm > 80.0, "a 1px shift should still score well");
    assert!(summary.mean_fm > 80.0);
    assert!(summary.mean_drd > 0.0);

    for d in [results, truth] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn an_unmatched_result_is_reported_rather_than_silently_dropped() {
    let results = dir("res2");
    let truth = dir("gt2");
    let rgba = vec![255u8; 8 * 8 * 4];
    inklift_cli::save_rgba(&results.join("orphan.png"), 8, 8, &rgba).unwrap();
    inklift_cli::save_rgba(&truth.join("other_GT.png"), 8, 8, &rgba).unwrap();

    let config = ScoreConfig {
        results: results.clone(),
        truth: truth.clone(),
        threshold: 128,
        invert: false,
        csv: None,
        resize: false,
    };
    let summary = run_score(&config).unwrap();

    assert_eq!(summary.rows.len(), 0);
    assert_eq!(summary.unmatched, vec!["orphan".to_string()]);
    for d in [results, truth] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn score_arguments_need_two_directories() {
    assert!(parse_score_args(&["only-one".to_string()]).is_err());
    let cfg = parse_score_args(&["res".into(), "gt".into(), "--invert".into()]).unwrap();
    assert!(cfg.invert);
}

fn write_bar(path: &std::path::Path, w: usize, h: usize, scale: usize) {
    let mut rgba = vec![255u8; w * h * 4];
    for y in (10 * scale)..(20 * scale) {
        for x in (5 * scale)..(25 * scale) {
            for c in 0..3 {
                rgba[(y * w + x) * 4 + c] = 0;
            }
        }
    }
    inklift_cli::save_rgba(path, w, h, &rgba).unwrap();
}

/// Other tools return whatever size they like. Without help, that makes the
/// comparison impossible, so the mismatch has to be explained, not just refused.
#[test]
fn a_size_mismatch_is_reported_with_both_dimensions() {
    let results = dir("res3");
    let truth = dir("gt3");
    write_bar(&results.join("a.png"), 64, 64, 2);
    write_bar(&truth.join("a_GT.png"), 32, 32, 1);

    let err = run_score(&ScoreConfig {
        results: results.clone(),
        truth: truth.clone(),
        threshold: 128,
        invert: false,
        csv: None,
        resize: false,
    })
    .unwrap_err()
    .to_string();

    assert!(err.contains("64") && err.contains("32"), "got {err}");
    for d in [results, truth] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn resize_lets_a_differently_sized_result_be_scored() {
    let results = dir("res4");
    let truth = dir("gt4");
    write_bar(&results.join("a.png"), 64, 64, 2);
    write_bar(&truth.join("a_GT.png"), 32, 32, 1);

    let summary = run_score(&ScoreConfig {
        results: results.clone(),
        truth: truth.clone(),
        threshold: 128,
        invert: false,
        csv: None,
        resize: true,
    })
    .expect("resize should make this scoreable");

    assert_eq!(summary.rows.len(), 1);
    assert!(summary.rows[0].scores.fm > 95.0, "got {}", summary.rows[0].scores.fm);
    for d in [results, truth] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Transparent PNGs are exactly what inklift itself writes, and dropping the
/// alpha channel leaves cleared pixels holding whatever RGB sat underneath -
/// usually black, which then counts as solid ink. Transparent must read as
/// background, the same as white paper does.
#[test]
fn transparent_pixels_read_as_background_not_ink() {
    let d = dir("alpha");
    let path = d.join("a.png");
    let (w, h) = (16usize, 16usize);
    // Everything cleared, with black left underneath, plus one opaque black dot.
    let mut rgba = vec![0u8; w * h * 4];
    let i = 8 * w + 8;
    rgba[i * 4 + 3] = 255;

    inklift_cli::save_rgba(&path, w, h, &rgba).unwrap();
    let mask = load_mask(&path, 128, false).unwrap();

    assert_eq!(mask.count(), 1, "only the opaque dot is ink, not the cleared field");
    assert!(mask.get(8, 8));
    assert!(!mask.get(2, 2), "a cleared pixel must not count as ink");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn semi_transparent_ink_still_counts_when_dark_enough() {
    let d = dir("alpha2");
    let path = d.join("b.png");
    let (w, h) = (8usize, 8usize);
    let mut rgba = vec![0u8; w * h * 4];
    // A black pixel at 75% opacity composites to ~64/255 over white: still ink.
    let i = 4 * w + 4;
    rgba[i * 4 + 3] = 191;
    inklift_cli::save_rgba(&path, w, h, &rgba).unwrap();

    let mask = load_mask(&path, 128, false).unwrap();

    assert_eq!(mask.count(), 1);
    let _ = std::fs::remove_dir_all(d);
}
