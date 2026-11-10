//! JPEG-LS decoding for oracle comparisons and timing.
//!
//! `jls dump OUT_DIR FILE...` writes `NAME.raw` (the planes as little-endian u16, in component
//! order) and `NAME.txt` (`ok W H N truncated` or `err MESSAGE`) per file.
//! `jls time FILE...` prints the best of 15 decode times and the throughput per file.

use std::path::Path;
use std::time::Instant;

use specular_jpegls::{DecodeOptions, decode};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("dump") if args.len() >= 2 => {
            let out = Path::new(&args[1]);
            std::fs::create_dir_all(out).expect("output directory");
            for f in &args[2..] {
                let data = std::fs::read(f).expect("read");
                let name = Path::new(f)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let report = match decode(&data, &DecodeOptions::default()) {
                    Ok(img) => {
                        let mut raw = Vec::new();
                        for p in &img.planes {
                            for v in &p.data {
                                raw.extend_from_slice(&v.to_le_bytes());
                            }
                        }
                        std::fs::write(out.join(format!("{name}.raw")), raw).expect("write");
                        format!(
                            "ok {} {} {} {}\n",
                            img.width,
                            img.height,
                            img.planes.len(),
                            img.truncated
                        )
                    }
                    Err(e) => format!("err {e}\n"),
                };
                std::fs::write(out.join(format!("{name}.txt")), report).expect("write");
            }
        }
        Some("time") => {
            for f in &args[1..] {
                let data = std::fs::read(f).expect("read");
                let mut best = f64::MAX;
                let mut samples = 0u64;
                for _ in 0..15 {
                    let t = Instant::now();
                    let img = decode(&data, &DecodeOptions::default()).expect("decode");
                    best = best.min(t.elapsed().as_secs_f64());
                    samples = img.planes.iter().map(|p| p.data.len() as u64).sum();
                    std::hint::black_box(&img);
                }
                println!(
                    "{f}: {:.3} ms, {:.1} Msamples/s, {:.1} MB/s compressed",
                    best * 1e3,
                    samples as f64 / best / 1e6,
                    data.len() as f64 / best / 1e6
                );
            }
        }
        _ => {
            eprintln!("usage: jls dump OUT_DIR FILE... | jls time FILE...");
            std::process::exit(2);
        }
    }
}
