//! MPEG transport streams (`.ts`, and AVCHD / Blu-ray `.mts` / `.m2ts` with their 4-byte
//! timestamp before each packet) into the same [`Movie`] the MP4 demuxer gives.
//!
//! - Packets of 188 bytes, or 192 (BDAV); the first sync bytes found decide which.
//! - PAT → the first program's PMT → its first video stream (H.264, HEVC; MPEG-1/2 video,
//!   MPEG-4 Part 2 and VC-1 are named) and first audio stream (Blu-ray LPCM plays; AC-3,
//!   E-AC-3, DTS, TrueHD, AAC and MPEG audio are named).
//! - PES packets: one per sample (an access unit, or for field-coded H.264 one field); PTS and
//!   DTS unwrapped across the 33-bit wrap-around, and across a jump of the program clock (PCR
//!   discontinuity, or a jump of over a second), so the timeline keeps going.
//! - A sample's bytes are spread over packets: [`TsLayout::read`] gathers them, and for H.264
//!   and HEVC turns the Annex B start codes into 4-byte lengths, so the decoders take them as
//!   they take an MP4's. The configuration record (`avcC` / `hvcC`) is made from the stream's
//!   first parameter sets.
//! - Keyframes: the random access indicator, an IDR picture, a recovery point SEI or an
//!   intra-only first slice. The recording date and camera make of AVCHD camcorders come from
//!   the `MDPM` user data in the first picture's SEI.
//!
//! Times are in 90 kHz ticks from the first picture or sound (whichever is earlier).

use crate::mp4::{
    AudioCodec, AudioTrack, Codec, Edit, Error, Layout, MAX_SAMPLES, Movie, Result, Sample,
    VideoTrack,
};

const SYNC: u8 = 0x47;
/// The PTS wrap (33 bits).
const WRAP: i64 = 1 << 33;
/// Bytes of an access unit read at parse time to find keyframes and parameter sets.
const PEEK: usize = 4096;

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// What the elementary stream of a track carries, for [`TsLayout::read`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EsKind {
    /// H.264 / HEVC Annex B: start codes become 4-byte lengths.
    AnnexB,
    /// Blu-ray LPCM: the 4-byte header of each PES packet is dropped.
    BdLpcm,
    /// MPEG-1/2 video: a sample (a picture, or a field pair) may span several PES packets.
    Mpeg,
    /// As stored.
    Raw,
}

/// Where a transport stream track's samples are: `Sample::offset` is the file offset of the
/// sync byte of the packet where the sample's PES packet starts, `Sample::size` the count of
/// elementary stream bytes it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TsLayout {
    /// Packet stride: 188, or 192 with the BDAV timestamp.
    pub stride: u16,
    pub pid: u16,
    pub kind: EsKind,
}

/// A transport packet's payload (`None` when it has none or is not the PID's).
struct Packet<'a> {
    pid: u16,
    start: bool,
    payload: &'a [u8],
    /// Adaptation field: random access indicator, discontinuity indicator, PCR.
    random_access: bool,
    discontinuity: bool,
    pcr: Option<i64>,
}

fn packet(p: &[u8]) -> Option<Packet<'_>> {
    if p.len() < 188 || p[0] != SYNC {
        return None;
    }
    let pid = u16::from(p[1] & 0x1F) << 8 | u16::from(p[2]);
    let start = p[1] & 0x40 != 0;
    let afc = (p[3] >> 4) & 3;
    let mut at = 4;
    let (mut random_access, mut discontinuity, mut pcr) = (false, false, None);
    if afc & 2 != 0 {
        let len = usize::from(p[4]);
        if len > 0 && 5 + len <= 188 {
            let flags = p[5];
            discontinuity = flags & 0x80 != 0;
            random_access = flags & 0x40 != 0;
            if flags & 0x10 != 0 && len >= 7 {
                let b = &p[6..12];
                let base = (i64::from(b[0]) << 25)
                    | (i64::from(b[1]) << 17)
                    | (i64::from(b[2]) << 9)
                    | (i64::from(b[3]) << 1)
                    | i64::from(b[4] >> 7);
                pcr = Some(base);
            }
        }
        at = 5 + len;
    }
    let payload = if afc & 1 != 0 && at <= 188 {
        &p[at..188]
    } else {
        &[][..]
    };
    Some(Packet {
        pid,
        start,
        payload,
        random_access,
        discontinuity,
        pcr,
    })
}

/// The packet stride and the offset of the first sync byte, when `data` looks like a
/// transport stream (five sync bytes in a row at that stride).
fn framing(data: &[u8]) -> Option<(usize, usize)> {
    for stride in [188usize, 192] {
        for first in 0..stride.min(data.len()) {
            if (0..5).all(|k| data.get(first + k * stride) == Some(&SYNC)) {
                return Some((stride, first));
            }
        }
    }
    None
}

/// Whether `head` is the start of a transport stream (188 or 192-byte packets).
pub fn sniff(head: &[u8]) -> bool {
    // The sync byte must come at once (BDAV: after the 4-byte timestamp); five in a row.
    [(188usize, 0usize), (192, 4)]
        .iter()
        .any(|&(s, f)| (0..5).all(|k| head.get(f + k * s) == Some(&SYNC)))
}

/// A PES packet header: where the payload starts, PTS and DTS (33-bit, raw).
fn pes_header(b: &[u8]) -> Option<(usize, Option<i64>, Option<i64>)> {
    if b.len() < 9 || b[..3] != [0, 0, 1] {
        return None;
    }
    let id = b[3];
    // Streams without the optional header (padding, private 2, ECM...).
    if matches!(id, 0xBC | 0xBE | 0xBF | 0xF0 | 0xF1 | 0xF2 | 0xF8 | 0xFF) {
        return Some((6, None, None));
    }
    let flags = b[7] >> 6;
    let hl = usize::from(b[8]);
    let ts = |o: usize| -> Option<i64> {
        let t = b.get(o..o + 5)?;
        Some(
            (i64::from(t[0] >> 1 & 7) << 30)
                | (i64::from(t[1]) << 22)
                | (i64::from(t[2] >> 1) << 15)
                | (i64::from(t[3]) << 7)
                | i64::from(t[4] >> 1),
        )
    };
    let pts = if flags & 2 != 0 { ts(9) } else { None };
    let dts = if flags == 3 { ts(14) } else { None };
    Some((9 + hl, pts, dts))
}

impl TsLayout {
    /// Gathers up to `limit` elementary stream bytes of the sample whose PES packet starts in
    /// the packet at `offset`.
    fn gather(&self, data: &[u8], offset: usize, limit: usize) -> Option<Vec<u8>> {
        let stride = usize::from(self.stride);
        let mut out = Vec::with_capacity(limit.min(1 << 22));
        let mut at = offset;
        let mut first = true;
        while out.len() < limit {
            let Some(p) = data.get(at..at + 188).and_then(packet) else {
                // The file ends (or loses sync) inside the sample.
                break;
            };
            at += stride;
            if p.pid != self.pid {
                continue;
            }
            let mut pl = p.payload;
            if first {
                if !p.start {
                    return None;
                }
                let (h, ..) = pes_header(pl)?;
                pl = pl.get(h..)?;
                if self.kind == EsKind::BdLpcm {
                    pl = pl.get(4..).unwrap_or_default();
                }
                first = false;
            } else if p.start {
                if self.kind != EsKind::Mpeg {
                    break;
                }
                let (h, ..) = pes_header(pl)?;
                pl = pl.get(h..)?;
            }
            let n = pl.len().min(limit - out.len());
            out.extend_from_slice(&pl[..n]);
        }
        (!first).then_some(out)
    }

    /// The bytes of sample `s` (see [`crate::mp4::Layout::read`]).
    pub fn read(&self, data: &[u8], s: &Sample) -> Option<Vec<u8>> {
        let es = self.gather(data, usize::try_from(s.offset).ok()?, s.size as usize)?;
        Some(match self.kind {
            EsKind::AnnexB => length_prefixed(&es),
            _ => es,
        })
    }
}

