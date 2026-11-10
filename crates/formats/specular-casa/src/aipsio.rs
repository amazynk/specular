//! casacore's AipsIO framing, in its canonical (big-endian) form: the header files of a
//! table (`table.dat`, `table.f<n>`) are written with it whatever byte order the data use.
//!
//! An object is `[magic 0xbebebebe] u32 length, string type, u32 version, contents`; only
//! the outermost object of a file has the magic. The length counts from the length field
//! itself to the end of the object. Strings are `u32 length, bytes`; a Bool is one byte, and
//! Bool arrays are packed eight to a byte, least significant bit first.

use crate::{Error, Result};

const MAGIC: u32 = 0xbebe_bebe;
/// The longest string read.
const MAX_STRING: usize = 1 << 20;

/// A cursor over AipsIO bytes.
#[derive(Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// End of the innermost open object.
    end: usize,
}

fn short(what: &str) -> Error {
    Error::Malformed(format!("truncated {what}"))
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader {
            data,
            pos: 0,
            end: data.len(),
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Bytes left in the innermost open object.
    pub fn left(&self) -> usize {
        self.end.saturating_sub(self.pos)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| short("object"))?;
        if end > self.end {
            return Err(short("object"));
        }
        let b = &self.data[self.pos..end];
        self.pos = end;
        Ok(b)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let b = self.bytes(N)?;
        let mut a = [0; N];
        a.copy_from_slice(b);
        Ok(a)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.array()?))
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.array()?))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.array()?))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.array()?))
    }

    /// A count of items each at least `min` bytes long, checked against what is left.
    pub fn count(&mut self, min: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        self.check_count(n, min)
    }

    /// `n` items of at least `min` bytes each must fit in what is left.
    pub fn check_count(&self, n: usize, min: usize) -> Result<usize> {
        if n.saturating_mul(min.max(1)) > self.left() {
            return Err(Error::Malformed(format!("count {n} beyond the object")));
        }
        Ok(n)
    }

    pub fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        if n > MAX_STRING {
            return Err(Error::Malformed("string too long".into()));
        }
        let b = self.bytes(n)?;
        Ok(String::from_utf8_lossy(b).into_owned())
    }

    /// `n` Bools packed eight to a byte.
    pub fn bits(&mut self, n: usize) -> Result<Vec<bool>> {
        let b = self.bytes(n.div_ceil(8))?;
        Ok((0..n).map(|i| b[i / 8] >> (i % 8) & 1 != 0).collect())
    }

    /// Opens an object: checks the magic when `top`, reads the length, type and version, and
    /// limits reading to the object until [`Reader::close`]. Returns the version and the end
    /// of the enclosing object to give back to `close`.
    pub fn open(&mut self, top: bool, types: &[&str]) -> Result<(u32, usize)> {
        if top && self.u32()? != MAGIC {
            return Err(Error::NotCasa);
        }
        let at = self.pos;
        let len = self.u32()? as usize;
        let end = at
            .checked_add(len)
            .filter(|&e| e <= self.end && len >= 8)
            .ok_or_else(|| Error::Malformed("object length beyond its parent".into()))?;
        let outer = self.end;
        self.end = end;
        let ty = self.string()?;
        if !types.is_empty() && !types.iter().any(|t| ty.trim_end() == *t) {
            return Err(Error::Malformed(format!(
                "expected {} object, found {ty:?}",
                types.join(" or ")
            )));
        }
        let version = self.u32()?;
        Ok((version, outer))
    }

    /// Opens an object of any type, returning its type name too.
    pub fn open_any(&mut self) -> Result<(String, u32, usize)> {
        let at = self.pos;
        let len = self.u32()? as usize;
        let end = at
            .checked_add(len)
            .filter(|&e| e <= self.end && len >= 8)
            .ok_or_else(|| Error::Malformed("object length beyond its parent".into()))?;
        let outer = self.end;
        self.end = end;
        let ty = self.string()?;
        let version = self.u32()?;
        Ok((ty, version, outer))
    }

    /// Closes the innermost object, skipping what is left of it.
    pub fn close(&mut self, outer: usize) {
        self.pos = self.end;
        self.end = outer;
    }

    /// An `IPosition` object (a shape or position).
    pub fn iposition(&mut self) -> Result<Vec<i64>> {
        let (v, outer) = self.open(false, &["IPosition"])?;
        let n = self.u32()? as usize;
        let size = if v >= 2 { 8 } else { 4 };
        self.check_count(n, size)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(if v >= 2 {
                self.i64()?
            } else {
                i64::from(self.i32()?)
            });
        }
        self.close(outer);
        Ok(out)
    }

    /// A `Block` object of unsigned integers.
    pub fn block_u32(&mut self) -> Result<Vec<u32>> {
        let (_, outer) = self.open(false, &["Block"])?;
        let n = self.count(4)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.u32()?);
        }
        self.close(outer);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(ty: &str, version: u32, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        let len = 4 + 4 + ty.len() + 4 + body.len();
        v.extend((len as u32).to_be_bytes());
        v.extend((ty.len() as u32).to_be_bytes());
        v.extend(ty.as_bytes());
        v.extend(version.to_be_bytes());
        v.extend(body);
        v
    }

    #[test]
    fn objects_nest_and_bound() {
        let mut body = 3u32.to_be_bytes().to_vec();
        for x in [5i32, 4, 2] {
            body.extend(x.to_be_bytes());
        }
        let ip = obj("IPosition", 1, &body);
        let mut top = MAGIC.to_be_bytes().to_vec();
        top.extend(obj("Thing", 7, &ip));
        let mut r = Reader::new(&top);
        let (v, outer) = r.open(true, &["Thing"]).unwrap();
        assert_eq!(v, 7);
        assert_eq!(r.iposition().unwrap(), [5, 4, 2]);
        r.close(outer);
        assert_eq!(r.pos(), top.len());
        // Truncated: the length claims more than there is.
        let mut r = Reader::new(&top[..top.len() - 1]);
        assert!(r.open(true, &["Thing"]).is_err());
        // Wrong magic.
        let mut r = Reader::new(&top[1..]);
        assert!(r.open(true, &[]).is_err());
    }

    #[test]
    fn huge_counts_refused() {
        let mut body = u32::MAX.to_be_bytes().to_vec();
        body.extend([0; 8]);
        let ip = obj("IPosition", 1, &body);
        assert!(Reader::new(&ip).iposition().is_err());
        assert_eq!(
            Reader::new(&[0b0000_0101]).bits(3).unwrap(),
            [true, false, true]
        );
    }
}
