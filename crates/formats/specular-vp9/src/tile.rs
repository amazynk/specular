//! Tile decoding (section 6.4): partitions, mode info, motion vector prediction (6.5),
//! coefficient tokens, and the prediction and reconstruction of each block.

use crate::bits::BoolDecoder;
use crate::frame::{FrameBuf, Pixel, PlaneCols};
use crate::header::{
    COMPOUND_REFERENCE, Compressed, FrameHeader, REFERENCE_MODE_SELECT, SEG_LVL_REF_FRAME,
    SEG_LVL_SKIP, SWITCHABLE, TX_MODE_SELECT,
};
use crate::inter::{self, McScratch};
use crate::intra::{self, IntraArgs};
use crate::itx::{self, ADST_DCT, DCT_ADST, DCT_DCT, Dst};
use crate::probs::{Counts, PARETO_FULL, Probs};
use crate::tables::*;
use crate::{Result, malformed};

pub(crate) const BLOCK_8X8: usize = 3;
pub(crate) const BLOCK_64X64: usize = 12;
pub(crate) const BLOCK_INVALID: u8 = 14;
pub(crate) const NEARESTMV: u8 = 10;
pub(crate) const NEARMV: u8 = 11;
pub(crate) const ZEROMV: u8 = 12;
pub(crate) const NEWMV: u8 = 13;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Mv {
    pub row: i16,
    pub col: i16,
}

/// What later blocks, the loop filter and the next frame need of each 8x8 block.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MiInfo {
    pub size: u8,
    pub skip: bool,
    pub tx_size: u8,
    pub y_mode: u8,
    pub seg_id: u8,
    pub interp_filter: u8,
    pub sub_modes: [u8; 4],
    /// INTRA_FRAME (0), LAST..ALTREF (1..3), NONE (-1).
    pub ref_frame: [i8; 2],
    pub mv: [[Mv; 4]; 2],
}

impl Default for MiInfo {
    fn default() -> Self {
        MiInfo {
            size: 0,
            skip: false,
            tx_size: 0,
            y_mode: 0,
            seg_id: 0,
            interp_filter: 0,
            sub_modes: [0; 4],
            ref_frame: [0, -1],
            mv: [[Mv::default(); 4]; 2],
        }
    }
}

/// Reference scaling of one of LAST, GOLDEN, ALTREF (8.5.2.3).
#[derive(Clone, Copy, Default)]
pub(crate) struct Scale {
    pub x: i64,
    pub y: i64,
    pub step_x: i64,
    pub step_y: i64,
    pub valid: bool,
}

/// Everything the tiles of a frame share (read-only).
pub(crate) struct FrameCtx<'a, P> {
    pub h: &'a FrameHeader,
    pub c: Compressed,
    pub fc: &'a Probs,
    pub refs: [Option<&'a FrameBuf<P>>; 3],
    pub scale: [Scale; 3],
    /// The previous frame's mode info when UsePrevFrameMvs.
    pub prev_mi: Option<&'a [MiInfo]>,
    pub prev_seg: &'a [u8],
    /// Quantizer values per segment: y dc, y ac, uv dc, uv ac.
    pub q: [[i32; 4]; 8],
    pub count: bool,
    pub ss_x: usize,
    pub ss_y: usize,
}

/// One tile's mutable share of the frame.
pub(crate) struct TileData<'s, P> {
    pub planes: [PlaneCols<'s, P>; 3],
    /// Mode info rows (all MiRows), columns from the tile start.
    pub mi: Vec<&'s mut [MiInfo]>,
    pub above_nz: [&'s mut [u8]; 3],
    pub above_part: &'s mut [u8],
    pub above_seg: &'s mut [u8],
}

pub(crate) struct Tile<'s, 'a, P> {
    f: &'s FrameCtx<'a, P>,
    bd: BoolDecoder<'a>,
    col_start: usize,
    col_end: usize,
    mi_rows: usize,
    mi_cols: usize,
    d: TileData<'s, P>,
    left_nz: [[u8; 16]; 3],
    left_part: [u8; 8],
    left_seg: [u8; 8],
    pub counts: Box<Counts>,
    coef: Vec<i32>,
    token_cache: Vec<u8>,
    mc: McScratch<P>,
    pred: [Vec<u16>; 2],
}

/// The block being decoded.
#[derive(Clone, Copy, Default)]
struct Blk {
    r: usize,
    c: usize,
    size: usize,
    avail_u: bool,
    avail_l: bool,
    seg_id: usize,
    skip: bool,
    is_inter: bool,
    tx_size: usize,
    y_mode: u8,
    sub_modes: [u8; 4],
    uv_mode: u8,
    ref_frame: [i8; 2],
    interp_filter: u8,
    mv: [[Mv; 4]; 2],
}

struct ScanSet {
    scan: &'static [u16],
    nb: Vec<[u16; 2]>,
}

/// Scans and coefficient neighbours per transform size and type (6.4.25, 9.3.2).
static SCANS: std::sync::LazyLock<Vec<ScanSet>> = std::sync::LazyLock::new(|| {
    let mut v = Vec::new();
    for tx in 0..4 {
        for ty in [DCT_DCT, ADST_DCT, DCT_ADST] {
            let scan: &'static [u16] = match (tx, ty) {
                (0, ADST_DCT) => &ROW_SCAN_4X4,
                (0, DCT_ADST) => &COL_SCAN_4X4,
                (0, _) => &DEFAULT_SCAN_4X4,
                (1, ADST_DCT) => &ROW_SCAN_8X8,
                (1, DCT_ADST) => &COL_SCAN_8X8,
                (1, _) => &DEFAULT_SCAN_8X8,
                (2, ADST_DCT) => &ROW_SCAN_16X16,
                (2, DCT_ADST) => &COL_SCAN_16X16,
                (2, _) => &DEFAULT_SCAN_16X16,
                _ => &DEFAULT_SCAN_32X32,
            };
            let n = 4usize << tx;
            let nb = scan
                .iter()
                .map(|&pos| {
                    let pos = pos as usize;
                    let (i, j) = (pos / n, pos % n);
                    let (a, b) = if i > 0 && j > 0 {
                        let up = (i - 1) * n + j;
                        let left = i * n + j - 1;
                        match ty {
                            DCT_ADST => (up, up),
                            ADST_DCT => (left, left),
                            _ => (up, left),
                        }
                    } else if i > 0 {
                        ((i - 1) * n + j, (i - 1) * n + j)
                    } else if j > 0 {
                        (j - 1, j - 1)
                    } else {
                        (0, 0)
                    };
                    [a as u16, b as u16]
                })
                .collect();
            v.push(ScanSet { scan, nb });
        }
    }
    v
});

fn scan_set(tx: usize, ty: u8) -> &'static ScanSet {
    let ty = if tx == 3 { DCT_DCT } else { ty };
    let k = match ty {
        ADST_DCT => 1,
        DCT_ADST => 2,
        _ => 0,
    };
    &SCANS[tx * 3 + k]
}

/// Warms up the lazily built tables.
pub(crate) fn init_tables() {
    let _ = &*SCANS;
    let _ = &*PARETO_FULL;
}

#[inline(always)]
fn uv_tx_size(size: usize, tx: usize, ss_x: usize, ss_y: usize) -> usize {
    if size < BLOCK_8X8 {
        return 0;
    }
    let p = SS_SIZE_LOOKUP[size][ss_x][ss_y] as usize;
    tx.min(MAX_TXSIZE_LOOKUP[p.min(12)] as usize)
}

