//! PE Core: a tiny random-weight model on the GPU against the CPU reference (always), and
//! the real `facebook/PE-Core-S16-384` (read from Meta's `.pt`) against open_clip and timm
//! on timm's copy of the weights (when the files are there):
//!
//! ```text
//! SPECULAR_PECORE_DIR=target/pecore tools/corpus/fetch-pecore.sh   # .pt, merges, oracle, Kodak
//! python -I tools/corpus/pecore-oracle.py target/pecore \
//!     crates/specular-infer/tests/data/siglip-checkset.tsv       # the reference outputs
//! SPECULAR_PECORE_DIR=target/pecore cargo test -p specular-infer --test pecore -- --nocapture
//! ```
//!
//! `SPECULAR_PECORE_DIR` defaults to `target/pecore` under the workspace. Without its files
//! the real-model tests say what is missing and pass (no network in tests).

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use common::{check, ctx};
use specular_infer::bpe::{Bpe, CLIP_MERGES};
use specular_infer::clip::{Clip, ImageInput, Layout};
use specular_infer::gpu::GpuGraph;
use specular_infer::models::RandomWeights;
use specular_infer::pecore;
use specular_infer::tensor::Rng;
use specular_infer::{Feed, Tensor, cpu};

const HALF: [f32; 3] = [0.5; 3];

/// A few merges over the byte alphabet (the vocabulary is 256 · 2 + merges + 2 ids).
const TINY_MERGES: &str = "#version: 0.2\nc a\nca t</w>\no n</w>\nm a\nma t</w>\n";

fn tiny(input: ImageInput, f16: bool) -> Clip {
    let bpe = Bpe::from_merges(TINY_MERGES, CLIP_MERGES).unwrap();
    let c = pecore::test_config(bpe.vocab_size());
    Clip::build(c, &mut RandomWeights::new(5), bpe, input, f16).unwrap()
}

#[test]
fn tiny_pecore_gpu_matches_cpu() {
    for f16 in [false, true] {
        let name = format!("pecore f16 {f16}");
        let m = tiny(
            ImageInput::Rgba {
                mean: HALF,
                std: HALF,
            },
            f16,
        );
        assert_eq!(m.config.layout, Layout::PeCore);
        let (w, h) = (61u32, 37u32);
        let mut rng = Rng::new(8);
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

#[test]
fn tiny_pecore_batch_is_per_image() {
    let m = tiny(ImageInput::Tensor { batch: 3 }, false);
    let x = Rng::new(2).tensor(&[3, 3, 32, 32], 1.0);
    let (out, _) = check("batch", &m.image, &[Feed::Tensor(&x)], 1e-3, 1e-3);
    assert_eq!(out[0].shape, [3, 32]);
    let one = tiny(ImageInput::Tensor { batch: 1 }, false);
    let x1 = Tensor::f32(&[1, 3, 32, 32], x.to_f32()[3 * 1024..6 * 1024].to_vec());
    let alone = cpu::run(&one.image, &[Feed::Tensor(&x1)], 1).unwrap();
    let d =
        specular_infer::tensor::compare(&alone[0].to_f32(), &out[0].to_f32()[32..64], 1e-5, 1e-5);
    assert!(d.worst <= 1.0);

    // CLIP's text tower: causal, pooled at the end-of-text token; padding after it changes
    // nothing, a word before it does.
    let t = &one.text;
    let (ids, end) = t.tokenizer.tokenize("cat", t.context);
    assert_eq!(ids[end], ids.iter().copied().max().unwrap());
    let run = |ids: &[u32]| {
        let (x, e) = t.feeds_for_ids(ids, end);
        cpu::run(&t.graph, &[Feed::Tensor(&x), Feed::Tensor(&e)], 1).unwrap()[0].to_f32()
    };
    let mut pad = ids.clone();
    *pad.last_mut().unwrap() = 7;
    assert_eq!(run(&ids), run(&pad));
    let mut first = ids.clone();
    first[1] = 7;
    assert_ne!(run(&ids), run(&first));
}

// ---------------------------------------------------------------------------------------------
// The real model against open_clip.

fn dir() -> PathBuf {
    std::env::var_os("SPECULAR_PECORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/pecore")
                .to_path_buf()
        })
}

