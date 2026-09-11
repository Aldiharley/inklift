use serde_json::{Value, json};

use crate::base64::{b64_decode, b64_encode};
use crate::provider::Provider;

/// The instruction sent with the image.
///
/// Written to fight the failure mode the OCRGenBench evaluation documents:
/// asked to "clean up" a page, these models re-render the text in their own
/// hand. Whether this wording is enough is exactly what the comparison is for.
pub const DEFAULT_PROMPT: &str = "\
Remove the background from this photograph of handwriting and place the \
handwriting on a pure white background. Preserve the exact original strokes, \
letterforms, slant, pressure variation and pen colour. Do not redraw, \
re-render, beautify, straighten or transcribe the handwriting. Do not add, \
remove or complete any stroke. Output the same page at the same aspect ratio.";

/// A prepared HTTP request, built without touching the network so it can be tested.
#[derive(Clone, Debug)]
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

fn multipart_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n");
}

pub fn build_request(
    provider: Provider,
    model: &str,
    api_key: &str,
    image: &[u8],
    mime: &str,
    prompt: &str,
    transparent: bool,
) -> Request {
    match provider {
        Provider::Gemini => {
            let body = json!({
                "contents": [{
                    "parts": [
                        {"text": prompt},
                        {"inlineData": {"mimeType": mime, "data": b64_encode(image)}}
                    ]
                }],
                "generationConfig": {
                    "responseModalities": ["IMAGE"],
                    "imageConfig": {"imageSize": "2K"}
                }
            });
            Request {
                url: format!(
                    "https://generativelanguage.googleapis.com/v1/models/{model}:generateContent"
                ),
                // The key goes in a header, never the URL, which ends up in logs.
                headers: vec![
                    ("x-goog-api-key".into(), api_key.to_string()),
                    ("Content-Type".into(), "application/json".into()),
                ],
                body: body.to_string().into_bytes(),
            }
        }
        Provider::OpenAi => {
            let boundary = "inkliftBoundaryQ1W2E3R4T5Y6";
            let mut body = Vec::new();
            multipart_field(&mut body, boundary, "model", model);
            multipart_field(&mut body, boundary, "prompt", prompt);
            multipart_field(&mut body, boundary, "output_format", "png");
            multipart_field(&mut body, boundary, "input_fidelity", "high");
            multipart_field(
                &mut body,
                boundary,
                "background",
                if transparent { "transparent" } else { "opaque" },
            );
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                b"Content-Disposition: form-data; name=\"image\"; filename=\"page.png\"\r\n",
            );
            body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
            body.extend_from_slice(image);
            body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

            Request {
                url: "https://api.openai.com/v1/images/edits".into(),
                headers: vec![
                    ("Authorization".into(), format!("Bearer {api_key}")),
                    ("Content-Type".into(), format!("multipart/form-data; boundary={boundary}")),
                ],
                body,
            }
        }
    }
}

/// Pull the returned image out of a provider response, or explain why there isn't one.
pub fn parse_response(provider: Provider, body: &[u8]) -> Result<Vec<u8>, String> {
    let root: Value =
        serde_json::from_slice(body).map_err(|e| format!("response was not JSON: {e}"))?;

    if let Some(message) = root.pointer("/error/message").and_then(Value::as_str) {
        let status = root
            .pointer("/error/status")
            .and_then(Value::as_str)
            .or_else(|| root.pointer("/error/type").and_then(Value::as_str))
            .unwrap_or("error");
        return Err(format!("{}: {status}: {message}", provider.name()));
    }

    match provider {
        Provider::Gemini => {
            let parts = root
                .pointer("/candidates/0/content/parts")
                .and_then(Value::as_array)
                .ok_or_else(|| "gemini: response had no candidates".to_string())?;
            for part in parts {
                // The REST API has shipped both spellings; accept either.
                let data = part
                    .pointer("/inlineData/data")
                    .or_else(|| part.pointer("/inline_data/data"))
                    .and_then(Value::as_str);
                if let Some(data) = data {
                    return b64_decode(data).map_err(|e| format!("gemini: {e}"));
                }
            }
            let said: Vec<&str> = parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect();
            Err(format!(
                "gemini: reply contained no image. The model said: {}",
                if said.is_empty() { "(nothing)".into() } else { said.join(" ") }
            ))
        }
        Provider::OpenAi => {
            let data = root
                .pointer("/data/0/b64_json")
                .and_then(Value::as_str)
                .ok_or_else(|| "openai: reply contained no image".to_string())?;
            b64_decode(data).map_err(|e| format!("openai: {e}"))
        }
    }
}
