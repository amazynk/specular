//! Isolation: a bad file or folder costs one job, never the process.
//!
//! Release builds unwind (`panic = "unwind"` in the workspace's release profile), so a panic in
//! a decoder, a parser or a third-party crate can be caught where a job starts. [`run`] is that
//! boundary: it runs a job, and if the job panics it logs what the job was (its file), where it
//! panicked and why, and returns [`Panicked`] for the caller to turn into an ordinary error (an
//! error tile, "internal error" in the viewer, a file marked unreadable). The worker thread goes
//! on with its next job.
//!
//! The app's event loop callbacks are such a boundary too (the app's `ui_guard`, through
//! [`run_reported`]): a panic while handling one event costs that event and shows a warning,
//! and still gets a crash report with its backtrace (not offered at the next start: the app
//! went on). A UI that keeps panicking, or a panic outside any boundary on the UI thread,
//! still ends the app. [`install`] sets a panic
//! hook that then writes a local crash report (never sent anywhere: Specular has no telemetry
//! of any kind) and a marker that the next start reads with [`take_pending_report`] to say
//! "Specular closed unexpectedly last time; a report was saved at …".
//!
//! Not covered by unwinding: a panic while already panicking (a `Drop` that panics during an
//! unwind) and stack overflow still abort; out-of-memory aborts. The C/C++ decoders run in the
//! sandboxed decode host (DESIGN.md §13), where a crash costs that child process.

use std::cell::Cell;
use std::fmt::{self, Display};
use std::io::Write;
use std::panic::{AssertUnwindSafe, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;

use specular_log::{JOBS, error};

/// A job that panicked: what the panic said and where (`file:line:col`, when the hook saw it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panicked {
    pub message: String,
    pub location: Option<String>,
}

impl Display for Panicked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            Some(l) => write!(f, "internal error: {} (at {l})", self.message),
            None => write!(f, "internal error: {}", self.message),
        }
    }
}

impl std::error::Error for Panicked {}

impl Panicked {
    /// The crate the panic happened in, from its location: `specular-jpeg` for
    /// `crates/formats/specular-jpeg/src/…`, `zune-jpeg` for a registry crate's
    /// `…/zune-jpeg-0.4.14/src/…`. `None` when unknown or in the standard library.
    pub fn component(&self) -> Option<String> {
        let loc = self.location.as_deref()?;
        // `file:line:col`; the file may hold colons itself (`C:\…`).
        let file = loc.rsplitn(3, ':').nth(2)?.replace('\\', "/");
        let root = &file[..file.rfind("/src/")?];
        let name = root.rsplit('/').next().filter(|n| !n.is_empty())?;
        if ["core", "alloc", "std"].contains(&name) {
            return None;
        }
        // Registry and git checkouts name their folders `name-version`.
        let versioned = file.contains("/registry/src/") || file.contains("/git/checkouts/");
        let name = match name
            .match_indices('-')
            .find(|(i, _)| name[i + 1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            Some((i, _)) if versioned => &name[..i],
            _ => name,
        };
        Some(name.to_owned())
    }

    /// What the user is told: "internal error in specular-jpeg" (the panic's message and
    /// location are in the log, where [`run`] put them).
    pub fn brief(&self) -> String {
        match self.component() {
            Some(c) => format!("internal error in {c}"),
            None => "internal error".to_owned(),
        }
    }
}

thread_local! {
    /// How many [`run`] boundaries this thread is inside: the hook keeps quiet about panics a
    /// boundary will report.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// Where this thread's last caught panic happened (the payload does not say).
    static LAST_LOCATION: Cell<Option<String>> = const { Cell::new(None) };
    /// The [`DEPTH`] inside the innermost [`run_reported`] boundary (0: none): a panic
    /// caught right there, not by a boundary within it, gets a crash report too.
    static REPORTED_AT: Cell<u32> = const { Cell::new(0) };
}

/// Panics caught by [`run`] in this process.
static CAUGHT: AtomicU64 = AtomicU64::new(0);

/// Where the hook writes crash reports, and the UI thread (only its panics end the app).
struct HookConfig {
    crash_dir: Option<PathBuf>,
    ui_thread: ThreadId,
}

static HOOK: OnceLock<HookConfig> = OnceLock::new();

/// Name of the marker file in the crash folder that holds the path of a report not shown yet.
const PENDING: &str = "pending";
/// Crash reports kept; older ones are removed when a new one is written.
const KEEP_REPORTS: usize = 10;
/// Log lines a crash report ends with.
pub const REPORT_LOG_LINES: usize = 200;

/// What the app adds to a crash report: set once with [`set_report_hooks`]. Both run in the
/// panic hook, so they must not wait for locks the UI thread may hold (keep a snapshot).
pub struct ReportHooks {
    /// More lines for the report: the GPU adapter and driver, the settings.
    pub context: Box<dyn Fn() -> String + Send + Sync>,
    /// Applied to the whole report before it is written: the home folder, the user name.
    pub redact: Box<dyn Fn(&str) -> String + Send + Sync>,
}

static REPORT_HOOKS: OnceLock<ReportHooks> = OnceLock::new();

/// Sets what crash reports add and how they are redacted (once; later calls do nothing).
pub fn set_report_hooks(hooks: ReportHooks) {
    let _ = REPORT_HOOKS.set(hooks);
}

/// What a crash report records about one panic.
pub(crate) struct Crash<'a> {
    pub stamp: &'a str,
    pub thread: &'a str,
    pub location: &'a str,
    pub message: &'a str,
    pub backtrace: &'a str,
}

