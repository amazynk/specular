//! FITS tables, read lazily from the file's bytes: binary tables (`BINTABLE`, FITS 4.0 §7.3)
//! and ASCII tables (`TABLE`, §7.2).
//!
//! [`Table::from_hdu`] reads the column descriptions only; values are decoded on request
//! ([`Table::f64`], [`Table::cell`]) straight from the bytes (a memory map), so a table of
//! any size costs its column list. Binary columns of every `TFORMn` type: logical `L`, bits
//! `X`, bytes `B`, integers `I` / `J` / `K`, characters `A`, floats `E` / `D`, complex `C` /
//! `M`, and variable-length arrays of any of them through `P` (32-bit) or `Q` (64-bit)
//! descriptors into the heap (`THEAP`, `PCOUNT`). Integers are scaled by `TSCALn` /
//! `TZEROn` (unsigned 16, 32 and 64-bit columns by their conventional offsets), `TNULLn`
//! marks undefined integers, and IEEE NaN undefined floats. ASCII columns (`Aw`, `Iw`,
//! `Fw.d`, `Ew.d`, `Dw.d` at `TBCOLn`) parse their text, with `TNULLn` as a string.
//! Descriptors and offsets are checked against the HDU: a malformed one reads as undefined,
//! never past the data.

use crate::header::Header;
use crate::{Error, Hdu, Kind, Result};

/// The element type of a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// `L`: `T`, `F`, or 0 for undefined.
    Logical,
    /// `X`: bits, most significant first.
    Bit,
    /// `B`: unsigned bytes.
    U8,
    /// `I`, `J`, `K`: signed big-endian integers.
    I16,
    I32,
    I64,
    /// `A`: characters.
    Char,
    /// `E`, `D`: IEEE floats.
    F32,
    F64,
    /// `C`, `M`: pairs of floats.
    C32,
    C64,
}

impl Type {
    fn from_code(c: u8) -> Option<Type> {
        Some(match c {
            b'L' => Type::Logical,
            b'X' => Type::Bit,
            b'B' => Type::U8,
            b'I' => Type::I16,
            b'J' => Type::I32,
            b'K' => Type::I64,
            b'A' => Type::Char,
            b'E' => Type::F32,
            b'D' => Type::F64,
            b'C' => Type::C32,
            b'M' => Type::C64,
            _ => return None,
        })
    }

    /// Bytes per element (bits: 0, counted per byte separately).
    pub fn size(self) -> usize {
        match self {
            Type::Bit => 0,
            Type::Logical | Type::U8 | Type::Char => 1,
            Type::I16 => 2,
            Type::I32 | Type::F32 => 4,
            Type::I64 | Type::F64 | Type::C32 => 8,
            Type::C64 => 16,
        }
    }

    /// Integer storage (scaled by `TSCAL` / `TZERO`, nulls by `TNULL`).
    pub fn is_int(self) -> bool {
        matches!(self, Type::U8 | Type::I16 | Type::I32 | Type::I64)
    }

    /// Holds numbers (integers, floats; complex numbers give their real part).
    pub fn is_numeric(self) -> bool {
        !matches!(self, Type::Logical | Type::Bit | Type::Char)
    }
}

/// An ASCII table column's format (`TFORMn` of a `TABLE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsciiForm {
    Char,
    Int,
    Float,
}

/// Where the elements of a cell are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// In the row: `repeat` elements.
    Fixed,
    /// In the heap, through a 4-byte (`P`) descriptor pair.
    VarP,
    /// In the heap, through an 8-byte (`Q`) descriptor pair.
    VarQ,
    /// Text of an ASCII table.
    Ascii(AsciiForm),
}

/// One column.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// `TTYPEn` (trimmed), may be empty.
    pub name: String,
    /// `TUNITn` (trimmed), may be empty.
    pub unit: String,
    /// `TFORMn` as written.
    pub tform: String,
    pub ty: Type,
    /// Elements per row (fixed), the maximum length (variable, when given), or the field
    /// width in characters (ASCII).
    pub repeat: usize,
    pub storage: Storage,
    /// Byte offset of the column in a row.
    pub offset: usize,
    /// Bytes the column takes in a row.
    pub width: usize,
    pub scale: f64,
    pub zero: f64,
    /// `TNULLn` of an integer column.
    pub null: Option<i64>,
    /// `TNULLn` of an ASCII column.
    pub null_text: Option<String>,
    /// `TDIMn`, fastest first.
    pub dim: Option<Vec<usize>>,
}