fn need(paths: &[PathBuf]) -> bool {
    let missing: Vec<String> = paths
        .iter()
        .filter(|p| !p.exists())
        .map(|p| p.display().to_string())
        .collect();
    if missing.is_empty() {
        return true;
    }
    eprintln!(
        "skipped: {} missing (run tools/corpus/fetch-pecore.sh and tools/corpus/pecore-oracle.py)",
        missing.join(", ")
    );
    false
}

fn unhex(s: &str) -> String {
    let b: Vec<u8> = (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    String::from_utf8(b).unwrap()
}

/// `key TAB numbers` lines.
fn table(path: &Path) -> Vec<(String, Vec<f32>)> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| {
            let (k, v) = l.split_once('\t').unwrap();
            (
                k.to_owned(),
                v.split(' ').map(|x| x.parse::<f32>().unwrap()).collect(),
            )
        })
        .collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| f64::from(x) * f64::from(y))
        .sum();
    let na: f64 = a.iter().map(|&x| f64::from(x).powi(2)).sum();
    let nb: f64 = b.iter().map(|&x| f64::from(x).powi(2)).sum();
    (dot / (na.sqrt() * nb.sqrt())) as f32
}

#[test]
fn tokenizer_matches_open_clip() {
    let d = dir();
    let (merges, tokens) = (d.join("merges.txt"), d.join("ref/tokens.tsv"));
    if !need(&[merges.clone(), tokens.clone()]) {
        return;
    }
    let bpe = Bpe::from_merges(&std::fs::read_to_string(&merges).unwrap(), CLIP_MERGES).unwrap();
    let (mut n, mut bad) = (0, Vec::new());
    for (k, want) in table(&tokens) {
        let text = unhex(&k);
        let want: Vec<u32> = want.iter().map(|&x| x as u32).collect();
        let (got, _) = bpe.tokenize(&text, 32);
        n += 1;
        if got != want {
            bad.push((text, got, want));
        }
    }
    eprintln!(
        "tokenizer: {} of {n} texts differ from open_clip",
        bad.len()
    );
    for (text, got, want) in bad.iter().take(5) {
        eprintln!("  {text:?}\n    ours {got:?}\n    ref  {want:?}");
    }
    assert!(bad.is_empty());
}

/// The check set: (language, photos that answer, query).
fn checkset() -> Vec<(String, Vec<usize>, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/siglip-checkset.tsv");
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let mut f = l.split('\t');
            let lang = f.next().unwrap().to_owned();
            let want = f
                .next()
                .unwrap()
                .split(',')
                .map(|n| n.parse().unwrap())
                .collect();
            (lang, want, f.next().unwrap().to_owned())
        })
        .collect()
}

