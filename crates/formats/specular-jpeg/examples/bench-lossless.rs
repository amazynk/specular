//! Huffman lossless (SOF3) benchmark and instruction-count harness.
//!
//! ```text
//! bench-lossless gen DIR                 write the fixtures (small, medium, large; with and
//!                                        without restart markers) into DIR
//! bench-lossless measure PATH FILE       one counted decode (see `perfgate_measure`)
//! bench-lossless time FILE...            wall-clock, best and median of BENCH_RUNS (default 7)
//! bench-lossless raw FILE OUT            `decode_wide` samples, interleaved u16 LE, to OUT
//!                                        (for comparing with libjpeg-turbo)
//! ```
//!
//! PATH is `display` (the 8-bit display path, single-threaded), `display-mt` (the same with
//! `Parallelism::Auto`), `wide` (`decode_wide`, raw samples, in a one-thread pool),
//! `wide-mt` (the same on the global pool) or `oracle` (`jpeg-decoder`, the
//! test oracle, plus the same 8-bit output stage as the old fallback). Count instructions with
//! `valgrind --tool=callgrind --toggle-collect='*perfgate_measure*'`, as tools/bench/perf-gate.py
//! does, so the cases can join the perf gate as they are.
//!
//! The fixtures come from the integer-only encoder below, so they are byte-identical on every
//! machine: 16-bit-container sensor-like data (smooth gradient, texture and noise), predictor 1
//! (as DNG and Canon CR2 write) or 6, per-image optimal Huffman tables.

use std::hint::black_box;
use std::time::{Duration, Instant};

use specular_jpeg::{DecodeOptions, Parallelism, decode, decode_wide};

/// Fixture shapes: name, width, height, components, precision, predictor, restart rows.
const FIXTURES: &[(&str, u32, u32, usize, u8, u8, u32)] = &[
    ("small", 320, 240, 1, 12, 1, 0),
    ("small-rst", 320, 240, 1, 12, 1, 8),
    ("medium", 2048, 1536, 1, 14, 1, 0),
    ("medium-rst", 2048, 1536, 1, 14, 1, 8),
    ("medium-rgb", 1600, 1200, 3, 8, 6, 0),
    ("large", 6000, 4000, 1, 14, 1, 0),
    ("large-rst", 6000, 4000, 1, 14, 1, 8),
];

/// The one-thread pool of the `wide` path, on the main thread (built before the counted
/// region; callgrind counts the main thread).
static ONE: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();