impl Column {
    /// Variable-length elements in the heap.
    pub fn is_var(&self) -> bool {
        matches!(self.storage, Storage::VarP | Storage::VarQ)
    }

    /// Holds numbers.
    pub fn is_numeric(&self) -> bool {
        match self.storage {
            Storage::Ascii(f) => f != AsciiForm::Char,
            _ => self.ty.is_numeric(),
        }
    }

    /// An unsigned 64-bit column (`K` with `TZERO = 2^63`), whose values f64 cannot hold
    /// exactly.
    fn unsigned64(&self) -> bool {
        self.ty == Type::I64 && self.scale == 1.0 && self.zero == 9_223_372_036_854_775_808.0
    }
}

/// One element of a cell, as stored (scaled).
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    /// Undefined (`TNULL`, a logical 0, an ASCII null).
    Null,
    Bool(bool),
    /// An integer with identity scaling (`TZERO` 0 or an unsigned offset folded in).
    Int(i64),
    UInt(u64),
    Float(f64),
    Complex(f64, f64),
    Str(String),
}

impl Datum {
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Datum::Int(i) => Some(i as f64),
            Datum::UInt(u) => Some(u as f64),
            Datum::Float(f) if !f.is_nan() => Some(f),
            Datum::Complex(r, _) if !r.is_nan() => Some(r),
            Datum::Bool(b) => Some(f64::from(u8::from(b))),
            _ => None,
        }
    }

    /// The datum as text for display.
    pub fn text(&self) -> String {
        match self {
            Datum::Null => String::new(),
            Datum::Bool(b) => if *b { "T" } else { "F" }.to_owned(),
            Datum::Int(i) => i.to_string(),
            Datum::UInt(u) => u.to_string(),
            Datum::Float(f) => format_float(*f),
            Datum::Complex(a, b) => format!("({}, {})", format_float(*a), format_float(*b)),
            Datum::Str(s) => s.clone(),
        }
    }
}

/// A float as short as it reads back exactly.
fn format_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    format!("{f}")
}

/// A table HDU's columns and layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub columns: Vec<Column>,
    /// `NAXIS2`.
    pub rows: usize,
    /// `NAXIS1`: bytes per row.
    pub row_len: usize,
    /// Byte offsets of the rows and of the heap in the file, and the heap's length.
    pub data_offset: usize,
    pub heap_offset: usize,
    pub heap_len: usize,
    /// An ASCII table.
    pub ascii: bool,
    /// `EXTNAME`, when the HDU has one.
    pub name: Option<String>,
}

/// Parses a binary table's `TFORMn`: repeat, type, storage.
fn binary_form(s: &str) -> Option<(usize, Type, Storage, Option<usize>)> {
    let s = s.trim();
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let repeat = if digits == 0 {
        1
    } else {
        s[..digits].parse().ok()?
    };
    let rest = &s.as_bytes()[digits..];
    let t = *rest.first()?;
    match t {
        b'P' | b'Q' => {
            let ty = Type::from_code(*rest.get(1)?)?;
            // `1PE(100)`: the maximum length.
            let max = s[digits + 2..]
                .strip_prefix('(')
                .and_then(|m| m.split(')').next())
                .and_then(|m| m.trim().parse().ok());
            let st = if t == b'P' {
                Storage::VarP
            } else {
                Storage::VarQ
            };
            Some((repeat, ty, st, max))
        }
        _ => Some((repeat, Type::from_code(t)?, Storage::Fixed, None)),
    }
}

/// Parses an ASCII table's `TFORMn` (`A12`, `I8`, `F10.3`, `E15.7`, `D25.17`): format and
/// width.
fn ascii_form(s: &str) -> Option<(AsciiForm, usize)> {
    let s = s.trim();
    let f = match s.as_bytes().first()? {
        b'A' => AsciiForm::Char,
        b'I' => AsciiForm::Int,
        b'F' | b'E' | b'D' => AsciiForm::Float,
        _ => return None,
    };
    let w = s[1..].split('.').next()?.trim().parse().ok()?;
    Some((f, w))
}