/// The text of a crash report: the panic, the app's context, the last log lines, redacted.
pub(crate) fn report_text(c: &Crash<'_>, hooks: Option<&ReportHooks>, log: &[String]) -> String {
    let mut s = format!(
        "Specular closed unexpectedly.\n\
         This report stays on this computer; nothing was sent anywhere.\n\n\
         version:  {}\n\
         time:     {} UTC\n\
         os:       {} {}\n\
         thread:   {}\n\
         location: {}\n\
         message:  {}\n",
        env!("CARGO_PKG_VERSION"),
        c.stamp,
        std::env::consts::OS,
        std::env::consts::ARCH,
        c.thread,
        c.location,
        c.message,
    );
    if let Some(h) = hooks {
        let extra = (h.context)();
        if !extra.trim().is_empty() {
            s.push('\n');
            s.push_str(extra.trim_end());
            s.push('\n');
        }
    }
    s.push_str("\nbacktrace:\n");
    s.push_str(c.backtrace);
    s.push_str(&format!("\n\nlast {} log lines:\n", log.len()));
    for l in log {
        s.push_str(l);
        s.push('\n');
    }
    match hooks {
        Some(h) => (h.redact)(&s),
        None => s,
    }
}

/// Runs `f`; if it panics, logs `what` (the job and its file) with the panic and returns
/// [`Panicked`]. Panics in rayon `join`/`scope` work and scoped threads that `f` started come
/// back here too (rayon and `std::thread::scope` resume them on the caller).
pub fn run<T>(what: impl Display, f: impl FnOnce() -> T) -> Result<T, Panicked> {
    struct Depth;
    impl Drop for Depth {
        fn drop(&mut self) {
            let _ = DEPTH.try_with(|d| d.set(d.get().saturating_sub(1)));
        }
    }
    let _ = DEPTH.try_with(|d| d.set(d.get() + 1));
    let depth = Depth;
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    drop(depth);
    result.map_err(|payload| {
        CAUGHT.fetch_add(1, Ordering::Relaxed);
        let message = payload_message(payload.as_ref());
        // Dropping a payload runs foreign code (its Drop) that could panic again: forget it.
        std::mem::forget(payload);
        let location = LAST_LOCATION.try_with(Cell::take).ok().flatten();
        let p = Panicked { message, location };
        error!(target: JOBS, "{what}: {p}; this job is dropped, the app goes on");
        p
    })
}

