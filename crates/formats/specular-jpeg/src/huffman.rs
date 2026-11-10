//! Huffman tables with a combined lookahead LUT, and the 64-bit entropy bit reader.

use crate::error::{JpegError, Result};
use crate::marker::has_ff;

pub(crate) const LUT_BITS: u32 = 10;
const LUT_SIZE: usize = 1 << LUT_BITS;
/// Lookahead of the multi-symbol skip table (AC tables only).
pub(crate) const SKIP_BITS: u32 = 12;

/// Lookup entry layout (u32):
/// - bits 0..=4: bits to consume. For "fast" entries this is code length + magnitude bits,
///   otherwise the code length alone.
/// - bits 8..=15: the Huffman symbol (run << 4 | size).
/// - bits 16..=31: for fast entries, the decoded (sign-extended) coefficient value, never 0.
///   Non-fast entries have these bits clear.
/// - entry == 0: code longer than LUT_BITS, use the slow path.
pub(crate) struct HuffTable {
    pub lut: [u32; LUT_SIZE],
    /// maxcode[l] = largest code of length l (as i32), -1 if none. Index 17 is a sentinel.
    maxcode: [i32; 18],
    valoffset: [i32; 17],
    values: [u8; 256],
    /// AC tables: multi-symbol skip lookup (sequential scans and progressive AC first scans), indexed by the next
    /// SKIP_BITS bits (see [`skip_entry`] for the layout). Empty for DC tables.
    pub skip: Box<[u32]>,
}

/// Skip-table entry fields: (bits to consume, k-advance, limit, ends with EOB).
/// The entry covers one or more complete AC symbols (code + magnitude bits). It may be applied
/// at coefficient index `k` only if `k + limit < 64`, i.e. every symbol after the first is
/// still inside the block; `limit` is the summed advance of all symbols but the last
/// (all of them when the last is an EOB). Entry 0: the first symbol does not fit.
#[inline(always)]
pub(crate) fn skip_entry(e: u32) -> (u32, u32, u32, bool) {
    (e & 15, (e >> 4) & 127, (e >> 11) & 127, e >> 18 != 0)
}

impl std::fmt::Debug for HuffTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HuffTable")
    }
}

#[inline(always)]
pub(crate) fn extend(v: u32, s: u32) -> i32 {
    // s in 1..=16: values with top bit clear are negative.
    let v = v as i32;
    if v < (1 << (s - 1)) {
        v - (1 << s) + 1
    } else {
        v
    }
}

impl HuffTable {
    pub fn new(counts: &[u8; 16], values: &[u8], is_ac: bool) -> Result<HuffTable> {
        let mut t = HuffTable {
            lut: [0; LUT_SIZE],
            maxcode: [-1; 18],
            valoffset: [0; 17],
            values: [0; 256],
            skip: Box::new([]),
        };
        t.values[..values.len()].copy_from_slice(values);
        let mut code: u32 = 0;
        let mut k = 0usize;
        for l in 1..=16u32 {
            let n = counts[l as usize - 1] as usize;
            if n > 0 {
                t.valoffset[l as usize] = k as i32 - code as i32;
                for _ in 0..n {
                    // Like libjpeg: codes must fit in l bits and the all-ones code is invalid.
                    if code + 1 >= (1 << l) {
                        return Err(JpegError::Malformed("bad Huffman table"));
                    }
                    let sym = values[k];
                    if l <= LUT_BITS {
                        let shift = LUT_BITS - l;
                        let base = (code << shift) as usize;
                        let size = (sym & 15) as u32;
                        let run = (sym >> 4) as u32;
                        for fill in 0..(1usize << shift) {
                            let idx = base + fill;
                            let e = if size != 0 && l + size <= LUT_BITS && (is_ac || run == 0) {
                                // Magnitude bits are fully inside the lookahead window.
                                let bits = (fill as u32) >> (shift - size);
                                let val = extend(bits, size);
                                ((val as i16 as u16 as u32) << 16)
                                    | ((sym as u32) << 8)
                                    | (l + size)
                            } else {
                                ((sym as u32) << 8) | l
                            };
                            t.lut[idx] = e;
                        }
                    }
                    k += 1;
                    code += 1;
                }
                t.maxcode[l as usize] = code as i32 - 1;
            }
            code <<= 1;
        }
        t.maxcode[17] = i32::MAX;
        if is_ac {
            t.skip = (0..1u32 << SKIP_BITS)
                .map(|w| t.build_skip_entry(w))
                .collect();
        }
        Ok(t)
    }

