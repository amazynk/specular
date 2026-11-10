//! Motion compensation: fractional sample interpolation (8.5.3.3.3) and
//! weighted sample prediction (8.5.3.3.4).
//!
//! A block whose filter taps stay inside the reference picture's columns is
//! filtered straight from its rows (rows above or below the picture being
//! its first or last one); any other is first copied, with the coordinates
//! clamped to the picture as the specification's `Clip3` does per sample,
//! into a small buffer, and filtered from there. Either way the reference
//! rows the taps reach are waited for first: the reference may still be
//! decoding on another thread. The filters are instantiated per block width
//! (every prediction block width is one of a handful), so each row is a
//! fixed number of vector operations. A whole-sample, unweighted block
//! predicted from one list is the reference samples themselves and is
//! copied straight into the picture.

use super::super::state::Dec;
use super::RefPic;
use crate::picture::{MvField, Plane};
use alloc::boxed::Box;

/// Largest prediction block side.
const MAX: usize = 64;

/// `fL[xFracL][i]` (Table 8-11), fractions 1 to 3.
const LUMA: [[i32; 8]; 4] = [
    [0, 0, 0, 64, 0, 0, 0, 0],
    [-1, 4, -10, 58, 17, -5, 1, 0],
    [-1, 4, -11, 40, 40, -11, 4, -1],
    [0, 1, -5, 17, 58, -10, 4, -1],
];

/// `fC[xFracC][i]` (Table 8-12), fractions 1 to 7.
const CHROMA: [[i32; 4]; 8] = [
    [0, 64, 0, 0],
    [-2, 58, 10, -2],
    [-4, 54, 16, -2],
    [-6, 46, 28, -4],
    [-4, 36, 36, -4],
    [-4, 28, 46, -6],
    [-2, 16, 54, -4],
    [-2, 10, 58, -2],
];

/// An intermediate prediction sample (`predSamplesLX`): 16 bits hold it up to 12-bit video
/// (14-bit precision); deeper video (RExt, up to 16 bits: shift3 stays 2, so up to 19 bits
/// with the filter's gain) takes 32.
trait Mid: Copy + Default {
    fn of(v: i32) -> Self;
    fn get(self) -> i32;
}

impl Mid for i16 {
    #[inline(always)]
    fn of(v: i32) -> i16 {
        v as i16
    }
    #[inline(always)]
    fn get(self) -> i32 {
        i32::from(self)
    }
}

impl Mid for i32 {
    #[inline(always)]
    fn of(v: i32) -> i32 {
        v
    }
    #[inline(always)]
    fn get(self) -> i32 {
        self
    }
}

/// Intermediate buffers of one prediction block.
struct Mids<M> {
    /// Intermediate prediction from list 0, or the only one.
    a: [M; MAX * MAX],
    /// Intermediate prediction from list 1.
    b: [M; MAX * MAX],
    /// Horizontally filtered rows for the two-dimensional case.
    tmp: [M; (MAX + 7) * MAX],
}

impl<M: Mid> Mids<M> {
    fn new() -> Box<Mids<M>> {
        Box::new(Mids {
            a: [M::default(); MAX * MAX],
            b: [M::default(); MAX * MAX],
            tmp: [M::default(); (MAX + 7) * MAX],
        })
    }
}

/// Scratch buffers for one prediction block.
pub struct McScratch {
    /// Up to 12 bits.
    narrow: Box<Mids<i16>>,
    /// Deeper video, allocated when first needed.
    wide: Option<Box<Mids<i32>>>,
    /// Edge-emulated reference samples.
    emu: [u16; (MAX + 7) * (MAX + 7)],
}

impl McScratch {
    /// Allocates the buffers.
    pub fn new() -> Box<McScratch> {
        Box::new(McScratch {
            narrow: Mids::new(),
            wide: None,
            emu: [0; (MAX + 7) * (MAX + 7)],
        })
    }
}