impl<'s, 'a, P: Pixel> Tile<'s, 'a, P> {
    pub(crate) fn new(
        f: &'s FrameCtx<'a, P>,
        data: &'a [u8],
        col_start: usize,
        col_end: usize,
        d: TileData<'s, P>,
    ) -> Result<Self> {
        Ok(Tile {
            f,
            bd: BoolDecoder::new(data)?,
            col_start,
            col_end,
            mi_rows: f.h.mi_rows,
            mi_cols: f.h.mi_cols,
            d,
            left_nz: [[0; 16]; 3],
            left_part: [0; 8],
            left_seg: [0; 8],
            counts: Box::default(),
            coef: vec![0; 1024],
            token_cache: vec![0; 1024],
            mc: McScratch::new(),
            pred: [vec![0; 64 * 64], vec![0; 64 * 64]],
        })
    }

    #[inline(always)]
    fn mi(&self, r: usize, c: usize) -> &MiInfo {
        &self.d.mi[r][c - self.col_start]
    }

    /// decode_tile for mode-info rows `row_start..row_end`.
    pub(crate) fn decode(&mut self, row_start: usize, row_end: usize) -> Result<()> {
        let mut r = row_start;
        while r < row_end {
            self.left_nz = [[0; 16]; 3];
            self.left_part = [0; 8];
            self.left_seg = [0; 8];
            let mut c = self.col_start;
            while c < self.col_end {
                self.partition(r, c, BLOCK_64X64)?;
                c += 8;
            }
            r += 8;
        }
        Ok(())
    }

    fn partition(&mut self, r: usize, c: usize, bsize: usize) -> Result<()> {
        if r >= self.mi_rows || c >= self.mi_cols {
            return Ok(());
        }
        let n8 = NUM_8X8_BLOCKS_WIDE_LOOKUP[bsize] as usize;
        let half = n8 >> 1;
        let has_rows = r + half < self.mi_rows;
        let has_cols = c + half < self.mi_cols;
        let bsl = MI_WIDTH_LOG2_LOOKUP[bsize] as usize;
        let boffset = 3 - bsl;
        let mut above = 0u8;
        let mut left = 0u8;
        for i in 0..n8 {
            above |= self.d.above_part[c - self.col_start + i];
            left |= self.left_part[(r + i) & 7];
        }
        let above = (above >> boffset) & 1;
        let left = (left >> boffset) & 1;
        let ctx = bsl * 4 + left as usize * 2 + above as usize;
        let probs = if self.f.h.intra {
            &KF_PARTITION_PROBS[ctx]
        } else {
            &self.f.fc.partition[ctx]
        };
        let p = if has_rows && has_cols {
            self.bd.tree(&PARTITION_TREE, |n| probs[n])
        } else if has_cols {
            if self.bd.read(probs[1]) { 3 } else { 1 }
        } else if has_rows {
            if self.bd.read(probs[2]) { 3 } else { 2 }
        } else {
            3
        };
        if self.f.count {
            self.counts.partition[ctx][p] += 1;
        }
        let sub = SUBSIZE_LOOKUP[p][bsize] as usize;
        if sub >= BLOCK_8X8 && SS_SIZE_LOOKUP[sub][self.f.ss_x][self.f.ss_y] == BLOCK_INVALID {
            return Err(malformed(
                "block size not allowed with this chroma subsampling",
            ));
        }
        if sub < BLOCK_8X8 || p == 0 {
            self.block(r, c, sub)?;
        } else if p == 1 {
            self.block(r, c, sub)?;
            if has_rows {
                self.block(r + half, c, sub)?;
            }
        } else if p == 2 {
            self.block(r, c, sub)?;
            if has_cols {
                self.block(r, c + half, sub)?;
            }
        } else {
            self.partition(r, c, sub)?;
            self.partition(r, c + half, sub)?;
            self.partition(r + half, c, sub)?;
            self.partition(r + half, c + half, sub)?;
        }
        if bsize == BLOCK_8X8 || p != 3 {
            let a = 15 >> B_WIDTH_LOG2_LOOKUP[sub];
            let l = 15 >> B_HEIGHT_LOG2_LOOKUP[sub];
            for i in 0..n8 {
                self.d.above_part[c - self.col_start + i] = a;
                self.left_part[(r + i) & 7] = l;
            }
        }
        Ok(())
    }

    fn block(&mut self, r: usize, c: usize, size: usize) -> Result<()> {
        let mut b = Blk {
            r,
            c,
            size,
            avail_u: r > 0,
            avail_l: c > self.col_start,
            ref_frame: [0, -1],
            ..Default::default()
        };
        let above = b.avail_u.then(|| *self.mi(r - 1, c));
        let left = b.avail_l.then(|| *self.mi(r, c - 1));
        if self.f.h.intra {
            self.intra_frame_mode_info(&mut b, above.as_ref(), left.as_ref());
        } else {
            self.inter_frame_mode_info(&mut b, above.as_ref(), left.as_ref())?;
        }
        let eob_total = self.residual(&b)?;
        if b.is_inter && size >= BLOCK_8X8 && eob_total == 0 {
            b.skip = true;
        }
        let info = MiInfo {
            size: size as u8,
            skip: b.skip,
            tx_size: b.tx_size as u8,
            y_mode: b.y_mode,
            seg_id: b.seg_id as u8,
            interp_filter: b.interp_filter,
            sub_modes: b.sub_modes,
            ref_frame: b.ref_frame,
            mv: b.mv,
        };
        let h8 = (NUM_8X8_BLOCKS_HIGH_LOOKUP[size] as usize).min(self.mi_rows - r);
        let w8 = (NUM_8X8_BLOCKS_WIDE_LOOKUP[size] as usize).min(self.col_end - c);
        for y in 0..h8 {
            let row = &mut self.d.mi[r + y][c - self.col_start..c - self.col_start + w8];
            row.fill(info);
        }
        Ok(())
    }

    fn read_skip(&mut self, b: &mut Blk, above: Option<&MiInfo>, left: Option<&MiInfo>) {
        if self.f.h.seg.active(b.seg_id, SEG_LVL_SKIP) {
            b.skip = true;
        } else {
            let ctx = above.map_or(0, |a| a.skip as usize) + left.map_or(0, |l| l.skip as usize);
            b.skip = self.bd.read(self.f.fc.skip[ctx]);
            if self.f.count {
                self.counts.skip[ctx][b.skip as usize] += 1;
            }
        }
    }

    fn read_tx_size(
        &mut self,
        b: &mut Blk,
        allow_select: bool,
        above: Option<&MiInfo>,
        left: Option<&MiInfo>,
    ) {
        let max_tx = MAX_TXSIZE_LOOKUP[b.size] as usize;
        let tx_mode = self.f.c.tx_mode;
        if allow_select && tx_mode == TX_MODE_SELECT && b.size >= BLOCK_8X8 {
            let mut a = max_tx;
            let mut l = max_tx;
            if let Some(m) = above
                && !m.skip
            {
                a = m.tx_size as usize;
            }
            if let Some(m) = left
                && !m.skip
            {
                l = m.tx_size as usize;
            }
            if left.is_none() {
                l = a;
            }
            if above.is_none() {
                a = l;
            }
            let ctx = (a + l > max_tx) as usize;
            let probs = &self.f.fc.tx[max_tx][ctx];
            let tree: &[i8] = match max_tx {
                3 => &TX_SIZE_32_TREE,
                2 => &TX_SIZE_16_TREE,
                _ => &TX_SIZE_8_TREE,
            };
            let tx = self.bd.tree(tree, |n| probs[n]);
            if self.f.count {
                self.counts.tx[max_tx][ctx][tx] += 1;
            }
            b.tx_size = tx;
        } else {
            b.tx_size = max_tx.min(TX_MODE_TO_BIGGEST_TX_SIZE[tx_mode as usize] as usize);
        }
    }

