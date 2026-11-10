//! One star measured: an elliptical Moffat profile with a flat background fitted by
//! Levenberg–Marquardt to the pixels around it (neighbouring stars masked out), giving the
//! FWHM along both axes, the eccentricity and the angle; and the half-flux diameter from the
//! pixels themselves (each split into 4 × 4 parts so small stars are not quantised).
//!
//! The model is `B + A · (1 + qa·dx² + 2·qb·dx·dy + qc·dy²)^(−β)`. Along an axis whose
//! quadratic-form eigenvalue is λ, the half-maximum radius is `sqrt((2^(1/β) − 1) / λ)`; a
//! Gaussian is the limit of large β.

// Small dense linear algebra reads best indexed.
#![allow(clippy::needless_range_loop)]

use crate::Image;
use crate::background::Background;
use crate::detect::Cand;

/// Parameters: B, A, x0, y0, qa, qb, qc, β.
const NP: usize = 8;
const MAX_ITER: usize = 25;
/// β is kept within these (a Gaussian is β → ∞; past 60 the two cannot be told apart).
const BETA_MIN: f64 = 1.05;
const BETA_MAX: f64 = 60.0;

/// One measured star. Positions are in pixels, (0, 0) the centre of the first pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    pub x: f32,
    pub y: f32,
    /// The fitted peak above the background, and the background.
    pub amplitude: f32,
    pub background: f32,
    /// The light within the aperture above the fitted background.
    pub flux: f32,
    /// FWHM along the major and minor axes and their geometric mean (px).
    pub fwhm_major: f32,
    pub fwhm_minor: f32,
    pub fwhm: f32,
    /// `sqrt(1 − (minor / major)²)`: 0 round, towards 1 a line.
    pub eccentricity: f32,
    /// The major axis' angle from +x towards +y (radians, −π/2..π/2).
    pub angle: f32,
    pub beta: f32,
    /// Half-flux diameter (px): the diameter of the circle about the centre holding half of
    /// the aperture's light.
    pub hfd: f32,
    /// Peak over the noise.
    pub snr: f32,
}

/// What the fit needs of the frame.
pub(crate) struct Ctx<'a> {
    pub img: &'a Image<'a>,
    pub bg: &'a Background,
    /// Every candidate, for masking a neighbour's light out of a window.
    pub all: &'a [Cand],
    /// The candidates' positions by grid cell (`cell` px), as indices into `all`.
    pub grid: &'a [Vec<u32>],
    pub cell: usize,
    pub grid_w: usize,
}

impl Ctx<'_> {
    /// The candidates near (`x`, `y`) within `r` px, but `me`.
    fn neighbours(&self, me: usize, x: f32, y: f32, r: f32) -> Vec<(f32, f32, f32)> {
        let mut out = Vec::new();
        let c = self.cell as f32;
        let gx0 = ((x - r) / c).floor().max(0.0) as usize;
        let gx1 = ((x + r) / c).floor().max(0.0) as usize;
        let gy0 = ((y - r) / c).floor().max(0.0) as usize;
        let gy1 = ((y + r) / c).floor().max(0.0) as usize;
        for gy in gy0..=gy1 {
            for gx in gx0..=gx1.min(self.grid_w - 1) {
                let Some(list) = self.grid.get(gy * self.grid_w + gx) else {
                    continue;
                };
                for &i in list {
                    let i = i as usize;
                    if i == me {
                        continue;
                    }
                    let o = &self.all[i];
                    if (o.cx - x).abs() <= r && (o.cy - y).abs() <= r {
                        // Its core: a little more than its half-maximum radius.
                        out.push((o.cx, o.cy, (o.sigma * 3.0).max(2.5)));
                    }
                }
            }
        }
        out
    }
}

/// A pixel of the window: offset from the window's corner and value.
struct Px {
    x: f64,
    y: f64,
    v: f64,
}

