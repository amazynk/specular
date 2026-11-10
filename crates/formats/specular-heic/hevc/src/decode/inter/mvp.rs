//! Motion vector prediction: merge mode (8.5.3.2.2 to 8.5.3.2.5), the
//! luma motion vector predictors of AMVP (8.5.3.2.6 and 8.5.3.2.7) and
//! temporal prediction (8.5.3.2.8 and 8.5.3.2.9).

use super::super::state::Dec;
use super::scale_mv;
use crate::picture::{MvField, flags};

/// `PartMode` values of an inter coding unit (Table 7-10).
pub mod part {
    /// `PART_2Nx2N`.
    pub const P2NX2N: u8 = 0;
    /// `PART_2NxN`.
    pub const P2NXN: u8 = 1;
    /// `PART_Nx2N`.
    pub const PNX2N: u8 = 2;
    /// `PART_NxN`.
    pub const PNXN: u8 = 3;
    /// `PART_2NxnU`.
    pub const P2NXNU: u8 = 4;
    /// `PART_2NxnD`.
    pub const P2NXND: u8 = 5;
    /// `PART_nLx2N`.
    pub const PNLX2N: u8 = 6;
    /// `PART_nRx2N`.
    pub const PNRX2N: u8 = 7;
}

/// The coding block a prediction block belongs to.
#[derive(Clone, Copy)]
pub struct Cb {
    /// Luma position.
    pub x: usize,
    /// Luma position.
    pub y: usize,
    /// `nCbS`.
    pub size: usize,
    /// `PartMode`.
    pub part: u8,
}

/// A prediction block.
#[derive(Clone, Copy)]
pub struct Pb {
    /// Luma position.
    pub x: usize,
    /// Luma position.
    pub y: usize,
    /// `nPbW`.
    pub w: usize,
    /// `nPbH`.
    pub h: usize,
    /// `partIdx`.
    pub idx: usize,
}

/// Same motion for pruning: the lists used, and their vectors and indices.
#[inline]
fn same_motion(a: &MvField, b: &MvField) -> bool {
    (0..2).all(|l| {
        a.pred(l) == b.pred(l)
            && (!a.pred(l) || (a.mv[l] == b.mv[l] && a.ref_idx[l] == b.ref_idx[l]))
    })
}

/// The motion stored for luma position `(x, y)`.
#[inline]
fn mvf_at(d: &Dec<'_>, x: usize, y: usize) -> MvField {
    d.pic.mvf.get(d.pic.idx4(x, y)).copied().unwrap_or_default()
}

/// Availability of a neighbouring prediction block (clause 6.4.2).
fn pb_avail(d: &Dec<'_>, cb: &Cb, pb: &Pb, xn: isize, yn: isize) -> bool {
    let same_cb = xn >= cb.x as isize
        && yn >= cb.y as isize
        && xn < (cb.x + cb.size) as isize
        && yn < (cb.y + cb.size) as isize;
    let avail = if !same_cb {
        d.available(pb.x, pb.y, xn, yn)
    } else {
        !((pb.w << 1) == cb.size
            && (pb.h << 1) == cb.size
            && pb.idx == 1
            && (cb.y + pb.h) as isize <= yn
            && ((cb.x + pb.w) as isize) > xn)
    };
    avail && d.pic.cu_flags[d.pic.idx4(xn as usize, yn as usize)] & flags::INTER != 0
}

/// `PicOrderCntVal` of the picture the reference table entry `id` holds.
#[inline]
fn poc_of(d: &Dec<'_>, id: u8) -> i32 {
    d.ictx
        .and_then(|c| c.refs.get(usize::from(id)))
        .map_or(0, |e| e.poc)
}

/// Long-term marking of the reference table entry `id`.
#[inline]
fn lt_of(d: &Dec<'_>, id: u8) -> bool {
    d.ictx
        .and_then(|c| c.refs.get(usize::from(id)))
        .is_some_and(|e| e.lt)
}

/// The temporal luma motion vector prediction for list `x`, reference
/// `ref_idx` (clause 8.5.3.2.8).
pub fn temporal(d: &Dec<'_>, pb: &Pb, ref_idx: i8, x: usize) -> Option<[i16; 2]> {
    if !d.sh.temporal_mvp {
        return None;
    }
    let col = d.sl.col.as_ref()?;
    if !col.has_col() {
        return None;
    }
    let (xbr, ybr) = (pb.x + pb.w, pb.y + pb.h);
    let log2_ctb = d.sps.log2_ctb;
    if pb.y >> log2_ctb == ybr >> log2_ctb && ybr < d.sps.height && xbr < d.sps.width {
        let r = col_mv(d, col, (xbr >> 4) << 4, (ybr >> 4) << 4, ref_idx, x);
        if r.is_some() {
            return r;
        }
    }
    let (xc, yc) = (pb.x + (pb.w >> 1), pb.y + (pb.h >> 1));
    col_mv(d, col, (xc >> 4) << 4, (yc >> 4) << 4, ref_idx, x)
}

