//! Frame decoding: the header and prediction records on the calling thread, then the
//! macroblock rows (tokens, reconstruction, loop filter) on worker threads in a wavefront,
//! each row a little behind the one above it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use crate::boolcoder::BoolDecoder;
use crate::frame::{Frame, Plane, copy_n};
use crate::header::{self, Dequant, Header, LfDeltas, Persistent, Segmentation, Tag};
use crate::idct;
use crate::inter;
use crate::intra::{self, CS, YS};
use crate::loopfilter::{self, Params};
use crate::modes::{self, B_PRED, MbInfo, SPLITMV};
use crate::tokens::{self, Ctx, DC_ONLY, FULL, MbCoeffs, NONE};
use crate::{Error, Picture, malformed};

/// Macroblocks between progress updates a row publishes to the row below.
const BATCH: usize = 4;

/// What a row has finished, and what the row below reads from it.
#[derive(Default)]
struct RowState {
    /// Macroblocks whose tokens are read.
    tok: usize,
    /// Macroblocks reconstructed.
    recon: usize,
    /// Macroblocks whose bottom four lines are final.
    filt: usize,
    waiting: bool,
    /// Token contexts after this row.
    ctx: Vec<Ctx>,
    /// The bottom line of each plane before loop filtering (intra prediction edges).
    edge: [Vec<u8>; 3],
    /// The bottom four lines of each plane after this row's filtering, line by line.
    bottom: [Vec<u8>; 3],
}

#[derive(Default)]
struct RowSync {
    m: Mutex<RowState>,
    cv: Condvar,
}