/// Solves `a · x = b` (n × n, row-major) by Gaussian elimination with partial pivoting.
fn solve(a: &mut [[f64; NP]; NP], b: &mut [f64; NP]) -> Option<[f64; NP]> {
    for col in 0..NP {
        let piv = (col..NP).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[piv][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for row in col + 1..NP {
            let f = a[row][col] / a[col][col];
            if f == 0.0 {
                continue;
            }
            for k in col..NP {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; NP];
    for row in (0..NP).rev() {
        let mut s = b[row];
        for k in row + 1..NP {
            s -= a[row][k] * x[k];
        }
        x[row] = s / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

fn valid(p: &[f64; NP]) -> bool {
    p[1] > 0.0
        && p[4] > 0.0
        && p[6] > 0.0
        && p[4] * p[6] - p[5] * p[5] > 0.0
        && (BETA_MIN..=BETA_MAX).contains(&p[7])
        && p.iter().all(|v| v.is_finite())
}

/// The squared residuals, and with `jac` the normal equations, at `p`.
fn system(px: &[Px], p: &[f64; NP], jac: Option<(&mut [[f64; NP]; NP], &mut [f64; NP])>) -> f64 {
    let [b, a, x0, y0, qa, qb, qc, beta] = *p;
    let mut chi = 0.0;
    match jac {
        None => {
            for q in px {
                let (dx, dy) = (q.x - x0, q.y - y0);
                let u = 1.0 + qa * dx * dx + 2.0 * qb * dx * dy + qc * dy * dy;
                let m = b + a * (-beta * u.ln()).exp();
                chi += (q.v - m) * (q.v - m);
            }
        }
        Some((jtj, jtr)) => {
            *jtj = [[0.0; NP]; NP];
            *jtr = [0.0; NP];
            for q in px {
                let (dx, dy) = (q.x - x0, q.y - y0);
                let u = 1.0 + qa * dx * dx + 2.0 * qb * dx * dy + qc * dy * dy;
                let lu = u.ln();
                let pw = (-beta * lu).exp();
                let m = b + a * pw;
                let r = q.v - m;
                chi += r * r;
                let dm_du = -a * beta * pw / u;
                let g = [
                    1.0,
                    pw,
                    -dm_du * (2.0 * qa * dx + 2.0 * qb * dy),
                    -dm_du * (2.0 * qb * dx + 2.0 * qc * dy),
                    dm_du * dx * dx,
                    dm_du * 2.0 * dx * dy,
                    dm_du * dy * dy,
                    -a * pw * lu,
                ];
                for i in 0..NP {
                    jtr[i] += g[i] * r;
                    for j in 0..=i {
                        jtj[i][j] += g[i] * g[j];
                    }
                }
            }
            for i in 0..NP {
                for j in i + 1..NP {
                    jtj[i][j] = jtj[j][i];
                }
            }
        }
    }
    chi
}

/// Fits candidate `me` of `ctx.all`; `None` when the fit fails or the result is not a star.
pub(crate) fn star(ctx: &Ctx<'_>, me: usize) -> Option<Star> {
    let c = &ctx.all[me];
    let img = ctx.img;
    let (w, h) = (img.width, img.height);
    let r = crate::detect::radius_for(c.sigma);
    if c.x < r || c.y < r || c.x + r >= w || c.y + r >= h {
        return None;
    }
    let (wx, wy) = (c.x - r, c.y - r);
    let noise = f64::from(ctx.bg.tile_at(c.x, c.y).1).max(1e-12);
    let near = ctx.neighbours(me, c.cx, c.cy, r as f32 + 8.0);
    let masked = |x: f32, y: f32| {
        near.iter().any(|&(ox, oy, or)| {
            let (dx, dy) = (x - ox, y - oy);
            dx * dx + dy * dy < or * or
        })
    };
    let side = 2 * r + 1;
    let mut px = Vec::with_capacity(side * side);
    for y in wy..wy + side {
        let row = &img.values[y * w..(y + 1) * w];
        for (x, &v) in row.iter().enumerate().skip(wx).take(side) {
            if !v.is_finite() || masked(x as f32, y as f32) {
                continue;
            }
            px.push(Px {
                x: (x - wx) as f64,
                y: (y - wy) as f64,
                v: f64::from(v),
            });
        }
    }
    if px.len() < side * side / 2 {
        return None;
    }
    // Start: the local background, the peak, the centroid, a round profile of the moments'
    // width at β = 3.
    let level = f64::from(ctx.bg.level_at(c.cx, c.cy));
    let beta0 = 3.0f64;
    let fwhm0 = f64::from(c.sigma.max(0.5)) * 2.354_82;
    let alpha0 = fwhm0 / (2.0 * (2f64.powf(1.0 / beta0) - 1.0).sqrt());
    let q0 = 1.0 / (alpha0 * alpha0);
    let mut p = [
        level,
        f64::from(c.height).max(noise),
        f64::from(c.cx) - wx as f64,
        f64::from(c.cy) - wy as f64,
        q0,
        0.0,
        q0,
        beta0,
    ];
    let mut jtj = [[0.0; NP]; NP];
    let mut jtr = [0.0; NP];
    let mut chi = system(&px, &p, Some((&mut jtj, &mut jtr)));
    let mut lambda = 1e-3;
    let mut converged = false;
    for _ in 0..MAX_ITER {
        let mut a = jtj;
        for (i, row) in a.iter_mut().enumerate() {
            row[i] += lambda * jtj[i][i].max(1e-12);
        }
        let mut b = jtr;
        let Some(step) = solve(&mut a, &mut b) else {
            lambda *= 10.0;
            if lambda > 1e10 {
                break;
            }
            continue;
        };
        let mut next = p;
        for i in 0..NP {
            next[i] += step[i];
        }
        // β is held to its range (a Gaussian star runs it up to the top).
        next[7] = next[7].clamp(BETA_MIN, BETA_MAX);
        let trial = if valid(&next) {
            system(&px, &next, None)
        } else {
            f64::INFINITY
        };
        if trial < chi {
            let done = (chi - trial) <= 1e-6 * chi.max(1e-300);
            p = next;
            chi = system(&px, &p, Some((&mut jtj, &mut jtr)));
            lambda = (lambda * 0.3).max(1e-9);
            if done {
                converged = true;
                break;
            }
        } else {
            lambda *= 8.0;
            if lambda > 1e10 {
                // No step improves: at the minimum as far as can be told.
                converged = true;
                break;
            }
        }
    }
    // Out of iterations: still a usable fit when the parameters are sound (a Gaussian
    // star's β creeps towards the top without changing the profile).
    let _ = converged;
    if !valid(&p) {
        return None;
    }
    let [b, a, x0, y0, qa, qb, qc, beta] = p;
    // The centre stayed near the peak, inside the window.
    let (sx, sy) = (x0 + wx as f64, y0 + wy as f64);
    if (sx - c.x as f64).abs() > 2.5 || (sy - c.y as f64).abs() > 2.5 {
        return None;
    }
    // Eigenvalues of [[qa, qb], [qb, qc]]: the smaller is the major axis.
    let tr = qa + qc;
    let disc = ((qa - qc) * (qa - qc) / 4.0 + qb * qb).sqrt();
    let (l_major, l_minor) = (tr / 2.0 - disc, tr / 2.0 + disc);
    if l_major <= 0.0 {
        return None;
    }
    let k = 2.0 * (2f64.powf(1.0 / beta) - 1.0).sqrt();
    let major = k / l_major.sqrt();
    let minor = k / l_minor.sqrt();
    let fwhm = (major * minor).sqrt();
    // Narrower than a pixel: a hot pixel or a cosmic ray. Wider than the window: not fitted.
    if !(0.9..=r as f64 * 1.2).contains(&fwhm) || a < 3.0 * noise {
        return None;
    }
    let ecc = (1.0 - (minor / major).powi(2)).max(0.0).sqrt();
    let angle = 0.5 * (2.0 * qb).atan2(qa - qc) + std::f64::consts::FRAC_PI_2;
    let angle = if angle > std::f64::consts::FRAC_PI_2 {
        angle - std::f64::consts::PI
    } else {
        angle
    };
    let (hfd, flux) = half_flux(&px, x0, y0, b, r as f64);
    Some(Star {
        x: sx as f32,
        y: sy as f32,
        amplitude: a as f32,
        background: b as f32,
        flux: flux as f32,
        fwhm_major: major as f32,
        fwhm_minor: minor as f32,
        fwhm: fwhm as f32,
        eccentricity: ecc as f32,
        angle: angle as f32,
        beta: beta as f32,
        hfd: hfd as f32,
        snr: (a / noise) as f32,
    })
}

/// Bins of the radial light profile per pixel of radius.
const BINS_PER_PX: usize = 20;

/// The half-flux diameter about (`x0`, `y0`) within radius `rad` of the pixels `px` less the
/// background `b`, and the light within it. Each pixel is split into 4 × 4 parts.
fn half_flux(px: &[Px], x0: f64, y0: f64, b: f64, rad: f64) -> (f64, f64) {
    let nb = (rad * BINS_PER_PX as f64).ceil() as usize + 1;
    let mut bins = vec![0.0f64; nb];
    let mut total = 0.0;
    const SUB: [f64; 4] = [-0.375, -0.125, 0.125, 0.375];
    for q in px {
        let part = (q.v - b) / 16.0;
        for oy in SUB {
            for ox in SUB {
                let (dx, dy) = (q.x + ox - x0, q.y + oy - y0);
                let d = (dx * dx + dy * dy).sqrt();
                if d <= rad {
                    bins[(d * BINS_PER_PX as f64) as usize] += part;
                    total += part;
                }
            }
        }
    }
    if total <= 0.0 {
        return (f64::NAN, total);
    }
    let half = total / 2.0;
    let mut acc = 0.0;
    for (i, v) in bins.iter().enumerate() {
        if acc + v >= half && *v > 0.0 {
            let f = (half - acc) / v;
            let r = (i as f64 + f) / BINS_PER_PX as f64;
            return (2.0 * r, total);
        }
        acc += v;
    }
    (2.0 * rad, total)
}
