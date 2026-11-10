//! In-loop filters (8.8): deblocking (8.8.3), SAO (8.8.4), ALF and CC-ALF (8.8.5).
//!
//! The block decoder reports every coding unit and transform unit it reconstructs to
//! [`Meta`] ([`Meta::cu`], [`Meta::tu`]); after the picture is reconstructed (and luma is
//! inverse mapped when LMCS is on) [`run`] filters it in place.
//!
//! Every stage reads a snapshot of its input and writes the picture in row bands, so the
//! bands run on any number of threads with the same result: the edges of one deblocking
//! direction, the CTBs of SAO and of ALF do not depend on each other's output.

use std::sync::{Arc, Mutex, PoisonError};

use crate::picture::Picture;
use crate::ps::alf::AlfAps;
use crate::ps::ph::AlfSlice;
use crate::ps::pps::{DbfOffsets, Layout, Pps};
use crate::ps::sps::Sps;

mod alf;
#[rustfmt::skip]
mod alf_tables;
mod deblock;
mod sao;

/// Which tree a coding unit or transform unit belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tree {
    /// Luma and chroma together.
    Single,
    /// Dual tree, luma only.
    Luma,
    /// Dual tree, chroma only.
    Chroma,
}

/// CuPredMode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pred {
    Intra,
    Inter,
    Ibc,
    Palette,
}

/// One reconstructed coding unit. Coordinates and sizes are in luma samples for every tree
/// (a dual-tree chroma CU covers `w x h` luma samples, `w / SubWidthC x h / SubHeightC`
/// chroma samples).
#[derive(Clone, Copy, Debug)]
pub struct CuRec {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub tree: Tree,
    pub pred: Pred,
    /// QpY of the coding unit.
    pub qp_y: i32,
    /// intra_bdpcm_luma_flag, intra_bdpcm_chroma_flag.
    pub bdpcm: [bool; 2],
    /// Intra sub-partitions: (horizontal split, number of sub-partitions).
    pub isp: Option<(bool, u32)>,
    /// Luma block vector of an IBC coding unit in 1/16 samples; (0, 0) otherwise.
    pub bv: (i32, i32),
    /// cu_act_enabled_flag (not used by the filters of this version).
    #[allow(dead_code)]
    pub act: bool,
}

/// One transform unit, in luma coordinates; `tree` says which components it carries.
#[derive(Clone, Copy, Debug)]
pub struct TuRec {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub tree: Tree,
    /// Non-zero coefficients per component: tu_y_coded_flag, and for Cb and Cr their coded
    /// flag or tu_joint_cbcr_residual_flag.
    pub cbf: [bool; 3],
    /// transform_skip_flag per component (not used by the filters of this version).
    #[allow(dead_code)]
    pub ts: [bool; 3],
}

/// SAO parameters of one CTB (7.3.11.3 / 7.4.12.3), per component.
#[derive(Clone, Copy, Debug, Default)]
pub struct SaoCtb {
    /// SaoTypeIdx: 0 off, 1 band offset, 2 edge offset.
    pub type_idx: [u8; 3],
    /// SaoOffsetVal[ cIdx ][ rx ][ ry ][ 1..4 ] (already scaled by the bit depth shift).
    pub offset: [[i16; 4]; 3],
    /// sao_band_position.
    pub band_position: [u8; 3],
    /// SaoEoClass.
    pub eo_class: [u8; 3],
}

/// ALF parameters of one CTB (7.3.11.2), per component.
#[derive(Clone, Copy, Debug, Default)]
pub struct AlfCtb {
    /// alf_ctb_flag[ cIdx ].
    pub enabled: [bool; 3],
    /// AlfCtbFiltSetIdxY: 0..15 a fixed filter set, 16 + i the i-th luma APS of the slice.
    pub luma_set: u8,
    /// alf_ctb_filter_alt_idx for Cb and Cr.
    pub chroma_alt: [u8; 2],
    /// alf_ctb_cc_cb_idc, alf_ctb_cc_cr_idc (0 off).
    pub cc: [u8; 2],
}

