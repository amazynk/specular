//! MSB-first bit reading over one coded picture (Simple/Main) or one BDU payload with its
//! emulation prevention removed (Advanced). Reading past the end gives zeros and is noted.

#[derive(Clone)]
pub struct Bits<'a> {
    data: &'a [u8],
    /// Bit position of the next unread bit.
    pos: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, pos: 0 }
    }

    /// The next 32 bits without consuming them (zeros past the end).
    #[inline(always)]
    pub fn peek32(&self) -> u32 {
        let byte = self.pos >> 3;
        let shift = (self.pos & 7) as u32;
        let mut w = [0u8; 8];
        let end = (byte + 5).min(self.data.len());
        if byte < end {
            w[..end - byte].copy_from_slice(&self.data[byte..end]);
        }
        let v = u64::from_be_bytes(w);
        ((v << shift) >> 32) as u32
    }

    #[inline(always)]
    pub fn skip(&mut self, n: u32) {
        self.pos += n as usize;
    }

    /// `n` bits (0..=32) as an unsigned number.
    #[inline(always)]
    pub fn u(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek32() >> (32 - n);
        self.pos += n as usize;
        v
    }

    #[inline(always)]
    pub fn bit(&mut self) -> u32 {
        self.u(1)
    }

    #[inline(always)]
    pub fn flag(&mut self) -> bool {
        self.u(1) != 0
    }

    /// Bits consumed.
    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn set_position(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Skips to the next byte boundary.
    pub fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// Whether reads went past the end of the data.
    pub fn overrun(&self) -> bool {
        self.pos > self.data.len() * 8
    }

    /// Reads a unary-coded value: counts `1` bits up to `max`, stopping after a `0`
    /// (the "VLC" fields such as MVRANGE, TRANSACFRM, FCM and PTYPE).
    pub fn unary(&mut self, max: u32) -> u32 {
        let mut n = 0;
        while n < max && self.flag() {
            n += 1;
        }
        n
    }
}
