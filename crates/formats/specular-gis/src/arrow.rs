//! Apache Arrow: the IPC schema (also embedded base64 in Parquet as `ARROW:schema`), IPC
//! files and streams (`.arrow`, `.feather` v2, `.arrows`) with LZ4-frame or Zstandard body
//! compression and dictionaries, and GeoArrow geometry: WKB, WKT and the native nested
//! encodings (point, linestring, polygon and their multi forms, with separated `x`/`y`
//! structs or interleaved fixed-size lists).
//!
//! Arrays decode into [`Arr`]; [`Nested`] is the coordinate layout both this and the
//! Parquet reader rebuild native geometries from. View types and unions are refused.

use crate::flatbuf::{Table, Vector};
use crate::model::{Geometry, Pt, Value};

// ------------------------------------------------------------------------------ schema

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Null,
    Int {
        bits: u8,
        signed: bool,
    },
    Float(u8),
    Binary,
    LargeBinary,
    Utf8,
    LargeUtf8,
    Bool,
    Decimal {
        scale: i32,
        bits: u16,
    },
    /// Date in days (true) or milliseconds.
    Date(bool),
    /// Time of day: units per second, bit width.
    Time(u32, u8),
    /// Timestamp: units per second, and the zone's offset from UTC in minutes when the
    /// type names a fixed one ("+02:00", "UTC"); other zones show in UTC.
    Timestamp(u32, Option<i32>),
    Duration,
    List,
    LargeList,
    FixedList(usize),
    FixedBinary(usize),
    Struct,
    Map,
    /// Not readable here: named.
    Other(&'static str),
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub children: Vec<Field>,
    /// Dictionary id and index type, when dictionary-encoded.
    pub dict: Option<(i64, Type)>,
    pub meta: Vec<(String, String)>,
}

impl Field {
    pub fn meta(&self, k: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
    /// The GeoArrow extension name ("geoarrow.wkb", "geoarrow.point", …).
    pub fn extension(&self) -> Option<&str> {
        self.meta("ARROW:extension:name")
    }
}

fn kv(v: Option<Vector<'_>>) -> Vec<(String, String)> {
    v.map(|v| {
        v.tables()
            .filter_map(|t| Some((t.str(0)?, t.str(1).unwrap_or_default())))
            .collect()
    })
    .unwrap_or_default()
}

fn int_type(t: Option<Table<'_>>) -> Type {
    let t = match t {
        Some(t) => t,
        None => {
            return Type::Int {
                bits: 32,
                signed: true,
            };
        }
    };
    Type::Int {
        bits: u8::try_from(t.i32(0, 32)).unwrap_or(32),
        signed: t.bool(1, false),
    }
}

fn unit(u: i16) -> u32 {
    match u {
        0 => 1,
        1 => 1000,
        2 => 1_000_000,
        _ => 1_000_000_000,
    }
}

/// Most fields read from a schema, children included: tables may be shared, so nested
/// children could otherwise multiply without bound below the depth cap.
const MAX_FIELDS: usize = 1 << 16;

/// The fields of `v`, while the budget lasts.
fn fields(v: Vector<'_>, depth: usize, budget: &mut usize) -> Vec<Field> {
    let mut out = Vec::new();
    for t in v.tables() {
        if *budget == 0 {
            break;
        }
        out.push(field(t, depth, budget));
    }
    out
}

fn field(t: Table<'_>, depth: usize, budget: &mut usize) -> Field {
    *budget = budget.saturating_sub(1);
    let tt = t.u8(2, 0);
    let ty_t = t.table(3);
    let get_i32 = |i, d| ty_t.map_or(d, |x| x.i32(i, d));
    let get_i16 = |i, d| ty_t.map_or(d, |x| x.i16(i, d));
    let ty = match tt {
        1 => Type::Null,
        2 => int_type(ty_t),
        3 => Type::Float(match get_i16(0, 0) {
            0 => 16,
            1 => 32,
            _ => 64,
        }),
        4 => Type::Binary,
        5 => Type::Utf8,
        6 => Type::Bool,
        7 => Type::Decimal {
            scale: get_i32(1, 0),
            bits: u16::try_from(get_i32(2, 128)).unwrap_or(128),
        },
        8 => Type::Date(get_i16(0, 1) == 0),
        9 => Type::Time(
            unit(get_i16(0, 1)),
            u8::try_from(get_i32(1, 32)).unwrap_or(32),
        ),
        10 => Type::Timestamp(
            unit(get_i16(0, 0)),
            ty_t.and_then(|t| t.str(1)).as_deref().and_then(tz_offset),
        ),
        11 => Type::Other("interval"),
        12 => Type::List,
        13 => Type::Struct,
        14 => Type::Other("union"),
        15 => Type::FixedBinary(usize::try_from(get_i32(0, 0)).unwrap_or(0)),
        16 => Type::FixedList(usize::try_from(get_i32(0, 0)).unwrap_or(0)),
        17 => Type::Map,
        18 => Type::Duration,
        19 => Type::LargeBinary,
        20 => Type::LargeUtf8,
        21 => Type::LargeList,
        22 => Type::Other("run-end encoded"),
        23 => Type::Other("binary view"),
        24 => Type::Other("string view"),
        25 => Type::Other("list view"),
        26 => Type::Other("large list view"),
        _ => Type::Other("unknown"),
    };
    let dict = t.table(4).map(|d| (d.i64(0, 0), int_type(d.table(1))));
    let children = if depth < 32 {
        t.vector(5)
            .map(|v| fields(v, depth + 1, budget))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Field {
        name: t.str(0).unwrap_or_default(),
        ty,
        children,
        dict,
        meta: kv(t.vector(6)),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub fields: Vec<Field>,
    pub meta: Vec<(String, String)>,
}

impl Schema {
    pub fn meta(&self, k: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    }
}

fn schema_of(t: Table<'_>) -> Schema {
    let mut budget = MAX_FIELDS;
    Schema {
        fields: t
            .vector(1)
            .map(|v| fields(v, 0, &mut budget))
            .unwrap_or_default(),
        meta: kv(t.vector(2)),
    }
}

/// One IPC message: its flatbuffer and where its body starts. `None` at the end mark.
fn message(data: &[u8], at: usize) -> Result<Option<(Table<'_>, usize, usize)>, String> {
    let rd = |i: usize| -> Option<u32> {
        Some(u32::from_le_bytes(data.get(i..i + 4)?.try_into().ok()?))
    };
    let mut i = at;
    let mut len = rd(i).ok_or("an Arrow message is truncated")?;
    i += 4;
    if len == 0xFFFF_FFFF {
        len = rd(i).ok_or("an Arrow message is truncated")?;
        i += 4;
    }
    if len == 0 {
        return Ok(None);
    }
    let len = len as usize;
    let meta = data
        .get(i..i.checked_add(len).ok_or("bad message")?)
        .ok_or("an Arrow message is truncated")?;
    let m = Table::root(meta).ok_or("an Arrow message is damaged")?;
    let body_len = usize::try_from(m.i64(3, 0)).map_err(|_| "bad body length")?;
    let body = i + len;
    if body.checked_add(body_len).is_none_or(|e| e > data.len()) {
        return Err("an Arrow message body is truncated".into());
    }
    Ok(Some((m, body, body_len)))
}

/// Standard base64 (padding optional, whitespace ignored).
pub fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut n) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
    }
    Some(out)
}