    /// Canonical decode of the symbol at the top of the `avail`-bit window `w`
    /// (left-aligned in SKIP_BITS bits). Returns (symbol, code length) if it fits.
    fn symbol_in(&self, w: u32, avail: u32) -> Option<(u32, u32)> {
        for l in 1..=avail.min(16) {
            let c = (w >> (SKIP_BITS - l)) as i32;
            if c <= self.maxcode[l as usize] {
                let idx = (c + self.valoffset[l as usize]) as usize & 255;
                return Some((self.values[idx] as u32, l));
            }
        }
        None
    }

    fn build_skip_entry(&self, w: u32) -> u32 {
        let (mut used, mut adv, mut lim, mut eob, mut n) = (0u32, 0u32, 0u32, false, 0);
        while adv < 64 {
            let window = (w << used) & ((1 << SKIP_BITS) - 1);
            let Some((sym, len)) = self.symbol_in(window, SKIP_BITS - used) else {
                break;
            };
            let (r, sz) = (sym >> 4, sym & 15);
            if used + len + sz > SKIP_BITS {
                break;
            }
            // EOBn (n > 0) has extra run-length bits (progressive scans; invalid in sequential
            // ones): never part of an entry, the single-symbol path handles it.
            if sz == 0 && r != 15 && r != 0 {
                break;
            }
            used += len + sz;
            n += 1;
            if sz == 0 && r != 15 {
                eob = true;
                lim = adv;
                break;
            }
            lim = adv;
            adv += if sz == 0 { 16 } else { r + 1 };
        }
        if n == 0 {
            return 0;
        }
        used | (adv << 4) | (lim << 11) | ((eob as u32) << 18)
    }

    /// Slow path for codes longer than LUT_BITS. Returns (symbol, code length).
    #[inline(never)]
    #[cold]
    pub fn decode_slow(&self, peek16: u32) -> (u32, u32) {
        let mut l = LUT_BITS + 1;
        while l <= 16 {
            let c = (peek16 >> (16 - l)) as i32;
            if c <= self.maxcode[l as usize] {
                let idx = (c + self.valoffset[l as usize]) as usize & 255;
                return (self.values[idx] as u32, l);
            }
            l += 1;
        }
        // Corrupt data: no code matches. Consume 16 bits and return symbol 0 (EOB / zero DC).
        (0, 16)
    }
}

/// MSB-first bit reader over entropy-coded data with 0xFF00 unstuffing.
///
/// `buf` holds `bits` valid bits left-aligned. The fast refill may leave further *correct*
/// stream bits below the valid ones (they are re-ORed identically by the next refill), so the
/// slow path clears them first.
#[derive(Clone)]
pub(crate) struct BitReader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub buf: u64,
    pub bits: u32,
    /// Stopped at a marker (or end of data); zeros are being fed.
    pub marker_hit: bool,
    /// Zero bytes fed after the marker / end of data.
    pub zero_fill: u32,
}

impl<'a> BitReader<'a> {
    #[inline(always)]
    pub fn refill(&mut self) {
        if let Some(b) = self.data.get(self.pos..self.pos + 8) {
            let w = u64::from_be_bytes(b.try_into().unwrap());
            if !has_ff(w) {
                self.buf |= w >> self.bits;
                let n = (63 - self.bits) >> 3;
                self.pos += n as usize;
                self.bits += n << 3;
                return;
            }
        }
        self.refill_slow();
    }

    #[inline(never)]
    fn refill_slow(&mut self) {
        self.buf &= if self.bits == 0 {
            0
        } else {
            !0u64 << (64 - self.bits)
        };
        while self.bits <= 56 {
            let byte = if self.marker_hit {
                0
            } else if self.pos >= self.data.len() {
                self.marker_hit = true;
                0
            } else {
                let b = self.data[self.pos];
                if b != 0xFF {
                    self.pos += 1;
                    b
                } else if self.data.get(self.pos + 1) == Some(&0) {
                    self.pos += 2;
                    0xFF
                } else {
                    self.marker_hit = true;
                    0
                }
            };
            if self.marker_hit {
                self.zero_fill += 1;
            }
            self.buf |= (byte as u64) << (56 - self.bits);
            self.bits += 8;
        }
    }

