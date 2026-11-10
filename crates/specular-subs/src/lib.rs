//! Subtitles: own decoders for the text and picture formats movies carry or sit beside.
//!
//! - Text: SubRip (`.srt`), WebVTT (`.vtt`), ASS/SSA (`.ass`, `.ssa`), MicroDVD (`.sub`),
//!   LRC lyrics (`.lrc`), TTML/DFXP (`.ttml`, `.dfxp`), Scenarist closed captions (`.scc`),
//!   Matroska's
//!   `S_TEXT/UTF8`, `S_TEXT/ASS|SSA` and `S_TEXT/WEBVTT`, MP4 timed text (`tx3g`) and MP4
//!   WebVTT (`wvtt`).
//! - Pictures: Blu-ray PGS (`.sup`, `S_HDMV/PGS`) and DVD VobSub (`.idx` + `.sub`,
//!   `S_VOBSUB`).
//! - Closed captions carried in the video: CEA-608 ([`cea608`]) and CEA-708 ([`cea708`]).
//!
//! Every decoder turns its input into [`Cue`]s on one timeline (microseconds from the movie's
//! start). Text cues keep basic styling (bold, italic, underline, colour, size, alignment,
//! position); the caller lays them out with its own text stack (bidirectional text included).
//! ASS effects beyond that (animation, karaoke, clipping, drawings) are left out quietly: the
//! words still show, plainly.
//!
//! Text files without a byte-order mark are read as UTF-8 when they are valid UTF-8, else
//! their character set is guessed ([`charset`]); the caller can override the guess.

use std::path::Path;
use std::sync::Arc;

pub mod ass;
pub mod cea608;
pub mod cea708;
pub mod charset;
pub mod lrc;
pub mod markup;
pub mod microdvd;
pub mod mp4text;
pub mod pgs;
pub mod scc;
pub mod srt;
pub mod ttml;
pub mod vobsub;
pub mod vtt;
pub mod write;

/// A colour as straight RGBA.
pub type Rgba = [u8; 4];

/// A run of text with one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// The fill colour (`None`: the default, white).
    pub colour: Option<Rgba>,
    /// The outline colour (`None`: the default, black).
    pub outline: Option<Rgba>,
    /// Size relative to the default (1.0).
    pub scale: f32,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            bold: false,
            italic: false,
            underline: false,
            colour: None,
            outline: None,
            scale: 1.0,
        }
    }
}

/// A text cue: lines of styled runs, placed by alignment (numpad: 1 bottom left, 2 bottom
/// centre, …, 9 top right) or at a position.
#[derive(Clone, Debug, PartialEq)]
pub struct TextCue {
    pub lines: Vec<Vec<Span>>,
    pub align: u8,
    /// The anchor point as fractions of the picture (0..1), when the cue gives one.
    pub pos: Option<(f32, f32)>,
    /// Vertical margin as a fraction of the picture's height, when the cue gives one.
    pub margin_v: Option<f32>,
}

impl TextCue {
    /// The cue's words without styling, lines joined by `\n`.
    pub fn plain_text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines
            .iter()
            .all(|l| l.iter().all(|s| s.text.trim().is_empty()))
    }
}

/// A picture cue: an RGBA bitmap placed on a canvas (the video's size as the subtitle stream
/// gives it).
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub canvas: (u32, u32),
    pub rgba: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Text(TextCue),
    Bitmap(Bitmap),
}

/// One subtitle on the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start_us: i64,
    pub end_us: i64,
    pub body: Body,
}

impl Cue {
    pub fn text(&self) -> Option<&TextCue> {
        match &self.body {
            Body::Text(t) => Some(t),
            Body::Bitmap(_) => None,
        }
    }
}

/// A decoded subtitle track: its cues in start order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub cues: Vec<Cue>,
    /// Some styling or effects were left out (ASS animation, karaoke, drawings).
    pub simplified: bool,
}

