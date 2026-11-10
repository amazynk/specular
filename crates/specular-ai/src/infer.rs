//! A [`Runtime`] over Specular's own inference engine (`specular-infer`): a model built from
//! a safetensors pack, run on the CPU reference (`cpu::run`) or as a compiled GPU graph
//! (`GpuGraph`).
//!
//! The graph takes the thumbnail's RGBA8 pixels, crops, resizes and normalises them
//! (`Graph::preprocess`) and runs the network; its `[1, d]` output becomes a unit f16
//! embedding. When loaded with a GPU, each model checks its own GPU graph against the CPU on
//! a built-in test picture and tells the PathSelector (`Task::Inference`): a match trusts
//! the GPU path, a mismatch blocks it and the model runs on the CPU only.
//!
//! # The pack
//!
//! One `.safetensors` file: the weights under their timm / PyTorch names (see
//! `specular_infer::models`) and the architecture in `__metadata__`, every value a string:
//!
//! | key | value |
//! |---|---|
//! | `specular.arch` | `vit` or `cnn` |
//! | `specular.crop` | `center` (CLIP-style) or `stretch` (SigLIP-style) |
//! | `specular.mean`, `specular.std` | three comma-separated numbers |
//! | `specular.vit.{image,patch,dim,depth,heads,mlp}` | integers |
//! | `specular.vit.pool` | `cls` or `mean` |
//! | `specular.vit.out_dim` | integer; absent for no projection |
//! | `specular.vit.eps` | number |
//! | `specular.vit.act` | `gelu` or `gelu_tanh` |
//! | `specular.cnn.{image,stem,last,classes}` | integers |
//! | `specular.cnn.blocks` | `expand:out:stride` per block, comma-separated |
//! | `specular.cnn.bn_eps` | number |
//!
//! # CLIP
//!
//! Search's real models are upstream CLIP checkpoints (OpenCLIP or Hugging Face names, see
//! `specular_infer::clip`), installed as they are with the tokenizer's merges file beside
//! them; their sizes come from the tensor shapes and the input normalisation from the
//! manifest. [`InferRuntime::load_clip`] gives the image half as this module's runtime and
//! the text half as a [`ClipTextEncoder`], each checked on the GPU against the CPU.

use std::path::Path;

use parking_lot::Mutex;
use specular_core::paths::{Engine, PathSelector, Task, Trust, selector};
use specular_infer::clip::{
    Clip, ClipConfig, ClipText, ImageInput, Layout, Tokenizer, image_tower,
};
use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::graph::{Crop, Preprocess};
use specular_infer::models::{Block, CnnConfig, VitConfig, VitPool, Weights, cnn, vit};
use specular_infer::safetensors::SafeTensorsFile;
use specular_infer::{Act, Error, Feed, Graph, Tensor};

use crate::install::{Installed, Store};
use crate::manifest::{Feature, Norm};
use crate::runtime::{AGREE, Outputs, RunError, Runtime, Thumb, agree, similarity};

/// The network.
#[derive(Clone, Debug, PartialEq)]
pub enum Arch {
    Vit(VitConfig),
    Cnn(CnnConfig),
    /// The image tower of a CLIP checkpoint (sizes read from its tensors, not metadata).
    Clip(ClipConfig),
}

/// A model's architecture and input preprocessing: what a pack's metadata describes.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub arch: Arch,
    pub crop: Crop,
    pub mean: [f32; 3],
    pub std: [f32; 3],
}

/// Bounds on the numbers a pack may give (no real encoder is near them).
const MAX_IMAGE: usize = 2048;
const MAX_SIZE: usize = 1 << 16;
const MAX_DEPTH: usize = 256;

fn bad<T>(s: impl Into<String>) -> Result<T, Error> {
    Err(Error::Weights(s.into()))
}

struct Meta<'a>(&'a [(String, String)]);