/// The schema of an IPC message (Parquet's `ARROW:schema`, base64).
pub fn schema_from_base64(s: &str) -> Option<Schema> {
    let raw = base64(s)?;
    let (m, _, _) = message(&raw, 0).ok()??;
    (m.u8(1, 0) == 1).then(|| m.table(2).map(schema_of))?
}

// ------------------------------------------------------------------------------ arrays

/// A decoded array.
#[derive(Clone, Debug)]
pub enum Arr {
    Null(usize),
    /// Fixed-width values (bytes of `width` each; bits for Bool, width 0).
    Prim {
        valid: Option<Vec<bool>>,
        data: Vec<u8>,
        width: usize,
        len: usize,
    },
    Bytes {
        valid: Option<Vec<bool>>,
        offsets: Vec<i64>,
        data: Vec<u8>,
    },
    List {
        valid: Option<Vec<bool>>,
        offsets: Vec<i64>,
        child: Box<Arr>,
    },
    FixedList {
        valid: Option<Vec<bool>>,
        size: usize,
        len: usize,
        child: Box<Arr>,
    },
    Struct {
        valid: Option<Vec<bool>>,
        len: usize,
        children: Vec<Arr>,
    },
    Dict {
        indices: Box<Arr>,
        dict: std::sync::Arc<Arr>,
    },
}

