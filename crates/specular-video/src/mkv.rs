//! Matroska and WebM demux (`.mkv`, `.webm`, `.mka`) into the same [`Movie`] the MP4 demuxer
//! gives, so playback, sound and the facts work the same way.
//!
//! Zero-copy over the whole file: the EBML header (DocType `matroska` or `webm`), the Segment
//! (also of unknown size, as live recordings write it), `Info` (timestamp scale, duration,
//! date), `Tracks` (codec, codec private data, default and enabled flags, default duration,
//! codec delay, the video size, display size and colour elements, the audio rate and
//! channels), `Cues`, `Tags` (a recording date) and every `Cluster` (also of unknown size):
//! `SimpleBlock` and `BlockGroup` with Xiph, fixed-size and EBML lacing. The default video
//! track and the default audio track are taken (the first of each when none is marked).
//!
//! Every frame of the two tracks becomes a [`Sample`] (its bytes are contiguous, so
//! [`Layout::Contiguous`] reads them); times are in microseconds. Block timestamps are
//! presentation times: a video sample's `cts` is its block's, the frame lasts until the next
//! one in presentation order. Without a keyframe flag (a `BlockGroup` without `ReferenceBlock`
//! is one), the cue points mark the keyframes. Invisible frames take the time of the next
//! shown frame, so they never count as frames of their own.
//!
//! Without samples (`samples == false`) the clusters are not read: the duration comes from
//! `Info`, else from the last cluster (found from the end of the file), else from the cues.
//!
//! Tracks stored compressed (`ContentEncodings`: header stripping, zlib, bzip2) are read
//! through [`Layout::Mkv`], which undoes it per frame (see [`coding`]); LZO and encrypted
//! tracks are refused by name. A file cut short keeps the frames before the cut.

use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Colour, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};
use std::borrow::Cow;
use std::sync::Arc;

pub(crate) mod coding;
pub use coding::Coding;
use coding::Refusal;

/// How a Matroska track's frames are read when they are stored encoded: each sample's
/// bytes as stored, made readable by the track's [`Coding`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MkvLayout {
    pub coding: Arc<Coding>,
}

impl MkvLayout {
    /// A sample's readable bytes from its bytes as stored.
    pub fn read<'a>(&self, raw: &'a [u8]) -> Option<Cow<'a, [u8]>> {
        self.coding.frame(raw)
    }
}

/// The layout of a track stored with `coding`.
fn layout(coding: &Coding) -> Layout {
    if coding.is_plain() {
        Layout::Contiguous
    } else {
        Layout::Mkv(MkvLayout {
            coding: Arc::new(coding.clone()),
        })
    }
}

const EBML_HEADER: u32 = 0x1A45_DFA3;
const DOC_TYPE: u32 = 0x4282;
const DOC_TYPE_READ_VERSION: u32 = 0x4285;
const SEGMENT: u32 = 0x1853_8067;
const SEEK_HEAD: u32 = 0x114D_9B74;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2AD7B1;
const DURATION: u32 = 0x4489;
const DATE_UTC: u32 = 0x4461;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const FLAG_ENABLED: u32 = 0xB9;
const FLAG_DEFAULT: u32 = 0x88;
const DEFAULT_DURATION: u32 = 0x23E383;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const CODEC_DELAY: u32 = 0x56AA;
const CONTENT_ENCODINGS: u32 = 0x6D80;
const VIDEO: u32 = 0xE0;
const PIXEL_WIDTH: u32 = 0xB0;
const COLOUR_SPACE: u32 = 0x2EB524;
const PIXEL_HEIGHT: u32 = 0xBA;
const DISPLAY_WIDTH: u32 = 0x54B0;
const DISPLAY_HEIGHT: u32 = 0x54BA;
const FLAG_INTERLACED: u32 = 0x9A;
const COLOUR: u32 = 0x55B0;
const MATRIX: u32 = 0x55B1;
const RANGE: u32 = 0x55B9;
const TRANSFER: u32 = 0x55BA;
const PRIMARIES: u32 = 0x55BB;
const PROJECTION: u32 = 0x7670;
const PROJECTION_TYPE: u32 = 0x7671;
const PROJECTION_POSE_YAW: u32 = 0x7673;
const PROJECTION_POSE_PITCH: u32 = 0x7674;
const PROJECTION_POSE_ROLL: u32 = 0x7675;
const AUDIO: u32 = 0xE1;
const SAMPLING_FREQUENCY: u32 = 0xB5;
const CHANNELS: u32 = 0x9F;
const BIT_DEPTH: u32 = 0x6264;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const BLOCK_DURATION: u32 = 0x9B;
const REFERENCE_BLOCK: u32 = 0xFB;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;
const CUE_CLUSTER_POSITION: u32 = 0xF1;
const TAGS: u32 = 0x1254_C367;
const TAG: u32 = 0x7373;
const SIMPLE_TAG: u32 = 0x67C8;
const TAG_NAME: u32 = 0x45A3;
const TAG_STRING: u32 = 0x4487;

/// Top-level elements of a Segment: one of these ends a cluster of unknown size.
const TOP_LEVEL: [u32; 9] = [
    CLUSTER,
    CUES,
    TAGS,
    INFO,
    TRACKS,
    SEEK_HEAD,
    0x1043_A770, // Chapters
    0x1941_A469, // Attachments
    EBML_HEADER,
];

/// Deepest nesting followed (tags nest).
const MAX_DEPTH: usize = 8;
/// Nanoseconds from 1970-01-01 to 2001-01-01 (Matroska's `DateUTC` epoch).
const EPOCH_2001_S: i64 = 978_307_200;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Whether `head` starts with an EBML header of a Matroska or WebM file.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(&EBML_HEADER.to_be_bytes()) && doc_type(head).is_some()
}

/// The DocType of the EBML header at the start of `data` (`matroska` or `webm`).
fn doc_type(data: &[u8]) -> Option<&'static str> {
    let e = element(data, 0)?;
    if e.id != EBML_HEADER {
        return None;
    }
    let mut kind = None;
    for c in children(data, e.body, e.end.min(data.len())) {
        if c.id == DOC_TYPE {
            kind = match data.get(c.body..c.end)? {
                b"matroska" => Some("matroska"),
                b"webm" => Some("webm"),
                _ => None,
            };
        }
    }
    kind
}

/// An element: its ID, where its body starts and ends (`end` is the data's length for an
/// unknown size, and may lie past it when the file is cut short).
#[derive(Clone, Copy, Debug)]
pub(crate) struct El {
    pub(crate) id: u32,
    pub(crate) body: usize,
    pub(crate) end: usize,
    pub(crate) unknown: bool,
}

/// An EBML variable-size integer at `p`: value (marker removed) and length; `None` for the
/// all-ones value (an unknown size) is reported as `u64::MAX`.
fn vint(data: &[u8], p: usize) -> Option<(u64, usize)> {
    let first = *data.get(p)?;
    if first == 0 {
        return None;
    }
    let len = first.leading_zeros() as usize + 1;
    let bytes = data.get(p..p + len)?;
    let mut v = u64::from(first) & (0xFF >> len);
    for &b in &bytes[1..] {
        v = (v << 8) | u64::from(b);
    }
    let all_ones = (1u64 << (7 * len)) - 1;
    Some((if v == all_ones { u64::MAX } else { v }, len))
}

/// An element ID at `p` (marker kept, up to 4 bytes).
fn id(data: &[u8], p: usize) -> Option<(u32, usize)> {
    let first = *data.get(p)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 4 {
        return None;
    }
    let bytes = data.get(p..p + len)?;
    Some((
        bytes.iter().fold(0u32, |a, &b| (a << 8) | u32::from(b)),
        len,
    ))
}

