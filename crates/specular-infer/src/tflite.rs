//! A bounded reader for TensorFlow Lite model files (`.tflite`: a FlatBuffer), enough for the
//! float and float16 vision models the Local AI pack runs (MediaPipe's BlazeFace, FaceMesh
//! and Blendshape models).
//!
//! Written from the published schema (`schema.fbs`): the model's operator codes, the first
//! subgraph's tensors, operators and inputs/outputs, and the buffers holding constant data.
//! Every offset is checked against the file; a bad file gives [`Error::Weights`], never a
//! panic. Builtin options are kept as a table position and read by field when an op is
//! translated ([`Options`]).
//!
//! [`crate::tflite_graph`] turns a model into a [`crate::Graph`].

use crate::Error;
use crate::tensor::Tensor;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(format!("tflite: {}", s.into())))
}

/// The file's bytes with FlatBuffer accessors. Every read is bounds-checked.
#[derive(Clone, Copy)]
struct Fb<'a>(&'a [u8]);

impl<'a> Fb<'a> {
    fn u8(self, at: usize) -> Result<u8, Error> {
        match self.0.get(at) {
            Some(&b) => Ok(b),
            None => bad("offset past the end"),
        }
    }

    fn u16(self, at: usize) -> Result<u16, Error> {
        match self.0.get(at..at.wrapping_add(2)) {
            Some(b) => Ok(u16::from_le_bytes([b[0], b[1]])),
            None => bad("offset past the end"),
        }
    }

    fn u32(self, at: usize) -> Result<u32, Error> {
        match self.0.get(at..at.wrapping_add(4)) {
            Some(b) => Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            None => bad("offset past the end"),
        }
    }

    fn u64(self, at: usize) -> Result<u64, Error> {
        match self.0.get(at..at.wrapping_add(8)) {
            Some(b) => {
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                Ok(u64::from_le_bytes(a))
            }
            None => bad("offset past the end"),
        }
    }

    /// Follows a `uoffset` stored at `at`.
    fn deref(self, at: usize) -> Result<usize, Error> {
        let off = self.u32(at)? as usize;
        let to = at.checked_add(off).filter(|&t| t < self.0.len());
        match to {
            Some(t) => Ok(t),
            None => bad("reference past the end"),
        }
    }

    fn table(self, at: usize) -> Result<Table<'a>, Error> {
        let soff = self.u32(at)? as i32 as i64;
        let vt = at as i64 - soff;
        if vt < 0 || vt as usize >= self.0.len() {
            return bad("vtable past the end");
        }
        let vt = vt as usize;
        let vlen = self.u16(vt)? as usize;
        if vlen < 4 || !vlen.is_multiple_of(2) {
            return bad("bad vtable");
        }
        Ok(Table {
            fb: self,
            at,
            vt,
            fields: (vlen - 4) / 2,
        })
    }
}

/// A FlatBuffer table.
#[derive(Clone, Copy)]
struct Table<'a> {
    fb: Fb<'a>,
    at: usize,
    vt: usize,
    fields: usize,
}

impl<'a> Table<'a> {
    /// The position of field `i`, or `None` when it is absent (default value).
    fn field(&self, i: usize) -> Result<Option<usize>, Error> {
        if i >= self.fields {
            return Ok(None);
        }
        let off = self.fb.u16(self.vt + 4 + 2 * i)? as usize;
        Ok(if off == 0 { None } else { Some(self.at + off) })
    }

    fn u8(&self, i: usize, def: u8) -> Result<u8, Error> {
        match self.field(i)? {
            Some(p) => self.fb.u8(p),
            None => Ok(def),
        }
    }

    fn i32(&self, i: usize, def: i32) -> Result<i32, Error> {
        match self.field(i)? {
            Some(p) => Ok(self.fb.u32(p)? as i32),
            None => Ok(def),
        }
    }

    fn u64(&self, i: usize, def: u64) -> Result<u64, Error> {
        match self.field(i)? {
            Some(p) => self.fb.u64(p),
            None => Ok(def),
        }
    }

    fn f32(&self, i: usize, def: f32) -> Result<f32, Error> {
        match self.field(i)? {
            Some(p) => Ok(f32::from_bits(self.fb.u32(p)?)),
            None => Ok(def),
        }
    }

