//! Depth Anything V2 Small (Yang et al. 2024; Apache-2.0, the larger sizes are not): a
//! monocular relative depth network for Develop's depth lens blur, relight and depth masks.
//! Read from the Hugging Face `DepthAnythingForDepthEstimation` checkpoint;
//! written from the papers (DINOv2, DPT) and the checkpoint's tensor names and shapes,
//! checked against the `transformers` implementation run locally as an oracle (never
//! shipped).
//!
//! - **Backbone**: DINOv2 ViT-S/14: 14×14 patches, a class token, learnt positions (37×37,
//!   resized bicubically for other grids on the CPU when the graph is built), 12 pre-norm
//!   blocks with layer scales (folded into the output projections) and exact GELU. The
//!   outputs of blocks 3, 6, 9 and 12 go through the final layer norm.
//! - **Neck** (DPT): each output without its class token as a map, a 1×1 projection and a
//!   resize (4× and 2× transposed convs, identity, a 3×3 stride-2 conv), a 3×3 conv to 64;
//!   four fusion steps from the coarsest (pre-activation residual units, bilinear
//!   upsampling with aligned corners to the next map's size, 1×1 projection).
//! - **Head**: 3×3 conv to 32, upsampled to the input size, 3×3 conv and ReLU, 1×1 conv and
//!   ReLU: relative inverse depth (larger is nearer), one value per input pixel.

use crate::Error;
use crate::graph::{Act, Conv2d, Crop, Graph, MatMul, Preprocess, ResizeMode, Value};
use crate::models::{Weights, konst, layer_norm, linear};
use crate::tensor::{DType, Tensor};

/// The patch size.
pub const PATCH: usize = 14;
/// The short side the model was trained at (a multiple of [`PATCH`]).
pub const SIDE: usize = 518;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const EPS: f32 = 1e-6;
const B: &str = "backbone";

/// Sizes, read from the checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepthConfig {
    pub dim: usize,
    pub heads: usize,
    pub layers: usize,
    /// The blocks whose outputs feed the neck (1-based).
    pub taps: [usize; 4],
    pub neck: [usize; 4],
    pub fusion: usize,
    pub head: usize,
    /// The position table's side (37 for 518²).
    pub grid: usize,
}

impl DepthConfig {
    /// Depth Anything V2 Small.
    pub fn small() -> DepthConfig {
        DepthConfig {
            dim: 384,
            heads: 6,
            layers: 12,
            taps: [3, 6, 9, 12],
            neck: [48, 96, 192, 384],
            fusion: 64,
            head: 32,
            grid: 37,
        }
    }

    /// From the checkpoint's tensor shapes (Small's layout: the taps of a 12-block model).
    pub fn from_header(h: &crate::safetensors::Header) -> Result<DepthConfig, Error> {
        let shape = |n: &str| {
            h.get(n)
                .map(|t| t.shape.clone())
                .ok_or_else(|| Error::Weights(format!("depth: no {n}")))
        };
        let dim = shape(&format!("{B}.embeddings.cls_token"))?[2];
        let pos = shape(&format!("{B}.embeddings.position_embeddings"))?[1];
        let grid = ((pos.saturating_sub(1)) as f64).sqrt().round() as usize;
        if grid * grid + 1 != pos {
            return Err(Error::Weights(format!("depth: {pos} positions")));
        }
        let layers = (0..64)
            .take_while(|i| {
                h.get(&format!("{B}.encoder.layer.{i}.norm1.weight"))
                    .is_some()
            })
            .count();
        let mut neck = [0; 4];
        for (i, n) in neck.iter_mut().enumerate() {
            *n = shape(&format!("neck.reassemble_stage.layers.{i}.projection.bias"))?[0];
        }
        let fusion = shape("neck.fusion_stage.layers.0.projection.bias")?[0];
        let head = shape("head.conv2.bias")?[0];
        if layers != 12 || dim % 64 != 0 {
            return Err(Error::Weights(format!("depth: {layers} blocks of {dim}")));
        }
        Ok(DepthConfig {
            dim,
            heads: dim / 64,
            layers,
            taps: [3, 6, 9, 12],
            neck,
            fusion,
            head,
            grid,
        })
    }
}

