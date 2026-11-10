//! USDC, the binary "crate" form of a USD layer (version 0.4 and later): a bootstrap header
//! ("PXR-USDC", version, table-of-contents offset), sections of tokens (LZ4), strings,
//! fields (token and value-rep arrays, integer-coded and LZ4), field sets, paths (a tree
//! coded as parent-relative jumps) and specs. Each field's value is a 64-bit rep: type,
//! array, inlined and compressed flags and a payload (an inlined value or a file offset).
//! Values the viewer has no use for (most dictionaries, unregistered types) read as none.

use super::lz4::{compressed_ints, decompress};
use super::value::{Arc, Layer, Spec, SpecKind, V};
use crate::{Error, Result};

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Malformed(format!("USDC: {}", msg.into())))
}

/// Elements an array may declare at most (the bytes behind it are checked too).
const MAX_ELEMENTS: u64 = 1 << 30;

struct Reader<'a> {
    b: &'a [u8],
    version: (u8, u8, u8),
    tokens: Vec<String>,
    strings: Vec<u32>,
    paths: Vec<String>,
    /// Recursion depth of value reads (dictionaries inside dictionaries).
    depth: u32,
    /// Values read behind offsets so far ([`MAX_NESTED_VALUES`]).
    nested: u64,
}

/// Values read behind dictionary offsets in one file, at most: every entry of a dictionary
/// may point at the same nested dictionary, so 16 levels of a few entries would otherwise
/// read exponentially many values.
const MAX_NESTED_VALUES: u64 = 1 << 22;

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    match b.get(at..at + 4) {
        Some(s) => Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]])),
        None => bad("read past the end"),
    }
}

fn u64_at(b: &[u8], at: usize) -> Result<u64> {
    match b.get(at..at + 8) {
        Some(s) => Ok(u64::from_le_bytes(s.try_into().expect("8"))),
        None => bad("read past the end"),
    }
}

fn f64_at(b: &[u8], at: usize) -> Result<f64> {
    u64_at(b, at).map(f64::from_bits)
}

fn f16(h: u16) -> f64 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f64::from(h & 0x3ff);
    s * match e {
        0 => m * 2f64.powi(-24),
        31 => f64::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f64.powi(e - 15),
    }
}

/// A cursor over the file for reading values.
struct Cur {
    at: usize,
}

/// Crate value types (the numbering of the format).
mod ty {
    pub const BOOL: u8 = 1;
    pub const UCHAR: u8 = 2;
    pub const INT: u8 = 3;
    pub const UINT: u8 = 4;
    pub const INT64: u8 = 5;
    pub const UINT64: u8 = 6;
    pub const HALF: u8 = 7;
    pub const FLOAT: u8 = 8;
    pub const DOUBLE: u8 = 9;
    pub const STRING: u8 = 10;
    pub const TOKEN: u8 = 11;
    pub const ASSET: u8 = 12;
    pub const MATRIX2D: u8 = 13;
    pub const MATRIX3D: u8 = 14;
    pub const MATRIX4D: u8 = 15;
    pub const QUATD: u8 = 16;
    pub const QUATF: u8 = 17;
    pub const QUATH: u8 = 18;
    pub const VEC2D: u8 = 19;
    pub const VEC2F: u8 = 20;
    pub const VEC2H: u8 = 21;
    pub const VEC2I: u8 = 22;
    pub const VEC3D: u8 = 23;
    pub const VEC3F: u8 = 24;
    pub const VEC3H: u8 = 25;
    pub const VEC3I: u8 = 26;
    pub const VEC4D: u8 = 27;
    pub const VEC4F: u8 = 28;
    pub const VEC4H: u8 = 29;
    pub const VEC4I: u8 = 30;
    pub const DICTIONARY: u8 = 31;
    pub const TOKEN_LIST_OP: u8 = 32;
    pub const STRING_LIST_OP: u8 = 33;
    pub const PATH_LIST_OP: u8 = 34;
    pub const REFERENCE_LIST_OP: u8 = 35;
    pub const PATH_VECTOR: u8 = 40;
    pub const TOKEN_VECTOR: u8 = 41;
    pub const SPECIFIER: u8 = 42;
    pub const VARIABILITY: u8 = 44;
    pub const VARIANT_SELECTION_MAP: u8 = 45;
    pub const TIME_SAMPLES: u8 = 46;
    pub const PAYLOAD: u8 = 47;
    pub const DOUBLE_VECTOR: u8 = 48;
    pub const STRING_VECTOR: u8 = 50;
    pub const PAYLOAD_LIST_OP: u8 = 55;
    pub const TIME_CODE: u8 = 56;
}

