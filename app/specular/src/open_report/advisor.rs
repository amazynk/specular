//! The advisor (DESIGN.md §12.7): patterns that recur across the session's recent open
//! reports, with the change that would help and an estimate of what it would gain.
//!
//! It only reads what the reports already hold (the [`Timeline`] and its [`Verdict`]), keeps
//! no state but the last [`HISTORY`] reports (this session's, after the slow opens of the past
//! week that [`store`](super::store) kept), and is deterministic: the same sequence of reports
//! gives the same advice. It says something only when a pattern is clear
//! ([`MIN_SAMPLES`] slow opens, [`SHARE`] of them alike) and the estimate is a range from the
//! selector's own predictions and the throughput the session has seen, never a promise.

use super::{Hint, SLOW_MS, SLOW_STORAGE_MB_S, Software, Stage, Timeline, Verdict};
use std::collections::VecDeque;

/// Reports kept.
pub const HISTORY: usize = 64;
/// Slow opens looked at for a pattern (the most recent).
const WINDOW: usize = 20;
/// Slow opens a pattern needs before it is one.
pub const MIN_SAMPLES: usize = 5;
/// Of the slow opens looked at, the share that must agree.
const SHARE: f64 = 0.8;
/// Storage faster than this is not assumed for an estimate (a fast local SSD, MB/s).
const BEST_STORAGE_MB_S: f64 = 1500.0;

/// A pattern across recent opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pattern {
    /// Slow opens of `format` decoded in software for the same reason `why`.
    Software {
        format: &'static str,
        why: Software,
        count: usize,
        of: usize,
    },
    /// Reading the file held up slow opens, at about `mb_s` (median), of files of about
    /// `bytes` (median: too small, and the throughput says nothing; `super::speed`).
    SlowStorage {
        mb_s: f64,
        bytes: u64,
        archive: bool,
        count: usize,
        of: usize,
    },
    /// The session's first open paid for starting up (the window, the GPU, its shaders):
    /// `first_ms` until sharp against a median of `later_ms` since.
    FirstOpen { first_ms: f64, later_ms: f64 },
}

impl Pattern {
    pub fn key(&self) -> String {
        match self {
            Pattern::Software { format, why, .. } => format!("software:{format}:{}", why.key()),
            Pattern::SlowStorage { archive: true, .. } => "slow-archive".into(),
            Pattern::SlowStorage { .. } => "slow-storage".into(),
            Pattern::FirstOpen { .. } => "first-open".into(),
        }
    }
}

/// What would help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// A setting or an action the open report's hint names.
    Hint(Hint),
    /// A graphics card (or driver) with a hardware decoder for `codec`.
    HardwareDecoder { codec: &'static str },
}

/// How many times faster the opens of the pattern would have been: `low` to `high`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gain {
    pub low: f64,
    pub high: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Advice {
    pub pattern: Pattern,
    pub change: Option<Change>,
    pub gain: Option<Gain>,
    /// Worth showing: a change at least doubles the speed, or brings the typical open under a
    /// second.
    pub marked: bool,
}

/// The recent reports, oldest first.
#[derive(Debug)]
pub struct History {
    reports: VecDeque<(Timeline, Verdict)>,
    /// The advice given so far ([`Pattern::key`] and whether it was marked), to log changes.
    told: Vec<(String, bool)>,
    /// From how long (ms) an item counts as one the user waited for.
    slow_ms: f64,
}

impl Default for History {
    fn default() -> History {
        History::new()
    }
}

impl History {
    pub const fn new() -> History {
        History::with_slow(SLOW_MS)
    }

    /// A history of items (such as thumbnails) that count as waited for from `slow_ms`.
    pub const fn with_slow(slow_ms: f64) -> History {
        History {
            reports: VecDeque::new(),
            told: Vec::new(),
            slow_ms,
        }
    }

