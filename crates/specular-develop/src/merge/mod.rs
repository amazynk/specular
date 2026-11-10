//! Bracket merge: several raws of one scene at different exposures
//! become one scene-referred raw with the shadows of the longest and the highlights of the
//! shortest, still a mosaic, ready for Develop or saved as a floating point DNG.
//!
//! The steps, each in its own module:
//!
//! 1. [`detect`]: which frames form a bracket (capture time, camera, exposure steps, the
//!    camera's own bracketing flag).
//! 2. Normalise: each frame's stored samples to `(v − black)/(white − black)` over the
//!    reference's visible area, unclipped; samples at 97 % of the clip level or above are
//!    marked saturated.
//! 3. [`align`]: ECC on a log-luma proxy of whole CFA blocks, from translation up to a
//!    homography (hand-held frames), coarse to fine; then each frame is resampled into the reference's
//!    geometry colour by colour (`warp`), never demosaiced.
//! 4. `fuse`: exposures measured on the data, ghosts found against the reference, and the
//!    noise-optimal weighted merge of Granados et al. 2010 with each frame's raw noise model.
//! 5. Output: [`Merged::to_sensor`] is a [`SensorData`] with float samples (Develop's raw
//!    front end takes it as it takes a file's), and [`dng`] writes it as a floating point DNG.
//!
//! Everything runs on the CPU with rayon: the proxy pyramid is small and the per-sample work
//! is a few multiply-adds per frame, so a GPU round trip would not pay (see the lane's
//! timings).

pub mod align;
pub mod blend;
pub mod bundle;
pub mod detect;
pub mod dng;
pub mod features;
pub mod focus;
mod fuse;
pub mod fusion;
pub mod homography;
pub mod lens_fit;
pub mod pano;
pub mod project;
pub mod ransac;
pub mod rgb;
pub mod seam;
pub(crate) mod warp;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, NeedsMemory};
use specular_raw::sensor::{BlackLevels, Layout, SensorData, SensorMeta};

pub use align::{Motion, Transform};
pub use detect::{BracketSet, Shot, bracket_sets};
pub use homography::Homography;

/// The most frames one merge takes.
pub const MAX_FRAMES: usize = 9;
/// Samples at this share of the clip level and above count as saturated.
const NEAR_CLIP: f32 = 0.97;

/// The colour layout of the merged area (positions relative to its top-left).
#[derive(Clone, Debug, PartialEq)]
pub enum Mosaic {
    Bayer([u8; 4]),
    Pattern { w: u32, h: u32, colors: Vec<u8> },
    Linear,
    Mono,
}

impl Mosaic {
    /// The layout of `layout`'s samples seen from `(ox, oy)`.
    fn of(layout: &Layout, ox: u32, oy: u32) -> Mosaic {
        match layout {
            Layout::Bayer(_) => Mosaic::Bayer(std::array::from_fn(|k| {
                layout.color(ox + (k as u32 & 1), oy + (k as u32 >> 1))
            })),
            Layout::Pattern(c) => Mosaic::Pattern {
                w: c.width,
                h: c.height,
                colors: (0..c.height)
                    .flat_map(|y| (0..c.width).map(move |x| (x, y)))
                    .map(|(x, y)| layout.color(ox + x, oy + y))
                    .collect(),
            },
            Layout::Linear => Mosaic::Linear,
            Layout::Mono => Mosaic::Mono,
        }
    }

    #[inline]
    pub fn color(&self, x: u32, y: u32) -> u8 {
        match self {
            Mosaic::Bayer(p) => p[((y & 1) * 2 + (x & 1)) as usize],
            Mosaic::Pattern { w, h, colors } => colors[((y % h) * w + x % w) as usize],
            Mosaic::Linear | Mosaic::Mono => 1,
        }
    }

    /// The side of the block the proxy averages: one whole set of colours.
    pub fn block(&self) -> u32 {
        match self {
            Mosaic::Bayer(_) => 2,
            Mosaic::Pattern { .. } => 3,
            Mosaic::Linear | Mosaic::Mono => 1,
        }
    }

    /// As a sensor layout.
    pub fn layout(&self) -> Layout {
        match self {
            Mosaic::Bayer(p) => Layout::Bayer(*p),
            Mosaic::Pattern { w, h, colors } => {
                specular_raw::Cfa::new(*w, *h, colors.clone()).map_or(Layout::Mono, Layout::Pattern)
            }
            Mosaic::Linear => Layout::Linear,
            Mosaic::Mono => Layout::Mono,
        }
    }
}

