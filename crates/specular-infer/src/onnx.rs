//! A bounded reader for ONNX models (`.onnx`: a protobuf `ModelProto`), and a translator of
//! the small CNN op set face recognisers use (SFace's MobileFaceNet, AuraFace's IResNet) into
//! a [`Graph`].
//!
//! Written from the published `onnx.proto`: the graph's nodes with their attributes, its
//! initializers (raw or typed data), inputs and outputs. Every length is checked against the
//! file; a bad file gives [`Error::Weights`], never a panic. External data is refused.
//!
//! Translated ops: `Conv` (batch norms that follow a conv alone are folded into it),
//! `BatchNormalization`, `PRelu`, `Relu`, `Add`, `Sub`, `Mul`, `Div` (constants broadcast),
//! `Dropout` (identity), `Flatten`, `Reshape` (constant shape), `Gemm`, `MatMul`,
//! `MaxPool`, `AveragePool` (with `ceil_mode`), `GlobalAveragePool`, `Sigmoid`, `Identity`;
//! for U²-Net/IS-Net-style exports: `Concat`, `Resize` (linear with half-pixel
//! centres, or nearest), and the shape arithmetic that feeds it (`Constant`, `Shape`,
//! `Slice`, `Concat`, `Cast` of integer vectors), folded at build time since every shape is
//! known then.

use std::collections::HashMap;

use crate::Error;
use crate::graph::{
    Act, BatchNorm, BinaryOp, Conv2d, Graph, MatMul, Pool2d, PoolKind, ResizeMode, Value,
};
use crate::tensor::Tensor;

/// Longest integer vector folded at build time (shapes, sizes, slice bounds).
const MAX_RANK_INTS: usize = 16;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(format!("onnx: {}", s.into())))
}

/// One protobuf field: its number and value.
#[derive(Clone, Copy, Debug)]
enum Field<'a> {
    Varint(u64),
    Fixed64,
    Bytes(&'a [u8]),
    Fixed32(u32),
}

/// Iterates the fields of a protobuf message.
struct Fields<'a> {
    buf: &'a [u8],
    at: usize,
}

fn varint(buf: &[u8], at: &mut usize) -> Result<u64, Error> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let Some(&b) = buf.get(*at) else {
            return bad("truncated varint");
        };
        *at += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    bad("varint too long")
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u32, Field<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.buf.len() {
            return None;
        }
        Some(self.field())
    }
}

impl<'a> Fields<'a> {
    fn new(buf: &'a [u8]) -> Fields<'a> {
        Fields { buf, at: 0 }
    }

    fn field(&mut self) -> Result<(u32, Field<'a>), Error> {
        let key = varint(self.buf, &mut self.at)?;
        let num = (key >> 3) as u32;
        let take = |at: &mut usize, n: usize| -> Result<&'a [u8], Error> {
            let end = at.checked_add(n).filter(|&e| e <= self.buf.len());
            let Some(end) = end else {
                return bad("field past the end");
            };
            let s = &self.buf[*at..end];
            *at = end;
            Ok(s)
        };
        let f = match key & 7 {
            0 => Field::Varint(varint(self.buf, &mut self.at)?),
            1 => {
                let b = take(&mut self.at, 8)?;
                let mut a = [0u8; 8];
                a.copy_from_slice(b);
                let _ = u64::from_le_bytes(a);
                Field::Fixed64
            }
            2 => {
                let n = varint(self.buf, &mut self.at)?;
                let n = usize::try_from(n).map_err(|_| Error::Weights("onnx: length".into()))?;
                Field::Bytes(take(&mut self.at, n)?)
            }
            5 => {
                let b = take(&mut self.at, 4)?;
                Field::Fixed32(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            w => return bad(format!("wire type {w}")),
        };
        Ok((num, f))
    }
}

fn string(f: Field) -> String {
    match f {
        Field::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        _ => String::new(),
    }
}

/// Repeated int64 values, packed or not.
fn push_ints(f: Field, out: &mut Vec<i64>) -> Result<(), Error> {
    match f {
        Field::Varint(v) => out.push(v as i64),
        Field::Bytes(b) => {
            let mut at = 0;
            while at < b.len() {
                out.push(varint(b, &mut at)? as i64);
            }
        }
        _ => return bad("bad integer list"),
    }
    Ok(())
}

/// Repeated floats, packed or not.
fn push_floats(f: Field, out: &mut Vec<f32>) -> Result<(), Error> {
    match f {
        Field::Fixed32(v) => out.push(f32::from_bits(v)),
        Field::Bytes(b) => {
            if !b.len().is_multiple_of(4) {
                return bad("bad float list");
            }
            out.extend(
                b.chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])),
            );
        }
        _ => return bad("bad float list"),
    }
    Ok(())
}

