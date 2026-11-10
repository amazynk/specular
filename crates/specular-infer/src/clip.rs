//! CLIP image–text encoders (search by description): a ViT image tower and a causal text
//! transformer, each pooled, projected into one embedding space and L2-normalised, so the
//! dot product of an image's and a text's embeddings ranks how well they match.
//!
//! Weights are read as upstream publishes them, in either layout ([`Layout`]): OpenCLIP's
//! (`visual.transformer.resblocks.N.attn.in_proj_weight`, …) or Hugging Face
//! `CLIPModel`'s (`vision_model.encoder.layers.N.self_attn.q_proj.weight`, …); the sizes
//! come from the tensor shapes ([`ClipConfig::detect`]). Matrices and convolution kernels
//! are kept as f16 (half the memory; computed in f32), the rest as f32.
//!
//! Both towers are pre-norm transformers with fused-QKV attention (HF's separate q, k and v
//! projections are stacked at load) and an exact-GELU MLP. The image tower embeds 2-D
//! patches with a bias-free convolution, prepends a class token, adds learned positions,
//! normalises (`ln_pre`), runs its blocks, normalises (`ln_post`) and projects the class
//! token. The text tower adds learned positions to the token embeddings (looked up on the
//! host: [`ClipText::feeds`]), runs causal blocks, normalises (`ln_final`) and projects the
//! end-of-text token's state (picked with a one-hot row).
//!
//! Checkpoints that use QuickGELU (OpenAI's original weights) are not supported: nothing in
//! the file says so, and their activations differ.

use std::path::Path;

use crate::bpe::{Bpe, CLIP_MERGES};
use crate::graph::{Act, Conv2d, Crop, Graph, MatMul, Preprocess, Value};
use crate::models::Weights;
use crate::safetensors::{Header, SafeTensorsFile};
use crate::spm::SentencePiece;
use crate::tensor::Tensor;
use crate::{DType, Error};

/// How a checkpoint names its tensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    /// OpenCLIP (`open_clip_model.safetensors`).
    OpenClip,
    /// Hugging Face transformers' `CLIPModel` (`model.safetensors`).
    HuggingFace,
    /// Hugging Face `SiglipModel` (SigLIP and SigLIP 2 at a fixed resolution): a different
    /// network on the same names (see [`crate::siglip`]).
    Siglip,
    /// Meta's Perception Encoder (PE Core): OpenCLIP's names plus an attention-pooling
    /// head, and 2-D rotary positions in the image tower (see [`crate::pecore`]).
    PeCore,
}

impl Layout {
    /// Whether the model takes the whole image squashed to its square (SigLIP, PE Core)
    /// rather than a centre crop (CLIP).
    pub fn squashes(self) -> bool {
        matches!(self, Layout::Siglip | Layout::PeCore)
    }
}

/// One transformer stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tower {
    pub width: usize,
    pub layers: usize,
    pub heads: usize,
    /// Hidden width of the MLP.
    pub mlp: usize,
}

/// A CLIP model's sizes.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipConfig {
    pub layout: Layout,
    /// Side of the square input image, pixels.
    pub image: usize,
    pub patch: usize,
    pub vision: Tower,
    /// Tokens per text (77 for CLIP).
    pub context: usize,
    /// Rows of the token table.
    pub vocab: usize,
    pub text: Tower,
    /// Size of the shared embedding.
    pub embed: usize,
    pub eps: f32,
    pub act: Act,
}

/// Width of one attention head in every CLIP this reads (heads = width / 64).
pub const HEAD_WIDTH: usize = 64;

/// The tensor names of one layout.
pub(crate) struct Names(pub(crate) Layout);

