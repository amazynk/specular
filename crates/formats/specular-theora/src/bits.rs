//! MSB-first bit reader; reads past the end of the packet return zero bits (spec 5.2.4).

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, pos: 0 }
    }

    /// The next 32 bits, zero-padded past the end.
    #[inline(always)]
    pub(crate) fn peek32(&self) -> u32 {
        let byte = self.pos >> 3;
        let w = match self.data.get(byte..byte + 8) {
            Some(s) => u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]),
            None => self.peek_slow(byte),
        };
        ((w << (self.pos & 7)) >> 32) as u32
    }

    #[cold]
    fn peek_slow(&self, byte: usize) -> u64 {
        let mut b = [0u8; 8];
        for (i, v) in b.iter_mut().enumerate() {
            *v = byte
                .checked_add(i)
                .and_then(|j| self.data.get(j))
                .copied()
                .unwrap_or(0);
        }
        u64::from_be_bytes(b)
    }

    /// Reads `n` (0..=32) bits.
    #[inline(always)]
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        let v = (u64::from(self.peek32()) >> (32 - n)) as u32;
        self.pos += n as usize;
        v
    }

    #[inline(always)]
    pub(crate) fn bit(&mut self) -> u32 {
        self.read(1)
    }

    #[inline(always)]
    pub(crate) fn skip(&mut self, n: u32) {
        self.pos += n as usize;
    }

    /// Whether more bits were read than the packet holds.
    pub(crate) fn overrun(&self) -> bool {
        self.pos > self.data.len() * 8
    }
}
