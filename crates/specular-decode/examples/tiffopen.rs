//! Real-file open / first-pixels / sharp-fit timings and peak memory, through the registry like
//! the app's loader (mmap'd `Source`, overview plan 1/8 then the sharp fit level).
//!
//! `cargo run --release -p specular-decode --example tiffopen -- <file> [WxH]`
//! Run one file per process: peak RSS is per process.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Rect, Registry};

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

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: tiffopen <file> [WxH]");
    let (vw, vh) = args
        .next()
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((1920u32, 1080u32));

    // Peak anonymous memory (decode buffers), sampled; VmHWM also counts mapped file pages.
    let stop = Arc::new(AtomicBool::new(false));
    let peak_anon = Arc::new(AtomicU64::new(0));
    let sampler = {
        let (stop, peak) = (stop.clone(), peak_anon.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                peak.fetch_max(status_kb("RssAnon:"), Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(2));
            }
        })
    };

    let t0 = Instant::now();
    let src = Arc::new(Source::open(std::path::Path::new(&path)).expect("open file"));
    let s = Registry::default().open(src).expect("decoder open");
    let open = ms(t0);
    let i = s.info().clone();
    let (dw, dh) = i.display_size();
    let sharp = needed_scale(dw, dh, vw, vh);
    let c = CancelToken::new();
    println!("{path}");
    println!(
        "  {}x{} {} | fast regions {} | viewport {vw}x{vh} -> sharp 1/{sharp}",
        i.width, i.height, i.coding, i.fast_regions
    );
    println!(
        "  estimate open_memory([8,{sharp}]) {} MiB",
        s.open_memory(&[8, sharp]) >> 20
    );
    println!("  open                 {open:8.1} ms");

    let t = Instant::now();
    let first = s
        .decode(
            &DecodeRequest {
                prepare_regions: true,
                ..DecodeRequest::full(8)
            },
            &c,
        )
        .expect("1/8");
    println!(
        "  first pixels (1/8)   {:8.1} ms  (+{:.1} decode) {}x{}",
        ms(t0),
        ms(t),
        first.width,
        first.height
    );
    drop(first);
    if sharp != 8 {
        let t = Instant::now();
        let r = s
            .decode(
                &DecodeRequest {
                    prepare_regions: true,
                    ..DecodeRequest::full(sharp)
                },
                &c,
            )
            .expect("sharp");
        println!(
            "  sharp fit (1/{sharp})     {:8.1} ms  (+{:.1} decode) {}x{}",
            ms(t0),
            ms(t),
            r.width,
            r.height
        );
    } else {
        println!("  sharp fit (1/8)      {:8.1} ms  (same level)", ms(t0));
    }
    println!(
        "  session cache before zoom {} MiB, RSS anon {} MiB",
        s.memory_bytes() >> 20,
        status_kb("RssAnon:") >> 10
    );
    // 100% zoom at the centre: one viewport-sized region at full resolution.
    let (rw, rh) = (vw.min(i.width), vh.min(i.height));
    let r = Rect {
        x: (i.width - rw) / 2,
        y: (i.height - rh) / 2,
        w: rw,
        h: rh,
    };
    let t = Instant::now();
    let reg = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(1)
            },
            &c,
        )
        .expect("region");
    println!(
        "  1:1 centre {rw}x{rh}  {:8.1} ms ({}x{} returned)",
        ms(t),
        reg.width,
        reg.height
    );
    drop(reg);
    let t = Instant::now();
    let r2 = Rect {
        x: 0,
        y: 0,
        w: rw,
        h: rh,
    };
    let _ = s
        .decode(
            &DecodeRequest {
                region: Some(r2),
                ..DecodeRequest::full(1)
            },
            &c,
        )
        .expect("region");
    println!("  1:1 top-left         {:8.1} ms", ms(t));
    // Panning right from the centre in 256-px steps, then back to the centre.
    let mut pans = Vec::new();
    for k in 1..=6u32 {
        let p = Rect {
            x: (r.x + k * 256).min(i.width - rw),
            ..r
        };
        let t = Instant::now();
        let _ = s
            .decode(
                &DecodeRequest {
                    region: Some(p),
                    ..DecodeRequest::full(1)
                },
                &c,
            )
            .expect("pan");
        pans.push(format!("{:.1}", ms(t)));
    }
    println!("  1:1 pan 6x256 px     {} ms", pans.join(" "));
    let t = Instant::now();
    let _ = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(1)
            },
            &c,
        )
        .expect("region");
    println!("  1:1 centre again     {:8.1} ms", ms(t));

    stop.store(true, Ordering::Relaxed);
    let _ = sampler.join();
    println!(
        "  peak RSS {} MiB (anon {} MiB, file-backed mmap pages make up the rest); session cache {} MiB",
        status_kb("VmHWM:") >> 10,
        peak_anon.load(Ordering::Relaxed) >> 10,
        s.memory_bytes() >> 20
    );
}