    fn read_segment_id(&mut self) -> usize {
        let p = self.f.h.seg.tree_probs;
        self.bd.tree(&SEGMENT_TREE, |n| p[n])
    }

    fn intra_frame_mode_info(
        &mut self,
        b: &mut Blk,
        above: Option<&MiInfo>,
        left: Option<&MiInfo>,
    ) {
        let seg = &self.f.h.seg;
        b.seg_id = if seg.enabled && seg.update_map {
            self.read_segment_id()
        } else {
            0
        };
        self.read_skip(b, above, left);
        self.read_tx_size(b, true, above, left);
        b.ref_frame = [0, -1];
        b.is_inter = false;
        if b.size >= BLOCK_8X8 {
            let am = above.map_or(0, |a| a.sub_modes[2]) as usize;
            let lm = left.map_or(0, |l| l.sub_modes[1]) as usize;
            let m = self
                .bd
                .tree(&INTRA_MODE_TREE, |n| KF_Y_MODE_PROBS[am][lm][n]) as u8;
            b.y_mode = m;
            b.sub_modes = [m; 4];
        } else {
            let w4 = NUM_4X4_BLOCKS_WIDE_LOOKUP[b.size] as usize;
            let h4 = NUM_4X4_BLOCKS_HIGH_LOOKUP[b.size] as usize;
            let mut m = 0;
            for idy in (0..2).step_by(h4) {
                for idx in (0..2).step_by(w4) {
                    let am = if idy > 0 {
                        b.sub_modes[idx]
                    } else {
                        above.map_or(0, |a| a.sub_modes[2 + idx])
                    } as usize;
                    let lm = if idx > 0 {
                        b.sub_modes[idy * 2]
                    } else {
                        left.map_or(0, |l| l.sub_modes[1 + idy * 2])
                    } as usize;
                    m = self
                        .bd
                        .tree(&INTRA_MODE_TREE, |n| KF_Y_MODE_PROBS[am][lm][n])
                        as u8;
                    for y2 in 0..h4 {
                        for x2 in 0..w4 {
                            b.sub_modes[(idy + y2) * 2 + idx + x2] = m;
                        }
                    }
                }
            }
            b.y_mode = m;
        }
        let ym = b.y_mode as usize;
        b.uv_mode = self.bd.tree(&INTRA_MODE_TREE, |n| KF_UV_MODE_PROBS[ym][n]) as u8;
    }

    fn get_segment_id(&self, b: &Blk) -> usize {
        let bw = NUM_8X8_BLOCKS_WIDE_LOOKUP[b.size] as usize;
        let bh = NUM_8X8_BLOCKS_HIGH_LOOKUP[b.size] as usize;
        let xmis = (self.mi_cols - b.c).min(bw);
        let ymis = (self.mi_rows - b.r).min(bh);
        let mut seg = 7;
        for y in 0..ymis {
            for x in 0..xmis {
                seg = seg.min(self.f.prev_seg[(b.r + y) * self.mi_cols + b.c + x] as usize);
            }
        }
        seg
    }

    fn inter_frame_mode_info(
        &mut self,
        b: &mut Blk,
        above: Option<&MiInfo>,
        left: Option<&MiInfo>,
    ) -> Result<()> {
        let left_ref = left.map_or([0, -1], |l| l.ref_frame);
        let above_ref = above.map_or([0, -1], |a| a.ref_frame);
        let left_intra = left_ref[0] <= 0;
        let above_intra = above_ref[0] <= 0;
        // inter_segment_id
        let seg = self.f.h.seg;
        if seg.enabled {
            let pred = self.get_segment_id(b);
            if seg.update_map {
                if seg.temporal_update {
                    let ctx = self.left_seg[b.r & 7] as usize
                        + self.d.above_seg[b.c - self.col_start] as usize;
                    let predicted = self.bd.read(seg.pred_probs[ctx]);
                    b.seg_id = if predicted {
                        pred
                    } else {
                        self.read_segment_id()
                    };
                    let bw = NUM_8X8_BLOCKS_WIDE_LOOKUP[b.size] as usize;
                    let bh = NUM_8X8_BLOCKS_HIGH_LOOKUP[b.size] as usize;
                    for i in 0..bw {
                        self.d.above_seg[b.c - self.col_start + i] = predicted as u8;
                    }
                    for i in 0..bh {
                        self.left_seg[(b.r + i) & 7] = predicted as u8;
                    }
                } else {
                    b.seg_id = self.read_segment_id();
                }
            } else {
                b.seg_id = pred;
            }
        }
        self.read_skip(b, above, left);
        // read_is_inter
        if seg.active(b.seg_id, SEG_LVL_REF_FRAME) {
            b.is_inter = seg.feature_data[b.seg_id][SEG_LVL_REF_FRAME] != 0;
        } else {
            let ctx = match (above.is_some(), left.is_some()) {
                (true, true) => {
                    if left_intra && above_intra {
                        3
                    } else {
                        (left_intra || above_intra) as usize
                    }
                }
                (true, false) => 2 * above_intra as usize,
                (false, true) => 2 * left_intra as usize,
                (false, false) => 0,
            };
            b.is_inter = self.bd.read(self.f.fc.is_inter[ctx]);
            if self.f.count {
                self.counts.is_inter[ctx][b.is_inter as usize] += 1;
            }
        }
        let allow = !b.skip || !b.is_inter;
        self.read_tx_size(b, allow, above, left);
        if b.is_inter {
            self.inter_block_mode_info(b, above, left, left_ref, above_ref)
        } else {
            self.intra_block_mode_info(b);
            Ok(())
        }
    }

    fn intra_block_mode_info(&mut self, b: &mut Blk) {
        b.ref_frame = [0, -1];
        let fc = self.f.fc;
        if b.size >= BLOCK_8X8 {
            let ctx = SIZE_GROUP_LOOKUP[b.size] as usize;
            let m = self.bd.tree(&INTRA_MODE_TREE, |n| fc.y_mode[ctx][n]);
            if self.f.count {
                self.counts.intra_mode[ctx][m] += 1;
            }
            b.y_mode = m as u8;
            b.sub_modes = [m as u8; 4];
        } else {
            let w4 = NUM_4X4_BLOCKS_WIDE_LOOKUP[b.size] as usize;
            let h4 = NUM_4X4_BLOCKS_HIGH_LOOKUP[b.size] as usize;
            let mut m = 0;
            for idy in (0..2).step_by(h4) {
                for idx in (0..2).step_by(w4) {
                    m = self.bd.tree(&INTRA_MODE_TREE, |n| fc.y_mode[0][n]);
                    if self.f.count {
                        self.counts.intra_mode[0][m] += 1;
                    }
                    for y2 in 0..h4 {
                        for x2 in 0..w4 {
                            b.sub_modes[(idy + y2) * 2 + idx + x2] = m as u8;
                        }
                    }
                }
            }
            b.y_mode = m as u8;
        }
        let ym = b.y_mode as usize;
        let uv = self.bd.tree(&INTRA_MODE_TREE, |n| fc.uv_mode[ym][n]);
        if self.f.count {
            self.counts.uv_mode[ym][uv] += 1;
        }
        b.uv_mode = uv as u8;
    }

