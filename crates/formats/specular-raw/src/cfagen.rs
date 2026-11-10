//! Demosaic for colour filter arrays the develop step does not handle: four-colour arrays
//! (cyan, magenta, yellow and green, as in older compacts) and red/green/blue patterns where
//! some 3x3 window lacks a colour.
//!
//! Every pattern cell gets, for each filter colour of the pattern, a small kernel of the
//! nearest cells of that colour (weighted by inverse squared distance). The filter readings
//! are then mapped to RGB by least squares over the ideal filter responses (cyan = green +
//! blue, magenta = red + blue, yellow = red + green), scaled so that a sensor saturated in
//! every filter stays white. The result replaces the CFA samples by linear camera RGB
//! (cpp 3), black already subtracted.
//!
//! A four-colour sensor with a published colour matrix ([`FourColor`]) is converted with it
//! instead: each filter reading is white balanced (as-shot multipliers, or the matrix's
//! daylight balance followed by the develop's estimate), then mapped to linear Rec.709 by the pseudo-inverse of the camera's
//! filter responses to sRGB primaries (each filter's responses scaled so that daylight white
//! reads 1), as Adobe-style matrices are used.

use rayon::prelude::*;

use crate::RawImage;
use crate::cfa::Cfa;

/// RGB response of the filter colours, in the DNG `CFAPlaneColor` / EXIF numbering: red,
/// green, blue, cyan, magenta, yellow, white, and [`EMERALD`] (taken as cyan).
const RESPONSE: [[f32; 3]; 8] = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
];

/// Filter colour code of the emerald filter of Sony's RGBE sensor (beyond the DNG codes).
pub(crate) const EMERALD: u8 = 7;

/// A four-colour sensor's colour data: the filter colour codes (DNG numbering, [`EMERALD`])
/// in the order of the rows of `cam_xyz`, the camera's XYZ -> filter reading matrix, and the
/// as-shot multipliers per filter (same order) when the file has them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FourColor {
    pub colors: [u8; 4],
    pub cam_xyz: [[f64; 3]; 4],
    pub mul: Option<[f32; 4]>,
}

impl FourColor {
    /// From a published entry (`colors` named as dcraw names them: "GMCY", "RGBE").
    pub(crate) fn new(names: &str, m: &[i16; 12], mul: Option<[f32; 4]>) -> Option<FourColor> {
        let code = |c: u8| match c {
            b'R' => Some(0),
            b'G' => Some(1),
            b'B' => Some(2),
            b'C' => Some(3),
            b'M' => Some(4),
            b'Y' => Some(5),
            b'E' => Some(EMERALD),
            _ => None,
        };
        let b = names.as_bytes();
        let colors = [code(*b.first()?)?, code(b[1])?, code(b[2])?, code(b[3])?];
        Some(FourColor {
            colors,
            cam_xyz: std::array::from_fn(|i| std::array::from_fn(|j| m[i * 3 + j] as f64 / 1e4)),
            mul: mul.filter(|m| m.iter().all(|v| v.is_finite() && *v > 0.0)),
        })
    }

