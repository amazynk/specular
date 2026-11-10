//! Meta's Perception Encoder, PE Core (search by description, English only; such as
//! `facebook/PE-Core-S16-384`), from its PyTorch checkpoint ([`crate::pt`]) or the f16
//! safetensors file the pack converts it to ([`convert`]).
//!
//! Read through [`crate::clip`] (`ClipConfig::detect` recognises the image tower's pooling
//! probe and gives [`Layout::PeCore`]); the names are OpenCLIP's. What differs from CLIP:
//!
//! - **Image tower**: patches embedded by a bias-free convolution, a class token, learned
//!   positions, `ln_pre`, pre-norm blocks with **2-D rotary positions** on the queries and
//!   keys (below), `ln_post` over every token, then attention pooling (one learned probe,
//!   [`POOL_HEADS`] heads, a residual LayerNorm–MLP, as SigLIP's head) and the projection.
//! - **Text tower**: CLIP's (causal blocks, the end-of-text token's state projected), with a
//!   context of 32 tokens and CLIP's BPE tokenizer.
//! - **Input**: the whole image squashed to the square, normalised to [-1, 1].
//!
//! Exact GELU and LayerNorm eps 1e-5 throughout; both embeddings are L2-normalised.
//!
//! **Rotary positions.** Each head's `h` dimensions are split in two: the first half is
//! rotated by the patch's column, the second by its row (both counted from 1; the class
//! token is at 0, unrotated). Within a half, dimensions `2i` and `2i+1` turn together by the
//! angle `pos · 10000^(−4i/h)`: `(a, b) → (a cos − b sin, b cos + a sin)`. In the graph this
//! is `qkv ⊙ C + swap(qkv) ⊙ S`, with `swap` exchanging the two of each pair and the tables
//! `C`, `S` constants (1 and 0 over the values).

use std::path::Path;

use crate::Error;
use crate::clip::{Build, ClipConfig, HEAD_WIDTH, Layout, Names, Tower};
use crate::graph::{Act, Conv2d, Graph, MatMul, Value};
use crate::models::Weights;
use crate::safetensors::{Header, TensorInfo, convert_f16_from};
use crate::siglip::{PoolNames, attention_pool};
use crate::tensor::Tensor;

const PROBE: &str = "visual.attn_pool.probe";
const POOL: &str = "visual.attn_pool";

/// Heads of the pooling attention (`attn_pooler_heads` of every PE Core).
pub const POOL_HEADS: usize = 8;
/// LayerNorm epsilon.
pub const EPS: f32 = 1e-5;
/// Rotary base.
const THETA: f32 = 10_000.0;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

/// Whether `h` is a PE Core checkpoint (it has the image tower's pooling probe).
pub fn is_pecore(h: &Header) -> bool {
    h.get(PROBE).is_some() && h.get("text_projection").is_some()
}