/// The network input for a `w × h` picture: the aspect ratio kept, the short side about
/// `short` (the reference processor's rule: the scale that changes the picture least,
/// each side rounded to a multiple of [`PATCH`]).
pub fn input_size(w: usize, h: usize, short: usize) -> (usize, usize) {
    let (w, h) = (w.max(1) as f64, h.max(1) as f64);
    let (sw, sh) = (short as f64 / w, short as f64 / h);
    let s = if (1.0 - sw).abs() < (1.0 - sh).abs() {
        sw
    } else {
        sh
    };
    let snap = |v: f64| (((v / PATCH as f64).round() as usize) * PATCH).max(PATCH);
    (snap(s * w), snap(s * h))
}

/// The preprocessing: stretched to `w × h` (already the picture's aspect), area-filtered.
pub fn preprocess(w: usize, h: usize) -> Preprocess {
    Preprocess {
        width: w,
        height: h,
        crop: Crop::Stretch,
        antialias: true,
        mean: MEAN,
        std: STD,
    }
}

/// PyTorch's bicubic weights (A = −0.75).
fn cubic(t: f64) -> [f64; 4] {
    const A: f64 = -0.75;
    let w1 = |x: f64| ((A + 2.0) * x - (A + 3.0)) * x * x + 1.0;
    let w2 = |x: f64| ((A * x - 5.0 * A) * x + 8.0 * A) * x - 4.0 * A;
    [w2(t + 1.0), w1(t), w1(1.0 - t), w2(2.0 - t)]
}

/// The position table `[1 + g², d]` resized to a `gh × gw` grid as the reference does
/// (`interpolate(mode = "bicubic", align_corners = false)`, edge-clamped taps); the class
/// position is kept.
pub fn resize_positions(table: &[f32], g: usize, d: usize, (gh, gw): (usize, usize)) -> Tensor {
    if gh == g && gw == g {
        return Tensor::f32(&[1, 1 + g * g, d], table.to_vec());
    }
    let mut out = table[..d].to_vec();
    out.reserve(gh * gw * d);
    let taps = |o: usize, n_out: usize| {
        let s = g as f64 / n_out as f64;
        let c = (o as f64 + 0.5) * s - 0.5;
        let i0 = c.floor();
        let w = cubic(c - i0);
        let idx: [usize; 4] = std::array::from_fn(|k| {
            (i0 as isize - 1 + k as isize).clamp(0, g as isize - 1) as usize
        });
        (idx, w)
    };
    for oy in 0..gh {
        let (iy, wy) = taps(oy, gh);
        for ox in 0..gw {
            let (ix, wx) = taps(ox, gw);
            for c in 0..d {
                let mut acc = 0f64;
                for a in 0..4 {
                    for b in 0..4 {
                        acc += wy[a] * wx[b] * table[d * (1 + iy[a] * g + ix[b]) + c] as f64;
                    }
                }
                out.push(acc as f32);
            }
        }
    }
    Tensor::f32(&[1, 1 + gh * gw, d], out)
}

/// `x [1, T, d]` through `p`'s linear with its output rows scaled by `scale`
/// (a layer scale folded in).
fn scaled_linear(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    (out, inp): (usize, usize),
    scale: &[f32],
) -> Result<Value, Error> {
    let wt = w.get(&format!("{p}.weight"), &[out, inp])?;
    let f16 = wt.dtype() == DType::F16;
    let mut wf = wt.to_f32();
    let mut bf = w.get(&format!("{p}.bias"), &[out])?.to_f32();
    for (o, row) in wf.chunks_mut(inp).enumerate() {
        row.iter_mut().for_each(|v| *v *= scale[o]);
        bf[o] *= scale[o];
    }
    let mut t = Tensor::f32(&[out, inp], wf);
    if f16 {
        t = t.to_f16();
    }
    let wv = g.constant(t)?;
    let bv = g.constant(Tensor::f32(&[out], bf))?;
    g.matmul(
        x,
        wv,
        Some(bv),
        MatMul {
            trans_b: true,
            act: Act::Identity,
        },
    )
}

