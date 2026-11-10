//! The reference picture: padded planes with the three half-sample planes precomputed (the
//! 6-tap filter of 8.4.2.2.1), and motion-compensated prediction from them.
//!
//! Padding replicates the edge samples, which is exactly the coordinate clamping of the
//! standard, so any block whose support stays inside the padded area predicts as a decoder
//! does. Motion vectors are kept inside [`mv_range`].

use rayon::prelude::*;
use std::simd::Simd;

type U8x16 = Simd<u8, 16>;

/// Luma padding in samples on each side.
pub const PAD: usize = 64;
/// Chroma padding.
pub const PADC: usize = 40;

pub struct RefFrame {
    /// Macroblock-aligned luma size.
    pub w: usize,
    pub h: usize,
    pub stride: usize,
    /// Full-sample luma (G).
    pub f: Vec<u8>,
    /// Horizontal half samples: `hp[x, y]` is b at (x + ½, y).
    pub hp: Vec<u8>,
    /// Vertical half samples: `vp[x, y]` is h at (x, y + ½).
    pub vp: Vec<u8>,
    /// Centre half samples: `cp[x, y]` is j at (x + ½, y + ½).
    pub cp: Vec<u8>,
    pub cstride: usize,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

#[inline]
fn tap6(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> i32 {
    a - 5 * b + 20 * c + 20 * d - 5 * e + f
}

#[inline]
fn clip8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Copies a `w`×`h` plane into a padded buffer, replicating edges.
fn pad_plane(src: &[u8], w: usize, h: usize, pad: usize) -> Vec<u8> {
    let stride = w + 2 * pad;
    let rows = h + 2 * pad;
    let mut out = vec![0u8; stride * rows];
    out.par_chunks_mut(stride).enumerate().for_each(|(y, row)| {
        let sy = y.saturating_sub(pad).min(h - 1);
        let s = &src[sy * w..sy * w + w];
        row[..pad].fill(s[0]);
        row[pad..pad + w].copy_from_slice(s);
        row[pad + w..].fill(s[w - 1]);
    });
    out
}

impl RefFrame {
    /// Builds the reference from reconstructed (deblocked) planes of macroblock-aligned size
    /// `w`×`h` (chroma `w/2`×`h/2`).
    pub fn build(y: &[u8], u: &[u8], v: &[u8], w: usize, h: usize) -> RefFrame {
        let stride = w + 2 * PAD;
        let rows = h + 2 * PAD;
        let f = pad_plane(y, w, h, PAD);
        let (cw, ch) = (w / 2, h / 2);
        let (u, v) = rayon::join(|| pad_plane(u, cw, ch, PADC), || pad_plane(v, cw, ch, PADC));
        let last_x = stride as isize - 1;
        let last_y = rows as isize - 1;
        // Unrounded horizontal sums b1 (they fit i16: −2550..=10710).
        let mut b1 = vec![0i16; stride * rows];
        let mut hp = vec![0u8; stride * rows];
        b1.par_chunks_mut(stride)
            .zip(hp.par_chunks_mut(stride))
            .enumerate()
            .for_each(|(yy, (brow, hrow))| {
                let r = &f[yy * stride..yy * stride + stride];
                let at = |x: isize| i32::from(r[x.clamp(0, last_x) as usize]);
                let mut put = |x: usize, s: i32| {
                    brow[x] = s as i16;
                    hrow[x] = clip8((s + 16) >> 5);
                };
                for x in (0..2).chain(stride - 3..stride) {
                    let xi = x as isize;
                    let s = tap6(
                        at(xi - 2),
                        at(xi - 1),
                        at(xi),
                        at(xi + 1),
                        at(xi + 2),
                        at(xi + 3),
                    );
                    put(x, s);
                }
                // The interior from six shifted views of the row (vectorises).
                let n = stride - 5;
                let t: [&[u8]; 6] = std::array::from_fn(|k| &r[k..k + n]);
                let (bo, ho) = (&mut brow[2..2 + n], &mut hrow[2..2 + n]);
                for x in 0..n {
                    let g = |k: usize| i32::from(t[k][x]);
                    let s = tap6(g(0), g(1), g(2), g(3), g(4), g(5));
                    bo[x] = s as i16;
                    ho[x] = clip8((s + 16) >> 5);
                }
            });
        let mut vp = vec![0u8; stride * rows];
        let mut cp = vec![0u8; stride * rows];
        vp.par_chunks_mut(stride)
            .zip(cp.par_chunks_mut(stride))
            .enumerate()
            .for_each(|(yy, (vrow, crow))| {
                let ry = |d: isize| ((yy as isize + d).clamp(0, last_y) as usize) * stride;
                let o = [ry(-2), ry(-1), ry(0), ry(1), ry(2), ry(3)];
                let t: [&[u8]; 6] = o.map(|o| &f[o..o + stride]);
                let tb: [&[i16]; 6] = o.map(|o| &b1[o..o + stride]);
                let (vrow, crow) = (&mut vrow[..stride], &mut crow[..stride]);
                for x in 0..stride {
                    let g = |k: usize| i32::from(t[k][x]);
                    let s = tap6(g(0), g(1), g(2), g(3), g(4), g(5));
                    vrow[x] = clip8((s + 16) >> 5);
                    let bb = |k: usize| i32::from(tb[k][x]);
                    let j = tap6(bb(0), bb(1), bb(2), bb(3), bb(4), bb(5));
                    crow[x] = clip8((j + 512) >> 10);
                }
            });
        RefFrame {
            w,
            h,
            stride,
            f,
            hp,
            vp,
            cp,
            cstride: cw + 2 * PADC,
            u,
            v,
        }
    }

    /// Offset of luma sample (x, y) (picture coordinates; may be negative within the pad).
    #[inline]
    pub fn off(&self, x: isize, y: isize) -> usize {
        ((y + PAD as isize) as usize) * self.stride + (x + PAD as isize) as usize
    }

    /// The full-sample luma plane from (x, y).
    #[inline]
    pub fn full_at(&self, x: isize, y: isize) -> &[u8] {
        &self.f[self.off(x, y)..]
    }

    /// Predicts a `bw`×`bh` luma block whose top-left is at quarter-sample position
    /// (qx, qy), into `out` (stride `bw`).
    pub fn luma(&self, qx: i32, qy: i32, bw: usize, bh: usize, out: &mut [u8]) {
        let (ix, iy) = ((qx >> 2) as isize, (qy >> 2) as isize);
        let (fx, fy) = (qx & 3, qy & 3);
        // (plane, dx, dy) pairs; one entry when the position is a plane sample itself.
        let (f, h, v, c) = (&self.f, &self.hp, &self.vp, &self.cp);
        type P<'a> = (&'a Vec<u8>, isize, isize);
        let (a, b): (P, Option<P>) = match (fx, fy) {
            (0, 0) => ((f, 0, 0), None),
            (1, 0) => ((f, 0, 0), Some((h, 0, 0))),
            (2, 0) => ((h, 0, 0), None),
            (3, 0) => ((f, 1, 0), Some((h, 0, 0))),
            (0, 1) => ((f, 0, 0), Some((v, 0, 0))),
            (0, 2) => ((v, 0, 0), None),
            (0, 3) => ((f, 0, 1), Some((v, 0, 0))),
            (1, 1) => ((h, 0, 0), Some((v, 0, 0))),
            (3, 1) => ((h, 0, 0), Some((v, 1, 0))),
            (1, 3) => ((v, 0, 0), Some((h, 0, 1))),
            (3, 3) => ((v, 1, 0), Some((h, 0, 1))),
            (2, 1) => ((h, 0, 0), Some((c, 0, 0))),
            (2, 3) => ((h, 0, 1), Some((c, 0, 0))),
            (1, 2) => ((v, 0, 0), Some((c, 0, 0))),
            (3, 2) => ((v, 1, 0), Some((c, 0, 0))),
            _ => ((c, 0, 0), None),
        };
        let s = self.stride;
        // The rows the block reads, sliced once so the loops index plain slices.
        let len = (bh - 1) * s + bw;
        let oa = self.off(ix + a.1, iy + a.2);
        let ra = &a.0[oa..oa + len];
        let rb = match b {
            Some((p, dx, dy)) => {
                let o = self.off(ix + dx, iy + dy);
                Some(&p[o..o + len])
            }
            None => None,
        };
        match rb {
            None => {
                for (o, r) in out.chunks_exact_mut(bw).take(bh).zip(ra.chunks(s)) {
                    o.copy_from_slice(&r[..bw]);
                }
            }
            Some(rb) if bw == 16 => {
                let rows = ra.chunks(s).zip(rb.chunks(s));
                for (o, (a, b)) in out.chunks_exact_mut(16).take(bh).zip(rows) {
                    let (a, b) = (U8x16::from_slice(a), U8x16::from_slice(b));
                    // (a + b + 1) >> 1 without widening.
                    let avg = (a | b) - ((a ^ b) >> 1);
                    avg.copy_to_slice(o);
                }
            }
            Some(rb) => {
                let rows = ra.chunks(s).zip(rb.chunks(s));
                for (o, (a, b)) in out.chunks_exact_mut(bw).take(bh).zip(rows) {
                    for ((o, &p), &q) in o.iter_mut().zip(a).zip(b) {
                        *o = ((u16::from(p) + u16::from(q) + 1) >> 1) as u8;
                    }
                }
            }
        }
    }

    /// Predicts a `bw`×`bh` chroma block of `plane` (0 Cb, 1 Cr) whose top-left is at
    /// eighth-sample position (ex, ey) (8.4.2.2.2).
    pub fn chroma(&self, plane: usize, ex: i32, ey: i32, bw: usize, bh: usize, out: &mut [u8]) {
        let src = if plane == 0 { &self.u } else { &self.v };
        let (ix, iy) = ((ex >> 3) as isize, (ey >> 3) as isize);
        let (fx, fy) = ((ex & 7) as u16, (ey & 7) as u16);
        let s = self.cstride;
        let o = ((iy + PADC as isize) as usize) * s + (ix + PADC as isize) as usize;
        let (wa, wb, wc, wd) = ((8 - fx) * (8 - fy), fx * (8 - fy), (8 - fx) * fy, fx * fy);
        for (y, row) in out[..bw * bh].chunks_exact_mut(bw).enumerate() {
            let r0 = &src[o + y * s..o + y * s + bw + 1];
            let r1 = &src[o + (y + 1) * s..o + (y + 1) * s + bw + 1];
            for (x, px) in row.iter_mut().enumerate() {
                let v = wa * u16::from(r0[x])
                    + wb * u16::from(r0[x + 1])
                    + wc * u16::from(r1[x])
                    + wd * u16::from(r1[x + 1]);
                *px = ((v + 32) >> 6) as u8;
            }
        }
    }
}

/// The motion vector range (quarter samples, inclusive) that keeps a 16×16 block at
/// macroblock (mbx, mby) inside the padded reference, for both luma and chroma.
pub fn mv_range(mbx: usize, mby: usize, w: usize, h: usize) -> (i32, i32, i32, i32) {
    let margin = PAD as i32 - 8;
    let x0 = (mbx * 16) as i32;
    let y0 = (mby * 16) as i32;
    (
        (-margin - x0) * 4,
        (w as i32 + margin - 16 - x0) * 4,
        (-margin - y0) * 4,
        (h as i32 + margin - 16 - y0) * 4,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Straight 6-tap interpolation with clamped coordinates, as the standard writes it.
    fn reference_sample(y: &[u8], w: usize, h: usize, qx: i32, qy: i32) -> u8 {
        let g = |x: i32, yy: i32| -> i32 {
            i32::from(
                y[(yy.clamp(0, h as i32 - 1) as usize) * w + x.clamp(0, w as i32 - 1) as usize],
            )
        };
        let b1 = |x: i32, yy: i32| {
            tap6(
                g(x - 2, yy),
                g(x - 1, yy),
                g(x, yy),
                g(x + 1, yy),
                g(x + 2, yy),
                g(x + 3, yy),
            )
        };
        let h1 = |x: i32, yy: i32| {
            tap6(
                g(x, yy - 2),
                g(x, yy - 1),
                g(x, yy),
                g(x, yy + 1),
                g(x, yy + 2),
                g(x, yy + 3),
            )
        };
        let bb = |x: i32, yy: i32| clip8((b1(x, yy) + 16) >> 5) as i32;
        let hh = |x: i32, yy: i32| clip8((h1(x, yy) + 16) >> 5) as i32;
        let jj = |x: i32, yy: i32| {
            let j1 = tap6(
                b1(x, yy - 2),
                b1(x, yy - 1),
                b1(x, yy),
                b1(x, yy + 1),
                b1(x, yy + 2),
                b1(x, yy + 3),
            );
            clip8((j1 + 512) >> 10) as i32
        };
        let (x, yy, fx, fy) = (qx >> 2, qy >> 2, qx & 3, qy & 3);
        let av = |a: i32, b: i32| ((a + b + 1) >> 1) as u8;
        match (fx, fy) {
            (0, 0) => g(x, yy) as u8,
            (1, 0) => av(g(x, yy), bb(x, yy)),
            (2, 0) => bb(x, yy) as u8,
            (3, 0) => av(g(x + 1, yy), bb(x, yy)),
            (0, 1) => av(g(x, yy), hh(x, yy)),
            (0, 2) => hh(x, yy) as u8,
            (0, 3) => av(g(x, yy + 1), hh(x, yy)),
            (1, 1) => av(bb(x, yy), hh(x, yy)),
            (3, 1) => av(bb(x, yy), hh(x + 1, yy)),
            (1, 3) => av(hh(x, yy), bb(x, yy + 1)),
            (3, 3) => av(hh(x + 1, yy), bb(x, yy + 1)),
            (2, 1) => av(bb(x, yy), jj(x, yy)),
            (2, 3) => av(bb(x, yy + 1), jj(x, yy)),
            (1, 2) => av(hh(x, yy), jj(x, yy)),
            (3, 2) => av(hh(x + 1, yy), jj(x, yy)),
            _ => jj(x, yy) as u8,
        }
    }

    #[test]
    fn half_planes_match_direct_interpolation() {
        let (w, h) = (32usize, 32usize);
        let mut s = 12345u32;
        let y: Vec<u8> = (0..w * h)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect();
        let c = vec![128u8; w * h / 4];
        let r = RefFrame::build(&y, &c, &c, w, h);
        let mut out = [0u8; 16];
        for qy in -40..(h as i32 * 4 + 20) {
            for qx in (-40..(w as i32 * 4 + 20)).step_by(3) {
                r.luma(qx, qy, 1, 1, &mut out);
                assert_eq!(out[0], reference_sample(&y, w, h, qx, qy), "{qx},{qy}");
            }
        }
    }
}