/// The sizes of the PE Core checkpoint `h` describes.
pub fn detect(h: &Header) -> Result<ClipConfig, Error> {
    let n = Names(Layout::PeCore);
    let shape = |name: &str| -> Result<Vec<usize>, Error> {
        match h.get(name) {
            Some(t) => Ok(t.shape.clone()),
            None => bad(format!("PE Core checkpoint without {name:?}")),
        }
    };
    let pw = shape(n.patch())?;
    if pw.len() != 4 || pw[1] != 3 || pw[2] != pw[3] || pw[2] == 0 {
        return bad(format!("patch embedding {pw:?}"));
    }
    let (vw, patch) = (pw[0], pw[2]);
    let pos = shape(n.vision_pos())?;
    let grid = (pos.first().copied().unwrap_or(0).saturating_sub(1) as f64).sqrt() as usize;
    if pos.len() != 2 || grid == 0 || grid * grid + 1 != pos[0] || pos[1] != vw {
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
    let vp = shape(n.vision_proj())?;
    let tp = shape(n.text_proj())?;
    if vp.len() != 2 || vp[0] != vw || tp.len() != 2 || tp[0] != tok[1] || tp[1] != vp[1] {
        return bad(format!("projections {vp:?}, {tp:?}"));
    }
    let probe = shape(PROBE)?;
    if probe != [1, 1, vw] || !vw.is_multiple_of(POOL_HEADS) {
        return bad(format!("pooling probe {probe:?}"));
    }
    Ok(ClipConfig {
        layout: Layout::PeCore,
        image: grid * patch,
        patch,
        vision: tower(true, vw)?,
        context: tpos[0],
        vocab: tok[0],
        text: tower(false, tok[1])?,
        embed: vp[1],
        eps: EPS,
        act: Act::Gelu,
    })
}

/// A small PE Core for tests: 32² images in 8² patches, 2 blocks per tower, 8 tokens.
pub fn test_config(vocab: usize) -> ClipConfig {
    let t = |width| Tower {
        width,
        layers: 2,
        heads: width / HEAD_WIDTH,
        mlp: 2 * width,
    };
    ClipConfig {
        layout: Layout::PeCore,
        image: 32,
        patch: 8,
        vision: t(128),
        context: 8,
        vocab,
        text: t(64),
        embed: 32,
        eps: EPS,
        act: Act::Gelu,
    }
}

/// The rotary tables for a `grid × grid` image with a class token, over a packed
/// `[1 + grid², 3·width]` QKV: `(C, S)`, each that shape. Values get 1 and 0.
pub fn rope_tables(grid: usize, width: usize, heads: usize) -> (Vec<f32>, Vec<f32>) {
    let hd = width / heads;
    let half = hd / 2;
    let tokens = 1 + grid * grid;
    let row = 3 * width;
    let mut c = vec![1f32; tokens * row];
    let mut s = vec![0f32; tokens * row];
    for t in 1..tokens {
        let (y, x) = ((t - 1) / grid, (t - 1) % grid);
        for part in 0..2 {
            for h in 0..heads {
                for j in 0..hd {
                    let pos = if j < half { x + 1 } else { y + 1 } as f32;
                    // As the reference computes it, in f32: θ^-(2i / half) for pair i.
                    let i = (j % half) / 2;
                    let freq = 1.0 / THETA.powf((2 * i) as f32 / half as f32);
                    let a = f64::from(pos * freq);
                    let at = t * row + part * width + h * hd + j;
                    c[at] = a.cos() as f32;
                    s[at] = if j % 2 == 0 { -a.sin() } else { a.sin() } as f32;
                }
            }
        }
    }
    (c, s)
}

/// Rotates the queries and keys of a packed `qkv [N, T, 3d]` by the tables `c`, `s`
/// (constants `[1, T, 3d]`).
fn rotate(g: &mut Graph, qkv: Value, c: Value, s: Value) -> Result<Value, Error> {
    let shape = g.shape(qkv).to_vec();
    let (n, t, w) = (shape[0], shape[1], shape[2]);
    let pairs = g.reshape(qkv, &[n, t, w / 2, 2])?;
    let odd = g.slice_axis(pairs, 3, 1, 2)?;
    let even = g.slice_axis(pairs, 3, 0, 1)?;
    let swapped = g.concat(&[odd, even], 3)?;
    let swapped = g.reshape(swapped, &[n, t, w])?;
    let a = g.mul(qkv, c)?;
    let b = g.mul(swapped, s)?;
    g.add(a, b)
}

/// One image block: CLIP's, with the queries and keys rotated.
#[allow(clippy::too_many_arguments)]
fn block(
    b: &mut Build,
    g: &mut Graph,
    x: Value,
    p: &str,
    t: &Tower,
    eps: f32,
    rope: (Value, Value),
) -> Result<Value, Error> {
    let d = t.width;
    let y = b.layer_norm(g, x, &format!("{p}.ln_1"), d, eps)?;
    let (w, bias) = b.qkv(p, d)?;
    let w = b.mat(g, w)?;
    let bias = g.constant(Tensor::f32(&[3 * d], bias.to_f32()))?;
    let qkv = g.matmul(
        y,
        w,
        Some(bias),
        MatMul {
            trans_b: true,
            act: Act::Identity,
        },
    )?;
    let qkv = rotate(g, qkv, rope.0, rope.1)?;
    let a = g.attention_packed(qkv, t.heads, None, false)?;
    let o = b.linear(g, a, &format!("{p}.attn.out_proj"), (d, d), Act::Identity)?;
    let x = g.add(x, o)?;
    let y = b.layer_norm(g, x, &format!("{p}.ln_2"), d, eps)?;
    let m = b.linear(g, y, &format!("{p}.mlp.c_fc"), (t.mlp, d), Act::Gelu)?;
    let m = b.linear(g, m, &format!("{p}.mlp.c_proj"), (d, t.mlp), Act::Identity)?;
    g.add(x, m)
}

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
            "PE Core wants [N, 3, {0}, {0}], got {s:?}",
            c.image
        )));
    }
    let mut b = Build {
        w,
        n: Names(Layout::PeCore),
        f16,
    };
    let (n, d) = (s[0], c.vision.width);
    let grid = c.image / c.patch;
    let tokens = grid * grid + 1;
    let pw = b.get(b.n.patch(), &[d, 3, c.patch, c.patch])?;
    let pw = b.mat(g, pw)?;
    let conv = Conv2d {
        stride: [c.patch; 2],
        ..Default::default()
    };
    let t = g.conv2d(x, pw, None, conv)?;
    let t = g.reshape(t, &[n, d, grid * grid])?;
    let h = g.transpose(t, &[0, 2, 1])?;
    let cls = b.get(b.n.class(), &[d])?;
    let cls = g.constant(Tensor::f32(&[1, 1, d], cls.to_f32()))?;
    let cls = if n == 1 {
        cls
    } else {
        let z = g.constant(Tensor::zeros(&[n, 1, d]))?;
        g.add(z, cls)?
    };
    let h = g.concat(&[cls, h], 1)?;
    let pos = b.get(b.n.vision_pos(), &[tokens, d])?;
    let pos = g.constant(Tensor::f32(&[1, tokens, d], pos.to_f32()))?;
    let mut h = g.add(h, pos)?;
    h = b.layer_norm(g, h, b.n.ln_pre(), d, c.eps)?;
    let (rc, rs) = rope_tables(grid, d, c.vision.heads);
    let rc = g.constant(Tensor::f32(&[1, tokens, 3 * d], rc))?;
    let rs = g.constant(Tensor::f32(&[1, tokens, 3 * d], rs))?;
    for i in 0..c.vision.layers {
        let p = b.n.block(true, i);
        h = block(&mut b, g, h, &p, &c.vision, c.eps, (rc, rs))?;
    }
    h = b.layer_norm(g, h, b.n.ln_post(), d, c.eps)?;
    let names = PoolNames {
        probe: PROBE,
        in_proj: &format!("{POOL}.attn.in_proj"),
        out_proj: &format!("{POOL}.attn.out_proj"),
        norm: &format!("{POOL}.layernorm"),
        fc1: &format!("{POOL}.mlp.c_fc"),
        fc2: &format!("{POOL}.mlp.c_proj"),
    };
    let mlp = 4 * d;
    let e = attention_pool(&mut b, g, h, &names, POOL_HEADS, mlp, c.eps, c.act)?;
    let e = g.reshape(e, &[n, d])?;
    let proj = b.projection(g, b.n.vision_proj(), d, c.embed)?;
    let e = g.matmul(
        e,
        proj,
        None,
        MatMul {
            trans_b: true,
            act: Act::Identity,
        },
    )?;
    g.l2_norm(e, 1e-12)
}

