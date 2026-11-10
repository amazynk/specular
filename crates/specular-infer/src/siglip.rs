//! SigLIP and SigLIP 2 image–text encoders (search by description; Hugging Face
//! `SiglipModel` checkpoints at a fixed resolution, such as `google/siglip2-base-patch16-256`).
//!
//! Read through [`crate::clip`] (`ClipConfig::detect` recognises the attention-pooling probe
//! and gives [`Layout::Siglip`]); this module is the parts that differ from CLIP:
//!
//! - **Image tower**: patches embedded by a convolution *with* bias, learned positions, no
//!   class token and no pre-norm; pre-norm blocks (tanh-GELU MLP, LayerNorm eps 1e-6); a
//!   post-norm; then a multi-head attention-pooling head: one learned probe query attends
//!   over the tokens, followed by a residual LayerNorm–MLP. No projection: the head's output
//!   is the embedding. The probe's query projection is a constant, folded on the host.
//! - **Text tower**: token embeddings plus positions, blocks *without* a causal mask,
//!   final LayerNorm, the **last** position (the text is padded to the full context) through
//!   a linear head with bias.
//! - **Input**: the whole image squashed to the square (no crop), normalised to [-1, 1]; text
//!   lower-cased and tokenized by SentencePiece with EOS appended
//!   ([`crate::spm::siglip_tokens`]).
//!
//! Both embeddings are L2-normalised. The checkpoint's `logit_scale` and `logit_bias` turn a
//! cosine into the sigmoid match probability the model was trained on ([`Logit`]).

use crate::Error;
use crate::clip::{Build, ClipConfig, HEAD_WIDTH, Layout, Names, Tower};
use crate::graph::{Act, Conv2d, Graph, MatMul, Value};
use crate::models::Weights;
use crate::safetensors::{Header, TensorInfo, convert_f16};
use crate::tensor::Tensor;
use std::path::Path;

const PROBE: &str = "vision_model.head.probe";
const PATCH_BIAS: &str = "vision_model.embeddings.patch_embedding.bias";
const HEAD: &str = "vision_model.head";
const TEXT_HEAD: &str = "text_model.head";

/// LayerNorm epsilon of every SigLIP.
pub const EPS: f32 = 1e-6;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

/// Whether `h` is a SigLIP checkpoint (it has the image tower's pooling probe).
pub fn is_siglip(h: &Header) -> bool {
    h.get(PROBE).is_some() && h.get("text_model.head.weight").is_some()
}

/// The sizes of the SigLIP checkpoint `h` describes.
pub fn detect(h: &Header) -> Result<ClipConfig, Error> {
    let n = Names(Layout::Siglip);
    let shape = |name: &str| -> Result<Vec<usize>, Error> {
        match h.get(name) {
            Some(t) => Ok(t.shape.clone()),
            None => bad(format!("SigLIP checkpoint without {name:?}")),
        }
    };
    let pw = shape(n.patch())?;
    if pw.len() != 4 || pw[1] != 3 || pw[2] != pw[3] || pw[2] == 0 {
        return bad(format!("patch embedding {pw:?}"));
    }
    let (vw, patch) = (pw[0], pw[2]);
    let pos = shape(n.vision_pos())?;
    let grid = (pos.first().copied().unwrap_or(0) as f64).sqrt() as usize;
    if pos.len() != 2 || grid == 0 || grid * grid != pos[0] || pos[1] != vw {
        return bad(format!("image positions {pos:?}"));
    }
    let tok = shape(n.tokens())?;
    let tpos = shape(n.text_pos())?;
    if tok.len() != 2 || tpos.len() != 2 || tpos[1] != tok[1] || tpos[0] == 0 {
        return bad(format!("text embeddings {tok:?}, {tpos:?}"));
    }
    let tower = |vision: bool, width: usize| -> Result<Tower, Error> {
        let ln = |i: usize| format!("{}.{}.weight", n.block(vision, i), n.ln1());
        let layers = (0..).take_while(|&i| h.get(&ln(i)).is_some()).count();
        let fc1 = shape(&format!("{}.{}.weight", n.block(vision, 0), n.fc1()))?;
        if layers == 0 || !width.is_multiple_of(HEAD_WIDTH) || fc1.len() != 2 {
            return bad(format!("tower of width {width}, {layers} layers"));
        }
        Ok(Tower {
            width,
            layers,
            heads: width / HEAD_WIDTH,
            mlp: fc1[0],
        })
    };
    let head = shape(&format!("{TEXT_HEAD}.weight"))?;
    if head.len() != 2 || head[0] != vw || head[1] != tok[1] {
        return bad(format!(
            "text head {head:?}: the embedding must be the image width {vw}"
        ));
    }
    let probe = shape(PROBE)?;
    if probe != [1, 1, vw] {
        return bad(format!("pooling probe {probe:?}"));
    }
    Ok(ClipConfig {
        layout: Layout::Siglip,
        image: grid * patch,
        patch,
        vision: tower(true, vw)?,
        context: tpos[0],
        vocab: tok[0],
        text: tower(false, tok[1])?,
        embed: vw,
        eps: EPS,
        act: Act::GeluTanh,
    })
}

