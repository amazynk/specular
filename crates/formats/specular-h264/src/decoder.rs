//! The decoder front end: NAL units, parameter sets, picture assembly, the worker pool and
//! picture output.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::bits::{BitReader, unescape};
use crate::dpb::{CurPic, Dpb, PocResult};
use crate::params::{Pps, SliceGroups, Sps, parse_pps, parse_sps};
use crate::picture::{PicShared, Structure};
use crate::pixel::Format;
use crate::slice::{SliceHeader, parse_slice_header, peek_pps_id};
use crate::slicedec::{BufPool, Partition, PicJob, RefPic, SliceJob, decode_picture};
use crate::{Error, Result, malformed};

/// Order in which pictures come out of [`Decoder::next_picture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OutputOrder {
    /// Display order (picture order count), with the reordering delay the stream signals
    /// (or its DPB size when it signals none).
    #[default]
    Display,
    /// Decoding order, as soon as each picture is decoded: for containers that carry
    /// presentation times and reorder themselves.
    Decode,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// Decoding threads; 0 picks the number of CPUs (at most 16), 1 decodes on the calling
    /// thread.
    pub threads: usize,
    pub output: OutputOrder,
}

/// The video signal type from the VUI, when the stream has one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signal {
    pub full_range: bool,
    /// colour_primaries, transfer_characteristics, matrix_coefficients.
    pub colour: Option<(u8, u8, u8)>,
}

/// Chroma sampling of a [`Picture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    /// Half width, half height (monochrome streams come out as 4:2:0 with mid-grey chroma).
    Yuv420,
    /// Half width, full height.
    Yuv422,
    /// Full width and height.
    Yuv444,
}

/// A decoded, cropped picture: a frame, a complementary field pair (interleaved) or a
/// non-paired field with each line repeated.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Bits per sample (the larger of the luma and chroma depths): 8, or 9..=14 with samples
    /// stored as little-endian `u16`.
    pub bit_depth: u8,
    pub chroma: Chroma,
    /// Y (width x height), Cb and Cr (ceil(width/2) x ceil(height/2) for 4:2:0,
    /// ceil(width/2) x height for 4:2:2, width x height for 4:4:4), tightly packed; two
    /// bytes per sample when
    /// `bit_depth` > 8.
    pub planes: [Vec<u8>; 3],
    pub poc: i32,
    /// The tag passed with the data holding the picture's first slice.
    pub tag: i64,
    pub signal: Option<Signal>,
}

#[derive(Clone)]
struct OutPic {
    pic: Arc<PicShared>,
    poc: i32,
    tag: i64,
    crop: [u32; 4],
    signal: Option<Signal>,
}

impl OutPic {
    fn picture(self) -> Picture {
        let (width, height, planes) = self.pic.planes(self.crop);
        let f = self.pic.fmt;
        Picture {
            width,
            height,
            bit_depth: f.depth[0].max(f.depth[1]),
            chroma: match f.cfi {
                2 => Chroma::Yuv422,
                3 => Chroma::Yuv444,
                _ => Chroma::Yuv420,
            },
            planes,
            poc: self.poc,
            tag: self.tag,
            signal: self.signal,
        }
    }
}

/// The picture being assembled from its slices.
struct Cur {
    sps: Arc<Sps>,
    hdr0: SliceHeader,
    /// first_mb_in_slice of the slices taken so far (they may come in any order).
    seen: Vec<u32>,
    /// MbToSliceGroupMap with slice groups (FMO).
    sg_map: Option<Arc<[u8]>>,
    poc: PocResult,
    shared: Arc<PicShared>,
    id: u32,
    tag: i64,
    /// Field parity of a field picture, and whether it is the second field of a frame.
    parity: Option<u8>,
    second: bool,
    slices: Vec<SliceJob>,
}

/// A decoded first field waiting for its second field.
struct Pending {
    out: OutPic,
    id: u32,
    parity: u8,
    frame_num: u32,
    is_ref: bool,
    idr: bool,
    width_mbs: u32,
    height_mbs: u32,
}

struct Pool {
    tx: Option<Sender<Box<PicJob>>>,
    handles: Vec<JoinHandle<()>>,
}

