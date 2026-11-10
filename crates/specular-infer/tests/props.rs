//! Property tests over shapes: the graph builder never panics (bad shapes are errors), the
//! shapes it infers match what both engines produce, and the GPU matches the CPU on random
//! valid shapes (matmul, conv2d, broadcasting, transpose/slice/concat, pooling, resize).

mod common;

use common::check;
use proptest::prelude::*;
use specular_infer::graph::{BinaryOp, Conv2d, MatMul, Pool2d, PoolKind, ResizeMode};
use specular_infer::tensor::Rng;
use specular_infer::{Act, DType, Feed, Graph};

fn cfg(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(cfg(256))]

    /// Arbitrary (possibly invalid) parameters: building returns Ok or Err, never panics.
    #[test]
    fn builder_never_panics(
        xs in prop::collection::vec(0usize..9, 0..6),
        ws in prop::collection::vec(0usize..9, 0..6),
        stride in prop::array::uniform2(0usize..4),
        pad in prop::array::uniform4(0usize..4),
        dil in prop::array::uniform2(0usize..3),
        groups in 0usize..5,
        perm in prop::collection::vec(0usize..6, 0..6),
        axis in 0usize..6,
        k in prop::array::uniform2(0usize..5),
        heads in 0usize..5,
    ) {
        let mut g = Graph::new();
        let x = g.input(&xs, DType::F32).ok();
        let w = g.input(&ws, DType::F32).ok();
        if let (Some(x), Some(w)) = (x, w) {
            let p = Conv2d { stride, pad, dilation: dil, groups, act: Act::Relu };
            let _ = g.conv2d(x, w, None, p);
            let _ = g.matmul(x, w, None, MatMul { trans_b: groups % 2 == 0, act: Act::Identity });
            let _ = g.binary(BinaryOp::Add, x, w);
            let _ = g.transpose(x, &perm);
            let _ = g.concat(&[x, w], axis);
            let _ = g.reduce(x, specular_infer::graph::ReduceOp::Max, axis);
            let _ = g.pool2d(x, Pool2d { kind: PoolKind::Avg, kernel: k, stride, pad });
            let _ = g.resize(x, k[0], k[1], ResizeMode::Bilinear);
            let _ = g.attention(x, w, w, heads, None, false);
            let _ = g.attention_packed(x, heads, None, true);
            let _ = g.softmax(x);
            let _ = g.layer_norm(x, w, w, 1e-5);
            let _ = g.gather(x, w);
            let _ = g.slice(x, &perm, &xs, &ws);
            let _ = g.reshape(x, &ws);
            let _ = g.global_avg_pool(x);
            let _ = g.preprocess(x, specular_infer::graph::Preprocess {
                width: k[0], height: k[1], crop: specular_infer::graph::Crop::Center,
                antialias: true, mean: [0.0; 3], std: [1.0; 3],
            });
        }
    }
}

