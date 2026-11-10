//! FFDNet (Zhang, Zuo and Zhang 2018, "FFDNet: Toward a Fast and Flexible Solution for
//! CNN-Based Image Denoising"; KAIR's colour weights, MIT): Develop's AI denoise,
//! the model that takes a noise level.
//!
//! Written from the paper and the checkpoint's tensor names and shapes (the deepinv
//! layout, `blocks.N…`); checked against deepinv's implementation run locally as an
//! oracle (never shipped).
//!
//! - The image (`[1, 3, H, W]`, H and W even) and a constant noise-level map (σ, as one
//!   more channel) go to half resolution by space to depth: 16 channels.
//! - Eleven 3×3 convs with ReLU (96 channels), a last 3×3 conv to 12 channels, depth to
//!   space back to full resolution: the clean image (not a residual).
//! - Trained on white Gaussian noise of σ (0 … 75/255) over images in 0..1. Develop feeds it
//!   the picture through the generalised Anscombe transform of its own noise model, where
//!   the noise is unit and white, scaled so white is about 1: σ is then known exactly.
//!
//! No op looks further than 12 half-resolution pixels (25 full-resolution ones), so tiles
//! with a 32-pixel margin join exactly.

use crate::Error;
use crate::graph::{Act, Conv2d, Graph, Value};
use crate::models::{Weights, konst};
use crate::tensor::{DType, Tensor};

/// Input sides are multiples of this.
pub const SIDE_STEP: usize = 2;
/// Pixels each output pixel depends on around it (full resolution).
pub const REACH: usize = 25;

/// Sizes, read from the checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FfdConfig {
    /// Image channels (3: colour).
    pub channels: usize,
    /// Feature channels.
    pub width: usize,
    /// Convolutions in all.
    pub layers: usize,
}

impl FfdConfig {
    /// KAIR's colour model.
    pub fn color() -> FfdConfig {
        FfdConfig {
            channels: 3,
            width: 96,
            layers: 12,
        }
    }

    /// From the tensor names and shapes of a checkpoint.
    pub fn from_header(h: &crate::safetensors::Header) -> Result<FfdConfig, Error> {
        let first = h
            .get("blocks.0.0.weight")
            .ok_or_else(|| Error::Weights("ffdnet: no blocks.0.0.weight".into()))?;
        let (width, inp) = match first.shape.as_slice() {
            [o, i, 3, 3] if i.is_multiple_of(4) && *i >= 8 => (*o, *i),
            s => return Err(Error::Weights(format!("ffdnet: first conv {s:?}"))),
        };
        let mid = (1..64)
            .take_while(|i| h.get(&format!("blocks.{i}.0.weight")).is_some())
            .count();
        let layers = mid + 2;
        if h.get(&format!("blocks.{}.weight", layers - 1)).is_none() {
            return Err(Error::Weights("ffdnet: no last conv".into()));
        }
        Ok(FfdConfig {
            channels: inp / 4 - 1,
            width,
            layers,
        })
    }
}

/// Space to depth by 2 (PyTorch's `PixelUnshuffle(2)`): channel `c·4 + i·2 + j` holds
/// pixels `(2y + i, 2x + j)` of channel `c`.
pub fn space_to_depth2(g: &mut Graph, x: Value) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4 || !s[2].is_multiple_of(2) || !s[3].is_multiple_of(2) {
        return Err(Error::Shape(format!("space to depth of {s:?}")));
    }
    let (n, c, h, w) = (s[0], s[1], s[2] / 2, s[3] / 2);
    let r = g.reshape(x, &[n, c, h, 2, w, 2])?;
    let t = g.transpose(r, &[0, 1, 3, 5, 2, 4])?;
    g.reshape(t, &[n, c * 4, h, w])
}

/// The clean image from `x` (`[1, C, H, W]`, H and W even) at noise level `sigma`
/// (`[1, 1, 1, 1]`, units of the image's range).
pub fn denoise(
    g: &mut Graph,
    x: Value,
    sigma: Value,
    w: &mut dyn Weights,
    c: &FfdConfig,
) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4
        || s[0] != 1
        || s[1] != c.channels
        || !s[2].is_multiple_of(2)
        || !s[3].is_multiple_of(2)
    {
        return Err(Error::Shape(format!(
            "ffdnet wants [1, {}, H, W] with even sides: {s:?}",
            c.channels
        )));
    }
    let ones = g.constant(Tensor::f32(&[1, 1, s[2], s[3]], vec![1.0; s[2] * s[3]]))?;
    let map = g.mul(ones, sigma)?;
    let cat = g.concat(&[x, map], 1)?;
    let mut h = space_to_depth2(g, cat)?;
    let inp = 4 * (c.channels + 1);
    for i in 0..c.layers - 1 {
        let cin = if i == 0 { inp } else { c.width };
        let wt = konst(g, w, &format!("blocks.{i}.0.weight"), &[c.width, cin, 3, 3])?;
        let b = konst(g, w, &format!("blocks.{i}.0.bias"), &[c.width])?;
        h = g.conv2d(h, wt, Some(b), Conv2d::same(3, 1).act(Act::Relu))?;
    }
    let last = c.layers - 1;
    let out = 4 * c.channels;
    let wt = konst(
        g,
        w,
        &format!("blocks.{last}.weight"),
        &[out, c.width, 3, 3],
    )?;
    let b = konst(g, w, &format!("blocks.{last}.bias"), &[out])?;
    let h = g.conv2d(h, wt, Some(b), Conv2d::same(3, 1))?;
    g.depth_to_space(h, 2)
}

/// The graph for one `w × h` tile: inputs the image `[1, C, h, w]` and σ `[1, 1, 1, 1]`.
pub fn graph(w: &mut dyn Weights, c: &FfdConfig, (tw, th): (usize, usize)) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let x = g.input(&[1, c.channels, th, tw], DType::F32)?;
    let sigma = g.input(&[1, 1, 1, 1], DType::F32)?;
    let y = denoise(&mut g, x, sigma, w, c)?;
    g.output(y)?;
    Ok(g)
}
