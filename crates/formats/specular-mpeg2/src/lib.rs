//! Specular's own MPEG-1 and MPEG-2 video decoder (ISO/IEC 11172-2, ISO/IEC 13818-2 /
//! ITU-T H.262).
//!
//! - MPEG-2 Simple, Main, High and 4:2:2 profiles at every level; MPEG-1 (including
//!   constrained-parameters and full-pel streams).
//! - I, P and B pictures; frame and field pictures; frame, field, 16×8 and dual-prime
//!   prediction; field DCT; 4:2:0, 4:2:2 and 4:4:4 chroma; both scans, both coefficient
//!   tables, linear and non-linear quantiser scales, quantiser matrix extensions.
//! - MPEG-1 D-pictures (DC coefficients only) decode; they stand alone, used by nothing.
//! - Scalable extensions (SNR, spatial, temporal) are refused.
//!
//! Input is the elementary stream in any pieces (a container's samples): pictures are cut at
//! start codes, and a picture ends at the next picture, sequence or GOP header, or where its
//! last macroblock row has arrived and the piece ends. Field pictures pair into frames.
//! Pictures come out in decoding order or in display order ([`OutputOrder`]), each tagged
//! with the tag of the piece its picture header came in (its first field's, for a field
//! pair).
//!
//! Macroblock rows (MPEG-2 slices never span rows) decode on worker threads. The inverse DCT
//! is the integer one libavcodec selects with `-idct int`, so output is bit-exact with
//! `ffmpeg -idct int`. Damaged slices are concealed from the reference picture.
#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod bits;
mod frame;
mod headers;
mod idct;
mod slice;
mod tables;
mod vlc;

/// Building blocks shared with the MPEG-4 Part 2 decoder (`specular-mpeg4`): the bit reader,
/// two-level VLC tables and the integer inverse DCT.
#[doc(hidden)]
pub mod util {
    pub use crate::bits::Bits;
    pub use crate::idct::idct;
    pub use crate::vlc::Vlc;
}

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub use headers::{PictureType, Sequence, Structure};

use frame::Frame;
use headers::PictureHeader;

/// Why a stream can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A valid stream using something this decoder doesn't do.
    Unsupported(String),
    /// The bitstream is damaged or breaks the format.
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "damaged MPEG video: {s}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// Chroma sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chroma {
    Yuv420,
    Yuv422,
    Yuv444,
}

impl Chroma {
    /// Horizontal and vertical subsampling shifts.
    pub fn shift(self) -> (u32, u32) {
        match self {
            Chroma::Yuv420 => (1, 1),
            Chroma::Yuv422 => (1, 0),
            Chroma::Yuv444 => (0, 0),
        }
    }

    /// Blocks per macroblock.
    pub(crate) fn blocks(self) -> usize {
        match self {
            Chroma::Yuv420 => 6,
            Chroma::Yuv422 => 8,
            Chroma::Yuv444 => 12,
        }
    }
}

/// The order pictures come out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputOrder {
    /// As each frame is finished (B pictures before the reference they precede in time).
    Decode,
    /// In time order.
    Display,
}

/// A decoded frame, cropped to the coded size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub chroma: Chroma,
    /// Y, Cb, Cr, rows tightly packed; chroma `ceil(width / 2)` wide unless 4:4:4, and
    /// `ceil(height / 2)` high in 4:2:0.
    pub planes: [Vec<u8>; 3],
    pub tag: i64,
    pub kind: PictureType,
    /// MPEG-1, or chroma co-sited with even luma columns (MPEG-2).
    pub mpeg1: bool,
    /// The frame's two fields are from the same instant (progressive_frame).
    pub progressive: bool,
    /// Coded as two field pictures.
    pub field_pictures: bool,
    pub top_field_first: bool,
    pub repeat_first_field: bool,
    pub temporal_reference: u16,
    /// Colour description of the sequence display extension: primaries, transfer, matrix.
    pub colour: Option<(u8, u8, u8)>,
}

/// The decoding state of a [`Decoder`] between two pictures: the sequence, the quantiser
/// matrices and the reference frames (shared, not copied). See [`Decoder::snapshot`].
#[derive(Clone)]
pub struct Snapshot {
    seq: Option<Sequence>,
    in_sequence: bool,
    quant: [[u8; 64]; 4],
    refs: [Option<Arc<Frame>>; 2],
    closed_gop: bool,
    need_intra: bool,
}

