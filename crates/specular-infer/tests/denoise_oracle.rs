//! The oracle run for AI denoise (`tools/corpus/denoise-oracle.py`), skipped unless
//! `SPECULAR_DENOISE_MODELS` names a folder with the downloads (`NAFNet-SIDD-width32.pth`,
//! optionally `NAFNet-SIDD-width64.pth`).
//!
//! - **Conversion** (always, with the models folder): each checkpoint is read without
//!   running code and written as f32 safetensors for the reference
//!   (`NAFNet-SIDD-width32.f32.safetensors`) and as the install's f16 file
//!   (`….f16.safetensors`, whose hash the manifest pins).
//! - **Against the reference** (with `SPECULAR_DENOISE_ORACLE`, a folder the script
//!   prepared): per input, our network whole on the GPU (and the first one on the CPU
//!   reference) with the checkpoint's f32 weights and with the installed f16 weights,
//!   against the reference output; then tiled as Develop runs it. Prints one line per
//!   image and a summary.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use specular_infer::ffdnet::{self, FfdConfig};
use specular_infer::gpu::GpuGraph;
use specular_infer::nafnet::{self, NafConfig};
use specular_infer::pt::PtFile;
use specular_infer::safetensors::{SafeTensorsFile, convert_f16_from, serialize};
use specular_infer::tiles::{Planar, Rows, Tiling};
use specular_infer::{Feed, Tensor, cpu};

fn read(p: &Path) -> Option<(usize, usize, Vec<f32>)> {
    let b = std::fs::read(p).ok()?;
    let w = u32::from_le_bytes(b[0..4].try_into().ok()?) as usize;
    let h = u32::from_le_bytes(b[4..8].try_into().ok()?) as usize;
    let v: Vec<f32> = b[8..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    (v.len() == 3 * w * h).then_some((w, h, v))
}

fn models() -> Option<PathBuf> {
    std::env::var_os("SPECULAR_DENOISE_MODELS").map(PathBuf::from)
}

/// PSNR (dB) of `a` against `b`, values 0..1.
fn psnr(a: &[f32], b: &[f32]) -> f64 {
    let se: f64 = a
        .iter()
        .zip(b)
        .map(|(&p, &q)| (f64::from(p) - f64::from(q)).powi(2))
        .sum();
    10.0 * (a.len() as f64 / se.max(1e-30)).log10()
}

fn max_abs(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q).abs())
        .fold(0f32, f32::max)
}

/// Writes the f32 and f16 copies of `pth` beside it (once).
fn convert(pth: &Path) -> (PathBuf, PathBuf) {
    let stem = pth.with_extension("");
    let f32p = stem.with_extension("f32.safetensors");
    let f16p = stem.with_extension("f16.safetensors");
    if !f32p.exists() {
        let mut st = PtFile::open(pth).unwrap();
        let names: Vec<String> = st.header.tensors.iter().map(|t| t.name.clone()).collect();
        let tensors: Vec<(String, Tensor)> = names
            .iter()
            .filter(|n| !nafnet::unused(n))
            .map(|n| {
                let t = st.tensor(n).unwrap();
                (n.clone(), Tensor::f32(&t.shape, t.to_f32()))
            })
            .collect();
        let refs: Vec<(&str, &Tensor)> = tensors.iter().map(|(n, t)| (n.as_str(), t)).collect();
        std::fs::write(&f32p, serialize(&refs, &[])).unwrap();
    }
    if !f16p.exists() {
        let st = PtFile::open(pth).unwrap();
        convert_f16_from(st, &f16p, &|_| false, &|| false).unwrap();
    }
    (f32p, f16p)
}