/// A node attribute's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    Float(f32),
    Int(i64),
    Ints(Vec<i64>),
    Floats(Vec<f32>),
    String(String),
    Tensor(Tensor),
    /// A kind we don't read (graphs, sparse tensors, …).
    Other,
}

/// A node of the graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub op: String,
    pub domain: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub attrs: Vec<(String, Attr)>,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, a)| a)
    }

    fn int(&self, name: &str, def: i64) -> i64 {
        match self.attr(name) {
            Some(Attr::Int(v)) => *v,
            _ => def,
        }
    }

    fn float(&self, name: &str, def: f32) -> f32 {
        match self.attr(name) {
            Some(Attr::Float(v)) => *v,
            _ => def,
        }
    }

    fn ints(&self, name: &str) -> Option<&[i64]> {
        match self.attr(name) {
            Some(Attr::Ints(v)) => Some(v),
            _ => None,
        }
    }
}

/// A graph input or output: its name and shape (0 for a symbolic dimension).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Io {
    pub name: String,
    pub shape: Vec<usize>,
}

/// A parsed model.
#[derive(Debug)]
pub struct Model {
    pub opset: i64,
    pub nodes: Vec<Node>,
    pub initializers: HashMap<String, Tensor>,
    pub inputs: Vec<Io>,
    pub outputs: Vec<Io>,
}

/// Largest rank and element count of an initializer.
const MAX_RANK: usize = 6;
const MAX_ELEMENTS: usize = 1 << 28;

fn tensor(buf: &[u8]) -> Result<(String, Tensor), Error> {
    let mut dims = Vec::new();
    let (mut dtype, mut name) = (0i64, String::new());
    let mut raw: Option<&[u8]> = None;
    let mut floats = Vec::new();
    let mut ints = Vec::new();
    let mut external = false;
    for f in Fields::new(buf) {
        let (n, f) = f?;
        match n {
            1 => push_ints(f, &mut dims)?,
            2 => {
                if let Field::Varint(v) = f {
                    dtype = v as i64;
                }
            }
            4 => push_floats(f, &mut floats)?,
            5 | 7 => push_ints(f, &mut ints)?,
            8 => name = string(f),
            9 => {
                if let Field::Bytes(b) = f {
                    raw = Some(b);
                }
            }
            13 => external = true,
            14 => {
                if let Field::Varint(1) = f {
                    external = true;
                }
            }
            _ => {}
        }
    }
    if external {
        return bad(format!("{name}: external data is not supported"));
    }
    if dims.len() > MAX_RANK || dims.iter().any(|&d| d < 0) {
        return bad(format!("{name}: shape {dims:?}"));
    }
    let shape: Vec<usize> = dims.iter().map(|&d| d as usize).collect();
    let n = shape
        .iter()
        .try_fold(1usize, |a, &d| a.checked_mul(d))
        .filter(|&n| n <= MAX_ELEMENTS);
    let Some(n) = n else {
        return bad(format!("{name}: too large"));
    };
    let wrong = |got: usize| bad(format!("{name}: {got} values for shape {shape:?}"));
    let t = match (dtype, raw) {
        (1, Some(b)) => {
            if b.len() != n * 4 {
                return wrong(b.len() / 4);
            }
            Tensor::f32(
                &shape,
                b.chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
            )
        }
        (1, None) => {
            if floats.len() != n {
                return wrong(floats.len());
            }
            Tensor::f32(&shape, floats)
        }
        (10, Some(b)) => {
            if b.len() != n * 2 {
                return wrong(b.len() / 2);
            }
            Tensor::f16_bits(
                &shape,
                b.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect(),
            )
        }
        (10, None) => {
            if ints.len() != n {
                return wrong(ints.len());
            }
            Tensor::f16_bits(&shape, ints.iter().map(|&v| v as u16).collect())
        }
        (6 | 7, Some(b)) => {
            let w = if dtype == 6 { 4 } else { 8 };
            if b.len() != n * w {
                return wrong(b.len() / w);
            }
            Tensor::f32(
                &shape,
                b.chunks_exact(w)
                    .map(|c| {
                        let mut a = [0u8; 8];
                        a[..w].copy_from_slice(c);
                        if w == 4 {
                            i32::from_le_bytes([a[0], a[1], a[2], a[3]]) as f32
                        } else {
                            i64::from_le_bytes(a) as f32
                        }
                    })
                    .collect(),
            )
        }
        (6 | 7, None) => {
            if ints.len() != n {
                return wrong(ints.len());
            }
            Tensor::f32(&shape, ints.iter().map(|&v| v as f32).collect())
        }
        (t, _) => return bad(format!("{name}: data type {t}")),
    };
    Ok((name, t))
}