/// A frame's samples over the merged area, normalised (black 0, white 1), unclipped. +∞ marks
/// a saturated sample, NaN one that is not there (outside the frame after alignment).
#[derive(Clone, Debug)]
pub(crate) struct Plane {
    pub w: u32,
    pub h: u32,
    pub cpp: u32,
    pub v: Vec<f32>,
    /// The normalised level saturation starts at.
    pub sat: f32,
}

impl Plane {
    fn clone_empty(&self) -> Plane {
        Plane {
            v: Vec::new(),
            ..*self
        }
    }
}

/// The normalised plane of a sensor's visible area.
fn normalise(s: &SensorData) -> Plane {
    let m = &s.meta;
    let c = s.crop;
    let cpp = s.cpp as usize;
    let clip = if m.clip_white > 0.0 {
        if s.float.is_some() {
            m.clip_white
        } else {
            m.clip_white.min(m.white)
        }
    } else {
        m.white
    } * NEAR_CLIP;
    let black_mean = m.black.max().min(m.white - 1.0).max(0.0);
    let sat = (clip - black_mean) / (m.white - black_mean).max(1.0);
    let mut v = vec![0f32; c.w as usize * c.h as usize * cpp];
    use rayon::prelude::*;
    v.par_chunks_mut(c.w as usize * cpp)
        .enumerate()
        .for_each(|(row, out)| {
            let y = c.y + row as u32;
            for x in 0..c.w {
                let sx = c.x + x;
                for ch in 0..cpp {
                    let i = (y as usize * s.width as usize + sx as usize) * cpp + ch;
                    let l = match (&s.float, &m.linearization) {
                        (Some(f), _) => f[i],
                        (None, Some(t)) => f32::from(t[(s.samples[i] as usize).min(t.len() - 1)]),
                        (None, None) => f32::from(s.samples[i]),
                    };
                    let b = m.black.at(sx, y, ch as u32);
                    out[x as usize * cpp + ch] = if l >= clip {
                        f32::INFINITY
                    } else {
                        (l - b) / (m.white - b).max(1.0)
                    };
                }
            }
        });
    Plane {
        w: c.w,
        h: c.h,
        cpp: s.cpp,
        v,
        sat,
    }
}

/// A frame to merge: its sensor data and what its metadata says.
pub struct Frame {
    pub sensor: SensorData,
    pub shot: Shot,
}

impl Frame {
    /// Reads and decodes a raw file.
    pub fn read(path: &Path) -> Result<Frame, Error> {
        let data = std::fs::read(path)
            .map_err(|e| Error::Read(path.display().to_string(), e.to_string()))?;
        let f = specular_raw::parse(&data)
            .map_err(|e| Error::Read(path.display().to_string(), e.to_string()))?;
        if !f.can_decode_sensor() {
            return Err(Error::NotRaw(path.display().to_string()));
        }
        let sensor = f
            .decode_sensor(&data, &specular_raw::DecodeOptions::default())
            .map_err(|e| Error::Read(path.display().to_string(), e.to_string()))?;
        let shot = Shot::of_raw(path.to_path_buf(), &data).unwrap_or_default();
        Ok(Frame { sensor, shot })
    }
}

/// How to merge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub motion: Motion,
    /// Leave out frames where they disagree with the reference beyond noise: the `z` they
    /// must exceed (4 is a good default). `None`: no ghost handling.
    pub ghosts: Option<f32>,
    /// The frame others are aligned to and that wins in ghosts; `None` picks the middle
    /// exposure.
    pub reference: Option<usize>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            motion: Motion::Euclidean,
            ghosts: Some(4.0),
            reference: None,
        }
    }
}

