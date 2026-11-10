//! UperNet with a ConvNeXt backbone (Xiao et al. 2018; Liu et al. 2022), trained on
//! ADE20K's 150 classes, read from the Hugging Face `UperNetForSemanticSegmentation`
//! checkpoint (MIT): Develop's "Sky" mask, and later people, water, vegetation and
//! buildings from the same pass.
//!
//! Written from the papers and the checkpoint's tensor names and shapes; checked against
//! the `transformers` implementation run locally as an oracle (never shipped).
//!
//! - **Backbone**: a 4×4 stride-4 stem and channel norm, then four stages of ConvNeXt
//!   blocks (7×7 depthwise conv, channel norm, two pointwise convs with GELU, a learnt
//!   per-channel scale folded into the second, residual), stages joined by a channel norm
//!   and a 2×2 stride-2 conv; each stage's output channel-normed.
//! - **Head**: a pyramid pooling module on the last stage (average pools to 1, 2, 3 and 6
//!   bins, 1×1 conv-BN-ReLU each, upsampled bilinearly, concatenated, 3×3 conv), a feature
//!   pyramid (1×1 laterals, top-down bilinear adds, 3×3 convs), all levels upsampled to the
//!   finest and fused by a 3×3 conv, then a 1×1 classifier at a quarter of the input.
//! - Batch norms fold into their convs. Every pool bin is whole because the input sides are
//!   multiples of [`SIDE_STEP`] (32 × 6), so adaptive pooling is a plain average pool.

use crate::Error;
use crate::graph::{
    Act, BatchNorm, Conv2d, Crop, Graph, Pool2d, PoolKind, Preprocess, ResizeMode, Value,
    fold_batchnorm,
};
use crate::models::{Weights, konst};
use crate::tensor::{DType, Tensor};

/// Input sides are multiples of this (the backbone's stride times the largest pool bin
/// count).
pub const SIDE_STEP: usize = 192;
/// ADE20K classes.
pub const CLASSES: usize = 150;
/// ADE20K class ids (0-based, as the checkpoint's `id2label`).
pub mod class {
    pub const WALL: usize = 0;
    pub const BUILDING: usize = 1;
    pub const SKY: usize = 2;
    pub const TREE: usize = 4;
    pub const PERSON: usize = 12;
    pub const WATER: usize = 21;
    pub const MOUNTAIN: usize = 16;
    pub const GRASS: usize = 9;
    pub const PLANT: usize = 17;
    pub const SEA: usize = 26;
}
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const POOLS: [usize; 4] = [1, 2, 3, 6];
const HEAD: usize = 512;

/// Backbone sizes, read from the checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UperConfig {
    pub widths: [usize; 4],
    pub depths: [usize; 4],
}

impl UperConfig {
    /// ConvNeXt-T.
    pub fn tiny() -> UperConfig {
        UperConfig {
            widths: [96, 192, 384, 768],
            depths: [3, 3, 9, 3],
        }
    }

    /// From the tensor names and shapes of a checkpoint.
    pub fn from_header(h: &crate::safetensors::Header) -> Result<UperConfig, Error> {
        let mut c = UperConfig {
            widths: [0; 4],
            depths: [0; 4],
        };
        for s in 0..4 {
            let p = format!("backbone.encoder.stages.{s}.layers.");
            let name = format!("{p}0.dwconv.bias");
            c.widths[s] = h
                .get(&name)
                .and_then(|t| t.shape.first().copied())
                .ok_or_else(|| Error::Weights(format!("upernet: no {name}")))?;
            c.depths[s] = (0..64)
                .take_while(|i| h.get(&format!("{p}{i}.dwconv.bias")).is_some())
                .count();
        }
        Ok(c)
    }
}

/// The input size for an image of `w × h`: the long side `long` (a multiple of
/// [`SIDE_STEP`]), the short side the nearest multiple keeping the aspect ratio (at least
/// one step).
pub fn input_size(w: usize, h: usize, long: usize) -> (usize, usize) {
    let long = long.max(SIDE_STEP) / SIDE_STEP * SIDE_STEP;
    let snap = |v: f64| ((v / SIDE_STEP as f64).round().max(1.0) as usize) * SIDE_STEP;
    if w >= h {
        (long, snap(long as f64 * h.max(1) as f64 / w.max(1) as f64))
    } else {
        (snap(long as f64 * w.max(1) as f64 / h.max(1) as f64), long)
    }
}

/// The preprocessing: stretched to `w × h`, area-filtered.
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

/// The whole graph: an RGBA image in, the probabilities of `classes` out
/// (`[1, classes.len(), h/4, w/4]`).
pub fn graph(
    w: &mut dyn Weights,
    c: &UperConfig,
    (iw, ih): (usize, usize),
    classes: &[usize],
) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let img = g.image_input()?;
    let x = g.preprocess(img, preprocess(iw, ih))?;
    let p = probabilities(&mut g, x, w, c, classes)?;
    g.output(p)?;
    Ok(g)
}

