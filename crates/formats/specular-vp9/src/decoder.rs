//! Frame-level decoding (sections 6.1 and 8): headers, probability contexts, tiles in
//! parallel, the loop filter, backward adaptation and the reference frame update.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::frame::{AnyFrame, FrameBuf, Pixel, PlaneCols, split_cols};
use crate::header::{
    self, FrameHeader, KEY_FRAME, Persist, SEG_LVL_ALT_Q, TX_MODE_SELECT, parse_compressed,
    parse_uncompressed,
};
use crate::lf::{self, LfFrame};
use crate::probs::{Counts, Probs, adapt_coef_probs, adapt_noncoef_probs};
use crate::tables::{AC_QLOOKUP, DC_QLOOKUP};
use crate::tile::{FrameCtx, MiInfo, Scale, Tile, TileData};
use crate::{Picture, Result, malformed};

pub(crate) struct State {
    pool: Option<rayon::ThreadPool>,
    threads: usize,
    refs: [Option<AnyFrame>; 8],
    ctxs: [Probs; 4],
    persist: Persist,
    prev_mi: Vec<MiInfo>,
    prev_seg: Vec<u8>,
    /// Frame size and show_frame the last time compute_image_size ran.
    last_size: Option<(u32, u32)>,
    last_show: bool,
    need_key: bool,
}

