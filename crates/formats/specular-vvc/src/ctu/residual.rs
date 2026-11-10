//! `residual_coding()` and `residual_ts_coding()` (7.3.11.11) with their context
//! derivations (9.3.4.2.4 to 9.3.4.2.10) and binarizations (9.3.3.2, 9.3.3.11, 9.3.3.12).

use std::sync::OnceLock;

use crate::cabac::{self, Engine};
use crate::error::{Error, Result};

use super::Dec;
use super::cu::{Cu, Tu};

/// DiagScanOrder[ log2w ][ log2h ] for sizes up to 32x32: (x, y) per scan position.
fn diag(log2w: u32, log2h: u32) -> &'static [(u8, u8)] {
    static T: OnceLock<Vec<Vec<(u8, u8)>>> = OnceLock::new();
    let t = T.get_or_init(|| {
        let mut v = Vec::with_capacity(36);
        for lw in 0..6 {
            for lh in 0..6 {
                v.push(crate::ps::aps::diag_scan(1 << lw, 1 << lh));
            }
        }
        v
    });
    &t[(log2w * 6 + log2h) as usize]
}

/// cRiceParam from locSumAbs (Table 128).
const RICE: [u32; 32] = [
    0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3,
];

const QSTATE: [[u8; 2]; 4] = [[0, 2], [2, 0], [1, 3], [3, 1]];

/// abs_remainder / dec_abs_level binarization: TR prefix (cMax 6 << rice) and a limited
/// EG(k + 1) suffix (maxPreExtLen 11, truncSuffixLen 15).
fn rice_value(e: &mut Engine<'_>, rice: u32) -> u32 {
    let mut prefix = 0;
    while prefix < 6 && e.bypass() == 1 {
        prefix += 1;
    }
    if prefix < 6 {
        return (prefix << rice) + e.bypass_bits(rice);
    }
    let k = rice + 1;
    let mut pre = 0;
    while pre < 11 && e.bypass() == 1 {
        pre += 1;
    }
    let esc = if pre == 11 { 15 } else { pre + k };
    let sym = e.bypass_bits(esc) + (((1u32 << pre) - 1) << k);
    (6 << rice) + sym
}

