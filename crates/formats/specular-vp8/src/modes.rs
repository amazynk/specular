//! Per-macroblock prediction records from the first partition (RFC 6386 sections 10, 11,
//! 16 and 17): segment ids, skip flags, intra modes, reference frames and motion vectors.

use crate::boolcoder::BoolDecoder;
use crate::header::{Header, Persistent};
use crate::tables::*;

pub(crate) const DC_PRED: u8 = 0;
pub(crate) const V_PRED: u8 = 1;
pub(crate) const H_PRED: u8 = 2;
pub(crate) const TM_PRED: u8 = 3;
pub(crate) const B_PRED: u8 = 4;
pub(crate) const NEARESTMV: u8 = 5;
pub(crate) const NEARMV: u8 = 6;
pub(crate) const ZEROMV: u8 = 7;
pub(crate) const NEWMV: u8 = 8;
pub(crate) const SPLITMV: u8 = 9;

pub(crate) const B_DC_PRED: u8 = 0;
pub(crate) const B_TM_PRED: u8 = 1;
pub(crate) const B_VE_PRED: u8 = 2;
pub(crate) const B_HE_PRED: u8 = 3;

/// A motion vector in eighth luma pixels (decoded quarter-pixel values doubled, as the
/// prediction process uses them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Mv {
    pub x: i32,
    pub y: i32,
}

