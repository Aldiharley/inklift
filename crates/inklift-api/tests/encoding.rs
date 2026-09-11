use inklift_api::{Provider, b64_decode, b64_encode};

/// RFC 4648 test vectors.
#[test]
fn base64_matches_the_rfc_vectors() {
    for (raw, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(b64_encode(raw.as_bytes()), encoded, "encoding {raw:?}");
        assert_eq!(b64_decode(encoded).unwrap(), raw.as_bytes(), "decoding {encoded:?}");
    }
}

#[test]
fn base64_round_trips_binary_of_every_length() {
    for len in 0..200usize {
        let bytes: Vec<u8> = (0..len).map(|i| (i * 37 % 256) as u8).collect();
        assert_eq!(b64_decode(&b64_encode(&bytes)).unwrap(), bytes, "length {len}");
    }
}

/// Providers wrap long base64 payloads, so newlines must not break decoding.
#[test]
fn base64_ignores_embedded_whitespace() {
    assert_eq!(b64_decode("Zm9v\nYmFy").unwrap(), b"foobar");
    assert_eq!(b64_decode("Zm9v YmFy\r\n").unwrap(), b"foobar");
}

#[test]
fn base64_rejects_invalid_input() {
    assert!(b64_decode("Zm9v!!!!").is_err());
    assert!(b64_decode("Zg=").is_err(), "truncated group should be rejected");
}

#[test]
fn providers_are_selectable_by_name() {
    assert_eq!("gemini".parse::<Provider>().unwrap(), Provider::Gemini);
    assert_eq!("openai".parse::<Provider>().unwrap(), Provider::OpenAi);
    assert!("wat".parse::<Provider>().is_err());
}

#[test]
fn each_provider_names_a_default_model_and_key_variable() {
    assert_eq!(Provider::Gemini.default_model(), "gemini-3-pro-image");
    assert_eq!(Provider::Gemini.key_env(), "GEMINI_API_KEY");
    assert_eq!(Provider::OpenAi.default_model(), "gpt-image-2");
    assert_eq!(Provider::OpenAi.key_env(), "OPENAI_API_KEY");
}
