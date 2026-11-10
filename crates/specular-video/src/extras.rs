//! What a movie holds besides the picture and the sound that plays: its sound tracks (to
//! choose one), its subtitle streams (raw packets, for `specular-subs` to decode) and its
//! chapters. Matroska and MP4/MOV; transport streams and WTV recordings list their sound
//! tracks; other containers have none here.
//!
//! - Matroska: `Tracks` (type, number, codec, language, name, default and forced flags;
//!   their stored coding undone by [`mkv::Coding`]), every block of the
//!   subtitle tracks, and `Chapters` (the default edition's top-level atoms with their names
//!   by language; hidden or disabled ones left out).
//! - MP4/MOV: `soun` tracks; `sbtl`/`subt`/`text` tracks with `tx3g`, `text` or `wvtt`
//!   samples; chapters from a `chap` track reference (QuickTime text or tx3g samples), else
//!   Nero's `chpl`. Accessibility flags from the track's `udta` (`kind` roles, Apple's
//!   `tagc` characteristics) and tx3g's "all samples forced".
//! - Closed captions in the video (any container; see [`crate::captions`]) as subtitle
//!   streams `CEA-608` and `CEA-708`, flagged for the deaf and hard of hearing.
//! - Track names that say so ("SDH", "Audio Description", "Forced") set the flags too.
//!
//! Times are microseconds on the movie's presentation timeline; `picture_start_us` is where
//! the player's time 0 (the first frame) lies on it.

use crate::mkv;
use crate::mp4::{self, Rd};

/// A sound or subtitle track's label.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackInfo {
    /// Matroska's track number, MP4's track ID or a transport stream's PID (what
    /// [`crate::parse_movie_with`] takes).
    pub id: u64,
    /// The codec: Matroska's CodecID (`S_TEXT/UTF8`, `A_AAC`) or the MP4 sample entry
    /// (`tx3g`, `mp4a`).
    pub codec: String,
    /// The language code as stored (ISO 639-2 or BCP 47), `None` when unset or `und`.
    pub lang: Option<String>,
    pub name: Option<String>,
    pub default: bool,
    pub forced: bool,
    /// For the deaf and hard of hearing: subtitles that describe sounds too (SDH), closed
    /// captions.
    pub hearing_impaired: bool,
    /// Audio description: sound that describes the picture for the blind and partially
    /// sighted.
    pub visual_impaired: bool,
    pub channels: u16,
}

impl TrackInfo {
    /// Sets the flags a track name spells out ("English SDH", "Audio Description").
    fn flags_from_name(&mut self) {
        let Some(n) = self.name.as_deref() else {
            return;
        };
        let n = n.to_lowercase();
        let words: Vec<&str> = n
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let has = |w: &str| words.contains(&w);
        if has("sdh")
            || has("cc")
            || n.contains("hearing impaired")
            || n.contains("hard of hearing")
            || n.contains("for the deaf")
            || n.contains("closed caption")
        {
            self.hearing_impaired = true;
        }
        if has("ad")
            || has("dvs")
            || n.contains("audio description")
            || n.contains("audio-description")
            || n.contains("audiodescription")
            || n.contains("described video")
            || n.contains("descriptive")
            || n.contains("visually impaired")
        {
            self.visual_impaired = true;
        }
        if has("forced") {
            self.forced = true;
        }
    }
}

/// One subtitle packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub start_us: i64,
    pub dur_us: Option<i64>,
    pub data: Vec<u8>,
}

/// A subtitle stream: its label, codec data and packets in time order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubStream {
    pub info: TrackInfo,
    pub private: Vec<u8>,
    pub packets: Vec<Packet>,
}

/// A chapter: where it starts (and ends, when stored) and its names by language.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chapter {
    pub start_us: i64,
    pub end_us: Option<i64>,
    /// (language, name); the language empty when unknown.
    pub names: Vec<(String, String)>,
}

impl Chapter {
    /// The name in `lang` (a language code prefix match), else the first.
    pub fn name(&self, lang: &str) -> Option<&str> {
        let base = lang.split(['-', '_']).next().unwrap_or(lang);
        self.names
            .iter()
            .find(|(l, _)| !base.is_empty() && lang_matches(l, base))
            .or(self.names.first())
            .map(|(_, n)| n.as_str())
    }
}

