//! PLS: an INI file with a `[playlist]` section of `FileN=`, `TitleN=` and `LengthN=` (seconds,
//! −1 unknown) keys, numbered from 1; items play in number order.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{Entry, Playlist, locate};

pub(crate) fn read(text: &str, base: &Path) -> Playlist {
    #[derive(Default)]
    struct Row {
        file: Option<String>,
        title: Option<String>,
        length: Option<f64>,
    }
    let mut rows: BTreeMap<u32, Row> = BTreeMap::new();
    let mut in_playlist = true;
    for line in text.lines() {
        let line = line.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        if line.starts_with('[') {
            in_playlist = line.eq_ignore_ascii_case("[playlist]");
            continue;
        }
        if !in_playlist || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        let split = key.find(|c: char| c.is_ascii_digit()).unwrap_or(key.len());
        let Ok(n) = key[split..].parse::<u32>() else {
            continue;
        };
        let row = rows.entry(n).or_default();
        match &key[..split] {
            "file" => row.file = Some(value.to_owned()),
            "title" if !value.is_empty() => row.title = Some(value.to_owned()),
            "length" => {
                row.length = value
                    .parse::<f64>()
                    .ok()
                    .filter(|l| l.is_finite() && *l >= 0.0)
            }
            _ => {}
        }
    }
    let entries = rows
        .into_values()
        .filter_map(|r| {
            Some(Entry {
                location: locate(r.file.as_deref()?, base)?,
                title: r.title,
                duration: r.length,
            })
        })
        .collect();
    Playlist {
        title: None,
        entries,
    }
}
