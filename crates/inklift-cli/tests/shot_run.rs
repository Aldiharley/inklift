#![cfg(feature = "shot")]

use inklift_cli::{parse_shot_args, run_shot};
use std::path::PathBuf;

fn out(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("inklift-shot-{}-{name}.png", std::process::id()));
    p
}

/// X11 needs a server to talk to. Windows has a desktop to read whenever a user
/// is logged in, which is the only way these tests get run there.
fn have_display() -> bool {
    cfg!(windows) || std::env::var("DISPLAY").is_ok()
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn an_explicit_region_captures_and_extracts_at_the_right_size() {
    if !have_display() {
        eprintln!("skipped: no DISPLAY");
        return;
    }
    let dst = out("region");
    let cfg = parse_shot_args(&args(&[
        "--region", "0,0,320,200",
        "--white",
        "--no-clipboard",
        "-q",
        "-o", dst.to_str().unwrap(),
    ]))
    .unwrap();

    let report = run_shot(&cfg).expect("capture should succeed").expect_captured();

    assert_eq!(report.region.to_string(), "0,0,320,200");
    assert_eq!(report.written.len(), 1);
    assert!(dst.exists(), "output was not written");

    let img = image_size(&dst);
    assert_eq!(img, (320, 200), "output must match the requested region");
    let _ = std::fs::remove_file(dst);
}

#[test]
fn keep_raw_saves_the_untouched_capture_alongside() {
    if !have_display() {
        return;
    }
    let dst = out("raw");
    let cfg = parse_shot_args(&args(&[
        "--region", "0,0,160,120",
        "--keep-raw", "--no-clipboard", "-q",
        "-o", dst.to_str().unwrap(),
    ]))
    .unwrap();

    let report = run_shot(&cfg).unwrap().expect_captured();

    let raw = report.raw.expect("raw path should be reported");
    assert!(raw.exists(), "raw capture missing");
    assert_eq!(image_size(&raw), (160, 120));
    for p in [dst, raw] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn without_keep_raw_no_extra_file_appears() {
    if !have_display() {
        return;
    }
    let dst = out("noraw");
    let cfg = parse_shot_args(&args(&[
        "--region", "0,0,64,64", "--no-clipboard", "-q",
        "-o", dst.to_str().unwrap(),
    ]))
    .unwrap();

    let report = run_shot(&cfg).unwrap().expect_captured();

    assert!(report.raw.is_none());
    assert!(!inklift_cli::raw_path(&dst).exists());
    let _ = std::fs::remove_file(dst);
}

#[test]
fn a_region_off_the_desktop_fails_without_writing_anything() {
    if !have_display() {
        return;
    }
    let dst = out("offscreen");
    let cfg = parse_shot_args(&args(&[
        "--region", "90000,90000,100,100", "--no-clipboard", "-q",
        "-o", dst.to_str().unwrap(),
    ]))
    .unwrap();

    assert!(run_shot(&cfg).is_err());
    assert!(!dst.exists(), "a failed capture must not leave a file");
}

#[test]
fn full_screen_capture_matches_the_monitor_size() {
    if !have_display() {
        return;
    }
    let dst = out("full");
    let cfg = parse_shot_args(&args(&[
        "--full", "--no-clipboard", "-q", "-o", dst.to_str().unwrap(),
    ]))
    .unwrap();

    let report = run_shot(&cfg).unwrap().expect_captured();

    assert!(report.region.width >= 640 && report.region.height >= 400);
    assert_eq!(image_size(&dst), (report.region.width, report.region.height));
    let _ = std::fs::remove_file(dst);
}

fn image_size(p: &std::path::Path) -> (u32, u32) {
    let planes = inklift_cli::load_rgb(p).expect("should be readable");
    (planes[0].width() as u32, planes[0].height() as u32)
}

/// Cancelling is a normal outcome, not a failure, and must be distinguishable
/// from a capture that went wrong. The cancel path itself is driven by
/// SelectionState (unit-tested) and confirmed by the manual checklist.
#[test]
fn a_successful_capture_reports_itself_as_captured() {
    if !have_display() {
        return;
    }
    let dst = out("outcome");
    let cfg = parse_shot_args(&args(&[
        "--region", "0,0,64,64", "--no-clipboard", "-q",
        "-o", dst.to_str().unwrap(),
    ]))
    .unwrap();

    match run_shot(&cfg).unwrap() {
        inklift_cli::ShotOutcome::Captured(r) => assert_eq!(r.region.to_string(), "0,0,64,64"),
        inklift_cli::ShotOutcome::Cancelled => panic!("an explicit region cannot be cancelled"),
    }
    let _ = std::fs::remove_file(dst);
}
