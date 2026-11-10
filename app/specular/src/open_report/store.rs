//! The advisor's history across sessions (DESIGN.md §12.7): the slow opens of the past week,
//! so a pattern can span sessions ("five slow HEIC opens this week").
//!
//! Only opens the user waited for are kept ([`worth_keeping`]), at most
//! [`advisor::HISTORY`](super::advisor::HISTORY) of them and none older than [`MAX_AGE_S`],
//! as one tab-separated line each in a small text file in the cache folder. A line holds what
//! the advisor reads (the stage times, the bottleneck, why the decode ran in software and the
//! selector's estimate for the hardware decoder) and nothing about the file but its format and
//! size: no names, no paths. It stays on this computer.
//!
//! Version 2 of the file also keeps the insights the user hid with the indicator's × (their
//! pattern keys, such as `slow-storage`), so they stay hidden after a restart until "Show
//! hidden insights again". A hide is about one context ([`Scope`]): the viewer's opens or the
//! gallery's thumbnails, named in a fourth column (`hidden<TAB>key<TAB>when<TAB>opens`). A
//! line without it (written before hides were scoped) hides the key in both. A version 1 file
//! is still read (its records; it had no hidden insights) and replaced by version 2 at the
//! next save.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;

use specular_core::paths::{Engine, Failure, Mode, Reason, Task, Trust};

use super::advisor::HISTORY;
use super::{
    DecodeStage, Finding, HwState, PathFacts, SLOW_MS, Software, Stage, Timeline, Verdict,
};

/// The file's name in the cache folder.
pub const FILE: &str = "advisor-v2.txt";
/// The name version 1 had: read when there is no version 2 file yet.
pub const FILE_V1: &str = "advisor-v1.txt";
/// Opens older than this (seconds) are forgotten: a week.
pub const MAX_AGE_S: u64 = 7 * 24 * 3600;
/// Hidden insights kept at most (the oldest go first); they do not expire.
pub const MAX_HIDDEN: usize = 64;
const HEADER: &str =
    "# Specular advisor history v2 (slow opens of the past week, hidden insights; local only)";
const HEADER_V1: &str = "# Specular advisor history v1 (slow opens of the past week; local only)";
/// The first column of a hidden insight's line: `hidden<TAB>key<TAB>when[<TAB>scope]`.
const HIDDEN: &str = "hidden";

/// Where a hidden insight stays hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The viewer's indicator, about opening images.
    Opens,
    /// The gallery's indicator, about making thumbnails.
    Thumbnails,
    /// Both (hidden before hides were scoped).
    Both,
}

impl Scope {
    /// The context of an indicator: thumbnails (the gallery's) or opens (the viewer's).
    pub const fn of(thumbnails: bool) -> Scope {
        if thumbnails {
            Scope::Thumbnails
        } else {
            Scope::Opens
        }
    }

    /// Whether a hide in this scope covers `other`'s context.
    pub fn covers(self, other: Scope) -> bool {
        self == Scope::Both || self == other
    }

    const fn key(self) -> Option<&'static str> {
        match self {
            Scope::Opens => Some("opens"),
            Scope::Thumbnails => Some("thumbnails"),
            Scope::Both => None,
        }
    }

    fn parse(s: Option<&str>) -> Option<Scope> {
        match s {
            None => Some(Scope::Both),
            Some("opens") => Some(Scope::Opens),
            Some("thumbnails") => Some(Scope::Thumbnails),
            Some(_) => None,
        }
    }
}

/// An insight the user hid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hidden {
    /// Its pattern key (`slow-storage`).
    pub key: String,
    /// When (Unix seconds).
    pub at: u64,
    pub scope: Scope,
}

/// One remembered open, at `at` (Unix seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub at: u64,
    pub t: Timeline,
    pub v: Verdict,
}

