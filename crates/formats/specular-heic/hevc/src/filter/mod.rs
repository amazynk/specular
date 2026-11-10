//! In-loop filters: deblocking (clause 8.7.2) and SAO (clause 8.7.3).

mod deblock;
mod sao;

pub use deblock::deblock;

use crate::picture::{Picture, Plane, PlaneMut};
use alloc::vec::Vec;
use std::sync::{Mutex, PoisonError};

/// The per-block side information the filters read, borrowed from a whole
/// [`Picture`] apart from its sample planes.
#[derive(Debug)]
pub struct Side<'a> {
    /// [`Picture::qp_y`].
    pub qp_y: &'a [i8],
    /// [`Picture::cu_flags`].
    pub cu_flags: &'a [u8],
    /// [`Picture::edges`].
    pub edges: &'a [u8],
    /// [`Picture::bs`].
    pub bs: &'a [u8],
    /// [`Picture::min4_w`].
    pub min4_w: usize,
    /// [`Picture::min4_h`].
    pub min4_h: usize,
    /// [`Picture::min8_w`].
    pub min8_w: usize,
    /// [`Picture::min8_h`].
    pub min8_h: usize,
}

impl Side<'_> {
    /// [`Picture::idx4`] of a whole picture.
    #[inline]
    pub fn idx4(&self, x: usize, y: usize) -> usize {
        let xi = (x >> 2).min(self.min4_w - 1);
        let yi = (y >> 2).min(self.min4_h - 1);
        (yi * self.min4_w + xi).min(self.qp_y.len() - 1)
    }
}

/// Splits a whole picture into its planes and the side information.
fn parts(pic: &mut Picture) -> (&mut Plane, &mut Plane, &mut Plane, Side<'_>) {
    debug_assert!(
        pic.oy4 == 0 && pic.oy8 == 0 && pic.ox4 == 0 && pic.y.ox == 0,
        "filters run on whole pictures"
    );
    let Picture {
        y,
        cb,
        cr,
        min4_w,
        min4_h,
        qp_y,
        cu_flags,
        min8_w,
        min8_h,
        edges,
        bs,
        ..
    } = pic;
    let side = Side {
        qp_y,
        cu_flags,
        edges,
        bs,
        min4_w: *min4_w,
        min4_h: *min4_h,
        min8_w: *min8_w,
        min8_h: *min8_h,
    };
    (y, cb, cr, side)
}

/// Runs `f` on every job, on up to `threads` scoped threads. The jobs
/// touch disjoint data, so the order they run in does not matter.
fn run_jobs<J: Send>(jobs: Vec<J>, threads: usize, f: impl Fn(J) + Sync) {
    let n = threads.min(jobs.len());
    if n <= 1 {
        jobs.into_iter().for_each(f);
        return;
    }
    let queue = Mutex::new(jobs.into_iter());
    std::thread::scope(|s| {
        for wi in 0..n {
            specular_thread::spawn_scoped(s, format!("dec-hevc-lf-{wi}"), || {
                loop {
                    let job = queue.lock().unwrap_or_else(PoisonError::into_inner).next();
                    let Some(job) = job else { break };
                    f(job);
                }
            });
        }
    });
}

