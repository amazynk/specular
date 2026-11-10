//! Nikon High Efficiency raw (HE and HE*; Z 8, Z 9, Z f, Z 6III, Z 50II, Z5II, ...).
//!
//! The sensor data is an intoPIX TicoRAW codestream: JPEG XS (ISO/IEC 21122) with a few
//! changes of its own, which `specular-jpegxs` reads up to the inverse wavelet transform
//! (`decode_coefficients`). What follows here was worked out from the codestreams and checked
//! against Adobe DNG Converter's decodes of the same files (no specification is public):
//!
//! - Four components on a grid half the mosaic's size each way, one per site of the 2×2 cell:
//!   a luma-like value at the top-right green, a red and a blue colour difference, and the
//!   difference of the two greens at the bottom-left green (the one component without a
//!   wavelet transform).
//! - The inverse colour transform is JPEG XS's Star-Tetrix with no extra weights (e1 = e2 = 0),
//!   lifting in that order: the luma green from the green difference, the green difference
//!   from the luma green, both greens from the reds and blues, then the reds and blues from
//!   the greens. Neighbours mirror at the mosaic's edges.
//!   This is not `specular_jpegxs`'s `inverse_tetrix`: the lifting order and the e1/e2
//!   weights differ, so the step lives here.
//! - The mosaic values (with the codestream's 4 fractional bits) are a square-root-like
//!   companding of the linear sensor values; [`linear`] undoes it. Adobe's decode is a
//!   function of exactly these values: ours matches it within one count everywhere tested.

use std::sync::OnceLock;

use rayon::prelude::*;

use crate::error::{RawError, Result};

/// The companding curve, fitted to Adobe DNG Converter 18.4's decodes of 15 files from six
/// bodies (Z 8, Z 9, Z f, Z 6III, Z 50II, Z5II; HE and HE*), which all use the same curve:
/// above the knee the linear value is `(v + V0)² / D + BLACK`, below it (shadows that noise
/// takes under the black level) `BLACK - (v + V0)² / D_TOE`. Over the 49 893 distinct mosaic
/// values of those files it gives Adobe's value for 94% and is one count off for the rest.
const V0: f64 = 16512.9952;
const D: f64 = 157948.8416;
const BLACK: f64 = 1008.12626;
const D_TOE: f64 = 262004.0;
/// Mosaic values outside ±2^17 are clamped (the curve already clips there).
const RANGE: i32 = 1 << 17;
const MAX: f64 = 16383.0;

/// Linear sensor value of a reconstructed mosaic value.
pub(crate) fn linear(v: i32) -> u16 {
    let t = f64::from(v) + V0;
    let l = if t >= 0.0 {
        t * t / D + BLACK
    } else {
        BLACK - t * t / D_TOE
    };
    l.round().clamp(0.0, MAX) as u16
}

fn lut() -> &'static [u16] {
    static LUT: OnceLock<Vec<u16>> = OnceLock::new();
    LUT.get_or_init(|| (-RANGE..RANGE).map(linear).collect())
}

fn map(v: i32) -> u16 {
    lut()[(v.clamp(-RANGE, RANGE - 1) + RANGE) as usize]
}

/// Plain-language reasons for the codestreams this decoder cannot read.
fn refuse(e: specular_jpegxs::Error) -> RawError {
    match e {
        // The coefficient planes reserve 10 bytes per sample.
        specular_jpegxs::Error::NeedsMemory(n) => RawError::TooLarge(n.needed / 10),
        specular_jpegxs::Error::TooLarge => RawError::TooLarge(specular_jpegxs::MAX_SAMPLES),
        specular_jpegxs::Error::Unsupported(_) => RawError::Unsupported(
            "Nikon High Efficiency raw: this variant of the codec is not supported",
        ),
        specular_jpegxs::Error::NotRecognised => {
            RawError::Malformed("Nikon High Efficiency raw: the sensor data is not a codestream")
        }
        specular_jpegxs::Error::Truncated(_) | specular_jpegxs::Error::Malformed(_) => {
            RawError::Malformed("Nikon High Efficiency raw: damaged codestream header")
        }
    }
}

/// Whether `src` starts like a TicoRAW codestream (JPEG XS SOC then CAP).
pub(crate) fn probe(src: &[u8]) -> bool {
    src.starts_with(&[0xFF, 0x10, 0xFF, 0x50])
}

