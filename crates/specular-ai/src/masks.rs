//! Develop's AI masks: the Masks feature's installed networks run on a photo.
//!
//! - **Object at a click**: EfficientSAM (`specular_infer::sam`), the feature's own model
//!   (small or standard). The photo is encoded once ([`Masker::embed`]); each set of
//!   clicks and box is then a few milliseconds ([`Masker::object`]).
//! - **Subject**: IS-Net (`specular_infer::isnet`), the `subject` add-on.
//! - **Sky**: UperNet ConvNeXt-T on ADE20K (`specular_infer::upernet`), the `sky` add-on.
//!
//! Every result is a [`ProbMap`]: probabilities 0..1 over the whole picture it was given,
//! at the network's own resolution; Develop upsamples it to the photo guided by the photo
//! (`specular_develop::mask::ai`). The networks run on the GPU when the path selector allows
//! and a small random network computed the same there as on the CPU, else on the CPU.

use std::path::{Path, PathBuf};

use specular_core::paths::{Engine, PathSelector, Task, Trust};
use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::models::RandomWeights;
use specular_infer::onnx::Model as OnnxModel;
use specular_infer::safetensors::SafeTensorsFile;
use specular_infer::sam::{self, Prompt, PromptTables, SamConfig};
use specular_infer::upernet::{self, UperConfig};
use specular_infer::{Error, Feed, Graph, Tensor, cpu, isnet};

use crate::install::Installed;
use crate::manifest::{Feature, Manifest};

/// What a mask row asks the networks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Subject,
    Sky,
    Object,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Subject, Kind::Sky, Kind::Object];

    /// Stable id (sidecars, the cache, the add-on's id for subject and sky).
    pub const fn id(self) -> &'static str {
        match self {
            Kind::Subject => "subject",
            Kind::Sky => "sky",
            Kind::Object => "object",
        }
    }

    pub fn from_id(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.id() == s)
    }

    /// The manifest role of the model it needs.
    pub const fn role(self) -> &'static str {
        match self {
            Kind::Subject => "subject-segmenter",
            Kind::Sky => "semantic-segmenter",
            Kind::Object => "click-segmenter",
        }
    }

    /// The add-on that brings its model (None: the feature's own model).
    pub const fn addon(self) -> Option<&'static str> {
        match self {
            Kind::Subject => Some("subject"),
            Kind::Sky => Some("sky"),
            Kind::Object => None,
        }
    }
}

/// One installed model file: where, and its manifest id (part of a cached mask's key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaskModel {
    pub path: PathBuf,
    pub id: String,
}

/// The Masks feature's installed files, per kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaskFiles {
    pub object: Option<MaskModel>,
    pub subject: Option<MaskModel>,
    pub sky: Option<MaskModel>,
}

impl MaskFiles {
    /// The files among `installed` (paths relative to `root`), their roles from `manifest`.
    pub fn find(manifest: &Manifest, root: &Path, installed: &[Installed]) -> MaskFiles {
        let mut f = MaskFiles::default();
        for i in installed.iter().filter(|i| i.feature == Feature::Masks) {
            let Some(m) = manifest.model(&i.id) else {
                continue;
            };
            let found = Some(MaskModel {
                path: root.join(&i.file),
                id: m.id.clone(),
            });
            match m.role.as_str() {
                "click-segmenter" => f.object = found,
                "subject-segmenter" => f.subject = found,
                "semantic-segmenter" => f.sky = found,
                _ => {}
            }
        }
        f
    }

    pub fn get(&self, k: Kind) -> Option<&MaskModel> {
        match k {
            Kind::Subject => self.subject.as_ref(),
            Kind::Sky => self.sky.as_ref(),
            Kind::Object => self.object.as_ref(),
        }
    }
}

/// The name of the model a kind needs, for "needs the … add-on" (the manifest's choice
/// name: the default size for objects).
pub fn model_name(manifest: &Manifest, k: Kind) -> String {
    manifest
        .choices
        .iter()
        .filter(|c| c.feature == Feature::Masks)
        .find(|c| match k.addon() {
            Some(a) => matches!(&c.slot, crate::manifest::Slot::Addon(x) if x == a),
            None => c.default,
        })
        .map_or_else(|| k.id().to_owned(), |c| c.name.clone())
}

/// Probabilities 0..1 over a whole picture, row-major, `w × h`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbMap {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

/// An RGBA8 picture (rows top to bottom, no padding).
#[derive(Clone, Copy, Debug)]
pub struct Rgba<'a> {
    pub w: u32,
    pub h: u32,
    pub data: &'a [u8],
}

impl Rgba<'_> {
    fn feed(&self) -> Feed<'_> {
        Feed::Rgba {
            width: self.w,
            height: self.h,
            data: self.data,
        }
    }
}

/// A photo's EfficientSAM embedding (kept by the app while the photo is open).
#[derive(Clone, Debug)]
pub struct Embedding {
    pub tensor: Tensor,
    /// The model it came from.
    pub model: String,
}

/// A network on the GPU (when allowed and verified) or the CPU.
struct Net {
    graph: Graph,
    gpu: Option<GpuGraph>,
}

