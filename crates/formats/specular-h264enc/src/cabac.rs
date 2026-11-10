//! CABAC: the arithmetic encoding engine (9.3.4.2) and the binarisations and context
//! selection of the syntax elements this encoder writes (9.3.2, 9.3.3.1).
//!
//! Context increments that depend on neighbouring macroblocks are worked out by the caller
//! (it owns the macroblock information) and passed in.

use crate::bits::BitWriter;
use crate::cabac_tables::{CABAC_INIT, LAST8X8, RANGE_LPS, SIG8X8, TRANS_IDX};
use std::simd::Simd;
use std::simd::cmp::SimdPartialEq;

pub const NUM_CTX: usize = 460;

/// ctxIdxBlockCatOffset of coded_block_flag, significant/last and coeff_abs_level_minus1
/// (Table 9-40) for ctxBlockCat 0..=4.
const CBF_OFF: [usize; 5] = [0, 4, 8, 12, 16];
const SIG_OFF: [usize; 5] = [0, 15, 29, 44, 47];
const ABS_OFF: [usize; 5] = [0, 10, 20, 30, 39];

/// Residual block categories (ctxBlockCat).
pub const CAT_LUMA_DC: usize = 0;
pub const CAT_LUMA_AC: usize = 1;
pub const CAT_LUMA_4X4: usize = 2;
pub const CAT_CHROMA_DC: usize = 3;
pub const CAT_CHROMA_AC: usize = 4;
pub const CAT_LUMA_8X8: usize = 5;

/// Next packed state (pStateIdx << 1 | valMPS) after an MPS / an LPS, and rangeTabLPS by
/// packed state.
struct Trans {
    mps: [u8; 128],
    lps: [u8; 128],
    range: [[u8; 4]; 128],
}

static TRANS: Trans = build_trans();

const fn build_trans() -> Trans {
    let mut t = Trans {
        mps: [0; 128],
        lps: [0; 128],
        range: [[0; 4]; 128],
    };
    let mut s = 0;
    while s < 128 {
        let p = s >> 1;
        let m = s & 1;
        t.mps[s] = ((TRANS_IDX[p].1 as usize) << 1 | m) as u8;
        let nm = if p == 0 { 1 - m } else { m };
        t.lps[s] = ((TRANS_IDX[p].0 as usize) << 1 | nm) as u8;
        t.range[s] = RANGE_LPS[p];
        s += 1;
    }
    t
}

/// The arithmetic encoder with its context variables, appending to a slice's RBSP.
pub struct CabacEnc {
    w: BitWriter,
    low: u32,
    range: u32,
    outstanding: u32,
    first: bool,
    ctx: [u8; NUM_CTX],
}

impl CabacEnc {
    /// Starts the engine after the slice header in `w` (aligned here with
    /// cabac_alignment_one_bit), with contexts initialised for `set` (0: I slices,
    /// 1 + cabac_init_idc otherwise) and SliceQPY `qp`.
    pub fn new(mut w: BitWriter, set: usize, qp: u8) -> CabacEnc {
        w.align_ones();
        let qp = i32::from(qp.min(51));
        let mut ctx = [0u8; NUM_CTX];
        for (c, &(m, n)) in ctx.iter_mut().zip(CABAC_INIT[set].iter()) {
            let pre = (((i32::from(m) * qp) >> 4) + i32::from(n)).clamp(1, 126);
            *c = if pre <= 63 {
                ((63 - pre) << 1) as u8
            } else {
                (((pre - 64) << 1) | 1) as u8
            };
        }
        CabacEnc {
            w,
            low: 0,
            range: 510,
            outstanding: 0,
            first: true,
            ctx,
        }
    }

    #[inline]
    fn put_bit(&mut self, b: u32) {
        if self.first {
            self.first = false;
        } else {
            self.w.put(1, b);
        }
        while self.outstanding > 0 {
            // Up to 32 outstanding bits at a time.
            let n = self.outstanding.min(32);
            let v = if b == 0 { u32::MAX } else { 0 };
            self.w.put(n, v);
            self.outstanding -= n;
        }
    }