#[test]
fn nafnet_against_the_reference() {
    let Some(dir) = models() else {
        eprintln!("SPECULAR_DENOISE_MODELS not set: skipped");
        return;
    };
    let mut nets = Vec::new();
    for width in [32, 64] {
        let pth = dir.join(format!("NAFNet-SIDD-width{width}.pth"));
        if !pth.exists() {
            continue;
        }
        let (_, f16p) = convert(&pth);
        let mut st = PtFile::open(&pth).unwrap();
        let c = NafConfig::from_header(&st.header).unwrap();
        assert_eq!(c, NafConfig::sidd(width), "w{width} layout");
        let params: usize = st
            .header
            .tensors
            .iter()
            .map(|t| t.shape.iter().product::<usize>())
            .sum();
        eprintln!("NAFNet-SIDD-width{width}: {params} parameters, f16 file {f16p:?}");
        // The checkpoint as downloaded must build.
        nafnet::graph(&mut st, &c, (64, 64)).unwrap();
        nets.push((width, pth, f16p, c));
    }
    assert!(!nets.is_empty(), "no checkpoints in {dir:?}");
    let Some(oracle) = std::env::var_os("SPECULAR_DENOISE_ORACLE").map(PathBuf::from) else {
        eprintln!("SPECULAR_DENOISE_ORACLE not set: conversion only");
        return;
    };
    let ctx = common::ctx().expect("a GPU");
    let mut names: Vec<String> = std::fs::read_dir(oracle.join("img"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".in.f32").map(str::to_owned))
        .collect();
    names.sort();
    for (width, pth, f16p, c) in &nets {
        let tag = format!("w{width}");
        let (mut worst32, mut worst16, mut worst_tiled) = (0f32, 0f32, 0f32);
        let (mut min_p32, mut min_p16, mut min_tiled) = (f64::MAX, f64::MAX, f64::MAX);
        let mut ms = Vec::new();
        let mut graphs: Vec<((usize, usize), GpuGraph, GpuGraph, specular_infer::Graph)> =
            Vec::new();
        let mut st16 = SafeTensorsFile::open(f16p).unwrap();
        let tiling = Tiling::for_width(c.width);
        let tg = nafnet::graph(&mut st16, c, (tiling.size, tiling.size)).unwrap();
        let mut tgpu = GpuGraph::new(ctx, &tg).unwrap();
        for (k, id) in names.iter().enumerate() {
            let Some((w, h, x)) = read(&oracle.join("img").join(format!("{id}.in.f32"))) else {
                continue;
            };
            let Some((_, _, want)) = read(&oracle.join("ref").join(format!("{id}.{tag}.f32")))
            else {
                continue;
            };
            if !graphs.iter().any(|g| g.0 == (w, h)) {
                let mut st = PtFile::open(pth).unwrap();
                let g32 = nafnet::graph(&mut st, c, (w, h)).unwrap();
                let mut st16 = SafeTensorsFile::open(f16p).unwrap();
                let g16 = nafnet::graph(&mut st16, c, (w, h)).unwrap();
                graphs.push((
                    (w, h),
                    GpuGraph::new(ctx, &g32).unwrap(),
                    GpuGraph::new(ctx, &g16).unwrap(),
                    g32,
                ));
            }
            let (_, gpu32, gpu16, g32) = graphs.iter_mut().find(|g| g.0 == (w, h)).unwrap();
            let xt = Tensor::f32(&[1, 3, h, w], x.clone());
            let t0 = Instant::now();
            let o32 = gpu32.run(&[Feed::Tensor(&xt)]).unwrap()[0].to_f32();
            ms.push(t0.elapsed().as_secs_f64() * 1e3);
            let o16 = gpu16.run(&[Feed::Tensor(&xt)]).unwrap()[0].to_f32();
            if k == 0 {
                let t0 = Instant::now();
                let oc = cpu::run(g32, &[Feed::Tensor(&xt)], 8).unwrap()[0].to_f32();
                eprintln!(
                    "{tag} {id}: CPU reference {:.1} s, GPU vs CPU max {:.2e}",
                    t0.elapsed().as_secs_f64(),
                    max_abs(&o32, &oc)
                );
                assert!(max_abs(&o32, &oc) < 2e-3);
            }
            // Tiled as Develop runs it, f16 weights.
            let mut tiled = vec![0f32; 3 * w * h];
            let src = Planar { w, h, data: &x };
            let all = tiling.cells_of((w, h), 0, 0, w, h);
            tiling
                .run(
                    &src,
                    all,
                    &mut |t| Ok(tgpu.run(&[Feed::Tensor(t)])?.remove(0)),
                    &mut |r: Rows<'_>| {
                        for ch in 0..3 {
                            for y in 0..r.h {
                                for xx in 0..r.w {
                                    tiled[(ch * h + r.y + y) * w + r.x + xx] =
                                        r.data[(ch * r.h + y) * r.w + xx];
                                }
                            }
                        }
                    },
                    &mut |_, _| true,
                )
                .unwrap();
            let (p32, p16, pt) = (psnr(&o32, &want), psnr(&o16, &want), psnr(&tiled, &want));
            let (m32, m16, mt) = (
                max_abs(&o32, &want),
                max_abs(&o16, &want),
                max_abs(&tiled, &want),
            );
            eprintln!(
                "{tag} {id}: vs reference f32 {p32:.1} dB (max {m32:.1e}), f16 {p16:.1} dB \
                 (max {m16:.1e}), tiled f16 {pt:.1} dB (max {mt:.1e})"
            );
            worst32 = worst32.max(m32);
            worst16 = worst16.max(m16);
            worst_tiled = worst_tiled.max(mt);
            min_p32 = min_p32.min(p32);
            min_p16 = min_p16.min(p16);
            min_tiled = min_tiled.min(pt);
        }
        ms.sort_by(f64::total_cmp);
        eprintln!(
            "{tag} summary: {} images; f32 worst {worst32:.1e} (min {min_p32:.1} dB), f16 worst \
             {worst16:.1e} (min {min_p16:.1} dB), tiled worst {worst_tiled:.1e} (min \
             {min_tiled:.1} dB); GPU f32 512×384 median {:.0} ms",
            ms.len(),
            ms.get(ms.len() / 2).copied().unwrap_or(0.0)
        );
        assert!(min_p32 > 70.0, "f32 against the reference: {min_p32} dB");
        assert!(min_p16 > 45.0, "f16 against the reference: {min_p16} dB");
        assert!(min_tiled > 40.0, "tiled against whole: {min_tiled} dB");
    }
}

