//! Big-LaMa (Suvorov et al. 2022, "Resolution-robust Large Mask Inpainting with Fourier
//! Convolutions"; Apache-2.0): Develop's "Remove (AI)" spots. Read from the
//! generator's state dict (`generator.model.*`, a safetensors conversion of the authors'
//! release); written from the paper, the published config and the checkpoint's tensor
//! names and shapes, checked against the authors' code run locally as an oracle (never
//! shipped).
//!
//! - **Input** `[1, 4, H, W]` (sides multiples of [`SIDE_STEP`]): the picture in 0..1 with
//!   the hole set to 0, then the hole mask (1 = fill).
//! - **Encoder**: reflect pad 3, 7×7 conv to 64; three 3×3 stride-2 convs (reflect padded)
//!   to 128, 256, then 512 channels split into a local quarter (128) and a global rest (384).
//! - **18 FFC blocks** at an eighth of the input: each of two fast Fourier convolutions maps
//!   (local, global) to (local, global): local out = 3×3 conv of both; global out = 3×3
//!   conv of the local plus the spectral transform of the global (1×1 conv to half, then a
//!   Fourier unit: `rfft2`, a 1×1 conv over the real and imaginary parts, `irfft2`, added
//!   back, 1×1 conv); ReLU after each; both halves residual.
//! - **Decoder**: three 3×3 stride-2 transposed convs (512 → 256 → 128 → 64), reflect pad 3,
//!   a 7×7 conv to RGB and a sigmoid.
//! - Batch norms fold into their convs. The local and global inputs of a local output share
//!   one conv (their weights side by side), so the network is kept as one 512-channel map,
//!   local channels first.

use crate::Error;
use crate::graph::{Act, BatchNorm, Conv2d, Dft, Graph, Value, fold_batchnorm};
use crate::models::Weights;
use crate::tensor::{DType, Tensor};

/// Input sides are multiples of this (three stride-2 downsamplings).
pub const SIDE_STEP: usize = 8;
/// Downsampling steps (the FFC blocks run at 1/2^this of the input).
const DOWN: usize = 3;
const BLOCKS: usize = 18;
const NGF: usize = 64;
/// The local share of the bottleneck's channels (ratio_gin = ratio_gout = 0.75).
const LOCAL: usize = 128;
const GLOBAL: usize = 384;
const P: &str = "generator.model";

/// Smallest side the network takes: the bottleneck must be at least 2 px for the reflect
/// padding of its 3×3 convs.
pub const MIN_SIDE: usize = 16;

fn get_f32(w: &mut dyn Weights, name: &str, shape: &[usize]) -> Result<Vec<f32>, Error> {
    Ok(w.get(name, shape)?.to_f32())
}

fn bn(w: &mut dyn Weights, p: &str, c: usize) -> Result<BatchNorm, Error> {
    Ok(BatchNorm {
        gamma: get_f32(w, &format!("{p}.weight"), &[c])?,
        beta: get_f32(w, &format!("{p}.bias"), &[c])?,
        mean: get_f32(w, &format!("{p}.running_mean"), &[c])?,
        var: get_f32(w, &format!("{p}.running_var"), &[c])?,
        eps: 1e-5,
    })
}

/// Stores `t` as the checkpoint stored its matrices (f16 stays f16).
fn like(t: Tensor, f16: bool) -> Tensor {
    if f16 { t.to_f16() } else { t }
}

/// Conv weights `[O, C, K, K]` with a batch norm folded in (no bias of their own).
fn folded(
    w: &mut dyn Weights,
    name: &str,
    shape: [usize; 4],
    norm: &str,
) -> Result<(Tensor, Tensor, bool), Error> {
    let norm = bn(w, norm, shape[0])?;
    let t = w.get(name, &shape)?;
    let f16 = t.dtype() == DType::F16;
    let (wf, bf) = fold_batchnorm(&Tensor::f32(&t.shape, t.to_f32()), None, &norm)?;
    Ok((wf, bf, f16))
}

