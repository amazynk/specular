//! Per-file timings: header parse + preview location, preview decode, sensor decode (full and
//! half size). Usage:
//!
//! ```text
//! cargo run --release -p specular-raw --example bench-raw -- [FILES or DIRS...]
//! ```
//!
//! Without arguments it reads `target/agent-jpeg/raw-samples/`. With `SPECULAR_RAW_DUMP=<dir>`
//! the developed images are written there as 8-bit sRGB PPMs for eyeballing. With
//! `SPECULAR_RAW_CFA=<dir>` the undeveloped sensor data is written as `<name>.cfa` (8 x u32 LE
//! header: width, height, crop x, y, w, h, cpp, white; then u16 LE samples) for oracle comparison.
//! With `SPECULAR_RAW_PROFILE=<n>` it only runs the single-threaded sensor decode `n` times per
//! file and prints the best time and throughput (for `perf record`). With
//! `SPECULAR_RAW_REGION=<n>` it times a 1:1 1920x1080 region against the full develop.

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_raw::{DecodeOptions, OutputSize, RgbImage};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn best<T>(n: usize, mut f: impl FnMut() -> T) -> (T, f64) {
    let mut best = f64::INFINITY;
    let mut out = None;
    for _ in 0..n {
        let t = Instant::now();
        let v = f();
        best = best.min(ms(t));
        out = Some(v);
    }
    (out.expect("n > 0"), best)
}

