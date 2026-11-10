//! Focus, motion blur and noise measured where they are resolvable: on full-resolution crops.
//!
//! The 256 px signature thumbnail is a ~16:1 reduction of a 12 MP frame: a burst frame
//! blurred by a few pixels looks as crisp there as its sharp neighbours, and the blur, which
//! also smooths the noise, makes it look the cleanest. So the members of similar-shot groups
//! get a second, finer measure: one crop of up to [`CROP`] px at 1:1 at the centre of each
//! cell of the 4×4 grid the thumbnail measures, decoded once per file (JPEG: the row index,
//! then 16 small region decodes, about 1.7× the cost of the thumbnail's 1/8 decode; formats
//! without fast regions: one whole-image decode at the finest scale within a pixel budget).
//!
//! On the crops:
//!
//! - **noise**: Immerkær's estimator per 32 px block; the image's sigma is a low percentile of
//!   the blocks (flat areas show the noise alone; texture only adds to a block's estimate),
//!   leaving out clipped blocks (no noise there to see);
//! - **detail** per cell: RMS of the Laplacian with the noise's share (20 σ² for white noise)
//!   taken out, so a noisy frame does not pass for a sharp one;
//! - **anisotropy**: of the gradient structure tensor over all crops, the noise's (isotropic)
//!   share taken out: the smear of motion blur.
//!
//! [`Focus`] is stored with the file's signature record, so a file is measured once.

use crate::sig::Luma;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster};
use specular_decode::{DecodeRequest, Rect, Registry, Session};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Longest side of a crop (image pixels at the measuring scale).
pub const CROP: u32 = 256;
/// Side of the blocks the noise is estimated on.
const BLOCK: usize = 32;
/// The percentile of block noise estimates taken as the image's noise.
const NOISE_PERCENTILE: f32 = 0.10;
/// Formats without fast region decoding decode the whole image once, at the finest scale with
/// at most this many pixels and within [`WHOLE_MEMORY`].
const WHOLE_PIXELS: u64 = 16_000_000;
const WHOLE_MEMORY: u64 = 512 << 20;

/// Bytes of [`Focus::to_bytes`]: the measures, then [`Focus::version`], [`Focus::decoded`],
/// [`Focus::regions`] and a spare byte.
pub const FOCUS_BYTES: usize = LEGACY_FOCUS_BYTES + 4;
/// Bytes of a [`Focus`] stored before its provenance was (signatures.bin SPSIM2/3).
pub const LEGACY_FOCUS_BYTES: usize = 4 + 4 * 19;

/// The measuring method's version, stored with each [`Focus`]: records of an older version
/// are measured again. 0 is a record stored before versions were (see
/// [`Focus::needs_check`]); bump this when a change to the measure or to how it picks the
/// decode makes earlier records incomparable.
pub const FOCUS_VERSION: u8 = 1;

/// The fine measures of one image.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Focus {
    /// Image pixels per measured pixel (1: full resolution); 0: the image could not be
    /// measured (decode failed or unsupported), so it is not tried again until it changes.
    pub scale: u32,
    /// Noise-compensated RMS of the Laplacian of the crop at the centre of each cell of the
    /// oriented 4×4 grid (row-major, as [`crate::Quality::grid`]).
    pub grid: [f32; 16],
    /// RMS of [`Focus::grid`]: detail over the whole frame.
    pub detail: f32,
    /// Anisotropy of the gradients over all crops (0 every way … 1 one way), noise removed.
    pub anisotropy: f32,
    /// Noise sigma (0–255) at the measuring scale.
    pub noise: f32,
    /// [`FOCUS_VERSION`] when measured; 0: stored before versions were.
    pub version: u8,
    /// The decoder scale the crops came from (0: unknown or not measured), see [`Plan`].
    pub decoded: u8,
    /// Whether the crops were region decodes (else one whole-image decode).
    pub regions: bool,
}

/// How [`measure_session_at`] decodes an image: region decodes or one whole decode, at
/// decoder scale 1/`decode`. The same plan gives the same measures, whichever decoder version
/// (decoders keep their regions identical to the same pixels of their whole decodes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub regions: bool,
    pub decode: u32,
}

impl Focus {
    /// Measured (not a failed attempt).
    pub fn ok(&self) -> bool {
        self.scale > 0
    }

    /// The record of an image that could not be measured.
    pub const UNAVAILABLE: Focus = Focus {
        scale: 0,
        grid: [0.0; 16],
        detail: 0.0,
        anisotropy: 0.0,
        noise: 0.0,
        version: FOCUS_VERSION,
        decoded: 0,
        regions: false,
    };

    /// Detail at `cell` (0–15).
    pub fn at(&self, cell: usize) -> f32 {
        self.grid[cell.min(15)]
    }

    /// Measured by an older version of the method: measure again.
    pub fn outdated(&self) -> bool {
        self.version != 0 && self.version < FOCUS_VERSION
    }

