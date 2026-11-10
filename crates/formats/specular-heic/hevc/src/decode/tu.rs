//! Transform tree and transform unit parsing (clauses 7.3.8.8 and 7.3.8.10).

use super::recon::reconstruct;
use super::residual::residual_coding;
use super::state::Dec;
use crate::cabac::off;
use crate::error::Result;
use crate::picture::{edge, flags};

/// Parses `transform_tree(x0, y0, xBase, yBase, log2TrafoSize, trafoDepth, blkIdx)`.
#[allow(clippy::too_many_arguments)]
pub fn transform_tree(
    d: &mut Dec<'_>,
    x0: usize,
    y0: usize,
    x_base: usize,
    y_base: usize,
    log2_size: usize,
    depth: u32,
    blk_idx: usize,
    max_depth: u32,
) -> Result<()> {
    let dp = depth as usize;
    let forced = log2_size > d.sps.log2_max_tb || ((d.intra_split || d.inter_split) && depth == 0);
    let split = if log2_size <= d.sps.log2_max_tb
        && log2_size > d.sps.log2_min_tb
        && depth < max_depth
        && !(d.intra_split && depth == 0)
    {
        d.cab.decision(off::SPLIT_TRANSFORM + 5 - log2_size) != 0
    } else {
        forced
    };
    if (log2_size > 2 || d.cat == 3) && d.cat != 0 {
        let extra = d.cat == 2 && (!split || log2_size == 3);
        for c in 0..2 {
            let parent = if depth == 0 {
                true
            } else if c == 0 {
                d.cbf_cb[dp - 1][0] || d.cbf_cb[dp - 1][1]
            } else {
                d.cbf_cr[dp - 1][0] || d.cbf_cr[dp - 1][1]
            };
            let mut v = [false; 2];
            if parent {
                v[0] = d.cab.decision(off::CBF_CHROMA + dp) != 0;
                if extra {
                    v[1] = d.cab.decision(off::CBF_CHROMA + dp) != 0;
                }
            }
            if c == 0 {
                d.cbf_cb[dp] = v;
            } else {
                d.cbf_cr[dp] = v;
            }
        }
    } else if log2_size == 2 && d.cat == 1 && depth > 0 || log2_size == 2 && d.cat == 2 && depth > 0
    {
        d.cbf_cb[dp] = d.cbf_cb[dp - 1];
        d.cbf_cr[dp] = d.cbf_cr[dp - 1];
    }
    if split {
        let half = 1usize << (log2_size - 1);
        for (i, (dx, dy)) in [(0, 0), (half, 0), (0, half), (half, half)]
            .into_iter()
            .enumerate()
        {
            transform_tree(
                d,
                x0 + dx,
                y0 + dy,
                x0,
                y0,
                log2_size - 1,
                depth + 1,
                i,
                max_depth,
            )?;
        }
        return Ok(());
    }
    // An inter transform tree with no chroma residual at its root has a
    // luma residual there: rqt_root_cbf said so.
    let cbf_luma =
        if !d.cu_inter || depth != 0 || d.cbf_cb[dp].iter().chain(&d.cbf_cr[dp]).any(|&c| c) {
            d.cab.decision(off::CBF_LUMA + usize::from(depth == 0)) != 0
        } else {
            true
        };
    transform_unit(
        d, x0, y0, x_base, y_base, log2_size, depth, blk_idx, cbf_luma,
    )
}

