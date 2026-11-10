//! Where each video was left, and its subtitle and sound settings: kept in the state folder
//! (`$XDG_STATE_HOME/specular/playback.json` on Linux; `logs/..` beside the portable copy),
//! never in tests (they use their own folder).
//!
//! A video is known by its path, size and modification time, so an edited or replaced file
//! starts afresh. Positions are kept only for videos longer than [`MIN_LENGTH_S`] and are
//! dropped near the end; delays and track choices are kept for any video. The oldest
//! entries go past [`MAX_ENTRIES`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// Videos shorter than this start from the beginning (seconds).
pub(crate) const MIN_LENGTH_S: f64 = 180.0;
/// A position this close to the end counts as finished (seconds).
pub(crate) const END_MARGIN_S: f64 = 20.0;
/// A position this close to the start is not worth resuming (seconds).
pub(crate) const START_MARGIN_S: f64 = 10.0;
const MAX_ENTRIES: usize = 2000;
const FILE: &str = "playback.json";

/// What is kept for one video.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Entry {
    /// Where it was left (seconds).
    pub position: Option<f64>,
    pub sub_delay_ms: i64,
    pub sound_delay_ms: i64,
    /// The sound track chosen (its container id).
    pub sound: Option<u64>,
    /// The subtitle chosen (its key, see `subs::Source::key`); `Some("")`: chosen off.
    pub subtitle: Option<String>,
    /// A character set forced for its subtitle file.
    pub charset: Option<String>,
    /// The picture's turn on top of the file's own (an EXIF orientation, 2 to 8).
    pub turn: Option<u8>,
    /// When it was last used (seconds since 1970).
    pub used: u64,
}

impl Entry {
    fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(p) = self.position {
            m.insert("position".into(), json!(p));
        }
        if self.sub_delay_ms != 0 {
            m.insert("sub_delay_ms".into(), json!(self.sub_delay_ms));
        }
        if self.sound_delay_ms != 0 {
            m.insert("sound_delay_ms".into(), json!(self.sound_delay_ms));
        }
        if let Some(s) = self.sound {
            m.insert("sound".into(), json!(s));
        }
        if let Some(s) = &self.subtitle {
            m.insert("subtitle".into(), json!(s));
        }
        if let Some(c) = &self.charset {
            m.insert("charset".into(), json!(c));
        }
        if let Some(t) = self.turn {
            m.insert("turn".into(), json!(t));
        }
        m.insert("used".into(), json!(self.used));
        Value::Object(m)
    }

    fn from_json(v: &Value) -> Entry {
        Entry {
            position: v
                .get("position")
                .and_then(Value::as_f64)
                .filter(|p| p.is_finite() && *p >= 0.0),
            sub_delay_ms: v.get("sub_delay_ms").and_then(Value::as_i64).unwrap_or(0),
            sound_delay_ms: v.get("sound_delay_ms").and_then(Value::as_i64).unwrap_or(0),
            sound: v.get("sound").and_then(Value::as_u64),
            subtitle: v.get("subtitle").and_then(Value::as_str).map(str::to_owned),
            charset: v.get("charset").and_then(Value::as_str).map(str::to_owned),
            turn: v
                .get("turn")
                .and_then(Value::as_u64)
                .filter(|t| (2..=8).contains(t))
                .map(|t| t as u8),
            used: v.get("used").and_then(Value::as_u64).unwrap_or(0),
        }
    }

    fn is_empty(&self) -> bool {
        self.position.is_none()
            && self.sub_delay_ms == 0
            && self.sound_delay_ms == 0
            && self.sound.is_none()
            && self.subtitle.is_none()
            && self.charset.is_none()
            && self.turn.is_none()
    }
}

/// The position to keep for a video of `duration` seconds left at `pos`.
pub(crate) fn keep_position(pos: f64, duration: f64) -> Option<f64> {
    (duration >= MIN_LENGTH_S && pos >= START_MARGIN_S && pos < duration - END_MARGIN_S)
        .then_some(pos)
}

