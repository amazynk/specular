//! Big-endian bit reader over one frame's bytes.

/// Reads bits most significant first. Past the end it reads zeros and counts them, so callers
/// check [`Bits::overrun`] after each piece instead of on every read.
pub(crate) struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load.
    pos: usize,
    /// Bits not yet read, at the top. Bits below the top `avail` are zero or the true next
    /// bits of `data`, so loading the same bytes again over them is harmless.
    cache: u64,
    avail: u32,
    /// Zero bytes loaded past the end.
    pad: usize,
}

/// Longest unary run accepted (keeps `q << k` within 64 bits).
const UNARY_LIMIT: u32 = 1 << 31;

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        let mut b = Bits {
            data,
            pos: 0,
            cache: 0,
            avail: 0,
            pad: 0,
        };
        b.refill();
        b
    }

    /// Tops the cache up to at least 56 bits.
    #[inline]
    fn refill(&mut self) {
        if self.avail > 56 {
            return;
        }
        if let Some(chunk) = self.data.get(self.pos..self.pos + 8) {
            let mut a = [0u8; 8];
            a.copy_from_slice(chunk);
            self.cache |= u64::from_be_bytes(a) >> self.avail;
            let n = (63 - self.avail) >> 3;
            self.pos += n as usize;
            self.avail += n * 8;
        } else {
            while self.avail <= 56 {
                let b = match self.data.get(self.pos) {
                    Some(&b) => {
                        self.pos += 1;
                        b
                    }
                    None => {
                        self.pad += 1;
                        0
                    }
                };
                self.cache |= u64::from(b) << (56 - self.avail);
                self.avail += 8;
            }
        }
    }

    /// Bits read so far.
    pub(crate) fn bit_pos(&self) -> usize {
        (self.pos + self.pad) * 8 - self.avail as usize
    }

    /// Whether reads went past the end of the data.
    pub(crate) fn overrun(&self) -> bool {
        self.bit_pos() > self.data.len() * 8
    }

    /// `n` bits (at most 56) as an unsigned number.
    #[inline]
    pub(crate) fn read(&mut self, n: u32) -> u64 {
        debug_assert!(n <= 56);
        if n == 0 {
            return 0;
        }
        if self.avail < n {
            self.refill();
        }
        let v = self.cache >> (64 - n);
        self.cache <<= n;
        self.avail -= n;
        v
    }

    /// `n` bits (at most 56) as a two's complement number.
    #[inline]
    pub(crate) fn read_signed(&mut self, n: u32) -> i64 {
        if n == 0 {
            return 0;
        }
        let v = self.read(n);
        ((v << (64 - n)) as i64) >> (64 - n)
    }

    /// Skips to the next byte boundary.
    pub(crate) fn align(&mut self) {
        let r = (self.bit_pos() & 7) as u32;
        if r != 0 {
            self.read(8 - r);
        }
    }

    /// Zero bits before the next one bit (which is consumed); `None` when the run is absurdly
    /// long or runs past the end.
    pub(crate) fn unary(&mut self) -> Option<u32> {
        let mut q = 0u32;
        loop {
            if self.avail == 0 {
                self.refill();
            }
            let lz = self.cache.leading_zeros();
            if lz < self.avail {
                self.cache = self.cache << lz << 1;
                self.avail -= lz + 1;
                return q.checked_add(lz);
            }
            q = q.checked_add(self.avail)?;
            self.cache = 0;
            self.avail = 0;
            if self.pad > 8 || q > UNARY_LIMIT {
                return None;
            }
            self.refill();
        }
    }

    /// One Rice code word with parameter `k` (at most 30), unfolded.
    #[inline(always)]
    pub(crate) fn rice(&mut self, k: u32) -> Option<i32> {
        if self.avail < 32 {
            self.refill();
        }
        let lz = self.cache.leading_zeros();
        let total = lz + 1 + k;
        let folded = if total <= self.avail {
            let rest = self.cache << lz << 1;
            let low = rest >> (63 - k) >> 1;
            self.cache = rest << k;
            self.avail -= total;
            (u64::from(lz) << k) | low
        } else {
            let q = self.unary()?;
            (u64::from(q) << k) | self.read(k)
        };
        // Valid streams keep residuals within 32 bits; damaged ones just wrap.
        let f = folded as u32;
        Some(((f >> 1) as i32) ^ -((f & 1) as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_across_refills() {
        let data: Vec<u8> = (0..40u8).collect();
        let mut b = Bits::new(&data);
        for (i, &want) in data.iter().enumerate() {
            assert_eq!(b.read(8), u64::from(want), "byte {i}");
        }
        assert!(!b.overrun());
        b.read(1);
        assert!(b.overrun());
    }

    #[test]
    fn rice_and_unary() {
        // 0001 1: q=3, low 1 -> folded 7 -> -4; then 01 1: folded 3 -> -2; then 72 zeros.
        let data = [0b0001_1011, 0b0000_0000, 0, 0, 0, 0, 0, 0, 0, 0, 0x80];
        let mut b = Bits::new(&data);
        assert_eq!(b.rice(1), Some(-4));
        assert_eq!(b.rice(1), Some(-2));
        assert_eq!(b.unary(), Some(72));
        assert_eq!(b.unary(), None);
    }
}