/// What the filters need to know about one slice.
#[derive(Clone, Debug)]
pub struct SliceFilter {
    /// Deblocking offsets and sh_deblocking_filter_disabled_flag.
    pub dbf: DbfOffsets,
    pub sao_luma: bool,
    pub sao_chroma: bool,
    /// ALF enables and APS ids.
    pub alf: AlfSlice,
    /// sh_cb_qp_offset and sh_cr_qp_offset (the chroma QP of a transform block, used by
    /// chroma deblocking when [`Meta::tu_chroma_qp`] does not give it).
    pub cb_qp_offset: i32,
    pub cr_qp_offset: i32,
}

/// Everything the filters read besides the samples and [`Meta`].
pub struct FilterCtx<'a> {
    pub sps: &'a Sps,
    pub pps: &'a Pps,
    pub layout: &'a Layout,
    /// Slice index of each CTB (picture raster address).
    pub ctb_slice: &'a [u32],
    pub slices: &'a [SliceFilter],
    /// Per CTB, in raster order.
    pub sao: &'a [SaoCtb],
    pub alf: &'a [AlfCtb],
    /// The ALF APSs in force for the picture, by aps_adaptation_parameter_set_id.
    pub alf_aps: &'a [Option<Arc<AlfAps>>; 8],
    /// Virtual boundary positions (luma samples) of the picture.
    pub vb_x: &'a [u32],
    pub vb_y: &'a [u32],
}

/// No CU or TB recorded for a grid cell.
const NONE: u32 = u32::MAX;

/// A coding block of one channel, as the deblocking filter needs it.
#[derive(Clone, Copy, Debug)]
struct CuInfo {
    /// Top-left in luma samples.
    x: u32,
    y: u32,
    pred: Pred,
    qp_y: i32,
    /// intra_bdpcm_*_flag of this channel.
    bdpcm: bool,
    /// Block vector (IBC), 1/16 luma samples.
    bv: (i32, i32),
}

/// A transform block of one channel, in that channel's samples.
#[derive(Clone, Copy, Debug)]
struct TbInfo {
    w: u32,
    h: u32,
    /// Luma: [Y, -, -]; chroma: [-, Cb, Cr].
    cbf: [bool; 3],
    /// Qp′ of the block for Cb and Cr deblocking (Qp′CbCr for both when TuCResMode is 2).
    qpc: Option<[i32; 2]>,
}

/// One channel's block maps: cells of `cell x cell` samples of that channel.
struct Grid {
    log2_cell: u32,
    w: usize,
    h: usize,
    cu: Vec<u32>,
    tb: Vec<u32>,
}

impl Grid {
    fn new(plane_w: u32, plane_h: u32, log2_cell: u32) -> Grid {
        let w = (plane_w as usize).div_ceil(1 << log2_cell);
        let h = (plane_h as usize).div_ceil(1 << log2_cell);
        Grid {
            log2_cell,
            w,
            h,
            cu: vec![NONE; w * h],
            tb: vec![NONE; w * h],
        }
    }

    /// Sets `v` in the cells whose first sample lies in the area (in this channel's
    /// samples). A block narrower than a cell (an ISP sub-partition) only marks the cell it
    /// starts; the blocks after it in the same cell are found by [`p_side`].
    fn fill(which: &mut [u32], w: usize, h: usize, l: u32, area: (u32, u32, u32, u32), v: u32) {
        let (x, y, aw, ah) = area;
        let x0 = (x as usize).div_ceil(1 << l);
        let y0 = (y as usize).div_ceil(1 << l);
        let x1 = (((x + aw) as usize).div_ceil(1 << l)).min(w);
        let y1 = (((y + ah) as usize).div_ceil(1 << l)).min(h);
        for r in y0.min(h)..y1 {
            which[r * w + x0.min(x1)..r * w + x1].fill(v);
        }
    }

