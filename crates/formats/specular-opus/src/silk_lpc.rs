//! SILK normalized LSF decoding and conversion to LPC coefficients (RFC 6716 sections
//! 4.2.7.5.2 to 4.2.7.5.8), bit-exact fixed point.

use crate::range::ilog;
use crate::rfc_tables::{
    LSF_CB1_NBMB, LSF_CB1_WB, LSF_COS_Q12, LSF_MIN_SPACING_NBMB, LSF_MIN_SPACING_WB,
    LSF_ORDER_NBMB, LSF_ORDER_WB, LSF_PRED_NBMB, LSF_PRED_SEL_NBMB, LSF_PRED_SEL_WB, LSF_PRED_WB,
};

/// Reconstructs the normalized LSFs (Q15) from the stage-1 index and stage-2 indices.
pub(crate) fn nlsf_decode(wb: bool, i1: usize, i2: &[i32], out: &mut [i32]) {
    let order = if wb { 16 } else { 10 };
    let qstep = if wb { 9830 } else { 11796 };
    let cb1: &[u8] = if wb {
        &LSF_CB1_WB[i1]
    } else {
        &LSF_CB1_NBMB[i1]
    };
    // Backwards prediction of the residual.
    let mut res = [0i32; 16];
    let mut next = 0i32;
    for k in (0..order).rev() {
        let pred = if k + 1 < order {
            let w = if wb {
                LSF_PRED_WB[usize::from(LSF_PRED_SEL_WB[i1][k])][k]
            } else {
                LSF_PRED_NBMB[usize::from(LSF_PRED_SEL_NBMB[i1][k])][k]
            };
            (next * i32::from(w)) >> 8
        } else {
            0
        };
        let q = i2[k];
        let v = pred + ((((q << 10) - q.signum() * 102) * qstep) >> 16);
        res[k] = v;
        next = v;
    }
    // Inverse harmonic mean weighting.
    for k in 0..order {
        let prev = if k == 0 { 0 } else { i32::from(cb1[k - 1]) };
        let nxt = if k + 1 == order {
            256
        } else {
            i32::from(cb1[k + 1])
        };
        let cur = i32::from(cb1[k]);
        let w2 = (1024 / (cur - prev) + 1024 / (nxt - cur)) << 16;
        let i = ilog(w2 as u32);
        let f = (w2 >> (i - 8)) & 127;
        let y = (if i & 1 != 0 { 32768 } else { 46214 }) >> ((32 - i) >> 1);
        let w = y + ((213 * f * y) >> 16);
        out[k] = ((cur << 7) + (res[k] << 14) / w).clamp(0, 32767);
    }
    stabilize(
        out,
        order,
        if wb {
            &LSF_MIN_SPACING_WB
        } else {
            &LSF_MIN_SPACING_NBMB
        },
    );
}

fn stabilize(n: &mut [i32], order: usize, dmin: &[i32]) {
    for _ in 0..20 {
        let mut min_diff = n[0] - dmin[0];
        let mut idx = 0;
        for i in 1..order {
            let d = n[i] - (n[i - 1] + dmin[i]);
            if d < min_diff {
                min_diff = d;
                idx = i;
            }
        }
        let d = 32768 - (n[order - 1] + dmin[order]);
        if d < min_diff {
            min_diff = d;
            idx = order;
        }
        if min_diff >= 0 {
            return;
        }
        if idx == 0 {
            n[0] = dmin[0];
        } else if idx == order {
            n[order - 1] = 32768 - dmin[order];
        } else {
            let mut min_center = dmin[idx] >> 1;
            for &d in &dmin[..idx] {
                min_center += d;
            }
            let mut max_center = 32768 - (dmin[idx] >> 1);
            for &d in &dmin[idx + 1..=order] {
                max_center -= d;
            }
            let center =
                ((n[idx - 1] + n[idx] + 1) >> 1).clamp(min_center, max_center.max(min_center));
            n[idx - 1] = center - (dmin[idx] >> 1);
            n[idx] = n[idx - 1] + dmin[idx];
        }
    }
    // Fallback.
    n[..order].sort_unstable();
    n[0] = n[0].max(dmin[0]);
    for i in 1..order {
        n[i] = n[i].max((n[i - 1] + dmin[i]).min(32767));
    }
    n[order - 1] = n[order - 1].min(32768 - dmin[order]);
    for i in (0..order - 1).rev() {
        n[i] = n[i].min(n[i + 1] - dmin[i + 1]);
    }
}

fn bwexpand32(a: &mut [i64], sc0: i64) {
    let mut sc = sc0;
    for v in a.iter_mut() {
        *v = (*v * sc) >> 16;
        sc = (sc0 * sc + 32768) >> 16;
    }
}

