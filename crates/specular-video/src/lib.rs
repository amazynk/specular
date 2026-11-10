//! Motion Photos and Live Photos (DESIGN §6 "Motion and video", §12.3).
//!
//! - [`motion`] finds the clip: Google / Android Motion Photos (XMP `GCamera:MotionPhoto`,
//!   `MicroVideo*`, `Container:Directory`), the Samsung `MotionPhoto_Data` trailer, the HEIF
//!   `mpvd` box, and Apple Live Photos (a sibling `.MOV` of the same stem, confirmed by the
//!   content identifier when both sides carry one).
//! - [`mp4`] is the in-tree ISO-BMFF / QuickTime demuxer (`moov`/`trak`/`stsd`/`stsz`/`stco`/
//!   `co64`/`stts`/`ctts`/`stss`, `avcC`/`hvcC`); [`mp4write`] writes faststart MP4s with one
//!   H.264 track (the slideshow export), whose pictures [`encode`] codes on the GPU's
//!   hardware encoder or on the CPU.
//! - [`play`] decodes the clip on the GPU's video engine through `specular-hwdec` (VA-API on
//!   Linux, D3D12 Video on Windows; H.264 and HEVC) and hands RGBA frames to the viewer's
//!   animation player. H.264 falls back
//!   to the in-tree software decoder ([`sw_h264`]); without hardware decode other clips name
//!   that instead.
//!
//! The still image always shows first; nothing here runs on the open path except
//! [`probe_embedded`], which only looks at bytes the metadata read already has in memory.
//!
//! Movies of their own (MP4, MOV, M4V, 3GP, 3G2) open as images through [`session`]
//! ([`install`] registers it with `specular-decode`): their frames play in the animation
//! player as YCbCr the GPU converts, and their sound plays through [`audio`], which is the
//! master clock.
//!
//! Video codecs: H.264 and HEVC (hardware, else in-tree software), AV1, VP8, VP9, Theora and
//! VP3, MPEG-1, MPEG-2, MPEG-4 Part 2, H.263 and Sorenson Spark, VC-1, ProRes, DV, Motion
//! JPEG, JPEG 2000, JPEG XS, FFV1 ([`ffv1`]), uncompressed video ([`rawvid`]), and the old
//! codecs QuickTime Animation, Cinepak, Microsoft Video 1, Microsoft RLE, Apple SMC and
//! Apple Video, Flash Screen Video 1 and 2 ([`oldvid`]), CineForm, HAP and VC-2 / Dirac
//! ([`newvid`]). Refused by name: DNxHD/DNxHR, VVC, Dolby Vision, ProRes RAW, WMV 7/8,
//! Microsoft MPEG-4, RealVideo, VP6, Indeo, AVC-Intra without parameter sets and the rest the
//! Info panel names.

mod aac_sound;
mod ac3_sound;
mod alac_sound;
mod amr_sound;
pub mod apv;
pub mod asf;
pub mod audio;
pub mod avi;
pub mod captions;
mod cdg;
pub mod deint;
pub mod dialogue;
mod dsd_files;
mod dshow;
pub mod dsp;
mod dts_sound;
pub mod dvfile;
pub mod encode;
pub mod eq;
pub mod es;
pub mod extras;
pub mod ffv1;
pub mod flv;
mod hevc_fields;
pub mod ivf;
pub mod jp2v;
pub mod levelling;
pub mod live_tap;
mod lossless_sound;
pub mod loudness;
pub mod lyrics;
pub mod mjpeg;
pub mod mkv;
pub mod motion;
pub mod mp4;
pub mod mp4write;
mod mpc_sound;
pub mod mpeg;
pub mod mpeg4;
pub mod mxf;
mod nelly_data;
mod nelly_sound;
pub mod newvid;
pub mod ogg;
pub mod oldvid;
mod opus_sound;
mod pcm_files;
pub mod play;
pub mod prores;
pub mod ps;
mod psxstr;
mod qcelp_sound;
pub mod qoa_sound;
pub mod rawvid;
mod seek_cache;
pub mod session;
mod shn_sound;
pub mod sound_feed;
pub mod sound_file;
pub mod sound_page;
pub mod sound_tags;
pub mod sound_view;
pub mod speakers;
mod speech_files;
pub mod speech_sound;
mod speex_sound;
pub mod stereo;
mod stretch;
pub mod sw_h264;
pub mod synth_sound;
mod tracker_sound;
pub mod ts;
mod tta_sound;
pub mod vpx;
mod wave_sound;
pub mod wtv;
mod xiph_sound;
pub mod y4m;

