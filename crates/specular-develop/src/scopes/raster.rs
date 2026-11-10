//! The scopes as pictures: opaque RGBA8 (sRGB-encoded) rows of the size they are shown at,
//! so they stay sharp at any panel width. Traces add up on a dark ground (red, green and
//! blue where they overlap make white, as on a video scope). The axes, labels and marks are
//! drawn over them by the panel.

use super::{Bins, COLS, LEVELS, VEC};

/// The scopes' ground.
pub const GROUND: [u8; 3] = [18, 18, 20];

/// Trace colours (sRGB 0..1 per unit of brightness).
const RED: [f32; 3] = [1.0, 0.22, 0.2];
const GREEN: [f32; 3] = [0.25, 1.0, 0.3];
const BLUE: [f32; 3] = [0.3, 0.45, 1.0];
const LUMA: [f32; 3] = [0.92, 0.92, 0.92];

fn canvas(w: usize, h: usize) -> Vec<[f32; 3]> {
    vec![GROUND.map(|v| f32::from(v) / 255.0); w * h]
}

fn finish(px: Vec<[f32; 3]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len() * 4);
    for p in px {
        for v in p {
            out.push((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        }
        out.push(255);
    }
    out
}

fn add(p: &mut [f32; 3], c: [f32; 3], k: f32) {
    for i in 0..3 {
        p[i] += c[i] * k;
    }
}

/// Brightness of a waveform or vectorscope cell holding `n` samples where `typical` is a
/// well-filled cell: rises fast, then saturates (single samples still show).
fn glow(n: f32, typical: f32) -> f32 {
    if n <= 0.0 {
        return 0.0;
    }
    (1.0 - (-n / typical.max(1e-6)).exp()).max(0.12)
}

/// The R, G and B histograms over each other, `w`×`h` px; heights relative to the tallest
/// bin away from the ends (clipped pixels pile up there and would flatten the rest).
///
/// `log`: heights on a log scale, `ln(1 + n) / ln(1 + top)`, so the shape of the tones shows
/// next to a pile of shadows or highlights (a dark photo's histogram is a spike at the left
/// on a linear one); a bin holding one sample is still a sliver.
pub fn histogram(b: &Bins, w: u32, h: u32, log: bool) -> Vec<u8> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut px = canvas(w, h);
    let mut top = 1u32;
    for c in 0..3 {
        top = top.max(*b.hist(c)[1..LEVELS - 1].iter().max().unwrap_or(&1));
    }
    for (c, col) in [RED, GREEN, BLUE].into_iter().enumerate() {
        let hist = b.hist(c);
        for x in 0..w {
            // The bins this column covers: their highest.
            let l0 = x * LEVELS / w;
            let l1 = ((x + 1) * LEVELS).div_ceil(w).clamp(l0 + 1, LEVELS);
            let n = hist[l0..l1].iter().copied().max().unwrap_or(0);
            let f = if log {
                ((n as f32).ln_1p() / (top as f32).ln_1p()).min(1.0)
            } else {
                (n as f32 / top as f32).min(1.0)
            };
            let bar = (f * h as f32).round() as usize;
            for y in h - bar..h {
                add(&mut px[y * w + x], col, 0.55);
            }
        }
    }
    finish(px)
}

/// Plane `p` of the waveform drawn into columns `x0..x0 + pw` of a `w`-wide picture.
#[allow(clippy::too_many_arguments)]
fn wave_into(
    px: &mut [[f32; 3]],
    w: usize,
    h: usize,
    b: &Bins,
    p: usize,
    x0: usize,
    pw: usize,
    col: [f32; 3],
) {
    let plane = b.wave(p);
    // A well-filled cell: the samples of one column spread over a quarter of the levels.
    let per_col = b.samples as f32 / COLS as f32;
    for x in 0..pw {
        let c0 = x * COLS / pw;
        let c1 = ((x + 1) * COLS).div_ceil(pw).clamp(c0 + 1, COLS);
        for y in 0..h {
            // Level rows this pixel covers (top = white).
            let l1 = LEVELS - y * LEVELS / h;
            let l0 = LEVELS - ((y + 1) * LEVELS).div_ceil(h).min(LEVELS);
            let l0 = l0.min(l1.saturating_sub(1));
            let mut n = 0u32;
            for c in c0..c1 {
                n += plane[c * LEVELS + l0..c * LEVELS + l1].iter().sum::<u32>();
            }
            let cells = ((c1 - c0) * (l1 - l0)) as f32;
            let typical = per_col / (LEVELS as f32 / 4.0) * cells / 6.0;
            let k = glow(n as f32, typical);
            if k > 0.0 {
                add(&mut px[y * w + x0 + x], col, k);
            }
        }
    }
}

