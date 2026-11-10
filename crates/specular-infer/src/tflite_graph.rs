//! A TensorFlow Lite model ([`crate::tflite::Model`]) as a [`Graph`].
//!
//! TFLite feature maps are NHWC; ours are NCHW. Each rank-4 tensor is kept in whichever
//! layout its producer gave (convolutions and pools give NCHW) and converted only where an
//! op needs the other one (a reshape or fully connected layer reads NHWC order). Constants
//! are permuted on the host. float16 weights followed by `DEQUANTIZE` stay f16 constants.
//! Spatial `PAD`s read only by convolutions fold into the convolution's padding.
//!
//! The op set is what MediaPipe's face models use (BlazeFace, FaceMesh V2, Blendshape V2)
//! plus close relatives; anything else is refused by name.

use std::collections::HashMap;

use crate::Error;
use crate::graph::{
    Act, BinaryOp, Conv2d, Graph, MatMul, Pool2d, PoolKind, ReduceOp, ResizeMode, Value,
};
use crate::tensor::{Tensor, strides};
use crate::tflite::{Model, Operator, TensorType, code};

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(format!("tflite: {}", s.into())))
}

/// How a rank-4 value is stored in the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// The file's shape and order (NHWC for feature maps, or any rank ≠ 4).
    Plain,
    /// NCHW: the graph value is the file's NHWC tensor transposed.
    Nchw,
}

/// NHWC → NCHW.
const TO_NCHW: [usize; 4] = [0, 3, 1, 2];
/// NCHW → NHWC.
const TO_NHWC: [usize; 4] = [0, 2, 3, 1];

/// Permutes a host tensor's axes.
pub fn permute(t: &Tensor, perm: &[usize]) -> Tensor {
    let src = t.to_f32();
    let shape: Vec<usize> = perm.iter().map(|&p| t.shape[p]).collect();
    let st = strides(&t.shape);
    let n = src.len();
    let mut out = Vec::with_capacity(n);
    let r = shape.len();
    let mut idx = vec![0usize; r];
    for _ in 0..n {
        let off: usize = (0..r).map(|d| idx[d] * st[perm[d]]).sum();
        out.push(src[off]);
        for d in (0..r).rev() {
            idx[d] += 1;
            if idx[d] < shape[d] {
                break;
            }
            idx[d] = 0;
        }
    }
    let p = Tensor::f32(&shape, out);
    if t.dtype() == crate::DType::F16 {
        p.to_f16()
    } else {
        p
    }
}

fn act_of(code: u8) -> Result<Act, Error> {
    Ok(match code {
        0 => Act::Identity,
        1 => Act::Relu,
        3 => Act::Relu6,
        4 => Act::Tanh,
        c => return bad(format!("fused activation {c}")),
    })
}

/// TensorFlow's padding for `SAME` (before, after), or zero for `VALID`.
fn same_pad(same: bool, size: usize, k: usize, stride: usize, dil: usize) -> (usize, usize) {
    if !same {
        return (0, 0);
    }
    let out = size.div_ceil(stride);
    let eff = (k - 1) * dil + 1;
    let total = ((out - 1) * stride + eff).saturating_sub(size);
    (total / 2, total - total / 2)
}

struct Tr<'a, 'm> {
    m: &'a Model<'m>,
    g: &'a mut Graph,
    vals: HashMap<usize, (Value, Layout)>,
    /// A spatial pad not yet applied: (value before the pad, layout, [t, l, b, r]).
    pads: HashMap<usize, (Value, [usize; 4])>,
    /// Tensors that are dequantized constants: the tensor holding the data.
    alias: HashMap<usize, usize>,
    /// How many ops read each tensor.
    readers: Vec<Vec<i32>>,
    f16: bool,
}

