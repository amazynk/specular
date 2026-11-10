//! CLIP towers, GPU against the CPU reference: a tiny random-weight CLIP in both weight
//! layouts, f32 and f16, image (RGBA and batched tensor input) and text.
//!
//! `reference` (ignored; run by hand) checks a real checkpoint against dumps of the upstream
//! implementation, and times it:
//!
//! ```text
//! SPECULAR_CLIP_WEIGHTS=…/open_clip_model.safetensors SPECULAR_CLIP_MERGES=…/merges.txt \
//! SPECULAR_CLIP_REF=<a directory of upstream outputs, see `reference`> \
//!   cargo test --release -p specular-infer --test clip -- --ignored --nocapture
//! ```

mod common;

use std::path::PathBuf;
use std::time::Instant;

use common::{check, ctx};
use specular_infer::bpe::{Bpe, CLIP_MERGES};
use specular_infer::clip::{Clip, ClipConfig, ImageInput, Layout};
use specular_infer::gpu::GpuGraph;
use specular_infer::models::RandomWeights;
use specular_infer::tensor::{Rng, compare};
use specular_infer::{Feed, Tensor, cpu};

/// Merges over a few letters: enough ids for the tiny model's 600-row table.
fn tiny_bpe() -> Bpe {
    let mut m = String::from("#version: 0.2\n");
    let letters = "abcdefghijklmnopqrstuvwxyz";
    let mut n = 0;
    'outer: for a in letters.chars() {
        for b in letters.chars() {
            m += &format!("{a} {b}</w>\n");
            n += 1;
            if n == 86 {
                break 'outer;
            }
        }
    }
    Bpe::from_merges(&m, CLIP_MERGES).unwrap()
}

fn tiny(layout: Layout, input: ImageInput, f16: bool) -> Clip {
    let bpe = tiny_bpe();
    let c = ClipConfig::test(layout, bpe.vocab_size());
    Clip::build(c, &mut RandomWeights::new(21), bpe, input, f16).unwrap()
}

const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];

#[test]
fn tiny_clip_gpu_matches_cpu() {
    for layout in [Layout::OpenClip, Layout::HuggingFace] {
        for f16 in [false, true] {
            let name = format!("{layout:?} f16 {f16}");
            let m = tiny(
                layout,
                ImageInput::Rgba {
                    mean: MEAN,
                    std: STD,
                },
                f16,
            );
            let (w, h) = (45u32, 37u32);
            let mut rng = Rng::new(4);
            let rgba: Vec<u8> = (0..w * h * 4).map(|_| rng.below(256) as u8).collect();
            let feeds = [Feed::Rgba {
                width: w,
                height: h,
                data: &rgba,
            }];
            let (out, d) = check(&format!("{name} image"), &m.image, &feeds, 1e-3, 1e-3);
            assert_eq!(out[0].shape, [1, 32]);
            assert!(d.cosine >= 0.99999, "{name}: {}", d.cosine);
            let norm: f32 = out[0].to_f32().iter().map(|v| v * v).sum();
            assert!((norm - 1.0).abs() < 1e-4);

            let (x, end) = m.text.feeds("a cat on a mat");
            let feeds = [Feed::Tensor(&x), Feed::Tensor(&end)];
            let (out, d) = check(&format!("{name} text"), &m.text.graph, &feeds, 1e-3, 1e-3);
            assert_eq!(out[0].shape, [1, 32]);
            assert!(d.cosine >= 0.99999, "{name}: {}", d.cosine);
        }
    }
}

#[test]
fn tiny_clip_batch_and_causality() {
    let m = tiny(Layout::OpenClip, ImageInput::Tensor { batch: 2 }, false);
    let x = Rng::new(9).tensor(&[2, 3, 32, 32], 1.0);
    let (out, _) = check("batch", &m.image, &[Feed::Tensor(&x)], 1e-3, 1e-3);
    assert_eq!(out[0].shape, [2, 32]);
    // The batch's first image alone gives the same embedding.
    let one = tiny(Layout::OpenClip, ImageInput::Tensor { batch: 1 }, false);
    let x1 = Tensor::f32(&[1, 3, 32, 32], x.to_f32()[..3 * 32 * 32].to_vec());
    let alone = cpu::run(&one.image, &[Feed::Tensor(&x1)], 1).unwrap();
    let d = compare(&alone[0].to_f32(), &out[0].to_f32()[..32], 1e-5, 1e-5);
    assert!(d.worst <= 1.0);

    // Causal: tokens after the end mark do not change the text's embedding.
    let t = &one.text;
    let (ids, end) = t.tokenizer.tokenize("abc", t.context);
    let mut other = ids.clone();
    for id in &mut other[end + 1..] {
        *id = 7;
    }
    let run = |ids: &[u32]| {
        let (x, e) = t.feeds_for_ids(ids, end);
        cpu::run(&t.graph, &[Feed::Tensor(&x), Feed::Tensor(&e)], 1).unwrap()[0].to_f32()
    };
    assert_eq!(run(&ids), run(&other));
    let mut changed = ids.clone();
    changed[1] = 3;
    assert_ne!(run(&ids), run(&changed));
}

