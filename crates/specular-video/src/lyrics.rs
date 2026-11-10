//! A song's lyrics: timed lines (LRC and enhanced LRC, ID3 SYLT) or plain text (ID3 USLT,
//! Vorbis `LYRICS`/`UNSYNCEDLYRICS`, MP4 `©lyr`, APE `Lyrics`).
//!
//! - LRC: `[mm:ss.xx]` (also `[mm:ss]`, `[mm:ss:xx]`, `[mm:ss.xxx]`, `[hh:mm:ss.xx]`) before
//!   a line, several stamps for a repeated line, `[offset:±ms]` (positive: lines come
//!   earlier), other `[tag:value]` lines left out. Enhanced LRC adds `<mm:ss.xx>` before
//!   words. Written from the format's community descriptions (fileformat.com's LRC page,
//!   the A2 Media Player word-time extension).
//! - SYLT: the id3.org v2.3/v2.4 frame description; only times in milliseconds are read
//!   (MPEG-frame times need the stream's frame rate, and taggers do not write them).
//! - A sidecar `.lrc` beside the sound file is read in preference to embedded lyrics.

use std::path::{Path, PathBuf};

/// The largest lyrics read (a sidecar file, or a tag's text).
pub const MAX_BYTES: usize = 1 << 20;

/// A timed word of enhanced LRC.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    /// When it starts (seconds).
    pub at: f64,
    pub text: String,
}

/// A line of lyrics.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// When it starts (seconds); `None` in plain lyrics.
    pub at: Option<f64>,
    pub text: String,
    /// Its words with their times (enhanced LRC; empty otherwise).
    pub words: Vec<Word>,
}

/// Where lyrics came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    Sidecar,
    Tags,
    /// A transcript made on this computer (generated subtitles of the speech or singing).
    Generated,
}

/// A song's lyrics.
#[derive(Clone, Debug, PartialEq)]
pub struct Lyrics {
    /// In time order when synced, else as written.
    pub lines: Vec<Line>,
    pub synced: bool,
    pub from: From,
}

impl Lyrics {
    /// The line sung at `t` seconds: the last that started (none before the first).
    pub fn line_at(&self, t: f64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        let i = self
            .lines
            .partition_point(|l| l.at.is_some_and(|a| a <= t + 1e-9));
        i.checked_sub(1)
    }

    /// How far into its words line `i` is at `t` (0…1 by words: the share of its text sung,
    /// each word filling over its time); `None` without word times.
    pub fn sung(&self, i: usize, t: f64) -> Option<f32> {
        let line = self.lines.get(i)?;
        if line.words.is_empty() {
            return None;
        }
        let total: usize = line.words.iter().map(|w| w.text.chars().count()).sum();
        let end = self
            .lines
            .get(i + 1)
            .and_then(|l| l.at)
            .unwrap_or(f64::INFINITY);
        let mut done = 0.0f64;
        for (k, w) in line.words.iter().enumerate() {
            let n = w.text.chars().count() as f64;
            if t < w.at {
                break;
            }
            let next = line.words.get(k + 1).map_or(end, |x| x.at);
            let span = (next - w.at).clamp(1e-3, 2.0);
            done += n * ((t - w.at) / span).clamp(0.0, 1.0);
        }
        Some((done / total.max(1) as f64).clamp(0.0, 1.0) as f32)
    }

    /// All of the text, a line each (for search).
    pub fn text(&self) -> String {
        let mut s = String::new();
        for l in &self.lines {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&l.text);
        }
        s
    }

    /// Lyrics from text: LRC when it has time stamps, else plain lines. `None` when empty.
    pub fn from_text(text: &str, from: From) -> Option<Lyrics> {
        let lrc = parse_lrc(text, from);
        if lrc.as_ref().is_some_and(|l| l.synced) {
            return lrc;
        }
        plain(text, from)
    }
}

/// Plain lyrics: the lines as written, trailing blank ones dropped.
fn plain(text: &str, from: From) -> Option<Lyrics> {
    let mut lines: Vec<Line> = text
        .lines()
        .map(|l| Line {
            at: None,
            text: l.trim_end().to_owned(),
            words: Vec::new(),
        })
        .collect();
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|l| l.text.is_empty()) {
        lines.remove(0);
    }
    (!lines.is_empty()).then_some(Lyrics {
        lines,
        synced: false,
        from,
    })
}

