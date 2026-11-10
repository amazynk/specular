//! Big-endian bit reading for slice data, and the ProRes variable-length codewords
//! (RDD 36 §7.1.1): Golomb-Rice codes that switch to exponential Golomb codes past a
//! prefix length, described by one codebook byte.

/// A reader over one component's coded bytes. Reading past the end yields zero bits.
pub struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load into the cache.
    next: usize,
    /// Unread bits, most significant first.
    cache: u64,
    /// Valid bits in `cache` (past the end the zeros count as valid).
    avail: u32,
    /// Bits consumed.
    pos: usize,
    end: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Bits<'a> {
        let mut b = Bits {
            data,
            next: 0,
            cache: 0,
            avail: 0,
            pos: 0,
            end: data.len() * 8,
        };
        b.refill();
        b
    }

    /// Tops the cache up to at least 57 bits.
    #[inline(always)]
    fn refill(&mut self) {
        if let Some(w) = self.data.get(self.next..self.next + 8) {
            let w = u64::from_be_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]);
            self.cache |= w >> self.avail;
            self.next += ((63 - self.avail) >> 3) as usize;
            self.avail |= 56;
        } else {
            while self.avail <= 56 {
                let byte = self.data.get(self.next).copied();
                self.next += 1;
                match byte {
                    Some(b) => self.cache |= u64::from(b) << (56 - self.avail),
                    // Past the end: zeros, as many as anyone reads.
                    None => {
                        self.avail = 64;
                        break;
                    }
                }
                self.avail += 8;
            }
        }
    }

    /// The next 32 bits, most significant first (zeros past the end).
    #[inline(always)]
    pub fn peek32(&mut self) -> u32 {
        if self.avail < 32 {
            self.refill();
        }
        (self.cache >> 32) as u32
    }

    /// Drops `n` (at most 32, all peeked) bits.
    #[inline(always)]
    fn consume(&mut self, n: u32) {
        self.cache = self.cache.checked_shl(n).unwrap_or(0);
        self.avail = self.avail.saturating_sub(n);
        self.pos += n as usize;
    }

    /// `n` bits (0..=32) as an unsigned number.
    #[inline(always)]
    pub fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek32() >> (32 - n);
        self.consume(n);
        v
    }

    #[inline(always)]
    pub fn bit(&mut self) -> bool {
        self.read(1) == 1
    }

    /// Bits left before the end (zero once past it).
    #[inline(always)]
    pub fn left(&self) -> usize {
        self.end.saturating_sub(self.pos)
    }

    /// Whether everything left is zero (the padding after the last coefficient).
    #[inline(always)]
    pub fn only_padding(&mut self) -> bool {
        let left = self.left();
        left == 0 || left < 32 && self.peek32() >> (32 - left) == 0
    }

    /// One codeword of `codebook`: bits 7..5 the Rice order, bits 4..2 the exponential
    /// Golomb order, bits 1..0 the last Rice prefix length (the switch). `None` when the
    /// codeword runs past 32 bits (damaged data).
    #[inline(always)]
    pub fn codeword(&mut self, codebook: u8) -> Option<u32> {
        let rice = u32::from(codebook >> 5);
        let exp = u32::from((codebook >> 2) & 7);
        let switch = u32::from(codebook & 3);
        let buf = self.peek32();
        let q = buf.leading_zeros();
        if q > switch {
            // Exponential Golomb: `q - switch - 1` more zeros, then the number itself.
            let bits = exp + 2 * q - switch;
            if bits > 32 {
                return None;
            }
            let n = buf >> (32 - bits);
            self.consume(bits);
            Some(n.wrapping_sub(1 << exp).wrapping_add((switch + 1) << rice))
        } else if rice > 0 {
            let v = (buf << (q + 1)) >> (32 - rice);
            self.consume(q + 1 + rice);
            Some((q << rice) | v)
        } else {
            self.consume(q + 1);
            Some(q)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rice_and_exp_golomb() {
        // Codebook 0x04: Rice order 0, exp order 1, switch 0. "1" = 0; "01x" exp-Golomb
        // order 1 after one escape zero.
        let mut b = Bits::new(&[0b1010_0000, 0]);
        assert_eq!(b.codeword(0x04), Some(0));
        // 0 1 0: q = 1 > 0, bits = 1 + 2 - 0 = 3, n = 0b010 = 2, 2 - 2 + 1 = 1.
        assert_eq!(b.codeword(0x04), Some(1));
        // Rice order 2 (0x40 | switch 0): "1 11" = 3.
        let mut b = Bits::new(&[0b1110_0000]);
        assert_eq!(b.codeword(0x40), Some(3));
        assert!(b.left() == 5);
        // Past 32 bits: none.
        let mut b = Bits::new(&[0; 8]);
        assert_eq!(b.codeword(0x04), None);
    }

    #[test]
    fn reading_past_the_end_is_zeros() {
        let mut b = Bits::new(&[0xff]);
        assert_eq!(b.read(4), 0xf);
        assert_eq!(b.read(8), 0xf0);
        assert_eq!(b.left(), 0);
        assert!(b.only_padding());
        assert_eq!(b.read(32), 0);
    }

    #[test]
    fn long_streams_refill() {
        let data: Vec<u8> = (0..40u8).collect();
        let mut b = Bits::new(&data);
        for (i, &want) in data.iter().enumerate() {
            assert_eq!(b.read(8), u32::from(want), "byte {i}");
        }
        assert_eq!(b.left(), 0);
    }
}
