//! Playback: the clip's samples through the hardware decoder (`specular-hwdec`), reordered to
//! presentation order, converted to RGBA and shaped to the still's frame, as a
//! [`specular_decode::Frames`] source for the viewer's animation player.
//!
//! A movie opened on its own ([`Clip::plain_frames`]) is not shaped: its frames keep the
//! decoder's YCbCr planes ([`PixelLayout::Yuv`]), which the GPU converts at display time
//! (BT.601 / BT.709 / BT.2020, either range, HLG and PQ through the HDR path), and the
//! session turns them by the track's rotation like any other image. AV1 tracks decode through
//! `specular-avif`'s rav1d fork. Every frame's time is known from the sample table, so the
//! player gets the whole timeline up front ([`Frames::delays`]) and seeks by decoding from the
//! nearest preceding sync sample ([`Frames::seek`]).
//!
//! HEVC clips fall back to the software decoder of `specular-hevc` when there is no hardware
//! decoder, when it refuses the stream, or when it fails on a sample (the clip then restarts in
//! software, skipping what was already shown). The animation player pulls frames on its own
//! decode-ahead thread into a byte-bounded ring and plays at the decoder's pace when it falls
//! behind, so a slow software decode lowers the frame rate instead of stalling; cancellation
//! is polled once per CTU row.
//!
//! The frames replace the still in place, so each one is turned the way the viewer turns the
//! still: the track's rotation (`tkhd`) makes it upright, the inverse of the still's EXIF
//! orientation puts it in the still's stored frame, and black bars pad it to the still's
//! aspect when the two differ (Samsung's 16:9 clips behind 4:3 photos).
//!
//! H.264 clips fall back to the software decoder of `specular-h264` ([`crate::sw_h264`]) when
//! there is no hardware decoder, it refuses the stream, or it fails part-way (then decoding
//! starts over in software, skipping what was already shown).
//!
//! Interlaced H.264 may store one field per sample, two composition times per frame. The slice
//! headers ([`specular_h264::FieldScan`]) tell which samples carry a second field: those times
//! are not frames of their own, a field pair is labelled with its first field's time on both
//! decoders, and its frame lasts both fields' durations. A clip ending on a first field without
//! its pair shows that field line-doubled; when the hardware decoder leaves it out, the last
//! pictures are decoded once in software for it.

use std::collections::{BTreeSet, HashMap};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use specular_avif::video::{Av1Video, VideoPicture};
use specular_core::{
    CancelToken, Orientation, PixelLayout, Raster, RasterPlane, YuvEncoding, YuvMatrix, yuv,
};
use specular_decode::{AnimationInfo, DecodeError, Frame, Frames};
use specular_heic::heif::{Chroma, HeifFrame, HeifPixelFormat, Plane};
use specular_heic::hevc;
use specular_heic::{ConvertOptions, Ycbcr};
use specular_hwdec::video::{
    Chroma as VChroma, Signal, VideoCodec, VideoDecoder, VideoFrame, video_supported,
};
use specular_log::debug;

use specular_h264::{FieldScan, SampleFields};

use crate::hevc_fields::Weaver;
use crate::mp4::{self, Codec, VideoTrack};
use crate::mpeg::Mpeg2Dec;
use crate::mpeg4::Mpeg4Dec;
use crate::seek_cache::{SeekCache, State};
use crate::sw_h264::SwH264;
use crate::vpx::Vpx;
use crate::{MotionInfo, Source};

/// Why a clip does not play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    Unsupported(String),
    Malformed(String),
    Io(String),
    /// The decoder's buffers do not fit the memory budget now.
    NeedsMemory(specular_core::NeedsMemory),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayError::Unsupported(s) => write!(f, "unsupported: {s}"),
            PlayError::Malformed(s) => write!(f, "malformed: {s}"),
            PlayError::Io(s) => write!(f, "i/o: {s}"),
            PlayError::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

impl From<specular_hwdec::Error> for PlayError {
    fn from(e: specular_hwdec::Error) -> PlayError {
        match e {
            specular_hwdec::Error::Unavailable => {
                PlayError::Unsupported("no hardware video decoder".into())
            }
            specular_hwdec::Error::Unsupported(s) => PlayError::Unsupported(s),
            specular_hwdec::Error::Malformed(s) => PlayError::Malformed(s),
            other => PlayError::Unsupported(other.to_string()),
        }
    }
}

/// A movie's bytes: read into memory (a still's embedded clip) or the mapped file.
pub enum ClipData {
    Owned(Vec<u8>),
    Mapped(Arc<specular_core::Source>),
}

impl Deref for ClipData {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            ClipData::Owned(v) => v,
            ClipData::Mapped(s) => s.bytes(),
        }
    }
}

/// A clip's bytes and its parsed video track.
pub struct Clip {
    data: ClipData,
    track: VideoTrack,
    codec: Codec,
}

/// How frames are handed out.
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// RGBA in the still's stored frame (a Motion Photo or Live Photo replacing its still).
    Still {
        orientation: Orientation,
        display: (u32, u32),
    },
    /// The decoder's YCbCr planes, as coded (a movie of its own).
    Plain,
}

impl Clip {
    /// Reads the clip `info` describes (the still's own bytes for an embedded clip, the paired
    /// movie for a Live Photo) and its sample table.
    pub fn load(still: &Path, info: &MotionInfo) -> Result<Clip, PlayError> {
        let io = |e: std::io::Error| PlayError::Io(e.to_string());
        let data = match &info.source {
            Source::Embedded(r) => {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(still).map_err(io)?;
                f.seek(SeekFrom::Start(r.start as u64)).map_err(io)?;
                let mut v = vec![0u8; r.len()];
                f.read_exact(&mut v).map_err(io)?;
                v
            }
            Source::Sibling(p) => std::fs::read(p).map_err(io)?,
        };
        Clip::from_bytes(data)
    }

    /// A clip from a movie's bytes.
    pub fn from_bytes(data: Vec<u8>) -> Result<Clip, PlayError> {
        let movie =
            crate::parse_movie(&data, true).map_err(|e| PlayError::Malformed(e.to_string()))?;
        Clip::new(ClipData::Owned(data), movie)
    }

    /// A clip from a movie file (mapped, not read) already parsed with its sample table.
    pub fn from_source(
        source: Arc<specular_core::Source>,
        movie: mp4::Movie,
    ) -> Result<Clip, PlayError> {
        Clip::new(ClipData::Mapped(source), movie)
    }

    fn new(data: ClipData, movie: mp4::Movie) -> Result<Clip, PlayError> {
        let track = movie
            .video
            .ok_or_else(|| PlayError::Malformed("no video track".into()))?;
        let codec = match track.codec {
            Some(
                c @ (Codec::H264
                | Codec::Hevc
                | Codec::Av1
                | Codec::Vp8
                | Codec::Vp9
                | Codec::JpegXs
                | Codec::Mpeg2
                | Codec::Mpeg1
                | Codec::Mpeg4
                | Codec::H263
                | Codec::Theora
                | Codec::ProRes
                | Codec::Mjpeg
                | Codec::Jpeg2000
                | Codec::Dv
                | Codec::Flv1
                | Codec::Vc1
                | Codec::Vp3
                | Codec::Ffv1
                | Codec::Raw
                | Codec::Classic(_)
                | Codec::CineForm
                | Codec::Hap(_)
                | Codec::Vc2
                | Codec::Apv),
            ) => c,
            Some(c) => return Err(PlayError::Unsupported(c.name())),
            None => return Err(PlayError::Malformed("no sample entry".into())),
        };
        if track.samples.is_empty() {
            return Err(PlayError::Malformed("no samples".into()));
        }
        Ok(Clip { data, track, codec })
    }

    pub fn track(&self) -> &VideoTrack {
        &self.track
    }

    /// The codec as the hardware decoder knows it (it takes no AV1).
    fn hw_codec(&self) -> Option<VideoCodec> {
        match self.codec {
            Codec::H264 => Some(VideoCodec::H264),
            Codec::Hevc => Some(VideoCodec::Hevc),
            _ => None,
        }
    }

    /// Whether an H.264 track codes fields (PAFF or MBAFF): its parameter sets, or the
    /// container's `fiel` box, say so.
    pub fn interlaced(&self) -> bool {
        track_interlaced(&self.track)
    }

    /// The frame source of a movie shown on its own: frames in presentation order within the
    /// track's edit, as YCbCr planes, the timeline known up front. Interlaced H.264 is
    /// deinterlaced ([`crate::deint`]).
    pub fn plain_frames(self) -> Result<(VideoFrames, AnimationInfo), PlayError> {
        self.open(Shape::Plain)
    }

    /// The frame source for a still shown with `orientation` at `display` size (its size on
    /// screen, after the orientation). Opens the hardware decoder, or for HEVC and H.264 the software
    /// one when there is no hardware decoder or it refuses the stream.
    pub fn frames(
        self,
        orientation: Orientation,
        display: (u32, u32),
    ) -> Result<(VideoFrames, AnimationInfo), PlayError> {
        self.open(Shape::Still {
            orientation,
            display,
        })
    }

