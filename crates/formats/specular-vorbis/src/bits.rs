//! The Vorbis bit packer's reverse: least significant bit of each byte first (spec section 2).

/// Reads bits from a packet. Reading past the end is the end-of-packet condition: the read
/// fails and so does every later one.
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    /// Next byte to load into `acc`.
    pos: usize,
    /// Loaded bits, the next one lowest. Bits above `n` are either zero or the true following
    /// bits (refills may load them early), so ORing a byte in again is harmless.
    acc: u64,
    n: u32,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            acc: 0,
            n: 0,
        }
    }

    /// Loads bytes until at least 57 bits are present or the packet is exhausted.
    #[inline(always)]
    fn refill(&mut self) {
        if self.n > 56 {
            return;
        }
        if let Some(b) = self.data.get(self.pos..self.pos + 8) {
            let mut w = [0u8; 8];
            w.copy_from_slice(b);
            self.acc |= u64::from_le_bytes(w) << self.n;
            let adv = (63 - self.n) >> 3;
            self.pos += adv as usize;
            self.n += adv * 8;
        } else {
            while self.n <= 56 {
                let Some(&b) = self.data.get(self.pos) else {
                    break;
                };
                self.acc |= u64::from(b) << self.n;
                self.pos += 1;
                self.n += 8;
            }
        }
    }

    /// Reads `bits` (0..=32) bits as an unsigned integer.
    #[inline(always)]
    pub(crate) fn read(&mut self, bits: u32) -> Option<u32> {
        debug_assert!(bits <= 32);
        if bits == 0 {
            return Some(0);
        }
        self.refill();
        if self.n < bits {
            self.n = 0;
            self.acc = 0;
            return None;
        }
        let v = (self.acc & ((1u64 << bits) - 1)) as u32;
        self.acc >>= bits;
        self.n -= bits;
        Some(v)
    }

    #[inline(always)]
    pub(crate) fn bit(&mut self) -> Option<bool> {
        self.read(1).map(|b| b != 0)
    }

    /// The next 32 bits (zeros past the end) and how many of them are real.
    #[inline(always)]
    pub(crate) fn peek32(&mut self) -> (u32, u32) {
        self.refill();
        (self.acc as u32, self.n.min(32))
    }

    /// Drops `bits` bits already seen through `peek32` (`bits` <= the count it gave).
    #[inline(always)]
    pub(crate) fn consume(&mut self, bits: u32) {
        self.acc >>= bits;
        self.n -= bits;
    }

    /// Marks the end of the packet reached.
    pub(crate) fn exhaust(&mut self) {
        self.pos = self.data.len();
        self.n = 0;
        self.acc = 0;
    }
}

/// The position of the highest set bit (1-based); 0 for 0.
pub(crate) fn ilog(x: u32) -> u32 {
    32 - x.leading_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example() {
        // Section 2.1.6/2.1.7: 4 bits 12, 3 bits -1 (7), 7 bits 17, 13 bits 6969.
        let data = [0b1111_1100, 0b0100_1000, 0b1100_1110, 0b0000_0110];
        let mut r = BitReader::new(&data);
        assert_eq!(r.read(4), Some(12));
        assert_eq!(r.read(3), Some(7));
        assert_eq!(r.read(7), Some(17));
        assert_eq!(r.read(13), Some(6969));
        assert_eq!(r.read(5), Some(0));
        assert_eq!(r.read(1), None);
        assert_eq!(r.read(1), None);
        let long: Vec<u8> = (0..40).collect();
        let mut r = BitReader::new(&long);
        for i in 0..40 {
            assert_eq!(r.read(8), Some(i));
        }
        assert_eq!(r.read(8), None);
        assert_eq!(ilog(0), 0);
        assert_eq!(ilog(7), 3);
    }
}
