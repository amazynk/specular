//! MSB-first bit reader. Reads past the end yield zero bits; `overrun` reports them.

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load into `acc`.
    pos: usize,
    /// Bits left-aligned at the top of the accumulator.
    acc: u64,
    /// Valid bits in `acc`.
    n: u32,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        let mut b = Bits {
            data,
            pos: 0,
            acc: 0,
            n: 0,
        };
        b.refill();
        b
    }

    #[inline]
    fn refill(&mut self) {
        if let Some(w) = self.data.get(self.pos..self.pos + 8) {
            let v = u64::from_be_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]);
            let take = (64 - self.n) / 8;
            let bits = take * 8;
            let chunk = if bits == 64 {
                v
            } else {
                v >> (64 - bits) << (64 - bits)
            };
            self.acc |= chunk >> self.n;
            self.n += bits;
            self.pos += take as usize;
            return;
        }
        while self.n <= 56 {
            let byte = self.data.get(self.pos).copied().unwrap_or(0);
            self.acc |= (byte as u64) << (56 - self.n);
            self.n += 8;
            self.pos += 1;
        }
    }

    /// The next `k` (1..=32) bits without consuming them.
    #[inline]
    pub(crate) fn peek(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.refill();
        }
        (self.acc >> (64 - k)) as u32
    }

    #[inline]
    pub(crate) fn skip(&mut self, k: u32) {
        if self.n < k {
            self.refill();
        }
        self.acc <<= k;
        self.n -= k;
    }

    /// Reads `k` (0..=32) bits.
    #[inline]
    pub(crate) fn u(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        let v = self.peek(k);
        self.skip(k);
        v
    }

    #[inline]
    pub(crate) fn bit(&mut self) -> bool {
        self.u(1) != 0
    }

    /// Bit position from the start of the data.
    pub(crate) fn bit_pos(&self) -> u64 {
        self.pos as u64 * 8 - self.n as u64
    }

    pub(crate) fn align(&mut self) {
        let r = (self.bit_pos() % 8) as u32;
        if r != 0 {
            self.skip(8 - r);
        }
    }

    /// Whether more bits were consumed than the data holds.
    pub(crate) fn overrun(&self) -> bool {
        self.bit_pos() > self.data.len() as u64 * 8
    }
}

#[cfg(test)]
mod tests {
    use super::Bits;

    #[test]
    fn reads_msb_first() {
        let d = [
            0b1010_0000u8,
            0xFF,
            0x00,
            0x12,
            0x34,
            0x56,
            0x78,
            0x9A,
            0xBC,
            0xDE,
            0xF0,
        ];
        let mut b = Bits::new(&d);
        assert_eq!(b.u(1), 1);
        assert_eq!(b.u(2), 1);
        b.align();
        assert_eq!(b.u(8), 0xFF);
        assert_eq!(b.u(16), 0x0012);
        assert_eq!(b.u(32), 0x3456_789A);
        assert_eq!(b.u(24), 0xBC_DEF0);
        assert!(!b.overrun());
        assert_eq!(b.u(8), 0);
        assert!(b.overrun());
    }
}