/// Parses a movie of any container this crate reads: Matroska / WebM (EBML header), an MPEG
/// transport stream (sync bytes every 188 or 192 bytes), an MPEG programme stream or bare
/// MPEG-1/2 video, AVI, Ogg, IVF, else ISO-BMFF / QuickTime.
pub fn parse_movie(data: &[u8], samples: bool) -> mp4::Result<mp4::Movie> {
    parse_movie_with(data, samples, None)
}

/// [`parse_movie`] taking the sound track `audio` (an id from [`extras::read`]) of a movie
/// with several, where the container allows the choice (Matroska, MP4/MOV).
pub fn parse_movie_with(data: &[u8], samples: bool, audio: Option<u64>) -> mp4::Result<mp4::Movie> {
    let mut movie = if mkv::sniff(data) {
        mkv::parse_with(data, samples, audio)
    } else if ts::sniff(data) {
        ts::parse_with(data, samples, audio)
    } else if ps::sniff(data) {
        ps::parse(data, samples)
    } else if avi::sniff(data) {
        avi::parse(data, samples)
    } else if ogg::sniff(data) {
        ogg::parse(data, samples)
    } else if ivf::sniff(data) {
        ivf::parse(data, samples)
    } else if mxf::sniff(data) {
        mxf::parse(data, samples)
    } else if dvfile::sniff(data) {
        dvfile::parse(data, samples)
    } else if wtv::sniff(data) {
        wtv::parse_with(data, samples, audio)
    } else if asf::sniff(data) {
        asf::parse(data, samples)
    } else if flv::sniff(data) {
        flv::parse(data, samples)
    } else if y4m::sniff(data) {
        y4m::parse(data, samples)
    } else if sound_file::sniff(data).is_some() {
        sound_file::parse(data, samples)
    } else if psxstr::sniff(data) {
        psxstr::parse(data, samples)
    } else if cdg::sniff(data) {
        cdg::parse(data, samples)
    } else if apv::sniff(data) {
        apv::parse(data, samples)
    } else if es::sniff(data).is_some() {
        es::parse(data, samples)
    } else {
        mp4::parse_with(data, samples, audio.and_then(|a| u32::try_from(a).ok()))
    }?;
    if let Some(v) = movie.video.as_mut()
        && matches!(v.codec, Some(mp4::Codec::Mpeg2 | mp4::Codec::Mpeg1))
    {
        mpeg::complete_track(data, v);
    }
    if let Some(v) = movie.video.as_mut() {
        mpeg4::complete_track(data, v);
    }
    if let Some(v) = movie.video.as_mut()
        && v.codec == Some(mp4::Codec::ProRes)
        && !v.interlaced
    {
        // Not every container says; the frame header does.
        v.interlaced = prores::interlaced(data, v);
    }
    if let Some(v) = movie.video.as_mut()
        && v.codec == Some(mp4::Codec::Dv)
        && movie.brand != *b"DV  "
    {
        dvfile::complete_track(data, v);
        if movie.date.is_none() {
            movie.date = dvfile::recorded(data, v);
        }
        if movie.audio.is_none() && movie.brand == *b"AVI " {
            // A type-1 AVI: the sound is in the frames.
            let first = v.samples.first().and_then(|s| v.sample_data(data, s));
            if let Some(a) = first.and_then(|f| specular_dv::audio(&f)) {
                movie.audio = Some(mp4::AudioTrack {
                    codec: mp4::AudioCodec::Dv,
                    channels: a.channels,
                    sample_rate: a.rate,
                    timescale: v.timescale,
                    duration: v.duration,
                    sample_count: v.sample_count,
                    samples: v.samples.clone(),
                    edit: v.edit,
                    layout: v.layout.clone(),
                    extra: Default::default(),
                });
                movie.has_audio = true;
            }
        }
    }
    if let Some(v) = movie.video.as_mut()
        && matches!(v.codec, Some(mp4::Codec::Mjpeg | mp4::Codec::Jpeg2000))
        && !v.interlaced
    {
        // Field pairs in one sample: the samples say, not every container.
        let first = v.samples.first().and_then(|s| v.sample_data(data, s));
        v.interlaced = first.is_some_and(|b| match v.codec {
            Some(mp4::Codec::Mjpeg) => mjpeg::two_fields(&b),
            _ => jp2v::two_fields(&b),
        });
        // AVI has no field order: capture cards there write the bottom field first.
        if v.interlaced && movie.brand == *b"AVI " && v.codec == Some(mp4::Codec::Mjpeg) {
            v.bottom_first = true;
        }
    }
    if let Some(a) = movie.audio.as_mut() {
        audio::complete_sound(data, a);
    }
    Ok(movie)
}

