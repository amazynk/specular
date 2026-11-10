//! An Apache Parquet reader limited to what GeoParquet needs: the footer metadata (Thrift
//! compact protocol), the schema tree, and column chunks of every physical type through
//! data pages v1 and v2 with PLAIN, dictionary (PLAIN_DICTIONARY, RLE_DICTIONARY), RLE,
//! bit-packed, DELTA_BINARY_PACKED, DELTA_LENGTH_BYTE_ARRAY, DELTA_BYTE_ARRAY and
//! BYTE_STREAM_SPLIT encodings, compressed with Snappy, gzip, Zstandard, Brotli or LZ4.
//!
//! A column chunk decodes to its leaf values plus repetition and definition levels;
//! [`nest`] rebuilds list offsets from the levels for nested (GeoArrow) columns. LZO
//! is refused by name. Encryption is refused.

use crate::codec;

// ---------------------------------------------------------------- Thrift compact protocol

/// A Thrift value.
#[derive(Clone, Debug)]
pub enum T<'a> {
    Bool(bool),
    Int(i64),
    Double(f64),
    Bin(&'a [u8]),
    List(Vec<T<'a>>),
    Map(Vec<(T<'a>, T<'a>)>),
    Struct(Vec<(i16, T<'a>)>),
}

impl<'a> T<'a> {
    pub fn get(&self, id: i16) -> Option<&T<'a>> {
        match self {
            T::Struct(f) => f.iter().find(|(i, _)| *i == id).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn int(&self, id: i16) -> Option<i64> {
        match self.get(id)? {
            T::Int(v) => Some(*v),
            _ => None,
        }
    }
    pub fn bool(&self, id: i16) -> Option<bool> {
        match self.get(id)? {
            T::Bool(v) => Some(*v),
            _ => None,
        }
    }
    pub fn bin(&self, id: i16) -> Option<&'a [u8]> {
        match self.get(id)? {
            T::Bin(v) => Some(v),
            _ => None,
        }
    }
    pub fn str(&self, id: i16) -> Option<String> {
        self.bin(id)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }
    pub fn list(&self, id: i16) -> &[T<'a>] {
        match self.get(id) {
            Some(T::List(v)) => v,
            _ => &[],
        }
    }
    /// The one field of a union.
    pub fn union(&self) -> Option<(i16, &T<'a>)> {
        match self {
            T::Struct(f) => f.first().map(|(i, v)| (*i, v)),
            _ => None,
        }
    }
}

pub struct Thrift<'a> {
    b: &'a [u8],
    pub i: usize,
}

impl<'a> Thrift<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Thrift { b, i: 0 }
    }

    fn byte(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.i).ok_or("Parquet metadata is truncated")?;
        self.i += 1;
        Ok(v)
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..70).step_by(7) {
            let b = self.byte()?;
            if shift < 64 {
                v |= u64::from(b & 0x7F) << shift;
            }
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("Parquet metadata has a malformed number".into())
    }

    fn zigzag(&mut self) -> Result<i64, String> {
        let v = self.varint()?;
        Ok((v >> 1) as i64 ^ -((v & 1) as i64))
    }

    fn value(&mut self, ty: u8, depth: usize) -> Result<T<'a>, String> {
        if depth > 32 {
            return Err("Parquet metadata nests too deep".into());
        }
        Ok(match ty {
            1 => T::Bool(true),
            2 => T::Bool(false),
            3 => T::Int(i64::from(self.byte()? as i8)),
            4..=6 => T::Int(self.zigzag()?),
            7 => {
                let s = self.b.get(self.i..self.i + 8).ok_or("truncated")?;
                self.i += 8;
                T::Double(f64::from_le_bytes(s.try_into().unwrap_or_default()))
            }
            8 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "bad length")?;
                let s = self
                    .b
                    .get(self.i..self.i.checked_add(n).ok_or("bad length")?)
                    .ok_or("Parquet metadata is truncated")?;
                self.i += n;
                T::Bin(s)
            }
            9 | 10 => {
                let h = self.byte()?;
                let mut n = usize::from(h >> 4);
                if n == 15 {
                    n = usize::try_from(self.varint()?).map_err(|_| "bad length")?;
                }
                let et = h & 15;
                if n > self.b.len() {
                    return Err("Parquet metadata list is too long".into());
                }
                let mut v = Vec::with_capacity(n.min(1 << 16));
                for _ in 0..n {
                    v.push(if et == 1 || et == 2 {
                        T::Bool(self.byte()? == 1)
                    } else {
                        self.value(et, depth + 1)?
                    });
                }
                T::List(v)
            }
            11 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "bad length")?;
                if n > self.b.len() {
                    return Err("Parquet metadata map is too long".into());
                }
                let mut v = Vec::new();
                if n > 0 {
                    let kv = self.byte()?;
                    for _ in 0..n {
                        let k = self.value(kv >> 4, depth + 1)?;
                        let x = self.value(kv & 15, depth + 1)?;
                        v.push((k, x));
                    }
                }
                T::Map(v)
            }
            12 => self.structure(depth + 1)?,
            t => return Err(format!("Parquet metadata has an unknown type {t}")),
        })
    }

    pub fn structure(&mut self, depth: usize) -> Result<T<'a>, String> {
        let mut fields = Vec::new();
        let mut last = 0i16;
        loop {
            let h = self.byte()?;
            if h == 0 {
                break;
            }
            let ty = h & 15;
            let id = if h >> 4 == 0 {
                i16::try_from(self.zigzag()?).map_err(|_| "bad field id")?
            } else {
                last.wrapping_add(i16::from(h >> 4))
            };
            last = id;
            fields.push((id, self.value(ty, depth)?));
        }
        Ok(T::Struct(fields))
    }
}

