//! Slice data: CTUs, coding trees, coding units and transform units (7.3.11), parsed and
//! reconstructed together, one coding unit at a time.

mod cu;
mod ibc;
mod palette;
mod recon;
mod residual;

use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::cabac::{self, Contexts, Engine};
use crate::error::{Error, Result};
use crate::filter::{self, AlfCtb, SaoCtb};
use crate::lmcs::Lmcs;
use crate::picture::Picture;
use crate::ps::alf::AlfAps;
use crate::ps::aps::ScalingAps;
use crate::ps::ph::{PictureHeader, SliceHeader, SliceType};
use crate::ps::pps::{Layout, Pps};
use crate::ps::sps::Sps;

/// The RBSP of a slice and the byte ranges of its substreams.
pub struct SliceData {
    pub rbsp: Vec<u8>,
    /// Substream k is `rbsp[bounds[k]..bounds[k + 1]]` (when entry points are signalled; the
    /// first bound is the start of the slice data either way).
    pub bounds: Vec<usize>,
    pub has_entry_points: bool,
}

/// Everything a slice is decoded with.
pub struct SliceCtx<'a> {
    pub sps: &'a Sps,
    pub pps: &'a Pps,
    pub layout: &'a Layout,
    pub ph: &'a PictureHeader,
    pub sh: &'a SliceHeader,
    pub slice_idx: u32,
    pub lmcs: Option<&'a Lmcs>,
    pub scaling: Option<&'a ScalingAps>,
    pub alf_aps: &'a [Option<Arc<AlfAps>>; 8],
    pub cancel: Option<&'a AtomicBool>,
}

/// CuPredMode values in the maps.
pub const MODE_INTRA: u8 = 0;
#[allow(dead_code)]
pub const MODE_INTER: u8 = 1;
pub const MODE_IBC: u8 = 2;
pub const MODE_PLT: u8 = 3;
pub const MODE_NONE: u8 = 255;

/// Per 4x4 luma block information shared by the slices of a picture.
pub struct Maps {
    pub w4: usize,
    pub h4: usize,
    /// IsAvailable per colour component.
    pub avail: [Vec<bool>; 3],
    /// CuPredMode per channel type.
    pub pred: [Vec<u8>; 2],
    pub cqt_depth: [Vec<u8>; 2],
    /// Log2 of CbWidth / CbHeight per channel type (luma samples).
    pub log2_w: [Vec<u8>; 2],
    pub log2_h: [Vec<u8>; 2],
    /// MttSplitMode at multi-type tree depths 0 and 1 (0 none, 1 BT_HOR, 2 BT_VER, 3 TT_HOR,
    /// 4 TT_VER).
    pub mtt: [Vec<u8>; 2],
    pub skip: Vec<bool>,
    pub mip: Vec<bool>,
    pub isp: Vec<bool>,
    /// IntraPredModeY.
    pub mode_y: Vec<u8>,
    /// QpY of the luma coding unit.
    pub qp_y: Vec<i8>,
    /// Top-left of the luma coding unit (for chroma residual scaling).
    pub cu_pos: Vec<(u16, u16)>,
    /// Block vector of IBC coding units (1/16 sample).
    pub bv: Vec<(i32, i32)>,
}

impl Maps {
    fn new(w: u32, h: u32) -> Maps {
        let w4 = w.div_ceil(4) as usize;
        let h4 = h.div_ceil(4) as usize;
        let n = w4 * h4;
        Maps {
            w4,
            h4,
            avail: [vec![false; n], vec![false; n], vec![false; n]],
            pred: [vec![MODE_NONE; n], vec![MODE_NONE; n]],
            cqt_depth: [vec![0; n], vec![0; n]],
            log2_w: [vec![0; n], vec![0; n]],
            log2_h: [vec![0; n], vec![0; n]],
            mtt: [vec![0; n], vec![0; n]],
            skip: vec![false; n],
            mip: vec![false; n],
            isp: vec![false; n],
            mode_y: vec![0; n],
            qp_y: vec![0; n],
            cu_pos: vec![(0, 0); n],
            bv: vec![(0, 0); n],
        }
    }

    #[inline]
    pub fn idx(&self, x: u32, y: u32) -> usize {
        (y as usize >> 2) * self.w4 + (x as usize >> 2)
    }

    /// The 4x4 indices of a luma rectangle (clipped to the picture).
    #[inline]
    pub fn cells(&self, x: u32, y: u32, w: u32, h: u32) -> impl Iterator<Item = usize> + use<> {
        let w4 = self.w4;
        let x0 = x as usize >> 2;
        let y0 = y as usize >> 2;
        let x1 = ((x + w) as usize).div_ceil(4).min(self.w4);
        let y1 = ((y + h) as usize).div_ceil(4).min(self.h4);
        (y0..y1).flat_map(move |yy| (x0..x1).map(move |xx| yy * w4 + xx))
    }
}

/// The picture under reconstruction and the per-block state its slices share.
pub struct PicState {
    pub pic: Picture,
    pub meta: filter::Meta,
    pub sao: Vec<SaoCtb>,
    pub alf: Vec<AlfCtb>,
    pub ctb_slice: Vec<u32>,
    pub ctb_done: Vec<bool>,
    /// Slice index + 1 of each CTB once its slice started (0: not yet).
    pub ctb_slice_started: Vec<u32>,
    pub maps: Maps,
}

