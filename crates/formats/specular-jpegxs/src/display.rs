//! Decoded samples to display pixels: greyscale, RGB, YCbCr through its matrix (chroma
//! interpolated to full resolution), colour filter mosaics through a bilinear demosaic.

use crate::Pixels;
use crate::decode::{SamplePlane, Samples};
use crate::file::Cicp;
use crate::header::Cfa;
use rayon::prelude::*;

/// How the components are to be read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    Grey,
    GreyAlpha,
    Rgb {
        alpha: bool,
    },
    YCbCr {
        kr: f32,
        kb: f32,
        full: bool,
        alpha: bool,
    },
    Mosaic(Cfa),
}

/// Luma coefficients of a CICP matrix code (BT.709 for unknown codes).
pub(crate) fn luma_weights(matrix: u16) -> (f32, f32) {
    match matrix {
        4 => (0.30, 0.11),
        5 | 6 => (0.299, 0.114),
        7 => (0.212, 0.087),
        9 | 10 => (0.2627, 0.0593),
        _ => (0.2126, 0.0722),
    }
}

pub(crate) fn kind(
    nc: usize,
    subsampled: bool,
    rct: bool,
    cfa: Option<Cfa>,
    cicp: Option<Cicp>,
) -> Kind {
    if let Some(c) = cfa {
        return Kind::Mosaic(c);
    }
    match nc {
        1 => Kind::Grey,
        2 => Kind::GreyAlpha,
        _ => {
            let alpha = nc >= 4;
            let ycc = match cicp {
                _ if rct => false,
                Some(c) => c.matrix != 0,
                None => subsampled,
            };
            if ycc {
                let (kr, kb) = luma_weights(cicp.map(|c| c.matrix).unwrap_or(1));
                Kind::YCbCr {
                    kr,
                    kb,
                    full: cicp.is_some_and(|c| c.full_range),
                    alpha,
                }
            } else {
                Kind::Rgb { alpha }
            }
        }
    }
}

/// One row of a component at full resolution, in sample units: subsampled components
/// interpolated (co-sited horizontally, centred vertically).
fn upsampled_row(
    p: &SamplePlane,
    sx: usize,
    sy: usize,
    y: usize,
    out: &mut [f32],
    tmp: &mut Vec<f32>,
) {
    if p.data.is_empty() || p.width == 0 || p.height == 0 {
        out.fill(0.0);
        return;
    }
    let row_of = |yy: usize| &p.data[yy.min(p.height - 1) * p.width..][..p.width];
    let horizontal = |src: &[u16], dst: &mut [f32]| {
        if sx == 1 {
            for (d, &v) in dst.iter_mut().zip(src) {
                *d = v as f32;
            }
            let last = src.last().copied().unwrap_or(0) as f32;
            for d in dst.iter_mut().skip(src.len()) {
                *d = last;
            }
        } else {
            let n = src.len();
            for (x, d) in dst.iter_mut().enumerate() {
                let i = (x / 2).min(n - 1);
                *d = if x % 2 == 0 {
                    src[i] as f32
                } else {
                    (src[i] as f32 + src[(i + 1).min(n - 1)] as f32) * 0.5
                };
            }
        }
    };
    if sy == 1 {
        horizontal(row_of(y), out);
    } else {
        let c = y / 2;
        let other = if y.is_multiple_of(2) {
            c.saturating_sub(1)
        } else {
            c + 1
        };
        horizontal(row_of(c), out);
        tmp.resize(out.len(), 0.0);
        horizontal(row_of(other), tmp);
        for (o, t) in out.iter_mut().zip(tmp.iter()) {
            *o = (*o * 3.0 + *t) * 0.25;
        }
    }
}

