//! The bit reader for the uncompressed header (section 9.1) and the boolean decoder
//! (section 9.2).

use crate::{Error, Result, malformed};

/// Reads `f(n)` fields, most significant bit first.
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> BitReader<'a> {
        BitReader { data, pos: 0 }
    }

    /// Bits read so far.
    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    pub(crate) fn bit(&mut self) -> Result<u32> {
        let byte = *self
            .data
            .get(self.pos >> 3)
            .ok_or_else(|| malformed("frame header cut short"))?;
        let b = (byte >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Ok(b as u32)
    }

    pub(crate) fn flag(&mut self) -> Result<bool> {
        Ok(self.bit()? == 1)
    }

    pub(crate) fn f(&mut self, n: u32) -> Result<u32> {
        let mut x = 0;
        for _ in 0..n {
            x = (x << 1) | self.bit()?;
        }
        Ok(x)
    }

    /// `s(n)`: a magnitude of n bits then a sign bit.
    pub(crate) fn s(&mut self, n: u32) -> Result<i32> {
        let v = self.f(n)? as i32;
        Ok(if self.flag()? { -v } else { v })
    }
}

/// The boolean (arithmetic) decoder of section 9.2.
///
/// `value` holds the next bits of the stream left-aligned: its top 8 bits are the
/// specification's BoolValue and the bits below them are look-ahead; `bits` counts the valid
/// bits. Past the end of the data the stream reads as zeros, as the specification requires.
pub(crate) struct BoolDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    value: u64,
    bits: i32,
    range: u32,
}

impl<'a> BoolDecoder<'a> {
    /// `init_bool( sz )` with sz = `data.len()`, including the marker bit.
    pub(crate) fn new(data: &'a [u8]) -> Result<BoolDecoder<'a>> {
        if data.is_empty() {
            return Err(malformed("empty arithmetic-coded partition"));
        }
        let mut d = BoolDecoder {
            data,
            pos: 0,
            value: 0,
            bits: 0,
            range: 255,
        };
        d.fill();
        if d.read(128) {
            return Err(malformed("bad marker bit in an arithmetic-coded partition"));
        }
        Ok(d)
    }

    #[inline(always)]
    fn fill(&mut self) {
        if self.pos + 8 <= self.data.len() {
            let mut b = [0u8; 8];
            b.copy_from_slice(&self.data[self.pos..self.pos + 8]);
            // Bits beyond the whole bytes counted here are loaded too; the next fill ORs in the
            // same bits at the same places, which changes nothing.
            self.value |= u64::from_be_bytes(b) >> self.bits;
            let n = ((64 - self.bits) >> 3) as usize;
            self.pos += n;
            self.bits += 8 * n as i32;
        } else {
            while self.bits <= 56 {
                if self.pos < self.data.len() {
                    self.value |= (self.data[self.pos] as u64) << (56 - self.bits);
                    self.pos += 1;
                    self.bits += 8;
                } else {
                    // Zeros from here on: enough of them to never refill again.
                    self.bits = 1 << 20;
                    break;
                }
            }
        }
    }

    /// `read_bool( p )`.
    #[inline(always)]
    pub(crate) fn read(&mut self, p: u8) -> bool {
        if self.bits < 16 {
            self.fill();
        }
        let split = 1 + (((self.range - 1) * p as u32) >> 8);
        let big = (split as u64) << 56;
        let bit = if self.value >= big {
            self.range -= split;
            self.value -= big;
            true
        } else {
            self.range = split;
            false
        };
        let shift = (self.range as u8).leading_zeros();
        self.range <<= shift;
        self.value <<= shift;
        self.bits -= shift as i32;
        bit
    }

    /// `read_literal( n )`: n bools at probability 128.
    pub(crate) fn literal(&mut self, n: u32) -> u32 {
        let mut x = 0;
        for _ in 0..n {
            x = (x << 1) | self.read(128) as u32;
        }
        x
    }

    /// Decodes a tree (section 9.3.3); `prob(node)` gives the probability of node `n >> 1`.
    #[inline(always)]
    pub(crate) fn tree(&mut self, tree: &[i8], mut prob: impl FnMut(usize) -> u8) -> usize {
        let mut n = 0i32;
        loop {
            n = tree[(n + self.read(prob((n >> 1) as usize)) as i32) as usize] as i32;
            if n <= 0 {
                return (-n) as usize;
            }
        }
    }
}

/// Fails with `Malformed(msg)` unless `c` holds.
pub(crate) fn check(c: bool, msg: &str) -> Result<()> {
    if c {
        Ok(())
    } else {
        Err(Error::Malformed(msg.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification's boolean decoder, bit by bit (section 9.2), for comparison.
    struct SpecBool<'a> {
        data: &'a [u8],
        bitpos: usize,
        value: u32,
        range: u32,
        max_bits: i64,
    }

    impl SpecBool<'_> {
        fn bit(&mut self) -> u32 {
            let b = self
                .data
                .get(self.bitpos >> 3)
                .map_or(0, |b| (b >> (7 - (self.bitpos & 7))) & 1);
            self.bitpos += 1;
            b as u32
        }
        fn read(&mut self, p: u8) -> bool {
            let split = 1 + (((self.range - 1) * p as u32) >> 8);
            let bit = if self.value < split {
                self.range = split;
                false
            } else {
                self.range -= split;
                self.value -= split;
                true
            };
            while self.range < 128 {
                let nb = if self.max_bits > 0 {
                    self.max_bits -= 1;
                    self.bit()
                } else {
                    0
                };
                self.range *= 2;
                self.value = (self.value << 1) + nb;
            }
            bit
        }
    }

    #[test]
    fn matches_the_specification_bit_by_bit() {
        let mut seed = 0x1234_5678u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for len in [1usize, 2, 7, 9, 64, 300] {
            let data: Vec<u8> = (0..len).map(|_| rnd() as u8).collect();
            let mut data = data;
            data[0] &= 0x7f; // marker bit reads as 0 for p = 128 when the first bit is 0
            let mut spec = SpecBool {
                data: &data,
                bitpos: 8,
                value: data[0] as u32,
                range: 255,
                max_bits: 8 * len as i64 - 8,
            };
            assert!(!spec.read(128));
            let mut ours = BoolDecoder::new(&data).unwrap();
            for _ in 0..4000 {
                let p = (rnd() % 255 + 1) as u8;
                assert_eq!(ours.read(p), spec.read(p));
            }
        }
    }
}
