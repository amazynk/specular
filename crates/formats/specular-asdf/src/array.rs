//! ndarrays: found in the tree, read plane by plane.

use std::sync::Arc;

use crate::yaml::{Node, Value};
use crate::{Block, Error, MAX_ARRAYS, Result};

/// Element types (the ASDF core ndarray datatypes this reader handles).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dtype {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
    /// Read as the magnitude.
    C64,
    /// Read as the magnitude.
    C128,
}

impl Dtype {
    pub fn parse(s: &str) -> Option<Dtype> {
        Some(match s {
            "bool8" | "bool" => Dtype::Bool,
            "int8" => Dtype::I8,
            "uint8" => Dtype::U8,
            "int16" => Dtype::I16,
            "uint16" => Dtype::U16,
            "int32" => Dtype::I32,
            "uint32" => Dtype::U32,
            "int64" => Dtype::I64,
            "uint64" => Dtype::U64,
            "float16" => Dtype::F16,
            "float32" => Dtype::F32,
            "float64" => Dtype::F64,
            "complex64" => Dtype::C64,
            "complex128" => Dtype::C128,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Dtype::Bool => "bool8",
            Dtype::I8 => "int8",
            Dtype::U8 => "uint8",
            Dtype::I16 => "int16",
            Dtype::U16 => "uint16",
            Dtype::I32 => "int32",
            Dtype::U32 => "uint32",
            Dtype::I64 => "int64",
            Dtype::U64 => "uint64",
            Dtype::F16 => "float16",
            Dtype::F32 => "float32",
            Dtype::F64 => "float64",
            Dtype::C64 => "complex64",
            Dtype::C128 => "complex128",
        }
    }

    /// Bytes per element.
    pub fn size(self) -> usize {
        match self {
            Dtype::Bool | Dtype::I8 | Dtype::U8 => 1,
            Dtype::I16 | Dtype::U16 | Dtype::F16 => 2,
            Dtype::I32 | Dtype::U32 | Dtype::F32 => 4,
            Dtype::I64 | Dtype::U64 | Dtype::F64 | Dtype::C64 => 8,
            Dtype::C128 => 16,
        }
    }

    pub fn is_int(self) -> bool {
        matches!(
            self,
            Dtype::I8
                | Dtype::U8
                | Dtype::I16
                | Dtype::U16
                | Dtype::I32
                | Dtype::U32
                | Dtype::I64
                | Dtype::U64
        )
    }

    pub fn is_complex(self) -> bool {
        matches!(self, Dtype::C64 | Dtype::C128)
    }

    /// Bits of a sample, for display.
    pub fn bits(self) -> u32 {
        if self == Dtype::Bool {
            1
        } else {
            self.size() as u32 * 8
        }
    }
}

/// Where an array's elements are.
#[derive(Clone, Debug, PartialEq)]
pub enum ArraySource {
    /// A block of this file.
    Block(usize),
    /// The first block of another ASDF file, named relative to this one.
    External(String),
    /// Written in the tree: values in C order (complex as magnitude, bool as 0/1, null as NaN).
    Inline(Arc<Vec<f64>>),
}

/// One `core/ndarray` in the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct NdArray {
    /// Dotted tree path (`roman.data`, `history.extensions[0]...`).
    pub path: String,
    pub source: ArraySource,
    pub dtype: Dtype,
    pub big_endian: bool,
    /// Sizes, slowest axis first (numpy order); a streamed array's first size is found from
    /// its block's length.
    pub shape: Vec<u64>,
    /// Byte strides per axis; `None` for C-contiguous.
    pub strides: Option<Vec<i64>>,
    /// Byte offset of element 0 in the source.
    pub offset: u64,
    /// Elements equal to this read as NaN.
    pub mask: Option<f64>,
    /// A mask array of the same shape: elements where it is non-zero read as NaN (apply with
    /// [`NdArray::apply_mask`]).
    pub mask_array: Option<Box<NdArray>>,
    /// The unit, for a `unit/quantity` wrapping the array.
    pub unit: Option<String>,
}