    #[inline]
    fn renorm(&mut self) {
        while self.range < 256 {
            if self.low < 256 {
                self.put_bit(0);
            } else if self.low >= 512 {
                self.low -= 512;
                self.put_bit(1);
            } else {
                self.low -= 256;
                self.outstanding += 1;
            }
            self.range <<= 1;
            self.low <<= 1;
        }
    }

    /// The slice RBSP after the final `terminate(true)`.
    pub fn finish(mut self) -> Vec<u8> {
        self.w.align_zeros();
        self.w.into_bytes()
    }

    /// A bit counter starting from the current context states.
    pub fn cost(&self) -> CabacCost {
        CabacCost {
            ctx: self.ctx,
            bits: 0,
        }
    }

    /// The current context states (pStateIdx << 1 | valMPS).
    pub fn contexts(&self) -> &[u8; NUM_CTX] {
        &self.ctx
    }
}

impl Bins for CabacEnc {
    /// EncodeDecision.
    #[inline]
    fn bin(&mut self, ctx: usize, b: bool) {
        let s = usize::from(self.ctx[ctx]);
        let r_lps = u32::from(TRANS.range[s][((self.range >> 6) & 3) as usize]);
        self.range -= r_lps;
        if u32::from(b) != (s & 1) as u32 {
            self.low += self.range;
            self.range = r_lps;
            self.ctx[ctx] = TRANS.lps[s];
        } else {
            self.ctx[ctx] = TRANS.mps[s];
        }
        self.renorm();
    }

    /// EncodeBypass.
    #[inline]
    fn bypass(&mut self, b: bool) {
        self.low <<= 1;
        if b {
            self.low += self.range;
        }
        if self.low >= 1024 {
            self.put_bit(1);
            self.low -= 1024;
        } else if self.low < 512 {
            self.put_bit(0);
        } else {
            self.low -= 512;
            self.outstanding += 1;
        }
    }

    /// EncodeTerminate; a 1 also flushes (end of slice; the last bit written is the
    /// rbsp_stop_one_bit).
    #[inline]
    fn terminate(&mut self, b: bool) {
        self.range -= 2;
        if b {
            self.low += self.range;
            self.range = 2;
            self.renorm();
            self.put_bit((self.low >> 9) & 1);
            self.w.put(2, ((self.low >> 7) & 3) | 1);
        } else {
            self.renorm();
        }
    }
}

/// The first context of significant_coeff_flag, last_significant_coeff_flag and
/// coeff_abs_level_minus1 of residual category `cat` (frame coded).
#[inline]
pub fn residual_ctx(cat: usize) -> (usize, usize, usize) {
    if cat == CAT_LUMA_8X8 {
        (402, 417, 426)
    } else {
        (105 + SIG_OFF[cat], 166 + SIG_OFF[cat], 227 + ABS_OFF[cat])
    }
}

/// The coded_block_flag context of category `cat` (not 8×8 luma) with increment `inc`.
#[inline]
pub fn cbf_ctx(cat: usize, inc: usize) -> usize {
    85 + CBF_OFF[cat] + inc
}

/// ctxIdxInc of the significant and last flags at scan position `i` of category `cat`.
#[inline]
pub fn sig_last_inc(cat: usize, i: usize) -> (usize, usize) {
    match cat {
        CAT_CHROMA_DC => (i.min(2), i.min(2)),
        CAT_LUMA_8X8 => (usize::from(SIG8X8[i]), usize::from(LAST8X8[i])),
        _ => (i, i),
    }
}

/// The last position of `c` (at most 64 long) holding a non-zero level, if any.
#[inline]
pub fn last_nonzero(c: &[i16]) -> Option<usize> {
    let zero = Simd::<i16, 16>::splat(0);
    let mut mask = 0u64;
    let mut chunks = c.chunks_exact(16);
    for (k, v) in chunks.by_ref().enumerate() {
        mask |= Simd::from_slice(v).simd_ne(zero).to_bitmask() << (16 * k);
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let base = c.len() - rest.len();
        mask |= Simd::load_or_default(rest).simd_ne(zero).to_bitmask() << base;
    }
    (mask != 0).then(|| 63 - mask.leading_zeros() as usize)
}

