//! Bounds-checked big-endian reads.

/// A big-endian cursor over a byte slice; every read past the end returns `None`.
#[derive(Clone, Copy)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    pub fn at(data: &'a [u8], pos: usize) -> Reader<'a> {
        Reader { data, pos }
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn skip(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n)?;
        (end <= self.data.len()).then(|| self.pos = end)
    }

    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    pub fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    pub fn u16(&mut self) -> Option<u16> {
        let b = self.bytes(2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn i16(&mut self) -> Option<i16> {
        self.u16().map(|v| v as i16)
    }

    pub fn u32(&mut self) -> Option<u32> {
        let b = self.bytes(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }

    /// A 16.16 fixed-point number.
    pub fn fixed(&mut self) -> Option<f32> {
        self.i32().map(|v| v as f32 / 65536.0)
    }

    pub fn tag(&mut self) -> Option<[u8; 4]> {
        let b = self.bytes(4)?;
        Some([b[0], b[1], b[2], b[3]])
    }
}

pub(crate) fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Reader::at(d, at).u16()
}

pub(crate) fn i16_at(d: &[u8], at: usize) -> Option<i16> {
    Reader::at(d, at).i16()
}

pub(crate) fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Reader::at(d, at).u32()
}

/// `d[at..at + len]` when it is all there.
pub(crate) fn slice(d: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    d.get(at..at.checked_add(len)?)
}