/// Adds the model to `g`, its (single) image input read from `input`, an NCHW value of the
/// input's shape. Returns the model's outputs in the file's shapes and order.
/// Weights are stored as f16 when `f16` is set (or when they are f16 in the file).
pub fn build(m: &Model, g: &mut Graph, input: Value, f16: bool) -> Result<Vec<Value>, Error> {
    let mut readers = vec![Vec::new(); m.tensors.len()];
    for op in &m.operators {
        for &i in op.inputs.iter().filter(|&&i| i >= 0) {
            readers[i as usize].push(op.code);
        }
    }
    let mut t = Tr {
        m,
        g,
        vals: HashMap::new(),
        pads: HashMap::new(),
        alias: HashMap::new(),
        readers,
        f16,
    };
    let [inp] = m.inputs[..] else {
        return bad(format!("{} inputs: one expected", m.inputs.len()));
    };
    let shape = &m.tensors[inp].shape;
    let fed = t.g.shape(input).to_vec();
    let layout = if shape.len() == 4 && fed == [shape[0], shape[3], shape[1], shape[2]] {
        Layout::Nchw
    } else if fed == *shape {
        Layout::Plain
    } else {
        return bad(format!("input is {shape:?}, fed {fed:?}"));
    };
    t.vals.insert(inp, (input, layout));
    for op in &m.operators {
        t.op(op)?;
    }
    let outs = m.outputs.clone();
    outs.iter().map(|&o| t.plain(o)).collect()
}

impl Tr<'_, '_> {
    fn shape(&self, t: usize) -> &[usize] {
        &self.m.tensors[t].shape
    }

    fn input(&self, op: &Operator, k: usize) -> Result<usize, Error> {
        match op.inputs.get(k) {
            Some(&i) if i >= 0 => Ok(i as usize),
            _ => bad(format!("op {} lacks input {k}", op.code)),
        }
    }

    fn is_const(&self, t: usize) -> bool {
        self.alias.contains_key(&t) || self.m.is_constant(t)
    }

    /// Constant data of `t` (following a dequantize), in the file's layout.
    fn konst(&self, t: usize) -> Result<Tensor, Error> {
        let src = *self.alias.get(&t).unwrap_or(&t);
        if !self.m.is_constant(src) {
            return bad(format!("{:?} must be a constant", self.m.tensors[t].name));
        }
        self.m.constant(src)
    }

    fn ints(&self, t: usize) -> Result<Vec<i64>, Error> {
        let src = *self.alias.get(&t).unwrap_or(&t);
        self.m.ints(src)
    }

    fn weight(&mut self, t: Tensor) -> Result<Value, Error> {
        let t = if self.f16 { t.to_f16() } else { t };
        self.g.constant(t)
    }

    fn f32_const(&mut self, t: Tensor) -> Result<Value, Error> {
        let shape = t.shape.clone();
        self.g.constant(Tensor::f32(&shape, t.to_f32()))
    }

    /// `t` in whatever layout it has (constants plain), any pending pad applied.
    fn get(&mut self, t: usize) -> Result<(Value, Layout), Error> {
        if let Some(&v) = self.vals.get(&t) {
            return Ok(v);
        }
        if let Some(&(x, p)) = self.pads.get(&t) {
            let v = self.materialize_pad(x, p, 0.0)?;
            self.vals.insert(t, (v, Layout::Nchw));
            return Ok((v, Layout::Nchw));
        }
        if self.is_const(t) {
            let k = self.konst(t)?;
            let v = self.f32_const(k)?;
            self.vals.insert(t, (v, Layout::Plain));
            return Ok((v, Layout::Plain));
        }
        bad(format!(
            "tensor {:?} is read before it is computed",
            self.m.tensors[t].name
        ))
    }

    /// `t` in the file's layout.
    fn plain(&mut self, t: usize) -> Result<Value, Error> {
        match self.get(t)? {
            (v, Layout::Plain) => Ok(v),
            (v, Layout::Nchw) => self.g.transpose(v, &TO_NHWC),
        }
    }

    /// Rank-4 `t` as NCHW.
    fn nchw(&mut self, t: usize) -> Result<Value, Error> {
        match self.get(t)? {
            (v, Layout::Nchw) => Ok(v),
            (v, Layout::Plain) if self.g.shape(v).len() == 4 => self.g.transpose(v, &TO_NCHW),
            _ => bad(format!("{:?} is not a feature map", self.m.tensors[t].name)),
        }
    }