/// [`run`], and a panic it catches (not one a boundary inside it catches) also gets a crash
/// report with its backtrace, as one outside any boundary on the UI thread does, but not
/// offered at the next start: the app went on. For the app's event loop boundary, where a
/// panic is a bug in Specular worth the backtrace; decoder jobs use [`run`].
pub fn run_reported<T>(what: impl Display, f: impl FnOnce() -> T) -> Result<T, Panicked> {
    struct Restore(u32);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = REPORTED_AT.try_with(|r| r.set(self.0));
        }
    }
    let outer = REPORTED_AT.try_with(Cell::get).unwrap_or(0);
    let depth = DEPTH.try_with(Cell::get).unwrap_or(0);
    // `run` adds one to the depth: a panic at exactly that depth is this boundary's.
    let _ = REPORTED_AT.try_with(|r| r.set(depth + 1));
    let _restore = Restore(outer);
    run(what, f)
}

/// Panics caught by [`run`] since the process started.
pub fn caught() -> u64 {
    CAUGHT.load(Ordering::Relaxed)
}

fn payload_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic with a non-text payload".to_owned()
    }
}

/// Installs the panic hook (once; later calls do nothing). Call it on the UI thread: its
/// panics outside any [`run`] boundary write a crash report to `crash_dir` (`None`: no
/// report, only the log). Panics on other threads are logged; a boundary reports its own.
pub fn install(crash_dir: Option<PathBuf>) {
    let cfg = HookConfig {
        crash_dir,
        ui_thread: std::thread::current().id(),
    };
    if HOOK.set(cfg).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info.location().map(|l| l.to_string());
        let caught = DEPTH.try_with(Cell::get).unwrap_or(0) > 0;
        if caught {
            // The boundary logs it with the job's file; only the location is lost otherwise.
            let _ = LAST_LOCATION.try_with(|l| l.set(location));
            let depth = DEPTH.try_with(Cell::get).unwrap_or(0);
            let reported = REPORTED_AT.try_with(Cell::get).unwrap_or(0) == depth;
            if reported && let Some(dir) = HOOK.get().and_then(|c| c.crash_dir.as_ref()) {
                let thread = std::thread::current();
                let name = thread.name().unwrap_or("unnamed");
                match write_report(dir, info, name, false) {
                    Ok(p) => error!(target: JOBS, "crash report saved at {}", p.display()),
                    Err(e) => error!(target: JOBS, "crash report not saved: {e}"),
                }
            }
            return;
        }
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed").to_owned();
        let message = info
            .payload_as_str()
            .unwrap_or("panic with a non-text payload");
        let at = location.as_deref().unwrap_or("unknown location");
        let Some(cfg) = HOOK.get() else {
            previous(info);
            return;
        };
        if thread.id() == cfg.ui_thread {
            error!(target: JOBS, "the UI thread panicked at {at}: {message}; Specular closes");
            if let Some(dir) = &cfg.crash_dir {
                match write_report(dir, info, &name, true) {
                    Ok(p) => error!(target: JOBS, "crash report saved at {}", p.display()),
                    Err(e) => error!(target: JOBS, "crash report not saved: {e}"),
                }
            }
        } else {
            // A thread outside any boundary: it ends, the app goes on (a rayon pool's worker
            // survives through the pool's panic handler).
            error!(target: JOBS, "thread {name} panicked at {at}: {message}");
        }
        previous(info);
    }));
}

