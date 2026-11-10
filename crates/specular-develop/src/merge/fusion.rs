//! Exposure brackets of non-raw photos (JPEG, HEIC, TIFF): already developed pictures, so
//! the merge works on their display-encoded values ([`super::rgb::CodeImage`]).
//!
//! 1. **Pre-alignment**: Ward's median threshold bitmaps ("Fast, robust image registration
//!    for compositing high dynamic range photographs from hand-held exposures", JGT 2003):
//!    each picture's grey thresholded at its own median is nearly the same bitmap whatever
//!    the exposure; pixels near the median are excluded (they flip with noise); the shift
//!    with the fewest differing bits is found coarse to fine, ±1 pixel per level.
//! 2. **Refinement**: the lane's ECC ([`super::align::ecc_from`]) from that shift, on the
//!    log-luma proxy of the linearised values, a homography by default (hand-held), the
//!    frame then resampled into the reference's geometry ([`super::warp::warp`]).
//! 3. **Exposure fusion** (the default output; Mertens, Kautz & Van Reeth, "Exposure
//!    fusion", Pacific Graphics 2007): per pixel and frame a weight, contrast (|Laplacian|
//!    of grey) × saturation (standard deviation of R, G, B) × well-exposedness (a Gaussian
//!    of σ 0.2 around 0.5, per channel, multiplied), each to its exponent; the weights
//!    normalised over the frames and the pictures blended band by band (Laplacian pyramids
//!    under Gaussian pyramids of the weights, [`super::blend`]). The result is a display
//!    picture, nothing to tone map.
//! 4. **Radiance** (optional): the camera's response recovered by Debevec & Malik
//!    ("Recovering high dynamic range radiance maps from photographs", SIGGRAPH 1997: the
//!    least-squares system over sampled pixels with a hat weight and a smoothness term) or
//!    by Robertson, Borman & Stevenson ("Estimation-theoretic approach to dynamic range
//!    enhancement using multiple exposures", JEI 2003: alternating radiance and response
//!    estimates with a Gaussian weight), then each pixel's radiance as the weighted mean of
//!    `ln f(Z) − ln t` over the frames; written as OpenEXR.

// Small systems and per-channel loops read clearest with indices.
#![allow(clippy::needless_range_loop)]

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use specular_core::CancelToken;

use super::align::{self, Motion};
use super::blend::{self, Img};
use super::detect::Shot;
use super::homography::{Homography, cholesky_solve};
use super::rgb::{self, CodeImage, RgbFormat, RgbImage};
use super::{Error, MAX_FRAMES, Mosaic, Plane};

/// How the camera's response is recovered for the radiance output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Response {
    /// Debevec & Malik: `samples` pixels on a grid, smoothness `lambda`.
    Debevec { samples: usize, lambda: f32 },
    /// Robertson et al.: at most `iterations`, stopping when the response moves less
    /// than `threshold` on average.
    Robertson { iterations: usize, threshold: f32 },
}

impl Default for Response {
    fn default() -> Self {
        Response::Debevec {
            samples: 70,
            lambda: 10.0,
        }
    }
}

/// How to merge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FusionOptions {
    /// What alignment may move ([`Motion::None`]: a tripod).
    pub motion: Motion,
    /// Exponents of contrast, saturation and well-exposedness.
    pub weights: [f32; 3],
    /// Also recover the response and the radiance (the EXR output).
    pub radiance: Option<Response>,
    /// The frame the others align to; `None` picks the middle exposure.
    pub reference: Option<usize>,
}

impl Default for FusionOptions {
    fn default() -> Self {
        FusionOptions {
            motion: Motion::Homography,
            weights: [1.0, 1.0, 1.0],
            radiance: None,
            reference: None,
        }
    }
}

/// A camera response per channel: the linear value of each code 0…255 (1 at code 128).
pub type Curves = [Vec<f32>; 3];

/// A merged bracket of non-raw photos.
#[derive(Clone, Debug)]
pub struct Fused {
    /// The exposure fusion.
    pub image: RgbImage,
    /// Radiance in linear Rec.2020, the reference frame's exposure at 1.
    pub radiance: Option<RgbImage>,
    pub response: Option<Curves>,
    pub reference: usize,
    /// Each frame's alignment (reference → frame), in pixels.
    pub warps: Vec<Homography>,
    pub shots: Vec<Shot>,
}