// -------------------------------------------------------------------------- metadata

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Physical {
    Boolean,
    Int32,
    Int64,
    Int96,
    Float,
    Double,
    ByteArray,
    FixedLen(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repetition {
    Required,
    Optional,
    Repeated,
}

/// What a leaf's bytes mean (the converted or logical type, simplified).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logical {
    None,
    String,
    Json,
    /// Days since 1970-01-01.
    Date,
    /// Timestamps in units per second (1000, 1_000_000, 1_000_000_000).
    Timestamp(u32),
    /// Time of day in units per second.
    Time(u32),
    Decimal(i32),
    Unsigned,
}

#[derive(Clone, Debug)]
pub struct SchemaNode {
    pub name: String,
    pub repetition: Repetition,
    pub physical: Option<Physical>,
    pub logical: Logical,
    pub children: Vec<usize>,
}

/// A leaf column with its path and level maxima.
#[derive(Clone, Debug)]
pub struct Leaf {
    pub path: Vec<String>,
    /// Schema node indices from the root's child down to the leaf.
    pub nodes: Vec<usize>,
    pub physical: Physical,
    pub logical: Logical,
    pub max_def: u16,
    pub max_rep: u16,
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub codec: i64,
    pub num_values: i64,
    pub data_page: u64,
    pub dict_page: Option<u64>,
    pub compressed: u64,
}

#[derive(Clone, Debug)]
pub struct RowGroup {
    pub rows: u64,
    /// One per leaf, in leaf order.
    pub chunks: Vec<Option<Chunk>>,
}

pub struct File<'a> {
    pub data: &'a [u8],
    pub nodes: Vec<SchemaNode>,
    pub leaves: Vec<Leaf>,
    pub rows: u64,
    pub row_groups: Vec<RowGroup>,
    pub kv: Vec<(String, String)>,
    pub created_by: Option<String>,
}

pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(b"PAR1")
}

fn physical(t: i64, len: i64) -> Option<Physical> {
    Some(match t {
        0 => Physical::Boolean,
        1 => Physical::Int32,
        2 => Physical::Int64,
        3 => Physical::Int96,
        4 => Physical::Float,
        5 => Physical::Double,
        6 => Physical::ByteArray,
        7 => Physical::FixedLen(usize::try_from(len).ok().filter(|&n| n > 0)?),
        _ => return None,
    })
}

fn logical(e: &T<'_>) -> Logical {
    if let Some(l @ T::Struct(_)) = e.get(10)
        && let Some((id, v)) = l.union()
    {
        let unit = |v: &T<'_>| match v.get(2).and_then(T::union).map(|(u, _)| u) {
            Some(1) => 1000,
            Some(2) => 1_000_000,
            Some(3) => 1_000_000_000,
            _ => 1_000_000,
        };
        return match id {
            1 | 4 => Logical::String, // STRING, ENUM
            12 => Logical::Json,
            6 => Logical::Date,
            8 => Logical::Timestamp(unit(v)),
            7 => Logical::Time(unit(v)),
            5 => Logical::Decimal(v.int(1).unwrap_or(0) as i32),
            10 => {
                if v.bool(2) == Some(false) {
                    Logical::Unsigned
                } else {
                    Logical::None
                }
            }
            _ => Logical::None,
        };
    }
    match e.int(6) {
        Some(0 | 16) => Logical::String, // UTF8, ENUM
        Some(19) => Logical::Json,
        Some(6) => Logical::Date,
        Some(9) => Logical::Timestamp(1000),
        Some(10) => Logical::Timestamp(1_000_000),
        Some(7) => Logical::Time(1000),
        Some(8) => Logical::Time(1_000_000),
        Some(5) => Logical::Decimal(e.int(7).unwrap_or(0) as i32),
        Some(11..=14) => Logical::Unsigned,
        _ => Logical::None,
    }
}

