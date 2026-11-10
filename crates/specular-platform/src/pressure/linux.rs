//! Memory pressure on Linux from PSI (pressure stall information) and cgroup v2 events.
//!
//! * `/proc/pressure/memory` triggers: one file handle per trigger, `poll()`ed for `POLLPRI`.
//!   A trigger fires when tasks stalled on memory for more than its threshold within its window:
//!   `some` (at least one task waiting) for [`MemoryPressure::Warn`], `full` (every runnable task
//!   waiting) for [`MemoryPressure::Critical`]. Windows of 1 s need `CAP_SYS_RESOURCE`; since
//!   Linux 6.5 anyone may set multiples of 2 s, tried next. Where no trigger is accepted (older
//!   kernels, unprivileged), the totals are read every few seconds instead.
//! * `memory.events` of the nearest cgroup v2 ancestor with a `memory.high` or `memory.max`
//!   limit (systemd scopes, containers): `high` (throttled at the soft limit) means Warn, `max`,
//!   `oom` and `oom_kill` mean Critical. The kernel notifies changes as `POLLPRI` on the open
//!   file. That cgroup's `memory.pressure` is sampled beside the system's.
//!
//! Triggers only say "now"; how long the pressure lasts is measured from the `total=` stall
//! counters, sampled every [`RAISED_PERIOD`] while the level is raised and fed through
//! [`Hysteresis`]. While calm with working triggers the thread sleeps in `poll()` with no
//! timeout. Without `/proc/pressure` (kernels without PSI, `psi=0`) nothing is started.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};

use super::Hysteresis;
use crate::{MemoryPressure, report_memory_pressure};

const PSI: &str = "/proc/pressure/memory";
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Sampling period while the level is raised.
const RAISED_PERIOD: Duration = Duration::from_secs(2);
/// Sampling period while calm when no trigger was accepted.
const FALLBACK_PERIOD: Duration = Duration::from_secs(4);
/// At most one round per this long, however often the cgroup notifies.
const MIN_GAP: Duration = Duration::from_secs(1);

/// Trigger lines (stall µs, window µs), tried in order: 1 s windows (privileged), then 2 s.
const WARN_TRIGGERS: [&str; 2] = ["some 150000 1000000", "some 300000 2000000"];
const CRITICAL_TRIGGERS: [&str; 2] = ["full 100000 1000000", "full 200000 2000000"];

/// Shares of wall time stalled that raise the level (enter) or keep it (stay).
const WARN_SOME: (f64, f64) = (0.10, 0.03);
const CRITICAL_FULL: (f64, f64) = (0.10, 0.03);
const CRITICAL_SOME: (f64, f64) = (0.40, 0.20);

pub(crate) fn start() {
    if !Path::new(PSI).exists() {
        specular_log::debug!(target: specular_log::CACHE, "memory pressure: no PSI, not watched");
        return;
    }
    let spawned = specular_thread::builder("plat-memory").spawn(|| super::supervised(run));
    if let Err(e) = spawned {
        specular_log::debug!(target: specular_log::CACHE, "memory pressure: no watcher thread: {e}");
    }
}

