use std::time::Duration;

use crate::provider::Provider;
use crate::wire::{Request, build_request, parse_response};

/// Find the API key: an explicit value wins, otherwise the provider's
/// environment variable. `lookup` is injected so this stays testable.
pub fn resolve_key(
    provider: Provider,
    explicit: Option<&str>,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let candidate = explicit
        .map(str::to_string)
        .or_else(|| lookup(provider.key_env()));
    match candidate {
        Some(key) if !key.trim().is_empty() => Ok(key.trim().to_string()),
        _ => Err(format!(
            "no API key for {}. Set {} in the environment, or pass --api-key.",
            provider.name(),
            provider.key_env()
        )),
    }
}

/// Read the key from the process environment.
pub fn key_from_env(provider: Provider, explicit: Option<&str>) -> Result<String, String> {
    resolve_key(provider, explicit, |name| std::env::var(name).ok())
}

fn send(request: &Request, timeout: Duration) -> Result<Vec<u8>, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .new_agent();

    let mut call = agent.post(&request.url);
    for (name, value) in &request.headers {
        call = call.header(name.as_str(), value.as_str());
    }

    // A 4xx/5xx still carries a JSON error body worth showing the user, so the
    // body is read either way and handed to the parser.
    let mut response = match call.send(&request.body[..]) {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(code)) => {
            return Err(format!("HTTP {code} from {}", request.url));
        }
        Err(e) => return Err(format!("request failed: {e}")),
    };
    response
        .body_mut()
        .read_to_vec()
        .map_err(|e| format!("could not read the response body: {e}"))
}

/// Send one page to a hosted model and return the image it replies with.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    provider: Provider,
    model: &str,
    api_key: &str,
    image: &[u8],
    mime: &str,
    prompt: &str,
    transparent: bool,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let request = build_request(provider, model, api_key, image, mime, prompt, transparent);
    let body = send(&request, timeout)?;
    parse_response(provider, &body)
}