impl Table {
    /// The table of a `BINTABLE` or `TABLE` HDU (not a tile-compressed image).
    pub fn from_hdu(hdu: &Hdu) -> Result<Table> {
        let ascii = match hdu.kind {
            Kind::BinTable => false,
            Kind::Table => true,
            _ => return Err(Error::Unsupported("HDU is not a table".into())),
        };
        Self::from_header(&hdu.header, hdu.data_offset, hdu.data_len, ascii)
    }

    pub(crate) fn from_header(
        h: &Header,
        data_offset: usize,
        data_len: usize,
        ascii: bool,
    ) -> Result<Table> {
        let get = |k: &str| {
            h.i64(k)
                .filter(|v| *v >= 0)
                .ok_or_else(|| Error::Malformed(format!("no {k}")))
        };
        let row_len = get("NAXIS1")? as usize;
        let rows = get("NAXIS2")? as usize;
        let n = get("TFIELDS")? as usize;
        if n > 999 {
            return Err(Error::Malformed(format!("TFIELDS = {n}")));
        }
        let table_len = row_len
            .checked_mul(rows)
            .ok_or_else(|| Error::Malformed("table too large".into()))?;
        let mut columns = Vec::with_capacity(n);
        let mut offset = 0usize;
        for i in 1..=n {
            let key = |k: &str| format!("{k}{i}");
            let s = |k: &str| h.str(&key(k)).unwrap_or("").trim().to_owned();
            let tform = s("TFORM");
            let scale = h
                .f64(&key("TSCAL"))
                .filter(|v| *v != 0.0 && v.is_finite())
                .unwrap_or(1.0);
            let zero = h
                .f64(&key("TZERO"))
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            let dim = h.str(&key("TDIM")).and_then(|d| {
                d.trim()
                    .strip_prefix('(')?
                    .strip_suffix(')')?
                    .split(',')
                    .map(|v| v.trim().parse().ok())
                    .collect::<Option<Vec<usize>>>()
            });
            let col = if ascii {
                let (f, w) = ascii_form(&tform)
                    .ok_or_else(|| Error::Malformed(format!("TFORM{i} = {tform}")))?;
                let start = h
                    .i64(&key("TBCOL"))
                    .filter(|&b| b >= 1)
                    .ok_or_else(|| Error::Malformed(format!("no TBCOL{i}")))?
                    as usize
                    - 1;
                if start.saturating_add(w) > row_len {
                    return Err(Error::Malformed(format!("column {i} past the row")));
                }
                Column {
                    name: s("TTYPE"),
                    unit: s("TUNIT"),
                    tform: tform.clone(),
                    ty: match f {
                        AsciiForm::Char => Type::Char,
                        AsciiForm::Int => Type::I64,
                        AsciiForm::Float => Type::F64,
                    },
                    repeat: w,
                    storage: Storage::Ascii(f),
                    offset: start,
                    width: w,
                    scale,
                    zero,
                    null: None,
                    null_text: h.str(&key("TNULL")).map(|t| t.trim().to_owned()),
                    dim: None,
                }
            } else {
                let (repeat, ty, storage, max) = binary_form(&tform)
                    .ok_or_else(|| Error::Malformed(format!("TFORM{i} = {tform}")))?;
                let width = match storage {
                    Storage::VarP => 8 * repeat.min(1),
                    Storage::VarQ => 16 * repeat.min(1),
                    _ if ty == Type::Bit => repeat.div_ceil(8),
                    _ => repeat
                        .checked_mul(ty.size())
                        .ok_or_else(|| Error::Malformed(format!("TFORM{i} = {tform}")))?,
                };
                let c = Column {
                    name: s("TTYPE"),
                    unit: s("TUNIT"),
                    tform: tform.clone(),
                    ty,
                    repeat: max.unwrap_or(repeat),
                    storage,
                    offset,
                    width,
                    scale,
                    zero,
                    null: if ty.is_int() {
                        h.i64(&key("TNULL"))
                    } else {
                        None
                    },
                    null_text: None,
                    dim,
                };
                offset = offset
                    .checked_add(width)
                    .ok_or_else(|| Error::Malformed("row too long".into()))?;
                c
            };
            columns.push(col);
        }
        if !ascii && offset > row_len {
            return Err(Error::Malformed(format!(
                "columns take {offset} bytes, NAXIS1 = {row_len}"
            )));
        }
        let pcount = h.i64("PCOUNT").unwrap_or(0).max(0) as usize;
        let theap = h
            .i64("THEAP")
            .filter(|&t| t >= 0)
            .map_or(table_len, |t| t as usize);
        // The heap lies within the data unit (rows, gap, heap = data_len).
        let heap_offset = data_offset.saturating_add(theap);
        let heap_len = (table_len + pcount).saturating_sub(theap).min(data_len);
        Ok(Table {
            columns,
            rows,
            row_len,
            data_offset,
            heap_offset,
            heap_len,
            ascii,
            name: h
                .str("EXTNAME")
                .map(|n| n.trim().to_owned())
                .filter(|n| !n.is_empty()),
        })
    }

