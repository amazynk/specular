//! Progressive picture decoding: the macroblock and block layers of I, BI, P and B pictures
//! (§8.1–§8.4), overlap smoothing and the loop filter, for all three profiles.

use std::sync::LazyLock;

use crate::bits::Bits;
use crate::coefs::{self, Esc3, coding_set, dc_step, dequant, run_levels};
use crate::filter;
use crate::mc::{self, Edges, Frame};
use crate::picture::{DqProfile, MvMode, PicHeader, PicType};
use crate::seq::{Profile, Seq};
use crate::small::{
    MV_OFFSET, MV_SIZE, MVRANGE, SUBBLKPAT, TT_4X4, TT_4X8, TT_8X4, TT_8X8, TTBLK, TTMB,
    rate_index, tt_parts,
};
use crate::tables as t;
use crate::transform::inverse;
use crate::vlc::Vlc;
use crate::{Error, malformed};

static I_CBPCY: LazyLock<Vlc> = LazyLock::new(|| Vlc::new(&t::I_CBPCY));
static P_CBPCY: LazyLock<[Vlc; 4]> = LazyLock::new(|| {
    [
        Vlc::new(&t::P_CBPCY0),
        Vlc::new(&t::P_CBPCY1),
        Vlc::new(&t::P_CBPCY2),
        Vlc::new(&t::P_CBPCY3),
    ]
});
static MV_DIFF: LazyLock<[Vlc; 4]> = LazyLock::new(|| {
    [
        Vlc::new(&t::MV_DIFF0),
        Vlc::new(&t::MV_DIFF1),
        Vlc::new(&t::MV_DIFF2),
        Vlc::new(&t::MV_DIFF3),
    ]
});

/// What later blocks predict from an intra block: its quantized DC, first row and first
/// column of AC coefficients, and its quantizer.
#[derive(Clone, Copy, Default)]
pub struct Pred {
    pub intra: bool,
    pub dc: i32,
    pub row: [i32; 7],
    pub col: [i32; 7],
    pub mquant: u8,
    pub dcstep: i32,
    pub step: i32,
}

/// What the loop filter needs of a block.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct LfBlk {
    pub intra: bool,
    pub tt: u8,
    /// Coded sub-blocks: 8x8 bit 0; 8x4 / 4x8 bit 1 first, bit 0 second; 4x4 8·SB0 + 4·SB1 +
    /// 2·SB2 + SB3.
    pub sbp: u8,
    pub mv: [i32; 2],
    /// The block's own coefficients made it count as coded (Main profile exception 2).
    pub coded: bool,
    /// The macroblock signalled 4x4 at macroblock level (Main profile exception 3).
    pub mb_tt4x4: bool,
}

/// A motion vector for prediction: None when the block or macroblock is intra.
pub type Mv = Option<[i32; 2]>;

/// Per-picture working state, kept between pictures to reuse its memory.
#[derive(Default)]
pub struct State {
    pub mb_w: usize,
    pub mb_h: usize,
    /// Luma block grid (2·mb_w × 2·mb_h): motion vectors (None: intra).
    pub mv: Vec<Mv>,
    /// Coded status of luma blocks, for I-picture CBPCY prediction.
    pub coded: Vec<bool>,
    pub pred_y: Vec<Pred>,
    pub pred_c: Vec<[Pred; 2]>,
    pub lf: Vec<[LfBlk; 6]>,
    /// The row each macroblock's slice starts at.
    pub slice_top: Vec<u16>,
    /// Intra residuals awaiting overlap smoothing, per plane, and which blocks are intra.
    pub ovl: [Vec<i16>; 3],
    pub ovl_intra: Vec<[bool; 6]>,
    pub overflag: Vec<bool>,
    /// B pictures: forward and backward prediction contexts per macroblock.
    pub fwd: Vec<Mv>,
    pub bwd: Vec<Mv>,
    /// The motion vector each macroblock leaves for direct mode in later B pictures.
    pub direct: Vec<Mv>,
    pub mquant: Vec<u8>,
}

impl State {
    pub fn bytes(mb_w: usize, mb_h: usize) -> u64 {
        let n = (mb_w * mb_h) as u64;
        n * (4 * 16 + 4 + 4 * 72 + 2 * 72 + 6 * 24 + 2 + 384 * 2 + 6 + 1 + 3 * 16 + 1)
    }

    pub fn reset(&mut self, mb_w: usize, mb_h: usize) {
        let n = mb_w * mb_h;
        self.mb_w = mb_w;
        self.mb_h = mb_h;
        self.mv.clear();
        self.mv.resize(4 * n, Some([0, 0]));
        self.coded.clear();
        self.coded.resize(4 * n, false);
        self.pred_y.clear();
        self.pred_y.resize(4 * n, Pred::default());
        self.pred_c.clear();
        self.pred_c.resize(n, [Pred::default(); 2]);
        self.lf.clear();
        self.lf.resize(n, [LfBlk::default(); 6]);
        self.slice_top.clear();
        self.slice_top.resize(n, 0);
        self.ovl_intra.clear();
        self.ovl_intra.resize(n, [false; 6]);
        self.overflag.clear();
        self.overflag.resize(n, false);
        self.fwd.clear();
        self.fwd.resize(n, Some([0, 0]));
        self.bwd.clear();
        self.bwd.resize(n, Some([0, 0]));
        self.direct.clear();
        self.direct.resize(n, Some([0, 0]));
        self.mquant.clear();
        self.mquant.resize(n, 0);
    }
}

/// The references of a picture.
pub struct Refs<'a> {
    /// P: the reference; B: the previous anchor.
    pub fwd: Option<&'a Frame>,
    /// B: the next anchor.
    pub bwd: Option<&'a Frame>,
    /// B: the next anchor's direct-mode vectors (None when it was an I picture).
    pub direct: Option<&'a [Mv]>,
    pub edges: Edges,
}

const BLK_X: [usize; 4] = [0, 1, 0, 1];
const BLK_Y: [usize; 4] = [0, 0, 1, 1];

fn median3(a: i32, b: i32, c: i32) -> i32 {
    a.max(b).min(a.min(b).max(c))
}

fn median4(a: i32, b: i32, c: i32, d: i32) -> i32 {
    let mx = a.max(b).max(c).max(d);
    let mn = a.min(b).min(c).min(d);
    (a + b + c + d - mx - mn) / 2
}

fn smod(a: i32, b: i32) -> i32 {
    ((a + b) & (2 * b - 1)) - b
}

/// One picture's decoding.
pub struct Pic<'a> {
    pub seq: &'a Seq,
    pub h: &'a PicHeader,
    pub st: &'a mut State,
    pub cur: &'a mut Frame,
    pub refs: Refs<'a>,
    pub mb_w: usize,
    pub mb_h: usize,
    esc: Esc3,
    /// Overlap smoothing applies to this picture (some edges).
    overlap: bool,
    /// Simple/Main I picture without overlap: intra blocks are not offset by 128.
    no_offset: bool,
    k: (u32, u32),
    range: (i32, i32),
    halfpel: bool,
    bilinear: bool,
    rnd: i32,
    /// Advanced profile.
    adv: bool,
    /// Rows where slices start (Advanced), for availability.
    slice_row: usize,
}

struct MvData {
    dmv: [i32; 2],
    intra: bool,
    more: bool,
}