/// The NAL units of an Annex B byte stream (start codes removed, trailing zeros kept off).
pub(crate) fn nal_units(es: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= es.len() {
        if es[i] == 0 && es[i + 1] == 0 && es[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let n = starts.len();
    (0..n).map(move |k| {
        let from = starts[k];
        let mut to = if k + 1 < n {
            starts[k + 1] - 3
        } else {
            es.len()
        };
        while to > from && es[to - 1] == 0 {
            to -= 1;
        }
        &es[from..to]
    })
}

/// Annex B as 4-byte length-prefixed NAL units.
pub(crate) fn length_prefixed(es: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(es.len() + 16);
    for nal in nal_units(es).filter(|n| !n.is_empty()) {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

/// A stream of the PMT.
#[derive(Clone, Copy, Debug)]
struct Stream {
    pid: u16,
    stream_type: u8,
    /// A registration or codec descriptor's tag (AC-3, E-AC-3, DTS, Opus...).
    hint: Option<[u8; 4]>,
    /// The ISO 639 language descriptor's (tag 0x0A) code, lower case.
    lang: Option<[u8; 3]>,
    /// Who the sound is for, from the ISO 639 descriptor's `audio_type` or DVB's
    /// supplementary audio descriptor.
    purpose: AudioPurpose,
}

/// Who a sound stream is for (ISO/IEC 13818-1 `audio_type`; ETSI EN 300 468
/// `editorial_classification`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum AudioPurpose {
    #[default]
    Main,
    /// Effects without dialogue (`audio_type` 1).
    CleanEffects,
    /// For the hard of hearing (`audio_type` 2; DVB clean audio).
    HearingImpaired,
    /// Audio description, commentary for the blind and partially sighted (`audio_type` 3;
    /// DVB audio description and spoken subtitles).
    VisualImpaired,
}

impl AudioPurpose {
    fn from_audio_type(t: u8) -> AudioPurpose {
        match t {
            1 => AudioPurpose::CleanEffects,
            2 => AudioPurpose::HearingImpaired,
            3 => AudioPurpose::VisualImpaired,
            _ => AudioPurpose::Main,
        }
    }
}

/// The program's streams from a PMT section.
fn pmt(section: &[u8]) -> Option<Vec<Stream>> {
    if section.first() != Some(&0x02) || section.len() < 12 {
        return None;
    }
    let len = (usize::from(section[1] & 0x0F) << 8 | usize::from(section[2])) + 3;
    let end = len.min(section.len()).checked_sub(4)?;
    let info = usize::from(section[10] & 0x0F) << 8 | usize::from(section[11]);
    let mut p = 12 + info;
    let mut out = Vec::new();
    while p + 5 <= end {
        let stream_type = section[p];
        let pid = u16::from(section[p + 1] & 0x1F) << 8 | u16::from(section[p + 2]);
        let es_len = usize::from(section[p + 3] & 0x0F) << 8 | usize::from(section[p + 4]);
        let d = section
            .get(p + 5..(p + 5 + es_len).min(end))
            .unwrap_or_default();
        let mut hint = None;
        let (mut lang, mut purpose) = (None, AudioPurpose::Main);
        let mut q = 0;
        while q + 2 <= d.len() {
            let (tag, l) = (d[q], usize::from(d[q + 1]));
            let body = d.get(q + 2..q + 2 + l).unwrap_or_default();
            // ISO 639 language: the first entry's code and audio type
            if tag == 0x0A && body.len() >= 4 && lang.is_none() {
                let code = [body[0], body[1], body[2]].map(|b| b.to_ascii_lowercase());
                if code.iter().all(u8::is_ascii_alphabetic) {
                    lang = Some(code);
                }
                if purpose == AudioPurpose::Main {
                    purpose = AudioPurpose::from_audio_type(body[3]);
                }
            }
            // DVB supplementary audio (extension 0x06): editorial classification
            if tag == 0x7F && body.len() >= 2 && body[0] == 0x06 {
                match (body[1] >> 2) & 0x1F {
                    1 | 3 => purpose = AudioPurpose::VisualImpaired,
                    2 => purpose = AudioPurpose::HearingImpaired,
                    _ => {}
                }
            }
            hint = hint.or(match tag {
                0x05 if body.len() >= 4 => body[..4].try_into().ok(),
                0x6A => Some(*b"AC-3"),
                0x7A => Some(*b"EAC3"),
                0x7B => Some(*b"DTS1"),
                0x7C => Some(*b"AAC "),
                _ => None,
            });
            q += 2 + l;
        }
        out.push(Stream {
            pid,
            stream_type,
            hint,
            lang,
            purpose,
        });
        p += 5 + es_len;
    }
    Some(out)
}

/// The caption services the first program's map lists (ATSC caption service descriptors,
/// tag 0x86, in the program or the video stream's descriptors): their languages.
pub fn caption_services(data: &[u8]) -> Vec<crate::captions::ServiceLang> {
    let Some((stride, first)) = framing(data) else {
        return Vec::new();
    };
    let n_packets = (data.len() - first + stride - 188) / stride;
    let mut pmt_pid = None;
    for k in 0..n_packets.min(200_000) {
        let Some(p) = packet(&data[first + k * stride..]) else {
            continue;
        };
        if !p.start || p.payload.is_empty() {
            continue;
        }
        let ptr = usize::from(p.payload[0]);
        let Some(section) = p.payload.get(1 + ptr..) else {
            continue;
        };
        if p.pid == 0 && pmt_pid.is_none() {
            pmt_pid = pat(section);
        } else if Some(p.pid) == pmt_pid {
            return pmt_caption_services(section);
        }
    }
    Vec::new()
}

/// Caption service descriptors in a PMT section.
fn pmt_caption_services(section: &[u8]) -> Vec<crate::captions::ServiceLang> {
    let mut out = Vec::new();
    if section.first() != Some(&0x02) || section.len() < 12 {
        return out;
    }
    let len = (usize::from(section[1] & 0x0F) << 8 | usize::from(section[2])) + 3;
    let Some(end) = len.min(section.len()).checked_sub(4) else {
        return out;
    };
    let mut loops = Vec::new();
    let info = usize::from(section[10] & 0x0F) << 8 | usize::from(section[11]);
    loops.push(section.get(12..(12 + info).min(end)).unwrap_or_default());
    let mut p = 12 + info;
    while p + 5 <= end {
        let es_len = usize::from(section[p + 3] & 0x0F) << 8 | usize::from(section[p + 4]);
        loops.push(
            section
                .get(p + 5..(p + 5 + es_len).min(end))
                .unwrap_or_default(),
        );
        p += 5 + es_len;
    }
    for d in loops {
        let mut q = 0;
        while q + 2 <= d.len() {
            let (tag, l) = (d[q], usize::from(d[q + 1]));
            let body = d.get(q + 2..q + 2 + l).unwrap_or_default();
            if tag == 0x86 && !body.is_empty() {
                let n = usize::from(body[0] & 0x1F);
                for s in body[1..].chunks_exact(6).take(n) {
                    let digital = s[3] & 0x80 != 0;
                    let lang: String = s[..3]
                        .iter()
                        .filter(|b| b.is_ascii_alphabetic())
                        .map(|&b| char::from(b).to_ascii_lowercase())
                        .collect();
                    out.push(crate::captions::ServiceLang {
                        digital,
                        number: if digital { s[3] & 0x3F } else { (s[3] & 1) + 1 },
                        lang,
                    });
                }
            }
            q += 2 + l;
        }
    }
    out
}

/// The first program's PMT PID from a PAT section.
fn pat(section: &[u8]) -> Option<u16> {
    if section.first() != Some(&0x00) || section.len() < 12 {
        return None;
    }
    let len = (usize::from(section[1] & 0x0F) << 8 | usize::from(section[2])) + 3;
    let end = len.min(section.len()).checked_sub(4)?;
    let mut p = 8;
    while p + 4 <= end {
        let program = u16::from_be_bytes([section[p], section[p + 1]]);
        let pid = u16::from(section[p + 2] & 0x1F) << 8 | u16::from(section[p + 3]);
        if program != 0 {
            return Some(pid);
        }
        p += 4;
    }
    None
}

/// The video coding of a PMT stream type (with its registration hint), if video.
fn video_codec(s: &Stream) -> Option<Codec> {
    Some(match s.stream_type {
        0x1B => Codec::H264,
        0x24 => Codec::Hevc,
        0x01 => Codec::Mpeg1,
        0x02 => Codec::Mpeg2,
        0x10 => Codec::Mpeg4,
        0xEA => Codec::Vc1,
        0x20 => Codec::Other(*b"mvc1"),
        0x33 => Codec::Other(*b"vvc1"),
        0x06 if s.hint == Some(*b"VC-1") => Codec::Vc1,
        _ => return None,
    })
}

/// The audio coding of a PMT stream type, if audio; Blu-ray LPCM is PCM decoded later from
/// its header.
fn audio_codec(s: &Stream, bdav: bool) -> Option<AudioCodec> {
    let other = |c: &[u8; 4]| Some(AudioCodec::Other(*c, 0));
    match (s.stream_type, s.hint.as_ref().map(|h| &h[..])) {
        (0x80, _) if bdav => Some(AudioCodec::Pcm {
            bits: 16,
            float: false,
            little_endian: false,
        }),
        (0x03 | 0x04, _) => Some(AudioCodec::Mpa(0)),
        (0x0F, _) => Some(AudioCodec::Aac(Vec::new())),
        (0x11, _) => Some(AudioCodec::Aac(Vec::new())),
        (0x81, _) | (0x06, Some(b"AC-3")) => Some(AudioCodec::Ac3),
        // Blu-ray's secondary E-AC-3 (0xA1) and E-AC-3 (0x84 in BDAV, 0x87 in ATSC)
        (0x84 | 0x87 | 0xA1, _) | (0x06, Some(b"EAC3")) => Some(AudioCodec::Eac3),
        // Blu-ray: DTS-HD High Resolution (0x85), Master Audio (0x86), Express (0xA2).
        (0x85, _) => other(b"dtsh"),
        (0x86, _) => other(b"dtsl"),
        (0xA2, _) => other(b"dtse"),
        (0x82, _) | (0x06, Some(b"DTS1" | b"DTS2" | b"DTS3")) => other(b"dtsc"),
        (0x83, _) => other(b"mlpa"),
        (0x06, Some(b"Opus")) => other(b"Opus"),
        _ => None,
    }
}

/// A PES packet seen while scanning: where it starts, its ES byte count, its times, and the
/// random access indicator of its first packet.
#[derive(Clone, Copy, Debug)]
struct Pes {
    offset: usize,
    size: usize,
    pts: Option<i64>,
    dts: Option<i64>,
    random_access: bool,
}

/// Unwraps 33-bit times into a running 64-bit clock, with an offset added after each jump of
/// the program clock.
#[derive(Default)]
struct Clock {
    last_pcr: Option<i64>,
    /// Added to every time read from here on.
    shift: i64,
}

impl Clock {
    fn pcr(&mut self, raw: i64, discontinuity: bool) {
        let Some(last) = self.last_pcr else {
            self.last_pcr = Some(raw);
            return;
        };
        let raw = unwrap(raw, last - self.shift) + self.shift;
        let jump = raw - last;
        if discontinuity || !(-90_000 / 10..=90_000).contains(&jump) {
            // The clock jumped: the new times continue from the old.
            self.shift += last - raw;
            self.last_pcr = Some(last);
        } else {
            self.last_pcr = Some(raw);
        }
    }
}

/// The 33-bit `raw` time nearest to `near` (unwrapped).
fn unwrap(raw: i64, near: i64) -> i64 {
    let mut d = (raw - near).rem_euclid(WRAP);
    if d >= WRAP / 2 {
        d -= WRAP;
    }
    near + d
}

/// Parses a transport stream. With `samples`, every PES packet of the chosen video and audio
/// streams is listed; without, only the first and last few megabytes are read for the streams
/// and the duration.
pub fn parse(data: &[u8], samples: bool) -> Result<Movie> {
    parse_with(data, samples, None)
}

/// The first program's sound streams, labelled (the `id` is the PID, what [`parse_with`]
/// takes): language and who each is for, from the ISO 639 and DVB supplementary audio
/// descriptors. Empty with fewer than two (nothing to choose).
pub fn audio_tracks(data: &[u8]) -> Vec<crate::extras::TrackInfo> {
    let Some((stride, first)) = framing(data) else {
        return Vec::new();
    };
    let bdav = stride == 192;
    let Some(streams) = program(data, stride, first) else {
        return Vec::new();
    };
    let sound: Vec<&Stream> = streams
        .iter()
        .filter(|s| audio_codec(s, bdav).is_some())
        .collect();
    if sound.len() < 2 {
        return Vec::new();
    }
    let main = default_sound(&sound);
    sound
        .iter()
        .map(|s| {
            let codec = audio_codec(s, bdav).map_or_else(String::new, |c| c.name());
            crate::extras::TrackInfo {
                id: u64::from(s.pid),
                codec,
                lang: s
                    .lang
                    .map(|c| String::from_utf8_lossy(&c).into_owned())
                    .filter(|c| c != "und"),
                default: main.is_some_and(|m| m.pid == s.pid),
                hearing_impaired: s.purpose == AudioPurpose::HearingImpaired,
                visual_impaired: s.purpose == AudioPurpose::VisualImpaired,
                ..Default::default()
            }
        })
        .collect()
}

/// The sound stream that plays unless one is picked: the first for everyone (not clean
/// effects, not for the hard of hearing or audio description), else the first.
fn default_sound<'a>(sound: &[&'a Stream]) -> Option<&'a Stream> {
    sound
        .iter()
        .find(|s| s.purpose == AudioPurpose::Main)
        .or(sound.first())
        .copied()
}

