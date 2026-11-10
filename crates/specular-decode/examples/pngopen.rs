//! Raw `specular-png` vs the registry path on the same PNG files, interleaved run by run (the
//! machine may be shared, so the variants see the same load).
//!
//! `cargo run --release -p specular-decode --example pngopen -- [--scale N] [--runs N] file.png...`
//!
//! - `raw`: `specular_png::decode` on bytes already in memory (the decoder alone).
//! - `raw+map`: the same on a freshly opened (mmapped) `Source`, so file mapping is included.
//! - `registry`: `Registry::open_path` + `Session::decode` (whole image, parallel), what the
//!   viewer does; `open` and `decode` are also shown separately.
//!
//! - `--levels 8,1`: also `specular_png::decode_scales` (all the levels from one pass).
//! - `--raw-only`: skip the mapped and registry variants (for very large files).
//! - `--no-wavefront`: unfilter row by row (to measure the parallel unfilter).
//!
//! Best of N runs, milliseconds.
use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry};
use specular_png::{DecodeOptions, Output, Parallelism};
use std::path::Path;
use std::time::Instant;

fn main() {
    let mut scale = 1u32;
    let mut runs = 5usize;
    let mut files = Vec::new();
    let mut levels: Vec<u32> = Vec::new();
    let mut raw_only = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => scale = args.next().and_then(|s| s.parse().ok()).expect("scale"),
            "--runs" => runs = args.next().and_then(|s| s.parse().ok()).expect("runs"),
            "--levels" => {
                levels = args
                    .next()
                    .expect("levels")
                    .split(',')
                    .map(|s| s.parse().expect("level"))
                    .collect()
            }
            "--raw-only" => raw_only = true,
            "--no-wavefront" => specular_png::set_wavefront_min(u64::MAX, 0),
            _ => files.push(a),
        }
    }
    let reg = Registry::new();
    let cancel = CancelToken::new();
    for path in &files {
        let bytes = std::fs::read(path).expect("read");
        let info = specular_png::read_info(&bytes).expect("png");
        println!(
            "{path}: {}x{} {} ({:.1} MB)",
            info.width,
            info.height,
            info.coding(),
            bytes.len() as f64 / 1e6
        );
        let opts = DecodeOptions {
            output: Output::Auto8,
            scale,
            parallelism: Parallelism::Auto,
            verify_checksums: false,
            cancel: None,
            wavefront: specular_png::Wavefront::Auto,
        };
        if !levels.is_empty() {
            let mut best = f64::MAX;
            for _ in 0..runs {
                let t = Instant::now();
                let imgs = specular_png::decode_scales(&bytes, &opts, &levels).expect("levels");
                best = best.min(t.elapsed().as_secs_f64());
                drop(std::hint::black_box(imgs));
            }
            println!("  levels {levels:?} in one pass: {:7.1} ms", best * 1e3);
        }
        if raw_only {
            let mut best = f64::MAX;
            for _ in 0..runs {
                let t = Instant::now();
                let img = specular_png::decode(&bytes, &opts).expect("raw");
                best = best.min(t.elapsed().as_secs_f64());
                drop(std::hint::black_box(img));
            }
            println!("  1/{scale}: raw {:7.1} ms", best * 1e3);
            continue;
        }
        let mut copied = f64::MAX;
        let (mut raw, mut raw_map, mut reg_total, mut reg_open, mut reg_dec) =
            (f64::MAX, f64::MAX, f64::MAX, f64::MAX, f64::MAX);
        for _ in 0..runs {
            let t = Instant::now();
            let img = specular_png::decode(&bytes, &opts).expect("raw");
            raw = raw.min(t.elapsed().as_secs_f64());
            drop(std::hint::black_box(img));

            let t = Instant::now();
            let src = Source::open(Path::new(path)).expect("source");
            let img = specular_png::decode(src.bytes(), &opts).expect("raw+map");
            raw_map = raw_map.min(t.elapsed().as_secs_f64());
            drop(std::hint::black_box(img));
            drop(src);

            let t = Instant::now();
            let s = reg.open_path(Path::new(path)).expect("open");
            let t1 = t.elapsed().as_secs_f64();
            let req = DecodeRequest {
                parallel: true,
                ..DecodeRequest::full(scale)
            };
            let r = s.decode(&req, &cancel).expect("decode");
            let total = t.elapsed().as_secs_f64();
            reg_total = reg_total.min(total);
            reg_open = reg_open.min(t1);
            reg_dec = reg_dec.min(total - t1);
            // What the old adapter added per whole-level request (a copy of its cached level).
            let t = Instant::now();
            let copy = r.clone();
            copied = copied.min(t.elapsed().as_secs_f64());
            drop(std::hint::black_box(copy));
            drop(std::hint::black_box(r));
            drop(s);
        }
        println!(
            "  1/{scale}: raw {:7.1} ms | raw+map {:7.1} ms | registry {:7.1} ms (open {:.1} + decode {:.1}) | overhead {:+.1} ms | level copy (old adapter) {:.1} ms",
            raw * 1e3,
            raw_map * 1e3,
            reg_total * 1e3,
            reg_open * 1e3,
            reg_dec * 1e3,
            (reg_total - raw) * 1e3,
            copied * 1e3
        );
    }
}
