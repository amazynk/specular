//! People and Culling on our own engine (`specular_infer::face`).
//!
//! - **People**: BlazeFace full range finds faces (whole photo and tiles), a recogniser
//!   (SFace or AuraFace) embeds each aligned face; [`regroup`] groups the embeddings into
//!   people by average-linkage clustering, keeping what the user named or placed.
//! - **Culling**: the Face Landmarker bundle's detector, mesh and blendshapes give how closed
//!   each face's eyes are; blur and exposure come from the pixels alone.
//!
//! Both read the photo at up to [`SIDE`] pixels (decoded from the file: a 512 px thumbnail
//! holds group-photo faces of a few pixels). Each [`FaceRuntime`] keeps a CPU pipeline (the
//! reference) and, when the GPU matched it on a test picture at load, a GPU one.

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::Connection;
use specular_core::paths::{Engine, PathSelector, Task, Trust};
use specular_infer::Error;
use specular_infer::face::blazeface::{Detector, Face, Range};
use specular_infer::face::embed::{Embedder, Kind};
use specular_infer::face::landmarker::Landmarker;
use specular_infer::face::{Affine, Image, cluster, cosine, normalize, quality, shrink, warp};
use specular_infer::gpu::Context;
use specular_library::ai::{self as lib, EyeFace, Grouping, Scores};

use crate::infer::GpuCheck;
use crate::manifest::Feature;
use crate::pack::{IndexModel, Pack};
use crate::runtime::{FaceBox, Outputs, RunError, Runtime, Thumb};

/// The long side photos are read at for faces.
pub const SIDE: u32 = 2048;
/// Faces smaller than this (pixels, at the size read) are not embedded or scored: too
/// little to tell anyone or any eye apart.
pub const MIN_FACE: f32 = 24.0;
/// At most this many faces (the largest) are scored for closed eyes per photo.
const MAX_EYES: usize = 12;
/// Below this eyes count as closed in a photo's flag (`eyes_open`).
pub const EYES_OPEN_MIN: f32 = 0.5;

/// The cosine at which two faces' groups join, per recogniser: the highest with no wrong
/// merges on the identity check set (`tools/corpus/face-oracle.py`, 40 people).
pub fn threshold(kind: Kind) -> f32 {
    match kind {
        Kind::Sface => 0.40,
        Kind::AuraFace => 0.30,
    }
}

/// The files of an installed People or Culling size.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceParts {
    /// People: the BlazeFace full-range file.
    pub detector: Option<PathBuf>,
    pub recogniser: Option<(PathBuf, Kind)>,
    /// Culling: the Face Landmarker bundle.
    pub landmarker: Option<PathBuf>,
}

/// The installed files of `im` by their manifest roles (None: a needed one is missing).
pub fn face_parts(pack: &Pack, im: &IndexModel) -> Option<FaceParts> {
    let mut p = FaceParts {
        detector: None,
        recogniser: None,
        landmarker: None,
    };
    let root = pack.store().root();
    let mut feature = None;
    for m in &im.models {
        let Some(mm) = pack
            .manifest()
            .models
            .iter()
            .find(|x| x.feature == m.feature && x.id == m.id && x.version == m.version)
        else {
            continue;
        };
        feature = Some(m.feature);
        let path = root.join(&m.file);
        match (mm.role.as_str(), mm.runtime.arch.as_deref()) {
            ("face-detector", _) => p.detector = Some(path),
            ("face-recogniser", Some("sface")) => p.recogniser = Some((path, Kind::Sface)),
            ("face-recogniser", Some("auraface")) => p.recogniser = Some((path, Kind::AuraFace)),
            ("face-landmarker", _) => p.landmarker = Some(path),
            _ => {}
        }
    }
    match feature? {
        Feature::People if p.detector.is_some() && p.recogniser.is_some() => Some(p),
        Feature::Culling if p.landmarker.is_some() => Some(p),
        _ => None,
    }
}

/// One engine's networks.
struct Pipeline {
    det: Option<Detector>,
    emb: Option<Embedder>,
    lm: Option<Landmarker>,
}