/// The shifts of a component `bd` bits deep: interpolation's shift1 and shift3
/// (8.5.3.3.3.1), and weighted prediction's shift1 (8.5.3.3.4.2, 8.5.3.3.4.3).
#[inline]
fn shifts(bd: u32) -> (u32, u32, i32) {
    (bd.min(12) - 8, 14 - bd.min(12), 14 - bd.min(12) as i32)
}

/// One list's contribution to a component: the reference, the integer
/// position of the block in it and the fractional offset.
#[derive(Clone, Copy)]
struct ListSrc<'r> {
    pic: &'r RefPic,
    pos: (isize, isize),
    frac: (usize, usize),
}

/// Predicts the `w` x `h` luma block at `(x, y)` and its chroma from the
/// motion `m`, writing the prediction samples into the picture.
pub fn predict(d: &mut Dec<'_>, x: usize, y: usize, w: usize, h: usize, m: &MvField) {
    let (Some(ctx), Some(mut sc)) = (d.ictx, d.mc.take()) else {
        return;
    };
    let comps = if d.cat == 0 { 1 } else { 3 };
    for c in 0..comps {
        let (sw, sh) = if c == 0 {
            (1, 1)
        } else {
            (d.sps.sub_w, d.sps.sub_h)
        };
        let (bx, by, bw, bh) = (x / sw, y / sh, w / sw, h / sh);
        let bd = u32::from(if c == 0 {
            d.sps.bit_depth_y
        } else {
            d.sps.bit_depth_c
        });
        if bw == 0 || bh == 0 || bw > MAX || bh > MAX {
            continue;
        }
        // The lists that predict, in order.
        let mut srcs: [Option<ListSrc<'_>>; 2] = [None, None];
        let mut n = 0;
        for l in 0..2 {
            if !m.pred(l) {
                continue;
            }
            let Some(e) = ctx.refs.get(usize::from(m.ref_id[l])) else {
                continue;
            };
            let mv = m.mv[l];
            let (pos, frac) = if c == 0 {
                (
                    (
                        bx as isize + (isize::from(mv[0]) >> 2),
                        by as isize + (isize::from(mv[1]) >> 2),
                    ),
                    ((mv[0] & 3) as usize, (mv[1] & 3) as usize),
                )
            } else {
                // mvCLX in units of 1/8 chroma sample (8-228, 8-229).
                let mx = i32::from(mv[0]) * 2 / sw as i32;
                let my = i32::from(mv[1]) * 2 / sh as i32;
                (
                    (
                        bx as isize + (mx >> 3) as isize,
                        by as isize + (my >> 3) as isize,
                    ),
                    ((mx & 7) as usize, (my & 7) as usize),
                )
            };
            srcs[n] = Some(ListSrc {
                pic: &e.pic,
                pos,
                frac,
            });
            n += 1;
        }
        let Some(first) = srcs[0] else {
            continue;
        };
        let dst = match c {
            0 => &mut d.pic.y,
            1 => &mut d.pic.cb,
            _ => &mut d.pic.cr,
        };
        let cw = bw.min(dst.width.saturating_sub(bx));
        let ch = bh.min(dst.height.saturating_sub(by));
        if cw == 0 || ch == 0 {
            // Wholly outside the plane (a damaged stream): nothing to write.
            continue;
        }
        let (pw, ph) = first.pic.dims(c);
        if n == 1 && d.sh.weights.is_none() && first.frac == (0, 0) && pw > 0 && ph > 0 {
            // ((s << shift3) + offset1) >> shift1 is s itself.
            copy_block(first.pic, c, first.pos, (cw, ch), dst, (bx, by));
            continue;
        }
        let stride = bw_stride(w / sw);
        let weights = d.sh.weights.as_deref().map(|wt| {
            let r0 = m.ref_idx[0].max(0) as usize;
            let r1 = m.ref_idx[1].max(0) as usize;
            let k = usize::from(c > 0);
            (
                wt.log2_denom[k],
                [wt.w[0][r0.min(15)].w[c], wt.w[1][r1.min(15)].w[c]],
                [wt.w[0][r0.min(15)].o[c], wt.w[1][r1.min(15)].o[c]],
            )
        });
        let job = Comp {
            srcs: &srcs[..n],
            c,
            size: (bw, bh),
            clip: (cw, ch),
            at: (bx, by),
            bd,
            stride,
            weights,
            l1_only: !m.pred(0),
        };
        let McScratch { narrow, wide, emu } = &mut *sc;
        if bd > 12 {
            job.run(wide.get_or_insert_with(Mids::new), emu, dst);
        } else {
            job.run(narrow, emu, dst);
        }
    }
    d.mc = Some(sc);
}

/// One component of a prediction block that is filtered or weighted.
struct Comp<'s, 'r> {
    /// The lists that predict, in order.
    srcs: &'s [Option<ListSrc<'r>>],
    c: usize,
    /// The block in the component's samples, and the part of it inside the plane.
    size: (usize, usize),
    clip: (usize, usize),
    at: (usize, usize),
    bd: u32,
    stride: usize,
    /// log2 denominator, weights and offsets of the two lists (explicit weighting).
    weights: Option<(u32, [i32; 2], [i32; 2])>,
    /// The one list used is list 1.
    l1_only: bool,
}

