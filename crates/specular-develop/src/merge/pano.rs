//! Panorama stitching on the merge pipeline: frames of one camera turning about its centre
//! become one scene-referred raw, still a colour mosaic, saved as a floating point DNG
//! (`-Pano.dng`); JPEG/HEIC/TIFF frames become an RGB picture.
//!
//! 1. **Normalise** each frame as a bracket merge does (black 0, white 1), with lens
//!    vignetting corrected from the frame's own lens tables when it has them.
//! 2. **Features** ([`super::features`]) on a log-luma proxy of 1–2 MP, with the lens
//!    distortion taken out, so the geometry below sees a pinhole camera.
//! 3. **Pairs**: every pair compared on its 300 strongest corners; each frame's six best
//!    candidates (and its neighbours in capture order) matched fully, a homography found by
//!    RANSAC ([`super::ransac`]) and kept when it passes Brown & Lowe's overlap test.
//! 4. **Cameras**: bundle adjustment of rotations and focal lengths ([`super::bundle`]), on
//!    the largest connected group of frames; the others are named as left out. Without
//!    lens tables one radial term `k1` is estimated too.
//! 5. **Projection** ([`super::project`]): rectilinear up to 110° across, cylindrical beyond
//!    (or as asked), at the frames' median focal length; optionally cropped to the largest
//!    rectangle the frames fill.
//! 6. **Gains**: one scalar per frame in linear raw (Brown & Lowe's gain compensation: least
//!    squares over the mean brightness of each overlap), on top of the exposure ratios.
//! 7. **Seams** ([`super::seam`]) on the output proxy, around moving things.
//! 8. **Render**: in strips of output rows, each CFA lattice blended with multiband
//!    ([`super::blend`]) from the frames resampled colour by colour ([`super::warp`]), the
//!    kernel widened where the projection shrinks a frame.

use std::path::PathBuf;

use rayon::prelude::*;
use specular_core::CancelToken;
use specular_raw::sensor::{LensCorrection, SensorMeta};

use super::align::Proxy;
use super::blend::{self, Img, Part};
use super::bundle::{self, BundleOptions, Camera, FrameInfo, PairMatches};
use super::features::{self, DetectOptions, Features, Grey};
use super::project::{self, OutputFrame, Projection};
use super::ransac::{self, Pair};
use super::seam::{self, Layer, NONE};
use super::warp;
use super::{Error, Frame, GhostMask, Merged, Mosaic, Plane, Shot};
use crate::ops::geometry::lens::MakerModel;

/// The most frames one panorama takes.
pub const MAX_FRAMES: usize = 60;
/// The long side of the output proxy (gains, seams, crop).
const PROXY_SIDE: u32 = 1600;
/// Feature proxies are halved down to this many pixels at most.
const FEATURE_PIXELS: usize = 1_600_000;
/// Candidate partners per frame for full matching.
const CANDIDATES: usize = 6;
/// Inlier matches kept per pair for the bundle adjustment.
const BUNDLE_MATCHES: usize = 250;

/// How to stitch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanoOptions {
    /// `None`: chosen by the field of view.
    pub projection: Option<Projection>,
    /// Crop to the largest rectangle the frames fill (else the empty corners stay, with
    /// [`Panorama::coverage`] saying where).
    pub crop: bool,
    /// Multiband bands.
    pub bands: usize,
    /// Leave seams to the distance transform (no graph cut).
    pub plain_seams: bool,
}

impl Default for PanoOptions {
    fn default() -> Self {
        PanoOptions {
            projection: None,
            crop: true,
            bands: 6,
            plain_seams: false,
        }
    }
}

/// A stitched panorama.
#[derive(Clone, Debug)]
pub struct Panorama {
    /// The panorama as a merge (mosaic samples over the output frame; for RGB frames
    /// three channels per sample): `exposures` holds each frame's gain.
    pub merged: Merged,
    pub frame: OutputFrame,
    /// Which samples any frame covers (row-major over the output; all when cropped).
    pub coverage: Vec<bool>,
    /// Each frame's camera (`None`: left out).
    pub cameras: Vec<Option<Camera>>,
    /// The frames left out for not overlapping the others.
    pub left_out: Vec<PathBuf>,
    /// Reprojection error of the bundle adjustment, pixels.
    pub rms: f64,
}

/// A frame's lens: distortion back-map and vignetting from its own tables.
struct Lens {
    model: MakerModel,
    cx: f64,
    cy: f64,
    /// Half the diagonal (the tables' unit radius).
    r: f64,
}

impl Lens {
    fn of(lens: &LensCorrection, w: u32, h: u32) -> Option<Lens> {
        let (fw, fh) = (f64::from(w), f64::from(h));
        let model = MakerModel::new(lens, fw, fh);
        (model.has_distortion() || model.has_vignetting()).then(|| Lens {
            model,
            cx: (fw - 1.0) / 2.0,
            cy: (fh - 1.0) / 2.0,
            r: fw.hypot(fh) / 2.0,
        })
    }

    /// The stored position a corrected position reads, for colour plane `plane`.
    #[inline]
    fn back(&self, x: f64, y: f64, plane: usize) -> (f64, f64) {
        if !self.model.has_distortion() && !self.model.has_ca() {
            return (x, y);
        }
        let (dx, dy) = (x - self.cx, y - self.cy);
        let rho = dx.hypot(dy) / self.r;
        let f = self.model.factor(plane.min(2), rho, true, true);
        (self.cx + f * dx, self.cy + f * dy)
    }

    fn gain(&self, x: f64, y: f64) -> f64 {
        if !self.model.has_vignetting() {
            return 1.0;
        }
        self.model
            .vignette_gain((x - self.cx).hypot(y - self.cy) / self.r)
    }
}

/// A frame ready to stitch.
struct Source {
    plane: Plane,
    lens: Option<Lens>,
    shot: Shot,
    /// Noise of a normalised sample: variance `a·v + b`.
    noise: (f32, f32),
    /// Exposure relative to the reference (metadata; 1 when unknown).
    exposure: f64,
}