impl<'a> File<'a> {
    pub fn open(data: &'a [u8]) -> Result<File<'a>, String> {
        if data.len() < 12 || !sniff(data) {
            return Err("not a Parquet file".into());
        }
        let tail = &data[data.len() - 8..];
        if &tail[4..] == b"PARE" {
            return Err("encrypted Parquet files are not supported".into());
        }
        if &tail[4..] != b"PAR1" {
            return Err("the Parquet footer is missing (a truncated file?)".into());
        }
        let n = u32::from_le_bytes(tail[..4].try_into().unwrap_or_default()) as usize;
        let start = (data.len() - 8)
            .checked_sub(n)
            .filter(|&s| s >= 4)
            .ok_or("the Parquet footer size is wrong")?;
        let meta = Thrift::new(&data[start..data.len() - 8]).structure(0)?;
        let schema = meta.list(2);
        if schema.is_empty() {
            return Err("the Parquet schema is empty".into());
        }
        let mut nodes: Vec<SchemaNode> = Vec::with_capacity(schema.len());
        let mut counts: Vec<usize> = Vec::with_capacity(schema.len());
        for e in schema {
            let rep = match e.int(3) {
                Some(1) => Repetition::Optional,
                Some(2) => Repetition::Repeated,
                _ => Repetition::Required,
            };
            let kids = e.int(5).unwrap_or(0).max(0) as usize;
            nodes.push(SchemaNode {
                name: e.str(4).unwrap_or_default(),
                repetition: rep,
                physical: if kids == 0 {
                    e.int(1).and_then(|t| physical(t, e.int(2).unwrap_or(0)))
                } else {
                    None
                },
                logical: logical(e),
                children: Vec::new(),
            });
            counts.push(kids);
        }
        // Children follow their parent depth-first.
        let mut next = 1usize;
        fn link(
            i: usize,
            next: &mut usize,
            nodes: &mut Vec<SchemaNode>,
            counts: &[usize],
            depth: usize,
        ) -> Result<(), String> {
            if depth > 64 {
                return Err("the Parquet schema nests too deep".into());
            }
            for _ in 0..counts[i] {
                let c = *next;
                if c >= nodes.len() {
                    return Err("the Parquet schema is inconsistent".into());
                }
                *next += 1;
                nodes[i].children.push(c);
                link(c, next, nodes, counts, depth + 1)?;
            }
            Ok(())
        }
        link(0, &mut next, &mut nodes, &counts, 0)?;
        let mut leaves = Vec::new();
        fn walk(nodes: &[SchemaNode], i: usize, path: &mut Vec<usize>, out: &mut Vec<Leaf>) {
            path.push(i);
            if nodes[i].children.is_empty() {
                if let Some(p) = nodes[i].physical {
                    let (mut d, mut r) = (0u16, 0u16);
                    for &n in path.iter() {
                        match nodes[n].repetition {
                            Repetition::Optional => d += 1,
                            Repetition::Repeated => {
                                d += 1;
                                r += 1;
                            }
                            Repetition::Required => {}
                        }
                    }
                    out.push(Leaf {
                        path: path.iter().map(|&n| nodes[n].name.clone()).collect(),
                        nodes: path.clone(),
                        physical: p,
                        logical: nodes[i].logical,
                        max_def: d,
                        max_rep: r,
                    });
                }
            } else {
                for &c in &nodes[i].children {
                    walk(nodes, c, path, out);
                }
            }
            path.pop();
        }
        for &c in &nodes[0].children.clone() {
            walk(&nodes, c, &mut Vec::new(), &mut leaves);
        }
        let mut row_groups = Vec::new();
        for g in meta.list(4) {
            let mut chunks = Vec::new();
            for c in g.list(1) {
                if c.str(1).is_some() {
                    chunks.push(None); // in another file
                    continue;
                }
                let Some(m) = c.get(3) else {
                    chunks.push(None);
                    continue;
                };
                chunks.push(Some(Chunk {
                    codec: m.int(4).unwrap_or(0),
                    num_values: m.int(5).unwrap_or(0),
                    data_page: m.int(9).unwrap_or(0).max(0) as u64,
                    dict_page: m.int(11).filter(|&v| v > 0).map(|v| v as u64),
                    compressed: m.int(7).unwrap_or(0).max(0) as u64,
                }));
            }
            row_groups.push(RowGroup {
                rows: g.int(3).unwrap_or(0).max(0) as u64,
                chunks,
            });
        }
        let kv = meta
            .list(5)
            .iter()
            .filter_map(|e| Some((e.str(1)?, e.str(2).unwrap_or_default())))
            .collect();
        Ok(File {
            data,
            nodes,
            leaves,
            rows: meta.int(3).unwrap_or(0).max(0) as u64,
            row_groups,
            kv,
            created_by: meta.str(6),
        })
    }

    pub fn meta(&self, key: &str) -> Option<&str> {
        self.kv
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The leaves under the top-level column `name`, in order.
    pub fn leaves_of(&self, name: &str) -> Vec<usize> {
        (0..self.leaves.len())
            .filter(|&i| self.leaves[i].path.first().is_some_and(|p| p == name))
            .collect()
    }

    /// Reads leaf `leaf` of row group `g`.
    pub fn column(&self, g: usize, leaf: usize, limit: usize) -> Result<Column, String> {
        let l = self.leaves.get(leaf).ok_or("no such column")?;
        let c = self
            .row_groups
            .get(g)
            .and_then(|r| r.chunks.get(leaf))
            .and_then(Option::as_ref)
            .ok_or("the column chunk is missing or in another file")?;
        read_chunk(self.data, l, c, limit)
    }
}

// ------------------------------------------------------------------------- values

/// Leaf values of one column chunk (only the defined ones), and the levels.
#[derive(Clone, Debug, Default)]
pub struct Column {
    pub values: Values,
    /// One per entry (empty when the max definition level is 0: all defined).
    pub def: Vec<u16>,
    /// One per entry (empty when the max repetition level is 0).
    pub rep: Vec<u16>,
    /// Entries (values plus nulls and empty lists).
    pub entries: usize,
}

#[derive(Clone, Debug)]
pub enum Values {
    Bool(Vec<bool>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    /// Nanoseconds of day and Julian day.
    I96(Vec<(u64, u32)>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    /// Byte strings: all bytes and the end of each.
    Bytes(Vec<u8>, Vec<usize>),
}

impl Default for Values {
    fn default() -> Self {
        Values::I32(Vec::new())
    }
}

impl Values {
    pub fn len(&self) -> usize {
        match self {
            Values::Bool(v) => v.len(),
            Values::I32(v) => v.len(),
            Values::I64(v) => v.len(),
            Values::I96(v) => v.len(),
            Values::F32(v) => v.len(),
            Values::F64(v) => v.len(),
            Values::Bytes(_, e) => e.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bytes(&self, i: usize) -> Option<&[u8]> {
        match self {
            Values::Bytes(b, e) => {
                let end = *e.get(i)?;
                let start = if i == 0 { 0 } else { e[i - 1] };
                b.get(start..end)
            }
            _ => None,
        }
    }

    /// Value `i` as a float (numbers only).
    pub fn f64(&self, i: usize) -> Option<f64> {
        Some(match self {
            Values::F64(v) => *v.get(i)?,
            Values::F32(v) => f64::from(*v.get(i)?),
            Values::I32(v) => f64::from(*v.get(i)?),
            Values::I64(v) => *v.get(i)? as f64,
            _ => return None,
        })
    }

    fn empty_like(p: Physical) -> Values {
        match p {
            Physical::Boolean => Values::Bool(Vec::new()),
            Physical::Int32 => Values::I32(Vec::new()),
            Physical::Int64 => Values::I64(Vec::new()),
            Physical::Int96 => Values::I96(Vec::new()),
            Physical::Float => Values::F32(Vec::new()),
            Physical::Double => Values::F64(Vec::new()),
            Physical::ByteArray | Physical::FixedLen(_) => Values::Bytes(Vec::new(), Vec::new()),
        }
    }

    fn push_from(&mut self, other: &Values, i: usize) -> Result<(), String> {
        match (self, other) {
            (Values::Bool(a), Values::Bool(b)) => a.push(*b.get(i).ok_or(BAD_INDEX)?),
            (Values::I32(a), Values::I32(b)) => a.push(*b.get(i).ok_or(BAD_INDEX)?),
            (Values::I64(a), Values::I64(b)) => a.push(*b.get(i).ok_or(BAD_INDEX)?),
            (Values::I96(a), Values::I96(b)) => a.push(*b.get(i).ok_or(BAD_INDEX)?),
            (Values::F32(a), Values::F32(b)) => a.push(*b.get(i).ok_or(BAD_INDEX)?),
            (Values::F64(a), Values::F64(b)) => a.push(*b.get(i).ok_or(BAD_INDEX)?),
            (Values::Bytes(a, e), src @ Values::Bytes(..)) => {
                a.extend_from_slice(src.bytes(i).ok_or(BAD_INDEX)?);
                e.push(a.len());
            }
            _ => return Err("dictionary type mismatch".into()),
        }
        Ok(())
    }

    fn append(&mut self, other: Values) -> Result<(), String> {
        match (self, other) {
            (Values::Bool(a), Values::Bool(b)) => a.extend(b),
            (Values::I32(a), Values::I32(b)) => a.extend(b),
            (Values::I64(a), Values::I64(b)) => a.extend(b),
            (Values::I96(a), Values::I96(b)) => a.extend(b),
            (Values::F32(a), Values::F32(b)) => a.extend(b),
            (Values::F64(a), Values::F64(b)) => a.extend(b),
            (Values::Bytes(a, e), Values::Bytes(b, f)) => {
                let base = a.len();
                a.extend_from_slice(&b);
                e.extend(f.into_iter().map(|x| x + base));
            }
            _ => return Err("page type mismatch".into()),
        }
        Ok(())
    }
}

const BAD_INDEX: &str = "a dictionary index is out of range";

/// The RLE / bit-packing hybrid: `n` values of `width` bits.
fn rle_hybrid(b: &[u8], width: u32, n: usize, out: &mut Vec<u32>) -> Result<(), String> {
    if width > 32 {
        return Err("bad bit width".into());
    }
    let mut i = 0usize;
    let target = out.len() + n;
    let bytes_w = width.div_ceil(8) as usize;
    while out.len() < target {
        let mut h = 0u64;
        let mut shift = 0;
        loop {
            let x = *b.get(i).ok_or("level or index data is truncated")?;
            i += 1;
            h |= u64::from(x & 0x7F) << shift;
            if x & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 56 {
                return Err("bad run header".into());
            }
        }
        if h & 1 == 0 {
            let count = (h >> 1) as usize;
            let v = b.get(i..i + bytes_w).ok_or("run is truncated")?;
            i += bytes_w;
            let mut x = 0u32;
            for (k, &byte) in v.iter().enumerate() {
                x |= u32::from(byte) << (8 * k);
            }
            let take = count.min(target - out.len());
            out.extend(std::iter::repeat_n(x, take));
        } else {
            let groups = (h >> 1) as usize;
            let nbytes = groups
                .checked_mul(width as usize)
                .ok_or("bad bit-packed run")?;
            let data = b.get(i..i + nbytes).ok_or("bit-packed run is truncated")?;
            i += nbytes;
            let total = groups * 8;
            let mut bit = 0usize;
            for _ in 0..total {
                if out.len() >= target {
                    break;
                }
                let mut v = 0u64;
                for k in 0..width as usize {
                    let p = bit + k;
                    if data[p / 8] >> (p % 8) & 1 != 0 {
                        v |= 1 << k;
                    }
                }
                bit += width as usize;
                out.push(v as u32);
            }
        }
    }
    Ok(())
}

fn bit_width(max: u16) -> u32 {
    16 - max.leading_zeros()
}

fn plain(b: &[u8], p: Physical, n: usize) -> Result<Values, String> {
    let need = |size: usize| -> Result<&[u8], String> {
        b.get(..n.checked_mul(size).ok_or("too many values")?)
            .ok_or_else(|| "a page's values are truncated".to_string())
    };
    Ok(match p {
        Physical::Boolean => {
            let d = b
                .get(..n.div_ceil(8))
                .ok_or("a page's values are truncated")?;
            Values::Bool((0..n).map(|k| d[k / 8] >> (k % 8) & 1 != 0).collect())
        }
        Physical::Int32 => Values::I32(
            need(4)?
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes(c.try_into().unwrap_or_default()))
                .collect(),
        ),
        Physical::Int64 => Values::I64(
            need(8)?
                .chunks_exact(8)
                .map(|c| i64::from_le_bytes(c.try_into().unwrap_or_default()))
                .collect(),
        ),
        Physical::Int96 => Values::I96(
            need(12)?
                .chunks_exact(12)
                .map(|c| {
                    (
                        u64::from_le_bytes(c[..8].try_into().unwrap_or_default()),
                        u32::from_le_bytes(c[8..].try_into().unwrap_or_default()),
                    )
                })
                .collect(),
        ),
        Physical::Float => Values::F32(
            need(4)?
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap_or_default()))
                .collect(),
        ),
        Physical::Double => Values::F64(
            need(8)?
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or_default()))
                .collect(),
        ),
        Physical::FixedLen(w) => {
            let d = need(w)?;
            Values::Bytes(d.to_vec(), (1..=n).map(|k| k * w).collect())
        }
        Physical::ByteArray => {
            let mut out = Vec::new();
            let mut ends = Vec::with_capacity(n.min(1 << 20));
            let mut i = 0usize;
            for _ in 0..n {
                let l = b.get(i..i + 4).ok_or("a page's values are truncated")?;
                let l = u32::from_le_bytes(l.try_into().unwrap_or_default()) as usize;
                i += 4;
                let s = b
                    .get(i..i.checked_add(l).ok_or("bad length")?)
                    .ok_or("a page's values are truncated")?;
                out.extend_from_slice(s);
                ends.push(out.len());
                i += l;
            }
            Values::Bytes(out, ends)
        }
    })
}

/// DELTA_BINARY_PACKED: `n` values (or all in the block header when `n` is None), and
/// the bytes consumed.
fn delta_binary(b: &[u8], n: Option<usize>) -> Result<(Vec<i64>, usize), String> {
    let mut i = 0usize;
    let read_uv = |i: &mut usize| -> Result<u64, String> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let x = *b.get(*i).ok_or("delta data is truncated")?;
            *i += 1;
            if shift < 64 {
                v |= u64::from(x & 0x7F) << shift;
            }
            if x & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 70 {
                return Err("bad delta varint".into());
            }
        }
    };
    let block = read_uv(&mut i)? as usize;
    let minis = read_uv(&mut i)? as usize;
    let total = read_uv(&mut i)? as usize;
    let zz = |v: u64| (v >> 1) as i64 ^ -((v & 1) as i64);
    let first = zz(read_uv(&mut i)?);
    let want = n.unwrap_or(total).min(total);
    if minis == 0 || block == 0 || !block.is_multiple_of(minis) || want > b.len() * 64 + 1 {
        return Err("bad delta header".into());
    }
    let per_mini = block / minis;
    let mut out = Vec::with_capacity(want);
    if want > 0 {
        out.push(first);
    }
    let mut last = first;
    while out.len() < total.max(want) && out.len() < want {
        let min_delta = zz(read_uv(&mut i)?);
        let widths = b
            .get(i..i + minis)
            .ok_or("delta data is truncated")?
            .to_vec();
        i += minis;
        for &w in &widths {
            if out.len() >= want {
                break;
            }
            let w = usize::from(w);
            if w > 64 {
                return Err("bad delta bit width".into());
            }
            let nbytes = per_mini * w / 8;
            let d = b.get(i..i + nbytes).ok_or("delta data is truncated")?;
            i += nbytes;
            for k in 0..per_mini {
                if out.len() >= want {
                    break;
                }
                let mut v = 0u64;
                for bit in 0..w {
                    let p = k * w + bit;
                    if d[p / 8] >> (p % 8) & 1 != 0 {
                        v |= 1 << bit;
                    }
                }
                last = last.wrapping_add(min_delta).wrapping_add(v as i64);
                out.push(last);
            }
        }
    }
    // Skip any remaining miniblocks of the last block (their bytes are present).
    Ok((out, i))
}