impl Comp<'_, '_> {
    /// Interpolates each list's prediction into `mids`, then weights them into `dst`.
    fn run<M: Mid>(&self, mids: &mut Mids<M>, emu: &mut [u16], dst: &mut Plane) {
        let (c, bd) = (self.c, self.bd);
        let (bw, bh) = self.size;
        let (sh1, sh3, shift1) = shifts(bd);
        let Mids { a, b, tmp } = mids;
        for (k, s) in self.srcs.iter().flatten().enumerate() {
            let out = if k == 0 { &mut a[..] } else { &mut b[..] };
            let (pw, ph) = s.pic.dims(c);
            let (fx, fy) = s.frac;
            if pw == 0 || ph == 0 {
                out[..bw * bh].fill(M::of(1 << (bd - 1 + sh3)));
            } else if c == 0 {
                let f = (&LUMA[fx], &LUMA[fy]);
                interp::<8, M>(
                    s.pic,
                    c,
                    s.pos,
                    s.frac,
                    (bw, bh),
                    f,
                    (sh1, sh3),
                    out,
                    tmp,
                    emu,
                );
            } else {
                let f = (&CHROMA[fx], &CHROMA[fy]);
                interp::<4, M>(
                    s.pic,
                    c,
                    s.pos,
                    s.frac,
                    (bw, bh),
                    f,
                    (sh1, sh3),
                    out,
                    tmp,
                    emu,
                );
            }
        }
        let (bw, bh) = self.clip;
        let (bx, by) = self.at;
        let stride = self.stride;
        let bi = self.srcs.len() == 2;
        let max = (1i32 << bd) - 1;
        for j in 0..bh {
            let o = dst.offset(bx, by + j);
            let row = &mut dst.data[o..o + bw];
            let pa = &a[j * stride..j * stride + bw];
            let pb = &b[j * stride..j * stride + bw];
            match self.weights {
                None if !bi => {
                    let off = 1i32 << (shift1 - 1);
                    for (s, &p) in row.iter_mut().zip(pa) {
                        *s = ((p.get() + off) >> shift1).clamp(0, max) as u16;
                    }
                }
                None => {
                    let sh2 = shift1 + 1;
                    let off = 1i32 << (sh2 - 1);
                    for ((s, &p), &q) in row.iter_mut().zip(pa).zip(pb) {
                        *s = ((p.get() + q.get() + off) >> sh2).clamp(0, max) as u16;
                    }
                }
                Some((denom, w, o)) => {
                    let log2wd = denom as i32 + shift1;
                    if bi {
                        let (w0, w1) = (w[0], w[1]);
                        let rnd = (o[0] + o[1] + 1) << log2wd;
                        for ((s, &p), &q) in row.iter_mut().zip(pa).zip(pb) {
                            let v = (p.get() * w0 + q.get() * w1 + rnd) >> (log2wd + 1);
                            *s = v.clamp(0, max) as u16;
                        }
                    } else {
                        let l = usize::from(self.l1_only);
                        let (w0, o0) = (w[l], o[l]);
                        for (s, &p) in row.iter_mut().zip(pa) {
                            let v = if log2wd >= 1 {
                                ((p.get() * w0 + (1 << (log2wd - 1))) >> log2wd) + o0
                            } else {
                                p.get() * w0 + o0
                            };
                            *s = v.clamp(0, max) as u16;
                        }
                    }
                }
            }
        }
    }
}

