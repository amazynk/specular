//! GDSII records: a 2-byte big-endian length (header included), a record type and a data
//! type, then the data. Lengths are bounded by the format (at most 65535 bytes) and checked
//! against the file; reals are the 8-byte excess-64 base-16 kind.

use crate::GdsError;

pub const HEADER: u8 = 0x00;
pub const BGNLIB: u8 = 0x01;
pub const LIBNAME: u8 = 0x02;
pub const UNITS: u8 = 0x03;
pub const ENDLIB: u8 = 0x04;
pub const BGNSTR: u8 = 0x05;
pub const STRNAME: u8 = 0x06;
pub const ENDSTR: u8 = 0x07;
pub const BOUNDARY: u8 = 0x08;
pub const PATH: u8 = 0x09;
pub const SREF: u8 = 0x0A;
pub const AREF: u8 = 0x0B;
pub const TEXT: u8 = 0x0C;
pub const LAYER: u8 = 0x0D;
pub const DATATYPE: u8 = 0x0E;
pub const WIDTH: u8 = 0x0F;
pub const XY: u8 = 0x10;
pub const ENDEL: u8 = 0x11;
pub const SNAME: u8 = 0x12;
pub const COLROW: u8 = 0x13;
pub const NODE: u8 = 0x15;
pub const TEXTTYPE: u8 = 0x16;
pub const PRESENTATION: u8 = 0x17;
pub const STRING: u8 = 0x19;
pub const STRANS: u8 = 0x1A;
pub const MAG: u8 = 0x1B;
pub const ANGLE: u8 = 0x1C;
pub const PATHTYPE: u8 = 0x21;
pub const NODETYPE: u8 = 0x2A;
pub const PROPATTR: u8 = 0x2B;
pub const PROPVALUE: u8 = 0x2C;
pub const BOX: u8 = 0x2D;
pub const BOXTYPE: u8 = 0x2E;
pub const BGNEXTN: u8 = 0x30;
pub const ENDEXTN: u8 = 0x31;

/// Data types.
pub const DT_NONE: u8 = 0;
pub const DT_BITS: u8 = 1;
pub const DT_I16: u8 = 2;
pub const DT_I32: u8 = 3;
pub const DT_REAL4: u8 = 4;
pub const DT_REAL8: u8 = 5;
pub const DT_ASCII: u8 = 6;

/// One record, borrowing its data.
#[derive(Clone, Copy, Debug)]
pub struct Record<'a> {
    pub rtype: u8,
    pub dtype: u8,
    /// Byte offset of the record in the file.
    pub offset: usize,
    pub data: &'a [u8],
}

/// The name of a record type, for messages.
pub fn name(rtype: u8) -> &'static str {
    const NAMES: [&str; 0x3C] = [
        "HEADER",
        "BGNLIB",
        "LIBNAME",
        "UNITS",
        "ENDLIB",
        "BGNSTR",
        "STRNAME",
        "ENDSTR",
        "BOUNDARY",
        "PATH",
        "SREF",
        "AREF",
        "TEXT",
        "LAYER",
        "DATATYPE",
        "WIDTH",
        "XY",
        "ENDEL",
        "SNAME",
        "COLROW",
        "TEXTNODE",
        "NODE",
        "TEXTTYPE",
        "PRESENTATION",
        "SPACING",
        "STRING",
        "STRANS",
        "MAG",
        "ANGLE",
        "UINTEGER",
        "USTRING",
        "REFLIBS",
        "FONTS",
        "PATHTYPE",
        "GENERATIONS",
        "ATTRTABLE",
        "STYPTABLE",
        "STRTYPE",
        "ELFLAGS",
        "ELKEY",
        "LINKTYPE",
        "LINKKEYS",
        "NODETYPE",
        "PROPATTR",
        "PROPVALUE",
        "BOX",
        "BOXTYPE",
        "PLEX",
        "BGNEXTN",
        "ENDEXTN",
        "TAPENUM",
        "TAPECODE",
        "STRCLASS",
        "RESERVED",
        "FORMAT",
        "MASK",
        "ENDMASKS",
        "LIBDIRSIZE",
        "SRFNAME",
        "LIBSECUR",
    ];
    NAMES.get(rtype as usize).copied().unwrap_or("unknown")
}

/// Walks the records of a file.
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, pos: 0 }
    }

    /// The next record; `Ok(None)` at the end of the bytes.
    pub fn next_record(&mut self) -> Result<Option<Record<'a>>, GdsError> {
        let rest = &self.bytes[self.pos..];
        if rest.is_empty() {
            return Ok(None);
        }
        if rest.len() < 4 {
            return Err(GdsError::Truncated(self.pos));
        }
        let len = usize::from(u16::from_be_bytes([rest[0], rest[1]]));
        if len < 4 || !len.is_multiple_of(2) {
            return Err(GdsError::Malformed(format!(
                "record of {len} bytes at offset {}",
                self.pos
            )));
        }
        if len > rest.len() {
            return Err(GdsError::Truncated(self.pos));
        }
        let r = Record {
            rtype: rest[2],
            dtype: rest[3],
            offset: self.pos,
            data: &rest[4..len],
        };
        self.pos += len;
        Ok(Some(r))
    }

    /// Offset of the next record.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// The bytes after the current position.
    pub fn rest(&self) -> &'a [u8] {
        &self.bytes[self.pos..]
    }
}

/// An 8-byte GDSII real: sign, 7-bit exponent (excess 64, base 16), 56-bit mantissa.
pub fn real8(b: [u8; 8]) -> f64 {
    let sign = b[0] & 0x80 != 0;
    let exp = i32::from(b[0] & 0x7F) - 64;
    let mut m: u64 = 0;
    for &x in &b[1..] {
        m = (m << 8) | u64::from(x);
    }
    // m / 2^56 * 16^exp = m * 2^(4 exp - 56)
    let v = m as f64 * 2f64.powi(4 * exp - 56);
    if sign { -v } else { v }
}