    /// Per filter colour of `order`: the white-balanced reading -> linear Rec.709 column, and
    /// the multiplier (minimum 1).
    fn conversion(&self, order: &[u8]) -> Option<(Vec<[f32; 3]>, Vec<f32>)> {
        /// Linear sRGB (D65) -> XYZ.
        const RGB_XYZ: [[f64; 3]; 3] = [
            [0.4124564, 0.3575761, 0.1804375],
            [0.2126729, 0.7151522, 0.0721750],
            [0.0193339, 0.1191920, 0.9503041],
        ];
        // Filter responses to the sRGB primaries, each row scaled to read 1 for white.
        let mut cam_rgb = [[0f64; 3]; 4];
        let mut daylight = [0f64; 4];
        for i in 0..4 {
            for j in 0..3 {
                cam_rgb[i][j] = (0..3).map(|k| self.cam_xyz[i][k] * RGB_XYZ[k][j]).sum();
            }
            let sum: f64 = cam_rgb[i].iter().sum();
            if sum.abs() < 1e-6 {
                return None;
            }
            for v in &mut cam_rgb[i] {
                *v /= sum;
            }
            daylight[i] = 1.0 / sum;
        }
        // Pseudo-inverse: (A^T A)^-1 A^T, A = cam_rgb (4 x 3).
        let mut ata = [[0f64; 3]; 3];
        for row in &cam_rgb {
            for i in 0..3 {
                for j in 0..3 {
                    ata[i][j] += row[i] * row[j];
                }
            }
        }
        let inv = invert3(ata)?;
        let mut mul: [f64; 4] = match self.mul {
            Some(m) => m.map(f64::from),
            None => daylight,
        };
        let lo = mul.iter().copied().fold(f64::INFINITY, f64::min);
        if !(lo.is_finite() && lo > 0.0) {
            return None;
        }
        for m in &mut mul {
            *m /= lo;
        }
        let mut cols = Vec::new();
        let mut muls = Vec::new();
        for &c in order {
            let i = self.colors.iter().position(|&k| k == c)?;
            cols.push(std::array::from_fn(|r| {
                (0..3).map(|j| inv[r][j] * cam_rgb[i][j]).sum::<f64>() as f32
            }));
            muls.push(mul[i] as f32);
        }
        Some((cols, muls))
    }
}

/// The filter colours of `cfa` in first-seen order, and the 3 x n matrix from their readings
/// to RGB. None when the colours cannot give red, green and blue.
fn conversion(cfa: &Cfa) -> Option<(Vec<u8>, Vec<[f32; 3]>)> {
    let mut colors: Vec<u8> = Vec::new();
    for &c in &cfa.colors {
        if (c as usize) >= RESPONSE.len() {
            return None;
        }
        if !colors.contains(&c) {
            colors.push(c);
        }
    }
    // A^T A (3 x 3) for the responses A (n x 3) of the colours present.
    let mut ata = [[0f64; 3]; 3];
    for &c in &colors {
        let r = RESPONSE[c as usize];
        for i in 0..3 {
            for j in 0..3 {
                ata[i][j] += r[i] as f64 * r[j] as f64;
            }
        }
    }
    let inv = invert3(ata)?;
    // pinv(A) = (A^T A)^-1 A^T: one column per colour.
    let mut cols: Vec<[f32; 3]> = colors
        .iter()
        .map(|&c| {
            let r = RESPONSE[c as usize];
            std::array::from_fn(|i| (0..3).map(|j| inv[i][j] * r[j] as f64).sum::<f64>() as f32)
        })
        .collect();
    // Saturation in every filter maps to equal RGB.
    for i in 0..3 {
        let s: f32 = cols.iter().map(|c| c[i]).sum();
        if s <= 1e-3 {
            return None;
        }
        for c in cols.iter_mut() {
            c[i] /= s;
        }
    }
    Some((colors, cols))
}

fn invert3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let c = |r: usize, k: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (k1, k2) = ((k + 1) % 3, (k + 2) % 3);
        m[r1][k1] * m[r2][k2] - m[r1][k2] * m[r2][k1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    (det.abs() > 1e-9).then(|| std::array::from_fn(|i| std::array::from_fn(|j| c(j, i) / det)))
}

/// Whether [`to_rgb`] handles `cfa`: a pattern the develop step cannot demosaic whose filter
/// colours give red, green and blue.
pub(crate) fn supported(cfa: &Cfa) -> bool {
    !cfa.developable() && conversion(cfa).is_some()
}

/// Whether the filter colours of `cfa` give red, green and blue (any pattern; what [`to_rgb`]
/// needs for a staggered layout).
pub(crate) fn convertible(cfa: &Cfa) -> bool {
    conversion(cfa).is_some()
}

/// One weighted neighbour: row and column offset, weight.
type Tap = (i32, i32, f32);

