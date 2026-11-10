//! Repeated identical lines are rate-limited: a bad file met by every pass of a loop (a tile
//! decode failing 60 times a second, a folder watch reporting the same error on each event)
//! must not fill the log and push the lines that explain it out of the rotated file.
//!
//! A line repeats another when it comes from the same place in the code (file and line) with
//! the same component, level, message and fields. The first [`BURST`] of each such line in a
//! [`WINDOW`] are written; the rest are counted, and the count is written as a summary of its
//! own ("… (repeated 95 more times)") when the window is over: before the line's next
//! occurrence, or at the next line written at all once a second has passed (so a burst that
//! stops is still summed up). The first occurrence of a distinct line is always written.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tracing::Level;

/// Identical lines written per window.
pub(crate) const BURST: u32 = 5;
pub(crate) const WINDOW: Duration = Duration::from_secs(10);
/// Distinct lines followed at once (the oldest is forgotten beyond this, its count written).
const KEEP: usize = 512;
/// How often the counts of lines that stopped repeating are looked for.
const SWEEP: Duration = Duration::from_secs(1);

/// One line as the limiter sees it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line<'a> {
    /// Where in the code: `file:line` (empty when unknown).
    pub site: &'a str,
    pub target: &'a str,
    pub level: Level,
    /// The message and fields, as written.
    pub text: &'a str,
}

/// Lines left out, to be written as one summary line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Summary {
    pub target: String,
    pub level: Level,
    pub text: String,
    pub repeated: u64,
}

impl Summary {
    /// What follows the line's own text.
    pub(crate) fn suffix(&self) -> String {
        format!(" (repeated {} more times)", self.repeated)
    }
}

/// What to do with a line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Verdict {
    /// Write the line (else it is counted).
    pub write: bool,
    /// Summaries to write first.
    pub summaries: Vec<Summary>,
}

struct Seen {
    since: Instant,
    written: u32,
    dropped: u64,
    target: String,
    level: Level,
    text: String,
}

impl Seen {
    fn summary(&mut self) -> Option<Summary> {
        let n = std::mem::take(&mut self.dropped);
        (n > 0).then(|| Summary {
            target: self.target.clone(),
            level: self.level,
            text: self.text.clone(),
            repeated: n,
        })
    }
}

#[derive(Default)]
pub(crate) struct Limiter {
    seen: HashMap<u64, Seen>,
    swept: Option<Instant>,
}

fn key_of(l: &Line<'_>) -> u64 {
    let mut h = DefaultHasher::new();
    (l.site, l.target, l.level.as_str(), l.text).hash(&mut h);
    h.finish()
}

impl Limiter {
    pub(crate) fn check(&mut self, line: Line<'_>, now: Instant) -> Verdict {
        let key = key_of(&line);
        let mut summaries = Vec::new();
        // Counts of lines that stopped repeating, now and then.
        if self.swept.is_none_or(|t| now.duration_since(t) >= SWEEP) {
            self.swept = Some(now);
            for (k, s) in &mut self.seen {
                if *k != key && now.duration_since(s.since) >= WINDOW {
                    summaries.extend(s.summary());
                }
            }
        }
        if self.seen.len() >= KEEP && !self.seen.contains_key(&key) {
            self.seen.retain(|_, s| {
                let keep = now.duration_since(s.since) < WINDOW;
                if !keep {
                    summaries.extend(s.summary());
                }
                keep
            });
            if self.seen.len() >= KEEP
                && let Some(oldest) = self
                    .seen
                    .iter()
                    .min_by_key(|(_, s)| s.since)
                    .map(|(k, _)| *k)
                && let Some(mut s) = self.seen.remove(&oldest)
            {
                summaries.extend(s.summary());
            }
        }
        let s = self.seen.entry(key).or_insert_with(|| Seen {
            since: now,
            written: 0,
            dropped: 0,
            target: line.target.to_owned(),
            level: line.level,
            text: line.text.to_owned(),
        });
        let write = if now.duration_since(s.since) >= WINDOW {
            summaries.extend(s.summary());
            s.since = now;
            s.written = 1;
            true
        } else if s.written < BURST {
            s.written += 1;
            true
        } else {
            s.dropped += 1;
            false
        };
        Verdict { write, summaries }
    }
}

