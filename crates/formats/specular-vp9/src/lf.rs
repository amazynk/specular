//! The loop filter (section 8.8).
//!
//! Superblocks are filtered in the specification's raster order within a superblock row. Rows
//! run in parallel as a wavefront: a row only shares the bottom 8 sample rows of the row above
//! (its top edge filters them), and the row above hands each 64-column piece of those rows over
//! once it is done with it (after filtering the superblock to the right of it). Each superblock
//! is filtered in a small local buffer holding it plus the 8 samples to its left and above.

use std::simd::{Select, Simd, cmp::SimdOrd, cmp::SimdPartialOrd, num::SimdInt, num::SimdUint};
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::frame::{FrameBuf, Pixel};
use crate::header::{FrameHeader, SEG_LVL_ALT_L};
use crate::tables::*;
use crate::tile::{BLOCK_8X8, MiInfo, NEARESTMV, NEARMV, NEWMV};

/// LvlLookup (8.8.1) and the frame facts the filter needs.
pub(crate) struct LfFrame<'a> {
    lvl: [[[u8; 2]; 4]; 8],
    sharpness: u8,
    mi_rows: usize,
    mi_cols: usize,
    ss: [usize; 2],
    bit_depth: u8,
    mi: &'a [MiInfo],
}

impl<'a> LfFrame<'a> {
    pub(crate) fn new(h: &FrameHeader, mi: &'a [MiInfo]) -> LfFrame<'a> {
        let mut lvl = [[[0u8; 2]; 4]; 8];
        let level = h.lf.level as i32;
        let shift = level >> 5;
        for (seg, l) in lvl.iter_mut().enumerate() {
            let mut lvl_seg = level;
            if h.seg.active(seg, SEG_LVL_ALT_L) {
                let d = h.seg.feature_data[seg][SEG_LVL_ALT_L];
                lvl_seg = if h.seg.abs_delta { d } else { level + d };
                lvl_seg = lvl_seg.clamp(0, 63);
            }
            if !h.lf.delta_enabled {
                *l = [[lvl_seg as u8; 2]; 4];
            } else {
                let intra = lvl_seg + (h.lf.ref_deltas[0] << shift);
                l[0][0] = intra.clamp(0, 63) as u8;
                for rf in 1..4 {
                    for mode in 0..2 {
                        let v = lvl_seg
                            + (h.lf.ref_deltas[rf] << shift)
                            + (h.lf.mode_deltas[mode] << shift);
                        l[rf][mode] = v.clamp(0, 63) as u8;
                    }
                }
            }
        }
        LfFrame {
            lvl,
            sharpness: h.lf.sharpness,
            mi_rows: h.mi_rows,
            mi_cols: h.mi_cols,
            ss: [h.color.ss_x as usize, h.color.ss_y as usize],
            bit_depth: h.color.bit_depth,
            mi,
        }
    }

    /// The filter for the samples at luma position (x, y): `None` if they are not filtered,
    /// else (filter size, lvl).
    #[inline]
    fn edge(&self, pass: usize, plane: usize, x: usize, y: usize, edge: usize) -> Option<(u8, u8)> {
        let (sub_x, sub_y) = if plane == 0 {
            (0, 0)
        } else {
            (self.ss[0], self.ss[1])
        };
        if x >= 8 * self.mi_cols || y >= 8 * self.mi_rows {
            return None;
        }
        if (pass == 0 && x == 0) || (pass == 1 && y == 0) {
            return None;
        }
        let loop_col = ((x >> 3) >> sub_x) << sub_x;
        let loop_row = ((y >> 3) >> sub_y) << sub_y;
        let m = &self.mi[loop_row * self.mi_cols + loop_col];
        let mi_size = m.size as usize;
        let tx_sz = if plane > 0 {
            if mi_size < BLOCK_8X8 {
                0
            } else {
                let p = SS_SIZE_LOOKUP[mi_size][sub_x][sub_y] as usize;
                (m.tx_size as usize).min(MAX_TXSIZE_LOOKUP[p.min(12)] as usize)
            }
        } else {
            m.tx_size as usize
        };
        let sub = if pass == 0 { sub_x } else { sub_y };
        let sb_size = if sub == 0 { mi_size } else { mi_size.max(6) };
        let is_intra = m.ref_frame[0] <= 0;
        let block_edge = if pass == 0 {
            x.is_multiple_of(8 * NUM_8X8_BLOCKS_WIDE_LOOKUP[sb_size] as usize)
        } else {
            y.is_multiple_of(8 * NUM_8X8_BLOCKS_HIGH_LOOKUP[sb_size] as usize)
        };
        let tx_edge = if pass == 1
            && sub_x == 1
            && self.mi_cols & 1 == 1
            && edge & 1 == 1
            && x + 8 >= self.mi_cols * 8
        {
            false
        } else {
            edge.is_multiple_of(1 << tx_sz)
        };
        let is32 = edge.is_multiple_of(8);
        let apply = block_edge || (tx_edge && (is_intra || !m.skip));
        if !apply {
            return None;
        }
        let base = if tx_sz == 0 && is32 { 1 } else { tx_sz.min(2) };
        let fsize = if (pass == 0 && sub_x == 1 && base == 2 && (x >> 3) == self.mi_cols - 1)
            || (pass == 1 && sub_y == 1 && base == 2 && (y >> 3) == self.mi_rows - 1)
        {
            1
        } else {
            base
        };
        let mode = m.y_mode;
        let mode_type = (mode == NEARESTMV || mode == NEARMV || mode == NEWMV) as usize;
        let lvl = self.lvl[m.seg_id as usize & 7][m.ref_frame[0].max(0) as usize][mode_type];
        if lvl == 0 {
            return None;
        }
        Some((fsize as u8, lvl))
    }

    fn limits(&self, lvl: u8) -> (i32, i32, i32) {
        let lvl = lvl as i32;
        let sh = self.sharpness as i32;
        let shift = if sh > 4 {
            2
        } else if sh > 0 {
            1
        } else {
            0
        };
        let limit = if sh > 0 {
            (lvl >> shift).clamp(1, 9 - sh)
        } else {
            (lvl >> shift).max(1)
        };
        let blimit = 2 * (lvl + 2) + limit;
        let thresh = lvl >> 4;
        let s = self.bit_depth as i32 - 8;
        (limit << s, blimit << s, thresh << s)
    }
}

/// Local buffer stride: 64 samples plus 8 on the left.
const LS: usize = 72;

/// Filters one superblock of one plane in the local buffer `buf` (origin at (8, 8)).
fn filter_sb(lf: &LfFrame, buf: &mut [u16], plane: usize, row: usize, col: usize) {
    let (sub_x, sub_y) = if plane == 0 {
        (0, 0)
    } else {
        (lf.ss[0], lf.ss[1])
    };
    for pass in 0..2 {
        let (sub, edge_len) = if pass == 0 {
            (sub_x, 64 >> sub_y)
        } else {
            (sub_y, 64 >> sub_x)
        };
        for edge in 0..(16 >> sub) {
            let mut i = 0;
            while i < edge_len {
                // Filter parameters are constant over runs of 8 samples along an edge, except
                // where the edge leaves the picture or the special chroma case applies: check
                // the second half separately.
                let at = |i: usize| {
                    let (x, y) = if pass == 0 {
                        (col * 8 + edge * (4 << sub_x), row * 8 + (i << sub_y))
                    } else {
                        (col * 8 + (i << sub_x), row * 8 + edge * (4 << sub_y))
                    };
                    lf.edge(pass, plane, x, y, edge)
                };
                let a = at(i);
                let b = at(i + 4);
                let runs: [(usize, Option<(u8, u8)>); 2] = if a == b {
                    [(8, a), (0, None)]
                } else {
                    [(4, a), (4, b)]
                };
                let mut j = i;
                for (n, p) in runs {
                    if let Some((fsize, lvl)) = p {
                        let (limit, blimit, thresh) = lf.limits(lvl);
                        let (lx, ly) = if pass == 0 {
                            (8 + edge * 4, 8 + j)
                        } else {
                            (8 + j, 8 + edge * 4)
                        };
                        let pos = ly * LS + lx;
                        if pass == 1 && n == 8 {
                            filter8_lines_h(buf, pos, limit, blimit, thresh, fsize, lf.bit_depth);
                        } else {
                            let (step, along) = if pass == 0 { (1, LS) } else { (LS, 1) };
                            for k in 0..n {
                                filter_line(
                                    buf,
                                    pos + k * along,
                                    step,
                                    limit,
                                    blimit,
                                    thresh,
                                    fsize,
                                    lf.bit_depth,
                                );
                            }
                        }
                    }
                    j += n;
                }
                i += 8;
            }
        }
    }
}

/// The sample filtering process (8.8.5) across one line of samples: q0 at `pos`, p0 at
/// `pos - step`.
#[allow(clippy::too_many_arguments)]
#[inline]
fn filter_line(
    buf: &mut [u16],
    pos: usize,
    step: usize,
    limit: i32,
    blimit: i32,
    thresh: i32,
    fsize: u8,
    bd: u8,
) {
    let g = |k: isize| buf[(pos as isize + k * step as isize) as usize] as i32;
    let (q0, q1, q2, q3) = (g(0), g(1), g(2), g(3));
    let (p0, p1, p2, p3) = (g(-1), g(-2), g(-3), g(-4));
    let hev = (p1 - p0).abs() > thresh || (q1 - q0).abs() > thresh;
    let mask = (p3 - p2).abs() > limit
        || (p2 - p1).abs() > limit
        || (p1 - p0).abs() > limit
        || (q1 - q0).abs() > limit
        || (q2 - q1).abs() > limit
        || (q3 - q2).abs() > limit
        || (p0 - q0).abs() * 2 + (p1 - q1).abs() / 2 > blimit;
    if mask {
        return;
    }
    let one = 1 << (bd - 8);
    let flat = fsize >= 1
        && (p1 - p0).abs() <= one
        && (q1 - q0).abs() <= one
        && (p2 - p0).abs() <= one
        && (q2 - q0).abs() <= one
        && (p3 - p0).abs() <= one
        && (q3 - q0).abs() <= one;
    if fsize == 0 || !flat {
        narrow(buf, pos, step, hev, p1, p0, q0, q1, bd);
        return;
    }
    let flat2 = fsize >= 2 && {
        let (q4, q5, q6, q7) = (g(4), g(5), g(6), g(7));
        let (p4, p5, p6, p7) = (g(-5), g(-6), g(-7), g(-8));
        (p7 - p0).abs() <= one
            && (q7 - q0).abs() <= one
            && (p6 - p0).abs() <= one
            && (q6 - q0).abs() <= one
            && (p5 - p0).abs() <= one
            && (q5 - q0).abs() <= one
            && (p4 - p0).abs() <= one
            && (q4 - q0).abs() <= one
    };
    let log2 = if fsize == 1 || !flat2 { 3 } else { 4 };
    wide(buf, pos, step, log2);
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn narrow(
    buf: &mut [u16],
    pos: usize,
    step: usize,
    hev: bool,
    p1: i32,
    p0: i32,
    q0: i32,
    q1: i32,
    bd: u8,
) {
    let lo = -(1 << (bd - 1));
    let hi = (1 << (bd - 1)) - 1;
    let c = |v: i32| v.clamp(lo, hi);
    let off = 0x80 << (bd - 8);
    let (ps1, ps0, qs0, qs1) = (p1 - off, p0 - off, q0 - off, q1 - off);
    let mut filter = if hev { c(ps1 - qs1) } else { 0 };
    filter = c(filter + 3 * (qs0 - ps0));
    let f1 = c(filter + 4) >> 3;
    let f2 = c(filter + 3) >> 3;
    buf[pos] = (c(qs0 - f1) + off) as u16;
    buf[pos - step] = (c(ps0 + f2) + off) as u16;
    if !hev {
        let f = (f1 + 1) >> 1;
        buf[pos + step] = (c(qs1 - f) + off) as u16;
        buf[pos - 2 * step] = (c(ps1 + f) + off) as u16;
    }
}

#[inline]
fn wide(buf: &mut [u16], pos: usize, step: usize, log2: u32) {
    let n = (1isize << (log2 - 1)) - 1;
    let mut v = [0i32; 16];
    // v[k + 8] = sample k (k = -8..7).
    for k in -(n + 1)..=n {
        v[(k + 8) as usize] = buf[(pos as isize + k * step as isize) as usize] as i32;
    }
    let mut out = [0i32; 16];
    for i in -n..n {
        let mut t = v[(i + 8) as usize];
        for j in -n..=n {
            let p = (i + j).clamp(-(n + 1), n);
            t += v[(p + 8) as usize];
        }
        out[(i + 8) as usize] = (t + (1 << (log2 - 1))) >> log2;
    }
    for i in -n..n {
        buf[(pos as isize + i * step as isize) as usize] = out[(i + 8) as usize] as u16;
    }
}

type V = Simd<i32, 8>;

/// Filters 8 adjacent vertical lines across a horizontal edge (q0 row at `pos`), all with the
/// same parameters, 8 lanes at a time.
#[allow(clippy::too_many_arguments)]
fn filter8_lines_h(
    buf: &mut [u16],
    pos: usize,
    limit: i32,
    blimit: i32,
    thresh: i32,
    fsize: u8,
    bd: u8,
) {
    let ld = |k: isize| -> V {
        let o = (pos as isize + k * LS as isize) as usize;
        Simd::<u16, 8>::from_slice(&buf[o..o + 8]).cast()
    };
    let (q0, q1, q2, q3) = (ld(0), ld(1), ld(2), ld(3));
    let (p0, p1, p2, p3) = (ld(-1), ld(-2), ld(-3), ld(-4));
    let lim = V::splat(limit);
    let ad = |a: V, b: V| (a - b).abs();
    let hev = ad(p1, p0).simd_gt(V::splat(thresh)) | ad(q1, q0).simd_gt(V::splat(thresh));
    let mask = ad(p3, p2).simd_gt(lim)
        | ad(p2, p1).simd_gt(lim)
        | ad(p1, p0).simd_gt(lim)
        | ad(q1, q0).simd_gt(lim)
        | ad(q2, q1).simd_gt(lim)
        | ad(q3, q2).simd_gt(lim)
        | (ad(p0, q0) * V::splat(2) + (ad(p1, q1) >> V::splat(1))).simd_gt(V::splat(blimit));
    let filt = !mask;
    if !filt.any() {
        return;
    }
    let one = V::splat(1 << (bd - 8));
    let flat = if fsize >= 1 {
        ad(p1, p0).simd_le(one)
            & ad(q1, q0).simd_le(one)
            & ad(p2, p0).simd_le(one)
            & ad(q2, q0).simd_le(one)
            & ad(p3, p0).simd_le(one)
            & ad(q3, q0).simd_le(one)
            & filt
    } else {
        filt & !filt
    };
    let narrow_m = filt & !flat;
    let mut outs: [V; 14] = [p6_dummy(); 14]; // p6..q6 results, index k + 7
    let mut set = [false; 14];
    // Narrow filter.
    if narrow_m.any() {
        let lo = V::splat(-(1 << (bd - 1)));
        let hi = V::splat((1 << (bd - 1)) - 1);
        let c = |v: V| v.simd_clamp(lo, hi);
        let off = V::splat(0x80 << (bd - 8));
        let (ps1, ps0, qs0, qs1) = (p1 - off, p0 - off, q0 - off, q1 - off);
        let zero = V::splat(0);
        let mut f = hev.select(c(ps1 - qs1), zero);
        f = c(f + V::splat(3) * (qs0 - ps0));
        let f1 = c(f + V::splat(4)) >> V::splat(3);
        let f2 = c(f + V::splat(3)) >> V::splat(3);
        let oq0 = c(qs0 - f1) + off;
        let op0 = c(ps0 + f2) + off;
        let f = (f1 + V::splat(1)) >> V::splat(1);
        let oq1 = hev.select(q1, c(qs1 - f) + off);
        let op1 = hev.select(p1, c(ps1 + f) + off);
        for (k, val, orig) in [
            (-2isize, op1, p1),
            (-1, op0, p0),
            (0, oq0, q0),
            (1, oq1, q1),
        ] {
            outs[(k + 7) as usize] = narrow_m.select(val, orig);
            set[(k + 7) as usize] = true;
        }
    }
    if flat.any() {
        let flat2 = if fsize >= 2 {
            let (q4, q5, q6, q7) = (ld(4), ld(5), ld(6), ld(7));
            let (p4, p5, p6, p7) = (ld(-5), ld(-6), ld(-7), ld(-8));
            ad(p7, p0).simd_le(one)
                & ad(q7, q0).simd_le(one)
                & ad(p6, p0).simd_le(one)
                & ad(q6, q0).simd_le(one)
                & ad(p5, p0).simd_le(one)
                & ad(q5, q0).simd_le(one)
                & ad(p4, p0).simd_le(one)
                & ad(q4, q0).simd_le(one)
                & flat
        } else {
            flat & !flat
        };
        let f8 = flat & !flat2;
        for (mask, log2) in [(f8, 3u32), (flat2, 4)] {
            if !mask.any() {
                continue;
            }
            let n = (1isize << (log2 - 1)) - 1;
            let mut v = [V::splat(0); 16];
            for k in -(n + 1)..=n {
                v[(k + 8) as usize] = ld(k);
            }
            for i in -n..n {
                let mut t = v[(i + 8) as usize];
                for j in -n..=n {
                    let p = (i + j).clamp(-(n + 1), n);
                    t += v[(p + 8) as usize];
                }
                let r = (t + V::splat(1 << (log2 - 1))) >> V::splat(log2 as i32);
                let idx = (i + 7) as usize;
                let prev = if set[idx] {
                    outs[idx]
                } else {
                    v[(i + 8) as usize]
                };
                outs[idx] = mask.select(r, prev);
                set[idx] = true;
            }
        }
    }
    for (idx, s) in set.iter().enumerate() {
        if *s {
            let k = idx as isize - 7;
            let o = (pos as isize + k * LS as isize) as usize;
            let v: Simd<u16, 8> = outs[idx].cast();
            buf[o..o + 8].copy_from_slice(v.as_array());
        }
    }
}

#[inline(always)]
fn p6_dummy() -> V {
    V::splat(0)
}

/// One superblock row's share of the frame: its sample rows, each split into 64-column
/// (luma) pieces.
pub(crate) struct RowWork<'a, P> {
    pub r: usize,
    /// Per plane: the rows of this superblock row, each as pieces per superblock column.
    pub rows: [Vec<Vec<&'a mut [P]>>; 3],
    pub rx: Option<Receiver<Seam<'a, P>>>,
    pub tx: Option<Sender<Seam<'a, P>>>,
}

/// The bottom 8 rows of a superblock row at one superblock column, per plane.
pub(crate) struct Seam<'a, P> {
    c: usize,
    rows: [Vec<&'a mut [P]>; 3],
}

/// Cuts the frame into superblock rows for `run_row`.
pub(crate) fn split_rows<'a, P: Pixel>(
    f: &'a mut FrameBuf<P>,
    sb_cols: usize,
) -> Vec<RowWork<'a, P>> {
    let ss = [
        (0usize, 0usize),
        (f.ss_x as usize, f.ss_y as usize),
        (f.ss_x as usize, f.ss_y as usize),
    ];
    let sb_rows = f.planes[0].rows / 64;
    let mut works: Vec<RowWork<'a, P>> = (0..sb_rows)
        .map(|r| RowWork {
            r,
            rows: Default::default(),
            rx: None,
            tx: None,
        })
        .collect();
    for (p, plane) in f.planes.iter_mut().enumerate() {
        let (sx, sy) = ss[p];
        let w = 64 >> sx;
        let hgt = 64 >> sy;
        let stride = plane.stride;
        for (y, row) in plane.data.chunks_mut(stride).enumerate() {
            let r = y / hgt;
            if r >= sb_rows {
                break;
            }
            let pieces: Vec<&'a mut [P]> = row.chunks_mut(w).take(sb_cols).collect();
            works[r].rows[p].push(pieces);
        }
    }
    for r in 1..sb_rows {
        let (tx, rx) = channel();
        works[r - 1].tx = Some(tx);
        works[r].rx = Some(rx);
    }
    works
}

