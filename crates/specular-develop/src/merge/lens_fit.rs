//! The reference's own lens corrections (maker notes: Sony, Fujifilm) as DNG opcodes for the
//! merged DNG, which has no maker notes.
//!
//! Develop corrects such a raw with [`Radial`]: per colour plane a back-map factor `F(ρ)` at
//! the output radius (distortion, colour fringes and the fill zoom, lens card defaults) and a
//! vignetting gain at the source radius. DNG 1.4 `WarpRectilinear` back-maps with
//! `kr0 + kr1 r² + kr2 r⁴ + kr3 r⁶` of the radius normalised by the centre's distance to the
//! farthest corner: with the centre in the middle that is the half diagonal, Develop's `ρ`.
//! So each plane's `F` is fitted by least squares in those four terms, reweighted towards the
//! smallest largest error (Lawson), with the corner pinned, so the corners land within a
//! fraction of a pixel. `FixVignetteRadial` (`1 + k0 r² … + k4 r¹⁰`)
//! follows the warp, so it is fitted to the gain at the source radius the warp reads,
//! `G(ρ · F_green(ρ))`. Sony's vignetting is not applied by Develop either (see
//! [`crate::ops::geometry::lens`]), so it is not written.

use specular_raw::sensor::LensCorrection;

use crate::ops::geometry::lens::{MakerModel, Manual, Radial};

/// Samples over `0..=1` the fits use.
const SAMPLES: usize = 512;
/// Extra weight of the corner sample (pins the fit there).
const CORNER_WEIGHT: f64 = 1e4;
/// Reweighting rounds towards the smallest largest error.
const LAWSON: usize = 60;

/// A fit of one raw's lens tables (see the module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct LensFit {
    /// `WarpRectilinear` radial terms per plane (red, green, blue), no tangential terms.
    pub warp: Option<[[f64; 4]; 3]>,
    /// `FixVignetteRadial` terms.
    pub vignette: Option<[f64; 5]>,
    /// Largest position error of the warp over the frame, pixels (all planes).
    pub max_px: f64,
    /// Position error at the corner, pixels (all planes).
    pub corner_px: f64,
    /// Largest relative error of the vignetting gain.
    pub gain_err: f64,
}

/// Develop's radial model of `lens` on a `w`×`h` frame with the lens card's defaults
/// (distortion, colour and vignetting on, filled).
pub fn develop_radial(lens: &LensCorrection, w: f64, h: f64) -> Radial {
    let mut r = Radial {
        maker: Some(MakerModel::new(lens, w, h)),
        use_distortion: true,
        use_ca: true,
        use_vignetting: true,
        manual: Manual::default(),
        zoom: 1.0,
    };
    if !r.is_identity() {
        r.zoom = r.fill_zoom(w, h);
    }
    r
}

