//! MSB-first bit reader with optional JPEG byte stuffing.

/// MSB-first bit reader. With `stuffed`, `FF 00` reads as `FF` and any other `FF xx` is a marker
/// that ends the data (zeros are fed from there on). Reading past the end feeds zeros and is
/// reported by [`BitReader::overrun`].
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Bits left-aligned: the next bit is bit 63.
    buf: u64,
    nbits: u32,
    zeros: u32,
    stuffed: bool,
    /// MSB-first over little-endian 32-bit words (Phase One, Hasselblad, Samsung).
    le32: bool,
    marker: bool,
    /// An earlier restart interval ran out of data.
    lost: bool,
}

const FF_MASK: u64 = 0x8080_8080_8080_8080;
const ONES: u64 = 0x0101_0101_0101_0101;

#[inline(always)]
fn has_ff(w: u64) -> bool {
    // A byte is 0xFF iff the same byte of !w is zero.
    let x = !w;
    (x.wrapping_sub(ONES) & !x & FF_MASK) != 0
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], stuffed: bool) -> Self {
        let mut r = BitReader {
            data,
            pos: 0,
            buf: 0,
            nbits: 0,
            zeros: 0,
            stuffed,
            le32: false,
            marker: false,
            lost: false,
        };
        r.refill();
        r
    }

    /// MSB-first bits over little-endian 32-bit words, no stuffing.
    pub fn new_le32(data: &'a [u8]) -> Self {
        let mut r = BitReader::new(&[], false);
        r.data = data;
        r.le32 = true;
        r.zeros = 0;
        r.nbits = 0;
        r.refill();
        r
    }

    /// More bits were consumed than the data holds (the stream is truncated).
    pub fn overrun(&self) -> bool {
        self.lost || self.zeros as u64 * 8 > self.nbits as u64
    }

    /// Ensure at least 57 valid bits are buffered.
    #[inline(always)]
    pub fn refill(&mut self) {
        if self.nbits > 56 {
            return;
        }
        if self.le32 {
            while self.nbits <= 32 {
                let w = match self.data.get(self.pos..self.pos + 4) {
                    Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    None => {
                        let mut b = [0u8; 4];
                        let rest = self.data.get(self.pos..).unwrap_or(&[]);
                        b[..rest.len()].copy_from_slice(rest);
                        self.zeros = self.zeros.saturating_add(4 - rest.len() as u32);
                        u32::from_le_bytes(b)
                    }
                };
                self.pos = (self.pos + 4).min(self.data.len());
                self.buf |= (w as u64) << (32 - self.nbits);
                self.nbits += 32;
            }
            return;
        }
        if !self.marker
            && let Some(b) = self.data.get(self.pos..self.pos + 8)
        {
            let w = u64::from_be_bytes(b.try_into().unwrap_or_default());
            if !self.stuffed || !has_ff(w) {
                self.buf |= w >> self.nbits;
                let n = (63 - self.nbits) >> 3;
                self.pos += n as usize;
                self.nbits += n * 8;
                return;
            }
        }
        self.refill_slow();
    }

    #[cold]
    fn refill_slow(&mut self) {
        // Drop speculative bits below `nbits` left by the fast path.
        self.buf &= if self.nbits == 0 {
            0
        } else {
            !(u64::MAX >> self.nbits)
        };
        while self.nbits <= 56 {
            let mut byte = 0u8;
            if self.marker || self.pos >= self.data.len() {
                self.zeros = self.zeros.saturating_add(1);
            } else {
                byte = self.data[self.pos];
                if self.stuffed && byte == 0xff {
                    match self.data.get(self.pos + 1) {
                        Some(0) => self.pos += 2,
                        Some(_) | None => {
                            self.marker = true;
                            self.zeros = self.zeros.saturating_add(1);
                            byte = 0;
                        }
                    }
                } else {
                    self.pos += 1;
                }
            }
            self.buf |= (byte as u64) << (56 - self.nbits);
            self.nbits += 8;
        }
    }

    /// Peek `n` (1..=32) bits; requires a preceding [`refill`](Self::refill) covering them.
    #[inline(always)]
    pub fn peek(&self, n: u32) -> u32 {
        (self.buf >> (64 - n)) as u32
    }

    #[inline(always)]
    pub fn consume(&mut self, n: u32) {
        let n = n.min(self.nbits);
        self.buf = if n >= 64 { 0 } else { self.buf << n };
        self.nbits -= n;
    }

    /// Read `n` (0..=32) bits.
    #[inline(always)]
    pub fn bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        if self.nbits < n {
            self.refill();
        }
        let v = self.peek(n);
        self.consume(n);
        v
    }

    #[inline(always)]
    pub fn nbits(&self) -> u32 {
        self.nbits
    }

    /// Skip to the byte after the next `FF Dx` restart marker (JPEG restart interval).
    /// Returns false if none is found.
    pub fn restart(&mut self) -> bool {
        self.lost = self.overrun();
        self.zeros = 0;
        self.buf = 0;
        self.nbits = 0;
        self.marker = false;
        let mut p = self.pos;
        while p + 1 < self.data.len() {
            if self.data[p] == 0xff && (0xd0..=0xd7).contains(&self.data[p + 1]) {
                self.pos = p + 2;
                self.refill();
                return true;
            }
            p += 1;
        }
        self.pos = self.data.len();
        self.refill();
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuffing_and_markers() {
        let d = [0xff, 0x00, 0x12, 0xff, 0xd9, 0x34];
        let mut r = BitReader::new(&d, true);
        assert_eq!(r.bits(8), 0xff);
        assert_eq!(r.bits(8), 0x12);
        assert!(!r.overrun());
        assert_eq!(r.bits(8), 0);
        assert!(r.overrun());
    }

    #[test]
    fn plain_bits() {
        let d: Vec<u8> = (0..32u8).collect();
        let mut r = BitReader::new(&d, false);
        for i in 0..32u32 {
            assert_eq!(r.bits(4), i >> 4);
            assert_eq!(r.bits(4), i & 15);
        }
        assert!(!r.overrun());
        r.bits(1);
        assert!(r.overrun());
    }

    #[test]
    fn has_ff_detects() {
        assert!(has_ff(0x00ff_0000_0000_0000));
        assert!(has_ff(0x0000_0000_0000_00ff));
        assert!(!has_ff(0xfefe_fefe_fefe_fefe));
        assert!(!has_ff(0x7f80_0001_0203_0405));
    }
}