/// A sink of CABAC bins: the arithmetic encoder, or a counter of their cost. The syntax
/// elements are written through it.
pub trait Bins {
    /// EncodeDecision of bin `b` with context `ctx`.
    fn bin(&mut self, ctx: usize, b: bool);
    /// EncodeBypass.
    fn bypass(&mut self, b: bool);
    /// EncodeTerminate.
    fn terminate(&mut self, b: bool);

    /// mb_skip_flag; `p_slice` picks the P or B context range.
    fn skip_flag(&mut self, p_slice: bool, inc: usize, skip: bool) {
        self.bin(if p_slice { 11 } else { 24 } + inc, skip);
    }

    /// The bins of an intra mb_type (Table 9-36) from bin 1 on (after the I_NxN/other bin),
    /// at contexts `ctx_b2..`: Intra16x16 with `pred` mode, coded block patterns.
    /// `off` is 3 for I slices (prefix contexts 3..10) or the suffix offset 17 (P) / 32 (B).
    fn i16_rest(&mut self, off: usize, pred: u8, cbp_luma: u8, cbp_chroma: u8) {
        self.terminate(false); // not I_PCM
        let i_slice = off == 3;
        let (c2, c3, c4a, c4b, c5a, c5b, c6) = if i_slice {
            (3, 4, 5, 6, 6, 7, 7)
        } else {
            (1, 2, 2, 3, 3, 3, 3)
        };
        self.bin(off + c2, cbp_luma != 0);
        self.bin(off + c3, cbp_chroma != 0);
        if cbp_chroma != 0 {
            self.bin(off + c4a, cbp_chroma == 2);
            self.bin(off + c5a, pred >> 1 != 0);
            self.bin(off + c6, pred & 1 != 0);
        } else {
            self.bin(off + c4b, pred >> 1 != 0);
            self.bin(off + c5b, pred & 1 != 0);
        }
    }

    /// mb_type of an I slice: I_NxN (`None`) or Intra16x16 (pred, cbp luma, cbp chroma).
    fn mb_type_i(&mut self, inc: usize, i16: Option<(u8, u8, u8)>) {
        self.bin(3 + inc, i16.is_some());
        if let Some((p, l, c)) = i16 {
            self.i16_rest(3, p, l, c);
        }
    }

    /// mb_type of a P slice: P_L0_16x16 (`inter`) or an intra type (prefix 1, suffix as I).
    fn mb_type_p(&mut self, inter: bool, i16: Option<(u8, u8, u8)>) {
        if inter {
            self.bin(14, false);
            self.bin(15, false);
            self.bin(16, false);
            return;
        }
        self.bin(14, true);
        self.bin(17, i16.is_some());
        if let Some((p, l, c)) = i16 {
            self.i16_rest(17, p, l, c);
        }
    }

    /// mb_type of a B slice. `kind`: 0 B_Direct_16x16, 1 B_L0_16x16, 2 B_L1_16x16,
    /// 3 B_Bi_16x16, 4 intra (with `i16` as for I slices).
    fn mb_type_b(&mut self, inc: usize, kind: u8, i16: Option<(u8, u8, u8)>) {
        match kind {
            0 => self.bin(27 + inc, false),
            1 | 2 => {
                self.bin(27 + inc, true);
                self.bin(30, false);
                self.bin(32, kind == 2);
            }
            3 => {
                // 1 1 0 0 0 0
                self.bin(27 + inc, true);
                self.bin(30, true);
                self.bin(31, false);
                for _ in 0..3 {
                    self.bin(32, false);
                }
            }
            _ => {
                // Intra prefix 1 1 1 1 0 1, then the I suffix at offset 32.
                self.bin(27 + inc, true);
                self.bin(30, true);
                self.bin(31, true);
                self.bin(32, true);
                self.bin(32, false);
                self.bin(32, true);
                self.bin(32, i16.is_some());
                if let Some((p, l, c)) = i16 {
                    self.i16_rest(32, p, l, c);
                }
            }
        }
    }

    /// transform_size_8x8_flag.
    fn t8_flag(&mut self, inc: usize, t8: bool) {
        self.bin(399 + inc, t8);
    }

    /// prev_intra4x4_pred_mode_flag / rem_intra4x4_pred_mode (also the 8×8 ones).
    fn intra_mode(&mut self, prev: bool, rem: u8) {
        self.bin(68, prev);
        if !prev {
            for k in 0..3 {
                self.bin(69, (rem >> k) & 1 != 0);
            }
        }
    }