    fn materialize_pad(&mut self, x: Value, p: [usize; 4], value: f32) -> Result<Value, Error> {
        let mut v = x;
        // H then W: concat constant strips.
        for (axis, before, after) in [(2usize, p[0], p[2]), (3, p[1], p[3])] {
            let mut parts = Vec::new();
            let s = self.g.shape(v).to_vec();
            let strip = |g: &mut Graph, n: usize| -> Result<Value, Error> {
                let mut sh = s.clone();
                sh[axis] = n;
                let len: usize = sh.iter().product();
                g.constant(Tensor::f32(&sh, vec![value; len]))
            };
            if before > 0 {
                parts.push(strip(self.g, before)?);
            }
            parts.push(v);
            if after > 0 {
                parts.push(strip(self.g, after)?);
            }
            if parts.len() > 1 {
                v = self.g.concat(&parts, axis)?;
            }
        }
        Ok(v)
    }

    fn set(&mut self, op: &Operator, v: Value, l: Layout) -> Result<(), Error> {
        let Some(&o) = op.outputs.first() else {
            return bad("op without output");
        };
        let o = o as usize;
        let want = self.shape(o).to_vec();
        let got = self.g.shape(v).to_vec();
        let ok = match l {
            Layout::Plain => got == want || want.is_empty(),
            Layout::Nchw => want.len() == 4 && got == [want[0], want[3], want[1], want[2]],
        };
        if !ok {
            return bad(format!(
                "op {} gives {got:?} ({l:?}), the file says {want:?}",
                op.code
            ));
        }
        self.vals.insert(o, (v, l));
        Ok(())
    }

    /// A binary op's operand converted to `layout` (constants permuted on the host).
    fn operand(&mut self, t: usize, layout: Layout, rank: usize) -> Result<Value, Error> {
        if self.is_const(t) {
            let k = self.konst(t)?;
            if layout == Layout::Nchw && rank == 4 {
                let mut shape = k.shape.clone();
                while shape.len() < 4 {
                    shape.insert(0, 1);
                }
                let k = permute(&k.reshaped(&shape), &TO_NCHW);
                return self.f32_const(k);
            }
            return self.f32_const(k);
        }
        match layout {
            Layout::Nchw => self.nchw(t),
            Layout::Plain => self.plain(t),
        }
    }

    fn binary(&mut self, op: &Operator, b: BinaryOp, act: Act) -> Result<(), Error> {
        let (x, y) = (self.input(op, 0)?, self.input(op, 1)?);
        let lx = (!self.is_const(x))
            .then(|| self.get(x))
            .transpose()?
            .map(|v| v.1);
        let ly = (!self.is_const(y))
            .then(|| self.get(y))
            .transpose()?
            .map(|v| v.1);
        let rank = self.shape(op.outputs[0] as usize).len();
        let layout = if rank == 4 && (lx == Some(Layout::Nchw) || ly == Some(Layout::Nchw)) {
            Layout::Nchw
        } else {
            Layout::Plain
        };
        let a = self.operand(x, layout, rank)?;
        let c = self.operand(y, layout, rank)?;
        let mut v = self.g.binary(b, a, c)?;
        if act != Act::Identity {
            v = self.g.act(v, act)?;
        }
        self.set(op, v, layout)
    }

    fn unary(&mut self, op: &Operator, act: Act, mul: f32, add: f32) -> Result<(), Error> {
        let (x, l) = self.get(self.input(op, 0)?)?;
        let v = self.g.unary(x, act, mul, add)?;
        self.set(op, v, l)
    }

    /// Maps an axis of the file's rank-4 tensor to the NCHW value's.
    fn axis(l: Layout, rank: usize, a: i64) -> Result<usize, Error> {
        let a = if a < 0 { a + rank as i64 } else { a };
        if a < 0 || a as usize >= rank {
            return bad(format!("axis {a}"));
        }
        let a = a as usize;
        Ok(if l == Layout::Nchw && rank == 4 {
            [0, 2, 3, 1][a]
        } else {
            a
        })
    }