    /// The transform block covering the last sample of the cell before an edge, from the
    /// block covering the cell's first sample: sub-partitions narrower (or lower) than a
    /// cell are recorded consecutively with equal sizes.
    fn p_side(&self, tbs: &[TbInfo], tb: u32, vertical: bool) -> u32 {
        let cell = 1u32 << self.log2_cell;
        let t = tbs[tb as usize];
        let s = if vertical { t.w } else { t.h };
        if s >= cell || s == 0 {
            return tb;
        }
        let k = tb + cell / s - 1;
        if (k as usize) < tbs.len() { k } else { tb }
    }

    #[inline]
    fn at(&self, x: u32, y: u32) -> usize {
        ((y >> self.log2_cell) as usize) * self.w + (x >> self.log2_cell) as usize
    }
}

/// Per-block metadata gathered during reconstruction.
pub struct Meta {
    /// Chroma sampling shifts; `None` for 4:0:0.
    shift: Option<(u32, u32)>,
    /// [luma, chroma] grids.
    grids: Vec<Grid>,
    /// [luma, chroma] block lists.
    cus: [Vec<CuInfo>; 2],
    tbs: [Vec<TbInfo>; 2],
    /// The CU being reported, per channel, with its ISP flag.
    cur: [Option<(u32, bool)>; 2],
    /// The default (CU-sized) chroma TB of an ISP coding unit.
    isp_chroma_tb: Option<u32>,
    /// TBs added by the last [`Meta::tu`] call, per channel.
    last_tb: [Option<u32>; 2],
}

impl Meta {
    pub fn new(sps: &Sps, pps: &Pps) -> Meta {
        let shift = match sps.chroma_format_idc {
            0 => None,
            1 => Some((1, 1)),
            2 => Some((1, 0)),
            _ => Some((0, 0)),
        };
        let mut grids = vec![Grid::new(pps.width, pps.height, 2)];
        if let Some((sx, sy)) = shift {
            grids.push(Grid::new(
                pps.width.div_ceil(1 << sx),
                pps.height.div_ceil(1 << sy),
                1,
            ));
        }
        Meta {
            shift,
            grids,
            cus: [Vec::new(), Vec::new()],
            tbs: [Vec::new(), Vec::new()],
            cur: [None, None],
            isp_chroma_tb: None,
            last_tb: [None, None],
        }
    }

