//! Deblocking (8.8.3): edges on the 4x4 luma and 8x8 chroma grids, boundary strength from
//! [`Meta`], the luma decisions and short / long filters, and the chroma filters.
//!
//! The edges of one direction never touch each other's samples (filter lengths shrink
//! where edges are close), so each direction reads a snapshot and writes row bands, and an
//! edge whose samples straddle two bands is computed by both, each writing its own rows.

use super::{Band, CuInfo, Meta, NONE, Pred, Shared, TbInfo, bands, cuts, run_jobs};
use crate::picture::Picture;

/// β′ (Table 43), Q = 0..63.
const BETA: [u8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
    20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64, 66,
    68, 70, 72, 74, 76, 78, 80, 82, 84, 86, 88,
];

/// tC′ (Table 43), Q = 0..65.
const TC: [u16; 66] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 4, 4, 4, 4, 5, 5, 5, 5, 7, 7, 8, 9,
    10, 10, 11, 13, 14, 15, 17, 19, 21, 24, 25, 29, 33, 36, 41, 45, 51, 57, 64, 71, 80, 89, 100,
    112, 125, 141, 157, 177, 198, 222, 250, 280, 314, 352, 395,
];

/// What an edge segment is filtered with.
#[derive(Clone, Copy, Debug)]
struct Edge {
    bs: u8,
    mfl_p: u32,
    mfl_q: u32,
    plt_p: bool,
    plt_q: bool,
    /// Slice of the coding block containing q0,0.
    slice: usize,
    /// Luma: (QpQ + QpP + 1) >> 1. Chroma: per component QpC (8.8.3.6.4, eq. 1343).
    qp: [i32; 2],
    /// Chroma bS per component (Cb, Cr); `bs` is their maximum.
    bs_c: [u8; 2],
}

fn tc_of(tc: u16, bd: u32) -> i32 {
    let tc = i32::from(tc);
    if bd < 10 {
        (tc + (1 << (9 - bd))) >> (10 - bd)
    } else {
        tc << (bd - 10)
    }
}

struct Ctx<'a> {
    meta: &'a Meta,
    sh: &'a Shared<'a>,
    bd: u32,
    /// Chroma shifts.
    sx: u32,
    sy: u32,
}