// ---------------------------------------------------------------------------------------------
// A real checkpoint against the upstream implementation.

fn env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn f32s(path: &std::path::Path) -> Vec<f32> {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn argmax(v: &[f32]) -> usize {
    (0..v.len())
        .max_by(|&a, &b| v[a].total_cmp(&v[b]))
        .unwrap_or(0)
}

/// The reference directory, written with the upstream implementation (OpenCLIP or
/// transformers): `texts.txt` (one text per line, then a tab and its 77 token ids),
/// `text.f32` (their embeddings), `classes.txt` (how many of the texts, from the first,
/// are zero-shot classes), `images.txt` (per line: file, width, height, the index of the
/// class that describes it), `<file>` (RGBA8), `pre.f32` (the upstream preprocessing of
/// each image), `image.f32` (the upstream embedding of each), and `norm.txt` (`openai` or
/// `inception`). Embeddings are unit length; all floats little-endian f32.
#[test]
#[ignore]
fn reference() {
    let (Some(weights), Some(merges), Some(dir)) = (
        env("SPECULAR_CLIP_WEIGHTS"),
        env("SPECULAR_CLIP_MERGES"),
        env("SPECULAR_CLIP_REF"),
    ) else {
        eprintln!("SPECULAR_CLIP_WEIGHTS, _MERGES and _REF not set: skipped");
        return;
    };
    let f16 = std::env::var_os("SPECULAR_CLIP_F32").is_none();
    let norm = std::fs::read_to_string(dir.join("norm.txt")).unwrap();
    let (mean, std) = if norm.trim() == "inception" {
        ([0.5; 3], [0.5; 3])
    } else {
        (MEAN, STD)
    };
    let t0 = Instant::now();
    let m = Clip::load(&weights, &merges, ImageInput::Rgba { mean, std }, f16).unwrap();
    let c = m.config.clone();
    eprintln!(
        "{c:?}\nloaded in {:.1} s; image {:.2} GMACs, text {:.2} GMACs",
        t0.elapsed().as_secs_f64(),
        c.image_macs() as f64 / 1e9,
        c.text_macs() as f64 / 1e9
    );
    let mut file = specular_infer::safetensors::SafeTensorsFile::open(&weights).unwrap();
    let net = Clip::build(
        c.clone(),
        &mut file,
        Bpe::from_merges(&std::fs::read_to_string(&merges).unwrap(), CLIP_MERGES).unwrap(),
        ImageInput::Tensor { batch: 1 },
        f16,
    )
    .unwrap();
    drop(file);
    let e = c.embed;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut gpu = ctx().map(|c| {
        (
            GpuGraph::new(c, &m.image).unwrap(),
            GpuGraph::new(c, &net.image).unwrap(),
            GpuGraph::new(c, &m.text.graph).unwrap(),
        )
    });

    // Tokens and text embeddings.
    let texts = std::fs::read_to_string(dir.join("texts.txt")).unwrap();
    let want_text = f32s(&dir.join("text.f32"));
    let mut ours_text = Vec::new();
    let mut worst_text = 1f32;
    let mut worst_text_gpu = 1f32;
    for (i, line) in texts.lines().enumerate() {
        let (text, ids) = line.split_once('\t').unwrap();
        let want: Vec<u32> = ids.split(' ').map(|s| s.parse().unwrap()).collect();
        let (got, end) = m.text.tokenizer.tokenize(text, c.context);
        assert_eq!(got, want, "tokens of {text:?}");
        let (x, onehot) = m.text.feeds_for_ids(&got, end);
        let feeds = [Feed::Tensor(&x), Feed::Tensor(&onehot)];
        let cpu_e = cpu::run(&m.text.graph, &feeds, threads).unwrap()[0].to_f32();
        let w = &want_text[i * e..(i + 1) * e];
        worst_text = worst_text.min(dot(&cpu_e, w));
        if let Some((_, _, tg)) = &mut gpu {
            let g = tg.run(&feeds).unwrap()[0].to_f32();
            worst_text_gpu = worst_text_gpu.min(dot(&g, &cpu_e));
        }
        ours_text.push(cpu_e);
    }
    eprintln!(
        "{} texts: tokens identical; cosine to upstream ≥ {worst_text:.5}, GPU to CPU ≥ {worst_text_gpu:.6}",
        ours_text.len()
    );

    // Images.
    let images = std::fs::read_to_string(dir.join("images.txt")).unwrap();
    let classes: usize = std::fs::read_to_string(dir.join("classes.txt"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let pre = f32s(&dir.join("pre.f32"));
    let want_img = f32s(&dir.join("image.f32"));
    let px = 3 * c.image * c.image;
    let (mut worst_net, mut worst_full, mut worst_gpu) = (1f32, 1f32, 1f32);
    let (mut ours_right, mut ref_right, mut n) = (0, 0, 0);
    for (i, line) in images.lines().enumerate() {
        let f: Vec<&str> = line.split(' ').collect();
        let (w, h, label): (u32, u32, usize) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        let rgba = std::fs::read(dir.join(f[0])).unwrap();
        let want = &want_img[i * e..(i + 1) * e];
        // The network alone, on the upstream preprocessing.
        let x = Tensor::f32(
            &[1, 3, c.image, c.image],
            pre[i * px..(i + 1) * px].to_vec(),
        );
        let got = cpu::run(&net.image, &[Feed::Tensor(&x)], threads).unwrap()[0].to_f32();
        worst_net = worst_net.min(dot(&got, want));
        // The whole pipeline from pixels.
        let feeds = [Feed::Rgba {
            width: w,
            height: h,
            data: &rgba,
        }];
        let full = cpu::run(&m.image, &feeds, threads).unwrap()[0].to_f32();
        worst_full = worst_full.min(dot(&full, want));
        if let Some((ig, ng, _)) = &mut gpu {
            let g = ig.run(&feeds).unwrap()[0].to_f32();
            worst_gpu = worst_gpu.min(dot(&g, &full));
            let g = ng.run(&[Feed::Tensor(&x)]).unwrap()[0].to_f32();
            worst_gpu = worst_gpu.min(dot(&g, &got));
        }
        // Zero-shot: the text that matches best.
        let scores: Vec<f32> = ours_text[..classes].iter().map(|t| dot(t, &full)).collect();
        let ref_scores: Vec<f32> = (0..classes)
            .map(|j| dot(&want_text[j * e..(j + 1) * e], want))
            .collect();
        ours_right += usize::from(argmax(&scores) == label);
        ref_right += usize::from(argmax(&ref_scores) == label);
        n += 1;
        eprintln!(
            "{}: label {label}, ours {} ({:.3}), upstream {} ({:.3})",
            f[0],
            argmax(&scores),
            scores[argmax(&scores)],
            argmax(&ref_scores),
            ref_scores[argmax(&ref_scores)]
        );
    }
    eprintln!(
        "{n} images: network cosine to upstream ≥ {worst_net:.5}; from pixels ≥ {worst_full:.4}; \
         GPU to CPU ≥ {worst_gpu:.6}; zero-shot right: ours {ours_right}, upstream {ref_right}"
    );
    assert!(worst_text >= 0.999, "text cosine {worst_text}");
    assert!(worst_net >= 0.999, "image cosine {worst_net}");
    assert!(worst_full >= 0.95, "pipeline cosine {worst_full}");
    assert!(worst_gpu >= 0.9999 && worst_text_gpu >= 0.9999);
    assert!(
        ours_right + 1 >= ref_right,
        "zero-shot: {ours_right} vs {ref_right}"
    );

    // Speed: an image from pixels and a text, warm, on each engine.
    let (w, h) = (640u32, 480u32);
    let rgba: Vec<u8> = (0..w * h * 4).map(|i| (i * 7 % 251) as u8).collect();
    let feeds = [Feed::Rgba {
        width: w,
        height: h,
        data: &rgba,
    }];
    let (x, onehot) = m.text.feeds("a photo of a dog");
    let tfeeds = [Feed::Tensor(&x), Feed::Tensor(&onehot)];
    let time = |f: &mut dyn FnMut(), n: usize| {
        f();
        let t = Instant::now();
        for _ in 0..n {
            f();
        }
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    };
    let cpu_img = time(
        &mut || drop(cpu::run(&m.image, &feeds, threads).unwrap()),
        3,
    );
    let cpu_txt = time(
        &mut || drop(cpu::run(&m.text.graph, &tfeeds, threads).unwrap()),
        3,
    );
    eprintln!("cpu ({threads} threads): image {cpu_img:.1} ms, text {cpu_txt:.1} ms");
    if let Some((ig, _, tg)) = &mut gpu {
        let gi = time(&mut || drop(ig.run(&feeds).unwrap()), 20);
        let gt = time(&mut || drop(tg.run(&tfeeds).unwrap()), 20);
        eprintln!("gpu: image {gi:.1} ms, text {gt:.1} ms");
    }
}
