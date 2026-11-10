//! Throughput of specular-png vs the `png` and `zune-png` crates on large generated images.
//!
//! `cargo run --release -p specular-png --example bench-png [-- <file.png>...]`
//!
//! Generated images are cached under `target/agent-jpeg/png-bench/`. Checksums are skipped by
//! all three decoders. Reported: megapixels per second, best of several runs.

#[path = "../tests/common/mod.rs"]
mod common;

use std::path::PathBuf;
use std::time::Instant;

use common::*;
use specular_png::{DecodeOptions, Output, Parallelism, decode};

/// One timed variant.
type Bench<'a> = Box<dyn FnMut() -> usize + 'a>;

fn best(mut f: impl FnMut() -> usize, runs: usize) -> f64 {
    let mut b = f64::MAX;
    for _ in 0..runs {
        let t = Instant::now();
        std::hint::black_box(f());
        b = b.min(t.elapsed().as_secs_f64());
    }
    b
}

fn cases() -> Vec<(String, Vec<u8>)> {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target/agent-jpeg/png-bench");
    let _ = std::fs::create_dir_all(&dir);
    let specs: [(&str, usize, usize, u8, u8, bool, Content); 5] = [
        ("photo RGB8 24MP", 6000, 4000, 2, 8, false, Content::Photo),
        (
            "screenshot RGBA8 8MP",
            3840,
            2160,
            6,
            8,
            false,
            Content::Screen,
        ),
        (
            "screenshot RGB8 8MP",
            3840,
            2160,
            2,
            8,
            false,
            Content::Screen,
        ),
        ("photo RGB16 12MP", 4000, 3000, 2, 16, false, Content::Photo),
        (
            "photo RGB8 12MP Adam7",
            4000,
            3000,
            2,
            8,
            true,
            Content::Photo,
        ),
    ];
    let mut v = Vec::new();
    for (name, w, h, ct, depth, il, content) in specs {
        let path = dir.join(format!("{}.png", name.replace(' ', "_")));
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("generating {name}...");
                let s = make(w, h, ct, depth, content, 42);
                let d = encode(&s, il, Filters::Mixed, &Extra::default(), 1);
                let _ = std::fs::write(&path, &d);
                d
            }
        };
        v.push((name.to_string(), data));
    }
    v
}

