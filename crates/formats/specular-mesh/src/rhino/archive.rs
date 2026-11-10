//! The 3DM archive: a 32-byte header ("3D Geometry File Format " and a right-aligned
//! version number), then nested chunks. A chunk is a 32-bit type code and a value: for a
//! short chunk (the type's high bit set) the value is the data; otherwise it is the length of
//! the data that follows (4 bytes before version 5 archives, 8 bytes after). Long chunks
//! whose type has the CRC bit end in a CRC-32 inside that length, which readers here skip
//! (they read fields from the start and jump to the chunk's end).
//!
//! [`Cur`] reads little-endian fields inside one chunk's bounds; nothing reads past them.

use crate::{Error, Result};

pub(crate) const TCODE_SHORT: u32 = 0x8000_0000;
pub(crate) const TCODE_ENDOFFILE: u32 = 0x0000_7FFF;
pub(crate) const TCODE_ANONYMOUS_CHUNK: u32 = 0x4000_8000;
pub(crate) const TCODE_MODEL_ATTRIBUTES_CHUNK: u32 = 0x4000_8002;

pub(crate) const TCODE_MATERIAL_TABLE: u32 = 0x1000_0010;
pub(crate) const TCODE_LAYER_TABLE: u32 = 0x1000_0011;
pub(crate) const TCODE_OBJECT_TABLE: u32 = 0x1000_0013;
pub(crate) const TCODE_PROPERTIES_TABLE: u32 = 0x1000_0014;
pub(crate) const TCODE_SETTINGS_TABLE: u32 = 0x1000_0015;
pub(crate) const TCODE_INSTANCE_DEFINITION_TABLE: u32 = 0x1000_0021;
pub(crate) const TCODE_ENDOFTABLE: u32 = 0xFFFF_FFFF;

pub(crate) const TCODE_PROPERTIES_REVISIONHISTORY: u32 = 0x2000_8021;
pub(crate) const TCODE_PROPERTIES_NOTES: u32 = 0x2000_8022;
pub(crate) const TCODE_PROPERTIES_PREVIEWIMAGE: u32 = 0x2000_8023;
pub(crate) const TCODE_PROPERTIES_APPLICATION: u32 = 0x2000_8024;
pub(crate) const TCODE_PROPERTIES_COMPRESSED_PREVIEWIMAGE: u32 = 0x2000_8025;
pub(crate) const TCODE_PROPERTIES_OPENNURBS_VERSION: u32 = 0xA000_0026;
pub(crate) const TCODE_SETTINGS_UNITSANDTOLS: u32 = 0x2000_8031;

pub(crate) const TCODE_MATERIAL_RECORD: u32 = 0x2000_8040;
pub(crate) const TCODE_LAYER_RECORD: u32 = 0x2000_8050;
pub(crate) const TCODE_OBJECT_RECORD: u32 = 0x2000_8070;
pub(crate) const TCODE_INSTANCE_DEFINITION_RECORD: u32 = 0x2000_8076;
pub(crate) const TCODE_OBJECT_RECORD_TYPE: u32 = 0x8200_0071;
pub(crate) const TCODE_OBJECT_RECORD_ATTRIBUTES: u32 = 0x0200_8072;

pub(crate) const TCODE_OPENNURBS_CLASS: u32 = 0x0002_7FFA;
pub(crate) const TCODE_OPENNURBS_CLASS_UUID: u32 = 0x0002_FFFB;
pub(crate) const TCODE_OPENNURBS_CLASS_DATA: u32 = 0x0002_FFFC;

/// The file's archive version and where its chunks start.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Header {
    /// As written: 1, 2, 3, 4, 5, 50, 60, 70, 80…
    pub raw: u32,
    /// Offset of the first chunk (32 unless the file starts with other bytes).
    pub start: usize,
}

impl Header {
    /// The Rhino version the archive is for (5 for both "5" and "50").
    pub fn rhino(&self) -> u32 {
        if self.raw >= 50 {
            self.raw / 10
        } else {
            self.raw
        }
    }

