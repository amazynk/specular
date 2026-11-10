//! Develop's "Remove (AI)" spots and depth tools: the Remove and Depth features'
//! installed networks run on a photo.
//!
//! - **Remove**: Big-LaMa (`specular_infer::lama`) fills a hole in an RGB crop
//!   ([`Inpainter::fill`]). The network takes sides that are multiples of 8; a crop is run
//!   at a size from a short list ([`work_side`]), so the GPU builds few graphs.
//! - **Depth**: Depth Anything V2 Small (`specular_infer::depth`) gives relative inverse
//!   depth over a whole picture ([`DepthNet::depth`]), larger nearer.
//!
//! Each runs on the GPU when the path selector allows and the GPU computed a small random
//! network as the CPU did (the masks' check, [`crate::masks::choose_gpu`]), else on the CPU.
//! A long run checks `stop` between the graph build and the run (a run itself is one GPU
//! submission and is not interrupted).

use std::path::PathBuf;

use specular_infer::depth::{self, DepthConfig};
use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::safetensors::SafeTensorsFile;
use specular_infer::{Error, Feed, Graph, Tensor, cpu, lama};

use crate::install::Installed;
use crate::manifest::{Feature, Manifest};
use crate::masks::{ProbMap, Rgba};

/// One installed model file: where, and its manifest id (part of a cached result's key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetModel {
    pub path: PathBuf,
    pub id: String,
}

/// The installed model of `feature` (the first installed one).
pub fn installed_model(
    manifest: &Manifest,
    root: &std::path::Path,
    installed: &[Installed],
    feature: Feature,
) -> Option<NetModel> {
    installed
        .iter()
        .filter(|i| i.feature == feature)
        .find_map(|i| {
            manifest.model(&i.id).map(|m| NetModel {
                path: root.join(&i.file),
                id: m.id.clone(),
            })
        })
}

/// The name of `feature`'s default model, for "needs the … add-on".
pub fn model_name(manifest: &Manifest, feature: Feature) -> String {
    manifest
        .choices
        .iter()
        .filter(|c| c.feature == feature)
        .find(|c| c.default)
        .map_or_else(|| feature.id().to_owned(), |c| c.name.clone())
}

/// A network on the GPU (when given) or the CPU.
struct Net {
    graph: Graph,
    gpu: Option<GpuGraph>,
}

impl Net {
    fn new(graph: Graph, ctx: Option<&Context>) -> Net {
        let gpu = ctx.and_then(|c| match GpuGraph::new(c, &graph) {
            Ok(g) => Some(g),
            Err(e) => {
                specular_log::warn!(target: specular_log::JOBS, "ai: develop network on the CPU: {e}");
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
                        "ai: develop network: GPU run failed, using the CPU: {e}");
                    self.gpu = None;
                }
            }
        }
        cpu::run(&self.graph, feeds, threads)
    }
}

/// Network input sides for removal crops: each crop is resampled to the smallest of these
/// that holds it (or the largest), so a few graphs serve every crop. 512² lost the Radeon
/// 860M device under load (one dispatch past the driver's timeout); 480² ran.
pub const WORK_SIDES: [usize; 7] = [64, 128, 192, 256, 320, 384, 448];

/// The work side for a crop of long side `long` pixels.
pub fn work_side(long: usize) -> usize {
    WORK_SIDES
        .iter()
        .copied()
        .find(|&s| s >= long)
        .unwrap_or(WORK_SIDES[WORK_SIDES.len() - 1])
}

/// An RGB crop in 0..1 (row-major, interleaved), `w × h`.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgb {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

/// The Remove feature's network, built per work size on first use.
pub struct Inpainter {
    model: NetModel,
    ctx: Option<Context>,
    threads: usize,
    net: Option<((usize, usize), Net)>,
}

impl Inpainter {
    /// `ctx`: a GPU context already passed through [`crate::masks::choose_gpu`], or None.
    pub fn new(model: NetModel, ctx: Option<Context>, threads: usize) -> Inpainter {
        Inpainter {
            model,
            ctx,
            threads: threads.max(1),
            net: None,
        }
    }

    pub fn model(&self) -> &NetModel {
        &self.model
    }

