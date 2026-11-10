//! Slice data (7.3.4) and macroblock decoding: parsing with CAVLC or CABAC, motion vector
//! and intra mode prediction, reconstruction, and the per-picture work area that deblocks
//! and publishes macroblock rows as they complete.

use std::sync::{Arc, Mutex};

use crate::bits::{BitReader, PAD, rbsp_end};
use crate::cabac::Cabac;
use crate::cavlc::residual_block;
use crate::deblock::{self, DB_OFF, DbParams};
use crate::inter::{self, Weighting};
use crate::intra::{self, Avail};
use crate::mb::{DIRECT, I4, I8, I16, INTER, MbInfo, PCM, SKIP};
use crate::params::{Pps, Sps};
use crate::picture::{INTRA_MOTION, MbMotion, PicShared, Planes, Structure, Unit, View};
use crate::pixel::{Format, Pix};
use crate::slice::{SliceHeader, SliceType};
use crate::spec_tables::{
    CBP_INTER, CBP_INTER_MONO, CBP_INTRA, CBP_INTRA_MONO, LAST8X8, SIG8X8, SIG8X8_FIELD,
};
use crate::tables::{BLK_TO_RASTER, FIELD4, FIELD8, RASTER_TO_BLK, ZIGZAG4, ZIGZAG8, qpc};
use crate::transform::{bypass_add, hadamard4, idct4_add, idct8_add};

/// A reference picture as one slice sees it: a frame, or a field (`parity`).
#[derive(Clone)]
pub struct RefPic {
    pub pic: Arc<PicShared>,
    /// Identifies the frame or field (see `dpb::RefFrame::id`).
    pub id: u32,
    /// PicOrderCnt of the frame or field.
    pub poc: i32,
    /// Field order counts of the frame (top, bottom).
    pub pocs: [i32; 2],
    pub long: bool,
    pub parity: Option<u8>,
}

impl RefPic {
    #[inline(always)]
    pub fn view(&self) -> View<'_> {
        View::new(&self.pic, self.parity)
    }
}

/// One slice, ready for decoding: header, RBSP and reference lists.
pub struct SliceJob {
    pub hdr: SliceHeader,
    pub pps: Arc<Pps>,
    pub rbsp: Vec<u8>,
    pub len: usize,
    pub lists: [Vec<RefPic>; 2],
    /// slice_id of a partitioned slice (partition A), whose partitions B and C, when they
    /// came, are in `parts`.
    pub slice_id: Option<u32>,
    pub parts: [Option<Partition>; 2],
}

/// Slice data partition B or C: its RBSP and where its slice_data() starts.
pub struct Partition {
    pub rbsp: Vec<u8>,
    pub len: usize,
    pub data_bit: usize,
}

/// One picture (a frame or a field): all its slices.
pub struct PicJob {
    pub sps: Arc<Sps>,
    pub shared: Arc<PicShared>,
    /// Field parity of a field picture.
    pub parity: Option<u8>,
    /// PicOrderCnt of the picture.
    pub poc: i32,
    /// TopFieldOrderCnt and BottomFieldOrderCnt.
    pub pocs: [i32; 2],
    /// nal_ref_idc is not 0: later pictures may predict from this one.
    pub is_ref: bool,
    pub slices: Vec<SliceJob>,
    /// MbToSliceGroupMap with slice groups (FMO): slices then cover scattered macroblocks,
    /// and the picture is deblocked once all of them are decoded.
    pub sg_map: Option<Arc<[u8]>>,
    /// Work areas of earlier pictures, to reuse.
    pub pool: Arc<BufPool>,
}

/// The large buffers of a [`Work`] (samples and macroblock state), kept between pictures so
/// they are neither allocated nor cleared for each one.
pub struct Bufs<P> {
    y: Vec<P>,
    cb: Vec<P>,
    cr: Vec<P>,
    mbs: Vec<MbInfo>,
}

/// [`Bufs`] of either sample type.
pub enum AnyBufs {
    B(Bufs<u8>),
    W(Bufs<u16>),
}

/// What a work area's buffers fit: picture width and height in macroblocks (a field's
/// height for field pictures), and sample format.
type BufKey = (usize, usize, Format);

/// Work-area buffers of finished pictures. Holds at most as many as there can be pictures
/// decoding at once; buffers of another picture size or format are dropped when a picture
/// returns its own.
pub struct BufPool {
    cap: usize,
    free: Mutex<Vec<(BufKey, AnyBufs)>>,
}

impl BufPool {
    pub fn new(cap: usize) -> BufPool {
        BufPool {
            cap,
            free: Mutex::new(Vec::new()),
        }
    }

    fn take<P: Pix>(&self, key: BufKey) -> Option<Bufs<P>> {
        let mut f = self.free.lock().unwrap_or_else(|e| e.into_inner());
        let i = f.iter().position(|e| e.0 == key)?;
        P::bufs_out(f.swap_remove(i).1)
    }

    fn put<P: Pix>(&self, key: BufKey, b: Bufs<P>) {
        let mut f = self.free.lock().unwrap_or_else(|e| e.into_inner());
        // Same width and format: the frame and field pictures of one stream.
        f.retain(|e| e.0.0 == key.0 && e.0.2 == key.2);
        if f.len() >= self.cap {
            f.remove(0);
        }
        f.push((key, P::bufs_in(b)));
    }

