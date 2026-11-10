//! EfficientSAM (Xiong et al. 2023, Apache-2.0): "segment anything" from clicks and a box,
//! for Develop's "Object at click" mask.
//!
//! Written from the paper and the published model's tensor names and shapes; checked
//! against the authors' reference implementation run locally as an oracle (never shipped).
//!
//! - **Encoder** ([`encoder`]): the image stretched to 1024², ImageNet-normalised, a plain
//!   ViT (16-pixel patches, 12 blocks, global attention; ViT-Ti 192 wide, ViT-S 384) with its
//!   224-pixel position table resized bicubically to the 64² grid, then a neck (1×1 conv,
//!   channel norm, 3×3 conv, channel norm) to a 256 × 64 × 64 embedding. Runs once per photo.
//! - **Prompt** ([`Prompt::embed`]): up to [`MAX_POINTS`] points (clicks and a box's two
//!   corners) as random Fourier features plus a learnt embedding per kind, padded with
//!   "invalid" points; computed on the host (a few thousand multiply-adds).
//! - **Decoder** ([`decoder`]): SAM's two-way transformer (two blocks of token
//!   self-attention, token-to-image and image-to-token cross-attention), a 4× upscaling
//!   head (two transposed convolutions), and one hypernetwork MLP per mask token: three
//!   candidate masks as 256² logits with a predicted IoU each. Runs per click in a few ms.

use crate::Error;
use crate::graph::{Act, Conv2d, Crop, Graph, Preprocess, Value};
use crate::models::{Weights, konst, layer_norm, linear};
use crate::tensor::Tensor;

/// Side of the square the encoder sees.
pub const INPUT: usize = 1024;
/// Patch side.
const PATCH: usize = 16;
/// Embedding grid side.
pub const GRID: usize = INPUT / PATCH;
/// Embedding channels (the neck's and the decoder's width).
pub const DIM: usize = 256;
/// Prompt points the decoder takes (fewer are padded, more refused).
pub const MAX_POINTS: usize = 6;
/// Side of the decoder's mask logits.
pub const MASK: usize = GRID * 4;
/// Mask tokens (one single-mask output and three candidates).
const MASK_TOKENS: usize = 4;
/// ViT blocks.
const DEPTH: usize = 12;
/// The pretraining grid the position table was learnt on (224 / 16).
const PRETRAIN_GRID: usize = 14;
/// ImageNet mean and standard deviation.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// The encoder's width, read from the checkpoint (192: ViT-Ti, 384: ViT-S).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamConfig {
    pub width: usize,
    pub heads: usize,
}

impl SamConfig {
    /// From the patch embedding's shape (64-wide heads, as both published sizes).
    pub fn from_width(width: usize) -> Result<SamConfig, Error> {
        if width == 0 || !width.is_multiple_of(64) || width > 1024 {
            return Err(Error::Weights(format!("efficientsam: width {width}")));
        }
        Ok(SamConfig {
            width,
            heads: width / 64,
        })
    }

    /// ViT-Ti, for tests with random weights.
    pub fn tiny() -> SamConfig {
        SamConfig {
            width: 192,
            heads: 3,
        }
    }
}

/// The input preprocessing: stretched (as the reference), area-filtered when shrinking.
pub fn preprocess(antialias: bool) -> Preprocess {
    Preprocess {
        width: INPUT,
        height: INPUT,
        crop: Crop::Stretch,
        antialias,
        mean: MEAN,
        std: STD,
    }
}

/// The encoder graph: one RGBA image input (any size), one output `[1, 256, 64, 64]`.
pub fn encoder(w: &mut dyn Weights, c: SamConfig, antialias: bool) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let img = g.image_input()?;
    let x = g.preprocess(img, preprocess(antialias))?;
    let e = encoder_from(&mut g, x, w, c)?;
    g.output(e)?;
    Ok(g)
}