impl Dec<'_> {
    /// Last significant coefficient prefix (TR, context coded).
    fn last_prefix(
        &mut self,
        e: &mut Engine<'_>,
        base: usize,
        cmax: u32,
        log2: u32,
        ci: usize,
    ) -> u32 {
        let (off, shift) = if ci == 0 {
            const OFF: [usize; 6] = [0, 0, 3, 6, 10, 15];
            (OFF[(log2 - 1) as usize], (log2 + 1) >> 2)
        } else {
            (20, ((1u32 << log2) >> 3).min(2))
        };
        let mut v = 0;
        while v < cmax && e.decode(&mut self.ctx.0[base + off + (v >> shift) as usize]) == 1 {
            v += 1;
        }
        v
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn residual_coding(
        &mut self,
        e: &mut Engine<'_>,
        cu: &mut Cu,
        tu: &Tu,
        w: u32,
        h: u32,
        ci: usize,
        out: &mut [i32],
    ) -> Result<()> {
        let _ = tu;
        let sh = self.c.sh;
        let log2w = w.trailing_zeros();
        let log2h = h.trailing_zeros();
        let zw = log2w.min(5);
        let zh = log2h.min(5);
        let ts = tu_ts(tu, ci);
        let lxp = if log2w > 0 {
            self.last_prefix(e, cabac::LAST_SIG_COEFF_X_PREFIX, (zw << 1) - 1, log2w, ci)
        } else {
            0
        };
        let lyp = if log2h > 0 {
            self.last_prefix(e, cabac::LAST_SIG_COEFF_Y_PREFIX, (zh << 1) - 1, log2h, ci)
        } else {
            0
        };
        let last_x = if lxp > 3 {
            let n = (lxp >> 1) - 1;
            let s = e.bypass_bits(n);
            (1 << n) * (2 + (lxp & 1)) + s
        } else {
            lxp
        };
        let last_y = if lyp > 3 {
            let n = (lyp >> 1) - 1;
            let s = e.bypass_bits(n);
            (1 << n) * (2 + (lyp & 1)) + s
        } else {
            lyp
        };
        if last_x >= (1 << zw) || last_y >= (1 << zh) {
            return Err(Error::InvalidData(
                "last significant coefficient outside the block",
            ));
        }
        let mut rem_bins: i32 = (((1u32 << (zw + zh)) * 7) >> 2) as i32;
        let (mut sbw, mut sbh) = if zw.min(zh) < 2 { (1, 1) } else { (2, 2) };
        if zw + zh > 3 {
            if zw < 2 {
                sbw = zw;
                sbh = 4 - sbw;
            } else if zh < 2 {
                sbh = zh;
                sbw = 4 - sbh;
            }
        }
        let num_sb_coeff = 1usize << (sbw + sbh);
        let sb_cols = 1u32 << (zw - sbw);
        let sb_rows = 1u32 << (zh - sbh);
        let sb_scan = diag(zw - sbw, zh - sbh);
        let co_scan = diag(sbw, sbh);
        // lastSubBlock / lastScanPos.
        let (lsx, lsy) = (last_x >> sbw, last_y >> sbh);
        let last_sb = sb_scan
            .iter()
            .position(|&(x, y)| u32::from(x) == lsx && u32::from(y) == lsy)
            .ok_or(Error::InvalidData("last subblock not found"))?;
        let (px, py) = (last_x & ((1 << sbw) - 1), last_y & ((1 << sbh) - 1));
        let last_pos = co_scan
            .iter()
            .position(|&(x, y)| u32::from(x) == px && u32::from(y) == py)
            .ok_or(Error::InvalidData("last position not found"))?;
        if last_sb == 0 && zw >= 2 && zh >= 2 && !ts && last_pos > 0 {
            cu.lfnst_dc_only = false;
        }
        if (last_sb > 0 && zw >= 2 && zh >= 2) || (last_pos > 7 && (zw == 2 || zw == 3) && zw == zh)
        {
            cu.lfnst_zero_out = false;
        }
        if (last_sb > 0 || last_pos > 0) && ci == 0 {
            cu.mts_dc_only = false;
        }
        let tw = 1usize << zw;
        let th = 1usize << zh;
        let full_w = w as usize;
        let full_h = h as usize;
        // Working arrays over the full block (Rice derivation reads beyond the zero-out).
        let n = full_w * full_h;
        let sc = &mut self.scratch;
        sc.sig.clear();
        sc.sig.resize(n, 0);
        sc.pass1.clear();
        sc.pass1.resize(n, 0);
        sc.abs.clear();
        sc.abs.resize(n, 0);
        let mut sig = std::mem::take(&mut sc.sig);
        let mut pass1 = std::mem::take(&mut sc.pass1);
        let mut abs = std::mem::take(&mut sc.abs);
        let mut sb_coded = [false; 64];
        let dq = sh.dep_quant;
        let sdh = sh.sign_data_hiding;
        let mut qstate = 0usize;
        let ts_ctx = ts && !sh.ts_residual_coding_disabled;
        for i in (0..=last_sb).rev() {
            let start_q = qstate;
            let (xs, ys) = (u32::from(sb_scan[i].0), u32::from(sb_scan[i].1));
            let mut infer_dc = false;
            let coded = if i < last_sb && i > 0 {
                // sb_coded_flag context (9.3.4.2.6, non-TS branch).
                let (mut lsw, mut lsh) = if zw.min(zh) < 2 { (1, 1) } else { (2, 2) };
                if zw < 2 && ci == 0 {
                    lsw = zw;
                    lsh = 4 - lsw;
                } else if zh < 2 && ci == 0 {
                    lsh = zh;
                    lsw = 4 - lsh;
                }
                let cols = (1i32 << (zw as i32 - lsw as i32).max(0)) - 1;
                let rows = (1i32 << (zh as i32 - lsh as i32).max(0)) - 1;
                let mut csbf = 0;
                if (xs as i32) < cols && xs + 1 < sb_cols {
                    csbf += usize::from(sb_coded[(ys * sb_cols + xs + 1) as usize]);
                }
                if (ys as i32) < rows && ys + 1 < sb_rows {
                    csbf += usize::from(sb_coded[((ys + 1) * sb_cols + xs) as usize]);
                }
                let inc = csbf.min(1) + if ci > 0 { 2 } else { 0 };
                let f = e.decode(&mut self.ctx.0[cabac::SB_CODED_FLAG + inc]) == 1;
                infer_dc = true;
                f
            } else {
                true
            };
            sb_coded[(ys * sb_cols + xs) as usize] = coded;
            if coded && (xs > 3 || ys > 3) && ci == 0 {
                cu.mts_zero_out = false;
            }
            let mut first_sig = num_sb_coeff as i32;
            let mut last_sig = -1i32;
            let first_mode0 = if i == last_sb {
                last_pos as i32
            } else {
                num_sb_coeff as i32 - 1
            };
            let mut first_mode1 = first_mode0;
            let mut gt1 = [false; 16];
            let mut nidx = first_mode0;
            while nidx >= 0 && rem_bins >= 4 {
                let nn = nidx as usize;
                let xc = ((xs << sbw) + u32::from(co_scan[nn].0)) as usize;
                let yc = ((ys << sbh) + u32::from(co_scan[nn].1)) as usize;
                let p = yc * full_w + xc;
                let is_last = xc as u32 == last_x && yc as u32 == last_y;
                let mut s = false;
                if coded && (nidx > 0 || !infer_dc) && !is_last {
                    let (num_sig, sum_abs) = loc_regular(&sig, &pass1, xc, yc, tw, th, full_w);
                    let d = xc + yc;
                    let inc = if ts_ctx {
                        60 + num_sig as usize
                    } else if ci == 0 {
                        12 * qstate.saturating_sub(1)
                            + (((sum_abs + 1) >> 1).min(3)) as usize
                            + if d < 2 {
                                8
                            } else if d < 5 {
                                4
                            } else {
                                0
                            }
                    } else {
                        36 + 8 * qstate.saturating_sub(1)
                            + (((sum_abs + 1) >> 1).min(3)) as usize
                            + if d < 2 { 4 } else { 0 }
                    };
                    s = e.decode(&mut self.ctx.0[cabac::SIG_COEFF_FLAG + inc]) == 1;
                    rem_bins -= 1;
                    if s {
                        infer_dc = false;
                    }
                } else if is_last || (coded && nidx == 0 && infer_dc) {
                    s = true;
                }
                let mut a1 = 0i32;
                if s {
                    sig[p] = 1;
                    let (num_sig, sum_abs) = loc_regular(&sig, &pass1, xc, yc, tw, th, full_w);
                    let base = if is_last {
                        if ci == 0 { 0 } else { 21 }
                    } else {
                        let off = (sum_abs - num_sig).min(4) as usize;
                        let d = xc + yc;
                        if ci == 0 {
                            1 + off
                                + if d == 0 {
                                    15
                                } else if d < 3 {
                                    10
                                } else if d < 10 {
                                    5
                                } else {
                                    0
                                }
                        } else {
                            22 + off + if d == 0 { 5 } else { 0 }
                        }
                    };
                    let g0 = e.decode(&mut self.ctx.0[cabac::ABS_LEVEL_GTX_FLAG + base]) == 1;
                    rem_bins -= 1;
                    a1 = 1;
                    if g0 {
                        let par = e.decode(&mut self.ctx.0[cabac::PAR_LEVEL_FLAG + base]);
                        rem_bins -= 1;
                        let g1 =
                            e.decode(&mut self.ctx.0[cabac::ABS_LEVEL_GTX_FLAG + base + 32]) == 1;
                        rem_bins -= 1;
                        a1 = 1 + par as i32 + 1 + 2 * i32::from(g1);
                        gt1[nn] = g1;
                    }
                    if last_sig == -1 {
                        last_sig = nidx;
                    }
                    first_sig = nidx;
                }
                pass1[p] = a1;
                if dq {
                    qstate = usize::from(QSTATE[qstate][(a1 & 1) as usize]);
                }
                first_mode1 = nidx - 1;
                nidx -= 1;
            }
            // abs_remainder for gt1 positions.
            let mut nidx = first_mode0;
            while nidx > first_mode1 {
                let nn = nidx as usize;
                let xc = ((xs << sbw) + u32::from(co_scan[nn].0)) as usize;
                let yc = ((ys << sbh) + u32::from(co_scan[nn].1)) as usize;
                let p = yc * full_w + xc;
                let mut rem = 0;
                if gt1[nn] {
                    let rice = rice_param(&abs, xc, yc, full_w, full_h, 4);
                    rem = rice_value(e, rice) as i32;
                }
                abs[p] = pass1[p] + 2 * rem;
                nidx -= 1;
            }
            // dec_abs_level for the bypass-coded positions.
            let mut nidx = first_mode1;
            while nidx >= 0 {
                let nn = nidx as usize;
                let xc = ((xs << sbw) + u32::from(co_scan[nn].0)) as usize;
                let yc = ((ys << sbh) + u32::from(co_scan[nn].1)) as usize;
                let p = yc * full_w + xc;
                if coded {
                    let rice = rice_param(&abs, xc, yc, full_w, full_h, 0);
                    let zero_pos = (if qstate < 2 { 1u32 } else { 2 }) << rice;
                    let v = rice_value(e, rice);
                    abs[p] = if v == zero_pos {
                        0
                    } else if v < zero_pos {
                        v as i32 + 1
                    } else {
                        v as i32
                    };
                }
                if abs[p] > 0 {
                    if last_sig == -1 {
                        last_sig = nidx;
                    }
                    first_sig = nidx;
                }
                if dq {
                    qstate = usize::from(QSTATE[qstate][(abs[p] & 1) as usize]);
                }
                nidx -= 1;
            }
            let hidden = sdh && last_sig - first_sig > 3;
            let mut signs = [false; 16];
            for nn in (0..num_sb_coeff).rev() {
                let xc = ((xs << sbw) + u32::from(co_scan[nn].0)) as usize;
                let yc = ((ys << sbh) + u32::from(co_scan[nn].1)) as usize;
                let p = yc * full_w + xc;
                if abs[p] > 0 && (!hidden || nn as i32 != first_sig) {
                    signs[nn] = e.bypass() == 1;
                }
            }
            if dq {
                let mut q = start_q;
                for nn in (0..num_sb_coeff).rev() {
                    let xc = ((xs << sbw) + u32::from(co_scan[nn].0)) as usize;
                    let yc = ((ys << sbh) + u32::from(co_scan[nn].1)) as usize;
                    let p = yc * full_w + xc;
                    let a = abs[p];
                    if a > 0 {
                        let v = 2 * a - i32::from(q > 1);
                        out[p] = if signs[nn] { -v } else { v };
                    }
                    q = usize::from(QSTATE[q][(a & 1) as usize]);
                }
            } else {
                let mut sum = 0;
                for nn in (0..num_sb_coeff).rev() {
                    let xc = ((xs << sbw) + u32::from(co_scan[nn].0)) as usize;
                    let yc = ((ys << sbh) + u32::from(co_scan[nn].1)) as usize;
                    let p = yc * full_w + xc;
                    let a = abs[p];
                    if a > 0 {
                        let mut v = if signs[nn] { -a } else { a };
                        if hidden {
                            sum += a;
                            if nn as i32 == first_sig && sum % 2 == 1 {
                                v = -v;
                            }
                        }
                        out[p] = v;
                    }
                }
            }
        }
        let sc = &mut self.scratch;
        sc.sig = sig;
        sc.pass1 = pass1;
        sc.abs = abs;
        Ok(())
    }

    pub(super) fn residual_ts_coding(
        &mut self,
        e: &mut Engine<'_>,
        cu: &Cu,
        w: u32,
        h: u32,
        ci: usize,
        out: &mut [i32],
    ) -> Result<()> {
        let log2w = w.trailing_zeros();
        let log2h = h.trailing_zeros();
        let bdpcm = cu.bdpcm[usize::from(ci > 0)];
        let (mut sbw, mut sbh) = if log2w.min(log2h) < 2 { (1, 1) } else { (2, 2) };
        if log2w + log2h > 3 {
            if log2w < 2 {
                sbw = log2w;
                sbh = 4 - sbw;
            } else if log2h < 2 {
                sbh = log2h;
                sbw = 4 - sbh;
            }
        }
        let num_sb_coeff = 1usize << (sbw + sbh);
        let sb_cols = 1u32 << (log2w - sbw);
        let last_sb = (1usize << (log2w + log2h - sbw - sbh)) - 1;
        let sb_scan = diag(log2w - sbw, log2h - sbh);
        let co_scan = diag(sbw, sbh);
        let fw = w as usize;
        let n = fw * h as usize;
        let mut infer_sbcbf = true;
        let mut rem: i32 = (((1u32 << (log2w + log2h)) * 7) >> 2) as i32;
        let sc = &mut self.scratch;
        sc.sig.clear();
        sc.sig.resize(n, 0);
        sc.abs.clear();
        sc.abs.resize(n, 0);
        sc.sign.clear();
        sc.sign.resize(n, 0);
        let mut sig = std::mem::take(&mut sc.sig);
        let mut abs = std::mem::take(&mut sc.abs);
        let mut sgn = std::mem::take(&mut sc.sign);
        let mut sb_coded = vec![false; last_sb + 1];
        let rice = self.c.sh.ts_rice_idx;
        for i in 0..=last_sb {
            let (xs, ys) = (u32::from(sb_scan[i].0), u32::from(sb_scan[i].1));
            let coded = if i != last_sb || !infer_sbcbf {
                let mut csbf = 0;
                if xs > 0 {
                    csbf += usize::from(sb_coded[(ys * sb_cols + xs - 1) as usize]);
                }
                if ys > 0 {
                    csbf += usize::from(sb_coded[((ys - 1) * sb_cols + xs) as usize]);
                }
                e.decode(&mut self.ctx.0[cabac::SB_CODED_FLAG + 4 + csbf]) == 1
            } else {
                true
            };
            sb_coded[(ys * sb_cols + xs) as usize] = coded;
            if coded && i < last_sb {
                infer_sbcbf = false;
            }
            let mut infer_sig = true;
            let mut last_p1 = -1i32;
            let mut pass1 = [0i32; 16];
            let mut gtx0 = [false; 16];
            let mut nidx = 0usize;
            while nidx < num_sb_coeff && rem >= 4 {
                let xc = ((xs << sbw) + u32::from(co_scan[nidx].0)) as usize;
                let yc = ((ys << sbh) + u32::from(co_scan[nidx].1)) as usize;
                let p = yc * fw + xc;
                last_p1 = nidx as i32;
                let mut s = false;
                if coded && (nidx != num_sb_coeff - 1 || !infer_sig) {
                    let mut num = 0;
                    if xc > 0 {
                        num += sig[p - 1];
                    }
                    if yc > 0 {
                        num += sig[p - fw];
                    }
                    s = e.decode(&mut self.ctx.0[cabac::SIG_COEFF_FLAG + 60 + num as usize]) == 1;
                    rem -= 1;
                    if s {
                        infer_sig = false;
                    }
                } else if coded && nidx == num_sb_coeff - 1 && infer_sig {
                    s = true;
                }
                let mut a = 0;
                if s {
                    sig[p] = 1;
                    let left = if xc > 0 { sgn[p - 1] } else { 0 };
                    let above = if yc > 0 { sgn[p - fw] } else { 0 };
                    let inc = if (left == 0 && above == 0) || left == -above {
                        0
                    } else if left >= 0 && above >= 0 {
                        1
                    } else {
                        2
                    } + if bdpcm { 3 } else { 0 };
                    let neg = e.decode(&mut self.ctx.0[cabac::COEFF_SIGN_FLAG + inc]) == 1;
                    rem -= 1;
                    sgn[p] = if neg { -1 } else { 1 };
                    let inc = if bdpcm {
                        67
                    } else {
                        let mut v = 64;
                        if xc > 0 {
                            v += sig[p - 1] as usize;
                        }
                        if yc > 0 {
                            v += sig[p - fw] as usize;
                        }
                        v
                    };
                    let g0 = e.decode(&mut self.ctx.0[cabac::ABS_LEVEL_GTX_FLAG + inc]) == 1;
                    rem -= 1;
                    a = 1;
                    if g0 {
                        let par = e.decode(&mut self.ctx.0[cabac::PAR_LEVEL_FLAG + 32]);
                        rem -= 1;
                        a = 2 + par as i32;
                        gtx0[nidx] = true;
                    }
                }
                pass1[nidx] = a;
                nidx += 1;
            }
            // Greater-than-X pass.
            let mut last_p2 = -1i32;
            let mut pass2 = pass1;
            let mut gtx_last = gtx0;
            let mut nidx = 0usize;
            while nidx < num_sb_coeff && rem >= 4 {
                for j in 1..5 {
                    if gtx_last[nidx] {
                        let g = e.decode(&mut self.ctx.0[cabac::ABS_LEVEL_GTX_FLAG + 67 + j]) == 1;
                        rem -= 1;
                        gtx_last[nidx] = g;
                        if g {
                            pass2[nidx] += 2;
                        }
                    }
                }
                last_p2 = nidx as i32;
                nidx += 1;
            }
            // Remainder pass.
            for nidx in 0..num_sb_coeff {
                let xc = ((xs << sbw) + u32::from(co_scan[nidx].0)) as usize;
                let yc = ((ys << sbh) + u32::from(co_scan[nidx].1)) as usize;
                let p = yc * fw + xc;
                let ni = nidx as i32;
                let coded_rem = (ni <= last_p2 && pass2[nidx] >= 10)
                    || (ni > last_p2 && ni <= last_p1 && pass1[nidx] >= 2)
                    || (ni > last_p1 && coded);
                let r = if coded_rem {
                    rice_value(e, rice) as i32
                } else {
                    0
                };
                let mut a = if ni <= last_p2 {
                    pass2[nidx] + 2 * r
                } else if ni <= last_p1 {
                    pass1[nidx] + 2 * r
                } else {
                    if r != 0 && e.bypass() == 1 {
                        sgn[p] = -1;
                    } else if r != 0 {
                        sgn[p] = 1;
                    }
                    r
                };
                if !bdpcm && ni <= last_p1 {
                    let l = if xc > 0 { abs[p - 1] } else { 0 };
                    let t = if yc > 0 { abs[p - fw] } else { 0 };
                    let pred = l.max(t);
                    if a == 1 && pred > 0 {
                        a = pred;
                    } else if a > 0 && a <= pred {
                        a -= 1;
                    }
                }
                abs[p] = a;
                out[p] = if sgn[p] < 0 { -a } else { a };
            }
        }
        let sc = &mut self.scratch;
        sc.sig = sig;
        sc.abs = abs;
        sc.sign = sgn;
        Ok(())
    }
}