impl Mv {
    const ZERO: Mv = Mv { x: 0, y: 0 };
    fn is_zero(self) -> bool {
        self.x == 0 && self.y == 0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MbInfo {
    pub y_mode: u8,
    pub uv_mode: u8,
    /// 0 intra, 1 last, 2 golden, 3 altref.
    pub ref_frame: u8,
    pub segment: u8,
    /// The bitstream says the macroblock has no coefficients.
    pub skip: bool,
    pub mv: Mv,
    /// Subblock intra modes (B_PRED; for key frame contexts also the implied ones).
    pub bmodes: [u8; 16],
    /// Subblock vectors (all equal to `mv` unless SPLITMV; zero for intra).
    pub mvs: [Mv; 16],
}

impl MbInfo {
    /// The macroblock has a Y2 (second order) block.
    pub(crate) fn has_y2(&self) -> bool {
        self.y_mode != B_PRED && self.y_mode != SPLITMV
    }
}

/// Clamping bounds for a macroblock's vectors (section 16.3, `vp8_clamp_mv`).
#[derive(Clone, Copy)]
struct Bounds {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl Bounds {
    fn new(col: usize, row: usize, cols: usize, rows: usize) -> Bounds {
        Bounds {
            left: -((col as i32 * 16) << 3) - 128,
            right: (((cols - 1 - col) as i32 * 16) << 3) + 128,
            top: -((row as i32 * 16) << 3) - 128,
            bottom: (((rows - 1 - row) as i32 * 16) << 3) + 128,
        }
    }
    fn clamp(&self, mv: Mv) -> Mv {
        Mv {
            x: mv.x.clamp(self.left, self.right),
            y: mv.y.clamp(self.top, self.bottom),
        }
    }
}

fn read_mv_component(d: &mut BoolDecoder<'_>, p: &[u8; 19]) -> i32 {
    let mut a: i32;
    if d.read(p[0]) {
        a = 0;
        for i in 0..3 {
            a += (d.read(p[9 + i]) as i32) << i;
        }
        for i in (4..10).rev() {
            a += (d.read(p[9 + i]) as i32) << i;
        }
        if a & 0xfff0 == 0 || d.read(p[9 + 3]) {
            a += 8;
        }
    } else {
        a = d.tree(&SMALL_MV_TREE, &p[2..9]) as i32;
    }
    if a != 0 && d.read(p[1]) { -a } else { a }
}

fn read_mv(d: &mut BoolDecoder<'_>, probs: &[[u8; 19]; 2]) -> Mv {
    let y = read_mv_component(d, &probs[0]) * 2;
    let x = read_mv_component(d, &probs[1]) * 2;
    Mv { x, y }
}

/// Split partitionings: the partition index of each subblock (section 16.4).
const SPLITS: [[u8; 16]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 3, 3, 2, 2, 3, 3],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
];
const SPLIT_COUNTS: [usize; 4] = [2, 2, 4, 16];

/// Parses the prediction records of every macroblock. `seg_map` carries segment ids from
/// frame to frame.
pub(crate) fn parse_modes(
    d: &mut BoolDecoder<'_>,
    h: &Header,
    st: &Persistent,
    seg_map: &mut [u8],
    cols: usize,
    rows: usize,
    out: &mut Vec<MbInfo>,
) {
    out.clear();
    out.resize(cols * rows, MbInfo::default());
    // Key frames: the subblock modes above (per column) and to the left.
    let mut above_b = vec![[B_DC_PRED; 4]; if h.key { cols } else { 0 }];
    for row in 0..rows {
        let mut left_b = [B_DC_PRED; 4];
        for col in 0..cols {
            let i = row * cols + col;
            let mut mb = MbInfo::default();
            if st.seg.update_map {
                seg_map[i] = d.tree(&SEGMENT_TREE, &st.seg.tree_probs);
            } else if h.key {
                seg_map[i] = 0;
            }
            mb.segment = seg_map[i];
            mb.skip = h.skip_coded && d.read(h.prob_skip);
            if h.key {
                key_modes(d, &mut mb, &mut above_b[col], &mut left_b);
            } else if d.read(h.prob_intra) {
                inter_modes(d, h, st, &mut mb, out, col, row, cols, rows);
            } else {
                intra_modes(d, st, &mut mb);
            }
            out[i] = mb;
        }
    }
}

fn implied_bmode(y_mode: u8) -> u8 {
    match y_mode {
        V_PRED => B_VE_PRED,
        H_PRED => B_HE_PRED,
        TM_PRED => B_TM_PRED,
        _ => B_DC_PRED,
    }
}

fn key_modes(d: &mut BoolDecoder<'_>, mb: &mut MbInfo, above: &mut [u8; 4], left: &mut [u8; 4]) {
    mb.y_mode = d.tree(&KF_YMODE_TREE, &KF_YMODE_PROB);
    if mb.y_mode == B_PRED {
        for k in 0..16 {
            let a = above[k & 3] as usize;
            let l = left[k >> 2] as usize;
            let p = &KF_BMODE_PROB[(a * 10 + l) * 9..(a * 10 + l) * 9 + 9];
            let m = d.tree(&BMODE_TREE, p);
            mb.bmodes[k] = m;
            above[k & 3] = m;
            left[k >> 2] = m;
        }
    } else {
        let m = implied_bmode(mb.y_mode);
        mb.bmodes = [m; 16];
        *above = [m; 4];
        *left = [m; 4];
    }
    mb.uv_mode = d.tree(&UV_MODE_TREE, &KF_UV_MODE_PROB);
}

fn intra_modes(d: &mut BoolDecoder<'_>, st: &Persistent, mb: &mut MbInfo) {
    mb.y_mode = d.tree(&YMODE_TREE, &st.probs.ymode);
    if mb.y_mode == B_PRED {
        for k in 0..16 {
            mb.bmodes[k] = d.tree(&BMODE_TREE, &BMODE_PROB);
        }
    } else {
        mb.bmodes = [implied_bmode(mb.y_mode); 16];
    }
    mb.uv_mode = d.tree(&UV_MODE_TREE, &st.probs.uv_mode);
}

#[allow(clippy::too_many_arguments)]
fn inter_modes(
    d: &mut BoolDecoder<'_>,
    h: &Header,
    st: &Persistent,
    mb: &mut MbInfo,
    done: &[MbInfo],
    col: usize,
    row: usize,
    cols: usize,
    rows: usize,
) {
    mb.ref_frame = if d.read(h.prob_last) {
        2 + d.read(h.prob_golden) as u8
    } else {
        1
    };
    // Survey the neighbours (section 16.3).
    let above = (row > 0).then(|| &done[(row - 1) * cols + col]);
    let left = (col > 0).then(|| &done[row * cols + col - 1]);
    let above_left = (row > 0 && col > 0).then(|| &done[(row - 1) * cols + col - 1]);
    let mut near = [Mv::ZERO; 4];
    let mut cnt = [0i32; 4];
    let mut n = 0usize;
    let bias = |m: &MbInfo| {
        if h.sign_bias[m.ref_frame as usize] != h.sign_bias[mb.ref_frame as usize] {
            Mv {
                x: -m.mv.x,
                y: -m.mv.y,
            }
        } else {
            m.mv
        }
    };
    for (nb, weight) in [(above, 2), (left, 2), (above_left, 1)] {
        let Some(m) = nb.filter(|m| m.ref_frame != 0) else {
            continue;
        };
        if m.mv.is_zero() {
            cnt[0] += weight;
            continue;
        }
        let v = bias(m);
        if n == 0 || v != near[n] {
            n += 1;
            near[n] = v;
        }
        cnt[n] += weight;
    }
    // Three distinct vectors: merge above-left into nearest when they agree.
    if cnt[3] > 0 && near[3] == near[1] {
        cnt[1] += 1;
    }
    let is_split = |m: Option<&MbInfo>| m.is_some_and(|m| m.y_mode == SPLITMV) as i32;
    cnt[3] = (is_split(above) + is_split(left)) * 2 + is_split(above_left);
    if cnt[2] > cnt[1] {
        cnt.swap(1, 2);
        near.swap(1, 2);
    }
    if cnt[1] >= cnt[0] {
        near[0] = near[1];
    }
    let bounds = Bounds::new(col, row, cols, rows);
    let best = bounds.clamp(near[0]);
    let probs = [
        MODE_CONTEXTS[cnt[0] as usize * 4],
        MODE_CONTEXTS[cnt[1] as usize * 4 + 1],
        MODE_CONTEXTS[cnt[2] as usize * 4 + 2],
        MODE_CONTEXTS[cnt[3] as usize * 4 + 3],
    ];
    mb.y_mode = NEARESTMV + d.tree(&MV_REF_TREE, &probs);
    match mb.y_mode {
        NEARESTMV => mb.mv = bounds.clamp(near[1]),
        NEARMV => mb.mv = bounds.clamp(near[2]),
        ZEROMV => mb.mv = Mv::ZERO,
        NEWMV => {
            let delta = read_mv(d, &st.probs.mv);
            mb.mv = Mv {
                x: best.x + delta.x,
                y: best.y + delta.y,
            };
        }
        _ => {
            split_mvs(d, st, mb, left, above, best);
            mb.mv = mb.mvs[15];
            return;
        }
    }
    mb.mvs = [mb.mv; 16];
}

fn split_mvs(
    d: &mut BoolDecoder<'_>,
    st: &Persistent,
    mb: &mut MbInfo,
    left: Option<&MbInfo>,
    above: Option<&MbInfo>,
    best: Mv,
) {
    let kind = d.tree(&MV_PARTITION_TREE, &MV_PARTITION_PROB) as usize;
    let parts = &SPLITS[kind];
    for j in 0..SPLIT_COUNTS[kind] {
        let k = parts.iter().position(|&p| p as usize == j).unwrap_or(0);
        let l = if k & 3 != 0 {
            mb.mvs[k - 1]
        } else {
            left.map_or(Mv::ZERO, |m| m.mvs[k + 3])
        };
        let a = if k >= 4 {
            mb.mvs[k - 4]
        } else {
            above.map_or(Mv::ZERO, |m| m.mvs[k + 12])
        };
        let ctx = if l == a {
            if l.is_zero() { 4 } else { 3 }
        } else if a.is_zero() {
            2
        } else if l.is_zero() {
            1
        } else {
            0
        };
        let v = match d.tree(&SUB_MV_REF_TREE, &SUB_MV_REF_PROB[ctx * 3..ctx * 3 + 3]) {
            0 => l,
            1 => a,
            2 => Mv::ZERO,
            _ => {
                let delta = read_mv(d, &st.probs.mv);
                Mv {
                    x: best.x + delta.x,
                    y: best.y + delta.y,
                }
            }
        };
        for (m, &p) in mb.mvs.iter_mut().zip(parts) {
            if p as usize == j {
                *m = v;
            }
        }
    }
}
