//! Real-world shakedown: every decoder over whole corpora, one child process per file, so a
//! panic, abort, hang or memory blow-up costs that file and the sweep goes on.
//!
//! ```text
//! cargo run -p specular-thumbs --example shakedown -- [--jobs N] [--timeout SECS]
//!     [--rss-limit MIB] [--out DIR] [--label NAME] [--exclude SUBSTR]... [--max-files N]
//!     [--thumbnail-exe SPECULAR [--thumbnail-size PX]] [--compare PREVIOUS_DIR]
//!     [--files-from LIST] ROOT...
//! ```
//!
//! Each file is opened through the registry as the viewer opens it, then thumbnailed as the
//! gallery does ([`generate`], 256 px class), decoded at the viewer's first level (the
//! coarsest scale still covering 2048 px), its next three pages opened and decoded, its first
//! three animation frames decoded and, for medical images, the modality frame read. The child
//! reports one line; the parent classifies it:
//!
//! - `ok`: every step succeeded;
//! - `unrecognised`: no decoder recognises the content (not counted against pass rates);
//! - `error`: a clean [`DecodeError`] (truncated, malformed, unsupported feature, too large, I/O);
//! - `panic`: a panic, caught in the child (or by a decoder's own boundary, `Internal`);
//! - `crash`: the child died otherwise (abort, stack overflow, signal);
//! - `timeout`: over the per-file budget (killed);
//! - `oom`: over the resident-memory limit (killed).
//!
//! With `--thumbnail-exe`, each file is then given to the desktop thumbnailer
//! (`specular --thumbnail IN OUT PX`, in its own process with empty settings and cache) and
//! classified the same way (`thumb_*` columns). Children run under GNU time (`/usr/bin/time`)
//! when it is installed, in their own process group, for their exact peak resident set and
//! CPU time; otherwise the peak is polled from `/proc`.
//!
//! The sweep is meant for Linux. Elsewhere it builds and runs, but untimed, with no memory peak
//! (so no `oom`), and a crash is reported by its exit code instead of as `crash`.
//!
//! Results go to `DIR` (default `$CARGO_TARGET_DIR/shakedown/<label>`): `files.tsv` (one row per
//! file), `summary.tsv` and `summary.json` (per format: files, ok, error, panic, crash,
//! timeout, oom, unrecognised, pass rate, slowest file and peak memory outliers), `top.tsv`
//! (the 30 slowest files and the 30 largest peaks) and, with `--compare`, `compare.tsv` (status
//! changes, twice-as-slow and twice-as-large files against the previous run's `files.tsv`). Formats are
//! the decoder's name when the file opened, else the registry's probe of its first bytes, else
//! `ext:<extension>`. `tools/shakedown.sh` runs it over `tools/corpus`.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use specular_core::CancelToken;
use specular_decode::{DecodeError, DecodeRequest, Registry, Session};
use specular_thumbs::generate::{GenOptions, generate, meta_of};
use specular_thumbs::pixels::pick_scale;

/// The steps of one file, in order (the child reports which one failed).
const STEPS: [&str; 7] = [
    "open",
    "thumbnail",
    "view",
    "pages",
    "frames",
    "modality",
    "done",
];
static STEP: AtomicUsize = AtomicUsize::new(0);
static PANIC: Mutex<Option<String>> = Mutex::new(None);
/// When the current step started, and the time each finished step took (ms).
type StepTimes = (Option<Instant>, Vec<(usize, u128)>);
static TIMES: Mutex<StepTimes> = Mutex::new((None, Vec::new()));

/// A whole-image decode larger than this (estimated) is skipped: the viewer would tile it.
const VIEW_MEMORY: u64 = 3 << 30;
/// Peak memory (MiB) above which a file is listed as an outlier.
const OUTLIER_MIB: u64 = 1024;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--one") {
        let path = PathBuf::from(args.get(1).expect("--one PATH"));
        child(&path);
        return;
    }
    parent(args);
}

// ---- child ---------------------------------------------------------------------------------

fn set_step(i: usize) {
    let prev = STEP.swap(i, Ordering::Relaxed);
    if let Ok(mut t) = TIMES.lock() {
        let now = Instant::now();
        if let Some(t0) = t.0 {
            t.1.push((prev, now.duration_since(t0).as_millis()));
        }
        t.0 = Some(now);
    }
    // On stderr too, so a timeout or a crash (whose stderr tail the parent keeps) names it.
    eprintln!("step: {}", STEPS[i]);
}