/// Class probabilities at a quarter of the input from a normalised `[1, 3, H, W]` input.
pub fn probabilities(
    g: &mut Graph,
    x: Value,
    w: &mut dyn Weights,
    c: &UperConfig,
    classes: &[usize],
) -> Result<Value, Error> {
    let logits = logits(g, x, w, c)?;
    let s = g.shape(logits).to_vec();
    let t = g.transpose(logits, &[0, 2, 3, 1])?;
    let sm = g.softmax(t)?;
    let sm = g.transpose(sm, &[0, 3, 1, 2])?;
    let mut parts = Vec::with_capacity(classes.len());
    for &k in classes {
        if k >= s[1] {
            return Err(Error::Shape(format!("upernet: class {k} of {}", s[1])));
        }
        parts.push(g.slice_axis(sm, 1, k, k + 1)?);
    }
    if parts.len() == 1 {
        Ok(parts[0])
    } else {
        g.concat(&parts, 1)
    }
}

fn get_f32(w: &mut dyn Weights, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
    let t = w.get(name, shape)?;
    Ok(Tensor::f32(&t.shape, t.to_f32()))
}

/// Stores `t` as the checkpoint stored its matrices (f16 stays f16).
fn like(t: Tensor, f16: bool) -> Tensor {
    if f16 { t.to_f16() } else { t }
}

fn channel_norm(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    d: usize,
) -> Result<Value, Error> {
    let gamma = konst(g, w, &format!("{p}.weight"), &[d])?;
    let beta = konst(g, w, &format!("{p}.bias"), &[d])?;
    g.channel_norm(x, gamma, beta, 1e-6)
}

/// A conv without bias, its batch norm folded in, then ReLU.
fn conv_bn_relu(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    (out, inp, k): (usize, usize, usize),
) -> Result<Value, Error> {
    let wt = w.get(&format!("{p}.conv.weight"), &[out, inp, k, k])?;
    let f16 = wt.dtype() == DType::F16;
    let mut bn = |n: &str| get_f32(w, &format!("{p}.batch_norm.{n}"), &[out]).map(|t| t.to_f32());
    let bn = BatchNorm {
        gamma: bn("weight")?,
        beta: bn("bias")?,
        mean: bn("running_mean")?,
        var: bn("running_var")?,
        eps: 1e-5,
    };
    let (wf, bf) = fold_batchnorm(&Tensor::f32(&wt.shape, wt.to_f32()), None, &bn)?;
    let wv = g.constant(like(wf, f16))?;
    let bv = g.constant(bf)?;
    g.conv2d(x, wv, Some(bv), Conv2d::same(k, 1).act(Act::Relu))
}

/// The ConvNeXt backbone's four stage outputs (channel-normed).
fn backbone(
    g: &mut Graph,
    x: Value,
    w: &mut dyn Weights,
    c: &UperConfig,
) -> Result<Vec<Value>, Error> {
    let b = "backbone";
    let d0 = c.widths[0];
    let sw = konst(
        g,
        w,
        &format!("{b}.embeddings.patch_embeddings.weight"),
        &[d0, 3, 4, 4],
    )?;
    let sb = konst(
        g,
        w,
        &format!("{b}.embeddings.patch_embeddings.bias"),
        &[d0],
    )?;
    let conv = Conv2d {
        stride: [4, 4],
        ..Default::default()
    };
    let mut h = g.conv2d(x, sw, Some(sb), conv)?;
    h = channel_norm(g, w, h, &format!("{b}.embeddings.layernorm"), d0)?;
    let mut outs = Vec::with_capacity(4);
    for s in 0..4 {
        let d = c.widths[s];
        let st = format!("{b}.encoder.stages.{s}");
        if s > 0 {
            let dp = c.widths[s - 1];
            h = channel_norm(g, w, h, &format!("{st}.downsampling_layer.0"), dp)?;
            let dw = konst(
                g,
                w,
                &format!("{st}.downsampling_layer.1.weight"),
                &[d, dp, 2, 2],
            )?;
            let db = konst(g, w, &format!("{st}.downsampling_layer.1.bias"), &[d])?;
            let conv = Conv2d {
                stride: [2, 2],
                ..Default::default()
            };
            h = g.conv2d(h, dw, Some(db), conv)?;
        }
        for i in 0..c.depths[s] {
            let l = format!("{st}.layers.{i}");
            let dw = konst(g, w, &format!("{l}.dwconv.weight"), &[d, 1, 7, 7])?;
            let db = konst(g, w, &format!("{l}.dwconv.bias"), &[d])?;
            let y = g.conv2d(h, dw, Some(db), Conv2d::same(7, 1).groups(d))?;
            let y = channel_norm(g, w, y, &format!("{l}.layernorm"), d)?;
            let w1 = w.get(&format!("{l}.pwconv1.weight"), &[4 * d, d])?;
            let w1 = g.constant(w1.reshaped(&[4 * d, d, 1, 1]))?;
            let b1 = konst(g, w, &format!("{l}.pwconv1.bias"), &[4 * d])?;
            let y = g.conv2d(y, w1, Some(b1), Conv2d::default().act(Act::Gelu))?;
            // The layer scale folds into the second pointwise conv.
            let w2 = w.get(&format!("{l}.pwconv2.weight"), &[d, 4 * d])?;
            let f16 = w2.dtype() == DType::F16;
            let scale = get_f32(w, &format!("{l}.layer_scale_parameter"), &[d])?.to_f32();
            let mut b2 = get_f32(w, &format!("{l}.pwconv2.bias"), &[d])?.to_f32();
            let mut w2 = w2.to_f32();
            for (o, row) in w2.chunks_mut(4 * d).enumerate() {
                row.iter_mut().for_each(|v| *v *= scale[o]);
                b2[o] *= scale[o];
            }
            let w2 = g.constant(like(Tensor::f32(&[d, 4 * d, 1, 1], w2), f16))?;
            let b2 = g.constant(Tensor::f32(&[d], b2))?;
            let y = g.conv2d(y, w2, Some(b2), Conv2d::default())?;
            h = g.add(h, y)?;
        }
        let n = format!("{b}.hidden_states_norms.stage{}", s + 1);
        outs.push(channel_norm(g, w, h, &n, d)?);
    }
    Ok(outs)
}

