//! AVIF decode benchmark: specular-avif at 1/2/4/8, single-thread vs parallel, against
//! `ffmpeg -c:v libdav1d` (decode + swscale to rgb24, area downscale for 1/s).
//!
//! `cargo run --release -p specular-avif --example avif_bench [--features asm] -- [file.avif] [runs]`
//! Default file: target/laneb-bench/24mp-q75.avif.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use specular_avif::AvifFile;

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn ffmpeg(path: &str, scale: u32, threads: u32) -> Option<Duration> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-v",
        "error",
        "-threads",
        &threads.to_string(),
        "-c:v",
        "libdav1d",
    ])
    .args(["-i", path, "-frames:v", "1"]);
    if scale > 1 {
        cmd.args([
            "-vf",
            &format!("scale=iw/{scale}:ih/{scale}:flags=area"),
            "-threads",
            &threads.to_string(),
        ]);
    }
    cmd.args(["-pix_fmt", "rgb24", "-f", "null", "-"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let t = Instant::now();
    let ok = cmd.status().ok()?.success();
    ok.then(|| t.elapsed())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| "target/laneb-bench/24mp-q75.avif".to_owned());
    let runs: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));

    let t = Instant::now();
    let file = AvifFile::open(bytes).expect("open");
    let open = t.elapsed();
    let i = file.info();
    println!(
        "{path}: {}x{} {}-bit {} alpha={} grid={:?}  open {:.2} ms  asm={}",
        i.width,
        i.height,
        i.bit_depth,
        i.chroma,
        i.has_alpha,
        i.grid,
        ms(open),
        cfg!(feature = "asm"),
    );
    let cpus = std::thread::available_parallelism().map_or(1, |n| n.get()) as u32;
    println!("runs={runs} cpus={cpus}  (median ms)");
    println!("scale | specular 1T | specular par | ffmpeg 1T | ffmpeg par");
    for scale in [1u32, 2, 4, 8] {
        let mut row = Vec::new();
        for par in [false, true] {
            let _ = file.decode(scale, par).expect("decode"); // warm-up
            let times = (0..runs)
                .map(|_| {
                    let t = Instant::now();
                    let img = file.decode(scale, par).expect("decode");
                    std::hint::black_box(&img.data);
                    t.elapsed()
                })
                .collect();
            row.push(median(times));
        }
        let mut ff = Vec::new();
        for threads in [1, 0] {
            let times: Option<Vec<_>> = (0..runs.min(3))
                .map(|_| ffmpeg(&path, scale, threads))
                .collect();
            ff.push(times.map(median));
        }
        let f = |d: Option<Duration>| d.map_or("n/a".to_owned(), |d| format!("{:.1}", ms(d)));
        println!(
            "1/{scale}   | {:>10.1} | {:>11.1} | {:>9} | {:>10}",
            ms(row[0]),
            ms(row[1]),
            f(ff[0]),
            f(ff[1])
        );
    }
}
