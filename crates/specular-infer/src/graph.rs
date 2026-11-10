//! The graph: inputs, constants (weights) and op nodes, with shapes inferred and checked as
//! it is built. Building never panics on a bad shape (weights come from files): every op
//! returns an [`Error`] instead.
//!
//! Every op's output is f32 except [`Op::Reshape`] (the input's type) and [`Op::CastF16`].
//!
//! Layouts: images and feature maps are NCHW, token sequences `[N, T, D]`, linear weights
//! `[out, in]` (PyTorch's) or `[in, out]`, conv weights `[O, C/groups, KH, KW]`.

use crate::Error;
use crate::tensor::{DType, Tensor, numel};

/// Highest tensor rank an op accepts.
pub const MAX_RANK: usize = 6;
/// Most states an [`Op::Lstm`] has (the GPU kernel keeps them in workgroup memory).
pub const LSTM_MAX_STATES: usize = 1024;
/// Largest number of elements of any value: GPU indices are u32.
pub const MAX_ELEMENTS: usize = (1 << 31) - 1;

/// A value in a graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Value(pub(crate) u32);

impl Value {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An activation, applied elementwise (also fused into conv and matmul).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    Identity,
    Relu,
    /// min(max(x, 0), 6).
    Relu6,
    /// x·Φ(x) with an erf accurate to 1.5e-7 (Abramowitz–Stegun 7.1.26).
    Gelu,
    /// The tanh approximation of GELU.
    GeluTanh,
    /// x·σ(x) (swish).
    Silu,
    Sigmoid,
    Tanh,
    /// clamp(x/6 + 1/2, 0, 1).
    HardSigmoid,
    /// x·HardSigmoid(x).
    HardSwish,
    /// √x.
    Sqrt,
    /// 1/√x.
    Rsqrt,
}

impl Act {
    /// The code the shaders switch on.
    pub fn code(self) -> u32 {
        self as u32
    }

    /// The reference implementation (the shaders compute the same formulas in f32).
    pub fn apply(self, x: f32) -> f32 {
        match self {
            Act::Identity => x,
            Act::Relu => x.max(0.0),
            Act::Relu6 => x.clamp(0.0, 6.0),
            Act::Gelu => 0.5 * x * (1.0 + erf(x * std::f32::consts::FRAC_1_SQRT_2)),
            Act::GeluTanh => {
                let k = (2.0 / std::f32::consts::PI).sqrt();
                0.5 * x * (1.0 + (k * (x + 0.044_715 * x * x * x)).tanh())
            }
            Act::Silu => x * sigmoid(x),
            Act::Sigmoid => sigmoid(x),
            Act::Tanh => x.tanh(),
            Act::HardSigmoid => (x / 6.0 + 0.5).clamp(0.0, 1.0),
            Act::HardSwish => x * (x / 6.0 + 0.5).clamp(0.0, 1.0),
            Act::Sqrt => x.sqrt(),
            Act::Rsqrt => 1.0 / x.sqrt(),
        }
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// erf(x), Abramowitz–Stegun 7.1.26 (|error| ≤ 1.5e-7), the formula the shaders use.
pub fn erf(x: f32) -> f32 {
    let a = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * a);
    let y = 1.0
        - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_74) * t
            + 0.254_829_6)
            * t
            * (-a * a).exp();
    y.copysign(x)
}

/// 2-D convolution of an NCHW input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Conv2d {
    /// (y, x).
    pub stride: [usize; 2],
    /// Top, left, bottom, right (zeros).
    pub pad: [usize; 4],
    /// (y, x).
    pub dilation: [usize; 2],
    /// Channel groups: 1 is a full conv, C a depthwise one.
    pub groups: usize,
    /// Fused activation after the bias.
    pub act: Act,
}

impl Default for Conv2d {
    fn default() -> Conv2d {
        Conv2d {
            stride: [1, 1],
            pad: [0; 4],
            dilation: [1, 1],
            groups: 1,
            act: Act::Identity,
        }
    }
}

impl Conv2d {
    /// Same-size ("same") symmetric padding for an odd kernel, with a stride.
    pub fn same(kernel: usize, stride: usize) -> Conv2d {
        let p = kernel / 2;
        Conv2d {
            stride: [stride; 2],
            pad: [p; 4],
            ..Default::default()
        }
    }

    pub fn groups(mut self, g: usize) -> Conv2d {
        self.groups = g;
        self
    }

    pub fn act(mut self, a: Act) -> Conv2d {
        self.act = a;
        self
    }
}

/// Batched matrix product `a · b` (or `a · bᵀ`), plus an optional bias over the last axis
/// and a fused activation. f32 accumulation whatever the storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MatMul {
    /// `b` is `[.., N, K]` (a PyTorch `Linear` weight) rather than `[.., K, N]`.
    pub trans_b: bool,
    pub act: Act,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Max,
    Min,
    /// `a` where `a ≥ 0`, else `a·b` (a parametric ReLU with slopes `b`).
    Prelu,
    /// (a − b)².
    SquaredDiff,
}