/// A 4-byte GDSII real (rarely written): sign, 7-bit exponent, 24-bit mantissa.
pub fn real4(b: [u8; 4]) -> f64 {
    let sign = b[0] & 0x80 != 0;
    let exp = i32::from(b[0] & 0x7F) - 64;
    let m = (u32::from(b[1]) << 16) | (u32::from(b[2]) << 8) | u32::from(b[3]);
    let v = f64::from(m) * 2f64.powi(4 * exp - 24);
    if sign { -v } else { v }
}

impl<'a> Record<'a> {
    fn bad(&self, what: &str) -> GdsError {
        GdsError::Malformed(format!(
            "{} record at offset {}: {what}",
            name(self.rtype),
            self.offset
        ))
    }

    /// 16-bit integers (also bit arrays).
    pub fn i16s(&self) -> impl Iterator<Item = i16> + 'a {
        self.data
            .chunks_exact(2)
            .map(|c| i16::from_be_bytes([c[0], c[1]]))
    }

    pub fn i16(&self) -> Result<i16, GdsError> {
        if !matches!(self.dtype, DT_I16 | DT_BITS) {
            return Err(self.bad("expected a 16-bit integer"));
        }
        self.i16s().next().ok_or_else(|| self.bad("no data"))
    }

    pub fn i32s(&self) -> impl Iterator<Item = i32> + 'a {
        self.data
            .chunks_exact(4)
            .map(|c| i32::from_be_bytes([c[0], c[1], c[2], c[3]]))
    }

    /// A 32-bit integer; a 16-bit one is widened (some writers use it for WIDTH).
    pub fn i32(&self) -> Result<i32, GdsError> {
        match self.dtype {
            DT_I32 => self.i32s().next().ok_or_else(|| self.bad("no data")),
            DT_I16 => self.i16().map(i32::from),
            _ => Err(self.bad("expected a 32-bit integer")),
        }
    }

    /// Reals of either size.
    pub fn reals(&self) -> Result<Vec<f64>, GdsError> {
        match self.dtype {
            DT_REAL8 => Ok(self
                .data
                .chunks_exact(8)
                .map(|c| real8([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
                .collect()),
            DT_REAL4 => Ok(self
                .data
                .chunks_exact(4)
                .map(|c| real4([c[0], c[1], c[2], c[3]]))
                .collect()),
            _ => Err(self.bad("expected a real")),
        }
    }

    pub fn real(&self) -> Result<f64, GdsError> {
        self.reals()?
            .first()
            .copied()
            .ok_or_else(|| self.bad("no data"))
    }

    /// An ASCII string without its NUL padding (other bytes as Latin-1).
    pub fn string(&self) -> String {
        let d = self.data;
        let end = d.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
        let d = &d[..end];
        // A NUL inside ends the string (some writers pad oddly).
        let d = d.split(|&c| c == 0).next().unwrap_or(&[]);
        d.iter().map(|&c| char::from(c)).collect()
    }

    /// Coordinate pairs.
    pub fn points(&self) -> Result<impl Iterator<Item = [i32; 2]> + 'a, GdsError> {
        if self.dtype != DT_I32 || !self.data.len().is_multiple_of(8) {
            return Err(self.bad("coordinates are not pairs of 32-bit integers"));
        }
        Ok(self.data.chunks_exact(8).map(|c| {
            [
                i32::from_be_bytes([c[0], c[1], c[2], c[3]]),
                i32::from_be_bytes([c[4], c[5], c[6], c[7]]),
            ]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reals() {
        // 1e-3 and 1e-9 as written by every tool.
        let um = real8([0x3E, 0x41, 0x89, 0x37, 0x4B, 0xC6, 0xA7, 0xF0]);
        assert!((um - 1e-3).abs() < 1e-18, "{um}");
        let nm = real8([0x39, 0x44, 0xB8, 0x2F, 0xA0, 0x9B, 0x5A, 0x54]);
        assert!((nm - 1e-9).abs() < 1e-24, "{nm}");
        assert_eq!(real8([0x41, 0x10, 0, 0, 0, 0, 0, 0]), 1.0);
        assert_eq!(real8([0xC1, 0x10, 0, 0, 0, 0, 0, 0]), -1.0);
        assert_eq!(real8([0x42, 0x5A, 0, 0, 0, 0, 0, 0]), 90.0);
        assert_eq!(real8([0; 8]), 0.0);
        assert_eq!(real4([0x41, 0x10, 0, 0]), 1.0);
    }

    #[test]
    fn records_are_bounded() {
        // A record claiming more bytes than there are.
        let mut r = Reader::new(&[0x00, 0x10, 0x00, 0x02, 0x02, 0x58]);
        assert!(matches!(r.next_record(), Err(GdsError::Truncated(0))));
        // Odd and too-short lengths.
        let mut r = Reader::new(&[0x00, 0x05, 0x00, 0x02, 0x02]);
        assert!(matches!(r.next_record(), Err(GdsError::Malformed(_))));
        let mut r = Reader::new(&[0x00, 0x02, 0x00, 0x02]);
        assert!(matches!(r.next_record(), Err(GdsError::Malformed(_))));
        let mut r = Reader::new(&[0x00, 0x06, 0x00, 0x02, 0x02, 0x58]);
        let rec = r.next_record().unwrap().unwrap();
        assert_eq!(rec.i16().unwrap(), 600);
        assert!(r.next_record().unwrap().is_none());
    }
}
