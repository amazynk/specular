//! MPEG-2, H.264 and HEVC pixel data (PS3.5 8.2.5–8.2.8): the video stream in the fragments,
//! decoded by the in-tree `specular-mpeg2`, `specular-h264` and `specular-hevc` decoders, one
//! frame per picture (an MPEG-2 field pair is one frame).
//!
//! An MPEG-2 stream is cut at its start codes: a picture (with the sequence and GOP headers
//! before it, and its second field) is an access unit, and decoding can start at an I picture
//! that follows a sequence header (after the B pictures of an open GOP that precede it).
//!
//! [`Video::index`] splits the Annex B stream into NAL units and access units and finds the
//! points decoding can start at (the first picture, IDR pictures, and HEVC CRA / BLA pictures
//! whose leading pictures are all of one kind). A frame is reached by continuing the last
//! decode when that is cheaper, else by restarting at the nearest of those points; a bounded
//! cache keeps recently decoded frames so stepping back a few frames does not decode from the
//! restart point again. [`Sequence`] decodes front to back with its own decoder for cine.
//!
//! The decoded YCbCr is converted to RGB here: the matrix from the stream's VUI (BT.601 / 709
//! / 2020, else 709 above 576 lines), limited or full range, chroma interpolated at MPEG
//! siting (co-sited horizontally, centred vertically), in row bands on a few threads.
//! Pictures larger or smaller than Rows x Columns are cropped or edge-extended to it.
//!
//! SMPTE ST 2110 video is refused with a reason, as is a stream that turns out to be MPEG-2
//! or MPEG-4 Part 2 under an H.264 / HEVC label, or something else under an MPEG-2 one.

use std::collections::{HashSet, VecDeque};
use std::ops::Range;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::parse::{Fragments, Values};
use crate::pixel::{Frame, Grey, PixelInfo, Pixels};
use crate::ts::TransferSyntax;
use crate::{Error, Result, cancelled};

/// The video codecs this crate decodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Mpeg2,
    H264,
    Hevc,
}

impl Codec {
    pub fn name(self) -> &'static str {
        match self {
            Codec::Mpeg2 => "MPEG-2",
            Codec::H264 => "H.264",
            Codec::Hevc => "HEVC",
        }
    }
}

const VIDEO_PREFIX: &str = "1.2.840.10008.1.2.4.";

/// The codec of a video transfer syntax this crate decodes.
pub fn codec(ts: &TransferSyntax) -> Option<Codec> {
    let rest = ts.uid.strip_prefix(VIDEO_PREFIX)?;
    let (n, frag) = match rest.split_once('.') {
        Some((n, "1")) => (n, true),
        Some(_) => return None,
        None => (rest, false),
    };
    match n.parse::<u32>().ok()? {
        100 | 101 => Some(Codec::Mpeg2),
        102..=106 => Some(Codec::H264),
        107 | 108 if !frag => Some(Codec::Hevc),
        _ => None,
    }
}

/// Why video pixel data of `ts` is not decoded (for syntaxes [`codec`] does not take).
pub(crate) fn refusal(ts: &TransferSyntax) -> String {
    if ts.uid.starts_with("1.2.840.10008.1.2.7.") {
        format!("{} pixel data", ts.name)
    } else {
        format!("{} video pixel data", ts.name)
    }
}

/// Bytes kept in the frame cache of one file (at least two frames are always kept).
const CACHE_BYTES: usize = 128 << 20;

/// Where the stream is: one fragment in the data set buffer, or several joined.
enum Data {
    Borrowed(Range<usize>),
    Owned(Vec<u8>),
}

/// One access unit (one coded picture: a frame, or a field of an H.264 field pair).
#[derive(Clone, Debug)]
struct Au {
    /// Its NAL units (indices into [`Video::nals`]).
    nals: Range<usize>,
    /// The first VCL NAL unit.
    first_vcl: usize,
}

/// A picture decoding can start at, and the frame index of the first frame it outputs.
#[derive(Clone, Copy, Debug)]
struct Restart {
    au: usize,
    base: u32,
}

/// An indexed video stream.
pub struct Video {
    codec: Codec,
    data: Data,
    nals: Vec<Range<usize>>,
    aus: Vec<Au>,
    restarts: Vec<Restart>,
    /// Parameter set NAL units, in stream order.
    params: Vec<usize>,
    /// Pictures per frame: 2 for H.264 field-coded streams.
    per_frame: u32,
    state: Mutex<State>,
}

impl std::fmt::Debug for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video")
            .field("codec", &self.codec)
            .field("nals", &self.nals.len())
            .field("pictures", &self.aus.len())
            .field("restarts", &self.restarts.len())
            .field("per_frame", &self.per_frame)
            .finish()
    }
}

#[derive(Default)]
struct State {
    cursor: Option<Cursor>,
    cache: Cache,
}

/// Recently decoded frames, least recently used first.
#[derive(Default)]
struct Cache {
    entries: VecDeque<(u32, Arc<Frame>)>,
    bytes: usize,
}

impl Cache {
    fn get(&mut self, index: u32) -> Option<Arc<Frame>> {
        let at = self.entries.iter().position(|e| e.0 == index)?;
        let e = self.entries.remove(at)?;
        let f = e.1.clone();
        self.entries.push_back(e);
        Some(f)
    }

    fn put(&mut self, index: u32, f: Arc<Frame>) {
        if self.entries.iter().any(|e| e.0 == index) {
            return;
        }
        self.bytes += f.bytes();
        self.entries.push_back((index, f));
        while self.bytes > CACHE_BYTES && self.entries.len() > 2 {
            if let Some((_, old)) = self.entries.pop_front() {
                self.bytes -= old.bytes();
            }
        }
    }