    /// A record of an image's own measure (no reduction) stored before versions were, by a
    /// decoder that may have changed its plan since: camera RAWs, which took one whole decode
    /// until their sessions offered fast regions (and then measure at scale 1 where they
    /// took scale 2 or coarser). Such a record is still right where the plan comes out the
    /// same ([`keep_or_measure`]); a legacy scale-1 record always is (both paths decode at 1).
    pub fn needs_check(&self, raw: bool) -> bool {
        raw && self.version == 0 && self.scale != 1
    }

    /// A legacy measure at a reduced side of a camera RAW: its decode scale is unknown, so it
    /// is measured again (only mixed-size groups have them).
    pub fn legacy_reduced_stale(&self, raw: bool) -> bool {
        raw && self.version == 0
    }

    pub fn to_bytes(&self) -> [u8; FOCUS_BYTES] {
        let mut o = [0u8; FOCUS_BYTES];
        o[..4].copy_from_slice(&self.scale.to_le_bytes());
        let tail = [self.detail, self.anisotropy, self.noise];
        let vals = self.grid.iter().chain(&tail);
        for (k, v) in vals.enumerate() {
            o[4 + 4 * k..8 + 4 * k].copy_from_slice(&v.to_le_bytes());
        }
        let p = LEGACY_FOCUS_BYTES;
        o[p] = self.version;
        o[p + 1] = self.decoded;
        o[p + 2] = u8::from(self.regions);
        o
    }

    /// From [`Focus::to_bytes`], or from the [`LEGACY_FOCUS_BYTES`] of a record stored before
    /// versions were (version 0, provenance unknown).
    pub fn from_bytes(b: &[u8]) -> Option<Focus> {
        if b.len() < LEGACY_FOCUS_BYTES {
            return None;
        }
        let f = |k: usize| f32::from_le_bytes(b[4 + 4 * k..8 + 4 * k].try_into().unwrap_or([0; 4]));
        let mut grid = [0.0f32; 16];
        for (k, g) in grid.iter_mut().enumerate() {
            *g = f(k);
        }
        let prov = b.get(LEGACY_FOCUS_BYTES..FOCUS_BYTES).unwrap_or(&[0; 4]);
        Some(Focus {
            scale: u32::from_le_bytes(b[..4].try_into().ok()?),
            grid,
            detail: f(16),
            anisotropy: f(17),
            noise: f(18),
            version: prov[0],
            decoded: prov[1],
            regions: prov[2] != 0,
        })
    }
}

/// Sums over one crop.
#[derive(Default)]
struct CropStats {
    lap2: f64,
    n: u64,
    gxx: f64,
    gyy: f64,
    gxy: f64,
}

/// Laplacian energy and structure tensor of a luma crop with neighbours `d` px apart, and the
/// Immerkær noise sigma of each of its unclipped blocks (pushed to `blocks`).
fn crop_stats(l: &Luma, d: usize, block: usize, blocks: &mut Vec<f32>) -> CropStats {
    let (w, h) = (l.w as usize, l.h as usize);
    let px = &l.px;
    let mut s = CropStats::default();
    if w < 2 * d + 1 || h < 2 * d + 1 {
        return s;
    }
    let block = block.min(w - 2 * d).min(h - 2 * d).max(1);
    let (bw, bh) = ((w - 2 * d) / block, (h - 2 * d) / block);
    // Per block: sum |Immerkær|, sum of luma, count.
    let mut bsum = vec![(0.0f64, 0.0f64, 0u32); bw * bh];
    let dw = d * w;
    for y in d..h - d {
        let by = (y - d) / block;
        for x in d..w - d {
            let i = y * w + x;
            let c = px[i];
            let (nn, so, e, ww) = (px[i - dw], px[i + dw], px[i + d], px[i - d]);
            let lap = 4.0 * c - nn - so - e - ww;
            s.lap2 += f64::from(lap * lap);
            s.n += 1;
            let gx = f64::from(e - ww) * 0.5;
            let gy = f64::from(so - nn) * 0.5;
            s.gxx += gx * gx;
            s.gyy += gy * gy;
            s.gxy += gx * gy;
            let bx = (x - d) / block;
            if bx < bw && by < bh {
                // Immerkær: [1 -2 1; -2 4 -2; 1 -2 1].
                let m = px[i - w - 1] + px[i - w + 1] + px[i + w - 1] + px[i + w + 1]
                    - 2.0 * (px[i - w] + px[i + w] + px[i + 1] + px[i - 1])
                    + 4.0 * c;
                let b = &mut bsum[by * bw + bx];
                b.0 += f64::from(m.abs());
                b.1 += f64::from(c);
                b.2 += 1;
            }
        }
    }
    let k = (std::f64::consts::FRAC_PI_2).sqrt() / 6.0;
    for &(m, sum, n) in &bsum {
        if n == 0 {
            continue;
        }
        let n = f64::from(n);
        // Clipped areas hold no noise to see.
        if (8.0..=247.0).contains(&(sum / n)) {
            blocks.push((m / n * k) as f32);
        }
    }
    s
}