fn rgba_files(dir: &Path) -> Vec<(String, u32, u32, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rgba"))
        .map(|p| {
            let stem = p.file_stem().unwrap().to_str().unwrap().to_owned();
            let (name, dims) = stem.split_once('-').unwrap();
            let (w, h) = dims.split_once('x').unwrap();
            (
                name.to_owned(),
                w.parse().unwrap(),
                h.parse().unwrap(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn f32s(path: &Path) -> Vec<f32> {
    std::fs::read(path)
        .unwrap()
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The real model from Meta's `.pt`: the towers on the reference's exact input (f32 and f16
/// weights) and on the photos' RGBA (the app's path, f16, CPU and GPU), the text tower on the
/// check set, the f16 conversion, and the check set's recall.
#[test]
fn s16_matches_open_clip() {
    let d = dir();
    let r = d.join("ref");
    let weights = d.join("PE-Core-S16-384.pt");
    let tok = d.join("merges.txt");
    if !need(&[
        weights.clone(),
        tok.clone(),
        r.join("images.tsv"),
        r.join("text.tsv"),
    ]) {
        return;
    }
    let images = table(&r.join("images.tsv"));
    let texts = table(&r.join("text.tsv"));

    // Exact input, f32 and f16 weights, CPU: the towers themselves, all 24 photos and the
    // English queries (and a few others: the tower does not know the language).
    for (f16, min) in [(false, 0.9999), (true, 0.9999)] {
        let t0 = Instant::now();
        let m = Clip::load(&weights, &tok, ImageInput::Tensor { batch: 1 }, f16).unwrap();
        eprintln!("f16 {f16}: loaded from the .pt in {:?}", t0.elapsed());
        assert_eq!(m.config.layout, Layout::PeCore);
        assert_eq!((m.config.image, m.config.embed), (384, 512));
        let mut worst = 1f32;
        let t0 = Instant::now();
        for (name, want) in &images {
            let px = f32s(&r.join(format!("pixels/{name}.f32")));
            let x = Tensor::f32(&[1, 3, 384, 384], px);
            let out = cpu::run(&m.image, &[Feed::Tensor(&x)], 16).unwrap();
            worst = worst.min(cosine(&out[0].to_f32(), want));
        }
        eprintln!(
            "  24 photos: worst cosine {worst:.6} (CPU {:?})",
            t0.elapsed()
        );
        assert!(worst >= min, "f16 {f16}: image cosine {worst}");
        let mut worst = 1f32;
        for (k, want) in texts.iter().take(40) {
            let (x, e) = m.text.feeds(&unhex(k));
            let out = cpu::run(&m.text.graph, &[Feed::Tensor(&x), Feed::Tensor(&e)], 16).unwrap();
            worst = worst.min(cosine(&out[0].to_f32(), want));
        }
        eprintln!("  text: worst cosine {worst:.6} over 40 queries");
        assert!(worst >= min, "f16 {f16}: text cosine {worst}");
    }

    // The f16 file the pack keeps: built once next to the download, same graph.
    let conv = d.join("model-f16.safetensors");
    if !conv.exists() {
        let t0 = Instant::now();
        let n = pecore::convert(&weights, &conv, &|| false).unwrap();
        eprintln!("converted to f16: {n} bytes in {:?}", t0.elapsed());
    }
    let input = ImageInput::Rgba {
        mean: HALF,
        std: HALF,
    };
    let a = Clip::load(&weights, &tok, input, true).unwrap();
    let t0 = Instant::now();
    let m = Clip::load(&conv, &tok, input, true).unwrap();
    eprintln!("f16 file: loaded in {:?}", t0.elapsed());
    let rgba = rgba_files(&r.join("rgba"));
    let (_, w, h, px) = &rgba[0];
    let feed = [Feed::Rgba {
        width: *w,
        height: *h,
        data: px,
    }];
    assert_eq!(
        cpu::run(&a.image, &feed, 16).unwrap(),
        cpu::run(&m.image, &feed, 16).unwrap(),
        "the converted file builds a different image tower"
    );
    let (x, e) = a.text.feeds("a red car");
    let tf = [Feed::Tensor(&x), Feed::Tensor(&e)];
    assert_eq!(
        cpu::run(&a.text.graph, &tf, 16).unwrap(),
        cpu::run(&m.text.graph, &tf, 16).unwrap(),
        "the converted file builds a different text tower"
    );
    drop(a);

    // The app's path: RGBA through the graph's resampling, GPU (matched against the CPU on
    // one photo), text on the GPU; then the check set's recall.
    let gpu = ctx();
    let (_, d0) = check("pecore rgba", &m.image, &feed, 2e-2, 2e-2);
    eprintln!("GPU vs CPU, one photo: cosine {:.6}", d0.cosine);
    let mut gi = gpu.map(|c| GpuGraph::new(c, &m.image).unwrap());
    let mut gt = gpu.map(|c| GpuGraph::new(c, &m.text.graph).unwrap());
    if let Some(g) = gi.as_mut() {
        g.run(&feed).unwrap();
    }
    let mut img_e = Vec::new();
    let mut worst = 1f32;
    let t0 = Instant::now();
    for ((name, w, h, px), (rname, want)) in rgba.iter().zip(&images) {
        assert_eq!(name, rname);
        let feed = [Feed::Rgba {
            width: *w,
            height: *h,
            data: px,
        }];
        let out = match gi.as_mut() {
            Some(g) => g.run(&feed).unwrap(),
            None => cpu::run(&m.image, &feed, 16).unwrap(),
        };
        let e = out[0].to_f32();
        worst = worst.min(cosine(&e, want));
        img_e.push(e);
    }
    eprintln!(
        "24 photos from RGBA ({}): {:?}, worst cosine to open_clip {worst:.5}",
        if gpu.is_some() { "GPU" } else { "CPU" },
        t0.elapsed()
    );
    // PIL rounds the resized image to 8 bits; the graph resamples in float.
    assert!(worst >= 0.99, "RGBA path cosine {worst}");

    let set = checkset();
    let mut per: Vec<(String, usize, usize, usize)> = Vec::new();
    let mut worst_text = 1f32;
    let mut best_scores = Vec::new();
    let t0 = Instant::now();
    for ((lang, want, q), (k, reference)) in set.iter().zip(&texts) {
        assert_eq!(&unhex(k), q);
        let (x, e) = m.text.feeds(q);
        let feeds = [Feed::Tensor(&x), Feed::Tensor(&e)];
        let out = match gt.as_mut() {
            Some(g) => g.run(&feeds).unwrap(),
            None => cpu::run(&m.text.graph, &feeds, 16).unwrap(),
        };
        let te = out[0].to_f32();
        worst_text = worst_text.min(cosine(&te, reference));
        let mut rank: Vec<(f32, usize)> = img_e
            .iter()
            .enumerate()
            .map(|(i, ie)| (cosine(ie, &te), i + 1))
            .collect();
        rank.sort_by(|a, b| b.0.total_cmp(&a.0));
        let at1 = usize::from(want.contains(&rank[0].1));
        let at3 = usize::from(rank[..3].iter().any(|(_, i)| want.contains(i)));
        if lang == "en" {
            // How far below the best the right photo scores, and the best score.
            let right = rank.iter().find(|(_, i)| want.contains(i)).unwrap().0;
            best_scores.push((rank[0].0, rank[0].0 - right, rank[3].0));
        }
        match per.iter_mut().find(|p| &p.0 == lang) {
            Some(p) => {
                p.1 += at1;
                p.2 += at3;
                p.3 += 1;
            }
            None => per.push((lang.clone(), at1, at3, 1)),
        }
    }
    eprintln!(
        "{} queries: {:?}, worst text cosine to open_clip {worst_text:.5}",
        set.len(),
        t0.elapsed()
    );
    let (lo, hi) = best_scores
        .iter()
        .fold((1f32, -1f32), |(a, b), s| (a.min(s.0), b.max(s.0)));
    let gap = best_scores.iter().map(|s| s.1).fold(0f32, f32::max);
    let fourth = best_scores.iter().map(|s| s.0 - s.2).fold(1f32, f32::min);
    eprintln!(
        "en: best score {lo:.3}..{hi:.3}, right photo at most {gap:.3} below it, 4th at least {fourth:.3} below"
    );
    let (mut a1, mut a3, mut n) = (0, 0, 0);
    for (lang, h1, h3, c) in &per {
        eprintln!("  {lang}: recall@1 {h1}/{c}, recall@3 {h3}/{c}");
        a1 += h1;
        a3 += h3;
        n += c;
    }
    eprintln!("all: recall@1 {a1}/{n}, recall@3 {a3}/{n}");
    assert!(worst_text >= 0.9999, "text cosine {worst_text}");
    // open_clip itself: English 19/19 at 1; 63/152 at 1 and 73/152 at 3 over all eight
    // languages (PE Core is English only).
    let en = per.iter().find(|p| p.0 == "en").unwrap();
    assert!(en.1 >= 18 && en.2 == 19, "English recall {}/{}", en.1, en.2);
}
