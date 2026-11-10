//! Standard base64 (RFC 4648, with padding) for the sidecar's compact fields.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(b: &[u8]) -> String {
    let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                o.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                o.push('=');
            }
        }
    }
    o
}

/// Decodes, ignoring whitespace; `None` on any other stray character or bad length.
pub fn decode(s: &str) -> Option<Vec<u8>> {
    let v = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let s: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if !s.len().is_multiple_of(4) {
        return None;
    }
    let mut o = Vec::with_capacity(s.len() / 4 * 3);
    for (k, q) in s.chunks(4).enumerate() {
        let last = k + 1 == s.len() / 4;
        let pad = q.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for &c in &q[..4 - pad] {
            n = (n << 6) | v(c)?;
        }
        n <<= 6 * pad as u32;
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        o.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(o)
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip_and_rfc_vectors() {
        for (p, e) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(super::encode(p.as_bytes()), e);
            assert_eq!(super::decode(e).unwrap(), p.as_bytes());
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(super::decode(&super::encode(&all)).unwrap(), all);
        assert!(super::decode("Zm9v!").is_none());
        assert!(super::decode("Zg==Zg==").is_none());
    }
}
