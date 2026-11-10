//! Baseline: upstream image-webp decode to RGB(A).
//! `cargo run --release -p specular-webp --example webp_bench_upstream -- FILE [RUNS]`
use std::io::Cursor;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("file");
    let runs: usize = args.next().map(|s| s.parse().unwrap()).unwrap_or(5);
    let data = std::fs::read(&path).unwrap();
    let mut times = Vec::new();
    for _ in 0..runs {
        let t = Instant::now();
        let mut d = image_webp::WebPDecoder::new(Cursor::new(&data)).unwrap();
        let mut buf = vec![0u8; d.output_buffer_size().unwrap()];
        d.read_image(&mut buf).unwrap();
        std::hint::black_box(&buf);
        times.push(t.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "upstream image-webp: median {:.1} ms (min {:.1})",
        times[runs / 2],
        times[0]
    );
}