impl Ctx<'_> {
    /// Whether the boundary between the CTBs of luma samples p and q may be filtered
    /// (filterEdgeFlag, 8.8.3.2 step 1), for a coding block edge.
    fn edge_allowed(&self, (px, py): (u32, u32), (qx, qy): (u32, u32)) -> bool {
        let sh = self.sh;
        let a = sh.ctb_of(qx, qy);
        let b = sh.ctb_of(px, py);
        if a == b {
            return true;
        }
        if sh.ctb_subpic[a] != sh.ctb_subpic[b]
            && (!sh.lf_across_subpic(a) || !sh.lf_across_subpic(b))
        {
            return false;
        }
        if sh.ctb_tile[a] != sh.ctb_tile[b] && !sh.ctx.pps.loop_filter_across_tiles {
            return false;
        }
        if sh.slice_of_ctb(a) != sh.slice_of_ctb(b) && !sh.ctx.pps.loop_filter_across_slices {
            return false;
        }
        true
    }

    fn cu_slice(&self, cu: &CuInfo) -> usize {
        self.sh.slice_of_ctb(self.sh.ctb_of(cu.x, cu.y))
    }

    /// Qp′Cb / Qp′Cr of a chroma transform block (8.7.1, equations 1126 and 1127).
    fn chroma_qp(&self, tb: &TbInfo, cu: &CuInfo, c: usize) -> i32 {
        if let Some(q) = tb.qpc {
            return q[c];
        }
        let sps = self.sh.ctx.sps;
        let pps = self.sh.ctx.pps;
        let bd = sps.qp_bd_offset();
        let qpi = cu.qp_y.clamp(-bd, 63);
        let base = sps.chroma_qp(c, qpi);
        let slice = self.sh.ctx.slices.get(self.cu_slice(cu));
        let (po, so) = if c == 0 {
            (pps.cb_qp_offset, slice.map_or(0, |s| s.cb_qp_offset))
        } else {
            (pps.cr_qp_offset, slice.map_or(0, |s| s.cr_qp_offset))
        };
        (base + po + so).clamp(-bd, 63) + bd
    }

    fn on_vb(&self, pos: u32, vertical: bool, scale: u32) -> bool {
        let v = if vertical {
            self.sh.ctx.vb_x
        } else {
            self.sh.ctx.vb_y
        };
        v.iter().any(|&b| b / scale == pos)
    }

    /// The edge segment of channel `ch` whose q0,0 is at (x, y) in that channel's samples.
    fn edge(&self, ch: usize, vertical: bool, x: u32, y: u32) -> Option<Edge> {
        let g = &self.meta.grids[ch];
        let (px, py) = if vertical {
            (x.checked_sub(1)?, y)
        } else {
            (x, y.checked_sub(1)?)
        };
        let qi = g.at(x, y);
        let pi = g.at(px, py);
        let qc = *g.cu.get(qi)?;
        let pc = *g.cu.get(pi)?;
        if qc == NONE || pc == NONE {
            return None;
        }
        let cus = &self.meta.cus[ch];
        let qcu = &cus[qc as usize];
        let pcu = &cus[pc as usize];
        let slice = self.cu_slice(qcu);
        let sf = self.sh.ctx.slices.get(slice)?;
        if sf.dbf.disabled {
            return None;
        }
        let (scale_x, scale_y) = if ch == 0 { (0, 0) } else { (self.sx, self.sy) };
        if ch == 1 {
            // Only coding blocks starting on the 8x8 chroma grid have their edges filtered.
            let o = if vertical {
                qcu.x >> scale_x
            } else {
                qcu.y >> scale_y
            };
            if !o.is_multiple_of(8) {
                return None;
            }
        }
        let tbs = &self.meta.tbs[ch];
        let qt = g.tb[qi];
        let pt = g.tb[pi];
        if qt == NONE || pt == NONE {
            return None;
        }
        let pt = g.p_side(tbs, pt, vertical);
        let cu_edge = pc != qc;
        if cu_edge {
            let l = |a: u32, s: u32| a << s;
            if !self.edge_allowed(
                (l(px, scale_x), l(py, scale_y)),
                (l(x, scale_x), l(y, scale_y)),
            ) {
                return None;
            }
        } else if qt == pt {
            return None;
        }
        let scale = if vertical { 1 << scale_x } else { 1 << scale_y };
        if self.on_vb(if vertical { x } else { y }, vertical, scale) {
            return None;
        }
        let tq = &tbs[qt as usize];
        let tp = &tbs[pt as usize];
        let (sq, sp) = if vertical { (tq.w, tp.w) } else { (tq.h, tp.h) };
        let plt_p = pcu.pred == Pred::Palette;
        let plt_q = qcu.pred == Pred::Palette;
        let intra = pcu.pred == Pred::Intra || qcu.pred == Pred::Intra;
        if ch == 0 {
            let (mfl_p, mfl_q) = if sq <= 4 || sp <= 4 {
                (1, 1)
            } else {
                (if sp >= 32 { 7 } else { 3 }, if sq >= 32 { 7 } else { 3 })
            };
            let bs = if qcu.bdpcm && pcu.bdpcm {
                0
            } else if intra {
                2
            } else if tq.cbf[0] || tp.cbf[0] {
                1
            } else if cu_edge && pcu.pred != qcu.pred {
                // edgeIdc is 2 on coding block edges (8.8.3.4).
                1
            } else if cu_edge
                && qcu.pred == Pred::Ibc
                && ((qcu.bv.0 - pcu.bv.0).abs() >= 8 || (qcu.bv.1 - pcu.bv.1).abs() >= 8)
            {
                1
            } else {
                // Inter motion is not compared yet.
                0
            };
            if bs == 0 {
                return None;
            }
            Some(Edge {
                bs,
                mfl_p,
                mfl_q,
                plt_p,
                plt_q,
                slice,
                qp: [(qcu.qp_y + pcu.qp_y + 1) >> 1, 0],
                bs_c: [0; 2],
            })
        } else {
            let (mut mfl_p, mfl_q) = if sq >= 8 && sp >= 8 { (3, 3) } else { (1, 1) };
            if !vertical && mfl_q == 3 {
                let ctb_h = (1u32 << self.sh.log2_ctb) >> scale_y;
                if y.is_multiple_of(ctb_h) {
                    mfl_p = 1;
                }
            }
            let mut bs_c = [0u8; 2];
            for (c, b) in bs_c.iter_mut().enumerate() {
                *b = if qcu.bdpcm && pcu.bdpcm {
                    0
                } else if intra {
                    2
                } else if tq.cbf[1 + c] || tp.cbf[1 + c] {
                    1
                } else {
                    0
                };
            }
            let bs = bs_c[0].max(bs_c[1]);
            if bs == 0 {
                return None;
            }
            let bd = self.sh.ctx.sps.qp_bd_offset();
            let mut qp = [0; 2];
            for (c, q) in qp.iter_mut().enumerate() {
                let qq = self.chroma_qp(tq, qcu, c);
                let qpp = self.chroma_qp(tp, pcu, c);
                *q = (qq - bd + qpp - bd + 1) >> 1;
            }
            Some(Edge {
                bs,
                mfl_p,
                mfl_q,
                plt_p,
                plt_q,
                slice,
                qp,
                bs_c,
            })
        }
    }
}