impl Pipeline {
    fn build(
        bytes: &Bytes,
        kind: Option<Kind>,
        gpu: Option<&Context>,
        threads: usize,
    ) -> Result<Pipeline, Error> {
        Ok(Pipeline {
            det: match &bytes.det {
                Some(b) => Some(Detector::new(b, Range::Full, gpu, threads)?),
                None => None,
            },
            emb: match (&bytes.emb, kind) {
                (Some(b), Some(k)) => Some(Embedder::new(b, k, true, gpu, threads)?),
                _ => None,
            },
            lm: match &bytes.lm {
                Some(b) => Some(Landmarker::new(b, gpu, threads)?),
                None => None,
            },
        })
    }

    /// Whether every network compiled for the GPU.
    fn all_on_gpu(&self) -> bool {
        self.det.as_ref().is_none_or(Detector::on_gpu)
            && self.emb.as_ref().is_none_or(Embedder::on_gpu)
            && self.lm.as_ref().is_none_or(Landmarker::on_gpu)
    }

    /// The raw outputs on a test picture (detector logits and boxes, embedding, mesh), for
    /// the GPU check.
    fn probe(&mut self, img: Image, cpu: bool) -> Result<Vec<f32>, Error> {
        let mut out = Vec::new();
        let face = probe_face(img);
        if let Some(d) = self.det.as_mut() {
            let x = d.input(img, Detector::whole(img));
            for t in d.raw(&x, cpu)? {
                out.extend(t.to_f32());
            }
        }
        if let Some(e) = self.emb.as_mut()
            && let Some(x) = e.input(img, &face)
        {
            out.extend(e.raw(&x, cpu)?);
        }
        if let Some(l) = self.lm.as_mut() {
            let x = l.mesh_input(img, &face);
            let (p, _) = l.mesh_raw(&x, cpu)?;
            out.extend(p);
        }
        Ok(out)
    }
}

struct Bytes {
    det: Option<Vec<u8>>,
    emb: Option<Vec<u8>>,
    lm: Option<Vec<u8>>,
}

/// A face where the test picture draws one.
fn probe_face(img: Image) -> Face {
    let (w, h) = (img.width as f32, img.height as f32);
    let (cx, cy, s) = (w * 0.5, h * 0.5, h * 0.5);
    Face {
        rect: [cx - s / 2.0, cy - s / 2.0, s, s],
        points: [
            [cx - s * 0.18, cy - s * 0.12],
            [cx + s * 0.18, cy - s * 0.12],
            [cx, cy + s * 0.05],
            [cx, cy + s * 0.25],
            [cx - s * 0.45, cy],
            [cx + s * 0.45, cy],
        ],
        score: 1.0,
    }
}

/// The GPU check's picture: a skin-coloured oval with eyes and a mouth on a gradient.
fn probe_picture() -> (usize, usize, Vec<u8>) {
    let (w, h) = (320usize, 240usize);
    let mut v = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = ((x as f32 - 160.0) / 60.0, (y as f32 - 120.0) / 80.0);
            let mut c = [(x * 255 / w) as u8, (y * 255 / h) as u8, 128];
            if dx * dx + dy * dy < 1.0 {
                c = [225, 180, 160];
            }
            for ex in [-0.45f32, 0.45] {
                let (u, q) = (dx - ex, dy + 0.25);
                if u * u + q * q * 4.0 < 0.04 {
                    c = [40, 30, 30];
                }
            }
            if dy > 0.35 && dy < 0.45 && dx.abs() < 0.4 {
                c = [150, 60, 60];
            }
            v.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    (w, h, v)
}

/// People or Culling on our engine.
pub struct FaceRuntime {
    feature: Feature,
    model: String,
    kind: Option<Kind>,
    engines: Vec<Engine>,
    cpu: Mutex<Pipeline>,
    gpu: Option<Mutex<Pipeline>>,
    check: GpuCheck,
}

fn read(p: &Option<PathBuf>) -> Result<Option<Vec<u8>>, Error> {
    match p {
        Some(p) => std::fs::read(p)
            .map(Some)
            .map_err(|e| Error::Weights(format!("{}: {e}", p.display()))),
        None => Ok(None),
    }
}