/// Writes a crash report for the panic `info` on thread `name` into `dir`, marks it to be
/// shown at the next start when `pending` (the app is ending), and keeps only the newest
/// [`KEEP_REPORTS`]. Local only.
fn write_report(
    dir: &Path,
    info: &PanicHookInfo<'_>,
    name: &str,
    pending: bool,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let stamp = utc(now);
    let mut path = dir.join(format!("crash-{}.txt", stamp.replace([':', ' '], "-")));
    let mut n = 1;
    while path.exists() {
        n += 1;
        path = dir.join(format!("crash-{}-{n}.txt", stamp.replace([':', ' '], "-")));
    }
    let message = info
        .payload_as_str()
        .unwrap_or("panic with a non-text payload");
    let location = info
        .location()
        .map_or_else(|| "unknown".to_owned(), |l| l.to_string());
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let text = report_text(
        &Crash {
            stamp: &stamp,
            thread: name,
            location: &location,
            message,
            backtrace: &backtrace,
        },
        REPORT_HOOKS.get(),
        &specular_log::recent_lines(REPORT_LOG_LINES),
    );
    let mut f = std::fs::File::create(&path)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    if pending {
        std::fs::write(dir.join(PENDING), path.to_string_lossy().as_bytes())?;
    }
    prune(dir);
    Ok(path)
}

/// The crash reports in `crash_dir`, oldest first (the names sort by time).
pub fn reports(crash_dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(crash_dir) else {
        return Vec::new();
    };
    let mut reports: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt"))
        })
        .collect();
    reports.sort();
    reports
}

/// Removes all but the newest [`KEEP_REPORTS`] reports in `dir`.
fn prune(dir: &Path) {
    let reports = reports(dir);
    let extra = reports.len().saturating_sub(KEEP_REPORTS);
    for p in &reports[..extra] {
        let _ = std::fs::remove_file(p);
    }
}

/// The report the last run left for this start to mention (once: the marker is removed).
pub fn take_pending_report(crash_dir: &Path) -> Option<PathBuf> {
    let marker = crash_dir.join(PENDING);
    let text = std::fs::read_to_string(&marker).ok()?;
    let _ = std::fs::remove_file(&marker);
    let path = PathBuf::from(text.trim());
    path.is_file().then_some(path)
}

/// `YYYY-MM-DD HH:MM:SS` for seconds since 1970 (UTC; Howard Hinnant's civil-from-days).
fn utc(secs: u64) -> String {
    let secs = secs as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// The panic handler for rayon pools: a panic in work spawned onto a pool (`spawn`, not
/// `join`/`scope`, which hand it to their caller) would otherwise abort the process. The panic
/// hook has logged it already.
pub fn rayon_panic_handler(payload: Box<dyn std::any::Any + Send>) {
    CAUGHT.fetch_add(1, Ordering::Relaxed);
    let message = payload_message(payload.as_ref());
    std::mem::forget(payload);
    error!(target: JOBS, "a rayon task panicked ({message}); the pool goes on");
}

/// File names that make a debug build panic where files are opened (test hook): a file whose
/// name contains this panics in [`trip`].
pub const TEST_PANIC_MARK: &str = "specular-test-panic";

/// Test hook: in debug builds, panics if `path`'s file name contains [`TEST_PANIC_MARK`], so
/// tests can put a panicking "decoder" behind any boundary with an ordinary file. Release
/// builds compile it to nothing.
#[inline]
pub fn trip(path: &Path) {
    #[cfg(debug_assertions)]
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(TEST_PANIC_MARK))
    {
        panic!("test panic for {}", path.display());
    }
    #[cfg(not(debug_assertions))]
    let _ = path;
}

/// File and folder names that make a debug build panic where files are sniffed (their
/// headers read to tell whether they are images) and where the Library lists a folder: see
/// [`trip_sniff`]. Distinct from [`TEST_PANIC_MARK`], so a test can list a file that panics
/// only once it is opened.
pub const TEST_SNIFF_MARK: &str = "specular-test-sniff";

