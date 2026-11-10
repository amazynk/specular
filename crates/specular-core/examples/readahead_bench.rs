//! Cold-cache neighbour prefetch: the decoder's page faults alone (the old path) against
//! read-ahead with `posix_fadvise` and with io_uring (`specular_platform::readahead`).
//!
//! ```text
//! readahead_bench [DIR] [--files N] [--mb M] [--rounds R] [--threads T] [--cpu MS_PER_MB]
//! ```
//!
//! Writes N files of M MiB of incompressible data to DIR (default
//! `$CARGO_TARGET_DIR/readahead-bench`), then per round and backend: drops those files (and only
//! those) from the page cache with `posix_fadvise(DONTNEED)`, starts the read-ahead of all of
//! them, and has T "decode" threads open each file through `Source::open` (the viewer's memory
//! map), touch every page and spend MS_PER_MB of CPU per MiB, as prefetch decodes would. Prints
//! the median wall time per backend. Linux only.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use specular_platform::readahead::{self, Backend};

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir = None;
    let (mut files, mut mb, mut rounds, mut threads, mut cpu) =
        (8usize, 16usize, 5usize, 2usize, 0.0f64);
    while let Some(a) = args.next() {
        let mut num = || {
            args.next()
                .and_then(|v| v.parse::<f64>().ok())
                .expect("number")
        };
        match a.as_str() {
            "--files" => files = num() as usize,
            "--mb" => mb = num() as usize,
            "--rounds" => rounds = num() as usize,
            "--threads" => threads = num() as usize,
            "--cpu" => cpu = num(),
            _ => dir = Some(PathBuf::from(a)),
        }
    }
    let dir = dir.unwrap_or_else(|| {
        PathBuf::from(std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into()))
            .join("readahead-bench")
    });
    let paths = make_files(&dir, files, mb);
    println!(
        "{files} files × {mb} MiB, {threads} decode threads, {cpu} ms CPU per MiB, {rounds} rounds; io_uring available: {}",
        readahead::uring_available()
    );
    let backends = [Backend::Off, Backend::Fadvise, Backend::IoUring];
    let mut times: Vec<Vec<f64>> = vec![Vec::new(); backends.len()];
    let mut first: Vec<Vec<f64>> = vec![Vec::new(); backends.len()];
    for _ in 0..rounds {
        // Interleave the backends so drift in the machine's load hits all of them alike.
        for (i, &b) in backends.iter().enumerate() {
            let (total, first_ms) = round(&paths, b, threads, cpu);
            times[i].push(total);
            first[i].push(first_ms);
        }
    }
    let total_mb = (files * mb) as f64;
    for (i, b) in backends.iter().enumerate() {
        let t = median(&mut times[i]);
        let f = median(&mut first[i]);
        println!(
            "{:<8} median {t:7.1} ms ({:6.0} MiB/s), first file done {f:6.1} ms, all: {:?}",
            format!("{b:?}"),
            total_mb / (t / 1e3),
            times[i]
                .iter()
                .map(|t| t.round() as i64)
                .collect::<Vec<_>>()
        );
    }
}

fn make_files(dir: &Path, n: usize, mb: usize) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).unwrap();
    (0..n)
        .map(|i| {
            let p = dir.join(format!("f{i:02}-{mb}m.bin"));
            if std::fs::metadata(&p).map(|m| m.len()).ok() != Some((mb << 20) as u64) {
                let mut f = File::create(&p).unwrap();
                let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ i as u64;
                let mut block = vec![0u8; 1 << 20];
                for _ in 0..mb {
                    for c in block.chunks_exact_mut(8) {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        c.copy_from_slice(&x.to_le_bytes());
                    }
                    f.write_all(&block).unwrap();
                }
                f.sync_all().unwrap();
            }
            p
        })
        .collect()
}

/// One cold round: returns the wall time until every file was consumed, and until the first
/// (most wanted) one was.
fn round(paths: &[PathBuf], backend: Backend, threads: usize, cpu: f64) -> (f64, f64) {
    for p in paths {
        let f = File::open(p).unwrap();
        assert!(readahead::evict(&f), "posix_fadvise(DONTNEED) failed");
    }
    // Let the device settle after eviction.
    std::thread::sleep(Duration::from_millis(200));
    let t0 = Instant::now();
    let warm = (backend != Backend::Off).then(|| {
        let paths = paths.to_vec();
        std::thread::spawn(move || {
            let files: Vec<File> = paths.iter().map(|p| File::open(p).unwrap()).collect();
            readahead::warm_blocking(&files, backend).unwrap()
        })
    });
    let next = AtomicUsize::new(0);
    let first_done = std::sync::Mutex::new(None::<f64>);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = paths.get(i) else { break };
                    consume(p, cpu);
                    if i == 0 {
                        *first_done.lock().unwrap() = Some(t0.elapsed().as_secs_f64() * 1e3);
                    }
                }
            });
        }
    });
    let total = t0.elapsed().as_secs_f64() * 1e3;
    if let Some(w) = warm {
        w.join().unwrap();
    }
    (total, first_done.into_inner().unwrap().unwrap_or(total))
}

/// What a decode does to the file: map it, read every page in order, and compute.
fn consume(p: &Path, cpu_ms_per_mb: f64) {
    let src = specular_core::Source::open(p).unwrap();
    let bytes = src.bytes();
    let mut sum = 0u64;
    let per_mb = Duration::from_secs_f64(cpu_ms_per_mb / 1e3);
    for mb in bytes.chunks(1 << 20) {
        for page in mb.chunks(4096) {
            sum = sum.wrapping_add(u64::from(page[0]));
        }
        if !per_mb.is_zero() {
            let t = Instant::now();
            while t.elapsed() < per_mb {
                sum = std::hint::black_box(sum.wrapping_mul(31).wrapping_add(7));
            }
        }
    }
    std::hint::black_box(sum);
}

fn median(v: &mut [f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}