impl Arr {
    pub fn len(&self) -> usize {
        match self {
            Arr::Null(n) => *n,
            Arr::Prim { len, .. } | Arr::FixedList { len, .. } | Arr::Struct { len, .. } => *len,
            Arr::Bytes { offsets, .. } | Arr::List { offsets, .. } => {
                offsets.len().saturating_sub(1)
            }
            Arr::Dict { indices, .. } => indices.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_valid(&self, i: usize) -> bool {
        let v = match self {
            Arr::Null(_) => return false,
            Arr::Prim { valid, .. }
            | Arr::Bytes { valid, .. }
            | Arr::List { valid, .. }
            | Arr::FixedList { valid, .. }
            | Arr::Struct { valid, .. } => valid,
            Arr::Dict { indices, .. } => return indices.is_valid(i),
        };
        v.as_ref()
            .is_none_or(|v| v.get(i).copied().unwrap_or(false))
    }

    pub fn bytes(&self, i: usize) -> Option<&[u8]> {
        match self {
            Arr::Bytes { offsets, data, .. } => {
                let s = usize::try_from(*offsets.get(i)?).ok()?;
                let e = usize::try_from(*offsets.get(i + 1)?).ok()?;
                data.get(s..e)
            }
            Arr::Prim { data, width, .. } if *width > 0 => data.get(i * width..(i + 1) * width),
            Arr::Dict { indices, dict } => dict.bytes(usize::try_from(indices.int(i)?).ok()?),
            _ => None,
        }
    }

    /// An integer value (any width; little-endian, signed per `signed` of the caller's
    /// type, read here as signed except 64-bit unsigned handled by the caller).
    pub fn int(&self, i: usize) -> Option<i64> {
        match self {
            Arr::Prim { data, width, .. } => {
                let b = data.get(i * width..(i + 1) * width)?;
                Some(match width {
                    1 => i64::from(b[0] as i8),
                    2 => i64::from(i16::from_le_bytes([b[0], b[1]])),
                    4 => i64::from(i32::from_le_bytes(b.try_into().ok()?)),
                    8 => i64::from_le_bytes(b.try_into().ok()?),
                    _ => return None,
                })
            }
            _ => None,
        }
    }

    pub fn uint(&self, i: usize) -> Option<u64> {
        match self {
            Arr::Prim { data, width, .. } => {
                let b = data.get(i * width..(i + 1) * width)?;
                let mut v = [0u8; 8];
                v[..*width.min(&8)].copy_from_slice(&b[..*width.min(&8)]);
                Some(u64::from_le_bytes(v))
            }
            _ => None,
        }
    }

    pub fn f64(&self, i: usize) -> Option<f64> {
        match self {
            Arr::Prim { data, width: 8, .. } => Some(f64::from_le_bytes(
                data.get(i * 8..i * 8 + 8)?.try_into().ok()?,
            )),
            Arr::Prim { data, width: 4, .. } => Some(f64::from(f32::from_le_bytes(
                data.get(i * 4..i * 4 + 4)?.try_into().ok()?,
            ))),
            _ => None,
        }
    }

    pub fn bit(&self, i: usize) -> Option<bool> {
        match self {
            Arr::Prim { data, width: 0, .. } => Some(data.get(i / 8)? >> (i % 8) & 1 != 0),
            _ => None,
        }
    }
}

fn bits(b: &[u8], n: usize) -> Vec<bool> {
    (0..n)
        .map(|k| b.get(k / 8).is_some_and(|x| x >> (k % 8) & 1 != 0))
        .collect()
}

struct Body<'a> {
    body: &'a [u8],
    buffers: Vec<(usize, usize)>,
    nodes: Vec<(usize, usize)>,
    next_buf: usize,
    next_node: usize,
    codec: Option<u8>,
    limit: usize,
    variadic: Vec<i64>,
    next_var: usize,
}

impl Body<'_> {
    fn buffer(&mut self) -> Result<Vec<u8>, String> {
        let (off, len) = *self
            .buffers
            .get(self.next_buf)
            .ok_or("an Arrow record batch has too few buffers")?;
        self.next_buf += 1;
        let b = self
            .body
            .get(off..off.checked_add(len).ok_or("bad buffer")?)
            .ok_or("an Arrow buffer lies outside its message")?;
        match self.codec {
            None => Ok(b.to_vec()),
            Some(_) if b.is_empty() => Ok(Vec::new()),
            Some(c) => {
                let n = i64::from_le_bytes(
                    b.get(..8)
                        .ok_or("a compressed Arrow buffer is truncated")?
                        .try_into()
                        .unwrap_or_default(),
                );
                if n == -1 {
                    return Ok(b[8..].to_vec());
                }
                let n = usize::try_from(n).map_err(|_| "bad buffer size")?;
                // `limit` is what all buffers of the file may decompress to, together.
                self.limit = self
                    .limit
                    .checked_sub(n)
                    .ok_or("an Arrow buffer is too large")?;
                let out = if c == 0 {
                    crate::codec::lz4_frame(&b[8..], n)?
                } else {
                    crate::codec::zstd(&b[8..], n)?
                };
                if out.len() != n {
                    return Err("an Arrow buffer decompresses to the wrong size".into());
                }
                Ok(out)
            }
        }
    }

    fn node(&mut self) -> Result<(usize, usize), String> {
        let n = *self
            .nodes
            .get(self.next_node)
            .ok_or("an Arrow record batch has too few nodes")?;
        self.next_node += 1;
        Ok(n)
    }

    fn validity(&mut self, len: usize, nulls: usize) -> Result<Option<Vec<bool>>, String> {
        let b = self.buffer()?;
        if nulls == 0 || b.is_empty() {
            return Ok(None);
        }
        // One bit per value: a long array over a short buffer is damaged, not padded.
        if b.len().saturating_mul(8) < len {
            return Err("an Arrow validity buffer is too short".into());
        }
        Ok(Some(bits(&b, len)))
    }

    fn offsets(&mut self, len: usize, large: bool) -> Result<Vec<i64>, String> {
        let b = self.buffer()?;
        if len == 0 && b.is_empty() {
            return Ok(vec![0]);
        }
        let w = if large { 8 } else { 4 };
        let b = b
            .get(..(len + 1) * w)
            .ok_or("Arrow offsets are truncated")?;
        let v: Vec<i64> = if large {
            b.chunks_exact(8)
                .map(|c| i64::from_le_bytes(c.try_into().unwrap_or_default()))
                .collect()
        } else {
            b.chunks_exact(4)
                .map(|c| i64::from(i32::from_le_bytes(c.try_into().unwrap_or_default())))
                .collect()
        };
        if v.windows(2).any(|w| w[1] < w[0]) || v.first().is_some_and(|&x| x < 0) {
            return Err("Arrow offsets go backwards".into());
        }
        Ok(v)
    }