fn child(path: &Path) {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-text panic".into());
        // Also on stderr: a panic that aborts (one that cannot unwind) leaves only that.
        eprintln!("panic: {msg}{loc}");
        if let Ok(mut p) = PANIC.lock()
            && p.is_none()
        {
            *p = Some(format!("{msg}{loc}"));
        }
    }));
    let format = Mutex::new(String::new());
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| steps(path, &format)));
    let format = format.into_inner().unwrap_or_default();
    let step = STEPS[STEP.load(Ordering::Relaxed)];
    let (status, msg) = match r {
        Ok(Ok(note)) => ("ok", note),
        Ok(Err(DecodeError::UnsupportedFormat)) => ("unrecognised", String::new()),
        Ok(Err(DecodeError::Internal(m))) => ("panic", m),
        Ok(Err(e)) => ("error", e.to_string()),
        Err(_) => (
            "panic",
            PANIC
                .lock()
                .ok()
                .and_then(|p| p.clone())
                .unwrap_or_default(),
        ),
    };
    let hwm = peak_kib();
    // Per-step times, as `open=12 thumbnail=340 …` (the step that failed included).
    set_step(STEP.load(Ordering::Relaxed));
    let times = TIMES
        .lock()
        .map(|t| {
            t.1.iter()
                .map(|(s, ms)| format!("{}={ms}", STEPS[*s]))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let line = format!(
        "RESULT\t{status}\t{format}\t{step}\t{hwm}\t{times}\t{}\n",
        clean(&msg)
    );
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.flush();
}

fn steps(path: &Path, format: &Mutex<String>) -> Result<String, DecodeError> {
    let reg = Registry::new();
    let cancel = CancelToken::new();
    let mut notes = Vec::new();
    set_step(0);
    let s = reg.open_path(path)?;
    if let Ok(mut f) = format.lock() {
        *f = s.info().format.name().to_owned();
    }
    let _ = meta_of(s.as_ref());
    set_step(1);
    generate(s.as_ref(), 256, GenOptions::default(), &cancel, &mut |_| {})?;
    set_step(2);
    let info = s.info().clone();
    let scales: Vec<u32> = if info.scales.is_empty() {
        vec![1]
    } else {
        info.scales.to_vec()
    };
    let long = info.width.max(info.height);
    let req = DecodeRequest::full(pick_scale(&scales, long, 2048));
    let mem = s.decode_memory(&req);
    if mem > VIEW_MEMORY {
        notes.push(format!("view skipped ({} MiB)", mem >> 20));
    } else {
        s.decode(&req, &cancel)?;
    }
    set_step(3);
    let small = |s: &dyn Session| {
        let i = s.info();
        let sc = if i.scales.is_empty() {
            vec![1]
        } else {
            i.scales.to_vec()
        };
        DecodeRequest::full(pick_scale(&sc, i.width.max(i.height), 256))
    };
    let pages = s.page_count().min(4);
    for p in 1..pages {
        match s.open_page(p) {
            Ok(ps) => {
                let r = small(ps.as_ref());
                if ps.decode_memory(&r) <= VIEW_MEMORY {
                    ps.decode(&r, &cancel)?;
                }
            }
            Err(DecodeError::Unsupported(_)) => break,
            Err(e) => return Err(e),
        }
    }
    set_step(4);
    if s.animation().is_some() {
        let mut fr = s.frames(small(s.as_ref()).scale)?;
        for _ in 0..3 {
            match fr.next_frame(&cancel) {
                Some(f) => {
                    f?;
                }
                None => break,
            }
        }
    }
    set_step(5);
    if let Some(m) = s.modality_frame(&cancel) {
        m?;
    }
    set_step(6);
    Ok(notes.join("; "))
}

/// The process's peak resident set (KiB) from `/proc`, 0 where there is none.
fn peak_kib() -> u64 {
    proc_status_kib("self", "VmHWM:")
}

fn proc_status_kib(pid: &str, key: &str) -> u64 {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

fn clean(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if s.len() > 400 {
        let mut end = 400;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    } else {
        s
    }
}

// ---- parent --------------------------------------------------------------------------------

struct Opts {
    jobs: usize,
    timeout: Duration,
    rss_limit_kib: u64,
    out: PathBuf,
    exclude: Vec<String>,
    max_files: usize,
    roots: Vec<PathBuf>,
    /// `specular` to run as `--thumbnail IN OUT SIZE` after each file's own steps.
    thumbnail_exe: Option<PathBuf>,
    thumbnail_size: u32,
    /// A previous run's output folder (or its `files.tsv`) to compare against.
    compare: Option<PathBuf>,
}

fn parse(args: Vec<String>) -> Opts {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    let mut o = Opts {
        jobs: 4,
        timeout: Duration::from_secs(60),
        rss_limit_kib: 8 << 20,
        out: PathBuf::new(),
        exclude: Vec::new(),
        max_files: usize::MAX,
        roots: Vec::new(),
        thumbnail_exe: None,
        thumbnail_size: 256,
        compare: None,
    };
    let mut label = "latest".to_owned();
    let mut out = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--jobs" => o.jobs = val().parse().expect("jobs"),
            "--timeout" => o.timeout = Duration::from_secs_f64(val().parse().expect("timeout")),
            "--rss-limit" => o.rss_limit_kib = val().parse::<u64>().expect("MiB") << 10,
            "--out" => out = Some(PathBuf::from(val())),
            "--label" => label = val(),
            "--exclude" => o.exclude.push(val()),
            "--max-files" => o.max_files = val().parse().expect("max files"),
            "--thumbnail-exe" => o.thumbnail_exe = Some(PathBuf::from(val())),
            "--thumbnail-size" => o.thumbnail_size = val().parse().expect("thumbnail size"),
            "--compare" => o.compare = Some(PathBuf::from(val())),
            // One path per line (a previous run's slow or failing files, say).
            "--files-from" => {
                let list = std::fs::read_to_string(val()).expect("--files-from list");
                o.roots
                    .extend(list.lines().filter(|l| !l.is_empty()).map(PathBuf::from));
            }
            _ => o.roots.push(PathBuf::from(a)),
        }
    }
    o.out = out.unwrap_or_else(|| target.join("shakedown").join(label));
    o
}