    /// intra_chroma_pred_mode (TU, cMax 3).
    fn chroma_mode(&mut self, inc: usize, m: u8) {
        self.bin(64 + inc, m > 0);
        if m > 0 {
            self.bin(67, m > 1);
            if m > 1 {
                self.bin(67, m > 2);
            }
        }
    }

    /// coded_block_pattern: luma bins with their increments (worked out by the caller,
    /// which needs the earlier bins), then chroma.
    fn cbp_luma_bin(&mut self, inc: usize, b: bool) {
        self.bin(73 + inc, b);
    }

    /// The chroma part of coded_block_pattern; `inc0`/`inc1` are condA + 2·condB for the two
    /// bins (the second bin adds 4 here).
    fn cbp_chroma(&mut self, inc0: usize, inc1: usize, c: u8) {
        self.bin(77 + inc0, c != 0);
        if c != 0 {
            self.bin(77 + 4 + inc1, c == 2);
        }
    }

    /// mb_qp_delta of zero (the previous macroblock never codes a non-zero one).
    fn qp_delta_zero(&mut self) {
        self.bin(60, false);
    }

    /// mvd_lX component (`comp` 0 horizontal: ctxIdxOffset 40, 1 vertical: 47); `sum` is
    /// absMvdComp of the neighbours A + B.
    fn mvd(&mut self, comp: usize, sum: u32, v: i32) {
        let off = if comp == 0 { 40 } else { 47 };
        let inc0 = if sum < 3 {
            0
        } else if sum <= 32 {
            1
        } else {
            2
        };
        let a = v.unsigned_abs();
        let prefix = a.min(9);
        self.bin(off + inc0, prefix > 0);
        if prefix > 0 {
            const INC: [usize; 9] = [0, 3, 4, 5, 6, 6, 6, 6, 6];
            for k in 1..9u32 {
                let one = prefix > k;
                self.bin(off + INC[k as usize], one);
                if !one {
                    break;
                }
            }
            if a >= 9 {
                self.exp_golomb_bypass(a - 9, 3);
            }
            self.bypass(v < 0);
        }
    }

    /// k-th order Exp-Golomb suffix in bypass bins (9.3.2.3).
    fn exp_golomb_bypass(&mut self, mut v: u32, mut k: u32) {
        loop {
            if v >= (1 << k) {
                self.bypass(true);
                v -= 1 << k;
                k += 1;
            } else {
                self.bypass(false);
                while k > 0 {
                    k -= 1;
                    self.bypass((v >> k) & 1 != 0);
                }
                return;
            }
        }
    }

    /// A residual block in scan order (`coeffs.len()` = maxNumCoeff) of category `cat`.
    /// `cbf_inc` is the coded_block_flag increment (condA + 2·condB), unused for 8×8 blocks
    /// (whose flag is inferred in 4:2:0). Returns whether any level is non-zero.
    fn residual(&mut self, cat: usize, coeffs: &[i16], cbf_inc: usize) -> bool {
        let last = last_nonzero(coeffs);
        if cat != CAT_LUMA_8X8 {
            self.bin(cbf_ctx(cat, cbf_inc), last.is_some());
        }
        let Some(last) = last else {
            return false;
        };
        let n = coeffs.len();
        let (sig_base, last_base, abs_base) = residual_ctx(cat);
        for (i, &c) in coeffs.iter().enumerate().take(n - 1) {
            let (si, li) = sig_last_inc(cat, i);
            let sig = c != 0;
            self.bin(sig_base + si, sig);
            if sig {
                self.bin(last_base + li, i == last);
                if i == last {
                    break;
                }
            }
        }
        let gt1_cap = if cat == CAT_CHROMA_DC { 3 } else { 4 };
        let (mut eq1, mut gt1) = (0usize, 0usize);
        for &c in coeffs[..=last].iter().rev() {
            if c == 0 {
                continue;
            }
            let a = u32::from(c.unsigned_abs()) - 1;
            let inc0 = if gt1 != 0 { 0 } else { (1 + eq1).min(4) };
            self.bin(abs_base + inc0, a > 0);
            if a > 0 {
                let ctx = abs_base + 5 + gt1.min(gt1_cap);
                let prefix = a.min(14);
                for k in 1..14 {
                    let one = prefix > k;
                    self.bin(ctx, one);
                    if !one {
                        break;
                    }
                }
                if a >= 14 {
                    self.exp_golomb_bypass(a - 14, 0);
                }
                gt1 += 1;
            } else {
                eq1 += 1;
            }
            self.bypass(c < 0);
        }
        true
    }
}