/// The first program's streams: the PAT, then the PMT (sections in one packet, as almost
/// every muxer writes).
fn program(data: &[u8], stride: usize, first: usize) -> Option<Vec<Stream>> {
    let n_packets = (data.len() - first + stride - 188) / stride;
    let mut pmt_pid = None;
    for k in 0..n_packets.min(200_000) {
        let Some(p) = packet(&data[first + k * stride..]) else {
            continue;
        };
        if !p.start || p.payload.is_empty() {
            continue;
        }
        let ptr = usize::from(p.payload[0]);
        let Some(section) = p.payload.get(1 + ptr..) else {
            continue;
        };
        if p.pid == 0 && pmt_pid.is_none() {
            pmt_pid = pat(section);
        } else if Some(p.pid) == pmt_pid
            && let Some(s) = pmt(section)
        {
            return Some(s);
        }
    }
    None
}

/// [`parse`] with the sound stream `audio` (a PID, from [`audio_tracks`]) instead of the
/// default one.
pub fn parse_with(data: &[u8], samples: bool, audio: Option<u64>) -> Result<Movie> {
    let (stride, first) = framing(data).ok_or_else(|| bad("not a transport stream"))?;
    let bdav = stride == 192;
    // Whole 188-byte packets from the first sync byte (a BDAV file's last one ends 4 bytes
    // short of a 192-byte stride).
    let n_packets = (data.len() - first + stride - 188) / stride;
    let at = |k: usize| first + k * stride;

    let streams = program(data, stride, first).ok_or_else(|| bad("no program map"))?;
    let vs = streams
        .iter()
        .filter(|s| video_codec(s).is_some())
        .min_by_key(|s| !matches!(video_codec(s), Some(Codec::H264 | Codec::Hevc)))
        .copied();
    let sound: Vec<&Stream> = streams
        .iter()
        .filter(|s| audio_codec(s, bdav).is_some())
        .collect();
    let au = audio
        .and_then(|id| sound.iter().find(|s| u64::from(s.pid) == id).copied())
        .or_else(|| default_sound(&sound))
        .copied();
    if vs.is_none() && au.is_none() {
        return Err(Error::Unsupported("no video or sound in the stream".into()));
    }

    let stride16 = stride as u16;
    let mut video = vs.map(|s| {
        let codec = video_codec(&s);
        let annexb = matches!(codec, Some(Codec::H264 | Codec::Hevc));
        let mpeg12 = matches!(codec, Some(Codec::Mpeg2 | Codec::Mpeg1));
        VideoTrack {
            codec,
            timescale: 90_000,
            layout: Layout::Ts(TsLayout {
                stride: stride16,
                pid: s.pid,
                kind: if annexb {
                    EsKind::AnnexB
                } else if mpeg12 {
                    EsKind::Mpeg
                } else {
                    EsKind::Raw
                },
            }),
            ..VideoTrack::default()
        }
    });
    let mut audio = au.map(|s| {
        let codec = audio_codec(&s, bdav).unwrap_or(AudioCodec::Other(*b"????", 0));
        let lpcm = matches!(codec, AudioCodec::Pcm { .. });
        AudioTrack {
            codec,
            channels: 2,
            sample_rate: 48_000,
            timescale: 90_000,
            duration: 0,
            sample_count: 0,
            samples: Vec::new(),
            edit: Edit::default(),
            layout: Layout::Ts(TsLayout {
                stride: stride16,
                pid: s.pid,
                kind: if lpcm { EsKind::BdLpcm } else { EsKind::Raw },
            }),
            extra: Default::default(),
        }
    });

    // The PES packets of the two streams (all of them, or those at both ends of the file).
    let vpid = vs.map(|s| s.pid);
    let apid = au.map(|s| s.pid);
    let ranges: Vec<std::ops::Range<usize>> = if samples {
        std::iter::once(0..n_packets).collect()
    } else {
        let edge = (4usize << 20) / stride;
        if n_packets <= 2 * edge {
            std::iter::once(0..n_packets).collect()
        } else {
            vec![0..edge, n_packets - edge..n_packets]
        }
    };
    let mut vpes: Vec<Pes> = Vec::new();
    let mut apes: Vec<Pes> = Vec::new();
    for r in ranges {
        // The clock starts afresh in each range (the gap between them is not a jump).
        let mut clock = Clock::default();
        let mut open_v: Option<Pes> = None;
        let mut open_a: Option<Pes> = None;
        for k in r {
            let Some(p) = packet(&data[at(k)..]) else {
                continue;
            };
            if let Some(pcr) = p.pcr {
                clock.pcr(pcr, p.discontinuity);
            }
            let which = if Some(p.pid) == vpid {
                (&mut open_v, &mut vpes)
            } else if Some(p.pid) == apid {
                (&mut open_a, &mut apes)
            } else {
                continue;
            };
            let (open, list) = which;
            if p.start {
                if let Some(done) = open.take() {
                    list.push(done);
                }
                let Some((h, pts, dts)) = pes_header(p.payload) else {
                    continue;
                };
                let shift = |t: i64| {
                    let near = clock.last_pcr.unwrap_or(t);
                    unwrap(t, near - clock.shift) + clock.shift
                };
                *open = Some(Pes {
                    offset: at(k),
                    size: p.payload.len().saturating_sub(h),
                    pts: pts.map(shift),
                    dts: dts.map(shift),
                    random_access: p.random_access,
                });
            } else if let Some(o) = open.as_mut() {
                o.size += p.payload.len();
            }
            if vpes.len() + apes.len() > 2 * MAX_SAMPLES {
                return Err(Error::Unsupported("too many pictures".into()));
            }
        }
        // A PES packet cut by the end of a range: the last one is kept only at the end of the
        // file (it may be short; its decoder copes).
        vpes.extend(open_v);
        apes.extend(open_a);
    }
    // Without a PCR the times were unwrapped against themselves: make them run on.
    unwrap_list(&mut vpes);
    unwrap_list(&mut apes);

    // The origin: the earliest presentation of a picture or a sound.
    let origin = vpes
        .iter()
        .chain(apes.iter())
        .filter_map(|p| p.pts)
        .min()
        .unwrap_or(0);
    let mut movie = Movie {
        brand: if bdav { *b"bdav" } else { *b"mpts" },
        timescale: 90_000,
        has_audio: audio.is_some(),
        ..Movie::default()
    };

    if let Some(v) = video.as_mut()
        && let Layout::Ts(layout) = v.layout
    {
        video_track(data, layout, &vpes, origin, v, &mut movie, samples)?;
    }
    if let Some(a) = audio.as_mut()
        && let Layout::Ts(layout) = a.layout
    {
        audio_track(data, layout, &apes, origin, a, samples);
    }
    let end = |t: Option<&VideoTrack>, a: Option<&AudioTrack>| {
        let v = t.map_or(0, |v| v.duration);
        let a = a.map_or(0, |a| a.duration);
        v.max(a)
    };
    let d = end(video.as_ref(), audio.as_ref());
    movie.duration_us = (d > 0).then(|| d * 1_000_000 / 90_000);
    movie.video = video;
    movie.audio = audio;
    Ok(movie)
}

