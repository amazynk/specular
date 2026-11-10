//! Robustness corpus.
//!
//! * `cargo run --release --example robust -- gen OUT_DIR [SEED.jpg...]` writes a
//!   deterministic corpus of mutated JPEGs (bit flips, byte overwrites, truncations, deleted /
//!   duplicated / inserted chunks, stray markers, edited segment lengths and header fields,
//!   splices) of built-in seeds (every sampling, greyscale, CMYK/YCCK, progressive, restart
//!   intervals; with `cjpeg` installed also arithmetic coding and libjpeg-turbo's own
//!   progressive scripts; 12-bit relabels) and of the given seed files.
//!   `ROBUST_PER_SEED` (default 160) sets the mutants per small seed.
//! * `cargo run --release --example robust -- check DIR...` decodes every file with every
//!   entry point (4 scales, regions, index) and fails on a panic, a file over the time limit
//!   (`ROBUST_LIMIT_MS`, default 3000), a hang (30 s) or a peak RSS over `ROBUST_RSS_MB`
//!   (default 2048).
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

#[path = "../tests/common/robust_check.rs"]
mod robust_check;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

/// xorshift64*: deterministic, no dependencies.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Photo-like RGB test image: gradients, edges and noise.
fn image(w: u32, h: u32, seed: u64) -> Vec<u8> {
    let mut r = Rng(seed | 1);
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let n = (r.next() % 24) as i32 - 12;
            let edge = if (x / 17 + y / 11) % 3 == 0 { 60 } else { 0 };
            let c = [
                (x * 255 / w.max(1)) as i32 + n + edge,
                (y * 255 / h.max(1)) as i32 - n,
                ((x ^ y) & 0xFF) as i32 / 2 + 64 + n,
            ];
            v.extend(c.map(|c| c.clamp(0, 255) as u8));
        }
    }
    v
}

fn encode(
    w: u32,
    h: u32,
    seed: u64,
    f: impl FnOnce(&mut Encoder<&mut Vec<u8>>),
    ct: ColorType,
) -> Vec<u8> {
    let rgb = image(w, h, seed);
    let px: Vec<u8> = match ct {
        ColorType::Luma => rgb.chunks_exact(3).map(|p| p[1]).collect(),
        ColorType::Cmyk | ColorType::CmykAsYcck => rgb
            .chunks_exact(3)
            .flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2], p[0] / 4])
            .collect(),
        _ => rgb,
    };
    let mut out = Vec::new();
    let mut e = Encoder::new(&mut out, 85);
    f(&mut e);
    e.encode(&px, w as u16, h as u16, ct).expect("encode");
    out
}

/// Built-in seeds: (name, bytes, mutants relative to a small seed).
fn seeds() -> Vec<(String, Vec<u8>, f64)> {
    use SamplingFactor::*;
    let mut v: Vec<(String, Vec<u8>, f64)> = Vec::new();
    let samplings = [
        ("444", R_4_4_4),
        ("422", R_4_2_2),
        ("420", R_4_2_0),
        ("440", R_4_4_0),
        ("411", F_4_1),
    ];
    for (i, (sn, sf)) in samplings.into_iter().enumerate() {
        for (pn, prog) in [("base", false), ("prog", true)] {
            let (w, h) = (97 + 13 * i as u32, 61 + 7 * i as u32);
            let rst = if i % 2 == 1 { 3 } else { 0 };
            let jpg = encode(
                w,
                h,
                i as u64 + 1,
                |e| {
                    e.set_sampling_factor(sf);
                    e.set_progressive(prog);
                    e.set_restart_interval(rst);
                    e.set_optimized_huffman_tables(i % 3 == 2);
                },
                ColorType::Rgb,
            );
            v.push((format!("{sn}-{pn}-rst{rst}"), jpg, 1.0));
        }
    }
    for (n, ct) in [
        ("gray", ColorType::Luma),
        ("cmyk", ColorType::Cmyk),
        ("ycck", ColorType::CmykAsYcck),
    ] {
        for prog in [false, true] {
            let jpg = encode(83, 59, 7, |e| e.set_progressive(prog), ct);
            v.push((
                format!("{n}-{}", if prog { "prog" } else { "base" }),
                jpg,
                1.0,
            ));
        }
    }
    // Larger seeds reach the parallel and speculative paths (fewer mutants each).
    for (n, prog) in [("big-base", false), ("big-prog", true)] {
        let jpg = encode(
            1024,
            768,
            9,
            |e| {
                e.set_quality(95);
                e.set_sampling_factor(R_4_4_4);
                e.set_progressive(prog);
            },
            ColorType::Rgb,
        );
        v.push((n.into(), jpg, 0.25));
    }
    // libjpeg-turbo: arithmetic coding and its own progressive scripts.
    let src = image(131, 89, 5);
    let mut ppm = b"P6\n131 89\n255\n".to_vec();
    ppm.extend_from_slice(&src);
    for (n, args) in [
        ("tj-arith", &["-arithmetic", "-sample", "2x2"][..]),
        (
            "tj-arith-prog",
            &["-arithmetic", "-progressive", "-sample", "1x1"],
        ),
        (
            "tj-prog-420",
            &["-progressive", "-sample", "2x2", "-restart", "2"],
        ),
        ("tj-prog-gray", &["-progressive", "-grayscale"]),
        ("tj-422-opt", &["-sample", "2x1", "-optimize"]),
    ] {
        if let Some(j) = cjpeg(&ppm, args) {
            v.push((n.into(), j, 1.0));
        } else {
            eprintln!("cjpeg not available: no {n} seed");
        }
    }
    // 12-bit relabels of 8-bit Huffman streams (same coefficients, SOF1 / SOF2 precision 12).
    let twelve: Vec<(String, Vec<u8>, f64)> = v
        .iter()
        .filter(|(n, _, _)| n.starts_with("420-") || n.starts_with("gray-"))
        .filter_map(|(n, j, k)| Some((format!("{n}-12bit"), relabel_12bit(j)?, *k)))
        .collect();
    v.extend(twelve);
    v
}