    /// Chunk lengths are 8 bytes from version 50 on.
    pub fn big(&self) -> bool {
        self.raw >= 50
    }
}

const MAGIC: &[u8; 24] = b"3D Geometry File Format ";

/// The header, also when the file starts with up to 64 KiB of other bytes (OLE wrappers).
pub(crate) fn header(d: &[u8]) -> Option<Header> {
    let at = if d.starts_with(MAGIC) {
        0
    } else {
        let scan = &d[..d.len().min(65_536)];
        scan.windows(MAGIC.len()).position(|w| w == MAGIC)?
    };
    let v = d.get(at + 24..at + 32)?;
    let digits = v.trim_ascii_start();
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let raw: u32 = std::str::from_utf8(digits).ok()?.parse().ok()?;
    (raw > 0).then_some(Header {
        raw,
        start: at + 32,
    })
}

/// One chunk: its type, its value (a short chunk's data, a long chunk's length) and, for a
/// long chunk, a cursor over its data.
pub(crate) struct Chunk<'a> {
    pub tcode: u32,
    pub value: i64,
    pub body: Cur<'a>,
}

/// A cursor inside `d[p..end]`.
#[derive(Clone, Copy)]
pub(crate) struct Cur<'a> {
    d: &'a [u8],
    p: usize,
    end: usize,
    big: bool,
}

fn short() -> Error {
    Error::Malformed("truncated 3DM data".into())
}