/// Times of PES packets without a PCR to unwrap them against: each against the one before.
fn unwrap_list(list: &mut [Pes]) {
    let mut last: Option<i64> = None;
    for p in list.iter_mut() {
        if let Some(t) = p.pts {
            let u = last.map_or(t, |l| unwrap(t, l));
            p.pts = Some(u);
            if let Some(d) = p.dts {
                p.dts = Some(unwrap(d, u));
            }
            last = Some(u);
        }
    }
}

/// Fills the video track from its PES packets: samples, times, keyframes, the configuration
/// record, the coded size and colour, and the camcorder's date.
fn video_track(
    data: &[u8],
    layout: TsLayout,
    pes: &[Pes],
    origin: i64,
    v: &mut VideoTrack,
    movie: &mut Movie,
    samples: bool,
) -> Result<()> {
    if pes.len() > MAX_SAMPLES {
        return Err(Error::Unsupported(format!("{} pictures", pes.len())));
    }
    let codec = v.codec;
    let h264 = codec == Some(Codec::H264);
    let hevc = codec == Some(Codec::Hevc);
    let mpeg12 = matches!(codec, Some(Codec::Mpeg2 | Codec::Mpeg1));
    let mut mpeg_config = Vec::new();
    // MPEG-1/2: the first field of a frame waiting for its second.
    let mut open_field: Option<specular_mpeg2::Structure> = None;
    // Per sample: an MPEG B picture (shown before the reference decoded ahead of it when a
    // muxer stamps both with one time).
    let mut b_picture: Vec<bool> = Vec::with_capacity(pes.len());
    let (mut started, mut leading_b, mut last_skipped) = (!mpeg12, false, false);
    // Presentation times: a PES packet without a PTS follows the previous one.
    let mut list: Vec<Sample> = Vec::with_capacity(pes.len());
    let mut sets = ParamSets::default();
    let mut last_pts = None;
    for (i, p) in pes.iter().enumerate() {
        let pts = p.pts.or(last_pts.map(|t: i64| t + 1)).unwrap_or(origin);
        last_pts = Some(pts);
        let mut sync = p.random_access;
        if codec == Some(Codec::Vc1) && !sync {
            // VC-1: an access unit with a sequence header or entry point starts a GOP.
            let head = layout.gather(data, p.offset, 64).unwrap_or_default();
            sync = head
                .windows(4)
                .any(|w| w[..3] == [0, 0, 1] && matches!(w[3], 0x0E | 0x0F));
        }
        if mpeg12 && (samples || i < 64) {
            // MPEG-1/2: a sample starting with an I picture; the first sequence header is
            // the configuration.
            let mut head = layout.gather(data, p.offset, PEEK).unwrap_or_default();
            head.truncate(p.size);
            let scan = specular_mpeg2::scan(&head);
            // A packet that doesn't start a picture (the rest of one, or its second field)
            // joins the sample before it.
            let lead = head.iter().take_while(|&&b| b == 0).count();
            let starts = lead >= 2
                && head.get(lead) == Some(&1)
                && matches!(head.get(lead + 1), Some(0x00 | 0xB3 | 0xB8));
            let first = scan.pictures.first();
            let second = head.get(lead + 1) == Some(&0x00)
                && matches!((open_field, first), (Some(o), Some(f)) if f.structure != specular_mpeg2::Structure::Frame && f.structure != o);
            for pic in &scan.pictures {
                open_field = match (open_field, pic.structure) {
                    (_, specular_mpeg2::Structure::Frame) => None,
                    (Some(o), st) if st != o => None,
                    (_, st) => Some(st),
                };
            }
            if (!starts || second) && !list.is_empty() {
                if !last_skipped && let Some(prev) = list.last_mut() {
                    prev.size = prev
                        .size
                        .saturating_add(p.size.min(u32::MAX as usize) as u32);
                }
                continue;
            }
            // A recording that starts mid-stream: pictures before the first sequence header
            // and I picture, and the B pictures right after it that predict from before it,
            // can't be decoded and are left out.
            if samples {
                let kind = first.map(|f| f.kind);
                last_skipped = false;
                if !started {
                    if scan.sequence.is_some() && kind == Some(specular_mpeg2::PictureType::I) {
                        started = true;
                        leading_b = !(scan.gop && scan.closed_gop);
                    } else {
                        last_skipped = true;
                        continue;
                    }
                } else if leading_b {
                    if kind == Some(specular_mpeg2::PictureType::B) {
                        last_skipped = true;
                        continue;
                    }
                    leading_b = false;
                }
            }
            b_picture.push(first.is_some_and(|f| f.kind == specular_mpeg2::PictureType::B));
            sync |= scan
                .pictures
                .first()
                .is_some_and(|p| p.kind == specular_mpeg2::PictureType::I);
            if mpeg_config.is_empty()
                && let Some(b) = specular_mpeg2::sequence_bytes(&head)
            {
                mpeg_config = b.to_vec();
            }
        }
        if (h264 || hevc) && (samples || i < 64) {
            let head = layout.gather(data, p.offset, PEEK).unwrap_or_default();
            let k = if h264 {
                h264_kind(&head, &mut sets, movie)
            } else {
                hevc_kind(&head, &mut sets)
            };
            sync |= k;
        }
        list.push(Sample {
            offset: p.offset as u64,
            size: p.size.min(u32::MAX as usize) as u32,
            dts: (p.dts.unwrap_or(pts) - origin).max(0) as u64,
            cts: pts - origin,
            duration: 0,
            sync,
        });
    }
    // Two pictures stamped with the same time (muxers do that) stay two frames: the later in
    // decoding order a tick after.
    let mut by_time: Vec<usize> = (0..list.len()).collect();
    by_time.sort_by_key(|&i| (list[i].cts, !b_picture.get(i).copied().unwrap_or(false), i));
    for w in 1..by_time.len() {
        let (a, b) = (by_time[w - 1], by_time[w]);
        if list[b].cts <= list[a].cts {
            list[b].cts = list[a].cts + 1;
        }
    }
    // Each lasts until the next in presentation order.
    let mut order: Vec<i64> = list.iter().map(|s| s.cts).collect();
    order.sort_unstable();
    order.dedup();
    let typical = order
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 0)
        .min()
        .unwrap_or(3003);
    for s in &mut list {
        let k = order.partition_point(|&c| c <= s.cts);
        s.duration = order
            .get(k)
            .map_or(typical, |n| n - s.cts)
            .clamp(0, i64::from(u32::MAX)) as u32;
    }
    if !list.iter().any(|s| s.sync)
        && let Some(s) = list.first_mut()
    {
        // Nothing marks a keyframe: decoding starts at the first picture.
        s.sync = true;
    }
    v.duration = order.last().map_or(0, |&l| (l + typical).max(0) as u64);
    v.config = if h264 {
        sets.avcc()
    } else if hevc {
        sets.hvcc()
    } else {
        mpeg_config
    };
    if let Some((w, h)) = sets.size {
        (v.width, v.height) = (w, h);
        v.display = (w, h);
    }
    if let Some(par) = sets.aspect {
        v.pixel_aspect = Some(par);
        v.display = v.shown_size();
    }
    v.interlaced = sets.interlaced;
    if (h264 || hevc) && v.config.is_empty() && !list.is_empty() {
        return Err(bad("no parameter sets in the video stream"));
    }
    v.sample_count = list.len() as u32;
    if samples {
        v.samples = list;
    }
    Ok(())
}

