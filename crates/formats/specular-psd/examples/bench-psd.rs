//! Decode timings on generated PSDs (≤ 12 MP) against the `psd` crate, and of one
//! canvas-sized RGBA layer stored with the same compression (planes, then to 8-bit RGBA).
//!
//! `cargo run --release -p specular-psd --example bench-psd [-- WIDTH HEIGHT RUNS]`

#[path = "../tests/common/mod.rs"]
mod common;

use std::time::{Duration, Instant};

use common::*;
use specular_psd::Options;

fn best<T>(runs: usize, mut f: impl FnMut() -> T) -> Duration {
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(f());
            t.elapsed()
        })
        .min()
        .unwrap_or_default()
}

fn ms(d: Duration) -> String {
    format!("{:8.1} ms", d.as_secs_f64() * 1e3)
}

fn main() {
    let a: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    let (w, h, runs) = (
        a.first().copied().unwrap_or(4000),
        a.get(1).copied().unwrap_or(3000),
        a.get(2).copied().unwrap_or(5) as usize,
    );
    assert!(
        w as u64 * h as u64 <= 12_000_000,
        "keep test images ≤ 12 MP"
    );
    println!(
        "{w}×{h} RGB ({:.1} MP), best of {runs}; threads = {}",
        (w * h) as f64 / 1e6,
        rayon::current_num_threads()
    );
    println!(
        "{:<18} {:>9} {:>11} {:>11} {:>11} {:>11} {:>11}",
        "file", "size", "rgba8 par", "rgba8 seq", "planes", "layer", "psd crate"
    );
    for (depth, comp, name) in [
        (8u16, 0u16, "8-bit raw"),
        (8, 1, "8-bit RLE"),
        (8, 2, "8-bit ZIP"),
        (8, 3, "8-bit ZIP+pred"),
        (16, 1, "16-bit RLE"),
        (16, 3, "16-bit ZIP+pred"),
    ] {
        let bps = depth as usize / 8;
        let planes: Vec<Vec<u8>> = (0..3).map(|c| pattern(w, h, c, bps)).collect();
        let mut channels: Vec<(i16, Vec<u8>)> =
            (0..3).map(|c| (c as i16, planes[c].clone())).collect();
        channels.push((-1, pattern(w, h, 7, bps)));
        let mut spec = Spec::new(w, h, depth, 3, planes);
        spec.compression = comp;
        spec.layer_count = 1;
        spec.layers = vec![LayerSpec {
            name: "layer".into(),
            bounds: [0, 0, h as i32, w as i32],
            opacity: 255,
            compression: comp,
            channels,
            ..Default::default()
        }];
        let file = write(&spec);
        drop(spec);
        let psd = specular_psd::parse(&file).expect("parse");
        let par = Options { parallel: true };
        let seq = Options { parallel: false };
        let t_par = best(runs, || psd.decode_rgba8(&file, &par).unwrap());
        let t_seq = best(runs, || psd.decode_rgba8(&file, &seq).unwrap());
        let t_planes = best(runs, || psd.decode_planes(&file, &par).unwrap());
        let layer = psd.pixel_layers()[0];
        let t_layer = best(runs, || {
            psd.decode_layer_planes(&file, layer, &par, None)
                .unwrap()
                .to_rgba8()
        });
        // The oracle only handles raw/RLE (it panics on ZIP); guard it anyway.
        let oracle = if comp <= 1 {
            std::panic::catch_unwind(|| {
                best(runs.min(3), || psd::Psd::from_bytes(&file).unwrap().rgba())
            })
            .map(ms)
            .unwrap_or_else(|_| "panicked".into())
        } else {
            "n/a".into()
        };
        println!(
            "{name:<18} {:>6.1} MB {} {} {} {} {:>11}",
            file.len() as f64 / 1e6,
            ms(t_par),
            ms(t_seq),
            ms(t_planes),
            ms(t_layer),
            oracle
        );
    }
}
