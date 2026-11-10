//! The oracle run for faces (`tools/corpus/face-oracle.py`), skipped unless
//! `SPECULAR_FACE_ORACLE` names a folder that script prepared and `SPECULAR_FACE_MODELS` the
//! model downloads.
//!
//! Reads `img/<id>.rgba` (u32 width, u32 height, RGBA8), runs our pipeline and writes:
//! - `ours/<id>.whole.tsv`, `ours/<id>.tiled.tsv`: faces (score, x, y, w, h, 6 keypoints)
//!   from the whole image and with tiles;
//! - `ours/<id>.eyes.tsv`: per tiled face, the left and right blink scores and presence;
//! - `ours/<id>.sface.f32`, `ours/<id>.aura.f32`: per tiled face, the unit embedding;
//! - `dump/<kind>/<id>.in.f32` and `.out.f32` for the first images: the exact tensors fed to
//!   each network and what it gave (GPU), for LiteRT and ONNX Runtime to recompute.
//! - `ours/timing.tsv`: per image, detect and embed milliseconds.

mod common;

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use specular_infer::Tensor;
use specular_infer::face::blazeface::{Detector, Face, Range};
use specular_infer::face::embed::{Embedder, Kind};
use specular_infer::face::landmarker::Landmarker;
use specular_infer::face::{Image, normalize};

fn write_f32(path: &Path, v: &[f32]) {
    let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(path, b).unwrap();
}

fn faces_tsv(path: &Path, faces: &[Face]) {
    let mut f = std::fs::File::create(path).unwrap();
    for x in faces {
        let mut line = format!(
            "{}\t{}\t{}\t{}\t{}",
            x.score, x.rect[0], x.rect[1], x.rect[2], x.rect[3]
        );
        for p in &x.points {
            line += &format!("\t{}\t{}", p[0], p[1]);
        }
        writeln!(f, "{line}").unwrap();
    }
}

fn dump(dir: &Path, kind: &str, id: &str, input: &Tensor, out: &[f32]) {
    let d = dir.join("dump").join(kind);
    std::fs::create_dir_all(&d).unwrap();
    write_f32(&d.join(format!("{id}.in.f32")), &input.to_f32());
    write_f32(&d.join(format!("{id}.out.f32")), out);
}