impl Track {
    pub fn new(mut cues: Vec<Cue>) -> Track {
        cues.retain(|c| c.end_us > c.start_us);
        cues.sort_by_key(|c| (c.start_us, c.end_us));
        Track {
            cues,
            simplified: false,
        }
    }

    /// The cues showing at `t_us`.
    pub fn active(&self, t_us: i64) -> impl Iterator<Item = &Cue> {
        // Cues start in order; one may last long, so look back over all earlier starts.
        let end = self.cues.partition_point(|c| c.start_us <= t_us);
        self.cues[..end].iter().filter(move |c| c.end_us > t_us)
    }

    /// The last cue start before `t_us` (strictly), for "previous subtitle".
    pub fn prev_start(&self, t_us: i64) -> Option<i64> {
        let i = self.cues.partition_point(|c| c.start_us < t_us);
        self.cues[..i].last().map(|c| c.start_us)
    }

    /// The first cue start after `t_us` (strictly), for "next subtitle".
    pub fn next_start(&self, t_us: i64) -> Option<i64> {
        let i = self.cues.partition_point(|c| c.start_us <= t_us);
        self.cues.get(i).map(|c| c.start_us)
    }

    pub fn is_bitmap(&self) -> bool {
        self.cues
            .first()
            .is_some_and(|c| matches!(c.body, Body::Bitmap(_)))
    }
}

/// Why a subtitle could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a subtitle format we read (the name says which).
    Unsupported(String),
    /// Damaged or not what it says it is.
    Malformed(String),
    Io(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::Io(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Subtitle file kinds beside a movie, by extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Srt,
    Vtt,
    Ass,
    Ssa,
    /// PGS dumped to a file (`.sup`).
    Sup,
    /// VobSub: the `.idx` beside its `.sub`.
    Idx,
    /// `.sub`: MicroDVD text, or the pictures of a VobSub pair (MPEG program stream).
    Sub,
    /// LRC lyrics.
    Lrc,
    /// TTML or DFXP.
    Ttml,
    /// Scenarist closed captions (CEA-608 pairs).
    Scc,
}

impl FileKind {
    pub fn of(path: &Path) -> Option<FileKind> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "srt" => FileKind::Srt,
            "vtt" | "webvtt" => FileKind::Vtt,
            "ass" => FileKind::Ass,
            "ssa" => FileKind::Ssa,
            "sup" => FileKind::Sup,
            "idx" => FileKind::Idx,
            "sub" => FileKind::Sub,
            "lrc" => FileKind::Lrc,
            "ttml" | "dfxp" => FileKind::Ttml,
            "scc" => FileKind::Scc,
            _ => return None,
        })
    }

    pub fn is_text(self) -> bool {
        matches!(
            self,
            FileKind::Srt
                | FileKind::Vtt
                | FileKind::Ass
                | FileKind::Ssa
                | FileKind::Lrc
                | FileKind::Ttml
                | FileKind::Scc
        )
    }
}

/// Whether `data` is an MPEG program stream (a VobSub `.sub`): a pack header first.
pub fn is_vobsub_data(data: &[u8]) -> bool {
    data.starts_with(&[0, 0, 1, 0xBA])
}

/// The `.idx` beside a VobSub `.sub` (either case), when there is one.
pub fn idx_of(sub: &Path) -> Option<std::path::PathBuf> {
    ["idx", "IDX"]
        .iter()
        .map(|e| sub.with_extension(e))
        .find(|p| p.is_file())
}

/// What a text file was read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Read {
    /// The character set used (detected or forced).
    pub encoding: &'static encoding_rs::Encoding,
}