pub use session::install;

/// [`parse_movie_with`] for a file with its name: formats chosen by extension
/// (signed-linear `.sln`) or that read files beside them (SFZ instruments) are parsed here,
/// and what a track plays from is attached (a WavPack correction file, an OP-Atom MXF's
/// sound files, a MIDI song's SoundFont, a bank's own bytes).
pub fn parse_source(
    source: &std::sync::Arc<specular_core::Source>,
    samples: bool,
    audio: Option<u64>,
) -> mp4::Result<mp4::Movie> {
    let data = source.bytes();
    let ext = source
        .path()
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let mut track = None;
    if ext == "sfz" && specular_synth::sfz::is_sfz(data) {
        track = Some(synth_sound::sfz_track(source)?);
    } else if let Some(rate) = speech_files::sln_rate(&ext).filter(|_| ext != "raw")
        && sound_file::sniff(data).is_none()
    {
        track = Some(speech_files::sln(data, rate)?);
    }
    let mut movie = match track {
        Some(mut t) => {
            t.sample_count = t.samples.len() as u32;
            if t.duration == 0 {
                t.duration = t
                    .samples
                    .last()
                    .map_or(0, |s| s.dts + u64::from(s.duration));
            }
            let duration_us = (t.timescale > 0).then(|| t.us(t.duration as i64).max(0) as u64);
            if !samples {
                t.samples = Vec::new();
            }
            mp4::Movie {
                brand: if ext == "sfz" { *b"sfz " } else { *b"sln " },
                timescale: t.timescale,
                duration_us,
                has_audio: true,
                audio: Some(t),
                ..mp4::Movie::default()
            }
        }
        None => parse_movie_with(data, samples, audio)?,
    };
    session::sibling_sound(source, &mut movie);
    if let Some(a) = movie.audio.as_mut() {
        synth_sound::attach(source, a);
    }
    Ok(movie)
}

use std::ops::Range;
use std::path::{Path, PathBuf};

pub use motion::Kind;

/// Where the clip lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Bytes of the still's own file.
    Embedded(Range<usize>),
    /// A paired movie file (Live Photo).
    Sibling(PathBuf),
}

/// What the viewer knows about a still's motion clip.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionInfo {
    pub kind: Kind,
    pub source: Source,
    /// Clip length, when the movie header says.
    pub duration_us: Option<u64>,
    /// The video codec ("H.264", "HEVC", or the sample-entry four-character code).
    pub codec: Option<String>,
    /// Displayed size (after the track's rotation).
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    /// The timestamp of the frame the still was taken from (Google `MotionPhotoPresentation
    /// TimestampUs`), when given.
    pub presentation_us: Option<i64>,
    /// Live Photo: whether both files carried the same content identifier (`Some(true)`),
    /// or neither / only one carried one (`None`). Mismatches are not Live Photos at all.
    pub paired: Option<bool>,
}