/// Why a merge could not be made. Each has a plain-language reason (`Display`); the app
/// words them through its own strings by variant.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Fewer than two frames.
    TooFew,
    /// More than [`MAX_FRAMES`].
    TooMany,
    /// The frames are not from one sensor (size, layout or samples per pixel differ).
    NotOneCamera,
    /// A frame is not a raw the develop can read.
    NotRaw(String),
    /// A frame could not be read: (file, why).
    Read(String, String),
    /// The exposures could neither be measured nor read from the files.
    NoExposures,
    /// A frame could not be aligned (too little overlap or detail): its index.
    NotAligned(usize),
    /// No two photos of a panorama overlap.
    NoOverlap,
    /// More photos than a merge of this kind takes: the most it takes.
    Cap(usize),
    NeedsMemory(NeedsMemory),
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TooFew => write!(f, "a merge needs at least two photos"),
            Error::TooMany => write!(f, "a merge takes at most {MAX_FRAMES} photos"),
            Error::NotOneCamera => write!(f, "the photos are not from one camera"),
            Error::NotRaw(p) => write!(f, "{p} is not a raw photo"),
            Error::Read(p, e) => write!(f, "{p} could not be read: {e}"),
            Error::NoExposures => write!(
                f,
                "the photos' exposures are unknown and could not be measured"
            ),
            Error::NotAligned(i) => write!(f, "photo {} could not be aligned", i + 1),
            Error::NoOverlap => write!(f, "the photos do not overlap"),
            Error::Cap(n) => write!(f, "this merge takes at most {n} photos"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
            Error::Cancelled => write!(f, "stopped"),
        }
    }
}

impl std::error::Error for Error {}

/// Where frames were left out for disagreeing with the reference, on the block grid.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GhostMask {
    pub width: u32,
    pub height: u32,
    /// Samples per block side.
    pub block: u32,
    /// 0 nothing left out … 255 left out fully.
    pub mask: Vec<u8>,
}

impl GhostMask {
    /// The share of the picture where a frame was left out (at least half).
    pub fn coverage(&self) -> f32 {
        let n = self.mask.iter().filter(|&&m| m >= 128).count();
        n as f32 / self.mask.len().max(1) as f32
    }
}

/// A merged bracket.
#[derive(Clone, Debug)]
pub struct Merged {
    pub width: u32,
    pub height: u32,
    pub cpp: u32,
    pub mosaic: Mosaic,
    /// Radiance per sample in the reference's units (1 = its white level), unclipped.
    pub radiance: Vec<f32>,
    /// The radiance where every frame clipped (the brightest the merge holds).
    pub clip: f32,
    pub reference: usize,
    /// Each frame's exposure relative to the reference.
    pub exposures: Vec<f64>,
    /// Each frame's alignment (reference → frame), in samples of the merged area: rotation
    /// and translation (the nearest for richer motions).
    pub transforms: Vec<Transform>,
    /// Each frame's whole alignment (reference → frame), in samples of the merged area.
    pub warps: Vec<Homography>,
    pub ghosts: GhostMask,
    /// The reference's sensor metadata (colour, white balance, lens, orientation, its DNG
    /// opcode lists in stored coordinates).
    pub meta: SensorMeta,
    /// The reference's visible area in its stored image (the merged area).
    pub crop: specular_raw::Rect,
    pub shots: Vec<Shot>,
}

impl Merged {
    /// The merge as sensor data for Develop: float samples with the reference's white at
    /// 65535 (brighter values above it), black 0; the 16-bit samples are those clipped there.
    pub fn to_sensor(&self) -> SensorData {
        let scale = 65535.0f32;
        let float: Arc<[f32]> = self.radiance.iter().map(|&r| r * scale).collect();
        let samples = float
            .iter()
            .map(|&v| if v > 0.0 { v.min(65535.0) + 0.5 } else { 0.0 } as u16)
            .collect();
        let mut meta = self.meta.clone();
        meta.layout = self.mosaic.layout();
        meta.black = BlackLevels::simple([0.0; 4], self.cpp);
        meta.white = scale;
        meta.clip_white = self.clip * scale;
        meta.linearization = None;
        // The opcode lists the saved DNG gets (moved to the merged area, the maker-note lens
        // tables as opcodes), read as a DNG reader reads them; no maker notes, as in the file.
        let lists = dng::opcode_list_bytes(self, self.cpp);
        meta.opcodes = specular_raw::sensor::opcode_lists_info(
            [
                lists[0].as_deref(),
                lists[1].as_deref(),
                lists[2].as_deref(),
            ],
            self.width,
            self.height,
            self.cpp,
        );
        meta.opcodes_unknown = Vec::new();
        meta.lens = None;
        meta.truncated = false;
        SensorData {
            width: self.width,
            height: self.height,
            cpp: self.cpp,
            samples,
            float: Some(float),
            crop: specular_raw::Rect {
                x: 0,
                y: 0,
                w: self.width,
                h: self.height,
            },
            meta,
        }
    }