    #[allow(clippy::too_many_arguments)]
    fn inter_block_mode_info(
        &mut self,
        b: &mut Blk,
        above: Option<&MiInfo>,
        left: Option<&MiInfo>,
        left_ref: [i8; 2],
        above_ref: [i8; 2],
    ) -> Result<()> {
        self.read_ref_frames(b, above.is_some(), left.is_some(), left_ref, above_ref);
        let mut best = [Mv::default(); 2];
        let mut nearest = [Mv::default(); 2];
        let mut near = [Mv::default(); 2];
        let mut mode_ctx = 0usize;
        for j in 0..2 {
            if b.ref_frame[j] > 0 {
                let (list, ctx) = self.find_mv_refs(b, b.ref_frame[j], -1);
                if j == 0 {
                    mode_ctx = ctx as usize;
                }
                let l = self.find_best_ref_mvs(b, list);
                nearest[j] = l[0];
                near[j] = l[1];
                best[j] = l[0];
            }
        }
        let is_compound = b.ref_frame[1] > 0;
        let fc = self.f.fc;
        if self.f.h.seg.active(b.seg_id, SEG_LVL_SKIP) {
            b.y_mode = ZEROMV;
        } else if b.size >= BLOCK_8X8 {
            let m = self
                .bd
                .tree(&INTER_MODE_TREE, |n| fc.inter_mode[mode_ctx][n]);
            if self.f.count {
                self.counts.inter_mode[mode_ctx][m] += 1;
            }
            b.y_mode = NEARESTMV + m as u8;
        }
        if self.f.h.interp_filter == SWITCHABLE {
            let li = if left.is_some() && left_ref[0] > 0 {
                left.map_or(3, |l| l.interp_filter)
            } else {
                3
            };
            let ai = if above.is_some() && above_ref[0] > 0 {
                above.map_or(3, |a| a.interp_filter)
            } else {
                3
            };
            let ctx = if li == ai {
                li
            } else if li == 3 && ai != 3 {
                ai
            } else if li != 3 && ai == 3 {
                li
            } else {
                3
            } as usize;
            let f = self
                .bd
                .tree(&INTERP_FILTER_TREE, |n| fc.interp_filter[ctx][n]);
            if self.f.count {
                self.counts.interp_filter[ctx][f] += 1;
            }
            b.interp_filter = f as u8;
        } else {
            b.interp_filter = self.f.h.interp_filter;
        }
        let nrefs = 1 + is_compound as usize;
        if b.size < BLOCK_8X8 {
            let w4 = NUM_4X4_BLOCKS_WIDE_LOOKUP[b.size] as usize;
            let h4 = NUM_4X4_BLOCKS_HIGH_LOOKUP[b.size] as usize;
            for idy in (0..2).step_by(h4) {
                for idx in (0..2).step_by(w4) {
                    let m = self
                        .bd
                        .tree(&INTER_MODE_TREE, |n| fc.inter_mode[mode_ctx][n]);
                    if self.f.count {
                        self.counts.inter_mode[mode_ctx][m] += 1;
                    }
                    b.y_mode = NEARESTMV + m as u8;
                    let blk = idy * 2 + idx;
                    if b.y_mode == NEARESTMV || b.y_mode == NEARMV {
                        for j in 0..nrefs {
                            let (ns, nr) = self.append_sub8x8_mvs(b, blk, j);
                            nearest[j] = ns;
                            near[j] = nr;
                        }
                    }
                    let mv = self.assign_mv(b, is_compound, &nearest, &near, &best);
                    for y2 in 0..h4 {
                        for x2 in 0..w4 {
                            let k = (idy + y2) * 2 + idx + x2;
                            for (rl, m) in mv.iter().enumerate().take(nrefs) {
                                b.mv[rl][k] = *m;
                            }
                        }
                    }
                }
            }
        } else {
            let mv = self.assign_mv(b, is_compound, &nearest, &near, &best);
            for rl in 0..nrefs {
                b.mv[rl] = [mv[rl]; 4];
            }
        }
        Ok(())
    }

