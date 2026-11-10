//! Denoise: the à-trous levels per workgroup on the level's lattice
//! equal the per-thread kernel bit for bit.

use super::tests_diffuse::{after_exposure, edit, engine, graph_at, module, scene};
use crate::gpu::Request;
use crate::graph::tile::Tiling;
use crate::graph::{Precision, Roi};
use crate::model::{Fields, ModuleKind, Value};
use crate::testgpu::lock;

fn set(flag: &std::sync::atomic::AtomicBool, on: bool) {
    flag.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// `ATROUS_WG` equals `ATROUS` bit for bit: odd sizes and one wider than a level-16
/// block, full size (5 levels) and reduced, f32 and f16, whole and in tiles, with and
/// without non-local means.
#[test]
fn atrous_lattice_equals_atrous_threads() {
    use super::denoise::PLAIN_ATROUS;
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let mut checked = 0usize;
    for (w, h) in [(333u32, 217u32), (64, 40), (611, 300)] {
        let img = scene(w, h, 0.05, 9);
        let src = en.upload(w, h, &img).unwrap();
        for nlm in [true, false] {
            let f = Fields::new()
                .with("luminance", Value::Float(0.8))
                .with("colour", Value::Float(0.5))
                .with("detail", Value::Float(0.3))
                .with("nlm", Value::Bool(nlm));
            let e = after_exposure(edit(vec![module(ModuleKind::DENOISE, f)]));
            for (scale, prec, tiling) in [
                (1.0, Precision::F32, Tiling::Auto),
                (0.5, Precision::F16, Tiling::Auto),
                (1.0, Precision::F16, Tiling::Auto),
                (1.0, Precision::F32, Tiling::Side(160)),
            ] {
                let g = graph_at(&e, w, h, None);
                let req = Request {
                    checkpoints: false,
                    precision: prec,
                    scale,
                    tiling,
                    ..Request::new(Roi::full(w, h))
                };
                let mut out = Vec::new();
                for plain in [true, false] {
                    set(&PLAIN_ATROUS, plain);
                    let r = en.render(&g, &src, &req).unwrap();
                    out.push(
                        en.read(&r)
                            .unwrap()
                            .iter()
                            .map(|v| v.to_bits())
                            .collect::<Vec<_>>(),
                    );
                    en.recycle(r);
                }
                set(&PLAIN_ATROUS, false);
                let same = out[0].iter().zip(&out[1]).filter(|(a, b)| a == b).count();
                assert_eq!(
                    same,
                    out[0].len(),
                    "{w}x{h} nlm {nlm} scale {scale} {prec:?} {tiling:?}: {} values differ",
                    out[0].len() - same
                );
                checked += same;
            }
        }
    }
    eprintln!("{checked} values, 0 differ");
}

/// GPU milliseconds of the à-trous levels per thread and on the lattice, 3000×2000
/// (5 levels), alternating (the best of 7 each; prints).
#[test]
#[ignore = "timing run"]
fn atrous_timing() {
    use super::denoise::PLAIN_ATROUS;
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (3000u32, 2000u32);
    let src = en.upload(w, h, &scene(w, h, 0.05, 9)).unwrap();
    let f = Fields::new().with("luminance", Value::Float(0.8));
    let e = edit(vec![module(ModuleKind::DENOISE, f)]);
    let g = graph_at(&e, w, h, None);
    let req = Request {
        checkpoints: false,
        precision: Precision::F32,
        profile: true,
        ..Request::new(Roi::full(w, h))
    };
    let mut best = [f64::MAX; 2];
    let mut levels = [Vec::new(), Vec::new()];
    for _ in 0..7 {
        for (k, plain) in [true, false].into_iter().enumerate() {
            set(&PLAIN_ATROUS, plain);
            let r = en.render(&g, &src, &req).unwrap();
            let each: Vec<f64> = r
                .stats
                .gpu_ms
                .iter()
                .flatten()
                .filter(|(l, _)| l.contains("atrous") || l.contains("vst"))
                .map(|(_, m)| *m)
                .collect();
            let ms: f64 = each.iter().sum();
            if ms < best[k] {
                levels[k] = each;
            }
            best[k] = best[k].min(ms);
            en.recycle(r);
        }
    }
    set(&PLAIN_ATROUS, false);
    eprintln!(
        "atrous 3000x2000: per thread {:.1} ms {:.1?}, on the lattice {:.1} ms {:.1?}",
        best[0], levels[0], best[1], levels[1]
    );
}

/// The lattice threads cover every pixel exactly once.
#[test]
fn atrous_lattice_covers_each_pixel_once() {
    use super::kernels::{atrous_wg_pixel, atrous_wg_threads};
    for (w, h) in [(333u32, 217u32), (16, 16), (1, 1), (611, 300)] {
        for step in [1u32, 2, 4, 8, 16] {
            let [tw, th] = atrous_wg_threads(w, h, step);
            let mut seen = vec![0u8; (w * h) as usize];
            for ty in 0..th {
                for tx in 0..tw {
                    let (x, y) = atrous_wg_pixel(tx, ty, step);
                    if x < w && y < h {
                        seen[(y * w + x) as usize] += 1;
                    }
                }
            }
            assert!(seen.iter().all(|&n| n == 1), "{w}x{h} step {step}");
        }
    }
}
