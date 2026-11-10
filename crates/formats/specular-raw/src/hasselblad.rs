//! Hasselblad lossless compression (3FR and FFF, TIFF compression 7).
//!
//! The stream starts with JPEG markers (SOF3 and DHT give the size and the one Huffman table),
//! but the entropy-coded data is read MSB-first from little-endian 32-bit words, without byte
//! stuffing. Each pair of columns codes two lengths and then two differences; the prediction is
//! the previous sample of the same colour in the row (`0x8000` at the row start), plus, for
//! predictor 11, the change between the same two columns two rows up.

use crate::bits::BitReader;
use crate::error::{RawError, Result};
use crate::huffman::Huffman;
use crate::ljpeg;

/// One difference of `len` bits: values with the top bit clear are negative.
#[inline(always)]
fn diff(br: &mut BitReader, len: u32) -> i32 {
    if len == 0 {
        return 0;
    }
    let len = len.min(16);
    let mut v = br.bits(len) as i32;
    if v & (1 << (len - 1)) == 0 {
        v -= (1 << len) - 1;
    }
    if v == 65535 { -32768 } else { v }
}

/// Decodes `w x h` samples into `out` (row-major). Returns whether the data ran short.
pub(crate) fn decode(src: &[u8], w: usize, h: usize, base: i32, out: &mut [u16]) -> Result<bool> {
    let p = ljpeg::parse_vendor(src)?;
    let spec = p.tables[p.header.tables[0] as usize]
        .or(p.tables[0])
        .ok_or(RawError::Malformed("Hasselblad: no Huffman table"))?;
    let huff = Huffman::new(&spec.counts, &spec.symbols[..spec.n], false)
        .ok_or(RawError::Malformed("Hasselblad: bad Huffman table"))?;
    let psv = p.header.predictor;
    let data = src.get(p.header.data_start..).unwrap_or(&[]);
    let mut br = BitReader::new_le32(data);
    // Rows r - 2, r - 1 and r of the running predictions (wider than 16 bits on purpose).
    let ww = w + (w & 1);
    let mut rows = vec![0i32; ww * 3];
    for row in 0..h {
        rows.copy_within(ww..3 * ww, 0);
        let (older, cur) = rows.split_at_mut(2 * ww);
        let up2 = &older[..ww];
        let cur = &mut cur[..ww];
        let dst = &mut out[row * w..row * w + w];
        for col in (0..ww).step_by(2) {
            let l0 = huff.decode(&mut br);
            let l1 = huff.decode(&mut br);
            let d = [diff(&mut br, l0), diff(&mut br, l1)];
            for (k, &dk) in d.iter().enumerate() {
                let s = col + k;
                let mut pred = if col == 0 { 0x8000 + base } else { cur[s - 2] };
                if psv == 11 && col > 0 && row > 1 {
                    pred += up2[s] / 2 - up2[s - 2] / 2;
                }
                pred += dk;
                cur[s] = pred;
                if let Some(o) = dst.get_mut(s) {
                    *o = pred as u16;
                }
            }
        }
        if br.overrun() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stream whose Huffman table has one 1-bit code for length 0 and one for length 2.
    fn stream(words: &[u32], w: u16, h: u16) -> Vec<u8> {
        let mut s = vec![0xff, 0xd8, 0xff, 0xc3, 0, 11, 16];
        s.extend_from_slice(&h.to_be_bytes());
        s.extend_from_slice(&w.to_be_bytes());
        s.extend_from_slice(&[1, 0, 0x11, 0]);
        s.extend_from_slice(&[0xff, 0xc4, 0, 21, 0, 2]);
        s.extend_from_slice(&[0; 15]);
        s.extend_from_slice(&[0, 2]);
        s.extend_from_slice(&[0xff, 0xda, 0, 8, 1, 0, 0, 8, 0, 0]);
        for wd in words {
            s.extend_from_slice(&wd.to_le_bytes());
        }
        s
    }

    #[test]
    fn decodes_pairs() {
        // Row of 2: codes "0" (len 0) and "1" (len 2), then diff "11" = +3.
        // Bits: 0 1 11 -> 0111 then zeros.
        let src = stream(&[0x7000_0000, 0], 2, 1);
        let mut out = vec![0u16; 2];
        let t = decode(&src, 2, 1, 0, &mut out).unwrap();
        assert!(!t);
        assert_eq!(out, [0x8000, 0x8003]);
        // "01" with len 2 is negative: 1 - 3 = -2.
        let src = stream(&[0x5000_0000, 0], 2, 1);
        decode(&src, 2, 1, 0, &mut out).unwrap();
        assert_eq!(out, [0x8000, 0x8000 - 2]);
    }

    #[test]
    fn truncated_and_hostile() {
        let src = stream(&[], 64, 64);
        let mut out = vec![0u16; 64 * 64];
        assert!(decode(&src, 64, 64, 0, &mut out).unwrap());
        assert!(decode(&src[..10], 64, 64, 0, &mut out).is_err());
        assert!(decode(&[], 64, 64, 0, &mut out).is_err());
    }
}