/// The element starting at `p`.
pub(crate) fn element(data: &[u8], p: usize) -> Option<El> {
    let (id, il) = id(data, p)?;
    let (size, sl) = vint(data, p + il)?;
    let body = p + il + sl;
    let (end, unknown) = if size == u64::MAX {
        (data.len(), true)
    } else {
        (
            usize::try_from(size)
                .ok()
                .and_then(|s| body.checked_add(s))
                .unwrap_or(usize::MAX),
            false,
        )
    };
    Some(El {
        id,
        body,
        end,
        unknown,
    })
}

/// The child elements of the body `from..to` (stops at the first one that can't be read).
pub(crate) fn children(data: &[u8], from: usize, to: usize) -> impl Iterator<Item = El> + '_ {
    let to = to.min(data.len());
    let mut p = from;
    std::iter::from_fn(move || {
        if p >= to {
            return None;
        }
        let e = element(data, p)?;
        p = if e.unknown { to } else { e.end };
        Some(e)
    })
}

pub(crate) fn uint(data: &[u8], e: &El) -> Option<u64> {
    let b = data.get(e.body..e.end)?;
    (b.len() <= 8).then(|| b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x)))
}

fn sint(data: &[u8], e: &El) -> Option<i64> {
    let b = data.get(e.body..e.end)?;
    if b.is_empty() || b.len() > 8 {
        return (b.is_empty()).then_some(0);
    }
    let v = b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x));
    let shift = 64 - 8 * b.len() as u32;
    Some(((v << shift) as i64) >> shift)
}

fn float(data: &[u8], e: &El) -> Option<f64> {
    let b = data.get(e.body..e.end)?;
    match b.len() {
        4 => Some(f64::from(f32::from_be_bytes(b.try_into().ok()?))),
        8 => Some(f64::from_be_bytes(b.try_into().ok()?)),
        0 => Some(0.0),
        _ => None,
    }
    .filter(|f| f.is_finite())
}

pub(crate) fn string(data: &[u8], e: &El) -> Option<String> {
    let b = data.get(e.body..e.end)?;
    let b = b.split(|&c| c == 0).next().unwrap_or_default();
    Some(String::from_utf8_lossy(b).trim().to_owned())
}

/// A track entry as read.
#[derive(Clone, Debug, Default)]
struct Entry {
    number: u64,
    kind: u64,
    enabled: bool,
    default: bool,
    default_duration_ns: Option<u64>,
    codec_id: String,
    private: Vec<u8>,
    codec_delay_ns: u64,
    /// How the frames and the codec private data are stored (`ContentEncodings`).
    coding: Coding,
    /// Why the frames can't be read (encrypted, LZO, damaged codec private data).
    refusal: Option<Refusal>,
    width: u32,
    height: u32,
    display: Option<(u64, u64)>,
    interlaced: bool,
    colour: Option<Colour>,
    roll: Option<f64>,
    /// `ProjectionPoseYaw` is ±180 (a rectangular picture mirrored left to right, before
    /// the roll).
    mirror: bool,
    rate: f64,
    channels: u16,
    bits: u16,
    /// `ColourSpace`: the four-character code of an uncompressed track's layout.
    colour_space: Option<[u8; 4]>,
}