    /// How many frames of `bytes` each fit.
    fn capacity(bytes: usize) -> usize {
        (CACHE_BYTES / bytes.max(1)).max(2)
    }
}

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What a NAL unit does to access unit boundaries.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A coded slice; `first` when it is the picture's first; `ty` the NAL unit type.
    Vcl { first: bool, ty: u8 },
    /// A parameter set (it also starts an access unit when it follows a picture).
    Param,
    /// Starts a new access unit when it follows a picture (AUD, prefix SEI, ...).
    Starts,
    /// Belongs to the access unit it is in (suffix SEI, end of sequence, filler, other
    /// layers or views).
    Other,
}

fn classify(codec: Codec, nal: &[u8]) -> Kind {
    match codec {
        // Indexed by `index_mpeg2` instead.
        Codec::Mpeg2 => Kind::Other,
        Codec::H264 => {
            let ty = nal[0] & 31;
            match ty {
                // first_mb_in_slice == 0 codes as the single bit 1.
                1..=5 => Kind::Vcl {
                    first: nal.get(1).is_some_and(|b| b & 0x80 != 0),
                    ty,
                },
                7 | 8 => Kind::Param,
                6 | 9 | 13..=18 => Kind::Starts,
                _ => Kind::Other,
            }
        }
        Codec::Hevc => {
            let ty = (nal[0] >> 1) & 63;
            let layer = ((nal[0] & 1) << 5) | (nal.get(1).copied().unwrap_or(0) >> 3);
            if layer > 0 {
                return Kind::Other;
            }
            match ty {
                0..=31 => Kind::Vcl {
                    // first_slice_segment_in_pic_flag
                    first: nal.get(2).is_some_and(|b| b & 0x80 != 0),
                    ty,
                },
                32..=34 => Kind::Param,
                35 | 39 | 41..=44 | 48..=55 => Kind::Starts,
                _ => Kind::Other,
            }
        }
    }
}

/// The NAL units of an Annex B stream, as ranges (start codes and trailing zero bytes
/// removed, empty units dropped).
#[cfg(test)]
pub(crate) fn split_for_tests(data: &[u8]) -> Vec<&[u8]> {
    split(data).into_iter().map(|r| &data[r]).collect()
}

fn split(data: &[u8]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let n = data.len();
    let mut i = 0;
    let mut start: Option<usize> = None;
    let close = |s: usize, e: usize, out: &mut Vec<Range<usize>>| {
        let mut e = e;
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(s..e);
        }
    };
    while i + 2 < n {
        // A start code ends in 1 after two zeros: skip three bytes while the third is > 1.
        if data[i + 2] > 1 {
            i += 3;
            continue;
        }
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            if let Some(s) = start {
                close(s, i, &mut out);
            }
            i += 3;
            start = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(s) = start {
        close(s, n, &mut out);
    }
    out
}

/// What an elementary stream that is not H.264 / HEVC looks like, by its first start code.
fn foreign(first: u8) -> Option<&'static str> {
    match first {
        0xB3 => Some("MPEG-2 video"),
        0xB0 | 0xB5 | 0x20..=0x2F => Some("MPEG-4 Part 2 video"),
        _ => None,
    }
}

/// The stream of the fragments: the one fragment in place, or several joined.
fn gather(frags: &Fragments, v: &Values) -> Result<Data> {
    let items: Vec<&Range<usize>> = frags.items.iter().filter(|r| !r.is_empty()).collect();
    Ok(match items.as_slice() {
        [] => return Err(Error::Truncated),
        [one] => Data::Borrowed((*one).clone()),
        many => {
            let len: usize = many.iter().map(|r| r.len()).sum();
            let mut out = Vec::with_capacity(len);
            for r in many {
                out.extend_from_slice(v.buf.get((*r).clone()).unwrap_or(&[]));
            }
            Data::Owned(out)
        }
    })
}