/// Whether a stored language code (`eng`, `en`, `en-US`, `heb`, `iw`) is the two-letter
/// `base`.
pub fn lang_matches(code: &str, base: &str) -> bool {
    let c = code
        .split(['-', '_'])
        .next()
        .unwrap_or(code)
        .to_ascii_lowercase();
    let b = base.to_ascii_lowercase();
    c == b || iso639_2(&c).is_some_and(|two| two == b)
}

/// ISO 639-2 (B and T) codes of the languages subtitles and dubs most often come in, as their
/// two-letter codes.
pub fn iso639_2(code: &str) -> Option<&'static str> {
    Some(match code {
        "eng" => "en",
        "heb" | "iw" => "he",
        "ara" => "ar",
        "fre" | "fra" => "fr",
        "ger" | "deu" => "de",
        "spa" => "es",
        "ita" => "it",
        "por" => "pt",
        "dut" | "nld" => "nl",
        "rus" => "ru",
        "ukr" => "uk",
        "pol" => "pl",
        "cze" | "ces" => "cs",
        "slo" | "slk" => "sk",
        "hun" => "hu",
        "rum" | "ron" => "ro",
        "gre" | "ell" => "el",
        "tur" => "tr",
        "swe" => "sv",
        "dan" => "da",
        "nor" | "nob" => "no",
        "fin" => "fi",
        "jpn" => "ja",
        "chi" | "zho" => "zh",
        "kor" => "ko",
        "hin" => "hi",
        "tha" => "th",
        "vie" => "vi",
        "ind" => "id",
        "per" | "fas" => "fa",
        "bul" => "bg",
        "hrv" => "hr",
        "srp" => "sr",
        "slv" => "sl",
        "cat" => "ca",
        "est" => "et",
        "lav" => "lv",
        "lit" => "lt",
        _ => return None,
    })
}

/// Everything [`read`] finds.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub audio: Vec<TrackInfo>,
    pub subtitles: Vec<SubStream>,
    pub chapters: Vec<Chapter>,
    /// The first frame's time on the presentation timeline (the player's time 0).
    pub picture_start_us: i64,
}

/// Reads a movie's sound tracks, subtitle streams and chapters.
pub fn read(data: &[u8]) -> Extras {
    let (mut x, captions) = if mkv::sniff(data) {
        (read_mkv(data), None)
    } else if looks_mp4(data) {
        read_mp4(data)
    } else if crate::ts::sniff(data) {
        let x = Extras {
            audio: crate::ts::audio_tracks(data),
            ..Extras::default()
        };
        (x, None)
    } else if crate::wtv::sniff(data) {
        let x = Extras {
            audio: crate::wtv::audio_tracks(data),
            ..Extras::default()
        };
        (x, None)
    } else {
        (Extras::default(), None)
    };
    let movie = crate::parse_movie(data, true).ok();
    x.picture_start_us = movie.as_ref().map_or(0, |m| picture_start(data, m));
    for t in x.audio.iter_mut() {
        t.flags_from_name();
    }
    for s in x.subtitles.iter_mut() {
        s.info.flags_from_name();
    }
    // Closed captions: an MP4 caption track's, else those in the video.
    let cc = match captions {
        Some(frames) => crate::captions::streams(frames, &[]),
        None => movie.as_ref().map_or_else(Vec::new, |m| {
            let langs = if crate::ts::sniff(data) {
                crate::ts::caption_services(data)
            } else {
                Vec::new()
            };
            crate::captions::from_video(data, m, &langs)
        }),
    };
    x.subtitles.extend(cc);
    x
}

fn looks_mp4(data: &[u8]) -> bool {
    data.get(4..8)
        .is_some_and(|k| matches!(k, b"ftyp" | b"moov" | b"mdat" | b"wide" | b"free" | b"skip"))
}

/// Where the player's time 0 lies on the presentation timeline (as `open_sound` times it).
fn picture_start(data: &[u8], movie: &crate::mp4::Movie) -> i64 {
    movie.video.as_ref().map_or(0, |v| {
        let order = crate::play::plain_timeline_in(data, v);
        crate::play::presented_at_us(v, order.first().copied().unwrap_or(0))
    })
}

// ---- Matroska ----

