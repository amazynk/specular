//! Decode timings: `cargo run -p specular-legacy2 --example time -- FILE...` (best of 5).

use specular_legacy2::{miff, tim, xwd};

fn main() {
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).expect("read");
        let decode: fn(&[u8], u32) -> _ = match path.rsplit('.').next() {
            Some("miff") => miff::decode,
            Some("xwd") => xwd::decode,
            _ => tim::decode,
        };
        let mut best = f64::MAX;
        let mut size = (0, 0);
        for _ in 0..5 {
            let t = std::time::Instant::now();
            let img = decode(&data, 0).expect("decode");
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
            size = (img.width, img.height);
        }
        let mp = size.0 as f64 * size.1 as f64 / 1e6;
        println!(
            "{path}: {}x{} in {best:.1} ms ({:.0} MP/s)",
            size.0,
            size.1,
            mp / best * 1e3
        );
    }
}