/// Fills the audio track from its PES packets.
fn audio_track(
    data: &[u8],
    layout: TsLayout,
    pes: &[Pes],
    origin: i64,
    a: &mut AudioTrack,
    samples: bool,
) {
    let mut list: Vec<Sample> = pes
        .iter()
        .filter_map(|p| {
            let t = p.pts? - origin;
            Some(Sample {
                offset: p.offset as u64,
                size: p.size.min(u32::MAX as usize) as u32,
                dts: t.max(0) as u64,
                cts: t,
                duration: 0,
                sync: true,
            })
        })
        .collect();
    for i in 0..list.len() {
        let next = list.get(i + 1).map(|n| n.dts);
        let s = &mut list[i];
        s.duration = next
            .map_or(0, |n| n.saturating_sub(s.dts))
            .min(u64::from(u32::MAX)) as u32;
    }
    if let Some(l) = list.len().checked_sub(2) {
        let d = list[l].duration;
        list[l + 1].duration = d;
    }
    if let AudioCodec::Pcm { bits, .. } = &mut a.codec
        && let Some(p) = pes.first()
        && let Some(h) = (TsLayout {
            kind: EsKind::Raw,
            ..layout
        })
        .gather(data, p.offset, 4)
        .filter(|h| h.len() == 4)
    {
        // Blu-ray LPCM header: size, channel assignment and rate, bits per sample.
        let (ch, rate) = (h[2] >> 4, h[2] & 0x0F);
        let coded = match ch {
            1 => 1,
            3 => 2,
            4 | 5 => 3,
            6 | 7 => 4,
            8 => 5,
            9 => 6,
            10 => 7,
            11 => 8,
            _ => 2,
        };
        // Odd channel counts carry a padding channel.
        a.channels = coded + coded % 2;
        a.sample_rate = match rate {
            4 => 96_000,
            5 => 192_000,
            _ => 48_000,
        };
        *bits = match h[3] >> 6 {
            2 | 3 => 24,
            _ => 16,
        };
    }
    if matches!(&a.codec, AudioCodec::Aac(asc) if asc.is_empty())
        && let Some(h) = pes.first().and_then(|p| layout.gather(data, p.offset, 16))
    {
        // the first ADTS header or LATM StreamMuxConfig says the core rate and channels
        let cfg = specular_aac::adts::parse(&h)
            .and_then(|h| h.config())
            .ok()
            .or_else(|| {
                let big = pes
                    .first()
                    .and_then(|p| layout.gather(data, p.offset, 256))?;
                let mut latm = specular_aac::latm::Latm::new();
                let el = specular_aac::latm::loas_frames(&big).next()?;
                let _ = latm.parse(el, &mut Vec::new());
                latm.config().cloned()
            });
        if let Some(c) = cfg {
            a.sample_rate = c.sample_rate;
            a.channels = c.core_channels().max(1) as u16;
        }
    }
    a.duration = list.last().map_or(0, |s| s.dts + u64::from(s.duration));
    a.sample_count = list.len() as u32;
    if samples {
        a.samples = list;
    }
}

/// Parameter sets and stream facts collected from the first pictures.
#[derive(Default)]
pub(crate) struct ParamSets {
    vps: Vec<Vec<u8>>,
    sps: Vec<Vec<u8>>,
    pps: Vec<Vec<u8>>,
    pub(crate) size: Option<(u32, u32)>,
    pub(crate) aspect: Option<(u32, u32)>,
    pub(crate) interlaced: bool,
    date_seen: bool,
}

impl ParamSets {
    fn keep(list: &mut Vec<Vec<u8>>, nal: &[u8]) {
        if list.len() < 16 && !list.iter().any(|x| x == nal) {
            list.push(nal.to_vec());
        }
    }