/// A DNG `CFALayout` other than rectangular (DNG 1.7, tag 50711): the photosites of some rows
/// or columns sit half a cell off the stored grid. Values 2..=9 as the specification numbers
/// them; "even" counts from the active area's first row and column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stagger {
    pub layout: u8,
    /// Parity of the active area's first stored row and column (`top & 1`, `left & 1`).
    pub row0: u8,
    pub col0: u8,
    /// Width of a stored cell over its height (DNG `DefaultScale` H / V), for distances.
    pub aspect: f32,
}

impl Stagger {
    /// For a `CFALayout` value: None for 1 (rectangular) and values the specification does
    /// not define.
    pub(crate) fn new(layout: u32, top: u32, left: u32, aspect: f32) -> Option<Stagger> {
        (2..=9).contains(&layout).then(|| Stagger {
            layout: layout as u8,
            row0: (top & 1) as u8,
            col0: (left & 1) as u8,
            aspect: if aspect.is_finite() && (0.1..=10.0).contains(&aspect) {
                aspect
            } else {
                1.0
            },
        })
    }

    /// Position of the photosite stored at (row, col), in stored cells (y, x scaled by the
    /// aspect).
    pub(crate) fn position(&self, row: i32, col: i32) -> (f32, f32) {
        let even_row = (row & 1) as u8 == self.row0;
        let even_col = (col & 1) as u8 == self.col0;
        let half = |b: bool| if b { 0.5 } else { 0.0 };
        let (dy, dx) = match self.layout {
            // A, B: even columns down / up by half a row.
            2 => (half(even_col), 0.0),
            3 => (-half(even_col), 0.0),
            // C, D: even rows right / left by half a column.
            4 => (0.0, half(even_row)),
            5 => (0.0, -half(even_row)),
            // E..H: even rows up (E, F) or down (G, H) by half a row, even columns left (E,
            // G) or right (F, H) by half a column.
            6 => (-half(even_row), -half(even_col)),
            7 => (-half(even_row), half(even_col)),
            8 => (half(even_row), -half(even_col)),
            _ => (half(even_row), half(even_col)),
        };
        (row as f32 + dy, (col as f32 + dx) * self.aspect)
    }
}

/// Kernel table: per cell of the period, per colour, the taps; and the period (columns, rows).
type Kernels = (Vec<Vec<Vec<Tap>>>, (usize, usize));

/// Per pattern cell, per colour: the taps that estimate that colour there (inverse squared
/// distance over the nearest cells of that colour). The table repeats every pattern period,
/// doubled where a stagger's row or column parity needs it.
fn kernels(cfa: &Cfa, colors: &[u8], stagger: Option<&Stagger>) -> Kernels {
    let (pw, ph) = (cfa.width as i32, cfa.height as i32);
    let even = |n: i32| {
        if stagger.is_some() && n % 2 == 1 {
            2 * n
        } else {
            n
        }
    };
    let (kw, kh) = (even(pw), even(ph));
    let reach = pw.max(ph) + 2 * i32::from(stagger.is_some());
    let mut out = Vec::with_capacity((kw * kh) as usize);
    for py in 0..kh {
        for px in 0..kw {
            let here = stagger.map_or((0.0, 0.0), |s| s.position(py, px));
            let per: Vec<Vec<Tap>> = colors
                .iter()
                .map(|&k| {
                    // (row, column offset, squared distance, true offset y, x).
                    let mut taps: Vec<(i32, i32, f32, f32, f32)> = Vec::new();
                    for dy in -reach..=reach {
                        for dx in -reach..=reach {
                            let (y, x) = ((py + dy).rem_euclid(ph), (px + dx).rem_euclid(pw));
                            if cfa.color(y as u32, x as u32) != k {
                                continue;
                            }
                            let (ey, ex) = match stagger {
                                Some(s) => {
                                    let (yy, xx) = s.position(py + dy, px + dx);
                                    (yy - here.0, xx - here.1)
                                }
                                None => (dy as f32, dx as f32),
                            };
                            taps.push((dy, dx, ey * ey + ex * ex, ey, ex));
                        }
                    }
                    let near = taps.iter().map(|t| t.2).fold(f32::INFINITY, f32::min);
                    if near <= 1e-6 {
                        return vec![(0, 0, 1.0)];
                    }
                    if stagger.is_some()
                        && let Some(v) = plane_fit(&taps, near)
                    {
                        return v;
                    }
                    let mut v: Vec<Tap> = taps
                        .into_iter()
                        .filter(|t| t.2 <= 2.0 * near + 1e-4)
                        .map(|(dy, dx, d, _, _)| (dy, dx, 1.0 / d))
                        .collect();
                    let sum: f32 = v.iter().map(|t| t.2).sum();
                    for t in v.iter_mut() {
                        t.2 /= sum;
                    }
                    v
                })
                .collect();
            out.push(per);
        }
    }
    (out, (kw as usize, kh as usize))
}