/// Sample access for one edge segment: q0 of line 0 at `q0`, `across` towards q, `along`
/// to the next line.
struct Seg<'a> {
    s: &'a [u16],
    q0: isize,
    across: isize,
    along: isize,
}

impl Seg<'_> {
    #[inline]
    fn p(&self, i: usize, k: usize) -> i32 {
        i32::from(
            self.s[(self.q0 + k as isize * self.along - (i as isize + 1) * self.across) as usize],
        )
    }
    #[inline]
    fn q(&self, j: usize, k: usize) -> i32 {
        i32::from(self.s[(self.q0 + k as isize * self.along + j as isize * self.across) as usize])
    }
    #[inline]
    fn p_idx(&self, i: usize, k: usize) -> usize {
        (self.q0 + k as isize * self.along - (i as isize + 1) * self.across) as usize
    }
    #[inline]
    fn q_idx(&self, j: usize, k: usize) -> usize {
        (self.q0 + k as isize * self.along + j as isize * self.across) as usize
    }
}

/// Writes `v` at plane index `idx` when its row lies in the band.
#[inline]
fn put(b: &mut Band<'_>, stride: usize, idx: usize, v: i32) {
    let row = idx / stride;
    if row >= b.y0 && row < b.y0 + b.rows {
        b.data[idx - b.y0 * stride] = v as u16;
    }
}

/// Decision process for a luma sample (8.8.3.6.6).
#[allow(clippy::too_many_arguments)]
fn luma_dsam(
    (p0, p3, q0, q3): (i32, i32, i32, i32),
    dpq: i32,
    mut sp: i32,
    mut sq: i32,
    spq: i32,
    large_p: bool,
    large_q: bool,
    beta: i32,
    tc: i32,
) -> bool {
    if large_p {
        sp = (sp + (p3 - p0).abs() + 1) >> 1;
    }
    if large_q {
        sq = (sq + (q3 - q0).abs() + 1) >> 1;
    }
    let (t1, t2) = if large_p || large_q {
        ((3 * beta) >> 5, beta >> 4)
    } else {
        (beta >> 3, beta >> 2)
    };
    dpq < t2 && sp + sq < t1 && spq < (5 * tc + 1) >> 1
}