fn attribute(buf: &[u8]) -> Result<(String, Attr), Error> {
    let mut name = String::new();
    let (mut f1, mut i1, mut s1, mut t1) = (None, None, None, None);
    let (mut fl, mut il) = (Vec::new(), Vec::new());
    let mut ty = 0u64;
    for f in Fields::new(buf) {
        let (n, f) = f?;
        match n {
            1 => name = string(f),
            2 => {
                if let Field::Fixed32(v) = f {
                    f1 = Some(f32::from_bits(v));
                }
            }
            3 => {
                if let Field::Varint(v) = f {
                    i1 = Some(v as i64);
                }
            }
            4 => s1 = Some(string(f)),
            5 => {
                if let Field::Bytes(b) = f {
                    t1 = Some(tensor(b)?.1);
                }
            }
            7 => push_floats(f, &mut fl)?,
            8 => push_ints(f, &mut il)?,
            20 => {
                if let Field::Varint(v) = f {
                    ty = v;
                }
            }
            _ => {}
        }
    }
    // AttributeType: FLOAT 1, INT 2, STRING 3, TENSOR 4, FLOATS 6, INTS 7.
    let a = match ty {
        1 => Attr::Float(f1.unwrap_or(0.0)),
        2 => Attr::Int(i1.unwrap_or(0)),
        3 => Attr::String(s1.unwrap_or_default()),
        4 => t1.map(Attr::Tensor).unwrap_or(Attr::Other),
        6 => Attr::Floats(fl),
        7 => Attr::Ints(il),
        // Old files leave the type out: guess from what is set.
        0 if f1.is_some() => Attr::Float(f1.unwrap_or(0.0)),
        0 if i1.is_some() => Attr::Int(i1.unwrap_or(0)),
        0 if !il.is_empty() => Attr::Ints(il),
        0 if !fl.is_empty() => Attr::Floats(fl),
        0 if s1.is_some() => Attr::String(s1.unwrap_or_default()),
        _ => Attr::Other,
    };
    Ok((name, a))
}

fn node(buf: &[u8]) -> Result<Node, Error> {
    let mut n = Node {
        op: String::new(),
        domain: String::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        attrs: Vec::new(),
    };
    for f in Fields::new(buf) {
        let (num, f) = f?;
        match (num, f) {
            (1, f) => n.inputs.push(string(f)),
            (2, f) => n.outputs.push(string(f)),
            (4, f) => n.op = string(f),
            (5, Field::Bytes(b)) => n.attrs.push(attribute(b)?),
            (7, f) => n.domain = string(f),
            _ => {}
        }
    }
    Ok(n)
}