fn block(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    c: &DepthConfig,
    i: usize,
) -> Result<Value, Error> {
    let d = c.dim;
    let p = format!("{B}.encoder.layer.{i}");
    let h = layer_norm(g, w, x, &format!("{p}.norm1"), d, EPS)?;
    let a = format!("{p}.attention.attention");
    let q = linear(g, w, h, &format!("{a}.query"), (d, d), true, Act::Identity)?;
    let k = linear(g, w, h, &format!("{a}.key"), (d, d), true, Act::Identity)?;
    let v = linear(g, w, h, &format!("{a}.value"), (d, d), true, Act::Identity)?;
    let o = g.attention(q, k, v, c.heads, None, false)?;
    let ls1 = w.get(&format!("{p}.layer_scale1.lambda1"), &[d])?.to_f32();
    let o = scaled_linear(
        g,
        w,
        o,
        &format!("{p}.attention.output.dense"),
        (d, d),
        &ls1,
    )?;
    let x = g.add(x, o)?;
    let h = layer_norm(g, w, x, &format!("{p}.norm2"), d, EPS)?;
    let h = linear(
        g,
        w,
        h,
        &format!("{p}.mlp.fc1"),
        (4 * d, d),
        true,
        Act::Gelu,
    )?;
    let ls2 = w.get(&format!("{p}.layer_scale2.lambda1"), &[d])?.to_f32();
    let h = scaled_linear(g, w, h, &format!("{p}.mlp.fc2"), (d, 4 * d), &ls2)?;
    g.add(x, h)
}

fn conv(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    (out, inp, k): (usize, usize, usize),
    bias: bool,
    c: Conv2d,
) -> Result<Value, Error> {
    let wt = konst(g, w, &format!("{p}.weight"), &[out, inp, k, k])?;
    let b = if bias {
        Some(konst(g, w, &format!("{p}.bias"), &[out])?)
    } else {
        None
    };
    g.conv2d(x, wt, b, c)
}

/// A pre-activation residual unit: x + conv(relu(conv(relu(x)))).
fn residual(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    f: usize,
) -> Result<Value, Error> {
    let h = g.act(x, Act::Relu)?;
    let h = conv(
        g,
        w,
        h,
        &format!("{p}.convolution1"),
        (f, f, 3),
        true,
        Conv2d::same(3, 1).act(Act::Relu),
    )?;
    let h = conv(
        g,
        w,
        h,
        &format!("{p}.convolution2"),
        (f, f, 3),
        true,
        Conv2d::same(3, 1),
    )?;
    g.add(x, h)
}