/// Every file below the roots (symbolic links to files followed, to folders not), plus the
/// folders that open as one image, in path order.
fn collect(o: &Opts, reg: &Registry) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack: Vec<PathBuf> = o.roots.clone();
    let excluded = |p: &Path| {
        // With a trailing separator, so `--exclude /real/` also stops at the folder itself.
        let s = format!("{}/", p.to_string_lossy());
        o.exclude.iter().any(|x| s.contains(x.as_str()))
    };
    while let Some(p) = stack.pop() {
        if excluded(&p) {
            continue;
        }
        let Ok(lm) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if lm.is_dir() {
            if specular_core::is_image_folder(&p) || reg.supports_file(&p) {
                files.push(p.clone());
            }
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.flatten().map(|e| e.path()));
            }
        } else if std::fs::metadata(&p).is_ok_and(|m| m.is_file())
            // A link into an excluded tree is excluded too.
            && !std::fs::canonicalize(&p).is_ok_and(|c| excluded(&c))
        {
            files.push(p);
        }
    }
    files.sort();
    files.truncate(o.max_files);
    files
}

/// The format a file is filed under when the child could not say.
fn guess(reg: &Registry, p: &Path) -> String {
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(p)
        && std::fs::metadata(p).is_ok_and(|m| m.is_file())
    {
        let _ = f.take(64).read_to_end(&mut head);
    }
    if let Some(d) = reg.probe(&head) {
        return d.format().name().to_owned();
    }
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "(none)".into());
    format!("ext:{ext}")
}

/// GNU time, which reports a child's exact peak resident set and CPU time on exit (polling
/// `/proc` misses the peak of a process that ends between two polls).
const TIME: &str = "/usr/bin/time";

/// What a slot is running for its file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The harness's own child: open, thumbnail, view, pages, frames, modality.
    Steps,
    /// `specular --thumbnail`, the desktop thumbnailer.
    Thumbnail,
}

struct Running {
    child: Child,
    phase: Phase,
    path: PathBuf,
    started: Instant,
    peak_kib: u64,
    err_file: PathBuf,
    /// GNU time's report, when the child runs under it (then in its own process group).
    rusage_file: Option<PathBuf>,
    /// The steps' result while the thumbnailer runs.
    row: Option<Row>,
}

#[derive(Clone, Default)]
struct Row {
    path: PathBuf,
    format: String,
    status: &'static str,
    step: String,
    ms: u128,
    cpu_ms: u128,
    peak_mib: u64,
    msg: String,
    /// Time per step (`open=12 thumbnail=340 …`).
    step_ms: String,
    thumb: Option<Thumb>,
}

/// The desktop thumbnailer's result for one file.
#[derive(Clone)]
struct Thumb {
    status: &'static str,
    ms: u128,
    peak_mib: u64,
    msg: String,
}

/// The process's exact peak (KiB) and CPU time (ms) from GNU time's report, if it ran to the
/// end under it.
struct Rusage {
    max_kib: u64,
    cpu_ms: u128,
}

fn read_rusage(p: &Path) -> Option<Rusage> {
    let s = std::fs::read_to_string(p).ok()?;
    let l = s.lines().rev().find_map(|l| l.strip_prefix("RUSAGE "))?;
    let f: Vec<&str> = l.split_whitespace().collect();
    let max_kib = f.first()?.parse().ok()?;
    let secs = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let cpu_ms = ((secs(1) + secs(2)) * 1000.0) as u128;
    Some(Rusage { max_kib, cpu_ms })
}

