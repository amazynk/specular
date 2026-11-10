//! Deblocking filter, ITU-T H.265 clause 8.7.2.
//!
//! Every filtered edge of an intra-only picture has boundary strength 2, so
//! for stills the derivation of clause 8.7.2.4 collapses to the constant
//! [`BS`]; inter pictures carry per-edge strengths computed while parsing.

use super::{FilterInfo, Side};
use crate::picture::{Picture, PlaneMut, edge, flags};

#[path = "deblock_chroma.rs"]
mod chroma;

/// `beta'` of Table 8-12, indexed by `Q` in `0..=51`.
const BETA_TABLE: [i32; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
    20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64,
];

/// `tc'` of Table 8-12, indexed by `Q` in `0..=53`.
const TC_TABLE: [i32; 54] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 3,
    3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 9, 10, 11, 13, 14, 16, 18, 20, 22, 24,
];

/// Boundary strength of every filtered edge of an intra-only picture.
const BS: i32 = 2;

/// The boundary strength of the edge segment whose Q side starts at luma
/// `(qx, qy)`: [`BS`] for an intra-only picture, else what
/// [`Picture::bs`](crate::picture::Picture::bs) recorded.
#[inline]
pub(super) fn strength(side: &Side<'_>, qx: usize, qy: usize, ver: bool) -> i32 {
    if side.bs.is_empty() {
        return BS;
    }
    let v = side.bs.get(side.idx4(qx, qy)).copied().unwrap_or(0);
    i32::from(if ver { v & 3 } else { v >> 2 })
}

/// `Clip3(lo, hi, v)` of clause 5.8, with no panicking path.
#[inline]
pub(super) fn clip3(lo: i32, hi: i32, v: i32) -> i32 {
    v.max(lo).min(hi)
}

/// Where the samples of one four-line segment sit in a plane.
///
/// Sample `j` of line `i` is at `first + i * line + j * along`, where `j`
/// counts from the outermost tap on the P side. Reading an edge of either
/// direction is then the same arithmetic with different steps, so the
/// direction never has to be re-examined per sample.
pub(super) struct Seg {
    first: usize,
    along: usize,
    line: usize,
}

impl Seg {
    /// The segment of the edge at `e`, starting at `s`, whose filter reads
    /// `taps` samples on each side of the edge.
    #[inline]
    pub(super) fn new(p: &PlaneMut<'_>, ver: bool, e: usize, s: usize, taps: usize) -> Seg {
        if ver {
            Seg {
                first: p.offset(e - taps, s),
                along: 1,
                line: p.stride,
            }
        } else {
            Seg {
                first: p.offset(s, e - taps),
                along: p.stride,
                line: 1,
            }
        }
    }

    /// Plane index of sample `j` of line `i`.
    #[inline]
    pub(super) fn at(&self, i: usize, j: usize) -> usize {
        self.first + i * self.line + j * self.along
    }
}

/// True when loop filtering must leave the samples of this coding unit alone.
#[inline]
fn locked(f: u8, info: &FilterInfo) -> bool {
    f & flags::TQ_BYPASS != 0 || (f & flags::PCM != 0 && info.pcm_loop_filter_disabled)
}

