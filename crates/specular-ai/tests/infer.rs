//! The `Runtime` over `specular-infer`: synthetic packs (random fixed-seed weights written as
//! safetensors with the architecture in the metadata) load, run on the CPU and, when the
//! GPU matches the CPU, on the GPU, which the selector then trusts. Bad packs are refused.
//!
//! Skips the GPU halves without an adapter unless SPECULAR_REQUIRE_GPU is set.

mod common;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use half::f16;
use specular_ai::Feature;
use specular_ai::infer::{Arch, GpuCheck, InferRuntime, Spec};
use specular_ai::manifest::Norm;
use specular_ai::runtime::{Dispatch, Outputs, RunError, Runtime, Thumb, agree};
use specular_core::paths::{Engine, PathSelector, Task, Trust};
use specular_infer::bpe::{Bpe, CLIP_MERGES};
use specular_infer::clip::{ClipConfig, Layout};
use specular_infer::gpu::Context;
use specular_infer::graph::Crop;
use specular_infer::models::{CnnConfig, RandomWeights, VitConfig, Weights};
use specular_infer::safetensors::serialize;
use specular_infer::{Error, Tensor};

/// The `(key, value)` metadata pairs of a pack.
type Metadata = Vec<(String, String)>;

fn ctx() -> Option<&'static Context> {
    static C: OnceLock<Option<(specular_gfx::Gpu, Context)>> = OnceLock::new();
    C.get_or_init(|| match specular_gfx::Gpu::new() {
        Ok(g) => {
            let c = Context::new(&g.device, &g.queue);
            Some((g, c))
        }
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU halves skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
    .map(|(_, c)| c)
}

/// Random weights that remember what the builder asked for, to write them as a pack.
struct Recorder {
    inner: RandomWeights,
    seen: Vec<(String, Tensor)>,
}

impl Weights for Recorder {
    fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        let t = self.inner.get(name, shape)?;
        self.seen.push((name.to_owned(), t.clone()));
        Ok(t)
    }
}

fn vit_spec() -> Spec {
    Spec {
        arch: Arch::Vit(VitConfig::test()),
        crop: Crop::Center,
        mean: [0.481, 0.458, 0.408],
        std: [0.269, 0.261, 0.276],
    }
}

fn cnn_spec() -> Spec {
    Spec {
        arch: Arch::Cnn(CnnConfig::test()),
        crop: Crop::Stretch,
        mean: [0.5; 3],
        std: [0.5; 3],
    }
}

/// A pack's bytes for `spec` with weights from `seed` (`f16`: matrices stored as f16).
fn pack(spec: &Spec, seed: u64, f16: bool) -> Vec<u8> {
    let mut rec = Recorder {
        inner: RandomWeights { seed, f16 },
        seen: Vec::new(),
    };
    // Building once lists the weights; the CPU-only build needs no selector state.
    static SEL: PathSelector = PathSelector::new();
    InferRuntime::build(
        Feature::Search,
        "probe",
        spec.clone(),
        &mut rec,
        None,
        &SEL,
        1,
    )
    .expect("builds");
    let tensors: Vec<(&str, &Tensor)> = rec.seen.iter().map(|(n, t)| (n.as_str(), t)).collect();
    let meta = spec.to_metadata();
    let meta: Vec<(&str, &str)> = meta.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    serialize(&tensors, &meta)
}

fn picture(w: u32, h: u32, seed: u32) -> Vec<u8> {
    (0..w * h * 4)
        .map(|i| (i.wrapping_mul(2_654_435_761).rotate_left(seed) >> 24) as u8)
        .collect()
}

fn embedding(o: &Outputs) -> &[u16] {
    let Outputs::Embedding(v) = o else {
        panic!("not an embedding: {o:?}")
    };
    v
}

