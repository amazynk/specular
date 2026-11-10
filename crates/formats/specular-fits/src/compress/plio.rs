//! `PLIO_1`: IRAF's pixel-list line lists (`plio`), for masks and other images of runs of
//! non-negative integers.
//!
//! The tile is a list of big-endian 16-bit words: a header, then instructions of a 4-bit opcode
//! and 12-bit datum `n`. The header gives where the instructions start and end: in the current
//! format word 1 is the header length, word 2 is negative and words 3–4 give the list length
//! (low 15 bits, then high); in the old format word 2 is the length and instructions start at
//! word 3. A "high value" starts at 1 and the opcodes are:
//!
//! | op | meaning |
//! |----|---------|
//! | 0 | `n` zeros |
//! | 1 | high value = next word · 4096 + `n` |
//! | 2, 3 | high value ± `n` |
//! | 4 | `n` pixels of the high value |
//! | 5 | `n` − 1 zeros, then one pixel of the high value |
//! | 6, 7 | high value ± `n`, then one pixel of it |
//!
//! Pixels past the end of the list are zero.

use super::CodecError;

/// Decodes a `PLIO_1` tile of `out.len()` pixels (`npix`, which must equal it).
pub fn plio(input: &[u8], npix: usize, out: &mut [i32]) -> Result<(), CodecError> {
    if npix != out.len() {
        return Err(CodecError::Corrupt(
            "PLIO pixel count differs from the output",
        ));
    }
    let words = input.len() / 2;
    let w = |i: usize| -> Option<i16> {
        let b = input.get(2 * i..2 * i + 2)?;
        Some(i16::from_be_bytes([b[0], b[1]]))
    };
    let (Some(hdr), Some(len2)) = (w(1), w(2)) else {
        return Err(CodecError::Corrupt("PLIO header truncated"));
    };
    let (first, len) = if len2 > 0 {
        (3, len2 as usize)
    } else {
        let (Some(lo), Some(hi)) = (w(3), w(4)) else {
            return Err(CodecError::Corrupt("PLIO header truncated"));
        };
        let (Ok(hdr), Ok(lo), Ok(hi)) = (
            usize::try_from(hdr),
            usize::try_from(lo),
            usize::try_from(hi),
        ) else {
            return Err(CodecError::Corrupt("PLIO negative header field"));
        };
        (hdr, (hi << 15) + lo)
    };
    if len > words {
        return Err(CodecError::Corrupt("PLIO list longer than the tile"));
    }

    let mut op = 0usize;
    let mut pv = 1i32;
    let mut ip = first;
    while ip < len && op < out.len() {
        // In bounds: ip < len ≤ words.
        let word = u16::from_be_bytes([input[2 * ip], input[2 * ip + 1]]);
        let data = i32::from(word & 0xFFF);
        let n = data as usize;
        let rest = out.len() - op;
        match word >> 12 {
            0 | 4 => {
                let v = if word >> 12 == 4 { pv } else { 0 };
                let k = n.min(rest);
                out[op..op + k].fill(v);
                op += k;
            }
            1 => {
                ip += 1;
                if ip >= len {
                    return Err(CodecError::Corrupt("PLIO list ends inside an instruction"));
                }
                let next = i16::from_be_bytes([input[2 * ip], input[2 * ip + 1]]);
                pv = (i32::from(next) << 12) + data;
            }
            2 => pv = pv.wrapping_add(data),
            3 => pv = pv.wrapping_sub(data),
            5 => {
                let k = n.saturating_sub(1).min(rest);
                out[op..op + k].fill(0);
                op += k;
                if op < out.len() {
                    out[op] = pv;
                    op += 1;
                }
            }
            6 | 7 => {
                pv = if word >> 12 == 6 {
                    pv.wrapping_add(data)
                } else {
                    pv.wrapping_sub(data)
                };
                out[op] = pv;
                op += 1;
            }
            _ => return Err(CodecError::Corrupt("PLIO opcode above 7")),
        }
        ip += 1;
    }
    out[op..].fill(0);
    Ok(())
}