impl Snapshot {
    /// The reference frames held, each as (identity, bytes of samples): frames shared
    /// between snapshots (and the decoder) count once by identity.
    pub fn pictures(&self) -> Vec<(usize, u64)> {
        self.refs
            .iter()
            .flatten()
            .map(|f| {
                let bytes: usize = f.planes.iter().map(Vec::len).sum();
                (Arc::as_ptr(f) as usize, bytes as u64)
            })
            .collect()
    }
}

/// A frame being decoded: its buffer, and after a first field, that field's parity.
struct Current {
    frame: Frame,
    tag: i64,
    hdr: PictureHeader,
    first_parity: Option<u8>,
    anchor: bool,
}

/// An MPEG-1/2 video decoder for one stream.
pub struct Decoder {
    threads: usize,
    order: OutputOrder,
    seq: Option<Sequence>,
    /// A sequence header is being read: its extensions follow.
    in_sequence: bool,
    /// The quantiser matrices in force.
    quant: [[u8; 64]; 4],
    /// Unprocessed input from `head`; `base` is the stream position of `buf[0]`.
    buf: Vec<u8>,
    head: usize,
    base: u64,
    /// Stream positions where each piece began, with its tag.
    marks: VecDeque<(u64, i64)>,
    /// The picture whose slices are being gathered.
    open: Option<Open>,
    cur: Option<Current>,
    /// Older and newer reference frames.
    refs: [Option<Arc<Frame>>; 2],
    /// Display order: the newest reference frame, not yet output.
    held: Option<Picture>,
    out: VecDeque<Picture>,
    dropped: VecDeque<i64>,
    closed_gop: bool,
    /// After a reset: pictures are dropped until an I picture.
    need_intra: bool,
    /// A picture's coding this decoder refuses (reported once from `decode`).
    refusal: Option<Error>,
    /// Pieces end on picture boundaries (container samples).
    aligned: bool,
}

struct Open {
    hdr: PictureHeader,
    tag: i64,
    /// Slice units: (macroblock row, range in `data`).
    slices: Vec<(usize, std::ops::Range<usize>)>,
    data: Vec<u8>,
    last_row: usize,
}

impl Decoder {
    /// `threads`: worker threads (0 = available parallelism, capped at 8; 1 = none).
    pub fn new(threads: usize, order: OutputOrder) -> Decoder {
        let threads = if threads == 0 {
            std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
        } else {
            threads
        };
        Decoder {
            threads,
            order,
            seq: None,
            in_sequence: false,
            quant: [[16; 64]; 4],
            buf: Vec::new(),
            head: 0,
            base: 0,
            marks: VecDeque::new(),
            open: None,
            cur: None,
            refs: [None, None],
            held: None,
            out: VecDeque::new(),
            dropped: VecDeque::new(),
            closed_gop: false,
            need_intra: false,
            refusal: None,
            aligned: false,
        }
    }

    /// Pieces given to [`Decoder::decode`] end where pictures end (a container's samples):
    /// a picture whose last macroblock row has arrived is decoded at the end of its piece
    /// instead of when the next one starts.
    pub fn set_aligned(&mut self, aligned: bool) {
        self.aligned = aligned;
    }

    /// The sequence in force (after its header has been read).
    pub fn sequence(&self) -> Option<&Sequence> {
        self.seq.as_ref()
    }

    /// Takes the next piece of the stream. Errors only for codings this decoder refuses;
    /// damage costs pictures (see [`Decoder::next_dropped`]).
    pub fn decode(&mut self, data: &[u8], tag: i64) -> Result<(), Error> {
        let pos = self.base + self.buf.len() as u64;
        self.marks.push_back((pos, tag));
        self.buf.extend_from_slice(data);
        self.units(false);
        // A picture whose last row has arrived ends with the piece.
        if self.aligned
            && let Some(code) = self.buf.get(self.head + 3).copied()
            && self.buf[self.head..].starts_with(&[0, 0, 1])
        {
            let complete = match code {
                headers::SEQUENCE_END => true,
                1..=headers::SLICE_LAST => {
                    let row = self.slice_row(code);
                    self.open.as_ref().is_some_and(|o| row == Some(o.last_row))
                }
                _ => false,
            };
            if complete {
                self.units(true);
                self.finish_picture();
            }
        }
        self.compact();
        self.refusal.take().map_or(Ok(()), Err)
    }