    /// A vector field: its first element's position and length (0 when absent).
    fn vector(&self, i: usize, elem: usize) -> Result<(usize, usize), Error> {
        let Some(p) = self.field(i)? else {
            return Ok((0, 0));
        };
        let v = self.fb.deref(p)?;
        let n = self.fb.u32(v)? as usize;
        let end = n
            .checked_mul(elem)
            .and_then(|b| b.checked_add(v + 4))
            .filter(|&e| e <= self.fb.0.len());
        if end.is_none() {
            return bad("vector past the end");
        }
        Ok((v + 4, n))
    }

    fn ints(&self, i: usize) -> Result<Vec<i32>, Error> {
        let (at, n) = self.vector(i, 4)?;
        (0..n)
            .map(|k| Ok(self.fb.u32(at + 4 * k)? as i32))
            .collect()
    }

    fn bytes(&self, i: usize) -> Result<&'a [u8], Error> {
        let (at, n) = self.vector(i, 1)?;
        Ok(&self.fb.0[at..at + n])
    }

    fn string(&self, i: usize) -> Result<String, Error> {
        Ok(String::from_utf8_lossy(self.bytes(i)?).into_owned())
    }

    fn tables(&self, i: usize) -> Result<Vec<Table<'a>>, Error> {
        let (at, n) = self.vector(i, 4)?;
        (0..n)
            .map(|k| {
                let p = self.fb.deref(at + 4 * k)?;
                self.fb.table(p)
            })
            .collect()
    }

    fn sub(&self, i: usize) -> Result<Option<Table<'a>>, Error> {
        match self.field(i)? {
            Some(p) => {
                let t = self.fb.deref(p)?;
                Ok(Some(self.fb.table(t)?))
            }
            None => Ok(None),
        }
    }
}

/// Element types of tensors (`TensorType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TensorType {
    F32,
    F16,
    I32,
    U8,
    I64,
    I8,
    /// Any other (strings, booleans, complex, …): never an input to an op we run.
    Other(u8),
}

impl TensorType {
    fn of(code: u8) -> TensorType {
        match code {
            0 => TensorType::F32,
            1 => TensorType::F16,
            2 => TensorType::I32,
            3 => TensorType::U8,
            4 => TensorType::I64,
            9 => TensorType::I8,
            c => TensorType::Other(c),
        }
    }

    fn size(self) -> Option<usize> {
        match self {
            TensorType::F32 | TensorType::I32 => Some(4),
            TensorType::F16 => Some(2),
            TensorType::U8 | TensorType::I8 => Some(1),
            TensorType::I64 => Some(8),
            TensorType::Other(_) => None,
        }
    }
}

/// A tensor of the subgraph.
#[derive(Clone, Debug, PartialEq)]
pub struct TensorDef {
    pub name: String,
    pub shape: Vec<usize>,
    pub ty: TensorType,
    /// Index into the model's buffers; buffer 0 is the empty sentinel.
    pub buffer: usize,
    /// Quantised (has a scale): not supported for arithmetic.
    pub quantized: bool,
}

/// Builtin operator codes this crate knows by name (`BuiltinOperator`).
pub mod code {
    pub const ADD: i32 = 0;
    pub const AVERAGE_POOL_2D: i32 = 1;
    pub const CONCATENATION: i32 = 2;
    pub const CONV_2D: i32 = 3;
    pub const DEPTHWISE_CONV_2D: i32 = 4;
    pub const DEQUANTIZE: i32 = 6;
    pub const FULLY_CONNECTED: i32 = 9;
    pub const LOGISTIC: i32 = 14;
    pub const MAX_POOL_2D: i32 = 17;
    pub const MUL: i32 = 18;
    pub const RELU: i32 = 19;
    pub const RELU6: i32 = 21;
    pub const RESHAPE: i32 = 22;
    pub const RESIZE_BILINEAR: i32 = 23;
    pub const SOFTMAX: i32 = 25;
    pub const TANH: i32 = 28;
    pub const PAD: i32 = 34;
    pub const TRANSPOSE: i32 = 39;
    pub const MEAN: i32 = 40;
    pub const SUB: i32 = 41;
    pub const DIV: i32 = 42;
    pub const SQUEEZE: i32 = 43;
    pub const NEG: i32 = 59;
    pub const STRIDED_SLICE: i32 = 45;
    pub const PRELU: i32 = 54;
    pub const MAXIMUM: i32 = 55;
    pub const MINIMUM: i32 = 57;
    pub const PADV2: i32 = 60;
    pub const SLICE: i32 = 65;
    pub const EXPAND_DIMS: i32 = 70;
    pub const SUM: i32 = 74;
    pub const SQRT: i32 = 75;
    pub const RSQRT: i32 = 76;
    pub const SQUARED_DIFFERENCE: i32 = 99;
    pub const HARD_SWISH: i32 = 117;
    pub const BATCH_MATMUL: i32 = 126;
    pub const GELU: i32 = 150;
}