/// Decodes the `w`×`h` mosaic from the codestream `src` into `out`; returns whether the
/// codestream ended early or was damaged (the rest stays at its decoded or black state).
pub(crate) fn decode(
    src: &[u8],
    w: usize,
    h: usize,
    out: &mut [u16],
    parallel: bool,
) -> Result<bool> {
    let c = specular_jpegxs::decode_coefficients(src).map_err(refuse)?;
    let hd = &c.header;
    if !hd.ticoraw() || c.planes.len() != 4 {
        return Err(RawError::Unsupported(
            "Nikon High Efficiency raw: this variant of the codec is not supported",
        ));
    }
    let (cw, ch) = (c.planes[0].0, c.planes[0].1);
    if c.planes.iter().any(|p| (p.0, p.1) != (cw, ch))
        || 2 * cw != hd.width as usize
        || 2 * ch != hd.height as usize
        || cw == 0
        || ch == 0
    {
        return Err(RawError::Unsupported(
            "Nikon High Efficiency raw: this variant of the codec is not supported",
        ));
    }
    if out.len() < w * h {
        return Err(RawError::Malformed("sensor buffer too small"));
    }
    let [y, cr, dg, cb] = <[_; 4]>::try_from(c.planes.into_iter().map(|p| p.2).collect::<Vec<_>>())
        .map_err(|_| RawError::Malformed("Nikon High Efficiency raw: components"))?;
    let mut p = Planes {
        w: cw,
        h: ch,
        q: [cr, y, dg, cb],
        parallel,
    };
    p.inverse_tetrix();
    // Mosaic sites outside the codestream's picture stay black.
    let black = map(-V0 as i32);
    let (mw, mh) = ((2 * cw).min(w), (2 * ch).min(h));
    let out_row = |(yy, row): (usize, &mut [u16])| {
        if yy >= mh {
            row.fill(black);
            return;
        }
        let (y, half) = (yy & 1, (yy >> 1) * p.w);
        let (even, odd) = (&p.q[2 * y][half..], &p.q[2 * y + 1][half..]);
        for (xx, o) in row.iter_mut().enumerate() {
            *o = if xx >= mw {
                black
            } else if xx & 1 == 0 {
                map(even[xx >> 1])
            } else {
                map(odd[xx >> 1])
            };
        }
    };
    if parallel {
        out[..w * h].par_chunks_mut(w).enumerate().for_each(out_row);
    } else {
        out[..w * h].chunks_mut(w).enumerate().for_each(out_row);
    }
    Ok(c.truncated)
}

/// The four sites of the 2×2 cell as planes: `q[2 * (y & 1) + (x & 1)]` holds mosaic site
/// (x, y): red-difference, luma green, green difference, blue-difference.
struct Planes {
    w: usize,
    h: usize,
    q: [Vec<i32>; 4],
    /// Rows in parallel (off for thumbnail and cull workers, which run many decodes at once).
    parallel: bool,
}

/// Site indices.
const CR: usize = 0;
const Y: usize = 1;
const DG: usize = 2;
const CB: usize = 3;

/// A mosaic coordinate mirrored into `0..n` (without repeating the edge sample).
fn mirror(v: isize, n: isize) -> usize {
    let v = if v < 0 { -v } else { v };
    let v = if v >= n { 2 * (n - 1) - v } else { v };
    v.clamp(0, n - 1) as usize
}

impl Planes {
    /// The value at mosaic site (x, y), mirrored at the edges.
    fn get(&self, x: isize, y: isize) -> i32 {
        let x = mirror(x, 2 * self.w as isize);
        let y = mirror(y, 2 * self.h as isize);
        self.q[2 * (y & 1) + (x & 1)][(y >> 1) * self.w + (x >> 1)]
    }

    /// One lifting step: every sample of site `s` gets `f(sample, neighbour sum)`, where the
    /// neighbours are those at the offsets `nb` (none of them on site `s`).
    fn step(&mut self, s: usize, nb: &[(isize, isize)], f: impl Fn(i32, i32) -> i32 + Sync) {
        let (sx, sy) = ((s & 1) as isize, (s >> 1) as isize);
        let mut target = std::mem::take(&mut self.q[s]);
        let this = &*self;
        let w = this.w;
        // Away from the edges each neighbour is a fixed plane at a fixed offset.
        let direct: Vec<(usize, isize, isize)> = nb
            .iter()
            .map(|&(dx, dy)| {
                let (nx, ny) = (sx + dx, sy + dy);
                (
                    (2 * (ny & 1) + (nx & 1)) as usize,
                    nx.div_euclid(2),
                    ny.div_euclid(2),
                )
            })
            .collect();
        let row_step = |(j, row): (usize, &mut [i32])| {
            let my = 2 * j as isize + sy;
            let inner_row = j >= 1 && j + 1 < this.h;
            for (i, v) in row.iter_mut().enumerate() {
                let sum = if inner_row && i >= 1 && i + 1 < w {
                    direct.iter().fold(0i32, |a, &(q, ox, oy)| {
                        let at = (j as isize + oy) as usize * w + (i as isize + ox) as usize;
                        a.wrapping_add(this.q[q][at])
                    })
                } else {
                    let mx = 2 * i as isize + sx;
                    nb.iter().fold(0i32, |a, &(dx, dy)| {
                        a.wrapping_add(this.get(mx + dx, my + dy))
                    })
                };
                *v = f(*v, sum);
            }
        };
        if this.parallel {
            target.par_chunks_mut(w).enumerate().for_each(row_step);
        } else {
            target.chunks_mut(w).enumerate().for_each(row_step);
        }
        self.q[s] = target;
    }