impl BinaryOp {
    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => a / b,
            BinaryOp::Max => a.max(b),
            BinaryOp::Min => a.min(b),
            BinaryOp::Prelu => {
                if a >= 0.0 {
                    a
                } else {
                    a * b
                }
            }
            BinaryOp::SquaredDiff => (a - b) * (a - b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PoolKind {
    Max,
    /// Averages the taps inside the input (padding is not counted).
    Avg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pool2d {
    pub kind: PoolKind,
    pub kernel: [usize; 2],
    pub stride: [usize; 2],
    /// Top, left, bottom, right.
    pub pad: [usize; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReduceOp {
    Sum,
    Mean,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeMode {
    /// Source pixel floor((o + ½)·in/out).
    Nearest,
    /// Half-pixel centres (`align_corners = false`), edge-clamped, no antialiasing.
    Bilinear,
    /// Source o·in/out, no half-pixel offset (TensorFlow's legacy sampling), edge-clamped.
    BilinearAsymmetric,
    /// Corners aligned (PyTorch's `align_corners = true`): source o·(in − 1)/(out − 1).
    BilinearAligned,
}

/// Which part of the image a [`Preprocess`] maps onto its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Crop {
    /// The whole image, aspect ratio not kept (SigLIP-style).
    Stretch,
    /// The largest centred region of the output's aspect ratio (CLIP-style: resize the short
    /// side, then centre-crop).
    Center,
}

/// RGBA8 image → `[1, 3, H, W]` f32: crop, resample with a triangle filter (widened by the
/// scale factor when downscaling with `antialias`, as PIL does), scale to [0, 1], subtract
/// `mean` and divide by `std` per channel. Alpha is ignored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preprocess {
    pub width: usize,
    pub height: usize,
    pub crop: Crop,
    pub antialias: bool,
    pub mean: [f32; 3],
    pub std: [f32; 3],
}

impl Preprocess {
    /// The source rectangle (x0, y0, w, h) in pixels for an image of `sw × sh`.
    pub fn source_rect(&self, sw: usize, sh: usize) -> [f32; 4] {
        let (sw, sh) = (sw as f32, sh as f32);
        match self.crop {
            Crop::Stretch => [0.0, 0.0, sw, sh],
            Crop::Center => {
                let want = self.width as f32 / self.height as f32;
                if sw / sh > want {
                    let w = sh * want;
                    [(sw - w) * 0.5, 0.0, w, sh]
                } else {
                    let h = sw / want;
                    [0.0, (sh - h) * 0.5, sw, h]
                }
            }
        }
    }
}

/// An operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Inputs `x`, `w`, optional `bias`.
    Conv2d(Conv2d),
    /// Inputs `a`, `b`, optional `bias` (over the last output axis).
    MatMul(MatMul),
    /// Inputs `a`, `b`, broadcast numpy-style.
    Binary(BinaryOp),
    /// `act(x·mul + add)`.
    Unary {
        act: Act,
        mul: f32,
        add: f32,
    },
    /// Over the last axis; inputs `x`, `gamma`, `beta` (both `[D]`).
    LayerNorm {
        eps: f32,
    },
    /// Layer norm over the channels of an NCHW map (each pixel normalised across `C`, as
    /// SAM's `LayerNorm2d` and ConvNeXt's channels-first norm); inputs `x`, `gamma`, `beta`
    /// (both `[C]`).
    ChannelNorm {
        eps: f32,
    },
    /// Group norm of an NCHW map (PyTorch's `GroupNorm`): each image's channels split into
    /// `groups` runs, each run normalised over its channels and pixels together; inputs `x`,
    /// `gamma`, `beta` (both `[C]`, per channel). One group is a layer norm over C·H·W.
    GroupNorm {
        groups: usize,
        eps: f32,
    },
    /// NAFNet's simple gate: an NCHW map's channels split into two halves, multiplied
    /// (`x[:, :C/2] · x[:, C/2:]`); output `[N, C/2, H, W]`.
    Gate,
    /// Depth to space (PyTorch's `PixelShuffle`): `[N, C·r², H, W]` to `[N, C, H·r, W·r]`,
    /// input channel `c·r² + i·r + j` going to output pixel `(y·r + i, x·r + j)`.
    DepthToSpace {
        block: usize,
    },
    /// Transposed 2-D convolution of an NCHW input (PyTorch's `ConvTranspose2d`, groups 1):
    /// inputs `x`, `w [C, O, KH, KW]`, optional `bias [O]`. Output size
    /// `(in − 1)·stride − pad_begin − pad_end + kernel`.
    ConvTranspose2d {
        /// (y, x).
        stride: [usize; 2],
        /// Top, left, bottom, right (cropped from the full output).
        pad: [usize; 4],
        act: Act,
    },
    /// x / max(‖x‖, eps) over the last axis.
    L2Norm {
        eps: f32,
    },
    /// Over the last axis.
    Softmax,
    /// Multi-head scaled dot-product attention, softmax(scale·QKᵀ)·V per head. Inputs
    /// `q [N, Tq, D]`, `k [N, Tk, D]`, `v [N, Tk, D]`, or one packed `qkv [N, T, 3D]`.
    Attention {
        heads: usize,
        scale: f32,
        causal: bool,
        packed: bool,
    },
    Pool2d(Pool2d),
    /// Over one axis, which is kept with size 1.
    Reduce {
        op: ReduceOp,
        axis: usize,
    },
    /// NCHW to `[N, C, h, w]`.
    Resize {
        height: usize,
        width: usize,
        mode: ResizeMode,
    },
    /// Same data, the output's shape.
    Reshape,
    Transpose {
        perm: Vec<usize>,
    },
    /// Per axis: start, end (exclusive), step ≥ 1.
    Slice {
        starts: Vec<usize>,
        ends: Vec<usize>,
        steps: Vec<usize>,
    },
    Concat {
        axis: usize,
    },
    /// Rows of `table [V, D]` at `indices` (f32 holding integers; out of range clamps).
    Gather,
    /// An image input to a model input.
    Preprocess(Preprocess),
    /// Stores the values as f16 (round to nearest even).
    CastF16,
    /// A 1-D LSTM along axis 1 of `pre [S, T, 4·ns]` (each sequence's input projections and
    /// biases, gates in the order cell input, input, forget, output), with recurrent
    /// weights `wh [ns, 4·ns]` (row j: what state j adds to each gate); state starts at zero, the cell is clamped to ±100 (as
    /// Tesseract does). Output `[S, T, ns]`, or `[S, 1, ns]` (the last state) when
    /// `summary`. `reverse` runs from the end. An optional third input `lens [L]` (f32
    /// holding integers) gives the valid length of every `per` consecutive sequences: steps
    /// past it are not run (a reversed sequence starts at its own end) and output zero.
    Lstm {
        reverse: bool,
        summary: bool,
        per: usize,
    },
    /// Pads an NCHW map by mirroring it about its edge pixels (PyTorch's `ReflectionPad2d`
    /// and `padding_mode = "reflect"`); each pad is smaller than its side.
    ReflectPad {
        /// Top, left, bottom, right.
        pads: [usize; 4],
    },
    /// A discrete Fourier transform along one axis ([`Dft`]).
    Dft(Dft),
}

/// A discrete Fourier transform, orthonormal (each way scaled by 1/√n, as numpy's and
/// PyTorch's `norm = "ortho"`). Complex values are a last axis of 2 (real, imaginary).
/// LaMa's Fourier units are `rfft2` = [`Dft::RealToComplex`] then [`Dft::Complex`] along
/// the rows, and `irfft2` the reverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dft {
    /// Real `[.., n]` to its half spectrum `[.., n/2 + 1, 2]` (forward, e^−iθ).
    RealToComplex,
    /// Complex `[.., n, m, 2]` transformed along the `n` axis, for each of the `m`.
    Complex { inverse: bool },
    /// A half spectrum `[.., n/2 + 1, 2]` to real `[.., n]` (inverse, e^+iθ): the
    /// spectrum's Hermitian completion, the imaginary parts of the zero and Nyquist bins
    /// ignored (numpy's `irfft`).
    ComplexToReal { n: usize },
}

/// How a value is defined.
#[derive(Clone, Debug, PartialEq)]
pub enum Def {
    /// The n-th tensor input.
    Input(usize),
    /// The n-th image input (RGBA8 of any size; only [`Op::Preprocess`] reads it).
    Image(usize),
    Const(Tensor),
    /// Output of the n-th node.
    Node(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueInfo {
    pub shape: Vec<usize>,
    pub dtype: DType,
    pub def: Def,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub op: Op,
    pub inputs: Vec<Value>,
    pub output: Value,
}

/// A graph under construction or ready to run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub(crate) values: Vec<ValueInfo>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) inputs: Vec<Value>,
    pub(crate) outputs: Vec<Value>,
}

fn shape_err<T>(msg: impl Into<String>) -> Result<T, Error> {
    Err(Error::Shape(msg.into()))
}

fn check_shape(shape: &[usize]) -> Result<(), Error> {
    if shape.len() > MAX_RANK {
        return shape_err(format!("rank {} above {MAX_RANK}", shape.len()));
    }
    match numel(shape) {
        Some(n) if n <= MAX_ELEMENTS => Ok(()),
        _ => shape_err(format!("{shape:?} has too many elements")),
    }
}

/// Numpy broadcasting of two shapes.
pub fn broadcast(a: &[usize], b: &[usize]) -> Result<Vec<usize>, Error> {
    let r = a.len().max(b.len());
    let mut out = vec![0; r];
    for i in 0..r {
        let da = if i + a.len() >= r {
            a[i + a.len() - r]
        } else {
            1
        };
        let db = if i + b.len() >= r {
            b[i + b.len() - r]
        } else {
            1
        };
        out[i] = if da == db || db == 1 {
            da
        } else if da == 1 {
            db
        } else {
            return shape_err(format!("{a:?} and {b:?} do not broadcast"));
        };
    }
    Ok(out)
}

/// Output size of a sliding window, or an error if the window does not fit.
fn window(
    len: usize,
    pad: usize,
    kernel: usize,
    stride: usize,
    dil: usize,
) -> Result<usize, Error> {
    if stride == 0 || dil == 0 || kernel == 0 {
        return shape_err("zero stride, dilation or kernel");
    }
    let span = dil
        .checked_mul(kernel - 1)
        .and_then(|s| s.checked_add(1))
        .ok_or(Error::Shape("window overflow".into()))?;
    let padded = len
        .checked_add(pad)
        .ok_or(Error::Shape("padding overflow".into()))?;
    if padded < span {
        return shape_err(format!("window {span} larger than padded input {padded}"));
    }
    Ok((padded - span) / stride + 1)
}

impl Graph {
    pub fn new() -> Graph {
        Graph::default()
    }

    pub fn info(&self, v: Value) -> &ValueInfo {
        &self.values[v.index()]
    }

    pub fn shape(&self, v: Value) -> &[usize] {
        &self.values[v.index()].shape
    }

    pub fn dtype(&self, v: Value) -> DType {
        self.values[v.index()].dtype
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn inputs(&self) -> &[Value] {
        &self.inputs
    }

    pub fn outputs(&self) -> &[Value] {
        &self.outputs
    }

    pub fn values(&self) -> &[ValueInfo] {
        &self.values
    }

    fn push(&mut self, shape: Vec<usize>, dtype: DType, def: Def) -> Result<Value, Error> {
        check_shape(&shape)?;
        let v = Value(self.values.len() as u32);
        self.values.push(ValueInfo { shape, dtype, def });
        Ok(v)
    }

    fn check(&self, v: Value) -> Result<(), Error> {
        if v.index() < self.values.len() {
            Ok(())
        } else {
            Err(Error::Shape(format!("value {} not in this graph", v.0)))
        }
    }

    fn tensor_value(&self, v: Value) -> Result<&[usize], Error> {
        self.check(v)?;
        if matches!(self.values[v.index()].def, Def::Image(_)) {
            return shape_err("an image input feeds only Preprocess");
        }
        Ok(self.shape(v))
    }

    fn node(
        &mut self,
        op: Op,
        inputs: Vec<Value>,
        shape: Vec<usize>,
        dtype: DType,
    ) -> Result<Value, Error> {
        let output = self.push(shape, dtype, Def::Node(self.nodes.len()))?;
        self.nodes.push(Node { op, inputs, output });
        Ok(output)
    }

    /// A tensor input (fed per run).
    pub fn input(&mut self, shape: &[usize], dtype: DType) -> Result<Value, Error> {
        let n = self.inputs.len();
        let v = self.push(shape.to_vec(), dtype, Def::Input(n))?;
        self.inputs.push(v);
        Ok(v)
    }

    /// An RGBA8 image input of any size (fed per run, read by [`Graph::preprocess`]).
    pub fn image_input(&mut self) -> Result<Value, Error> {
        let n = self.inputs.len();
        let v = self.push(vec![0, 0, 4], DType::F32, Def::Image(n))?;
        self.inputs.push(v);
        Ok(v)
    }

    /// A constant (a weight).
    pub fn constant(&mut self, t: Tensor) -> Result<Value, Error> {
        if numel(&t.shape) != Some(t.len()) {
            return shape_err("constant data does not match its shape");
        }
        let (shape, dtype) = (t.shape.clone(), t.dtype());
        self.push(shape, dtype, Def::Const(t))
    }

    /// Marks a graph output (outputs are read back in this order).
    pub fn output(&mut self, v: Value) -> Result<(), Error> {
        self.tensor_value(v)?;
        self.outputs.push(v);
        Ok(())
    }

    pub fn conv2d(
        &mut self,
        x: Value,
        w: Value,
        bias: Option<Value>,
        p: Conv2d,
    ) -> Result<Value, Error> {
        let xs = self.tensor_value(x)?.to_vec();
        let ws = self.tensor_value(w)?.to_vec();
        if xs.len() != 4 || ws.len() != 4 {
            return shape_err(format!(
                "conv2d wants NCHW input and OIHW weights: {xs:?}, {ws:?}"
            ));
        }
        let g = p.groups;
        if g == 0 || xs[1] % g != 0 || ws[0] % g != 0 || ws[1] * g != xs[1] {
            return shape_err(format!("conv2d groups {g}: input {xs:?}, weights {ws:?}"));
        }
        let mut inputs = vec![x, w];
        if let Some(b) = bias {
            if self.tensor_value(b)? != [ws[0]] {
                return shape_err("conv2d bias must be [O]");
            }
            inputs.push(b);
        }
        let oh = window(
            xs[2],
            p.pad[0] + p.pad[2],
            ws[2],
            p.stride[0],
            p.dilation[0],
        )?;
        let ow = window(
            xs[3],
            p.pad[1] + p.pad[3],
            ws[3],
            p.stride[1],
            p.dilation[1],
        )?;
        self.node(
            Op::Conv2d(p),
            inputs,
            vec![xs[0], ws[0], oh, ow],
            DType::F32,
        )
    }

    /// Batched matmul; batch axes (up to two) broadcast.
    pub fn matmul(
        &mut self,
        a: Value,
        b: Value,
        bias: Option<Value>,
        p: MatMul,
    ) -> Result<Value, Error> {
        let sa = self.tensor_value(a)?.to_vec();
        let sb = self.tensor_value(b)?.to_vec();
        if sa.len() < 2 || sb.len() < 2 || sa.len() > 4 || sb.len() > 4 {
            return shape_err(format!("matmul ranks 2..=4: {sa:?} x {sb:?}"));
        }
        let (m, k) = (sa[sa.len() - 2], sa[sa.len() - 1]);
        let (kb, n) = if p.trans_b {
            (sb[sb.len() - 1], sb[sb.len() - 2])
        } else {
            (sb[sb.len() - 2], sb[sb.len() - 1])
        };
        if k != kb {
            return shape_err(format!("matmul inner sizes differ: {sa:?} x {sb:?}"));
        }
        let mut shape = broadcast(&sa[..sa.len() - 2], &sb[..sb.len() - 2])?;
        shape.extend([m, n]);
        let mut inputs = vec![a, b];
        if let Some(bv) = bias {
            if self.tensor_value(bv)? != [n] {
                return shape_err("matmul bias must be [N]");
            }
            inputs.push(bv);
        }
        self.node(Op::MatMul(p), inputs, shape, DType::F32)
    }

    /// `x · wᵀ + b` with a PyTorch `Linear` weight `[out, in]`.
    pub fn linear(&mut self, x: Value, w: Value, b: Option<Value>) -> Result<Value, Error> {
        self.matmul(
            x,
            w,
            b,
            MatMul {
                trans_b: true,
                act: Act::Identity,
            },
        )
    }

    pub fn binary(&mut self, op: BinaryOp, a: Value, b: Value) -> Result<Value, Error> {
        let shape = broadcast(self.tensor_value(a)?, self.tensor_value(b)?)?;
        self.node(Op::Binary(op), vec![a, b], shape, DType::F32)
    }

    pub fn add(&mut self, a: Value, b: Value) -> Result<Value, Error> {
        self.binary(BinaryOp::Add, a, b)
    }

    pub fn mul(&mut self, a: Value, b: Value) -> Result<Value, Error> {
        self.binary(BinaryOp::Mul, a, b)
    }

    pub fn unary(&mut self, x: Value, act: Act, mul: f32, add: f32) -> Result<Value, Error> {
        let shape = self.tensor_value(x)?.to_vec();
        self.node(Op::Unary { act, mul, add }, vec![x], shape, DType::F32)
    }

    pub fn act(&mut self, x: Value, act: Act) -> Result<Value, Error> {
        self.unary(x, act, 1.0, 0.0)
    }

    pub fn layer_norm(
        &mut self,
        x: Value,
        gamma: Value,
        beta: Value,
        eps: f32,
    ) -> Result<Value, Error> {
        let shape = self.tensor_value(x)?.to_vec();
        let d = *shape.last().unwrap_or(&0);
        if d == 0 || self.tensor_value(gamma)? != [d] || self.tensor_value(beta)? != [d] {
            return shape_err(format!(
                "layer_norm of {shape:?} wants gamma and beta [{d}]"
            ));
        }
        self.node(
            Op::LayerNorm { eps },
            vec![x, gamma, beta],
            shape,
            DType::F32,
        )
    }

    /// Layer norm across the channels of an NCHW map, per pixel ([`Op::ChannelNorm`]).
    pub fn channel_norm(
        &mut self,
        x: Value,
        gamma: Value,
        beta: Value,
        eps: f32,
    ) -> Result<Value, Error> {
        let shape = self.tensor_value(x)?.to_vec();
        if shape.len() != 4 || shape[1] == 0 {
            return shape_err(format!("channel_norm wants an NCHW map: {shape:?}"));
        }
        let c = shape[1];
        if self.tensor_value(gamma)? != [c] || self.tensor_value(beta)? != [c] {
            return shape_err(format!(
                "channel_norm of {shape:?} wants gamma and beta [{c}]"
            ));
        }
        self.node(
            Op::ChannelNorm { eps },
            vec![x, gamma, beta],
            shape,
            DType::F32,
        )
    }

    /// NAFNet's simple gate ([`Op::Gate`]).
    pub fn gate(&mut self, x: Value) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if s.len() != 4 || s[1] == 0 || !s[1].is_multiple_of(2) {
            return shape_err(format!("gate wants an NCHW map with even channels: {s:?}"));
        }
        self.node(
            Op::Gate,
            vec![x],
            vec![s[0], s[1] / 2, s[2], s[3]],
            DType::F32,
        )
    }

    /// Depth to space by `block` ([`Op::DepthToSpace`]).
    pub fn depth_to_space(&mut self, x: Value, block: usize) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        let r2 = block * block;
        if s.len() != 4 || block == 0 || s[1] == 0 || !s[1].is_multiple_of(r2) {
            return shape_err(format!("depth_to_space by {block} of {s:?}"));
        }
        self.node(
            Op::DepthToSpace { block },
            vec![x],
            vec![s[0], s[1] / r2, s[2] * block, s[3] * block],
            DType::F32,
        )
    }

    /// Group norm of an NCHW map ([`Op::GroupNorm`]).
    pub fn group_norm(
        &mut self,
        x: Value,
        gamma: Value,
        beta: Value,
        groups: usize,
        eps: f32,
    ) -> Result<Value, Error> {
        let shape = self.tensor_value(x)?.to_vec();
        if shape.len() != 4 || groups == 0 || shape[1] % groups != 0 || shape[1] == 0 {
            return shape_err(format!("group_norm of {shape:?} in {groups} groups"));
        }
        let c = shape[1];
        if self.tensor_value(gamma)? != [c] || self.tensor_value(beta)? != [c] {
            return shape_err(format!(
                "group_norm of {shape:?} wants gamma and beta [{c}]"
            ));
        }
        self.node(
            Op::GroupNorm { groups, eps },
            vec![x, gamma, beta],
            shape,
            DType::F32,
        )
    }

    /// A transposed convolution ([`Op::ConvTranspose2d`]); `w` is `[C, O, KH, KW]`.
    pub fn conv_transpose2d(
        &mut self,
        x: Value,
        w: Value,
        bias: Option<Value>,
        stride: [usize; 2],
        pad: [usize; 4],
        act: Act,
    ) -> Result<Value, Error> {
        let xs = self.tensor_value(x)?.to_vec();
        let ws = self.tensor_value(w)?.to_vec();
        if xs.len() != 4 || ws.len() != 4 || ws[0] != xs[1] || stride.contains(&0) {
            return shape_err(format!(
                "conv_transpose2d wants NCHW input and [C, O, KH, KW] weights: {xs:?}, {ws:?}"
            ));
        }
        let mut inputs = vec![x, w];
        if let Some(b) = bias {
            if self.tensor_value(b)? != [ws[1]] {
                return shape_err("conv_transpose2d bias must be [O]");
            }
            inputs.push(b);
        }
        let full = |i: usize, s: usize, k: usize| (i.max(1) - 1) * s + k;
        let (fh, fw) = (full(xs[2], stride[0], ws[2]), full(xs[3], stride[1], ws[3]));
        if xs[2] == 0 || xs[3] == 0 || pad[0] + pad[2] >= fh || pad[1] + pad[3] >= fw {
            return shape_err(format!("conv_transpose2d padding {pad:?} of {xs:?}"));
        }
        self.node(
            Op::ConvTranspose2d { stride, pad, act },
            inputs,
            vec![xs[0], ws[1], fh - pad[0] - pad[2], fw - pad[1] - pad[3]],
            DType::F32,
        )
    }

    pub fn l2_norm(&mut self, x: Value, eps: f32) -> Result<Value, Error> {
        let shape = self.tensor_value(x)?.to_vec();
        if shape.last().is_none_or(|&d| d == 0) {
            return shape_err("l2_norm of an empty axis");
        }
        self.node(Op::L2Norm { eps }, vec![x], shape, DType::F32)
    }

    pub fn softmax(&mut self, x: Value) -> Result<Value, Error> {
        let shape = self.tensor_value(x)?.to_vec();
        if shape.last().is_none_or(|&d| d == 0) {
            return shape_err("softmax of an empty axis");
        }
        self.node(Op::Softmax, vec![x], shape, DType::F32)
    }

    /// Attention over separate `q`, `k`, `v`; `scale` defaults to 1/√(D/heads).
    pub fn attention(
        &mut self,
        q: Value,
        k: Value,
        v: Value,
        heads: usize,
        scale: Option<f32>,
        causal: bool,
    ) -> Result<Value, Error> {
        let (sq, sk, sv) = (
            self.tensor_value(q)?.to_vec(),
            self.tensor_value(k)?.to_vec(),
            self.tensor_value(v)?.to_vec(),
        );
        if sq.len() != 3 || sk != sv || sk.len() != 3 || sq[0] != sk[0] || sq[2] != sk[2] {
            return shape_err(format!("attention wants [N, T, D]: {sq:?} {sk:?} {sv:?}"));
        }
        if heads == 0 || sq[2] % heads != 0 || sq[2] == 0 || sk[1] == 0 {
            return shape_err(format!(
                "attention: D {} not split into {heads} heads",
                sq[2]
            ));
        }
        let scale = scale.unwrap_or(1.0 / ((sq[2] / heads) as f32).sqrt());
        self.node(
            Op::Attention {
                heads,
                scale,
                causal,
                packed: false,
            },
            vec![q, k, v],
            sq,
            DType::F32,
        )
    }

    /// Attention over a packed `qkv [N, T, 3D]` (one fused projection).
    pub fn attention_packed(
        &mut self,
        qkv: Value,
        heads: usize,
        scale: Option<f32>,
        causal: bool,
    ) -> Result<Value, Error> {
        let s = self.tensor_value(qkv)?.to_vec();
        if s.len() != 3 || s[2] % 3 != 0 || s[1] == 0 {
            return shape_err(format!("packed attention wants [N, T, 3D]: {s:?}"));
        }
        let d = s[2] / 3;
        if heads == 0 || d == 0 || d % heads != 0 {
            return shape_err(format!("attention: D {d} not split into {heads} heads"));
        }
        let scale = scale.unwrap_or(1.0 / ((d / heads) as f32).sqrt());
        self.node(
            Op::Attention {
                heads,
                scale,
                causal,
                packed: true,
            },
            vec![qkv],
            vec![s[0], s[1], d],
            DType::F32,
        )
    }

    pub fn pool2d(&mut self, x: Value, p: Pool2d) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if s.len() != 4 {
            return shape_err("pool2d wants NCHW");
        }
        if p.pad[0] >= p.kernel[0]
            || p.pad[2] >= p.kernel[0]
            || p.pad[1] >= p.kernel[1]
            || p.pad[3] >= p.kernel[1]
        {
            return shape_err("pool2d padding must be smaller than the kernel");
        }
        let oh = window(s[2], p.pad[0] + p.pad[2], p.kernel[0], p.stride[0], 1)?;
        let ow = window(s[3], p.pad[1] + p.pad[3], p.kernel[1], p.stride[1], 1)?;
        self.node(Op::Pool2d(p), vec![x], vec![s[0], s[1], oh, ow], DType::F32)
    }

    /// Mean over H and W, keeping them as 1.
    pub fn global_avg_pool(&mut self, x: Value) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if s.len() != 4 {
            return shape_err("global_avg_pool wants NCHW");
        }
        let flat = self.reshape(x, &[s[0], s[1], s[2] * s[3]])?;
        let m = self.reduce(flat, ReduceOp::Mean, 2)?;
        self.reshape(m, &[s[0], s[1], 1, 1])
    }