/// A small SigLIP for tests: 32² images in 8² patches, 2 blocks per tower, 8 tokens.
pub fn test_config(vocab: usize) -> ClipConfig {
    let t = |width| Tower {
        width,
        layers: 2,
        heads: width / HEAD_WIDTH,
        mlp: 2 * width,
    };
    ClipConfig {
        layout: Layout::Siglip,
        image: 32,
        patch: 8,
        vision: t(128),
        context: 8,
        vocab,
        text: t(64),
        embed: 128,
        eps: EPS,
        act: Act::GeluTanh,
    }
}

const NT: MatMul = MatMul {
    trans_b: true,
    act: Act::Identity,
};

/// The image tower on `x [N, 3, image, image]` (normalised pixels): `[N, embed]`, unit length.
pub fn image_tower(
    g: &mut Graph,
    x: Value,
    c: &ClipConfig,
    w: &mut dyn Weights,
    f16: bool,
) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4 || s[1] != 3 || s[2] != c.image || s[3] != c.image || c.patch == 0 {
        return Err(Error::Shape(format!(
            "SigLIP wants [N, 3, {0}, {0}], got {s:?}",
            c.image
        )));
    }
    let mut b = Build {
        w,
        n: Names(Layout::Siglip),
        f16,
    };
    let (n, d) = (s[0], c.vision.width);
    let grid = c.image / c.patch;
    let tokens = grid * grid;
    let pw = b.get(b.n.patch(), &[d, 3, c.patch, c.patch])?;
    let pw = b.mat(g, pw)?;
    let pb = b.vec(g, PATCH_BIAS, d)?;
    let conv = Conv2d {
        stride: [c.patch; 2],
        ..Default::default()
    };
    let t = g.conv2d(x, pw, Some(pb), conv)?;
    let t = g.reshape(t, &[n, d, tokens])?;
    let h = g.transpose(t, &[0, 2, 1])?;
    let pos = b.get(b.n.vision_pos(), &[tokens, d])?;
    let pos = g.constant(Tensor::f32(&[1, tokens, d], pos.to_f32()))?;
    let mut h = g.add(h, pos)?;
    for i in 0..c.vision.layers {
        let p = b.n.block(true, i);
        h = b.block(g, h, &p, &c.vision, c.eps, c.act, false)?;
    }
    h = b.layer_norm(g, h, b.n.ln_post(), d, c.eps)?;

    let names = PoolNames {
        probe: PROBE,
        in_proj: &format!("{HEAD}.attention.in_proj"),
        out_proj: &format!("{HEAD}.attention.out_proj"),
        norm: &format!("{HEAD}.layernorm"),
        fc1: &format!("{HEAD}.mlp.fc1"),
        fc2: &format!("{HEAD}.mlp.fc2"),
    };
    let e = attention_pool(
        &mut b,
        g,
        h,
        &names,
        c.vision.heads,
        c.vision.mlp,
        c.eps,
        c.act,
    )?;
    let e = g.reshape(e, &[n, d])?;
    g.l2_norm(e, 1e-12)
}

/// The tensor names of an attention-pooling head.
pub(crate) struct PoolNames<'a> {
    /// `[1, 1, d]`.
    pub(crate) probe: &'a str,
    /// Prefix of `_weight` `[3d, d]` and `_bias` `[3d]` (PyTorch `MultiheadAttention`).
    pub(crate) in_proj: &'a str,
    pub(crate) out_proj: &'a str,
    pub(crate) norm: &'a str,
    pub(crate) fc1: &'a str,
    pub(crate) fc2: &'a str,
}

