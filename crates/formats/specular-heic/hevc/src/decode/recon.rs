//! Block reconstruction: intra prediction plus the inverse-transformed residual.

use super::refs::gather_refs;
use super::state::Dec;
use crate::error::Result;
use crate::intra::{filter_refs, predict_with};
use crate::transform::{
    inverse_transform, inverse_transform_ext, scale_range, transform_skip, transform_skip_ext,
};

/// Predicts one transform block and adds its residual into the picture.
///
/// `x` and `y` are in the coordinates of component `c_idx`. When `has_resid`
/// is false there are no coded coefficients; the prediction is written out
/// unchanged unless cross-component prediction supplies a residual.
#[allow(clippy::too_many_arguments)]
pub fn reconstruct(
    d: &mut Dec<'_>,
    x: usize,
    y: usize,
    log2: usize,
    c_idx: usize,
    mode: u8,
    has_resid: bool,
    ts: bool,
) -> Result<()> {
    let n = 1usize << log2;
    let bd = if c_idx == 0 {
        d.sps.bit_depth_y
    } else {
        d.sps.bit_depth_c
    };
    let inter = d.cu_inter;
    if inter {
        // Motion compensation already wrote the prediction into the
        // picture; the residual is added to it there.
        let res_scale = if c_idx > 0 && d.cat == 3 {
            d.res_scale[c_idx - 1]
        } else {
            0
        };
        if !has_resid && res_scale == 0 {
            return Ok(());
        }
    } else {
        let raw = gather_refs(d, x, y, n, c_idx);
        let filtered = filter_refs(
            &raw,
            mode,
            c_idx,
            bd,
            d.cat,
            d.sps.strong_intra_smoothing,
            d.sps.intra_smoothing_disabled,
        );
        // disableIntraBoundaryFilter (clause 8.4.4.2.6).
        let edge_filter = !(d.sps.implicit_rdpcm && d.tq_bypass);
        predict_with(&filtered, mode, c_idx, bd, &mut d.pred, n, edge_filter);
    }
    if has_resid {
        residual(d, n, c_idx, mode, ts, bd);
    }
    // Cross-component prediction (clause 8.6.6), 4:4:4 only.
    let res_scale = if c_idx > 0 && d.cat == 3 {
        d.res_scale[c_idx - 1]
    } else {
        0
    };
    if c_idx == 0 && has_resid && d.pps.cross_component_prediction {
        d.res_y[..n * n].copy_from_slice(&d.coeffs[..n * n]);
    }
    if res_scale != 0 {
        if !has_resid {
            d.coeffs[..n * n].fill(0);
        }
        let (bd_y, bd_c) = (u32::from(d.sps.bit_depth_y), u32::from(bd));
        for (r, &ry) in d.coeffs[..n * n].iter_mut().zip(d.res_y[..n * n].iter()) {
            let add = (i64::from(res_scale) * ((i64::from(ry) << bd_c) >> bd_y)) >> 3;
            // Any |r| beyond 2^24 clips the same way in the final add
            // (samples are at most 16 bits), so the clamp is exact.
            *r = (i64::from(*r) + add).clamp(-(1 << 24), 1 << 24) as i32;
        }
    }
    let add_resid = has_resid || res_scale != 0;
    let max = (1i32 << bd) - 1;
    let plane = match c_idx {
        0 => &mut d.pic.y,
        1 => &mut d.pic.cb,
        _ => &mut d.pic.cr,
    };
    let w = n.min(plane.width.saturating_sub(x));
    let h = n.min(plane.height.saturating_sub(y));
    if w == 0 {
        // A damaged stream's block wholly right of the plane: an empty
        // row's offset could lie past the end of the samples.
        return Ok(());
    }
    for j in 0..h {
        let o = plane.offset(x, y + j);
        let dst = &mut plane.data[o..o + w];
        if inter {
            // add_resid holds: an inter block without one returned above.
            let res = &d.coeffs[j * n..j * n + w];
            for (o, &r) in dst.iter_mut().zip(res.iter()) {
                *o = (*o as i32 + r).clamp(0, max) as u16;
            }
            continue;
        }
        let pred = &d.pred[j * n..j * n + w];
        if add_resid {
            let res = &d.coeffs[j * n..j * n + w];
            for ((o, &p), &r) in dst.iter_mut().zip(pred.iter()).zip(res.iter()) {
                // |r| < 2^28: coefficients are clipped to CoeffMin/Max
                // (at most 2^22) before RDPCM sums n <= 32 of them.
                *o = (p as i32 + r).clamp(0, max) as u16;
            }
        } else {
            dst.copy_from_slice(pred);
        }
    }
    Ok(())
}

/// Turns the coefficient levels in `d.coeffs` into residual samples
/// (clause 8.6.2), followed by the implicit or explicit RDPCM of clause 8.6.5.
fn residual(d: &mut Dec<'_>, n: usize, c_idx: usize, mode: u8, ts: bool, bd: u8) {
    // rotateCoeffs, the DST and implicit RDPCM are for intra blocks only.
    let intra = !d.cu_inter;
    let rotate = d.sps.transform_skip_rotation && n == 4 && intra;
    if d.tq_bypass {
        // Residual samples are the coefficient levels themselves.
        if rotate {
            d.coeffs[..16].reverse();
        }
    } else {
        let qp = if c_idx == 0 {
            d.qp_luma()
        } else {
            d.qp_chroma(c_idx)
        };
        let log2 = n.trailing_zeros() as usize;
        let fac = d.factors(log2, c_idx, ts);
        let ext = d.sps.extended_precision;
        let range = d.log2_range(c_idx);
        scale_range(&mut d.coeffs, n, qp, bd, fac, range);
        let tr_type = u8::from(c_idx == 0 && n == 4 && intra);
        match (ts, ext) {
            (true, false) => transform_skip(&mut d.coeffs, n, bd, rotate),
            (true, true) => transform_skip_ext(&mut d.coeffs, n, bd, rotate),
            (false, false) => inverse_transform(&mut d.coeffs, n, tr_type, bd),
            (false, true) => inverse_transform_ext(&mut d.coeffs, n, tr_type, bd, range),
        }
    }
    // residualDpcm (clause 8.4.4.1) for intra, explicit_rdpcm_flag (8.5.4.2)
    // for inter, both through the modification of clause 8.6.5.
    let dir = if intra {
        let implicit = d.sps.implicit_rdpcm && (ts || d.tq_bypass) && (mode == 10 || mode == 26);
        if implicit {
            1 + u8::from(mode == 26)
        } else {
            0
        }
    } else {
        d.rdpcm
    };
    if dir != 0 {
        let b = &mut d.coeffs[..n * n];
        if dir == 1 {
            for row in b.chunks_exact_mut(n) {
                for i in 1..n {
                    row[i] = row[i].saturating_add(row[i - 1]);
                }
            }
        } else {
            for i in n..n * n {
                b[i] = b[i].saturating_add(b[i - n]);
            }
        }
    }
}