/// Deblocks then applies SAO to `pic`, on up to `threads` threads. The
/// output does not depend on `threads`.
///
/// Deblocking runs on row bands cut at four rows above each CTB row
/// (in each plane's own rows, rounded to a row 8k + 4): every edge's taps
/// then stay inside one band, and since vertical edges never reach across rows, each band can
/// filter its vertical then its horizontal edges on its own. SAO runs on
/// CTB rows, reading a snapshot of the deblocked picture.
pub fn loop_filters(pic: &mut Picture, info: &FilterInfo, threads: usize) {
    if threads <= 1 || info.pic_h_ctbs < 2 {
        deblock(pic, info);
        sao::run(pic, info, 1);
        return;
    }
    enum Job<'a> {
        Luma(PlaneMut<'a>),
        Chroma(PlaneMut<'a>, PlaneMut<'a>),
    }
    let ctb = 1usize << info.ctb_log2;
    let rows = info.pic_h_ctbs;
    {
        let (y, cb, cr, side) = parts(pic);
        // A cut at 8k + 4 keeps every edge's taps (at most four rows each
        // side of a row 8k) inside one band. With a legal CTB (16 to 64)
        // and 4:2:0 this is four rows above each CTB row; the rounding
        // keeps it so for the 8x8 CTBs a damaged SPS can declare.
        let cut = |row: usize| (row & !7).checked_sub(4);
        let cuts: Vec<usize> = (1..rows).filter_map(|r| cut(r * ctb)).collect();
        let mut jobs: Vec<Job<'_>> = y.split_rows(&cuts).into_iter().map(Job::Luma).collect();
        if info.chroma_array_type != 0 && info.sub_h > 0 {
            let cuts: Vec<usize> = (1..rows)
                .filter_map(|r| cut(r * ctb / info.sub_h))
                .collect();
            let (b, r) = (cb.split_rows(&cuts), cr.split_rows(&cuts));
            jobs.extend(b.into_iter().zip(r).map(|(b, r)| Job::Chroma(b, r)));
        }
        run_jobs(jobs, threads, |job| match job {
            Job::Luma(mut y) => deblock::deblock_rows(Some(&mut y), None, &side, info),
            Job::Chroma(mut b, mut r) => {
                deblock::deblock_rows(None, Some((&mut b, &mut r)), &side, info);
            }
        });
    }
    sao::run(pic, info, threads);
}

/// The loop filters of a picture run CTB row by CTB row while the picture
/// is still being decoded, giving the samples [`loop_filters`] gives.
///
/// Deblocking runs on the bands [`loop_filters`] cuts, band `k` once CTB
/// rows up to `k` are final: the band ends four rows above CTB row `k + 1`
/// (in each plane's own rows, rounded to a row 8k + 4), so it reads and
/// writes nothing of that row, and leaves the last rows of row `k`, which
/// intra prediction and concealment in row `k + 1` read, as they were. SAO
/// of a row follows once the bands down to the first line of the row below
/// are deblocked; it writes into the copy of the row handed out, leaving
/// the picture deblocked (SAO of the row below reads it), so the picture
/// itself never holds SAO output.
#[derive(Debug)]
pub struct RowFilter {
    /// Bands deblocked.
    bands: usize,
    /// Rows through SAO.
    sao_rows: usize,
}

impl RowFilter {
    /// A picture with nothing filtered yet.
    pub fn new() -> RowFilter {
        RowFilter {
            bands: 0,
            sao_rows: 0,
        }
    }

    /// The first row of deblocking band `k` of a plane `height` rows high
    /// whose CTB rows are `ctb` of its rows high.
    fn cut(k: usize, rows: usize, ctb: usize, height: usize) -> usize {
        if k == 0 {
            0
        } else if k >= rows {
            height
        } else {
            ((k * ctb) & !7).saturating_sub(4).min(height)
        }
    }

