//! Focus stacking: frames of one scene focused at different distances become one picture
//! sharp throughout, still a mosaic (raws) or linear RGB, with an optional depth map.
//!
//! - **Frames** stream through a [`FrameSource`]: a stack can hold 200 frames of 45 MP, so
//!   a frame is read when a pass needs it. When the memory budget allows, the normalised
//!   frames are kept after the first read; otherwise each pass reads them again.
//! - **Alignment** (focus breathing changes the scale): each frame to its neighbour by ECC
//!   with a similarity on a reduced log-luma proxy, the maps chained, then referred to the
//!   middle frame, whose geometry the result keeps.
//! - **Cells**: all weights are made on the grid of whole CFA periods (2×2 Bayer, 6×6
//!   X-Trans, one pixel for RGB) and applied to every sample of a cell, so the mosaic is kept.
//! - **Focus measure**: the sum-modified Laplacian (Nayar & Nakagawa, "Shape from focus",
//!   TPAMI 1994; the best general measure in Pertuz et al., "Analysis of focus measure
//!   operators for shape-from-focus", Pattern Recognition 2013) of the cells' linear luma
//!   over a 5×5 window, with the share white noise adds taken out (the noise from
//!   Immerkær's estimator, the 10th percentile of 32-cell blocks).
//! - **Depth map** ([`Method::DepthMap`], the default): the frame of the best measure per
//!   cell, smoothed by a confidence-weighted guided filter (He, Sun & Tang, "Guided image
//!   filtering", ECCV 2010) with the reference's log luma as guide, so the depth follows
//!   edges; each cell is then the blend of the two frames around its depth.
//! - **Pyramid** ([`Method::Pyramid`]): Laplacian pyramids of every colour lattice; per band
//!   and position the frame with the largest area salience (the 3×3 energy of the luma
//!   band) wins, the choice cleaned by a 3×3 majority filter (Burt & Kolczynski, "Enhanced
//!   image capture through fusion", ICCV 1993); the base band is the mean.

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use specular_core::CancelToken;

use super::align::{self, Motion, Proxy, Transform};
use super::blend::{self, Img};
use super::homography::Homography;
use super::{Error, Frame, GhostMask, Merged, Mosaic, Plane, Shot, normalise, warp};

/// The most frames one stack takes.
pub const MAX_FRAMES: usize = super::detect::MAX_FOCUS_FRAMES;
/// The alignment proxy's most pixels.
const ALIGN_PIXELS: usize = 1_500_000;
/// Half side of the focus measure's window, in cells.
const WINDOW: usize = 2;
/// Bands of the pyramid method at most.
const BANDS: usize = 7;

/// How the sharp parts are chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Method {
    /// Per cell, the frame where it is sharpest, smoothed along edges.
    #[default]
    DepthMap,
    /// Per band of a Laplacian pyramid, the strongest detail.
    Pyramid,
}

/// How to stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub method: Method,
    /// Align the frames (focus breathing); off for a focusing rail with a fixed lens.
    pub align: bool,
    /// Also give the depth map ([`Stacked::depth`]).
    pub depth_map: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: Method::DepthMap,
            align: true,
            depth_map: false,
        }
    }
}

/// Where a stack's frames come from, in focus order.
pub trait FrameSource: Sync {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Frame `i`, read again each time it is asked for.
    fn get(&self, i: usize) -> Result<Frame, Error>;
}

/// Frames already in memory (tests, small stacks).
pub struct InMemory(pub Vec<Frame>);

impl FrameSource for InMemory {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn get(&self, i: usize) -> Result<Frame, Error> {
        let f = &self.0[i];
        Ok(Frame {
            sensor: f.sensor.clone(),
            shot: f.shot.clone(),
        })
    }
}

/// Raw files, read when needed; `shots` (when given, same order) replace the files' facts.
pub struct Paths {
    pub paths: Vec<PathBuf>,
    pub shots: Vec<Shot>,
}

impl FrameSource for Paths {
    fn len(&self) -> usize {
        self.paths.len()
    }
    fn get(&self, i: usize) -> Result<Frame, Error> {
        let mut f = Frame::read(&self.paths[i])?;
        if let Some(s) = self.shots.get(i) {
            f.shot = s.clone();
        }
        Ok(f)
    }
}