/// The stats of each crop and the noise sigma over them: a low percentile of the block
/// estimates (flat areas show the noise alone; texture only adds to a block's estimate).
fn band(crops: &[Option<Luma>], d: usize) -> (Vec<Option<CropStats>>, f32) {
    let mut blocks = Vec::new();
    let stats = crops
        .iter()
        .map(|c| c.as_ref().map(|l| crop_stats(l, d, BLOCK, &mut blocks)))
        .collect();
    blocks.sort_by(f32::total_cmp);
    let sigma = match blocks.len() {
        0 => 0.0,
        n => blocks[((n - 1) as f32 * NOISE_PERCENTILE).round() as usize],
    };
    (stats, sigma)
}

/// 3×3 binomial smoothing (the inner `w-2`×`h-2`).
fn smooth(l: &Luma) -> Luma {
    let (sw, sh) = (l.w as usize, l.h as usize);
    if sw < 3 || sh < 3 {
        return Luma {
            w: 0,
            h: 0,
            px: Vec::new(),
        };
    }
    let (w, h) = (sw - 2, sh - 2);
    let p = &l.px;
    let mut px = Vec::with_capacity(w * h);
    for y in 1..=h {
        for x in 1..=w {
            let i = y * sw + x;
            let row = |j: usize| p[j - 1] + 2.0 * p[j] + p[j + 1];
            px.push((row(i - sw) + 2.0 * row(i) + row(i + sw)) / 16.0);
        }
    }
    Luma {
        w: w as u32,
        h: h as u32,
        px,
    }
}

/// Neighbour distance of the detail band.
const DETAIL_STEP: usize = 2;

/// How much white noise of unit variance adds, through [`smooth`] and the operators at
/// [`DETAIL_STEP`], to the mean squared Laplacian and to each axis's squared gradient.
fn noise_gains() -> (f64, f64) {
    const N: usize = 15;
    let c = N / 2;
    let mut imp = vec![0f32; N * N];
    imp[c * N + c] = 1.0;
    let s = smooth(&Luma {
        w: N as u32,
        h: N as u32,
        px: imp,
    });
    let (w, d) = (N - 2, DETAIL_STEP);
    let (mut lap, mut grad) = (0f64, 0f64);
    for y in d..w - d {
        for x in d..w - d {
            let i = y * w + x;
            let p = &s.px;
            let l = 4.0 * p[i] - p[i - d * w] - p[i + d * w] - p[i - d] - p[i + d];
            let g = 0.5 * (p[i + d] - p[i - d]);
            lap += f64::from(l * l);
            grad += f64::from(g * g);
        }
    }
    (lap, grad)
}

/// The measures from the crops of the 16 cells (display order); `None` crops are missing.
///
/// Noise is measured at full resolution. Detail and anisotropy are measured on the crops
/// smoothed 3×3 with operators 2 px wide: shift-invariant, still telling blur of a pixel or
/// two, and passing about a tenth of the noise's power, so the noise (which the camera's and
/// the codec's filtering colour, so its share can only be estimated) hardly counts; what is
/// left of it is taken out by the operators' gain on white noise of the measured sigma.
pub fn focus_of(crops: &[Option<Luma>; 16], scale: u32) -> Focus {
    let (_, noise) = band(crops, 1);
    let smoothed: Vec<Option<Luma>> = crops.iter().map(|c| c.as_ref().map(smooth)).collect();
    let (stats, _) = band(&smoothed, DETAIL_STEP);
    let (lap_gain, grad_gain) = noise_gains();
    let n2 = f64::from(noise) * f64::from(noise);
    let mut f = Focus {
        scale: scale.max(1),
        noise,
        ..Focus::default()
    };
    let (mut gxx, mut gyy, mut gxy, mut n) = (0.0f64, 0.0f64, 0.0f64, 0u64);
    let mut sum2 = 0.0f64;
    let mut cells = 0u32;
    for (k, s) in stats.iter().enumerate() {
        let Some(s) = s else { continue };
        if s.n == 0 {
            continue;
        }
        let e = (s.lap2 / s.n as f64 - lap_gain * n2).max(0.0);
        f.grid[k] = e.sqrt() as f32;
        sum2 += e;
        cells += 1;
        gxx += s.gxx;
        gyy += s.gyy;
        gxy += s.gxy;
        n += s.n;
    }
    f.detail = (sum2 / f64::from(cells.max(1))).sqrt() as f32;
    let gxx = (gxx - n as f64 * n2 * grad_gain).max(0.0);
    let gyy = (gyy - n as f64 * n2 * grad_gain).max(0.0);
    let tr = gxx + gyy;
    f.anisotropy = if tr > 1e-9 {
        (((gxx - gyy).powi(2) + 4.0 * gxy * gxy).sqrt() / tr).min(1.0) as f32
    } else {
        0.0
    };
    f
}