/// An open worth keeping: one the user waited for (slow, not prepared in the background, not
/// from the display cache), the only kind the advisor's patterns count.
pub fn worth_keeping(t: &Timeline, v: &Verdict) -> bool {
    v.total_ms >= SLOW_MS && !t.prefetched && !t.decode.is_some_and(|d| d.from_cache)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The records of the past week kept in memory, oldest first, and the hidden insights.
#[derive(Debug, Default)]
pub struct Store {
    pub records: VecDeque<Record>,
    /// The insights the user hid, oldest first.
    pub hidden: Vec<Hidden>,
}

impl Store {
    pub const fn new() -> Store {
        Store {
            records: VecDeque::new(),
            hidden: Vec::new(),
        }
    }

    /// Hides the insight with pattern key `key` in `scope` from `at` on; false if it already
    /// was. Hidden in one context and then in the other, it is hidden in both.
    pub fn hide(&mut self, key: &str, scope: Scope, at: u64) -> bool {
        let key = clean(key);
        if key.is_empty()
            || self
                .hidden
                .iter()
                .any(|h| h.key == key && h.scope.covers(scope))
        {
            return false;
        }
        let scope = match self.hidden.iter().position(|h| h.key == key) {
            // The other context's: one line for both.
            Some(i) => {
                self.hidden.remove(i);
                Scope::Both
            }
            None => scope,
        };
        if self.hidden.len() == MAX_HIDDEN {
            self.hidden.remove(0);
        }
        self.hidden.push(Hidden { key, at, scope });
        true
    }

    /// Shows the insights hidden in `scope`'s context again (both: every hidden insight);
    /// one hidden in both contexts stays hidden in the other. False if nothing changed.
    pub fn unhide(&mut self, scope: Scope) -> bool {
        let before = self.hidden.clone();
        self.hidden.retain_mut(|h| match (scope, h.scope) {
            (Scope::Both, _) => false,
            (_, Scope::Both) => {
                h.scope = match scope {
                    Scope::Opens => Scope::Thumbnails,
                    _ => Scope::Opens,
                };
                true
            }
            (s, hs) => s != hs,
        });
        self.hidden != before
    }

    /// The pattern keys hidden in `scope`'s context (both: in either), oldest first.
    pub fn hidden_keys(&self, scope: Scope) -> Vec<String> {
        self.hidden
            .iter()
            .filter(|h| scope == Scope::Both || h.scope.covers(scope))
            .map(|h| h.key.clone())
            .collect()
    }

    /// Adds `r` (dropping the oldest beyond [`HISTORY`] and those older than a week).
    pub fn push(&mut self, r: Record) {
        self.records.push_back(r);
        self.trim(r.at);
    }

    fn trim(&mut self, now: u64) {
        self.records
            .retain(|r| r.at <= now && now - r.at <= MAX_AGE_S);
        while self.records.len() > HISTORY {
            self.records.pop_front();
        }
    }
}

/// Reads the history at `path`: the records of the week before `now`, oldest first, and the
/// hidden insights. When `path` is a version 2 file ([`FILE`]) that does not exist yet, the
/// version 1 file next to it is read instead. A missing or unreadable file, or lines from
/// another version, give nothing.
pub fn load(path: &Path, now: u64) -> Store {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && path.file_name().is_some_and(|n| n == FILE) =>
        {
            match std::fs::read_to_string(path.with_file_name(FILE_V1)) {
                Ok(t) => t,
                Err(_) => return Store::new(),
            }
        }
        Err(_) => return Store::new(),
    };
    parse(&text, now)
}

/// The contents of a version 1 or 2 file.
fn parse(text: &str, now: u64) -> Store {
    let mut s = Store::new();
    let mut lines = text.lines();
    let v2 = match lines.next() {
        Some(HEADER) => true,
        Some(HEADER_V1) => false,
        _ => return s,
    };
    let mut records = Vec::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix(HIDDEN).and_then(|r| r.strip_prefix('\t')) {
            let mut c = rest.split('\t');
            if v2
                && let (Some(key), Some(at)) = (c.next(), c.next())
                && let Ok(at) = at.parse()
                && let Some(scope) = Scope::parse(c.next())
                && c.next().is_none()
            {
                s.hide(key, scope, at);
            }
        } else if let Some(r) = decode(line) {
            records.push(r);
        }
    }
    records.sort_by_key(|r| r.at);
    s.records = records.into();
    s.trim(now);
    s
}

