//! A sound file's tags and cover picture: ID3v2.2/2.3/2.4 (MP3, AAC, FLAC, DTS and AC-3
//! files, WAVE and AIFF chunks), APEv2 and APEv1 at the end (MP3, AAC; Monkey's Audio,
//! Musepack and WavPack write them too), ID3v1 at the end, Vorbis comments (FLAC, Ogg
//! Vorbis, Opus and FLAC), FLAC picture blocks, iTunes-style MP4 items
//! (`moov/udta/meta/ilst`), Matroska tags and cover attachments (MKA), RIFF `LIST/INFO` and
//! AIFF text chunks.
//!
//! Fields come out under stable keys in display order ([`KEYS`]); the first source to give
//! a field wins (ID3v2, then the container's, then APE, then ID3v1). Written from the
//! formats' public descriptions: the id3.org informal standards, the APEv2 specification on
//! the Hydrogenaudio wiki, the Xiph Vorbis I and FLAC format pages, Apple's QuickTime File
//! Format metadata chapter, the Matroska specification's tagging and attachment pages,
//! Microsoft's RIFF and Apple's AIFF-C documents.

/// The keys of [`Tags::fields`], in the order they are shown.
pub const KEYS: &[&str] = &[
    "title",
    "artist",
    "album",
    "album-artist",
    "composer",
    "genre",
    "date",
    "track",
    "disc",
    "comment",
    "copyright",
    "encoder",
    "track-gain",
    "album-gain",
    // MIDI songs and instrument banks (`crate::synth_sound::facts`).
    "midi",
    "instruments",
    "samples",
];

/// The largest cover picture read.
const MAX_COVER: usize = 16 << 20;
/// The longest field kept (lyrics and comments can be long).
const MAX_TEXT: usize = 4096;

/// A cover picture: its MIME type (as tagged, or from its signature) and bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cover {
    pub mime: String,
    pub data: Vec<u8>,
}

/// What a sound file's tags say.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tags {
    /// (key from [`KEYS`], value), in [`KEYS`] order.
    pub fields: Vec<(&'static str, String)>,
    /// The kinds of tag found ("ID3v2.4", "Vorbis comments", "MP4 items"...).
    pub kinds: Vec<&'static str>,
    /// The front cover, else the first picture.
    pub cover: Option<Cover>,
    /// The cover's picture type is "front cover" (ID3 and FLAC type 3).
    cover_front: bool,
    /// Volume levelling's tags (ReplayGain, R128, iTunNORM), from any tag kind.
    pub gain: crate::levelling::GainTags,
    /// Lyrics as text (ID3 USLT, Vorbis `LYRICS`/`UNSYNCEDLYRICS`, MP4 `©lyr`, APE and
    /// Matroska `LYRICS`); it may be LRC.
    pub lyrics: Option<String>,
    /// An ID3 SYLT frame's body (timed lyrics).
    pub synced_lyrics: Option<Vec<u8>>,
}

impl Tags {
    /// The value of `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    fn set(&mut self, key: &'static str, value: &str) {
        let v = clean(value);
        if v.is_empty() || self.get(key).is_some() {
            return;
        }
        let Some(rank) = KEYS.iter().position(|k| *k == key) else {
            return;
        };
        let at = self
            .fields
            .iter()
            .position(|(k, _)| KEYS.iter().position(|x| x == k).unwrap_or(usize::MAX) > rank)
            .unwrap_or(self.fields.len());
        self.fields.insert(at, (key, v));
    }

    /// A field that may be one of levelling's (any tag kind's key, upper or lower case):
    /// kept in [`Tags::gain`], and shown as the track's and album's gain.
    fn gain_field(&mut self, key: &str, value: &str) {
        let had = self.gain;
        self.gain.field(key, value);
        let db = |g: f32| format!("{g:+.2} dB").replace('-', "\u{2212}");
        if had.track_gain.is_none()
            && let Some(g) = self.gain.track_gain
        {
            self.set("track-gain", &db(g));
        }
        if had.album_gain.is_none()
            && let Some(g) = self.gain.album_gain
        {
            self.set("album-gain", &db(g));
        }
    }

    /// The lyrics: timed ones (SYLT, or text in LRC) first, else plain text.
    pub fn lyrics(&self) -> Option<crate::lyrics::Lyrics> {
        use crate::lyrics::{From, Lyrics, parse_sylt};
        let text = self
            .lyrics
            .as_deref()
            .and_then(|t| Lyrics::from_text(t, From::Tags));
        if text.as_ref().is_some_and(|l| l.synced) {
            return text;
        }
        self.synced_lyrics
            .as_deref()
            .and_then(parse_sylt)
            .filter(|l| !l.lines.is_empty())
            .or(text)
    }

    /// Keeps the first lyrics text found.
    fn set_lyrics(&mut self, v: &str) {
        if self.lyrics.is_some() {
            return;
        }
        let v = v.trim_matches(['\0', ' ', '\n', '\r', '\t']);
        if v.is_empty() {
            return;
        }
        let mut end = v.len().min(crate::lyrics::MAX_BYTES);
        while !v.is_char_boundary(end) {
            end -= 1;
        }
        self.lyrics = Some(v[..end].replace("\r\n", "\n").replace('\r', "\n"));
    }

    fn kind(&mut self, k: &'static str) {
        if !self.kinds.contains(&k) {
            self.kinds.push(k);
        }
    }

    /// Keeps a picture: a front cover replaces any other.
    fn picture(&mut self, mime: &str, data: &[u8], front: bool) {
        if data.is_empty() || data.len() > MAX_COVER {
            return;
        }
        if self.cover.is_some() && (self.cover_front || !front) {
            return;
        }
        let mime = match sniff_image(data) {
            Some(m) => m.to_owned(),
            None if mime.is_empty() => return,
            None => mime.to_ascii_lowercase(),
        };
        self.cover = Some(Cover {
            mime,
            data: data.to_vec(),
        });
        self.cover_front = front;
    }
}

/// A picture's MIME type by its signature.
fn sniff_image(d: &[u8]) -> Option<&'static str> {
    if d.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if d.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if d.starts_with(b"GIF8") {
        Some("image/gif")
    } else if d.len() > 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        Some("image/webp")
    } else if d.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}

/// Text as shown: NULs between values become "; ", outer space and NULs go, and it is cut
/// to [`MAX_TEXT`] characters.
fn clean(s: &str) -> String {
    let parts: Vec<&str> = s
        .split('\0')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let mut v = parts.join("; ");
    if v.chars().count() > MAX_TEXT {
        v = v.chars().take(MAX_TEXT).collect();
        v.push('…');
    }
    v
}