fn main() {
    ONE.get_or_init(|| {
        specular_thread::rayon_pool("bench-one")
            .num_threads(1)
            .use_current_thread()
            .build()
            .unwrap()
    });
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gen") if args.len() == 2 => {
            let dir = std::path::Path::new(&args[1]);
            std::fs::create_dir_all(dir).unwrap();
            for &(name, w, h, nc, p, pred, ri_rows) in FIXTURES {
                let px = synth(w as usize, h as usize, nc, p);
                let jpg = encode(w, h, nc, p, pred, ri_rows, &px);
                let path = dir.join(format!("{name}.jpg"));
                std::fs::write(&path, &jpg).unwrap();
                println!("{} {} bytes", path.display(), jpg.len());
            }
        }
        Some("measure") if args.len() == 3 => {
            let data = std::fs::read(&args[2]).unwrap();
            let sum = perfgate_measure(&args[1], &data);
            println!("{} {}: checksum {sum:016x}", args[1], args[2]);
        }
        Some("raw") if args.len() == 3 => {
            let data = std::fs::read(&args[1]).unwrap();
            let img = decode_wide(&data, None).unwrap();
            let nc = img.planes.len();
            let mut out = Vec::with_capacity(img.planes[0].data.len() * nc * 2);
            for i in 0..img.planes[0].data.len() {
                for p in &img.planes {
                    out.extend_from_slice(&p.data.get(i).copied().unwrap_or(0).to_le_bytes());
                }
            }
            std::fs::write(&args[2], out).unwrap();
        }
        Some("time") => {
            let runs: usize = std::env::var("BENCH_RUNS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(7);
            for path in &args[1..] {
                let data = std::fs::read(path).unwrap();
                let mut line = format!("{path}:");
                for what in ["oracle", "display", "display-mt", "wide", "wide-mt"] {
                    let (best, median) = time(runs, || perfgate_measure(what, &data));
                    line += &format!(" {what} {:.2}/{:.2} ms", ms(best), ms(median));
                }
                println!("{line}");
            }
        }
        _ => {
            eprintln!("usage: bench-lossless gen DIR | measure PATH FILE | time FILE...");
            std::process::exit(2);
        }
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn time(runs: usize, mut f: impl FnMut() -> u64) -> (Duration, Duration) {
    black_box(f());
    let mut t: Vec<Duration> = (0..runs.max(1))
        .map(|_| {
            let s = Instant::now();
            black_box(f());
            s.elapsed()
        })
        .collect();
    t.sort();
    (t[0], t[t.len() / 2])
}

/// The counted region: one full decode, including freeing the output. Returns a checksum of
/// the output so every path's result can be compared. The name is the callgrind toggle
/// pattern; keep it unique.
#[inline(never)]
fn perfgate_measure(path: &str, data: &[u8]) -> u64 {
    let data = black_box(data);
    match path {
        "display" | "display-mt" => {
            let opts = DecodeOptions {
                parallelism: if path == "display" {
                    Parallelism::Single
                } else {
                    Parallelism::Auto
                },
                ..Default::default()
            };
            let d = decode(data, &opts).unwrap();
            fnv(&d.planes[0].data)
        }
        "wide" => {
            let pool = ONE.get().expect("built in main");
            pool.install(|| perfgate_measure("wide-mt", data))
        }
        "wide-mt" => {
            let img = decode_wide(data, None).unwrap();
            img.planes
                .iter()
                .fold(0, |h, p| h ^ fnv16(&p.data).rotate_left(7))
        }
        "oracle" => fnv(&oracle(data)),
        _ => panic!("unknown path {path}"),
    }
}

/// `jpeg-decoder` to 8-bit RGBA, as the display path's former fallback did (no downscale).
fn oracle(data: &[u8]) -> Vec<u8> {
    let mut d = jpeg_decoder::Decoder::new(data);
    let px = d.decode().unwrap();
    let info = d.info().unwrap();
    let npx = info.width as usize * info.height as usize;
    let nc = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 | jpeg_decoder::PixelFormat::L16 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        jpeg_decoder::PixelFormat::CMYK32 => 4,
    };
    let bytes = px.len() / (npx * nc);
    let p = specular_jpeg::read_info(data).unwrap().precision as u32;
    let max = (1u32 << p) - 1;
    let s = |i: usize| -> u8 {
        if bytes == 1 {
            px[i]
        } else {
            let v = u16::from_ne_bytes([px[2 * i], px[2 * i + 1]]) as u32;
            ((v.min(max) * 255 + max / 2) / max) as u8
        }
    };
    let mut out = vec![0u8; npx * 4];
    for (i, o) in out.chunks_exact_mut(4).enumerate() {
        match nc {
            1 => o.copy_from_slice(&[s(i), s(i), s(i), 255]),
            _ => o.copy_from_slice(&[s(3 * i), s(3 * i + 1), s(3 * i + 2), 255]),
        }
    }
    out
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ x as u64).wrapping_mul(0x100_0000_01b3)
    })
}

fn fnv16(b: &[u16]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ x as u64).wrapping_mul(0x100_0000_01b3)
    })
}

/// Sensor-like samples, `nc` interleaved per pixel: a smooth gradient, a texture and about
/// five bits of noise at `p` bits.
fn synth(w: usize, h: usize, nc: usize, p: u8) -> Vec<u16> {
    let max = (1i64 << p) - 1;
    let mut s = 0x2545_f491_u32;
    let mut out = Vec::with_capacity(w * h * nc);
    for y in 0..h {
        for x in 0..w {
            for c in 0..nc {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let grad = (x * 3 + y * 2 + c * 97) as i64 * max / (3 * w + 2 * h + 300) as i64;
                let tex = ((x / 7 + y / 5) % 9) as i64 * (max >> 6);
                let noise = (s & 31) as i64 - 16 + ((s >> 8) & 7) as i64 * ((x ^ y) & 1) as i64;
                out.push((grad + tex + noise * (max >> 12).max(1)).clamp(0, max) as u16);
            }
        }
    }
    out
}