/// Relative inverse depth `[1, 1, H, W]` from a normalised `[1, 3, H, W]` input (sides
/// multiples of [`PATCH`]).
pub fn depth(
    g: &mut Graph,
    x: Value,
    w: &mut dyn Weights,
    c: &DepthConfig,
) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4
        || s[0] != 1
        || s[1] != 3
        || !s[2].is_multiple_of(PATCH)
        || !s[3].is_multiple_of(PATCH)
        || s[2] < 2 * PATCH
        || s[3] < 2 * PATCH
    {
        return Err(Error::Shape(format!(
            "depth wants [1, 3, H, W] with sides multiples of {PATCH}: {s:?}"
        )));
    }
    let d = c.dim;
    let (ph, pw) = (s[2] / PATCH, s[3] / PATCH);
    let t = ph * pw;
    // Embeddings.
    let e = format!("{B}.embeddings");
    let pe = conv(
        g,
        w,
        x,
        &format!("{e}.patch_embeddings.projection"),
        (d, 3, PATCH),
        true,
        Conv2d {
            stride: [PATCH, PATCH],
            ..Default::default()
        },
    )?;
    let pe = g.reshape(pe, &[1, d, t])?;
    let pe = g.transpose(pe, &[0, 2, 1])?;
    let cls = konst(g, w, &format!("{e}.cls_token"), &[1, 1, d])?;
    let mut h = g.concat(&[cls, pe], 1)?;
    let n = 1 + c.grid * c.grid;
    let table = w
        .get(&format!("{e}.position_embeddings"), &[1, n, d])?
        .to_f32();
    let pos = g.constant(resize_positions(&table, c.grid, d, (ph, pw)))?;
    h = g.add(h, pos)?;
    // Blocks, keeping the tapped outputs as maps.
    let mut maps = Vec::with_capacity(4);
    for i in 0..c.layers {
        h = block(g, w, h, c, i)?;
        if c.taps.contains(&(i + 1)) {
            let f = layer_norm(g, w, h, &format!("{B}.layernorm"), d, EPS)?;
            let f = g.slice_axis(f, 1, 1, 1 + t)?;
            let f = g.transpose(f, &[0, 2, 1])?;
            maps.push(g.reshape(f, &[1, d, ph, pw])?);
        }
    }
    // Reassemble.
    let mut feats = Vec::with_capacity(4);
    for (i, &m) in maps.iter().enumerate() {
        let ch = c.neck[i];
        let r = format!("neck.reassemble_stage.layers.{i}");
        let y = conv(
            g,
            w,
            m,
            &format!("{r}.projection"),
            (ch, d, 1),
            true,
            Conv2d::default(),
        )?;
        let y = match i {
            0 | 1 => {
                let k = if i == 0 { 4 } else { 2 };
                let wt = konst(g, w, &format!("{r}.resize.weight"), &[ch, ch, k, k])?;
                let b = konst(g, w, &format!("{r}.resize.bias"), &[ch])?;
                g.conv_transpose2d(y, wt, Some(b), [k, k], [0; 4], Act::Identity)?
            }
            2 => y,
            _ => conv(
                g,
                w,
                y,
                &format!("{r}.resize"),
                (ch, ch, 3),
                true,
                Conv2d::same(3, 2),
            )?,
        };
        let y = conv(
            g,
            w,
            y,
            &format!("neck.convs.{i}"),
            (c.fusion, ch, 3),
            false,
            Conv2d::same(3, 1),
        )?;
        feats.push(y);
    }
    // Fusion, coarsest first.
    let f = c.fusion;
    let mut fused: Option<Value> = None;
    for (l, &feat) in feats.iter().rev().enumerate() {
        let p = format!("neck.fusion_stage.layers.{l}");
        let mut y = match fused {
            None => feat,
            Some(prev) => {
                let r = residual(g, w, feat, &format!("{p}.residual_layer1"), f)?;
                g.add(prev, r)?
            }
        };
        y = residual(g, w, y, &format!("{p}.residual_layer2"), f)?;
        let ys = g.shape(y).to_vec();
        let (th, tw) = if l + 1 < feats.len() {
            let next = g.shape(feats[feats.len() - 2 - l]).to_vec();
            (next[2], next[3])
        } else {
            (2 * ys[2], 2 * ys[3])
        };
        y = g.resize(y, th, tw, ResizeMode::BilinearAligned)?;
        y = conv(
            g,
            w,
            y,
            &format!("{p}.projection"),
            (f, f, 1),
            true,
            Conv2d::default(),
        )?;
        fused = Some(y);
    }
    let y = fused.expect("four levels");
    // Head.
    let y = conv(
        g,
        w,
        y,
        "head.conv1",
        (f / 2, f, 3),
        true,
        Conv2d::same(3, 1),
    )?;
    let y = g.resize(y, s[2], s[3], ResizeMode::BilinearAligned)?;
    let y = conv(
        g,
        w,
        y,
        "head.conv2",
        (c.head, f / 2, 3),
        true,
        Conv2d::same(3, 1).act(Act::Relu),
    )?;
    conv(
        g,
        w,
        y,
        "head.conv3",
        (1, c.head, 1),
        true,
        Conv2d::default().act(Act::Relu),
    )
}

/// The whole graph: an RGBA image in, relative inverse depth `[1, 1, h, w]` out, at the
/// input size `w × h` ([`input_size`]).
pub fn graph(
    w: &mut dyn Weights,
    c: &DepthConfig,
    (iw, ih): (usize, usize),
) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let img = g.image_input()?;
    let x = g.preprocess(img, preprocess(iw, ih))?;
    let y = depth(&mut g, x, w, c)?;
    g.output(y)?;
    Ok(g)
}

/// The graph on a normalised tensor input `[1, 3, h, w]` (the oracle feeds the reference's
/// own preprocessed pixels).
pub fn graph_from_tensor(
    w: &mut dyn Weights,
    c: &DepthConfig,
    (iw, ih): (usize, usize),
) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = g.input(&[1, 3, ih, iw], DType::F32)?;
    let y = depth(&mut g, x, w, c)?;
    g.output(y)?;
    Ok(g)
}

/// Tensors kept in f32 when a download is converted to f16 (norms, biases, layer scales,
/// the position table and class token).
pub fn keep_f32(t: &crate::safetensors::TensorInfo) -> bool {
    t.shape.len() <= 1 || t.name.contains("embeddings.position") || t.name.contains("cls_token")
}
