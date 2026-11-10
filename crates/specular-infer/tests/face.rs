//! Face models: the ONNX and TFLite translations, CPU against GPU.
//!
//! - A small ONNX CNN written here (Conv, BatchNormalization, PRelu, Add, GlobalAveragePool,
//!   Flatten, Gemm, Sub/Mul by scalars) against a direct f64 computation.
//! - With `SPECULAR_FACE_MODELS` naming a folder holding the real downloads
//!   (`blaze_face_full_range.tflite`, `face_landmarker.task`,
//!   `face_recognition_sface_2021dec.onnx`, optionally `glintr100.onnx`): each network on the
//!   CPU and the GPU, outputs compared. Skipped when the folder is not given.
//!
//! The comparison against the upstream runtimes (LiteRT, ONNX Runtime, MediaPipe) is
//! `tools/corpus/face-oracle.py` with the `face_oracle` test.

mod common;

use specular_infer::face::blazeface::{Detector, Range};
use specular_infer::face::embed::{Embedder, Kind};
use specular_infer::face::landmarker::Landmarker;
use specular_infer::face::{Image, cosine};
use specular_infer::onnx::Model;
use specular_infer::tensor::Rng;
use specular_infer::{DType, Feed, Graph, Tensor};
use std::time::Instant;

// -- a tiny protobuf writer for the ONNX test model ---------------------------------------

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn bytes(field: u32, b: &[u8], out: &mut Vec<u8>) {
    varint(((field as u64) << 3) | 2, out);
    varint(b.len() as u64, out);
    out.extend_from_slice(b);
}

fn int(field: u32, v: i64, out: &mut Vec<u8>) {
    varint((field as u64) << 3, out);
    varint(v as u64, out);
}

fn tensor(name: &str, dims: &[usize], data: &[f32]) -> Vec<u8> {
    let mut t = Vec::new();
    for &d in dims {
        int(1, d as i64, &mut t);
    }
    int(2, 1, &mut t);
    bytes(8, name.as_bytes(), &mut t);
    let raw: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
    bytes(9, &raw, &mut t);
    t
}

enum A {
    I(i64),
    Is(Vec<i64>),
    F(f32),
}

fn node(op: &str, ins: &[&str], outs: &[&str], attrs: &[(&str, A)]) -> Vec<u8> {
    let mut n = Vec::new();
    for i in ins {
        bytes(1, i.as_bytes(), &mut n);
    }
    for o in outs {
        bytes(2, o.as_bytes(), &mut n);
    }
    bytes(4, op.as_bytes(), &mut n);
    for (name, a) in attrs {
        let mut at = Vec::new();
        bytes(1, name.as_bytes(), &mut at);
        match a {
            A::I(v) => {
                int(3, *v, &mut at);
                int(20, 2, &mut at);
            }
            A::Is(v) => {
                let mut packed = Vec::new();
                for &x in v {
                    varint(x as u64, &mut packed);
                }
                bytes(8, &packed, &mut at);
                int(20, 7, &mut at);
            }
            A::F(v) => {
                varint((2 << 3) | 5, &mut at);
                at.extend_from_slice(&v.to_le_bytes());
                int(20, 1, &mut at);
            }
        }
        bytes(5, &at, &mut n);
    }
    n
}

fn value_info(name: &str, dims: &[usize]) -> Vec<u8> {
    let mut shape = Vec::new();
    for &d in dims {
        let mut dim = Vec::new();
        int(1, d as i64, &mut dim);
        bytes(1, &dim, &mut shape);
    }
    let mut tt = Vec::new();
    int(1, 1, &mut tt);
    bytes(2, &shape, &mut tt);
    let mut ty = Vec::new();
    bytes(1, &tt, &mut ty);
    let mut v = Vec::new();
    bytes(1, name.as_bytes(), &mut v);
    bytes(2, &ty, &mut v);
    v
}

