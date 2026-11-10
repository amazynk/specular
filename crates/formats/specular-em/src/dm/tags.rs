//! The DigitalMicrograph tag tree (DM3 and DM4).
//!
//! The file header (big-endian): version (3 or 4), the file's size (`u32` in DM3, `u64` in
//! DM4) and the byte order of the data (1 = little-endian). Then the root group: `sorted`
//! and `open` bytes and a tag count (`u32` / `u64`). A tag is a kind byte (20 = group,
//! 21 = data), a big-endian `u16` name length and the name (Latin-1), and in DM4 a `u64`
//! byte size of what follows. A data tag is `%%%%`, the number of type words and the type
//! words (big-endian `u32` / `u64`), then the value in the data byte order: a scalar, a
//! string, a struct of scalars, or an array of scalars, structs or strings. Large arrays
//! (the pixels) are not read: they are kept as their offset and length.

use super::Version;
use crate::{Error, Result};

/// A named entry of a group (unnamed entries, list items, have an empty name).
#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
    pub name: String,
    pub node: Node,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Group(Vec<Tag>),
    Data(Value),
}

/// Scalar element types of the tag data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Elem {
    I16,
    I32,
    U16,
    U32,
    F32,
    F64,
    Bool,
    I8,
    U8,
    I64,
    U64,
}

impl Elem {
    fn from_code(c: u64) -> Option<Elem> {
        Some(match c {
            2 => Elem::I16,
            3 => Elem::I32,
            4 => Elem::U16,
            5 => Elem::U32,
            6 => Elem::F32,
            7 => Elem::F64,
            8 => Elem::Bool,
            9 => Elem::I8,
            10 => Elem::U8,
            11 => Elem::I64,
            12 => Elem::U64,
            _ => return None,
        })
    }

    pub fn size(self) -> usize {
        match self {
            Elem::Bool | Elem::I8 | Elem::U8 => 1,
            Elem::I16 | Elem::U16 => 2,
            Elem::I32 | Elem::U32 | Elem::F32 => 4,
            Elem::F64 | Elem::I64 | Elem::U64 => 8,
        }
    }
}

/// A data tag's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    /// A string (type 18), as Latin-1.
    Text(String),
    Struct(Vec<Value>),
    /// An array of scalars, not read: `count` elements at `offset` (`len` bytes). Arrays of
    /// `u16` are often UTF-16 text ([`Value::text`]).
    Array {
        elem: Elem,
        count: u64,
        offset: usize,
        len: usize,
    },
    /// An array of structs (complex pixels, for one), not read: `count` structs at `offset`
    /// (`len` bytes).
    Structs {
        count: u64,
        offset: usize,
        len: usize,
    },
    /// An array of strings.
    List(Vec<Value>),
}

/// Most structs or strings read from one array.
const MAX_LIST: u64 = 1 << 16;
/// Deepest group nesting.
const MAX_DEPTH: usize = 64;
/// Most type words of one data tag.
const MAX_INFO: u64 = 1 << 12;

impl Value {
    /// The value as a number.
    pub fn f64(&self) -> Option<f64> {
        match *self {
            Value::Int(v) => Some(v as f64),
            Value::UInt(v) => Some(v as f64),
            Value::Float(v) => Some(v),
            Value::Bool(b) => Some(f64::from(u8::from(b))),
            _ => None,
        }
    }

    /// The value as a non-negative integer.
    pub fn u64(&self) -> Option<u64> {
        match *self {
            Value::Int(v) => u64::try_from(v).ok(),
            Value::UInt(v) => Some(v),
            Value::Float(v) if v >= 0.0 && v.fract() == 0.0 && v < 1.8e19 => Some(v as u64),
            Value::Bool(b) => Some(u64::from(b)),
            _ => None,
        }
    }

    /// The value as text: a string, or an array of `u16` (UTF-16) or `u8` / `i8` (Latin-1)
    /// read from `file`. NULs end the text.
    pub fn text(&self, file: &[u8], little: bool) -> Option<String> {
        match self {
            Value::Text(s) => Some(s.clone()),
            Value::Array {
                elem: Elem::U16,
                offset,
                len,
                ..
            } => {
                let b = file.get(*offset..offset + len)?;
                let units: Vec<u16> = b
                    .chunks_exact(2)
                    .map(|c| {
                        if little {
                            u16::from_le_bytes([c[0], c[1]])
                        } else {
                            u16::from_be_bytes([c[0], c[1]])
                        }
                    })
                    .take_while(|&u| u != 0)
                    .collect();
                Some(String::from_utf16_lossy(&units))
            }
            Value::Array {
                elem: Elem::U8 | Elem::I8,
                offset,
                len,
                ..
            } => {
                let b = file.get(*offset..offset + len)?;
                Some(latin1(b.split(|&c| c == 0).next().unwrap_or(&[])))
            }
            _ => None,
        }
    }
}

pub(crate) fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// Parses the tag tree; returns the root group's entries.
pub(crate) fn parse(file: &[u8], v: Version) -> Result<Vec<Tag>> {
    let mut p = Parser {
        file,
        pos: v.header_len(),
        v,
    };
    p.group(0)
}