fn parent(args: Vec<String>) {
    let o = parse(args);
    assert!(!o.roots.is_empty(), "no roots given");
    std::fs::create_dir_all(&o.out).expect("output folder");
    let reg = Registry::new();
    let files = collect(&o, &reg);
    let exe = std::env::current_exe().expect("own path");
    let timed = Path::new(TIME).is_file();
    let home = o.out.join("home");
    if o.thumbnail_exe.is_some() {
        // The thumbnailer reads settings and the gallery's cache: give it empty ones, never
        // the user's.
        for d in ["config", "cache", "state", "data", "thumbs"] {
            std::fs::create_dir_all(home.join(d)).expect("thumbnailer home");
        }
    }
    eprintln!(
        "shakedown: {} files, {} jobs, {:?} budget{}{}, out {}",
        files.len(),
        o.jobs,
        o.timeout,
        if timed { ", GNU time" } else { "" },
        if o.thumbnail_exe.is_some() {
            ", desktop thumbnails"
        } else {
            ""
        },
        o.out.display()
    );
    let mut pending = files.into_iter();
    let mut running: Vec<Option<Running>> = (0..o.jobs).map(|_| None).collect();
    let mut rows: Vec<Row> = Vec::new();
    let mut tsv =
        std::io::BufWriter::new(std::fs::File::create(o.out.join("files.tsv")).expect("tsv"));
    let _ = writeln!(
        tsv,
        "path\tformat\tstatus\tstep\tms\tpeak_mib\tmessage\tcpu_ms\tstep_ms\tthumb_status\tthumb_ms\tthumb_peak_mib\tthumb_message"
    );
    let t0 = Instant::now();
    let mut left = true;
    loop {
        let mut busy = false;
        for (slot, run) in running.iter_mut().enumerate() {
            if run.is_none() && left {
                match pending.next() {
                    Some(p) => *run = Some(spawn(&o, &exe, &p, slot, Phase::Steps, timed, &home)),
                    None => left = false,
                }
            }
            let Some(r) = run.as_mut() else { continue };
            busy = true;
            r.peak_kib = r.peak_kib.max(poll_kib(&r.child, r.rusage_file.is_some()));
            let over_time = r.started.elapsed() > o.timeout;
            let over_mem = r.peak_kib > o.rss_limit_kib;
            let finished = match r.child.try_wait() {
                Ok(Some(st)) => Some(st),
                Ok(None) if over_time || over_mem => {
                    stop(&mut r.child, r.rusage_file.is_some());
                    None
                }
                Ok(None) => continue,
                Err(_) => None,
            };
            let mut r = run.take().expect("running");
            let row = match r.phase {
                Phase::Steps => {
                    let row = finish(&reg, &mut r, finished, over_time, over_mem);
                    let thumb = o.thumbnail_exe.is_some()
                        && !matches!(row.status, "timeout" | "oom" | "unrecognised");
                    if thumb {
                        let mut next =
                            spawn(&o, &exe, &r.path, slot, Phase::Thumbnail, timed, &home);
                        next.row = Some(row);
                        *run = Some(next);
                        continue;
                    }
                    row
                }
                Phase::Thumbnail => {
                    let t = finish_thumb(&o, &mut r, finished, over_time, over_mem);
                    let mut row = r.row.take().expect("steps row");
                    row.thumb = Some(t);
                    row
                }
            };
            let (ts, tms, tpeak, tmsg) = match &row.thumb {
                Some(t) => (
                    t.status,
                    t.ms.to_string(),
                    t.peak_mib.to_string(),
                    t.msg.as_str(),
                ),
                None => ("-", String::new(), String::new(), ""),
            };
            let _ = writeln!(
                tsv,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{ts}\t{tms}\t{tpeak}\t{tmsg}",
                row.path.display(),
                row.format,
                row.status,
                row.step,
                row.ms,
                row.peak_mib,
                row.msg,
                row.cpu_ms,
                row.step_ms,
            );
            if !matches!(row.status, "ok" | "error" | "unrecognised") {
                eprintln!(
                    "{:>8} {:<10} {} [{}] {}",
                    row.status,
                    row.format,
                    row.path.display(),
                    row.step,
                    row.msg
                );
            }
            if let Some(t) = &row.thumb
                && !matches!(t.status, "ok" | "error")
            {
                eprintln!(
                    "{:>8} {:<10} {} [desktop thumbnail] {}",
                    t.status,
                    row.format,
                    row.path.display(),
                    t.msg
                );
            }
            rows.push(row);
            if rows.len().is_multiple_of(500) {
                eprintln!(
                    "shakedown: {} done, {:.0} s",
                    rows.len(),
                    t0.elapsed().as_secs_f64()
                );
            }
        }
        if !busy && !left {
            break;
        }
        // Polling: no threads, a few children at a time.
        std::thread::sleep(Duration::from_millis(15));
    }
    drop(tsv);
    for slot in 0..o.jobs {
        for f in ["stderr", "rusage", "thumb"] {
            let ext = if f == "thumb" { "png" } else { "txt" };
            let _ = std::fs::remove_file(o.out.join(format!("{f}-{slot}.{ext}")));
        }
    }
    if o.thumbnail_exe.is_some() {
        let _ = std::fs::remove_dir_all(&home);
    }
    summarise(&o, &rows, t0.elapsed());
    if let Some(prev) = &o.compare {
        compare(&o, prev, &rows);
    }
}