fn decode_values(
    enc: i64,
    b: &[u8],
    p: Physical,
    n: usize,
    dict: Option<&Values>,
) -> Result<Values, String> {
    match enc {
        0 => plain(b, p, n),
        2 | 8 => {
            let dict = dict.ok_or("a dictionary page is missing")?;
            let w = u32::from(*b.first().ok_or("index data is truncated")?);
            let mut idx = Vec::with_capacity(n);
            if n > 0 {
                rle_hybrid(&b[1..], w, n, &mut idx)?;
            }
            let mut out = Values::empty_like(p);
            for k in idx {
                out.push_from(dict, k as usize)?;
            }
            Ok(out)
        }
        3 if p == Physical::Boolean => {
            let l = b.get(..4).ok_or("truncated")?;
            let l = u32::from_le_bytes(l.try_into().unwrap_or_default()) as usize;
            let mut v = Vec::new();
            rle_hybrid(b.get(4..4 + l).ok_or("truncated")?, 1, n, &mut v)?;
            Ok(Values::Bool(v.into_iter().map(|x| x != 0).collect()))
        }
        5 => {
            let (v, _) = delta_binary(b, Some(n))?;
            Ok(match p {
                Physical::Int32 => Values::I32(v.into_iter().map(|x| x as i32).collect()),
                Physical::Int64 => Values::I64(v),
                _ => return Err("delta encoding on a non-integer column".into()),
            })
        }
        6 => {
            let (lens, used) = delta_binary(b, Some(n))?;
            let mut out = Vec::new();
            let mut ends = Vec::with_capacity(n);
            let mut i = used;
            for l in lens {
                let l = usize::try_from(l).map_err(|_| "bad length")?;
                let s = b
                    .get(i..i.checked_add(l).ok_or("bad length")?)
                    .ok_or("truncated")?;
                out.extend_from_slice(s);
                ends.push(out.len());
                i += l;
            }
            Ok(Values::Bytes(out, ends))
        }
        7 => {
            let (pre, used) = delta_binary(b, Some(n))?;
            let (suf, used2) = delta_binary(&b[used..], Some(n))?;
            let mut i = used + used2;
            let mut out: Vec<u8> = Vec::new();
            let mut ends = Vec::with_capacity(n);
            let mut prev: Vec<u8> = Vec::new();
            for (p, s) in pre.into_iter().zip(suf) {
                let p = usize::try_from(p).map_err(|_| "bad prefix")?;
                let s = usize::try_from(s).map_err(|_| "bad suffix")?;
                let mut v = prev.get(..p).ok_or("bad prefix")?.to_vec();
                v.extend_from_slice(
                    b.get(i..i.checked_add(s).ok_or("bad")?)
                        .ok_or("truncated")?,
                );
                i += s;
                out.extend_from_slice(&v);
                ends.push(out.len());
                prev = v;
            }
            Ok(Values::Bytes(out, ends))
        }
        9 => {
            let w = match p {
                Physical::Float | Physical::Int32 => 4,
                Physical::Double | Physical::Int64 => 8,
                Physical::FixedLen(w) => w,
                _ => return Err("byte stream split on an unsupported type".into()),
            };
            let d = b.get(..n * w).ok_or("truncated")?;
            let mut joined = vec![0u8; n * w];
            for k in 0..n {
                for s in 0..w {
                    joined[k * w + s] = d[s * n + k];
                }
            }
            plain(&joined, p, n)
        }
        1 => Err("the deprecated Parquet bit-packed value encoding is not supported".into()),
        e => Err(format!("Parquet value encoding {e} is not supported")),
    }
}