/// Inverse prediction gain check: true if the Q12 filter is stable enough.
fn stable(a_q12: &[i32]) -> bool {
    let d = a_q12.len();
    let dc: i32 = a_q12.iter().sum();
    if dc > 4096 {
        return false;
    }
    let mut a: Vec<i64> = a_q12.iter().map(|&v| i64::from(v) << 12).collect();
    let mut inv_gain: i64 = 1 << 30;
    for k in (0..d).rev() {
        if a[k].abs() > 16_773_022 {
            return false;
        }
        let rc = -(a[k] << 7);
        let div = (1i64 << 30) - ((rc * rc) >> 32);
        inv_gain = ((inv_gain * div) >> 32) << 2;
        if inv_gain < 107_374 {
            return false;
        }
        if k > 0 {
            let b1 = ilog(div as u32);
            let b2 = b1 - 16;
            let inv_qb2 = ((1i64 << 29) - 1) / (div >> (b2 + 1));
            let err = (1i64 << 29) - (((div << (15 - b2)) * inv_qb2) >> 16);
            let gain = (inv_qb2 << 16) + ((err * inv_qb2) >> 13);
            let mut next = vec![0i64; k];
            for (nn, nv) in next.iter_mut().enumerate() {
                let num = (a[nn] - ((a[k - nn - 1] * rc + (1 << 30)) >> 31))
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX));
                let v = (num * gain + (1i64 << (b1 - 1))) >> b1;
                if v > i64::from(i32::MAX) || v < i64::from(i32::MIN) {
                    return false;
                }
                *nv = v;
            }
            a[..k].copy_from_slice(&next);
        }
    }
    true
}

/// Converts normalized LSFs to stable Q12 LPC coefficients (silk_NLSF2A per the RFC).
pub(crate) fn nlsf2a(nlsf: &[i32], out: &mut [i32]) {
    let d = nlsf.len();
    let ord: &[u8] = if d == 16 {
        &LSF_ORDER_WB
    } else {
        &LSF_ORDER_NBMB
    };
    let mut c = [0i64; 16];
    for k in 0..d {
        let i = (nlsf[k] >> 8) as usize;
        let f = i64::from(nlsf[k] & 255);
        let c0 = i64::from(LSF_COS_Q12[i]);
        let c1 = i64::from(LSF_COS_Q12[(i + 1).min(128)]);
        c[usize::from(ord[k])] = (c0 * 256 + (c1 - c0) * f + 4) >> 3;
    }
    let d2 = d / 2;
    let poly = |off: usize| -> Vec<i64> {
        // p[j] for j in 0..=d2.
        let mut p = vec![0i64; d2 + 2];
        p[0] = 1 << 16;
        p[1] = -c[off];
        for k in 1..d2 {
            let ck = c[2 * k + off];
            let mut np = p.clone();
            for j in 0..=k + 1 {
                let pm2 = if j >= 2 { p[j - 2] } else { 0 };
                // Symmetry: p[k][k+1] uses p[k-1][k+1] = p[k-1][k-1].
                let pj = if j <= k { p[j] } else { p[2 * k - j] };
                let pj1 = if j >= 1 {
                    if j - 1 <= k {
                        p[j - 1]
                    } else {
                        p[2 * k - (j - 1)]
                    }
                } else {
                    0
                };
                np[j] = pj + pm2 - ((ck * pj1 + 32768) >> 16);
            }
            p = np;
        }
        p
    };
    let p = poly(0);
    let q = poly(1);
    let mut a32 = [0i64; 16];
    for k in 0..d2 {
        let pt = p[k + 1] + p[k];
        let qt = q[k + 1] - q[k];
        a32[k] = -qt - pt;
        a32[d - k - 1] = qt - pt;
    }
    let a32 = &mut a32[..d];
    // Range limiting.
    let mut saturate = true;
    for _ in 0..10 {
        let mut maxabs = 0i64;
        let mut idx = 0usize;
        for (k, v) in a32.iter().enumerate() {
            if v.abs() > maxabs {
                maxabs = v.abs();
                idx = k;
            }
        }
        let maxabs = ((maxabs + 16) >> 5).min(163_838);
        if maxabs > 32767 {
            let sc = 65470 - ((maxabs - 32767) << 14) / ((maxabs * (idx as i64 + 1)) >> 2);
            bwexpand32(a32, sc);
        } else {
            saturate = false;
            break;
        }
    }
    if saturate {
        for v in a32.iter_mut() {
            *v = (((*v + 16) >> 5).clamp(-32768, 32767)) << 5;
        }
    }
    let mut a_q12 = [0i32; 16];
    for i in 0..16 {
        for k in 0..d {
            a_q12[k] = ((a32[k] + 16) >> 5) as i32;
        }
        if stable(&a_q12[..d]) {
            break;
        }
        bwexpand32(a32, 65536 - (2i64 << i));
    }
    for k in 0..d {
        a_q12[k] = ((a32[k] + 16) >> 5) as i32;
    }
    out[..d].copy_from_slice(&a_q12[..d]);
}