/// Filters one superblock row.
pub(crate) fn run_row<P: Pixel>(lf: &LfFrame, mut w: RowWork<P>, sb_cols: usize) {
    let r = w.r;
    let mut above: Vec<Option<[Vec<&mut [P]>; 3]>> = (0..sb_cols).map(|_| None).collect();
    let mut buf = vec![0u16; LS * LS];
    for c in 0..sb_cols {
        if let Some(rx) = &w.rx {
            while above[c].is_none() {
                match rx.recv() {
                    Ok(s) => above[s.c] = Some(s.rows),
                    Err(_) => return,
                }
            }
        }
        if c * 8 < lf.mi_cols && r * 8 < lf.mi_rows {
            for plane in 0..3 {
                let (sx, sy) = if plane == 0 {
                    (0, 0)
                } else {
                    (lf.ss[0], lf.ss[1])
                };
                let wd = 64 >> sx;
                let ht = 64 >> sy;
                // Gather.
                if let Some(a) = &above[c] {
                    for (k, src) in a[plane].iter().enumerate() {
                        for (d, s) in buf[k * LS + 8..k * LS + 8 + wd].iter_mut().zip(src.iter()) {
                            *d = s.get() as u16;
                        }
                    }
                }
                for y in 0..ht {
                    let row = &w.rows[plane][y];
                    let o = (y + 8) * LS;
                    for (d, s) in buf[o + 8..o + 8 + wd].iter_mut().zip(row[c].iter()) {
                        *d = s.get() as u16;
                    }
                    if c > 0 {
                        for (d, s) in buf[o..o + 8].iter_mut().zip(row[c - 1][wd - 8..].iter()) {
                            *d = s.get() as u16;
                        }
                    }
                }
                filter_sb(lf, &mut buf, plane, r * 8, c * 8);
                // Scatter.
                if let Some(a) = &mut above[c] {
                    for (k, dst) in a[plane].iter_mut().enumerate() {
                        for (d, s) in dst.iter_mut().zip(&buf[k * LS + 8..k * LS + 8 + wd]) {
                            *d = P::put(*s as i32);
                        }
                    }
                }
                for y in 0..ht {
                    let o = (y + 8) * LS;
                    let row = &mut w.rows[plane][y];
                    for (d, s) in row[c].iter_mut().zip(&buf[o + 8..o + 8 + wd]) {
                        *d = P::put(*s as i32);
                    }
                    if c > 0 {
                        let wd8 = row[c - 1].len() - 8;
                        for (d, s) in row[c - 1][wd8..].iter_mut().zip(&buf[o..o + 8]) {
                            *d = P::put(*s as i32);
                        }
                    }
                }
            }
        }
        above[c] = None;
        if c > 0 {
            send_seam(&mut w, c - 1);
        }
    }
    if sb_cols > 0 {
        send_seam(&mut w, sb_cols - 1);
    }
}

fn send_seam<P: Pixel>(w: &mut RowWork<P>, c: usize) {
    let Some(tx) = &w.tx else { return };
    let mut rows: [Vec<&mut [P]>; 3] = Default::default();
    for (p, out) in rows.iter_mut().enumerate() {
        let n = w.rows[p].len();
        for y in n.saturating_sub(8)..n {
            out.push(std::mem::take(&mut w.rows[p][y][c]));
        }
    }
    let _ = tx.send(Seam { c, rows });
}