    /// Frees every buffer held.
    pub fn clear(&self) {
        self.free.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// A frame's output planes being written (`Work::emit`): cropped, as bytes.
struct Out {
    planes: [Vec<u8>; 3],
    /// Cropped luma left column, top line, width and height.
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
}

type Res<T = ()> = Result<T, ()>;

/// Samples of `Work::mcb`: luma per list, then Cb and Cr per list (up to 256 each in 4:4:4).
const MCB_LEN: usize = 6 * 256;

/// The picture being decoded: samples before deblocking, macroblock state, progress. For a
/// field picture it holds just that field.
pub struct Work<P: Pix> {
    wmb: usize,
    hmb: usize,
    parity: Option<usize>,
    /// An MBAFF frame: macroblocks are decoded in pairs, and macroblock (x, y) of pair row
    /// r is stored at row 2 * r + y (see `Sd::start_mb`).
    mbaff: bool,
    fmt: Format,
    /// Chroma lines per macroblock: 8 (4:2:0) or 16 (4:2:2, 4:4:4).
    mbch: usize,
    /// Chroma samples per macroblock line: 8, or 16 in 4:4:4.
    mbcw: usize,
    y: Vec<P>,
    cb: Vec<P>,
    cr: Vec<P>,
    mbs: Vec<MbInfo>,
    db: Vec<DbParams>,
    shared: Arc<PicShared>,
    published: usize,
    /// Units carry samples and motion: the picture is a reference, or a field (fields are
    /// output from their units). A frame is output from `y`, `cb` and `cr` themselves.
    unit_samples: bool,
    /// Motion compensation scratch (`Sd::mc_block`), kept to skip clearing it per block:
    /// luma predictions of lists 0 and 1 (256 each), then Cb and Cr per list (256 each).
    mcb: Box<[P]>,
    /// Interpolation windows (`inter::luma`, `inter::chroma`).
    mcs: Box<inter::Scratch<P>>,
    /// A frame's output, written row by row as rows are published.
    out: Option<Out>,
    pool: Arc<BufPool>,
    /// MbToSliceGroupMap (FMO): macroblocks are decoded out of raster order, so rows are
    /// deblocked and published only once the picture is complete.
    sg_map: Option<Arc<[u8]>>,
}

impl<P: Pix> Work<P> {
    pub fn new(job: &PicJob) -> Work<P> {
        let shared = job.shared.clone();
        let parity = job.parity;
        let wmb = shared.width_mbs;
        let hmb = match parity {
            None => shared.height_mbs,
            Some(_) => shared.height_mbs / 2,
        };
        let fmt = shared.fmt;
        let mbch = 16 >> fmt.cshift();
        let mbcw = fmt.mbcw();
        let grey = P::of(fmt.grey(1));
        let (yn, cn) = (wmb * hmb * 256, wmb * hmb * mbcw * mbch);
        // Every macroblock writes all its samples (decoded or concealed), and macroblocks
        // read only the state of macroblocks before them, so reused buffers need no clearing
        // beyond marking every macroblock as not decoded. Monochrome chroma stays grey.
        let mono = job.sps.chroma_format_idc == 0;
        let b = match job.pool.take::<P>((wmb, hmb, fmt)) {
            Some(mut b) => {
                for m in &mut b.mbs {
                    m.slice = 0;
                }
                if mono {
                    b.cb.fill(grey);
                    b.cr.fill(grey);
                }
                b
            }
            None => Bufs {
                y: vec![P::default(); yn],
                cb: vec![grey; cn],
                cr: vec![grey; cn],
                mbs: vec![MbInfo::default(); wmb * hmb],
            },
        };
        let out = parity.is_none().then(|| {
            let c = job.sps.crop.map(|c| c as usize);
            let (w, h) = (wmb * 16 - c[0] - c[1], hmb * 16 - c[2] - c[3]);
            let (cw, ch) = (
                w.div_ceil(1 << fmt.cxshift()),
                h.div_ceil(1 << fmt.cshift()),
            );
            let bps = size_of::<P>();
            Out {
                planes: [
                    Vec::with_capacity(w * h * bps),
                    Vec::with_capacity(cw * ch * bps),
                    Vec::with_capacity(cw * ch * bps),
                ],
                x0: c[0],
                y0: c[2],
                w,
                h,
            }
        });
        Work {
            wmb,
            hmb,
            parity: parity.map(|p| p as usize),
            mbaff: job.sps.mbaff && parity.is_none(),
            fmt,
            mbch,
            mbcw,
            y: b.y,
            cb: b.cb,
            cr: b.cr,
            mbs: b.mbs,
            db: vec![DB_OFF],
            shared,
            published: 0,
            unit_samples: job.is_ref || parity.is_some(),
            mcb: vec![P::default(); MCB_LEN].into_boxed_slice(),
            mcs: inter::Scratch::new(fmt.max(1)),
            out,
            pool: job.pool.clone(),
            sg_map: job.sg_map.clone(),
        }
    }

    /// NextMbAddress (8-16): the next macroblock of the same slice group.
    fn next_addr(&self, addr: usize) -> usize {
        let Some(m) = &self.sg_map else {
            return addr + 1;
        };
        let total = self.wmb * self.hmb;
        let g = m.get(addr).copied();
        (addr + 1..total)
            .find(|&i| m.get(i).copied() == g)
            .unwrap_or(total)
    }

    /// Appends the lines of frame macroblock row `r` inside the crop window to the output.
    fn emit(&mut self, r: usize) {
        let Some(o) = &mut self.out else { return };
        let s = self.fmt.cshift();
        let sx = self.fmt.cxshift();
        let (ls, cs) = (self.wmb * 16, self.wmb * self.mbcw);
        let [oy, ocb, ocr] = &mut o.planes;
        let lines = (r * 16).max(o.y0)..((r + 1) * 16).min(o.y0 + o.h);
        for l in lines {
            let a = l * ls + o.x0;
            P::put_le(oy, &self.y[a..a + o.w]);
        }
        let (cx0, cy0) = (o.x0 >> sx, o.y0 >> s);
        let (cw, ch) = (o.w.div_ceil(1 << sx), o.h.div_ceil(1 << s));
        let ch_ = self.mbch;
        for l in (r * ch_).max(cy0)..((r + 1) * ch_).min(cy0 + ch) {
            let a = l * cs + cx0;
            P::put_le(ocb, &self.cb[a..a + cw]);
            P::put_le(ocr, &self.cr[a..a + cw]);
        }
    }

    /// Called once per macroblock, in address order.
    fn mb_done(&mut self, addr: usize) {
        if self.sg_map.is_some() {
            return;
        }
        if self.mbaff {
            // A pair row is complete: deblock the one above (intra prediction of this one
            // needed it unfiltered), which finishes the pair row above that.
            if (addr + 1).is_multiple_of(2 * self.wmb) {
                let r = addr / (2 * self.wmb);
                if r >= 1 {
                    self.deblock(r - 1);
                }
                if r >= 2 {
                    self.publish(2 * (r - 2));
                    self.publish(2 * (r - 2) + 1);
                }
            }
            return;
        }
        if (addr + 1).is_multiple_of(self.wmb) {
            let r = addr / self.wmb;
            if r >= 1 {
                self.deblock(r - 1);
            }
            if r >= 2 {
                self.publish(r - 2);
            }
        }
    }

    /// Deblocks macroblock row `r` (pair row for MBAFF).
    fn deblock(&mut self, r: usize) {
        if self.mbaff {
            deblock::mbaff_row(
                &mut self.y,
                &mut self.cb,
                &mut self.cr,
                &self.mbs,
                &self.db,
                self.wmb,
                r,
                self.fmt,
            );
            return;
        }
        deblock::row(
            &mut self.y,
            &mut self.cb,
            &mut self.cr,
            &self.mbs,
            &self.db,
            self.wmb,
            r,
            self.fmt,
        );
    }

    /// One field unit from every `step`-th line of 8 luma (4 or 8 chroma) lines starting at
    /// `y0` (`c0`).
    fn unit(&self, y0: usize, c0: usize, step: usize, motion: Option<Box<[MbMotion]>>) -> Unit {
        if !self.unit_samples {
            let none = || P::wrap(Box::default());
            return Unit {
                y: none(),
                cb: none(),
                cr: none(),
                motion: None,
            };
        }
        let ls = self.wmb * 16;
        let cs = self.wmb * self.mbcw;
        let lines = |pl: &[P], start: usize, n: usize, w: usize| {
            let mut out = Vec::with_capacity(n * w);
            for k in 0..n {
                let o = (start + k * step) * w;
                out.extend_from_slice(&pl[o..o + w]);
            }
            P::wrap(out.into())
        };
        let cn = self.mbch / 2;
        Unit {
            y: lines(&self.y, y0, 8, ls),
            cb: lines(&self.cb, c0, cn, cs),
            cr: lines(&self.cr, c0, cn, cs),
            motion,
        }
    }

    /// The two units of macroblock row `r`: (field, unit index, unit).
    fn units(&self, r: usize) -> [(usize, usize, Unit); 2] {
        let motion = self.unit_samples.then(|| {
            self.mbs[r * self.wmb..(r + 1) * self.wmb]
                .iter()
                .map(motion_of)
                .collect()
        });
        let ch = self.mbch;
        match self.parity {
            // Frame row r: its even lines are unit r of the top field, odd of the bottom.
            None => [
                (0, r, self.unit(r * 16, r * ch, 2, motion)),
                (1, r, self.unit(r * 16 + 1, r * ch + 1, 2, None)),
            ],
            Some(p) => [
                (p, 2 * r, self.unit(r * 16, r * ch, 1, motion)),
                (
                    p,
                    2 * r + 1,
                    self.unit(r * 16 + 8, r * ch + ch / 2, 1, None),
                ),
            ],
        }
    }

    fn publish(&mut self, r: usize) {
        if r < self.published || r >= self.hmb {
            return;
        }
        self.emit(r);
        for (p, u, unit) in self.units(r) {
            self.shared.publish(p, u, unit);
        }
        self.published = r + 1;
    }

    /// Deblocks the last row and publishes everything left.
    pub fn finish(&mut self) {
        if self.published < self.hmb {
            let rows = if self.mbaff { self.hmb / 2 } else { self.hmb };
            let from = if self.sg_map.is_some() { 0 } else { rows - 1 };
            for r in from..rows {
                self.deblock(r);
            }
        }
        self.close();
    }

    /// Publishes the rows left; a frame first hands its samples over for output, so they are
    /// there once the picture is done.
    fn close(&mut self) {
        if self.published >= self.hmb {
            return;
        }
        let rest: Vec<_> = (self.published..self.hmb)
            .map(|r| {
                self.emit(r);
                self.units(r)
            })
            .collect();
        if let Some(o) = self.out.take() {
            self.shared.set_frame(Planes(o.planes));
        }
        self.published = self.hmb;
        for (p, u, unit) in rest.into_iter().flatten() {
            self.shared.publish(p, u, unit);
        }
    }

    /// Where macroblock address `addr` is stored.
    fn storage(&self, addr: usize) -> usize {
        if self.mbaff {
            let pair = addr / 2;
            (2 * (pair / self.wmb) + (addr & 1)) * self.wmb + pair % self.wmb
        } else {
            addr
        }
    }

    /// Fills a macroblock that no slice covered (as a frame macroblock): the co-located
    /// samples of `src`, or grey.
    fn conceal(&mut self, addr: usize, src: Option<View>) {
        let addr = self.storage(addr);
        let (mx, my) = (addr % self.wmb, addr / self.wmb);
        let ls = self.wmb * 16;
        let cw = self.mbcw;
        let cs = self.wmb * cw;
        let ch = self.mbch;
        let lo = my * 16 * ls + mx * 16;
        let co = my * ch * cs + mx * cw;
        let max = self.fmt.max(0);
        match src {
            Some(v)
                if v.pic.width_mbs == self.wmb
                    && v.height() == self.hmb * 16
                    && v.pic.fmt == self.fmt =>
            {
                let mut buf = [P::default(); 256];
                inter::luma(
                    v,
                    mx as i32 * 16,
                    my as i32 * 16,
                    [0, 0],
                    16,
                    16,
                    &mut buf,
                    16,
                    max,
                    &mut self.mcs,
                );
                for r in 0..16 {
                    self.y[lo + r * ls..lo + r * ls + 16]
                        .copy_from_slice(&buf[r * 16..r * 16 + 16]);
                }
                let mut b = [[P::default(); 256]; 2];
                let (x, y) = ((mx * cw) as i32, (my * ch) as i32);
                if cw == 16 {
                    let maxc = self.fmt.max(1);
                    for (c, b) in b.iter_mut().enumerate() {
                        let mcs = &mut self.mcs;
                        inter::luma_plane(v, 1 + c, x, y, [0, 0], 16, 16, b, 16, maxc, mcs);
                    }
                } else {
                    let [b0, b1] = &mut b;
                    inter::chroma(v, x, y, [0, 0], 8, ch, [b0, b1], 8, &mut self.mcs);
                }
                for (plane, b) in [&mut self.cb, &mut self.cr].into_iter().zip(&b) {
                    for r in 0..ch {
                        plane[co + r * cs..co + r * cs + cw]
                            .copy_from_slice(&b[r * cw..r * cw + cw]);
                    }
                }
            }
            _ => {
                let (gy, gc) = (P::of(self.fmt.grey(0)), P::of(self.fmt.grey(1)));
                for r in 0..16 {
                    self.y[lo + r * ls..lo + r * ls + 16].fill(gy);
                }
                for r in 0..ch {
                    self.cb[co + r * cs..co + r * cs + cw].fill(gc);
                    self.cr[co + r * cs..co + r * cs + cw].fill(gc);
                }
            }
        }
        self.mbs[addr] = MbInfo {
            kind: INTER,
            field: self.parity.is_some(),
            ..MbInfo::default()
        };
    }
}

impl<P: Pix> Drop for Work<P> {
    fn drop(&mut self) {
        // Never leave readers waiting, even when decoding stopped early.
        self.close();
        let b = Bufs {
            y: std::mem::take(&mut self.y),
            cb: std::mem::take(&mut self.cb),
            cr: std::mem::take(&mut self.cr),
            mbs: std::mem::take(&mut self.mbs),
        };
        self.pool.put((self.wmb, self.hmb, self.fmt), b);
    }
}

fn motion_of(m: &MbInfo) -> MbMotion {
    if m.intra() {
        return MbMotion {
            intra: true,
            field: m.field,
            ref_idx: [-1; 4],
            ..MbMotion::default()
        };
    }
    let mut out = MbMotion {
        field: m.field,
        ..MbMotion::default()
    };
    for b8 in 0..4 {
        let l = if m.ref_idx[0][b8] >= 0 { 0 } else { 1 };
        out.ref_idx[b8] = m.ref_idx[l][b8];
        out.ref_id[b8] = m.ref_id[l][b8];
        for i in 0..4 {
            let blk = B8_BLKS[b8][i];
            out.mv[blk] = m.mv[l][blk];
        }
    }
    out
}

/// Raster 4x4 blocks of each 8x8 partition.
const B8_BLKS: [[usize; 4]; 4] = [[0, 1, 4, 5], [2, 3, 6, 7], [8, 9, 12, 13], [10, 11, 14, 15]];

#[inline(always)]
fn b8_of(blk: usize) -> usize {
    (blk >> 3) * 2 + ((blk & 3) >> 1)
}

/// Decodes the whole picture; returns the number of errors concealed.
pub fn decode_picture(job: &PicJob) -> u32 {
    if job.shared.fmt.wide() {
        decode_picture_as::<u16>(job)
    } else {
        decode_picture_as::<u8>(job)
    }
}

fn decode_picture_as<P: Pix>(job: &PicJob) -> u32 {
    let mut w = Work::<P>::new(job);
    let total = w.wmb * w.hmb;
    let mut next = 0usize;
    let mut errors = 0;
    let src_ref = job.slices.iter().find_map(|s| s.lists[0].first());
    let src = src_ref.map(RefPic::view);
    if job.sg_map.is_some() {
        return decode_scattered(&mut w, job, src);
    }
    // Slices may come in any order (ASO); each depends only on itself.
    let mut slices: Vec<&SliceJob> = job.slices.iter().collect();
    slices.sort_by_key(|s| s.hdr.first_mb);
    for sj in slices {
        let first = sj.hdr.first_mb as usize * (1 + w.mbaff as usize);
        if first < next || first >= total || w.db.len() >= u16::MAX as usize {
            errors += 1;
            continue;
        }
        if next < first {
            errors += 1;
        }
        while next < first {
            w.conceal(next, src);
            w.mb_done(next);
            next += 1;
        }
        let num = w.db.len() as u16;
        w.db.push(DbParams {
            idc: sj.hdr.disable_deblocking,
            alpha: sj.hdr.alpha_offset,
            beta: sj.hdr.beta_offset,
        });
        match decode_slice(&mut w, job, sj, num) {
            Ok(end) => next = end,
            Err(end) => {
                next = end;
                errors += 1;
            }
        }
    }
    if next < total {
        errors += 1;
    }
    while next < total {
        w.conceal(next, src);
        w.mb_done(next);
        next += 1;
    }
    w.finish();
    errors
}

/// Decodes a picture with slice groups (FMO): each slice walks its group's macroblocks;
/// macroblocks no slice covered are concealed, then the whole picture is deblocked.
fn decode_scattered<P: Pix>(w: &mut Work<P>, job: &PicJob, src: Option<View>) -> u32 {
    let total = w.wmb * w.hmb;
    let mut errors = 0;
    for sj in &job.slices {
        let first = sj.hdr.first_mb as usize * (1 + w.mbaff as usize);
        if first >= total || w.mbs[w.storage(first)].slice != 0 || w.db.len() >= u16::MAX as usize {
            errors += 1;
            continue;
        }
        let num = w.db.len() as u16;
        w.db.push(DbParams {
            idc: sj.hdr.disable_deblocking,
            alpha: sj.hdr.alpha_offset,
            beta: sj.hdr.beta_offset,
        });
        if let Err(end) = decode_slice(w, job, sj, num) {
            errors += 1;
            // The macroblock that failed is concealed below with the ones never reached.
            if end < total {
                let s = w.storage(end);
                w.mbs[s].slice = 0;
            }
        }
    }
    let mut missing = false;
    for a in 0..total {
        if w.mbs[w.storage(a)].slice == 0 {
            missing = true;
            w.conceal(a, src);
        }
    }
    errors += missing as u32;
    w.finish();
    errors
}

/// Decodes one slice; returns the address after its last complete macroblock (`Err` when
/// the data was damaged).
fn decode_slice<P: Pix>(
    w: &mut Work<P>,
    job: &PicJob,
    sj: &SliceJob,
    slice: u16,
) -> Result<usize, usize> {
    let h = &sj.hdr;
    let data = &sj.rbsp[..(sj.len + PAD).min(sj.rbsp.len())];
    let total = w.wmb * w.hmb;
    let mbaff = w.mbaff;
    // Field macroblocks of an MBAFF frame use lists of fields (8.4.2.1): each frame splits
    // into the field of the macroblock's parity, then the other one.
    let field_lists: [[Vec<RefPic>; 2]; 2] = if mbaff {
        std::array::from_fn(|par| std::array::from_fn(|l| split_fields(&sj.lists[l], par)))
    } else {
        Default::default()
    };
    let mut sd = Sd::new(w, job, sj, slice, &field_lists);
    let mut addr = h.first_mb as usize * (1 + mbaff as usize);
    if sj.pps.cabac {
        let pos = h.data_bit.div_ceil(8);
        let mut e = CabacE {
            c: Cabac::new(data, pos),
            len: sj.len,
        };
        let set = if h.slice_type == SliceType::I {
            0
        } else {
            1 + h.cabac_init_idc as usize
        };
        e.c.init_contexts(set, h.qp, job.sps.chroma_format_idc == 3);
        // The bottom macroblock's mb_skip_flag, read ahead with a skipped top macroblock.
        let mut bot_skip: Option<bool> = None;
        loop {
            if addr >= total {
                return Err(addr);
            }
            let skip = if !mbaff {
                sd.start_mb(addr);
                h.slice_type != SliceType::I && e.skip_flag(&sd)
            } else if addr & 1 == 1 {
                sd.start_mb(addr);
                match bot_skip.take() {
                    Some(s) => s,
                    None => h.slice_type != SliceType::I && e.skip_flag(&sd),
                }
            } else {
                sd.field = sd.infer_field(addr);
                sd.start_mb(addr);
                if h.slice_type == SliceType::I || !e.skip_flag(&sd) {
                    sd.field = e.field_flag(&sd);
                    sd.start_mb(addr);
                    false
                } else {
                    // A skipped top macroblock takes the field flag of its pair, which comes
                    // with the bottom macroblock when that one is not skipped.
                    sd.cur().kind = INTER | SKIP;
                    sd.start_mb(addr + 1);
                    let bs = e.skip_flag(&sd);
                    if !bs {
                        sd.field = e.field_flag(&sd);
                    }
                    bot_skip = Some(bs);
                    sd.start_mb(addr);
                    true
                }
            };
            let ok = if skip {
                sd.skip_mb();
                Ok(())
            } else {
                sd.macroblock(&mut e)
            };
            if ok.is_err() || e.c.overrun() {
                return Err(addr);
            }
            sd.w.mb_done(addr);
            addr = sd.w.next_addr(addr);
            if mbaff && addr & 1 == 1 {
                continue;
            }
            if e.c.terminate() {
                return Ok(addr);
            }
        }
    } else {
        let stop = rbsp_end(data, sj.len);
        fn part(p: &Option<Partition>) -> Option<BitReader<'_>> {
            p.as_ref().map(|p| {
                let mut r = BitReader::new(&p.rbsp[..(p.len + PAD).min(p.rbsp.len())], p.len);
                r.set_pos(p.data_bit);
                r
            })
        }
        let mut e = Cavlc {
            r: BitReader::new(data, sj.len),
            parts: sj
                .slice_id
                .map(|_| [part(&sj.parts[0]), part(&sj.parts[1])]),
        };
        e.r.set_pos(h.data_bit);
        loop {
            if h.slice_type != SliceType::I {
                let run = e.r.ue() as usize;
                if run > total - addr {
                    return Err(addr);
                }
                // A bottom macroblock after a skipped top one carries the pair's field flag,
                // which the skipped top macroblock needs: read it ahead.
                let mut ahead = None;
                if mbaff && run > 0 && (addr + run) & 1 == 1 && e.r.more_rbsp_data(stop) {
                    ahead = Some(e.r.flag());
                }
                for i in 0..run {
                    if mbaff && addr & 1 == 0 {
                        sd.field = match ahead {
                            Some(f) if i + 1 == run => f,
                            _ => sd.infer_field(addr),
                        };
                    }
                    if addr >= total {
                        return Err(addr);
                    }
                    sd.start_mb(addr);
                    sd.skip_mb();
                    sd.w.mb_done(addr);
                    addr = sd.w.next_addr(addr);
                }
                if run > 0 && !e.r.more_rbsp_data(stop) {
                    return Ok(addr);
                }
            }
            if addr >= total {
                return Err(addr);
            }
            if mbaff && addr & 1 == 0 {
                sd.field = e.r.flag();
            }
            sd.start_mb(addr);
            if sd.macroblock(&mut e).is_err() || e.r.overrun() {
                return Err(addr);
            }
            sd.w.mb_done(addr);
            addr = sd.w.next_addr(addr);
            if !e.r.more_rbsp_data(stop) {
                return Ok(addr);
            }
        }
    }
}

/// Vertical motion vector scaling between the co-located and the current macroblock.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VScale {
    One,
    FrmToFld,
    FldToFrm,
}

/// The co-located macroblock (two frame macroblocks for Frm_To_Fld, by yCol / 8).
#[derive(Clone, Copy)]
struct Col {
    m: [MbMotion; 2],
    scale: VScale,
    /// Fld_To_Frm: yM of the upper half.
    ybase: usize,
}

/// A neighbouring partition's motion for one list.
#[derive(Clone, Copy)]
struct Nb {
    avail: bool,
    refi: i8,
    mv: [i16; 2],
}

const NB_NONE: Nb = Nb {
    avail: false,
    refi: -1,
    mv: [0, 0],
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Other,
    H16x8,
    V8x16,
}

/// Macroblock types after mb_type: the intra ones, or an inter partitioning.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MbT {
    INxN,
    I16 {
        pred: u8,
        cbp: u8,
    },
    Pcm,
    /// Partitioning (0 16x16, 1 16x8, 2 8x16) with prediction flags per partition.
    Inter {
        part: u8,
        pred: [u8; 2],
    },
    /// P_8x8 / P_8x8ref0 / B_8x8.
    Inter8x8 {
        ref0: bool,
    },
    BDirect,
}

/// Prediction flags (1 L0, 2 L1, 3 Bi) of the B_16x8/8x16 types 4..=21.
const B_PAIRS: [[u8; 2]; 9] = [
    [1, 1],
    [2, 2],
    [1, 2],
    [2, 1],
    [1, 3],
    [2, 3],
    [3, 1],
    [3, 2],
    [3, 3],
];

/// B sub_mb_type: (prediction flags, shape 0 8x8 / 1 8x4 / 2 4x8 / 3 4x4); 0 is direct.
const B_SUB: [(u8, u8); 13] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (1, 1),
    (1, 2),
    (2, 1),
    (2, 2),
    (3, 1),
    (3, 2),
    (1, 3),
    (2, 3),
    (3, 3),
];

/// Sub-partitions per shape: (x4, y4, w4, h4) within the 8x8.
const SUB_PARTS: [&[(usize, usize, usize, usize)]; 4] = [
    &[(0, 0, 2, 2)],
    &[(0, 0, 2, 1), (0, 1, 2, 1)],
    &[(0, 0, 1, 2), (1, 0, 1, 2)],
    &[(0, 0, 1, 1), (1, 0, 1, 1), (0, 1, 1, 1), (1, 1, 1, 1)],
];

struct Sd<'a, P: Pix> {
    w: &'a mut Work<P>,
    sps: &'a Sps,
    pps: &'a Pps,
    h: &'a SliceHeader,
    /// The reference lists of the current macroblock: the slice's, or for a field
    /// macroblock of an MBAFF frame the field lists of its parity.
    lists: &'a [Vec<RefPic>; 2],
    frame_lists: &'a [Vec<RefPic>; 2],
    field_lists: &'a [[Vec<RefPic>; 2]; 2],
    slice: u16,
    st: SliceType,
    wmb: usize,
    /// PicOrderCnt of the current macroblock's frame or field.
    poc: i32,
    frame_poc: i32,
    /// TopFieldOrderCnt and BottomFieldOrderCnt of the picture.
    pocs: [i32; 2],
    mbaff: bool,
    /// num_ref_idx_active per list, doubled for field macroblocks of an MBAFF frame.
    nref: [usize; 2],
    /// QP_Y (negative values at high bit depths).
    qp: i32,
    prev_dqp_nz: bool,
    /// 4:0:0: no chroma syntax; the chroma planes stay mid-grey.
    mono: bool,
    /// 4:4:4: Cb and Cr are coded and predicted like luma (ChromaArrayType 3).
    c444: bool,
    /// TransformBypassModeFlag of the current macroblock (lossless: residuals are the
    /// levels themselves).
    bypass: bool,
    /// Chroma samples per macroblock line: 8, or 16 in 4:4:4.
    mbcw: usize,
    fmt: Format,
    /// QpBdOffsetY and QpBdOffsetC.
    qoff: [i32; 2],
    /// Largest luma and chroma sample values.
    max: [i32; 2],
    /// Chroma lines per macroblock: 8 (4:2:0) or 16 (4:2:2).
    mbch: usize,
    /// Implicit bi-prediction weights (w0, w1) by refIdxL0 * n1 + refIdxL1: for frame
    /// macroblocks, then for top and bottom field macroblocks of an MBAFF frame (`imp`).
    implicit: [Vec<(i32, i32)>; 3],
    imp: usize,
    /// Luma 4x4 blocks 0..256, then chroma blocks at 256 + 16 * (component * 8 + block);
    /// in 4:4:4 the 4x4 blocks of plane c (0 = Y, 1 = Cb, 2 = Cr) at 256 * c + 16 * block.
    coef: [i32; 768],
    /// Luma 4x4 (raster) bits 0..16 and chroma blocks bits 16 + component * 8 + block
    /// holding coefficients; in 4:4:4 bit 16 * c + block.
    coded: u64,
    /// 8x8 blocks holding coefficients (transform_size_8x8_flag): bit 4 * c + block.
    coded8: u16,
    /// Storage index of the current macroblock (`Work::storage`).
    addr: usize,
    mbx: usize,
    /// Storage row.
    mby: usize,
    /// MBAFF: pair row, and 1 for the bottom macroblock of the pair.
    pr: usize,
    bot: usize,
    /// Luma / chroma sample offset and line stride of the macroblock, and its top luma line
    /// in its frame or field.
    lo: usize,
    lst: usize,
    co: usize,
    cst: usize,
    ypix: usize,
    /// MBAFF: the left and above macroblock pairs (their top macroblocks).
    pa: Option<usize>,
    pb: Option<usize>,
    /// MBAFF: the pairs around the current one, by row (0: the pair row above, 1: the
    /// current one) and column offset + 1: top macroblock | `PAIR_FIELD` when a field pair,
    /// or `PAIR_NONE` when unavailable (set by `start_mbaff` for `nb_px`).
    pairs: [[u32; 3]; 2],
    na: Option<usize>,
    nb: Option<usize>,
    nc: Option<usize>,
    nd: Option<usize>,
    spatial: Option<([i8; 2], [[i16; 2]; 2], bool)>,
    col: Option<Col>,
    /// The current macroblock is a field macroblock, of parity `par`.
    field: bool,
    par: u8,
    scan4: &'static [u8; 16],
    scan8: &'static [u8; 64],
}