    /// Make sure at least `n` (<= 57) bits are buffered.
    #[inline(always)]
    pub fn ensure(&mut self, n: u32) {
        if self.bits < n {
            self.refill();
        }
    }

    #[inline(always)]
    pub fn peek(&self, n: u32) -> u32 {
        (self.buf >> (64 - n)) as u32
    }

    #[inline(always)]
    pub fn consume(&mut self, n: u32) {
        self.buf <<= n;
        self.bits -= n;
    }

    /// Read n bits (1..=16); caller ensured enough bits.
    #[inline(always)]
    pub fn get(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.consume(n);
        v
    }

    #[inline(always)]
    pub fn get_extend(&mut self, s: u32) -> i32 {
        if s == 0 {
            0
        } else {
            let v = self.get(s);
            extend(v, s)
        }
    }

    /// Decode a Huffman symbol (no magnitude). Caller ensured >= 16 bits.
    #[inline(always)]
    pub fn decode_symbol(&mut self, t: &HuffTable) -> u32 {
        let e = t.lut[self.peek(LUT_BITS) as usize];
        if e != 0 {
            let sym = (e >> 8) & 0xFF;
            let len = (e & 31) - if e >> 16 != 0 { sym & 15 } else { 0 };
            self.consume(len);
            sym
        } else {
            let (sym, len) = t.decode_slow(self.peek(16));
            self.consume(len);
            sym
        }
    }

    /// Decode a DC difference. Caller ensured >= 27 bits.
    #[inline(always)]
    pub fn decode_dc_diff(&mut self, t: &HuffTable) -> i32 {
        let e = t.lut[self.peek(LUT_BITS) as usize];
        if e >> 16 != 0 {
            self.consume(e & 31);
            return (e >> 16) as u16 as i16 as i32;
        }
        let s = if e != 0 {
            self.consume(e & 31);
            (e >> 8) & 15
        } else {
            let (sym, len) = t.decode_slow(self.peek(16));
            self.consume(len);
            sym & 15
        };
        if s == 0 {
            0
        } else {
            let s = s.min(16);
            self.ensure(s);
            self.get_extend(s)
        }
    }

    /// True if the decoder consumed bits past the end of the real data.
    #[inline]
    pub fn overrun(&self) -> bool {
        self.zero_fill as u64 * 8 > self.bits as u64
    }

    /// Process a restart marker: drop buffered bits and consume the next RSTn (resyncing
    /// forward to the next RSTn if it is not where expected).
    pub fn restart(&mut self) {
        self.buf = 0;
        self.bits = 0;
        self.marker_hit = false;
        self.zero_fill = 0;
        let d = self.data;
        let mut p = self.pos;
        // Normal case: the marker is right here (possibly after fill bytes).
        let mut q = p;
        while q < d.len() && d[q] == 0xFF {
            q += 1;
        }
        if q > p && q < d.len() && (0xD0..=0xD7).contains(&d[q]) {
            self.pos = q + 1;
            return;
        }
        // Corrupt: search forward for the next RSTn, stop at any other marker.
        while p + 1 < d.len() {
            if d[p] == 0xFF {
                let m = d[p + 1];
                if (0xD0..=0xD7).contains(&m) {
                    self.pos = p + 2;
                    return;
                }
                if m != 0 && m != 0xFF {
                    self.pos = p;
                    return;
                }
            }
            p += 1;
        }
        self.pos = d.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversubscribed_tables_are_rejected() {
        // Three 1-bit codes (fuzzer find: used to index past the lookup table).
        let mut counts = [0u8; 16];
        counts[0] = 3;
        assert!(HuffTable::new(&counts, &[0, 1, 2], true).is_err());
        // All-ones code (two 1-bit codes), rejected like libjpeg.
        counts[0] = 2;
        assert!(HuffTable::new(&counts, &[0, 1], true).is_err());
        // "0", "10", then 9-bit codes from 110000000: 127 fit (the all-ones code excluded).
        let mut counts = [0u8; 16];
        counts[0] = 1;
        counts[1] = 1;
        counts[8] = 127;
        assert!(HuffTable::new(&counts, &[0; 129], true).is_ok());
        counts[8] = 128;
        assert!(HuffTable::new(&counts, &[0; 130], true).is_err());
    }
}