struct Parser<'a> {
    file: &'a [u8],
    pos: usize,
    v: Version,
}

impl Parser<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.file.len())
            .ok_or(Error::Truncated("tag tree"))?;
        let s = &self.file[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn be_u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// A big-endian count or type word: `u32` in DM3, `u64` in DM4.
    fn word(&mut self) -> Result<u64> {
        if self.v.dm4 {
            let b = self.take(8)?;
            let mut a = [0u8; 8];
            a.copy_from_slice(b);
            Ok(u64::from_be_bytes(a))
        } else {
            let b = self.take(4)?;
            Ok(u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
        }
    }

    fn group(&mut self, depth: usize) -> Result<Vec<Tag>> {
        if depth > MAX_DEPTH {
            return Err(Error::Malformed("tag groups nested too deeply".into()));
        }
        let _sorted = self.u8()?;
        let _open = self.u8()?;
        let n = self.word()?;
        // A tag takes at least 3 bytes.
        if n > (self.file.len() - self.pos) as u64 / 3 + 1 {
            return Err(Error::Malformed(format!("{n} tags in a group")));
        }
        let mut tags = Vec::with_capacity(n.min(1024) as usize);
        for _ in 0..n {
            let kind = self.u8()?;
            if kind == 0 {
                // End-of-file padding.
                break;
            }
            let len = usize::from(self.be_u16()?);
            let name = latin1(self.take(len)?);
            let size = if self.v.dm4 { Some(self.word()?) } else { None };
            let start = self.pos;
            let node = match kind {
                20 => Node::Group(self.group(depth + 1)?),
                21 => Node::Data(self.data(&name)?),
                k => {
                    return Err(Error::Malformed(format!("tag kind {k} for {name:?}")));
                }
            };
            // DM4 says how long each tag is; trust it over what was read.
            if let Some(size) = size
                && let Some(end) = (start as u64)
                    .checked_add(size)
                    .filter(|&e| e <= self.file.len() as u64)
            {
                self.pos = end as usize;
            }
            tags.push(Tag { name, node });
        }
        Ok(tags)
    }

    fn data(&mut self, name: &str) -> Result<Value> {
        if self.take(4)? != b"%%%%" {
            return Err(Error::Malformed(format!("data tag {name:?} without %%%%")));
        }
        let n = self.word()?;
        if n == 0 || n > MAX_INFO {
            return Err(Error::Malformed(format!("{n} type words in {name:?}")));
        }
        let info = (0..n).map(|_| self.word()).collect::<Result<Vec<u64>>>()?;
        let mut i = 0;
        let v = self.value(&info, &mut i)?;
        Ok(v)
    }

    fn scalar(&mut self, e: Elem) -> Result<Value> {
        let little = self.v.little;
        let b = self.take(e.size())?;
        let mut a = [0u8; 8];
        if little {
            a[..b.len()].copy_from_slice(b);
        } else {
            // Right-align big-endian bytes, then read as big-endian u64.
            a[8 - b.len()..].copy_from_slice(b);
        }
        let raw = if little {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        };
        Ok(match e {
            Elem::I8 => Value::Int(i64::from(raw as u8 as i8)),
            Elem::I16 => Value::Int(i64::from(raw as u16 as i16)),
            Elem::I32 => Value::Int(i64::from(raw as u32 as i32)),
            Elem::I64 => Value::Int(raw as i64),
            Elem::U8 | Elem::U16 | Elem::U32 | Elem::U64 => Value::UInt(raw),
            Elem::F32 => Value::Float(f64::from(f32::from_bits(raw as u32))),
            Elem::F64 => Value::Float(f64::from_bits(raw)),
            Elem::Bool => Value::Bool(raw != 0),
        })
    }

    fn info(info: &[u64], i: &mut usize) -> Result<u64> {
        let v = *info
            .get(*i)
            .ok_or_else(|| Error::Malformed("short type words".into()))?;
        *i += 1;
        Ok(v)
    }

    /// A struct's field types (after its type word 15).
    fn struct_def(info: &[u64], i: &mut usize) -> Result<Vec<Elem>> {
        let _name_len = Self::info(info, i)?;
        let n = Self::info(info, i)?;
        if n > MAX_INFO {
            return Err(Error::Malformed("struct with too many fields".into()));
        }
        (0..n)
            .map(|_| {
                let _field_name_len = Self::info(info, i)?;
                let t = Self::info(info, i)?;
                Elem::from_code(t)
                    .ok_or_else(|| Error::Unsupported(format!("struct field of type {t}")))
            })
            .collect()
    }

    fn value(&mut self, info: &[u64], i: &mut usize) -> Result<Value> {
        let t = Self::info(info, i)?;
        if let Some(e) = Elem::from_code(t) {
            return self.scalar(e);
        }
        match t {
            15 => {
                let fields = Self::struct_def(info, i)?;
                Ok(Value::Struct(
                    fields
                        .into_iter()
                        .map(|e| self.scalar(e))
                        .collect::<Result<_>>()?,
                ))
            }
            18 => {
                let len = Self::info(info, i)?;
                let b = self.take(usize::try_from(len).map_err(|_| Error::TooLarge)?)?;
                Ok(Value::Text(latin1(
                    b.split(|&c| c == 0).next().unwrap_or(&[]),
                )))
            }
            20 => {
                let et = Self::info(info, i)?;
                if let Some(elem) = Elem::from_code(et) {
                    let count = Self::info(info, i)?;
                    let len = count
                        .checked_mul(elem.size() as u64)
                        .and_then(|l| usize::try_from(l).ok())
                        .ok_or(Error::TooLarge)?;
                    let offset = self.pos;
                    self.take(len)?;
                    return Ok(Value::Array {
                        elem,
                        count,
                        offset,
                        len,
                    });
                }
                match et {
                    15 => {
                        let fields = Self::struct_def(info, i)?;
                        let count = Self::info(info, i)?;
                        let size: usize = fields.iter().map(|f| f.size()).sum();
                        let len = count
                            .checked_mul(size as u64)
                            .and_then(|l| usize::try_from(l).ok())
                            .ok_or(Error::TooLarge)?;
                        let offset = self.pos;
                        self.take(len)?;
                        Ok(Value::Structs { count, offset, len })
                    }
                    18 => {
                        let len = Self::info(info, i)?;
                        let count = Self::info(info, i)?;
                        let total = len
                            .checked_mul(count)
                            .and_then(|l| usize::try_from(l).ok())
                            .ok_or(Error::TooLarge)?;
                        let b = self.take(total)?;
                        if count > MAX_LIST || len == 0 {
                            return Ok(Value::List(Vec::new()));
                        }
                        Ok(Value::List(
                            b.chunks(len as usize)
                                .map(|c| {
                                    Value::Text(latin1(c.split(|&x| x == 0).next().unwrap_or(&[])))
                                })
                                .collect(),
                        ))
                    }
                    t => Err(Error::Unsupported(format!("array of type {t}"))),
                }
            }
            t => Err(Error::Unsupported(format!("tag data of type {t}"))),
        }
    }
}

/// Finds a tag by a dotted path of names below `tags` ("ImageData.Calibrations").
pub fn find<'a>(tags: &'a [Tag], path: &str) -> Option<&'a Node> {
    let mut cur = tags;
    let mut node = None;
    for part in path.split('.') {
        let t = cur.iter().find(|t| t.name == part)?;
        node = Some(&t.node);
        cur = match &t.node {
            Node::Group(g) => g,
            Node::Data(_) => &[],
        };
    }
    node
}

