//! Specular's VP8 key-frame pipeline.
//!
//! The upstream decoder (`Vp8Decoder::decode_frame`) decodes every macroblock into a full-frame
//! buffer and then loop-filters the whole frame in a second pass. This module reuses its header
//! parsing but replaces the per-macroblock work with a three-stage, row-granular pipeline:
//!
//! 1. **parse** (calling thread): macroblock modes (first partition) and DCT tokens with a
//!    libwebp-style token reader (`get_coeffs`) over an inlined boolean decoder;
//! 2. **reconstruct** (thread 2): intra prediction + inverse transforms into a 16-row band,
//!    skipping all-zero blocks and using a DC-only shortcut;
//! 3. **filter** (thread 3): the in-loop deblocking filter, then the finished rows go to a
//!    [`Sink`] (range conversion, scaling, RGB...).
//!
//! VP8 intra prediction reads *unfiltered* pixels, so stage 2 keeps its own copy of the
//! unfiltered borders and stage 3 can filter immediately. Row buffers are recycled through
//! return channels; nothing is allocated per row. With `parallel == false` the same stages run
//! one after another on the calling thread.
//!
//! Output is bit-exact with the upstream decoder (differential tests in `tests/`).

use super::*;
use crate::loop_filter_simd::{self, Edges, Limits};
use std::sync::mpsc;

/// Receives finished rows of the (macroblock-padded) frame, top to bottom.
pub(crate) trait Sink: Send {
    /// `y` holds whole luma rows (`luma_stride` bytes each) starting at image row `y0` (even);
    /// `u`/`v` hold the matching chroma rows (half as many, `chroma_stride` bytes each). Rows
    /// may extend past the image height (macroblock padding).
    fn rows(&mut self, y0: usize, y: &[u8], u: &[u8], v: &[u8]);
}

#[cold]
#[inline(never)]
fn tail_byte(buf: &[u8], pos: usize) -> Option<u8> {
    buf.get(pos).copied()
}

// ---------------------------------------------------------------------------------------------
// Boolean decoder (RFC 6386 §7), libwebp layout: `range` holds range - 1 and `value` buffers up
// to 56 bits ahead.

struct BoolReader<'a> {
    buf: &'a [u8],
    pos: usize,
    value: u64,
    range: u32,
    bits: i32,
    /// Zero bytes appended past the end of the buffer.
    overrun: u32,
}

impl<'a> BoolReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        let mut br = BoolReader {
            buf,
            pos: 0,
            value: 0,
            range: 255 - 1,
            bits: -8,
            overrun: 0,
        };
        br.load();
        br
    }

    /// Continue where an [`ArithmeticDecoder`] over `buf` stopped (see
    /// [`ArithmeticDecoder::handover`]).
    fn resume(buf: &'a [u8], from: &ArithmeticDecoder) -> Option<Self> {
        let (pos, overrun, value, range, bits) = from.handover(buf.len())?;
        Some(BoolReader {
            buf,
            pos,
            value,
            range: range.checked_sub(1)?,
            bits,
            overrun,
        })
    }

    fn empty() -> Self {
        BoolReader {
            buf: &[],
            pos: 0,
            value: 0,
            range: 254,
            bits: -8,
            overrun: 0,
        }
    }

    #[inline(always)]
    fn load(&mut self) {
        if let Some(b) = self.buf.get(self.pos..self.pos + 8) {
            let v = u64::from_be_bytes(b.try_into().unwrap_or([0; 8])) >> 8;
            self.pos += 7;
            self.value = (self.value << 56) | v;
            self.bits += 56;
        } else {
            // The cold path only sees and returns plain values, so the reader's address never
            // escapes and its fields can stay in registers.
            match tail_byte(self.buf, self.pos) {
                Some(b) => {
                    self.pos += 1;
                    self.value = (self.value << 8) | u64::from(b);
                }
                None => {
                    self.value <<= 8;
                    self.overrun = self.overrun.saturating_add(1);
                }
            }
            self.bits += 8;
        }
    }

    /// Whether more than one zero byte past the end had to be read (the upstream decoder,
    /// like libwebp, tolerates exactly one).
    fn past_eof(&self) -> bool {
        self.overrun > 1
    }

    #[inline(always)]
    fn get_bit(&mut self, prob: u8) -> bool {
        if self.bits < 0 {
            self.load();
        }
        let pos = self.bits as u32;
        let split = (self.range * u32::from(prob)) >> 8;
        let value = (self.value >> pos) as u32;
        let bit = value > split;
        let range = if bit {
            self.value -= u64::from(split + 1) << pos;
            self.range - split
        } else {
            split + 1
        };
        let shift = 7 ^ (31 - range.leading_zeros());
        self.range = (range << shift) - 1;
        self.bits -= shift as i32;
        bit
    }

    /// Read a value with a tree whose leaves have the high bit set (`TreeNode` layout).
    #[inline(always)]
    fn get_tree(&mut self, nodes: &[TreeNode]) -> i8 {
        let mut i = 0;
        loop {
            let n = nodes[i];
            let t = if self.get_bit(n.prob) {
                n.right
            } else {
                n.left
            };
            if t & 0x80 != 0 {
                return TreeNode::value_from_branch(t);
            }
            i = usize::from(t);
        }
    }

    /// `v` with a sign read at probability 1/2. Signs are coin flips, so this is branch-free.
    #[inline(always)]
    fn get_signed(&mut self, v: i32) -> i32 {
        if self.bits < 0 {
            self.load();
        }
        let pos = self.bits as u32;
        let split = self.range >> 1;
        let value = (self.value >> pos) as u32;
        // All ones when the bit is 1.
        let mask = (split.wrapping_sub(value) as i32 >> 31) as u32;
        self.value -= u64::from((split + 1) & mask) << pos;
        // A 1 leaves `self.range - split`; the difference to `split + 1` is -1 when
        // `self.range` (range - 1) is even, hence the wrapping arithmetic.
        let range = (split + 1).wrapping_add(mask & self.range.wrapping_sub(2 * split + 1));
        let shift = 7 ^ (31 - range.leading_zeros());
        self.range = (range << shift) - 1;
        self.bits -= shift as i32;
        (v ^ mask as i32) - mask as i32
    }
}