impl Video {
    /// Indexes the stream in `frags` (values in `v`). `frames` is the declared Number of
    /// Frames. Fails when the stream has no picture or is another codec's.
    pub(crate) fn index(codec: Codec, frags: &Fragments, v: &Values, frames: u32) -> Result<Video> {
        if codec == Codec::Mpeg2 {
            return Video::index_mpeg2(frags, v);
        }
        let data = gather(frags, v)?;
        let bytes = match &data {
            Data::Borrowed(r) => v.buf.get(r.clone()).unwrap_or(&[]),
            Data::Owned(o) => o,
        };
        let nals = if crate::mp4::is_mp4(bytes) {
            crate::mp4::nals(bytes, codec)?
        } else {
            split(bytes)
        };
        let Some(first) = nals.first() else {
            return Err(Error::Codec(format!(
                "no {} start code in the video stream",
                codec.name()
            )));
        };
        let mut aus = Vec::new();
        let mut params = Vec::new();
        let mut kinds = Vec::with_capacity(nals.len());
        let (mut start, mut first_vcl) = (0usize, None::<usize>);
        for (i, r) in nals.iter().enumerate() {
            let k = classify(codec, &bytes[r.clone()]);
            kinds.push(k);
            let starts = match k {
                Kind::Vcl { first, .. } => first,
                Kind::Param | Kind::Starts => true,
                Kind::Other => false,
            };
            if starts && let Some(f) = first_vcl.take() {
                aus.push(Au {
                    nals: start..i,
                    first_vcl: f,
                });
                start = i;
            }
            match k {
                Kind::Vcl { .. } if first_vcl.is_none() => first_vcl = Some(i),
                Kind::Param => params.push(i),
                _ => {}
            }
        }
        if let Some(f) = first_vcl {
            aus.push(Au {
                nals: start..nals.len(),
                first_vcl: f,
            });
        } else if let Some(last) = aus.last_mut() {
            last.nals.end = nals.len();
        }
        if aus.is_empty() || params.is_empty() {
            let b = bytes.get(first.start).copied().unwrap_or(0);
            if let Some(what) = foreign(b) {
                return Err(Error::Unsupported(format!(
                    "{what} labelled as {} (not decoded)",
                    codec.name()
                )));
            }
            return Err(Error::Codec(if aus.is_empty() {
                format!("no {} picture in the video stream", codec.name())
            } else {
                format!("no {} parameter sets in the video stream", codec.name())
            }));
        }
        let per_frame =
            if codec == Codec::H264 && frames > 1 && aus.len() as u64 >= 2 * frames as u64 - 1 {
                2
            } else {
                1
            };
        let vcl_type = |au: &Au| match kinds[au.first_vcl] {
            Kind::Vcl { ty, .. } => ty,
            _ => 0,
        };
        let mut restarts = Vec::new();
        for (i, au) in aus.iter().enumerate() {
            let ty = vcl_type(au);
            let base = match codec {
                Codec::H264 if i == 0 || ty == 5 => (i as u32)
                    .is_multiple_of(per_frame)
                    .then_some(i as u32 / per_frame),
                Codec::H264 | Codec::Mpeg2 => None,
                Codec::Hevc if matches!(ty, 19 | 20) => Some(i as u32),
                // CRA and BLA: the leading pictures that follow precede it in output order;
                // RASL ones are skipped when decoding starts here. Usable when they are not a
                // mix of RASL and RADL (whose output positions would interleave).
                Codec::Hevc if i == 0 || (16..=21).contains(&ty) => {
                    let (mut rasl, mut radl) = (0u32, 0u32);
                    for a in &aus[i + 1..] {
                        match vcl_type(a) {
                            8 | 9 => rasl += 1,
                            6 | 7 => radl += 1,
                            _ => break,
                        }
                    }
                    (rasl == 0 || radl == 0).then_some(i as u32 + rasl)
                }
                Codec::Hevc => None,
            };
            if let Some(base) = base
                && restarts
                    .last()
                    .is_none_or(|r: &Restart| r.base < base || i == 0)
            {
                restarts.push(Restart { au: i, base });
            }
        }
        if restarts.is_empty() {
            restarts.push(Restart { au: 0, base: 0 });
        }
        Ok(Video {
            codec,
            data,
            nals,
            aus,
            restarts,
            params,
            per_frame,
            state: Mutex::new(State::default()),
        })
    }

    /// An MPEG-2 stream: units at its start codes, pictures (a field pair together) as
    /// access units, restart points at I pictures after a sequence header.
    fn index_mpeg2(frags: &Fragments, v: &Values) -> Result<Video> {
        let data = gather(frags, v)?;
        let bytes = match &data {
            Data::Borrowed(r) => v.buf.get(r.clone()).unwrap_or(&[]),
            Data::Owned(o) => o,
        };
        if crate::mp4::is_mp4(bytes) {
            return Err(Error::Unsupported(
                "MPEG-2 video in an MP4 file in the pixel data".into(),
            ));
        }
        let nals = split(bytes);
        let Some(first) = nals.first() else {
            return Err(Error::Codec(
                "no MPEG-2 start code in the video stream".into(),
            ));
        };
        let code = |i: usize| bytes.get(nals[i].start).copied().unwrap_or(0xFF);
        if !matches!(code(0), 0xB3 | 0xB5 | 0xB8 | 0xB2 | 0x00) {
            let b = bytes.get(first.start).copied().unwrap_or(0);
            return Err(Error::Unsupported(match b & 0x1F {
                7 | 9 => "H.264 video labelled as MPEG-2 (not decoded)".into(),
                _ => "video that is not MPEG-2 under an MPEG-2 label".into(),
            }));
        }
        // The picture headers: kind and structure.
        let picture = |i: usize| {
            let r = &nals[i];
            let from = r.start.saturating_sub(3);
            let head = &bytes[from..(r.start + 64).min(bytes.len())];
            let mut with_ext = head.to_vec();
            // The coding extension follows in the next unit.
            if let Some(n) = nals.get(i + 1)
                && bytes.get(n.start) == Some(&0xB5)
            {
                with_ext.extend_from_slice(
                    &bytes[n.start.saturating_sub(3)..(n.start + 16).min(bytes.len())],
                );
            }
            specular_mpeg2::scan(&with_ext).pictures.first().copied()
        };
        let mut aus: Vec<Au> = Vec::new();
        let mut kinds: Vec<(Option<specular_mpeg2::PictureInfo>, bool, bool)> = Vec::new();
        let mut params = Vec::new();
        let (mut start, mut pic_at) = (0usize, None::<usize>);
        let mut open_field = None;
        let (mut has_seq, mut closed) = (false, false);
        let mut after_seq = false;
        for (i, nal) in nals.iter().enumerate() {
            let c = code(i);
            let header = matches!(c, 0xB3 | 0xB8);
            let mut info = None;
            let mut second = false;
            if c == 0x00 {
                info = picture(i);
                second = matches!((open_field, info), (Some(o), Some(p)) if p.structure != specular_mpeg2::Structure::Frame && p.structure != o);
            }
            if (header || c == 0x00 && !second)
                && let Some(p) = pic_at.take()
            {
                aus.push(Au {
                    nals: start..i,
                    first_vcl: p,
                });
                start = i;
                has_seq = false;
                closed = false;
            }
            match c {
                0xB3 => {
                    params.push(i);
                    has_seq = true;
                    after_seq = true;
                }
                0xB5 | 0xB2 if after_seq => params.push(i),
                0xB8 => {
                    closed = bytes.get(nal.start + 4).is_some_and(|b| b & 0x40 != 0);
                    after_seq = false;
                }
                0x00 => {
                    after_seq = false;
                    if second {
                        open_field = None;
                    } else {
                        pic_at = Some(i);
                        kinds.push((info, has_seq, closed));
                        open_field = info
                            .map(|p| p.structure)
                            .filter(|&s| s != specular_mpeg2::Structure::Frame);
                    }
                }
                _ => after_seq = false,
            }
        }
        if let Some(p) = pic_at {
            aus.push(Au {
                nals: start..nals.len(),
                first_vcl: p,
            });
        }
        if aus.is_empty() || params.is_empty() {
            return Err(Error::Codec(if aus.is_empty() {
                "no MPEG-2 picture in the video stream".into()
            } else {
                "no MPEG-2 sequence header in the video stream".into()
            }));
        }
        let is = |k: usize, t: specular_mpeg2::PictureType| kinds[k].0.is_some_and(|p| p.kind == t);
        let mut restarts = Vec::new();
        for (i, kind) in kinds.iter().enumerate().take(aus.len()) {
            if !(is(i, specular_mpeg2::PictureType::I) && (kind.1 || i == 0)) {
                continue;
            }
            // An open GOP's B pictures right after the I picture precede it on screen and
            // can't be decoded from here.
            let leading = if kind.2 {
                0
            } else {
                (i + 1..aus.len())
                    .take_while(|&k| is(k, specular_mpeg2::PictureType::B))
                    .count() as u32
            };
            let base = i as u32 + if i == 0 { 0 } else { leading };
            if restarts.last().is_none_or(|r: &Restart| r.base < base) {
                restarts.push(Restart { au: i, base });
            }
        }
        if restarts.is_empty() {
            restarts.push(Restart { au: 0, base: 0 });
        }
        Ok(Video {
            codec: Codec::Mpeg2,
            data,
            nals,
            aus,
            restarts,
            params,
            per_frame: 1,
            state: Mutex::new(State::default()),
        })
    }

