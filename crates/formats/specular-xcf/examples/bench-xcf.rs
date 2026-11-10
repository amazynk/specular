//! Composite timings on XCF files (default: the corpus' generated 2400×1600, six layers with
//! a masked group, multiply, screen, overlay and soft light).
//!
//! `cargo run --release -p specular-xcf --example bench-xcf [-- FILE... ] [RUNS=N]`

use std::path::PathBuf;
use std::time::{Duration, Instant};

use specular_xcf::{Options, decompress, parse};

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
    let mut runs = 5;
    let mut files = Vec::new();
    for a in std::env::args().skip(1) {
        match a.strip_prefix("RUNS=") {
            Some(n) => runs = n.parse().expect("RUNS=N"),
            None => files.push(PathBuf::from(a)),
        }
    }
    if files.is_empty() {
        let root = std::env::var_os("SPECULAR_LARGE_CORPUS")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("tools/corpus/large"));
        files.push(root.join("xcf/gen/bench-2400x1600.xcf"));
    }
    println!("best of {runs}; threads = {}", rayon::current_num_threads());
    println!(
        "{:<28} {:>7} {:>7} {:>11} {:>11} {:>11} {:>11}",
        "file", "MP", "layers", "unpack", "parse", "render par", "render seq"
    );
    for f in files {
        let raw = std::fs::read(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let t_unpack = best(runs, || decompress(&raw).unwrap());
        let data = decompress(&raw).unwrap().unwrap_or(raw);
        let t_parse = best(runs, || parse(&data).unwrap());
        let x = parse(&data).unwrap();
        let par = Options {
            parallel: true,
            cancel: None,
        };
        let seq = Options::default();
        let t_par = best(runs, || x.render(&data, &par).unwrap());
        let t_seq = best(runs.min(3), || x.render(&data, &seq).unwrap());
        let info = x.info();
        println!(
            "{:<28} {:>7.2} {:>7} {} {} {} {}",
            f.file_name().unwrap().to_string_lossy(),
            f64::from(info.width) * f64::from(info.height) / 1e6,
            info.layer_count + info.group_count,
            ms(t_unpack),
            ms(t_parse),
            ms(t_par),
            ms(t_seq)
        );
    }
}