/// Bytes the fusion of `frames` pictures of `w × h` holds at its peak.
pub fn memory_needed(frames: usize, w: u32, h: u32) -> u64 {
    let n = u64::from(w) * u64::from(h);
    // The frames (three floats a pixel), the result's pyramids (three channels, 4/3), a
    // frame's channel and weight pyramids, and the radiance.
    n * 4 * (3 * frames as u64 + 3 * 4 / 3 * 2 + 3 + 3) + n * 4 * 3
}

/// Grey of display values (Ward's 54/183/19 weights).
fn grey(c: &[f32]) -> f32 {
    (54.0 * c[0] + 183.0 * c[1] + 19.0 * c[2]) / 256.0
}

/// `img` reduced by `b × b` box means to grey.
fn grey_reduced(img: &CodeImage, b: usize) -> Img {
    let (w, h) = (img.w as usize / b, img.h as usize / b);
    let iw = img.w as usize;
    let mut v = vec![0f32; w * h];
    v.par_chunks_mut(w.max(1)).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for dy in 0..b {
                for dx in 0..b {
                    let i = ((y * b + dy) * iw + x * b + dx) * 3;
                    s += grey(&img.rgb[i..i + 3]);
                }
            }
            *o = s / (b * b) as f32;
        }
    });
    Img { w, h, v }
}

fn median(v: &[f32]) -> f32 {
    let mut s: Vec<f32> = v.to_vec();
    if s.is_empty() {
        return 0.5;
    }
    let n = s.len() / 2;
    *s.select_nth_unstable_by(n, f32::total_cmp).1
}

/// The threshold and exclusion bitmaps of a grey image.
fn bitmaps(g: &Img) -> (Vec<bool>, Vec<bool>) {
    let m = median(&g.v);
    let tol = 4.0 / 255.0;
    (
        g.v.iter().map(|&v| v > m).collect(),
        g.v.iter().map(|&v| (v - m).abs() > tol).collect(),
    )
}

/// Ward's shift (sx, sy) with `frame(x + s) ≈ reference(x)`, in pixels of `a` and `b`
/// (same size), searched over `levels` halvings.
pub fn mtb_shift(a: &Img, b: &Img, levels: usize) -> (i32, i32) {
    let mut pa = vec![a.clone()];
    let mut pb = vec![b.clone()];
    while pa.len() < levels && pa.last().is_some_and(|i| i.w.min(i.h) >= 32) {
        let na = blend::down(pa.last().unwrap_or(a));
        let nb = blend::down(pb.last().unwrap_or(b));
        pa.push(na);
        pb.push(nb);
    }
    let (mut sx, mut sy) = (0i32, 0i32);
    for l in (0..pa.len()).rev() {
        let (ga, gb) = (&pa[l], &pb[l]);
        let (ta, ea) = bitmaps(ga);
        let (tb, eb) = bitmaps(gb);
        let (w, h) = (ga.w as i32, ga.h as i32);
        let (cx, cy) = (sx * 2, sy * 2);
        let mut best = (u64::MAX, cx, cy);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (ox, oy) = (cx + dx, cy + dy);
                let err: u64 = (0..h)
                    .into_par_iter()
                    .map(|y| {
                        let yb = y + oy;
                        if yb < 0 || yb >= h {
                            return 0;
                        }
                        let mut e = 0u64;
                        for x in 0..w {
                            let xb = x + ox;
                            if xb < 0 || xb >= w {
                                continue;
                            }
                            let (i, j) = ((y * w + x) as usize, (yb * w + xb) as usize);
                            if ea[i] && eb[j] && ta[i] != tb[j] {
                                e += 1;
                            }
                        }
                        e
                    })
                    .sum();
                if err < best.0 {
                    best = (err, ox, oy);
                }
            }
        }
        sx = best.1;
        sy = best.2;
        if l == 0 {
            break;
        }
    }
    (sx, sy)
}