    /// The channels (0 luma, 1 chroma) a tree carries in this picture.
    fn channels(&self, tree: Tree) -> &'static [usize] {
        match (tree, self.shift.is_some()) {
            (Tree::Single, true) => &[0, 1],
            (Tree::Single, false) | (Tree::Luma, _) => &[0],
            (Tree::Chroma, true) => &[1],
            (Tree::Chroma, false) => &[],
        }
    }

    /// An area in luma samples, in channel `ch`'s samples.
    fn area(&self, ch: usize, x: u32, y: u32, w: u32, h: u32) -> (u32, u32, u32, u32) {
        match (ch, self.shift) {
            (1, Some((sx, sy))) => (x >> sx, y >> sy, (w >> sx).max(1), (h >> sy).max(1)),
            _ => (x, y, w, h),
        }
    }

    fn add_tb(&mut self, ch: usize, area: (u32, u32, u32, u32), tb: TbInfo) -> u32 {
        let id = self.tbs[ch].len() as u32;
        self.tbs[ch].push(tb);
        let g = &mut self.grids[ch];
        Grid::fill(&mut g.tb, g.w, g.h, g.log2_cell, area, id);
        id
    }

    /// Records a reconstructed coding unit (called once per CU, before its TUs).
    pub fn cu(&mut self, cu: &CuRec) {
        self.isp_chroma_tb = None;
        for &ch in self.channels(cu.tree) {
            let area = self.area(ch, cu.x, cu.y, cu.w, cu.h);
            let id = self.cus[ch].len() as u32;
            self.cus[ch].push(CuInfo {
                x: cu.x,
                y: cu.y,
                pred: cu.pred,
                qp_y: cu.qp_y,
                bdpcm: cu.bdpcm[ch],
                bv: cu.bv,
            });
            let g = &mut self.grids[ch];
            Grid::fill(&mut g.cu, g.w, g.h, g.log2_cell, area, id);
            // Until its TUs say otherwise (palette CUs have none), the CU is one transform
            // block without coefficients.
            let tb = self.add_tb(
                ch,
                area,
                TbInfo {
                    w: area.2,
                    h: area.3,
                    cbf: [false; 3],
                    qpc: None,
                },
            );
            self.cur[ch] = Some((id, cu.isp.is_some()));
            if ch == 1 && cu.isp.is_some() {
                self.isp_chroma_tb = Some(tb);
            }
        }
    }

    /// Records a transform unit (called once per TU, in decoding order; an ISP coding unit
    /// reports each sub-partition as a TU).
    pub fn tu(&mut self, tu: &TuRec) {
        self.last_tb = [None, None];
        for &ch in self.channels(tu.tree) {
            let isp = self.cur[ch].is_some_and(|c| c.1);
            if ch == 1 && isp {
                // The chroma of an ISP CU is one block covering the CU, coded with the last
                // sub-partition.
                if let Some(t) = self.isp_chroma_tb {
                    let tb = &mut self.tbs[1][t as usize];
                    tb.cbf[1] |= tu.cbf[1];
                    tb.cbf[2] |= tu.cbf[2];
                    self.last_tb[1] = Some(t);
                }
                continue;
            }
            let area = self.area(ch, tu.x, tu.y, tu.w, tu.h);
            let cbf = if ch == 0 {
                [tu.cbf[0], false, false]
            } else {
                [false, tu.cbf[1], tu.cbf[2]]
            };
            let id = self.add_tb(
                ch,
                area,
                TbInfo {
                    w: area.2,
                    h: area.3,
                    cbf,
                    qpc: None,
                },
            );
            self.last_tb[ch] = Some(id);
        }
    }

    /// Optional: the chroma QPs the last [`Meta::tu`] was dequantised with, for chroma
    /// deblocking (8.8.3.6.4): `[Qp′Cb, Qp′Cr]`, or `[Qp′CbCr, Qp′CbCr]` when its
    /// TuCResMode is 2. Without it they are derived from the CU's QpY and the PPS and slice
    /// offsets (exact unless CU chroma QP offsets or joint CbCr mode 2 are used).
    pub fn tu_chroma_qp(&mut self, qp: [i32; 2]) {
        if let Some(t) = self.last_tb[1] {
            self.tbs[1][t as usize].qpc = Some(qp);
        }
    }
}

/// Picture-wide facts the stages share.
struct Shared<'a> {
    ctx: &'a FilterCtx<'a>,
    /// Subpicture index of each CTB.
    ctb_subpic: Vec<u16>,
    /// Tile index of each CTB.
    ctb_tile: Vec<u32>,
    log2_ctb: u32,
    w_ctbs: u32,
    h_ctbs: u32,
    pic_w: u32,
    pic_h: u32,
}