impl Names {
    pub(crate) fn patch(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "visual.conv1.weight",
            Layout::HuggingFace | Layout::Siglip => {
                "vision_model.embeddings.patch_embedding.weight"
            }
        }
    }
    pub(crate) fn class(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "visual.class_embedding",
            Layout::HuggingFace | Layout::Siglip => "vision_model.embeddings.class_embedding",
        }
    }
    pub(crate) fn vision_pos(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "visual.positional_embedding",
            Layout::HuggingFace | Layout::Siglip => {
                "vision_model.embeddings.position_embedding.weight"
            }
        }
    }
    pub(crate) fn ln_pre(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "visual.ln_pre",
            Layout::HuggingFace | Layout::Siglip => "vision_model.pre_layrnorm",
        }
    }
    pub(crate) fn ln_post(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "visual.ln_post",
            Layout::HuggingFace | Layout::Siglip => "vision_model.post_layernorm",
        }
    }
    /// The image projection: `[width, embed]` (OpenCLIP, `x @ proj`) or `[embed, width]`.
    pub(crate) fn vision_proj(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "visual.proj",
            Layout::HuggingFace | Layout::Siglip => "visual_projection.weight",
        }
    }
    pub(crate) fn tokens(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "token_embedding.weight",
            Layout::HuggingFace | Layout::Siglip => "text_model.embeddings.token_embedding.weight",
        }
    }
    pub(crate) fn text_pos(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "positional_embedding",
            Layout::HuggingFace | Layout::Siglip => {
                "text_model.embeddings.position_embedding.weight"
            }
        }
    }
    pub(crate) fn ln_final(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "ln_final",
            Layout::HuggingFace | Layout::Siglip => "text_model.final_layer_norm",
        }
    }
    pub(crate) fn text_proj(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "text_projection",
            Layout::HuggingFace | Layout::Siglip => "text_projection.weight",
        }
    }
    /// Prefix of block `i` of the image (`vision`) or text tower.
    pub(crate) fn block(&self, vision: bool, i: usize) -> String {
        match (self.0, vision) {
            (Layout::OpenClip | Layout::PeCore, true) => {
                format!("visual.transformer.resblocks.{i}")
            }
            (Layout::OpenClip | Layout::PeCore, false) => format!("transformer.resblocks.{i}"),
            (Layout::HuggingFace | Layout::Siglip, true) => {
                format!("vision_model.encoder.layers.{i}")
            }
            (Layout::HuggingFace | Layout::Siglip, false) => {
                format!("text_model.encoder.layers.{i}")
            }
        }
    }
    pub(crate) fn ln1(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "ln_1",
            Layout::HuggingFace | Layout::Siglip => "layer_norm1",
        }
    }
    pub(crate) fn ln2(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "ln_2",
            Layout::HuggingFace | Layout::Siglip => "layer_norm2",
        }
    }
    pub(crate) fn out_proj(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "attn.out_proj",
            Layout::HuggingFace | Layout::Siglip => "self_attn.out_proj",
        }
    }
    pub(crate) fn fc1(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "mlp.c_fc",
            Layout::HuggingFace | Layout::Siglip => "mlp.fc1",
        }
    }
    pub(crate) fn fc2(&self) -> &'static str {
        match self.0 {
            Layout::OpenClip | Layout::PeCore => "mlp.c_proj",
            Layout::HuggingFace | Layout::Siglip => "mlp.fc2",
        }
    }
}

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