/// Copies the `w` x `h` block of plane `c` of `pic` at `(xi, yi)`, with the
/// coordinates clamped to the picture, to `(bx, by)` of `dst`.
fn copy_block(
    pic: &RefPic,
    c: usize,
    (xi, yi): (isize, isize),
    (w, h): (usize, usize),
    dst: &mut Plane,
    (bx, by): (usize, usize),
) {
    if w == 0 || h == 0 {
        return;
    }
    let (pw, ph) = pic.dims(c);
    let yy = |j: usize| (yi + j as isize).clamp(0, ph as isize - 1) as usize;
    pic.wait_line(c, yy(h - 1));
    if xi < 0 || xi as usize + w > pw {
        for j in 0..h {
            let line = pic.line(c, yy(j));
            let o = dst.offset(bx, by + j);
            for (i, s) in dst.data[o..o + w].iter_mut().enumerate() {
                *s = line[(xi + i as isize).clamp(0, pw as isize - 1) as usize];
            }
        }
        return;
    }
    let x = xi as usize;
    // Rows of a fixed width are copied inline rather than by a library call,
    // which for a few dozen bytes is mostly call overhead.
    match w {
        2 => copy_rows::<2>(pic, c, x, yy, h, dst, (bx, by)),
        4 => copy_rows::<4>(pic, c, x, yy, h, dst, (bx, by)),
        6 => copy_rows::<6>(pic, c, x, yy, h, dst, (bx, by)),
        8 => copy_rows::<8>(pic, c, x, yy, h, dst, (bx, by)),
        12 => copy_rows::<12>(pic, c, x, yy, h, dst, (bx, by)),
        16 => copy_rows::<16>(pic, c, x, yy, h, dst, (bx, by)),
        24 => copy_rows::<24>(pic, c, x, yy, h, dst, (bx, by)),
        32 => copy_rows::<32>(pic, c, x, yy, h, dst, (bx, by)),
        48 => copy_rows::<48>(pic, c, x, yy, h, dst, (bx, by)),
        64 => copy_rows::<64>(pic, c, x, yy, h, dst, (bx, by)),
        _ => {
            for j in 0..h {
                let o = dst.offset(bx, by + j);
                dst.data[o..o + w].copy_from_slice(&pic.line(c, yy(j))[x..x + w]);
            }
        }
    }
}

/// The rows of [`copy_block`] for a block `W` samples wide inside the picture.
#[inline(always)]
fn copy_rows<const W: usize>(
    pic: &RefPic,
    c: usize,
    x: usize,
    yy: impl Fn(usize) -> usize,
    h: usize,
    dst: &mut Plane,
    (bx, by): (usize, usize),
) {
    for j in 0..h {
        let s: &[u16; W] = pic.line(c, yy(j))[x..x + W].try_into().unwrap();
        let o = dst.offset(bx, by + j);
        let d: &mut [u16; W] = (&mut dst.data[o..o + W]).try_into().unwrap();
        *d = *s;
    }
}

/// Row stride of the intermediate prediction of a block `w` samples wide.
#[inline]
fn bw_stride(w: usize) -> usize {
    w.clamp(1, MAX)
}