/// The display values through a pure 2.2 power as a merge plane, clipped codes marked
/// saturated. The camera's curve is unknown; a pure power keeps the log of a pixel an
/// exposure offset plus a scale (which the correlation coefficient ignores) all the way
/// down, where sRGB's linear toe would bend the dark frames.
fn linear_plane(img: &CodeImage) -> Plane {
    Plane {
        w: img.w,
        h: img.h,
        cpp: 3,
        v: img
            .rgb
            .par_iter()
            .map(|&c| {
                if c >= 0.985 {
                    f32::INFINITY
                } else {
                    c.max(0.0).powf(2.2)
                }
            })
            .collect(),
        sat: 0.985f32.powf(2.2),
    }
}

/// The proxy block for a `w × h` picture: a long side of about 1536 at most.
fn block_for(w: u32, h: u32) -> u32 {
    w.max(h).div_ceil(1536).max(1)
}

/// The alignment of `frame` to `reference` (reference → frame, pixels); `None` when it
/// runs off by more than a fifth of the picture.
pub fn align(reference: &CodeImage, frame: &CodeImage, motion: Motion) -> Option<Homography> {
    if motion == Motion::None {
        return Some(Homography::IDENTITY);
    }
    let b = block_for(reference.w, reference.h);
    let (ga, gb) = (
        grey_reduced(reference, b as usize),
        grey_reduced(frame, b as usize),
    );
    let (sx, sy) = mtb_shift(&ga, &gb, 6);
    let init = Homography::translation(f64::from(sx), f64::from(sy));
    // Below about code 20 quantisation is most of the signal.
    let dark = 0.004;
    let (pt, pi) = (
        align::proxy(&linear_plane(reference), b, dark),
        align::proxy(&linear_plane(frame), b, dark),
    );
    let limit = f64::from(reference.w.max(reference.h)) / 5.0;
    let ok = |h: &Homography| h.is_finite() && h.max_shift(reference.w, reference.h) <= limit;
    // Far from the bitmaps' shift (proxy pixels): ECC slipped to another optimum.
    let reach = 4.0 + 0.02 * pt.w.max(pt.h) as f64;
    let accept = |f: &align::Found| {
        let hs = align::h_to_samples(&f.h, b);
        ok(&hs) && f.rho > 0.5 && f.h.distance(&init, pt.w as u32, pt.h as u32) < reach
    };
    // Rotation and translation first; a richer model only when it fits clearly better
    // (with clipped and dark areas left out, a homography easily bends to the noise).
    let rigid_motion = match motion {
        Motion::Translation => Motion::Translation,
        _ => Motion::Euclidean,
    };
    let rigid = align::ecc_from(&pt, &pi, rigid_motion, &init).filter(accept);
    let rich = (motion.params() > rigid_motion.params())
        .then(|| align::ecc_from(&pt, &pi, motion, &rigid.map_or(init, |r| r.h)))
        .flatten()
        .filter(accept);
    let best = match (rigid, rich) {
        (Some(r), Some(h)) if (1.0 - h.rho) < 0.8 * (1.0 - r.rho) => Some(h),
        (Some(r), _) => Some(r),
        (None, h) => h,
    };
    if let Some(f) = best {
        return Some(align::h_to_samples(&f.h, b));
    }
    let hs = align::h_to_samples(&init, b);
    ok(&hs).then_some(hs)
}

/// The level below at `w × h`: the twice-size expansion, cropped when `w` or `h` is odd
/// (so an odd edge sample sees the same neighbours as in the even size, as OpenCV's
/// pyramids do; [`blend::up`] mirrors at the odd edge instead).
fn expand(a: &Img, w: usize, h: usize) -> Img {
    let (ew, eh) = (2 * a.w, 2 * a.h);
    if (ew, eh) == (w, h) {
        return blend::up(a, w, h);
    }
    let e = blend::up(a, ew, eh);
    let mut v = Vec::with_capacity(w * h);
    for y in 0..h {
        v.extend_from_slice(&e.v[y * ew..y * ew + w]);
    }
    Img { w, h, v }
}

/// The Laplacian pyramid of `a` with `n` levels, expanded by [`expand`].
fn laplacian(a: &Img, n: usize) -> Vec<Img> {
    let g = blend::gaussian(a, n);
    let mut l = Vec::with_capacity(n);
    for k in 0..n - 1 {
        let e = expand(&g[k + 1], g[k].w, g[k].h);
        let mut b = g[k].clone();
        b.v.par_iter_mut()
            .zip(e.v.par_iter())
            .for_each(|(x, y)| *x -= y);
        l.push(b);
    }
    l.push(g[n - 1].clone());
    l
}