    fn array(
        &mut self,
        f: &Field,
        dicts: &std::collections::HashMap<i64, std::sync::Arc<Arr>>,
        depth: usize,
    ) -> Result<Arr, String> {
        if depth > 32 {
            return Err("Arrow arrays nest too deep".into());
        }
        let (len, nulls) = self.node()?;
        if len > self.limit {
            return Err("an Arrow array is too long".into());
        }
        if let Some((id, it)) = &f.dict {
            let valid = self.validity(len, nulls)?;
            let w = match it {
                Type::Int { bits, .. } => usize::from(*bits / 8).max(1),
                _ => 4,
            };
            let data = self.buffer()?;
            let indices = Arr::Prim {
                valid,
                data,
                width: w,
                len,
            };
            let dict = dicts
                .get(id)
                .cloned()
                .ok_or("an Arrow dictionary is missing")?;
            return Ok(Arr::Dict {
                indices: Box::new(indices),
                dict,
            });
        }
        let prim = |s: &mut Self, width: usize| -> Result<Arr, String> {
            let valid = s.validity(len, nulls)?;
            let data = s.buffer()?;
            if data.len() < (len * width).max(if width == 0 { len.div_ceil(8) } else { 0 }) {
                return Err("an Arrow buffer is too short".into());
            }
            Ok(Arr::Prim {
                valid,
                data,
                width,
                len,
            })
        };
        Ok(match &f.ty {
            Type::Null => Arr::Null(len),
            Type::Int { bits, .. } => prim(self, usize::from(*bits / 8))?,
            Type::Float(b) => prim(self, usize::from(*b / 8))?,
            Type::Bool => prim(self, 0)?,
            Type::Decimal { bits, .. } => prim(self, usize::from(*bits / 8))?,
            Type::Date(days) => prim(self, if *days { 4 } else { 8 })?,
            Type::Time(_, b) => prim(self, usize::from(*b / 8))?,
            Type::Timestamp(..) | Type::Duration => prim(self, 8)?,
            Type::FixedBinary(w) => prim(self, *w)?,
            Type::Binary | Type::Utf8 | Type::LargeBinary | Type::LargeUtf8 => {
                let large = matches!(f.ty, Type::LargeBinary | Type::LargeUtf8);
                let valid = self.validity(len, nulls)?;
                let offsets = self.offsets(len, large)?;
                let data = self.buffer()?;
                if offsets.last().is_some_and(|&e| e as usize > data.len()) {
                    return Err("Arrow string data is truncated".into());
                }
                Arr::Bytes {
                    valid,
                    offsets,
                    data,
                }
            }
            Type::List | Type::LargeList | Type::Map => {
                let valid = self.validity(len, nulls)?;
                let offsets = self.offsets(len, f.ty == Type::LargeList)?;
                let c = f.children.first().ok_or("an Arrow list has no child")?;
                let child = self.array(c, dicts, depth + 1)?;
                if offsets.last().is_some_and(|&e| e as usize > child.len()) {
                    return Err("Arrow list offsets pass their values".into());
                }
                Arr::List {
                    valid,
                    offsets,
                    child: Box::new(child),
                }
            }
            Type::FixedList(size) => {
                let valid = self.validity(len, nulls)?;
                let c = f.children.first().ok_or("an Arrow list has no child")?;
                let child = self.array(c, dicts, depth + 1)?;
                if child.len() < len * size {
                    return Err("an Arrow fixed-size list is short".into());
                }
                Arr::FixedList {
                    valid,
                    size: *size,
                    len,
                    child: Box::new(child),
                }
            }
            Type::Struct => {
                let valid = self.validity(len, nulls)?;
                let mut children = Vec::new();
                for c in &f.children {
                    children.push(self.array(c, dicts, depth + 1)?);
                }
                Arr::Struct {
                    valid,
                    len,
                    children,
                }
            }
            // Not shown, but their buffers are stepped over so later columns still read.
            Type::Other(what) => {
                match *what {
                    "binary view" | "string view" => {
                        self.buffer()?;
                        self.buffer()?;
                        let n = self.variadic.get(self.next_var).copied().unwrap_or(0);
                        self.next_var += 1;
                        for _ in 0..n {
                            self.buffer()?;
                        }
                    }
                    "list view" | "large list view" => {
                        for _ in 0..3 {
                            self.buffer()?;
                        }
                        if let Some(c) = f.children.first() {
                            self.array(c, dicts, depth + 1)?;
                        }
                    }
                    "interval" => {
                        self.buffer()?;
                        self.buffer()?;
                    }
                    "run-end encoded" => {
                        for c in &f.children {
                            self.array(c, dicts, depth + 1)?;
                        }
                    }
                    _ => return Err(format!("Arrow {what} columns are not supported")),
                }
                Arr::Null(len)
            }
        })
    }
}