    /// Filters what CTB rows `0..final_rows` (decoded, concealed, with
    /// their boundary strengths) allow, handing each row filtered for good
    /// to `emit` in order (with the picture, the row and its lines per
    /// component, each as wide as the plane); returns
    /// how many rows are.
    pub fn advance(
        &mut self,
        pic: &mut Picture,
        info: &FilterInfo,
        final_rows: usize,
        mut emit: impl FnMut(&Picture, usize, [Vec<u16>; 3]),
    ) -> usize {
        let rows = info.pic_h_ctbs;
        let ctb = 1usize << info.ctb_log2;
        let chroma = info.chroma_array_type != 0 && info.sub_h > 0;
        let csh = info.sub_h.max(1);
        let (hy, hc) = (pic.y.height, pic.cb.height);
        while self.bands < final_rows.min(rows) {
            let k = self.bands;
            let (y, cb, cr, side) = parts(pic);
            let (a, b) = (Self::cut(k, rows, ctb, hy), Self::cut(k + 1, rows, ctb, hy));
            let mut yv = y.rows_mut(a, b);
            if chroma {
                let (a, b) = (
                    Self::cut(k, rows, ctb / csh, hc),
                    Self::cut(k + 1, rows, ctb / csh, hc),
                );
                let (mut bv, mut rv) = (cb.rows_mut(a, b), cr.rows_mut(a, b));
                deblock::deblock_rows(Some(&mut yv), Some((&mut bv, &mut rv)), &side, info);
            } else {
                deblock::deblock_rows(Some(&mut yv), None, &side, info);
            }
            self.bands += 1;
        }
        // Deblocked so far: every row above these.
        let done_y = Self::cut(self.bands, rows, ctb, hy);
        let done_c = Self::cut(self.bands, rows, ctb / csh, hc);
        while self.sao_rows < rows {
            let q = self.sao_rows;
            let need_y = (((q + 1) * ctb) + 1).min(hy);
            let need_c = (((q + 1) * ctb / csh) + 1).min(hc);
            if done_y < need_y || (chroma && done_c < need_c) {
                break;
            }
            let lines = sao::row_out(pic, info, q);
            emit(pic, q, lines);
            self.sao_rows += 1;
        }
        self.sao_rows
    }
}

impl Default for RowFilter {
    fn default() -> Self {
        RowFilter::new()
    }
}

/// Sample adaptive offset parameters for one component of one CTB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaoCtb {
    /// `SaoTypeIdx`: 0 = not applied, 1 = band offset, 2 = edge offset.
    pub type_idx: u8,
    /// `SaoEoClass`, 0..=3; meaningful when `type_idx == 2`.
    pub eo_class: u8,
    /// `sao_band_position`; meaningful when `type_idx == 1`.
    pub band_position: u8,
    /// `SaoOffsetVal[1..=4]`, already signed and scaled.
    pub offsets: [i16; 4],
}

/// Loop-filter parameters that vary per coding tree block.
#[derive(Clone, Copy, Debug)]
pub struct CtbFilter {
    /// `slice_deblocking_filter_disabled_flag` for the covering slice.
    pub deblock_disabled: bool,
    /// `slice_beta_offset_div2`.
    pub beta_offset_div2: i8,
    /// `slice_tc_offset_div2`.
    pub tc_offset_div2: i8,
    /// `slice_loop_filter_across_slices_enabled_flag`.
    pub across_slices: bool,
    /// `pps_cb_qp_offset` (`cQpPicOffset` for Cb, clause 8.7.2.5.5).
    pub cb_qp_offset: i16,
    /// `pps_cr_qp_offset` (`cQpPicOffset` for Cr).
    pub cr_qp_offset: i16,
    /// SAO parameters for Y, Cb and Cr.
    pub sao: [SaoCtb; 3],
}

impl Default for CtbFilter {
    fn default() -> Self {
        CtbFilter {
            deblock_disabled: true,
            beta_offset_div2: 0,
            tc_offset_div2: 0,
            across_slices: true,
            cb_qp_offset: 0,
            cr_qp_offset: 0,
            sao: [SaoCtb::default(); 3],
        }
    }
}