/// Writes `store` to `path` (creating its folder; through a temporary file, so a crash never
/// leaves half a file). A version 1 file next to a version 2 one is removed: it is replaced.
pub fn save(path: &Path, store: &Store) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = String::with_capacity(96 + store.records.len() * 120 + store.hidden.len() * 40);
    out.push_str(HEADER);
    out.push('\n');
    for r in &store.records {
        out.push_str(&encode(r));
        out.push('\n');
    }
    for h in &store.hidden {
        out.push_str(&format!("{HIDDEN}\t{}\t{}", clean(&h.key), h.at));
        if let Some(scope) = h.scope.key() {
            out.push('\t');
            out.push_str(scope);
        }
        out.push('\n');
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(out.as_bytes())?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    if path.file_name().is_some_and(|n| n == FILE) {
        let _ = std::fs::remove_file(path.with_file_name(FILE_V1));
    }
    Ok(())
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn opt(s: Option<&str>) -> &str {
    s.filter(|s| !s.is_empty()).unwrap_or("-")
}

/// One line: the time, the format and size, the seven stage times, the total, the
/// bottleneck, why the decode ran in software, and the hardware decoder's estimate.
pub fn encode(r: &Record) -> String {
    let t = &r.t;
    let v = &r.v;
    let st = t.stages();
    let path = t.decode.and_then(|d| d.path);
    let (sw, api, codec) = match v.software {
        None => ("-", None, None),
        Some(s @ Software::NoDevice { api }) => (s.key(), Some(api), None),
        Some(s @ Software::NoCodec { api, codec }) => (s.key(), Some(api), Some(codec)),
        Some(s) => (s.key(), None, None),
    };
    let mut cols = vec![
        r.at.to_string(),
        clean(opt(Some(t.format))),
        t.bytes.to_string(),
        u8::from(t.in_archive).to_string(),
    ];
    cols.extend(st.iter().map(|(_, ms)| format!("{:.1}", ms.max(0.0))));
    cols.push(format!("{:.1}", v.total_ms));
    match v.bottleneck {
        Some((s, p)) => {
            cols.push(s.key().into());
            cols.push(p.to_string());
        }
        None => {
            cols.push("-".into());
            cols.push("0".into());
        }
    }
    cols.push(sw.into());
    cols.push(clean(opt(api)));
    cols.push(clean(opt(codec)));
    cols.push(opt(path.map(|p| p.task.key())).into());
    cols.push(
        path.and_then(|p| p.alt_est_ms)
            .map_or("-".into(), |e| format!("{e:.1}")),
    );
    cols.push(u8::from(path.is_some_and(|p| p.measured)).to_string());
    cols.join("\t")
}

/// A name read back as the `&'static str` the reports carry: the known ones as they are,
/// others (formats) kept once each, at most a few hundred.
pub(super) fn intern(s: &str) -> &'static str {
    const KNOWN: [&str; 6] = [
        "JPEG",
        "HEVC",
        "HEIC",
        "VA-API",
        "Media Foundation",
        "VideoToolbox",
    ];
    if let Some(k) = KNOWN.iter().find(|k| **k == s) {
        return k;
    }
    static SEEN: parking_lot::Mutex<Vec<&'static str>> = parking_lot::Mutex::new(Vec::new());
    let mut seen = SEEN.lock();
    if let Some(k) = seen.iter().find(|k| **k == s) {
        return k;
    }
    if seen.len() >= 256 {
        return "?";
    }
    let k: &'static str = Box::leak(s.to_owned().into_boxed_str());
    seen.push(k);
    k
}

fn stage_of(key: &str) -> Option<Stage> {
    [
        Stage::Startup,
        Stage::Read,
        Stage::Parse,
        Stage::Colour,
        Stage::Decode,
        Stage::Upload,
        Stage::Waiting,
    ]
    .into_iter()
    .find(|s| s.key() == key)
}