/// Times [`run`] started (tests).
#[cfg(test)]
static RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn run() {
    #[cfg(test)]
    RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut warn = trigger(&WARN_TRIGGERS);
    let mut critical = trigger(&CRITICAL_TRIGGERS);
    if warn.is_none() || critical.is_none() {
        // Both or neither: one alone would leave the other level to sampling anyway.
        (warn, critical) = (None, None);
    }
    let mut cgroup = limited_cgroup(Path::new(CGROUP_ROOT)).and_then(|d| CgroupEvents::open(&d));
    let mut psi = vec![PathBuf::from(PSI)];
    if let Some(c) = &cgroup {
        let p = c.dir.join("memory.pressure");
        if p.exists() {
            psi.push(p);
        }
    }
    specular_log::debug!(
        target: specular_log::CACHE,
        "memory-pressure watcher started (PSI triggers: {}, cgroup limit: {})",
        if warn.is_some() && critical.is_some() { "yes" } else { "no, sampling" },
        cgroup.as_ref().map_or("none".into(), |c| c.dir.display().to_string())
    );
    let mut sampler = Sampler::new(psi);
    let mut level = Hysteresis::default();
    let mut last_round = Instant::now();
    loop {
        let triggers = warn.is_some() && critical.is_some();
        let timeout = if level.level() > MemoryPressure::Normal {
            Some(RAISED_PERIOD)
        } else if triggers {
            None
        } else {
            Some(FALLBACK_PERIOD)
        };
        specular_log::service::trip();
        let mut seen = MemoryPressure::Normal;
        {
            let mut fds = Vec::with_capacity(3);
            for f in [&warn, &critical].into_iter().flatten() {
                fds.push(PollFd::new(f, PollFlags::PRI));
            }
            if let Some(c) = &cgroup {
                fds.push(PollFd::new(&c.file, PollFlags::PRI));
            }
            let ts = timeout.map(|t| Timespec {
                tv_sec: t.as_secs() as _,
                tv_nsec: t.subsec_nanos() as _,
            });
            if fds.is_empty() {
                std::thread::sleep(timeout.unwrap_or(FALLBACK_PERIOD));
            } else {
                match poll(&mut fds, ts.as_ref()) {
                    Ok(_) | Err(rustix::io::Errno::INTR) => {}
                    Err(e) => {
                        specular_log::debug!(target: specular_log::CACHE, "memory pressure: poll failed ({e}), sampling");
                        warn = None;
                        critical = None;
                        cgroup = None;
                        continue;
                    }
                }
            }
            let revents: Vec<PollFlags> = fds.iter().map(|f| f.revents()).collect();
            let mut i = 0;
            if triggers {
                let (w, c) = (revents[0], revents[1]);
                i = 2;
                if (w | c).contains(PollFlags::ERR) || (w | c).contains(PollFlags::NVAL) {
                    // The trigger is gone; fall back to sampling.
                    warn = None;
                    critical = None;
                }
                if c.contains(PollFlags::PRI) {
                    seen = MemoryPressure::Critical;
                } else if w.contains(PollFlags::PRI) {
                    seen = MemoryPressure::Warn;
                }
            }
            if cgroup.is_some() && revents.get(i).is_some_and(|r| !r.is_empty()) {
                // kernfs reports a change as POLLPRI | POLLERR; reading re-arms it.
                match cgroup.as_mut().and_then(CgroupEvents::changed) {
                    Some(Some(l)) => seen = seen.max(l),
                    Some(None) => {}
                    // Unreadable (the cgroup went away): stop watching it.
                    None => cgroup = None,
                }
            }
        }
        if let Some((some, full)) = sampler.sample() {
            seen = seen.max(classify(level.level(), some, full));
        }
        let before = level.level();
        let now = level.step(seen);
        if now != before {
            report_memory_pressure(now);
        }
        // However often the cgroup notifies, at most one round per MIN_GAP.
        let spent = last_round.elapsed();
        if spent < MIN_GAP {
            std::thread::sleep(MIN_GAP - spent);
        }
        last_round = Instant::now();
    }
}

/// A PSI trigger: the first of `lines` the kernel accepts, on its own file handle.
fn trigger(lines: &[&str]) -> Option<File> {
    for line in lines {
        let Ok(mut f) = OpenOptions::new().read(true).write(true).open(PSI) else {
            return None;
        };
        // One write; the kernel turns the last byte into the terminator, so send one.
        let bytes = format!("{line}\0");
        if f.write(bytes.as_bytes()).is_ok_and(|n| n == bytes.len()) {
            return Some(f);
        }
    }
    None
}

/// The level stall shares `some` and `full` (0..1 of the sampled time) show, with the lower
/// "stay" thresholds for the level already reported.
fn classify(current: MemoryPressure, some: f64, full: f64) -> MemoryPressure {
    let pick = |t: (f64, f64), at: MemoryPressure| if current >= at { t.1 } else { t.0 };
    if full >= pick(CRITICAL_FULL, MemoryPressure::Critical)
        || some >= pick(CRITICAL_SOME, MemoryPressure::Critical)
    {
        MemoryPressure::Critical
    } else if some >= pick(WARN_SOME, MemoryPressure::Warn) {
        MemoryPressure::Warn
    } else {
        MemoryPressure::Normal
    }
}

/// The `total=` stall counters (µs) of a PSI file's `some` and `full` lines.
fn parse_totals(text: &str) -> Option<(u64, u64)> {
    let mut some = None;
    let mut full = 0;
    for line in text.lines() {
        let mut words = line.split_ascii_whitespace();
        let kind = words.next();
        let total = words
            .find_map(|w| w.strip_prefix("total="))
            .and_then(|v| v.parse().ok());
        match (kind, total) {
            (Some("some"), Some(t)) => some = Some(t),
            (Some("full"), Some(t)) => full = t,
            _ => {}
        }
    }
    Some((some?, full))
}

/// Stall shares between consecutive reads of the PSI files (the largest of them).
struct Sampler {
    files: Vec<PathBuf>,
    last: Option<(Instant, Vec<(u64, u64)>)>,
}

impl Sampler {
    fn new(files: Vec<PathBuf>) -> Sampler {
        Sampler { files, last: None }
    }

