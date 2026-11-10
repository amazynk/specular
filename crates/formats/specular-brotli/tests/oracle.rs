//! Round trips through the upstream encoder (a dev-dependency oracle), and broken streams.

use specular_brotli::{Error, decompress};

fn compress(data: &[u8], quality: i32, lgwin: i32) -> Vec<u8> {
    let params = brotli::enc::BrotliEncoderParams {
        quality,
        lgwin,
        ..Default::default()
    };
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &data[..], &mut out, &params).unwrap();
    out
}

/// A deterministic xorshift generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn random(n: usize, seed: u64) -> Vec<u8> {
    let mut r = Rng(seed);
    (0..n).map(|_| r.next() as u8).collect()
}

/// English-like text, heavy on dictionary words, with some markup and UTF-8.
fn text(n: usize, seed: u64) -> Vec<u8> {
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
        "сегодня",
        "東京",
        "function",
        "return",
        "Government",
        "INTERNATIONAL",
        "\n",
        ", ",
        ". ",
        "the following",
        "that",
        "would",
    ];
    let mut r = Rng(seed);
    let mut out = Vec::with_capacity(n + 32);
    while out.len() < n {
        out.extend_from_slice(WORDS[r.next() as usize % WORDS.len()].as_bytes());
        out.push(b' ');
    }
    out.truncate(n);
    out
}