impl Meta<'_> {
    fn opt(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim())
    }

    fn str(&self, key: &str) -> Result<&str, Error> {
        self.opt(key)
            .ok_or_else(|| Error::Weights(format!("the pack has no {key:?}")))
    }

    fn size(&self, key: &str, max: usize) -> Result<usize, Error> {
        parse_size(key, self.str(key)?, max)
    }

    fn num(&self, key: &str) -> Result<f32, Error> {
        parse_num(key, self.str(key)?)
    }

    fn three(&self, key: &str) -> Result<[f32; 3], Error> {
        let v = self
            .str(key)?
            .split(',')
            .map(|s| parse_num(key, s.trim()))
            .collect::<Result<Vec<f32>, Error>>()?;
        <[f32; 3]>::try_from(v).or_else(|_| bad(format!("{key:?} wants three numbers")))
    }
}

fn parse_size(key: &str, s: &str, max: usize) -> Result<usize, Error> {
    match s.parse::<usize>() {
        Ok(n) if (1..=max).contains(&n) => Ok(n),
        _ => bad(format!("{key:?} is {s:?}, not a size from 1 to {max}")),
    }
}

fn parse_num(key: &str, s: &str) -> Result<f32, Error> {
    match s.parse::<f32>() {
        Ok(x) if x.is_finite() => Ok(x),
        _ => bad(format!("{key:?} is {s:?}, not a number")),
    }
}

impl Spec {
    /// The side of the square input the network takes.
    pub fn image(&self) -> usize {
        match &self.arch {
            Arch::Vit(c) => c.image,
            Arch::Cnn(c) => c.image,
            Arch::Clip(c) => c.image,
        }
    }

    /// A CLIP, SigLIP or PE Core checkpoint's image tower with `norm`'s input normalisation
    /// (CLIP centre-crops, SigLIP and PE Core squash the whole image).
    pub fn clip(c: ClipConfig, norm: Norm) -> Spec {
        let (mean, std) = norm.mean_std();
        let crop = if c.layout.squashes() {
            Crop::Stretch
        } else {
            Crop::Center
        };
        Spec {
            arch: Arch::Clip(c),
            crop,
            mean,
            std,
        }
    }

    /// Reads a pack's `__metadata__`.
    pub fn from_metadata(meta: &[(String, String)]) -> Result<Spec, Error> {
        let m = Meta(meta);
        let arch = match m.str("specular.arch")? {
            "vit" => Arch::Vit(VitConfig {
                image: m.size("specular.vit.image", MAX_IMAGE)?,
                patch: m.size("specular.vit.patch", MAX_IMAGE)?,
                dim: m.size("specular.vit.dim", MAX_SIZE)?,
                depth: m.size("specular.vit.depth", MAX_DEPTH)?,
                heads: m.size("specular.vit.heads", MAX_SIZE)?,
                mlp: m.size("specular.vit.mlp", MAX_SIZE)?,
                pool: match m.str("specular.vit.pool")? {
                    "cls" => VitPool::Cls,
                    "mean" => VitPool::Mean,
                    p => return bad(format!("unknown pooling {p:?}")),
                },
                out_dim: m
                    .opt("specular.vit.out_dim")
                    .map(|s| parse_size("specular.vit.out_dim", s, MAX_SIZE))
                    .transpose()?,
                eps: m.num("specular.vit.eps")?,
                act: match m.str("specular.vit.act")? {
                    "gelu" => Act::Gelu,
                    "gelu_tanh" => Act::GeluTanh,
                    a => return bad(format!("unknown activation {a:?}")),
                },
            }),
            "cnn" => {
                let blocks = m
                    .str("specular.cnn.blocks")?
                    .split(',')
                    .map(|b| {
                        let f: Vec<&str> = b.trim().split(':').collect();
                        let [e, o, s] = f[..] else {
                            return bad(format!("block {b:?} is not expand:out:stride"));
                        };
                        let key = "specular.cnn.blocks";
                        Ok(Block {
                            expand: parse_size(key, e, 64)?,
                            out: parse_size(key, o, MAX_SIZE)?,
                            stride: parse_size(key, s, 4)?,
                        })
                    })
                    .collect::<Result<Vec<Block>, Error>>()?;
                if blocks.len() > MAX_DEPTH {
                    return bad("too many blocks");
                }
                Arch::Cnn(CnnConfig {
                    image: m.size("specular.cnn.image", MAX_IMAGE)?,
                    stem: m.size("specular.cnn.stem", MAX_SIZE)?,
                    blocks,
                    last: m.size("specular.cnn.last", MAX_SIZE)?,
                    classes: m.size("specular.cnn.classes", MAX_SIZE)?,
                    bn_eps: m.num("specular.cnn.bn_eps")?,
                })
            }
            "clip" => return bad("CLIP checkpoints load with load_clip"),
            a => return bad(format!("unknown architecture {a:?}")),
        };
        let crop = match m.str("specular.crop")? {
            "center" => Crop::Center,
            "stretch" => Crop::Stretch,
            c => return bad(format!("unknown crop {c:?}")),
        };
        Ok(Spec {
            arch,
            crop,
            mean: m.three("specular.mean")?,
            std: m.three("specular.std")?,
        })
    }

