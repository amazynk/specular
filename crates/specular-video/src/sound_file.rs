//! Sound files without a picture, read as a movie with only a sound track: bare MPEG audio
//! (`.mp3`, `.mp2`) with its ID3 tags and Xing/LAME frame, ADTS AAC (`.aac`), native FLAC
//! (`.flac`), WAVE and RF64 (`.wav`), AIFF and AIFF-C (`.aiff`, `.aif`, `.aifc`), raw DTS
//! (`.dts`), raw AC-3 / E-AC-3 (`.ac3`, `.eac3`) and AMR / AMR-WB storage files (`.amr`,
//! `.awb`, also multichannel), QCP speech (`.qcp`), WavPack (`.wv`, with its `.wvc` correction file beside it)
//! and Monkey's Audio (`.ape`). M4A, Ogg and Matroska sound are their
//! containers' (`crate::mp4`, `crate::ogg`, `crate::mkv`).
//!
//! The track's samples are the stream's frames (PCM: runs of 4096 frames), timed at the
//! sampling rate, so the player seeks to the sample. A LAME frame's encoder delay becomes the
//! track's edit, as gapless players trim it.

use crate::mp4::{AdpcmKind, AudioCodec, AudioTrack, Edit, Error, Layout, Movie, Result, Sample};

/// A sound file's kind, by its first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mpa,
    Adts,
    Flac,
    Wave,
    Aiff,
    Dts,
    Ac3,
    Amr,
    WavPack,
    Ape,
    /// Musepack (`.mpc`), stream versions 7 and 8.
    Mpc,
    /// AU, CAF, Wave64, VOC, 8SVX, AVR, SPHERE, IRCAM ([`crate::pcm_files`]).
    More(crate::pcm_files::Kind),
    /// TTA (True Audio).
    Tta,
    /// Shorten.
    Shorten,
    /// DSD: Sony DSF and Philips DSDIFF.
    Dsf,
    Dff,
    /// Tracker modules (MOD, S3M, XM, IT), rendered by `specular-tracker`.
    Tracker,
    /// Raw GSM 06.10 frames (`.gsm`).
    Gsm,
    /// QCP speech (`.qcp`, RFC 3625): QCELP-13 or EVRC.
    Qcp,
    /// QOA, "Quite OK Audio" (`.qoa`).
    Qoa,
    /// HTK speech files (`.htk`).
    Htk,
    /// MIDI songs (`.mid`, `.rmi`), played by `specular-synth`.
    Midi,
    /// SoundFont 2, DLS and XI instruments: their samples played.
    Bank,
}

impl Kind {
    /// The movie's brand for this kind (what [`Movie::brand`] says).
    pub fn brand(self) -> [u8; 4] {
        match self {
            Kind::Mpa => *b"mpga",
            Kind::Adts => *b"adts",
            Kind::Flac => *b"fLaC",
            Kind::Wave => *b"WAVE",
            Kind::Aiff => *b"AIFF",
            Kind::Dts => *b"dts ",
            Kind::Ac3 => *b"ac-3",
            Kind::Amr => *b"amr ",
            Kind::WavPack => *b"wvpk",
            Kind::Ape => *b"MAC ",
            Kind::Mpc => *b"MPC ",
            Kind::More(k) => k.brand(),
            Kind::Tta => *b"TTA1",
            Kind::Shorten => *b"ajkg",
            Kind::Dsf => *b"DSF ",
            Kind::Dff => *b"DFF ",
            Kind::Tracker => *b"trkr",
            Kind::Gsm => *b"gsm ",
            Kind::Qcp => *b"QLCM",
            Kind::Qoa => *b"qoaf",
            Kind::Htk => *b"HTK ",
            Kind::Midi => *b"MThd",
            Kind::Bank => *b"bank",
        }
    }
}

/// PCM frames in one sample of the track.
pub(crate) const PCM_FRAMES: u64 = 4096;
/// The most frames a file's track lists (about 13 hours of MP3 at 48 kHz).
pub(crate) const MAX_SAMPLES: usize = 1 << 20;

