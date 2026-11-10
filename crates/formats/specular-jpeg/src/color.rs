//! Planar → interleaved RGBA conversion (BT.601 full range, libjpeg-turbo arithmetic) and
//! CMYK / YCCK handling.

use rayon::prelude::*;

use crate::{DecodedPlanes, Plane, PlanesLayout};

const SCALEBITS: i32 = 16;
const ONE_HALF: i32 = 1 << (SCALEBITS - 1);
const fn fix(x: f64) -> i32 {
    (x * (1 << SCALEBITS) as f64 + 0.5) as i32
}

/// Precomputed libjpeg `jdcolor.c` tables.
struct Tables {
    cr_r: [i32; 256],
    cb_b: [i32; 256],
    cr_g: [i32; 256],
    cb_g: [i32; 256],
}

static TABLES: std::sync::LazyLock<Tables> = std::sync::LazyLock::new(|| {
    let mut t = Tables {
        cr_r: [0; 256],
        cb_b: [0; 256],
        cr_g: [0; 256],
        cb_g: [0; 256],
    };
    for i in 0..256 {
        let x = i as i32 - 128;
        t.cr_r[i] = (fix(1.40200) * x + ONE_HALF) >> SCALEBITS;
        t.cb_b[i] = (fix(1.77200) * x + ONE_HALF) >> SCALEBITS;
        t.cr_g[i] = -fix(0.71414) * x;
        t.cb_g[i] = -fix(0.34414) * x + ONE_HALF;
    }
    t
});

#[inline(always)]
fn clamp(x: i32) -> u8 {
    x.clamp(0, 255) as u8
}

#[inline(always)]
pub(crate) fn ycc_to_rgb(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
    let t = &*TABLES;
    let y = y as i32;
    let r = y + t.cr_r[cr as usize];
    let g = y + ((t.cb_g[cb as usize] + t.cr_g[cr as usize]) >> SCALEBITS);
    let b = y + t.cb_b[cb as usize];
    (clamp(r), clamp(g), clamp(b))
}

/// How subsampled chroma is brought to full resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Upsampling {
    /// Centered bilinear ("fancy") upsampling. For 2x horizontal and/or vertical factors this is
    /// bit-exact with libjpeg-turbo's default (`h2v1`/`h2v2`/`h1v2` fancy upsampling); other
    /// factors use a generic centered bilinear filter.
    #[default]
    Bilinear,
    /// Sample replication (matches `djpeg -nosmooth`).
    Nearest,
}

