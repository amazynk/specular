//! Develop's AI denoise: the Denoise feature's installed network run on a photo.
//!
//! FFDNet (`specular_infer::ffdnet`, told the noise level) or NAFNet trained on SIDD
//! (`specular_infer::nafnet`, blind), by the checkpoint's layout, tiled with seamless joins
//! (`specular_infer::tiles`): the photo's cells asked for (the 100 % view's, or all of them
//! for an export) run tile by tile, each finished row of cells handed back at once. The
//! network runs on the GPU when the path selector allows and a small random network
//! computed the same there as on the CPU, else on the CPU.
//!
//! The pixels are what the network takes, planar: for FFDNet the Anscombe transform of the
//! photo's noise model with σ given ([`Denoiser::takes_sigma`]); for NAFNet sRGB-encoded
//! RGB. Develop makes them from its denoise input and turns the result back
//! (`specular_develop::ops::detail::ai`).

use std::path::{Path, PathBuf};

use specular_core::paths::{Engine, PathSelector, Task, Trust};
use specular_infer::ffdnet::{self, FfdConfig};
use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::models::RandomWeights;
use specular_infer::nafnet::{self, NafConfig};
use specular_infer::safetensors::SafeTensorsFile;
use specular_infer::tiles::{Cells, Pixels, Rows, Tiling};
use specular_infer::{Error, Feed, Graph, Tensor, cpu};

use crate::install::Installed;
use crate::manifest::{Feature, Manifest};

/// The installed denoiser: where, and its manifest id (part of a cached result's key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenoiseModel {
    pub path: PathBuf,
    pub id: String,
}

/// The installed Denoise model among `installed` (paths relative to `root`); with both
/// sizes installed, `prefer` (a manifest id) when it is one of them, else the first.
pub fn find(
    manifest: &Manifest,
    root: &Path,
    installed: &[Installed],
    prefer: Option<&str>,
) -> Option<DenoiseModel> {
    let mut found: Vec<DenoiseModel> = installed
        .iter()
        .filter(|i| i.feature == Feature::Denoise)
        .filter(|i| manifest.model(&i.id).is_some_and(|m| m.role == "denoiser"))
        .map(|i| DenoiseModel {
            path: root.join(&i.file),
            id: i.id.clone(),
        })
        .collect();
    if let Some(p) = prefer
        && let Some(k) = found.iter().position(|m| m.id == p)
    {
        return Some(found.swap_remove(k));
    }
    found.into_iter().next()
}

/// A model's name for "needs the … add-on" (the manifest's choice name), or the id.
pub fn model_name(manifest: &Manifest, id: &str) -> String {
    let tier = manifest
        .models
        .iter()
        .find(|m| m.id == id)
        .and_then(|m| m.tiers.first().copied());
    manifest
        .choices
        .iter()
        .filter(|c| c.feature == Feature::Denoise)
        .find(|c| c.tier().is_some() && c.tier() == tier)
        .map_or_else(|| id.to_owned(), |c| c.name.clone())
}

/// The default model's manifest id (what a new AI denoise row names).
pub fn default_model(manifest: &Manifest) -> Option<String> {
    let c = manifest
        .choices
        .iter()
        .find(|c| c.feature == Feature::Denoise && c.default)?;
    let t = c.tier()?;
    manifest
        .models
        .iter()
        .find(|m| m.feature == Feature::Denoise && m.tiers.contains(&t))
        .map(|m| m.id.clone())
}

/// Whether the GPU computes a small random NAFNet as the CPU does.
fn gpu_matches(ctx: &Context) -> Result<f32, Error> {
    let mut w = RandomWeights {
        seed: 24,
        f16: true,
    };
    let c = NafConfig {
        width: 8,
        enc: vec![1, 1, 1, 1],
        middle: 1,
        dec: vec![1, 1, 1, 1],
    };
    let g = nafnet::graph(&mut w, &c, (32, 32))?;
    let x = specular_infer::tensor::Rng::new(7).tensor(&[1, 3, 32, 32], 0.5);
    let feeds = [Feed::Tensor(&x)];
    let a = GpuGraph::new(ctx, &g)?.run(&feeds)?;
    let b = cpu::run(&g, &feeds, 4)?;
    let d = specular_infer::tensor::compare(&a[0].to_f32(), &b[0].to_f32(), 1e-3, 1e-3);
    let f = FfdConfig {
        channels: 3,
        width: 16,
        layers: 4,
    };
    let g = ffdnet::graph(&mut w, &f, (32, 32))?;
    let s = Tensor::f32(&[1, 1, 1, 1], vec![0.05]);
    let feeds = [Feed::Tensor(&x), Feed::Tensor(&s)];
    let a = GpuGraph::new(ctx, &g)?.run(&feeds)?;
    let b = cpu::run(&g, &feeds, 4)?;
    let e = specular_infer::tensor::compare(&a[0].to_f32(), &b[0].to_f32(), 1e-3, 1e-3);
    Ok(d.max_abs.max(e.max_abs))
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
                "ai: denoise on the GPU {} the CPU (max difference {worst:.2e})",
                if ok { "matches" } else { "differs from" });
            ok.then_some(ctx)
        }
        Err(e) => {
            if matches!(e, Error::Gpu(_)) {
                selector.block(Task::Inference, Engine::Gpu, &format!("denoise: {e}"));
            }
            specular_log::warn!(target: specular_log::JOBS, "ai: denoise on the GPU: {e}");
            None
        }
    }
}