/// Reads a subtitle file. `encoding` forces a character set for text files; `hint` is a
/// language code (the UI's or the file name's) that breaks ties in detection.
pub fn read_file(
    path: &Path,
    encoding: Option<&'static encoding_rs::Encoding>,
    hint: Option<&str>,
) -> Result<(Track, Option<Read>)> {
    let kind =
        FileKind::of(path).ok_or_else(|| Error::Unsupported("not a subtitle file".into()))?;
    let bytes = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    if kind == FileKind::Idx {
        let sub = path.with_extension("sub");
        let sub = if sub.exists() {
            sub
        } else {
            path.with_extension("SUB")
        };
        let data = std::fs::read(&sub).map_err(|e| Error::Io(e.to_string()))?;
        let (text, enc) = charset::decode(&bytes, None, None);
        let _ = enc;
        return vobsub::from_files(&text, &data).map(|t| (t, None));
    }
    if kind == FileKind::Sup {
        return pgs::from_sup(&bytes).map(|t| (t, None));
    }
    if kind == FileKind::Sub && is_vobsub_data(&bytes) {
        let idx = idx_of(path).ok_or_else(|| {
            Error::Unsupported("a VobSub .sub needs the .idx file beside it".into())
        })?;
        let idx = std::fs::read(idx).map_err(|e| Error::Io(e.to_string()))?;
        let (text, _) = charset::decode(&idx, None, None);
        return vobsub::from_files(&text, &bytes).map(|t| (t, None));
    }
    let (text, enc) = charset::decode(&bytes, encoding, hint);
    let track = parse_text(kind, &text)?;
    Ok((track, Some(Read { encoding: enc })))
}

/// Parses a text subtitle file's contents.
pub fn parse_text(kind: FileKind, text: &str) -> Result<Track> {
    match kind {
        FileKind::Srt => {
            // A file named .srt that is WebVTT or ASS inside is read as what it is.
            if text.trim_start().starts_with("WEBVTT") {
                vtt::parse(text)
            } else if text.trim_start().starts_with("[Script Info]") {
                ass::parse(text)
            } else {
                srt::parse(text)
            }
        }
        FileKind::Vtt => vtt::parse(text),
        FileKind::Ass | FileKind::Ssa => ass::parse(text),
        FileKind::Sub => {
            if microdvd::sniff(text) {
                microdvd::parse(text, None)
            } else if text.trim_start().starts_with("[INFORMATION]") {
                Err(Error::Unsupported("SubViewer .sub files".into()))
            } else {
                Err(Error::Malformed("not a MicroDVD .sub file".into()))
            }
        }
        FileKind::Lrc => lrc::parse(text).map(|(t, _)| t),
        FileKind::Ttml => ttml::parse(text),
        FileKind::Scc => scc::parse(text, 1),
        FileKind::Sup | FileKind::Idx => Err(Error::Unsupported("not a text subtitle".into())),
    }
}

/// How long a text cue without a duration shows when nothing follows it.
const OPEN_TEXT_US: i64 = 5_000_000;