/// The encoder on a `[1, 3, 1024, 1024]` normalised input.
pub fn encoder_from(
    g: &mut Graph,
    x: Value,
    w: &mut dyn Weights,
    c: SamConfig,
) -> Result<Value, Error> {
    let d = c.width;
    let p = "image_encoder";
    let pw = konst(
        g,
        w,
        &format!("{p}.patch_embed.proj.weight"),
        &[d, 3, PATCH, PATCH],
    )?;
    let pb = konst(g, w, &format!("{p}.patch_embed.proj.bias"), &[d])?;
    let conv = Conv2d {
        stride: [PATCH; 2],
        ..Default::default()
    };
    let t = g.conv2d(x, pw, Some(pb), conv)?;
    let t = g.reshape(t, &[1, d, GRID * GRID])?;
    let mut h = g.transpose(t, &[0, 2, 1])?;
    let table = w.get(
        &format!("{p}.pos_embed"),
        &[1, PRETRAIN_GRID * PRETRAIN_GRID + 1, d],
    )?;
    let pos = g.constant(resize_positions(
        &table.to_f32()[d..],
        PRETRAIN_GRID,
        GRID,
        d,
    ))?;
    h = g.add(h, pos)?;
    for i in 0..DEPTH {
        let b = format!("{p}.blocks.{i}");
        let y = layer_norm(g, w, h, &format!("{b}.norm1"), d, 1e-6)?;
        let qkv = linear(
            g,
            w,
            y,
            &format!("{b}.attn.qkv"),
            (3 * d, d),
            true,
            Act::Identity,
        )?;
        let a = g.attention_packed(qkv, c.heads, None, false)?;
        let o = linear(
            g,
            w,
            a,
            &format!("{b}.attn.proj"),
            (d, d),
            true,
            Act::Identity,
        )?;
        h = g.add(h, o)?;
        let y = layer_norm(g, w, h, &format!("{b}.norm2"), d, 1e-6)?;
        let m = linear(
            g,
            w,
            y,
            &format!("{b}.mlp.fc1"),
            (4 * d, d),
            true,
            Act::Gelu,
        )?;
        let m = linear(
            g,
            w,
            m,
            &format!("{b}.mlp.fc2"),
            (d, 4 * d),
            true,
            Act::Identity,
        )?;
        h = g.add(h, m)?;
    }
    let t = g.transpose(h, &[0, 2, 1])?;
    let t = g.reshape(t, &[1, d, GRID, GRID])?;
    let w0 = konst(g, w, &format!("{p}.neck.0.weight"), &[DIM, d, 1, 1])?;
    let t = g.conv2d(t, w0, None, Conv2d::default())?;
    let t = channel_norm(g, w, t, &format!("{p}.neck.1"))?;
    let w2 = konst(g, w, &format!("{p}.neck.2.weight"), &[DIM, DIM, 3, 3])?;
    let t = g.conv2d(t, w2, None, Conv2d::same(3, 1))?;
    channel_norm(g, w, t, &format!("{p}.neck.3"))
}

fn channel_norm(g: &mut Graph, w: &mut dyn Weights, x: Value, p: &str) -> Result<Value, Error> {
    let gamma = konst(g, w, &format!("{p}.weight"), &[DIM])?;
    let beta = konst(g, w, &format!("{p}.bias"), &[DIM])?;
    g.channel_norm(x, gamma, beta, 1e-6)
}

/// PyTorch's bicubic interpolation (A = −0.75, half-pixel centres, edge-clamped taps) of a
/// `from`² table of `d`-vectors (row-major, `[from·from, d]`) to `[1, to·to, d]`.
pub fn resize_positions(table: &[f32], from: usize, to: usize, d: usize) -> Tensor {
    let taps = |o: usize| -> ([usize; 4], [f32; 4]) {
        let s = (o as f32 + 0.5) * from as f32 / to as f32 - 0.5;
        let f = s.floor();
        let t = s - f;
        let a = -0.75f32;
        let near = |x: f32| ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0;
        let far = |x: f32| ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a;
        let wts = [far(t + 1.0), near(t), near(1.0 - t), far(2.0 - t)];
        let idx = [-1i64, 0, 1, 2].map(|k| (f as i64 + k).clamp(0, from as i64 - 1) as usize);
        (idx, wts)
    };
    let mut out = vec![0f32; to * to * d];
    for oy in 0..to {
        let (iy, wy) = taps(oy);
        for ox in 0..to {
            let (ix, wx) = taps(ox);
            let o = &mut out[(oy * to + ox) * d..][..d];
            for (&y, &wyv) in iy.iter().zip(&wy) {
                // PyTorch sums along x first, then y.
                let mut row = vec![0f32; d];
                for (&x, &wxv) in ix.iter().zip(&wx) {
                    let src = &table[(y * from + x) * d..][..d];
                    for (r, &s) in row.iter_mut().zip(src) {
                        *r += s * wxv;
                    }
                }
                for (o, r) in o.iter_mut().zip(&row) {
                    *o += r * wyv;
                }
            }
        }
    }
    Tensor::f32(&[1, to * to, d], out)
}

