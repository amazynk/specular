//! MSB-first bit reader. Reads past the end yield zero bits and set the position beyond the
//! data, which callers check with [`Bits::overrun`] at element boundaries; nothing here panics.

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0 }
    }

    /// The next `n` (≤ 32) bits without consuming them, zero-padded past the end.
    #[inline]
    pub(crate) fn peek(&self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let byte = self.pos >> 3;
        let word = match self
            .data
            .get(byte..byte.saturating_add(8))
            .and_then(|s| <[u8; 8]>::try_from(s).ok())
        {
            Some(w) => u64::from_be_bytes(w),
            None => {
                let mut word = 0u64;
                for i in 0..8 {
                    word = (word << 8) | u64::from(self.data.get(byte + i).copied().unwrap_or(0));
                }
                word
            }
        };
        let shifted = word << (self.pos & 7);
        (shifted >> (64 - n)) as u32
    }

    #[inline]
    pub(crate) fn skip(&mut self, n: usize) {
        self.pos = self.pos.saturating_add(n);
    }

    #[inline]
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        let v = self.peek(n);
        self.pos = self.pos.saturating_add(n as usize);
        v
    }

    #[inline]
    pub(crate) fn bit(&mut self) -> bool {
        self.read(1) != 0
    }

    /// Bits read so far.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Bits left; negative once reads ran past the end.
    pub(crate) fn left(&self) -> isize {
        (self.data.len() * 8) as isize - self.pos.min(isize::MAX as usize) as isize
    }

    pub(crate) fn overrun(&self) -> bool {
        self.pos > self.data.len() * 8
    }

    pub(crate) fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_msb_first_and_pads() {
        let mut b = Bits::new(&[0b1010_0000, 0xff]);
        assert!(b.bit());
        assert_eq!(b.read(3), 0b010);
        assert_eq!(b.peek(8), 0b0000_1111);
        assert_eq!(b.read(12), 0x0ff);
        assert!(!b.overrun());
        assert_eq!(b.read(32), 0);
        assert!(b.overrun());
        assert!(b.left() < 0);
    }
}