/// The collocated motion vectors (clause 8.5.3.2.9).
fn col_mv(
    d: &Dec<'_>,
    col: &super::RefPic,
    x: usize,
    y: usize,
    ref_idx: i8,
    lx: usize,
) -> Option<[i16; 2]> {
    let c = col.col_at(x, y);
    if c.flags & 3 == 0 {
        return None;
    }
    let list = if !c.pred(0) {
        1
    } else if !c.pred(1) {
        0
    } else if d.sl.no_backward {
        lx
    } else {
        usize::from(d.sh.collocated_from_l0)
    };
    let ri = (ref_idx.max(0) as usize).min(15);
    let cur_lt = d.sl.lt[lx][ri];
    if cur_lt != c.lt(list) {
        return None;
    }
    let mv = c.mv[list];
    let col_diff = col.poc - c.poc[list];
    let cur_diff = d.sl.cur_poc - d.sl.poc[lx][ri];
    if cur_lt || col_diff == cur_diff {
        Some(mv)
    } else {
        Some(scale_mv(mv, col_diff, cur_diff))
    }
}

/// Derives the motion of a merge-mode prediction block (clause 8.5.3.2.2).
pub fn merge(d: &Dec<'_>, cb: &Cb, pb: &Pb, merge_idx: usize) -> MvField {
    let orig = *pb;
    let pb = if d.pps.log2_par_mrg_level > 2 && cb.size == 8 {
        Pb {
            x: cb.x,
            y: cb.y,
            w: cb.size,
            h: cb.size,
            idx: 0,
        }
    } else {
        *pb
    };
    let mut list = [MvField::default(); 5];
    let mut n = 0usize;
    let pml = d.pps.log2_par_mrg_level;
    let same_mer = |xn: isize, yn: isize| {
        (pb.x >> pml) as isize == xn >> pml && (pb.y >> pml) as isize == yn >> pml
    };
    let cand = |xn: isize, yn: isize| -> Option<MvField> {
        if same_mer(xn, yn) || !pb_avail(d, cb, &pb, xn, yn) {
            None
        } else {
            Some(mvf_at(d, xn as usize, yn as usize))
        }
    };
    let (x, y, w, h) = (pb.x as isize, pb.y as isize, pb.w as isize, pb.h as isize);
    let vertical_second =
        pb.idx == 1 && matches!(cb.part, part::PNX2N | part::PNLX2N | part::PNRX2N);
    let horizontal_second =
        pb.idx == 1 && matches!(cb.part, part::P2NXN | part::P2NXNU | part::P2NXND);
    let a1 = if vertical_second {
        None
    } else {
        cand(x - 1, y + h - 1)
    };
    // Pruning compares with the neighbours' motion whenever they are
    // available, even when they were themselves pruned from the list.
    let b1_raw = if horizontal_second {
        None
    } else {
        cand(x + w - 1, y - 1)
    };
    let b1 = b1_raw.filter(|b| a1.is_none_or(|a| !same_motion(&a, b)));
    let b0 = cand(x + w, y - 1).filter(|b| b1_raw.is_none_or(|a| !same_motion(&a, b)));
    let a0 = cand(x - 1, y + h).filter(|b| a1.is_none_or(|a| !same_motion(&a, b)));
    let four = [a1, b1, b0, a0].iter().all(Option::is_some);
    let b2 = if four {
        None
    } else {
        cand(x - 1, y - 1).filter(|b| {
            a1.is_none_or(|a| !same_motion(&a, b)) && b1_raw.is_none_or(|a| !same_motion(&a, b))
        })
    };
    for c in [a1, b1, b0, a0, b2].into_iter().flatten() {
        list[n] = c;
        n += 1;
    }
    let max = (d.sh.max_num_merge_cand as usize).clamp(1, 5);
    let b_slice = d.sh.is_b();
    if merge_idx >= n {
        // Temporal candidate, reference index 0 in each list.
        let mut c = MvField::default();
        if let Some(mv) = temporal(d, &pb, 0, 0) {
            c.mv[0] = mv;
            c.ref_idx[0] = 0;
        }
        if b_slice && let Some(mv) = temporal(d, &pb, 0, 1) {
            c.mv[1] = mv;
            c.ref_idx[1] = 0;
        }
        if c.pred(0) || c.pred(1) {
            list[n] = c;
            n += 1;
        }
    }
    let mut out = if merge_idx < n {
        list[merge_idx]
    } else {
        let orig_n = n;
        let mut extra = [MvField::default(); 5];
        let mut m = 0usize;
        // Combined bi-predictive candidates (8.5.3.2.4).
        if b_slice && orig_n > 1 && orig_n < max {
            const L0: [usize; 12] = [0, 1, 0, 2, 1, 2, 0, 3, 1, 3, 2, 3];
            const L1: [usize; 12] = [1, 0, 2, 0, 2, 1, 3, 0, 3, 1, 3, 2];
            let mut k = 0usize;
            while k < orig_n * (orig_n - 1) && orig_n + m < max {
                let (p, q) = (list[L0[k]], list[L1[k]]);
                if p.pred(0) && q.pred(1) {
                    let id0 = d.sl.id[0][(p.ref_idx[0] as usize).min(15)];
                    let id1 = d.sl.id[1][(q.ref_idx[1] as usize).min(15)];
                    if id0 != id1 || p.mv[0] != q.mv[1] {
                        let mut c = MvField::default();
                        c.mv[0] = p.mv[0];
                        c.ref_idx[0] = p.ref_idx[0];
                        c.mv[1] = q.mv[1];
                        c.ref_idx[1] = q.ref_idx[1];
                        extra[m] = c;
                        m += 1;
                    }
                }
                k += 1;
            }
        }
        let i = merge_idx - orig_n;
        if i < m {
            extra[i]
        } else {
            // Zero candidates (8.5.3.2.5).
            let zero_idx = i - m;
            let num_ref = if b_slice {
                d.sl.n[0].min(d.sl.n[1])
            } else {
                d.sl.n[0]
            };
            let r = if zero_idx < num_ref {
                zero_idx as i8
            } else {
                0
            };
            let mut c = MvField::default();
            c.ref_idx[0] = r;
            if b_slice {
                c.ref_idx[1] = r;
            }
            c
        }
    };
    if out.pred(0) && out.pred(1) && orig.w + orig.h == 12 {
        out.ref_idx[1] = -1;
        out.mv[1] = [0, 0];
    }
    for l in 0..2 {
        if !out.pred(l) {
            out.mv[l] = [0, 0];
        }
    }
    out
}