/// The image of a pyramid from [`laplacian`].
fn collapse(l: &[Img]) -> Img {
    let mut a = l[l.len() - 1].clone();
    for k in (0..l.len() - 1).rev() {
        let mut e = expand(&a, l[k].w, l[k].h);
        e.v.par_iter_mut()
            .zip(l[k].v.par_iter())
            .for_each(|(x, y)| *x += y);
        a = e;
    }
    a
}

/// The weights of Mertens et al. for one frame (unnormalised).
fn weights_of(img: &CodeImage, e: [f32; 3]) -> Vec<f32> {
    let (w, h) = (img.w as usize, img.h as usize);
    let g: Vec<f32> = img
        .rgb
        .chunks_exact(3)
        .map(|c| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2])
        .collect();
    let mirror = |i: i64, n: usize| -> usize {
        if n == 1 {
            return 0;
        }
        let i = if i < 0 { -i } else { i };
        let i = i as usize;
        if i >= n { 2 * (n - 1) - i } else { i }
    };
    let mut out = vec![0f32; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            let at = |dx: i64, dy: i64| g[mirror(y as i64 + dy, h) * w + mirror(x as i64 + dx, w)];
            let contrast = (at(-1, 0) + at(1, 0) + at(0, -1) + at(0, 1) - 4.0 * at(0, 0)).abs();
            let c = &img.rgb[(y * w + x) * 3..][..3];
            let mu = (c[0] + c[1] + c[2]) / 3.0;
            let sat =
                (((c[0] - mu).powi(2) + (c[1] - mu).powi(2) + (c[2] - mu).powi(2)) / 3.0).sqrt();
            let mut wexp = 1.0f32;
            for v in c {
                wexp *= (-(v - 0.5).powi(2) / (2.0 * 0.2 * 0.2)).exp();
            }
            *o = contrast.powf(e[0]) * sat.powf(e[1]) * wexp.powf(e[2]) + 1e-12;
        }
    });
    out
}

/// Exposure fusion of aligned frames (same size, display values); `None` when cancelled.
pub fn mertens(frames: &[CodeImage], e: [f32; 3], cancel: &CancelToken) -> Option<CodeImage> {
    let first = frames.first()?;
    let (w, h) = (first.w as usize, first.h as usize);
    let levels = ((w.min(h) as f32).ln() / 2f32.ln()) as usize + 1;
    let mut sum = vec![0f32; w * h];
    for f in frames {
        let wt = weights_of(f, e);
        sum.par_iter_mut()
            .zip(wt.par_iter())
            .for_each(|(s, v)| *s += v);
    }
    let mut out: Vec<Vec<Img>> = Vec::new();
    for f in frames {
        if cancel.is_cancelled() {
            return None;
        }
        let wt = weights_of(f, e);
        let norm = Img {
            w,
            h,
            v: wt.iter().zip(&sum).map(|(a, s)| a / s).collect(),
        };
        let gw = blend::gaussian(&norm, levels);
        for c in 0..3 {
            let ch = Img {
                w,
                h,
                v: f.rgb.iter().skip(c).step_by(3).copied().collect(),
            };
            let l = laplacian(&ch, levels);
            if out.len() <= c {
                out.push(l.iter().map(|b| Img::new(b.w, b.h, 0.0)).collect());
            }
            for (k, band) in l.iter().enumerate() {
                let acc = &mut out[c][k];
                acc.v
                    .par_iter_mut()
                    .zip(band.v.par_iter().zip(gw[k].v.par_iter()))
                    .for_each(|(a, (b, g))| *a += b * g);
            }
        }
    }
    let chans: Vec<Img> = out.iter().map(|p| collapse(p)).collect();
    let mut rgb = vec![0f32; w * h * 3];
    rgb.par_chunks_mut(3).enumerate().for_each(|(i, o)| {
        for c in 0..3 {
            o[c] = chans[c].v[i];
        }
    });
    Some(CodeImage {
        w: first.w,
        h: first.h,
        rgb,
    })
}

#[inline]
fn code(v: f32) -> usize {
    (v * 255.0).round().clamp(0.0, 255.0) as usize
}

/// The hat weight of Debevec & Malik (never 0, so every row counts).
#[inline]
fn hat(z: usize) -> f64 {
    if z <= 127 {
        z as f64 + 1.0
    } else {
        256.0 - z as f64
    }
}