const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2AD7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_TYPE: u32 = 0x83;
const FLAG_ENABLED: u32 = 0xB9;
const FLAG_DEFAULT: u32 = 0x88;
const FLAG_FORCED: u32 = 0x55AA;
const FLAG_HEARING_IMPAIRED: u32 = 0x55AB;
const FLAG_VISUAL_IMPAIRED: u32 = 0x55AC;
const NAME: u32 = 0x536E;
const LANGUAGE: u32 = 0x22B59C;
const LANGUAGE_BCP47: u32 = 0x22B59D;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const AUDIO: u32 = 0xE1;
const CHANNELS: u32 = 0x9F;
const CONTENT_ENCODINGS: u32 = 0x6D80;
const CLUSTER: u32 = 0x1F43_B675;
const CHAPTERS: u32 = 0x1043_A770;
const EDITION_ENTRY: u32 = 0x45B9;
const EDITION_FLAG_DEFAULT: u32 = 0x45DB;
const EDITION_FLAG_HIDDEN: u32 = 0x45BD;
const CHAPTER_ATOM: u32 = 0xB6;
const CHAPTER_TIME_START: u32 = 0x91;
const CHAPTER_TIME_END: u32 = 0x92;
const CHAPTER_FLAG_HIDDEN: u32 = 0x98;
const CHAPTER_FLAG_ENABLED: u32 = 0x4598;
const CHAPTER_DISPLAY: u32 = 0x80;
const CHAP_STRING: u32 = 0x85;
const CHAP_LANGUAGE: u32 = 0x437C;
const CHAP_LANGUAGE_BCP47: u32 = 0x437D;

struct MkvTrack {
    kind: u64,
    info: TrackInfo,
    private: Vec<u8>,
    coding: mkv::Coding,
}

fn mkv_track(data: &[u8], e: &mkv::El) -> MkvTrack {
    let mut t = MkvTrack {
        kind: 0,
        info: TrackInfo {
            default: true,
            channels: 1,
            ..TrackInfo::default()
        },
        private: Vec::new(),
        coding: mkv::Coding::default(),
    };
    let mut enabled = true;
    let mut bcp47 = None;
    for c in mkv::children(data, e.body, e.end) {
        match c.id {
            TRACK_NUMBER => t.info.id = mkv::uint(data, &c).unwrap_or(0),
            TRACK_TYPE => t.kind = mkv::uint(data, &c).unwrap_or(0),
            FLAG_ENABLED => enabled = mkv::uint(data, &c) != Some(0),
            FLAG_DEFAULT => t.info.default = mkv::uint(data, &c) != Some(0),
            FLAG_FORCED => t.info.forced = mkv::uint(data, &c).is_some_and(|v| v != 0),
            FLAG_HEARING_IMPAIRED => {
                t.info.hearing_impaired = mkv::uint(data, &c).is_some_and(|v| v != 0)
            }
            FLAG_VISUAL_IMPAIRED => {
                t.info.visual_impaired = mkv::uint(data, &c).is_some_and(|v| v != 0)
            }
            NAME => t.info.name = mkv::string(data, &c).filter(|s| !s.is_empty()),
            LANGUAGE => t.info.lang = mkv::string(data, &c),
            LANGUAGE_BCP47 => bcp47 = mkv::string(data, &c),
            CODEC_ID => t.info.codec = mkv::string(data, &c).unwrap_or_default(),
            CODEC_PRIVATE => t.private = data.get(c.body..c.end).unwrap_or_default().to_vec(),
            AUDIO => {
                for a in mkv::children(data, c.body, c.end) {
                    if a.id == CHANNELS {
                        t.info.channels = mkv::uint(data, &a).unwrap_or(1).clamp(1, 64) as u16;
                    }
                }
            }
            CONTENT_ENCODINGS => t.coding = mkv::Coding::read(data, &c),
            _ => {}
        }
    }
    if let Some(b) = bcp47.filter(|s| !s.is_empty()) {
        t.info.lang = Some(b);
    }
    if t.info.lang.as_deref() == Some("und") {
        t.info.lang = None;
    }
    if !enabled {
        t.kind = 0;
    }
    t
}