/// What a prompt point is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointKind {
    /// Part of the object.
    Positive,
    /// Not part of it (no learnt embedding: the position alone).
    Negative,
    BoxTopLeft,
    BoxBottomRight,
}

/// Clicks and a box, in fractions 0..1 of the image (x right, y down).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prompt {
    pub points: Vec<([f32; 2], PointKind)>,
}

/// The prompt encoder's learnt tables, read once.
#[derive(Clone, Debug)]
pub struct PromptTables {
    /// `[2, 128]` random Fourier frequencies.
    gauss: Vec<f32>,
    invalid: Vec<f32>,
    point: Vec<f32>,
    top_left: Vec<f32>,
    bottom_right: Vec<f32>,
}

impl PromptTables {
    pub fn read(w: &mut dyn Weights) -> Result<PromptTables, Error> {
        let p = "prompt_encoder";
        let mut get = |n: &str, s: &[usize]| -> Result<Vec<f32>, Error> {
            Ok(w.get(&format!("{p}.{n}"), s)?.to_f32())
        };
        Ok(PromptTables {
            gauss: get(
                "pe_layer.positional_encoding_gaussian_matrix",
                &[2, DIM / 2],
            )?,
            invalid: get("invalid_points.weight", &[1, DIM])?,
            point: get("point_embeddings.weight", &[1, DIM])?,
            top_left: get("bbox_top_left_embeddings.weight", &[1, DIM])?,
            bottom_right: get("bbox_bottom_right_embeddings.weight", &[1, DIM])?,
        })
    }

    /// sin/cos of 2π·(2c − 1)·G for a point `c` in fractions.
    fn fourier(&self, c: [f32; 2]) -> Vec<f32> {
        let h = DIM / 2;
        let (x, y) = (2.0 * c[0] - 1.0, 2.0 * c[1] - 1.0);
        let mut out = vec![0f32; DIM];
        for j in 0..h {
            let a = 2.0 * std::f32::consts::PI * (x * self.gauss[j] + y * self.gauss[h + j]);
            out[j] = a.sin();
            out[h + j] = a.cos();
        }
        out
    }

    /// The dense positional encoding of the embedding grid, `[1, 4096, 256]` (token-major).
    pub fn dense(&self) -> Tensor {
        let mut out = Vec::with_capacity(GRID * GRID * DIM);
        for y in 0..GRID {
            for x in 0..GRID {
                let c = [
                    (x as f32 + 0.5) / GRID as f32,
                    (y as f32 + 0.5) / GRID as f32,
                ];
                out.extend(self.fourier(c));
            }
        }
        Tensor::f32(&[1, GRID * GRID, DIM], out)
    }
}

impl Prompt {
    /// The decoder's sparse input `[1, 6, 256]`. Points are taken in the 1024² input frame
    /// as the reference does (shifted half a pixel, padding points at −1 with the "invalid"
    /// embedding). More than [`MAX_POINTS`] is an error.
    pub fn embed(&self, t: &PromptTables) -> Result<Tensor, Error> {
        if self.points.len() > MAX_POINTS {
            return Err(Error::Shape(format!(
                "efficientsam takes at most {MAX_POINTS} points, got {}",
                self.points.len()
            )));
        }
        let n = INPUT as f32;
        let mut out = Vec::with_capacity(MAX_POINTS * DIM);
        for i in 0..MAX_POINTS {
            let (c, add) = match self.points.get(i) {
                Some(&(p, kind)) => {
                    let px = p.map(|v| (v.clamp(0.0, 1.0) * n + 0.5) / n);
                    let add = match kind {
                        PointKind::Positive => Some(&t.point),
                        PointKind::Negative => None,
                        PointKind::BoxTopLeft => Some(&t.top_left),
                        PointKind::BoxBottomRight => Some(&t.bottom_right),
                    };
                    (px, add)
                }
                None => ([-0.5 / n; 2], Some(&t.invalid)),
            };
            let mut e = t.fourier(c);
            if let Some(a) = add {
                e.iter_mut().zip(a).for_each(|(e, a)| *e += a);
            }
            out.extend(e);
        }
        Ok(Tensor::f32(&[1, MAX_POINTS, DIM], out))
    }
}

