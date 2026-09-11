const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn value_of(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a') as u32 + 26),
        b'0'..=b'9' => Some((c - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decode standard base64, tolerating whitespace, which providers insert when
/// wrapping multi-megabyte payloads.
pub fn b64_decode(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: Vec<u8> = text
        .bytes()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let body: &[u8] = cleaned.strip_suffix(b"==").or_else(|| cleaned.strip_suffix(b"=")).unwrap_or(&cleaned);
    let padding = cleaned.len() - body.len();

    if cleaned.len() % 4 != 0 && !cleaned.is_empty() {
        return Err(format!("base64 length {} is not a multiple of 4", cleaned.len()));
    }
    if body.len() % 4 == 1 {
        return Err("base64 has a dangling character".into());
    }

    let mut out = Vec::with_capacity(body.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for &c in body {
        let v = value_of(c).ok_or_else(|| format!("invalid base64 character {:?}", c as char))?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    let _ = padding;
    Ok(out)
}
