//! Macroblock decisions, residual coding and reconstruction for one slice.
//!
//! A slice is a band of whole macroblock rows. Its reconstruction (before deblocking) is
//! written into its own rows of the picture, so slices run in parallel: intra prediction and
//! neighbour contexts never cross a slice boundary, and motion compensation only reads the
//! (shared, read-only) reference pictures.
//!
//! Slices are I, P (one reference: the previous picture) or B. B slices here never look
//! ahead: both of their references are in the past — list 0 is the previous picture and
//! list 1 the long-term *anchor* (the picture before a cross-fade started) — so a B picture
//! is coded as soon as it arrives and is itself a reference.

// Block geometry: one index addresses several parallel per-block arrays.
#![allow(clippy::needless_range_loop)]

use crate::bits::{BitCount, BitWriter, Sink, se_bits};
use crate::cabac::{
    Bins, CAT_CHROMA_AC, CAT_CHROMA_DC, CAT_LUMA_4X4, CAT_LUMA_8X8, CAT_LUMA_AC, CAT_LUMA_DC,
    CabacEnc,
};
use crate::cavlc::write_block;
use crate::dsp::{sa8d8, sad16, satd8, satd16, ssd};
use crate::mc::{RefFrame, mv_range};
use crate::pred::{self, Edge4, Edge8, EdgeN};
use crate::tables::{BLK_XY, CBP_INTER, CBP_INTRA, CHROMA_QP, ZIGZAG};
use crate::transform::{
    ZIGZAG8, dequant_chroma_dc, dequant_luma_dc, dequant4, dequant8, fdct4_diff, fdct8, frac4,
    frac8, hadamard2, hadamard4, idct4_add, idct8_add, qstep2, quant_dc, quant4, quant8,
};
use crate::trellis;
use crate::weights::{Weights, average};
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::num::SimdInt;
use std::simd::{Simd, SimdElement, simd_swizzle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceKind {
    I,
    P,
    B,
}

impl SliceKind {
    fn slice_type(self) -> u32 {
        match self {
            SliceKind::P => 0,
            SliceKind::B => 1,
            SliceKind::I => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MbKind {
    /// P_Skip or B_Skip.
    #[default]
    Skip,
    /// P_L0_16x16, B_L0/L1/Bi_16x16 or B_Direct_16x16.
    Inter,
    I4,
    I16,
}

/// What later macroblocks (prediction contexts) and the deblocking filter need to know.
#[derive(Clone, Copy, Debug)]
pub struct MbInfo {
    pub kind: MbKind,
    /// B_Skip / B_Direct_16x16.
    pub direct: bool,
    /// Prediction lists used (bit 0: list 0, bit 1: list 1); 0 for intra.
    pub pred: u8,
    /// Motion vectors per list in quarter samples (zero for unused lists and intra).
    pub mv: [[i32; 2]; 2],
    /// |mvd| per list and component (CABAC contexts); zero when not coded.
    pub mvd: [[u16; 2]; 2],
    /// Non-zero levels of each 4×4 luma block, raster order (AC only for Intra16x16; the
    /// whole 8×8 block's count for each of its 4×4 blocks with the 8×8 transform).
    pub nz: [u8; 16],
    /// Non-zero AC levels of the chroma blocks, [Cb, Cr] × raster 2×2.
    pub nzc: [[u8; 4]; 2],
    /// Coded DC blocks with levels: bit 0 Intra16x16 luma, bit 1 Cb, bit 2 Cr.
    pub dc_cbf: u8,
    /// coded_block_pattern (luma bits 0..3, chroma << 4); 0 or 15 luma for Intra16x16.
    pub cbp: u8,
    pub chroma_mode: u8,
    /// transform_size_8x8_flag.
    pub t8: bool,
    /// Intra4x4PredMode per 4×4 block, raster order; 2 (DC) for other macroblock types.
    pub modes: [u8; 16],
}

impl Default for MbInfo {
    fn default() -> Self {
        MbInfo {
            kind: MbKind::Skip,
            direct: false,
            pred: 1,
            mv: [[0; 2]; 2],
            mvd: [[0; 2]; 2],
            nz: [0; 16],
            nzc: [[0; 4]; 2],
            dc_cbf: 0,
            cbp: 0,
            chroma_mode: 0,
            t8: false,
            modes: [2; 16],
        }
    }
}

impl MbInfo {
    #[inline]
    pub fn is_intra(&self) -> bool {
        matches!(self.kind, MbKind::I4 | MbKind::I16)
    }

    /// An inter macroblock without luma coefficients.
    #[inline]
    pub fn is_skip_like(&self) -> bool {
        !self.is_intra() && self.nz.iter().all(|&c| c == 0)
    }

    /// coded_block_flag of luma 4×4 block `r` (raster), as a neighbour sees it: an 8×8
    /// transform block counts as coded.
    #[inline]
    fn cbf_luma(&self, r: usize) -> bool {
        if self.t8 {
            self.cbp >> b8_of(r) & 1 != 0
        } else {
            self.nz[r] > 0
        }
    }
}

/// The 8×8 block of raster 4×4 block `r`.
#[inline]
fn b8_of(r: usize) -> usize {
    (r / 8) * 2 + (r % 4) / 2
}

/// Raster 4×4 index → luma4x4BlkIdx.
const RASTER_TO_BLK: [usize; 16] = {
    let mut t = [0usize; 16];
    let mut i = 0;
    while i < 16 {
        let (x, y) = BLK_XY[i];
        t[y * 4 + x] = i;
        i += 1;
    }
    t
};

/// A per-macroblock motion guess from the caller (a known global zoom and pan).
#[derive(Clone, Copy, Debug)]
pub struct MotionHint {
    /// Sample (x, y) of this picture shows (x·scale + dx, y·scale + dy) of the previous one.
    pub scale: f32,
    pub dx: f32,
    pub dy: f32,
}

impl MotionHint {
    /// The motion vector (quarter samples) of macroblock (mx, my).
    pub(crate) fn mv(&self, mx: usize, my: usize) -> [i32; 2] {
        let cx = (mx * 16) as f32 + 7.5;
        let cy = (my * 16) as f32 + 7.5;
        [
            ((cx * self.scale + self.dx - cx) * 4.0).round() as i32,
            ((cy * self.scale + self.dy - cy) * 4.0).round() as i32,
        ]
    }
}

/// The picture-wide inputs shared by all slices.
pub struct Picture<'a> {
    pub mb_w: usize,
    /// Source planes, macroblock aligned: luma stride `mb_w·16`, chroma `mb_w·8`.
    pub src: [&'a [u8]; 3],
    pub kind: SliceKind,
    /// RefPicList0[0] (the previous picture) and RefPicList1[0] (the anchor, B only).
    pub refs: [Option<&'a RefFrame>; 2],
    /// The explicit weight table of P and B slices when the PPS enables weighted prediction.
    pub weights: Option<Weights>,
    /// Macroblock info of the previous picture (co-located motion candidates).
    pub prev_info: Option<&'a [MbInfo]>,
    pub qp: u8,
    /// CABAC (Main, High) rather than CAVLC (Baseline).
    pub cabac: bool,
    /// transform_8x8_mode_flag (High).
    pub t8: bool,
    /// Rate-distortion optimised quantisation (CABAC only).
    pub trellis: bool,
    pub hint: Option<MotionHint>,
}

/// One slice's slice-header fields that vary.
#[derive(Clone, Copy, Debug)]
pub struct SliceHeader {
    pub idr: bool,
    pub idr_pic_id: u32,
    pub frame_num: u32,
    pub init_qp: u8,
    pub pps_id: u8,
    /// Long-term reference marking (Main/High): the IDR becomes the long-term anchor.
    pub long_term: bool,
    /// MMCO 3: the previous picture becomes the long-term anchor.
    pub mmco_prev: bool,
}

/// Writes slice_header() for a slice starting at macroblock `first_mb`.
pub fn write_slice_header(
    w: &mut BitWriter,
    first_mb: u32,
    kind: SliceKind,
    weights: Option<&Weights>,
    cabac: bool,
    qp: u8,
    hdr: &SliceHeader,
) {
    w.ue(first_mb);
    w.ue(kind.slice_type());
    w.ue(u32::from(hdr.pps_id)); // pic_parameter_set_id
    w.put(crate::params::LOG2_MAX_FRAME_NUM, hdr.frame_num);
    if hdr.idr {
        w.ue(hdr.idr_pic_id);
    }
    if kind == SliceKind::B {
        w.flag(true); // direct_spatial_mv_pred_flag
    }
    if kind != SliceKind::I {
        w.flag(false); // num_ref_idx_active_override_flag
        w.flag(false); // ref_pic_list_modification_flag_l0
    }
    if kind == SliceKind::B {
        w.flag(false); // ref_pic_list_modification_flag_l1
    }
    if kind != SliceKind::I
        && let Some(t) = weights
    {
        // pred_weight_table()
        w.ue(t.luma_log2);
        w.ue(t.chroma_log2);
        for l in 0..1 + usize::from(kind == SliceKind::B) {
            let luma = !t.is_default(l, 0);
            w.flag(luma);
            if luma {
                w.se(t.wp[l][0].w);
                w.se(t.wp[l][0].o);
            }
            let chroma = !t.is_default(l, 1) || !t.is_default(l, 2);
            w.flag(chroma);
            if chroma {
                for p in 1..3 {
                    w.se(t.wp[l][p].w);
                    w.se(t.wp[l][p].o);
                }
            }
        }
    }
    // dec_ref_pic_marking() (every picture is a reference)
    if hdr.idr {
        w.flag(false); // no_output_of_prior_pics_flag
        w.flag(hdr.long_term); // long_term_reference_flag
    } else {
        w.flag(hdr.mmco_prev); // adaptive_ref_pic_marking_mode_flag
        if hdr.mmco_prev {
            w.ue(3); // short-term → long-term
            w.ue(0); // difference_of_pic_nums_minus1: the previous picture
            w.ue(0); // long_term_frame_idx
            w.ue(0); // end
        }
    }
    if cabac && kind != SliceKind::I {
        w.ue(0); // cabac_init_idc
    }
    w.se(i32::from(qp) - i32::from(hdr.init_qp)); // slice_qp_delta
    w.ue(0); // disable_deblocking_filter_idc
    w.se(0); // slice_alpha_c0_offset_div2
    w.se(0); // slice_beta_offset_div2
}

/// A slice of `n` skipped macroblocks from `first_mb` (a P picture equal to its reference).
pub fn skip_slice(
    first_mb: u32,
    n: u32,
    weights: Option<&Weights>,
    cabac: bool,
    qp: u8,
    hdr: &SliceHeader,
) -> Vec<u8> {
    let mut w = BitWriter::new();
    write_slice_header(&mut w, first_mb, SliceKind::P, weights, cabac, qp, hdr);
    if !cabac {
        w.ue(n); // mb_skip_run
        w.trailing();
        return w.into_bytes();
    }
    let mut ac = CabacEnc::new(w, 1, qp);
    for i in 0..n {
        // Neighbours are skipped too: context increment 0.
        ac.skip_flag(true, 0, true);
        ac.terminate(i + 1 == n);
    }
    ac.finish()
}

/// A slice's band of the reconstruction and macroblock info.
pub struct Band<'a> {
    pub mb_y0: usize,
    pub mb_y1: usize,
    pub y: &'a mut [u8],
    pub u: &'a mut [u8],
    pub v: &'a mut [u8],
    pub info: &'a mut [MbInfo],
}

/// Per-slice statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct SliceStats {
    pub intra: u32,
    pub inter: u32,
    pub skip: u32,
}

/// λ for SAD/SATD + bits costs.
pub fn lambda(qp: u8) -> u32 {
    let l = 0.92 * 2f64.powf((f64::from(qp) - 12.0) / 6.0);
    l.round().max(1.0) as u32
}

/// λ of rate-distortion costs (SSD + λ·bits), in 1/16: 0.85·2^((qp − 12)/3).
pub fn lambda_rd(qp: u8) -> u64 {
    let l = 0.85 * 2f64.powf((f64::from(qp) - 12.0) / 3.0) * 16.0;
    l.round().max(1.0) as u64
}

/// The levels of one macroblock, ready to write.
#[derive(Clone)]
struct Coded {
    kind: MbKind,
    /// B macroblock type: 0 B_Direct_16x16, 1 L0, 2 L1, 3 Bi.
    b_type: u8,
    /// Intra16x16 prediction mode, or the chroma-only fields for others.
    mode16: u8,
    chroma_mode: u8,
    /// Intra4x4: (prev_intra4x4_pred_mode_flag, rem_intra4x4_pred_mode) per luma4x4BlkIdx.
    i4: [(bool, u8); 16],
    mvd: [[i32; 2]; 2],
    cbp_luma: u8,
    cbp_chroma: u8,
    /// The 8×8 transform (levels in `ac`, see [`Coded::ac8`]).
    t8: bool,
    /// Intra16x16 DC levels, scan order.
    dc: [i16; 16],
    /// Luma levels per raster 4×4 block, scan order (index 0 unused for Intra16x16); with
    /// the 8×8 transform per 8×8 block instead (64 each, 8×8 scan order).
    ac: [[i16; 16]; 16],
    /// Chroma DC levels per component.
    cdc: [[i16; 4]; 2],
    /// Chroma AC levels per component and raster 4×4 block, scan order (index 0 unused).
    cac: [[[i16; 16]; 4]; 2],
}

impl Coded {
    /// Levels of 8×8 block `b8` (8×8 scan order) when `t8`.
    #[inline]
    fn ac8(&self, b8: usize) -> &[i16] {
        &self.ac.as_flattened()[b8 * 64..b8 * 64 + 64]
    }

    #[inline]
    fn ac8_mut(&mut self, b8: usize) -> &mut [i16] {
        &mut self.ac.as_flattened_mut()[b8 * 64..b8 * 64 + 64]
    }

    fn new(kind: MbKind) -> Coded {
        Coded {
            kind,
            b_type: 0,
            mode16: 0,
            chroma_mode: 0,
            i4: [(true, 0); 16],
            mvd: [[0; 2]; 2],
            cbp_luma: 0,
            cbp_chroma: 0,
            t8: false,
            dc: [0; 16],
            ac: [[0; 16]; 16],
            cdc: [[0; 4]; 2],
            cac: [[[0; 16]; 4]; 2],
        }
    }
}

/// The coded chroma of an intra macroblock.
struct Chroma {
    mode: u8,
    dc: [[i16; 4]; 2],
    ac: [[[i16; 16]; 4]; 2],
    cbp: u8,
}

impl Chroma {
    fn apply(&self, c: &mut Coded) {
        c.chroma_mode = self.mode;
        c.cdc = self.dc;
        c.cac = self.ac;
        c.cbp_chroma = self.cbp;
    }
}

/// Neighbour availability of the current macroblock inside its slice.
#[derive(Clone, Copy)]
struct Avail {
    left: bool,
    top: bool,
    tr: bool,
    tl: bool,
}

/// An inter prediction: lists used and motion vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Motion {
    pred: u8,
    mv: [[i32; 2]; 2],
}

struct SliceEnc<'p, 'b, 'a, 'c> {
    pic: &'p Picture<'a>,
    band: &'b mut Band<'c>,
    qp: u8,
    qpc: u8,
    lambda: u32,
    /// [`lambda_rd`].
    lambda2: u64,
    /// CAVLC output (and the slice header before CABAC starts).
    w: BitWriter,
    ac: Option<CabacEnc>,
    skip_run: u32,
    stats: SliceStats,
}

/// Encodes the slice covering `band` and returns its RBSP (slice header included).
pub fn encode_slice(
    pic: &Picture<'_>,
    band: &mut Band<'_>,
    hdr: SliceHeader,
) -> (Vec<u8>, SliceStats) {
    let qp = pic.qp;
    let mut w = BitWriter::with_capacity(64 << 10);
    write_slice_header(
        &mut w,
        (band.mb_y0 * pic.mb_w) as u32,
        pic.kind,
        pic.weights.as_ref(),
        pic.cabac,
        qp,
        &hdr,
    );
    let (w, ac) = if pic.cabac {
        let set = if pic.kind == SliceKind::I { 0 } else { 1 };
        (BitWriter::new(), Some(CabacEnc::new(w, set, qp)))
    } else {
        (w, None)
    };
    let mut enc = SliceEnc {
        pic,
        band,
        qp,
        qpc: CHROMA_QP[usize::from(qp)],
        lambda: lambda(qp),
        lambda2: lambda_rd(qp),
        w,
        ac,
        skip_run: 0,
        stats: SliceStats::default(),
    };
    let (y0, y1) = (enc.band.mb_y0, enc.band.mb_y1);
    for my in y0..y1 {
        for mx in 0..pic.mb_w {
            enc.macroblock(mx, my);
            if let Some(ac) = enc.ac.as_mut() {
                ac.terminate(my + 1 == y1 && mx + 1 == pic.mb_w); // end_of_slice_flag
            }
        }
    }
    let stats = enc.stats;
    if let Some(ac) = enc.ac.take() {
        return (ac.finish(), stats);
    }
    if enc.skip_run > 0 {
        enc.w.ue(enc.skip_run);
    }
    enc.w.trailing();
    (enc.w.into_bytes(), stats)
}

/// Decimation score of a block's levels (scan order, 16 or 64 of them; `skip_dc`: from
/// position 1): small isolated ±1 levels cost more bits than they return in quality. 99 =
/// keep. Each ±1 scores by the run of zeros before it: 3 for none, 2 for 1–2, 1 for 3–5.
fn decimate_score(levels: &[i16], skip_dc: bool) -> u32 {
    let (mut nz, mut big) = (0u64, 0u64);
    for (k, c) in levels.chunks_exact(16).enumerate() {
        let v = Simd::<i16, 16>::from_slice(c);
        nz |= v.simd_ne(Simd::splat(0)).to_bitmask() << (16 * k);
        big |= v.abs().simd_gt(Simd::splat(1)).to_bitmask() << (16 * k);
    }
    let s = u32::from(skip_dc);
    let (mut nz, big) = (nz >> s, big >> s);
    if big != 0 {
        return 99;
    }
    let (mut score, mut next) = (0, 0);
    while nz != 0 {
        let p = nz.trailing_zeros();
        score += match p - next {
            0 => 3,
            1..=2 => 2,
            3..=5 => 1,
            _ => 0,
        };
        next = p + 1;
        nz &= nz - 1;
    }
    score
}

/// Inverse of a scan: the scan position of each raster position.
const fn unzig<const N: usize>(z: [usize; N]) -> [usize; N] {
    let mut o = [0; N];
    let mut i = 0;
    while i < N {
        o[z[i]] = i;
        i += 1;
    }
    o
}

const UNZIG: [usize; 16] = unzig(ZIGZAG);
const UNZIG8: [usize; 64] = unzig(ZIGZAG8);

/// A 4×4 block's raster coefficients in scan order ([`ZIGZAG`]).
#[inline(always)]
fn scan4<T: SimdElement>(v: [T; 16]) -> [T; 16] {
    simd_swizzle!(Simd::from_array(v), ZIGZAG).to_array()
}

/// Scan order back to raster.
#[inline(always)]
fn unscan4<T: SimdElement>(v: [T; 16]) -> [T; 16] {
    simd_swizzle!(Simd::from_array(v), UNZIG).to_array()
}

#[inline(always)]
fn scan8<T: SimdElement>(v: [T; 64]) -> [T; 64] {
    simd_swizzle!(Simd::from_array(v), ZIGZAG8).to_array()
}

#[inline(always)]
fn unscan8<T: SimdElement>(v: [T; 64]) -> [T; 64] {
    simd_swizzle!(Simd::from_array(v), UNZIG8).to_array()
}

/// Stores 4×4 luma levels (raster blocks, raster coefficients) in `c`, whose luma levels
/// are all 0.
fn set_luma4(lv: &[[i16; 16]; 16], cbp: u8, c: &mut Coded) {
    c.t8 = false;
    c.cbp_luma = cbp;
    for (d, l) in c.ac.iter_mut().zip(lv) {
        if l.iter().any(|&v| v != 0) {
            *d = scan4(*l);
        }
    }
}

/// Removes the luma levels of inter macroblock `c`.
fn clear_luma(c: &mut Coded) {
    c.t8 = false;
    c.cbp_luma = 0;
    c.ac = [[0; 16]; 16];
}

fn count_nz(levels: &[i16]) -> u8 {
    levels.iter().filter(|&&l| l != 0).count() as u8
}

#[inline]
fn in_range(r: (i32, i32, i32, i32), m: [i32; 2]) -> bool {
    m[0] >= r.0 && m[0] <= r.1 && m[1] >= r.2 && m[1] <= r.3
}

/// MinPositive (8-191).
#[inline]
fn min_positive(a: i32, b: i32) -> i32 {
    if a >= 0 && b >= 0 { a.min(b) } else { a.max(b) }
}

impl SliceEnc<'_, '_, '_, '_> {
    fn ls(&self) -> usize {
        self.pic.mb_w * 16
    }
    fn cs(&self) -> usize {
        self.pic.mb_w * 8
    }

    fn avail(&self, mx: usize, my: usize) -> Avail {
        let top = my > self.band.mb_y0;
        Avail {
            left: mx > 0,
            top,
            tr: top && mx + 1 < self.pic.mb_w,
            tl: top && mx > 0,
        }
    }

    fn idx(&self, mx: usize, my: usize) -> usize {
        (my - self.band.mb_y0) * self.pic.mb_w + mx
    }

    fn info_at(&self, mx: usize, my: usize) -> &MbInfo {
        &self.band.info[self.idx(mx, my)]
    }

    /// Offset of luma sample (x, y) (picture coordinates) in the band.
    #[inline]
    fn loff(&self, x: usize, y: usize) -> usize {
        (y - self.band.mb_y0 * 16) * self.ls() + x
    }
    #[inline]
    fn coff(&self, x: usize, y: usize) -> usize {
        (y - self.band.mb_y0 * 8) * self.cs() + x
    }

    fn source(&self, mx: usize, my: usize) -> ([u8; 256], [[u8; 64]; 2]) {
        let ls = self.ls();
        let cs = self.cs();
        let mut y = [0u8; 256];
        for r in 0..16 {
            let o = (my * 16 + r) * ls + mx * 16;
            y[r * 16..r * 16 + 16].copy_from_slice(&self.pic.src[0][o..o + 16]);
        }
        let mut c = [[0u8; 64]; 2];
        for (k, cc) in c.iter_mut().enumerate() {
            for r in 0..8 {
                let o = (my * 8 + r) * cs + mx * 8;
                cc[r * 8..r * 8 + 8].copy_from_slice(&self.pic.src[1 + k][o..o + 8]);
            }
        }
        (y, c)
    }

    fn edge16(&self, mx: usize, my: usize, a: Avail) -> EdgeN<16> {
        let (x0, y0) = (mx * 16, my * 16);
        let mut e = EdgeN {
            top: [0; 16],
            left: [0; 16],
            tl: 0,
            has_top: a.top,
            has_left: a.left,
            has_tl: a.tl,
        };
        if a.top {
            let o = self.loff(x0, y0 - 1);
            e.top.copy_from_slice(&self.band.y[o..o + 16]);
        }
        if a.left {
            for r in 0..16 {
                e.left[r] = self.band.y[self.loff(x0 - 1, y0 + r)];
            }
        }
        if a.tl {
            e.tl = self.band.y[self.loff(x0 - 1, y0 - 1)];
        }
        e
    }

    fn edge_c(&self, plane: usize, mx: usize, my: usize, a: Avail) -> EdgeN<8> {
        let (x0, y0) = (mx * 8, my * 8);
        let p: &[u8] = if plane == 0 { self.band.u } else { self.band.v };
        let mut e = EdgeN {
            top: [0; 8],
            left: [0; 8],
            tl: 0,
            has_top: a.top,
            has_left: a.left,
            has_tl: a.tl,
        };
        if a.top {
            let o = self.coff(x0, y0 - 1);
            e.top.copy_from_slice(&p[o..o + 8]);
        }
        if a.left {
            for r in 0..8 {
                e.left[r] = p[self.coff(x0 - 1, y0 + r)];
            }
        }
        if a.tl {
            e.tl = p[self.coff(x0 - 1, y0 - 1)];
        }
        e
    }

    /// Neighbours of 4×4 block (bx, by) of macroblock (mx, my), from the reconstruction
    /// (which already holds the earlier blocks of this macroblock).
    fn edge4(&self, mx: usize, my: usize, bx: usize, by: usize, a: Avail) -> Edge4 {
        let x0 = mx * 16 + bx * 4;
        let y0 = my * 16 + by * 4;
        let has_left = bx > 0 || a.left;
        let has_top = by > 0 || a.top;
        let has_tl = match (bx > 0, by > 0) {
            (true, true) => true,
            (false, true) => a.left,
            (true, false) => a.top,
            (false, false) => a.tl,
        };
        let has_tr = if by == 0 {
            if bx < 3 { a.top } else { a.tr }
        } else if bx == 3 {
            false
        } else {
            RASTER_TO_BLK[(by - 1) * 4 + bx + 1] < RASTER_TO_BLK[by * 4 + bx]
        };
        let mut e = Edge4 {
            has_top,
            has_left,
            has_tl,
            ..Edge4::default()
        };
        if has_top {
            let o = self.loff(x0, y0 - 1);
            e.top[..4].copy_from_slice(&self.band.y[o..o + 4]);
            if has_tr {
                e.top[4..].copy_from_slice(&self.band.y[o + 4..o + 8]);
            } else {
                let t3 = e.top[3];
                e.top[4..].fill(t3);
            }
        }
        if has_left {
            for r in 0..4 {
                e.left[r] = self.band.y[self.loff(x0 - 1, y0 + r)];
            }
        }
        if has_tl {
            e.tl = self.band.y[self.loff(x0 - 1, y0 - 1)];
        }
        e
    }

    fn macroblock(&mut self, mx: usize, my: usize) {
        let a = self.avail(mx, my);
        let (src, csrc) = self.source(mx, my);
        let coded = match self.pic.kind {
            SliceKind::I => self.intra_mb(mx, my, a, &src, &csrc, u32::MAX),
            SliceKind::P => self.p_mb(mx, my, a, &src, &csrc),
            SliceKind::B => self.b_mb(mx, my, a, &src, &csrc),
        };
        let Some(coded) = coded else {
            self.stats.skip += 1;
            self.write_skip(mx, my, a);
            return;
        };
        if coded.kind == MbKind::Inter {
            self.stats.inter += 1;
        } else {
            self.stats.intra += 1;
        }
        self.finish_info(mx, my, &coded);
        if self.ac.is_some() {
            self.write_mb_cabac(mx, my, a, &coded);
        } else {
            self.write_mb_cavlc(mx, my, a, &coded);
        }
    }

    // ---- intra -------------------------------------------------------------------------

    /// Chooses and codes an intra macroblock (prediction written into the reconstruction).
    /// With a finite `bound` (the inter cost), returns `None` without touching the
    /// reconstruction when Intra16x16 already costs more.
    fn intra_mb(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
        bound: u32,
    ) -> Option<Coded> {
        let lambda = self.lambda;
        // Intra16x16 by SATD.
        let e16 = self.edge16(mx, my, a);
        let mut best16 = (u32::MAX, 2u8);
        let mut p16 = [0u8; 256];
        for m in 0..4 {
            if !pred::mode16_ok(m, &e16) {
                continue;
            }
            pred::pred16(m, &e16, &mut p16);
            let c = satd16(src, &p16) + lambda * 4;
            if c < best16.0 {
                best16 = (c, m);
            }
        }
        if best16.0 >= bound {
            return None;
        }
        let ch = self.intra_chroma(mx, my, a, csrc);
        let mut coded = self.intra_luma_rd(mx, my, a, src, &e16, best16, &ch);
        ch.apply(&mut coded);
        Some(coded)
    }

    /// Chroma prediction mode by SATD over both components, coded: (mode, DC and AC levels,
    /// cbp).
    fn intra_chroma(&mut self, mx: usize, my: usize, a: Avail, csrc: &[[u8; 64]; 2]) -> Chroma {
        let ec = [self.edge_c(0, mx, my, a), self.edge_c(1, mx, my, a)];
        let mut bestc = (u32::MAX, 0u8);
        let mut pc = [[0u8; 64]; 2];
        for m in 0..4 {
            if !pred::mode_c_ok(m, &ec[0]) {
                continue;
            }
            let mut c = 0;
            for k in 0..2 {
                pred::pred_chroma(m, &ec[k], &mut pc[k]);
                c += satd8(&csrc[k], &pc[k]);
            }
            if c < bestc.0 {
                bestc = (c, m);
            }
        }
        for k in 0..2 {
            pred::pred_chroma(bestc.1, &ec[k], &mut pc[k]);
        }
        let mut c = Coded::new(MbKind::I16);
        self.code_chroma(mx, my, csrc, &pc, true, &mut c);
        Chroma {
            mode: bestc.1,
            dc: c.cdc,
            ac: c.cac,
            cbp: c.cbp_chroma,
        }
    }

    /// The luma of an intra macroblock by rate-distortion cost: Intra16x16 (`m16`: SATD cost
    /// and mode), Intra4x4 and, with the 8×8 transform, Intra8x8, each coded in full unless
    /// its Hadamard cost runs clearly over the best one's. Leaves the
    /// winner's reconstruction and prediction modes in place.
    #[allow(clippy::too_many_arguments)]
    fn intra_luma_rd(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        e16: &EdgeN<16>,
        m16: (u32, u8),
        ch: &Chroma,
    ) -> Coded {
        let (cost16, m16) = m16;
        let idx = self.idx(mx, my);
        let base = self.band.info[idx];
        // (rd cost, coded, modes, reconstruction)
        let mut best: Option<(u64, Coded, [u8; 16], [u8; 256])> = None;
        let mut consider = |s: &mut Self, mut c: Coded, modes: [u8; 16]| {
            ch.apply(&mut c);
            let cur = coded_info(&c, base);
            let bits = s.mb_cost(mx, my, a, &c, &cur);
            let j = s.rd(s.luma_ssd(mx, my, src), bits);
            if best.as_ref().is_none_or(|b| j < b.0) {
                best = Some((j, c, modes, s.luma_recon(mx, my)));
            }
        };
        let mut c16 = Coded::new(MbKind::I16);
        c16.mode16 = m16;
        let mut p16 = [0u8; 256];
        pred::pred16(m16, e16, &mut p16);
        self.code_luma16(mx, my, src, &p16, &mut c16);
        consider(self, c16, [2; 16]);
        // Intra8x8 and Intra4x4 are given up once their Hadamard cost is 25 % above the best
        // so far (Intra16x16's included): their full coding is the costly part, and they
        // rarely win from there (1.1 and 1.5 measured no better on slideshows).
        let stop = |c: u32| c.saturating_add(c / 4);
        let mut best_satd = cost16;
        if self.pic.t8
            && let Some((c8, modes, cost8)) = self.code_i8(mx, my, a, src, stop(best_satd))
        {
            best_satd = best_satd.min(cost8);
            consider(self, c8, modes);
        }
        if let Some((c4, modes, _)) = self.code_i4(mx, my, a, src, stop(best_satd)) {
            consider(self, c4, modes);
        }
        let (_, c, modes, rec) = best.unwrap();
        self.set_luma_recon(mx, my, &rec);
        self.band.info[idx].modes = modes;
        c
    }

    /// Levels of a transformed 4×4 block (raster order) of category `cat` at `qp`: by
    /// trellis when enabled, else by rounding (`intra` picks the offset). AC categories leave
    /// position 0 alone.
    fn quant4(&self, w: &[i32; 16], qp: u8, intra: bool, cat: usize) -> [i16; 16] {
        let ac = cat == CAT_LUMA_AC || cat == CAT_CHROMA_AC;
        let Some(enc) = self.ac.as_ref().filter(|_| self.pic.trellis) else {
            return quant4(w, qp, intra, ac);
        };
        let x = scan4(frac4(w, qp));
        let mut l = [0i16; 16];
        let s = usize::from(ac);
        trellis::quantise(
            enc.contexts(),
            cat,
            None,
            &x[s..],
            self.trellis_lambda(qp),
            &mut l[s..],
        );
        // Position 0 of `l` stays 0 for AC categories.
        unscan4(l)
    }

    /// As [`Self::quant4`] for 8×8 luma blocks.
    fn quant8(&self, w: &[i32; 64], intra: bool) -> [i16; 64] {
        let Some(enc) = self.ac.as_ref().filter(|_| self.pic.trellis) else {
            return quant8(w, self.qp, intra);
        };
        let x = scan8(frac8(w, self.qp));
        let mut l = [0i16; 64];
        trellis::quantise(
            enc.contexts(),
            CAT_LUMA_8X8,
            None,
            &x,
            self.trellis_lambda(self.qp),
            &mut l,
        );
        unscan8(l)
    }

    /// The trellis' λ: squared level errors per 1/256 bit at `qp`. Three quarters of the mode
    /// decisions' λ measured best (the range 0.5–1 is within 0.5 % on slideshows).
    fn trellis_lambda(&self, qp: u8) -> f32 {
        self.lambda2 as f32 * 0.75 / 4096.0 / qstep2(qp)
    }

    /// Rate-distortion cost of `ssd` and `bits` (1/256 bits), in SSD/4096 units.
    #[inline]
    fn rd(&self, ssd: u64, bits: u32) -> u64 {
        (ssd << 12) + self.lambda2 * u64::from(bits)
    }

    /// SSD of the macroblock's luma reconstruction against `src`.
    fn luma_ssd(&self, mx: usize, my: usize, src: &[u8; 256]) -> u64 {
        let o = self.loff(mx * 16, my * 16);
        ssd(src, 16, &self.band.y[o..], self.ls(), 16, 16)
    }

    /// SSD of the macroblock's chroma reconstruction against `csrc`.
    fn chroma_ssd(&self, mx: usize, my: usize, csrc: &[[u8; 64]; 2]) -> u64 {
        let o = self.coff(mx * 8, my * 8);
        let cs = self.cs();
        ssd(&csrc[0], 8, &self.band.u[o..], cs, 8, 8)
            + ssd(&csrc[1], 8, &self.band.v[o..], cs, 8, 8)
    }

    fn luma_recon(&self, mx: usize, my: usize) -> [u8; 256] {
        let (ls, o) = (self.ls(), self.loff(mx * 16, my * 16));
        let mut r = [0u8; 256];
        for y in 0..16 {
            r[y * 16..y * 16 + 16].copy_from_slice(&self.band.y[o + y * ls..o + y * ls + 16]);
        }
        r
    }

    fn set_luma_recon(&mut self, mx: usize, my: usize, r: &[u8; 256]) {
        let (ls, o) = (self.ls(), self.loff(mx * 16, my * 16));
        for y in 0..16 {
            self.band.y[o + y * ls..o + y * ls + 16].copy_from_slice(&r[y * 16..y * 16 + 16]);
        }
    }

    /// The predicted Intra4x4/8x8 modes of the left and top neighbour macroblocks.
    fn neighbour_modes(
        &self,
        mx: usize,
        my: usize,
        a: Avail,
    ) -> (Option<[u8; 16]>, Option<[u8; 16]>) {
        (
            a.left.then(|| self.info_at(mx - 1, my).modes),
            a.top.then(|| self.info_at(mx, my - 1).modes),
        )
    }

    /// Intra4x4 luma, block by block (each needs its predecessors' reconstruction), modes by
    /// SATD + λ·bits. Gives up (`None`) once the cost reaches `stop`. Returns the levels,
    /// the modes (raster) and the cost.
    fn code_i4(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        stop: u32,
    ) -> Option<(Coded, [u8; 16], u32)> {
        let lambda = self.lambda;
        let mut c4 = Coded::new(MbKind::I4);
        let mut cost4 = lambda * 24;
        let mut modes = [2u8; 16];
        let (left_modes, top_modes) = self.neighbour_modes(mx, my, a);
        let ls = self.ls();
        for blk in 0..16 {
            let (bx, by) = BLK_XY[blk];
            let ma = if bx > 0 {
                Some(modes[by * 4 + bx - 1])
            } else {
                left_modes.map(|m| m[by * 4 + 3])
            };
            let mb = if by > 0 {
                Some(modes[(by - 1) * 4 + bx])
            } else {
                top_modes.map(|m| m[12 + bx])
            };
            let pm = match (ma, mb) {
                (Some(x), Some(y)) => x.min(y),
                _ => 2,
            };
            let e = self.edge4(mx, my, bx, by, a);
            let sblk = &src[by * 4 * 16 + bx * 4..];
            let mut best = (u32::MAX, 2u8, [0u8; 16]);
            for m in 0..9u8 {
                if !pred::mode4_ok(m, &e) {
                    continue;
                }
                let p = pred::pred4(m, &e);
                let bits = if m == pm { 1 } else { 4 };
                let c = crate::dsp::satd4(sblk, 16, &p, 4) + lambda * bits;
                if c < best.0 {
                    best = (c, m, p);
                }
            }
            cost4 += best.0;
            if cost4 >= stop {
                return None;
            }
            let m = best.1;
            modes[by * 4 + bx] = m;
            c4.i4[blk] = if m == pm {
                (true, 0)
            } else {
                (false, if m < pm { m } else { m - 1 })
            };
            // Residual, levels, reconstruction.
            let lv = self.quant4(
                &fdct4_diff(sblk, 16, &best.2, 4),
                self.qp,
                true,
                CAT_LUMA_4X4,
            );
            let o = self.loff(mx * 16 + bx * 4, my * 16 + by * 4);
            for y in 0..4 {
                self.band.y[o + y * ls..o + y * ls + 4].copy_from_slice(&best.2[y * 4..y * 4 + 4]);
            }
            if lv.iter().any(|&v| v != 0) {
                idct4_add(&dequant4(&lv, self.qp, false), &mut self.band.y[o..], ls);
                let r = by * 4 + bx;
                c4.ac[r] = scan4(lv);
                c4.cbp_luma |= 1 << (blk / 4);
            }
        }
        Some((c4, modes, cost4))
    }

    /// Neighbours of 8×8 block (bx, by) of macroblock (mx, my), from the reconstruction.
    fn edge8(&self, mx: usize, my: usize, bx: usize, by: usize, a: Avail) -> Edge8 {
        let x0 = mx * 16 + bx * 8;
        let y0 = my * 16 + by * 8;
        let has_left = bx > 0 || a.left;
        let has_top = by > 0 || a.top;
        let has_tl = match (bx > 0, by > 0) {
            (true, true) => true,
            (false, true) => a.left,
            (true, false) => a.top,
            (false, false) => a.tl,
        };
        let has_tr = match (bx, by) {
            (0, 0) => a.top,
            (1, 0) => a.tr,
            (0, _) => true,
            _ => false,
        };
        let mut e = Edge8 {
            has_top,
            has_left,
            has_tl,
            ..Edge8::default()
        };
        if has_top {
            let o = self.loff(x0, y0 - 1);
            e.top[..8].copy_from_slice(&self.band.y[o..o + 8]);
            if has_tr {
                e.top[8..].copy_from_slice(&self.band.y[o + 8..o + 16]);
            } else {
                let t7 = e.top[7];
                e.top[8..].fill(t7);
            }
        }
        if has_left {
            for r in 0..8 {
                e.left[r] = self.band.y[self.loff(x0 - 1, y0 + r)];
            }
        }
        if has_tl {
            e.tl = self.band.y[self.loff(x0 - 1, y0 - 1)];
        }
        e
    }

    /// Intra8x8 luma (8×8 transform), block by block, modes by SA8D + λ·bits. Gives up
    /// (`None`) once the cost reaches `stop`. Returns the levels, the modes (raster 4×4, each
    /// 8×8 block's mode four times) and the cost.
    fn code_i8(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        stop: u32,
    ) -> Option<(Coded, [u8; 16], u32)> {
        let lambda = self.lambda;
        let mut cost8 = lambda * 6;
        let mut c = Coded::new(MbKind::I4);
        c.t8 = true;
        let mut modes = [2u8; 16];
        let (left_modes, top_modes) = self.neighbour_modes(mx, my, a);
        let ls = self.ls();
        for b8 in 0..4 {
            let (bx, by) = (b8 % 2, b8 / 2);
            // Neighbouring 4×4 blocks left of and above the block's top-left one.
            let ma = if bx > 0 {
                Some(modes[by * 8])
            } else {
                left_modes.map(|m| m[by * 8 + 3])
            };
            let mb = if by > 0 {
                Some(modes[bx * 2])
            } else {
                top_modes.map(|m| m[12 + bx * 2])
            };
            let pm = match (ma, mb) {
                (Some(x), Some(y)) => x.min(y),
                _ => 2,
            };
            let e = self.edge8(mx, my, bx, by, a);
            let f = pred::filter8(&e);
            let sblk = &src[by * 8 * 16 + bx * 8..];
            let mut best = (u32::MAX, 2u8, [0u8; 64]);
            for m in 0..9u8 {
                if !pred::mode8_ok(m, &e) {
                    continue;
                }
                let p = pred::pred8(m, &e, &f);
                let bits = if m == pm { 1 } else { 4 };
                let cost = sa8d8(sblk, 16, &p, 8) + lambda * bits;
                if cost < best.0 {
                    best = (cost, m, p);
                }
            }
            cost8 += best.0;
            if cost8 >= stop {
                return None;
            }
            let m = best.1;
            for r in [0, 1, 4, 5] {
                modes[by * 8 + bx * 2 + r] = m;
            }
            c.i4[b8] = if m == pm {
                (true, 0)
            } else {
                (false, if m < pm { m } else { m - 1 })
            };
            let mut res = [0i32; 64];
            for y in 0..8 {
                for x in 0..8 {
                    res[y * 8 + x] = i32::from(sblk[y * 16 + x]) - i32::from(best.2[y * 8 + x]);
                }
            }
            let lv = self.quant8(&fdct8(&res), true);
            let o = self.loff(mx * 16 + bx * 8, my * 16 + by * 8);
            for y in 0..8 {
                self.band.y[o + y * ls..o + y * ls + 8].copy_from_slice(&best.2[y * 8..y * 8 + 8]);
            }
            if lv.iter().any(|&v| v != 0) {
                idct8_add(&dequant8(&lv, self.qp), &mut self.band.y[o..], ls);
                c.ac8_mut(b8).copy_from_slice(&scan8(lv));
                c.cbp_luma |= 1 << b8;
            }
        }
        Some((c, modes, cost8))
    }

    /// Intra16x16 luma: transforms, DC Hadamard, levels and reconstruction.
    fn code_luma16(
        &mut self,
        mx: usize,
        my: usize,
        src: &[u8; 256],
        pred: &[u8; 256],
        c: &mut Coded,
    ) {
        let qp = self.qp;
        let mut w = [[0i32; 16]; 16]; // per raster block
        let mut dc = [0i32; 16];
        for r in 0..16 {
            let (bx, by) = (r % 4, r / 4);
            let i = by * 4 * 16 + bx * 4;
            w[r] = fdct4_diff(&src[i..], 16, &pred[i..], 16);
            dc[r] = w[r][0];
        }
        let hd = hadamard4(&dc).map(|v| v >> 1);
        let mut dcl = [0i16; 16];
        quant_dc(&hd, &mut dcl, qp, true);
        c.dc = scan4(dcl);
        let dcy = dequant_luma_dc(&dcl, qp);
        let ls = self.ls();
        let mut any_ac = false;
        let mut levels = [[0i16; 16]; 16];
        for r in 0..16 {
            levels[r] = self.quant4(&w[r], qp, true, CAT_LUMA_AC);
            any_ac |= levels[r].iter().any(|&v| v != 0);
        }
        for r in 0..16 {
            let (bx, by) = (r % 4, r / 4);
            let o = self.loff(mx * 16 + bx * 4, my * 16 + by * 4);
            for y in 0..4 {
                let s = (by * 4 + y) * 16 + bx * 4;
                self.band.y[o + y * ls..o + y * ls + 4].copy_from_slice(&pred[s..s + 4]);
            }
            let mut d = dequant4(&levels[r], qp, true);
            d[0] = dcy[r];
            if d.iter().any(|&v| v != 0) {
                idct4_add(&d, &mut self.band.y[o..], ls);
            }
            c.ac[r] = scan4(levels[r]);
        }
        c.cbp_luma = if any_ac { 15 } else { 0 };
    }

    /// Chroma residual of both components against `pred`: levels, decimation (inter only)
    /// and reconstruction.
    fn code_chroma(
        &mut self,
        mx: usize,
        my: usize,
        csrc: &[[u8; 64]; 2],
        pred: &[[u8; 64]; 2],
        intra: bool,
        c: &mut Coded,
    ) {
        let qpc = self.qpc;
        let cs = self.cs();
        let mut any_ac = false;
        let mut any_dc = false;
        let mut lv = [[[0i16; 16]; 4]; 2];
        for k in 0..2 {
            let mut dc = [0i32; 4];
            let mut w = [[0i32; 16]; 4];
            for b in 0..4 {
                let (bx, by) = (b % 2, b / 2);
                let i = by * 4 * 8 + bx * 4;
                w[b] = fdct4_diff(&csrc[k][i..], 8, &pred[k][i..], 8);
                dc[b] = w[b][0];
            }
            quant_dc(&hadamard2(&dc), &mut c.cdc[k], qpc, intra);
            any_dc |= c.cdc[k].iter().any(|&v| v != 0);
            let mut score = 0;
            for b in 0..4 {
                lv[k][b] = self.quant4(&w[b], qpc, intra, CAT_CHROMA_AC);
                let scan = scan4(lv[k][b]);
                score += decimate_score(&scan, true);
                c.cac[k][b] = scan;
            }
            if !intra && score < 7 {
                lv[k] = [[0; 16]; 4];
                c.cac[k] = [[0; 16]; 4];
            }
            any_ac |= lv[k].iter().any(|b| b.iter().any(|&v| v != 0));
        }
        c.cbp_chroma = if any_ac {
            2
        } else if any_dc {
            1
        } else {
            0
        };
        for k in 0..2 {
            let dcs = dequant_chroma_dc(&c.cdc[k], qpc);
            let plane: &mut [u8] = if k == 0 { self.band.u } else { self.band.v };
            let o0 = (my - self.band.mb_y0) * 8 * cs + mx * 8;
            for b in 0..4 {
                let (bx, by) = (b % 2, b / 2);
                let o = o0 + by * 4 * cs + bx * 4;
                for y in 0..4 {
                    let s = (by * 4 + y) * 8 + bx * 4;
                    plane[o + y * cs..o + y * cs + 4].copy_from_slice(&pred[k][s..s + 4]);
                }
                let mut d = dequant4(&lv[k][b], qpc, true);
                d[0] = dcs[b];
                if d.iter().any(|&v| v != 0) {
                    idct4_add(&d, &mut plane[o..], cs);
                }
            }
        }
    }

    // ---- inter -------------------------------------------------------------------------

    /// (refIdx, mv) of list `l` of neighbour macroblock (mx + dx, my + dy); `None` when it is
    /// outside the picture or slice.
    fn neighbour(
        &self,
        mx: usize,
        my: usize,
        dx: isize,
        dy: isize,
        l: usize,
    ) -> Option<(i32, [i32; 2])> {
        let x = mx as isize + dx;
        let y = my as isize + dy;
        if x < 0 || x >= self.pic.mb_w as isize || y < self.band.mb_y0 as isize {
            return None;
        }
        let n = self.info_at(x as usize, y as usize);
        Some(if n.is_intra() || n.pred & (1 << l) == 0 {
            (-1, [0, 0])
        } else {
            (0, n.mv[l])
        })
    }

    /// Neighbours A, B and C (D when C is outside) of list `l`.
    #[allow(clippy::type_complexity)]
    fn abc(&self, mx: usize, my: usize, l: usize) -> [Option<(i32, [i32; 2])>; 3] {
        let a = self.neighbour(mx, my, -1, 0, l);
        let b = self.neighbour(mx, my, 0, -1, l);
        let c = self
            .neighbour(mx, my, 1, -1, l)
            .or_else(|| self.neighbour(mx, my, -1, -1, l));
        [a, b, c]
    }

    /// The 16×16 motion vector predictor of list `l` for refIdx 0, and the P_Skip motion
    /// vector (8.4.1.1, 8.4.1.3).
    fn mv_pred(&self, mx: usize, my: usize, l: usize) -> ([i32; 2], [i32; 2]) {
        let [a, b, c] = self.abc(mx, my, l);
        let (mut bb, mut cc) = (b, c);
        if b.is_none() && c.is_none() && a.is_some() {
            bb = a;
            cc = a;
        }
        let un = (-1, [0, 0]);
        let (ra, ma) = a.unwrap_or(un);
        let (rb, mb) = bb.unwrap_or(un);
        let (rc, mc) = cc.unwrap_or(un);
        let hits = [ra == 0, rb == 0, rc == 0];
        let mvp = if hits.iter().filter(|&&h| h).count() == 1 {
            if hits[0] {
                ma
            } else if hits[1] {
                mb
            } else {
                mc
            }
        } else {
            let med = |x: i32, y: i32, z: i32| x.max(y).min(x.min(y).max(z));
            [med(ma[0], mb[0], mc[0]), med(ma[1], mb[1], mc[1])]
        };
        let skip = match (a, b) {
            (Some((ra, ma)), Some((rb, mb))) => {
                if (ra == 0 && ma == [0, 0]) || (rb == 0 && mb == [0, 0]) {
                    [0, 0]
                } else {
                    mvp
                }
            }
            _ => [0, 0],
        };
        (mvp, skip)
    }

    /// Spatial direct prediction of a B_Skip / B_Direct_16x16 macroblock (8.4.1.2.2). The
    /// co-located picture (RefPicList1[0]) is long-term, so colZeroFlag is always 0.
    fn direct_pred(&self, mx: usize, my: usize) -> Motion {
        let mut refs = [-1i32; 2];
        for (l, r) in refs.iter_mut().enumerate() {
            let n = self.abc(mx, my, l).map(|n| n.map_or(-1, |n| n.0));
            *r = min_positive(n[0], min_positive(n[1], n[2]));
        }
        if refs[0] < 0 && refs[1] < 0 {
            return Motion {
                pred: 3,
                mv: [[0; 2]; 2],
            };
        }
        let mut m = Motion {
            pred: 0,
            mv: [[0; 2]; 2],
        };
        for l in 0..2 {
            if refs[l] >= 0 {
                m.pred |= 1 << l;
                m.mv[l] = self.mv_pred(mx, my, l).0;
            }
        }
        m
    }

    fn mv_cost(&self, mv: [i32; 2], mvp: [i32; 2]) -> u32 {
        self.lambda * (se_bits(mv[0] - mvp[0]) + se_bits(mv[1] - mvp[1]))
    }

    /// Motion search of `target` in `r`: the best of the candidates in full samples, a
    /// coarse-to-fine diamond, then half and quarter sample refinement by SATD. Returns
    /// (mv, SATD + λ·bits).
    fn motion_search(
        &self,
        r: &RefFrame,
        mx: usize,
        my: usize,
        target: &[u8; 256],
        mvp: [i32; 2],
        cands: &[[i32; 2]],
    ) -> ([i32; 2], u32) {
        let range = mv_range(mx, my, r.w, r.h);
        let x0 = (mx * 16) as isize;
        let y0 = (my * 16) as isize;
        let sad_cost = |ix: i32, iy: i32| -> u32 {
            let b = r.full_at(x0 + ix as isize, y0 + iy as isize);
            sad16(target, 16, b, r.stride) + self.mv_cost([ix * 4, iy * 4], mvp)
        };
        let mut best = ([0i32; 2], u32::MAX);
        for c in std::iter::once(&mvp).chain(cands) {
            let ip = [(c[0] + 2) >> 2, (c[1] + 2) >> 2];
            if !in_range(range, [ip[0] * 4, ip[1] * 4]) || ip == best.0 && best.1 != u32::MAX {
                continue;
            }
            let cost = sad_cost(ip[0], ip[1]);
            if cost < best.1 {
                best = (ip, cost);
            }
        }
        // Diamond refinement, coarse to fine.
        for step in [4, 2, 1] {
            for _ in 0..8 {
                let mut moved = false;
                let centre = best.0;
                for (dx, dy) in [(-step, 0), (step, 0), (0, -step), (0, step)] {
                    let p = [centre[0] + dx, centre[1] + dy];
                    if !in_range(range, [p[0] * 4, p[1] * 4]) {
                        continue;
                    }
                    let cost = sad_cost(p[0], p[1]);
                    if cost < best.1 {
                        best = (p, cost);
                        moved = true;
                    }
                }
                if !moved {
                    break;
                }
            }
        }
        // Sub-sample refinement by SATD; a fractional candidate (the predictor, a hint) may
        // start closer than the full-sample winner.
        let mut pbuf = [0u8; 256];
        let satd_cost = |m: [i32; 2], pbuf: &mut [u8; 256]| -> u32 {
            r.luma(
                (mx * 64) as i32 + m[0],
                (my * 64) as i32 + m[1],
                16,
                16,
                pbuf,
            );
            satd16(target, pbuf) + self.mv_cost(m, mvp)
        };
        let mut q = [best.0[0] * 4, best.0[1] * 4];
        let mut qc = satd_cost(q, &mut pbuf);
        for &c in std::iter::once(&mvp).chain(cands) {
            if c != q && (c[0] - q[0]).abs() <= 4 && (c[1] - q[1]).abs() <= 4 && in_range(range, c)
            {
                let cc = satd_cost(c, &mut pbuf);
                if cc < qc {
                    q = c;
                    qc = cc;
                }
            }
        }
        for d in [2, 1] {
            let centre = q;
            for dy in [-d, 0, d] {
                for dx in [-d, 0, d] {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let m = [centre[0] + dx, centre[1] + dy];
                    if !in_range(range, m) {
                        continue;
                    }
                    let c = satd_cost(m, &mut pbuf);
                    if c < qc {
                        q = m;
                        qc = c;
                    }
                }
            }
        }
        (q, qc)
    }

    /// Unweighted luma and chroma prediction from `r` for motion vector `mv`.
    fn inter_pred(
        &self,
        r: &RefFrame,
        mx: usize,
        my: usize,
        mv: [i32; 2],
    ) -> ([u8; 256], [[u8; 64]; 2]) {
        let mut p = [0u8; 256];
        r.luma(
            (mx * 64) as i32 + mv[0],
            (my * 64) as i32 + mv[1],
            16,
            16,
            &mut p,
        );
        let mut pc = [[0u8; 64]; 2];
        for (k, pk) in pc.iter_mut().enumerate() {
            r.chroma(
                k,
                (mx * 64) as i32 + mv[0],
                (my * 64) as i32 + mv[1],
                8,
                8,
                pk,
            );
        }
        (p, pc)
    }

    /// The (weighted) inter prediction of `m`.
    fn mc(&self, mx: usize, my: usize, m: Motion) -> ([u8; 256], [[u8; 64]; 2]) {
        let w = self.pic.weights.as_ref();
        if m.pred == 3 {
            let (mut p, mut pc) = self.inter_pred(self.pic.refs[0].unwrap(), mx, my, m.mv[0]);
            let (p1, pc1) = self.inter_pred(self.pic.refs[1].unwrap(), mx, my, m.mv[1]);
            match w {
                Some(w) => {
                    w.apply2(0, &mut p, &p1);
                    w.apply2(1, &mut pc[0], &pc1[0]);
                    w.apply2(2, &mut pc[1], &pc1[1]);
                }
                None => {
                    average(&mut p, &p1);
                    average(&mut pc[0], &pc1[0]);
                    average(&mut pc[1], &pc1[1]);
                }
            }
            return (p, pc);
        }
        let l = usize::from(m.pred >> 1);
        let (mut p, mut pc) = self.inter_pred(self.pic.refs[l].unwrap(), mx, my, m.mv[l]);
        if let Some(w) = w {
            w.apply1(l, 0, &mut p);
            w.apply1(l, 1, &mut pc[0]);
            w.apply1(l, 2, &mut pc[1]);
        }
        (p, pc)
    }

    /// Luma inter residual levels with decimation, by trellis when enabled and `rdoq`.
    /// Returns (raster levels, cbp).
    fn inter_luma_levels(
        &self,
        src: &[u8; 256],
        pred: &[u8; 256],
        rdoq: bool,
    ) -> ([[i16; 16]; 16], u8) {
        let mut lv = [[0i16; 16]; 16];
        let mut cbp = 0u8;
        let mut total_score = 0;
        for i8 in 0..4 {
            let mut score = 0;
            for i4 in 0..4 {
                let (bx, by) = BLK_XY[i8 * 4 + i4];
                let i = by * 4 * 16 + bx * 4;
                let w = fdct4_diff(&src[i..], 16, &pred[i..], 16);
                let l = if rdoq {
                    self.quant4(&w, self.qp, false, CAT_LUMA_4X4)
                } else {
                    quant4(&w, self.qp, false, false)
                };
                score += decimate_score(&scan4(l), false);
                lv[by * 4 + bx] = l;
            }
            if score < 4 {
                for i4 in 0..4 {
                    let (bx, by) = BLK_XY[i8 * 4 + i4];
                    lv[by * 4 + bx] = [0; 16];
                }
            } else {
                cbp |= 1 << i8;
                total_score += score;
            }
        }
        if cbp != 0 && total_score < 6 {
            lv = [[0; 16]; 16];
            cbp = 0;
        }
        (lv, cbp)
    }

    /// Luma inter residual levels with the 8×8 transform (raster 8×8 blocks, raster
    /// coefficients), decimated like the 4×4 ones. Returns (levels, cbp, levels' cost).
    fn inter_luma_levels8(&self, src: &[u8; 256], pred: &[u8; 256]) -> ([[i16; 64]; 4], u8) {
        let mut lv = [[0i16; 64]; 4];
        let mut cbp = 0u8;
        let mut total_score = 0;
        for b8 in 0..4 {
            let (bx, by) = (b8 % 2, b8 / 2);
            let mut res = [0i32; 64];
            for y in 0..8 {
                for x in 0..8 {
                    let i = (by * 8 + y) * 16 + bx * 8 + x;
                    res[y * 8 + x] = i32::from(src[i]) - i32::from(pred[i]);
                }
            }
            let l = self.quant8(&fdct8(&res), false);
            let score = decimate_score(&scan8(l), false);
            if score >= 4 {
                lv[b8] = l;
                cbp |= 1 << b8;
                total_score += score;
            }
        }
        if cbp != 0 && total_score < 6 {
            return ([[0; 64]; 4], 0);
        }
        (lv, cbp)
    }

    /// Would chroma quantise to nothing at this prediction (inter rounding, decimated)?
    fn chroma_zero(&self, csrc: &[[u8; 64]; 2], pred: &[[u8; 64]; 2]) -> bool {
        for k in 0..2 {
            let mut dc = [0i32; 4];
            let mut score = 0;
            for b in 0..4 {
                let (bx, by) = (b % 2, b / 2);
                let i = by * 4 * 8 + bx * 4;
                let w = fdct4_diff(&csrc[k][i..], 8, &pred[k][i..], 8);
                dc[b] = w[0];
                let l = quant4(&w, self.qpc, false, true);
                score += decimate_score(&scan4(l), true);
            }
            if score >= 7 {
                return false;
            }
            let mut q = [0i16; 4];
            quant_dc(&hadamard2(&dc), &mut q, self.qpc, false);
            if q.iter().any(|&v| v != 0) {
                return false;
            }
        }
        true
    }

    /// Whether prediction `p` leaves nothing to code (a skip).
    fn residual_free(
        &self,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
        p: &[u8; 256],
        pc: &[[u8; 64]; 2],
    ) -> bool {
        // Plain rounding: a quick test (the rate-distortion skip check follows).
        let (_, cbp) = self.inter_luma_levels(src, p, false);
        cbp == 0 && self.chroma_zero(csrc, pc)
    }

    /// Records a skipped macroblock (its prediction is already in the reconstruction).
    fn set_skip(&mut self, mx: usize, my: usize, m: Motion, direct: bool) {
        let idx = self.idx(mx, my);
        self.band.info[idx] = MbInfo {
            kind: MbKind::Skip,
            direct,
            pred: m.pred,
            mv: m.mv,
            ..MbInfo::default()
        };
    }

    /// Records the motion of a coded inter macroblock.
    fn set_motion(&mut self, mx: usize, my: usize, m: Motion, mvd: [[i32; 2]; 2], direct: bool) {
        let idx = self.idx(mx, my);
        let info = &mut self.band.info[idx];
        info.direct = direct;
        info.pred = m.pred;
        info.mv = m.mv;
        info.mvd = mvd.map(|v| v.map(|c| c.unsigned_abs().min(u32::from(u16::MAX)) as u16));
    }

    /// Integer motion candidates for list `l`: neighbours, the co-located macroblock of the
    /// previous picture, and the caller's hint (list 0).
    fn candidates(&self, mx: usize, my: usize, l: usize, out: &mut Vec<[i32; 2]>) {
        out.push([0, 0]);
        for (dx, dy) in [(-1, 0), (0, -1), (1, -1)] {
            if let Some((0, m)) = self.neighbour(mx, my, dx, dy, l) {
                out.push(m);
            }
        }
        if let Some(prev) = self.pic.prev_info {
            let p = &prev[my * self.pic.mb_w + mx];
            if !p.is_intra() && p.pred & (1 << l) != 0 {
                out.push(p.mv[l]);
            }
            // Below and right: not yet coded here, a good guess for smooth motion.
            if my + 1 < prev.len() / self.pic.mb_w {
                let q = &prev[(my + 1) * self.pic.mb_w + mx];
                if !q.is_intra() && q.pred & (1 << l) != 0 {
                    out.push(q.mv[l]);
                }
            }
        }
        if l == 0
            && let Some(h) = self.pic.hint
        {
            out.push(h.mv(mx, my));
        }
    }

    /// The source block mapped through the inverse of list `l`'s luma weight: what the
    /// unweighted reference should look like (motion search target of weighted prediction).
    fn unweighted_target(&self, src: &[u8; 256], l: usize) -> Option<[u8; 256]> {
        let w = self.pic.weights.as_ref()?;
        if w.is_default(l, 0) {
            return None;
        }
        let wp = w.wp[l][0];
        let den = 1i32 << w.luma_log2;
        if wp.w * 8 < den {
            return None;
        }
        Some(src.map(|s| (((i32::from(s) - wp.o) * den + wp.w / 2) / wp.w).clamp(0, 255) as u8))
    }

    /// The list-`l` reference block that would complete a bi-prediction whose other list
    /// predicts `other` (luma, explicit weights): the target of a refinement search.
    fn bi_target(&self, src: &[u8; 256], other: &[u8; 256], l: usize) -> Option<[u8; 256]> {
        let w = self.pic.weights.as_ref()?;
        let (a, b) = (w.wp[1 - l][0], w.wp[l][0]);
        let den = 1i32 << (w.luma_log2 + 1);
        if b.w * 8 < den {
            return None;
        }
        let o = (a.o + b.o + 1) >> 1;
        let mut t = [0u8; 256];
        for i in 0..256 {
            let v = (i32::from(src[i]) - o) * den - i32::from(other[i]) * a.w;
            t[i] = ((v + b.w / 2) / b.w).clamp(0, 255) as u8;
        }
        Some(t)
    }

    /// Codes a P slice macroblock: `None` for P_Skip (reconstruction and info written).
    fn p_mb(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
    ) -> Option<Coded> {
        let r = self.pic.refs[0].unwrap();
        let (mvp, skip_mv) = self.mv_pred(mx, my, 0);
        let range = mv_range(mx, my, r.w, r.h);
        let skip_ok = in_range(range, skip_mv);
        let one = |mv| Motion {
            pred: 1,
            mv: [mv, [0, 0]],
        };
        // P_Skip first: cheap when the picture did not change here.
        let mut skip_pred = None;
        if skip_ok {
            let (p, pc) = self.mc(mx, my, one(skip_mv));
            if self.residual_free(src, csrc, &p, &pc) {
                self.put_pred(mx, my, &p, &pc);
                self.set_skip(mx, my, one(skip_mv), false);
                return None;
            }
            skip_pred = Some((p, pc));
        }
        let mut cands = Vec::with_capacity(10);
        cands.push(skip_mv);
        self.candidates(mx, my, 0, &mut cands);
        let target = self.unweighted_target(src, 0);
        let (mv, mut cost) =
            self.motion_search(r, mx, my, target.as_ref().unwrap_or(src), mvp, &cands);
        let (p, pc) = self.mc(mx, my, one(mv));
        if target.is_some() {
            cost = satd16(src, &p) + self.mv_cost(mv, mvp);
        }
        let inter_cost = cost + self.lambda * 2;
        // Intra when it predicts better (cross-fades into new content, cuts): by SATD, then
        // by rate-distortion cost against the coded inter macroblock.
        let intra = self.intra_mb(mx, my, a, src, csrc, inter_cost);
        let mut c = Coded::new(MbKind::Inter);
        c.mvd[0] = [mv[0] - mvp[0], mv[1] - mvp[1]];
        let base = MbInfo {
            kind: MbKind::Inter,
            pred: 1,
            mv: [mv, [0, 0]],
            ..MbInfo::default()
        };
        let (c, j) = self.inter_or_intra(mx, my, a, src, csrc, &p, &pc, base, c, intra);
        if c.kind != MbKind::Inter {
            return Some(c);
        }
        if c.cbp_luma == 0 && c.cbp_chroma == 0 && mv == skip_mv && skip_ok {
            // Identical to P_Skip (prediction already in place, no residual).
            self.set_skip(mx, my, one(mv), false);
            return None;
        }
        if let Some((sp, spc)) = skip_pred {
            // P_Skip when its distortion costs less than the coded macroblock's bits.
            let d = ssd(src, 16, &sp, 16, 16, 16)
                + ssd(&csrc[0], 8, &spc[0], 8, 8, 8)
                + ssd(&csrc[1], 8, &spc[1], 8, 8, 8);
            if self.rd(d, self.skip_cost(mx, my, a)) < j {
                self.put_pred(mx, my, &sp, &spc);
                self.set_skip(mx, my, one(skip_mv), false);
                return None;
            }
        }
        self.set_motion(mx, my, one(mv), c.mvd, false);
        Some(c)
    }

    /// Codes a B slice macroblock: `None` for B_Skip.
    fn b_mb(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
    ) -> Option<Coded> {
        let (r0, r1) = (self.pic.refs[0].unwrap(), self.pic.refs[1].unwrap());
        let range = mv_range(mx, my, r0.w, r0.h);
        let lam = self.lambda;
        let sat = |p: &[u8; 256]| satd16(src, p);
        // (cost, b_type, motion)
        let mut best = (
            u32::MAX,
            0u8,
            Motion {
                pred: 3,
                mv: [[0; 2]; 2],
            },
        );
        // B_Skip / B_Direct_16x16.
        let d = self.direct_pred(mx, my);
        if (0..2).all(|l| d.pred & (1 << l) == 0 || in_range(range, d.mv[l])) {
            let (p, pc) = self.mc(mx, my, d);
            if self.residual_free(src, csrc, &p, &pc) {
                self.put_pred(mx, my, &p, &pc);
                self.set_skip(mx, my, d, true);
                return None;
            }
            best = (sat(&p) + lam, 0, d);
        }
        let (mvp0, _) = self.mv_pred(mx, my, 0);
        let (mvp1, _) = self.mv_pred(mx, my, 1);
        let mut cands = Vec::with_capacity(10);
        if d.pred & 1 != 0 {
            cands.push(d.mv[0]);
        }
        self.candidates(mx, my, 0, &mut cands);
        let (mv0, _) = self.motion_search(r0, mx, my, src, mvp0, &cands);
        let l0 = Motion {
            pred: 1,
            mv: [mv0, [0, 0]],
        };
        let (p, _) = self.mc(mx, my, l0);
        let c = sat(&p) + self.mv_cost(mv0, mvp0) + lam * 3;
        if c < best.0 {
            best = (c, 1, l0);
        }
        // Bi-prediction: list 1 refined against what list 0 leaves, then list 0 against
        // list 1.
        let mut raw0 = [0u8; 256];
        r0.luma(
            (mx * 64) as i32 + mv0[0],
            (my * 64) as i32 + mv0[1],
            16,
            16,
            &mut raw0,
        );
        cands.clear();
        if d.pred & 2 != 0 {
            cands.push(d.mv[1]);
        }
        self.candidates(mx, my, 1, &mut cands);
        let mv1 = match self.bi_target(src, &raw0, 1) {
            Some(t) => self.motion_search(r1, mx, my, &t, mvp1, &cands).0,
            None => mvp1,
        };
        let mut raw1 = [0u8; 256];
        r1.luma(
            (mx * 64) as i32 + mv1[0],
            (my * 64) as i32 + mv1[1],
            16,
            16,
            &mut raw1,
        );
        let mut mv0b = mv0;
        if let Some(t) = self.bi_target(src, &raw1, 0) {
            mv0b = self.motion_search(r0, mx, my, &t, mvp0, &[mv0]).0;
        }
        for m0 in [mv0, mv0b] {
            let bi = Motion {
                pred: 3,
                mv: [m0, mv1],
            };
            let (p, _) = self.mc(mx, my, bi);
            let c = sat(&p) + self.mv_cost(m0, mvp0) + self.mv_cost(mv1, mvp1) + lam * 5;
            if c < best.0 {
                best = (c, 3, bi);
            }
            if mv0b == mv0 {
                break;
            }
        }
        let intra = self.intra_mb(mx, my, a, src, csrc, best.0);
        let (_, b_type, m) = best;
        let (p, pc) = self.mc(mx, my, m);
        let mut c = Coded::new(MbKind::Inter);
        c.b_type = b_type;
        let mvp = [mvp0, mvp1];
        if b_type != 0 {
            for l in 0..2 {
                if m.pred & (1 << l) != 0 {
                    c.mvd[l] = [m.mv[l][0] - mvp[l][0], m.mv[l][1] - mvp[l][1]];
                }
            }
        }
        let base = MbInfo {
            kind: MbKind::Inter,
            direct: b_type == 0,
            pred: m.pred,
            mv: m.mv,
            ..MbInfo::default()
        };
        let (c, _) = self.inter_or_intra(mx, my, a, src, csrc, &p, &pc, base, c, intra);
        if c.kind != MbKind::Inter {
            return Some(c);
        }
        if b_type == 0 {
            if c.cbp_luma == 0 && c.cbp_chroma == 0 {
                self.set_skip(mx, my, m, true);
                return None;
            }
            self.set_motion(mx, my, m, [[0; 2]; 2], true);
            return Some(c);
        }
        self.set_motion(mx, my, m, c.mvd, false);
        Some(c)
    }

    /// Codes the residual of an inter macroblock (info `base`: motion; `c`: mvd and B type)
    /// against prediction (`p`, `pc`) and writes its reconstruction. The luma is chosen by
    /// rate-distortion cost (bits counted by CABAC or CAVLC) among no residual, the 4×4 and
    /// the 8×8 transform; that cost is returned.
    #[allow(clippy::too_many_arguments)]
    fn code_inter(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
        p: &[u8; 256],
        pc: &[[u8; 64]; 2],
        base: MbInfo,
        mut c: Coded,
    ) -> (Coded, u64) {
        self.put_pred(mx, my, p, pc);
        self.code_chroma(mx, my, csrc, pc, false, &mut c);
        let (lv, cbp) = self.inter_luma_levels(src, p, true);
        let cssd = self.chroma_ssd(mx, my, csrc);
        // The candidates take turns in `c` (no luma levels on entry): (rd cost, candidate)
        // of the best, and its reconstruction.
        let mut best: Option<(u64, u8)> = None;
        let mut rec = [0u8; 256];
        let mut consider = |s: &mut Self, c: &Coded, k: u8| {
            let cur = coded_info(c, base);
            let bits = s.mb_cost(mx, my, a, c, &cur);
            let j = s.rd(s.luma_ssd(mx, my, src) + cssd, bits);
            if best.is_none_or(|b| j < b.0) {
                best = Some((j, k));
                rec = s.luma_recon(mx, my);
            }
        };
        // No luma residual: the prediction is in place.
        consider(self, &c, 0);
        let mut last = 0;
        if cbp != 0 {
            self.put_luma4(mx, my, &lv, cbp, &mut c);
            consider(self, &c, 1);
            last = 1;
        }
        if self.pic.t8 {
            let (lv8, cbp8) = self.inter_luma_levels8(src, p);
            if cbp8 != 0 {
                self.put_luma_pred(mx, my, p);
                clear_luma(&mut c);
                self.put_luma8(mx, my, &lv8, cbp8, &mut c);
                consider(self, &c, 2);
                last = 2;
            }
        }
        let (j, k) = best.unwrap();
        if k != last {
            clear_luma(&mut c);
            if k == 1 {
                set_luma4(&lv, cbp, &mut c);
            }
        }
        self.set_luma_recon(mx, my, &rec);
        (c, j)
    }

    /// Adds 4×4 luma levels (raster blocks, raster coefficients) to the reconstruction (the
    /// prediction) and stores them in `c`.
    fn put_luma4(&mut self, mx: usize, my: usize, lv: &[[i16; 16]; 16], cbp: u8, c: &mut Coded) {
        let ls = self.ls();
        for rr in 0..16 {
            let (bx, by) = (rr % 4, rr / 4);
            if lv[rr].iter().all(|&v| v == 0) {
                continue;
            }
            let o = self.loff(mx * 16 + bx * 4, my * 16 + by * 4);
            idct4_add(
                &dequant4(&lv[rr], self.qp, false),
                &mut self.band.y[o..],
                ls,
            );
        }
        set_luma4(lv, cbp, c);
    }

    /// As [`Self::put_luma4`] for 8×8 levels.
    fn put_luma8(&mut self, mx: usize, my: usize, lv: &[[i16; 64]; 4], cbp: u8, c: &mut Coded) {
        let ls = self.ls();
        // Without luma levels the flag is not coded (and inferred 0).
        c.t8 = cbp != 0;
        c.cbp_luma = cbp;
        for b8 in 0..4 {
            if lv[b8].iter().all(|&v| v == 0) {
                continue;
            }
            let (bx, by) = (b8 % 2, b8 / 2);
            let o = self.loff(mx * 16 + bx * 8, my * 16 + by * 8);
            idct8_add(&dequant8(&lv[b8], self.qp), &mut self.band.y[o..], ls);
            c.ac8_mut(b8).copy_from_slice(&scan8(lv[b8]));
        }
    }

    fn put_luma_pred(&mut self, mx: usize, my: usize, p: &[u8; 256]) {
        self.set_luma_recon(mx, my, p);
    }

    /// The macroblock's reconstruction (luma, Cb, Cr).
    fn mb_recon(&self, mx: usize, my: usize) -> ([u8; 256], [[u8; 64]; 2]) {
        let (cs, o) = (self.cs(), self.coff(mx * 8, my * 8));
        let mut c = [[0u8; 64]; 2];
        for y in 0..8 {
            c[0][y * 8..y * 8 + 8].copy_from_slice(&self.band.u[o + y * cs..o + y * cs + 8]);
            c[1][y * 8..y * 8 + 8].copy_from_slice(&self.band.v[o + y * cs..o + y * cs + 8]);
        }
        (self.luma_recon(mx, my), c)
    }

    /// The cost (1/256 bits) of a skipped macroblock here.
    fn skip_cost(&self, mx: usize, my: usize, a: Avail) -> u32 {
        let Some(ac) = self.ac.as_ref() else {
            // CAVLC: a longer mb_skip_run, about the bit the coded macroblock's run of 0
            // takes.
            return 256;
        };
        let mut k = ac.cost();
        k.skip_flag(
            self.pic.kind == SliceKind::P,
            self.skip_inc(mx, my, a),
            true,
        );
        k.bits
    }

    fn skip_inc(&self, mx: usize, my: usize, a: Avail) -> usize {
        let not_skip = |n: &MbInfo| usize::from(n.kind != MbKind::Skip);
        usize::from(a.left && not_skip(self.info_at(mx - 1, my)) != 0)
            + usize::from(a.top && not_skip(self.info_at(mx, my - 1)) != 0)
    }

    /// The rate-distortion cost of intra macroblock `c` whose reconstruction is in place.
    fn intra_rd(
        &self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
        c: &Coded,
    ) -> u64 {
        let idx = self.idx(mx, my);
        let cur = coded_info(c, self.band.info[idx]);
        let bits = self.mb_cost(mx, my, a, c, &cur);
        self.rd(
            self.luma_ssd(mx, my, src) + self.chroma_ssd(mx, my, csrc),
            bits,
        )
    }

    /// Codes inter macroblock (`base`, `c`) unless intra (`intra`, its reconstruction in
    /// place) costs less. Returns the winner, with its reconstruction in place, and its cost.
    #[allow(clippy::too_many_arguments)]
    fn inter_or_intra(
        &mut self,
        mx: usize,
        my: usize,
        a: Avail,
        src: &[u8; 256],
        csrc: &[[u8; 64]; 2],
        p: &[u8; 256],
        pc: &[[u8; 64]; 2],
        base: MbInfo,
        c: Coded,
        intra: Option<Coded>,
    ) -> (Coded, u64) {
        let Some(ic) = intra else {
            return self.code_inter(mx, my, a, src, csrc, p, pc, base, c);
        };
        let ji = self.intra_rd(mx, my, a, src, csrc, &ic);
        let (rec, crec) = self.mb_recon(mx, my);
        let (c, j) = self.code_inter(mx, my, a, src, csrc, p, pc, base, c);
        if j <= ji {
            return (c, j);
        }
        self.put_pred(mx, my, &rec, &crec);
        (ic, ji)
    }

    /// Writes a prediction into the reconstruction.
    fn put_pred(&mut self, mx: usize, my: usize, p: &[u8; 256], pc: &[[u8; 64]; 2]) {
        let ls = self.ls();
        let cs = self.cs();
        let o = self.loff(mx * 16, my * 16);
        for y in 0..16 {
            self.band.y[o + y * ls..o + y * ls + 16].copy_from_slice(&p[y * 16..y * 16 + 16]);
        }
        let oc = self.coff(mx * 8, my * 8);
        for y in 0..8 {
            self.band.u[oc + y * cs..oc + y * cs + 8].copy_from_slice(&pc[0][y * 8..y * 8 + 8]);
            self.band.v[oc + y * cs..oc + y * cs + 8].copy_from_slice(&pc[1][y * 8..y * 8 + 8]);
        }
    }

    // ---- syntax ------------------------------------------------------------------------

    /// Fills in the coded macroblock's info: kind, coefficient counts, patterns.
    fn finish_info(&mut self, mx: usize, my: usize, c: &Coded) {
        let idx = self.idx(mx, my);
        self.band.info[idx] = coded_info(c, self.band.info[idx]);
    }

    fn write_skip(&mut self, mx: usize, my: usize, a: Avail) {
        let Some(ac) = self.ac.as_mut() else {
            self.skip_run += 1;
            return;
        };
        let not_skip = |n: &MbInfo| usize::from(n.kind != MbKind::Skip);
        let inc = if a.left {
            not_skip(&self.band.info[(my - self.band.mb_y0) * self.pic.mb_w + mx - 1])
        } else {
            0
        } + if a.top {
            not_skip(&self.band.info[(my - 1 - self.band.mb_y0) * self.pic.mb_w + mx])
        } else {
            0
        };
        ac.skip_flag(self.pic.kind == SliceKind::P, inc, true);
    }

    fn write_mb_cabac(&mut self, mx: usize, my: usize, a: Avail, c: &Coded) {
        let cur = *self.info_at(mx, my);
        let left = a.left.then(|| *self.info_at(mx - 1, my));
        let top = a.top.then(|| *self.info_at(mx, my - 1));
        let (kind, t8) = (self.pic.kind, self.pic.t8);
        mb_bins(self.ac.as_mut().unwrap(), kind, t8, &cur, left, top, c);
    }

    /// The cost (1/256 bits) of coding `c` with info `cur` here: CABAC from the current
    /// context states, or the CAVLC length (with an mb_skip_run of 0 in P slices).
    fn mb_cost(&self, mx: usize, my: usize, a: Avail, c: &Coded, cur: &MbInfo) -> u32 {
        let left = a.left.then(|| *self.info_at(mx - 1, my));
        let top = a.top.then(|| *self.info_at(mx, my - 1));
        let Some(ac) = self.ac.as_ref() else {
            let mut k = BitCount::default();
            let run = (self.pic.kind == SliceKind::P).then_some(0);
            mb_cavlc(&mut k, run, cur, left, top, c);
            return k.0 * 256;
        };
        let mut k = ac.cost();
        mb_bins(&mut k, self.pic.kind, self.pic.t8, cur, left, top, c);
        k.bits
    }

    fn write_mb_cavlc(&mut self, mx: usize, my: usize, a: Avail, c: &Coded) {
        let cur = *self.info_at(mx, my);
        let left = a.left.then(|| *self.info_at(mx - 1, my));
        let top = a.top.then(|| *self.info_at(mx, my - 1));
        let run = (self.pic.kind == SliceKind::P).then_some(self.skip_run);
        self.skip_run = 0;
        mb_cavlc(&mut self.w, run, &cur, left, top, c);
    }
}

/// The CAVLC syntax of coded macroblock `c` (info `cur`, neighbours `left`, `top`), after
/// `mb_skip_run` in P slices.
fn mb_cavlc<S: Sink>(
    w: &mut S,
    skip_run: Option<u32>,
    cur: &MbInfo,
    left: Option<MbInfo>,
    top: Option<MbInfo>,
    c: &Coded,
) {
    let (nz, nzc) = (cur.nz, cur.nzc);
    let p_slice = skip_run.is_some();
    if let Some(run) = skip_run {
        w.ue(run);
    }
    let intra_off = if p_slice { 5 } else { 0 };
    match c.kind {
        MbKind::I4 => w.ue(intra_off),
        MbKind::I16 => w.ue(intra_off
            + 1
            + u32::from(c.mode16)
            + 4 * u32::from(c.cbp_chroma)
            + if c.cbp_luma == 15 { 12 } else { 0 }),
        _ => w.ue(0),
    }
    match c.kind {
        MbKind::I4 => {
            for &(flag, rem) in &c.i4 {
                w.flag(flag);
                if !flag {
                    w.put(3, u32::from(rem));
                }
            }
            w.ue(u32::from(c.chroma_mode));
        }
        MbKind::I16 => w.ue(u32::from(c.chroma_mode)),
        _ => {
            w.se(c.mvd[0][0]);
            w.se(c.mvd[0][1]);
        }
    }
    let cbp = usize::from(c.cbp_luma | (c.cbp_chroma << 4));
    match c.kind {
        MbKind::I4 => w.ue(u32::from(CBP_INTRA[cbp])),
        MbKind::Inter => w.ue(u32::from(CBP_INTER[cbp])),
        _ => {}
    }
    if cbp == 0 && c.kind != MbKind::I16 {
        return;
    }
    w.se(0); // mb_qp_delta
    // Predicted TotalCoeff for luma block raster r.
    let nc_luma = |r: usize| -> i32 {
        let (bx, by) = (r % 4, r / 4);
        let na = if bx > 0 {
            Some(nz[r - 1])
        } else {
            left.map(|l| l.nz[by * 4 + 3])
        };
        let nb = if by > 0 {
            Some(nz[r - 4])
        } else {
            top.map(|t| t.nz[12 + bx])
        };
        nc_of(na, nb)
    };
    if c.kind == MbKind::I16 {
        write_block(w, &c.dc, nc_luma(0));
        if c.cbp_luma == 15 {
            for blk in 0..16 {
                let (bx, by) = BLK_XY[blk];
                let r = by * 4 + bx;
                write_block(w, &c.ac[r][1..], nc_luma(r));
            }
        }
    } else {
        for blk in 0..16 {
            if c.cbp_luma & (1 << (blk / 4)) == 0 {
                continue;
            }
            let (bx, by) = BLK_XY[blk];
            let r = by * 4 + bx;
            write_block(w, &c.ac[r], nc_luma(r));
        }
    }
    if c.cbp_chroma > 0 {
        for k in 0..2 {
            write_block(w, &c.cdc[k], -1);
        }
    }
    if c.cbp_chroma == 2 {
        for k in 0..2 {
            for b in 0..4 {
                let (bx, by) = (b % 2, b / 2);
                let na = if bx > 0 {
                    Some(nzc[k][b - 1])
                } else {
                    left.map(|l| l.nzc[k][by * 2 + 1])
                };
                let nb = if by > 0 {
                    Some(nzc[k][b - 2])
                } else {
                    top.map(|t| t.nzc[k][2 + bx])
                };
                write_block(w, &c.cac[k][b][1..], nc_of(na, nb));
            }
        }
    }
}

#[inline]
fn nc_of(a: Option<u8>, b: Option<u8>) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => (i32::from(a) + i32::from(b) + 1) >> 1,
        (Some(a), None) => i32::from(a),
        (None, Some(b)) => i32::from(b),
        (None, None) => 0,
    }
}

