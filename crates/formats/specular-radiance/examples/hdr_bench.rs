//! Decode timings for a generated Radiance .hdr (new RLE and flat) and a PFM.
//!
//! `cargo run --release -p specular-radiance --example hdr_bench [-- WIDTH HEIGHT]` (in memory only)

use std::time::{Duration, Instant};

use specular_radiance::{DecodeOptions, OutputChannels, decode};

fn rgbe(rgb: [f32; 3]) -> [u8; 4] {
    let v = rgb[0].max(rgb[1]).max(rgb[2]);
    if v < 1e-32 {
        return [0; 4];
    }
    let e = v.log2().floor() as i32 + 1;
    let s = 256.0 / (e as f32).exp2();
    [
        (rgb[0] * s) as u8,
        (rgb[1] * s) as u8,
        (rgb[2] * s) as u8,
        (e + 128) as u8,
    ]
}

fn rle_line(line: &[[u8; 4]], out: &mut Vec<u8>) {
    let n = line.len();
    out.extend_from_slice(&[2, 2, (n >> 8) as u8, n as u8]);
    for c in 0..4 {
        let comp: Vec<u8> = line.iter().map(|p| p[c]).collect();
        let mut i = 0;
        while i < n {
            let mut run = 1;
            while i + run < n && comp[i + run] == comp[i] && run < 127 {
                run += 1;
            }
            if run >= 3 {
                out.extend_from_slice(&[128 + run as u8, comp[i]]);
                i += run;
                continue;
            }
            let start = i;
            while i < n && i - start < 128 {
                if i + 2 < n && comp[i] == comp[i + 1] && comp[i] == comp[i + 2] {
                    break;
                }
                i += 1;
            }
            out.push((i - start) as u8);
            out.extend_from_slice(&comp[start..i]);
        }
    }
}

fn pixel(x: usize, y: usize, w: usize, h: usize) -> [f32; 3] {
    let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
    let d = ((u - 0.3).powi(2) + (v - 0.2).powi(2)).sqrt();
    let sun = 2000.0 / (1.0 + 4000.0 * d * d);
    let n = ((x * 7919 + y * 104729) % 97) as f32 * 0.002;
    [
        0.3 + u + sun + n,
        0.5 + 0.5 * v + sun * 0.9 + n,
        1.2 - 0.4 * v + sun * 0.7 + n,
    ]
}

fn time(runs: usize, mut f: impl FnMut()) -> Duration {
    f();
    let mut v: Vec<Duration> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    v.sort();
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let w: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4000);
    let h: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let mut rle =
        format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\nEXPOSURE=1\n\n-Y {h} +X {w}\n").into_bytes();
    let mut flat = rle.clone();
    let mut pfm = format!("PF\n{w} {h}\n-1.0\n").into_bytes();
    let mut line = Vec::with_capacity(w);
    for y in 0..h {
        line.clear();
        line.extend((0..w).map(|x| rgbe(pixel(x, y, w, h))));
        rle_line(&line, &mut rle);
        line.iter().for_each(|p| flat.extend_from_slice(p));
    }
    for y in (0..h).rev() {
        for x in 0..w {
            pixel(x, y, w, h)
                .iter()
                .for_each(|v| pfm.extend_from_slice(&v.to_le_bytes()));
        }
    }
    println!(
        "{w}x{h} ({:.1} MP), {} threads",
        (w * h) as f64 / 1e6,
        rayon::current_num_threads()
    );
    for (name, bytes) in [("hdr-rle", &rle), ("hdr-flat", &flat), ("pfm-rgb", &pfm)] {
        for (label, ch, par) in [
            ("rgba", OutputChannels::Rgba, true),
            ("rgb", OutputChannels::Rgb, true),
            ("rgba 1 thread", OutputChannels::Rgba, false),
        ] {
            let o = DecodeOptions {
                channels: ch,
                parallel: par,
                ..Default::default()
            };
            let t = time(7, || {
                let img = decode(bytes, &o).expect("decode");
                assert!(!img.truncated);
                std::hint::black_box(img);
            });
            println!(
                "{name:<9} {:>6.1} MB  {label:<14} {:>8.1} ms  {:>6.0} MP/s",
                bytes.len() as f64 / 1e6,
                t.as_secs_f64() * 1e3,
                (w * h) as f64 / 1e6 / t.as_secs_f64()
            );
        }
    }
}