fn cjpeg(ppm: &[u8], args: &[&str]) -> Option<Vec<u8>> {
    let mut c = Command::new("cjpeg")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdin = c.stdin.take()?;
    let data = ppm.to_vec();
    let feeder = std::thread::spawn(move || std::io::Write::write_all(&mut stdin, &data));
    let out = c.wait_with_output().ok()?;
    feeder.join().ok()?.ok()?;
    out.status.success().then_some(out.stdout)
}

fn relabel_12bit(j: &[u8]) -> Option<Vec<u8>> {
    let mut v = j.to_vec();
    let p = v
        .windows(2)
        .position(|w| w[0] == 0xFF && (w[1] == 0xC0 || w[1] == 0xC2))?;
    if v[p + 1] == 0xC0 {
        v[p + 1] = 0xC1;
    }
    v[p + 4] = 12;
    Some(v)
}

/// Offsets of the markers (FF xx, xx not 00/FF) of `j`.
fn markers(j: &[u8]) -> Vec<usize> {
    (0..j.len().saturating_sub(1))
        .filter(|&i| j[i] == 0xFF && j[i + 1] != 0 && j[i + 1] != 0xFF)
        .collect()
}

/// End of the headers (first SOS segment end), or the whole file.
fn header_end(j: &[u8]) -> usize {
    markers(j)
        .into_iter()
        .find(|&i| j[i + 1] == 0xDA)
        .map_or(j.len(), |i| {
            let len = j
                .get(i + 2..i + 4)
                .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]) as usize);
            (i + 2 + len).min(j.len())
        })
}

const KINDS: [&str; 12] = [
    "flip", "bytes", "trunc", "delete", "dup", "marker", "seglen", "header", "splice", "zero",
    "ff", "multi",
];