/// The group at `path`.
pub fn group<'a>(tags: &'a [Tag], path: &str) -> Option<&'a [Tag]> {
    match find(tags, path)? {
        Node::Group(g) => Some(g),
        Node::Data(_) => None,
    }
}

/// The data value at `path`.
pub fn value<'a>(tags: &'a [Tag], path: &str) -> Option<&'a Value> {
    match find(tags, path)? {
        Node::Data(v) => Some(v),
        Node::Group(_) => None,
    }
}

/// Most entries [`flatten`] lists.
const MAX_FLAT: usize = 4000;

/// The data tags below `tags` as (dotted path, value as text), depth first, at most
/// [`MAX_FLAT`]. Unnamed list items are numbered; long arrays are summarised by their length.
pub fn flatten(tags: &[Tag], file: &[u8], little: bool) -> Vec<(String, String)> {
    fn scalar(v: &Value) -> Option<String> {
        Some(match v {
            Value::Int(i) => i.to_string(),
            Value::UInt(u) => u.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Text(s) => s.clone(),
            _ => return None,
        })
    }
    fn text(v: &Value, file: &[u8], little: bool) -> String {
        match v {
            Value::Struct(items) => format!(
                "({})",
                items
                    .iter()
                    .map(|i| scalar(i).unwrap_or_else(|| "…".into()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::List(items) => items
                .iter()
                .map(|i| scalar(i).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", "),
            Value::Array {
                elem: Elem::U16 | Elem::U8 | Elem::I8,
                count,
                ..
            } if *count <= 1024 => v.text(file, little).unwrap_or_default(),
            Value::Array { count, .. } | Value::Structs { count, .. } => {
                format!("[{count} values]")
            }
            v => scalar(v).unwrap_or_default(),
        }
    }
    fn walk(
        tags: &[Tag],
        prefix: &str,
        file: &[u8],
        little: bool,
        out: &mut Vec<(String, String)>,
    ) {
        for (k, t) in tags.iter().enumerate() {
            if out.len() >= MAX_FLAT {
                return;
            }
            let name = if t.name.is_empty() {
                k.to_string()
            } else {
                t.name.clone()
            };
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}.{name}")
            };
            match &t.node {
                Node::Group(g) => walk(g, &path, file, little, out),
                Node::Data(v) => out.push((path, text(v, file, little))),
            }
        }
    }
    let mut out = Vec::new();
    walk(tags, "", file, little, &mut out);
    out
}