/// Grid positions for `samples` samples over `w × h`.
fn grid(w: usize, h: usize, samples: usize) -> Vec<(usize, usize)> {
    let xs = ((samples as f64 * w as f64 / h as f64).sqrt().round() as usize).max(1);
    let ys = (samples / xs).max(1);
    let (sx, sy) = (w / xs, h / ys);
    let mut out = Vec::with_capacity(xs * ys);
    for j in 0..ys {
        for i in 0..xs {
            out.push(((sx / 2 + i * sx).min(w - 1), (sy / 2 + j * sy).min(h - 1)));
        }
    }
    out
}

/// Debevec & Malik's response from frames with exposure times `times` (see the module
/// documentation): per channel the linear value of each code, 1 at code 128.
pub fn debevec(frames: &[CodeImage], times: &[f64], samples: usize, lambda: f32) -> Curves {
    let Some(first) = frames.first() else {
        return std::array::from_fn(|_| vec![1.0; 256]);
    };
    let (w, h) = (first.w as usize, first.h as usize);
    let pos: Vec<(usize, usize)> = grid(w, h, samples)
        .into_iter()
        .filter(|&(x, y)| {
            frames
                .iter()
                .all(|f| f.rgb[(y * w + x) * 3..][..3].iter().all(|v| v.is_finite()))
        })
        .collect();
    let lambda = f64::from(lambda);
    std::array::from_fn(|c| {
        let n = 256 + pos.len();
        let mut ata = vec![0f64; n * n];
        let mut atb = vec![0f64; n];
        let mut row = |idx: &[(usize, f64)], b: f64| {
            for &(i, a) in idx {
                atb[i] += a * b;
                for &(j, bj) in idx {
                    ata[i * n + j] += a * bj;
                }
            }
        };
        for (k, &(x, y)) in pos.iter().enumerate() {
            for (f, &t) in frames.iter().zip(times) {
                let z = code(f.rgb[(y * w + x) * 3 + c]);
                let wz = hat(z);
                row(&[(z, wz), (256 + k, -wz)], wz * t.ln());
            }
        }
        row(&[(128, 1.0)], 0.0);
        for z in 1..255 {
            let s = lambda * hat(z);
            row(&[(z - 1, s), (z, -2.0 * s), (z + 1, s)], 0.0);
        }
        // Tiny ridge for codes no sample shows (the smoothness rows still tie them).
        for i in 0..n {
            ata[i * n + i] += 1e-10;
        }
        match cholesky_solve(&ata, &atb, n) {
            Some(x) => (0..256).map(|z| x[z].exp() as f32).collect(),
            None => (0..256).map(|z| z as f32 / 128.0).collect(),
        }
    })
}

/// Robertson et al.'s weight; 0 at the clipped ends (a clipped code says only "at least"
/// or "at most", and weighting it biases the radiance).
#[inline]
fn gauss_weight(z: usize) -> f64 {
    if z == 0 || z == 255 {
        return 0.0;
    }
    let d = z as f64 - 127.5;
    (-4.0 * d * d / (127.5 * 127.5)).exp()
}

/// The response with the codes no pixel showed (and the clipped ends) interpolated from
/// their neighbours in log, held at the nearest seen beyond the last one.
fn fill_unseen(f: &mut [f64], seen: &[bool]) {
    let known: Vec<usize> = (1..255).filter(|&z| seen[z] && f[z] > 0.0).collect();
    let (Some(&lo), Some(&hi)) = (known.first(), known.last()) else {
        return;
    };
    let mut k = 0;
    for z in 0..256 {
        if z <= lo || z >= hi {
            f[z] = f[if z <= lo { lo } else { hi }];
            continue;
        }
        while known[k + 1] < z {
            k += 1;
        }
        let (a, b) = (known[k], known[k + 1]);
        if z == a || z == b {
            continue;
        }
        let t = (z - a) as f64 / (b - a) as f64;
        f[z] = (f[a].ln() * (1.0 - t) + f[b].ln() * t).exp();
    }
}