    /// Adds a report; returns the advice that is new or newly marked since the last report.
    pub fn push(&mut self, t: Timeline, v: Verdict) -> Vec<Advice> {
        if self.reports.len() == HISTORY {
            self.reports.pop_front();
        }
        self.reports.push_back((t, v));
        let now = self.advise();
        let new = now
            .iter()
            .filter(|a| !self.told.contains(&(a.pattern.key(), a.marked)))
            .copied()
            .collect();
        self.told = now.iter().map(|a| (a.pattern.key(), a.marked)).collect();
        new
    }

    /// Puts `older` reports (oldest first, from an earlier session) before the ones there
    /// are, keeping the newest [`HISTORY`]; their advice counts as told (it is not logged
    /// again).
    pub fn seed(&mut self, older: impl IntoIterator<Item = (Timeline, Verdict)>) {
        let mut all: VecDeque<(Timeline, Verdict)> = older.into_iter().collect();
        all.append(&mut self.reports);
        while all.len() > HISTORY {
            all.pop_front();
        }
        self.reports = all;
        self.told = self
            .advise()
            .iter()
            .map(|a| (a.pattern.key(), a.marked))
            .collect();
    }

    /// The newest report.
    pub fn last(&self) -> Option<&(Timeline, Verdict)> {
        self.reports.back()
    }

    /// The newest report the user waited for (a slow one).
    pub fn last_slow(&self) -> Option<&(Timeline, Verdict)> {
        self.reports
            .iter()
            .rev()
            .find(|(t, v)| waited(t, v, self.slow_ms))
    }

    pub fn advise(&self) -> Vec<Advice> {
        let (a, b) = self.reports.as_slices();
        if b.is_empty() {
            advise_at(a, self.slow_ms)
        } else {
            advise_at(&[a, b].concat(), self.slow_ms)
        }
    }
}

/// The median (the lower of the two middle values for an even count; deterministic).
fn median(mut xs: Vec<f64>) -> Option<f64> {
    quantile(&mut xs, 0.5)
}

/// The `q` quantile by the nearest rank below.
fn quantile(xs: &mut [f64], q: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    let i = ((xs.len() - 1) as f64 * q).floor() as usize;
    Some(xs[i])
}

/// An open the user waited for: slower than `slow_ms`, not prepared in the background, not
/// from the cache.
pub(super) fn waited(t: &Timeline, v: &Verdict, slow_ms: f64) -> bool {
    v.total_ms >= slow_ms && !t.prefetched && !t.decode.is_some_and(|d| d.from_cache)
}

/// Software reasons that are a choice made for speed or power, or no fault: no pattern.
fn telling(s: Software) -> bool {
    !matches!(
        s,
        Software::Faster { .. } | Software::LowerPower | Software::Scaled
    )
}

/// Patterns in `reports` (oldest first), with what would help (opens).
#[cfg(test)]
pub fn advise(reports: &[(Timeline, Verdict)]) -> Vec<Advice> {
    advise_at(reports, SLOW_MS)
}

/// Patterns in `reports` (oldest first), with what would help, for items that count as
/// waited for from `slow_ms`.
pub fn advise_at(reports: &[(Timeline, Verdict)], slow_ms: f64) -> Vec<Advice> {
    let mut out = Vec::new();
    software(reports, slow_ms, &mut out);
    storage(reports, slow_ms, &mut out);
    first_open(reports, slow_ms, &mut out);
    out
}

fn marked(change: Option<Change>, gain: Option<Gain>, before_ms: f64, after_ms: f64) -> bool {
    change.is_some()
        && gain.is_some_and(|g| g.low >= 2.0 || (before_ms >= 1000.0 && after_ms < 1000.0))
}