impl<'a> Cur<'a> {
    pub fn new(d: &'a [u8], start: usize, big: bool) -> Cur<'a> {
        Cur {
            d,
            p: start.min(d.len()),
            end: d.len(),
            big,
        }
    }

    pub fn remaining(&self) -> usize {
        self.end - self.p
    }

    pub fn at_end(&self) -> bool {
        self.p >= self.end
    }

    pub fn pos(&self) -> usize {
        self.p
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(short());
        }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    fn arr<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.bytes(N)?);
        Ok(a)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.arr::<1>()?[0])
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.arr()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.arr()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.arr()?))
    }

    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.arr()?))
    }

    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.arr()?))
    }

    pub fn f64s<const N: usize>(&mut self) -> Result<[f64; N]> {
        let mut a = [0f64; N];
        for v in &mut a {
            *v = self.f64()?;
        }
        Ok(a)
    }

    pub fn uuid(&mut self) -> Result<Uuid> {
        Ok(Uuid(self.arr()?))
    }

    /// An `ON_Color`: red, green, blue and a transparency byte.
    pub fn color(&mut self) -> Result<[u8; 4]> {
        self.arr()
    }

    /// A count that must fit in what is left (`each` bytes per element).
    pub fn count(&mut self, each: usize) -> Result<usize> {
        let n = self.i32()?;
        let n = usize::try_from(n).map_err(|_| Error::Malformed("negative count".into()))?;
        if n.saturating_mul(each.max(1)) > self.remaining() {
            return Err(short());
        }
        Ok(n)
    }

    /// A UTF-16 string: its element count with the terminator, then the elements.
    pub fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        if n == 0 {
            return Ok(String::new());
        }
        let raw = self.bytes(n.checked_mul(2).ok_or_else(short)?)?;
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }

    /// A chunk version byte: (major, minor).
    pub fn version(&mut self) -> Result<(u8, u8)> {
        let v = self.u8()?;
        Ok((v >> 4, v & 15))
    }

    /// The next chunk; this cursor moves past it.
    pub fn chunk(&mut self) -> Result<Chunk<'a>> {
        let tcode = self.u32()?;
        let value = if self.big {
            self.i64()?
        } else if tcode & TCODE_SHORT == 0
            || tcode == TCODE_OBJECT_RECORD_TYPE
            || tcode == TCODE_PROPERTIES_OPENNURBS_VERSION
        {
            i64::from(self.u32()?)
        } else {
            i64::from(self.i32()?)
        };
        if tcode & TCODE_SHORT != 0 {
            return Ok(Chunk {
                tcode,
                value,
                body: Cur {
                    d: self.d,
                    p: self.p,
                    end: self.p,
                    big: self.big,
                },
            });
        }
        let len = usize::try_from(value).map_err(|_| short())?;
        if len > self.remaining() {
            return Err(short());
        }
        let body = Cur {
            d: self.d,
            p: self.p,
            end: self.p + len,
            big: self.big,
        };
        self.p += len;
        Ok(Chunk { tcode, value, body })
    }

    /// The next 32-bit value (a chunk's type code) without moving.
    pub fn peek_u32(&self) -> Option<u32> {
        let mut c = *self;
        c.u32().ok()
    }

    pub fn peek_tcode(&self) -> Option<u32> {
        self.peek_u32()
    }

    /// An anonymous chunk written with a version (two ints): its body after them, and the
    /// version.
    pub fn versioned_chunk(&mut self) -> Result<(Cur<'a>, i32, i32)> {
        let c = self.chunk()?;
        if c.tcode != TCODE_ANONYMOUS_CHUNK && c.tcode != TCODE_MODEL_ATTRIBUTES_CHUNK {
            return Err(Error::Malformed(format!(
                "expected a versioned chunk, found {:08x}",
                c.tcode
            )));
        }
        let mut b = c.body;
        let major = b.i32()?;
        let minor = b.i32()?;
        Ok((b, major, minor))
    }

    /// Skips one chunk.
    pub fn skip_chunk(&mut self) -> Result<()> {
        self.chunk().map(|_| ())
    }

    /// A compressed buffer of `expected` bytes: its size, CRC and method, then the bytes
    /// as they are (method 0) or zlib-deflated in an anonymous chunk (method 1). `None` when
    /// the size is 0 (the array is empty).
    pub fn compressed(&mut self, expected: usize) -> Result<Option<Vec<u8>>> {
        let size = self.u32()? as usize;
        if size == 0 {
            return Ok(None);
        }
        if size != expected {
            return Err(Error::Malformed(format!(
                "compressed buffer of {size} bytes where {expected} were expected"
            )));
        }
        let _crc = self.u32()?;
        match self.u8()? {
            0 => Ok(Some(self.bytes(size)?.to_vec())),
            1 => {
                let c = self.chunk()?;
                let mut b = c.body;
                let n = b.remaining().saturating_sub(4);
                let z = b.bytes(n)?;
                let out = specular_png::inflate_zlib(z, size)
                    .map_err(|e| Error::Malformed(format!("compressed buffer: {e:?}")))?;
                if out.len() != size {
                    return Err(Error::Malformed("compressed buffer is short".into()));
                }
                Ok(Some(out))
            }
            m => Err(Error::Malformed(format!("compression method {m}"))),
        }
    }

    /// Iterates the chunks that follow until the end of this cursor (or a malformed one).
    pub fn chunks(self) -> Chunks<'a> {
        Chunks {
            c: self,
            done: false,
        }
    }
}

pub(crate) struct Chunks<'a> {
    c: Cur<'a>,
    done: bool,
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Result<Chunk<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.c.remaining() < 8 {
            return None;
        }
        let r = self.c.chunk();
        match &r {
            Err(_) => self.done = true,
            Ok(ch) if ch.tcode == TCODE_ENDOFFILE || ch.tcode == TCODE_ENDOFTABLE => {
                self.done = true
            }
            Ok(_) => {}
        }
        Some(r)
    }
}

/// A class or object id as stored (Data1, Data2, Data3 little-endian, then 8 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) struct Uuid(pub [u8; 16]);

impl Uuid {
    /// From the canonical text form; for constants.
    pub const fn parse(s: &str) -> Uuid {
        let b = s.as_bytes();
        let mut hex = [0u8; 32];
        let (mut i, mut n) = (0, 0);
        while i < b.len() {
            let c = b[i];
            let v = match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            if let Some(v) = v {
                if n < 32 {
                    hex[n] = v;
                }
                n += 1;
            }
            i += 1;
        }
        // Text order is Data1 (4 bytes), Data2 (2), Data3 (2), Data4 (8), the first three
        // big-endian in text and little-endian in the file.
        const ORDER: [usize; 16] = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
        let mut out = [0u8; 16];
        let mut k = 0;
        while k < 16 {
            let j = ORDER[k];
            out[k] = (hex[2 * j] << 4) | hex[2 * j + 1];
            k += 1;
        }
        Uuid(out)
    }

