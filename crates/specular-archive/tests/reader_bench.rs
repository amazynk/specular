//! Reading-mode latency of solid 7z archives (ignored; needs real archives):
//!
//! ```text
//! SPECULAR_7Z_BENCH=a.7z,b.7z SPECULAR_7Z_THREADS=1,8 SPECULAR_7Z_ROUNDS=3 SPECULAR_7Z_AHEAD=64 \
//!   cargo test --release -p specular-archive --test reader_bench -- --ignored --nocapture
//! ```
//!
//! Per archive and thread count, interleaved round by round: time to open, to the first page (open
//! and read), a page turn right after the last (next), a turn after a pause (the read-ahead
//! had time), a jump to the middle, the pages before it (previous), 120 pages read on from
//! there back to back, and a jump back near the start; and the CPU time the process spent.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::time::{Duration, Instant};

use specular_archive::{Archive, Options};

fn cpu_ms() -> f64 {
    // utime + stime of the whole process, from /proc (Linux only; 0 elsewhere).
    let Ok(s) = std::fs::read_to_string("/proc/self/stat") else {
        return 0.0;
    };
    let Some(rest) = s.rsplit(')').next() else {
        return 0.0;
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    let t = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    // Fields 14 and 15 (1-based) are the 12th and 13th after the state.
    (t(11) + t(12)) * 10.0
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn run(path: &str, threads: u32) -> Vec<(&'static str, f64)> {
    let cpu0 = cpu_ms();
    let t = Instant::now();
    let mut opts = Options {
        decode_threads: threads,
        ..Options::default()
    };
    if let Some(mib) = std::env::var("SPECULAR_7Z_AHEAD")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
    {
        opts.solid_read_ahead = mib << 20;
    }
    let a = Archive::open_with(path, opts).unwrap();
    let open = ms(t);
    let pages = a.pages(|e| e.name.ends_with(".jpg"));
    let n = pages.len();
    let read = |p: usize| {
        let t = Instant::now();
        let v = a.read(pages[p]).unwrap();
        assert_eq!(v.len() as u64, a.entries()[pages[p]].size);
        ms(t)
    };
    read(0);
    let first = ms(t);
    let next: Vec<f64> = (1..=8).map(read).collect();
    std::thread::sleep(Duration::from_millis(300));
    let paused = read(9);
    let mid = n / 2;
    let jump = read(mid);
    let after = read(mid + 1);
    let prev: Vec<f64> = (1..=4).map(|d| read(mid - d)).collect();
    // Flipping through as fast as pages come (a held arrow key, a scrubbed slider).
    let t = Instant::now();
    for p in mid + 2..(mid + 122).min(n) {
        read(p);
    }
    let flip = ms(t);
    let back = read(5);
    let med = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let worst = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
    vec![
        ("open", open),
        ("first", first),
        ("next med", med(next.clone())),
        ("next max", worst(&next)),
        ("paused", paused),
        ("jump mid", jump),
        ("after jump", after),
        ("prev med", med(prev.clone())),
        ("prev max", worst(&prev)),
        ("flip 120", flip),
        ("back to 5", back),
        ("cpu", cpu_ms() - cpu0),
    ]
}

#[test]
#[ignore]
fn solid_7z_reader_latency() {
    let Ok(paths) = std::env::var("SPECULAR_7Z_BENCH") else {
        eprintln!("SPECULAR_7Z_BENCH not set");
        return;
    };
    let threads: Vec<u32> = std::env::var("SPECULAR_7Z_THREADS")
        .unwrap_or_else(|_| "1,8".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let rounds: usize = std::env::var("SPECULAR_7Z_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    for path in paths.split(',') {
        let mut all: Vec<Vec<Vec<(&str, f64)>>> = vec![Vec::new(); threads.len()];
        for _ in 0..rounds {
            for (k, &t) in threads.iter().enumerate() {
                all[k].push(run(path, t));
            }
        }
        for (k, &t) in threads.iter().enumerate() {
            let mut line = format!("{path} threads {t}:");
            for j in 0..all[k][0].len() {
                let mut v: Vec<f64> = all[k].iter().map(|r| r[j].1).collect();
                v.sort_by(f64::total_cmp);
                line += &format!(" {} {:.0}", all[k][0][j].0, v[v.len() / 2]);
            }
            println!("{line} (ms, median of {rounds})");
        }
    }
}

/// The gallery's view of a solid 7z (thumbnails of every page): `$SPECULAR_7Z_READERS`
/// readers (default 16) take the pages in order, each reading the next one not yet taken,
/// with the thumbnail opener's options (a 16 MiB cache). Time to read them all, and CPU.
#[test]
#[ignore]
fn solid_7z_all_pages() {
    let Ok(paths) = std::env::var("SPECULAR_7Z_BENCH") else {
        eprintln!("SPECULAR_7Z_BENCH not set");
        return;
    };
    let threads: u32 = std::env::var("SPECULAR_7Z_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let readers: usize = std::env::var("SPECULAR_7Z_READERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);
    for path in paths.split(',') {
        let cpu0 = cpu_ms();
        let t = Instant::now();
        let opts = Options {
            decode_threads: threads,
            solid_cache_bytes: 16 << 20,
            ..Options::default()
        };
        let a = Archive::open_with(path, opts).unwrap();
        let pages = a.pages(|e| e.name.ends_with(".jpg"));
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..readers {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&p) = pages.get(i) else { break };
                        a.read(p).unwrap();
                    }
                });
            }
        });
        println!(
            "{path} all {} pages, threads {threads}, readers {readers}: {:.0} ms, cpu {:.0} ms",
            pages.len(),
            ms(t),
            cpu_ms() - cpu0
        );
    }
}

