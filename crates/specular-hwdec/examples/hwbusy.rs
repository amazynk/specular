//! Hardware JPEG decode under contention:
//! `cargo run --release -p specular-hwdec --example hwbusy -- CURRENT.jpg PREFETCH.jpg`
//!
//! Models the viewer's background prefetch: the current image decodes in software on all
//! threads while a second (baseline) image is prefetched either in software (all threads too)
//! or on the hardware decoder. Prints, best of `RUNS` (default 5): the current image's decode
//! time and the wall time until both are done.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn sw(data: &[u8]) -> bool {
    let o = specular_jpeg::DecodeOptions {
        parallelism: specular_jpeg::Parallelism::Auto,
        ..Default::default()
    };
    specular_jpeg::decode(data, &o).is_ok()
}

/// (current image ms, both done ms).
fn pair(cur: &[u8], pre: &[u8], hw: bool) -> (f64, f64) {
    let t = Instant::now();
    std::thread::scope(|s| {
        let p = s.spawn(|| {
            if hw {
                specular_hwdec::decode_jpeg_unchecked(pre).is_ok()
            } else {
                sw(pre)
            }
        });
        assert!(sw(cur));
        let c = ms(t);
        assert!(p.join().unwrap());
        (c, ms(t))
    })
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    assert!(a.len() == 2, "usage: hwbusy CURRENT.jpg PREFETCH.jpg");
    let cur = std::fs::read(&a[0]).expect("read current");
    let pre = std::fs::read(&a[1]).expect("read prefetch");
    let runs: usize = std::env::var("RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    if let Err(e) = specular_hwdec::decode_jpeg_unchecked(&pre) {
        println!("hardware decode of the prefetch image: {e}");
        return;
    }
    let best = |f: &dyn Fn() -> (f64, f64)| {
        f();
        (0..runs)
            .map(|_| f())
            .fold((f64::INFINITY, f64::INFINITY), |b, x| {
                (b.0.min(x.0), b.1.min(x.1))
            })
    };
    let alone = best(&|| {
        let t = Instant::now();
        assert!(sw(&cur));
        (ms(t), ms(t))
    });
    let s = best(&|| pair(&cur, &pre, false));
    let h = best(&|| pair(&cur, &pre, true));
    println!("current alone (software):            {:6.1} ms", alone.0);
    println!(
        "prefetch in software: current {:6.1} ms, both done {:6.1} ms",
        s.0, s.1
    );
    println!(
        "prefetch on hardware: current {:6.1} ms, both done {:6.1} ms",
        h.0, h.1
    );
}