    fn conv(&mut self, op: &Operator, depthwise: bool) -> Result<(), Error> {
        let m = self.m;
        let o = op.options;
        let same = m.opt_u8(o, 0, 0)? == 0;
        let (sw, sh) = (m.opt_i32(o, 1, 1)?, m.opt_i32(o, 2, 1)?);
        let (act_f, dw_f, dh_f) = if depthwise { (4, 5, 6) } else { (3, 4, 5) };
        let act = act_of(m.opt_u8(o, act_f, 0)?)?;
        let (dw, dh) = (m.opt_i32(o, dw_f, 1)?, m.opt_i32(o, dh_f, 1)?);
        if sw < 1 || sh < 1 || dw < 1 || dh < 1 {
            return bad("conv strides");
        }
        let (sw, sh, dw, dh) = (sw as usize, sh as usize, dw as usize, dh as usize);
        let xi = self.input(op, 0)?;
        let wi = self.input(op, 1)?;
        let w = self.konst(wi)?;
        if w.shape.len() != 4 {
            return bad(format!("conv weights {:?}", w.shape));
        }
        let xs = self.shape(xi).to_vec();
        if xs.len() != 4 {
            return bad("conv input rank");
        }
        // [O, KH, KW, I] or depthwise [1, KH, KW, O] → [O, I/groups, KH, KW].
        let (w, groups) = if depthwise {
            let w = permute(&w, &[3, 0, 1, 2]);
            (w, xs[3])
        } else {
            (permute(&w, &[0, 3, 1, 2]), 1)
        };
        let (kh, kw) = (w.shape[2], w.shape[3]);
        let (pt, pb) = same_pad(same, xs[1], kh, sh, dh);
        let (pl, pr) = same_pad(same, xs[2], kw, sw, dw);
        let (x, extra) = match self.pads.get(&xi) {
            Some(&(x, p)) if !self.vals.contains_key(&xi) => (x, p),
            _ => (self.nchw(xi)?, [0; 4]),
        };
        let wv = self.weight(w)?;
        let bv = match op.inputs.get(2) {
            Some(&b) if b >= 0 => {
                let k = self.konst(b as usize)?;
                Some(self.f32_const(k)?)
            }
            _ => None,
        };
        let c = Conv2d {
            stride: [sh, sw],
            pad: [pt + extra[0], pl + extra[1], pb + extra[2], pr + extra[3]],
            dilation: [dh, dw],
            groups,
            act,
        };
        let v = self.g.conv2d(x, wv, bv, c)?;
        self.set(op, v, Layout::Nchw)
    }

    fn pool(&mut self, op: &Operator, kind: PoolKind) -> Result<(), Error> {
        let m = self.m;
        let o = op.options;
        let same = m.opt_u8(o, 0, 0)? == 0;
        let (sw, sh) = (m.opt_i32(o, 1, 1)?, m.opt_i32(o, 2, 1)?);
        let (fw, fh) = (m.opt_i32(o, 3, 1)?, m.opt_i32(o, 4, 1)?);
        let act = act_of(m.opt_u8(o, 5, 0)?)?;
        if sw < 1 || sh < 1 || fw < 1 || fh < 1 {
            return bad("pool sizes");
        }
        let xi = self.input(op, 0)?;
        let xs = self.shape(xi).to_vec();
        let (pt, pb) = same_pad(same, xs[1], fh as usize, sh as usize, 1);
        let (pl, pr) = same_pad(same, xs[2], fw as usize, sw as usize, 1);
        let x = self.nchw(xi)?;
        let mut v = self.g.pool2d(
            x,
            Pool2d {
                kind,
                kernel: [fh as usize, fw as usize],
                stride: [sh as usize, sw as usize],
                pad: [pt, pl, pb, pr],
            },
        )?;
        if act != Act::Identity {
            v = self.g.act(v, act)?;
        }
        self.set(op, v, Layout::Nchw)
    }