/// Reads the tags of a sound file's bytes, whatever its container.
pub fn read(d: &[u8]) -> Tags {
    let mut t = Tags::default();
    // ID3v2 at the start (MP3, AAC, FLAC, DTS, AC-3 files).
    let mut at = 0;
    while d.get(at..at + 3) == Some(b"ID3") {
        let Some(n) = id3v2(&d[at..], &mut t) else {
            break;
        };
        at += n;
    }
    let s = &d[at.min(d.len())..];
    if s.len() >= 12 && matches!(&s[..4], b"RIFF" | b"RF64" | b"BW64")
        // AVI keeps the same INFO list (a movie's tags).
        && matches!(&s[8..12], b"WAVE" | b"AVI ")
    {
        riff(s, &mut t);
    } else if s.len() >= 12 && &s[..4] == b"FORM" && matches!(&s[8..12], b"AIFF" | b"AIFC") {
        aiff(s, &mut t);
    } else if s.starts_with(b"fLaC") {
        flac_blocks(&s[4..], &mut t);
    } else if s.starts_with(b"OggS") {
        ogg(s, &mut t);
    } else if s.get(4..8) == Some(b"ftyp") {
        mp4(s, &mut t);
    } else if crate::mkv::sniff(s) {
        matroska(s, &mut t);
    }
    if s.starts_with(b"MThd")
        || (s.len() >= 12 && &s[..4] == b"RIFF" && matches!(&s[8..12], b"RMID" | b"sfbk" | b"DLS "))
        || specular_synth::xi::is_xi(s)
        || specular_synth::sfz::is_sfz(s)
    {
        if s.starts_with(b"RIFF") && &s[8..12] == b"RMID" {
            riff(s, &mut t);
        }
        let facts = crate::synth_sound::facts(s, true);
        if !facts.is_empty() {
            t.kind(match &s.get(8..12) {
                _ if s.starts_with(b"MThd") => "MIDI text",
                Some(b"RMID") => "MIDI text",
                Some(b"sfbk") => "SoundFont INFO",
                Some(b"DLS ") => "DLS INFO",
                _ if specular_synth::xi::is_xi(s) => "XI header",
                _ => "SFZ",
            });
        }
        for (k, v) in facts {
            t.set(k, &v);
        }
        return t;
    }
    ape(d, &mut t);
    id3v1(d, &mut t);
    if specular_mpc::is_musepack(s) {
        musepack(s, &mut t);
    }
    if specular_tracker::sniff(d).is_some() {
        module(d, &mut t);
    }
    t
}

/// A tracker module's title, and its kind as the encoder.
fn module(d: &[u8], t: &mut Tags) {
    let Ok(m) = specular_tracker::Module::load(d) else {
        return;
    };
    t.set("title", &m.title);
    t.set(
        "encoder",
        &format!("{} ({} channels)", m.kind.name(), m.channels),
    );
}

/// A Musepack stream header's encoder and ReplayGain, when the tags don't name them.
fn musepack(s: &[u8], t: &mut Tags) {
    let Ok(st) = specular_mpc::scan(s) else {
        return;
    };
    let i = st.info;
    if let Some(e) = &i.encoder {
        t.set("encoder", &format!("Musepack {e}"));
    }
    let db = |g: f32| format!("{g:+.2} dB").replace('-', "−");
    if let Some(g) = i.track_gain {
        t.set("track-gain", &db(g));
        t.gain.track_gain.get_or_insert(g);
    }
    if let Some(g) = i.album_gain {
        t.set("album-gain", &db(g));
        t.gain.album_gain.get_or_insert(g);
    }
}

// --- APE tags -------------------------------------------------------------------------------

/// Where tags at the end of `d` stop before ID3v1 and Lyrics3v2 (which sit after APE).
fn before_trailers(d: &[u8]) -> usize {
    let mut end = d.len();
    if end >= 128 && &d[end - 128..end - 125] == b"TAG" {
        end -= 128;
    }
    if end >= 15 && &d[end - 9..end] == b"LYRICS200" {
        let size = std::str::from_utf8(&d[end - 15..end - 9])
            .ok()
            .and_then(|n| n.parse::<usize>().ok());
        if let Some(n) = size.filter(|&n| n + 15 <= end) {
            end -= n + 15;
        }
    }
    end
}

/// An APEv2 (or APEv1) tag at the end of `d`: its 32-byte footer, then its items.
fn ape(d: &[u8], t: &mut Tags) {
    let end = before_trailers(d);
    let Some(foot) = end.checked_sub(32).map(|i| &d[i..end]) else {
        return;
    };
    if &foot[..8] != b"APETAGEX" {
        return;
    }
    let (Some(version), Some(size), Some(count)) = (le32(foot, 8), le32(foot, 12), le32(foot, 16))
    else {
        return;
    };
    // The size counts the items and the footer (not a header).
    let Some(items) = end
        .checked_sub(size)
        .filter(|_| size >= 32)
        .and_then(|from| d.get(from..end - 32))
    else {
        return;
    };
    let v2 = version >= 2000;
    t.kind(if v2 { "APEv2" } else { "APEv1" });
    let mut at = 0;
    for _ in 0..count.min(10_000) {
        let (Some(len), Some(flags)) = (le32(items, at), le32(items, at + 4)) else {
            break;
        };
        let Some(nul) = items
            .get(at + 8..)
            .and_then(|r| r.iter().position(|&b| b == 0))
        else {
            break;
        };
        let key = String::from_utf8_lossy(&items[at + 8..at + 8 + nul]).to_ascii_uppercase();
        let from = at + 8 + nul + 1;
        let Some(value) = items.get(from..from.saturating_add(len)) else {
            break;
        };
        at = from + len;
        match (flags >> 1) & 3 {
            // Text (UTF-8; APEv1's is ASCII); a list's values are NUL-separated.
            0 => {
                let v = String::from_utf8_lossy(value);
                t.gain_field(&key, &v);
                if key == "LYRICS" || key == "UNSYNCED LYRICS" {
                    t.set_lyrics(&v);
                }
                if let Some(k) = ape_key(&key) {
                    let v = if k == "date" {
                        v.split('\0').next().unwrap_or_default().to_owned()
                    } else {
                        v.into_owned()
                    };
                    t.set(k, &v);
                }
            }
            // Binary: a cover is its file name, a NUL, then the picture.
            1 if key.starts_with("COVER ART") => {
                if let Some(z) = value.iter().position(|&b| b == 0) {
                    t.picture("", &value[z + 1..], key == "COVER ART (FRONT)");
                }
            }
            _ => {}
        }
    }
}

/// The field an APE item's key (upper case) gives.
fn ape_key(key: &str) -> Option<&'static str> {
    Some(match key {
        "TITLE" => "title",
        "ARTIST" => "artist",
        "ALBUM" => "album",
        "ALBUM ARTIST" | "ALBUMARTIST" => "album-artist",
        "COMPOSER" => "composer",
        "GENRE" => "genre",
        "YEAR" | "RECORD DATE" | "DATE" => "date",
        "TRACK" => "track",
        "DISC" => "disc",
        "COMMENT" => "comment",
        "COPYRIGHT" => "copyright",
        "ENCODED BY" | "ENCODER" | "TOOL NAME" => "encoder",
        _ => return None,
    })
}

// --- Matroska -------------------------------------------------------------------------------

/// Matroska tags (MKA): album-level (target 50 and up) and track-level ones, the segment's
/// title and writing application, and a cover among the attached files.
fn matroska(d: &[u8], t: &mut Tags) {
    let Some(m) = crate::mkv::file_tags(d) else {
        return;
    };
    if !m.simple.is_empty() {
        t.kind("Matroska tags");
    }
    // A track's own fields first; an album's (the default target) fill what is left.
    let mut ordered: Vec<&(u64, String, String)> = m.simple.iter().filter(|x| x.0 < 50).collect();
    ordered.extend(m.simple.iter().filter(|x| x.0 >= 50));
    let track_artist = m.simple.iter().any(|x| x.0 < 50 && x.1 == "ARTIST");
    let (mut part, mut parts) = (None, None);
    for (target, name, value) in ordered {
        t.gain_field(name, value);
        let album = *target >= 50;
        let k = match name.as_str() {
            "TITLE" if album && m.simple.iter().any(|x| x.0 < 50 && x.1 == "TITLE") => "album",
            "TITLE" => "title",
            "ALBUM" => "album",
            "ARTIST" if album && track_artist => "album-artist",
            "ARTIST" | "PERFORMER" => "artist",
            "ALBUM_ARTIST" | "ALBUMARTIST" => "album-artist",
            "COMPOSER" => "composer",
            "GENRE" => "genre",
            "DATE_RELEASED" | "DATE_RECORDED" | "DATE" | "YEAR" => "date",
            "COMMENT" | "DESCRIPTION" => "comment",
            "COPYRIGHT" => "copyright",
            "ENCODER" | "ENCODED_BY" => "encoder",
            "TRACK" | "TRACKNUMBER" => "track",
            "DISC" | "DISCNUMBER" => "disc",
            "PART_NUMBER" if !album => {
                part.get_or_insert(value.clone());
                continue;
            }
            "TOTAL_PARTS" if album => {
                parts.get_or_insert(value.clone());
                continue;
            }
            "LYRICS" => {
                t.set_lyrics(value);
                continue;
            }
            _ => continue,
        };
        t.set(k, value);
    }
    match (part, parts) {
        (Some(n), Some(of)) => t.set("track", &format!("{}/{}", n.trim(), of.trim())),
        (Some(n), None) => t.set("track", &n),
        _ => {}
    }
    if let Some(title) = &m.title {
        t.set("title", title);
    }
    if let Some(app) = &m.writing_app {
        t.set("encoder", app);
    }
    // The cover: `cover.*` is the front one (the specification's names), else any picture.
    for (name, mime, bytes) in &m.attachments {
        let lower = name.to_ascii_lowercase();
        let front = lower.starts_with("cover.");
        let picture = mime.starts_with("image/") || sniff_image(bytes).is_some();
        if picture && !lower.starts_with("small_") {
            t.picture(mime, bytes, front);
        }
    }
}