/// `base` (motion, intra modes) with the coded macroblock's kind, coefficient counts and
/// patterns.
fn coded_info(c: &Coded, base: MbInfo) -> MbInfo {
    let mut nz = [0u8; 16];
    if c.t8 {
        for r in 0..16 {
            nz[r] = count_nz(c.ac8(b8_of(r)));
        }
    } else {
        for r in 0..16 {
            let blk = RASTER_TO_BLK[r];
            let coded = match c.kind {
                MbKind::I16 => c.cbp_luma == 15,
                _ => c.cbp_luma & (1 << (blk / 4)) != 0,
            };
            if coded {
                nz[r] = if c.kind == MbKind::I16 {
                    count_nz(&c.ac[r][1..])
                } else {
                    count_nz(&c.ac[r])
                };
            }
        }
    }
    let mut nzc = [[0u8; 4]; 2];
    if c.cbp_chroma == 2 {
        for k in 0..2 {
            for b in 0..4 {
                nzc[k][b] = count_nz(&c.cac[k][b][1..]);
            }
        }
    }
    let mut dc_cbf = 0u8;
    if c.kind == MbKind::I16 && c.dc.iter().any(|&v| v != 0) {
        dc_cbf |= 1;
    }
    if c.cbp_chroma > 0 {
        for k in 0..2 {
            if c.cdc[k].iter().any(|&v| v != 0) {
                dc_cbf |= 2 << k;
            }
        }
    }
    let mut info = base;
    info.kind = c.kind;
    info.nz = nz;
    info.nzc = nzc;
    info.dc_cbf = dc_cbf;
    info.cbp = c.cbp_luma | c.cbp_chroma << 4;
    info.t8 = c.t8;
    if c.kind == MbKind::Inter {
        info.chroma_mode = 0;
        info.modes = [2; 16];
    } else {
        info.chroma_mode = c.chroma_mode;
        info.direct = false;
        info.pred = 0;
        info.mv = [[0; 2]; 2];
        info.mvd = [[0; 2]; 2];
        if c.kind == MbKind::I16 {
            info.modes = [2; 16];
        }
    }
    info
}