/// Scalar layout of a numeric type: (components, bytes per component, kind) where kind is
/// 0 signed int, 1 unsigned int, 2 float.
fn numeric(t: u8) -> Option<(usize, usize, u8)> {
    use ty::*;
    Some(match t {
        BOOL | UCHAR => (1, 1, 1),
        INT => (1, 4, 0),
        UINT => (1, 4, 1),
        INT64 => (1, 8, 0),
        UINT64 => (1, 8, 1),
        HALF => (1, 2, 2),
        FLOAT => (1, 4, 2),
        DOUBLE | TIME_CODE => (1, 8, 2),
        MATRIX2D => (4, 8, 2),
        MATRIX3D => (9, 8, 2),
        MATRIX4D => (16, 8, 2),
        QUATD => (4, 8, 2),
        QUATF => (4, 4, 2),
        QUATH => (4, 2, 2),
        VEC2D => (2, 8, 2),
        VEC2F => (2, 4, 2),
        VEC2H => (2, 2, 2),
        VEC2I => (2, 4, 0),
        VEC3D => (3, 8, 2),
        VEC3F => (3, 4, 2),
        VEC3H => (3, 2, 2),
        VEC3I => (3, 4, 0),
        VEC4D => (4, 8, 2),
        VEC4F => (4, 4, 2),
        VEC4H => (4, 2, 2),
        VEC4I => (4, 4, 0),
        _ => return None,
    })
}

fn scalar(b: &[u8], size: usize, kind: u8) -> f64 {
    match (size, kind) {
        (1, 0) => f64::from(b[0] as i8),
        (1, _) => f64::from(b[0]),
        (2, 2) => f16(u16::from_le_bytes([b[0], b[1]])),
        (2, 0) => f64::from(i16::from_le_bytes([b[0], b[1]])),
        (2, _) => f64::from(u16::from_le_bytes([b[0], b[1]])),
        (4, 2) => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        (4, 0) => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        (4, _) => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        (8, 2) => f64::from_le_bytes(b[..8].try_into().expect("8")),
        (8, 0) => i64::from_le_bytes(b[..8].try_into().expect("8")) as f64,
        _ => u64::from_le_bytes(b[..8].try_into().expect("8")) as f64,
    }
}

/// Quaternions are stored imaginary first (x, y, z, w); the text and the viewer use w first.
fn quat_wxyz(d: &mut [f64]) {
    for q in d.chunks_exact_mut(4) {
        q.rotate_right(1);
    }
}

