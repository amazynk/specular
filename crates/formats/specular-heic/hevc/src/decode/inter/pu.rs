//! Inter coding units: `cu_skip_flag`, `part_mode`, `prediction_unit()`
//! (7.3.8.5, 7.3.8.6, 7.3.8.9) and their decoding (8.5.3).

use super::super::state::Dec;
use super::super::tu::transform_tree;
use super::mc;
use super::mvp::{self, Cb, Pb, part};
use crate::cabac::off;
use crate::error::Result;
use crate::picture::{MvField, Picture, edge, flags};

/// `cu_skip_flag`, with its context from the neighbours (9.3.4.2.2).
pub fn skip_flag(d: &mut Dec<'_>, x0: usize, y0: usize) -> bool {
    let mut inc = 0usize;
    if d.available(x0, y0, x0 as isize - 1, y0 as isize)
        && d.pic.cu_flags[d.pic.idx4(x0 - 1, y0)] & flags::SKIP != 0
    {
        inc += 1;
    }
    if d.available(x0, y0, x0 as isize, y0 as isize - 1)
        && d.pic.cu_flags[d.pic.idx4(x0, y0 - 1)] & flags::SKIP != 0
    {
        inc += 1;
    }
    d.cab.decision(off::SKIP + inc) != 0
}

/// `part_mode` of an inter coding unit (Table 9-43).
fn part_mode(d: &mut Dec<'_>, log2: usize) -> u8 {
    if d.cab.decision(off::PART_MODE) != 0 {
        return part::P2NX2N;
    }
    let min = log2 == d.sps.log2_min_cb;
    let horizontal = d.cab.decision(off::PART_MODE_EXT) != 0;
    if min {
        if horizontal {
            return part::P2NXN;
        }
        if log2 == 3 {
            return part::PNX2N;
        }
        return if d.cab.decision(off::PART_MODE_EXT + 1) != 0 {
            part::PNX2N
        } else {
            part::PNXN
        };
    }
    if !d.sps.amp {
        return if horizontal { part::P2NXN } else { part::PNX2N };
    }
    let symmetric = d.cab.decision(off::PART_MODE_EXT + 2) != 0;
    match (horizontal, symmetric) {
        (true, true) => part::P2NXN,
        (false, true) => part::PNX2N,
        (true, false) => {
            if d.cab.bypass() == 0 {
                part::P2NXNU
            } else {
                part::P2NXND
            }
        }
        (false, false) => {
            if d.cab.bypass() == 0 {
                part::PNLX2N
            } else {
                part::PNRX2N
            }
        }
    }
}

/// The prediction blocks of a coding block of side `s` (Table 7-10).
fn layout(p: u8, s: usize) -> ([(usize, usize, usize, usize); 4], usize) {
    let (h, q) = (s / 2, s / 4);
    let z = (0, 0, 0, 0);
    match p {
        part::P2NXN => ([(0, 0, s, h), (0, h, s, h), z, z], 2),
        part::PNX2N => ([(0, 0, h, s), (h, 0, h, s), z, z], 2),
        part::PNXN => ([(0, 0, h, h), (h, 0, h, h), (0, h, h, h), (h, h, h, h)], 4),
        part::P2NXNU => ([(0, 0, s, q), (0, q, s, s - q), z, z], 2),
        part::P2NXND => ([(0, 0, s, s - q), (0, s - q, s, q), z, z], 2),
        part::PNLX2N => ([(0, 0, q, s), (q, 0, s - q, s), z, z], 2),
        part::PNRX2N => ([(0, 0, s - q, s), (s - q, 0, q, s), z, z], 2),
        _ => ([(0, 0, s, s), z, z, z], 1),
    }
}

/// `merge_idx`: truncated rice with the first bin context coded.
fn merge_idx(d: &mut Dec<'_>) -> usize {
    let max = (d.sh.max_num_merge_cand as usize).saturating_sub(1);
    if max == 0 {
        return 0;
    }
    let mut v = 0usize;
    if d.cab.decision(off::MERGE_IDX) == 0 {
        return 0;
    }
    v += 1;
    while v < max && d.cab.bypass() != 0 {
        v += 1;
    }
    v
}

/// `ref_idx_lX`: truncated rice, two context coded bins then bypass.
fn ref_idx(d: &mut Dec<'_>, n: usize) -> i8 {
    let max = n.saturating_sub(1);
    let mut v = 0usize;
    while v < max {
        let bit = if v < 2 {
            d.cab.decision(off::REF_IDX + v)
        } else {
            d.cab.bypass()
        };
        if bit == 0 {
            break;
        }
        v += 1;
    }
    v as i8
}

/// First-order Exp-Golomb bypass bins (9.3.3.3), bounded.
fn eg1(d: &mut Dec<'_>) -> i32 {
    let mut k = 1u32;
    let mut v = 0i32;
    while d.cab.bypass() != 0 {
        v = v.saturating_add(1 << k);
        k += 1;
        if k > 20 {
            break;
        }
    }
    v.saturating_add(d.cab.bypass_bits(k) as i32)
}