impl RowSync {
    fn lock(&self) -> MutexGuard<'_, RowState> {
        self.m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits until `ready`, then runs `then` on the state; None when the frame is aborted.
    fn wait<T>(
        &self,
        abort: &AtomicBool,
        ready: impl Fn(&RowState) -> bool,
        then: impl FnOnce(&RowState) -> T,
    ) -> Option<T> {
        let mut g = self.lock();
        loop {
            if abort.load(Ordering::SeqCst) {
                return None;
            }
            if ready(&g) {
                return Some(then(&g));
            }
            g.waiting = true;
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn publish(&self, f: impl FnOnce(&mut RowState)) {
        let mut g = self.lock();
        f(&mut g);
        if g.waiting {
            g.waiting = false;
            self.cv.notify_all();
        }
    }
}

/// Everything the row workers share for one frame.
struct Shared<'a> {
    h: &'a Header,
    seg: &'a Segmentation,
    lf: &'a LfDeltas,
    modes: &'a [MbInfo],
    probs: &'a [[[[u8; 11]; 3]; 8]; 4],
    dq: [Dequant; 4],
    refs: [Option<&'a Frame>; 4],
    cols: usize,
    parts: Vec<Mutex<Option<BoolDecoder<'a>>>>,
    sync: &'a [RowSync],
    abort: AtomicBool,
    error: Mutex<Option<Error>>,
    filter: bool,
    lfp: [Params; 64],
    bilinear: bool,
    full_pixel: bool,
    /// Stride and border of each plane of the frame being decoded.
    geom: [(usize, usize); 3],
}

impl Shared<'_> {
    fn fail(&self, e: Error) {
        {
            let mut g = self.error.lock().unwrap_or_else(|e| e.into_inner());
            g.get_or_insert(e);
        }
        self.abort.store(true, Ordering::SeqCst);
        for s in self.sync {
            let _g = s.lock();
            s.cv.notify_all();
        }
    }
}

/// One macroblock row of the frame being decoded: its lines of each plane, and the three
/// lines above it that its loop filtering changes (written back after the frame).
struct Job<'f> {
    r: usize,
    planes: [&'f mut [u8]; 3],
    fix: [&'f mut [u8]; 3],
}

/// Per-thread buffers.
#[derive(Default)]
struct Scratch {
    coeffs: Vec<MbCoeffs>,
    has: Vec<bool>,
    above_ctx: Vec<Ctx>,
    my_ctx: Vec<Ctx>,
    above_edge: [Vec<u8>; 3],
    my_edge: [Vec<u8>; 3],
    above_bottom: [Vec<u8>; 3],
}

impl Scratch {
    fn size(&mut self, cols: usize) {
        self.coeffs.resize(cols, MbCoeffs::default());
        self.has.resize(cols, false);
        self.above_ctx.resize(cols, [0; 9]);
        self.my_ctx.resize(cols, [0; 9]);
        for p in 0..3 {
            let w = if p == 0 { 16 } else { 8 } * cols;
            self.above_edge[p].resize(w, 0);
            self.my_edge[p].resize(w, 0);
            self.above_bottom[p].resize(4 * w, 0);
        }
    }
}

pub(crate) struct State {
    threads: usize,
    st: Persistent,
    seg_map: Vec<u8>,
    modes: Vec<MbInfo>,
    /// Indexed by reference frame: 1 last, 2 golden, 3 altref.
    refs: [Option<Arc<Frame>>; 4],
    spare: Vec<Frame>,
    width: u32,
    height: u32,
    cols: usize,
    rows: usize,
    reserved_color_space: bool,
    need_key: bool,
    sync: Vec<RowSync>,
    scratch: Vec<Scratch>,
    _budget: Option<specular_core::Reservation>,
}

impl State {
    pub(crate) fn new(threads: usize) -> State {
        let threads = match threads {
            0 => std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .min(8),
            n => n,
        };
        State {
            threads,
            st: Persistent::new(),
            seg_map: Vec::new(),
            modes: Vec::new(),
            refs: [None, None, None, None],
            spare: Vec::new(),
            width: 0,
            height: 0,
            cols: 0,
            rows: 0,
            reserved_color_space: false,
            need_key: true,
            sync: Vec::new(),
            scratch: Vec::new(),
            _budget: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        let threads = self.threads;
        *self = State::new(threads);
    }

    pub(crate) fn decode(&mut self, data: &[u8]) -> Result<Option<Picture>, Error> {
        let r = self.decode_frame(data);
        if r.is_err() {
            self.need_key = true;
        }
        r
    }

    fn resize(&mut self, tag: &Tag) -> Result<(), Error> {
        if tag.width == 0 || tag.height == 0 {
            return Err(malformed("key frame with no width or height"));
        }
        if tag.width == self.width && tag.height == self.height {
            return Ok(());
        }
        let cols = tag.width.div_ceil(16) as usize;
        let rows = tag.height.div_ceil(16) as usize;
        self.refs = [None, None, None, None];
        self.spare.clear();
        self._budget = None;
        // The current frame, three references and a spare.
        let bytes = 5 * Frame::bytes(cols, rows) + (cols * rows * 200) as u64;
        let r = specular_core::budget::reserve(bytes)
            .map_err(|n| Error::Unsupported(format!("this VP8 video {n}")))?;
        self._budget = Some(r);
        self.width = tag.width;
        self.height = tag.height;
        self.cols = cols;
        self.rows = rows;
        self.seg_map = vec![0; cols * rows];
        self.sync = (0..rows).map(|_| RowSync::default()).collect();
        Ok(())
    }

    fn decode_frame(&mut self, data: &[u8]) -> Result<Option<Picture>, Error> {
        let tag = header::parse_tag(data)?;
        if tag.version > 3 {
            return Err(Error::Unsupported(format!(
                "VP8 bitstream version {} can't be played",
                tag.version
            )));
        }
        if tag.key {
            self.resize(&tag)?;
        } else if self.need_key || self.refs[1..].iter().any(Option::is_none) {
            return Err(malformed("an interframe needs a key frame before it"));
        }
        let first_end = tag
            .header_len
            .checked_add(tag.first_size)
            .filter(|&e| e <= data.len())
            .ok_or_else(|| malformed("first partition runs past the end of the frame"))?;
        let mut d = BoolDecoder::new(&data[tag.header_len..first_end]);
        let (h, saved) = header::parse_header(&mut d, &tag, &mut self.st)?;
        if tag.key {
            self.reserved_color_space = h.color_space;
        }
        // Token partitions.
        let n = h.partitions;
        let sizes_end = first_end + 3 * (n - 1);
        if sizes_end > data.len() {
            return Err(malformed("partition sizes run past the end of the frame"));
        }
        let mut parts = Vec::with_capacity(n);
        let mut at = sizes_end;
        for i in 0..n {
            let size = if i + 1 < n {
                let s = &data[first_end + 3 * i..];
                s[0] as usize | (s[1] as usize) << 8 | (s[2] as usize) << 16
            } else {
                data.len() - at
            };
            if at + size > data.len() {
                return Err(malformed(
                    "a token partition runs past the end of the frame",
                ));
            }
            parts.push(Mutex::new(Some(BoolDecoder::new(&data[at..at + size]))));
            at += size;
        }
        let (cols, rows) = (self.cols, self.rows);
        modes::parse_modes(
            &mut d,
            &h,
            &self.st,
            &mut self.seg_map,
            cols,
            rows,
            &mut self.modes,
        );
        if d.overrun_bits() > 0 {
            return Err(malformed("the first partition is truncated"));
        }

        let mut cur = self.spare.pop().unwrap_or_else(|| Frame::new(cols, rows));
        let filter = h.filter_level != 0;
        let mut lfp = [Params::default(); 64];
        for (l, p) in lfp.iter_mut().enumerate() {
            *p = loopfilter::params(l as i32, h.sharpness, h.key);
        }
        let geom = [0, 1, 2].map(|p| (cur.planes[p].stride, cur.planes[p].border));
        let shared = Shared {
            h: &h,
            seg: &self.st.seg,
            lf: &self.st.lf,
            modes: &self.modes,
            probs: &self.st.probs.coeff,
            dq: header::dequant_factors(&h, &self.st.seg),
            refs: [0, 1, 2, 3].map(|i| self.refs[i].as_deref()),
            cols,
            parts,
            sync: &self.sync,
            abort: AtomicBool::new(false),
            error: Mutex::new(None),
            filter,
            lfp,
            bilinear: h.version != 0,
            full_pixel: h.version == 3,
            geom,
        };
        for s in &self.sync {
            let mut g = s.lock();
            g.tok = 0;
            g.recon = 0;
            g.filt = 0;
            g.waiting = false;
            g.ctx.resize(cols, [0; 9]);
            for p in 0..3 {
                let w = if p == 0 { 16 } else { 8 } * cols;
                g.edge[p].resize(w, 0);
                g.bottom[p].resize(4 * w, 0);
            }
        }
        // Lines 13..15 (luma) or 5..7 (chroma) of each row above another, as the row
        // below's loop filtering leaves them.
        let fix_w = [16 * cols, 8 * cols, 8 * cols];
        let mut fixes: [Vec<u8>; 3] = [0, 1, 2].map(|p| vec![0u8; 3 * fix_w[p] * rows]);
        let workers = self.threads.min(rows).max(1);
        self.scratch.resize_with(workers, Scratch::default);
        {
            let mut jobs: Vec<Vec<Job<'_>>> = (0..workers).map(|_| Vec::new()).collect();
            let [py, pu, pv] = &mut cur.planes;
            let ys = mb_rows(py, 16, rows);
            let us = mb_rows(pu, 8, rows);
            let vs = mb_rows(pv, 8, rows);
            let [fy, fu, fv] = &mut fixes;
            let fys = fy.chunks_mut(3 * fix_w[0]);
            let fus = fu.chunks_mut(3 * fix_w[1]);
            let fvs = fv.chunks_mut(3 * fix_w[2]);
            for (r, ((((y, u), v), fy), (fu, fv))) in ys
                .into_iter()
                .zip(us)
                .zip(vs)
                .zip(fys)
                .zip(fus.zip(fvs))
                .enumerate()
            {
                jobs[r % workers].push(Job {
                    r,
                    planes: [y, u, v],
                    fix: [fy, fu, fv],
                });
            }
            let shared = &shared;
            if workers == 1 {
                let s = &mut self.scratch[0];
                s.size(cols);
                run_rows(shared, jobs.pop().unwrap_or_default(), s);
            } else {
                std::thread::scope(|scope| {
                    for (t, (js, s)) in jobs.into_iter().zip(self.scratch.iter_mut()).enumerate() {
                        s.size(cols);
                        specular_thread::spawn_scoped(scope, format!("vp8-row-{t}"), move || {
                            run_rows(shared, js, s)
                        });
                    }
                });
            }
        }
        if let Some(e) = shared.error.into_inner().unwrap_or_else(|e| e.into_inner()) {
            self.spare.push(cur);
            return Err(e);
        }
        if filter {
            for (p, plane) in cur.planes.iter_mut().enumerate() {
                let mb_h = if p == 0 { 16 } else { 8 };
                let w = fix_w[p];
                for r in 1..rows {
                    let fix = &fixes[p][3 * w * r..3 * w * (r + 1)];
                    for k in 0..3 {
                        let o = plane.row(mb_h * r - 3 + k);
                        plane.data[o..o + w].copy_from_slice(&fix[k * w..k * w + w]);
                    }
                }
            }
        }
        for p in &mut cur.planes {
            p.extend();
        }
        if let Some(saved) = saved {
            self.st.probs = saved;
        }
        let pic = tag.show.then(|| Picture {
            width: self.width,
            height: self.height,
            planes: [
                cur.planes[0].crop(self.width as usize, self.height as usize),
                cur.planes[1].crop(
                    self.width.div_ceil(2) as usize,
                    self.height.div_ceil(2) as usize,
                ),
                cur.planes[2].crop(
                    self.width.div_ceil(2) as usize,
                    self.height.div_ceil(2) as usize,
                ),
            ],
            reserved_color_space: self.reserved_color_space,
        });
        self.update_refs(&h, cur);
        self.need_key = false;
        Ok(pic)
    }

    fn update_refs(&mut self, h: &Header, cur: Frame) {
        let cur = Arc::new(cur);
        let old = self.refs.clone();
        match h.copy_to_alt {
            1 => self.refs[3] = self.refs[1].clone(),
            2 => self.refs[3] = self.refs[2].clone(),
            _ => {}
        }
        match h.copy_to_golden {
            1 => self.refs[2] = self.refs[1].clone(),
            2 => self.refs[2] = self.refs[3].clone(),
            _ => {}
        }
        if h.refresh_golden {
            self.refs[2] = Some(cur.clone());
        }
        if h.refresh_alt {
            self.refs[3] = Some(cur.clone());
        }
        if h.refresh_last {
            self.refs[1] = Some(cur.clone());
        }
        drop(cur);
        for f in old.into_iter().flatten() {
            if let Ok(f) = Arc::try_unwrap(f)
                && self.spare.len() < 2
            {
                self.spare.push(f);
            }
        }
    }
}

/// The lines of each macroblock row of a plane.
fn mb_rows(p: &mut Plane, mb_h: usize, rows: usize) -> Vec<&mut [u8]> {
    let (stride, border) = (p.stride, p.border);
    p.data[border * stride..]
        .chunks_mut(mb_h * stride)
        .take(rows)
        .collect()
}

fn run_rows(sh: &Shared<'_>, jobs: Vec<Job<'_>>, s: &mut Scratch) {
    for job in jobs {
        if run_row(sh, job, s).is_none() {
            // Wake the rows waiting on this one.
            if !sh.abort.load(Ordering::SeqCst) {
                sh.fail(malformed("a token partition is missing"));
            }
            return;
        }
    }
}

/// Decodes one macroblock row; None when the frame is aborted.
fn run_row(sh: &Shared<'_>, job: Job<'_>, s: &mut Scratch) -> Option<()> {
    let Job {
        r,
        planes: [py, pu, pv],
        fix,
    } = job;
    let cols = sh.cols;
    let me = &sh.sync[r];
    let above = (r > 0).then(|| &sh.sync[r - 1]);
    let np = sh.parts.len();

    // Tokens.
    if r >= np {
        sh.sync[r - np].wait(&sh.abort, |g| g.tok == cols, |_| ())?;
    }
    let mut d = sh.parts[r % np]
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()?;
    let mut left: Ctx = [0; 9];
    let mut avail = 0;
    let mut published = 0;
    for c in 0..cols {
        let mut ctx = match above {
            None => [0; 9],
            Some(a) => {
                if c >= avail {
                    avail = a.wait(
                        &sh.abort,
                        |g| g.tok > c,
                        |g| {
                            s.above_ctx[avail..g.tok].copy_from_slice(&g.ctx[avail..g.tok]);
                            g.tok
                        },
                    )?;
                }
                s.above_ctx[c]
            }
        };
        let mb = &sh.modes[r * cols + c];
        if mb.skip {
            tokens::skip_mb(mb, &mut ctx, &mut left);
            s.has[c] = false;
        } else {
            s.has[c] = tokens::read_mb(
                &mut d,
                sh.probs,
                mb,
                &sh.dq[mb.segment as usize & 3],
                &mut ctx,
                &mut left,
                &mut s.coeffs[c],
            );
        }
        s.my_ctx[c] = ctx;
        if (c + 1) % BATCH == 0 && c + 1 < cols {
            me.publish(|g| {
                g.ctx[published..=c].copy_from_slice(&s.my_ctx[published..=c]);
                g.tok = c + 1;
            });
            published = c + 1;
        }
    }
    if d.overrun_bits() > 0 {
        sh.fail(malformed("a token partition is truncated"));
        return None;
    }
    *sh.parts[r % np].lock().unwrap_or_else(|e| e.into_inner()) = Some(d);
    me.publish(|g| {
        g.ctx[published..].copy_from_slice(&s.my_ctx[published..]);
        g.tok = cols;
    });

    // Reconstruction, with the loop filter one macroblock behind.
    let mut avail = 0;
    let mut published = 0;
    let mut fdone = 0;
    let mut favail = 0;
    let mut ws_y = [0u8; YS * 17];
    let mut ws_u = [0u8; CS * 9];
    let mut ws_v = [0u8; CS * 9];
    let [fy, fu, fv] = fix;
    for c in 0..=cols {
        if c < cols {
            if let Some(a) = above {
                let need = (c + 2).min(cols);
                if avail < need {
                    avail = a.wait(
                        &sh.abort,
                        |g| g.recon >= need,
                        |g| {
                            let n = g.recon;
                            for p in 0..3 {
                                let w = if p == 0 { 16 } else { 8 };
                                s.above_edge[p][avail * w..n * w]
                                    .copy_from_slice(&g.edge[p][avail * w..n * w]);
                            }
                            n
                        },
                    )?;
                }
            }
            let mb = &sh.modes[r * cols + c];
            reconstruct(
                sh,
                r,
                c,
                mb,
                &mut s.coeffs[c],
                [&mut *py, &mut *pu, &mut *pv],
                [&mut ws_y[..], &mut ws_u[..], &mut ws_v[..]],
                [
                    &s.above_edge[0][..],
                    &s.above_edge[1][..],
                    &s.above_edge[2][..],
                ],
            );
            for p in 0..3 {
                let (w, src, ws_s) = match p {
                    0 => (16, &ws_y[..], YS),
                    1 => (8, &ws_u[..], CS),
                    _ => (8, &ws_v[..], CS),
                };
                copy_n(&mut s.my_edge[p][c * w..], &src[w * ws_s + 1..], w);
            }
            if (c + 1) % BATCH == 0 && c + 1 < cols {
                me.publish(|g| {
                    for p in 0..3 {
                        let w = if p == 0 { 16 } else { 8 };
                        g.edge[p][published * w..(c + 1) * w]
                            .copy_from_slice(&s.my_edge[p][published * w..(c + 1) * w]);
                    }
                    g.recon = c + 1;
                });
                published = c + 1;
            }
        } else {
            me.publish(|g| {
                for p in 0..3 {
                    let w = if p == 0 { 16 } else { 8 };
                    g.edge[p][published * w..].copy_from_slice(&s.my_edge[p][published * w..]);
                }
                g.recon = cols;
            });
        }
        if !sh.filter || c == 0 {
            continue;
        }
        // Filter macroblock j once its right neighbour is reconstructed.
        let j = c - 1;
        if let Some(a) = above {
            let need = j + 1;
            if favail < need {
                favail = a.wait(
                    &sh.abort,
                    |g| g.filt >= need,
                    |g| {
                        let n = g.filt;
                        for p in 0..3 {
                            let w = if p == 0 { 16 } else { 8 };
                            let lw = w * cols;
                            for k in 0..4 {
                                s.above_bottom[p][k * lw + favail * w..k * lw + n * w]
                                    .copy_from_slice(
                                        &g.bottom[p][k * lw + favail * w..k * lw + n * w],
                                    );
                            }
                        }
                        n
                    },
                )?;
            }
        }
        let mb = &sh.modes[r * cols + j];
        let lvl = loopfilter::level(sh.h, sh.seg, sh.lf, mb);
        if lvl > 0 {
            let inner = mb.y_mode == B_PRED || mb.y_mode == SPLITMV || s.has[j];
            filter_mb(
                sh,
                r,
                j,
                &sh.lfp[lvl as usize],
                inner,
                [&mut *py, &mut *pu, &mut *pv],
                [&mut *fy, &mut *fu, &mut *fv],
                &s.above_bottom,
            );
        } else if r > 0 {
            // Unfiltered: the lines above stay as the row above left them.
            for (p, f) in [&mut *fy, &mut *fu, &mut *fv].into_iter().enumerate() {
                let w = if p == 0 { 16 } else { 8 };
                let lw = w * cols;
                for k in 0..3 {
                    copy_n(
                        &mut f[k * lw + j * w..],
                        &s.above_bottom[p][(k + 1) * lw + j * w..],
                        w,
                    );
                }
            }
        }
        // Macroblocks before j are final now; all of them at the end of the row.
        let done = if c == cols { cols } else { j };
        if done > fdone && (done - fdone >= BATCH || done == cols) {
            let from = fdone;
            me.publish(|g| {
                for p in 0..3 {
                    let (w, mb_h) = if p == 0 { (16, 16) } else { (8, 8) };
                    let (stride, border) = sh.geom[p];
                    let plane: &[u8] = match p {
                        0 => py,
                        1 => pu,
                        _ => pv,
                    };
                    let lw = w * cols;
                    for k in 0..4 {
                        let o = (mb_h - 4 + k) * stride + border;
                        g.bottom[p][k * lw + from * w..k * lw + done * w]
                            .copy_from_slice(&plane[o + from * w..o + done * w]);
                    }
                }
                g.filt = done;
            });
            fdone = done;
        }
    }
    if !sh.filter {
        me.publish(|g| g.filt = cols);
    }
    Some(())
}

/// Builds macroblock (`r`, `c`) in the workspaces and copies it into the row.
#[allow(clippy::too_many_arguments)]
fn reconstruct(
    sh: &Shared<'_>,
    r: usize,
    c: usize,
    mb: &MbInfo,
    co: &mut MbCoeffs,
    rows: [&mut [u8]; 3],
    ws: [&mut [u8]; 3],
    above: [&[u8]; 3],
) {
    let [wy, wu, wv] = ws;
    if mb.ref_frame == 0 {
        // Edges: 127 above the frame, 129 left of it.
        for (p, w) in [(0usize, &mut *wy), (1, &mut *wu), (2, &mut *wv)] {
            let (n, st) = if p == 0 { (16, YS) } else { (8, CS) };
            let (stride, border) = sh.geom[p];
            if r == 0 {
                w[..n + 1 + if p == 0 { 4 } else { 0 }].fill(127);
            } else {
                let a = above[p];
                w[0] = if c == 0 { 129 } else { a[n * c - 1] };
                copy_n(&mut w[1..], &a[n * c..], n);
                if p == 0 {
                    if c + 1 < sh.cols {
                        w[17..21].copy_from_slice(&a[16 * c + 16..16 * c + 20]);
                    } else {
                        w[17..21].fill(a[16 * c + 15]);
                    }
                }
            }
            let row = &rows[p];
            for y in 0..n {
                w[(y + 1) * st] = if c == 0 {
                    129
                } else {
                    row[y * stride + border + n * c - 1]
                };
            }
        }
        if mb.y_mode == B_PRED {
            let ar: [u8; 4] = [wy[17], wy[18], wy[19], wy[20]];
            for k in [4, 8, 12] {
                wy[k * YS + 17..k * YS + 21].copy_from_slice(&ar);
            }
            for k in 0..16 {
                let o = (4 * (k >> 2) + 1) * YS + 4 * (k & 3) + 1;
                intra::predict_sub(wy, o, mb.bmodes[k]);
                add_block(co, k, wy, o, YS);
            }
        } else {
            intra::predict_mb(wy, YS, 16, mb.y_mode, r > 0, c > 0);
            add_luma(co, wy);
        }
        intra::predict_mb(wu, CS, 8, mb.uv_mode, r > 0, c > 0);
        intra::predict_mb(wv, CS, 8, mb.uv_mode, r > 0, c > 0);
    } else {
        // Interframes start only once a key frame has set every reference.
        if let Some(refr) = sh.refs[mb.ref_frame as usize & 3] {
            predict_inter(sh, r, c, mb, refr, wy, wu, wv);
        }
        if mb.y_mode == SPLITMV {
            for k in 0..16 {
                let o = (4 * (k >> 2) + 1) * YS + 4 * (k & 3) + 1;
                add_block(co, k, wy, o, YS);
            }
        } else {
            add_luma(co, wy);
        }
    }
    for b in 0..4 {
        let o = (4 * (b >> 1) + 1) * CS + 4 * (b & 1) + 1;
        add_block(co, 16 + b, wu, o, CS);
        add_block(co, 20 + b, wv, o, CS);
    }
    co.clear();
    // Into the frame.
    let [ry, ru, rv] = rows;
    let (ys, yb) = sh.geom[0];
    for y in 0..16 {
        let o = y * ys + yb + 16 * c;
        ry[o..o + 16].copy_from_slice(&wy[(y + 1) * YS + 1..(y + 1) * YS + 17]);
    }
    let (cs, cb) = sh.geom[1];
    for y in 0..8 {
        let o = y * cs + cb + 8 * c;
        ru[o..o + 8].copy_from_slice(&wu[(y + 1) * CS + 1..(y + 1) * CS + 9]);
        rv[o..o + 8].copy_from_slice(&wv[(y + 1) * CS + 1..(y + 1) * CS + 9]);
    }
}

/// Adds the residue of luma when the macroblock has a Y2 block.
fn add_luma(co: &mut MbCoeffs, wy: &mut [u8]) {
    if co.kinds[24] != NONE {
        let mut dc = [0i16; 16];
        if co.kinds[24] == DC_ONLY {
            dc = [((co.c[384] as i32 + 3) >> 3) as i16; 16];
        } else {
            idct::iwht(&co.c[384..400], &mut dc);
        }
        for (k, &v) in dc.iter().enumerate() {
            co.c[k * 16] = v;
            if v != 0 && co.kinds[k] == NONE {
                co.kinds[k] = DC_ONLY;
            }
        }
    }
    for k in 0..16 {
        let o = (4 * (k >> 2) + 1) * YS + 4 * (k & 3) + 1;
        add_block(co, k, wy, o, YS);
    }
}

#[inline]
fn add_block(co: &MbCoeffs, b: usize, ws: &mut [u8], o: usize, stride: usize) {
    match co.kinds[b] {
        NONE => {}
        DC_ONLY => idct::dc_add(co.c[b * 16], ws, o, stride),
        FULL => idct::idct_add(&co.c[b * 16..b * 16 + 16], ws, o, stride),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn predict_inter(
    sh: &Shared<'_>,
    r: usize,
    c: usize,
    mb: &MbInfo,
    f: &Frame,
    wy: &mut [u8],
    wu: &mut [u8],
    wv: &mut [u8],
) {
    let [ry, ru, rv] = &f.planes;
    let (x, y) = (16 * c as i32, 16 * r as i32);
    let bl = sh.bilinear;
    let fp = |v: i32| if sh.full_pixel { v & !7 } else { v };
    if mb.y_mode != SPLITMV {
        let mv = mb.mv;
        inter::predict(ry, wy, YS + 1, YS, x, y, mv.x, mv.y, 16, 16, bl);
        let half = |v: i32| fp((v + if v < 0 { -1 } else { 1 }) / 2);
        let (ux, uy) = (half(mv.x), half(mv.y));
        inter::predict(ru, wu, CS + 1, CS, x / 2, y / 2, ux, uy, 8, 8, bl);
        inter::predict(rv, wv, CS + 1, CS, x / 2, y / 2, ux, uy, 8, 8, bl);
        return;
    }
    let m = &mb.mvs;
    for q in 0..4 {
        let k0 = (q >> 1) * 8 + (q & 1) * 2;
        let (bx, by) = (x + 8 * (q & 1) as i32, y + 8 * (q >> 1) as i32);
        let o = (8 * (q >> 1) + 1) * YS + 8 * (q & 1) + 1;
        if m[k0] == m[k0 + 1] && m[k0] == m[k0 + 4] && m[k0] == m[k0 + 5] {
            inter::predict(ry, wy, o, YS, bx, by, m[k0].x, m[k0].y, 8, 8, bl);
        } else {
            for k in [k0, k0 + 1, k0 + 4, k0 + 5] {
                let (kx, ky) = (x + 4 * (k & 3) as i32, y + 4 * (k >> 2) as i32);
                let o = (4 * (k >> 2) + 1) * YS + 4 * (k & 3) + 1;
                inter::predict(ry, wy, o, YS, kx, ky, m[k].x, m[k].y, 4, 4, bl);
            }
        }
        // Chroma: the average of the four luma vectors.
        let avg = |f: fn(&modes::Mv) -> i32| {
            let s = f(&m[k0]) + f(&m[k0 + 1]) + f(&m[k0 + 4]) + f(&m[k0 + 5]);
            fp((s + if s < 0 { -4 } else { 4 }) / 8)
        };
        let (ux, uy) = (avg(|v| v.x), avg(|v| v.y));
        let (cx, cy) = (x / 2 + 4 * (q & 1) as i32, y / 2 + 4 * (q >> 1) as i32);
        let o = (4 * (q >> 1) + 1) * CS + 4 * (q & 1) + 1;
        inter::predict(ru, wu, o, CS, cx, cy, ux, uy, 4, 4, bl);
        inter::predict(rv, wv, o, CS, cx, cy, ux, uy, 4, 4, bl);
    }
}

/// Loop-filters macroblock (`r`, `j`) in the order of section 15.1.
#[allow(clippy::too_many_arguments)]
fn filter_mb(
    sh: &Shared<'_>,
    r: usize,
    j: usize,
    p: &Params,
    inner: bool,
    rows: [&mut [u8]; 3],
    fix: [&mut [u8]; 3],
    above: &[Vec<u8>; 3],
) {
    let planes = if sh.h.simple_filter { 1 } else { 3 };
    for (pi, (row, fx)) in rows.into_iter().zip(fix).enumerate() {
        let (stride, border) = sh.geom[pi];
        let n = if pi == 0 { 16 } else { 8 };
        if pi >= planes {
            // The simple filter leaves chroma alone.
            if r > 0 {
                let lw = n * sh.cols;
                for k in 0..3 {
                    copy_n(
                        &mut fx[k * lw + n * j..],
                        &above[pi][(k + 1) * lw + n * j..],
                        n,
                    );
                }
            }
            continue;
        }
        let x0 = border + n * j;
        let simple = sh.h.simple_filter;
        // Left macroblock edge, then the subblock edges.
        if j > 0 {
            if simple {
                loopfilter::simple(row, x0, 1, stride, n, p.mb_limit);
            } else {
                loopfilter::mb_edge(row, x0, 1, stride, n, p);
            }
        }
        if inner {
            for e in (4..n).step_by(4) {
                if simple {
                    loopfilter::simple(row, x0 + e, 1, stride, n, p.sub_limit);
                } else {
                    loopfilter::sub_edge(row, x0 + e, 1, stride, n, p);
                }
            }
        }
        // Top macroblock edge, through a copy of the four lines above.
        if r > 0 {
            let mut t = [0u8; 8 * 16];
            let lw = n * sh.cols;
            for k in 0..4 {
                copy_n(&mut t[k * n..], &above[pi][k * lw + n * j..], n);
                copy_n(&mut t[(4 + k) * n..], &row[k * stride + x0..], n);
            }
            if simple {
                loopfilter::simple(&mut t, 4 * n, n, 1, n, p.mb_limit);
            } else {
                loopfilter::mb_edge(&mut t, 4 * n, n, 1, n, p);
            }
            let fw = n * sh.cols;
            for k in 0..3 {
                copy_n(&mut fx[k * fw + n * j..], &t[(1 + k) * n..], n);
            }
            for k in 0..4 {
                copy_n(&mut row[k * stride + x0..], &t[(4 + k) * n..], n);
            }
        }
        if inner {
            for e in (4..n).step_by(4) {
                if simple {
                    loopfilter::simple(row, e * stride + x0, stride, 1, n, p.sub_limit);
                } else {
                    loopfilter::sub_edge(row, e * stride + x0, stride, 1, n, p);
                }
            }
        }
    }
}