    pub fn codec(&self) -> Codec {
        self.codec
    }

    /// Frames in the stream (pictures, or field pairs).
    pub fn frames(&self) -> u32 {
        (self.aus.len() as u32).div_ceil(self.per_frame)
    }

    /// Random access points.
    pub fn restart_points(&self) -> usize {
        self.restarts.len()
    }

    fn bytes<'a>(&'a self, buf: &'a [u8]) -> &'a [u8] {
        match &self.data {
            Data::Borrowed(r) => buf.get(r.clone()).unwrap_or(&[]),
            Data::Owned(o) => o,
        }
    }

    /// The restart point to reach frame `index` from.
    fn restart_for(&self, index: u32) -> Restart {
        let at = self.restarts.partition_point(|r| r.base <= index);
        self.restarts[at.saturating_sub(1)]
    }

    /// A decoder positioned at restart point `r`, with the parameter sets seen before it.
    fn start(&self, buf: &[u8], r: Restart) -> Result<Cursor> {
        let bytes = self.bytes(buf);
        let before = self.aus[r.au].first_vcl;
        // Each distinct parameter set once, at its last occurrence before the picture (so a
        // set redefined under the same id ends up as the latest definition).
        let mut seen = HashSet::new();
        let mut sets: Vec<&[u8]> = Vec::new();
        for &i in self.params.iter().rev().filter(|&&i| i < before) {
            let n = &bytes[self.nals[i].clone()];
            if seen.insert(n) {
                sets.push(n);
            }
        }
        sets.reverse();
        let dec = match self.codec {
            Codec::Mpeg2 => {
                let mut d = specular_mpeg2::Decoder::new(0, specular_mpeg2::OutputOrder::Display);
                d.set_aligned(true);
                for s in &sets {
                    let mut unit = vec![0, 0, 1];
                    unit.extend_from_slice(s);
                    if let Err(specular_mpeg2::Error::Unsupported(m)) = d.decode(&unit, -1) {
                        return Err(Error::Unsupported(m));
                    }
                }
                Dec::Mpeg2(Box::new(d))
            }
            Codec::H264 => {
                let mut d = specular_h264::Decoder::new(specular_h264::Options {
                    threads: 0,
                    output: specular_h264::OutputOrder::Display,
                });
                for s in &sets {
                    if let Err(specular_h264::Error::Unsupported(m)) = d.decode_nal(s, -1) {
                        return Err(Error::Unsupported(m));
                    }
                }
                Dec::H264(Box::new(d))
            }
            Codec::Hevc => {
                let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
                let d = hevc::VideoDecoder::new(&sets, threads).map_err(hevc_err)?;
                Dec::Hevc(Box::new(d))
            }
        };
        Ok(Cursor {
            dec,
            next_au: r.au,
            next_out: r.base,
            flushed: false,
        })
    }

    /// Decodes frame `index` (stored values: RGB, or luma for a single-sample image).
    pub(crate) fn frame(
        &self,
        buf: &[u8],
        info: &PixelInfo,
        index: u32,
        cancel: Option<&AtomicBool>,
    ) -> Result<Frame> {
        let mut st = lock(&self.state);
        if let Some(f) = st.cache.get(index) {
            return Ok((*f).clone());
        }
        let r = self.restart_for(index);
        let reuse = st
            .cursor
            .as_ref()
            .is_some_and(|c| c.next_out <= index && c.next_out >= r.base);
        if !reuse {
            st.cursor = None;
            st.cursor = Some(self.start(buf, r)?);
        }
        let frame_bytes = info.pixels() as usize * if info.bits_stored > 8 { 6 } else { 3 };
        let keep = Cache::capacity(frame_bytes) as u32 / 2;
        let State { cursor, cache } = &mut *st;
        loop {
            let Some(c) = cursor.as_mut() else {
                return Err(Error::Truncated);
            };
            let got = c.next_frame(self, buf, cancel);
            let pic = match got {
                Ok(Some(p)) => p,
                Ok(None) => {
                    *cursor = None;
                    return Err(Error::Codec(format!(
                        "the video stream ends before frame {}",
                        index + 1
                    )));
                }
                Err(e) => {
                    *cursor = None;
                    return Err(e);
                }
            };
            let i = c.next_out;
            c.next_out += 1;
            if i >= index {
                let f = Arc::new(to_frame(&pic, info));
                cache.put(index, f.clone());
                return Ok(Arc::unwrap_or_clone(f));
            }
            // Frames just before the target are kept for stepping back.
            if !reuse && index - i <= keep {
                cache.put(i, Arc::new(to_frame(&pic, info)));
            }
        }
    }