impl NdArray {
    /// The last path component (`data` for `roman.data`).
    pub fn name(&self) -> &str {
        self.path.rsplit('.').next().unwrap_or(&self.path)
    }

    /// The path of the mapping holding the array (`roman` for `roman.data`).
    pub fn parent(&self) -> &str {
        self.path.rfind('.').map_or("", |p| &self.path[..p])
    }

    pub fn len(&self) -> u64 {
        self.shape.iter().product()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Byte strides, explicit or C-contiguous.
    pub fn byte_strides(&self) -> Vec<i64> {
        if let Some(s) = &self.strides {
            return s.clone();
        }
        let mut out = vec![0i64; self.shape.len()];
        let mut acc = self.dtype.size() as i64;
        for k in (0..self.shape.len()).rev() {
            out[k] = acc;
            acc = acc.saturating_mul(self.shape[k] as i64);
        }
        out
    }

    /// The same array with its length-1 axes removed (keeping at least two axes when it has
    /// them), so that `(20, 24, 1)` reads as a 20 × 24 image.
    pub fn squeezed(&self) -> NdArray {
        let strides = self.byte_strides();
        let mut keep: Vec<usize> = (0..self.shape.len())
            .filter(|&k| self.shape[k] != 1)
            .collect();
        if keep.len() < 2 && self.shape.len() >= 2 {
            // Keep the last axes that make up two.
            let mut k = self.shape.len();
            while keep.len() < 2 && k > 0 {
                k -= 1;
                if !keep.contains(&k) {
                    keep.push(k);
                }
            }
            keep.sort_unstable();
        }
        NdArray {
            shape: keep.iter().map(|&k| self.shape[k]).collect(),
            strides: Some(keep.iter().map(|&k| strides[k]).collect()),
            ..self.clone()
        }
    }

    /// The element range `[lo, hi)` in bytes the array touches, from its offset and strides.
    fn byte_span(&self) -> Option<(i128, i128)> {
        let strides = self.byte_strides();
        let mut lo = self.offset as i128;
        let mut hi = self.offset as i128;
        if self.shape.contains(&0) {
            return Some((lo, lo));
        }
        for (k, &n) in self.shape.iter().enumerate() {
            let reach = i128::from(strides[k]).checked_mul(n as i128 - 1)?;
            if reach < 0 {
                lo += reach;
            } else {
                hi += reach;
            }
        }
        Some((lo, hi + self.dtype.size() as i128))
    }

    /// Checks the array lies inside `len` source bytes.
    pub fn check(&self, len: usize) -> Result<()> {
        if matches!(self.source, ArraySource::Inline(_)) {
            return Ok(());
        }
        match self.byte_span() {
            Some((lo, hi)) if lo >= 0 && hi <= len as i128 => Ok(()),
            _ => Err(Error::Malformed(format!(
                "array {} (shape {:?}, offset {}) lies outside its {len}-byte block",
                self.path, self.shape, self.offset
            ))),
        }
    }

    /// One 2-D plane as `f32`, rows along the second-to-last axis, in array order (row 0
    /// first): `lead` fixes every other axis. A 1-D array is one row. `bytes` is the source
    /// data ([`crate::Asdf::array_data`]); it is ignored for inline arrays.
    pub fn plane(&self, bytes: &[u8], lead: &[u64]) -> Result<Vec<f32>> {
        let nd = self.shape.len();
        if nd == 0 {
            return Err(Error::Unsupported(format!(
                "array {} has no axes",
                self.path
            )));
        }
        let (rows, cols) = if nd == 1 {
            (1, self.shape[0])
        } else {
            (self.shape[nd - 2], self.shape[nd - 1])
        };
        let fixed = nd.saturating_sub(2);
        if lead.len() != fixed || lead.iter().zip(&self.shape).any(|(&i, &n)| i >= n) {
            return Err(Error::Malformed(format!(
                "plane {lead:?} outside shape {:?}",
                self.shape
            )));
        }
        let n = usize::try_from(rows.saturating_mul(cols)).map_err(|_| Error::TooLarge)?;
        if n > 1 << 30 {
            return Err(Error::TooLarge);
        }
        // What the data holds is checked before the plane is allocated: a shape alone (inline
        // values, or zero strides over a short block) must not ask for gigabytes.
        if let ArraySource::Inline(values) = &self.source
            && n > values.len()
        {
            return Err(Error::Malformed(format!(
                "inline array {} holds fewer values than its shape {:?}",
                self.path, self.shape
            )));
        }
        self.check(bytes.len())?;
        let inline = matches!(self.source, ArraySource::Inline(_));
        if !inline && n > (bytes.len() as u64).max(1 << 20).saturating_mul(64) as usize {
            return Err(Error::TooLarge);
        }
        let mut out = Vec::with_capacity(n);
        if let ArraySource::Inline(values) = &self.source {
            // C-order element strides.
            let mut es = vec![1u64; nd];
            for k in (0..nd.saturating_sub(1)).rev() {
                es[k] = es[k + 1] * self.shape[k + 1];
            }
            let base: u64 = lead.iter().zip(&es).map(|(i, s)| i * s).sum();
            let (rs, cs) = if nd == 1 {
                (0, es[0])
            } else {
                (es[nd - 2], es[nd - 1])
            };
            for r in 0..rows {
                for c in 0..cols {
                    let v = values
                        .get((base + r * rs + c * cs) as usize)
                        .copied()
                        .unwrap_or(f64::NAN);
                    out.push(self.masked(v) as f32);
                }
            }
            return Ok(out);
        }
        let strides = self.byte_strides();
        let base = self.offset as i64
            + lead
                .iter()
                .zip(&strides)
                .map(|(&i, &s)| i as i64 * s)
                .sum::<i64>();
        let (rs, cs) = if nd == 1 {
            (0, strides[0])
        } else {
            (strides[nd - 2], strides[nd - 1])
        };
        let be = self.big_endian;
        macro_rules! run {
            ($n:literal, $conv:expr) => {{
                let conv = $conv;
                for r in 0..rows as i64 {
                    let row = base + r * rs;
                    for c in 0..cols as i64 {
                        let o = (row + c * cs) as usize;
                        let mut a = [0u8; $n];
                        a.copy_from_slice(&bytes[o..o + $n]);
                        let v: f64 = conv(a);
                        out.push(self.masked(v) as f32);
                    }
                }
            }};
        }
        match self.dtype {
            Dtype::Bool => run!(1, |a: [u8; 1]| f64::from(u8::from(a[0] != 0))),
            Dtype::I8 => run!(1, |a: [u8; 1]| f64::from(a[0] as i8)),
            Dtype::U8 => run!(1, |a: [u8; 1]| f64::from(a[0])),
            Dtype::I16 => run!(2, |a| f64::from(if be {
                i16::from_be_bytes(a)
            } else {
                i16::from_le_bytes(a)
            })),
            Dtype::U16 => run!(2, |a| f64::from(if be {
                u16::from_be_bytes(a)
            } else {
                u16::from_le_bytes(a)
            })),
            Dtype::I32 => run!(4, |a| f64::from(if be {
                i32::from_be_bytes(a)
            } else {
                i32::from_le_bytes(a)
            })),
            Dtype::U32 => run!(4, |a| f64::from(if be {
                u32::from_be_bytes(a)
            } else {
                u32::from_le_bytes(a)
            })),
            Dtype::I64 => run!(8, |a| (if be {
                i64::from_be_bytes(a)
            } else {
                i64::from_le_bytes(a)
            }) as f64),
            Dtype::U64 => run!(8, |a| (if be {
                u64::from_be_bytes(a)
            } else {
                u64::from_le_bytes(a)
            }) as f64),
            Dtype::F16 => run!(2, |a| {
                let h = if be {
                    u16::from_be_bytes(a)
                } else {
                    u16::from_le_bytes(a)
                };
                f64::from(specular_hdf5::f16_to_f32(h))
            }),
            Dtype::F32 => run!(4, |a| f64::from(if be {
                f32::from_be_bytes(a)
            } else {
                f32::from_le_bytes(a)
            })),
            Dtype::F64 => run!(8, |a| if be {
                f64::from_be_bytes(a)
            } else {
                f64::from_le_bytes(a)
            }),
            Dtype::C64 => run!(8, |a: [u8; 8]| {
                let (re, im) = (&a[..4], &a[4..]);
                let f = |b: &[u8]| {
                    let b: [u8; 4] = b.try_into().unwrap_or([0; 4]);
                    f64::from(if be {
                        f32::from_be_bytes(b)
                    } else {
                        f32::from_le_bytes(b)
                    })
                };
                f(re).hypot(f(im))
            }),
            Dtype::C128 => run!(16, |a: [u8; 16]| {
                let f = |b: &[u8]| {
                    let b: [u8; 8] = b.try_into().unwrap_or([0; 8]);
                    if be {
                        f64::from_be_bytes(b)
                    } else {
                        f64::from_le_bytes(b)
                    }
                };
                f(&a[..8]).hypot(f(&a[8..]))
            }),
        }
        Ok(out)
    }

    /// Sets `values` (a plane of this array) to NaN where `mask` (the same plane of
    /// [`NdArray::mask_array`]) is non-zero.
    pub fn apply_mask(values: &mut [f32], mask: &[f32]) {
        for (v, &m) in values.iter_mut().zip(mask) {
            if m != 0.0 {
                *v = f32::NAN;
            }
        }
    }

    fn masked(&self, v: f64) -> f64 {
        match self.mask {
            Some(m) if v == m || (m.is_nan() && v.is_nan()) => f64::NAN,
            _ => v,
        }
    }
}

/// Tags whose subtrees are not searched for images (GWCS and its transforms carry arrays of
/// coefficients, tables carry columns, not pictures).
fn opaque(v: &Value) -> bool {
    v.tag.as_deref().is_some_and(|t| {
        t.contains("gwcs/")
            || t.contains("/transform/")
            || t.contains(":transform/")
            || t.contains("/coordinates/")
            || t.contains("/table/")
            || t.contains(":table/")
    }) || v.tag_is("table")
}

/// Every ndarray in the tree, and those that cannot be read with why.
pub(crate) fn collect(tree: &Value, blocks: &[Block]) -> (Vec<NdArray>, Vec<(String, String)>) {
    let mut found = Vec::new();
    let mut skipped = Vec::new();
    walk(tree, String::new(), blocks, None, &mut found, &mut skipped);
    (found, skipped)
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn walk(
    v: &Value,
    path: String,
    blocks: &[Block],
    unit: Option<String>,
    found: &mut Vec<NdArray>,
    skipped: &mut Vec<(String, String)>,
) {
    if found.len() >= MAX_ARRAYS {
        return;
    }
    if v.tag_is("ndarray") {
        match ndarray(v, &path, blocks) {
            Ok(mut a) => {
                a.unit = unit;
                found.push(a);
            }
            Err(why) => skipped.push((path, why)),
        }
        return;
    }
    if path == "history" || path == "asdf_library" || opaque(v) {
        return;
    }
    if v.tag_is("quantity")
        && let Some(inner) = v.get("value")
        && inner.tag_is("ndarray")
    {
        let u = v.get("unit").and_then(Value::scalar_text);
        walk(inner, path, blocks, u, found, skipped);
        return;
    }
    match &v.node {
        Node::Map(m) => {
            for (k, c) in m {
                walk(c, join(&path, k), blocks, None, found, skipped);
            }
        }
        Node::Seq(items) => {
            for (i, c) in items.iter().enumerate() {
                walk(c, format!("{path}[{i}]"), blocks, None, found, skipped);
            }
        }
        _ => {}
    }
}

fn ndarray(v: &Value, path: &str, blocks: &[Block]) -> std::result::Result<NdArray, String> {
    if let Node::Seq(_) = v.node {
        // An ndarray written as a bare nested list.
        let (values, shape, all_int) = inline(v)?;
        return Ok(NdArray {
            path: path.to_owned(),
            source: ArraySource::Inline(Arc::new(values)),
            dtype: if all_int { Dtype::I64 } else { Dtype::F64 },
            big_endian: false,
            shape,
            strides: None,
            offset: 0,
            mask: None,
            mask_array: None,
            unit: None,
        });
    }
    let datatype = v.get("datatype");
    let dtype = match datatype.map(|d| &d.node) {
        Some(Node::Str(s)) => Dtype::parse(s).ok_or_else(|| format!("datatype {s}"))?,
        Some(Node::Seq(_)) => return Err("string or structured datatype".into()),
        None if v.get("data").is_some() => Dtype::F64,
        _ => return Err("no datatype".into()),
    };
    let big_endian = match v.get("byteorder").and_then(Value::as_str) {
        Some("little") => false,
        Some("big") => true,
        None => false,
        Some(o) => return Err(format!("byteorder {o}")),
    };
    let mut streamed = false;
    let mut shape = Vec::new();
    if let Some(s) = v.get("shape") {
        let items = s.as_seq().ok_or("shape is not a list")?;
        for (k, d) in items.iter().enumerate() {
            match (&d.node, k) {
                (Node::Int(n), _) if *n >= 0 => {
                    shape.push(u64::try_from(*n).map_err(|_| "shape too large")?)
                }
                (Node::Str(s), 0) if s == "*" => {
                    streamed = true;
                    shape.push(0);
                }
                _ => return Err(format!("shape {:?}", s.scalar_text())),
            }
        }
    }
    let strides = match v.get("strides") {
        Some(s) => Some(
            s.as_seq()
                .ok_or("strides is not a list")?
                .iter()
                .map(|x| x.as_i64().ok_or("stride is not an integer"))
                .collect::<std::result::Result<Vec<i64>, _>>()?,
        ),
        None => None,
    };
    if strides.as_ref().is_some_and(|s| s.len() != shape.len()) {
        return Err("strides and shape differ in length".into());
    }
    let offset = match v.get("offset") {
        Some(o) => u64::try_from(o.as_i64().ok_or("offset is not an integer")?)
            .map_err(|_| "negative offset")?,
        None => 0,
    };
    let mask = match v.get("mask").map(|m| &m.node) {
        None | Some(Node::Null) => None,
        Some(Node::Int(i)) => Some(*i as f64),
        Some(Node::Float(f)) => Some(*f),
        Some(Node::Bool(b)) => Some(f64::from(u8::from(*b))),
        Some(_) => None,
    };
    let mask_array = match v.get("mask") {
        Some(m) if m.tag_is("ndarray") || matches!(m.node, Node::Seq(_)) => {
            let m = ndarray(m, &format!("{path}.mask"), blocks)?;
            if m.shape != shape && !streamed {
                return Err("mask and array shapes differ".into());
            }
            Some(Box::new(m))
        }
        _ => None,
    };
    let source = match (v.get("source"), v.get("data")) {
        (Some(s), _) => match &s.node {
            Node::Int(i) => {
                let i = if *i < 0 {
                    blocks.len() as i128 + *i
                } else {
                    *i
                };
                let i = usize::try_from(i).map_err(|_| "negative block index")?;
                if i >= blocks.len() {
                    return Err(format!("block {i} missing ({} in the file)", blocks.len()));
                }
                ArraySource::Block(i)
            }
            Node::Str(name) if name.starts_with("fits:") => {
                return Err(format!("stored in the FITS HDU {}", &name[5..]));
            }
            Node::Str(name) => ArraySource::External(name.clone()),
            _ => return Err("source is neither a block number nor a file name".into()),
        },
        (None, Some(d)) => {
            let (values, ishape, _) = inline(d)?;
            if shape.is_empty() {
                shape = ishape;
            } else if shape.iter().product::<u64>() != values.len() as u64 {
                return Err("inline data and shape disagree".into());
            }
            ArraySource::Inline(Arc::new(values))
        }
        (None, None) => return Err("no source and no data".into()),
    };
    if streamed {
        let ArraySource::Block(i) = source else {
            return Err("a streamed shape outside a block".into());
        };
        let row = shape[1..]
            .iter()
            .try_fold(dtype.size() as u64, |a, &d| a.checked_mul(d))
            .ok_or("too many elements")?;
        let avail = blocks[i].data_size.saturating_sub(offset);
        shape[0] = avail.checked_div(row).unwrap_or(0);
    }
    if shape.len() > 32 {
        return Err("more than 32 axes".into());
    }
    let mut total: u64 = 1;
    for &d in &shape {
        total = total.checked_mul(d).ok_or("too many elements")?;
    }
    Ok(NdArray {
        path: path.to_owned(),
        source,
        dtype,
        big_endian,
        shape,
        strides,
        offset,
        mask,
        mask_array,
        unit: None,
    })
}

/// Inline data: values in C order, the shape found from the nesting, whether all are integers.
fn inline(v: &Value) -> std::result::Result<(Vec<f64>, Vec<u64>, bool), String> {
    let mut values = Vec::new();
    let mut shape: Vec<u64> = Vec::new();
    let mut all_int = true;
    fn go(
        v: &Value,
        depth: usize,
        values: &mut Vec<f64>,
        shape: &mut Vec<u64>,
        all_int: &mut bool,
    ) -> std::result::Result<(), String> {
        match &v.node {
            Node::Seq(items) => {
                if shape.len() == depth {
                    shape.push(items.len() as u64);
                } else if shape.get(depth) != Some(&(items.len() as u64)) {
                    return Err("ragged inline data".into());
                }
                for i in items {
                    go(i, depth + 1, values, shape, all_int)?;
                }
                Ok(())
            }
            _ => {
                if shape.len() > depth {
                    return Err("ragged inline data".into());
                }
                let x = match &v.node {
                    Node::Int(i) => *i as f64,
                    Node::Float(f) => {
                        *all_int = false;
                        *f
                    }
                    Node::Bool(b) => f64::from(u8::from(*b)),
                    Node::Null => {
                        *all_int = false;
                        f64::NAN
                    }
                    Node::Str(s) => {
                        *all_int = false;
                        complex_magnitude(s).ok_or_else(|| format!("inline value {s}"))?
                    }
                    _ => return Err("nested mapping in inline data".into()),
                };
                values.push(x);
                Ok(())
            }
        }
    }
    go(v, 0, &mut values, &mut shape, &mut all_int)?;
    Ok((values, shape, all_int))
}

/// `1+2j`, `(1-2j)`, `3j`, `inf+nanj`: the magnitude.
fn complex_magnitude(s: &str) -> Option<f64> {
    let t = s
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .replace(' ', "");
    let Some(body) = t.strip_suffix(['j', 'J']) else {
        return parse_f(&t);
    };
    // Split at the last sign that is not part of an exponent or the leading sign.
    let b = body.as_bytes();
    let mut split = None;
    for k in (1..b.len()).rev() {
        if (b[k] == b'+' || b[k] == b'-') && !matches!(b[k - 1], b'e' | b'E') {
            split = Some(k);
            break;
        }
    }
    let (re, im) = match split {
        Some(k) => (parse_f(&body[..k])?, parse_f(&body[k..])?),
        None => (0.0, parse_f(body)?),
    };
    Some(re.hypot(im))
}

fn parse_f(s: &str) -> Option<f64> {
    match s.trim_start_matches('+') {
        "inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        "nan" | "-nan" => Some(f64::NAN),
        t => t.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_text() {
        assert_eq!(complex_magnitude("3+4j"), Some(5.0));
        assert_eq!(complex_magnitude("(3-4j)"), Some(5.0));
        assert_eq!(complex_magnitude("-4j"), Some(4.0));
        assert_eq!(complex_magnitude("1e2+0j"), Some(100.0));
        assert_eq!(complex_magnitude("1e-2-0j"), Some(0.01));
        assert_eq!(complex_magnitude("2.5"), Some(2.5));
        assert!(
            complex_magnitude("inf+nanj").unwrap().is_nan()
                || complex_magnitude("inf+nanj").unwrap().is_infinite()
        );
    }
}
