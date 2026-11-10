//! NAFNet (Chen, Chu, Zhang and Sun 2022, "Simple Baselines for Image Restoration", MIT),
//! the SIDD denoisers: Develop's AI denoise.
//!
//! Written from the paper and the checkpoint's tensor names and shapes; checked against the
//! authors' implementation run locally as an oracle (never shipped).
//!
//! - A 3×3 conv to `width` channels, then a U-Net: four encoder levels of NAF blocks, each
//!   followed by a 2×2 stride-2 conv doubling the channels, middle blocks, and four decoder
//!   levels, each started by a 1×1 conv and a depth-to-space by 2 (halving the channels)
//!   plus the encoder's skip; a 3×3 conv back to RGB, added to the input.
//! - **NAF block** (no activation functions): channel norm, 1×1 conv to twice the
//!   channels, 3×3 depthwise conv, simple gate (one half of the channels times the other),
//!   simplified channel attention (the map times a 1×1 conv of its mean), 1×1 conv, scaled
//!   by a learnt β and added; then channel norm, 1×1 conv, simple gate, 1×1 conv, scaled by
//!   γ and added. β and γ fold into the convs before them.
//! - The input is sRGB-encoded RGB in 0..1, `[1, 3, H, W]` with H and W multiples of
//!   [`SIDE_STEP`]; the output is the denoised image in the same encoding.
//!
//! The channel attention takes the mean over the whole input, so a tile's result depends a
//! little on the tile: tiles of one size, overlapped and blended, keep that invisible.

use crate::Error;
use crate::graph::{Conv2d, Graph, Value};
use crate::models::{Weights, konst};
use crate::tensor::{DType, Tensor};

/// Input sides are multiples of this (four stride-2 levels).
pub const SIDE_STEP: usize = 16;
const EPS: f32 = 1e-6;

/// Network sizes, read from the checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NafConfig {
    pub width: usize,
    pub enc: Vec<usize>,
    pub middle: usize,
    pub dec: Vec<usize>,
}

impl NafConfig {
    /// NAFNet-SIDD-width32 (and width64 with `width` 64).
    pub fn sidd(width: usize) -> NafConfig {
        NafConfig {
            width,
            enc: vec![2, 2, 4, 8],
            middle: 12,
            dec: vec![2, 2, 2, 2],
        }
    }

    /// From the tensor names and shapes of a checkpoint.
    pub fn from_header(h: &crate::safetensors::Header) -> Result<NafConfig, Error> {
        let width = h
            .get("intro.bias")
            .and_then(|t| t.shape.first().copied())
            .ok_or_else(|| Error::Weights("nafnet: no intro.bias".into()))?;
        let blocks = |p: &str| {
            (0..256)
                .take_while(|i| h.get(&format!("{p}{i}.beta")).is_some())
                .count()
        };
        let levels = |p: &str| {
            (0..8)
                .take_while(|l| h.get(&format!("{p}.{l}.0.beta")).is_some())
                .map(|l| blocks(&format!("{p}.{l}.")))
                .collect::<Vec<_>>()
        };
        let c = NafConfig {
            width,
            enc: levels("encoders"),
            middle: blocks("middle_blks."),
            dec: levels("decoders"),
        };
        if c.enc.is_empty() || c.enc.len() != c.dec.len() || c.enc.len() > 6 {
            return Err(Error::Weights(format!(
                "nafnet: levels {:?} / {:?}",
                c.enc, c.dec
            )));
        }
        Ok(c)
    }

    /// The side multiple the input needs (2 per level).
    pub fn side_step(&self) -> usize {
        1 << self.enc.len()
    }
}

fn get_f32(w: &mut dyn Weights, name: &str, shape: &[usize]) -> Result<Vec<f32>, Error> {
    Ok(w.get(name, shape)?.to_f32())
}

/// A conv's weight scaled per output channel by `scale` (β or γ), stored as the checkpoint
/// stored it, with its scaled bias.
fn scaled_conv(
    g: &mut Graph,
    w: &mut dyn Weights,
    p: &str,
    (out, inp): (usize, usize),
    scale: &[f32],
) -> Result<(Value, Value), Error> {
    let wt = w.get(&format!("{p}.weight"), &[out, inp, 1, 1])?;
    let f16 = wt.dtype() == DType::F16;
    let mut wv = wt.to_f32();
    let mut bv = get_f32(w, &format!("{p}.bias"), &[out])?;
    for (o, row) in wv.chunks_mut(inp).enumerate() {
        row.iter_mut().for_each(|v| *v *= scale[o]);
        bv[o] *= scale[o];
    }
    let t = Tensor::f32(&[out, inp, 1, 1], wv);
    let wv = g.constant(if f16 { t.to_f16() } else { t })?;
    let bv = g.constant(Tensor::f32(&[out], bv))?;
    Ok((wv, bv))
}

