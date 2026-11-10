//! Most-significant-bit-first reader over a byte slice. Reading past the end gives zero bits
//! and records the overrun, so damaged streams never panic.

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    /// Bit position from the start of `data`.
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8], byte: usize) -> Self {
        Bits {
            data,
            pos: byte.saturating_mul(8),
        }
    }

    /// The bit position.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Whether reading went past the end of the data.
    pub(crate) fn overrun(&self) -> bool {
        self.pos > self.data.len() * 8
    }

    /// The 64 bits from the current position, zero-filled past the end.
    #[inline]
    fn window(&self) -> u64 {
        let byte = self.pos >> 3;
        let shift = (self.pos & 7) as u32;
        let w = if let Some(s) = self.data.get(byte..byte + 8) {
            let mut b = [0u8; 8];
            b.copy_from_slice(s);
            u64::from_be_bytes(b)
        } else {
            let mut b = [0u8; 8];
            if byte < self.data.len() {
                let s = &self.data[byte..];
                b[..s.len()].copy_from_slice(s);
            }
            u64::from_be_bytes(b)
        };
        if shift == 0 {
            w
        } else {
            let next = self.data.get(byte + 8).copied().unwrap_or(0) as u64;
            (w << shift) | (next >> (8 - shift))
        }
    }

    /// The next `n` bits (0 ..= 32) as an unsigned number.
    #[inline]
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = (self.window() >> (64 - n)) as u32;
        self.pos += n as usize;
        v
    }

    #[inline]
    pub(crate) fn bit(&mut self) -> u32 {
        self.read(1)
    }

    /// Counts one-bits up to the first zero (which is consumed), at most `limit` ones; stops
    /// without consuming a zero when the limit is reached.
    #[inline]
    pub(crate) fn ones(&mut self, limit: u32) -> u32 {
        let mut n = 0;
        loop {
            let w = self.window();
            let run = w.leading_ones().min(limit - n);
            n += run;
            self.pos += run as usize;
            if n >= limit {
                return n;
            }
            if run < 64 {
                // The terminating zero.
                self.pos += 1;
                return n;
            }
            if self.overrun() {
                return n;
            }
        }
    }

    /// Skips to the next multiple of 8 bits.
    pub(crate) fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }

    pub(crate) fn seek(&mut self, bit: usize) {
        self.pos = bit;
    }
}

#[cfg(test)]
mod tests {
    use super::Bits;

    #[test]
    fn reads_msb_first_and_counts_ones() {
        let d = [0b1011_0011, 0xFF, 0xFF, 0b1110_0000];
        let mut b = Bits::new(&d, 0);
        assert_eq!(b.read(1), 1);
        assert_eq!(b.read(3), 0b011);
        assert_eq!(b.ones(32), 0);
        assert_eq!(b.pos(), 5);
        assert_eq!(b.read(3), 0b011);
        assert_eq!(b.ones(32), 19);
        assert_eq!(b.pos(), 28);
        b.align();
        assert_eq!(b.pos(), 32);
        assert!(!b.overrun());
        assert_eq!(b.read(8), 0);
        assert!(b.overrun());
    }

    #[test]
    fn ones_stops_at_limit_without_consuming_a_zero() {
        let d = [0xFF, 0x00];
        let mut b = Bits::new(&d, 0);
        assert_eq!(b.ones(5), 5);
        assert_eq!(b.pos(), 5);
    }
}