fn read_mkv(data: &[u8]) -> Extras {
    let mut x = Extras::default();
    let Some(header) = mkv::element(data, 0) else {
        return x;
    };
    let mut p = header.end;
    let seg = loop {
        let Some(e) = mkv::element(data, p) else {
            return x;
        };
        if e.id == SEGMENT {
            break e;
        }
        if e.unknown || e.end <= p {
            return x;
        }
        p = e.end;
    };
    let seg_end = seg.end.min(data.len());
    let mut scale_ns: u64 = 1_000_000;
    let mut tracks: Vec<MkvTrack> = Vec::new();
    let mut clusters: Vec<mkv::El> = Vec::new();
    let mut tracks_seen = false;
    let mut p = seg.body;
    while p < seg_end {
        let Some(e) = mkv::element(data, p) else {
            break;
        };
        match e.id {
            INFO => {
                for c in mkv::children(data, e.body, e.end) {
                    if c.id == TIMESTAMP_SCALE {
                        scale_ns = mkv::uint(data, &c).filter(|&s| s > 0).unwrap_or(1_000_000);
                    }
                }
            }
            TRACKS if !tracks_seen => {
                tracks_seen = true;
                tracks = mkv::children(data, e.body, e.end)
                    .filter(|c| c.id == TRACK_ENTRY)
                    .map(|c| mkv_track(data, &c))
                    .collect();
            }
            CHAPTERS if x.chapters.is_empty() => x.chapters = mkv_chapters(data, &e),
            CLUSTER => {
                clusters.push(e);
                if e.unknown {
                    let mut scratch = Vec::new();
                    p = mkv::cluster(data, e.body, e.end, true, &[], &mut scratch);
                    continue;
                }
            }
            _ => {}
        }
        if e.unknown || e.end <= p {
            break;
        }
        p = e.end;
    }
    x.audio = tracks
        .iter()
        .filter(|t| t.kind == 2 && t.info.id > 0)
        .map(|t| t.info.clone())
        .collect();
    let subs: Vec<&MkvTrack> = tracks
        .iter()
        .filter(|t| t.kind == 0x11 && t.info.id > 0 && t.coding.refusal().is_none())
        .collect();
    if subs.is_empty() {
        return x;
    }
    let want: Vec<u64> = subs.iter().map(|t| t.info.id).collect();
    let mut frames = Vec::new();
    for e in &clusters {
        mkv::cluster(data, e.body, e.end, e.unknown, &want, &mut frames);
    }
    let us = |ticks: i64| -> i64 {
        (i128::from(ticks) * i128::from(scale_ns) / 1000).clamp(i64::MIN as i128, i64::MAX as i128)
            as i64
    };
    for t in subs {
        let mut s = SubStream {
            info: t.info.clone(),
            private: t
                .coding
                .private(&t.private)
                .map_or_else(|| t.private.clone(), |p| p.into_owned()),
            packets: Vec::new(),
        };
        for (n, f) in &frames {
            if *n != t.info.id {
                continue;
            }
            let Some(raw) = data.get(f.offset..f.offset + f.size) else {
                continue;
            };
            let Some(bytes) = t.coding.frame(raw).map(|b| b.into_owned()) else {
                continue;
            };
            s.packets.push(Packet {
                start_us: us(f.ticks),
                dur_us: f.duration.map(|d| us(d.min(i64::MAX as u64) as i64)),
                data: bytes,
            });
        }
        s.packets.sort_by_key(|p| p.start_us);
        x.subtitles.push(s);
    }
    x
}

fn mkv_chapters(data: &[u8], e: &mkv::El) -> Vec<Chapter> {
    // The default edition (else the first that is not hidden).
    let editions: Vec<mkv::El> = mkv::children(data, e.body, e.end)
        .filter(|c| c.id == EDITION_ENTRY)
        .collect();
    let flag = |ed: &mkv::El, id: u32| {
        mkv::children(data, ed.body, ed.end)
            .find(|c| c.id == id)
            .and_then(|c| mkv::uint(data, &c))
            .is_some_and(|v| v != 0)
    };
    let ed = editions
        .iter()
        .find(|ed| flag(ed, EDITION_FLAG_DEFAULT))
        .or_else(|| editions.iter().find(|ed| !flag(ed, EDITION_FLAG_HIDDEN)))
        .or(editions.first());
    let Some(ed) = ed else { return Vec::new() };
    let mut out = Vec::new();
    for a in mkv::children(data, ed.body, ed.end).filter(|c| c.id == CHAPTER_ATOM) {
        let mut ch = Chapter::default();
        let (mut hidden, mut enabled) = (false, true);
        for c in mkv::children(data, a.body, a.end) {
            match c.id {
                // Chapter times are nanoseconds, not scaled ticks.
                CHAPTER_TIME_START => {
                    ch.start_us = (mkv::uint(data, &c).unwrap_or(0) / 1000) as i64
                }
                CHAPTER_TIME_END => ch.end_us = mkv::uint(data, &c).map(|v| (v / 1000) as i64),
                CHAPTER_FLAG_HIDDEN => hidden = mkv::uint(data, &c).is_some_and(|v| v != 0),
                CHAPTER_FLAG_ENABLED => enabled = mkv::uint(data, &c) != Some(0),
                CHAPTER_DISPLAY => {
                    let (mut name, mut lang, mut bcp) = (None, None, None);
                    for d in mkv::children(data, c.body, c.end) {
                        match d.id {
                            CHAP_STRING => name = mkv::string(data, &d),
                            CHAP_LANGUAGE => lang = mkv::string(data, &d),
                            CHAP_LANGUAGE_BCP47 => bcp = mkv::string(data, &d),
                            _ => {}
                        }
                    }
                    if let Some(n) = name.filter(|n| !n.is_empty()) {
                        ch.names.push((bcp.or(lang).unwrap_or_default(), n));
                    }
                }
                _ => {}
            }
        }
        if !hidden && enabled {
            out.push(ch);
        }
    }
    out.sort_by_key(|c| c.start_us);
    out
}