/// An operator's builtin options: a table read by field index when the op is translated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    at: Option<usize>,
}

/// An operator of the subgraph.
#[derive(Clone, Debug, PartialEq)]
pub struct Operator {
    /// The builtin code (`BuiltinOperator`), or -1 for a custom op.
    pub code: i32,
    /// A custom op's name.
    pub custom: Option<String>,
    /// Tensor indices; -1 is an omitted optional input.
    pub inputs: Vec<i32>,
    pub outputs: Vec<i32>,
    pub options: Options,
}

/// A parsed model: the first subgraph and the buffers, borrowing the file's bytes.
pub struct Model<'a> {
    bytes: &'a [u8],
    pub description: String,
    pub tensors: Vec<TensorDef>,
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    pub operators: Vec<Operator>,
    /// (start, length) in `bytes` of each buffer's data.
    buffers: Vec<(usize, usize)>,
}

impl std::fmt::Debug for Model<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("tensors", &self.tensors.len())
            .field("operators", &self.operators.len())
            .finish()
    }
}

/// Largest tensor rank read.
const MAX_RANK: usize = 6;

impl<'a> Model<'a> {
    /// Parses a `.tflite` file held in memory.
    pub fn parse(bytes: &'a [u8]) -> Result<Model<'a>, Error> {
        if bytes.len() < 8 {
            return bad("file too short");
        }
        if &bytes[4..8] != b"TFL3" {
            return bad("not a TensorFlow Lite model (no TFL3 identifier)");
        }
        let fb = Fb(bytes);
        let root = fb.table(fb.deref(0)?)?;
        let version = root.i32(0, 0)?;
        if version != 3 {
            return bad(format!("schema version {version}"));
        }
        let mut codes = Vec::new();
        for oc in root.tables(1)? {
            let deprecated = oc.u8(0, 0)? as i8 as i32;
            let builtin = oc.i32(3, 0)?;
            let custom = if oc.field(1)?.is_some() {
                Some(oc.string(1)?)
            } else {
                None
            };
            let c = if custom.is_some() && deprecated == code_custom() {
                -1
            } else {
                deprecated.max(builtin)
            };
            codes.push((c, custom));
        }
        let subgraphs = root.tables(2)?;
        let Some(sg) = subgraphs.first() else {
            return bad("no subgraph");
        };
        let description = if root.field(3)?.is_some() {
            root.string(3)?
        } else {
            String::new()
        };
        let mut buffers = Vec::new();
        for b in root.tables(4)? {
            let off = b.u64(1, 0)?;
            let size = b.u64(2, 0)?;
            if off > 1 {
                let (o, s) = (off as usize, size as usize);
                if o.checked_add(s).is_none_or(|e| e > bytes.len()) {
                    return bad("buffer past the end");
                }
                buffers.push((o, s));
            } else {
                let (at, n) = b.vector(0, 1)?;
                buffers.push((at, n));
            }
        }
        let mut tensors = Vec::new();
        for t in sg.tables(0)? {
            let shape = t.ints(0)?;
            if shape.len() > MAX_RANK || shape.iter().any(|&d| d < 0) {
                return bad(format!("tensor shape {shape:?}"));
            }
            let buffer = t.i32(2, 0)? as u32 as usize;
            if buffer >= buffers.len().max(1) {
                return bad("tensor buffer index");
            }
            let quantized = match t.sub(4)? {
                Some(q) => q.vector(2, 4)?.1 > 0,
                None => false,
            };
            tensors.push(TensorDef {
                name: if t.field(3)?.is_some() {
                    t.string(3)?
                } else {
                    String::new()
                },
                shape: shape.iter().map(|&d| d as usize).collect(),
                ty: TensorType::of(t.u8(1, 0)?),
                buffer,
                quantized,
            });
        }
        let index = |v: i32, what: &str| -> Result<usize, Error> {
            if v < 0 || v as usize >= tensors.len() {
                return bad(format!("{what} tensor index {v}"));
            }
            Ok(v as usize)
        };
        let inputs = sg
            .ints(1)?
            .into_iter()
            .map(|v| index(v, "input"))
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = sg
            .ints(2)?
            .into_iter()
            .map(|v| index(v, "output"))
            .collect::<Result<Vec<_>, _>>()?;
        let mut operators = Vec::new();
        for op in sg.tables(3)? {
            let oc = op.i32(0, 0)? as u32 as usize;
            let Some((code, custom)) = codes.get(oc).cloned() else {
                return bad("operator code index");
            };
            let ins = op.ints(1)?;
            let outs = op.ints(2)?;
            for &v in ins.iter().filter(|&&v| v != -1) {
                index(v, "operator input")?;
            }
            for &v in &outs {
                index(v, "operator output")?;
            }
            let options = match op.field(4)? {
                Some(p) => {
                    let t = fb.deref(p)?;
                    fb.table(t)?;
                    Options { at: Some(t) }
                }
                None => Options { at: None },
            };
            operators.push(Operator {
                code,
                custom,
                inputs: ins,
                outputs: outs,
                options,
            });
        }
        Ok(Model {
            bytes,
            description,
            tensors,
            inputs,
            outputs,
            operators,
            buffers,
        })
    }

    fn opts(&self, o: Options) -> Result<Option<Table<'a>>, Error> {
        match o.at {
            Some(at) => Ok(Some(Fb(self.bytes).table(at)?)),
            None => Ok(None),
        }
    }