/// A conv of `x` (already padded) with folded weights; `act` after the bias.
fn conv(
    g: &mut Graph,
    x: Value,
    (wt, b, f16): (Tensor, Tensor, bool),
    stride: usize,
    act: Act,
) -> Result<Value, Error> {
    let wv = g.constant(like(wt, f16))?;
    let bv = g.constant(b)?;
    let c = Conv2d {
        stride: [stride; 2],
        ..Default::default()
    }
    .act(act);
    g.conv2d(x, wv, Some(bv), c)
}

/// `rfft2` (orthonormal) of `x [1, C, H, W]` as LaMa's Fourier unit lays it out:
/// `[1, 2C, H, W/2 + 1]`, real and imaginary parts of each channel side by side.
pub fn rfft2(g: &mut Graph, x: Value) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    let f = g.dft(x, Dft::RealToComplex)?;
    let f = g.dft(f, Dft::Complex { inverse: false })?;
    let wr = s[3] / 2 + 1;
    let f = g.transpose(f, &[0, 1, 4, 2, 3])?;
    g.reshape(f, &[s[0], 2 * s[1], s[2], wr])
}

/// The inverse of [`rfft2`] back to `[1, C, h, w]`.
pub fn irfft2(g: &mut Graph, f: Value, (h, w): (usize, usize)) -> Result<Value, Error> {
    let s = g.shape(f).to_vec();
    let c = s[1] / 2;
    let f = g.reshape(f, &[s[0], c, 2, h, s[3]])?;
    let f = g.transpose(f, &[0, 1, 3, 4, 2])?;
    let f = g.dft(f, Dft::Complex { inverse: true })?;
    g.dft(f, Dft::ComplexToReal { n: w })
}

/// One fast Fourier convolution with its norms and ReLU, on the 512-channel map `x`
/// (local channels first); returns the new map.
fn ffc(g: &mut Graph, w: &mut dyn Weights, x: Value, p: &str) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    let (h, wd) = (s[2], s[3]);
    let f = format!("{p}.ffc");
    let bn_l = bn(w, &format!("{p}.bn_l"), LOCAL)?;
    let bn_g = bn(w, &format!("{p}.bn_g"), GLOBAL)?;
    // Local out: both inputs through one 3×3 conv.
    let l2l = w.get(&format!("{f}.convl2l.weight"), &[LOCAL, LOCAL, 3, 3])?;
    let g2l = w.get(&format!("{f}.convg2l.weight"), &[LOCAL, GLOBAL, 3, 3])?;
    let f16 = l2l.dtype() == DType::F16;
    let (l2l, g2l) = (l2l.to_f32(), g2l.to_f32());
    let mut both = Vec::with_capacity(LOCAL * (LOCAL + GLOBAL) * 9);
    for o in 0..LOCAL {
        both.extend_from_slice(&l2l[o * LOCAL * 9..(o + 1) * LOCAL * 9]);
        both.extend_from_slice(&g2l[o * GLOBAL * 9..(o + 1) * GLOBAL * 9]);
    }
    let both = Tensor::f32(&[LOCAL, LOCAL + GLOBAL, 3, 3], both);
    let (wl, bl) = fold_batchnorm(&both, None, &bn_l)?;
    let padded = g.reflect_pad(x, [1; 4])?;
    let out_l = conv(g, padded, (wl, bl, f16), 1, Act::Relu)?;
    // Global out: the local input's 3×3 conv plus the spectral transform, one norm.
    let pl = g.slice_axis(padded, 1, 0, LOCAL)?;
    let l2g = folded(
        w,
        &format!("{f}.convl2g.weight"),
        [GLOBAL, LOCAL, 3, 3],
        &format!("{p}.bn_g"),
    )?;
    let a = conv(g, pl, l2g, 1, Act::Identity)?;
    let xg = g.slice_axis(x, 1, LOCAL, LOCAL + GLOBAL)?;
    let st = format!("{f}.convg2g");
    let half = GLOBAL / 2;
    let c1 = folded(
        w,
        &format!("{st}.conv1.0.weight"),
        [half, GLOBAL, 1, 1],
        &format!("{st}.conv1.1"),
    )?;
    let x1 = conv(g, xg, c1, 1, Act::Relu)?;
    let spec = rfft2(g, x1)?;
    let fu = folded(
        w,
        &format!("{st}.fu.conv_layer.weight"),
        [2 * half, 2 * half, 1, 1],
        &format!("{st}.fu.bn"),
    )?;
    let spec = conv(g, spec, fu, 1, Act::Relu)?;
    let back = irfft2(g, spec, (h, wd))?;
    let sum = g.add(x1, back)?;
    // conv2 (no bias) with the global norm's scale; its shift is already in `a`.
    let c2 = w.get(&format!("{st}.conv2.weight"), &[GLOBAL, half, 1, 1])?;
    let (scale, _) = bn_g.scale_shift();
    let mut c2w = c2.to_f32();
    for (o, row) in c2w.chunks_mut(half).enumerate() {
        row.iter_mut().for_each(|v| *v *= scale[o]);
    }
    let c2v = g.constant(like(Tensor::f32(&[GLOBAL, half, 1, 1], c2w), f16))?;
    let b = g.conv2d(sum, c2v, None, Conv2d::default())?;
    let out_g = g.add(a, b)?;
    let out_g = g.act(out_g, Act::Relu)?;
    g.concat(&[out_l, out_g], 1)
}

