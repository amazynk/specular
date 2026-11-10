//! `coding_unit()` (7.3.11.5), `transform_tree()` and `transform_unit()` (7.3.11.9-10) for
//! intra coding units, with the luma and chroma intra mode derivations (8.4.2 to 8.4.4).

use crate::cabac::{self, Engine};
use crate::error::{Error, Result};

use super::{
    Dec, MODE_IBC, MODE_INTRA, MODE_PLT, ModeType, SPLIT_BT_HOR, SPLIT_BT_VER, TreeType, tb_bypass,
};

pub const ISP_NONE: u8 = 0;
pub const ISP_HOR: u8 = 1;
pub const ISP_VER: u8 = 2;

/// One transform unit of a coding unit, with its coefficients.
#[derive(Clone, Debug, Default)]
pub struct Tu {
    /// Luma position and size (luma samples); for a dual-tree chroma TU the luma area.
    pub x0: u32,
    pub y0: u32,
    pub w: u32,
    pub h: u32,
    /// Chroma position and size (chroma samples).
    pub xc: u32,
    pub yc: u32,
    pub wc: u32,
    pub hc: u32,
    pub has_luma: bool,
    pub has_chroma: bool,
    pub cbf: [bool; 3],
    pub joint: bool,
    /// TuCResMode.
    pub res_mode: u8,
    pub ts: [bool; 3],
    /// TransCoeffLevel per component, row-major; empty when not coded.
    pub coeff: [Vec<i32>; 3],
}

/// A parsed coding unit.
#[derive(Clone, Debug)]
pub struct Cu {
    pub x0: u32,
    pub y0: u32,
    pub w: u32,
    pub h: u32,
    pub cqt: u32,
    pub tree: TreeType,
    pub pred: u8,
    pub plt: bool,
    pub act: bool,
    pub bdpcm: [bool; 2],
    pub bdpcm_dir: [bool; 2],
    pub mip: bool,
    pub mip_transposed: bool,
    pub mip_mode: u32,
    pub mrl: u32,
    pub isp: u8,
    pub isp_parts: u32,
    pub mode_y: i32,
    pub mode_c: i32,
    pub mip_chroma_direct: bool,
    pub lfnst_idx: u32,
    pub mts_idx: u32,
    pub qp_y: i32,
    /// Qp′Y, Qp′Cb, Qp′Cr, Qp′CbCr.
    pub qp: [i32; 4],
    pub tus: Vec<Tu>,
    pub lfnst_dc_only: bool,
    pub lfnst_zero_out: bool,
    pub mts_dc_only: bool,
    pub mts_zero_out: bool,
}