    /// The end of the stream: everything decoded comes out.
    pub fn flush(&mut self) {
        self.units(true);
        self.finish_picture();
        self.finish_lone_field();
        if let Some(p) = self.held.take() {
            self.out.push_back(p);
        }
        self.compact();
    }

    /// Forgets references and pending input (the sequence header is kept); decoding goes on
    /// from the next I picture.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.head = 0;
        self.marks.clear();
        self.open = None;
        self.cur = None;
        self.refs = [None, None];
        self.held = None;
        self.out.clear();
        self.dropped.clear();
        self.need_intra = true;
        self.in_sequence = false;
    }

    pub fn next_picture(&mut self) -> Option<Picture> {
        self.out.pop_front()
    }

    /// The decoding state between two pictures, to come back to with
    /// [`Decoder::restore`]: a seek can resume from the snapshot taken nearest before its
    /// target instead of from the I picture before it. Cheap (the reference frames are
    /// shared, not copied), but it keeps them alive; [`Snapshot::pictures`] sizes them.
    /// `None` while a picture or a field pair is incomplete, input is pending, or pictures
    /// wait to be taken.
    pub fn snapshot(&self) -> Option<Snapshot> {
        let idle = self.open.is_none()
            && self.cur.is_none()
            && self.held.is_none()
            && self.out.is_empty()
            && self.buf.len() == self.head
            && self.refusal.is_none();
        idle.then(|| Snapshot {
            seq: self.seq.clone(),
            in_sequence: self.in_sequence,
            quant: self.quant,
            refs: self.refs.clone(),
            closed_gop: self.closed_gop,
            need_intra: self.need_intra,
        })
    }

    /// Continues as the decoder was when `snapshot` was taken: decoding the pieces that
    /// followed then gives the same pictures.
    pub fn restore(&mut self, snapshot: &Snapshot) {
        self.buf.clear();
        self.head = 0;
        self.marks.clear();
        self.open = None;
        self.cur = None;
        self.held = None;
        self.out.clear();
        self.dropped.clear();
        self.in_sequence = snapshot.in_sequence;
        self.refusal = None;
        self.seq = snapshot.seq.clone();
        self.quant = snapshot.quant;
        self.refs = snapshot.refs.clone();
        self.closed_gop = snapshot.closed_gop;
        self.need_intra = snapshot.need_intra;
    }

    /// Tags of pictures that will never come out (damaged, or missing their references).
    pub fn next_dropped(&mut self) -> Option<i64> {
        self.dropped.pop_front()
    }

    fn compact(&mut self) {
        if self.head > 0 {
            self.buf.drain(..self.head);
            self.base += self.head as u64;
            self.head = 0;
        }
        while self.marks.len() > 1 && self.marks[1].0 <= self.base {
            self.marks.pop_front();
        }
    }

    fn tag_at(&self, pos: u64) -> i64 {
        self.marks
            .iter()
            .rev()
            .find(|m| m.0 <= pos)
            .or(self.marks.front())
            .map_or(0, |m| m.1)
    }

    /// The macroblock row a slice start code gives, for the open picture.
    fn slice_row(&self, code: u8) -> Option<usize> {
        Some(usize::from(code) - 1)
    }

    /// Handles every complete unit; with `all`, the trailing one too.
    fn units(&mut self, all: bool) {
        loop {
            let b = &self.buf[self.head..];
            // The unit's start code.
            let Some(s) = find_start(b, 0) else {
                // No start code at all: keep the last bytes (a start code may straddle).
                let keep = b.len().min(3);
                self.head = self.buf.len() - keep;
                return;
            };
            if s > 0 {
                self.head += s;
                continue;
            }
            let end = match find_start(b, 4) {
                Some(e) => e,
                None if all && b.len() >= 4 => b.len(),
                None => return,
            };
            let code = b[3];
            let pos = self.base + self.head as u64;
            let tag = self.tag_at(pos);
            let start = self.head;
            let last = end == b.len();
            self.head += end;
            self.unit(code, start + 4, start + end, tag);
            if last {
                return;
            }
        }
    }

    fn unit(&mut self, code: u8, from: usize, to: usize, tag: i64) {
        match code {
            headers::PICTURE => {
                self.finish_picture();
                self.in_sequence = false;
                match headers::picture_header(&self.buf[from..to]) {
                    Ok(hdr) => {
                        let last_row = self.seq.as_ref().map_or(0, |s| s.mb_size().1 as usize - 1);
                        self.open = Some(Open {
                            hdr,
                            tag,
                            slices: Vec::new(),
                            data: Vec::new(),
                            last_row,
                        });
                    }
                    Err(_) => self.dropped.push_back(tag),
                }
            }
            1..=headers::SLICE_LAST => {
                if let Some(o) = self.open.as_mut() {
                    let at = o.data.len();
                    o.data.extend_from_slice(&self.buf[from..to]);
                    o.slices.push((usize::from(code) - 1, at..o.data.len()));
                }
            }
            headers::SEQUENCE => {
                self.finish_picture();
                match headers::sequence_header(&self.buf[from..to]) {
                    Ok(s) => {
                        if let Some(old) = &self.seq
                            && (old.width != s.width || old.height != s.height)
                        {
                            // A new size: the old references can't serve.
                            self.finish_lone_field();
                            self.refs = [None, None];
                            if let Some(p) = self.held.take() {
                                self.out.push_back(p);
                            }
                        }
                        let keep_ext = self
                            .seq
                            .as_ref()
                            .filter(|o| o.width == s.width && o.height == s.height);
                        let mut s = s;
                        if let Some(o) = keep_ext {
                            // Repeated headers: the extensions that follow restate these.
                            s.display = o.display;
                            s.colour = o.colour;
                        }
                        self.quant = s.quant;
                        self.seq = Some(s);
                        self.in_sequence = true;
                    }
                    Err(e) => {
                        specular_log_debug(&format!("sequence header: {e}"));
                    }
                }
            }
            headers::EXTENSION => {
                let data = &self.buf[from..to].to_vec();
                if let Some(o) = self.open.as_mut() {
                    if let Some(seq) = &self.seq {
                        let _ = headers::picture_extension(&mut o.hdr, &mut self.quant, data);
                        // Field pictures have half the rows.
                        let (_, h) = seq.mb_size();
                        o.last_row = if o.hdr.structure == Structure::Frame {
                            h as usize - 1
                        } else {
                            h as usize / 2 - 1
                        };
                    }
                } else if self.in_sequence
                    && let Some(seq) = self.seq.as_mut()
                {
                    let _ = headers::sequence_extension(seq, data);
                    self.quant = seq.quant;
                }
            }
            headers::GOP => {
                self.finish_picture();
                self.in_sequence = false;
                let (closed, _broken) = headers::gop_header(&self.buf[from..to]);
                self.closed_gop = closed;
            }
            headers::SEQUENCE_END => {
                self.finish_picture();
            }
            _ => {}
        }
    }

    /// A first field never paired: its frame comes out as it is.
    fn finish_lone_field(&mut self) {
        if let Some(c) = self.cur.take() {
            self.complete(c);
        }
    }

    /// Decodes the open picture, if any.
    fn finish_picture(&mut self) {
        let Some(open) = self.open.take() else {
            return;
        };
        let Some(seq) = self.seq.clone() else {
            self.dropped.push_back(open.tag);
            return;
        };
        let quant = self.quant;
        if let Err(e) = self.picture(&seq, &quant, open) {
            self.refusal.get_or_insert(e);
        }
    }

    fn picture(&mut self, seq: &Sequence, quant: &[[u8; 64]; 4], open: Open) -> Result<(), Error> {
        let hdr = &open.hdr;
        if seq.scalable || hdr.scalable {
            self.dropped.push_back(open.tag);
            return Err(Error::Unsupported("scalable MPEG-2 video".into()));
        }
        if seq.height > 2800 {
            self.dropped.push_back(open.tag);
            return Err(Error::Unsupported(format!(
                "MPEG-2 video {} lines high",
                seq.height
            )));
        }
        if seq.width > 8192 {
            self.dropped.push_back(open.tag);
            return Err(Error::Unsupported(format!(
                "MPEG-2 video {} pixels wide",
                seq.width
            )));
        }
        let (mb_w, mb_h) = seq.mb_size();
        let (mb_w, mb_h) = (mb_w as usize, mb_h as usize);
        let field = match hdr.structure {
            Structure::Top => Some(0u8),
            Structure::Bottom => Some(1),
            Structure::Frame => None,
        };
        // A second field: the opposite parity of the frame in progress.
        let second = match (&self.cur, field) {
            (Some(c), Some(p)) => c.first_parity.is_some_and(|fp| fp != p),
            _ => false,
        };
        if !second {
            self.finish_lone_field();
        }
        // D-pictures (MPEG-1, DC coefficients only) stand alone: they predict from nothing
        // and nothing predicts from them.
        let anchor = !matches!(hdr.kind, PictureType::B | PictureType::D);
        if !second {
            if self.need_intra && !matches!(hdr.kind, PictureType::I | PictureType::D) {
                self.dropped.push_back(open.tag);
                return Ok(());
            }
            // References: a P picture needs one, a B picture two (or a closed GOP).
            let ok = match hdr.kind {
                PictureType::P => self.refs[1].is_some(),
                PictureType::B => {
                    self.refs[1].is_some() && (self.refs[0].is_some() || self.closed_gop)
                }
                _ => true,
            };
            if !ok {
                self.dropped.push_back(open.tag);
                return Ok(());
            }
            self.need_intra &= hdr.kind == PictureType::D;
            if anchor {
                self.refs[0] = self.refs[1].take();
                if let Some(p) = self.held.take() {
                    self.out.push_back(p);
                }
            }
            self.cur = Some(Current {
                frame: Frame::new(mb_w, mb_h, seq.chroma),
                tag: open.tag,
                hdr: hdr.clone(),
                first_parity: field,
                anchor,
            });
        }
        let Some(mut cur) = self.cur.take() else {
            return Ok(());
        };
        // The references of this picture.
        let (fwd, bwd) = match hdr.kind {
            PictureType::P => (self.refs[0].clone(), None),
            PictureType::B => (self.refs[0].clone(), self.refs[1].clone()),
            _ => (None, None),
        };
        let snapshot = (second && hdr.kind == PictureType::P).then(|| cur.frame.clone());
        let mb_rows = if field.is_some() { mb_h / 2 } else { mb_h };
        let ctx = slice::Ctx {
            seq,
            pic: hdr,
            quant,
            mb_w,
            mb_rows,
            fwd: fwd.as_deref(),
            bwd: bwd.as_deref(),
            first_field: snapshot.as_ref().zip(cur.first_parity).map(|(f, p)| (p, f)),
        };
        let damaged = decode_slices(&ctx, &mut cur.frame, &open, self.threads);
        if damaged > 0 {
            specular_log_debug(&format!("{damaged} damaged slices"));
        }
        if field.is_some() && !second {
            // Wait for the other field.
            self.cur = Some(cur);
            return Ok(());
        }
        self.complete_with(cur, second);
        Ok(())
    }

    fn complete(&mut self, cur: Current) {
        self.complete_with(cur, false);
    }

    /// A finished frame: kept as a reference, and output.
    fn complete_with(&mut self, cur: Current, field_pair: bool) {
        let Some(seq) = &self.seq else {
            return;
        };
        let frame = Arc::new(cur.frame);
        let pic = output(
            seq,
            &frame,
            &cur.hdr,
            cur.tag,
            field_pair || cur.first_parity.is_some(),
        );
        if cur.anchor {
            self.refs[1] = Some(frame);
            match self.order {
                OutputOrder::Decode => self.out.push_back(pic),
                OutputOrder::Display => self.held = Some(pic),
            }
        } else {
            self.out.push_back(pic);
        }
    }
}

