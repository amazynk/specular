//! MSB-first bit reader; reads past the end give zeros and set `overrun`.

pub(crate) struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Bits<'a> {
        Bits { b, pos: 0 }
    }

    /// `n` bits (at most 32).
    pub(crate) fn u(&mut self, n: u32) -> u32 {
        let mut v = 0u64;
        for _ in 0..n {
            let byte = self.b.get(self.pos >> 3).copied().unwrap_or(0);
            v = (v << 1) | u64::from((byte >> (7 - (self.pos & 7))) & 1);
            self.pos += 1;
        }
        v as u32
    }

    pub(crate) fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    pub(crate) fn overrun(&self) -> bool {
        self.pos > self.b.len() * 8
    }
}