/// Cost in 1/256 bits of coding a bin with packed state `s` (pStateIdx << 1 | valMPS) as
/// `b`: −log2 of its probability (pLPS = 0.5·αᵖ, 9.3.1.1). Written out (float maths isn't
/// `const`) so lookups need no lazy initialisation check; `bin_cost_table` rebuilds it.
pub static BIN_COST: [[u16; 2]; 128] = BIN_COST_T;

/// [`BIN_COST`] with both costs in one word (`b` = 1 in the upper half), for vector gathers.
pub static BIN_COST_PAIR: [u32; 128] = {
    let mut t = [0u32; 128];
    let mut s = 0;
    while s < 128 {
        t[s] = BIN_COST_T[s][0] as u32 | (BIN_COST_T[s][1] as u32) << 16;
        s += 1;
    }
    t
};

/// Per packed state and bin value: the bin's cost (low half) and the next state (high).
static BIN_STEP: [[u32; 2]; 128] = {
    let mut t = [[0u32; 2]; 128];
    let mut s = 0;
    while s < 128 {
        let mut b = 0;
        while b < 2 {
            let next = if b == s & 1 {
                TRANS.mps[s]
            } else {
                TRANS.lps[s]
            };
            t[s][b] = BIN_COST_T[s][b] as u32 | (next as u32) << 16;
            b += 1;
        }
        s += 1;
    }
    t
};

#[rustfmt::skip]
const BIN_COST_T: [[u16; 2]; 128] = [
    [256, 256], [256, 256], [238, 275], [275, 238], [221, 294], [294, 221], [206, 314], [314, 206],
    [192, 333], [333, 192], [180, 352], [352, 180], [168, 371], [371, 168], [157, 391], [391, 157],
    [148, 410], [410, 148], [139, 429], [429, 139], [130, 448], [448, 130], [122, 468], [468, 122],
    [115, 487], [487, 115], [108, 506], [506, 108], [102, 525], [525, 102], [96, 545], [545, 96],
    [90, 564], [564, 90], [85, 583], [583, 85], [80, 602], [602, 80], [76, 622], [622, 76],
    [72, 641], [641, 72], [68, 660], [660, 68], [64, 679], [679, 64], [60, 699], [699, 60],
    [57, 718], [718, 57], [54, 737], [737, 54], [51, 756], [756, 51], [48, 776], [776, 48],
    [46, 795], [795, 46], [43, 814], [814, 43], [41, 833], [833, 41], [39, 853], [853, 39],
    [37, 872], [872, 37], [35, 891], [891, 35], [33, 910], [910, 33], [31, 930], [930, 31],
    [29, 949], [949, 29], [28, 968], [968, 28], [26, 987], [987, 26], [25, 1007], [1007, 25],
    [24, 1026], [1026, 24], [22, 1045], [1045, 22], [21, 1064], [1064, 21], [20, 1084], [1084, 20],
    [19, 1103], [1103, 19], [18, 1122], [1122, 18], [17, 1141], [1141, 17], [16, 1161], [1161, 16],
    [15, 1180], [1180, 15], [15, 1199], [1199, 15], [14, 1218], [1218, 14], [13, 1238], [1238, 13],
    [12, 1257], [1257, 12], [12, 1276], [1276, 12], [11, 1295], [1295, 11], [11, 1315], [1315, 11],
    [10, 1334], [1334, 10], [10, 1353], [1353, 10], [9, 1372], [1372, 9], [9, 1392], [1392, 9],
    [8, 1411], [1411, 8], [8, 1430], [1430, 8], [7, 1449], [1449, 7], [7, 1469], [1469, 7],
];

/// Counts the cost of bins (1/256 bits) with adapting contexts, writing nothing.
#[derive(Clone)]
pub struct CabacCost {
    pub ctx: [u8; NUM_CTX],
    pub bits: u32,
}

