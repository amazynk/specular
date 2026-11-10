//! The compression of USD crate files: LZ4 blocks (in a chunked wrapper: a chunk count byte,
//! 0 for one block, else size-prefixed chunks) and the integer coding on top of it (a common
//! value, 2-bit codes, then 8/16/32-bit (or 16/32/64-bit) deltas).

use crate::{Error, Result};

fn corrupt<T>(what: &str) -> Result<T> {
    Err(Error::Malformed(format!("USDC: {what}")))
}

/// Decodes one LZ4 block, appending to `out` without passing `limit` bytes in all.
pub(crate) fn lz4_block(src: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<()> {
    let mut i = 0usize;
    let start = out.len();
    let len_ext = |i: &mut usize, mut n: usize| -> Result<usize> {
        if n == 15 {
            loop {
                let Some(&b) = src.get(*i) else {
                    return corrupt("LZ4 length runs past the block");
                };
                *i += 1;
                n += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(n)
    };
    while i < src.len() {
        let token = src[i];
        i += 1;
        let lit = len_ext(&mut i, (token >> 4) as usize)?;
        let Some(l) = src.get(i..i + lit) else {
            return corrupt("LZ4 literals run past the block");
        };
        if out.len() + lit > limit {
            return corrupt("LZ4 output larger than said");
        }
        out.extend_from_slice(l);
        i += lit;
        if i >= src.len() {
            break;
        }
        let Some(o) = src.get(i..i + 2) else {
            return corrupt("LZ4 offset cut short");
        };
        let offset = u16::from_le_bytes([o[0], o[1]]) as usize;
        i += 2;
        let m = len_ext(&mut i, (token & 15) as usize)? + 4;
        if offset == 0 || offset > out.len() - start {
            return corrupt("LZ4 match before the start");
        }
        if out.len() + m > limit {
            return corrupt("LZ4 output larger than said");
        }
        let from = out.len() - offset;
        if offset >= m {
            out.extend_from_within(from..from + m);
        } else {
            for k in 0..m {
                let b = out[from + k];
                out.push(b);
            }
        }
    }
    Ok(())
}

/// Decompresses a crate buffer (the chunked LZ4 wrapper) of at most `limit` bytes.
pub(crate) fn decompress(src: &[u8], limit: usize) -> Result<Vec<u8>> {
    let Some((&chunks, rest)) = src.split_first() else {
        return corrupt("empty compressed buffer");
    };
    // `limit` comes from the file: reserve no more than the input can expand to at once
    // (LZ4 grows a block at most 255 times); the buffer grows as data proves more.
    let mut out = Vec::with_capacity(limit.min(src.len().saturating_mul(255)).min(1 << 28));
    if chunks == 0 {
        lz4_block(rest, &mut out, limit)?;
        return Ok(out);
    }
    let mut at = 0usize;
    for _ in 0..chunks {
        let Some(n) = rest.get(at..at + 4) else {
            return corrupt("compressed chunk size cut short");
        };
        let n = i32::from_le_bytes([n[0], n[1], n[2], n[3]]);
        let n = usize::try_from(n).map_err(|_| Error::Malformed("USDC: chunk size".into()))?;
        at += 4;
        let Some(c) = rest.get(at..at + n) else {
            return corrupt("compressed chunk runs past the buffer");
        };
        lz4_block(c, &mut out, limit)?;
        at += n;
    }
    Ok(out)
}

/// Decodes `count` integers of the crate's integer coding from `enc` (already decompressed):
/// 32-bit when `wide` is false (deltas of 8, 16 or 32 bits), 64-bit otherwise (16, 32, 64).
pub(crate) fn decode_ints(enc: &[u8], count: usize, wide: bool) -> Result<Vec<i64>> {
    let w = if wide { 8 } else { 4 };
    let codes_len = (count * 2).div_ceil(8);
    if enc.len() < w + codes_len {
        return corrupt("integer codes cut short");
    }
    let common = if wide {
        i64::from_le_bytes(enc[..8].try_into().expect("8"))
    } else {
        i64::from(i32::from_le_bytes(enc[..4].try_into().expect("4")))
    };
    let codes = &enc[w..w + codes_len];
    let mut at = w + codes_len;
    let mut out = Vec::with_capacity(count);
    let mut prev = 0i64;
    for i in 0..count {
        let code = (codes[i / 4] >> ((i % 4) * 2)) & 3;
        let size = match code {
            0 => 0,
            1 => w / 4,
            2 => w / 2,
            _ => w,
        };
        let d = if size == 0 {
            common
        } else {
            let Some(b) = enc.get(at..at + size) else {
                return corrupt("integer data cut short");
            };
            at += size;
            match size {
                1 => i64::from(b[0] as i8),
                2 => i64::from(i16::from_le_bytes([b[0], b[1]])),
                4 => i64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                _ => i64::from_le_bytes(b.try_into().expect("8")),
            }
        };
        prev = prev.wrapping_add(d);
        out.push(if wide { prev } else { i64::from(prev as i32) });
    }
    Ok(out)
}

/// A compressed integer block: decompress, then decode.
pub(crate) fn compressed_ints(src: &[u8], count: usize, wide: bool) -> Result<Vec<i64>> {
    let w = if wide { 8 } else { 4 };
    // The encoded form is at most the common value, the codes and full-width values.
    let limit = w + (count * 2).div_ceil(8) + count * w + 16;
    let enc = decompress(src, limit)?;
    decode_ints(&enc, count, wide)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An LZ4 block of literals, then a match of `m` at `offset`.
    #[test]
    fn lz4_overlapping_matches() {
        // "abc" then copy 9 from offset 3 → "abcabcabcabc".
        let block = [0x35, b'a', b'b', b'c', 3, 0, 0x00];
        let mut out = Vec::new();
        lz4_block(&block[..6], &mut out, 100).unwrap();
        assert_eq!(out, b"abcabcabcabc");
        // Wrapped, one block.
        let mut w = vec![0u8];
        w.extend_from_slice(&block[..6]);
        assert_eq!(decompress(&w, 100).unwrap(), b"abcabcabcabc");
        assert!(lz4_block(&[0x35, b'a', b'b', b'c', 9, 0], &mut Vec::new(), 100).is_err());
        assert!(lz4_block(&block[..6], &mut Vec::new(), 5).is_err());
    }

    #[test]
    fn integer_deltas() {
        // common = 1; codes: common, int8, int16, int32 → 1, 1+(-3), +300, +70000.
        let mut enc = 1i32.to_le_bytes().to_vec();
        enc.push(0b11_10_01_00);
        enc.push((-3i8) as u8);
        enc.extend_from_slice(&300i16.to_le_bytes());
        enc.extend_from_slice(&70000i32.to_le_bytes());
        assert_eq!(decode_ints(&enc, 4, false).unwrap(), [1, -2, 298, 70298]);
        assert!(decode_ints(&enc[..6], 4, false).is_err());
    }
}