/// The bins of coded macroblock `c` (info `cur`, neighbours `left`, `top`).
fn mb_bins<B: Bins>(
    ac: &mut B,
    kind: SliceKind,
    t8_pps: bool,
    cur: &MbInfo,
    left: Option<MbInfo>,
    top: Option<MbInfo>,
    c: &Coded,
) {
    let intra = cur.is_intra();
    // condTermFlagA + 2·condTermFlagB (or + condTermFlagB) helpers.
    let cond = |n: Option<MbInfo>, f: &dyn Fn(&MbInfo) -> bool| n.as_ref().is_some_and(f);
    let inc2 =
        |f: &dyn Fn(&MbInfo) -> bool| usize::from(cond(left, f)) + 2 * usize::from(cond(top, f));
    let inc1 = |f: &dyn Fn(&MbInfo) -> bool| usize::from(cond(left, f)) + usize::from(cond(top, f));
    if kind != SliceKind::I {
        ac.skip_flag(
            kind == SliceKind::P,
            inc1(&|n| n.kind != MbKind::Skip),
            false,
        );
    }
    let i16 = (c.kind == MbKind::I16).then_some((c.mode16, c.cbp_luma, c.cbp_chroma));
    match kind {
        SliceKind::I => ac.mb_type_i(inc1(&|n| n.kind != MbKind::I4), i16),
        SliceKind::P => ac.mb_type_p(c.kind == MbKind::Inter, i16),
        SliceKind::B => {
            let inc = inc1(&|n| !(n.kind == MbKind::Skip || n.direct));
            let t = if intra { 4 } else { c.b_type };
            ac.mb_type_b(inc, t, i16);
        }
    }
    match c.kind {
        MbKind::I4 => {
            if t8_pps {
                ac.t8_flag(inc1(&|n| n.t8), c.t8);
            }
            for &(flag, rem) in &c.i4[..if c.t8 { 4 } else { 16 }] {
                ac.intra_mode(flag, rem);
            }
        }
        MbKind::Inter if !cur.direct => {
            for l in 0..2 {
                if cur.pred & (1 << l) == 0 {
                    continue;
                }
                for comp in 0..2 {
                    let sum = left.map_or(0, |n| u32::from(n.mvd[l][comp]))
                        + top.map_or(0, |n| u32::from(n.mvd[l][comp]));
                    ac.mvd(comp, sum, c.mvd[l][comp]);
                }
            }
        }
        _ => {}
    }
    if intra {
        ac.chroma_mode(inc1(&|n| n.is_intra() && n.chroma_mode != 0), c.chroma_mode);
    }
    if c.kind != MbKind::I16 {
        let mut bins = 0u8;
        for b8 in 0..4 {
            let (bx, by) = (b8 & 1, b8 >> 1);
            let ca = if bx == 1 {
                bins >> (b8 - 1) & 1 == 0
            } else {
                left.is_some_and(|n| n.cbp >> (b8 + 1) & 1 == 0)
            };
            let cb = if by == 1 {
                bins >> (b8 - 2) & 1 == 0
            } else {
                top.is_some_and(|n| n.cbp >> (b8 + 2) & 1 == 0)
            };
            let bit = c.cbp_luma >> b8 & 1;
            ac.cbp_luma_bin(usize::from(ca) + 2 * usize::from(cb), bit != 0);
            bins |= bit << b8;
        }
        ac.cbp_chroma(
            inc2(&|n| n.cbp >> 4 != 0),
            inc2(&|n| n.cbp >> 4 == 2),
            c.cbp_chroma,
        );
        if c.cbp_luma != 0 && t8_pps && c.kind == MbKind::Inter {
            ac.t8_flag(inc1(&|n| n.t8), c.t8);
        }
        if c.cbp_luma == 0 && c.cbp_chroma == 0 {
            return;
        }
    }
    ac.qp_delta_zero();
    // coded_block_flag increments: an unavailable neighbour counts as coded for intra
    // macroblocks only.
    let side = |n: Option<MbInfo>, f: &dyn Fn(&MbInfo) -> bool| match n {
        None => intra,
        Some(n) => f(&n),
    };
    let luma_inc = |r: usize| -> usize {
        let (bx, by) = (r % 4, r / 4);
        let ca = if bx > 0 {
            cur.cbf_luma(r - 1)
        } else {
            side(left, &|n| n.cbf_luma(by * 4 + 3))
        };
        let cb = if by > 0 {
            cur.cbf_luma(r - 4)
        } else {
            side(top, &|n| n.cbf_luma(12 + bx))
        };
        usize::from(ca) + 2 * usize::from(cb)
    };
    if c.kind == MbKind::I16 {
        let f = |n: &MbInfo| n.kind == MbKind::I16 && n.dc_cbf & 1 != 0;
        let inc = usize::from(side(left, &f)) + 2 * usize::from(side(top, &f));
        ac.residual(CAT_LUMA_DC, &c.dc, inc);
        if c.cbp_luma == 15 {
            for blk in 0..16 {
                let (bx, by) = BLK_XY[blk];
                let r = by * 4 + bx;
                ac.residual(CAT_LUMA_AC, &c.ac[r][1..], luma_inc(r));
            }
        }
    } else if c.t8 {
        for b8 in 0..4 {
            if c.cbp_luma >> b8 & 1 != 0 {
                ac.residual(CAT_LUMA_8X8, c.ac8(b8), 0);
            }
        }
    } else {
        for blk in 0..16 {
            if c.cbp_luma & (1 << (blk / 4)) == 0 {
                continue;
            }
            let (bx, by) = BLK_XY[blk];
            let r = by * 4 + bx;
            ac.residual(CAT_LUMA_4X4, &c.ac[r], luma_inc(r));
        }
    }
    if c.cbp_chroma > 0 {
        for k in 0..2 {
            let f = |n: &MbInfo| n.dc_cbf >> (1 + k) & 1 != 0;
            let inc = usize::from(side(left, &f)) + 2 * usize::from(side(top, &f));
            ac.residual(CAT_CHROMA_DC, &c.cdc[k], inc);
        }
    }
    if c.cbp_chroma == 2 {
        for k in 0..2 {
            for b in 0..4 {
                let (bx, by) = (b % 2, b / 2);
                let ca = if bx > 0 {
                    cur.nzc[k][b - 1] > 0
                } else {
                    side(left, &|n| n.nzc[k][by * 2 + 1] > 0)
                };
                let cb = if by > 0 {
                    cur.nzc[k][b - 2] > 0
                } else {
                    side(top, &|n| n.nzc[k][2 + bx] > 0)
                };
                let inc = usize::from(ca) + 2 * usize::from(cb);
                ac.residual(CAT_CHROMA_AC, &c.cac[k][b][1..], inc);
            }
        }
    }
}