impl PicState {
    pub fn new(sps: &Sps, pps: &Pps, layout: &Layout, pic: Picture) -> Result<PicState> {
        let n = (layout.w_ctbs * layout.h_ctbs) as usize;
        Ok(PicState {
            pic,
            meta: filter::Meta::new(sps, pps),
            sao: vec![SaoCtb::default(); n],
            alf: vec![AlfCtb::default(); n],
            ctb_slice: vec![0; n],
            ctb_done: vec![false; n],
            ctb_slice_started: vec![0; n],
            maps: Maps::new(pps.width, pps.height),
        })
    }

    pub fn undecoded(&self) -> usize {
        self.ctb_done.iter().filter(|d| !**d).count()
    }

    pub fn undecoded_in(&self, ctbs: &[u32]) -> usize {
        ctbs.iter()
            .filter(|&&c| !self.ctb_done.get(c as usize).copied().unwrap_or(true))
            .count()
    }
}

/// Tree types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeType {
    Single,
    DualLuma,
    DualChroma,
}

/// Mode types of the coding tree (local dual tree constraints).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeType {
    All,
    Intra,
    Inter,
}

pub const SPLIT_NONE: u8 = 0;
pub const SPLIT_BT_HOR: u8 = 1;
pub const SPLIT_BT_VER: u8 = 2;
pub const SPLIT_TT_HOR: u8 = 3;
pub const SPLIT_TT_VER: u8 = 4;

/// Palette predictor state (7.4.12.6, 8.4.5.3).
#[derive(Clone, Default)]
pub struct PalettePred {
    pub entries: [Vec<u16>; 3],
    pub size: [usize; 2],
}

/// The decoder of one slice.
pub struct Dec<'a> {
    pub c: &'a SliceCtx<'a>,
    pub st: &'a mut PicState,
    pub ctx: Contexts,
    pub wpp_ctx: Option<Contexts>,
    pub wpp_pal: Option<PalettePred>,
    pub pal: PalettePred,
    // Constants.
    pub ctb_log2: u32,
    pub pic_w: u32,
    pub pic_h: u32,
    pub sw: u32,
    pub sh: u32,
    pub chroma: bool,
    pub bd: u32,
    pub min_cb_log2: u32,
    pub min_qt: [u32; 2],
    pub max_bt: [u32; 2],
    pub max_tt: [u32; 2],
    pub max_mtt: [u32; 2],
    pub max_tb_log2: u32,
    pub max_ts: u32,
    pub cu_qp_delta_subdiv: u32,
    pub cu_chroma_qp_offset_subdiv: u32,
    pub intra_slice: bool,
    pub dual_tree: bool,
    // Quantisation group state.
    pub qg: (u32, u32),
    pub is_cu_qp_delta_coded: bool,
    pub cu_qp_delta_val: i32,
    pub is_cu_chroma_qp_offset_coded: bool,
    pub cu_qp_offset: [i32; 3],
    pub qp_prev_qg: i32,
    pub last_qp_y: i32,
    // CTU state.
    pub ctb: (u32, u32),
    pub first_ctb_row_in_slice: bool,
    /// Tile column start (luma) of the current CTU.
    pub tile_x0: u32,
    /// Scratch for one coding unit.
    pub scratch: recon::Scratch,
    pub ibc: ibc::IbcState,
    pub ibc_bv: (i32, i32),
}

/// Decodes one slice into `st`.
pub fn decode_slice(c: &SliceCtx<'_>, data: &SliceData, st: &mut PicState) -> Result<()> {
    let sps = c.sps;
    let sh = c.sh;
    if sh.slice_type != SliceType::I {
        return Err(Error::Unsupported(
            "inter-coded pictures (P and B slices); only intra pictures decode",
        ));
    }
    for &ctb in &sh.ctbs {
        let i = ctb as usize;
        if i >= st.ctb_slice.len() {
            return Err(Error::InvalidData("slice CTB outside the picture"));
        }
        st.ctb_slice[i] = c.slice_idx;
        st.ctb_slice_started[i] = c.slice_idx + 1;
    }
    let ph = c.ph;
    let intra = true;
    let pc = &ph.pc;
    let min_cb_log2 = sps.log2_min_cb_size;
    let (li, ci) = if intra { (0, 1) } else { (2, 2) };
    let min_qt_y = min_cb_log2 + pc.log2_diff_min_qt_min_cb[li];
    let min_qt_c = min_cb_log2 + pc.log2_diff_min_qt_min_cb[ci];
    let mut d = Dec {
        c,
        ctx: Contexts::new(sh.init_type(), sh.qp_y),
        wpp_ctx: None,
        wpp_pal: None,
        pal: PalettePred::default(),
        ctb_log2: sps.log2_ctu_size,
        pic_w: c.pps.width,
        pic_h: c.pps.height,
        sw: if matches!(sps.chroma_format_idc, 1 | 2) {
            1
        } else {
            0
        },
        sh: if sps.chroma_format_idc == 1 { 1 } else { 0 },
        chroma: sps.chroma_format_idc != 0,
        bd: sps.bit_depth,
        min_cb_log2,
        min_qt: [1 << min_qt_y, 1 << min_qt_c],
        max_bt: [
            1 << (min_qt_y + pc.log2_diff_max_bt_min_qt[li]),
            1 << (min_qt_c + pc.log2_diff_max_bt_min_qt[ci]),
        ],
        max_tt: [
            1 << (min_qt_y + pc.log2_diff_max_tt_min_qt[li]),
            1 << (min_qt_c + pc.log2_diff_max_tt_min_qt[ci]),
        ],
        max_mtt: [pc.max_mtt_depth[li], pc.max_mtt_depth[ci]],
        max_tb_log2: if sps.max_luma_tb_64 { 6 } else { 5 },
        max_ts: 1 << sps.log2_ts_max_size,
        cu_qp_delta_subdiv: ph.cu_qp_delta_subdiv[if intra { 0 } else { 1 }],
        cu_chroma_qp_offset_subdiv: ph.cu_chroma_qp_offset_subdiv[if intra { 0 } else { 1 }],
        intra_slice: sh.slice_type == SliceType::I,
        dual_tree: sh.slice_type == SliceType::I && sps.qtbtt_dual_tree_intra,
        qg: (0, 0),
        is_cu_qp_delta_coded: false,
        cu_qp_delta_val: 0,
        is_cu_chroma_qp_offset_coded: false,
        cu_qp_offset: [0; 3],
        qp_prev_qg: sh.qp_y,
        last_qp_y: sh.qp_y,
        ctb: (0, 0),
        first_ctb_row_in_slice: true,
        tile_x0: 0,
        scratch: recon::Scratch::default(),
        ibc: ibc::IbcState::default(),
        ibc_bv: (0, 0),
        st,
    };
    if sps.ibc {
        d.ibc_init();
    }
    d.run(data)
}