impl Net {
    fn new(graph: Graph, ctx: Option<&Context>) -> Net {
        let gpu = ctx.and_then(|c| match GpuGraph::new(c, &graph) {
            Ok(g) => Some(g),
            Err(e) => {
                specular_log::warn!(target: specular_log::JOBS, "ai: masks on the CPU: {e}");
                None
            }
        });
        Net { graph, gpu }
    }

    fn run(&mut self, feeds: &[Feed], threads: usize) -> Result<Vec<Tensor>, Error> {
        if let Some(g) = self.gpu.as_mut() {
            match g.run(feeds) {
                Ok(o) => return Ok(o),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS,
                        "ai: masks: GPU run failed, using the CPU: {e}");
                    self.gpu = None;
                }
            }
        }
        cpu::run(&self.graph, feeds, threads)
    }
}

/// Whether the GPU computes a small random SAM decoder as the CPU does.
fn gpu_matches(ctx: &Context) -> Result<f32, Error> {
    let mut w = RandomWeights::new(23);
    let tables = PromptTables::read(&mut w)?;
    let g = sam::decoder(&mut w, &tables)?;
    let emb = specular_infer::tensor::Rng::new(5).tensor(&[1, sam::DIM, sam::GRID, sam::GRID], 1.0);
    let prompt = Prompt {
        points: vec![([0.5, 0.5], sam::PointKind::Positive)],
    };
    let sparse = prompt.embed(&tables)?;
    let feeds = [Feed::Tensor(&emb), Feed::Tensor(&sparse)];
    let a = GpuGraph::new(ctx, &g)?.run(&feeds)?;
    let b = cpu::run(&g, &feeds, 4)?;
    let d = specular_infer::tensor::compare(&a[0].to_f32(), &b[0].to_f32(), 1e-3, 1e-3);
    Ok(d.max_abs)
}

/// The GPU context to use, after the selector and a check (None: the CPU).
pub fn choose_gpu<'a>(gpu: Option<&'a Context>, selector: &PathSelector) -> Option<&'a Context> {
    let ctx = gpu?;
    let path = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
    if selector.trust(Task::Inference, path) == Trust::Blocked {
        return None;
    }
    match gpu_matches(ctx) {
        Ok(worst) => {
            let ok = worst < 1e-2;
            selector.verified(Task::Inference, Engine::Gpu, ok);
            specular_log::info!(target: specular_log::JOBS,
                "ai: masks on the GPU {} the CPU (max difference {worst:.2e})",
                if ok { "match" } else { "differ from" });
            ok.then_some(ctx)
        }
        Err(e) => {
            if matches!(e, Error::Gpu(_)) {
                selector.block(Task::Inference, Engine::Gpu, &format!("masks: {e}"));
            }
            specular_log::warn!(target: specular_log::JOBS, "ai: masks on the GPU: {e}");
            None
        }
    }
}

/// The loaded networks, each built on first use.
pub struct Masker {
    files: MaskFiles,
    ctx: Option<Context>,
    threads: usize,
    sam: Option<(Net, Net, PromptTables, String)>,
    isnet: Option<Net>,
    /// UperNet per input size.
    uper: Option<((usize, usize), Net)>,
}

/// Long side of UperNet's input (sky regions are large; edges come from the guided
/// upsampling).
pub const SKY_SIDE: usize = 384;

impl Masker {
    /// `ctx`: a GPU context already passed through [`choose_gpu`], or None for the CPU.
    pub fn new(files: MaskFiles, ctx: Option<Context>, threads: usize) -> Masker {
        Masker {
            files,
            ctx,
            threads: threads.max(1),
            sam: None,
            isnet: None,
            uper: None,
        }
    }

    pub fn files(&self) -> &MaskFiles {
        &self.files
    }

    fn missing(k: Kind) -> Error {
        Error::Weights(format!("masks: no {} model installed", k.id()))
    }

    fn sam(&mut self) -> Result<&mut (Net, Net, PromptTables, String), Error> {
        if self.sam.is_none() {
            let m = self
                .files
                .object
                .clone()
                .ok_or(Self::missing(Kind::Object))?;
            let mut st = SafeTensorsFile::open(&m.path)?;
            let c = SamConfig::from_width(sam::width_of(&st)?)?;
            let enc = sam::encoder(&mut st, c, true)?;
            let tables = PromptTables::read(&mut st)?;
            let dec = sam::decoder(&mut st, &tables)?;
            let ctx = self.ctx.as_ref();
            self.sam = Some((Net::new(enc, ctx), Net::new(dec, ctx), tables, m.id));
        }
        Ok(self.sam.as_mut().expect("loaded above"))
    }

    /// The photo's embedding for clicks (about a second; keep it while the photo is open).
    pub fn embed(&mut self, img: Rgba) -> Result<Embedding, Error> {
        let threads = self.threads;
        let (enc, _, _, id) = self.sam()?;
        let t = enc.run(&[img.feed()], threads)?.remove(0);
        Ok(Embedding {
            tensor: t,
            model: id.clone(),
        })
    }