/// The stored-image cell (index into a 4×4 grid over the stored pixels) under each displayed
/// cell, for an EXIF orientation (flip first, then quarter turns clockwise, as the view).
fn stored_cells(o: Orientation) -> [usize; 16] {
    let (q, flip) = o.to_rotation_flip();
    let mut out = [0usize; 16];
    for sc in 0..16usize {
        let (mut u, mut v) = (((sc % 4) as f32 + 0.5) / 4.0, ((sc / 4) as f32 + 0.5) / 4.0);
        if flip {
            u = 1.0 - u;
        }
        for _ in 0..q {
            (u, v) = (1.0 - v, u);
        }
        let dc = ((v * 4.0) as usize).min(3) * 4 + ((u * 4.0) as usize).min(3);
        out[dc] = sc;
    }
    out
}

/// The crop rectangle (scaled stored coordinates) at the centre of stored cell `sc` of a
/// `w`×`h` image.
fn crop_rect(sc: usize, w: u32, h: u32) -> Rect {
    let (cw, ch) = (w / 4, h / 4);
    let side_w = CROP.min(cw).max(1);
    let side_h = CROP.min(ch).max(1);
    let (cx, cy) = ((sc % 4) as u32, (sc / 4) as u32);
    let x = (cx * w / 4 + cw / 2).saturating_sub(side_w / 2);
    let y = (cy * h / 4 + ch / 2).saturating_sub(side_h / 2);
    Rect {
        x: x.min(w.saturating_sub(side_w)),
        y: y.min(h.saturating_sub(side_h)),
        w: side_w.min(w),
        h: side_h.min(h),
    }
}

/// The luma of `want` (scaled image coordinates) cut from `r` (which covers it, from its
/// origin).
fn luma_of(r: &Raster, want: Rect) -> Option<Luma> {
    let (ox, oy) = r.origin;
    if want.x < ox
        || want.y < oy
        || want.x + want.w > ox + r.width
        || want.y + want.h > oy + r.height
        || r.planes.is_empty()
    {
        return None;
    }
    let (x0, y0) = ((want.x - ox) as usize, (want.y - oy) as usize);
    let (w, h) = (want.w as usize, want.h as usize);
    match r.layout {
        // Luma as coded.
        PixelLayout::Gray | PixelLayout::YCbCr => {
            let p = &r.planes[0];
            let mut px = Vec::with_capacity(w * h);
            for y in y0..y0 + h {
                let row = p.data.get(y * p.stride + x0..y * p.stride + x0 + w)?;
                px.extend(row.iter().map(|&v| f32::from(v)));
            }
            Some(Luma {
                w: want.w,
                h: want.h,
                px,
            })
        }
        // 8-bit YUV (lossy WebP, video): luma brought to 0..255.
        PixelLayout::Yuv(e) if e.bits <= 8 => {
            let ([yo, ys], _) = e.levels();
            let p = &r.planes[0];
            let mut px = Vec::with_capacity(w * h);
            for y in y0..y0 + h {
                let row = p.data.get(y * p.stride + x0..y * p.stride + x0 + w)?;
                px.extend(row.iter().map(|&v| (f32::from(v) - yo) * ys * 255.0));
            }
            Some(Luma {
                w: want.w,
                h: want.h,
                px,
            })
        }
        _ => {
            // Anything else through the thumbnail path's conversion (RGB, alpha, wide).
            let sub = sub_raster(r, x0, y0, w, h)?;
            let px =
                specular_thumbs::pixels::thumbnail(&sub, Orientation::NORMAL, want.w.max(want.h));
            Some(Luma::from_rgba(px.width, px.height, &px.rgba))
        }
    }
}

/// A copy of part of a raster whose planes are full size (no subsampling).
fn sub_raster(r: &Raster, x0: usize, y0: usize, w: usize, h: usize) -> Option<Raster> {
    let mut planes = Vec::with_capacity(r.planes.len());
    for p in &r.planes {
        if p.h_div > 1 || p.v_div > 1 {
            return None;
        }
        let bpp = p.bytes_per_pixel();
        let mut data = Vec::with_capacity(w * h * bpp);
        for y in y0..y0 + h {
            let a = y * p.stride + x0 * bpp;
            data.extend_from_slice(p.data.get(a..a + w * bpp)?);
        }
        planes.push(specular_core::RasterPlane {
            width: w as u32,
            height: h as u32,
            stride: w * bpp,
            data,
            h_div: 1,
            v_div: 1,
        });
    }
    Some(Raster {
        width: w as u32,
        height: h as u32,
        origin: (0, 0),
        scale: r.scale,
        layout: r.layout,
        planes,
        truncated: r.truncated,
    })
}