/// The matrices a converted checkpoint keeps in f32: those the towers read at full
/// precision (positions, the pooling probe and its projection, folded on the host).
pub fn keep_f32(t: &TensorInfo) -> bool {
    t.name.contains("positional_embedding")
        || t.name == PROBE
        || t.name.starts_with("visual.attn_pool.attn.in_proj")
}

/// Converts a downloaded `.pt` checkpoint to the f16 safetensors file the pack keeps (the
/// tensors in the checkpoint's order, [`keep_f32`] kept): about half the download's size,
/// the same embeddings. Returns the bytes written.
pub fn convert(src: &Path, dst: &Path, cancel: &dyn Fn() -> bool) -> Result<u64, Error> {
    convert_f16_from(crate::pt::PtFile::open(src)?, dst, &keep_f32, cancel)
}

/// Multiply-adds of one image.
pub fn image_macs(c: &ClipConfig) -> u64 {
    let p = (c.image / c.patch).pow(2) as u64;
    let t = p + 1;
    let (d, m, e) = (c.vision.width as u64, c.vision.mlp as u64, c.embed as u64);
    let blocks = c.vision.layers as u64 * (t * (4 * d * d + 2 * d * m) + 2 * t * t * d);
    // Patches, blocks, then the head (K and V over the tokens, one query, out projection,
    // MLP of 4d) and the projection.
    p * d * 3 * (c.patch as u64).pow(2)
        + blocks
        + 2 * t * d * d
        + 2 * t * d
        + d * d
        + 8 * d * d
        + d * e
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
            if !self.0.iter().any(|(n, _)| n == name) {
                self.0.push((name.to_owned(), t.clone()));
            }
            Ok(t)
        }
    }

    /// Every tensor a test-size PE Core reads, with random values.
    pub(crate) fn tensors(c: &ClipConfig, seed: u64) -> Vec<(String, Tensor)> {
        let mut g = Graph::new();
        let mut rec = Recorder(Vec::new(), RandomWeights::new(seed));
        let x = g.input(&[1, 3, c.image, c.image], DType::F32).unwrap();
        image_tower(&mut g, x, c, &mut rec, false).unwrap();
        let x = g.input(&[1, c.context, c.text.width], DType::F32).unwrap();
        let e = g.input(&[1, 1, c.context], DType::F32).unwrap();
        crate::clip::text_tower(&mut g, x, e, c, &mut rec, false).unwrap();
        rec.get(Names(Layout::PeCore).tokens(), &[c.vocab, c.text.width])
            .unwrap();
        rec.0
    }

    #[test]
    fn detects_a_pecore_checkpoint() {
        let c = test_config(300);
        let t = tensors(&c, 3);
        let named: Vec<(&str, &Tensor)> = t.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = serialize(&named, &[]);
        let n = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        let h = parse_header(&bytes[8..8 + n], bytes.len() as u64).unwrap();
        assert!(is_pecore(&h));
        assert!(!crate::siglip::is_siglip(&h));
        assert_eq!(ClipConfig::detect(&h).unwrap(), c);
    }

    #[test]
    fn pt_checkpoint_converts_to_the_same_model() {
        use crate::safetensors::{SafeTensorsFile, StDType};
        let c = test_config(300);
        let t = tensors(&c, 7);
        let named: Vec<(&str, &Tensor)> = t.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let dir = std::env::temp_dir().join(format!("specular-pecore-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (pt, st) = (dir.join("m.pt"), dir.join("m.safetensors"));
        std::fs::write(&pt, crate::pt::serialize(&named)).unwrap();
        let n = convert(&pt, &st, &|| false).unwrap();
        assert_eq!(n, std::fs::metadata(&st).unwrap().len());
        let f = SafeTensorsFile::open(&st).unwrap();
        let dtype = |n: &str| f.header.get(n).unwrap().dtype;
        assert_eq!(dtype("visual.conv1.weight"), StDType::F16);
        assert_eq!(
            dtype("visual.transformer.resblocks.0.attn.in_proj_weight"),
            StDType::F16
        );
        assert_eq!(dtype("token_embedding.weight"), StDType::F16);
        for name in [
            PROBE,
            "visual.positional_embedding",
            "positional_embedding",
            "visual.attn_pool.attn.in_proj_weight",
            "visual.class_embedding",
            "visual.ln_post.bias",
        ] {
            assert_eq!(dtype(name), StDType::F32, "{name}");
        }
        // The same image tower from either file (f16 matrices, as the app builds it).
        let x = crate::tensor::Rng::new(1).tensor(&[1, 3, 32, 32], 1.0);
        let run = |w: &mut dyn Weights| {
            let mut g = Graph::new();
            let xi = g.input(&[1, 3, 32, 32], DType::F32).unwrap();
            let e = image_tower(&mut g, xi, &c, w, true).unwrap();
            g.output(e).unwrap();
            crate::cpu::run(&g, &[crate::Feed::Tensor(&x)], 1).unwrap()
        };
        let a = run(&mut crate::clip::open_weights(&pt).unwrap());
        let b = run(&mut crate::clip::open_weights(&st).unwrap());
        assert_eq!(a, b);
        assert_eq!(
            ClipConfig::detect(&crate::pt::PtFile::open(&pt).unwrap().header).unwrap(),
            c
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rope_tables_match_the_definition() {
        // Grid 2, width 128, 2 heads of 64: token 4 is (row 1, column 1) → positions 2, 2;
        // token 2 is (row 0, column 1): column 2, row 1.
        let (c, s) = rope_tables(2, 128, 2);
        let row = 3 * 128;
        assert_eq!(c.len(), 5 * row);
        // The class token and the values are untouched.
        assert!(c[..row].iter().all(|&x| x == 1.0) && s[..row].iter().all(|&x| x == 0.0));
        assert!(c[2 * row + 256..3 * row].iter().all(|&x| x == 1.0));
        // Head 1 of the keys, pair 3 of the row half (dimensions 32 + 6, 7): angle
        // 1 · 10000^(-6/32).
        let at = 2 * row + 128 + 64 + 38;
        let a = 10000f64.powf(-6.0 / 32.0);
        assert!((f64::from(c[at]) - a.cos()).abs() < 1e-6);
        assert!((f64::from(s[at]) + a.sin()).abs() < 1e-6);
        assert!((f64::from(s[at + 1]) - a.sin()).abs() < 1e-6);
        // Column half, pair 0: angle 2 (column 1 counted from 1).
        let at = 2 * row;
        assert!((f64::from(c[at]) - 2f64.cos()).abs() < 1e-6);
    }

    #[test]
    fn rotation_in_the_graph_is_the_pairwise_turn() {
        let (grid, d, heads) = (2, 128, 2);
        let t = 1 + grid * grid;
        let mut rng = crate::tensor::Rng::new(4);
        let x = rng.tensor(&[2, t, 3 * d], 1.0);
        let (c, s) = rope_tables(grid, d, heads);
        let mut g = Graph::new();
        let xi = g.input(&[2, t, 3 * d], DType::F32).unwrap();
        let cv = g.constant(Tensor::f32(&[1, t, 3 * d], c.clone())).unwrap();
        let sv = g.constant(Tensor::f32(&[1, t, 3 * d], s.clone())).unwrap();
        let y = rotate(&mut g, xi, cv, sv).unwrap();
        g.output(y).unwrap();
        let out = crate::cpu::run(&g, &[crate::Feed::Tensor(&x)], 1).unwrap();
        let (xv, yv) = (x.to_f32(), out[0].to_f32());
        for b in 0..2 {
            for tok in 0..t {
                for j in (0..3 * d).step_by(2) {
                    let at = (b * t + tok) * 3 * d + j;
                    let k = tok * 3 * d + j;
                    let (a0, a1) = (xv[at], xv[at + 1]);
                    let (cs, sn) = (c[k], s[k + 1]);
                    let want = [a0 * cs - a1 * sn, a1 * cs + a0 * sn];
                    assert!((yv[at] - want[0]).abs() < 1e-5, "{b} {tok} {j}");
                    assert!((yv[at + 1] - want[1]).abs() < 1e-5, "{b} {tok} {j}");
                }
            }
        }
    }

    #[test]
    fn s16_384_macs() {
        // PE Core S16 at 384²: 12 blocks of width 384 over 577 tokens, about 15 GMACs.
        let t = |width, heads| Tower {
            width,
            layers: 12,
            heads,
            mlp: 4 * width,
        };
        let c = ClipConfig {
            image: 384,
            patch: 16,
            vision: t(384, 6),
            text: t(512, 8),
            context: 32,
            vocab: 49_408,
            embed: 512,
            ..test_config(1)
        };
        let g = image_macs(&c) as f64 / 1e9;
        assert!((14.5..16.5).contains(&g), "{g}");
    }
}