    /// Fills the hole `mask` (0..1 over the crop, ≥ ½ is the hole) of `img`, both `w × h`
    /// with sides multiples of [`lama::SIDE_STEP`]: the network's RGB (0..1) everywhere.
    pub fn fill(&mut self, img: &Rgb, mask: &[f32], stop: &dyn Fn() -> bool) -> Result<Rgb, Error> {
        let size = (img.w, img.h);
        if mask.len() != img.w * img.h || img.data.len() != 3 * img.w * img.h {
            return Err(Error::Feed("remove: crop and hole sizes differ".into()));
        }
        if self.net.as_ref().is_none_or(|(s, _)| *s != size) {
            let mut st = SafeTensorsFile::open(&self.model.path)?;
            let g = lama::graph(&mut st, size)?;
            self.net = Some((size, Net::new(g, self.ctx.as_ref())));
        }
        if stop() {
            return Err(Error::Feed("stopped".into()));
        }
        let x = lama::input(&img.data, mask, size);
        let threads = self.threads;
        let (_, net) = self.net.as_mut().expect("built above");
        let out = net.run(&[Feed::Tensor(&x)], threads)?.remove(0).to_f32();
        let n = img.w * img.h;
        let mut data = vec![0f32; 3 * n];
        for i in 0..n {
            for c in 0..3 {
                data[3 * i + c] = out[c * n + i];
            }
        }
        Ok(Rgb {
            w: img.w,
            h: img.h,
            data,
        })
    }
}

/// The Depth feature's network, built per input size on first use.
pub struct DepthNet {
    model: NetModel,
    ctx: Option<Context>,
    threads: usize,
    net: Option<((usize, usize), Net)>,
}

impl DepthNet {
    pub fn new(model: NetModel, ctx: Option<Context>, threads: usize) -> DepthNet {
        DepthNet {
            model,
            ctx,
            threads: threads.max(1),
            net: None,
        }
    }

    pub fn model(&self) -> &NetModel {
        &self.model
    }

    /// Relative inverse depth over the whole picture (larger is nearer), at the network's
    /// input size (short side about [`depth::SIDE`]).
    pub fn depth(&mut self, img: Rgba) -> Result<ProbMap, Error> {
        let size = depth::input_size(img.w as usize, img.h as usize, depth::SIDE);
        if self.net.as_ref().is_none_or(|(s, _)| *s != size) {
            let mut st = SafeTensorsFile::open(&self.model.path)?;
            let c = DepthConfig::from_header(&st.header)?;
            let g = depth::graph(&mut st, &c, size)?;
            self.net = Some((size, Net::new(g, self.ctx.as_ref())));
        }
        let threads = self.threads;
        let (_, net) = self.net.as_mut().expect("built above");
        let feed = Feed::Rgba {
            width: img.w,
            height: img.h,
            data: img.data,
        };
        let p = net.run(&[feed], threads)?.remove(0);
        Ok(ProbMap {
            w: size.0,
            h: size.1,
            data: p.to_f32(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_sides_hold_the_crop() {
        assert_eq!(work_side(10), 64);
        assert_eq!(work_side(64), 64);
        assert_eq!(work_side(65), 128);
        assert_eq!(work_side(400), 448);
        assert_eq!(work_side(5000), 448);
        for s in WORK_SIDES {
            assert!(s.is_multiple_of(lama::SIDE_STEP) && s >= lama::MIN_SIDE);
        }
    }

    #[test]
    fn manifest_offers_remove_and_depth() {
        let m = Manifest::load();
        assert_eq!(model_name(&m, Feature::Remove), "Big-LaMa");
        assert_eq!(model_name(&m, Feature::Depth), "Depth Anything V2 Small");
        let lama = m.model("big-lama").unwrap();
        assert_eq!(lama.feature, Feature::Remove);
        assert_eq!(lama.runtime.arch.as_deref(), Some("lama"));
        let da = m.model("depth-anything-v2-small").unwrap();
        assert_eq!(da.feature, Feature::Depth);
        // Only permissive weights: the larger Depth Anything sizes are non-commercial.
        for x in m
            .models
            .iter()
            .filter(|x| matches!(x.feature, Feature::Remove | Feature::Depth))
        {
            assert!(
                ["Apache-2.0", "MIT", "BSD-3-Clause"].contains(&x.licence.as_str()),
                "{}",
                x.id
            );
        }
    }

    #[test]
    fn finds_the_installed_model() {
        let m = Manifest::load();
        let root = std::path::Path::new("/data/ai");
        let inst = Installed {
            feature: Feature::Depth,
            tier: None,
            addon: None,
            id: "depth-anything-v2-small".into(),
            version: "1".into(),
            file: "models/depth-anything-v2-small-1/model.safetensors".into(),
            sha256: [0; 32],
            size: 0,
            unverified: false,
        };
        let got = installed_model(&m, root, &[inst], Feature::Depth).unwrap();
        assert_eq!(got.id, "depth-anything-v2-small");
        assert!(installed_model(&m, root, &[], Feature::Remove).is_none());
    }
}