    fn read_ref_frames(
        &mut self,
        b: &mut Blk,
        avail_u: bool,
        avail_l: bool,
        left_ref: [i8; 2],
        above_ref: [i8; 2],
    ) {
        let seg = &self.f.h.seg;
        if seg.active(b.seg_id, SEG_LVL_REF_FRAME) {
            b.ref_frame = [seg.feature_data[b.seg_id][SEG_LVL_REF_FRAME] as i8, -1];
            return;
        }
        let c = self.f.c;
        let fc = self.f.fc;
        let left_intra = left_ref[0] <= 0;
        let above_intra = above_ref[0] <= 0;
        let left_single = left_ref[1] <= 0;
        let above_single = above_ref[1] <= 0;
        let fixed = c.comp_fixed_ref;
        let comp_mode = if c.reference_mode == REFERENCE_MODE_SELECT {
            let ctx = if avail_u && avail_l {
                if above_single && left_single {
                    ((above_ref[0] == fixed) ^ (left_ref[0] == fixed)) as usize
                } else if above_single {
                    2 + (above_ref[0] == fixed || above_intra) as usize
                } else if left_single {
                    2 + (left_ref[0] == fixed || left_intra) as usize
                } else {
                    4
                }
            } else if avail_u {
                if above_single {
                    (above_ref[0] == fixed) as usize
                } else {
                    3
                }
            } else if avail_l {
                if left_single {
                    (left_ref[0] == fixed) as usize
                } else {
                    3
                }
            } else {
                1
            };
            let v = self.bd.read(fc.comp_mode[ctx]);
            if self.f.count {
                self.counts.comp_mode[ctx][v as usize] += 1;
            }
            v as u8
        } else {
            c.reference_mode
        };
        if comp_mode == COMPOUND_REFERENCE {
            let sb = &self.f.h.sign_bias;
            let idx = sb[fixed as usize] as usize;
            let var_idx = 1 - idx;
            let var1 = c.comp_var_ref[1];
            let ctx = if avail_u && avail_l {
                if above_intra && left_intra {
                    2
                } else if left_intra {
                    if above_single {
                        1 + 2 * (above_ref[0] != var1) as usize
                    } else {
                        1 + 2 * (above_ref[var_idx] != var1) as usize
                    }
                } else if above_intra {
                    if left_single {
                        1 + 2 * (left_ref[0] != var1) as usize
                    } else {
                        1 + 2 * (left_ref[var_idx] != var1) as usize
                    }
                } else {
                    let vrfa = if above_single {
                        above_ref[0]
                    } else {
                        above_ref[var_idx]
                    };
                    let vrfl = if left_single {
                        left_ref[0]
                    } else {
                        left_ref[var_idx]
                    };
                    if vrfa == vrfl && var1 == vrfa {
                        0
                    } else if left_single && above_single {
                        let var0 = c.comp_var_ref[0];
                        if (vrfa == fixed && vrfl == var0) || (vrfl == fixed && vrfa == var0) {
                            4
                        } else if vrfa == vrfl {
                            3
                        } else {
                            1
                        }
                    } else if left_single || above_single {
                        let vrfc = if left_single { vrfa } else { vrfl };
                        let rfs = if above_single { vrfa } else { vrfl };
                        if vrfc == var1 && rfs != var1 {
                            1
                        } else if rfs == var1 && vrfc != var1 {
                            2
                        } else {
                            4
                        }
                    } else if vrfa == vrfl {
                        4
                    } else {
                        2
                    }
                }
            } else if avail_u {
                if above_intra {
                    2
                } else if above_single {
                    3 * (above_ref[0] != var1) as usize
                } else {
                    4 * (above_ref[var_idx] != var1) as usize
                }
            } else if avail_l {
                if left_intra {
                    2
                } else if left_single {
                    3 * (left_ref[0] != var1) as usize
                } else {
                    4 * (left_ref[var_idx] != var1) as usize
                }
            } else {
                2
            };
            let v = self.bd.read(fc.comp_ref[ctx]);
            if self.f.count {
                self.counts.comp_ref[ctx][v as usize] += 1;
            }
            b.ref_frame[idx] = fixed;
            b.ref_frame[var_idx] = c.comp_var_ref[v as usize];
        } else {
            const LAST: i8 = 1;
            const GOLDEN: i8 = 2;
            const ALTREF: i8 = 3;
            let ctx = if avail_u && avail_l {
                if above_intra && left_intra {
                    2
                } else if left_intra {
                    if above_single {
                        4 * (above_ref[0] == LAST) as usize
                    } else {
                        1 + (above_ref[0] == LAST || above_ref[1] == LAST) as usize
                    }
                } else if above_intra {
                    if left_single {
                        4 * (left_ref[0] == LAST) as usize
                    } else {
                        1 + (left_ref[0] == LAST || left_ref[1] == LAST) as usize
                    }
                } else if above_single && left_single {
                    2 * (above_ref[0] == LAST) as usize + 2 * (left_ref[0] == LAST) as usize
                } else if !above_single && !left_single {
                    1 + (above_ref[0] == LAST
                        || above_ref[1] == LAST
                        || left_ref[0] == LAST
                        || left_ref[1] == LAST) as usize
                } else {
                    let rfs = if above_single {
                        above_ref[0]
                    } else {
                        left_ref[0]
                    };
                    let crf1 = if above_single {
                        left_ref[0]
                    } else {
                        above_ref[0]
                    };
                    let crf2 = if above_single {
                        left_ref[1]
                    } else {
                        above_ref[1]
                    };
                    if rfs == LAST {
                        3 + (crf1 == LAST || crf2 == LAST) as usize
                    } else {
                        (crf1 == LAST || crf2 == LAST) as usize
                    }
                }
            } else if avail_u {
                if above_intra {
                    2
                } else if above_single {
                    4 * (above_ref[0] == LAST) as usize
                } else {
                    1 + (above_ref[0] == LAST || above_ref[1] == LAST) as usize
                }
            } else if avail_l {
                if left_intra {
                    2
                } else if left_single {
                    4 * (left_ref[0] == LAST) as usize
                } else {
                    1 + (left_ref[0] == LAST || left_ref[1] == LAST) as usize
                }
            } else {
                2
            };
            let p1 = self.bd.read(fc.single_ref[ctx][0]);
            if self.f.count {
                self.counts.single_ref[ctx][0][p1 as usize] += 1;
            }
            if p1 {
                let ctx = if avail_u && avail_l {
                    if above_intra && left_intra {
                        2
                    } else if left_intra {
                        if above_single {
                            if above_ref[0] == LAST {
                                3
                            } else {
                                4 * (above_ref[0] == GOLDEN) as usize
                            }
                        } else {
                            1 + 2 * (above_ref[0] == GOLDEN || above_ref[1] == GOLDEN) as usize
                        }
                    } else if above_intra {
                        if left_single {
                            if left_ref[0] == LAST {
                                3
                            } else {
                                4 * (left_ref[0] == GOLDEN) as usize
                            }
                        } else {
                            1 + 2 * (left_ref[0] == GOLDEN || left_ref[1] == GOLDEN) as usize
                        }
                    } else if above_single && left_single {
                        if above_ref[0] == LAST && left_ref[0] == LAST {
                            3
                        } else if above_ref[0] == LAST {
                            4 * (left_ref[0] == GOLDEN) as usize
                        } else if left_ref[0] == LAST {
                            4 * (above_ref[0] == GOLDEN) as usize
                        } else {
                            2 * (above_ref[0] == GOLDEN) as usize
                                + 2 * (left_ref[0] == GOLDEN) as usize
                        }
                    } else if !above_single && !left_single {
                        if above_ref[0] == left_ref[0] && above_ref[1] == left_ref[1] {
                            3 * (above_ref[0] == GOLDEN || above_ref[1] == GOLDEN) as usize
                        } else {
                            2
                        }
                    } else {
                        let rfs = if above_single {
                            above_ref[0]
                        } else {
                            left_ref[0]
                        };
                        let crf1 = if above_single {
                            left_ref[0]
                        } else {
                            above_ref[0]
                        };
                        let crf2 = if above_single {
                            left_ref[1]
                        } else {
                            above_ref[1]
                        };
                        if rfs == GOLDEN {
                            3 + (crf1 == GOLDEN || crf2 == GOLDEN) as usize
                        } else if rfs == ALTREF {
                            (crf1 == GOLDEN || crf2 == GOLDEN) as usize
                        } else {
                            1 + 2 * (crf1 == GOLDEN || crf2 == GOLDEN) as usize
                        }
                    }
                } else if avail_u {
                    if above_intra || (above_ref[0] == LAST && above_single) {
                        2
                    } else if above_single {
                        4 * (above_ref[0] == GOLDEN) as usize
                    } else {
                        3 * (above_ref[0] == GOLDEN || above_ref[1] == GOLDEN) as usize
                    }
                } else if avail_l {
                    if left_intra || (left_ref[0] == LAST && left_single) {
                        2
                    } else if left_single {
                        4 * (left_ref[0] == GOLDEN) as usize
                    } else {
                        3 * (left_ref[0] == GOLDEN || left_ref[1] == GOLDEN) as usize
                    }
                } else {
                    2
                };
                let p2 = self.bd.read(fc.single_ref[ctx][1]);
                if self.f.count {
                    self.counts.single_ref[ctx][1][p2 as usize] += 1;
                }
                b.ref_frame = [if p2 { ALTREF } else { GOLDEN }, -1];
            } else {
                b.ref_frame = [LAST, -1];
            }
        }
    }

    #[inline]
    fn inside(&self, r: isize, c: isize) -> bool {
        r >= 0
            && (r as usize) < self.mi_rows
            && c >= self.col_start as isize
            && (c as usize) < self.col_end
    }

    fn clamp_mv_row(&self, b: &Blk, v: i32, border: i32) -> i32 {
        let bh = NUM_8X8_BLOCKS_HIGH_LOOKUP[b.size] as i32;
        let top = -((b.r as i32 * 8) * 8);
        let bottom = (self.mi_rows as i32 - bh - b.r as i32) * 64;
        v.clamp(top - border, (bottom + border).max(top - border))
    }

    fn clamp_mv_col(&self, b: &Blk, v: i32, border: i32) -> i32 {
        let bw = NUM_8X8_BLOCKS_WIDE_LOOKUP[b.size] as i32;
        let left = -((b.c as i32 * 8) * 8);
        let right = (self.mi_cols as i32 - bw - b.c as i32) * 64;
        v.clamp(left - border, (right + border).max(left - border))
    }

