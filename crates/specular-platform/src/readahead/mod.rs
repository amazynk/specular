//! Read-ahead of the neighbour files the viewer is about to open (DESIGN.md §12.2
//! "Fast I/O").
//!
//! When the prefetch window moves, the app hands the files it is about to open to [`warm`]. A
//! single background thread then pulls them into the page cache, so the decode jobs that map them
//! a moment later fault on memory instead of on the disk. On Linux the reads go through io_uring
//! (a few large reads in flight at once, all files in one batch). When io_uring is unavailable
//! (old kernel, `kernel.io_uring_disabled`, seccomp, ENOSYS, EPERM, ENOMEM), decided at run time,
//! nothing is done and the decoder's page faults read the files as before. Other systems do
//! nothing here (macOS already asks for the whole file with `F_RDADVISE` when it is opened).
//!
//! `posix_fadvise(WILLNEED)` is kept as an explicit choice for comparison only: on a cold cache
//! it was slower than plain page faults (examples/readahead_bench.rs in specular-core).
//!
//! The backend is chosen by [`set_mode`] and can be forced with
//! `SPECULAR_READAHEAD=uring|fadvise|off|auto` for A/B runs.

use std::collections::VecDeque;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Instant, SystemTime};

#[cfg(target_os = "linux")]
mod uring;

/// How read-ahead is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backend {
    /// Batched io_uring reads (Linux 5.6+).
    IoUring,
    /// `posix_fadvise(POSIX_FADV_WILLNEED)` per file: the kernel reads it in the background.
    Fadvise,
    /// Nothing: the decoder's page faults read the file (the path before read-ahead existed).
    Off,
}

/// What the user (or the environment) asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// io_uring where available, else nothing.
    #[default]
    Auto,
    IoUring,
    Fadvise,
    Off,
}

/// io_uring reads in flight at once (`SPECULAR_READAHEAD_DEPTH` overrides it, for tuning).
pub const DEPTH: usize = 8;
/// Bytes per io_uring read (`SPECULAR_READAHEAD_CHUNK_KB` overrides it, for tuning).
pub const CHUNK: usize = 1 << 20;

/// [`DEPTH`] and [`CHUNK`], or their environment overrides.
#[cfg(target_os = "linux")]
fn ring_shape() -> (usize, usize) {
    static SHAPE: OnceLock<(usize, usize)> = OnceLock::new();
    *SHAPE.get_or_init(|| {
        let env = |k: &str| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.trim().parse::<usize>().ok())
        };
        (
            env("SPECULAR_READAHEAD_DEPTH").unwrap_or(DEPTH),
            env("SPECULAR_READAHEAD_CHUNK_KB").map_or(CHUNK, |k| k << 10),
        )
    })
}

/// Files remembered as already read, so a window that moves by one image re-reads nothing.
const RECENT: usize = 64;
/// The ring and its buffers are freed after this long without a request.
const IDLE: std::time::Duration = std::time::Duration::from_secs(10);

static MODE: AtomicU8 = AtomicU8::new(0);

/// Sets the read-ahead mode (from settings). `SPECULAR_READAHEAD` overrides it.
pub fn set_mode(mode: Mode) {
    MODE.store(mode as u8, Ordering::Relaxed);
}

fn mode() -> Mode {
    static ENV: OnceLock<Option<Mode>> = OnceLock::new();
    let env = *ENV.get_or_init(|| {
        let v = std::env::var("SPECULAR_READAHEAD").ok()?;
        match v.trim() {
            "uring" | "io_uring" => Some(Mode::IoUring),
            "fadvise" => Some(Mode::Fadvise),
            "off" | "0" => Some(Mode::Off),
            "auto" => Some(Mode::Auto),
            other => {
                specular_log::warn!(target: specular_log::IO,
                    "SPECULAR_READAHEAD={other}: expected uring, fadvise, off or auto");
                None
            }
        }
    });
    env.unwrap_or(match MODE.load(Ordering::Relaxed) {
        0 => Mode::Auto,
        1 => Mode::IoUring,
        2 => Mode::Fadvise,
        _ => Mode::Off,
    })
}

/// The backend [`warm`] uses right now: the mode, limited by what this system supports
/// (probed once, on first use).
pub fn backend() -> Backend {
    match mode() {
        Mode::Off => Backend::Off,
        Mode::Fadvise => fadvise_or_off(),
        Mode::Auto | Mode::IoUring => {
            if uring_available() {
                Backend::IoUring
            } else {
                Backend::Off
            }
        }
    }
}

fn fadvise_or_off() -> Backend {
    if cfg!(target_os = "linux") {
        Backend::Fadvise
    } else {
        Backend::Off
    }
}

/// Whether an io_uring ring with `IORING_OP_READ` can be set up in this process.
pub fn uring_available() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        #[cfg(target_os = "linux")]
        match uring::Ring::new(1, 4096) {
            Ok(_) => true,
            Err(e) => {
                specular_log::info!(target: specular_log::IO,
                    "io_uring unavailable ({e}); no read-ahead");
                false
            }
        }
        #[cfg(not(target_os = "linux"))]
        false
    })
}

