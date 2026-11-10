//! ISOBMFF primitives: a bounds-checked big-endian reader and a box iterator.
//!
//! Nothing here panics on malformed input: every read is checked and returns
//! [`Error::Malformed`] (inside a parent box) or [`Error::Truncated`] (a top-level box
//! running past the end of the file).

use crate::Error;

pub(crate) type FourCc = [u8; 4];

/// A cursor over a byte slice; all reads are big-endian and bounds-checked.
#[derive(Clone)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

fn short(what: &str) -> Error {
    Error::Malformed(format!("{what}: box too short"))
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub(crate) fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if n > self.remaining() {
            return Err(short("read"));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub(crate) fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.bytes(n).map(|_| ())
    }

    pub(crate) fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, Error> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub(crate) fn u24(&mut self) -> Result<u32, Error> {
        let b = self.bytes(3)?;
        Ok(u32::from_be_bytes([0, b[0], b[1], b[2]]))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Error> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, Error> {
        let b = self.bytes(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_be_bytes(a))
    }

    /// Unsigned integer of `n` bytes (0, 1, 2, 4 or 8; 0 reads as 0).
    pub(crate) fn uint(&mut self, n: u8) -> Result<u64, Error> {
        match n {
            0 => Ok(0),
            1 => self.u8().map(u64::from),
            2 => self.u16().map(u64::from),
            4 => self.u32().map(u64::from),
            8 => self.u64(),
            _ => Err(Error::Malformed(format!("invalid integer size {n}"))),
        }
    }

    pub(crate) fn fourcc(&mut self) -> Result<FourCc, Error> {
        let b = self.bytes(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    /// FullBox header: (version, flags).
    pub(crate) fn full(&mut self) -> Result<(u8, u32), Error> {
        let v = self.u8()?;
        let f = self.u24()?;
        Ok((v, f))
    }

    /// A NUL-terminated string (the terminator is consumed; a missing one ends at the box end).
    pub(crate) fn cstr(&mut self) -> &'a [u8] {
        let rest = self.rest();
        match rest.iter().position(|&b| b == 0) {
            Some(i) => {
                self.pos += i + 1;
                &rest[..i]
            }
            None => {
                self.pos = self.data.len();
                rest
            }
        }
    }
}

/// One box: its type, payload and the payload's absolute offset in the file.
#[derive(Clone, Copy)]
pub(crate) struct BoxRef<'a> {
    pub typ: FourCc,
    pub payload: &'a [u8],
}

/// Iterates the boxes in `data`. With `top_level`, a box running past the end yields
/// [`Error::Truncated`] (the file was cut short); otherwise [`Error::Malformed`].
pub(crate) struct Boxes<'a> {
    data: &'a [u8],
    pos: usize,
    top_level: bool,
    failed: bool,
}

impl<'a> Boxes<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Boxes {
            data,
            pos: 0,
            top_level: false,
            failed: false,
        }
    }

    pub(crate) fn top_level(data: &'a [u8]) -> Self {
        Boxes {
            data,
            pos: 0,
            top_level: true,
            failed: false,
        }
    }

    fn overrun(&self, what: &str) -> Error {
        if self.top_level {
            Error::Truncated
        } else {
            Error::Malformed(format!("{what} overruns its parent"))
        }
    }
}

impl<'a> Iterator for Boxes<'a> {
    type Item = Result<BoxRef<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.pos >= self.data.len() {
            return None;
        }
        let rest = &self.data[self.pos..];
        let r = (|| {
            if rest.len() < 8 {
                return Err(self.overrun("box header"));
            }
            let mut rd = Reader::new(rest);
            let size32 = rd.u32()?;
            let typ = rd.fourcc()?;
            let (hdr, size) = match size32 {
                0 => (8usize, rest.len() as u64),
                1 => {
                    if rest.len() < 16 {
                        return Err(self.overrun("box header"));
                    }
                    (16usize, rd.u64()?)
                }
                s => (8usize, u64::from(s)),
            };
            let hdr = if &typ == b"uuid" { hdr + 16 } else { hdr };
            if size < hdr as u64 {
                return Err(Error::Malformed(format!(
                    "box '{}' smaller than its header",
                    String::from_utf8_lossy(&typ)
                )));
            }
            if size > rest.len() as u64 {
                return Err(self.overrun("box"));
            }
            let size = size as usize;
            Ok((
                BoxRef {
                    typ,
                    payload: &rest[hdr..size],
                },
                size,
            ))
        })();
        match r {
            Ok((b, size)) => {
                self.pos += size;
                Some(Ok(b))
            }
            Err(e) => {
                self.failed = true;
                Some(Err(e))
            }
        }
    }
}

/// Finds the first child box of type `typ`.
pub(crate) fn find<'a>(data: &'a [u8], typ: &FourCc) -> Result<Option<&'a [u8]>, Error> {
    for b in Boxes::new(data) {
        let b = b?;
        if &b.typ == typ {
            return Ok(Some(b.payload));
        }
    }
    Ok(None)
}

pub(crate) fn fourcc_str(t: &FourCc) -> String {
    t.iter()
        .map(|&c| {
            if c.is_ascii_graphic() || c == b' ' {
                c as char
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_bounds() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert_eq!(r.u16().unwrap(), 0x0102);
        assert!(r.u16().is_err());
        assert_eq!(r.u8().unwrap(), 3);
        assert!(r.u8().is_err());
    }

    #[test]
    fn boxes_iterate_and_fail() {
        let data = [
            0, 0, 0, 9, b'a', b'b', b'c', b'd', 7, 0, 0, 0, 20, b'x', b'y', b'z', b'w',
        ];
        let v: Vec<_> = Boxes::top_level(&data).collect();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].as_ref().unwrap().payload, &[7]);
        assert!(matches!(v[1], Err(Error::Truncated)));
        let v: Vec<_> = Boxes::new(&data).collect();
        assert!(matches!(v[1], Err(Error::Malformed(_))));
    }
}