/// The noise level the script told FFDNet per level.
fn ffd_sigma(id: &str) -> f32 {
    match id.rsplit('-').next() {
        Some("iso400") => 0.01,
        Some("iso1600") => 0.02,
        _ => 0.05,
    }
}

/// FFDNet (`ffdnet_color_deepinv.pth`) against deepinv's implementation: whole on the GPU
/// (f32 and the installed f16), the first image on the CPU too, and tiled as Develop runs it
/// (equal to the whole picture away from its own edges).
#[test]
fn ffdnet_against_the_reference() {
    let Some(dir) = models() else {
        eprintln!("SPECULAR_DENOISE_MODELS not set: skipped");
        return;
    };
    let pth = dir.join("ffdnet_color_deepinv.pth");
    if !pth.exists() {
        eprintln!("no {pth:?}: skipped");
        return;
    }
    let (_, f16p) = convert(&pth);
    let st = PtFile::open(&pth).unwrap();
    let c = FfdConfig::from_header(&st.header).unwrap();
    assert_eq!(c, FfdConfig::color());
    let params: usize = st
        .header
        .tensors
        .iter()
        .map(|t| t.shape.iter().product::<usize>())
        .sum();
    eprintln!("FFDNet colour: {params} parameters, f16 file {f16p:?}");
    let Some(oracle) = std::env::var_os("SPECULAR_DENOISE_ORACLE").map(PathBuf::from) else {
        return;
    };
    let ctx = common::ctx().expect("a GPU");
    let mut names: Vec<String> = std::fs::read_dir(oracle.join("img"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".in.f32").map(str::to_owned))
        .collect();
    names.sort();
    let tiling = Tiling::FFDNET;
    let mut st16 = SafeTensorsFile::open(&f16p).unwrap();
    let tg = ffdnet::graph(&mut st16, &c, (tiling.size, tiling.size)).unwrap();
    let mut tgpu = GpuGraph::new(ctx, &tg).unwrap();
    let mut graphs: Vec<((usize, usize), GpuGraph, GpuGraph, specular_infer::Graph)> = Vec::new();
    let (mut w32, mut w16, mut wt) = (0f32, 0f32, 0f32);
    let mut ms = Vec::new();
    let mut n = 0;
    for (k, id) in names.iter().enumerate() {
        let (Some((w, h, x)), Some((_, _, want))) = (
            read(&oracle.join("img").join(format!("{id}.in.f32"))),
            read(&oracle.join("ref").join(format!("{id}.ffd.f32"))),
        ) else {
            continue;
        };
        if !graphs.iter().any(|g| g.0 == (w, h)) {
            let mut st = PtFile::open(&pth).unwrap();
            let g32 = ffdnet::graph(&mut st, &c, (w, h)).unwrap();
            let mut st16 = SafeTensorsFile::open(&f16p).unwrap();
            let g16 = ffdnet::graph(&mut st16, &c, (w, h)).unwrap();
            graphs.push((
                (w, h),
                GpuGraph::new(ctx, &g32).unwrap(),
                GpuGraph::new(ctx, &g16).unwrap(),
                g32,
            ));
        }
        let (_, gpu32, gpu16, g32) = graphs.iter_mut().find(|g| g.0 == (w, h)).unwrap();
        let xt = Tensor::f32(&[1, 3, h, w], x.clone());
        let sg = Tensor::f32(&[1, 1, 1, 1], vec![ffd_sigma(id)]);
        let feeds = [Feed::Tensor(&xt), Feed::Tensor(&sg)];
        let t0 = Instant::now();
        let o32 = gpu32.run(&feeds).unwrap()[0].to_f32();
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        let o16 = gpu16.run(&feeds).unwrap()[0].to_f32();
        if k == 0 {
            let t0 = Instant::now();
            let oc = cpu::run(g32, &feeds, 8).unwrap()[0].to_f32();
            eprintln!(
                "ffdnet {id}: CPU reference {:.1} s, GPU vs CPU max {:.2e}",
                t0.elapsed().as_secs_f64(),
                max_abs(&o32, &oc)
            );
            assert!(max_abs(&o32, &oc) < 1e-4);
        }
        let mut tiled = vec![0f32; 3 * w * h];
        let src = Planar { w, h, data: &x };
        let all = tiling.cells_of((w, h), 0, 0, w, h);
        tiling
            .run(
                &src,
                all,
                &mut |t| Ok(tgpu.run(&[Feed::Tensor(t), Feed::Tensor(&sg)])?.remove(0)),
                &mut |r: Rows<'_>| {
                    for ch in 0..3 {
                        for y in 0..r.h {
                            for xx in 0..r.w {
                                tiled[(ch * h + r.y + y) * w + r.x + xx] =
                                    r.data[(ch * r.h + y) * r.w + xx];
                            }
                        }
                    }
                },
                &mut |_, _| true,
            )
            .unwrap();
        // Tiles read edge pixels repeated past the picture, the reference zeros: compare
        // away from the picture's edges.
        let inner = |v: &[f32]| -> Vec<f32> {
            let mut o = Vec::new();
            for ch in 0..3 {
                for y in 32..h - 32 {
                    o.extend_from_slice(&v[(ch * h + y) * w + 32..(ch * h + y) * w + w - 32]);
                }
            }
            o
        };
        let (m32, m16) = (max_abs(&o32, &want), max_abs(&o16, &want));
        let mt = max_abs(&inner(&tiled), &inner(&o16));
        w32 = w32.max(m32);
        w16 = w16.max(m16);
        wt = wt.max(mt);
        n += 1;
    }
    // Speed: tiles as Develop runs them, and what a 24 MP photo takes.
    let x = Tensor::f32(
        &[1, 3, tiling.size, tiling.size],
        vec![0.4; 3 * tiling.size.pow(2)],
    );
    let sg = Tensor::f32(&[1, 1, 1, 1], vec![0.02]);
    tgpu.run(&[Feed::Tensor(&x), Feed::Tensor(&sg)]).unwrap();
    let t0 = Instant::now();
    for _ in 0..10 {
        tgpu.run(&[Feed::Tensor(&x), Feed::Tensor(&sg)]).unwrap();
    }
    let per = t0.elapsed().as_secs_f64() / 10.0;
    let tiles24 = tiling.tiles_for(
        (6000, 4000),
        tiling.cells_of((6000, 4000), 0, 0, 6000, 4000),
    );
    eprintln!(
        "ffdnet: {:.0} ms per {}² tile (f16 weights); 24 MP = {tiles24} tiles ≈ {:.0} s",
        per * 1e3,
        tiling.size,
        per * tiles24 as f64
    );
    ms.sort_by(f64::total_cmp);
    eprintln!(
        "ffdnet summary: {n} images; vs reference f32 worst {w32:.1e}, f16 worst {w16:.1e}; \
         tiled vs whole (f16, inside) worst {wt:.1e}; GPU f32 512×384 median {:.0} ms",
        ms.get(ms.len() / 2).copied().unwrap_or(0.0)
    );
    assert!(n > 0);
    assert!(w32 < 1e-4, "f32 against the reference: {w32}");
    assert!(w16 < 2e-2, "f16 against the reference: {w16}");
    assert!(wt < 1e-4, "tiled against whole: {wt}");
}
