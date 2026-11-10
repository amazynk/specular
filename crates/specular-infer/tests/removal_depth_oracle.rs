//! The oracle run for object removal and depth (`tools/corpus/removal-depth-oracle.py`), skipped
//! unless `SPECULAR_REMOVAL_DEPTH_ORACLE` names a folder that script prepared and
//! `SPECULAR_REMOVAL_DEPTH_MODELS` the downloads (`big-lama.safetensors`, `da2-small.safetensors`).
//!
//! Per photo:
//! - **Depth** (Depth Anything V2 Small): our network on the reference's own preprocessed
//!   pixels against its depth, with the f32 download and with the f16 file an install
//!   keeps; and end to end from the picture with our preprocessing. AbsRel (mean
//!   |ours − ref| / ref) and δ1 (share within 1.25×).
//! - **Remove** (Big-LaMa): our network on the reference's input crop against its output,
//!   f32 and f16: worst and mean |ours − ref| inside the hole (0..1 RGB) and the PSNR there.
//!
//! The first photo also runs both networks on the CPU reference (GPU = CPU). Writes the
//! f16 files next to the downloads (`converted/`) for the manifest's pins. Prints one line
//! per photo and a summary with GPU timings.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_infer::depth::{self, DepthConfig};
use specular_infer::gpu::GpuGraph;
use specular_infer::safetensors::{self, SafeTensorsFile};
use specular_infer::tensor::compare;
use specular_infer::{Feed, Graph, Tensor, cpu, lama};

