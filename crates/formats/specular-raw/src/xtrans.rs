//! Develop path for non-Bayer RGB colour filter arrays (Fujifilm X-Trans and any other
//! pattern whose every 3x3 window holds red, green and blue).
//!
//! * Half size: per-colour mean over the 3x3 window centred on each 2x2 output block.
//! * Full size: four candidate reconstructions, one per interpolation direction (green along
//!   the line, then red and blue by colour difference), and per pixel the average of the
//!   most homogeneous ones (the technique of Frank Markesteijn's X-Trans demosaic, in a
//!   simpler form; see [`full_band`]).
//!
//! Row bands run in parallel; each band reads a padded, clamped and normalised copy of its
//! rows (`PAD` pixels on every side), so no step depends on another band.

use std::sync::atomic::Ordering;

use rayon::prelude::*;

use crate::cfa::{BLUE, Cfa, GREEN, RED};
use crate::develop::{BAND, Matrix};
use crate::error::{RawError, Result};
use crate::{DecodeOptions, OutputSize, RawImage, Rect, RgbImage, WbSource};

/// Plane padding: green needs radius 2, colour difference 2 more, the derivatives 1, the
/// homogeneity window 1 and its sum 2.
const PAD: usize = 8;

/// Whether [`develop`] handles this pattern: colours 0..=2 only, every 3x3 window (wrapping)
/// holding all three.
pub(crate) fn supported(cfa: &Cfa) -> bool {
    if cfa.colors.iter().any(|&c| c > BLUE) {
        return false;
    }
    (0..cfa.height).all(|y| {
        (0..cfa.width).all(|x| {
            let mut seen = [false; 3];
            for dy in 0..3 {
                for dx in 0..3 {
                    seen[cfa.color(y + dy, x + dx) as usize] = true;
                }
            }
            seen == [true; 3]
        })
    })
}

/// The LUTs of [`develop`]: one per colour (`lut_len` entries, see [`crate::DevelopStats`])
/// for white balance multipliers `mul` (smallest 1); the black level of a colour is the mean
/// over its 2x2 slots that the pattern uses for it (RAF stores the same value in all four).
pub(crate) fn luts(
    raw: &RawImage,
    cfa: &Cfa,
    mul: [f32; 3],
    stats: &crate::DevelopStats,
) -> Vec<Vec<u16>> {
    let lut_len = stats.lut_len().clamp(1, 65536);
    (0..3usize)
        .map(|col| {
            let (mut s, mut n) = (0f32, 0f32);
            for y in 0..2u32 {
                for x in 0..2u32 {
                    if cfa.color(y, x) as usize == col {
                        s += raw.black[(y * 2 + x) as usize];
                        n += 1.0;
                    }
                }
            }
            let black = if n > 0.0 {
                s / n
            } else {
                raw.black.iter().sum::<f32>() / 4.0
            };
            let scale = mul[col] / (stats.white() - black).max(1.0) * 65535.0;
            let clip = stats.clip(col);
            (0..lut_len)
                .map(|v| {
                    if v >= clip {
                        return 65535;
                    }
                    let lin = match &raw.linearization {
                        Some(t) if !t.is_empty() => t[v.min(t.len() - 1)] as f32,
                        _ => v as f32,
                    };
                    ((lin - black) * scale).clamp(0.0, 65535.0).round() as u16
                })
                .collect()
        })
        .collect()
}