/// Test hook like [`trip`], for [`TEST_SNIFF_MARK`]. Release builds compile it to nothing.
#[inline]
pub fn trip_sniff(path: &Path) {
    #[cfg(debug_assertions)]
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(TEST_SNIFF_MARK))
    {
        panic!("test sniff panic for {}", path.display());
    }
    #[cfg(not(debug_assertions))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_becomes_an_error_and_the_thread_goes_on() {
        let before = caught();
        let r = run(format_args!("test job {}", 1), || -> u32 {
            panic!("boom {}", 7)
        });
        let p = r.unwrap_err();
        assert_eq!(p.message, "boom 7");
        assert!(p.to_string().starts_with("internal error: boom 7"));
        assert!(caught() > before);
        assert_eq!(run("next job", || 5).unwrap(), 5);
        assert_eq!(DEPTH.with(Cell::get), 0);
    }

    #[test]
    fn panics_in_rayon_work_and_scoped_threads_come_back_to_the_boundary() {
        let r = run("rayon", || {
            rayon::join(|| 1, || -> i32 { panic!("in a rayon task") })
        });
        assert_eq!(r.unwrap_err().message, "in a rayon task");
        let r = run("scoped", || {
            std::thread::scope(|s| {
                specular_thread::spawn_scoped(s, "test-scoped", || panic!("in a scoped thread"));
            })
        });
        assert!(r.is_err());
    }

    /// A job that panics while holding a std lock poisons it; the lock stays usable for the
    /// jobs after it (`PoisonError::into_inner`, or `clear_poison` at the boundary), and
    /// parking_lot locks (most of Specular's) do not poison at all.
    #[test]
    fn a_lock_a_panicking_job_held_does_not_cascade() {
        use std::sync::{Mutex, PoisonError, RwLock};
        let m = std::sync::Arc::new(Mutex::new(vec![1]));
        let r = run("holds the lock", || {
            let mut g = m.lock().unwrap();
            g.push(2);
            panic!("while holding it");
        });
        assert!(r.is_err());
        assert!(m.is_poisoned());
        // The next job reads what was there.
        let next = run("next job", || {
            m.lock().unwrap_or_else(PoisonError::into_inner).push(3);
            m.lock().unwrap_or_else(PoisonError::into_inner).clone()
        });
        assert_eq!(next.unwrap(), [1, 2, 3]);
        m.clear_poison();
        assert!(m.lock().is_ok());

        let rw = RwLock::new(0);
        let _ = run("writer", || {
            let _w = rw.write().unwrap();
            panic!("writing");
        });
        assert_eq!(*rw.read().unwrap_or_else(PoisonError::into_inner), 0);

        let pl = parking_lot::Mutex::new(5);
        let _ = run("parking_lot", || {
            let _g = pl.lock();
            panic!("holding a parking_lot lock");
        });
        assert_eq!(*pl.lock(), 5);
    }

    #[test]
    fn the_component_is_the_crate_the_panic_happened_in() {
        let at = |l: &str| Panicked {
            message: "x".into(),
            location: Some(l.into()),
        };
        let p = at("crates/formats/specular-jpeg/src/huffman.rs:120:9");
        assert_eq!(p.component().as_deref(), Some("specular-jpeg"));
        assert_eq!(p.brief(), "internal error in specular-jpeg");
        assert_eq!(
            at("crates/specular-render-3d/src/a.rs:1:1")
                .component()
                .as_deref(),
            Some("specular-render-3d")
        );
        let reg = "/home/u/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/zune-jpeg-0.4.14/src/mcu.rs:3:4";
        assert_eq!(at(reg).component().as_deref(), Some("zune-jpeg"));
        let win =
            r"C:\Users\u\.cargo\registry\src\index.crates.io-1\hayro-syntax-0.3.0\src\lib.rs:3:4";
        assert_eq!(at(win).component().as_deref(), Some("hayro-syntax"));
        let std = at("/rustc/abc/library/core/src/slice/index.rs:3:4");
        assert_eq!(std.component(), None);
        assert_eq!(std.brief(), "internal error");
        let none = Panicked {
            message: "x".into(),
            location: None,
        };
        assert_eq!(none.brief(), "internal error");
    }

    #[test]
    fn the_test_hook_trips_on_the_marked_name_only() {
        assert!(run("plain", || trip(Path::new("/x/a.jpg"))).is_ok());
        let r = run("marked", || trip(Path::new("/x/a-specular-test-panic.jpg")));
        assert!(r.unwrap_err().message.contains("test panic for"));
    }

    #[test]
    fn pending_reports_are_taken_once_and_old_ones_pruned() {
        let dir = test_dir("pending");
        let report = dir.join("crash-2026-10-11-12-00-00.txt");
        std::fs::write(&report, "x").unwrap();
        std::fs::write(dir.join(PENDING), report.to_string_lossy().as_bytes()).unwrap();
        assert_eq!(take_pending_report(&dir), Some(report));
        assert_eq!(take_pending_report(&dir), None);
        for i in 0..KEEP_REPORTS + 3 {
            std::fs::write(dir.join(format!("crash-2026-10-11-12-00-{i:02}.txt")), "x").unwrap();
        }
        prune(&dir);
        let left = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(left, KEEP_REPORTS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn utc_formats() {
        assert_eq!(utc(1_700_000_000), "2023-11-14 22:13:20");
    }

    #[test]
    fn a_ui_thread_panic_leaves_a_report_for_the_next_start() {
        let dir = test_dir("report");
        let d = dir.clone();
        // The "UI thread" is a thread of its own, so its panic ends only it.
        let r = specular_thread::spawn("test-ui", move || {
            install(Some(d));
            // Inside a boundary: no report.
            assert!(run("job", || panic!("caught one")).is_err());
            panic!("the UI thread broke");
        })
        .join();
        assert!(r.is_err());
        let report = take_pending_report(&dir).expect("a pending report");
        let text = std::fs::read_to_string(&report).unwrap();
        assert!(text.contains("the UI thread broke"), "{text}");
        assert!(text.contains("nothing was sent anywhere"));
        assert!(!text.contains("caught one"));
        assert_eq!(take_pending_report(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_carry_context_and_log_lines_and_are_redacted() {
        let crash = Crash {
            stamp: "2026-10-12 10:00:00",
            thread: "main",
            location: "/home/ada/src/app.rs:1:1",
            message: "boom in /home/ada/Pictures/x.jpg",
            backtrace: "0: specular::main\n   at /home/ada/src/main.rs:5",
        };
        let hooks = ReportHooks {
            context: Box::new(|| "gpu:      Test GPU (driver 1.2)\n".into()),
            redact: Box::new(|s| s.replace("/home/ada", "~")),
        };
        let log = ["line one".to_owned(), "opened /home/ada/b.png".to_owned()];
        let text = report_text(&crash, Some(&hooks), &log);
        assert!(text.contains("gpu:      Test GPU (driver 1.2)"), "{text}");
        assert!(
            text.contains("last 2 log lines:\nline one\nopened ~/b.png\n"),
            "{text}"
        );
        assert!(text.contains("message:  boom in ~/Pictures/x.jpg"));
        assert!(text.contains("at ~/src/main.rs:5"));
        assert!(!text.contains("/home/ada"));
        // Without hooks: the panic and the log only.
        let plain = report_text(&crash, None, &[]);
        assert!(plain.contains("/home/ada/src/app.rs") && plain.contains("last 0 log lines"));
    }

    #[test]
    fn reports_lists_only_reports_oldest_first() {
        let dir = test_dir("list");
        std::fs::create_dir_all(&dir).unwrap();
        for n in [
            "crash-2026-10-12-b.txt",
            "crash-2026-10-11-a.txt",
            "pending",
            "x.txt",
        ] {
            std::fs::write(dir.join(n), "r").unwrap();
        }
        let names: Vec<String> = reports(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["crash-2026-10-11-a.txt", "crash-2026-10-12-b.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A fresh folder under the workspace's target folder (never the user's state folder).
    pub(crate) fn test_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/isolate"
        ))
        .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