    /// find_mv_refs (6.5.1): the candidate list (clamped) and the mode context.
    fn find_mv_refs(&self, b: &Blk, ref_frame: i8, block: i32) -> ([Mv; 2], u8) {
        let mut list = [Mv::default(); 2];
        let mut count = 0usize;
        let mut add = |mv: Mv, list: &mut [Mv; 2]| {
            if count >= 2 || (count > 0 && mv == list[0]) {
                return;
            }
            list[count] = mv;
            count += 1;
        };
        let sb = &self.f.h.sign_bias;
        let mut diff_found = false;
        let mut ctx_counter = 0usize;
        let search = &MV_REF_BLOCKS[b.size];
        for (i, s) in search.iter().enumerate().take(2) {
            let cr = b.r as isize + s[0] as isize;
            let cc = b.c as isize + s[1] as isize;
            if self.inside(cr, cc) {
                let cand = self.mi(cr as usize, cc as usize);
                diff_found = true;
                ctx_counter += MODE_2_COUNTER[cand.y_mode as usize] as usize;
                for j in 0..2 {
                    if cand.ref_frame[j] == ref_frame {
                        let idx = if block >= 0 {
                            IDX_N_COLUMN_TO_SUBBLOCK[block as usize][(s[1] == 0) as usize] as usize
                        } else {
                            3
                        };
                        add(cand.mv[j][idx], &mut list);
                        break;
                    }
                }
            }
            let _ = i;
        }
        for s in search.iter().skip(2) {
            let cr = b.r as isize + s[0] as isize;
            let cc = b.c as isize + s[1] as isize;
            if self.inside(cr, cc) {
                diff_found = true;
                let cand = self.mi(cr as usize, cc as usize);
                for j in 0..2 {
                    if cand.ref_frame[j] == ref_frame {
                        add(cand.mv[j][3], &mut list);
                        break;
                    }
                }
            }
        }
        let prev = self.f.prev_mi.map(|p| p[b.r * self.mi_cols + b.c]);
        if let Some(p) = &prev {
            for j in 0..2 {
                if p.ref_frame[j] == ref_frame {
                    add(p.mv[j][3], &mut list);
                    break;
                }
            }
        }
        let diff_add =
            |cand: &MiInfo, list: &mut [Mv; 2], add: &mut dyn FnMut(Mv, &mut [Mv; 2])| {
                let mvs_same = cand.mv[0][3] == cand.mv[1][3];
                let scale = |mv: Mv, f: i8| {
                    if sb[f as usize] != sb[ref_frame as usize] {
                        Mv {
                            row: mv.row.wrapping_neg(),
                            col: mv.col.wrapping_neg(),
                        }
                    } else {
                        mv
                    }
                };
                let f0 = cand.ref_frame[0];
                if f0 > 0 && f0 != ref_frame {
                    add(scale(cand.mv[0][3], f0), list);
                }
                let f1 = cand.ref_frame[1];
                if f1 > 0 && f1 != ref_frame && !mvs_same {
                    add(scale(cand.mv[1][3], f1), list);
                }
            };
        if diff_found {
            for s in search.iter() {
                let cr = b.r as isize + s[0] as isize;
                let cc = b.c as isize + s[1] as isize;
                if self.inside(cr, cc) {
                    let cand = *self.mi(cr as usize, cc as usize);
                    diff_add(&cand, &mut list, &mut add);
                }
            }
        }
        if let Some(p) = &prev {
            diff_add(p, &mut list, &mut add);
        }
        let ctx = COUNTER_TO_CONTEXT[ctx_counter.min(18)];
        for m in list.iter_mut() {
            m.row = self.clamp_mv_row(b, m.row as i32, 128) as i16;
            m.col = self.clamp_mv_col(b, m.col as i32, 128) as i16;
        }
        (list, ctx)
    }

    fn use_mv_hp(m: Mv) -> bool {
        ((m.row as i32).abs() >> 3) < 8 && ((m.col as i32).abs() >> 3) < 8
    }

    /// find_best_ref_mvs (6.5.12).
    fn find_best_ref_mvs(&self, b: &Blk, mut list: [Mv; 2]) -> [Mv; 2] {
        for m in list.iter_mut() {
            let mut dr = m.row as i32;
            let mut dc = m.col as i32;
            if !self.f.h.allow_hp || !Self::use_mv_hp(*m) {
                if dr & 1 != 0 {
                    dr += if dr > 0 { -1 } else { 1 };
                }
                if dc & 1 != 0 {
                    dc += if dc > 0 { -1 } else { 1 };
                }
            }
            m.row = self.clamp_mv_row(b, dr, (160 - 4) << 3) as i16;
            m.col = self.clamp_mv_col(b, dc, (160 - 4) << 3) as i16;
        }
        list
    }

    /// append_sub8x8_mvs (6.5.14): NearestMv and NearMv of a sub-block.
    fn append_sub8x8_mvs(&self, b: &Blk, block: usize, rl: usize) -> (Mv, Mv) {
        let (list, _) = self.find_mv_refs(b, b.ref_frame[rl], block as i32);
        let mut out = [Mv::default(); 2];
        let mut dst;
        let bm = &b.mv[rl];
        if block == 0 {
            out = list;
            dst = 2;
        } else if block <= 2 {
            out[0] = bm[0];
            dst = 1;
        } else {
            out[0] = bm[2];
            dst = 1;
            for idx in [1usize, 0] {
                if dst >= 2 {
                    break;
                }
                if bm[idx] != out[0] {
                    out[dst] = bm[idx];
                    dst += 1;
                }
            }
        }
        for n in 0..2 {
            if dst >= 2 {
                break;
            }
            if list[n] != out[0] {
                out[dst] = list[n];
                dst += 1;
            }
        }
        if dst < 2 {
            out[dst] = Mv::default();
        }
        (out[0], out[1])
    }

    fn assign_mv(
        &mut self,
        b: &Blk,
        is_compound: bool,
        nearest: &[Mv; 2],
        near: &[Mv; 2],
        best: &[Mv; 2],
    ) -> [Mv; 2] {
        let mut mv = [Mv::default(); 2];
        for i in 0..1 + is_compound as usize {
            mv[i] = match b.y_mode {
                NEWMV => self.read_mv(best[i]),
                NEARESTMV => nearest[i],
                NEARMV => near[i],
                _ => Mv::default(),
            };
        }
        mv
    }

    fn read_mv(&mut self, best: Mv) -> Mv {
        let use_hp = self.f.h.allow_hp && Self::use_mv_hp(best);
        let fc = self.f.fc;
        let j = self.bd.tree(&MV_JOINT_TREE, |n| fc.mv_joint[n]);
        if self.f.count {
            self.counts.mv_joint[j] += 1;
        }
        let mut diff = [0i32; 2];
        if j == 2 || j == 3 {
            diff[0] = self.read_mv_component(0, use_hp);
        }
        if j == 1 || j == 3 {
            diff[1] = self.read_mv_component(1, use_hp);
        }
        Mv {
            row: best.row.wrapping_add(diff[0] as i16),
            col: best.col.wrapping_add(diff[1] as i16),
        }
    }

    #[allow(clippy::needless_late_init)] // Both branches count what they read.
    fn read_mv_component(&mut self, comp: usize, use_hp: bool) -> i32 {
        let fc = self.f.fc;
        let count = self.f.count;
        let sign = self.bd.read(fc.mv_sign[comp]);
        let class = self.bd.tree(&MV_CLASS_TREE, |n| fc.mv_class[comp][n]);
        let mag;
        if class == 0 {
            let c0 = self.bd.read(fc.mv_class0_bit[comp]) as usize;
            let fr = self.bd.tree(&MV_FR_TREE, |n| fc.mv_class0_fr[comp][c0][n]);
            let hp = if use_hp {
                self.bd.read(fc.mv_class0_hp[comp]) as usize
            } else {
                1
            };
            if count {
                let c = &mut self.counts;
                c.mv_class0_bit[comp][c0] += 1;
                c.mv_class0_fr[comp][c0][fr] += 1;
                c.mv_class0_hp[comp][hp] += 1;
            }
            mag = ((c0 << 3) | (fr << 1) | hp) as i32 + 1;
        } else {
            let mut d = 0usize;
            for i in 0..class {
                let bit = self.bd.read(fc.mv_bits[comp][i]) as usize;
                if count {
                    self.counts.mv_bits[comp][i][bit] += 1;
                }
                d |= bit << i;
            }
            let fr = self.bd.tree(&MV_FR_TREE, |n| fc.mv_fr[comp][n]);
            let hp = if use_hp {
                self.bd.read(fc.mv_hp[comp]) as usize
            } else {
                1
            };
            if count {
                self.counts.mv_fr[comp][fr] += 1;
                self.counts.mv_hp[comp][hp] += 1;
            }
            mag = (2 << (class + 2)) + ((d << 3) | (fr << 1) | hp) as i32 + 1;
        }
        if count {
            self.counts.mv_sign[comp][sign as usize] += 1;
            self.counts.mv_class[comp][class] += 1;
        }
        if sign { -mag } else { mag }
    }