    /// An option field read as a byte (enums, booleans), or `def` when absent.
    pub fn opt_u8(&self, o: Options, field: usize, def: u8) -> Result<u8, Error> {
        match self.opts(o)? {
            Some(t) => t.u8(field, def),
            None => Ok(def),
        }
    }

    /// An option field read as an int32.
    pub fn opt_i32(&self, o: Options, field: usize, def: i32) -> Result<i32, Error> {
        match self.opts(o)? {
            Some(t) => t.i32(field, def),
            None => Ok(def),
        }
    }

    /// An option field read as a float.
    pub fn opt_f32(&self, o: Options, field: usize, def: f32) -> Result<f32, Error> {
        match self.opts(o)? {
            Some(t) => t.f32(field, def),
            None => Ok(def),
        }
    }

    /// An option field read as a vector of int32 (empty when absent).
    pub fn opt_ints(&self, o: Options, field: usize) -> Result<Vec<i32>, Error> {
        match self.opts(o)? {
            Some(t) => t.ints(field),
            None => Ok(Vec::new()),
        }
    }

    /// The raw bytes of tensor `t`'s constant data (empty for a computed tensor).
    pub fn data(&self, t: usize) -> &'a [u8] {
        let Some(def) = self.tensors.get(t) else {
            return &[];
        };
        match self.buffers.get(def.buffer) {
            Some(&(at, n)) => &self.bytes[at..at + n],
            None => &[],
        }
    }

    /// Whether tensor `t` holds constant data.
    pub fn is_constant(&self, t: usize) -> bool {
        !self.data(t).is_empty()
    }

    /// Tensor `t`'s constant data as a [`Tensor`] (f32, or f16 kept as f16), in the file's
    /// layout. Integer tensors are read as f32 (exact up to 2^24).
    pub fn constant(&self, t: usize) -> Result<Tensor, Error> {
        let def = &self.tensors[t];
        let d = self.data(t);
        let n: usize = def.shape.iter().product();
        let Some(es) = def.ty.size() else {
            return bad(format!("tensor {:?}: unsupported type", def.name));
        };
        if def.quantized && def.ty != TensorType::F16 && def.ty != TensorType::F32 {
            return bad(format!("tensor {:?} is quantised", def.name));
        }
        if d.len() != n * es {
            return bad(format!(
                "tensor {:?}: {} bytes for shape {:?}",
                def.name,
                d.len(),
                def.shape
            ));
        }
        let shape = &def.shape;
        Ok(match def.ty {
            TensorType::F32 => Tensor::f32(
                shape,
                d.chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect(),
            ),
            TensorType::F16 => Tensor::f16_bits(
                shape,
                d.chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect(),
            ),
            TensorType::I32 => Tensor::f32(
                shape,
                d.chunks_exact(4)
                    .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32)
                    .collect(),
            ),
            TensorType::I64 => Tensor::f32(
                shape,
                d.chunks_exact(8)
                    .map(|b| {
                        let mut a = [0u8; 8];
                        a.copy_from_slice(b);
                        i64::from_le_bytes(a) as f32
                    })
                    .collect(),
            ),
            TensorType::U8 => Tensor::f32(shape, d.iter().map(|&b| b as f32).collect()),
            TensorType::I8 => Tensor::f32(shape, d.iter().map(|&b| b as i8 as f32).collect()),
            TensorType::Other(_) => unreachable!("sized above"),
        })
    }

    /// Integer constant data (shapes, axes, paddings, permutations).
    pub fn ints(&self, t: usize) -> Result<Vec<i64>, Error> {
        let def = &self.tensors[t];
        let d = self.data(t);
        match def.ty {
            TensorType::I32 => Ok(d
                .chunks_exact(4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as i64)
                .collect()),
            TensorType::I64 => Ok(d
                .chunks_exact(8)
                .map(|b| {
                    let mut a = [0u8; 8];
                    a.copy_from_slice(b);
                    i64::from_le_bytes(a)
                })
                .collect()),
            _ => bad(format!("tensor {:?} is not an integer constant", def.name)),
        }
    }

    /// The builtin codes the subgraph uses, sorted, with how often.
    pub fn op_counts(&self) -> Vec<(i32, usize)> {
        let mut m = std::collections::BTreeMap::new();
        for op in &self.operators {
            *m.entry(op.code).or_insert(0usize) += 1;
        }
        m.into_iter().collect()
    }
}