#[test]
fn vit_pack_loads_runs_and_trusts_a_matching_gpu() {
    static SEL: PathSelector = PathSelector::new();
    let dir = common::Dir::new("vit-pack");
    let spec = vit_spec();
    let path = dir.path().join("vit.safetensors");
    std::fs::write(&path, pack(&spec, 11, false)).unwrap();

    let rt =
        InferRuntime::load(&path, Feature::Search, "vit-test-1", ctx(), &SEL, 2).expect("loads");
    assert_eq!(rt.spec(), &spec);
    assert_eq!(rt.model(), "vit-test-1");
    assert_eq!(rt.input_side(), 32);
    assert_eq!(rt.engines()[0], Engine::Cpu);

    // The pack gives what the same random weights give directly.
    let direct = InferRuntime::build(
        Feature::Search,
        "direct",
        spec.clone(),
        &mut RandomWeights::new(11),
        None,
        &SEL,
        1,
    )
    .unwrap();
    let px = picture(57, 41, 3);
    let thumb = Thumb {
        width: 57,
        height: 41,
        rgba: &px,
    };
    let cpu = rt.run(Engine::Cpu, &thumb).unwrap();
    assert_eq!(cpu, direct.run(Engine::Cpu, &thumb).unwrap());
    assert_eq!(embedding(&cpu).len(), 32);
    // A different picture gives a different embedding.
    let other = picture(57, 41, 9);
    let o = Thumb {
        rgba: &other,
        ..thumb
    };
    assert_ne!(cpu, rt.run(Engine::Cpu, &o).unwrap());

    let gpu_path = Task::Inference.path_of(Engine::Gpu).unwrap();
    if ctx().is_none() {
        assert_eq!(rt.gpu_check(), &GpuCheck::NotTried);
        assert_eq!(rt.engines(), [Engine::Cpu]);
        assert_eq!(rt.run(Engine::Gpu, &thumb), Err(RunError::Unsupported));
        return;
    }
    assert_eq!(rt.gpu_check(), &GpuCheck::Matched);
    assert_eq!(rt.engines(), [Engine::Cpu, Engine::Gpu]);
    assert_eq!(SEL.trust(Task::Inference, gpu_path), Trust::Trusted);
    let gpu = rt.run(Engine::Gpu, &thumb).unwrap();
    assert!(agree(&gpu, &cpu), "GPU and CPU embeddings differ");
    // Repeatable.
    assert_eq!(gpu, rt.run(Engine::Gpu, &thumb).unwrap());

    // Through the dispatcher: whichever engine runs, the embedding agrees with the CPU's.
    let d = Dispatch::new(Arc::new(rt), &SEL);
    for _ in 0..8 {
        assert!(agree(&d.run(&thumb).unwrap(), &cpu));
    }
}

#[test]
fn cnn_pack_with_f16_weights_matches_on_both_engines() {
    static SEL: PathSelector = PathSelector::new();
    let dir = common::Dir::new("cnn-pack");
    let spec = cnn_spec();
    let path = dir.path().join("cnn.safetensors");
    std::fs::write(&path, pack(&spec, 5, true)).unwrap();
    let rt = InferRuntime::load(&path, Feature::Search, "cnn-test-1", ctx(), &SEL, 3).unwrap();
    assert_eq!(rt.input_side(), 64);
    let px = picture(80, 120, 7);
    let thumb = Thumb {
        width: 80,
        height: 120,
        rgba: &px,
    };
    let cpu = rt.run(Engine::Cpu, &thumb).unwrap();
    assert_eq!(embedding(&cpu).len(), 10);
    if ctx().is_some() {
        assert_eq!(rt.gpu_check(), &GpuCheck::Matched);
        assert!(agree(&rt.run(Engine::Gpu, &thumb).unwrap(), &cpu));
    }
}

#[test]
fn a_blocked_gpu_is_not_tried() {
    static SEL: PathSelector = PathSelector::new();
    SEL.block(Task::Inference, Engine::Gpu, "test");
    let rt = InferRuntime::build(
        Feature::Search,
        "m",
        vit_spec(),
        &mut RandomWeights::new(1),
        ctx(),
        &SEL,
        1,
    )
    .unwrap();
    assert_eq!(rt.gpu_check(), &GpuCheck::NotTried);
    assert_eq!(rt.engines(), [Engine::Cpu]);
}

