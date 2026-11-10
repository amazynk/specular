//! A big-endian bit reader over one start-code unit. Reads past the end give zero bits and
//! mark the reader as overrun, which the callers check once per macroblock.

#[derive(Clone)]
pub struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load into `cache`.
    pos: usize,
    /// Bits not yet consumed, MSB first.
    cache: u64,
    /// Valid bits in `cache`.
    avail: u32,
    /// Bits read past the end of `data`.
    over: u32,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Bits<'a> {
        let mut b = Bits {
            data,
            pos: 0,
            cache: 0,
            avail: 0,
            over: 0,
        };
        b.refill();
        b
    }

    #[inline(always)]
    fn refill(&mut self) {
        if self.pos + 8 <= self.data.len() && self.avail <= 32 {
            // A whole word at once: as many bytes as fit.
            let mut w = [0u8; 8];
            w.copy_from_slice(&self.data[self.pos..self.pos + 8]);
            let take = (64 - self.avail) / 8;
            // The cache's unused low bits are zero; the new bytes go right after the valid
            // ones, and the part of a byte that doesn't fit is cut off.
            self.cache |= u64::from_be_bytes(w) >> self.avail;
            let keep = self.avail + take * 8;
            if keep < 64 {
                self.cache &= !(u64::MAX >> keep);
            }
            self.pos += take as usize;
            self.avail = keep;
            return;
        }
        while self.avail <= 56 {
            let byte = match self.data.get(self.pos) {
                Some(&b) => b,
                None => {
                    // Zero padding past the end.
                    if self.avail >= 32 {
                        return;
                    }
                    0
                }
            };
            if self.pos >= self.data.len() {
                self.over += 8;
            }
            self.cache |= u64::from(byte) << (56 - self.avail);
            self.avail += 8;
            self.pos += 1;
        }
    }

    /// Skips `n` bits, any number.
    pub fn skip_bits(&mut self, n: usize) {
        let mut left = n;
        while left > 0 {
            let k = left.min(32) as u32;
            self.skip(k);
            left -= k as usize;
        }
    }

    /// The next 32 bits, MSB first, without consuming them.
    #[inline(always)]
    pub fn peek32(&mut self) -> u32 {
        if self.avail < 32 {
            self.refill();
        }
        (self.cache >> 32) as u32
    }

    #[inline(always)]
    pub fn skip(&mut self, n: u32) {
        debug_assert!(n <= 32);
        if self.avail < n {
            self.refill();
        }
        self.cache <<= n;
        self.avail = self.avail.saturating_sub(n);
    }

    /// `n` bits (0..=32) as an unsigned number.
    #[inline(always)]
    pub fn u(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek32() >> (32 - n);
        self.skip(n);
        v
    }

    #[inline(always)]
    pub fn flag(&mut self) -> bool {
        self.u(1) != 0
    }

    /// Bits consumed so far.
    pub fn position(&self) -> usize {
        self.pos * 8 - self.avail as usize
    }

    /// Whether reads went past the end of the data (beyond the zero padding a start code
    /// would supply).
    pub fn overrun(&self) -> bool {
        // The data's own bytes plus the 23 zero bits of a following start code are legal.
        self.position() > self.data.len() * 8 + 23 || self.over > 64
    }

    /// Bits left before the end of the data.
    pub fn left(&self) -> isize {
        self.data.len() as isize * 8 - self.position() as isize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_across_words_and_past_the_end() {
        let data: Vec<u8> = (0..20u8).map(|i| i.wrapping_mul(37) ^ 0x5a).collect();
        let mut want = Vec::new();
        for b in &data {
            for k in (0..8).rev() {
                want.push((b >> k) & 1);
            }
        }
        for step in [1u32, 3, 5, 7, 11, 13, 17, 32] {
            let mut r = Bits::new(&data);
            let mut i = 0;
            while i + step as usize <= want.len() {
                let v = r.u(step);
                let mut e = 0u32;
                for k in 0..step as usize {
                    e = (e << 1) | u32::from(want[i + k]);
                }
                assert_eq!(v, e, "step {step} at {i}");
                i += step as usize;
            }
            assert!(!r.overrun());
            r.skip((want.len() - i) as u32);
            assert_eq!(r.u(8), 0);
        }
        let mut r = Bits::new(&data[..2]);
        r.skip(16);
        assert_eq!(r.left(), 0);
        for _ in 0..10 {
            r.u(32);
        }
        assert!(r.overrun());
    }
}