fn track_entry(data: &[u8], e: &El) -> Entry {
    let mut t = Entry {
        enabled: true,
        default: true,
        channels: 1,
        rate: 8000.0,
        ..Entry::default()
    };
    for c in children(data, e.body, e.end) {
        match c.id {
            TRACK_NUMBER => t.number = uint(data, &c).unwrap_or(0),
            TRACK_TYPE => t.kind = uint(data, &c).unwrap_or(0),
            FLAG_ENABLED => t.enabled = uint(data, &c) != Some(0),
            FLAG_DEFAULT => t.default = uint(data, &c) != Some(0),
            DEFAULT_DURATION => t.default_duration_ns = uint(data, &c).filter(|&d| d > 0),
            CODEC_ID => t.codec_id = string(data, &c).unwrap_or_default(),
            CODEC_PRIVATE => {
                t.private = data.get(c.body..c.end).unwrap_or_default().to_vec();
            }
            CODEC_DELAY => t.codec_delay_ns = uint(data, &c).unwrap_or(0),
            CONTENT_ENCODINGS => t.coding = Coding::read(data, &c),
            VIDEO => video_settings(data, &c, &mut t),
            AUDIO => {
                for a in children(data, c.body, c.end) {
                    match a.id {
                        SAMPLING_FREQUENCY => t.rate = float(data, &a).unwrap_or(t.rate),
                        CHANNELS => t.channels = uint(data, &a).unwrap_or(1).clamp(1, 64) as u16,
                        BIT_DEPTH => t.bits = uint(data, &a).unwrap_or(0).min(64) as u16,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    // The codec private data as the codec reads it (it may be stored compressed).
    t.refusal = t.coding.refusal();
    if t.refusal.is_none() && !t.private.is_empty() {
        match t.coding.private(&t.private) {
            Some(Cow::Owned(p)) => t.private = p,
            Some(Cow::Borrowed(_)) => {}
            None => t.refusal = Some(Refusal::Compressed),
        }
    }
    // The Musepack decoder reads its packets in place in the file.
    if t.codec_id == "A_MPC" && !t.coding.is_plain() {
        t.refusal = t.refusal.or(Some(Refusal::Compressed));
    }
    t
}

fn video_settings(data: &[u8], v: &El, t: &mut Entry) {
    let (mut dw, mut dh) = (None, None);
    for c in children(data, v.body, v.end) {
        match c.id {
            PIXEL_WIDTH => t.width = uint(data, &c).unwrap_or(0).min(1 << 16) as u32,
            PIXEL_HEIGHT => t.height = uint(data, &c).unwrap_or(0).min(1 << 16) as u32,
            DISPLAY_WIDTH => dw = uint(data, &c).filter(|&x| x > 0),
            DISPLAY_HEIGHT => dh = uint(data, &c).filter(|&x| x > 0),
            FLAG_INTERLACED => t.interlaced = uint(data, &c) == Some(1),
            COLOUR_SPACE => {
                t.colour_space = data.get(c.body..c.end).and_then(|b| b.try_into().ok());
            }
            COLOUR => {
                let (mut m, mut tr, mut p, mut r) = (2u16, 2u16, 2u16, 0u64);
                let mut any = false;
                for k in children(data, c.body, c.end) {
                    let v = || uint(data, &k).unwrap_or(2).min(255) as u16;
                    match k.id {
                        MATRIX => (m, any) = (v(), true),
                        TRANSFER => (tr, any) = (v(), true),
                        PRIMARIES => (p, any) = (v(), true),
                        RANGE => (r, any) = (uint(data, &k).unwrap_or(0), true),
                        _ => {}
                    }
                }
                if any {
                    t.colour = Some(Colour {
                        primaries: p,
                        transfer: tr,
                        matrix: m,
                        full_range: r == 2,
                    });
                }
            }
            PROJECTION => {
                // A flat picture's turn and mirror (RFC 9559 5.1.4.1.32.3.4–6): yaw 180 mirrors
                // it, then the roll turns it. Anything else (a pitch, another yaw, a 360°
                // projection) is not a flat turn: ignored, as ffmpeg does.
                let (mut kind, mut yaw, mut pitch, mut roll) = (0, 0.0, 0.0, None);
                for k in children(data, c.body, c.end) {
                    match k.id {
                        PROJECTION_TYPE => kind = uint(data, &k).unwrap_or(0),
                        PROJECTION_POSE_YAW => yaw = float(data, &k).unwrap_or(0.0),
                        PROJECTION_POSE_PITCH => pitch = float(data, &k).unwrap_or(0.0),
                        PROJECTION_POSE_ROLL => roll = float(data, &k),
                        _ => {}
                    }
                }
                if kind == 0 && pitch == 0.0 && (yaw == 0.0 || yaw.abs() == 180.0) {
                    t.roll = roll;
                    t.mirror = yaw != 0.0;
                }
            }
            _ => {}
        }
    }
    if let (Some(w), Some(h)) = (dw, dh) {
        t.display = Some((w, h));
    }
}

/// Matroska's video codec IDs as the MP4 demuxer's codecs.
fn video_codec(id: &str, private: &[u8]) -> Codec {
    match id {
        "V_MPEG4/ISO/AVC" => Codec::H264,
        "V_MPEGH/ISO/HEVC" => Codec::Hevc,
        "V_AV1" => Codec::Av1,
        "V_VP8" => Codec::Vp8,
        "V_VP9" => Codec::Vp9,
        "V_MPEG1" => Codec::Mpeg1,
        "V_MPEG2" => Codec::Mpeg2,
        "V_THEORA" => Codec::Theora,
        "V_PRORES" => Codec::ProRes,
        "V_MJPEG" => Codec::Mjpeg,
        "V_FFV1" => Codec::Ffv1,
        "V_DIRAC" => Codec::Vc2,
        "V_UNCOMPRESSED" => Codec::Raw,
        "V_MS/VFW/FOURCC" => {
            // BITMAPINFOHEADER: biCompression at offset 16.
            let c = private
                .get(16..20)
                .and_then(|c| <[u8; 4]>::try_from(c).ok());
            c.map_or(Codec::Other(*b"VFW "), crate::avi::fourcc_codec)
        }
        "V_MPEG4/ISO/SP" | "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/AP" => Codec::Mpeg4,
        s if s.starts_with("V_MPEG4/ISO/") => Codec::Other(*b"mp4v"),
        s if s.starts_with("V_REAL/") => Codec::Other(*b"RV40"),
        _ => Codec::Other(*b"????"),
    }
}

/// Matroska's audio codec IDs as the MP4 demuxer's codecs.
fn audio_codec(t: &Entry) -> AudioCodec {
    let id = t.codec_id.as_str();
    match id {
        "A_OPUS" => AudioCodec::Opus(t.private.clone()),
        "A_VORBIS" => AudioCodec::Vorbis(t.private.clone()),
        "A_PCM/INT/LIT" | "A_PCM/INT/BIG" | "A_PCM/FLOAT/IEEE" => AudioCodec::Pcm {
            bits: t.bits,
            float: id == "A_PCM/FLOAT/IEEE",
            little_endian: id != "A_PCM/INT/BIG",
        },
        "A_FLAC" => AudioCodec::Flac(t.private.clone()),
        "A_AC3" | "A_AC3/BSID9" | "A_AC3/BSID10" => AudioCodec::Ac3,
        "A_EAC3" => AudioCodec::Eac3,
        "A_DTS" => AudioCodec::Other(*b"dtsc", 0),
        "A_AMR/NB" => AudioCodec::Other(*b"samr", 0),
        "A_AMR/WB" => AudioCodec::Other(*b"sawb", 0),
        "A_DTS/EXPRESS" => AudioCodec::Other(*b"dtse", 0),
        "A_DTS/LOSSLESS" => AudioCodec::Other(*b"dtsl", 0),
        "A_TRUEHD" => AudioCodec::Other(*b"mlpa", 0),
        "A_MPEG/L3" => AudioCodec::Mpa(3),
        "A_MPEG/L2" => AudioCodec::Mpa(2),
        "A_MPEG/L1" => AudioCodec::Mpa(1),
        "A_ALAC" => AudioCodec::Alac(t.private.clone()),
        // Musepack SV8: the stream header is the codec private ([`parse_with`] reads it).
        "A_MPC" => AudioCodec::Other(*b"MPC8", 0),
        "A_MS/ACM" => {
            crate::avi::wave_codec(&t.private).map_or(AudioCodec::Other(*b"????", 0), |(c, _, _)| c)
        }
        s if s.starts_with("A_AAC") => AudioCodec::Aac(if t.private.is_empty() {
            aac_config(s, t)
        } else {
            t.private.clone()
        }),
        s if s.starts_with("A_REAL/") => AudioCodec::Other(*b"cook", 0),
        _ => AudioCodec::Other(*b"????", 0),
    }
}

/// An AudioSpecificConfig for the old codec ids that carry none (`A_AAC/MPEG4/LC/SBR` and
/// the like): the profile from the id, rate and channels from the track.
fn aac_config(id: &str, t: &Entry) -> Vec<u8> {
    let object_type = if id.contains("/MAIN") {
        1
    } else if id.contains("/SSR") {
        3
    } else if id.contains("/LTP") {
        4
    } else {
        2
    };
    let rate = t.rate.round() as u32;
    let channel_config = match t.channels {
        n @ 1..=6 => n as u8,
        8 => 7,
        _ => 2,
    };
    if id.ends_with("/SBR") {
        // the track may give the core rate or the output rate
        let core = if rate > 24000 { rate / 2 } else { rate };
        specular_aac::make_asc(object_type, core, channel_config, Some(2 * core))
    } else {
        specular_aac::make_asc(object_type, rate, channel_config, None)
    }
}

/// A frame of a block: where its bytes are, its time (in timestamp-scale ticks, from the
/// segment's start), the block duration when given, and the keyframe and invisible flags.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) offset: usize,
    pub(crate) size: usize,
    pub(crate) ticks: i64,
    /// The frame's index in its lace and the lace's frame count.
    lace: (u16, u16),
    pub(crate) duration: Option<u64>,
    key: Option<bool>,
    invisible: bool,
}

/// The frames of a block body at `b..end` of a track in `want` (by track number).
fn block_frames(
    data: &[u8],
    b: usize,
    end: usize,
    cluster_ticks: i64,
    simple: bool,
    want: &[u64],
    out: &mut Vec<(u64, Frame)>,
) -> Option<()> {
    let end = end.min(data.len());
    // Laced frames may be empty (256 frames from a 7-byte block): the frame cap is checked
    // per block, not only per cluster (one unknown-size cluster can span the file).
    if out.len() > 2 * MAX_SAMPLES {
        return None;
    }
    let (track, tl) = vint(data, b)?;
    if !want.contains(&track) {
        return Some(());
    }
    let p = b + tl;
    let rel = i16::from_be_bytes(data.get(p..p + 2)?.try_into().ok()?);
    let flags = *data.get(p + 2)?;
    let mut p = p + 3;
    let ticks = cluster_ticks + i64::from(rel);
    let key = simple.then_some(flags & 0x80 != 0);
    let invisible = flags & 0x08 != 0;
    let lacing = (flags >> 1) & 3;
    let mut sizes: Vec<usize> = Vec::new();
    if lacing != 0 {
        let n = usize::from(*data.get(p)?) + 1;
        p += 1;
        match lacing {
            1 => {
                // Xiph: each size but the last as a run of 255s and a final byte.
                for _ in 0..n - 1 {
                    let mut s = 0usize;
                    loop {
                        let x = *data.get(p)?;
                        p += 1;
                        s += usize::from(x);
                        if x != 255 {
                            break;
                        }
                    }
                    sizes.push(s);
                }
            }
            3 => {
                // EBML: the first size, then signed differences.
                let (first, l) = vint(data, p)?;
                p += l;
                let mut s = usize::try_from(first).ok()?;
                sizes.push(s);
                for _ in 1..n - 1 {
                    let (raw, l) = vint(data, p)?;
                    p += l;
                    let bias = (1i64 << (7 * l - 1)) - 1;
                    let d = i64::try_from(raw).ok()? - bias;
                    s = usize::try_from(i64::try_from(s).ok()? + d).ok()?;
                    sizes.push(s);
                }
            }
            _ => {
                // Fixed: equal parts.
                let total = end.checked_sub(p)?;
                if total % n != 0 {
                    return None;
                }
                sizes = vec![total / n; n - 1];
            }
        }
        let used: usize = sizes.iter().sum();
        sizes.push(end.checked_sub(p)?.checked_sub(used)?);
    } else {
        sizes.push(end.checked_sub(p)?);
    }
    let n = sizes.len() as u16;
    for (i, s) in sizes.into_iter().enumerate() {
        if p + s > end {
            return None;
        }
        out.push((
            track,
            Frame {
                offset: p,
                size: s,
                ticks,
                lace: (i as u16, n),
                duration: None,
                key,
                invisible,
            },
        ));
        p += s;
    }
    Some(())
}

/// Reads the blocks of the cluster whose body starts at `body` (ending at `end`, or for an
/// unknown size at the first top-level element) into `out`; returns where the cluster ends.
pub(crate) fn cluster(
    data: &[u8],
    body: usize,
    end: usize,
    unknown: bool,
    want: &[u64],
    out: &mut Vec<(u64, Frame)>,
) -> usize {
    let to = end.min(data.len());
    let mut ticks = 0i64;
    let mut p = body;
    while p < to {
        if out.len() > 2 * MAX_SAMPLES {
            return to;
        }
        let Some(e) = element(data, p) else {
            return to;
        };
        if unknown && TOP_LEVEL.contains(&e.id) {
            return p;
        }
        match e.id {
            CLUSTER_TIMESTAMP => ticks = uint(data, &e).unwrap_or(0).min(i64::MAX as u64) as i64,
            SIMPLE_BLOCK => {
                // A block cut short by the end of the file is dropped.
                let _ = block_frames(data, e.body, e.end, ticks, true, want, out);
            }
            BLOCK_GROUP => {
                let at = out.len();
                let (mut duration, mut referenced) = (None, false);
                for c in children(data, e.body, e.end) {
                    match c.id {
                        BLOCK => {
                            let _ = block_frames(data, c.body, c.end, ticks, false, want, out);
                        }
                        BLOCK_DURATION => duration = uint(data, &c),
                        REFERENCE_BLOCK => referenced = true,
                        _ => {}
                    }
                }
                for (_, f) in &mut out[at..] {
                    f.key = Some(!referenced);
                    f.duration = duration;
                }
            }
            _ => {}
        }
        if e.unknown {
            return to;
        }
        p = e.end;
    }
    to
}

/// Samples per channel at 48 kHz of an Opus packet (RFC 6716 §3.1), for laced frames' times.
pub(crate) fn opus_samples(p: &[u8]) -> Option<u64> {
    let toc = *p.first()?;
    let config = toc >> 3;
    let frame = match config {
        0..=11 => [480, 960, 1920, 2880][usize::from(config & 3)],
        12..=15 => [480, 960][usize::from(config & 1)],
        _ => [120, 240, 480, 960][usize::from(config & 3)],
    };
    let count = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => u64::from(*p.get(1)? & 0x3F),
    };
    Some(frame * count)
}

const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const TITLE: u32 = 0x7BA9;
const WRITING_APP: u32 = 0x5741;
const TARGETS: u32 = 0x63C0;
const TARGET_TYPE_VALUE: u32 = 0x68CA;
const ATTACHMENTS: u32 = 0x1941_A469;
const ATTACHED_FILE: u32 = 0x61A7;
const FILE_NAME: u32 = 0x466E;
const FILE_MIME_TYPE: u32 = 0x4660;
const FILE_DATA: u32 = 0x465C;

/// What a Matroska file says of itself for the Info panel: its title, the program that wrote
/// it, its tags and its attached files.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileTags<'a> {
    /// The segment's title (`Info`).
    pub title: Option<String>,
    /// The program that wrote it (`Info`).
    pub writing_app: Option<String>,
    /// Every `SimpleTag` (nested ones too): its tag's target type value (50 album, 30
    /// track; 50 when the tag names none), its name in upper case and its text.
    pub simple: Vec<(u64, String, String)>,
    /// Attached files: name, MIME type and bytes.
    pub attachments: Vec<(String, String, &'a [u8])>,
}

/// The text tags of a movie for its Info panel: the segment's title and writing program, and
/// the simple tags that describe the whole file (target 50 and above, or none), in order.
fn movie_tags(data: &[u8]) -> Vec<(String, String)> {
    let Some(t) = file_tags(data) else {
        return Vec::new();
    };
    let info = [("Title", t.title), ("WritingApp", t.writing_app)];
    info.into_iter()
        .filter_map(|(k, v)| Some((k.to_owned(), v.filter(|v| !v.is_empty())?)))
        .chain(
            t.simple
                .into_iter()
                .filter(|(target, _, v)| *target >= 50 && !v.is_empty())
                .map(|(_, k, v)| (k, v))
                .take(256),
        )
        .collect()
}

/// The tags of the Matroska file `data`: `Info`, `Tags` and `Attachments`, found in order or,
/// past clusters, through the `SeekHead`. `None` when it is not a Matroska file.
pub fn file_tags<'a>(data: &'a [u8]) -> Option<FileTags<'a>> {
    doc_type(data)?;
    let header = element(data, 0)?;
    let mut p = header.end;
    let seg = loop {
        let e = element(data, p)?;
        if e.id == SEGMENT {
            break e;
        }
        if e.unknown || e.end <= p {
            return None;
        }
        p = e.end;
    };
    let seg_end = seg.end.min(data.len());
    let mut out = FileTags::default();
    let mut seen: Vec<usize> = Vec::new();
    let mut seeks: Vec<usize> = Vec::new();
    let mut visit = |at: usize, out: &mut FileTags<'a>, seeks: &mut Vec<usize>| -> Option<El> {
        let e = element(data, at)?;
        if seen.contains(&at) {
            return Some(e);
        }
        seen.push(at);
        match e.id {
            SEEK_HEAD => {
                for s in children(data, e.body, e.end).filter(|c| c.id == SEEK) {
                    let (mut id, mut pos) = (None, None);
                    for c in children(data, s.body, s.end) {
                        match c.id {
                            SEEK_ID => id = uint(data, &c),
                            SEEK_POSITION => pos = uint(data, &c),
                            _ => {}
                        }
                    }
                    let wanted = matches!(
                        id.and_then(|i| u32::try_from(i).ok()),
                        Some(INFO | TAGS | ATTACHMENTS | SEEK_HEAD)
                    );
                    if wanted
                        && let Some(at) = pos
                            .and_then(|q| usize::try_from(q).ok())
                            .and_then(|q| seg.body.checked_add(q))
                            .filter(|&q| q < seg_end)
                    {
                        seeks.push(at);
                    }
                }
            }
            INFO => {
                for c in children(data, e.body, e.end) {
                    match c.id {
                        TITLE => out.title = out.title.take().or_else(|| string(data, &c)),
                        WRITING_APP => {
                            out.writing_app = out.writing_app.take().or_else(|| string(data, &c));
                        }
                        _ => {}
                    }
                }
            }
            TAGS => {
                for tag in children(data, e.body, e.end).filter(|c| c.id == TAG) {
                    let target = children(data, tag.body, tag.end)
                        .find(|c| c.id == TARGETS)
                        .and_then(|t| {
                            children(data, t.body, t.end)
                                .find(|c| c.id == TARGET_TYPE_VALUE)
                                .and_then(|c| uint(data, &c))
                        })
                        .unwrap_or(50);
                    simple_tags(data, &tag, target, 0, &mut out.simple);
                }
            }
            ATTACHMENTS => {
                for f in children(data, e.body, e.end).filter(|c| c.id == ATTACHED_FILE) {
                    let (mut name, mut mime, mut bytes) = (String::new(), String::new(), None);
                    for c in children(data, f.body, f.end) {
                        match c.id {
                            FILE_NAME => name = string(data, &c).unwrap_or_default(),
                            FILE_MIME_TYPE => mime = string(data, &c).unwrap_or_default(),
                            FILE_DATA => bytes = data.get(c.body..c.end),
                            _ => {}
                        }
                    }
                    if let Some(b) = bytes {
                        out.attachments.push((name, mime, b));
                    }
                }
            }
            _ => {}
        }
        Some(e)
    };
    // In order up to the first cluster of unknown size, then what the SeekHead points at.
    let mut p = seg.body;
    while p < seg_end {
        let Some(e) = visit(p, &mut out, &mut seeks) else {
            break;
        };
        if e.unknown || e.end <= p {
            break;
        }
        p = e.end;
    }
    let mut k = 0;
    while let Some(&at) = seeks.get(k) {
        visit(at, &mut out, &mut seeks);
        k += 1;
        if k > 64 {
            break;
        }
    }
    Some(out)
}