// ---- MP4 / QuickTime ----

/// The packed ISO 639-2 language of `mdhd` (`und` → `None`).
fn mdhd_lang(mdhd: &[u8]) -> Option<String> {
    let r = Rd(mdhd);
    let at = if r.u8(0).ok()? == 1 { 32 } else { 20 };
    let v = r.u16(at).ok()?;
    if v < 0x400 {
        // A Macintosh language code (QuickTime): 0 is English.
        return (v == 0).then(|| "eng".to_owned());
    }
    let s: String = [(v >> 10) & 31, (v >> 5) & 31, v & 31]
        .iter()
        .map(|&c| char::from(0x60 + c as u8))
        .collect();
    (s != "und" && s.chars().all(|c| c.is_ascii_lowercase())).then_some(s)
}

/// The track's `udta/name` (QuickTime), if any.
fn udta_name(trak: &[u8]) -> Option<String> {
    let udta = mp4::child(trak, b"udta").ok()??;
    let name = mp4::child(udta, b"name").ok()??;
    let s = String::from_utf8_lossy(name)
        .trim_matches(char::from(0))
        .trim()
        .to_owned();
    (!s.is_empty()).then_some(s)
}

/// A track's accessibility flags from its `udta`: (forced, hearing impaired, visual
/// impaired).
fn udta_flags(trak: &[u8]) -> (bool, bool, bool) {
    let mut f = (false, false, false);
    let Some(udta) = mp4::child(trak, b"udta").ok().flatten() else {
        return f;
    };
    for b in mp4::boxes(udta).flatten() {
        let text = match &b.kind {
            // Apple's media characteristics: one string a box.
            b"tagc" => String::from_utf8_lossy(b.body).to_ascii_lowercase(),
            // ISO kind: a full box, a scheme URI then a value (both null-terminated).
            b"kind" => String::from_utf8_lossy(b.body.get(4..).unwrap_or_default())
                .to_ascii_lowercase()
                .replace('\0', " "),
            _ => continue,
        };
        let has = |w: &str| text.split([' ', '\0']).any(|t| t.trim() == w);
        if text.contains("public.subtitles.forced-only") || has("forced-subtitle") {
            f.0 = true;
        }
        if text.contains("public.accessibility.describes-music-and-sound")
            || has("caption")
            || has("captions")
        {
            f.1 = true;
        }
        if text.contains("public.accessibility.describes-video")
            || has("description")
            || has("descriptions")
            || has("main-desc")
        {
            f.2 = true;
        }
    }
    f
}

struct Mp4Trak<'a> {
    id: u32,
    /// Forced, hearing impaired, visual impaired (see [`udta_flags`]).
    flags: (bool, bool, bool),
    handler: [u8; 4],
    entry: [u8; 4],
    lang: Option<String>,
    name: Option<String>,
    enabled: bool,
    timescale: u32,
    edit: mp4::Edit,
    stbl: Option<&'a [u8]>,
    stsd: Option<&'a [u8]>,
    chap: Vec<u32>,
}