fn decompress(codec: i64, b: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let out = match codec {
        0 => b.to_vec(),
        1 => codec::snappy(b, size)?,
        2 => codec::inflate(b, size)?,
        3 => return Err("LZO-compressed Parquet columns are not supported".into()),
        4 => specular_brotli::decompress(b, size)
            .map_err(|e| format!("Brotli data is damaged: {e:?}"))?,
        5 => codec::lz4_parquet(b, size, false)?,
        6 => codec::zstd(b, size)?,
        7 => codec::lz4_parquet(b, size, true)?,
        c => return Err(format!("Parquet compression codec {c} is not supported")),
    };
    if out.len() != size {
        return Err("a page decompresses to the wrong size".into());
    }
    Ok(out)
}

/// Most values one column chunk may hold (at 8 to 12 bytes each in memory).
const MAX_CHUNK_VALUES: usize = 1 << 27;

fn read_chunk(data: &[u8], leaf: &Leaf, c: &Chunk, limit: usize) -> Result<Column, String> {
    let start = c.dict_page.unwrap_or(c.data_page).min(c.data_page) as usize;
    let mut i = start;
    let end = start.saturating_add(c.compressed as usize).min(data.len());
    let mut col = Column {
        values: Values::empty_like(leaf.physical),
        ..Column::default()
    };
    let mut dict: Option<Values> = None;
    let total = usize::try_from(c.num_values.max(0)).unwrap_or(0);
    if total > MAX_CHUNK_VALUES {
        return Err("a Parquet column has too many values".into());
    }
    // What the chunk's pages may decompress to, together.
    let mut inflate_left = limit;
    let mut seen = 0usize;
    let (dw, rw) = (bit_width(leaf.max_def), bit_width(leaf.max_rep));
    while seen < total && i < end {
        let mut t = Thrift::new(&data[i..end]);
        let h = t.structure(0)?;
        i += t.i;
        let usize_of = |v: Option<i64>| -> Result<usize, String> {
            usize::try_from(v.unwrap_or(-1)).map_err(|_| "bad page size".to_string())
        };
        let csize = usize_of(h.int(3))?;
        let usize_ = usize_of(h.int(2))?;
        if usize_ > limit {
            return Err("a Parquet page is too large".into());
        }
        inflate_left = inflate_left
            .checked_sub(usize_)
            .ok_or("a Parquet column is too large")?;
        let body = data
            .get(i..i.checked_add(csize).ok_or("bad page")?)
            .ok_or("a Parquet page is truncated")?;
        i += csize;
        match h.int(1) {
            Some(2) => {
                let dh = h.get(7).ok_or("dictionary page header missing")?;
                let n = usize_of(dh.int(1))?;
                let raw = decompress(c.codec, body, usize_)?;
                dict = Some(plain(&raw, leaf.physical, n)?);
            }
            Some(0) => {
                let dh = h.get(5).ok_or("data page header missing")?;
                // One run of a level or dictionary-index encoding fills any number of
                // values: a page may hold no more than the chunk has left.
                let n = usize_of(dh.int(1))?;
                if n > total - seen {
                    return Err("a Parquet page has more values than its column".into());
                }
                let raw = decompress(c.codec, body, usize_)?;
                let mut o = 0usize;
                let levels = |o: &mut usize, w: u32, out: &mut Vec<u16>| -> Result<(), String> {
                    let l = raw.get(*o..*o + 4).ok_or("levels are truncated")?;
                    let l = u32::from_le_bytes(l.try_into().unwrap_or_default()) as usize;
                    *o += 4;
                    let mut v = Vec::with_capacity(n);
                    rle_hybrid(
                        raw.get(*o..o.checked_add(l).ok_or("bad")?)
                            .ok_or("levels are truncated")?,
                        w,
                        n,
                        &mut v,
                    )?;
                    *o += l;
                    out.extend(v.into_iter().map(|x| x as u16));
                    Ok(())
                };
                let before_def = col.def.len();
                if leaf.max_rep > 0 {
                    levels(&mut o, rw, &mut col.rep)?;
                }
                if leaf.max_def > 0 {
                    levels(&mut o, dw, &mut col.def)?;
                }
                let present = if leaf.max_def > 0 {
                    col.def[before_def..]
                        .iter()
                        .filter(|&&d| d == leaf.max_def)
                        .count()
                } else {
                    n
                };
                let v = decode_values(
                    dh.int(2).unwrap_or(0),
                    &raw[o..],
                    leaf.physical,
                    present,
                    dict.as_ref(),
                )?;
                col.values.append(v)?;
                col.entries += n;
                seen += n;
            }
            Some(3) => {
                let dh = h.get(8).ok_or("data page header missing")?;
                let n = usize_of(dh.int(1))?;
                if n > total - seen {
                    return Err("a Parquet page has more values than its column".into());
                }
                let dlen = usize_of(dh.int(5))?;
                let rlen = usize_of(dh.int(6))?;
                let rl = body.get(..rlen).ok_or("levels are truncated")?;
                let dl = body.get(rlen..rlen + dlen).ok_or("levels are truncated")?;
                let mut v = Vec::new();
                if leaf.max_rep > 0 {
                    rle_hybrid(rl, rw, n, &mut v)?;
                    col.rep.extend(v.drain(..).map(|x| x as u16));
                }
                let before_def = col.def.len();
                if leaf.max_def > 0 {
                    rle_hybrid(dl, dw, n, &mut v)?;
                    col.def.extend(v.drain(..).map(|x| x as u16));
                }
                let rest = &body[rlen + dlen..];
                let compressed = dh.bool(7).unwrap_or(true);
                let raw = if compressed && c.codec != 0 {
                    decompress(c.codec, rest, usize_.saturating_sub(rlen + dlen))?
                } else {
                    rest.to_vec()
                };
                let present = if leaf.max_def > 0 {
                    col.def[before_def..]
                        .iter()
                        .filter(|&&d| d == leaf.max_def)
                        .count()
                } else {
                    n
                };
                let v = decode_values(
                    dh.int(4).unwrap_or(0),
                    &raw,
                    leaf.physical,
                    present,
                    dict.as_ref(),
                )?;
                col.values.append(v)?;
                col.entries += n;
                seen += n;
            }
            _ => {} // index pages and unknown pages are skipped
        }
    }
    if seen < total {
        return Err("a Parquet column chunk is truncated".into());
    }
    Ok(col)
}

