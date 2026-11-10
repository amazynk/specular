//! Light weights of each basis: what each coefficient term is multiplied by for a light
//! direction `(x, y, z)` (x right, y up, z towards the viewer, unit length).

use std::f64::consts::PI;

/// HSH lights are clamped to this polar angle (radians from the zenith): the basis is fitted
/// on the hemisphere and is unstable at grazing angles.
pub const HSH_MAX_THETA: f64 = PI / 2.0 - 0.04;

/// PTM terms in the PTM file order `a0..a5`: `x², y², xy, x, y, 1`.
pub fn ptm_weights(l: [f64; 3]) -> [f64; 6] {
    let (u, v) = (l[0], l[1]);
    [u * u, v * v, u * v, u, v, 1.0]
}

/// PTM terms in Relight's order: `1, x, y, x², xy, y²`.
pub fn ptm_weights_relight(l: [f64; 3]) -> [f64; 6] {
    let (u, v) = (l[0], l[1]);
    [1.0, u, v, u * u, u * v, v * v]
}

/// Hemispherical harmonics (Gautron et al. 2004) up to 16 terms, in the term order and sign
/// convention of CHI's fitter: index `l² + l − m`, cosines for `m > 0`, sines for `m < 0`.
pub fn hsh_weights(l: [f64; 3], out: &mut [f64]) {
    let mut phi = l[1].atan2(l[0]);
    if phi < 0.0 {
        phi += 2.0 * PI;
    }
    let theta = l[2].clamp(-1.0, 1.0).acos().min(HSH_MAX_THETA);
    let c = theta.cos();
    let c2 = c * c;
    let s = (c - c2).max(0.0).sqrt();
    let (cp, sp) = (phi.cos(), phi.sin());
    let (c2p, s2p) = ((2.0 * phi).cos(), (2.0 * phi).sin());
    let (c3p, s3p) = ((3.0 * phi).cos(), (3.0 * phi).sin());
    let all = [
        1.0 / (2.0 * PI).sqrt(),
        (6.0 / PI).sqrt() * cp * s,
        (3.0 / (2.0 * PI)).sqrt() * (2.0 * c - 1.0),
        (6.0 / PI).sqrt() * sp * s,
        (30.0 / PI).sqrt() * c2p * (c2 - c),
        (30.0 / PI).sqrt() * cp * (2.0 * c - 1.0) * s,
        (5.0 / (2.0 * PI)).sqrt() * (1.0 - 6.0 * c + 6.0 * c2),
        (30.0 / PI).sqrt() * sp * (2.0 * c - 1.0) * s,
        (30.0 / PI).sqrt() * s2p * (c2 - c),
        2.0 * (35.0 / PI).sqrt() * c3p * s * s * s,
        (210.0 / PI).sqrt() * c2p * (2.0 * c - 1.0) * (c2 - c),
        2.0 * (21.0 / PI).sqrt() * cp * s * (1.0 - 5.0 * c + 5.0 * c2),
        (7.0 / (2.0 * PI)).sqrt() * (-1.0 + 12.0 * c - 30.0 * c2 + 20.0 * c2 * c),
        2.0 * (21.0 / PI).sqrt() * sp * s * (1.0 - 5.0 * c + 5.0 * c2),
        (210.0 / PI).sqrt() * s2p * (2.0 * c - 1.0) * (c2 - c),
        2.0 * (35.0 / PI).sqrt() * s3p * s * s * s,
    ];
    let n = out.len().min(16);
    out[..n].copy_from_slice(&all[..n]);
}

/// Real spherical harmonics up to 9 terms in Relight's order.
pub fn sh_weights(l: [f64; 3], out: &mut [f64]) {
    let (x, y, z) = (l[0], l[1], l[2]);
    let a = 0.5 * (3.0 / PI).sqrt();
    let b = 0.5 * (15.0 / PI).sqrt();
    let all = [
        0.5 / PI.sqrt(),
        a * x,
        a * z,
        a * y,
        b * x * y,
        b * x * z,
        0.5 * (5.0 / PI).sqrt() * (3.0 * z * z - 1.0),
        b * y * z,
        0.5 * b * (y * y - x * x),
    ];
    let n = out.len().min(9);
    out[..n].copy_from_slice(&all[..n]);
}

