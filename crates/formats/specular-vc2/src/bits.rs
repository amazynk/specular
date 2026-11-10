//! Bit input (Annex A of the Dirac specification): bits most significant first, interleaved
//! exp-Golomb codes, and the bounded block reads of the slice syntax, which give 1 once the
//! block is used up (so every remaining coefficient reads as 0).

use crate::{Result, bad};

/// Header reads: running off the end of the data unit is an error.
pub(crate) struct Reader<'a> {
    d: &'a [u8],
    /// Next bit, counted from the start of `d`.
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(d: &'a [u8]) -> Reader<'a> {
        Reader { d, pos: 0 }
    }

    pub fn bit(&mut self) -> Result<u32> {
        let byte = *self
            .d
            .get(self.pos >> 3)
            .ok_or_else(|| bad("header is truncated"))?;
        let b = (byte >> (7 - (self.pos & 7))) & 1;
        self.pos += 1;
        Ok(u32::from(b))
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.bit()? == 1)
    }

    /// `read_uint`: an unsigned interleaved exp-Golomb code (values beyond 32 bits are refused).
    pub fn uint(&mut self) -> Result<u32> {
        let mut v: u64 = 1;
        while self.bit()? == 0 {
            v = (v << 1) | u64::from(self.bit()?);
            if v > u64::from(u32::MAX) + 1 {
                return Err(bad("header value is out of range"));
            }
        }
        Ok((v - 1) as u32)
    }

    pub fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// `read_uint_lit(n)`: `n` whole bytes, big-endian, after aligning.
    pub fn lit(&mut self, n: usize) -> Result<u32> {
        self.align();
        let at = self.pos >> 3;
        let b = self
            .d
            .get(at..at + n)
            .ok_or_else(|| bad("header is truncated"))?;
        self.pos += 8 * n;
        Ok(b.iter().fold(0u32, |a, &x| (a << 8) | u32::from(x)))
    }

    /// The byte the next read starts in (after aligning).
    pub fn byte_pos(&mut self) -> usize {
        self.align();
        self.pos >> 3
    }
}

/// A block of `left` bits starting at bit `pos` of `d` (`read_bitb` and friends). Bits past
/// the end of the data read as 1 too: a truncated slice gives zero coefficients.
pub(crate) struct Block<'a> {
    d: &'a [u8],
    pos: usize,
    left: usize,
    /// Up to 64 bits, the next one in the top bit.
    acc: u64,
    n: u32,
}

impl<'a> Block<'a> {
    pub fn new(d: &'a [u8], pos: usize, left: usize) -> Block<'a> {
        Block {
            d,
            pos,
            left,
            acc: 0,
            n: 0,
        }
    }

    #[inline]
    fn refill(&mut self) {
        // Whole bytes from a byte-aligned position, else bit by bit up to alignment.
        while self.n <= 56 {
            let byte = self.pos >> 3;
            let off = (self.pos & 7) as u32;
            let take = 8 - off;
            let b = match self.d.get(byte) {
                Some(&b) => u64::from(b << off >> off),
                None => (1u64 << take) - 1,
            };
            self.acc |= b << (64 - self.n - take);
            self.n += take;
            self.pos += take as usize;
        }
    }

    /// `read_bitb`.
    #[inline]
    pub fn bit(&mut self) -> u32 {
        if self.left == 0 {
            return 1;
        }
        self.left -= 1;
        if self.n == 0 {
            self.refill();
        }
        let b = (self.acc >> 63) as u32;
        self.acc <<= 1;
        self.n -= 1;
        b
    }

    /// `read_uintb`.
    #[inline]
    pub fn uint(&mut self) -> u32 {
        let mut v: u32 = 1;
        while self.bit() == 0 {
            v = v.wrapping_shl(1) | self.bit();
        }
        v.wrapping_sub(1)
    }

    /// `read_sintb`.
    #[inline]
    pub fn sint(&mut self) -> i32 {
        let v = self.uint() as i32;
        if v != 0 && self.bit() == 1 {
            v.wrapping_neg()
        } else {
            v
        }
    }

    /// `read_nbits` inside the block.
    pub fn nbits(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit();
        }
        v
    }
}

/// `intlog2(n)`: the smallest m with n ≤ 2^m.
pub(crate) fn intlog2(n: u64) -> u32 {
    if n <= 1 {
        0
    } else {
        64 - (n - 1).leading_zeros()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_golomb_table_a1() {
        // 1 | 001 | 011 | 00001 | 00011 | 01001 | 01011 | 0000001 | 0000011 | 0001001
        let bits = "100101100001000110100101011000000100000110001001";
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, c) in bits.chars().enumerate() {
            if c == '1' {
                bytes[i / 8] |= 0x80 >> (i % 8);
            }
        }
        let mut r = Reader::new(&bytes);
        let mut b = Block::new(&bytes, 0, bits.len());
        for want in 0..10 {
            assert_eq!(r.uint().unwrap(), want);
            assert_eq!(b.uint(), want);
        }
        // The block is used up: everything reads as zero.
        assert_eq!(b.sint(), 0);
        assert_eq!(intlog2(1), 0);
        assert_eq!(intlog2(220), 8);
        assert_eq!(intlog2(256), 8);
        assert_eq!(intlog2(257), 9);
    }
}