/// Robertson, Borman & Stevenson's response (see the module documentation), over every
/// pixel (every `stride`-th in each direction).
pub fn robertson(
    frames: &[CodeImage],
    times: &[f64],
    iterations: usize,
    threshold: f32,
    stride: usize,
) -> Curves {
    let Some(first) = frames.first() else {
        return std::array::from_fn(|_| vec![1.0; 256]);
    };
    let (w, h) = (first.w as usize, first.h as usize);
    let s = stride.max(1);
    let idx: Vec<usize> = (0..h)
        .step_by(s)
        .flat_map(|y| (0..w).step_by(s).map(move |x| y * w + x))
        .collect();
    std::array::from_fn(|c| {
        let z: Vec<Vec<u8>> = frames
            .iter()
            .map(|f| idx.iter().map(|&i| code(f.rgb[i * 3 + c]) as u8).collect())
            .collect();
        let mut f: Vec<f64> = (0..256).map(|v| v as f64 / 128.0).collect();
        for _ in 0..iterations {
            // Radiance from the response.
            let e: Vec<f64> = (0..idx.len())
                .into_par_iter()
                .map(|p| {
                    let (mut num, mut den) = (0.0, 0.0);
                    for (j, &t) in times.iter().enumerate() {
                        let zz = z[j][p] as usize;
                        let wz = gauss_weight(zz);
                        num += wz * t * f[zz];
                        den += wz * t * t;
                    }
                    if den > 0.0 { num / den } else { f64::NAN }
                })
                .collect();
            // Response from the radiance.
            let mut sum = [0f64; 256];
            let mut cnt = [0usize; 256];
            for (j, &t) in times.iter().enumerate() {
                for (p, &zz) in z[j].iter().enumerate() {
                    if e[p].is_finite() {
                        sum[zz as usize] += t * e[p];
                        cnt[zz as usize] += 1;
                    }
                }
            }
            let mut next = f.clone();
            for m in 0..256 {
                if cnt[m] > 0 {
                    next[m] = sum[m] / cnt[m] as f64;
                }
            }
            let seen: Vec<bool> = cnt.iter().map(|&c| c > 0).collect();
            fill_unseen(&mut next, &seen);
            let mid = next[128];
            if mid > 0.0 {
                for v in &mut next {
                    *v /= mid;
                }
            }
            let diff = next.iter().zip(&f).map(|(a, b)| (a - b).abs()).sum::<f64>() / 256.0;
            f = next;
            if diff < f64::from(threshold) {
                break;
            }
        }
        f.iter().map(|&v| v as f32).collect()
    })
}

/// Each pixel's radiance (sRGB primaries, relative to exposure 1) from aligned frames and
/// a response; NaN-free frames assumed where finite.
pub fn merge_radiance(frames: &[CodeImage], times: &[f64], response: &Curves) -> Vec<f32> {
    let Some(first) = frames.first() else {
        return Vec::new();
    };
    let n = first.rgb.len();
    let ln: [Vec<f64>; 3] = std::array::from_fn(|c| {
        response[c]
            .iter()
            .map(|&v| f64::from(v.max(1e-12)).ln())
            .collect()
    });
    let mut out = vec![0f32; n];
    out.par_iter_mut().enumerate().for_each(|(i, o)| {
        let c = i % 3;
        let (mut num, mut den) = (0.0, 0.0);
        for (f, &t) in frames.iter().zip(times) {
            let v = f.rgb[i];
            if !v.is_finite() {
                continue;
            }
            let z = code(v);
            let wz = hat(z);
            num += wz * (ln[c][z] - t.ln());
            den += wz;
        }
        *o = if den > 0.0 {
            (num / den).exp() as f32
        } else {
            0.0
        };
    });
    out
}

/// The middle exposure (by metadata; the first when unknown).
fn reference_of(shots: &[Shot]) -> usize {
    super::default_reference(shots)
}