impl Dec<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn coding_unit(
        &mut self,
        e: &mut Engine<'_>,
        x0: u32,
        y0: u32,
        w: u32,
        h: u32,
        cqt: u32,
        tree: TreeType,
        mode: ModeType,
    ) -> Result<()> {
        let sps = self.c.sps;
        let mut mode = mode;
        if self.intra_slice && (w > 64 || h > 64) {
            mode = ModeType::Intra;
        }
        let ch = usize::from(tree == TreeType::DualChroma);
        let mut cu = Cu {
            x0,
            y0,
            w,
            h,
            cqt,
            tree,
            pred: MODE_INTRA,
            plt: false,
            act: false,
            bdpcm: [false; 2],
            bdpcm_dir: [false; 2],
            mip: false,
            mip_transposed: false,
            mip_mode: 0,
            mrl: 0,
            isp: ISP_NONE,
            isp_parts: 1,
            mode_y: 0,
            mode_c: 0,
            mip_chroma_direct: false,
            lfnst_idx: 0,
            mts_idx: 0,
            qp_y: 0,
            qp: [0; 4],
            tus: Vec::new(),
            lfnst_dc_only: true,
            lfnst_zero_out: true,
            mts_dc_only: true,
            mts_zero_out: true,
        };
        // CbWidth, CbHeight and CqtDepth are known from the start of the coding unit.
        self.store_cu_maps(&cu, false);
        let mut skip = false;
        if !self.intra_slice || sps.ibc {
            if tree != TreeType::DualChroma
                && ((!(w == 4 && h == 4) && mode != ModeType::Intra)
                    || (sps.ibc && w <= 64 && h <= 64))
            {
                let inc = self.lr_ctx(x0, y0, 0, |m, i| m.skip[i]);
                skip = e.decode(&mut self.ctx.0[cabac::CU_SKIP_FLAG + inc]) == 1;
            }
            // pred_mode_flag only in P/B slices.
            let ibc_cond = (self.intra_slice && !skip)
                && w <= 64
                && h <= 64
                && mode != ModeType::Inter
                && sps.ibc
                && tree != TreeType::DualChroma;
            let mut ibc = false;
            if ibc_cond {
                let inc = self.lr_ctx(x0, y0, ch, |m, i| m.pred[ch][i] == MODE_IBC);
                ibc = e.decode(&mut self.ctx.0[cabac::PRED_MODE_IBC_FLAG + inc]) == 1;
            } else if skip && ((w == 4 && h == 4) || mode == ModeType::Intra) {
                ibc = true;
            } else if w == 128
                || h == 128
                || mode == ModeType::Inter
                || tree == TreeType::DualChroma
            {
                ibc = false;
            } else if skip {
                ibc = sps.ibc && self.intra_slice;
            }
            if skip && self.intra_slice {
                ibc = true;
            }
            if ibc {
                cu.pred = MODE_IBC;
                self.store_cu_maps(&cu, skip);
                return self.ibc_cu(e, &mut cu, skip);
            }
        }
        if cu.pred == MODE_INTRA
            && sps.palette
            && w <= 64
            && h <= 64
            && !skip
            && mode != ModeType::Inter
            && w * h
                > if tree != TreeType::DualChroma {
                    16
                } else {
                    16 << (self.sw + self.sh)
                }
            && (mode != ModeType::Intra || tree != TreeType::DualChroma)
            && e.decode(&mut self.ctx.0[cabac::PRED_MODE_PLT_FLAG]) == 1
        {
            cu.plt = true;
            cu.pred = MODE_PLT;
            self.store_cu_maps(&cu, skip);
            self.palette_cu(e, &mut cu)?;
            return self.reconstruct(&mut cu);
        }
        if cu.pred == MODE_INTRA && sps.act && tree == TreeType::Single {
            cu.act = e.decode(&mut self.ctx.0[cabac::CU_ACT_ENABLED_FLAG]) == 1;
        }
        let max_tb = 1u32 << self.max_tb_log2;
        if tree != TreeType::DualChroma {
            if sps.bdpcm && w <= self.max_ts && h <= self.max_ts {
                cu.bdpcm[0] = e.decode(&mut self.ctx.0[cabac::INTRA_BDPCM_LUMA_FLAG]) == 1;
            }
            if cu.bdpcm[0] {
                cu.bdpcm_dir[0] = e.decode(&mut self.ctx.0[cabac::INTRA_BDPCM_LUMA_DIR_FLAG]) == 1;
                cu.mode_y = if cu.bdpcm_dir[0] { 50 } else { 18 };
            } else {
                if sps.mip {
                    let r = (w.trailing_zeros() as i32 - h.trailing_zeros() as i32).abs();
                    let inc = if r > 1 {
                        3
                    } else {
                        self.lr_ctx(x0, y0, 0, |m, i| m.mip[i])
                    };
                    cu.mip = e.decode(&mut self.ctx.0[cabac::INTRA_MIP_FLAG + inc]) == 1;
                }
                if cu.mip {
                    cu.mip_transposed = e.bypass() == 1;
                    let cmax = if w == 4 && h == 4 {
                        15
                    } else if w == 4 || h == 4 || (w == 8 && h == 8) {
                        7
                    } else {
                        5
                    };
                    cu.mip_mode = tb_bypass(e, cmax);
                    cu.mode_y = cu.mip_mode as i32;
                } else {
                    if sps.mrl && !y0.is_multiple_of(1 << self.ctb_log2) {
                        cu.mrl = if e.decode(&mut self.ctx.0[cabac::INTRA_LUMA_REF_IDX]) == 0 {
                            0
                        } else if e.decode(&mut self.ctx.0[cabac::INTRA_LUMA_REF_IDX + 1]) == 0 {
                            1
                        } else {
                            2
                        };
                    }
                    if sps.isp && cu.mrl == 0 && w <= max_tb && h <= max_tb && w * h > 16 && !cu.act
                    {
                        let isp =
                            e.decode(&mut self.ctx.0[cabac::INTRA_SUBPARTITIONS_MODE_FLAG]) == 1;
                        if isp {
                            let split =
                                e.decode(&mut self.ctx.0[cabac::INTRA_SUBPARTITIONS_SPLIT_FLAG]);
                            cu.isp = 1 + split as u8;
                            cu.isp_parts = if (w == 4 && h == 8) || (w == 8 && h == 4) {
                                2
                            } else {
                                4
                            };
                        }
                    }
                    let mpm_flag = if cu.mrl == 0 {
                        e.decode(&mut self.ctx.0[cabac::INTRA_LUMA_MPM_FLAG]) == 1
                    } else {
                        true
                    };
                    if mpm_flag {
                        let not_planar = if cu.mrl == 0 {
                            let inc = usize::from(cu.isp == ISP_NONE);
                            e.decode(&mut self.ctx.0[cabac::INTRA_LUMA_NOT_PLANAR_FLAG + inc]) == 1
                        } else {
                            true
                        };
                        if !not_planar {
                            cu.mode_y = 0;
                        } else {
                            let mut idx = 0;
                            while idx < 4 && e.bypass() == 1 {
                                idx += 1;
                            }
                            let cand = self.mpm_list(x0, y0, w, h);
                            cu.mode_y = cand[idx];
                        }
                    } else {
                        let rem = tb_bypass(e, 60) as i32;
                        let mut cand = self.mpm_list(x0, y0, w, h);
                        cand.sort_unstable();
                        let mut m = rem + 1;
                        for c in cand {
                            if m >= c {
                                m += 1;
                            }
                        }
                        cu.mode_y = m;
                    }
                }
            }
        }
        if tree != TreeType::DualLuma && self.chroma {
            let (cw, chh) = (w >> self.sw, h >> self.sh);
            if !cu.act {
                if cw <= self.max_ts && chh <= self.max_ts && sps.bdpcm {
                    cu.bdpcm[1] = e.decode(&mut self.ctx.0[cabac::INTRA_BDPCM_CHROMA_FLAG]) == 1;
                }
                if cu.bdpcm[1] {
                    cu.bdpcm_dir[1] =
                        e.decode(&mut self.ctx.0[cabac::INTRA_BDPCM_CHROMA_DIR_FLAG]) == 1;
                    cu.mode_c = if cu.bdpcm_dir[1] { 50 } else { 18 };
                } else {
                    let mut cclm = false;
                    let mut cclm_idx = 0;
                    if self.cclm_enabled(x0, y0, &cu) {
                        cclm = e.decode(&mut self.ctx.0[cabac::CCLM_MODE_FLAG]) == 1;
                    }
                    let mut icpm = 0;
                    if cclm {
                        cclm_idx = if e.decode(&mut self.ctx.0[cabac::CCLM_MODE_IDX]) == 0 {
                            0
                        } else {
                            1 + e.bypass()
                        };
                    } else if e.decode(&mut self.ctx.0[cabac::INTRA_CHROMA_PRED_MODE]) == 0 {
                        icpm = 4;
                    } else {
                        icpm = e.bypass_bits(2);
                    }
                    self.chroma_mode(&mut cu, cclm, cclm_idx, icpm);
                }
            }
        }
        if cu.act {
            // IntraPredModeC follows luma (8.4.3).
            self.chroma_mode(&mut cu, false, 0, 4);
        }
        // Store the CU-level maps the transform tree and later CUs read.
        self.store_cu_maps(&cu, skip);
        self.transform_tree(e, &mut cu, x0, y0, w, h)?;
        // lfnst_idx and mts_idx.
        let lw = if tree == TreeType::DualChroma {
            w >> self.sw
        } else if cu.isp == ISP_VER {
            w / cu.isp_parts
        } else {
            w
        };
        let lh = if tree == TreeType::DualChroma {
            h >> self.sh
        } else if cu.isp == ISP_HOR {
            h / cu.isp_parts
        } else {
            h
        };
        let first = cu.tus.first();
        // The chroma flags at the coding unit's origin: with ISP they belong to the last TU.
        let firstc = cu.tus.iter().find(|t| t.has_chroma);
        let ts0 = first.is_some_and(|t| t.ts[0]);
        let not_ts = (tree == TreeType::DualChroma || !first.is_some_and(|t| t.cbf[0]) || !ts0)
            && (tree == TreeType::DualLuma
                || ((!firstc.is_some_and(|t| t.cbf[1]) || !firstc.is_some_and(|t| t.ts[1]))
                    && (!firstc.is_some_and(|t| t.cbf[2]) || !firstc.is_some_and(|t| t.ts[2]))));
        if lw.min(lh) >= 4
            && sps.lfnst
            && cu.pred == MODE_INTRA
            && not_ts
            && (tree == TreeType::DualChroma || !cu.mip || lw.min(lh) >= 16)
            && w.max(h) <= max_tb
            && (cu.isp != ISP_NONE || !cu.lfnst_dc_only)
            && cu.lfnst_zero_out
        {
            let inc = usize::from(tree != TreeType::Single);
            if e.decode(&mut self.ctx.0[cabac::LFNST_IDX + inc]) == 1 {
                cu.lfnst_idx = 1 + e.decode(&mut self.ctx.0[cabac::LFNST_IDX + 2]);
            }
        }
        if tree != TreeType::DualChroma
            && cu.lfnst_idx == 0
            && !ts0
            && w.max(h) <= 32
            && cu.isp == ISP_NONE
            && cu.mts_zero_out
            && !cu.mts_dc_only
            && cu.pred == MODE_INTRA
            && sps.explicit_mts_intra
        {
            let mut v = 0;
            while v < 4 && e.decode(&mut self.ctx.0[cabac::MTS_IDX + v as usize]) == 1 {
                v += 1;
            }
            cu.mts_idx = v;
        }
        self.cu_qp(&mut cu);
        self.reconstruct(&mut cu)
    }

    /// ctxInc from left and above flags (9.3.4.2.2): condL && availableL + condA && availableA.
    fn lr_ctx(
        &self,
        x0: u32,
        y0: u32,
        ch: usize,
        f: impl Fn(&super::Maps, usize) -> bool,
    ) -> usize {
        let m = &self.st.maps;
        let l = self.available(x0, y0, x0 as i32 - 1, y0 as i32, ch) && f(m, m.idx(x0 - 1, y0));
        let a = self.available(x0, y0, x0 as i32, y0 as i32 - 1, ch) && f(m, m.idx(x0, y0 - 1));
        usize::from(l) + usize::from(a)
    }

    /// candModeList (8.4.2).
    fn mpm_list(&self, x0: u32, y0: u32, w: u32, h: u32) -> [i32; 5] {
        let m = &self.st.maps;
        let cand = |xn: i32, yn: i32, is_b: bool| -> i32 {
            if !self.available(x0, y0, xn, yn, 0) {
                return 0;
            }
            let i = m.idx(xn as u32, yn as u32);
            if m.pred[0][i] != MODE_INTRA || m.mip[i] {
                return 0;
            }
            if is_b && (y0 as i32 - 1) < (((y0 >> self.ctb_log2) << self.ctb_log2) as i32) {
                return 0;
            }
            i32::from(m.mode_y[i])
        };
        let a = cand(x0 as i32 - 1, (y0 + h - 1) as i32, false);
        let b = cand((x0 + w - 1) as i32, y0 as i32 - 1, true);
        let wrap = |v: i32| 2 + v.rem_euclid(64);
        if a == b && a > 1 {
            [a, wrap(a + 61), wrap(a - 1), wrap(a + 60), wrap(a)]
        } else if a != b && (a > 1 || b > 1) {
            let (mn, mx) = (a.min(b), a.max(b));
            if a > 1 && b > 1 {
                let d = mx - mn;
                if d == 1 {
                    [a, b, wrap(mn + 61), wrap(mx - 1), wrap(mn + 60)]
                } else if d >= 62 {
                    [a, b, wrap(mn - 1), wrap(mx + 61), wrap(mn)]
                } else if d == 2 {
                    [a, b, wrap(mn - 1), wrap(mn + 61), wrap(mx - 1)]
                } else {
                    [a, b, wrap(mn + 61), wrap(mn - 1), wrap(mx + 61)]
                }
            } else {
                [mx, wrap(mx + 61), wrap(mx - 1), wrap(mx + 60), wrap(mx)]
            }
        } else {
            [1, 50, 18, 46, 54]
        }
    }

    /// CclmEnabled (8.4.4).
    fn cclm_enabled(&self, x0: u32, y0: u32, cu: &Cu) -> bool {
        let sps = self.c.sps;
        if !sps.cclm {
            return false;
        }
        if !sps.qtbtt_dual_tree_intra || !self.intra_slice || self.ctb_log2 < 6 {
            return true;
        }
        let _ = cu;
        let m = &self.st.maps;
        let x64 = (x0 >> 6) << 6;
        let y64 = (y0 >> 6) << 6;
        let y32 = (y0 >> 5) << 5;
        let i64_ = m.idx(x64, y64);
        let i32_ = m.idx(x64, y32);
        let d6 = self.ctb_log2 - 6;
        let w1 = 1u32 << m.log2_w[1][i64_];
        let h1 = 1u32 << m.log2_h[1][i64_];
        let cqt1 = u32::from(m.cqt_depth[1][i64_]);
        let on = (w1 == 64 && h1 == 64)
            || (cqt1 == d6
                && m.mtt[0][i64_] == SPLIT_BT_HOR
                && (1u32 << m.log2_w[1][i32_]) == 64
                && (1u32 << m.log2_h[1][i32_]) == 32)
            || cqt1 > d6
            || (cqt1 == d6 && m.mtt[0][i64_] == SPLIT_BT_HOR && m.mtt[1][i32_] == SPLIT_BT_VER);
        if !on {
            return false;
        }
        let w0 = 1u32 << m.log2_w[0][i64_];
        let h0 = 1u32 << m.log2_h[0][i64_];
        let cqt0 = u32::from(m.cqt_depth[0][i64_]);
        !((w0 == 64 && h0 == 64 && m.isp[i64_]) || ((w0 < 64 || h0 < 64) && cqt0 == d6))
    }

    /// IntraPredModeC (8.4.3).
    fn chroma_mode(&self, cu: &mut Cu, cclm: bool, cclm_idx: u32, icpm: u32) {
        let sps = self.c.sps;
        let single = cu.tree == TreeType::Single;
        cu.mip_chroma_direct =
            single && sps.chroma_format_idc == 3 && (icpm == 4 || cu.act) && cu.mip;
        if cu.mip_chroma_direct {
            cu.mode_c = cu.mode_y;
            return;
        }
        let (xc, yc) = (cu.x0 + cu.w / 2, cu.y0 + cu.h / 2);
        let luma = if single {
            if cu.mip {
                if sps.chroma_format_idc == 3 { -1 } else { 0 }
            } else if cu.pred == MODE_IBC || cu.pred == MODE_PLT {
                1
            } else {
                cu.mode_y
            }
        } else {
            let m = &self.st.maps;
            let i = m.idx(xc, yc);
            if m.mip[i] {
                0
            } else if m.pred[0][i] == MODE_IBC || m.pred[0][i] == MODE_PLT {
                1
            } else {
                i32::from(m.mode_y[i])
            }
        };
        let mut mode = if cu.act {
            luma
        } else if cclm {
            81 + cclm_idx as i32
        } else if icpm == 4 {
            luma
        } else {
            let base = [0, 50, 18, 1][icpm as usize];
            if luma == base { 66 } else { base }
        };
        if sps.chroma_format_idc == 2 && (0..=66).contains(&mode) {
            const MAP422: [u8; 67] = [
                0, 1, 61, 62, 63, 64, 65, 66, 2, 3, 5, 6, 8, 10, 12, 13, 14, 16, 18, 20, 22, 23,
                24, 26, 28, 30, 31, 33, 34, 35, 36, 37, 38, 39, 40, 41, 41, 42, 43, 43, 44, 44, 45,
                45, 46, 47, 48, 48, 49, 49, 50, 51, 51, 52, 52, 53, 54, 55, 55, 56, 56, 57, 57, 58,
                59, 59, 60,
            ];
            mode = i32::from(MAP422[mode as usize]);
        }
        cu.mode_c = mode;
    }

    fn store_cu_maps(&mut self, cu: &Cu, skip: bool) {
        let ch = usize::from(cu.tree == TreeType::DualChroma);
        let single = cu.tree == TreeType::Single;
        let m = &mut self.st.maps;
        let lw = cu.w.trailing_zeros() as u8;
        let lh = cu.h.trailing_zeros() as u8;
        let (x0, y0, w, h) = (cu.x0, cu.y0, cu.w, cu.h);
        let (w4, h4) = (m.w4, m.h4);
        let x1 = ((x0 + w) as usize).div_ceil(4).min(w4);
        let y1 = ((y0 + h) as usize).div_ceil(4).min(h4);
        for yy in (y0 as usize >> 2)..y1 {
            for xx in (x0 as usize >> 2)..x1 {
                let i = yy * w4 + xx;
                for c in 0..2 {
                    if single || c == ch {
                        m.pred[c][i] = cu.pred;
                        m.cqt_depth[c][i] = cu.cqt as u8;
                        m.log2_w[c][i] = lw;
                        m.log2_h[c][i] = lh;
                    }
                }
                if ch == 0 {
                    m.skip[i] = skip;
                    m.mip[i] = cu.mip;
                    m.isp[i] = cu.isp != ISP_NONE;
                    m.mode_y[i] = cu.mode_y as u8;
                    m.cu_pos[i] = (x0 as u16, y0 as u16);
                }
            }
        }
    }

    pub(super) fn transform_tree(
        &mut self,
        e: &mut Engine<'_>,
        cu: &mut Cu,
        x0: u32,
        y0: u32,
        w: u32,
        h: u32,
    ) -> Result<()> {
        let max_tb = 1u32 << self.max_tb_log2;
        if cu.isp == ISP_NONE {
            if w > max_tb || h > max_tb {
                let ver = w > max_tb && w > h;
                let (tw, th) = if ver { (w / 2, h) } else { (w, h / 2) };
                self.transform_tree(e, cu, x0, y0, tw, th)?;
                if ver {
                    self.transform_tree(e, cu, x0 + tw, y0, tw, th)
                } else {
                    self.transform_tree(e, cu, x0, y0 + th, tw, th)
                }
            } else {
                self.transform_unit(e, cu, x0, y0, w, h, 0)
            }
        } else if cu.isp == ISP_HOR {
            let th = h / cu.isp_parts;
            for p in 0..cu.isp_parts {
                self.transform_unit(e, cu, x0, y0 + th * p, w, th, p)?;
            }
            Ok(())
        } else {
            let tw = w / cu.isp_parts;
            for p in 0..cu.isp_parts {
                self.transform_unit(e, cu, x0 + tw * p, y0, tw, h, p)?;
            }
            Ok(())
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transform_unit(
        &mut self,
        e: &mut Engine<'_>,
        cu: &mut Cu,
        x0: u32,
        y0: u32,
        w: u32,
        h: u32,
        sub: u32,
    ) -> Result<()> {
        let sps = self.c.sps;
        let pps = self.c.pps;
        let tree = cu.tree;
        let isp = cu.isp != ISP_NONE;
        let last_sub = sub + 1 == cu.isp_parts;
        let (xc, yc, wc, hc) = if isp && tree == TreeType::Single && last_sub {
            (
                cu.x0 >> self.sw,
                cu.y0 >> self.sh,
                cu.w >> self.sw,
                cu.h >> self.sh,
            )
        } else {
            (x0 >> self.sw, y0 >> self.sh, w >> self.sw, h >> self.sh)
        };
        let chroma_avail = tree != TreeType::DualLuma && self.chroma && (!isp || last_sub);
        let mut tu = Tu {
            x0,
            y0,
            w,
            h,
            xc,
            yc,
            wc,
            hc,
            has_luma: tree != TreeType::DualChroma,
            has_chroma: chroma_avail,
            ..Tu::default()
        };
        if (tree == TreeType::Single || tree == TreeType::DualChroma)
            && self.chroma
            && (!isp || last_sub)
        {
            let inc = usize::from(cu.bdpcm[1]);
            tu.cbf[1] = e.decode(&mut self.ctx.0[cabac::TU_CB_CODED_FLAG + inc]) == 1;
            let inc = if cu.bdpcm[1] {
                2
            } else {
                usize::from(tu.cbf[1])
            };
            tu.cbf[2] = e.decode(&mut self.ctx.0[cabac::TU_CR_CODED_FLAG + inc]) == 1;
        }
        let max_tb = 1u32 << self.max_tb_log2;
        if tree != TreeType::DualChroma {
            let coded = if !isp {
                (cu.pred == MODE_INTRA && !cu.act)
                    || (chroma_avail && (tu.cbf[1] || tu.cbf[2]))
                    || cu.w > max_tb
                    || cu.h > max_tb
            } else {
                sub + 1 < cu.isp_parts || !self.scratch.infer_cbf_luma
            };
            if sub == 0 {
                self.scratch.infer_cbf_luma = true;
                self.scratch.prev_cbf_y = false;
            }
            if coded {
                let inc = if cu.bdpcm[0] {
                    1
                } else if !isp {
                    0
                } else {
                    2 + usize::from(self.scratch.prev_cbf_y)
                };
                tu.cbf[0] = e.decode(&mut self.ctx.0[cabac::TU_Y_CODED_FLAG + inc]) == 1;
            } else {
                tu.cbf[0] = true;
            }
            if isp {
                self.scratch.infer_cbf_luma = self.scratch.infer_cbf_luma && !tu.cbf[0];
                self.scratch.prev_cbf_y = tu.cbf[0];
            }
        }
        let big = cu.w > 64 || cu.h > 64;
        let ch_coded = chroma_avail && (tu.cbf[1] || tu.cbf[2]);
        if (big || tu.cbf[0] || ch_coded)
            && tree != TreeType::DualChroma
            && pps.cu_qp_delta_enabled
            && !self.is_cu_qp_delta_coded
        {
            self.cu_qp_delta(e)?;
        }
        if (big || ch_coded)
            && tree != TreeType::DualLuma
            && self.c.sh.cu_chroma_qp_offset_enabled
            && !self.is_cu_chroma_qp_offset_coded
        {
            self.cu_chroma_qp_offset(e);
        }
        if sps.joint_cbcr
            && ((cu.pred == MODE_INTRA && (tu.cbf[1] || tu.cbf[2])) || (tu.cbf[1] && tu.cbf[2]))
            && chroma_avail
        {
            let inc = 2 * usize::from(tu.cbf[1]) + usize::from(tu.cbf[2]) - 1;
            tu.joint = e.decode(&mut self.ctx.0[cabac::TU_JOINT_CBCR_RESIDUAL_FLAG + inc]) == 1;
        }
        if tu.joint {
            tu.res_mode = match (tu.cbf[1], tu.cbf[2]) {
                (true, false) => 1,
                (true, true) => 2,
                _ => 3,
            };
        }
        let ts_disabled = self.c.sh.ts_residual_coding_disabled;
        if tu.cbf[0] && tree != TreeType::DualChroma {
            if sps.transform_skip && !cu.bdpcm[0] && w <= self.max_ts && h <= self.max_ts && !isp {
                tu.ts[0] = e.decode(&mut self.ctx.0[cabac::TRANSFORM_SKIP_FLAG]) == 1;
            }
            if cu.bdpcm[0] {
                tu.ts[0] = true;
            }
            let mut buf = vec![0i32; (w * h) as usize];
            if !tu.ts[0] || ts_disabled {
                self.residual_coding(e, cu, &tu, w, h, 0, &mut buf)?;
            } else {
                self.residual_ts_coding(e, cu, w, h, 0, &mut buf)?;
            }
            tu.coeff[0] = buf;
        }
        for ci in 1..3 {
            if !tu.cbf[ci] || tree == TreeType::DualLuma || !chroma_avail {
                continue;
            }
            if ci == 2 && tu.cbf[1] && tu.joint {
                continue;
            }
            if sps.transform_skip && !cu.bdpcm[1] && wc <= self.max_ts && hc <= self.max_ts {
                tu.ts[ci] = e.decode(&mut self.ctx.0[cabac::TRANSFORM_SKIP_FLAG + 1]) == 1;
            }
            if cu.bdpcm[1] {
                tu.ts[ci] = true;
            }
            let mut buf = vec![0i32; (wc * hc) as usize];
            if !tu.ts[ci] || ts_disabled {
                self.residual_coding(e, cu, &tu, wc, hc, ci, &mut buf)?;
            } else {
                self.residual_ts_coding(e, cu, wc, hc, ci, &mut buf)?;
            }
            tu.coeff[ci] = buf;
        }
        cu.tus.push(tu);
        Ok(())
    }

    pub(super) fn cu_qp_delta(&mut self, e: &mut Engine<'_>) -> Result<()> {
        let mut v = 0u32;
        if e.decode(&mut self.ctx.0[cabac::CU_QP_DELTA_ABS]) == 1 {
            v = 1;
            while v < 5 && e.decode(&mut self.ctx.0[cabac::CU_QP_DELTA_ABS + 1]) == 1 {
                v += 1;
            }
            if v == 5 {
                // EG0 suffix.
                let mut k = 0;
                while e.bypass() == 1 {
                    k += 1;
                    if k > 31 {
                        return Err(Error::InvalidData("cu_qp_delta_abs too large"));
                    }
                }
                let suf = ((1u32 << k) - 1) + e.bypass_bits(k);
                v += suf;
            }
        }
        let mut d = v as i32;
        if v > 0 && e.bypass() == 1 {
            d = -d;
        }
        let qbd = self.c.sps.qp_bd_offset();
        if d < -(32 + qbd / 2) || d > 31 + qbd / 2 {
            return Err(Error::InvalidData("CuQpDeltaVal out of range"));
        }
        self.is_cu_qp_delta_coded = true;
        self.cu_qp_delta_val = d;
        Ok(())
    }

    pub(super) fn cu_chroma_qp_offset(&mut self, e: &mut Engine<'_>) {
        let pps = self.c.pps;
        let flag = e.decode(&mut self.ctx.0[cabac::CU_CHROMA_QP_OFFSET_FLAG]) == 1;
        let mut idx = 0usize;
        let n = pps.chroma_qp_offset_list.len();
        if flag && n > 1 {
            while idx < n - 1 && e.decode(&mut self.ctx.0[cabac::CU_CHROMA_QP_OFFSET_IDX]) == 1 {
                idx += 1;
            }
        }
        self.is_cu_chroma_qp_offset_coded = true;
        self.cu_qp_offset = if flag {
            pps.chroma_qp_offset_list[idx]
        } else {
            [0; 3]
        };
    }

    /// Quantisation parameters of a coding unit (8.7.1).
    pub(super) fn cu_qp(&mut self, cu: &mut Cu) {
        let sps = self.c.sps;
        let pps = self.c.pps;
        let sh = self.c.sh;
        let qbd = sps.qp_bd_offset();
        let qp_y = if cu.tree != TreeType::DualChroma {
            let (xq, yq) = self.qg;
            let prev = self.qp_prev_qg;
            let l = self.ctb_log2;
            let m = &self.st.maps;
            let qa = if self.available(cu.x0, cu.y0, xq as i32 - 1, yq as i32, 0)
                && ((xq - 1) >> l) == (cu.x0 >> l)
                && (yq >> l) == (cu.y0 >> l)
            {
                i32::from(m.qp_y[m.idx(xq - 1, yq)])
            } else {
                prev
            };
            let avail_b = self.available(cu.x0, cu.y0, xq as i32, yq as i32 - 1, 0);
            let qb = if avail_b && (xq >> l) == (cu.x0 >> l) && ((yq - 1) >> l) == (cu.y0 >> l) {
                i32::from(m.qp_y[m.idx(xq, yq - 1)])
            } else {
                prev
            };
            let first_in_row = xq == self.tile_x0 && (yq & ((1 << l) - 1)) == 0;
            let pred = if avail_b && first_in_row {
                i32::from(m.qp_y[m.idx(xq, yq - 1)])
            } else {
                (qa + qb + 1) >> 1
            };
            let q = ((pred + self.cu_qp_delta_val + 64 + 2 * qbd) % (64 + qbd)) - qbd;
            self.last_qp_y = q;
            let m = &mut self.st.maps;
            let (x0, y0, w, h) = (cu.x0, cu.y0, cu.w, cu.h);
            for i in m.cells(x0, y0, w, h) {
                m.qp_y[i] = q as i8;
            }
            q
        } else {
            let m = &self.st.maps;
            i32::from(m.qp_y[m.idx(cu.x0 + cu.w / 2, cu.y0 + cu.h / 2)])
        };
        cu.qp_y = qp_y;
        cu.qp[0] = qp_y + qbd;
        if self.chroma && cu.tree != TreeType::DualLuma {
            let qc = qp_y.clamp(-qbd, 63);
            let o = self.cu_qp_offset;
            let f = |t: usize, off: i32| (sps.chroma_qp(t, qc) + off).clamp(-qbd, 63) + qbd;
            cu.qp[1] = f(0, pps.cb_qp_offset + sh.cb_qp_offset + o[0]);
            cu.qp[2] = f(1, pps.cr_qp_offset + sh.cr_qp_offset + o[1]);
            cu.qp[3] = f(2, pps.joint_cbcr_qp_offset + sh.joint_cbcr_qp_offset + o[2]);
        }
    }
}