/// Develop output region `reg` (non-empty, within the output of [`DecodeOptions::size`]) of a
/// whole-sensor single-sample image with a [`supported`] CFA to linear Rec.709 (colour
/// transform `mat`) through the [`luts`]. Every output pixel depends only on the samples
/// around it (clamped at the crop's edges), so a region equals the same pixels of the whole
/// develop.
pub(crate) fn develop(
    raw: &RawImage,
    cfa: &Cfa,
    luts: &[Vec<u16>],
    mat: &Matrix,
    wb_source: WbSource,
    reg: Rect,
    opts: &DecodeOptions,
) -> Result<RgbImage> {
    let w = raw.width as usize;
    let crop = raw.crop;
    let (cx, cy) = (crop.x as usize, crop.y as usize);
    let (cw, ch) = (crop.w as usize, crop.h as usize);
    let lut = |c: u8, v: u16| -> i32 {
        let l = &luts[c as usize];
        l[(v as usize).min(l.len() - 1)] as i32
    };
    let cancel = || opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed));
    let color = |y: usize, x: usize| cfa.color(y as u32, x as u32);

    let half = opts.size == OutputSize::Half;
    let (rx, ry) = (reg.x as usize, reg.y as usize);
    let (ow, oh) = (reg.w as usize, reg.h as usize);
    let mut out = vec![0u16; ow * oh * 3];

    // Full size pads each band by PAD rows: taller bands waste less.
    let band_rows = if half { BAND } else { FULL_BAND };
    let band = |(bi, chunk): (usize, &mut [u16])| -> Result<()> {
        if cancel() {
            return Err(RawError::Cancelled);
        }
        let rows = chunk.len() / (ow * 3);
        let oy0 = ry + bi * band_rows;
        if half {
            for r in 0..rows {
                let yc = cy + (2 * (oy0 + r) + 1).min(ch - 1);
                for j in 0..ow {
                    let ox = rx + j;
                    let xc = cx + (2 * ox + 1).min(cw - 1);
                    let mut acc = [0i32; 3];
                    let mut n = [0i32; 3];
                    let mut rad = 1usize;
                    loop {
                        let ys = yc.saturating_sub(rad).max(cy)..=(yc + rad).min(cy + ch - 1);
                        for y in ys {
                            let row = &raw.data[y * w..];
                            let xs = xc.saturating_sub(rad).max(cx)..=(xc + rad).min(cx + cw - 1);
                            for x in xs {
                                let c = color(y, x);
                                acc[c as usize] += lut(c, row[x]);
                                n[c as usize] += 1;
                            }
                        }
                        // Tiny crops can miss a colour; widen the window.
                        if n.iter().all(|&k| k > 0) || rad >= 3 {
                            break;
                        }
                        rad += 1;
                        acc = [0; 3];
                        n = [0; 3];
                    }
                    let v: [i32; 3] =
                        std::array::from_fn(|c| if n[c] > 0 { acc[c] / n[c] } else { 0 });
                    mat.apply(v[0], v[1], v[2], &mut chunk[(r * ow + j) * 3..]);
                }
            }
            return Ok(());
        }
        full_band(
            &FullCtx {
                raw,
                lut: &lut,
                color: &color,
                mat,
                cx,
                cy,
                cw,
                ch,
                rx,
                ow,
            },
            oy0,
            rows,
            chunk,
        );
        Ok(())
    };
    let chunk = band_rows * ow * 3;
    if opts.parallel {
        out.par_chunks_mut(chunk).enumerate().try_for_each(band)?;
    } else {
        out.chunks_mut(chunk).enumerate().try_for_each(band)?;
    }
    Ok(RgbImage {
        width: ow as u32,
        height: oh as u32,
        data: out,
        orientation: raw.orientation,
        matrix_source: raw.matrix_source,
        wb_source,
        truncated: raw.truncated,
    })
}

struct FullCtx<'a> {
    raw: &'a RawImage,
    lut: &'a dyn Fn(u8, u16) -> i32,
    color: &'a dyn Fn(usize, usize) -> u8,
    mat: &'a Matrix,
    cx: usize,
    cy: usize,
    cw: usize,
    ch: usize,
    /// Output columns `rx..rx + ow` of the crop.
    rx: usize,
    ow: usize,
}

/// The 8 neighbour directions with their base weights (orthogonal neighbours are closer).
const DIRS: [(isize, isize, f32); 8] = [
    (0, -1, 2.0),
    (0, 1, 2.0),
    (-1, 0, 2.0),
    (1, 0, 2.0),
    (-1, -1, 1.0),
    (-1, 1, 1.0),
    (1, -1, 1.0),
    (1, 1, 1.0),
];

/// Gradient softening constant (16-bit normalised units).
const GRAD_K: f32 = 8192.0;
/// Green-similarity softening for the colour-difference weights.
const COLOR_K: f32 = 8192.0;

/// The interpolation directions of the full-size path: horizontal, vertical, both diagonals.
const LINES: [(isize, isize); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];
/// Output rows per band of the full-size path.
const FULL_BAND: usize = 48;
/// Positions along a line that may hold green samples.
const STEPS: [isize; 4] = [-2, -1, 1, 2];

/// Lagrange weights at 0 through the positions of [`STEPS`] present in `mask` (bit k for
/// `STEPS[k]`).
fn lagrange(mask: usize) -> [f32; 4] {
    std::array::from_fn(|k| {
        if mask & (1 << k) == 0 {
            return 0.0;
        }
        let xk = STEPS[k] as f32;
        (0..4)
            .filter(|&j| j != k && mask & (1 << j) != 0)
            .map(|j| -(STEPS[j] as f32) / (xk - STEPS[j] as f32))
            .product()
    })
}

/// Cube-root perceptual encoding of a normalised sample (0..=65535), for comparing the
/// candidate reconstructions.
fn perceptual(v: i32) -> f32 {
    static CBRT: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let t = CBRT.get_or_init(|| {
        (0..=65535u32)
            .map(|v| (v as f32 / 65535.0).cbrt() * 1024.0)
            .collect()
    });
    t[v.clamp(0, 65535) as usize]
}