/// One attention of the two-way transformer: projections to `internal` wide, heads, out.
#[allow(clippy::too_many_arguments)]
fn attn(
    g: &mut Graph,
    w: &mut dyn Weights,
    p: &str,
    q: Value,
    k: Value,
    v: Value,
    internal: usize,
) -> Result<Value, Error> {
    let qp = linear(
        g,
        w,
        q,
        &format!("{p}.q_proj"),
        (internal, DIM),
        true,
        Act::Identity,
    )?;
    let kp = linear(
        g,
        w,
        k,
        &format!("{p}.k_proj"),
        (internal, DIM),
        true,
        Act::Identity,
    )?;
    let vp = linear(
        g,
        w,
        v,
        &format!("{p}.v_proj"),
        (internal, DIM),
        true,
        Act::Identity,
    )?;
    let heads = 8;
    let scale = 1.0 / ((internal / heads) as f32).sqrt();
    let o = g.attention(qp, kp, vp, heads, Some(scale), false)?;
    linear(
        g,
        w,
        o,
        &format!("{p}.out_proj"),
        (DIM, internal),
        true,
        Act::Identity,
    )
}

/// A 2-layer-plus-output MLP block (`layers.k.0` then `fc`), GELU between.
fn mlp(
    g: &mut Graph,
    w: &mut dyn Weights,
    p: &str,
    x: Value,
    (layers, hidden, out): (usize, usize, usize),
) -> Result<Value, Error> {
    let mut h = x;
    let mut inp = DIM;
    for i in 0..layers {
        let name = format!("{p}.layers.{i}.0");
        h = linear(g, w, h, &name, (hidden, inp), true, Act::Gelu)?;
        inp = hidden;
    }
    linear(
        g,
        w,
        h,
        &format!("{p}.fc"),
        (out, hidden),
        true,
        Act::Identity,
    )
}

/// The decoder graph. Inputs: the image embedding `[1, 256, 64, 64]` and the prompt
/// `[1, 6, 256]` ([`Prompt::embed`]). Outputs: three candidate masks' logits
/// `[1, 3, 256, 256]` and their predicted IoU `[1, 3]`.
pub fn decoder(w: &mut dyn Weights, tables: &PromptTables) -> Result<Graph, Error> {
    let mut g = Graph::new();
    let emb = g.input(&[1, DIM, GRID, GRID], crate::DType::F32)?;
    let sparse = g.input(&[1, MAX_POINTS, DIM], crate::DType::F32)?;
    let (masks, iou) = decoder_from(&mut g, emb, sparse, w, tables)?;
    g.output(masks)?;
    g.output(iou)?;
    Ok(g)
}

