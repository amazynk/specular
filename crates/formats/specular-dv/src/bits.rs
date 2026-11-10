//! A small MSB-first bit buffer: the compressed-data areas of a video segment and the overflow
//! pools the three distribution passes fill (SMPTE 314M 5.6).

/// Bits, MSB first, `len` of them.
#[derive(Default, Clone)]
pub(crate) struct Bits {
    bytes: Vec<u8>,
    len: usize,
}

impl Bits {
    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
        self.len = 0;
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Appends `n` (at most 32) bits, the low bits of `v`.
    pub(crate) fn push(&mut self, v: u32, n: usize) {
        for i in (0..n).rev() {
            let bit = (v >> i) & 1;
            if self.len.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if bit != 0 {
                let last = self.bytes.len() - 1;
                self.bytes[last] |= 0x80 >> (self.len % 8);
            }
            self.len += 1;
        }
    }

    /// Appends bits `from..to` of `src` (MSB first).
    pub(crate) fn push_slice(&mut self, src: &[u8], from: usize, to: usize) {
        let mut p = from;
        // Bit by bit to a byte boundary of the destination, then whole bytes.
        while p < to && !self.len.is_multiple_of(8) {
            self.push(u32::from(get_bit(src, p)), 1);
            p += 1;
        }
        while p + 8 <= to {
            let b = read_bits(src, p, 8) as u8;
            self.bytes.push(b);
            self.len += 8;
            p += 8;
        }
        while p < to {
            self.push(u32::from(get_bit(src, p)), 1);
            p += 1;
        }
    }

    /// Appends bits `from..` of another buffer.
    pub(crate) fn push_bits(&mut self, other: &Bits, from: usize) {
        if from < other.len {
            self.push_slice(&other.bytes, from, other.len);
        }
    }

    /// The 16 bits starting at `pos`, zero past the end.
    #[inline]
    pub(crate) fn peek16(&self, pos: usize) -> u16 {
        let i = pos / 8;
        let mut w = 0u32;
        for k in 0..3 {
            w = w << 8 | u32::from(self.bytes.get(i + k).copied().unwrap_or(0));
        }
        let w = (w << (pos % 8)) >> 8;
        let w = w & 0xFFFF;
        // Bits past `len` read as zero.
        let valid = self.len.saturating_sub(pos).min(16);
        if valid < 16 {
            (w & !((1u32 << (16 - valid)) - 1)) as u16
        } else {
            w as u16
        }
    }

    /// `n` (at most 16) bits at `pos`.
    pub(crate) fn read(&self, pos: usize, n: usize) -> u32 {
        u32::from(self.peek16(pos)) >> (16 - n)
    }
}

#[inline]
fn get_bit(src: &[u8], p: usize) -> u8 {
    (src[p / 8] >> (7 - p % 8)) & 1
}

/// `n` (at most 24) bits of `src` at bit `p`; the bits must exist.
fn read_bits(src: &[u8], p: usize, n: usize) -> u32 {
    let i = p / 8;
    let mut w = 0u32;
    for k in 0..4 {
        w = w << 8 | u32::from(src.get(i + k).copied().unwrap_or(0));
    }
    (w << (p % 8)) >> (32 - n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_and_peeks_agree() {
        let src = [0b1010_1100u8, 0b0101_0011, 0xF0, 0x0F];
        let mut b = Bits::default();
        b.push(0b101, 3);
        b.push_slice(&src, 5, 29);
        assert_eq!(b.len(), 27);
        // 101 then src bits 5..29: 100 01010011 11110000 00001
        assert_eq!(b.read(0, 3), 0b101);
        assert_eq!(b.read(3, 11), 0b100_0101_0011);
        assert_eq!(b.peek16(14), 0b1111_0000_0000_1000);
        assert_eq!(b.peek16(26), 0b1000_0000_0000_0000);
        assert_eq!(b.peek16(27), 0);
    }
}