/// Where each cell is sharpest: a continuous frame index on the cell grid of the result.
#[derive(Clone, Debug, PartialEq)]
pub struct DepthMap {
    pub width: u32,
    pub height: u32,
    /// Samples per cell side (2 Bayer, 6 X-Trans, 1 RGB).
    pub cell: (u32, u32),
    /// 0 = the first frame's focus … frames − 1 = the last's.
    pub depth: Vec<f32>,
    pub frames: usize,
}

impl DepthMap {
    /// As 16-bit grey samples: the first frame's focus 0, the last's 65535.
    pub fn to_u16(&self) -> Vec<u16> {
        let k = 65535.0 / (self.frames.max(2) - 1) as f32;
        self.depth
            .iter()
            .map(|d| (d * k).round().clamp(0.0, 65535.0) as u16)
            .collect()
    }

    /// A 16-bit grey PNG.
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let px = specular_convert::pixels::Pixels {
            width: self.width,
            height: self.height,
            channels: 1,
            samples: specular_convert::pixels::Samples::Sixteen(self.to_u16()),
        };
        let o = specular_convert::Options {
            format: specular_convert::Format::Png,
            ..specular_convert::Options::default()
        };
        specular_convert::encode::encode(&o, &px, &specular_convert::encode::Meta::default())
    }
}

/// The depth map's file name beside a stack named after `reference`:
/// `IMG_0002-Stack-depth.png`.
pub fn depth_file_name(reference: &Path) -> String {
    let stem = reference
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stack".into());
    format!("{stem}-Stack-depth.png")
}

/// A made stack.
#[derive(Clone, Debug)]
pub struct Stacked {
    /// The picture: the reference's geometry and metadata, exposures 1, no ghosts.
    pub merged: Merged,
    /// When asked for.
    pub depth: Option<DepthMap>,
}

/// The cell side of a mosaic: one whole colour period.
fn period(m: &Mosaic) -> (usize, usize) {
    match m {
        Mosaic::Bayer(_) => (2, 2),
        Mosaic::Pattern { w, h, .. } => (*w as usize, *h as usize),
        Mosaic::Linear | Mosaic::Mono => (1, 1),
    }
}

/// Bytes a stack of `w × h × cpp` samples needs at its peak beyond the frames it keeps:
/// a frame read, normalised and warped, the result and its weights, the cell maps and (for
/// the pyramid) the result's bands.
pub fn memory_needed(w: u32, h: u32, cpp: u32, method: Method) -> u64 {
    let n = u64::from(w) * u64::from(h) * u64::from(cpp);
    let cells = n / u64::from(cpp.max(1));
    let raw = n * 2;
    let base = raw + n * 4 * 3 + cells * 4 * 12;
    match method {
        Method::DepthMap => base + n * 4,
        // The result's bands (4/3 of the samples) plus a lattice's pyramid and the
        // selection maps (index and salience per band).
        Method::Pyramid => base + n * 4 * 3 + cells * 6 * 2,
    }
}

/// Bytes the normalised frames take when kept.
fn kept_bytes(frames: usize, w: u32, h: u32, cpp: u32) -> u64 {
    frames as u64 * u64::from(w) * u64::from(h) * u64::from(cpp) * 4
}

/// The block means of `p` (2×2, NaN when fewer than two usable).
fn halve(p: &Proxy) -> Proxy {
    let (w, h) = (p.w / 2, p.h / 2);
    let mut v = vec![f32::NAN; w * h];
    v.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let (mut s, mut n) = (0.0, 0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let q = p.v[(2 * y + dy) * p.w + 2 * x + dx];
                if q.is_finite() {
                    s += q;
                    n += 1;
                }
            }
            if n >= 2 {
                *o = s / n as f32;
            }
        }
    });
    Proxy { w, h, v }
}

/// The alignment proxy of a plane and how many times it was halved.
fn align_proxy(p: &Plane, block: u32) -> (Proxy, u32) {
    let mut q = align::proxy(p, block, 0.002);
    let mut halvings = 0;
    while q.w * q.h > ALIGN_PIXELS && q.w.min(q.h) >= 128 {
        q = halve(&q);
        halvings += 1;
    }
    (q, halvings)
}

