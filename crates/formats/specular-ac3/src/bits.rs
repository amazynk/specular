//! An MSB-first bit reader over one syncframe. Reading past the end yields zeros and sets
//! `overrun`, which the frame decoder turns into an error once the block is read.

pub(crate) struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    pub(crate) overrun: bool,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Bits<'a> {
        Bits {
            data,
            pos: 0,
            overrun: false,
        }
    }

    /// Reads `n` bits (0..=32).
    #[inline]
    pub(crate) fn u(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let end = self.pos + n as usize;
        if end > self.data.len() * 8 {
            self.overrun = true;
            self.pos = end;
            return 0;
        }
        let byte = self.pos >> 3;
        // Eight bytes from `byte` (zero-padded) cover any 32-bit field at any bit offset.
        let mut w = [0u8; 8];
        let avail = (self.data.len() - byte).min(8);
        w[..avail].copy_from_slice(&self.data[byte..byte + avail]);
        let v = u64::from_be_bytes(w) << (self.pos & 7);
        self.pos = end;
        (v >> (64 - n)) as u32
    }

    #[inline]
    pub(crate) fn bit(&mut self) -> bool {
        self.u(1) != 0
    }

    /// Reads `n` bits as a two's complement value.
    #[inline]
    pub(crate) fn s(&mut self, n: u32) -> i32 {
        let v = self.u(n) as i32;
        if n == 0 || n >= 32 {
            return v;
        }
        (v << (32 - n)) >> (32 - n)
    }

    pub(crate) fn skip(&mut self, n: usize) {
        self.pos += n;
        if self.pos > self.data.len() * 8 {
            self.overrun = true;
        }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }
}

/// An MSB-first bit writer (for test streams made from real frames).
#[derive(Default)]
pub(crate) struct Writer {
    bytes: Vec<u8>,
    bits: usize,
}

impl Writer {
    /// Appends the low `n` bits of `v` (n ≤ 32).
    pub(crate) fn put(&mut self, v: u32, n: u32) {
        for k in (0..n).rev() {
            if self.bits & 7 == 0 {
                self.bytes.push(0);
            }
            if (v >> k) & 1 == 1 {
                let last = self.bytes.len() - 1;
                self.bytes[last] |= 0x80 >> (self.bits & 7);
            }
            self.bits += 1;
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.bits
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// CRC-16 with the polynomial x^16 + x^15 + x^2 + 1, MSB first, from zero (A/52 §7.10.1).
pub(crate) fn crc16(data: &[u8]) -> u16 {
    static TABLE: std::sync::OnceLock<[u16; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u16; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = (i as u16) << 8;
            for _ in 0..8 {
                c = if c & 0x8000 != 0 {
                    (c << 1) ^ 0x8005
                } else {
                    c << 1
                };
            }
            *e = c;
        }
        t
    });
    data.iter().fold(0u16, |crc, &b| {
        (crc << 8) ^ t[usize::from((crc >> 8) as u8 ^ b)]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_across_bytes() {
        let mut b = Bits::new(&[0b1010_1100, 0b0011_1111, 0xFF]);
        assert_eq!(b.u(3), 0b101);
        assert_eq!(b.u(7), 0b011_0000);
        assert_eq!(b.s(4), 0b1111 - 16);
        assert!(!b.overrun);
        assert_eq!(b.u(16), 0);
        assert!(b.overrun);
    }

    #[test]
    fn crc_of_message_with_its_crc_is_zero() {
        let mut m = b"specular ac-3".to_vec();
        let c = crc16(&m);
        m.extend_from_slice(&c.to_be_bytes());
        assert_eq!(crc16(&m), 0);
    }
}