/// Bytes a run holds besides the photo itself: the network's weights and activations
/// (`min_memory_mb` of the model, or an estimate) and three rows of cells of accumulators
/// for an image `w` wide.
pub fn memory_needed(model_mb: u64, tiling: Tiling, w: usize) -> u64 {
    let rows = 3 * (tiling.stride() + tiling.overlap) as u64 * w as u64;
    model_mb.max(256) * (1 << 20) + rows * 16 + 2 * 3 * (tiling.size as u64).pow(2) * 4
}

/// The loaded network.
pub struct Denoiser {
    model: DenoiseModel,
    graph: Graph,
    gpu: Option<GpuGraph>,
    threads: usize,
    tiling: Tiling,
    /// FFDNet: a second input, σ.
    sigma: bool,
}

impl Denoiser {
    /// Loads `model`; `ctx` is a GPU context already passed through [`choose_gpu`], or None
    /// for the CPU.
    pub fn new(
        model: DenoiseModel,
        ctx: Option<&Context>,
        threads: usize,
    ) -> Result<Denoiser, Error> {
        let mut st = SafeTensorsFile::open(&model.path)?;
        let sigma = st.header.get("blocks.0.0.weight").is_some();
        let (graph, tiling) = if sigma {
            let c = FfdConfig::from_header(&st.header)?;
            let t = Tiling::FFDNET;
            (ffdnet::graph(&mut st, &c, (t.size, t.size))?, t)
        } else {
            let c = NafConfig::from_header(&st.header)?;
            let t = Tiling::for_width(c.width);
            (nafnet::graph(&mut st, &c, (t.size, t.size))?, t)
        };
        let gpu = ctx.and_then(|c| match GpuGraph::new(c, &graph) {
            Ok(g) => Some(g),
            Err(e) => {
                specular_log::warn!(target: specular_log::JOBS, "ai: denoise on the CPU: {e}");
                None
            }
        });
        Ok(Denoiser {
            model,
            graph,
            gpu,
            threads: threads.max(1),
            tiling,
            sigma,
        })
    }

    /// The manifest id of the model (part of a cached result's key).
    pub fn id(&self) -> &str {
        &self.model.id
    }

    pub fn tiling(&self) -> Tiling {
        self.tiling
    }

    /// Whether the network is told the noise level (FFDNet: the Anscombe encoding).
    pub fn takes_sigma(&self) -> bool {
        self.sigma
    }

    /// Whether it runs on the GPU.
    pub fn on_gpu(&self) -> bool {
        self.gpu.is_some()
    }

    fn tile(&mut self, x: &Tensor, sigma: f32) -> Result<Tensor, Error> {
        let s = Tensor::f32(&[1, 1, 1, 1], vec![sigma]);
        let feeds: Vec<Feed> = if self.sigma {
            vec![Feed::Tensor(x), Feed::Tensor(&s)]
        } else {
            vec![Feed::Tensor(x)]
        };
        if let Some(g) = self.gpu.as_mut() {
            match g.run(&feeds) {
                Ok(mut o) => return Ok(o.remove(0)),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS,
                        "ai: denoise: GPU run failed, using the CPU: {e}");
                    self.gpu = None;
                }
            }
        }
        Ok(cpu::run(&self.graph, &feeds, self.threads)?.remove(0))
    }

    /// Denoises `cells` of `src` (at noise level `sigma` when the network takes one),
    /// handing each finished row of cells to `emit`; `progress(done, total)` after each
    /// tile, false to stop (then `Ok(false)`).
    pub fn run(
        &mut self,
        src: &dyn Pixels,
        cells: Cells,
        sigma: f32,
        emit: &mut dyn FnMut(Rows<'_>),
        progress: &mut dyn FnMut(usize, usize) -> bool,
    ) -> Result<bool, Error> {
        let tiling = self.tiling;
        tiling.run(src, cells, &mut |t| self.tile(t, sigma), emit, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(id: &str) -> Installed {
        Installed {
            feature: Feature::Denoise,
            tier: None,
            addon: None,
            id: id.into(),
            version: "1".into(),
            file: format!("models/{id}-1/model.safetensors"),
            sha256: [0; 32],
            size: 0,
            unverified: false,
        }
    }

    #[test]
    fn finds_the_installed_model() {
        let m = Manifest::load();
        let root = Path::new("/data/ai");
        assert_eq!(find(&m, root, &[], None), None);
        let one = [inst("ffdnet-color")];
        assert_eq!(find(&m, root, &one, None).unwrap().id, "ffdnet-color");
        let f = find(&m, root, &one, Some("ffdnet-color")).unwrap();
        assert_eq!(f.path, root.join("models/ffdnet-color-1/model.safetensors"));
        // A model the manifest does not have is not a denoiser.
        assert_eq!(find(&m, root, &[inst("gone")], None), None);
        // Another feature's files are not a denoiser.
        let mut other = inst("efficientsam-ti");
        other.feature = Feature::Masks;
        assert_eq!(find(&m, root, &[other], None), None);
    }

    #[test]
    fn names_and_default() {
        let m = Manifest::load();
        assert_eq!(default_model(&m).as_deref(), Some("ffdnet-color"));
        assert_eq!(model_name(&m, "ffdnet-color"), "FFDNet");
        assert_eq!(model_name(&m, "gone"), "gone");
    }

    #[test]
    fn memory_grows_with_width() {
        let a = memory_needed(700, Tiling::DEFAULT, 4000);
        let b = memory_needed(700, Tiling::DEFAULT, 8000);
        assert!(b > a && a > 700 << 20);
    }
}