/// Fills an RGBA output row by row in parallel: `f(y, rgba)` gives the row as values 0‥1.
fn rgba_rows(
    width: usize,
    height: usize,
    eight: bool,
    f: impl Fn(usize, &mut [f32]) + Sync,
) -> Pixels {
    let n = width * height * 4;
    if eight {
        let mut out = vec![0u8; n];
        out.par_chunks_mut(width * 4)
            .enumerate()
            .for_each_init(Vec::new, |row, (y, o)| {
                row.resize(width * 4, 0.0);
                f(y, row);
                for (d, &v) in o.iter_mut().zip(row.iter()) {
                    *d = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            });
        Pixels::Rgba8(out)
    } else {
        let mut out = vec![0u16; n];
        out.par_chunks_mut(width * 4)
            .enumerate()
            .for_each_init(Vec::new, |row, (y, o)| {
                row.resize(width * 4, 0.0);
                f(y, row);
                for (d, &v) in o.iter_mut().zip(row.iter()) {
                    *d = (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
                }
            });
        Pixels::Rgba16(out)
    }
}

/// Converts decoded samples to pixels of `width`×`height`.
pub(crate) fn pixels(
    s: &Samples,
    k: Kind,
    sub: &[(usize, usize)],
    width: usize,
    height: usize,
) -> Pixels {
    let maxi = (1u32 << s.depth) - 1;
    let max = maxi as f32;
    let eight = s.depth <= 8;
    if width == 0 || height == 0 {
        return Pixels::Gray8(Vec::new());
    }
    match k {
        Kind::Grey => {
            let p = &s.planes[0];
            if eight {
                Pixels::Gray8(
                    p.data
                        .par_iter()
                        .map(|&v| ((v as u32 * 255 + maxi / 2) / maxi) as u8)
                        .collect(),
                )
            } else {
                Pixels::Gray16(
                    p.data
                        .par_iter()
                        .map(|&v| ((v as u32 * 65535 + maxi / 2) / maxi) as u16)
                        .collect(),
                )
            }
        }
        Kind::Mosaic(cfa) => {
            let p = &s.planes[0];
            rgba_rows(width, height, eight, |y, row| {
                demosaic_row(&p.data, p.width, p.height, cfa, y, row, 1.0 / max);
            })
        }
        _ => {
            let chans = s.planes.len().min(4);
            let mid = (1u32 << (s.depth - 1)) as f32;
            let st = (1u32 << s.depth) as f32 / 256.0;
            rgba_rows(width, height, eight, |y, row| {
                // Each component's row at full resolution, then the colour model.
                let mut c = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                let mut tmp = Vec::new();
                for (i, v) in c.iter_mut().enumerate().take(chans) {
                    v.resize(width, 0.0);
                    let (sx, sy) = sub[i];
                    upsampled_row(&s.planes[i], sx, sy, y, v, &mut tmp);
                }
                let px = row.chunks_exact_mut(4);
                match k {
                    Kind::GreyAlpha => {
                        for (x, o) in px.enumerate() {
                            let v = c[0][x] / max;
                            o.copy_from_slice(&[v, v, v, c[1][x] / max]);
                        }
                    }
                    Kind::Rgb { alpha } => {
                        for (x, o) in px.enumerate() {
                            let a = if alpha { c[3][x] / max } else { 1.0 };
                            o.copy_from_slice(&[c[0][x] / max, c[1][x] / max, c[2][x] / max, a]);
                        }
                    }
                    Kind::YCbCr {
                        kr,
                        kb,
                        full,
                        alpha,
                    } => {
                        // Full range: 0‥max; narrow: luma 16‥235, chroma 16‥240 (8-bit steps).
                        let (y0, ys, cs) = if full {
                            (0.0, 1.0 / max, 1.0 / max)
                        } else {
                            (16.0 * st, 1.0 / (219.0 * st), 1.0 / (224.0 * st))
                        };
                        let kg = 1.0 - kr - kb;
                        let (rc, bc) = (2.0 * (1.0 - kr), 2.0 * (1.0 - kb));
                        for (x, o) in px.enumerate() {
                            let yy = (c[0][x] - y0) * ys;
                            let cb = (c[1][x] - mid) * cs;
                            let cr = (c[2][x] - mid) * cs;
                            let r = yy + rc * cr;
                            let b = yy + bc * cb;
                            let g = (yy - kr * r - kb * b) / kg;
                            let a = if alpha { c[3][x] / max } else { 1.0 };
                            o.copy_from_slice(&[r, g, b, a]);
                        }
                    }
                    _ => {}
                }
            })
        }
    }
}

/// Bilinear demosaic of row `y` into RGBA (times `scale`): each pixel's own colour as is,
/// the others the mean of the nearest samples of that colour, mirrored at the edges.
fn demosaic_row(v: &[u16], w: usize, h: usize, cfa: Cfa, y: usize, out: &mut [f32], scale: f32) {
    let at = |xx: usize, yy: usize| v[yy * w + xx] as f32;
    if w < 2 || h < 2 {
        for (x, o) in out.chunks_exact_mut(4).enumerate() {
            let s = at(x.min(w - 1), y.min(h - 1)) * scale;
            o.copy_from_slice(&[s, s, s, 1.0]);
        }
        return;
    }
    let ym = if y == 0 { 1 } else { y - 1 };
    let yp = if y + 1 == h { h - 2 } else { y + 1 };
    for (x, o) in out.chunks_exact_mut(4).enumerate() {
        let xm = if x == 0 { 1 } else { x - 1 };
        let xp = if x + 1 == w { w - 2 } else { x + 1 };
        let own = cfa.at(x, y);
        let mut c = [0.0f32; 3];
        c[own] = at(x, y);
        if own == 1 {
            let along = cfa.at(x + 1, y);
            c[along] = (at(xm, y) + at(xp, y)) * 0.5;
            c[2 - along] = (at(x, ym) + at(x, yp)) * 0.5;
        } else {
            c[1] = (at(xm, y) + at(xp, y) + at(x, ym) + at(x, yp)) * 0.25;
            c[2 - own] = (at(xm, ym) + at(xp, ym) + at(xm, yp) + at(xp, yp)) * 0.25;
        }
        o.copy_from_slice(&[c[0] * scale, c[1] * scale, c[2] * scale, 1.0]);
    }
}