    fn open(self, shape: Shape) -> Result<(VideoFrames, AnimationInfo), PlayError> {
        // HEVC fields are woven after the software decoder.
        let hevc_fields =
            self.codec == Codec::Hevc && crate::hevc_fields::field_seq(&self.track.config);
        let hard = match self.hw_codec() {
            Some(c) if video_supported(c) && !hevc_fields => {
                VideoDecoder::new(c, &self.track.config)
            }
            _ => Err(specular_hwdec::Error::Unavailable),
        };
        let dec = match hard {
            Ok(d) => Backend::Hard(Box::new(d)),
            Err(e) if self.codec == Codec::Hevc => {
                debug!(target: specular_log::DECODE, "video: software HEVC ({e})");
                Backend::Soft(Box::new(Soft::new(&self.track)?))
            }
            Err(e) if self.codec == Codec::H264 => {
                debug!(target: specular_log::DECODE, "video: software H.264 ({e})");
                Backend::H264(Box::new(SwH264::with_order(
                    &self.track.config,
                    self.track.slot_timing,
                )?))
            }
            Err(_) if self.codec == Codec::Av1 => Backend::Av1(Box::new(new_av1()?)),
            Err(_) if Vpx::handles(self.codec) => {
                Backend::Vpx(Box::new(Vpx::for_track(&self.track)?))
            }
            Err(_) if matches!(self.codec, Codec::Mpeg2 | Codec::Mpeg1) => {
                Backend::Mpeg2(Box::new(Mpeg2Dec::new(&self.track.config)?))
            }
            Err(_)
                if matches!(
                    self.codec,
                    Codec::Mpeg4 | Codec::H263 | Codec::Flv1 | Codec::Vc1
                ) =>
            {
                Backend::Mpeg4(Box::new(Mpeg4Dec::new(&self.track)?))
            }
            Err(e) => return Err(e.into()),
        };
        let samples = &self.track.samples;
        // A progressive movie of its own has a frame in every sample without reading every
        // slice header; interlaced ones may hold a field per sample.
        let interlaced = self.interlaced();
        let fields =
            if self.codec == Codec::H264 && (matches!(shape, Shape::Still { .. }) || interlaced) {
                field_scan(&self.data, &self.track)
            } else if self.codec == Codec::Mpeg2 && interlaced {
                crate::mpeg::field_scan(&self.data, &self.track)
            } else if hevc_fields {
                crate::hevc_fields::sample_fields(&self.track)
            } else {
                Vec::new()
            };
        let field = |i: usize| fields.get(i).copied().unwrap_or(ALL_FRAMES);
        // The time each sample's finished frame comes out under: a second field's frame under
        // the time of the sample with its first field.
        let mut labels = Vec::with_capacity(samples.len());
        let mut open = None;
        for (i, s) in samples.iter().enumerate() {
            let f = field(i);
            labels.push(if f.completes_open {
                open.unwrap_or(s.cts)
            } else {
                s.cts
            });
            open = f.leaves_open.then_some(s.cts);
        }
        let lone_tail = fields
            .last()
            .filter(|f| f.leaves_open && f.starts_frame)
            .and_then(|_| samples.last())
            .map(|s| s.cts);
        let mut pos = HashMap::with_capacity(samples.len());
        for (i, s) in samples.iter().enumerate() {
            pos.entry(s.cts).or_insert(i);
        }
        // Presentation order: the composition times of the samples that start a frame, sorted;
        // each frame lasts until the next (the last one until the end of the last sample).
        let mut order: Vec<i64> = samples
            .iter()
            .enumerate()
            .filter(|&(i, _)| field(i).starts_frame)
            .map(|(_, s)| s.cts)
            .collect();
        order.sort_unstable();
        order.dedup();
        let mut end = samples
            .iter()
            .map(|s| s.cts.saturating_add(s.duration as i64))
            .max()
            .unwrap_or(0);
        if matches!(shape, Shape::Plain) {
            (order, end) = trim_to_edit(order, end, &self.track);
        }
        let ts = self.track.timescale.max(1) as u64;
        let delays = order
            .iter()
            .enumerate()
            .map(|(k, &cts)| {
                let ticks = order
                    .get(k + 1)
                    .map_or(end.saturating_sub(cts).max(0) as u64, |n| n.abs_diff(cts))
                    .min(ts * 10);
                let d = Duration::from_micros(ticks * 1_000_000 / ts);
                if d.is_zero() {
                    Duration::from_millis(33)
                } else {
                    d
                }
            })
            .collect();
        let info = AnimationInfo {
            frame_count: Some(order.len() as u32),
            loop_count: 0,
        };
        let chroma_loc = chroma_loc(&self.track);
        // H.264 timed by slots (AVI, Matroska VFW, MP4 without `ctts` while it is fetched):
        // pictures come out in presentation order and take the times of the samples fed in
        // turn. (Fields keep their own labels.)
        let slots = (self.codec == Codec::H264 && self.track.slot_timing && fields.is_empty())
            .then(BTreeSet::new);
        let progressive_h264 = self.codec == Codec::H264 && !interlaced;
        Ok((
            VideoFrames {
                slots,
                chroma_loc,
                clip: self,
                dec,
                order,
                delays,
                fields,
                labels,
                pos,
                lone_tail,
                tail: None,
                last_out: None,
                next_sample: 0,
                hard_flushed: false,
                k: 0,
                buffered: Vec::new(),
                shape,
                av1_cosited: false,
                av1_cosited_y: false,
                deinterlace: interlaced && matches!(shape, Shape::Plain),
                woven: None,
                alpha: HashMap::new(),
                cache: SeekCache::new(),
                run_from: i64::MIN,
                out_max: i64::MIN,
                resumed: 0,
                samples_decoded: 0,
                recovery: HashMap::new(),
                interrupt: None,
                progressive_h264,
            },
            info,
        ))
    }
}

/// Whether an H.264 sample (an `avcC` track's) holds an IDR picture.
fn idr_sample(sample: &[u8], avcc: &[u8]) -> bool {
    let Ok((ls, _)) = specular_h264::parse_avcc(avcc) else {
        return true;
    };
    let mut p = 0;
    while p + ls <= sample.len() {
        let n = sample[p..p + ls]
            .iter()
            .fold(0usize, |a, &b| a << 8 | b as usize);
        p += ls;
        if sample.get(p).is_some_and(|&h| h & 31 == 5) {
            return true;
        }
        p += n;
    }
    false
}

/// The H.264 / HEVC VUI `chroma_sample_loc_type_top_field` of the track's parameter sets:
/// 0 left (the default), 1 centre, 2 top-left, 3 top, 4 bottom-left, 5 bottom.
fn chroma_loc(track: &VideoTrack) -> Option<u8> {
    match track.codec? {
        Codec::H264 => FieldScan::with_avcc(&track.config).ok()?.chroma_loc(),
        Codec::Hevc => {
            let cfg = hevc::hvcc::parse(&track.config).ok()?;
            hevc::probe(&cfg.nal_units).ok()?.chroma_loc
        }
        _ => None,
    }
}

/// When the frame with composition time `cts` is presented on the movie's timeline: the
/// edit's empty lead-in plus how far the frame is from the edit's start, in microseconds.
pub fn presented_at_us(t: &VideoTrack, cts: i64) -> i64 {
    let delay = i64::try_from(t.edit.delay_us).unwrap_or(i64::MAX / 2);
    delay.saturating_add(t.us(cts.saturating_sub(t.edit.start)))
}

/// Composition times of a progressive movie's frames in presentation order within its edit,
/// as [`Clip::plain_frames`] plays them (the frame count and timeline without opening a
/// decoder).
pub fn plain_timeline(track: &VideoTrack) -> Vec<i64> {
    plain_order(track, &[])
}

/// [`plain_timeline`] of any movie, its bytes `data` at hand: interlaced H.264 that holds a
/// field per sample has a frame per field pair.
pub fn plain_timeline_in(data: &[u8], track: &VideoTrack) -> Vec<i64> {
    let interlaced = track_interlaced(track);
    match track.codec {
        Some(Codec::H264) if interlaced => plain_order(track, &field_scan(data, track)),
        Some(Codec::Mpeg2) if interlaced => {
            plain_order(track, &crate::mpeg::field_scan(data, track))
        }
        Some(Codec::Hevc) if crate::hevc_fields::field_seq(&track.config) => {
            plain_order(track, &crate::hevc_fields::sample_fields(track))
        }
        _ => plain_order(track, &[]),
    }
}

/// Whether a track's video is interlaced: the container says so (`fiel`, the MPEG-2
/// sequence header's progressive_sequence), or H.264 or HEVC parameter sets code fields.
pub fn track_interlaced(track: &VideoTrack) -> bool {
    track.interlaced
        || track.codec == Some(Codec::H264)
            && FieldScan::with_avcc(&track.config).is_ok_and(|s| s.interlaced())
        || track.codec == Some(Codec::Mpeg2)
            && specular_mpeg2::probe(&track.config).is_some_and(|s| !s.progressive)
        || track.codec == Some(Codec::Mpeg4)
            && specular_mpeg4::probe(&track.config).is_some_and(|v| v.interlaced)
        || track.codec == Some(Codec::Hevc) && crate::hevc_fields::field_seq(&track.config)
}

fn plain_order(track: &VideoTrack, fields: &[SampleFields]) -> Vec<i64> {
    let mut order: Vec<i64> = track
        .samples
        .iter()
        .enumerate()
        .filter(|&(i, _)| fields.get(i).is_none_or(|f| f.starts_frame))
        .map(|(_, s)| s.cts)
        .collect();
    order.sort_unstable();
    order.dedup();
    let end = track
        .samples
        .iter()
        .map(|s| s.cts.saturating_add(i64::from(s.duration)))
        .max()
        .unwrap_or(0);
    trim_to_edit(order, end, track).0
}

fn new_av1() -> Result<Av1Video, PlayError> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) as u32;
    Av1Video::new(threads).map_err(|e| PlayError::Unsupported(e.to_string()))
}

