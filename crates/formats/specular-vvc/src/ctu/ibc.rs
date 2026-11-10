//! Intra block copy (8.6): block vector syntax and prediction, the IBC virtual buffer
//! (IbcVirBuf, 8.7.5.1) and the history-based block vector list.

use crate::cabac::{self, Engine};
use crate::error::{Error, Result};

use super::cu::{Cu, Tu};
use super::{Dec, MODE_IBC};

/// IbcVirBuf per component and the IBC history list.
#[derive(Default)]
pub struct IbcState {
    pub buf: [Vec<u16>; 3],
    /// Width (luma: IbcBufWidthY) and height of each buffer.
    pub w: [usize; 3],
    pub h: [usize; 3],
    pub hmvp: Vec<(i32, i32)>,
}

/// abs_mvd_minus2: limited EG1 (maxPreExtLen 15, truncSuffixLen 17).
fn abs_mvd_minus2(e: &mut Engine<'_>) -> u32 {
    let k = 1;
    let mut pre = 0;
    while pre < 15 && e.bypass() == 1 {
        pre += 1;
    }
    let esc = if pre == 15 { 17 } else { pre + k };
    e.bypass_bits(esc) + (((1u32 << pre) - 1) << k)
}

impl Dec<'_> {
    pub(super) fn ibc_init(&mut self) {
        let ctb = 1usize << self.ctb_log2;
        let bw = 256 * 128 / ctb;
        let s = &mut self.ibc;
        s.w = [bw, bw >> self.sw, bw >> self.sw];
        s.h = [ctb, ctb >> self.sh, ctb >> self.sh];
        let n = if self.chroma { 3 } else { 1 };
        for c in 0..n {
            s.buf[c] = vec![0; s.w[c] * s.h[c]];
        }
    }

    /// Copies a reconstructed block of component `c` (component position and size) into the
    /// IBC virtual buffer.
    pub(super) fn ibc_store(&mut self, c: usize, x: usize, y: usize, w: usize, h: usize) {
        if self.ibc.buf[c].is_empty() {
            return;
        }
        let plane = &self.st.pic.planes[c];
        let (bw, bh) = (self.ibc.w[c], self.ibc.h[c]);
        let buf = &mut self.ibc.buf[c];
        for yy in y..y + h {
            let row = (yy % bh) * bw;
            for xx in x..x + w {
                buf[row + xx % bw] = plane.at(xx, yy);
            }
        }
    }

    fn mvd(&mut self, e: &mut Engine<'_>) -> [i32; 2] {
        let g0 = [
            e.decode(&mut self.ctx.0[cabac::ABS_MVD_GREATER0_FLAG]) == 1,
            e.decode(&mut self.ctx.0[cabac::ABS_MVD_GREATER0_FLAG]) == 1,
        ];
        let mut g1 = [false; 2];
        for k in 0..2 {
            if g0[k] {
                g1[k] = e.decode(&mut self.ctx.0[cabac::ABS_MVD_GREATER1_FLAG]) == 1;
            }
        }
        let mut out = [0i32; 2];
        for k in 0..2 {
            if g0[k] {
                let a = if g1[k] {
                    abs_mvd_minus2(e) as i32 + 2
                } else {
                    1
                };
                out[k] = if e.bypass() == 1 { -a } else { a };
            }
        }
        out
    }

    /// The rest of an IBC coding unit after pred_mode_ibc_flag: block vector, residual,
    /// reconstruction.
    pub(super) fn ibc_cu(&mut self, e: &mut Engine<'_>, cu: &mut Cu, skip: bool) -> Result<()> {
        let sps = self.c.sps;
        let (x0, y0, w, h) = (cu.x0, cu.y0, cu.w, cu.h);
        let max_cand = sps.max_num_ibc_merge_cand as usize;
        let merge = skip || e.decode(&mut self.ctx.0[cabac::GENERAL_MERGE_FLAG]) == 1;
        let mut idx = 0usize;
        let mut mvd = [0i32; 2];
        let mut shift = 4;
        if merge {
            if max_cand > 1 && e.decode(&mut self.ctx.0[cabac::MERGE_IDX]) == 1 {
                idx = 1;
                while idx < max_cand - 1 && e.bypass() == 1 {
                    idx += 1;
                }
            }
        } else {
            mvd = self.mvd(e);
            if max_cand > 1 {
                idx = e.decode(&mut self.ctx.0[cabac::MVP_FLAG]) as usize;
            }
            if sps.amvr
                && (mvd[0] != 0 || mvd[1] != 0)
                && e.decode(&mut self.ctx.0[cabac::AMVR_PRECISION_IDX + 1]) == 1
            {
                shift = 6;
            }
        }
        let gt4 = w * h > 16;
        // Candidate list (8.6.2.2).
        let m = &self.st.maps;
        let nb = |xn: i32, yn: i32| -> Option<(i32, i32)> {
            if !self.available(x0, y0, xn, yn, 0) {
                return None;
            }
            let i = m.idx(xn as u32, yn as u32);
            (m.pred[0][i] == MODE_IBC).then(|| m.bv[i])
        };
        let mut cands: Vec<(i32, i32)> = Vec::with_capacity(6);
        let mut spatial: Vec<(i32, i32)> = Vec::new();
        if gt4 {
            let a1 = nb(x0 as i32 - 1, (y0 + h - 1) as i32);
            let b1 = nb((x0 + w - 1) as i32, y0 as i32 - 1);
            if let Some(a) = a1 {
                cands.push(a);
                spatial.push(a);
            }
            if let Some(b) = b1
                && a1 != Some(b)
            {
                cands.push(b);
                spatial.push(b);
            }
        }
        let n = self.ibc.hmvp.len();
        for k in 1..=n {
            if cands.len() >= max_cand {
                break;
            }
            let c = self.ibc.hmvp[n - k];
            if gt4 && k == 1 && spatial.contains(&c) {
                continue;
            }
            cands.push(c);
        }
        while cands.len() < max_cand {
            cands.push((0, 0));
        }
        let mut bv = *cands
            .get(idx)
            .ok_or(Error::InvalidData("IBC candidate index"))?;
        if !merge {
            let round = |v: i32| -> i32 {
                let off = (1 << (shift - 1)) - 1;
                v.signum() * (((v.abs() + off) >> shift) << shift)
            };
            let wrap = |v: i32| -> i32 {
                let u = v & ((1 << 18) - 1);
                if u >= 1 << 17 { u - (1 << 18) } else { u }
            };
            bv = (
                wrap(round(bv.0) + (mvd[0] << shift)),
                wrap(round(bv.1) + (mvd[1] << shift)),
            );
        }
        if gt4 {
            let l = &mut self.ibc.hmvp;
            if let Some(p) = l.iter().position(|&c| c == bv) {
                l.remove(p);
            } else if l.len() == 5 {
                l.remove(0);
            }
            l.push(bv);
        }
        let mm = &mut self.st.maps;
        for i in mm.cells(x0, y0, w, h) {
            mm.bv[i] = bv;
        }
        // Residual (cu_coded_flag, transform tree).
        let coded = if skip {
            false
        } else if merge {
            true
        } else {
            e.decode(&mut self.ctx.0[cabac::CU_CODED_FLAG]) == 1
        };
        if coded {
            if sps.act && cu.tree == super::TreeType::Single {
                cu.act = e.decode(&mut self.ctx.0[cabac::CU_ACT_ENABLED_FLAG]) == 1;
            }
            self.transform_tree(e, cu, x0, y0, w, h)?;
        }
        if cu.tus.is_empty() {
            cu.tus.push(Tu {
                x0,
                y0,
                w,
                h,
                xc: x0 >> self.sw,
                yc: y0 >> self.sh,
                wc: w >> self.sw,
                hc: h >> self.sh,
                has_luma: true,
                has_chroma: self.chroma,
                ..Tu::default()
            });
        }
        self.cu_qp(cu);
        self.ibc_bv = bv;
        self.reconstruct(cu)
    }

    /// IBC prediction of the `w x h` block of component `c` at component position (x, y).
    pub(super) fn ibc_predict(
        &self,
        c: usize,
        x: u32,
        y: u32,
        w: usize,
        h: usize,
        out: &mut [i32],
    ) {
        let bv = self.ibc_bv;
        let (dx, dy) = if c == 0 {
            (bv.0 >> 4, bv.1 >> 4)
        } else {
            ((bv.0 >> 4) >> self.sw, (bv.1 >> 4) >> self.sh)
        };
        let (bw, bh) = (self.ibc.w[c], self.ibc.h[c]);
        let buf = &self.ibc.buf[c];
        for yy in 0..h {
            let vy = (y as i32 + yy as i32 + dy).rem_euclid(bh as i32) as usize;
            for xx in 0..w {
                let vx = (x as i32 + xx as i32 + dx).rem_euclid(bw as i32) as usize;
                out[yy * w + xx] = i32::from(buf[vy * bw + vx]);
            }
        }
    }
}
