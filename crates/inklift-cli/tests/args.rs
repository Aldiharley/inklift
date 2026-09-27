use inklift_cli::{Config, Output, parse_args};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn an_input_path_alone_is_enough() {
    let cfg: Config = parse_args(&args(&["photo.jpg"])).expect("should parse");

    assert_eq!(cfg.input.to_str().unwrap(), "photo.jpg");
    assert_eq!(cfg.output.to_str().unwrap(), "photo.ink.png");
    assert_eq!(cfg.mode, Output::Transparent);
}

#[test]
fn the_white_flag_switches_the_export() {
    let cfg = parse_args(&args(&["note.png", "--white"])).expect("should parse");
    assert_eq!(cfg.mode, Output::White);
}

#[test]
fn an_explicit_output_path_wins() {
    let cfg = parse_args(&args(&["a.png", "-o", "b.png"])).expect("should parse");
    assert_eq!(cfg.output.to_str().unwrap(), "b.png");
}

#[test]
fn tuning_flags_reach_the_core_options() {
    let cfg = parse_args(&args(&["a.png", "--k", "0.35", "--min-area", "24"])).expect("parse");

    assert!((cfg.options.sauvola_k - 0.35).abs() < 1e-6);
    assert_eq!(cfg.options.min_area, 24);
}

#[test]
fn an_unknown_flag_is_rejected() {
    assert!(parse_args(&args(&["a.png", "--wat"])).is_err());
}

#[test]
fn a_flag_missing_its_value_is_rejected() {
    assert!(parse_args(&args(&["a.png", "--k"])).is_err());
}

#[test]
fn no_arguments_at_all_is_rejected() {
    assert!(parse_args(&[]).is_err());
}

#[test]
fn help_is_requestable() {
    let err = parse_args(&args(&["--help"])).unwrap_err();
    assert!(err.to_string().contains("inklift"), "help text should name the tool");
}

#[test]
fn invert_is_opt_in_and_reaches_the_pipeline() {
    assert!(!parse_args(&args(&["a.png"])).unwrap().options.invert);
    assert!(parse_args(&args(&["a.png", "--invert"])).unwrap().options.invert);
}