impl<'a> Shared<'a> {
    fn new(ctx: &'a FilterCtx<'a>) -> Shared<'a> {
        let l = ctx.layout;
        let n = (l.w_ctbs * l.h_ctbs) as usize;
        let mut ctb_subpic = vec![0u16; n];
        let mut ctb_tile = vec![0u32; n];
        for y in 0..l.h_ctbs {
            for x in 0..l.w_ctbs {
                let i = (y * l.w_ctbs + x) as usize;
                ctb_tile[i] = l.tile_of(x, y);
                for (k, sp) in ctx.sps.subpics.iter().enumerate() {
                    if x >= sp.ctu_x && x < sp.ctu_x + sp.w && y >= sp.ctu_y && y < sp.ctu_y + sp.h
                    {
                        ctb_subpic[i] = k as u16;
                        break;
                    }
                }
            }
        }
        Shared {
            ctx,
            ctb_subpic,
            ctb_tile,
            log2_ctb: l.log2_ctb,
            w_ctbs: l.w_ctbs,
            h_ctbs: l.h_ctbs,
            pic_w: ctx.pps.width,
            pic_h: ctx.pps.height,
        }
    }

    /// CTB raster address of a luma sample.
    #[inline]
    fn ctb_of(&self, x: u32, y: u32) -> usize {
        ((y >> self.log2_ctb) * self.w_ctbs + (x >> self.log2_ctb)) as usize
    }

    #[inline]
    fn slice_of_ctb(&self, c: usize) -> usize {
        self.ctx.ctb_slice.get(c).copied().unwrap_or(0) as usize
    }

    fn slice(&self, c: usize) -> Option<&SliceFilter> {
        self.ctx.slices.get(self.slice_of_ctb(c))
    }

    fn lf_across_subpic(&self, c: usize) -> bool {
        self.ctx
            .sps
            .subpics
            .get(usize::from(self.ctb_subpic[c]))
            .is_none_or(|s| s.loop_filter_across)
    }

    /// Whether loop filtering may use samples of CTB `b` for CTB `a` (8.8.4.2 / 8.8.5.5
    /// conditions: subpicture of `a`, slices, tiles).
    fn across_ok(&self, a: usize, b: usize) -> bool {
        if a == b {
            return true;
        }
        if self.ctb_subpic[a] != self.ctb_subpic[b] && !self.lf_across_subpic(a) {
            return false;
        }
        if !self.ctx.pps.loop_filter_across_slices && self.slice_of_ctb(a) != self.slice_of_ctb(b) {
            return false;
        }
        if !self.ctx.pps.loop_filter_across_tiles && self.ctb_tile[a] != self.ctb_tile[b] {
            return false;
        }
        true
    }
}

/// Runs `f` on every job, on up to `threads` scoped threads. The jobs touch disjoint data,
/// so the order they run in does not matter.
fn run_jobs<J: Send>(jobs: Vec<J>, threads: usize, f: impl Fn(J) + Sync) {
    let n = threads.min(jobs.len());
    if n <= 1 {
        jobs.into_iter().for_each(f);
        return;
    }
    let queue = Mutex::new(jobs.into_iter());
    std::thread::scope(|s| {
        for wi in 0..n {
            specular_thread::spawn_scoped(s, format!("dec-vvc-lf-{wi}"), || {
                loop {
                    let job = queue.lock().unwrap_or_else(PoisonError::into_inner).next();
                    let Some(job) = job else { break };
                    f(job);
                }
            });
        }
    });
}

/// A band of rows `y0..y0 + rows` of a plane, writable.
pub(crate) struct Band<'a> {
    pub data: &'a mut [u16],
    pub y0: usize,
    pub rows: usize,
}

/// Cuts a plane into bands at the given row boundaries (ascending, first 0, last `h`).
fn bands<'a>(data: &'a mut [u16], stride: usize, cuts: &[usize]) -> Vec<Band<'a>> {
    let mut out = Vec::with_capacity(cuts.len());
    let mut rest = data;
    for w in cuts.windows(2) {
        let rows = w[1] - w[0];
        let (a, b) = rest.split_at_mut((rows * stride).min(rest.len()));
        out.push(Band {
            data: a,
            y0: w[0],
            rows,
        });
        rest = b;
    }
    out
}

/// Row cuts of a plane of height `h` every `step` rows.
fn cuts(h: usize, step: usize) -> Vec<usize> {
    let mut v: Vec<usize> = (0..h).step_by(step.max(1)).collect();
    v.push(h);
    v
}