impl MotionInfo {
    fn from_movie(kind: Kind, source: Source, m: &mp4::Movie) -> MotionInfo {
        let v = m.video.as_ref();
        let (mut width, mut height) = v.map_or((0, 0), |v| {
            if v.display.0 > 0 && v.display.1 > 0 {
                v.display
            } else {
                (v.width, v.height)
            }
        });
        if v.is_some_and(|v| v.rotation % 180 == 90) {
            std::mem::swap(&mut width, &mut height);
        }
        MotionInfo {
            kind,
            source,
            duration_us: m.duration_us(),
            codec: v.and_then(|v| v.codec).map(mp4::Codec::name),
            width,
            height,
            frames: v.map_or(0, |v| v.sample_count),
            presentation_us: None,
            paired: None,
        }
    }
}

/// The embedded clip of a still: no I/O, only `file` (the whole still, already mapped) and its
/// XMP packet. Reads the clip's movie header, not its sample table.
pub fn probe_embedded(file: &[u8], xmp: Option<&str>) -> Option<MotionInfo> {
    let e = motion::detect_embedded(file, xmp)?;
    let clip = file.get(e.range.clone())?;
    let movie = mp4::parse(clip, false).ok()?;
    movie.video.as_ref()?;
    let mut info = MotionInfo::from_movie(e.kind, Source::Embedded(e.range), &movie);
    info.presentation_us = e.presentation_us;
    Some(info)
}

/// Largest `moov` read to describe a Live Photo's movie (a 3 s clip's is ~20 KB).
const MAX_MOOV: u64 = 16 << 20;

/// A Live Photo: the still's sibling movie, if one exists and its content identifier does not
/// contradict the still's (Apple MakerNote tag 0x0011 in `exif_tiff`). Costs a `stat` per
/// candidate name when there is no movie, and a header read (the `moov` box) when there is.
pub fn probe_live(still: &Path, exif_tiff: Option<&[u8]>) -> Option<MotionInfo> {
    let mov = motion::live_photo_sibling(still)?;
    let moov = read_moov(&mov)?;
    let movie = mp4::parse(&moov, false).ok()?;
    movie.video.as_ref()?;
    let still_id = exif_tiff.and_then(motion::apple_content_id);
    let paired = match (still_id, &movie.content_id) {
        (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => Some(true),
        (Some(_), Some(_)) => return None,
        _ => None,
    };
    let mut info = MotionInfo::from_movie(Kind::LivePhoto, Source::Sibling(mov), &movie);
    info.paired = paired;
    Some(info)
}

/// The `moov` box of a movie file, found by walking the top-level box headers (seeks, no
/// read of the media data).
pub fn read_moov(path: &Path) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut pos = 0u64;
    while pos + 8 <= len {
        f.seek(SeekFrom::Start(pos)).ok()?;
        let mut h = [0u8; 16];
        f.read_exact(&mut h[..8]).ok()?;
        let mut size = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
        let mut header = 8;
        if size == 1 {
            f.read_exact(&mut h[8..16]).ok()?;
            size = u64::from_be_bytes([h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]]);
            header = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < header || pos.checked_add(size)? > len {
            return None;
        }
        if &h[4..8] == b"moov" {
            if size > MAX_MOOV {
                return None;
            }
            let mut out = vec![0u8; size as usize];
            f.seek(SeekFrom::Start(pos)).ok()?;
            f.read_exact(&mut out).ok()?;
            if header == 16 {
                // Rewrite as a 32-bit box header so the demuxer sees a plain box.
                let body = out.split_off(16);
                out = Vec::with_capacity(body.len() + 8);
                out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
                out.extend_from_slice(b"moov");
                out.extend_from_slice(&body);
            }
            return Some(out);
        }
        pos += size;
    }
    None
}