impl Source {
    fn w(&self) -> u32 {
        self.plane.w
    }
    fn h(&self) -> u32 {
        self.plane.h
    }
}

/// A log2-luma proxy of whole CFA blocks with saturation kept (at the clip level) and a
/// floor for black: (proxy, block side in samples).
fn luma_proxy(p: &Plane, block: u32) -> Proxy {
    let b = block.max(1) as usize;
    let (pw, ph, cpp) = (p.w as usize, p.h as usize, p.cpp as usize);
    let (w, h) = (pw / b, ph / b);
    let mut v = vec![f32::NAN; w * h];
    v.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0f32;
            let mut n = 0;
            for dy in 0..b {
                for &q in &p.v[((y * b + dy) * pw + x * b) * cpp..][..b * cpp] {
                    if q.is_nan() {
                        continue;
                    }
                    s += if q.is_finite() { q } else { p.sat };
                    n += 1;
                }
            }
            if n > 0 {
                *o = (s / n as f32).max(1e-5).log2();
            }
        }
    });
    Proxy { w, h, v }
}

/// Proxy pixel of level `k` (after `k` halvings of a `block` proxy) → sample position.
fn proxy_scale(block: u32, k: u32) -> (f64, f64) {
    let s = f64::from(block.max(1)) * f64::from(1u32 << k);
    (s, (s - 1.0) / 2.0)
}

/// The feature proxy of a source in corrected (undistorted) geometry, and its scale.
fn feature_proxy(s: &Source, block: u32) -> (Proxy, (f64, f64)) {
    let mut p = luma_proxy(&s.plane, block);
    let mut k = 0;
    while p.w * p.h > FEATURE_PIXELS && p.w.min(p.h) > 64 {
        p = p.half();
        k += 1;
    }
    let (sc, off) = proxy_scale(block, k);
    if let Some(l) = &s.lens {
        let src = p.clone();
        p.v.par_chunks_mut(p.w).enumerate().for_each(|(y, row)| {
            for (x, o) in row.iter_mut().enumerate() {
                let (cx, cy) = (x as f64 * sc + off, y as f64 * sc + off);
                let (sx, sy) = l.back(cx, cy, 1);
                *o = src.sample((sx - off) / sc, (sy - off) / sc);
            }
        });
    }
    (p, (sc, off))
}

/// The pairs worth matching fully: each frame's best candidates by a quick count over its
/// strongest corners, and its neighbours in capture order.
fn candidates(feats: &[Features]) -> Vec<(usize, usize)> {
    let n = feats.len();
    let strongest: Vec<Features> = feats
        .iter()
        .map(|f| {
            let mut idx: Vec<usize> = (0..f.points.len()).collect();
            idx.sort_by(|&a, &b| f.points[b].score.total_cmp(&f.points[a].score));
            idx.truncate(300);
            Features {
                points: idx.iter().map(|&i| f.points[i]).collect(),
                descriptors: idx.iter().map(|&i| f.descriptors[i]).collect(),
            }
        })
        .collect();
    let all: Vec<(usize, usize)> = (0..n)
        .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
        .collect();
    let counts: Vec<usize> = all
        .par_iter()
        .map(|&(a, b)| features::match_features(&strongest[a], &strongest[b], 0.8, 64).len())
        .collect();
    let mut keep = vec![false; all.len()];
    for f in 0..n {
        let mut mine: Vec<(usize, usize)> = all
            .iter()
            .enumerate()
            .filter(|(_, p)| p.0 == f || p.1 == f)
            .map(|(k, _)| (k, counts[k]))
            .collect();
        mine.sort_by_key(|m| std::cmp::Reverse(m.1));
        for &(k, c) in mine.iter().take(CANDIDATES) {
            if c >= 6 {
                keep[k] = true;
            }
        }
    }
    all.iter()
        .zip(keep)
        .filter(|(p, k)| *k || p.1 == p.0 + 1)
        .map(|(p, _)| *p)
        .collect()
}

/// Every `n`-th of `v` so that at most `max` remain.
fn thin<T: Clone>(v: &[T], max: usize) -> Vec<T> {
    if v.len() <= max {
        return v.to_vec();
    }
    let step = v.len() as f64 / max as f64;
    (0..max)
        .map(|i| v[(i as f64 * step) as usize].clone())
        .collect()
}

/// A match refined to a fraction of a pixel: B's position found by Gauss–Newton on a
/// translation of a 15×15 patch of A carried through `h` (A → B, samples), on the
/// zero-mean log luma (so exposure differences do not pull).
fn refine(
    pa: &(Proxy, (f64, f64)),
    pb: &(Proxy, (f64, f64)),
    h: &super::Homography,
    p: (f64, f64),
) -> Option<(f64, f64)> {
    let ((a, (sa, oa)), (b, (sb, ob))) = (pa, pb);
    let (ax, ay) = ((p.0 - oa) / sa, (p.1 - oa) / sa);
    const R: i32 = 7;
    let mut ref_v = Vec::with_capacity(((2 * R + 1) * (2 * R + 1)) as usize);
    let mut at = Vec::with_capacity(ref_v.capacity());
    for dy in -R..=R {
        for dx in -R..=R {
            let v = a.sample(ax + f64::from(dx), ay + f64::from(dy));
            if !v.is_finite() {
                return None;
            }
            ref_v.push(f64::from(v));
            let s = (
                (ax + f64::from(dx)) * sa + oa,
                (ay + f64::from(dy)) * sa + oa,
            );
            let q = h.apply(s.0, s.1);
            at.push(((q.0 - ob) / sb, (q.1 - ob) / sb));
        }
    }
    let ma = ref_v.iter().sum::<f64>() / ref_v.len() as f64;
    let (mut tx, mut ty) = (0.0f64, 0.0f64);
    for _ in 0..8 {
        let vals: Option<Vec<(f64, f64, f64)>> = at
            .iter()
            .map(|&(x, y)| {
                let (x, y) = (x + tx, y + ty);
                let v = b.sample(x, y);
                let gx = 0.5 * (b.sample(x + 1.0, y) - b.sample(x - 1.0, y));
                let gy = 0.5 * (b.sample(x, y + 1.0) - b.sample(x, y - 1.0));
                (v.is_finite() && gx.is_finite() && gy.is_finite())
                    .then(|| (f64::from(v), f64::from(gx), f64::from(gy)))
            })
            .collect();
        let vals = vals?;
        let mb = vals.iter().map(|v| v.0).sum::<f64>() / vals.len() as f64;
        let (mut hxx, mut hxy, mut hyy, mut bx, mut by) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (k, &(v, gx, gy)) in vals.iter().enumerate() {
            let r = (v - mb) - (ref_v[k] - ma);
            hxx += gx * gx;
            hxy += gx * gy;
            hyy += gy * gy;
            bx += gx * r;
            by += gy * r;
        }
        let det = hxx * hyy - hxy * hxy;
        if det.abs() < 1e-12 {
            return None;
        }
        let (dx, dy) = ((hyy * bx - hxy * by) / det, (hxx * by - hxy * bx) / det);
        tx -= dx;
        ty -= dy;
        if tx.hypot(ty) > 2.0 {
            return None;
        }
        if dx.hypot(dy) < 1e-4 {
            break;
        }
    }
    let q = h.apply(p.0, p.1);
    Some((q.0 + tx * sb, q.1 + ty * sb))
}