impl<'a> Dec<'a> {
    fn run(&mut self, data: &SliceData) -> Result<()> {
        let c = self.c;
        let layout = c.layout;
        let sps = c.sps;
        let ctbs = &c.sh.ctbs;
        let mut sub = 0usize;
        let start = data.bounds[0];
        let end_of = |k: usize| -> usize {
            if data.has_entry_points {
                data.bounds[k + 1]
            } else {
                data.rbsp.len()
            }
        };
        let mut pos = start;
        let mut e = Engine::new(&data.rbsp[pos..end_of(0)]);
        for (i, &addr) in ctbs.iter().enumerate() {
            let cx = addr % layout.w_ctbs;
            let cy = addr / layout.w_ctbs;
            let tcol = layout.ctb_to_col[cx as usize] as usize;
            let trow = layout.ctb_to_row[cy as usize] as usize;
            let tile_x0 = layout.col_bd[tcol];
            let tile_y0 = layout.row_bd[trow];
            self.tile_x0 = tile_x0 << self.ctb_log2;
            if i > 0 {
                let p = ctbs[i - 1];
                let (px, py) = (p % layout.w_ctbs, p / layout.w_ctbs);
                let new_tile = layout.ctb_to_col[px as usize] as usize != tcol
                    || layout.ctb_to_row[py as usize] as usize != trow;
                let new_row = cy != py && sps.entropy_coding_sync;
                if new_tile || new_row {
                    // A new substream: the next entry point, or where the last one ended.
                    sub += 1;
                    if data.has_entry_points {
                        if sub + 1 >= data.bounds.len() {
                            return Err(Error::InvalidData("fewer entry points than substreams"));
                        }
                        pos = data.bounds[sub];
                    } else {
                        pos += e.position();
                    }
                    if pos > data.rbsp.len() {
                        return Err(Error::Truncated);
                    }
                    e = Engine::new(&data.rbsp[pos..end_of(sub).max(pos)]);
                    if new_tile {
                        self.ctx = Contexts::new(c.sh.init_type(), c.sh.qp_y);
                        self.pal = PalettePred::default();
                        self.last_qp_y = c.sh.qp_y;
                    } else {
                        // Wavefront: synchronise from the CTU above when it is available.
                        let x0 = cx << self.ctb_log2;
                        let y0 = cy << self.ctb_log2;
                        if self.available(x0, y0, x0 as i32, y0 as i32 - (1 << self.ctb_log2), 0)
                            && let Some(s) = &self.wpp_ctx
                        {
                            self.ctx = s.clone();
                            self.pal = self.wpp_pal.clone().unwrap_or_default();
                        } else {
                            self.ctx = Contexts::new(c.sh.init_type(), c.sh.qp_y);
                            self.pal = PalettePred::default();
                        }
                    }
                }
                let _ = tile_y0;
            }
            if c.cancel.is_some_and(|f| f.load(Ordering::Relaxed)) && cx == tile_x0 {
                return Err(Error::Cancelled);
            }
            if cx == tile_x0 {
                // NumHmvpIbcCand = 0 at the start of each CTU row of a tile.
                self.ibc.hmvp.clear();
            }
            self.ctb = (cx, cy);
            self.ctu(&mut e)?;
            self.st.ctb_done[addr as usize] = true;
            if sps.entropy_coding_sync && cx == tile_x0 {
                self.wpp_ctx = Some(self.ctx.clone());
                self.wpp_pal = Some(self.pal.clone());
            }
            if e.overrun() {
                return Err(Error::Truncated);
            }
            // end_of_slice_one_bit / end_of_tile_one_bit / end_of_subset_one_bit.
            let last = i + 1 == ctbs.len();
            let tile_end_x = cx + 1 == layout.col_bd[tcol + 1];
            let tile_end_y = cy + 1 == layout.row_bd[trow + 1];
            if (last || (tile_end_x && (tile_end_y || sps.entropy_coding_sync)))
                && e.terminate() != 1
            {
                return Err(Error::InvalidData(
                    "missing end of slice, tile or subset bit",
                ));
            }
            if !last && tile_end_x {
                self.first_ctb_row_in_slice = false;
            }
        }
        Ok(())
    }