/// "HEIC always falls back to software": per format, its recent slow opens mostly decoded in
/// software for one reason.
fn software(reports: &[(Timeline, Verdict)], slow_ms: f64, out: &mut Vec<Advice>) {
    let mut formats: Vec<&'static str> = Vec::new();
    for (t, _) in reports {
        if !formats.contains(&t.format) {
            formats.push(t.format);
        }
    }
    for format in formats {
        let slow: Vec<&(Timeline, Verdict)> = reports
            .iter()
            .rev()
            .filter(|(t, v)| t.format == format && waited(t, v, slow_ms))
            .take(WINDOW)
            .collect();
        // The most common reason (on a tie, the first by key).
        let mut tally: Vec<(Software, usize)> = Vec::new();
        for (_, v) in &slow {
            let Some(s) = v.software.filter(|s| telling(*s)) else {
                continue;
            };
            match tally.iter_mut().find(|e| e.0.key() == s.key()) {
                Some(e) => e.1 += 1,
                None => tally.push((s, 1)),
            }
        }
        let Some(&(why, count)) = tally
            .iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.key().cmp(a.0.key())))
        else {
            continue;
        };
        if count < MIN_SAMPLES || (count as f64) < SHARE * slow.len() as f64 {
            continue;
        }
        let change = match why {
            Software::Blocked(_) => Some(Change::Hint(Hint::TryAgain)),
            Software::CpuOnly => Some(Change::Hint(Hint::Automatic)),
            Software::SettingOff => Some(Change::Hint(Hint::TurnOn)),
            Software::NoDevice { .. } => slow
                .iter()
                .find_map(|(t, _)| t.decode.and_then(|d| d.path))
                .map(|p| Change::HardwareDecoder {
                    codec: super::codec_of(p.task),
                }),
            Software::NoCodec { codec, .. } => Some(Change::HardwareDecoder { codec }),
            _ => None,
        };
        // Each such open with its decode on the hardware decoder at the selector's estimate:
        // `high` takes the estimate as it is, `low` allows it to be off (more so when it is
        // the built-in guess, not measured here) and never slower than it was.
        let mut low = Vec::new();
        let mut high = Vec::new();
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (t, v) in &slow {
            if v.software.map(Software::key) != Some(why.key()) {
                continue;
            }
            let Some(d) = t.decode else { continue };
            let Some(hw) = d.path.and_then(|p| p.alt_est_ms.map(|e| (e, p.measured))) else {
                continue;
            };
            let decode = (d.ms - d.io_ms.min(d.ms)).max(0.0);
            let total = v.total_ms;
            let unsure = if hw.1 { 1.25 } else { 2.0 };
            let best = (total - decode + hw.0.min(decode)).max(1.0);
            let worst = (total - decode + (hw.0 * unsure).min(decode)).max(1.0);
            high.push(total / best);
            low.push(total / worst);
            before.push(total);
            after.push(worst);
        }
        let gain = change
            .and(median(low).zip(median(high)))
            .map(|(low, high)| Gain { low, high });
        let (b, a) = (median(before).unwrap_or(0.0), median(after).unwrap_or(0.0));
        out.push(Advice {
            pattern: Pattern::Software {
                format,
                why,
                count,
                of: slow.len(),
            },
            change,
            marked: marked(change, gain, b, a),
            gain,
        });
    }
}

/// The throughput an open read its file at (MB/s), when reading took long enough to say.
fn read_speed(t: &Timeline) -> Option<f64> {
    let read = t.stages()[1].1;
    (read >= 1.0 && t.bytes > 0).then(|| t.bytes as f64 / 1e6 / (read / 1e3))
}