/// Deblocks, then applies SAO, then ALF and CC-ALF to `pic`, on up to `threads` threads. The
/// output does not depend on `threads`.
pub fn run(pic: &mut Picture, meta: &Meta, ctx: &FilterCtx<'_>, threads: usize) {
    let sh = Shared::new(ctx);
    let threads = threads.max(1);
    deblock::run(pic, meta, &sh, threads);
    if ctx.sps.sao {
        sao::run(pic, &sh, threads);
    }
    if ctx.sps.alf {
        alf::run(pic, &sh, threads);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ps::sps::Subpic;

    fn setup() -> (Sps, Pps) {
        let sps = Sps {
            chroma_format_idc: 1,
            log2_ctu_size: 5,
            bit_depth: 8,
            pic_width_max: 32,
            pic_height_max: 16,
            subpics: vec![Subpic {
                w: 1,
                h: 1,
                treated_as_pic: true,
                ..Subpic::default()
            }],
            chroma_qp_table: core::array::from_fn(|_| (0..=63).map(|q| q as i8).collect()),
            ..Sps::default()
        };
        let pps = Pps {
            width: 32,
            height: 16,
            no_pic_partition: true,
            ..Pps::default()
        };
        (sps, pps)
    }

    fn picture(threads: usize) -> Picture {
        let (sps, pps) = setup();
        let layout = Layout::new(&sps, &pps).unwrap();
        let mut pic = Picture::new(32, 16, 1, 8).unwrap();
        for y in 0..16 {
            for x in 0..32 {
                pic.planes[0].set(x, y, if x < 16 { 100 } else { 140 });
            }
        }
        for c in 1..3 {
            for y in 0..8 {
                for x in 0..16 {
                    pic.planes[c].set(x, y, if x < 8 { 60 } else { 90 });
                }
            }
        }
        let mut meta = Meta::new(&sps, &pps);
        for x in [0, 16] {
            meta.cu(&CuRec {
                x,
                y: 0,
                w: 16,
                h: 16,
                tree: Tree::Single,
                pred: Pred::Intra,
                qp_y: 37,
                bdpcm: [false; 2],
                isp: None,
                bv: (0, 0),
                act: false,
            });
            meta.tu(&TuRec {
                x,
                y: 0,
                w: 16,
                h: 16,
                tree: Tree::Single,
                cbf: [false; 3],
                ts: [false; 3],
            });
        }
        let slices = vec![SliceFilter {
            dbf: DbfOffsets::default(),
            sao_luma: false,
            sao_chroma: false,
            alf: AlfSlice::default(),
            cb_qp_offset: 0,
            cr_qp_offset: 0,
        }];
        let aps: [Option<Arc<AlfAps>>; 8] = Default::default();
        let ctx = FilterCtx {
            sps: &sps,
            pps: &pps,
            layout: &layout,
            ctb_slice: &[0],
            slices: &slices,
            sao: &[SaoCtb::default()],
            alf: &[AlfCtb::default()],
            alf_aps: &aps,
            vb_x: &[],
            vb_y: &[],
        };
        run(&mut pic, &meta, &ctx, threads);
        pic
    }

    #[test]
    fn intra_edge_is_weakly_filtered_the_same_on_any_thread_count() {
        let p = picture(1);
        // qP 37, tC (Q 39, 8 bit) = 5: the weak filter moves p0 and q0 by tC; p1 and q1 by
        // tC / 2 (flat sides make dEp and dEq true).
        let row: Vec<u16> = (12..20).map(|x| p.planes[0].at(x, 3)).collect();
        assert_eq!(row, vec![100, 100, 102, 105, 135, 138, 140, 140]);
        // The chroma edge at x = 8 (8x8 chroma blocks) is filtered too.
        assert_ne!(p.planes[1].at(8, 2), 90);
        let q = picture(4);
        for c in 0..3 {
            assert_eq!(p.planes[c].data, q.planes[c].data);
        }
    }
}