/// The cells' mean linear luma (NaN where a sample is missing; saturated samples count at
/// the saturation level).
fn cell_luma(p: &Plane, (px, py): (usize, usize)) -> Img {
    let (w, cpp) = (p.w as usize, p.cpp as usize);
    let (cw, ch) = (w / px, p.h as usize / py);
    let mut v = vec![f32::NAN; cw * ch];
    let n = (px * py * cpp) as f32;
    v.par_chunks_mut(cw.max(1))
        .enumerate()
        .for_each(|(cy, row)| {
            for (cx, o) in row.iter_mut().enumerate() {
                let mut s = 0.0f32;
                let mut ok = true;
                for dy in 0..py {
                    let line = &p.v[((cy * py + dy) * w + cx * px) * cpp..][..px * cpp];
                    for &q in line {
                        if q == f32::INFINITY {
                            s += p.sat;
                        } else if q.is_finite() {
                            s += q;
                        } else {
                            ok = false;
                        }
                    }
                }
                if ok {
                    *o = s / n;
                }
            }
        });
    Img { w: cw, h: ch, v }
}

/// Box sums of radius `r` (mirror-free: the window shrinks at the border), as means.
fn box_mean(a: &[f64], w: usize, h: usize, r: usize) -> Vec<f64> {
    // Integral image with a zero row and column.
    let mut s = vec![0.0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0.0;
        for x in 0..w {
            row += a[y * w + x];
            s[(y + 1) * (w + 1) + x + 1] = s[y * (w + 1) + x + 1] + row;
        }
    }
    let mut out = vec![0.0; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, o)| {
        let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
        for (x, v) in o.iter_mut().enumerate() {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let sum = s[y1 * (w + 1) + x1] - s[y0 * (w + 1) + x1] - s[y1 * (w + 1) + x0]
                + s[y0 * (w + 1) + x0];
            *v = sum / ((y1 - y0) * (x1 - x0)) as f64;
        }
    });
    out
}

/// The guided filter of `p` with guide `g` (He et al. 2010), radius `r`, regulariser `eps`.
pub(crate) fn guided(g: &[f64], p: &[f64], w: usize, h: usize, r: usize, eps: f64) -> Vec<f64> {
    let mi = box_mean(g, w, h, r);
    let mp = box_mean(p, w, h, r);
    let gg: Vec<f64> = g.iter().map(|v| v * v).collect();
    let gp: Vec<f64> = g.iter().zip(p).map(|(a, b)| a * b).collect();
    let mii = box_mean(&gg, w, h, r);
    let mip = box_mean(&gp, w, h, r);
    let mut a = vec![0.0; w * h];
    let mut b = vec![0.0; w * h];
    for i in 0..w * h {
        let var = mii[i] - mi[i] * mi[i];
        let cov = mip[i] - mi[i] * mp[i];
        a[i] = cov / (var + eps);
        b[i] = mp[i] - a[i] * mi[i];
    }
    let ma = box_mean(&a, w, h, r);
    let mb = box_mean(&b, w, h, r);
    (0..w * h).map(|i| ma[i] * g[i] + mb[i]).collect()
}

/// Immerkær's noise estimate of `y`: the 10th percentile of 32×32 blocks' estimates.
fn noise_sigma(y: &Img) -> f32 {
    const B: usize = 32;
    let (w, h) = (y.w, y.h);
    let mut est = Vec::new();
    for by in (1..h.saturating_sub(B)).step_by(B) {
        for bx in (1..w.saturating_sub(B)).step_by(B) {
            let (mut s, mut n) = (0.0f64, 0usize);
            for yy in by..by + B - 2 {
                for xx in bx..bx + B - 2 {
                    let at = |dx: i64, dy: i64| {
                        f64::from(y.v[(yy as i64 + dy) as usize * w + (xx as i64 + dx) as usize])
                    };
                    let v = at(-1, -1) - 2.0 * at(0, -1) + at(1, -1) - 2.0 * at(-1, 0)
                        + 4.0 * at(0, 0)
                        - 2.0 * at(1, 0)
                        + at(-1, 1)
                        - 2.0 * at(0, 1)
                        + at(1, 1);
                    if v.is_finite() {
                        s += v.abs();
                        n += 1;
                    }
                }
            }
            if n > 64 {
                est.push((std::f64::consts::FRAC_PI_2.sqrt() * s / (6.0 * n as f64)) as f32);
            }
        }
    }
    if est.is_empty() {
        return 0.0;
    }
    est.sort_by(f32::total_cmp);
    est[est.len() / 10]
}

