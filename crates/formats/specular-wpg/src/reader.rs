//! A bounds-checked little-endian reader.

pub(crate) struct R<'a> {
    pub b: &'a [u8],
    pub p: usize,
}

impl<'a> R<'a> {
    pub fn new(b: &'a [u8]) -> R<'a> {
        R { b, p: 0 }
    }
    pub fn left(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }
    pub fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    pub fn u16(&mut self) -> Option<u16> {
        let s = self.b.get(self.p..self.p + 2)?;
        self.p += 2;
        Some(u16::from_le_bytes([s[0], s[1]]))
    }
    pub fn i16(&mut self) -> Option<i16> {
        self.u16().map(|v| v as i16)
    }
    pub fn u32(&mut self) -> Option<u32> {
        let s = self.b.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }
    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let s = self.b.get(self.p..).unwrap_or(&[]);
        self.p = self.b.len();
        s
    }
    /// The WordPerfect variable-length integer: one byte below 0xFF; else a 16-bit value,
    /// which with its top bit set is the high half of a 31-bit value whose low half follows.
    pub fn varint(&mut self) -> Option<u32> {
        let v = self.u8()?;
        if v != 0xFF {
            return Some(u32::from(v));
        }
        let hi = self.u16()?;
        if hi & 0x8000 == 0 {
            return Some(u32::from(hi));
        }
        let lo = self.u16()?;
        Some((u32::from(hi & 0x7FFF) << 16) | u32::from(lo))
    }
}
