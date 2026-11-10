//! `jxr <file>`: prints the headers. `jxr <file> <out>`: writes the native samples
//! (little-endian, interleaved, no header) and prints `width height channels`.
//! `jxr <file> --bench N`: decodes N times and prints the best wall-clock time.

use std::time::Instant;

use specular_jxr::{DecodeOptions, Samples, decode, decode_native, read_info};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: jxr <file> [<out.raw> | --bench N]");
        std::process::exit(2);
    };
    let data = std::fs::read(path).expect("read");
    let info = read_info(&data).unwrap_or_else(|e| {
        eprintln!("{path}: {e}");
        std::process::exit(1)
    });
    match args.get(2).map(String::as_str) {
        None => println!("{info:#?}"),
        Some("--bench") => {
            let n: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);
            let mut best = f64::MAX;
            for _ in 0..n {
                let t = Instant::now();
                let img = decode(&data, &DecodeOptions::default()).expect("decode");
                std::hint::black_box(&img);
                best = best.min(t.elapsed().as_secs_f64());
            }
            let mp = info.width as f64 * info.height as f64 / 1e6;
            println!(
                "{mp:.1} MP: best of {n} {:.1} ms ({:.0} MP/s)",
                best * 1e3,
                mp / best
            );
        }
        Some(out) => {
            let n = decode_native(&data, &DecodeOptions::default()).unwrap_or_else(|e| {
                eprintln!("{path}: {e}");
                std::process::exit(1)
            });
            let bytes: Vec<u8> = match &n.samples {
                Samples::U8(v) => v.clone(),
                Samples::U16(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
                Samples::U32(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            };
            std::fs::write(out, bytes).expect("write");
            println!(
                "{} {} {}{}",
                n.width,
                n.height,
                n.channels,
                if n.truncated { " truncated" } else { "" }
            );
        }
    }
}