/// The sum-modified Laplacian of `y` over the window, the noise's share taken out; −∞ where
/// a cell is missing.
fn measure(y: &Img, sigma: f32) -> Vec<f32> {
    let (w, h) = (y.w, y.h);
    // |2a − b − c| of white noise σ has mean 2σ·√(3/π); two of them.
    let noise = 4.0 * sigma * (3.0 / std::f32::consts::PI).sqrt();
    let mut ml = vec![0f64; w * h];
    let mut bad = vec![false; w * h];
    ml.par_chunks_mut(w)
        .zip(bad.par_chunks_mut(w))
        .enumerate()
        .for_each(|(yy, (row, b))| {
            for x in 0..w {
                let c = y.v[yy * w + x];
                let at = |dx: i64, dy: i64| {
                    let (xx, yv) = (x as i64 + dx, yy as i64 + dy);
                    if xx < 0 || yv < 0 || xx >= w as i64 || yv >= h as i64 {
                        c
                    } else {
                        y.v[yv as usize * w + xx as usize]
                    }
                };
                let v =
                    (2.0 * c - at(-1, 0) - at(1, 0)).abs() + (2.0 * c - at(0, -1) - at(0, 1)).abs();
                if v.is_finite() {
                    row[x] = f64::from((v - noise).max(0.0));
                } else {
                    b[x] = true;
                }
            }
        });
    let s = box_mean(&ml, w, h, WINDOW);
    s.iter()
        .zip(&bad)
        .map(|(&v, &b)| if b { f32::NEG_INFINITY } else { v as f32 })
        .collect()
}

/// The 3×3 energy (salience) of a band.
fn salience(b: &Img) -> Vec<f32> {
    let e: Vec<f64> = b.v.iter().map(|v| f64::from(v * v)).collect();
    box_mean(&e, b.w, b.h, 1)
        .into_iter()
        .map(|v| v as f32)
        .collect()
}

/// The 3×3 majority of `idx` (kept where no index holds five of the nine).
fn majority(idx: &[u16], w: usize, h: usize) -> Vec<u16> {
    let mut out = idx.to_vec();
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        if y == 0 || y + 1 >= h {
            return;
        }
        for x in 1..w.saturating_sub(1) {
            let mut n = [(0u16, 0u8); 9];
            let mut k = 0;
            for dy in 0..3 {
                for dx in 0..3 {
                    let v = idx[(y + dy - 1) * w + x + dx - 1];
                    match n[..k].iter_mut().find(|e| e.0 == v) {
                        Some(e) => e.1 += 1,
                        None => {
                            n[k] = (v, 1);
                            k += 1;
                        }
                    }
                }
            }
            if let Some(&(v, c)) = n[..k].iter().max_by_key(|e| e.1)
                && c >= 5
            {
                row[x] = v;
            }
        }
    });
    out
}

/// A lattice of `p`: the samples at offset (ox, oy) of each cell, channel `c` (saturated at
/// the saturation level, missing NaN).
fn lattice(p: &Plane, (px, py): (usize, usize), (ox, oy, c): (usize, usize, usize)) -> Img {
    let (w, cpp) = (p.w as usize, p.cpp as usize);
    let (cw, ch) = (w / px, p.h as usize / py);
    let mut v = vec![0f32; cw * ch];
    v.par_chunks_mut(cw.max(1))
        .enumerate()
        .for_each(|(cy, row)| {
            for (cx, o) in row.iter_mut().enumerate() {
                let q = p.v[((cy * py + oy) * w + cx * px + ox) * cpp + c];
                *o = if q == f32::INFINITY { p.sat } else { q };
            }
        });
    Img { w: cw, h: ch, v }
}

/// The nearest rotation and translation of a sample-unit map, about the area's centre.
fn euclidean_of(h: &Homography, w: u32, hh: u32) -> Transform {
    let (cx, cy) = ((f64::from(w) - 1.0) / 2.0, (f64::from(hh) - 1.0) / 2.0);
    let (qx, qy) = h.apply(cx, cy);
    Transform {
        angle: h.0[3].atan2(h.0[0]),
        tx: qx - cx,
        ty: qy - cy,
        cx,
        cy,
    }
}