impl Pool {
    fn new(n: usize) -> Pool {
        let (tx, rx) = channel::<Box<PicJob>>();
        let rx = Arc::new(Mutex::new(rx));
        let handles = (0..n)
            .map(|i| {
                let rx: Arc<Mutex<Receiver<Box<PicJob>>>> = rx.clone();
                specular_thread::builder(format!("dec-h264-{i}"))
                    .spawn(move || {
                        loop {
                            let job = {
                                let g = rx.lock().unwrap_or_else(|e| e.into_inner());
                                g.recv()
                            };
                            match job {
                                Ok(j) => run(&j),
                                Err(_) => break,
                            }
                        }
                    })
                    .ok()
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        Pool {
            tx: Some(tx),
            handles,
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.tx = None;
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

/// Decodes one picture; a panic (a bug) leaves a concealed picture instead of a hang.
fn run(job: &PicJob) {
    let _ = catch_unwind(AssertUnwindSafe(|| decode_picture(job)));
    // `Work` publishes every row on drop, also while unwinding; should it never have been
    // created, readers still must not wait forever.
    let p = &job.shared;
    match job.parity {
        None => {
            p.fill_grey(0);
            p.fill_grey(1);
        }
        Some(q) => p.fill_grey(q as usize),
    }
}

/// The decoding state of a [`Decoder`] between two access units: parameter sets, the
/// decoded picture buffer with its reference marking and POC state, and the pictures due for
/// output. The pictures are shared, not copied. See [`Decoder::snapshot`].
#[derive(Clone)]
pub struct Snapshot {
    sps: Vec<Option<Arc<Sps>>>,
    pps: Vec<Option<Arc<Pps>>>,
    active: Option<Arc<Sps>>,
    dpb: Dpb,
    waiting: Vec<OutPic>,
    ready: VecDeque<OutPic>,
    next_id: u32,
    /// The plane decoders' snapshots (separate colour planes).
    split: Option<Arc<[Snapshot; 3]>>,
}

impl Snapshot {
    /// The pictures held, each as (identity, bytes): pictures shared between snapshots (and
    /// the decoder) count once by identity. The bytes count a picture's samples twice: its
    /// reference samples and its output frame.
    pub fn pictures(&self) -> Vec<(usize, u64)> {
        let mut v: Vec<(usize, u64)> = self
            .dpb
            .refs
            .iter()
            .map(|r| &r.pic)
            .chain(self.waiting.iter().map(|o| &o.pic))
            .chain(self.ready.iter().map(|o| &o.pic))
            .map(|p| {
                let f = p.fmt;
                let chroma = match f.cfi {
                    1 => 2 * 64,
                    2 => 2 * 128,
                    _ => 2 * 256,
                };
                let sample = if f.wide() { 2 } else { 1 };
                let bytes = (p.width_mbs * p.height_mbs * (256 + chroma) * sample * 2) as u64;
                (Arc::as_ptr(p) as usize, bytes)
            })
            .collect();
        for s in self.split.iter().flat_map(|s| s.iter()) {
            v.extend(s.pictures());
        }
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// Picture ids stay below 2^30 (`RefPic::id` holds `id << 2`).
const ID_LIMIT: u32 = 1 << 30;

/// The sample format of pictures of `sps`.
fn format_of(sps: &Sps) -> Format {
    Format {
        cfi: sps.chroma_format_idc.clamp(1, 3) as u8,
        depth: [sps.bit_depth_luma as u8, sps.bit_depth_chroma as u8],
    }
}

fn next_pic_id(id: u32) -> u32 {
    ((id + 1) % ID_LIMIT).max(1)
}

pub struct Decoder {
    opts: Options,
    threads: usize,
    sps: Vec<Option<Arc<Sps>>>,
    pps: Vec<Option<Arc<Pps>>>,
    active: Option<Arc<Sps>>,
    dpb: Dpb,
    cur: Option<Cur>,
    pool: Option<Pool>,
    /// Pictures being decoded, with the field parity of field pictures.
    inflight: VecDeque<(Arc<PicShared>, Option<u8>)>,
    pending: Option<Pending>,
    waiting: Vec<OutPic>,
    ready: VecDeque<OutPic>,
    next_id: u32,
    grey: Option<Arc<PicShared>>,
    length_size: usize,
    buf: Vec<u8>,
    /// Work areas of decoded pictures, reused by later ones.
    bufs: Arc<BufPool>,
    /// The decoded picture buffer of the active SPS (and the picture being decoded) in the
    /// memory budget: its size in bytes and the reservation.
    dpb_budget: Option<(u64, specular_core::Reservation)>,
    /// Separate colour planes: one monochrome decoder per colour_plane_id.
    split: Option<Box<Split>>,
    /// Joined pictures of separate colour planes due before the pictures of `ready`.
    joined: VecDeque<Picture>,
}

/// The three plane decoders of a separate-colour-plane stream (7.4.2.1.1: each colour plane
/// is coded as a monochrome picture with its own macroblocks, references and deblocking).
/// They see the same slice headers, so their reference marking and output order agree; their
/// pictures are joined into 4:4:4 ones.
struct Split {
    planes: [Decoder; 3],
    /// The parameter sets the planes' derived ones were made from.
    sps_from: Vec<Option<Arc<Sps>>>,
    pps_from: Vec<Option<Arc<Pps>>>,
}

impl Split {
    fn new(opts: Options, threads: usize) -> Split {
        let opts = Options {
            threads: threads.div_ceil(3),
            ..opts
        };
        Split {
            planes: std::array::from_fn(|_| Decoder::new(opts)),
            sps_from: vec![None; 32],
            pps_from: vec![None; 256],
        }
    }

    /// Gives the planes the monochrome forms of `sps` and `pps`, when they changed.
    fn load(&mut self, sps: &Arc<Sps>, pps: &Arc<Pps>) {
        let (si, pi) = (sps.id as usize, pps.id as usize);
        if !self.sps_from[si]
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, sps))
        {
            let mono = Arc::new(sps.plane_sps());
            for d in &mut self.planes {
                d.finish_picture();
                d.sps[si] = Some(mono.clone());
            }
            self.sps_from[si] = Some(sps.clone());
        }
        if !self.pps_from[pi]
            .as_ref()
            .is_some_and(|p| Arc::ptr_eq(p, pps))
        {
            for (k, d) in self.planes.iter_mut().enumerate() {
                d.finish_picture();
                d.pps[pi] = Some(Arc::new(pps.plane_pps(k)));
            }
            self.pps_from[pi] = Some(pps.clone());
        }
    }

    /// The next joined picture, when all three planes have theirs.
    fn next_picture(&mut self, wait: bool) -> Option<Picture> {
        let ok = self
            .planes
            .iter()
            .all(|d| d.ready.front().is_some_and(|f| wait || f.pic.is_done()));
        if !ok {
            return None;
        }
        let [y, cb, cr] = self.planes.each_mut().map(|d| d.next_picture(true));
        let (mut y, cb, cr) = (y?, cb?, cr?);
        let [py, _, _] = std::mem::take(&mut y.planes);
        y.planes = [py, take_luma(cb), take_luma(cr)];
        y.chroma = Chroma::Yuv444;
        Some(y)
    }
}

fn take_luma(p: Picture) -> Vec<u8> {
    let [l, _, _] = p.planes;
    l
}

/// Bytes of one decoded frame of `sps`: luma and both chroma planes.
fn frame_bytes(sps: &Sps) -> u64 {
    let chroma = match sps.chroma_format_idc {
        0 => 0,
        1 => 2 * 64,
        2 => 2 * 128,
        _ => 2 * 256,
    };
    let sample = if sps.bit_depth_luma > 8 || sps.bit_depth_chroma > 8 {
        2
    } else {
        1
    };
    u64::from(sps.width_mbs) * u64::from(sps.height_mbs) * (256 + chroma) * sample
}

impl Decoder {
    pub fn new(opts: Options) -> Decoder {
        let threads = match opts.threads {
            0 => std::thread::available_parallelism().map_or(1, |n| n.get().min(16)),
            n => n.min(64),
        };
        Decoder {
            opts,
            threads,
            sps: vec![None; 32],
            pps: vec![None; 256],
            active: None,
            dpb: Dpb::default(),
            cur: None,
            pool: None,
            inflight: VecDeque::new(),
            pending: None,
            waiting: Vec::new(),
            ready: VecDeque::new(),
            next_id: 1,
            grey: None,
            length_size: 4,
            buf: Vec::new(),
            // One per picture that can be decoding at once (`submit`).
            bufs: Arc::new(BufPool::new(threads + 1)),
            dpb_budget: None,
            split: None,
            joined: VecDeque::new(),
        }
    }

    /// Ends the separate-colour-plane decoders, keeping their pictures for output.
    fn flush_split(&mut self) {
        if let Some(mut s) = self.split.take() {
            for d in &mut s.planes {
                d.flush();
            }
            while let Some(p) = s.next_picture(true) {
                self.joined.push_back(p);
            }
        }
    }

    /// Reserves the DPB of `sps` (its frames and the one being decoded) from the memory
    /// budget, once per picture size.
    fn reserve_dpb(&mut self, sps: &Sps) -> Result<()> {
        let bytes = frame_bytes(sps).saturating_mul(u64::from(sps.dpb_frames()) + 1);
        if self.dpb_budget.as_ref().is_none_or(|(b, _)| *b != bytes) {
            // The old size's buffers go as the new one's come.
            self.dpb_budget = None;
            self.dpb_budget = Some((bytes, specular_core::budget::reserve(bytes)?));
        }
        Ok(())
    }

    /// A decoder for an MP4 track with this `avcC` record: its parameter sets loaded and its
    /// NAL length size used by [`Decoder::decode_sample`].
    pub fn with_avcc(avcc: &[u8], opts: Options) -> Result<Decoder> {
        let (length_size, nals) = parse_avcc(avcc)?;
        let mut d = Decoder::new(opts);
        d.length_size = length_size;
        for n in nals {
            d.decode_nal(n, 0)?;
        }
        Ok(d)
    }

    /// Decodes one MP4 sample (an access unit of length-prefixed NAL units).
    pub fn decode_sample(&mut self, sample: &[u8], tag: i64) -> Result<()> {
        let ls = self.length_size;
        let mut p = 0;
        let mut first_err = None;
        while p + ls <= sample.len() {
            let n = sample[p..p + ls]
                .iter()
                .fold(0usize, |a, &b| (a << 8) | b as usize);
            p += ls;
            if n > sample.len() - p {
                first_err.get_or_insert(malformed("NAL unit length past the end of the sample"));
                break;
            }
            match self.decode_nal(&sample[p..p + n], tag) {
                Ok(()) => {}
                Err(e @ (Error::Unsupported(_) | Error::NeedsMemory(_))) => return Err(e),
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
            p += n;
        }
        self.finish_picture();
        first_err.map_or(Ok(()), Err)
    }

    /// Decodes an Annex B byte stream chunk (start-code delimited NAL units). Pictures are
    /// completed by the next one's first slice or by [`Decoder::flush`].
    pub fn decode_annexb(&mut self, data: &[u8], tag: i64) -> Result<()> {
        let mut first_err = None;
        for nal in split_annexb(data) {
            match self.decode_nal(nal, tag) {
                Ok(()) => {}
                Err(e @ (Error::Unsupported(_) | Error::NeedsMemory(_))) => return Err(e),
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        first_err.map_or(Ok(()), Err)
    }

    /// Decodes one NAL unit (with its header byte, without start code or length).
    pub fn decode_nal(&mut self, nal: &[u8], tag: i64) -> Result<()> {
        let Some(&h) = nal.first() else {
            return Ok(());
        };
        if h & 0x80 != 0 {
            return Err(malformed("forbidden_zero_bit set"));
        }
        let ref_idc = (h >> 5) & 3;
        let ty = h & 31;
        match ty {
            // Data partitioning: partition A here, B and C in `partition` (checked against
            // the JM reference decoder: tools/corpus/h264-jm-dp.sh and CI1_FT_B).
            1 | 2 | 5 => self.slice(nal, ty, ref_idc, tag),
            3 | 4 => self.partition(nal, ty),
            7 => {
                self.finish_picture();
                let mut b = std::mem::take(&mut self.buf);
                let len = unescape(&nal[1..], &mut b);
                let r = parse_sps(&b, len);
                self.buf = b;
                let s = r?;
                let id = s.id as usize;
                self.sps[id] = Some(Arc::new(s));
                Ok(())
            }
            8 => {
                self.finish_picture();
                let mut b = std::mem::take(&mut self.buf);
                let len = unescape(&nal[1..], &mut b);
                let r = parse_pps(&b, len, &self.sps);
                self.buf = b;
                let p = r?;
                let id = p.id as usize;
                self.pps[id] = Some(Arc::new(p));
                Ok(())
            }
            6 | 9..=11 | 13..=18 => {
                // These start a new access unit (7.4.1.2.3).
                self.finish_picture();
                Ok(())
            }
            // Filler, auxiliary pictures and the SVC/MVC extensions (the base layer/view
            // decodes on its own).
            _ => Ok(()),
        }
    }

    fn slice(&mut self, nal: &[u8], ty: u8, ref_idc: u8, tag: i64) -> Result<()> {
        let mut rbsp = Vec::new();
        let len = unescape(&nal[1..], &mut rbsp);
        let (_, _, pps_id) =
            peek_pps_id(&rbsp, len).ok_or_else(|| malformed("truncated slice header"))?;
        let pps = self
            .pps
            .get(pps_id as usize)
            .and_then(|p| p.clone())
            .ok_or_else(|| malformed("slice refers to a missing PPS"))?;
        let sps = self.sps[pps.sps_id as usize]
            .clone()
            .ok_or_else(|| malformed("slice refers to a missing SPS"))?;
        if let Some(why) = sps.unsupported() {
            return Err(Error::Unsupported(why));
        }
        if sps.split_planes() {
            // A picture of a stream without separate colour planes ends here.
            self.finish_own();
            let (opts, threads) = (self.opts, self.threads);
            let split = self
                .split
                .get_or_insert_with(|| Box::new(Split::new(opts, threads)));
            split.load(&sps, &pps);
            let hdr = parse_slice_header(&rbsp, len, ty.min(5), ref_idc, &sps, &pps)?;
            return split.planes[hdr.colour_plane as usize].decode_nal(nal, tag);
        }
        if self.split.is_some() {
            // Back to a stream without separate colour planes: the planes' pictures go
            // out first.
            self.flush_split();
        }
        // Partition A: the slice header, then slice_id.
        let mut hdr = parse_slice_header(&rbsp, len, ty.min(5), ref_idc, &sps, &pps)?;
        let slice_id = if ty == 2 {
            if pps.cabac {
                return Err(malformed("data partitioning with CABAC"));
            }
            let mut r = BitReader::new(&rbsp, len);
            r.set_pos(hdr.data_bit);
            let id = r.ue();
            if r.overrun() {
                return Err(malformed("truncated slice data partition A"));
            }
            hdr.data_bit = r.pos();
            Some(id)
        } else {
            None
        };
        if hdr.redundant_pic_cnt > 0 {
            return Ok(());
        }
        if hdr.switching != crate::slice::Switching::None {
            return Err(Error::Unsupported("H.264 SP/SI slices".into()));
        }
        let new = match &self.cur {
            None => true,
            Some(c) => {
                !Arc::ptr_eq(&c.sps, &sps)
                    || first_slice_of_new_picture(&c.hdr0, &hdr, &sps, c.seen.contains(&0))
            }
        };
        if new {
            self.finish_picture();
            self.reserve_dpb(&sps)?;
            self.start_picture(sps, hdr.clone(), tag);
            if let (Some(c), Some(g)) = (self.cur.as_mut(), &pps.slice_groups) {
                c.sg_map = Some(mb_slice_group_map(&c.sps, &hdr, g));
            }
        } else if let Some(c) = &mut self.cur {
            // Slices may come in any order (ASO); a repeated one is dropped.
            if c.seen.contains(&hdr.first_mb) {
                return Ok(());
            }
            c.seen.push(hdr.first_mb);
        }
        let Some(sps) = self.cur.as_ref().map(|c| c.sps.clone()) else {
            return Ok(());
        };
        let grey = self.grey_for(&sps);
        let c = self.cur.as_mut().expect("current picture");
        let fallback = RefPic {
            pic: grey,
            id: u32::MAX,
            poc: 0,
            pocs: [0; 2],
            long: false,
            parity: hdr.parity(),
        };
        let lists = self.dpb.lists(&c.sps, &hdr, c.poc.poc, &fallback);
        c.slices.push(SliceJob {
            hdr,
            pps,
            rbsp,
            len,
            lists,
            slice_id,
            parts: [None, None],
        });
        Ok(())
    }

    /// Slice data partition B (`ty` 3) or C (4): attached to the partition A with the same
    /// slice_id in the current picture.
    fn partition(&mut self, nal: &[u8], ty: u8) -> Result<()> {
        let Some(c) = self.cur.as_mut() else {
            return Ok(());
        };
        let mut rbsp = Vec::new();
        let len = unescape(&nal[1..], &mut rbsp);
        let mut r = BitReader::new(&rbsp, len);
        let id = r.ue();
        let Some(job) = c.slices.iter_mut().rev().find(|s| s.slice_id == Some(id)) else {
            // Its partition A was lost (or redundant): nothing to attach it to.
            return Ok(());
        };
        if job.pps.redundant_pic_cnt_present && r.ue() > 0 {
            return Ok(());
        }
        if r.overrun() {
            return Err(malformed("truncated slice data partition"));
        }
        job.parts[ty as usize - 3] = Some(Partition {
            data_bit: r.pos(),
            rbsp,
            len,
        });
        Ok(())
    }

    fn grey_for(&mut self, sps: &Sps) -> Arc<PicShared> {
        let (w, h) = (sps.width_mbs as usize, sps.height_mbs as usize);
        let fmt = format_of(sps);
        match &self.grey {
            Some(g) if g.width_mbs == w && g.height_mbs == h && g.fmt == fmt => g.clone(),
            _ => {
                let g = Arc::new(PicShared::grey(w, h, fmt));
                self.grey = Some(g.clone());
                g
            }
        }
    }

    fn start_picture(&mut self, sps: Arc<Sps>, hdr: SliceHeader, tag: i64) {
        // The second field of the pending first field: same frame, opposite parity.
        let second = match (&self.pending, hdr.parity()) {
            (Some(p), Some(par)) => {
                par != p.parity
                    && hdr.frame_num == p.frame_num
                    && (hdr.nal_ref_idc != 0) == p.is_ref
                    && (!hdr.idr || p.idr)
                    && sps.width_mbs == p.width_mbs
                    && sps.height_mbs == p.height_mbs
                    && format_of(&sps) == p.out.pic.fmt
            }
            _ => false,
        };
        if !second {
            self.close_pending();
        }
        let resized = self.active.as_ref().is_some_and(|a| {
            a.width_mbs != sps.width_mbs
                || a.height_mbs != sps.height_mbs
                || a.max_num_ref_frames != sps.max_num_ref_frames
                || format_of(a) != format_of(&sps)
        });
        if resized {
            self.bump_all();
            self.dpb.clear();
        }
        self.active = Some(sps.clone());
        if second {
            // Same frame_num as the first field: neither a new IDR nor a gap.
        } else if hdr.idr {
            self.dpb.clear();
        } else {
            let max = 1u32 << sps.log2_max_frame_num;
            let prev = self.dpb.prev_ref_frame_num;
            if hdr.frame_num != prev
                && hdr.frame_num != (prev + 1) % max
                && !self.dpb.refs.is_empty()
            {
                let mut id = self.next_id;
                self.dpb.fill_gap(&sps, hdr.frame_num, || {
                    id = next_pic_id(id);
                    id
                });
                self.next_id = next_pic_id(id);
            }
        }
        let poc = self.dpb.compute_poc(&sps, &hdr);
        let (shared, id) = match &self.pending {
            Some(p) if second => (p.out.pic.clone(), p.id),
            _ => {
                let structure = if hdr.field_pic {
                    Structure::Field
                } else if sps.mbaff {
                    Structure::Mbaff
                } else {
                    Structure::Frame
                };
                let shared = Arc::new(PicShared::new(
                    sps.width_mbs as usize,
                    sps.height_mbs as usize,
                    structure,
                    format_of(&sps),
                ));
                let id = self.next_id;
                self.next_id = next_pic_id(self.next_id);
                (shared, id)
            }
        };
        self.cur = Some(Cur {
            parity: hdr.parity(),
            second,
            sps,
            seen: vec![hdr.first_mb],
            sg_map: None,
            hdr0: hdr,
            poc,
            shared,
            id,
            tag,
            slices: Vec::new(),
        });
    }

    /// Hands the assembled picture to a worker and does its reference marking and output.
    fn finish_picture(&mut self) {
        if let Some(s) = &mut self.split {
            for d in &mut s.planes {
                d.finish_picture();
            }
        }
        self.finish_own();
    }

    /// [`Decoder::finish_picture`] for this decoder's own picture, not the colour planes'.
    fn finish_own(&mut self) {
        let Some(c) = self.cur.take() else {
            return;
        };
        let job = Box::new(PicJob {
            sps: c.sps.clone(),
            shared: c.shared.clone(),
            parity: c.parity,
            poc: c.poc.poc,
            pocs: [c.poc.top, c.poc.bottom],
            is_ref: c.hdr0.nal_ref_idc != 0,
            slices: c.slices,
            sg_map: c.sg_map,
            pool: self.bufs.clone(),
        });
        self.submit(job);
        let sps = c.sps;
        let h = &c.hdr0;
        let out_poc = self.dpb.update_poc(&sps, h, &c.poc);
        if h.idr || h.has_mmco5() {
            self.bump_all();
        }
        self.dpb.mark(
            &sps,
            h,
            CurPic {
                pic: c.shared.clone(),
                id: c.id,
                parity: c.parity,
                second: c.second,
                poc: out_poc,
            },
        );
        let poc = match c.parity {
            None => out_poc[0].min(out_poc[1]),
            Some(p) => out_poc[p as usize],
        };
        let mut out = OutPic {
            pic: c.shared,
            poc,
            tag: c.tag,
            crop: sps.crop,
            signal: sps.vui.signal.then_some(Signal {
                full_range: sps.vui.full_range,
                colour: sps.vui.colour,
            }),
        };
        if let Some(par) = c.parity {
            if c.second {
                if let Some(first) = self.pending.take() {
                    out.poc = out.poc.min(first.out.poc);
                    out.tag = first.out.tag;
                }
            } else {
                self.pending = Some(Pending {
                    out,
                    id: c.id,
                    parity: par,
                    frame_num: h.frame_num,
                    is_ref: h.nal_ref_idc != 0,
                    idr: h.idr,
                    width_mbs: sps.width_mbs,
                    height_mbs: sps.height_mbs,
                });
                return;
            }
        }
        self.push_out(out, sps.reorder_depth() as usize);
    }

    /// Outputs a first field that never got its second field, as a frame of its lines
    /// repeated.
    fn close_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            p.out.pic.set_lone(p.parity as usize);
            let depth = self
                .active
                .as_ref()
                .map_or(0, |s| s.reorder_depth() as usize);
            self.push_out(p.out, depth);
        }
    }

    fn push_out(&mut self, out: OutPic, depth: usize) {
        match self.opts.output {
            OutputOrder::Decode => self.ready.push_back(out),
            OutputOrder::Display => {
                self.waiting.push(out);
                while self.waiting.len() > depth {
                    self.bump_one();
                }
            }
        }
    }

    fn bump_one(&mut self) {
        if let Some(i) = (0..self.waiting.len()).min_by_key(|&i| self.waiting[i].poc) {
            let p = self.waiting.remove(i);
            self.ready.push_back(p);
        }
    }

    fn bump_all(&mut self) {
        while !self.waiting.is_empty() {
            self.bump_one();
        }
    }

    fn submit(&mut self, job: Box<PicJob>) {
        if self.threads <= 1 {
            run(&job);
            return;
        }
        while self.inflight.front().is_some_and(|(p, par)| done(p, *par)) {
            self.inflight.pop_front();
        }
        while self.inflight.len() > self.threads {
            if let Some((p, par)) = self.inflight.pop_front() {
                wait(&p, par);
            }
        }
        let threads = self.threads;
        let pool = self.pool.get_or_insert_with(|| Pool::new(threads));
        self.inflight.push_back((job.shared.clone(), job.parity));
        let job = match pool.tx.as_ref() {
            Some(tx) if !pool.handles.is_empty() => match tx.send(job) {
                Ok(()) => return,
                Err(e) => e.0,
            },
            _ => job,
        };
        // No worker threads (spawning failed): decode here from now on.
        self.threads = 1;
        run(&job);
    }

    /// The next picture in output order. With `wait` false, `None` also when it is still
    /// decoding.
    pub fn next_picture(&mut self, wait: bool) -> Option<Picture> {
        if let Some(p) = self.joined.pop_front() {
            return Some(p);
        }
        if self.ready.is_empty()
            && let Some(s) = &mut self.split
        {
            return s.next_picture(wait);
        }
        let front = self.ready.front()?;
        if !wait && !front.pic.is_done() {
            return None;
        }
        self.ready.pop_front().map(OutPic::picture)
    }

    /// Ends the stream: completes the current picture and releases every picture waiting
    /// for output.
    pub fn flush(&mut self) {
        if let Some(s) = &mut self.split {
            for d in &mut s.planes {
                d.flush();
            }
        }
        self.finish_picture();
        self.close_pending();
        self.bump_all();
        self.bufs.clear();
    }

    /// Back to the state after the parameter sets: for seeking to the start of a stream.
    pub fn reset(&mut self) {
        self.split = None;
        self.joined.clear();
        self.cur = None;
        self.pending = None;
        for (p, par) in self.inflight.drain(..) {
            wait(&p, par);
        }
        self.dpb.clear();
        self.waiting.clear();
        self.ready.clear();
        self.active = None;
        self.bufs.clear();
    }

    /// The decoding state between two access units, to come back to with
    /// [`Decoder::restore`]: a seek can resume from the snapshot taken nearest before its
    /// target instead of from the sync sample before it. Cheap (the pictures are shared, not
    /// copied), but it keeps the pictures it holds alive; [`Snapshot::pictures`] sizes them.
    /// `None` while a picture or a field pair is incomplete.
    pub fn snapshot(&self) -> Option<Snapshot> {
        if self.cur.is_some() || self.pending.is_some() || !self.joined.is_empty() {
            return None;
        }
        let split = match &self.split {
            Some(s) => {
                let [a, b, c] = s.planes.each_ref().map(|d| d.snapshot());
                Some(Arc::new([a?, b?, c?]))
            }
            None => None,
        };
        Some(Snapshot {
            split,
            sps: self.sps.clone(),
            pps: self.pps.clone(),
            active: self.active.clone(),
            dpb: self.dpb.clone(),
            waiting: self.waiting.clone(),
            ready: self.ready.clone(),
            next_id: self.next_id,
        })
    }

    /// Continues as the decoder was when `snapshot` was taken: decoding the access units
    /// that followed then gives the same pictures. Pictures still decoding finish on their
    /// threads (the snapshot's pictures among them); the threads are kept.
    pub fn restore(&mut self, snapshot: &Snapshot) {
        self.cur = None;
        self.pending = None;
        for (p, par) in self.inflight.drain(..) {
            wait(&p, par);
        }
        self.sps.clone_from(&snapshot.sps);
        self.pps.clone_from(&snapshot.pps);
        self.active.clone_from(&snapshot.active);
        self.dpb = snapshot.dpb.clone();
        self.waiting.clone_from(&snapshot.waiting);
        self.ready.clone_from(&snapshot.ready);
        self.next_id = snapshot.next_id;
        self.joined.clear();
        match &snapshot.split {
            Some(snaps) => {
                let (opts, threads) = (self.opts, self.threads);
                let split = self
                    .split
                    .get_or_insert_with(|| Box::new(Split::new(opts, threads)));
                for (d, sn) in split.planes.iter_mut().zip(snaps.iter()) {
                    d.restore(sn);
                }
            }
            None => self.split = None,
        }
    }

    /// The NAL length size from the `avcC` record.
    pub fn length_size(&self) -> usize {
        self.length_size
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        for (p, par) in self.inflight.drain(..) {
            wait(&p, par);
        }
    }
}

/// Whether the picture (or field `parity` of it) is decoded.
fn done(p: &PicShared, parity: Option<u8>) -> bool {
    match parity {
        None => p.is_done(),
        Some(q) => p.field_done(q as usize),
    }
}

fn wait(p: &PicShared, parity: Option<u8>) {
    match parity {
        None => p.wait_done(),
        Some(q) => p.wait_field(q as usize),
    }
}

/// 7.4.1.2.4: whether `h` starts a new primary coded picture after `prev`.
/// Whether slice `h` starts a new picture after `prev` (7.4.1.2.4). Slices may come in any
/// order (ASO), so a first macroblock of 0 starts one only when the picture already has it.
pub(crate) fn first_slice_of_new_picture(
    prev: &SliceHeader,
    h: &SliceHeader,
    sps: &Sps,
    has_first_mb0: bool,
) -> bool {
    (h.first_mb == 0 && has_first_mb0)
        || prev.frame_num != h.frame_num
        || prev.field_pic != h.field_pic
        || prev.bottom_field != h.bottom_field
        || prev.pps_id != h.pps_id
        || (prev.nal_ref_idc == 0) != (h.nal_ref_idc == 0)
        || prev.idr != h.idr
        || (h.idr && prev.idr_pic_id != h.idr_pic_id)
        || (sps.poc_type == 0
            && (prev.poc_lsb != h.poc_lsb || prev.delta_poc_bottom != h.delta_poc_bottom))
        || (sps.poc_type == 1 && prev.delta_poc != h.delta_poc)
}

/// MbToSliceGroupMap of a picture (8.2.2.8) with slice groups `g`.
fn mb_slice_group_map(sps: &Sps, hdr: &SliceHeader, g: &SliceGroups) -> Arc<[u8]> {
    let w = sps.width_mbs;
    let units_h = sps.height_mbs / if sps.frame_mbs_only { 1 } else { 2 };
    let units = g.unit_map(w, units_h, hdr.slice_group_change_cycle);
    let n = (sps.width_mbs * sps.height_mbs / if hdr.field_pic { 2 } else { 1 }) as usize;
    let at = |u: usize| units.get(u).copied().unwrap_or(0);
    if sps.frame_mbs_only || hdr.field_pic {
        (0..n).map(at).collect()
    } else if sps.mbaff {
        (0..n).map(|i| at(i / 2)).collect()
    } else {
        let w = w as usize;
        (0..n).map(|i| at((i / (2 * w)) * w + i % w)).collect()
    }
}

/// Parses an `avcC` record: the NAL length size and its SPS and PPS NAL units.
pub fn parse_avcc(c: &[u8]) -> Result<(usize, Vec<&[u8]>)> {
    let bad = || malformed("truncated avcC record");
    if c.len() < 6 || c[0] != 1 {
        return Err(malformed("not an avcC record"));
    }
    let length_size = (c[4] & 3) as usize + 1;
    let mut p = 5;
    let mut nals = Vec::new();
    for k in 0..2 {
        let n = if k == 0 {
            let n = (*c.get(p).ok_or_else(bad)? & 31) as usize;
            p += 1;
            n
        } else {
            let n = *c.get(p).ok_or_else(bad)? as usize;
            p += 1;
            n
        };
        for _ in 0..n {
            let l = u16::from_be_bytes([*c.get(p).ok_or_else(bad)?, *c.get(p + 1).ok_or_else(bad)?])
                as usize;
            p += 2;
            let nal = c.get(p..p + l).ok_or_else(bad)?;
            nals.push(nal);
            p += l;
        }
    }
    Ok((length_size, nals))
}

/// The NAL units of an Annex B byte stream (between `00 00 01` start codes, trailing zero
/// bytes removed).
pub fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut start: Option<usize> = None;
    let n = data.len();
    while i + 2 < n {
        if data[i + 2] > 1 {
            i += 3;
            continue;
        }
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            if let Some(s) = start {
                let mut e = i;
                while e > s && data[e - 1] == 0 {
                    e -= 1;
                }
                out.push(&data[s..e]);
            }
            i += 3;
            start = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(s) = start {
        let mut e = n;
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        out.push(&data[s..e]);
    }
    out.retain(|n| !n.is_empty());
    out
}