    /// Bytes held by the frame cache.
    pub fn memory_bytes(&self) -> usize {
        lock(&self.state).cache.bytes
    }

    /// Drops the frame cache and the decoder state; returns the cache's bytes.
    pub fn trim(&self) -> usize {
        let mut st = lock(&self.state);
        st.cursor = None;
        std::mem::take(&mut st.cache).bytes
    }
}

fn hevc_err(e: hevc::Error) -> Error {
    match e {
        hevc::Error::Cancelled => Error::Cancelled,
        hevc::Error::Unsupported(m) => Error::Unsupported(format!("HEVC {m}")),
        e => Error::Codec(format!("HEVC: {e}")),
    }
}

enum Dec {
    Mpeg2(Box<specular_mpeg2::Decoder>),
    H264(Box<specular_h264::Decoder>),
    Hevc(Box<hevc::VideoDecoder>),
}

/// A decoded picture, as the decoder handed it back.
enum Pic {
    Mpeg2(specular_mpeg2::Picture),
    H264(specular_h264::Picture),
    Hevc(hevc::Decoded),
}

/// A decoder part-way through the stream.
struct Cursor {
    dec: Dec,
    /// The next access unit to feed.
    next_au: usize,
    /// The frame index of the next picture out.
    next_out: u32,
    flushed: bool,
}

impl Cursor {
    fn take(&mut self) -> Option<Pic> {
        match &mut self.dec {
            Dec::Mpeg2(d) => loop {
                // Pictures of the sequence header fed before the restart point carry no tag.
                match d.next_picture() {
                    Some(p) if p.tag < 0 => continue,
                    p => break p.map(Pic::Mpeg2),
                }
            },
            Dec::H264(d) => d.next_picture(true).map(Pic::H264),
            Dec::Hevc(d) => {
                while d.next_dropped().is_some() {}
                d.next_output().map(Pic::Hevc)
            }
        }
    }

    /// The next picture in output order (`None` at the end of the stream); for field-coded
    /// H.264 the decoder pairs fields itself.
    fn next_frame(
        &mut self,
        v: &Video,
        buf: &[u8],
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<Pic>> {
        let bytes = v.bytes(buf);
        loop {
            if let Some(p) = self.take() {
                return Ok(Some(p));
            }
            if let Some(au) = v.aus.get(self.next_au) {
                cancelled(cancel)?;
                let tag = self.next_au as i64;
                self.next_au += 1;
                let nals = v.nals[au.nals.clone()].iter().map(|r| &bytes[r.clone()]);
                match &mut self.dec {
                    Dec::Mpeg2(d) => {
                        // The picture's bytes from its first start code on, as one piece.
                        let from = v.nals[au.nals.start].start.saturating_sub(3);
                        let to = v.nals[au.nals.end - 1].end;
                        if let Err(specular_mpeg2::Error::Unsupported(m)) =
                            d.decode(&bytes[from..to], tag)
                        {
                            return Err(Error::Unsupported(m));
                        }
                    }
                    Dec::H264(d) => {
                        for n in nals {
                            // Damaged slices are concealed; only refusals stop the stream.
                            if let Err(specular_h264::Error::Unsupported(m)) = d.decode_nal(n, tag)
                            {
                                return Err(Error::Unsupported(m));
                            }
                        }
                    }
                    Dec::Hevc(d) => {
                        let nals: Vec<&[u8]> = nals.collect();
                        match d.decode_tagged(&nals, tag, cancel) {
                            Ok(()) => {}
                            Err(e @ (hevc::Error::Cancelled | hevc::Error::Unsupported(_))) => {
                                return Err(hevc_err(e));
                            }
                            // Only this picture is lost.
                            Err(_) => {}
                        }
                    }
                }
            } else if !self.flushed {
                self.flushed = true;
                match &mut self.dec {
                    Dec::Mpeg2(d) => d.flush(),
                    Dec::H264(d) => d.flush(),
                    Dec::Hevc(d) => d.flush(),
                }
            } else {
                return Ok(None);
            }
        }
    }
}

/// Front-to-back decoding for cine, with its own decoder.
pub struct Sequence {
    video: Arc<Video>,
    info: PixelInfo,
    cursor: Option<Cursor>,
    /// The frame index the next call returns.
    next: u32,
}

impl Sequence {
    pub(crate) fn new(video: Arc<Video>, info: PixelInfo) -> Sequence {
        Sequence {
            video,
            info,
            cursor: None,
            next: 0,
        }
    }

    /// The next frame with its index; `None` after the last.
    pub fn next_frame(
        &mut self,
        buf: &[u8],
        cancel: Option<&AtomicBool>,
    ) -> Option<Result<(u32, Frame)>> {
        if self.next >= self.info.frames {
            return None;
        }
        let r = (|| {
            if self.cursor.is_none() {
                let r = self.video.restart_for(self.next);
                self.cursor = Some(self.video.start(buf, r)?);
            }
            loop {
                let Some(c) = self.cursor.as_mut() else {
                    return Err(Error::Truncated);
                };
                let Some(pic) = c.next_frame(&self.video, buf, cancel)? else {
                    return Err(Error::Codec(format!(
                        "the video stream ends before frame {}",
                        self.next + 1
                    )));
                };
                let i = c.next_out;
                c.next_out += 1;
                if i >= self.next {
                    let index = self.next;
                    self.next = i + 1;
                    return Ok((index, to_frame(&pic, &self.info)));
                }
            }
        })();
        if r.is_err() {
            // Picks up at the next frame from a restart point on the next call.
            self.cursor = None;
            self.next += 1;
        }
        Some(r)
    }