impl State {
    pub(crate) fn new(threads: usize) -> State {
        crate::tile::init_tables();
        let threads = if threads == 0 {
            std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .min(8)
        } else {
            threads
        };
        let pool = (threads > 1)
            .then(|| {
                specular_thread::rayon_pool("dec-vp9")
                    .num_threads(threads)
                    .build()
                    .ok()
            })
            .flatten();
        State {
            pool,
            threads,
            refs: Default::default(),
            ctxs: Default::default(),
            persist: Persist::default(),
            prev_mi: Vec::new(),
            prev_seg: Vec::new(),
            last_size: None,
            last_show: false,
            need_key: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.refs = Default::default();
        self.ctxs = Default::default();
        self.persist = Persist::default();
        self.prev_mi.clear();
        self.prev_seg.clear();
        self.last_size = None;
        self.last_show = false;
        self.need_key = false;
    }

    pub(crate) fn decode(&mut self, data: &[u8], out: &mut Vec<Picture>) -> Result<()> {
        for frame in header::split_superframe(data) {
            if self.need_key && !header::frame_is_key(frame) {
                return Err(malformed("waiting for a keyframe after an error"));
            }
            if let Err(e) = self.decode_frame(frame, out) {
                self.need_key = true;
                return Err(e);
            }
            self.need_key = false;
        }
        Ok(())
    }

    fn decode_frame(&mut self, data: &[u8], out: &mut Vec<Picture>) -> Result<()> {
        let ref_sizes: [Option<(u32, u32)>; 8] =
            std::array::from_fn(|i| self.refs[i].as_ref().map(|f| f.size()));
        let h = parse_uncompressed(data, &mut self.persist, &ref_sizes)?;
        if let Some(idx) = h.show_existing {
            let f = self.refs[idx]
                .as_ref()
                .ok_or_else(|| malformed("show_existing_frame of an empty slot"))?;
            out.push(f.picture());
            return Ok(());
        }
        if h.header_size == 0 {
            return Err(malformed("empty compressed header"));
        }
        let comp_end = h.uncomp_size + h.header_size;
        if comp_end > data.len() {
            return Err(malformed("frame shorter than its headers"));
        }
        if !h.intra {
            for &i in &h.ref_idx {
                let f = self.refs[i]
                    .as_ref()
                    .ok_or_else(|| malformed("inter frame refers to an empty slot"))?;
                let (bd, sx, sy) = f.format();
                if bd != h.color.bit_depth || sx != h.color.ss_x || sy != h.color.ss_y {
                    return Err(malformed("reference frame of another format"));
                }
            }
        }
        // Probability contexts (6.2 setup_past_independence, 6.1 load_probs).
        if h.past_independence {
            if h.reset_all {
                self.ctxs = Default::default();
            } else if let Some(i) = h.reset_one {
                self.ctxs[i] = Probs::default();
            }
        }
        let mut fc = self.ctxs[h.ctx_idx].clone();
        let comp = parse_compressed(&data[h.uncomp_size..comp_end], &h, &mut fc)?;
        // compute_image_size (7.2.6).
        let size = (h.width, h.height);
        let n_mi = h.mi_rows * h.mi_cols;
        let same_size = self.last_size == Some(size);
        if !same_size || h.past_independence || self.prev_seg.len() != n_mi {
            self.prev_seg.clear();
            self.prev_seg.resize(n_mi, 0);
        }
        let use_prev =
            same_size && self.last_show && !h.error_res && !h.intra && self.prev_mi.len() == n_mi;
        self.last_size = Some(size);
        self.last_show = h.show_frame;

        let mut cur_mi = vec![MiInfo::default(); n_mi];
        let tile_data = &data[comp_end..];
        let (frame, counts) = if h.color.bit_depth == 8 {
            let refs = self.ref_bufs8(&h);
            let (f, c) =
                self.decode_pixels::<u8>(&h, comp, &fc, refs, use_prev, tile_data, &mut cur_mi)?;
            (AnyFrame::B8(Arc::new(f)), c)
        } else {
            let refs = self.ref_bufs16(&h);
            let (f, c) =
                self.decode_pixels::<u16>(&h, comp, &fc, refs, use_prev, tile_data, &mut cur_mi)?;
            (AnyFrame::B16(Arc::new(f)), c)
        };
        // refresh_probs (6.1.2).
        if !h.error_res && !h.parallel {
            let saved = &self.ctxs[h.ctx_idx];
            fc.load_probs(saved);
            adapt_coef_probs(
                &mut fc,
                &counts,
                h.intra,
                h.last_frame_type == Some(KEY_FRAME),
            );
            if !h.intra {
                fc.load_probs2(saved);
                adapt_noncoef_probs(
                    &mut fc,
                    &counts,
                    h.interp_filter == header::SWITCHABLE,
                    comp.tx_mode == TX_MODE_SELECT,
                    h.allow_hp,
                );
            }
        }
        if h.refresh_ctx {
            self.ctxs[h.ctx_idx] = fc;
        }
        // Step 3 of 8.1, the output and the reference update (8.10).
        if h.seg.enabled && h.seg.update_map {
            for (p, m) in self.prev_seg.iter_mut().zip(&cur_mi) {
                *p = m.seg_id;
            }
        }
        if h.show_frame {
            out.push(frame.picture());
        }
        for i in 0..8 {
            if (h.refresh_flags >> i) & 1 == 1 {
                self.refs[i] = Some(frame.clone());
            }
        }
        self.prev_mi = cur_mi;
        Ok(())
    }

    fn ref_bufs8(&self, h: &FrameHeader) -> [Option<Arc<FrameBuf<u8>>>; 3] {
        std::array::from_fn(|i| {
            if h.intra {
                return None;
            }
            match &self.refs[h.ref_idx[i]] {
                Some(AnyFrame::B8(f)) => Some(f.clone()),
                _ => None,
            }
        })
    }

    fn ref_bufs16(&self, h: &FrameHeader) -> [Option<Arc<FrameBuf<u16>>>; 3] {
        std::array::from_fn(|i| {
            if h.intra {
                return None;
            }
            match &self.refs[h.ref_idx[i]] {
                Some(AnyFrame::B16(f)) => Some(f.clone()),
                _ => None,
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_pixels<P: Pixel>(
        &self,
        h: &FrameHeader,
        comp: header::Compressed,
        fc: &Probs,
        refs: [Option<Arc<FrameBuf<P>>>; 3],
        use_prev: bool,
        data: &[u8],
        cur_mi: &mut [MiInfo],
    ) -> Result<(FrameBuf<P>, Counts)> {
        let mut frame = FrameBuf::<P>::new(h.width, h.height, h.color)?;
        frame.render_w = h.render_w;
        frame.render_h = h.render_h;
        // Reference scaling (8.5.2.3).
        let mut scale = [Scale::default(); 3];
        for (s, r) in scale.iter_mut().zip(&refs) {
            if let Some(r) = r {
                let (rw, rh) = (r.width as i64, r.height as i64);
                let (w, hh) = (h.width as i64, h.height as i64);
                s.valid = 2 * w >= rw && 2 * hh >= rh && w <= 16 * rw && hh <= 16 * rh;
                s.x = (rw << 14) / w;
                s.y = (rh << 14) / hh;
                s.step_x = (16 * s.x) >> 14;
                s.step_y = (16 * s.y) >> 14;
            }
        }
        // Quantizers per segment (8.6.1).
        let bdi = ((h.color.bit_depth - 8) >> 1) as usize;
        let dc_q = |b: i32| DC_QLOOKUP[bdi][b.clamp(0, 255) as usize] as i32;
        let ac_q = |b: i32| AC_QLOOKUP[bdi][b.clamp(0, 255) as usize] as i32;
        let mut q = [[0i32; 4]; 8];
        for (seg, qs) in q.iter_mut().enumerate() {
            let qi = if h.seg.active(seg, SEG_LVL_ALT_Q) {
                let d = h.seg.feature_data[seg][SEG_LVL_ALT_Q];
                (if h.seg.abs_delta { d } else { h.base_q + d }).clamp(0, 255)
            } else {
                h.base_q
            };
            *qs = [
                dc_q(qi + h.dq_y_dc),
                ac_q(qi),
                dc_q(qi + h.dq_uv_dc),
                ac_q(qi + h.dq_uv_ac),
            ];
        }
        let fx = FrameCtx {
            h,
            c: comp,
            fc,
            refs: std::array::from_fn(|i| refs[i].as_deref()),
            scale,
            prev_mi: use_prev.then_some(&self.prev_mi[..]),
            prev_seg: &self.prev_seg,
            q,
            count: !h.error_res && !h.parallel,
            ss_x: h.color.ss_x as usize,
            ss_y: h.color.ss_y as usize,
        };
        // Tile sizes (6.4).
        let tile_cols = 1usize << h.tile_cols_log2;
        let tile_rows = 1usize << h.tile_rows_log2;
        let mut tiles: Vec<Vec<&[u8]>> = Vec::with_capacity(tile_rows);
        let mut pos = 0usize;
        for tr in 0..tile_rows {
            let mut row = Vec::with_capacity(tile_cols);
            for tc in 0..tile_cols {
                let last = tr == tile_rows - 1 && tc == tile_cols - 1;
                let size = if last {
                    data.len() - pos
                } else {
                    let b = data
                        .get(pos..pos + 4)
                        .ok_or_else(|| malformed("tile size cut short"))?;
                    pos += 4;
                    u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize
                };
                let t = data
                    .get(pos..pos.saturating_add(size))
                    .ok_or_else(|| malformed("tile runs past the end of the frame"))?;
                pos += size;
                row.push(t);
            }
            tiles.push(row);
        }
        let offset = |i: usize, mis: usize, log2: u32| {
            let sbs = mis.div_ceil(8);
            (((i * sbs) >> log2) << 3).min(mis)
        };
        let col_bounds: Vec<usize> = (0..=tile_cols)
            .map(|i| offset(i, h.mi_cols, h.tile_cols_log2))
            .collect();
        // Above contexts, cleared once per frame.
        let ss = [(0usize, 0usize), (fx.ss_x, fx.ss_y), (fx.ss_x, fx.ss_y)];
        let mut above_nz: [Vec<u8>; 3] =
            std::array::from_fn(|p| vec![0u8; frame.planes[p].stride / 4 + 16]);
        let mut above_part = vec![0u8; h.sb64_cols * 8 + 8];
        let mut above_seg = vec![0u8; h.sb64_cols * 8 + 8];
        let mut counts = Counts::default();
        for (tr, row) in tiles.iter().enumerate() {
            let row_start = offset(tr, h.mi_rows, h.tile_rows_log2);
            let row_end = offset(tr + 1, h.mi_rows, h.tile_rows_log2);
            // Split the frame among the tile columns.
            let mut planes: [Vec<Vec<&mut [P]>>; 3] = Default::default();
            let mut nz: [Vec<Vec<&mut [u8]>>; 3] = Default::default();
            let mut x0s: [Vec<usize>; 3] = Default::default();
            for (p, pl) in frame.planes.iter_mut().enumerate() {
                let (sx, _) = ss[p];
                let mut b: Vec<usize> = col_bounds.iter().map(|&c| (c * 8) >> sx).collect();
                *b.last_mut().unwrap() = pl.stride;
                x0s[p] = b.clone();
                planes[p] = split_cols(&mut pl.data, pl.stride, &b);
            }
            for (p, an) in above_nz.iter_mut().enumerate() {
                let mut nb: Vec<usize> = x0s[p].iter().map(|&x| x / 4).collect();
                let len = an.len();
                *nb.last_mut().unwrap() = len;
                nz[p] = split_cols(an, len, &nb);
            }
            let mut mb = col_bounds.clone();
            *mb.last_mut().unwrap() = h.mi_cols;
            let mut mi_split = split_cols(cur_mi, h.mi_cols.max(1), &mb);
            let mut pb = col_bounds.clone();
            *pb.last_mut().unwrap() = above_part.len();
            let len = above_part.len();
            let mut part_split = split_cols(&mut above_part, len, &pb);
            let len = above_seg.len();
            let mut seg_split = split_cols(&mut above_seg, len, &pb);
            let mut jobs = Vec::with_capacity(tile_cols);
            let mut plane_iters: [std::vec::IntoIter<Vec<&mut [P]>>; 3] =
                planes.map(|v| v.into_iter());
            let mut nz_iters: [std::vec::IntoIter<Vec<&mut [u8]>>; 3] = nz.map(|v| v.into_iter());
            let mut mi_iter = mi_split.drain(..);
            let mut part_iter = part_split.drain(..);
            let mut seg_iter = seg_split.drain(..);
            for tc in 0..tile_cols {
                let pls: [PlaneCols<P>; 3] = std::array::from_fn(|p| PlaneCols {
                    rows: plane_iters[p].next().unwrap_or_default(),
                    x0: x0s[p][tc],
                });
                let anz: [&mut [u8]; 3] = std::array::from_fn(|p| {
                    nz_iters[p]
                        .next()
                        .and_then(|mut v| v.pop())
                        .unwrap_or_default()
                });
                let d = TileData {
                    planes: pls,
                    mi: mi_iter.next().unwrap_or_default(),
                    above_nz: anz,
                    above_part: part_iter
                        .next()
                        .and_then(|mut v| v.pop())
                        .unwrap_or_default(),
                    above_seg: seg_iter
                        .next()
                        .and_then(|mut v| v.pop())
                        .unwrap_or_default(),
                };
                jobs.push((tc, d));
            }
            let run = |(tc, d): (usize, TileData<P>)| -> Result<Box<Counts>> {
                let mut t = Tile::new(&fx, row[tc], col_bounds[tc], col_bounds[tc + 1], d)?;
                t.decode(row_start, row_end)?;
                Ok(t.counts)
            };
            let results: Vec<Result<Box<Counts>>> = match &self.pool {
                Some(pool) if tile_cols > 1 => {
                    let mut slots: Vec<Option<Result<Box<Counts>>>> =
                        (0..jobs.len()).map(|_| None).collect();
                    pool.scope(|s| {
                        for (job, slot) in jobs.into_iter().zip(slots.iter_mut()) {
                            let run = &run;
                            s.spawn(move |_| *slot = Some(run(job)));
                        }
                    });
                    slots
                        .into_iter()
                        .map(|s| s.unwrap_or_else(|| Err(malformed("tile not decoded"))))
                        .collect()
                }
                _ => jobs.into_iter().map(run).collect(),
            };
            for r in results {
                counts.merge(&*r?);
            }
        }
        // Loop filter (8.8).
        if h.lf.level > 0 {
            let lfr = LfFrame::new(h, cur_mi);
            let sb_cols = h.sb64_cols;
            let works = lf::split_rows(&mut frame, sb_cols);
            match &self.pool {
                Some(pool) if works.len() > 1 => {
                    let n = works.len();
                    let slots: Vec<Mutex<Option<lf::RowWork<P>>>> =
                        works.into_iter().map(|w| Mutex::new(Some(w))).collect();
                    let next = AtomicUsize::new(0);
                    pool.scope(|s| {
                        for _ in 0..self.threads.min(n) {
                            s.spawn(|_| {
                                loop {
                                    let i = next.fetch_add(1, Ordering::SeqCst);
                                    if i >= n {
                                        break;
                                    }
                                    let w =
                                        slots[i].lock().unwrap_or_else(|e| e.into_inner()).take();
                                    if let Some(w) = w {
                                        lf::run_row(&lfr, w, sb_cols);
                                    }
                                }
                            });
                        }
                    });
                }
                _ => {
                    for w in works {
                        lf::run_row(&lfr, w, sb_cols);
                    }
                }
            }
        }
        Ok((frame, counts))
    }
}