/// Parses `transform_unit()` and reconstructs the block it covers.
#[allow(clippy::too_many_arguments)]
fn transform_unit(
    d: &mut Dec<'_>,
    x0: usize,
    y0: usize,
    x_base: usize,
    y_base: usize,
    log2_size: usize,
    depth: u32,
    blk_idx: usize,
    cbf_luma: bool,
) -> Result<()> {
    let dp = depth as usize;
    let small = d.cat != 3 && log2_size == 2;
    let log2_c = if d.cat == 3 {
        log2_size
    } else {
        log2_size.max(3) - 1
    };
    let cbf_depth_c = if small { dp - 1 } else { dp };
    let (xc, yc) = if small { (x_base, y_base) } else { (x0, y0) };
    let n_chroma = if d.cat == 2 { 2 } else { 1 };
    let chroma_here = d.cat != 0 && (!small || blk_idx == 3);
    let cbf_chroma =
        d.cat != 0 && (0..n_chroma).any(|t| d.cbf_cb[cbf_depth_c][t] || d.cbf_cr[cbf_depth_c][t]);
    if cbf_luma || cbf_chroma {
        if d.pps.cu_qp_delta_enabled && !d.qp_delta_coded {
            let delta = read_qp_delta(d)?;
            d.qp_delta_coded = true;
            d.set_qp_delta(delta);
        }
        if d.sh.cu_chroma_qp_offset_enabled && cbf_chroma && !d.tq_bypass && !d.cqo_coded {
            read_chroma_qp_offset(d)?;
        }
    }
    d.pic
        .mark_edge(x0, y0, 1 << log2_size, edge::VER | edge::HOR);
    let mode_y = if d.cu_inter { 1 } else { d.mode_at(x0, y0) };
    d.res_scale = [0; 2];
    if cbf_luma && d.cu_inter {
        // For the deblocking boundary strength (8.7.2.4).
        let n = 1usize << log2_size;
        let (x4, y4, w4) = ((x0 >> 2) - d.pic.ox4, (y0 >> 2) - d.pic.oy4, d.pic.min4_w);
        for j in 0..n >> 2 {
            let row = (y4 + j) * w4 + x4;
            let end = (row + (n >> 2)).min((y4 + j + 1) * w4);
            if let Some(s) = d.pic.cu_flags.get_mut(row..end) {
                for f in s {
                    *f |= flags::NZ;
                }
            }
        }
    }
    if cbf_luma {
        let ts = residual_coding(d, log2_size, 0, mode_y)?;
        reconstruct(d, x0, y0, log2_size, 0, mode_y, true, ts)?;
    } else {
        reconstruct(d, x0, y0, log2_size, 0, mode_y, false, false)?;
    }
    if chroma_here {
        let pb = d.pb_index(x0, y0);
        let mode_c = if d.cu_inter { 1 } else { d.mode_c[pb] };
        // cross_comp_pred() precedes each chroma component's residuals; it
        // can only occur in 4:4:4, where chroma is never deferred to blkIdx 3.
        let ccp = d.pps.cross_component_prediction
            && cbf_luma
            && (d.cu_inter || d.chroma_dm[pb])
            && !small;
        let (sw, sh) = (d.sps.sub_w, d.sps.sub_h);
        let (cx, cy) = (xc / sw, yc / sh);
        for c in 1..3usize {
            if ccp {
                d.res_scale[c - 1] = read_cross_comp_pred(d, c - 1);
            }
            for t in 0..n_chroma {
                let cbf = if c == 1 {
                    d.cbf_cb[cbf_depth_c][t]
                } else {
                    d.cbf_cr[cbf_depth_c][t]
                };
                let y = cy + (t << log2_c);
                if cbf {
                    let ts = residual_coding(d, log2_c, c, mode_c)?;
                    reconstruct(d, cx, y, log2_c, c, mode_c, true, ts)?;
                } else {
                    reconstruct(d, cx, y, log2_c, c, mode_c, false, false)?;
                }
            }
        }
    }
    Ok(())
}

/// Parses `cross_comp_pred(x0, y0, c)` (clause 7.3.8.12) and returns
/// `ResScaleVal[c + 1]` (equation 7-80).
fn read_cross_comp_pred(d: &mut Dec<'_>, c: usize) -> i32 {
    // log2_res_scale_abs_plus1: TR with cMax = 4, ctxInc = 4 * c + binIdx.
    let mut v = 0usize;
    while v < 4 && d.cab.decision(off::RES_SCALE_ABS + 4 * c + v) != 0 {
        v += 1;
    }
    if v == 0 {
        return 0;
    }
    let neg = d.cab.decision(off::RES_SCALE_SIGN + c) != 0;
    let mag = 1i32 << (v - 1);
    if neg { -mag } else { mag }
}

/// Parses `cu_qp_delta_abs` and `cu_qp_delta_sign_flag` (clause 9.3.3.10).
fn read_qp_delta(d: &mut Dec<'_>) -> Result<i32> {
    let mut prefix = 0u32;
    while prefix < 5 {
        let ctx = off::CU_QP_DELTA + usize::from(prefix > 0);
        if d.cab.decision(ctx) == 0 {
            break;
        }
        prefix += 1;
    }
    let mut v = prefix;
    if prefix == 5 {
        let mut k = 0u32;
        while d.cab.bypass() == 1 && k < 24 {
            k += 1;
        }
        v = 5 + (1u32 << k) - 1 + d.cab.bypass_bits(k);
    }
    if v == 0 {
        return Ok(0);
    }
    let sign = d.cab.bypass();
    Ok(if sign == 1 { -(v as i32) } else { v as i32 })
}

/// Parses `cu_chroma_qp_offset_flag` and `cu_chroma_qp_offset_idx`.
fn read_chroma_qp_offset(d: &mut Dec<'_>) -> Result<()> {
    d.cqo_coded = true;
    if d.cab.decision(off::CHROMA_QP_FLAG) == 0 {
        d.cqo_cb = 0;
        d.cqo_cr = 0;
        return Ok(());
    }
    let len = d.pps.cb_qp_offset_list.len();
    let mut idx = 0usize;
    while idx + 1 < len {
        if d.cab.decision(off::CHROMA_QP_IDX) == 0 {
            break;
        }
        idx += 1;
    }
    d.cqo_cb = d.pps.cb_qp_offset_list.get(idx).copied().unwrap_or(0);
    d.cqo_cr = d.pps.cr_qp_offset_list.get(idx).copied().unwrap_or(0);
    Ok(())
}
