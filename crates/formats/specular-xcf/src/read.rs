//! Bounds-checked big-endian reader over the XCF bytes.

use crate::{Error, Result};

#[derive(Clone)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pub(crate) pos: usize,
    /// 64-bit pointers (XCF 11 and later).
    wide: bool,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], wide: bool) -> Reader<'a> {
        Reader { data, pos: 0, wide }
    }

    pub(crate) fn at(data: &'a [u8], pos: u64, wide: bool, what: &str) -> Result<Reader<'a>> {
        if pos >= data.len() as u64 {
            return Err(Error::Truncated(format!("{what} at offset {pos}")));
        }
        Ok(Reader {
            data,
            pos: pos as usize,
            wide,
        })
    }

    pub(crate) fn bytes(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or_else(|| Error::Truncated(what.to_string()))?;
        let b = &self.data[self.pos..end];
        self.pos = end;
        Ok(b)
    }

    pub(crate) fn u32(&mut self, what: &str) -> Result<u32> {
        let b = self.bytes(4, what)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn i32(&mut self, what: &str) -> Result<i32> {
        Ok(self.u32(what)? as i32)
    }

    pub(crate) fn f32(&mut self, what: &str) -> Result<f32> {
        Ok(f32::from_bits(self.u32(what)?))
    }

    /// A file offset: 32 bits up to XCF 10, 64 bits from XCF 11.
    pub(crate) fn ptr(&mut self, what: &str) -> Result<u64> {
        if self.wide {
            let b = self.bytes(8, what)?;
            Ok(u64::from_be_bytes([
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            ]))
        } else {
            Ok(u64::from(self.u32(what)?))
        }
    }

    /// A string: u32 length including the NUL, the bytes, the NUL. Length 0 is the empty
    /// string. Invalid UTF-8 is replaced.
    pub(crate) fn string(&mut self, what: &str) -> Result<String> {
        let n = self.u32(what)? as usize;
        if n == 0 {
            return Ok(String::new());
        }
        let b = self.bytes(n, what)?;
        let b = b.strip_suffix(&[0]).unwrap_or(b);
        let b = b.split(|&c| c == 0).next().unwrap_or(b);
        Ok(String::from_utf8_lossy(b).into_owned())
    }

    pub(crate) fn skip(&mut self, n: usize, what: &str) -> Result<()> {
        self.bytes(n, what).map(|_| ())
    }
}