    pub fn rewind(&mut self) {
        let fresh = self.cursor.as_ref().is_some_and(|c| c.next_out == 0);
        if !fresh {
            self.cursor = None;
        }
        self.next = 0;
    }
}

// ---------------------------------------------------------------------------------------------
// YCbCr to RGB

#[derive(Clone, Copy)]
enum Samples<'a> {
    U8(&'a [u8]),
    /// Little-endian 16-bit samples, as bytes.
    U16Le(&'a [u8]),
    U16(&'a [u16]),
}

#[derive(Clone, Copy)]
struct Plane<'a> {
    s: Samples<'a>,
    /// Samples per row.
    stride: usize,
    w: usize,
    h: usize,
}

impl Plane<'_> {
    /// Row `y` (clamped) into `out` as numbers, the last sample repeated past the width.
    fn row(&self, y: usize, out: &mut [f32]) {
        let y = y.min(self.h.saturating_sub(1));
        let base = y * self.stride;
        let w = self.w.min(out.len());
        match self.s {
            Samples::U8(d) => {
                let src = d.get(base..base + w).unwrap_or(&[]);
                for (o, &s) in out.iter_mut().zip(src) {
                    *o = s as f32;
                }
            }
            Samples::U16Le(d) => {
                let src = d.get(2 * base..2 * (base + w)).unwrap_or(&[]);
                for (o, s) in out.iter_mut().zip(src.chunks_exact(2)) {
                    *o = u16::from_le_bytes([s[0], s[1]]) as f32;
                }
            }
            Samples::U16(d) => {
                let src = d.get(base..base + w).unwrap_or(&[]);
                for (o, &s) in out.iter_mut().zip(src) {
                    *o = s as f32;
                }
            }
        }
        let have = match self.s {
            Samples::U8(d) => d.len().saturating_sub(base).min(w),
            Samples::U16Le(d) => (d.len() / 2).saturating_sub(base).min(w),
            Samples::U16(d) => d.len().saturating_sub(base).min(w),
        };
        let last = if have > 0 { out[have - 1] } else { 0.0 };
        for o in &mut out[have..] {
            *o = last;
        }
    }
}

/// A decoded picture's planes and signal description.
struct Yuv<'a> {
    w: usize,
    h: usize,
    y: Plane<'a>,
    /// Cb, Cr, and whether chroma is halved horizontally / vertically.
    c: Option<(Plane<'a>, Plane<'a>, bool, bool)>,
    depth_y: u32,
    depth_c: u32,
    full: bool,
    matrix: Option<u8>,
}

fn yuv(p: &Pic) -> Yuv<'_> {
    match p {
        Pic::Mpeg2(p) => {
            let (w, h) = (p.width as usize, p.height as usize);
            let (sx, sy) = p.chroma.shift();
            let (cw, ch) = ((w + (1 << sx) - 1) >> sx, (h + (1 << sy) - 1) >> sy);
            let plane = |d, w, h| Plane {
                s: Samples::U8(d),
                stride: w,
                w,
                h,
            };
            Yuv {
                w,
                h,
                y: plane(&p.planes[0], w, h),
                c: Some((
                    plane(&p.planes[1], cw, ch),
                    plane(&p.planes[2], cw, ch),
                    sx == 1,
                    sy == 1,
                )),
                depth_y: 8,
                depth_c: 8,
                full: false,
                matrix: p.colour.map(|c| c.2),
            }
        }
        Pic::H264(p) => {
            let (w, h) = (p.width as usize, p.height as usize);
            let half_v = p.chroma == specular_h264::Chroma::Yuv420;
            let half_h = p.chroma != specular_h264::Chroma::Yuv444;
            let cw = if half_h { w.div_ceil(2) } else { w };
            let ch = if half_v { h.div_ceil(2) } else { h };
            let wide = p.bit_depth > 8;
            let plane = |d, w, h| {
                let s = if wide {
                    Samples::U16Le(d)
                } else {
                    Samples::U8(d)
                };
                Plane { s, stride: w, w, h }
            };
            let signal = p.signal.unwrap_or_default();
            Yuv {
                w,
                h,
                y: plane(&p.planes[0], w, h),
                c: Some((
                    plane(&p.planes[1], cw, ch),
                    plane(&p.planes[2], cw, ch),
                    half_h,
                    half_v,
                )),
                depth_y: p.bit_depth as u32,
                depth_c: p.bit_depth as u32,
                full: signal.full_range,
                matrix: signal.colour.map(|c| c.2),
            }
        }
        Pic::Hevc(d) => {
            let f = &d.frame;
            let (w, h) = (f.width as usize, f.height as usize);
            let (cw, ch) = f.chroma.chroma_size(f.width, f.height);
            let c = (f.chroma != hevc::ChromaFormat::Monochrome).then(|| {
                let plane = |s| Plane {
                    s,
                    stride: f.c_stride as usize,
                    w: cw as usize,
                    h: ch as usize,
                };
                (
                    plane(Samples::U16(&f.cb)),
                    plane(Samples::U16(&f.cr)),
                    f.chroma.x_shift() == 1,
                    f.chroma.y_shift() == 1,
                )
            });
            Yuv {
                w,
                h,
                y: Plane {
                    s: Samples::U16(&f.y),
                    stride: f.y_stride as usize,
                    w,
                    h,
                },
                c,
                depth_y: f.bit_depth as u32,
                depth_c: f.bit_depth_chroma as u32,
                full: d.signal.is_some_and(|s| s.full_range),
                matrix: d.signal.and_then(|s| s.colour).map(|c| c.2),
            }
        }
    }
}