/// The reference rows a block's filter taps read, `rw` samples each.
#[derive(Clone, Copy)]
struct Rows<'r> {
    pic: &'r RefPic,
    c: usize,
    /// First column (when read from the picture).
    x0: usize,
    /// First row, which may lie above the picture.
    y0: isize,
    /// Height of the plane.
    ph: isize,
    rw: usize,
    /// The edge-emulated rows, when the taps reach outside the picture.
    emu: Option<&'r [u16]>,
}

impl<'r> Rows<'r> {
    /// Row `j` of the taps' window.
    #[inline(always)]
    fn get(&self, j: usize) -> &'r [u16] {
        match self.emu {
            Some(e) => &e[j * self.rw..(j + 1) * self.rw],
            None => {
                // Rows above or below the picture repeat its first or last
                // row, as the specification's per-sample `Clip3` does.
                let y = (self.y0 + j as isize).clamp(0, self.ph - 1) as usize;
                &self.pic.line(self.c, y)[self.x0..self.x0 + self.rw]
            }
        }
    }
}

/// Interpolates a `w` x `h` block of plane `c` of `pic` whose integer
/// position is `(xi, yi)` and fraction `(fx, fy)` with the `N`-tap filters
/// `cx`, `cy`, producing the intermediate samples `predSamplesLX` in
/// `out` (stride `w`). Waits for the reference rows the taps reach.
#[allow(clippy::too_many_arguments)]
fn interp<const N: usize, M: Mid>(
    pic: &RefPic,
    c: usize,
    (xi, yi): (isize, isize),
    frac: (usize, usize),
    (w, h): (usize, usize),
    (cx, cy): (&[i32; N], &[i32; N]),
    (shift1, shift3): (u32, u32),
    out: &mut [M],
    tmp: &mut [M],
    emu: &mut [u16],
) {
    let half = N / 2 - 1;
    let (rw, rh) = (w + N - 1, h + N - 1);
    let (x0, y0) = (xi - half as isize, yi - half as isize);
    let (pw, ph) = pic.dims(c);
    let (pw, ph) = (pw as isize, ph as isize);
    pic.wait_line(c, (y0 + rh as isize - 1).clamp(0, ph - 1) as usize);
    let inside = x0 >= 0 && x0 + rw as isize <= pw;
    if !inside {
        // Columns left or right of the picture repeat its edge columns.
        for j in 0..rh {
            let row = pic.line(c, (y0 + j as isize).clamp(0, ph - 1) as usize);
            let dst = &mut emu[j * rw..(j + 1) * rw];
            let first_in = (-x0).clamp(0, rw as isize) as usize;
            let last_in = (pw - x0).clamp(0, rw as isize) as usize;
            if first_in > 0 {
                dst[..first_in].fill(row[0]);
            }
            if last_in > first_in {
                let sx = (x0 + first_in as isize) as usize;
                dst[first_in..last_in].copy_from_slice(&row[sx..sx + last_in - first_in]);
            }
            if last_in < rw {
                dst[last_in.max(first_in)..].fill(row[row.len() - 1]);
            }
        }
    }
    let emu: &[u16] = emu;
    let rows = Rows {
        pic,
        c,
        x0: x0.max(0) as usize,
        y0,
        ph,
        rw,
        emu: (!inside).then_some(emu),
    };
    let k = Kernel {
        rows,
        frac,
        cx,
        cy,
        shift1,
        shift3,
        h,
        stride: w,
    };
    match w {
        2 => k.run::<2, M>(0, out, tmp),
        4 => k.run::<4, M>(0, out, tmp),
        6 => k.run::<6, M>(0, out, tmp),
        8 => k.run::<8, M>(0, out, tmp),
        12 => k.run::<12, M>(0, out, tmp),
        16 => k.run::<16, M>(0, out, tmp),
        24 => k.run::<24, M>(0, out, tmp),
        32 => k.run::<32, M>(0, out, tmp),
        48 => k.run::<48, M>(0, out, tmp),
        64 => k.run::<64, M>(0, out, tmp),
        _ => {
            // Not a prediction block width; filtered a column pair at a time.
            for x in (0..w & !1).step_by(2) {
                k.run::<2, M>(x, out, tmp);
            }
            if w & 1 == 1 {
                k.run::<1, M>(w - 1, out, tmp);
            }
        }
    }
}