    pub fn reduce(&mut self, x: Value, op: ReduceOp, axis: usize) -> Result<Value, Error> {
        let mut s = self.tensor_value(x)?.to_vec();
        if axis >= s.len() || s[axis] == 0 {
            return shape_err(format!("reduce over axis {axis} of {s:?}"));
        }
        s[axis] = 1;
        self.node(Op::Reduce { op, axis }, vec![x], s, DType::F32)
    }

    pub fn resize(
        &mut self,
        x: Value,
        height: usize,
        width: usize,
        mode: ResizeMode,
    ) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if s.len() != 4 || s[2] == 0 || s[3] == 0 || height == 0 || width == 0 {
            return shape_err(format!("resize of {s:?} to {height}x{width}"));
        }
        self.node(
            Op::Resize {
                height,
                width,
                mode,
            },
            vec![x],
            vec![s[0], s[1], height, width],
            DType::F32,
        )
    }

    pub fn reshape(&mut self, x: Value, shape: &[usize]) -> Result<Value, Error> {
        let s = self.tensor_value(x)?;
        if numel(s) != numel(shape) {
            return shape_err(format!("reshape {s:?} to {shape:?}"));
        }
        let dtype = self.dtype(x);
        self.node(Op::Reshape, vec![x], shape.to_vec(), dtype)
    }

    pub fn transpose(&mut self, x: Value, perm: &[usize]) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        let mut seen = vec![false; s.len()];
        if perm.len() != s.len() {
            return shape_err(format!("transpose {perm:?} of {s:?}"));
        }
        for &p in perm {
            if p >= s.len() || seen[p] {
                return shape_err(format!("transpose {perm:?} is not a permutation"));
            }
            seen[p] = true;
        }
        let shape = perm.iter().map(|&p| s[p]).collect();
        self.node(
            Op::Transpose {
                perm: perm.to_vec(),
            },
            vec![x],
            shape,
            DType::F32,
        )
    }

    /// Every axis sliced `start..end` by `step`.
    pub fn slice(
        &mut self,
        x: Value,
        starts: &[usize],
        ends: &[usize],
        steps: &[usize],
    ) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if starts.len() != s.len() || ends.len() != s.len() || steps.len() != s.len() {
            return shape_err("slice wants a start, end and step per axis");
        }
        let mut shape = Vec::with_capacity(s.len());
        for i in 0..s.len() {
            if steps[i] == 0 || starts[i] > ends[i] || ends[i] > s[i] {
                return shape_err(format!(
                    "slice {}..{} by {} of axis {i} ({})",
                    starts[i], ends[i], steps[i], s[i]
                ));
            }
            shape.push((ends[i] - starts[i]).div_ceil(steps[i]));
        }
        self.node(
            Op::Slice {
                starts: starts.to_vec(),
                ends: ends.to_vec(),
                steps: steps.to_vec(),
            },
            vec![x],
            shape,
            DType::F32,
        )
    }

    /// `start..end` of one axis.
    pub fn slice_axis(
        &mut self,
        x: Value,
        axis: usize,
        start: usize,
        end: usize,
    ) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if axis >= s.len() {
            return shape_err("slice axis out of range");
        }
        let mut starts = vec![0; s.len()];
        let mut ends = s.clone();
        starts[axis] = start;
        ends[axis] = end;
        self.slice(x, &starts, &ends, &vec![1; s.len()])
    }

    pub fn concat(&mut self, xs: &[Value], axis: usize) -> Result<Value, Error> {
        let Some(&first) = xs.first() else {
            return shape_err("concat of nothing");
        };
        let mut shape = self.tensor_value(first)?.to_vec();
        if axis >= shape.len() {
            return shape_err("concat axis out of range");
        }
        for &x in &xs[1..] {
            let s = self.tensor_value(x)?;
            let same = s.len() == shape.len()
                && s.iter()
                    .zip(&shape)
                    .enumerate()
                    .all(|(i, (a, b))| i == axis || a == b);
            if !same {
                return shape_err(format!("concat of {shape:?} and {s:?}"));
            }
            shape[axis] = shape[axis]
                .checked_add(s[axis])
                .ok_or(Error::Shape("concat overflow".into()))?;
        }
        self.node(Op::Concat { axis }, xs.to_vec(), shape, DType::F32)
    }

    /// Rows of `table [V, D]` picked by `indices` (any shape) → `indices.shape + [D]`.
    pub fn gather(&mut self, table: Value, indices: Value) -> Result<Value, Error> {
        let t = self.tensor_value(table)?.to_vec();
        let mut shape = self.tensor_value(indices)?.to_vec();
        if t.len() != 2 || t[0] == 0 {
            return shape_err("gather wants a [V, D] table");
        }
        shape.push(t[1]);
        self.node(Op::Gather, vec![table, indices], shape, DType::F32)
    }

    pub fn preprocess(&mut self, image: Value, p: Preprocess) -> Result<Value, Error> {
        self.check(image)?;
        if !matches!(self.values[image.index()].def, Def::Image(_)) {
            return shape_err("preprocess reads an image input");
        }
        if p.width == 0 || p.height == 0 || p.std.iter().any(|&s| s == 0.0 || !s.is_finite()) {
            return shape_err("preprocess wants a size and non-zero std");
        }
        self.node(
            Op::Preprocess(p),
            vec![image],
            vec![1, 3, p.height, p.width],
            DType::F32,
        )
    }

    /// See [`Op::Lstm`].
    pub fn lstm(
        &mut self,
        pre: Value,
        wh: Value,
        lens: Option<(Value, usize)>,
        reverse: bool,
        summary: bool,
    ) -> Result<Value, Error> {
        let sp = self.tensor_value(pre)?.to_vec();
        let sw = self.tensor_value(wh)?.to_vec();
        if sp.len() != 3 || sw.len() != 2 || sw[1] != 4 * sw[0] || sp[2] != sw[1] || sw[0] == 0 {
            return shape_err(format!(
                "lstm wants pre [S, T, 4ns] and wh [ns, 4ns]: {sp:?} {sw:?}"
            ));
        }
        if sw[0] > LSTM_MAX_STATES {
            return shape_err(format!("lstm with more than {LSTM_MAX_STATES} states"));
        }
        let mut inputs = vec![pre, wh];
        let mut per = 0;
        if let Some((l, p)) = lens {
            let sl = self.tensor_value(l)?.to_vec();
            if p == 0 || sl.len() != 1 || sl[0] * p != sp[0] {
                return shape_err(format!(
                    "lstm lengths {sl:?} by {p} for {} sequences",
                    sp[0]
                ));
            }
            inputs.push(l);
            per = p;
        }
        let t = if summary { 1 } else { sp[1] };
        self.node(
            Op::Lstm {
                reverse,
                summary,
                per,
            },
            inputs,
            vec![sp[0], t, sw[0]],
            DType::F32,
        )
    }

    /// Mirror padding of an NCHW map ([`Op::ReflectPad`]); `pads` top, left, bottom, right.
    pub fn reflect_pad(&mut self, x: Value, pads: [usize; 4]) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        if s.len() != 4 || pads[0].max(pads[2]) >= s[2] || pads[1].max(pads[3]) >= s[3] {
            return shape_err(format!("reflect_pad {pads:?} of {s:?}"));
        }
        if pads == [0; 4] {
            return Ok(x);
        }
        let shape = vec![
            s[0],
            s[1],
            s[2] + pads[0] + pads[2],
            s[3] + pads[1] + pads[3],
        ];
        self.node(Op::ReflectPad { pads }, vec![x], shape, DType::F32)
    }

    /// A Fourier transform ([`Op::Dft`]).
    pub fn dft(&mut self, x: Value, kind: Dft) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        let r = s.len();
        let bad = || shape_err(format!("dft {kind:?} of {s:?}"));
        let shape = match kind {
            Dft::RealToComplex => {
                if r == 0 || r + 1 > MAX_RANK || s[r - 1] == 0 {
                    return bad();
                }
                let mut o = s.clone();
                o[r - 1] = s[r - 1] / 2 + 1;
                o.push(2);
                o
            }
            Dft::Complex { .. } => {
                if r < 3 || s[r - 1] != 2 || s[r - 3] == 0 {
                    return bad();
                }
                s.clone()
            }
            Dft::ComplexToReal { n } => {
                if r < 2 || s[r - 1] != 2 || n == 0 || s[r - 2] != n / 2 + 1 {
                    return bad();
                }
                let mut o = s[..r - 1].to_vec();
                o[r - 2] = n;
                o
            }
        };
        self.node(Op::Dft(kind), vec![x], shape, DType::F32)
    }

    pub fn cast_f16(&mut self, x: Value) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        self.node(Op::CastF16, vec![x], s, DType::F16)
    }

    /// Inference batch norm as a per-channel affine (NCHW, or the last axis for rank < 4),
    /// computed from the statistics here. Prefer [`fold_batchnorm`] into the conv before it.
    pub fn batch_norm(&mut self, x: Value, bn: &BatchNorm) -> Result<Value, Error> {
        let s = self.tensor_value(x)?.to_vec();
        let c = bn.gamma.len();
        let shape: Vec<usize> = if s.len() == 4 { vec![c, 1, 1] } else { vec![c] };
        let ch = if s.len() == 4 {
            s[1]
        } else {
            *s.last().unwrap_or(&0)
        };
        if ch != c || bn.beta.len() != c || bn.mean.len() != c || bn.var.len() != c {
            return shape_err(format!("batch_norm of {s:?} with {c} channels"));
        }
        let (scale, shift) = bn.scale_shift();
        let sc = self.constant(Tensor::f32(&shape, scale))?;
        let sh = self.constant(Tensor::f32(&shape, shift))?;
        let y = self.mul(x, sc)?;
        self.add(y, sh)
    }
}