/// Decides and filters one 4-line luma edge segment (8.8.3.6.2, 8.8.3.6.3, 8.8.3.6.7,
/// 8.8.3.6.8).
#[allow(clippy::too_many_arguments)]
fn luma_segment(
    c: &Ctx<'_>,
    seg: &Seg<'_>,
    out: &mut Band<'_>,
    stride: usize,
    e: &Edge,
    horizontal_ctb_boundary: bool,
) {
    let sps = c.sh.ctx.sps;
    let bd = c.bd;
    let dbf = &c.sh.ctx.slices[e.slice].dbf;
    let mut qp_off = 0;
    if sps.ladf {
        let level = (seg.p(0, 0) + seg.p(0, 3) + seg.q(0, 0) + seg.q(0, 3)) >> 2;
        qp_off = sps.ladf_lowest_interval_qp_offset;
        for i in 0..sps.num_ladf_intervals.saturating_sub(1) {
            if level > sps.ladf_lower_bound[i + 1] {
                qp_off = sps.ladf_qp_offset[i];
            } else {
                break;
            }
        }
    }
    let qp = e.qp[0] + qp_off;
    let beta = i32::from(BETA[(qp + (dbf.beta_div2[0] << 1)).clamp(0, 63) as usize]) << (bd - 8);
    let tq = (qp + 2 * (i32::from(e.bs) - 1) + (dbf.tc_div2[0] << 1)).clamp(0, 65);
    let tc = tc_of(TC[tq as usize], bd);
    let mut mfl_p = e.mfl_p as usize;
    let mut mfl_q = e.mfl_q as usize;
    let dp0 = (seg.p(2, 0) - 2 * seg.p(1, 0) + seg.p(0, 0)).abs();
    let dp3 = (seg.p(2, 3) - 2 * seg.p(1, 3) + seg.p(0, 3)).abs();
    let dq0 = (seg.q(2, 0) - 2 * seg.q(1, 0) + seg.q(0, 0)).abs();
    let dq3 = (seg.q(2, 3) - 2 * seg.q(1, 3) + seg.q(0, 3)).abs();
    let (mut sp0, mut sq0, mut spq0, mut sp3, mut sq3, mut spq3) = (0, 0, 0, 0, 0, 0);
    if mfl_p >= 3 && mfl_q >= 3 {
        sp0 = (seg.p(3, 0) - seg.p(0, 0)).abs();
        sq0 = (seg.q(0, 0) - seg.q(3, 0)).abs();
        spq0 = (seg.p(0, 0) - seg.q(0, 0)).abs();
        sp3 = (seg.p(3, 3) - seg.p(0, 3)).abs();
        sq3 = (seg.q(0, 3) - seg.q(3, 3)).abs();
        spq3 = (seg.p(0, 3) - seg.q(0, 3)).abs();
    }
    let mut large_p = mfl_p > 3;
    let large_q = mfl_q > 3;
    if horizontal_ctb_boundary {
        large_p = false;
    }
    let (mut ds0, mut ds3) = (false, false);
    if large_p || large_q {
        let (dp0l, dp3l) = if large_p {
            (
                (dp0 + (seg.p(5, 0) - 2 * seg.p(4, 0) + seg.p(3, 0)).abs() + 1) >> 1,
                (dp3 + (seg.p(5, 3) - 2 * seg.p(4, 3) + seg.p(3, 3)).abs() + 1) >> 1,
            )
        } else {
            mfl_p = 3;
            (dp0, dp3)
        };
        let (dq0l, dq3l) = if large_q {
            (
                (dq0 + (seg.q(5, 0) - 2 * seg.q(4, 0) + seg.q(3, 0)).abs() + 1) >> 1,
                (dq3 + (seg.q(5, 3) - 2 * seg.q(4, 3) + seg.q(3, 3)).abs() + 1) >> 1,
            )
        } else {
            (dq0, dq3)
        };
        let (sp0l, sp3l) = if mfl_p == 7 {
            (
                sp0 + (seg.p(7, 0) - seg.p(6, 0) - seg.p(5, 0) + seg.p(4, 0)).abs(),
                sp3 + (seg.p(7, 3) - seg.p(6, 3) - seg.p(5, 3) + seg.p(4, 3)).abs(),
            )
        } else {
            (sp0, sp3)
        };
        let (sq0l, sq3l) = if mfl_q == 7 {
            (
                sq0 + (seg.q(4, 0) - seg.q(5, 0) - seg.q(6, 0) + seg.q(7, 0)).abs(),
                sq3 + (seg.q(4, 3) - seg.q(5, 3) - seg.q(6, 3) + seg.q(7, 3)).abs(),
            )
        } else {
            (sq0, sq3)
        };
        let dpq0l = dp0l + dq0l;
        let dpq3l = dp3l + dq3l;
        if dpq0l + dpq3l < beta {
            let ends = |k: usize| {
                let (mut a, mut b, mut cc, mut d) = (0, 0, 0, 0);
                if large_p {
                    b = seg.p(3, k);
                    a = seg.p(mfl_p, k);
                }
                if large_q {
                    d = seg.q(3, k);
                    cc = seg.q(mfl_q, k);
                }
                (a, b, cc, d)
            };
            ds0 = luma_dsam(
                ends(0),
                2 * dpq0l,
                sp0l,
                sq0l,
                spq0,
                large_p,
                large_q,
                beta,
                tc,
            );
            ds3 = luma_dsam(
                ends(3),
                2 * dpq3l,
                sp3l,
                sq3l,
                spq3,
                large_p,
                large_q,
                beta,
                tc,
            );
        }
    }
    let (de, dep, deq);
    if ds0 && ds3 {
        (de, dep, deq) = (3, true, true);
    } else {
        let dpq0 = dp0 + dq0;
        let dpq3 = dp3 + dq3;
        let dp = dp0 + dp3;
        let dq = dq0 + dq3;
        let d = dpq0 + dpq3;
        let (mut e1, mut ep, mut eq) = (0, false, false);
        let (mut s0, mut s3) = (false, false);
        if d < beta && mfl_p > 2 && mfl_q > 2 {
            s0 = luma_dsam(
                (0, 0, 0, 0),
                2 * dpq0,
                sp0,
                sq0,
                spq0,
                false,
                false,
                beta,
                tc,
            );
            s3 = luma_dsam(
                (0, 0, 0, 0),
                2 * dpq3,
                sp3,
                sq3,
                spq3,
                false,
                false,
                beta,
                tc,
            );
        }
        if d < beta {
            e1 = 1;
            if s0 && s3 {
                e1 = 2;
                mfl_p = 3;
                mfl_q = 3;
            }
            let t = (beta + (beta >> 1)) >> 3;
            if mfl_p > 1 && mfl_q > 1 && dp < t {
                ep = true;
            }
            if mfl_p > 1 && mfl_q > 1 && dq < t {
                eq = true;
            }
            if e1 == 1 {
                mfl_p = 1 + usize::from(ep);
                mfl_q = 1 + usize::from(eq);
            }
        }
        (de, dep, deq) = (e1, ep, eq);
    }
    if de == 0 {
        return;
    }
    let max = (1 << bd) - 1;
    for k in 0..4 {
        if de == 3 {
            luma_long(seg, out, stride, k, mfl_p, mfl_q, tc, e.plt_p, e.plt_q);
            continue;
        }
        let p = |i| seg.p(i, k);
        let q = |j| seg.q(j, k);
        let (p0, p1, p2, q0, q1, q2) = (p(0), p(1), p(2), q(0), q(1), q(2));
        let mut pv = [0i32; 3];
        let mut qv = [0i32; 3];
        let (ndp, ndq);
        if de == 2 {
            let (p3, q3) = (p(3), q(3));
            pv[0] = ((p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3).clamp(p0 - 3 * tc, p0 + 3 * tc);
            pv[1] = ((p2 + p1 + p0 + q0 + 2) >> 2).clamp(p1 - 2 * tc, p1 + 2 * tc);
            pv[2] = ((2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3).clamp(p2 - tc, p2 + tc);
            qv[0] = ((p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3).clamp(q0 - 3 * tc, q0 + 3 * tc);
            qv[1] = ((p0 + q0 + q1 + q2 + 2) >> 2).clamp(q1 - 2 * tc, q1 + 2 * tc);
            qv[2] = ((p0 + q0 + q1 + 3 * q2 + 2 * q3 + 4) >> 3).clamp(q2 - tc, q2 + tc);
            (ndp, ndq) = (3, 3);
        } else {
            let mut delta = (9 * (q0 - p0) - 3 * (q1 - p1) + 8) >> 4;
            if delta.abs() < tc * 10 {
                delta = delta.clamp(-tc, tc);
                pv[0] = (p0 + delta).clamp(0, max);
                qv[0] = (q0 - delta).clamp(0, max);
                if dep {
                    let dp = ((((p2 + p0 + 1) >> 1) - p1 + delta) >> 1).clamp(-(tc >> 1), tc >> 1);
                    pv[1] = (p1 + dp).clamp(0, max);
                }
                if deq {
                    let dq = ((((q2 + q0 + 1) >> 1) - q1 - delta) >> 1).clamp(-(tc >> 1), tc >> 1);
                    qv[1] = (q1 + dq).clamp(0, max);
                }
                (ndp, ndq) = (1 + usize::from(dep), 1 + usize::from(deq));
            } else {
                (ndp, ndq) = (0, 0);
            }
        }
        if !e.plt_p {
            for (i, &v) in pv.iter().enumerate().take(ndp) {
                put(out, stride, seg.p_idx(i, k), v);
            }
        }
        if !e.plt_q {
            for (j, &v) in qv.iter().enumerate().take(ndq) {
                put(out, stride, seg.q_idx(j, k), v);
            }
        }
    }
}

/// Filtering process for a luma sample using long filters (8.8.3.6.8).
#[allow(clippy::too_many_arguments)]
fn luma_long(
    seg: &Seg<'_>,
    out: &mut Band<'_>,
    stride: usize,
    k: usize,
    mp: usize,
    mq: usize,
    tc: i32,
    plt_p: bool,
    plt_q: bool,
) {
    let p = |i| seg.p(i, k);
    let q = |j| seg.q(j, k);
    let mid = if mp == mq && mp == 5 {
        (p(4) + p(3) + 2 * (p(2) + p(1) + p(0) + q(0) + q(1) + q(2)) + q(3) + q(4) + 8) >> 4
    } else if mp == mq {
        (p(6)
            + p(5)
            + p(4)
            + p(3)
            + p(2)
            + p(1)
            + 2 * (p(0) + q(0))
            + q(1)
            + q(2)
            + q(3)
            + q(4)
            + q(5)
            + q(6)
            + 8)
            >> 4
    } else if (mq == 7 && mp == 5) || (mq == 5 && mp == 7) {
        (p(5)
            + p(4)
            + p(3)
            + p(2)
            + 2 * (p(1) + p(0) + q(0) + q(1))
            + q(2)
            + q(3)
            + q(4)
            + q(5)
            + 8)
            >> 4
    } else if (mq == 5 && mp == 3) || (mq == 3 && mp == 5) {
        (p(3) + p(2) + p(1) + p(0) + q(0) + q(1) + q(2) + q(3) + 4) >> 3
    } else if mq == 7 && mp == 3 {
        (2 * (p(2) + p(1) + p(0) + q(0))
            + p(0)
            + p(1)
            + q(1)
            + q(2)
            + q(3)
            + q(4)
            + q(5)
            + q(6)
            + 8)
            >> 4
    } else {
        (p(6)
            + p(5)
            + p(4)
            + p(3)
            + p(2)
            + p(1)
            + 2 * (q(2) + q(1) + q(0) + p(0))
            + q(0)
            + q(1)
            + 8)
            >> 4
    };
    let ref_p = (p(mp) + p(mp - 1) + 1) >> 1;
    let ref_q = (q(mq) + q(mq - 1) + 1) >> 1;
    let taps = |m: usize| -> (&'static [i32], &'static [i32]) {
        match m {
            7 => (&[59, 50, 41, 32, 23, 14, 5], &[6, 5, 4, 3, 2, 1, 1]),
            5 => (&[58, 45, 32, 19, 6], &[6, 5, 4, 3, 2]),
            _ => (&[53, 32, 11], &[6, 4, 2]),
        }
    };
    let (f, tp) = taps(mp);
    let (g, tq) = taps(mq);
    if !plt_p {
        for i in 0..mp {
            let v = p(i);
            let lim = (tc * tp[i]) >> 1;
            let r = ((mid * f[i] + ref_p * (64 - f[i]) + 32) >> 6).clamp(v - lim, v + lim);
            put(out, stride, seg.p_idx(i, k), r);
        }
    }
    if !plt_q {
        for j in 0..mq {
            let v = q(j);
            let lim = (tc * tq[j]) >> 1;
            let r = ((mid * g[j] + ref_q * (64 - g[j]) + 32) >> 6).clamp(v - lim, v + lim);
            put(out, stride, seg.q_idx(j, k), r);
        }
    }
}

/// Decides and filters one chroma edge segment of component `c` (0 Cb, 1 Cr) (8.8.3.6.4,
/// 8.8.3.6.5, 8.8.3.6.9, 8.8.3.6.10).
#[allow(clippy::too_many_arguments)]
fn chroma_segment(
    cx: &Ctx<'_>,
    seg: &Seg<'_>,
    out: &mut Band<'_>,
    stride: usize,
    e: &Edge,
    c: usize,
    vertical: bool,
) {
    let bs = i32::from(e.bs_c[c]);
    if bs == 0 {
        return;
    }
    let bd = cx.bd;
    let sub_sample = if vertical { 1 << cx.sy } else { 1 << cx.sx };
    let max_k = if sub_sample == 1 { 3 } else { 1 };
    let dbf = &cx.sh.ctx.slices[e.slice].dbf;
    let qpc = e.qp[c];
    let beta =
        i32::from(BETA[(qpc + (dbf.beta_div2[1 + c] << 1)).clamp(0, 63) as usize]) << (bd - 8);
    let tq = (qpc + 2 * (bs - 1) + (dbf.tc_div2[1 + c] << 1)).clamp(0, 65);
    let tc = tc_of(TC[tq as usize], bd);
    let (mut mp, mut mq) = (e.mfl_p as usize, e.mfl_q as usize);
    if mp == 1 && mq == 1 && bs != 2 {
        return;
    }
    if mq == 3 {
        let n1 = if sub_sample == 2 { 1 } else { 3 };
        let pp = |i: usize, k: usize| {
            if mp == 1 && i >= 2 {
                seg.p(1, k)
            } else {
                seg.p(i, k)
            }
        };
        let dp0 = (pp(2, 0) - 2 * pp(1, 0) + pp(0, 0)).abs();
        let dp1 = (pp(2, n1) - 2 * pp(1, n1) + pp(0, n1)).abs();
        let dq0 = (seg.q(2, 0) - 2 * seg.q(1, 0) + seg.q(0, 0)).abs();
        let dq1 = (seg.q(2, n1) - 2 * seg.q(1, n1) + seg.q(0, n1)).abs();
        let dpq0 = dp0 + dq0;
        let dpq1 = dp1 + dq1;
        let d = dpq0 + dpq1;
        let dsam = |k: usize, dpq: i32| {
            dpq < (beta >> 2)
                && (pp(3, k) - pp(0, k)).abs() + (seg.q(0, k) - seg.q(3, k)).abs() < (beta >> 3)
                && (pp(0, k) - seg.q(0, k)).abs() < (5 * tc + 1) >> 1
        };
        let (mut s0, mut s1) = (false, false);
        if d < beta {
            s0 = dsam(0, 2 * dpq0);
            s1 = dsam(n1, 2 * dpq1);
        }
        if !s0 || !s1 {
            mp = 1;
            mq = 1;
        }
    }
    let max = (1 << bd) - 1;
    for k in 0..=max_k {
        let p = |i| seg.p(i, k);
        let q = |j| seg.q(j, k);
        let mut pv = [0i32; 3];
        let mut qv = [0i32; 3];
        let (np, nq);
        if mp == 3 && mq == 3 {
            let (p0, p1, p2, p3, q0, q1, q2, q3) = (p(0), p(1), p(2), p(3), q(0), q(1), q(2), q(3));
            pv[0] = ((p3 + p2 + p1 + 2 * p0 + q0 + q1 + q2 + 4) >> 3).clamp(p0 - tc, p0 + tc);
            pv[1] = ((2 * p3 + p2 + 2 * p1 + p0 + q0 + q1 + 4) >> 3).clamp(p1 - tc, p1 + tc);
            pv[2] = ((3 * p3 + 2 * p2 + p1 + p0 + q0 + 4) >> 3).clamp(p2 - tc, p2 + tc);
            qv[0] = ((p2 + p1 + p0 + 2 * q0 + q1 + q2 + q3 + 4) >> 3).clamp(q0 - tc, q0 + tc);
            qv[1] = ((p1 + p0 + q0 + 2 * q1 + q2 + 2 * q3 + 4) >> 3).clamp(q1 - tc, q1 + tc);
            qv[2] = ((p0 + q0 + q1 + 2 * q2 + 3 * q3 + 4) >> 3).clamp(q2 - tc, q2 + tc);
            (np, nq) = (3, 3);
        } else if mp == 1 && mq == 3 {
            let (p0, p1, q0, q1, q2, q3) = (p(0), p(1), q(0), q(1), q(2), q(3));
            pv[0] = ((3 * p1 + 2 * p0 + q0 + q1 + q2 + 4) >> 3).clamp(p0 - tc, p0 + tc);
            qv[0] = ((2 * p1 + p0 + 2 * q0 + q1 + q2 + q3 + 4) >> 3).clamp(q0 - tc, q0 + tc);
            qv[1] = ((p1 + p0 + q0 + 2 * q1 + q2 + 2 * q3 + 4) >> 3).clamp(q1 - tc, q1 + tc);
            qv[2] = ((p0 + q0 + q1 + 2 * q2 + 3 * q3 + 4) >> 3).clamp(q2 - tc, q2 + tc);
            (np, nq) = (1, 3);
        } else {
            let (p0, p1, q0, q1) = (p(0), p(1), q(0), q(1));
            let delta = ((((q0 - p0) << 2) + p1 - q1 + 4) >> 3).clamp(-tc, tc);
            pv[0] = (p0 + delta).clamp(0, max);
            qv[0] = (q0 - delta).clamp(0, max);
            (np, nq) = (1, 1);
        }
        if !e.plt_p {
            for (i, &v) in pv.iter().enumerate().take(np) {
                put(out, stride, seg.p_idx(i, k), v);
            }
        }
        if !e.plt_q {
            for (j, &v) in qv.iter().enumerate().take(nq) {
                put(out, stride, seg.q_idx(j, k), v);
            }
        }
    }
}

/// Filters every edge of one direction of plane `ci` (0 luma, 1 Cb, 2 Cr).
fn direction(pic: &mut Picture, ci: usize, vertical: bool, cx: &Ctx<'_>, threads: usize) {
    let ch = usize::from(ci > 0);
    let plane = &mut pic.planes[ci];
    let (w, h, stride) = (plane.w as u32, plane.h as u32, plane.stride);
    let src = plane.data.clone();
    let ctb_rows = if ch == 0 {
        1usize << cx.sh.log2_ctb
    } else {
        (1usize << cx.sh.log2_ctb) >> cx.sy
    };
    let (grid, seg_len) = if ch == 0 {
        (4u32, 4u32)
    } else {
        // Segments along the edge: 4 / SubHeightC rows for vertical edges, 4 / SubWidthC
        // columns for horizontal ones.
        (8, if vertical { 4 >> cx.sy } else { 4 >> cx.sx })
    };
    let reach: u32 = if ch == 0 { 8 } else { 4 };
    let jobs = bands(&mut plane.data, stride, &cuts(h as usize, ctb_rows));
    let src = &src;
    run_jobs(jobs, threads, |mut band: Band<'_>| {
        let a = band.y0 as u32;
        let b = (band.y0 + band.rows) as u32;
        if vertical {
            let mut y = a - a % seg_len;
            while y < b {
                let mut x = grid;
                while x < w {
                    if let Some(e) = cx.edge(ch, true, x, y) {
                        let seg = Seg {
                            s: src,
                            q0: (y as usize * stride + x as usize) as isize,
                            across: 1,
                            along: stride as isize,
                        };
                        if ch == 0 {
                            luma_segment(cx, &seg, &mut band, stride, &e, false);
                        } else {
                            chroma_segment(cx, &seg, &mut band, stride, &e, ci - 1, true);
                        }
                    }
                    x += grid;
                }
                y += seg_len;
            }
        } else {
            let lo = a.saturating_sub(reach);
            let mut y = (lo.div_ceil(grid) * grid).max(grid);
            while y < h && y < b + reach {
                let mut x = 0;
                while x < w {
                    if let Some(e) = cx.edge(ch, false, x, y) {
                        let seg = Seg {
                            s: src,
                            q0: (y as usize * stride + x as usize) as isize,
                            across: stride as isize,
                            along: 1,
                        };
                        if ch == 0 {
                            let ctb_b = y % (1 << cx.sh.log2_ctb) == 0;
                            luma_segment(cx, &seg, &mut band, stride, &e, ctb_b);
                        } else {
                            chroma_segment(cx, &seg, &mut band, stride, &e, ci - 1, false);
                        }
                    }
                    x += seg_len;
                }
                y += grid;
            }
        }
    });
}

/// The deblocking filter process (8.8.3.1): luma vertical then horizontal edges, then
/// chroma vertical then horizontal edges.
pub(super) fn run(pic: &mut Picture, meta: &Meta, sh: &Shared<'_>, threads: usize) {
    if sh.ctx.slices.iter().all(|s| s.dbf.disabled) {
        return;
    }
    let (sx, sy) = meta.shift.unwrap_or((0, 0));
    let cx = Ctx {
        meta,
        sh,
        bd: pic.bit_depth,
        sx,
        sy,
    };
    direction(pic, 0, true, &cx, threads);
    direction(pic, 0, false, &cx, threads);
    if meta.shift.is_some() && pic.planes.len() == 3 {
        for ci in 1..3 {
            direction(pic, ci, true, &cx, threads);
        }
        for ci in 1..3 {
            direction(pic, ci, false, &cx, threads);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tc_scaling_follows_bit_depth() {
        assert_eq!(tc_of(395, 10), 395);
        assert_eq!(tc_of(395, 8), (395 + 2) >> 2);
        assert_eq!(BETA[63], 88);
        assert_eq!(TC[65], 395);
        assert!(luma_dsam((0, 0, 0, 0), 0, 0, 0, 0, false, false, 64, 10));
        assert!(!luma_dsam((0, 0, 0, 0), 64, 0, 0, 0, false, false, 64, 10));
    }
}