    fn pad(&mut self, op: &Operator) -> Result<(), Error> {
        let xi = self.input(op, 0)?;
        let p = self.ints(self.input(op, 1)?)?;
        let value = match op.inputs.get(2) {
            Some(&c) if c >= 0 && op.code == code::PADV2 => self
                .konst(c as usize)?
                .to_f32()
                .first()
                .copied()
                .unwrap_or(0.0),
            _ => 0.0,
        };
        let rank = self.shape(xi).len();
        if p.len() != 2 * rank || p.iter().any(|&v| v < 0) {
            return bad("pad amounts");
        }
        let p: Vec<usize> = p.iter().map(|&v| v as usize).collect();
        let out = op.outputs[0] as usize;
        if rank != 4 || p[0] != 0 || p[1] != 0 {
            return bad("pad of a non-feature map");
        }
        let spatial = [p[2], p[4], p[3], p[5]];
        let (cb, ca) = (p[6], p[7]);
        let x = self.nchw(xi)?;
        // Read only by convolutions with zero padding: fold into them.
        let conv_only = self.readers[out]
            .iter()
            .all(|&c| c == code::CONV_2D || c == code::DEPTHWISE_CONV_2D);
        if cb == 0 && ca == 0 && value == 0.0 && conv_only && !self.m.outputs.contains(&out) {
            self.pads.insert(out, (x, spatial));
            return Ok(());
        }
        let mut v = if spatial != [0; 4] {
            self.materialize_pad(x, spatial, value)?
        } else {
            x
        };
        if cb > 0 || ca > 0 {
            let s = self.g.shape(v).to_vec();
            let mut parts = Vec::new();
            for (n, first) in [(cb, true), (ca, false)] {
                if n == 0 {
                    continue;
                }
                let sh = [s[0], n, s[2], s[3]];
                let len: usize = sh.iter().product();
                let c = self.g.constant(Tensor::f32(&sh, vec![value; len]))?;
                if first {
                    parts.insert(0, c);
                } else {
                    parts.push(c);
                }
            }
            let at = if cb > 0 { 1 } else { 0 };
            parts.insert(at, v);
            v = self.g.concat(&parts, 1)?;
        }
        self.set(op, v, Layout::Nchw)
    }

    fn reshape(&mut self, op: &Operator) -> Result<(), Error> {
        let xi = self.input(op, 0)?;
        let x = self.plain(xi)?;
        let want = self.shape(op.outputs[0] as usize).to_vec();
        let v = self.g.reshape(x, &want)?;
        self.set(op, v, Layout::Plain)
    }

    fn concat(&mut self, op: &Operator) -> Result<(), Error> {
        let m = self.m;
        let axis = m.opt_i32(op.options, 0, 0)? as i64;
        let act = act_of(m.opt_u8(op.options, 1, 0)?)?;
        let rank = self.shape(op.outputs[0] as usize).len();
        let ins: Vec<usize> = op
            .inputs
            .iter()
            .filter(|&&i| i >= 0)
            .map(|&i| i as usize)
            .collect();
        let mut nchw = rank == 4;
        for &i in &ins {
            if !self.is_const(i) && self.get(i)?.1 == Layout::Plain {
                nchw = false;
            }
        }
        let layout = if nchw { Layout::Nchw } else { Layout::Plain };
        let mut vs = Vec::new();
        for &i in &ins {
            vs.push(self.operand(i, layout, rank)?);
        }
        let a = Self::axis(layout, rank, axis)?;
        let mut v = self.g.concat(&vs, a)?;
        if act != Act::Identity {
            v = self.g.act(v, act)?;
        }
        self.set(op, v, layout)
    }

    fn fully_connected(&mut self, op: &Operator) -> Result<(), Error> {
        let m = self.m;
        let act = act_of(m.opt_u8(op.options, 0, 0)?)?;
        if m.opt_u8(op.options, 1, 0)? != 0 {
            return bad("shuffled fully connected weights");
        }
        let xi = self.input(op, 0)?;
        let w = self.konst(self.input(op, 1)?)?;
        if w.shape.len() != 2 {
            return bad("fully connected weights");
        }
        let x = self.plain(xi)?;
        let s = self.g.shape(x).to_vec();
        let total: usize = s.iter().product();
        let inner = w.shape[1];
        if inner == 0 || !total.is_multiple_of(inner) {
            return bad("fully connected input size");
        }
        let x = if s.last() == Some(&inner) {
            x
        } else {
            self.g.reshape(x, &[total / inner, inner])?
        };
        let wv = self.weight(w)?;
        let bv = match op.inputs.get(2) {
            Some(&b) if b >= 0 => {
                let k = self.konst(b as usize)?;
                Some(self.f32_const(k)?)
            }
            _ => None,
        };
        let v = self.g.matmul(x, wv, bv, MatMul { trans_b: true, act })?;
        let want = self.shape(op.outputs[0] as usize).to_vec();
        let v = if self.g.shape(v) == want.as_slice() {
            v
        } else {
            self.g.reshape(v, &want)?
        };
        self.set(op, v, Layout::Plain)
    }

