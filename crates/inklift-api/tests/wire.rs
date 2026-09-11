use inklift_api::{DEFAULT_PROMPT, Provider, b64_encode, build_request, parse_response};

const FAKE_PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];

#[test]
fn the_default_prompt_asks_for_preservation_not_beautification() {
    let p = DEFAULT_PROMPT.to_ascii_lowercase();
    assert!(p.contains("white"), "prompt must specify a white background");
    assert!(
        p.contains("exact") || p.contains("preserve") || p.contains("do not"),
        "prompt must tell the model not to redraw the strokes"
    );
}

#[test]
fn gemini_request_targets_generate_content_and_carries_the_image() {
    let req = build_request(Provider::Gemini, "gemini-3-pro-image", "KEY123", FAKE_PNG, "image/png", "TIDY IT", false);

    assert!(req.url.contains("gemini-3-pro-image:generateContent"), "got {}", req.url);
    assert!(req.headers.iter().any(|(k, v)| k == "x-goog-api-key" && v == "KEY123"));
    let body = String::from_utf8(req.body.clone()).unwrap();
    assert!(body.contains("TIDY IT"));
    assert!(body.contains(&b64_encode(FAKE_PNG)), "image data missing from body");
    assert!(body.contains("inlineData") && body.contains("responseModalities"));
}

#[test]
fn the_api_key_never_appears_in_the_gemini_url() {
    let req = build_request(Provider::Gemini, "m", "SECRET", FAKE_PNG, "image/png", "p", false);
    assert!(!req.url.contains("SECRET"), "key leaked into the URL, which gets logged");
}

#[test]
fn gemini_response_yields_the_image_bytes() {
    let body = format!(
        r#"{{"candidates":[{{"content":{{"parts":[
            {{"text":"Here you go"}},
            {{"inlineData":{{"mimeType":"image/png","data":"{}"}}}}
        ]}}}}]}}"#,
        b64_encode(FAKE_PNG)
    );

    let bytes = parse_response(Provider::Gemini, body.as_bytes()).expect("should parse");

    assert_eq!(bytes, FAKE_PNG);
}

#[test]
fn gemini_snake_case_inline_data_is_also_accepted() {
    let body = format!(
        r#"{{"candidates":[{{"content":{{"parts":[
            {{"inline_data":{{"mime_type":"image/png","data":"{}"}}}}
        ]}}}}]}}"#,
        b64_encode(FAKE_PNG)
    );
    assert_eq!(parse_response(Provider::Gemini, body.as_bytes()).unwrap(), FAKE_PNG);
}

#[test]
fn a_gemini_api_error_surfaces_its_message() {
    let body = r#"{"error":{"code":429,"message":"Quota exceeded","status":"RESOURCE_EXHAUSTED"}}"#;

    let err = parse_response(Provider::Gemini, body.as_bytes()).unwrap_err();

    assert!(err.contains("Quota exceeded"), "got {err}");
}

/// A text-only reply means the model refused or misread the request. That must
/// read as a clear failure, not an empty file.
#[test]
fn a_gemini_reply_with_no_image_is_an_error() {
    let body = r#"{"candidates":[{"content":{"parts":[{"text":"I cannot help with that"}]}}]}"#;

    let err = parse_response(Provider::Gemini, body.as_bytes()).unwrap_err();

    assert!(err.contains("no image"), "got {err}");
    assert!(err.contains("I cannot help"), "the model's own words should be shown");
}

#[test]
fn openai_request_is_multipart_and_embeds_the_raw_bytes() {
    let req = build_request(Provider::OpenAi, "gpt-image-2", "sk-test", FAKE_PNG, "image/png", "TIDY IT", true);

    assert!(req.url.ends_with("/v1/images/edits"), "got {}", req.url);
    assert!(req.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-test"));
    let content_type = req.headers.iter().find(|(k, _)| k == "Content-Type").unwrap();
    assert!(content_type.1.starts_with("multipart/form-data; boundary="));

    let boundary = content_type.1.rsplit('=').next().unwrap();
    assert!(req.body.windows(boundary.len()).any(|w| w == boundary.as_bytes()));
    assert!(
        req.body.windows(FAKE_PNG.len()).any(|w| w == FAKE_PNG),
        "raw image bytes should be sent verbatim, not re-encoded"
    );
    let text = String::from_utf8_lossy(&req.body);
    assert!(text.contains("TIDY IT"));
    assert!(text.contains("transparent"), "transparent was requested");
}

#[test]
fn openai_opaque_mode_does_not_ask_for_transparency() {
    let req = build_request(Provider::OpenAi, "gpt-image-2", "k", FAKE_PNG, "image/png", "p", false);
    let text = String::from_utf8_lossy(&req.body);
    assert!(text.contains("opaque") && !text.contains("transparent"));
}

#[test]
fn openai_response_yields_the_image_bytes() {
    let body = format!(r#"{{"data":[{{"b64_json":"{}"}}]}}"#, b64_encode(FAKE_PNG));
    assert_eq!(parse_response(Provider::OpenAi, body.as_bytes()).unwrap(), FAKE_PNG);
}

#[test]
fn an_openai_api_error_surfaces_its_message() {
    let body = r#"{"error":{"message":"Billing hard limit reached","type":"invalid_request_error"}}"#;
    let err = parse_response(Provider::OpenAi, body.as_bytes()).unwrap_err();
    assert!(err.contains("Billing hard limit"), "got {err}");
}

#[test]
fn malformed_json_is_an_error_not_a_panic() {
    assert!(parse_response(Provider::Gemini, b"not json at all").is_err());
    assert!(parse_response(Provider::OpenAi, b"{").is_err());
}