struct Small {
    w1: Vec<f32>,
    b1: Vec<f32>,
    bn: [Vec<f32>; 4],
    slope: Vec<f32>,
    w2: Vec<f32>,
    fc: Vec<f32>,
    fcb: Vec<f32>,
}

const C: usize = 4;
const S: usize = 7;

fn small_model(r: &mut Rng) -> (Vec<u8>, Small) {
    let mut v = |n: usize, a: f32| (0..n).map(|_| r.sym(a)).collect::<Vec<f32>>();
    let s = Small {
        w1: v(C * 3 * 9, 0.4),
        b1: v(C, 0.1),
        bn: [
            v(C, 0.3).iter().map(|x| 1.0 + x).collect(),
            v(C, 0.2),
            v(C, 0.2),
            v(C, 0.3).iter().map(|x| 1.2 + x).collect(),
        ],
        slope: v(C, 0.3).iter().map(|x| 0.25 + x).collect(),
        w2: v(C * 3, 0.5),
        fc: v(5 * C, 0.5),
        fcb: v(5, 0.1),
    };
    let mut g = Vec::new();
    let nodes = [
        node("Sub", &["data", "mean"], &["d0"], &[]),
        node("Mul", &["d0", "scale"], &["d1"], &[]),
        node(
            "Conv",
            &["d1", "w1", "b1"],
            &["c1"],
            &[
                ("kernel_shape", A::Is(vec![3, 3])),
                ("pads", A::Is(vec![1, 1, 1, 1])),
                ("strides", A::Is(vec![1, 1])),
                ("group", A::I(1)),
            ],
        ),
        node(
            "BatchNormalization",
            &["c1", "g", "b", "m", "var"],
            &["n1"],
            &[("epsilon", A::F(1e-3))],
        ),
        node("PRelu", &["n1", "slope"], &["p1"], &[]),
        node(
            "Conv",
            &["d1", "w2"],
            &["c2"],
            &[("kernel_shape", A::Is(vec![1, 1]))],
        ),
        node("Add", &["p1", "c2"], &["a1"], &[]),
        node("Dropout", &["a1"], &["a2"], &[]),
        node("GlobalAveragePool", &["a2"], &["gp"], &[]),
        node("Flatten", &["gp"], &["fl"], &[("axis", A::I(1))]),
        node(
            "Gemm",
            &["fl", "fc", "fcb"],
            &["out"],
            &[("transB", A::I(1))],
        ),
    ];
    for n in &nodes {
        bytes(1, n, &mut g);
    }
    let inits: [(&str, Vec<usize>, &[f32]); 12] = [
        ("mean", vec![1], &[127.5]),
        ("scale", vec![1], &[1.0 / 128.0]),
        ("w1", vec![C, 3, 3, 3], &s.w1),
        ("b1", vec![C], &s.b1),
        ("g", vec![C], &s.bn[0]),
        ("b", vec![C], &s.bn[1]),
        ("m", vec![C], &s.bn[2]),
        ("var", vec![C], &s.bn[3]),
        ("slope", vec![C, 1, 1], &s.slope),
        ("w2", vec![C, 3, 1, 1], &s.w2),
        ("fc", vec![5, C], &s.fc),
        ("fcb", vec![5], &s.fcb),
    ];
    for (name, dims, data) in &inits {
        bytes(5, &tensor(name, dims, data), &mut g);
        bytes(11, &value_info(name, dims), &mut g);
    }
    bytes(11, &value_info("data", &[1, 3, S, S]), &mut g);
    bytes(12, &value_info("out", &[1, 5]), &mut g);
    let mut m = Vec::new();
    int(1, 6, &mut m);
    bytes(7, &g, &mut m);
    let mut op = Vec::new();
    int(2, 11, &mut op);
    bytes(8, &op, &mut m);
    (m, s)
}

