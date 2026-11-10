//! Standard base64 (RFC 4648 alphabet), as VTK's XML writers encode binary data.

use crate::{Error, Result};

const INVALID: u8 = 0xff;

const TABLE: [u8; 256] = {
    let mut t = [INVALID; 256];
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i = 0;
    while i < 64 {
        t[alphabet[i] as usize] = i as u8;
        i += 1;
    }
    t
};

/// Characters that encode `n` bytes as one stream (with padding).
pub fn encoded_len(n: u64) -> u64 {
    n.div_ceil(3) * 4
}

/// Decodes `src` (no white space; `=` padding allowed at the end of each 4-character group,
/// after which decoding stops).
pub fn decode(src: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.try_reserve_exact(src.len() / 4 * 3 + 3)
        .map_err(|_| Error::TooLarge)?;
    let mut chunks = src.chunks_exact(4);
    for c in &mut chunks {
        let (a, b, cc, d) = (
            TABLE[c[0] as usize],
            TABLE[c[1] as usize],
            TABLE[c[2] as usize],
            TABLE[c[3] as usize],
        );
        if (a | b | cc | d) < 64 {
            let v =
                (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(cc) << 6) | u32::from(d);
            out.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
            continue;
        }
        // Padding (or a bad character).
        if a == INVALID || b == INVALID {
            return Err(Error::Malformed("bad base64 data".into()));
        }
        let v = (u32::from(a) << 18) | (u32::from(b) << 12);
        out.push((v >> 16) as u8);
        if c[2] == b'=' {
            return Ok(out);
        }
        if cc == INVALID {
            return Err(Error::Malformed("bad base64 data".into()));
        }
        let v = v | (u32::from(cc) << 6);
        out.push((v >> 8) as u8);
        if c[3] == b'=' {
            return Ok(out);
        }
        return Err(Error::Malformed("bad base64 data".into()));
    }
    let rest = chunks.remainder();
    if rest.len() >= 2 {
        // An unpadded tail.
        let mut v = 0u32;
        for (i, &ch) in rest.iter().enumerate() {
            let x = TABLE[ch as usize];
            if x == INVALID {
                return Err(Error::Malformed("bad base64 data".into()));
            }
            v |= u32::from(x) << (18 - 6 * i);
        }
        out.push((v >> 16) as u8);
        if rest.len() == 3 {
            out.push((v >> 8) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(decode(b"").unwrap(), b"");
        assert_eq!(decode(b"Zg==").unwrap(), b"f");
        assert_eq!(decode(b"Zm8=").unwrap(), b"fo");
        assert_eq!(decode(b"Zm9v").unwrap(), b"foo");
        assert_eq!(decode(b"Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(decode(b"Zm9vYg").unwrap(), b"foob");
        assert!(decode(b"Zm9!").is_err());
        assert_eq!(encoded_len(4), 8);
    }
}
