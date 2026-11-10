//! LRC lyrics (`.lrc`): `[mm:ss.xx]words` lines, a line showing until the next one starts.
//! A line may carry several times (a repeated chorus); `[offset:+250]` shifts every time
//! by milliseconds (positive: earlier); tags like `[ti:…]`, `[ar:…]`, `[al:…]` name the
//! song. Enhanced LRC's word times (`<mm:ss.xx>`) are dropped, the words kept.

use crate::{Body, Cue, Error, Result, Span, TextCue, Track};

/// How long the last line shows.
const LAST_US: i64 = 5_000_000;

/// What the tags say about the song.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// `mm:ss.xx` (also `mm:ss:xx`, `mm:ss`, `h:mm:ss.xx`) in microseconds.
fn time(s: &str) -> Option<i64> {
    let s = s.trim();
    let parts: Vec<&str> = s.split([':', '.']).collect();
    let n = |p: &str| -> Option<i64> {
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse().ok())
            .flatten()
    };
    let (h, m, sec, frac) = match parts.as_slice() {
        [m, s] => (0, n(m)?, n(s)?, ""),
        [m, s, f] if s.len() == 2 => (0, n(m)?, n(s)?, *f),
        [h, m, s, f] => (n(h)?, n(m)?, n(s)?, *f),
        _ => return None,
    };
    if sec >= 60 {
        return None;
    }
    Some(crate::hms_us(h, m, sec, crate::frac_us(frac)?))
}

/// Whether `text` reads as LRC: one of its first lines starts with a time tag.
pub fn sniff(text: &str) -> bool {
    text.lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim())
        .filter(|l| !l.is_empty())
        .take(20)
        .any(|l| !leading(l).0.is_empty())
}

/// The time tags at the start of a line and the rest.
fn leading(mut l: &str) -> (Vec<i64>, &str) {
    let mut times = Vec::new();
    while let Some(rest) = l.strip_prefix('[') {
        let Some((tag, after)) = rest.split_once(']') else {
            break;
        };
        let Some(t) = time(tag) else { break };
        times.push(t);
        l = after;
    }
    (times, l)
}

/// Removes enhanced LRC word times (`<00:12.34>`).
fn strip_word_times(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        match rest[i + 1..].split_once('>') {
            Some((tag, after)) if time(tag).is_some() => rest = after,
            _ => {
                out.push('<');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parses an LRC file: its lines as cues and its tags.
pub fn parse(text: &str) -> Result<(Track, Tags)> {
    let mut tags = Tags::default();
    let mut offset_ms: i64 = 0;
    let mut timed: Vec<(i64, String)> = Vec::new();
    for l in text.lines() {
        let l = l.trim_start_matches('\u{feff}').trim();
        let (times, rest) = leading(l);
        if times.is_empty() {
            // An ID tag: `[ar:Artist]`.
            if let Some(inner) = l.strip_prefix('[').and_then(|r| r.strip_suffix(']'))
                && let Some((k, v)) = inner.split_once(':')
            {
                let v = v.trim();
                let v = (!v.is_empty()).then(|| v.to_owned());
                match k.trim().to_ascii_lowercase().as_str() {
                    "ti" => tags.title = v,
                    "ar" => tags.artist = v,
                    "al" => tags.album = v,
                    "offset" => {
                        offset_ms = v
                            .and_then(|v| v.trim_start_matches('+').parse().ok())
                            .unwrap_or(0)
                    }
                    _ => {}
                }
            }
            continue;
        }
        let words = strip_word_times(rest).trim().to_owned();
        for t in times {
            timed.push(((t - offset_ms * 1000).max(0), words.clone()));
        }
    }
    if timed.is_empty() {
        return Err(Error::Malformed("no timed lyrics".into()));
    }
    timed.sort_by_key(|(t, _)| *t);
    let mut cues = Vec::with_capacity(timed.len());
    for (i, (start, words)) in timed.iter().enumerate() {
        // A blank timed line ends the one before it.
        if words.is_empty() {
            continue;
        }
        let end = timed.get(i + 1).map(|n| n.0).unwrap_or(start + LAST_US);
        cues.push(Cue {
            start_us: *start,
            end_us: end,
            body: Body::Text(TextCue {
                lines: vec![vec![Span::plain(words.clone())]],
                align: 2,
                pos: None,
                margin_v: None,
            }),
        });
    }
    Ok((Track::new(cues), tags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_tags_and_offset() {
        let (t, tags) = parse(
            "[ti:Song]\n[ar:Someone]\n[offset:+500]\n[00:01.50]First\n\
             [00:03.00][00:10.00]Chorus <00:03.50>again\n[00:05.00]\n[00:07]Fourth\n",
        )
        .unwrap();
        assert_eq!(tags.title.as_deref(), Some("Song"));
        assert_eq!(tags.artist.as_deref(), Some("Someone"));
        let words: Vec<String> = t
            .cues
            .iter()
            .map(|c| c.text().unwrap().plain_text())
            .collect();
        assert_eq!(words, ["First", "Chorus again", "Fourth", "Chorus again"]);
        assert_eq!(t.cues[0].start_us, 1_000_000);
        assert_eq!(t.cues[0].end_us, 2_500_000);
        // The blank line at 5 s ends the chorus.
        assert_eq!(t.cues[1].end_us, 4_500_000);
        assert_eq!(t.cues[3].end_us, 9_500_000 + LAST_US);
    }

    #[test]
    fn sniffs() {
        assert!(sniff("[ar:x]\n[00:12.00]la"));
        assert!(!sniff("[Script Info]\nTitle: x"));
        assert!(parse("[ar:x]").is_err());
        assert_eq!(time("1:02:03.5"), Some(3_723_500_000));
        assert_eq!(time("00:61"), None);
    }
}