/// Relight's radial-basis interpolation: per plane (plus the mean, `planes + 1`) and channel,
/// the weighted sum of the basis over the sampled lights near `l`.
pub(crate) fn rbf_weights(
    l: [f64; 3],
    lights: &[[f32; 3]],
    sigma: f32,
    basis: &[f32],
    planes: usize,
) -> Vec<[f64; 3]> {
    let radius = 1.0 / (sigma as f64 * sigma as f64);
    let mut w: Vec<(usize, f64)> = lights
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let d2 = (0..3).map(|k| (p[k] as f64 - l[k]).powi(2)).sum::<f64>();
            (i, (-radius * d2).exp())
        })
        .collect();
    let total: f64 = w.iter().map(|p| p.1).sum();
    if total > 0.0 {
        for p in &mut w {
            p.1 /= total;
        }
    }
    w.retain(|p| p.1 > 0.001);
    let total: f64 = w.iter().map(|p| p.1).sum();
    let n = lights.len();
    let mut out = vec![[0.0; 3]; planes + 1];
    for (p, o) in out.iter_mut().enumerate() {
        for (k, ok) in o.iter_mut().enumerate() {
            for &(i, wi) in &w {
                *ok += wi / total * basis[(p * n + i) * 3 + k] as f64;
            }
        }
    }
    out
}

/// Relight's bilinear interpolation over an octahedral map of lights.
pub(crate) fn bilinear_weights(
    l: [f64; 3],
    resolution: usize,
    basis: &[f32],
    planes: usize,
) -> Vec<[f64; 3]> {
    let r = resolution as f64;
    let s = l[0].abs() + l[1].abs() + l[2].abs();
    let x = ((l[0] + l[1]) / s + 1.0) / 2.0 * (r - 1.0);
    let y = ((l[1] - l[0]) / s + 1.0) / 2.0 * (r - 1.0);
    let sx = x.floor().clamp(0.0, r - 2.0);
    let sy = y.floor().clamp(0.0, r - 2.0);
    let (dx, dy) = (x - sx, y - sy);
    let (sx, sy) = (sx as usize, sy as usize);
    let corners = [
        (sx, sy, (1.0 - dx) * (1.0 - dy)),
        (sx + 1, sy, dx * (1.0 - dy)),
        (sx, sy + 1, (1.0 - dx) * dy),
        (sx + 1, sy + 1, dx * dy),
    ];
    let cells = resolution * resolution;
    let mut out = vec![[0.0; 3]; planes + 1];
    for (p, o) in out.iter_mut().enumerate() {
        for (k, ok) in o.iter_mut().enumerate() {
            for &(cx, cy, w) in &corners {
                *ok += w * basis[(p * cells + cx + cy * resolution) * 3 + k] as f64;
            }
        }
    }
    out
}

/// The sample lights the normals are fitted over: the zenith and three rings of eight at
/// 25°, 45° and 65° from the horizon.
pub const NORMAL_LIGHTS: usize = 25;

pub(crate) fn normal_lights() -> [[f64; 3]; NORMAL_LIGHTS] {
    let mut out = [[0.0, 0.0, 1.0]; NORMAL_LIGHTS];
    let mut i = 1;
    for elevation in [25.0f64, 45.0, 65.0] {
        let e = elevation.to_radians();
        for k in 0..8 {
            let a = k as f64 * PI / 4.0 + elevation.to_radians() / 3.0;
            out[i] = [e.cos() * a.cos(), e.cos() * a.sin(), e.sin()];
            i += 1;
        }
    }
    out
}
