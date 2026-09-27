#![cfg(feature = "shot")]

use inklift_cli::{ShotSource, parse_shot_args};
use inklift_shot::Rect;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn with_no_flags_the_capture_is_interactive() {
    let cfg = parse_shot_args(&[]).unwrap();
    assert_eq!(cfg.source, ShotSource::Interactive);
    assert!(cfg.clipboard, "clipboard is on by default");
    assert!(!cfg.keep_raw, "the raw capture is kept only when asked for");
}

#[test]
fn an_explicit_region_skips_the_overlay() {
    let cfg = parse_shot_args(&args(&["--region", "100,50,400,200"])).unwrap();
    assert_eq!(cfg.source, ShotSource::Region(Rect::new(100, 50, 400, 200)));
}

#[test]
fn a_malformed_region_is_rejected_with_the_expected_format() {
    let err = parse_shot_args(&args(&["--region", "100,50"])).unwrap_err().to_string();
    assert!(err.contains("X,Y,W,H"), "got {err}");
}

#[test]
fn full_screen_capture_can_name_a_screen() {
    assert_eq!(parse_shot_args(&args(&["--full"])).unwrap().source, ShotSource::Full(None));
    assert_eq!(
        parse_shot_args(&args(&["--full", "--screen", "1"])).unwrap().source,
        ShotSource::Full(Some(1))
    );
}

/// Asking for two different sources at once is a mistake, not a preference.
#[test]
fn conflicting_sources_are_refused() {
    assert!(parse_shot_args(&args(&["--full", "--region", "0,0,10,10"])).is_err());
}

#[test]
fn extraction_flags_pass_through_to_the_pipeline() {
    let cfg = parse_shot_args(&args(&["--k", "0.35", "--min-area", "30", "--white"])).unwrap();
    assert!((cfg.options.sauvola_k - 0.35).abs() < 1e-6);
    assert_eq!(cfg.options.min_area, 30);
    assert_eq!(cfg.mode, inklift_cli::Output::White);
}

#[test]
fn the_clipboard_and_raw_capture_can_be_toggled() {
    let cfg = parse_shot_args(&args(&["--no-clipboard", "--keep-raw"])).unwrap();
    assert!(!cfg.clipboard);
    assert!(cfg.keep_raw);
}

#[test]
fn a_delay_is_accepted_for_opening_a_menu_first() {
    assert_eq!(parse_shot_args(&args(&["--delay", "3"])).unwrap().delay_secs, 3);
    assert_eq!(parse_shot_args(&[]).unwrap().delay_secs, 0);
}

#[test]
fn an_unknown_flag_is_refused() {
    assert!(parse_shot_args(&args(&["--wat"])).is_err());
}

#[test]
fn output_defaults_to_a_timestamped_name_in_the_working_directory() {
    let cfg = parse_shot_args(&[]).unwrap();
    let name = cfg.output.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("shot-"), "got {name}");
    assert!(name.ends_with(".png"), "got {name}");
}

#[test]
fn an_explicit_output_path_is_used_verbatim() {
    let cfg = parse_shot_args(&args(&["-o", "notes/page.png"])).unwrap();
    assert_eq!(cfg.output.to_str().unwrap(), "notes/page.png");
}

/// Hand-rolled calendar maths, pinned against known instants. Leap years and
/// the 1900/2100 century rule are where this kind of code usually goes wrong.
#[test]
fn epoch_seconds_convert_to_the_right_civil_date() {
    for (secs, expected) in [
        (0u64, (1970i64, 1u32, 1u32, 0u32, 0u32, 0u32)),
        (951_782_400, (2000, 2, 29, 0, 0, 0)),   // leap day, divisible-by-400 year
        (1_609_459_199, (2020, 12, 31, 23, 59, 59)),
        (1_735_689_600, (2025, 1, 1, 0, 0, 0)),
        (1_709_164_800, (2024, 2, 29, 0, 0, 0)), // ordinary leap year
        (4_102_444_800, (2100, 1, 1, 0, 0, 0)),  // 2100 is NOT a leap year
    ] {
        assert_eq!(inklift_cli::civil_from_epoch(secs), expected, "at epoch {secs}");
    }
}

#[test]
fn the_raw_capture_sits_beside_the_result() {
    use std::path::Path;
    assert_eq!(
        inklift_cli::raw_path(Path::new("notes/page.png")).to_str().unwrap(),
        "notes/page.raw.png"
    );
    assert_eq!(inklift_cli::raw_path(Path::new("x.png")).to_str().unwrap(), "x.raw.png");
}

#[test]
fn shot_accepts_invert_for_dark_themed_captures() {
    assert!(!parse_shot_args(&[]).unwrap().options.invert);
    assert!(parse_shot_args(&args(&["--invert"])).unwrap().options.invert);
}

#[test]
fn shot_accepts_an_ink_colour_for_pasting_onto_dark_slides() {
    assert!(parse_shot_args(&[]).unwrap().ink.is_none());
    assert_eq!(parse_shot_args(&args(&["--ink", "white"])).unwrap().ink.unwrap(), [1.0, 1.0, 1.0]);
}
