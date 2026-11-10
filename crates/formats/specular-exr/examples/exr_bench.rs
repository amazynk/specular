//! Decode timings: specular-exr vs the `exr` crate on a generated RGBA half image, per compression.
//!
//! `cargo run --release -p specular-exr --example exr_bench [-- WIDTH HEIGHT [DIR]]`
//! Files are cached in DIR (default `target/bench-exr`).
//!
//! `cargo run --release -p specular-exr --example exr_bench -- --files FILE...` times our decode
//! of existing files instead (e.g. the DWA files `tools/corpus/exr-dwa.sh --bench` writes, which
//! the `exr` crate cannot read), in parallel and on one thread.

use std::{
    io::Cursor,
    path::PathBuf,
    time::{Duration, Instant},
};

use exr::prelude::*;

fn image(
    w: usize,
    h: usize,
    c: Compression,
    tiles: bool,
) -> Image<Layer<AnyChannels<FlatSamples>>> {
    // A rendered-looking image: smooth gradients, soft highlights, mild noise.
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        ((seed >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.02
    };
    let mut planes: Vec<Vec<f16>> = (0..4).map(|_| Vec::with_capacity(w * h)).collect();
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let d = ((u - 0.6).powi(2) + (v - 0.4).powi(2)).sqrt();
            let glow = 4.0 / (1.0 + 40.0 * d * d);
            let rgb = [
                0.2 + 0.6 * u + glow,
                0.3 + 0.4 * v + 0.8 * glow,
                0.5 - 0.3 * u + 0.5 * glow,
            ];
            for c in 0..3 {
                planes[c].push(f16::from_f32((rgb[c] + noise()).max(0.0)));
            }
            planes[3].push(f16::from_f32(if d < 0.45 { 1.0 } else { 0.0 }));
        }
    }
    let mut it = planes.into_iter();
    let list = ["R", "G", "B", "A"]
        .iter()
        .map(|n| AnyChannel::new(*n, FlatSamples::F16(it.next().unwrap())))
        .collect::<Vec<_>>();
    let enc = Encoding {
        compression: c,
        blocks: if tiles {
            Blocks::Tiles(Vec2(64, 64))
        } else {
            Blocks::ScanLines
        },
        line_order: LineOrder::Increasing,
    };
    Image::from_layer(Layer::new(
        (w, h),
        LayerAttributes::named(""),
        enc,
        AnyChannels::sort(SmallVec::from_vec(list)),
    ))
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn time(runs: usize, mut f: impl FnMut()) -> Duration {
    f(); // warm-up
    median(
        (0..runs)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed()
            })
            .collect(),
    )
}

/// Our decode of each file, in MP/s: f32 and f16 output, parallel and on one thread.
fn files(paths: &[String]) {
    println!("{} threads", rayon::current_num_threads());
    println!(
        "{:<36} {:>8} {:>7} {:>10} {:>10} {:>10}",
        "file", "size MB", "MP", "f32 MP/s", "f16 MP/s", "1-thr MP/s"
    );
    for path in paths {
        let bytes = std::fs::read(path).expect("read");
        let info = specular_exr::read_info(&bytes).expect("info");
        let mp = info.parts[0].width() as f64 * info.parts[0].height() as f64 / 1e6;
        let run = |format, parallel| {
            let o = specular_exr::DecodeOptions {
                format,
                parallel,
                ..Default::default()
            };
            let t = time(7, || {
                let img = specular_exr::decode(&bytes, &o).expect("ours");
                assert_eq!(img.damaged_chunks, 0);
                std::hint::black_box(img);
            });
            mp / t.as_secs_f64()
        };
        let name = PathBuf::from(path);
        println!(
            "{:<36} {:>8.1} {:>7.1} {:>10.0} {:>10.0} {:>10.0}",
            name.file_name().unwrap().to_string_lossy(),
            bytes.len() as f64 / 1e6,
            mp,
            run(specular_exr::SampleFormat::F32, true),
            run(specular_exr::SampleFormat::F16, true),
            run(specular_exr::SampleFormat::F32, false),
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--files") {
        return files(&args[2..]);
    }
    let w: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4000);
    let h: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3000);
    let dir = PathBuf::from(
        args.get(3)
            .cloned()
            .unwrap_or_else(|| "target/bench-exr".into()),
    );
    std::fs::create_dir_all(&dir).expect("bench dir");
    println!(
        "{w}x{h} RGBA half ({:.1} MP), {} threads",
        (w * h) as f64 / 1e6,
        rayon::current_num_threads()
    );
    println!(
        "{:<14} {:>9} {:>11} {:>11} {:>11} {:>7}",
        "compression", "size MB", "ours f32", "ours f16", "exr crate", "speedup"
    );
    let cases = [
        (Compression::Uncompressed, false),
        (Compression::RLE, false),
        (Compression::ZIP1, false),
        (Compression::ZIP16, false),
        (Compression::PIZ, false),
        (Compression::PXR24, false),
        (Compression::B44, false),
        (Compression::B44A, false),
        (Compression::ZIP16, true),
        (Compression::PIZ, true),
    ];
    for (c, tiles) in cases {
        let name = format!("{c:?}{}", if tiles { "-tiled" } else { "" });
        let path = dir.join(format!("{name}-{w}x{h}.exr"));
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                let mut out = Vec::new();
                image(w, h, c, tiles)
                    .write()
                    .to_buffered(Cursor::new(&mut out))
                    .expect("write");
                std::fs::write(&path, &out).expect("save");
                out
            }
        };
        let runs = 7;
        let ours = time(runs, || {
            let img = specular_exr::decode(&bytes, &Default::default()).expect("ours");
            assert_eq!(img.damaged_chunks, 0);
            std::hint::black_box(img);
        });
        let ours16 = time(runs, || {
            let o = specular_exr::DecodeOptions {
                format: specular_exr::SampleFormat::F16,
                ..Default::default()
            };
            std::hint::black_box(specular_exr::decode(&bytes, &o).expect("ours"));
        });
        let theirs = time(runs.min(5), || {
            let img = read()
                .no_deep_data()
                .largest_resolution_level()
                .all_channels()
                .first_valid_layer()
                .all_attributes()
                .from_buffered(Cursor::new(&bytes))
                .expect("exr");
            std::hint::black_box(img);
        });
        println!(
            "{:<14} {:>9.1} {:>9.1}ms {:>9.1}ms {:>9.1}ms {:>6.1}x",
            name,
            bytes.len() as f64 / 1e6,
            ours.as_secs_f64() * 1e3,
            ours16.as_secs_f64() * 1e3,
            theirs.as_secs_f64() * 1e3,
            theirs.as_secs_f64() / ours.as_secs_f64()
        );
    }
}