    /// The column named `name` (case-insensitive, as FITS readers match `TTYPE`).
    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// The bytes of row `row` (shorter than a row when the file is cut short).
    fn row<'a>(&self, data: &'a [u8], row: usize) -> &'a [u8] {
        if row >= self.rows {
            return &[];
        }
        let start = self.data_offset.saturating_add(row * self.row_len);
        let end = start.saturating_add(self.row_len).min(data.len());
        data.get(start..end).unwrap_or(&[])
    }

    /// The bytes of column `col` in row `row`.
    fn field<'a>(&self, data: &'a [u8], row: usize, col: usize) -> Option<&'a [u8]> {
        let c = self.columns.get(col)?;
        self.row(data, row).get(c.offset..c.offset + c.width)
    }

    /// Where the elements of a cell are: their bytes (heap or row) and count.
    fn elements<'a>(&self, data: &'a [u8], row: usize, col: usize) -> Option<(&'a [u8], usize)> {
        let c = &self.columns[col];
        let f = self.field(data, row, col)?;
        match c.storage {
            Storage::Fixed => Some((f, c.repeat)),
            Storage::Ascii(_) => Some((f, 1)),
            Storage::VarP | Storage::VarQ => {
                let (n, off) = if c.storage == Storage::VarP {
                    (
                        u64::from(u32::from_be_bytes(f.get(0..4)?.try_into().ok()?)),
                        u64::from(u32::from_be_bytes(f.get(4..8)?.try_into().ok()?)),
                    )
                } else {
                    (
                        u64::from_be_bytes(f.get(0..8)?.try_into().ok()?),
                        u64::from_be_bytes(f.get(8..16)?.try_into().ok()?),
                    )
                };
                let n = usize::try_from(n).ok()?;
                let off = usize::try_from(off).ok()?;
                let bytes = if c.ty == Type::Bit {
                    n.div_ceil(8)
                } else {
                    n.checked_mul(c.ty.size())?
                };
                if off.checked_add(bytes)? > self.heap_len {
                    return None;
                }
                let start = self.heap_offset.checked_add(off)?;
                Some((data.get(start..start + bytes)?, n))
            }
        }
    }

    /// Rows that start within `data`: `NAXIS2` of a truncated file can be far larger.
    pub fn rows_in(&self, data: &[u8]) -> usize {
        if self.row_len == 0 {
            return 0;
        }
        let avail = data.len().saturating_sub(self.data_offset);
        self.rows.min(avail.div_ceil(self.row_len))
    }

    /// Elements in a cell: the repeat count, a variable-length array's length, 1 for an
    /// ASCII field; 0 when the cell lies outside the file or its descriptor is bad.
    pub fn len(&self, data: &[u8], row: usize, col: usize) -> usize {
        match self.columns.get(col) {
            Some(c) if c.ty == Type::Char && !c.is_var() => {
                usize::from(self.field(data, row, col).is_some())
            }
            Some(_) => self.elements(data, row, col).map_or(0, |e| e.1),
            None => 0,
        }
    }

    /// The physical value of element `elem` of a numeric cell (`TZERO + TSCAL × stored`);
    /// `None` when undefined (`TNULL`, NaN, blank) or not a number.
    pub fn f64(&self, data: &[u8], row: usize, col: usize, elem: usize) -> Option<f64> {
        let c = self.columns.get(col)?;
        if let Storage::Ascii(f) = c.storage {
            let s = std::str::from_utf8(self.field(data, row, col)?).ok()?;
            return ascii_number(c, f, s);
        }
        let (bytes, n) = self.elements(data, row, col)?;
        if elem >= n {
            return None;
        }
        let sz = c.ty.size();
        let b = bytes.get(elem * sz..elem * sz + sz)?;
        let int = |v: i64| {
            if c.null == Some(v) {
                None
            } else {
                Some(c.zero + c.scale * v as f64)
            }
        };
        match c.ty {
            Type::U8 => int(i64::from(b[0])),
            Type::I16 => int(i64::from(i16::from_be_bytes([b[0], b[1]]))),
            Type::I32 => int(i64::from(i32::from_be_bytes(b.try_into().ok()?))),
            Type::I64 => {
                let v = i64::from_be_bytes(b.try_into().ok()?);
                if c.unsigned64() {
                    return (c.null != Some(v)).then_some((v as u64 ^ (1 << 63)) as f64);
                }
                int(v)
            }
            Type::F32 => {
                let v = f64::from(f32::from_be_bytes(b.try_into().ok()?));
                (!v.is_nan()).then_some(c.zero + c.scale * v)
            }
            Type::F64 => {
                let v = f64::from_be_bytes(b.try_into().ok()?);
                (!v.is_nan()).then_some(c.zero + c.scale * v)
            }
            Type::C32 => {
                let v = f64::from(f32::from_be_bytes(b[..4].try_into().ok()?));
                (!v.is_nan()).then_some(c.zero + c.scale * v)
            }
            Type::C64 => {
                let v = f64::from_be_bytes(b[..8].try_into().ok()?);
                (!v.is_nan()).then_some(c.zero + c.scale * v)
            }
            Type::Logical => match b[0] {
                b'T' => Some(1.0),
                b'F' => Some(0.0),
                _ => None,
            },
            Type::Bit | Type::Char => None,
        }
    }

    /// Every element of a cell. A character column gives one string (trailing blanks and
    /// NULs dropped; a variable-length one its characters), bits one bool each.
    pub fn cell(&self, data: &[u8], row: usize, col: usize) -> Vec<Datum> {
        let Some(c) = self.columns.get(col) else {
            return Vec::new();
        };
        if let Storage::Ascii(f) = c.storage {
            let Some(s) = self
                .field(data, row, col)
                .and_then(|b| std::str::from_utf8(b).ok())
            else {
                return vec![Datum::Null];
            };
            return vec![match f {
                AsciiForm::Char => Datum::Str(s.trim_end().to_owned()),
                _ if c
                    .null_text
                    .as_deref()
                    .is_some_and(|n| n == s.trim() || (n.is_empty() && s.trim().is_empty())) =>
                {
                    Datum::Null
                }
                AsciiForm::Int if c.scale == 1.0 && c.zero.fract() == 0.0 => {
                    match s.trim().parse::<i64>() {
                        Ok(v) => int_plus(v, c.zero),
                        Err(_) => Datum::Null,
                    }
                }
                _ => ascii_number(c, f, s).map_or(Datum::Null, Datum::Float),
            }];
        }
        let Some((bytes, n)) = self.elements(data, row, col) else {
            return Vec::new();
        };
        if c.ty == Type::Char {
            let s: String = bytes[..n.min(bytes.len())]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect();
            return vec![Datum::Str(s.trim_end().to_owned())];
        }
        if c.ty == Type::Bit {
            return (0..n)
                .map(|k| Datum::Bool(bytes[k / 8] & (0x80 >> (k % 8)) != 0))
                .collect();
        }
        let sz = c.ty.size();
        let ident = c.scale == 1.0 && c.zero.fract() == 0.0 && c.zero.abs() < 9.0e15;
        (0..n)
            .map(|k| {
                let b = &bytes[k * sz..k * sz + sz];
                let int = |v: i64| {
                    if c.null == Some(v) {
                        Datum::Null
                    } else if ident {
                        int_plus(v, c.zero)
                    } else {
                        Datum::Float(c.zero + c.scale * v as f64)
                    }
                };
                let float = |v: f64| {
                    if c.scale == 1.0 && c.zero == 0.0 {
                        Datum::Float(v)
                    } else {
                        Datum::Float(c.zero + c.scale * v)
                    }
                };
                match c.ty {
                    Type::Logical => match b[0] {
                        b'T' => Datum::Bool(true),
                        b'F' => Datum::Bool(false),
                        _ => Datum::Null,
                    },
                    Type::U8 => int(i64::from(b[0])),
                    Type::I16 => int(i64::from(i16::from_be_bytes([b[0], b[1]]))),
                    Type::I32 => int(i64::from(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))),
                    Type::I64 => {
                        let v = i64::from_be_bytes(b.try_into().expect("8 bytes"));
                        if c.unsigned64() {
                            if c.null == Some(v) {
                                Datum::Null
                            } else {
                                Datum::UInt(v as u64 ^ (1 << 63))
                            }
                        } else {
                            int(v)
                        }
                    }
                    Type::F32 => float(f64::from(f32::from_be_bytes([b[0], b[1], b[2], b[3]]))),
                    Type::F64 => float(f64::from_be_bytes(b.try_into().expect("8 bytes"))),
                    Type::C32 => {
                        let re = f32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                        let im = f32::from_be_bytes([b[4], b[5], b[6], b[7]]);
                        Datum::Complex(
                            c.zero + c.scale * f64::from(re),
                            c.zero + c.scale * f64::from(im),
                        )
                    }
                    Type::C64 => {
                        let re = f64::from_be_bytes(b[..8].try_into().expect("8 bytes"));
                        let im = f64::from_be_bytes(b[8..].try_into().expect("8 bytes"));
                        Datum::Complex(c.zero + c.scale * re, c.zero + c.scale * im)
                    }
                    Type::Bit | Type::Char => Datum::Null,
                }
            })
            .collect()
    }

    /// A cell as text for display: one element as it is, several in brackets (at most
    /// `max` of them, then an ellipsis).
    pub fn text(&self, data: &[u8], row: usize, col: usize, max: usize) -> String {
        let cell = self.cell(data, row, col);
        match cell.as_slice() {
            [one] => one.text(),
            many => {
                let mut s: Vec<String> = many.iter().take(max).map(Datum::text).collect();
                if many.len() > max {
                    s.push("…".into());
                }
                format!("[{}]", s.join(", "))
            }
        }
    }

    /// A string cell's text (trailing blanks dropped); `None` for other columns.
    pub fn str<'a>(&self, data: &'a [u8], row: usize, col: usize) -> Option<&'a str> {
        let c = self.columns.get(col)?;
        if c.ty != Type::Char || c.is_var() {
            return None;
        }
        let f = self.field(data, row, col)?;
        let end = f.iter().position(|&b| b == 0).unwrap_or(f.len());
        std::str::from_utf8(&f[..end]).ok().map(str::trim_end)
    }
}

