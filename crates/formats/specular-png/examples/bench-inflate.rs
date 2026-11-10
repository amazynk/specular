//! Inflate-only throughput: specular-png's inflater (through its decode window) vs `fdeflate`
//! (the `png` crate's inflater, driven the way `png` drives it: a sliding output buffer).
//!
//! `cargo run --release -p specular-png --example bench-inflate -- <file.png>...`
//!
//! Checks that both produce the same bytes (length and Adler-32), then times them round-robin,
//! best of `PNG_BENCH_RUNS` (default 9). Reported: MB of inflated data per second and the
//! per-pixel rate in MP/s.

use std::time::Instant;

/// `fdeflate` over the concatenated IDAT data, keeping a 32 KiB window plus 256 KiB of new output
/// per call (like `png`'s `ZlibStream`). Returns the size and Adler-32 (if `adler`).
fn fdeflate_inflate(zlib: &[u8], adler: bool) -> (usize, u32) {
    const WIN: usize = 32 * 1024;
    const CHUNK: usize = 256 * 1024;
    let mut d = fdeflate::Decompressor::new();
    d.ignore_adler32();
    let mut buf = vec![0u8; WIN + CHUNK];
    let (mut pos, mut inp, mut total, mut a) = (0usize, 0usize, 0usize, 1u32);
    while !d.is_done() {
        if pos == buf.len() {
            buf.copy_within(pos - WIN..pos, 0);
            pos = WIN;
        }
        let (c, p) = d.read(&zlib[inp..], &mut buf, pos, true).expect("fdeflate");
        if adler {
            a = adler32(a, &buf[pos..pos + p]);
        }
        inp += c;
        pos += p;
        total += p;
    }
    (total, a)
}

fn adler32(adler: u32, data: &[u8]) -> u32 {
    let (mut a, mut b) = (adler & 0xffff, adler >> 16);
    for c in data.chunks(5552) {
        for &x in c {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    b << 16 | a
}

fn main() {
    let runs: usize = std::env::var("PNG_BENCH_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    // `PNG_BENCH_PROFILE=ours|fdeflate`: only loop that inflater (for `perf record`).
    let profile = std::env::var("PNG_BENCH_PROFILE").ok();
    println!(
        "{:<34} {:>7} {:>7} | {:>8} {:>8} | {:>6} {:>6} | ratio",
        "image", "MB in", "MB out", "ours MB/s", "fdfl MB/s", "ours MP", "fdfl MP"
    );
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).expect("read");
        let info = specular_png::read_info(&data).expect("info");
        let mut zlib = Vec::new();
        for r in &info.idat {
            zlib.extend_from_slice(&data[r.clone()]);
        }
        if let Some(p) = &profile {
            for _ in 0..8 {
                if p == "fdeflate" {
                    std::hint::black_box(fdeflate_inflate(&zlib, false));
                } else {
                    std::hint::black_box(specular_png::bench_inflate(&data, false).unwrap());
                }
            }
            continue;
        }
        let ours = specular_png::bench_inflate(&data, true).expect("ours");
        let theirs = fdeflate_inflate(&zlib, true);
        assert_eq!(ours, theirs, "{path}: output differs from fdeflate");
        let (mut to, mut tf) = (f64::MAX, f64::MAX);
        for _ in 0..runs {
            let t = Instant::now();
            std::hint::black_box(specular_png::bench_inflate(&data, false).unwrap());
            to = to.min(t.elapsed().as_secs_f64());
            let t = Instant::now();
            std::hint::black_box(fdeflate_inflate(&zlib, false));
            tf = tf.min(t.elapsed().as_secs_f64());
        }
        let mp = f64::from(info.width) * f64::from(info.height) / 1e6;
        let out = ours.0 as f64 / 1e6;
        let short: String = path
            .rsplit('/')
            .next()
            .unwrap_or(&path)
            .chars()
            .take(34)
            .collect();
        println!(
            "{short:<34} {:>7.1} {out:>7.1} | {:>8.0} {:>8.0} | {:>6.0} {:>6.0} | {:.2}x",
            zlib.len() as f64 / 1e6,
            out / to,
            out / tf,
            mp / to,
            mp / tf,
            tf / to
        );
    }
    println!("(exact: sizes and Adler-32 match; best of {runs}; ratio > 1 means ours is faster)");
}
