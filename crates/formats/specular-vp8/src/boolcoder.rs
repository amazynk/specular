//! The boolean entropy decoder (RFC 6386 section 7).

/// Reads bools from one partition. Past the end of the data it reads zeros and counts how
/// far it went, so a truncated partition can be told from a complete one.
#[derive(Clone)]
pub(crate) struct BoolDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    /// Undecoded bits, left-aligned: the top 8 bits are compared with the split.
    value: u64,
    /// Valid bits in `value` beyond the top 8 (negative: refill before the next bool).
    bits: i32,
    range: u32,
    /// Zero bits supplied past the end of `data`.
    padded: u64,
}

impl<'a> BoolDecoder<'a> {
    pub(crate) fn new(data: &'a [u8]) -> BoolDecoder<'a> {
        let mut d = BoolDecoder {
            data,
            pos: 0,
            value: 0,
            bits: -8,
            range: 255,
            padded: 0,
        };
        d.fill();
        d
    }

    #[inline(never)]
    fn fill(&mut self) {
        // The next byte goes just below the valid bits.
        let mut shift = 48 - self.bits;
        if self.pos + 8 <= self.data.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&self.data[self.pos..self.pos + 8]);
            let word = u64::from_be_bytes(b);
            let n = (shift / 8 + 1) as usize;
            // bits >= -8 before a refill, so shift <= 56 and n <= 8.
            let take = word >> (64 - 8 * n);
            self.value |= take << (shift - 8 * (n as i32 - 1));
            self.pos += n;
            self.bits += 8 * n as i32;
            return;
        }
        while shift >= 0 {
            if self.pos < self.data.len() {
                self.value |= (self.data[self.pos] as u64) << shift;
                self.pos += 1;
            } else {
                self.padded += 8;
            }
            self.bits += 8;
            shift -= 8;
        }
    }

    /// One bool whose probability of being 0 is `prob / 256`.
    #[inline(always)]
    pub(crate) fn read(&mut self, prob: u8) -> bool {
        let split = 1 + (((self.range - 1) * prob as u32) >> 8);
        if self.bits < 0 {
            self.fill();
        }
        let big = (split as u64) << 56;
        let bit = if self.value >= big {
            self.range -= split;
            self.value -= big;
            true
        } else {
            self.range = split;
            false
        };
        let shift = self.range.leading_zeros() - 24;
        self.range <<= shift;
        self.value <<= shift;
        self.bits -= shift as i32;
        bit
    }

    #[inline]
    pub(crate) fn flag(&mut self) -> bool {
        self.read(128)
    }

    /// An `n`-bit unsigned literal, most significant bit first.
    pub(crate) fn literal(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.flag() as u32;
        }
        v
    }

    /// An `n`-bit magnitude followed by a sign bit (1: negative).
    pub(crate) fn signed(&mut self, n: u32) -> i32 {
        let v = self.literal(n) as i32;
        if self.flag() { -v } else { v }
    }

    /// A flag, then (when set) an `n`-bit signed value; 0 when the flag is clear.
    pub(crate) fn optional_signed(&mut self, n: u32) -> i32 {
        if self.flag() { self.signed(n) } else { 0 }
    }

    /// A tree-coded value: `tree` holds child indices (positive, even) and leaves as
    /// `-value`; `probs[i / 2]` is the probability at node `i`.
    #[inline]
    pub(crate) fn tree(&mut self, tree: &[i8], probs: &[u8]) -> u8 {
        let mut i = 0usize;
        loop {
            let n = tree[i + self.read(probs[i >> 1]) as usize];
            if n <= 0 {
                return n.unsigned_abs();
            }
            i = n as usize;
        }
    }

    /// Bits consumed past the end of the data (beyond the two bytes of look-ahead a
    /// decoder may legitimately have loaded).
    pub(crate) fn overrun_bits(&self) -> u64 {
        let loaded = self.pos as u64 * 8 + self.padded;
        let used = loaded - (self.bits + 8) as u64;
        used.saturating_sub(self.data.len() as u64 * 8 + 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RFC's encoder (section 7.3), to round-trip against.
    struct Encoder {
        out: Vec<u8>,
        range: u32,
        bottom: u32,
        bit_count: i32,
    }

    impl Encoder {
        fn new() -> Encoder {
            Encoder {
                out: Vec::new(),
                range: 255,
                bottom: 0,
                bit_count: 24,
            }
        }
        fn carry(&mut self) {
            let mut i = self.out.len();
            while i > 0 {
                i -= 1;
                if self.out[i] == 255 {
                    self.out[i] = 0;
                } else {
                    self.out[i] += 1;
                    break;
                }
            }
        }
        fn put(&mut self, prob: u8, bit: bool) {
            let split = 1 + (((self.range - 1) * prob as u32) >> 8);
            if bit {
                self.bottom = self.bottom.wrapping_add(split);
                self.range -= split;
            } else {
                self.range = split;
            }
            while self.range < 128 {
                self.range <<= 1;
                if self.bottom & (1 << 31) != 0 {
                    self.carry();
                }
                self.bottom <<= 1;
                self.bit_count -= 1;
                if self.bit_count == 0 {
                    self.out.push((self.bottom >> 24) as u8);
                    self.bottom &= (1 << 24) - 1;
                    self.bit_count = 8;
                }
            }
        }
        fn finish(mut self) -> Vec<u8> {
            for _ in 0..32 {
                self.put(128, false);
            }
            self.out
        }
    }

    #[test]
    fn round_trip() {
        let mut e = Encoder::new();
        let mut want = Vec::new();
        let mut x = 12345u32;
        for i in 0..20000 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            let prob = ((x >> 8) & 255).max(1) as u8;
            let bit = (x >> 20) % 256 >= prob as u32 || i % 7 == 0;
            e.put(prob, bit);
            want.push((prob, bit));
        }
        let data = e.finish();
        let mut d = BoolDecoder::new(&data);
        for (i, &(p, b)) in want.iter().enumerate() {
            assert_eq!(d.read(p), b, "bool {i}");
        }
        assert_eq!(d.overrun_bits(), 0);
    }

    #[test]
    fn past_the_end_reads_zeros() {
        let mut d = BoolDecoder::new(&[]);
        for _ in 0..100 {
            assert!(!d.flag());
        }
        assert!(d.overrun_bits() > 0);
    }
}