impl ClipConfig {
    /// The sizes of the checkpoint `h` describes, in whichever layout it is.
    pub fn detect(h: &Header) -> Result<ClipConfig, Error> {
        if crate::siglip::is_siglip(h) {
            return crate::siglip::detect(h);
        }
        if crate::pecore::is_pecore(h) {
            return crate::pecore::detect(h);
        }
        let layout = if h.get(Names(Layout::OpenClip).patch()).is_some() {
            Layout::OpenClip
        } else if h.get(Names(Layout::HuggingFace).patch()).is_some() {
            Layout::HuggingFace
        } else {
            return bad("not a CLIP checkpoint (no patch embedding)");
        };
        let n = Names(layout);
        let shape = |name: &str| -> Result<Vec<usize>, Error> {
            match h.get(name) {
                Some(t) => Ok(t.shape.clone()),
                None => bad(format!("CLIP checkpoint without {name:?}")),
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
        if tok.len() != 2 || tpos.len() != 2 || tpos[1] != tok[1] {
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
        let embed = match layout {
            Layout::OpenClip | Layout::PeCore => vp.get(1).copied(),
            Layout::HuggingFace | Layout::Siglip => vp.first().copied(),
        }
        .unwrap_or(0);
        Ok(ClipConfig {
            layout,
            image: grid * patch,
            patch,
            vision: tower(true, vw)?,
            context: tpos[0],
            vocab: tok[0],
            text: tower(false, tok[1])?,
            embed,
            eps: 1e-5,
            act: Act::Gelu,
        })
    }

    /// A small CLIP for tests: 32² images in 8² patches, 2 blocks per tower, 8 tokens.
    pub fn test(layout: Layout, vocab: usize) -> ClipConfig {
        let t = |width| Tower {
            width,
            layers: 2,
            heads: width / HEAD_WIDTH,
            mlp: 2 * width,
        };
        ClipConfig {
            layout,
            image: 32,
            patch: 8,
            vision: t(128),
            context: 8,
            vocab,
            text: t(64),
            embed: 32,
            eps: 1e-5,
            act: Act::Gelu,
        }
    }

    /// Multiply-adds of one image.
    pub fn image_macs(&self) -> u64 {
        let p = (self.image / self.patch).pow(2) as u64;
        let (d, e) = (self.vision.width as u64, self.embed as u64);
        p * d * 3 * (self.patch as u64).pow(2) + tower_macs(&self.vision, p + 1) + d * e
    }

    /// Multiply-adds of one text.
    pub fn text_macs(&self) -> u64 {
        tower_macs(&self.text, self.context as u64) + (self.text.width * self.embed) as u64
    }
}

fn tower_macs(t: &Tower, tokens: u64) -> u64 {
    let (d, m) = (t.width as u64, t.mlp as u64);
    t.layers as u64 * (tokens * (4 * d * d + 2 * d * m) + 2 * tokens * tokens * d)
}

/// How weights become graph constants.
pub(crate) struct Build<'a> {
    pub(crate) w: &'a mut dyn Weights,
    pub(crate) n: Names,
    pub(crate) f16: bool,
}

impl Build<'_> {
    /// A matrix or kernel (f16 when asked).
    pub(crate) fn mat(&mut self, g: &mut Graph, t: Tensor) -> Result<Value, Error> {
        g.constant(if self.f16 { t.to_f16() } else { t })
    }

    pub(crate) fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        self.w.get(name, shape)
    }

    pub(crate) fn vec(&mut self, g: &mut Graph, name: &str, d: usize) -> Result<Value, Error> {
        let t = self.get(name, &[d])?;
        g.constant(Tensor::f32(&[d], t.to_f32()))
    }

    pub(crate) fn layer_norm(
        &mut self,
        g: &mut Graph,
        x: Value,
        p: &str,
        d: usize,
        eps: f32,
    ) -> Result<Value, Error> {
        let gamma = self.vec(g, &format!("{p}.weight"), d)?;
        let beta = self.vec(g, &format!("{p}.bias"), d)?;
        g.layer_norm(x, gamma, beta, eps)
    }

    pub(crate) fn linear(
        &mut self,
        g: &mut Graph,
        x: Value,
        p: &str,
        (out, inp): (usize, usize),
        act: Act,
    ) -> Result<Value, Error> {
        let w = self.get(&format!("{p}.weight"), &[out, inp])?;
        let w = self.mat(g, w)?;
        let b = self.vec(g, &format!("{p}.bias"), out)?;
        g.matmul(x, w, Some(b), MatMul { trans_b: true, act })
    }

