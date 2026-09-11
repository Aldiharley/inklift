use inklift_cli::parse_args;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn without_the_flag_the_local_pipeline_is_used() {
    let cfg = parse_args(&args(&["a.png"])).unwrap();
    assert!(cfg.via.is_none(), "the hosted path must be opt-in");
}

#[test]
fn the_via_flag_selects_a_provider() {
    let cfg = parse_args(&args(&["a.png", "--via", "gemini"])).unwrap();
    let via = cfg.via.expect("provider should be set");
    assert_eq!(via.provider, inklift_api::Provider::Gemini);
    assert_eq!(via.model, "gemini-3-pro-image");
}

#[test]
fn the_model_can_be_overridden() {
    let cfg = parse_args(&args(&["a.png", "--via", "gemini", "--model", "gemini-3.1-flash-image"]))
        .unwrap();
    assert_eq!(cfg.via.unwrap().model, "gemini-3.1-flash-image");
}

#[test]
fn a_custom_prompt_replaces_the_default() {
    let cfg = parse_args(&args(&["a.png", "--via", "openai", "--prompt", "just do it"])).unwrap();
    assert_eq!(cfg.via.unwrap().prompt, "just do it");
}

#[test]
fn an_unknown_provider_is_rejected() {
    assert!(parse_args(&args(&["a.png", "--via", "skynet"])).is_err());
}

#[test]
fn model_without_via_is_rejected_rather_than_silently_ignored() {
    assert!(parse_args(&args(&["a.png", "--model", "gpt-image-2"])).is_err());
}