/// Reads `files` into the page cache with `backend`, in order, and returns the bytes read (for
/// [`Backend::Fadvise`], the bytes advised). Blocks until done; [`warm`] is the asynchronous
/// form. For benchmarks and tests.
pub fn warm_blocking(files: &[File], backend: Backend) -> io::Result<u64> {
    run(files, backend, &|| false, &mut None)
}

/// Starts reading `paths` (most wanted first) into the page cache on the read-ahead thread and
/// returns at once. A newer call replaces the list: reads for the old one stop being issued.
/// Files read recently are skipped. A no-op with [`Mode::Off`] and on systems other than Linux.
pub fn warm(paths: Vec<PathBuf>) {
    // The backend itself is probed on the read-ahead thread, not the caller's (the UI thread).
    if paths.is_empty() || mode() == Mode::Off || cfg!(not(target_os = "linux")) {
        return;
    }
    let q = queue();
    let mut st = q.state.lock().unwrap_or_else(|e| e.into_inner());
    st.want = paths;
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    st.generation = generation;
    if !st.started {
        st.started = specular_thread::builder("io-readahead")
            .spawn(worker)
            .is_ok();
    }
    drop(st);
    q.wake.notify_one();
}

struct Queue {
    state: Mutex<Request>,
    wake: Condvar,
}

#[derive(Default)]
struct Request {
    want: Vec<PathBuf>,
    generation: u64,
    started: bool,
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

fn queue() -> &'static Queue {
    static Q: OnceLock<Queue> = OnceLock::new();
    Q.get_or_init(|| Queue {
        state: Mutex::new(Request::default()),
        wake: Condvar::new(),
    })
}

/// A file as last read: path, size and modification time.
type Seen = (PathBuf, u64, Option<SystemTime>);

/// Batches the worker finished (tests).
static BATCHES: AtomicU64 = AtomicU64::new(0);

fn worker() {
    crate::set_thread_class(crate::ThreadClass::Utility);
    // A panic costs the batch in hand: the worker restarts after a back-off (`started` stays
    // set, so nothing else would start it again).
    specular_log::service::supervise(
        "io-readahead",
        specular_log::service::Policy::RESTART,
        || false,
        work,
        |_| {},
    );
}

fn work() {
    let q = queue();
    let mut done = 0u64;
    let mut recent: VecDeque<Seen> = VecDeque::with_capacity(RECENT);
    let mut ring: RingSlot = None;
    loop {
        let (paths, generation) = {
            let mut st = q.state.lock().unwrap_or_else(|e| e.into_inner());
            while st.generation == done {
                let (next, timeout) = q
                    .wake
                    .wait_timeout(st, IDLE)
                    .unwrap_or_else(|e| e.into_inner());
                st = next;
                if timeout.timed_out() {
                    // Browsing paused: give the ring's buffers back.
                    ring = None;
                }
            }
            done = st.generation;
            (std::mem::take(&mut st.want), st.generation)
        };
        specular_log::service::trip();
        let stop = || GENERATION.load(Ordering::Acquire) != generation;
        let mut files = Vec::with_capacity(paths.len());
        let mut keys = Vec::with_capacity(paths.len());
        for path in paths {
            // Only regular files: opening a FIFO blocks.
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let key = (path, meta.len(), meta.modified().ok());
            if !meta.is_file() || recent.contains(&key) {
                continue;
            }
            if let Ok(f) = File::open(&key.0) {
                files.push(f);
                keys.push(key);
            }
        }
        if files.is_empty() {
            continue;
        }
        let backend = backend();
        let t0 = Instant::now();
        let r = run(&files, backend, &stop, &mut ring);
        let stopped = stop();
        specular_log::debug!(target: specular_log::IO,
            "read-ahead ({backend:?}): {} files, {:?} bytes in {:.1} ms{}",
            files.len(), r.as_ref().ok(), t0.elapsed().as_secs_f64() * 1e3,
            if stopped { " (superseded)" } else { "" });
        BATCHES.fetch_add(1, Ordering::Relaxed);
        if r.is_ok() && !stopped {
            for k in keys {
                if recent.len() == RECENT {
                    recent.pop_front();
                }
                recent.push_back(k);
            }
        }
    }
}

#[cfg(target_os = "linux")]
type RingSlot = Option<uring::Ring>;
#[cfg(not(target_os = "linux"))]
type RingSlot = Option<()>;