// --- ID3v2 ------------------------------------------------------------------------------

/// Undoes unsynchronisation (FF 00 → FF).
fn resync(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len());
    let mut i = 0;
    while i < d.len() {
        out.push(d[i]);
        if d[i] == 0xFF && d.get(i + 1) == Some(&0) {
            i += 1;
        }
        i += 1;
    }
    out
}

fn syncsafe(b: &[u8]) -> usize {
    b.iter()
        .fold(0usize, |a, &x| (a << 7) | usize::from(x & 0x7F))
}

/// Text in an ID3 encoding: 0 ISO-8859-1, 1 UTF-16 with a byte-order mark, 2 UTF-16BE,
/// 3 UTF-8.
pub(crate) fn id3_text(enc: u8, b: &[u8]) -> String {
    match enc {
        0 => b.iter().map(|&c| char::from(c)).collect(),
        1 | 2 => {
            let (be, body) = match b {
                [0xFE, 0xFF, rest @ ..] => (true, rest),
                [0xFF, 0xFE, rest @ ..] => (false, rest),
                _ => (enc == 2, b),
            };
            let units: Vec<u16> = body
                .chunks_exact(2)
                .map(|c| {
                    if be {
                        u16::from_be_bytes([c[0], c[1]])
                    } else {
                        u16::from_le_bytes([c[0], c[1]])
                    }
                })
                .collect();
            // Every value of a list starts with its own byte-order mark.
            String::from_utf16_lossy(&units).replace('\u{FEFF}', "")
        }
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

/// Where a string in encoding `enc` ends in `b` (its terminator: one zero byte, two
/// aligned ones for UTF-16), and where what follows starts.
pub(crate) fn id3_terminated(enc: u8, b: &[u8]) -> (usize, usize) {
    if matches!(enc, 1 | 2) {
        let mut i = 0;
        while i + 1 < b.len() {
            if b[i] == 0 && b[i + 1] == 0 {
                return (i, i + 2);
            }
            i += 2;
        }
        (b.len(), b.len())
    } else {
        match b.iter().position(|&c| c == 0) {
            Some(i) => (i, i + 1),
            None => (b.len(), b.len()),
        }
    }
}

/// The 80 genres of ID3v1 (the numbers ID3v2 `TCON` may refer to).
const GENRES: [&str; 80] = [
    "Blues",
    "Classic Rock",
    "Country",
    "Dance",
    "Disco",
    "Funk",
    "Grunge",
    "Hip-Hop",
    "Jazz",
    "Metal",
    "New Age",
    "Oldies",
    "Other",
    "Pop",
    "R&B",
    "Rap",
    "Reggae",
    "Rock",
    "Techno",
    "Industrial",
    "Alternative",
    "Ska",
    "Death Metal",
    "Pranks",
    "Soundtrack",
    "Euro-Techno",
    "Ambient",
    "Trip-Hop",
    "Vocal",
    "Jazz+Funk",
    "Fusion",
    "Trance",
    "Classical",
    "Instrumental",
    "Acid",
    "House",
    "Game",
    "Sound Clip",
    "Gospel",
    "Noise",
    "Alternative Rock",
    "Bass",
    "Soul",
    "Punk",
    "Space",
    "Meditative",
    "Instrumental Pop",
    "Instrumental Rock",
    "Ethnic",
    "Gothic",
    "Darkwave",
    "Techno-Industrial",
    "Electronic",
    "Pop-Folk",
    "Eurodance",
    "Dream",
    "Southern Rock",
    "Comedy",
    "Cult",
    "Gangsta",
    "Top 40",
    "Christian Rap",
    "Pop/Funk",
    "Jungle",
    "Native American",
    "Cabaret",
    "New Wave",
    "Psychedelic",
    "Rave",
    "Showtunes",
    "Trailer",
    "Lo-Fi",
    "Tribal",
    "Acid Punk",
    "Acid Jazz",
    "Polka",
    "Retro",
    "Musical",
    "Rock & Roll",
    "Hard Rock",
];

fn genre_name(n: usize) -> String {
    GENRES
        .get(n)
        .map_or_else(|| format!("Genre {n}"), |g| (*g).to_owned())
}

/// A `TCON` value: "(17)", "17", "(17)Rock" or text; "RX" and "CR" are remix and cover.
fn genre(v: &str) -> String {
    let mut out = Vec::new();
    for part in v.split('\0').map(str::trim).filter(|p| !p.is_empty()) {
        let mut rest = part;
        let mut named = false;
        while let Some(r) = rest.strip_prefix('(') {
            let Some(close) = r.find(')') else { break };
            let code = &r[..close];
            if let Ok(n) = code.parse::<usize>() {
                out.push(genre_name(n));
                named = true;
            } else if code == "RX" {
                out.push("Remix".into());
                named = true;
            } else if code == "CR" {
                out.push("Cover".into());
                named = true;
            } else {
                break;
            }
            rest = &r[close + 1..];
        }
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        match rest.parse::<usize>() {
            Ok(n) if !named => out.push(genre_name(n)),
            _ if named && out.last().is_some_and(|l| l == rest) => {}
            _ => out.push(rest.to_owned()),
        }
    }
    out.dedup();
    out.join("; ")
}

/// Reads one ID3v2 tag at the start of `d`; its length.
fn id3v2(d: &[u8], t: &mut Tags) -> Option<usize> {
    let h = d.get(..10)?;
    let ver = h[3];
    if !(2..=4).contains(&ver) || h[6..10].iter().any(|b| b & 0x80 != 0) {
        return None;
    }
    let flags = h[5];
    let size = syncsafe(&h[6..10]);
    let total = 10 + size + if flags & 0x10 != 0 { 10 } else { 0 };
    let raw = d.get(10..(10 + size).min(d.len()))?;
    // Whole-tag unsynchronisation (v2.2 and v2.3; v2.4 marks it per frame).
    let body = if flags & 0x80 != 0 && ver < 4 {
        resync(raw)
    } else {
        raw.to_vec()
    };
    let mut at = 0;
    if flags & 0x40 != 0 && ver >= 3 {
        // Extended header: v2.3 counts the size without itself, v2.4 with.
        let n = if ver == 3 {
            u32::from_be_bytes(body.get(..4)?.try_into().ok()?) as usize + 4
        } else {
            syncsafe(body.get(..4)?)
        };
        at = n;
    }
    t.kind(match ver {
        2 => "ID3v2.2",
        3 => "ID3v2.3",
        _ => "ID3v2.4",
    });
    let (id_len, head_len) = if ver == 2 { (3, 6) } else { (4, 10) };
    let mut year = None;
    while at + head_len <= body.len() {
        let id = &body[at..at + id_len];
        if id[0] == 0 {
            break;
        }
        let size = match ver {
            2 => u32::from_be_bytes([0, body[at + 3], body[at + 4], body[at + 5]]) as usize,
            3 => u32::from_be_bytes(body[at + 4..at + 8].try_into().ok()?) as usize,
            _ => syncsafe(&body[at + 4..at + 8]),
        };
        let fflags = if ver == 2 {
            0
        } else {
            u16::from_be_bytes([body[at + 8], body[at + 9]])
        };
        let start = at + head_len;
        let end = start.checked_add(size)?.min(body.len());
        at = end;
        let mut f: Vec<u8> = body[start..end].to_vec();
        // Compression and encryption (v2.3: 0x0080, 0x0040; v2.4: 0x0008, 0x0004) are
        // skipped; v2.4 frame unsynchronisation and data length indicators are undone.
        let (compressed, encrypted) = match ver {
            3 => (fflags & 0x80 != 0, fflags & 0x40 != 0),
            4 => (fflags & 0x08 != 0, fflags & 0x04 != 0),
            _ => (false, false),
        };
        if compressed || encrypted {
            continue;
        }
        if ver == 4 {
            if fflags & 0x01 != 0 && f.len() >= 4 {
                f.drain(..4);
            }
            if fflags & 0x02 != 0 || flags & 0x80 != 0 {
                f = resync(&f);
            }
        } else if ver == 3 && fflags & 0x20 != 0 && !f.is_empty() {
            // A grouping byte.
            f.remove(0);
        }
        let id = std::str::from_utf8(id).unwrap_or("");
        id3_frame(id, &f, t, &mut year);
    }
    if let Some(y) = year {
        t.set("date", &y);
    }
    Some(total.min(d.len()))
}

fn id3_frame(id: &str, f: &[u8], t: &mut Tags, year: &mut Option<String>) {
    let text = || {
        let (&enc, rest) = f.split_first()?;
        Some(id3_text(enc, rest))
    };
    let key = match id {
        "TIT2" | "TT2" => Some("title"),
        "TPE1" | "TP1" => Some("artist"),
        "TALB" | "TAL" => Some("album"),
        "TPE2" | "TP2" => Some("album-artist"),
        "TCOM" | "TCM" => Some("composer"),
        "TRCK" | "TRK" => Some("track"),
        "TPOS" | "TPA" => Some("disc"),
        "TCOP" | "TCR" => Some("copyright"),
        "TSSE" | "TSS" | "TENC" | "TEN" => Some("encoder"),
        "TDRC" => Some("date"),
        _ => None,
    };
    if let Some(k) = key {
        if let Some(v) = text() {
            t.set(k, &v);
        }
        return;
    }
    match id {
        "TCON" | "TCO" => {
            if let Some(v) = text() {
                t.set("genre", &genre(&v));
            }
        }
        // v2.3 and v2.2: the year (TDAT and TIME are left out).
        "TYER" | "TYE" | "TORY" | "TDOR" => {
            if year.is_none() {
                *year = text().map(|v| clean(&v)).filter(|v| !v.is_empty());
            }
        }
        "COMM" | "COM" => {
            // Encoding, language, a description (iTunes keeps its own data in described
            // comments), then the text.
            let Some((&enc, rest)) = f.split_first() else {
                return;
            };
            let Some(rest) = rest.get(3..) else { return };
            let (dend, tstart) = id3_terminated(enc, rest);
            let desc = id3_text(enc, &rest[..dend]);
            if desc.is_empty() || desc.eq_ignore_ascii_case("comment") {
                t.set("comment", &id3_text(enc, &rest[tstart..]));
            } else {
                // iTunes' Sound Check (`iTunNORM`).
                t.gain_field(&desc, &id3_text(enc, &rest[tstart..]));
            }
        }
        // User text: a description, then the value (ReplayGain's fields).
        "TXXX" | "TXX" => {
            let Some((&enc, rest)) = f.split_first() else {
                return;
            };
            let (dend, tstart) = id3_terminated(enc, rest);
            let desc = id3_text(enc, &rest[..dend]);
            t.gain_field(
                &desc,
                &id3_text(enc, rest.get(tstart..).unwrap_or_default()),
            );
        }
        "USLT" | "ULT" => {
            // Encoding, language, a description, then the text.
            let Some((&enc, rest)) = f.split_first() else {
                return;
            };
            let Some(rest) = rest.get(3..) else { return };
            let (_, tstart) = id3_terminated(enc, rest);
            t.set_lyrics(&id3_text(enc, &rest[tstart.min(rest.len())..]));
        }
        "SYLT" | "SLT" => {
            if t.synced_lyrics.is_none() && f.len() <= crate::lyrics::MAX_BYTES {
                t.synced_lyrics = Some(f.to_vec());
            }
        }
        "APIC" => {
            let Some((&enc, rest)) = f.split_first() else {
                return;
            };
            let Some(m) = rest.iter().position(|&c| c == 0) else {
                return;
            };
            let mime = String::from_utf8_lossy(&rest[..m]).into_owned();
            let Some(&kind) = rest.get(m + 1) else { return };
            let after = &rest[m + 2..];
            let (_, data) = id3_terminated(enc, after);
            t.picture(&mime, &after[data..], kind == 3);
        }
        "PIC" => {
            let Some((&enc, rest)) = f.split_first() else {
                return;
            };
            let Some(fmt) = rest.get(..3) else { return };
            let mime = match fmt {
                b"JPG" => "image/jpeg",
                b"PNG" => "image/png",
                _ => "",
            };
            let Some(&kind) = rest.get(3) else { return };
            let after = &rest[4..];
            let (_, data) = id3_terminated(enc, after);
            t.picture(mime, &after[data..], kind == 3);
        }
        _ => {}
    }
}

/// An ID3v1 (or v1.1) tag in the last 128 bytes.
fn id3v1(d: &[u8], t: &mut Tags) {
    let Some(tag) = d.len().checked_sub(128).map(|i| &d[i..]) else {
        return;
    };
    if &tag[..3] != b"TAG" {
        return;
    }
    let field = |r: std::ops::Range<usize>| -> String {
        let b = &tag[r];
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        b[..end].iter().map(|&c| char::from(c)).collect()
    };
    t.kind("ID3v1");
    t.set("title", &field(3..33));
    t.set("artist", &field(33..63));
    t.set("album", &field(63..93));
    t.set("date", &field(93..97));
    if tag[125] == 0 && tag[126] != 0 {
        t.set("comment", &field(97..125));
        t.set("track", &tag[126].to_string());
    } else {
        t.set("comment", &field(97..127));
    }
    if usize::from(tag[127]) < GENRES.len() {
        t.set("genre", GENRES[usize::from(tag[127])]);
    }
}

// --- Vorbis comments and FLAC ---------------------------------------------------------------

fn le32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn be32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// A Vorbis comment block (vendor string, then `KEY=value` comments).
fn vorbis_comments(d: &[u8], t: &mut Tags) {
    let Some(vlen) = le32(d, 0) else { return };
    let vendor = d
        .get(4..4 + vlen)
        .map(|v| String::from_utf8_lossy(v).into_owned());
    let mut at = 4 + vlen;
    let Some(n) = le32(d, at) else { return };
    at += 4;
    t.kind("Vorbis comments");
    let (mut track_total, mut disc_total) = (None, None);
    let (mut track, mut disc) = (None, None);
    let mut mine: Vec<&'static str> = Vec::new();
    for _ in 0..n.min(10_000) {
        let Some(len) = le32(d, at) else { break };
        let Some(c) = d.get(at + 4..at + 4 + len) else {
            break;
        };
        at += 4 + len;
        let Some(eq) = c.iter().position(|&b| b == b'=') else {
            continue;
        };
        let key = String::from_utf8_lossy(&c[..eq]).to_ascii_uppercase();
        let raw = &c[eq + 1..];
        if key == "METADATA_BLOCK_PICTURE" || key == "COVERART" {
            if let Some(b) = base64(raw) {
                if key == "COVERART" {
                    t.picture("", &b, false);
                } else {
                    flac_picture(&b, t);
                }
            }
            continue;
        }
        let v = String::from_utf8_lossy(raw).into_owned();
        t.gain_field(&key, &v);
        let k = match key.as_str() {
            "TITLE" => "title",
            "ARTIST" => "artist",
            "ALBUM" => "album",
            "ALBUMARTIST" | "ALBUM ARTIST" | "ALBUM_ARTIST" => "album-artist",
            "COMPOSER" => "composer",
            "GENRE" => "genre",
            "DATE" | "YEAR" => "date",
            "COMMENT" | "DESCRIPTION" => "comment",
            "COPYRIGHT" => "copyright",
            "ENCODER" | "ENCODED-BY" | "ENCODED_BY" => "encoder",
            "LYRICS" | "UNSYNCEDLYRICS" | "UNSYNCED LYRICS" => {
                t.set_lyrics(&v);
                continue;
            }
            "TRACKNUMBER" => {
                track.get_or_insert(v);
                continue;
            }
            "TRACKTOTAL" | "TOTALTRACKS" => {
                track_total.get_or_insert(v);
                continue;
            }
            "DISCNUMBER" => {
                disc.get_or_insert(v);
                continue;
            }
            "DISCTOTAL" | "TOTALDISCS" => {
                disc_total.get_or_insert(v);
                continue;
            }
            _ => continue,
        };
        // Repeated fields are values of a list (this block's: an earlier tag's value stays).
        match t.fields.iter_mut().find(|(x, _)| *x == k) {
            Some((_, have)) if mine.contains(&k) => {
                let v = clean(&v);
                if !v.is_empty() && !have.split("; ").any(|h| h == v) {
                    have.push_str("; ");
                    have.push_str(&v);
                }
            }
            Some(_) => {}
            None => {
                t.set(k, &v);
                mine.push(k);
            }
        }
    }
    let joined = |n: Option<String>, total: Option<String>| match (n, total) {
        (Some(n), Some(tot)) if !n.contains('/') => Some(format!("{}/{}", n.trim(), tot.trim())),
        (n, _) => n,
    };
    if let Some(v) = joined(track, track_total) {
        t.set("track", &v);
    }
    if let Some(v) = joined(disc, disc_total) {
        t.set("disc", &v);
    }
    if let Some(v) = vendor {
        t.set("encoder", &v);
    }
}

/// Standard base64 (RFC 4648), padding optional; `None` on a stray character.
fn base64(s: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut n) = (0u32, 0);
    for &c in s {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((acc >> n) as u8);
        }
        if out.len() > MAX_COVER + 1024 {
            return None;
        }
    }
    Some(out)
}