    /// Star-Tetrix inverse, e1 = e2 = 0.
    fn inverse_tetrix(&mut self) {
        const DIAG: [(isize, isize); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
        const CROSS: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        self.step(Y, &DIAG, |v, s| v.wrapping_sub(s >> 3));
        self.step(DG, &DIAG, |v, s| v.wrapping_add(s >> 2));
        self.step(Y, &CROSS, |v, s| v.wrapping_sub(s >> 3));
        self.step(DG, &CROSS, |v, s| v.wrapping_sub(s >> 3));
        self.step(CR, &CROSS, |v, s| v.wrapping_add(s >> 2));
        self.step(CB, &CROSS, |v, s| v.wrapping_add(s >> 2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The forward transform the inverse undoes (lifting steps in reverse order, opposite
    /// signs), on the companded mosaic.
    fn forward(p: &mut Planes) {
        const DIAG: [(isize, isize); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
        const CROSS: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        p.step(CB, &CROSS, |v, s| v.wrapping_sub(s >> 2));
        p.step(CR, &CROSS, |v, s| v.wrapping_sub(s >> 2));
        p.step(DG, &CROSS, |v, s| v.wrapping_add(s >> 3));
        p.step(Y, &CROSS, |v, s| v.wrapping_add(s >> 3));
        p.step(DG, &DIAG, |v, s| v.wrapping_sub(s >> 2));
        p.step(Y, &DIAG, |v, s| v.wrapping_add(s >> 3));
    }

    #[test]
    fn tetrix_round_trip() {
        let (w, h) = (7, 5);
        let mut seed = 0x1234_5678u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 60000) as i32 - 20000
        };
        let q: [Vec<i32>; 4] = std::array::from_fn(|_| (0..w * h).map(|_| rnd()).collect());
        let mut p = Planes {
            w,
            h,
            q: q.clone(),
            parallel: true,
        };
        forward(&mut p);
        assert_ne!(p.q, q);
        p.inverse_tetrix();
        assert_eq!(p.q, q);
    }

    /// The lifting step with every neighbour read through the mirrored accessor.
    fn naive_step(p: &mut Planes, s: usize, nb: &[(isize, isize)], f: impl Fn(i32, i32) -> i32) {
        let (sx, sy) = ((s & 1) as isize, (s >> 1) as isize);
        let mut out = p.q[s].clone();
        for j in 0..p.h {
            for i in 0..p.w {
                let (mx, my) = (2 * i as isize + sx, 2 * j as isize + sy);
                let sum = nb
                    .iter()
                    .fold(0i32, |a, &(dx, dy)| a.wrapping_add(p.get(mx + dx, my + dy)));
                out[j * p.w + i] = f(out[j * p.w + i], sum);
            }
        }
        p.q[s] = out;
    }

    #[test]
    fn fast_path_matches_mirrored_reads() {
        let (w, h) = (9, 6);
        let q: [Vec<i32>; 4] = std::array::from_fn(|k| {
            (0..w * h)
                .map(|i| ((i * 7919 + k * 104_729) % 4001) as i32 - 2000)
                .collect()
        });
        const DIAG: [(isize, isize); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
        const CROSS: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        for (s, nb) in [
            (Y, DIAG),
            (DG, DIAG),
            (Y, CROSS),
            (DG, CROSS),
            (CR, CROSS),
            (CB, CROSS),
        ] {
            let new = |parallel| Planes {
                w,
                h,
                q: q.clone(),
                parallel,
            };
            let (mut a, mut serial, mut b) = (new(true), new(false), new(false));
            a.step(s, &nb, |v, s| v.wrapping_sub(s >> 3));
            serial.step(s, &nb, |v, s| v.wrapping_sub(s >> 3));
            naive_step(&mut b, s, &nb, |v, s| v.wrapping_sub(s >> 3));
            assert_eq!(a.q, b.q, "site {s}");
            assert_eq!(serial.q, b.q, "site {s}, one thread");
        }
    }

    #[test]
    fn mirror_edges() {
        assert_eq!(mirror(-1, 8), 1);
        assert_eq!(mirror(8, 8), 6);
        assert_eq!(mirror(0, 8), 0);
        assert_eq!(mirror(7, 8), 7);
    }

    #[test]
    fn curve() {
        // Values Adobe DNG Converter gives for these mosaic values (toe, knee, body).
        for (v, l) in [(-22060, 891), (-19827, 966), (-16513, 1008), (-12000, 1137)] {
            assert_eq!(linear(v), l, "{v}");
        }
        for (v, l) in [(12000, 6155), (20709, 9780)] {
            assert_eq!(linear(v), l, "{v}");
        }
        // Monotonic, the 14-bit clip at the top.
        assert_eq!(linear(1 << 16), 16383);
        let mut last = 0;
        for v in -RANGE..RANGE {
            let l = linear(v);
            assert!(l >= last, "{v}");
            last = l;
        }
        assert_eq!(map(i32::MIN), map(-RANGE));
        assert_eq!(map(i32::MAX), 16383);
    }
}