/// Everything the in-loop filters need that is not already in the picture.
#[derive(Debug, Clone)]
pub struct FilterInfo {
    /// `BitDepthY`.
    pub bit_depth_y: u8,
    /// `BitDepthC`.
    pub bit_depth_c: u8,
    /// `ChromaArrayType`, 0..=3.
    pub chroma_array_type: u8,
    /// `SubWidthC`, or 0 for monochrome.
    pub sub_w: usize,
    /// `SubHeightC`, or 0 for monochrome.
    pub sub_h: usize,
    /// `CtbLog2SizeY`.
    pub ctb_log2: usize,
    /// `PicWidthInCtbsY`.
    pub pic_w_ctbs: usize,
    /// `PicHeightInCtbsY`.
    pub pic_h_ctbs: usize,
    /// `pcm_loop_filter_disabled_flag`.
    pub pcm_loop_filter_disabled: bool,
    /// `loop_filter_across_tiles_enabled_flag`.
    pub across_tiles: bool,
    /// True when SAO is enabled for at least one component of the picture.
    pub sao_enabled: bool,
    /// Per-CTB parameters, indexed by CTB raster address.
    pub ctb: Vec<CtbFilter>,
    /// `TileId` per CTB raster address.
    pub tile_id: Vec<u16>,
    /// Address of the first CTB of the containing slice, per CTB raster address.
    pub slice_addr: Vec<u32>,
}

impl FilterInfo {
    /// True when a filter may cross from CTB `a` into CTB `b` (raster addresses).
    ///
    /// `b` is the CTB containing the samples being modified.
    #[inline]
    pub fn may_cross(&self, a: usize, b: usize) -> bool {
        if a == b {
            return true;
        }
        if self.tile_id[a] != self.tile_id[b] && !self.across_tiles {
            return false;
        }
        if self.slice_addr[a] != self.slice_addr[b] && !self.ctb[b].across_slices {
            return false;
        }
        true
    }

    /// CTB raster address covering luma position `(x, y)`.
    #[inline]
    pub fn ctb_of(&self, x: usize, y: usize) -> usize {
        (y >> self.ctb_log2) * self.pic_w_ctbs + (x >> self.ctb_log2)
    }
}

