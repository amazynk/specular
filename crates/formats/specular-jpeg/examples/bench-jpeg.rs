//! Benchmark: `cargo run --release -p specular-jpeg --example bench-jpeg -- FILE...`
//!
//! For every file: read_info, decode at each scale (Auto and Single), build_index, a 512x512
//! full-resolution region decode through the index, zune-jpeg full decode and a `djpeg -scale`
//! subprocess (if djpeg is installed). Best of N runs (default 5, `BENCH_RUNS`), warm cache.

use std::time::{Duration, Instant};

use specular_jpeg::*;

fn best<T>(runs: usize, mut f: impl FnMut() -> T) -> Duration {
    let _ = f(); // warm-up
    (0..runs)
        .map(|_| {
            let t = Instant::now();
            std::hint::black_box(f());
            t.elapsed()
        })
        .min()
        .unwrap()
}

/// Files this decoder rejects (arithmetic coding, corrupt headers, ...) are reported and
/// skipped instead of aborting the run.
fn decodable(path: &str, data: &[u8]) -> bool {
    match read_info(data).and_then(|_| decode(data, &DecodeOptions::default())) {
        Ok(_) => true,
        Err(e) => {
            println!("skip {path}: {e}");
            false
        }
    }
}

fn ms(d: Duration) -> String {
    format!("{:8.1}", d.as_secs_f64() * 1e3)
}