fn spawn(
    o: &Opts,
    exe: &Path,
    p: &Path,
    slot: usize,
    phase: Phase,
    timed: bool,
    home: &Path,
) -> Running {
    let err_file = o.out.join(format!("stderr-{slot}.txt"));
    let err = std::fs::File::create(&err_file).expect("stderr file");
    let rusage_file = timed.then(|| o.out.join(format!("rusage-{slot}.txt")));
    let mut argv: Vec<std::ffi::OsString> = Vec::new();
    if let Some(f) = &rusage_file {
        let _ = std::fs::remove_file(f);
        argv.extend(["-f".into(), "RUSAGE %M %U %S".into(), "-o".into()]);
        argv.push(f.into());
    }
    match phase {
        Phase::Steps => {
            argv.push(exe.into());
            argv.push("--one".into());
            argv.push(p.into());
        }
        Phase::Thumbnail => {
            argv.push(o.thumbnail_exe.clone().expect("thumbnailer").into());
            argv.push("--thumbnail".into());
            argv.push(p.into());
            let png = o.out.join(format!("thumb-{slot}.png"));
            let _ = std::fs::remove_file(&png);
            argv.push(png.into());
            argv.push(o.thumbnail_size.to_string().into());
        }
    }
    let (prog, rest) = if timed {
        (std::ffi::OsString::from(TIME), &argv[..])
    } else {
        (argv[0].clone(), &argv[1..])
    };
    let mut cmd = Command::new(prog);
    cmd.args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(err)
        // Decoders' own threads: a few, as a gallery worker has.
        .env("RAYON_NUM_THREADS", "2");
    if phase == Phase::Thumbnail {
        cmd.env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("SPECULAR_THUMB_CACHE", home.join("thumbs"));
    }
    #[cfg(unix)]
    if timed {
        // Its own group, so a timeout ends the decoder and not only GNU time.
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    }
    let child = cmd.spawn().expect("spawn child");
    Running {
        child,
        phase,
        path: p.to_path_buf(),
        started: Instant::now(),
        peak_kib: 0,
        err_file,
        rusage_file,
        row: None,
    }
}

/// The peak resident set (KiB) seen so far: the decoder's own, under GNU time its child's.
fn poll_kib(child: &Child, timed: bool) -> u64 {
    let pid = child.id().to_string();
    let pid = if timed {
        match std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) {
            Ok(c) => match c.split_whitespace().next() {
                Some(c) => c.to_owned(),
                None => return 0,
            },
            Err(_) => return 0,
        }
    } else {
        pid
    };
    proc_status_kib(&pid, "VmHWM:").max(proc_status_kib(&pid, "VmRSS:"))
}

/// Kills a child over its budget (its whole process group under GNU time).
fn stop(child: &mut Child, group: bool) {
    if group {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The child's stderr without the step lines and GNU time's own notes, last 600 bytes.
fn stderr_tail(err: &str) -> String {
    let tail: Vec<&str> = err
        .lines()
        .filter(|l| !l.starts_with("step: ") && !l.starts_with("Command "))
        .collect();
    let tail = tail.join(" ");
    let mut cut = tail.len().saturating_sub(600);
    while !tail.is_char_boundary(cut) {
        cut += 1;
    }
    clean(tail[cut..].trim())
}

fn exit_text(st: Option<std::process::ExitStatus>, ru: Option<&Path>) -> String {
    #[cfg(unix)]
    let signal = |s: std::process::ExitStatus| std::os::unix::process::ExitStatusExt::signal(&s);
    // Elsewhere there are no signals: a crash reads as its exit code (an NTSTATUS on Windows).
    #[cfg(not(unix))]
    let signal = |_: std::process::ExitStatus| None::<i32>;
    // Under GNU time the decoder's signal shows as time's exit status 128 + signal, and in
    // its report as "Command terminated by signal N".
    if let Some(sig) = ru
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Command terminated by signal "))
                .and_then(|n| n.trim().parse::<i32>().ok())
        })
    {
        return format!("signal {sig}");
    }
    st.map_or("lost".into(), |s| match signal(s) {
        Some(sig) => format!("signal {sig}"),
        None => format!("exit {}", s.code().unwrap_or(-1)),
    })
}

