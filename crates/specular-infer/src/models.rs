//! Builders for the network families the Local AI pack considers, from named weights:
//! a ViT image encoder (CLIP/SigLIP-style: patch embedding, pre-norm blocks with fused-QKV
//! attention and a GELU MLP, pooled, projected and L2-normalised) and a MobileNetV2-style
//! CNN (batch norms folded into the convolutions at build time, ReLU6, inverted residuals).
//!
//! Weight names follow the timm / PyTorch layout (`blocks.0.attn.qkv.weight`, …) so a
//! converted checkpoint maps one to one. [`RandomWeights`] makes fixed-seed weights of the
//! right shapes for tests and benchmarks.

use crate::graph::{Act, BatchNorm, Conv2d, Graph, MatMul, ReduceOp, Value, fold_batchnorm};
use crate::safetensors::{SafeTensors, SafeTensorsFile};
use crate::tensor::{Rng, Tensor, numel};
use crate::{DType, Error};

/// A source of named weights. The builder states the shape it expects; a source must
/// return exactly that shape (f32 or f16).
pub trait Weights {
    fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error>;
}

fn check(name: &str, t: Tensor, shape: &[usize]) -> Result<Tensor, Error> {
    if t.shape != shape {
        return Err(Error::Weights(format!(
            "{name:?} is {:?}, the model wants {shape:?}",
            t.shape
        )));
    }
    Ok(t)
}

impl Weights for SafeTensors<'_> {
    fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        check(name, self.tensor(name)?, shape)
    }
}

impl Weights for SafeTensorsFile {
    fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        check(name, self.tensor(name)?, shape)
    }
}

/// Deterministic random weights: each tensor depends only on the seed and its name.
/// Matrices get uniform values scaled by 1/√fan-in, norm scales are near 1, biases and
/// norm shifts small, batch-norm variances positive.
#[derive(Clone, Debug)]
pub struct RandomWeights {
    pub seed: u64,
    /// Store matrices and convolution kernels as f16.
    pub f16: bool,
}

impl RandomWeights {
    pub fn new(seed: u64) -> RandomWeights {
        RandomWeights { seed, f16: false }
    }
}

fn name_hash(s: &str) -> u64 {
    // FNV-1a.
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Weights for RandomWeights {
    fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        let mut rng = Rng::new(self.seed ^ name_hash(name));
        let n = numel(shape).ok_or(Error::Shape("weight size overflows".into()))?;
        let fill = |rng: &mut Rng, f: &dyn Fn(&mut Rng) -> f32| -> Vec<f32> {
            (0..n).map(|_| f(rng)).collect()
        };
        let v = if name.ends_with("running_var") {
            fill(&mut rng, &|r| 0.5 + r.unit())
        } else if name.ends_with("running_mean") {
            fill(&mut rng, &|r| r.sym(0.2))
        } else if shape.len() == 1
            && (name.contains("norm") || name.contains("ln") || name.contains("bn"))
            && name.ends_with("weight")
        {
            fill(&mut rng, &|r| 1.0 + r.sym(0.2))
        } else if shape.len() == 1 {
            fill(&mut rng, &|r| r.sym(0.1))
        } else {
            let fan_in = if shape.len() >= 2 {
                shape[1..].iter().product::<usize>().max(1)
            } else {
                1
            };
            let a = (3.0 / fan_in as f32).sqrt();
            fill(&mut rng, &|r| r.sym(a))
        };
        let t = Tensor::f32(shape, v);
        Ok(if self.f16 && shape.len() >= 2 {
            t.to_f16()
        } else {
            t
        })
    }
}

/// How a ViT pools its tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitPool {
    /// The class token.
    Cls,
    /// The mean of the patch tokens (no class token).
    Mean,
}

/// A ViT image encoder.
#[derive(Clone, Debug, PartialEq)]
pub struct VitConfig {
    pub image: usize,
    pub patch: usize,
    pub dim: usize,
    pub depth: usize,
    pub heads: usize,
    /// Hidden width of the MLP.
    pub mlp: usize,
    pub pool: VitPool,
    /// Projection to an embedding (`head.weight [out, dim]`), if any.
    pub out_dim: Option<usize>,
    pub eps: f32,
    pub act: Act,
}

impl VitConfig {
    /// ViT-Ti/16 at 224² (5.7 M parameters), the benchmark size.
    pub fn tiny() -> VitConfig {
        VitConfig {
            image: 224,
            patch: 16,
            dim: 192,
            depth: 12,
            heads: 3,
            mlp: 768,
            pool: VitPool::Cls,
            out_dim: Some(512),
            eps: 1e-6,
            act: Act::Gelu,
        }
    }

