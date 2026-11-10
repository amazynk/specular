//! Time and peak memory of one whole level of an HDR float file (Radiance, PFM, OpenEXR)
//! through the registry, as the viewer opens it (mmap'd `Source`).
//!
//! `cargo run --release -p specular-decode --example hdrlevels -- <file> <scale> [runs]`
//! Run one level per process: peak RSS is per process. Scale 0 times the format crate's whole
//! `f32` decode instead (the old starting point).

use std::sync::Arc;
use std::time::Instant;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeRequest, Registry};

fn status_mb(key: &str) -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        })
        .unwrap_or(0.0)
        / 1024.0
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: hdrlevels <file> <scale> [runs]");
    let scale: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let runs: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);
    if scale == 0 {
        // Baseline: the format crate's whole `f32` decode (what a full-size level used to
        // start from, before its conversion to half floats).
        let src = Source::open(&path).unwrap();
        let mut times = Vec::new();
        for _ in 0..runs {
            let t = Instant::now();
            let n = if path.ends_with(".exr") {
                let img =
                    specular_exr::decode(src.bytes(), &specular_exr::DecodeOptions::default())
                        .unwrap();
                img.width as usize * img.height as usize
            } else {
                let opts = specular_radiance::DecodeOptions {
                    channels: specular_radiance::OutputChannels::Rgba,
                    ..Default::default()
                };
                specular_radiance::decode(src.bytes(), &opts)
                    .unwrap()
                    .data
                    .len()
                    / 4
            };
            times.push(t.elapsed().as_secs_f64() * 1e3);
            assert!(n > 0);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "whole f32 decode: median {:.0} ms (min {:.0}), peak RSS {:.0} MiB",
            times[times.len() / 2],
            times[0],
            status_mb("VmHWM:")
        );
        return;
    }
    let t = Instant::now();
    let s = Registry::new()
        .open(Arc::new(Source::open(&path).unwrap()))
        .unwrap();
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    let req = DecodeRequest::full(scale);
    let est = s.decode_memory(&req) as f64 / (1 << 20) as f64;
    let mut times = Vec::new();
    for _ in 0..runs {
        let t = Instant::now();
        let r = s.decode(&req, &CancelToken::new()).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(r.scale, scale);
    }
    times.sort_by(f64::total_cmp);
    let i = s.info();
    println!(
        "{}x{} 1/{scale}: open {open_ms:.1} ms, decode median {:.0} ms (min {:.0}), \
         estimate {est:.0} MiB, peak RSS {:.0} MiB (file mapped: {} MiB)",
        i.width,
        i.height,
        times[times.len() / 2],
        times[0],
        status_mb("VmHWM:"),
        std::fs::metadata(&path).map_or(0, |m| m.len() >> 20),
    );
}
