//! Throughput of the decoders' CMYK path (`CmykConverter::convert_row`), in ns per pixel, and
//! of PSD Lab's (`LabConverter::convert_planes8`, single thread, noise planes).
//!
//! `cargo run --release -p specular-color --example cmyk_bench [profile.icc|-] [threads] [runs]`
//!
//! Two 2048×2048 images: "smooth" (gradients, as in photographs) and "noise" (random inks,
//! the worst case for the table's cache footprint). Prints the best of several runs, single
//! threaded and over `threads` threads (rows split in bands). The machine is usually shared,
//! so compare builds by alternating runs rather than trusting one number.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_color::{CmykConverter, Intent, LabConverter, Profile, profiles};
use std::time::Instant;

const W: usize = 2048;
const H: usize = 2048;

fn smooth() -> Vec<u8> {
    let mut v = Vec::with_capacity(W * H * 4);
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32 / W as f32, y as f32 / H as f32);
            let s = |a: f32| ((a.sin() * 0.5 + 0.5) * 255.0) as u8;
            v.extend_from_slice(&[
                (fx * 255.0) as u8,
                (fy * 255.0) as u8,
                s(fx * 7.0 + fy * 3.0),
                s(fx * 2.0 - fy * 5.0),
            ]);
        }
    }
    v
}

fn noise() -> Vec<u8> {
    let mut seed = 0x1234_5678u32;
    (0..W * H * 4)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 24) as u8
        })
        .collect()
}

fn run(conv: &CmykConverter, ink: &[u8], out: &mut [u8], threads: usize) -> f64 {
    let t0 = Instant::now();
    if threads <= 1 {
        for (i, o) in ink.chunks_exact(W * 4).zip(out.chunks_exact_mut(W * 4)) {
            conv.convert_row(i, o);
        }
    } else {
        let band = H.div_ceil(threads * 4) * W * 4;
        let work = std::sync::Mutex::new(ink.chunks(band).zip(out.chunks_mut(band)));
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    loop {
                        let Some((i, o)) = work.lock().unwrap().next() else {
                            break;
                        };
                        for (i, o) in i.chunks_exact(W * 4).zip(o.chunks_exact_mut(W * 4)) {
                            conv.convert_row(i, o);
                        }
                    }
                });
            }
        });
    }
    t0.elapsed().as_secs_f64() * 1e9 / (W * H) as f64
}

fn main() {
    let mut args = std::env::args().skip(1);
    let prof = args
        .next()
        .filter(|p| p != "-")
        .and_then(|p| Profile::parse(&std::fs::read(p).ok()?).ok())
        .unwrap_or_else(profiles::generic_cmyk);
    let threads: usize = args.next().and_then(|t| t.parse().ok()).unwrap_or(8);
    let runs: usize = args.next().and_then(|t| t.parse().ok()).unwrap_or(9);
    let t0 = Instant::now();
    let conv = CmykConverter::new(&prof, Intent::Relative, true).unwrap();
    let build = t0.elapsed().as_secs_f64() * 1e3;
    let mut out = vec![0u8; W * H * 4];
    let mut sum = 0u64;
    let mut line = format!("build {build:.1} ms");
    for (name, ink) in [("smooth", smooth()), ("noise", noise())] {
        for th in [1, threads] {
            let best = (0..runs)
                .map(|_| run(&conv, &ink, &mut out, th))
                .fold(f64::INFINITY, f64::min);
            line += &format!("  {name}/{th}t {best:.2}");
        }
        sum += out.iter().map(|&v| v as u64).sum::<u64>();
    }
    let lab = noise();
    let conv = LabConverter::default();
    let p = |c: usize| &lab[c * W * H..(c + 1) * W * H];
    let best = (0..runs)
        .map(|_| {
            let t0 = Instant::now();
            for y in 0..H {
                let r = |c: usize| &p(c)[y * W..(y + 1) * W];
                conv.convert_planes8(r(0), r(1), r(2), &mut out[y * W * 4..(y + 1) * W * 4]);
            }
            t0.elapsed().as_secs_f64() * 1e9 / (W * H) as f64
        })
        .fold(f64::INFINITY, f64::min);
    line += &format!("  lab/1t {best:.2}");
    sum += out.iter().map(|&v| v as u64).sum::<u64>();
    println!("{line}  (checksum {sum})");
}
