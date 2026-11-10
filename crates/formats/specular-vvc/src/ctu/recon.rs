//! Reconstruction of intra coding units (8.4.1, 8.4.5.1): prediction, scaling (8.7.3),
//! transforms (8.7.4), joint chroma residuals (8.7.2), chroma residual scaling (8.7.5.3).

use crate::error::Result;
use crate::filter::{CuRec, Pred, Tree, TuRec};
use crate::intra::{self, cclm, mip};
use crate::transform;

use super::cu::{Cu, ISP_HOR, ISP_NONE, ISP_VER, Tu};
use super::{Dec, MODE_IBC, MODE_INTRA, MODE_PLT, TreeType};

/// Reused buffers.
#[derive(Default)]
pub struct Scratch {
    pub infer_cbf_luma: bool,
    pub prev_cbf_y: bool,
    pub sig: Vec<i32>,
    pub pass1: Vec<i32>,
    pub abs: Vec<i32>,
    pub sign: Vec<i32>,
    top: Vec<i32>,
    left: Vec<i32>,
    avail: Vec<bool>,
    pred: Vec<i32>,
    res: Vec<i32>,
    tmp: Vec<i32>,
    /// Residuals of an ACT coding unit per component, CU-sized.
    act: [Vec<i32>; 3],
}

const LEVEL_SCALE: [[i64; 6]; 2] = [[40, 45, 51, 57, 64, 72], [57, 64, 72, 80, 90, 102]];