    fn reduce(&mut self, op: &Operator, r: ReduceOp) -> Result<(), Error> {
        let xi = self.input(op, 0)?;
        let axes = self.ints(self.input(op, 1)?)?;
        let keep = self.m.opt_u8(op.options, 0, 0)? != 0;
        let (mut v, l) = self.get(xi)?;
        let rank = self.shape(xi).len();
        let mut ax: Vec<usize> = axes
            .iter()
            .map(|&a| Self::axis(l, rank, a))
            .collect::<Result<_, _>>()?;
        ax.sort_unstable();
        ax.dedup();
        for &a in &ax {
            v = self.g.reduce(v, r, a)?;
        }
        if keep {
            return self.set(op, v, l);
        }
        // Dropping axes: go back to the file's order first.
        if l == Layout::Nchw {
            v = self.g.transpose(v, &TO_NHWC)?;
        }
        let want = self.shape(op.outputs[0] as usize).to_vec();
        let v = self.g.reshape(v, &want)?;
        self.set(op, v, Layout::Plain)
    }

    fn transpose(&mut self, op: &Operator) -> Result<(), Error> {
        let xi = self.input(op, 0)?;
        let perm = self.ints(self.input(op, 1)?)?;
        let rank = self.shape(xi).len();
        if perm.len() != rank || perm.iter().any(|&p| p < 0 || p as usize >= rank) {
            return bad("transpose permutation");
        }
        let perm: Vec<usize> = perm.iter().map(|&p| p as usize).collect();
        let x = self.plain(xi)?;
        let v = self.g.transpose(x, &perm)?;
        self.set(op, v, Layout::Plain)
    }

    fn strided_slice(&mut self, op: &Operator) -> Result<(), Error> {
        let m = self.m;
        let o = op.options;
        let (bm, em) = (m.opt_i32(o, 0, 0)?, m.opt_i32(o, 1, 0)?);
        let (ell, newax, shrink) = (
            m.opt_i32(o, 2, 0)?,
            m.opt_i32(o, 3, 0)?,
            m.opt_i32(o, 4, 0)?,
        );
        if ell != 0 || newax != 0 || m.opt_u8(o, 5, 0)? != 0 {
            return bad("strided slice masks");
        }
        let xi = self.input(op, 0)?;
        let begin = self.ints(self.input(op, 1)?)?;
        let end = self.ints(self.input(op, 2)?)?;
        let step = self.ints(self.input(op, 3)?)?;
        let s = self.shape(xi).to_vec();
        let r = s.len();
        if begin.len() != r || end.len() != r || step.len() != r {
            return bad("strided slice sizes");
        }
        let (mut st, mut en, mut sp) = (vec![0; r], vec![0; r], vec![1; r]);
        for d in 0..r {
            if step[d] < 1 {
                return bad("strided slice step");
            }
            let n = s[d] as i64;
            let clamp = |v: i64| (if v < 0 { v + n } else { v }).clamp(0, n) as usize;
            st[d] = if bm >> d & 1 == 1 { 0 } else { clamp(begin[d]) };
            en[d] = if em >> d & 1 == 1 {
                s[d]
            } else if shrink >> d & 1 == 1 {
                st[d] + 1
            } else {
                clamp(end[d])
            };
            en[d] = en[d].max(st[d]);
            sp[d] = step[d] as usize;
        }
        let x = self.plain(xi)?;
        let v = self.g.slice(x, &st, &en, &sp)?;
        let want = self.shape(op.outputs[0] as usize).to_vec();
        let v = if self.g.shape(v) == want.as_slice() {
            v
        } else {
            self.g.reshape(v, &want)?
        };
        self.set(op, v, Layout::Plain)
    }