    /// Neighbouring block availability (6.4.4) without the prediction mode check.
    pub fn available(&self, xc: u32, yc: u32, xn: i32, yn: i32, c: usize) -> bool {
        if xn < 0 || yn < 0 || xn as u32 >= self.pic_w || yn as u32 >= self.pic_h {
            return false;
        }
        let (xn, yn) = (xn as u32, yn as u32);
        let l = self.ctb_log2;
        let (cxc, cyc) = (xc >> l, yc >> l);
        let (cxn, cyn) = (xn >> l, yn >> l);
        if cxn > cxc && cyn >= cyc {
            return false;
        }
        if cyn > cyc {
            return false;
        }
        if !self.st.maps.avail[c.min(2)][self.st.maps.idx(xn, yn)] {
            return false;
        }
        let layout = self.c.layout;
        let an = (cyn * layout.w_ctbs + cxn) as usize;
        let ac = (cyc * layout.w_ctbs + cxc) as usize;
        if an != ac {
            if self.st.ctb_slice_started[an] != self.st.ctb_slice_started[ac] {
                return false;
            }
            if layout.tile_of(cxn, cyn) != layout.tile_of(cxc, cyc) {
                return false;
            }
            if self.c.sps.entropy_coding_sync && cxn > cxc {
                return false;
            }
        }
        true
    }