fn srgb8(v: u16) -> u8 {
    let x = v as f32 / 65535.0;
    let y = if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (y * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

fn dump(dir: &Path, name: &str, img: &RgbImage) {
    let mut buf = format!("P6\n{} {}\n255\n", img.width, img.height).into_bytes();
    buf.extend(img.data.iter().map(|&v| srgb8(v)));
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join(format!("{name}.ppm")), buf);
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// `SPECULAR_RAW_REGION=<n>`: time to a 1:1 view of a centred 1920x1080 region, `n`
/// interleaved runs each of the full decode + develop (A) and the region decode from a cold
/// tile cache (B), then panning by 256 px steps with a warm cache (C).
fn bench_region(file: &specular_raw::RawFile, data: &[u8], n: usize) {
    use specular_raw::{Rect, TileCache};
    let Some(s) = file.info().sensor.as_ref() else {
        return;
    };
    let (w, h) = (s.crop.w, s.crop.h);
    let (rw, rh) = (1920.min(w), 1080.min(h));
    let r = Rect {
        x: (w - rw) / 2,
        y: (h - rh) / 2,
        w: rw,
        h: rh,
    };
    let full = DecodeOptions {
        size: OutputSize::Full,
        ..Default::default()
    };
    let (mut ta, mut tb) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let t = Instant::now();
        let img = file.decode(data, &full).expect("full decode");
        ta.push(ms(t));
        drop(img);
        let cache = TileCache::new(128 << 20);
        let o = DecodeOptions {
            region: Some(r),
            tiles: Some(&cache),
            ..full
        };
        let t = Instant::now();
        let img = file.decode(data, &o).expect("region decode");
        tb.push(ms(t));
        assert_eq!((img.width, img.height), (rw, rh));
    }
    // Panning right with a warm cache.
    let cache = TileCache::new(128 << 20);
    let mut tc = Vec::new();
    for i in 0..n.max(4) {
        let x = (r.x + 256 * i as u32).min(w - rw);
        let o = DecodeOptions {
            region: Some(Rect { x, ..r }),
            tiles: Some(&cache),
            ..full
        };
        let t = Instant::now();
        file.decode(data, &o).expect("region decode");
        if i > 0 {
            tc.push(ms(t));
        }
    }
    let (a, b, c) = (median(&mut ta), median(&mut tb), median(&mut tc));
    println!(
        "    region {rw}x{rh} at {},{} (decodable {}): full decode+develop median {a:.1} ms (min {:.1}), region cold {b:.1} ms (min {:.1}, {:.1}x), pan step warm {c:.1} ms; tiles held {} ({} MiB)",
        r.x,
        r.y,
        file.region_decodable(),
        ta[0],
        tb[0],
        a / b,
        cache.len(),
        cache.memory_bytes() >> 20
    );
}

fn main() {
    let mut files: Vec<PathBuf> = Vec::new();
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let roots = if args.is_empty() {
        vec![PathBuf::from("target/agent-jpeg/raw-samples")]
    } else {
        args
    };
    for r in roots {
        if r.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&r)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            v.sort();
            files.extend(v.into_iter().filter(|p| p.is_file()));
        } else {
            files.push(r);
        }
    }
    let dump_dir = std::env::var_os("SPECULAR_RAW_DUMP").map(PathBuf::from);
    let profile_runs = std::env::var("SPECULAR_RAW_PROFILE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0);
    let region_runs = std::env::var("SPECULAR_RAW_REGION")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0);
    for path in files {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (parsed, t_parse) = best(20, || specular_raw::parse(&data));
        let file = match parsed {
            Ok(f) => f,
            Err(e) => {
                println!("{name}: parse error: {e}");
                continue;
            }
        };
        let info = file.info();
        println!(
            "{name}: {} | {} | orient {} | ISO {:?} | t {:?} | f {:?} | parse {t_parse:.3} ms",
            info.format.map_or("?", |f| f.name()),
            info.camera_name(),
            info.orientation,
            info.iso,
            info.exposure_time,
            info.f_number
        );
        for p in &info.previews {
            println!(
                "    preview {:?} {}x{} @{}+{} ({})",
                p.kind,
                p.width,
                p.height,
                p.range.start,
                p.range.len(),
                p.source
            );
        }
        if let Some(p) = info.largest_preview().filter(|_| profile_runs.is_none()) {
            let (r, t) = best(3, || specular_raw::decode_preview(&data, p, 0));
            match r {
                Ok(img) => println!(
                    "    largest preview decode {}x{}: {t:.1} ms",
                    img.width, img.height
                ),
                Err(e) => println!("    largest preview decode failed: {e}"),
            }
            let (r, t) = best(3, || specular_raw::decode_preview(&data, p, 1024));
            if let Ok(img) = r {
                println!(
                    "    preview decode for 1024 px ({}x{}): {t:.1} ms",
                    img.width, img.height
                );
            }
        }
        if let Some(s) = &info.sensor {
            println!(
                "    sensor {}x{} crop {:?} cpp {} bits {} {:?} cfa {} decodable {} {}",
                s.width,
                s.height,
                s.crop,
                s.samples_per_pixel,
                s.bits,
                s.compression,
                s.cfa.as_ref().map(|c| c.name()).unwrap_or_default(),
                s.decodable,
                s.unsupported_reason.unwrap_or("")
            );
        }
        if !file.can_decode_sensor() {
            continue;
        }
        if let Some(n) = region_runs {
            bench_region(&file, &data, n);
            continue;
        }
        if let Some(n) = profile_runs {
            let st = DecodeOptions {
                parallel: false,
                ..Default::default()
            };
            let (r, t) = best(n, || file.decode_raw(&data, &st));
            if let Ok(r) = r {
                let mp = r.width as f64 * r.height as f64 / 1e6;
                println!(
                    "    decode_raw single-thread {t:.1} ms, {:.0} MP/s",
                    mp / t * 1e3
                );
            }
            continue;
        }
        let opts = DecodeOptions::default();
        let (raw, t_raw) = best(3, || file.decode_raw(&data, &opts));
        let raw = match raw {
            Ok(r) => r,
            Err(e) => {
                println!("    decode_raw failed: {e}");
                continue;
            }
        };
        let st = DecodeOptions {
            parallel: false,
            ..opts
        };
        let (_, t_st) = best(2, || file.decode_raw(&data, &st));
        println!("    decode_raw single-thread {t_st:.1} ms");
        if let Some(dir) = std::env::var_os("SPECULAR_RAW_CFA").map(PathBuf::from) {
            let c = raw.crop;
            let hdr = [
                raw.width,
                raw.height,
                c.x,
                c.y,
                c.w,
                c.h,
                raw.cpp,
                raw.white as u32,
            ];
            let mut buf: Vec<u8> = hdr.iter().flat_map(|v| v.to_le_bytes()).collect();
            buf.extend(raw.data.iter().flat_map(|v| v.to_le_bytes()));
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(format!("{name}.cfa")), buf);
        }
        println!(
            "    decode_raw {t_raw:.1} ms | black {:?} white {} wb {:?} matrix {:?} truncated {}",
            raw.black, raw.white, raw.wb, raw.matrix_source, raw.truncated
        );
        for (size, label) in [(OutputSize::Half, "half"), (OutputSize::Full, "full")] {
            let o = DecodeOptions { size, ..opts };
            let (img, t_dev) = best(3, || specular_raw::develop(&raw, &o));
            let Ok(img) = img else { continue };
            let (_, t_total) = best(2, || file.decode(&data, &o));
            println!(
                "    {label}: {}x{} develop {t_dev:.1} ms, decode+develop {t_total:.1} ms (wb {:?})",
                img.width, img.height, img.wb_source
            );
            if let Some(d) = &dump_dir {
                dump(d, &format!("{name}.{label}"), &img);
            }
        }
    }
}