fn read_f32(p: &Path) -> Vec<f32> {
    std::fs::read(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn read_rgba(p: &Path) -> (usize, usize, Vec<u8>) {
    let b = std::fs::read(p).unwrap();
    let w = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let h = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    (w, h, b[8..].to_vec())
}

fn nums(p: &Path) -> Vec<usize> {
    std::fs::read_to_string(p)
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect()
}

/// AbsRel and δ1 of `a` against the reference `r` (values of `r` near 0 left out).
fn depth_metrics(a: &[f32], r: &[f32]) -> (f64, f64) {
    let rmax = r.iter().copied().fold(0f32, f32::max);
    let (mut rel, mut ok, mut n) = (0f64, 0usize, 0usize);
    for (&x, &y) in a.iter().zip(r) {
        if y <= 1e-3 * rmax {
            continue;
        }
        let (x, y) = (f64::from(x.max(1e-9)), f64::from(y));
        rel += (x - y).abs() / y;
        ok += usize::from((x / y).max(y / x) < 1.25);
        n += 1;
    }
    (rel / n.max(1) as f64, ok as f64 / n.max(1) as f64)
}

/// Worst and mean |a − r| over the hole, and the PSNR there (peak 1).
fn hole_metrics(a: &[f32], r: &[f32], hole: &[f32], n: usize) -> (f32, f32, f64) {
    let (mut worst, mut sum, mut sq, mut k) = (0f32, 0f64, 0f64, 0usize);
    for c in 0..3 {
        for i in 0..n {
            if hole[i] < 0.5 {
                continue;
            }
            let d = (a[c * n + i] - r[c * n + i]).abs();
            worst = worst.max(d);
            sum += f64::from(d);
            sq += f64::from(d) * f64::from(d);
            k += 1;
        }
    }
    let mse = sq / k.max(1) as f64;
    (
        worst,
        (sum / k.max(1) as f64) as f32,
        10.0 * (1.0 / mse.max(1e-20)).log10(),
    )
}

fn convert(src: &Path, dst: &Path, keep: &dyn Fn(&safetensors::TensorInfo) -> bool) {
    if !dst.exists() {
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        safetensors::convert_f16(src, dst, keep, &|| false).unwrap();
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

#[test]
fn removal_depth_oracle() {
    let (Some(dir), Some(models)) = (
        std::env::var_os("SPECULAR_REMOVAL_DEPTH_ORACLE").map(PathBuf::from),
        std::env::var_os("SPECULAR_REMOVAL_DEPTH_MODELS").map(PathBuf::from),
    ) else {
        eprintln!("SPECULAR_REMOVAL_DEPTH_ORACLE / SPECULAR_REMOVAL_DEPTH_MODELS not set: skipped");
        return;
    };
    let ctx = common::ctx().expect("the oracle runs on the GPU");
    let da_f32 = models.join("da2-small.safetensors");
    let lama_f32 = models.join("big-lama.safetensors");
    let da_f16 = models.join("converted/da2-small-f16.safetensors");
    let lama_f16 = models.join("converted/big-lama-f16.safetensors");
    convert(&da_f32, &da_f16, &depth::keep_f32);
    convert(&lama_f32, &lama_f16, &lama::keep_f32);
    let mut ids: Vec<String> = std::fs::read_dir(dir.join("img"))
        .unwrap()
        .filter_map(|e| {
            let n = e.ok()?.file_name().into_string().ok()?;
            n.strip_suffix(".rgba").map(str::to_owned)
        })
        .collect();
    ids.sort();
    let cfg = DepthConfig::from_header(&SafeTensorsFile::open(&da_f32).unwrap().header).unwrap();
    assert_eq!(cfg, DepthConfig::small());
    let (mut d32, mut d16, mut dend) = (Vec::new(), Vec::new(), Vec::new());
    let (mut l32, mut l16) = (Vec::new(), Vec::new());
    let (mut t_depth, mut t_lama) = (Vec::new(), Vec::new());
    let mut checked_cpu = (false, false);
    for id in &ids {
        let rf = |s: &str| dir.join("ref").join(format!("{id}.{s}"));
        // Depth on the reference's pixels.
        let sz = nums(&rf("da2.size"));
        let (iw, ih) = (sz[0], sz[1]);
        let x = Tensor::f32(&[1, 3, ih, iw], read_f32(&rf("da2.in.f32")));
        let want = read_f32(&rf("da2.f32"));
        let mut line = format!("{id}: depth {iw}x{ih}");
        for (path, acc) in [(&da_f32, &mut d32), (&da_f16, &mut d16)] {
            let mut st = SafeTensorsFile::open(path).unwrap();
            let g = depth::graph_from_tensor(&mut st, &cfg, (iw, ih)).unwrap();
            let mut gg = GpuGraph::new(ctx, &g).unwrap();
            let t = Instant::now();
            let out = gg.run(&[Feed::Tensor(&x)]).unwrap().remove(0).to_f32();
            if path == &da_f32 {
                t_depth.push(t.elapsed().as_secs_f64() * 1e3);
                if !checked_cpu.0 {
                    let c = cpu::run(&g, &[Feed::Tensor(&x)], 8)
                        .unwrap()
                        .remove(0)
                        .to_f32();
                    let d = compare(&out, &c, 1e-3, 1e-3);
                    eprintln!(
                        "depth GPU vs CPU: max {:.2e}, cosine {:.7}",
                        d.max_abs, d.cosine
                    );
                    assert!(d.worst <= 1.0);
                    checked_cpu.0 = true;
                }
            }
            let m = depth_metrics(&out, &want);
            line += &format!(" AbsRel {:.5} δ1 {:.4}", m.0, m.1);
            acc.push(m);
        }
        // End to end from the picture with our preprocessing.
        let (w, h, rgba) = read_rgba(&dir.join("img").join(format!("{id}.rgba")));
        assert_eq!(depth::input_size(w, h, depth::SIDE), (iw, ih));
        let mut st = SafeTensorsFile::open(&da_f16).unwrap();
        let g: Graph = depth::graph(&mut st, &cfg, (iw, ih)).unwrap();
        let out = GpuGraph::new(ctx, &g)
            .unwrap()
            .run(&[Feed::Rgba {
                width: w as u32,
                height: h as u32,
                data: &rgba,
            }])
            .unwrap()
            .remove(0)
            .to_f32();
        let m = depth_metrics(&out, &want);
        line += &format!(" | from the picture AbsRel {:.4} δ1 {:.4}", m.0, m.1);
        dend.push(m);
        // Remove.
        let s = nums(&rf("lama.size"));
        let (cw, ch) = (s[2], s[3]);
        let x = Tensor::f32(&[1, 4, ch, cw], read_f32(&rf("lama.in.f32")));
        if cw.max(ch) > 480 {
            // Past the app's 448 px work side (512² can lose the iGPU device under load).
            eprintln!("{line} | remove {cw}x{ch} skipped (over the work side)");
            continue;
        }
        let hole = x.to_f32()[3 * cw * ch..].to_vec();
        let want = read_f32(&rf("lama.f32"));
        line += &format!(" | remove {cw}x{ch}");
        for (path, acc) in [(&lama_f32, &mut l32), (&lama_f16, &mut l16)] {
            let mut st = SafeTensorsFile::open(path).unwrap();
            let g = lama::graph(&mut st, (cw, ch)).unwrap();
            let mut gg = GpuGraph::new(ctx, &g).unwrap();
            let t = Instant::now();
            let out = gg.run(&[Feed::Tensor(&x)]).unwrap().remove(0).to_f32();
            if path == &lama_f32 {
                t_lama.push((cw, t.elapsed().as_secs_f64() * 1e3));
                if !checked_cpu.1 && cw <= 256 {
                    let c = cpu::run(&g, &[Feed::Tensor(&x)], 8)
                        .unwrap()
                        .remove(0)
                        .to_f32();
                    let d = compare(&out, &c, 1e-3, 1e-3);
                    eprintln!(
                        "lama {cw}² GPU vs CPU: max {:.2e}, cosine {:.7}",
                        d.max_abs, d.cosine
                    );
                    assert!(d.worst <= 1.0);
                    checked_cpu.1 = true;
                }
            }
            let m = hole_metrics(&out, &want, &hole, cw * ch);
            line += &format!(" worst {:.4} mean {:.5} PSNR {:.1} dB", m.0, m.1, m.2);
            acc.push(m);
        }
        eprintln!("{line}");
    }
    let mean2 = |v: &[(f64, f64)]| {
        let n = v.len().max(1) as f64;
        (
            v.iter().map(|m| m.0).sum::<f64>() / n,
            v.iter().map(|m| m.1).sum::<f64>() / n,
            v.iter().map(|m| m.0).fold(0f64, f64::max),
            v.iter().map(|m| m.1).fold(1f64, f64::min),
        )
    };
    for (name, v) in [
        ("f32", &d32),
        ("f16", &d16),
        ("from the picture (f16)", &dend),
    ] {
        let m = mean2(v);
        eprintln!(
            "depth {name}: AbsRel mean {:.5} worst {:.5}; δ1 mean {:.4} worst {:.4}",
            m.0, m.2, m.1, m.3
        );
    }
    for (name, v) in [("f32", &l32), ("f16", &l16)] {
        let worst = v.iter().map(|m| m.0).fold(0f32, f32::max);
        let mean = v.iter().map(|m| m.1).sum::<f32>() / v.len().max(1) as f32;
        let psnr = v.iter().map(|m| m.2).fold(f64::MAX, f64::min);
        eprintln!("remove {name}: worst {worst:.4}, mean {mean:.5}, lowest PSNR {psnr:.1} dB");
    }
    eprintln!(
        "GPU: depth median {:.0} ms; remove {}",
        median(t_depth.clone()),
        t_lama
            .iter()
            .map(|(s, t)| format!("{s}² {t:.0} ms"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    // f32 must agree with the reference to float rounding; the f16 file closely.
    let m = mean2(&d32);
    assert!(m.2 < 2e-3 && m.3 > 0.999, "depth f32 {m:?}");
    let m = mean2(&d16);
    assert!(m.2 < 2e-2 && m.3 > 0.99, "depth f16 {m:?}");
    assert!(l32.iter().all(|m| m.2 > 40.0), "remove f32");
    assert!(l16.iter().all(|m| m.2 > 30.0), "remove f16");
}