    /// The metadata [`from_metadata`](Self::from_metadata) reads (for writing packs).
    pub fn to_metadata(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = Vec::new();
        let mut put = |k: &str, val: String| v.push((format!("specular.{k}"), val));
        let three = |x: [f32; 3]| format!("{},{},{}", x[0], x[1], x[2]);
        match &self.arch {
            Arch::Vit(c) => {
                put("arch", "vit".into());
                put("vit.image", c.image.to_string());
                put("vit.patch", c.patch.to_string());
                put("vit.dim", c.dim.to_string());
                put("vit.depth", c.depth.to_string());
                put("vit.heads", c.heads.to_string());
                put("vit.mlp", c.mlp.to_string());
                put(
                    "vit.pool",
                    match c.pool {
                        VitPool::Cls => "cls",
                        VitPool::Mean => "mean",
                    }
                    .into(),
                );
                if let Some(d) = c.out_dim {
                    put("vit.out_dim", d.to_string());
                }
                put("vit.eps", c.eps.to_string());
                put(
                    "vit.act",
                    if c.act == Act::GeluTanh {
                        "gelu_tanh"
                    } else {
                        "gelu"
                    }
                    .into(),
                );
            }
            Arch::Cnn(c) => {
                put("arch", "cnn".into());
                put("cnn.image", c.image.to_string());
                put("cnn.stem", c.stem.to_string());
                let blocks: Vec<String> = c
                    .blocks
                    .iter()
                    .map(|b| format!("{}:{}:{}", b.expand, b.out, b.stride))
                    .collect();
                put("cnn.blocks", blocks.join(","));
                put("cnn.last", c.last.to_string());
                put("cnn.classes", c.classes.to_string());
                put("cnn.bn_eps", c.bn_eps.to_string());
            }
            // Its sizes live in the tensor shapes.
            Arch::Clip(_) => put("arch", "clip".into()),
        }
        put(
            "crop",
            match self.crop {
                Crop::Center => "center",
                Crop::Stretch => "stretch",
            }
            .into(),
        );
        put("mean", three(self.mean));
        put("std", three(self.std));
        v
    }

    /// Thumbnail RGBA in, the network's `[1, d]` output out.
    fn graph(&self, weights: &mut dyn Weights) -> Result<Graph, Error> {
        let mut g = Graph::new();
        let img = g.image_input()?;
        let side = self.image();
        let x = g.preprocess(
            img,
            Preprocess {
                width: side,
                height: side,
                crop: self.crop,
                antialias: true,
                mean: self.mean,
                std: self.std,
            },
        )?;
        let y = match &self.arch {
            Arch::Vit(c) => vit(&mut g, x, c, weights)?,
            Arch::Cnn(c) => cnn(&mut g, x, c, weights)?,
            Arch::Clip(c) => image_tower(&mut g, x, c, weights, true)?,
        };
        g.output(y)?;
        Ok(g)
    }
}