    /// The ghost mask over a dimmed grey picture of the merge, as RGBA8 on the block grid
    /// (for a preview): left-out areas red.
    pub fn ghost_preview(&self) -> (u32, u32, Vec<u8>) {
        let g = &self.ghosts;
        let (w, h, b) = (g.width as usize, g.height as usize, g.block.max(1) as usize);
        let mut out = vec![0u8; w * h * 4];
        let cpp = self.cpp as usize;
        let (mw, mh) = (self.width as usize, self.height as usize);
        for gy in 0..h {
            for gx in 0..w {
                let (mut s, mut c) = (0.0f32, 0);
                for y in gy * b..((gy + 1) * b).min(mh) {
                    for x in gx * b..((gx + 1) * b).min(mw) {
                        for ch in 0..cpp {
                            s += self.radiance[(y * mw + x) * cpp + ch];
                            c += 1;
                        }
                    }
                }
                let r = (s / c.max(1) as f32).max(0.0);
                // A plain tone curve: mid grey (0.18) near the middle.
                let grey = (r / (r + 0.18) * 200.0) as u8;
                let m = f32::from(g.mask[gy * w + gx]) / 255.0;
                let o = &mut out[(gy * w + gx) * 4..][..4];
                let mix = |a: f32, b: f32| (a + (b - a) * m * 0.7).round() as u8;
                o[0] = mix(f32::from(grey), 255.0);
                o[1] = mix(f32::from(grey), 40.0);
                o[2] = mix(f32::from(grey), 40.0);
                o[3] = 255;
            }
        }
        (w as u32, h as u32, out)
    }
}

/// Bytes a merge of `frames` sensors of `w × h × cpp` visible samples holds at its peak.
pub fn memory_needed(frames: usize, w: u32, h: u32, cpp: u32) -> u64 {
    let n = u64::from(w) * u64::from(h) * u64::from(cpp);
    // Each frame normalised and warped (two planes while warping), the merge, the sensor
    // made from it (float and 16-bit).
    n * 4 * (frames as u64 + 1) + n * (4 + 4 + 2)
}

/// The middle exposure (by metadata; the first when unknown).
pub fn default_reference(shots: &[Shot]) -> usize {
    let mut by: Vec<(usize, f64)> = shots
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.exposure().unwrap_or(0.0)))
        .collect();
    by.sort_by(|a, b| a.1.total_cmp(&b.1));
    by.get(by.len() / 2).map_or(0, |b| b.0)
}