    /// An `avcC` record of the H.264 parameter sets seen.
    pub(crate) fn avcc(&self) -> Vec<u8> {
        let Some(sps) = self.sps.first().filter(|s| s.len() >= 4) else {
            return Vec::new();
        };
        let mut v = vec![1, sps[1], sps[2], sps[3], 0xFF, 0xE0 | self.sps.len() as u8];
        for s in &self.sps {
            v.extend_from_slice(&(s.len() as u16).to_be_bytes());
            v.extend_from_slice(s);
        }
        v.push(self.pps.len() as u8);
        for p in &self.pps {
            v.extend_from_slice(&(p.len() as u16).to_be_bytes());
            v.extend_from_slice(p);
        }
        v
    }

    /// An `hvcC` record of the HEVC parameter sets seen.
    pub(crate) fn hvcc(&self) -> Vec<u8> {
        let Some(sps) = self.sps.first() else {
            return Vec::new();
        };
        // general_profile_space … general_level_idc: 12 bytes after the SPS's first byte of
        // payload (video parameter set id, sub-layers, nesting).
        let rbsp = unescape(sps.get(2..).unwrap_or_default());
        let mut ptl = [0u8; 12];
        if let Some(p) = rbsp.get(1..13) {
            ptl.copy_from_slice(p);
        }
        let mut v = vec![1];
        v.extend_from_slice(&ptl[..1]);
        v.extend_from_slice(&ptl[1..5]);
        v.extend_from_slice(&ptl[5..11]);
        v.push(ptl[11]);
        v.extend_from_slice(&[0xF0, 0x00, 0xFC, 0xFD, 0xF8, 0xF8, 0, 0, 0x0F]);
        let arrays = [(32u8, &self.vps), (33, &self.sps), (34, &self.pps)];
        v.push(arrays.iter().filter(|a| !a.1.is_empty()).count() as u8);
        for (kind, list) in arrays {
            if list.is_empty() {
                continue;
            }
            v.push(0x80 | kind);
            v.extend_from_slice(&(list.len() as u16).to_be_bytes());
            for n in list.iter() {
                v.extend_from_slice(&(n.len() as u16).to_be_bytes());
                v.extend_from_slice(n);
            }
        }
        v
    }
}

/// Emulation prevention bytes removed.
pub(crate) fn unescape(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut zeros = 0;
    for &x in b {
        if zeros >= 2 && x == 3 {
            zeros = 0;
            continue;
        }
        zeros = if x == 0 { zeros + 1 } else { 0 };
        out.push(x);
    }
    out
}

/// A bit reader for the few header fields read here.
struct Bits<'a> {
    b: &'a [u8],
    p: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.b.get(self.p / 8)?;
            v = (v << 1) | u32::from(byte >> (7 - self.p % 8) & 1);
            self.p += 1;
        }
        Some(v)
    }
    fn ue(&mut self) -> Option<u32> {
        let mut z = 0;
        while self.u(1)? == 0 {
            z += 1;
            if z > 31 {
                return None;
            }
        }
        Some((1u32 << z) - 1 + self.u(z)?)
    }
    fn se(&mut self) -> Option<i32> {
        let k = self.ue()?;
        Some(if k & 1 == 1 {
            (k / 2 + 1) as i32
        } else {
            -((k / 2) as i32)
        })
    }
}

/// Whether an H.264 access unit (Annex B) starts at a keyframe (IDR, a recovery point, or an
/// intra slice); keeps its parameter sets and reads the size and the camcorder's date.
pub(crate) fn h264_kind(es: &[u8], sets: &mut ParamSets, movie: &mut Movie) -> bool {
    let mut key = false;
    for nal in nal_units(es) {
        let Some(&h) = nal.first() else { continue };
        match h & 0x1F {
            5 => key = true,
            7 => {
                if sets.size.is_none() {
                    h264_sps(nal, sets);
                }
                ParamSets::keep(&mut sets.sps, nal);
            }
            8 => ParamSets::keep(&mut sets.pps, nal),
            6 => {
                let rbsp = unescape(&nal[1..]);
                key |= sei_recovery_point(&rbsp);
                if !sets.date_seen {
                    sets.date_seen |= mdpm(&rbsp, movie);
                }
            }
            1 => {
                // slice_type 2 / 7: I; 4 / 9: SI.
                let rbsp = unescape(&nal[1..nal.len().min(16)]);
                let mut b = Bits { b: &rbsp, p: 0 };
                if b.ue().is_some() && matches!(b.ue(), Some(2 | 4 | 7 | 9)) {
                    key = true;
                }
                break;
            }
            _ => {}
        }
    }
    key
}

/// Whether an HEVC access unit starts at an intra random access point; keeps its parameter
/// sets.
pub(crate) fn hevc_kind(es: &[u8], sets: &mut ParamSets) -> bool {
    let mut key = false;
    for nal in nal_units(es) {
        if nal.len() < 2 {
            continue;
        }
        match (nal[0] >> 1) & 0x3F {
            16..=21 => key = true,
            32 => ParamSets::keep(&mut sets.vps, nal),
            33 => {
                if sets.size.is_none() {
                    hevc_sps(nal, sets);
                }
                ParamSets::keep(&mut sets.sps, nal);
            }
            34 => ParamSets::keep(&mut sets.pps, nal),
            _ => {}
        }
    }
    key
}

/// The coded size, field coding and sample aspect ratio from an H.264 SPS.
/// The coded size an H.264 sequence parameter set gives (after cropping).
pub(crate) fn h264_size(sps: &[u8]) -> Option<(u32, u32)> {
    let mut sets = ParamSets::default();
    h264_sps(sps, &mut sets);
    sets.size
}