/// Measures the image at `path`. `None` when cancelled; [`Focus::UNAVAILABLE`] when it
/// cannot be decoded.
pub fn measure(registry: &Registry, path: &Path, cancel: &AtomicBool) -> Option<Focus> {
    measure_at(registry, path, None, cancel)
}

/// [`measure`], or with `side`, at the scale where the image's longer side is `side` px: the
/// crops then cover the same share of the frame, at the same sampling, as the full-resolution
/// crops of a `side`-px image (a smaller member of its group), so the two compare.
pub fn measure_at(
    registry: &Registry,
    path: &Path,
    side: Option<u32>,
    cancel: &AtomicBool,
) -> Option<Focus> {
    keep_or_measure(registry, path, side, None, 1, cancel)
}

/// [`measure_at`], unless `legacy` (a record from before versions, of the image's own measure
/// with `side` None) is what the measure would give now: then that, tagged with today's
/// [`Plan`]. A legacy record took one whole decode at its scale (or failed: scale 0), so it
/// stands where today's plan decodes at the same scale (regions are identical to the same
/// pixels of the whole decode) or, failed, where no plan is left. `threads`: as
/// [`measure_session_with`].
pub fn keep_or_measure(
    registry: &Registry,
    path: &Path,
    side: Option<u32>,
    legacy: Option<Focus>,
    threads: usize,
    cancel: &AtomicBool,
) -> Option<Focus> {
    let Ok(s) = registry.open_path(path) else {
        return Some(Focus::UNAVAILABLE);
    };
    if let Some(old) = legacy.filter(|_| side.is_none()) {
        let plan = plan(&*s, None);
        let same = match plan {
            Some(p) => old.ok() && p.decode == old.scale,
            None => !old.ok(),
        };
        if same {
            return Some(Focus {
                version: FOCUS_VERSION,
                decoded: plan.map_or(0, |p| p.decode.min(255) as u8),
                regions: plan.is_some_and(|p| p.regions),
                ..old
            });
        }
    }
    measure_session_with(&*s, side, threads, cancel)
}

/// [`measure`] on an open session.
pub fn measure_session(s: &dyn Session, cancel: &AtomicBool) -> Option<Focus> {
    measure_session_at(s, None, cancel)
}

/// Reductions above this are not measured at a common scale (the crops would need much of
/// the image decoded); such groups compare on their thumbnails.
pub const MAX_REDUCTION: f64 = 8.0;

/// Where one crop comes from at a reduction: the crop in the reduced image, the decoded
/// pixels that cover it (coordinates at the decode's scale), and where the crop starts in
/// those, with `k` decoded pixels per crop pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reduced {
    target: Rect,
    source: Rect,
    off: (f64, f64),
    k: f64,
}

/// The crop of stored cell `sc` of a `w`×`h` image reduced `r` times (to about `w / r` ×
/// `h / r`), from pixels decoded at 1/`scale` (`sw`×`sh` of them).
fn reduced_crop(
    sc: usize,
    (w, h): (u32, u32),
    r: f64,
    scale: u32,
    (sw, sh): (u32, u32),
) -> Reduced {
    let tw = ((f64::from(w) / r).round() as u32).max(1);
    let th = ((f64::from(h) / r).round() as u32).max(1);
    let target = crop_rect(sc, tw, th);
    let k = r / f64::from(scale.max(1));
    let (fx0, fy0) = (f64::from(target.x) * k, f64::from(target.y) * k);
    let fx1 = (f64::from(target.x + target.w) * k).min(f64::from(sw));
    let fy1 = (f64::from(target.y + target.h) * k).min(f64::from(sh));
    let x0 = (fx0.floor() as u32).min(sw.saturating_sub(1));
    let y0 = (fy0.floor() as u32).min(sh.saturating_sub(1));
    let x1 = (fx1.ceil() as u32).clamp(x0 + 1, sw.max(x0 + 1));
    let y1 = (fy1.ceil() as u32).clamp(y0 + 1, sh.max(y0 + 1));
    Reduced {
        target,
        source: Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        },
        off: (fx0 - f64::from(x0), fy0 - f64::from(y0)),
        k,
    }
}

/// The weights of one axis of [`area_reduce`]: per output pixel, its first input pixel and
/// the share of each input pixel from there.
fn area_weights(n_in: usize, off: f64, k: f64, n_out: usize) -> Vec<(usize, Vec<f32>)> {
    (0..n_out)
        .map(|i| {
            let a = (off + i as f64 * k).min(n_in as f64);
            let b = (off + (i + 1) as f64 * k).min(n_in as f64).max(a);
            let first = (a.floor() as usize).min(n_in.saturating_sub(1));
            let mut ws = Vec::new();
            let mut j = first;
            while (j as f64) < b && j < n_in {
                let lo = a.max(j as f64);
                let hi = b.min((j + 1) as f64);
                ws.push((hi - lo).max(0.0) as f32);
                j += 1;
            }
            let sum: f32 = ws.iter().sum();
            if sum > 0.0 {
                ws.iter_mut().for_each(|v| *v /= sum);
            } else {
                ws = vec![1.0];
            }
            (first, ws)
        })
        .collect()
}