fn body<'a>(rb: Table<'_>, body: &'a [u8], limit: usize) -> Result<Body<'a>, String> {
    let structs = |v: Option<Vector<'_>>| -> Result<Vec<(usize, usize)>, String> {
        let Some(v) = v else { return Ok(Vec::new()) };
        (0..v.len)
            .map(|i| {
                let b = v.struct_bytes(i, 16).ok_or("bad Arrow record batch")?;
                let a = i64::from_le_bytes(b[..8].try_into().unwrap_or_default());
                let c = i64::from_le_bytes(b[8..].try_into().unwrap_or_default());
                Ok((
                    usize::try_from(a).map_err(|_| "bad Arrow record batch")?,
                    usize::try_from(c).map_err(|_| "bad Arrow record batch")?,
                ))
            })
            .collect()
    };
    Ok(Body {
        body,
        nodes: structs(rb.vector(1))?,
        buffers: structs(rb.vector(2))?,
        next_buf: 0,
        next_node: 0,
        codec: rb.table(3).map(|c| c.u8(0, 0)),
        limit,
        variadic: rb
            .vector(4)
            .and_then(|v| v.bytes(8))
            .map(|b| {
                b.chunks_exact(8)
                    .map(|c| i64::from_le_bytes(c.try_into().unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default(),
        next_var: 0,
    })
}

/// An IPC file or stream: its schema and each record batch's columns.
pub struct Ipc {
    pub schema: Schema,
    pub batches: Vec<Vec<Arr>>,
}

pub fn sniff_file(head: &[u8]) -> bool {
    head.starts_with(b"ARROW1")
}

/// An IPC stream starts with a schema message.
pub fn sniff_stream(head: &[u8]) -> bool {
    let Ok(Some((m, _, _))) = message(head, 0) else {
        return false;
    };
    m.u8(1, 0) == 1
}

/// Reads an Arrow IPC file (`ARROW1` magic) or stream.
pub fn read_ipc(data: &[u8], limit: usize) -> Result<Ipc, String> {
    let mut at = if sniff_file(data) { 8 } else { 0 };
    let mut schema_: Option<Schema> = None;
    let mut dicts: std::collections::HashMap<i64, std::sync::Arc<Arr>> = Default::default();
    // Decompressed bytes left for the whole file (each batch spends from it).
    let mut limit = limit;
    let mut batches = Vec::new();
    while at + 8 <= data.len() {
        if data[at..].starts_with(b"ARROW1") {
            break;
        }
        let Some((m, b, blen)) = message(data, at)? else {
            break;
        };
        let meta_end = b;
        let bodyb = &data[b..b + blen];
        match m.u8(1, 0) {
            1 => schema_ = m.table(2).map(schema_of),
            2 => {
                let s = schema_
                    .as_ref()
                    .ok_or("an Arrow dictionary comes before the schema")?;
                let d = m.table(2).ok_or("bad Arrow dictionary")?;
                let id = d.i64(0, 0);
                let rb = d.table(1).ok_or("bad Arrow dictionary")?;
                // The dictionary's value type is the field's own type.
                let f = find_dict_field(&s.fields, id).ok_or("an Arrow dictionary has no field")?;
                let mut vf = f.clone();
                vf.dict = None;
                // Dictionary values that are dictionary-encoded themselves would chain each
                // dictionary to the one before (dropping a long chain overflows the stack).
                if has_dict(&vf.children) {
                    return Err("nested Arrow dictionaries are not supported".into());
                }
                let mut bd = body(rb, bodyb, limit)?;
                let arr = bd.array(&vf, &dicts, 0)?;
                limit = bd.limit;
                if d.bool(2, false) {
                    return Err("Arrow delta dictionaries are not supported".into());
                }
                dicts.insert(id, std::sync::Arc::new(arr));
            }
            3 => {
                let s = schema_
                    .as_ref()
                    .ok_or("an Arrow batch comes before the schema")?;
                let rb = m.table(2).ok_or("bad Arrow record batch")?;
                let mut bd = body(rb, bodyb, limit)?;
                let mut cols = Vec::with_capacity(s.fields.len());
                for f in &s.fields {
                    cols.push(bd.array(f, &dicts, 0)?);
                }
                limit = bd.limit;
                batches.push(cols);
            }
            _ => {}
        }
        at = meta_end + blen;
    }
    Ok(Ipc {
        schema: schema_.ok_or("no Arrow schema")?,
        batches,
    })
}

fn has_dict(fields: &[Field]) -> bool {
    fields
        .iter()
        .any(|f| f.dict.is_some() || has_dict(&f.children))
}

fn find_dict_field(fields: &[Field], id: i64) -> Option<&Field> {
    for f in fields {
        if f.dict.as_ref().is_some_and(|(i, _)| *i == id) {
            return Some(f);
        }
        if let Some(c) = find_dict_field(&f.children, id) {
            return Some(c);
        }
    }
    None
}

// ---------------------------------------------------------------------- values

fn civil(days: i64) -> (i64, u32, u32) {
    // Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn date_text(days: i64) -> String {
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A timestamp in `per_second` units since the epoch as ISO 8601.
pub fn timestamp_text(v: i64, per_second: u32) -> String {
    let ps = i64::from(per_second.max(1));
    let secs = v.div_euclid(ps);
    let frac = v.rem_euclid(ps);
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let mut s = format!(
        "{}T{:02}:{:02}:{:02}",
        date_text(days),
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    );
    if frac != 0 {
        let digits = ps.ilog10() as usize;
        let f = format!("{frac:0digits$}");
        s.push('.');
        s.push_str(f.trim_end_matches('0'));
    }
    s
}

/// A fixed time zone's offset in minutes: "+02:00", "-0215", "UTC", "Z", "Etc/UTC".
pub fn tz_offset(tz: &str) -> Option<i32> {
    let t = tz.trim();
    if matches!(
        t.to_ascii_uppercase().as_str(),
        "UTC" | "Z" | "GMT" | "ETC/UTC" | "ETC/GMT"
    ) {
        return Some(0);
    }
    let sign = match t.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits: String = t[1..].chars().filter(char::is_ascii_digit).collect();
    let (h, m) = match digits.len() {
        2 => (digits.parse::<i32>().ok()?, 0),
        4 => (
            digits[..2].parse::<i32>().ok()?,
            digits[2..].parse::<i32>().ok()?,
        ),
        _ => return None,
    };
    Some(sign * (h * 60 + m))
}

/// A timestamp as local time in a fixed zone with its offset ("…T14:00:00+02:00"), or in
/// UTC without one.
pub fn timestamp_tz(v: i64, per_second: u32, tz: Option<i32>) -> String {
    match tz {
        None => timestamp_text(v, per_second),
        Some(off) => {
            let shifted = v.saturating_add(i64::from(off) * 60 * i64::from(per_second.max(1)));
            let sign = if off < 0 { '-' } else { '+' };
            format!(
                "{}{sign}{:02}:{:02}",
                timestamp_text(shifted, per_second),
                off.abs() / 60,
                off.abs() % 60
            )
        }
    }
}

/// A decimal stored as little-endian two's complement bytes, scaled.
fn decimal(b: &[u8], scale: i32, big_endian: bool) -> f64 {
    let mut v: i128 = 0;
    let n = b.len().min(16);
    let bytes: Vec<u8> = if big_endian {
        b[..n].iter().rev().copied().collect()
    } else {
        b[..n].to_vec()
    };
    for (k, &x) in bytes.iter().enumerate() {
        v |= i128::from(x) << (8 * k);
    }
    if n < 16 && bytes.last().is_some_and(|&x| x & 0x80 != 0) {
        v -= 1i128 << (8 * n);
    }
    v as f64 / 10f64.powi(scale)
}

pub fn decimal_be(b: &[u8], scale: i32) -> f64 {
    decimal(b, scale, true)
}

/// Value `i` of a flat attribute array of type `t`.
pub fn value(a: &Arr, t: &Type, i: usize) -> Value {
    if !a.is_valid(i) {
        return Value::Null;
    }
    if let Arr::Dict { indices, dict } = a {
        return match indices.int(i).and_then(|k| usize::try_from(k).ok()) {
            Some(k) => value(dict, t, k),
            None => Value::Null,
        };
    }
    match t {
        Type::Null => Value::Null,
        Type::Bool => a.bit(i).map_or(Value::Null, Value::Bool),
        Type::Int {
            bits: 64,
            signed: false,
        } => a.uint(i).map_or(Value::Null, |u| {
            i64::try_from(u).map_or(Value::Float(u as f64), Value::Int)
        }),
        Type::Int { signed: false, .. } => a.uint(i).map_or(Value::Null, |u| Value::Int(u as i64)),
        Type::Int { .. } => a.int(i).map_or(Value::Null, Value::Int),
        Type::Float(16) => a
            .uint(i)
            .map_or(Value::Null, |h| Value::Float(half(h as u16))),
        Type::Float(_) => a.f64(i).map_or(Value::Null, Value::Float),
        Type::Utf8 | Type::LargeUtf8 => a.bytes(i).map_or(Value::Null, |b| {
            Value::Text(String::from_utf8_lossy(b).into_owned())
        }),
        Type::Binary | Type::LargeBinary | Type::FixedBinary(_) => {
            a.bytes(i).map_or(Value::Null, |b| Value::Blob(b.len()))
        }
        Type::Decimal { scale, .. } => a
            .bytes(i)
            .map_or(Value::Null, |b| Value::Float(decimal(b, *scale, false))),
        Type::Date(true) => a.int(i).map_or(Value::Null, |d| Value::Date(date_text(d))),
        Type::Date(false) => a.int(i).map_or(Value::Null, |ms| {
            Value::Date(date_text(ms.div_euclid(86_400_000)))
        }),
        Type::Timestamp(u, tz) => a
            .int(i)
            .map_or(Value::Null, |v| Value::Date(timestamp_tz(v, *u, *tz))),
        Type::Time(u, _) => a.int(i).map_or(Value::Null, |v| {
            let t = timestamp_text(v, *u);
            Value::Date(t.split_once('T').map_or(t.clone(), |(_, b)| b.to_string()))
        }),
        Type::Duration => a.int(i).map_or(Value::Null, Value::Int),
        _ => Value::Null,
    }
}

fn half(h: u16) -> f64 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1F);
    let m = f64::from(h & 0x3FF);
    s * match e {
        0 => m * 2f64.powi(-24),
        31 => {
            if m == 0.0 {
                f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => (1.0 + m / 1024.0) * 2f64.powi(e - 15),
    }
}

// ------------------------------------------------------------------- GeoArrow geometry

/// The GeoArrow / GeoParquet geometry encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Wkb,
    Wkt,
    Point,
    LineString,
    Polygon,
    MultiPoint,
    MultiLineString,
    MultiPolygon,
}

impl Encoding {
    /// From a GeoParquet `encoding` or a GeoArrow extension name (without "geoarrow.").
    pub fn parse(s: &str) -> Option<Encoding> {
        Some(
            match s.to_ascii_lowercase().trim_start_matches("geoarrow.") {
                "wkb" | "ogc.wkb" => Encoding::Wkb,
                "wkt" | "ogc.wkt" => Encoding::Wkt,
                "point" => Encoding::Point,
                "linestring" => Encoding::LineString,
                "polygon" => Encoding::Polygon,
                "multipoint" => Encoding::MultiPoint,
                "multilinestring" => Encoding::MultiLineString,
                "multipolygon" => Encoding::MultiPolygon,
                _ => return None,
            },
        )
    }

    /// List levels above the coordinates.
    pub fn depth(self) -> usize {
        match self {
            Encoding::Wkb | Encoding::Wkt | Encoding::Point => 0,
            Encoding::LineString | Encoding::MultiPoint => 1,
            Encoding::Polygon | Encoding::MultiLineString => 2,
            Encoding::MultiPolygon => 3,
        }
    }
}

/// Native geometries as offsets per list level (outermost first; `levels[0]` has one more
/// entry than rows) over a flat coordinate array, with each row's validity.
#[derive(Clone, Debug, Default)]
pub struct Nested {
    pub levels: Vec<Vec<usize>>,
    pub coords: Vec<Pt>,
    pub row_valid: Vec<bool>,
}

impl Nested {
    /// Geometry of row `r`.
    pub fn geometry(&self, enc: Encoding, r: usize) -> Option<Geometry> {
        if !self.row_valid.get(r).copied().unwrap_or(false) {
            return None;
        }
        let span = |lv: usize, i: usize| -> std::ops::Range<usize> {
            let o = &self.levels[lv];
            let s = o.get(i).copied().unwrap_or(0);
            let e = o.get(i + 1).copied().unwrap_or(s).max(s);
            s..e
        };
        let pts = |rg: std::ops::Range<usize>| -> Vec<Pt> {
            self.coords
                .get(rg.start.min(self.coords.len())..rg.end.min(self.coords.len()))
                .unwrap_or(&[])
                .to_vec()
        };
        Some(match enc {
            Encoding::Point => {
                let p = *self.coords.get(r)?;
                if p[0].is_nan() && p[1].is_nan() {
                    return Some(Geometry::MultiPoint(Vec::new()));
                }
                Geometry::Point(p)
            }
            Encoding::LineString => Geometry::LineString(pts(span(0, r))),
            Encoding::MultiPoint => Geometry::MultiPoint(pts(span(0, r))),
            Encoding::Polygon => Geometry::Polygon(span(0, r).map(|k| pts(span(1, k))).collect()),
            Encoding::MultiLineString => {
                Geometry::MultiLineString(span(0, r).map(|k| pts(span(1, k))).collect())
            }
            Encoding::MultiPolygon => Geometry::MultiPolygon(
                span(0, r)
                    .map(|p| span(1, p).map(|k| pts(span(2, k))).collect())
                    .collect(),
            ),
            Encoding::Wkb | Encoding::Wkt => return None,
        })
    }
}

/// Coordinates of a GeoArrow point array: a struct of x, y (…) or a fixed-size list.
fn coords(a: &Arr, out: &mut Vec<Pt>) -> Result<(), String> {
    match a {
        Arr::Struct {
            children,
            len,
            valid,
        } => {
            let (x, y) = (
                children.first().ok_or("a point struct has no x")?,
                children.get(1).ok_or("a point struct has no y")?,
            );
            for i in 0..*len {
                let ok = valid
                    .as_ref()
                    .is_none_or(|v| v.get(i).copied().unwrap_or(false));
                out.push(if ok {
                    [x.f64(i).unwrap_or(f64::NAN), y.f64(i).unwrap_or(f64::NAN)]
                } else {
                    [f64::NAN; 2]
                });
            }
        }
        Arr::FixedList {
            size, len, child, ..
        } => {
            if *size < 2 {
                return Err("a coordinate list has fewer than two values".into());
            }
            for i in 0..*len {
                out.push([
                    child.f64(i * size).unwrap_or(f64::NAN),
                    child.f64(i * size + 1).unwrap_or(f64::NAN),
                ]);
            }
        }
        _ => return Err("GeoArrow coordinates are neither a struct nor a list".into()),
    }
    Ok(())
}

/// The native layout of a GeoArrow array of `depth` list levels.
pub fn nested(a: &Arr, depth: usize) -> Result<Nested, String> {
    let rows = a.len();
    let mut n = Nested {
        row_valid: (0..rows).map(|i| a.is_valid(i)).collect(),
        ..Nested::default()
    };
    let mut cur = a;
    for _ in 0..depth {
        match cur {
            Arr::List { offsets, child, .. } => {
                let base = offsets.first().copied().unwrap_or(0);
                n.levels.push(
                    offsets
                        .iter()
                        .map(|&o| usize::try_from(o - base).unwrap_or(0))
                        .collect(),
                );
                if base != 0 {
                    return Err("sliced GeoArrow lists are not supported".into());
                }
                cur = child;
            }
            _ => return Err("a GeoArrow geometry is not nested as its type says".into()),
        }
    }
    coords(cur, &mut n.coords)?;
    Ok(n)
}

/// Geometry of row `r` of a GeoArrow array (any encoding).
pub fn geometry(a: &Arr, enc: Encoding, nested: Option<&Nested>, r: usize) -> Option<Geometry> {
    match enc {
        Encoding::Wkb => {
            if !a.is_valid(r) {
                return None;
            }
            crate::gpkg::Wkb::new(a.bytes(r)?).geometry(0).ok()
        }
        Encoding::Wkt => {
            if !a.is_valid(r) {
                return None;
            }
            crate::wkt_geom::parse(std::str::from_utf8(a.bytes(r)?).ok()?).ok()
        }
        e => nested?.geometry(e, r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatbuf::build::{Builder, F};

    /// A schema whose field lists the same child twice, 32 levels down: 2^32 fields
    /// without the field budget.
    #[test]
    fn shared_children_bounded() {
        let mut b = Builder::default();
        b.buf.extend_from_slice(&[0; 4]);
        let (schema, offs) = b.table(&[None, Some(F::Off(0))]);
        b.patch(0, schema);
        let fields = b.offsets_n(1);
        b.patch(offs[0].0, fields);
        let mut slots = vec![fields + 4];
        for _ in 0..33 {
            let (t, offs) = b.table(&[None, None, Some(F::U8(13)), None, None, Some(F::Off(0))]);
            for s in slots {
                b.patch(s, t);
            }
            let kids = b.offsets_n(2);
            b.patch(offs[0].0, kids);
            slots = vec![kids + 4, kids + 8];
        }
        let leaf = b.table(&[None, None, Some(F::U8(6))]).0;
        for s in slots {
            b.patch(s, leaf);
        }
        fn count(f: &[Field]) -> usize {
            f.iter().map(|f| 1 + count(&f.children)).sum()
        }
        let s = schema_of(Table::root(&b.buf).unwrap());
        assert!(count(&s.fields) <= MAX_FIELDS);
    }

    /// A dictionary whose values are dictionary-encoded too: refused (each dictionary
    /// would hold the one before, and dropping a long chain overflows the stack).
    #[test]
    fn nested_dictionary_refused() {
        let message = |header_type: u8, build: &dyn Fn(&mut Builder) -> usize| {
            let mut b = Builder::default();
            b.buf.extend_from_slice(&[0; 4]);
            let (m, offs) = b.table(&[
                None,
                Some(F::U8(header_type)),
                Some(F::Off(0)),
                Some(F::U64(0)),
            ]);
            b.patch(0, m);
            let h = build(&mut b);
            b.patch(offs[0].0, h);
            while b.buf.len() % 8 != 0 {
                b.buf.push(0);
            }
            let mut out = (b.buf.len() as u32).to_le_bytes().to_vec();
            out.extend_from_slice(&b.buf);
            out
        };
        // Schema: one Utf8 field encoded with dictionary 0, whose child is encoded with 0 too.
        let mut data = message(1, &|b| {
            let (s, offs) = b.table(&[None, Some(F::Off(0))]);
            let fields = b.offsets_n(1);
            b.patch(offs[0].0, fields);
            let dict = |b: &mut Builder| b.table(&[Some(F::U64(0))]).0;
            let (f, offs) = b.table(&[
                None,
                None,
                Some(F::U8(12)),
                None,
                Some(F::Off(0)),
                Some(F::Off(0)),
            ]);
            b.patch(fields + 4, f);
            let d = dict(b);
            b.patch(offs[0].0, d);
            let kids = b.offsets_n(1);
            b.patch(offs[1].0, kids);
            let (c, offs) = b.table(&[None, None, Some(F::U8(5)), None, Some(F::Off(0))]);
            b.patch(kids + 4, c);
            let d = dict(b);
            b.patch(offs[0].0, d);
            s
        });
        for _ in 0..2 {
            data.extend(message(2, &|b| {
                let (d, offs) = b.table(&[Some(F::U64(0)), Some(F::Off(0))]);
                let rb = b.table(&[Some(F::U64(0))]).0;
                b.patch(offs[0].0, rb);
                d
            }));
        }
        let e = read_ipc(&data, 1 << 20).map(|_| ()).unwrap_err();
        assert!(e.contains("nested Arrow dictionaries"), "{e}");
    }

    /// A null bitmap of 1 byte for an array of 2^31 values: refused (8 bytes per value of
    /// `Vec<bool>` from a 1-byte buffer).
    #[test]
    fn short_validity_refused() {
        let body = [0u8; 8];
        let mut b = Body {
            body: &body,
            buffers: vec![(0, 1)],
            nodes: Vec::new(),
            next_buf: 0,
            next_node: 0,
            codec: None,
            limit: 1 << 20,
            variadic: Vec::new(),
            next_var: 0,
        };
        let e = b.validity(1 << 31, 1).unwrap_err();
        assert!(e.contains("too short"), "{e}");
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(date_text(0), "1970-01-01");
        assert_eq!(date_text(19_723), "2024-01-01");
        assert_eq!(date_text(-1), "1969-12-31");
        assert_eq!(
            timestamp_text(1_704_067_200_500, 1000),
            "2024-01-01T00:00:00.5"
        );
        assert_eq!(decimal(&[0x39, 0x30], 2, false), 123.45);
        assert_eq!(decimal(&[0xFF, 0xFF], 0, false), -1.0);
        assert_eq!(half(0x3C00), 1.0);
    }

    #[test]
    fn nested_polygons() {
        // Two polygon rows: one ring of 4, then a null row.
        let pts = Arr::FixedList {
            valid: None,
            size: 2,
            len: 4,
            child: Box::new(Arr::Prim {
                valid: None,
                data: [0.0f64, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0]
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect(),
                width: 8,
                len: 8,
            }),
        };
        let rings = Arr::List {
            valid: None,
            offsets: vec![0, 4],
            child: Box::new(pts),
        };
        let polys = Arr::List {
            valid: Some(vec![true, false]),
            offsets: vec![0, 1, 1],
            child: Box::new(rings),
        };
        let n = nested(&polys, 2).unwrap();
        assert_eq!(
            geometry(&polys, Encoding::Polygon, Some(&n), 0),
            Some(Geometry::Polygon(vec![vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 0.0]
            ]]))
        );
        assert_eq!(geometry(&polys, Encoding::Polygon, Some(&n), 1), None);
    }
}
