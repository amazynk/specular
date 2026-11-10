//! Canonical Huffman tables (JPEG DHT style: code counts per length + symbols) with an 11-bit
//! lookup table and a combined "code + extra bits" fast path for lossless-JPEG-style differences.

use crate::bits::BitReader;

pub(crate) const LUT_BITS: u32 = 11;

#[derive(Clone, Debug)]
pub(crate) struct Huffman {
    /// `(len << 8) | symbol` for codes up to LUT_BITS long; 0 = longer code.
    lut: Vec<u16>,
    /// Fast differences: `(diff << 8) | total_len` when code + extra bits fit LUT_BITS; 0 = no.
    fast: Vec<i32>,
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    symbols: Vec<u8>,
}

/// JPEG sign extension of an `s`-bit magnitude category value.
#[inline(always)]
pub(crate) fn extend(v: u32, s: u32) -> i32 {
    if s == 0 {
        0
    } else if v < (1 << (s - 1)) {
        v as i32 - (1 << s) + 1
    } else {
        v as i32
    }
}

impl Huffman {
    /// Build from 16 code counts and the symbols. `with_fast` precomputes the combined
    /// difference table (for symbol = number of extra bits, as in lossless JPEG).
    pub fn new(counts: &[u8; 16], symbols: &[u8], with_fast: bool) -> Option<Huffman> {
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        if total == 0 || total > 256 || symbols.len() < total {
            return None;
        }
        let symbols = symbols[..total].to_vec();
        let mut lut = vec![0u16; 1 << LUT_BITS];
        let mut maxcode = [-1i32; 18];
        let mut valptr = [0i32; 17];
        let mut mincode = [0i32; 17];
        let mut code: u32 = 0;
        let mut k = 0usize;
        for len in 1..=16u32 {
            let n = counts[len as usize - 1] as usize;
            valptr[len as usize] = k as i32;
            mincode[len as usize] = code as i32;
            for _ in 0..n {
                if code >= (1 << len) {
                    return None; // over-subscribed
                }
                if len <= LUT_BITS {
                    let shift = LUT_BITS - len;
                    let base = (code << shift) as usize;
                    for e in &mut lut[base..base + (1 << shift)] {
                        *e = ((len as u16) << 8) | symbols[k] as u16;
                    }
                }
                code += 1;
                k += 1;
            }
            if n > 0 {
                maxcode[len as usize] = code as i32 - 1;
            }
            code <<= 1;
        }
        maxcode[17] = i32::MAX;
        let mut fast = Vec::new();
        if with_fast {
            fast = vec![0i32; 1 << LUT_BITS];
            for (i, f) in fast.iter_mut().enumerate() {
                let e = lut[i];
                let len = (e >> 8) as u32;
                let s = (e & 0xff) as u32;
                if len == 0 || s > 15 || len + s > LUT_BITS {
                    continue;
                }
                let extra = ((i as u32) >> (LUT_BITS - len - s)) & ((1u32 << s) - 1);
                let diff = extend(extra, s);
                *f = (diff << 8) | (len + s) as i32;
            }
        }
        Some(Huffman {
            lut,
            fast,
            maxcode,
            valptr,
            mincode,
            symbols,
        })
    }

    /// Decode one symbol. Invalid codes decode as symbol 0 (after consuming 16 bits).
    #[inline(always)]
    pub fn decode(&self, br: &mut BitReader) -> u32 {
        if br.nbits() < 16 {
            br.refill();
        }
        let e = self.lut[br.peek(LUT_BITS) as usize];
        if e != 0 {
            br.consume((e >> 8) as u32);
            return (e & 0xff) as u32;
        }
        self.decode_slow(br)
    }

    #[cold]
    fn decode_slow(&self, br: &mut BitReader) -> u32 {
        let bits = br.peek(16) as i32;
        for len in LUT_BITS + 1..=16 {
            let code = bits >> (16 - len);
            if code <= self.maxcode[len as usize] {
                br.consume(len);
                let idx = self.valptr[len as usize] + code - self.mincode[len as usize];
                return self.symbols.get(idx as usize).copied().unwrap_or(0) as u32;
            }
        }
        br.consume(16);
        0
    }

    /// Decode a lossless-JPEG difference: a category symbol `s` then `s` extra bits
    /// (`s == 16` means 32768 with no extra bits).
    #[inline(always)]
    pub fn diff(&self, br: &mut BitReader) -> i32 {
        if br.nbits() < 32 {
            br.refill();
        }
        let f = self.fast[br.peek(LUT_BITS) as usize];
        if f != 0 {
            br.consume((f & 0xff) as u32);
            return f >> 8;
        }
        let s = self.decode(br);
        match s {
            0 => 0,
            16 => 32768,
            s if s < 16 => extend(br.bits(s), s),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversubscribed() {
        let mut c = [0u8; 16];
        c[0] = 3; // three 1-bit codes
        assert!(Huffman::new(&c, &[0, 1, 2], true).is_none());
    }

    #[test]
    fn decodes_long_codes() {
        // One code of each length 1..=15 (+ two of length 16): symbol = length.
        let mut c = [1u8; 16];
        c[15] = 2;
        let syms: Vec<u8> = (1..=17).collect();
        let h = Huffman::new(&c, &syms, false).unwrap();
        // Code for length 13: twelve 1s then a 0.
        let mut d = vec![0xff, 0xf0, 0x00, 0x00];
        d.extend_from_slice(&[0; 8]);
        let mut br = BitReader::new(&d, false);
        assert_eq!(h.decode(&mut br), 13);
    }
}