/// A FLAC PICTURE block body (type, MIME type, description, size, then the data).
fn flac_picture(b: &[u8], t: &mut Tags) {
    let Some(kind) = be32(b, 0) else { return };
    let Some(ml) = be32(b, 4) else { return };
    let mime = b
        .get(8..8 + ml)
        .map(|m| String::from_utf8_lossy(m).into_owned())
        .unwrap_or_default();
    let at = 8 + ml;
    let Some(dl) = be32(b, at) else { return };
    let at = at + 4 + dl + 16;
    let Some(len) = be32(b, at) else { return };
    if let Some(data) = b.get(at + 4..at + 4 + len) {
        t.picture(&mime, data, kind == 3);
    }
}

/// FLAC metadata blocks (after `fLaC`).
fn flac_blocks(d: &[u8], t: &mut Tags) {
    let mut at = 0;
    while let Some(h) = d.get(at..at + 4) {
        let len = u32::from_be_bytes([0, h[1], h[2], h[3]]) as usize;
        let Some(body) = d.get(at + 4..at + 4 + len) else {
            break;
        };
        match h[0] & 0x7F {
            4 => vorbis_comments(body, t),
            6 => flac_picture(body, t),
            _ => {}
        }
        at += 4 + len;
        if h[0] & 0x80 != 0 {
            break;
        }
    }
}