    pub fn is_nil(&self) -> bool {
        self.0 == [0; 16]
    }
}

impl std::fmt::Debug for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-",
            b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6]
        )?;
        for (i, x) in b[8..].iter().enumerate() {
            if i == 2 {
                f.write_str("-")?;
            }
            write!(f, "{x:02x}")?;
        }
        Ok(())
    }
}

/// An object written with its class: a class chunk holding the class id and the data.
pub(crate) struct Object<'a> {
    pub class: Uuid,
    pub data: Cur<'a>,
}

/// Reads an object (`TCODE_OPENNURBS_CLASS`) from the cursor; `None` for a nil class.
pub(crate) fn read_object<'a>(c: &mut Cur<'a>) -> Result<Option<Object<'a>>> {
    let ch = c.chunk()?;
    if ch.tcode != TCODE_OPENNURBS_CLASS {
        return Err(Error::Malformed(format!(
            "expected an object, found chunk {:08x}",
            ch.tcode
        )));
    }
    object_in(ch.body)
}

/// The class id and data of a class chunk's body.
pub(crate) fn object_in(body: Cur<'_>) -> Result<Option<Object<'_>>> {
    let mut class = None;
    let mut data = None;
    for ch in body.chunks() {
        let ch = ch?;
        match ch.tcode {
            TCODE_OPENNURBS_CLASS_UUID => {
                let mut b = ch.body;
                class = Some(b.uuid()?);
            }
            TCODE_OPENNURBS_CLASS_DATA => {
                data = Some(ch.body);
                break;
            }
            _ => {}
        }
    }
    match (class, data) {
        (Some(c), _) if c.is_nil() => Ok(None),
        (Some(class), Some(data)) => Ok(Some(Object { class, data })),
        _ => Err(Error::Malformed("object without class or data".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_text_round_trip() {
        let u = Uuid::parse("4ED7D4E4-E947-11d3-BFE5-0010830122F0");
        assert_eq!(
            u.0,
            [
                0xe4, 0xd4, 0xd7, 0x4e, 0x47, 0xe9, 0xd3, 0x11, 0xbf, 0xe5, 0x00, 0x10, 0x83, 0x01,
                0x22, 0xf0
            ]
        );
        assert_eq!(format!("{u:?}"), "4ed7d4e4-e947-11d3-bfe5-0010830122f0");
    }

    #[test]
    fn headers() {
        let h = header(b"3D Geometry File Format       50xxxx").unwrap();
        assert_eq!((h.raw, h.rhino(), h.big(), h.start), (50, 5, true, 32));
        let h = header(b"3D Geometry File Format        3").unwrap();
        assert_eq!((h.rhino(), h.big()), (3, false));
        assert!(header(b"3D Geometry File Format     abc").is_none());
        let mut ole = vec![0u8; 100];
        ole.extend_from_slice(b"3D Geometry File Format       60");
        assert_eq!(header(&ole).unwrap().start, 132);
    }

    #[test]
    fn chunks_stay_in_bounds() {
        // A long chunk claiming more than is there.
        let mut d = Vec::new();
        d.extend_from_slice(&0x1000_0013u32.to_le_bytes());
        d.extend_from_slice(&1000u64.to_le_bytes());
        d.extend_from_slice(&[0; 8]);
        let mut c = Cur::new(&d, 0, true);
        assert!(c.chunk().is_err());
        // A short chunk carries its value.
        let mut d = Vec::new();
        d.extend_from_slice(&TCODE_OBJECT_RECORD_TYPE.to_le_bytes());
        d.extend_from_slice(&32i64.to_le_bytes());
        let mut c = Cur::new(&d, 0, true);
        let ch = c.chunk().unwrap();
        assert_ne!(ch.tcode & TCODE_SHORT, 0);
        assert_eq!(ch.value, 32);
        assert!(c.at_end());
    }
}