/// The small model computed directly in f64.
fn small_reference(s: &Small, x: &[f32]) -> Vec<f64> {
    let at = |c: usize, y: i64, xx: i64| -> f64 {
        if y < 0 || xx < 0 || y >= S as i64 || xx >= S as i64 {
            return 0.0;
        }
        (x[c * S * S + y as usize * S + xx as usize] as f64 - 127.5) / 128.0
    };
    let mut pooled = [0f64; C];
    for o in 0..C {
        for y in 0..S as i64 {
            for xx in 0..S as i64 {
                let mut c1 = s.b1[o] as f64;
                for c in 0..3 {
                    for ky in 0..3 {
                        for kx in 0..3 {
                            c1 += s.w1[((o * 3 + c) * 3 + ky) * 3 + kx] as f64
                                * at(c, y + ky as i64 - 1, xx + kx as i64 - 1);
                        }
                    }
                }
                let [g, b, m, v] = [&s.bn[0], &s.bn[1], &s.bn[2], &s.bn[3]];
                let n =
                    (c1 - m[o] as f64) / (v[o] as f64 + 1e-3).sqrt() * g[o] as f64 + b[o] as f64;
                let p = if n >= 0.0 { n } else { n * s.slope[o] as f64 };
                let c2: f64 = (0..3).map(|c| s.w2[o * 3 + c] as f64 * at(c, y, xx)).sum();
                pooled[o] += (p + c2) / (S * S) as f64;
            }
        }
    }
    (0..5)
        .map(|k| {
            s.fcb[k] as f64
                + (0..C)
                    .map(|o| s.fc[k * C + o] as f64 * pooled[o])
                    .sum::<f64>()
        })
        .collect()
}

#[test]
fn onnx_cnn_matches_a_direct_computation_on_cpu_and_gpu() {
    let mut r = Rng::new(11);
    let (bytes, s) = small_model(&mut r);
    let m = Model::parse(&bytes).unwrap();
    assert_eq!(m.opset, 11);
    assert_eq!(m.inputs[0].shape, [1, 3, S, S]);
    let mut g = Graph::new();
    let x = g.input(&[1, 3, S, S], DType::F32).unwrap();
    let outs = m.build(&mut g, x, false).unwrap();
    g.output(outs[0]).unwrap();
    // The batch norm folded into its conv: no separate multiply-add.
    assert_eq!(
        g.nodes()
            .iter()
            .filter(|n| matches!(n.op, specular_infer::graph::Op::Conv2d(_)))
            .count(),
        2
    );
    let input: Vec<f32> = (0..3 * S * S).map(|_| r.unit() * 255.0).collect();
    let t = Tensor::f32(&[1, 3, S, S], input.clone());
    let want = small_reference(&s, &input);
    let (got, diff) = common::check("onnx small", &g, &[Feed::Tensor(&t)], 1e-4, 1e-4);
    for (a, b) in got[0].to_f32().iter().zip(&want) {
        assert!((*a as f64 - b).abs() < 1e-4, "{a} vs {b}");
    }
    assert!(diff.max_abs < 1e-3 || common::ctx().is_none());
}

#[test]
fn onnx_reader_refuses_bad_files() {
    assert!(Model::parse(b"").is_err());
    assert!(Model::parse(&[0x3a, 0xff, 0xff]).is_err());
    let mut r = Rng::new(1);
    let (bytes, _) = small_model(&mut r);
    for cut in [10, bytes.len() / 2, bytes.len() - 3] {
        assert!(Model::parse(&bytes[..cut]).is_err(), "cut at {cut}");
    }
}

#[test]
fn tflite_reader_refuses_bad_files() {
    use specular_infer::tflite::{Model, zip_entry};
    assert!(Model::parse(b"").is_err());
    assert!(Model::parse(b"\x10\0\0\0TFL3\0\0\0\0\0\0\0\0").is_err());
    assert!(Model::parse(b"\x04\0\0\0XXXX\0\0\0\0").is_err());
    assert!(zip_entry(b"not a zip", "a").is_err());
}