    /// residual (6.4.21) with prediction and reconstruction; returns EobTotal.
    fn residual(&mut self, b: &Blk) -> Result<u32> {
        let f = self.f;
        let bsize = b.size.max(BLOCK_8X8);
        let mut eob_total = 0;
        let uv_tx = uv_tx_size(b.size, b.tx_size, f.ss_x, f.ss_y);
        for plane in 0..3 {
            let (sx, sy) = if plane == 0 { (0, 0) } else { (f.ss_x, f.ss_y) };
            let tx_sz = if plane > 0 { uv_tx } else { b.tx_size };
            let step = 1usize << tx_sz;
            let psz = SS_SIZE_LOOKUP[bsize][sx][sy] as usize;
            if psz > 12 {
                return Err(malformed(
                    "block size not allowed with this chroma subsampling",
                ));
            }
            let n4w = NUM_4X4_BLOCKS_WIDE_LOOKUP[psz] as usize;
            let n4h = NUM_4X4_BLOCKS_HIGH_LOOKUP[psz] as usize;
            let base_x = (b.c * 8) >> sx;
            let base_y = (b.r * 8) >> sy;
            if b.is_inter {
                if b.size < BLOCK_8X8 {
                    for y in 0..n4h {
                        for x in 0..n4w {
                            self.predict_inter(
                                b,
                                plane,
                                base_x + 4 * x,
                                base_y + 4 * y,
                                4,
                                4,
                                y * n4w + x,
                            )?;
                        }
                    }
                } else {
                    self.predict_inter(b, plane, base_x, base_y, n4w * 4, n4h * 4, 0)?;
                }
            }
            let max_x = (self.mi_cols * 8) >> sx;
            let max_y = (self.mi_rows * 8) >> sy;
            let sb_y4 = (((b.r & !7) * 8) >> sy) >> 2;
            let x04 = ((self.col_start * 8) >> sx) >> 2;
            let mut blk_idx = 0;
            let mut y = 0;
            while y < n4h {
                let mut x = 0;
                while x < n4w {
                    let start_x = base_x + 4 * x;
                    let start_y = base_y + 4 * y;
                    let mut nonzero = false;
                    if start_x < max_x && start_y < max_y {
                        let mode = if plane > 0 {
                            b.uv_mode
                        } else if b.size >= BLOCK_8X8 {
                            b.y_mode
                        } else {
                            b.sub_modes[blk_idx]
                        };
                        if !b.is_inter {
                            let args = IntraArgs {
                                x: start_x,
                                y: start_y,
                                x0: self.d.planes[plane].x0,
                                have_left: b.avail_l || x > 0,
                                have_above: b.avail_u || y > 0,
                                not_on_right: x + step < n4w,
                                tx_sz,
                                mode,
                                max_x: max_x - 1,
                                max_y: max_y - 1,
                                bit_depth: f.h.color.bit_depth,
                            };
                            intra::predict(&mut self.d.planes[plane].rows, &args);
                        }
                        if !b.skip {
                            let tx_type = if plane > 0 || tx_sz == 3 {
                                DCT_DCT
                            } else if tx_sz == 0 {
                                if f.h.lossless || b.is_inter {
                                    DCT_DCT
                                } else {
                                    MODE2TXFM_MAP[mode as usize]
                                }
                            } else {
                                MODE2TXFM_MAP[b.y_mode as usize]
                            };
                            let (eob, rows) = self.tokens(
                                b, plane, start_x, start_y, tx_sz, tx_type, max_x, max_y, sb_y4,
                            );
                            if eob > 0 {
                                nonzero = true;
                                eob_total += 1;
                                let pl = &mut self.d.planes[plane];
                                let mut dst = Dst {
                                    rows: &mut pl.rows,
                                    x: start_x - pl.x0,
                                    y: start_y,
                                };
                                itx::reconstruct(
                                    &mut self.coef,
                                    tx_sz,
                                    tx_type,
                                    f.h.lossless,
                                    eob,
                                    rows,
                                    &mut dst,
                                    f.h.color.bit_depth,
                                );
                            }
                        }
                    }
                    let ax = (start_x >> 2) - x04;
                    let an = &mut self.d.above_nz[plane];
                    let end = (ax + step).min(an.len());
                    an[ax.min(end)..end].fill(nonzero as u8);
                    let ly = (start_y >> 2) - sb_y4;
                    let lend = (ly + step).min(16);
                    self.left_nz[plane][ly.min(lend)..lend].fill(nonzero as u8);
                    blk_idx += 1;
                    x += step;
                }
                y += step;
            }
        }
        Ok(eob_total)
    }