impl FaceRuntime {
    /// Loads `parts` for `feature` under catalogue key `model`; with `gpu`, compiles them
    /// there too and checks them against the CPU, recording the outcome in `sel`.
    pub fn load(
        feature: Feature,
        model: &str,
        parts: &FaceParts,
        gpu: Option<&Context>,
        sel: &PathSelector,
        threads: usize,
    ) -> Result<FaceRuntime, Error> {
        let bytes = Bytes {
            det: if feature == Feature::People {
                read(&parts.detector)?
            } else {
                None
            },
            emb: if feature == Feature::People {
                read(&parts.recogniser.as_ref().map(|r| r.0.clone()))?
            } else {
                None
            },
            lm: if feature == Feature::Culling {
                read(&parts.landmarker)?
            } else {
                None
            },
        };
        let kind = parts.recogniser.as_ref().map(|r| r.1);
        let cpu = Pipeline::build(&bytes, kind, None, threads)?;
        let mut rt = FaceRuntime {
            feature,
            model: model.to_owned(),
            kind,
            engines: vec![Engine::Cpu],
            cpu: Mutex::new(cpu),
            gpu: None,
            check: GpuCheck::NotTried,
        };
        if let Some(ctx) = gpu {
            rt.check_gpu(&bytes, ctx, sel, threads);
        }
        Ok(rt)
    }

    fn check_gpu(&mut self, bytes: &Bytes, ctx: &Context, sel: &PathSelector, threads: usize) {
        let path = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
        if sel.trust(Task::Inference, path) == Trust::Blocked {
            return;
        }
        let mut g = match Pipeline::build(bytes, self.kind, Some(ctx), threads) {
            Ok(g) if g.all_on_gpu() => g,
            Ok(_) => {
                self.check = GpuCheck::Failed(format!("{}: not built on the GPU", self.model));
                return;
            }
            Err(e) => {
                self.check = GpuCheck::Failed(format!("{}: {e}", self.model));
                return;
            }
        };
        let (w, h, px) = probe_picture();
        let img = Image {
            width: w,
            height: h,
            rgba: &px,
        };
        let got = g.probe(img, false);
        let want = self.cpu.lock().probe(img, true);
        let (ok, close) = match (&got, &want) {
            (Ok(a), Ok(b)) if a.len() == b.len() && !a.is_empty() => {
                let c = cosine(a, b);
                (c >= crate::runtime::AGREE, c)
            }
            _ => (false, 0.0),
        };
        sel.verified(Task::Inference, Engine::Gpu, ok);
        if ok {
            specular_log::info!(target: specular_log::JOBS,
                "ai: {} on the GPU matches the CPU (cosine {close:.6}): the GPU is used", self.model);
            self.engines.push(Engine::Gpu);
            self.gpu = Some(Mutex::new(g));
            self.check = GpuCheck::Matched;
        } else {
            specular_log::warn!(target: specular_log::JOBS,
                "ai: {} on the GPU differs from the CPU (cosine {close:.6}): runs on the CPU", self.model);
            self.check = GpuCheck::Differed;
        }
    }

    pub fn gpu_check(&self) -> &GpuCheck {
        &self.check
    }

    /// The recogniser (People).
    pub fn kind(&self) -> Option<Kind> {
        self.kind
    }
}

/// Faces of a photo with their embeddings (People).
fn people(p: &mut Pipeline, img: Image) -> Result<Outputs, Error> {
    let (Some(det), Some(emb)) = (p.det.as_mut(), p.emb.as_mut()) else {
        return Err(Error::Weights(
            "People needs a detector and a recogniser".into(),
        ));
    };
    let faces: Vec<_> = specular_infer::face::detect_any_roll(det, img)?
        .into_iter()
        .filter(|f| f.rect[2].min(f.rect[3]) >= MIN_FACE)
        .collect();
    let (w, h) = (img.width as f32, img.height as f32);
    let mut out = Vec::new();
    let vecs = emb.embed_all(img, &faces)?;
    for (f, v) in faces.into_iter().zip(vecs) {
        let Some(mut v) = v else {
            continue;
        };
        if v.iter().any(|x| !x.is_finite()) {
            return Err(Error::Feed("the embedding is not finite".into()));
        }
        normalize(&mut v);
        out.push(FaceBox {
            x: (f.rect[0] / w).clamp(0.0, 1.0),
            y: (f.rect[1] / h).clamp(0.0, 1.0),
            w: (f.rect[2] / w).clamp(0.0, 1.0),
            h: (f.rect[3] / h).clamp(0.0, 1.0),
            score: f.score,
            roll: f.roll(),
            vec: v
                .iter()
                .map(|&x| half::f16::from_f32(x).to_bits())
                .collect(),
        });
    }
    Ok(Outputs::Faces(out))
}