fn tu_ts(tu: &Tu, ci: usize) -> bool {
    tu.ts[ci]
}

/// locNumSig and locSumAbsPass1 (9.3.4.2.7), non transform-skip branch.
#[inline]
fn loc_regular(
    sig: &[i32],
    pass1: &[i32],
    xc: usize,
    yc: usize,
    tw: usize,
    th: usize,
    stride: usize,
) -> (i32, i32) {
    let mut n = 0;
    let mut s = 0;
    let p = yc * stride + xc;
    if xc + 1 < tw {
        n += sig[p + 1];
        s += pass1[p + 1];
        if xc + 2 < tw {
            n += sig[p + 2];
            s += pass1[p + 2];
        }
        if yc + 1 < th {
            n += sig[p + stride + 1];
            s += pass1[p + stride + 1];
        }
    }
    if yc + 1 < th {
        n += sig[p + stride];
        s += pass1[p + stride];
        if yc + 2 < th {
            n += sig[p + 2 * stride];
            s += pass1[p + 2 * stride];
        }
    }
    (n, s)
}

/// Rice parameter (9.3.3.2) from the full-size AbsLevel array (HistValue 0).
#[inline]
fn rice_param(abs: &[i32], xc: usize, yc: usize, w: usize, h: usize, base: i32) -> u32 {
    let p = yc * w + xc;
    let mut s = 0;
    if xc + 1 < w {
        s += abs[p + 1];
        if xc + 2 < w {
            s += abs[p + 2];
        }
        if yc + 1 < h {
            s += abs[p + w + 1];
        }
    }
    if yc + 1 < h {
        s += abs[p + w];
        if yc + 2 < h {
            s += abs[p + 2 * w];
        }
    }
    let s = (s - base * 5).clamp(0, 31);
    RICE[s as usize]
}