/// Token probabilities indexed by coefficient position (band applied), context, node.
type PosProbs = [[[u8; NUM_DCT_TOKENS - 1]; 3]; 17];

const BANDS17: [usize; 17] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 0];

fn position_probs(nodes: &TokenProbTreeNodes) -> Box<[PosProbs; 4]> {
    let mut out = Box::new([[[[0u8; NUM_DCT_TOKENS - 1]; 3]; 17]; 4]);
    for (plane, o) in out.iter_mut().enumerate() {
        for (n, pos) in o.iter_mut().enumerate() {
            for (ctx, p) in pos.iter_mut().enumerate() {
                for (t, v) in p.iter_mut().enumerate() {
                    *v = nodes[plane][BANDS17[n]][ctx][t].prob;
                }
            }
        }
    }
    out
}

#[inline(always)]
fn get_large_value(br: &mut BoolReader<'_>, p: &[u8; NUM_DCT_TOKENS - 1]) -> i32 {
    if !br.get_bit(p[3]) {
        if !br.get_bit(p[4]) {
            2
        } else {
            3 + i32::from(br.get_bit(p[5]))
        }
    } else if !br.get_bit(p[6]) {
        if !br.get_bit(p[7]) {
            5 + i32::from(br.get_bit(159))
        } else {
            let v = 7 + 2 * i32::from(br.get_bit(165));
            v + i32::from(br.get_bit(145))
        }
    } else {
        const CAT3: &[u8] = &[173, 148, 140];
        const CAT4: &[u8] = &[176, 155, 140, 135];
        const CAT5: &[u8] = &[180, 157, 141, 134, 130];
        const CAT6: &[u8] = &[254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129];
        let bit1 = usize::from(br.get_bit(p[8]));
        let bit0 = usize::from(br.get_bit(p[9 + bit1]));
        let cat = 2 * bit1 + bit0;
        let tab = [CAT3, CAT4, CAT5, CAT6][cat];
        let mut v = 0;
        for &t in tab {
            v = 2 * v + i32::from(br.get_bit(t));
        }
        v + 3 + (8 << cat)
    }
}

/// Read one block's tokens starting at position `n`; returns the position after the last
/// non-zero coefficient (`n` itself when the block is empty), like libwebp's `GetCoeffs`.
#[inline(always)]
fn get_coeffs(
    br: &mut BoolReader<'_>,
    prob: &PosProbs,
    ctx: usize,
    dq: [i32; 2],
    mut n: usize,
    out: &mut [i32; 16],
) -> usize {
    let mut p = &prob[n][ctx];
    while n < 16 {
        if !br.get_bit(p[0]) {
            return n;
        }
        while !br.get_bit(p[1]) {
            n += 1;
            if n == 16 {
                return 16;
            }
            p = &prob[n][0];
        }
        let v = if !br.get_bit(p[2]) {
            p = &prob[n + 1][1];
            1
        } else {
            let v = get_large_value(br, p);
            p = &prob[n + 1][2];
            v
        };
        out[usize::from(ZIGZAG[n])] = br.get_signed(v) * dq[usize::from(n > 0)];
        n += 1;
    }
    16
}