/// Closed eyes, blur and exposure of a photo (Culling).
fn culling(p: &mut Pipeline, img: Image) -> Result<Outputs, Error> {
    let Some(lm) = p.lm.as_mut() else {
        return Err(Error::Weights("Culling needs the Face Landmarker".into()));
    };
    let small = shrink(img, 512);
    let sharpness = quality::sharpness(small.view());
    let e = quality::exposure(small.view());
    let mut faces = lm.detector.detect(img)?;
    faces.retain(|f| f.rect[2].min(f.rect[3]) >= MIN_FACE);
    faces.sort_by(|a, b| (b.rect[2] * b.rect[3]).total_cmp(&(a.rect[2] * a.rect[3])));
    faces.truncate(MAX_EYES);
    let (w, h) = (img.width as f32, img.height as f32);
    let mut seen = Vec::new();
    let mut eye_focus = None;
    for f in &faces {
        let eyes = lm.eyes(img, f)?;
        if eyes.presence < 0.5 {
            continue;
        }
        if eye_focus.is_none() {
            eye_focus = Some(face_sharpness(img, f));
        }
        seen.push(EyeFace {
            x: f.rect[0] / w,
            y: f.rect[1] / h,
            w: f.rect[2] / w,
            h: f.rect[3] / h,
            closed: eyes.closed(),
        });
    }
    let eyes_open = seen.iter().map(|f| 1.0 - f.closed).min_by(f32::total_cmp);
    Ok(Outputs::Scores(Scores {
        sharpness: Some(sharpness),
        eyes_open,
        eye_focus,
        expression: None,
        exposure: Some(e.mean),
        clipped: Some(e.clipped + e.crushed),
        faces: seen,
    }))
}

/// Sharpness of a face's box resampled to 128×128.
fn face_sharpness(img: Image, f: &Face) -> f32 {
    let s = f.rect[2].max(f.rect[3]) / 128.0;
    let m = Affine([s, 0.0, f.rect[0], 0.0, s, f.rect[1]]);
    let t = warp(img, &m, 128, 128, 1.0, 0.0).to_f32();
    let y: Vec<f32> = (0..128 * 128)
        .map(|i| 0.299 * t[i] + 0.587 * t[128 * 128 + i] + 0.114 * t[2 * 128 * 128 + i])
        .collect();
    quality::sharpness_of(&y, 128, 128)
}