fn software_of(
    key: &str,
    api: Option<&'static str>,
    codec: Option<&'static str>,
) -> Option<Software> {
    Some(match key {
        "not-probed" => Software::NotProbed,
        "no-device" => Software::NoDevice { api: api? },
        "no-codec" => Software::NoCodec {
            api: api?,
            codec: codec?,
        },
        "blocked-mismatch" => Software::Blocked(Failure::CheckFailed),
        "blocked-error" => Software::Blocked(Failure::Error),
        "blocked-lost" => Software::Blocked(Failure::DeviceLost),
        "failed" => Software::Failed,
        "busy" => Software::Busy,
        "unsupported" => Software::Unsupported,
        "unavailable" => Software::Unavailable,
        "cpu-only" => Software::CpuOnly,
        "setting-off" => Software::SettingOff,
        "unchecked" => Software::Unchecked,
        "lower-power" => Software::LowerPower,
        "faster" => Software::Faster {
            cpu_ms: 0.0,
            hw_ms: 0.0,
        },
        "scaled" => Software::Scaled,
        _ => return None,
    })
}

/// Reads a line written by [`encode`]: a timeline and verdict with what the advisor reads (the
/// rest is left at its default; the finding is the bottleneck stage).
pub fn decode(line: &str) -> Option<Record> {
    let c: Vec<&str> = line.split('\t').collect();
    if c.len() != 20 {
        return None;
    }
    let f = |i: usize| -> Option<f64> { c[i].parse::<f64>().ok().filter(|x| x.is_finite()) };
    let named = |i: usize| (c[i] != "-").then(|| intern(c[i]));
    let at = c[0].parse().ok()?;
    let ms: Vec<f64> = (4..11).map(f).collect::<Option<_>>()?;
    let total_ms = f(11)?;
    let bottleneck = match c[12] {
        "-" => None,
        k => Some((stage_of(k)?, c[13].parse().ok()?)),
    };
    let software = match c[14] {
        "-" => None,
        k => Some(software_of(k, named(15), named(16))?),
    };
    let task = match c[17] {
        "-" => None,
        k => Some(Task::ALL.into_iter().find(|t| t.key() == k)?),
    };
    let alt_est_ms = match c[18] {
        "-" => None,
        _ => Some(f(18)?),
    };
    let path = task.map(|task| PathFacts {
        task,
        engine: Engine::Cpu,
        reason: Reason::Only,
        fallback: None,
        checked: false,
        alt_engine: Some(Engine::HwDecoder),
        alt_trust: Trust::Absent,
        alt_blocked: None,
        cpu_est_ms: None,
        alt_est_ms,
        measured: c[19] == "1",
        mode: Mode::Automatic,
        forced_off: false,
        hw: HwState::NotProbed,
        scaled: None,
    });
    let t = Timeline {
        format: intern(c[1]),
        bytes: c[2].parse().ok()?,
        in_archive: c[3] == "1",
        startup_ms: ms[0],
        read_ms: ms[1],
        parse_ms: ms[2],
        colour_ms: ms[3],
        decode: Some(DecodeStage {
            scale: 1,
            ms: ms[4],
            path,
            ..DecodeStage::default()
        }),
        upload_ms: ms[5],
        sharp_ms: ms.iter().sum(),
        ..Timeline::default()
    };
    let v = Verdict {
        total_ms,
        bottleneck,
        finding: match bottleneck {
            Some((stage, percent)) => Finding::Stage { stage, percent },
            None => Finding::Fast,
        },
        hint: None,
        software,
    };
    Some(Record { at, t, v })
}

#[cfg(test)]
mod tests {
    use super::super::advisor::{self, Pattern};
    use super::super::verdict;
    use super::*;
    use std::path::PathBuf;