fn value_info(buf: &[u8]) -> Result<Io, Error> {
    let mut io = Io {
        name: String::new(),
        shape: Vec::new(),
    };
    // ValueInfoProto.type (2) → TypeProto.tensor_type (1) → shape (2) → dim (1) →
    // dim_value (1).
    let sub = |b: &[u8], want: u32| -> Result<Option<Vec<u8>>, Error> {
        for f in Fields::new(b) {
            if let (n, Field::Bytes(x)) = f?
                && n == want
            {
                return Ok(Some(x.to_vec()));
            }
        }
        Ok(None)
    };
    for f in Fields::new(buf) {
        match f? {
            (1, f) => io.name = string(f),
            (2, Field::Bytes(ty)) => {
                let Some(tt) = sub(ty, 1)? else { continue };
                let Some(shape) = sub(&tt, 2)? else { continue };
                for d in Fields::new(&shape) {
                    if let (1, Field::Bytes(dim)) = d? {
                        let mut v = 0usize;
                        for x in Fields::new(dim) {
                            if let (1, Field::Varint(n)) = x? {
                                v = n as usize;
                            }
                        }
                        io.shape.push(v);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(io)
}

impl Model {
    /// Parses an ONNX model held in memory.
    pub fn parse(bytes: &[u8]) -> Result<Model, Error> {
        let mut graph = None;
        let mut opset = 0;
        for f in Fields::new(bytes) {
            match f? {
                (7, Field::Bytes(g)) => graph = Some(g),
                (8, Field::Bytes(o)) => {
                    let mut domain = String::new();
                    let mut v = 0;
                    for x in Fields::new(o) {
                        match x? {
                            (1, f) => domain = string(f),
                            (2, Field::Varint(n)) => v = n as i64,
                            _ => {}
                        }
                    }
                    if domain.is_empty() || domain == "ai.onnx" {
                        opset = v;
                    }
                }
                _ => {}
            }
        }
        let Some(g) = graph else {
            return bad("no graph (not an ONNX model)");
        };
        let mut m = Model {
            opset,
            nodes: Vec::new(),
            initializers: HashMap::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        for f in Fields::new(g) {
            match f? {
                (1, Field::Bytes(b)) => m.nodes.push(node(b)?),
                (5, Field::Bytes(b)) => {
                    let (name, t) = tensor(b)?;
                    m.initializers.insert(name, t);
                }
                (11, Field::Bytes(b)) => m.inputs.push(value_info(b)?),
                (12, Field::Bytes(b)) => m.outputs.push(value_info(b)?),
                _ => {}
            }
        }
        // Inputs that are initializers are weights, not feeds.
        m.inputs.retain(|i| !m.initializers.contains_key(&i.name));
        if m.inputs.len() != 1 || m.outputs.is_empty() {
            return bad(format!(
                "{} inputs and {} outputs: one image input expected",
                m.inputs.len(),
                m.outputs.len()
            ));
        }
        Ok(m)
    }

    /// The op types used, with how often (sorted by name).
    pub fn op_counts(&self) -> Vec<(String, usize)> {
        let mut c = std::collections::BTreeMap::new();
        for n in &self.nodes {
            *c.entry(n.op.clone()).or_insert(0usize) += 1;
        }
        c.into_iter().collect()
    }

    /// Adds the network to `g`, reading `input` (NCHW) for the model's input; returns the
    /// values of its outputs, in order. Weights are stored as f16 when `f16` is set.
    pub fn build(&self, g: &mut Graph, input: Value, f16: bool) -> Result<Vec<Value>, Error> {
        Translator {
            m: self,
            g,
            values: HashMap::new(),
            ints: HashMap::new(),
            small: HashMap::new(),
            f16,
        }
        .run(input)
    }
}

struct Translator<'a> {
    m: &'a Model,
    g: &'a mut Graph,
    values: HashMap<String, Value>,
    /// Integer vectors known at build time (shapes and their arithmetic).
    ints: HashMap<String, Vec<i64>>,
    /// Small `Constant` outputs, as tensors.
    small: HashMap<String, Tensor>,
    f16: bool,
}

impl Translator<'_> {
    fn value(&mut self, name: &str) -> Result<Value, Error> {
        if let Some(&v) = self.values.get(name) {
            return Ok(v);
        }
        if let Some(t) = self.m.initializers.get(name).or(self.small.get(name)) {
            let v = self.g.constant(t.clone())?;
            self.values.insert(name.to_owned(), v);
            return Ok(v);
        }
        bad(format!("no value {name:?}"))
    }

    /// An integer vector known at build time: folded shape arithmetic or a constant.
    fn int_vec(&self, name: &str) -> Result<Vec<i64>, Error> {
        if let Some(v) = self.ints.get(name) {
            return Ok(v.clone());
        }
        match self.m.initializers.get(name) {
            Some(t) if t.len() <= MAX_RANK_INTS => {
                Ok(t.to_f32().iter().map(|&x| x as i64).collect())
            }
            _ => bad(format!("{name:?} must be a constant integer vector")),
        }
    }

    fn init(&self, name: &str) -> Result<&Tensor, Error> {
        match self.m.initializers.get(name) {
            Some(t) => Ok(t),
            None => bad(format!("{name:?} must be a constant")),
        }
    }

    /// A per-channel constant broadcast over NCHW (`[C]`, `[C,1,1]` or `[1,C,1,1]`) for a
    /// value of rank `rank`.
    fn channel_const(&mut self, t: &Tensor, rank: usize) -> Result<Value, Error> {
        let n = t.len();
        let mut t = t.clone();
        if rank == 4 && t.shape.len() < 3 && n > 1 {
            t = t.reshaped(&[n, 1, 1]);
        }
        let t = Tensor::f32(&t.shape.clone(), t.to_f32());
        self.g.constant(t)
    }

    fn readers(&self, name: &str) -> usize {
        self.m
            .nodes
            .iter()
            .flat_map(|n| &n.inputs)
            .filter(|i| *i == name)
            .count()
    }

    fn bn(&self, n: &Node) -> Result<BatchNorm, Error> {
        if n.inputs.len() != 5 {
            return bad("BatchNormalization wants 5 inputs");
        }
        Ok(BatchNorm {
            gamma: self.init(&n.inputs[1])?.to_f32(),
            beta: self.init(&n.inputs[2])?.to_f32(),
            mean: self.init(&n.inputs[3])?.to_f32(),
            var: self.init(&n.inputs[4])?.to_f32(),
            eps: n.float("epsilon", 1e-5),
        })
    }

    fn run(mut self, input: Value) -> Result<Vec<Value>, Error> {
        let inp = &self.m.inputs[0];
        let want = self.g.shape(input).to_vec();
        if inp.shape.len() != want.len()
            // A batch of 1 in the file takes any batch (as onnxruntime does not, but the
            // network is the same per image).
            || inp
                .shape
                .iter()
                .zip(&want)
                .enumerate()
                .any(|(k, (&a, &b))| a != 0 && a != b && !(k == 0 && a == 1))
        {
            return bad(format!(
                "input {:?} is {:?}, fed {want:?}",
                inp.name, inp.shape
            ));
        }
        self.values.insert(inp.name.clone(), input);
        let nodes = &self.m.nodes;
        let mut skip = vec![false; nodes.len()];
        for i in 0..nodes.len() {
            if skip[i] {
                continue;
            }
            let n = &nodes[i];
            if !n.domain.is_empty() && n.domain != "ai.onnx" {
                return bad(format!("op {} of domain {}", n.op, n.domain));
            }
            let Some(out) = n.outputs.first() else {
                return bad(format!("{} without output", n.op));
            };
            // Shape arithmetic, folded.
            match n.op.as_str() {
                "Constant" => {
                    let Some(Attr::Tensor(t)) = n.attr("value") else {
                        return bad("Constant without a tensor value");
                    };
                    if t.len() <= MAX_RANK_INTS {
                        // Small: folded as integers, made a graph constant only if a
                        // tensor op reads it (`value`).
                        let v = t.to_f32().iter().map(|&x| x as i64).collect();
                        self.ints.insert(out.clone(), v);
                        self.small.insert(out.clone(), t.clone());
                    } else {
                        let c = self.g.constant(Tensor::f32(&t.shape, t.to_f32()))?;
                        self.values.insert(out.clone(), c);
                    }
                    continue;
                }
                "Shape" => {
                    let x = self.value(&n.inputs[0])?;
                    let s = self.g.shape(x).iter().map(|&d| d as i64).collect();
                    self.ints.insert(out.clone(), s);
                    continue;
                }
                "Cast" if self.ints.contains_key(&n.inputs[0]) => {
                    let v = self.int_vec(&n.inputs[0])?;
                    self.ints.insert(out.clone(), v);
                    continue;
                }
                "Slice" if self.ints.contains_key(&n.inputs[0]) => {
                    let data = self.int_vec(&n.inputs[0])?;
                    let one = |k: usize, def: i64| -> Result<i64, Error> {
                        match n.inputs.get(k).filter(|s| !s.is_empty()) {
                            Some(name) => match self.int_vec(name)?.as_slice() {
                                [v] => Ok(*v),
                                v => bad(format!("Slice of a shape with {v:?}")),
                            },
                            None => Ok(def),
                        }
                    };
                    let len = data.len() as i64;
                    let (st, en, step) = (one(1, 0)?, one(2, len)?, one(4, 1)?);
                    if one(3, 0)? != 0 || step != 1 {
                        return bad("Slice of a shape on another axis or with a step");
                    }
                    let clamp = |v: i64| (if v < 0 { v + len } else { v }).clamp(0, len) as usize;
                    let (a, b) = (clamp(st), clamp(en));
                    self.ints.insert(out.clone(), data[a..b.max(a)].to_vec());
                    continue;
                }
                "Concat" if n.inputs.iter().all(|i| self.ints.contains_key(i)) => {
                    let mut v = Vec::new();
                    for i in &n.inputs {
                        v.extend(self.int_vec(i)?);
                    }
                    self.ints.insert(out.clone(), v);
                    continue;
                }
                _ => {}
            }
            let v = match n.op.as_str() {
                "Concat" => {
                    let xs = n
                        .inputs
                        .iter()
                        .map(|i| self.value(i))
                        .collect::<Result<Vec<_>, _>>()?;
                    let rank = self.g.shape(xs[0]).len() as i64;
                    let axis = n.int("axis", 0);
                    let axis = if axis < 0 { axis + rank } else { axis };
                    if !(0..rank).contains(&axis) {
                        return bad(format!("Concat axis {axis}"));
                    }
                    self.g.concat(&xs, axis as usize)?
                }
                "Resize" | "Upsample" => {
                    let x = self.value(&n.inputs[0])?;
                    let s = self.g.shape(x).to_vec();
                    if s.len() != 4 {
                        return bad("Resize of a non-NCHW value");
                    }
                    let sizes = n.inputs.get(3).filter(|i| !i.is_empty());
                    let scales = if n.op == "Upsample" {
                        n.inputs.get(1)
                    } else {
                        n.inputs.get(2)
                    }
                    .filter(|i| !i.is_empty());
                    let (h, w) = match (sizes, scales) {
                        (Some(sz), _) => match self.int_vec(sz)?.as_slice() {
                            &[a, b, h, w]
                                if a == s[0] as i64 && b == s[1] as i64 && h > 0 && w > 0 =>
                            {
                                (h as usize, w as usize)
                            }
                            v => return bad(format!("Resize sizes {v:?}")),
                        },
                        (None, Some(sc)) => {
                            let f = self.init(sc)?.to_f32();
                            match f.as_slice() {
                                &[1.0, 1.0, a, b] if a > 0.0 && b > 0.0 => (
                                    (s[2] as f32 * a).floor() as usize,
                                    (s[3] as f32 * b).floor() as usize,
                                ),
                                v => return bad(format!("Resize scales {v:?}")),
                            }
                        }
                        _ => return bad("Resize without sizes or scales"),
                    };
                    let mode = match n.attr("mode") {
                        Some(Attr::String(m)) => m.as_str(),
                        _ => "nearest",
                    };
                    let coord = match n.attr("coordinate_transformation_mode") {
                        Some(Attr::String(m)) => m.as_str(),
                        _ => "half_pixel",
                    };
                    let mode = match (mode, coord) {
                        ("linear" | "bilinear", "half_pixel" | "pytorch_half_pixel") => {
                            ResizeMode::Bilinear
                        }
                        ("linear" | "bilinear", "asymmetric") => ResizeMode::BilinearAsymmetric,
                        ("nearest", "asymmetric") => ResizeMode::Nearest,
                        (m, c) => return bad(format!("Resize {m} with {c} coordinates")),
                    };
                    if mode == ResizeMode::Nearest && !(h % s[2] == 0 && w % s[3] == 0) {
                        return bad("nearest Resize by a non-integer factor");
                    }
                    self.g.resize(x, h, w, mode)?
                }
                "Conv" => {
                    // A batch norm read only from this conv folds into it.
                    let next = nodes.get(i + 1).filter(|m| {
                        m.op == "BatchNormalization"
                            && m.inputs.first() == Some(out)
                            && self.readers(out) == 1
                    });
                    let w = self.init(&n.inputs[1])?;
                    let b = match n.inputs.get(2).filter(|s| !s.is_empty()) {
                        Some(b) => Some(self.init(b)?.clone()),
                        None => None,
                    };
                    let (w, b) = match next {
                        Some(bn) => {
                            let bn = self.bn(bn)?;
                            let (w, b) = crate::graph::fold_batchnorm(w, b.as_ref(), &bn)?;
                            (w, Some(b))
                        }
                        None => (w.clone(), b),
                    };
                    let k = n.ints("kernel_shape").unwrap_or(&[]);
                    if w.shape.len() != 4
                        || (!k.is_empty() && k != [w.shape[2] as i64, w.shape[3] as i64])
                    {
                        return bad(format!("Conv weights {:?}", w.shape));
                    }
                    if n.attr("auto_pad").is_some_and(
                        |a| !matches!(a, Attr::String(s) if s == "NOTSET" || s.is_empty()),
                    ) {
                        return bad("Conv auto_pad");
                    }
                    let two = |name: &str| -> Result<[usize; 2], Error> {
                        match n.ints(name) {
                            None => Ok([1, 1]),
                            Some(&[a, b]) if a > 0 && b > 0 => Ok([a as usize, b as usize]),
                            Some(v) => bad(format!("Conv {name} {v:?}")),
                        }
                    };
                    let pads = match n.ints("pads") {
                        None => [0; 4],
                        Some(&[t, l, b, r]) if t >= 0 && l >= 0 && b >= 0 && r >= 0 => {
                            [t as usize, l as usize, b as usize, r as usize]
                        }
                        Some(v) => return bad(format!("Conv pads {v:?}")),
                    };
                    let c = Conv2d {
                        stride: two("strides")?,
                        pad: pads,
                        dilation: two("dilations")?,
                        groups: n.int("group", 1).max(1) as usize,
                        act: Act::Identity,
                    };
                    if next.is_some() {
                        skip[i + 1] = true;
                    }
                    let x = self.value(&n.inputs[0])?;
                    let w = if self.f16 { w.to_f16() } else { w };
                    let wv = self.g.constant(w)?;
                    let bv = match b {
                        Some(b) => Some(self.g.constant(Tensor::f32(&b.shape, b.to_f32()))?),
                        None => None,
                    };
                    let v = self.g.conv2d(x, wv, bv, c)?;
                    if let Some(bn) = next {
                        self.values.insert(bn.outputs[0].clone(), v);
                    }
                    v
                }
                "BatchNormalization" => {
                    let bn = self.bn(n)?;
                    let x = self.value(&n.inputs[0])?;
                    self.g.batch_norm(x, &bn)?
                }
                "PRelu" | "Add" | "Sub" | "Mul" | "Div" => {
                    let op = match n.op.as_str() {
                        "PRelu" => BinaryOp::Prelu,
                        "Add" => BinaryOp::Add,
                        "Sub" => BinaryOp::Sub,
                        "Mul" => BinaryOp::Mul,
                        _ => BinaryOp::Div,
                    };
                    let a = self.value(&n.inputs[0])?;
                    let rank = self.g.shape(a).len();
                    let b = match self.m.initializers.get(&n.inputs[1]) {
                        Some(t) if op == BinaryOp::Prelu || t.len() > 1 => {
                            let t = t.clone();
                            self.channel_const(&t, rank)?
                        }
                        Some(t) => {
                            let t = Tensor::f32(&t.shape, t.to_f32());
                            self.g.constant(t)?
                        }
                        None => self.value(&n.inputs[1])?,
                    };
                    self.g.binary(op, a, b)?
                }
                "Relu" => {
                    let x = self.value(&n.inputs[0])?;
                    self.g.act(x, Act::Relu)?
                }
                "Sigmoid" => {
                    let x = self.value(&n.inputs[0])?;
                    self.g.act(x, Act::Sigmoid)?
                }
                "Dropout" | "Identity" => self.value(&n.inputs[0])?,
                "Flatten" => {
                    let x = self.value(&n.inputs[0])?;
                    let s = self.g.shape(x).to_vec();
                    let axis = n.int("axis", 1);
                    if axis < 0 || axis as usize > s.len() {
                        return bad(format!("Flatten axis {axis}"));
                    }
                    let a = axis as usize;
                    let shape = [s[..a].iter().product(), s[a..].iter().product()];
                    self.g.reshape(x, &shape)?
                }
                "Reshape" => {
                    let x = self.value(&n.inputs[0])?;
                    let s = self.g.shape(x).to_vec();
                    let mut want = self.init(&n.inputs[1])?.to_f32();
                    // Exports traced at batch 1 often reshape to [1, -1]: with a batch of N
                    // fed, the leading 1 is the batch.
                    if s.len() > 1
                        && s[0] > 1
                        && want.len() > 1
                        && want[0] == 1.0
                        && want.contains(&-1.0)
                    {
                        want[0] = 0.0;
                    }
                    let total: usize = s.iter().product();
                    let mut shape: Vec<usize> = Vec::new();
                    let mut infer = None;
                    for (k, &d) in want.iter().enumerate() {
                        let d = d as i64;
                        match d {
                            0 => shape.push(*s.get(k).unwrap_or(&1)),
                            -1 => {
                                infer = Some(k);
                                shape.push(1);
                            }
                            d if d > 0 => shape.push(d as usize),
                            _ => return bad("Reshape shape"),
                        }
                    }
                    if let Some(k) = infer {
                        let rest: usize = shape.iter().product();
                        if rest == 0 || !total.is_multiple_of(rest) {
                            return bad("Reshape shape");
                        }
                        shape[k] = total / rest;
                    }
                    self.g.reshape(x, &shape)?
                }
                "Gemm" | "MatMul" => {
                    let a = self.value(&n.inputs[0])?;
                    let trans_b = n.op == "Gemm" && n.int("transB", 0) == 1;
                    if n.op == "Gemm"
                        && (n.int("transA", 0) != 0
                            || n.float("alpha", 1.0) != 1.0
                            || n.float("beta", 1.0) != 1.0)
                    {
                        return bad("Gemm with transA, alpha or beta");
                    }
                    let w = self.init(&n.inputs[1])?.clone();
                    let w = if self.f16 { w.to_f16() } else { w };
                    let wv = self.g.constant(w)?;
                    let bv = match n.inputs.get(2).filter(|s| !s.is_empty()) {
                        Some(b) => {
                            let t = self.init(b)?;
                            let len = t.len();
                            Some(self.g.constant(Tensor::f32(&[len], t.to_f32()))?)
                        }
                        None => None,
                    };
                    self.g.matmul(
                        a,
                        wv,
                        bv,
                        MatMul {
                            trans_b,
                            act: Act::Identity,
                        },
                    )?
                }
                "MaxPool" | "AveragePool" => {
                    let x = self.value(&n.inputs[0])?;
                    let k = match n.ints("kernel_shape") {
                        Some(&[a, b]) if a > 0 && b > 0 => [a as usize, b as usize],
                        _ => return bad("pool kernel"),
                    };
                    let s = match n.ints("strides") {
                        Some(&[a, b]) if a > 0 && b > 0 => [a as usize, b as usize],
                        None => [1, 1],
                        _ => return bad("pool strides"),
                    };
                    let mut pad = match n.ints("pads") {
                        Some(&[t, l, b, r]) if t >= 0 && l >= 0 && b >= 0 && r >= 0 => {
                            [t as usize, l as usize, b as usize, r as usize]
                        }
                        None => [0; 4],
                        _ => return bad("pool pads"),
                    };
                    if n.int("ceil_mode", 0) == 1 {
                        // Extra bottom/right padding so a partial last window counts (it
                        // must start inside the input or its left padding, as PyTorch
                        // requires).
                        let xs = self.g.shape(x).to_vec();
                        for d in 0..2 {
                            let span = xs[2 + d] + pad[d] + pad[d + 2];
                            if span < k[d] {
                                return bad("pool window larger than the input");
                            }
                            let mut o = (span - k[d]).div_ceil(s[d]) + 1;
                            if (o - 1) * s[d] >= xs[2 + d] + pad[d] {
                                o -= 1;
                            }
                            pad[d + 2] += ((o - 1) * s[d] + k[d]).saturating_sub(span);
                        }
                    }
                    let kind = if n.op == "MaxPool" {
                        PoolKind::Max
                    } else {
                        PoolKind::Avg
                    };
                    self.g.pool2d(
                        x,
                        Pool2d {
                            kind,
                            kernel: k,
                            stride: s,
                            pad,
                        },
                    )?
                }
                "GlobalAveragePool" => {
                    let x = self.value(&n.inputs[0])?;
                    self.g.global_avg_pool(x)?
                }
                op => return bad(format!("unsupported op {op}")),
            };
            self.values.insert(out.clone(), v);
        }
        self.m.outputs.iter().map(|o| self.value(&o.name)).collect()
    }
}