/// What the GPU check found when the model was loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuCheck {
    /// No GPU was given, or the selector had already blocked it for inference.
    NotTried,
    /// The GPU's output matched the CPU's: the GPU path is trusted.
    Matched,
    /// It differed: the GPU path is blocked.
    Differed,
    /// The graph could not be built or run on this GPU (the reason).
    Failed(String),
}

/// One model on our own engine.
pub struct InferRuntime {
    feature: Feature,
    model: String,
    spec: Spec,
    graph: Graph,
    threads: usize,
    engines: Vec<Engine>,
    gpu: Option<Mutex<GpuGraph>>,
    check: GpuCheck,
}

impl InferRuntime {
    /// Builds the model from `weights` and, with `gpu`, compiles it there and checks it
    /// against the CPU, recording the outcome in `selector`.
    pub fn build(
        feature: Feature,
        model: &str,
        spec: Spec,
        weights: &mut dyn Weights,
        gpu: Option<&Context>,
        selector: &PathSelector,
        threads: usize,
    ) -> Result<InferRuntime, Error> {
        if feature != Feature::Search {
            // The other features need heads (boxes, text, scores) not written yet.
            return bad(format!("{feature} models are not supported yet"));
        }
        let graph = spec.graph(weights)?;
        Ok(InferRuntime::with_graph(
            feature, model, spec, graph, gpu, selector, threads,
        ))
    }

    fn with_graph(
        feature: Feature,
        model: &str,
        spec: Spec,
        graph: Graph,
        gpu: Option<&Context>,
        selector: &PathSelector,
        threads: usize,
    ) -> InferRuntime {
        let mut rt = InferRuntime {
            feature,
            model: model.into(),
            spec,
            graph,
            threads: threads.max(1),
            engines: vec![Engine::Cpu],
            gpu: None,
            check: GpuCheck::NotTried,
        };
        match gpu {
            Some(ctx) => rt.check_gpu(ctx, selector),
            None => no_gpu(&rt.model),
        }
        rt
    }

    /// Builds a CLIP from `weights` (the checkpoint's tensors, shapes as `c` says) and the
    /// tokenizer: the image half as a runtime, the text half as an encoder, both checked on
    /// `gpu` against the CPU. Matrices are kept as f16.
    #[allow(clippy::too_many_arguments)]
    pub fn build_clip(
        model: &str,
        c: ClipConfig,
        norm: Norm,
        weights: &mut dyn Weights,
        tokenizer: impl Into<Tokenizer>,
        gpu: Option<&Context>,
        selector: &PathSelector,
        threads: usize,
    ) -> Result<(InferRuntime, ClipTextEncoder), Error> {
        let spec = Spec::clip(c.clone(), norm);
        let input = ImageInput::Rgba {
            mean: spec.mean,
            std: spec.std,
        };
        let layout = c.layout;
        let clip = Clip::build(c, weights, tokenizer, input, true)?;
        let image = InferRuntime::with_graph(
            Feature::Search,
            model,
            spec,
            clip.image,
            gpu,
            selector,
            threads,
        );
        let text = ClipTextEncoder::new(model, clip.text, layout, gpu, selector, threads);
        Ok((image, text))
    }

    /// Loads a CLIP checkpoint (either layout) and its merges file, or a SigLIP checkpoint
    /// and its SentencePiece `tokenizer.model` (the kind is read from the weights).
    pub fn load_clip(
        weights: &Path,
        tokenizer: &Path,
        model: &str,
        norm: Norm,
        gpu: Option<&Context>,
        selector: &PathSelector,
        threads: usize,
    ) -> Result<(InferRuntime, ClipTextEncoder), Error> {
        let mut file = specular_infer::clip::open_weights(weights)?;
        let c = ClipConfig::detect(&file.header)?;
        let tok = Tokenizer::open(tokenizer, c.layout)?;
        InferRuntime::build_clip(model, c, norm, &mut file, tok, gpu, selector, threads)
    }

