//! The oracle run for AI masks (`tools/corpus/masks-oracle.py`), skipped unless
//! `SPECULAR_MASKS_ORACLE` names a folder that script prepared and `SPECULAR_MASKS_MODELS`
//! the model downloads (`efficient_sam_vitt.pt`, `isnet-general-use.onnx`,
//! `upernet-convnext-tiny.safetensors`).
//!
//! Per image: EfficientSAM's embedding, mask logits and IoU, IS-Net's probability and
//! UperNet's sky probability against the references; mask agreement (IoU of the
//! thresholded masks) with the references and, for generated scenes, with the exact ground
//! truth. The full networks run on the GPU and, for the first image, on the CPU reference
//! too (GPU = CPU). Prints one line per image and a summary.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_infer::gpu::GpuGraph;
use specular_infer::onnx::Model;
use specular_infer::safetensors::SafeTensorsFile;
use specular_infer::sam::{self, PointKind, Prompt, PromptTables, SamConfig};
use specular_infer::tensor::compare;
use specular_infer::upernet::{self, UperConfig};
use specular_infer::{DType, Feed, Graph, Tensor, cpu, isnet, pt::PtFile};

fn read_f32(p: &Path) -> Option<Vec<f32>> {
    let b = std::fs::read(p).ok()?;
    Some(
        b.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

fn read_rgba(p: &Path) -> (usize, usize, Vec<u8>) {
    let b = std::fs::read(p).unwrap();
    let w = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let h = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    (w, h, b[8..].to_vec())
}

/// `[1, 3, H, W]` from RGBA, (v/255 − mean)/std.
fn normalised(w: usize, h: usize, rgba: &[u8], mean: [f32; 3], std: [f32; 3]) -> Tensor {
    let mut v = vec![0f32; 3 * w * h];
    for i in 0..w * h {
        for c in 0..3 {
            v[c * w * h + i] = (rgba[4 * i + c] as f32 / 255.0 - mean[c]) / std[c];
        }
    }
    Tensor::f32(&[1, 3, h, w], v)
}

/// Bilinear (half-pixel) resize of a square map, then a threshold.
fn mask_at(src: &[f32], n: usize, to: usize, thr: f32) -> Vec<bool> {
    let mut out = vec![false; to * to];
    let s = n as f32 / to as f32;
    for y in 0..to {
        let fy = ((y as f32 + 0.5) * s - 0.5).clamp(0.0, (n - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(n - 1);
        for x in 0..to {
            let fx = ((x as f32 + 0.5) * s - 0.5).clamp(0.0, (n - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(n - 1);
            let v = (src[y0 * n + x0] * (1.0 - tx) + src[y0 * n + x1] * tx) * (1.0 - ty)
                + (src[y1 * n + x0] * (1.0 - tx) + src[y1 * n + x1] * tx) * ty;
            out[y * to + x] = v > thr;
        }
    }
    out
}

fn iou(a: &[bool], b: &[bool]) -> f64 {
    let (mut i, mut u) = (0usize, 0usize);
    for (&x, &y) in a.iter().zip(b) {
        i += usize::from(x && y);
        u += usize::from(x || y);
    }
    if u == 0 { 1.0 } else { i as f64 / u as f64 }
}

fn gt(dir: &Path, id: &str, kind: &str) -> Option<Vec<bool>> {
    let b = std::fs::read(dir.join("gt").join(format!("{id}.{kind}.u8"))).ok()?;
    Some(b.into_iter().map(|v| v != 0).collect())
}

fn clicks(p: &Path) -> Prompt {
    let mut pr = Prompt::default();
    for line in std::fs::read_to_string(p).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let kind = match f[2] {
            "pos" => PointKind::Positive,
            "neg" => PointKind::Negative,
            "tl" => PointKind::BoxTopLeft,
            _ => PointKind::BoxBottomRight,
        };
        pr.points
            .push(([f[0].parse().unwrap(), f[1].parse().unwrap()], kind));
    }
    pr
}

/// UperNet's input side (`UPERNET_SIDE`, as the prep run).
fn side() -> usize {
    std::env::var("UPERNET_SIDE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(384)
}

struct Nets {
    enc: GpuGraph,
    enc_g: Graph,
    dec: GpuGraph,
    dec_g: Graph,
    tables: PromptTables,
    isnet: GpuGraph,
    isnet_g: Graph,
    uper: GpuGraph,
    uper_g: Graph,
}

/// The networks from the downloads as they are (f32), or (`f16`) as an install keeps them:
/// the checkpoints converted to f16 next to the downloads, IS-Net built with f16 weights.
fn load(models: &Path, f16: bool) -> Nets {
    let ctx = common::ctx().expect("a GPU");
    let pt = models.join("efficient_sam_vitt.pt");
    let mut st = if f16 {
        let dst = models.join("efficient_sam_vitt.f16.safetensors");
        let n = specular_infer::safetensors::convert_f16_from(
            PtFile::open(&pt).unwrap(),
            &dst,
            &sam::keep_f32,
            &|| false,
        )
        .unwrap();
        eprintln!("efficientsam converted: {n} bytes at {}", dst.display());
        // The larger size too, for its pin (not run here).
        let s = models.join("efficient_sam_vits.pt");
        if s.exists() {
            let d = models.join("efficient_sam_vits.f16.safetensors");
            let st = PtFile::open(&s).unwrap();
            let n =
                specular_infer::safetensors::convert_f16_from(st, &d, &sam::keep_f32, &|| false);
            eprintln!("efficientsam-s converted: {n:?} bytes at {}", d.display());
        }
        SafeTensorsFile::open(&dst).unwrap()
    } else {
        PtFile::open(&pt).unwrap()
    };
    let c = SamConfig::from_width(sam::width_of(&st).unwrap()).unwrap();
    let mut enc_g = Graph::new();
    let x = enc_g
        .input(&[1, 3, sam::INPUT, sam::INPUT], DType::F32)
        .unwrap();
    let e = sam::encoder_from(&mut enc_g, x, &mut st, c).unwrap();
    enc_g.output(e).unwrap();
    let tables = PromptTables::read(&mut st).unwrap();
    let dec_g = sam::decoder(&mut st, &tables).unwrap();
    let bytes = std::fs::read(models.join("isnet-general-use.onnx")).unwrap();
    let m = Model::parse(&bytes).unwrap();
    let isnet_g = isnet::graph_from_tensor(&m, f16).unwrap();
    let up = models.join("upernet-convnext-tiny.safetensors");
    let mut us = if f16 {
        let dst = models.join("upernet-convnext-tiny.f16.safetensors");
        let n = specular_infer::safetensors::convert_f16(&up, &dst, &|_| false, &|| false).unwrap();
        eprintln!("upernet converted: {n} bytes at {}", dst.display());
        SafeTensorsFile::open(&dst).unwrap()
    } else {
        SafeTensorsFile::open(&up).unwrap()
    };
    let uc = UperConfig::from_header(&us.header).unwrap();
    assert_eq!(uc, UperConfig::tiny());
    let mut uper_g = Graph::new();
    let x = uper_g.input(&[1, 3, side(), side()], DType::F32).unwrap();
    let p = upernet::probabilities(&mut uper_g, x, &mut us, &uc, &[upernet::class::SKY]).unwrap();
    uper_g.output(p).unwrap();
    Nets {
        enc: GpuGraph::new(ctx, &enc_g).unwrap(),
        dec: GpuGraph::new(ctx, &dec_g).unwrap(),
        isnet: GpuGraph::new(ctx, &isnet_g).unwrap(),
        uper: GpuGraph::new(ctx, &uper_g).unwrap(),
        enc_g,
        dec_g,
        tables,
        isnet_g,
        uper_g,
    }
}

fn diff_line(name: &str, got: &[f32], want: &[f32]) -> String {
    let d = compare(got, want, 1e-3, 1e-3);
    format!("{name} max|Δ| {:.2e} cos {:.6}", d.max_abs, d.cosine)
}

#[test]
fn masks_oracle() {
    run(false);
}

/// The same with the weights as an install keeps them (f16).
#[test]
fn masks_oracle_f16() {
    run(true);
}

fn run(f16: bool) {
    let (Some(dir), Some(models)) = (
        std::env::var_os("SPECULAR_MASKS_ORACLE").map(PathBuf::from),
        std::env::var_os("SPECULAR_MASKS_MODELS").map(PathBuf::from),
    ) else {
        eprintln!("SPECULAR_MASKS_ORACLE / SPECULAR_MASKS_MODELS not set: skipped");
        return;
    };
    let t0 = Instant::now();
    let mut n = load(&models, f16);
    eprintln!("weights f16: {f16}");
    eprintln!("loaded in {:.1} s", t0.elapsed().as_secs_f64());
    let mut ids: Vec<String> = std::fs::read_dir(dir.join("img"))
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".rgba").map(str::to_owned)
        })
        .collect();
    ids.sort();
    let (mut agree, mut truth) = (Vec::new(), Vec::new());
    let mut times = [0f64; 4];
    for (k, id) in ids.iter().enumerate() {
        let r = |s: &str| dir.join("ref").join(format!("{id}.{s}"));
        let (w, h, rgba) = read_rgba(&dir.join("img").join(format!("{id}.rgba")));
        assert_eq!((w, h), (1024, 1024));
        // EfficientSAM.
        let x = normalised(w, h, &rgba, [0.485, 0.456, 0.406], [0.229, 0.224, 0.225]);
        let t = Instant::now();
        let emb = n.enc.run(&[Feed::Tensor(&x)]).unwrap().remove(0);
        times[0] += t.elapsed().as_secs_f64();
        let prompt = clicks(&dir.join("img").join(format!("{id}.clicks.tsv")));
        let sparse = prompt.embed(&n.tables).unwrap();
        let t = Instant::now();
        let out = n
            .dec
            .run(&[Feed::Tensor(&emb), Feed::Tensor(&sparse)])
            .unwrap();
        times[1] += t.elapsed().as_secs_f64();
        let (masks, ious) = (out[0].to_f32(), out[1].to_f32());
        if k == 0 {
            let c = cpu::run(&n.enc_g, &[Feed::Tensor(&x)], 16)
                .unwrap()
                .remove(0);
            eprintln!(
                "{}",
                diff_line("sam encoder gpu/cpu", &emb.to_f32(), &c.to_f32())
            );
            let cd = cpu::run(&n.dec_g, &[Feed::Tensor(&c), Feed::Tensor(&sparse)], 16).unwrap();
            eprintln!(
                "{}",
                diff_line("sam decoder gpu/cpu", &masks, &cd[0].to_f32())
            );
        }
        let mut line = format!("{id}:");
        let best = sam::best(&ious);
        let mm = sam::MASK * sam::MASK;
        let ours = mask_at(&masks[best * mm..][..mm], sam::MASK, 1024, 0.0);
        if let (Some(re), Some(rm), Some(ri)) = (
            read_f32(&r("sam.emb.f32")),
            read_f32(&r("sam.masks.f32")),
            read_f32(&r("sam.iou.f32")),
        ) {
            line += &format!(" [{}]", diff_line("emb", &emb.to_f32(), &re));
            line += &format!(" [{}]", diff_line("logits", &masks, &rm));
            line += &format!(" iou {ious:.3?} ref {ri:.3?}");
            let rb = sam::best(&ri);
            let theirs = mask_at(&rm[rb * mm..][..mm], sam::MASK, 1024, 0.0);
            let a = iou(&ours, &theirs);
            line += &format!(" sam-agree {a:.4}");
            agree.push(("sam", a));
        }
        if let Some(g) = gt(&dir, id, "object") {
            let a = iou(&ours, &g);
            line += &format!(" sam-gt {a:.3}");
            truth.push(("sam", a));
        }
        // IS-Net.
        let x = normalised(w, h, &rgba, [0.5; 3], [1.0; 3]);
        let t = Instant::now();
        let p = n.isnet.run(&[Feed::Tensor(&x)]).unwrap().remove(0).to_f32();
        times[2] += t.elapsed().as_secs_f64();
        if k == 0 {
            let c = cpu::run(&n.isnet_g, &[Feed::Tensor(&x)], 16)
                .unwrap()
                .remove(0);
            eprintln!("{}", diff_line("isnet gpu/cpu", &p, &c.to_f32()));
        }
        let ours = mask_at(&p, 1024, 1024, 0.5);
        if let Some(rp) = read_f32(&r("isnet.f32")) {
            line += &format!(" [{}]", diff_line("isnet", &p, &rp));
            let a = iou(&ours, &mask_at(&rp, 1024, 1024, 0.5));
            line += &format!(" isnet-agree {a:.4}");
            agree.push(("isnet", a));
        }
        if let Some(g) = gt(&dir, id, "subject") {
            let a = iou(&ours, &g);
            line += &format!(" isnet-gt {a:.3}");
            truth.push(("isnet", a));
        }
        // UperNet sky.
        if let (Some(xin), Some(rs)) = (read_f32(&r("upernet.in.f32")), read_f32(&r("upernet.f32")))
        {
            let x = Tensor::f32(&[1, 3, side(), side()], xin);
            let t = Instant::now();
            let p = n.uper.run(&[Feed::Tensor(&x)]).unwrap().remove(0).to_f32();
            times[3] += t.elapsed().as_secs_f64();
            if k == 0 {
                let c = cpu::run(&n.uper_g, &[Feed::Tensor(&x)], 16)
                    .unwrap()
                    .remove(0);
                eprintln!("{}", diff_line("upernet gpu/cpu", &p, &c.to_f32()));
                eprintln!("{}", diff_line("upernet cpu/ref", &c.to_f32(), &rs));
            }
            line += &format!(" [{}]", diff_line("sky", &p, &rs));
            let ours = mask_at(&p, side() / 4, 1024, 0.5);
            let a = iou(&ours, &mask_at(&rs, side() / 4, 1024, 0.5));
            line += &format!(" sky-agree {a:.4}");
            agree.push(("sky", a));
            if let Some(g) = gt(&dir, id, "sky") {
                let a = iou(&ours, &g);
                line += &format!(" sky-gt {a:.3}");
                truth.push(("sky", a));
            }
        }
        eprintln!("{line}");
    }
    let n_img = ids.len().max(1) as f64;
    eprintln!(
        "gpu ms per image: sam encoder {:.0}, decoder {:.1}, isnet {:.0}, upernet {:.0}",
        times[0] * 1e3 / n_img,
        times[1] * 1e3 / n_img,
        times[2] * 1e3 / n_img,
        times[3] * 1e3 / n_img
    );
    for kind in ["sam", "isnet", "sky"] {
        let a: Vec<f64> = agree.iter().filter(|x| x.0 == kind).map(|x| x.1).collect();
        let t: Vec<f64> = truth.iter().filter(|x| x.0 == kind).map(|x| x.1).collect();
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let min = |v: &[f64]| v.iter().copied().fold(1.0, f64::min);
        eprintln!(
            "{kind}: agreement with the reference mean {:.4} min {:.4} ({}); ground truth mean {:.3} min {:.3} ({})",
            mean(&a),
            min(&a),
            a.len(),
            mean(&t),
            min(&t),
            t.len()
        );
        let floor = if f16 { 0.95 } else { 0.97 };
        assert!(
            a.is_empty() || min(&a) > floor,
            "{kind}: masks disagree with the reference"
        );
    }
}