/// Upsample one row of a component plane to `width` samples at output row `y`.
/// `(fx, fy)` are the integer upsampling factors (max_h / h, max_v / v).
fn upsample_row(
    p: &Plane,
    (fx, fy): (usize, usize),
    y: usize,
    width: usize,
    mode: Upsampling,
    tmp: &mut Vec<i32>,
    out: &mut [u8],
) {
    let out = &mut out[..width];
    let ph = p.height.max(1) as usize;
    let pw = p.width.max(1) as usize;
    let row = |r: usize| &p.data[r.min(ph - 1) * p.stride..][..pw];
    if fx == 1 && fy == 1 && pw >= width {
        out.copy_from_slice(&row(y)[..width]);
        return;
    }
    // Defensive: a plane too narrow for the output (cannot happen for frames accepted by the
    // decoder) is replicated with clamping instead of indexing out of bounds.
    if mode == Upsampling::Nearest || pw * fx < width {
        let src = row(y / fy);
        for (x, o) in out.iter_mut().enumerate() {
            *o = src[(x / fx).min(pw - 1)];
        }
        return;
    }
    let cy = y / fy;
    match (fx, fy) {
        (2, 2) => {
            let cur = row(cy);
            let other = if y.is_multiple_of(2) {
                row(cy.saturating_sub(1))
            } else {
                row(cy + 1)
            };
            tmp.clear();
            tmp.extend(
                cur.iter()
                    .zip(other)
                    .map(|(&a, &b)| a as i32 * 3 + b as i32),
            );
            let cs = &tmp[..];
            for (x, o) in out.iter_mut().enumerate() {
                let i = x / 2;
                let v = if x % 2 == 0 {
                    (cs[i] * 3 + cs[i.saturating_sub(1)] + 8) >> 4
                } else {
                    (cs[i] * 3 + cs[(i + 1).min(pw - 1)] + 7) >> 4
                };
                *o = v as u8;
            }
        }
        (2, 1) => {
            let s = row(cy);
            for (x, o) in out.iter_mut().enumerate() {
                let i = x / 2;
                let v = if x % 2 == 0 {
                    (s[i] as i32 * 3 + s[i.saturating_sub(1)] as i32 + 1) >> 2
                } else {
                    (s[i] as i32 * 3 + s[(i + 1).min(pw - 1)] as i32 + 2) >> 2
                };
                *o = v as u8;
            }
        }
        (1, 2) => {
            let cur = row(cy);
            let (other, bias) = if y.is_multiple_of(2) {
                (row(cy.saturating_sub(1)), 1)
            } else {
                (row(cy + 1), 2)
            };
            for (x, o) in out.iter_mut().enumerate() {
                *o = ((cur[x] as i32 * 3 + other[x] as i32 + bias) >> 2) as u8;
            }
        }
        _ => {
            // Generic centered bilinear: sample i is centered at (i + 0.5) * f - 0.5.
            // Fixed point with 8 fractional bits.
            let syf = (((2 * y + 1) * 256) / (2 * fy)) as i64 - 128;
            let sy = syf.max(0);
            let (y0, wy) = ((sy >> 8) as usize, (sy & 255) as i32);
            let r0 = row(y0);
            let r1 = row(y0 + 1);
            for (x, o) in out.iter_mut().enumerate() {
                let sxf = (((2 * x + 1) * 256) / (2 * fx)) as i64 - 128;
                let sx = sxf.max(0);
                let (x0, wx) = (((sx >> 8) as usize).min(pw - 1), (sx & 255) as i32);
                let x1 = (x0 + 1).min(pw - 1);
                let top = r0[x0] as i32 * (256 - wx) + r0[x1] as i32 * wx;
                let bot = r1[x0] as i32 * (256 - wx) + r1[x1] as i32 * wx;
                *o = ((top * (256 - wy) + bot * wy + (1 << 15)) >> 16) as u8;
            }
        }
    }
}

fn factors(planes: &[Plane]) -> Vec<(usize, usize)> {
    let mh = planes.iter().map(|q| q.h_samp).max().unwrap_or(1).max(1) as usize;
    let mv = planes.iter().map(|q| q.v_samp).max().unwrap_or(1).max(1) as usize;
    planes
        .iter()
        .map(|q| {
            (
                (mh / q.h_samp.max(1) as usize).max(1),
                (mv / q.v_samp.max(1) as usize).max(1),
            )
        })
        .collect()
}

/// Convert decoded planes to tightly packed RGBA8 (`width * height * 4` bytes) with
/// bilinear chroma upsampling.
pub fn planes_to_rgba(p: &DecodedPlanes) -> Vec<u8> {
    let mut out = vec![0u8; p.width as usize * p.height as usize * 4];
    planes_to_rgba_into(p, Upsampling::Bilinear, &mut out, p.width as usize * 4);
    out
}

/// Like [`planes_to_rgba`] but into a caller buffer with the given row stride (bytes) and
/// upsampling mode. Rows are converted in parallel on the current rayon pool.
///
/// # Panics
/// If `out` is smaller than `stride * (height - 1) + width * 4` or `stride < width * 4`.
pub fn planes_to_rgba_into(p: &DecodedPlanes, mode: Upsampling, out: &mut [u8], stride: usize) {
    let w = p.width as usize;
    let h = p.height as usize;
    if w == 0 || h == 0 {
        return;
    }
    assert!(
        stride >= w * 4 && out.len() >= stride * (h - 1) + w * 4,
        "output buffer too small"
    );
    let f = factors(&p.planes);
    let rows = out.par_chunks_mut(stride).take(h).enumerate();
    match p.layout {
        PlanesLayout::Rgba => {
            let src = &p.planes[0];
            rows.for_each(|(y, o)| {
                o[..w * 4].copy_from_slice(&src.data[y * src.stride..y * src.stride + w * 4])
            });
        }
        PlanesLayout::Gray => {
            let src = &p.planes[0];
            rows.for_each(|(y, o)| {
                for (x, px) in o[..w * 4].chunks_exact_mut(4).enumerate() {
                    let v = src.data[y * src.stride + x];
                    px.copy_from_slice(&[v, v, v, 255]);
                }
            });
        }
        PlanesLayout::YCbCr | PlanesLayout::Rgb => {
            let ycc = p.layout == PlanesLayout::YCbCr;
            // A fourth plane ([`crate::decode_components`]) is alpha.
            let alpha = p.planes.get(3).map(|pl| (pl, f[3]));
            rows.for_each_init(
                || {
                    (
                        vec![0u8; w],
                        vec![0u8; w],
                        vec![0u8; w],
                        vec![255u8; w],
                        Vec::new(),
                    )
                },
                |(a, b, c, al, tmp), (y, o)| {
                    upsample_row(&p.planes[0], f[0], y, w, mode, tmp, a);
                    upsample_row(&p.planes[1], f[1], y, w, mode, tmp, b);
                    upsample_row(&p.planes[2], f[2], y, w, mode, tmp, c);
                    if let Some((pl, fa)) = alpha {
                        upsample_row(pl, fa, y, w, mode, tmp, al);
                    }
                    for (x, px) in o[..w * 4].chunks_exact_mut(4).enumerate() {
                        let (r, g, bb) = if ycc {
                            ycc_to_rgb(a[x], b[x], c[x])
                        } else {
                            (a[x], b[x], c[x])
                        };
                        px.copy_from_slice(&[r, g, bb, al[x]]);
                    }
                },
            );
        }
    }
}