impl Bins for CabacCost {
    #[inline]
    fn bin(&mut self, ctx: usize, b: bool) {
        let e = BIN_STEP[usize::from(self.ctx[ctx])][usize::from(b)];
        self.bits += u32::from(e as u16);
        self.ctx[ctx] = (e >> 16) as u8;
    }

    #[inline]
    fn bypass(&mut self, _b: bool) {
        self.bits += 256;
    }

    #[inline]
    fn terminate(&mut self, b: bool) {
        if b {
            self.bits += 7 * 256;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal decoding engine (9.3.3.2) to check the encoder round-trips.
    struct Dec<'a> {
        d: &'a [u8],
        bit: usize,
        range: u32,
        offset: u32,
        ctx: [u8; NUM_CTX],
    }

    impl Dec<'_> {
        fn read(&mut self) -> u32 {
            let b = self
                .d
                .get(self.bit / 8)
                .map_or(0, |&x| (x >> (7 - self.bit % 8)) & 1);
            self.bit += 1;
            u32::from(b)
        }
        fn new(d: &[u8], ctx: [u8; NUM_CTX]) -> Dec<'_> {
            let mut s = Dec {
                d,
                bit: 0,
                range: 510,
                offset: 0,
                ctx,
            };
            for _ in 0..9 {
                s.offset = s.offset << 1 | s.read();
            }
            s
        }
        fn bin(&mut self, i: usize) -> bool {
            let s = usize::from(self.ctx[i]);
            let r_lps = u32::from(TRANS.range[s][((self.range >> 6) & 3) as usize]);
            self.range -= r_lps;
            let v;
            if self.offset >= self.range {
                v = (s & 1) == 0;
                self.offset -= self.range;
                self.range = r_lps;
                self.ctx[i] = TRANS.lps[s];
            } else {
                v = (s & 1) == 1;
                self.ctx[i] = TRANS.mps[s];
            }
            while self.range < 256 {
                self.range <<= 1;
                self.offset = self.offset << 1 | self.read();
            }
            v
        }
        fn bypass(&mut self) -> bool {
            self.offset = self.offset << 1 | self.read();
            if self.offset >= self.range {
                self.offset -= self.range;
                true
            } else {
                false
            }
        }
        fn terminate(&mut self) -> bool {
            self.range -= 2;
            if self.offset >= self.range {
                true
            } else {
                while self.range < 256 {
                    self.range <<= 1;
                    self.offset = self.offset << 1 | self.read();
                }
                false
            }
        }
    }

    #[test]
    fn bin_cost_table() {
        let alpha = (0.01875f64 / 0.5).powf(1.0 / 63.0);
        for (s, e) in BIN_COST.iter().enumerate() {
            let lps = 0.5 * alpha.powi((s >> 1) as i32);
            let mps = s & 1;
            assert_eq!(e[mps], (-(1.0 - lps).log2() * 256.0).round() as u16, "{s}");
            assert_eq!(e[1 - mps], (-lps.log2() * 256.0).round() as u16, "{s}");
        }
    }

    #[test]
    fn engine_round_trip() {
        let mut e = CabacEnc::new(BitWriter::new(), 1, 26);
        let ctx0 = e.ctx;
        let mut s = 7u32;
        let mut seq = Vec::new();
        for i in 0..20_000 {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let kind = i % 7;
            let ctx = (s >> 8) as usize % 12;
            // Skewed bins so contexts adapt.
            let b = (s >> 20) % 10 < if ctx < 6 { 2 } else { 8 };
            seq.push((kind, ctx, b));
            match kind {
                0..=4 => e.bin(ctx, b),
                5 => e.bypass(b),
                _ => e.terminate(false),
            }
        }
        e.terminate(true);
        let data = e.finish();
        let mut d = Dec::new(&data, ctx0);
        for (i, &(kind, ctx, b)) in seq.iter().enumerate() {
            let got = match kind {
                0..=4 => d.bin(ctx),
                5 => d.bypass(),
                _ => d.terminate(),
            };
            assert_eq!(got, b && kind != 6, "bin {i}");
        }
        assert!(d.terminate());
    }
}