fn u16be(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn u32be(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn u16le(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}
fn u32le(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}
fn u64le(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

fn bad(m: &str) -> Error {
    Error::Malformed(m.into())
}

/// The bytes after the ID3v2 tags at the start (several may follow each other).
pub(crate) fn skip_id3v2(d: &[u8]) -> usize {
    let mut at = 0;
    while let Some(h) = d.get(at..at + 10) {
        if &h[..3] != b"ID3" || h[3] == 0xFF || h[6..10].iter().any(|b| b & 0x80 != 0) {
            break;
        }
        let size = h[6..10]
            .iter()
            .fold(0usize, |a, &b| (a << 7) | usize::from(b));
        let footer = if h[5] & 0x10 != 0 { 10 } else { 0 };
        at += 10 + size + footer;
    }
    at.min(d.len())
}

/// Where the stream ends: before an ID3v1 tag, an APEv2 tag and Lyrics3 at the end.
fn stream_end(d: &[u8], start: usize) -> usize {
    let mut end = d.len();
    if end >= start + 128 && &d[end - 128..end - 125] == b"TAG" {
        end -= 128;
    }
    if end >= start + 32 && &d[end - 32..end - 24] == b"APETAGEX" {
        let size = u32le(d, end - 20).unwrap_or(0) as usize;
        let header = u32le(d, end - 12).is_some_and(|f| f & 0x8000_0000 != 0);
        let total = size + if header { 32 } else { 0 };
        if total <= end - start {
            end -= total;
        }
    }
    end
}

/// Two MPEG audio frames in a row from `at`, of one stream (a free-format one with its frame
/// length).
fn mpa_at(d: &[u8], at: usize) -> Option<specular_mpa::Header> {
    let mut h = specular_mpa::Header::parse(d.get(at..)?)?;
    if h.bitrate == 0 {
        h.free_len = specular_mpa::free_len(&d[at..], &h)?;
    }
    let len = h.frame_len()?;
    let n = specular_mpa::Header::parse(d.get(at + len..)?)?;
    (n.version == h.version && n.layer == h.layer && n.sample_rate == h.sample_rate).then_some(h)
}

/// An ADTS frame at `at` followed by another (or the end).
fn adts_at(d: &[u8], at: usize) -> Option<specular_aac::adts::Header> {
    let h = specular_aac::adts::parse(d.get(at..)?).ok()?;
    if h.frame_len < 7 {
        return None;
    }
    let next = at + h.frame_len;
    if next + 7 > d.len() {
        return (next <= d.len()).then_some(h);
    }
    let n = specular_aac::adts::parse(&d[next..]).ok()?;
    (n.rate_index == h.rate_index && n.object_type == h.object_type).then_some(h)
}

/// The 32-bit sync words of a DTS core frame (ETSI TS 102 114 §5.3.1): 16-bit big- and
/// little-endian words, 14-bit words of either order.
fn dts_sync(b: &[u8]) -> bool {
    matches!(
        b,
        [0x7F, 0xFE, 0x80, 0x01, ..]
            | [0xFE, 0x7F, 0x01, 0x80, ..]
            | [0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0..=0xFF, ..]
            | [0xFF, 0x1F, 0x00, 0xE8, 0xF0..=0xFF, 0x07, ..]
    )
}

/// The kind of sound file `d` is, if it is one.
pub fn sniff(d: &[u8]) -> Option<Kind> {
    if crate::speech_files::is_qcp(d) {
        return Some(Kind::Qcp);
    }
    if d.len() >= 12 && matches!(&d[..4], b"RIFF" | b"RF64" | b"BW64") && &d[8..12] == b"WAVE" {
        return Some(Kind::Wave);
    }
    if d.len() >= 12 && &d[..4] == b"FORM" && matches!(&d[8..12], b"AIFF" | b"AIFC") {
        return Some(Kind::Aiff);
    }
    if specular_amr::storage::is_amr_file(d) {
        return Some(Kind::Amr);
    }
    if let Some(k) = crate::pcm_files::sniff(d) {
        return Some(Kind::More(k));
    }
    if specular_shorten::is_shorten(d) {
        return Some(Kind::Shorten);
    }
    if crate::dsd_files::is_dsf(d) {
        return Some(Kind::Dsf);
    }
    if crate::dsd_files::is_dff(d) {
        return Some(Kind::Dff);
    }
    // By content only (`.mod` is also a camcorder's MPEG programme stream).
    if specular_tracker::sniff(d).is_some() {
        return Some(Kind::Tracker);
    }
    if specular_synth::smf::is_midi(d) {
        return Some(Kind::Midi);
    }
    if specular_synth::sf2::is_sf2(d)
        || specular_synth::dls::is_dls(d)
        || specular_synth::xi::is_xi(d)
    {
        return Some(Kind::Bank);
    }
    if specular_decode::sound_ident::is_htk(d) {
        return Some(Kind::Htk);
    }
    let start = skip_id3v2(d);
    let s = &d[start..];
    if s.starts_with(b"TTA1") {
        return Some(Kind::Tta);
    }
    if crate::qoa_sound::sniff(s) {
        return Some(Kind::Qoa);
    }
    if specular_mpc::is_musepack(s) {
        return Some(Kind::Mpc);
    }
    if s.starts_with(b"fLaC") {
        return Some(Kind::Flac);
    }
    if s.starts_with(b"wvpk") && specular_wavpack::Header::parse(s).is_some() {
        return Some(Kind::WavPack);
    }
    if s.len() >= 6 && (s.starts_with(b"MAC ") || s.starts_with(b"MACF")) {
        return Some(Kind::Ape);
    }
    if dts_sync(s) && specular_dts::probe(&s[..s.len().min(64)]).is_some() {
        return Some(Kind::Dts);
    }
    if s.starts_with(&[0x0B, 0x77]) && specular_ac3::FrameInfo::parse(s).is_some() {
        return Some(Kind::Ac3);
    }
    if s.len() >= 2 && s[0] == 0xFF && s[1] & 0xF6 == 0xF0 && adts_at(s, 0).is_some() {
        return Some(Kind::Adts);
    }
    // MPEG audio: two frames in a row at the start, or after an ID3 tag within a little
    // padding (some taggers leave zeros).
    let id3 = start > 0;
    let reach = if id3 { 4096 } else { 0 };
    (0..=reach.min(s.len()))
        .take_while(|&i| i == 0 || s[i - 1] == 0)
        .find(|&i| s.get(i) == Some(&0xFF) && mpa_at(s, i).is_some())
        .map(|_| Kind::Mpa)
        // Raw GSM by its frames' signatures, last (it has no magic of its own).
        .or_else(|| specular_gsm::is_plain_stream(d).then_some(Kind::Gsm))
}

/// Parses a sound file as a movie with a sound track and no picture.
pub fn parse(d: &[u8], samples: bool) -> Result<Movie> {
    let kind = sniff(d).ok_or_else(|| bad("not a sound file"))?;
    let mut track = match kind {
        Kind::Mpa => mpa(d)?,
        Kind::Adts => adts(d)?,
        Kind::Flac => flac(d)?,
        Kind::Wave => wave(d)?,
        Kind::Aiff => aiff(d)?,
        Kind::Dts => dts(d)?,
        Kind::Ac3 => ac3(d)?,
        Kind::Amr => amr(d)?,
        Kind::WavPack => wavpack(d)?,
        Kind::Ape => ape(d)?,
        Kind::Mpc => crate::mpc_sound::track(d)?,
        Kind::More(k) => crate::pcm_files::track_of(k, d)?,
        Kind::Tta => crate::tta_sound::track(d)?,
        Kind::Shorten => crate::shn_sound::track(d)?,
        Kind::Dsf => crate::dsd_files::dsf(d)?,
        Kind::Dff => crate::dsd_files::dff(d)?,
        Kind::Tracker => crate::tracker_sound::track(d)?,
        Kind::Gsm => crate::speech_files::gsm(d)?,
        Kind::Qcp => crate::speech_files::qcp(d)?,
        Kind::Qoa => crate::qoa_sound::qoa(d)?,
        Kind::Htk => crate::speech_files::htk(d)?,
        Kind::Midi => crate::synth_sound::midi_track(d)?,
        Kind::Bank => crate::synth_sound::bank_track(d)?,
    };
    track.sample_count = track.samples.len() as u32;
    if track.duration == 0 {
        track.duration = track
            .samples
            .last()
            .map_or(0, |s| s.dts + u64::from(s.duration));
    }
    let duration_us = track
        .edit
        .length_us
        .or_else(|| (track.timescale > 0).then(|| track.us(track.duration as i64).max(0) as u64));
    if !samples {
        track.samples = Vec::new();
    }
    Ok(Movie {
        brand: kind.brand(),
        timescale: track.timescale,
        duration_us,
        has_audio: true,
        audio: Some(track),
        ..Movie::default()
    })
}

pub(crate) fn track(
    codec: AudioCodec,
    channels: u16,
    rate: u32,
    samples: Vec<Sample>,
) -> AudioTrack {
    AudioTrack {
        codec,
        channels,
        sample_rate: rate,
        timescale: rate.max(1),
        duration: 0,
        sample_count: 0,
        samples,
        edit: Edit::default(),
        layout: Layout::Contiguous,
        extra: Default::default(),
    }
}

pub(crate) fn sample(offset: usize, size: usize, dts: u64, duration: u64) -> Sample {
    Sample {
        offset: offset as u64,
        size: size as u32,
        dts,
        cts: dts as i64,
        duration: duration as u32,
        sync: true,
    }
}

/// A Xing, Info or VBRI frame (no sound: the stream's frame count and, from LAME, the
/// encoder's delay and padding).
struct VbrFrame {
    frames: Option<u32>,
    delay_padding: Option<(u32, u32)>,
}

fn vbr_frame(f: &[u8], h: &specular_mpa::Header) -> Option<VbrFrame> {
    let mono = h.channels() == 1;
    let side = match (h.version, mono) {
        (specular_mpa::Version::Mpeg1, false) => 32,
        (specular_mpa::Version::Mpeg1, true) => 17,
        (_, false) => 17,
        (_, true) => 9,
    };
    let x = 4 + side;
    if let Some(tag) = f.get(x..x + 4)
        && (tag == b"Xing" || tag == b"Info")
    {
        let flags = u32be(f, x + 4)?;
        let mut at = x + 8;
        let mut frames = None;
        if flags & 1 != 0 {
            frames = u32be(f, at);
            at += 4;
        }
        if flags & 2 != 0 {
            at += 4;
        }
        if flags & 4 != 0 {
            at += 100;
        }
        if flags & 8 != 0 {
            at += 4;
        }
        // The LAME extension: a 9-byte encoder name, 12 bytes of settings, then 12-bit
        // delay and padding.
        let delay_padding = f
            .get(at..at + 24)
            .filter(|l| l[..4].iter().all(u8::is_ascii_alphanumeric))
            .map(|l| {
                let v = u32::from_be_bytes([0, l[21], l[22], l[23]]);
                (v >> 12, v & 0xFFF)
            });
        return Some(VbrFrame {
            frames,
            delay_padding,
        });
    }
    if f.get(36..40) == Some(b"VBRI") {
        return Some(VbrFrame {
            frames: u32be(f, 36 + 14),
            delay_padding: None,
        });
    }
    None
}

/// The MPEG audio synthesis filter's delay in samples (528 + 1), which a LAME delay omits.
const MPA_DECODER_DELAY: u64 = 529;

fn mpa(d: &[u8]) -> Result<AudioTrack> {
    let start = skip_id3v2(d);
    let end = stream_end(d, start);
    let d = &d[..end];
    let mut at = (start..d.len().min(start + 65_536))
        .find(|&i| d[i] == 0xFF && mpa_at(d, i).is_some())
        .ok_or_else(|| bad("no MPEG audio frames"))?;
    let first = mpa_at(d, at).ok_or_else(|| bad("MPEG audio header"))?;
    let spf = first.samples() as u64;
    let mut samples = Vec::new();
    let mut t = 0u64;
    let mut edit = Edit::default();
    let mut gapless = None;
    let mut first_frame = true;
    while at + 4 <= d.len() && samples.len() < MAX_SAMPLES {
        let h = match specular_mpa::Header::parse(&d[at..]) {
            Some(mut h)
                if h.version == first.version
                    && h.layer == first.layer
                    && h.sample_rate == first.sample_rate
                    && (h.bitrate == 0) == (first.bitrate == 0) =>
            {
                h.free_len = first.free_len;
                h
            }
            _ => {
                // Lost sync (damage, junk between frames): find the next pair of frames.
                match (at + 1..d.len()).find(|&i| d[i] == 0xFF && mpa_at(d, i).is_some()) {
                    Some(i) => {
                        at = i;
                        continue;
                    }
                    None => break,
                }
            }
        };
        let Some(len) = h.frame_len() else {
            return Err(Error::Unsupported("free-format MPEG audio".into()));
        };
        let len = len.min(d.len() - at);
        if first_frame && h.layer == 3 {
            first_frame = false;
            if let Some(v) = vbr_frame(&d[at..at + len], &h) {
                gapless = Some(v);
                at += len;
                continue;
            }
        }
        first_frame = false;
        samples.push(sample(at, len, t, spf));
        t += spf;
        at += len;
    }
    if samples.is_empty() {
        return Err(bad("no MPEG audio frames"));
    }
    let rate = first.sample_rate;
    if let Some(VbrFrame {
        frames,
        delay_padding: Some((delay, padding)),
    }) = gapless
    {
        let total = frames.map_or(t, |f| u64::from(f) * spf).min(t);
        let skip = u64::from(delay) + MPA_DECODER_DELAY;
        let played = total.saturating_sub(u64::from(delay) + u64::from(padding));
        if skip < total && played > 0 {
            edit.start = skip as i64;
            edit.length_us = Some(played * 1_000_000 / u64::from(rate.max(1)));
        }
    }
    let mut tr = track(
        AudioCodec::Mpa(first.layer),
        first.channels() as u16,
        rate,
        samples,
    );
    tr.edit = edit;
    Ok(tr)
}

fn adts(d: &[u8]) -> Result<AudioTrack> {
    let start = skip_id3v2(d);
    let end = stream_end(d, start);
    let d = &d[..end];
    let first = adts_at(d, start).ok_or_else(|| bad("no ADTS frames"))?;
    // The core rate by its index (ISO/IEC 14496-3 Table 1.18); SBR doubles what is heard,
    // not the frames' timing.
    const RATES: [u32; 13] = [
        96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025,
        8_000, 7_350,
    ];
    let rate = *RATES
        .get(usize::from(first.rate_index))
        .ok_or_else(|| Error::Unsupported("AAC at a reserved sampling rate".into()))?;
    let mut at = start;
    let mut samples = Vec::new();
    let mut t = 0u64;
    while at + 7 <= d.len() && samples.len() < MAX_SAMPLES {
        let Ok(h) = specular_aac::adts::parse(&d[at..]) else {
            match (at + 1..d.len().saturating_sub(7)).find(|&i| adts_at(d, i).is_some()) {
                Some(i) => {
                    at = i;
                    continue;
                }
                None => break,
            }
        };
        if h.frame_len < 7 {
            break;
        }
        let len = h.frame_len.min(d.len() - at);
        let n = 1024 * u64::from(h.blocks.max(1));
        samples.push(sample(at, len, t, n));
        t += n;
        at += len;
    }
    let ch = u16::from(first.channel_config.max(1));
    Ok(track(AudioCodec::Aac(Vec::new()), ch, rate, samples))
}

fn flac(d: &[u8]) -> Result<AudioTrack> {
    let start = skip_id3v2(d);
    let s = &d[start..];
    let mut at = 4;
    let mut info = None;
    let mut config = b"fLaC".to_vec();
    loop {
        let h = s
            .get(at..at + 4)
            .ok_or_else(|| bad("FLAC metadata cut short"))?;
        let len = u32::from_be_bytes([0, h[1], h[2], h[3]]) as usize;
        let body = s
            .get(at + 4..at + 4 + len)
            .ok_or_else(|| bad("FLAC metadata block cut short"))?;
        if h[0] & 0x7F == 0 && info.is_none() {
            // The configuration: `fLaC` and STREAMINFO alone (all the decoder needs).
            config.extend_from_slice(&[0x80, h[1], h[2], h[3]]);
            config.extend_from_slice(body);
            info = Some(
                specular_flac::StreamInfo::parse(body).map_err(|e| bad(&format!("FLAC: {e}")))?,
            );
        }
        at += 4 + len;
        if h[0] & 0x80 != 0 {
            break;
        }
    }
    let info = info.ok_or_else(|| bad("FLAC without STREAMINFO"))?;
    let frames_at = start + at;
    let body = &d[frames_at..];
    let mut samples = Vec::new();
    let mut t = 0u64;
    for f in specular_flac::frames(body) {
        if samples.len() >= MAX_SAMPLES {
            break;
        }
        let off = f.as_ptr() as usize - body.as_ptr() as usize + frames_at;
        let n = specular_flac::frame_block_size(f).unwrap_or(usize::from(info.max_block)) as u64;
        samples.push(sample(off, f.len(), t, n));
        t += n;
    }
    let mut tr = track(
        AudioCodec::Flac(config),
        u16::from(info.channels),
        info.sample_rate,
        samples,
    );
    if info.total_samples > 0 {
        tr.duration = info.total_samples;
    }
    Ok(tr)
}

/// Runs of `PCM_FRAMES` frames (or whole blocks) over `data`.
pub(crate) fn pcm_samples(
    off: usize,
    len: usize,
    frame_bytes: usize,
    frames_per: u64,
    max: usize,
) -> Vec<Sample> {
    let chunk = frame_bytes * frames_per as usize;
    let mut out = Vec::new();
    let mut at = 0;
    let mut t = 0u64;
    while at < len && out.len() < max {
        let n = chunk.min(len - at);
        let frames = (n / frame_bytes.max(1)) as u64;
        if frames == 0 {
            break;
        }
        out.push(sample(off + at, frames as usize * frame_bytes, t, frames));
        t += frames;
        at += n;
    }
    out
}

/// A track of PCM, G.711 or ADPCM data of `len` bytes at `off`.
pub(crate) fn plain_track(
    codec: AudioCodec,
    channels: u16,
    rate: u32,
    bits: u16,
    off: usize,
    len: usize,
) -> Result<AudioTrack> {
    if channels == 0 || rate == 0 {
        return Err(bad("no channels or sampling rate"));
    }
    let ch = usize::from(channels);
    let samples = match &codec {
        AudioCodec::Adpcm { kind, block } => {
            let block = usize::from(*block).max(1);
            let frames = crate::wave_sound::block_frames(kind, block, ch) as u64;
            let mut out = Vec::new();
            let mut at = 0;
            let mut t = 0;
            while at < len && out.len() < MAX_SAMPLES {
                let n = block.min(len - at);
                out.push(sample(off + at, n, t, frames));
                t += frames;
                at += n;
            }
            out
        }
        AudioCodec::G711 { .. } => pcm_samples(off, len, ch, PCM_FRAMES, MAX_SAMPLES),
        AudioCodec::Pcm { .. } => {
            let fb = ch * usize::from(bits.div_ceil(8)).max(1);
            pcm_samples(off, len, fb, PCM_FRAMES, MAX_SAMPLES)
        }
        // Compressed sound in WAVE (MP3, AC-3, DTS...) is split into its own frames by the
        // caller.
        _ => Vec::new(),
    };
    Ok(track(codec, channels, rate, samples))
}

fn wave(d: &[u8]) -> Result<AudioTrack> {
    let rf64 = &d[..4] == b"RF64" || &d[..4] == b"BW64";
    let mut at = 12;
    let mut fmt = None;
    let mut data = None;
    let mut ds64_data = None;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let mut size = u32le(d, at + 4).unwrap_or(0) as u64;
        if id == b"ds64" {
            ds64_data = u64le(d, at + 16);
        }
        if id == b"data" && rf64 && size == 0xFFFF_FFFF {
            size = ds64_data.unwrap_or(0);
        }
        let body = at + 8;
        let len = (size as usize).min(d.len() - body);
        match id {
            b"fmt " => fmt = Some(&d[body..body + len]),
            b"data" if data.is_none() => data = Some((body, len)),
            _ => {}
        }
        at = body.saturating_add(len + (len & 1));
        if data.is_some() && fmt.is_some() && at >= d.len() {
            break;
        }
    }
    let f = fmt.ok_or_else(|| bad("WAVE without a format chunk"))?;
    let (off, len) = data.ok_or_else(|| bad("WAVE without a data chunk"))?;
    wave_format_track(d, f, off, len)
}

/// The track of WAVE-style sound (RIFF WAVE, RF64, BW64, Sony Wave64): its WAVEFORMATEX `f`
/// and the `len` bytes of data at `off`.
pub(crate) fn wave_format_track(d: &[u8], f: &[u8], off: usize, len: usize) -> Result<AudioTrack> {
    // WAVE_FORMAT_EXTENSIBLE: the sub-format GUID's first two bytes are the format tag.
    let mut f = f.to_vec();
    if u16le(&f, 0) == Some(0xFFFE)
        && let Some(sub) = u16le(&f, 24)
    {
        f[..2].copy_from_slice(&sub.to_le_bytes());
    }
    let (codec, channels, rate) =
        crate::avi::wave_codec(&f).ok_or_else(|| bad("WAVE format chunk cut short"))?;
    let bits = u16le(&f, 14).unwrap_or(16);
    match codec {
        AudioCodec::Mpa(_) => {
            let mut t = mpa(&d[..off + len]).map_err(|_| bad("MPEG audio in WAVE"))?;
            t.edit = Edit::default();
            Ok(t)
        }
        AudioCodec::Ac3 => ac3_in(d, off, off + len),
        AudioCodec::Other(t, _) if &t == b"dtsc" => dts_in(d, off, off + len),
        c if crate::speech_files::is_gsm(&c) => {
            crate::speech_files::gsm_in_wave(channels, rate, off, len)
        }
        AudioCodec::Pcm { .. } | AudioCodec::G711 { .. } | AudioCodec::Adpcm { .. } => {
            let mut t = plain_track(codec, channels, rate, bits, off, len)?;
            // 16-bit PCM that is really DTS (DTS CDs ripped to WAVE).
            crate::dts_sound::complete(d, &mut t);
            if crate::dts_sound::is_dts(&t.codec) {
                return dts_in(d, off, off + len);
            }
            Ok(t)
        }
        other => Err(Error::Unsupported(format!(
            "{} sound in WAVE",
            match other {
                AudioCodec::Other(t, _) if &t == b"WMA " || &t == b"WMAV" => "WMA".to_owned(),
                AudioCodec::Other(..) => format!("format {:#06x}", u16le(&f, 0).unwrap_or(0)),
                o => o.name(),
            }
        ))),
    }
}

/// An AMR storage file: one sample per frame block (a frame per channel), 20 ms each. A
/// header this decoder doesn't know is refused in words.
fn amr(d: &[u8]) -> Result<AudioTrack> {
    use specular_amr::{Codec, Error as AmrError, storage};
    let (info, stats) = storage::scan(d).map_err(|e| match e {
        AmrError::Unsupported(m) => Error::Unsupported(m),
        AmrError::Malformed(m) => bad(&m),
    })?;
    let spf = info.codec.frame_len() as u64;
    let mut samples = Vec::new();
    let mut at = info.header_len;
    let mut t = 0;
    while at < d.len() && samples.len() < MAX_SAMPLES {
        let start = at;
        for _ in 0..info.channels {
            match storage::read_frame(info.codec, &d[at.min(d.len())..]) {
                Some((_, used)) => at += used,
                None => break,
            }
        }
        if at == start {
            break;
        }
        samples.push(sample(start, at - start, t, spf));
        t += spf;
    }
    let tag = if info.codec == Codec::Nb {
        *b"samr"
    } else {
        *b"sawb"
    };
    let modes = crate::amr_sound::modes_byte(&stats);
    Ok(track(
        AudioCodec::Other(tag, modes),
        info.channels,
        info.codec.sample_rate(),
        samples,
    ))
}

fn wavpack(d: &[u8]) -> Result<AudioTrack> {
    let info = specular_wavpack::info(d).map_err(|e| match e {
        specular_wavpack::Error::Unsupported(m) => Error::Unsupported(format!("WavPack: {m}")),
        specular_wavpack::Error::Malformed(m) => bad(&format!("WavPack: {m}")),
    })?;
    let samples: Vec<Sample> = specular_wavpack::frames(d, MAX_SAMPLES)
        .into_iter()
        .map(|f| sample(f.offset, f.len, f.index, u64::from(f.samples)))
        .collect();
    let mut tr = track(
        AudioCodec::WavPack {
            channels: info.channels as u16,
            bits: match (info.dsd, info.float) {
                (Some(_), _) => 1,
                (None, true) => 32,
                (None, false) => u16::from(info.bits),
            },
            hybrid: info.hybrid,
            float: info.float && info.dsd.is_none(),
            dsd: info.dsd.map(|d| (d.rate, d.samples)),
        },
        info.channels as u16,
        info.sample_rate,
        samples,
    );
    if let Some(n) = info.total_samples {
        tr.duration = n;
    }
    Ok(tr)
}

fn ape(d: &[u8]) -> Result<AudioTrack> {
    let info = specular_ape::info(d).map_err(|e| match e {
        specular_ape::Error::Unsupported(m) => Error::Unsupported(format!("Monkey's Audio: {m}")),
        specular_ape::Error::Malformed(m) => bad(&format!("Monkey's Audio: {m}")),
    })?;
    let mut t = 0u64;
    let mut samples = Vec::with_capacity(info.frames.len().min(MAX_SAMPLES));
    for f in info.frames.iter().take(MAX_SAMPLES) {
        samples.push(sample(f.offset, f.len, t, u64::from(f.blocks)));
        t += u64::from(f.blocks);
    }
    let mut tr = track(
        AudioCodec::Ape {
            bits: info.bits,
            level: info.level,
        },
        info.channels,
        info.sample_rate,
        samples,
    );
    tr.duration = info.total_blocks;
    Ok(tr)
}

/// An 80-bit IEEE 754 extended float (AIFF's sampling rate).
fn extended(b: &[u8]) -> Option<f64> {
    let exp = i32::from(u16be(b, 0)? & 0x7FFF);
    let mant = u64::from_be_bytes(b.get(2..10)?.try_into().ok()?);
    if exp == 0 && mant == 0 {
        return Some(0.0);
    }
    Some(mant as f64 * 2f64.powi(exp - 16383 - 63))
}

fn aiff(d: &[u8]) -> Result<AudioTrack> {
    let aifc = &d[8..12] == b"AIFC";
    let end = (u32be(d, 4).unwrap_or(0) as usize + 8).min(d.len());
    let mut at = 12;
    let mut comm = None;
    let mut ssnd = None;
    while at + 8 <= end {
        let id = &d[at..at + 4];
        let size = u32be(d, at + 4).unwrap_or(0) as usize;
        let body = at + 8;
        let len = size.min(end - body);
        match id {
            b"COMM" => comm = Some(&d[body..body + len]),
            b"SSND" => ssnd = Some((body, len)),
            _ => {}
        }
        at = body.saturating_add(len + (len & 1));
    }
    let c = comm.ok_or_else(|| bad("AIFF without a COMM chunk"))?;
    let (sbody, slen) = ssnd.ok_or_else(|| bad("AIFF without sound data"))?;
    let channels = u16be(c, 0).ok_or_else(|| bad("COMM cut short"))?;
    let bits = u16be(c, 6).unwrap_or(16);
    let rate = c.get(8..18).and_then(extended).unwrap_or(0.0).round() as u32;
    let kind: [u8; 4] = if aifc {
        c.get(18..22)
            .and_then(|b| b.try_into().ok())
            .unwrap_or(*b"NONE")
    } else {
        *b"NONE"
    };
    let skip = u32be(d, sbody).unwrap_or(0) as usize;
    let off = sbody + 8 + skip;
    let len = slen.saturating_sub(8 + skip);
    let pcm = |bits: u16, float: bool, le: bool| AudioCodec::Pcm {
        bits,
        float,
        little_endian: le,
    };
    let (codec, bits) = match &kind {
        b"NONE" | b"twos" => (pcm(bits, false, false), bits),
        b"sowt" => (pcm(bits, false, true), bits),
        b"in24" => (pcm(24, false, false), 24),
        b"in32" => (pcm(32, false, false), 32),
        b"fl32" | b"FL32" => (pcm(32, true, false), 32),
        b"fl64" | b"FL64" => (pcm(64, true, false), 64),
        b"raw " => (pcm(8, false, true), 8),
        b"ulaw" | b"ULAW" => (AudioCodec::G711 { alaw: false }, 8),
        b"alaw" | b"ALAW" => (AudioCodec::G711 { alaw: true }, 8),
        b"ima4" => (
            AudioCodec::Adpcm {
                kind: AdpcmKind::ImaQt,
                block: 34 * channels,
            },
            4,
        ),
        other => {
            return Err(Error::Unsupported(format!(
                "AIFF-C '{}' sound",
                String::from_utf8_lossy(other).trim()
            )));
        }
    };
    // 8-bit AIFF samples are signed: big-endian 8-bit PCM is (`AudioCodec::Pcm`).
    plain_track(codec, channels, rate, bits, off, len)
}

/// DTS frames from one core sync word to the next.
fn dts_in(d: &[u8], start: usize, end: usize) -> Result<AudioTrack> {
    let d = &d[..end];
    let info = specular_dts::probe(&d[start..end.min(start + 65_536)])
        .ok_or_else(|| bad("no DTS frames"))?;
    let first = (start..end)
        .find(|&i| dts_sync(&d[i..]))
        .ok_or_else(|| bad("no DTS frames"))?;
    let spf = info.first.samples as u64;
    // 14-bit words carry 14 bits of every 16: a frame takes 16/14 of its size.
    let step = match info.packing {
        specular_dts::Packing::Be14 | specular_dts::Packing::Le14 => info.first.size * 16 / 14,
        _ => info.first.size,
    }
    .max(16);
    let sync = &d[first..first + 4];
    let mut samples = Vec::new();
    let mut at = first;
    let mut t = 0;
    while at < end && samples.len() < MAX_SAMPLES {
        // The next sync of the same packing, near where the frame size says.
        let from = (at + step.saturating_sub(4)).min(end);
        let next = (from..end)
            .find(|&i| d[i..].starts_with(sync) && dts_sync(&d[i..]))
            .unwrap_or(end);
        samples.push(sample(at, next - at, t, spf));
        t += spf;
        at = next;
    }
    Ok(track(
        AudioCodec::Other(*b"dtsc", 0),
        info.first.channels as u16,
        info.first.sample_rate,
        samples,
    ))
}

fn dts(d: &[u8]) -> Result<AudioTrack> {
    let start = skip_id3v2(d);
    dts_in(d, start, stream_end(d, start))
}

/// AC-3 or E-AC-3 syncframes: each independent frame with the dependent frames after it.
fn ac3_in(d: &[u8], start: usize, end: usize) -> Result<AudioTrack> {
    let d = &d[..end];
    let mut at = (start..end.min(start + 65_536))
        .find(|&i| {
            d[i..].starts_with(&[0x0B, 0x77]) && specular_ac3::FrameInfo::parse(&d[i..]).is_some()
        })
        .ok_or_else(|| bad("no AC-3 frames"))?;
    let first = specular_ac3::FrameInfo::parse(&d[at..]).ok_or_else(|| bad("AC-3 header"))?;
    let eac3 = first.bsid > 10;
    let mut samples: Vec<Sample> = Vec::new();
    let mut t = 0u64;
    while at + 6 <= end && samples.len() < MAX_SAMPLES {
        let Some(h) = specular_ac3::FrameInfo::parse(&d[at..]) else {
            match (at + 1..end).find(|&i| {
                d[i..].starts_with(&[0x0B, 0x77])
                    && specular_ac3::FrameInfo::parse(&d[i..]).is_some()
            }) {
                Some(i) => {
                    at = i;
                    continue;
                }
                None => break,
            }
        };
        let len = h.frame_bytes.clamp(6, end - at);
        let n = h.samples() as u64;
        match samples.last_mut() {
            // Dependent substreams and further programmes belong with the programme-0
            // frame before them (one time slot).
            Some(last)
                if (h.strmtyp == 1 || h.substreamid != 0)
                    && last.offset + u64::from(last.size) == at as u64 =>
            {
                last.size += len as u32;
            }
            _ => {
                samples.push(sample(at, len, t, n));
                t += n;
            }
        }
        at += len;
    }
    Ok(track(
        if eac3 {
            AudioCodec::Eac3
        } else {
            AudioCodec::Ac3
        },
        // with dependent substreams' extension channels (8 for 7.1)
        specular_ac3::probe(&d[start..end.min(start + 65_536)])
            .map_or(first.channels(), |s| s.channels) as u16,
        first.sample_rate,
        samples,
    ))
}

fn ac3(d: &[u8]) -> Result<AudioTrack> {
    let start = skip_id3v2(d);
    ac3_in(d, start, stream_end(d, start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id3v2_is_skipped() {
        let mut d = b"ID3\x04\x00\x00\x00\x00\x01\x00".to_vec();
        d.extend(std::iter::repeat_n(0u8, 128));
        d.extend_from_slice(b"fLaC");
        assert_eq!(skip_id3v2(&d), 138);
        assert_eq!(sniff(&d), Some(Kind::Flac));
        // A tag with a footer, then a second tag.
        let mut d = b"ID3\x04\x00\x10\x00\x00\x00\x02".to_vec();
        d.extend_from_slice(&[0, 0]);
        d.extend_from_slice(b"3DI\x04\x00\x10\x00\x00\x00\x02");
        d.extend_from_slice(b"ID3\x03\x00\x00\x00\x00\x00\x00");
        assert_eq!(skip_id3v2(&d), 32);
    }

    #[test]
    fn extended_rates() {
        // 44100 and 48000 Hz as AIFF writes them.
        let r = extended(&[0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(r, 44_100.0);
        let r = extended(&[0x40, 0x0E, 0xBB, 0x80, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(r, 48_000.0);
    }

    #[test]
    fn not_sound() {
        assert_eq!(sniff(b"\xFF\xD8\xFF\xE0\0\x10JFIF\0"), None);
        assert_eq!(sniff(b"RIFF\0\0\0\0AVI LIST"), None);
        assert_eq!(sniff(&[0xFF; 64]), None);
        assert_eq!(sniff(&[]), None);
    }
}