#[inline(always)]
fn dq(v: i64) -> i32 {
    v.clamp(-(1 << 20), 1 << 20) as i32
}

#[inline(always)]
fn deq4(l: i32, ls: i32, qp: i32) -> i32 {
    let v = l as i64 * ls as i64;
    let s = qp / 6;
    dq(if s >= 4 {
        v << (s - 4)
    } else {
        (v + (1 << (3 - s))) >> (4 - s)
    })
}

#[inline(always)]
fn deq8(l: i32, ls: i32, qp: i32) -> i32 {
    let v = l as i64 * ls as i64;
    let s = qp / 6;
    dq(if s >= 6 {
        v << (s - 6)
    } else {
        (v + (1 << (5 - s))) >> (6 - s)
    })
}

const PAIR_FIELD: u32 = 1 << 31;
const PAIR_NONE: u32 = u32::MAX;

fn min_positive(a: i8, b: i8) -> i8 {
    if a >= 0 && b >= 0 { a.min(b) } else { a.max(b) }
}

fn median(a: i16, b: i16, c: i16) -> i16 {
    a.max(b).min(a.min(b).max(c))
}

impl<'a, P: Pix> Sd<'a, P> {
    fn new(
        w: &'a mut Work<P>,
        job: &'a PicJob,
        sj: &'a SliceJob,
        slice: u16,
        field_lists: &'a [[Vec<RefPic>; 2]; 2],
    ) -> Sd<'a, P> {
        let h = &sj.hdr;
        let wmb = w.wmb;
        let mbaff = w.mbaff;
        let fmt = w.fmt;
        let mbch = w.mbch;
        let mbcw = w.mbcw;
        let nref = [h.num_ref_idx[0] as usize, h.num_ref_idx[1] as usize];
        let mut sd = Sd {
            w,
            sps: &job.sps,
            pps: &sj.pps,
            h,
            lists: &sj.lists,
            frame_lists: &sj.lists,
            field_lists,
            slice,
            st: h.slice_type,
            wmb,
            poc: job.poc,
            frame_poc: job.poc,
            pocs: job.pocs,
            mbaff,
            nref,
            qp: h.qp,
            prev_dqp_nz: false,
            mono: job.sps.chroma_format_idc == 0,
            c444: job.sps.chroma_format_idc == 3,
            bypass: false,
            mbcw,
            fmt,
            qoff: [6 * (fmt.depth[0] as i32 - 8), 6 * (fmt.depth[1] as i32 - 8)],
            max: [fmt.max(0), fmt.max(1)],
            mbch,
            implicit: Default::default(),
            imp: 0,
            coef: [0; 768],
            coded: 0,
            coded8: 0,
            addr: 0,
            mbx: 0,
            mby: 0,
            pr: 0,
            bot: 0,
            lo: 0,
            lst: wmb * 16,
            co: 0,
            cst: wmb * mbcw,
            ypix: 0,
            pa: None,
            pb: None,
            pairs: [[PAIR_NONE; 3]; 2],
            na: None,
            nb: None,
            nc: None,
            nd: None,
            spatial: None,
            col: None,
            field: job.parity.is_some(),
            par: job.parity.unwrap_or(0),
            scan4: &ZIGZAG4,
            scan8: &ZIGZAG8,
        };
        if sd.field {
            sd.scan4 = &FIELD4;
            sd.scan8 = &FIELD8;
        }
        if h.slice_type == SliceType::B && sj.pps.weighted_bipred_idc == 2 {
            let table = |poc: i32, l: &[Vec<RefPic>; 2]| {
                let mut t = Vec::with_capacity(l[0].len() * l[1].len());
                for r0 in &l[0] {
                    for r1 in &l[1] {
                        t.push(implicit_weights(poc, r0, r1));
                    }
                }
                t
            };
            sd.implicit[0] = table(job.poc, &sj.lists);
            if mbaff {
                for par in 0..2 {
                    sd.implicit[1 + par] = table(job.pocs[par], &field_lists[par]);
                }
            }
        }
        sd
    }

    /// The top macroblock of pair (x, pr) of an MBAFF frame, when in the current slice.
    #[inline]
    fn pair_top(&self, x: usize, pr: usize) -> Option<usize> {
        let t = 2 * pr * self.wmb + x;
        (self.w.mbs[t].slice == self.slice).then_some(t)
    }

    /// The inferred mb_field_decoding_flag of the pair of MBAFF address `addr` (7.4.4): that
    /// of the left pair, else of the above pair, else frame.
    fn infer_field(&self, addr: usize) -> bool {
        let pair = addr / 2;
        let (x, pr) = (pair % self.wmb, pair / self.wmb);
        let left = if x > 0 {
            self.pair_top(x - 1, pr)
        } else {
            None
        };
        let above = if pr > 0 {
            self.pair_top(x, pr - 1)
        } else {
            None
        };
        left.or(above).is_some_and(|m| self.mb(m).field)
    }

    /// Starts macroblock `addr` (an MBAFF address in MBAFF frames, where `field` must be
    /// set to the pair's mb_field_decoding_flag first).
    fn start_mb(&mut self, addr: usize) {
        if self.mbaff {
            self.start_mbaff(addr);
            return;
        }
        self.addr = addr;
        self.mbx = addr % self.wmb;
        self.mby = addr / self.wmb;
        let (ls, cs) = (self.wmb * 16, self.wmb * self.mbcw);
        self.lo = self.mby * 16 * ls + self.mbx * 16;
        self.co = self.mby * self.mbch * cs + self.mbx * self.mbcw;
        self.ypix = self.mby * 16;
        let s = self.slice;
        let mbs = &self.w.mbs;
        let same = |a: usize| (mbs[a].slice == s).then_some(a);
        let (x, y, w) = (self.mbx, self.mby, self.wmb);
        self.na = if x > 0 { same(addr - 1) } else { None };
        self.nb = if y > 0 { same(addr - w) } else { None };
        self.nc = if y > 0 && x + 1 < w {
            same(addr - w + 1)
        } else {
            None
        };
        self.nd = if y > 0 && x > 0 {
            same(addr - w - 1)
        } else {
            None
        };
        self.w.mbs[addr] = MbInfo {
            slice: s,
            field: self.field,
            ..MbInfo::default()
        };
        self.spatial = None;
        self.col = None;
        self.coded = 0;
        self.coded8 = 0;
    }

    fn start_mbaff(&mut self, addr: usize) {
        let w = self.wmb;
        let (ls, cs) = (w * 16, w * self.mbcw);
        let pair = addr / 2;
        let bot = addr & 1;
        let (mbx, pr) = (pair % w, pair / w);
        let st = (2 * pr + bot) * w + mbx;
        self.addr = st;
        self.mbx = mbx;
        self.mby = 2 * pr + bot;
        self.pr = pr;
        self.bot = bot;
        let field = self.field;
        let ch = self.mbch;
        if field {
            self.lo = (pr * 32 + bot) * ls + mbx * 16;
            self.lst = 2 * ls;
            self.co = (pr * 2 * ch + bot) * cs + mbx * self.mbcw;
            self.cst = 2 * cs;
            self.ypix = pr * 16;
            self.lists = &self.field_lists[bot];
            self.poc = self.pocs[bot];
            self.imp = 1 + bot;
            self.scan4 = &FIELD4;
            self.scan8 = &FIELD8;
            self.par = bot as u8;
            for l in 0..2 {
                self.nref[l] = 2 * self.h.num_ref_idx[l] as usize;
            }
        } else {
            self.lo = (pr * 32 + bot * 16) * ls + mbx * 16;
            self.lst = ls;
            self.co = (pr * 2 * ch + bot * ch) * cs + mbx * self.mbcw;
            self.cst = cs;
            self.ypix = (2 * pr + bot) * 16;
            self.lists = self.frame_lists;
            self.poc = self.frame_poc;
            self.imp = 0;
            self.scan4 = &ZIGZAG4;
            self.scan8 = &ZIGZAG8;
            self.par = 0;
            for l in 0..2 {
                self.nref[l] = self.h.num_ref_idx[l] as usize;
            }
        }
        self.pa = if mbx > 0 {
            self.pair_top(mbx - 1, pr)
        } else {
            None
        };
        self.pb = if pr > 0 {
            self.pair_top(mbx, pr - 1)
        } else {
            None
        };
        self.w.mbs[st] = MbInfo {
            slice: self.slice,
            field,
            ..MbInfo::default()
        };
        let pair = |t: Option<usize>| match t {
            Some(t) => t as u32 | if self.mb(t).field { PAIR_FIELD } else { 0 },
            None => PAIR_NONE,
        };
        let above = |dx: usize| {
            if pr > 0 && mbx + dx >= 1 && mbx + dx <= w {
                self.pair_top(mbx + dx - 1, pr - 1)
            } else {
                None
            }
        };
        // The current pair is looked at only from its bottom macroblock.
        let cur = (st - bot * w) as u32 | if field { PAIR_FIELD } else { 0 };
        self.pairs = [
            [pair(above(0)), pair(self.pb), pair(above(2))],
            [pair(self.pa), cur, PAIR_NONE],
        ];
        self.na = self.nb_px(-1, 0, 16, 16).map(|n| n.0);
        self.nb = self.nb_px(0, -1, 16, 16).map(|n| n.0);
        self.nc = self.nb_px(16, -1, 16, 16).map(|n| n.0);
        self.nd = self.nb_px(-1, -1, 16, 16).map(|n| n.0);
        self.spatial = None;
        self.col = None;
        self.coded = 0;
        self.coded8 = 0;
    }

    /// The neighbouring location (xn, yn) relative to the current macroblock, in a block of
    /// maxw x maxh samples (16x16 luma, 8x8 chroma): (macroblock, xW, yW) (6.4.12, with
    /// Table 6-4 for MBAFF frames).
    #[inline]
    fn nb_px(&self, xn: i32, yn: i32, maxw: i32, maxh: i32) -> Option<(usize, i32, i32)> {
        if yn >= maxh || (xn >= maxw && yn >= 0) {
            return None;
        }
        if xn >= 0 && xn < maxw && yn >= 0 {
            return Some((self.addr, xn, yn));
        }
        let xw = xn.rem_euclid(maxw);
        if !self.mbaff {
            let m = if yn >= 0 {
                self.na
            } else if xn < 0 {
                self.nd
            } else if xn < maxw {
                self.nb
            } else {
                self.nc
            }?;
            return Some((m, xw, yn.rem_euclid(maxh)));
        }
        // MBAFF: the line in the current pair's frame lines (negative: the pair above),
        // then the macroblock of the neighbouring pair holding that line.
        let bot = self.bot as i32;
        let mut fy = if self.field {
            2 * yn + bot
        } else {
            bot * maxh + yn
        };
        let dx = if xn < 0 {
            0
        } else if xn >= maxw {
            2
        } else {
            1
        };
        let row = (fy >= 0) as usize;
        if fy < 0 {
            fy += 2 * maxh;
        }
        // The pair to the right in the current pair row is not decoded yet (PAIR_NONE).
        let e = self.pairs[row][dx];
        if e == PAIR_NONE {
            return None;
        }
        let top = (e & !PAIR_FIELD) as usize;
        Some(if e & PAIR_FIELD != 0 {
            (top + (fy & 1) as usize * self.wmb, xw, fy >> 1)
        } else {
            (top + (fy >= maxh) as usize * self.wmb, xw, fy % maxh)
        })
    }

    #[inline(always)]
    fn cur(&mut self) -> &mut MbInfo {
        &mut self.w.mbs[self.addr]
    }

    #[inline(always)]
    fn mb(&self, a: usize) -> &MbInfo {
        &self.w.mbs[a]
    }

    fn set_qp_info(&mut self, qp: i32) {
        let off = self.pps.chroma_qp_offset;
        let oc = self.qoff[1];
        let m = self.cur();
        m.qp = qp as i8;
        m.qpc = [qpc(qp + off[0], oc) as i8, qpc(qp + off[1], oc) as i8];
    }

    /// QP'_C of chroma component `comp` (with the bit-depth offset).
    fn qpc_prime(&self, comp: usize) -> i32 {
        let oc = self.qoff[1];
        qpc(self.qp + self.pps.chroma_qp_offset[comp], oc) + oc
    }

    /// The luma 4x4 block holding sample (xn, yn) of an MBAFF frame, relative to the current
    /// macroblock: (macroblock, raster block).
    fn nb4_px(&self, xn: i32, yn: i32) -> Option<(usize, usize)> {
        let (m, xw, yw) = self.nb_px(xn, yn, 16, 16)?;
        Some((m, ((yw / 4) * 4 + xw / 4) as usize))
    }

    /// Luma 4x4 block at (x4, y4) relative to the current macroblock: (macroblock, raster
    /// block), for left/above/above-left/above-right neighbours.
    #[inline]
    fn nb4(&self, x4: i32, y4: i32) -> Option<(usize, usize)> {
        if self.mbaff {
            let xn = if x4 < 0 { -1 } else { x4 * 4 };
            let yn = if y4 < 0 { -1 } else { y4 * 4 };
            let (m, xw, yw) = self.nb_px(xn, yn, 16, 16)?;
            return Some((m, ((yw / 4) * 4 + xw / 4) as usize));
        }
        if y4 >= 0 {
            if x4 >= 4 || y4 >= 4 {
                return None;
            }
            if x4 >= 0 {
                return Some((self.addr, (y4 * 4 + x4) as usize));
            }
            return self.na.map(|m| (m, (y4 * 4 + 3) as usize));
        }
        if x4 < 0 {
            self.nd.map(|m| (m, 15))
        } else if x4 < 4 {
            self.nb.map(|m| (m, (12 + x4) as usize))
        } else {
            self.nc.map(|m| (m, 12))
        }
    }

    /// Chroma 4x4 block (x2, y2) in the grid of 2 columns and 2 (4:2:0) or 4 (4:2:2) rows:
    /// (macroblock, block in raster order).
    #[inline]
    fn nbc(&self, x2: i32, y2: i32) -> Option<(usize, usize)> {
        if self.mbaff {
            let xn = if x2 < 0 { -1 } else { x2 * 4 };
            let yn = if y2 < 0 { -1 } else { y2 * 4 };
            let (m, xw, yw) = self.nb_px(xn, yn, 8, self.mbch as i32)?;
            return Some((m, ((yw / 4) * 2 + xw / 4) as usize));
        }
        if x2 >= 0 && y2 >= 0 {
            Some((self.addr, (y2 * 2 + x2) as usize))
        } else if x2 < 0 {
            self.na.map(|m| (m, (y2 * 2 + 1) as usize))
        } else {
            let rows = (self.mbch / 4) as i32;
            self.nb.map(|m| (m, ((rows - 1) * 2 + x2) as usize))
        }
    }

    /// Motion of the partition covering 4x4 position (x4, y4) for `list`.
    #[inline]
    fn nb_mv(&self, list: usize, x4: i32, y4: i32) -> Nb {
        self.nb_mv_at(list, self.nb4(x4, y4))
    }

    /// Motion of the neighbouring 4x4 block `n` (from `nb4`), scaled to the current
    /// macroblock's frame/field kind.
    fn nb_mv_at(&self, list: usize, n: Option<(usize, usize)>) -> Nb {
        match n {
            None => NB_NONE,
            Some((m, blk)) => {
                let mi = self.mb(m);
                if mi.intra() {
                    return Nb {
                        avail: true,
                        refi: -1,
                        mv: [0, 0],
                    };
                }
                let mut r = mi.ref_idx[list][b8_of(blk)];
                let mut mv = if r >= 0 { mi.mv[list][blk] } else { [0, 0] };
                // 8.4.1.3.1: frame and field neighbours in MBAFF frames.
                if self.mbaff && r >= 0 && mi.field != self.field {
                    if self.field {
                        mv[1] /= 2;
                        r = r.saturating_mul(2);
                    } else {
                        mv[1] = mv[1].wrapping_mul(2);
                        r >>= 1;
                    }
                }
                Nb {
                    avail: true,
                    refi: r,
                    mv,
                }
            }
        }
    }

    /// Neighbours A, B and C (or D) of a partition (8.4.1.3.2).
    fn neighbours(&self, list: usize, x4: usize, y4: usize, w4: usize) -> [Nb; 3] {
        let (x, y) = (x4 as i32, y4 as i32);
        let a = self.nb_mv(list, x - 1, y);
        let b = self.nb_mv(list, x, y - 1);
        let cx = x + w4 as i32;
        let c_inside = cx < 4 && y >= 1;
        let c_ok = if c_inside {
            RASTER_TO_BLK[((y - 1) * 4 + cx) as usize] < RASTER_TO_BLK[y4 * 4 + x4]
        } else {
            true
        };
        let mut c = if c_ok {
            self.nb_mv(list, cx, y - 1)
        } else {
            NB_NONE
        };
        if !c.avail {
            // D is the sample above-left of the partition: in an MBAFF frame, line y - 1 of
            // a left pair can be in the other macroblock than line y (Table 6-4).
            c = if self.mbaff && x == 0 && y > 0 {
                self.nb_mv_at(list, self.nb4_px(-1, 4 * y - 1))
            } else {
                self.nb_mv(list, x - 1, y - 1)
            };
        }
        [a, b, c]
    }

    fn pred_mv(
        &self,
        list: usize,
        x4: usize,
        y4: usize,
        w4: usize,
        refi: i8,
        shape: Shape,
    ) -> [i16; 2] {
        let [a, mut b, mut c] = self.neighbours(list, x4, y4, w4);
        match shape {
            Shape::H16x8 => {
                if y4 == 0 && b.refi == refi {
                    return b.mv;
                }
                if y4 != 0 && a.refi == refi {
                    return a.mv;
                }
            }
            Shape::V8x16 => {
                if x4 == 0 && a.refi == refi {
                    return a.mv;
                }
                if x4 != 0 && c.refi == refi {
                    return c.mv;
                }
            }
            Shape::Other => {}
        }
        if !b.avail && !c.avail && a.avail {
            b = a;
            c = a;
        }
        let m = [a.refi == refi, b.refi == refi, c.refi == refi];
        match m {
            [true, false, false] => a.mv,
            [false, true, false] => b.mv,
            [false, false, true] => c.mv,
            _ => [
                median(a.mv[0], b.mv[0], c.mv[0]),
                median(a.mv[1], b.mv[1], c.mv[1]),
            ],
        }
    }

    fn set_ref(&mut self, list: usize, b8: usize, refi: i8) {
        let id = if refi >= 0 {
            self.lists[list].get(refi as usize).map_or(0, |r| r.id)
        } else {
            0
        };
        let m = self.cur();
        m.ref_idx[list][b8] = refi;
        m.ref_id[list][b8] = id;
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_mv(
        &mut self,
        list: usize,
        x4: usize,
        y4: usize,
        w4: usize,
        h4: usize,
        mv: [i16; 2],
        mvd: [u8; 2],
    ) {
        let m = self.cur();
        for y in y4..y4 + h4 {
            for x in x4..x4 + w4 {
                m.mv[list][y * 4 + x] = mv;
                m.mvd[list][y * 4 + x] = mvd;
            }
        }
    }

    fn skip_mb(&mut self) {
        let qp = self.qp;
        self.set_qp_info(qp);
        self.prev_dqp_nz = false;
        if self.st == SliceType::B {
            self.cur().kind = INTER | SKIP | DIRECT;
            self.cur().direct8 = 15;
            for b8 in 0..4 {
                self.direct8x8(b8);
            }
        } else {
            self.cur().kind = INTER | SKIP;
            let a = self.nb_mv(0, -1, 0);
            let b = self.nb_mv(0, 0, -1);
            let mv = if !a.avail
                || !b.avail
                || (a.refi == 0 && a.mv == [0, 0])
                || (b.refi == 0 && b.mv == [0, 0])
            {
                [0, 0]
            } else {
                self.pred_mv(0, 0, 0, 4, 0, Shape::Other)
            };
            for b8 in 0..4 {
                self.set_ref(0, b8, 0);
            }
            self.fill_mv(0, 0, 0, 4, 4, mv, [0, 0]);
        }
        self.mc_mb();
    }

    /// The co-located motion of the four blocks of partition `b8` (`col_at`, in the order
    /// of `B8_BLKS`) and the vertical scaling. The co-located macroblock is looked up once
    /// per macroblock and read in place.
    fn col_b8(&mut self, b8: usize) -> (VScale, [([i16; 2], i8, u32); 4]) {
        if self.col.is_none() {
            self.col = Some(self.col_motion());
        }
        let Some(col) = &self.col else { unreachable!() };
        (col.scale, B8_BLKS[b8].map(|blk| self.col_at(col, blk)))
    }

    /// The co-located macroblock(s) of the current one (8.4.1.2.1, Tables 8-6 and 8-8).
    fn col_motion(&self) -> Col {
        let one = |m: MbMotion| Col {
            m: [m; 2],
            scale: VScale::One,
            ybase: 0,
        };
        if self.mbaff {
            return self.col_mbaff();
        }
        let (mbx, mby) = (self.mbx, self.mby);
        match self.lists[1].first() {
            None => one(INTRA_MOTION),
            Some(r) => {
                let pic = &r.pic;
                match (self.field, r.parity) {
                    // A field picture: the field, or the frame containing it.
                    (true, Some(rp)) => {
                        if pic.structure == Structure::Field {
                            one(pic.motion_field(rp as usize, mbx, mby))
                        } else {
                            let top = pic.motion_frame(mbx, 2 * mby);
                            if top.field {
                                one(pic.motion_frame(mbx, 2 * mby + self.par as usize))
                            } else {
                                Col {
                                    m: [top, pic.motion_frame(mbx, 2 * mby + 1)],
                                    scale: VScale::FrmToFld,
                                    ybase: 0,
                                }
                            }
                        }
                    }
                    // A frame macroblock: the frame, or the field of a field pair closer in
                    // output order.
                    (false, None) => {
                        if pic.structure == Structure::Field {
                            let d = |p: i32| (p as i64 - self.poc as i64).abs();
                            let sel = (d(r.pocs[0]) >= d(r.pocs[1])) as usize;
                            Col {
                                m: [pic.motion_field(sel, mbx, mby / 2); 2],
                                scale: VScale::FldToFrm,
                                ybase: 8 * (mby % 2),
                            }
                        } else {
                            one(pic.motion_frame(mbx, mby))
                        }
                    }
                    _ => one(INTRA_MOTION),
                }
            }
        }
    }

    /// The co-located macroblock(s) in an MBAFF frame (Table 8-8, AFRM rows).
    fn col_mbaff(&self) -> Col {
        let one = |m: MbMotion| Col {
            m: [m; 2],
            scale: VScale::One,
            ybase: 0,
        };
        let Some(r) = self.frame_lists[1].first() else {
            return one(INTRA_MOTION);
        };
        let pic = &r.pic;
        let (mbx, pr, bot) = (self.mbx, self.pr, self.bot);
        let d = |p: i32| (p as i64 - self.frame_poc as i64).abs();
        // A frame macroblock with a field co-located: the field closer in output order.
        let sel = (d(r.pocs[0]) >= d(r.pocs[1])) as usize;
        if pic.structure == Structure::Field {
            return if self.field {
                one(pic.motion_field(bot, mbx, pr))
            } else {
                Col {
                    m: [pic.motion_field(sel, mbx, pr); 2],
                    scale: VScale::FldToFrm,
                    ybase: 8 * bot,
                }
            };
        }
        let x = pic.motion_frame(mbx, 2 * pr + bot);
        match (self.field, x.field) {
            (false, false) | (true, true) => one(x),
            (false, true) => Col {
                m: [pic.motion_frame(mbx, 2 * pr + sel); 2],
                scale: VScale::FldToFrm,
                ybase: 8 * bot,
            },
            (true, false) => Col {
                m: [
                    pic.motion_frame(mbx, 2 * pr),
                    pic.motion_frame(mbx, 2 * pr + 1),
                ],
                scale: VScale::FrmToFld,
                ybase: 0,
            },
        }
    }

    /// Motion of the co-located 4x4 block for raster block `blk`: vector, refIdxCol and
    /// the picture it referred to (intra gives refIdxCol -1).
    fn col_at(&self, col: &Col, blk: usize) -> ([i16; 2], i8, u32) {
        let (x4, y4) = (blk & 3, blk >> 2);
        let (xc, yc) = if self.sps.direct_8x8_inference {
            ((x4 >> 1) * 3, (y4 >> 1) * 3)
        } else {
            (x4, y4)
        };
        let ycol = 4 * yc;
        let (m, ym) = match col.scale {
            VScale::One => (&col.m[0], ycol),
            VScale::FrmToFld => (&col.m[ycol / 8], (2 * ycol) % 16),
            VScale::FldToFrm => (&col.m[0], col.ybase + 4 * (ycol / 8)),
        };
        if m.intra {
            return ([0, 0], -1, 0);
        }
        let b = (ym / 4) * 4 + xc;
        let b8 = b8_of(b);
        (m.mv[b], m.ref_idx[b8], m.ref_id[b8])
    }

    /// Direct prediction of 8x8 partition `b8` (8.4.1.2).
    fn direct8x8(&mut self, b8: usize) {
        let (scale, cols) = self.col_b8(b8);
        if self.h.direct_spatial {
            let (refs, mvp, zero) = match self.spatial {
                Some(s) => s,
                None => {
                    let mut refs = [-1i8; 2];
                    for (l, r) in refs.iter_mut().enumerate() {
                        let [a, b, c] = self.neighbours(l, 0, 0, 4);
                        *r = min_positive(a.refi, min_positive(b.refi, c.refi));
                    }
                    let zero = refs[0] < 0 && refs[1] < 0;
                    if zero {
                        refs = [0, 0];
                    }
                    let mut mvp = [[0i16; 2]; 2];
                    for l in 0..2 {
                        if refs[l] >= 0 && !zero {
                            mvp[l] = self.pred_mv(l, 0, 0, 4, refs[l], Shape::Other);
                        }
                    }
                    self.spatial = Some((refs, mvp, zero));
                    (refs, mvp, zero)
                }
            };
            let col_short = self.lists[1].first().is_some_and(|r| !r.long);
            for l in 0..2 {
                self.set_ref(l, b8, refs[l]);
            }
            for (&blk, &(cm, cref, _)) in B8_BLKS[b8].iter().zip(&cols) {
                let col_zero = col_short
                    && cref == 0
                    && (-1..=1).contains(&cm[0])
                    && (-1..=1).contains(&cm[1]);
                for l in 0..2 {
                    if refs[l] < 0 {
                        continue;
                    }
                    let mv = if zero || (refs[l] == 0 && col_zero) {
                        [0, 0]
                    } else {
                        mvp[l]
                    };
                    let m = self.cur();
                    m.mv[l][blk] = mv;
                    m.mvd[l][blk] = [0, 0];
                }
            }
        } else {
            let (_, cref, cid) = cols[0];
            let ref0 = if cref < 0 {
                0
            } else {
                // MapColToList0: the same picture, or the field of the current parity of a
                // frame, or the frame containing a field.
                let id = match scale {
                    VScale::One => cid,
                    VScale::FrmToFld => (cid & !3) | (1 + self.par as u32),
                    VScale::FldToFrm => cid & !3,
                };
                self.lists[0].iter().position(|r| r.id == id).unwrap_or(0) as i8
            };
            self.set_ref(0, b8, ref0);
            self.set_ref(1, b8, 0);
            let (p0, p1) = (self.lists[0].get(ref0 as usize), self.lists[1].first());
            for (&blk, &(mut mvcol, _, _)) in B8_BLKS[b8].iter().zip(&cols) {
                match scale {
                    VScale::One => {}
                    VScale::FrmToFld => mvcol[1] /= 2,
                    VScale::FldToFrm => mvcol[1] = mvcol[1].wrapping_mul(2),
                }
                let (mv0, mv1) = match (p0, p1) {
                    (Some(p0), Some(p1)) if !p0.long && p1.poc != p0.poc => {
                        let tb = poc_diff(self.poc, p0.poc);
                        let td = poc_diff(p1.poc, p0.poc);
                        let tx = (16384 + (td / 2).abs()) / td;
                        let dsf = ((tb * tx + 32) >> 6).clamp(-1024, 1023);
                        let s = |c: i16| ((dsf * c as i32 + 128) >> 8).clamp(-32768, 32767) as i16;
                        let m0 = [s(mvcol[0]), s(mvcol[1])];
                        (
                            m0,
                            [m0[0].wrapping_sub(mvcol[0]), m0[1].wrapping_sub(mvcol[1])],
                        )
                    }
                    _ => (mvcol, [0, 0]),
                };
                let m = self.cur();
                m.mv[0][blk] = mv0;
                m.mv[1][blk] = mv1;
                m.mvd[0][blk] = [0, 0];
                m.mvd[1][blk] = [0, 0];
            }
        }
    }

    fn weighting(&self, r0: i8, r1: i8) -> [Weighting; 3] {
        let explicit = match self.st {
            SliceType::P => self.pps.weighted_pred,
            SliceType::B => self.pps.weighted_bipred_idc == 1,
            SliceType::I => false,
        };
        if explicit && let Some(wt) = &self.h.weights {
            // refIdxWP: field macroblocks of MBAFF frames index the frame's weights.
            let sh = (self.mbaff && self.field) as u32;
            let get = |l: usize, r: i8| wt.l[l].get((r.max(0) as usize) >> sh).copied();
            let (w0, w1) = (get(0, r0), get(1, r1));
            let mut out = [Weighting::Default; 3];
            for (c, o) in out.iter_mut().enumerate() {
                let lwd = if c == 0 {
                    wt.luma_log2_denom
                } else {
                    wt.chroma_log2_denom
                };
                // Offsets are in 8-bit units (7.4.3.2).
                let sh = self.fmt.depth[(c > 0) as usize] as u32 - 8;
                let (a, oa) = w0.map_or((1 << lwd, 0), |w| (w.w[c], w.o[c] << sh));
                let (b, ob) = w1.map_or((1 << lwd, 0), |w| (w.w[c], w.o[c] << sh));
                *o = Weighting::Weighted(lwd, a, b, oa, ob);
            }
            return out;
        }
        if self.st == SliceType::B && self.pps.weighted_bipred_idc == 2 && r0 >= 0 && r1 >= 0 {
            let n1 = self.lists[1].len();
            if let Some(&(w0, w1)) = self.implicit[self.imp].get(r0 as usize * n1 + r1 as usize) {
                return [Weighting::Weighted(5, w0, w1, 0, 0); 3];
            }
        }
        [Weighting::Default; 3]
    }

    /// Inter prediction of a block in 4x4 units from the current macroblock's motion.
    fn mc_block(&mut self, x4: usize, y4: usize, w4: usize, h4: usize) {
        let blk = y4 * 4 + x4;
        let b8 = b8_of(blk);
        let m = self.mb(self.addr);
        let refs = [m.ref_idx[0][b8], m.ref_idx[1][b8]];
        let mvs = [m.mv[0][blk], m.mv[1][blk]];
        let (w, h) = (w4 * 4, h4 * 4);
        let px = (self.mbx * 16 + x4 * 4) as i32;
        let py = (self.ypix + y4 * 4) as i32;
        let cshift = self.fmt.cshift();
        let cxs = self.fmt.cxshift();
        let ch = h >> cshift;
        // Chroma width and column of the block; 4:4:4 predicts Cb and Cr like luma.
        let (cw, cx) = (w >> cxs, (x4 * 4) >> cxs);
        let c444 = self.c444;
        let lists = self.lists;
        let rps = [0, 1].map(|l| usize::try_from(refs[l]).ok().and_then(|r| lists[l].get(r)));
        if let [None, None] = rps {
            // A damaged stream referring to nothing: keep whatever is there.
            return;
        }
        let wt = self.weighting(refs[0], refs[1]);
        if let [Some(rp), None] | [None, Some(rp)] = rps
            && wt == [Weighting::Default; 3]
        {
            // One unweighted prediction: straight into the picture.
            let mv = mvs[rps[0].is_none() as usize];
            let (lo, co) = (
                self.lo + y4 * 4 * self.lst + x4 * 4,
                self.co + ((y4 * 4) >> cshift) * self.cst + cx,
            );
            let v = rp.view();
            let cmv = self.chroma_mv(mv, rp);
            let (cst, maxc) = (self.cst, self.max[1]);
            let w_ = &mut *self.w;
            let dst = &mut w_.y[lo..];
            inter::luma(v, px, py, mv, w, h, dst, self.lst, self.max[0], &mut w_.mcs);
            if c444 {
                for (c, pl) in [&mut w_.cb, &mut w_.cr].into_iter().enumerate() {
                    let dst = &mut pl[co..];
                    inter::luma_plane(v, 1 + c, px, py, mv, w, h, dst, cst, maxc, &mut w_.mcs);
                }
                return;
            }
            let dst = [&mut w_.cb[co..], &mut w_.cr[co..]];
            inter::chroma(
                v,
                px >> cxs,
                py >> cshift,
                cmv,
                cw,
                ch,
                dst,
                cst,
                &mut w_.mcs,
            );
            return;
        }
        let mut buf = std::mem::take(&mut self.w.mcb);
        let (pl, pc) = buf.split_at_mut(512);
        let (pl0, pl1) = pl.split_at_mut(256);
        let pl = [pl0, pl1];
        let (c0, c1) = pc.split_at_mut(512);
        let (c00, c01) = c0.split_at_mut(256);
        let (c10, c11) = c1.split_at_mut(256);
        let mut pc = [[c00, c01], [c10, c11]];
        // Stride of the chroma predictions.
        let cbs = 16 >> cxs;
        let used = rps.map(|r| r.is_some());
        for l in 0..2 {
            let Some(rp) = rps[l] else {
                continue;
            };
            let v = rp.view();
            inter::luma(
                v,
                px,
                py,
                mvs[l],
                w,
                h,
                pl[l],
                16,
                self.max[0],
                &mut self.w.mcs,
            );
            let [d0, d1] = &mut pc[l];
            if c444 {
                for (c, d) in [d0, d1].into_iter().enumerate() {
                    let (maxc, mcs) = (self.max[1], &mut self.w.mcs);
                    inter::luma_plane(v, 1 + c, px, py, mvs[l], w, h, d, 16, maxc, mcs);
                }
                continue;
            }
            let cmv = self.chroma_mv(mvs[l], rp);
            inter::chroma(
                v,
                px >> cxs,
                py >> cshift,
                cmv,
                cw,
                ch,
                [d0, d1],
                8,
                &mut self.w.mcs,
            );
        }
        let (ls, cs) = (self.lst, self.cst);
        let lo = self.lo + y4 * 4 * ls + x4 * 4;
        let co = self.co + ((y4 * 4) >> cshift) * cs + cx;
        fn p<P>(used: bool, s: &[P]) -> Option<&[P]> {
            used.then_some(s)
        }
        let [ml, mc] = self.max;
        inter::combine(
            &mut self.w.y[lo..],
            ls,
            p(used[0], pl[0]),
            p(used[1], pl[1]),
            16,
            w,
            h,
            wt[0],
            ml,
        );
        let (cb, cr) = (&mut self.w.cb, &mut self.w.cr);
        inter::combine(
            &mut cb[co..],
            cs,
            p(used[0], pc[0][0]),
            p(used[1], pc[1][0]),
            cbs,
            cw,
            ch,
            wt[1],
            mc,
        );
        inter::combine(
            &mut cr[co..],
            cs,
            p(used[0], pc[0][1]),
            p(used[1], pc[1][1]),
            cbs,
            cw,
            ch,
            wt[2],
            mc,
        );
        self.w.mcb = buf;
    }

    /// The chroma vector (eighth samples, 4:2:2 vertical in quarter samples) for luma vector
    /// `mv` into reference `rp`.
    fn chroma_mv(&self, mv: [i16; 2], rp: &RefPic) -> [i32; 2] {
        let mv = [mv[0] as i32, mv[1] as i32];
        if self.fmt.cshift() == 0 {
            // 4:2:2: full vertical chroma resolution, the vector in quarter samples.
            [mv[0], 2 * mv[1]]
        } else {
            // Table 8-9: a field of the other parity sits half a chroma line away.
            match (self.field, rp.parity) {
                (true, Some(rpar)) if rpar != self.par => {
                    [mv[0], mv[1] + if rpar == 1 { -2 } else { 2 }]
                }
                _ => mv,
            }
        }
    }

    /// Prefetches the reference samples of the current macroblock's first partition.
    fn prefetch_mc(&self) {
        let m = self.mb(self.addr);
        for l in 0..2 {
            let r = m.ref_idx[l][0];
            if r < 0 {
                continue;
            }
            if let Some(rp) = self.lists[l].get(r as usize) {
                let mv = m.mv[l][0];
                let x = (self.mbx * 16) as i32 + (mv[0] as i32 >> 2) - 2;
                let y = self.ypix as i32 + (mv[1] as i32 >> 2) - 2;
                rp.view().prefetch::<P>(x, y, 21, 21);
            }
        }
    }

    fn uniform(m: &MbInfo, blks: &[usize]) -> bool {
        let b0 = blks[0];
        let r = [m.ref_idx[0][b8_of(b0)], m.ref_idx[1][b8_of(b0)]];
        blks.iter().all(|&b| {
            let rb = [m.ref_idx[0][b8_of(b)], m.ref_idx[1][b8_of(b)]];
            rb == r && (0..2).all(|l| r[l] < 0 || m.mv[l][b] == m.mv[l][b0])
        })
    }

    fn mc_mb(&mut self) {
        // The partition shape first, so the macroblock is not copied out.
        let m = self.mb(self.addr);
        const ALL: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        if Self::uniform(m, &ALL) {
            self.mc_block(0, 0, 4, 4);
            return;
        }
        let whole = B8_BLKS.map(|b| Self::uniform(m, &b));
        for b8 in 0..4 {
            let (x4, y4) = (2 * (b8 & 1), 2 * (b8 >> 1));
            if whole[b8] {
                self.mc_block(x4, y4, 2, 2);
            } else {
                for i in 0..4 {
                    self.mc_block(x4 + (i & 1), y4 + (i >> 1), 1, 1);
                }
            }
        }
    }

    /// Intra availability of a neighbour (constrained_intra_pred hides inter macroblocks).
    fn ia(&self, n: Option<usize>) -> bool {
        n.is_some_and(|a| !self.pps.constrained_intra_pred || self.mb(a).intra())
    }

    /// Intra availability of the sample at (xn, yn) relative to the macroblock (block
    /// maxw x maxh).
    #[inline]
    fn pav(&self, xn: i32, yn: i32, maxw: i32, maxh: i32) -> bool {
        if xn >= 0 && xn < maxw && yn >= 0 && yn < maxh {
            return true;
        }
        self.ia(self.nb_px(xn, yn, maxw, maxh).map(|n| n.0))
    }

    /// Intra availability of the left samples of lines y0..y0 + h of a maxw x maxh block (in
    /// an MBAFF frame they can come from two macroblocks).
    fn lav(&self, y0: i32, h: i32, maxw: i32, maxh: i32) -> bool {
        if !self.mbaff {
            return self.ia(self.na);
        }
        (y0..y0 + h).all(|y| self.pav(-1, y, maxw, maxh))
    }

    fn pred_intra_mode(&self, x4: i32, y4: i32) -> u8 {
        let a = self.nb4(x4 - 1, y4);
        let b = self.nb4(x4, y4 - 1);
        let mode = |n: Option<(usize, usize)>| -> Option<u8> {
            let (m, blk) = n?;
            let mi = self.mb(m);
            if m != self.addr && !mi.intra() && self.pps.constrained_intra_pred {
                return None;
            }
            if mi.kind & (I4 | I8) == 0 {
                return Some(2);
            }
            Some(mi.ipred[blk])
        };
        // An Intra8x8 macroblock takes 4x4 block n of a neighbouring 8x8 block (8-72).
        let (a, b) = if self.mb(self.addr).kind & I8 != 0 {
            let fix = |n: Option<(usize, usize)>, k: usize| {
                n.map(|(m, blk)| {
                    if m == self.addr {
                        (m, blk)
                    } else {
                        (m, B8_BLKS[b8_of(blk)][k])
                    }
                })
            };
            let ka = match a {
                Some((m, _))
                    if self.mbaff && !self.field && self.mb(m).field && y4 == 2 && x4 == 0 =>
                {
                    3
                }
                _ => 1,
            };
            (fix(a, ka), fix(b, 2))
        } else {
            (a, b)
        };
        match (mode(a), mode(b)) {
            (Some(x), Some(y)) => x.min(y),
            _ => 2,
        }
    }

    fn macroblock<E: Ent>(&mut self, e: &mut E) -> Res {
        let raw = e.mb_type(self)?;
        let t = match self.st {
            SliceType::I => intra_type(raw)?,
            SliceType::P => match raw {
                0 => MbT::Inter {
                    part: 0,
                    pred: [1, 1],
                },
                1 => MbT::Inter {
                    part: 1,
                    pred: [1, 1],
                },
                2 => MbT::Inter {
                    part: 2,
                    pred: [1, 1],
                },
                3 => MbT::Inter8x8 { ref0: false },
                4 => MbT::Inter8x8 { ref0: true },
                _ => intra_type(raw - 5)?,
            },
            SliceType::B => match raw {
                0 => MbT::BDirect,
                1..=3 => MbT::Inter {
                    part: 0,
                    pred: [raw as u8, 0],
                },
                4..=21 => {
                    let k = raw as usize - 4;
                    MbT::Inter {
                        part: 1 + (k & 1) as u8,
                        pred: B_PAIRS[k >> 1],
                    }
                }
                22 => MbT::Inter8x8 { ref0: false },
                _ => intra_type(raw - 23)?,
            },
        };
        if t == MbT::Pcm {
            return self.pcm(e);
        }
        let mut t8 = false;
        let mut no_sub_lt8 = true;
        let mut cbp = 0u8;
        let intra = matches!(t, MbT::INxN | MbT::I16 { .. });
        match t {
            MbT::INxN => {
                self.cur().kind = I4;
                if self.pps.transform_8x8_mode {
                    t8 = e.t8_flag(self);
                }
                if t8 {
                    self.cur().kind = I8;
                    self.cur().t8 = true;
                    for b8 in 0..4 {
                        let (x4, y4) = (2 * (b8 & 1) as i32, 2 * (b8 >> 1) as i32);
                        let pred = self.pred_intra_mode(x4, y4);
                        let mode = match e.intra_mode()? {
                            None => pred,
                            Some(r) if r < pred => r,
                            Some(r) => r + 1,
                        };
                        let m = self.cur();
                        for &blk in &B8_BLKS[b8] {
                            m.ipred[blk] = mode;
                        }
                    }
                } else {
                    for bi in 0..16 {
                        let blk = BLK_TO_RASTER[bi] as usize;
                        let pred = self.pred_intra_mode((blk & 3) as i32, (blk >> 2) as i32);
                        let mode = match e.intra_mode()? {
                            None => pred,
                            Some(r) if r < pred => r,
                            Some(r) => r + 1,
                        };
                        self.cur().ipred[blk] = mode;
                    }
                }
                if !self.mono && !self.c444 {
                    let cp = e.chroma_pred(self)?;
                    self.cur().chroma_pred = cp;
                }
            }
            MbT::I16 { pred: _, cbp: c } => {
                self.cur().kind = I16;
                cbp = c;
                if !self.mono && !self.c444 {
                    let cp = e.chroma_pred(self)?;
                    self.cur().chroma_pred = cp;
                }
            }
            MbT::BDirect => {
                self.cur().kind = INTER | DIRECT;
                self.cur().direct8 = 15;
                for b8 in 0..4 {
                    self.direct8x8(b8);
                }
                no_sub_lt8 = self.sps.direct_8x8_inference;
            }
            MbT::Inter { part, pred } => {
                self.cur().kind = INTER;
                self.inter_parts(e, part, pred)?;
            }
            MbT::Inter8x8 { ref0 } => {
                self.cur().kind = INTER;
                no_sub_lt8 = self.inter_8x8(e, ref0)?;
            }
            MbT::Pcm => unreachable!(),
        }
        if !intra {
            // The reference samples arrive while the residual is parsed.
            self.prefetch_mc();
        }
        if !matches!(t, MbT::I16 { .. }) {
            cbp = e.cbp(self, intra)?;
            self.cur().cbp = cbp;
            if cbp & 15 != 0
                && self.pps.transform_8x8_mode
                && !intra
                && no_sub_lt8
                && (t != MbT::BDirect || self.sps.direct_8x8_inference)
            {
                t8 = e.t8_flag(self);
                self.cur().t8 = t8;
            }
        } else {
            self.cur().cbp = cbp;
        }
        if cbp != 0 || matches!(t, MbT::I16 { .. }) {
            let d = e.dqp(self)?;
            let oy = self.qoff[0];
            if !(-(26 + oy / 2)..=25 + oy / 2).contains(&d) {
                return Err(());
            }
            self.prev_dqp_nz = d != 0;
            self.qp = (self.qp + d + 52 + 2 * oy) % (52 + oy) - oy;
            let qp = self.qp;
            self.set_qp_info(qp);
            self.bypass = self.sps.transform_bypass && qp + oy == 0;
            self.residual(e, t, cbp, t8)?;
        } else {
            self.prev_dqp_nz = false;
            let qp = self.qp;
            self.set_qp_info(qp);
            self.bypass = false;
        }
        // Reconstruction.
        match t {
            MbT::INxN => self.recon_intra_nxn(t8),
            MbT::I16 { pred, .. } => self.recon_i16(pred),
            _ => {
                self.mc_mb();
                self.add_luma_residual(t8);
            }
        }
        if self.c444 {
            return Ok(());
        }
        if intra && !self.mono {
            self.pred_chroma();
        }
        self.add_chroma_residual();
        Ok(())
    }

    fn pcm<E: Ent>(&mut self, e: &mut E) -> Res {
        let (ch, cw) = (self.mbch, self.mbcw);
        let nc = cw * ch;
        let mut s = [self.fmt.grey(1) as u16; 768];
        let depth = [self.fmt.depth[0] as u32, self.fmt.depth[1] as u32];
        e.pcm(&mut s[..if self.mono { 256 } else { 256 + 2 * nc }], depth)?;
        let (ls, cs, lo, co) = (self.lst, self.cst, self.lo, self.co);
        let put = |d: &mut [P], s: &[u16]| {
            for (d, &s) in d.iter_mut().zip(s) {
                *d = P::of(s as i32);
            }
        };
        for r in 0..16 {
            put(&mut self.w.y[lo + r * ls..lo + r * ls + 16], &s[r * 16..]);
        }
        for r in 0..ch {
            put(
                &mut self.w.cb[co + r * cs..co + r * cs + cw],
                &s[256 + r * cw..],
            );
            put(
                &mut self.w.cr[co + r * cs..co + r * cs + cw],
                &s[256 + nc + r * cw..],
            );
        }
        self.set_qp_info(0);
        self.prev_dqp_nz = false;
        let m = self.cur();
        m.kind = PCM;
        m.cbp = 0x2f;
        m.nzc = [16; 48];
        m.cbf = u64::MAX;
        m.nzmask = 0xffff;
        Ok(())
    }

    fn inter_parts<E: Ent>(&mut self, e: &mut E, part: u8, pred: [u8; 2]) -> Res {
        let parts: &[(usize, usize, usize, usize)] = match part {
            0 => &[(0, 0, 4, 4)],
            1 => &[(0, 0, 4, 2), (0, 2, 4, 2)],
            _ => &[(0, 0, 2, 4), (2, 0, 2, 4)],
        };
        let shape = match part {
            1 => Shape::H16x8,
            2 => Shape::V8x16,
            _ => Shape::Other,
        };
        let mut refs = [[-1i8; 2]; 2];
        for l in 0..2 {
            let n = self.nref[l];
            for (i, &(x4, y4, w4, h4)) in parts.iter().enumerate() {
                if pred[i] & (1 << l) == 0 {
                    continue;
                }
                let r = if n > 1 {
                    e.ref_idx(self, l, x4, y4, n)?
                } else {
                    0
                };
                if r >= n.max(1) {
                    return Err(());
                }
                refs[l][i] = r as i8;
                for y in (y4..y4 + h4).step_by(2) {
                    for x in (x4..x4 + w4).step_by(2) {
                        self.set_ref(l, (y / 2) * 2 + x / 2, r as i8);
                    }
                }
            }
        }
        for l in 0..2 {
            for (i, &(x4, y4, w4, h4)) in parts.iter().enumerate() {
                if pred[i] & (1 << l) == 0 {
                    continue;
                }
                let mvp = self.pred_mv(l, x4, y4, w4, refs[l][i], shape);
                let dx = e.mvd(self, l, x4, y4, 0)?;
                let dy = e.mvd(self, l, x4, y4, 1)?;
                let mv = [add_mv(mvp[0], dx), add_mv(mvp[1], dy)];
                let ad = [
                    dx.unsigned_abs().min(127) as u8,
                    dy.unsigned_abs().min(127) as u8,
                ];
                self.fill_mv(l, x4, y4, w4, h4, mv, ad);
            }
        }
        Ok(())
    }

    /// P_8x8 / B_8x8; returns noSubMbPartSizeLessThan8x8Flag.
    fn inter_8x8<E: Ent>(&mut self, e: &mut E, ref0: bool) -> Res<bool> {
        let mut sub = [(1u8, 0u8); 4];
        let mut direct = [false; 4];
        let mut no_sub_lt8 = true;
        for b8 in 0..4 {
            let s = e.sub_mb_type(self)? as usize;
            if self.st == SliceType::B {
                if s > 12 {
                    return Err(());
                }
                sub[b8] = B_SUB[s];
                direct[b8] = s == 0;
            } else {
                if s > 3 {
                    return Err(());
                }
                sub[b8] = (1, s as u8);
            }
        }
        for b8 in 0..4 {
            if direct[b8] {
                self.cur().direct8 |= 1 << b8;
                self.direct8x8(b8);
                if !self.sps.direct_8x8_inference {
                    no_sub_lt8 = false;
                }
            } else if sub[b8].1 != 0 {
                no_sub_lt8 = false;
            }
        }
        let mut refs = [[-1i8; 4]; 2];
        for l in 0..2 {
            let n = self.nref[l];
            for b8 in 0..4 {
                if direct[b8] || sub[b8].0 & (1 << l) == 0 {
                    continue;
                }
                let (x4, y4) = (2 * (b8 & 1), 2 * (b8 >> 1));
                let r = if n > 1 && !ref0 {
                    e.ref_idx(self, l, x4, y4, n)?
                } else {
                    0
                };
                if r >= n.max(1) {
                    return Err(());
                }
                refs[l][b8] = r as i8;
                self.set_ref(l, b8, r as i8);
            }
        }
        for l in 0..2 {
            for b8 in 0..4 {
                if direct[b8] || sub[b8].0 & (1 << l) == 0 {
                    continue;
                }
                let (bx, by) = (2 * (b8 & 1), 2 * (b8 >> 1));
                for &(sx, sy, w4, h4) in SUB_PARTS[sub[b8].1 as usize] {
                    let (x4, y4) = (bx + sx, by + sy);
                    let mvp = self.pred_mv(l, x4, y4, w4, refs[l][b8], Shape::Other);
                    let dx = e.mvd(self, l, x4, y4, 0)?;
                    let dy = e.mvd(self, l, x4, y4, 1)?;
                    let mv = [add_mv(mvp[0], dx), add_mv(mvp[1], dy)];
                    let ad = [
                        dx.unsigned_abs().min(127) as u8,
                        dy.unsigned_abs().min(127) as u8,
                    ];
                    self.fill_mv(l, x4, y4, w4, h4, mv, ad);
                }
            }
        }
        Ok(no_sub_lt8)
    }

    fn residual<E: Ent>(&mut self, e: &mut E, t: MbT, cbp: u8, t8: bool) -> Res {
        let intra = matches!(t, MbT::INxN | MbT::I16 { .. });
        let pps = self.pps;
        let mut lv = [0i32; 64];
        // Lossless macroblocks take the levels as residual samples (8.5.12, 8.5.13).
        let bypass = self.bypass;
        let deq4 = |l: i32, ls: i32, qp: i32| if bypass { l } else { deq4(l, ls, qp) };
        let deq8 = |l: i32, ls: i32, qp: i32| if bypass { l } else { deq8(l, ls, qp) };
        // Luma, and in 4:4:4 Cb and Cr the same way (7.3.5.3: residual_luma per component).
        for c in 0..if self.c444 { 3 } else { 1 } {
            // qP' (8.5.12.1): with the bit-depth offset, never negative.
            let qp = if c == 0 {
                self.qp + self.qoff[0]
            } else {
                self.qpc_prime(c - 1)
            };
            let qm = (qp % 6) as usize;
            // ctxBlockCat of this component's DC, AC, 4x4 and 8x8 blocks (Table 9-42).
            let cat = |base: u8| match (c, base) {
                (0, b) => b,
                (_, 5) => 5 + 4 * c as u8,
                (_, b) => 2 + 4 * c as u8 + b,
            };
            let (co, cb) = (c * 256, 16 * c);
            if matches!(t, MbT::I16 { .. }) {
                lv[..16].fill(0);
                let n = e.residual(self, cat(0), 0, 16, &mut lv)?;
                if n > 0 {
                    self.cur().cbf |= 1 << (48 + c);
                    let mut d = [0i32; 16];
                    for k in 0..16 {
                        d[self.scan4[k] as usize] = lv[k].clamp(-(1 << 24), 1 << 24);
                    }
                    let f = if bypass { d } else { hadamard4(&d) };
                    let ls = pps.level4[c][qm][0] as i64;
                    let s = qp / 6;
                    for (blk, &v) in f.iter().enumerate() {
                        let d = if bypass {
                            v
                        } else {
                            let v = v as i64 * ls;
                            dq(if s >= 6 {
                                v << (s - 6)
                            } else {
                                (v + (1 << (5 - s))) >> (6 - s)
                            })
                        };
                        if d != 0 {
                            self.coef[co + blk * 16] = d;
                            self.coded |= 1 << (cb + blk);
                        }
                    }
                }
                if cbp & 15 != 0 {
                    let l4 = &pps.level4[c][qm];
                    for bi in 0..16 {
                        let blk = BLK_TO_RASTER[bi] as usize;
                        lv[..15].fill(0);
                        let n = e.residual(self, cat(1), blk, 15, &mut lv)?;
                        self.note4(c, blk, n);
                        for k in 0..15 {
                            if lv[k] != 0 {
                                let p = self.scan4[k + 1] as usize;
                                self.coef[co + blk * 16 + p] = deq4(lv[k], l4[p], qp);
                                self.coded |= 1 << (cb + blk);
                            }
                        }
                    }
                }
                continue;
            }
            let list = if intra { c } else { 3 + c };
            for b8 in 0..4 {
                if cbp & (1 << b8) == 0 {
                    continue;
                }
                if t8 {
                    let l8 = &pps.level8[2 * c + usize::from(!intra)][qm];
                    if E::CABAC {
                        lv.fill(0);
                        let n = e.residual(self, cat(5), b8, 64, &mut lv)?;
                        // Without 4:4:4 the 8x8 block has no coded_block_flag: it counts as
                        // coded for its neighbours' contexts.
                        let c444 = self.c444;
                        let coded = !c444 || n > 0;
                        let m = self.cur();
                        if c444 && n > 0 {
                            m.cbf |= 1 << (52 + 4 * c + b8);
                        }
                        for &blk in &B8_BLKS[b8] {
                            if coded {
                                m.cbf |= 1 << (cb + blk);
                            }
                            m.nzc[cb + blk] = n as u8;
                            if c == 0 && n > 0 {
                                m.nzmask |= 1 << blk;
                            }
                        }
                        for k in 0..64 {
                            if lv[k] != 0 {
                                let p = self.scan8[k] as usize;
                                self.coef[co + b8 * 64 + p] = deq8(lv[k], l8[p], qp);
                                self.coded8 |= 1 << (4 * c + b8);
                            }
                        }
                    } else {
                        let mut any = false;
                        for i in 0..4 {
                            let blk = B8_BLKS[b8][i];
                            lv[..16].fill(0);
                            let n = e.residual(self, cat(2), blk, 16, &mut lv)?;
                            self.cur().nzc[cb + blk] = n as u8;
                            any |= n > 0;
                            for k in 0..16 {
                                if lv[k] != 0 {
                                    let p = self.scan8[4 * k + i] as usize;
                                    self.coef[co + b8 * 64 + p] = deq8(lv[k], l8[p], qp);
                                    self.coded8 |= 1 << (4 * c + b8);
                                }
                            }
                        }
                        if any && c == 0 {
                            let m = self.cur();
                            for &blk in &B8_BLKS[b8] {
                                m.nzmask |= 1 << blk;
                            }
                        }
                    }
                } else {
                    let l4 = &pps.level4[list][qm];
                    for i in 0..4 {
                        let blk = BLK_TO_RASTER[b8 * 4 + i] as usize;
                        lv[..16].fill(0);
                        let n = e.residual(self, cat(2), blk, 16, &mut lv)?;
                        self.note4(c, blk, n);
                        for k in 0..16 {
                            if lv[k] != 0 {
                                let p = self.scan4[k] as usize;
                                self.coef[co + blk * 16 + p] = deq4(lv[k], l4[p], qp);
                                self.coded |= 1 << (cb + blk);
                            }
                        }
                    }
                }
            }
        }
        let chroma = cbp >> 4;
        // Chroma 4x4 blocks per component: 4 (4:2:0) or 8 (4:2:2).
        let nblk = self.mbch / 2;
        if chroma != 0 {
            for comp in 0..2 {
                lv[..nblk].fill(0);
                let n = e.residual(self, 3, comp, nblk, &mut lv)?;
                if n == 0 {
                    continue;
                }
                self.cur().cbf |= 1 << (49 + comp);
                let qc = self.qpc_prime(comp);
                let list = if intra { 1 + comp } else { 4 + comp };
                let mut f = [0i32; 8];
                if bypass {
                    // dcC = c (8.5.11.1): the levels in the raster of the DC matrix.
                    let order: &[usize] = if nblk == 4 {
                        &[0, 1, 2, 3]
                    } else {
                        &[0, 2, 1, 5, 3, 6, 4, 7]
                    };
                    for (d, &k) in f.iter_mut().zip(order) {
                        *d = lv[k];
                    }
                } else if nblk == 4 {
                    let ls = pps.level4[list][(qc % 6) as usize][0] as i64;
                    let c = [lv[0], lv[1], lv[2], lv[3]].map(|v| v.clamp(-(1 << 24), 1 << 24));
                    let h = [
                        c[0] + c[1] + c[2] + c[3],
                        c[0] - c[1] + c[2] - c[3],
                        c[0] + c[1] - c[2] - c[3],
                        c[0] - c[1] - c[2] + c[3],
                    ];
                    for (d, &v) in f.iter_mut().zip(&h) {
                        *d = dq(((v as i64 * ls) << (qc / 6)) >> 5);
                    }
                } else {
                    // 4:2:2 (8.5.11.1): a 2x4 transform of the DC levels in the order of
                    // 8.5.11.1 (rows of c: 0 2, 1 5, 3 6, 4 7), scaled with QP'c + 3.
                    let q = qc + 3;
                    let ls = pps.level4[list][(q % 6) as usize][0] as i64;
                    let l = |k: usize| lv[k].clamp(-(1 << 24), 1 << 24);
                    let c = [[l(0), l(2)], [l(1), l(5)], [l(3), l(6)], [l(4), l(7)]];
                    // Rows: A4 * c, then the 2-point transform across the columns.
                    let col = |j: usize| {
                        let (a, b, cc, d) = (c[0][j], c[1][j], c[2][j], c[3][j]);
                        [
                            a + b + cc + d,
                            a + b - cc - d,
                            a - b - cc + d,
                            a - b + cc - d,
                        ]
                    };
                    let (c0, c1) = (col(0), col(1));
                    for i in 0..4 {
                        let s = [c0[i] + c1[i], c0[i] - c1[i]];
                        for j in 0..2 {
                            let v = s[j] as i64 * ls;
                            f[2 * i + j] = dq(if q >= 36 {
                                v << (q / 6 - 6)
                            } else {
                                (v + (1 << (5 - q / 6))) >> (6 - q / 6)
                            });
                        }
                    }
                }
                for (blk, &d) in f.iter().take(nblk).enumerate() {
                    if d != 0 {
                        self.coef[256 + (comp * 8 + blk) * 16] = d;
                        self.coded |= 1 << (16 + comp * 8 + blk);
                    }
                }
            }
            if chroma == 2 {
                for comp in 0..2 {
                    let qc = self.qpc_prime(comp);
                    let list = if intra { 1 + comp } else { 4 + comp };
                    let l4 = &pps.level4[list][(qc % 6) as usize];
                    for blk in 0..nblk {
                        lv[..15].fill(0);
                        let idx = comp * 8 + blk;
                        let n = e.residual(self, 4, idx, 15, &mut lv)?;
                        let m = self.cur();
                        m.nzc[16 + idx] = n as u8;
                        if n > 0 {
                            m.cbf |= 1 << (16 + idx);
                        }
                        for k in 0..15 {
                            if lv[k] != 0 {
                                let p = self.scan4[k + 1] as usize;
                                self.coef[256 + idx * 16 + p] = deq4(lv[k], l4[p], qc);
                                self.coded |= 1 << (16 + idx);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Notes 4x4 block `blk` of plane `c` (0 = Y; Cb and Cr in 4:4:4) with `n` coefficients.
    fn note4(&mut self, c: usize, blk: usize, n: u32) {
        let m = self.cur();
        m.nzc[16 * c + blk] = n as u8;
        if n > 0 {
            m.cbf |= 1 << (16 * c + blk);
            if c == 0 {
                m.nzmask |= 1 << blk;
            }
        }
    }

    /// The macroblock's offset and line stride in plane `c` (0 = Y, 1 = Cb, 2 = Cr), and the
    /// plane's largest sample value; 4:4:4 Cb and Cr are as large as luma.
    fn geo(&self, c: usize) -> (usize, usize, i32) {
        let max = self.max[usize::from(c > 0)];
        if c == 0 {
            (self.lo, self.lst, max)
        } else {
            (self.co, self.cst, max)
        }
    }

    /// The planes predicted and transformed like luma: Y, and Cb and Cr in 4:4:4.
    fn luma_planes(&self) -> usize {
        if self.c444 { 3 } else { 1 }
    }

    fn recon_intra_nxn(&mut self, t8: bool) {
        for c in 0..self.luma_planes() {
            self.recon_intra_nxn_plane(c, t8);
        }
    }

    fn recon_intra_nxn_plane(&mut self, c: usize, t8: bool) {
        if t8 {
            for b8 in 0..4 {
                let (x8, y8) = (b8 & 1, b8 >> 1);
                let (x, y) = (x8 as i32 * 8, y8 as i32 * 8);
                let av = Avail {
                    left: x8 > 0 || self.lav(y, 8, 16, 16),
                    top: y8 > 0 || self.pav(x, -1, 16, 16),
                    top_left: self.pav(x - 1, y - 1, 16, 16),
                    top_right: match (x8, y8) {
                        (_, 0) => self.pav(x + 8, -1, 16, 16),
                        (0, 1) => true,
                        _ => false,
                    },
                };
                let mode = self.mb(self.addr).ipred[B8_BLKS[b8][0]];
                let coded = self.coded8 & (1 << (4 * c + b8)) != 0;
                let (o, ls, max) = self.geo(c);
                let pl = plane_of(self.w, c);
                let off = o + y8 * 8 * ls + x8 * 8;
                intra::pred8x8(pl, off, ls, mode, av, max);
                if coded && self.bypass {
                    let dpcm = if mode < 2 { 1 + mode } else { 0 };
                    let cf = &mut self.coef[c * 256 + b8 * 64..];
                    bypass_add(cf, 8, 8, pl, off, ls, max, dpcm);
                } else if coded {
                    idct8_add(&mut self.coef[c * 256 + b8 * 64..], pl, off, ls, max);
                }
            }
            return;
        }
        for bi in 0..16 {
            let blk = BLK_TO_RASTER[bi] as usize;
            let (x4, y4) = (blk & 3, blk >> 2);
            let (x, y) = (x4 as i32 * 4, y4 as i32 * 4);
            let tr = if y4 == 0 {
                self.pav(x + 4, -1, 16, 16)
            } else if x4 < 3 {
                (RASTER_TO_BLK[(y4 - 1) * 4 + x4 + 1] as usize) < bi
            } else {
                false
            };
            let av = Avail {
                left: x4 > 0 || self.lav(y, 4, 16, 16),
                top: y4 > 0 || self.pav(x, -1, 16, 16),
                top_left: self.pav(x - 1, y - 1, 16, 16),
                top_right: tr,
            };
            let mode = self.mb(self.addr).ipred[blk];
            let coded = self.coded & (1 << (16 * c + blk)) != 0;
            let (o, ls, max) = self.geo(c);
            let pl = plane_of(self.w, c);
            let off = o + y4 * 4 * ls + x4 * 4;
            intra::pred4x4(pl, off, ls, mode, av, max);
            if coded && self.bypass {
                let dpcm = if mode < 2 { 1 + mode } else { 0 };
                let cf = &mut self.coef[c * 256 + blk * 16..];
                bypass_add(cf, 4, 4, pl, off, ls, max, dpcm);
            } else if coded {
                idct4_add(&mut self.coef[c * 256 + blk * 16..], pl, off, ls, max);
            }
        }
    }

    fn recon_i16(&mut self, pred: u8) {
        let av = Avail {
            left: self.lav(0, 16, 16, 16),
            top: self.pav(0, -1, 16, 16),
            top_left: self.pav(-1, -1, 16, 16),
            top_right: false,
        };
        for c in 0..self.luma_planes() {
            let (o, ls, max) = self.geo(c);
            intra::pred16x16(plane_of(self.w, c), o, ls, pred, av, max);
            if self.bypass && pred < 2 {
                // Lossless vertical or horizontal prediction: the residual of the whole
                // macroblock is accumulated (8.5.10).
                let mut r = [0i32; 256];
                for blk in 0..16 {
                    let (x, y) = ((blk & 3) * 4, (blk >> 2) * 4);
                    let cf = &mut self.coef[c * 256 + blk * 16..c * 256 + blk * 16 + 16];
                    for (i, v) in cf.iter_mut().enumerate() {
                        r[(y + i / 4) * 16 + x + i % 4] = std::mem::take(v);
                    }
                }
                bypass_add(&mut r, 16, 16, plane_of(self.w, c), o, ls, max, 1 + pred);
                continue;
            }
            self.add_residual(c, false);
        }
    }

    fn add_luma_residual(&mut self, t8: bool) {
        for c in 0..self.luma_planes() {
            self.add_residual(c, t8);
        }
    }

    /// Adds the residual of plane `c` (luma, or a 4:4:4 chroma plane).
    fn add_residual(&mut self, c: usize, t8: bool) {
        let (coded, coded8, bypass) = (self.coded, self.coded8, self.bypass);
        let (lo, ls, max) = self.geo(c);
        let (pl, coef) = (plane_of(self.w, c), &mut self.coef);
        if t8 {
            for b8 in 0..4 {
                if coded8 & (1 << (4 * c + b8)) != 0 {
                    let off = lo + (b8 >> 1) * 8 * ls + (b8 & 1) * 8;
                    let cf = &mut coef[c * 256 + b8 * 64..];
                    if bypass {
                        bypass_add(cf, 8, 8, pl, off, ls, max, 0);
                    } else {
                        idct8_add(cf, pl, off, ls, max);
                    }
                }
            }
        } else {
            let mut bits = (coded >> (16 * c)) & 0xffff;
            while bits != 0 {
                let blk = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let off = lo + (blk >> 2) * 4 * ls + (blk & 3) * 4;
                let cf = &mut coef[c * 256 + blk * 16..];
                if bypass {
                    bypass_add(cf, 4, 4, pl, off, ls, max, 0);
                } else {
                    idct4_add(cf, pl, off, ls, max);
                }
            }
        }
    }

    fn pred_chroma(&mut self) {
        let (cs, co) = (self.cst, self.co);
        let ch = self.mbch as i32;
        let halves = if self.mbaff {
            [self.lav(0, ch / 2, 8, ch), self.lav(ch / 2, ch / 2, 8, ch)]
        } else {
            [self.ia(self.na); 2]
        };
        let av = Avail {
            left: halves[0] && halves[1],
            top: self.pav(0, -1, 8, ch),
            top_left: self.pav(-1, -1, 8, ch),
            top_right: false,
        };
        let mode = self.mb(self.addr).chroma_pred;
        let (h, max) = (self.mbch, self.max[1]);
        intra::pred_chroma(&mut self.w.cb, co, cs, mode, av, halves, h, max);
        intra::pred_chroma(&mut self.w.cr, co, cs, mode, av, halves, h, max);
    }

    fn add_chroma_residual(&mut self) {
        let (cs, co, max) = (self.cst, self.co, self.max[1]);
        if self.bypass {
            // Lossless: intra horizontal (1) or vertical (2) chroma prediction accumulates
            // the residual of the whole component (8.5.11.2).
            let m = self.mb(self.addr);
            let mode = m.chroma_pred;
            let dpcm = if m.intra() && (mode == 1 || mode == 2) {
                3 - mode
            } else {
                0
            };
            let h = self.mbch;
            for comp in 0..2 {
                let mut r = [0i32; 128];
                for blk in 0..h / 2 {
                    let (x, y) = ((blk & 1) * 4, (blk >> 1) * 4);
                    let k = 256 + (comp * 8 + blk) * 16;
                    for (i, v) in self.coef[k..k + 16].iter_mut().enumerate() {
                        r[(y + i / 4) * 8 + x + i % 4] = std::mem::take(v);
                    }
                }
                let plane = if comp == 0 {
                    &mut self.w.cb
                } else {
                    &mut self.w.cr
                };
                bypass_add(&mut r, 8, h, plane, co, cs, max, dpcm);
            }
            return;
        }
        let mut bits = (self.coded >> 16) & 0xffff;
        while bits != 0 {
            let idx = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            let (comp, blk) = (idx >> 3, idx & 7);
            let off = co + (blk >> 1) * 4 * cs + (blk & 1) * 4;
            let plane = if comp == 0 {
                &mut self.w.cb
            } else {
                &mut self.w.cr
            };
            idct4_add(&mut self.coef[256 + idx * 16..], plane, off, cs, max);
        }
    }
}

/// Plane `c` (0 = Y, 1 = Cb, 2 = Cr) of the picture being decoded.
#[inline(always)]
fn plane_of<P: Pix>(w: &mut Work<P>, c: usize) -> &mut Vec<P> {
    match c {
        0 => &mut w.y,
        1 => &mut w.cb,
        _ => &mut w.cr,
    }
}

fn add_mv(p: i16, d: i32) -> i16 {
    (p as i32 + d).clamp(-32768, 32767) as i16
}

fn intra_type(t: u32) -> Res<MbT> {
    Ok(match t {
        0 => MbT::INxN,
        1..=24 => {
            let k = t - 1;
            MbT::I16 {
                pred: (k % 4) as u8,
                cbp: ((((k / 4) % 3) << 4) | if k >= 12 { 15 } else { 0 }) as u8,
            }
        }
        25 => MbT::Pcm,
        _ => return Err(()),
    })
}

/// The field list of parity `par` of a frame list (8.4.2.1): each frame's field of that
/// parity, then the other.
fn split_fields(frames: &[RefPic], par: usize) -> Vec<RefPic> {
    let mut v = Vec::with_capacity(frames.len() * 2);
    for f in frames {
        for k in 0..2 {
            let p = par ^ k;
            v.push(RefPic {
                pic: f.pic.clone(),
                id: (f.id & !3) | (1 + p as u32),
                poc: f.pocs[p],
                pocs: f.pocs,
                long: f.long,
                parity: Some(p as u8),
            });
        }
    }
    v
}

/// DiffPicOrderCnt(a, b) clipped to -128..=127 (8-197, 8-198), in i64 so that damaged
/// streams' extreme counts cannot overflow.
fn poc_diff(a: i32, b: i32) -> i32 {
    (a as i64 - b as i64).clamp(-128, 127) as i32
}

pub(crate) fn implicit_weights(poc: i32, p0: &RefPic, p1: &RefPic) -> (i32, i32) {
    if p1.poc == p0.poc || p0.long || p1.long {
        return (32, 32);
    }
    let tb = poc_diff(poc, p0.poc);
    let td = poc_diff(p1.poc, p0.poc);
    let tx = (16384 + (td / 2).abs()) / td;
    let dsf = ((tb * tx + 32) >> 6).clamp(-1024, 1023);
    if !(-64..=128).contains(&(dsf >> 2)) {
        (32, 32)
    } else {
        (64 - (dsf >> 2), dsf >> 2)
    }
}

/// Entropy decoding of the macroblock syntax elements.
trait Ent {
    const CABAC: bool;
    fn mb_type<P: Pix>(&mut self, sd: &Sd<P>) -> Res<u32>;
    fn sub_mb_type<P: Pix>(&mut self, sd: &Sd<P>) -> Res<u32>;
    fn t8_flag<P: Pix>(&mut self, sd: &Sd<P>) -> bool;
    /// None when prev_intra_pred_mode_flag is set, else rem_intra_pred_mode.
    fn intra_mode(&mut self) -> Res<Option<u8>>;
    fn chroma_pred<P: Pix>(&mut self, sd: &Sd<P>) -> Res<u8>;
    fn ref_idx<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        list: usize,
        x4: usize,
        y4: usize,
        n: usize,
    ) -> Res<usize>;
    fn mvd<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        list: usize,
        x4: usize,
        y4: usize,
        comp: usize,
    ) -> Res<i32>;
    fn cbp<P: Pix>(&mut self, sd: &Sd<P>, intra: bool) -> Res<u8>;
    fn dqp<P: Pix>(&mut self, sd: &Sd<P>) -> Res<i32>;
    /// A residual block of category `cat` (`idx`: luma raster 4x4, chroma component, chroma
    /// component * 8 + block, or 8x8 index) into `lv` in scan order; returns the number of
    /// non-zero coefficients.
    fn residual<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        cat: u8,
        idx: usize,
        max: usize,
        lv: &mut [i32],
    ) -> Res<u32>;
    /// pcm_sample_luma (256) then pcm_sample_chroma, of `depth` (luma, chroma) bits.
    fn pcm(&mut self, out: &mut [u16], depth: [u32; 2]) -> Res;
}

struct Cavlc<'a> {
    /// The slice data, or partition A (category 2) of a partitioned slice.
    r: BitReader<'a>,
    /// A partitioned slice: partitions B (residual of intra macroblocks) and C (of inter
    /// ones), each when it came. A missing one reads as no coefficients.
    parts: Option<[Option<BitReader<'a>>; 2]>,
}

impl<'a> Cavlc<'a> {
    /// Where the residual of the current macroblock is read: `None` when its partition is
    /// missing.
    fn residual_reader<P: Pix>(&mut self, sd: &Sd<P>) -> Option<&mut BitReader<'a>> {
        match &mut self.parts {
            None => Some(&mut self.r),
            Some(p) => p[!sd.mb(sd.addr).intra() as usize].as_mut(),
        }
    }
}

impl Ent for Cavlc<'_> {
    const CABAC: bool = false;

    fn mb_type<P: Pix>(&mut self, _: &Sd<P>) -> Res<u32> {
        Ok(self.r.ue())
    }

    fn sub_mb_type<P: Pix>(&mut self, _: &Sd<P>) -> Res<u32> {
        Ok(self.r.ue())
    }

    fn t8_flag<P: Pix>(&mut self, _: &Sd<P>) -> bool {
        self.r.flag()
    }

    fn intra_mode(&mut self) -> Res<Option<u8>> {
        Ok(if self.r.flag() {
            None
        } else {
            Some(self.r.u(3) as u8)
        })
    }

    fn chroma_pred<P: Pix>(&mut self, _: &Sd<P>) -> Res<u8> {
        let v = self.r.ue();
        if v > 3 { Err(()) } else { Ok(v as u8) }
    }

    fn ref_idx<P: Pix>(&mut self, _: &Sd<P>, _: usize, _: usize, _: usize, n: usize) -> Res<usize> {
        Ok(if n == 2 {
            (!self.r.flag()) as usize
        } else {
            self.r.ue() as usize
        })
    }

    fn mvd<P: Pix>(&mut self, _: &Sd<P>, _: usize, _: usize, _: usize, _: usize) -> Res<i32> {
        let v = self.r.se();
        if !(-(1 << 15)..(1 << 15)).contains(&v) {
            return Err(());
        }
        Ok(v)
    }

    fn cbp<P: Pix>(&mut self, sd: &Sd<P>, intra: bool) -> Res<u8> {
        let v = self.r.ue() as usize;
        if sd.mono || sd.c444 {
            return match (v < 16, intra) {
                (false, _) => Err(()),
                (true, true) => Ok(CBP_INTRA_MONO[v]),
                (true, false) => Ok(CBP_INTER_MONO[v]),
            };
        }
        if v > 47 {
            return Err(());
        }
        Ok(if intra { CBP_INTRA[v] } else { CBP_INTER[v] })
    }

    fn dqp<P: Pix>(&mut self, _: &Sd<P>) -> Res<i32> {
        Ok(self.r.se())
    }

    fn residual<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        cat: u8,
        idx: usize,
        max: usize,
        lv: &mut [i32],
    ) -> Res<u32> {
        let (c, base) = cat_plane(cat);
        // With data partitioning and constrained intra prediction, an intra macroblock
        // counts an inter neighbour as available with nN = 0 (9.2.1).
        let cip = self.parts.is_some() && sd.pps.constrained_intra_pred && sd.mb(sd.addr).intra();
        let zero = |m: usize| cip && !sd.mb(m).intra();
        let nc = match base {
            3 if max == 8 => -2,
            3 => -1,
            4 => {
                let (comp, blk) = (idx >> 3, idx & 7);
                let (x2, y2) = ((blk & 1) as i32, (blk >> 1) as i32);
                let get = |n: Option<(usize, usize)>| {
                    n.map(|(m, b)| {
                        if zero(m) {
                            0
                        } else {
                            sd.mb(m).nzc[16 + comp * 8 + b] as i32
                        }
                    })
                };
                combine_nc(get(sd.nbc(x2 - 1, y2)), get(sd.nbc(x2, y2 - 1)))
            }
            _ => {
                let (x4, y4) = ((idx & 3) as i32, (idx >> 2) as i32);
                let get = |n: Option<(usize, usize)>| {
                    n.map(|(m, b)| {
                        if zero(m) {
                            0
                        } else {
                            sd.mb(m).nzc[16 * c + b] as i32
                        }
                    })
                };
                combine_nc(get(sd.nb4(x4 - 1, y4)), get(sd.nb4(x4, y4 - 1)))
            }
        };
        let Some(r) = self.residual_reader(sd) else {
            return Ok(0);
        };
        let n = residual_block(r, nc, max, lv).ok_or(())?;
        Ok(n)
    }

    fn pcm(&mut self, out: &mut [u16], depth: [u32; 2]) -> Res {
        // Category 3: partition B of a partitioned slice.
        let r = match &mut self.parts {
            None => &mut self.r,
            Some(p) => match p[0].as_mut() {
                Some(r) => r,
                None => return Err(()),
            },
        };
        r.align();
        for (i, v) in out.iter_mut().enumerate() {
            *v = r.u(depth[(i >= 256) as usize]) as u16;
        }
        if r.overrun() { Err(()) } else { Ok(()) }
    }
}

fn combine_nc(a: Option<i32>, b: Option<i32>) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => (a + b + 1) >> 1,
        (Some(a), None) => a,
        (None, Some(b)) => b,
        _ => 0,
    }
}