/// An integer cell plus an integral `TZERO`, as a float when the sum overflows.
fn int_plus(v: i64, zero: f64) -> Datum {
    v.checked_add(zero as i64)
        .map_or(Datum::Float(zero + v as f64), Datum::Int)
}

/// A number in an ASCII table field (Fortran `D` exponents; an implied decimal point is not
/// applied, as no writer in use relies on it).
fn ascii_number(c: &Column, f: AsciiForm, s: &str) -> Option<f64> {
    if f == AsciiForm::Char {
        return None;
    }
    let t = s.trim();
    if t.is_empty() || c.null_text.as_deref() == Some(t) {
        return None;
    }
    let v: f64 = t.replace(['D', 'd'], "E").parse().ok()?;
    Some(c.zero + c.scale * v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdr(cards: &[(&str, &str)]) -> Header {
        Header::from_pairs(cards.iter().map(|(k, v)| (*k, *v, "")))
    }

    #[test]
    fn forms() {
        assert_eq!(
            binary_form("1PE(100)"),
            Some((1, Type::F32, Storage::VarP, Some(100)))
        );
        assert_eq!(binary_form("QD"), Some((1, Type::F64, Storage::VarQ, None)));
        assert_eq!(
            binary_form("1024E"),
            Some((1024, Type::F32, Storage::Fixed, None))
        );
        assert_eq!(
            binary_form("12X"),
            Some((12, Type::Bit, Storage::Fixed, None))
        );
        assert_eq!(binary_form("Z"), None);
        assert_eq!(ascii_form("F10.3"), Some((AsciiForm::Float, 10)));
        assert_eq!(ascii_form("A8"), Some((AsciiForm::Char, 8)));
    }

    #[test]
    fn layout_and_values() {
        // Two rows: J with TNULL, 3X bits, A4, 1PI into the heap, I unsigned.
        let h = hdr(&[
            ("NAXIS1", "19"),
            ("NAXIS2", "2"),
            ("PCOUNT", "6"),
            ("TFIELDS", "5"),
            ("TFORM1", "'J'"),
            ("TNULL1", "-1"),
            ("TFORM2", "'3X'"),
            ("TFORM3", "'4A'"),
            ("TFORM4", "'1PI'"),
            ("TFORM5", "'I'"),
            ("TZERO5", "32768"),
            ("TTYPE5", "'U16'"),
        ]);
        let t = Table::from_header(&h, 0, 2 * 19 + 6, false).unwrap();
        assert_eq!(t.columns[3].offset, 9);
        let mut d = Vec::new();
        for (j, bits, s, n, off, u) in [
            (7i32, 0b1010_0000u8, *b"ab  ", 2u32, 0u32, -32768i16),
            (-1, 0b0100_0000, *b"xyz\0", 1, 4, 100),
        ] {
            d.extend(j.to_be_bytes());
            d.push(bits);
            d.extend(s);
            d.extend(n.to_be_bytes());
            d.extend(off.to_be_bytes());
            d.extend(u.to_be_bytes());
        }
        d.extend(5i16.to_be_bytes());
        d.extend(6i16.to_be_bytes());
        d.extend(9i16.to_be_bytes());
        assert_eq!(t.f64(&d, 0, 0, 0), Some(7.0));
        assert_eq!(t.f64(&d, 1, 0, 0), None);
        assert_eq!(t.cell(&d, 1, 0), vec![Datum::Null]);
        assert_eq!(
            t.cell(&d, 0, 1),
            vec![Datum::Bool(true), Datum::Bool(false), Datum::Bool(true)]
        );
        assert_eq!(t.cell(&d, 0, 2), vec![Datum::Str("ab".into())]);
        assert_eq!(t.str(&d, 1, 2), Some("xyz"));
        assert_eq!(t.cell(&d, 0, 3), vec![Datum::Int(5), Datum::Int(6)]);
        assert_eq!(t.cell(&d, 1, 3), vec![Datum::Int(9)]);
        assert_eq!(t.len(&d, 0, 3), 2);
        assert_eq!(t.cell(&d, 0, 4), vec![Datum::Int(0)]);
        assert_eq!(t.f64(&d, 1, 4, 0), Some(32868.0));
        assert_eq!(t.column("u16"), Some(4));
        // A descriptor past the heap reads as nothing.
        let mut bad = d.clone();
        bad[13..17].copy_from_slice(&100u32.to_be_bytes());
        assert_eq!(t.len(&bad, 0, 3), 0);
        // Rows past the end of the file.
        assert_eq!(t.f64(&d[..10], 1, 0, 0), None);
    }

    #[test]
    fn ascii_table() {
        let h = hdr(&[
            ("NAXIS1", "20"),
            ("NAXIS2", "1"),
            ("TFIELDS", "3"),
            ("TFORM1", "'A5'"),
            ("TBCOL1", "1"),
            ("TFORM2", "'I4'"),
            ("TBCOL2", "7"),
            ("TFORM3", "'D9.2'"),
            ("TBCOL3", "12"),
            ("TNULL2", "'-99'"),
        ]);
        let t = Table::from_header(&h, 0, 20, true).unwrap();
        let d = b"M 31   -99 1.5D+02  ";
        assert_eq!(t.cell(d, 0, 0), vec![Datum::Str("M 31".into())]);
        assert_eq!(t.cell(d, 0, 1), vec![Datum::Null]);
        assert_eq!(t.f64(d, 0, 2, 0), Some(150.0));
    }
}
