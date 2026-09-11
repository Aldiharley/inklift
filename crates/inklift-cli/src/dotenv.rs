//! A minimal `.env` reader.
//!
//! Values here never override the real environment: an exported variable wins,
//! so a stale file cannot quietly shadow the key you meant to use.

use std::path::Path;

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Parse `KEY=VALUE` lines, tolerating comments, blanks and an `export` prefix.
///
/// Entries with an empty value are dropped: a template copied but not filled in
/// must not shadow a variable that is properly set.
pub fn parse_env_file(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = unquote(value.trim()).trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        out.push((key.to_string(), value.to_string()));
    }
    out
}

/// Read and parse a `.env`. A missing or unreadable file reads as empty.
pub fn read_env_file(path: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path).map(|t| parse_env_file(&t)).unwrap_or_default()
}

/// Look a name up in the real environment first, then in `pairs`.
pub fn env_or(pairs: &[(String, String)], name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()))
}