/// The frames of `order` (composition times, sorted) that the track's edit list presents:
/// from the one on screen at the edit's start to the edit's end (returned as the new `end`).
fn trim_to_edit(order: Vec<i64>, end: i64, track: &VideoTrack) -> (Vec<i64>, i64) {
    let e = track.edit;
    let ts = i128::from(track.timescale.max(1));
    let first = order.partition_point(|&c| c <= e.start).saturating_sub(1);
    let stop = e
        .length_us
        .map(|l| {
            let ticks = i128::from(l) * ts / 1_000_000;
            e.start
                .saturating_add(i64::try_from(ticks).unwrap_or(i64::MAX))
        })
        .filter(|&s| s > e.start)
        .map_or(end, |s| s.min(end));
    let last = order
        .partition_point(|&c| c < stop)
        .max(first + 1)
        .min(order.len());
    if first >= last {
        return (order, end);
    }
    (order[first..last].to_vec(), stop)
}

/// A sample whose picture is a frame of its own: what every sample is without a field scan.
const ALL_FRAMES: SampleFields = SampleFields {
    starts_frame: true,
    completes_open: false,
    leaves_open: false,
    idr: false,
};

/// How an H.264 track's samples make up frames (empty when the headers can't be read: then
/// every sample is taken for a frame).
fn field_scan(data: &[u8], track: &VideoTrack) -> Vec<SampleFields> {
    let Ok(mut scan) = FieldScan::with_avcc(&track.config) else {
        return Vec::new();
    };
    track
        .samples
        .iter()
        .map(|s| {
            track
                .sample_data(data, s)
                .and_then(|b| scan.sample(&b).ok())
                .unwrap_or(ALL_FRAMES)
        })
        .collect()
}

/// The software HEVC decoder and what it needs to hand pictures back with their times.
struct Soft {
    dec: hevc::VideoDecoder,
    length_size: usize,
    /// Composition times of samples whose pictures will never come out: RASL pictures skipped
    /// after the random access point decoding started at, `pic_output_flag` 0, pictures
    /// discarded by `no_output_of_prior_pics_flag`, and samples that were lost.
    gone: Vec<i64>,
    flushed: bool,
    /// Field-coded sequences: pairs the fields into frames.
    weaver: Option<Weaver>,
    /// The decoder as built: restarting restores it, keeping the worker threads.
    fresh: hevc::Snapshot,
    /// The stream's highest `TemporalId` (`sps_max_sub_layers_minus1`): sub-layer
    /// non-reference pictures of that sub-layer are referenced by no picture.
    top_tid: u8,
    /// The latest composition time of a picture dropped by the decoder or taken without
    /// making its frame (see [`VideoFrames::run_from`]).
    latest_lost: i64,
}

impl Soft {
    fn new(track: &VideoTrack) -> Result<Soft, PlayError> {
        let config = &track.config[..];
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
        let bad = |e: hevc::Error| PlayError::Malformed(e.to_string());
        let cfg = hevc::hvcc::parse(config).map_err(bad)?;
        let dec = hevc::VideoDecoder::new(&cfg.nal_units, threads).map_err(|e| match e {
            hevc::Error::Unsupported(m) => PlayError::Unsupported(m.into()),
            e => bad(e),
        })?;
        let fields = hevc::probe(&cfg.nal_units).is_ok_and(|i| i.field_seq);
        // SPS (type 33): the byte after the NAL header holds sps_max_sub_layers_minus1.
        let top_tid = cfg
            .nal_units
            .iter()
            .filter(|n| n.len() > 2 && (n[0] >> 1) & 63 == 33)
            .map(|n| (n[2] >> 1) & 7)
            .max()
            .unwrap_or(6);
        Ok(Soft {
            fresh: dec.snapshot(),
            dec,
            length_size: cfg.length_size,
            gone: Vec::new(),
            flushed: false,
            weaver: fields.then(|| Weaver::for_track(track)),
            top_tid,
            latest_lost: i64::MIN,
        })
    }

    /// Back to the state of a new decoder (the next sample is a sync sample).
    fn reset(&mut self) {
        // The fresh state holds no pictures: restoring it cannot fail.
        self.dec.restore(&self.fresh);
        self.gone.clear();
        self.flushed = false;
        self.latest_lost = i64::MIN;
        if let Some(w) = &mut self.weaver {
            w.reset();
        }
    }

    /// Whether `sample` holds only a picture no other picture predicts from: sub-layer
    /// non-reference slices of the highest sub-layer, and no parameter sets or end of
    /// sequence. Such a picture presented before a seek target need not be decoded at all.
    fn unreferenced(&self, sample: &[u8]) -> bool {
        if self.weaver.is_some() {
            return false;
        }
        let Ok(nals) = hevc::hvcc::split(sample, self.length_size) else {
            return false;
        };
        let mut vcl = false;
        for n in nals {
            if n.len() < 2 {
                return false;
            }
            let (t, tid) = ((n[0] >> 1) & 63, (n[1] & 7).wrapping_sub(1));
            match t {
                // TRAIL_N, TSA_N, STSA_N, RADL_N, RASL_N and the reserved _N types.
                0..=14 if t % 2 == 0 && tid == self.top_tid => vcl = true,
                // SEI, AUD and filler data change nothing a later picture needs.
                35 | 38..=40 => {}
                _ => return false,
            }
        }
        vcl
    }

    /// Decodes one sample (`None`: the end of the stream) and moves the pictures that are
    /// ready, with their composition times, to `out`; pictures presented before `from` are
    /// taken from the decoder without being made into frames (a seek discards them).
    fn step(
        &mut self,
        sample: Option<(&[u8], i64)>,
        from: i64,
        cancel: &CancelToken,
        out: &mut Vec<(i64, VideoFrame)>,
    ) -> Result<(), DecodeError> {
        match sample {
            Some((bytes, cts)) => {
                let nals = hevc::hvcc::split(bytes, self.length_size)
                    .map_err(|e| DecodeError::Malformed(e.to_string()))?;
                if let Some(w) = &mut self.weaver {
                    w.note(cts, &nals);
                }
                // Each picture comes back tagged with its sample's composition time.
                match self.dec.decode_tagged(&nals, cts, Some(cancel.flag())) {
                    Ok(()) => {}
                    Err(hevc::Error::Cancelled) => return Err(DecodeError::Cancelled),
                    Err(hevc::Error::Unsupported(m)) => {
                        return Err(DecodeError::Unsupported(m.into()));
                    }
                    // Only this picture is lost; later ones predict from a stand-in.
                    Err(e) => {
                        debug!(target: specular_log::DECODE, "motion clip: sample at {cts} lost: {e}");
                    }
                }
            }
            None => {
                self.dec.flush();
                self.flushed = true;
            }
        }
        loop {
            if self.weaver.is_none()
                && let Some(t) = self.dec.ready_tag()
                && t < from
            {
                self.dec.skip_output();
                self.latest_lost = self.latest_lost.max(t);
                continue;
            }
            let Some(d) = self.dec.next_output() else {
                break;
            };
            let tag = d.tag;
            let f = soft_frame(d)?;
            match &mut self.weaver {
                Some(w) => w.push(tag, f, out),
                None => out.push((tag, f)),
            }
        }
        if self.flushed
            && let Some(w) = &mut self.weaver
        {
            w.finish(out);
        }
        while let Some(cts) = self.dec.next_dropped() {
            // A dropped field: its partner may still show for the frame (line-doubled).
            let gone = match &mut self.weaver {
                Some(w) => w.dropped(cts, out),
                None => Some(cts),
            };
            if let Some(g) = gone {
                self.gone.push(g);
                self.latest_lost = self.latest_lost.max(g);
            }
        }
        Ok(())
    }
}

/// A software-decoded picture in the hardware decoder's layout (monochrome as 4:2:0 with
/// mid-grey chroma).
fn soft_frame(d: hevc::Decoded) -> Result<VideoFrame, DecodeError> {
    let f = d.frame;
    let mono = f.chroma == hevc::ChromaFormat::Monochrome;
    let chroma = match f.chroma {
        hevc::ChromaFormat::Yuv420 | hevc::ChromaFormat::Monochrome => VChroma::Yuv420,
        hevc::ChromaFormat::Yuv422 => VChroma::Yuv422,
        hevc::ChromaFormat::Yuv444 => VChroma::Yuv444,
    };
    let wide = f.bit_depth > 8 || f.bit_depth_chroma > 8;
    let pack = |p: &[u16], stride: u32, w: u32, h: u32| -> Vec<u8> {
        let (stride, w) = (stride as usize, w as usize);
        let mut v = Vec::with_capacity(w * h as usize * if wide { 2 } else { 1 });
        for row in p.chunks(stride.max(1)).take(h as usize) {
            let row = &row[..w.min(row.len())];
            if wide {
                // Little-endian bytes: write each sample's two bytes into place (the
                // compiler turns this into a copy on little-endian hosts), not an
                // iterator of arrays flattened byte by byte.
                let at = v.len();
                v.resize(at + row.len() * 2, 0);
                for (o, &s) in v[at..].chunks_exact_mut(2).zip(row) {
                    o.copy_from_slice(&s.to_le_bytes());
                }
            } else {
                v.extend(row.iter().map(|&s| s as u8));
            }
        }
        v
    };
    let (cw, ch) = chroma.plane_size(f.width, f.height);
    let planes = if mono {
        let depth = f.bit_depth.max(f.bit_depth_chroma).max(1);
        let grey = vec![1u16 << (depth - 1); (cw * ch) as usize];
        let g = pack(&grey, cw, cw, ch);
        [pack(&f.y, f.y_stride, f.width, f.height), g.clone(), g]
    } else {
        [
            pack(&f.y, f.y_stride, f.width, f.height),
            pack(&f.cb, f.c_stride, cw, ch),
            pack(&f.cr, f.c_stride, cw, ch),
        ]
    };
    Ok(VideoFrame {
        width: f.width,
        height: f.height,
        bit_depth: f.bit_depth.max(f.bit_depth_chroma),
        chroma,
        planes,
        signal: d.signal.map(|s| Signal {
            full_range: s.full_range,
            primaries: s.colour.map(|c| c.0),
            transfer: s.colour.map(|c| c.1),
            matrix: s.colour.map(|c| c.2),
        }),
    })
}