    /// A private folder under the build's target folder (never the user's cache).
    fn dir(name: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-data/advisor")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn heic(decode_ms: f64) -> (Timeline, Verdict) {
        let p = PathFacts {
            task: Task::HeicDecode,
            engine: Engine::Cpu,
            reason: Reason::Only,
            fallback: None,
            checked: false,
            alt_engine: Some(Engine::HwDecoder),
            alt_trust: Trust::Absent,
            alt_blocked: None,
            cpu_est_ms: Some(decode_ms),
            alt_est_ms: Some(100.0),
            measured: false,
            mode: Mode::Automatic,
            forced_off: false,
            hw: HwState::NoCodec { api: "VA-API" },
            scaled: None,
        };
        let t = Timeline {
            format: "HEIC",
            bytes: 3_000_000,
            pixels: 12_000_000,
            read_ms: 2.0,
            parse_ms: 1.0,
            decode: Some(DecodeStage {
                scale: 1,
                ms: decode_ms,
                path: Some(p),
                ..DecodeStage::default()
            }),
            upload_ms: 5.0,
            sharp_ms: decode_ms + 30.0,
            ..Timeline::default()
        };
        (t, verdict(&t))
    }

    fn slow_read(read_ms: f64) -> (Timeline, Verdict) {
        let t = Timeline {
            format: "Radiance HDR",
            bytes: 20_000_000,
            read_ms,
            decode: Some(DecodeStage {
                scale: 1,
                ms: 40.0,
                ..DecodeStage::default()
            }),
            sharp_ms: read_ms + 60.0,
            ..Timeline::default()
        };
        (t, verdict(&t))
    }

    #[test]
    fn a_line_round_trips_what_the_advisor_reads() {
        for (t, v) in [heic(900.0), slow_read(800.0)] {
            let r = Record {
                at: 1_700_000_000,
                t,
                v,
            };
            let back = decode(&encode(&r)).expect("decodes");
            assert_eq!(back.at, r.at);
            assert_eq!(back.t.format, t.format);
            assert_eq!(back.t.bytes, t.bytes);
            assert_eq!(back.v.software, v.software);
            assert_eq!(back.v.bottleneck, v.bottleneck);
            assert!((back.v.total_ms - v.total_ms).abs() < 0.1);
            for ((a, x), (b, y)) in back.t.stages().iter().zip(t.stages()) {
                assert_eq!(*a, b);
                assert!((x - y).abs() < 0.11, "{a:?}: {x} against {y}");
            }
            let (p, q) = (back.t.decode.unwrap().path, t.decode.unwrap().path);
            assert_eq!(p.map(|p| p.task), q.map(|p| p.task));
            assert_eq!(p.and_then(|p| p.alt_est_ms), q.and_then(|p| p.alt_est_ms));
        }
    }