/// Decodes a movie's subtitle stream: `codec` is the Matroska CodecID or the MP4 sample
/// entry, `private` its codec data, `packets` (start, duration, bytes) in time order.
pub fn decode_stream<'a>(
    codec: &str,
    private: &[u8],
    packets: impl IntoIterator<Item = (i64, Option<i64>, &'a [u8])>,
) -> Result<Track> {
    let packets: Vec<(i64, Option<i64>, &[u8])> = packets.into_iter().collect();
    let end_of = |i: usize, start: i64, dur: Option<i64>| -> i64 {
        match dur.filter(|d| *d > 0) {
            Some(d) => start + d,
            None => packets
                .get(i + 1)
                .map(|p| p.0)
                .filter(|&n| n > start)
                .unwrap_or(start + OPEN_TEXT_US),
        }
    };
    let matroska_text = codec.starts_with("S_TEXT/") || codec.starts_with("D_WEBVTT/");
    let text = |f: &dyn Fn(&[u8], &mut bool) -> Vec<TextCue>| -> Track {
        let mut simplified = false;
        let mut cues = Vec::new();
        for (i, &(start, dur, data)) in packets.iter().enumerate() {
            // Matroska text blocks carry their duration; one without is a zero-length cue
            // (ffmpeg writes those as plain blocks): never shown.
            if dur.is_none() && matroska_text {
                continue;
            }
            let end = end_of(i, start, dur);
            for c in f(data, &mut simplified) {
                cues.push(Cue {
                    start_us: start,
                    end_us: end,
                    body: Body::Text(c),
                });
            }
        }
        let mut t = Track::new(cues);
        t.simplified = simplified;
        t
    };
    match codec {
        "S_TEXT/UTF8" | "S_TEXT/ASCII" => {
            Ok(text(&|d, _| mp4text::utf8_block(d).into_iter().collect()))
        }
        "S_TEXT/ASS" | "S_TEXT/SSA" | "S_ASS" | "S_SSA" => {
            let script = ass::Script::parse_header(&String::from_utf8_lossy(private));
            Ok(text(&|d, simp| {
                script
                    .mkv_block(&String::from_utf8_lossy(d), simp)
                    .into_iter()
                    .collect()
            }))
        }
        // ffmpeg writes WebVTT in Matroska under the older `D_WEBVTT/…` ids.
        "S_TEXT/WEBVTT" | "D_WEBVTT/SUBTITLES" | "D_WEBVTT/CAPTIONS" | "D_WEBVTT/DESCRIPTIONS" => {
            Ok(text(&|d, _| {
                let c = vtt::cue_text(&String::from_utf8_lossy(d), "");
                if c.is_empty() { Vec::new() } else { vec![c] }
            }))
        }
        "tx3g" | "text" => Ok(text(&|d, _| mp4text::tx3g(d).into_iter().collect())),
        "wvtt" => Ok(text(&|d, _| mp4text::wvtt(d))),
        "S_HDMV/PGS" => {
            let mut dec = pgs::Decoder::new();
            for &(start, _, data) in &packets {
                dec.block(data, start);
            }
            let last_end = packets.last().and_then(|p| p.1.map(|d| p.0 + d));
            Ok(dec.finish(last_end))
        }
        "S_VOBSUB" => {
            let h = vobsub::Header::parse(&String::from_utf8_lossy(private));
            let mut cues = Vec::new();
            for (i, &(start, dur, data)) in packets.iter().enumerate() {
                let next = dur
                    .filter(|d| *d > 0)
                    .map(|d| start + d)
                    .or_else(|| packets.get(i + 1).map(|p| p.0));
                if let Some(c) = vobsub::cue(data, &h, start, next) {
                    cues.push(c);
                }
            }
            Ok(Track::new(cues))
        }
        // Closed captions: `cc_data` triplets a frame; the codec data names the channel
        // (1..=4 CC1..CC4, 5..=8 T1..T4) or the service.
        CEA_608 => Ok(cea608::decode(
            private.first().copied().unwrap_or(1),
            packets.iter().map(|&(t, _, d)| (t, d)),
        )),
        CEA_708 => Ok(cea708::decode(
            private.first().copied().unwrap_or(1),
            packets.iter().map(|&(t, _, d)| (t, d)),
        )),
        other => Err(Error::Unsupported(other.to_owned())),
    }
}

/// The codec name of a CEA-608 caption stream (see [`decode_stream`]).
pub const CEA_608: &str = "CEA-608";
/// The codec name of a CEA-708 caption stream.
pub const CEA_708: &str = "CEA-708";

/// What a subtitle file's name says about it besides its language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileFlags {
    /// `.forced.`: only the parts in a foreign language.
    pub forced: bool,
    /// `.sdh.`, `.cc.` or `.hoh.`: for the deaf and hard of hearing (sounds described).
    pub sdh: bool,
}

/// The flags in a subtitle file's name: `Movie.en.forced.srt`, `Movie.en.sdh.srt`.
pub fn file_flags(sub: &Path) -> FileFlags {
    let name = sub
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut f = FileFlags::default();
    for part in name.split(['.', '_', ' ', '-', '[', ']', '(', ')']).skip(1) {
        match part {
            "forced" | "foreign" => f.forced = true,
            "sdh" | "cc" | "hoh" => f.sdh = true,
            _ => {}
        }
    }
    f
}