/// The matched pairs that pass RANSAC and the overlap test, in corrected sample units.
fn pair_geometry(
    sources: &[Source],
    feats: &[Features],
    proxies: &[(Proxy, (f64, f64))],
    focal: &[Option<f64>],
    cancel: &CancelToken,
) -> Vec<PairMatches> {
    let scales: Vec<(f64, f64)> = proxies.iter().map(|p| p.1).collect();
    let pairs = candidates(feats);
    pairs
        .par_iter()
        .filter_map(|&(a, b)| {
            if cancel.is_cancelled() {
                return None;
            }
            let m = features::match_features(&feats[a], &feats[b], 0.8, 64);
            if m.len() < 12 {
                return None;
            }
            let (sa, oa) = scales[a];
            let (sb, ob) = scales[b];
            let pts: Vec<Pair> = m
                .iter()
                .map(|&(i, j, _)| {
                    let (p, q) = (feats[a].points[i], feats[b].points[j]);
                    (
                        (f64::from(p.x) * sa + oa, f64::from(p.y) * sa + oa),
                        (f64::from(q.x) * sb + ob, f64::from(q.y) * sb + ob),
                    )
                })
                .collect();
            // Two proxy pixels.
            let threshold = 2.0 * sa.max(sb);
            let seed = (a * 131 + b) as u64;
            // With both focal lengths known, the two-point rotation model (fewer outliers
            // pass by chance); else the four-point homography.
            let intr = |i: usize| {
                focal.get(i).copied().flatten().map(|f| ransac::Intrinsics {
                    f,
                    cx: f64::from(sources[i].w()) / 2.0,
                    cy: f64::from(sources[i].h()) / 2.0,
                })
            };
            let fit = match (intr(a), intr(b)) {
                (Some(ia), Some(ib)) => ransac::ransac_rotation(&pts, &ia, &ib, threshold, seed)
                    .map(|f| ransac::Fit {
                        model: ransac::rotation_homography(&f.model, &ia, &ib),
                        inliers: f.inliers,
                    })
                    .or_else(|| ransac::ransac_homography(&pts, threshold, seed)),
                _ => ransac::ransac_homography(&pts, threshold, seed),
            }?;
            let size = |s: &Source| (s.w(), s.h());
            let inside = ransac::in_overlap(&pts, &fit.model, size(&sources[a]), size(&sources[b]));
            if !ransac::overlaps(fit.inliers.len(), inside) {
                return None;
            }
            // Each inlier to a fraction of a pixel (those that do not converge go).
            let inl: Vec<Pair> = fit
                .inliers
                .iter()
                .filter_map(|&k| {
                    let (pa, _) = pts[k];
                    refine(&proxies[a], &proxies[b], &fit.model, pa).map(|q| (pa, q))
                })
                .collect();
            if inl.len() < 8 {
                return None;
            }
            Some(PairMatches {
                a,
                b,
                pairs: thin(&inl, BUNDLE_MATCHES),
                h: fit.model,
            })
        })
        .collect()
}

/// A frame's source position (stored samples) for output sample (x, y) of colour plane
/// `plane`, through camera `c` and the lens.
#[inline]
fn source_of(
    out: &OutputFrame,
    c: &Camera,
    lens: Option<&Lens>,
    x: f64,
    y: f64,
    plane: usize,
) -> Option<(f64, f64)> {
    let (px, py) = c.project(out.dir(x, y))?;
    Some(match lens {
        Some(l) => l.back(px, py, plane),
        None => (px, py),
    })
}

/// A frame's footprint in an output frame: its bounding box (x0, y0, x1, y1), clipped.
fn footprint(out: &OutputFrame, c: &Camera, w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let (fw, fh) = (f64::from(w), f64::from(h));
    let mut b = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    let n = 48;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        for (x, y) in [(t * fw, 0.0), (fw, t * fh), (t * fw, fh), (0.0, t * fh)] {
            if let Some((u, v)) = out.pos(c.unproject(x, y)) {
                b = (b.0.min(u), b.1.min(v), b.2.max(u), b.3.max(v));
            }
        }
    }
    if !b.0.is_finite() {
        return None;
    }
    let cl = |v: f64, m: u32| v.clamp(0.0, f64::from(m)) as u32;
    let r = (
        cl(b.0.floor() - 2.0, out.w),
        cl(b.1.floor() - 2.0, out.h),
        cl(b.2.ceil() + 2.0, out.w),
        cl(b.3.ceil() + 2.0, out.h),
    );
    (r.2 > r.0 && r.3 > r.1).then_some(r)
}