    /// Loads an installed CLIP tier (`weights` the checkpoint, `merges` its tokenizer;
    /// `norm` from the weights' manifest entry), checked against the app's selector.
    pub fn load_installed_clip(
        store: &Store,
        weights: &Installed,
        merges: &Installed,
        norm: Norm,
        gpu: Option<&Context>,
    ) -> Result<(InferRuntime, ClipTextEncoder), Error> {
        let sel = selector();
        let threads = usize::from(sel.threads()).max(1);
        InferRuntime::load_clip(
            &store.root().join(&weights.file),
            &store.root().join(&merges.file),
            &weights.key(),
            norm,
            gpu,
            sel,
            threads,
        )
    }

    /// Loads a pack (see the module docs).
    pub fn load(
        path: &Path,
        feature: Feature,
        model: &str,
        gpu: Option<&Context>,
        selector: &PathSelector,
        threads: usize,
    ) -> Result<InferRuntime, Error> {
        let mut file = SafeTensorsFile::open(path)?;
        let spec = Spec::from_metadata(&file.header.metadata)?;
        InferRuntime::build(feature, model, spec, &mut file, gpu, selector, threads)
    }

    /// Loads an installed model, checked against the app's selector.
    pub fn load_installed(
        store: &Store,
        m: &Installed,
        gpu: Option<&Context>,
    ) -> Result<InferRuntime, Error> {
        let sel = selector();
        let threads = usize::from(sel.threads()).max(1);
        InferRuntime::load(
            &store.root().join(&m.file),
            m.feature,
            &m.key(),
            gpu,
            sel,
            threads,
        )
    }

    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// The outcome of the GPU check at load.
    pub fn gpu_check(&self) -> &GpuCheck {
        &self.check
    }

    /// The test picture of the GPU check: a gradient with a pattern, wider than tall so the
    /// crop and resize run too.
    fn probe(&self) -> (u32, u32, Vec<u8>) {
        let side = self.spec.image() as u32;
        let (w, h) = (side + side / 2 + 3, side + 1);
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let p = (x * 7 + y * 13) ^ (x * y);
                px.extend_from_slice(&[
                    (x * 255 / w) as u8,
                    (y * 255 / h) as u8,
                    (p & 0xff) as u8,
                    255,
                ]);
            }
        }
        (w, h, px)
    }

    fn check_gpu(&mut self, ctx: &Context, sel: &PathSelector) {
        let path = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
        if sel.trust(Task::Inference, path) == Trust::Blocked {
            specular_log::info!(target: specular_log::JOBS,
                "ai: {}: the GPU is blocked for inference on this machine, runs on the CPU",
                self.model);
            return;
        }
        let (width, height, rgba) = self.probe();
        let thumb = Thumb {
            width,
            height,
            rgba: &rgba,
        };
        let fail = |why: String, e: &Error| {
            // A device error blocks the path; a model this GPU cannot hold only keeps this
            // model on the CPU.
            if matches!(e, Error::Gpu(_)) {
                sel.block(Task::Inference, Engine::Gpu, &why);
            }
            specular_log::warn!(target: specular_log::JOBS, "ai: {why}: runs on the CPU");
            GpuCheck::Failed(why)
        };
        let mut gg = match GpuGraph::new(ctx, &self.graph) {
            Ok(g) => g,
            Err(e) => {
                self.check = fail(format!("{}: building on the GPU: {e}", self.model), &e);
                return;
            }
        };
        let got = match gg.run(&[feed(&thumb)]) {
            Ok(out) => out,
            Err(e) => {
                self.check = fail(format!("{}: running on the GPU: {e}", self.model), &e);
                return;
            }
        };
        let (ok, close) = match (self.cpu(&thumb), embedding(got)) {
            (Ok(want), Ok(got)) => (agree(&got, &want), similarity(&got, &want)),
            _ => (false, None),
        };
        sel.verified(Task::Inference, Engine::Gpu, ok);
        log_check(&self.model, "image", ok, close);
        if ok {
            self.engines.push(Engine::Gpu);
            self.gpu = Some(Mutex::new(gg));
            self.check = GpuCheck::Matched;
        } else {
            self.check = GpuCheck::Differed;
        }
    }

    fn cpu(&self, thumb: &Thumb) -> Result<Outputs, RunError> {
        let out = specular_infer::cpu::run(&self.graph, &[feed(thumb)], self.threads)
            .map_err(|e| RunError::Failed(e.to_string()))?;
        embedding(out)
    }
}