/// The kind of moving photo `file` (the whole still, at `path`) is, for a thumbnail's badge:
/// an embedded clip (found from `xmp`, or for a JPEG from its own XMP packet, or a trailer or
/// `mpvd` box), else a Live Photo sibling (a `stat` per candidate name). Cheaper than
/// [`probe_embedded`] / [`probe_live`]: no movie header is read and a Live Photo's content
/// identifier is not compared.
pub fn quick_kind(file: &[u8], path: &Path, xmp: Option<&[u8]>) -> Option<Kind> {
    if !may_have_motion(path) {
        return None;
    }
    let xmp = xmp
        .or_else(|| jpeg_xmp(file))
        .and_then(|x| std::str::from_utf8(x).ok());
    if let Some(e) = motion::detect_embedded(file, xmp) {
        return Some(e.kind);
    }
    motion::live_photo_sibling(path).map(|_| Kind::LivePhoto)
}

/// The standard XMP packet of a JPEG (APP1 "http://ns.adobe.com/xap/1.0/"), found by walking
/// the marker segments up to the scan.
pub fn jpeg_xmp(file: &[u8]) -> Option<&[u8]> {
    const SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    if !file.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    loop {
        let m = file.get(at..at + 4)?;
        if m[0] != 0xFF {
            return None;
        }
        match m[1] {
            // Fill bytes.
            0xFF => {
                at += 1;
                continue;
            }
            // Start of scan, end of image: no XMP before the data.
            0xDA | 0xD9 => return None,
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => {
                at += 2;
                continue;
            }
            _ => {}
        }
        let len = u16::from_be_bytes([m[2], m[3]]) as usize;
        if len < 2 {
            return None;
        }
        let body = file.get(at + 4..at + 2 + len)?;
        if m[1] == 0xE1
            && let Some(x) = body.strip_prefix(SIG)
        {
            return Some(x);
        }
        at += 2 + len;
    }
}

/// Whether this is a still format that can carry a motion clip (cheap pre-check by name).
pub fn may_have_motion(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["jpg", "jpeg", "heic", "heif", "hif"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::tests::container_xmp;
    use crate::mp4::build::{Spec, movie};

    /// A JPEG with an Exif APP1 (after a fill byte), the XMP APP1 and a scan, then `trailer`.
    fn jpeg_with_xmp(xmp: &str, trailer: &[u8]) -> Vec<u8> {
        let mut j = vec![0xff, 0xd8, 0xff, 0xff, 0xe1, 0, 8];
        j.extend(b"Exif\0\0");
        let sig = b"http://ns.adobe.com/xap/1.0/\0";
        j.extend([0xff, 0xe1]);
        j.extend(((2 + sig.len() + xmp.len()) as u16).to_be_bytes());
        j.extend(sig);
        j.extend(xmp.as_bytes());
        j.extend([0xff, 0xda, 0, 2, 0x55, 0x55, 0xff, 0xd9]);
        j.extend(trailer);
        j
    }

    #[test]
    fn jpeg_xmp_walks_to_the_packet() {
        let j = jpeg_with_xmp("<x:xmpmeta/>", &[]);
        assert_eq!(jpeg_xmp(&j), Some(&b"<x:xmpmeta/>"[..]));
        // None after the scan, none in a truncated segment, none without SOI.
        let mut late = vec![0xff, 0xd8, 0xff, 0xda, 0, 2];
        late.extend(&j[2..]);
        assert_eq!(jpeg_xmp(&late), None);
        assert_eq!(jpeg_xmp(&j[..20]), None);
        assert_eq!(jpeg_xmp(&j[2..]), None);
    }

    #[test]
    fn quick_kind_finds_a_motion_photo_from_its_own_xmp() {
        let mp4 = movie(&Spec::default());
        let j = jpeg_with_xmp(&container_xmp(mp4.len(), 0, None), &mp4);
        let p = Path::new("/nonexistent/PXL_1.MP.jpg");
        assert_eq!(quick_kind(&j, p, None), Some(Kind::MotionPhoto));
        // The same bytes under a name that cannot carry motion; a plain JPEG.
        assert_eq!(quick_kind(&j, Path::new("/nonexistent/a.png"), None), None);
        let plain = jpeg_with_xmp("<x:xmpmeta/>", &[]);
        assert_eq!(quick_kind(&plain, p, None), None);
    }
}