/// Inference batch-norm statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchNorm {
    pub gamma: Vec<f32>,
    pub beta: Vec<f32>,
    pub mean: Vec<f32>,
    pub var: Vec<f32>,
    pub eps: f32,
}

impl BatchNorm {
    /// Per channel: y = x·scale + shift.
    pub fn scale_shift(&self) -> (Vec<f32>, Vec<f32>) {
        let scale: Vec<f32> = self
            .gamma
            .iter()
            .zip(&self.var)
            .map(|(&g, &v)| g / (v + self.eps).sqrt())
            .collect();
        let shift = self
            .beta
            .iter()
            .zip(&self.mean)
            .zip(&scale)
            .map(|((&b, &m), &s)| b - m * s)
            .collect();
        (scale, shift)
    }
}

/// Folds a batch norm into the conv before it: returns the new weights (same dtype as
/// `w`) and bias (f32). Errors when the channel counts disagree.
pub fn fold_batchnorm(
    w: &Tensor,
    bias: Option<&Tensor>,
    bn: &BatchNorm,
) -> Result<(Tensor, Tensor), Error> {
    let o = *w.shape.first().unwrap_or(&0);
    if w.shape.len() != 4 || bn.gamma.len() != o || bn.beta.len() != o {
        return shape_err("fold_batchnorm: weights and statistics disagree");
    }
    if bn.mean.len() != o || bn.var.len() != o || bias.is_some_and(|b| b.shape != [o]) {
        return shape_err("fold_batchnorm: weights and statistics disagree");
    }
    let (scale, shift) = bn.scale_shift();
    let per = w.len() / o.max(1);
    let mut wf = w.to_f32();
    for (oc, chunk) in wf.chunks_mut(per.max(1)).enumerate() {
        for x in chunk {
            *x *= scale[oc];
        }
    }
    let b0 = bias.map(|b| b.to_f32()).unwrap_or_else(|| vec![0.0; o]);
    let nb: Vec<f32> = (0..o).map(|i| b0[i] * scale[i] + shift[i]).collect();
    let mut nw = Tensor::f32(&w.shape, wf);
    if w.dtype() == DType::F16 {
        nw = nw.to_f16();
    }
    Ok((nw, Tensor::f32(&[o], nb)))
}