/// A transposed 3×3 stride-2 conv (output padding 1) with its batch norm folded, ReLU.
fn up(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    (i, j): (usize, usize),
    (cin, cout): (usize, usize),
) -> Result<Value, Error> {
    let t = w.get(&format!("{P}.{i}.weight"), &[cin, cout, 3, 3])?;
    let f16 = t.dtype() == DType::F16;
    let b = get_f32(w, &format!("{P}.{i}.bias"), &[cout])?;
    let (scale, shift) = bn(w, &format!("{P}.{j}"), cout)?.scale_shift();
    let mut wf = t.to_f32();
    for (k, v) in wf.iter_mut().enumerate() {
        *v *= scale[(k / 9) % cout];
    }
    let bf: Vec<f32> = (0..cout).map(|o| b[o] * scale[o] + shift[o]).collect();
    // As a plain conv over the input spread out with zeros (x at even places): the
    // transposed conv's taps, flipped, with one pixel of zero padding each side. The conv
    // path splits large work where the gather kernel of a transposed conv ran as one long
    // dispatch (a 448² crop lost the iGPU device).
    let mut flipped = vec![0f32; cout * cin * 9];
    for c in 0..cin {
        for o in 0..cout {
            for k in 0..9 {
                flipped[(o * cin + c) * 9 + (8 - k)] = wf[(c * cout + o) * 9 + k];
            }
        }
    }
    let wv = g.constant(like(Tensor::f32(&[cout, cin, 3, 3], flipped), f16))?;
    let bv = g.constant(Tensor::f32(&[cout], bf))?;
    let z = spread(g, x)?;
    g.conv2d(z, wv, Some(bv), Conv2d::same(3, 1).act(Act::Relu))
}

/// `x [1, C, H, W]` to `[1, C, 2H, 2W]` with `x` at even rows and columns, zeros between.
pub fn spread(g: &mut Graph, x: Value) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    let (c, h, w) = (s[1], s[2], s[3]);
    let cols = g.reshape(x, &[1, c, h, w, 1])?;
    let zero = g.unary(cols, Act::Identity, 0.0, 0.0)?;
    let cols = g.concat(&[cols, zero], 4)?;
    let rows = g.reshape(cols, &[1, c, h, 1, 2 * w])?;
    let zero = g.unary(rows, Act::Identity, 0.0, 0.0)?;
    let rows = g.concat(&[rows, zero], 3)?;
    g.reshape(rows, &[1, c, 2 * h, 2 * w])
}

