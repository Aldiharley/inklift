use inklift_api::{Provider, resolve_key};

fn nothing_set(_: &str) -> Option<String> {
    None
}

#[test]
fn an_explicit_key_is_used_as_given() {
    let key = resolve_key(Provider::Gemini, Some("abc"), nothing_set).unwrap();
    assert_eq!(key, "abc");
}

#[test]
fn the_environment_supplies_the_key_when_no_flag_is_given() {
    let key = resolve_key(Provider::OpenAi, None, |name| {
        (name == "OPENAI_API_KEY").then(|| "sk-from-env".to_string())
    })
    .unwrap();
    assert_eq!(key, "sk-from-env");
}

#[test]
fn a_missing_key_names_the_variable_to_set() {
    let err = resolve_key(Provider::Gemini, None, nothing_set).unwrap_err();
    assert!(err.contains("GEMINI_API_KEY"), "got {err}");
}

#[test]
fn a_blank_key_counts_as_missing() {
    let err = resolve_key(Provider::Gemini, Some("   "), nothing_set).unwrap_err();
    assert!(err.contains("GEMINI_API_KEY"), "got {err}");
}