/// "Slow storage dominates": reading the file held up most recent slow opens.
fn storage(reports: &[(Timeline, Verdict)], slow_ms: f64, out: &mut Vec<Advice>) {
    let slow: Vec<&(Timeline, Verdict)> = reports
        .iter()
        .rev()
        .filter(|(t, v)| waited(t, v, slow_ms))
        .take(WINDOW)
        .collect();
    let held: Vec<&(Timeline, Verdict)> = slow
        .iter()
        .copied()
        .filter(|(_, v)| v.bottleneck.is_some_and(|b| b.0 == Stage::Read))
        .collect();
    if held.len() < MIN_SAMPLES || (held.len() as f64) < SHARE * slow.len() as f64 {
        return;
    }
    let Some(mb_s) = median(held.iter().filter_map(|(t, _)| read_speed(t)).collect()) else {
        return;
    };
    let bytes = median(held.iter().map(|(t, _)| t.bytes as f64).collect()).unwrap_or(0.0) as u64;
    let archive = held.iter().filter(|(t, _)| t.in_archive).count() * 2 > held.len();
    if mb_s >= SLOW_STORAGE_MB_S && !archive {
        return;
    }
    // The fastest reading this session has seen (any open), within reason.
    let seen = reports
        .iter()
        .filter_map(|(t, _)| read_speed(t))
        .fold(0.0_f64, f64::max)
        .clamp(SLOW_STORAGE_MB_S, BEST_STORAGE_MB_S);
    let mut low = Vec::new();
    let mut high = Vec::new();
    let mut before = Vec::new();
    let mut after = Vec::new();
    for (t, v) in &held {
        let read = t.stages()[1].1;
        let rest = (v.total_ms - read).max(0.0);
        let at = |mb_s: f64| (rest + (t.bytes as f64 / 1e6 / mb_s * 1e3).min(read)).max(1.0);
        low.push(v.total_ms / at(SLOW_STORAGE_MB_S));
        high.push(v.total_ms / at(seen));
        before.push(v.total_ms);
        after.push(at(SLOW_STORAGE_MB_S));
    }
    let change = Some(Change::Hint(if archive {
        Hint::Extract
    } else {
        Hint::CopyLocal
    }));
    let gain = median(low)
        .zip(median(high))
        .map(|(low, high)| Gain { low, high });
    let (b, a) = (median(before).unwrap_or(0.0), median(after).unwrap_or(0.0));
    out.push(Advice {
        pattern: Pattern::SlowStorage {
            mb_s,
            bytes,
            archive,
            count: held.len(),
            of: slow.len(),
        },
        change,
        marked: marked(change, gain, b, a),
        gain,
    });
}

/// "The first decode pays for starting up": the session's first open against those since.
/// Nothing the user can change, so it is only logged; the gain is what a warm start makes.
fn first_open(reports: &[(Timeline, Verdict)], slow_ms: f64, out: &mut Vec<Advice>) {
    let Some(i) = reports.iter().rposition(|(t, _)| t.startup_ms > 0.0) else {
        return;
    };
    let first = reports[i].1.total_ms;
    let mut later: Vec<f64> = reports[i + 1..]
        .iter()
        .filter(|(t, v)| {
            t.startup_ms <= 0.0
                && !t.prefetched
                && !t.decode.is_some_and(|d| d.from_cache)
                && v.total_ms > 0.0
        })
        .map(|(_, v)| v.total_ms)
        .collect();
    if later.len() < MIN_SAMPLES || first < slow_ms {
        return;
    }
    let (Some(p25), Some(mid), Some(p75)) = (
        quantile(&mut later, 0.25),
        quantile(&mut later, 0.5),
        quantile(&mut later, 0.75),
    ) else {
        return;
    };
    if first < 2.0 * mid {
        return;
    }
    out.push(Advice {
        pattern: Pattern::FirstOpen {
            first_ms: first,
            later_ms: mid,
        },
        change: None,
        gain: Some(Gain {
            low: first / p75.max(1.0),
            high: first / p25.max(1.0),
        }),
        marked: false,
    });
}