/// Converts a decoded picture to a frame of Rows x Columns.
fn to_frame(p: &Pic, info: &PixelInfo) -> Frame {
    let src = yuv(p);
    let (cols, rows) = (info.columns as usize, info.rows as usize);
    let depth = src.depth_y.max(src.depth_c).clamp(8, 16);
    let truncated = src.w == 0 || src.h == 0;
    if info.samples == 1 {
        // A single-sample image: the luma as stored.
        let mut out = vec![0u16; cols * rows];
        let mut row = vec![0f32; src.w.max(1)];
        for (y, o) in out.chunks_exact_mut(cols.max(1)).enumerate() {
            src.y.row(y, &mut row);
            for (x, o) in o.iter_mut().enumerate() {
                *o = row[x.min(row.len() - 1)] as u16;
            }
        }
        return Frame {
            width: info.columns,
            height: info.rows,
            pixels: Pixels::Grey(Grey::U16(out)),
            bits: src.depth_y as u16,
            signed: false,
            truncated,
        };
    }
    let pixels = if depth == 8 {
        let mut out = vec![0u8; cols * rows * 3];
        convert(&src, cols, 255.0, &mut out);
        Pixels::Rgb8(out)
    } else {
        let mut out = vec![0u16; cols * rows * 3];
        convert(&src, cols, ((1u32 << depth) - 1) as f32, &mut out);
        Pixels::Rgb16(out)
    };
    Frame {
        width: info.columns,
        height: info.rows,
        pixels,
        bits: depth as u16,
        signed: false,
        truncated,
    }
}

trait Out: Copy + Send + Default {
    fn from(v: f32) -> Self;
}

impl Out for u8 {
    fn from(v: f32) -> u8 {
        v as u8
    }
}

impl Out for u16 {
    fn from(v: f32) -> u16 {
        v as u16
    }
}

/// Kr and Kb of a matrix_coefficients value (H.273), `None` for identity (GBR).
fn kr_kb(matrix: Option<u8>, height: usize) -> Option<(f32, f32)> {
    Some(match matrix {
        Some(0) => return None,
        Some(1) => (0.2126, 0.0722),
        Some(4) => (0.30, 0.11),
        Some(5 | 6) => (0.299, 0.114),
        Some(7) => (0.212, 0.087),
        Some(9 | 10) => (0.2627, 0.0593),
        // Unspecified: HD sizes are BT.709, SD ones BT.601.
        _ if height > 576 => (0.2126, 0.0722),
        _ => (0.299, 0.114),
    })
}

/// The per-picture constants of the conversion.
#[derive(Clone, Copy)]
struct Coeffs {
    /// Normalised luma = (Y - y_off) * y_scale; chroma = (C - c_off) * c_scale (in -0.5..0.5).
    y_off: f32,
    y_scale: f32,
    c_off: f32,
    c_scale: f32,
    /// R = Y + rv Cr; G = Y - gu Cb - gv Cr; B = Y + bu Cb (identity when `None`).
    m: Option<(f32, f32, f32, f32)>,
    max: f32,
}

impl Coeffs {
    fn new(src: &Yuv, max: f32) -> Coeffs {
        let (dy, dc) = (src.depth_y.clamp(8, 16), src.depth_c.clamp(8, 16));
        let (sy, sc) = ((1u32 << (dy - 8)) as f32, (1u32 << (dc - 8)) as f32);
        let (y_off, y_scale, c_scale) = if src.full {
            (
                0.0,
                1.0 / ((1u32 << dy) - 1) as f32,
                1.0 / ((1u32 << dc) - 1) as f32,
            )
        } else {
            (16.0 * sy, 1.0 / (219.0 * sy), 1.0 / (224.0 * sc))
        };
        let m = kr_kb(src.matrix, src.h).map(|(kr, kb)| {
            let kg = 1.0 - kr - kb;
            (
                2.0 * (1.0 - kr),
                2.0 * kb * (1.0 - kb) / kg,
                2.0 * kr * (1.0 - kr) / kg,
                2.0 * (1.0 - kb),
            )
        });
        Coeffs {
            y_off,
            y_scale,
            c_off: (1u32 << (dc - 1)) as f32,
            c_scale,
            m,
            max,
        }
    }
}

/// Converts to interleaved RGB, `cols` wide, in row bands on several threads.
fn convert<T: Out>(src: &Yuv, cols: usize, max: f32, out: &mut [T]) {
    let k = Coeffs::new(src, max);
    let row_len = cols * 3;
    if row_len == 0 {
        return;
    }
    let rows = out.len() / row_len;
    let threads = if rows * cols >= 1 << 18 {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
    } else {
        1
    };
    let band = rows.div_ceil(threads).max(1);
    if threads <= 1 {
        convert_rows(src, &k, cols, 0, out);
        return;
    }
    std::thread::scope(|s| {
        for (i, chunk) in out.chunks_mut(band * row_len).enumerate() {
            let k = &k;
            specular_thread::spawn_scoped(s, format!("dcm-ybr-{i}"), move || {
                convert_rows(src, k, cols, i * band, chunk);
            });
        }
    });
}