/// Attention pooling of `h [N, T, d]` (SigLIP's head, PE's `attn_pool`): one learned probe
/// query attends over the tokens with `heads` heads, then a residual LayerNorm–MLP:
/// `[N, 1, d]`. The probe's query projection (probe · Wqᵀ + bq) is a constant, folded on
/// the host.
#[allow(clippy::too_many_arguments)]
pub(crate) fn attention_pool(
    b: &mut Build,
    g: &mut Graph,
    h: Value,
    names: &PoolNames,
    heads: usize,
    mlp: usize,
    eps: f32,
    act: Act,
) -> Result<Value, Error> {
    let s = g.shape(h).to_vec();
    let (n, d) = (s[0], s[2]);
    let probe = b.get(names.probe, &[1, 1, d])?.to_f32();
    let ipw = b.get(&format!("{}_weight", names.in_proj), &[3 * d, d])?;
    let ipw = ipw.to_f32();
    let ipb = b.get(&format!("{}_bias", names.in_proj), &[3 * d])?;
    let ipb = ipb.to_f32();
    let q: Vec<f32> = (0..d)
        .map(|j| {
            let row = &ipw[j * d..j * d + d];
            let dot: f64 = row
                .iter()
                .zip(&probe)
                .map(|(&a, &p)| f64::from(a) * f64::from(p))
                .sum();
            dot as f32 + ipb[j]
        })
        .collect();
    let q = g.constant(Tensor::f32(&[1, 1, d], q))?;
    let q = if n == 1 {
        q
    } else {
        let z = g.constant(Tensor::zeros(&[n, 1, d]))?;
        g.add(z, q)?
    };
    let proj = |b: &mut Build, g: &mut Graph, part: usize| -> Result<Value, Error> {
        let wt = Tensor::f32(&[d, d], ipw[part * d * d..(part + 1) * d * d].to_vec());
        let wt = b.mat(g, wt)?;
        let bias = g.constant(Tensor::f32(&[d], ipb[part * d..(part + 1) * d].to_vec()))?;
        g.matmul(h, wt, Some(bias), NT)
    };
    let k = proj(b, g, 1)?;
    let v = proj(b, g, 2)?;
    let a = g.attention(q, k, v, heads, None, false)?;
    let o = b.linear(g, a, names.out_proj, (d, d), Act::Identity)?;
    let y = b.layer_norm(g, o, names.norm, d, eps)?;
    let m = b.linear(g, y, names.fc1, (mlp, d), act)?;
    let m = b.linear(g, m, names.fc2, (d, mlp), Act::Identity)?;
    g.add(o, m)
}

/// The text tower on token embeddings `x [1, context, width]` (positions not added) and a
/// one-hot `end [1, 1, context]` marking the pooled token (the last): `[1, embed]`, unit
/// length.
pub fn text_tower(
    g: &mut Graph,
    x: Value,
    end: Value,
    c: &ClipConfig,
    w: &mut dyn Weights,
    f16: bool,
) -> Result<Value, Error> {
    let d = c.text.width;
    if g.shape(x) != [1, c.context, d] || g.shape(end) != [1, 1, c.context] {
        return Err(Error::Shape(format!(
            "SigLIP text wants [1, {0}, {d}] and [1, 1, {0}]",
            c.context
        )));
    }
    let mut b = Build {
        w,
        n: Names(Layout::Siglip),
        f16,
    };
    let pos = b.get(b.n.text_pos(), &[c.context, d])?;
    let pos = g.constant(Tensor::f32(&[1, c.context, d], pos.to_f32()))?;
    let mut h = g.add(x, pos)?;
    for i in 0..c.text.layers {
        let p = b.n.block(false, i);
        h = b.block(g, h, &p, &c.text, c.eps, c.act, false)?;
    }
    h = b.layer_norm(g, h, b.n.ln_final(), d, c.eps)?;
    let pooled = g.matmul(
        end,
        h,
        None,
        MatMul {
            trans_b: false,
            act: Act::Identity,
        },
    )?;
    let pooled = g.reshape(pooled, &[1, d])?;
    let e = b.linear(g, pooled, TEXT_HEAD, (c.embed, d), Act::Identity)?;
    g.l2_norm(e, 1e-12)
}

/// The checkpoint's calibration of a cosine into a match probability:
/// `sigmoid(cos · scale + bias)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Logit {
    /// `exp(logit_scale)`.
    pub scale: f32,
    pub bias: f32,
}

impl Logit {
    pub fn read(w: &mut dyn Weights) -> Result<Logit, Error> {
        let s = w.get("logit_scale", &[1])?.to_f32()[0];
        let b = w.get("logit_bias", &[1])?.to_f32()[0];
        if !s.is_finite() || !b.is_finite() || s > 20.0 {
            return bad(format!("logit scale {s}, bias {b}"));
        }
        Ok(Logit {
            scale: s.exp(),
            bias: b,
        })
    }

