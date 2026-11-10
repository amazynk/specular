//! Tiny end-to-end networks, GPU against the CPU reference, with fixed-seed random weights:
//! a 2-block ViT (32², patch 8, dim 64, 4 heads) and a MobileNetV2-style CNN (64², five
//! inverted-residual blocks), in f32 and f16 weights; also loaded back through safetensors.
//!
//! Tolerance: |gpu − cpu| ≤ 1e-3 + 1e-3·|cpu| per element, and cosine ≥ 0.99999.

mod common;

use common::check;
use specular_infer::graph::{Crop, Preprocess};
use specular_infer::models::{
    CnnConfig, RandomWeights, VitConfig, VitPool, Weights, cnn, image_batch, vit,
};
use specular_infer::safetensors::{SafeTensors, serialize};
use specular_infer::tensor::Rng;
use specular_infer::{Error, Feed, Graph, Tensor};

const ATOL: f32 = 1e-3;
const RTOL: f32 = 1e-3;

/// Records every tensor a model asks for, so it can be written to a safetensors file.
struct Recording {
    inner: RandomWeights,
    seen: Vec<(String, Tensor)>,
}

impl Weights for Recording {
    fn get(&mut self, name: &str, shape: &[usize]) -> Result<Tensor, Error> {
        let t = self.inner.get(name, shape)?;
        self.seen.push((name.to_owned(), t.clone()));
        Ok(t)
    }
}

fn run_vit(cfg: &VitConfig, n: usize, f16: bool) {
    let mut g = Graph::new();
    let x = image_batch(&mut g, n, cfg.image).unwrap();
    let mut w = RandomWeights { seed: 11, f16 };
    let y = vit(&mut g, x, cfg, &mut w).unwrap();
    g.output(y).unwrap();
    let input = Rng::new(5).tensor(&[n, 3, cfg.image, cfg.image], 1.5);
    let name = format!("vit {cfg:?} n{n} f16 {f16}");
    let (out, d) = check(&name, &g, &[Feed::Tensor(&input)], ATOL, RTOL);
    assert!(d.cosine >= 0.99999, "{name}: cosine {}", d.cosine);
    let dim = cfg.out_dim.unwrap_or(cfg.dim);
    assert_eq!(out[0].shape, [n, dim]);
    assert!(out[0].to_f32().iter().all(|v| v.is_finite()));
    eprintln!("{name}: max |error| {:.2e}, cosine {}", d.max_abs, d.cosine);
}

#[test]
fn vit_f32() {
    run_vit(&VitConfig::test(), 2, false);
}

#[test]
fn vit_f16() {
    run_vit(&VitConfig::test(), 1, true);
}

#[test]
fn vit_mean_pool_no_head() {
    let cfg = VitConfig {
        pool: VitPool::Mean,
        out_dim: None,
        ..VitConfig::test()
    };
    run_vit(&cfg, 2, false);
}

fn run_cnn(n: usize, f16: bool) {
    let cfg = CnnConfig::test();
    let mut g = Graph::new();
    let x = image_batch(&mut g, n, cfg.image).unwrap();
    let y = cnn(&mut g, x, &cfg, &mut RandomWeights { seed: 12, f16 }).unwrap();
    g.output(y).unwrap();
    let input = Rng::new(6).tensor(&[n, 3, cfg.image, cfg.image], 1.5);
    let name = format!("cnn n{n} f16 {f16}");
    let (out, d) = check(&name, &g, &[Feed::Tensor(&input)], ATOL, RTOL);
    assert!(d.cosine >= 0.99999, "{name}: cosine {}", d.cosine);
    assert_eq!(out[0].shape, [n, cfg.classes]);
    eprintln!("{name}: max |error| {:.2e}, cosine {}", d.max_abs, d.cosine);
}

#[test]
fn cnn_f32() {
    run_cnn(2, false);
}

#[test]
fn cnn_f16() {
    run_cnn(1, true);
}

/// The ViT from an RGBA image (preprocess in the graph), with its weights round-tripped
/// through a safetensors file: the same outputs as from the random weights directly.
#[test]
fn vit_from_image_via_safetensors() {
    let cfg = VitConfig::test();
    let pre = Preprocess {
        width: cfg.image,
        height: cfg.image,
        crop: Crop::Center,
        antialias: true,
        mean: [0.5; 3],
        std: [0.5; 3],
    };
    let build = |w: &mut dyn Weights| {
        let mut g = Graph::new();
        let img = g.image_input().unwrap();
        let x = g.preprocess(img, pre).unwrap();
        let y = vit(&mut g, x, &cfg, w).unwrap();
        g.output(y).unwrap();
        g
    };
    let mut rec = Recording {
        inner: RandomWeights {
            seed: 13,
            f16: true,
        },
        seen: Vec::new(),
    };
    let g1 = build(&mut rec);
    let named: Vec<(&str, &Tensor)> = rec.seen.iter().map(|(n, t)| (n.as_str(), t)).collect();
    let bytes = serialize(&named, &[("format", "pt")]);
    let mut st = SafeTensors::parse(&bytes).unwrap();
    let g2 = build(&mut st);
    let mut rng = Rng::new(9);
    let (w, h) = (45u32, 38u32);
    let data: Vec<u8> = (0..w * h * 4).map(|_| rng.below(256) as u8).collect();
    let feed = [Feed::Rgba {
        width: w,
        height: h,
        data: &data,
    }];
    let (a, _) = check("vit image", &g1, &feed, ATOL, RTOL);
    let (b, _) = check("vit image (safetensors)", &g2, &feed, ATOL, RTOL);
    assert_eq!(a, b);
}

/// The calibration benchmark model builds and has the expected size.
#[test]
fn vit_tiny_builds() {
    let cfg = VitConfig::tiny();
    let mut g = Graph::new();
    let x = image_batch(&mut g, 1, cfg.image).unwrap();
    let mut rec = Recording {
        inner: RandomWeights::new(1),
        seen: Vec::new(),
    };
    let y = vit(&mut g, x, &cfg, &mut rec).unwrap();
    g.output(y).unwrap();
    let params: usize = rec.seen.iter().map(|(_, t)| t.len()).sum();
    assert!(
        (5_600_000..5_900_000).contains(&params),
        "{params} parameters"
    );
    assert_eq!(g.shape(y), [1, 512]);
}
