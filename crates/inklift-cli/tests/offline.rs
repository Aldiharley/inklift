//! inklift runs entirely on the machine it is given: there is no hosted-model
//! path. These pin that the flags which once reached one are gone for good,
//! rather than lingering as options that do nothing.

use inklift_cli::parse_args;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// A flag that was accepted and ignored would leave someone believing they
/// had called a model. Each must be refused as an option that does not exist.
#[test]
fn the_old_hosted_model_flags_are_unknown_options() {
    for flag in ["--via", "--model", "--prompt", "--api-key", "--timeout"] {
        let err = parse_args(&args(&["a.png", flag, "x"]))
            .expect_err("a removed flag must be refused")
            .to_string();
        assert!(err.contains("unknown option"), "{flag} should be unknown: {err}");
    }
}

#[test]
fn the_usage_text_does_not_advertise_a_hosted_model() {
    let help = parse_args(&args(&["--help"])).unwrap_err().to_string();
    for word in ["--via", "--api-key", "HOSTED"] {
        assert!(!help.contains(word), "the help still mentions {word}");
    }
}

/// A build without the capture feature must not advertise the subcommand.
/// Checked against the usage line rather than the bare word, which also
/// appears in the prose describing --invert.
#[cfg(not(feature = "shot"))]
#[test]
fn the_shot_subcommand_is_not_advertised_without_its_feature() {
    let help = parse_args(&args(&["--help"])).unwrap_err().to_string();
    assert!(
        !help.contains("inklift shot"),
        "a build without the shot feature should not advertise the subcommand"
    );
}