/// One spatial candidate of AMVP, searched without scaling: the
/// neighbour's vector in list `x` or else `y` when it refers to the target
/// picture.
fn same_pic(d: &Dec<'_>, m: &MvField, x: usize, target_poc: i32) -> Option<[i16; 2]> {
    for l in [x, 1 - x] {
        if m.pred(l) && poc_of(d, m.ref_id[l]) == target_poc {
            return Some(m.mv[l]);
        }
    }
    None
}

/// The second, scaling search: a neighbour whose reference has the same
/// long-term marking as the target, scaled when both are short-term.
fn scaled(
    d: &Dec<'_>,
    m: &MvField,
    x: usize,
    target_poc: i32,
    target_lt: bool,
) -> Option<[i16; 2]> {
    for l in [x, 1 - x] {
        if m.pred(l) && lt_of(d, m.ref_id[l]) == target_lt {
            let mv = m.mv[l];
            if target_lt {
                return Some(mv);
            }
            let rpoc = poc_of(d, m.ref_id[l]);
            let td = d.sl.cur_poc - rpoc;
            let tb = d.sl.cur_poc - target_poc;
            return Some(if rpoc == target_poc {
                mv
            } else {
                scale_mv(mv, td, tb)
            });
        }
    }
    None
}

/// The luma motion vector predictor `mvpLX` (clause 8.5.3.2.6).
pub fn amvp(d: &Dec<'_>, cb: &Cb, pb: &Pb, x: usize, ref_idx: i8, flag: usize) -> [i16; 2] {
    let ri = (ref_idx.max(0) as usize).min(15);
    let target_poc = d.sl.poc[x][ri];
    let target_lt = d.sl.lt[x][ri];
    let (px, py, w, h) = (pb.x as isize, pb.y as isize, pb.w as isize, pb.h as isize);
    let nb = |xn: isize, yn: isize| -> Option<MvField> {
        pb_avail(d, cb, pb, xn, yn).then(|| mvf_at(d, xn as usize, yn as usize))
    };
    let a = [nb(px - 1, py + h), nb(px - 1, py + h - 1)];
    let is_scaled = a.iter().any(Option::is_some);
    let mut mv_a = a
        .iter()
        .flatten()
        .find_map(|m| same_pic(d, m, x, target_poc));
    if mv_a.is_none() {
        mv_a = a
            .iter()
            .flatten()
            .find_map(|m| scaled(d, m, x, target_poc, target_lt));
    }
    let b = [
        nb(px + w, py - 1),
        nb(px + w - 1, py - 1),
        nb(px - 1, py - 1),
    ];
    let mut mv_b = b
        .iter()
        .flatten()
        .find_map(|m| same_pic(d, m, x, target_poc));
    if !is_scaled {
        if mv_b.is_some() {
            mv_a = mv_b;
        }
        mv_b = b
            .iter()
            .flatten()
            .find_map(|m| scaled(d, m, x, target_poc, target_lt));
    }
    let mut list = [[0i16; 2]; 2];
    let mut n = 0usize;
    if let Some(v) = mv_a {
        list[n] = v;
        n += 1;
    }
    if let Some(v) = mv_b
        && (mv_a != Some(v))
    {
        list[n] = v;
        n += 1;
    }
    if n < 2
        && flag >= n
        && let Some(v) = temporal(d, pb, ref_idx, x)
    {
        list[n] = v;
    }
    list[flag.min(1)]
}