#[test]
fn face_oracle_run() {
    let (Some(dir), Some(models)) = (
        std::env::var_os("SPECULAR_FACE_ORACLE"),
        std::env::var_os("SPECULAR_FACE_MODELS"),
    ) else {
        eprintln!("SPECULAR_FACE_ORACLE / SPECULAR_FACE_MODELS not set: skipped");
        return;
    };
    let (dir, models) = (Path::new(&dir), Path::new(&models));
    let ctx = common::ctx();
    let threads = 4;
    let read = |f: &str| std::fs::read(models.join(f)).unwrap();
    let mut det = Detector::new(
        &read("blaze_face_full_range.tflite"),
        Range::Full,
        ctx,
        threads,
    )
    .unwrap();
    if let Ok(v) = std::env::var("FACE_MIN_SCORE") {
        det.min_score = v.parse().unwrap();
    }
    let mut lm = Landmarker::new(&read("face_landmarker.task"), ctx, threads).unwrap();
    let mut sface = Embedder::new(
        &read("face_recognition_sface_2021dec.onnx"),
        Kind::Sface,
        true,
        ctx,
        threads,
    )
    .unwrap();
    let aura_bytes = std::fs::read(models.join("glintr100.onnx")).ok();
    let mut aura = aura_bytes
        .as_ref()
        .map(|b| Embedder::new(b, Kind::AuraFace, true, ctx, threads).unwrap());
    let ours = dir.join("ours");
    std::fs::create_dir_all(&ours).unwrap();
    let dumps: usize = std::env::var("FACE_DUMPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let mut timing = std::fs::File::create(ours.join("timing.tsv")).unwrap();
    let mut ids: Vec<String> = std::fs::read_dir(dir.join("img"))
        .unwrap()
        .filter_map(|e| {
            let n = e.ok()?.file_name().into_string().ok()?;
            n.strip_suffix(".rgba").map(str::to_owned)
        })
        .collect();
    ids.sort();
    for (k, id) in ids.iter().enumerate() {
        let raw = std::fs::read(dir.join("img").join(format!("{id}.rgba"))).unwrap();
        let w = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
        let h = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
        let img = Image::new(w, h, &raw[8..]).unwrap();
        let t = Instant::now();
        let whole = det.detect_whole(img).unwrap();
        faces_tsv(&ours.join(format!("{id}.whole.tsv")), &whole);
        let t1 = Instant::now();
        let tiled = det.detect(img).unwrap();
        let detect_ms = t1.elapsed().as_secs_f64() * 1e3;
        faces_tsv(&ours.join(format!("{id}.tiled.tsv")), &tiled);
        if k < dumps {
            let small = specular_infer::face::shrink(img, det.size);
            let sv = small.view();
            let x = det.input(sv, Detector::whole(sv));
            let out = det.raw(&x, false).unwrap();
            let mut o = out[0].to_f32();
            o.extend(out[1].to_f32());
            dump(dir, "blaze", id, &x, &o);
        }
        let t2 = Instant::now();
        let mut sv = Vec::new();
        let mut av = Vec::new();
        for f in &tiled {
            let x = sface.input(img, f).unwrap();
            let mut e = sface.raw(&x, false).unwrap();
            if k < dumps && sv.is_empty() {
                dump(dir, "sface", id, &x, &e);
            }
            normalize(&mut e);
            sv.extend(e);
        }
        let embed_ms = t2.elapsed().as_secs_f64() * 1e3;
        write_f32(&ours.join(format!("{id}.sface.f32")), &sv);
        if let Some(a) = aura.as_mut() {
            for f in &tiled {
                let x = a.input(img, f).unwrap();
                let mut e = a.raw(&x, false).unwrap();
                if k < dumps.min(12) && av.is_empty() {
                    dump(dir, "aura", id, &x, &e);
                }
                normalize(&mut e);
                av.extend(e);
            }
            write_f32(&ours.join(format!("{id}.aura.f32")), &av);
        }
        let t3 = Instant::now();
        let mut eyes = std::fs::File::create(ours.join(format!("{id}.eyes.tsv"))).unwrap();
        for (i, f) in tiled.iter().enumerate() {
            if k < dumps && i == 0 {
                let mx = lm.mesh_input(img, f);
                let (pts, p) = lm.mesh_raw(&mx, false).unwrap();
                let mut o = pts.clone();
                o.push(p);
                dump(dir, "mesh", id, &mx, &o);
                let mesh = lm.mesh(img, f).unwrap();
                let si = Landmarker::shapes_input(&mesh);
                let so = lm.shapes_raw(&si).unwrap();
                dump(dir, "shapes", id, &si, &so);
            }
            let e = lm.eyes(img, f).unwrap();
            writeln!(eyes, "{}\t{}\t{}", e.left, e.right, e.presence).unwrap();
        }
        let eyes_ms = t3.elapsed().as_secs_f64() * 1e3;
        writeln!(
            timing,
            "{id}\t{w}\t{h}\t{}\t{detect_ms:.1}\t{embed_ms:.1}\t{eyes_ms:.1}\t{:.1}",
            tiled.len(),
            t.elapsed().as_secs_f64() * 1e3
        )
        .unwrap();
    }
    eprintln!("{} images, gpu {}", ids.len(), det.on_gpu());
    identities(dir, &ours);
}

fn read_f32(path: &Path) -> Option<Vec<f32>> {
    let b = std::fs::read(path).ok()?;
    Some(
        b.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

/// Groups the identity photos' faces (the one matching the reference's single face) at a
/// few thresholds, for the script's pairwise precision and recall.
fn identities(dir: &Path, ours: &Path) {
    let Ok(list) = std::fs::read_to_string(dir.join("identity.tsv")) else {
        return;
    };
    let thresholds: Vec<f32> = std::env::var("FACE_THRESH")
        .unwrap_or_else(|_| "0.25,0.3,0.35,0.4,0.45,0.5".into())
        .split(',')
        .map(|t| t.parse().unwrap())
        .collect();
    for kind in ["sface", "aura"] {
        let mut vecs: Vec<Vec<f32>> = Vec::new();
        let mut rows = String::new();
        for line in list.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let rb: Vec<f32> = f[2..6].iter().map(|v| v.parse().unwrap()).collect();
            let Ok(faces) = std::fs::read_to_string(ours.join(format!("{}.tiled.tsv", f[0])))
            else {
                continue;
            };
            let Some(emb) = read_f32(&ours.join(format!("{}.{kind}.f32", f[0]))) else {
                continue;
            };
            let n = faces.lines().count();
            if n == 0 || emb.len() % n != 0 {
                continue;
            }
            let d = emb.len() / n;
            let best = faces
                .lines()
                .enumerate()
                .map(|(i, l)| {
                    let v: Vec<f32> = l.split('\t').map(|x| x.parse().unwrap()).collect();
                    let iou = specular_infer::face::blazeface::iou(
                        &[v[1], v[2], v[3], v[4]],
                        &[rb[0], rb[1], rb[2], rb[3]],
                    );
                    (i, iou)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, iou)) = best
                && iou >= 0.5
            {
                vecs.push(emb[i * d..(i + 1) * d].to_vec());
                rows += &format!("{}\t{}\n", f[0], f[1]);
            }
        }
        if vecs.is_empty() {
            continue;
        }
        write_f32(&ours.join(format!("identity.{kind}.f32")), &vecs.concat());
        std::fs::write(ours.join(format!("identity.{kind}.tsv")), rows).unwrap();
        for &t in &thresholds {
            let l = specular_infer::face::cluster::agglomerate(&vecs, t);
            let s: Vec<String> = l.iter().map(|x| x.to_string()).collect();
            let name = format!(
                "identity.{kind}.clusters.{}.tsv",
                format!("{t:.2}").replace('.', "_")
            );
            std::fs::write(ours.join(name), s.join("\n")).unwrap();
        }
    }
}

/// Detect and embed timings on a 24 MP photo (a group photo of the oracle set enlarged to
/// 6000×4000), skipped unless `FACE_TIME_24MP` names one of its `img/<id>.rgba`.
#[test]
fn face_timing_24mp() {
    let (Some(dir), Some(models), Ok(id)) = (
        std::env::var_os("SPECULAR_FACE_ORACLE"),
        std::env::var_os("SPECULAR_FACE_MODELS"),
        std::env::var("FACE_TIME_24MP"),
    ) else {
        return;
    };
    let (dir, models) = (Path::new(&dir), Path::new(&models));
    let ctx = common::ctx();
    let read = |f: &str| std::fs::read(models.join(f)).unwrap();
    let raw = std::fs::read(dir.join("img").join(format!("{id}.rgba"))).unwrap();
    let w = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
    let h = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    let src = Image::new(w, h, &raw[8..]).unwrap();
    let big = specular_infer::face::resample(src, 6000, 4000);
    let img = big.view();
    let mut det =
        Detector::new(&read("blaze_face_full_range.tflite"), Range::Full, ctx, 4).unwrap();
    let mut sface = Embedder::new(
        &read("face_recognition_sface_2021dec.onnx"),
        Kind::Sface,
        true,
        ctx,
        4,
    )
    .unwrap();
    // Warm up (shader compilation, buffers).
    let _ = det.detect(img).unwrap();
    for round in 0..3 {
        let t = Instant::now();
        let faces = det.detect(img).unwrap();
        let td = t.elapsed();
        let t = Instant::now();
        for f in &faces {
            sface.embed(img, f).unwrap();
        }
        let te = t.elapsed();
        eprintln!(
            "24 MP round {round}: {} faces, detect {:.0} ms, embed {:.0} ms ({:.1} ms per face), gpu {}",
            faces.len(),
            td.as_secs_f64() * 1e3,
            te.as_secs_f64() * 1e3,
            te.as_secs_f64() * 1e3 / faces.len().max(1) as f64,
            det.on_gpu()
        );
    }
}