fn h264_sps(nal: &[u8], sets: &mut ParamSets) -> Option<()> {
    let rbsp = unescape(&nal[1..]);
    let mut b = Bits { b: &rbsp, p: 0 };
    let profile = b.u(8)?;
    b.u(16)?;
    b.ue()?;
    let mut chroma = 1;
    if matches!(
        profile,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        chroma = b.ue()?;
        if chroma == 3 {
            b.u(1)?;
        }
        b.ue()?;
        b.ue()?;
        b.u(1)?;
        if b.u(1)? == 1 {
            for i in 0..if chroma == 3 { 12 } else { 8 } {
                if b.u(1)? == 1 {
                    let n = if i < 6 { 16 } else { 64 };
                    let (mut last, mut next) = (8i32, 8i32);
                    for _ in 0..n {
                        if next != 0 {
                            next = (last + b.se()? + 256) % 256;
                        }
                        if next != 0 {
                            last = next;
                        }
                    }
                }
            }
        }
    }
    b.ue()?;
    match b.ue()? {
        0 => {
            b.ue()?;
        }
        1 => {
            b.u(1)?;
            b.se()?;
            b.se()?;
            for _ in 0..b.ue()?.min(255) {
                b.se()?;
            }
        }
        _ => {}
    }
    b.ue()?;
    b.u(1)?;
    let w_mbs = b.ue()? + 1;
    let h_units = b.ue()? + 1;
    let frame_mbs_only = b.u(1)?;
    if frame_mbs_only == 0 {
        b.u(1)?;
        sets.interlaced = true;
    }
    b.u(1)?;
    let (mut cl, mut cr, mut ct, mut cb) = (0, 0, 0, 0);
    if b.u(1)? == 1 {
        (cl, cr, ct, cb) = (b.ue()?, b.ue()?, b.ue()?, b.ue()?);
    }
    let (sub_w, sub_h) = match chroma {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    let crop_y = sub_h * (2 - frame_mbs_only);
    let w = (w_mbs * 16).saturating_sub(sub_w * (cl + cr));
    let h = (h_units * 16 * (2 - frame_mbs_only)).saturating_sub(crop_y * (ct + cb));
    sets.size = Some((w.min(1 << 16), h.min(1 << 16)));
    // VUI: the sample aspect ratio.
    if b.u(1)? == 1 && b.u(1)? == 1 {
        let idc = b.u(8)?;
        let par = match idc {
            255 => (b.u(16)?, b.u(16)?),
            i => *[
                (1, 1),
                (1, 1),
                (12, 11),
                (10, 11),
                (16, 11),
                (40, 33),
                (24, 11),
                (20, 11),
                (32, 11),
                (80, 33),
                (18, 11),
                (15, 11),
                (64, 33),
                (160, 99),
                (4, 3),
                (3, 2),
                (2, 1),
            ]
            .get(i as usize)
            .unwrap_or(&(1, 1)),
        };
        if par.0 > 0 && par.1 > 0 && par.0 != par.1 {
            sets.aspect = Some(par);
        }
    }
    Some(())
}

/// The coded size (after the conformance window) from an HEVC SPS.
fn hevc_sps(nal: &[u8], sets: &mut ParamSets) -> Option<()> {
    let rbsp = unescape(nal.get(2..)?);
    let mut b = Bits { b: &rbsp, p: 0 };
    b.u(4)?;
    let sub_layers = b.u(3)?;
    b.u(1)?;
    // profile_tier_level(1, sub_layers)
    b.u(8)?;
    b.u(32)?;
    b.u(32)?;
    b.u(16)?;
    b.u(8)?;
    let mut flags = Vec::new();
    for _ in 0..sub_layers {
        flags.push((b.u(1)?, b.u(1)?));
    }
    if sub_layers > 0 {
        for _ in sub_layers..8 {
            b.u(2)?;
        }
    }
    for (profile, level) in flags {
        if profile == 1 {
            b.u(32)?;
            b.u(32)?;
            b.u(24)?;
        }
        if level == 1 {
            b.u(8)?;
        }
    }
    b.ue()?;
    let chroma = b.ue()?;
    if chroma == 3 {
        b.u(1)?;
    }
    let (w, h) = (b.ue()?, b.ue()?);
    let (mut cl, mut cr, mut ct, mut cb) = (0, 0, 0, 0);
    if b.u(1)? == 1 {
        (cl, cr, ct, cb) = (b.ue()?, b.ue()?, b.ue()?, b.ue()?);
    }
    let (sw, sh) = match chroma {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    sets.size = Some((
        w.saturating_sub(sw * (cl + cr)).min(1 << 16),
        h.saturating_sub(sh * (ct + cb)).min(1 << 16),
    ));
    Some(())
}

/// The SEI messages of an SEI RBSP: (payload type, payload).
fn sei_messages(rbsp: &[u8]) -> Vec<(u32, &[u8])> {
    let mut out = Vec::new();
    let mut p = 0;
    while p + 2 <= rbsp.len() && rbsp[p] != 0x80 {
        let mut t = 0u32;
        while rbsp.get(p) == Some(&0xFF) {
            t += 255;
            p += 1;
        }
        t += u32::from(*rbsp.get(p).unwrap_or(&0));
        p += 1;
        let mut s = 0usize;
        while rbsp.get(p) == Some(&0xFF) {
            s += 255;
            p += 1;
        }
        s += usize::from(*rbsp.get(p).unwrap_or(&0));
        p += 1;
        let Some(body) = rbsp.get(p..p + s) else {
            break;
        };
        out.push((t, body));
        p += s;
    }
    out
}

fn sei_recovery_point(rbsp: &[u8]) -> bool {
    sei_messages(rbsp).iter().any(|m| m.0 == 6)
}

/// The UUID of AVCHD's `MDPM` user data (camcorder metadata in the SEI).
const MDPM_UUID: [u8; 16] = [
    0x17, 0xEE, 0x8C, 0x60, 0xF8, 0x4D, 0x11, 0xD9, 0x8C, 0xD6, 0x08, 0x00, 0x20, 0x0C, 0x9A, 0x66,
];

/// Reads the recording date and the make from AVCHD `MDPM` user data; true when found.
fn mdpm(rbsp: &[u8], movie: &mut Movie) -> bool {
    for (t, body) in sei_messages(rbsp) {
        if t != 5 || body.len() < 21 || body[..16] != MDPM_UUID || &body[16..20] != b"MDPM" {
            continue;
        }
        let n = usize::from(body[20]);
        let mut tags = std::collections::HashMap::new();
        for e in body[21..].chunks_exact(5).take(n) {
            tags.insert(e[0], [e[1], e[2], e[3], e[4]]);
        }
        let bcd = |b: u8| -> Option<u32> {
            let (hi, lo) = (u32::from(b >> 4), u32::from(b & 0x0F));
            (hi < 10 && lo < 10).then_some(hi * 10 + lo)
        };
        if let (Some(a), Some(b)) = (tags.get(&0x18), tags.get(&0x19)) {
            // 0x18: zone, year (two BCD bytes), month; 0x19: day, hour, minute, second.
            let date = (|| {
                let year = bcd(a[1])? * 100 + bcd(a[2])?;
                let (mo, d, h, mi, s) =
                    (bcd(a[3])?, bcd(b[0])?, bcd(b[1])?, bcd(b[2])?, bcd(b[3])?);
                Some(format!("{year:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}"))
            })();
            if date.is_some() {
                movie.date = date;
            }
        }
        if let Some(m) = tags.get(&0xE0) {
            movie.make = match u16::from_be_bytes([m[0], m[1]]) {
                0x0103 => Some("Panasonic".into()),
                0x0108 => Some("Sony".into()),
                0x1011 => Some("Canon".into()),
                0x1104 => Some("JVC".into()),
                _ => movie.make.take(),
            };
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn caption_service_descriptors() {
        // A 608 service on field 1 in English, a 708 service 2 in Spanish, on the video.
        let desc = [
            0x86, 13, 0xE2, b'e', b'n', b'g', 0x7E, 0x3F, 0xFF, b's', b'p', b'a', 0xC2, 0x3F, 0xFF,
        ];
        let mut es = vec![0x1B, 0xE1, 0x00, 0xF0, desc.len() as u8];
        es.extend(desc);
        let mut sec = vec![0x02, 0xB0, 0, 0, 1, 0xC1, 0, 0, 0xE1, 0x00, 0xF0, 0];
        sec.extend(es);
        sec.extend([0; 4]);
        sec[2] = (sec.len() - 3) as u8;
        let s = super::pmt_caption_services(&sec);
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].digital, s[0].number, s[0].lang.as_str()),
            (false, 1, "eng")
        );
        assert_eq!(
            (s[1].digital, s[1].number, s[1].lang.as_str()),
            (true, 2, "spa")
        );
    }

    use super::*;

    /// A 188-byte packet of `pid` with `payload` (stuffed with an adaptation field when short).
    fn ts_packet(pid: u16, start: bool, cc: u8, payload: &[u8], pcr: Option<i64>) -> Vec<u8> {
        let mut p = vec![
            SYNC,
            (pid >> 8) as u8 | if start { 0x40 } else { 0 },
            pid as u8,
        ];
        let room = 184 - payload.len();
        if room == 0 && pcr.is_none() {
            p.push(0x10 | cc);
        } else {
            p.push(0x30 | cc);
            let len = room - 1;
            p.push(len as u8);
            if len > 0 {
                let mut flags = 0u8;
                let mut af = Vec::new();
                if let Some(t) = pcr {
                    flags |= 0x10;
                    af.extend_from_slice(&[
                        (t >> 25) as u8,
                        (t >> 17) as u8,
                        (t >> 9) as u8,
                        (t >> 1) as u8,
                        ((t & 1) << 7) as u8 | 0x7E,
                        0,
                    ]);
                }
                p.push(flags);
                p.extend_from_slice(&af);
                p.resize(5 + len, 0xFF);
            }
        }
        p.extend_from_slice(payload);
        assert_eq!(p.len(), 188);
        p
    }

    fn pts_bytes(marker: u8, t: i64) -> [u8; 5] {
        [
            marker << 4 | ((t >> 29) & 0x0E) as u8 | 1,
            (t >> 22) as u8,
            ((t >> 14) & 0xFE) as u8 | 1,
            (t >> 7) as u8,
            ((t << 1) & 0xFE) as u8 | 1,
        ]
    }

    /// A Baseline SPS for 320×240 (progressive, no VUI).
    fn sps_320x240() -> Vec<u8> {
        let mut bits: Vec<u8> = Vec::new();
        let mut u = |v: u32, n: u32| {
            for k in (0..n).rev() {
                bits.push((v >> k & 1) as u8);
            }
        };
        let ue = |u: &mut dyn FnMut(u32, u32), v: u32| {
            let x = v + 1;
            let n = 32 - x.leading_zeros();
            u(0, n - 1);
            u(x, n);
        };
        u(66, 8);
        u(0xC0, 8);
        u(30, 8);
        ue(&mut u, 0); // sps id
        ue(&mut u, 0); // log2_max_frame_num - 4
        ue(&mut u, 2); // poc type
        ue(&mut u, 1); // refs
        u(0, 1); // gaps
        ue(&mut u, 19); // width in MBs - 1
        ue(&mut u, 14); // height in map units - 1
        u(1, 1); // frame_mbs_only
        u(1, 1); // direct_8x8
        u(0, 1); // cropping
        u(0, 1); // vui
        u(1, 1); // stop bit
        while !bits.len().is_multiple_of(8) {
            bits.push(0);
        }
        let mut out = vec![0x67];
        out.extend(
            bits.chunks(8)
                .map(|c| c.iter().fold(0u8, |a, &b| a << 1 | b)),
        );
        out
    }

    fn section(table: u8, body: &[u8]) -> Vec<u8> {
        let len = body.len() + 5 + 4;
        let mut s = vec![
            0,
            table,
            0xB0 | (len >> 8) as u8,
            len as u8,
            0,
            1,
            0xC1,
            0,
            0,
        ];
        s.extend_from_slice(body);
        s.extend_from_slice(&[0, 0, 0, 0]);
        s
    }

    /// A stream with one H.264 PID (0x100) and one LPCM-less MPEG audio PID: `n` access units
    /// of `au` bytes, PTS from `t0` by 3003 (wrapping past 2^33).
    fn stream(n: usize, t0: i64) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(ts_packet(
            0,
            true,
            0,
            &section(0, &[0, 1, 0xE0, 0x20]),
            None,
        ));
        let pmt_body = [
            0xE1, 0x00, 0xF0, 0x00, 0x1B, 0xE1, 0x00, 0xF0, 0x00, 0x03, 0xE1, 0x01, 0xF0, 0x00,
        ];
        out.extend(ts_packet(0x20, true, 0, &section(2, &pmt_body), None));
        let sps = sps_320x240();
        for i in 0..n {
            let t = (t0 + i as i64 * 3003) % WRAP;
            let mut pes = vec![0, 0, 1, 0xE0, 0, 0, 0x80, 0x80, 5];
            pes.extend_from_slice(&pts_bytes(2, t));
            pes.extend_from_slice(&[0, 0, 0, 1]);
            pes.extend_from_slice(&sps);
            pes.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xCE, 0x38, 0x80]);
            // An IDR slice every 4, else a P slice; padded to cross a packet boundary.
            // first_mb 0; slice_type 2 (I) or 5 (P).
            let mut slice = if i % 4 == 0 {
                vec![0, 0, 1, 0x65, 0xB8]
            } else {
                vec![0, 0, 1, 0x41, 0x98]
            };
            slice.resize(300, i as u8 | 0x10);
            pes.extend_from_slice(&slice);
            let mut chunks = pes.chunks(176).peekable();
            let mut first = true;
            while let Some(c) = chunks.next() {
                let pcr = (first && i % 2 == 0).then_some(t);
                out.extend(ts_packet(0x100, first, i as u8, c, pcr));
                first = false;
                // Interleave an audio packet.
                if chunks.peek().is_some() {
                    let mut a = vec![0, 0, 1, 0xC0, 0, 0, 0x80, 0x80, 5];
                    a.extend_from_slice(&pts_bytes(2, t));
                    a.resize(100, 0xAA);
                    out.extend(ts_packet(0x101, true, i as u8, &a, None));
                }
            }
        }
        out
    }

    #[test]
    fn pes_packets_become_samples() {
        let data = stream(10, WRAP - 3 * 3003);
        assert!(sniff(&data));
        let m = parse(&data, true).unwrap();
        let v = m.video.as_ref().unwrap();
        assert_eq!(v.codec, Some(Codec::H264));
        assert_eq!(v.samples.len(), 10);
        // Times run on across the 33-bit wrap.
        let cts: Vec<i64> = v.samples.iter().map(|s| s.cts).collect();
        assert_eq!(cts, (0..10).map(|i| i * 3003).collect::<Vec<_>>());
        assert_eq!(
            v.samples.iter().map(|s| s.sync).collect::<Vec<_>>(),
            (0..10).map(|i| i % 4 == 0).collect::<Vec<_>>()
        );
        assert_eq!((v.width, v.height), (320, 240));
        // The sample reads back length-prefixed.
        let b = v.sample_data(&data, &v.samples[1]).unwrap();
        assert_eq!(&b[..5], &[0, 0, 0, sps_320x240().len() as u8, 0x67]);
        let mut p = 0;
        let mut kinds = Vec::new();
        while p + 4 <= b.len() {
            let n = u32::from_be_bytes(b[p..p + 4].try_into().unwrap()) as usize;
            kinds.push(b[p + 4] & 0x1F);
            p += 4 + n;
        }
        assert_eq!(kinds, [7, 8, 1]);
        assert_eq!(p, b.len());
        // avcC from the stream's parameter sets.
        assert_eq!(&v.config[..6], &[1, 66, 0xC0, 30, 0xFF, 0xE1]);
        let a = m.audio.as_ref().unwrap();
        // Stream type 3 says MPEG audio; the layer comes from frames (these hold none).
        assert_eq!(a.codec.name(), "MPEG audio");
        assert_eq!(m.duration_us, Some(10 * 3003 * 1_000_000 / 90_000));
    }

    #[test]
    fn bdav_packets_and_quick_parse() {
        let plain = stream(6, 1000);
        let mut bdav = Vec::new();
        for p in plain.chunks(188) {
            bdav.extend_from_slice(&[0, 0, 0, 0]);
            bdav.extend_from_slice(p);
        }
        assert!(sniff(&bdav));
        let m = parse(&bdav, true).unwrap();
        assert_eq!(m.video.as_ref().unwrap().samples.len(), 6);
        let q = parse(&bdav, false).unwrap();
        assert!(q.video.as_ref().unwrap().samples.is_empty());
        assert_eq!(q.duration_us, m.duration_us);
    }

    #[test]
    fn cut_and_garbled_streams_never_panic() {
        let data = stream(8, 0);
        for cut in (0..data.len()).step_by(61) {
            let _ = parse(&data[..cut], true);
        }
        let mut bad = data.clone();
        for i in (5..bad.len()).step_by(97) {
            bad[i] ^= 0x5A;
        }
        if let Ok(m) = parse(&bad, true)
            && let Some(v) = m.video.as_ref()
        {
            for s in &v.samples {
                let _ = v.sample_data(&bad, s);
            }
        }
    }

    #[test]
    fn camcorder_date() {
        let mut sei = vec![5, 21 + 15];
        sei.extend_from_slice(&MDPM_UUID);
        sei.extend_from_slice(b"MDPM");
        sei.push(3);
        sei.extend_from_slice(&[0x18, 0x00, 0x20, 0x13, 0x07]);
        sei.extend_from_slice(&[0x19, 0x04, 0x15, 0x30, 0x59]);
        sei.extend_from_slice(&[0xE0, 0x01, 0x08, 0, 0]);
        sei.push(0x80);
        let mut m = Movie::default();
        assert!(mdpm(&sei, &mut m));
        assert_eq!(m.date.as_deref(), Some("2013-07-04T15:30:59"));
        assert_eq!(m.make.as_deref(), Some("Sony"));
    }

    #[test]
    fn clock_jumps_keep_time_running() {
        let mut c = Clock::default();
        c.pcr(1000, false);
        c.pcr(4000, false);
        // A jump back of ten seconds: the times after it shift forward by as much.
        c.pcr(4000 - 900_000, false);
        assert_eq!(c.shift, 900_000);
        assert_eq!(
            unwrap(5000 - 900_000, c.last_pcr.unwrap() - c.shift) + c.shift,
            5000
        );
    }
}