/// The generator on an input `x [1, 4, H, W]`; returns RGB `[1, 3, H, W]` in 0..1.
pub fn generator(g: &mut Graph, w: &mut dyn Weights, x: Value) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4
        || s[1] != 4
        || !s[2].is_multiple_of(SIDE_STEP)
        || !s[3].is_multiple_of(SIDE_STEP)
        || s[2].min(s[3]) < MIN_SIDE
    {
        return Err(Error::Shape(format!(
            "lama wants [1, 4, H, W], sides multiples of {SIDE_STEP} and at least {MIN_SIDE}: {s:?}"
        )));
    }
    let pad = g.reflect_pad(x, [3; 4])?;
    let c = folded(
        w,
        &format!("{P}.1.ffc.convl2l.weight"),
        [NGF, 4, 7, 7],
        &format!("{P}.1.bn_l"),
    )?;
    let mut h = conv(g, pad, c, 1, Act::Relu)?;
    let mut ch = NGF;
    for i in 0..DOWN {
        let m = 2 + i;
        let p = g.reflect_pad(h, [1; 4])?;
        let l = folded(
            w,
            &format!("{P}.{m}.ffc.convl2l.weight"),
            [if i + 1 == DOWN { LOCAL } else { 2 * ch }, ch, 3, 3],
            &format!("{P}.{m}.bn_l"),
        )?;
        if i + 1 < DOWN {
            h = conv(g, p, l, 2, Act::Relu)?;
        } else {
            let gl = folded(
                w,
                &format!("{P}.{m}.ffc.convl2g.weight"),
                [GLOBAL, ch, 3, 3],
                &format!("{P}.{m}.bn_g"),
            )?;
            // One conv for both outputs: the local rows, then the global.
            let wt = Tensor::f32(
                &[LOCAL + GLOBAL, ch, 3, 3],
                [l.0.to_f32(), gl.0.to_f32()].concat(),
            );
            let bt = Tensor::f32(&[LOCAL + GLOBAL], [l.1.to_f32(), gl.1.to_f32()].concat());
            h = conv(g, p, (wt, bt, l.2), 2, Act::Relu)?;
        }
        ch *= 2;
    }
    for b in 0..BLOCKS {
        let m = 2 + DOWN + b;
        let y = ffc(g, w, h, &format!("{P}.{m}.conv1"))?;
        let y = ffc(g, w, y, &format!("{P}.{m}.conv2"))?;
        h = g.add(h, y)?;
    }
    // After the concat layer (index 2 + DOWN + BLOCKS): up-sampling triples.
    let mut i = 3 + DOWN + BLOCKS;
    for _ in 0..DOWN {
        h = up(g, w, h, (i, i + 1), (ch, ch / 2))?;
        ch /= 2;
        i += 3;
    }
    let p = g.reflect_pad(h, [3; 4])?;
    let i = i + 1;
    let ow = w.get(&format!("{P}.{i}.weight"), &[3, NGF, 7, 7])?;
    let ob = w.get(&format!("{P}.{i}.bias"), &[3])?;
    let ow = g.constant(ow)?;
    let ob = g.constant(ob)?;
    g.conv2d(p, ow, Some(ob), Conv2d::default().act(Act::Sigmoid))
}

/// The whole graph for an `w × h` input: input `[1, 4, h, w]`, output `[1, 3, h, w]`.
pub fn graph(weights: &mut dyn Weights, (w, h): (usize, usize)) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = g.input(&[1, 4, h, w], DType::F32)?;
    let y = generator(&mut g, weights, x)?;
    g.output(y)?;
    Ok(g)
}

/// The network input for an RGB picture `rgb` (`w × h`, 0..1, row-major interleaved) and
/// its hole `mask` (0..1, ≥ ½ is a hole): `[1, 4, h, w]` as LaMa's training fed it.
pub fn input(rgb: &[f32], mask: &[f32], (w, h): (usize, usize)) -> Tensor {
    let n = w * h;
    let mut v = vec![0f32; 4 * n];
    for i in 0..n {
        let m = if mask[i] >= 0.5 { 1.0 } else { 0.0 };
        for c in 0..3 {
            v[c * n + i] = rgb[3 * i + c] * (1.0 - m);
        }
        v[3 * n + i] = m;
    }
    Tensor::f32(&[1, 4, h, w], v)
}

/// Tensors kept in f32 when a download is converted to f16 (norm statistics and biases).
pub fn keep_f32(t: &crate::safetensors::TensorInfo) -> bool {
    t.shape.len() <= 1
}