/// The first logical stream's header packets of an Ogg file: its comments (Vorbis, Opus,
/// Ogg FLAC).
fn ogg(d: &[u8], t: &mut Tags) {
    let mut serial = None;
    let mut packets: Vec<Vec<u8>> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut at = 0;
    let mut bytes = 0usize;
    'pages: while let Some(h) = d.get(at..at + 27) {
        if &h[..4] != b"OggS" {
            break;
        }
        let s = u32::from_le_bytes([h[14], h[15], h[16], h[17]]);
        let nseg = usize::from(h[26]);
        let Some(lacing) = d.get(at + 27..at + 27 + nseg) else {
            break;
        };
        let mut body = at + 27 + nseg;
        let mine = *serial.get_or_insert(s) == s;
        for &l in lacing {
            let Some(seg) = d.get(body..body + usize::from(l)) else {
                break 'pages;
            };
            body += usize::from(l);
            if !mine {
                continue;
            }
            cur.extend_from_slice(seg);
            bytes += seg.len();
            if l < 255 {
                packets.push(std::mem::take(&mut cur));
                if packets.len() >= 8 {
                    break 'pages;
                }
            }
        }
        if bytes > MAX_COVER * 2 {
            break;
        }
        at = body;
    }
    let Some(first) = packets.first() else { return };
    if first.starts_with(b"\x01vorbis") {
        if let Some(c) = packets.get(1).filter(|p| p.starts_with(b"\x03vorbis")) {
            vorbis_comments(&c[7..], t);
        }
    } else if first.starts_with(b"OpusHead") {
        if let Some(c) = packets.get(1).filter(|p| p.starts_with(b"OpusTags")) {
            vorbis_comments(&c[8..], t);
        }
    } else if first.starts_with(b"Speex   ") {
        // Speex: the second packet is a bare Vorbis comment block (no packet type or magic).
        if let Some(c) = packets.get(1) {
            vorbis_comments(c, t);
        }
    } else if first.starts_with(b"\x7fFLAC") {
        // Mapping 1.0: the first packet holds `fLaC` and STREAMINFO; each later header
        // packet is one metadata block.
        for p in &packets[1..] {
            let mut block = p.clone();
            if let Some(b) = block.first_mut() {
                *b |= 0x80;
            }
            flac_blocks(&block, t);
        }
    }
}

// --- MP4 ------------------------------------------------------------------------------------

/// The child boxes of `d`: (type, body).
fn boxes(d: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let size = be32(d, at)?;
        let kind: [u8; 4] = d.get(at + 4..at + 8)?.try_into().ok()?;
        let (head, size) = match size {
            1 => (
                16,
                u64::from_be_bytes(d.get(at + 8..at + 16)?.try_into().ok()?) as usize,
            ),
            0 => (8, d.len() - at),
            n => (8, n),
        };
        if size < head {
            return None;
        }
        let body = d.get(at + head..at.checked_add(size)?)?;
        at += size;
        Some((kind, body))
    })
}

fn child<'a>(d: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(d).find(|(k, _)| k == kind).map(|(_, b)| b)
}