/// Taps that estimate a colour at the origin by a weighted least-squares plane through the
/// nearest cells of that colour (weights 1 / d²; the radius grows from twice the nearest
/// distance until the cells span a plane): exact on any locally linear scene, whatever the
/// cells' offsets. None when no radius within reach does.
fn plane_fit(taps: &[(i32, i32, f32, f32, f32)], near: f32) -> Option<Vec<Tap>> {
    [4.0f32, 8.0, 16.0, 32.0]
        .iter()
        .find_map(|&f| plane_fit_within(taps, f * near + 1e-4))
}

fn plane_fit_within(taps: &[(i32, i32, f32, f32, f32)], r2: f32) -> Option<Vec<Tap>> {
    let used: Vec<_> = taps.iter().filter(|t| t.2 <= r2).collect();
    let mut m = [[0f64; 3]; 3];
    for t in &used {
        let f = [1.0, t.3 as f64, t.4 as f64];
        let w = 1.0 / t.2 as f64;
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += w * f[i] * f[j];
            }
        }
    }
    // Collinear cells leave the normal matrix singular: its determinant against the product
    // of its diagonal (1 for independent directions) is then ~0.
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() <= 1e-3 * (m[0][0] * m[1][1] * m[2][2]).abs() {
        return None;
    }
    let inv = invert3(m)?;
    Some(
        used.iter()
            .map(|t| {
                let w = 1.0 / t.2 as f64;
                let a = inv[0][0] + inv[0][1] * t.3 as f64 + inv[0][2] * t.4 as f64;
                (t.0, t.1, (w * a) as f32)
            })
            .collect(),
    )
}