/// The `SimpleTag`s of `e` (a `Tag` or a `SimpleTag`) with their target, nested ones too.
fn simple_tags(
    data: &[u8],
    e: &El,
    target: u64,
    depth: usize,
    out: &mut Vec<(u64, String, String)>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for c in children(data, e.body, e.end).filter(|c| c.id == SIMPLE_TAG) {
        let (mut name, mut value) = (None, None);
        for k in children(data, c.body, c.end) {
            match k.id {
                TAG_NAME => name = string(data, &k),
                TAG_STRING => value = string(data, &k),
                _ => {}
            }
        }
        if let (Some(n), Some(v)) = (name, value)
            && out.len() < 4096
        {
            out.push((target, n.to_ascii_uppercase(), v));
        }
        simple_tags(data, &c, target, depth + 1, out);
    }
}

/// Parses a Matroska or WebM file. With `samples`, every frame of the default video and audio
/// tracks is listed.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    parse_with(data, samples, None)
}

/// [`parse`] taking the sound track numbered `audio` (Matroska's `TrackNumber`) when there
/// is one, else the default sound track.
pub fn parse_with(data: &[u8], samples: bool, audio: Option<u64>) -> Result<Movie> {
    if doc_type(data).is_none() {
        return Err(bad("not a Matroska or WebM file"));
    }
    let header = element(data, 0).ok_or_else(|| bad("EBML header"))?;
    for c in children(data, header.body, header.end) {
        if c.id == DOC_TYPE_READ_VERSION && uint(data, &c).is_some_and(|v| v > 4) {
            return Err(Error::Unsupported("a newer Matroska version".into()));
        }
    }
    // The segment follows the header (after any Void elements).
    let mut p = header.end;
    let seg = loop {
        let e = element(data, p).ok_or_else(|| bad("no Segment"))?;
        if e.id == SEGMENT {
            break e;
        }
        if e.unknown || e.end <= p {
            return Err(bad("no Segment"));
        }
        p = e.end;
    };
    let seg_end = seg.end.min(data.len());

    let mut scale_ns: u64 = 1_000_000;
    let mut duration_ticks: Option<f64> = None;
    let mut date_utc: Option<i64> = None;
    let mut entries: Vec<Entry> = Vec::new();
    let mut cues: Vec<(u64, u64, u64)> = Vec::new();
    let mut tag_date: Option<String> = None;
    let mut cluster_at: Vec<(usize, El)> = Vec::new();
    let mut tracks_seen = false;

    let mut p = seg.body;
    while p < seg_end {
        let Some(e) = element(data, p) else { break };
        match e.id {
            INFO => {
                for c in children(data, e.body, e.end) {
                    match c.id {
                        TIMESTAMP_SCALE => {
                            scale_ns = uint(data, &c).filter(|&s| s > 0).unwrap_or(1_000_000);
                        }
                        DURATION => duration_ticks = float(data, &c).filter(|&d| d > 0.0),
                        DATE_UTC => date_utc = sint(data, &c),
                        _ => {}
                    }
                }
            }
            TRACKS if !tracks_seen => {
                tracks_seen = true;
                entries = children(data, e.body, e.end)
                    .filter(|c| c.id == TRACK_ENTRY)
                    .map(|c| track_entry(data, &c))
                    .collect();
            }
            CUES => {
                for cp in children(data, e.body, e.end).filter(|c| c.id == CUE_POINT) {
                    let mut time = 0;
                    for c in children(data, cp.body, cp.end) {
                        match c.id {
                            CUE_TIME => time = uint(data, &c).unwrap_or(0),
                            CUE_TRACK_POSITIONS => {
                                let (mut track, mut pos) = (0, 0);
                                for k in children(data, c.body, c.end) {
                                    match k.id {
                                        CUE_TRACK => track = uint(data, &k).unwrap_or(0),
                                        CUE_CLUSTER_POSITION => pos = uint(data, &k).unwrap_or(0),
                                        _ => {}
                                    }
                                }
                                cues.push((time, track, pos));
                            }
                            _ => {}
                        }
                    }
                }
            }
            TAGS => tag_date = tag_date.or_else(|| recorded_tag(data, &e, 0)),
            CLUSTER => {
                if !samples {
                    // Clusters are not read; jump past a sized one, or stop at an unknown one
                    // (what follows is found from the end of the file).
                    if e.unknown {
                        break;
                    }
                } else {
                    cluster_at.push((p, e));
                    if e.unknown {
                        // Its end is found by reading it (below); continue from there.
                        let mut scratch = Vec::new();
                        p = cluster(data, e.body, e.end, true, &[], &mut scratch);
                        continue;
                    }
                }
            }
            _ => {}
        }
        if e.unknown || e.end <= p {
            break;
        }
        p = e.end;
    }

    if entries.is_empty() {
        return Err(bad("no Tracks"));
    }
    // The default track of each kind: enabled and marked default, else the first enabled.
    let pick = |kind: u64| {
        entries
            .iter()
            .filter(|t| t.kind == kind && t.enabled && t.number > 0)
            .max_by_key(|t| (t.default, std::cmp::Reverse(t.number)))
            .cloned()
    };
    let vt = pick(1);
    let at = audio
        .and_then(|n| {
            entries
                .iter()
                .find(|t| t.kind == 2 && t.number == n)
                .cloned()
        })
        .or_else(|| pick(2));
    let us = |ticks: i64| -> i64 {
        (i128::from(ticks) * i128::from(scale_ns) / 1000).clamp(i64::MIN as i128, i64::MAX as i128)
            as i64
    };

    let mut movie = Movie {
        brand: *b"mkv ",
        timescale: 1_000_000,
        duration_us: duration_ticks.map(|d| (d * scale_ns as f64 / 1000.0) as u64),
        created: date_utc.map(|ns| EPOCH_2001_S + ns.div_euclid(1_000_000_000)),
        has_audio: at.is_some(),
        date: tag_date,
        tags: movie_tags(data),
        ..Movie::default()
    };

    let mut video = vt.as_ref().map(|t| {
        let codec = video_codec(&t.codec_id, &t.private);
        let pixel_aspect = t.display.and_then(|(dw, dh)| {
            let (a, b) = (dw * u64::from(t.height), dh * u64::from(t.width));
            let g = gcd(a, b).max(1);
            let (a, b) = (a / g, b / g);
            (a != b && a > 0 && b > 0 && a <= u64::from(u32::MAX) && b <= u64::from(u32::MAX))
                .then_some((a as u32, b as u32))
        });
        let rotation = t.roll.map_or(0, |r| {
            // A counter-clockwise roll; the display turns the other way.
            match ((-r).round() as i64).rem_euclid(360) {
                90 => 90,
                180 => 180,
                270 => 270,
                _ => 0,
            }
        });
        // Video for Windows tracks: a BITMAPINFOHEADER before the codec's own data, and no
        // presentation times (B-frames are shown in their blocks' time slots, like AVI).
        let vfw = t.codec_id == "V_MS/VFW/FOURCC";
        let config = if vfw {
            crate::avi::bitmap_config(codec, &t.private)
        } else if codec == Codec::Raw {
            // The layout is the track's colour space code.
            crate::rawvid::PixDesc {
                fourcc: t.colour_space.unwrap_or(*b"????"),
                ..Default::default()
            }
            .to_config()
        } else {
            t.private.clone()
        };
        let mut v = VideoTrack {
            codec: Some(codec),
            config,
            slot_timing: vfw,
            width: t.width,
            height: t.height,
            rotation,
            mirror: t.mirror,
            timescale: 1_000_000,
            colour: t.colour,
            pixel_aspect,
            interlaced: t.interlaced,
            layout: layout(&t.coding),
            ..VideoTrack::default()
        };
        v.display = v.shown_size();
        v
    });
    let mut audio = at.as_ref().map(|t| {
        let codec = audio_codec(t);
        let opus = matches!(codec, AudioCodec::Opus(_));
        // Opus decodes at 48 kHz; its first `pre-skip` samples are not heard (the codec
        // delay, or the head's pre-skip when no delay is given).
        let start_us = if opus {
            let pre_skip = t
                .private
                .get(10..12)
                .map_or(0, |b| u64::from(u16::from_le_bytes([b[0], b[1]])));
            if t.codec_delay_ns > 0 {
                t.codec_delay_ns / 1000
            } else {
                pre_skip * 1_000_000 / 48_000
            }
        } else {
            t.codec_delay_ns / 1000
        };
        AudioTrack {
            codec,
            channels: t.channels,
            sample_rate: if opus { 48_000 } else { t.rate.round() as u32 },
            timescale: 1_000_000,
            duration: 0,
            sample_count: 0,
            samples: Vec::new(),
            edit: Edit {
                start: start_us as i64,
                delay_us: 0,
                length_us: None,
            },
            layout: layout(&t.coding),
            extra: Default::default(),
        }
    });
    // Encrypted or LZO-compressed frames can't be read: said by name.
    if let (Some(v), Some(why)) = (video.as_mut(), vt.as_ref().and_then(|t| t.refusal)) {
        v.codec = Some(Codec::Other(why.code()));
    }
    if let (Some(a), Some(t)) = (audio.as_mut(), at.as_ref())
        && t.codec_id == "A_MPC"
    {
        // The header kept with the track (a damaged one is said when it is played).
        a.extra.head = t.private.clone();
        if let Ok((codec, rate, channels, edit)) = crate::mpc_sound::matroska(&t.private) {
            a.codec = codec;
            a.sample_rate = rate;
            a.channels = channels;
            a.edit = edit;
        }
    }
    if let (Some(a), Some(why)) = (audio.as_mut(), at.as_ref().and_then(|t| t.refusal)) {
        a.codec = AudioCodec::Other(why.code(), 0);
    }

    if samples {
        let want: Vec<u64> = [vt.as_ref(), at.as_ref()]
            .into_iter()
            .flatten()
            .map(|t| t.number)
            .collect();
        let mut frames: Vec<(u64, Frame)> = Vec::new();
        for &(_, e) in &cluster_at {
            cluster(data, e.body, e.end, e.unknown, &want, &mut frames);
            if frames.len() > 2 * MAX_SAMPLES {
                return Err(Error::Unsupported(format!("{} frames", frames.len())));
            }
        }
        if let (Some(v), Some(t)) = (video.as_mut(), vt.as_ref()) {
            let keys: Vec<u64> = cues
                .iter()
                .filter(|c| c.1 == t.number)
                .map(|c| c.0)
                .collect();
            let f: Vec<Frame> = frames
                .iter()
                .filter(|f| f.0 == t.number)
                .map(|f| f.1)
                .collect();
            v.samples = video_samples(&f, t, scale_ns, &keys, &us)?;
            v.sample_count = v.samples.len() as u32;
            v.duration = v
                .samples
                .iter()
                .map(|s| s.cts + i64::from(s.duration))
                .max()
                .unwrap_or(0)
                .max(0) as u64;
        }
        if let (Some(a), Some(t)) = (audio.as_mut(), at.as_ref()) {
            let f: Vec<Frame> = frames
                .iter()
                .filter(|f| f.0 == t.number)
                .map(|f| f.1)
                .collect();
            a.samples = audio_samples(data, &f, t, a, scale_ns, &us)?;
            if !a.extra.head.is_empty() {
                mpc_samples(data, a);
            }
            // Blocks before time 0 (an encoder's priming, as ffmpeg writes MP3 and AAC):
            // the timeline is shifted so they keep their place, and time 0 is where the
            // edit starts.
            let first = a.samples.iter().map(|s| s.cts).min().unwrap_or(0);
            if first < 0 {
                for s in &mut a.samples {
                    s.dts = (s.cts - first) as u64;
                }
                a.edit.start -= first;
            }
            a.sample_count = a.samples.len() as u32;
            a.duration = a
                .samples
                .last()
                .map_or(0, |s| s.dts + u64::from(s.duration));
        }
    } else {
        // Without the blocks: the duration from the header, else the last cluster, else
        // the cues.
        if movie.duration_us.is_none() {
            let want: Vec<u64> = vt.iter().chain(at.iter()).map(|t| t.number).collect();
            let end = last_cluster_end(data, seg.body, seg_end, &want)
                .or_else(|| cues.iter().map(|c| c.0 as i64).max());
            movie.duration_us = end.map(|t| us(t).max(0) as u64);
        }
        if let Some(v) = video.as_mut()
            && let Some(d) = movie.duration_us
        {
            v.duration = d;
        }
    }
    if movie.duration_us.is_none() {
        movie.duration_us = video
            .as_ref()
            .map(|v| v.duration)
            .filter(|&d| d > 0)
            .or(audio.as_ref().map(|a| a.duration).filter(|&d| d > 0));
    }
    movie.video = video;
    movie.audio = audio;
    Ok(movie)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A recording date from `Tags`: `DATE_RECORDED`, else `DATE`, as written.
fn recorded_tag(data: &[u8], e: &El, depth: usize) -> Option<String> {
    if depth > MAX_DEPTH {
        return None;
    }
    let mut found: Option<(u8, String)> = None;
    for c in children(data, e.body, e.end) {
        let v = match c.id {
            TAG => recorded_tag(data, &c, depth + 1).map(|s| (0, s)),
            SIMPLE_TAG => {
                let (mut name, mut value) = (None, None);
                for k in children(data, c.body, c.end) {
                    match k.id {
                        TAG_NAME => name = string(data, &k),
                        TAG_STRING => value = string(data, &k),
                        _ => {}
                    }
                }
                match (name.as_deref(), value) {
                    (Some("DATE_RECORDED"), Some(v)) => Some((0, v)),
                    (Some("DATE"), Some(v)) => Some((1, v)),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(v) = v.filter(|v| !v.1.is_empty())
            && found.as_ref().is_none_or(|f| v.0 < f.0)
        {
            found = Some(v);
        }
    }
    found.map(|f| f.1)
}

/// The end time (ticks) of the last cluster that can be read, found from the end of the file:
/// its last block's time plus its duration (or the gap before it).
fn last_cluster_end(data: &[u8], from: usize, to: usize, want: &[u64]) -> Option<i64> {
    let id = CLUSTER.to_be_bytes();
    let window = to.saturating_sub(from).min(64 << 20);
    let mut p = to;
    let lo = to - window;
    while p > lo + 4 {
        p -= 1;
        if data.get(p..p + 4) != Some(&id[..]) {
            continue;
        }
        let Some(e) = element(data, p) else { continue };
        if e.id != CLUSTER {
            continue;
        }
        let mut out = Vec::new();
        cluster(data, e.body, e.end, e.unknown, want, &mut out);
        let last = out.iter().map(|f| f.1).max_by_key(|f| f.ticks)?;
        let mut times: Vec<i64> = out.iter().map(|f| f.1.ticks).collect();
        times.sort_unstable();
        times.dedup();
        let gap = times
            .windows(2)
            .map(|w| w[1] - w[0])
            .min()
            .unwrap_or(0)
            .max(0);
        let d = last.duration.map_or(gap, |d| d.min(i64::MAX as u64) as i64);
        return Some(last.ticks + d);
    }
    None
}

/// The video track's samples in decode order (the block order), with presentation times.
fn video_samples(
    f: &[Frame],
    t: &Entry,
    scale_ns: u64,
    cue_keys: &[u64],
    us: &dyn Fn(i64) -> i64,
) -> Result<Vec<Sample>> {
    if f.len() > MAX_SAMPLES {
        return Err(Error::Unsupported(format!("{} frames", f.len())));
    }
    let frame_ticks = t
        .default_duration_ns
        .map(|d| (d as f64 / scale_ns as f64).round() as i64);
    let mut s: Vec<Sample> = f
        .iter()
        .map(|f| {
            let ticks = f.ticks + i64::from(f.lace.0) * frame_ticks.unwrap_or(0);
            Sample {
                offset: f.offset as u64,
                size: f.size.min(u32::MAX as usize) as u32,
                dts: 0,
                cts: us(ticks),
                duration: 0,
                sync: f.key.unwrap_or(false) && f.lace.0 == 0
                    || f.key.is_none() && cue_keys.contains(&(f.ticks.max(0) as u64)),
            }
        })
        .collect();
    // Invisible frames take the next shown frame's time.
    let mut next: Option<i64> = None;
    for (i, fr) in f.iter().enumerate().rev() {
        if fr.invisible {
            if let Some(n) = next {
                s[i].cts = n;
            }
        } else {
            next = Some(s[i].cts);
        }
    }
    // Each frame lasts until the next in presentation order; the last one its block's
    // duration, else the default duration, else the typical gap.
    let mut order: Vec<i64> = s.iter().map(|x| x.cts).collect();
    order.sort_unstable();
    order.dedup();
    let typical = order
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 0)
        .min()
        .unwrap_or(33_333);
    let last_us = f
        .iter()
        .rev()
        .find(|f| !f.invisible)
        .and_then(|f| f.duration)
        .map(|d| us(d.min(i64::MAX as u64) as i64))
        .or(t.default_duration_ns.map(|d| (d / 1000) as i64))
        .filter(|&d| d > 0)
        .unwrap_or(typical);
    for x in &mut s {
        let k = order.partition_point(|&c| c <= x.cts);
        let d = order.get(k).map_or(last_us, |n| n - x.cts);
        x.duration = d.clamp(0, i64::from(u32::MAX)) as u32;
        x.dts = x.cts.max(0) as u64;
    }
    if !s.is_empty() && !s.iter().any(|x| x.sync) {
        // Nothing says which frames are keyframes: take the first.
        s[0].sync = true;
    }
    Ok(s)
}

/// The audio track's samples (one per frame of a lace), times in microseconds.
fn audio_samples(
    data: &[u8],
    f: &[Frame],
    t: &Entry,
    a: &AudioTrack,
    scale_ns: u64,
    us: &dyn Fn(i64) -> i64,
) -> Result<Vec<Sample>> {
    if f.len() > MAX_SAMPLES {
        return Err(Error::Unsupported(format!("{} sound frames", f.len())));
    }
    let opus = matches!(a.codec, AudioCodec::Opus(_));
    let mut out: Vec<Sample> = Vec::with_capacity(f.len());
    // Laced frames: each after the previous by its own length.
    let mut at_us = 0i64;
    for (i, fr) in f.iter().enumerate() {
        let start = if fr.lace.0 == 0 { us(fr.ticks) } else { at_us };
        let dur_us = if opus {
            data.get(fr.offset..fr.offset + fr.size)
                .and_then(|b| t.coding.frame(b))
                .and_then(|b| opus_samples(&b))
                .map(|n| (n * 1_000_000 / 48_000) as i64)
        } else {
            None
        }
        .or(t.default_duration_ns.map(|d| (d / 1000) as i64))
        .or(fr
            .duration
            .map(|d| us(d.min(i64::MAX as u64) as i64) / i64::from(fr.lace.1.max(1))))
        .or_else(|| {
            f.get(i + 1)
                .filter(|n| n.lace.0 == 0)
                .map(|n| (us(n.ticks) - start) / i64::from(fr.lace.1.max(1)))
        })
        .unwrap_or(0)
        .max(0);
        at_us = start + dur_us;
        out.push(Sample {
            offset: fr.offset as u64,
            size: fr.size.min(u32::MAX as usize) as u32,
            dts: start.max(0) as u64,
            cts: start,
            duration: dur_us.min(i64::from(u32::MAX)) as u32,
            sync: true,
        });
    }
    let _ = scale_ns;
    Ok(out)
}

/// Musepack SV8 blocks: a block holding a whole `AP` packet is read from its body, and each
/// lasts its frames (the header's frames per packet, the last cut to the stream's length).
fn mpc_samples(data: &[u8], a: &mut AudioTrack) {
    let Ok(info) = specular_mpc::sv8_header(&a.extra.head) else {
        return;
    };
    let rate = u64::from(info.sample_rate.max(1));
    let mut left = specular_mpc::frames_needed(&info);
    for s in &mut a.samples {
        let (o, n) = (s.offset as usize, s.size as usize);
        if let Some(b) = data.get(o..o + n) {
            let r = specular_mpc::sv8_audio_body(b);
            s.offset += r.start as u64;
            s.size = r.len() as u32;
        }
        let frames = u64::from(info.frames_per_packet.max(1)).min(left);
        left -= frames;
        let us = frames * specular_mpc::FRAME as u64 * 1_000_000 / rate;
        s.duration = us.min(u64::from(u32::MAX)) as u32;
    }
}

#[cfg(test)]
mod remux_tests;

#[cfg(test)]
pub(crate) mod build {
    //! Writes small Matroska files for the tests.

    pub fn el(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        let idb = id.to_be_bytes();
        let skip = idb.iter().position(|&b| b != 0).unwrap_or(3);
        v.extend_from_slice(&idb[skip..]);
        // 8-byte size.
        v.push(0x01);
        v.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
        v.extend_from_slice(body);
        v
    }

    pub fn uel(id: u32, x: u64) -> Vec<u8> {
        el(id, &x.to_be_bytes())
    }

    /// An element of unknown size.
    pub fn open(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        let idb = id.to_be_bytes();
        let skip = idb.iter().position(|&b| b != 0).unwrap_or(3);
        v.extend_from_slice(&idb[skip..]);
        v.push(0xFF);
        v.extend_from_slice(body);
        v
    }

    pub fn header(doc: &str) -> Vec<u8> {
        el(super::EBML_HEADER, &el(super::DOC_TYPE, doc.as_bytes()))
    }

    pub fn simple_block(track: u8, rel: i16, key: bool, payload: &[u8]) -> Vec<u8> {
        let mut b = vec![0x80 | track];
        b.extend_from_slice(&rel.to_be_bytes());
        b.push(if key { 0x80 } else { 0 });
        b.extend_from_slice(payload);
        el(super::SIMPLE_BLOCK, &b)
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    fn tracks() -> Vec<u8> {
        let mut v = uel(TRACK_NUMBER, 1);
        v.extend(uel(TRACK_TYPE, 1));
        v.extend(el(CODEC_ID, b"V_VP9"));
        let mut video = uel(PIXEL_WIDTH, 64);
        video.extend(uel(PIXEL_HEIGHT, 48));
        video.extend(uel(DISPLAY_WIDTH, 128));
        video.extend(uel(DISPLAY_HEIGHT, 48));
        v.extend(el(VIDEO, &video));
        let mut a = uel(TRACK_NUMBER, 2);
        a.extend(uel(TRACK_TYPE, 2));
        a.extend(el(CODEC_ID, b"A_OPUS"));
        let mut head = b"OpusHead\x01\x02".to_vec();
        head.extend_from_slice(&312u16.to_le_bytes());
        head.extend_from_slice(&48000u32.to_le_bytes());
        head.extend_from_slice(&[0, 0, 0]);
        a.extend(el(CODEC_PRIVATE, &head));
        a.extend(uel(CODEC_DELAY, 6_500_000));
        let mut t = el(TRACK_ENTRY, &v);
        t.extend(el(TRACK_ENTRY, &a));
        el(TRACKS, &t)
    }

    fn file(unknown: bool) -> Vec<u8> {
        let mut seg = el(INFO, &uel(TIMESTAMP_SCALE, 1_000_000));
        seg.extend(tracks());
        for c in 0..2u64 {
            let mut cl = uel(CLUSTER_TIMESTAMP, c * 100);
            for k in 0..3i16 {
                cl.extend(simple_block(1, k * 33, k == 0, &[c as u8, k as u8, 9]));
                // Two 20 ms Opus frames (config 1: SILK NB 20 ms), Xiph-laced.
                let mut b = vec![0x82];
                b.extend_from_slice(&(k * 33).to_be_bytes());
                b.push(0x82);
                b.push(1);
                b.push(2);
                b.extend_from_slice(&[0x08, 0xAA, 0x08, 0xBB, 0xCC]);
                cl.extend(el(SIMPLE_BLOCK, &b));
            }
            seg.extend(if unknown {
                open(CLUSTER, &cl)
            } else {
                el(CLUSTER, &cl)
            });
        }
        let mut f = header("webm");
        f.extend(if unknown {
            open(SEGMENT, &seg)
        } else {
            el(SEGMENT, &seg)
        });
        f
    }

    /// Fixed-laced blocks of 256 empty frames each (14 bytes a block) in one cluster: the
    /// frame list stops at the cap instead of growing with the file.
    #[test]
    fn empty_laced_frames_capped() {
        let block = el(SIMPLE_BLOCK, &[0x81, 0, 0, 0x04, 255]);
        let blocks = 2 * MAX_SAMPLES / 256 + 2048;
        let body = block.repeat(blocks);
        let mut out = Vec::new();
        cluster(&body, 0, body.len(), true, &[1], &mut out);
        assert!(out.len() <= 2 * MAX_SAMPLES + 256, "{}", out.len());
    }

    #[test]
    fn blocks_become_samples() {
        for unknown in [false, true] {
            let data = file(unknown);
            assert!(sniff(&data));
            let m = parse(&data, true).unwrap();
            let v = m.video.as_ref().unwrap();
            assert_eq!(v.codec, Some(Codec::Vp9));
            assert_eq!(v.samples.len(), 6, "unknown size: {unknown}");
            let cts: Vec<i64> = v.samples.iter().map(|s| s.cts).collect();
            assert_eq!(cts, [0, 33_000, 66_000, 100_000, 133_000, 166_000]);
            assert_eq!(
                v.samples.iter().map(|s| s.sync).collect::<Vec<_>>(),
                [true, false, false, true, false, false]
            );
            assert_eq!(v.samples[1].duration, 33_000);
            assert_eq!(
                crate::mp4::sample_bytes(&data, &v.samples[4]).unwrap(),
                &[1, 1, 9]
            );
            // 64×48 shown as 128×48.
            assert_eq!(v.pixel_aspect, Some((2, 1)));
            assert_eq!(v.display, (128, 48));
            let a = m.audio.as_ref().unwrap();
            assert!(matches!(a.codec, AudioCodec::Opus(_)));
            assert_eq!(a.sample_rate, 48_000);
            assert_eq!(a.edit.start, 6_500);
            assert_eq!(a.samples.len(), 12);
            assert_eq!(a.samples[1].dts, 20_000);
            assert_eq!(
                crate::mp4::sample_bytes(&data, &a.samples[1]).unwrap(),
                &[0x08, 0xBB, 0xCC]
            );
            assert_eq!(m.duration_us(), Some(199_000));
        }
    }

    #[test]
    fn lacing_kinds() {
        // EBML lacing: 3 frames of 2, 4 and 1 bytes.
        let mut b = vec![0x81, 0, 0, 0x86, 2];
        b.push(0x82); // first size 2
        b.push(0xC1); // +2 (bias 63)
        b.extend_from_slice(&[1, 1, 2, 2, 2, 2, 3]);
        let mut out = Vec::new();
        block_frames(&b, 0, b.len(), 0, true, &[1], &mut out).unwrap();
        let sizes: Vec<usize> = out.iter().map(|f| f.1.size).collect();
        assert_eq!(sizes, [2, 4, 1]);
        // Fixed lacing: 2 frames of 3.
        let b = [0x81, 0, 0, 0x84, 1, 1, 2, 3, 4, 5, 6];
        out.clear();
        block_frames(&b, 0, b.len(), 0, true, &[1], &mut out).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].1.offset, 8);
        // Lacing past the end of the block is refused, not a panic.
        let b = [0x81, 0, 0, 0x82, 3, 200];
        assert!(block_frames(&b, 0, b.len(), 0, true, &[1], &mut out).is_none());
    }

    #[test]
    fn quick_parse_reads_the_last_cluster() {
        let data = file(false);
        let m = parse(&data, false).unwrap();
        assert!(m.video.as_ref().unwrap().samples.is_empty());
        // Last block at 166 ms; the gap between frames is 33 ms.
        assert_eq!(m.duration_us, Some(199_000));
    }

    #[test]
    fn cut_short_keeps_what_is_there() {
        let data = file(false);
        for cut in (40..data.len()).step_by(7) {
            let _ = parse(&data[..cut], true);
            let _ = parse(&data[..cut], false);
        }
        let m = parse(&data[..data.len() - 2], true).unwrap();
        assert_eq!(m.video.unwrap().samples.len(), 6);
    }

    #[test]
    fn old_aac_ids_get_a_configuration() {
        let t = Entry {
            rate: 44100.0,
            channels: 2,
            ..Entry::default()
        };
        let c = specular_aac::parse_asc(&aac_config("A_AAC/MPEG4/LC/SBR", &t)).unwrap();
        assert_eq!(
            (c.sample_rate, c.output_rate(), c.core_channels()),
            (22050, 44100, 2)
        );
        assert_eq!(c.name(), "HE-AAC");
        let c = specular_aac::parse_asc(&aac_config("A_AAC/MPEG2/LC", &t)).unwrap();
        assert_eq!((c.sample_rate, c.name()), (44100, "AAC"));
        // AAC Main and LTP decode; SSR is named when refused
        let c = specular_aac::parse_asc(&aac_config("A_AAC/MPEG4/MAIN", &t)).unwrap();
        assert_eq!(c.name(), "AAC Main");
        assert!(specular_aac::parse_asc(&aac_config("A_AAC/MPEG4/SSR", &t)).is_err());
    }

    #[test]
    fn opus_frame_lengths() {
        assert_eq!(opus_samples(&[0x08]), Some(960)); // SILK NB 20 ms
        assert_eq!(opus_samples(&[0xFD]), Some(1920)); // CELT FB 20 ms, two frames
        assert_eq!(opus_samples(&[0x7B, 0x03]), Some(2880)); // hybrid FB 20 ms, three frames
    }
}
