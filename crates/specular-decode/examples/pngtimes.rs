//! Registry-path timing for a PNG: `cargo run --release -p specular-decode --example pngtimes -- file.png`.
//!
//! Each run opens a fresh session (sessions cache decoded levels), so the times include
//! `read_info` and the conversion to a `Raster`. Best of 3, in megapixels (of the full image)
//! per second.
use specular_core::CancelToken;
use specular_decode::{DecodeRequest, Registry};
use std::path::Path;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("file");
    let reg = Registry::new();
    let s = reg.open_path(Path::new(&path)).expect("open");
    let (w, h) = (s.info().width, s.info().height);
    let mp = f64::from(w) * f64::from(h) / 1e6;
    println!("{path}: {w}x{h} {}", s.info().coding);
    drop(s);
    for (scale, parallel) in [(1, false), (1, true), (4, true), (8, true)] {
        let mut best = f64::MAX;
        let mut out = (0, 0);
        for _ in 0..3 {
            let t = Instant::now();
            let s = reg.open_path(Path::new(&path)).expect("open");
            let req = DecodeRequest {
                parallel,
                ..DecodeRequest::full(scale)
            };
            let r = s.decode(&req, &CancelToken::new()).expect("decode");
            best = best.min(t.elapsed().as_secs_f64());
            out = (r.width, r.height);
        }
        println!(
            "registry 1/{scale} {:<8} {:>7.1} ms {:>6.0} MP/s  -> {}x{}",
            if parallel { "parallel" } else { "single" },
            best * 1e3,
            mp / best,
            out.0,
            out.1
        );
    }
}