/// Solves the `n`×`n` system `a x = b` (Gaussian elimination, partial pivoting).
fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for c in 0..N {
        let p = (c..N).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..N {
            let f = a[r][c] / a[c][c];
            let pivot = a[c];
            for (v, pv) in a[r].iter_mut().zip(pivot).skip(c) {
                *v -= f * pv;
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; N];
    for r in (0..N).rev() {
        let s: f64 = (r + 1..N).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// Fits `y(ρ) ≈ Σ c_i · basis_i(ρ)` close to minimax (Lawson's iteratively reweighted
/// least squares from the weights `weight`), the corner pinned; the coefficients with the
/// smallest largest error (scaled by `scale(ρ)`) over the samples.
fn fit<const N: usize>(
    basis: impl Fn(f64) -> [f64; N],
    y: impl Fn(f64) -> f64,
    weight: impl Fn(f64) -> f64,
    scale: impl Fn(f64) -> f64,
) -> Option<[f64; N]> {
    let rho: Vec<f64> = (0..=SAMPLES).map(|i| i as f64 / SAMPLES as f64).collect();
    let fs: Vec<[f64; N]> = rho.iter().map(|&r| basis(r)).collect();
    let ys: Vec<f64> = rho.iter().map(|&r| y(r)).collect();
    let mut w: Vec<f64> = rho.iter().map(|&r| weight(r)).collect();
    let mut best: Option<([f64; N], f64)> = None;
    for _ in 0..LAWSON {
        let mut a = [[0.0; N]; N];
        let mut b = [0.0; N];
        for i in 0..=SAMPLES {
            let wi = w[i] + if i == SAMPLES { CORNER_WEIGHT } else { 0.0 };
            for r in 0..N {
                for c in 0..N {
                    a[r][c] += wi * fs[i][r] * fs[i][c];
                }
                b[r] += wi * fs[i][r] * ys[i];
            }
        }
        let c = solve(a, b)?;
        let e: Vec<f64> = (0..=SAMPLES)
            .map(|i| {
                let v: f64 = (0..N).map(|k| c[k] * fs[i][k]).sum();
                ((v - ys[i]) * scale(rho[i])).abs()
            })
            .collect();
        let worst = e.iter().copied().fold(0.0, f64::max);
        if best.as_ref().is_none_or(|b| worst < b.1) {
            best = Some((c, worst));
        }
        // Lawson: weights grow where the error is large.
        let sum: f64 = w.iter().zip(&e).map(|(w, e)| w * e).sum();
        if sum <= 0.0 {
            break;
        }
        for (wi, ei) in w.iter_mut().zip(&e) {
            *wi *= ei / sum * (SAMPLES + 1) as f64;
            *wi = wi.max(1e-12);
        }
    }
    best.map(|b| b.0)
}

/// The DNG opcodes for `lens` on a `w`×`h` frame, `None` when nothing applies.
pub fn fit_lens(lens: &LensCorrection, w: u32, h: u32) -> Option<LensFit> {
    let (fw, fh) = (f64::from(w), f64::from(h));
    if fw < 2.0 || fh < 2.0 {
        return None;
    }
    let radial = develop_radial(lens, fw, fh);
    if radial.is_identity() {
        return None;
    }
    let half_diag = (fw * fw + fh * fh).sqrt() / 2.0;
    let mm = radial.maker.as_ref()?;
    let moves = (mm.has_distortion() || mm.has_ca()) || radial.zoom != 1.0;
    let (mut warp, mut max_px, mut corner_px) = (None, 0.0f64, 0.0f64);
    if moves {
        let mut k = [[0.0; 4]; 3];
        for (p, kp) in k.iter_mut().enumerate() {
            // In displacement terms (ρ·F), weighted by ρ (the ring's pixels).
            *kp = fit(
                |r: f64| {
                    let r2 = r * r;
                    [r, r * r2, r * r2 * r2, r * r2 * r2 * r2]
                },
                |r| r * radial.factor(p, r),
                |r| r + 1e-3,
                |_| 1.0,
            )?;
        }
        for (p, kp) in k.iter().enumerate() {
            for i in 0..=SAMPLES {
                let r = i as f64 / SAMPLES as f64;
                let r2 = r * r;
                let f = kp[0] + r2 * (kp[1] + r2 * (kp[2] + r2 * kp[3]));
                let e = (r * (f - radial.factor(p, r))).abs() * half_diag;
                max_px = max_px.max(e);
                if i == SAMPLES {
                    corner_px = corner_px.max(e);
                }
            }
        }
        warp = Some(k);
    }
    let (mut vignette, mut gain_err) = (None, 0.0f64);
    if mm.has_vignetting() {
        // The gain where the warp reads (green plane).
        let target = |r: f64| radial.gain(r * radial.factor(1, r));
        let v = fit(
            |r: f64| {
                let r2 = r * r;
                [r2, r2 * r2, r2 * r2 * r2, r2.powi(4), r2.powi(5)]
            },
            |r| target(r) - 1.0,
            |r| r + 1e-3,
            |r| 1.0 / target(r),
        )?;
        for i in 0..=SAMPLES {
            let r = i as f64 / SAMPLES as f64;
            let r2 = r * r;
            let g = 1.0 + r2 * (v[0] + r2 * (v[1] + r2 * (v[2] + r2 * (v[3] + r2 * v[4]))));
            let t = target(r);
            gain_err = gain_err.max(((g - t) / t).abs());
        }
        vignette = Some(v);
    }
    (warp.is_some() || vignette.is_some()).then_some(LensFit {
        warp,
        vignette,
        max_px,
        corner_px,
        gain_err,
    })
}