fn finish(
    reg: &Registry,
    r: &mut Running,
    st: Option<std::process::ExitStatus>,
    over_time: bool,
    over_mem: bool,
) -> Row {
    let ms = r.started.elapsed().as_millis();
    let mut out = String::new();
    if let Some(mut so) = r.child.stdout.take() {
        let _ = so.read_to_string(&mut out);
    }
    let ru = r.rusage_file.as_deref().and_then(read_rusage);
    let line = out.lines().find(|l| l.starts_with("RESULT\t"));
    let mut row = Row {
        path: r.path.clone(),
        status: "crash",
        ms,
        cpu_ms: ru.as_ref().map_or(0, |u| u.cpu_ms),
        peak_mib: r.peak_kib.max(ru.as_ref().map_or(0, |u| u.max_kib)) >> 10,
        ..Row::default()
    };
    // The child's stderr: its step lines (the last names the step a timeout or a crash was in)
    // and anything else it printed.
    let err = std::fs::read(&r.err_file).unwrap_or_default();
    let err = String::from_utf8_lossy(&err);
    if line.is_none() {
        row.step = err
            .lines()
            .filter_map(|l| l.strip_prefix("step: "))
            .next_back()
            .unwrap_or_default()
            .to_owned();
    }
    match (line, st) {
        (Some(l), _) => {
            let f: Vec<&str> = l.splitn(7, '\t').collect();
            row.status = match f.get(1).copied() {
                Some("ok") => "ok",
                Some("unrecognised") => "unrecognised",
                Some("error") => "error",
                _ => "panic",
            };
            row.format = f.get(2).copied().unwrap_or_default().to_owned();
            row.step = f.get(3).copied().unwrap_or_default().to_owned();
            let hwm: u64 = f.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
            row.peak_mib = row.peak_mib.max(hwm >> 10);
            row.step_ms = f.get(5).copied().unwrap_or_default().to_owned();
            row.msg = f.get(6).copied().unwrap_or_default().to_owned();
        }
        (None, _) if over_mem => {
            row.status = "oom";
            row.msg = format!("over {} MiB resident", r.peak_kib >> 10);
        }
        (None, _) if over_time => {
            row.status = "timeout";
        }
        (None, st) => {
            row.msg = format!(
                "{}; {}",
                exit_text(st, r.rusage_file.as_deref()),
                stderr_tail(&err)
            );
        }
    }
    if row.format.is_empty() {
        row.format = guess(reg, &r.path);
    }
    row
}

/// Classifies `specular --thumbnail`: exit 0 with a PNG written is ok, exit 1 a clean error
/// (or its own watchdog), exit 101 a panic, a signal a crash.
fn finish_thumb(
    o: &Opts,
    r: &mut Running,
    st: Option<std::process::ExitStatus>,
    over_time: bool,
    over_mem: bool,
) -> Thumb {
    let ms = r.started.elapsed().as_millis();
    let ru = r.rusage_file.as_deref().and_then(read_rusage);
    let peak_mib = r.peak_kib.max(ru.as_ref().map_or(0, |u| u.max_kib)) >> 10;
    let err = std::fs::read(&r.err_file).unwrap_or_default();
    let err = String::from_utf8_lossy(&err);
    let tail = stderr_tail(&err);
    let code = st.and_then(|s| s.code());
    let signalled = exit_text(st, r.rusage_file.as_deref()).starts_with("signal");
    let png = r
        .err_file
        .parent()
        .map(|d| d.join(format!("thumb-{}.png", slot_of(&r.err_file))));
    let wrote_png = png
        .and_then(|p| std::fs::read(p).ok())
        .is_some_and(|b| b.starts_with(b"\x89PNG\r\n\x1a\n"));
    let (status, msg) = if over_mem {
        ("oom", format!("over {} MiB resident", r.peak_kib >> 10))
    } else if over_time {
        ("timeout", format!("over {:?}", o.timeout))
    } else if signalled {
        (
            "crash",
            format!("{}; {tail}", exit_text(st, r.rusage_file.as_deref())),
        )
    } else if code == Some(0) && wrote_png {
        ("ok", String::new())
    } else if code == Some(0) {
        ("error", "exit 0 without a PNG".to_owned())
    } else if code == Some(101) || tail.contains("panicked") {
        ("panic", tail)
    } else if tail.contains("gave up after") {
        ("timeout", tail)
    } else {
        ("error", tail)
    };
    Thumb {
        status,
        ms,
        peak_mib,
        msg,
    }
}

/// The slot number in `stderr-N.txt`.
fn slot_of(err_file: &Path) -> String {
    err_file
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("stderr-"))
        .unwrap_or("0")
        .to_owned()
}