enum Backend {
    Hard(Box<VideoDecoder>),
    Soft(Box<Soft>),
    H264(Box<SwH264>),
    Av1(Box<Av1Video>),
    Vpx(Box<Vpx>),
    Mpeg2(Box<Mpeg2Dec>),
    Mpeg4(Box<Mpeg4Dec>),
}

/// An AV1 picture in the hardware decoder's layout (monochrome comes with neutral chroma).
/// AV1's colour_config cannot signal 4:4:0 (subsampling_y is read only when subsampling_x is
/// set), but a picture so shaped is passed on as 4:4:0 like VP9's, not refused.
fn av1_frame(p: VideoPicture) -> Result<VideoFrame, DecodeError> {
    let chroma = match (p.ssx, p.ssy) {
        (true, true) => VChroma::Yuv420,
        (true, false) => VChroma::Yuv422,
        (false, false) => VChroma::Yuv444,
        (false, true) => VChroma::Yuv440,
    };
    let code = |c: u16| u8::try_from(c).ok();
    Ok(VideoFrame {
        width: p.width,
        height: p.height,
        bit_depth: p.bits,
        chroma,
        planes: p.planes,
        signal: Some(Signal {
            full_range: p.full_range,
            primaries: code(p.primaries),
            transfer: code(p.transfer),
            matrix: code(p.matrix),
        }),
    })
}

/// Decoded frames in presentation order (see the module docs).
pub struct VideoFrames {
    clip: Clip,
    dec: Backend,
    /// Composition times of the frames in presentation order.
    order: Vec<i64>,
    delays: Vec<Duration>,
    /// Per sample (decoding order) of an H.264 track: the frames and fields it holds.
    fields: Vec<SampleFields>,
    /// Per sample: the composition time of the frame it completes.
    labels: Vec<i64>,
    /// Decoding position of the sample with each composition time.
    pos: HashMap<i64, usize>,
    /// The composition time of a last sample holding a first field without its pair.
    lone_tail: Option<i64>,
    /// That field's frame decoded in software, when the hardware decoder left it out (kept
    /// across loops).
    tail: Option<(i64, VideoFrame)>,
    /// The latest decoding position of a picture that came out since decoding (re)started:
    /// a decode-order decoder will not output an earlier picture after it.
    last_out: Option<usize>,
    /// Next sample to decode (decode order).
    next_sample: usize,
    /// The hardware decoder was flushed at the end of the samples.
    hard_flushed: bool,
    /// Next frame to hand out (presentation order).
    k: usize,
    /// Decoded pictures waiting for their turn (at most the stream's reorder depth).
    buffered: Vec<(i64, VideoFrame)>,
    /// Slot timing (H.264): the times of the samples fed whose slot no picture took yet.
    slots: Option<BTreeSet<i64>>,
    shape: Shape,
    /// The last AV1 picture's chroma was co-sited with even luma columns.
    av1_cosited: bool,
    /// ... and with even luma rows (AV1 "colocated", H.273 top-left).
    av1_cosited_y: bool,
    /// Where H.264 / HEVC chroma sits, from the VUI ([`chroma_loc`]).
    chroma_loc: Option<u8>,
    /// Interlaced video shown on its own: frames are deinterlaced.
    deinterlace: bool,
    /// The previous frame as decoded (woven), for the deinterlacer's motion.
    woven: Option<VideoFrame>,
    /// Alpha planes of buffered pictures (ProRes 4444), by composition time.
    alpha: HashMap<i64, Vec<u8>>,
    /// Software HEVC, H.264 and MPEG-1/2: decoder snapshots near recent seek targets.
    cache: SeekCache,
    /// A snapshot taken now serves only targets presented from this time on. Since decoding
    /// last (re)started, pictures before it were skipped, dropped or discarded, or are
    /// leading pictures of the sync sample it started at.
    run_from: i64,
    /// Decoding-order decoders (H.264, MPEG-1/2): the latest composition time of a picture
    /// that came out since decoding last (re)started. A snapshot holds none of these, so it
    /// serves only targets presented after it.
    out_max: i64,
    /// Seeks that resumed from a snapshot.
    resumed: u32,
    /// Samples handed to the decoder (for measurements and tests).
    samples_decoded: u64,
    /// H.264 samples probed for an open GOP's recovery point (see
    /// [`VideoFrames::recovery_key`]), and whether they are one.
    recovery: HashMap<usize, bool>,
    /// Set by the player while a decode is no longer wanted (scrubbing on): `fill` stops
    /// between two samples ([`Frames::set_interrupt`]).
    interrupt: Option<Arc<AtomicBool>>,
    /// Progressive H.264: unreferenced pictures before a seek target are not decoded.
    progressive_h264: bool,
}

/// Raises `max` to the latest composition time of `out`.
fn note_max(out: &[(i64, VideoFrame)], max: &mut i64) {
    if let Some(m) = out.iter().map(|b| b.0).max() {
        *max = (*max).max(m);
    }
}

/// [`VideoFrames::discard_before`] on the fields it needs.
fn discard_before(buffered: &mut Vec<(i64, VideoFrame)>, run_from: &mut i64, cts: i64) {
    let mut latest = i64::MIN;
    buffered.retain(|b| {
        let keep = b.0 >= cts;
        if !keep {
            latest = latest.max(b.0);
        }
        keep
    });
    *run_from = (*run_from).max(latest.saturating_add(1));
}

fn hw_err(e: specular_hwdec::Error) -> DecodeError {
    match PlayError::from(e) {
        PlayError::Malformed(s) => DecodeError::Malformed(s),
        PlayError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
        other => DecodeError::Unsupported(other.to_string()),
    }
}

impl VideoFrames {
    /// Whether a decode-order decoder is past the picture with composition time `cts`: a
    /// picture decoded after it came out, so it never will (e.g. pictures before the first
    /// IDR the hardware decoder skips).
    fn passed(&self, cts: i64) -> bool {
        match (self.last_out, self.pos.get(&cts)) {
            (Some(o), Some(&p)) => o > p,
            _ => false,
        }
    }

    /// Notes the decoding positions of the pictures buffered from index `from` on.
    fn note_out(&mut self, from: usize) {
        for b in &self.buffered[from..] {
            if let Some(&p) = self.pos.get(&b.0) {
                self.last_out = Some(self.last_out.map_or(p, |o| o.max(p)));
            }
        }
    }

    /// Buffers the pictures the hardware decoder has handed out, labelled by their samples.
    fn take_hard(&mut self) {
        let Backend::Hard(dec) = &mut self.dec else {
            return;
        };
        let from = self.buffered.len();
        while let Some((tag, f)) = dec.next_output() {
            let i = tag as usize;
            let label = self
                .labels
                .get(i)
                .copied()
                .unwrap_or_else(|| self.clip.track.samples.get(i).map_or(0, |s| s.cts));
            let label = match self.slots.as_mut() {
                Some(s) => s.pop_first().unwrap_or(label),
                None => label,
            };
            self.buffered.push((label, f));
        }
        self.note_out(from);
    }

    /// The trailing lone first field `cts` decoded in software, from the last IDR picture
    /// before it: the hardware decoder only shows field pairs.
    fn soft_tail(&mut self, cts: i64, cancel: &CancelToken) -> Result<(), DecodeError> {
        if self.tail.as_ref().is_none_or(|t| t.0 != cts) {
            let samples = &self.clip.track.samples;
            let i = self.pos.get(&cts).copied().unwrap_or(samples.len());
            let start = (0..=i.min(samples.len().saturating_sub(1)))
                .rev()
                .find(|&k| self.fields.get(k).is_some_and(|f| f.idr))
                .unwrap_or(0);
            debug!(target: specular_log::DECODE, "motion clip: last field in software (samples {start}..)");
            let mut sw = SwH264::new(&self.clip.track.config)
                .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
            let mut out = Vec::new();
            for s in &samples[start..] {
                let bytes = self
                    .clip
                    .track
                    .sample_data(&self.clip.data, s)
                    .ok_or_else(|| {
                        DecodeError::Truncated("sample past the end of the clip".into())
                    })?;
                sw.step(Some((&bytes, s.cts)), cancel, &mut out)?;
                out.retain(|b| b.0 == cts);
            }
            sw.step(None, cancel, &mut out)?;
            self.tail = out.into_iter().find(|b| b.0 == cts);
        }
        if let Some((t, f)) = &self.tail {
            let copy = copy_frame(f);
            self.buffered.push((*t, copy));
        }
        Ok(())
    }