    /// A 2-block ViT on 32² images for tests.
    pub fn test() -> VitConfig {
        VitConfig {
            image: 32,
            patch: 8,
            dim: 64,
            depth: 2,
            heads: 4,
            mlp: 128,
            pool: VitPool::Cls,
            out_dim: Some(32),
            eps: 1e-6,
            act: Act::Gelu,
        }
    }
}

pub(crate) fn konst(
    g: &mut Graph,
    w: &mut dyn Weights,
    name: &str,
    shape: &[usize],
) -> Result<Value, Error> {
    let t = w.get(name, shape)?;
    g.constant(t)
}

pub(crate) fn layer_norm(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    d: usize,
    eps: f32,
) -> Result<Value, Error> {
    let gamma = konst(g, w, &format!("{p}.weight"), &[d])?;
    let beta = konst(g, w, &format!("{p}.bias"), &[d])?;
    g.layer_norm(x, gamma, beta, eps)
}

pub(crate) fn linear(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    (out, inp): (usize, usize),
    bias: bool,
    act: Act,
) -> Result<Value, Error> {
    let wt = konst(g, w, &format!("{p}.weight"), &[out, inp])?;
    let b = if bias {
        Some(konst(g, w, &format!("{p}.bias"), &[out])?)
    } else {
        None
    };
    g.matmul(x, wt, b, MatMul { trans_b: true, act })
}

/// A ViT on `x [N, 3, image, image]`; returns `[N, out_dim or dim]`, L2-normalised when
/// projected.
pub fn vit(g: &mut Graph, x: Value, c: &VitConfig, w: &mut dyn Weights) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4 || s[1] != 3 || s[2] != c.image || s[3] != c.image || c.patch == 0 {
        return Err(Error::Shape(format!(
            "vit wants [N, 3, {0}, {0}], got {s:?}",
            c.image
        )));
    }
    let (n, d) = (s[0], c.dim);
    let grid = c.image / c.patch;
    let pw = konst(g, w, "patch_embed.proj.weight", &[d, 3, c.patch, c.patch])?;
    let pb = konst(g, w, "patch_embed.proj.bias", &[d])?;
    let conv = Conv2d {
        stride: [c.patch; 2],
        ..Default::default()
    };
    let t = g.conv2d(x, pw, Some(pb), conv)?;
    let t = g.reshape(t, &[n, d, grid * grid])?;
    let mut h = g.transpose(t, &[0, 2, 1])?;
    let mut tokens = grid * grid;
    if c.pool == VitPool::Cls {
        let cls = konst(g, w, "cls_token", &[1, 1, d])?;
        let cls = if n == 1 {
            cls
        } else {
            let z = g.constant(Tensor::zeros(&[n, 1, d]))?;
            g.add(z, cls)?
        };
        h = g.concat(&[cls, h], 1)?;
        tokens += 1;
    }
    let pos = konst(g, w, "pos_embed", &[1, tokens, d])?;
    h = g.add(h, pos)?;
    for i in 0..c.depth {
        let p = format!("blocks.{i}");
        let y = layer_norm(g, w, h, &format!("{p}.norm1"), d, c.eps)?;
        let qkv = linear(
            g,
            w,
            y,
            &format!("{p}.attn.qkv"),
            (3 * d, d),
            true,
            Act::Identity,
        )?;
        let a = g.attention_packed(qkv, c.heads, None, false)?;
        let o = linear(
            g,
            w,
            a,
            &format!("{p}.attn.proj"),
            (d, d),
            true,
            Act::Identity,
        )?;
        h = g.add(h, o)?;
        let y = layer_norm(g, w, h, &format!("{p}.norm2"), d, c.eps)?;
        let m = linear(g, w, y, &format!("{p}.mlp.fc1"), (c.mlp, d), true, c.act)?;
        let m = linear(
            g,
            w,
            m,
            &format!("{p}.mlp.fc2"),
            (d, c.mlp),
            true,
            Act::Identity,
        )?;
        h = g.add(h, m)?;
    }
    h = layer_norm(g, w, h, "norm", d, c.eps)?;
    let pooled = match c.pool {
        VitPool::Cls => g.slice_axis(h, 1, 0, 1)?,
        VitPool::Mean => g.reduce(h, ReduceOp::Mean, 1)?,
    };
    let pooled = g.reshape(pooled, &[n, d])?;
    match c.out_dim {
        Some(o) => {
            let e = linear(g, w, pooled, "head", (o, d), false, Act::Identity)?;
            g.l2_norm(e, 1e-12)
        }
        None => Ok(pooled),
    }
}