/// Each frame's log-luma picture on the proxy frame `pf`, with its noise.
fn layers(
    sources: &[Source],
    cams: &[Camera],
    proxies: &[(Proxy, u32)],
    pf: &OutputFrame,
) -> Vec<Layer> {
    let (w, h) = (pf.w as usize, pf.h as usize);
    sources
        .par_iter()
        .zip(cams)
        .zip(proxies)
        .map(|((s, c), (p, block))| {
            let (sc, off) = proxy_scale(*block, 0);
            let shift = s.exposure.recip().log2() as f32;
            let mut v = vec![f32::NAN; w * h];
            let mut sigma = vec![0f32; w * h];
            let per = (f64::from(*block) * f64::from(*block)) as f32;
            for i in 0..w * h {
                let (x, y) = ((i % w) as f64, (i / w) as f64);
                let Some((sx, sy)) = source_of(pf, c, s.lens.as_ref(), x, y, 1) else {
                    continue;
                };
                if sx < 0.0
                    || sy < 0.0
                    || sx > f64::from(s.w()) - 1.0
                    || sy > f64::from(s.h()) - 1.0
                {
                    continue;
                }
                let q = p.sample((sx - off) / sc, (sy - off) / sc);
                if q.is_finite() {
                    let lin = q.exp2();
                    let var = (s.noise.0 * lin + s.noise.1).max(1e-12) / per;
                    v[i] = q + shift;
                    sigma[i] = var.sqrt() / (lin * std::f32::consts::LN_2);
                }
            }
            Layer { v, sigma }
        })
        .collect()
}

/// Brown & Lowe's gain compensation over the layers' overlaps (σ_N 0.1, σ_g 1 in
/// relative units): one gain per frame.
fn gains(layers: &[Layer]) -> Vec<f64> {
    let n = layers.len();
    let size = layers.first().map_or(0, |l| l.v.len());
    // I_ij: mean linear brightness of frame i where it overlaps j; N_ij the count.
    let mut sum = vec![0.0f64; n * n];
    let mut cnt = vec![0.0f64; n * n];
    for p in 0..size {
        let here: Vec<usize> = (0..n).filter(|&i| layers[i].v[p].is_finite()).collect();
        for &i in &here {
            let v = f64::from(layers[i].v[p]).exp2();
            for &j in &here {
                if i != j {
                    sum[i * n + j] += v;
                    cnt[i * n + j] += 1.0;
                }
            }
        }
    }
    let mean: Vec<f64> = (0..n * n)
        .map(|k| if cnt[k] > 0.0 { sum[k] / cnt[k] } else { 0.0 })
        .collect();
    let scale = {
        let (s, c) = (0..n * n)
            .filter(|&k| cnt[k] > 0.0)
            .fold((0.0, 0.0), |a, k| (a.0 + mean[k], a.1 + 1.0));
        if c > 0.0 { s / c } else { 1.0 }
    };
    // A weak prior (σ_g 1 rather than the paper's 0.1): it only fixes the overall level, so
    // the frames end up at one level in linear raw instead of halfway there (the blend
    // hides a residual in a display picture, not in radiance).
    let (sn, sg) = (0.1f64, 1.0f64);
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n];
    for i in 0..n {
        a[i * n + i] += 1e-9;
        for j in 0..n {
            let c = cnt[i * n + j];
            if i == j || c == 0.0 {
                continue;
            }
            let (iij, iji) = (mean[i * n + j] / scale, mean[j * n + i] / scale);
            a[i * n + i] += c * (iij * iij / (sn * sn) + 1.0 / (sg * sg));
            a[i * n + j] -= c * iij * iji / (sn * sn);
            b[i] += c / (sg * sg);
        }
        if b[i] == 0.0 {
            // Overlaps nothing: keep its brightness.
            a[i * n + i] = 1.0;
            b[i] = 1.0;
        }
    }
    super::homography::cholesky_solve(&a, &b, n)
        .filter(|g| g.iter().all(|v| v.is_finite() && *v > 0.0))
        .unwrap_or_else(|| vec![1.0; n])
}

/// How the output mosaic splits into images blended on their own.
#[derive(Clone, Copy, Debug)]
struct Channel {
    /// Lattice offset and pitch in output samples.
    ox: u32,
    oy: u32,
    pitch: u32,
    /// Plane channel (linear data) or colour (patterns: samples of other colours are
    /// left out of this channel).
    c: usize,
    colour_only: Option<u8>,
}

fn channels(m: &Mosaic, cpp: u32) -> Vec<Channel> {
    match m {
        Mosaic::Bayer(_) => (0..4)
            .map(|k| Channel {
                ox: k & 1,
                oy: k >> 1,
                pitch: 2,
                c: 0,
                colour_only: None,
            })
            .collect(),
        Mosaic::Pattern { colors, .. } => {
            let mut cs: Vec<u8> = colors.clone();
            cs.sort_unstable();
            cs.dedup();
            cs.into_iter()
                .map(|c| Channel {
                    ox: 0,
                    oy: 0,
                    pitch: 1,
                    c: 0,
                    colour_only: Some(c),
                })
                .collect()
        }
        Mosaic::Linear | Mosaic::Mono => (0..cpp as usize)
            .map(|c| Channel {
                ox: 0,
                oy: 0,
                pitch: 1,
                c,
                colour_only: None,
            })
            .collect(),
    }
}

/// The value a frame gives output sample (x, y) of channel `ch` (NaN: none), in the
/// reference's units, with the local kernel widening `s`.
#[allow(clippy::too_many_arguments)]
#[inline]
fn value(
    src: &Source,
    mosaic: &Mosaic,
    out: &OutputFrame,
    cam: &Camera,
    ch: &Channel,
    colour: u8,
    x: u32,
    y: u32,
    scale: f32,
    widen: f64,
) -> f32 {
    let plane = match mosaic {
        Mosaic::Linear => ch.c,
        _ => usize::from(colour),
    };
    let Some(q) = source_of(
        out,
        cam,
        src.lens.as_ref(),
        f64::from(x),
        f64::from(y),
        plane,
    ) else {
        return f32::NAN;
    };
    let v = warp::sample(&src.plane, mosaic, (x & 1, y & 1), colour, q, ch.c, widen);
    if v.is_nan() {
        f32::NAN
    } else if v.is_infinite() {
        src.plane.sat * scale
    } else {
        v * scale
    }
}