impl Reader<'_> {
    fn at_least(&self, v: (u8, u8, u8)) -> bool {
        self.version >= v
    }

    fn token(&self, i: u64) -> Result<String> {
        match self.tokens.get(i as usize) {
            Some(t) => Ok(t.clone()),
            None => bad("token index out of range"),
        }
    }

    fn string(&self, i: u64) -> Result<String> {
        match self.strings.get(i as usize) {
            Some(&t) => self.token(u64::from(t)),
            None => bad("string index out of range"),
        }
    }

    fn path(&self, i: u64) -> Result<String> {
        match self.paths.get(i as usize) {
            Some(p) => Ok(p.clone()),
            None => bad("path index out of range"),
        }
    }

    fn u32(&self, c: &mut Cur) -> Result<u32> {
        let v = u32_at(self.b, c.at)?;
        c.at += 4;
        Ok(v)
    }

    fn u64(&self, c: &mut Cur) -> Result<u64> {
        let v = u64_at(self.b, c.at)?;
        c.at += 8;
        Ok(v)
    }

    fn count(&self, c: &mut Cur) -> Result<u64> {
        let n = self.u64(c)?;
        if n > MAX_ELEMENTS || n as usize > self.b.len() {
            return bad("count past the file");
        }
        Ok(n)
    }

    /// A vector of u32 indices (tokens, paths, strings) mapped through `f`.
    fn index_vec(
        &self,
        c: &mut Cur,
        f: impl Fn(&Self, u64) -> Result<String>,
    ) -> Result<Vec<String>> {
        let n = self.count(c)?;
        let _res = specular_core::budget::reserve_n(n, size_of::<String>() as u64)?;
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let i = self.u32(c)?;
            out.push(f(self, u64::from(i))?);
        }
        Ok(out)
    }

    /// Reads a list op of `item` and flattens it (explicit, else prepended + added +
    /// appended).
    fn list_op<T>(
        &self,
        c: &mut Cur,
        item: impl Fn(&Self, &mut Cur) -> Result<T>,
    ) -> Result<Vec<T>> {
        let Some(&h) = self.b.get(c.at) else {
            return bad("list op cut short");
        };
        c.at += 1;
        let mut lists: Vec<(u8, Vec<T>)> = Vec::new();
        for bit in [1u8, 2, 5, 6, 3, 4] {
            if h & (1 << bit) != 0 {
                let n = self.count(c)?;
                let mut v = Vec::with_capacity(n.min(1 << 16) as usize);
                for _ in 0..n {
                    v.push(item(self, c)?);
                }
                lists.push((bit, v));
            }
        }
        let mut out = Vec::new();
        // Explicit items, else prepended, added, appended (deleted and ordered dropped).
        if let Some((_, v)) = lists.iter_mut().find(|(b, _)| *b == 1) {
            return Ok(std::mem::take(v));
        }
        for want in [5u8, 2, 6] {
            if let Some((_, v)) = lists.iter_mut().find(|(b, _)| *b == want) {
                out.append(v);
            }
        }
        Ok(out)
    }

    fn skip_dict(&mut self, c: &mut Cur) -> Result<V> {
        self.depth += 1;
        let r = if self.depth > 16 {
            bad("dictionaries nested too deeply")
        } else {
            self.dict_entries(c)
        };
        self.depth -= 1;
        r
    }

    fn dict_entries(&mut self, c: &mut Cur) -> Result<V> {
        // Keys and values, each value behind a relative offset.
        let n = self.count(c)?;
        let mut out = Vec::new();
        for _ in 0..n {
            let k = self.u32(c)?;
            let key = self.string(u64::from(k))?;
            let v = self.value_behind_offset(c)?;
            out.push((key, v));
        }
        Ok(V::Dict(out))
    }

    /// An int64 offset relative to its own position, the value rep there.
    fn value_behind_offset(&mut self, c: &mut Cur) -> Result<V> {
        let start = c.at;
        let off = self.u64(c)? as i64;
        let at = (start as i64)
            .checked_add(off)
            .filter(|&a| a >= 0)
            .ok_or_else(|| Error::Malformed("USDC: value offset".into()))?;
        let rep = u64_at(self.b, at as usize)?;
        self.nested += 1;
        if self.nested > MAX_NESTED_VALUES {
            return bad("too many nested values");
        }
        self.depth += 1;
        let v = if self.depth > 16 {
            bad("values nested too deeply")
        } else {
            self.value(rep)
        };
        self.depth -= 1;
        v
    }

    fn arc(&self, c: &mut Cur, reference: bool) -> Result<Arc> {
        let asset = self.u32(c)?;
        let prim = self.u32(c)?;
        let a = Arc {
            asset: self.string(u64::from(asset))?,
            prim: self.path(u64::from(prim))?,
        };
        if reference || self.at_least((0, 8, 0)) {
            // Layer offset: offset, scale.
            c.at += 16;
        }
        Ok(a)
    }

    /// The value of a rep (`V::None` for types not read).
    fn value(&mut self, rep: u64) -> Result<V> {
        let array = rep & (1 << 63) != 0;
        let inlined = rep & (1 << 62) != 0;
        let compressed = rep & (1 << 61) != 0;
        let t = ((rep >> 48) & 0xff) as u8;
        let payload = rep & ((1 << 48) - 1);
        if array {
            return self.array(t, payload, compressed);
        }
        if inlined {
            return self.inlined(t, payload);
        }
        let mut c = Cur {
            at: payload as usize,
        };
        if c.at >= self.b.len() {
            return bad("value offset past the end");
        }
        use ty::*;
        Ok(match t {
            TOKEN_VECTOR => V::Strs(self.index_vec(&mut c, |r, i| r.token(i))?),
            STRING_VECTOR => V::Strs(self.index_vec(&mut c, |r, i| r.string(i))?),
            PATH_VECTOR => V::Paths(self.index_vec(&mut c, |r, i| r.path(i))?),
            DOUBLE_VECTOR => {
                let n = self.count(&mut c)? as usize;
                let mut d = Vec::with_capacity(n);
                for k in 0..n {
                    d.push(f64_at(self.b, c.at + k * 8)?);
                }
                V::Nums { data: d, arity: 1 }
            }
            PATH_LIST_OP => V::Paths(self.list_op(&mut c, |r, c| {
                let i = r.u32(c)?;
                r.path(u64::from(i))
            })?),
            TOKEN_LIST_OP => V::Strs(self.list_op(&mut c, |r, c| {
                let i = r.u32(c)?;
                r.token(u64::from(i))
            })?),
            STRING_LIST_OP => V::Strs(self.list_op(&mut c, |r, c| {
                let i = r.u32(c)?;
                r.string(u64::from(i))
            })?),
            REFERENCE_LIST_OP => {
                // Each reference ends with a dictionary (custom data), so read in turn.
                let Some(&h) = self.b.get(c.at) else {
                    return bad("list op cut short");
                };
                c.at += 1;
                let mut out = Vec::new();
                let mut explicit = None;
                for bit in [1u8, 2, 5, 6, 3, 4] {
                    if h & (1 << bit) == 0 {
                        continue;
                    }
                    let n = self.count(&mut c)?;
                    let mut v = Vec::new();
                    for _ in 0..n {
                        let a = self.arc(&mut c, true)?;
                        self.skip_dict(&mut c)?;
                        v.push(a);
                    }
                    match bit {
                        1 => explicit = Some(v),
                        2 | 5 | 6 => out.extend(v),
                        _ => {}
                    }
                }
                V::Arcs(explicit.unwrap_or(out))
            }
            PAYLOAD => V::Arcs(vec![self.arc(&mut c, false)?]),
            PAYLOAD_LIST_OP => V::Arcs(self.list_op(&mut c, |r, c| r.arc(c, false))?),
            VARIANT_SELECTION_MAP => {
                let n = self.count(&mut c)?;
                let mut d = Vec::new();
                for _ in 0..n {
                    let k = self.u32(&mut c)?;
                    let v = self.u32(&mut c)?;
                    d.push((
                        self.string(u64::from(k))?,
                        V::Str(self.string(u64::from(v))?),
                    ));
                }
                V::Dict(d)
            }
            DICTIONARY => self.skip_dict(&mut c)?,
            TIME_SAMPLES => self.time_samples(&mut c)?,
            STRING => {
                let i = self.u32(&mut c)?;
                V::Str(self.string(u64::from(i))?)
            }
            t => match numeric(t) {
                Some((n, size, kind)) => {
                    let mut d = Vec::with_capacity(n);
                    for k in 0..n {
                        let Some(s) = self.b.get(c.at + k * size..c.at + (k + 1) * size) else {
                            return bad("value past the end");
                        };
                        d.push(scalar(s, size, kind));
                    }
                    if matches!(t, QUATD | QUATF | QUATH) {
                        quat_wxyz(&mut d);
                    }
                    if n == 1 {
                        V::Num(d[0])
                    } else {
                        V::Nums { data: d, arity: n }
                    }
                }
                None => V::None,
            },
        })
    }

    fn time_samples(&mut self, c: &mut Cur) -> Result<V> {
        // [offset to the times rep][…][times rep][offset to the values][…][count][reps]
        let start = c.at;
        let off = self.u64(c)? as i64;
        let times_at = usize::try_from(start as i64 + off)
            .map_err(|_| Error::Malformed("USDC: time samples".into()))?;
        let times_rep = u64_at(self.b, times_at)?;
        let times = self.value(times_rep)?;
        let mut c2 = Cur { at: times_at + 8 };
        let start = c2.at;
        let off = self.u64(&mut c2)? as i64;
        let vals_at = usize::try_from(start as i64 + off)
            .map_err(|_| Error::Malformed("USDC: time samples".into()))?;
        let mut c3 = Cur { at: vals_at };
        let n = self.count(&mut c3)? as usize;
        let times = times.nums().map(|t| t.into_owned()).unwrap_or_default();
        let mut out = Vec::with_capacity(n.min(4096));
        for k in 0..n {
            let rep = u64_at(self.b, c3.at + k * 8)?;
            let t = times.get(k).copied().unwrap_or(k as f64);
            // Only the first sample is used; the rest are kept as none to save time.
            let v = if k == 0 { self.value(rep)? } else { V::None };
            out.push((t, v));
        }
        Ok(V::Samples(out))
    }

    fn inlined(&self, t: u8, p: u64) -> Result<V> {
        use ty::*;
        let lo = p as u32;
        Ok(match t {
            BOOL => V::Bool(lo != 0),
            UCHAR => V::Num(f64::from(lo as u8)),
            INT => V::Num(f64::from(lo as i32)),
            UINT => V::Num(f64::from(lo)),
            INT64 => V::Num(f64::from(lo as i32)),
            UINT64 => V::Num(f64::from(lo)),
            HALF => V::Num(f16(lo as u16)),
            // Doubles that fit are stored as floats.
            FLOAT | DOUBLE | TIME_CODE => V::Num(f64::from(f32::from_bits(lo))),
            TOKEN => V::Str(self.token(u64::from(lo))?),
            STRING => V::Str(self.string(u64::from(lo))?),
            ASSET => V::Asset(self.token(u64::from(lo))?),
            SPECIFIER => V::Ident(
                match lo {
                    0 => "def",
                    1 => "over",
                    _ => "class",
                }
                .into(),
            ),
            VARIABILITY => V::Ident(if lo == 1 { "uniform" } else { "varying" }.into()),
            DICTIONARY => V::Dict(Vec::new()),
            MATRIX2D | MATRIX3D | MATRIX4D => {
                // The diagonal as signed bytes.
                let n = match t {
                    MATRIX2D => 2,
                    MATRIX3D => 3,
                    _ => 4,
                };
                let mut d = vec![0.0; n * n];
                for k in 0..n {
                    d[k * n + k] = f64::from((lo >> (8 * k)) as u8 as i8);
                }
                V::Nums {
                    data: d,
                    arity: n * n,
                }
            }
            t => match numeric(t) {
                // Vectors whose components fit signed bytes.
                Some((n, _, _)) if n > 1 => {
                    let mut d: Vec<f64> = (0..n)
                        .map(|k| f64::from((lo >> (8 * k)) as u8 as i8))
                        .collect();
                    if matches!(t, QUATD | QUATF | QUATH) {
                        quat_wxyz(&mut d);
                    }
                    V::Nums { data: d, arity: n }
                }
                _ => V::None,
            },
        })
    }

    fn array(&mut self, t: u8, offset: u64, compressed: bool) -> Result<V> {
        let mut c = Cur {
            at: offset as usize,
        };
        if offset == 0 {
            // An empty array is written with no data.
            return Ok(V::Nums {
                data: Vec::new(),
                arity: numeric(t).map_or(1, |n| n.0),
            });
        }
        if !self.at_least((0, 5, 0)) {
            c.at += 4;
        }
        let n = if self.at_least((0, 7, 0)) {
            self.count(&mut c)?
        } else {
            u64::from(self.u32(&mut c)?)
        };
        if n > MAX_ELEMENTS {
            return bad("array too large");
        }
        let n = n as usize;
        use ty::*;
        match t {
            TOKEN => return Ok(V::Strs(self.raw_indices(&mut c, n, |r, i| r.token(i))?)),
            STRING => return Ok(V::Strs(self.raw_indices(&mut c, n, |r, i| r.string(i))?)),
            ASSET => return Ok(V::Strs(self.raw_indices(&mut c, n, |r, i| r.token(i))?)),
            _ => {}
        }
        let Some((comps, size, kind)) = numeric(t) else {
            return Ok(V::None);
        };
        let total = n
            .checked_mul(comps)
            .filter(|&v| v as u64 <= MAX_ELEMENTS)
            .ok_or_else(|| Error::Malformed("USDC: array too large".into()))?;
        // f64 values, and as much again for a compressed array's integers on the way.
        let _res = specular_core::budget::reserve_n(total as u64, 16)?;
        let ints = matches!(t, INT | UINT | INT64 | UINT64);
        let floats = matches!(t, HALF | FLOAT | DOUBLE);
        let mut data = Vec::new();
        if compressed && n >= 16 && ints && self.at_least((0, 5, 0)) {
            let len = self.u64(&mut c)? as usize;
            let src = self.slice(c.at, len)?;
            let v = compressed_ints(src, n, size == 8)?;
            data = v
                .into_iter()
                .map(|x| {
                    if kind == 1 && size == 4 {
                        f64::from(x as u32)
                    } else {
                        x as f64
                    }
                })
                .collect();
        } else if compressed && n >= 16 && floats && self.at_least((0, 6, 0)) {
            let Some(&code) = self.b.get(c.at) else {
                return bad("array cut short");
            };
            c.at += 1;
            match code {
                b'i' => {
                    let len = self.u64(&mut c)? as usize;
                    let src = self.slice(c.at, len)?;
                    data = compressed_ints(src, n, false)?
                        .into_iter()
                        .map(|x| x as f64)
                        .collect();
                }
                b't' => {
                    let lut_n = self.u32(&mut c)? as usize;
                    let lut_bytes = lut_n
                        .checked_mul(size)
                        .ok_or_else(|| Error::Malformed("USDC: lookup table".into()))?;
                    let lut_src = self.slice(c.at, lut_bytes)?;
                    let lut: Vec<f64> = lut_src
                        .chunks_exact(size)
                        .map(|s| scalar(s, size, kind))
                        .collect();
                    c.at += lut_bytes;
                    let len = self.u64(&mut c)? as usize;
                    let src = self.slice(c.at, len)?;
                    let idx = compressed_ints(src, n, false)?;
                    data.reserve(n);
                    for i in idx {
                        match lut.get(i as usize) {
                            Some(&v) => data.push(v),
                            None => return bad("lookup index out of range"),
                        }
                    }
                }
                _ => return bad("unknown float array coding"),
            }
        } else {
            let bytes = total
                .checked_mul(size)
                .ok_or_else(|| Error::Malformed("USDC: array too large".into()))?;
            let src = self.slice(c.at, bytes)?;
            data = src
                .chunks_exact(size)
                .map(|s| scalar(s, size, kind))
                .collect();
        }
        if matches!(t, QUATD | QUATF | QUATH) {
            quat_wxyz(&mut data);
        }
        Ok(V::Nums { data, arity: comps })
    }

    fn raw_indices(
        &self,
        c: &mut Cur,
        n: usize,
        f: impl Fn(&Self, u64) -> Result<String>,
    ) -> Result<Vec<String>> {
        let src = self.slice(c.at, n.saturating_mul(4))?;
        let _res = specular_core::budget::reserve_n(n as u64, size_of::<String>() as u64)?;
        src.chunks_exact(4)
            .map(|s| {
                f(
                    self,
                    u64::from(u32::from_le_bytes([s[0], s[1], s[2], s[3]])),
                )
            })
            .collect()
    }

    fn slice(&self, at: usize, len: usize) -> Result<&[u8]> {
        match at.checked_add(len).and_then(|e| self.b.get(at..e)) {
            Some(s) => Ok(s),
            None => bad("data past the end"),
        }
    }
}

