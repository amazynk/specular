//! M3U and M3U8: one location per line; `#EXTM3U` extended playlists add `#EXTINF:<seconds>
//! [attributes],<title>` before an item and `#PLAYLIST:<title>`. Other `#` lines are
//! comments or directives this player does not use.

use std::path::Path;

use crate::{Entry, Location, Playlist, locate, relative_to};

pub(crate) fn read(text: &str, base: &Path) -> Playlist {
    let mut out = Playlist::default();
    let mut pending: Option<(Option<f64>, Option<String>)> = None;
    for line in text.lines() {
        let line = line.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        if line.is_empty() {
            continue;
        }
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            pending = Some(extinf(info));
            continue;
        }
        if let Some(t) = line.strip_prefix("#PLAYLIST:") {
            let t = t.trim();
            if !t.is_empty() {
                out.title = Some(t.to_owned());
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some(location) = locate(line, base) else {
            continue;
        };
        let (duration, title) = pending.take().unwrap_or((None, None));
        out.entries.push(Entry {
            location,
            title,
            duration,
        });
    }
    out
}

/// `<seconds>[ key="value"…],<title>`: the length (negative: unknown) and the title.
fn extinf(info: &str) -> (Option<f64>, Option<String>) {
    // The title follows the first comma outside quotes (attribute values may hold commas).
    let mut quoted = false;
    let mut comma = None;
    for (i, c) in info.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                comma = Some(i);
                break;
            }
            _ => {}
        }
    }
    let (head, title) = match comma {
        Some(i) => (&info[..i], Some(info[i + 1..].trim())),
        None => (info, None),
    };
    let secs = head
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s >= 0.0);
    let title = title.filter(|t| !t.is_empty()).map(str::to_owned);
    (secs, title)
}

pub(crate) fn write(title: Option<&str>, entries: &[Entry], dir: &Path) -> String {
    let mut s = String::from("#EXTM3U\n");
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        s.push_str(&format!("#PLAYLIST:{}\n", one_line(t)));
    }
    for e in entries {
        let secs = e.duration.map_or(-1, |d| d.round() as i64);
        let name = e.title.clone().unwrap_or_else(|| e.name());
        s.push_str(&format!("#EXTINF:{secs},{}\n", one_line(&name)));
        let loc = match &e.location {
            Location::File(p) => {
                relative_to(p, dir).unwrap_or_else(|| p.to_string_lossy().into_owned())
            }
            Location::Url(u) => u.clone(),
        };
        s.push_str(&one_line(&loc));
        s.push('\n');
    }
    s
}

fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}