/// The filters of one block, applied a fixed number of columns at a time.
struct Kernel<'r, const N: usize> {
    rows: Rows<'r>,
    frac: (usize, usize),
    cx: &'r [i32; N],
    cy: &'r [i32; N],
    shift1: u32,
    shift3: u32,
    h: usize,
    /// Row stride of the output.
    stride: usize,
}

impl<const N: usize> Kernel<'_, N> {
    /// Filters columns `x .. x + W` of the block into `out`.
    #[inline(always)]
    fn run<const W: usize, M: Mid>(&self, x: usize, out: &mut [M], tmp: &mut [M]) {
        let half = N / 2 - 1;
        let (h, st, sh1) = (self.h, self.stride, self.shift1);
        let put = |out: &mut [M], j: usize, acc: &[i32; W], s: u32| {
            let o: &mut [M; W] = (&mut out[j * st + x..][..W]).try_into().unwrap();
            for i in 0..W {
                o[i] = M::of(acc[i] >> s);
            }
        };
        match self.frac {
            (0, 0) => {
                for j in 0..h {
                    let s: &[u16; W] = self.rows.get(j + half)[x + half..][..W].try_into().unwrap();
                    let o: &mut [M; W] = (&mut out[j * st + x..][..W]).try_into().unwrap();
                    for i in 0..W {
                        o[i] = M::of(i32::from(s[i]) << self.shift3);
                    }
                }
            }
            (_, 0) => {
                for j in 0..h {
                    let acc = hfilt::<N, W>(&self.rows.get(j + half)[x..], self.cx);
                    put(out, j, &acc, sh1);
                }
            }
            (0, _) => {
                let mut win: [&[u16]; N] =
                    core::array::from_fn(|k| &self.rows.get(k)[x + half..][..W]);
                for j in 0..h {
                    let mut acc = [0i32; W];
                    for (s, &c) in win.iter().zip(self.cy) {
                        let s: &[u16; W] = (*s).try_into().unwrap();
                        for i in 0..W {
                            acc[i] += c * i32::from(s[i]);
                        }
                    }
                    put(out, j, &acc, sh1);
                    if j + 1 < h {
                        win.rotate_left(1);
                        win[N - 1] = &self.rows.get(j + N)[x + half..][..W];
                    }
                }
            }
            _ => {
                for j in 0..h + N - 1 {
                    let acc = hfilt::<N, W>(&self.rows.get(j)[x..], self.cx);
                    let t: &mut [M; W] = (&mut tmp[j * W..][..W]).try_into().unwrap();
                    for i in 0..W {
                        t[i] = M::of(acc[i] >> sh1);
                    }
                }
                for j in 0..h {
                    let mut acc = [0i32; W];
                    for (k, &c) in self.cy.iter().enumerate() {
                        let t: &[M; W] = tmp[(j + k) * W..][..W].try_into().unwrap();
                        for i in 0..W {
                            acc[i] += c * t[i].get();
                        }
                    }
                    put(out, j, &acc, 6);
                }
            }
        }
    }
}

/// One row of the horizontal filter: `acc[i] = sum_k c[k] * s[i + k]`.
#[inline(always)]
fn hfilt<const N: usize, const W: usize>(s: &[u16], c: &[i32; N]) -> [i32; W] {
    let mut acc = [0i32; W];
    for (k, &ck) in c.iter().enumerate() {
        let t: &[u16; W] = s[k..k + W].try_into().unwrap();
        for i in 0..W {
            acc[i] += ck * i32::from(t[i]);
        }
    }
    acc
}
