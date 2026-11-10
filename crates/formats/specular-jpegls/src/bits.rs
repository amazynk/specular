//! Bit reader for one restart interval of JPEG-LS scan data (T.87 A.1).
//!
//! Bits are read MSB first. After an `FF` byte the encoder stuffs a 0 bit, so the next byte
//! carries 7 data bits. The slice never contains a marker (the caller splits the scan at
//! `FF` followed by a byte >= 0x80), so the hot path does not look for one. Past the end of
//! the slice the reader feeds zero bits and remembers how many: consuming one of them is an
//! overrun, which the decoder reports as truncation.

use crate::Error;

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGHS: u64 = 0x8080_8080_8080_8080;

/// Whether any of the eight bytes of `w` is `FF`.
#[inline(always)]
fn has_ff(w: u64) -> bool {
    let x = !w;
    x.wrapping_sub(ONES) & !x & HIGHS != 0
}

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    /// Unread bits, left-aligned; the bits below `count` are zero.
    cache: u64,
    count: u32,
    /// Zero bits fed after the end of `data` (all at the tail of the stream).
    phantom: u64,
    /// The last byte taken was `FF`, so the next one has a stuffed bit.
    ff: bool,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Bits<'a> {
        Bits {
            data,
            pos: 0,
            cache: 0,
            count: 0,
            phantom: 0,
            ff: false,
        }
    }

    /// Bits past the end of the data have been consumed.
    #[inline]
    pub(crate) fn overrun(&self) -> bool {
        (self.count as u64) < self.phantom
    }

    /// The error for undecodable bits: data that ran out is truncation, not corruption.
    #[cold]
    pub(crate) fn corrupt(&self, what: &'static str) -> Error {
        if self.overrun() {
            Error::Truncated
        } else {
            Error::Malformed(what)
        }
    }

    /// Fills the cache to at least 57 bits.
    #[inline(always)]
    fn refill(&mut self) {
        if !self.ff
            && let Some(b) = self.data.get(self.pos..self.pos + 8)
        {
            let w = u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
            if !has_ff(w) {
                let take = (64 - self.count) & !7;
                self.cache |= (w >> (64 - take)) << (64 - take - self.count);
                self.count += take;
                self.pos += (take / 8) as usize;
                return;
            }
        }
        self.refill_slow();
    }

    #[inline(never)]
    fn refill_slow(&mut self) {
        while self.count <= 56 {
            if let Some(&b) = self.data.get(self.pos) {
                self.pos += 1;
                if self.ff {
                    self.cache |= ((b & 0x7F) as u64) << (57 - self.count);
                    self.count += 7;
                } else {
                    self.cache |= (b as u64) << (56 - self.count);
                    self.count += 8;
                }
                self.ff = b == 0xFF;
            } else {
                self.count += 8;
                self.phantom += 8;
            }
        }
    }

    #[inline(always)]
    pub(crate) fn bit(&mut self) -> bool {
        if self.count == 0 {
            self.refill();
        }
        let b = self.cache >> 63 != 0;
        self.cache <<= 1;
        self.count -= 1;
        b
    }

    /// Reads `n` (<= 32) bits.
    #[inline(always)]
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        if self.count < n {
            self.refill();
        }
        let v = ((self.cache >> 32) >> (32 - n)) as u32;
        self.cache <<= n;
        self.count -= n;
        v
    }

    /// Reads a limited-length Golomb code LG(k, LIMIT) (T.87 A.5.3). `escape` is
    /// LIMIT - qbpp - 1: a unary prefix of that length is followed by the value - 1 in `qbpp`
    /// bits instead of the k low bits. `k` must be <= 32.
    #[inline(always)]
    pub(crate) fn golomb(&mut self, k: u32, escape: u32, qbpp: u32) -> Result<u32, Error> {
        if self.count <= 56 {
            self.refill();
        }
        let z = self.cache.leading_zeros();
        let used = z + 1 + k;
        if z < escape && used <= self.count {
            let c = self.cache << z << 1;
            let low = ((c >> 32) >> (32 - k)) as u32;
            self.cache = c << k;
            self.count -= used;
            return Ok(z.wrapping_shl(k) | low);
        }
        self.golomb_slow(k, escape, qbpp)
    }

    #[inline(never)]
    fn golomb_slow(&mut self, k: u32, escape: u32, qbpp: u32) -> Result<u32, Error> {
        let mut z = 0u32;
        loop {
            if self.count <= 56 {
                self.refill();
            }
            let lz = self.cache.leading_zeros();
            if lz < self.count {
                z += lz;
                self.cache <<= lz;
                self.count -= lz;
                break;
            }
            z += self.count;
            self.cache = 0;
            self.count = 0;
            if z > escape {
                return Err(self.corrupt("Golomb code longer than LIMIT"));
            }
        }
        if z > escape {
            return Err(self.corrupt("Golomb code longer than LIMIT"));
        }
        self.cache <<= 1;
        self.count -= 1;
        if z < escape {
            let low = self.read(k);
            Ok(z.wrapping_shl(k) | low)
        } else {
            Ok(self.read(qbpp) + 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuffing_and_phantom_bits() {
        // FF 7F: the 0 after FF is stuffed, so the bits are 8 ones then 7 ones.
        let mut b = Bits::new(&[0xFF, 0x7F, 0x80]);
        assert_eq!(b.read(15), 0x7FFF);
        assert!(b.bit());
        assert_eq!(b.read(7), 0);
        assert!(!b.overrun());
        assert_eq!(b.read(1), 0);
        assert!(b.overrun());
    }

    #[test]
    fn fast_and_slow_refills_agree() {
        let data: Vec<u8> = (0..200u32)
            .map(|i| match i % 37 {
                5 => 0xFF,
                6 => (i * 7 % 128) as u8,
                _ => (i * 151 % 251) as u8,
            })
            .collect();
        // Reference: expand to a bit vector by hand.
        let mut bits = Vec::new();
        let mut prev_ff = false;
        for &b in &data {
            let n = if prev_ff { 7 } else { 8 };
            for i in (0..n).rev() {
                bits.push((b >> i) & 1 == 1);
            }
            prev_ff = b == 0xFF;
        }
        for step in [1u32, 3, 7, 13, 32] {
            let mut r = Bits::new(&data);
            let mut at = 0;
            while at + step as usize <= bits.len() {
                let v = r.read(step);
                let want = bits[at..at + step as usize]
                    .iter()
                    .fold(0u32, |a, &b| (a << 1) | b as u32);
                assert_eq!(v, want, "step {step} at {at}");
                at += step as usize;
            }
            assert!(!r.overrun());
        }
    }

    #[test]
    fn golomb_codes() {
        // k = 2: prefix 3 zeros, 1, then 10 → (3 << 2) | 2 = 14.
        let mut b = Bits::new(&[0b0001_1000]);
        assert_eq!(b.golomb(2, 20, 8).unwrap(), 14);
        // Escape: prefix of `escape` zeros, 1, then qbpp bits of value - 1.
        let mut b = Bits::new(&[0b0001_0000, 0b0101_0000]);
        assert_eq!(b.golomb(2, 3, 8).unwrap(), 6);
        // Prefix too long.
        let mut b = Bits::new(&[0, 0, 0x80]);
        assert_eq!(
            b.golomb(0, 7, 8),
            Err(Error::Malformed("Golomb code longer than LIMIT"))
        );
        let mut b = Bits::new(&[0]);
        assert_eq!(b.golomb(0, 23, 8), Err(Error::Truncated));
    }
}
