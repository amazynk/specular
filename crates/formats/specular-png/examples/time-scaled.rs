//! Wall time of one scaled decode, single-threaded and threaded.
//!
//! `cargo run --release -p specular-png --example time-scaled -- <file.png> [scale]`

use std::time::Instant;

use specular_png::{DecodeOptions, Parallelism, decode};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("file");
    let scale: u32 = args.next().map_or(8, |s| s.parse().expect("scale"));
    let data = std::fs::read(&path).expect("read");
    for par in [Parallelism::Single, Parallelism::Auto] {
        let opts = DecodeOptions {
            scale,
            parallelism: par,
            ..DecodeOptions::default()
        };
        let t = Instant::now();
        let img = decode(&data, &opts).expect("decode");
        println!(
            "{par:?} 1/{scale}: {}x{} in {:.0} ms",
            img.width,
            img.height,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
}