/// `mvd_coding()` (7.3.8.9).
fn mvd(d: &mut Dec<'_>) -> [i32; 2] {
    let g0 = [
        d.cab.decision(off::MVD_GT0) != 0,
        d.cab.decision(off::MVD_GT0) != 0,
    ];
    let g1 = [
        g0[0] && d.cab.decision(off::MVD_GT1) != 0,
        g0[1] && d.cab.decision(off::MVD_GT1) != 0,
    ];
    let mut out = [0i32; 2];
    for c in 0..2 {
        if g0[c] {
            let mag = if g1[c] { eg1(d) + 2 } else { 1 };
            out[c] = if d.cab.bypass() != 0 { -mag } else { mag };
        }
    }
    out
}

/// Parses and decodes one prediction block; returns `merge_flag`.
fn prediction_unit(d: &mut Dec<'_>, cb: &Cb, pb: &Pb, skip: bool, depth: u8) -> bool {
    let merge = skip || d.cab.decision(off::MERGE_FLAG) != 0;
    let m = if merge {
        let idx = merge_idx(d);
        mvp::merge(d, cb, pb, idx)
    } else {
        // inter_pred_idc: 0 PRED_L0, 1 PRED_L1, 2 PRED_BI.
        let idc = if d.sh.is_b() {
            if pb.w + pb.h != 12 && d.cab.decision(off::INTER_PRED_IDC + usize::from(depth)) != 0 {
                2
            } else {
                d.cab.decision(off::INTER_PRED_IDC + 4)
            }
        } else {
            0
        };
        let mut m = MvField::default();
        let mut parsed = [(0i8, [0i32; 2], 0usize); 2];
        for (l, p) in parsed.iter_mut().enumerate() {
            let uses = if l == 0 { idc != 1 } else { idc != 0 };
            if !uses {
                continue;
            }
            let r = ref_idx(d, d.sl.n[l]);
            let v = if l == 1 && d.sh.mvd_l1_zero && idc == 2 {
                [0, 0]
            } else {
                mvd(d)
            };
            let flag = d.cab.decision(off::MVP_FLAG) as usize;
            *p = (r, v, flag);
            m.ref_idx[l] = r;
        }
        for (l, &(r, v, flag)) in parsed.iter().enumerate() {
            if !m.pred(l) {
                continue;
            }
            let p = mvp::amvp(d, cb, pb, l, r, flag);
            m.mv[l] = [
                (i32::from(p[0]) + v[0]) as i16,
                (i32::from(p[1]) + v[1]) as i16,
            ];
        }
        m
    };
    let mut m = m;
    for l in 0..2 {
        if m.pred(l) {
            m.ref_id[l] = d.sl.id[l][(m.ref_idx[l] as usize).min(15)];
        }
    }
    Picture::fill4_rect(d.pic.g4(), &mut d.pic.mvf, pb.x, pb.y, pb.w, pb.h, m);
    d.pic.mark_pu_edges(pb.x, pb.y, pb.w, pb.h);
    mc::predict(d, pb.x, pb.y, pb.w, pb.h, &m);
    merge
}

/// Decodes an inter coding unit after `cu_skip_flag` (and, when not
/// skipped, `pred_mode_flag`) has been read.
pub fn coding_unit(
    d: &mut Dec<'_>,
    x0: usize,
    y0: usize,
    log2: usize,
    depth: u8,
    skip: bool,
) -> Result<()> {
    let size = 1usize << log2;
    let p = if skip {
        part::P2NX2N
    } else {
        part_mode(d, log2)
    };
    d.cu_inter = true;
    d.intra_split = false;
    d.inter_split = d.sps.max_tr_depth_inter == 0 && p != part::P2NX2N;
    let mut cu_flags = flags::INTER;
    if skip {
        cu_flags |= flags::SKIP;
    }
    if d.tq_bypass {
        cu_flags |= flags::TQ_BYPASS;
    }
    Picture::fill4(d.pic.g4(), &mut d.pic.cu_flags, x0, y0, size, cu_flags);
    d.pic.mark_edge(x0, y0, size, edge::VER | edge::HOR);
    let cb = Cb {
        x: x0,
        y: y0,
        size,
        part: p,
    };
    let (pbs, n) = layout(p, size);
    let mut merge0 = false;
    for (i, &(dx, dy, w, h)) in pbs[..n].iter().enumerate() {
        let pb = Pb {
            x: x0 + dx,
            y: y0 + dy,
            w,
            h,
            idx: i,
        };
        let merged = prediction_unit(d, &cb, &pb, skip, depth);
        if i == 0 {
            merge0 = merged;
        }
    }
    if !skip {
        let root = if p == part::P2NX2N && merge0 {
            true
        } else {
            d.cab.decision(off::RQT_ROOT_CBF) != 0
        };
        if root {
            d.cbf_cb = [[false; 2]; 6];
            d.cbf_cr = [[false; 2]; 6];
            let max_depth = d.sps.max_tr_depth_inter;
            transform_tree(d, x0, y0, x0, y0, log2, 0, 0, max_depth)?;
        }
    }
    d.store_qp(x0, y0, size);
    d.cu_inter = false;
    d.inter_split = false;
    Ok(())
}