fn mp4(d: &[u8], t: &mut Tags) {
    let Some(moov) = child(d, b"moov") else {
        return;
    };
    let Some(meta) = child(moov, b"udta").and_then(|u| child(u, b"meta")) else {
        return;
    };
    // A full box: version and flags before the children.
    let meta = meta.get(4..).unwrap_or_default();
    let Some(ilst) = child(meta, b"ilst") else {
        return;
    };
    t.kind("MP4 items");
    for (kind, item) in boxes(ilst) {
        let Some(data) = child(item, b"data") else {
            continue;
        };
        let ty = be32(data, 0).unwrap_or(0) & 0xFF_FFFF;
        let v = data.get(8..).unwrap_or_default();
        let text = || String::from_utf8_lossy(v).into_owned();
        let pair = || {
            let n = u16::from_be_bytes([*v.get(2)?, *v.get(3)?]);
            let total = v.get(4..6).map_or(0, |b| u16::from_be_bytes([b[0], b[1]]));
            (n > 0).then(|| {
                if total > 0 {
                    format!("{n}/{total}")
                } else {
                    n.to_string()
                }
            })
        };
        match &kind {
            b"\xa9nam" => t.set("title", &text()),
            b"\xa9ART" => t.set("artist", &text()),
            b"\xa9alb" => t.set("album", &text()),
            b"aART" => t.set("album-artist", &text()),
            b"\xa9wrt" => t.set("composer", &text()),
            b"\xa9gen" => t.set("genre", &text()),
            b"gnre" => {
                // An ID3v1 genre number plus one.
                if let Some(n) = v.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]])) {
                    t.set("genre", &genre_name(usize::from(n).saturating_sub(1)));
                }
            }
            b"\xa9day" => t.set("date", &text()),
            b"trkn" => {
                if let Some(p) = pair() {
                    t.set("track", &p);
                }
            }
            b"disk" => {
                if let Some(p) = pair() {
                    t.set("disc", &p);
                }
            }
            b"\xa9cmt" => t.set("comment", &text()),
            b"\xa9lyr" => t.set_lyrics(&text()),
            b"cprt" => t.set("copyright", &text()),
            b"\xa9too" => t.set("encoder", &text()),
            // Freeform: `mean` (the owner), `name` and the data (ReplayGain, iTunNORM).
            b"----" => {
                let name = child(item, b"name")
                    .and_then(|n| n.get(4..))
                    .unwrap_or_default();
                t.gain_field(&String::from_utf8_lossy(name), &text());
            }
            b"covr" => {
                let mime = match ty {
                    13 => "image/jpeg",
                    14 => "image/png",
                    27 => "image/bmp",
                    _ => "",
                };
                t.picture(mime, v, true);
            }
            _ => {}
        }
    }
}

// --- RIFF and AIFF ---------------------------------------------------------------------------

fn riff(d: &[u8], t: &mut Tags) {
    let mut at = 12;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let size = le32(d, at + 4).unwrap_or(0);
        let body = &d[at + 8..(at + 8).saturating_add(size).min(d.len())];
        match id {
            b"LIST" if body.starts_with(b"INFO") => {
                t.kind("RIFF INFO");
                let mut i = 4;
                while i + 8 <= body.len() {
                    let k = &body[i..i + 4];
                    let n = le32(body, i + 4).unwrap_or(0);
                    let v = body.get(i + 8..i + 8 + n).unwrap_or_default();
                    // INFO text is usually Latin-1 or UTF-8: UTF-8 when it is valid.
                    let v = match std::str::from_utf8(v) {
                        Ok(s) => s.to_owned(),
                        Err(_) => v.iter().map(|&c| char::from(c)).collect(),
                    };
                    let key = match k {
                        b"INAM" => Some("title"),
                        b"IART" => Some("artist"),
                        b"IPRD" => Some("album"),
                        b"ICRD" => Some("date"),
                        b"IGNR" => Some("genre"),
                        b"ICMT" => Some("comment"),
                        b"ICOP" => Some("copyright"),
                        b"ISFT" => Some("encoder"),
                        b"ITRK" | b"IPRT" => Some("track"),
                        b"IMUS" => Some("composer"),
                        _ => None,
                    };
                    if let Some(k) = key {
                        t.set(k, &v);
                    }
                    i += 8 + n + (n & 1);
                }
            }
            b"id3 " | b"ID3 " => {
                id3v2(body, t);
            }
            _ => {}
        }
        if size > d.len() {
            break;
        }
        at += 8 + size + (size & 1);
    }
}