/// A time stamp's inside: `mm:ss`, `mm:ss.xx`, `mm:ss:xx`, `mm:ss.xxx`, `h:mm:ss.xx`.
fn stamp(s: &str) -> Option<f64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split(':').collect();
    let num = |p: &str| -> Option<f64> {
        let p = p.trim();
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return None;
        }
        p.parse::<f64>().ok()
    };
    let frac = |p: &str| -> Option<f64> {
        let p = p.trim();
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(p.parse::<f64>().ok()? / 10f64.powi(p.len() as i32))
    };
    match parts.as_slice() {
        [m, s] => Some(num(m)? * 60.0 + num(s)?),
        // h:mm:ss.xx
        [h, m, s] if s.contains('.') => Some(num(h)? * 3600.0 + num(m)? * 60.0 + num(s)?),
        // mm:ss:xx (the last part a fraction of a second)
        [m, s, x] => Some(num(m)? * 60.0 + num(s)? + frac(x)?),
        _ => None,
    }
}

/// Parses LRC (and enhanced LRC). `None` when it has no lines.
pub fn parse_lrc(text: &str, from: From) -> Option<Lyrics> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut offset = 0.0;
    let mut timed: Vec<Line> = Vec::new();
    let mut untimed: Vec<String> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        let mut tag = false;
        while let Some(r) = rest.strip_prefix('[') {
            let Some(close) = r.find(']') else { break };
            let inner = &r[..close];
            if let Some(t) = stamp(inner) {
                stamps.push(t);
            } else if let Some((k, v)) = inner.split_once(':') {
                if k.trim().eq_ignore_ascii_case("offset")
                    && let Ok(ms) = v.trim().trim_start_matches('+').parse::<f64>()
                {
                    offset = ms / 1000.0;
                }
                tag = true;
            } else {
                break;
            }
            rest = r[close + 1..].trim_start();
        }
        if stamps.is_empty() {
            if !tag {
                untimed.push(raw.trim_end().to_owned());
            }
            continue;
        }
        let (text, words) = words_of(rest);
        for &at in &stamps {
            timed.push(Line {
                at: Some(at),
                text: text.clone(),
                words: words.clone(),
            });
        }
    }
    if timed.is_empty() {
        return plain(&untimed.join("\n"), from);
    }
    // A positive offset brings the lines earlier.
    for l in &mut timed {
        l.at = l.at.map(|a| (a - offset).max(0.0));
        for w in &mut l.words {
            w.at = (w.at - offset).max(0.0);
        }
    }
    timed.sort_by(|a, b| a.at.unwrap_or(0.0).total_cmp(&b.at.unwrap_or(0.0)));
    Some(Lyrics {
        lines: timed,
        synced: true,
        from,
    })
}