static LIMITER: LazyLock<Mutex<Limiter>> = LazyLock::new(Mutex::default);

/// The process-wide verdict for `line`.
pub(crate) fn check(line: Line<'_>) -> Verdict {
    LIMITER.lock().check(line, Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at<'a>(site: &'a str, text: &'a str) -> Line<'a> {
        Line {
            site,
            target: "decode",
            level: Level::WARN,
            text,
        }
    }

    fn written(v: &Verdict) -> bool {
        v.write
    }

    #[test]
    fn a_burst_then_a_summary() {
        let mut l = Limiter::default();
        let t = Instant::now();
        let bad = at("a.rs:1", "tile 3: bad");
        for _ in 0..BURST {
            assert!(written(&l.check(bad, t)));
        }
        for _ in 0..95 {
            let v = l.check(bad, t);
            assert!(!v.write && v.summaries.is_empty());
        }
        // Other lines are not held back meanwhile.
        assert!(written(&l.check(at("a.rs:1", "tile 4: bad"), t)));
        // The window is over: the count first, then the line.
        let v = l.check(bad, t + WINDOW);
        assert!(v.write);
        assert_eq!(
            v.summaries,
            vec![Summary {
                target: "decode".into(),
                level: Level::WARN,
                text: "tile 3: bad".into(),
                repeated: 95,
            }]
        );
        assert_eq!(v.summaries[0].suffix(), " (repeated 95 more times)");
        let v = l.check(bad, t + WINDOW);
        assert!(v.write && v.summaries.is_empty());
    }

    #[test]
    fn the_same_text_from_two_places_is_two_lines() {
        let mut l = Limiter::default();
        let t = Instant::now();
        for _ in 0..BURST {
            assert!(l.check(at("a.rs:1", "failed"), t).write);
        }
        assert!(!l.check(at("a.rs:1", "failed"), t).write);
        // Another call site says the same: its first occurrence is written.
        assert!(l.check(at("b.rs:9", "failed"), t).write);
        // So is another level or component.
        let mut e = at("a.rs:1", "failed");
        e.level = Level::ERROR;
        assert!(l.check(e, t).write);
    }

    #[test]
    fn a_burst_that_stops_is_summed_up_by_the_next_line() {
        let mut l = Limiter::default();
        let t = Instant::now();
        let bad = at("a.rs:1", "watch: no space");
        for _ in 0..(BURST + 40) {
            l.check(bad, t);
        }
        // Within the window: nothing to sum up yet.
        let v = l.check(at("c.rs:2", "other"), t + Duration::from_secs(2));
        assert!(v.summaries.is_empty());
        // After it, any line brings the count out, once.
        let v = l.check(at("c.rs:3", "later"), t + WINDOW + SWEEP);
        assert!(v.write);
        assert_eq!(v.summaries.len(), 1);
        assert_eq!(v.summaries[0].repeated, 40);
        let v = l.check(at("c.rs:3", "later"), t + WINDOW + SWEEP * 3);
        assert!(v.summaries.is_empty());
    }

    #[test]
    fn many_distinct_lines_stay_bounded_and_are_all_written_once() {
        let mut l = Limiter::default();
        let t = Instant::now();
        let mut summed = 0;
        for i in 0..5000 {
            let text = format!("line {i}");
            for _ in 0..(BURST + 2) {
                let v = l.check(at("a.rs:1", &text), t);
                summed += v.summaries.iter().map(|s| s.repeated).sum::<u64>();
            }
        }
        assert!(l.seen.len() <= KEEP);
        // A forgotten line's count is written as it goes, not lost.
        let last: u64 = l.seen.values().map(|s| s.dropped).sum();
        assert_eq!(summed + last, 5000 * 2);
    }
}