    /// The fused `[3d, d]` QKV weight and `[3d]` bias of a block.
    pub(crate) fn qkv(&mut self, p: &str, d: usize) -> Result<(Tensor, Tensor), Error> {
        match self.n.0 {
            Layout::OpenClip | Layout::PeCore => Ok((
                self.get(&format!("{p}.attn.in_proj_weight"), &[3 * d, d])?,
                self.get(&format!("{p}.attn.in_proj_bias"), &[3 * d])?,
            )),
            Layout::HuggingFace | Layout::Siglip => {
                let mut w = Vec::with_capacity(3 * d * d);
                let mut b = Vec::with_capacity(3 * d);
                for s in ["q", "k", "v"] {
                    w.extend(
                        self.get(&format!("{p}.self_attn.{s}_proj.weight"), &[d, d])?
                            .to_f32(),
                    );
                    b.extend(
                        self.get(&format!("{p}.self_attn.{s}_proj.bias"), &[d])?
                            .to_f32(),
                    );
                }
                Ok((Tensor::f32(&[3 * d, d], w), Tensor::f32(&[3 * d], b)))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn block(
        &mut self,
        g: &mut Graph,
        x: Value,
        p: &str,
        t: &Tower,
        eps: f32,
        act: Act,
        causal: bool,
    ) -> Result<Value, Error> {
        let d = t.width;
        let y = self.layer_norm(g, x, &format!("{p}.{}", self.n.ln1()), d, eps)?;
        let (w, b) = self.qkv(p, d)?;
        let w = self.mat(g, w)?;
        let b = g.constant(Tensor::f32(&[3 * d], b.to_f32()))?;
        let qkv = g.matmul(
            y,
            w,
            Some(b),
            MatMul {
                trans_b: true,
                act: Act::Identity,
            },
        )?;
        let a = g.attention_packed(qkv, t.heads, None, causal)?;
        let o = self.linear(
            g,
            a,
            &format!("{p}.{}", self.n.out_proj()),
            (d, d),
            Act::Identity,
        )?;
        let x = g.add(x, o)?;
        let y = self.layer_norm(g, x, &format!("{p}.{}", self.n.ln2()), d, eps)?;
        let m = self.linear(g, y, &format!("{p}.{}", self.n.fc1()), (t.mlp, d), act)?;
        let m = self.linear(
            g,
            m,
            &format!("{p}.{}", self.n.fc2()),
            (d, t.mlp),
            Act::Identity,
        )?;
        g.add(x, m)
    }

    /// A projection to `[embed, width]` for `x · Pᵀ` (OpenCLIP stores `[width, embed]`).
    pub(crate) fn projection(
        &mut self,
        g: &mut Graph,
        name: &str,
        width: usize,
        embed: usize,
    ) -> Result<Value, Error> {
        let t = match self.n.0 {
            Layout::OpenClip | Layout::PeCore => {
                let t = self.get(name, &[width, embed])?.to_f32();
                let mut out = vec![0f32; width * embed];
                for r in 0..width {
                    for c in 0..embed {
                        out[c * width + r] = t[r * embed + c];
                    }
                }
                Tensor::f32(&[embed, width], out)
            }
            Layout::HuggingFace | Layout::Siglip => self.get(name, &[embed, width])?,
        };
        self.mat(g, t)
    }
}

/// The image tower on `x [N, 3, image, image]` (normalised pixels): `[N, embed]`, unit length.
pub fn image_tower(
    g: &mut Graph,
    x: Value,
    c: &ClipConfig,
    w: &mut dyn Weights,
    f16: bool,
) -> Result<Value, Error> {
    match c.layout {
        Layout::Siglip => return crate::siglip::image_tower(g, x, c, w, f16),
        Layout::PeCore => return crate::pecore::image_tower(g, x, c, w, f16),
        Layout::OpenClip | Layout::HuggingFace => {}
    }
    let s = g.shape(x).to_vec();
    if s.len() != 4 || s[1] != 3 || s[2] != c.image || s[3] != c.image || c.patch == 0 {
        return Err(Error::Shape(format!(
            "CLIP wants [N, 3, {0}, {0}], got {s:?}",
            c.image
        )));
    }
    let mut b = Build {
        w,
        n: Names(c.layout),
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
    for i in 0..c.vision.layers {
        let p = b.n.block(true, i);
        h = b.block(g, h, &p, &c.vision, c.eps, c.act, false)?;
    }
    let pooled = g.slice_axis(h, 1, 0, 1)?;
    let pooled = b.layer_norm(g, pooled, b.n.ln_post(), d, c.eps)?;
    let pooled = g.reshape(pooled, &[n, d])?;
    let proj = b.projection(g, b.n.vision_proj(), d, c.embed)?;
    let e = g.matmul(
        pooled,
        proj,
        None,
        MatMul {
            trans_b: true,
            act: Act::Identity,
        },
    )?;
    g.l2_norm(e, 1e-12)
}

/// The text tower on token embeddings `x [1, context, width]` (positions not added) and a
/// one-hot `end [1, 1, context]` marking the end-of-text token: `[1, embed]`, unit length.
pub fn text_tower(
    g: &mut Graph,
    x: Value,
    end: Value,
    c: &ClipConfig,
    w: &mut dyn Weights,
    f16: bool,
) -> Result<Value, Error> {
    if c.layout == Layout::Siglip {
        return crate::siglip::text_tower(g, x, end, c, w, f16);
    }
    let d = c.text.width;
    if g.shape(x) != [1, c.context, d] || g.shape(end) != [1, 1, c.context] {
        return Err(Error::Shape(format!(
            "CLIP text wants [1, {0}, {d}] and [1, 1, {0}]",
            c.context
        )));
    }
    let mut b = Build {
        w,
        n: Names(c.layout),
        f16,
    };
    let pos = b.get(b.n.text_pos(), &[c.context, d])?;
    let pos = g.constant(Tensor::f32(&[1, c.context, d], pos.to_f32()))?;
    let mut h = g.add(x, pos)?;
    for i in 0..c.text.layers {
        let p = b.n.block(false, i);
        h = b.block(g, h, &p, &c.text, c.eps, c.act, true)?;
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
    let proj = b.projection(g, b.n.text_proj(), d, c.embed)?;
    let e = g.matmul(
        pooled,
        proj,
        None,
        MatMul {
            trans_b: true,
            act: Act::Identity,
        },
    )?;
    g.l2_norm(e, 1e-12)
}

/// A text tower's tokenizer: CLIP's byte-level BPE, or SigLIP's SentencePiece.
#[derive(Clone, Debug)]
pub enum Tokenizer {
    Bpe(Bpe),
    SentencePiece(SentencePiece),
}

impl From<Bpe> for Tokenizer {
    fn from(b: Bpe) -> Tokenizer {
        Tokenizer::Bpe(b)
    }
}

impl From<SentencePiece> for Tokenizer {
    fn from(s: SentencePiece) -> Tokenizer {
        Tokenizer::SentencePiece(s)
    }
}

impl Tokenizer {
    /// Reads the tokenizer file of a checkpoint in `layout`: CLIP's merges file, or SigLIP's
    /// `tokenizer.model`.
    pub fn open(path: &Path, layout: Layout) -> Result<Tokenizer, Error> {
        match layout {
            Layout::Siglip => Ok(SentencePiece::open(path)?.into()),
            Layout::OpenClip | Layout::HuggingFace | Layout::PeCore => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| Error::Weights(format!("{}: {e}", path.display())))?;
                Ok(Bpe::from_merges(&text, CLIP_MERGES)?.into())
            }
        }
    }

    /// Number of ids.
    pub fn vocab_size(&self) -> usize {
        match self {
            Tokenizer::Bpe(b) => b.vocab_size(),
            Tokenizer::SentencePiece(s) => s.vocab_size(),
        }
    }

    /// `context` ids for `text` and the index of the token the tower pools: CLIP's
    /// end-of-text token, SigLIP's last position.
    pub fn tokenize(&self, text: &str, context: usize) -> (Vec<u32>, usize) {
        match self {
            Tokenizer::Bpe(b) => b.tokenize(text, context),
            Tokenizer::SentencePiece(s) => crate::spm::siglip_tokens(s, text, context),
        }
    }
}

/// What the image graph takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageInput {
    /// An RGBA8 image, centre-cropped to a square (CLIP) or squashed to one (SigLIP),
    /// resampled and normalised in the graph with this per-channel mean and std (of [0, 1]
    /// values).
    Rgba { mean: [f32; 3], std: [f32; 3] },
    /// `[N, 3, image, image]` already normalised (reference checks, batches).
    Tensor { batch: usize },
}

/// A loaded CLIP: the image graph, the text graph, the token table and the tokenizer.
#[derive(Debug)]
pub struct Clip {
    pub config: ClipConfig,
    /// One input (see [`ImageInput`]), one output `[N, embed]`.
    pub image: Graph,
    pub text: ClipText,
}

/// The text half: tokenizer, token table (on the host) and graph.
#[derive(Debug)]
pub struct ClipText {
    pub tokenizer: Tokenizer,
    /// `[vocab, width]`.
    pub table: Tensor,
    pub context: usize,
    /// Inputs `[1, context, width]` and `[1, 1, context]` (see [`ClipText::feeds`]); output
    /// `[1, embed]`.
    pub graph: Graph,
}

impl ClipText {
    /// The text graph's feeds for `text`.
    pub fn feeds(&self, text: &str) -> (Tensor, Tensor) {
        let (ids, end) = self.tokenizer.tokenize(text, self.context);
        self.feeds_for_ids(&ids, end)
    }

    /// The feeds for token ids already tokenized (length `context`; ids past the table
    /// read as 0).
    pub fn feeds_for_ids(&self, ids: &[u32], end: usize) -> (Tensor, Tensor) {
        let (v, d) = (self.table.shape[0], self.table.shape[1]);
        let mut x = vec![0f32; self.context * d];
        // Only the rows used are widened (SigLIP's f16 table has 256,000).
        for (row, &id) in x.chunks_exact_mut(d).zip(ids) {
            let id = (id as usize).min(v - 1);
            match &self.table.data {
                crate::tensor::Data::F32(t) => row.copy_from_slice(&t[id * d..id * d + d]),
                crate::tensor::Data::F16(t) => {
                    for (o, &h) in row.iter_mut().zip(&t[id * d..id * d + d]) {
                        *o = half::f16::from_bits(h).to_f32();
                    }
                }
            }
        }
        let mut onehot = vec![0f32; self.context];
        onehot[end.min(self.context - 1)] = 1.0;
        (
            Tensor::f32(&[1, self.context, d], x),
            Tensor::f32(&[1, 1, self.context], onehot),
        )
    }
}

impl Clip {
    /// Builds both towers from `w` (shapes as `c` says) with `tokenizer`.
    pub fn build(
        c: ClipConfig,
        w: &mut dyn Weights,
        tokenizer: impl Into<Tokenizer>,
        input: ImageInput,
        f16: bool,
    ) -> Result<Clip, Error> {
        let tokenizer = tokenizer.into();
        if !matches!(
            (&tokenizer, c.layout),
            (Tokenizer::SentencePiece(_), Layout::Siglip)
                | (
                    Tokenizer::Bpe(_),
                    Layout::OpenClip | Layout::HuggingFace | Layout::PeCore
                )
        ) {
            return bad("the tokenizer is not the model's kind");
        }
        if tokenizer.vocab_size() != c.vocab {
            return bad(format!(
                "the tokenizer has {} ids, the model {}",
                tokenizer.vocab_size(),
                c.vocab
            ));
        }
        let mut image = Graph::new();
        let x = match input {
            ImageInput::Rgba { mean, std } => {
                let img = image.image_input()?;
                image.preprocess(
                    img,
                    Preprocess {
                        width: c.image,
                        height: c.image,
                        crop: if c.layout.squashes() {
                            Crop::Stretch
                        } else {
                            Crop::Center
                        },
                        antialias: true,
                        mean,
                        std,
                    },
                )?
            }
            ImageInput::Tensor { batch } => {
                image.input(&[batch, 3, c.image, c.image], DType::F32)?
            }
        };
        let e = image_tower(&mut image, x, &c, w, f16)?;
        image.output(e)?;

        let mut graph = Graph::new();
        let x = graph.input(&[1, c.context, c.text.width], DType::F32)?;
        let end = graph.input(&[1, 1, c.context], DType::F32)?;
        let e = text_tower(&mut graph, x, end, &c, w, f16)?;
        graph.output(e)?;
        let n = Names(c.layout);
        let table = w.get(n.tokens(), &[c.vocab, c.text.width])?;
        let table = if f16 { table.to_f16() } else { table };
        Ok(Clip {
            text: ClipText {
                tokenizer,
                table,
                context: c.context,
                graph,
            },
            config: c,
            image,
        })
    }

    /// Loads an upstream checkpoint (any layout; safetensors, or a PyTorch `.pt` read by
    /// [`crate::pt`]) and its tokenizer file (CLIP's merges, SigLIP's `tokenizer.model`).
    pub fn load(
        weights: &Path,
        tokenizer: &Path,
        input: ImageInput,
        f16: bool,
    ) -> Result<Clip, Error> {
        let mut file = open_weights(weights)?;
        let c = ClipConfig::detect(&file.header)?;
        let tok = Tokenizer::open(tokenizer, c.layout)?;
        Clip::build(c, &mut file, tok, input, f16)
    }
}

/// Opens a weights file: a PyTorch archive (by its zip signature) or safetensors.
pub fn open_weights(path: &Path) -> Result<SafeTensorsFile, Error> {
    let mut magic = [0u8; 4];
    let n = std::fs::File::open(path)
        .and_then(|mut f| std::io::Read::read(&mut f, &mut magic))
        .map_err(|e| Error::Weights(format!("{}: {e}", path.display())))?;
    if crate::pt::is_zip(&magic[..n]) {
        crate::pt::PtFile::open(path)
    } else {
        SafeTensorsFile::open(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RandomWeights;
    use crate::safetensors::{parse_header, serialize};

    #[test]
    fn macs_of_known_models() {
        // OpenCLIP ViT-B/32 at 224²: 4.4 GMACs for the image (published ~4.4 G).
        let t = |width, layers| Tower {
            width,
            layers,
            heads: width / 64,
            mlp: 4 * width,
        };
        let b32 = ClipConfig {
            layout: Layout::OpenClip,
            image: 224,
            patch: 32,
            vision: t(768, 12),
            context: 77,
            vocab: 49_408,
            text: t(512, 12),
            embed: 512,
            eps: 1e-5,
            act: Act::Gelu,
        };
        let g = b32.image_macs() as f64 / 1e9;
        assert!((4.3..4.5).contains(&g), "{g}");
        let l14 = ClipConfig {
            image: 224,
            patch: 14,
            vision: t(1024, 24),
            text: t(768, 12),
            embed: 768,
            ..b32.clone()
        };
        let g = l14.image_macs() as f64 / 1e9;
        assert!((80.0..82.0).contains(&g), "{g}");
    }

    /// A checkpoint header with the test config's shapes, in `layout`.
    fn header(layout: Layout) -> Header {
        let c = ClipConfig::test(layout, 600);
        let mut g = Graph::new();
        let mut rec = Recorder(Vec::new(), RandomWeights::new(1));
        let x = g.input(&[1, 3, 32, 32], DType::F32).unwrap();
        image_tower(&mut g, x, &c, &mut rec, false).unwrap();
        let x = g.input(&[1, 8, 64], DType::F32).unwrap();
        let e = g.input(&[1, 1, 8], DType::F32).unwrap();
        text_tower(&mut g, x, e, &c, &mut rec, false).unwrap();
        rec.get(Names(layout).tokens(), &[600, 64]).unwrap();
        let named: Vec<(&str, &Tensor)> = rec.0.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let bytes = serialize(&named, &[]);
        let n = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
        parse_header(&bytes[8..8 + n], bytes.len() as u64).unwrap()
    }

    /// Weights that remember what was asked for.
    struct Recorder(Vec<(String, Tensor)>, RandomWeights);

    impl Weights for Recorder {
        fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
            let t = self.1.get(name, shape)?;
            self.0.push((name.to_owned(), t.clone()));
            Ok(t)
        }
    }

    #[test]
    fn detects_both_layouts() {
        for layout in [Layout::OpenClip, Layout::HuggingFace] {
            let h = header(layout);
            let c = ClipConfig::detect(&h).unwrap();
            assert_eq!(c, ClipConfig::test(layout, 600), "{layout:?}");
        }
    }
}