/// One inverted-residual block: expansion factor, output channels, stride.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub expand: usize,
    pub out: usize,
    pub stride: usize,
}

/// A MobileNetV2-style CNN.
#[derive(Clone, Debug, PartialEq)]
pub struct CnnConfig {
    pub image: usize,
    pub stem: usize,
    pub blocks: Vec<Block>,
    pub last: usize,
    pub classes: usize,
    pub bn_eps: f32,
}

impl CnnConfig {
    /// A small CNN on 64² images for tests: stem, five blocks (with and without expansion,
    /// residual and strided), head.
    pub fn test() -> CnnConfig {
        let b = |expand, out, stride| Block {
            expand,
            out,
            stride,
        };
        CnnConfig {
            image: 64,
            stem: 16,
            blocks: vec![
                b(1, 16, 1),
                b(4, 24, 2),
                b(4, 24, 1),
                b(4, 40, 2),
                b(6, 40, 1),
            ],
            last: 128,
            classes: 10,
            bn_eps: 1e-5,
        }
    }
}

/// Conv + folded batch norm + activation; weights `{p}.conv.weight`, `{p}.bn.*`.
#[allow(clippy::too_many_arguments)]
fn conv_bn(
    g: &mut Graph,
    w: &mut dyn Weights,
    x: Value,
    p: &str,
    (out, inp, k): (usize, usize, usize),
    conv: Conv2d,
    eps: f32,
) -> Result<Value, Error> {
    let cg = inp / conv.groups.max(1);
    let wt = w.get(&format!("{p}.conv.weight"), &[out, cg, k, k])?;
    let mut stat = |s: &str| -> Result<Vec<f32>, Error> {
        Ok(w.get(&format!("{p}.bn.{s}"), &[out])?.to_f32())
    };
    let bn = BatchNorm {
        gamma: stat("weight")?,
        beta: stat("bias")?,
        mean: stat("running_mean")?,
        var: stat("running_var")?,
        eps,
    };
    let (fw, fb) = fold_batchnorm(&wt, None, &bn)?;
    let fw = g.constant(fw)?;
    let fb = g.constant(fb)?;
    g.conv2d(x, fw, Some(fb), conv)
}

/// The CNN on `x [N, 3, image, image]`; returns logits `[N, classes]`.
pub fn cnn(g: &mut Graph, x: Value, c: &CnnConfig, w: &mut dyn Weights) -> Result<Value, Error> {
    let s = g.shape(x).to_vec();
    if s.len() != 4 || s[1] != 3 {
        return Err(Error::Shape(format!("cnn wants [N, 3, H, W], got {s:?}")));
    }
    let eps = c.bn_eps;
    let mut h = conv_bn(
        g,
        w,
        x,
        "stem",
        (c.stem, 3, 3),
        Conv2d::same(3, 2).act(Act::Relu6),
        eps,
    )?;
    let mut ch = c.stem;
    for (i, b) in c.blocks.iter().enumerate() {
        let p = format!("blocks.{i}");
        let hidden = ch * b.expand;
        let mut y = h;
        if b.expand != 1 {
            let pw = Conv2d::default().act(Act::Relu6);
            y = conv_bn(g, w, y, &format!("{p}.expand"), (hidden, ch, 1), pw, eps)?;
        }
        let dw = Conv2d::same(3, b.stride).groups(hidden).act(Act::Relu6);
        y = conv_bn(g, w, y, &format!("{p}.dw"), (hidden, hidden, 3), dw, eps)?;
        y = conv_bn(
            g,
            w,
            y,
            &format!("{p}.project"),
            (b.out, hidden, 1),
            Conv2d::default(),
            eps,
        )?;
        h = if b.stride == 1 && ch == b.out {
            g.add(h, y)?
        } else {
            y
        };
        ch = b.out;
    }
    let pw = Conv2d::default().act(Act::Relu6);
    h = conv_bn(g, w, h, "head", (c.last, ch, 1), pw, eps)?;
    let pooled = g.global_avg_pool(h)?;
    let n = s[0];
    let pooled = g.reshape(pooled, &[n, c.last])?;
    linear(
        g,
        w,
        pooled,
        "classifier",
        (c.classes, c.last),
        true,
        Act::Identity,
    )
}

/// A graph input of `[n, 3, size, size]` f32 for a model.
pub fn image_batch(g: &mut Graph, n: usize, size: usize) -> Result<Value, Error> {
    g.input(&[n, 3, size, size], DType::F32)
}