fn upsample(g: &mut Graph, x: Value, to: Value) -> Result<Value, Error> {
    let s = g.shape(to).to_vec();
    if g.shape(x)[2..] == s[2..] {
        return Ok(x);
    }
    g.resize(x, s[2], s[3], ResizeMode::Bilinear)
}

/// The classifier's logits `[1, 150, H/4, W/4]`.
pub fn logits(
    g: &mut Graph,
    x: Value,
    w: &mut dyn Weights,
    c: &UperConfig,
) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4 || !s[2].is_multiple_of(SIDE_STEP) || !s[3].is_multiple_of(SIDE_STEP) {
        return Err(Error::Shape(format!(
            "upernet wants sides that are multiples of {SIDE_STEP}: {s:?}"
        )));
    }
    let feats = backbone(g, x, w, c)?;
    let h = "decode_head";
    // Pyramid pooling on the last stage.
    let last = feats[3];
    let ls = g.shape(last).to_vec();
    let dl = c.widths[3];
    let mut psp = vec![last];
    for (i, &bins) in POOLS.iter().enumerate() {
        let (ky, kx) = (ls[2] / bins, ls[3] / bins);
        let pooled = g.pool2d(
            last,
            Pool2d {
                kind: PoolKind::Avg,
                kernel: [ky, kx],
                stride: [ky, kx],
                pad: [0; 4],
            },
        )?;
        let y = conv_bn_relu(
            g,
            w,
            pooled,
            &format!("{h}.psp_modules.{i}.1"),
            (HEAD, dl, 1),
        )?;
        psp.push(upsample(g, y, last)?);
    }
    let cat = g.concat(&psp, 1)?;
    let top = conv_bn_relu(
        g,
        w,
        cat,
        &format!("{h}.bottleneck"),
        (HEAD, dl + 4 * HEAD, 3),
    )?;
    // Feature pyramid.
    let mut lat = Vec::with_capacity(4);
    for (i, (&f, &d)) in feats.iter().zip(&c.widths).take(3).enumerate() {
        let p = format!("{h}.lateral_convs.{i}");
        lat.push(conv_bn_relu(g, w, f, &p, (HEAD, d, 1))?);
    }
    lat.push(top);
    for i in (1..4).rev() {
        let up = upsample(g, lat[i], lat[i - 1])?;
        lat[i - 1] = g.add(lat[i - 1], up)?;
    }
    let mut outs = Vec::with_capacity(4);
    for (i, &l) in lat.iter().enumerate().take(3) {
        outs.push(conv_bn_relu(
            g,
            w,
            l,
            &format!("{h}.fpn_convs.{i}"),
            (HEAD, HEAD, 3),
        )?);
    }
    outs.push(lat[3]);
    for i in 1..4 {
        outs[i] = upsample(g, outs[i], outs[0])?;
    }
    let cat = g.concat(&outs, 1)?;
    let fused = conv_bn_relu(
        g,
        w,
        cat,
        &format!("{h}.fpn_bottleneck"),
        (HEAD, 4 * HEAD, 3),
    )?;
    let cw = konst(
        g,
        w,
        &format!("{h}.classifier.weight"),
        &[CLASSES, HEAD, 1, 1],
    )?;
    let cb = konst(g, w, &format!("{h}.classifier.bias"), &[CLASSES])?;
    g.conv2d(fused, cw, Some(cb), Conv2d::default())
}

/// Tensors the runtime never reads (the training-only auxiliary head): left out when
/// converting a download.
pub fn unused(name: &str) -> bool {
    name.starts_with("auxiliary_head.") || name.ends_with("num_batches_tracked")
}