fn run(
    files: &[File],
    backend: Backend,
    stop: &dyn Fn() -> bool,
    ring: &mut RingSlot,
) -> io::Result<u64> {
    match backend {
        Backend::Off => Ok(0),
        Backend::Fadvise => fadvise(files, stop),
        #[cfg(target_os = "linux")]
        Backend::IoUring => {
            if ring.is_none() {
                let (depth, chunk) = ring_shape();
                match uring::Ring::new(depth, chunk) {
                    Ok(r) => *ring = Some(r),
                    Err(e) => {
                        // Transient (ENOMEM from the locked-memory limit, seen with several
                        // rings per user): leave it to the decoder's page faults this time.
                        specular_log::debug!(target: specular_log::IO,
                            "io_uring ring setup failed ({e}); no read-ahead this time");
                        return Ok(0);
                    }
                }
            }
            let r = ring.as_mut().map_or(Ok(0), |r| r.read(files, stop));
            match r {
                Ok(n) => Ok(n),
                Err(e) => {
                    // The kernel refused the reads themselves: the decoder's page faults read
                    // these files, and the next request sets up a new ring.
                    specular_log::info!(target: specular_log::IO,
                        "io_uring read-ahead failed ({e}); left to page faults");
                    *ring = None;
                    Ok(0)
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        Backend::IoUring => {
            let _ = ring;
            Ok(0)
        }
    }
}

/// `posix_fadvise(WILLNEED)` on each whole file: the kernel queues the reads and returns.
fn fadvise(files: &[File], stop: &dyn Fn() -> bool) -> io::Result<u64> {
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut n = 0;
    for f in files {
        if stop() {
            break;
        }
        #[cfg(target_os = "linux")]
        {
            let len = f.metadata()?.len();
            if rustix::fs::fadvise(
                f,
                0,
                std::num::NonZeroU64::new(len),
                rustix::fs::Advice::WillNeed,
            )
            .is_ok()
            {
                n += len;
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = f;
    }
    Ok(n)
}

/// Drops `file` from the page cache (`posix_fadvise(DONTNEED)`), for cold-cache benchmarks.
/// Only clean, unmapped pages go. Returns whether the call succeeded.
pub fn evict(file: &File) -> bool {
    #[cfg(target_os = "linux")]
    return rustix::fs::fadvise(file, 0, None, rustix::fs::Advice::DontNeed).is_ok();
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        // Under the build's target directory (never the system temp directory).
        let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
            || PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target")),
            PathBuf::from,
        );
        let dir = target.join("test-scratch").join("readahead");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn file_of(name: &str, len: usize) -> File {
        let p = scratch(name);
        let mut f = File::create(&p).unwrap();
        let data: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
        f.write_all(&data).unwrap();
        f.sync_all().unwrap();
        File::open(&p).unwrap()
    }

    #[test]
    fn every_backend_reads_whole_files() {
        let files = [
            file_of("a", 3 * CHUNK + 17),
            file_of("b", 0),
            file_of("c", 5000),
        ];
        let total = (3 * CHUNK + 17 + 5000) as u64;
        assert_eq!(warm_blocking(&files, Backend::Off).unwrap(), 0);
        #[cfg(target_os = "linux")]
        {
            for f in &files {
                assert!(evict(f));
            }
            assert_eq!(warm_blocking(&files, Backend::Fadvise).unwrap(), total);
            if uring_available() {
                assert_eq!(warm_blocking(&files, Backend::IoUring).unwrap(), total);
                // Again, reusing nothing: a fresh ring each call.
                assert_eq!(
                    warm_blocking(&files[..1], Backend::IoUring).unwrap(),
                    total - 5000
                );
            }
        }
        let _ = total;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn uring_stops_when_superseded() {
        if !uring_available() {
            return;
        }
        let files = [file_of("big", 16 * CHUNK)];
        let mut ring = uring::Ring::new(2, CHUNK).unwrap();
        let calls = std::cell::Cell::new(0);
        // Stop after the first two reads were queued.
        let n = ring
            .read(&files, &|| {
                calls.set(calls.get() + 1);
                calls.get() > 2
            })
            .unwrap();
        assert!(n >= CHUNK as u64 && n < 16 * CHUNK as u64, "{n}");
        // The ring is reusable after an early stop.
        assert_eq!(ring.read(&files, &|| false).unwrap(), 16 * CHUNK as u64);
    }

    /// A panic in the worker costs one batch: it restarts and reads the next one.
    #[test]
    fn worker_restarts_after_a_panic() {
        if mode() == Mode::Off || cfg!(not(target_os = "linux")) {
            return;
        }
        specular_log::service::arm("io-readahead", 1);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut i = 0;
        let before = loop {
            assert!(
                std::time::Instant::now() < deadline,
                "the worker never panicked"
            );
            let p = scratch(&format!("panic-{i}"));
            std::fs::write(&p, b"hello").unwrap();
            warm(vec![p]);
            i += 1;
            std::thread::sleep(std::time::Duration::from_millis(50));
            if specular_log::service::deaths("io-readahead") > 0 {
                break BATCHES.load(Ordering::Relaxed);
            }
        };
        while BATCHES.load(Ordering::Relaxed) == before {
            assert!(
                std::time::Instant::now() < deadline,
                "no batch after the restart"
            );
            let p = scratch(&format!("after-{i}"));
            std::fs::write(&p, b"hello again").unwrap();
            warm(vec![p]);
            i += 1;
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn warm_is_asynchronous_and_harmless() {
        let p = scratch("w");
        std::fs::write(&p, b"hello").unwrap();
        warm(vec![
            p.clone(),
            scratch("missing"),
            PathBuf::from("/dev/null"),
        ]);
        warm(vec![p]);
    }
}
