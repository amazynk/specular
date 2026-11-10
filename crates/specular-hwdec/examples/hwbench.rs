//! Hardware vs software JPEG decode: `cargo run --release -p specular-hwdec --example hwbench -- FILE...`
//!
//! Prints the hardware decoders found, then per file: software decode (all threads and one
//! thread), hardware decode (VA-API, read back to planes) and the PSNR between the two.

use std::time::Instant;

fn best(runs: usize, mut f: impl FnMut() -> bool) -> Option<f64> {
    let mut b = f64::INFINITY;
    for _ in 0..runs {
        let t = Instant::now();
        if !f() {
            return None;
        }
        b = b.min(t.elapsed().as_secs_f64() * 1e3);
    }
    Some(b)
}

fn main() {
    let t0 = Instant::now();
    let caps = specular_hwdec::capabilities();
    println!("probe: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    match caps {
        Some(c) => c.report().iter().for_each(|l| println!("  {l}")),
        None => println!("  no hardware decoder"),
    }
    let runs: usize = std::env::var("RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).expect("read");
        let info = specular_jpeg::read_info(&data).expect("info");
        let sw = |p| {
            let o = specular_jpeg::DecodeOptions {
                parallelism: p,
                ..Default::default()
            };
            specular_jpeg::decode(&data, &o)
        };
        let auto = best(runs, || sw(specular_jpeg::Parallelism::Auto).is_ok());
        let one = best(runs.min(3), || {
            sw(specular_jpeg::Parallelism::Threads(1)).is_ok()
        });
        let hw_first = Instant::now();
        let hw = specular_hwdec::decode_jpeg_unchecked(&data);
        let first = hw_first.elapsed().as_secs_f64() * 1e3;
        let name = path.rsplit('/').next().unwrap_or(&path);
        print!(
            "{name}: {}x{} sw {:.1} ms (1 thread {:.1})",
            info.width,
            info.height,
            auto.unwrap_or(f64::NAN),
            one.unwrap_or(f64::NAN)
        );
        match hw {
            Ok(r) => {
                let t = best(runs, || {
                    specular_hwdec::decode_jpeg_unchecked(&data).is_ok()
                });
                let s = sw(specular_jpeg::Parallelism::Auto).unwrap();
                let max_h = s.planes.iter().map(|p| p.h_samp).max().unwrap_or(1);
                let max_v = s.planes.iter().map(|p| p.v_samp).max().unwrap_or(1);
                let sr = specular_core::Raster {
                    width: s.width,
                    height: s.height,
                    origin: (0, 0),
                    scale: 1,
                    layout: if s.planes.len() == 1 {
                        specular_core::PixelLayout::Gray
                    } else {
                        specular_core::PixelLayout::YCbCr
                    },
                    planes: s
                        .planes
                        .into_iter()
                        .map(|p| specular_core::RasterPlane {
                            width: p.width,
                            height: p.height,
                            stride: p.stride,
                            data: p.data,
                            h_div: max_h / p.h_samp,
                            v_div: max_v / p.v_samp,
                        })
                        .collect(),
                    truncated: false,
                };
                if let Some(dir) = std::env::var_os("SPECULAR_HWDEC_DUMP") {
                    // Y planes as PGM, for looking at a mismatch.
                    for (tag, ras) in [("hw", &r), ("sw", &sr)] {
                        let p = &ras.planes[0];
                        let mut f = format!("P5 {} {} 255\n", p.width, p.height).into_bytes();
                        for row in p.data.chunks(p.stride).take(p.height as usize) {
                            f.extend_from_slice(&row[..p.width as usize]);
                        }
                        let out = std::path::Path::new(&dir).join(format!("{name}.{tag}.pgm"));
                        std::fs::write(out, f).unwrap();
                    }
                }
                println!(
                    ", hw {:.1} ms (first {first:.1}), PSNR {:.1} dB",
                    t.unwrap_or(f64::NAN),
                    specular_hwdec::raster_psnr(&r, &sr).unwrap_or(f64::NAN)
                );
            }
            Err(e) => println!(", hw: {e}"),
        }
    }
}
