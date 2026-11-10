//! Decode-everything check shared by `examples/robust.rs` and `tests/robust_corpus.rs`: every
//! public entry point at all four scales, with regions and with an index, on every file of a
//! directory. Reports panics, files over the per-file time limit, hangs and the peak RSS.

#![allow(dead_code)]
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use specular_jpeg::*;

const SCALES: [Scale; 4] = [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth];

/// What happened to one file.
#[derive(Default)]
pub struct FileResult {
    /// Decodes that returned an image / an error.
    pub ok: usize,
    pub err: usize,
    /// Images flagged as truncated.
    pub truncated: usize,
}

/// Every decode of one file (panics propagate to the caller).
pub fn check_bytes(data: &[u8]) -> FileResult {
    let mut r = FileResult::default();
    let mut note = |d: &Result<DecodedPlanes>| match d {
        Ok(d) => {
            r.ok += 1;
            r.truncated += d.truncated as usize;
            let rgba = planes_to_rgba(d);
            assert_eq!(rgba.len(), d.width as usize * d.height as usize * 4);
        }
        Err(_) => r.err += 1,
    };
    let info = read_info(data);
    for scale in SCALES {
        for parallelism in [Parallelism::Single, Parallelism::Threads(3)] {
            let o = DecodeOptions {
                scale,
                parallelism,
                ..Default::default()
            };
            note(&decode(data, &o));
        }
    }
    let Ok(info) = info else {
        return r;
    };
    let _ = choose_scale(&info, 300, 200);
    let _ = memory_estimate(&info, Scale::Full, None);
    let ix = build_index(data);
    for scale in SCALES {
        let f = scale.factor();
        let (w, h) = (info.width.div_ceil(f), info.height.div_ceil(f));
        let region = Region {
            x: w / 3,
            y: h / 4,
            w: (w / 2).max(1),
            h: (h / 3).max(1),
        };
        let o = DecodeOptions {
            scale,
            region: Some(region),
            parallelism: Parallelism::Threads(3),
            ..Default::default()
        };
        let plain = decode(data, &o);
        note(&plain);
        if let Ok(ix) = &ix {
            let with = decode_with_index(data, ix, &o);
            note(&with);
            if let (Ok(a), Ok(b)) = (&plain, &with)
                && !a.truncated
                && !b.truncated
            {
                assert!(a == b, "indexed region decode differs at {scale:?}");
            }
        }
    }
    let _ = decode_and_index(
        data,
        &DecodeOptions {
            scale: Scale::Quarter,
            ..Default::default()
        },
    );
    r
}

#[derive(Default)]
pub struct Report {
    pub files: usize,
    pub decodes_ok: usize,
    pub decodes_err: usize,
    pub truncated: usize,
    /// (file, panic message)
    pub panics: Vec<(PathBuf, String)>,
    /// Files over the per-file time limit (file, seconds).
    pub slow: Vec<(PathBuf, f64)>,
    /// Files still running after the hang limit (the check stops waiting for them).
    pub hangs: Vec<PathBuf>,
    pub slowest: (f64, PathBuf),
    /// Peak resident set size of the process during the run (bytes; 0 if unknown).
    pub peak_rss: u64,
    pub elapsed: Duration,
}

impl Report {
    pub fn failed(&self, rss_limit: u64) -> bool {
        !self.panics.is_empty()
            || !self.slow.is_empty()
            || !self.hangs.is_empty()
            || self.peak_rss > rss_limit
    }

    pub fn summary(&self) -> String {
        let mut s = format!(
            "{} files, {} decodes ok ({} truncated), {} errors, {} panics, {} over the time limit, \
             {} hangs; slowest {:.0} ms ({}); peak RSS {} MiB; {:.1} s",
            self.files,
            self.decodes_ok,
            self.truncated,
            self.decodes_err,
            self.panics.len(),
            self.slow.len(),
            self.hangs.len(),
            self.slowest.0 * 1e3,
            self.slowest.1.display(),
            self.peak_rss >> 20,
            self.elapsed.as_secs_f64()
        );
        for (p, m) in &self.panics {
            s += &format!("\n  panic {}: {m}", p.display());
        }
        for (p, t) in &self.slow {
            s += &format!("\n  slow {}: {t:.2} s", p.display());
        }
        for p in &self.hangs {
            s += &format!("\n  hang {}", p.display());
        }
        s
    }
}

/// Peak RSS (VmHWM) of this process in bytes.
pub fn peak_rss() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb << 10)
}

/// Files of `dir` (sorted, `.jpg` / `.jpeg`).
pub fn list(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg")
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

type Running = Option<(usize, Instant)>;

/// Checks every file on `workers` threads. A file over `limit` counts as slow; one still
/// running after `hang` is reported as a hang and the check returns without it.
pub fn check_all(files: &[PathBuf], workers: usize, limit: Duration, hang: Duration) -> Report {
    let t0 = Instant::now();
    let files: Arc<Vec<PathBuf>> = Arc::new(files.to_vec());
    let next = Arc::new(AtomicUsize::new(0));
    let report = Arc::new(Mutex::new(Report::default()));
    // Per worker: the file it is on and since when.
    let running: Arc<Mutex<Vec<Running>>> = Arc::new(Mutex::new(vec![None; workers.max(1)]));
    // Panics are expected to be caught and reported; keep the default hook quiet.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut handles = Vec::new();
    for w in 0..workers.max(1) {
        let (files, next, report, running) =
            (files.clone(), next.clone(), report.clone(), running.clone());
        handles.push(std::thread::spawn(move || {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = files.get(i) else { break };
                running.lock().unwrap()[w] = Some((i, Instant::now()));
                let Ok(data) = std::fs::read(path) else {
                    continue;
                };
                let t = Instant::now();
                let res = std::panic::catch_unwind(|| check_bytes(&data));
                let dt = t.elapsed();
                running.lock().unwrap()[w] = None;
                let mut r = report.lock().unwrap();
                r.files += 1;
                match res {
                    Ok(f) => {
                        r.decodes_ok += f.ok;
                        r.decodes_err += f.err;
                        r.truncated += f.truncated;
                    }
                    Err(e) => {
                        let msg = e
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_default();
                        r.panics.push((path.clone(), msg));
                    }
                }
                if dt > limit {
                    r.slow.push((path.clone(), dt.as_secs_f64()));
                }
                if dt.as_secs_f64() > r.slowest.0 {
                    r.slowest = (dt.as_secs_f64(), path.clone());
                }
            }
        }));
    }
    let mut hung = Vec::new();
    while !handles.iter().all(|h| h.is_finished()) {
        std::thread::sleep(Duration::from_millis(50));
        hung = running
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .filter(|(_, s)| s.elapsed() > hang)
            .map(|&(i, _)| files[i].clone())
            .collect();
        if !hung.is_empty() {
            break;
        }
    }
    std::panic::set_hook(hook);
    let finished = hung.is_empty();
    if finished {
        for h in handles {
            let _ = h.join();
        }
    }
    let mut r = std::mem::take(&mut *report.lock().unwrap());
    r.hangs = hung;
    r.peak_rss = peak_rss();
    r.elapsed = t0.elapsed();
    r
}