/// The decoder on graph values (see [`decoder`]).
pub fn decoder_from(
    g: &mut Graph,
    emb: Value,
    sparse: Value,
    w: &mut dyn Weights,
    tables: &PromptTables,
) -> Result<(Value, Value), Error> {
    let p = "mask_decoder";
    let n_img = GRID * GRID;
    let iou_tok = konst(g, w, &format!("{p}.iou_token.weight"), &[1, DIM])?;
    let mask_tok = konst(
        g,
        w,
        &format!("{p}.mask_tokens.weight"),
        &[MASK_TOKENS, DIM],
    )?;
    let out_tok = g.concat(&[iou_tok, mask_tok], 0)?;
    let out_tok = g.reshape(out_tok, &[1, 1 + MASK_TOKENS, DIM])?;
    let tokens = g.concat(&[out_tok, sparse], 1)?;
    let pe = g.constant(tables.dense())?;
    let keys0 = g.reshape(emb, &[1, DIM, n_img])?;
    let mut keys = g.transpose(keys0, &[0, 2, 1])?;
    let mut queries = tokens;
    let t = format!("{p}.transformer");
    for i in 0..2 {
        let b = format!("{t}.layers.{i}");
        // Self-attention; after the first block the prompt's position is added to the
        // queries first (residual included, as the reference does).
        if i > 0 {
            queries = g.add(queries, tokens)?;
        }
        let a = attn(
            g,
            w,
            &format!("{b}.self_attn"),
            queries,
            queries,
            queries,
            DIM,
        )?;
        queries = g.add(queries, a)?;
        queries = layer_norm(g, w, queries, &format!("{b}.norm1"), DIM, 1e-5)?;
        // Tokens to image.
        let q = g.add(queries, tokens)?;
        let k = g.add(keys, pe)?;
        let a = attn(
            g,
            w,
            &format!("{b}.cross_attn_token_to_image"),
            q,
            k,
            keys,
            DIM / 2,
        )?;
        queries = g.add(queries, a)?;
        queries = layer_norm(g, w, queries, &format!("{b}.norm2"), DIM, 1e-5)?;
        let m = mlp(g, w, &format!("{b}.mlp"), queries, (1, 2048, DIM))?;
        queries = g.add(queries, m)?;
        queries = layer_norm(g, w, queries, &format!("{b}.norm3"), DIM, 1e-5)?;
        // Image to tokens.
        let q = g.add(queries, tokens)?;
        let k = g.add(keys, pe)?;
        let a = attn(
            g,
            w,
            &format!("{b}.cross_attn_image_to_token"),
            k,
            q,
            queries,
            DIM / 2,
        )?;
        keys = g.add(keys, a)?;
        keys = layer_norm(g, w, keys, &format!("{b}.norm4"), DIM, 1e-5)?;
    }
    let q = g.add(queries, tokens)?;
    let k = g.add(keys, pe)?;
    let a = attn(
        g,
        w,
        &format!("{t}.final_attn_token_to_image"),
        q,
        k,
        keys,
        DIM / 2,
    )?;
    queries = g.add(queries, a)?;
    queries = layer_norm(g, w, queries, &format!("{t}.norm_final_attn"), DIM, 1e-5)?;

    // Upscale the image tokens 4×.
    let src = g.transpose(keys, &[0, 2, 1])?;
    let src = g.reshape(src, &[1, DIM, GRID, GRID])?;
    let u = format!("{p}.final_output_upscaling_layers");
    let w0 = konst(g, w, &format!("{u}.0.0.weight"), &[DIM, 64, 2, 2])?;
    let b0 = konst(g, w, &format!("{u}.0.0.bias"), &[64])?;
    let up = g.conv_transpose2d(src, w0, Some(b0), [2, 2], [0; 4], Act::Identity)?;
    let gn_w = konst(g, w, &format!("{u}.0.1.weight"), &[64])?;
    let gn_b = konst(g, w, &format!("{u}.0.1.bias"), &[64])?;
    let up = g.group_norm(up, gn_w, gn_b, 1, 1e-5)?;
    let up = g.act(up, Act::Gelu)?;
    let w1 = konst(g, w, &format!("{u}.1.0.weight"), &[64, 32, 2, 2])?;
    let b1 = konst(g, w, &format!("{u}.1.0.bias"), &[32])?;
    let up = g.conv_transpose2d(up, w1, Some(b1), [2, 2], [0; 4], Act::Gelu)?;
    let up = g.reshape(up, &[1, 32, MASK * MASK])?;

    // One hypernetwork per mask token; the candidates are tokens 1..4.
    let mut hyper = Vec::with_capacity(MASK_TOKENS - 1);
    for i in 1..MASK_TOKENS {
        let tok = g.slice_axis(queries, 1, 1 + i, 2 + i)?;
        let name = format!("{p}.output_hypernetworks_mlps.{i}");
        hyper.push(mlp(g, w, &name, tok, (2, DIM, 32))?);
    }
    let hyper = g.concat(&hyper, 1)?;
    let masks = g.matmul(
        hyper,
        up,
        None,
        crate::graph::MatMul {
            trans_b: false,
            act: Act::Identity,
        },
    )?;
    let masks = g.reshape(masks, &[1, MASK_TOKENS - 1, MASK, MASK])?;
    let iou_out = g.slice_axis(queries, 1, 0, 1)?;
    let iou = mlp(
        g,
        w,
        &format!("{p}.iou_prediction_head"),
        iou_out,
        (2, 256, MASK_TOKENS),
    )?;
    let iou = g.reshape(iou, &[1, MASK_TOKENS])?;
    let iou = g.slice_axis(iou, 1, 1, MASK_TOKENS)?;
    Ok((masks, iou))
}

/// The best candidate by predicted IoU: its index.
pub fn best(iou: &[f32]) -> usize {
    iou.iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(0, |(i, _)| i)
}

/// Tensors an f16 conversion keeps in f32: the position table (resized bicubically at
/// load), the prompt encoder's frequencies and embeddings, and the decoder's tokens (all
/// small; the frequencies are multiplied by 2π before a sine).
pub fn keep_f32(t: &crate::safetensors::TensorInfo) -> bool {
    t.name.ends_with("pos_embed")
        || t.name.starts_with("prompt_encoder.")
        || t.name.ends_with("_token.weight")
        || t.name.ends_with("_tokens.weight")
}

/// The width of a checkpoint (from its patch embedding).
pub fn width_of(st: &crate::safetensors::SafeTensorsFile) -> Result<usize, Error> {
    st.header
        .get("image_encoder.patch_embed.proj.weight")
        .map(|t| t.shape[0])
        .ok_or_else(|| Error::Weights("efficientsam: no patch embedding".into()))
}