/// Area-weighted reduction of `l` to `w`×`h`: output pixel `i` averages the input over
/// `[off + i·k, off + (i + 1)·k)` along each axis (clamped to the input).
fn area_reduce(l: &Luma, off: (f64, f64), k: f64, w: u32, h: u32) -> Luma {
    let (iw, ih) = (l.w as usize, l.h as usize);
    let (ow, oh) = (w as usize, h as usize);
    if iw == 0 || ih == 0 || ow == 0 || oh == 0 {
        return Luma {
            w: 0,
            h: 0,
            px: Vec::new(),
        };
    }
    let xs = area_weights(iw, off.0, k, ow);
    let ys = area_weights(ih, off.1, k, oh);
    let mut rows = vec![0f32; ow * ih];
    for y in 0..ih {
        let src = &l.px[y * iw..(y + 1) * iw];
        for (x, (first, ws)) in xs.iter().enumerate() {
            rows[y * ow + x] = ws
                .iter()
                .enumerate()
                .map(|(t, w)| w * src[(first + t).min(iw - 1)])
                .sum();
        }
    }
    let mut px = vec![0f32; ow * oh];
    for (y, (first, ws)) in ys.iter().enumerate() {
        for (t, wt) in ws.iter().enumerate() {
            let row = &rows[(first + t).min(ih - 1) * ow..][..ow];
            for (o, v) in px[y * ow..(y + 1) * ow].iter_mut().zip(row) {
                *o += wt * v;
            }
        }
    }
    Luma { w, h, px }
}

/// The crop `c` cut from `img` and reduced to its target size.
fn reduced_luma(img: &Raster, c: &Reduced) -> Option<Luma> {
    let l = luma_of(img, c.source)?;
    if c.k == 1.0 && c.off == (0.0, 0.0) && (l.w, l.h) == (c.target.w, c.target.h) {
        return Some(l);
    }
    Some(area_reduce(&l, c.off, c.k, c.target.w, c.target.h))
}

/// The reduction for `side` (1: full resolution); `None` above [`MAX_REDUCTION`].
fn reduction(long: u32, side: Option<u32>) -> Option<f64> {
    let r = match side {
        Some(sd) if sd > 0 && long > sd => f64::from(long) / f64::from(sd),
        _ => 1.0,
    };
    (r <= MAX_REDUCTION).then_some(r)
}

/// The native scales of `info` (1 when it lists none).
fn scales_of(info: &specular_decode::ImageInfo) -> &[u32] {
    if info.scales.is_empty() {
        &[1]
    } else {
        info.scales
    }
}

/// How [`measure_session_at`] would decode `s` for `side`, without decoding: region decodes
/// at the coarsest native scale with every crop pixel still there, or one whole decode at
/// the finest scale within the budget. `None`: not measurable (too small, too large a
/// reduction or over the whole decode's budget).
pub fn plan(s: &dyn Session, side: Option<u32>) -> Option<Plan> {
    let info = s.info();
    if info.width < 8 || info.height < 8 {
        return None;
    }
    let r = reduction(info.width.max(info.height), side)?;
    let scales = scales_of(info);
    if info.fast_regions {
        // The decoder does most of the reduction, the area filter the rest.
        let decode = scales
            .iter()
            .copied()
            .filter(|&sc| f64::from(sc.max(1)) <= r)
            .max()
            .unwrap_or(1)
            .max(1);
        return Some(Plan {
            regions: true,
            decode,
        });
    }
    let px = u64::from(info.width) * u64::from(info.height);
    let decode = scales
        .iter()
        .copied()
        .find(|&sc| {
            let sc64 = u64::from(sc.max(1));
            px / (sc64 * sc64) <= WHOLE_PIXELS
                && s.decode_memory(&DecodeRequest::full(sc)) <= WHOLE_MEMORY
        })?
        .max(1);
    // Reduced: the decode must not be coarser than the reduction (3 % allowed).
    if r > 1.0 && f64::from(decode) > r * 1.03 {
        return None;
    }
    Some(Plan {
        regions: false,
        decode,
    })
}

/// [`measure_at`] on an open session.
pub fn measure_session_at(
    s: &dyn Session,
    side: Option<u32>,
    cancel: &AtomicBool,
) -> Option<Focus> {
    measure_session_with(s, side, 1, cancel)
}

/// Most threads one measure's crops are decoded on ([`measure_session_with`]).
pub const MAX_CROP_THREADS: usize = 8;