/// Stacks the frames of `source` (in focus order; see the module documentation).
/// `progress` gets 0..=1.
pub fn stack(
    source: &dyn FrameSource,
    o: &Options,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<Stacked, Error> {
    let n = source.len();
    if n < 2 {
        return Err(Error::TooFew);
    }
    if n > MAX_FRAMES {
        return Err(Error::TooMany);
    }
    let check = || {
        if cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    };
    let reference = n / 2;

    // 1. Read, normalise, align to the neighbour.
    let first = source.get(0)?;
    let s0 = &first.sensor;
    let (w, h, cpp, crop) = (s0.crop.w, s0.crop.h, s0.cpp, s0.crop);
    let (sw, sh) = (s0.width, s0.height);
    let layout = s0.meta.layout.clone();
    let mosaic = Mosaic::of(&layout, crop.x, crop.y);
    let same = |s: &specular_raw::SensorData| {
        s.width == sw && s.height == sh && s.cpp == cpp && s.crop == crop && s.meta.layout == layout
    };
    let _work = specular_core::budget::reserve(memory_needed(w, h, cpp, o.method))
        .map_err(Error::NeedsMemory)?;
    // Frames are kept when the budget allows, else read again in each pass.
    let keep = specular_core::budget::reserve(kept_bytes(n, w, h, cpp)).ok();
    let mut kept: Vec<Option<Plane>> = Vec::with_capacity(n);
    let block = mosaic.block();
    let mut shots = Vec::with_capacity(n);
    let mut meta = None;
    let mut clip = super::NEAR_CLIP;
    // Frame 0 → frame i, sample units.
    let mut chain = vec![Homography::IDENTITY; n];
    let mut prev: Option<(Proxy, u32)> = None;
    let mut first = Some(first);
    for i in 0..n {
        check()?;
        let f = match first.take() {
            Some(f) => f,
            None => source.get(i)?,
        };
        if !same(&f.sensor) {
            return Err(Error::NotOneCamera);
        }
        if i == reference {
            meta = Some(f.sensor.meta.clone());
        }
        shots.push(f.shot.clone());
        let plane = normalise(&f.sensor);
        drop(f);
        if i == reference {
            clip = plane.sat;
        }
        if o.align {
            let cur = align_proxy(&plane, block);
            if let Some((p, halvings)) = &prev {
                // Neighbours differ in blur, not in size: one ECC from the identity.
                let found =
                    align::ecc(p, &cur.0, Motion::Similarity).ok_or(Error::NotAligned(i))?;
                let in_proxy = align::h_to_samples(&found.h, 1 << halvings);
                let step = align::h_to_samples(&in_proxy, block);
                if step.max_shift(w, h) > f64::from(w.max(h)) / 5.0 {
                    return Err(Error::NotAligned(i));
                }
                chain[i] = step.then_after(&chain[i - 1]);
            }
            prev = Some(cur);
        }
        kept.push(keep.as_ref().map(|_| plane));
        progress(0.3 * (i + 1) as f32 / n as f32);
    }
    drop(prev);
    let meta = meta.ok_or(Error::TooFew)?;
    // Reference → frame i.
    let to_ref = chain[reference]
        .inverse()
        .ok_or(Error::NotAligned(reference))?;
    let maps: Vec<Homography> = chain.iter().map(|c| c.then_after(&to_ref)).collect();
    let warped = |i: usize| -> Result<Plane, Error> {
        let plane = match &kept[i] {
            Some(p) => p.clone(),
            None => normalise(&source.get(i)?.sensor),
        };
        Ok(if maps[i].is_identity(w, h) {
            plane
        } else {
            warp::warp(&plane, &mosaic, &maps[i])
        })
    };

    // 2. Focus measures (and the pyramid's selection).
    let cell = period(&mosaic);
    let (cw, ch) = (w as usize / cell.0, h as usize / cell.1);
    let cells = cw * ch;
    let mut best = vec![f32::NEG_INFINITY; cells];
    let mut second = vec![f32::NEG_INFINITY; cells];
    let mut index = vec![reference as u16; cells];
    let mut guide: Option<Img> = None;
    let levels = blend::levels_for(cw, ch, BANDS);
    let mut sel: Vec<(Vec<u16>, Vec<f32>, usize)> = Vec::new();
    for i in 0..n {
        check()?;
        let p = warped(i)?;
        let y = cell_luma(&p, cell);
        drop(p);
        let f = measure(&y, noise_sigma(&y));
        for k in 0..cells {
            let v = f[k];
            if v > best[k] {
                second[k] = best[k];
                best[k] = v;
                index[k] = i as u16;
            } else if v > second[k] {
                second[k] = v;
            }
        }
        if o.method == Method::Pyramid
            && let Some(filled) = blend::fill(&y)
        {
            let l = blend::laplacian(&filled, levels);
            if sel.is_empty() {
                sel = l
                    .iter()
                    .take(levels - 1)
                    .map(|b| {
                        (
                            vec![0u16; b.w * b.h],
                            vec![f32::NEG_INFINITY; b.w * b.h],
                            b.w,
                        )
                    })
                    .collect();
            }
            for (k, (b, (idx, sal, _))) in l.iter().zip(sel.iter_mut()).enumerate() {
                let s = salience(b);
                for (q, sv) in s.iter().enumerate() {
                    // A frame never wins where it has no picture.
                    let missing = k == 0 && !y.v[q].is_finite();
                    if !missing && *sv > sal[q] {
                        sal[q] = *sv;
                        idx[q] = i as u16;
                    }
                }
            }
        }
        if i == reference {
            guide = Some(y);
        }
        progress(0.3 + 0.35 * (i + 1) as f32 / n as f32);
    }

    // The depth: the sharpest frame per cell, weighted by how clear the choice was,
    // smoothed along the reference's edges.
    let g = guide.unwrap_or_else(|| Img::new(cw, ch, 1.0));
    let gl: Vec<f64> = {
        let finite: Vec<f32> = g.v.iter().copied().filter(|v| v.is_finite()).collect();
        let mean = finite.iter().map(|&v| f64::from(v)).sum::<f64>() / finite.len().max(1) as f64;
        g.v.iter()
            .map(|&v| {
                f64::from(if v.is_finite() { v } else { mean as f32 })
                    .max(1e-6)
                    .log2()
            })
            .collect()
    };
    let conf: Vec<f64> = best
        .iter()
        .zip(&second)
        .map(|(&b, &s)| {
            if !b.is_finite() {
                0.0
            } else if !s.is_finite() {
                f64::from(b)
            } else {
                f64::from(b - s)
            }
        })
        .collect();
    let dc: Vec<f64> = index
        .iter()
        .zip(&conf)
        .map(|(&d, &c)| f64::from(d) * c)
        .collect();
    let r = 4usize.max(cw.min(ch) / 64);
    let num = guided(&gl, &dc, cw, ch, r, 0.01);
    let den = guided(&gl, &conf, cw, ch, r, 0.01);
    let cmax = conf.iter().copied().fold(0.0f64, f64::max);
    let depth: Vec<f32> = num
        .iter()
        .zip(&den)
        .map(|(&a, &b)| {
            if b > 1e-4 * cmax && b > 0.0 {
                (a / b).clamp(0.0, (n - 1) as f64) as f32
            } else {
                reference as f32
            }
        })
        .collect();
    drop((best, second, conf, dc, num, den));

    // 3. The picture.
    let len = (w * h * cpp) as usize;
    let radiance = match o.method {
        Method::DepthMap => {
            let mut acc = vec![0f32; len];
            let mut wsum = vec![0f32; len];
            let mut base: Option<Vec<f32>> = None;
            for i in 0..n {
                check()?;
                let fi = i as f32;
                if i != reference && !depth.iter().any(|d| (d - fi).abs() < 1.0) {
                    continue;
                }
                let p = warped(i)?;
                let row = w as usize * cpp as usize;
                acc.par_chunks_mut(row)
                    .zip(wsum.par_chunks_mut(row))
                    .enumerate()
                    .for_each(|(y, (a, ws))| {
                        let cy = y / cell.1;
                        if cy >= ch {
                            return;
                        }
                        for x in 0..w as usize {
                            let cx = x / cell.0;
                            if cx >= cw {
                                continue;
                            }
                            let wt = (1.0 - (depth[cy * cw + cx] - fi).abs()).max(0.0);
                            if wt <= 0.0 {
                                continue;
                            }
                            for c in 0..cpp as usize {
                                let k = x * cpp as usize + c;
                                let v = p.v[y * row + k];
                                let v = if v == f32::INFINITY { p.sat } else { v };
                                if v.is_finite() {
                                    a[k] += wt * v;
                                    ws[k] += wt;
                                }
                            }
                        }
                    });
                if i == reference {
                    base = Some(p.v);
                }
                progress(0.65 + 0.35 * (i + 1) as f32 / n as f32);
            }
            let base = base.unwrap_or_else(|| vec![0.0; len]);
            acc.par_iter_mut()
                .zip(wsum.par_iter())
                .zip(base.par_iter())
                .for_each(|((a, &ws), &b)| {
                    *a = if ws > 0.0 {
                        *a / ws
                    } else if b.is_finite() {
                        b
                    } else {
                        0.0
                    };
                });
            acc
        }
        Method::Pyramid => {
            let sel: Vec<(Vec<u16>, usize)> = sel
                .into_iter()
                .map(|(idx, _, bw)| {
                    let bh = idx.len() / bw.max(1);
                    (majority(&idx, bw, bh), bw)
                })
                .collect();
            let lattices: Vec<(usize, usize, usize)> = (0..cell.1)
                .flat_map(|oy| {
                    (0..cell.0).flat_map(move |ox| (0..cpp as usize).map(move |c| (ox, oy, c)))
                })
                .collect();
            // Per lattice: the result's bands (the base summed).
            let dims: Vec<(usize, usize)> = std::iter::successors(Some((cw, ch)), |&(a, b)| {
                Some((a.div_ceil(2), b.div_ceil(2)))
            })
            .take(levels)
            .collect();
            let mut out: Vec<Vec<Img>> = lattices
                .iter()
                .map(|_| dims.iter().map(|&(a, b)| Img::new(a, b, 0.0)).collect())
                .collect();
            let base_len = dims[levels - 1].0 * dims[levels - 1].1;
            let mut counts: Vec<Vec<f32>> = vec![vec![0.0; base_len]; lattices.len()];
            let mut base: Option<Vec<f32>> = None;
            for i in 0..n {
                check()?;
                let p = warped(i)?;
                for (li, &lat) in lattices.iter().enumerate() {
                    let img = lattice(&p, cell, lat);
                    let Some(filled) = blend::fill(&img) else {
                        continue;
                    };
                    let l = blend::laplacian(&filled, levels);
                    for k in 0..levels - 1 {
                        let idx = &sel[k].0;
                        let o = &mut out[li][k];
                        for (q, v) in l[k].v.iter().enumerate() {
                            if idx[q] as usize == i {
                                o.v[q] = *v;
                            }
                        }
                    }
                    let last = &l[levels - 1];
                    let o = &mut out[li][levels - 1];
                    for (q, v) in last.v.iter().enumerate() {
                        o.v[q] += v;
                        counts[li][q] += 1.0;
                    }
                }
                if i == reference {
                    base = Some(p.v);
                }
                progress(0.65 + 0.35 * (i + 1) as f32 / n as f32);
            }
            let mut res = base.unwrap_or_else(|| vec![0.0; len]);
            for v in res.iter_mut() {
                if !v.is_finite() {
                    *v = 0.0;
                }
            }
            let row = w as usize * cpp as usize;
            for (li, &(ox, oy, c)) in lattices.iter().enumerate() {
                let bands = &mut out[li];
                let last = levels - 1;
                for (v, k) in bands[last].v.iter_mut().zip(&counts[li]) {
                    *v /= k.max(1.0);
                }
                let img = blend::collapse(bands);
                for cy in 0..ch {
                    for cx in 0..cw {
                        let k = (cy * cell.1 + oy) * row + (cx * cell.0 + ox) * cpp as usize + c;
                        res[k] = img.v[cy * cw + cx].max(0.0);
                    }
                }
            }
            res
        }
    };
    let radiance: Vec<f32> = radiance
        .into_par_iter()
        .map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 })
        .collect();
    drop(kept);
    drop(keep);
    progress(1.0);
    let merged = Merged {
        width: w,
        height: h,
        cpp,
        mosaic,
        radiance,
        clip,
        reference,
        exposures: vec![1.0; n],
        transforms: maps.iter().map(|m| euclidean_of(m, w, h)).collect(),
        warps: maps,
        ghosts: GhostMask::default(),
        meta,
        crop,
        shots,
    };
    let depth = o.depth_map.then_some(DepthMap {
        width: cw as u32,
        height: ch as u32,
        cell: (cell.0 as u32, cell.1 as u32),
        depth,
        frames: n,
    });
    Ok(Stacked { merged, depth })
}

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