// ---------------------------------------------------------------------------------------------
// Row data passed between the stages.

#[derive(Clone, Copy, Default)]
struct MbInfo {
    mb: MacroBlock,
    /// Bit `i` (0..24): block `i` has any non-zero coefficient (luma 0..16, U 16..20, V 20..24).
    nz: u32,
    /// Bit `i`: block `i` has non-zero AC coefficients (needs the full inverse DCT).
    ac: u32,
    /// Loop filter level, interior limit, HEV threshold.
    filter: (u8, u8, u8),
}

struct RowData {
    mby: usize,
    mbs: Vec<MbInfo>,
    /// 384 coefficients per macroblock; all zero between uses.
    coeffs: Vec<i32>,
}

struct Band {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

// ---------------------------------------------------------------------------------------------
// Stage 1: parsing.

struct Parser<'a, R> {
    d: Vp8Decoder<R>,
    probs: Box<[PosProbs; 4]>,
    /// [`Vp8Decoder::calculate_filter_parameters`] by segment and `luma_mode == B`.
    filter_params: [[(u8, u8, u8); 2]; 4],
    /// The first partition after the frame header: macroblock modes.
    modes: BoolReader<'a>,
    readers: Vec<BoolReader<'a>>,
}

impl<R: Read> Parser<'_, R> {
    fn parse_row(&mut self, mby: usize, row: &mut RowData) -> Result<(), DecodingError> {
        let mbw = usize::from(self.d.mbwidth);
        let p = mby % self.readers.len();
        row.mby = mby;
        self.d.left = MacroBlock::default();
        // A local copy keeps the decoder state in registers.
        let mut br = std::mem::replace(&mut self.readers[p], BoolReader::empty());
        let mut modes = std::mem::replace(&mut self.modes, BoolReader::empty());
        for mbx in 0..mbw {
            let mut mb = self.macroblock_header(&mut modes, mbx)?;
            let coeffs: &mut [i32; 384] = (&mut row.coeffs[mbx * 384..][..384])
                .try_into()
                .map_err(|_| DecodingError::BitStreamError)?;
            let (nz, ac) = if !mb.coeffs_skipped {
                self.residuals(&mut mb, mbx, &mut br, coeffs)
            } else {
                if mb.luma_mode != LumaMode::B {
                    self.d.left.complexity[0] = 0;
                    self.d.top[mbx].complexity[0] = 0;
                }
                for i in 1usize..9 {
                    self.d.left.complexity[i] = 0;
                    self.d.top[mbx].complexity[i] = 0;
                }
                (0, 0)
            };
            let filter = self.filter_params[usize::from(mb.segmentid & 3)]
                [usize::from(mb.luma_mode == LumaMode::B)];
            row.mbs[mbx] = MbInfo { mb, nz, ac, filter };
        }
        let eof = br.past_eof() || modes.past_eof();
        self.readers[p] = br;
        self.modes = modes;
        if eof {
            return Err(DecodingError::BitStreamError);
        }
        Ok(())
    }

    /// `Vp8Decoder::read_macroblock_header` (key frames) on the fast reader.
    #[inline(always)]
    fn macroblock_header(
        &mut self,
        br: &mut BoolReader<'_>,
        mbx: usize,
    ) -> Result<MacroBlock, DecodingError> {
        let d = &mut self.d;
        let mut mb = MacroBlock::default();
        if d.segments_enabled && d.segments_update_map {
            let p = &d.segment_tree_nodes;
            mb.segmentid = if !br.get_bit(p[0].prob) {
                u8::from(br.get_bit(p[1].prob))
            } else {
                2 + u8::from(br.get_bit(p[2].prob))
            };
        }
        mb.coeffs_skipped = d.prob_skip_false.is_some_and(|p| br.get_bit(p));
        // KEYFRAME_YMODE_TREE with KEYFRAME_YMODE_PROBS.
        mb.luma_mode = if !br.get_bit(145) {
            LumaMode::B
        } else if !br.get_bit(156) {
            if !br.get_bit(163) {
                LumaMode::DC
            } else {
                LumaMode::V
            }
        } else if !br.get_bit(128) {
            LumaMode::H
        } else {
            LumaMode::TM
        };
        let top = &mut d.top[mbx];
        match mb.luma_mode.into_intra() {
            None => {
                for y in 0usize..4 {
                    let mut left = d.left.bpred[y];
                    for x in 0usize..4 {
                        let t = top.bpred[12 + x];
                        let v = br.get_tree(&KEYFRAME_BPRED_MODE_NODES[t as usize][left as usize]);
                        // Lazily: building the error eagerly costs measurable time here.
                        #[allow(clippy::unnecessary_lazy_evaluations)]
                        let m = IntraMode::from_i8(v)
                            .ok_or_else(|| DecodingError::IntraPredictionModeInvalid(v))?;
                        mb.bpred[x + y * 4] = m;
                        top.bpred[12 + x] = m;
                        left = m;
                    }
                    d.left.bpred[y] = left;
                }
            }
            Some(mode) => {
                for i in 0usize..4 {
                    mb.bpred[12 + i] = mode;
                    d.left.bpred[i] = mode;
                }
            }
        }
        // KEYFRAME_UV_MODE_TREE with KEYFRAME_UV_MODE_PROBS.
        mb.chroma_mode = if !br.get_bit(142) {
            ChromaMode::DC
        } else if !br.get_bit(114) {
            ChromaMode::V
        } else if !br.get_bit(183) {
            ChromaMode::H
        } else {
            ChromaMode::TM
        };
        top.chroma_mode = mb.chroma_mode;
        top.luma_mode = mb.luma_mode;
        top.bpred = mb.bpred;
        Ok(mb)
    }

    #[inline(always)]
    fn residuals(
        &mut self,
        mb: &mut MacroBlock,
        mbx: usize,
        br: &mut BoolReader<'_>,
        blocks: &mut [i32; 384],
    ) -> (u32, u32) {
        let seg = self.d.segment[usize::from(mb.segmentid)];
        let top = &mut self.d.top[mbx].complexity;
        let left = &mut self.d.left.complexity;
        let (mut nz, mut ac) = (0u32, 0u32);

        let (plane, first) = if mb.luma_mode != LumaMode::B {
            let mut dc = [0i32; 16];
            let ctx = usize::from(top[0] + left[0]);
            let n = get_coeffs(
                br,
                &self.probs[1],
                ctx,
                [i32::from(seg.y2dc), i32::from(seg.y2ac)],
                0,
                &mut dc,
            );
            let has = u8::from(n > 0);
            top[0] = has;
            left[0] = has;
            if n > 1 {
                transform::iwht4x4(&mut dc);
            } else {
                // DC-only WHT: every output is (dc + 3) >> 3.
                let v = (dc[0] + 3) >> 3;
                dc = [v; 16];
            }
            for (k, &d) in dc.iter().enumerate() {
                blocks[16 * k] = d;
                if d != 0 {
                    nz |= 1 << k;
                }
            }
            (0usize, 1usize)
        } else {
            (3, 0)
        };

        let dq = [i32::from(seg.ydc), i32::from(seg.yac)];
        for y in 0..4 {
            let mut l = left[y + 1];
            for x in 0..4 {
                let i = x + y * 4;
                let block: &mut [i32; 16] = (&mut blocks[i * 16..][..16]).try_into().unwrap();
                let ctx = usize::from(top[x + 1] + l);
                let n = get_coeffs(br, &self.probs[plane], ctx, dq, first, block);
                let has = n > first;
                l = u8::from(has);
                top[x + 1] = l;
                if n > 1 {
                    ac |= 1 << i;
                    nz |= 1 << i;
                } else if block[0] != 0 {
                    nz |= 1 << i;
                }
            }
            left[y + 1] = l;
        }

        let dq = [i32::from(seg.uvdc), i32::from(seg.uvac)];
        for (j, base) in [(5usize, 16usize), (7, 20)] {
            for y in 0..2 {
                let mut l = left[y + j];
                for x in 0..2 {
                    let i = base + x + y * 2;
                    let block: &mut [i32; 16] = (&mut blocks[i * 16..][..16]).try_into().unwrap();
                    let ctx = usize::from(top[x + j] + l);
                    let n = get_coeffs(br, &self.probs[2], ctx, dq, 0, block);
                    l = u8::from(n > 0);
                    top[x + j] = l;
                    if n > 1 {
                        ac |= 1 << i;
                        nz |= 1 << i;
                    } else if block[0] != 0 {
                        nz |= 1 << i;
                    }
                }
                left[y + j] = l;
            }
        }
        mb.non_zero_dct = nz != 0;
        (nz, ac)
    }
}