    fn resize(&mut self, op: &Operator) -> Result<(), Error> {
        let m = self.m;
        let align = m.opt_u8(op.options, 2, 0)? != 0;
        let half = m.opt_u8(op.options, 3, 0)? != 0;
        let xi = self.input(op, 0)?;
        let size = self.ints(self.input(op, 1)?)?;
        let [h, w] = size[..] else {
            return bad("resize size");
        };
        if h < 1 || w < 1 {
            return bad("resize size");
        }
        let xs = self.shape(xi).to_vec();
        let mode = if half {
            ResizeMode::Bilinear
        } else if align {
            return bad("resize with aligned corners");
        } else if (h as usize).is_multiple_of(xs[1]) && (w as usize).is_multiple_of(xs[2]) {
            // Legacy TF sampling (src = dst · in/out, no half-pixel offset) for an integer
            // upscale.
            ResizeMode::BilinearAsymmetric
        } else {
            return bad("resize without half-pixel centres");
        };
        let x = self.nchw(xi)?;
        let v = self.g.resize(x, h as usize, w as usize, mode)?;
        self.set(op, v, Layout::Nchw)
    }

    fn op(&mut self, op: &Operator) -> Result<(), Error> {
        let m = self.m;
        let c = op.code;
        let act0 = || -> Result<Act, Error> { act_of(m.opt_u8(op.options, 0, 0)?) };
        match c {
            code::DEQUANTIZE => {
                let x = self.input(op, 0)?;
                let out = op.outputs[0] as usize;
                if self.m.is_constant(x) {
                    let t = &self.m.tensors[x];
                    if !matches!(t.ty, TensorType::F16 | TensorType::F32) {
                        return bad("dequantize of an integer tensor");
                    }
                    self.alias.insert(out, x);
                    Ok(())
                } else {
                    let (v, l) = self.get(x)?;
                    self.set(op, v, l)
                }
            }
            code::CONV_2D => self.conv(op, false),
            code::DEPTHWISE_CONV_2D => self.conv(op, true),
            code::MAX_POOL_2D => self.pool(op, PoolKind::Max),
            code::AVERAGE_POOL_2D => self.pool(op, PoolKind::Avg),
            code::ADD => self.binary(op, BinaryOp::Add, act0()?),
            code::SUB => self.binary(op, BinaryOp::Sub, act0()?),
            code::MUL => self.binary(op, BinaryOp::Mul, act0()?),
            code::DIV => self.binary(op, BinaryOp::Div, act0()?),
            code::MAXIMUM => self.binary(op, BinaryOp::Max, Act::Identity),
            code::MINIMUM => self.binary(op, BinaryOp::Min, Act::Identity),
            code::SQUARED_DIFFERENCE => self.binary(op, BinaryOp::SquaredDiff, Act::Identity),
            code::PRELU => self.binary(op, BinaryOp::Prelu, Act::Identity),
            code::RELU => self.unary(op, Act::Relu, 1.0, 0.0),
            code::RELU6 => self.unary(op, Act::Relu6, 1.0, 0.0),
            code::LOGISTIC => self.unary(op, Act::Sigmoid, 1.0, 0.0),
            code::TANH => self.unary(op, Act::Tanh, 1.0, 0.0),
            code::HARD_SWISH => self.unary(op, Act::HardSwish, 1.0, 0.0),
            code::SQRT => self.unary(op, Act::Sqrt, 1.0, 0.0),
            code::RSQRT => self.unary(op, Act::Rsqrt, 1.0, 0.0),
            code::NEG => self.unary(op, Act::Identity, -1.0, 0.0),
            code::GELU => {
                let approx = m.opt_u8(op.options, 0, 0)? != 0;
                self.unary(op, if approx { Act::GeluTanh } else { Act::Gelu }, 1.0, 0.0)
            }
            code::PAD | code::PADV2 => self.pad(op),
            code::RESHAPE | code::SQUEEZE | code::EXPAND_DIMS => self.reshape(op),
            code::CONCATENATION => self.concat(op),
            code::FULLY_CONNECTED => self.fully_connected(op),
            code::MEAN => self.reduce(op, ReduceOp::Mean),
            code::SUM => self.reduce(op, ReduceOp::Sum),
            code::TRANSPOSE => self.transpose(op),
            code::STRIDED_SLICE => self.strided_slice(op),
            code::RESIZE_BILINEAR => self.resize(op),
            code::SOFTMAX => {
                let x = self.plain(self.input(op, 0)?)?;
                let v = self.g.softmax(x)?;
                self.set(op, v, Layout::Plain)
            }
            -1 => bad(format!(
                "custom op {}",
                op.custom.as_deref().unwrap_or("(unnamed)")
            )),
            c => bad(format!("unsupported op {c}")),
        }
    }
}
