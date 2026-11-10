//! The AI mask networks with fixed-seed random weights: EfficientSAM's encoder and
//! decoder and UperNet-ConvNeXt, whole, GPU against the CPU reference (with f16 and f32
//! weights), plus the host-side pieces (prompt encoding, bicubic position resize, input
//! sizes). The real checkpoints are checked against their references in `masks_oracle.rs`.

mod common;

use common::check;
use specular_infer::models::{RandomWeights, Weights};
use specular_infer::sam::{self, PointKind, Prompt, PromptTables, SamConfig};
use specular_infer::upernet::{self, UperConfig};
use specular_infer::{DType, Feed, Graph};

/// Whole networks: f32 sums over thousands of terms in a different order.
const NET: (f32, f32) = (2e-3, 2e-3);

#[test]
fn sam_decoder_gpu_matches_cpu() {
    let mut w = RandomWeights::new(5);
    let tables = PromptTables::read(&mut w).unwrap();
    let g = sam::decoder(&mut w, &tables).unwrap();
    let mut rng = specular_infer::tensor::Rng::new(9);
    let emb = rng.tensor(&[1, sam::DIM, sam::GRID, sam::GRID], 1.0);
    let prompt = Prompt {
        points: vec![
            ([0.4, 0.5], PointKind::Positive),
            ([0.1, 0.9], PointKind::Negative),
            ([0.2, 0.2], PointKind::BoxTopLeft),
            ([0.8, 0.7], PointKind::BoxBottomRight),
        ],
    };
    let sparse = prompt.embed(&tables).unwrap();
    let (out, d) = check(
        "sam decoder",
        &g,
        &[Feed::Tensor(&emb), Feed::Tensor(&sparse)],
        NET.0,
        NET.1,
    );
    assert_eq!(out[0].shape, [1, 3, sam::MASK, sam::MASK]);
    assert_eq!(out[1].shape, [1, 3]);
    assert!(d.cosine > 0.99999, "{d:?}");
    assert!(out[0].to_f32().iter().all(|v| v.is_finite()));
}

#[test]
fn sam_encoder_gpu_matches_cpu() {
    // f16 weights (what an install keeps); about a minute of CPU reference.
    {
        let f16 = true;
        let mut w = RandomWeights { seed: 3, f16 };
        let g = {
            let mut g = Graph::new();
            let x = g
                .input(&[1, 3, sam::INPUT, sam::INPUT], DType::F32)
                .unwrap();
            let e = sam::encoder_from(&mut g, x, &mut w, SamConfig::tiny()).unwrap();
            g.output(e).unwrap();
            g
        };
        let x = specular_infer::tensor::Rng::new(4).tensor(&[1, 3, sam::INPUT, sam::INPUT], 1.0);
        let (out, d) = check(
            &format!("sam encoder f16 {f16}"),
            &g,
            &[Feed::Tensor(&x)],
            NET.0,
            NET.1,
        );
        assert_eq!(out[0].shape, [1, sam::DIM, sam::GRID, sam::GRID]);
        assert!(d.cosine > 0.99999, "{d:?}");
    }
}

#[test]
fn upernet_gpu_matches_cpu() {
    let mut w = RandomWeights { seed: 8, f16: true };
    let c = UperConfig {
        widths: [16, 32, 48, 64],
        depths: [1, 1, 2, 1],
    };
    let mut g = Graph::new();
    let x = g.input(&[1, 3, 192, 384], DType::F32).unwrap();
    let p = upernet::probabilities(&mut g, x, &mut w, &c, &[upernet::class::SKY, 12]).unwrap();
    g.output(p).unwrap();
    let x = specular_infer::tensor::Rng::new(2).tensor(&[1, 3, 192, 384], 1.0);
    let (out, d) = check("upernet", &g, &[Feed::Tensor(&x)], NET.0, NET.1);
    assert_eq!(out[0].shape, [1, 2, 48, 96]);
    assert!(d.cosine > 0.99999, "{d:?}");
    assert!(out[0].to_f32().iter().all(|&v| (0.0..=1.0).contains(&v)));
    // Sides must allow whole pooling bins.
    let mut g = Graph::new();
    let x = g.input(&[1, 3, 200, 384], DType::F32).unwrap();
    assert!(upernet::logits(&mut g, x, &mut w, &c).is_err());
}

#[test]
fn upernet_input_sizes() {
    assert_eq!(upernet::input_size(6000, 4000, 768), (768, 576));
    assert_eq!(upernet::input_size(4000, 6000, 768), (576, 768));
    assert_eq!(upernet::input_size(1000, 1000, 768), (768, 768));
    assert_eq!(upernet::input_size(8000, 1000, 768), (768, 192));
    assert_eq!(upernet::input_size(100, 100, 100), (192, 192));
}

#[test]
fn prompt_encoding() {
    let mut w = RandomWeights::new(1);
    let t = PromptTables::read(&mut w).unwrap();
    let p = Prompt {
        points: vec![([0.5, 0.5], PointKind::Positive)],
    };
    let e = p.embed(&t).unwrap();
    assert_eq!(e.shape, [1, sam::MAX_POINTS, sam::DIM]);
    let v = e.to_f32();
    // Padding rows are all the same (the invalid point).
    assert_eq!(v[sam::DIM..2 * sam::DIM], v[5 * sam::DIM..]);
    // The positive point differs from a negative one at the same place by exactly the
    // learnt "point" embedding.
    let n = Prompt {
        points: vec![([0.5, 0.5], PointKind::Negative)],
    }
    .embed(&t)
    .unwrap()
    .to_f32();
    let pt = w
        .get("prompt_encoder.point_embeddings.weight", &[1, sam::DIM])
        .unwrap()
        .to_f32();
    for j in 0..sam::DIM {
        assert!((v[j] - n[j] - pt[j]).abs() < 1e-6);
    }
    let seven = Prompt {
        points: vec![([0.1, 0.1], PointKind::Positive); 7],
    };
    assert!(seven.embed(&t).is_err());
    // The dense encoding is sin/cos pairs: each pair on the unit circle.
    let d = t.dense().to_f32();
    let h = sam::DIM / 2;
    for tok in [0, 100, 4095] {
        let r = &d[tok * sam::DIM..][..sam::DIM];
        for j in 0..h {
            assert!((r[j] * r[j] + r[h + j] * r[h + j] - 1.0).abs() < 1e-5);
        }
    }
}

#[test]
fn bicubic_positions() {
    // A constant table stays constant; a linear ramp stays a ramp inside (bicubic is exact
    // on linear data away from the clamped edges).
    let (from, to, d) = (14, 64, 2);
    let mut table = vec![0f32; from * from * d];
    for y in 0..from {
        for x in 0..from {
            table[(y * from + x) * d] = 3.0;
            table[(y * from + x) * d + 1] = x as f32;
        }
    }
    let r = sam::resize_positions(&table, from, to, d).to_f32();
    for y in 0..to {
        for x in 0..to {
            let o = (y * to + x) * d;
            assert!((r[o] - 3.0).abs() < 1e-5);
            let src = (x as f32 + 0.5) * from as f32 / to as f32 - 0.5;
            // Keys' cubic with a = −0.75 (PyTorch's) only follows a ramp closely.
            if (2.0..from as f32 - 3.0).contains(&src) {
                assert!((r[o + 1] - src).abs() < 0.06, "{x}: {} vs {src}", r[o + 1]);
            }
            if x > 0 {
                assert!(r[o + 1] >= r[o + 1 - d] - 1e-6, "not monotonic at {x}");
            }
        }
    }
}
