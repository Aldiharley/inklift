use inklift_cli::{parse_env_file, read_env_file};

fn pairs(text: &str) -> Vec<(String, String)> {
    parse_env_file(text)
}

#[test]
fn a_plain_assignment_is_read() {
    assert_eq!(pairs("GEMINI_API_KEY=abc123"), vec![("GEMINI_API_KEY".into(), "abc123".into())]);
}

#[test]
fn comments_and_blank_lines_are_skipped() {
    let text = "# a comment\n\n  \nOPENAI_API_KEY=sk-1\n# trailing note\n";
    assert_eq!(pairs(text), vec![("OPENAI_API_KEY".into(), "sk-1".into())]);
}

#[test]
fn an_export_prefix_is_tolerated() {
    assert_eq!(pairs("export GEMINI_API_KEY=abc"), vec![("GEMINI_API_KEY".into(), "abc".into())]);
}

#[test]
fn surrounding_quotes_are_stripped() {
    assert_eq!(pairs(r#"K="quoted value""#), vec![("K".into(), "quoted value".into())]);
    assert_eq!(pairs("K='single'"), vec![("K".into(), "single".into())]);
}

#[test]
fn whitespace_around_the_assignment_is_trimmed() {
    assert_eq!(pairs("  K  =  v  "), vec![("K".into(), "v".into())]);
}

/// API keys and base64 fragments contain '='; only the first one splits.
#[test]
fn only_the_first_equals_splits_the_line() {
    assert_eq!(pairs("K=a=b=c"), vec![("K".into(), "a=b=c".into())]);
}

#[test]
fn lines_without_an_assignment_are_ignored() {
    assert_eq!(pairs("just some text\nK=v\n=novalue\n"), vec![("K".into(), "v".into())]);
}

#[test]
fn an_empty_value_is_dropped_rather_than_masking_a_real_variable() {
    // A template copied but not filled in must not shadow a key that is
    // properly set in the environment.
    assert!(pairs("GEMINI_API_KEY=").is_empty());
    assert!(pairs("GEMINI_API_KEY=\"\"").is_empty());
}

#[test]
fn a_missing_file_reads_as_empty_rather_than_failing() {
    assert!(read_env_file(std::path::Path::new("definitely/not/here/.env")).is_empty());
}