/// Lightness-like and two opponent channels of a perceptually encoded RGB.
fn opponent(p: [i32; 3]) -> [f32; 3] {
    let (r, g, b) = (perceptual(p[0]), perceptual(p[1]), perceptual(p[2]));
    [(r + 2.0 * g + b) * 0.25, r - g, b - g]
}

/// Full-size demosaic of one band of output rows.
///
/// 1. Green at red/blue sites along each of the four [`LINES`]: interpolation through the
///    green samples on that line within two pixels on either side, limited to the range of
///    the nearest ones (where a line has none, the gradient-weighted mean of all green
///    neighbours).
/// 2. Red and blue for each line's green by colour difference (`C - G` of the same colour
///    within two pixels, by inverse squared distance and green similarity).
/// 3. For each line, the second derivative of the reconstruction along it (in a cube-root
///    space); a line is homogeneous at a pixel where its derivative is within 8 times the
///    smallest of the four. Over a 3x3 window that count, summed over 5x5, scores each line;
///    the output averages the lines scoring within 1/8 of the best.
///
/// Per-pixel values of the four lines sit side by side (`[_; 4]`).
fn full_band(ctx: &FullCtx, oy0: usize, rows: usize, chunk: &mut [u16]) {
    let FullCtx {
        raw,
        lut,
        color,
        mat,
        cx,
        cy,
        cw,
        ch,
        rx,
        ow,
    } = *ctx;
    let w = raw.width as usize;
    // Plane column `px` is crop column `rx + px - PAD`, clamped to the crop.
    let pw = ow + 2 * PAD;
    let ph = rows + 2 * PAD;
    let n = pw * ph;
    // Normalised values and colours with clamped (replicated) borders.
    let mut val = vec![0i32; n];
    let mut col = vec![0u8; n];
    for py in 0..ph {
        let y = cy + (oy0 + py).saturating_sub(PAD).min(ch - 1);
        let src = &raw.data[y * w..y * w + w];
        let vrow = &mut val[py * pw..(py + 1) * pw];
        let crow = &mut col[py * pw..(py + 1) * pw];
        for px in 0..pw {
            let x = cx + (rx + px).saturating_sub(PAD).min(cw - 1);
            let c = color(y, x);
            crow[px] = c;
            vrow[px] = lut(c, src[x]);
        }
    }
    let off = |dy: isize, dx: isize| dy * pw as isize + dx;
    let at = |i: usize, d: isize| (i as isize + d) as usize;
    let weights: [[f32; 4]; 16] = std::array::from_fn(lagrange);
    let line_off: [isize; 4] = LINES.map(|(dy, dx)| off(dy, dx));

    // 1. Green per line (rows/cols 2..len-2).
    let mut green = vec![[0i32; 4]; n];
    for py in 2..ph - 2 {
        for px in 2..pw - 2 {
            let i = py * pw + px;
            if col[i] == GREEN {
                green[i] = [val[i]; 4];
                continue;
            }
            let mut fb = None;
            for (l, &d) in line_off.iter().enumerate() {
                let q: [usize; 4] = std::array::from_fn(|k| at(i, STEPS[k] * d));
                let mut mask = 0;
                for (k, &qk) in q.iter().enumerate() {
                    if col[qk] == GREEN {
                        mask |= 1 << k;
                    }
                }
                // Nearest green on each side.
                let neg = [1, 0].into_iter().find(|&k| mask & (1 << k) != 0);
                let pos = [2, 3].into_iter().find(|&k| mask & (1 << k) != 0);
                green[i][l] = match (neg, pos) {
                    (Some(a), Some(b)) => {
                        let wt = &weights[mask];
                        let v: f32 = (0..4).map(|k| wt[k] * val[q[k]] as f32).sum();
                        let (a, b) = (val[q[a]], val[q[b]]);
                        (v.round() as i32).clamp(a.min(b), a.max(b))
                    }
                    _ => *fb.get_or_insert_with(|| fallback_green(&val, &col, i, pw)),
                };
            }
        }
    }

    // 2. Red and blue per line (rows/cols 4..len-4), with the perceptual triples.
    let mut rgb = vec![[[0i32; 3]; 4]; n];
    let mut opp = vec![[[0f32; 3]; 4]; n];
    let ring: Vec<(isize, f32)> = (-2isize..=2)
        .flat_map(|dy| (-2isize..=2).map(move |dx| (dy, dx)))
        .filter(|&(dy, dx)| (dy, dx) != (0, 0))
        .map(|(dy, dx)| (off(dy, dx), 1.0 / (dy * dy + dx * dx) as f32))
        .collect();
    for py in 4..ph - 4 {
        for px in 4..pw - 4 {
            let i = py * pw + px;
            let c0 = col[i];
            let gi = green[i];
            let mut sw = [[0f32; 4]; 3];
            let mut sd = [[0f32; 4]; 3];
            for &(d, inv) in &ring {
                let q = at(i, d);
                let c = col[q];
                if c == GREEN || c == c0 {
                    continue;
                }
                let gq = green[q];
                let (sw, sd) = (&mut sw[c as usize], &mut sd[c as usize]);
                for l in 0..4 {
                    let s = 1.0 + (gq[l] - gi[l]).abs() as f32 * (1.0 / COLOR_K);
                    let wgt = inv / (s * s);
                    sw[l] += wgt;
                    sd[l] += wgt * (val[q] - gq[l]) as f32;
                }
            }
            for l in 0..4 {
                let mut p = [0i32; 3];
                p[GREEN as usize] = gi[l];
                p[c0 as usize] = val[i];
                for c in [RED, BLUE] {
                    let c = c as usize;
                    if c != c0 as usize && sw[c][l] > 0.0 {
                        p[c] = (gi[l] as f32 + sd[c][l] / sw[c][l])
                            .round()
                            .clamp(0.0, 65535.0) as i32;
                    }
                }
                rgb[i][l] = p;
                opp[i][l] = opponent(p);
            }
        }
    }

    // 3. Second derivative along each line (rows/cols 5..len-5), homogeneity over 3x3
    //    (6..len-6), its 5-wide horizontal sums (6..len-6), then the choice.
    let mut drv = vec![[0f32; 4]; n];
    for py in 5..ph - 5 {
        for px in 5..pw - 5 {
            let i = py * pw + px;
            for (l, &d) in line_off.iter().enumerate() {
                let (a, b, c) = (opp[i][l], opp[at(i, d)][l], opp[at(i, -d)][l]);
                drv[i][l] = (0..3).map(|k| (2.0 * a[k] - b[k] - c[k]).powi(2)).sum();
            }
        }
    }
    let mut homo = vec![[0u8; 4]; n];
    for py in 6..ph - 6 {
        for px in 6..pw - 6 {
            let i = py * pw + px;
            let tr = 8.0 * drv[i].iter().copied().fold(f32::INFINITY, f32::min);
            let mut k = [0u8; 4];
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let d = &drv[at(i, off(dy, dx))];
                    for l in 0..4 {
                        k[l] += u8::from(d[l] <= tr);
                    }
                }
            }
            homo[i] = k;
        }
    }
    let mut hsum = vec![[0u16; 4]; n];
    for py in 6..ph - 6 {
        for px in 8..pw - 8 {
            let i = py * pw + px;
            let mut s = [0u16; 4];
            for dx in -2isize..=2 {
                let h = &homo[at(i, dx)];
                for l in 0..4 {
                    s[l] += u16::from(h[l]);
                }
            }
            hsum[i] = s;
        }
    }
    for r in 0..rows {
        let line = &mut chunk[r * ow * 3..(r + 1) * ow * 3];
        for ox in 0..ow {
            let i = (r + PAD) * pw + ox + PAD;
            let mut score = [0u32; 4];
            for dy in -2isize..=2 {
                let h = &hsum[at(i, off(dy, 0))];
                for l in 0..4 {
                    score[l] += u32::from(h[l]);
                }
            }
            let best = score.iter().copied().max().unwrap_or(0);
            let floor = best - best / 8;
            let (mut acc, mut k) = ([0i32; 3], 0i32);
            for (l, &s) in score.iter().enumerate() {
                if s >= floor {
                    let p = rgb[i][l];
                    for c in 0..3 {
                        acc[c] += p[c];
                    }
                    k += 1;
                }
            }
            let p = acc.map(|v| (v + k / 2) / k.max(1));
            mat.apply(p[0], p[1], p[2], &mut line[ox * 3..]);
        }
    }
}

/// Green at a red/blue site from all its green neighbours, each weighted by the inverse
/// local green gradient along its direction.
fn fallback_green(val: &[i32], col: &[u8], i: usize, pw: usize) -> i32 {
    let at = |d: isize| (i as isize + d) as usize;
    let (mut sw, mut sv) = (0f32, 0f32);
    for &(dy, dx, base) in &DIRS {
        let d = dy * pw as isize + dx;
        let nb = at(d);
        if col[nb] != GREEN {
            continue;
        }
        let (m, n2) = (at(-d), at(2 * d));
        let grad = if col[m] == GREEN {
            (val[nb] - val[m]).abs()
        } else if col[n2] == GREEN {
            2 * (val[nb] - val[n2]).abs()
        } else {
            (val[nb] - val[i]).abs()
        } as f32;
        let t = GRAD_K + grad;
        let wgt = base / (t * t);
        sw += wgt;
        sv += wgt * val[nb] as f32;
    }
    if sw > 0.0 {
        (sv / sw).round() as i32
    } else {
        val[i]
    }
}
