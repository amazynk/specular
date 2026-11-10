//! OCR: small random-weight versions of every PaddleOCR network family on the GPU against
//! the CPU reference (always), and the real exports against Hugging Face transformers' ports
//! (when the files and the oracle are there):
//!
//! ```text
//! SPECULAR_OCR_DIR=target/ocr tools/corpus/fetch-ocr.sh        # the pinned exports
//! SPECULAR_OCR_DIR=target/ocr SPECULAR_OCR_PYTHON=target/ocr-venv/bin/python \
//!     cargo test -p specular-infer --test ocr -- --nocapture
//! ```
//!
//! `SPECULAR_OCR_DIR` defaults to `target/ocr` under the workspace; each model is a folder
//! named as its Hugging Face repository. Without them the real-model tests say so and pass.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use common::{check, contexts};
use specular_infer::ocr::lines::{Img8, Rgba, crop_quad};
use specular_infer::ocr::paddle::{self, Arch, Bank, Input};
use specular_infer::ocr::tess::Tesseract;
use specular_infer::ocr::{Detector, PaddleNet, Recognizer, Runner, Widths};
use specular_infer::tensor::{Rng, compare};
use specular_infer::{Feed, Tensor, cpu};

const DET: &str = r#"{"model_type":"pp_ocrv6_small_det","reduction":4,
 "layer_list_out_channels":[16,24,32,40],"neck_out_channels":16,"kernel_list":[3,2,2],
 "dilated_kernel_size":7,"backbone_config":{"model_type":"pp_lcnet_v4",
 "stem_channels":[3,8,16],"stem_type":"large","block_configs":[
 [[3,16,16,1,true],[3,16,16,1,false]],[[3,16,24,2,false]],[[3,24,32,2,true]],[[3,32,40,2,false]]]}}"#;

const REC_V6: &str = r#"{"model_type":"pp_ocrv6_small_rec","hidden_act":"silu",
 "hidden_size":16,"num_attention_heads":2,"depth":1,"head_out_channels":11,
 "conv_kernel_size":[1,7],"backbone_config":{"model_type":"pp_lcnet_v4",
 "stem_channels":[3,8,16],"stem_type":"large","block_configs":[
 [[3,16,16,1,true]],[[3,16,16,1,false]],[[3,16,24,[2,1],false]],[[3,24,32,[2,1],true]]]}}"#;

const REC_TINY: &str = r#"{"model_type":"pp_ocrv6_tiny_rec","hidden_size":8,
 "head_out_channels":11,"backbone_config":{"model_type":"pp_lcnet_v4",
 "stem_channels":[3,8,16],"stem_type":"small","block_configs":[
 [[3,16,16,1,true]],[[3,16,16,1,false]],[[3,16,24,[2,1],false]],[[3,24,32,[2,1],false]]]}}"#;

const REC_V5: &str = r#"{"model_type":"pp_ocrv5_mobile_rec","hidden_size":16,
 "num_attention_heads":2,"depth":1,"head_out_channels":11,"conv_kernel_size":[1,3],
 "backbone_config":{"model_type":"pp_lcnet_v3","scale":1.0,"divisor":8,"conv_symmetric_num":2,
 "block_configs":[[[3,16,16,1,false]],[[3,16,16,1,false]],[[3,16,32,[2,1],false]],
 [[3,32,32,[1,2],false]],[[5,32,64,[2,1],true],[5,64,64,[2,1],false]]]}}"#;

fn noise(n: usize, seed: u64, a: f32) -> Vec<f32> {
    let mut r = Rng::new(seed);
    (0..n).map(|_| r.sym(a)).collect()
}