fn mp4_trak<'a>(body: &'a [u8], movie_ts: u32) -> Option<Mp4Trak<'a>> {
    let mdia = mp4::child(body, b"mdia").ok()??;
    let hdlr = mp4::child(mdia, b"hdlr").ok()??;
    let handler = hdlr.get(8..12)?.try_into().ok()?;
    let mdhd = mp4::child(mdia, b"mdhd").ok()??;
    let r = Rd(mdhd);
    let timescale = if r.u8(0).ok()? == 1 {
        r.u32(20).ok()?
    } else {
        r.u32(12).ok()?
    };
    let stbl = mp4::child(mdia, b"minf")
        .ok()
        .flatten()
        .and_then(|m| mp4::child(m, b"stbl").ok().flatten());
    let stsd = stbl.and_then(|s| mp4::child(s, b"stsd").ok().flatten());
    let entry = stsd
        .and_then(|s| s.get(12..16))
        .and_then(|e| e.try_into().ok())
        .unwrap_or([0; 4]);
    let enabled = mp4::child(body, b"tkhd")
        .ok()
        .flatten()
        .and_then(|t| t.get(3).copied())
        .is_none_or(|f| f & 1 != 0);
    let chap = mp4::child(body, b"tref")
        .ok()
        .flatten()
        .and_then(|t| mp4::child(t, b"chap").ok().flatten())
        .map(|c| {
            c.chunks_exact(4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                .collect()
        })
        .unwrap_or_default();
    let mut flags = udta_flags(body);
    if &entry == b"tx3g" {
        // tx3g display flags: all samples forced.
        let display = stsd
            .and_then(|s| s.get(24..28))
            .map_or(0, |d| u32::from_be_bytes(d.try_into().unwrap_or_default()));
        flags.0 |= display & 0x8000_0000 != 0;
    }
    Some(Mp4Trak {
        id: mp4::track_id(body)?,
        flags,
        handler,
        entry,
        lang: mdhd_lang(mdhd),
        name: udta_name(body),
        enabled,
        timescale,
        edit: mp4::edit_list(body, movie_ts).unwrap_or_default(),
        stbl,
        stsd,
        chap,
    })
}

/// A track's samples as (presented start, duration, bytes).
fn mp4_samples(data: &[u8], t: &Mp4Trak) -> Vec<(i64, i64, Vec<u8>)> {
    let Some(stbl) = t.stbl else {
        return Vec::new();
    };
    let mut tables = mp4::Tables {
        stts: None,
        ctts: None,
        stss: None,
        stsz: None,
        stsc: None,
        stco: None,
    };
    for b in mp4::boxes(stbl).flatten() {
        match &b.kind {
            b"stts" => tables.stts = Some(b.body),
            b"stsz" => tables.stsz = Some((b.body, false)),
            b"stz2" => tables.stsz = Some((b.body, true)),
            b"stsc" => tables.stsc = Some(b.body),
            b"stco" => tables.stco = Some((b.body, false)),
            b"co64" => tables.stco = Some((b.body, true)),
            _ => {}
        }
    }
    let Some(n) = tables.stsz.and_then(|(z, _)| Rd(z).u32(8).ok()) else {
        return Vec::new();
    };
    let n = (n as usize).min(200_000);
    let Ok(samples) = tables.expand(n, data.len() as u64) else {
        return Vec::new();
    };
    let ts = i128::from(t.timescale.max(1));
    let us = |v: i64| (i128::from(v) * 1_000_000 / ts) as i64;
    let delay = i64::try_from(t.edit.delay_us).unwrap_or(0);
    samples
        .iter()
        .filter_map(|s| {
            let b = mp4::sample_bytes(data, s)?;
            Some((
                delay + us(s.dts as i64 - t.edit.start),
                us(i64::from(s.duration)),
                b.to_vec(),
            ))
        })
        .collect()
}