/// The text half of a CLIP: a query's unit embedding, comparable with the image runtime's.
pub struct ClipTextEncoder {
    model: String,
    text: ClipText,
    layout: Layout,
    threads: usize,
    engines: Vec<Engine>,
    gpu: Option<Mutex<GpuGraph>>,
    check: GpuCheck,
}

impl ClipTextEncoder {
    /// The text the GPU check encodes.
    const PROBE: &'static str = "a photo of a dog on a beach, 2 people & a red kite!";

    fn new(
        model: &str,
        text: ClipText,
        layout: Layout,
        gpu: Option<&Context>,
        sel: &PathSelector,
        threads: usize,
    ) -> ClipTextEncoder {
        let mut enc = ClipTextEncoder {
            model: model.into(),
            text,
            layout,
            threads: threads.max(1),
            engines: vec![Engine::Cpu],
            gpu: None,
            check: GpuCheck::NotTried,
        };
        match gpu {
            Some(ctx) => enc.check_gpu(ctx, sel),
            None => no_gpu(&enc.model),
        }
        enc
    }

    /// The image model's key this encoder belongs to.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Engines it can run on, CPU first.
    pub fn engines(&self) -> &[Engine] {
        &self.engines
    }

    /// The outcome of the GPU check at load.
    pub fn gpu_check(&self) -> &GpuCheck {
        &self.check
    }

    /// Whether the model is a SigLIP (its similarities sit lower than CLIP's: a matching
    /// photo scores about 0.1 to 0.2).
    pub fn siglip(&self) -> bool {
        self.layout == Layout::Siglip
    }

    /// The checkpoint's kind (CLIP, SigLIP, PE Core).
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// The token ids `text` becomes (start mark, ids, end mark, padding).
    pub fn tokens(&self, text: &str) -> Vec<u32> {
        self.text.tokenizer.tokenize(text, self.text.context).0
    }

    fn check_gpu(&mut self, ctx: &Context, sel: &PathSelector) {
        let path = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
        if sel.trust(Task::Inference, path) == Trust::Blocked {
            specular_log::info!(target: specular_log::JOBS,
                "ai: {}: the GPU is blocked for inference on this machine, runs on the CPU",
                self.model);
            return;
        }
        let mut gg = match GpuGraph::new(ctx, &self.text.graph) {
            Ok(g) => g,
            Err(e) => {
                let why = format!("{}: text on the GPU: {e}", self.model);
                specular_log::warn!(target: specular_log::JOBS, "ai: {why}: runs on the CPU");
                self.check = GpuCheck::Failed(why);
                return;
            }
        };
        let (x, end) = self.text.feeds(Self::PROBE);
        let got = gg.run(&[Feed::Tensor(&x), Feed::Tensor(&end)]);
        let (ok, close) = match (got.map_err(RunError::from), self.cpu(Self::PROBE)) {
            (Ok(got), Ok(want)) => match embedding(got) {
                Ok(g) => (agree(&g, &want), similarity(&g, &want)),
                Err(_) => (false, None),
            },
            (Err(e), _) | (_, Err(e)) => {
                let why = format!("{}: text on the GPU: {e}", self.model);
                specular_log::warn!(target: specular_log::JOBS, "ai: {why}: runs on the CPU");
                self.check = GpuCheck::Failed(why);
                return;
            }
        };
        log_check(&self.model, "text", ok, close);
        if ok {
            self.engines.push(Engine::Gpu);
            self.gpu = Some(Mutex::new(gg));
            self.check = GpuCheck::Matched;
        } else {
            sel.verified(Task::Inference, Engine::Gpu, false);
            self.check = GpuCheck::Differed;
        }
    }