/// Convert 4-component planes (CMYK or YCCK, each possibly subsampled) to an RGBA plane.
/// CMYK is always treated as Adobe-inverted (as libjpeg-turbo, Chrome and Firefox do).
pub(crate) fn cmyk_to_rgba(
    planes: &[Plane],
    width: u32,
    height: u32,
    ycck: bool,
    icc: Option<&[u8]>,
) -> Plane {
    let w = width as usize;
    let managed = crate::cmyk_hook::converter(icc);
    let h = height as usize;
    let mut data = vec![0u8; w * h * 4];
    let f = factors(planes);
    if w > 0 {
        data.par_chunks_mut(w * 4).enumerate().for_each_init(
            || {
                (
                    [vec![0u8; w], vec![0u8; w], vec![0u8; w], vec![0u8; w]],
                    Vec::new(),
                    Vec::new(),
                )
            },
            |(rows, tmp, ink), (y, o)| {
                for i in 0..4 {
                    upsample_row(
                        &planes[i],
                        f[i],
                        y,
                        w,
                        Upsampling::Bilinear,
                        tmp,
                        &mut rows[i],
                    );
                }
                if let Some(conv) = &managed {
                    // Ink amounts (0 = none) from Adobe-inverted CMYK, or from YCCK (whose
                    // YCC part decodes to inverted C, M, Y).
                    ink.resize(w * 4, 0);
                    for (x, i) in ink.chunks_exact_mut(4).enumerate() {
                        let (c, m, yy, k) = (rows[0][x], rows[1][x], rows[2][x], rows[3][x]);
                        let (c, m, yy) = if ycck {
                            ycc_to_rgb(c, m, yy)
                        } else {
                            (255 - c, 255 - m, 255 - yy)
                        };
                        i.copy_from_slice(&[c, m, yy, 255 - k]);
                    }
                    conv(ink.as_slice(), o);
                    return;
                }
                for (x, px) in o.chunks_exact_mut(4).enumerate() {
                    let (mut c, mut m, mut yy, k) = (
                        rows[0][x] as u32,
                        rows[1][x] as u32,
                        rows[2][x] as u32,
                        rows[3][x] as u32,
                    );
                    if ycck {
                        // libjpeg: C = 255 - R(ycc) etc., K unchanged; result is Adobe-inverted CMYK.
                        let (r, g, b) = ycc_to_rgb(c as u8, m as u8, yy as u8);
                        c = 255 - r as u32;
                        m = 255 - g as u32;
                        yy = 255 - b as u32;
                    }
                    // Stored CMYK is taken as Adobe-inverted (255 = no ink) whether or not an Adobe
                    // marker is present, like libjpeg-turbo's djpeg, Chrome and Firefox: in practice
                    // CMYK JPEGs come from Photoshop-style writers.
                    let div = |a: u32| ((a * k + 127) / 255) as u8;
                    px.copy_from_slice(&[div(c), div(m), div(yy), 255]);
                }
            },
        );
    }
    Plane {
        width,
        height,
        stride: w * 4,
        data,
        h_samp: 1,
        v_samp: 1,
    }
}