/// `BuiltinOperator.CUSTOM`.
const fn code_custom() -> i32 {
    32
}

/// Reads one stored (uncompressed) file from a zip archive, such as a MediaPipe `.task`
/// bundle. Compressed entries are refused.
pub fn zip_entry<'a>(zip: &'a [u8], name: &str) -> Result<&'a [u8], Error> {
    let z = |s: &str| Error::Weights(format!("bundle: {s}"));
    let le16 = |at: usize| -> Result<usize, Error> {
        zip.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .ok_or_else(|| z("truncated"))
    };
    let le32 = |at: usize| -> Result<usize, Error> {
        zip.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .ok_or_else(|| z("truncated"))
    };
    // End of central directory: the last "PK\5\6" within the final 64 KiB + 22 bytes.
    let lo = zip.len().saturating_sub(65_557);
    let eocd = (lo..zip.len().saturating_sub(21))
        .rev()
        .find(|&i| zip[i..i + 4] == *b"PK\x05\x06")
        .ok_or_else(|| z("not a zip file"))?;
    let entries = le16(eocd + 10)?;
    let mut at = le32(eocd + 16)?;
    for _ in 0..entries {
        if zip.get(at..at + 4) != Some(b"PK\x01\x02") {
            return Err(z("bad central directory"));
        }
        let method = le16(at + 10)?;
        let csize = le32(at + 20)?;
        let usize_ = le32(at + 24)?;
        let nlen = le16(at + 28)?;
        let xlen = le16(at + 30)?;
        let clen = le16(at + 32)?;
        let local = le32(at + 42)?;
        let fname = zip
            .get(at + 46..at + 46 + nlen)
            .ok_or_else(|| z("truncated"))?;
        if fname == name.as_bytes() {
            if method != 0 || csize != usize_ {
                return Err(z(&format!("{name} is compressed")));
            }
            if zip.get(local..local + 4) != Some(b"PK\x03\x04") {
                return Err(z("bad local header"));
            }
            let start = local + 30 + le16(local + 26)? + le16(local + 28)?;
            return zip
                .get(start..start + csize)
                .ok_or_else(|| z("entry past the end"));
        }
        at += 46 + nlen + xlen + clen;
    }
    Err(z(&format!("no {name}")))
}
