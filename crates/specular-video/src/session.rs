//! Movies (MP4, QuickTime, M4V, 3GPP) opened as images: a [`specular_decode::Session`] whose
//! frames play in the animation player. [`install`] registers the opener with
//! `specular-decode`, which recognises the files but leaves them to this crate.
//!
//! - `decode` gives the first frame (converted to RGB on the CPU, for whatever reads pixels:
//!   export, copy, the histogram); playback hands out YCbCr frames the GPU converts.
//! - `decode_sampled` gives the frame about a tenth of the way in, for the thumbnail.
//! - The track's rotation is the session's orientation, so the viewer turns frames like any
//!   rotated photo.
//! - A movie that can't play (ProRes, MPEG-4 Part 2, VP9 in 4:4:4...) still opens: its
//!   facts show and decoding says why, in words.
//! - Matroska, WebM and MPEG transport streams (`crate::mkv`, `crate::ts`) open the same way.

use std::sync::Arc;
use std::time::Duration;

use specular_core::{CancelToken, Orientation, Raster, Source, yuv};
use specular_decode::{
    AnimationInfo, DecodeError, DecodeRequest, Format, Frames, ImageInfo, Result, Session,
    VideoFacts,
};

use crate::mp4::{self, Codec, Movie};
use crate::play::{self, Clip, PlayError};

/// Registers this crate as the opener of movies (once at start-up; also for thumbnailing and
/// indexing processes).
pub fn install() {
    specular_decode::set_video_opener(Some(Arc::new(open_any)));
}

/// Opens a movie, or a sound file (or a movie with sound and no picture) as its cover and
/// waveform ([`crate::sound_view::SoundSession`]).
pub fn open_any(source: Arc<Source>, format: Format) -> Result<Arc<dyn Session>> {
    let movie = parse(&source)?;
    if movie.video.is_none() && movie.audio.is_some() {
        let s = crate::sound_view::SoundSession::new(source, format, movie)?;
        return Ok(Arc::new(s));
    }
    open_movie(source, format, movie).map(|s| Arc::new(s) as Arc<dyn Session>)
}

fn parse(source: &Arc<Source>) -> Result<Movie> {
    crate::parse_source(source, true, None).map_err(|e| match e {
        mp4::Error::Unsupported(s) => DecodeError::Unsupported(s),
        e => DecodeError::Malformed(e.to_string()),
    })
}

/// A movie as an image.
pub struct VideoSession {
    source: Arc<Source>,
    movie: Movie,
    info: ImageInfo,
    anim: Option<AnimationInfo>,
    facts: VideoFacts,
}