    /// `(some, full)` shares of the time since the previous call; `None` on the first call,
    /// after a long sleep (the share would average the pressure away) or when a file is
    /// unreadable.
    fn sample(&mut self) -> Option<(f64, f64)> {
        let now = Instant::now();
        let totals: Option<Vec<_>> = self
            .files
            .iter()
            .map(|p| parse_totals(&std::fs::read_to_string(p).ok()?))
            .collect();
        let prev = std::mem::take(&mut self.last);
        let totals = totals?;
        self.last = Some((now, totals.clone()));
        let (at, before) = prev?;
        let span = now.duration_since(at);
        if span > 2 * FALLBACK_PERIOD || span.is_zero() {
            return None;
        }
        let us = span.as_micros() as f64;
        Some(
            totals
                .iter()
                .zip(&before)
                .fold((0.0f64, 0.0f64), |(s, f), (n, b)| {
                    (
                        s.max(n.0.saturating_sub(b.0) as f64 / us),
                        f.max(n.1.saturating_sub(b.1) as f64 / us),
                    )
                }),
        )
    }
}

/// The cgroup (under `root`) this process is in, or its nearest ancestor, with a memory limit
/// (`memory.high` or `memory.max` other than `max`). `None` on cgroup v1 or without limits.
fn limited_cgroup(root: &Path) -> Option<PathBuf> {
    let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    limited_cgroup_of(root, &own)
}

fn limited_cgroup_of(root: &Path, proc_cgroup: &str) -> Option<PathBuf> {
    // cgroup v2 is the "0::/path" line.
    let rel = proc_cgroup
        .lines()
        .find_map(|l| l.strip_prefix("0::"))?
        .trim()
        .trim_start_matches('/');
    let mut dir = root.join(rel);
    loop {
        let limited = |f: &str| {
            std::fs::read_to_string(dir.join(f)).is_ok_and(|v| {
                let v = v.trim();
                !v.is_empty() && v != "max"
            })
        };
        if limited("memory.high") || limited("memory.max") {
            return dir.join("memory.events").exists().then_some(dir);
        }
        if dir == root || !dir.pop() || !dir.starts_with(root) {
            return None;
        }
    }
}

/// Counters of `memory.events`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Events {
    high: u64,
    max: u64,
    oom: u64,
    oom_kill: u64,
}

fn parse_events(text: &str) -> Events {
    let mut e = Events::default();
    for line in text.lines() {
        let mut w = line.split_ascii_whitespace();
        let (Some(k), Some(v)) = (w.next(), w.next().and_then(|v| v.parse().ok())) else {
            continue;
        };
        match k {
            "high" => e.high = v,
            "max" => e.max = v,
            "oom" => e.oom = v,
            "oom_kill" => e.oom_kill = v,
            _ => {}
        }
    }
    e
}

/// The level counters moving from `a` to `b` show, if any moved.
fn events_level(a: Events, b: Events) -> Option<MemoryPressure> {
    if b.max > a.max || b.oom > a.oom || b.oom_kill > a.oom_kill {
        Some(MemoryPressure::Critical)
    } else if b.high > a.high {
        Some(MemoryPressure::Warn)
    } else {
        None
    }
}

/// An open `memory.events` and its last counters.
struct CgroupEvents {
    dir: PathBuf,
    file: File,
    last: Events,
}

impl CgroupEvents {
    fn open(dir: &Path) -> Option<CgroupEvents> {
        let mut c = CgroupEvents {
            dir: dir.to_path_buf(),
            file: File::open(dir.join("memory.events")).ok()?,
            last: Events::default(),
        };
        c.last = c.read()?;
        Some(c)
    }

    /// Reads the counters again through the polled handle (which re-arms the notification).
    fn read(&mut self) -> Option<Events> {
        let mut text = String::new();
        self.file.seek(SeekFrom::Start(0)).ok()?;
        self.file.read_to_string(&mut text).ok()?;
        Some(parse_events(&text))
    }