/// A line's text without its word stamps, and its timed words (enhanced LRC).
fn words_of(s: &str) -> (String, Vec<Word>) {
    let mut text = String::new();
    let mut words: Vec<Word> = Vec::new();
    let mut rest = s;
    loop {
        let Some(open) = rest.find('<') else {
            push_text(&mut text, &mut words, rest);
            break;
        };
        let Some(len) = rest[open..].find('>') else {
            push_text(&mut text, &mut words, rest);
            break;
        };
        match stamp(&rest[open + 1..open + len]) {
            Some(t) => {
                push_text(&mut text, &mut words, &rest[..open]);
                words.push(Word {
                    at: t,
                    text: String::new(),
                });
            }
            None => push_text(&mut text, &mut words, &rest[..open + len + 1]),
        }
        rest = &rest[open + len + 1..];
    }
    // A last stamp with no word after it marks the end of the line.
    while words.last().is_some_and(|w| w.text.trim().is_empty()) {
        let w = words.pop();
        if let (Some(w), Some(prev)) = (w, words.last_mut()) {
            prev.text.push_str(&w.text);
        }
    }
    let text = if words.is_empty() {
        text.trim().to_owned()
    } else {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    for w in &mut words {
        w.text = w.text.trim_start().to_owned();
    }
    if let Some(last) = words.last_mut() {
        last.text = last.text.trim_end().to_owned();
    }
    (text, words)
}

fn push_text(text: &mut String, words: &mut [Word], s: &str) {
    text.push_str(s);
    if let Some(w) = words.last_mut() {
        w.text.push_str(s);
    }
}

/// Lyrics from an ID3 SYLT frame's body (times in milliseconds only). Entries starting with
/// a line break start a line; others continue it as words. When no entry has a line break,
/// each is a line.
pub fn parse_sylt(f: &[u8]) -> Option<Lyrics> {
    let (&enc, rest) = f.split_first()?;
    let rest = rest.get(3..)?; // language
    let (&format, rest) = rest.split_first()?;
    if format != 2 {
        return None;
    }
    let rest = rest.get(1..)?; // content type
    let (dend, start) = crate::sound_tags::id3_terminated(enc, rest);
    let _ = dend;
    let mut at = start;
    let mut entries: Vec<(f64, String)> = Vec::new();
    while at < rest.len() {
        let body = &rest[at..];
        let (tend, after) = crate::sound_tags::id3_terminated(enc, body);
        let text = crate::sound_tags::id3_text(enc, &body[..tend]);
        let Some(ms) = body.get(after..after + 4) else {
            break;
        };
        let ms = u32::from_be_bytes([ms[0], ms[1], ms[2], ms[3]]);
        entries.push((f64::from(ms) / 1000.0, text));
        at += after + 4;
    }
    if entries.is_empty() {
        return None;
    }
    let breaks = entries
        .iter()
        .any(|(_, t)| t.starts_with('\n') || t.starts_with('\r'));
    let mut lines: Vec<Line> = Vec::new();
    for (t, text) in entries {
        let starts = !breaks || lines.is_empty() || text.starts_with(['\n', '\r']);
        let clean = text.trim_start_matches(['\n', '\r']).to_owned();
        if starts {
            lines.push(Line {
                at: Some(t),
                text: clean.clone(),
                words: vec![Word { at: t, text: clean }],
            });
        } else if let Some(l) = lines.last_mut() {
            l.text.push_str(&clean);
            l.words.push(Word { at: t, text: clean });
        }
    }
    for l in &mut lines {
        // One word: no word timing to show.
        if l.words.len() < 2 {
            l.words.clear();
        }
        l.text = l.text.trim().to_owned();
    }
    lines.sort_by(|a, b| a.at.unwrap_or(0.0).total_cmp(&b.at.unwrap_or(0.0)));
    Some(Lyrics {
        lines,
        synced: true,
        from: From::Tags,
    })
}

/// Text of a lyrics file: UTF-8 (with or without a byte-order mark), UTF-16 with one, else
/// Latin-1.
pub fn decode_text(b: &[u8]) -> String {
    let b = &b[..b.len().min(MAX_BYTES)];
    match b {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] | [0xFE, 0xFF, rest @ ..] => {
            let be = b[0] == 0xFE;
            let units: Vec<u16> = rest
                .chunks_exact(2)
                .map(|c| {
                    if be {
                        u16::from_be_bytes([c[0], c[1]])
                    } else {
                        u16::from_le_bytes([c[0], c[1]])
                    }
                })
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => match std::str::from_utf8(b) {
            Ok(s) => s.to_owned(),
            Err(_) => b.iter().map(|&c| char::from(c)).collect(),
        },
    }
}

/// The sidecar lyrics file of the sound at `path`: `name.lrc` (or `.LRC`) beside it.
pub fn sidecar_path(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?;
    let dir = path.parent()?;
    for ext in ["lrc", "LRC", "Lrc"] {
        let mut name = stem.to_os_string();
        name.push(".");
        name.push(ext);
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Reads the sidecar lyrics of the sound at `path`, if it has any.
pub fn read_sidecar(path: &Path) -> Option<Lyrics> {
    let p = sidecar_path(path)?;
    let meta = std::fs::metadata(&p).ok()?;
    if meta.len() as usize > MAX_BYTES {
        return None;
    }
    let b = std::fs::read(&p).ok()?;
    Lyrics::from_text(&decode_text(&b), From::Sidecar)
}

#[cfg(test)]
#[path = "lyrics_tests.rs"]
mod tests;