impl Dec<'_> {
    pub(super) fn reconstruct(&mut self, cu: &mut Cu) -> Result<()> {
        let tree = match cu.tree {
            TreeType::Single => Tree::Single,
            TreeType::DualLuma => Tree::Luma,
            TreeType::DualChroma => Tree::Chroma,
        };
        let pred = match cu.pred {
            MODE_IBC => Pred::Ibc,
            MODE_PLT => Pred::Palette,
            MODE_INTRA => Pred::Intra,
            _ => Pred::Inter,
        };
        self.st.meta.cu(&CuRec {
            x: cu.x0,
            y: cu.y0,
            w: cu.w,
            h: cu.h,
            tree,
            pred,
            qp_y: cu.qp_y,
            bdpcm: cu.bdpcm,
            isp: (cu.isp != ISP_NONE).then_some((cu.isp == ISP_HOR, cu.isp_parts)),
            bv: if cu.pred == MODE_IBC {
                self.ibc_bv
            } else {
                (0, 0)
            },
            act: cu.act,
        });
        let tus = std::mem::take(&mut cu.tus);
        if cu.plt {
            // Palette samples are already in the picture; one TU without residual.
            let tree = cu.tree;
            self.st.meta.tu(&TuRec {
                x: cu.x0,
                y: cu.y0,
                w: cu.w,
                h: cu.h,
                tree: match tree {
                    TreeType::Single => Tree::Single,
                    TreeType::DualLuma => Tree::Luma,
                    TreeType::DualChroma => Tree::Chroma,
                },
                cbf: [false; 3],
                ts: [false; 3],
            });
            return Ok(());
        }
        if cu.act {
            self.act_residuals(cu, &tus);
        }
        if cu.tree != TreeType::DualChroma {
            self.recon_luma(cu, &tus);
        }
        if cu.tree != TreeType::DualLuma && self.chroma {
            for ci in 1..3 {
                for tu in tus.iter().filter(|t| t.has_chroma) {
                    self.recon_chroma_tb(cu, tu, ci);
                }
            }
        }
        for tu in &tus {
            self.st.meta.tu(&TuRec {
                x: tu.x0,
                y: tu.y0,
                w: tu.w,
                h: tu.h,
                tree: if !tu.has_chroma {
                    Tree::Luma
                } else if !tu.has_luma {
                    Tree::Chroma
                } else {
                    Tree::Single
                },
                cbf: [
                    tu.cbf[0],
                    tu.cbf[1] || (tu.joint && tu.has_chroma),
                    tu.cbf[2] || (tu.joint && tu.has_chroma),
                ],
                ts: tu.ts,
            });
            if tu.has_chroma {
                let q = if tu.res_mode == 2 {
                    [cu.qp[3], cu.qp[3]]
                } else {
                    [cu.qp[1], cu.qp[2]]
                };
                self.st.meta.tu_chroma_qp(q);
            }
        }
        cu.tus = tus;
        Ok(())
    }

    fn recon_luma(&mut self, cu: &Cu, tus: &[Tu]) {
        let isp = cu.isp;
        let mut pb_pred: Vec<i32> = Vec::new();
        let mut pb_w = 0usize;
        for (k, tu) in tus.iter().enumerate() {
            let (w, h) = (tu.w as usize, tu.h as usize);
            let pred_w;
            if isp == ISP_VER && w < 4 {
                // Prediction per group of 4 / w partitions (nPbW = 4).
                let factor = 4 / w;
                if k % factor == 0 {
                    pb_w = 4;
                    pb_pred = vec![0; 4 * h];
                    self.predict(cu, 0, tu.x0, tu.y0, 4, h, &mut pb_pred);
                }
                pred_w = pb_w;
                let off = (k % factor) * w;
                let mut p = vec![0i32; w * h];
                for y in 0..h {
                    p[y * w..y * w + w]
                        .copy_from_slice(&pb_pred[y * pred_w + off..y * pred_w + off + w]);
                }
                self.finish_tb(cu, tu, 0, tu.x0, tu.y0, w, h, &p);
            } else {
                let mut p = std::mem::take(&mut self.scratch.pred);
                p.clear();
                p.resize(w * h, 0);
                if cu.pred == MODE_IBC {
                    self.ibc_predict(0, tu.x0, tu.y0, w, h, &mut p);
                } else {
                    self.predict(cu, 0, tu.x0, tu.y0, w, h, &mut p);
                }
                self.finish_tb(cu, tu, 0, tu.x0, tu.y0, w, h, &p);
                self.scratch.pred = p;
            }
        }
    }

    fn recon_chroma_tb(&mut self, cu: &Cu, tu: &Tu, ci: usize) {
        let (w, h) = (tu.wc as usize, tu.hc as usize);
        let mut p = std::mem::take(&mut self.scratch.pred);
        p.clear();
        p.resize(w * h, 0);
        if cu.pred == MODE_IBC {
            self.ibc_predict(ci, tu.xc, tu.yc, w, h, &mut p);
        } else {
            self.predict(cu, ci, tu.xc, tu.yc, w, h, &mut p);
        }
        self.finish_tb(cu, tu, ci, tu.xc, tu.yc, w, h, &p);
        self.scratch.pred = p;
    }

    /// Residual, reconstruction and availability marking of one transform block.
    #[allow(clippy::too_many_arguments)]
    fn finish_tb(
        &mut self,
        cu: &Cu,
        tu: &Tu,
        ci: usize,
        x: u32,
        y: u32,
        w: usize,
        h: usize,
        pred: &[i32],
    ) {
        let mut res = std::mem::take(&mut self.scratch.res);
        res.clear();
        res.resize(w * h, 0);
        let has_res = if cu.act {
            // Colour-converted residuals of the whole coding unit (8.7.4.6).
            let a = &self.scratch.act[ci];
            let (sx, sy) = if ci == 0 { (0, 0) } else { (self.sw, self.sh) };
            let cw = (cu.w >> sx) as usize;
            let (ox, oy) = ((x - (cu.x0 >> sx)) as usize, (y - (cu.y0 >> sy)) as usize);
            for yy in 0..h {
                res[yy * w..yy * w + w]
                    .copy_from_slice(&a[(oy + yy) * cw + ox..(oy + yy) * cw + ox + w]);
            }
            true
        } else {
            self.residual(cu, tu, ci, w, h, &mut res)
        };
        let bd = self.bd;
        let max = (1i32 << bd) - 1;
        let (sx, sy) = if ci == 0 { (0, 0) } else { (self.sw, self.sh) };
        // Chroma residual scaling (8.7.5.3).
        let mut var_scale = None;
        if ci > 0
            && let Some(l) = self.c.lmcs
            && self.c.ph.chroma_residual_scale
            && w * h > 4
            && cu.pred != MODE_PLT
            && (tu.cbf[1] || tu.cbf[2] || cu.act)
        {
            var_scale = Some(self.chroma_scale(l, x << sx, y << sy));
        }
        let tu_cbf = if ci == 1 {
            tu.cbf[1] || tu.joint
        } else {
            tu.cbf[2] || tu.joint
        };
        let plane = &mut self.st.pic.planes[ci];
        let stride = plane.stride;
        for yy in 0..h {
            let row = (y as usize + yy) * stride + x as usize;
            for xx in 0..w {
                let p = pred[yy * w + xx];
                let v = match var_scale {
                    Some(s) => {
                        if tu_cbf || cu.act {
                            let r = res[yy * w + xx].clamp(-(1 << bd), (1 << bd) - 1);
                            let m = (r.abs() * s + (1 << 10)) >> 11;
                            p + if r < 0 { -m } else { m }
                        } else {
                            p
                        }
                    }
                    None => {
                        if has_res {
                            p + res[yy * w + xx]
                        } else {
                            p
                        }
                    }
                };
                plane.data[row + xx] = v.clamp(0, max) as u16;
            }
        }
        self.scratch.res = res;
        self.ibc_store(ci, x as usize, y as usize, w, h);
        // IsAvailable.
        let c = ci;
        let m = &mut self.st.maps;
        let (lx, ly) = (x << sx, y << sy);
        let (lw, lh) = ((w as u32) << sx, (h as u32) << sy);
        for i in m.cells(lx, ly, lw, lh) {
            m.avail[c][i] = true;
        }
    }

    /// Residuals of the three components of an ACT coding unit, colour converted.
    fn act_residuals(&mut self, cu: &Cu, tus: &[Tu]) {
        let (w, h) = (cu.w as usize, cu.h as usize);
        let mut bufs: [Vec<i32>; 3] = std::mem::take(&mut self.scratch.act);
        for (ci, b) in bufs.iter_mut().enumerate() {
            b.clear();
            b.resize(w * h, 0);
            for tu in tus {
                let (tw, th, tx, ty) = if ci == 0 {
                    (tu.w, tu.h, tu.x0, tu.y0)
                } else {
                    (tu.wc, tu.hc, tu.xc, tu.yc)
                };
                let (tw, th) = (tw as usize, th as usize);
                let mut r = vec![0i32; tw * th];
                if self.residual(cu, tu, ci, tw, th, &mut r) {
                    let (ox, oy) = ((tx - cu.x0) as usize, (ty - cu.y0) as usize);
                    for yy in 0..th {
                        b[(oy + yy) * w + ox..(oy + yy) * w + ox + tw]
                            .copy_from_slice(&r[yy * tw..yy * tw + tw]);
                    }
                }
            }
        }
        let lim = 1i32 << (self.bd + 1);
        let [ry, rcb, rcr] = &mut bufs;
        for i in 0..w * h {
            let y = ry[i].clamp(-lim, lim - 1);
            let cb = rcb[i].clamp(-lim, lim - 1);
            let cr = rcr[i].clamp(-lim, lim - 1);
            let t = y - (cb >> 1);
            ry[i] = t + cb;
            let ncb = t - (cr >> 1);
            rcb[i] = ncb;
            rcr[i] = cr + ncb;
        }
        self.scratch.act = bufs;
    }

    /// varScale for a chroma block at luma position (xl, yl) (8.7.5.3).
    fn chroma_scale(&self, l: &crate::lmcs::Lmcs, xl: u32, yl: u32) -> i32 {
        let size_y = (1u32 << self.ctb_log2).min(64);
        let (vx, vy) = (xl / size_y * size_y, yl / size_y * size_y);
        let m = &self.st.maps;
        let (cx, cy) = m.cu_pos[m.idx(vx, vy)];
        let (cx, cy) = (u32::from(cx), u32::from(cy));
        let avail_l = self.available(cx, cy, cx as i32 - 1, cy as i32, 0);
        let avail_t = self.available(cx, cy, cx as i32, cy as i32 - 1, 0);
        let luma = &self.st.pic.planes[0];
        let mut sum = 0i32;
        let mut cnt = 0u32;
        if avail_l {
            for i in 0..size_y {
                sum += i32::from(luma.at(cx as usize - 1, (cy + i).min(self.pic_h - 1) as usize));
            }
            cnt += size_y;
        }
        if avail_t {
            for i in 0..size_y {
                sum += i32::from(luma.at((cx + i).min(self.pic_w - 1) as usize, cy as usize - 1));
            }
            cnt += size_y;
        }
        let avg = if cnt > 0 {
            (sum + (cnt as i32 >> 1)) >> cnt.trailing_zeros()
        } else {
            1 << (self.bd - 1)
        };
        l.chroma_scale_for(avg)
    }

    /// Scaling and transformation (8.7.2) of component `ci` of `tu`; false when all zero.
    fn residual(
        &mut self,
        cu: &Cu,
        tu: &Tu,
        ci: usize,
        w: usize,
        h: usize,
        out: &mut [i32],
    ) -> bool {
        let coded = if ci == 0 || tu.res_mode == 0 {
            ci
        } else if tu.res_mode == 1 || tu.res_mode == 2 {
            1
        } else {
            2
        };
        let coeff = &tu.coeff[coded];
        if coeff.is_empty() {
            return false;
        }
        let sps = self.c.sps;
        let sh = self.c.sh;
        let ts = tu.ts[coded];
        let qbd = sps.qp_bd_offset();
        let qp_in = if coded == 0 {
            cu.qp[0]
        } else if tu.res_mode == 2 {
            cu.qp[3]
        } else {
            cu.qp[coded]
        };
        let act_off = if !cu.act {
            0
        } else if coded == 0 {
            -5
        } else if tu.res_mode == 2 || coded == 1 {
            1
        } else {
            3
        };
        let qp_in = qp_in + act_off;
        let log2w = w.trailing_zeros();
        let log2h = h.trailing_zeros();
        let dq = sh.dep_quant && !ts;
        let (qp, rect, shift) = if !ts {
            let qp = qp_in.clamp(0, 63 + qbd);
            let rect = ((log2w + log2h) & 1) as usize;
            let shift = self.bd as i32 + rect as i32 + ((log2w + log2h) / 2) as i32 + 10 - 15
                + i32::from(sh.dep_quant);
            (qp, rect, shift)
        } else {
            (qp_in.clamp(sps.min_qp_prime_ts, 63 + qbd), 0, 10)
        };
        let lfnst = cu.lfnst_idx > 0
            && (if coded == 0 {
                cu.tree != TreeType::DualChroma
            } else {
                cu.tree == TreeType::DualChroma
            });
        let qq = if dq { qp + 1 } else { qp };
        let ls_base = LEVEL_SCALE[rect][(qq % 6) as usize] << (qq / 6);
        let off = (1i64 << shift) >> 1;
        let scaling = self.c.scaling.filter(|_| {
            !ts && !(sps.scaling_matrix_for_lfnst_disabled && lfnst)
                && !(sps.scaling_matrix_for_alt_colour_space_disabled
                    && sps.scaling_matrix_designated_colour_space == cu.act)
        });
        let mut d = std::mem::take(&mut self.scratch.tmp);
        d.clear();
        d.resize(w * h, 0);
        let bdpcm = cu.bdpcm[usize::from(coded > 0)];
        let bdpcm_dir = cu.bdpcm_dir[usize::from(coded > 0)];
        let mut dz = coeff.clone();
        if bdpcm {
            if !bdpcm_dir {
                for y in 0..h {
                    for x in 1..w {
                        let v = dz[y * w + x - 1] + dz[y * w + x];
                        dz[y * w + x] = v.clamp(transform::COEFF_MIN, transform::COEFF_MAX);
                    }
                }
            } else {
                for y in 1..h {
                    for x in 0..w {
                        let v = dz[(y - 1) * w + x] + dz[y * w + x];
                        dz[y * w + x] = v.clamp(transform::COEFF_MIN, transform::COEFF_MAX);
                    }
                }
            }
        }
        let id = scaling.map(|_| {
            let size = w.max(h);
            let l = size.trailing_zeros() as usize;
            // Table 38.
            match (cu.pred == MODE_INTRA, coded) {
                (true, 0) => [0, 0, 2, 8, 14, 20, 26][l],
                (true, 1) => [0, 0, 3, 9, 15, 21, 21][l],
                (true, _) => [0, 0, 4, 10, 16, 22, 22][l],
                (false, 0) => [0, 0, 5, 11, 17, 23, 27][l],
                (false, 1) => [0, 0, 6, 12, 18, 24, 24][l],
                (false, _) => [0, 1, 7, 13, 19, 25, 25][l],
            }
        });
        for y in 0..h {
            for x in 0..w {
                let z = dz[y * w + x];
                if z == 0 {
                    continue;
                }
                let m = match (scaling, id) {
                    (Some(s), Some(id)) => {
                        let lm = if id < 2 {
                            1
                        } else if id < 8 {
                            2
                        } else {
                            3
                        };
                        let i = (x << lm) >> log2w;
                        let j = (y << lm) >> log2h;
                        if id > 13 && x == 0 && y == 0 {
                            i64::from(s.dc[id - 14])
                        } else {
                            i64::from(s.rec[id][(j << lm) + i])
                        }
                    }
                    _ => 16,
                };
                let ls = m * ls_base;
                let v = (i64::from(z) * ls + off) >> shift;
                d[y * w + x] = v.clamp(
                    i64::from(transform::COEFF_MIN),
                    i64::from(transform::COEFF_MAX),
                ) as i32;
            }
        }
        if ts {
            out[..w * h].copy_from_slice(&d[..w * h]);
        } else {
            self.inverse_transform(cu, tu, coded, w, h, lfnst, &mut d, out);
        }
        self.scratch.tmp = d;
        if ci != coded {
            let sign = if self.c.ph.joint_cbcr_sign { -1 } else { 1 };
            for v in out[..w * h].iter_mut() {
                *v = if tu.res_mode == 2 {
                    sign * *v
                } else {
                    (sign * *v) >> 1
                };
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn inverse_transform(
        &self,
        cu: &Cu,
        tu: &Tu,
        ci: usize,
        w: usize,
        h: usize,
        lfnst: bool,
        d: &mut [i32],
        out: &mut [i32],
    ) {
        let _ = tu;
        let sps = self.c.sps;
        if lfnst {
            let mut mode = if ci == 0 {
                if cu.mip { 0 } else { cu.mode_y }
            } else {
                cu.mode_c
            };
            if (81..=83).contains(&mode) {
                let m = &self.st.maps;
                let i = m.idx(cu.x0 + cu.w / 2, cu.y0 + cu.h / 2);
                mode = if m.mip[i] {
                    0
                } else if m.pred[0][i] == MODE_IBC || m.pred[0][i] == MODE_PLT {
                    1
                } else {
                    i32::from(m.mode_y[i])
                };
            }
            let (nw, nh) = if ci == 0 && cu.isp != ISP_NONE {
                (cu.w as usize, cu.h as usize)
            } else {
                (w, h)
            };
            let mode = intra::wide_angle(mode, nw, nh);
            transform::lfnst(d, w, h, cu.lfnst_idx, mode);
        }
        let implicit = sps.mts
            && (cu.isp != ISP_NONE
                || (!sps.explicit_mts_intra
                    && cu.pred == MODE_INTRA
                    && cu.lfnst_idx == 0
                    && !cu.mip));
        let (th, tv) = if ci > 0 || (cu.isp != ISP_NONE && cu.lfnst_idx != 0) {
            (0, 0)
        } else if implicit {
            (
                u8::from((4..=16).contains(&w)),
                u8::from((4..=16).contains(&h)),
            )
        } else {
            match cu.mts_idx {
                1 => (1, 1),
                2 => (2, 1),
                3 => (1, 2),
                4 => (2, 2),
                _ => (0, 0),
            }
        };
        let (nzw, nzh) = if lfnst {
            let n = if w == 4 || h == 4 { 4 } else { 8 };
            (n, n)
        } else {
            (
                w.min(if th > 0 { 16 } else { 32 }),
                h.min(if tv > 0 { 16 } else { 32 }),
            )
        };
        transform::inverse(d, w, h, th, tv, nzw, nzh, self.bd, out);
    }

    /// Intra sample prediction (8.4.5.2.1) of a `w x h` block of component `ci` at component
    /// position (x, y).
    #[allow(clippy::too_many_arguments)]
    fn predict(&mut self, cu: &Cu, ci: usize, x: u32, y: u32, w: usize, h: usize, out: &mut [i32]) {
        let bd = self.bd;
        let max = (1i32 << bd) - 1;
        let isp_luma = ci == 0 && cu.isp != ISP_NONE;
        let use_mip = (ci == 0 && cu.mip) || (ci > 0 && cu.mip_chroma_direct);
        if use_mip {
            let (mut top, mut left) = self.take_refs();
            self.build_refs(ci, x, y, w + 1, h + 1, 0, &mut top, &mut left);
            mip::predict(
                &top[1..1 + w],
                &left[1..1 + h],
                w,
                h,
                cu.mip_mode as usize,
                cu.mip_transposed,
                bd,
                out,
            );
            self.put_refs(top, left);
            return;
        }
        let mode0 = if ci == 0 { cu.mode_y } else { cu.mode_c };
        let (cbw, cbh) = if isp_luma {
            (cu.w as usize, cu.h as usize)
        } else {
            (w, h)
        };
        let (ref_w, ref_h) = if isp_luma {
            (cbw + w, cbh + h)
        } else {
            (2 * w, 2 * h)
        };
        let r = if ci == 0 { cu.mrl as usize } else { 0 };
        let (mut top, mut left) = self.take_refs();
        self.build_refs(ci, x, y, ref_w, ref_h, r, &mut top, &mut left);
        if (81..=83).contains(&mode0) {
            self.cclm(cu, ci, x, y, w, h, mode0, &top, &left, out);
            self.put_refs(top, left);
            return;
        }
        let mode = intra::wide_angle(mode0, cbw, cbh);
        let ref_filter = intra::ref_filter_flag(mode);
        if r == 0 && w * h > 32 && ci == 0 && cu.isp == ISP_NONE && ref_filter {
            intra::filter_refs(&mut top, &mut left);
        }
        match mode {
            intra::PLANAR => intra::planar(&top, &left, w, h, out),
            intra::DC => intra::dc(&top, &left, r, w, h, out),
            _ => {
                let filter = !(ref_filter || r != 0 || cu.isp != ISP_NONE)
                    && intra::angular_filter_flag(mode, w, h);
                intra::angular(mode, r, &top, &left, w, h, filter, ci == 0, max, out);
            }
        }
        let bdpcm = cu.bdpcm[usize::from(ci > 0)];
        if w >= 4
            && h >= 4
            && r == 0
            && !bdpcm
            && (mode == intra::PLANAR
                || mode == intra::DC
                || mode <= 18
                || (50..81).contains(&mode))
        {
            intra::pdpc(mode, w, h, &top, &left, max, out);
        }
        self.put_refs(top, left);
    }

    fn take_refs(&mut self) -> (Vec<i32>, Vec<i32>) {
        (
            std::mem::take(&mut self.scratch.top),
            std::mem::take(&mut self.scratch.left),
        )
    }

    fn put_refs(&mut self, top: Vec<i32>, left: Vec<i32>) {
        self.scratch.top = top;
        self.scratch.left = left;
    }

    /// Reference samples with availability marking and substitution (8.4.5.2.8, 8.4.5.2.9).
    #[allow(clippy::too_many_arguments)]
    fn build_refs(
        &mut self,
        ci: usize,
        x: u32,
        y: u32,
        ref_w: usize,
        ref_h: usize,
        r: usize,
        top: &mut Vec<i32>,
        left: &mut Vec<i32>,
    ) {
        let (sx, sy) = if ci == 0 { (0, 0) } else { (self.sw, self.sh) };
        let (xl, yl) = (x << sx, y << sy);
        let nt = ref_w + r + 1;
        let nl = ref_h + r + 1;
        top.clear();
        top.resize(nt, 0);
        left.clear();
        left.resize(nl, 0);
        let mut av = std::mem::take(&mut self.scratch.avail);
        av.clear();
        // Sequence order of 8.4.5.2.9: left bottom-up (excluding the corner), corner, top.
        av.resize(nl + nt - 1, false);
        let plane = &self.st.pic.planes[ci];
        let ri = r as i32;
        let mut any = false;
        let mut all = true;
        // Left column: y' = j - 1 - r for j = 0..nl.
        let mut key = (i32::MIN, i32::MIN);
        let mut last = false;
        let mut avail = |d: &Self, xn: i32, yn: i32| -> bool {
            // IsAvailable and every other condition are constant per 4x4 luma unit.
            let k = (xn >> 2, yn >> 2);
            if k != key || xn < 0 || yn < 0 {
                key = k;
                last = d.available(xl, yl, xn, yn, ci);
            }
            last
        };
        for j in 1..nl {
            let yy = y as i32 + j as i32 - 1 - ri;
            let xx = x as i32 - 1 - ri;
            let a = avail(self, xx << sx, yy << sy);
            let k = nl - 1 - j;
            av[k] = a;
            if a {
                left[j] = i32::from(plane.at(xx as usize, yy as usize));
                any = true;
            } else {
                all = false;
            }
        }
        // Top row (index 0 is the corner).
        for i in 0..nt {
            let xx = x as i32 + i as i32 - 1 - ri;
            let yy = y as i32 - 1 - ri;
            let a = avail(self, xx << sx, yy << sy);
            av[nl - 1 + i] = a;
            if a {
                top[i] = i32::from(plane.at(xx as usize, yy as usize));
                any = true;
            } else {
                all = false;
            }
        }
        if !all {
            let get = |k: usize, top: &Vec<i32>, left: &Vec<i32>| -> i32 {
                if k < nl - 1 {
                    left[nl - 1 - k]
                } else {
                    top[k - (nl - 1)]
                }
            };
            let set = |k: usize, v: i32, top: &mut Vec<i32>, left: &mut Vec<i32>| {
                if k < nl - 1 {
                    left[nl - 1 - k] = v;
                } else {
                    top[k - (nl - 1)] = v;
                }
            };
            let n = av.len();
            if !any {
                let v = 1 << (self.bd - 1);
                top.fill(v);
                left.fill(v);
            } else {
                if !av[0] {
                    let k = av.iter().position(|&a| a).unwrap_or(0);
                    let v = get(k, top, left);
                    set(0, v, top, left);
                }
                for k in 1..n {
                    if !av[k] {
                        let v = get(k - 1, top, left);
                        set(k, v, top, left);
                    }
                }
            }
        }
        left[0] = top[0];
        self.scratch.avail = av;
    }

    #[allow(clippy::too_many_arguments)]
    fn cclm(
        &self,
        cu: &Cu,
        ci: usize,
        x: u32,
        y: u32,
        w: usize,
        h: usize,
        mode: i32,
        top: &[i32],
        left: &[i32],
        out: &mut [i32],
    ) {
        let _ = cu;
        let (sx, sy) = (self.sw, self.sh);
        let (xl, yl) = (x << sx, y << sy);
        let avail_l = self.available(xl, yl, xl as i32 - 1, yl as i32, ci);
        let avail_t = self.available(xl, yl, xl as i32, yl as i32 - 1, ci);
        let mut ntr = 0;
        if mode == intra::T_CCLM {
            for xx in w..2 * w {
                if !self.available(xl, yl, (xl + ((xx as u32) << sx)) as i32, yl as i32 - 1, ci) {
                    break;
                }
                ntr += 1;
            }
        }
        let mut nlb = 0;
        if mode == intra::L_CCLM {
            for yy in h..2 * h {
                if !self.available(xl, yl, xl as i32 - 1, (yl + ((yy as u32) << sy)) as i32, ci) {
                    break;
                }
                nlb += 1;
            }
        }
        let input = cclm::CclmIn {
            mode,
            xc: x as usize,
            yc: y as usize,
            w,
            h,
            avail_l,
            avail_t,
            num_top_right: ntr,
            num_left_below: nlb,
            top,
            left,
            sub_w: 1 << sx,
            sub_h: 1 << sy,
            vertical_collocated: self.c.sps.chroma_vertical_collocated,
            ctb_size: 1 << self.ctb_log2,
            bit_depth: self.bd,
        };
        cclm::predict(&input, &self.st.pic.planes[0], out);
    }
}