fn specular_log_debug(s: &str) {
    if cfg!(debug_assertions) && std::env::var_os("SPECULAR_MPEG2_DEBUG").is_some() {
        eprintln!("mpeg2: {s}");
    }
}

/// The position of the first start code (00 00 01) at or after `from`.
fn find_start(b: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= b.len() {
        // Skip quickly to a zero byte pair.
        match b[i + 2] {
            1 if b[i] == 0 && b[i + 1] == 0 => return Some(i),
            0 => i += 1,
            _ => i += 3,
        }
    }
    None
}

/// Decodes a picture's slices into `frame`, rows in parallel; returns the damaged count.
fn decode_slices(ctx: &slice::Ctx, frame: &mut Frame, open: &Open, threads: usize) -> usize {
    let (_, sy) = ctx.seq.chroma.shift();
    let field = ctx.pic.structure != Structure::Frame;
    // Frame lines per macroblock row (a field's rows interleave over twice as many).
    let luma_lines = if field { 32 } else { 16 };
    let rows_per_job = if ctx.seq.mpeg2 { 1 } else { ctx.mb_rows };
    let mut by_row: Vec<Vec<(usize, &[u8])>> = vec![Vec::new(); ctx.mb_rows.div_ceil(rows_per_job)];
    for (row, r) in &open.slices {
        if *row < ctx.mb_rows {
            by_row[row / rows_per_job].push((*row, &open.data[r.clone()]));
        }
    }
    let [py, pcb, pcr] = &mut frame.planes;
    let ys = frame.stride[0] * luma_lines * rows_per_job;
    let cs = frame.stride[1] * (luma_lines >> sy) * rows_per_job;
    let mut jobs: Vec<slice::Job> = Vec::with_capacity(by_row.len());
    let chunks = py
        .chunks_mut(ys)
        .zip(pcb.chunks_mut(cs))
        .zip(pcr.chunks_mut(cs));
    for (k, ((y, cb), cr)) in chunks.enumerate().take(by_row.len()) {
        let row0 = k * rows_per_job;
        jobs.push(slice::Job {
            row0,
            rows: rows_per_job.min(ctx.mb_rows - row0),
            slices: std::mem::take(&mut by_row[k]),
            out: [y, cb, cr],
        });
    }
    let n = threads.min(jobs.len());
    if n <= 1 {
        return jobs.iter_mut().map(|j| slice::decode_job(ctx, j)).sum();
    }
    let queue = Mutex::new(jobs.into_iter());
    let damaged = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for t in 0..n {
            let (queue, damaged) = (&queue, &damaged);
            specular_thread::spawn_scoped(scope, format!("mpeg2-row-{t}"), move || {
                loop {
                    let job = queue.lock().ok().and_then(|mut q| q.next());
                    let Some(mut job) = job else { break };
                    let d = slice::decode_job(ctx, &mut job);
                    damaged.fetch_add(d, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
    });
    damaged.into_inner()
}

/// A frame cropped to the coded size.
fn output(seq: &Sequence, f: &Frame, hdr: &PictureHeader, tag: i64, fields: bool) -> Picture {
    let (sx, sy) = seq.chroma.shift();
    let (w, h) = (seq.width as usize, seq.height as usize);
    let crop = |c: usize, pw: usize, ph: usize| {
        let mut v = Vec::with_capacity(pw * ph);
        for row in f.planes[c].chunks(f.stride[c]).take(ph) {
            v.extend_from_slice(&row[..pw]);
        }
        v
    };
    let (cw, ch) = ((w + (1 << sx) - 1) >> sx, (h + (1 << sy) - 1) >> sy);
    Picture {
        width: w as u32,
        height: h as u32,
        chroma: seq.chroma,
        planes: [crop(0, w, h), crop(1, cw, ch), crop(2, cw, ch)],
        tag,
        kind: hdr.kind,
        mpeg1: !seq.mpeg2,
        progressive: hdr.progressive_frame && !fields,
        field_pictures: fields,
        top_field_first: hdr.top_field_first,
        repeat_first_field: hdr.repeat_first_field,
        temporal_reference: hdr.temporal_reference,
        colour: seq.colour,
    }
}

/// One picture header found in a piece of the stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictureInfo {
    pub kind: PictureType,
    pub structure: Structure,
    pub temporal_reference: u16,
    pub progressive_frame: bool,
    pub top_field_first: bool,
}

/// What a piece of the stream holds: its sequence header (with extensions) if any, and its
/// picture headers in order. Reads headers only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub sequence: Option<Sequence>,
    pub pictures: Vec<PictureInfo>,
    /// A GOP header precedes the first picture.
    pub gop: bool,
    /// That GOP header says the GOP is closed (its B pictures predict only backwards).
    pub closed_gop: bool,
}

/// Scans `data` (a sample, or a stream's first bytes) for its headers.
pub fn scan(data: &[u8]) -> Scan {
    let mut out = Scan::default();
    let mut i = 0;
    let mut last: Option<u8> = None;
    let mut cur: Option<PictureHeader> = None;
    let flush = |cur: &mut Option<PictureHeader>, out: &mut Scan| {
        if let Some(h) = cur.take() {
            out.pictures.push(PictureInfo {
                kind: h.kind,
                structure: h.structure,
                temporal_reference: h.temporal_reference,
                progressive_frame: h.progressive_frame,
                top_field_first: h.top_field_first,
            });
        }
    };
    while let Some(s) = find_start(data, i) {
        let code = *data.get(s + 3).unwrap_or(&0xFF);
        let end = find_start(data, s + 4).unwrap_or(data.len());
        let body = &data[(s + 4).min(end)..end];
        match code {
            headers::SEQUENCE => {
                flush(&mut cur, &mut out);
                if out.sequence.is_none() {
                    out.sequence = headers::sequence_header(body).ok();
                }
            }
            headers::EXTENSION => {
                if let Some(h) = cur.as_mut() {
                    let mut q = out.sequence.as_ref().map_or([[16; 64]; 4], |s| s.quant);
                    let _ = headers::picture_extension(h, &mut q, body);
                } else if (last == Some(headers::SEQUENCE) || last == Some(headers::EXTENSION))
                    && let Some(s) = out.sequence.as_mut()
                {
                    let _ = headers::sequence_extension(s, body);
                }
            }
            headers::GOP => {
                flush(&mut cur, &mut out);
                if out.pictures.is_empty() {
                    out.gop = true;
                    out.closed_gop = headers::gop_header(body).0;
                }
            }
            headers::PICTURE => {
                flush(&mut cur, &mut out);
                cur = headers::picture_header(body).ok();
            }
            1..=headers::SLICE_LAST => {
                // Slices: nothing more to read until the next header.
                flush(&mut cur, &mut out);
            }
            _ => {}
        }
        if !(1..=headers::SLICE_LAST).contains(&code) {
            last = Some(code);
        }
        i = end;
        if i >= data.len() {
            break;
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// The bytes of the first sequence header in `data` with the extensions and user data after
/// it (up to the GOP or picture header that follows): a configuration record for
/// [`Decoder::decode`].
pub fn sequence_bytes(data: &[u8]) -> Option<&[u8]> {
    let mut i = 0;
    let start = loop {
        let s = find_start(data, i)?;
        if data.get(s + 3) == Some(&headers::SEQUENCE) {
            break s;
        }
        i = s + 3;
    };
    let mut j = start + 4;
    while let Some(s) = find_start(data, j) {
        match data.get(s + 3) {
            Some(&(headers::EXTENSION | 0xB2)) => j = s + 4,
            _ => return Some(&data[start..s]),
        }
    }
    Some(&data[start..])
}

/// Every start code in `data`: its position and code, in order.
pub fn start_codes(data: &[u8], mut f: impl FnMut(usize, u8)) {
    let mut i = 0;
    while let Some(s) = find_start(data, i) {
        if let Some(&c) = data.get(s + 3) {
            f(s, c);
        }
        i = s + 3;
    }
}

/// The sequence header (with its extensions) at the start of a stream or configuration
/// record, if any.
pub fn probe(data: &[u8]) -> Option<Sequence> {
    scan(data).sequence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_codes_are_found() {
        assert_eq!(find_start(&[0, 0, 1, 0xB3], 0), Some(0));
        assert_eq!(find_start(&[5, 0, 0, 0, 1, 0xB3], 0), Some(2));
        assert_eq!(find_start(&[0, 0, 1, 0xB3, 0, 0, 1, 0], 4), Some(4));
        assert_eq!(find_start(&[0, 0, 2, 0, 0], 0), None);
    }

    #[test]
    fn garbage_never_panics() {
        let mut d = Decoder::new(2, OutputOrder::Display);
        let mut seed = 7u32;
        let mut junk: Vec<u8> = (0..20000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        // A plausible sequence header and pictures inside the junk.
        junk[100..112].copy_from_slice(&[
            0, 0, 1, 0xB3, 0x02, 0x00, 0x20, 0x13, 0xFF, 0xFF, 0xE0, 0x00,
        ]);
        for k in 0..20 {
            junk[400 + k * 900..404 + k * 900].copy_from_slice(&[0, 0, 1, 0]);
            junk[420 + k * 900..424 + k * 900].copy_from_slice(&[0, 0, 1, 1 + k as u8]);
        }
        for chunk in junk.chunks(777) {
            let _ = d.decode(chunk, 0);
            while d.next_picture().is_some() {}
        }
        d.flush();
        while d.next_picture().is_some() {}
    }
}