    /// Whether CTB (x, y) (in CTBs) is in the current slice and tile, for CTU-level contexts.
    fn ctb_avail(&self, x: i32, y: i32) -> bool {
        let layout = self.c.layout;
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as u32, y as u32);
        let a = (y * layout.w_ctbs + x) as usize;
        let cur = (self.ctb.1 * layout.w_ctbs + self.ctb.0) as usize;
        self.st.ctb_slice_started[a] == self.st.ctb_slice_started[cur]
            && self.st.ctb_done[a]
            && layout.tile_of(x, y) == layout.tile_of(self.ctb.0, self.ctb.1)
    }

    fn ctu(&mut self, e: &mut Engine<'_>) -> Result<()> {
        let c = self.c;
        let sh = c.sh;
        let (cx, cy) = self.ctb;
        let layout = c.layout;
        let addr = (cy * layout.w_ctbs + cx) as usize;
        if sh.sao_luma || sh.sao_chroma {
            self.sao(e, cx, cy)?;
        }
        if sh.alf.enabled {
            self.alf_ctb(e, addr)?;
        }
        let x0 = cx << self.ctb_log2;
        let y0 = cy << self.ctb_log2;
        let size = 1u32 << self.ctb_log2;
        if self.dual_tree {
            self.dual_tree_implicit_qt_split(e, x0, y0, size, 0)
        } else {
            self.coding_tree(
                e,
                CtArgs {
                    x0,
                    y0,
                    w: size,
                    h: size,
                    qg_y: true,
                    qg_c: true,
                    subdiv: 0,
                    cqt: 0,
                    mtt: 0,
                    depth_offset: 0,
                    part: 0,
                    tree: TreeType::Single,
                    mode: ModeType::All,
                    parent_split: SPLIT_NONE,
                },
            )
        }
    }

    fn sao(&mut self, e: &mut Engine<'_>, rx: u32, ry: u32) -> Result<()> {
        let c = self.c;
        let layout = c.layout;
        let addr = (ry * layout.w_ctbs + rx) as usize;
        let mut merge_left = false;
        let mut merge_up = false;
        if rx > 0 {
            let left_avail = rx != layout.col_bd[layout.ctb_to_col[rx as usize] as usize];
            if left_avail {
                merge_left = e.decode(&mut self.ctx.0[cabac::SAO_MERGE_FLAG]) == 1;
            }
        }
        if ry > 0 && !merge_left {
            let up_avail = ry != layout.row_bd[layout.ctb_to_row[ry as usize] as usize]
                && !self.first_ctb_row_in_slice;
            if up_avail {
                merge_up = e.decode(&mut self.ctx.0[cabac::SAO_MERGE_FLAG]) == 1;
            }
        }
        if merge_left {
            self.st.sao[addr] = self.st.sao[addr - 1];
            return Ok(());
        }
        if merge_up {
            self.st.sao[addr] = self.st.sao[addr - layout.w_ctbs as usize];
            return Ok(());
        }
        let mut s = SaoCtb::default();
        let ncomp = if self.chroma { 3 } else { 1 };
        let shift = self.bd - self.bd.min(10);
        let cmax = (1u32 << (self.bd.min(10) - 5)) - 1;
        for ci in 0..ncomp {
            if !((c.sh.sao_luma && ci == 0) || (c.sh.sao_chroma && ci > 0)) {
                continue;
            }
            if ci == 2 {
                s.type_idx[2] = s.type_idx[1];
                s.eo_class[2] = s.eo_class[1];
            } else {
                // sao_type_idx: TR cMax 2, bin 0 context, bin 1 bypass.
                let t = if e.decode(&mut self.ctx.0[cabac::SAO_TYPE_IDX]) == 0 {
                    0
                } else if e.bypass() == 0 {
                    1
                } else {
                    2
                };
                s.type_idx[ci] = t;
            }
            if s.type_idx[ci] == 0 {
                continue;
            }
            let mut abs = [0u32; 4];
            for a in abs.iter_mut() {
                let mut v = 0;
                while v < cmax && e.bypass() == 1 {
                    v += 1;
                }
                *a = v;
            }
            let mut sign = [false; 4];
            if s.type_idx[ci] == 1 {
                for i in 0..4 {
                    if abs[i] != 0 {
                        sign[i] = e.bypass() == 1;
                    }
                }
                s.band_position[ci] = e.bypass_bits(5) as u8;
            } else {
                sign = [false, false, true, true];
                if ci == 0 {
                    s.eo_class[0] = e.bypass_bits(2) as u8;
                }
                if ci == 1 {
                    s.eo_class[1] = e.bypass_bits(2) as u8;
                }
            }
            for i in 0..4 {
                let v = (abs[i] << shift) as i16;
                s.offset[ci][i] = if sign[i] { -v } else { v };
            }
        }
        self.st.sao[addr] = s;
        Ok(())
    }

    fn alf_ctb(&mut self, e: &mut Engine<'_>, addr: usize) -> Result<()> {
        let c = self.c;
        let a = &c.sh.alf;
        let (cx, cy) = (self.ctb.0 as i32, self.ctb.1 as i32);
        let w = c.layout.w_ctbs as i32;
        let left = self
            .ctb_avail(cx - 1, cy)
            .then(|| (cy * w + cx - 1) as usize);
        let up = self
            .ctb_avail(cx, cy - 1)
            .then(|| ((cy - 1) * w + cx) as usize);
        let mut r = AlfCtb::default();
        let ncomp = if self.chroma { 3 } else { 1 };
        for ci in 0..ncomp {
            let on = match ci {
                0 => true,
                1 => a.cb,
                _ => a.cr,
            };
            if !on {
                continue;
            }
            let cond = |i: Option<usize>| i.is_some_and(|i| self.st.alf[i].enabled[ci]) as usize;
            let inc = cond(left) + cond(up) + 3 * ci;
            r.enabled[ci] = e.decode(&mut self.ctx.0[cabac::ALF_CTB_FLAG + inc]) == 1;
            if ci == 0 && r.enabled[0] {
                let use_aps = if !a.luma_aps.is_empty() {
                    e.decode(&mut self.ctx.0[cabac::ALF_USE_APS_FLAG]) == 1
                } else {
                    false
                };
                r.luma_set = if use_aps {
                    let n = a.luma_aps.len() as u32;
                    16 + if n > 1 { tb_bypass(e, n - 1) } else { 0 } as u8
                } else {
                    tb_bypass(e, 15) as u8
                };
            }
            if ci > 0 && r.enabled[ci] {
                let aps = c.alf_aps[usize::from(a.chroma_aps) & 7]
                    .as_ref()
                    .ok_or(Error::MissingParameterSet("ALF APS"))?;
                let n = aps.chroma_coeff.len() as u32;
                if n > 1 {
                    // TR cMax = n - 1, all bins with context ci - 1.
                    let mut v = 0;
                    while v < n - 1
                        && e.decode(&mut self.ctx.0[cabac::ALF_CTB_FILTER_ALT_IDX + ci - 1]) == 1
                    {
                        v += 1;
                    }
                    r.chroma_alt[ci - 1] = v as u8;
                }
            }
        }
        for k in 0..2 {
            let (on, id) = if k == 0 {
                (a.cc_cb, a.cc_cb_aps)
            } else {
                (a.cc_cr, a.cc_cr_aps)
            };
            if !on {
                continue;
            }
            let aps = c.alf_aps[usize::from(id) & 7]
                .as_ref()
                .ok_or(Error::MissingParameterSet("CC-ALF APS"))?;
            let n = if k == 0 {
                aps.cc_cb_coeff.len()
            } else {
                aps.cc_cr_coeff.len()
            } as u32;
            let cond = |i: Option<usize>| i.is_some_and(|i| self.st.alf[i].cc[k] != 0) as usize;
            let inc = cond(left) + cond(up);
            let base = if k == 0 {
                cabac::ALF_CTB_CC_CB_IDC
            } else {
                cabac::ALF_CTB_CC_CR_IDC
            };
            // TR cMax = n: bin 0 context, the rest bypass.
            let mut v = 0;
            if n > 0 && e.decode(&mut self.ctx.0[base + inc]) == 1 {
                v = 1;
                while v < n && e.bypass() == 1 {
                    v += 1;
                }
            }
            r.cc[k] = v as u8;
        }
        self.st.alf[addr] = r;
        Ok(())
    }

    fn reset_qg(&mut self, x0: u32, y0: u32, subdiv: u32, qg_y: bool, qg_c: bool) {
        let c = self.c;
        if c.pps.cu_qp_delta_enabled && qg_y && subdiv <= self.cu_qp_delta_subdiv {
            self.is_cu_qp_delta_coded = false;
            self.cu_qp_delta_val = 0;
            self.qg = (x0, y0);
            self.qp_prev_qg = self.last_qp_y;
        }
        if c.sh.cu_chroma_qp_offset_enabled && qg_c && subdiv <= self.cu_chroma_qp_offset_subdiv {
            self.is_cu_chroma_qp_offset_coded = false;
            self.cu_qp_offset = [0; 3];
        }
    }

    fn dual_tree_implicit_qt_split(
        &mut self,
        e: &mut Engine<'_>,
        x0: u32,
        y0: u32,
        size: u32,
        cqt: u32,
    ) -> Result<()> {
        let subdiv = 2 * cqt;
        if size > 64 {
            self.reset_qg(x0, y0, subdiv, true, true);
            let half = size / 2;
            let (x1, y1) = (x0 + half, y0 + half);
            self.dual_tree_implicit_qt_split(e, x0, y0, half, cqt + 1)?;
            if x1 < self.pic_w {
                self.dual_tree_implicit_qt_split(e, x1, y0, half, cqt + 1)?;
            }
            if y1 < self.pic_h {
                self.dual_tree_implicit_qt_split(e, x0, y1, half, cqt + 1)?;
            }
            if x1 < self.pic_w && y1 < self.pic_h {
                self.dual_tree_implicit_qt_split(e, x1, y1, half, cqt + 1)?;
            }
            Ok(())
        } else {
            let base = CtArgs {
                x0,
                y0,
                w: size,
                h: size,
                qg_y: true,
                qg_c: false,
                subdiv,
                cqt,
                mtt: 0,
                depth_offset: 0,
                part: 0,
                tree: TreeType::DualLuma,
                mode: ModeType::All,
                parent_split: SPLIT_NONE,
            };
            self.coding_tree(e, base)?;
            self.coding_tree(
                e,
                CtArgs {
                    qg_y: false,
                    qg_c: true,
                    tree: TreeType::DualChroma,
                    ..base
                },
            )
        }
    }

    /// Allowed splits of a coding tree node: (qt, bt_ver, bt_hor, tt_ver, tt_hor).
    fn allowed_splits(&self, a: &CtArgs) -> [bool; 5] {
        let chroma = a.tree == TreeType::DualChroma;
        let ch = usize::from(chroma);
        let (w, h) = (a.w, a.h);
        let sw = 1 << self.sw;
        let shh = 1 << self.sh;
        // 6.4.1.
        let qt = !(((a.tree != TreeType::DualChroma) && w <= self.min_qt[0])
            || (chroma && w <= self.min_qt[1])
            || a.mtt != 0
            || (chroma && w / sw <= 4)
            || (chroma && a.mode == ModeType::Intra));
        let min_qt = self.min_qt[ch];
        let max_bt = self.max_bt[ch];
        let max_tt = self.max_tt[ch];
        let max_mtt = self.max_mtt[ch] + a.depth_offset;
        let min_bt = 1u32 << self.min_cb_log2;
        let (pw, ph) = (self.pic_w, self.pic_h);
        let bt = |ver: bool| -> bool {
            let size = if ver { w } else { h };
            if size <= min_bt
                || w > max_bt
                || h > max_bt
                || a.mtt >= max_mtt
                || (chroma && (w / sw) * (h / shh) <= 16)
                || (chroma && w / sw == 4 && ver)
                || (chroma && a.mode == ModeType::Intra)
                || (w * h == 32 && a.mode == ModeType::Inter)
            {
                return false;
            }
            if ver && a.y0 + h > ph {
                return false;
            }
            if ver && h > 64 && a.x0 + w > pw {
                return false;
            }
            if !ver && w > 64 && a.y0 + h > ph {
                return false;
            }
            if a.x0 + w > pw && a.y0 + h > ph && w > min_qt {
                return false;
            }
            if !ver && a.x0 + w > pw && a.y0 + h <= ph {
                return false;
            }
            if a.mtt > 0 && a.part == 1 {
                let parallel_tt = if ver { SPLIT_TT_VER } else { SPLIT_TT_HOR };
                if a.parent_split == parallel_tt {
                    return false;
                }
            }
            if ver && w <= 64 && h > 64 {
                return false;
            }
            if !ver && w > 64 && h <= 64 {
                return false;
            }
            true
        };
        let tt = |ver: bool| -> bool {
            let size = if ver { w } else { h };
            !(size <= 2 * min_bt
                || w > max_tt.min(64)
                || h > max_tt.min(64)
                || a.mtt >= max_mtt
                || a.x0 + w > pw
                || a.y0 + h > ph
                || (chroma && (w / sw) * (h / shh) <= 32)
                || (chroma && w / sw == 8 && ver)
                || (chroma && a.mode == ModeType::Intra)
                || (w * h == 64 && a.mode == ModeType::Inter))
        };
        [qt, bt(true), bt(false), tt(true), tt(false)]
    }

    fn coding_tree(&mut self, e: &mut Engine<'_>, a: CtArgs) -> Result<()> {
        let [qt, btv, bth, ttv, tth] = self.allowed_splits(&a);
        let inside = a.x0 + a.w <= self.pic_w && a.y0 + a.h <= self.pic_h;
        let ch = usize::from(a.tree == TreeType::DualChroma);
        let split_cu = if (btv || bth || ttv || tth || qt) && inside {
            let inc = self.split_cu_ctx(&a, [qt, btv, bth, ttv, tth], ch);
            e.decode(&mut self.ctx.0[cabac::SPLIT_CU_FLAG + inc]) == 1
        } else {
            !inside
        };
        self.reset_qg(a.x0, a.y0, a.subdiv, a.qg_y, a.qg_c);
        if !split_cu {
            return self.coding_unit(e, a.x0, a.y0, a.w, a.h, a.cqt, a.tree, a.mode);
        }
        let split_qt = if (btv || bth || ttv || tth) && qt {
            let inc = self.split_qt_ctx(&a, ch);
            e.decode(&mut self.ctx.0[cabac::SPLIT_QT_FLAG + inc]) == 1
        } else {
            qt || !(btv || bth || ttv || tth)
        };
        let mut split = SPLIT_NONE;
        if !split_qt {
            let ver = if (bth || tth) && (btv || ttv) {
                let inc = self.mtt_vertical_ctx(&a, [btv, bth, ttv, tth], ch);
                e.decode(&mut self.ctx.0[cabac::MTT_SPLIT_CU_VERTICAL_FLAG + inc]) == 1
            } else {
                !(bth || tth)
            };
            let binary = if (btv && ttv && ver) || (bth && tth && !ver) {
                let inc = 2 * usize::from(ver) + usize::from(a.mtt <= 1);
                e.decode(&mut self.ctx.0[cabac::MTT_SPLIT_CU_BINARY_FLAG + inc]) == 1
            } else if !btv && !bth {
                false
            } else if !ttv && !tth {
                true
            } else if bth && ttv {
                !ver
            } else {
                ver
            };
            split = match (ver, binary) {
                (false, false) => SPLIT_TT_HOR,
                (false, true) => SPLIT_BT_HOR,
                (true, false) => SPLIT_TT_VER,
                (true, true) => SPLIT_BT_VER,
            };
            if a.mtt < 2 {
                let mtt = a.mtt as usize;
                let maps = &mut self.st.maps;
                let v = &mut maps.mtt[mtt];
                let (x0, y0, w, h) = (a.x0, a.y0, a.w, a.h);
                let w4 = maps.w4;
                let h4 = maps.h4;
                for yy in (y0 as usize >> 2)..((y0 + h) as usize).div_ceil(4).min(h4) {
                    for xx in (x0 as usize >> 2)..((x0 + w) as usize).div_ceil(4).min(w4) {
                        v[yy * w4 + xx] = split;
                    }
                }
            }
        }
        // ModeTypeCondition.
        let sps = self.c.sps;
        let area = a.w * a.h;
        let mode_cond = if (self.intra_slice && sps.qtbtt_dual_tree_intra)
            || a.mode != ModeType::All
            || sps.chroma_format_idc == 0
            || sps.chroma_format_idc == 3
        {
            0
        } else if (area == 64 && split_qt)
            || (area == 64 && !split_qt && (split == SPLIT_TT_HOR || split == SPLIT_TT_VER))
            || (area == 32 && (split == SPLIT_BT_HOR || split == SPLIT_BT_VER))
        {
            1
        } else if (area == 64
            && (split == SPLIT_BT_HOR || split == SPLIT_BT_VER)
            && sps.chroma_format_idc == 1)
            || (area == 128
                && (split == SPLIT_TT_HOR || split == SPLIT_TT_VER)
                && sps.chroma_format_idc == 1)
            || (a.w == 8 && split == SPLIT_BT_VER)
            || (a.w == 16 && !split_qt && split == SPLIT_TT_VER)
        {
            1 + u32::from(!self.intra_slice)
        } else {
            0
        };
        let mode = match mode_cond {
            1 => ModeType::Intra,
            2 => {
                let inc = self.non_inter_ctx(&a, ch);
                if e.decode(&mut self.ctx.0[cabac::NON_INTER_FLAG + inc]) == 1 {
                    ModeType::Intra
                } else {
                    ModeType::Inter
                }
            }
            _ => a.mode,
        };
        let tree = if mode == ModeType::Intra {
            TreeType::DualLuma
        } else {
            a.tree
        };
        let child = CtArgs {
            tree,
            mode,
            parent_split: split,
            ..a
        };
        let (pw, ph) = (self.pic_w, self.pic_h);
        if !split_qt {
            match split {
                SPLIT_BT_VER => {
                    let d = a.depth_offset + u32::from(a.x0 + a.w > pw);
                    let hw = a.w / 2;
                    let c0 = CtArgs {
                        w: hw,
                        subdiv: a.subdiv + 1,
                        mtt: a.mtt + 1,
                        depth_offset: d,
                        part: 0,
                        ..child
                    };
                    self.coding_tree(e, c0)?;
                    if a.x0 + hw < pw {
                        self.coding_tree(
                            e,
                            CtArgs {
                                x0: a.x0 + hw,
                                part: 1,
                                ..c0
                            },
                        )?;
                    }
                }
                SPLIT_BT_HOR => {
                    let d = a.depth_offset + u32::from(a.y0 + a.h > ph);
                    let hh = a.h / 2;
                    let c0 = CtArgs {
                        h: hh,
                        subdiv: a.subdiv + 1,
                        mtt: a.mtt + 1,
                        depth_offset: d,
                        part: 0,
                        ..child
                    };
                    self.coding_tree(e, c0)?;
                    if a.y0 + hh < ph {
                        self.coding_tree(
                            e,
                            CtArgs {
                                y0: a.y0 + hh,
                                part: 1,
                                ..c0
                            },
                        )?;
                    }
                }
                _ => {
                    let ver = split == SPLIT_TT_VER;
                    let qy = a.qg_y && a.subdiv + 2 <= self.cu_qp_delta_subdiv;
                    let qc = a.qg_c && a.subdiv + 2 <= self.cu_chroma_qp_offset_subdiv;
                    let base = CtArgs {
                        qg_y: qy,
                        qg_c: qc,
                        mtt: a.mtt + 1,
                        ..child
                    };
                    if ver {
                        let q = a.w / 4;
                        self.coding_tree(
                            e,
                            CtArgs {
                                w: q,
                                subdiv: a.subdiv + 2,
                                part: 0,
                                ..base
                            },
                        )?;
                        self.coding_tree(
                            e,
                            CtArgs {
                                x0: a.x0 + q,
                                w: 2 * q,
                                subdiv: a.subdiv + 1,
                                part: 1,
                                ..base
                            },
                        )?;
                        self.coding_tree(
                            e,
                            CtArgs {
                                x0: a.x0 + 3 * q,
                                w: q,
                                subdiv: a.subdiv + 2,
                                part: 2,
                                ..base
                            },
                        )?;
                    } else {
                        let q = a.h / 4;
                        self.coding_tree(
                            e,
                            CtArgs {
                                h: q,
                                subdiv: a.subdiv + 2,
                                part: 0,
                                ..base
                            },
                        )?;
                        self.coding_tree(
                            e,
                            CtArgs {
                                y0: a.y0 + q,
                                h: 2 * q,
                                subdiv: a.subdiv + 1,
                                part: 1,
                                ..base
                            },
                        )?;
                        self.coding_tree(
                            e,
                            CtArgs {
                                y0: a.y0 + 3 * q,
                                h: q,
                                subdiv: a.subdiv + 2,
                                part: 2,
                                ..base
                            },
                        )?;
                    }
                }
            }
        } else {
            let (hw, hh) = (a.w / 2, a.h / 2);
            let base = CtArgs {
                w: hw,
                h: hh,
                subdiv: a.subdiv + 2,
                cqt: a.cqt + 1,
                mtt: 0,
                depth_offset: 0,
                part: 0,
                parent_split: SPLIT_NONE,
                ..child
            };
            self.coding_tree(e, base)?;
            if a.x0 + hw < pw {
                self.coding_tree(
                    e,
                    CtArgs {
                        x0: a.x0 + hw,
                        part: 1,
                        ..base
                    },
                )?;
            }
            if a.y0 + hh < ph {
                self.coding_tree(
                    e,
                    CtArgs {
                        y0: a.y0 + hh,
                        part: 2,
                        ..base
                    },
                )?;
            }
            if a.y0 + hh < ph && a.x0 + hw < pw {
                self.coding_tree(
                    e,
                    CtArgs {
                        x0: a.x0 + hw,
                        y0: a.y0 + hh,
                        part: 3,
                        ..base
                    },
                )?;
            }
        }
        if a.mode == ModeType::All && mode == ModeType::Intra {
            self.coding_tree(
                e,
                CtArgs {
                    qg_y: false,
                    tree: TreeType::DualChroma,
                    mode,
                    depth_offset: 0,
                    part: 0,
                    ..a
                },
            )?;
        }
        Ok(())
    }

    fn nb(&self, a: &CtArgs, dx: i32, dy: i32, ch: usize) -> Option<usize> {
        let (xn, yn) = (a.x0 as i32 + dx, a.y0 as i32 + dy);
        self.available(a.x0, a.y0, xn, yn, ch)
            .then(|| self.st.maps.idx(xn as u32, yn as u32))
    }

    fn split_cu_ctx(&self, a: &CtArgs, al: [bool; 5], ch: usize) -> usize {
        let m = &self.st.maps;
        let l = self.nb(a, -1, 0, ch);
        let t = self.nb(a, 0, -1, ch);
        let cl = l.is_some_and(|i| (1u32 << m.log2_h[ch][i]) < a.h) as usize;
        let ca = t.is_some_and(|i| (1u32 << m.log2_w[ch][i]) < a.w) as usize;
        let n =
            al[1] as usize + al[2] as usize + al[3] as usize + al[4] as usize + 2 * al[0] as usize;
        let set = (n.saturating_sub(1)) / 2;
        cl + ca + set * 3
    }

    fn split_qt_ctx(&self, a: &CtArgs, ch: usize) -> usize {
        let m = &self.st.maps;
        let l = self.nb(a, -1, 0, ch);
        let t = self.nb(a, 0, -1, ch);
        let cl = l.is_some_and(|i| u32::from(m.cqt_depth[ch][i]) > a.cqt) as usize;
        let ca = t.is_some_and(|i| u32::from(m.cqt_depth[ch][i]) > a.cqt) as usize;
        cl + ca + 3 * usize::from(a.cqt >= 2)
    }

    fn mtt_vertical_ctx(&self, a: &CtArgs, al: [bool; 4], ch: usize) -> usize {
        let [btv, bth, ttv, tth] = al.map(usize::from);
        if btv + ttv > bth + tth {
            return 4;
        }
        if btv + ttv < bth + tth {
            return 3;
        }
        let m = &self.st.maps;
        let l = self.nb(a, -1, 0, ch);
        let t = self.nb(a, 0, -1, ch);
        let (Some(l), Some(t)) = (l, t) else {
            return 0;
        };
        let da = a.w / (1u32 << m.log2_w[ch][t]).max(1);
        let dl = a.h / (1u32 << m.log2_h[ch][l]).max(1);
        if da == dl {
            0
        } else if da < dl {
            1
        } else {
            2
        }
    }

    fn non_inter_ctx(&self, a: &CtArgs, ch: usize) -> usize {
        let m = &self.st.maps;
        let l = self.nb(a, -1, 0, ch);
        let t = self.nb(a, 0, -1, ch);
        let cl = l.is_some_and(|i| m.pred[ch][i] == MODE_INTRA);
        let ca = t.is_some_and(|i| m.pred[ch][i] == MODE_INTRA);
        usize::from(cl || ca)
    }
}

/// Arguments of `coding_tree()`.
#[derive(Clone, Copy, Debug)]
pub struct CtArgs {
    pub x0: u32,
    pub y0: u32,
    pub w: u32,
    pub h: u32,
    pub qg_y: bool,
    pub qg_c: bool,
    pub subdiv: u32,
    pub cqt: u32,
    pub mtt: u32,
    pub depth_offset: u32,
    pub part: u32,
    pub tree: TreeType,
    pub mode: ModeType,
    /// MttSplitMode of the parent node at depth mtt − 1.
    pub parent_split: u8,
}

/// Truncated binary bypass bins with cMax `cmax` (9.3.3.4).
pub fn tb_bypass(e: &mut Engine<'_>, cmax: u32) -> u32 {
    let n = cmax + 1;
    let k = 31 - n.leading_zeros();
    let u = (1u32 << (k + 1)) - n;
    let v = e.bypass_bits(k);
    if v < u {
        v
    } else {
        ((v << 1) | e.bypass()) - u
    }
}