#[derive(Default)]
struct Tally {
    files: u64,
    ok: u64,
    error: u64,
    panic: u64,
    crash: u64,
    timeout: u64,
    oom: u64,
    unrecognised: u64,
    /// Desktop thumbnails that panicked, crashed, timed out or ran out of memory.
    thumb_bad: u64,
    slowest: (u128, String),
    outliers: Vec<(u64, String)>,
}

fn summarise(o: &Opts, rows: &[Row], took: Duration) {
    let mut by: BTreeMap<String, Tally> = BTreeMap::new();
    let mut all = Tally::default();
    for r in rows {
        for t in [by.entry(r.format.clone()).or_default(), &mut all] {
            t.files += 1;
            match r.status {
                "ok" => t.ok += 1,
                "error" => t.error += 1,
                "panic" => t.panic += 1,
                "timeout" => t.timeout += 1,
                "oom" => t.oom += 1,
                "unrecognised" => t.unrecognised += 1,
                _ => t.crash += 1,
            }
            if r.thumb
                .as_ref()
                .is_some_and(|t| !matches!(t.status, "ok" | "error"))
            {
                t.thumb_bad += 1;
            }
            if r.ms > t.slowest.0 {
                t.slowest = (r.ms, r.path.display().to_string());
            }
            let peak = r.peak_mib.max(r.thumb.as_ref().map_or(0, |t| t.peak_mib));
            if peak >= OUTLIER_MIB {
                t.outliers.push((peak, r.path.display().to_string()));
            }
        }
    }
    let rate = |t: &Tally| {
        let n = t.files - t.unrecognised;
        if n == 0 {
            1.0
        } else {
            (t.ok + t.error) as f64 / n as f64
        }
    };
    let mut tsv = String::from(
        "format\tfiles\tok\terror\tpanic\tcrash\ttimeout\toom\tunrecognised\tclean_rate\tslowest_ms\tmem_outliers\tthumb_bad\n",
    );
    let mut json = format!(
        "{{\n  \"took_s\": {:.1},\n  \"timeout_s\": {},\n  \"roots\": [{}],\n  \"formats\": {{\n",
        took.as_secs_f64(),
        o.timeout.as_secs_f64(),
        o.roots
            .iter()
            .map(|r| jstr(&r.display().to_string()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut list: Vec<(String, &Tally)> = by.iter().map(|(k, t)| (k.clone(), t)).collect();
    list.push(("TOTAL".to_owned(), &all));
    for (i, (name, t)) in list.iter().enumerate() {
        tsv.push_str(&format!(
            "{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.4}\t{}\t{}\t{}\n",
            t.files,
            t.ok,
            t.error,
            t.panic,
            t.crash,
            t.timeout,
            t.oom,
            t.unrecognised,
            rate(t),
            t.slowest.0,
            t.outliers.len(),
            t.thumb_bad
        ));
        let mut outl = t.outliers.clone();
        outl.sort_by_key(|o| std::cmp::Reverse(o.0));
        outl.truncate(5);
        json.push_str(&format!(
            "    {}: {{\"files\": {}, \"ok\": {}, \"error\": {}, \"panic\": {}, \"crash\": {}, \"timeout\": {}, \"oom\": {}, \"unrecognised\": {}, \"clean_rate\": {:.4}, \"thumb_bad\": {}, \"slowest_ms\": {}, \"slowest\": {}, \"mem_outliers\": [{}]}}{}\n",
            jstr(name),
            t.files,
            t.ok,
            t.error,
            t.panic,
            t.crash,
            t.timeout,
            t.oom,
            t.unrecognised,
            rate(t),
            t.thumb_bad,
            t.slowest.0,
            jstr(&t.slowest.1),
            outl.iter()
                .map(|(m, p)| format!("{{\"mib\": {m}, \"path\": {}}}", jstr(p)))
                .collect::<Vec<_>>()
                .join(", "),
            if i + 1 < list.len() { "," } else { "" }
        ));
    }
    json.push_str("  }\n}\n");
    std::fs::write(o.out.join("summary.tsv"), &tsv).expect("summary.tsv");
    std::fs::write(o.out.join("summary.json"), &json).expect("summary.json");
    // The slowest files and the largest peaks, for the round's report.
    let mut tops = String::from("rank\tby\tms\tcpu_ms\tpeak_mib\tformat\tstatus\tpath\tstep_ms\n");
    let mut by_ms: Vec<&Row> = rows.iter().collect();
    by_ms.sort_by_key(|r| std::cmp::Reverse(r.ms));
    let mut by_mem: Vec<&Row> = rows.iter().collect();
    by_mem.sort_by_key(|r| std::cmp::Reverse(r.peak_mib));
    for (by, list) in [("time", &by_ms), ("memory", &by_mem)] {
        for (i, r) in list.iter().take(TOP).enumerate() {
            tops.push_str(&format!(
                "{}\t{by}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                i + 1,
                r.ms,
                r.cpu_ms,
                r.peak_mib,
                r.format,
                r.status,
                r.path.display(),
                r.step_ms
            ));
        }
    }
    std::fs::write(o.out.join("top.tsv"), &tops).expect("top.tsv");
    print!("{tsv}");
    eprintln!(
        "shakedown: {} files in {:.0} s; results in {}",
        rows.len(),
        took.as_secs_f64(),
        o.out.display()
    );
}

/// Rows listed in `top.tsv` (slowest files and largest peaks).
const TOP: usize = 30;

/// One file of a previous run's `files.tsv`.
struct Prev {
    status: String,
    ms: u128,
    peak_mib: u64,
    thumb: String,
}

/// Compares this run with a previous one (its folder or its `files.tsv`): status counts on
/// both sides, files whose status changed, and files at least twice as slow (and a second
/// slower) or twice as large (and 256 MiB larger). Writes `compare.tsv` and prints the counts.
fn compare(o: &Opts, prev: &Path, rows: &[Row]) {
    let file = if prev.is_dir() {
        prev.join("files.tsv")
    } else {
        prev.to_path_buf()
    };
    let Ok(text) = std::fs::read_to_string(&file) else {
        eprintln!("shakedown: no previous results at {}", file.display());
        return;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (Some(cp), Some(cs), Some(cm), Some(cr)) =
        (col("path"), col("status"), col("ms"), col("peak_mib"))
    else {
        eprintln!("shakedown: {} is not a files.tsv", file.display());
        return;
    };
    let ct = col("thumb_status");
    let mut before: BTreeMap<String, Prev> = BTreeMap::new();
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or_default();
        before.insert(
            get(cp).to_owned(),
            Prev {
                status: get(cs).to_owned(),
                ms: get(cm).parse().unwrap_or(0),
                peak_mib: get(cr).parse().unwrap_or(0),
                thumb: ct.map(get).unwrap_or("-").to_owned(),
            },
        );
    }
    let mut counts: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for p in before.values() {
        counts.entry(p.status.clone()).or_default().0 += 1;
    }
    for r in rows {
        counts.entry(r.status.to_owned()).or_default().1 += 1;
    }
    let mut out = String::from("change\tpath\tbefore\tafter\tdetail\n");
    let (mut changed, mut slower, mut faster, mut larger, mut new, mut thumb) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut seen = std::collections::BTreeSet::new();
    for r in rows {
        let path = r.path.display().to_string();
        let Some(p) = before.get(&path) else {
            new += 1;
            out.push_str(&format!("new\t{path}\t\t{}\t\n", r.status));
            continue;
        };
        seen.insert(path.clone());
        if p.status != r.status {
            changed += 1;
            out.push_str(&format!(
                "status\t{path}\t{}\t{}\t{}\n",
                p.status, r.status, r.msg
            ));
        }
        if let Some(t) = &r.thumb
            && p.thumb != "-"
            && p.thumb != t.status
        {
            thumb += 1;
            out.push_str(&format!(
                "thumb\t{path}\t{}\t{}\t{}\n",
                p.thumb, t.status, t.msg
            ));
        }
        if r.ms >= p.ms * 2 && r.ms >= p.ms + 1000 {
            slower += 1;
            out.push_str(&format!("slower\t{path}\t{}\t{}\tms\n", p.ms, r.ms));
        } else if p.ms >= r.ms * 2 && p.ms >= r.ms + 1000 {
            faster += 1;
            out.push_str(&format!("faster\t{path}\t{}\t{}\tms\n", p.ms, r.ms));
        }
        if r.peak_mib >= p.peak_mib * 2 && r.peak_mib >= p.peak_mib + 256 {
            larger += 1;
            out.push_str(&format!(
                "larger\t{path}\t{}\t{}\tMiB\n",
                p.peak_mib, r.peak_mib
            ));
        }
    }
    let gone = before.keys().filter(|k| !seen.contains(*k)).count();
    std::fs::write(o.out.join("compare.tsv"), &out).expect("compare.tsv");
    eprintln!("shakedown: compared with {}", file.display());
    eprintln!("  {:<14}{:>8}{:>8}", "status", "before", "after");
    for (s, (b, a)) in &counts {
        eprintln!("  {s:<14}{b:>8}{a:>8}");
    }
    eprintln!(
        "  {changed} changed status, {thumb} changed thumbnail status, {slower} slower, {faster} faster, {larger} larger peak, {new} new, {gone} gone; see compare.tsv"
    );
}

fn jstr(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