/// FNV-1a over the bytes: a stable key across runs and versions.
fn fnv(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for &b in *p {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The key of the file at `path` (its path, size and modification time).
pub(crate) fn key(path: &Path) -> Option<String> {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let p = path.to_string_lossy();
    Some(format!(
        "{:016x}",
        fnv(&[p.as_bytes(), &md.len().to_le_bytes(), &mtime.to_le_bytes()])
    ))
}

fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The playback memory.
#[derive(Debug, Default)]
pub(crate) struct Store {
    /// The folder (`None`: nothing is read or written).
    dir: Option<PathBuf>,
    entries: HashMap<String, Entry>,
    loaded: bool,
}

impl Store {
    pub(crate) fn at(dir: Option<PathBuf>) -> Store {
        Store {
            dir,
            entries: HashMap::new(),
            loaded: false,
        }
    }

    /// The state folder for this process (tests: their own temporary folder).
    pub(crate) fn default_dir() -> Option<PathBuf> {
        if cfg!(test) {
            return Some(
                std::env::temp_dir().join(format!("specular-playback-test-{}", std::process::id())),
            );
        }
        let l = specular_settings::Locations::detect();
        l.log_dir.parent().map(Path::to_path_buf)
    }

    fn file(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(FILE))
    }

    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(text) = self.file().and_then(|f| std::fs::read_to_string(f).ok()) else {
            return;
        };
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        if let Some(Value::Object(e)) = m.get("videos") {
            self.entries = e
                .iter()
                .map(|(k, v)| (k.clone(), Entry::from_json(v)))
                .collect();
        }
    }

    fn save(&mut self) {
        let Some(file) = self.file() else { return };
        if self.entries.len() > MAX_ENTRIES {
            let mut by_age: Vec<(u64, String)> = self
                .entries
                .iter()
                .map(|(k, e)| (e.used, k.clone()))
                .collect();
            by_age.sort();
            for (_, k) in by_age.iter().take(self.entries.len() - MAX_ENTRIES) {
                self.entries.remove(k);
            }
        }
        let videos: Map<String, Value> = self
            .entries
            .iter()
            .map(|(k, e)| (k.clone(), e.to_json()))
            .collect();
        let text =
            serde_json::to_string(&json!({ "version": 1, "videos": videos })).unwrap_or_default();
        if let Some(dir) = &self.dir {
            let _ = std::fs::create_dir_all(dir);
        }
        // Written beside and renamed: a crash never leaves half a file.
        let tmp = file.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &file);
        }
    }

    pub(crate) fn get(&mut self, key: &str) -> Option<Entry> {
        self.load();
        self.entries.get(key).cloned()
    }

    /// Stores `e` for `key` (an empty entry removes it).
    pub(crate) fn put(&mut self, key: &str, mut e: Entry) {
        self.load();
        if e.is_empty() {
            if self.entries.remove(key).is_some() {
                self.save();
            }
            return;
        }
        e.used = now_s();
        if self.entries.get(key).is_some_and(|old| {
            Entry {
                used: e.used,
                ..old.clone()
            } == e
        }) {
            return;
        }
        self.entries.insert(key.to_owned(), e);
        self.save();
    }

    /// Forgets every position (delays and track choices stay).
    pub(crate) fn forget_positions(&mut self) -> usize {
        self.load();
        let mut n = 0;
        for e in self.entries.values_mut() {
            if e.position.take().is_some() {
                n += 1;
            }
        }
        self.entries.retain(|_, e| !e.is_empty());
        self.save();
        n
    }

    pub(crate) fn positions(&mut self) -> usize {
        self.load();
        self.entries
            .values()
            .filter(|e| e.position.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("resume-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn positions_round_trip_and_forget() {
        let d = dir("rt");
        let mut s = Store::at(Some(d.clone()));
        let e = Entry {
            position: Some(754.5),
            sub_delay_ms: 350,
            sound_delay_ms: -120,
            sound: Some(2),
            subtitle: Some("file:Film.he.srt".into()),
            turn: Some(6),
            ..Entry::default()
        };
        s.put("k1", e.clone());
        s.put(
            "k2",
            Entry {
                position: Some(30.0),
                ..Entry::default()
            },
        );
        let mut again = Store::at(Some(d.clone()));
        let got = again.get("k1").unwrap();
        assert_eq!(Entry { used: 0, ..got }, e);
        assert_eq!(again.positions(), 2);
        assert_eq!(again.forget_positions(), 2);
        let mut third = Store::at(Some(d.clone()));
        assert_eq!(third.positions(), 0);
        assert_eq!(third.get("k1").unwrap().sub_delay_ms, 350, "delays stay");
        assert!(third.get("k2").is_none(), "an entry with nothing left goes");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn what_is_worth_keeping() {
        assert_eq!(keep_position(90.0, 120.0), None, "short videos start over");
        assert_eq!(keep_position(754.0, 3600.0), Some(754.0));
        assert_eq!(
            keep_position(3590.0, 3600.0),
            None,
            "near the end: finished"
        );
        assert_eq!(keep_position(3.0, 3600.0), None, "near the start");
    }

    #[test]
    fn keys_follow_the_file() {
        let d = dir("key");
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("v.mkv");
        std::fs::write(&f, b"one").unwrap();
        let a = key(&f).unwrap();
        assert_eq!(key(&f).unwrap(), a);
        std::fs::write(&f, b"longer").unwrap();
        assert_ne!(key(&f).unwrap(), a, "a changed file is another video");
        assert!(key(&d.join("missing")).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn no_folder_no_files() {
        let mut s = Store::at(None);
        s.put(
            "k",
            Entry {
                sub_delay_ms: 5,
                ..Entry::default()
            },
        );
        assert_eq!(s.get("k").unwrap().sub_delay_ms, 5);
    }

    #[test]
    fn oldest_entries_go() {
        let d = dir("cap");
        let mut s = Store::at(Some(d.clone()));
        s.load();
        for i in 0..MAX_ENTRIES + 5 {
            s.entries.insert(
                format!("{i}"),
                Entry {
                    sub_delay_ms: 1,
                    used: i as u64,
                    ..Entry::default()
                },
            );
        }
        s.save();
        assert_eq!(s.entries.len(), MAX_ENTRIES);
        assert!(!s.entries.contains_key("0"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