// ---------------------------------------------------------------------------------------------
// Stage 2: reconstruction.

struct Recon {
    mbw: usize,
    top_y: Vec<u8>,
    left_y: [u8; 17],
    top_u: Vec<u8>,
    left_u: [u8; 9],
    top_v: Vec<u8>,
    left_v: [u8; 9],
}

/// Add the inverse transform of `block` to the 4×4 pixels at `pos`, then clear `block`.
#[inline(always)]
fn add_block(
    ws: &mut [u8],
    pos: usize,
    stride: usize,
    block: &mut [i32],
    i: usize,
    nz: u32,
    ac: u32,
) {
    if nz & (1 << i) == 0 {
        return;
    }
    let block: &mut [i32; 16] = (&mut block[i * 16..][..16]).try_into().unwrap();
    if ac & (1 << i) != 0 {
        transform::idct4x4_add(block, ws, pos, stride);
    } else {
        transform::dc_add(block[0], ws, pos, stride);
    }
    *block = [0; 16];
}

impl Recon {
    fn new(width: usize, mbw: usize) -> Recon {
        Recon {
            mbw,
            top_y: vec![127u8; width + 4 + 16],
            left_y: [129; 17],
            top_u: vec![127u8; 8 * mbw],
            left_u: [129; 9],
            top_v: vec![127u8; 8 * mbw],
            left_v: [129; 9],
        }
    }