/// Replace the CFA samples of a whole-sensor `raw` (cpp 1, pattern [`supported`]) by linear
/// camera RGB (cpp 3). Black is subtracted and linearization applied on the way. With `four`,
/// the result is linear Rec.709 instead, white balanced by the as-shot multipliers (or by
/// the matrix's daylight balance, leaving `raw.wb` unknown so the develop estimates the rest).
/// With `stagger`, neighbours are weighted by their true distance on the staggered grid (any
/// pattern, Bayer included); the output stays on the stored grid.
pub(crate) fn to_rgb(
    raw: &mut RawImage,
    parallel: bool,
    four: Option<&FourColor>,
    stagger: Option<&Stagger>,
) -> bool {
    let Some(cfa) = raw.cfa.clone() else {
        return false;
    };
    let Some((colors, mut conv)) = conversion(&cfa) else {
        return false;
    };
    let mut gains = vec![1f32; colors.len()];
    let published = four.and_then(|f| f.conversion(&colors));
    if let Some((c, g)) = &published {
        conv = c.clone();
        gains = g.clone();
    }
    let (w, h) = (raw.width as usize, raw.height as usize);
    if raw.cpp != 1
        || raw.window.w as usize != w
        || raw.window.h as usize != h
        || raw.data.len() < w * h
    {
        return false;
    }
    let (ker, (pw, ph)) = kernels(&cfa, &colors, stagger);
    let black = raw.black;
    let lut = raw.linearization.take().filter(|t| !t.is_empty());
    let lin: Vec<f32> = (0..w * h)
        .map(|i| {
            let v = raw.data[i];
            let v = match &lut {
                Some(t) => t[(v as usize).min(t.len() - 1)] as f32,
                None => v as f32,
            };
            let (y, x) = (i / w, i % w);
            (v - black[(y & 1) * 2 + (x & 1)]).max(0.0)
        })
        .collect();
    let mut out = vec![0u16; w * h * 3];
    // Coordinate `v` moved into 0..n by whole periods `p`.
    let inside = |v: i64, n: usize, p: usize| {
        let (n, p) = (n as i64, p as i64);
        let v = if v < 0 { v + p * ((-v + p - 1) / p) } else { v };
        let v = if v >= n { v - p * ((v - n + p) / p) } else { v };
        (0..n).contains(&v).then_some(v as usize)
    };
    let row = |(y, line): (usize, &mut [u16])| {
        let mut est = vec![0f32; colors.len()];
        for (x, o) in line.chunks_exact_mut(3).enumerate() {
            let k = &ker[(y % ph) * pw + x % pw];
            for (e, taps) in est.iter_mut().zip(k) {
                let (mut s, mut wsum) = (0f32, 0f32);
                for &(dy, dx, wt) in taps {
                    let (yy, xx) = (y as i64 + dy as i64, x as i64 + dx as i64);
                    if yy >= 0 && xx >= 0 && (yy as usize) < h && (xx as usize) < w {
                        s += lin[yy as usize * w + xx as usize] * wt;
                        wsum += wt;
                    } else if stagger.is_some() {
                        // Plane-fit weights do not renormalise: every tap is needed.
                        wsum = 0.0;
                        break;
                    }
                }
                // Taps off the image on a staggered layout, or all of them (an edge photosite
                // whose nearest cells of a colour lie beyond it): the same taps moved inside
                // by whole periods, which keeps their colour and stagger.
                if wsum == 0.0 {
                    s = 0.0;
                    for &(dy, dx, wt) in taps {
                        let yy = inside(y as i64 + dy as i64, h, ph);
                        let xx = inside(x as i64 + dx as i64, w, pw);
                        if let (Some(yy), Some(xx)) = (yy, xx) {
                            s += lin[yy * w + xx] * wt;
                            wsum += wt;
                        }
                    }
                }
                *e = if wsum > 0.0 { s / wsum } else { 0.0 };
            }
            for (e, g) in est.iter_mut().zip(&gains) {
                *e *= g;
            }
            for (c, v) in o.iter_mut().enumerate() {
                let rgb: f32 = est.iter().zip(&conv).map(|(e, m)| e * m[c]).sum();
                *v = rgb.round().clamp(0.0, 65535.0) as u16;
            }
        }
    };
    if parallel {
        out.par_chunks_mut(w * 3).enumerate().for_each(row);
    } else {
        out.chunks_mut(w * 3).enumerate().for_each(row);
    }
    let b = black.iter().copied().fold(0f32, f32::max);
    raw.white = (raw.white - b).max(1.0);
    raw.black = [0.0; 4];
    raw.data = out;
    raw.cpp = 3;
    raw.cfa = None;
    if published.is_some() {
        raw.rgb_cam = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        raw.matrix_source = crate::MatrixSource::Published;
        raw.wb = four.and_then(|f| f.mul).map(|_| [1.0; 3]);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MatrixSource, Rect};

    fn image(cfa: Cfa, w: u32, h: u32, f: impl Fn(u32, u32) -> u16) -> RawImage {
        let data = (0..h)
            .flat_map(|y| (0..w).map(move |x| (y, x)))
            .map(|(y, x)| f(y, x))
            .collect();
        let full = Rect { x: 0, y: 0, w, h };
        RawImage {
            width: w,
            height: h,
            cpp: 1,
            data,
            window: full,
            cfa: Some(cfa),
            crop: full,
            black: [0.0; 4],
            white: 4095.0,
            linearization: None,
            wb: None,
            rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            matrix_source: MatrixSource::Generic,
            profile: None,
            orientation: 1,
            truncated: false,
            opcodes: None,
        }
    }

    #[test]
    fn cmyg_grey_and_colour() {
        // Yellow cyan / green magenta (Nikon E5700).
        let cfa = Cfa::new(2, 2, vec![5, 3, 1, 4]).unwrap();
        assert!(supported(&cfa));
        // A saturated sensor stays white.
        let mut raw = image(cfa.clone(), 8, 8, |_, _| 4000);
        assert!(to_rgb(&mut raw, false, None, None));
        assert_eq!(raw.cpp, 3);
        assert_eq!(&raw.data[(4 * 8 + 4) * 3..][..3], &[4000, 4000, 4000]);
        // Pure red light: yellow and magenta see it, cyan and green do not.
        let mut raw = image(cfa, 8, 8, |y, x| match (y & 1, x & 1) {
            (0, 0) | (1, 1) => 1000,
            _ => 0,
        });
        assert!(to_rgb(&mut raw, true, None, None));
        let p = &raw.data[(4 * 8 + 4) * 3..][..3];
        assert!(p[0] > 500 && p[1] < 100 && p[2] < 100, "{p:?}");
    }

    #[test]
    fn four_colour_matrix_keeps_daylight_white() {
        // Sony DSC-F828 (R, G, B, emerald), filters E R / B G.
        let e = crate::color::four_color_entry("Sony", "DSC-F828").unwrap();
        let f = FourColor::new(e.colors, &e.matrix, None).unwrap();
        // Readings of D65 white (sRGB 1, 1, 1) through the camera matrix.
        let white_xyz = [0.9505, 1.0, 1.089];
        let read: Vec<f32> = f
            .cam_xyz
            .iter()
            .map(|r| (r[0] * white_xyz[0] + r[1] * white_xyz[1] + r[2] * white_xyz[2]) as f32)
            .collect();
        let at = |c: u8| read[f.colors.iter().position(|&k| k == c).unwrap()];
        let cfa = Cfa::new(2, 2, vec![EMERALD, 0, 2, 1]).unwrap();
        assert!(supported(&cfa));
        let mut raw = image(cfa.clone(), 8, 8, |y, x| {
            (at(cfa.color(y, x)) * 1000.0).round() as u16
        });
        assert!(to_rgb(&mut raw, false, Some(&f), None));
        assert_eq!(raw.matrix_source, crate::MatrixSource::Published);
        let p = &raw.data[(4 * 8 + 4) * 3..][..3];
        let m = p.iter().copied().max().unwrap() as f32;
        assert!(p.iter().all(|&v| (v as f32 - m).abs() / m < 0.02), "{p:?}");
        // With as-shot multipliers the balance is set.
        let f = FourColor::new(e.colors, &e.matrix, Some([2.0, 1.0, 1.5, 1.2])).unwrap();
        let mut raw = image(cfa, 8, 8, |_, _| 500);
        assert!(to_rgb(&mut raw, true, Some(&f), None));
        assert_eq!(raw.wb, Some([1.0; 3]));
        assert!(FourColor::new("GMX", &e.matrix, None).is_none());
    }

    #[test]
    fn rgb_stripes() {
        // Vertical R G B G stripes: the window over G B G has no red.
        let cfa = Cfa::new(4, 1, vec![0, 1, 2, 1]).unwrap();
        assert!(supported(&cfa));
        let mut raw = image(cfa, 12, 4, |_, x| [100, 200, 300, 200][x as usize % 4]);
        raw.black = [10.0; 4];
        assert!(to_rgb(&mut raw, false, None, None));
        assert_eq!(&raw.data[(2 * 12 + 5) * 3..][..3], &[90, 190, 290]);
        assert_eq!(raw.white, 4085.0);
    }

    #[test]
    fn unsupported_patterns() {
        assert!(!supported(&Cfa::bayer("RGGB").unwrap()));
        // Cyan and green only cannot give red.
        assert!(!supported(&Cfa::new(2, 1, vec![3, 1]).unwrap()));
        assert!(!supported(&Cfa::new(2, 1, vec![9, 1]).unwrap()));
    }
}