    /// The probability that an image and a text of cosine `cos` match.
    pub fn probability(&self, cos: f32) -> f32 {
        1.0 / (1.0 + (-(cos * self.scale + self.bias)).exp())
    }

    /// The cosine at which the probability is `p` (0 < p < 1).
    pub fn cosine_at(&self, p: f32) -> f32 {
        ((p / (1.0 - p)).ln() - self.bias) / self.scale
    }
}

/// The matrices a converted checkpoint keeps in f32: those the towers read at full
/// precision (positions, the pooling probe and its projection, which is folded on the
/// host), so the converted file builds exactly the graph the original does.
pub fn keep_f32(t: &TensorInfo) -> bool {
    t.name.contains("position_embedding")
        || t.name == PROBE
        || t.name.starts_with("vision_model.head.attention.in_proj")
}

/// Converts a downloaded checkpoint to the f16 file the pack keeps ([`convert_f16`] with
/// [`keep_f32`]): about half the download's size, the same embeddings.
pub fn convert(src: &Path, dst: &Path, cancel: &dyn Fn() -> bool) -> Result<u64, Error> {
    convert_f16(src, dst, &keep_f32, cancel)
}

/// Multiply-adds of one image.
pub fn image_macs(c: &ClipConfig) -> u64 {
    let p = (c.image / c.patch).pow(2) as u64;
    let (d, m) = (c.vision.width as u64, c.vision.mlp as u64);
    let t = &c.vision;
    let blocks = t.layers as u64 * (p * (4 * d * d + 2 * d * m) + 2 * p * p * d);
    // Patches, then the head: K and V over the tokens, one query, out projection, MLP.
    p * d * 3 * (c.patch as u64).pow(2) + blocks + 2 * p * d * d + 2 * p * d + d * d + 2 * d * m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DType;
    use crate::models::RandomWeights;
    use crate::safetensors::{parse_header, serialize};

    struct Recorder(Vec<(String, Tensor)>, RandomWeights);

    impl Weights for Recorder {
        fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
            let t = self.1.get(name, shape)?;
            self.0.push((name.to_owned(), t.clone()));
            Ok(t)
        }
    }

    #[test]
    fn detects_a_siglip_checkpoint() {
        let c = test_config(300);
        let mut g = Graph::new();
        let mut rec = Recorder(Vec::new(), RandomWeights::new(3));
        let x = g.input(&[1, 3, 32, 32], DType::F32).unwrap();
        image_tower(&mut g, x, &c, &mut rec, false).unwrap();
        let x = g.input(&[1, 8, 64], DType::F32).unwrap();
        let e = g.input(&[1, 1, 8], DType::F32).unwrap();
        text_tower(&mut g, x, e, &c, &mut rec, false).unwrap();
        rec.get(Names(Layout::Siglip).tokens(), &[300, 64]).unwrap();
        let named: Vec<(&str, &Tensor)> = rec.0.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = serialize(&named, &[]);
        let n = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        let h = parse_header(&bytes[8..8 + n], bytes.len() as u64).unwrap();
        assert!(is_siglip(&h));
        assert_eq!(ClipConfig::detect(&h).unwrap(), c);
    }

    #[test]
    fn base_patch16_256_macs() {
        // SigLIP 2 B/16 at 256²: about 23 GMACs (46 GFLOPs) per image.
        let t = Tower {
            width: 768,
            layers: 12,
            heads: 12,
            mlp: 3072,
        };
        let c = ClipConfig {
            image: 256,
            patch: 16,
            vision: t,
            text: t,
            context: 64,
            vocab: 256_000,
            embed: 768,
            ..test_config(1)
        };
        let g = image_macs(&c) as f64 / 1e9;
        assert!((22.0..24.5).contains(&g), "{g}");
        let b32 = ClipConfig { patch: 32, ..c };
        let g = image_macs(&b32) as f64 / 1e9;
        assert!((5.0..6.5).contains(&g), "{g}");
    }

    #[test]
    fn logit_round_trips() {
        let l = Logit {
            scale: 110.0,
            bias: -16.5,
        };
        let c = l.cosine_at(0.5);
        assert!((l.probability(c) - 0.5).abs() < 1e-5);
        assert!((c - 0.15).abs() < 1e-6);
    }
}