fn conv(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    (out, inp, k): (usize, usize, usize),
    c: Conv2d,
    bias: bool,
) -> Result<Value, Error> {
    let wv = konst(g, w, &format!("{p}.weight"), &[out, inp / c.groups, k, k])?;
    let bv = if bias {
        Some(konst(g, w, &format!("{p}.bias"), &[out])?)
    } else {
        None
    };
    g.conv2d(x, wv, bv, c)
}

fn channel_norm(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    c: usize,
) -> Result<Value, Error> {
    let gamma = konst(g, w, &format!("{p}.weight"), &[c])?;
    let beta = konst(g, w, &format!("{p}.bias"), &[c])?;
    g.channel_norm(x, gamma, beta, EPS)
}

/// One NAF block on `x` (`c` channels).
fn block(g: &mut Graph, w: &mut dyn Weights, x: Value, p: &str, c: usize) -> Result<Value, Error> {
    let beta = get_f32(w, &format!("{p}.beta"), &[1, c, 1, 1])?;
    let gamma = get_f32(w, &format!("{p}.gamma"), &[1, c, 1, 1])?;
    let y = channel_norm(g, w, x, &format!("{p}.norm1"), c)?;
    let y = conv(
        g,
        w,
        y,
        &format!("{p}.conv1"),
        (2 * c, c, 1),
        Conv2d::default(),
        true,
    )?;
    let dw = Conv2d::same(3, 1).groups(2 * c);
    let y = conv(g, w, y, &format!("{p}.conv2"), (2 * c, 2 * c, 3), dw, true)?;
    let y = g.gate(y)?;
    // Simplified channel attention.
    let m = g.global_avg_pool(y)?;
    let a = conv(
        g,
        w,
        m,
        &format!("{p}.sca.1"),
        (c, c, 1),
        Conv2d::default(),
        true,
    )?;
    let y = g.mul(y, a)?;
    let (w3, b3) = scaled_conv(g, w, &format!("{p}.conv3"), (c, c), &beta)?;
    let y = g.conv2d(y, w3, Some(b3), Conv2d::default())?;
    let x = g.add(x, y)?;
    let y = channel_norm(g, w, x, &format!("{p}.norm2"), c)?;
    let y = conv(
        g,
        w,
        y,
        &format!("{p}.conv4"),
        (2 * c, c, 1),
        Conv2d::default(),
        true,
    )?;
    let y = g.gate(y)?;
    let (w5, b5) = scaled_conv(g, w, &format!("{p}.conv5"), (c, c), &gamma)?;
    let y = g.conv2d(y, w5, Some(b5), Conv2d::default())?;
    g.add(x, y)
}

/// The denoised image from `x` (`[1, 3, H, W]`, sRGB-encoded 0..1, sides multiples of
/// [`NafConfig::side_step`]).
pub fn denoise(
    g: &mut Graph,
    x: Value,
    w: &mut dyn Weights,
    c: &NafConfig,
) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    let step = c.side_step();
    if s.len() != 4
        || s[1] != 3
        || !s[2].is_multiple_of(step)
        || !s[3].is_multiple_of(step)
        || s[2] == 0
        || s[3] == 0
    {
        return Err(Error::Shape(format!(
            "nafnet wants [N, 3, H, W] with sides multiples of {step}: {s:?}"
        )));
    }
    let mut ch = c.width;
    let mut h = conv(g, w, x, "intro", (ch, 3, 3), Conv2d::same(3, 1), true)?;
    let mut skips = Vec::with_capacity(c.enc.len());
    for (l, &n) in c.enc.iter().enumerate() {
        for i in 0..n {
            h = block(g, w, h, &format!("encoders.{l}.{i}"), ch)?;
        }
        skips.push(h);
        let down = Conv2d {
            stride: [2, 2],
            ..Default::default()
        };
        h = conv(g, w, h, &format!("downs.{l}"), (2 * ch, ch, 2), down, true)?;
        ch *= 2;
    }
    for i in 0..c.middle {
        h = block(g, w, h, &format!("middle_blks.{i}"), ch)?;
    }
    for (l, &n) in c.dec.iter().enumerate() {
        h = conv(
            g,
            w,
            h,
            &format!("ups.{l}.0"),
            (2 * ch, ch, 1),
            Conv2d::default(),
            false,
        )?;
        h = g.depth_to_space(h, 2)?;
        ch /= 2;
        let skip = skips.pop().expect("one skip per level");
        h = g.add(h, skip)?;
        for i in 0..n {
            h = block(g, w, h, &format!("decoders.{l}.{i}"), ch)?;
        }
    }
    let out = conv(g, w, h, "ending", (3, ch, 3), Conv2d::same(3, 1), true)?;
    g.add(out, x)
}

/// The whole graph for one `w × h` tile: `[1, 3, h, w]` in, the same out.
pub fn graph(w: &mut dyn Weights, c: &NafConfig, (tw, th): (usize, usize)) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = g.input(&[1, 3, th, tw], DType::F32)?;
    let y = denoise(&mut g, x, w, c)?;
    g.output(y)?;
    Ok(g)
}

/// Tensors the runtime never reads: left out when converting a download.
pub fn unused(name: &str) -> bool {
    name.ends_with("num_batches_tracked")
}