/// Structured binary: runs, small deltas, repeated records, a little noise.
fn binary(n: usize, seed: u64) -> Vec<u8> {
    let mut r = Rng(seed);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        match r.next() % 4 {
            0 => {
                let b = r.next() as u8;
                out.extend(std::iter::repeat_n(b, (r.next() % 40) as usize));
            }
            1 => {
                let mut v = r.next() as u16;
                for _ in 0..(r.next() % 64) {
                    v = v.wrapping_add((r.next() % 7) as u16);
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            2 if out.len() > 300 => {
                let at = (r.next() as usize) % (out.len() - 200);
                let len = 4 + (r.next() % 150) as usize;
                out.extend_from_within(at..at + len);
            }
            _ => out.extend((0..(r.next() % 16)).map(|_| r.next() as u8)),
        }
    }
    out.truncate(n);
    out
}

fn round_trip(data: &[u8], quality: i32, lgwin: i32) {
    let c = compress(data, quality, lgwin);
    let d = decompress(&c, data.len()).unwrap_or_else(|e| {
        panic!("q{quality} w{lgwin} len {}: {e}", data.len());
    });
    assert!(
        d == data,
        "q{quality} w{lgwin} len {}: wrong output",
        data.len()
    );
}

#[test]
fn all_qualities() {
    let inputs = [text(70_000, 1), binary(70_000, 2), random(5_000, 3)];
    for q in 0..=11 {
        for data in &inputs {
            round_trip(data, q, 22);
        }
    }
}

#[test]
fn window_sizes() {
    let data = text(300_000, 4);
    let bin = binary(300_000, 5);
    for lgwin in [10, 11, 12, 16, 17, 18, 20, 24] {
        for q in [2, 5, 9, 11] {
            round_trip(&data, q, lgwin);
            round_trip(&bin, q, lgwin);
        }
    }
}

#[test]
fn small_and_empty() {
    for n in [0, 1, 2, 3, 4, 7, 16, 100] {
        for q in [0, 1, 5, 11] {
            round_trip(&text(n, n as u64 + 9), q, 16);
            round_trip(&random(n, n as u64 + 9), q, 16);
        }
    }
}

#[test]
fn uncompressed_meta_blocks() {
    // Incompressible data comes out as uncompressed meta-blocks; check that they are there.
    let data = random(400_000, 6);
    for q in [1, 5, 9] {
        let c = compress(&data, q, 22);
        assert!(c.len() > data.len(), "q{q}: expected stored meta-blocks");
        assert_eq!(decompress(&c, data.len()).unwrap(), data);
    }
    // And mixed with compressed ones.
    let mut mixed = random(100_000, 7);
    mixed.extend(text(100_000, 8));
    mixed.extend(random(100_000, 9));
    for q in [1, 5, 11] {
        round_trip(&mixed, q, 22);
    }
}

#[test]
fn dictionary_heavy_text() {
    // Short texts whose words are only in the static dictionary, at high quality.
    let data = text(4_000, 10);
    for q in [5, 9, 10, 11] {
        round_trip(&data, q, 22);
    }
    let html = b"<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>The International \
        Government Information</title></head><body><div class=\"content\"><p>However, the \
        following people would like to know that the time of the year is coming.</p></div>\
        </body></html>";
    for q in [5, 11] {
        round_trip(html, q, 22);
    }
}

#[test]
fn output_cap() {
    let data = text(10_000, 11);
    let c = compress(&data, 9, 22);
    assert_eq!(decompress(&c, data.len()).unwrap(), data);
    assert_eq!(
        decompress(&c, data.len() - 1),
        Err(Error::TooLarge(data.len() - 1))
    );
    assert_eq!(decompress(&c, 0), Err(Error::TooLarge(0)));
}

#[test]
fn truncated_streams_error() {
    let data = text(20_000, 12);
    for q in [0, 5, 11] {
        let c = compress(&data, q, 22);
        for cut in (0..c.len()).step_by(7).chain([c.len() - 1]) {
            let r = decompress(&c[..cut], data.len());
            assert!(r.is_err(), "q{q} cut at {cut} of {} decoded", c.len());
        }
    }
}

#[test]
fn corrupt_streams_do_not_panic() {
    let data = text(20_000, 13);
    let mut r = Rng(14);
    for q in [0, 5, 11] {
        let c = compress(&data, q, 22);
        for _ in 0..300 {
            let mut bad = c.clone();
            for _ in 0..1 + r.next() % 4 {
                let i = r.next() as usize % bad.len();
                bad[i] ^= 1 << (r.next() % 8);
            }
            // Either an error or some output within the cap; never a panic.
            if let Ok(out) = decompress(&bad, data.len()) {
                assert!(out.len() <= data.len());
            }
        }
    }
    // Pure noise.
    for seed in 0..500 {
        let noise = random(1 + seed as usize % 200, seed + 100);
        let _ = decompress(&noise, 1 << 16);
    }
}

/// Rules RFC 7932 gives that brotli-decompressor does not check (Google's C decoder does).
fn stricter(e: &Error) -> bool {
    matches!(
        e,
        Error::Invalid(
            "insert length past the meta-block"
                | "copy length past the meta-block"
                | "dictionary word past the meta-block"
                // 0010001: the large-window extension, not RFC 7932.
                | "window size"
        )
    )
}

/// Inputs the differential fuzzer found, against the reference decoder.
#[test]
fn fuzz_regressions() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/regressions");
    for e in std::fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        let data = std::fs::read(&path).unwrap();
        let mut reference = Vec::new();
        let ok = brotli_decompressor::BrotliDecompress(&mut &data[..], &mut reference).is_ok();
        let ours = decompress(&data, 1 << 20);
        if ok {
            match ours {
                Ok(out) => assert!(out == reference, "{}", path.display()),
                Err(e) => assert!(stricter(&e), "{}: {e}", path.display()),
            }
        } else {
            assert!(ours.is_err(), "{}", path.display());
        }
    }
}

#[test]
fn agrees_with_reference_decoder_on_noise() {
    // On random streams, success and output match the reference decoder whenever it succeeds
    // within the cap.
    for seed in 0..2000u64 {
        let noise = random(4 + seed as usize % 64, seed + 7000);
        let mut reference = Vec::new();
        let ok = brotli_decompressor::BrotliDecompress(&mut &noise[..], &mut reference).is_ok();
        let ours = decompress(&noise, 1 << 20);
        if ok && reference.len() <= 1 << 20 {
            match ours {
                Ok(out) => assert!(out == reference, "seed {seed}"),
                Err(e) => assert!(stricter(&e), "seed {seed}: {e}"),
            }
        }
    }
}