/// One mutation of `j` (`other`: another seed for splices).
fn mutate(j: &[u8], other: &[u8], kind: &str, r: &mut Rng) -> Vec<u8> {
    let mut v = j.to_vec();
    let n = v.len().max(1);
    let hdr = header_end(j).max(4);
    // Half of the edits hit the headers, where the structure is.
    let pos = |r: &mut Rng| {
        if r.below(2) == 0 {
            2 + r.below(hdr.saturating_sub(2))
        } else {
            r.below(n)
        }
    };
    match kind {
        "flip" => {
            for _ in 0..1 + r.below(8) {
                let p = pos(r).min(n - 1);
                v[p] ^= 1 << r.below(8);
            }
        }
        "bytes" => {
            for _ in 0..1 + r.below(6) {
                let p = pos(r).min(n - 1);
                v[p] = r.next() as u8;
            }
        }
        "trunc" => v.truncate(r.below(n)),
        "delete" => {
            let p = pos(r).min(n - 1);
            let l = 1 + r.below(64.min(n - p));
            v.drain(p..p + l.min(v.len() - p));
        }
        "dup" => {
            let p = pos(r).min(n - 1);
            let l = 1 + r.below(256.min(n - p));
            let chunk = v[p..p + l].to_vec();
            let at = r.below(v.len());
            v.splice(at..at, chunk);
        }
        "marker" => {
            let m = [
                0xD0 + r.below(8) as u8,
                0xD9,
                0xD8,
                0xDA,
                0xC4,
                0xDB,
                0xDD,
                0xC0,
                0xC2,
                0xCC,
            ][r.below(10)];
            let at = r.below(v.len());
            v.splice(at..at, [0xFF, m]);
        }
        "seglen" => {
            let ms = markers(&v);
            if let Some(&m) = ms.get(r.below(ms.len()))
                && m + 3 < v.len()
            {
                let d = [0u8, 1, 2, 0xFF][r.below(4)];
                v[m + 2 + r.below(2)] = if r.below(2) == 0 {
                    d
                } else {
                    v[m + 3].wrapping_add(r.next() as u8 % 5).wrapping_sub(2)
                };
            }
        }
        "header" => {
            // Frame / scan fields: dimensions, precision, component count, sampling, tables,
            // spectral selection, successive approximation, restart interval.
            let ms: Vec<usize> = markers(&v)
                .into_iter()
                .filter(|&m| matches!(v[m + 1], 0xC0..=0xC2 | 0xDA | 0xDD | 0xC4 | 0xDB))
                .collect();
            if let Some(&m) = ms.get(r.below(ms.len())) {
                let len = v
                    .get(m + 2..m + 4)
                    .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]) as usize);
                if len > 2 {
                    let p = (m + 4 + r.below(len - 2)).min(v.len() - 1);
                    v[p] =
                        [0, 1, 0xFF, 0x11, 0x44, 0x22, 63, 64, 12, 16, r.next() as u8][r.below(11)];
                }
            }
        }
        "splice" => {
            let a = r.below(n);
            let b = r.below(other.len().max(1));
            v.truncate(a);
            v.extend_from_slice(&other[b.min(other.len())..]);
        }
        "zero" => {
            let p = pos(r).min(n - 1);
            let l = 1 + r.below(4096.min(n - p));
            v[p..p + l].fill(0);
        }
        "ff" => {
            for _ in 0..1 + r.below(4) {
                let p = r.below(n);
                v[p] = 0xFF;
            }
        }
        _ => {
            let a = KINDS[r.below(KINDS.len() - 1)];
            let b = KINDS[r.below(KINDS.len() - 1)];
            v = mutate(&v, other, a, r);
            v = mutate(&v, other, b, r);
        }
    }
    v
}

fn generate(out: &Path, extra: &[PathBuf]) {
    let per: usize = std::env::var("ROBUST_PER_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(160);
    let mut seeds = seeds();
    for p in extra {
        match std::fs::read(p) {
            Ok(d) => seeds.push((
                format!(
                    "seed-{}",
                    p.file_stem().unwrap_or_default().to_string_lossy()
                ),
                d,
                0.5,
            )),
            Err(e) => eprintln!("skip seed {}: {e}", p.display()),
        }
    }
    std::fs::create_dir_all(out).expect("create output dir");
    let mut count = 0usize;
    let mut bytes = 0usize;
    for (si, (name, data, weight)) in seeds.iter().enumerate() {
        let other = &seeds[(si + 1) % seeds.len()].1;
        std::fs::write(out.join(format!("{name}-orig.jpg")), data).expect("write");
        count += 1;
        let n = ((per as f64) * weight).round() as usize;
        for k in 0..n {
            let kind = KINDS[k % KINDS.len()];
            let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ ((si as u64) << 32) ^ k as u64);
            r.next();
            let m = mutate(data, other, kind, &mut r);
            bytes += m.len();
            std::fs::write(out.join(format!("{name}-{k:04}-{kind}.jpg")), m).expect("write");
            count += 1;
        }
    }
    println!(
        "wrote {count} files ({:.1} MB) from {} seeds to {}",
        bytes as f64 / 1e6,
        seeds.len(),
        out.display()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env_num = |k: &str, d: u64| {
        std::env::var(k)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(d)
    };
    match args.first().map(String::as_str) {
        Some("gen") if args.len() >= 2 => {
            let extra: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            generate(Path::new(&args[1]), &extra);
        }
        Some("check") if args.len() >= 2 => {
            let files: Vec<PathBuf> = args[1..]
                .iter()
                .flat_map(|d| robust_check::list(Path::new(d)))
                .collect();
            let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
            let rep = robust_check::check_all(
                &files,
                workers,
                Duration::from_millis(env_num("ROBUST_LIMIT_MS", 3000)),
                Duration::from_secs(30),
            );
            println!("{}", rep.summary());
            if rep.failed(env_num("ROBUST_RSS_MB", 2048) << 20) {
                std::process::exit(1);
            }
            if !rep.hangs.is_empty() {
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage: robust gen OUT_DIR [SEED.jpg...] | robust check DIR...");
            std::process::exit(2);
        }
    }
}