/// Merges `frames` (see the module documentation). `progress` gets 0..=1.
pub fn merge(
    frames: Vec<Frame>,
    o: &Options,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<Merged, Error> {
    if frames.len() < 2 {
        return Err(Error::TooFew);
    }
    if frames.len() > MAX_FRAMES {
        return Err(Error::TooMany);
    }
    let shots: Vec<Shot> = frames.iter().map(|f| f.shot.clone()).collect();
    let reference = o
        .reference
        .filter(|&r| r < frames.len())
        .unwrap_or_else(|| default_reference(&shots));
    let r0 = &frames[reference].sensor;
    let same = |s: &SensorData| {
        s.width == r0.width
            && s.height == r0.height
            && s.cpp == r0.cpp
            && s.crop == r0.crop
            && s.meta.layout == r0.meta.layout
    };
    if !frames.iter().all(|f| same(&f.sensor)) {
        return Err(Error::NotOneCamera);
    }
    let (w, h, cpp) = (r0.crop.w, r0.crop.h, r0.cpp);
    let crop = r0.crop;
    let _mem = specular_core::budget::reserve(memory_needed(frames.len(), w, h, cpp))
        .map_err(Error::NeedsMemory)?;
    let mosaic = Mosaic::of(&r0.meta.layout, r0.crop.x, r0.crop.y);
    let meta = r0.meta.clone();
    let n = frames.len();
    let check = || {
        if cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    };

    // 1. Noise models and normalised planes (the sensors go as they are read).
    let mut noise = Vec::with_capacity(n);
    let mut planes = Vec::with_capacity(n);
    for (i, f) in frames.into_iter().enumerate() {
        check()?;
        let rn = crate::ops::detail::RawNoise::of_sensor(&f.sensor);
        noise.push(fuse::Noise {
            a: rn.a.map(|v| v as f32),
            b: rn.b.map(|v| v as f32),
        });
        planes.push(normalise(&f.sensor));
        progress(0.15 * (i + 1) as f32 / n as f32);
    }

    // 2. Alignment on the proxies, then each frame into the reference's geometry.
    let block = mosaic.block();
    let dark = 0.002f32;
    let mut transforms =
        vec![Transform::identity((f64::from(w) - 1.0) / 2.0, (f64::from(h) - 1.0) / 2.0); n];
    let mut warps = vec![Homography::IDENTITY; n];
    if o.motion != Motion::None {
        let template = align::proxy(&planes[reference], block, dark);
        for i in 0..n {
            check()?;
            if i == reference {
                continue;
            }
            let image = align::proxy(&planes[i], block, dark);
            // Richer models start from the rotation and translation: their extra
            // parameters are weakly fixed at the coarse levels.
            let found = if o.motion.params() > 3 {
                let first = align::ecc(&template, &image, Motion::Euclidean);
                let init = first.map_or(Homography::IDENTITY, |f| f.h);
                let rich = align::ecc_refine(&template, &image, o.motion, &init);
                // Kept only when it explains the frame clearly better (a tenth less
                // uncorrelated): extra parameters otherwise fit noise and clipping.
                match (rich, first) {
                    (Some((r, start)), Some(_)) if 1.0 - r.rho < 0.9 * (1.0 - start) => Some(r),
                    (r, None) => r.map(|r| r.0),
                    (_, f) => f,
                }
            } else {
                align::ecc(&template, &image, o.motion)
            }
            .ok_or(Error::NotAligned(i))?;
            let t = align::to_samples(&found.transform, block, w, h);
            let hs = align::h_to_samples(&found.h, block);
            // An alignment that runs off by more than a fifth of the picture failed.
            if hs.max_shift(w, h) > f64::from(w.max(h)) / 5.0 {
                return Err(Error::NotAligned(i));
            }
            transforms[i] = t;
            warps[i] = hs;
            progress(0.15 + 0.35 * (i + 1) as f32 / n as f32);
        }
        for i in 0..n {
            check()?;
            if i != reference {
                planes[i] = warp::warp(&planes[i], &mosaic, &warps[i]);
            }
            progress(0.5 + 0.2 * (i + 1) as f32 / n as f32);
        }
    }

    // 3. Exposures, ghosts and the weighted merge.
    check()?;
    let guess: Vec<Option<f64>> = shots.iter().map(Shot::exposure).collect();
    let exposures = fuse::exposures(&planes, &guess, reference).ok_or(Error::NoExposures)?;
    progress(0.75);
    let (keep, ghosts) = fuse::ghosts(&planes, &exposures, &noise, reference, block, o.ghosts);
    progress(0.8);
    // The reference first: the merge reads its size from there.
    planes.swap(0, reference);
    let mut e = exposures.clone();
    e.swap(0, reference);
    let mut nz = noise.clone();
    nz.swap(0, reference);
    let mut kp = keep;
    kp.swap(0, reference);
    let shortest = (0..n).min_by(|&a, &b| e[a].total_cmp(&e[b])).unwrap_or(0);
    let clip = planes[shortest].sat / e[shortest] as f32;
    let radiance =
        fuse::merge(&planes, &mosaic, &e, &nz, &kp, &ghosts, cancel).ok_or(Error::Cancelled)?;
    progress(1.0);
    Ok(Merged {
        width: w,
        height: h,
        cpp,
        mosaic,
        radiance,
        clip,
        reference,
        exposures,
        transforms,
        warps,
        ghosts,
        meta,
        crop,
        shots,
    })
}

/// Float sensors (HDR merges read back from a DNG): the DNG `BaselineExposure` applied to
/// the samples, so a merge written with its brightest value at 1.0 develops at its
/// reference frame's brightness; the clip level follows. Other sensors are unchanged.
pub fn apply_baseline(mut s: SensorData) -> SensorData {
    let Some(f) = s.float.take() else { return s };
    let be = s.meta.dng.as_ref().map_or(0.0, |d| d.baseline_exposure);
    let gain = 2f64.powf(be) as f32;
    let f: Arc<[f32]> = if (gain - 1.0).abs() > 1e-7 {
        f.iter().map(|v| v * gain).collect()
    } else {
        f
    };
    let max = f.iter().copied().fold(0.0f32, f32::max);
    s.samples = f
        .iter()
        .map(|&v| if v > 0.0 { v.min(65535.0) + 0.5 } else { 0.0 } as u16)
        .collect();
    if let Some(d) = s.meta.dng.as_mut() {
        d.baseline_exposure = 0.0;
    }
    // The brightest value is where the merge clipped.
    s.meta.clip_white = max.max(s.meta.white);
    s.float = Some(f);
    s
}