#[test]
fn bad_thumbnails_and_engines_are_refused() {
    static SEL: PathSelector = PathSelector::new();
    let rt = InferRuntime::build(
        Feature::Search,
        "m",
        vit_spec(),
        &mut RandomWeights::new(1),
        None,
        &SEL,
        1,
    )
    .unwrap();
    let px = [0u8; 15];
    let short = Thumb {
        width: 2,
        height: 2,
        rgba: &px,
    };
    assert!(matches!(
        rt.run(Engine::Cpu, &short),
        Err(RunError::Failed(_))
    ));
    let empty = Thumb {
        width: 0,
        height: 0,
        rgba: &[],
    };
    assert!(matches!(
        rt.run(Engine::Cpu, &empty),
        Err(RunError::Failed(_))
    ));
    let ok = [9u8; 16];
    let t = Thumb {
        width: 2,
        height: 2,
        rgba: &ok,
    };
    assert_eq!(rt.run(Engine::Npu, &t), Err(RunError::Unsupported));
    assert!(rt.run(Engine::Cpu, &t).is_ok());
}

#[test]
fn bad_packs_are_refused() {
    static SEL: PathSelector = PathSelector::new();
    let dir = common::Dir::new("bad-packs");
    let spec = vit_spec();
    let good = pack(&spec, 2, false);
    let load = |name: &str, bytes: &[u8]| {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        InferRuntime::load(&p, Feature::Search, "m", None, &SEL, 1).map(|_| ())
    };
    assert!(load("good", &good).is_ok());
    // Not the search feature: no head for it yet.
    let p = dir.path().join("good");
    assert!(InferRuntime::load(&p, Feature::People, "m", None, &SEL, 1).is_err());
    // Truncated.
    assert!(load("short", &good[..good.len() - 4]).is_err());
    assert!(load("empty", &[]).is_err());
    assert!(load("missing", b"").is_err());
    assert!(
        InferRuntime::load(
            &dir.path().join("absent"),
            Feature::Search,
            "m",
            None,
            &SEL,
            1
        )
        .is_err()
    );

    // Metadata changes.
    let with = |edit: &dyn Fn(&mut Metadata)| {
        let mut rec = Recorder {
            inner: RandomWeights::new(2),
            seen: Vec::new(),
        };
        InferRuntime::build(Feature::Search, "m", spec.clone(), &mut rec, None, &SEL, 1).unwrap();
        let mut meta = spec.to_metadata();
        edit(&mut meta);
        let tensors: Vec<(&str, &Tensor)> = rec.seen.iter().map(|(n, t)| (n.as_str(), t)).collect();
        let meta: Vec<(&str, &str)> = meta.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        serialize(&tensors, &meta)
    };
    let set = |key: &'static str, val: &'static str| {
        move |m: &mut Vec<(String, String)>| {
            for (k, v) in m.iter_mut() {
                if k == key {
                    *v = val.into();
                }
            }
        }
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("no-metadata", with(&|m| m.clear())),
        ("arch", with(&set("specular.arch", "rnn"))),
        ("crop", with(&set("specular.crop", "zoom"))),
        ("mean", with(&set("specular.mean", "0.5,0.5"))),
        ("std-nan", with(&set("specular.std", "0.5,NaN,0.5"))),
        ("huge", with(&set("specular.vit.dim", "99999999999"))),
        ("zero", with(&set("specular.vit.depth", "0"))),
        ("pool", with(&set("specular.vit.pool", "max"))),
        // Shapes that the weights do not have.
        ("wider", with(&set("specular.vit.dim", "128"))),
        ("deeper", with(&set("specular.vit.depth", "3"))),
        (
            "cnn-blocks",
            with(&|m| {
                *m = cnn_spec().to_metadata();
                set("specular.cnn.blocks", "1:16")(m);
            }),
        ),
    ];
    for (name, bytes) in &cases {
        assert!(load(name, bytes).is_err(), "{name} loaded");
    }
}

#[test]
fn metadata_round_trips() {
    for spec in [vit_spec(), cnn_spec()] {
        assert_eq!(Spec::from_metadata(&spec.to_metadata()).unwrap(), spec);
    }
    let mut no_proj = vit_spec();
    if let Arch::Vit(c) = &mut no_proj.arch {
        c.out_dim = None;
    }
    let meta = no_proj.to_metadata();
    assert!(!meta.iter().any(|(k, _)| k == "specular.vit.out_dim"));
    assert_eq!(Spec::from_metadata(&meta).unwrap(), no_proj);
}

/// A few merges over the byte alphabet: a 520-id vocabulary.
const MERGES: &str = "#version: 0.2\nh e\nl l\nhe ll\nhell o</w>\nc a\nca t</w>\n";