    fn recon_row(&mut self, row: &mut RowData, band: &mut Band) {
        let mby = row.mby;
        let lw = self.mbw * 16;
        let cw = self.mbw * 8;
        for mbx in 0..self.mbw {
            let info = row.mbs[mbx];
            let mb = &info.mb;
            let coeffs = &mut row.coeffs[mbx * 384..][..384];

            // Luma.
            let stride = 1usize + 16 + 4;
            let mut ws = create_border_luma(mbx, mby, self.mbw, &self.top_y, &self.left_y);
            match mb.luma_mode {
                LumaMode::V => predict_vpred(&mut ws, 16, 1, 1, stride),
                LumaMode::H => predict_hpred(&mut ws, 16, 1, 1, stride),
                LumaMode::TM => predict_tmpred(&mut ws, 16, 1, 1, stride),
                LumaMode::DC => predict_dcpred(&mut ws, 16, stride, mby != 0, mbx != 0),
                LumaMode::B => {
                    for sby in 0usize..4 {
                        for sbx in 0usize..4 {
                            let i = sbx + sby * 4;
                            let y0 = sby * 4 + 1;
                            let x0 = sbx * 4 + 1;
                            match mb.bpred[i] {
                                IntraMode::TM => predict_tmpred(&mut ws, 4, x0, y0, stride),
                                IntraMode::VE => predict_bvepred(&mut ws, x0, y0, stride),
                                IntraMode::HE => predict_bhepred(&mut ws, x0, y0, stride),
                                IntraMode::DC => predict_bdcpred(&mut ws, x0, y0, stride),
                                IntraMode::LD => predict_bldpred(&mut ws, x0, y0, stride),
                                IntraMode::RD => predict_brdpred(&mut ws, x0, y0, stride),
                                IntraMode::VR => predict_bvrpred(&mut ws, x0, y0, stride),
                                IntraMode::VL => predict_bvlpred(&mut ws, x0, y0, stride),
                                IntraMode::HD => predict_bhdpred(&mut ws, x0, y0, stride),
                                IntraMode::HU => predict_bhupred(&mut ws, x0, y0, stride),
                            }
                            add_block(
                                &mut ws,
                                y0 * stride + x0,
                                stride,
                                coeffs,
                                i,
                                info.nz,
                                info.ac,
                            );
                        }
                    }
                }
            }
            if mb.luma_mode != LumaMode::B {
                for i in 0..16 {
                    let (x, y) = (i % 4, i / 4);
                    let pos = (1 + y * 4) * stride + 1 + x * 4;
                    add_block(&mut ws, pos, stride, coeffs, i, info.nz, info.ac);
                }
            }
            self.left_y[0] = ws[16];
            for (i, l) in self.left_y[1..].iter_mut().enumerate() {
                *l = ws[(i + 1) * stride + 16];
            }
            self.top_y[mbx * 16..][..16].copy_from_slice(&ws[16 * stride + 1..][..16]);
            for y in 0..16 {
                band.y[y * lw + mbx * 16..][..16]
                    .copy_from_slice(&ws[(1 + y) * stride + 1..][..16]);
            }

            // Chroma.
            let stride = 1usize + 8;
            let mut uws = create_border_chroma(mbx, mby, &self.top_u, &self.left_u);
            let mut vws = create_border_chroma(mbx, mby, &self.top_v, &self.left_v);
            match mb.chroma_mode {
                ChromaMode::DC => {
                    predict_dcpred(&mut uws, 8, stride, mby != 0, mbx != 0);
                    predict_dcpred(&mut vws, 8, stride, mby != 0, mbx != 0);
                }
                ChromaMode::V => {
                    predict_vpred(&mut uws, 8, 1, 1, stride);
                    predict_vpred(&mut vws, 8, 1, 1, stride);
                }
                ChromaMode::H => {
                    predict_hpred(&mut uws, 8, 1, 1, stride);
                    predict_hpred(&mut vws, 8, 1, 1, stride);
                }
                ChromaMode::TM => {
                    predict_tmpred(&mut uws, 8, 1, 1, stride);
                    predict_tmpred(&mut vws, 8, 1, 1, stride);
                }
            }
            for i in 0..4 {
                let (x, y) = (i % 2, i / 2);
                let pos = (1 + y * 4) * stride + 1 + x * 4;
                add_block(&mut uws, pos, stride, coeffs, 16 + i, info.nz, info.ac);
                add_block(&mut vws, pos, stride, coeffs, 20 + i, info.nz, info.ac);
            }
            set_chroma_border(&mut self.left_u, &mut self.top_u, &uws, mbx);
            set_chroma_border(&mut self.left_v, &mut self.top_v, &vws, mbx);
            for y in 0..8 {
                let ws_index = (1 + y) * stride + 1;
                band.u[y * cw + mbx * 8..][..8].copy_from_slice(&uws[ws_index..][..8]);
                band.v[y * cw + mbx * 8..][..8].copy_from_slice(&vws[ws_index..][..8]);
            }
        }
        self.left_y = [129; 17];
        self.left_u = [129; 9];
        self.left_v = [129; 9];
    }
}