    /// The object at `prompt` (points in fractions of the picture): the best of the three
    /// candidates, 256² over the whole picture.
    pub fn object(&mut self, emb: &Embedding, prompt: &Prompt) -> Result<ProbMap, Error> {
        let threads = self.threads;
        let (_, dec, tables, _) = self.sam()?;
        let sparse = prompt.embed(tables)?;
        let out = dec.run(&[Feed::Tensor(&emb.tensor), Feed::Tensor(&sparse)], threads)?;
        let (masks, iou) = (out[0].to_f32(), out[1].to_f32());
        let best = sam::best(&iou);
        let n = sam::MASK * sam::MASK;
        let data = masks[best * n..(best + 1) * n]
            .iter()
            .map(|&l| 1.0 / (1.0 + (-l).exp()))
            .collect();
        Ok(ProbMap {
            w: sam::MASK,
            h: sam::MASK,
            data,
        })
    }

    /// The photo's main subject, 1024² over the whole picture.
    pub fn subject(&mut self, img: Rgba) -> Result<ProbMap, Error> {
        if self.isnet.is_none() {
            let m = self
                .files
                .subject
                .clone()
                .ok_or(Self::missing(Kind::Subject))?;
            let bytes = std::fs::read(&m.path)
                .map_err(|e| Error::Weights(format!("{}: {e}", m.path.display())))?;
            let model = OnnxModel::parse(&bytes)?;
            let g = isnet::graph(&model, true)?;
            self.isnet = Some(Net::new(g, self.ctx.as_ref()));
        }
        let threads = self.threads;
        let net = self.isnet.as_mut().expect("loaded above");
        let p = net.run(&[img.feed()], threads)?.remove(0);
        Ok(ProbMap {
            w: isnet::INPUT,
            h: isnet::INPUT,
            data: p.to_f32(),
        })
    }

    /// The sky's probability, at a quarter of UperNet's input.
    pub fn sky(&mut self, img: Rgba) -> Result<ProbMap, Error> {
        let size = upernet::input_size(img.w as usize, img.h as usize, SKY_SIDE);
        if self.uper.as_ref().is_none_or(|(s, _)| *s != size) {
            let m = self.files.sky.clone().ok_or(Self::missing(Kind::Sky))?;
            let mut st = SafeTensorsFile::open(&m.path)?;
            let c = UperConfig::from_header(&st.header)?;
            let g = upernet::graph(&mut st, &c, size, &[upernet::class::SKY])?;
            self.uper = Some((size, Net::new(g, self.ctx.as_ref())));
        }
        let threads = self.threads;
        let (_, net) = self.uper.as_mut().expect("loaded above");
        let p = net.run(&[img.feed()], threads)?.remove(0);
        Ok(ProbMap {
            w: size.0 / 4,
            h: size.1 / 4,
            data: p.to_f32(),
        })
    }

    /// The model id a kind's mask comes from now (part of its cache key).
    pub fn model_id(&self, k: Kind) -> Option<&str> {
        self.files.get(k).map(|m| m.id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(id: &str, addon: Option<&str>) -> Installed {
        Installed {
            feature: Feature::Masks,
            tier: None,
            addon: addon.map(Into::into),
            id: id.into(),
            version: "1".into(),
            file: format!("models/{id}-1/model.x"),
            sha256: [0; 32],
            size: 0,
            unverified: false,
        }
    }

    #[test]
    fn finds_the_installed_files() {
        let m = Manifest::load();
        let root = Path::new("/data/ai");
        let f = MaskFiles::find(
            &m,
            root,
            &[
                inst("efficientsam-ti", None),
                inst("upernet-convnext-tiny-ade20k", Some("sky")),
            ],
        );
        assert_eq!(f.object.as_ref().unwrap().id, "efficientsam-ti");
        assert_eq!(
            f.sky.as_ref().unwrap().path,
            root.join("models/upernet-convnext-tiny-ade20k-1/model.x")
        );
        assert!(f.subject.is_none());
        assert!(f.get(Kind::Subject).is_none());
    }

    #[test]
    fn kinds_and_names() {
        for k in Kind::ALL {
            assert_eq!(Kind::from_id(k.id()), Some(k));
        }
        let m = Manifest::load();
        assert_eq!(model_name(&m, Kind::Subject), "IS-Net");
        assert_eq!(model_name(&m, Kind::Sky), "UperNet ConvNeXt-T");
        assert_eq!(model_name(&m, Kind::Object), "EfficientSAM Ti");
        // Every role a kind needs is offered by the manifest.
        for k in Kind::ALL {
            assert!(
                m.models
                    .iter()
                    .any(|x| x.feature == Feature::Masks && x.role == k.role()),
                "{k:?}"
            );
        }
    }

    #[test]
    fn nothing_installed_is_an_error_not_a_panic() {
        let mut mk = Masker::new(MaskFiles::default(), None, 1);
        let px = [0u8; 16];
        let img = Rgba {
            w: 2,
            h: 2,
            data: &px,
        };
        assert!(mk.subject(img).is_err());
        assert!(mk.sky(img).is_err());
        assert!(mk.embed(img).is_err());
    }
}
