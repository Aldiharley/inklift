//! Behaviour of a default build, which has no hosted-model support linked in.
#![cfg(not(feature = "api"))]

use inklift_cli::parse_args;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Silently ignoring --via would leave someone believing they had called a
/// model. Refusing, and saying how to get one, is the only safe answer.
#[test]
fn via_is_refused_with_instructions_to_rebuild() {
    let err = parse_args(&args(&["a.png", "--via", "openai"]))
        .expect_err("a build without the feature must refuse --via")
        .to_string();

    assert!(
        err.contains("cargo build --release --features inklift-cli/api"),
        "must give the exact command from the workspace root: {err}"
    );
}

#[test]
fn the_hosted_flags_are_refused_too() {
    for flag in ["--model", "--prompt", "--api-key"] {
        assert!(
            parse_args(&args(&["a.png", flag, "x"])).is_err(),
            "{flag} should be refused in an offline build"
        );
    }
}

#[test]
fn the_usage_text_does_not_advertise_what_is_not_there() {
    let help = parse_args(&args(&["--help"])).unwrap_err().to_string();
    assert!(!help.contains("--via"), "offline build should not list --via");
}

#[test]
fn the_local_path_is_unaffected() {
    let cfg = parse_args(&args(&["a.png", "--white", "--min-area", "30"])).unwrap();
    assert_eq!(cfg.options.min_area, 30);
}

/// The screen-capture path is a separate feature from the hosted-model one;
/// a build without it must not advertise the subcommand. Checked against the
/// usage line rather than the bare word, which also appears in the prose
/// describing --invert.
#[cfg(not(feature = "shot"))]
#[test]
fn the_shot_subcommand_is_not_advertised_without_its_feature() {
    let help = parse_args(&args(&["--help"])).unwrap_err().to_string();
    assert!(
        !help.contains("inklift shot"),
        "a build without the shot feature should not advertise the subcommand"
    );
}