/// Exact match against the `png` crate (EXPAND) for every path that must be lossless: RGBA8 and
/// the Auto8 layout, single-threaded and with the default parallelism.
fn check(name: &str, data: &[u8]) {
    let mut d = png::Decoder::new(std::io::Cursor::new(data));
    d.set_transformations(png::Transformations::EXPAND);
    let mut r = d.read_info().expect("png read_info");
    let mut want = vec![0u8; r.output_buffer_size().expect("size")];
    let oi = r.next_frame(&mut want).expect("png decode");
    let ch = oi.color_type.samples();
    assert_eq!(
        oi.bit_depth,
        png::BitDepth::Eight,
        "check handles 8-bit files"
    );
    drop(r);
    let n = oi.width as usize * oi.height as usize;
    let px = |i: usize| -> [u8; 4] {
        let s = &want[i * ch..i * ch + ch];
        match ch {
            1 => [s[0], s[0], s[0], 255],
            2 => [s[0], s[0], s[0], s[1]],
            3 => [s[0], s[1], s[2], 255],
            _ => [s[0], s[1], s[2], s[3]],
        }
    };
    for (output, par) in [
        (Output::Rgba8, Parallelism::Single),
        (Output::Rgba8, Parallelism::Auto),
        (Output::Auto8, Parallelism::Single),
        (Output::Auto8, Parallelism::Auto),
    ] {
        let o = DecodeOptions {
            output,
            parallelism: par,
            ..Default::default()
        };
        let img = decode(data, &o).expect("decode");
        assert!(!img.truncated);
        let got = img.to_rgba8();
        let bad = (0..n).filter(|&i| got[i * 4..i * 4 + 4] != px(i)).count();
        println!(
            "check {name}: {output:?} {par:?} {:?}: {}",
            img.layout,
            if bad == 0 {
                "exact".to_string()
            } else {
                format!("{bad} pixels differ")
            }
        );
        assert_eq!(bad, 0);
    }
    for scale in [2, 4, 8] {
        let o = |par| DecodeOptions {
            scale,
            parallelism: par,
            ..Default::default()
        };
        let a = decode(data, &o(Parallelism::Single)).expect("decode");
        let b = decode(data, &o(Parallelism::Threads)).expect("decode");
        assert_eq!(a.planes, b.planes, "1/{scale}: threads changed the result");
    }
    println!("check {name}: 1/2 1/4 1/8 identical single vs threaded");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cases = if args.is_empty() {
        cases()
    } else {
        args.iter()
            .map(|p| (p.clone(), std::fs::read(p).expect("read")))
            .collect()
    };
    // `PNG_BENCH_PROFILE=single|auto|png`: only loop that decoder (for `perf record`).
    if let Ok(which) = std::env::var("PNG_BENCH_PROFILE") {
        for (_, data) in &cases {
            for _ in 0..8 {
                let par = if which == "auto" {
                    Parallelism::Auto
                } else {
                    Parallelism::Single
                };
                if which == "png" {
                    let mut d = png::Decoder::new(std::io::Cursor::new(&data[..]));
                    d.ignore_checksums(true);
                    let mut r = d.read_info().unwrap();
                    let mut buf = vec![0u8; r.output_buffer_size().unwrap()];
                    r.next_frame(&mut buf).unwrap();
                } else {
                    let o = DecodeOptions {
                        parallelism: par,
                        scale: std::env::var("PNG_BENCH_SCALE")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1),
                        ..Default::default()
                    };
                    std::hint::black_box(decode(data, &o).unwrap());
                }
            }
        }
        return;
    }
    if !args.is_empty() && std::env::var_os("PNG_BENCH_NOCHECK").is_none() {
        for (name, data) in &cases {
            check(name, data);
        }
    }
    // Round-robin: every variant runs once per round and keeps its best time, so load changes on
    // a shared machine hit all columns alike.
    let runs: usize = std::env::var("PNG_BENCH_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);
    let mut first = true;
    for (name, data) in &cases {
        let info = specular_png::read_info(data).expect("info");
        let mp = f64::from(info.width) * f64::from(info.height) / 1e6;
        let ours = |output, par, scale| {
            let o = DecodeOptions {
                output,
                parallelism: par,
                scale,
                ..Default::default()
            };
            move || decode(data, &o).unwrap().planes.len()
        };
        let png_f = || {
            let mut d = png::Decoder::new(std::io::Cursor::new(&data[..]));
            d.ignore_checksums(true);
            let mut r = d.read_info().unwrap();
            let mut buf = vec![0u8; r.output_buffer_size().unwrap()];
            r.next_frame(&mut buf).unwrap();
            buf.len()
        };
        let zune_f = || {
            let o = zune_core::options::DecoderOptions::default()
                .png_set_confirm_crc(false)
                .inflate_set_confirm_adler(false);
            let mut d = zune_png::PngDecoder::new_with_options(
                zune_core::bytestream::ZCursor::new(&data[..]),
                o,
            );
            d.decode_raw().unwrap().len()
        };
        let fs: Vec<(&str, Bench<'_>)> = vec![
            ("1T", Box::new(ours(Output::Auto8, Parallelism::Single, 1))),
            ("MT", Box::new(ours(Output::Auto8, Parallelism::Threads, 1))),
            ("auto", Box::new(ours(Output::Auto8, Parallelism::Auto, 1))),
            ("rgba", Box::new(ours(Output::Rgba8, Parallelism::Auto, 1))),
            (
                "1/4 1T",
                Box::new(ours(Output::Auto8, Parallelism::Single, 4)),
            ),
            ("1/4", Box::new(ours(Output::Auto8, Parallelism::Auto, 4))),
            ("1/8", Box::new(ours(Output::Auto8, Parallelism::Auto, 8))),
            ("png", Box::new(png_f)),
            ("zune", Box::new(zune_f)),
        ];
        let (labels, mut fs): (Vec<_>, Vec<_>) = fs.into_iter().unzip();
        if first {
            print!("{:<34} {:>6} |", "image", "MB");
            for l in &labels {
                print!(" {l:>7}");
            }
            println!();
            first = false;
        }
        let mut t = vec![f64::MAX; fs.len()];
        for _ in 0..runs {
            for (f, b) in fs.iter_mut().zip(&mut t) {
                *b = b.min(best(f, 1));
            }
        }
        let short: String = name
            .rsplit('/')
            .next()
            .unwrap_or(name)
            .chars()
            .take(34)
            .collect();
        print!("{short:<34} {:>6.1} |", data.len() as f64 / 1e6);
        for x in &t {
            print!(" {:>7.0}", mp / x);
        }
        println!();
    }
    println!(
        "(MP/s, best of {runs} round-robin runs. Ours: Auto8 layout; 1T single thread, MT forced \
         pipeline, auto = default; rgba = Rgba8 output; 1/4, 1/8 scaled)"
    );
}