fn convert_rows<T: Out>(src: &Yuv, k: &Coeffs, cols: usize, y0: usize, out: &mut [T]) {
    let w = src.w.max(1);
    let mut yrow = vec![0f32; w];
    let (cw, half_h, half_v) = match &src.c {
        Some((cb, _, hh, hv)) => (cb.w.max(1), *hh, *hv),
        None => (1, false, false),
    };
    let mut cb = vec![0f32; cw];
    let mut cr = vec![0f32; cw];
    let mut tmp = vec![0f32; cw];
    // Chroma per luma column, interpolated.
    let mut ub = vec![0f32; w];
    let mut vb = vec![0f32; w];
    for (r, o) in out.chunks_exact_mut(cols * 3).enumerate() {
        let y = (y0 + r).min(src.h.saturating_sub(1));
        src.y.row(y, &mut yrow);
        for v in &mut yrow {
            *v = (*v - k.y_off) * k.y_scale;
        }
        if let Some((pb, pr, _, _)) = &src.c {
            for (p, buf) in [(pb, &mut cb), (pr, &mut cr)] {
                if half_v {
                    // Chroma sits between luma rows 2i and 2i+1: 3/4 of the nearer row and
                    // 1/4 of the other neighbour.
                    let cy = y / 2;
                    let other = if y.is_multiple_of(2) {
                        cy.saturating_sub(1)
                    } else {
                        (cy + 1).min(p.h.saturating_sub(1))
                    };
                    p.row(cy, buf);
                    p.row(other, &mut tmp);
                    for (a, b) in buf.iter_mut().zip(&tmp) {
                        *a = 0.75 * *a + 0.25 * b;
                    }
                } else {
                    p.row(y, buf);
                }
                for v in buf.iter_mut() {
                    *v = (*v - k.c_off) * k.c_scale;
                }
            }
            if half_h {
                // Co-sited with the even luma columns; odd columns average their neighbours.
                for x in 0..w {
                    let j = (x / 2).min(cw - 1);
                    if x % 2 == 0 {
                        ub[x] = cb[j];
                        vb[x] = cr[j];
                    } else {
                        let j1 = (j + 1).min(cw - 1);
                        ub[x] = 0.5 * (cb[j] + cb[j1]);
                        vb[x] = 0.5 * (cr[j] + cr[j1]);
                    }
                }
            } else {
                let n = w.min(cw);
                ub[..n].copy_from_slice(&cb[..n]);
                vb[..n].copy_from_slice(&cr[..n]);
            }
        }
        let grey = src.c.is_none();
        for (x, px) in o.chunks_exact_mut(3).enumerate() {
            let sx = x.min(w - 1);
            let (l, u, v) = (yrow[sx], ub[sx], vb[sx]);
            let (rr, gg, bb) = if grey {
                (l, l, l)
            } else {
                match k.m {
                    Some((rv, gu, gv, bu)) => (l + rv * v, l - gu * u - gv * v, l + bu * u),
                    // Identity: Y is G, Cb is B, Cr is R, all at luma scale.
                    None => {
                        let luma = |c: f32| ((c / k.c_scale + k.c_off) - k.y_off) * k.y_scale;
                        (luma(v), l, luma(u))
                    }
                }
            };
            px[0] = T::from((rr * k.max + 0.5).clamp(0.0, k.max));
            px[1] = T::from((gg * k.max + 0.5).clamp(0.0, k.max));
            px[2] = T::from((bb * k.max + 0.5).clamp(0.0, k.max));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_codecs_by_uid() {
        let c = |u: &str| codec(&TransferSyntax::from_uid(u));
        assert_eq!(c("1.2.840.10008.1.2.4.102"), Some(Codec::H264));
        assert_eq!(c("1.2.840.10008.1.2.4.106.1"), Some(Codec::H264));
        assert_eq!(c("1.2.840.10008.1.2.4.107"), Some(Codec::Hevc));
        assert_eq!(c("1.2.840.10008.1.2.4.108"), Some(Codec::Hevc));
        assert_eq!(c("1.2.840.10008.1.2.4.100"), Some(Codec::Mpeg2));
        assert_eq!(c("1.2.840.10008.1.2.4.101.1"), Some(Codec::Mpeg2));
        assert_eq!(c("1.2.840.10008.1.2.4.70"), None);
        let r = refusal(&TransferSyntax::from_uid("1.2.840.10008.1.2.4.109"));
        assert!(r.contains("video"), "{r}");
    }

    #[test]
    fn splits_annex_b() {
        let s = [
            0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 0, 1, 0x65, 0x88, 0,
        ];
        let n = split(&s);
        let parts: Vec<&[u8]> = n.iter().map(|r| &s[r.clone()]).collect();
        assert_eq!(parts, [&[0x67, 1, 2][..], &[0x68, 3], &[0x65, 0x88]]);
        assert!(split(&[0, 0, 1]).is_empty());
        assert!(split(&[]).is_empty());
    }

    #[test]
    fn bt709_limited_white_and_red() {
        // One 2x2 4:2:0 picture: limited-range 709 red (Y 63, Cb 102, Cr 240).
        let y = [63u8; 4];
        let (cb, cr) = ([102u8], [240u8]);
        let p = |d: &'static [u8], w, h| Plane {
            s: Samples::U8(d),
            stride: w,
            w,
            h,
        };
        let y: &'static [u8] = Box::leak(Box::new(y));
        let cb: &'static [u8] = Box::leak(Box::new(cb));
        let cr: &'static [u8] = Box::leak(Box::new(cr));
        let src = Yuv {
            w: 2,
            h: 2,
            y: p(y, 2, 2),
            c: Some((p(cb, 1, 1), p(cr, 1, 1), true, true)),
            depth_y: 8,
            depth_c: 8,
            full: false,
            matrix: Some(1),
        };
        let mut out = [0u8; 12];
        convert(&src, 2, 255.0, &mut out);
        for px in out.chunks_exact(3) {
            assert!(px[0] >= 253 && px[1] <= 2 && px[2] <= 2, "{px:?}");
        }
    }
}