impl<'a> Pic<'a> {
    pub fn new(
        seq: &'a Seq,
        h: &'a PicHeader,
        st: &'a mut State,
        cur: &'a mut Frame,
        refs: Refs<'a>,
        rnd: bool,
    ) -> Pic<'a> {
        let mb_w = st.mb_w;
        let mb_h = st.mb_h;
        let adv = seq.profile == Profile::Advanced;
        let overlap = seq.overlap
            && match h.ptype {
                PicType::I | PicType::Bi => h.pquant >= 9 || (adv && h.condover != 0),
                PicType::P => h.pquant >= 9,
                _ => false,
            };
        let no_offset = !adv && h.is_intra() && !overlap;
        let (kx, ky, rx, ry) = MVRANGE[h.mvrange.min(3)];
        let halfpel = matches!(h.mvmode, MvMode::HalfPel | MvMode::HalfPelBilinear);
        let esc = Esc3 {
            conservative: h.pquant <= 7 || h.dq.in_frame,
            ..Esc3::default()
        };
        Pic {
            seq,
            h,
            st,
            cur,
            refs,
            mb_w,
            mb_h,
            esc,
            overlap,
            no_offset,
            k: (kx, ky),
            range: (rx, ry),
            halfpel,
            bilinear: h.mvmode == MvMode::HalfPelBilinear,
            rnd: i32::from(rnd),
            adv,
            slice_row: 0,
        }
    }

    /// Decodes the macroblock rows `rows` from `r`, a new slice when `slice` (predictors reset).
    pub fn decode_rows(
        &mut self,
        r: &mut Bits,
        rows: std::ops::Range<usize>,
        slice: bool,
    ) -> Result<(), Error> {
        if slice || rows.start == 0 {
            self.slice_row = rows.start;
            self.esc.seen = false;
        }
        for y in rows {
            for x in 0..self.mb_w {
                self.st.slice_top[y * self.mb_w + x] = self.slice_row as u16;
                match self.h.ptype {
                    PicType::I | PicType::Bi => self.mb_intra_pic(r, x, y)?,
                    PicType::P => self.mb_p(r, x, y)?,
                    PicType::B => self.mb_b(r, x, y)?,
                    PicType::Skipped => {}
                }
                if r.overrun() {
                    return Err(malformed("macroblock data runs past the picture"));
                }
            }
            if !self.adv
                && self.seq.syncmarker
                && y + 1 < self.mb_h
                && self.h.ptype != PicType::B
                && !r.flag()
            {
                r.align();
                let m = r.u(24);
                match m {
                    0xAA => {
                        for _ in 0..5 {
                            r.u(8);
                        }
                    }
                    0xAB => {
                        for _ in 0..11 {
                            r.u(8);
                        }
                    }
                    _ => return Err(malformed("bad sync marker")),
                }
            }
        }
        Ok(())
    }

    // ---- shared helpers -------------------------------------------------------------------

    /// The macroblock quantizer from the picture-level VOPDQUANT edges, or PQUANT.
    fn edge_mquant(&self, x: usize, y: usize) -> u8 {
        let dq = &self.h.dq;
        if !dq.in_frame {
            return self.h.pquant;
        }
        let (l, t, r, b) = (x == 0, y == 0, x + 1 == self.mb_w, y + 1 == self.mb_h);
        let alt = match dq.profile {
            DqProfile::AllEdges => l || t || r || b,
            DqProfile::DoubleEdge => match dq.edge {
                0 => l || t,
                1 => t || r,
                2 => r || b,
                _ => b || l,
            },
            DqProfile::SingleEdge => match dq.edge {
                0 => l,
                1 => t,
                2 => r,
                _ => b,
            },
            DqProfile::AllMbs => false,
        };
        if alt { dq.altpquant } else { self.h.pquant }
    }

    /// MQDIFF / ABSMQ when DQPROFILE is all macroblocks (Figure 29).
    fn read_mquant(&self, r: &mut Bits, x: usize, y: usize) -> u8 {
        let dq = &self.h.dq;
        if dq.in_frame && dq.profile == DqProfile::AllMbs {
            if dq.bilevel {
                if r.flag() {
                    dq.altpquant
                } else {
                    self.h.pquant
                }
            } else {
                let d = r.u(3) as u8;
                if d == 7 {
                    r.u(5) as u8
                } else {
                    self.h.pquant + d
                }
            }
        } else {
            self.edge_mquant(x, y)
        }
    }

    fn double_quant(&self, mquant: u8) -> i32 {
        2 * i32::from(mquant) + i32::from(self.h.halfqp && mquant == self.h.pquant)
    }

    // ---- intra blocks ---------------------------------------------------------------------

    /// The prediction of block `b` (0..6) of macroblock (x, y) from neighbour `dir`
    /// (0: top A, 1: top-left B, 2: left C), or None when outside the picture or slice.
    fn neighbour(&self, x: usize, y: usize, b: usize, dir: usize) -> Option<Pred> {
        let w = self.mb_w;
        let top = self.slice_row;
        if b < 4 {
            let bx = 2 * x + BLK_X[b];
            let by = 2 * y + BLK_Y[b];
            let (dx, dy) = [(0, 1), (1, 1), (1, 0)][dir];
            if bx < dx || by < dy {
                return None;
            }
            let (nx, ny) = (bx - dx, by - dy);
            if ny < 2 * top {
                return None;
            }
            Some(self.st.pred_y[ny * 2 * w + nx])
        } else {
            let (dx, dy) = [(0, 1), (1, 1), (1, 0)][dir];
            if x < dx || y < dy || y - dy < top {
                return None;
            }
            Some(self.st.pred_c[(y - dy) * w + x - dx][b - 4])
        }
    }

    /// Decodes an intra block into `out` (the residual, or the samples minus 128).
    #[allow(clippy::too_many_arguments)]
    fn intra_block(
        &mut self,
        r: &mut Bits,
        x: usize,
        y: usize,
        b: usize,
        coded: bool,
        acpred: bool,
        mquant: u8,
        out: &mut [i32; 64],
    ) -> Result<(), Error> {
        let chroma = b >= 4;
        let ipic = self.h.is_intra();
        let diff = coefs::dc_differential(r, self.h.transdctab, chroma, mquant)?;
        let dcstep = dc_step(mquant);
        let dq = self.double_quant(mquant);
        let step = dq - 1;
        // Neighbours, available when intra (all are in I pictures).
        let nb = |dir| self.neighbour(x, y, b, dir).filter(|p| p.intra);
        let (pa, pb, pc) = (nb(0), nb(1), nb(2));
        let scale = !(ipic && !self.adv);
        let sdc = |p: &Pred| -> i32 {
            if scale && p.mquant != mquant {
                ((i64::from(p.dc) * i64::from(p.dcstep) * i64::from(t::DQSCALE[dcstep as usize])
                    + 0x20000)
                    >> 18) as i32
            } else {
                p.dc
            }
        };
        let (dcpred, left) = if ipic && !self.adv {
            // Figure 39.
            let def = if self.seq.overlap && self.h.pquant >= 9 {
                0
            } else {
                (1024 + (dcstep >> 1)) / dcstep
            };
            let a = pa.map_or(def, |p| p.dc);
            let bb = pb.map_or(def, |p| p.dc);
            let c = pc.map_or(def, |p| p.dc);
            if (bb - a).abs() <= (bb - c).abs() {
                (c, true)
            } else {
                (a, false)
            }
        } else {
            // Figure 40 (B is zero when not intra).
            let a = pa.as_ref().map(sdc);
            let c = pc.as_ref().map(sdc);
            match (a, c) {
                (Some(a), Some(c)) => {
                    let bb = pb.as_ref().map_or(0, sdc);
                    if (bb - a).abs() <= (bb - c).abs() {
                        (c, true)
                    } else {
                        (a, false)
                    }
                }
                (Some(a), None) => (a, false),
                (None, Some(c)) => (c, true),
                (None, None) => (0, true),
            }
        };
        let dcq = dcpred + diff;
        let mut q1 = [0i32; 64];
        if coded {
            let set_index = if chroma || !ipic {
                self.h.transacfrm
            } else {
                self.h.transacfrm2
            };
            let set = coding_set(set_index, self.h.pqindex, !chroma);
            run_levels(r, set, &mut self.esc, &mut q1, 1, 64)?;
        }
        let scan: &[u8; 64] = if ipic || INTRA_SCANS_IN_P.load(std::sync::atomic::Ordering::Relaxed)
        {
            if !acpred {
                &t::SCAN_INTRA_NORMAL
            } else if left {
                &t::SCAN_INTRA_VERTICAL
            } else {
                &t::SCAN_INTRA_HORIZONTAL
            }
        } else {
            &t::SCAN_INTER_8X8
        };
        let mut q = [0i32; 64];
        for i in 1..64 {
            q[scan[i] as usize] = q1[i];
        }
        q[0] = dcq;
        if acpred {
            let p = if left { pc } else { pa };
            if let Some(p) = p {
                let sac = |v: i32| -> i32 {
                    if scale && p.mquant != mquant {
                        ((i64::from(v) * i64::from(p.step) * i64::from(t::DQSCALE[step as usize])
                            + 0x20000)
                            >> 18) as i32
                    } else {
                        v
                    }
                };
                if left {
                    for i in 0..7 {
                        q[(i + 1) * 8] += sac(p.col[i]);
                    }
                } else {
                    for i in 0..7 {
                        q[i + 1] += sac(p.row[i]);
                    }
                }
            }
        }
        let mut pr = Pred {
            intra: true,
            dc: dcq,
            row: [0; 7],
            col: [0; 7],
            mquant,
            dcstep,
            step,
        };
        for i in 0..7 {
            pr.row[i] = q[i + 1];
            pr.col[i] = q[(i + 1) * 8];
        }
        self.set_pred(x, y, b, pr);
        out[0] = q[0] * dcstep;
        for i in 1..64 {
            out[i] = dequant(q[i], dq, i32::from(mquant), self.h.uniform);
        }
        inverse(out, 8, 8);
        Ok(())
    }

    fn set_pred(&mut self, x: usize, y: usize, b: usize, p: Pred) {
        let w = self.mb_w;
        if b < 4 {
            self.st.pred_y[(2 * y + BLK_Y[b]) * 2 * w + 2 * x + BLK_X[b]] = p;
        } else {
            self.st.pred_c[y * w + x][b - 4] = p;
        }
    }

    /// Writes an intra block's samples: into the overlap buffer when smoothing applies, else
    /// offset by 128 (unless `no_offset`) and clamped into the picture.
    fn put_intra(&mut self, x: usize, y: usize, b: usize, res: &[i32; 64]) {
        let (pl, px, py) = block_origin(x, y, b);
        if self.overlap {
            let stride = self.cur.planes[pl].stride;
            let o = &mut self.st.ovl[pl];
            for r in 0..8 {
                for c in 0..8 {
                    o[(py + r) * stride + px + c] = res[r * 8 + c] as i16;
                }
            }
            self.st.ovl_intra[y * self.mb_w + x][b] = true;
            return;
        }
        let off = if self.no_offset { 0 } else { 128 };
        let p = &mut self.cur.planes[pl];
        for r in 0..8 {
            let row = &mut p.data[(py + r) * p.stride + px..];
            for c in 0..8 {
                row[c] = (res[r * 8 + c] + off).clamp(0, 255) as u8;
            }
        }
    }

    fn clear_pred(&mut self, x: usize, y: usize, b: usize) {
        self.set_pred(x, y, b, Pred::default());
    }

    // ---- I pictures ----------------------------------------------------------------------

    fn mb_intra_pic(&mut self, r: &mut Bits, x: usize, y: usize) -> Result<(), Error> {
        let i = y * self.mb_w + x;
        let raw = I_CBPCY.read(r).ok_or_else(|| malformed("bad CBPCY"))? as u8;
        let (acpred, mquant) = if self.adv {
            let ac = self.h.acpred.get(i, r);
            if self.h.condover == 3 {
                self.st.overflag[i] = self.h.overflags.get(i, r);
            }
            (ac, self.read_mquant(r, x, y))
        } else {
            (r.flag(), self.h.pquant)
        };
        self.st.mquant[i] = mquant;
        let cbp = self.predict_cbp(x, y, raw);
        let mut res = [0i32; 64];
        for b in 0..6 {
            let coded = cbp & (32 >> b) != 0;
            res.fill(0);
            self.intra_block(r, x, y, b, coded, acpred, mquant, &mut res)?;
            self.put_intra(x, y, b, &res);
        }
        self.set_mb_intra(x, y);
        Ok(())
    }

    /// Figure 34: the luma bits of CBPCY are predicted from the neighbours' coded status.
    fn predict_cbp(&mut self, x: usize, y: usize, raw: u8) -> u8 {
        let w2 = 2 * self.mb_w;
        let top = 2 * self.slice_row;
        let get = |bx: isize, by: isize| -> bool {
            if bx < 0 || by < top as isize {
                return false;
            }
            self.st.coded[by as usize * w2 + bx as usize]
        };
        let (bx, by) = (2 * x as isize, 2 * y as isize);
        let l1 = get(bx - 1, by);
        let l3 = get(bx - 1, by + 1);
        let lt3 = get(bx - 1, by - 1);
        let t2 = get(bx, by - 1);
        let t3 = get(bx + 1, by - 1);
        let mut y0 = if lt3 == t2 { l1 } else { t2 };
        y0 ^= raw >> 5 & 1 != 0;
        let mut y1 = if t2 == t3 { y0 } else { t3 };
        y1 ^= raw >> 4 & 1 != 0;
        let mut y2 = if l1 == y0 { l3 } else { y0 };
        y2 ^= raw >> 3 & 1 != 0;
        let mut y3 = if y0 == y1 { y2 } else { y1 };
        y3 ^= raw >> 2 & 1 != 0;
        let c = [y0, y1, y2, y3];
        for b in 0..4 {
            self.st.coded[(2 * y + BLK_Y[b]) * w2 + 2 * x + BLK_X[b]] = c[b];
        }
        (u8::from(y0) << 5)
            | (u8::from(y1) << 4)
            | (u8::from(y2) << 3)
            | (u8::from(y3) << 2)
            | (raw & 3)
    }

    fn set_mb_intra(&mut self, x: usize, y: usize) {
        let w = self.mb_w;
        for b in 0..4 {
            self.st.mv[(2 * y + BLK_Y[b]) * 2 * w + 2 * x + BLK_X[b]] = None;
        }
        let i = y * w + x;
        self.st.lf[i] = [LfBlk {
            intra: true,
            tt: TT_8X8,
            sbp: 1,
            mv: [0, 0],
            coded: true,
            mb_tt4x4: false,
        }; 6];
        self.st.direct[i] = None;
        self.st.fwd[i] = None;
        self.st.bwd[i] = None;
    }

    // ---- motion vectors ------------------------------------------------------------------

    /// MVDATA / BLKMVDATA / BMV (Figure 50).
    fn read_mvdata(&self, r: &mut Bits) -> Result<MvData, Error> {
        let idx = MV_DIFF[self.h.mvtab]
            .read(r)
            .ok_or_else(|| malformed("bad motion vector code"))? as usize;
        let mut index = idx + 1;
        let more = index >= 37;
        if more {
            index -= 37;
        }
        let hp = u32::from(self.halfpel);
        let mut d = MvData {
            dmv: [0, 0],
            intra: false,
            more,
        };
        match index {
            0 => {}
            35 => {
                d.dmv[0] = sign_extend(r.u(self.k.0 - hp), self.k.0 - hp);
                d.dmv[1] = sign_extend(r.u(self.k.1 - hp), self.k.1 - hp);
            }
            36 => d.intra = true,
            _ => {
                for (c, i1) in [index % 6, index / 6].into_iter().enumerate() {
                    let h = u32::from(self.halfpel && i1 == 5);
                    let val = r.u(MV_SIZE[i1] - h) as i32;
                    let sign = -(val & 1);
                    d.dmv[c] = (sign ^ ((val >> 1) + MV_OFFSET[i1])) - sign;
                }
            }
        }
        Ok(d)
    }

    /// The candidate predictors A, B, C of luma block (bx, by) per Figures 51–53, as
    /// Some(mv-or-intra) when in bounds.
    fn candidates(&self, x: usize, y: usize, blk: Option<usize>) -> [Option<Mv>; 3] {
        let w2 = 2 * self.mb_w as isize;
        let top = 2 * self.slice_row as isize;
        let get = |bx: isize, by: isize| -> Option<Mv> {
            if bx < 0 || bx >= w2 || by < top {
                return None;
            }
            Some(self.st.mv[(by * w2 + bx) as usize])
        };
        let (bx, by) = (2 * x as isize, 2 * y as isize);
        let last = x + 1 == self.mb_w;
        let first = x == 0;
        match blk {
            None => {
                let b = if last {
                    get(bx - 1, by - 1)
                } else {
                    get(bx + 2, by - 1)
                };
                [get(bx, by - 1), b, get(bx - 1, by)]
            }
            Some(0) => {
                let b = if first {
                    get(bx + 1, by - 1)
                } else {
                    get(bx - 1, by - 1)
                };
                [get(bx, by - 1), b, get(bx - 1, by)]
            }
            Some(1) => {
                let b = if last {
                    get(bx, by - 1)
                } else {
                    get(bx + 2, by - 1)
                };
                [get(bx + 1, by - 1), b, get(bx, by)]
            }
            Some(2) => [get(bx, by), get(bx + 1, by), get(bx - 1, by + 1)],
            _ => [get(bx + 1, by), get(bx, by), get(bx, by + 1)],
        }
    }

    /// The predicted motion vector (§8.3.5.3): median, pull-back, and in P pictures the
    /// hybrid prediction bit.
    fn predict(
        &self,
        r: &mut Bits,
        cands: [Option<Mv>; 3],
        x: usize,
        y: usize,
        blk: Option<usize>,
        hybrid: bool,
    ) -> [i32; 2] {
        let val = |c: Option<Mv>| c.flatten().unwrap_or([0, 0]);
        let [ca, cb, cc] = cands;
        let (a, b, c) = (val(ca), val(cb), val(cc));
        let mut p = if self.mb_w == 1 {
            a
        } else if ca.is_some() {
            if cb.is_none() && cc.is_none() {
                a
            } else {
                [median3(a[0], b[0], c[0]), median3(a[1], b[1], c[1])]
            }
        } else if cc.is_some() {
            c
        } else {
            [0, 0]
        };
        // Pull-back (§8.3.5.3.4).
        let xmax = 16 * self.mb_w as i32 * 4 - 4;
        let ymax = 16 * self.mb_h as i32 * 4 - 4;
        let (qx, qy, lim) = match blk {
            None => (x as i32 * 64, y as i32 * 64, -60),
            Some(k) => (
                (2 * x + BLK_X[k]) as i32 * 32,
                (2 * y + BLK_Y[k]) as i32 * 32,
                -28,
            ),
        };
        if qx + p[0] < lim {
            p[0] = lim - qx;
        } else if qx + p[0] > xmax {
            p[0] = xmax - qx;
        }
        if qy + p[1] < lim {
            p[1] = lim - qy;
        } else if qy + p[1] > ymax {
            p[1] = ymax - qy;
        }
        if hybrid && ca.is_some() && cc.is_some() && self.mb_w > 1 {
            let sum_a = (p[0] - a[0]).abs() + (p[1] - a[1]).abs();
            let sum = if sum_a > 32 {
                sum_a
            } else {
                (p[0] - c[0]).abs() + (p[1] - c[1]).abs()
            };
            if sum > 32 {
                return if r.flag() { a } else { c };
            }
        }
        p
    }

    fn reconstruct(&self, d: &MvData, p: [i32; 2]) -> [i32; 2] {
        let s = if self.halfpel { 2 } else { 1 };
        [
            smod(d.dmv[0] * s + p[0], self.range.0),
            smod(d.dmv[1] * s + p[1], self.range.1),
        ]
    }

    // ---- P pictures ----------------------------------------------------------------------

    fn mb_p(&mut self, r: &mut Bits, x: usize, y: usize) -> Result<(), Error> {
        let i = y * self.mb_w + x;
        let four = self.h.mvmode == MvMode::Mixed && self.h.mvtypemb.get(i, r);
        let skipped = self.h.skipmb.get(i, r);
        let w2 = 2 * self.mb_w;
        let mut mquant = self.edge_mquant(x, y);
        let mut lf = [LfBlk::default(); 6];
        if !four {
            let cands = self.candidates(x, y, None);
            if skipped {
                let p = self.predict(r, cands, x, y, None, true);
                for b in 0..4 {
                    self.st.mv[(2 * y + BLK_Y[b]) * w2 + 2 * x + BLK_X[b]] = Some(p);
                }
                self.st.mquant[i] = mquant;
                let cmv = self.mc_p_1mv(x, y, p);
                for (b, l) in lf.iter_mut().enumerate() {
                    l.mv = if b < 4 { p } else { cmv };
                }
                self.st.lf[i] = lf;
                self.st.direct[i] = Some(p);
                for b in 0..6 {
                    self.clear_pred(x, y, b);
                }
                return Ok(());
            }
            let d = self.read_mvdata(r)?;
            if d.intra {
                // An intra macroblock.
                let mut cbp = 0;
                let acpred;
                if !d.more {
                    mquant = self.read_mquant(r, x, y);
                    acpred = r.flag();
                } else {
                    acpred = r.flag();
                    cbp = P_CBPCY[self.h.cbptab]
                        .read(r)
                        .ok_or_else(|| malformed("bad CBPCY"))? as u8;
                    mquant = self.read_mquant(r, x, y);
                }
                self.st.mquant[i] = mquant;
                self.set_mb_intra(x, y);
                let mut res = [0i32; 64];
                for b in 0..6 {
                    res.fill(0);
                    self.intra_block(r, x, y, b, cbp & (32 >> b) != 0, acpred, mquant, &mut res)?;
                    self.put_intra(x, y, b, &res);
                }
                return Ok(());
            }
            let p = self.predict(r, cands, x, y, None, true);
            let mv = self.reconstruct(&d, p);
            for b in 0..4 {
                self.st.mv[(2 * y + BLK_Y[b]) * w2 + 2 * x + BLK_X[b]] = Some(mv);
            }
            self.st.direct[i] = Some(mv);
            let cmv = self.mc_p_1mv(x, y, mv);
            let mut cbp = 0;
            if d.more {
                cbp = P_CBPCY[self.h.cbptab]
                    .read(r)
                    .ok_or_else(|| malformed("bad CBPCY"))? as u8;
                mquant = self.read_mquant(r, x, y);
            }
            self.st.mquant[i] = mquant;
            for b in 0..6 {
                self.clear_pred(x, y, b);
                lf[b].mv = if b < 4 { mv } else { cmv };
            }
            let coded = [
                cbp & 32 != 0,
                cbp & 16 != 0,
                cbp & 8 != 0,
                cbp & 4 != 0,
                cbp & 2 != 0,
                cbp & 1 != 0,
            ];
            self.inter_residuals(r, x, y, mquant, coded, [false; 6], &mut lf)?;
            self.st.lf[i] = lf;
            return Ok(());
        }
        // 4-MV.
        let mut intra = [false; 6];
        let mut more = [false; 6];
        let mut cbp = 0u8;
        if !skipped {
            cbp = P_CBPCY[self.h.cbptab]
                .read(r)
                .ok_or_else(|| malformed("bad CBPCY"))? as u8;
        }
        let mut mvs = [[0i32; 2]; 4];
        for b in 0..4 {
            let bi = (2 * y + BLK_Y[b]) * w2 + 2 * x + BLK_X[b];
            let d = if !skipped && cbp & (32 >> b) != 0 {
                Some(self.read_mvdata(r)?)
            } else {
                None
            };
            if let Some(d) = d.as_ref().filter(|d| d.intra) {
                intra[b] = true;
                more[b] = d.more;
                self.st.mv[bi] = None;
                continue;
            }
            let cands = self.candidates(x, y, Some(b));
            let p = self.predict(r, cands, x, y, Some(b), true);
            let mv = match &d {
                Some(d) => {
                    more[b] = d.more;
                    self.reconstruct(d, p)
                }
                None => p,
            };
            mvs[b] = mv;
            self.st.mv[bi] = Some(mv);
        }
        let n_intra = intra[..4].iter().filter(|&&v| v).count();
        intra[4] = n_intra >= 3;
        intra[5] = intra[4];
        more[4] = cbp & 2 != 0;
        more[5] = cbp & 1 != 0;
        // The chroma vector and the direct-mode vector from the inter blocks.
        let inter: Vec<[i32; 2]> = (0..4).filter(|&b| !intra[b]).map(|b| mvs[b]).collect();
        let agg = match inter.len() {
            4 => Some([
                median4(inter[0][0], inter[1][0], inter[2][0], inter[3][0]),
                median4(inter[0][1], inter[1][1], inter[2][1], inter[3][1]),
            ]),
            3 => Some([
                median3(inter[0][0], inter[1][0], inter[2][0]),
                median3(inter[0][1], inter[1][1], inter[2][1]),
            ]),
            2 => Some([
                (inter[0][0] + inter[1][0]) / 2,
                (inter[0][1] + inter[1][1]) / 2,
            ]),
            _ => None,
        };
        self.st.direct[i] = agg;
        // Prediction for the inter blocks.
        for b in 0..4 {
            lf[b].mv = mvs[b];
            if !intra[b] {
                self.mc_luma_block(x, y, b, mvs[b]);
            }
        }
        let mut cmv = [0, 0];
        if let Some(m) = agg {
            cmv = self.chroma_mv(m);
            self.mc_chroma(x, y, cmv);
        }
        lf[4].mv = cmv;
        lf[5].mv = cmv;
        if skipped {
            self.st.mquant[i] = mquant;
            self.st.lf[i] = lf;
            for b in 0..6 {
                self.clear_pred(x, y, b);
            }
            return Ok(());
        }
        let any_intra = intra.iter().any(|&v| v);
        let any_coded = (0..6).any(|b| more[b] && !intra[b]) || (more[4] || more[5]);
        if !any_intra && !any_coded {
            self.st.mquant[i] = mquant;
            self.st.lf[i] = lf;
            for b in 0..6 {
                self.clear_pred(x, y, b);
            }
            return Ok(());
        }
        mquant = self.read_mquant(r, x, y);
        self.st.mquant[i] = mquant;
        // ACPRED when an intra block has an intra neighbour (Figure 58).
        let mut acpred = false;
        if any_intra {
            let mut nonzero = false;
            for b in 0..6 {
                if intra[b] {
                    // Blocks of this macroblock decoded before count once they are.
                    let a = self.neighbour_intra(x, y, b, 0, &intra);
                    let c = self.neighbour_intra(x, y, b, 2, &intra);
                    nonzero |= a || c;
                }
            }
            if nonzero {
                acpred = r.flag();
            }
        }
        let coded = [more[0], more[1], more[2], more[3], more[4], more[5]];
        for b in 0..6 {
            if !intra[b] {
                self.clear_pred(x, y, b);
            }
        }
        // Intra blocks and inter residuals in block order.
        self.inter_residuals_4mv(r, x, y, mquant, coded, intra, acpred, &mut lf)?;
        self.st.lf[i] = lf;
        Ok(())
    }

    /// Whether neighbour `dir` of block `b` is intra; blocks inside the current macroblock
    /// count by `intra`.
    fn neighbour_intra(&self, x: usize, y: usize, b: usize, dir: usize, intra: &[bool; 6]) -> bool {
        if b < 4 {
            let inside = match (b, dir) {
                (2, 0) => Some(0),
                (3, 0) => Some(1),
                (1, 2) => Some(0),
                (3, 2) => Some(2),
                _ => None,
            };
            if let Some(k) = inside {
                return intra[k];
            }
        }
        self.neighbour(x, y, b, dir).is_some_and(|p| p.intra)
    }

    /// The residual blocks of a 1-MV inter macroblock, added to the prediction in `cur`.
    #[allow(clippy::too_many_arguments)]
    fn inter_residuals(
        &mut self,
        r: &mut Bits,
        x: usize,
        y: usize,
        mquant: u8,
        coded: [bool; 6],
        _intra: [bool; 6],
        lf: &mut [LfBlk; 6],
    ) -> Result<(), Error> {
        let any = coded.iter().any(|&c| c);
        let mut tt = TtState::new(self, r, any)?;
        for b in 0..6 {
            if !coded[b] {
                lf[b].tt = tt.mb_tt.unwrap_or(TT_8X8);
                lf[b].mb_tt4x4 = tt.mb_level && tt.first_tt == TT_4X4;
                continue;
            }
            let mut res = [0i32; 64];
            let (btt, sbp) = self.inter_block(r, &mut tt, mquant, &mut res)?;
            lf[b].tt = btt;
            lf[b].sbp = sbp;
            lf[b].coded = sbp != 0;
            lf[b].mb_tt4x4 = tt.mb_level && tt.first_tt == TT_4X4;
            self.add_residual(x, y, b, &res);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn inter_residuals_4mv(
        &mut self,
        r: &mut Bits,
        x: usize,
        y: usize,
        mquant: u8,
        coded: [bool; 6],
        intra: [bool; 6],
        acpred: bool,
        lf: &mut [LfBlk; 6],
    ) -> Result<(), Error> {
        let any = (0..6).any(|b| coded[b] && !intra[b]);
        let mut tt = TtState::new(self, r, any)?;
        for b in 0..6 {
            let mut res = [0i32; 64];
            if intra[b] {
                self.intra_block(r, x, y, b, coded[b], acpred, mquant, &mut res)?;
                self.put_intra(x, y, b, &res);
                lf[b] = LfBlk {
                    intra: true,
                    tt: TT_8X8,
                    sbp: 1,
                    mv: lf[b].mv,
                    coded: true,
                    mb_tt4x4: false,
                };
                continue;
            }
            if !coded[b] {
                lf[b].tt = tt.mb_tt.unwrap_or(TT_8X8);
                lf[b].mb_tt4x4 = tt.mb_level && tt.first_tt == TT_4X4;
                continue;
            }
            let (btt, sbp) = self.inter_block(r, &mut tt, mquant, &mut res)?;
            lf[b].tt = btt;
            lf[b].sbp = sbp;
            lf[b].coded = sbp != 0;
            lf[b].mb_tt4x4 = tt.mb_level && tt.first_tt == TT_4X4;
            self.add_residual(x, y, b, &res);
        }
        Ok(())
    }

    /// Decodes an inter block's residual (§8.3.6.2); returns its transform type and coded
    /// sub-blocks.
    fn inter_block(
        &mut self,
        r: &mut Bits,
        tt: &mut TtState,
        mquant: u8,
        out: &mut [i32; 64],
    ) -> Result<(u8, u8), Error> {
        let ri = rate_index(self.h.pquant);
        // Pre-release WMV9: every split block after the first reads its sub-block pattern.
        let repeat_pat = !self.seq.rtm && !tt.first;
        let (btt, mut pat, from_tt) = if self.h.ttmbf {
            (self.h.ttfrm, 0, false)
        } else if tt.first {
            tt.first = false;
            (tt.first_tt, tt.first_pat, true)
        } else if let Some(m) = tt.mb_tt {
            (m, 0, false)
        } else {
            let s = TTBLK[ri].read(r).ok_or_else(|| malformed("bad TTBLK"))?;
            let (a, _, p) = tt_parts(s);
            (a, p, true)
        };
        let ri4 = ri;
        match btt {
            TT_4X4 => {
                pat = SUBBLKPAT[ri4]
                    .read(r)
                    .ok_or_else(|| malformed("bad SUBBLKPAT"))? as u8;
            }
            TT_8X4 | TT_4X8 if !from_tt || repeat_pat => {
                // Table 68.
                pat = if r.flag() {
                    if r.flag() { 2 } else { 1 }
                } else {
                    3
                };
            }
            TT_8X8 => pat = 1,
            _ => {}
        }
        let set = coding_set(self.h.transacfrm, self.h.pqindex, false);
        let dq = self.double_quant(mquant);
        let mq = i32::from(mquant);
        let uniform = self.h.uniform;
        let adv = self.adv;
        match btt {
            TT_8X8 => {
                let mut q1 = [0i32; 64];
                run_levels(r, set, &mut self.esc, &mut q1, 0, 64)?;
                for i in 0..64 {
                    out[t::SCAN_INTER_8X8[i] as usize] = dequant(q1[i], dq, mq, uniform);
                }
                inverse(out, 8, 8);
            }
            TT_8X4 => {
                let scan: &[u8; 32] = if adv {
                    &t::SCAN_INTER_8X4_ADV
                } else {
                    &t::SCAN_INTER_8X4_SM
                };
                for half in 0..2 {
                    if pat & (2 >> half) == 0 {
                        continue;
                    }
                    let mut q1 = [0i32; 64];
                    run_levels(r, set, &mut self.esc, &mut q1, 0, 32)?;
                    let mut blk = [0i32; 64];
                    for i in 0..32 {
                        let p = scan[i] as usize;
                        blk[(p / 8) * 8 + p % 8] = dequant(q1[i], dq, mq, uniform);
                    }
                    inverse(&mut blk, 8, 4);
                    out[half * 32..half * 32 + 32].copy_from_slice(&blk[..32]);
                }
            }
            TT_4X8 => {
                let scan: &[u8; 32] = if adv {
                    &t::SCAN_INTER_4X8_ADV
                } else {
                    &t::SCAN_INTER_4X8_SM
                };
                for half in 0..2 {
                    if pat & (2 >> half) == 0 {
                        continue;
                    }
                    let mut q1 = [0i32; 64];
                    run_levels(r, set, &mut self.esc, &mut q1, 0, 32)?;
                    let mut blk = [0i32; 64];
                    for i in 0..32 {
                        let p = scan[i] as usize;
                        blk[(p / 4) * 8 + p % 4] = dequant(q1[i], dq, mq, uniform);
                    }
                    inverse(&mut blk, 4, 8);
                    for row in 0..8 {
                        for c in 0..4 {
                            out[row * 8 + half * 4 + c] = blk[row * 8 + c];
                        }
                    }
                }
            }
            _ => {
                for sb in 0..4 {
                    if pat & (8 >> sb) == 0 {
                        continue;
                    }
                    let mut q1 = [0i32; 64];
                    run_levels(r, set, &mut self.esc, &mut q1, 0, 16)?;
                    let mut blk = [0i32; 64];
                    for i in 0..16 {
                        let p = t::SCAN_INTER_4X4[i] as usize;
                        blk[(p / 4) * 8 + p % 4] = dequant(q1[i], dq, mq, uniform);
                    }
                    inverse(&mut blk, 4, 4);
                    let (ox, oy) = ((sb % 2) * 4, (sb / 2) * 4);
                    for row in 0..4 {
                        for c in 0..4 {
                            out[(oy + row) * 8 + ox + c] = blk[row * 8 + c];
                        }
                    }
                }
            }
        }
        Ok((btt, pat))
    }

    fn add_residual(&mut self, x: usize, y: usize, b: usize, res: &[i32; 64]) {
        let (pl, px, py) = block_origin(x, y, b);
        let p = &mut self.cur.planes[pl];
        for r in 0..8 {
            let row = &mut p.data[(py + r) * p.stride + px..];
            for c in 0..8 {
                row[c] = (i32::from(row[c]) + res[r * 8 + c]).clamp(0, 255) as u8;
            }
        }
    }

    // ---- motion compensation -------------------------------------------------------------

    /// Simple/Main profile adjustment of a vector before motion compensation (Figures 61, 62,
    /// 73): `pos` the block's top-left, `lim` the macroblock (16) or chroma block (8) size,
    /// `n` the picture's size in those units.
    fn sm_adjust(&self, mv: [i32; 2], pos: (i32, i32), lim: i32) -> [i32; 2] {
        if self.adv {
            return mv;
        }
        let nx = self.mb_w as i32 * lim;
        let ny = self.mb_h as i32 * lim;
        let mut o = mv;
        let px = pos.0 + (mv[0] >> 2);
        let py = pos.1 + (mv[1] >> 2);
        if px < -lim {
            o[0] = ((-lim - pos.0) << 2) + (mv[0] & 3);
        } else if px > nx {
            o[0] = ((nx - pos.0) << 2) + (mv[0] & 3);
        }
        if py < -lim {
            o[1] = ((-lim - pos.1) << 2) + (mv[1] & 3);
        } else if py > ny {
            o[1] = ((ny - pos.1) << 2) + (mv[1] & 3);
        }
        o
    }

    /// The chroma vector of a luma vector (§8.3.5.4.3, §8.3.5.4.5).
    fn chroma_mv(&self, l: [i32; 2]) -> [i32; 2] {
        const RND: [i32; 4] = [0, 0, 0, 1];
        let mut c = [
            (l[0] + RND[(l[0] & 3) as usize]) >> 1,
            (l[1] + RND[(l[1] & 3) as usize]) >> 1,
        ];
        if self.seq.fastuvmc {
            for v in &mut c {
                // Quarter positions round towards zero.
                if *v % 2 != 0 {
                    *v += if *v > 0 { -1 } else { 1 };
                }
            }
        }
        c
    }

    fn mc_luma_from(
        &mut self,
        src: &Frame,
        x: usize,
        y: usize,
        b: Option<usize>,
        mv: [i32; 2],
        avg: bool,
    ) {
        let (px, py, size) = match b {
            None => (16 * x as i32, 16 * y as i32, 16),
            Some(k) => (
                16 * x as i32 + 8 * BLK_X[k] as i32,
                16 * y as i32 + 8 * BLK_Y[k] as i32,
                8,
            ),
        };
        let mut tmp = [0u8; 256];
        mc::luma(
            &src.planes[0],
            &self.refs.edges,
            px,
            py,
            mv[0],
            mv[1],
            size,
            self.bilinear,
            self.rnd,
            &mut tmp,
            size,
        );
        let p = &mut self.cur.planes[0];
        for r in 0..size {
            let row = &mut p.data[(py as usize + r) * p.stride + px as usize..][..size];
            if avg {
                mc::average(row, &tmp[r * size..r * size + size]);
            } else {
                row.copy_from_slice(&tmp[r * size..r * size + size]);
            }
        }
    }

    fn mc_chroma_from(&mut self, src: &Frame, x: usize, y: usize, cmv: [i32; 2], avg: bool) {
        for pl in 1..3 {
            let mut tmp = [0u8; 64];
            mc::chroma(
                &src.planes[pl],
                &self.refs.edges,
                pl,
                8 * x as i32,
                8 * y as i32,
                cmv[0],
                cmv[1],
                self.rnd,
                &mut tmp,
                8,
            );
            let p = &mut self.cur.planes[pl];
            for r in 0..8 {
                let row = &mut p.data[(8 * y + r) * p.stride + 8 * x..][..8];
                if avg {
                    mc::average(row, &tmp[r * 8..r * 8 + 8]);
                } else {
                    row.copy_from_slice(&tmp[r * 8..r * 8 + 8]);
                }
            }
        }
    }

    /// 1-MV P macroblock prediction; returns the chroma vector used.
    fn mc_p_1mv(&mut self, x: usize, y: usize, mv: [i32; 2]) -> [i32; 2] {
        let Some(src) = self.refs.fwd else {
            return [0, 0];
        };
        let lmv = self.sm_adjust(mv, (16 * x as i32, 16 * y as i32), 16);
        self.mc_luma_from(src, x, y, None, lmv, false);
        let c = self.chroma_mv(mv);
        let c = self.sm_adjust(c, (8 * x as i32, 8 * y as i32), 8);
        self.mc_chroma_from(src, x, y, c, false);
        c
    }

    fn mc_luma_block(&mut self, x: usize, y: usize, b: usize, mv: [i32; 2]) {
        let Some(src) = self.refs.fwd else {
            return;
        };
        let pos = (
            16 * x as i32 + 8 * BLK_X[b] as i32,
            16 * y as i32 + 8 * BLK_Y[b] as i32,
        );
        let lmv = self.sm_adjust(mv, pos, 16);
        self.mc_luma_from(src, x, y, Some(b), lmv, false);
    }

    fn mc_chroma(&mut self, x: usize, y: usize, cmv: [i32; 2]) {
        let Some(src) = self.refs.fwd else {
            return;
        };
        let c = self.sm_adjust(cmv, (8 * x as i32, 8 * y as i32), 8);
        self.mc_chroma_from(src, x, y, c, false);
    }

    // ---- B pictures ----------------------------------------------------------------------

    /// Figure 69: the direct-mode forward and backward vectors.
    fn direct_mvs(&self, x: usize, y: usize, m: [i32; 2]) -> ([i32; 2], [i32; 2]) {
        let sf = self.h.scale_factor;
        let (mut f, mut b);
        if self.halfpel {
            f = [2 * ((m[0] * sf + 255) >> 9), 2 * ((m[1] * sf + 255) >> 9)];
            b = [
                2 * ((m[0] * (sf - 256) + 255) >> 9),
                2 * ((m[1] * (sf - 256) + 255) >> 9),
            ];
        } else {
            f = [(m[0] * sf + 128) >> 8, (m[1] * sf + 128) >> 8];
            b = [
                (m[0] * (sf - 256) + 128) >> 8,
                (m[1] * (sf - 256) + 128) >> 8,
            ];
        }
        let qx = x as i32 * 64;
        let qy = y as i32 * 64;
        let maxx = self.mb_w as i32 * 64 - 4;
        let maxy = self.mb_h as i32 * 64 - 4;
        for v in [&mut f, &mut b] {
            if qx + v[0] < -60 {
                v[0] = -60 - qx;
            } else if qx + v[0] > maxx {
                v[0] = maxx - qx;
            }
            if qy + v[1] < -60 {
                v[1] = -60 - qy;
            } else if qy + v[1] > maxy {
                v[1] = maxy - qy;
            }
        }
        (f, b)
    }

    /// B-picture vector prediction from one context (§8.4.5.6–§8.4.5.8).
    fn predict_b(&self, ctx: &[Mv], x: usize, y: usize) -> [i32; 2] {
        let w = self.mb_w as isize;
        let top = self.slice_row as isize;
        let get = |mx: isize, my: isize| -> Option<Mv> {
            if mx < 0 || mx >= w || my < top {
                return None;
            }
            Some(ctx[(my * w + mx) as usize])
        };
        let (xi, yi) = (x as isize, y as isize);
        let last = x + 1 == self.mb_w;
        let ca = get(xi, yi - 1);
        let cb = if last {
            get(xi - 1, yi - 1)
        } else {
            get(xi + 1, yi - 1)
        };
        let cc = get(xi - 1, yi);
        let val = |c: Option<Mv>| c.flatten().unwrap_or([0, 0]);
        let (a, b, c) = (val(ca), val(cb), val(cc));
        let mut p = if self.mb_w == 1 {
            a
        } else if ca.is_some() {
            if cb.is_none() && cc.is_none() {
                a
            } else {
                [median3(a[0], b[0], c[0]), median3(a[1], b[1], c[1])]
            }
        } else if cc.is_some() {
            c
        } else {
            [0, 0]
        };
        if self.adv {
            let qx = x as i32 * 64;
            let qy = y as i32 * 64;
            let xm = 16 * self.mb_w as i32 * 4 - 4;
            let ym = 16 * self.mb_h as i32 * 4 - 4;
            if qx + p[0] < -60 {
                p[0] = -60 - qx;
            } else if qx + p[0] > xm {
                p[0] = xm - qx;
            }
            if qy + p[1] < -60 {
                p[1] = -60 - qy;
            } else if qy + p[1] > ym {
                p[1] = ym - qy;
            }
        } else {
            let qx = x as i32 * 32;
            let qy = y as i32 * 32;
            let xm = 8 * self.mb_w as i32 * 4 - 4;
            let ym = 8 * self.mb_h as i32 * 4 - 4;
            if qx + p[0] < -28 {
                p[0] = -28 - qx;
            } else if qx + p[0] > xm {
                p[0] = xm - qx;
            }
            if qy + p[1] < -28 {
                p[1] = -28 - qy;
            } else if qy + p[1] > ym {
                p[1] = ym - qy;
            }
        }
        p
    }

    /// B-picture prediction of a macroblock from the forward and/or backward vector.
    fn mc_b(&mut self, x: usize, y: usize, f: Option<[i32; 2]>, b: Option<[i32; 2]>) -> [i32; 2] {
        let mut first = true;
        let mut cmv_out = [0, 0];
        for (mv, src) in [(f, self.refs.fwd), (b, self.refs.bwd)] {
            let (Some(mv), Some(src)) = (mv, src) else {
                continue;
            };
            let lmv = self.sm_adjust(mv, (16 * x as i32, 16 * y as i32), 16);
            self.mc_luma_from(src, x, y, None, lmv, !first);
            let c = self.chroma_mv(lmv);
            self.mc_chroma_from(src, x, y, c, !first);
            cmv_out = c;
            first = false;
        }
        cmv_out
    }

    fn mb_b(&mut self, r: &mut Bits, x: usize, y: usize) -> Result<(), Error> {
        let i = y * self.mb_w + x;
        let direct = self.h.directmb.get(i, r);
        let skipped = self.h.skipmb.get(i, r);
        let mut mquant = self.edge_mquant(x, y);
        // The co-located vector of the next anchor.
        let col: Mv = match self.refs.direct {
            Some(d) => d[i],
            None => Some([0, 0]),
        };
        let (df, db) = match col {
            Some(m) => {
                let (f, b) = self.direct_mvs(x, y, m);
                (Some(f), Some(b))
            }
            None => (None, None),
        };
        let mut d1 = None;
        let mut btype = BType::Direct;
        if !direct {
            if !skipped {
                d1 = Some(self.read_mvdata(r)?);
            }
            if d1.as_ref().is_none_or(|d| !d.intra) {
                let back_first = self.h.scale_factor >= 128;
                btype = if !r.flag() {
                    if back_first {
                        BType::Backward
                    } else {
                        BType::Forward
                    }
                } else if !r.flag() {
                    if back_first {
                        BType::Forward
                    } else {
                        BType::Backward
                    }
                } else {
                    BType::Interpolated
                };
            } else {
                btype = BType::Intra;
            }
        }
        let mut lf = [LfBlk::default(); 6];
        if btype == BType::Intra {
            let d = d1.as_ref().map(|d| d.more).unwrap_or(false);
            let mut cbp = 0;
            let acpred;
            if !d {
                mquant = self.read_mquant(r, x, y);
                acpred = r.flag();
            } else {
                acpred = r.flag();
                cbp = P_CBPCY[self.h.cbptab]
                    .read(r)
                    .ok_or_else(|| malformed("bad CBPCY"))? as u8;
                mquant = self.read_mquant(r, x, y);
            }
            self.st.mquant[i] = mquant;
            self.set_mb_intra(x, y);
            let mut res = [0i32; 64];
            for b in 0..6 {
                res.fill(0);
                self.intra_block(r, x, y, b, cbp & (32 >> b) != 0, acpred, mquant, &mut res)?;
                self.put_intra(x, y, b, &res);
            }
            return Ok(());
        }
        // Vectors.
        let fwd_ctx = std::mem::take(&mut self.st.fwd);
        let bwd_ctx = std::mem::take(&mut self.st.bwd);
        let pf = self.predict_b(&fwd_ctx, x, y);
        let pb = self.predict_b(&bwd_ctx, x, y);
        self.st.fwd = fwd_ctx;
        self.st.bwd = bwd_ctx;
        let mut more = false;
        let (fmv, bmv): (Option<[i32; 2]>, Option<[i32; 2]>) = match btype {
            BType::Direct => (df.or(Some([0, 0])), db.or(Some([0, 0]))),
            BType::Forward => {
                let m = match &d1 {
                    Some(d) => {
                        more = d.more;
                        self.reconstruct(d, pf)
                    }
                    None => pf,
                };
                (Some(m), None)
            }
            BType::Backward => {
                let m = match &d1 {
                    Some(d) => {
                        more = d.more;
                        self.reconstruct(d, pb)
                    }
                    None => pb,
                };
                (None, Some(m))
            }
            _ => {
                // Interpolated: BMV1 backward, BMV2 forward.
                let bm = match &d1 {
                    Some(d) => {
                        more = d.more;
                        self.reconstruct(d, pb)
                    }
                    None => pb,
                };
                let mut fm = pf;
                if more {
                    let d2 = self.read_mvdata(r)?;
                    more = d2.more;
                    fm = self.reconstruct(&d2, pf);
                }
                (Some(fm), Some(bm))
            }
        };
        // Prediction contexts: the coded vectors, the direct ones elsewhere.
        let col_intra = col.is_none();
        self.st.fwd[i] = match btype {
            BType::Forward | BType::Interpolated => fmv,
            _ if col_intra => None,
            _ => df,
        };
        self.st.bwd[i] = match btype {
            BType::Backward | BType::Interpolated => bmv,
            _ if col_intra => None,
            _ => db,
        };
        let w2 = 2 * self.mb_w;
        for b in 0..4 {
            self.st.mv[(2 * y + BLK_Y[b]) * w2 + 2 * x + BLK_X[b]] = Some([0, 0]);
        }
        let cmv = self.mc_b(x, y, fmv, bmv);
        for b in 0..6 {
            self.clear_pred(x, y, b);
            lf[b].mv = if b < 4 {
                fmv.or(bmv).unwrap_or([0, 0])
            } else {
                cmv
            };
        }
        if skipped || (btype != BType::Direct && !more) {
            self.st.mquant[i] = mquant;
            self.st.lf[i] = lf;
            return Ok(());
        }
        let cbp = P_CBPCY[self.h.cbptab]
            .read(r)
            .ok_or_else(|| malformed("bad CBPCY"))? as u8;
        mquant = self.read_mquant(r, x, y);
        self.st.mquant[i] = mquant;
        let coded = [
            cbp & 32 != 0,
            cbp & 16 != 0,
            cbp & 8 != 0,
            cbp & 4 != 0,
            cbp & 2 != 0,
            cbp & 1 != 0,
        ];
        self.inter_residuals(r, x, y, mquant, coded, [false; 6], &mut lf)?;
        self.st.lf[i] = lf;
        Ok(())
    }

    // ---- post-processing -----------------------------------------------------------------

    /// Overlap smoothing over the stored intra residuals, then their samples into the picture.
    pub fn finish_overlap(&mut self) {
        if !self.overlap {
            return;
        }
        let w = self.mb_w;
        let h = self.mb_h;
        let all = self.h.is_intra() && (self.h.pquant >= 9 || self.h.condover == 2);
        let cond = self.h.is_intra() && self.h.pquant < 9 && self.h.condover == 3;
        for pl in 0..3 {
            let (bw, bh) = if pl == 0 { (2 * w, 2 * h) } else { (w, h) };
            let stride = self.cur.planes[pl].stride;
            let intra_at = |st: &State, bx: usize, by: usize| -> bool {
                if pl == 0 {
                    st.ovl_intra[(by / 2) * w + bx / 2][(by % 2) * 2 + bx % 2]
                } else {
                    st.ovl_intra[by * w + bx][3 + pl]
                }
            };
            let mb_of = |bx: usize, by: usize| if pl == 0 { (bx / 2, by / 2) } else { (bx, by) };
            let edge_ok = |st: &State, b0: (usize, usize), b1: (usize, usize)| -> bool {
                if !(intra_at(st, b0.0, b0.1) && intra_at(st, b1.0, b1.1)) {
                    return false;
                }
                let m0 = mb_of(b0.0, b0.1);
                let m1 = mb_of(b1.0, b1.1);
                if st.slice_top[m0.1 * w + m0.0] != st.slice_top[m1.1 * w + m1.0] {
                    return false;
                }
                if all || !cond {
                    return true;
                }
                let f0 = st.overflag[m0.1 * w + m0.0];
                let f1 = st.overflag[m1.1 * w + m1.0];
                if m0 == m1 { f0 } else { f0 && f1 }
            };
            let mut o = std::mem::take(&mut self.st.ovl[pl]);
            for by in 0..bh {
                for bx in 1..bw {
                    if edge_ok(self.st, (bx - 1, by), (bx, by)) {
                        filter::overlap_v(&mut o, stride, 8 * bx, 8 * by);
                    }
                }
            }
            for by in 1..bh {
                for bx in 0..bw {
                    if edge_ok(self.st, (bx, by - 1), (bx, by)) {
                        filter::overlap_h(&mut o, stride, 8 * bx, 8 * by);
                    }
                }
            }
            // Intra blocks into the picture.
            let p = &mut self.cur.planes[pl];
            for by in 0..bh {
                for bx in 0..bw {
                    if !intra_at(self.st, bx, by) {
                        continue;
                    }
                    for rr in 0..8 {
                        let base = (8 * by + rr) * stride + 8 * bx;
                        for c in 0..8 {
                            p.data[base + c] = (i32::from(o[base + c]) + 128).clamp(0, 255) as u8;
                        }
                    }
                }
            }
            self.st.ovl[pl] = o;
        }
    }

    /// The in-loop deblocking filter (§8.6).
    pub fn loop_filter(&mut self) {
        if !self.seq.loopfilter {
            return;
        }
        let pq = i32::from(self.h.pquant);
        let w = self.mb_w;
        let h = self.mb_h;
        let p_pic = self.h.ptype == PicType::P;
        for pl in 0..3 {
            let (bw, bh) = if pl == 0 { (2 * w, 2 * h) } else { (w, h) };
            let blk = |st: &State, bx: usize, by: usize| -> LfBlk {
                if pl == 0 {
                    st.lf[(by / 2) * w + bx / 2][(by % 2) * 2 + bx % 2]
                } else {
                    st.lf[by * w + bx][3 + pl]
                }
            };
            let slice_of = |st: &State, by: usize| -> u16 {
                let my = if pl == 0 { by / 2 } else { by };
                st.slice_top[my * w]
            };
            let plane = &mut self.cur.planes[pl];
            // Horizontal block edges.
            for by in 1..bh {
                if slice_of(self.st, by) != slice_of(self.st, by - 1) {
                    continue;
                }
                for bx in 0..bw {
                    for s in 0..2 {
                        let go = !p_pic || {
                            let u = blk(self.st, bx, by - 1);
                            let l = blk(self.st, bx, by);
                            u.intra
                                || l.intra
                                || u.mv != l.mv
                                || coded_side(&u, Side::Bottom, s)
                                || coded_side(&l, Side::Top, s)
                        };
                        if go {
                            filter::hseg(plane, 8 * bx + 4 * s, 8 * by, pq);
                        }
                    }
                }
            }
            if p_pic {
                for by in 0..bh {
                    for bx in 0..bw {
                        let b = blk(self.st, bx, by);
                        if b.intra || !(b.tt == TT_8X4 || b.tt == TT_4X4) {
                            continue;
                        }
                        for s in 0..2 {
                            if internal_coded(&b, true, s) {
                                filter::hseg(plane, 8 * bx + 4 * s, 8 * by + 4, pq);
                            }
                        }
                    }
                }
            }
            // Vertical block edges.
            for by in 0..bh {
                for bx in 1..bw {
                    for s in 0..2 {
                        let go = !p_pic || {
                            let l = blk(self.st, bx - 1, by);
                            let rr = blk(self.st, bx, by);
                            l.intra
                                || rr.intra
                                || l.mv != rr.mv
                                || coded_side(&l, Side::Right, s)
                                || coded_side(&rr, Side::Left, s)
                        };
                        if go {
                            filter::vseg(plane, 8 * bx, 8 * by + 4 * s, pq);
                        }
                    }
                }
            }
            if p_pic {
                for by in 0..bh {
                    for bx in 0..bw {
                        let b = blk(self.st, bx, by);
                        if b.intra || !(b.tt == TT_4X8 || b.tt == TT_4X4) {
                            continue;
                        }
                        for s in 0..2 {
                            if internal_coded(&b, false, s) {
                                filter::vseg(plane, 8 * bx + 4, 8 * by + 4 * s, pq);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Whether the overlap residual buffers are needed for this picture.
pub fn needs_ovl(seq: &Seq, h: &PicHeader) -> bool {
    let adv = seq.profile == Profile::Advanced;
    seq.overlap
        && match h.ptype {
            PicType::I | PicType::Bi => h.pquant >= 9 || (adv && h.condover != 0),
            PicType::P => h.pquant >= 9,
            _ => false,
        }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BType {
    Direct,
    Forward,
    Backward,
    Interpolated,
    Intra,
}

#[derive(Clone, Copy)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// Whether the sub-block of `b` along `side`, half `s` (0: left/top), has coefficients.
fn coded_side(b: &LfBlk, side: Side, s: usize) -> bool {
    match b.tt {
        TT_8X8 => b.sbp != 0,
        TT_8X4 => match side {
            Side::Top => b.sbp & 2 != 0,
            Side::Bottom => b.sbp & 1 != 0,
            // Left/right edges of an 8x4 block: segment s lies in the top (s = 0) or bottom half.
            _ => b.sbp & (2 >> s) != 0,
        },
        TT_4X8 => match side {
            Side::Left => b.sbp & 2 != 0,
            Side::Right => b.sbp & 1 != 0,
            _ => b.sbp & (2 >> s) != 0,
        },
        _ => {
            // 4x4: SB0 SB1 / SB2 SB3 as bits 3..0.
            let sb = match side {
                Side::Top => s,
                Side::Bottom => 2 + s,
                Side::Left => 2 * s,
                Side::Right => 2 * s + 1,
            };
            b.sbp & (8 >> sb) != 0
        }
    }
}

/// Whether the internal edge (horizontal when `horiz`) of a split block has coefficients on
/// either side in segment `s`.
fn internal_coded(b: &LfBlk, horiz: bool, s: usize) -> bool {
    match b.tt {
        TT_8X4 if horiz => b.sbp != 0,
        TT_4X8 if !horiz => b.sbp != 0,
        TT_4X4 => {
            if horiz {
                b.sbp & (8 >> s) != 0 || b.sbp & (8 >> (2 + s)) != 0
            } else {
                b.sbp & (8 >> (2 * s)) != 0 || b.sbp & (8 >> (2 * s + 1)) != 0
            }
        }
        _ => false,
    }
}

/// Transform type signalling within a macroblock (TTMB and the first coded block).
struct TtState {
    first: bool,
    first_tt: u8,
    first_pat: u8,
    mb_level: bool,
    /// Macroblock-level type applying to the blocks after the first.
    mb_tt: Option<u8>,
}

impl TtState {
    fn new(p: &Pic, r: &mut Bits, any_coded: bool) -> Result<TtState, Error> {
        if p.h.ttmbf || !any_coded {
            return Ok(TtState {
                first: false,
                first_tt: p.h.ttfrm,
                first_pat: 0,
                mb_level: p.h.ttmbf,
                mb_tt: if p.h.ttmbf { Some(p.h.ttfrm) } else { None },
            });
        }
        let s = TTMB[rate_index(p.h.pquant)]
            .read(r)
            .ok_or_else(|| malformed("bad TTMB"))?;
        let (tt, mb, pat) = tt_parts(s);
        Ok(TtState {
            first: true,
            first_tt: tt,
            first_pat: pat,
            mb_level: mb,
            mb_tt: if mb { Some(tt) } else { None },
        })
    }
}

fn sign_extend(v: u32, bits: u32) -> i32 {
    if bits == 0 {
        return 0;
    }
    let s = 32 - bits;
    ((v << s) as i32) >> s
}

/// Plane and top-left sample of block `b` of macroblock (x, y).
fn block_origin(x: usize, y: usize, b: usize) -> (usize, usize, usize) {
    if b < 4 {
        (0, 16 * x + 8 * BLK_X[b], 16 * y + 8 * BLK_Y[b])
    } else {
        (b - 3, 8 * x, 8 * y)
    }
}

/// Test switch: intra blocks of P and B pictures use the intra scans (else the inter 8x8 scan
/// of §8.3.6.1.5).
pub static INTRA_SCANS_IN_P: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