#[test]
fn configs_parse() {
    let a = Arch::parse(DET.as_bytes()).unwrap();
    assert!(a.is_detector());
    for (j, classes) in [(REC_V6, 11), (REC_TINY, 11), (REC_V5, 11)] {
        let a = Arch::parse(j.as_bytes()).unwrap();
        assert_eq!(a.classes(), classes);
    }
    assert!(Arch::parse(br#"{"model_type":"pp_ocrv6_small_det"}"#).is_err());
    assert!(Arch::parse(b"not json").is_err());
}

#[test]
fn tiny_detector_gpu_matches_cpu() {
    let arch = Arch::parse(DET.as_bytes()).unwrap();
    let bank = Bank::random(3);
    for f16 in [false, true] {
        // From an image (the app's path) and from a tensor.
        let g = paddle::build(
            &arch,
            &bank,
            Input::Image {
                height: 64,
                width: 96,
            },
            f16,
        )
        .unwrap();
        let mut r = Rng::new(9);
        let (w, h) = (75u32, 41u32);
        let rgba: Vec<u8> = (0..w * h * 4).map(|_| r.below(256) as u8).collect();
        let feeds = [Feed::Rgba {
            width: w,
            height: h,
            data: &rgba,
        }];
        let (out, d) = check(&format!("det image f16 {f16}"), &g, &feeds, 2e-4, 2e-3);
        assert_eq!(out[0].shape, [1, 1, 64, 96]);
        eprintln!("det f16 {f16}: max diff {:.2e}", d.max_abs);
        let g = paddle::build(
            &arch,
            &bank,
            Input::Tensor {
                batch: 2,
                height: 32,
                width: 64,
            },
            f16,
        )
        .unwrap();
        let x = Tensor::f32(&[2, 3, 32, 64], noise(2 * 3 * 32 * 64, 4, 1.5));
        check(
            &format!("det tensor f16 {f16}"),
            &g,
            &[Feed::Tensor(&x)],
            2e-4,
            2e-3,
        );
    }
}

#[test]
fn tiny_recognisers_gpu_match_cpu() {
    for (name, j) in [("v6", REC_V6), ("tiny", REC_TINY), ("v5", REC_V5)] {
        let arch = Arch::parse(j.as_bytes()).unwrap();
        let bank = Bank::random(11);
        for f16 in [false, true] {
            let g = paddle::build(
                &arch,
                &bank,
                Input::Tensor {
                    batch: 3,
                    height: 48,
                    width: 64,
                },
                f16,
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            let x = Tensor::f32(&[3, 3, 48, 64], noise(3 * 3 * 48 * 64, 5, 1.0));
            let (out, d) = check(
                &format!("rec {name} f16 {f16}"),
                &g,
                &[Feed::Tensor(&x)],
                1e-5,
                2e-3,
            );
            assert_eq!(out[0].shape, [3, 8, 11], "{name}");
            // Probabilities.
            for row in out[0].to_f32().chunks(11) {
                assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-4);
            }
            eprintln!("rec {name} f16 {f16}: max diff {:.2e}", d.max_abs);
        }
    }
}

#[test]
fn recogniser_batches_read_like_single_lines() {
    let arch = Arch::parse(REC_V6.as_bytes()).unwrap();
    let chars: Vec<String> = std::iter::once("blank".to_owned())
        .chain("abcdefghij".chars().map(String::from))
        .collect();
    let mut rec = Recognizer::new(PaddleNet::new(arch, Bank::random(2), false), chars).unwrap();
    let mut r = Rng::new(1);
    let lines: Vec<Img8> = [(90, 30), (40, 20), (200, 30), (60, 30), (70, 31)]
        .iter()
        .map(|&(w, h)| Img8 {
            width: w,
            height: h,
            channels: 3,
            data: (0..w * h * 3).map(|_| r.below(256) as u8).collect(),
        })
        .collect();
    let runner = Runner::Cpu(2);
    let all = rec.read(&runner, &lines).unwrap();
    for (i, l) in lines.iter().enumerate() {
        let one = rec.read(&runner, std::slice::from_ref(l)).unwrap();
        assert_eq!(one[0].text, all[i].text, "line {i}");
    }
}

/// Where the real exports are.
fn ocr_dir() -> PathBuf {
    std::env::var_os("SPECULAR_OCR_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ocr"))
}

fn model(name: &str) -> Option<PathBuf> {
    let d = ocr_dir().join(name);
    if d.join("model.safetensors").is_file() && d.join("config.json").is_file() {
        Some(d)
    } else {
        eprintln!("{} not there: real-model check skipped", d.display());
        None
    }
}

const REAL: [&str; 6] = [
    "PP-OCRv6_small_det_safetensors",
    "PP-OCRv6_small_rec_safetensors",
    "PP-OCRv6_tiny_rec_safetensors",
    "arabic_PP-OCRv5_mobile_rec_safetensors",
    "PP-OCRv5_mobile_det_safetensors",
    "PP-LCNet_x1_0_textline_ori_safetensors",
];

/// A smooth input in the normalised range: a few blurred bars on a light ground.
fn smooth_input(n: usize, h: usize, w: usize, seed: u64) -> Tensor {
    let mut r = Rng::new(seed);
    let mut v = vec![0f32; n * 3 * h * w];
    for b in 0..n {
        let bars: Vec<(f32, f32, f32)> = (0..12)
            .map(|_| {
                (
                    r.unit() * w as f32,
                    r.unit() * h as f32,
                    2.0 + r.unit() * 6.0,
                )
            })
            .collect();
        for y in 0..h {
            for x in 0..w {
                let mut ink = 0f32;
                for &(bx, by, s) in &bars {
                    let dx = (x as f32 - bx) / (3.0 * s);
                    let dy = (y as f32 - by) / s;
                    ink += (-(dx * dx + dy * dy)).exp();
                }
                let val = (0.9 - ink.min(1.0) * 0.8) * 2.0 - 1.0;
                for c in 0..3 {
                    v[((b * 3 + c) * h + y) * w + x] = val + 0.02 * c as f32;
                }
            }
        }
    }
    Tensor::f32(&[n, 3, h, w], v)
}

#[test]
fn real_models_gpu_match_cpu_and_transformers() {
    let py = std::env::var_os("SPECULAR_OCR_PYTHON");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/ocr-oracle.py");
    let tmp = std::env::temp_dir().join(format!("specular-ocr-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for name in REAL {
        let Some(dir) = model(name) else { continue };
        let net = PaddleNet::open(
            &dir.join("config.json"),
            &dir.join("model.safetensors"),
            false,
        )
        .unwrap();
        let (n, h, w) = if net.arch.is_detector() {
            (1, 160, 224)
        } else if net.arch.is_classifier() {
            (2, 80, 160)
        } else {
            (2, 48, 320)
        };
        let mut net = net;
        let x = smooth_input(n, h, w, 7);
        let input = Input::Tensor {
            batch: n,
            height: h,
            width: w,
        };
        let g = net.graph(input).unwrap().clone();
        let t0 = Instant::now();
        let (want, d) = check(name, &g, &[Feed::Tensor(&x)], 1e-4, 1e-3);
        eprintln!(
            "{name}: GPU vs CPU max diff {:.2e} (cpu+gpu {:?})",
            d.max_abs,
            t0.elapsed()
        );
        if let Some(py) = &py {
            let (fi, fo) = (tmp.join("in.f32"), tmp.join("out.f32"));
            std::fs::write(&fi, bytemuck_f32(&x.to_f32())).unwrap();
            let st = std::process::Command::new(py)
                .arg("-I")
                .arg(&script)
                .args(["tensor"])
                .arg(&dir)
                .arg(&fi)
                .arg(format!("{n},3,{h},{w}"))
                .arg(&fo)
                .status()
                .unwrap();
            assert!(st.success(), "{name}: oracle failed");
            let refv: Vec<f32> = std::fs::read(&fo)
                .unwrap()
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            let got = want[0].to_f32();
            assert_eq!(got.len(), refv.len(), "{name}: output size");
            let d = compare(&got, &refv, 1e-4, 1e-3);
            eprintln!(
                "{name}: ours vs transformers max diff {:.2e} at {} (cos {:.7})",
                d.max_abs, d.at, d.cosine
            );
            assert!(
                d.worst <= 1.0,
                "{name}: {} vs {} at {}",
                got[d.at],
                refv[d.at],
                d.at
            );
            if !net.arch.is_detector() && !net.arch.is_classifier() {
                // Same best class at every step.
                let c = net.arch.classes();
                let am = |v: &[f32]| {
                    v.chunks(c)
                        .map(|r| {
                            r.iter()
                                .enumerate()
                                .max_by(|a, b| a.1.total_cmp(b.1))
                                .unwrap()
                                .0
                        })
                        .collect::<Vec<_>>()
                };
                assert_eq!(am(&got), am(&refv), "{name}: argmax differs");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

fn bytemuck_f32(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

#[test]
fn real_detector_and_recogniser_end_to_end() {
    let (Some(det), Some(rec)) = (model(REAL[0]), model(REAL[1])) else {
        return;
    };
    let mut detector = Detector::new(
        PaddleNet::open(
            &det.join("config.json"),
            &det.join("model.safetensors"),
            true,
        )
        .unwrap(),
    )
    .unwrap();
    let mut recognizer = Recognizer::open(
        &rec.join("config.json"),
        &rec.join("model.safetensors"),
        &rec.join("preprocessor_config.json"),
        true,
    )
    .unwrap();
    recognizer.widths = Widths::Steps;
    // A synthetic page: dark bars where text would be (the detector finds text-like
    // strokes; this checks the plumbing, the oracle script checks quality).
    let (w, h) = (640usize, 480usize);
    let mut img = vec![255u8; w * h * 4];
    for (y0, x0, x1) in [(60, 40, 400), (160, 80, 560), (300, 40, 300)] {
        for y in y0..y0 + 24 {
            for x in x0..x1 {
                if (x / 6) % 3 != 0 && (y - y0) > 3 && (y - y0) < 21 {
                    let p = (y * w + x) * 4;
                    img[p..p + 3].copy_from_slice(&[20, 20, 20]);
                }
            }
        }
    }
    let rgba = Rgba {
        width: w,
        height: h,
        data: &img,
    };
    let mut runners = vec![Runner::Cpu(4)];
    if let Some(c) = contexts().first() {
        runners.push(Runner::Gpu(c.clone()));
    }
    let mut seen = Vec::new();
    for runner in &runners {
        let t0 = Instant::now();
        let boxes = detector.detect(runner, &rgba).unwrap();
        let t1 = t0.elapsed();
        let crops: Vec<Img8> = boxes
            .iter()
            .filter_map(|b| crop_quad(&rgba, b.pts))
            .collect();
        let texts = recognizer.read(runner, &crops).unwrap();
        assert_eq!(texts.len(), crops.len());
        eprintln!(
            "{runner:?}: {} boxes in {t1:?}, read in {:?}",
            boxes.len(),
            t0.elapsed() - t1
        );
        seen.push(boxes.len());
    }
    assert!(seen.iter().all(|&n| n == seen[0]), "{seen:?}");
}

#[test]
fn real_tesseract_hebrew_loads() {
    let p = ocr_dir().join("tessdata_best/heb.traineddata");
    if !p.is_file() {
        eprintln!("{} not there: skipped", p.display());
        return;
    }
    let t = Tesseract::open(&p).unwrap();
    assert_eq!(t.height, 36);
    assert!(t.spec.contains("Lfx"), "{}", t.spec);
    assert!(t.unichars.iter().any(|u| u == "א"));
    // A blank line reads as nothing.
    let line = Img8 {
        width: 120,
        height: 36,
        channels: 1,
        data: vec![255; 120 * 36],
    };
    let d = t.read_line(&line).unwrap();
    assert!(d.text.trim().is_empty(), "{:?}", d.text);
    let _ = cpu::run;
}

// ---- Tesseract's LSTM on the GPU ----

use specular_infer::graph::Graph;
use specular_infer::ocr::tess::gpu::{Front, build as tess_build};
use specular_infer::ocr::tess::net::{Act as TAct, Grid, Layer, Matrix, Plumb, TRand};
use specular_infer::tensor::DType;

fn rand_matrix(r: &mut Rng, rows: usize, inputs: usize) -> Matrix {
    Matrix {
        rows,
        cols: inputs + 1,
        w: (0..rows * (inputs + 1))
            .map(|_| r.unit() * 1.6 - 0.8)
            .collect(),
    }
}

fn rand_lstm(r: &mut Rng, ni: usize, ns: usize, summary: bool) -> Layer {
    Layer::Lstm {
        gates: Box::new([
            rand_matrix(r, ns, ni + ns),
            rand_matrix(r, ns, ni + ns),
            rand_matrix(r, ns, ni + ns),
            rand_matrix(r, ns, ni + ns),
        ]),
        ni,
        ns,
        summary,
    }
}

#[test]
fn lstm_op_gpu_matches_cpu() {
    let mut r = Rng::new(5);
    // 137 and 800 states (the step's workgroup memory sized to the states).
    for (reverse, summary, lens, ns) in [
        (false, false, false, 137),
        (true, false, true, 137),
        (false, true, false, 137),
        (false, false, true, 137),
        (true, false, true, 800),
        (false, true, false, 800),
    ] {
        let (s, t) = (6, 11);
        let mut g = Graph::new();
        let pre = g.input(&[s, t, 4 * ns], DType::F32).unwrap();
        let wh = g
            .constant(Tensor::f32(
                &[ns, 4 * ns],
                (0..4 * ns * ns).map(|_| r.unit() * 0.4 - 0.2).collect(),
            ))
            .unwrap();
        let l = g.input(&[3], DType::F32).unwrap();
        let y = g
            .lstm(pre, wh, lens.then_some((l, 2)), reverse, summary)
            .unwrap();
        g.output(y).unwrap();
        let x = Tensor::f32(&[s, t, 4 * ns], noise(s * t * 4 * ns, 9, 2.0));
        let lt = Tensor::f32(&[3], vec![11.0, 4.0, 0.0]);
        let (_, d) = check(
            &format!("lstm reverse={reverse} summary={summary} lens={lens} ns={ns}"),
            &g,
            &[Feed::Tensor(&x), Feed::Tensor(&lt)],
            1e-5,
            1e-4,
        );
        eprintln!(
            "lstm {reverse} {summary} {lens} {ns}: max diff {:.2e}",
            d.max_abs
        );
    }
}

/// A small Tesseract-shaped network (convolution, tanh, max-pool, summarising y LSTM, x
/// LSTMs both ways, softmax) with random weights.
fn small_tess(r: &mut Rng) -> Layer {
    Layer::Plumbing {
        kind: Plumb::Series,
        layers: vec![
            Layer::Input {
                height: 9,
                depth: 1,
            },
            Layer::Convolve {
                half_x: 1,
                half_y: 1,
            },
            Layer::Full {
                act: TAct::Tanh,
                m: rand_matrix(r, 4, 9),
            },
            Layer::Maxpool { sx: 3, sy: 3 },
            Layer::Transpose(Box::new(rand_lstm(r, 4, 5, true))),
            rand_lstm(r, 5, 6, false),
            Layer::ReverseX(Box::new(rand_lstm(r, 6, 6, false))),
            rand_lstm(r, 6, 7, false),
            Layer::Full {
                act: TAct::Softmax,
                m: rand_matrix(r, 5, 7),
            },
        ],
    }
}

#[test]
fn tesseract_graph_matches_the_cpu_forward_pass() {
    let mut r = Rng::new(11);
    let net = small_tess(&mut r);
    let front = Front::split(&net).unwrap();
    assert_eq!(front.cpu.len(), 2);
    let widths = [20usize, 7, 33];
    let lines: Vec<Grid> = widths
        .iter()
        .map(|&w| {
            let mut g = Grid::new(9, w, 1);
            for v in &mut g.v {
                *v = r.unit() * 2.0 - 1.0;
            }
            g
        })
        .collect();
    let (pad_w, n) = (36usize, 4usize);
    let mut x = vec![0f32; n * 9 * pad_w * 9];
    let mut lens = vec![0f32; n];
    let mut want = Vec::new();
    for (j, l) in lines.iter().enumerate() {
        want.push(net.forward(l.clone(), &mut TRand::for_line(3)).unwrap());
        let f = front.run_cpu(l.clone(), &mut TRand::for_line(3)).unwrap();
        for y in 0..9 {
            let src = &f.v[y * f.w * 9..(y + 1) * f.w * 9];
            let at = (j * 9 + y) * pad_w * 9;
            x[at..at + src.len()].copy_from_slice(src);
        }
        lens[j] = (f.w / 3) as f32;
    }
    let g = tess_build(&front, 3, n, 9, pad_w, 9).unwrap();
    let xt = Tensor::f32(&[n, 9, pad_w, 9], x);
    let lt = Tensor::f32(&[n], lens.clone());
    let (got, d) = check(
        "small tesseract",
        &g,
        &[Feed::Tensor(&xt), Feed::Tensor(&lt)],
        1e-5,
        1e-4,
    );
    eprintln!("small tesseract: GPU vs graph CPU {:.2e}", d.max_abs);
    let v = got[0].to_f32();
    let steps = pad_w / 3;
    let mut worst = 0f32;
    for (j, w) in want.iter().enumerate() {
        let len = lens[j] as usize;
        assert_eq!(w.w, len);
        for (a, b) in w.v.iter().zip(&v[j * steps * 5..(j * steps + len) * 5]) {
            worst = worst.max((a - b).abs());
        }
    }
    eprintln!("small tesseract: graph vs Layer::forward {worst:.2e}");
    assert!(worst < 1e-5, "{worst}");
}

fn read_pgm(p: &Path) -> Img8 {
    let b = std::fs::read(p).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).into_owned());
    }
    assert_eq!(fields[0], "P5");
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    Img8 {
        width: w,
        height: h,
        channels: 1,
        data: b[i + 1..i + 1 + w * h].to_vec(),
    }
}

/// Hebrew lines (`SPECULAR_TESS_LINES`: a folder of PGM line images) through Tesseract's
/// network on the GPU and the CPU.
#[test]
fn real_tesseract_gpu_matches_cpu() {
    let p = ocr_dir().join("tessdata_best/heb.traineddata");
    let Some(dir) = std::env::var_os("SPECULAR_TESS_LINES").map(PathBuf::from) else {
        eprintln!("SPECULAR_TESS_LINES not set: skipped");
        return;
    };
    let Some(ctx) = contexts().first() else {
        eprintln!("no GPU: skipped");
        return;
    };
    if !p.is_file() {
        eprintln!("{} not there: skipped", p.display());
        return;
    }
    let mut t = Tesseract::open(&p).unwrap();
    let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pgm"))
        .collect();
    names.sort();
    let lines: Vec<Img8> = names.iter().map(|p| read_pgm(p)).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let t0 = Instant::now();
    let cpu = t.read(&lines, threads);
    let t_cpu = t0.elapsed();
    // Warm up (graphs built), then time.
    let t1 = Instant::now();
    let _ = t.read_gpu(ctx, &lines, threads).unwrap();
    let t_first = t1.elapsed();
    let t1 = Instant::now();
    let gpu = t.read_gpu(ctx, &lines, threads).unwrap();
    let t_gpu = t1.elapsed();
    let mut worst = 0f32;
    let (probs, _) = t.probs_gpu(ctx, &lines, threads).unwrap();
    for (l, pg) in lines.iter().zip(&probs) {
        let Some(pg) = pg else { continue };
        let (pc, _) = t.probs(l).unwrap();
        assert_eq!(pc.len(), pg.len());
        for (a, b) in pc.iter().zip(pg) {
            worst = worst.max((a - b).abs());
        }
    }
    let same = cpu
        .iter()
        .zip(&gpu)
        .filter(|(a, b)| a.text == b.text)
        .count();
    eprintln!(
        "tesseract heb: {} lines, CPU ({threads} threads) {t_cpu:?}, GPU {t_gpu:?} \
         (first run with graph builds {t_first:?}); probabilities max abs {worst:.2e}; \
         same text {same}/{}",
        lines.len(),
        lines.len()
    );
    assert!(worst <= 2e-5, "{worst}");
    assert_eq!(same, lines.len());
}

// ---- Text-line orientation ----

fn read_ppm(p: &Path) -> Img8 {
    let b = std::fs::read(p).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).into_owned());
    }
    let ch = if fields[0] == "P6" { 3 } else { 1 };
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    Img8 {
        width: w,
        height: h,
        channels: ch,
        data: b[i + 1..i + 1 + w * h * ch].to_vec(),
    }
}

/// Synthetic lines upright and upside down (`SPECULAR_ORI_LINES`, made by
/// tools/corpus/ocr-orient.py): the classifier turns the upside-down ones, against
/// transformers' port on the same inputs, and the Latin/Chinese ones read as upright.
#[test]
fn real_orientation_turns_upside_down_lines() {
    let Some(dir) = std::env::var_os("SPECULAR_ORI_LINES").map(PathBuf::from) else {
        eprintln!("SPECULAR_ORI_LINES not set: skipped");
        return;
    };
    let (Some(cls), Some(rec)) = (
        model("PP-LCNet_x1_0_textline_ori_safetensors"),
        model(REAL[1]),
    ) else {
        return;
    };
    let truth = std::fs::read_to_string(dir.join("truth.tsv")).unwrap();
    let rows: Vec<(String, String, String)> = truth
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            Some((p.next()?.into(), p.next()?.into(), p.next()?.into()))
        })
        .collect();
    let up: Vec<Img8> = rows
        .iter()
        .map(|r| read_ppm(&dir.join(format!("{}.ppm", r.0))))
        .collect();
    let down: Vec<Img8> = rows
        .iter()
        .map(|r| read_ppm(&dir.join(format!("{}-r.ppm", r.0))))
        .collect();
    let mut ori = specular_infer::ocr::Orientation::open(
        &cls.join("config.json"),
        &cls.join("model.safetensors"),
        true,
    )
    .unwrap();
    let runner = match contexts().first() {
        Some(c) => Runner::Gpu(c.clone()),
        None => Runner::Cpu(4),
    };
    let t0 = Instant::now();
    let pu = ori.upside_down(&runner, &up).unwrap();
    let pd = ori.upside_down(&runner, &down).unwrap();
    let t = t0.elapsed();
    let th = specular_infer::ocr::UPSIDE_DOWN;
    let up_ok = pu.iter().filter(|&&p| p < th).count();
    let down_ok = pd.iter().filter(|&&p| p >= th).count();
    eprintln!(
        "orientation: upright kept {up_ok}/{n}, upside down turned {down_ok}/{n} ({:?} for {} lines, {runner:?})",
        t,
        2 * rows.len(),
        n = rows.len()
    );
    // Per script.
    let mut scripts: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    scripts.sort_unstable();
    scripts.dedup();
    for s in &scripts {
        let idx: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].1 == *s).collect();
        let u = idx.iter().filter(|&&i| pu[i] < th).count();
        let d = idx.iter().filter(|&&i| pd[i] >= th).count();
        eprintln!("  {s}: upright {u}/{n}, upside down {d}/{n}", n = idx.len());
    }
    assert!(up_ok * 100 >= rows.len() * 90, "{up_ok}");
    assert!(down_ok * 100 >= rows.len() * 80, "{down_ok}");

    // The same inputs through transformers' port: same decisions.
    if let Some(py) = std::env::var_os("SPECULAR_OCR_PYTHON") {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/ocr-oracle.py");
        let tmp = std::env::temp_dir().join(format!("specular-ori-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let all: Vec<&Img8> = up.iter().chain(&down).collect();
        let x = specular_infer::ocr::lines::cls_batch(&all);
        let (fi, fo) = (tmp.join("in.f32"), tmp.join("out.f32"));
        std::fs::write(&fi, bytemuck_f32(&x.to_f32())).unwrap();
        let st = std::process::Command::new(py)
            .arg("-I")
            .arg(&script)
            .arg("tensor")
            .arg(&cls)
            .arg(&fi)
            .arg(format!("{},3,80,160", all.len()))
            .arg(&fo)
            .status()
            .unwrap();
        assert!(st.success());
        let refv: Vec<f32> = std::fs::read(&fo)
            .unwrap()
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let ours: Vec<f32> = pu.iter().chain(&pd).copied().collect();
        let mut worst = 0f32;
        let mut differ = 0;
        for (o, r) in ours.iter().zip(refv.chunks_exact(2)) {
            worst = worst.max((o - r[1]).abs());
            if (*o >= th) != (r[1] >= th) {
                differ += 1;
            }
        }
        eprintln!(
            "orientation vs transformers: max abs {worst:.2e}, decisions differ {differ}/{}",
            ours.len()
        );
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(worst < 1e-2, "{worst}");
    }

    // Reading the Latin and Chinese lines turned: with the classifier as without the turn.
    let mut recognizer = Recognizer::open(
        &rec.join("config.json"),
        &rec.join("model.safetensors"),
        &rec.join("preprocessor_config.json"),
        true,
    )
    .unwrap();
    let idx: Vec<usize> = (0..rows.len())
        .filter(|&i| matches!(rows[i].1.as_str(), "en" | "latin" | "zh"))
        .collect();
    let pick = |v: &[Img8]| idx.iter().map(|&i| v[i].clone()).collect::<Vec<_>>();
    let exact = |got: &[specular_infer::ocr::ctc::Decoded]| {
        got.iter()
            .zip(&idx)
            .filter(|(d, i)| d.text.trim() == rows[**i].2)
            .count()
    };
    let upright = exact(&recognizer.read(&runner, &pick(&up)).unwrap());
    let raw = exact(&recognizer.read(&runner, &pick(&down)).unwrap());
    let mut fixed = pick(&down);
    let turned = ori.straighten(&runner, &mut fixed).unwrap();
    let after = exact(&recognizer.read(&runner, &fixed).unwrap());
    eprintln!(
        "reading {} upside-down lines: {raw} exact as they are, {after} after turning {turned} \
         (upright: {upright})",
        idx.len()
    );
    assert!(after > raw);
    assert!(after * 100 >= upright * 90, "{after} vs {upright}");
}

/// The rule the app reads lines by (`specular_ai::ocr`): a line the classifier gives at least
/// [`BOTH_WAYS`] of being upside down is read as it is and turned round, by the main
/// recogniser and its script's add-on, and the more confident reading is kept. On the same
/// synthetic lines as above (`SPECULAR_ORI_LINES`): per script, upright lines kept upright
/// and upside-down lines read turned, at `BOTH_WAYS` and at the classifier's own 0.9.
///
/// [`BOTH_WAYS`]: specular_infer::ocr::BOTH_WAYS
#[test]
fn real_orientation_reads_unsure_lines_both_ways() {
    let Some(dir) = std::env::var_os("SPECULAR_ORI_LINES").map(PathBuf::from) else {
        eprintln!("SPECULAR_ORI_LINES not set: skipped");
        return;
    };
    let Some(cls) = model("PP-LCNet_x1_0_textline_ori_safetensors") else {
        return;
    };
    // The recogniser each script's add-on installs (the main one reads the rest).
    let addon = |s: &str| match s {
        "ar" => Some("arabic_PP-OCRv5_mobile_rec_safetensors"),
        "ko" => Some("korean_PP-OCRv5_mobile_rec_safetensors"),
        "el" => Some("el_PP-OCRv5_mobile_rec_safetensors"),
        "ru" => Some("eslav_PP-OCRv5_mobile_rec_safetensors"),
        _ => None,
    };
    let Some(main) = model(REAL[1]) else { return };
    let truth = std::fs::read_to_string(dir.join("truth.tsv")).unwrap();
    let rows: Vec<(String, String)> = truth
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            Some((p.next()?.into(), p.next()?.into()))
        })
        .collect();
    let runner = match contexts().first() {
        Some(c) => Runner::Gpu(c.clone()),
        None => Runner::Cpu(4),
    };
    let open = |d: &Path| {
        Recognizer::open(
            &d.join("config.json"),
            &d.join("model.safetensors"),
            &d.join("preprocessor_config.json"),
            true,
        )
        .unwrap()
    };
    let mut ori = specular_infer::ocr::Orientation::open(
        &cls.join("config.json"),
        &cls.join("model.safetensors"),
        true,
    )
    .unwrap();
    let mut main = open(&main);
    let mut scripts: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    scripts.sort_unstable();
    scripts.dedup();
    let both = specular_infer::ocr::BOTH_WAYS;
    let below = specular_infer::ocr::BOTH_WAYS_BELOW;
    // (band, gate): a line flagged at the band whose upright reading is less confident than
    // the gate is read turned too. The app's rule is (BOTH_WAYS, BOTH_WAYS_BELOW).
    let mut bands = vec![
        (specular_infer::ocr::UPSIDE_DOWN, f32::INFINITY),
        (both, f32::INFINITY),
        (both, below),
    ];
    let list = |var: &str| -> Vec<f32> {
        std::env::var_os(var).map_or(Vec::new(), |v| {
            v.to_string_lossy()
                .split(',')
                .filter_map(|t| t.parse::<f32>().ok())
                .collect()
        })
    };
    bands.extend(
        list("SPECULAR_ORI_BANDS")
            .into_iter()
            .map(|b| (b, f32::INFINITY)),
    );
    bands.extend(list("SPECULAR_ORI_GATES").into_iter().map(|g| (both, g)));
    // Per band: (upright kept, turned caught, upright lines read twice) over all scripts.
    let mut total = vec![(0usize, 0usize, 0usize); bands.len()];
    let (mut n_all, mut ms_one, mut ms_two) = (0usize, 0f64, 0f64);
    for s in &scripts {
        let idx: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].1 == *s).collect();
        let up: Vec<Img8> = idx
            .iter()
            .map(|&i| read_ppm(&dir.join(format!("{}.ppm", rows[i].0))))
            .collect();
        let down: Vec<Img8> = idx
            .iter()
            .map(|&i| read_ppm(&dir.join(format!("{}-r.ppm", rows[i].0))))
            .collect();
        let mut recs = vec![&mut main];
        let mut own = addon(s).and_then(model).map(|d| open(&d));
        if let Some(r) = own.as_mut() {
            recs.push(r);
        }
        // Confidence of the best reading of each line, as it is and turned.
        let best = |lines: &[Img8], recs: &mut [&mut Recognizer]| -> Vec<f32> {
            let mut b = vec![0f32; lines.len()];
            for r in recs.iter_mut() {
                for (k, d) in r.read(&runner, lines).unwrap().into_iter().enumerate() {
                    if !d.text.trim().is_empty() {
                        b[k] = b[k].max(d.confidence);
                    }
                }
            }
            b
        };
        let turn = |v: &[Img8]| v.iter().map(Img8::rot180).collect::<Vec<_>>();
        let t0 = Instant::now();
        let c_up = best(&up, &mut recs);
        ms_one += t0.elapsed().as_secs_f64() * 1e3;
        let t0 = Instant::now();
        let c_up_t = best(&turn(&up), &mut recs);
        ms_two += t0.elapsed().as_secs_f64() * 1e3;
        let c_down = best(&down, &mut recs);
        let c_down_t = best(&turn(&down), &mut recs);
        let pu = ori.upside_down(&runner, &up).unwrap();
        let pd = ori.upside_down(&runner, &down).unwrap();
        n_all += idx.len();
        let mut line = format!("  {s}:");
        for (b, &(th, gate)) in bands.iter().enumerate() {
            // Read turned: flagged and unsure, and the turned reading the more confident.
            let kept = (0..idx.len())
                .filter(|&k| !(pu[k] >= th && c_up[k] < gate && c_up_t[k] > c_up[k]))
                .count();
            let caught = (0..idx.len())
                .filter(|&k| pd[k] >= th && c_down[k] < gate && c_down_t[k] > c_down[k])
                .count();
            let twice = (0..idx.len())
                .filter(|&k| pu[k] >= th && c_up[k] < gate)
                .count();
            total[b].0 += kept;
            total[b].1 += caught;
            total[b].2 += twice;
            line += &format!(
                " ≥{th} <{gate}: upright {kept}/{n}, turned {caught}/{n};",
                n = idx.len()
            );
        }
        eprintln!("{line}");
    }
    for (b, (th, gate)) in bands.iter().enumerate() {
        eprintln!(
            "both ways from {th} below confidence {gate}: upright kept {}/{n_all}, turned read turned {}/{n_all}, \
             upright lines read twice {}/{n_all}",
            total[b].0, total[b].1, total[b].2,
        );
    }
    eprintln!(
        "reading {n_all} upright lines once: {ms_one:.0} ms, turned too: +{ms_two:.0} ms \
         ({runner:?})"
    );
    let (kept, caught, _) = total[2];
    assert!(kept * 240 >= 235 * n_all, "upright kept {kept}/{n_all}");
    assert!(
        caught * 240 >= 230 * n_all,
        "turned caught {caught}/{n_all}"
    );
}