/// The tracks, subtitles and chapters, and the caption data of closed caption tracks
/// (`c608`/`c708`), if any.
fn read_mp4(data: &[u8]) -> (Extras, Option<Vec<crate::captions::Frame>>) {
    let mut x = Extras::default();
    let mut captions: Option<Vec<crate::captions::Frame>> = None;
    let mut moov = None;
    for b in mp4::boxes(data) {
        let Ok(b) = b else { break };
        if &b.kind == b"moov" {
            moov = Some(b.body);
            break;
        }
    }
    let Some(moov) = moov else {
        return (x, None);
    };
    let movie_ts = mp4::child(moov, b"mvhd")
        .ok()
        .flatten()
        .and_then(|m| {
            let r = Rd(m);
            if r.u8(0).ok()? == 1 {
                r.u32(20).ok()
            } else {
                r.u32(12).ok()
            }
        })
        .unwrap_or(600);
    let traks: Vec<Mp4Trak> = mp4::boxes(moov)
        .flatten()
        .filter(|b| &b.kind == b"trak")
        .filter_map(|b| mp4_trak(b.body, movie_ts))
        .collect();
    let chapter_ids: Vec<u32> = traks.iter().flat_map(|t| t.chap.iter().copied()).collect();
    let mut first_audio = true;
    for t in &traks {
        if &t.handler == b"soun" {
            let channels = t
                .stsd
                .and_then(|s| mp4::audio_entry(s).ok().flatten())
                .map_or(2, |a| a.channels);
            x.audio.push(TrackInfo {
                id: u64::from(t.id),
                codec: String::from_utf8_lossy(&t.entry).into_owned(),
                lang: t.lang.clone(),
                name: t.name.clone(),
                default: first_audio && t.enabled,
                visual_impaired: t.flags.2,
                hearing_impaired: t.flags.1,
                channels,
                ..TrackInfo::default()
            });
            first_audio = false;
            continue;
        }
        if &t.handler == b"clcp" && matches!(&t.entry, b"c608" | b"c708") {
            let frames = captions.get_or_insert_with(Vec::new);
            for (start, dur, b) in mp4_samples(data, t) {
                crate::captions::from_mp4_sample(&t.entry, &b, start, dur, frames);
            }
            continue;
        }
        let text = matches!(&t.handler, b"sbtl" | b"subt" | b"text")
            && matches!(&t.entry, b"tx3g" | b"text" | b"wvtt");
        if !text {
            continue;
        }
        let samples = mp4_samples(data, t);
        if chapter_ids.contains(&t.id) {
            if x.chapters.is_empty() {
                x.chapters = samples
                    .iter()
                    .filter_map(|(start, dur, b)| {
                        let name = text_sample(b)?;
                        Some(Chapter {
                            start_us: *start,
                            end_us: Some(start + dur),
                            names: vec![(t.lang.clone().unwrap_or_default(), name)],
                        })
                    })
                    .collect();
            }
            continue;
        }
        let codec = if &t.entry == b"wvtt" { "wvtt" } else { "tx3g" };
        x.subtitles.push(SubStream {
            info: TrackInfo {
                id: u64::from(t.id),
                codec: codec.to_owned(),
                lang: t.lang.clone(),
                name: t.name.clone(),
                forced: t.flags.0,
                hearing_impaired: t.flags.1,
                visual_impaired: t.flags.2,
                ..TrackInfo::default()
            },
            private: Vec::new(),
            packets: samples
                .into_iter()
                .map(|(start, dur, b)| Packet {
                    start_us: start,
                    dur_us: Some(dur),
                    data: b,
                })
                .collect(),
        });
    }
    if x.chapters.is_empty() {
        x.chapters = nero_chapters(moov);
    }
    (x, captions)
}

/// A text sample's string (a 16-bit length, then the text).
fn text_sample(sample: &[u8]) -> Option<String> {
    let n = usize::from(u16::from_be_bytes([*sample.first()?, *sample.get(1)?]));
    let b = sample.get(2..2 + n)?;
    let s = String::from_utf8_lossy(b).trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// Nero's chapter list (`moov/udta/chpl`): start times in 100 ns units and names.
fn nero_chapters(moov: &[u8]) -> Vec<Chapter> {
    let Some(chpl) = mp4::child(moov, b"udta")
        .ok()
        .flatten()
        .and_then(|u| mp4::child(u, b"chpl").ok().flatten())
    else {
        return Vec::new();
    };
    let mut p = 4;
    if chpl.first().copied().unwrap_or(0) == 1 {
        p += 4;
    }
    let Some(&n) = chpl.get(p) else {
        return Vec::new();
    };
    p += 1;
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(t) = chpl.get(p..p + 8) else { break };
        let start = u64::from_be_bytes(t.try_into().unwrap_or_default());
        let Some(&len) = chpl.get(p + 8) else { break };
        let Some(name) = chpl.get(p + 9..p + 9 + usize::from(len)) else {
            break;
        };
        out.push(Chapter {
            start_us: (start / 10) as i64,
            end_us: None,
            names: vec![(String::new(), String::from_utf8_lossy(name).into_owned())],
        });
        p += 9 + usize::from(len);
    }
    out.sort_by_key(|c| c.start_us);
    out
}

#[cfg(test)]
mod tests;