struct CabacE<'a> {
    c: Cabac<'a>,
    len: usize,
}

impl CabacE<'_> {
    fn field_flag<P: Pix>(&mut self, sd: &Sd<P>) -> bool {
        let cond = |n: Option<usize>| n.is_some_and(|m| sd.mb(m).field) as usize;
        self.c.decision(70 + cond(sd.pa) + cond(sd.pb)) == 1
    }

    fn skip_flag<P: Pix>(&mut self, sd: &Sd<P>) -> bool {
        let cond = |n: Option<usize>| n.is_some_and(|m| sd.mb(m).kind & SKIP == 0) as usize;
        let base = if sd.st == SliceType::B { 24 } else { 11 };
        self.c.decision(base + cond(sd.na) + cond(sd.nb)) == 1
    }

    fn mb_type_i(&mut self, base: usize, slice_i: bool, inc0: usize) -> u32 {
        let c = &mut self.c;
        if c.decision(base + if slice_i { inc0 } else { 0 }) == 0 {
            return 0;
        }
        if c.terminate() {
            return 25;
        }
        let (cl, cc1, cc2, cp1, cp2) = if slice_i {
            (3, 4, 5, 6, 7)
        } else {
            (1, 2, 2, 3, 3)
        };
        let luma = c.decision(base + cl);
        let mut chroma = c.decision(base + cc1);
        if chroma == 1 {
            chroma += c.decision(base + cc2);
        }
        let p = (c.decision(base + cp1) << 1) | c.decision(base + cp2);
        1 + p + 4 * chroma + 12 * luma
    }

    /// coded_block_flag context condition of a neighbouring block.
    #[inline]
    fn cbf_cond<P: Pix>(
        sd: &Sd<P>,
        n: Option<(usize, usize)>,
        bit: impl Fn(usize) -> u32,
    ) -> usize {
        match n {
            None => sd.mb(sd.addr).intra() as usize,
            Some((m, b)) => {
                let mi = sd.mb(m);
                if mi.kind & PCM != 0 {
                    1
                } else {
                    ((mi.cbf >> bit(b)) & 1) as usize
                }
            }
        }
    }
}