/// Maps a luma QP to a chroma QP, Table 8-10, for `ChromaArrayType == 1`.
#[inline]
pub fn chroma_qp_from_luma(qpi: i32, chroma_array_type: u8) -> i32 {
    if chroma_array_type != 1 {
        return qpi.min(51);
    }
    if qpi < 30 {
        qpi
    } else if qpi > 43 {
        qpi - 6
    } else {
        const T: [i32; 14] = [29, 30, 31, 32, 33, 33, 34, 34, 35, 35, 36, 36, 37, 37];
        T[(qpi - 30) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::edge;

    /// A small xorshift generator: the tests need noise, not quality.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// A noisy picture with every 8x8 edge marked, random QPs and SAO.
    fn mk(w: usize, h: usize, cat: u8, ctb_log2: usize, seed: u64) -> (Picture, FilterInfo) {
        let (sw, sh) = match cat {
            0 => (0, 0),
            1 => (2, 2),
            2 => (2, 1),
            _ => (1, 1),
        };
        let mut rng = Rng(seed);
        let mut pic = Picture::new(w, h, sw, sh).expect("picture");
        for p in [&mut pic.y, &mut pic.cb, &mut pic.cr] {
            for y in 0..p.height {
                for x in 0..p.width {
                    // Blocky noise, so that edges are worth filtering.
                    let base = ((x / 8 + y / 8) * 37 % 200) as u64;
                    p.put(x, y, (base + rng.below(24)) as u16);
                }
            }
        }
        for v in pic.qp_y.iter_mut() {
            *v = 20 + rng.below(25) as i8;
        }
        for y in (0..h).step_by(8) {
            for x in (0..w).step_by(8) {
                pic.mark_edge(x, y, 8, edge::VER | edge::HOR);
            }
        }
        let ctb = 1usize << ctb_log2;
        let (wc, hc) = (w.div_ceil(ctb), h.div_ceil(ctb));
        let ctbs = (0..wc * hc)
            .map(|_| {
                let mut f = CtbFilter {
                    deblock_disabled: rng.below(8) == 0,
                    beta_offset_div2: rng.below(5) as i8 - 2,
                    tc_offset_div2: rng.below(5) as i8 - 2,
                    ..CtbFilter::default()
                };
                for s in f.sao.iter_mut() {
                    s.type_idx = rng.below(3) as u8;
                    s.eo_class = rng.below(4) as u8;
                    s.band_position = rng.below(28) as u8;
                    for o in s.offsets.iter_mut() {
                        *o = rng.below(15) as i16 - 7;
                    }
                }
                f
            })
            .collect();
        let info = FilterInfo {
            bit_depth_y: 8,
            bit_depth_c: 8,
            chroma_array_type: cat,
            sub_w: sw,
            sub_h: sh,
            ctb_log2,
            pic_w_ctbs: wc,
            pic_h_ctbs: hc,
            pcm_loop_filter_disabled: false,
            across_tiles: true,
            sao_enabled: true,
            ctb: ctbs,
            tile_id: vec![0; wc * hc],
            slice_addr: vec![0; wc * hc],
        };
        (pic, info)
    }

    /// Threaded loop filtering equals serial filtering for every CTB size
    /// (including the 8x8 CTBs only a damaged SPS declares) and chroma format.
    /// The row-wise SAO paths give the samples the per-sample one gives, at
    /// both bit depths, across tile and slice boundaries that may or may not
    /// be crossed, and around bypassed and PCM coding units.
    #[test]
    fn sao_fast_paths_match_the_checked_path() {
        for cat in 0..=3 {
            for ctb_log2 in 3..=6 {
                for (w, h) in [(64, 48), (72, 88), (40, 136), (13, 9)] {
                    for variant in 0..4u64 {
                        let seed =
                            0x51ed_270b ^ ((cat as u64) << 8) ^ ((ctb_log2 as u64) << 4) ^ variant;
                        let (mut a, mut info) = mk(w, h, cat, ctb_log2, seed);
                        let (mut b, _) = mk(w, h, cat, ctb_log2, seed);
                        let mut rng = Rng(seed ^ 0xdead_beef);
                        if variant & 1 != 0 {
                            info.bit_depth_y = 10;
                            info.bit_depth_c = 10;
                            for p in [
                                &mut a.y, &mut a.cb, &mut a.cr, &mut b.y, &mut b.cb, &mut b.cr,
                            ] {
                                for v in p.data.iter_mut() {
                                    *v *= 4;
                                }
                            }
                        }
                        if variant & 2 != 0 {
                            info.across_tiles = rng.below(2) == 0;
                            info.pcm_loop_filter_disabled = rng.below(2) == 0;
                            let n = info.ctb.len();
                            for i in 0..n {
                                info.tile_id[i] =
                                    (i % info.pic_w_ctbs >= info.pic_w_ctbs / 2) as u16;
                                info.slice_addr[i] = if i >= n / 3 { (n / 3) as u32 } else { 0 };
                                info.ctb[i].across_slices = rng.below(2) == 0;
                            }
                            for i in 0..a.cu_flags.len() {
                                if rng.below(40) == 0 {
                                    let f = 1 + rng.below(3) as u8;
                                    a.cu_flags[i] = f;
                                    b.cu_flags[i] = f;
                                }
                            }
                        }
                        sao::run_impl(&mut a, &info, 1, false);
                        sao::run_impl(&mut b, &info, 1, true);
                        for (pa, pb) in [(&a.y, &b.y), (&a.cb, &b.cb), (&a.cr, &b.cr)] {
                            assert!(
                                pa.data == pb.data,
                                "cat {cat} ctb {ctb_log2} {w}x{h} variant {variant}: paths differ"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn threads_do_not_change_the_output() {
        for cat in 0..=3 {
            for ctb_log2 in 3..=6 {
                for (w, h) in [(64, 48), (72, 88), (40, 136)] {
                    let seed = 0x9e37_79b9 ^ ((cat as u64) << 8) ^ ctb_log2 as u64;
                    let (mut a, info) = mk(w, h, cat, ctb_log2, seed);
                    let (mut b, _) = mk(w, h, cat, ctb_log2, seed);
                    loop_filters(&mut a, &info, 1);
                    loop_filters(&mut b, &info, 4);
                    for (pa, pb) in [(&a.y, &b.y), (&a.cb, &b.cb), (&a.cr, &b.cr)] {
                        assert!(
                            pa.data == pb.data,
                            "cat {cat} ctb {ctb_log2} {w}x{h}: threads change the output"
                        );
                    }
                }
            }
        }
    }

    /// The row pipeline gives what filtering the whole picture gives, and
    /// reads nothing of the rows not yet final: those hold noise until
    /// they are handed over, one CTB row at a time.
    #[test]
    fn row_pipeline_matches_whole_picture_filtering() {
        for cat in 0..=3 {
            for ctb_log2 in 3..=6 {
                for (w, h) in [(64, 48), (72, 88), (40, 136), (13, 9), (24, 100)] {
                    for variant in 0..2u64 {
                        let seed =
                            0x7f4a_7c15 ^ ((cat as u64) << 8) ^ ((ctb_log2 as u64) << 4) ^ variant;
                        let (mut a, mut info) = mk(w, h, cat, ctb_log2, seed);
                        if variant == 1 {
                            let n = info.ctb.len();
                            let mut rng = Rng(seed ^ 0x1234_5678);
                            info.across_tiles = false;
                            for i in 0..n {
                                info.tile_id[i] =
                                    (i % info.pic_w_ctbs >= info.pic_w_ctbs / 2) as u16;
                                info.slice_addr[i] = if i >= n / 3 { (n / 3) as u32 } else { 0 };
                                info.ctb[i].across_slices = rng.below(2) == 0;
                            }
                        }
                        let (orig, _) = mk(w, h, cat, ctb_log2, seed);
                        let (mut b, _) = mk(w, h, cat, ctb_log2, seed);
                        let mut rng = Rng(seed ^ 0x0bad_cafe);
                        for p in [&mut b.y, &mut b.cb, &mut b.cr] {
                            for v in p.data.iter_mut() {
                                *v = rng.below(256) as u16;
                            }
                        }
                        loop_filters(&mut a, &info, 1);
                        let rows = info.pic_h_ctbs;
                        let ctb = 1usize << ctb_log2;
                        let csh = info.sub_h.max(1);
                        let mut f = RowFilter::new();
                        let mut done = 0;
                        let mut got: [Vec<u16>; 3] = Default::default();
                        let mut next = 0;
                        for k in 0..rows {
                            for (po, pb, lines) in [
                                (&orig.y, &mut b.y, ctb),
                                (&orig.cb, &mut b.cb, ctb / csh),
                                (&orig.cr, &mut b.cr, ctb / csh),
                            ] {
                                let y0 = (k * lines).min(po.height);
                                let y1 = ((k + 1) * lines).min(po.height);
                                pb.data[y0 * pb.stride..y1 * pb.stride]
                                    .copy_from_slice(&po.data[y0 * po.stride..y1 * po.stride]);
                            }
                            let r = f.advance(&mut b, &info, k + 1, |_, q, lines| {
                                assert_eq!(q, next);
                                next += 1;
                                for (g, l) in got.iter_mut().zip(lines) {
                                    g.extend_from_slice(&l);
                                }
                            });
                            assert!(r >= done && r <= k + 1);
                            done = r;
                        }
                        assert_eq!(done, rows);
                        for (pa, pb) in [&a.y, &a.cb, &a.cr].into_iter().zip(&got) {
                            let mut want = Vec::new();
                            for y in 0..pa.height {
                                let o = pa.offset(0, y);
                                want.extend_from_slice(&pa.data[o..o + pa.width]);
                            }
                            assert!(
                                want == *pb,
                                "cat {cat} ctb {ctb_log2} {w}x{h} variant {variant}: rows differ"
                            );
                        }
                    }
                }
            }
        }
    }
}