/// Filters one luma line, clause 8.7.2.5.7; `sm` is `p3,p2,p1,p0,q0,q1,q2,q3`.
fn filter_line(sm: &[i32; 8], de: u8, dep: bool, deq: bool, tc: i32, max: i32) -> [i32; 8] {
    let mut o = *sm;
    let (p3, p2, p1, p0) = (sm[0], sm[1], sm[2], sm[3]);
    let (q0, q1, q2, q3) = (sm[4], sm[5], sm[6], sm[7]);
    if de == 2 {
        o[3] = clip3(
            p0 - 2 * tc,
            p0 + 2 * tc,
            (p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3,
        );
        o[2] = clip3(p1 - 2 * tc, p1 + 2 * tc, (p2 + p1 + p0 + q0 + 2) >> 2);
        o[1] = clip3(
            p2 - 2 * tc,
            p2 + 2 * tc,
            (2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3,
        );
        o[4] = clip3(
            q0 - 2 * tc,
            q0 + 2 * tc,
            (p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3,
        );
        o[5] = clip3(q1 - 2 * tc, q1 + 2 * tc, (p0 + q0 + q1 + q2 + 2) >> 2);
        o[6] = clip3(
            q2 - 2 * tc,
            q2 + 2 * tc,
            (p0 + q0 + q1 + 3 * q2 + 2 * q3 + 4) >> 3,
        );
    } else if de == 1 {
        let raw = (9 * (q0 - p0) - 3 * (q1 - p1) + 8) >> 4;
        if raw.abs() < tc * 10 {
            let d = clip3(-tc, tc, raw);
            o[3] = clip3(0, max, p0 + d);
            o[4] = clip3(0, max, q0 - d);
            if dep {
                let dv = clip3(-(tc >> 1), tc >> 1, (((p2 + p0 + 1) >> 1) - p1 + d) >> 1);
                o[2] = clip3(0, max, p1 + dv);
            }
            if deq {
                let dv = clip3(-(tc >> 1), tc >> 1, (((q2 + q0 + 1) >> 1) - q1 - d) >> 1);
                o[5] = clip3(0, max, q1 + dv);
            }
        }
    }
    o
}

/// Filters one four-line luma segment of the edge at `e` starting at `s`.
fn luma_segment(
    y: &mut PlaneMut<'_>,
    side: &Side<'_>,
    info: &FilterInfo,
    ver: bool,
    e: usize,
    s: usize,
) {
    let (px, py) = if ver { (e - 1, s) } else { (s, e - 1) };
    let (qx, qy) = if ver { (e, s) } else { (s, e) };
    let qc = info.ctb_of(qx, qy);
    let cf = match info.ctb.get(qc) {
        Some(c) => *c,
        None => return,
    };
    if cf.deblock_disabled || !info.may_cross(info.ctb_of(px, py), qc) {
        return;
    }
    let bs = strength(side, qx, qy, ver);
    if bs == 0 {
        return;
    }
    let pl = locked(side.cu_flags[side.idx4(px, py)], info);
    let ql = locked(side.cu_flags[side.idx4(qx, qy)], info);
    if pl && ql {
        return;
    }
    let qpl = (side.qp_y[side.idx4(qx, qy)] as i32 + side.qp_y[side.idx4(px, py)] as i32 + 1) >> 1;
    let scale = 1i32 << info.bit_depth_y.saturating_sub(8);
    let beta = BETA_TABLE[clip3(0, 51, qpl + ((cf.beta_offset_div2 as i32) << 1)) as usize] * scale;
    let qtc = clip3(
        0,
        53,
        qpl + 2 * (bs - 1) + ((cf.tc_offset_div2 as i32) << 1),
    );
    let tc = TC_TABLE[qtc as usize] * scale;
    let seg = Seg::new(y, ver, e, s, 4);
    let data = &mut *y.data;
    // Lines run across the edge: a vertical edge reads each line as one
    // contiguous run, a horizontal one reads eight rows of four.
    let mut sm = [[0i32; 8]; 4];
    if ver {
        for (i, line) in sm.iter_mut().enumerate() {
            let row: &[u16; 8] = data[seg.first + i * seg.line..][..8]
                .try_into()
                .expect("eight taps");
            *line = row.map(i32::from);
        }
    } else {
        for j in 0..8 {
            let row: &[u16; 4] = data[seg.first + j * seg.along..][..4]
                .try_into()
                .expect("four lines");
            for (line, &v) in sm.iter_mut().zip(row) {
                line[j] = v as i32;
            }
        }
    }
    let dp = |l: &[i32; 8]| (l[1] - 2 * l[2] + l[3]).abs();
    let dq = |l: &[i32; 8]| (l[6] - 2 * l[5] + l[4]).abs();
    let (dp0, dq0) = (dp(&sm[0]), dq(&sm[0]));
    let (dp3, dq3) = (dp(&sm[3]), dq(&sm[3]));
    let (dpq0, dpq3) = (dp0 + dq0, dp3 + dq3);
    if dpq0 + dpq3 >= beta {
        return;
    }
    let sam = |l: &[i32; 8], dpq: i32| {
        2 * dpq < (beta >> 2)
            && (l[0] - l[3]).abs() + (l[4] - l[7]).abs() < (beta >> 3)
            && (l[3] - l[4]).abs() < ((5 * tc + 1) >> 1)
    };
    let de = if sam(&sm[0], dpq0) && sam(&sm[3], dpq3) {
        2u8
    } else {
        1u8
    };
    let thr = (beta + (beta >> 1)) >> 3;
    let (dep, deq) = (dp0 + dp3 < thr, dq0 + dq3 < thr);
    let max = (1i32 << info.bit_depth_y) - 1;
    // Only p2..q2 can change, and a locked side keeps its samples.
    let (lo, hi) = (if pl { 4 } else { 1 }, if ql { 4 } else { 7 });
    let out = sm.map(|line| filter_line(&line, de, dep, deq, tc, max));
    if ver {
        for (i, o) in out.iter().enumerate() {
            let row = &mut data[seg.first + i * seg.line..][..8];
            for j in lo..hi {
                row[j] = o[j] as u16;
            }
        }
    } else {
        for j in lo..hi {
            let row = &mut data[seg.first + j * seg.along..][..4];
            for (v, o) in row.iter_mut().zip(&out) {
                *v = o[j] as u16;
            }
        }
    }
}

/// Filters every marked luma edge of one direction that the rows of `y`
/// own: vertical edges by the four-line segments inside those rows,
/// horizontal edges lying on those rows. Rows must be cut at a multiple of
/// eight plus four, so that a horizontal edge's taps never leave them.
pub(super) fn luma_dir(y: &mut PlaneMut<'_>, side: &Side<'_>, info: &FilterInfo, ver: bool) {
    let kind = if ver {
        edge::VER | edge::PU_VER
    } else {
        edge::HOR | edge::PU_HOR
    };
    let (w, h) = (y.width, y.height);
    let (elen, slen) = if ver { (w, h) } else { (h, w) };
    let (r0, r1) = (y.oy, y.end());
    for by in r0 / 8..r1.div_ceil(8).min(side.min8_h) {
        for bx in 0..side.min8_w {
            if side.edges[by * side.min8_w + bx] & kind == 0 {
                continue;
            }
            let (e, s0) = if ver {
                (bx * 8, by * 8)
            } else {
                (by * 8, bx * 8)
            };
            if e == 0 || e + 3 >= elen || (!ver && !(r0..r1).contains(&e)) {
                continue;
            }
            for seg in 0..2 {
                let s = s0 + seg * 4;
                if s + 3 < slen && (!ver || (r0..r1).contains(&s)) {
                    luma_segment(y, side, info, ver, e, s);
                }
            }
        }
    }
}

/// Deblocks the rows the views own: luma rows of `y`, chroma rows of
/// `cb`/`cr` (either may be `None`), vertical edges before horizontal ones.
pub(super) fn deblock_rows<'p>(
    y: Option<&mut PlaneMut<'_>>,
    c: Option<(&mut PlaneMut<'p>, &mut PlaneMut<'p>)>,
    side: &Side<'_>,
    info: &FilterInfo,
) {
    if let Some(y) = y {
        luma_dir(y, side, info, true);
        luma_dir(y, side, info, false);
    }
    if let Some((cb, cr)) = c {
        chroma::dir(cb, cr, side, info, true);
        chroma::dir(cb, cr, side, info, false);
    }
}

/// Applies the in-loop deblocking filter of clause 8.7.2 to `pic` in place.
///
/// All vertical edges of the picture are filtered before any horizontal edge,
/// so the horizontal pass reads samples the vertical pass already modified.
pub fn deblock(pic: &mut Picture, info: &FilterInfo) {
    let (y, cb, cr, side) = super::parts(pic);
    deblock_rows(
        Some(&mut y.view()),
        Some((&mut cb.view(), &mut cr.view())),
        &side,
        info,
    );
}

#[cfg(test)]
#[path = "deblock_tests.rs"]
mod tests;
