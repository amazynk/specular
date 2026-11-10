//! RBSP bit reader (`u(n)`, `ue(v)`, `se(v)`, alignment) and emulation
//! prevention removal (7.4.2.1).

use crate::error::{Error, Result};

/// Removes the emulation prevention bytes (`00 00 03` → `00 00`) of a NAL unit payload.
#[cfg(test)]
pub fn unescape(nal: &[u8]) -> Vec<u8> {
    unescape_with_positions(nal).0
}

/// [`unescape`], also returning the positions (in `nal`) of the bytes removed.
pub fn unescape_with_positions(nal: &[u8]) -> (Vec<u8>, Vec<usize>) {
    let mut out = Vec::with_capacity(nal.len());
    let mut removed = Vec::new();
    let mut zeros = 0;
    for (i, &b) in nal.iter().enumerate() {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            removed.push(i);
            continue;
        }
        if b == 0 {
            zeros += 1;
        } else {
            zeros = 0;
        }
        out.push(b);
    }
    (out, removed)
}

/// MSB-first bit reader over an RBSP.
#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            end: data.len() * 8,
        }
    }

    #[inline]
    pub fn flag(&mut self) -> Result<bool> {
        Ok(self.u(1)? != 0)
    }

    /// `u(n)`, `n <= 32`.
    pub fn u(&mut self, n: u32) -> Result<u32> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err(Error::InvalidData("field wider than 32 bits"));
        }
        if self.pos + n as usize > self.end {
            return Err(Error::Truncated);
        }
        let mut v: u64 = 0;
        let mut left = n as usize;
        while left > 0 {
            let byte = self.data[self.pos >> 3];
            let off = self.pos & 7;
            let avail = 8 - off;
            let take = left.min(avail);
            let chunk = (u64::from(byte) >> (avail - take)) & ((1u64 << take) - 1);
            v = (v << take) | chunk;
            self.pos += take;
            left -= take;
        }
        Ok(v as u32)
    }

    /// `ue(v)`, up to 32 leading zeros.
    pub fn ue(&mut self) -> Result<u32> {
        let mut zeros = 0u32;
        while self.u(1)? == 0 {
            zeros += 1;
            if zeros > 32 {
                return Err(Error::InvalidData("Exp-Golomb code too long"));
            }
        }
        if zeros == 0 {
            return Ok(0);
        }
        let rest = u64::from(self.u(zeros)?);
        let v = (1u64 << zeros) - 1 + rest;
        u32::try_from(v).map_err(|_| Error::InvalidData("Exp-Golomb value too large"))
    }

    /// `ue(v)` that must not exceed `max`.
    pub fn ue_max(&mut self, max: u32, what: &'static str) -> Result<u32> {
        let v = self.ue()?;
        if v > max {
            return Err(Error::InvalidData(what));
        }
        Ok(v)
    }

    /// `se(v)`.
    pub fn se(&mut self) -> Result<i32> {
        let k = i64::from(self.ue()?);
        let v = if k & 1 == 1 { (k + 1) / 2 } else { -(k / 2) };
        i32::try_from(v).map_err(|_| Error::InvalidData("signed Exp-Golomb value too large"))
    }

    /// `se(v)` in `lo..=hi`.
    pub fn se_range(&mut self, lo: i32, hi: i32, what: &'static str) -> Result<i32> {
        let v = self.se()?;
        if v < lo || v > hi {
            return Err(Error::InvalidData(what));
        }
        Ok(v)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        if self.pos + n > self.end {
            return Err(Error::Truncated);
        }
        self.pos += n;
        Ok(())
    }

    pub fn byte_aligned(&self) -> bool {
        self.pos & 7 == 0
    }

    pub fn align_zero(&mut self) -> Result<()> {
        while !self.byte_aligned() {
            self.u(1)?;
        }
        Ok(())
    }

    /// `byte_alignment()`: a one bit then zero bits.
    pub fn byte_alignment(&mut self) -> Result<()> {
        if self.u(1)? != 1 {
            return Err(Error::InvalidData("byte_alignment() without its one bit"));
        }
        self.align_zero()
    }

    /// Bit position from the start.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Byte offset of the next byte (the reader must be byte aligned).
    pub fn byte_pos(&self) -> usize {
        self.pos.div_ceil(8)
    }
}

/// `Ceil( Log2( v ) )` for `v >= 1`.
pub fn ceil_log2(v: u32) -> u32 {
    if v <= 1 {
        0
    } else {
        32 - (v - 1).leading_zeros()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_golomb() {
        // 1 | 010 | 011 | 00100 | 1 (stop)
        let d = [0b1010_0110, 0b0100_1000];
        let mut r = BitReader::new(&d);
        assert_eq!(r.ue().unwrap(), 0);
        assert_eq!(r.ue().unwrap(), 1);
        assert_eq!(r.se().unwrap(), -1);
        assert_eq!(r.ue().unwrap(), 3);
    }

    #[test]
    fn unescape_drops_emulation_bytes() {
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3, 0]), vec![0, 0, 1, 0, 0, 0]);
        assert_eq!(ceil_log2(1), 0);
        assert_eq!(ceil_log2(5), 3);
        assert_eq!(ceil_log2(8), 3);
    }
}