/// Merges a bracket of non-raw photos (see the module documentation). `progress` gets
/// 0..=1.
pub fn fuse(
    frames: Vec<(CodeImage, Shot)>,
    o: &FusionOptions,
    progress: &(dyn Fn(f32) + Sync),
    cancel: &CancelToken,
) -> Result<Fused, Error> {
    let n = frames.len();
    if n < 2 {
        return Err(Error::TooFew);
    }
    if n > MAX_FRAMES {
        return Err(Error::TooMany);
    }
    let (w, h) = (frames[0].0.w, frames[0].0.h);
    if frames.iter().any(|f| f.0.w != w || f.0.h != h) {
        return Err(Error::NotOneCamera);
    }
    let _mem =
        specular_core::budget::reserve(memory_needed(n, w, h)).map_err(Error::NeedsMemory)?;
    let check = || {
        if cancel.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    };
    let shots: Vec<Shot> = frames.iter().map(|f| f.1.clone()).collect();
    let reference = o
        .reference
        .filter(|&r| r < n)
        .unwrap_or_else(|| reference_of(&shots));
    let times: Option<Vec<f64>> = if o.radiance.is_some() {
        Some(
            shots
                .iter()
                .map(Shot::exposure)
                .collect::<Option<Vec<f64>>>()
                .ok_or(Error::NoExposures)?,
        )
    } else {
        None
    };
    let mut images: Vec<CodeImage> = frames.into_iter().map(|f| f.0).collect();
    let mut warps = vec![Homography::IDENTITY; n];
    for i in 0..n {
        check()?;
        if i == reference {
            continue;
        }
        let hs = align(&images[reference], &images[i], o.motion).ok_or(Error::NotAligned(i))?;
        warps[i] = hs;
        if !hs.is_identity(w, h) {
            let p = super::warp::warp(&rgb::codes_to_plane(&images[i]), &Mosaic::Linear, &hs);
            // Outside the frame: the reference shows (its weight decides nothing there).
            let r = &images[reference].rgb;
            let v =
                p.v.iter()
                    .zip(r)
                    .map(|(&a, &b)| if a.is_finite() { a.clamp(0.0, 1.0) } else { b })
                    .collect();
            images[i] = CodeImage { w, h, rgb: v };
        }
        progress(0.4 * (i + 1) as f32 / n as f32);
    }
    check()?;
    let fusedc = mertens(&images, o.weights, cancel).ok_or(Error::Cancelled)?;
    progress(0.8);
    let mut image = fusedc.to_linear();
    image.linear.iter_mut().for_each(|v| *v = v.max(0.0));
    let (radiance, response) = match (o.radiance, &times) {
        (Some(r), Some(t)) => {
            check()?;
            let curves = match r {
                Response::Debevec { samples, lambda } => debevec(&images, t, samples, lambda),
                Response::Robertson {
                    iterations,
                    threshold,
                } => {
                    // About a million pixels are plenty for the curve.
                    let stride =
                        ((f64::from(w) * f64::from(h) / 1e6).sqrt().ceil() as usize).max(1);
                    robertson(&images, t, iterations, threshold, stride)
                }
            };
            let lin = merge_radiance(&images, t, &curves);
            let mut m = super::rgb::RgbImage {
                w,
                h,
                linear: lin.iter().map(|v| v * t[reference] as f32).collect(),
                alpha: None,
            };
            // sRGB primaries → Rec.2020.
            let k = crate::export::Space::Srgb.to_rec2020();
            m.linear.par_chunks_mut(3).for_each(|p| {
                let v = [p[0], p[1], p[2]];
                for r in 0..3 {
                    p[r] = (0..3).map(|c| k[r][c] as f32 * v[c]).sum();
                }
            });
            (Some(m), Some(curves))
        }
        _ => (None, None),
    };
    progress(1.0);
    Ok(Fused {
        image,
        radiance,
        response,
        reference,
        warps,
        shots,
    })
}

/// Writes the fusion as `<reference>-HDR.<fmt>` beside the reference, and the radiance as
/// `<reference>-HDR.exr` when there is one; the paths written.
pub fn save(
    f: &Fused,
    fmt: RgbFormat,
    reference_bytes: Option<&[u8]>,
) -> std::io::Result<Vec<PathBuf>> {
    use super::dng::{Kind, file_name_of, write_beside};
    let r: &Path = &f.shots[f.reference].path;
    let other = |e: String| std::io::Error::other(e);
    let bytes = rgb::encode(&f.image, fmt, reference_bytes).map_err(other)?;
    let mut out = vec![write_beside(
        r,
        &file_name_of(r, Kind::Hdr, fmt.ext()),
        &bytes,
    )?];
    if let Some(m) = &f.radiance {
        let bytes = rgb::encode_exr(m).map_err(other)?;
        out.push(write_beside(r, &file_name_of(r, Kind::Hdr, "exr"), &bytes)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