// -- the real models ------------------------------------------------------------------------

fn models() -> Option<std::path::PathBuf> {
    std::env::var_os("SPECULAR_FACE_MODELS").map(Into::into)
}

/// A test picture: a soft pink oval with two dark eyes and a mouth on grey.
fn picture(w: usize, h: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(w * h * 4);
    let (cx, cy, rx, ry) = (
        w as f32 * 0.5,
        h as f32 * 0.5,
        w as f32 * 0.2,
        h as f32 * 0.28,
    );
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
            let mut c = if dx * dx + dy * dy < 1.0 {
                [225, 180, 160]
            } else {
                [120, 125, 130]
            };
            for ex in [-0.4f32, 0.4] {
                let (u, v) = (dx - ex, dy + 0.25);
                if u * u + v * v * 4.0 < 0.03 {
                    c = [40, 30, 30];
                }
            }
            if dy > 0.35 && dy < 0.45 && dx.abs() < 0.35 {
                c = [150, 60, 60];
            }
            v.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    v
}

#[test]
fn real_face_models_agree_on_cpu_and_gpu() {
    let Some(dir) = models() else {
        eprintln!("SPECULAR_FACE_MODELS not set: skipped");
        return;
    };
    let Some(ctx) = common::ctx() else {
        eprintln!("no GPU: skipped");
        return;
    };
    let px = picture(320, 240);
    let img = Image::new(320, 240, &px).unwrap();

    let blaze = std::fs::read(dir.join("blaze_face_full_range.tflite")).unwrap();
    let mut d = Detector::new(&blaze, Range::Full, Some(ctx), 4).unwrap();
    assert!(d.on_gpu());
    let x = d.input(img, Detector::whole(img));
    let (cpu, gpu) = (d.raw(&x, true).unwrap(), d.raw(&x, false).unwrap());
    for (a, b) in cpu.iter().zip(&gpu) {
        let diff = specular_infer::tensor::compare(&b.to_f32(), &a.to_f32(), 1e-3, 1e-3);
        assert!(diff.max_abs < 2e-2, "BlazeFace GPU vs CPU: {diff:?}");
    }

    let task = std::fs::read(dir.join("face_landmarker.task")).unwrap();
    let mut lm = Landmarker::new(&task, Some(ctx), 4).unwrap();
    let f = specular_infer::face::blazeface::Face {
        rect: [100.0, 60.0, 120.0, 130.0],
        points: [
            [135.0, 95.0],
            [185.0, 95.0],
            [160.0, 120.0],
            [160.0, 160.0],
            [100.0, 110.0],
            [220.0, 110.0],
        ],
        score: 1.0,
    };
    let mx = lm.mesh_input(img, &f);
    let (c, cp) = lm.mesh_raw(&mx, true).unwrap();
    let (g, gp) = lm.mesh_raw(&mx, false).unwrap();
    let diff = specular_infer::tensor::compare(&g, &c, 1e-2, 1e-3);
    assert!(
        diff.max_abs < 0.05 && (cp - gp).abs() < 1e-3,
        "FaceMesh: {diff:?}"
    );
    let eyes = lm.eyes(img, &f).unwrap();
    assert!((0.0..=1.0).contains(&eyes.left) && (0.0..=1.0).contains(&eyes.right));

    for (file, kind) in [
        ("face_recognition_sface_2021dec.onnx", Kind::Sface),
        ("glintr100.onnx", Kind::AuraFace),
    ] {
        let Ok(bytes) = std::fs::read(dir.join(file)) else {
            continue;
        };
        let mut e = Embedder::new(&bytes, kind, false, Some(ctx), 4).unwrap();
        let x = e.input(img, &f).unwrap();
        let (a, b) = (e.raw(&x, true).unwrap(), e.raw(&x, false).unwrap());
        let cos = cosine(&a, &b);
        assert!(cos > 0.9999, "{file}: GPU vs CPU cosine {cos}");
        // f16 weights stay within the bar too.
        let mut h = Embedder::new(&bytes, kind, true, Some(ctx), 4).unwrap();
        let c16 = cosine(&a, &h.raw(&x, false).unwrap());
        assert!(c16 > 0.999, "{file}: f16 cosine {c16}");
    }
}