/// One line per piece of advice for the log (stable keys, not localised).
pub fn log_line(a: &Advice) -> String {
    use std::fmt::Write;
    let mut s = String::from("advisor: ");
    match a.pattern {
        Pattern::Software {
            format,
            why,
            count,
            of,
        } => {
            let _ = write!(
                s,
                "{count} of the last {of} slow {format} opens in software ({})",
                why.key()
            );
        }
        Pattern::SlowStorage {
            mb_s,
            bytes,
            archive,
            count,
            of,
        } => {
            let _ = write!(
                s,
                "{count} of the last {of} slow opens held up {} (~{mb_s:.1} MB/s, ~{} KB files)",
                if archive {
                    "unpacking archives"
                } else {
                    "reading"
                },
                bytes / 1000
            );
        }
        Pattern::FirstOpen { first_ms, later_ms } => {
            let _ = write!(
                s,
                "first open {first_ms:.0} ms, later median {later_ms:.0} ms"
            );
        }
    }
    match a.change {
        Some(Change::Hint(h)) => {
            let _ = write!(s, "; change {}", h.key());
        }
        Some(Change::HardwareDecoder { codec }) => {
            let _ = write!(s, "; change hardware {codec} decoder");
        }
        None => {}
    }
    if let Some(g) = a.gain {
        let _ = write!(s, "; gain {:.1}-{:.1}x", g.low, g.high);
    }
    if a.marked {
        s.push_str(" (shown)");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::{DecodeStage, HwState, PathFacts, verdict};
    use super::*;
    use specular_core::paths::{Engine, Failure, Mode, Reason, Task, Trust};

    fn report(t: Timeline) -> (Timeline, Verdict) {
        (t, verdict(&t))
    }

    /// A HEIC decoded in software because there is no hardware decoder (the selector guesses
    /// 100 ms for one), `decode_ms` long, 30 ms more until sharp.
    fn heic(decode_ms: f64, set: impl Fn(&mut PathFacts)) -> (Timeline, Verdict) {
        let mut p = PathFacts {
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
            hw: HwState::None { api: "VA-API" },
            scaled: None,
        };
        set(&mut p);
        report(Timeline {
            format: "HEIC",
            bytes: 3_000_000,
            pixels: 12_000_000,
            read_ms: 0.2,
            parse_ms: 0.5,
            decode: Some(DecodeStage {
                scale: 1,
                ms: decode_ms,
                path: Some(p),
                ..DecodeStage::default()
            }),
            upload_ms: 10.0,
            sharp_ms: decode_ms + 30.0,
            ..Timeline::default()
        })
    }

    /// A 10 MB JPEG whose file took `read_ms` to read (while decoding), 100 ms to decode and
    /// `total_ms` in all.
    fn jpeg_read(read_ms: f64, total_ms: f64) -> (Timeline, Verdict) {
        report(Timeline {
            format: "JPEG",
            bytes: 10_000_000,
            pixels: 24_000_000,
            decode: Some(DecodeStage {
                scale: 1,
                ms: read_ms + 100.0,
                io_ms: read_ms,
                faults: 1000,
                ..DecodeStage::default()
            }),
            upload_ms: 10.0,
            sharp_ms: total_ms,
            ..Timeline::default()
        })
    }

    /// An open `total_ms` long, mostly decoding, without a path decision.
    fn plain(total_ms: f64, startup_ms: f64) -> (Timeline, Verdict) {
        report(Timeline {
            format: "PNG",
            bytes: 1_000_000,
            pixels: 4_000_000,
            decode: Some(DecodeStage {
                scale: 1,
                ms: total_ms - startup_ms - 10.0,
                ..DecodeStage::default()
            }),
            startup_ms,
            sharp_ms: total_ms,
            ..Timeline::default()
        })
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn heic_always_in_software_suggests_a_hardware_decoder() {
        let reports: Vec<_> = (0..6).map(|i| heic(900.0 + f64::from(i), |_| {})).collect();
        let advice = advise(&reports);
        assert_eq!(advice.len(), 1, "{advice:?}");
        let a = advice[0];
        assert_eq!(
            a.pattern,
            Pattern::Software {
                format: "HEIC",
                why: Software::NoDevice { api: "VA-API" },
                count: 6,
                of: 6
            }
        );
        assert_eq!(a.change, Some(Change::HardwareDecoder { codec: "HEVC" }));
        // The median open (932 ms, 902 decoding) with the decode at the selector's guess of
        // 100 ms, and at twice that (a guess, not measured here).
        let g = a.gain.unwrap();
        assert!(close(g.high, 932.0 / 130.0), "{g:?}");
        assert!(close(g.low, 932.0 / 230.0), "{g:?}");
        assert!(a.marked);
        let line = log_line(&a);
        assert!(
            line.contains("6 of the last 6 slow HEIC opens in software (no-device)"),
            "{line}"
        );
        assert!(line.contains("change hardware HEVC decoder"), "{line}");
        // Deterministic.
        assert_eq!(advise(&reports), advice);
    }

    #[test]
    fn settings_and_blocks_suggest_their_fix() {
        let off: Vec<_> = (0..5)
            .map(|_| {
                heic(900.0, |p| {
                    p.forced_off = true;
                    p.alt_trust = Trust::Trusted;
                    p.measured = true;
                })
            })
            .collect();
        let a = advise(&off)[0];
        assert_eq!(a.change, Some(Change::Hint(Hint::TurnOn)));
        // Measured here: allowed to be a quarter slower than estimated.
        let g = a.gain.unwrap();
        assert!(close(g.low, 930.0 / (30.0 + 125.0)), "{g:?}");
        assert!(close(g.high, 930.0 / 130.0), "{g:?}");
        let blocked: Vec<_> = (0..5)
            .map(|_| {
                heic(900.0, |p| {
                    p.alt_trust = Trust::Blocked;
                    p.alt_blocked = Some(Failure::CheckFailed);
                })
            })
            .collect();
        assert_eq!(
            advise(&blocked)[0].change,
            Some(Change::Hint(Hint::TryAgain))
        );
        // A reason with nothing to change is a pattern for the log only.
        let early: Vec<_> = (0..5)
            .map(|_| heic(900.0, |p| p.hw = HwState::NotProbed))
            .collect();
        let a = advise(&early)[0];
        assert!(matches!(
            a.pattern,
            Pattern::Software {
                why: Software::NotProbed,
                ..
            }
        ));
        assert_eq!(a.change, None);
        assert_eq!(a.gain, None);
        assert!(!a.marked);
    }

    #[test]
    fn no_pattern_without_enough_alike_slow_opens() {
        // Four is not enough.
        let four: Vec<_> = (0..4).map(|_| heic(900.0, |_| {})).collect();
        assert!(advise(&four).is_empty());
        // Fast opens do not count, nor prefetched ones.
        let mut fast: Vec<_> = (0..10).map(|_| heic(100.0, |_| {})).collect();
        for _ in 0..10 {
            let (mut t, _) = heic(900.0, |_| {});
            t.prefetched = true;
            fast.push(report(t));
        }
        assert!(advise(&fast).is_empty());
        // Five of seven slow HEIC opens alike is under 80 %.
        let mut mixed: Vec<_> = (0..5).map(|_| heic(900.0, |_| {})).collect();
        mixed.push(heic(900.0, |p| p.hw = HwState::NotProbed));
        mixed.push(heic(900.0, |p| {
            p.engine = Engine::HwDecoder;
            p.alt_trust = Trust::Trusted;
        }));
        assert!(advise(&mixed).is_empty());
        // In software because it is faster there: no fault, no pattern.
        let faster: Vec<_> = (0..6)
            .map(|_| {
                heic(900.0, |p| {
                    p.reason = Reason::Faster;
                    p.alt_trust = Trust::Trusted;
                })
            })
            .collect();
        assert!(advise(&faster).is_empty());
    }

    #[test]
    fn slow_storage_suggests_a_local_copy() {
        // Once read at 800 MB/s (a fast open), then six opens read at 20 MB/s.
        let mut reports = vec![jpeg_read(12.5, 140.0)];
        reports.extend((0..6).map(|_| jpeg_read(500.0, 650.0)));
        let advice = advise(&reports);
        assert_eq!(advice.len(), 1, "{advice:?}");
        let a = advice[0];
        let Pattern::SlowStorage {
            mb_s,
            bytes,
            archive,
            count,
            of,
        } = a.pattern
        else {
            panic!("{a:?}")
        };
        assert!(close(mb_s, 20.0), "{mb_s}");
        assert_eq!(bytes, 10_000_000);
        assert!(!archive);
        assert_eq!((count, of), (6, 6));
        assert_eq!(a.change, Some(Change::Hint(Hint::CopyLocal)));
        // 150 ms besides reading; reading at 150 MB/s takes 66.7 ms, at 800 MB/s 12.5 ms.
        let g = a.gain.unwrap();
        assert!(close(g.low, 650.0 / (150.0 + 1e4 / 150.0)), "{g:?}");
        assert!(close(g.high, 650.0 / 162.5), "{g:?}");
        assert!(a.marked);
        // Without a faster read seen, the estimate assumes 150 MB/s both ways.
        let g = advise(&reports[1..])[0].gain.unwrap();
        assert!(close(g.low, g.high), "{g:?}");
        // In archives: extract.
        let archived: Vec<_> = reports[1..]
            .iter()
            .map(|(t, _)| {
                let mut t = *t;
                t.in_archive = true;
                report(t)
            })
            .collect();
        let a = advise(&archived)[0];
        assert!(matches!(
            a.pattern,
            Pattern::SlowStorage { archive: true, .. }
        ));
        assert_eq!(a.change, Some(Change::Hint(Hint::Extract)));
        // Reading 100 MB files at 200 MB/s is not slow storage.
        let big: Vec<_> = reports[1..]
            .iter()
            .map(|(t, _)| {
                let mut t = *t;
                t.bytes = 100_000_000;
                report(t)
            })
            .collect();
        assert!(advise(&big).is_empty(), "{:?}", advise(&big));
    }

    #[test]
    fn the_first_open_pays_for_starting_up() {
        let mut reports = vec![plain(900.0, 600.0)];
        reports.extend([100.0, 120.0, 110.0, 140.0, 130.0].map(|ms| plain(ms, 0.0)));
        let advice = advise(&reports);
        assert_eq!(advice.len(), 1, "{advice:?}");
        let a = advice[0];
        assert_eq!(
            a.pattern,
            Pattern::FirstOpen {
                first_ms: 900.0,
                later_ms: 120.0
            }
        );
        // Nothing to change: logged, never shown.
        assert_eq!(a.change, None);
        assert!(!a.marked);
        let g = a.gain.unwrap();
        assert!(
            close(g.low, 900.0 / 130.0) && close(g.high, 900.0 / 110.0),
            "{g:?}"
        );
        // Not yet five opens since, or a first open not much slower than the rest: nothing.
        assert!(advise(&reports[..5]).is_empty());
        let mut even = vec![plain(260.0, 100.0)];
        even.extend((0..5).map(|_| plain(200.0, 0.0)));
        assert!(advise(&even).is_empty());
    }

    #[test]
    fn history_keeps_the_latest_and_tells_each_pattern_once() {
        let mut h = History::new();
        let mut told = Vec::new();
        for _ in 0..HISTORY + 10 {
            let (t, v) = heic(900.0, |_| {});
            told.extend(h.push(t, v));
        }
        assert_eq!(h.reports.len(), HISTORY);
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(matches!(
            told[0].pattern,
            Pattern::Software { count: 5, .. }
        ));
        // The window is the latest 20 slow opens.
        assert!(matches!(
            h.advise()[0].pattern,
            Pattern::Software {
                count: 20,
                of: 20,
                ..
            }
        ));
    }
}