    #[test]
    fn the_advisor_finds_the_same_pattern_in_records_read_back() {
        let reports: Vec<_> = (0..6).map(|i| heic(900.0 + f64::from(i) * 10.0)).collect();
        let before = advisor::advise(&reports);
        let back: Vec<_> = reports
            .iter()
            .map(|&(t, v)| {
                let r = decode(&encode(&Record { at: 1, t, v })).unwrap();
                (r.t, r.v)
            })
            .collect();
        let after = advisor::advise(&back);
        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), 1);
        assert_eq!(before[0].pattern.key(), after[0].pattern.key());
        assert!(matches!(
            after[0].pattern,
            Pattern::Software { count: 6, .. }
        ));
        assert_eq!(before[0].marked, after[0].marked);
        let (g, h) = (before[0].gain.unwrap(), after[0].gain.unwrap());
        assert!((g.low - h.low).abs() < 0.01 && (g.high - h.high).abs() < 0.01);
    }

    #[test]
    fn saved_history_is_read_back_bounded_and_a_week_long() {
        let d = dir("bounded");
        let path = d.join(FILE);
        let now = 2_000_000_000;
        let mut s = Store::new();
        // Two weeks of slow opens, one an hour: only the last week, and at most HISTORY.
        for i in 0..(14 * 24) {
            let (t, v) = heic(900.0);
            s.push(Record {
                at: now - MAX_AGE_S * 2 + i * 3600,
                t,
                v,
            });
        }
        assert_eq!(s.records.len(), HISTORY);
        save(&path, &s).unwrap();
        let back = load(&path, now);
        assert_eq!(back.records.len(), HISTORY);
        // What the advisor reads comes back (the rest of a timeline is not kept).
        for (a, b) in back.records.iter().zip(&s.records) {
            assert_eq!((a.at, a.t.format, a.t.bytes), (b.at, b.t.format, b.t.bytes));
            assert_eq!(
                (a.v.bottleneck, a.v.software),
                (b.v.bottleneck, b.v.software)
            );
            assert!((a.v.total_ms - b.v.total_ms).abs() < 0.1);
        }
        // A week later, nothing is left.
        assert!(
            load(&path, now + MAX_AGE_S + 3600 * 24 * 3)
                .records
                .is_empty()
        );
        // The file stays small.
        let size = std::fs::metadata(&path).unwrap().len();
        assert!(size < 16 * 1024, "{size} bytes");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_foreign_or_damaged_file_gives_nothing_or_what_is_readable() {
        let d = dir("damaged");
        let path = d.join(FILE);
        assert!(load(&path, 10).records.is_empty());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(&path, "something else\n1\t2\n").unwrap();
        assert!(load(&path, 10).records.is_empty());
        let (t, v) = heic(900.0);
        let good = encode(&Record { at: 5, t, v });
        std::fs::write(
            &path,
            format!(
                "{HEADER}\n{good}\nnot a record\n{}\n",
                good.replace("HEVC", "")
            ),
        )
        .unwrap();
        assert_eq!(load(&path, 10).records.len(), 2);
        // A record from the future (the clock went back) is dropped.
        assert!(load(&path, 4).records.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hidden_insights_round_trip_with_the_records() {
        let d = dir("hidden");
        let path = d.join(FILE);
        let mut s = Store::new();
        let (t, v) = heic(900.0);
        s.push(Record { at: 100, t, v });
        assert!(s.hide("slow-storage", Scope::Opens, 90));
        assert!(s.hide("software:HEIC:no-device", Scope::Thumbnails, 95));
        // Hiding twice keeps one; tabs cannot break the line.
        assert!(!s.hide("slow-storage", Scope::Opens, 99));
        assert!(s.hide("odd\tkey", Scope::Opens, 96));
        save(&path, &s).unwrap();
        let back = load(&path, 200);
        assert_eq!(back.records.len(), 1);
        assert_eq!(back.hidden, s.hidden);
        assert_eq!(
            back.hidden_keys(Scope::Both),
            ["slow-storage", "software:HEIC:no-device", "odd key"]
        );
        assert_eq!(back.hidden_keys(Scope::Opens), ["slow-storage", "odd key"]);
        assert_eq!(
            back.hidden_keys(Scope::Thumbnails),
            ["software:HEIC:no-device"]
        );
        assert_eq!(back.hidden[0].at, 90);
        // Hidden insights do not expire with the records.
        let later = load(&path, 100 + MAX_AGE_S * 4);
        assert!(later.records.is_empty());
        assert_eq!(later.hidden.len(), 3);
        // At most MAX_HIDDEN, the oldest going first.
        let mut many = Store::new();
        for i in 0..(MAX_HIDDEN as u64 + 3) {
            many.hide(&format!("k{i}"), Scope::Opens, i);
        }
        assert_eq!(many.hidden.len(), MAX_HIDDEN);
        assert_eq!(many.hidden[0].key, "k3");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A hide is about the context it was made in: the viewer's opens or the gallery's
    /// thumbnails; hidden in both, it is one line.
    #[test]
    fn hides_are_kept_per_context() {
        let mut s = Store::new();
        assert!(s.hide("slow-storage", Scope::Thumbnails, 10));
        assert!(s.hidden_keys(Scope::Opens).is_empty());
        assert_eq!(s.hidden_keys(Scope::Thumbnails), ["slow-storage"]);
        // Hidden in the viewer too: both, in one entry.
        assert!(s.hide("slow-storage", Scope::Opens, 20));
        assert_eq!(s.hidden.len(), 1);
        assert_eq!(s.hidden[0].scope, Scope::Both);
        assert_eq!(s.hidden[0].at, 20);
        assert!(!s.hide("slow-storage", Scope::Thumbnails, 30));
        assert_eq!(s.hidden_keys(Scope::Opens), ["slow-storage"]);
        assert_eq!(s.hidden_keys(Scope::Thumbnails), ["slow-storage"]);
    }

    /// The panel's "Show hidden insights again" is about its own context; Settings' about
    /// both.
    #[test]
    fn unhide_shows_one_context_or_both() {
        let mut s = Store::new();
        s.hide("both", Scope::Opens, 1);
        s.hide("both", Scope::Thumbnails, 2);
        s.hide("opens", Scope::Opens, 3);
        s.hide("thumbs", Scope::Thumbnails, 4);
        // The viewer's: its own go, the shared one stays hidden in the gallery.
        assert!(s.unhide(Scope::Opens));
        assert!(s.hidden_keys(Scope::Opens).is_empty());
        assert_eq!(s.hidden_keys(Scope::Thumbnails), ["both", "thumbs"]);
        assert_eq!(s.hidden[0].scope, Scope::Thumbnails);
        assert_eq!(s.hidden[0].at, 2);
        assert!(!s.unhide(Scope::Opens));
        // The gallery's.
        assert!(s.unhide(Scope::Thumbnails));
        assert!(s.hidden.is_empty());
        assert!(!s.unhide(Scope::Thumbnails));
        // Settings: everything.
        s.hide("both", Scope::Opens, 1);
        s.hide("both", Scope::Thumbnails, 2);
        s.hide("thumbs", Scope::Thumbnails, 4);
        assert!(s.unhide(Scope::Both));
        assert!(s.hidden.is_empty());
        assert!(!s.unhide(Scope::Both));
    }

    /// Hides written before they were scoped (`hidden<TAB>key<TAB>when`) still hide the key
    /// everywhere, and are written back the same way; an unknown scope is not a hide.
    #[test]
    fn unscoped_hides_from_before_cover_both_contexts() {
        let d = dir("unscoped");
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join(FILE);
        std::fs::write(
            &path,
            format!(
                "{HEADER}\nhidden\tslow-storage\t5\nhidden\tslow-archive\t6\tthumbnails\n\
                 hidden\tfirst-open\t7\tsomewhere\nhidden\tx\t8\topens\textra\n"
            ),
        )
        .unwrap();
        let s = load(&path, 10);
        assert_eq!(
            s.hidden,
            [
                Hidden {
                    key: "slow-storage".into(),
                    at: 5,
                    scope: Scope::Both
                },
                Hidden {
                    key: "slow-archive".into(),
                    at: 6,
                    scope: Scope::Thumbnails
                },
            ]
        );
        assert_eq!(s.hidden_keys(Scope::Opens), ["slow-storage"]);
        save(&path, &s).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("hidden\tslow-storage\t5\n"), "{text}");
        assert!(
            text.contains("hidden\tslow-archive\t6\tthumbnails\n"),
            "{text}"
        );
        assert_eq!(load(&path, 10).hidden, s.hidden);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_version_1_file_is_read_and_replaced() {
        let d = dir("v1");
        std::fs::create_dir_all(&d).unwrap();
        let old = d.join(FILE_V1);
        let (t, v) = heic(900.0);
        let line = encode(&Record { at: 50, t, v });
        // Version 1 had no hidden insights: such a line in it is not one.
        std::fs::write(
            &old,
            format!("{HEADER_V1}\n{line}\n{line}\nhidden\tslow-storage\t1\n"),
        )
        .unwrap();
        let path = d.join(FILE);
        let s = load(&path, 60);
        assert_eq!(s.records.len(), 2);
        assert!(s.hidden.is_empty());
        assert_eq!(s.records[0].t.format, "HEIC");
        // A version 2 file wins over it, and saving one removes the old file.
        save(&path, &s).unwrap();
        assert!(!old.exists());
        assert_eq!(load(&path, 60).records.len(), 2);
        // Any other name is read as it is (no fallback).
        assert!(load(&d.join("other.txt"), 60).records.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_waited_opens_are_kept() {
        let (t, v) = heic(900.0);
        assert!(worth_keeping(&t, &v));
        let mut p = t;
        p.prefetched = true;
        assert!(!worth_keeping(&p, &verdict(&p)));
        let (t, v) = heic(10.0);
        assert!(!worth_keeping(&t, &v));
    }
}