/// A random CLIP checkpoint in `layout` and its merges file, written to `dir`.
fn clip_files(dir: &common::Dir, layout: Layout, seed: u64) -> (PathBuf, PathBuf) {
    let c = ClipConfig::test(layout, 520);
    let mut rec = Recorder {
        inner: RandomWeights { seed, f16: false },
        seen: Vec::new(),
    };
    static SEL: PathSelector = PathSelector::new();
    let bpe = Bpe::from_merges(MERGES, CLIP_MERGES).unwrap();
    InferRuntime::build_clip("probe", c, Norm::Openai, &mut rec, bpe, None, &SEL, 1)
        .expect("builds");
    let tensors: Vec<(&str, &Tensor)> = rec.seen.iter().map(|(n, t)| (n.as_str(), t)).collect();
    let w = dir.path().join(format!("clip-{layout:?}.safetensors"));
    std::fs::write(&w, serialize(&tensors, &[])).unwrap();
    let m = dir.path().join("merges.txt");
    std::fs::write(&m, MERGES).unwrap();
    (w, m)
}

fn dot(a: &[u16], b: &[u16]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| f16::from_bits(x).to_f32() * f16::from_bits(y).to_f32())
        .sum()
}

#[test]
fn clip_checkpoints_load_as_image_runtime_and_text_encoder() {
    let dir = common::Dir::new("clip");
    for (i, layout) in [Layout::OpenClip, Layout::HuggingFace]
        .into_iter()
        .enumerate()
    {
        static SEL: PathSelector = PathSelector::new();
        let (w, m) = clip_files(&dir, layout, 21 + i as u64);
        let (rt, text) =
            InferRuntime::load_clip(&w, &m, "clip-test-1", Norm::Inception, ctx(), &SEL, 2)
                .expect("loads");
        assert!(matches!(rt.spec().arch, Arch::Clip(_)));
        assert_eq!(rt.spec().mean, [0.5; 3]);
        assert_eq!(rt.input_side(), 32);
        assert_eq!(text.model(), "clip-test-1");
        assert_eq!(text.tokens("hello cat")[..4], [518, 515, 517, 519]);

        let px = picture(48, 40, 3);
        let thumb = Thumb {
            width: 48,
            height: 40,
            rgba: &px,
        };
        let img = rt.run(Engine::Cpu, &thumb).unwrap();
        let img = embedding(&img).to_vec();
        assert_eq!(img.len(), 32);
        let a = text.encode_on(Engine::Cpu, "hello cat").unwrap();
        let b = text.encode_on(Engine::Cpu, "a cat").unwrap();
        assert_eq!(a.len(), 32);
        assert!((dot(&a, &a) - 1.0).abs() < 0.01);
        assert!(dot(&a, &b) < 0.999, "different texts, different embeddings");
        // Case and spacing do not matter.
        assert_eq!(text.encode_on(Engine::Cpu, "  HELLO\tcat ").unwrap(), a);

        if ctx().is_some() {
            assert_eq!(rt.gpu_check(), &GpuCheck::Matched, "{layout:?}");
            assert_eq!(text.gpu_check(), &GpuCheck::Matched, "{layout:?}");
            assert_eq!(text.engines(), [Engine::Cpu, Engine::Gpu]);
            let g = rt.run(Engine::Gpu, &thumb).unwrap();
            assert!(dot(embedding(&g), &img) > 0.999);
            assert!(dot(&text.encode_on(Engine::Gpu, "hello cat").unwrap(), &a) > 0.999);
            assert!(dot(&text.encode("a cat").unwrap(), &b) > 0.999);
        } else {
            assert_eq!(text.engines(), [Engine::Cpu]);
            assert!(matches!(
                text.encode_on(Engine::Gpu, "x"),
                Err(RunError::Unsupported)
            ));
        }
    }
}

#[test]
fn clip_refuses_a_mismatched_tokenizer() {
    static SEL: PathSelector = PathSelector::new();
    let dir = common::Dir::new("clip-bad");
    let (w, m) = clip_files(&dir, Layout::OpenClip, 5);
    // One merge fewer: the vocabulary no longer matches the token table.
    std::fs::write(&m, "h e\nl l\nhe ll\nhell o</w>\nc a\n").unwrap();
    assert!(InferRuntime::load_clip(&w, &m, "m", Norm::Openai, None, &SEL, 1).is_err());
    std::fs::remove_file(&m).unwrap();
    assert!(InferRuntime::load_clip(&w, &m, "m", Norm::Openai, None, &SEL, 1).is_err());
    // A CLIP pack's metadata does not stand in for the checkpoint.
    let c = ClipConfig::test(Layout::OpenClip, 520);
    let meta = Spec::clip(c, Norm::Openai).to_metadata();
    assert!(Spec::from_metadata(&meta).is_err());
}