impl Runtime for FaceRuntime {
    fn feature(&self) -> Feature {
        self.feature
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn input_side(&self) -> u32 {
        SIDE
    }

    fn engines(&self) -> &[Engine] {
        &self.engines
    }

    fn run(&self, engine: Engine, thumb: &Thumb) -> Result<Outputs, RunError> {
        let img = Image::new(thumb.width as usize, thumb.height as usize, thumb.rgba)
            .map_err(|e| RunError::Failed(e.to_string()))?;
        let mut p = match engine {
            Engine::Cpu => self.cpu.lock(),
            Engine::Gpu => match &self.gpu {
                Some(g) => g.lock(),
                None => return Err(RunError::Unsupported),
            },
            _ => return Err(RunError::Unsupported),
        };
        let out = match self.feature {
            Feature::People => people(&mut p, img),
            Feature::Culling => culling(&mut p, img),
            _ => return Err(RunError::Unsupported),
        };
        out.map_err(RunError::from)
    }
}

/// Groups the faces `model` found into people, at `threshold`:
///
/// - faces the user placed, and those of named people, stay where they are;
/// - the other faces are clustered; a cluster close to a named person (its mean against
///   theirs) joins them, else two or more faces make an unnamed person (keeping the id most
///   of them had, so groups don't renumber), a lone face belongs to nobody.
///
/// Returns how many faces were grouped.
pub fn regroup(conn: &mut Connection, model: &str, threshold: f32) -> rusqlite::Result<usize> {
    let faces: Vec<lib::FaceRow> = lib::all_faces(conn)?
        .into_iter()
        .filter(|f| f.model == model)
        .collect();
    let named: std::collections::HashSet<i64> = lib::people(conn)?
        .into_iter()
        .filter(|p| p.name.is_some())
        .map(|p| p.id)
        .collect();
    let f32v = |f: &lib::FaceRow| -> Vec<f32> {
        let mut v: Vec<f32> = f
            .vec
            .iter()
            .map(|&b| half::f16::from_bits(b).to_f32())
            .collect();
        normalize(&mut v);
        v
    };
    // Named people's mean embedding.
    let mut means: std::collections::BTreeMap<i64, Vec<f32>> = std::collections::BTreeMap::new();
    let mut free = Vec::new();
    for f in &faces {
        let fixed = f.locked || f.person.is_some_and(|p| named.contains(&p));
        if !fixed {
            free.push(f);
            continue;
        }
        if let Some(p) = f.person.filter(|p| named.contains(p)) {
            let v = f32v(f);
            let m = means.entry(p).or_insert_with(|| vec![0.0; v.len()]);
            if m.len() == v.len() {
                m.iter_mut().zip(&v).for_each(|(a, b)| *a += b);
            }
        }
    }
    for m in means.values_mut() {
        normalize(m);
    }
    let vecs: Vec<Vec<f32>> = free.iter().map(|f| f32v(f)).collect();
    let labels = cluster::agglomerate(&vecs, threshold);
    let groups = labels.iter().max().map_or(0, |m| m + 1);
    let mut g = Grouping::default();
    for c in 0..groups {
        let members: Vec<usize> = (0..free.len()).filter(|&i| labels[i] == c).collect();
        let mut mean = vec![0f32; vecs.first().map_or(0, Vec::len)];
        for &i in &members {
            mean.iter_mut().zip(&vecs[i]).for_each(|(a, b)| *a += b);
        }
        normalize(&mut mean);
        let best = means
            .iter()
            .filter(|(_, m)| m.len() == mean.len())
            .map(|(&p, m)| (p, cosine(m, &mean)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((p, s)) = best
            && s >= threshold
        {
            g.to_person.extend(members.iter().map(|&i| (free[i].id, p)));
            continue;
        }
        if members.len() < 2 {
            g.to_nobody.extend(members.iter().map(|&i| free[i].id));
            continue;
        }
        // Keep the unnamed person most of them had.
        let mut votes: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
        for &i in &members {
            if let Some(p) = free[i].person {
                *votes.entry(p).or_default() += 1;
            }
        }
        let keep = votes
            .into_iter()
            .filter(|(p, _)| !named.contains(p))
            .max_by_key(|&(p, n)| (n, -p))
            .map(|(p, _)| p);
        match keep {
            Some(p) => g.to_person.extend(members.iter().map(|&i| (free[i].id, p))),
            None => g.to_new.extend(members.iter().map(|&i| (free[i].id, c))),
        }
    }
    // One unnamed person can't be kept by two clusters: the second gets a new one.
    let mut owner: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    let cluster_of: std::collections::HashMap<i64, usize> =
        free.iter().zip(&labels).map(|(f, &c)| (f.id, c)).collect();
    let mut moved = Vec::new();
    g.to_person.retain(|&(face, p)| {
        if named.contains(&p) {
            return true;
        }
        let c = cluster_of[&face];
        match owner.get(&p) {
            None => {
                owner.insert(p, c);
                true
            }
            Some(&o) if o == c => true,
            Some(_) => {
                moved.push((face, c));
                false
            }
        }
    });
    g.to_new.extend(moved);
    lib::apply_grouping(conn, &g)?;
    Ok(free.len())
}

/// The model key faces were found with (`<size>/<keys>`): what [`regroup`] reads.
pub fn people_key(pack: &Pack) -> Option<String> {
    pack.index_models(Feature::People).0.map(|m| m.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_are_cosines() {
        for k in [Kind::Sface, Kind::AuraFace] {
            assert!((0.2..0.6).contains(&threshold(k)));
        }
    }
}