    /// The level the counters' change since the last read shows (`Some(None)`: no change;
    /// `None`: unreadable).
    fn changed(&mut self) -> Option<Option<MemoryPressure>> {
        let now = self.read()?;
        let l = events_level(self.last, now);
        self.last = now;
        Some(l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MemoryPressure::*;

    /// A panic in the watcher drops a raised level back to normal (nothing would lower it
    /// otherwise) and the watcher restarts.
    #[test]
    fn watcher_restarts_after_a_panic() {
        if !Path::new(PSI).exists() {
            return;
        }
        let _one = crate::PRESSURE_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let deadline = Instant::now() + Duration::from_secs(15);
        crate::set_memory_pressure(Warn);
        let runs = RUNS.load(std::sync::atomic::Ordering::Relaxed);
        specular_log::service::arm("plat-memory", 1);
        start();
        while specular_log::service::deaths("plat-memory") == 0 {
            assert!(Instant::now() < deadline, "the watcher never panicked");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(crate::memory_pressure(), Normal);
        while RUNS.load(std::sync::atomic::Ordering::Relaxed) < runs + 2 {
            assert!(Instant::now() < deadline, "the watcher did not restart");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn totals() {
        let t = "some avg10=0.00 avg60=0.12 avg300=0.00 total=44174950\n\
                 full avg10=0.00 avg60=0.00 avg300=0.00 total=36481731\n";
        assert_eq!(parse_totals(t), Some((44174950, 36481731)));
        // Kernels before "full" for every resource, and garbage.
        assert_eq!(parse_totals("some avg10=1.00 total=7\n"), Some((7, 0)));
        assert_eq!(parse_totals("nonsense"), None);
    }

    #[test]
    fn shares_to_levels() {
        assert_eq!(classify(Normal, 0.0, 0.0), Normal);
        assert_eq!(classify(Normal, 0.05, 0.0), Normal);
        assert_eq!(classify(Normal, 0.12, 0.0), Warn);
        assert_eq!(classify(Normal, 0.12, 0.11), Critical);
        assert_eq!(classify(Normal, 0.45, 0.0), Critical);
        // Lower thresholds keep a raised level.
        assert_eq!(classify(Warn, 0.05, 0.0), Warn);
        assert_eq!(classify(Warn, 0.02, 0.0), Normal);
        assert_eq!(classify(Critical, 0.25, 0.0), Critical);
        assert_eq!(classify(Critical, 0.05, 0.05), Critical);
        assert_eq!(classify(Critical, 0.05, 0.01), Warn);
        assert_eq!(classify(Critical, 0.01, 0.01), Normal);
    }

    #[test]
    fn cgroup_events() {
        let a = parse_events("low 0\nhigh 3\nmax 0\noom 0\noom_kill 0\noom_group_kill 0\n");
        assert_eq!(a.high, 3);
        assert_eq!(events_level(a, a), None);
        let b = Events { high: 4, ..a };
        assert_eq!(events_level(a, b), Some(Warn));
        assert_eq!(events_level(a, Events { max: 1, ..b }), Some(Critical));
        assert_eq!(events_level(a, Events { oom_kill: 1, ..a }), Some(Critical));
    }

    #[test]
    fn finds_the_limited_ancestor() {
        let root = std::env::temp_dir().join(format!("specular-cgroup-{}", std::process::id()));
        let leaf = root.join("user.slice/app.slice/run-1.scope");
        std::fs::create_dir_all(&leaf).unwrap();
        let write = |d: &Path, f: &str, v: &str| std::fs::write(d.join(f), v).unwrap();
        for d in [
            &root.join("user.slice"),
            &root.join("user.slice/app.slice"),
            &leaf,
        ] {
            write(d, "memory.high", "max\n");
            write(d, "memory.max", "max\n");
            write(d, "memory.events", "high 0\nmax 0\n");
        }
        let own = "0::/user.slice/app.slice/run-1.scope\n";
        assert_eq!(limited_cgroup_of(&root, own), None);
        assert_eq!(
            limited_cgroup_of(&root, "12:memory:/x\n"),
            None,
            "cgroup v1"
        );
        write(&root.join("user.slice"), "memory.max", "8589934592\n");
        assert_eq!(limited_cgroup_of(&root, own), Some(root.join("user.slice")));
        write(&leaf, "memory.high", "1073741824\n");
        assert_eq!(limited_cgroup_of(&root, own), Some(leaf.clone()));
        // Events read through an open handle, as the watcher does.
        let mut ev = CgroupEvents::open(&leaf).unwrap();
        assert_eq!(ev.changed(), Some(None));
        write(&leaf, "memory.events", "high 2\nmax 0\n");
        assert_eq!(ev.changed(), Some(Some(Warn)));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn samples_this_machine() {
        if !Path::new(PSI).exists() {
            return;
        }
        let mut s = Sampler::new(vec![PathBuf::from(PSI)]);
        assert_eq!(s.sample(), None, "the first read is the baseline");
        std::thread::sleep(Duration::from_millis(20));
        let (some, full) = s.sample().unwrap();
        assert!((0.0..=1.5).contains(&some) && (0.0..=1.5).contains(&full));
        // Unprivileged on Linux ≥ 6.5 the 2 s triggers are accepted; neither may panic.
        let _ = (trigger(&WARN_TRIGGERS), trigger(&CRITICAL_TRIGGERS));
    }
}