proptest! {
    #![proptest_config(cfg(24))]

    #[test]
    fn matmul_matches(
        m in 1usize..90, k in 1usize..140, n in 1usize..90,
        batch in prop::collection::vec(1usize..4, 0..3),
        trans_b in any::<bool>(), bias in any::<bool>(), f16 in any::<bool>(),
        seed in any::<u64>(),
    ) {
        let mut rng = Rng::new(seed);
        let mut sa = batch.clone();
        sa.extend([m, k]);
        let sb = if trans_b { vec![n, k] } else { vec![k, n] };
        let a = rng.tensor(&sa, 1.0);
        let mut b = rng.tensor(&sb, 1.0 / (k as f32).sqrt());
        if f16 {
            b = b.to_f16();
        }
        let mut g = Graph::new();
        let x = g.input(&sa, DType::F32).unwrap();
        let w = g.constant(b).unwrap();
        let bv = bias.then(|| g.constant(rng.tensor(&[n], 0.5)).unwrap());
        let y = g.matmul(x, w, bv, MatMul { trans_b, act: Act::Identity }).unwrap();
        g.output(y).unwrap();
        let (out, _) = check("prop matmul", &g, &[Feed::Tensor(&a)], 1e-4, 1e-4);
        let mut want = sa.clone();
        *want.last_mut().unwrap() = n;
        prop_assert_eq!(&out[0].shape, &want);
    }

    #[test]
    fn conv2d_matches(
        n in 1usize..3, cg in 1usize..5, og in 1usize..5, groups in 1usize..4,
        h in 1usize..14, w in 1usize..14, kh in 1usize..4, kw in 1usize..4,
        stride in prop::array::uniform2(1usize..3), dil in prop::array::uniform2(1usize..3),
        pad in prop::array::uniform4(0usize..3), depthwise in any::<bool>(),
        seed in any::<u64>(),
    ) {
        let (c, o) = if depthwise { (groups, groups) } else { (cg * groups, og * groups) };
        let p = Conv2d { stride, pad, dilation: dil, groups, act: Act::Silu };
        let mut rng = Rng::new(seed);
        let x = rng.tensor(&[n, c, h, w], 1.0);
        let wt = rng.tensor(&[o, c / groups, kh, kw], 0.5);
        let mut g = Graph::new();
        let xi = g.input(&[n, c, h, w], DType::F32).unwrap();
        let wv = g.constant(wt).unwrap();
        let bv = g.constant(rng.tensor(&[o], 0.2)).unwrap();
        // Kernels larger than the padded input are shape errors: skip those.
        let Ok(y) = g.conv2d(xi, wv, Some(bv), p) else {
            return Ok(());
        };
        g.output(y).unwrap();
        check("prop conv2d", &g, &[Feed::Tensor(&x)], 1e-4, 1e-4);
    }

    #[test]
    fn broadcasting_matches(
        sa in prop::collection::vec(1usize..5, 1..5),
        mask in prop::collection::vec(any::<bool>(), 4),
        drop in 0usize..3,
        seed in any::<u64>(),
    ) {
        // b: a's shape with some axes set to 1 and leading axes dropped.
        let sb: Vec<usize> = sa.iter().enumerate().skip(drop.min(sa.len() - 1))
            .map(|(i, &d)| if mask[i % 4] { 1 } else { d }).collect();
        let mut rng = Rng::new(seed);
        let a = rng.tensor(&sa, 1.0);
        let b = rng.tensor(&sb, 1.0);
        let mut g = Graph::new();
        let x = g.input(&sa, DType::F32).unwrap();
        let y = g.input(&sb, DType::F32).unwrap();
        let z = g.binary(BinaryOp::Sub, y, x).unwrap();
        g.output(z).unwrap();
        let (out, _) = check("prop broadcast", &g, &[Feed::Tensor(&a), Feed::Tensor(&b)], 0.0, 0.0);
        prop_assert_eq!(&out[0].shape, &sa);
    }

    #[test]
    fn copies_match(
        shape in prop::collection::vec(1usize..6, 1..6),
        perm_seed in any::<u64>(),
        cut in prop::collection::vec((0usize..6, 0usize..6, 1usize..4), 6),
        axis in 0usize..6,
        seed in any::<u64>(),
    ) {
        let r = shape.len();
        let mut perm: Vec<usize> = (0..r).collect();
        let mut prng = Rng::new(perm_seed);
        for i in (1..r).rev() {
            perm.swap(i, prng.below(i + 1));
        }
        let starts: Vec<usize> = (0..r).map(|i| cut[i].0.min(shape[i])).collect();
        let ends: Vec<usize> = (0..r).map(|i| (starts[i] + cut[i].1).min(shape[i])).collect();
        let steps: Vec<usize> = (0..r).map(|i| cut[i].2).collect();
        let axis = axis % r;
        let x = Rng::new(seed).tensor(&shape, 1.0);
        let mut g = Graph::new();
        let xi = g.input(&shape, DType::F32).unwrap();
        let t = g.transpose(xi, &perm).unwrap();
        let s = g.slice(xi, &starts, &ends, &steps).unwrap();
        let c = g.concat(&[xi, xi], axis).unwrap();
        g.output(t).unwrap();
        g.output(s).unwrap();
        g.output(c).unwrap();
        check("prop copies", &g, &[Feed::Tensor(&x)], 0.0, 0.0);
    }

    #[test]
    fn pool_and_resize_match(
        h in 1usize..20, w in 1usize..20,
        k in prop::array::uniform2(1usize..5), stride in prop::array::uniform2(1usize..4),
        pad in prop::array::uniform4(0usize..2), max in any::<bool>(),
        oh in 1usize..25, ow in 1usize..25, bilinear in any::<bool>(),
        seed in any::<u64>(),
    ) {
        let x = Rng::new(seed).tensor(&[1, 2, h, w], 2.0);
        let mut g = Graph::new();
        let xi = g.input(&[1, 2, h, w], DType::F32).unwrap();
        let kind = if max { PoolKind::Max } else { PoolKind::Avg };
        if let Ok(p) = g.pool2d(xi, Pool2d { kind, kernel: k, stride, pad }) {
            g.output(p).unwrap();
        }
        let mode = if bilinear { ResizeMode::Bilinear } else { ResizeMode::Nearest };
        let r = g.resize(xi, oh, ow, mode).unwrap();
        g.output(r).unwrap();
        check("prop pool/resize", &g, &[Feed::Tensor(&x)], 1e-5, 1e-4);
    }
}
