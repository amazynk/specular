//! The chroma half of the deblocking filter, clause 8.7.2.5.5.
//!
//! Chroma is only ever filtered with the short two-tap form — there is no
//! strong filter and no `dE` derivation — so it shares nothing with the luma
//! path but the tables and the segment addressing, both of which stay in the
//! parent module.

use super::super::{FilterInfo, Side, chroma_qp_from_luma};
use super::{BS, Seg, TC_TABLE, clip3, locked, strength};
use crate::picture::{PlaneMut, edge};

/// Filters one four-line chroma segment of both chroma planes.
fn segment(
    planes: [&mut PlaneMut<'_>; 2],
    side: &Side<'_>,
    info: &FilterInfo,
    ver: bool,
    e: usize,
    s: usize,
) {
    let (sw, sh) = (info.sub_w, info.sub_h);
    let (lqx, lqy) = if ver {
        (e * sw, s * sh)
    } else {
        (s * sw, e * sh)
    };
    let (lpx, lpy) = if ver { (lqx - 1, lqy) } else { (lqx, lqy - 1) };
    let qc = info.ctb_of(lqx, lqy);
    let cf = match info.ctb.get(qc) {
        Some(c) => *c,
        None => return,
    };
    if cf.deblock_disabled || !info.may_cross(info.ctb_of(lpx, lpy), qc) {
        return;
    }
    // Chroma edges are filtered only where an intra block meets them.
    if strength(side, lqx, lqy, ver) != BS {
        return;
    }
    let pl = locked(side.cu_flags[side.idx4(lpx, lpy)], info);
    let ql = locked(side.cu_flags[side.idx4(lqx, lqy)], info);
    if pl && ql {
        return;
    }
    let qpl =
        (side.qp_y[side.idx4(lqx, lqy)] as i32 + side.qp_y[side.idx4(lpx, lpy)] as i32 + 1) >> 1;
    let scale = 1i32 << info.bit_depth_c.saturating_sub(8);
    let max = (1i32 << info.bit_depth_c) - 1;
    for (comp, plane) in planes.into_iter().enumerate() {
        let off = i32::from(if comp == 0 {
            cf.cb_qp_offset
        } else {
            cf.cr_qp_offset
        });
        let qpc = chroma_qp_from_luma(qpl + off, info.chroma_array_type);
        let qtc = clip3(
            0,
            53,
            qpc + 2 * (BS - 1) + ((cf.tc_offset_div2 as i32) << 1),
        );
        let tc = TC_TABLE[qtc as usize] * scale;
        let seg = Seg::new(plane, ver, e, s, 2);
        let data = &mut *plane.data;
        for i in 0..4 {
            let (p1, p0) = (data[seg.at(i, 0)] as i32, data[seg.at(i, 1)] as i32);
            let (q0, q1) = (data[seg.at(i, 2)] as i32, data[seg.at(i, 3)] as i32);
            let d = clip3(-tc, tc, (((q0 - p0) << 2) + p1 - q1 + 4) >> 3);
            if !pl {
                data[seg.at(i, 1)] = clip3(0, max, p0 + d) as u16;
            }
            if !ql {
                data[seg.at(i, 2)] = clip3(0, max, q0 - d) as u16;
            }
        }
    }
}

/// Filters every marked chroma edge of one direction that the rows of
/// `cb`/`cr` own (see `luma_dir`; chroma rows are cut at a multiple of
/// eight plus four too).
pub(super) fn dir<'p>(
    cb: &mut PlaneMut<'p>,
    cr: &mut PlaneMut<'p>,
    side: &Side<'_>,
    info: &FilterInfo,
    ver: bool,
) {
    if info.chroma_array_type == 0 || info.sub_w == 0 || info.sub_h == 0 {
        return;
    }
    let kind = if ver {
        edge::VER | edge::PU_VER
    } else {
        edge::HOR | edge::PU_HOR
    };
    let (sw, sh) = (info.sub_w, info.sub_h);
    let (cw, ch) = (cb.width, cb.height);
    let (elen, slen) = if ver { (cw, ch) } else { (ch, cw) };
    let (r0, r1) = (cb.oy, cb.end());
    // Edges lie on multiples of 8, segments start on multiples of 4: walk
    // only those inside the rows owned.
    let (mut e, e_end, s0, s_end) = if ver {
        (8, elen, r0.next_multiple_of(4), r1)
    } else {
        (r0.next_multiple_of(8).max(8), r1, 0, slen)
    };
    while e + 1 < elen && e < e_end {
        let mut s = s0;
        while s + 3 < slen && s < s_end {
            let (lx, ly) = if ver {
                (e * sw, s * sh)
            } else {
                (s * sw, e * sh)
            };
            let cell = (ly >> 3) * side.min8_w + (lx >> 3);
            if side.edges.get(cell).map_or(0u8, |v| *v) & kind != 0 {
                segment([&mut *cb, &mut *cr], side, info, ver, e, s);
            }
            s += 4;
        }
        e += 8;
    }
}
