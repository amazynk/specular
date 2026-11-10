//! Inter prediction (clause 8.5): the reference pictures a picture is
//! predicted from, the per-slice reference picture lists (8.3.4), motion
//! vector derivation and motion compensation.

pub mod bs;
pub mod mc;
pub mod mvp;
pub mod pu;
pub mod refpic;

pub use refpic::RefPic;

use crate::picture::MvField;
use crate::slice::SliceHeader;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// The motion of one 16x16 block of a reference picture, kept for temporal
/// motion vector prediction (the compressed storage of 8.5.3.2.8, which
/// only ever reads the top-left 4x4 block of each 16x16 one).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColMv {
    /// `MvL0` and `MvL1`.
    pub mv: [[i16; 2]; 2],
    /// `PicOrderCntVal` of the picture each list referred to.
    pub poc: [i32; 2],
    /// Bit 0/1: list 0/1 used; bit 2/3: its reference was long-term when
    /// the picture was decoded. Zero for an intra block.
    pub flags: u8,
}

impl ColMv {
    /// `PredFlagLX`.
    #[inline]
    pub fn pred(&self, l: usize) -> bool {
        self.flags & (1 << l) != 0
    }

    /// `LongTermRefPic()` of the reference list `l` used.
    #[inline]
    pub fn lt(&self, l: usize) -> bool {
        self.flags & (4 << l) != 0
    }
}

/// One entry of the current picture's reference table.
#[derive(Debug, Clone)]
pub struct RefEntry {
    /// The picture.
    pub pic: Arc<RefPic>,
    /// Its `PicOrderCntVal`.
    pub poc: i32,
    /// Marked as used for long-term reference.
    pub lt: bool,
}

/// What an inter picture is predicted from: `RefPicSetStCurrBefore`, then
/// `RefPicSetStCurrAfter`, then `RefPicSetLtCurr`. Motion vectors name
/// reference pictures by their index here, so that blocks of different
/// slices compare.
#[derive(Debug, Clone, Default)]
pub struct InterCtx {
    /// `PicOrderCntVal` of the current picture.
    pub poc: i32,
    /// The table.
    pub refs: Vec<RefEntry>,
    /// `NumPocStCurrBefore`.
    pub n_before: usize,
    /// `NumPocStCurrAfter`.
    pub n_after: usize,
}

/// The reference picture lists of one slice, as indices into the
/// [`InterCtx`] table, with what motion vector prediction asks of them.
#[derive(Debug, Clone, Default)]
pub struct SliceRefs {
    /// `RefPicList0` and `RefPicList1`.
    pub id: [[u8; 16]; 2],
    /// `num_ref_idx_lX_active_minus1 + 1`.
    pub n: [usize; 2],
    /// `PicOrderCntVal` of each entry.
    pub poc: [[i32; 16]; 2],
    /// Long-term marking of each entry.
    pub lt: [[bool; 16]; 2],
    /// `NoBackwardPredFlag` (8.5.3.2.9).
    pub no_backward: bool,
    /// `ColPic`, when temporal motion vector prediction is on.
    pub col: Option<Arc<RefPic>>,
    /// `PicOrderCntVal` of the current picture.
    pub cur_poc: i32,
}

impl SliceRefs {
    /// Builds the lists of slice `h` (clause 8.3.4). Returns `None` when the
    /// slice names no reference picture the table has.
    pub fn new(ctx: &InterCtx, h: &SliceHeader) -> Option<SliceRefs> {
        let total = ctx.refs.len();
        if total == 0 {
            return None;
        }
        let mut s = SliceRefs {
            cur_poc: ctx.poc,
            ..SliceRefs::default()
        };
        let lists = if h.is_b() { 2 } else { 1 };
        let lt0 = ctx.n_before + ctx.n_after;
        for l in 0..lists {
            let n = (h.num_ref_idx[l] as usize).clamp(1, 16);
            let len = n.max(total);
            // RefPicListTemp0 is Before, After, Lt; RefPicListTemp1 is
            // After, Before, Lt; both repeated up to `len` entries.
            let mut order: Vec<u8> = Vec::with_capacity(total);
            if l == 0 {
                order.extend((0..ctx.n_before).map(|i| i as u8));
                order.extend((ctx.n_before..lt0).map(|i| i as u8));
            } else {
                order.extend((ctx.n_before..lt0).map(|i| i as u8));
                order.extend((0..ctx.n_before).map(|i| i as u8));
            }
            order.extend((lt0..total).map(|i| i as u8));
            for i in 0..n {
                let k = if h.list_modified[l] {
                    usize::from(h.list_entry[l][i])
                } else {
                    i
                };
                // Repetition: entry k of the temporary list.
                let id = order[(k % len) % order.len()];
                s.id[l][i] = id;
                let e = &ctx.refs[usize::from(id)];
                s.poc[l][i] = e.poc;
                s.lt[l][i] = e.lt;
            }
            s.n[l] = n;
        }
        s.no_backward = (0..lists).all(|l| s.poc[l][..s.n[l]].iter().all(|&p| p <= ctx.poc));
        if h.temporal_mvp {
            let l = usize::from(h.is_b() && !h.collocated_from_l0);
            let i = (h.collocated_ref_idx as usize).min(s.n[l].saturating_sub(1));
            s.col = Some(Arc::clone(&ctx.refs[usize::from(s.id[l][i])].pic));
        }
        Some(s)
    }
}

/// Compresses the motion of the row `y16` of 16x16 blocks of a picture
/// `w16` blocks wide to what temporal prediction reads.
pub fn compress_row(
    mvf: &[MvField],
    min4_w: usize,
    w16: usize,
    y16: usize,
    ctx: &InterCtx,
) -> Box<[ColMv]> {
    let mut out = Vec::with_capacity(w16);
    for x in 0..w16 {
        let m = mvf
            .get(y16 * 4 * min4_w + x * 4)
            .copied()
            .unwrap_or_default();
        let mut c = ColMv::default();
        for l in 0..2 {
            if m.pred(l) {
                let e = ctx.refs.get(usize::from(m.ref_id[l]));
                c.mv[l] = m.mv[l];
                c.poc[l] = e.map_or(0, |e| e.poc);
                c.flags |= 1 << l;
                if e.is_some_and(|e| e.lt) {
                    c.flags |= 4 << l;
                }
            }
        }
        out.push(c);
    }
    out.into_boxed_slice()
}

/// Scales a motion vector by the ratio of two POC distances (8-179 to 8-183).
#[inline]
pub fn scale_mv(mv: [i16; 2], td: i32, tb: i32) -> [i16; 2] {
    let td = td.clamp(-128, 127);
    let tb = tb.clamp(-128, 127);
    if td == 0 {
        return mv;
    }
    let tx = (16384 + (td.abs() >> 1)) / td;
    let dsf = ((tb * tx + 32) >> 6).clamp(-4096, 4095);
    let s = |v: i16| {
        let p = dsf * i32::from(v);
        let r = (p.abs() + 127) >> 8;
        (if p < 0 { -r } else { r }).clamp(-32768, 32767) as i16
    };
    [s(mv[0]), s(mv[1])]
}