/// Parses a USDC layer.
pub(crate) fn parse(b: &[u8]) -> Result<Layer> {
    if b.len() < 88 || !b.starts_with(b"PXR-USDC") {
        return bad("not a USDC file");
    }
    let version = (b[8], b[9], b[10]);
    if version < (0, 4, 0) {
        return Err(Error::Unsupported(format!(
            "USDC version {}.{}.{} (older than 0.4)",
            version.0, version.1, version.2
        )));
    }
    let toc = u64_at(b, 16)? as usize;
    let n_sections = u64_at(b, toc)?;
    if n_sections > 64 {
        return bad("too many sections");
    }
    let mut sections = std::collections::HashMap::new();
    for i in 0..n_sections as usize {
        let at = toc + 8 + i * 32;
        let name = b
            .get(at..at + 16)
            .ok_or_else(|| Error::Malformed("USDC: table of contents".into()))?;
        let name =
            String::from_utf8_lossy(name.split(|&c| c == 0).next().unwrap_or(&[])).into_owned();
        let start = u64_at(b, at + 16)? as usize;
        let size = u64_at(b, at + 24)? as usize;
        if start.checked_add(size).is_none_or(|e| e > b.len()) {
            return bad("section past the end");
        }
        sections.insert(name, start);
    }
    let sec = |n: &str| {
        sections
            .get(n)
            .copied()
            .ok_or_else(|| Error::Malformed(format!("USDC: no {n} section")))
    };
    let mut r = Reader {
        b,
        version,
        tokens: Vec::new(),
        strings: Vec::new(),
        paths: Vec::new(),
        depth: 0,
        nested: 0,
    };
    // Tokens.
    let mut c = Cur { at: sec("TOKENS")? };
    let n_tokens = r.count(&mut c)? as usize;
    let raw = r.u64(&mut c)? as usize;
    let comp = r.u64(&mut c)? as usize;
    let buf = decompress(r.slice(c.at, comp)?, raw)?;
    r.tokens = buf
        .split(|&c| c == 0)
        .take(n_tokens)
        .map(|t| String::from_utf8_lossy(t).into_owned())
        .collect();
    if r.tokens.len() < n_tokens {
        return bad("fewer tokens than said");
    }
    // Strings.
    let mut c = Cur {
        at: sec("STRINGS")?,
    };
    let n = r.count(&mut c)? as usize;
    let s = r.slice(c.at, n * 4)?;
    r.strings = s
        .chunks_exact(4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .collect();
    // Fields.
    let mut c = Cur { at: sec("FIELDS")? };
    let n_fields = r.count(&mut c)? as usize;
    let len = r.u64(&mut c)? as usize;
    let names = compressed_ints(r.slice(c.at, len)?, n_fields, false)?;
    c.at += len;
    let len = r.u64(&mut c)? as usize;
    let reps = decompress(r.slice(c.at, len)?, n_fields * 8)?;
    if reps.len() < n_fields * 8 {
        return bad("field values cut short");
    }
    let fields: Vec<(u32, u64)> = names
        .iter()
        .zip(reps.chunks_exact(8))
        .map(|(&n, v)| (n as u32, u64::from_le_bytes(v.try_into().expect("8"))))
        .collect();
    // Field sets.
    let mut c = Cur {
        at: sec("FIELDSETS")?,
    };
    let n_sets = r.count(&mut c)? as usize;
    let len = r.u64(&mut c)? as usize;
    let sets = compressed_ints(r.slice(c.at, len)?, n_sets, false)?;
    // Paths.
    let mut c = Cur { at: sec("PATHS")? };
    let n_paths = r.count(&mut c)? as usize;
    let n_enc = r.count(&mut c)? as usize;
    let mut arrays = Vec::new();
    for _ in 0..3 {
        let len = r.u64(&mut c)? as usize;
        arrays.push(compressed_ints(r.slice(c.at, len)?, n_enc, false)?);
        c.at += len;
    }
    r.paths = build_paths(&arrays[0], &arrays[1], &arrays[2], n_paths, &r.tokens)?;
    // Specs.
    let mut c = Cur { at: sec("SPECS")? };
    let n_specs = r.count(&mut c)? as usize;
    let mut arrays = Vec::new();
    for _ in 0..3 {
        let len = r.u64(&mut c)? as usize;
        arrays.push(compressed_ints(r.slice(c.at, len)?, n_specs, false)?);
        c.at += len;
    }
    // Field sets are runs of field indices ended by −1 (all ones); a spec names its start.
    let mut layer = Layer::default();
    for ((&pi, &si), &ki) in arrays[0]
        .iter()
        .zip(&arrays[1])
        .zip(&arrays[2])
        .take(n_specs)
    {
        let path = r.path(pi as u64)?;
        let set = si as usize;
        let kind = match ki {
            1 => SpecKind::Attribute,
            6 => SpecKind::Prim,
            7 => SpecKind::PseudoRoot,
            8 => SpecKind::Relationship,
            10 => SpecKind::Variant,
            11 => SpecKind::VariantSet,
            _ => SpecKind::Other,
        };
        let mut spec = Spec::new(kind);
        let mut i = set;
        while let Some(&f) = sets.get(i) {
            if f as u32 == u32::MAX {
                break;
            }
            let Some(&(name, rep)) = fields.get(f as usize) else {
                return bad("field index out of range");
            };
            let name = r.token(u64::from(name))?;
            // A value that fails to read is left out, not the whole file.
            r.depth = 0;
            let v = r.value(rep).unwrap_or(V::None);
            spec.fields.push((name, v));
            i += 1;
        }
        layer.specs.insert(path, spec);
    }
    Ok(layer)
}

/// Paths from the coded tree: each entry's path is its parent's plus its element (a prim
/// name, a variant selection `{set=name}`, or a property for negative token indices); a
/// jump says whether a child follows (−1), a sibling follows (0), both (> 0: the sibling at
/// that distance) or neither (−2).
fn build_paths(
    index: &[i64],
    element: &[i64],
    jump: &[i64],
    n_paths: usize,
    tokens: &[String],
) -> Result<Vec<String>> {
    let mut paths = vec![String::new(); n_paths];
    // Work list of (entry, parent path); siblings with children are pushed for later.
    let mut stack: Vec<(usize, Option<String>)> = vec![(0, None)];
    let mut steps = 0usize;
    while let Some((mut cur, mut parent)) = stack.pop() {
        loop {
            steps += 1;
            if cur >= index.len() || steps > index.len() * 2 + 16 {
                break;
            }
            let this = cur;
            cur += 1;
            let slot = usize::try_from(index[this])
                .ok()
                .filter(|&s| s < n_paths)
                .ok_or_else(|| Error::Malformed("USDC: path index".into()))?;
            let path = match &parent {
                None => "/".to_owned(),
                Some(p) => {
                    let t = element[this];
                    let name = tokens
                        .get(t.unsigned_abs() as usize)
                        .ok_or_else(|| Error::Malformed("USDC: path token".into()))?;
                    if t < 0 {
                        format!("{p}.{name}")
                    } else if name.starts_with('{') {
                        format!("{p}{name}")
                    } else {
                        super::value::child_path(p, name)
                    }
                }
            };
            paths[slot] = path.clone();
            let j = jump[this];
            let has_child = j > 0 || j == -1;
            let has_sibling = j >= 0;
            if has_child {
                if has_sibling {
                    let s = usize::try_from(this as i64 + j)
                        .map_err(|_| Error::Malformed("USDC: path jump".into()))?;
                    stack.push((s, parent.clone()));
                }
                parent = Some(path);
            } else if !has_sibling {
                break;
            }
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_of_itself_is_bounded() {
        // Seven dictionaries, each with 64 values all pointing at the next one (the
        // last is empty): 64^7 values read before the budget, none past the depth cap.
        let (k, levels) = (64u64, 7u64);
        let size = 8 + 12 * k + 8;
        let mut b = Vec::new();
        for d in 0..levels {
            let at = d * size;
            let rep_at = at + 8 + 12 * k;
            b.extend(k.to_le_bytes());
            for i in 0..k {
                b.extend(0u32.to_le_bytes());
                let own = at + 8 + 12 * i + 4;
                b.extend(((rep_at - own) as i64).to_le_bytes());
            }
            let next = (d + 1) * size;
            b.extend(((u64::from(ty::DICTIONARY) << 48) | next).to_le_bytes());
        }
        b.extend(0u64.to_le_bytes());
        b.extend([0u8; 8]);
        let mut r = Reader {
            b: &b,
            version: (0, 8, 0),
            tokens: vec!["k".into()],
            strings: vec![0],
            paths: Vec::new(),
            depth: 0,
            nested: 0,
        };
        let t = std::time::Instant::now();
        let Err(e) = r.value(u64::from(ty::DICTIONARY) << 48) else {
            panic!("accepted")
        };
        assert!(e.to_string().contains("too many nested values"), "{e}");
        assert!(t.elapsed().as_secs() < 30);
        assert_eq!(r.depth, 0, "depth restored on errors");
        // Lz4 reserves by its input, not by the size the file claims.
        let out = decompress(&[0, 0x10, b'x'], usize::MAX).unwrap();
        assert!(out.capacity() < 1 << 20, "{}", out.capacity());
    }

    #[test]
    fn large_arrays_reserve_from_the_budget() {
        // A 1 Mi-element compressed integer array in a 1 MiB file: 16 MiB of values and
        // integers on the way, refused by a 4 MiB budget before anything is unpacked.
        let n = 1u64 << 20;
        let mut b = vec![0u8; 8];
        b.extend(n.to_le_bytes());
        b.extend(16u64.to_le_bytes());
        b.resize(n as usize + 64, 0);
        let mut r = Reader {
            b: &b,
            version: (0, 8, 0),
            tokens: vec!["k".into()],
            strings: vec![0],
            paths: Vec::new(),
            depth: 0,
            nested: 0,
        };
        let Err(e) = specular_core::budget::with_limit(4 << 20, || r.array(ty::INT, 8, true))
        else {
            panic!("accepted")
        };
        assert!(
            matches!(&e, Error::TooLarge(m) if m.contains("free RAM")),
            "{e}"
        );
    }
}