/// Batched embedding (`Embedder::embed_all`) gives what one face at a time gives, faster.
#[test]
fn real_batched_embeddings_match_single() {
    let Some(dir) = models() else {
        eprintln!("SPECULAR_FACE_MODELS not set: skipped");
        return;
    };
    let ctx = common::ctx();
    let px = picture(640, 480);
    let img = Image::new(640, 480, &px).unwrap();
    // 21 faces (two full batches and a partial one) at different places, sizes and rolls.
    let faces: Vec<specular_infer::face::blazeface::Face> = (0..21)
        .map(|i| {
            let (cx, cy) = (80.0 + (i % 7) as f32 * 75.0, 90.0 + (i / 7) as f32 * 130.0);
            let s = 30.0 + (i % 5) as f32 * 6.0;
            let a = (i as f32 - 10.0) * 0.06;
            let p = |dx: f32, dy: f32| {
                [
                    cx + dx * s * a.cos() - dy * s * a.sin(),
                    cy + dx * s * a.sin() + dy * s * a.cos(),
                ]
            };
            specular_infer::face::blazeface::Face {
                rect: [cx - s, cy - s, 2.0 * s, 2.0 * s],
                points: [
                    p(-0.35, -0.3),
                    p(0.35, -0.3),
                    p(0.0, 0.05),
                    p(0.0, 0.4),
                    p(-0.8, -0.1),
                    p(0.8, -0.1),
                ],
                score: 1.0,
            }
        })
        .collect();
    for (file, kind) in [
        ("face_recognition_sface_2021dec.onnx", Kind::Sface),
        ("glintr100.onnx", Kind::AuraFace),
    ] {
        let Ok(bytes) = std::fs::read(dir.join(file)) else {
            continue;
        };
        let mut e = Embedder::new(&bytes, kind, true, ctx, 8).unwrap();
        // Warm up both graphs.
        let _ = e.embed(img, &faces[0]).unwrap();
        let _ = e.embed_all(img, &faces[..8]).unwrap();
        let t0 = Instant::now();
        let xs: Vec<_> = faces.iter().map(|f| e.input(img, f).unwrap()).collect();
        let t_in = t0.elapsed();
        let t0 = Instant::now();
        for c in xs.chunks(8) {
            let _ = e.raw_batch(c).unwrap();
        }
        eprintln!(
            "{file}: inputs {t_in:?}, batched runs alone {:?}",
            t0.elapsed()
        );
        let t0 = Instant::now();
        let single: Vec<Vec<f32>> = faces
            .iter()
            .map(|f| e.embed(img, f).unwrap().unwrap())
            .collect();
        let t_single = t0.elapsed();
        let t0 = Instant::now();
        let batched = e.embed_all(img, &faces).unwrap();
        let t_batch = t0.elapsed();
        let worst = single
            .iter()
            .zip(&batched)
            .map(|(a, b)| cosine(a, b.as_ref().unwrap()))
            .fold(1f32, f32::min);
        let n = faces.len() as f64;
        eprintln!(
            "{file} ({}): one at a time {:.1} faces/s, batches of {} {:.1} faces/s; \
             lowest cosine {worst:.7}",
            if e.on_gpu() { "GPU" } else { "CPU" },
            n / t_single.as_secs_f64(),
            specular_infer::face::embed::BATCH,
            n / t_batch.as_secs_f64()
        );
        assert!(worst >= 0.99995, "{file}: {worst}");
    }
}