/// Zero-shot through the app's API on the reference's synthetic pictures (see
/// specular-infer's tests/clip.rs `reference`): SPECULAR_CLIP_WEIGHTS, SPECULAR_CLIP_MERGES,
/// SPECULAR_CLIP_REF.
#[test]
#[ignore = "needs a real checkpoint and reference dumps"]
fn clip_real_zero_shot() {
    static SEL: PathSelector = PathSelector::new();
    let var = |k| std::env::var_os(k).map(PathBuf::from).expect(k);
    let (w, m, r) = (
        var("SPECULAR_CLIP_WEIGHTS"),
        var("SPECULAR_CLIP_MERGES"),
        var("SPECULAR_CLIP_REF"),
    );
    let read = |n: &str| std::fs::read_to_string(r.join(n)).unwrap();
    let norm = if read("norm.txt").trim() == "inception" {
        Norm::Inception
    } else {
        Norm::Openai
    };
    let (rt, text) = InferRuntime::load_clip(&w, &m, "real", norm, ctx(), &SEL, 8).unwrap();
    let n: usize = read("classes.txt").trim().parse().unwrap();
    let texts = read("texts.txt");
    let classes: Vec<Vec<u16>> = texts
        .lines()
        .take(n)
        .map(|l| text.encode(l.split('\t').next().unwrap()).unwrap())
        .collect();
    let mut right = 0;
    for l in read("images.txt").lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        let (wd, ht, want): (u32, u32, usize) = (
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        let px = std::fs::read(r.join(f[0])).unwrap();
        let thumb = Thumb {
            width: wd,
            height: ht,
            rgba: &px,
        };
        let e = rt.run(*rt.engines().last().unwrap(), &thumb).unwrap();
        let e = embedding(&e);
        let best = (0..n)
            .max_by(|&a, &b| dot(e, &classes[a]).total_cmp(&dot(e, &classes[b])))
            .unwrap();
        right += usize::from(best == want);
    }
    eprintln!("zero-shot {right}/{n} (engines {:?})", rt.engines());
    assert!(right + 1 >= n);
}

/// What the selector picks for the indexing job's thumbnails from a saved table
/// (SPECULAR_PATHS_TABLE, read only).
#[test]
#[ignore = "needs a saved table"]
fn saved_table_choice() {
    use specular_core::paths::{Objective, Request};
    let p = std::env::var_os("SPECULAR_PATHS_TABLE").expect("SPECULAR_PATHS_TABLE");
    let text = std::fs::read_to_string(p).unwrap();
    for power in [false, true] {
        let sel = PathSelector::new();
        assert!(sel.load_table(&text, None));
        sel.set_power_saving(power);
        let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
        for side in [256u64, 320, 512] {
            let req = Request::new(side * side * 3 / 4, 0).objective(Objective::Background);
            let key = sel.key_of(&req);
            let mpx = req.pixels as f32 / 1e6;
            let mut n = [0; 3];
            for _ in 0..1024 {
                n[sel.choose(Task::Inference, &req).path] += 1;
            }
            eprintln!(
                "power {power} {side}: trust {:?}, cpu {:?}, gpu {:?}, chosen cpu/gpu {:?}",
                sel.trust(Task::Inference, gpu),
                sel.estimate(Task::Inference, 0, key, mpx),
                sel.estimate(Task::Inference, gpu, key, mpx),
                &n[..2]
            );
        }
    }
}

/// Cosine and largest absolute difference of two raw outputs.
fn deviation(a: &[f32], b: &[f32]) -> (f64, f32) {
    let (mut ab, mut aa, mut bb, mut max) = (0f64, 0f64, 0f64, 0f32);
    for (&x, &y) in a.iter().zip(b) {
        ab += f64::from(x) * f64::from(y);
        aa += f64::from(x) * f64::from(x);
        bb += f64::from(y) * f64::from(y);
        max = max.max((x - y).abs());
    }
    (ab / (aa.sqrt() * bb.sqrt()), max)
}

