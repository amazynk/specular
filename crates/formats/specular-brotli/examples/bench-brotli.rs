//! Decompression speed against the upstream decoder (`brotli-decompressor`, a dev-dependency):
//! `cargo run --release --example bench-brotli -p specular-brotli`.

use std::hint::black_box;
use std::time::{Duration, Instant};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn text(n: usize) -> Vec<u8> {
    const WORDS: &[&str] = &[
        "the",
        "time",
        "of",
        "and",
        "information",
        "people",
        "between",
        "however",
        "<div class=\"",
        "</a>",
        "http://www.",
        ".com/",
        "Über",
        "naïve",
        "function",
        "return",
        "Government",
        "\n",
        ", ",
        ". ",
        "that",
        "would",
        "compression",
        "dictionary",
        "window",
        "stream",
    ];
    let mut r = Rng(1);
    let mut out = Vec::with_capacity(n + 32);
    while out.len() < n {
        out.extend_from_slice(WORDS[r.next() as usize % WORDS.len()].as_bytes());
        out.push(b' ');
    }
    out.truncate(n);
    out
}

/// Image-like binary: 16-bit samples of a smooth signal with noise.
fn binary(n: usize) -> Vec<u8> {
    let mut r = Rng(2);
    let mut out = Vec::with_capacity(n);
    let mut v = 30_000i32;
    while out.len() < n {
        v = (v + (r.next() % 61) as i32 - 30).clamp(0, 65_535);
        out.extend_from_slice(&(v as u16).to_le_bytes());
    }
    out.truncate(n);
    out
}

fn time(mut f: impl FnMut()) -> Duration {
    f();
    let mut best = Duration::MAX;
    for _ in 0..15 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed());
    }
    best
}

fn main() {
    println!("| input | quality | compressed | ours MB/s | brotli-decompressor MB/s |");
    println!("|---|---|---|---|---|");
    for (name, data) in [
        ("1 MB text", text(1 << 20)),
        ("1 MB binary", binary(1 << 20)),
    ] {
        for q in [1, 5, 9, 11] {
            let params = brotli::enc::BrotliEncoderParams {
                quality: q,
                lgwin: 22,
                ..Default::default()
            };
            let mut c = Vec::new();
            brotli::BrotliCompress(&mut &data[..], &mut c, &params).unwrap();
            assert_eq!(specular_brotli::decompress(&c, data.len()).unwrap(), data);
            let ours = time(|| {
                black_box(specular_brotli::decompress(black_box(&c), data.len()).unwrap());
            });
            let theirs = time(|| {
                let mut out = Vec::with_capacity(data.len());
                brotli_decompressor::BrotliDecompress(&mut black_box(&c[..]), &mut out).unwrap();
                black_box(out);
            });
            let mbs = |d: Duration| data.len() as f64 / d.as_secs_f64() / 1e6;
            println!(
                "| {name} | {q} | {} | {:.0} | {:.0} |",
                c.len(),
                mbs(ours),
                mbs(theirs)
            );
        }
    }
}