    /// Reads the tokens of one transform block into `self.coef`, dequantised. Returns the
    /// end of block and one more than the last row with a coefficient.
    #[allow(clippy::too_many_arguments)]
    fn tokens(
        &mut self,
        b: &Blk,
        plane: usize,
        start_x: usize,
        start_y: usize,
        tx_sz: usize,
        tx_type: u8,
        max_x: usize,
        max_y: usize,
        sb_y4: usize,
    ) -> (usize, usize) {
        let f = self.f;
        let ss = scan_set(tx_sz, tx_type);
        let ptype = (plane > 0) as usize;
        let probs = &f.fc.coef[tx_sz][ptype][b.is_inter as usize];
        let seg_eob = 16usize << (tx_sz << 1);
        let n = 1usize << tx_sz;
        // Initial context from the neighbouring transform blocks.
        let x4 = start_x >> 2;
        let y4 = start_y >> 2;
        let x04 = ((self.col_start * 8) >> if plane > 0 { f.ss_x } else { 0 }) >> 2;
        let max_x4 = max_x >> 2;
        let max_y4 = max_y >> 2;
        let mut above = 0u8;
        let mut left = 0u8;
        for i in 0..n {
            if x4 + i < max_x4 {
                above |= self.d.above_nz[plane][x4 + i - x04];
            }
            if y4 + i < max_y4 {
                left |= self.left_nz[plane][y4 + i - sb_y4];
            }
        }
        let mut ctx = (above + left) as usize;
        let q = &f.q[b.seg_id];
        let (dcq, acq) = if plane == 0 {
            (q[0], q[1])
        } else {
            (q[2], q[3])
        };
        let dq_shift = (tx_sz == 3) as u32;
        let bands: &[u8] = if tx_sz == 0 {
            &COEFBAND_4X4
        } else {
            &COEFBAND_8X8PLUS
        };
        let bd = f.h.color.bit_depth as u32;
        let count = f.count;
        let log2n = tx_sz + 2;
        let mut max_row = 0usize;
        let mut c = 0usize;
        let tc = &mut self.token_cache;
        let coef = &mut self.coef;
        let bdec = &mut self.bd;
        let counts = &mut self.counts;
        'outer: while c < seg_eob {
            let band = bands[c] as usize;
            if c > 0 {
                let nb = ss.nb[c];
                ctx = (1 + tc[nb[0] as usize] as usize + tc[nb[1] as usize] as usize) >> 1;
            }
            let p = &probs[band][ctx];
            let more = bdec.read(p[0]);
            if count {
                counts.more_coefs[tx_sz][ptype][b.is_inter as usize][band][ctx][more as usize] += 1;
            }
            if !more {
                break;
            }
            let mut band = band;
            let mut p = p;
            loop {
                let pos = ss.scan[c] as usize;
                if !bdec.read(p[1]) {
                    if count {
                        counts.token[tx_sz][ptype][b.is_inter as usize][band][ctx][0] += 1;
                    }
                    tc[pos] = 0;
                    c += 1;
                    if c >= seg_eob {
                        break 'outer;
                    }
                    band = bands[c] as usize;
                    let nb = ss.nb[c];
                    ctx = (1 + tc[nb[0] as usize] as usize + tc[nb[1] as usize] as usize) >> 1;
                    p = &probs[band][ctx];
                    continue;
                }
                let token;
                let mut val: i32;
                if !bdec.read(p[2]) {
                    token = 1;
                    val = 1;
                    if count {
                        counts.token[tx_sz][ptype][b.is_inter as usize][band][ctx][1] += 1;
                    }
                } else {
                    if count {
                        counts.token[tx_sz][ptype][b.is_inter as usize][band][ctx][2] += 1;
                    }
                    let pp = &PARETO_FULL[p[2] as usize];
                    // The rest of token_tree from node 2 (TWO_TOKEN and up).
                    token = if !bdec.read(pp[0]) {
                        if !bdec.read(pp[1]) {
                            2
                        } else if !bdec.read(pp[2]) {
                            3
                        } else {
                            4
                        }
                    } else if !bdec.read(pp[3]) {
                        if !bdec.read(pp[4]) { 5 } else { 6 }
                    } else if !bdec.read(pp[5]) {
                        if !bdec.read(pp[6]) { 7 } else { 8 }
                    } else if !bdec.read(pp[7]) {
                        9
                    } else {
                        10
                    };
                    let eb = &EXTRA_BITS[token];
                    let cat = eb[0] as usize;
                    let num_extra = eb[1] as u32;
                    val = eb[2] as i32;
                    if token == 10 {
                        for e in 0..bd - 8 {
                            if bdec.read(255) {
                                val += 1 << (5 + bd - e);
                            }
                        }
                    }
                    for e in 0..num_extra {
                        if bdec.read(CAT_PROBS[cat][e as usize]) {
                            val += 1 << (num_extra - 1 - e);
                        }
                    }
                }
                tc[pos] = ENERGY_CLASS[token];
                let neg = bdec.read(128);
                let qv = if c == 0 { dcq } else { acq };
                let v = ((val as i64 * qv as i64) >> dq_shift) as i32;
                coef[pos] = if neg { v.wrapping_neg() } else { v };
                max_row = max_row.max(pos >> log2n);
                c += 1;
                break;
            }
        }
        (c, max_row + 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn predict_inter(
        &mut self,
        b: &Blk,
        plane: usize,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        blk_idx: usize,
    ) -> Result<()> {
        let f = self.f;
        let (sx, sy) = if plane == 0 { (0, 0) } else { (f.ss_x, f.ss_y) };
        let is_compound = b.ref_frame[1] > 0;
        for rl in 0..1 + is_compound as usize {
            // Motion vector selection (8.5.2.1).
            let m = &b.mv[rl];
            let mv = if plane == 0 || b.size >= BLOCK_8X8 || (sx == 0 && sy == 0) {
                m[blk_idx]
            } else {
                let q2 = |v: i32| (if v < 0 { v - 1 } else { v + 1 }) / 2;
                let q4 = |v: i32| (if v < 0 { v - 2 } else { v + 2 }) / 4;
                if sx == 0 {
                    let (a, c) = (m[blk_idx], m[blk_idx + 2]);
                    Mv {
                        row: q2(a.row as i32 + c.row as i32) as i16,
                        col: q2(a.col as i32 + c.col as i32) as i16,
                    }
                } else if sy == 0 {
                    let (a, c) = (m[blk_idx], m[blk_idx + 1]);
                    Mv {
                        row: q2(a.row as i32 + c.row as i32) as i16,
                        col: q2(a.col as i32 + c.col as i32) as i16,
                    }
                } else {
                    let r: i32 = m.iter().map(|v| v.row as i32).sum();
                    let c: i32 = m.iter().map(|v| v.col as i32).sum();
                    Mv {
                        row: q4(r) as i16,
                        col: q4(c) as i16,
                    }
                }
            };
            // Clamping (8.5.2.2).
            let bh = NUM_8X8_BLOCKS_HIGH_LOOKUP[b.size] as i32;
            let bw = NUM_8X8_BLOCKS_WIDE_LOOKUP[b.size] as i32;
            let to_top = -((b.r as i32 * 8) * 16) >> sy;
            let to_bottom = (((self.mi_rows as i32 - bh - b.r as i32) * 8) * 16) >> sy;
            let to_left = -((b.c as i32 * 8) * 16) >> sx;
            let to_right = (((self.mi_cols as i32 - bw - b.c as i32) * 8) * 16) >> sx;
            let spel_left = (4 + ((bw * 8) >> sx)) << 4;
            let spel_right = spel_left - 16;
            let spel_top = (4 + ((bh * 8) >> sy)) << 4;
            let spel_bottom = spel_top - 16;
            let cr = ((2 * mv.row as i32) >> sy).clamp(
                to_top - spel_top,
                (to_bottom + spel_bottom).max(to_top - spel_top),
            );
            let cc = ((2 * mv.col as i32) >> sx).clamp(
                to_left - spel_left,
                (to_right + spel_right).max(to_left - spel_left),
            );
            // Scaling (8.5.2.3).
            let ri = (b.ref_frame[rl] - 1) as usize;
            let Some(rf) = f.refs.get(ri).copied().flatten() else {
                return Err(malformed("inter block with a missing reference frame"));
            };
            let sc = f.scale[ri];
            if !sc.valid {
                return Err(malformed("reference frame of an invalid size"));
            }
            let (start_x, start_y) = if sc.x == 1 << 14 && sc.y == 1 << 14 {
                ((x as i64) * 16 + cc as i64, (y as i64) * 16 + cr as i64)
            } else {
                let base_x = (x as i64 * sc.x) >> 14;
                let base_y = (y as i64 * sc.y) >> 14;
                let luma_x = (x as i64) << sx;
                let luma_y = (y as i64) << sy;
                let frac_x = ((16 * luma_x * sc.x) >> 14) & 15;
                let frac_y = ((16 * luma_y * sc.y) >> 14) & 15;
                let dx = ((cc as i64 * sc.x) >> 14) + frac_x;
                let dy = ((cr as i64 * sc.y) >> 14) + frac_y;
                ((base_x << 4) + dx, (base_y << 4) + dy)
            };
            inter::predict(
                &rf.planes[plane],
                start_x,
                start_y,
                sc.step_x,
                sc.step_y,
                w,
                h,
                b.interp_filter as usize,
                f.h.color.bit_depth,
                &mut self.pred[rl],
                &mut self.mc,
            );
        }
        let pl = &mut self.d.planes[plane];
        let x0 = pl.x0;
        let (a, rest) = self.pred.split_at(1);
        inter::store(
            &mut pl.rows,
            x - x0,
            y,
            w,
            h,
            &a[0],
            is_compound.then_some(&rest[0][..]),
        );
        Ok(())
    }
}