    fn cpu(&self, text: &str) -> Result<Outputs, RunError> {
        let (x, end) = self.text.feeds(text);
        let out = specular_infer::cpu::run(
            &self.text.graph,
            &[Feed::Tensor(&x), Feed::Tensor(&end)],
            self.threads,
        )
        .map_err(|e| RunError::Failed(e.to_string()))?;
        embedding(out)
    }

    /// `text`'s unit embedding (f16 bits) on `engine`.
    pub fn encode_on(&self, engine: Engine, text: &str) -> Result<Vec<u16>, RunError> {
        let out = match engine {
            Engine::Cpu => self.cpu(text)?,
            Engine::Gpu => {
                let Some(gg) = &self.gpu else {
                    return Err(RunError::Unsupported);
                };
                let (x, end) = self.text.feeds(text);
                let out = gg
                    .lock()
                    .run(&[Feed::Tensor(&x), Feed::Tensor(&end)])
                    .map_err(RunError::from)?;
                embedding(out)?
            }
            _ => return Err(RunError::Unsupported),
        };
        match out {
            Outputs::Embedding(v) => Ok(v),
            _ => Err(RunError::Failed("not an embedding".into())),
        }
    }

    /// `text`'s unit embedding on the GPU when it passed its check, else the CPU.
    pub fn encode(&self, text: &str) -> Result<Vec<u16>, RunError> {
        let engine = if self.gpu.is_some() {
            Engine::Gpu
        } else {
            Engine::Cpu
        };
        self.encode_on(engine, text)
    }
}

/// Logs that a model loaded without a GPU device.
fn no_gpu(model: &str) {
    specular_log::info!(target: specular_log::JOBS, "ai: {model}: no GPU device, runs on the CPU");
}

/// Logs the GPU check's outcome: whether the GPU's output matched the CPU's, and how closely.
fn log_check(model: &str, half: &str, ok: bool, close: Option<f32>) {
    let close = close.map_or_else(
        || "no embedding".to_owned(),
        |c| format!("cosine {c:.5}, needs {AGREE}"),
    );
    if ok {
        specular_log::info!(target: specular_log::JOBS,
            "ai: {model} ({half}) on the GPU matches the CPU ({close}): the GPU is used");
    } else {
        specular_log::warn!(target: specular_log::JOBS,
            "ai: {model} ({half}) on the GPU differs from the CPU ({close}): runs on the CPU");
    }
}

fn feed<'a>(t: &Thumb<'a>) -> Feed<'a> {
    Feed::Rgba {
        width: t.width,
        height: t.height,
        data: t.rgba,
    }
}

/// The graph's one `[1, d]` output as a unit f16 embedding.
fn embedding(out: Vec<Tensor>) -> Result<Outputs, RunError> {
    let [t] = &out[..] else {
        return Err(RunError::Failed(format!(
            "{} outputs, wanted one",
            out.len()
        )));
    };
    let v = t.to_f32();
    if v.iter().any(|x| !x.is_finite()) {
        return Err(RunError::Failed("the output is not finite".into()));
    }
    Ok(Outputs::Embedding(crate::search::normalize_f16(&v)))
}

impl Runtime for InferRuntime {
    fn feature(&self) -> Feature {
        self.feature
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn input_side(&self) -> u32 {
        self.spec.image() as u32
    }

    fn engines(&self) -> &[Engine] {
        &self.engines
    }

    fn run(&self, engine: Engine, thumb: &Thumb) -> Result<Outputs, RunError> {
        let want = thumb.width as usize * thumb.height as usize * 4;
        if thumb.width == 0 || thumb.height == 0 || thumb.rgba.len() != want {
            return Err(RunError::Failed("bad thumbnail".into()));
        }
        match engine {
            Engine::Cpu => self.cpu(thumb),
            Engine::Gpu => {
                let Some(gg) = &self.gpu else {
                    return Err(RunError::Unsupported);
                };
                let out = gg.lock().run(&[feed(thumb)]).map_err(RunError::from)?;
                embedding(out)
            }
            _ => Err(RunError::Unsupported),
        }
    }
}