    /// Decodes until the picture with composition time `cts` is buffered (or the samples run
    /// out, or a picture decoded after it came out, or the software HEVC decoder reports it
    /// will never be output, e.g. a RASL picture whose references precede the random access
    /// point: then that picture is not there).
    fn fill(&mut self, cts: i64, cancel: &CancelToken) -> Result<(), DecodeError> {
        while !self.buffered.iter().any(|b| b.0 == cts) {
            if cancel.is_cancelled()
                || self
                    .interrupt
                    .as_ref()
                    .is_some_and(|f| f.load(Ordering::Acquire))
            {
                return Err(DecodeError::Cancelled);
            }
            if !matches!(self.dec, Backend::Soft(_)) && self.passed(cts) {
                break;
            }
            let owned = match self.clip.track.samples.get(self.next_sample) {
                Some(s) => Some((
                    self.clip
                        .track
                        .sample_data(&self.clip.data, s)
                        .ok_or_else(|| {
                            DecodeError::Truncated("sample past the end of the clip".into())
                        })?,
                    s.cts,
                )),
                None => None,
            };
            let sample = owned.as_ref().map(|(b, c)| (&b[..], *c));
            if sample.is_some() {
                self.samples_decoded += 1;
            }
            match &mut self.dec {
                Backend::Hard(dec) => {
                    let Some((bytes, at)) = sample else {
                        if !self.hard_flushed {
                            // HEVC pictures still waiting for output come out now.
                            self.hard_flushed = true;
                            dec.flush();
                            self.take_hard();
                            continue;
                        }
                        if self.lone_tail == Some(cts) {
                            self.soft_tail(cts, cancel)?;
                        }
                        break;
                    };
                    let i = self.next_sample;
                    self.next_sample += 1;
                    // A picture presented before the target is decoded for reference only:
                    // reading it back from the device would cost more than decoding it.
                    // (Slot timing: which one is presented when is known only as they come
                    // out, so every picture comes out and takes its slot.)
                    if let Some(s) = self.slots.as_mut() {
                        s.insert(at);
                    }
                    let shown =
                        self.slots.is_some() || self.labels.get(i).copied().unwrap_or(at) >= cts;
                    let r = if shown {
                        dec.decode(bytes, i as i64)
                    } else {
                        dec.decode_unshown(bytes, i as i64)
                    };
                    match r {
                        Ok(()) => self.take_hard(),
                        Err(e) if self.clip.codec == Codec::Hevc => {
                            // The hardware gave up: start over in software. Pictures before
                            // `cts` were shown already and are dropped as they come.
                            debug!(target: specular_log::DECODE, "motion clip: hardware decode failed ({e}), switching to software");
                            let soft = Soft::new(&self.clip.track)
                                .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
                            self.dec = Backend::Soft(Box::new(soft));
                            self.next_sample = 0;
                            self.last_out = None;
                            self.buffered.clear();
                        }
                        Err(e) if self.clip.codec == Codec::H264 => {
                            // The hardware gave up: start over in software. Pictures before
                            // `cts` were shown already and are dropped as they come.
                            debug!(target: specular_log::DECODE, "motion clip: hardware decode failed ({e}), switching to software H.264");
                            let soft =
                                SwH264::with_order(&self.clip.track.config, self.slots.is_some())
                                    .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
                            self.dec = Backend::H264(Box::new(soft));
                            self.next_sample = 0;
                            self.last_out = None;
                            self.buffered.clear();
                            if let Some(s) = self.slots.as_mut() {
                                s.clear();
                            }
                        }
                        Err(e) => return Err(hw_err(e)),
                    }
                }
                Backend::Soft(soft) => {
                    if sample.is_none() && soft.flushed || soft.gone.contains(&cts) {
                        break;
                    }
                    if let Some((bytes, at)) = sample {
                        self.next_sample += 1;
                        if at < cts && soft.unreferenced(bytes) {
                            // Not shown and predicted from by nothing: not decoded.
                            self.samples_decoded -= 1;
                            self.run_from = self.run_from.max(at.saturating_add(1));
                            continue;
                        }
                    }
                    soft.step(sample, cts, cancel, &mut self.buffered)?;
                    self.run_from = self.run_from.max(soft.latest_lost.saturating_add(1));
                    discard_before(&mut self.buffered, &mut self.run_from, cts);
                    soft.gone.retain(|&g| g >= cts);
                    if sample.is_some() && soft.weaver.is_none() && self.cache.due(self.next_sample)
                    {
                        let snap = soft.dec.snapshot();
                        self.cache
                            .insert(self.next_sample, State::Hevc(snap), self.run_from);
                    }
                }
                Backend::H264(soft) => {
                    if sample.is_none() && soft.flushed {
                        break;
                    }
                    if let Some((bytes, at)) = sample {
                        self.next_sample += 1;
                        // Not shown and predicted from by nothing: not decoded. (Progressive
                        // streams timed by their samples only: fields pair up, and slot
                        // timing gives each picture that comes out the next free slot.)
                        if at < cts
                            && self.slots.is_none()
                            && self.fields.is_empty()
                            && self.progressive_h264
                            && soft.unreferenced(bytes)
                        {
                            self.samples_decoded -= 1;
                            self.run_from = self.run_from.max(at.saturating_add(1));
                            continue;
                        }
                        if let Some(s) = self.slots.as_mut() {
                            s.insert(at);
                        }
                    }
                    let from = self.buffered.len();
                    soft.step(sample, cancel, &mut self.buffered)?;
                    // (Slot timing keeps no snapshots: the free slots are not in them.)
                    let snap = (sample.is_some()
                        && self.slots.is_none()
                        && self.cache.due(self.next_sample))
                    .then(|| soft.snapshot())
                    .flatten();
                    if let Some(s) = self.slots.as_mut() {
                        for b in &mut self.buffered[from..] {
                            if let Some(t) = s.pop_first() {
                                b.0 = t;
                            }
                        }
                    }
                    self.note_out(from);
                    note_max(&self.buffered[from..], &mut self.out_max);
                    self.buffered.retain(|b| b.0 >= cts);
                    if let Some(snap) = snap {
                        let from = self.run_from.max(self.out_max.saturating_add(1));
                        self.cache.insert(self.next_sample, State::H264(snap), from);
                    }
                }
                Backend::Av1(dec) => {
                    // One shown picture per temporal unit, in presentation order.
                    let Some((bytes, at)) = sample else {
                        break;
                    };
                    self.next_sample += 1;
                    match dec.decode(bytes) {
                        Ok(Some(p)) => {
                            self.av1_cosited = p.cosited_x;
                            self.av1_cosited_y = p.cosited_y;
                            let from = self.buffered.len();
                            self.buffered.push((at, av1_frame(p)?));
                            self.note_out(from);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            debug!(target: specular_log::DECODE, "video: AV1 sample at {at} lost: {e}");
                        }
                    }
                    self.buffered.retain(|b| b.0 >= cts);
                }
                Backend::Mpeg2(soft) => {
                    if sample.is_none() && soft.flushed {
                        break;
                    }
                    if sample.is_some() {
                        self.next_sample += 1;
                    }
                    let from = self.buffered.len();
                    soft.step(sample, cancel, &mut self.buffered)?;
                    let snap = (sample.is_some() && self.cache.due(self.next_sample))
                        .then(|| soft.snapshot())
                        .flatten();
                    self.note_out(from);
                    note_max(&self.buffered[from..], &mut self.out_max);
                    self.buffered.retain(|b| b.0 >= cts);
                    if let Some(snap) = snap {
                        let from = self.run_from.max(self.out_max.saturating_add(1));
                        self.cache
                            .insert(self.next_sample, State::Mpeg2(Box::new(snap)), from);
                    }
                }
                Backend::Mpeg4(soft) => {
                    if sample.is_none() && soft.flushed {
                        break;
                    }
                    if sample.is_some() {
                        self.next_sample += 1;
                    }
                    let from = self.buffered.len();
                    soft.step(sample, cancel, &mut self.buffered)?;
                    self.note_out(from);
                    self.buffered.retain(|b| b.0 >= cts);
                }
                Backend::Vpx(dec) => {
                    // At most one shown picture per block, in presentation order.
                    let Some((bytes, at)) = sample else {
                        break;
                    };
                    self.next_sample += 1;
                    if let Some(f) = dec.decode(bytes)? {
                        if let Some(a) = dec.take_alpha() {
                            self.alpha.insert(at, a);
                        }
                        let from = self.buffered.len();
                        self.buffered.push((at, f));
                        self.note_out(from);
                    }
                    self.buffered.retain(|b| b.0 >= cts);
                }
            }
        }
        Ok(())
    }

    fn ycbcr(&self, f: &VideoFrame) -> Ycbcr {
        let colr = self.clip.track.colour;
        let matrix = f
            .signal
            .and_then(|s| s.matrix)
            .map(u16::from)
            .filter(|&m| m != 2)
            .or(colr.map(|c| c.matrix).filter(|&m| m != 2))
            // Unstated: HD and larger is BT.709, SD BT.601 (what players assume).
            .unwrap_or(if f.height >= 720 { 1 } else { 6 });
        let full_range = f
            .signal
            .map(|s| s.full_range)
            .or(colr.map(|c| c.full_range))
            .unwrap_or(false);
        Ycbcr { matrix, full_range }
    }

    /// How a picture's planes are coded: the stream's signal, else the container's `colr`,
    /// else what players assume for the size.
    fn encoding(&self, f: &VideoFrame) -> YuvEncoding {
        let colr = self.clip.track.colour;
        let ycbcr = self.ycbcr(f);
        let sig = f.signal;
        let primaries = sig
            .and_then(|s| s.primaries)
            .map(u16::from)
            .filter(|&c| c != 2)
            .or(colr.map(|c| c.primaries));
        let transfer = sig
            .and_then(|s| s.transfer)
            .map(u16::from)
            .filter(|&c| c != 2)
            .or(colr.map(|c| c.transfer));
        YuvEncoding {
            matrix: YuvMatrix::from_h273(Some(ycbcr.matrix), f.height),
            full_range: ycbcr.full_range,
            bits: f.bit_depth.clamp(8, 16),
            cosited_x: match (&self.dec, self.chroma_loc) {
                // The VUI's siting (H.264, HEVC): left, top-left and bottom-left are co-sited.
                (_, Some(loc)) => loc % 2 == 0,
                (Backend::Vpx(v), _) => v.cosited_x(),
                (Backend::Mpeg2(m), _) => m.cosited_x(),
                (Backend::Mpeg4(m), _) => m.cosited_x(),
                _ => self.clip.codec != Codec::Av1 || self.av1_cosited,
            },
            // Top-left and top: chroma rows on the even luma rows (AV1 "colocated" too);
            // bottom-left and bottom (4, 5) on the odd ones.
            cosited_y: matches!(self.chroma_loc, Some(2 | 3))
                || (self.chroma_loc.is_none()
                    && self.clip.codec == Codec::Av1
                    && self.av1_cosited_y),
            bottom_y: matches!(self.chroma_loc, Some(4 | 5)),
            right_x: false,
            alpha: false,
            colour: yuv::h273_colour(primaries, transfer),
        }
    }