/// The thumbnail engine's view of a solid 7z: one reader takes the pages in batches of 48
/// through `read_many` and hands each to `$SPECULAR_7Z_READERS` workers (default 16) that
/// spend `$SPECULAR_7Z_WORK_US` (default 4000) on it, making one itself when more are queued
/// than there are workers. Time to make them all, and CPU.
#[test]
#[ignore]
fn solid_7z_batches() {
    use std::collections::VecDeque;
    use std::sync::{Condvar, Mutex};
    let Ok(paths) = std::env::var("SPECULAR_7Z_BENCH") else {
        eprintln!("SPECULAR_7Z_BENCH not set");
        return;
    };
    let env = |k: &str, d: u64| {
        std::env::var(k)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(d)
    };
    let threads = env("SPECULAR_7Z_THREADS", 8) as u32;
    let workers = env("SPECULAR_7Z_READERS", 16) as usize;
    let work = Duration::from_micros(env("SPECULAR_7Z_WORK_US", 4000));
    let spin = |v: &[u8]| {
        let t = Instant::now();
        let mut x = 0u64;
        while t.elapsed() < work {
            for b in v.iter().step_by(4096) {
                x = x.wrapping_mul(31).wrapping_add(*b as u64);
            }
        }
        std::hint::black_box(x);
    };
    for path in paths.split(',') {
        let cpu0 = cpu_ms();
        let t = Instant::now();
        let opts = Options {
            decode_threads: threads,
            solid_cache_bytes: 16 << 20,
            ..Options::default()
        };
        let a = Archive::open_with(path, opts).unwrap();
        let pages = a.pages(|e| e.name.ends_with(".jpg"));
        let q: Mutex<(VecDeque<Vec<u8>>, bool)> = Mutex::new((VecDeque::new(), false));
        let cv = Condvar::new();
        std::thread::scope(|s| {
            for _ in 0..workers.saturating_sub(1) {
                s.spawn(|| {
                    loop {
                        let mut g = q.lock().unwrap();
                        let v = loop {
                            if let Some(v) = g.0.pop_front() {
                                break Some(v);
                            }
                            if g.1 {
                                break None;
                            }
                            g = cv.wait(g).unwrap();
                        };
                        drop(g);
                        let Some(v) = v else { return };
                        spin(&v);
                    }
                });
            }
            for chunk in pages.chunks(48) {
                a.read_many(chunk, &mut |_, r| {
                    let v = r.unwrap();
                    let mut g = q.lock().unwrap();
                    g.0.push_back(v);
                    let own = if g.0.len() > workers {
                        g.0.pop_front()
                    } else {
                        None
                    };
                    drop(g);
                    cv.notify_one();
                    if let Some(v) = own {
                        spin(&v);
                    }
                    true
                });
            }
            q.lock().unwrap().1 = true;
            cv.notify_all();
        });
        println!(
            "{path} batches {} pages, threads {threads}, workers {workers}: {:.0} ms, cpu {:.0} ms",
            pages.len(),
            ms(t),
            cpu_ms() - cpu0
        );
    }
}