fn play_err(e: PlayError) -> DecodeError {
    match e {
        PlayError::Unsupported(s) => DecodeError::Unsupported(s),
        PlayError::Malformed(s) => DecodeError::Malformed(s),
        PlayError::Io(s) => DecodeError::Malformed(s),
        PlayError::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// Parses `source` as a movie.
pub fn open(source: Arc<Source>, format: Format) -> Result<VideoSession> {
    let movie = parse(&source)?;
    open_movie(source, format, movie)
}

fn open_movie(source: Arc<Source>, format: Format, movie: Movie) -> Result<VideoSession> {
    let Some(v) = movie.video.as_ref() else {
        return Err(DecodeError::Unsupported(if movie.has_audio {
            "sound only, no picture".into()
        } else {
            "no video track".into()
        }));
    };
    let frames = play::plain_timeline_in(source.bytes(), v).len() as u32;
    let unplayable = unplayable(source.bytes(), v);
    let interlaced = play::track_interlaced(v);
    let mut facts = facts(&movie, frames, unplayable.clone(), interlaced);
    if facts.tags.is_none() {
        // AVI's RIFF INFO, an Ogg video's Vorbis comments, ID3 or APE tags: read as a sound
        // file's are.
        let t = crate::sound_tags::read(source.bytes());
        let items: Vec<(String, String)> = t
            .fields
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        if let Some(kind) = t.kinds.first().filter(|_| !items.is_empty()) {
            facts.tags = Some(specular_decode::text_tags::movie_tags(kind, &items));
        }
    }
    facts.profile = match v.codec {
        Some(Codec::Mpeg2) => crate::mpeg::profile(v),
        Some(Codec::Mpeg4 | Codec::H263 | Codec::Flv1) => crate::mpeg4::profile(v, source.bytes()),
        Some(Codec::ProRes) => crate::prores::profile(source.bytes(), v),
        Some(Codec::Dv) => crate::dvfile::profile(source.bytes(), v),
        Some(Codec::Vc1) => crate::mpeg4::vc1_profile(v),
        Some(Codec::Ffv1) => crate::ffv1::profile(source.bytes(), v),
        Some(Codec::Raw) => {
            crate::rawvid::PixDesc::from_config(&v.config).and_then(|d| crate::rawvid::describe(&d))
        }
        Some(Codec::Classic(c)) => crate::oldvid::profile(c, v),
        Some(Codec::CineForm) => crate::newvid::cineform_profile(source.bytes(), v),
        Some(Codec::Apv) => crate::apv::profile(source.bytes(), v),
        Some(Codec::Vc2) => crate::newvid::vc2_profile(source.bytes(), v),
        Some(Codec::Jpeg2000) => v
            .samples
            .first()
            .and_then(|s| v.sample_data(source.bytes(), s))
            .and_then(|b| crate::jp2v::profile(&b)),
        _ => None,
    };
    if matches!(v.codec, Some(Codec::Mpeg2 | Codec::Mpeg1))
        && let Some(r) = specular_mpeg2::probe(&v.config).and_then(|s| s.frame_rate())
    {
        // The sequence header's rate (the container's timing averages over the sound too).
        facts.frame_rate = Some(r);
    }
    let orientation = Orientation(v.orientation());
    let prores = (v.codec == Some(Codec::ProRes))
        .then(|| crate::prores::first_header(source.bytes(), v))
        .flatten();
    let bits = match (&prores, &facts.hdr) {
        (Some(h), _) => h.bit_depth(),
        // The stream's own depth: VP9 profile 2 and 10-bit HEVC/AV1/H.264 are often SDR.
        (None, hdr) => {
            stream_bit_depth(v, source.bytes()).unwrap_or(if hdr.is_some() { 10 } else { 8 })
        }
    };
    let has_alpha = prores
        .as_ref()
        .is_some_and(|h| h.alpha != specular_prores::Alpha::None)
        || other_alpha(v, source.bytes());
    let coding = [
        Some(facts.codec.clone()),
        facts
            .frame_rate
            .map(|r| format!("{r:.3} fps").replace(".000 ", " ")),
        facts.audio.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    // The first picture's own size where the stream says it: an IVF header (or a container)
    // may give another (vp90-2-05-resize.ivf: 320×240 for a 352×288 first frame), which
    // stretched the picture.
    let (width, height) = first_frame_size(v, source.bytes()).unwrap_or((v.width, v.height));
    let info = ImageInfo {
        format,
        width: width.max(1),
        height: height.max(1),
        orientation,
        icc_profile: None,
        has_alpha,
        bit_depth: bits,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1],
    };
    let anim = (unplayable.is_none() && frames > 0).then_some(AnimationInfo {
        frame_count: Some(frames),
        loop_count: 1,
    });
    Ok(VideoSession {
        source,
        movie,
        info,
        anim,
        facts,
    })
}

/// Why the picture can't play, in words.
fn unplayable(data: &[u8], v: &mp4::VideoTrack) -> Option<String> {
    match v.codec {
        Some(Codec::H264 | Codec::Hevc | Codec::Av1 | Codec::Vp8 | Codec::Mjpeg) => {}
        Some(Codec::Jpeg2000) => {}
        Some(Codec::Flv1) => {
            let first = v.samples.first()?;
            let bytes = v.sample_data(data, first)?;
            if let Some(why) = crate::mpeg4::sorenson_refusal(&bytes) {
                return Some(why);
            }
        }
        Some(Codec::Vp3) => {}
        Some(Codec::Vc1) => {
            if let Some(why) = crate::mpeg4::vc1_refusal(v, data) {
                return Some(why);
            }
        }
        Some(Codec::Dv) => {
            let first = v.samples.first()?;
            let bytes = v.sample_data(data, first)?;
            if let Some(why) = crate::dvfile::refusal(&bytes) {
                return Some(why);
            }
        }
        Some(Codec::Mpeg2 | Codec::Mpeg1) => {
            let first = v.samples.iter().find(|s| s.sync)?;
            let bytes = v.sample_data(data, first)?;
            if let Some(why) = crate::mpeg::refusal(&v.config, &bytes)
                .or_else(|| crate::mpeg::no_sequence_header(data, v))
            {
                return Some(why);
            }
        }
        Some(Codec::Mpeg4 | Codec::H263) => {
            let first = v.samples.iter().find(|s| s.sync)?;
            let bytes = v.sample_data(data, first)?;
            if let Some(why) = crate::mpeg4::refusal(&v.config, &bytes)
                .or_else(|| crate::mpeg4::no_vol_header(data, v))
            {
                return Some(why);
            }
        }
        Some(Codec::Vp9) => {
            let first = v.samples.iter().find(|s| s.sync)?;
            let bytes = v.sample_data(data, first)?;
            return crate::vpx::vp9_refusal(&bytes);
        }
        Some(Codec::Theora) => return crate::vpx::theora_refusal(&v.config),
        Some(Codec::JpegXs) => {
            let first = v.samples.first()?;
            let bytes = v.sample_data(data, first)?;
            return crate::vpx::jpegxs_refusal(&bytes);
        }
        Some(Codec::ProRes) => {
            if let Some(why) = crate::prores::refusal(data, v) {
                return Some(why);
            }
        }
        Some(Codec::Ffv1) => {
            if let Some(why) = crate::ffv1::refusal(data, v) {
                return Some(why);
            }
        }
        Some(Codec::Raw) => {
            let Some(d) = crate::rawvid::PixDesc::from_config(&v.config) else {
                return Some("uncompressed video without a description".into());
            };
            if let Some(why) = crate::rawvid::refusal(&d) {
                return Some(why);
            }
        }
        Some(Codec::Classic(c)) => {
            if let Some(why) = crate::oldvid::refusal(c, v) {
                return Some(why);
            }
        }
        Some(Codec::CineForm) => {
            if let Some(why) = crate::newvid::cineform_refusal(data, v) {
                return Some(why);
            }
        }
        Some(Codec::Apv) => {
            if let Some(why) = crate::apv::refusal(data, v) {
                return Some(why);
            }
        }
        Some(Codec::Hap(_)) => {
            if let Some(why) = crate::newvid::hap_refusal(data, v) {
                return Some(why);
            }
        }
        Some(Codec::Vc2) => {
            if let Some(why) = crate::newvid::vc2_refusal(data, v) {
                return Some(why);
            }
        }
        // Panasonic's AVC-Intra leaves its parameter sets out; the ones decoders assume are
        // not in any public specification.
        Some(Codec::Other(c)) if &c == b"AVCI" => {
            return Some(
                "AVC-Intra without its parameter sets (their defaults are not public)".into(),
            );
        }
        Some(c) => return Some(format!("{} video", c.name())),
        None => return Some("video without a coding description".into()),
    }
    if v.samples.is_empty() {
        return Some("a video track without frames".into());
    }
    let _ = data;
    None
}

pub(crate) fn facts(
    m: &Movie,
    frames: u32,
    unplayable: Option<String>,
    interlaced: bool,
) -> VideoFacts {
    let v = m.video.as_ref();
    let duration_us = m.duration_us();
    let frame_rate = duration_us
        .filter(|&d| d > 0 && frames > 1)
        .map(|d| f64::from(frames) * 1e6 / d as f64);
    let audio = m.audio.as_ref().map(|a| {
        // AAC says its variant, channels and decoded rate in its configuration
        let aac = match &a.codec {
            mp4::AudioCodec::Aac(asc) => crate::aac_sound::facts(asc),
            _ => None,
        };
        let (name, channels, rate) = match aac {
            Some((name, ch, rate)) if ch > 0 => (name.to_owned(), ch, rate),
            _ => {
                // MXF's mono tracks played together count as one track's channels.
                let more: usize = a
                    .extra
                    .companions
                    .iter()
                    .map(|c| usize::from(c.channels))
                    .sum();
                (
                    a.codec.name(),
                    usize::from(a.channels) + more,
                    a.sample_rate,
                )
            }
        };
        let ch = match channels {
            1 => "mono".to_owned(),
            2 => "stereo".to_owned(),
            6 => "5.1".to_owned(),
            8 => "7.1".to_owned(),
            n => format!("{n} ch"),
        };
        let khz = f64::from(rate) / 1000.0;
        format!("{name} {ch} {khz} kHz")
    });
    // The coding of sound this build can't play (said in words by the application).
    let silent_reason = m
        .audio
        .as_ref()
        .and_then(|a| crate::audio::playable(a).err())
        .and_then(|s| match s {
            crate::audio::Silence::Unsupported(codec) => Some(codec),
            _ => None,
        });
    let date = m.date.clone().or_else(|| {
        m.xmp
            .as_deref()
            .and_then(|x| xmp_date(&String::from_utf8_lossy(x)))
    });
    let recorded = date.as_deref().and_then(wall_clock).or(m.created);
    // The container's `colr`, else the stream's own signalling (ffmpeg writes no `colr`).
    let transfer = v.and_then(|v| v.colour.map(|c| c.transfer).or_else(|| stream_transfer(v)));
    let hdr = match transfer {
        Some(16) => Some("PQ"),
        Some(18) => Some("HLG"),
        _ => None,
    };
    VideoFacts {
        duration_us,
        codec: {
            let c = v
                .and_then(|v| v.codec)
                .map_or_else(|| "unknown".into(), Codec::name);
            // Raw elementary streams store no timing (see `crate::es`).
            match &m.brand {
                b"raw " => format!(
                    "{c}, raw stream without timing: shown at {} fps",
                    crate::es::DEFAULT_FPS
                ),
                b"rawd" => format!(
                    "{c}, raw stream without timing: shown at {} fps, in decoding order",
                    crate::es::DEFAULT_FPS
                ),
                b"rawt" => format!("{c}, raw stream"),
                _ => c,
            }
        },
        frame_rate,
        frames,
        audio,
        recorded,
        date,
        make: m.make.clone(),
        model: m.model.clone(),
        rotation: v.map_or(0, |v| v.rotation),
        pixel_aspect: v
            .and_then(|v| v.pixel_aspect)
            .filter(|&(h, w)| h > 0 && w > 0 && h != w),
        hdr,
        interlaced,
        profile: None,
        unplayable,
        silent_reason,
        sound: None,
        tags: (!m.tags.is_empty()).then(|| {
            let kind = match &m.brand {
                b"mkv " => "Matroska",
                b"ASF " => "Windows Media",
                b"WTV " => "Media Center",
                _ => "QuickTime",
            };
            specular_decode::text_tags::movie_tags(kind, &m.tags)
        }),
        xmp: m.xmp.clone(),
    }
}

/// The transfer characteristics the parameter sets of the track's configuration record
/// signal (H.264 and HEVC VUI).
/// The decoded size of the first VP8/VP9 keyframe (from its header, without decoding).
fn first_frame_size(v: &mp4::VideoTrack, file: &[u8]) -> Option<(u32, u32)> {
    let first = v.samples.first().and_then(|s| v.sample_data(file, s))?;
    match v.codec? {
        Codec::Vp9 => specular_vp9::probe(&first).map(|f| (f.width, f.height)),
        Codec::Vp8 => specular_vp8::probe(&first).map(|f| (f.width, f.height)),
        _ => None,
    }
    .filter(|&(w, h)| w > 0 && h > 0)
}

/// The luma bit depth the stream says: the `avcC` high-profile extension, `hvcC`'s
/// bitDepthLumaMinus8, `av1C`'s high_bitdepth and twelve_bit, the first VP9 keyframe's
/// header. `None` where nothing says it.
pub(crate) fn stream_bit_depth(v: &mp4::VideoTrack, file: &[u8]) -> Option<u8> {
    let c = &v.config;
    match v.codec? {
        Codec::H264 => {
            let profile = *c.get(1)?;
            let mut at = 6;
            for _ in 0..(*c.get(5)? & 0x1f) {
                at += 2 + usize::from(u16::from_be_bytes([*c.get(at)?, *c.get(at + 1)?]));
            }
            let pps = *c.get(at)?;
            at += 1;
            for _ in 0..pps {
                at += 2 + usize::from(u16::from_be_bytes([*c.get(at)?, *c.get(at + 1)?]));
            }
            if matches!(profile, 100 | 110 | 122 | 144 | 244) {
                Some(8 + (*c.get(at + 1)? & 7))
            } else {
                Some(8)
            }
        }
        Codec::Hevc => (c.len() > 18 && c[0] == 1).then(|| 8 + (c[17] & 7)),
        Codec::Av1 => {
            let b = *c.get(2)?;
            Some(match (b >> 6 & 1, b >> 5 & 1) {
                (1, 1) => 12,
                (1, 0) => 10,
                _ => 8,
            })
        }
        Codec::Vp9 => v
            .samples
            .iter()
            .take(4)
            .filter_map(|s| v.sample_data(file, s))
            .find_map(|b| specular_vp9::probe(&b))
            .map(|f| f.bit_depth),
        Codec::Ffv1 => crate::ffv1::params(file, v).ok().map(|p| p.bits as u8),
        Codec::CineForm => crate::newvid::cineform_info(file, v).map(|i| i.precision),
        Codec::Apv => crate::apv::bit_depth(file, v),
        Codec::Vc2 => crate::newvid::vc2_info(file, v).map(|i| i.bit_depth),
        Codec::Raw => {
            let d = crate::rawvid::PixDesc::from_config(c)?;
            Some(match &d.fourcc {
                b"v210" | b"v410" | b"r210" | b"R10k" | b"R10g" => 10,
                b"b48r" | b"b64a" => 16,
                _ => 8,
            })
        }
        _ => None,
    }
}

/// Whether a track other than ProRes carries an alpha channel: FFV1 with a transparency
/// plane, raw layouts with alpha, QuickTime Animation and raw QuickTime at 32 bits.
fn other_alpha(v: &mp4::VideoTrack, file: &[u8]) -> bool {
    match v.codec {
        Some(Codec::Ffv1) => crate::ffv1::params(file, v).is_ok_and(|p| p.extra_plane),
        Some(Codec::CineForm) => crate::newvid::cineform_info(file, v)
            .is_some_and(|i| i.layout == specular_cineform::Layout::Rgba4444),
        Some(Codec::Hap(k)) => crate::newvid::hap_kind(file, v, k).has_alpha(),
        Some(Codec::Apv) => crate::apv::has_alpha(file, v),
        Some(Codec::Raw | Codec::Classic(mp4::Classic::QtRle)) => {
            crate::rawvid::PixDesc::from_config(&v.config).is_some_and(|d| {
                matches!(
                    &d.fourcc,
                    b"RGBA" | b"BGRA" | b"ABGR" | b"ARGB" | b"b64a" | b"v408"
                ) || (&d.fourcc == b"raw " || &d.fourcc == b"rle ") && d.depth == 32
            })
        }
        _ => false,
    }
}

fn stream_transfer(v: &mp4::VideoTrack) -> Option<u16> {
    let colour = match v.codec? {
        Codec::H264 => {
            specular_h264::FieldScan::with_avcc(&v.config)
                .ok()?
                .signal()?
                .colour
        }
        Codec::Hevc => {
            let cfg = specular_heic::hevc::hvcc::parse(&v.config).ok()?;
            specular_heic::hevc::probe(&cfg.nal_units)
                .ok()?
                .signal?
                .colour
        }
        _ => None,
    };
    colour.map(|c| u16::from(c.1))
}

/// The first recording date an XMP packet gives (`xmp:CreateDate`, `exif:DateTimeOriginal`,
/// `photoshop:DateCreated`), as an attribute or an element.
fn xmp_date(x: &str) -> Option<String> {
    for key in [
        "exif:DateTimeOriginal",
        "xmp:CreateDate",
        "photoshop:DateCreated",
    ] {
        let Some(i) = x.find(key) else { continue };
        let rest = &x[i + key.len()..];
        let value = if let Some(r) = rest.strip_prefix("=\"") {
            r.split('"').next()
        } else if let Some(r) = rest.strip_prefix('>') {
            r.split('<').next()
        } else {
            None
        };
        if let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) {
            return Some(v.to_owned());
        }
    }
    None
}

/// `YYYY-MM-DDTHH:MM:SS[zone]` as written (the zone dropped: the time on the camera's clock)
/// → seconds since 1970.
pub fn wall_clock(s: &str) -> Option<i64> {
    let d: Vec<i64> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .take(6)
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    if d.len() < 3
        || !(1900..=9999).contains(&d[0])
        || !(1..=12).contains(&d[1])
        || !(1..=31).contains(&d[2])
        || d.get(3).is_some_and(|h| *h > 24)
        || d.get(4).is_some_and(|m| *m > 59)
        || d.get(5).is_some_and(|s| *s > 60)
    {
        return None;
    }
    let get = |i: usize| d.get(i).copied().unwrap_or(0);
    let (y, m) = (if d[1] <= 2 { d[0] - 1 } else { d[0] }, d[1]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d[2] - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(days * 86_400 + get(3) * 3600 + get(4) * 60 + get(5))
}

impl VideoSession {
    pub fn movie(&self) -> &Movie {
        &self.movie
    }

    /// The decoder of the picture, positioned at the start.
    pub fn video_frames(&self) -> Result<play::VideoFrames> {
        if let Some(why) = &self.facts.unplayable {
            return Err(DecodeError::Unsupported(why.clone()));
        }
        let clip =
            Clip::from_source(Arc::clone(&self.source), self.movie.clone()).map_err(play_err)?;
        clip.plain_frames().map(|(f, _)| f).map_err(play_err)
    }

    /// Frame `index` converted to RGB on the CPU.
    fn frame_rgb(&self, index: u32, cancel: &CancelToken) -> Result<Raster> {
        let mut frames = self.video_frames()?;
        if index > 0 {
            frames.seek(index)?;
        }
        let f = frames
            .next_frame(cancel)
            .ok_or_else(|| DecodeError::Malformed("no frames".into()))??;
        if !matches!(f.raster.layout, specular_core::PixelLayout::Yuv(_)) {
            // Already RGB (ProRes 4444 with alpha).
            return Ok(f.raster);
        }
        yuv::to_rgb(&f.raster).ok_or_else(|| DecodeError::Malformed("frame layout".into()))
    }
}

impl Session for VideoSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode_sampled(&self, _long_side: u32, cancel: &CancelToken) -> Option<Result<Raster>> {
        // About a tenth of the way in: past fades from black and title cards.
        let n = self.anim.as_ref()?.frame_count?;
        // Thumbnails at the shown shape: non-square pixels stretched (the viewer stretches
        // them on the GPU).
        let par = self.facts.pixel_aspect;
        Some(self.frame_rgb(n / 10, cancel).map(|r| match par {
            Some(p) => stretch(&r, p).unwrap_or(r),
            None => r,
        }))
    }
    fn decode(&self, _req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.frame_rgb(0, cancel)
    }
    fn decode_memory(&self, _req: &DecodeRequest) -> u64 {
        // A frame in YCbCr and in RGBA, plus the decoder's reference pictures.
        let px = u64::from(self.info.width) * u64::from(self.info.height);
        px * if self.facts.hdr.is_some() {
            8 + 3
        } else {
            4 + 2
        } + px * 12
    }
    fn animation(&self) -> Option<&AnimationInfo> {
        self.anim.as_ref()
    }
    fn frames(&self, _scale: u32) -> Result<Box<dyn Frames>> {
        Ok(Box::new(self.video_frames()?))
    }
    fn video(&self) -> Option<&VideoFacts> {
        Some(&self.facts)
    }
}

/// Opens a movie's sound on `out`, timed for a player of the movie's frames
/// ([`VideoSession`]'s): media time 0 is the first frame's presentation.
pub fn open_sound(
    source: Arc<Source>,
    out: Box<dyn crate::audio::AudioOutput>,
) -> std::result::Result<crate::audio::AudioPlayer, crate::audio::Silence> {
    open_sound_track(source, out, None)
}

/// [`open_sound`] of sound track `track` (an id from [`crate::extras::read`]) of a movie
/// with several.
pub fn open_sound_track(
    source: Arc<Source>,
    out: Box<dyn crate::audio::AudioOutput>,
    track: Option<u64>,
) -> std::result::Result<crate::audio::AudioPlayer, crate::audio::Silence> {
    let (movie, first) = sound_movie_track(&source, track)?;
    crate::audio::AudioPlayer::open(source, &movie, first, out)
}

/// [`open_sound`] with the sound set (levelling, equaliser and the rest) from the start.
pub fn open_sound_with(
    source: Arc<Source>,
    out: Box<dyn crate::audio::AudioOutput>,
    sound: &crate::dsp::SoundSettings,
) -> std::result::Result<crate::audio::AudioPlayer, crate::audio::Silence> {
    open_sound_track_with(source, out, None, sound)
}

/// [`open_sound_track`] with the sound set from the start: a track picked from the
/// sound-track menu is heard with the levelling, equaliser and the rest from its first
/// buffer.
pub fn open_sound_track_with(
    source: Arc<Source>,
    out: Box<dyn crate::audio::AudioOutput>,
    track: Option<u64>,
    sound: &crate::dsp::SoundSettings,
) -> std::result::Result<crate::audio::AudioPlayer, crate::audio::Silence> {
    let (movie, first) = sound_movie_track(&source, track)?;
    crate::audio::AudioPlayer::open_with(source, &movie, first, out, sound)
}

/// A file parsed for its sound (with any sound files beside it) and the time its first
/// video frame is presented (0 without video): what [`crate::audio::AudioPlayer`] plays.
pub fn sound_movie(
    source: &Arc<Source>,
) -> std::result::Result<(mp4::Movie, i64), crate::audio::Silence> {
    sound_movie_track(source, None)
}

fn sound_movie_track(
    source: &Arc<Source>,
    track: Option<u64>,
) -> std::result::Result<(mp4::Movie, i64), crate::audio::Silence> {
    let movie =
        crate::parse_source(source, true, track).map_err(|_| crate::audio::Silence::NoTrack)?;
    let first = movie.video.as_ref().map_or(0, |v| {
        let order = play::plain_timeline_in(source.bytes(), v);
        play::presented_at_us(v, order.first().copied().unwrap_or(0))
    });
    Ok((movie, first))
}

/// An OP-Atom MXF picture file without sound of its own: the clip's sound files beside it.
/// A WavPack hybrid file: its correction file (`.wvc`, same stem) when it is beside it.
pub(crate) fn sibling_sound(source: &Arc<Source>, movie: &mut mp4::Movie) {
    if movie.brand == *b"wvpk"
        && let Some(a) = movie.audio.as_mut()
        && matches!(a.codec, mp4::AudioCodec::WavPack { hybrid: true, .. })
    {
        a.extra.correction = wavpack_correction(source.path());
    }
    if movie.brand == *b"CDG " && movie.audio.is_none() {
        movie.audio = crate::cdg::sibling_sound(source);
        movie.has_audio = movie.audio.is_some();
    }
    if movie.brand == *b"mxf " && movie.audio.is_none() {
        movie.audio = crate::mxf::sibling_sound(source);
        movie.has_audio = movie.audio.is_some();
    }
}

/// The `.wvc` file beside a `.wv` (either case), opened.
pub fn wavpack_correction(path: &std::path::Path) -> Option<mp4::External> {
    ["wvc", "WVC"].iter().find_map(|ext| {
        let p = path.with_extension(ext);
        let src = Source::open(&p).ok()?;
        specular_wavpack::Header::parse(src.bytes())?;
        Some(mp4::External(Arc::new(src)))
    })
}

/// An RGBA8 or 16-bit RGBA frame with pixels `hs`:`vs` (horizontal : vertical spacing)
/// resampled to square pixels: widened when they are wide, made taller when they are tall
/// (no detail is dropped), linearly. `None` for other layouts.
fn stretch(r: &Raster, (hs, vs): (u32, u32)) -> Option<Raster> {
    use specular_core::{PixelLayout, RasterPlane};
    let bpp = match r.layout {
        PixelLayout::Rgba => 4,
        PixelLayout::Rgba16(_) => 8,
        _ => return None,
    };
    let p = r.planes.first()?;
    let (w, h) = (r.width as usize, r.height as usize);
    let wide = hs > vs;
    let (nw, nh) = if wide {
        ((w as u64 * u64::from(hs) / u64::from(vs)) as usize, h)
    } else {
        (w, (h as u64 * u64::from(vs) / u64::from(hs)) as usize)
    };
    if (nw, nh) == (w, h) || nw == 0 || nh == 0 || nw > 1 << 16 || nh > 1 << 16 {
        return None;
    }
    let get = |x: usize, y: usize, c: usize| -> f32 {
        let o = y * p.stride + x * bpp;
        if bpp == 4 {
            f32::from(p.data[o + c])
        } else {
            f32::from(u16::from_ne_bytes([
                p.data[o + 2 * c],
                p.data[o + 2 * c + 1],
            ]))
        }
    };
    let stride = nw * bpp;
    let mut data = vec![0u8; stride * nh];
    let (from, to) = if wide { (w, nw) } else { (h, nh) };
    let scale = from as f32 / to as f32;
    for y in 0..nh {
        for x in 0..nw {
            let i = if wide { x } else { y };
            let f = ((i as f32 + 0.5) * scale - 0.5).clamp(0.0, (from - 1) as f32);
            let (i0, t) = (f.floor() as usize, f.fract());
            let i1 = (i0 + 1).min(from - 1);
            let at = |k: usize, c: usize| if wide { get(k, y, c) } else { get(x, k, c) };
            let o = y * stride + x * bpp;
            for c in 0..4 {
                let v = at(i0, c) + (at(i1, c) - at(i0, c)) * t;
                if bpp == 4 {
                    data[o + c] = v.round() as u8;
                } else {
                    data[o + 2 * c..o + 2 * c + 2]
                        .copy_from_slice(&(v.round() as u16).to_ne_bytes());
                }
            }
        }
    }
    Some(Raster {
        width: nw as u32,
        height: nh as u32,
        planes: vec![RasterPlane {
            width: nw as u32,
            height: nh as u32,
            stride,
            data,
            h_div: 1,
            v_div: 1,
        }],
        origin: r.origin,
        scale: r.scale,
        layout: r.layout,
        truncated: r.truncated,
    })
}

/// Frame `index`'s time from the first frame (for seek bars before a player exists).
pub fn frame_time(frames: &play::VideoFrames, index: u32) -> Duration {
    frames.frame_delays().iter().take(index as usize).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_as_written() {
        assert_eq!(wall_clock("1970-01-02T00:00:01+0200"), Some(86_401));
        assert_eq!(wall_clock("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(wall_clock("2024:13:01"), None);
        assert_eq!(
            xmp_date(r#"<x xmp:CreateDate="2021-06-01T10:00:00"/>"#).as_deref(),
            Some("2021-06-01T10:00:00")
        );
        assert_eq!(
            xmp_date("<photoshop:DateCreated>2020-01-01</photoshop:DateCreated>").as_deref(),
            Some("2020-01-01")
        );
    }
}