    /// The picture's planes as a [`PixelLayout::Yuv`] raster (no per-pixel work).
    fn yuv_raster(&self, f: VideoFrame) -> Raster {
        let e = self.encoding(&f);
        let bps = e.sample_bytes();
        let (w, h) = (f.width, f.height);
        let (cw, ch) = f.chroma.plane_size(w, h);
        let (h_div, v_div) = f.chroma.divisors();
        let (h_div, v_div) = (h_div as u8, v_div as u8);
        let [y, u, v] = f.planes;
        let plane = |mut data: Vec<u8>, pw: u32, ph: u32, h_div: u8, v_div: u8| {
            let stride = pw as usize * bps;
            data.resize(stride * ph as usize, 0);
            if cfg!(target_endian = "big") && bps == 2 {
                // Decoders hand out little-endian samples; the raster's are native.
                for s in data.chunks_exact_mut(2) {
                    s.swap(0, 1);
                }
            }
            RasterPlane {
                width: pw,
                height: ph,
                stride,
                data,
                h_div,
                v_div,
            }
        };
        Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Yuv(e),
            planes: vec![
                plane(y, w, h, 1, 1),
                plane(u, cw, ch, h_div, v_div),
                plane(v, cw, ch, h_div, v_div),
            ],
            truncated: false,
        }
    }

    /// A picture with an alpha plane (`alpha`: luma size, the picture's depth) as straight
    /// alpha RGBA at 16 bits in the picture's transfer and primaries: the viewer shows it
    /// over the checkerboard like any image with alpha.
    fn alpha_raster(&self, f: VideoFrame, alpha: Vec<u8>) -> Result<Raster, DecodeError> {
        let e = self.encoding(&f);
        let f = full_chroma(f, e);
        let ycbcr = self.ycbcr(&f);
        let (w, h) = (f.width, f.height);
        let (cw, chroma) = match f.chroma {
            VChroma::Yuv420 => (w.div_ceil(2) as usize, Chroma::Yuv420),
            VChroma::Yuv422 => (w.div_ceil(2) as usize, Chroma::Yuv422),
            VChroma::Yuv444 | VChroma::Yuv440 | VChroma::Yuv411 => (w as usize, Chroma::Yuv444),
        };
        let bps = if f.bit_depth > 8 { 2 } else { 1 };
        let [y, u, v] = f.planes;
        let frame = HeifFrame {
            width: w,
            height: h,
            format: HeifPixelFormat::new(chroma, f.bit_depth, true)
                .map_err(|e| DecodeError::Unsupported(e.to_string()))?,
            planes: vec![
                Plane::new(w as usize * bps, y),
                Plane::new(cw * bps, u),
                Plane::new(cw * bps, v),
                Plane::new(w as usize * bps, alpha),
            ],
        };
        let opts = ConvertOptions {
            parallel: true,
            wide: true,
            ..ConvertOptions::default()
        };
        let rgba = specular_heic::rgba::convert(&frame, ycbcr, false, &opts)
            .map_err(|e| DecodeError::Malformed(e.to_string()))?;
        Ok(Raster {
            width: rgba.width,
            height: rgba.height,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba16(e.colour),
            planes: vec![RasterPlane {
                width: rgba.width,
                height: rgba.height,
                stride: rgba.width as usize * 8,
                data: rgba.data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }

    fn to_raster(&self, f: VideoFrame) -> Result<Raster, DecodeError> {
        let Shape::Still {
            orientation,
            display,
        } = self.shape
        else {
            return Ok(self.yuv_raster(f));
        };
        let e = self.encoding(&f);
        let f = full_chroma(f, e);
        let ycbcr = self.ycbcr(&f);
        let bps = if f.bit_depth > 8 { 2 } else { 1 };
        let (w, h) = (f.width, f.height);
        let (cw, chroma) = match f.chroma {
            VChroma::Yuv420 => (w.div_ceil(2) as usize, Chroma::Yuv420),
            VChroma::Yuv422 => (w.div_ceil(2) as usize, Chroma::Yuv422),
            VChroma::Yuv444 | VChroma::Yuv440 | VChroma::Yuv411 => (w as usize, Chroma::Yuv444),
        };
        let [y, u, v] = f.planes;
        let frame = HeifFrame {
            width: w,
            height: h,
            format: HeifPixelFormat::new(chroma, f.bit_depth, false)
                .map_err(|e| DecodeError::Unsupported(e.to_string()))?,
            planes: vec![
                Plane::new(w as usize * bps, y),
                Plane::new(cw * bps, u),
                Plane::new(cw * bps, v),
            ],
        };
        let opts = ConvertOptions {
            parallel: true,
            ..ConvertOptions::default()
        };
        let rgba = specular_heic::rgba::convert(&frame, ycbcr, false, &opts)
            .map_err(|e| DecodeError::Malformed(e.to_string()))?;
        let (mut px, mut w, mut h) = (rgba.data, rgba.width, rgba.height);
        let rot = self.clip.track.orientation();
        if rot != 1 {
            (px, w, h) = orient(&px, w, h, rot);
        }
        (px, w, h) = letterbox(px, w, h, display);
        let inverse = match orientation.0 {
            6 => 8,
            8 => 6,
            o => o,
        };
        if inverse != 1 {
            (px, w, h) = orient(&px, w, h, inverse);
        }
        let stored = orientation.display_size(display.0, display.1);
        Ok(Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: (stored.0 / w.max(1)).max(1),
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data: px,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
}

impl Frames for VideoFrames {
    fn next_frame(&mut self, cancel: &CancelToken) -> Option<Result<Frame, DecodeError>> {
        while self.k < self.order.len() {
            let (k, cts) = (self.k, self.order[self.k]);
            self.k += 1;
            if let Err(e) = self.fill(cts, cancel) {
                if matches!(e, DecodeError::Cancelled) {
                    // Stopped between two samples: the next call goes on to this frame.
                    self.k = k;
                }
                return Some(Err(e));
            }
            // Pictures before this one that never came up are dropped with it.
            self.discard_before(cts);
            let Some(i) = self.buffered.iter().position(|b| b.0 == cts) else {
                continue;
            };
            let (_, mut f) = self.buffered.swap_remove(i);
            let alpha = self.alpha.remove(&cts);
            self.alpha.retain(|&t, _| t > cts);
            let progressive = matches!(&self.dec, Backend::Mpeg2(m) if m.progressive(cts));
            if self.deinterlace && !progressive {
                let raw = copy_frame(&f);
                crate::deint::deinterlace(&mut f, self.woven.as_ref());
                self.woven = Some(raw);
            }
            let raster = match alpha {
                Some(a) if matches!(self.shape, Shape::Plain) => self.alpha_raster(f, a),
                _ => self.to_raster(f),
            };
            return Some(raster.map(|raster| Frame {
                index: k as u32,
                raster,
                delay: self.delays[k],
            }));
        }
        None
    }

    fn rewind(&mut self) -> Result<(), DecodeError> {
        self.restart(0)?;
        self.k = 0;
        Ok(())
    }

    /// Positions on frame `index` (presentation order): decoding goes on from where it is
    /// when that reaches the frame from a sync sample already passed, else starts over at the
    /// nearest sync sample before it (one presented no later, so that leading pictures of an
    /// open GOP are not lost) and discards the pictures before it.
    fn seek(&mut self, index: u32) -> Result<bool, DecodeError> {
        let k = index as usize;
        let Some(&cts) = self.order.get(k) else {
            self.k = self.order.len();
            return Ok(true);
        };
        let samples = &self.clip.track.samples;
        let i = self
            .pos
            .get(&cts)
            .copied()
            .unwrap_or(0)
            .min(samples.len() - 1);
        let key = (0..=i)
            .rev()
            .find(|&j| samples[j].sync && samples[j].cts <= cts)
            .unwrap_or(0);
        let key = self.recovery_key(key, i, cts);
        let flushed = match &self.dec {
            Backend::Hard(_) => self.hard_flushed,
            Backend::Soft(s) => s.flushed,
            Backend::H264(s) => s.flushed,
            Backend::Mpeg2(s) => s.flushed,
            Backend::Mpeg4(s) => s.flushed,
            Backend::Av1(_) | Backend::Vpx(_) => false,
        };
        let ready = self.buffered.iter().any(|b| b.0 == cts);
        // Going on decoding serves only a target at or after the next frame to hand out: a
        // frame before it may sit at an earlier decoding position than frames already handed
        // out (B-pictures), and those would not come again. Seeking back from frame 101 to
        // 100 of h264-gop.mp4 skipped 101 when the decoder had not yet reached 100's sample.
        // A target whose sample was decoded already (a reference picture decoded ahead of
        // the B-pictures before it) still comes out: only pictures presented before the
        // frame being decoded for were left out on the way.
        let on_the_way = k >= self.k
            && self.next_sample >= key
            && !flushed
            && !self.passed(cts)
            && !matches!(&self.dec, Backend::Soft(s) if s.gone.contains(&cts));
        if !ready && !on_the_way && !self.resume(key, i, cts) {
            self.restart(key)?;
        }
        self.k = k;
        self.discard_before(cts);
        // The frame before the new position is not the one last shown.
        self.woven = None;
        Ok(true)
    }

    fn delays(&self) -> Option<Vec<Duration>> {
        Some(self.delays.clone())
    }

    /// The frames of the sync samples (where [`Frames::seek`] starts decoding).
    fn sync_frames(&self) -> Option<Vec<u32>> {
        let mut out: Vec<u32> = self
            .clip
            .track
            .samples
            .iter()
            .filter(|s| s.sync)
            .filter_map(|s| self.order.binary_search(&s.cts).ok())
            .filter_map(|k| u32::try_from(k).ok())
            .collect();
        out.sort_unstable();
        out.dedup();
        Some(out)
    }

    fn set_interrupt(&mut self, flag: Arc<AtomicBool>) -> bool {
        self.interrupt = Some(flag);
        true
    }
}

impl VideoFrames {
    /// H.264 in a container that marks only IDR pictures as sync samples (as ISO/IEC 14496-15
    /// has it): the latest open-GOP recovery point after the sync sample `key` and no later
    /// than the target (sample `i`, presented at `cts`), found from the samples' SEI, so the
    /// seek decodes from there rather than from the IDR picture. Progressive streams only.
    fn recovery_key(&mut self, key: usize, i: usize, cts: i64) -> usize {
        if self.clip.codec != Codec::H264 || i <= key || !self.fields.is_empty() {
            return key;
        }
        let Ok((ls, _)) = specular_h264::parse_avcc(&self.clip.track.config) else {
            return key;
        };
        let samples = &self.clip.track.samples;
        for j in (key + 1..=i).rev() {
            let s = &samples[j];
            if s.cts > cts || s.sync {
                continue;
            }
            let hit = *self.recovery.entry(j).or_insert_with(|| {
                self.clip
                    .track
                    .sample_data(&self.clip.data, s)
                    .is_some_and(|b| specular_h264::random_access(&b, ls))
            });
            if hit {
                return j;
            }
        }
        key
    }

    /// Software HEVC, H.264 and MPEG-1/2: resumes from the cached snapshot nearest before the
    /// target (decoding position `target`, presented at `cts`) after the sync sample `key`.
    /// False when there is none to use.
    fn resume(&mut self, key: usize, target: usize, cts: i64) -> bool {
        match &self.dec {
            Backend::Soft(soft) if soft.weaver.is_none() => {}
            Backend::H264(_) | Backend::Mpeg2(_) => {}
            _ => return false,
        }
        while let Some((at, snap)) = self.cache.best(key, target, cts) {
            let restored = match (&mut self.dec, &snap.state) {
                (Backend::Soft(soft), State::Hevc(s)) => {
                    let ok = soft.dec.restore(s);
                    if ok {
                        soft.gone.clear();
                        soft.flushed = false;
                        soft.latest_lost = i64::MIN;
                    }
                    ok
                }
                (Backend::H264(soft), State::H264(s)) => {
                    soft.restore(s);
                    true
                }
                (Backend::Mpeg2(soft), State::Mpeg2(s)) => {
                    soft.restore(s);
                    true
                }
                _ => false,
            };
            if restored {
                debug!(target: specular_log::DECODE, "video: seek resumes at sample {at} (sync sample {key}, target {target})");
                self.resumed += 1;
                self.run_from = snap.valid_from;
                self.after_restart(at);
                return true;
            }
            self.cache.forget(at);
        }
        false
    }

    /// Drops the buffered pictures presented before `cts`. A snapshot taken after this
    /// serves no target before the latest of them (software HEVC).
    fn discard_before(&mut self, cts: i64) {
        discard_before(&mut self.buffered, &mut self.run_from, cts);
    }

    /// The state that goes with a decoder about to decode sample `from`.
    fn after_restart(&mut self, from: usize) {
        self.next_sample = from;
        self.hard_flushed = false;
        self.last_out = None;
        self.buffered.clear();
        if let Some(s) = self.slots.as_mut() {
            s.clear();
        }
        self.woven = None;
        self.alpha.clear();
        self.cache.last_at = from;
        self.out_max = i64::MIN;
    }

    /// A fresh decoder about to decode sample `from` (a sync sample).
    fn restart(&mut self, from: usize) -> Result<(), DecodeError> {
        // Leading pictures of the sync sample may be left out from here (open GOPs):
        // snapshots of this run serve no target before it.
        self.run_from = self
            .clip
            .track
            .samples
            .get(from)
            .map_or(i64::MIN, |s| s.cts);
        match &mut self.dec {
            Backend::Hard(dec) => {
                // H.264 from an open GOP's recovery point: a decoder of its own. The device
                // keeps per-surface state (co-located motion) that a reset leaves behind, and
                // the leading pictures' stand-in references would predict from it.
                let fresh = self.clip.codec == Codec::H264
                    && self.clip.track.samples.get(from).is_some_and(|s| {
                        self.clip
                            .track
                            .sample_data(&self.clip.data, s)
                            .is_some_and(|b| !idr_sample(&b, &self.clip.track.config))
                    });
                match self.clip.hw_codec().filter(|_| fresh) {
                    Some(c) => {
                        **dec = VideoDecoder::new(c, &self.clip.track.config).map_err(hw_err)?;
                    }
                    None => dec.reset(),
                }
            }
            Backend::Soft(soft) => soft.reset(),
            Backend::H264(soft) => {
                **soft = SwH264::with_order(&self.clip.track.config, self.slots.is_some())
                    .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
            }
            Backend::Av1(dec) => dec.reset(),
            Backend::Vpx(dec) => dec.reset(),
            Backend::Mpeg2(soft) => {
                **soft = Mpeg2Dec::new(&self.clip.track.config)
                    .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
            }
            Backend::Mpeg4(soft) => {
                **soft = Mpeg4Dec::new(&self.clip.track)
                    .map_err(|e| DecodeError::Unsupported(e.to_string()))?;
            }
        }
        self.after_restart(from);
        Ok(())
    }

    /// Frame times in presentation order.
    pub fn frame_delays(&self) -> &[Duration] {
        &self.delays
    }

    /// When the first frame is presented on the movie's timeline (its edit's empty lead-in,
    /// plus how far the frame is from the edit's start), in microseconds.
    pub fn first_frame_us(&self) -> i64 {
        presented_at_us(&self.clip.track, self.order.first().copied().unwrap_or(0))
    }

    /// How many seeks resumed from a decoder snapshot near their target instead of from the
    /// sync sample before it (software HEVC, H.264 and MPEG-1/2; for diagnostics and tests).
    pub fn seeks_resumed(&self) -> u32 {
        self.resumed
    }

    /// How many samples were handed to the decoder so far: what seeks cost, in pictures
    /// decoded (for measurements and tests).
    pub fn samples_decoded(&self) -> u64 {
        self.samples_decoded
    }

    /// Turns the seek cache off (on by default) and drops its snapshots: to measure seeks
    /// without it.
    #[doc(hidden)]
    pub fn set_seek_cache(&mut self, on: bool) {
        if !on {
            self.cache = SeekCache::new();
        }
        self.cache.off = !on;
    }

    /// Bytes the seek cache reserved from the memory budget for the pictures its snapshots
    /// hold (H.264 and MPEG-1/2; HEVC pictures hold their own; for tests).
    #[doc(hidden)]
    pub fn seek_cache_reserved(&self) -> u64 {
        self.cache.reserved()
    }

    /// Software HEVC: bytes of the distinct decoded pictures the decoder and its seek
    /// snapshots hold (each reserved once from the memory budget; for tests).
    #[doc(hidden)]
    pub fn held_picture_bytes(&self) -> u64 {
        match &self.dec {
            Backend::Soft(soft) => self
                .cache
                .bytes_with(Some(&State::Hevc(soft.dec.snapshot()))),
            _ => 0,
        }
    }

    /// Which decoder runs: "hardware" or "software".
    pub fn backend(&self) -> &'static str {
        match self.dec {
            Backend::Hard(_) => "hardware",
            _ => "software",
        }
    }

    /// Whether frames are deinterlaced (interlaced video shown on its own). Off, an interlaced
    /// clip's frames come as decoded, both fields woven (the tests compare them with ffmpeg's).
    pub fn deinterlacing(&self) -> bool {
        self.deinterlace
    }

    pub fn set_deinterlace(&mut self, on: bool) {
        self.deinterlace = on && self.clip.interlaced();
        self.woven = None;
    }
}

fn copy_frame(f: &VideoFrame) -> VideoFrame {
    VideoFrame {
        width: f.width,
        height: f.height,
        bit_depth: f.bit_depth,
        chroma: f.chroma,
        planes: f.planes.clone(),
        signal: f.signal,
    }
}

/// `src` (RGBA, `w`×`h`, stored) as displayed with EXIF orientation `o`.
fn orient(src: &[u8], w: u32, h: u32, o: u8) -> (Vec<u8>, u32, u32) {
    let (w, h) = (w as usize, h as usize);
    let swap = o >= 5;
    let (dw, dh) = if swap { (h, w) } else { (w, h) };
    let mut out = vec![0u8; dw * dh * 4];
    for (y, row) in out.chunks_exact_mut(dw * 4).enumerate() {
        for (x, px) in row.chunks_exact_mut(4).enumerate() {
            let (sx, sy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                8 => (w - 1 - y, x),
                _ => (x, y),
            };
            let s = (sy * w + sx) * 4;
            px.copy_from_slice(&src[s..s + 4]);
        }
    }
    (out, dw as u32, dh as u32)
}

/// Pads `px` (RGBA, opaque) with black to the aspect of `target`, centred. Differences under
/// 1 % are left alone (rounding of the two sizes).
fn letterbox(px: Vec<u8>, w: u32, h: u32, target: (u32, u32)) -> (Vec<u8>, u32, u32) {
    let (tw, th) = (target.0.max(1) as u64, target.1.max(1) as u64);
    let (fw, fh) = (w as u64, h as u64);
    let (a, b) = (fw * th, fh * tw);
    if a.abs_diff(b) * 100 <= a.max(b) || w == 0 || h == 0 {
        return (px, w, h);
    }
    let (nw, nh) = if a > b {
        (fw, (fw * th).div_ceil(tw))
    } else {
        ((fh * tw).div_ceil(th), fh)
    };
    let (nw, nh) = (nw as usize, nh as usize);
    let (ox, oy) = ((nw - w as usize) / 2, (nh - h as usize) / 2);
    let mut out = [0u8, 0, 0, 255].repeat(nw * nh);
    let row = w as usize * 4;
    for (y, src) in px.chunks_exact(row).enumerate() {
        let d = ((oy + y) * nw + ox) * 4;
        out[d..d + row].copy_from_slice(src);
    }
    (out, nw as u32, nh as u32)
}

/// A 4:4:0 or 4:1:1 picture with its chroma interpolated to full size (bilinear, at the
/// encoding's siting, the shader's and `yuv::to_rgb`'s filter): the still-picture and alpha
/// conversions take 4:2:0, 4:2:2 and 4:4:4 only. Other pictures come back as they are.
fn full_chroma(mut f: VideoFrame, e: YuvEncoding) -> VideoFrame {
    if !matches!(f.chroma, VChroma::Yuv440 | VChroma::Yuv411) {
        return f;
    }
    let (hd, vd) = f.chroma.divisors();
    let (cw, ch) = f.chroma.plane_size(f.width, f.height);
    let (w, h, cw, ch) = (
        f.width as usize,
        f.height as usize,
        cw.max(1) as usize,
        ch.max(1) as usize,
    );
    let bps = if f.bit_depth > 8 { 2 } else { 1 };
    // Source position of an output column / row, and the two taps either side.
    let taps = |n: usize, div: u32, cosited: bool, bottom: bool, len: usize| {
        let d = div as f32;
        (0..n)
            .map(|i| {
                let p = if div <= 1 {
                    i as f32
                } else if cosited {
                    i as f32 / d
                } else if bottom {
                    (i as f32 + 1.0) / d - 1.0
                } else {
                    (i as f32 + 0.5) / d - 0.5
                }
                .max(0.0);
                let i0 = (p.floor() as usize).min(len - 1);
                (i0, (i0 + 1).min(len - 1), p - p.floor())
            })
            .collect::<Vec<_>>()
    };
    let cols = taps(w, hd, e.cosited_x, false, cw);
    let rows = taps(h, vd, e.cosited_y, e.bottom_y, ch);
    for c in 1..3 {
        let src = &f.planes[c];
        let at = |x: usize, y: usize| -> f32 {
            let o = (y * cw + x) * bps;
            match bps {
                2 => src
                    .get(o..o + 2)
                    .map_or(0.0, |b| f32::from(u16::from_le_bytes([b[0], b[1]]))),
                _ => src.get(o).map_or(0.0, |&v| f32::from(v)),
            }
        };
        let mut out = Vec::with_capacity(w * h * bps);
        for &(y0, y1, fy) in &rows {
            for &(x0, x1, fx) in &cols {
                let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
                let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
                let v = (top + (bottom - top) * fy).round();
                if bps == 2 {
                    out.extend_from_slice(&(v as u16).to_le_bytes());
                } else {
                    out.push(v as u8);
                }
            }
        }
        f.planes[c] = out;
    }
    f.chroma = VChroma::Yuv444;
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3×2 image whose pixels are numbered 0..6 in the first byte.
    fn numbered() -> Vec<u8> {
        (0..6u8).flat_map(|i| [i, 0, 0, 255]).collect()
    }

    fn firsts(v: &[u8]) -> Vec<u8> {
        v.chunks_exact(4).map(|p| p[0]).collect()
    }

    #[test]
    fn av1_pictures_in_4_4_0_pass_as_such() {
        let p = VideoPicture {
            width: 4,
            height: 4,
            bits: 8,
            planes: [vec![16; 16], vec![128; 8], vec![128; 8]],
            sizes: [(4, 4), (4, 2), (4, 2)],
            ssx: false,
            ssy: true,
            cosited_x: false,
            cosited_y: false,
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false,
        };
        let f = av1_frame(p).unwrap();
        assert_eq!(f.chroma, VChroma::Yuv440);
        assert_eq!(f.chroma.plane_size(4, 4), (4, 2));
    }

    #[test]
    fn orientations_turn_like_exif() {
        // 0 1 2
        // 3 4 5
        let src = numbered();
        let (o, w, h) = orient(&src, 3, 2, 6);
        assert_eq!((w, h), (2, 3));
        assert_eq!(firsts(&o), [3, 0, 4, 1, 5, 2]);
        let (o, ..) = orient(&src, 3, 2, 8);
        assert_eq!(firsts(&o), [2, 5, 1, 4, 0, 3]);
        let (o, ..) = orient(&src, 3, 2, 3);
        assert_eq!(firsts(&o), [5, 4, 3, 2, 1, 0]);
        let (o, ..) = orient(&src, 3, 2, 5);
        assert_eq!(firsts(&o), [0, 3, 1, 4, 2, 5]);
        let (o, ..) = orient(&src, 3, 2, 7);
        assert_eq!(firsts(&o), [5, 2, 4, 1, 3, 0]);
        // 6 then its inverse 8 is the identity.
        let (a, w, h) = orient(&src, 3, 2, 6);
        let (b, ..) = orient(&a, w, h, 8);
        assert_eq!(b, src);
    }

    /// A composition time that never labels a picture (here a second field's) stops `fill` at
    /// the first picture decoded after it (give or take the decoder's pipelining) instead of
    /// decoding the clip to its end.
    #[test]
    fn fill_stops_past_a_time_without_a_picture() {
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/corpus/out/video-ref/clips/h264-jvt-cvpa1.mp4");
        let Ok(data) = std::fs::read(&d) else {
            eprintln!("skipped: run tools/corpus/video-refs.sh");
            return;
        };
        let clip = Clip::from_bytes(data).unwrap();
        let (mut frames, _) = clip.frames(Orientation(1), (352, 288)).unwrap();
        let samples = frames.clip.track.samples.len();
        let second = frames
            .fields
            .iter()
            .position(|f| f.completes_open)
            .expect("a field per sample");
        let cts = frames.clip.track.samples[second].cts;
        assert!(
            !frames.order.contains(&cts),
            "a second field is not a frame"
        );
        let cancel = CancelToken::new();
        frames.fill(cts, &cancel).unwrap();
        assert!(frames.buffered.iter().all(|b| b.0 != cts));
        assert!(
            frames.next_sample < second + 32 && frames.next_sample < samples / 2,
            "decoded {} of {samples} samples for a time at sample {second}",
            frames.next_sample
        );
    }

    #[test]
    fn letterbox_pads_to_the_target_aspect() {
        // 16:9 into 4:3: bars above and below.
        let px = [9u8, 9, 9, 255].repeat(16 * 9);
        let (o, w, h) = letterbox(px.clone(), 16, 9, (4000, 3000));
        assert_eq!((w, h), (16, 12));
        assert_eq!(&o[..4], &[0, 0, 0, 255]);
        assert_eq!(&o[(16 * 2) * 4..][..4], &[9, 9, 9, 255]);
        // Same aspect: untouched.
        let (_, w, h) = letterbox(px, 16, 9, (1920, 1080));
        assert_eq!((w, h), (16, 9));
    }

    /// Software HEVC pictures (4:2:0 and 4:2:2, 8 and 10 bits) against libavcodec: every
    /// `NAME.mp4` in `SPECULAR_HEVC_CLIPS` with a `NAME.yuv` beside it (`ffmpeg -i NAME.mp4
    /// -f rawvideo -pix_fmt yuv422p10le NAME.yuv`, the stream's own format). Skips without it.
    #[test]
    fn software_hevc_matches_libavcodec() {
        let Some(dir) = std::env::var_os("SPECULAR_HEVC_CLIPS") else {
            return;
        };
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let path = e.unwrap().path();
            if path.extension().is_none_or(|e| e != "mp4") {
                continue;
            }
            let Ok(want) = std::fs::read(path.with_extension("yuv")) else {
                continue;
            };
            let data = std::fs::read(&path).unwrap();
            let m = mp4::parse(&data, true).unwrap();
            let v = m.video.as_ref().unwrap();
            let mut soft = Soft::new(v).unwrap();
            let cancel = CancelToken::new();
            let mut got = Vec::new();
            for s in &v.samples {
                let b = mp4::sample_bytes(&data, s).unwrap();
                soft.step(Some((b, s.cts)), i64::MIN, &cancel, &mut got)
                    .unwrap();
            }
            soft.step(None, i64::MIN, &cancel, &mut got).unwrap();
            got.sort_by_key(|f| f.0);
            let bytes: Vec<u8> = got.iter().flat_map(|(_, f)| f.planes.concat()).collect();
            let f = &got[0].1;
            eprintln!(
                "{}: {} frames, {:?}, {} bits",
                path.display(),
                got.len(),
                f.chroma,
                f.bit_depth
            );
            assert!(bytes == want, "{}", path.display());
            n += 1;
        }
        assert!(n > 0, "no clips with .yuv references");
    }
}