// ---------------------------------------------------------------------------------------------
// Stage 3: loop filter and output.

/// Luma rows kept from the previous band (the top-edge filter modifies 3 rows above the edge
/// and reads a 4th; 8 keeps chroma, at half resolution, aligned).
const LAG: usize = 8;

struct Filter {
    mbw: usize,
    mbh: usize,
    simple: bool,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl Filter {
    fn new(mbw: usize, mbh: usize, simple: bool) -> Filter {
        Filter {
            mbw,
            mbh,
            simple,
            y: vec![0; (LAG + 16) * mbw * 16],
            u: vec![0; (LAG / 2 + 8) * mbw * 8],
            v: vec![0; (LAG / 2 + 8) * mbw * 8],
        }
    }

    fn filter_row<S: Sink + ?Sized>(&mut self, row: &RowData, band: &Band, sink: &mut S) {
        let lw = self.mbw * 16;
        let cw = self.mbw * 8;
        let mby = row.mby;
        self.y[LAG * lw..].copy_from_slice(&band.y);
        self.u[LAG / 2 * cw..].copy_from_slice(&band.u);
        self.v[LAG / 2 * cw..].copy_from_slice(&band.v);
        for mbx in 0..self.mbw {
            self.filter_mb(mbx, mby, &row.mbs[mbx]);
        }
        let last = mby + 1 == self.mbh;
        // Rows [mby*16 - LAG, mby*16 + 16 - LAG) are final now (all of them on the last row).
        let (skip, end) = (
            if mby == 0 { LAG } else { 0 },
            if last { LAG + 16 } else { 16 },
        );
        let y0 = mby * 16 + skip - LAG;
        sink.rows(
            y0,
            &self.y[skip * lw..end * lw],
            &self.u[skip / 2 * cw..end / 2 * cw],
            &self.v[skip / 2 * cw..end / 2 * cw],
        );
        if !last {
            self.y.copy_within(16 * lw.., 0);
            self.u.copy_within(8 * cw.., 0);
            self.v.copy_within(8 * cw.., 0);
        }
    }

    /// The upstream `Vp8Decoder::loop_filter` on the sliding window (luma row 0 of the
    /// current macroblock row is window row `LAG`), 16 pixels at a time.
    fn filter_mb(&mut self, mbx: usize, mby: usize, info: &MbInfo) {
        let (filter_level, interior_limit, hev_threshold) = info.filter;
        if filter_level == 0 {
            return;
        }
        let mb = &info.mb;
        let limits = Limits::new(filter_level, interior_limit, hev_threshold);
        let edges = Edges {
            left: mbx > 0,
            top: mby > 0,
            inner: mb.luma_mode == LumaMode::B || (!mb.coeffs_skipped && mb.non_zero_dct),
        };
        let luma_w = self.mbw * 16;
        let chroma_w = self.mbw * 8;
        loop_filter_simd::luma_mb(
            &mut self.y,
            LAG * luma_w + mbx * 16,
            luma_w,
            edges,
            &limits,
            self.simple,
        );
        if !self.simple {
            loop_filter_simd::chroma_mb(
                &mut self.u,
                &mut self.v,
                LAG / 2 * chroma_w + mbx * 8,
                chroma_w,
                edges,
                &limits,
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Driver.

/// Writes rows into the frame's own (macroblock-padded) planes.
pub(crate) struct FrameSink {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
    luma_stride: usize,
    chroma_stride: usize,
}

impl Sink for FrameSink {
    fn rows(&mut self, y0: usize, y: &[u8], u: &[u8], v: &[u8]) {
        let at = y0 * self.luma_stride;
        if let Some(d) = self.y.get_mut(at..at + y.len()) {
            d.copy_from_slice(y);
        }
        let at = y0 / 2 * self.chroma_stride;
        if let Some(d) = self.u.get_mut(at..at + u.len()) {
            d.copy_from_slice(u);
        }
        if let Some(d) = self.v.get_mut(at..at + v.len()) {
            d.copy_from_slice(v);
        }
    }
}

impl<R: Read> Vp8Decoder<R> {
    /// [`Vp8Decoder::decode_frame`] through the pipeline (same output).
    pub(crate) fn decode_frame_fast(r: R, parallel: bool) -> Result<Frame, DecodingError> {
        let (mut frame, sink) = Self::decode_pipelined(r, parallel, |f| {
            let mbw = usize::from(f.width.div_ceil(16));
            Ok(FrameSink {
                y: std::mem::take(&mut f.ybuf),
                u: std::mem::take(&mut f.ubuf),
                v: std::mem::take(&mut f.vbuf),
                luma_stride: mbw * 16,
                chroma_stride: mbw * 8,
            })
        })?;
        frame.ybuf = sink.y;
        frame.ubuf = sink.u;
        frame.vbuf = sink.v;
        Ok(frame)
    }

    /// Decode a key frame through the pipeline. `make_sink` is called once the header is
    /// parsed, with the frame header (its planes are allocated but unused: a sink may take
    /// them). Rows reach the sink in order; on error the sink may have seen a prefix.
    pub(crate) fn decode_pipelined<S: Sink>(
        r: R,
        parallel: bool,
        make_sink: impl FnOnce(&mut Frame) -> Result<S, DecodingError>,
    ) -> Result<(Frame, S), DecodingError> {
        let mut d = Self::new(r);
        d.read_frame_header()?;
        let mbw = usize::from(d.mbwidth);
        let mbh = usize::from(d.mbheight);
        let mut sink = make_sink(&mut d.frame)?;
        // Free the upstream full-frame buffers; the pipeline does not use them.
        d.frame.ybuf = Vec::new();
        d.frame.ubuf = Vec::new();
        d.frame.vbuf = Vec::new();
        let frame = d.frame.clone();
        if mbw == 0 || mbh == 0 {
            return Ok((frame, sink));
        }
        let raw = std::mem::take(&mut d.raw_partitions);
        let first = std::mem::take(&mut d.raw_first);
        let modes = BoolReader::resume(&first, &d.b).ok_or(DecodingError::BitStreamError)?;
        let probs = position_probs(&d.token_probs);
        let width = usize::from(d.frame.width);
        let simple = d.frame.filter_type;
        // The filter parameters depend only on the segment and on whether the mode is B.
        let filter_params = std::array::from_fn(|segment| {
            std::array::from_fn(|b| {
                d.calculate_filter_parameters(&MacroBlock {
                    segmentid: segment as u8,
                    luma_mode: if b == 1 { LumaMode::B } else { LumaMode::DC },
                    ..MacroBlock::default()
                })
            })
        });
        let mut parser = Parser {
            d,
            probs,
            filter_params,
            modes,
            readers: raw.iter().map(|b| BoolReader::new(b)).collect(),
        };
        let mut recon = Recon::new(width, mbw);
        let mut filter = Filter::new(mbw, mbh, simple);
        let new_row = || RowData {
            mby: 0,
            mbs: vec![MbInfo::default(); mbw],
            coeffs: vec![0; mbw * 384],
        };
        let new_band = || Band {
            y: vec![0; 16 * mbw * 16],
            u: vec![0; 8 * mbw * 8],
            v: vec![0; 8 * mbw * 8],
        };

        if !parallel || mbh < 3 {
            let mut row = new_row();
            let mut band = new_band();
            for mby in 0..mbh {
                parser.parse_row(mby, &mut row)?;
                recon.recon_row(&mut row, &mut band);
                filter.filter_row(&row, &band, &mut sink);
            }
            return Ok((frame, sink));
        }

        const DEPTH: usize = 4;
        let (to_recon, recon_rx) = mpsc::channel::<RowData>();
        let (to_filter, filter_rx) = mpsc::channel::<(RowData, Band)>();
        let (free_rows_tx, free_rows) = mpsc::channel::<RowData>();
        let (free_bands_tx, free_bands) = mpsc::channel::<Band>();
        for _ in 0..DEPTH {
            free_rows_tx.send(new_row()).ok();
            free_bands_tx.send(new_band()).ok();
        }
        let result = std::thread::scope(|s| {
            specular_thread::spawn_scoped(s, "dec-vp8-recon", move || {
                for mut row in recon_rx {
                    let Ok(mut band) = free_bands.recv() else {
                        return;
                    };
                    recon.recon_row(&mut row, &mut band);
                    if to_filter.send((row, band)).is_err() {
                        return;
                    }
                }
            });
            let sink = &mut sink;
            specular_thread::spawn_scoped(s, "dec-vp8-filter", move || {
                for (row, band) in filter_rx {
                    filter.filter_row(&row, &band, sink);
                    // The receivers may be gone after an error; that is fine.
                    let _ = free_rows_tx.send(row);
                    let _ = free_bands_tx.send(band);
                }
            });
            for mby in 0..mbh {
                let Ok(mut row) = free_rows.recv() else {
                    return Err(DecodingError::BitStreamError);
                };
                parser.parse_row(mby, &mut row)?;
                if to_recon.send(row).is_err() {
                    return Err(DecodingError::BitStreamError);
                }
            }
            drop(to_recon);
            Ok(())
        });
        result.map(|()| (frame, sink))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The payload of the first `VP8 ` chunk of a simple-format file.
    fn vp8_chunk(file: &[u8]) -> &[u8] {
        let mut at = 12;
        while at + 8 <= file.len() {
            let len = u32::from_le_bytes(file[at + 4..at + 8].try_into().unwrap()) as usize;
            let body = &file[at + 8..(at + 8 + len).min(file.len())];
            if &file[at..at + 4] == b"VP8 " {
                return body;
            }
            at += 8 + len + (len & 1);
        }
        panic!("no VP8 chunk");
    }

    fn files() -> Vec<std::path::PathBuf> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("lossy-")
            })
            .collect();
        v.sort();
        // Optional large file (benchmark corpus), when present.
        let big = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../target/laneb-bench/24mp-lossy-q90.webp"
        );
        if std::env::var_os("SPECULAR_WEBP_BIG").is_some() && std::path::Path::new(big).exists() {
            v.push(big.into());
        }
        v
    }

    #[test]
    fn pipeline_matches_reference_decoder() {
        for path in files() {
            let file = std::fs::read(&path).unwrap();
            let chunk = vp8_chunk(&file);
            let reference = Vp8Decoder::decode_frame(chunk).unwrap();
            for parallel in [false, true] {
                let fast = Vp8Decoder::decode_frame_fast(chunk, parallel).unwrap();
                assert_eq!(
                    (fast.width, fast.height),
                    (reference.width, reference.height)
                );
                assert!(
                    fast.ybuf == reference.ybuf,
                    "{path:?} luma, parallel={parallel}"
                );
                assert!(
                    fast.ubuf == reference.ubuf,
                    "{path:?} u, parallel={parallel}"
                );
                assert!(
                    fast.vbuf == reference.vbuf,
                    "{path:?} v, parallel={parallel}"
                );
            }
        }
    }

    #[test]
    fn truncated_and_corrupt_never_panic() {
        for path in files().into_iter().take(5) {
            let file = std::fs::read(&path).unwrap();
            let chunk = vp8_chunk(&file);
            for len in (0..chunk.len()).step_by(7) {
                for parallel in [false, true] {
                    let _ = Vp8Decoder::decode_frame_fast(&chunk[..len], parallel);
                }
            }
            let mut bad = chunk.to_vec();
            for i in (10..bad.len()).step_by(13) {
                bad[i] ^= 0x5a;
                let _ = Vp8Decoder::decode_frame_fast(&bad[..], true);
            }
        }
    }
}