/// [`measure_session_at`] on up to `threads` threads: with more than one, the crops' region
/// decodes run side by side (each serial: the cheapest way to spread them, a RAW crop
/// being a tile or two that its decoder's own threads hardly split) and a whole decode may
/// use the decoder's threads. The same measures either way.
pub fn measure_session_with(
    s: &dyn Session,
    side: Option<u32>,
    threads: usize,
    cancel: &AtomicBool,
) -> Option<Focus> {
    let info = s.info().clone();
    let Some(plan) = plan(s, side) else {
        return Some(Focus::UNAVAILABLE);
    };
    let tag = |f: Focus| Focus {
        version: FOCUS_VERSION,
        decoded: plan.decode.min(255) as u8,
        regions: plan.regions,
        ..f
    };
    let long = info.width.max(info.height);
    let r = reduction(long, side).unwrap_or(1.0);
    let token = CancelToken::new();
    let cells = stored_cells(info.orientation);
    let mut crops: [Option<Luma>; 16] = Default::default();
    let cancelled = || cancel.load(Ordering::Relaxed);
    // Reduced measures report the reduction, rounded (their side is kept beside them).
    let reduced_scale = (r.round() as u32).max(1);
    let scale = plan.decode;
    if plan.regions {
        if s.prepare_regions(&token).is_err() {
            return if cancelled() {
                None
            } else {
                Some(Focus::UNAVAILABLE)
            };
        }
        let (sw, sh) = (info.width.div_ceil(scale), info.height.div_ceil(scale));
        let crop = |dc: usize| {
            let c = reduced_crop(cells[dc], (info.width, info.height), r, scale, (sw, sh));
            let req = DecodeRequest {
                scale,
                region: Some(c.source),
                parallel: false,
                prepare_regions: false,
            };
            s.decode(&req, &token)
                .ok()
                .and_then(|img| reduced_luma(&img, &c))
        };
        let next = std::sync::atomic::AtomicUsize::new(0);
        // Takes cells until none is left; what it decoded, by cell.
        let worker = || {
            let mut got = Vec::new();
            loop {
                let dc = next.fetch_add(1, Ordering::Relaxed);
                if dc >= 16 || cancelled() {
                    break;
                }
                got.push((dc, crop(dc)));
            }
            got
        };
        let k = threads.clamp(1, MAX_CROP_THREADS);
        let got: Vec<(usize, Option<Luma>)> = if k == 1 {
            worker()
        } else {
            std::thread::scope(|sc| {
                let others: Vec<_> = (1..k)
                    .map(|wi| specular_thread::spawn_scoped(sc, format!("sim-luma-{wi}"), worker))
                    .collect();
                let mut got = worker();
                for h in others {
                    got.extend(h.join().unwrap_or_default());
                }
                got
            })
        };
        if cancelled() {
            return None;
        }
        for (dc, l) in got {
            crops[dc] = l;
        }
        if crops.iter().all(Option::is_none) {
            return Some(Focus::UNAVAILABLE);
        }
        return Some(tag(focus_of(&crops, reduced_scale)));
    }
    // One whole-image decode at the finest scale within the budget.
    if cancelled() {
        return None;
    }
    let mut req = DecodeRequest::full(scale);
    req.parallel = threads > 1;
    let img = match s.decode(&req, &token) {
        Ok(img) => img,
        Err(_) if cancelled() => return None,
        Err(_) => return Some(Focus::UNAVAILABLE),
    };
    if r == 1.0 {
        for (dc, &sc) in cells.iter().enumerate() {
            crops[dc] = luma_of(&img, crop_rect(sc, img.width, img.height));
        }
        if crops.iter().all(Option::is_none) {
            return Some(Focus::UNAVAILABLE);
        }
        return Some(tag(focus_of(&crops, scale)));
    }
    let rr = r.max(f64::from(scale));
    for (dc, &sc) in cells.iter().enumerate() {
        let c = reduced_crop(
            sc,
            (info.width, info.height),
            rr,
            scale,
            (img.width, img.height),
        );
        crops[dc] = reduced_luma(&img, &c);
    }
    if crops.iter().all(Option::is_none) {
        return Some(Focus::UNAVAILABLE);
    }
    Some(tag(focus_of(&crops, reduced_scale)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let f = Focus {
            scale: 2,
            grid: std::array::from_fn(|k| k as f32 * 1.5),
            detail: 3.0,
            anisotropy: 0.25,
            noise: 2.5,
            version: FOCUS_VERSION,
            decoded: 2,
            regions: true,
        };
        assert_eq!(Focus::from_bytes(&f.to_bytes()), Some(f));
        // A record from before versions: the measures, version 0, provenance unknown.
        let old = Focus::from_bytes(&f.to_bytes()[..LEGACY_FOCUS_BYTES]).unwrap();
        let measures = Focus {
            version: 0,
            decoded: 0,
            regions: false,
            ..f
        };
        assert_eq!(old, measures);
        assert!(old.needs_check(true) && !old.needs_check(false) && !old.outdated());
        // Scale 1 decodes at 1 either way.
        assert!(!Focus { scale: 1, ..old }.needs_check(true));
        assert!(!f.needs_check(true) && !f.outdated() && !f.legacy_reduced_stale(true));
        assert!(old.legacy_reduced_stale(true) && !old.legacy_reduced_stale(false));
    }

    #[test]
    fn orientation_maps_cells() {
        // Normal: identity.
        let id = stored_cells(Orientation::NORMAL);
        assert_eq!(id, std::array::from_fn(|k| k));
        // A quarter turn clockwise (6): the displayed top-left shows the stored bottom-left.
        let r = stored_cells(Orientation(6));
        assert_eq!(r[0], 12);
        assert_eq!(r[3], 0);
        // A horizontal flip (2): the displayed top-left is the stored top-right.
        assert_eq!(stored_cells(Orientation(2))[0], 3);
        for v in 1..=8 {
            let mut c = stored_cells(Orientation(v)).to_vec();
            c.sort_unstable();
            assert_eq!(c, (0..16).collect::<Vec<_>>(), "orientation {v}");
        }
    }

    #[test]
    fn crops_fit_the_image() {
        for (w, h) in [(4000, 3000), (300, 200), (17, 9)] {
            for sc in 0..16 {
                let r = crop_rect(sc, w, h);
                assert!(r.x + r.w <= w && r.y + r.h <= h, "{w}x{h} {sc}: {r:?}");
                assert!(r.w > 0 && r.h > 0);
            }
        }
    }

    fn luma(w: u32, h: u32, f: impl Fn(u32, u32) -> f32) -> Luma {
        Luma {
            w,
            h,
            px: (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| f(x, y))
                .collect(),
        }
    }

    #[test]
    fn area_reduce_averages_what_each_pixel_covers() {
        // 2:1 from the origin: plain 2×2 means.
        let l = luma(4, 4, |x, y| (x + 10 * y) as f32);
        let r = area_reduce(&l, (0.0, 0.0), 2.0, 2, 2);
        assert_eq!(r.px, vec![5.5, 7.5, 25.5, 27.5]);
        // 1.5:1 with an offset: fractional weights, and a ramp stays a ramp (the mean of a
        // linear function over each span).
        let l = luma(10, 3, |x, _| x as f32);
        let r = area_reduce(&l, (0.25, 0.0), 1.5, 6, 2);
        for (i, v) in r.px[..6].iter().enumerate() {
            let (a, b) = (0.25 + 1.5 * i as f32, 0.25 + 1.5 * (i + 1) as f32);
            // Pixel x covers [x, x + 1) with value x: the mean over [a, b) is (a + b - 1) / 2.
            assert!((v - (a + b - 1.0) / 2.0).abs() < 1e-4, "{i}: {v}");
        }
        // A constant stays constant, edges clamped.
        let l = luma(7, 5, |_, _| 3.0);
        let r = area_reduce(&l, (0.5, 0.5), 2.5, 3, 2);
        assert!(r.px.iter().all(|&v| (v - 3.0).abs() < 1e-5), "{:?}", r.px);
    }

    #[test]
    fn reduced_crops_cover_the_same_share_of_the_frame() {
        let (w, h) = (4000, 3000);
        // No reduction: the full-resolution crops, untouched.
        for sc in 0..16 {
            let c = reduced_crop(sc, (w, h), 1.0, 1, (w, h));
            assert_eq!(
                (c.target, c.source, c.off, c.k),
                (crop_rect(sc, w, h), c.target, (0.0, 0.0), 1.0)
            );
        }
        // 4000 → 1000 decoded at 1/2 (2000×1500): 2 decoded pixels per crop pixel, the crop
        // where a 1000×750 image's would be.
        for sc in 0..16 {
            let c = reduced_crop(sc, (w, h), 4.0, 2, (2000, 1500));
            assert_eq!(c.target, crop_rect(sc, 1000, 750));
            assert_eq!(c.k, 2.0);
            assert_eq!((c.source.x, c.source.y), (c.target.x * 2, c.target.y * 2));
            assert_eq!((c.source.w, c.source.h), (c.target.w * 2, c.target.h * 2));
            assert!(c.source.x + c.source.w <= 2000 && c.source.y + c.source.h <= 1500);
        }
        // 3:1 decoded at 1/2: 1.5 per crop pixel, a fractional start kept as the offset.
        let c = reduced_crop(5, (w, h), 3.0, 2, (2000, 1500));
        assert_eq!(c.k, 1.5);
        let fx = f64::from(c.target.x) * 1.5;
        assert_eq!(f64::from(c.source.x) + c.off.0, fx);
        assert!(f64::from(c.source.x + c.source.w) >= fx + f64::from(c.target.w) * 1.5);
    }
}