/// The GPU check of an installed CLIP (SPECULAR_CLIP_WEIGHTS, SPECULAR_CLIP_MERGES, both
/// read in place): the raw GPU/CPU deviation of both towers, the checks' outcomes, and CPU
/// against GPU time per picture.
#[test]
#[ignore = "needs a real checkpoint"]
fn clip_real_gpu_check() {
    use specular_infer::clip::{Clip, ImageInput};
    use specular_infer::gpu::GpuGraph;
    use specular_infer::safetensors::SafeTensorsFile;
    use specular_infer::{Feed, cpu};
    static SEL: PathSelector = PathSelector::new();
    let var = |k| std::env::var_os(k).map(PathBuf::from).expect(k);
    let (w, m) = (var("SPECULAR_CLIP_WEIGHTS"), var("SPECULAR_CLIP_MERGES"));
    let c = ctx().expect("a GPU");
    eprintln!("features: {:?}", c.features());
    let ms = |t: std::time::Instant| t.elapsed().as_secs_f64() * 1e3;
    let px = picture(640, 480, 7);
    // Raw outputs, compiled directly.
    {
        let bpe = Bpe::from_merges(&std::fs::read_to_string(&m).unwrap(), CLIP_MERGES).unwrap();
        let mut file = SafeTensorsFile::open(&w).unwrap();
        let cfg = ClipConfig::detect(&file.header).unwrap();
        eprintln!("config: {cfg:?}");
        let (mean, std) = Norm::Openai.mean_std();
        let clip = Clip::build(cfg, &mut file, bpe, ImageInput::Rgba { mean, std }, true).unwrap();
        let feed = Feed::Rgba {
            width: 640,
            height: 480,
            data: &px,
        };
        let t = std::time::Instant::now();
        let mut gi = GpuGraph::new(c, &clip.image).unwrap();
        eprintln!("image graph on the GPU: {:.0} ms", ms(t));
        let g = gi.run(std::slice::from_ref(&feed)).unwrap()[0].to_f32();
        let cpu_out = cpu::run(&clip.image, std::slice::from_ref(&feed), 8).unwrap()[0].to_f32();
        let (cos, max) = deviation(&g, &cpu_out);
        eprintln!(
            "image raw: cosine {cos:.6}, max |diff| {max:.4}, norm {:.3}",
            { cpu_out.iter().map(|x| x * x).sum::<f32>().sqrt() }
        );
        let (x, end) = clip
            .text
            .feeds("a photo of a dog on a beach, 2 people & a red kite!");
        let feeds = [Feed::Tensor(&x), Feed::Tensor(&end)];
        let mut gt = GpuGraph::new(c, &clip.text.graph).unwrap();
        let g = gt.run(&feeds).unwrap()[0].to_f32();
        let cpu_out = cpu::run(&clip.text.graph, &feeds, 8).unwrap()[0].to_f32();
        let (cos, max) = deviation(&g, &cpu_out);
        eprintln!("text raw: cosine {cos:.6}, max |diff| {max:.4}");
    }
    let t = std::time::Instant::now();
    let (rt, text) =
        InferRuntime::load_clip(&w, &m, "real", Norm::Openai, Some(c), &SEL, 8).unwrap();
    eprintln!("load + checks: {:.0} ms", ms(t));
    eprintln!("image check: {:?}", rt.gpu_check());
    eprintln!("text check: {:?}", text.gpu_check());
    let gpu = Task::Inference.path_of(Engine::Gpu).unwrap();
    eprintln!("selector trust: {:?}", SEL.trust(Task::Inference, gpu));
    let thumb = Thumb {
        width: 640,
        height: 480,
        rgba: &px,
    };
    for &e in rt.engines() {
        let _ = rt.run(e, &thumb).unwrap();
        let t = std::time::Instant::now();
        for _ in 0..5 {
            rt.run(e, &thumb).unwrap();
        }
        eprintln!("image on {e:?}: {:.1} ms", ms(t) / 5.0);
    }
    for &e in text.engines() {
        let _ = text.encode_on(e, "a dog").unwrap();
        let t = std::time::Instant::now();
        for _ in 0..5 {
            text.encode_on(e, "a red kite over a beach").unwrap();
        }
        eprintln!("text on {e:?}: {:.1} ms", ms(t) / 5.0);
    }
}