/// Lossless JPEG (SOF3) encoder: `nc` components at 1x1 sampling, one interleaved scan,
/// predictor `pred`, a restart marker every `ri_rows` rows, an optimal Huffman table per image.
fn encode(w: u32, h: u32, nc: usize, p: u8, pred: u8, ri_rows: u32, px: &[u16]) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let init = 1i32 << (p - 1);
    // Differences (category, extra bits) in scan order, with restart positions.
    let mut syms: Vec<(u8, u16)> = Vec::with_capacity(px.len());
    let mut first_row = 0usize;
    for y in 0..hu {
        if ri_rows > 0 && y > 0 && y % ri_rows as usize == 0 {
            first_row = y;
        }
        for x in 0..wu {
            for c in 0..nc {
                let g = |xx: usize, yy: usize| px[(yy * wu + xx) * nc + c] as i32;
                let pv = if y == first_row {
                    if x == 0 { init } else { g(x - 1, y) }
                } else if x == 0 {
                    g(x, y - 1)
                } else {
                    let (ra, rb, rc) = (g(x - 1, y), g(x, y - 1), g(x - 1, y - 1));
                    match pred {
                        1 => ra,
                        2 => rb,
                        3 => rc,
                        4 => ra + rb - rc,
                        5 => ra + ((rb - rc) >> 1),
                        6 => rb + ((ra - rc) >> 1),
                        _ => (ra + rb) >> 1,
                    }
                };
                let d = ((g(x, y) - pv) as i16) as i32;
                let cat = if d == -32768 {
                    16
                } else {
                    32 - d.unsigned_abs().leading_zeros()
                };
                let bits = if d < 0 { d + (1 << cat) - 1 } else { d };
                syms.push((cat as u8, bits as u16));
            }
        }
    }
    let mut freq = [0u64; 18];
    for &(c, _) in &syms {
        freq[c as usize] += 1;
    }
    // Symbol 17 is the reserved all-ones code (T.81 K.2), never written.
    freq[17] = 1;
    let lens = code_lengths(&freq);
    let mut order: Vec<usize> = (0..18).filter(|&s| lens[s] > 0).collect();
    order.sort_by_key(|&s| (lens[s], s));
    let mut codes = [(0u32, 0u32); 18];
    let (mut code, mut len) = (0u32, 0u32);
    for &s in &order {
        code <<= lens[s] - len;
        len = lens[s];
        codes[s] = (code, len);
        code += 1;
    }
    let mut counts = [0u8; 16];
    let mut vals = Vec::new();
    for &s in order.iter().filter(|&&s| s < 17) {
        counts[lens[s] as usize - 1] += 1;
        vals.push(s as u8);
    }
    let mut o = vec![0xFF, 0xD8];
    let seg = |o: &mut Vec<u8>, m: u8, body: &[u8]| {
        o.extend_from_slice(&[0xFF, m]);
        o.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
        o.extend_from_slice(body);
    };
    let mut sof = vec![p];
    sof.extend_from_slice(&(h as u16).to_be_bytes());
    sof.extend_from_slice(&(w as u16).to_be_bytes());
    sof.push(nc as u8);
    for c in 0..nc {
        sof.extend_from_slice(&[c as u8 + 1, 0x11, 0]);
    }
    seg(&mut o, 0xC3, &sof);
    let mut dht = vec![0u8];
    dht.extend_from_slice(&counts);
    dht.extend_from_slice(&vals);
    seg(&mut o, 0xC4, &dht);
    if ri_rows > 0 {
        seg(&mut o, 0xDD, &((ri_rows * w) as u16).to_be_bytes());
    }
    let mut sos = vec![nc as u8];
    for c in 0..nc {
        sos.extend_from_slice(&[c as u8 + 1, 0]);
    }
    sos.extend_from_slice(&[pred, 0, 0]);
    seg(&mut o, 0xDA, &sos);
    let mut bw = Bits { o, acc: 0, n: 0 };
    let per_ri = ri_rows as usize * wu * nc;
    let mut rst = 0u8;
    for (i, &(cat, bits)) in syms.iter().enumerate() {
        if per_ri > 0 && i > 0 && i % per_ri == 0 {
            bw.pad();
            bw.o.extend_from_slice(&[0xFF, 0xD0 + rst]);
            rst = (rst + 1) & 7;
        }
        let (c, l) = codes[cat as usize];
        bw.put(c, l);
        if (1..16).contains(&cat) {
            bw.put(bits as u32, cat as u32);
        }
    }
    bw.pad();
    let mut o = bw.o;
    o.extend_from_slice(&[0xFF, 0xD9]);
    o
}

/// Huffman code lengths (at most 16 bits) for the used symbols of `freq`.
fn code_lengths(freq: &[u64; 18]) -> [u32; 18] {
    // Plain Huffman on (weight, symbols) groups; 18 symbols never need more than 16 bits
    // unless the counts are wildly skewed, which the fixtures are not.
    let mut groups: Vec<(u64, Vec<usize>)> = (0..18)
        .filter(|&s| freq[s] > 0)
        .map(|s| (freq[s], vec![s]))
        .collect();
    let mut lens = [0u32; 18];
    if groups.len() == 1 {
        lens[groups[0].1[0]] = 1;
        return lens;
    }
    while groups.len() > 1 {
        groups.sort_by_key(|g| std::cmp::Reverse(g.0));
        let (wa, a) = groups.pop().unwrap();
        let (wb, b) = groups.pop().unwrap();
        for &s in a.iter().chain(&b) {
            lens[s] += 1;
        }
        groups.push((wa + wb, a.into_iter().chain(b).collect()));
    }
    assert!(lens.iter().all(|&l| l <= 16), "code longer than 16 bits");
    lens
}

struct Bits {
    o: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, v: u32, l: u32) {
        self.acc = (self.acc << l) | (v as u64 & ((1u64 << l) - 1));
        self.n += l;
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.o.push(b);
            if b == 0xFF {
                self.o.push(0);
            }
            self.n -= 8;
        }
    }
    /// Pad to a byte with one bits.
    fn pad(&mut self) {
        let pad = (8 - self.n % 8) % 8;
        self.put((1 << pad) - 1, pad);
    }
}
