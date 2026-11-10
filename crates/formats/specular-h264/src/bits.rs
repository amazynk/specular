//! NAL unit payloads (emulation prevention removed) and an MSB-first bit reader with a 64-bit
//! window.

/// Zero bytes kept after the payload so the reader can always load eight bytes at once.
pub const PAD: usize = 16;

/// The RBSP of a NAL unit payload (`nal` without its header byte): `00 00 03` emulation
/// prevention bytes removed, `PAD` zero bytes appended. Returns the buffer and the payload
/// length in bytes.
pub fn unescape(nal: &[u8], out: &mut Vec<u8>) -> usize {
    out.clear();
    out.reserve(nal.len() + PAD);
    let mut start = 0;
    let mut i = 0;
    let n = nal.len();
    while i + 2 < n {
        // Skip quickly to a candidate: the third byte of 00 00 03 is 3, the second 0.
        if nal[i + 2] > 3 {
            i += 3;
            continue;
        }
        if nal[i] == 0 && nal[i + 1] == 0 && nal[i + 2] == 3 {
            out.extend_from_slice(&nal[start..i + 2]);
            start = i + 3;
            i += 3;
        } else {
            i += 1;
        }
    }
    out.extend_from_slice(&nal[start..]);
    let len = out.len();
    out.resize(len + PAD, 0);
    len
}

/// Bit position of the `rbsp_stop_one_bit` (the last set bit of the payload), or 0 if none.
pub fn rbsp_end(data: &[u8], len: usize) -> usize {
    let mut i = len;
    while i > 0 {
        let b = data[i - 1];
        if b != 0 {
            return (i - 1) * 8 + 7 - b.trailing_zeros() as usize;
        }
        i -= 1;
    }
    0
}

#[derive(Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Current position in bits.
    pos: usize,
    /// Payload length in bits (the data slice carries padding beyond it).
    end: usize,
}

impl<'a> BitReader<'a> {
    /// `data` must carry at least [`PAD`] bytes after `len` payload bytes (see [`unescape`]);
    /// a shorter slice is handled, just slower.
    pub fn new(data: &'a [u8], len: usize) -> BitReader<'a> {
        BitReader {
            data,
            pos: 0,
            end: len.min(data.len()) * 8,
        }
    }

    #[inline(always)]
    fn window(&self) -> u64 {
        let i = self.pos >> 3;
        let w = match self.data.get(i..i + 8) {
            Some(b) => u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
            None => {
                let mut b = [0u8; 8];
                if i < self.data.len() {
                    let r = &self.data[i..];
                    b[..r.len()].copy_from_slice(r);
                }
                u64::from_be_bytes(b)
            }
        };
        w << (self.pos & 7)
    }

    /// The next `n` (1..=32) bits without consuming them.
    #[inline(always)]
    pub fn peek(&self, n: u32) -> u32 {
        debug_assert!((1..=32).contains(&n));
        (self.window() >> (64 - n)) as u32
    }

    /// The next 32 bits.
    #[inline(always)]
    pub fn peek32(&self) -> u32 {
        (self.window() >> 32) as u32
    }

    #[inline(always)]
    pub fn skip(&mut self, n: u32) {
        self.pos += n as usize;
    }

    /// `n` (0..=32) bits.
    #[inline(always)]
    pub fn u(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek(n);
        self.pos += n as usize;
        v
    }

    #[inline(always)]
    pub fn flag(&mut self) -> bool {
        self.u(1) != 0
    }

    /// ue(v); `u32::MAX` for codes longer than 32 bits (callers range-check every value).
    #[inline(always)]
    pub fn ue(&mut self) -> u32 {
        let w = self.peek32();
        let lz = w.leading_zeros();
        if lz < 16 {
            let len = 2 * lz + 1;
            self.pos += len as usize;
            return (w >> (32 - len)) - 1;
        }
        if lz >= 32 {
            self.pos += 32;
            return u32::MAX;
        }
        self.pos += lz as usize;
        let v = self.u(lz + 1) as u64;
        (v - 1).min(u32::MAX as u64 - 1) as u32
    }

    /// se(v).
    #[inline(always)]
    pub fn se(&mut self) -> i32 {
        let k = self.ue();
        if k == u32::MAX {
            return i32::MIN;
        }
        let m = k.div_ceil(2) as i32;
        if k & 1 == 1 { m } else { -m }
    }

    #[inline(always)]
    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn overrun(&self) -> bool {
        self.pos > self.end
    }

    pub fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// more_rbsp_data(): whether there is data before the stop bit at `stop` (from
    /// [`rbsp_end`]).
    pub fn more_rbsp_data(&self, stop: usize) -> bool {
        self.pos < stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_removes_emulation_prevention() {
        let mut v = Vec::new();
        let n = unescape(&[0, 0, 3, 1, 0, 0, 3, 0, 0, 3], &mut v);
        assert_eq!(&v[..n], &[0, 0, 1, 0, 0, 0, 0]);
        let n = unescape(&[5, 0, 0, 3], &mut v);
        assert_eq!(&v[..n], &[5, 0, 0]);
        let n = unescape(&[9, 0, 0, 4, 3], &mut v);
        assert_eq!(&v[..n], &[9, 0, 0, 4, 3]);
    }

    #[test]
    fn exp_golomb() {
        // 1 | 010 | 011 | 00100 | 00101 -> ue 0,1,2,3,4
        let mut v = Vec::new();
        let n = unescape(&[0b1010_0110, 0b0100_0010, 0b1000_0000], &mut v);
        let mut r = BitReader::new(&v, n);
        assert_eq!([r.ue(), r.ue(), r.ue(), r.ue(), r.ue()], [0, 1, 2, 3, 4]);
        let mut r = BitReader::new(&v, n);
        assert_eq!([r.se(), r.se(), r.se()], [0, 1, -1]);
        assert_eq!(rbsp_end(&v, n), 16);
    }
}
