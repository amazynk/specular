//! Heavy-JPEG viewing benchmark through the registry, shaped like the app's loader: a decode
//! thread outside rayon (like the job pool), the 1/8 overview and the sharp fit level started
//! together, the row index, then 1:1 detail tiles at the centre (512-px tiles with an 8-px
//! halo, one job per tile row, run concurrently) and a pan to the right in quarter-viewport
//! steps (only newly exposed tile columns are decoded). Prints times and peak memory.
//!
//! `cargo run --release -p specular-decode --example bench-heavy -- <file> [WxH] [runs]`
//! Run one file per process: peak RSS is per process. Times are medians over `runs`
//! (default 3; each run opens a fresh session, so the index is rebuilt).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Rect, Registry, Session};

const TILE: u32 = 512;
const HALO: u32 = 8;

/// Same rule as the app's loader: the coarsest scale (8, 4, 2, 1) still covering the viewport.
fn needed_scale(w: u32, h: u32, vw: u32, vh: u32) -> u32 {
    let fit = (vw as f32 / w as f32).min(vh as f32 / h as f32).min(1.0);
    for s in [8u32, 4, 2] {
        if 1.0 / s as f32 >= fit * 0.999 {
            return s;
        }
    }
    1
}

fn status_kb(key: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Decodes the tiles of columns `c0..c1`, rows `r0..r1` at full resolution: one region per
/// tile row (with halo), all rows concurrently on their own threads. Returns the wall time.
fn tiles(s: &Arc<dyn Session>, (w, h): (u32, u32), c0: u32, c1: u32, r0: u32, r1: u32) -> f64 {
    let t = Instant::now();
    std::thread::scope(|sc| {
        for row in r0..r1 {
            let s = s.clone();
            sc.spawn(move || {
                let x0 = (c0 * TILE).saturating_sub(HALO);
                let y0 = (row * TILE).saturating_sub(HALO);
                let x1 = (c1 * TILE + HALO).min(w);
                let y1 = ((row + 1) * TILE + HALO).min(h);
                let req = DecodeRequest {
                    region: Some(Rect {
                        x: x0,
                        y: y0,
                        w: x1 - x0,
                        h: y1 - y0,
                    }),
                    ..DecodeRequest::full(1)
                };
                std::hint::black_box(s.decode(&req, &CancelToken::new()).expect("tiles"));
            });
        }
    });
    ms(t)
}

#[derive(Default)]
struct Run {
    first: f64,
    sharp: f64,
    l8: f64,
    lsharp: f64,
    index: f64,
    centre: f64,
    pans: Vec<f64>,
}

fn one_run(path: &str, vw: u32, vh: u32, verbose: bool) -> Run {
    let t0 = Instant::now();
    let src = Arc::new(Source::open(std::path::Path::new(path)).expect("open file"));
    let s: Arc<dyn Session> = Registry::default().open(src).expect("decoder open");
    let i = s.info().clone();
    let (w, h) = (i.width, i.height);
    let sharp = needed_scale(w, h, vw, vh);
    if verbose {
        println!(
            "  {}x{} {} | viewport {vw}x{vh} -> sharp 1/{sharp} | open_memory([8,{sharp}]) {} MiB",
            w,
            h,
            i.coding,
            s.open_memory(&[8, sharp]) >> 20
        );
    }
    let mut r = Run::default();
    // The 1/8 overview and the sharp level start together, on threads outside rayon.
    let level = |scale: u32| {
        let s = s.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            let req = DecodeRequest {
                prepare_regions: true,
                ..DecodeRequest::full(scale)
            };
            let out = s.decode(&req, &CancelToken::new()).expect("level");
            (ms(t), out.width, out.height)
        })
    };
    let a = level(8);
    let b = (sharp != 8).then(|| level(sharp));
    let (l8, ..) = a.join().unwrap();
    r.l8 = l8;
    r.first = ms(t0);
    if let Some(b) = b {
        let (ls, ..) = b.join().unwrap();
        r.lsharp = ls;
    } else {
        r.lsharp = l8;
    }
    r.sharp = ms(t0);
    if !s.regions_ready() {
        let t = Instant::now();
        s.prepare_regions(&CancelToken::new()).expect("index");
        r.index = ms(t);
    }
    // 1:1 at the centre: the tiles under a viewport-sized window.
    let (cx, cy) = (w / 2, h / 2);
    let x0 = cx.saturating_sub(vw / 2);
    let y0 = cy.saturating_sub(vh / 2);
    let (c0, c1) = (x0 / TILE, ((x0 + vw).min(w)).div_ceil(TILE));
    let (r0, r1) = (y0 / TILE, ((y0 + vh).min(h)).div_ceil(TILE));
    r.centre = tiles(&s, (w, h), c0, c1, r0, r1);
    // Pan right by a quarter viewport at a time: decode the newly exposed tile columns.
    let mut right = c1;
    let mut x = x0;
    let tiles_x = w.div_ceil(TILE);
    for _ in 0..8 {
        x += vw / 4;
        let need = ((x + vw).min(w)).div_ceil(TILE);
        if need > right {
            r.pans
                .push(tiles(&s, (w, h), right, need.min(tiles_x), r0, r1));
            right = need;
        }
    }
    r
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: bench-heavy <file> [WxH] [runs]");
    let (vw, vh) = args
        .next()
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((1920u32, 1080u32));
    let runs: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);

    // Peak anonymous memory (decode buffers), sampled; VmHWM also counts mapped file pages.
    let stop = Arc::new(AtomicBool::new(false));
    let peak_anon = Arc::new(AtomicU64::new(0));
    let sampler = {
        let (stop, peak) = (stop.clone(), peak_anon.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                peak.fetch_max(status_kb("RssAnon:"), Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };
    println!("{path}");
    let rs: Vec<Run> = (0..runs.max(1))
        .map(|k| one_run(&path, vw, vh, k == 0))
        .collect();
    let m = |f: &dyn Fn(&Run) -> f64| median(rs.iter().map(f).collect());
    let pans: Vec<f64> = rs.iter().flat_map(|r| r.pans.iter().copied()).collect();
    let pan_max = pans.iter().copied().fold(0.0, f64::max);
    println!(
        "  first pixels (1/8)   {:7.1} ms  (decode {:.1})",
        m(&|r| r.first),
        m(&|r| r.l8)
    );
    println!(
        "  sharp fit            {:7.1} ms  (decode {:.1}, concurrent with 1/8)",
        m(&|r| r.sharp),
        m(&|r| r.lsharp)
    );
    println!("  row index (extra)    {:7.1} ms", m(&|r| r.index));
    println!("  1:1 centre tiles     {:7.1} ms", m(&|r| r.centre));
    println!(
        "  pan step (1/4 vp)    {:7.1} ms median, {:.1} max ({} steps)",
        median(pans.clone()),
        pan_max,
        pans.len()
    );
    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    println!(
        "  peak RSS {} MiB (anon {} MiB; file-backed mmap pages make up the rest)",
        status_kb("VmHWM:") >> 10,
        peak_anon.load(Ordering::Relaxed) >> 10,
    );
}
