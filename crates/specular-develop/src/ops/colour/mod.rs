//! Colour grading: point ops in OkLab
//! ([`crate::colour::oklab`], where the choice of space is argued) on linear Rec.2020,
//! between the tone ops and the view transform.
//!
//! - [`balance`]: colour balance: four wheels (shadows, midtones, highlights, global) with
//!   luminance, then vibrance, chroma, saturation and brilliance.
//! - [`equalizer`]: the colour equalizer: hue, saturation and brightness per hue band, a
//!   smooth periodic curve through eight band nodes, faded out on near-neutral pixels.
//! - [`point`]: point colour (after Lightroom's): a picked colour and its neighbourhood in
//!   hue, saturation and lightness, shifted with smooth falloff.
//! - [`bw`]: black & white: luminance with a per-hue-band brightness mix.
//! - [`lut`]: 3D LUT files (.cube, .3dl, .spi3d, CLF/CTF) through `specular-ocio`, in a
//!   declared space, run on the GPU from a table the engine uploads once.
//!
//! Every op keeps neutrals neutral (no hue is invented where there is no chroma), changes
//! chroma by scaling OkLab a and b (so hue holds), and is left out of the graph at its
//! defaults. Each has a CPU twin in [`cpu`] (tests only) with the kernel's f32 arithmetic.

pub mod balance;
pub mod bw;
pub mod equalizer;
pub mod lut;
pub mod point;

#[cfg(test)]
pub(crate) mod cpu;
#[cfg(test)]
mod tests;

use crate::colour::{self, oklab};
use crate::model::Fields;

/// Hue bands of the equalizer and the B&W mix, by field-name stem.
pub const BANDS: [&str; 8] = [
    "red", "orange", "yellow", "green", "aqua", "blue", "purple", "magenta",
];

/// HSV hues (degrees) of the bands' reference colours (fully saturated sRGB).
const BAND_HSV: [f64; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];

/// Words of one band node in the uniforms: cos h, sin h, then three values.
pub(crate) const BAND_WORDS: usize = 5;

/// sRGB-encoded → linear.
fn srgb_decode(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// The fully saturated sRGB colour of HSV hue `deg`, as OkLab a and b (unit length): the
/// direction a colour wheel at `deg` pushes, and a band's centre.
pub fn hsv_direction(deg: f64) -> [f64; 2] {
    let h = deg.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let lin = [r, g, b].map(srgb_decode);
    let rgb = colour::mulv(&colour::rec709_to_rec2020(), lin);
    let lab = oklab::from_rec2020_rel(rgb);
    let c = lab[1].hypot(lab[2]).max(1e-12);
    [lab[1] / c, lab[2] / c]
}

/// The bands' centres as OkLab (cos, sin).
pub fn band_directions() -> [[f64; 2]; 8] {
    BAND_HSV.map(hsv_direction)
}

/// A finite field clamped to `lo..=hi`, else `d`.
pub(crate) fn num(f: &Fields, name: &str, d: f32, lo: f32, hi: f32) -> f32 {
    let v = f.f32_or(name, d);
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

/// Coefficients that make the normalised blend pass through each band's value at its
/// centre (the 8×8 system of the blend at the centres, solved in f64). Equal values give
/// equal coefficients, so a flat curve stays flat.
fn interpolating(vals: &[[f32; 3]; 8]) -> [[f32; 3]; 8] {
    let d = band_directions();
    let k = f64::from(BAND_KAPPA);
    let mut a = [[0f64; 11]; 8];
    for i in 0..8 {
        let w: [f64; 8] =
            std::array::from_fn(|j| (k * (d[i][0] * d[j][0] + d[i][1] * d[j][1] - 1.0)).exp());
        let s: f64 = w.iter().sum();
        for j in 0..8 {
            a[i][j] = w[j] / s;
        }
        for c in 0..3 {
            a[i][8 + c] = f64::from(vals[i][c]);
        }
    }
    // Gaussian elimination with partial pivoting (the matrix is diagonally dominant).
    for col in 0..8 {
        let p = (col..8)
            .max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs()))
            .unwrap_or(col);
        a.swap(col, p);
        let pr = a[col];
        let piv = pr[col];
        for (r, row) in a.iter_mut().enumerate() {
            if r != col {
                let f = row[col] / piv;
                for (x, p) in row[col..].iter_mut().zip(&pr[col..]) {
                    *x -= f * p;
                }
            }
        }
    }
    std::array::from_fn(|i| std::array::from_fn(|c| (a[i][8 + c] / a[i][i]) as f32))
}

/// Band nodes into `out` (`BAND_WORDS` each): direction and three coefficients per band,
/// so the band curves pass through `vals` at the band centres.
pub(crate) fn write_bands(vals: &[[f32; 3]; 8], out: &mut [u32]) {
    let vals = interpolating(vals);
    for (i, (d, v)) in band_directions().iter().zip(&vals).enumerate() {
        let o = i * BAND_WORDS;
        super::put(
            &mut out[o..o + BAND_WORDS],
            &[d[0] as f32, d[1] as f32, v[0], v[1], v[2]],
        );
    }
}

/// How sharply a band's weight falls with hue distance: `exp(κ (cos Δh − 1))`, about a
/// Gaussian of 24° σ, so neighbouring bands blend and a lone band stays local.
pub const BAND_KAPPA: f32 = 6.0;

/// Shared WGSL: the band curves at a hue (`ch`, `sh` = cos, sin), normalised weights
/// (a partition of unity, so equal bands give a flat curve).
pub(crate) const BANDS_WGSL: &str =
    "fn dv_bands(o: u32, kappa: f32, ch: f32, sh: f32) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    var ws = 0.0;
    for (var i = 0u; i < 8u; i++) {
        let q = o + i * 5u;
        let w = exp(kappa * (ch * pf(q) + sh * pf(q + 1u) - 1.0));
        acc += w * vec3<f32>(pf(q + 2u), pf(q + 3u), pf(q + 4u));
        ws += w;
    }
    return acc / ws;
}
";

/// The CPU twin of `dv_bands`.
#[cfg(test)]
pub(crate) fn bands_f32(u: &[f32], kappa: f32, ch: f32, sh: f32) -> [f32; 3] {
    let mut acc = [0f32; 3];
    let mut ws = 0f32;
    for i in 0..8 {
        let q = &u[i * BAND_WORDS..];
        let w = (kappa * (ch * q[0] + sh * q[1] - 1.0)).exp();
        for k in 0..3 {
            acc[k] += w * q[2 + k];
        }
        ws += w;
    }
    acc.map(|a| a / ws)
}