/// ctxBlockCat (Table 9-42) as (plane, the luma or 4:2:x chroma category it is coded
/// like): 6..=9 are Cb and 10..=13 Cr blocks of 4:4:4, coded like 0, 1, 2 and 5.
#[inline(always)]
fn cat_plane(cat: u8) -> (usize, usize) {
    match cat {
        0..=5 => (0, cat as usize),
        _ => {
            let k = cat as usize - 6;
            (1 + k / 4, [0, 1, 2, 5][k % 4])
        }
    }
}

const CAT_SIG: [usize; 5] = [0, 15, 29, 44, 47];
const CAT_ABS: [usize; 5] = [0, 10, 20, 30, 39];

impl Ent for CabacE<'_> {
    const CABAC: bool = true;

    fn mb_type<P: Pix>(&mut self, sd: &Sd<P>) -> Res<u32> {
        Ok(match sd.st {
            SliceType::I => {
                let cond =
                    |n: Option<usize>| n.is_some_and(|m| sd.mb(m).kind & (I4 | I8) == 0) as usize;
                self.mb_type_i(3, true, cond(sd.na) + cond(sd.nb))
            }
            SliceType::P => {
                let c = &mut self.c;
                if c.decision(14) == 1 {
                    5 + self.mb_type_i(17, false, 0)
                } else if c.decision(15) == 0 {
                    if c.decision(16) == 0 { 0 } else { 3 }
                } else if c.decision(17) == 1 {
                    1
                } else {
                    2
                }
            }
            SliceType::B => {
                let cond =
                    |n: Option<usize>| n.is_some_and(|m| sd.mb(m).kind & DIRECT == 0) as usize;
                let c = &mut self.c;
                if c.decision(27 + cond(sd.na) + cond(sd.nb)) == 0 {
                    0
                } else if c.decision(27 + 3) == 0 {
                    1 + c.decision(27 + 5)
                } else {
                    let mut bits = c.decision(27 + 4) << 3;
                    bits |= c.decision(27 + 5) << 2;
                    bits |= c.decision(27 + 5) << 1;
                    bits |= c.decision(27 + 5);
                    match bits {
                        0..=7 => bits + 3,
                        13 => 23 + self.mb_type_i(32, false, 0),
                        14 => 11,
                        15 => 22,
                        _ => 12 + (((bits & 7) << 1) | c.decision(27 + 5)),
                    }
                }
            }
        })
    }

    fn sub_mb_type<P: Pix>(&mut self, sd: &Sd<P>) -> Res<u32> {
        let c = &mut self.c;
        if sd.st != SliceType::B {
            return Ok(if c.decision(21) == 1 {
                0
            } else if c.decision(22) == 0 {
                1
            } else if c.decision(23) == 1 {
                2
            } else {
                3
            });
        }
        if c.decision(36) == 0 {
            return Ok(0);
        }
        if c.decision(37) == 0 {
            return Ok(1 + c.decision(39));
        }
        let mut t = 3;
        if c.decision(38) == 1 {
            if c.decision(39) == 1 {
                return Ok(11 + c.decision(39));
            }
            t += 4;
        }
        t += 2 * c.decision(39);
        t += c.decision(39);
        Ok(t)
    }

    fn t8_flag<P: Pix>(&mut self, sd: &Sd<P>) -> bool {
        let cond = |n: Option<usize>| n.is_some_and(|m| sd.mb(m).t8) as usize;
        self.c.decision(399 + cond(sd.na) + cond(sd.nb)) == 1
    }

    fn intra_mode(&mut self) -> Res<Option<u8>> {
        let c = &mut self.c;
        if c.decision(68) == 1 {
            return Ok(None);
        }
        let b0 = c.decision(69);
        let b1 = c.decision(69);
        let b2 = c.decision(69);
        Ok(Some((b0 | (b1 << 1) | (b2 << 2)) as u8))
    }

    fn chroma_pred<P: Pix>(&mut self, sd: &Sd<P>) -> Res<u8> {
        let cond = |n: Option<usize>| {
            n.is_some_and(|m| {
                let mi = sd.mb(m);
                mi.intra() && mi.kind & PCM == 0 && mi.chroma_pred != 0
            }) as usize
        };
        Ok(self.c.unary(64 + cond(sd.na) + cond(sd.nb), 67, 3) as u8)
    }

    fn ref_idx<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        list: usize,
        x4: usize,
        y4: usize,
        _: usize,
    ) -> Res<usize> {
        let cond = |n: Option<(usize, usize)>| {
            n.is_some_and(|(m, b)| {
                let mi = sd.mb(m);
                let b8 = b8_of(b);
                // A field neighbour of a frame macroblock counts in fields (9-12).
                let lim = if sd.mbaff && !sd.field && mi.field {
                    1
                } else {
                    0
                };
                !mi.intra() && mi.direct8 & (1 << b8) == 0 && mi.ref_idx[list][b8] > lim
            }) as usize
        };
        let (x, y) = (x4 as i32, y4 as i32);
        let inc = cond(sd.nb4(x - 1, y)) + 2 * cond(sd.nb4(x, y - 1));
        let c = &mut self.c;
        if c.decision(54 + inc) == 0 {
            return Ok(0);
        }
        let mut v = 1;
        if c.decision(54 + 4) == 1 {
            v = 2;
            while c.decision(54 + 5) == 1 {
                v += 1;
                if v > 32 {
                    return Err(());
                }
            }
        }
        Ok(v)
    }

    fn mvd<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        list: usize,
        x4: usize,
        y4: usize,
        comp: usize,
    ) -> Res<i32> {
        let (x, y) = (x4 as i32, y4 as i32);
        let get = |n: Option<(usize, usize)>| {
            n.map_or(0, |(m, b)| {
                let mi = sd.mb(m);
                let v = mi.mvd[list][b][comp] as u32;
                // 9-15 / 9-16: vertical components between frame and field macroblocks.
                if comp == 1 && sd.mbaff && mi.field != sd.field {
                    if sd.field { v / 2 } else { v * 2 }
                } else {
                    v
                }
            })
        };
        let sum = get(sd.nb4(x - 1, y)) + get(sd.nb4(x, y - 1));
        let inc = if sum < 3 {
            0
        } else if sum <= 32 {
            1
        } else {
            2
        };
        let base = if comp == 0 { 40 } else { 47 };
        let c = &mut self.c;
        if c.decision(base + inc) == 0 {
            return Ok(0);
        }
        let mut v = 1u32;
        while v < 9 {
            let ctx = base + [3, 4, 5, 6, 6, 6, 6, 6][v as usize - 1];
            if c.decision(ctx) == 0 {
                break;
            }
            v += 1;
        }
        if v >= 9 {
            v += c.exp_golomb_bypass(3);
            if v > 1 << 15 {
                return Err(());
            }
        }
        Ok(if c.bypass() == 1 {
            -(v as i32)
        } else {
            v as i32
        })
    }

    fn cbp<P: Pix>(&mut self, sd: &Sd<P>, _: bool) -> Res<u8> {
        let mut cbp = 0u8;
        for b8 in 0..4usize {
            // Left and above 8x8 blocks: inside this macroblock or in a neighbour.
            let (x4, y4) = (2 * (b8 & 1) as i32, 2 * (b8 >> 1) as i32);
            let cond = |n: Option<(usize, usize)>, cbp: u8| {
                n.map_or(0, |(m, blk)| {
                    let n8 = b8_of(blk);
                    if m == sd.addr {
                        ((cbp >> n8) & 1 == 0) as usize
                    } else {
                        let mi = sd.mb(m);
                        (mi.kind & PCM == 0 && (mi.cbp >> n8) & 1 == 0) as usize
                    }
                })
            };
            let a = cond(sd.nb4(x4 - 1, y4), cbp);
            let b = cond(sd.nb4(x4, y4 - 1), cbp);
            cbp |= (self.c.decision(73 + a + 2 * b) as u8) << b8;
        }
        let ch = |n: Option<usize>, two: bool| {
            n.is_some_and(|m| {
                let c = sd.mb(m).cbp >> 4;
                if two { c == 2 } else { c != 0 }
            }) as usize
        };
        if !sd.mono
            && !sd.c444
            && self
                .c
                .decision(77 + ch(sd.na, false) + 2 * ch(sd.nb, false))
                == 1
        {
            let second = self
                .c
                .decision(77 + 4 + ch(sd.na, true) + 2 * ch(sd.nb, true));
            cbp |= ((1 + second) as u8) << 4;
        }
        Ok(cbp)
    }

    fn dqp<P: Pix>(&mut self, sd: &Sd<P>) -> Res<i32> {
        let c = &mut self.c;
        if c.decision(60 + sd.prev_dqp_nz as usize) == 0 {
            return Ok(0);
        }
        let mut k = 1;
        let mut ctx = 62;
        while c.decision(ctx) == 1 {
            k += 1;
            ctx = 63;
            if k > 104 {
                return Err(());
            }
        }
        Ok(if k & 1 == 1 { (k + 1) / 2 } else { -(k / 2) })
    }

    fn residual<P: Pix>(
        &mut self,
        sd: &Sd<P>,
        cat: u8,
        idx: usize,
        max: usize,
        lv: &mut [i32],
    ) -> Res<u32> {
        let (c, cat) = cat_plane(cat);
        // Without 4:4:4 an 8x8 block has no coded_block_flag (7.3.5.3.3).
        if cat != 5 || sd.c444 {
            let inc = match cat {
                // The DC bits are only ever set in Intra16x16 and I_PCM macroblocks.
                0 => {
                    let f =
                        |n: Option<usize>| Self::cbf_cond(sd, n.map(|m| (m, 0)), |_| 48 + c as u32);
                    f(sd.na) + 2 * f(sd.nb)
                }
                3 => {
                    let f = |n: Option<usize>| {
                        Self::cbf_cond(sd, n.map(|m| (m, 0)), |_| 49 + idx as u32)
                    };
                    f(sd.na) + 2 * f(sd.nb)
                }
                4 => {
                    let (comp, blk) = (idx >> 3, idx & 7);
                    let (x2, y2) = ((blk & 1) as i32, (blk >> 1) as i32);
                    let bit = |b: usize| (16 + comp * 8 + b) as u32;
                    Self::cbf_cond(sd, sd.nbc(x2 - 1, y2), bit)
                        + 2 * Self::cbf_cond(sd, sd.nbc(x2, y2 - 1), bit)
                }
                5 => {
                    // The neighbouring 8x8 blocks, coded with the 8x8 transform (9.3.3.1.1.9).
                    let (x4, y4) = (2 * (idx & 1) as i32, 2 * (idx >> 1) as i32);
                    let bit = |b: usize| (52 + 4 * c + b8_of(b)) as u32;
                    Self::cbf_cond(sd, sd.nb4(x4 - 1, y4), bit)
                        + 2 * Self::cbf_cond(sd, sd.nb4(x4, y4 - 1), bit)
                }
                _ => {
                    let (x4, y4) = ((idx & 3) as i32, (idx >> 2) as i32);
                    let bit = |b: usize| (16 * c + b) as u32;
                    Self::cbf_cond(sd, sd.nb4(x4 - 1, y4), bit)
                        + 2 * Self::cbf_cond(sd, sd.nb4(x4, y4 - 1), bit)
                }
            };
            // ctxIdxOffset (Table 9-34) plus ctxIdxBlockCatOffset (Table 9-40).
            let base = match (c, cat) {
                (_, 5) => 1012 + 4 * c,
                (0, k) => 85 + 4 * k,
                (1, k) => 460 + 4 * k,
                (_, k) => 472 + 4 * k,
            };
            if self.c.decision(base + inc) == 0 {
                return Ok(0);
            }
        }
        // significant_coeff_flag, last_significant_coeff_flag and coeff_abs_level_minus1
        // offsets of the plane (Table 9-34), frame or field coded.
        let (sig, last, abs) = match (c, cat, sd.field) {
            (0, 5, false) => (402, 417, 426),
            (0, 5, true) => (436, 451, 426),
            (1, 5, false) => (660, 690, 708),
            (1, 5, true) => (675, 699, 708),
            (_, 5, false) => (718, 748, 766),
            (_, 5, true) => (733, 757, 766),
            (0, _, false) => (105 + CAT_SIG[cat], 166 + CAT_SIG[cat], 227 + CAT_ABS[cat]),
            (0, _, true) => (277 + CAT_SIG[cat], 338 + CAT_SIG[cat], 227 + CAT_ABS[cat]),
            (1, _, false) => (484 + CAT_SIG[cat], 572 + CAT_SIG[cat], 952 + CAT_ABS[cat]),
            (1, _, true) => (776 + CAT_SIG[cat], 864 + CAT_SIG[cat], 952 + CAT_ABS[cat]),
            (_, _, false) => (528 + CAT_SIG[cat], 616 + CAT_SIG[cat], 982 + CAT_ABS[cat]),
            (_, _, true) => (820 + CAT_SIG[cat], 908 + CAT_SIG[cat], 982 + CAT_ABS[cat]),
        };
        let sig8 = if sd.field { &SIG8X8_FIELD } else { &SIG8X8 };
        let mut pos = [0u8; 64];
        let mut n = 0usize;
        let mut ended = false;
        let c = &mut self.c;
        // NumC8x8 of the chroma DC block: 1 (4:2:0) or 2 (4:2:2).
        let numc8 = max / 4;
        for i in 0..max - 1 {
            let (si, li) = match cat {
                5 => (sig8[i] as usize, LAST8X8[i] as usize),
                3 => ((i / numc8).min(2), (i / numc8).min(2)),
                _ => (i, i),
            };
            if c.decision(sig + si) == 1 {
                pos[n] = i as u8;
                n += 1;
                if c.decision(last + li) == 1 {
                    ended = true;
                    break;
                }
            }
        }
        if !ended {
            pos[n] = (max - 1) as u8;
            n += 1;
        }
        let mut gt1 = 0usize;
        let mut eq1 = 0usize;
        let cap = if cat == 3 { 3 } else { 4 };
        for k in (0..n).rev() {
            let ctx0 = abs + if gt1 != 0 { 0 } else { (1 + eq1).min(4) };
            let v: u32 = if c.decision(ctx0) == 0 {
                1
            } else {
                let ctxr = abs + 5 + gt1.min(cap);
                let mut a = 2u32;
                while a < 15 && c.decision(ctxr) == 1 {
                    a += 1;
                }
                if a == 15 {
                    a = a.saturating_add(c.exp_golomb_bypass(0));
                }
                a
            };
            if v == 1 {
                eq1 += 1;
            } else {
                gt1 += 1;
            }
            let v = v.min(1 << 24) as i32;
            lv[pos[k] as usize] = if c.bypass() == 1 { -v } else { v };
        }
        Ok(n as u32)
    }

    fn pcm(&mut self, out: &mut [u16], depth: [u32; 2]) -> Res {
        let pos = self.c.pcm_pos();
        let data = self.c.data();
        let n = out.len();
        let bits = 256 * depth[0] as usize + (n - 256.min(n)) * depth[1] as usize;
        let bytes = bits.div_ceil(8);
        if pos + bytes > self.len {
            return Err(());
        }
        if depth == [8, 8] {
            for (v, &b) in out.iter_mut().zip(&data[pos..pos + n]) {
                *v = b as u16;
            }
        } else {
            let mut r = BitReader::new(data, self.len);
            r.set_pos(pos * 8);
            for (i, v) in out.iter_mut().enumerate() {
                *v = r.u(depth[(i >= 256) as usize]) as u16;
            }
        }
        self.c.start(pos + bytes);
        Ok(())
    }
}