/// The kernel widening of each 8×8 cell of an area: source samples per output sample.
fn shrink_grid(
    out: &OutputFrame,
    cam: &Camera,
    x0: u32,
    y0: u32,
    w: u32,
    h: u32,
) -> (u32, Vec<f64>) {
    let cell = 8u32;
    let (gw, gh) = (w.div_ceil(cell), h.div_ceil(cell));
    let g = (0..gw * gh)
        .map(|k| {
            let (x, y) = (
                f64::from(x0 + (k % gw) * cell + cell / 2),
                f64::from(y0 + (k / gw) * cell + cell / 2),
            );
            let p = |x: f64, y: f64| cam.project(out.dir(x, y));
            match (p(x, y), p(x + 1.0, y), p(x, y + 1.0)) {
                (Some(a), Some(b), Some(c)) => {
                    let det = ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs();
                    let s = det.sqrt();
                    if s > 1.05 { s.min(16.0) } else { 1.0 }
                }
                _ => 1.0,
            }
        })
        .collect();
    (gw, g)
}

/// Renders the panorama: per channel, strips of rows, multiband blended.
#[allow(clippy::too_many_arguments)]
fn render(
    sources: &[Source],
    cams: &[Camera],
    scales: &[f32],
    mosaic: &Mosaic,
    cpp: u32,
    out: &OutputFrame,
    labels: &[u16],
    pf: &OutputFrame,
    bands: usize,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<(Vec<f32>, Vec<bool>), Error> {
    let (w, h) = (out.w, out.h);
    let mut radiance = vec![0f32; w as usize * h as usize * cpp as usize];
    let mut covered = vec![false; w as usize * h as usize];
    let chans = channels(mosaic, cpp);
    let feet: Vec<Option<(u32, u32, u32, u32)>> = cams
        .iter()
        .zip(sources)
        .map(|(c, s)| footprint(out, c, s.w(), s.h()))
        .collect();
    // Output sample → proxy label.
    let k = pf.scale / out.scale;
    let label_at = |x: u32, y: u32| -> u16 {
        let px = ((f64::from(x) + out.x0) * k - pf.x0).round();
        let py = ((f64::from(y) + out.y0) * k - pf.y0).round();
        if px < 0.0 || py < 0.0 || px >= f64::from(pf.w) || py >= f64::from(pf.h) {
            return NONE;
        }
        labels[py as usize * pf.w as usize + px as usize]
    };
    let total = chans.len() as f32;
    for (ci, ch) in chans.iter().enumerate() {
        // The channel's image: lattice positions (a, b) ↔ samples (ox + pitch·a, …).
        let cw = (w.saturating_sub(ch.ox)).div_ceil(ch.pitch) as usize;
        let chh = (h.saturating_sub(ch.oy)).div_ceil(ch.pitch) as usize;
        if cw == 0 || chh == 0 {
            continue;
        }
        let levels = blend::levels_for(cw, chh, bands.max(1));
        let align = 1usize << (levels - 1);
        let margin = (4usize << levels).max(16);
        let core = (2 * margin).max(512).div_ceil(align) * align;
        let pitch = ch.pitch as usize;
        let mut ys = 0usize;
        while ys < chh {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let ya = ys.saturating_sub(margin) / align * align;
            let yb = (ys + core + margin).min(chh);
            let sh = yb - ya;
            let mut parts = Vec::new();
            for (i, s) in sources.iter().enumerate() {
                let Some((fx0, fy0, fx1, fy1)) = feet[i] else {
                    continue;
                };
                // The footprint in channel units, with the margin.
                let to_ch = |v: u32, o: u32| (v.saturating_sub(o) / ch.pitch) as usize;
                let (cx0, cx1) = (to_ch(fx0, ch.ox), to_ch(fx1, ch.ox) + 1);
                let (cy0, cy1) = (to_ch(fy0, ch.oy), to_ch(fy1, ch.oy) + 1);
                if cy1 + margin <= ya || cy0 >= yb + margin {
                    continue;
                }
                let x0 = cx0.saturating_sub(margin) / align * align;
                let x1 = (cx1 + margin).min(cw);
                let y0 = cy0.saturating_sub(margin).max(ya);
                let y0 = ya + (y0 - ya) / align * align;
                let y1 = (cy1 + margin).min(yb);
                if x1 <= x0 || y1 <= y0 {
                    continue;
                }
                let (pw, ph) = (x1 - x0, y1 - y0);
                let mut img = Img::new(pw, ph, f32::NAN);
                let mut wt = Img::new(pw, ph, 0.0);
                let (gx0, gy0) = (ch.ox + (x0 * pitch) as u32, ch.oy + (y0 * pitch) as u32);
                let (gw, grid) = shrink_grid(
                    out,
                    &cams[i],
                    gx0,
                    gy0,
                    (pw * pitch) as u32,
                    (ph * pitch) as u32,
                );
                img.v
                    .par_chunks_mut(pw)
                    .zip(wt.v.par_chunks_mut(pw))
                    .enumerate()
                    .for_each(|(r, (row, wrow))| {
                        let y = ch.oy + ((y0 + r) * pitch) as u32;
                        for (c, (o, wo)) in row.iter_mut().zip(wrow.iter_mut()).enumerate() {
                            let x = ch.ox + ((x0 + c) * pitch) as u32;
                            let colour = mosaic.color(x, y);
                            if ch.colour_only.is_some_and(|k| k != colour) {
                                continue;
                            }
                            let cell = ((y - gy0) / 8 * gw + (x - gx0) / 8) as usize;
                            let widen = grid.get(cell).copied().unwrap_or(1.0);
                            let v =
                                value(s, mosaic, out, &cams[i], ch, colour, x, y, scales[i], widen);
                            if v.is_finite() {
                                *o = v;
                                *wo = if label_at(x, y) == i as u16 {
                                    1.0
                                } else {
                                    1e-4
                                };
                            }
                        }
                    });
                parts.push(Part {
                    x0,
                    y0: y0 - ya,
                    img,
                    weight: wt,
                });
            }
            let b = blend::blend(&parts, cw, sh, levels, &|| cancel.is_cancelled())
                .ok_or(Error::Cancelled)?;
            drop(parts);
            let y_end = (ys + core).min(chh);
            for yy in ys..y_end {
                let y = ch.oy as usize + yy * pitch;
                for a in 0..cw {
                    let x = ch.ox as usize + a * pitch;
                    let v = b.v[(yy - ya) * cw + a];
                    if let Some(k) = ch.colour_only
                        && mosaic.color(x as u32, y as u32) != k
                    {
                        continue;
                    }
                    if v.is_finite() {
                        radiance[(y * w as usize + x) * cpp as usize + ch.c] = v;
                        covered[y * w as usize + x] = true;
                    }
                }
            }
            ys = y_end;
            progress((ci as f32 + ys as f32 / chh as f32) / total);
        }
    }
    Ok((radiance, covered))
}

/// The reference: the frame looking nearest the panorama's centre.
fn reference_of(cams: &[Camera], out: &OutputFrame) -> usize {
    let centre = out.dir(f64::from(out.w) / 2.0, f64::from(out.h) / 2.0);
    let n = (centre[0] * centre[0] + centre[1] * centre[1] + centre[2] * centre[2]).sqrt();
    let c = centre.map(|v| v / n);
    (0..cams.len())
        .max_by(|&a, &b| {
            let z = |k: usize| {
                let r = &cams[k].r;
                r[6] * c[0] + r[7] * c[1] + r[8] * c[2]
            };
            z(a).total_cmp(&z(b))
        })
        .unwrap_or(0)
}

/// Bytes the stitch holds at its peak for `frames` frames of `samples` samples each.
pub fn memory_needed(frames: usize, samples: u64) -> u64 {
    // The normalised planes, and an output of about the frames' combined size with its
    // radiance, coverage and the strips' pyramids.
    samples * 4 * frames as u64 + samples * frames as u64 * (4 + 1)
}

/// The focal length in pixels a frame's Exif gives (35 mm equivalent, by the diagonal).
pub fn focal_px(bytes: &[u8], w: u32, h: u32) -> Option<f64> {
    let e = specular_convert::source_meta(bytes).exif?;
    let t = e.exif.iter().find(|x| x.tag == 0xA405)?;
    let f35 = f64::from(u16::from_le_bytes([*t.data.first()?, *t.data.get(1)?]));
    (f35 > 0.0).then(|| f35 / 43.2666 * f64::from(w).hypot(f64::from(h)))
}

/// Stitches raw frames (see the module documentation). `focal` gives each frame's focal
/// length in pixels when known (e.g. from [`focal_px`]). `progress` gets 0..=1.
pub fn stitch(
    frames: Vec<Frame>,
    focal: &[Option<f64>],
    o: &PanoOptions,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<Panorama, Error> {
    if frames.len() < 2 {
        return Err(Error::TooFew);
    }
    if frames.len() > MAX_FRAMES {
        return Err(Error::Cap(MAX_FRAMES));
    }
    let r0 = &frames[0].sensor;
    let same = |s: &specular_raw::SensorData| {
        s.width == r0.width
            && s.height == r0.height
            && s.cpp == r0.cpp
            && s.crop == r0.crop
            && s.meta.layout == r0.meta.layout
    };
    if !frames.iter().all(|f| same(&f.sensor)) {
        return Err(Error::NotOneCamera);
    }
    let samples = u64::from(r0.crop.w) * u64::from(r0.crop.h) * u64::from(r0.cpp);
    let _mem = specular_core::budget::reserve(memory_needed(frames.len(), samples))
        .map_err(Error::NeedsMemory)?;
    let mosaic = Mosaic::of(&r0.meta.layout, r0.crop.x, r0.crop.y);
    let metas: Vec<SensorMeta> = frames.iter().map(|f| f.sensor.meta.clone()).collect();
    let expo: Vec<Option<f64>> = frames.iter().map(|f| f.shot.exposure()).collect();
    let mid = expo
        .iter()
        .flatten()
        .copied()
        .fold(None, |a: Option<f64>, e| Some(a.map_or(e, |a| a.max(e))));
    let mut sources = Vec::with_capacity(frames.len());
    for (i, f) in frames.into_iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let rn = crate::ops::detail::RawNoise::of_sensor(&f.sensor);
        let mut plane = super::normalise(&f.sensor);
        let lens = f
            .sensor
            .meta
            .lens
            .as_deref()
            .and_then(|l| Lens::of(l, plane.w, plane.h));
        if let Some(l) = &lens {
            correct_vignetting(&mut plane, l);
        }
        sources.push(Source {
            plane,
            lens,
            shot: f.shot,
            noise: (
                (rn.a.iter().sum::<f64>() / 3.0) as f32,
                (rn.b.iter().sum::<f64>() / 3.0) as f32,
            ),
            exposure: match (expo[i], mid) {
                (Some(e), Some(m)) if e > 0.0 => e / m,
                _ => 1.0,
            },
        });
        progress(0.1 * (i + 1) as f32 / expo.len() as f32);
    }
    let k1 = sources.iter().all(|s| s.lens.is_none());
    stitch_sources(sources, mosaic, metas, focal, k1, o, progress, cancel)
}

/// Stitches linear RGB frames (non-raw: `Mosaic::Linear`, three channels).
pub(crate) fn stitch_linear(
    planes: Vec<(Plane, Shot)>,
    focal: &[Option<f64>],
    o: &PanoOptions,
    meta: SensorMeta,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<Panorama, Error> {
    if planes.len() < 2 {
        return Err(Error::TooFew);
    }
    if planes.len() > MAX_FRAMES {
        return Err(Error::Cap(MAX_FRAMES));
    }
    let samples: u64 = planes.iter().map(|p| p.0.v.len() as u64).max().unwrap_or(0);
    let _mem = specular_core::budget::reserve(memory_needed(planes.len(), samples))
        .map_err(Error::NeedsMemory)?;
    let (w0, h0) = (planes[0].0.w, planes[0].0.h);
    if !planes
        .iter()
        .all(|p| p.0.w == w0 && p.0.h == h0 && p.0.cpp == planes[0].0.cpp)
    {
        return Err(Error::NotOneCamera);
    }
    let n = planes.len();
    let sources = planes
        .into_iter()
        .map(|(plane, shot)| Source {
            plane,
            lens: None,
            shot,
            // 8-bit quantisation in linear terms, about.
            noise: (1e-4, 1e-6),
            exposure: 1.0,
        })
        .collect();
    stitch_sources(
        sources,
        Mosaic::Linear,
        vec![meta; n],
        focal,
        true,
        o,
        progress,
        cancel,
    )
}

/// Neutral sensor facts for linear RGB frames (no raw metadata).
fn rgb_meta() -> SensorMeta {
    use specular_raw::sensor::{BlackLevels, Layout};
    SensorMeta {
        layout: Layout::Linear,
        black: BlackLevels::simple([0.0; 4], 3),
        white: 1.0,
        clip_white: 1.0,
        linearization: None,
        opcodes: Default::default(),
        opcodes_unknown: Vec::new(),
        wb: [1.0, 1.0, 1.0],
        wb_source: specular_raw::WbSource::AsShot,
        rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        matrix_source: specular_raw::MatrixSource::Generic,
        dng: None,
        profile: None,
        orientation: 1,
        truncated: false,
        lens: None,
        default_scale: None,
    }
}

/// Stitches JPEG/HEIC/TIFF frames (linear Rec.2020 RGB, [`super::rgb`]): the panorama and
/// its picture, with alpha where no frame reaches when it is not cropped.
pub fn stitch_rgb(
    images: Vec<(super::rgb::RgbImage, Shot)>,
    focal: &[Option<f64>],
    o: &PanoOptions,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<(Panorama, super::rgb::RgbImage), Error> {
    let planes = images
        .into_iter()
        .map(|(img, shot)| (super::rgb::to_plane(&img), shot))
        .collect();
    let p = stitch_linear(planes, focal, o, rgb_meta(), progress, cancel)?;
    let m = &p.merged;
    let mut v = m.radiance.clone();
    if !o.crop {
        // Where nothing reaches is a hole (NaN), which becomes alpha 0.
        for (px, &c) in v.chunks_mut(3).zip(&p.coverage) {
            if !c {
                px.fill(f32::NAN);
            }
        }
    }
    let plane = Plane {
        w: m.width,
        h: m.height,
        cpp: 3,
        v,
        sat: f32::INFINITY,
    };
    let img = super::rgb::from_plane(&plane);
    Ok((p, img))
}

/// Lens vignetting taken out of a normalised plane (by the stored position's radius).
fn correct_vignetting(p: &mut Plane, l: &Lens) {
    if !l.model.has_vignetting() {
        return;
    }
    let (w, cpp) = (p.w as usize, p.cpp as usize);
    p.v.par_chunks_mut(w * cpp)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let g = l.gain(x as f64, y as f64) as f32;
                for v in &mut row[x * cpp..][..cpp] {
                    if v.is_finite() {
                        *v *= g;
                    }
                }
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn stitch_sources(
    sources: Vec<Source>,
    mosaic: Mosaic,
    metas: Vec<SensorMeta>,
    focal: &[Option<f64>],
    k1: bool,
    o: &PanoOptions,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<Panorama, Error> {
    let check = || {
        if cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    };
    let n = sources.len();
    let block = mosaic.block();
    let cpp = sources[0].plane.cpp;
    // 2. Features.
    let fp: Vec<(Proxy, (f64, f64))> = sources.iter().map(|s| feature_proxy(s, block)).collect();
    check()?;
    let feats: Vec<Features> = fp
        .iter()
        .map(|(p, _)| features::detect(&Grey::of(p), &DetectOptions::default()))
        .collect();
    progress(0.25);
    check()?;
    // 3. Pairs.
    let pm = pair_geometry(&sources, &feats, &fp, focal, cancel);
    drop(feats);
    drop(fp);
    check()?;
    progress(0.35);
    if pm.is_empty() {
        return Err(Error::NoOverlap);
    }
    // 4. Cameras.
    let infos: Vec<FrameInfo> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| FrameInfo {
            w: s.w(),
            h: s.h(),
            focal: focal.get(i).copied().flatten(),
        })
        .collect();
    let b = bundle::bundle(
        &infos,
        &pm,
        &BundleOptions {
            k1,
            ..BundleOptions::default()
        },
    );
    check()?;
    progress(0.45);
    let kept: Vec<usize> = (0..n).filter(|&i| b.cameras[i].is_some()).collect();
    if kept.len() < 2 {
        return Err(Error::NoOverlap);
    }
    let left_out: Vec<PathBuf> = (0..n)
        .filter(|&i| b.cameras[i].is_none())
        .map(|i| sources[i].shot.path.clone())
        .collect();
    let cameras = b.cameras.clone();
    let mut sources: Vec<Source> = sources;
    let mut metas = metas;
    // Only the joined frames from here.
    let mut keep_flags = vec![false; n];
    for &i in &kept {
        keep_flags[i] = true;
    }
    let mut idx = 0;
    sources.retain(|_| {
        idx += 1;
        keep_flags[idx - 1]
    });
    let mut idx = 0;
    metas.retain(|_| {
        idx += 1;
        keep_flags[idx - 1]
    });
    let cams: Vec<Camera> = kept.iter().filter_map(|&i| b.cameras[i]).collect();
    // 5. Projection and frame.
    let sized: Vec<(Camera, (u32, u32))> = cams
        .iter()
        .zip(&sources)
        .map(|(c, s)| (*c, (s.w(), s.h())))
        .collect();
    let projection = o.projection.unwrap_or_else(|| project::choose(&sized));
    let full = project::frame(projection, &sized, None)
        .or_else(|| project::frame(Projection::Cylindrical, &sized, None))
        .ok_or(Error::NoOverlap)?;
    let pf = full.scaled(f64::from(PROXY_SIDE) / f64::from(full.w.max(full.h)).max(1.0));
    // 6. Gains on the proxy.
    let proxies: Vec<(Proxy, u32)> = sources
        .iter()
        .map(|s| (luma_proxy(&s.plane, block), block))
        .collect();
    let mut lay = layers(&sources, &cams, &proxies, &pf);
    drop(proxies);
    let g = gains(&lay);
    for (l, gi) in lay.iter_mut().zip(&g) {
        let d = gi.log2() as f32;
        l.v.iter_mut().for_each(|v| *v += d);
    }
    check()?;
    progress(0.5);
    // 7. Seams.
    let labels = if o.plain_seams {
        seam::voronoi(&lay, pf.w as usize, pf.h as usize)
    } else {
        seam::seams(&lay, pf.w as usize, pf.h as usize)
    };
    let coverage_proxy: Vec<bool> = (0..lay[0].v.len())
        .map(|i| lay.iter().any(|l| l.v[i].is_finite()))
        .collect();
    drop(lay);
    check()?;
    progress(0.55);
    // The crop.
    let out = if o.crop {
        let (x, y, w, h) =
            project::largest_rectangle(&coverage_proxy, pf.w as usize, pf.h as usize);
        if w < 2 || h < 2 {
            return Err(Error::NoOverlap);
        }
        let k = full.scale / pf.scale;
        // One proxy pixel in from each edge.
        let x0 = ((x as f64 + 1.0) * k).ceil() as u32;
        let y0 = ((y as f64 + 1.0) * k).ceil() as u32;
        let x1 = (((x + w) as f64 - 1.0) * k).floor() as u32;
        let y1 = (((y + h) as f64 - 1.0) * k).floor() as u32;
        let (x1, y1) = (x1.min(full.w), y1.min(full.h));
        if x1 <= x0 + 1 || y1 <= y0 + 1 {
            return Err(Error::NoOverlap);
        }
        full.crop(x0, y0, x1 - x0, y1 - y0)
    } else {
        full
    };
    // The output's memory.
    let out_bytes = u64::from(out.w) * u64::from(out.h) * (u64::from(cpp) * 4 * 2 + 1);
    let _out_mem = specular_core::budget::reserve(out_bytes).map_err(Error::NeedsMemory)?;
    // 8. Render.
    let scale: Vec<f32> = sources
        .iter()
        .zip(&g)
        .map(|(s, gi)| (gi / s.exposure) as f32)
        .collect();
    let (radiance, covered) = render(
        &sources,
        &cams,
        &scale,
        &mosaic,
        cpp,
        &out,
        &labels,
        &pf,
        o.bands,
        &|f| progress(0.55 + 0.45 * f),
        cancel,
    )?;
    let reference = reference_of(&cams, &out);
    let clip = sources
        .iter()
        .zip(&scale)
        .map(|(s, k)| s.plane.sat * k)
        .fold(0.0f32, f32::max);
    let mut meta = metas[reference].clone();
    // The geometry and vignetting are corrected; the reference's opcodes and lens tables
    // no longer apply.
    meta.lens = None;
    meta.opcodes = Default::default();
    meta.opcodes_unknown = Vec::new();
    let shots: Vec<Shot> = sources.iter().map(|s| s.shot.clone()).collect();
    let merged = Merged {
        width: out.w,
        height: out.h,
        cpp,
        mosaic,
        radiance,
        clip,
        reference,
        exposures: g.clone(),
        transforms: vec![super::Transform::identity(0.0, 0.0); shots.len()],
        warps: vec![super::Homography::IDENTITY; shots.len()],
        ghosts: GhostMask::default(),
        meta,
        crop: specular_raw::Rect {
            x: 0,
            y: 0,
            w: out.w,
            h: out.h,
        },
        shots,
    };
    progress(1.0);
    Ok(Panorama {
        merged,
        frame: out,
        coverage: covered,
        cameras,
        left_out,
        rms: b.rms,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The panorama's XMP packet: its sources (as a merge's, `spm:` namespace) and, for an
/// equirectangular projection, the Photo Sphere (`GPano`) facts that let viewers open it in
/// their 360° mode (the frame's place in the full 360° × 180° sphere).
pub fn xmp(p: &Panorama) -> Vec<u8> {
    let m = &p.merged;
    let name = |s: &Shot| {
        s.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let items: String = m
        .shots
        .iter()
        .map(|s| format!("<rdf:li>{}</rdf:li>", esc(&name(s))))
        .collect();
    let gains: Vec<String> = m.exposures.iter().map(|g| format!("{g:.4}")).collect();
    let gpano = if p.frame.projection == Projection::Equirectangular {
        let f = &p.frame;
        let full_w = (std::f64::consts::TAU * f.scale).round();
        let full_h = (std::f64::consts::PI * f.scale).round();
        let left = (f.x0 + std::f64::consts::PI * f.scale).round().max(0.0);
        let top = (f.y0 + std::f64::consts::FRAC_PI_2 * f.scale)
            .round()
            .max(0.0);
        format!(
            " xmlns:GPano=\"http://ns.google.com/photos/1.0/panorama/\" \
GPano:ProjectionType=\"equirectangular\" GPano:UsePanoramaViewer=\"True\" \
GPano:FullPanoWidthPixels=\"{full_w}\" GPano:FullPanoHeightPixels=\"{full_h}\" \
GPano:CroppedAreaImageWidthPixels=\"{}\" GPano:CroppedAreaImageHeightPixels=\"{}\" \
GPano:CroppedAreaLeftPixels=\"{left}\" GPano:CroppedAreaTopPixels=\"{top}\"",
            f.w, f.h
        )
    } else {
        String::new()
    };
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
<rdf:Description rdf:about=\"\" xmlns:spm=\"{}\" spm:reference=\"{}\" spm:gains=\"{}\"{gpano}>\
<spm:sources><rdf:Seq>{items}</rdf:Seq></spm:sources></rdf:Description></rdf:RDF></x:xmpmeta>\
<?xpacket end=\"w\"?>",
        super::dng::NS,
        esc(&name(&m.shots[m.reference])),
        gains.join(" "),
    )
    .into_bytes()
}

#[cfg(test)]
#[path = "pano_tests.rs"]
mod tests;