/// The waveform, `w`×`h` px: luma only, or R, G and B over each other.
pub fn waveform(b: &Bins, w: u32, h: u32, rgb: bool) -> Vec<u8> {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let mut px = canvas(w, h);
    if rgb {
        for (p, c) in [(1, RED), (2, GREEN), (3, BLUE)] {
            wave_into(&mut px, w, h, b, p, 0, w, c.map(|v| v * 0.7));
        }
    } else {
        wave_into(&mut px, w, h, b, 0, 0, w, LUMA);
    }
    finish(px)
}

/// The RGB parade, `w`×`h` px: the R, G and B waveforms side by side with small gaps.
pub fn parade(b: &Bins, w: u32, h: u32) -> Vec<u8> {
    let (w, h) = (w.max(3) as usize, h.max(1) as usize);
    let mut px = canvas(w, h);
    let gap = (w / 60).max(2);
    let pw = (w - 2 * gap) / 3;
    for (i, (p, c)) in [(1, RED), (2, GREEN), (3, BLUE)].into_iter().enumerate() {
        wave_into(&mut px, w, h, b, p, i * (pw + gap), pw, c);
    }
    finish(px)
}

/// The vectorscope, `side`×`side` px: each cell lit in its own hue.
pub fn vectorscope(b: &Bins, side: u32) -> Vec<u8> {
    let s = side.max(1) as usize;
    let mut px = canvas(s, s);
    let v = b.vector();
    let typical = b.samples as f32 / (VEC * VEC) as f32 * 8.0;
    let m = to_rec709();
    for y in 0..s {
        let r0 = y * VEC / s;
        let r1 = ((y + 1) * VEC).div_ceil(s).clamp(r0 + 1, VEC);
        for x in 0..s {
            let c0 = x * VEC / s;
            let c1 = ((x + 1) * VEC).div_ceil(s).clamp(c0 + 1, VEC);
            let mut n = 0u32;
            for r in r0..r1 {
                n += v[r * VEC + c0..r * VEC + c1].iter().sum::<u32>();
            }
            let cells = ((r1 - r0) * (c1 - c0)) as f32;
            let k = glow(n as f32, typical * cells);
            if k > 0.0 {
                let a = ((x as f32 + 0.5) / s as f32 - 0.5) * 2.0 * super::VEC_RANGE;
                let bb = (0.5 - (y as f32 + 0.5) / s as f32) * 2.0 * super::VEC_RANGE;
                add(&mut px[y * s + x], hue_colour(&m, a, bb), k);
            }
        }
    }
    finish(px)
}

/// A light colour of the hue at OkLab (a, b), sRGB-encoded, for lighting vectorscope cells.
fn hue_colour(m: &crate::colour::M3, a: f32, b: f32) -> [f32; 3] {
    let c = (a * a + b * b).sqrt();
    // Towards white near the centre; at most 0.12 chroma so the colour stays in sRGB.
    let k = if c > 1e-6 { c.min(0.12) / c } else { 0.0 };
    let lab = [0.82f64, f64::from(a * k), f64::from(b * k)];
    let xyz = crate::colour::oklab::oklab_to_xyz(lab);
    crate::colour::mulv(m, xyz).map(|v| super::encode(v as f32))
}

/// XYZ (D65) → linear Rec.709.
fn to_rec709() -> crate::colour::M3 {
    use crate::colour::{D65_XY, REC709_PRIMARIES, inv, rgb_to_xyz};
    inv(&rgb_to_xyz(&REC709_PRIMARIES, D65_XY)).expect("invertible")
}