/// The language a subtitle file's name gives: `movie.he.srt` → `he`, `movie.pt-BR.srt` →
/// `pt-BR` (a code of two or three letters, optionally with a region), else `None`.
pub fn file_language(movie_stem: &str, sub: &Path) -> Option<String> {
    let name = sub.file_stem()?.to_str()?;
    let rest = name.strip_prefix(movie_stem)?.strip_prefix('.')?;
    // `movie.forced.en` or `movie.en.forced`: any part that looks like a language code.
    rest.split('.')
        .filter(|p| !matches!(p.to_ascii_lowercase().as_str(), "sdh" | "cc" | "hoh"))
        .find(|p| is_lang_code(p))
        .map(str::to_owned)
}

fn is_lang_code(p: &str) -> bool {
    let (lang, region) = match p.split_once(['-', '_']) {
        Some((l, r)) => (l, Some(r)),
        None => (p, None),
    };
    (2..=3).contains(&lang.len())
        && lang.chars().all(|c| c.is_ascii_alphabetic())
        && region.is_none_or(|r| {
            (2..=4).contains(&r.len()) && r.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

/// Subtitle files beside `movie`: the same base name with a subtitle extension, also
/// `name.<lang>.srt` and similar. VobSub's `.sub` is found through its `.idx`.
pub fn beside(movie: &Path) -> Vec<std::path::PathBuf> {
    let (Some(dir), Some(stem)) = (movie.parent(), movie.file_stem().and_then(|s| s.to_str()))
    else {
        return Vec::new();
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<std::path::PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| FileKind::of(p).is_some())
        // A VobSub pair is found through its `.idx`.
        .filter(|p| FileKind::of(p) != Some(FileKind::Sub) || idx_of(p).is_none())
        .filter(|p| {
            let Some(s) = p.file_stem().and_then(|s| s.to_str()) else {
                return false;
            };
            s == stem || s.strip_prefix(stem).is_some_and(|r| r.starts_with('.'))
        })
        .collect();
    out.sort();
    out
}

/// Microseconds from `h:m:s.frac` parts.
pub(crate) fn hms_us(h: i64, m: i64, s: i64, frac_us: i64) -> i64 {
    ((h * 60 + m) * 60 + s) * 1_000_000 + frac_us
}

/// A fraction of a second written with `digits` digits, in microseconds.
pub(crate) fn frac_us(digits: &str) -> Option<i64> {
    if digits.is_empty() {
        return Some(0);
    }
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let d = &digits[..digits.len().min(6)];
    let v: i64 = d.parse().ok()?;
    Some(v * 10i64.pow(6 - d.len() as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(a: i64, b: i64) -> Cue {
        Cue {
            start_us: a,
            end_us: b,
            body: Body::Text(TextCue {
                lines: vec![vec![Span::plain("x")]],
                align: 2,
                pos: None,
                margin_v: None,
            }),
        }
    }

    #[test]
    fn active_and_steps() {
        let t = Track::new(vec![cue(5, 10), cue(0, 100), cue(20, 30)]);
        assert_eq!(t.active(7).count(), 2);
        assert_eq!(t.active(15).count(), 1);
        assert_eq!(t.active(100).count(), 0);
        assert_eq!(t.next_start(5), Some(20));
        assert_eq!(t.prev_start(20), Some(5));
        assert_eq!(t.prev_start(0), None);
        assert_eq!(t.next_start(20), None);
    }

    #[test]
    fn movie_streams() {
        let t = decode_stream(
            "S_TEXT/UTF8",
            &[],
            [
                (0, Some(1000), &b"<i>a</i>"[..]),
                (2000, None, b"hidden"),
                (9000, Some(500), b"c"),
            ],
        )
        .unwrap();
        assert_eq!(t.cues.len(), 2, "a block without a duration is zero-length");
        assert_eq!(t.cues[0].end_us, 1000);
        assert_eq!(t.cues[1].end_us, 9500);
        let t = decode_stream(
            "tx3g",
            &[],
            [
                (0, None, &[0u8, 1, b'a'][..]),
                (2000, None, &[0, 1, b'b'][..]),
            ],
        )
        .unwrap();
        assert_eq!(t.cues[0].end_us, 2000, "until the next one");
        assert_eq!(t.cues[1].end_us, 2000 + OPEN_TEXT_US);
        let header = "[Script Info]\nPlayResY: 288\n\n[V4+ Styles]\nFormat: Name, Fontsize, Alignment\nStyle: Default,20,8\n";
        let t = decode_stream(
            "S_TEXT/ASS",
            header.as_bytes(),
            [(5, Some(10), &b"1,0,Default,,0,0,0,,Top"[..])],
        )
        .unwrap();
        assert_eq!(t.cues[0].text().unwrap().align, 8);
        let block = pgs::fixture::block(&pgs::fixture::show(720, 576, 1, 2, 3, 4));
        let clear = pgs::fixture::block(&pgs::fixture::clear(720, 576));
        let t = decode_stream(
            "S_HDMV/PGS",
            &[],
            [(100, None, &block[..]), (900, None, &clear[..])],
        )
        .unwrap();
        assert_eq!(
            (t.cues.len(), t.cues[0].start_us, t.cues[0].end_us),
            (1, 100, 900)
        );
        let spu = vobsub::fixture::spu(0, 0, 4, 2, 0);
        let t = decode_stream(
            "S_VOBSUB",
            b"size: 720x480\npalette: 000000, ff0000\n",
            [(50, Some(500), &spu[..])],
        )
        .unwrap();
        assert_eq!((t.cues[0].start_us, t.cues[0].end_us), (50, 550));
        let Body::Bitmap(b) = &t.cues[0].body else {
            panic!()
        };
        assert_eq!(&b.rgba[..4], &[255, 0, 0, 255]);
        assert!(matches!(
            decode_stream("S_DVBSUB", &[], []),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn languages_from_names() {
        let p = Path::new("/v/Movie.he.srt");
        assert_eq!(file_language("Movie", p).as_deref(), Some("he"));
        assert_eq!(
            file_language("Movie", Path::new("Movie.pt-BR.srt")).as_deref(),
            Some("pt-BR")
        );
        assert_eq!(
            file_language("Movie", Path::new("Movie.forced.eng.srt")).as_deref(),
            Some("eng")
        );
        assert_eq!(file_language("Movie", Path::new("Movie.srt")), None);
        assert_eq!(
            file_language("Movie", Path::new("Movie.sdh.en.srt")).as_deref(),
            Some("en")
        );
        assert_eq!(file_language("Movie", Path::new("Movie.cc.srt")), None);
        let f = file_flags(Path::new("/v/Movie.en.forced.srt"));
        assert!(f.forced && !f.sdh);
        let f = file_flags(Path::new("/v/Movie.en.SDH.srt"));
        assert!(!f.forced && f.sdh);
        assert!(file_flags(Path::new("/v/Movie.en.cc.srt")).sdh);
        assert_eq!(file_flags(Path::new("/v/sdh.srt")), FileFlags::default());
        assert_eq!(
            file_language("Movie", Path::new("Movie.director.srt")),
            None
        );
    }

    #[test]
    fn files_beside_a_movie() {
        let dir = std::env::temp_dir().join(format!("subs-beside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in [
            "Film.mkv",
            "Film.srt",
            "Film.he.srt",
            "Film.ar.ass",
            "Film2.srt",
            "Film.idx",
            "Film.sub",
            "Other.vtt",
        ] {
            std::fs::write(dir.join(n), b"").unwrap();
        }
        let found: Vec<String> = beside(&dir.join("Film.mkv"))
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            found,
            ["Film.ar.ass", "Film.he.srt", "Film.idx", "Film.srt"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