fn main() {
    let runs: usize = std::env::var("BENCH_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let skip_ext = std::env::var_os("BENCH_NO_EXTERNAL").is_some();
    let have_djpeg = !skip_ext
        && std::process::Command::new("djpeg")
            .arg("-version")
            .output()
            .is_ok();
    // Memory-measurement modes (run under `/usr/bin/time -v`): one operation per file.
    // `full`: one Full decode; `index`: build_index only; `striped`: build_index, then a
    // Full decode in 256-row stripes through the index, dropping each stripe.
    if let Ok(mode) = std::env::var("BENCH_MODE") {
        if mode == "time" {
            // `time`: N decodes per file (after a warm-up), min and median.
            // `BENCH_THREADS=n`: decode with n threads (1: on the calling thread).
            let parallelism = std::env::var("BENCH_THREADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .map_or(Parallelism::Auto, Parallelism::Threads);
            // `BENCH_SCALE=1|2|4|8`: decode at that scale (default 1).
            let scale = match std::env::var("BENCH_SCALE").as_deref() {
                Ok("2") => Scale::Half,
                Ok("4") => Scale::Quarter,
                Ok("8") => Scale::Eighth,
                _ => Scale::Full,
            };
            for path in std::env::args().skip(1) {
                let data = match std::fs::read(&path) {
                    Ok(d) => d,
                    Err(e) => {
                        println!("skip {path}: {e}");
                        continue;
                    }
                };
                // Headers only: a trial decode would set the peak RSS this mode reports
                // (`/usr/bin/time -v`) before the measured decodes run.
                if let Err(e) = read_info(&data) {
                    println!("skip {path}: {e}");
                    continue;
                }
                // `BENCH_REGION=WxH`: decode that region at the centre (output pixels).
                // `BENCH_INDEX=1` (implied by a region): decode with a prebuilt row index.
                let region = std::env::var("BENCH_REGION").ok().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
                    let i = read_info(&data).ok()?;
                    let (ow, oh) = (
                        i.width.div_ceil(scale.factor()),
                        i.height.div_ceil(scale.factor()),
                    );
                    let (w, h) = (w.min(ow), h.min(oh));
                    Some(Region {
                        x: (ow - w) / 2,
                        y: (oh - h) / 2,
                        w,
                        h,
                    })
                });
                let t_ix = Instant::now();
                let ix = (region.is_some() || std::env::var_os("BENCH_INDEX").is_some())
                    .then(|| build_index(&data).unwrap());
                let t_ix = ix.as_ref().map(|_| t_ix.elapsed());
                let o = DecodeOptions {
                    parallelism,
                    scale,
                    region,
                    ..Default::default()
                };
                let run = || match &ix {
                    Some(ix) => decode_with_index(&data, ix, &o).unwrap(),
                    None => decode(&data, &o).unwrap(),
                };
                std::hint::black_box(run());
                let mut t: Vec<Duration> = (0..runs.max(1))
                    .map(|_| {
                        let s = Instant::now();
                        std::hint::black_box(run());
                        s.elapsed()
                    })
                    .collect();
                t.sort();
                println!(
                    "time {path}: min {} ms median {} ms{}",
                    ms(t[0]),
                    ms(t[t.len() / 2]),
                    t_ix.map_or(String::new(), |d| format!(" (index built in {} ms)", ms(d)))
                );
            }
            return;
        }
        for path in std::env::args().skip(1) {
            let data = match std::fs::read(&path) {
                Ok(d) => d,
                Err(e) => {
                    println!("skip {path}: {e}");
                    continue;
                }
            };
            if !decodable(&path, &data) {
                continue;
            }
            let o = DecodeOptions::default();
            let t = Instant::now();
            match mode.as_str() {
                "full" => {
                    std::hint::black_box(decode(&data, &o).unwrap());
                }
                "index" => {
                    if let Err(e) = std::hint::black_box(build_index(&data)) {
                        println!("skip {path}: build_index: {e}");
                        continue;
                    }
                }
                "striped" => {
                    let info = read_info(&data).unwrap();
                    let ix = match build_index(&data) {
                        Ok(ix) => ix,
                        Err(e) => {
                            println!("skip {path}: build_index: {e}");
                            continue;
                        }
                    };
                    let mut y = 0;
                    while y < info.height {
                        let h = 256.min(info.height - y);
                        let r = Region {
                            x: 0,
                            y,
                            w: info.width,
                            h,
                        };
                        let ro = DecodeOptions {
                            region: Some(r),
                            ..o
                        };
                        std::hint::black_box(decode_with_index(&data, &ix, &ro).unwrap());
                        y += h;
                    }
                }
                m => panic!("unknown BENCH_MODE {m}"),
            }
            println!("{mode} {path}: {} ms", ms(t.elapsed()));
        }
        return;
    }
    println!("threads available: {}", rayon::current_num_threads());
    for path in std::env::args().skip(1) {
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                println!("skip {path}: {e}");
                continue;
            }
        };
        if !decodable(&path, &data) {
            continue;
        }
        let info = read_info(&data).expect("read_info");
        println!(
            "\n== {path}\n   {}x{} ({:.1} MP), {:?}, {} comps {}x{} max, restart {}, {:.1} MB",
            info.width,
            info.height,
            info.width as f64 * info.height as f64 / 1e6,
            info.coding,
            info.components.len(),
            info.max_h,
            info.max_v,
            info.restart_interval,
            data.len() as f64 / 1e6
        );
        println!(
            "   read_info               {} ms",
            ms(best(runs, || read_info(&data).unwrap()))
        );
        println!(
            "   {:<8} {:>10} {:>10} {:>10}",
            "scale", "auto ms", "single ms", "djpeg ms"
        );
        for scale in [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth] {
            let o = |parallelism| DecodeOptions {
                scale,
                region: None,
                cancel: None,
                parallelism,
            };
            let auto = best(runs, || decode(&data, &o(Parallelism::Auto)).unwrap());
            let single = best(runs, || decode(&data, &o(Parallelism::Single)).unwrap());
            let dj = if have_djpeg {
                let frac = format!("1/{}", scale as u32);
                let t = best(runs.min(3), || {
                    let st = std::process::Command::new("djpeg")
                        .args(["-dct", "int", "-scale", &frac, "-outfile", "/dev/null"])
                        .arg(&path)
                        .status()
                        .unwrap();
                    assert!(st.success());
                });
                ms(t)
            } else {
                "-".into()
            };
            println!(
                "   {:<8} {} {} {}",
                format!("{scale:?}"),
                ms(auto),
                ms(single),
                dj
            );
        }
        let ix = build_index(&data);
        match ix {
            Ok(ix) => {
                println!(
                    "   build_index (auto)      {} ms   ({} checkpoints, {} KB)",
                    ms(best(runs, || build_index(&data).unwrap())),
                    ix.len(),
                    ix.memory_bytes() / 1024
                );
                let r = Region {
                    x: info.width / 2 - 256.min(info.width / 2),
                    y: info.height / 2 - 256.min(info.height / 2),
                    w: 512,
                    h: 512,
                };
                for par in [Parallelism::Auto, Parallelism::Single] {
                    let o = DecodeOptions {
                        scale: Scale::Full,
                        region: Some(r),
                        cancel: None,
                        parallelism: par,
                    };
                    let t = best(runs, || decode_with_index(&data, &ix, &o).unwrap());
                    let t2 = best(runs, || decode(&data, &o).unwrap());
                    println!(
                        "   region 512² Full {par:?}: with index {} ms, without {} ms",
                        ms(t),
                        ms(t2)
                    );
                }
            }
            Err(e) => println!("   build_index: {e}"),
        }
        let dz = best(runs, || {
            let mut d = zune_jpeg::JpegDecoder::new_with_options(
                zune_core::bytestream::ZCursor::new(&data[..]),
                zune_core::options::DecoderOptions::default()
                    .set_max_width(1 << 16)
                    .set_max_height(1 << 16),
            );
            d.decode().map(|v| v.len()).unwrap_or(0)
        });
        let full_rgba = best(runs, || {
            let d = decode(&data, &DecodeOptions::default()).unwrap();
            planes_to_rgba(&d)
        });
        println!(
            "   zune-jpeg full RGB      {} ms   (ours full + planes_to_rgba: {} ms)",
            ms(dz),
            ms(full_rgba)
        );
    }
}