fn aiff(d: &[u8], t: &mut Tags) {
    let mut at = 12;
    while at + 8 <= d.len() {
        let id = &d[at..at + 4];
        let size = be32(d, at + 4).unwrap_or(0);
        let body = &d[at + 8..(at + 8).saturating_add(size).min(d.len())];
        let text = || body.iter().map(|&c| char::from(c)).collect::<String>();
        match id {
            b"NAME" => t.set("title", &text()),
            b"AUTH" => t.set("artist", &text()),
            b"(c) " => t.set("copyright", &text()),
            b"ANNO" => t.set("comment", &text()),
            b"ID3 " | b"id3 " => {
                id3v2(body, t);
            }
            _ => {}
        }
        if matches!(id, b"NAME" | b"AUTH" | b"(c) " | b"ANNO") {
            t.kind("AIFF text");
        }
        if size > d.len() {
            break;
        }
        at += 8 + size + (size & 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(&[0, 0]);
        v.extend_from_slice(body);
        v
    }

    fn tag(ver: u8, frames: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = frames.concat();
        let n = body.len();
        let mut v = vec![b'I', b'D', b'3', ver, 0, 0];
        v.extend_from_slice(&[
            (n >> 21) as u8 & 127,
            (n >> 14) as u8 & 127,
            (n >> 7) as u8 & 127,
            n as u8 & 127,
        ]);
        v.extend(body);
        v
    }

    /// An Ogg Speex file's comment packet (the second packet: a bare Vorbis comment block),
    /// laid out as ffmpeg's libspeex writes it (exiftool 13.50 reads no Speex comments, so
    /// the bytes were checked by hand: vendor "Lavf62.3.100", title=Tone, artist=Specular).
    #[test]
    fn speex_comment_packet() {
        let page = |seq: u32, packet: &[u8]| {
            let mut p = b"OggS\0".to_vec();
            p.push(if seq == 0 { 2 } else { 0 });
            p.extend([0; 8]);
            p.extend(7u32.to_le_bytes());
            p.extend(seq.to_le_bytes());
            p.extend([0; 4]);
            let mut lacing = vec![255u8; packet.len() / 255];
            lacing.push((packet.len() % 255) as u8);
            p.push(lacing.len() as u8);
            p.extend(lacing);
            p.extend(packet);
            p
        };
        let mut head = b"Speex   1.2".to_vec();
        head.resize(80, 0);
        let mut c = 12u32.to_le_bytes().to_vec();
        c.extend(b"Lavf62.3.100");
        c.extend(2u32.to_le_bytes());
        for kv in ["title=Tone", "artist=Specular"] {
            c.extend((kv.len() as u32).to_le_bytes());
            c.extend(kv.as_bytes());
        }
        let d = [page(0, &head), page(1, &c)].concat();
        let t = read(&d);
        assert_eq!(t.get("title"), Some("Tone"));
        assert_eq!(t.get("artist"), Some("Specular"));
    }

    /// An AVI's INFO list, as ffmpeg writes it. exiftool on the same layout (ffmpeg
    /// `-metadata title=… comment=…`): Title "Tram 28", Comment "first ride".
    #[test]
    fn avi_info_list() {
        let mut info = b"INFO".to_vec();
        for (k, v) in [(b"INAM", &b"Tram 28\0"[..]), (b"ICMT", b"first ride\0")] {
            info.extend(k);
            info.extend((v.len() as u32).to_le_bytes());
            info.extend(v);
        }
        let mut d = b"RIFF\0\0\0\0AVI LIST".to_vec();
        d.extend((info.len() as u32).to_le_bytes());
        d.extend(info);
        let t = read(&d);
        assert_eq!(t.get("title"), Some("Tram 28"));
        assert_eq!(t.get("comment"), Some("first ride"));
        assert_eq!(t.kinds, ["RIFF INFO"]);
    }

    #[test]
    fn id3v23_text_comment_and_cover() {
        let mut utf16 = vec![1, 0xFF, 0xFE];
        utf16.extend("Été".encode_utf16().flat_map(u16::to_le_bytes));
        let mut comm = vec![0, b'e', b'n', b'g', 0];
        comm.extend_from_slice(b"Nice");
        let mut apic = vec![0];
        apic.extend_from_slice(b"image/png\0");
        apic.push(3);
        apic.extend_from_slice(b"cover\0");
        apic.extend_from_slice(b"\x89PNG\r\n\x1a\nrest");
        let d = tag(
            3,
            &[
                frame(b"TIT2", &utf16),
                frame(b"TPE1", b"\0Band"),
                frame(b"TCON", b"\0(17)"),
                frame(b"TYER", b"\x001999"),
                frame(b"TRCK", b"\x003/12"),
                frame(b"COMM", &comm),
                frame(b"APIC", &apic),
            ],
        );
        let t = read(&d);
        assert_eq!(t.get("title"), Some("Été"));
        assert_eq!(t.get("artist"), Some("Band"));
        assert_eq!(t.get("genre"), Some("Rock"));
        assert_eq!(t.get("date"), Some("1999"));
        assert_eq!(t.get("track"), Some("3/12"));
        assert_eq!(t.get("comment"), Some("Nice"));
        let c = t.cover.unwrap();
        assert_eq!(c.mime, "image/png");
        assert_eq!(c.data, b"\x89PNG\r\n\x1a\nrest");
        assert_eq!(t.kinds, ["ID3v2.3"]);
        // In display order whatever the frame order.
        let keys: Vec<_> = t.fields.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            keys,
            ["title", "artist", "genre", "date", "track", "comment"]
        );
    }

    #[test]
    fn genres() {
        assert_eq!(genre("(17)Rock"), "Rock");
        assert_eq!(genre("(13)(RX)"), "Pop; Remix");
        assert_eq!(genre("8"), "Jazz");
        assert_eq!(genre("Ambient\0Drone"), "Ambient; Drone");
        assert_eq!(genre("(200)"), "Genre 200");
    }

    #[test]
    fn id3v1_and_vorbis() {
        let mut d = vec![0u8; 10];
        let mut v1 = b"TAG".to_vec();
        let mut put = |s: &[u8], n: usize| {
            let mut f = s.to_vec();
            f.resize(n, 0);
            v1.extend(f);
        };
        put(b"Song", 30);
        put(b"Who", 30);
        put(b"Disc", 30);
        put(b"2001", 4);
        put(b"hi", 28);
        v1.extend_from_slice(&[0, 7, 8]);
        d.extend(v1);
        let t = read(&d);
        assert_eq!(t.get("title"), Some("Song"));
        assert_eq!(t.get("track"), Some("7"));
        assert_eq!(t.get("genre"), Some("Jazz"));

        let mut vc = Vec::new();
        vc.extend_from_slice(&3u32.to_le_bytes());
        vc.extend_from_slice(b"enc");
        let cs: [&[u8]; 4] = [b"title=A", b"ARTIST=x", b"ARTIST=y", b"TRACKNUMBER=2"];
        vc.extend_from_slice(&(cs.len() as u32 + 1).to_le_bytes());
        for c in cs.iter().chain(std::iter::once(&&b"TRACKTOTAL=9"[..])) {
            vc.extend_from_slice(&(c.len() as u32).to_le_bytes());
            vc.extend_from_slice(c);
        }
        let mut t = Tags::default();
        vorbis_comments(&vc, &mut t);
        assert_eq!(t.get("title"), Some("A"));
        assert_eq!(t.get("artist"), Some("x; y"));
        assert_eq!(t.get("track"), Some("2/9"));
        assert_eq!(t.get("encoder"), Some("enc"));
    }

    #[test]
    fn levelling_tags_from_every_kind() {
        // Vorbis comments (Opus's R128 here).
        let mut vc = Vec::new();
        vc.extend_from_slice(&0u32.to_le_bytes());
        let cs: [&[u8]; 2] = [b"R128_TRACK_GAIN=-1536", b"replaygain_album_peak=0.5"];
        vc.extend_from_slice(&(cs.len() as u32).to_le_bytes());
        for c in cs {
            vc.extend_from_slice(&(c.len() as u32).to_le_bytes());
            vc.extend_from_slice(c);
        }
        let mut t = Tags::default();
        vorbis_comments(&vc, &mut t);
        assert_eq!(t.gain.track_gain, Some(-1.0));
        assert_eq!(t.gain.album_peak, Some(0.5));
        assert_eq!(t.get("track-gain"), Some("\u{2212}1.00 dB"));
        // An MP4 freeform item.
        let bx = |k: &[u8; 4], body: &[u8]| {
            let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
            v.extend_from_slice(k);
            v.extend_from_slice(body);
            v
        };
        let mut name = vec![0u8; 4];
        name.extend_from_slice(b"replaygain_track_gain");
        let mut data = vec![0, 0, 0, 1, 0, 0, 0, 0];
        data.extend_from_slice(b"-7.5 dB");
        let mut item = bx(b"mean", b"\0\0\0\0com.apple.iTunes");
        item.extend(bx(b"name", &name));
        item.extend(bx(b"data", &data));
        let ilst = bx(b"ilst", &bx(b"----", &item));
        let mut meta = vec![0u8; 4];
        meta.extend(ilst);
        let moov = bx(b"moov", &bx(b"udta", &bx(b"meta", &meta)));
        let mut file = bx(b"ftyp", b"M4A \0\0\0\0");
        file.extend(moov);
        let t = read(&file);
        assert_eq!(t.gain.track_gain, Some(-7.5));
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64(b"aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64(b"aGVsbG8").unwrap(), b"hello");
        assert!(base64(b"a*b").is_none());
    }

    /// An APE tag's item: value size, flags, key, NUL, value.
    fn ape_item(key: &str, flags: u32, value: &[u8]) -> Vec<u8> {
        let mut v = (value.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(&flags.to_le_bytes());
        v.extend_from_slice(key.as_bytes());
        v.push(0);
        v.extend_from_slice(value);
        v
    }

    /// An APE tag of `items`: a header (APEv2 only), the items and the footer.
    fn ape_tag(version: u32, items: &[Vec<u8>]) -> Vec<u8> {
        let body = items.concat();
        let size = body.len() as u32 + 32;
        let block = |header: bool| {
            let mut v = b"APETAGEX".to_vec();
            for x in [version, size, items.len() as u32] {
                v.extend_from_slice(&x.to_le_bytes());
            }
            let flags = if header { 0xA000_0000u32 } else { 0x8000_0000 };
            v.extend_from_slice(&flags.to_le_bytes());
            v.extend_from_slice(&[0; 8]);
            v
        };
        let mut out = if version >= 2000 {
            block(true)
        } else {
            Vec::new()
        };
        out.extend(body);
        out.extend(block(false));
        out
    }

    /// APEv2 at the end of an MP3, before Lyrics3v2 and ID3v1: its text, lists, year and
    /// front cover; ID3v2 at the start wins a field both give, ID3v1 fills only what is left.
    #[test]
    fn ape_tags_at_the_end() {
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        let mut cover = b"front.png\0".to_vec();
        cover.extend_from_slice(&png);
        let ape = ape_tag(
            2000,
            &[
                ape_item("Title", 0, "Ŝong".as_bytes()),
                ape_item("Artist", 0, b"A\0B"),
                ape_item("Album Artist", 0, b"Various"),
                ape_item("Year", 0, b"1999"),
                ape_item("Track", 0, b"3/12"),
                ape_item("Cover Art (Front)", 2, &cover),
                ape_item("Unknown", 0, b"x"),
            ],
        );
        let mut d = tag(4, &[frame(b"TALB", b"\x03Album")]);
        d.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        d.extend(vec![0; 400]);
        d.extend(&ape);
        // Lyrics3v2: its fields, their size in six digits, LYRICS200.
        let lyrics = b"LYRICSBEGININD0000210";
        d.extend_from_slice(lyrics);
        d.extend_from_slice(format!("{:06}LYRICS200", lyrics.len()).as_bytes());
        let mut v1 = vec![0u8; 128];
        v1[..3].copy_from_slice(b"TAG");
        v1[3..9].copy_from_slice(b"Old v1");
        v1[63..69].copy_from_slice(b"V1 alb");
        v1[127] = 8;
        d.extend(v1);
        let t = read(&d);
        assert_eq!(t.kinds, ["ID3v2.4", "APEv2", "ID3v1"]);
        assert_eq!(t.get("title"), Some("Ŝong"));
        assert_eq!(t.get("artist"), Some("A; B"));
        assert_eq!(t.get("album"), Some("Album"), "ID3v2 first");
        assert_eq!(t.get("album-artist"), Some("Various"));
        assert_eq!(t.get("date"), Some("1999"));
        assert_eq!(t.get("track"), Some("3/12"));
        assert_eq!(t.get("genre"), Some("Jazz"), "ID3v1 fills the rest");
        let c = t.cover.as_ref().expect("the cover");
        assert_eq!(
            (c.mime.as_str(), c.data.as_slice()),
            ("image/png", &png[..])
        );

        // APEv1 (no header, version 1000) right at the end; a cut tag is ignored.
        let mut d = vec![0u8; 64];
        d.extend(ape_tag(1000, &[ape_item("Title", 0, b"Old")]));
        assert_eq!(read(&d).get("title"), Some("Old"));
        assert_eq!(read(&d).kinds, ["APEv1"]);
        let cut = d[70..].to_vec();
        assert!(read(&cut).fields.is_empty());
    }

    /// An EBML element with an 8-byte size (`known == false`: unknown).
    fn el(id: u32, body: &[u8], known: bool) -> Vec<u8> {
        let mut v: Vec<u8> = id
            .to_be_bytes()
            .into_iter()
            .skip_while(|&b| b == 0)
            .collect();
        if known {
            v.push(0x01);
            v.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
        } else {
            v.extend_from_slice(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        }
        v.extend_from_slice(body);
        v
    }

    fn e(id: u32, body: &[u8]) -> Vec<u8> {
        el(id, body, true)
    }

    fn simple(name: &str, value: &str) -> Vec<u8> {
        [e(0x45A3, name.as_bytes()), e(0x4487, value.as_bytes())].concat()
    }

    /// An MKA's tags: the track's title and artist, the album's title, artist, date and
    /// track count, the segment's writing application and the cover attachment, found
    /// through the SeekHead past a cluster of unknown size.
    #[test]
    fn matroska_tags_and_cover() {
        let jpeg = b"\xFF\xD8\xFF\xE0jpeg".to_vec();
        let album = e(
            0x7373,
            &[
                e(0x63C0, &e(0x68CA, &[50])),
                e(0x67C8, &simple("TITLE", "Live at the Hall")),
                e(0x67C8, &simple("ARTIST", "The Band")),
                e(0x67C8, &simple("DATE_RELEASED", "2001-05-04")),
                e(0x67C8, &simple("TOTAL_PARTS", "9")),
            ]
            .concat(),
        );
        let track = e(
            0x7373,
            &[
                e(0x63C0, &e(0x68CA, &[30])),
                e(0x67C8, &simple("TITLE", "Opening")),
                e(0x67C8, &simple("ARTIST", "Ann Lead")),
                e(0x67C8, &simple("PART_NUMBER", "1")),
                // Nested tags count too.
                e(
                    0x67C8,
                    &[
                        simple("COMPOSER", "C. Writer"),
                        e(0x67C8, &simple("GENRE", "Rock")),
                    ]
                    .concat(),
                ),
            ]
            .concat(),
        );
        let tags = e(0x1254_C367, &[album, track].concat());
        let attachments = e(
            0x1941_A469,
            &e(
                0x61A7,
                &[
                    e(0x466E, b"cover.jpg"),
                    e(0x4660, b"image/jpeg"),
                    e(0x465C, &jpeg),
                ]
                .concat(),
            ),
        );
        let info = e(
            0x1549_A966,
            &[e(0x2AD7B1, &[0x0F, 0x42, 0x40]), e(0x5741, b"mkvmerge")].concat(),
        );
        // Seek entries with 8-byte positions, so the SeekHead's length is known first.
        let seek = |id: u32, pos: u64| {
            e(
                0x4DBB,
                &[e(0x53AB, &id.to_be_bytes()), e(0x53AC, &pos.to_be_bytes())].concat(),
            )
        };
        let head_len = e(0x114D_9B74, &[seek(0, 0), seek(0, 0)].concat()).len();
        let cluster = el(0x1F43_B675, &e(0xE7, &[0]), false);
        let tags_at = (head_len + info.len() + cluster.len()) as u64;
        let att_at = tags_at + tags.len() as u64;
        let head = e(
            0x114D_9B74,
            &[seek(0x1254_C367, tags_at), seek(0x1941_A469, att_at)].concat(),
        );
        let body = [head, info, cluster, tags, attachments].concat();
        let mut d = e(0x1A45_DFA3, &e(0x4282, b"matroska"));
        d.extend(e(0x1853_8067, &body));
        let t = read(&d);
        assert_eq!(t.kinds, ["Matroska tags"]);
        assert_eq!(t.get("title"), Some("Opening"));
        assert_eq!(t.get("album"), Some("Live at the Hall"));
        assert_eq!(t.get("artist"), Some("Ann Lead"));
        assert_eq!(t.get("album-artist"), Some("The Band"));
        assert_eq!(t.get("date"), Some("2001-05-04"));
        assert_eq!(t.get("track"), Some("1/9"));
        assert_eq!(t.get("composer"), Some("C. Writer"));
        assert_eq!(t.get("genre"), Some("Rock"));
        assert_eq!(t.get("encoder"), Some("mkvmerge"));
        let c = t.cover.as_ref().expect("the cover");
        assert_eq!(
            (c.mime.as_str(), c.data.as_slice()),
            ("image/jpeg", &jpeg[..])
        );

        // Tags without a target type (as ffmpeg writes them) are the album's: one artist.
        let plain = e(
            0x1254_C367,
            &e(
                0x7373,
                &[
                    e(0x63C0, &[]),
                    e(0x67C8, &simple("ARTIST", "Solo")),
                    e(0x67C8, &simple("ALBUM", "First")),
                ]
                .concat(),
            ),
        );
        let mut d = e(0x1A45_DFA3, &e(0x4282, b"matroska"));
        d.extend(e(0x1853_8067, &plain));
        let t = read(&d);
        assert_eq!(t.get("artist"), Some("Solo"));
        assert_eq!(t.get("album"), Some("First"));
        assert_eq!(t.get("album-artist"), None);
    }
}

#[cfg(test)]
#[path = "sound_tags_lyrics_tests.rs"]
mod lyrics_tests;