/// Rebuilt nesting of one leaf column: for each repeated level (outermost first) the
/// length of every list, and for each leaf slot whether it holds a value. Lists at level
/// `k` are one per element of level `k − 1` (level 0: rows), null and empty lists
/// included (as length 0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nesting {
    pub rows: usize,
    pub lists: Vec<Vec<usize>>,
    /// Per leaf slot: whether a value is there (else null).
    pub present: Vec<bool>,
    /// Per row: whether the outermost optional field is null.
    pub row_null: Vec<bool>,
}

/// The nesting of a leaf from its levels. `nodes` is the leaf's schema path.
pub fn nest(f: &File<'_>, leaf: &Leaf, col: &Column) -> Result<Nesting, String> {
    // The definition level at each repeated node (the level at which that list has an
    // element), outermost first.
    let mut d = 0u16;
    let mut rep_def: Vec<u16> = Vec::new();
    // The definition level at which the row's top-level value is not null.
    let mut top_def: Option<u16> = None;
    for &n in &leaf.nodes {
        match f.nodes[n].repetition {
            Repetition::Optional => {
                d += 1;
                if top_def.is_none() && rep_def.is_empty() {
                    top_def = Some(d);
                }
            }
            Repetition::Repeated => {
                d += 1;
                rep_def.push(d);
            }
            Repetition::Required => {}
        }
    }
    let levels = rep_def.len();
    let mut out = Nesting {
        lists: vec![Vec::new(); levels],
        ..Nesting::default()
    };
    let n = col.entries;
    for e in 0..n {
        let r = if leaf.max_rep > 0 {
            *col.rep.get(e).ok_or("levels are inconsistent")? as usize
        } else {
            0
        };
        let dv = if leaf.max_def > 0 {
            *col.def.get(e).ok_or("levels are inconsistent")?
        } else {
            0
        };
        if r > levels {
            return Err("a repetition level is out of range".into());
        }
        if r == 0 {
            out.rows += 1;
            out.row_null.push(top_def.is_some_and(|t| dv < t));
        } else if out.rows == 0 {
            return Err("the first entry continues a list".into());
        }
        // Level k (1-based) gets a new element when the entry repeats at k or opens it.
        let mut exists = true; // the parent element of level k exists
        for k in 1..=levels {
            if k > r {
                if !exists {
                    break;
                }
                out.lists[k - 1].push(0);
            }
            if k >= r && k >= 1 {
                if dv >= rep_def[k - 1] {
                    if let Some(l) = out.lists[k - 1].last_mut() {
                        *l += 1;
                    }
                } else {
                    exists = false;
                    continue;
                }
            }
        }
        if exists && (levels == 0 || dv >= rep_def[levels - 1]) {
            out.present.push(dv == leaf.max_def);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle_and_bitpacked_runs() {
        // A run of 5 × 3 (width 2), then one bit-packed group of 8 values 0..7 (width 3).
        let mut b = vec![5 << 1, 3];
        b.push((1 << 1) | 1);
        // Values 0..8 packed LSB first in 3 bits: 0b...
        let mut bits = 0u32;
        for (k, v) in (0u32..8).enumerate() {
            bits |= v << (3 * k);
        }
        b.extend_from_slice(&bits.to_le_bytes()[..3]);
        let mut out = Vec::new();
        rle_hybrid(&b[..2], 2, 5, &mut out).unwrap();
        assert_eq!(out, vec![3; 5]);
        out.clear();
        rle_hybrid(&b[2..], 3, 8, &mut out).unwrap();
        assert_eq!(out, (0..8).collect::<Vec<_>>());
        assert!(rle_hybrid(&b[..1], 2, 5, &mut Vec::new()).is_err());
    }

    #[test]
    fn delta_binary_packed() {
        // Block 128, 4 miniblocks, 5 values: 7, 5, 3, 1, 2 (deltas -2, -2, -2, 1).
        // min delta -2, adjusted deltas 0, 0, 0, 3 in 2 bits.
        let mut b = vec![128, 1, 4, 5, 14]; // first value 7 zigzag 14
        b.push(3); // min delta -2 zigzag 3
        b.extend_from_slice(&[2, 0, 0, 0]);
        let mut packed = vec![0u8; 32 * 2 / 8];
        packed[0] = 3 << 6;
        b.extend_from_slice(&packed);
        let (v, _) = delta_binary(&b, None).unwrap();
        assert_eq!(v, vec![7, 5, 3, 1, 2]);
    }

    /// A data page claiming 2^30 values in a column chunk of 10, and a chunk claiming 2^40:
    /// refused before any value is decoded.
    #[test]
    fn page_value_count_bomb() {
        let leaf = Leaf {
            path: vec!["v".into()],
            nodes: vec![1],
            physical: Physical::Int32,
            logical: Logical::None,
            max_def: 0,
            max_rep: 0,
        };
        // PageHeader {1: DATA_PAGE, 2: 0, 3: 0, 5: DataPageHeader {1: 2^30}}.
        let page = [
            0x15, 0, 0x15, 0, 0x15, 0, 0x2C, 0x15, 0x80, 0x80, 0x80, 0x80, 0x08, 0, 0,
        ];
        let chunk = |num_values| Chunk {
            codec: 0,
            num_values,
            data_page: 0,
            dict_page: None,
            compressed: page.len() as u64,
        };
        let e = read_chunk(&page, &leaf, &chunk(10), 1 << 20)
            .map(|_| ())
            .unwrap_err();
        assert!(e.contains("more values than its column"), "{e}");
        let e = read_chunk(&page, &leaf, &chunk(1 << 40), 1 << 20)
            .map(|_| ())
            .unwrap_err();
        assert!(e.contains("too many values"), "{e}");
    }

    #[test]
    fn thrift_struct() {
        // Field 1 i32 = 3 (zigzag 6), field 4 binary "ab", field 5 list of 2 i32.
        let b = [0x15, 6, 0x38, 2, b'a', b'b', 0x19, 0x25, 2, 4, 0];
        let t = Thrift::new(&b).structure(0).unwrap();
        assert_eq!(t.int(1), Some(3));
        assert_eq!(t.str(4).as_deref(), Some("ab"));
        assert_eq!(t.list(5).len(), 2);
    }
}
