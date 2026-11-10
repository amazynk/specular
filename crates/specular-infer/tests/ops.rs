//! Every op, GPU against the CPU reference, over odd shapes. Tolerances are
//! |gpu − cpu| ≤ atol + rtol·|cpu| per element (stated per test); data is scaled so outputs
//! are O(1).

mod common;

use common::{check, check_with, contexts, ctx};
use specular_infer::gpu::Context;
use specular_infer::graph::{
    BatchNorm, BinaryOp, Conv2d, Crop, MatMul, Pool2d, PoolKind, Preprocess, ReduceOp, ResizeMode,
    fold_batchnorm,
};
use specular_infer::tensor::{Rng, compare};
use specular_infer::{Act, DType, Feed, Graph, Tensor, cpu};

const ACTS: [Act; 11] = [
    Act::Identity,
    Act::Relu,
    Act::Relu6,
    Act::Gelu,
    Act::GeluTanh,
    Act::Silu,
    Act::Sigmoid,
    Act::Tanh,
    Act::HardSigmoid,
    Act::HardSwish,
    Act::Identity,
];

/// Elementwise ops: transcendental functions differ by a few ulp between the GPU and libm.
const EW: (f32, f32) = (2e-6, 2e-5);
/// Accumulating ops (matmul, conv, attention): f32 sums in a different order.
const ACC: (f32, f32) = (1e-4, 1e-4);
/// Row statistics (norms, softmax) and resampling.
const ROW: (f32, f32) = (1e-5, 1e-4);
/// Image preprocessing: the GPU may contract the tap centre into an FMA, moving filter
/// weights by an ulp; normalised outputs (÷ std ≈ 0.27) amplify that. 2e-4 is under 1/70 of
/// an 8-bit level.
const PRE: (f32, f32) = (2e-4, 1e-4);

#[test]
fn unary_activations() {
    let mut rng = Rng::new(1);
    let x = rng.tensor(&[3, 5, 37], 7.0);
    for (i, act) in ACTS.into_iter().enumerate() {
        let mut g = Graph::new();
        let a = g.input(&[3, 5, 37], DType::F32).unwrap();
        let (mul, add) = if i == 10 { (-0.5, 1.25) } else { (1.0, 0.0) };
        let y = g.unary(a, act, mul, add).unwrap();
        g.output(y).unwrap();
        check(&format!("{act:?}"), &g, &[Feed::Tensor(&x)], EW.0, EW.1);
    }
}

#[test]
fn f16_input_and_cast() {
    let mut rng = Rng::new(2);
    let x = rng.tensor(&[7, 9, 3], 3.0).to_f16();
    let mut g = Graph::new();
    let a = g.input(&[7, 9, 3], DType::F16).unwrap();
    let y = g.act(a, Act::Silu).unwrap();
    let r = g.reshape(a, &[189]).unwrap();
    let c = g.cast_f16(y).unwrap();
    g.output(y).unwrap();
    g.output(r).unwrap();
    g.output(c).unwrap();
    // The cast's output is f16 and rounds the same way on both (round to nearest even), up
    // to a rounding boundary the activation's last ulp can cross: one f16 ulp.
    check("f16", &g, &[Feed::Tensor(&x)], 1e-3, 1e-3);
}

#[test]
fn binary_broadcasting() {
    let shapes: [(&[usize], &[usize]); 6] = [
        (&[2, 3, 5, 7], &[3, 1, 7]),
        (&[1], &[4, 5]),
        (&[5, 1], &[1, 6]),
        (&[2, 1, 3, 1], &[1, 4, 1, 5]),
        (&[33], &[33]),
        (&[3, 0, 2], &[1, 2]),
    ];
    let mut rng = Rng::new(3);
    for (sa, sb) in shapes {
        let a = rng.tensor(sa, 2.0);
        let mut b = rng.tensor(sb, 1.0);
        // Away from zero, for the division.
        if let specular_infer::tensor::Data::F32(v) = &mut b.data {
            for x in v {
                *x = x.signum() * (0.25 + x.abs());
            }
        }
        for op in [
            BinaryOp::Add,
            BinaryOp::Sub,
            BinaryOp::Mul,
            BinaryOp::Div,
            BinaryOp::Max,
            BinaryOp::Min,
        ] {
            let mut g = Graph::new();
            let x = g.input(sa, DType::F32).unwrap();
            let y = g.input(sb, DType::F32).unwrap();
            let z = g.binary(op, x, y).unwrap();
            let w = g.binary(op, y, x).unwrap();
            g.output(z).unwrap();
            g.output(w).unwrap();
            let name = format!("{op:?} {sa:?} {sb:?}");
            check(&name, &g, &[Feed::Tensor(&a), Feed::Tensor(&b)], EW.0, EW.1);
        }
    }
}

/// a `[.., m, k]` · b (`[.., k, n]` or `[.., n, k]`) with optional bias, act and f16 b.
fn matmul_case(sa: &[usize], sb: &[usize], trans_b: bool, bias: bool, act: Act, f16: bool) {
    let mut rng = Rng::new(sa.iter().chain(sb).fold(7, |h, &d| h * 31 + d as u64));
    let k = sa[sa.len() - 1];
    let n = if trans_b {
        sb[sb.len() - 2]
    } else {
        sb[sb.len() - 1]
    };
    let a = rng.tensor(sa, 1.0);
    let mut b = rng.tensor(sb, 1.0 / (k as f32).sqrt());
    if f16 {
        b = b.to_f16();
    }
    let mut g = Graph::new();
    let x = g.input(sa, DType::F32).unwrap();
    let w = g.constant(b).unwrap();
    let bv = bias.then(|| g.constant(rng.tensor(&[n], 0.5)).unwrap());
    let y = g.matmul(x, w, bv, MatMul { trans_b, act }).unwrap();
    g.output(y).unwrap();
    let name = format!("matmul {sa:?} x {sb:?} t{trans_b} {act:?} f16 {f16}");
    check(&name, &g, &[Feed::Tensor(&a)], ACC.0, ACC.1);
}

#[test]
fn matmul_shapes() {
    for &(m, k, n) in &[
        (1, 1, 1),
        (7, 13, 5),
        (64, 64, 64),
        (65, 17, 63),
        (129, 300, 67),
        (3, 1000, 2),
        (200, 3, 130),
    ] {
        matmul_case(&[m, k], &[k, n], false, false, Act::Identity, false);
        matmul_case(&[m, k], &[n, k], true, true, Act::Gelu, false);
        matmul_case(&[m, k], &[n, k], true, true, Act::Identity, true);
    }
}

#[test]
fn matmul_batches() {
    matmul_case(&[2, 3, 9, 20], &[20, 11], false, true, Act::Relu, false);
    matmul_case(
        &[2, 1, 9, 20],
        &[1, 3, 11, 20],
        true,
        false,
        Act::Identity,
        true,
    );
    matmul_case(&[4, 70, 33], &[4, 33, 70], false, false, Act::Silu, false);
    matmul_case(&[5, 17], &[3, 17, 6], false, true, Act::Identity, false);
    matmul_case(
        &[3, 1, 6, 16],
        &[2, 16, 5],
        false,
        false,
        Act::Identity,
        true,
    );
}

/// A matmul whose `a` is a transposed view: exercises the m-major A loader.
#[test]
fn matmul_of_f16_constants() {
    let mut rng = Rng::new(4);
    let mut g = Graph::new();
    let a = g.constant(rng.tensor(&[33, 47], 1.0).to_f16()).unwrap();
    let b = g.constant(rng.tensor(&[47, 21], 0.15).to_f16()).unwrap();
    let y = g
        .matmul(
            a,
            b,
            None,
            MatMul {
                trans_b: false,
                act: Act::Tanh,
            },
        )
        .unwrap();
    g.output(y).unwrap();
    check("f16 x f16", &g, &[], ACC.0, ACC.1);
}

/// Residual adds after products and convolutions run in the GEMM epilogue where that is
/// safe (the product read only by the add, same shapes), and stay separate otherwise.
#[test]
fn residual_adds() {
    let mut rng = Rng::new(9);
    let mut g = Graph::new();
    let x = g.input(&[37, 50], DType::F32).unwrap();
    let r = g.input(&[37, 24], DType::F32).unwrap();
    let mm = |act| MatMul {
        trans_b: false,
        act,
    };
    let w1 = g.constant(rng.tensor(&[50, 24], 0.14)).unwrap();
    let b1 = g.constant(rng.tensor(&[24], 0.5)).unwrap();
    let y1 = g.matmul(x, w1, Some(b1), mm(Act::Gelu)).unwrap();
    let a1 = g.add(r, y1).unwrap(); // fused (the product second)
    let w2 = g.constant(rng.tensor(&[24, 24], 0.2).to_f16()).unwrap();
    let y2 = g.matmul(a1, w2, None, mm(Act::Identity)).unwrap();
    let a2 = g.add(y2, a1).unwrap(); // fused (the other operand is a skipped add)
    let w3 = g.constant(rng.tensor(&[24, 24], 0.2)).unwrap();
    let y3 = g.matmul(a2, w3, None, mm(Act::Relu)).unwrap();
    let a3 = g.add(y3, a2).unwrap(); // not fused: y3 is an output
    let c = g.constant(rng.tensor(&[24], 1.0)).unwrap();
    let a4 = g.add(a3, c).unwrap(); // broadcast, not a product
    g.output(y3).unwrap();
    g.output(a4).unwrap();
    // Convolutions: implicit GEMM and pointwise, each plus its input.
    let xi = g.input(&[2, 6, 9, 9], DType::F32).unwrap();
    let wc = g.constant(rng.tensor(&[6, 6, 3, 3], 0.13)).unwrap();
    let yc = g.conv2d(xi, wc, None, Conv2d::same(3, 1)).unwrap();
    let ac = g.add(yc, xi).unwrap(); // fused
    let wp = g.constant(rng.tensor(&[6, 6, 1, 1], 0.4)).unwrap();
    let bp = g.constant(rng.tensor(&[6], 0.5)).unwrap();
    let yp = g
        .conv2d(ac, wp, Some(bp), Conv2d::default().act(Act::Relu6))
        .unwrap();
    let ap = g.add(ac, yp).unwrap(); // fused, and an output
    g.output(ap).unwrap();
    let feeds = [
        rng.tensor(&[37, 50], 1.0),
        rng.tensor(&[37, 24], 1.0),
        rng.tensor(&[2, 6, 9, 9], 1.0),
    ];
    let feeds: Vec<Feed> = feeds.iter().map(Feed::Tensor).collect();
    check("residual adds", &g, &feeds, ACC.0, ACC.1);
    if let Some(c) = ctx() {
        let gg = specular_infer::gpu::GpuGraph::new(c, &g).unwrap();
        // 11 nodes, 4 adds in epilogues.
        assert_eq!(gg.dispatches(), 7);
    }
}

struct ConvCase {
    n: usize,
    c: usize,
    h: usize,
    w: usize,
    o: usize,
    k: [usize; 2],
    p: Conv2d,
    bias: bool,
    f16: bool,
}

fn conv_case(cc: ConvCase) {
    let ConvCase {
        n,
        c,
        h,
        w,
        o,
        k,
        p,
        bias,
        f16,
    } = cc;
    let mut rng = Rng::new((n * 1000 + c * 100 + h * 10 + w + o) as u64);
    let cg = c / p.groups;
    let x = rng.tensor(&[n, c, h, w], 1.0);
    let mut wt = rng.tensor(
        &[o, cg, k[0], k[1]],
        1.0 / ((cg * k[0] * k[1]) as f32).sqrt(),
    );
    if f16 {
        wt = wt.to_f16();
    }
    let mut g = Graph::new();
    let xi = g.input(&[n, c, h, w], DType::F32).unwrap();
    let wv = g.constant(wt).unwrap();
    let bv = bias.then(|| g.constant(rng.tensor(&[o], 0.5)).unwrap());
    let y = g.conv2d(xi, wv, bv, p).unwrap();
    g.output(y).unwrap();
    let name = format!("conv {n}x{c}x{h}x{w} -> {o} k{k:?} {p:?} f16 {f16}");
    check(&name, &g, &[Feed::Tensor(&x)], ACC.0, ACC.1);
}

#[test]
fn conv2d_general() {
    let base = |n, c, h, w, o, k, p| ConvCase {
        n,
        c,
        h,
        w,
        o,
        k,
        p,
        bias: true,
        f16: false,
    };
    conv_case(base(1, 3, 17, 13, 5, [3, 3], Conv2d::same(3, 1)));
    conv_case(base(
        2,
        3,
        33,
        31,
        16,
        [3, 3],
        Conv2d::same(3, 2).act(Act::Relu6),
    ));
    conv_case(ConvCase {
        bias: false,
        ..base(
            1,
            5,
            12,
            10,
            7,
            [5, 3],
            Conv2d {
                stride: [2, 1],
                pad: [2, 0, 1, 3],
                dilation: [2, 3],
                ..Conv2d::default().act(Act::Silu)
            },
        )
    });
    // Grouped (not depthwise).
    conv_case(base(2, 4, 9, 11, 6, [3, 3], Conv2d::same(3, 2).groups(2)));
    // A 1×1 strided conv is not pointwise: implicit GEMM.
    conv_case(base(1, 6, 9, 7, 4, [1, 1], Conv2d::same(1, 2)));
    // Patch embedding: kernel = stride.
    conv_case(ConvCase {
        f16: true,
        ..base(
            2,
            3,
            32,
            32,
            24,
            [8, 8],
            Conv2d {
                stride: [8, 8],
                ..Conv2d::default()
            },
        )
    });
    // Large K (C·KH·KW = 576) and P spanning several tiles.
    conv_case(base(
        1,
        64,
        15,
        15,
        70,
        [3, 3],
        Conv2d::same(3, 1).act(Act::HardSwish),
    ));
}

#[test]
fn conv2d_depthwise() {
    let dw = |n, c, h, w, k: usize, p: Conv2d, f16| ConvCase {
        n,
        c,
        h,
        w,
        o: c,
        k: [k, k],
        p: p.groups(c),
        bias: true,
        f16,
    };
    conv_case(dw(1, 8, 15, 9, 3, Conv2d::same(3, 2), false));
    conv_case(dw(2, 5, 7, 13, 3, Conv2d::same(3, 1).act(Act::Relu6), true));
    conv_case(dw(1, 3, 20, 20, 5, Conv2d::same(5, 1), false));
    conv_case(dw(
        1,
        4,
        11,
        12,
        3,
        Conv2d {
            dilation: [2, 2],
            pad: [2, 1, 2, 0],
            stride: [1, 2],
            ..Conv2d::default()
        },
        false,
    ));
}

#[test]
fn conv2d_pointwise() {
    let pw = |n, c, h, w, o, f16, act| ConvCase {
        n,
        c,
        h,
        w,
        o,
        k: [1, 1],
        p: Conv2d::default().act(act),
        bias: true,
        f16,
    };
    conv_case(pw(1, 6, 7, 5, 9, false, Act::Identity));
    conv_case(pw(3, 16, 8, 8, 96, true, Act::Relu6));
    conv_case(pw(2, 70, 13, 11, 33, false, Act::Gelu));
    // Multiplier depthwise (Cg = 1, Og = 2) takes the implicit GEMM.
    conv_case(ConvCase {
        k: [3, 3],
        p: Conv2d::same(3, 1).groups(4),
        ..pw(1, 4, 9, 9, 8, false, Act::Identity)
    });
}

#[test]
fn batchnorm_folding() {
    let mut rng = Rng::new(5);
    let (c, o) = (6, 10);
    let x = rng.tensor(&[2, c, 9, 7], 1.0);
    let w = rng.tensor(&[o, c, 3, 3], 0.15);
    let b = rng.tensor(&[o], 0.2);
    let bn = BatchNorm {
        gamma: (0..o).map(|_| 1.0 + rng.sym(0.3)).collect(),
        beta: (0..o).map(|_| rng.sym(0.3)).collect(),
        mean: (0..o).map(|_| rng.sym(0.3)).collect(),
        var: (0..o).map(|_| 0.3 + rng.unit()).collect(),
        eps: 1e-5,
    };
    // Unfolded: conv, then the batch norm as broadcast mul + add.
    let mut g1 = Graph::new();
    let xi = g1.input(&[2, c, 9, 7], DType::F32).unwrap();
    let wv = g1.constant(w.clone()).unwrap();
    let bv = g1.constant(b.clone()).unwrap();
    let y = g1.conv2d(xi, wv, Some(bv), Conv2d::same(3, 1)).unwrap();
    let y = g1.batch_norm(y, &bn).unwrap();
    g1.output(y).unwrap();
    let (unfolded, _) = check("batch_norm", &g1, &[Feed::Tensor(&x)], ACC.0, ACC.1);
    // Folded into the conv.
    let (fw, fb) = fold_batchnorm(&w, Some(&b), &bn).unwrap();
    let mut g2 = Graph::new();
    let xi = g2.input(&[2, c, 9, 7], DType::F32).unwrap();
    let wv = g2.constant(fw).unwrap();
    let bv = g2.constant(fb).unwrap();
    let y = g2.conv2d(xi, wv, Some(bv), Conv2d::same(3, 1)).unwrap();
    g2.output(y).unwrap();
    let (folded, _) = check("folded", &g2, &[Feed::Tensor(&x)], ACC.0, ACC.1);
    let d = compare(&folded[0].to_f32(), &unfolded[0].to_f32(), 1e-5, 1e-5);
    assert!(
        d.worst <= 1.0,
        "folding changed the result by {}",
        d.max_abs
    );
    // Rank 2: the last axis is the channel.
    let mut g3 = Graph::new();
    let t = rng.tensor(&[5, o], 1.0);
    let xi = g3.input(&[5, o], DType::F32).unwrap();
    let y = g3.batch_norm(xi, &bn).unwrap();
    g3.output(y).unwrap();
    check("batch_norm rank 2", &g3, &[Feed::Tensor(&t)], EW.0, EW.1);
}

#[test]
fn norms_and_softmax() {
    let mut rng = Rng::new(6);
    for shape in [
        &[3, 7, 1][..],
        &[2, 5, 33],
        &[1, 300],
        &[4, 1025],
        &[2, 3, 256],
        &[1, 4097],
    ] {
        let d = *shape.last().unwrap();
        let x = rng.tensor(shape, 6.0);
        let mut g = Graph::new();
        let xi = g.input(shape, DType::F32).unwrap();
        let gamma = g.constant(rng.tensor(&[d], 1.0)).unwrap();
        let beta = g.constant(rng.tensor(&[d], 0.5)).unwrap();
        let ln = g.layer_norm(xi, gamma, beta, 1e-5).unwrap();
        let l2 = g.l2_norm(xi, 1e-6).unwrap();
        let sm = g.softmax(xi).unwrap();
        g.output(ln).unwrap();
        g.output(l2).unwrap();
        g.output(sm).unwrap();
        check(
            &format!("rows {shape:?}"),
            &g,
            &[Feed::Tensor(&x)],
            ROW.0,
            ROW.1,
        );
    }
    // A constant row: layer norm must give beta, softmax uniform; large logits stay finite.
    let mut g = Graph::new();
    let xi = g.input(&[2, 9], DType::F32).unwrap();
    let gamma = g.constant(Tensor::f32(&[9], vec![2.0; 9])).unwrap();
    let beta = g.constant(Tensor::f32(&[9], vec![0.5; 9])).unwrap();
    let ln = g.layer_norm(xi, gamma, beta, 1e-5).unwrap();
    let sm = g.softmax(xi).unwrap();
    g.output(ln).unwrap();
    g.output(sm).unwrap();
    let mut v = vec![3.0; 9];
    v.extend([1e4, -1e4, 80.0, 0.0, 88.0, -88.0, 3e4, 5.0, 5.0]);
    let x = Tensor::f32(&[2, 9], v);
    let (out, _) = check("rows special", &g, &[Feed::Tensor(&x)], ROW.0, ROW.1);
    assert!(out[0].to_f32()[..9].iter().all(|&y| (y - 0.5).abs() < 1e-6));
    assert!(out[1].to_f32().iter().all(|y| y.is_finite()));
    // Softmax subtracts the row maximum: huge logits are fine. (Layer norm is not checked
    // this far out: the GPU's f32 variance overflows past ~1e19 where the CPU's f64 does
    // not.)
    let mut g = Graph::new();
    let xi = g.input(&[1, 6], DType::F32).unwrap();
    let sm = g.softmax(xi).unwrap();
    g.output(sm).unwrap();
    let x = Tensor::f32(&[1, 6], vec![1e30, -1e30, 1e30, 0.0, 3e38, -3e38]);
    let (out, _) = check("softmax huge", &g, &[Feed::Tensor(&x)], ROW.0, ROW.1);
    assert_eq!(out[0].to_f32(), [0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
}

/// The row reductions at workgroup sizes that are not a multiple of the subgroup size (so
/// subgroups are partial, and there are more or fewer of them than at 256 threads), over
/// rows shorter and longer than the workgroup: the subgroup path must not depend on how
/// the driver lays invocations out in subgroups, nor on their size.
#[test]
fn row_reductions_at_any_workgroup_size() {
    let base = contexts();
    if let Some(c) = base.first() {
        eprintln!("row kernels: subgroups {}", c.features().subgroups);
    }
    let mut rng = Rng::new(16);
    for threads in [1, 7, 32, 48, 63, 96, 100, 129, 200, 255, 256] {
        let ctxs: Vec<Context> = base
            .iter()
            .map(|c| Context::with_row_threads(c.device(), c.queue(), c.features(), threads))
            .collect();
        for c in &ctxs {
            assert_eq!(c.row_threads(), threads);
        }
        for shape in [
            &[3, 1][..],
            &[2, 3],
            &[5, 31],
            &[2, 65],
            &[3, 191],
            &[2, 257],
            &[2, 1001],
        ] {
            let d = shape[1];
            let x = rng.tensor(shape, 6.0);
            let mut g = Graph::new();
            let xi = g.input(shape, DType::F32).unwrap();
            let gamma = g.constant(rng.tensor(&[d], 1.0)).unwrap();
            let beta = g.constant(rng.tensor(&[d], 0.5)).unwrap();
            let ln = g.layer_norm(xi, gamma, beta, 1e-5).unwrap();
            let l2 = g.l2_norm(xi, 1e-6).unwrap();
            let sm = g.softmax(xi).unwrap();
            g.output(ln).unwrap();
            g.output(l2).unwrap();
            g.output(sm).unwrap();
            let name = format!("rows {shape:?} at {threads} threads");
            check_with(&name, &ctxs, &g, &[Feed::Tensor(&x)], ROW.0, ROW.1);
        }
        // Causal attention's softmax runs in place on the scores.
        let (t, dh) = (19, 8);
        let qkv = rng.tensor(&[1, t, 3 * dh], 1.0);
        let mut g = Graph::new();
        let xi = g.input(&[1, t, 3 * dh], DType::F32).unwrap();
        let y = g.attention_packed(xi, 1, None, true).unwrap();
        g.output(y).unwrap();
        let name = format!("causal attention at {threads} threads");
        check_with(&name, &ctxs, &g, &[Feed::Tensor(&qkv)], ACC.0, ACC.1);
    }
}

#[test]
fn attention() {
    let mut rng = Rng::new(8);
    for (n, tq, tk, d, heads, causal) in [
        (2, 5, 9, 12, 3, false),
        (2, 5, 9, 12, 3, true),
        (1, 1, 1, 8, 1, false),
        (1, 65, 65, 64, 4, false),
        (3, 17, 33, 16, 2, true),
        (1, 197, 197, 192, 3, false),
    ] {
        let q = rng.tensor(&[n, tq, d], 1.0);
        let k = rng.tensor(&[n, tk, d], 1.0);
        let v = rng.tensor(&[n, tk, d], 1.0);
        let mut g = Graph::new();
        let (qi, ki, vi) = (
            g.input(&[n, tq, d], DType::F32).unwrap(),
            g.input(&[n, tk, d], DType::F32).unwrap(),
            g.input(&[n, tk, d], DType::F32).unwrap(),
        );
        let y = g.attention(qi, ki, vi, heads, None, causal).unwrap();
        g.output(y).unwrap();
        let name = format!("attention {n} {tq}x{tk} d{d} h{heads} causal {causal}");
        let feeds = [Feed::Tensor(&q), Feed::Tensor(&k), Feed::Tensor(&v)];
        check(&name, &g, &feeds, ACC.0, ACC.1);
    }
    for (n, t, d, heads, causal) in [(2, 17, 16, 4, true), (1, 50, 48, 3, false)] {
        let qkv = rng.tensor(&[n, t, 3 * d], 1.0);
        let mut g = Graph::new();
        let x = g.input(&[n, t, 3 * d], DType::F32).unwrap();
        let y = g.attention_packed(x, heads, Some(0.3), causal).unwrap();
        g.output(y).unwrap();
        let name = format!("packed attention {n}x{t} d{d} h{heads} causal {causal}");
        check(&name, &g, &[Feed::Tensor(&qkv)], ACC.0, ACC.1);
    }
}

#[test]
fn pooling_and_reduce() {
    let mut rng = Rng::new(9);
    let x = rng.tensor(&[2, 3, 13, 11], 3.0);
    for kind in [PoolKind::Max, PoolKind::Avg] {
        for (kernel, stride, pad) in [
            ([2, 2], [2, 2], [0; 4]),
            ([3, 3], [2, 2], [1; 4]),
            ([3, 2], [1, 3], [2, 0, 1, 1]),
            ([13, 11], [1, 1], [0; 4]),
        ] {
            let mut g = Graph::new();
            let xi = g.input(&[2, 3, 13, 11], DType::F32).unwrap();
            let p = Pool2d {
                kind,
                kernel,
                stride,
                pad,
            };
            let y = g.pool2d(xi, p).unwrap();
            g.output(y).unwrap();
            check(&format!("{p:?}"), &g, &[Feed::Tensor(&x)], ROW.0, ROW.1);
        }
    }
    let mut g = Graph::new();
    let xi = g.input(&[2, 3, 13, 11], DType::F32).unwrap();
    let y = g.global_avg_pool(xi).unwrap();
    g.output(y).unwrap();
    check("global avg pool", &g, &[Feed::Tensor(&x)], ROW.0, ROW.1);
    let x = rng.tensor(&[3, 5, 7], 3.0);
    for op in [ReduceOp::Sum, ReduceOp::Mean, ReduceOp::Max] {
        for axis in 0..3 {
            let mut g = Graph::new();
            let xi = g.input(&[3, 5, 7], DType::F32).unwrap();
            let y = g.reduce(xi, op, axis).unwrap();
            g.output(y).unwrap();
            check(
                &format!("{op:?} axis {axis}"),
                &g,
                &[Feed::Tensor(&x)],
                ROW.0,
                ROW.1,
            );
        }
    }
    let big = rng.tensor(&[2, 5000], 1.0);
    let mut g = Graph::new();
    let xi = g.input(&[2, 5000], DType::F32).unwrap();
    let y = g.reduce(xi, ReduceOp::Sum, 1).unwrap();
    g.output(y).unwrap();
    check("sum 5000", &g, &[Feed::Tensor(&big)], 1e-4, 1e-4);
}

#[test]
fn resize() {
    let mut rng = Rng::new(10);
    let x = rng.tensor(&[2, 3, 7, 5], 2.0);
    for mode in [ResizeMode::Bilinear, ResizeMode::Nearest] {
        for (h, w) in [(13, 4), (3, 11), (1, 1), (7, 5), (28, 20)] {
            let mut g = Graph::new();
            let xi = g.input(&[2, 3, 7, 5], DType::F32).unwrap();
            let y = g.resize(xi, h, w, mode).unwrap();
            g.output(y).unwrap();
            check(
                &format!("{mode:?} {h}x{w}"),
                &g,
                &[Feed::Tensor(&x)],
                ROW.0,
                ROW.1,
            );
        }
    }
}

#[test]
fn copies_are_exact() {
    let mut rng = Rng::new(11);
    let x = rng.tensor(&[2, 3, 4, 5], 1.0);
    let y = rng.tensor(&[2, 1, 4, 5], 1.0);
    let mut g = Graph::new();
    let xi = g.input(&[2, 3, 4, 5], DType::F32).unwrap();
    let yi = g.input(&[2, 1, 4, 5], DType::F32).unwrap();
    for perm in [[0, 1, 2, 3], [3, 2, 1, 0], [0, 2, 3, 1], [1, 0, 3, 2]] {
        let t = g.transpose(xi, &perm).unwrap();
        g.output(t).unwrap();
    }
    let s = g
        .slice(xi, &[1, 0, 1, 0], &[2, 3, 4, 5], &[1, 2, 2, 3])
        .unwrap();
    g.output(s).unwrap();
    let e = g.slice_axis(xi, 2, 2, 2).unwrap();
    g.output(e).unwrap();
    for axis in 0..4 {
        let other = if axis == 1 { yi } else { xi };
        let c = g.concat(&[xi, other, xi], axis).unwrap();
        g.output(c).unwrap();
    }
    let r = g.reshape(xi, &[6, 20]).unwrap();
    let r = g.transpose(r, &[1, 0]).unwrap();
    g.output(r).unwrap();
    let table = g.reshape(xi, &[24, 5]).unwrap();
    let ids = g
        .constant(Tensor::f32(
            &[2, 4],
            vec![0.0, 23.0, 23.9, -3.0, 100.0, f32::NAN, 7.5, 1.0],
        ))
        .unwrap();
    let rows = g.gather(table, ids).unwrap();
    g.output(rows).unwrap();
    check(
        "copies",
        &g,
        &[Feed::Tensor(&x), Feed::Tensor(&y)],
        0.0,
        0.0,
    );
}

fn image(w: usize, h: usize, seed: u64) -> Vec<u8> {
    // Smooth gradients plus noise, so both the filter and the edges matter.
    let mut rng = Rng::new(seed);
    let mut v = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            v.push(((x * 255) / w.max(1)) as u8 ^ (rng.below(32) as u8));
            v.push(((y * 255) / h.max(1)) as u8);
            v.push(rng.below(256) as u8);
            v.push(255);
        }
    }
    v
}

fn preprocess_graph(p: Preprocess) -> Graph {
    let mut g = Graph::new();
    let img = g.image_input().unwrap();
    let y = g.preprocess(img, p).unwrap();
    g.output(y).unwrap();
    g
}

const CLIP: Preprocess = Preprocess {
    width: 24,
    height: 20,
    crop: Crop::Center,
    antialias: true,
    mean: [0.481_454_66, 0.457_827_5, 0.408_210_73],
    std: [0.268_629_54, 0.261_302_6, 0.275_777_1],
};

#[test]
fn preprocess_from_rgba() {
    for (w, h) in [(61, 47), (47, 61), (7, 5), (24, 20), (1, 1), (300, 17)] {
        let data = image(w, h, (w * h) as u64);
        for p in [
            CLIP,
            Preprocess {
                crop: Crop::Stretch,
                antialias: false,
                ..CLIP
            },
            Preprocess {
                width: 64,
                height: 64,
                ..CLIP
            },
        ] {
            let g = preprocess_graph(p);
            let feed = Feed::Rgba {
                width: w as u32,
                height: h as u32,
                data: &data,
            };
            check(
                &format!("preprocess {w}x{h} {p:?}"),
                &g,
                &[feed],
                PRE.0,
                PRE.1,
            );
        }
    }
}

/// One GPU graph fed images of different sizes in turn (the image buffer grows).
#[test]
fn preprocess_resized_inputs() {
    let Some(c) = ctx() else { return };
    let g = preprocess_graph(CLIP);
    let mut gg = specular_infer::gpu::GpuGraph::new(c, &g).unwrap();
    for (w, h) in [(10, 10), (200, 150), (33, 9), (200, 150)] {
        let data = image(w, h, 1);
        let feed = [Feed::Rgba {
            width: w as u32,
            height: h as u32,
            data: &data,
        }];
        let got = gg.run(&feed).unwrap();
        let want = cpu::run(&g, &feed, 1).unwrap();
        let d = compare(&got[0].to_f32(), &want[0].to_f32(), PRE.0, PRE.1);
        assert!(d.worst <= 1.0, "{w}x{h}: {d:?}");
    }
    let short = [Feed::Rgba {
        width: 4,
        height: 4,
        data: &[0; 63],
    }];
    assert!(gg.run(&short).is_err());
    assert!(cpu::run(&g, &short, 1).is_err());
}

#[test]
fn preprocess_from_texture() {
    let Some(c) = ctx() else { return };
    let (w, h) = (97u32, 61u32);
    let data = image(w as usize, h as usize, 3);
    let tex = c.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("preprocess test"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    c.queue().write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    for p in [
        CLIP,
        Preprocess {
            crop: Crop::Stretch,
            ..CLIP
        },
    ] {
        let g = preprocess_graph(p);
        let mut gg = specular_infer::gpu::GpuGraph::new(c, &g).unwrap();
        let got = gg.run(&[Feed::Texture(&tex)]).unwrap();
        let want = cpu::run(
            &g,
            &[Feed::Rgba {
                width: w,
                height: h,
                data: &data,
            }],
            1,
        )
        .unwrap();
        let d = compare(&got[0].to_f32(), &want[0].to_f32(), PRE.0, PRE.1);
        assert!(d.worst <= 1.0, "texture {p:?}: {d:?}");
        // The same graph then takes a buffer.
        let again = gg
            .run(&[Feed::Rgba {
                width: w,
                height: h,
                data: &data,
            }])
            .unwrap();
        let d = compare(&again[0].to_f32(), &want[0].to_f32(), PRE.0, PRE.1);
        assert!(d.worst <= 1.0, "buffer after texture: {d:?}");
    }
    // The CPU cannot read a texture.
    let g = preprocess_graph(CLIP);
    assert!(cpu::run(&g, &[Feed::Texture(&tex)], 1).is_err());
}

#[test]
fn bad_feeds_are_errors() {
    let mut g = Graph::new();
    let x = g.input(&[2, 3], DType::F32).unwrap();
    let y = g.act(x, Act::Relu).unwrap();
    g.output(y).unwrap();
    let wrong = Tensor::f32(&[3, 2], vec![0.0; 6]);
    let half = Tensor::f32(&[2, 3], vec![0.0; 6]).to_f16();
    for feeds in [&[][..], &[Feed::Tensor(&wrong)], &[Feed::Tensor(&half)]] {
        assert!(cpu::run(&g, feeds, 1).is_err());
        if let Some(c) = ctx() {
            let mut gg = specular_infer::gpu::GpuGraph::new(c, &g).unwrap();
            assert!(gg.run(feeds).is_err());
        }
    }
}

/// Buffer planning reuses memory: a long chain needs a few slots, not one per value.
#[test]
fn planner_reuses_buffers() {
    let Some(c) = ctx() else { return };
    let mut g = Graph::new();
    let x = g.input(&[256, 256], DType::F32).unwrap();
    let mut y = x;
    for i in 0..40 {
        y = g.unary(y, Act::Tanh, 1.0, i as f32 * 0.01).unwrap();
    }
    g.output(y).unwrap();
    let gg = specular_infer::gpu::GpuGraph::new(c, &g).unwrap();
    let one = 256 * 256 * 4;
    assert!(
        gg.memory_bytes() <= 4 * one + 64 * 1024,
        "{} bytes for a chain of {one}-byte values",
        gg.memory_bytes()
    );
    assert_eq!(gg.dispatches(), 40);
}

/// The calibration self-test passes on this GPU.
#[test]
fn self_test() {
    let Some(c) = ctx() else { return };
    specular_infer::calibrate::self_test(c).unwrap();
}

/// Layer norm across channels (SAM's LayerNorm2d, ConvNeXt's channels-first norm),
/// against the row layer norm of the transposed map.
#[test]
fn channel_norm() {
    let mut rng = Rng::new(23);
    for shape in [[1, 1, 3, 5], [2, 7, 5, 3], [1, 96, 9, 11], [1, 256, 4, 4]] {
        let x = rng.tensor(&shape, 4.0);
        let c = shape[1];
        let mut g = Graph::new();
        let xi = g.input(&shape, DType::F32).unwrap();
        let gamma = g.constant(rng.tensor(&[c], 1.0)).unwrap();
        let beta = g.constant(rng.tensor(&[c], 0.5)).unwrap();
        let cn = g.channel_norm(xi, gamma, beta, 1e-6).unwrap();
        g.output(cn).unwrap();
        // The same through transposes and the row norm.
        let t = g.transpose(xi, &[0, 2, 3, 1]).unwrap();
        let ln = g.layer_norm(t, gamma, beta, 1e-6).unwrap();
        let back = g.transpose(ln, &[0, 3, 1, 2]).unwrap();
        g.output(back).unwrap();
        let (out, _) = check(
            &format!("channel norm {shape:?}"),
            &g,
            &[Feed::Tensor(&x)],
            ROW.0,
            ROW.1,
        );
        let d = compare(&out[0].to_f32(), &out[1].to_f32(), 1e-6, 1e-6);
        assert!(
            d.worst <= 1.0,
            "channel norm {shape:?} vs layer norm: {d:?}"
        );
    }
    let mut g = Graph::new();
    let xi = g.input(&[1, 3, 2, 2], DType::F32).unwrap();
    let gamma = g.constant(Tensor::f32(&[2], vec![1.0; 2])).unwrap();
    assert!(g.channel_norm(xi, gamma, gamma, 1e-6).is_err());
}

/// Naive scatter form of a transposed convolution: every input tap adds its kernel to the
/// output, then the padding is cropped.
fn conv_transpose_scatter(
    x: &Tensor,
    w: &Tensor,
    b: &[f32],
    stride: [usize; 2],
    pad: [usize; 4],
) -> Vec<f32> {
    let (xs, ws) = (&x.shape, &w.shape);
    let (n, c, ih, iw) = (xs[0], xs[1], xs[2], xs[3]);
    let (o, kh, kw) = (ws[1], ws[2], ws[3]);
    let (fh, fw) = ((ih - 1) * stride[0] + kh, (iw - 1) * stride[1] + kw);
    let (xv, wv) = (x.to_f32(), w.to_f32());
    let mut full = vec![0f64; n * o * fh * fw];
    for ni in 0..n {
        for ic in 0..c {
            for iy in 0..ih {
                for ix in 0..iw {
                    let xval = xv[((ni * c + ic) * ih + iy) * iw + ix] as f64;
                    for oc in 0..o {
                        for ky in 0..kh {
                            for kx in 0..kw {
                                let wv = wv[((ic * o + oc) * kh + ky) * kw + kx] as f64;
                                let (y, xx) = (iy * stride[0] + ky, ix * stride[1] + kx);
                                full[((ni * o + oc) * fh + y) * fw + xx] += xval * wv;
                            }
                        }
                    }
                }
            }
        }
    }
    let (oh, ow) = (fh - pad[0] - pad[2], fw - pad[1] - pad[3]);
    let mut out = Vec::with_capacity(n * o * oh * ow);
    for ni in 0..n {
        for oc in 0..o {
            for y in 0..oh {
                for xx in 0..ow {
                    let v = full[((ni * o + oc) * fh + y + pad[0]) * fw + xx + pad[1]];
                    out.push(v as f32 + b[oc]);
                }
            }
        }
    }
    out
}

type ConvTCase = ([usize; 4], [usize; 4], [usize; 2], [usize; 4], bool, Act);

/// Transposed convolutions (SAM's 2× upscaling head), against a scatter-form oracle
/// and GPU = CPU, with and without bias, odd strides, kernels and crops.
#[test]
fn conv_transpose2d() {
    let mut rng = Rng::new(32);
    let cases: [ConvTCase; 6] = [
        (
            [1, 3, 4, 5],
            [3, 2, 2, 2],
            [2, 2],
            [0; 4],
            true,
            Act::Identity,
        ),
        (
            [2, 4, 3, 3],
            [4, 5, 3, 3],
            [2, 2],
            [1, 1, 1, 1],
            false,
            Act::Gelu,
        ),
        (
            [1, 2, 5, 4],
            [2, 3, 4, 3],
            [3, 2],
            [0, 1, 2, 0],
            true,
            Act::Relu,
        ),
        (
            [1, 1, 1, 1],
            [1, 1, 2, 2],
            [2, 2],
            [0; 4],
            true,
            Act::Identity,
        ),
        (
            [1, 16, 8, 8],
            [16, 8, 2, 2],
            [2, 2],
            [0; 4],
            true,
            Act::Gelu,
        ),
        (
            [1, 5, 6, 7],
            [5, 4, 1, 1],
            [1, 1],
            [0; 4],
            false,
            Act::Identity,
        ),
    ];
    for (xs, ws, stride, pad, bias, act) in cases {
        let x = rng.tensor(&xs, 1.0);
        let w = rng.tensor(&ws, 0.5);
        let bv = rng.tensor(&[ws[1]], 0.3);
        let mut g = Graph::new();
        let xi = g.input(&xs, DType::F32).unwrap();
        let wc = g.constant(w.clone()).unwrap();
        let bc = bias.then(|| g.constant(bv.clone()).unwrap());
        let y = g.conv_transpose2d(xi, wc, bc, stride, pad, act).unwrap();
        g.output(y).unwrap();
        let name = format!("conv transpose {xs:?} {ws:?} s{stride:?} p{pad:?}");
        let (out, _) = check(&name, &g, &[Feed::Tensor(&x)], ACC.0, ACC.1);
        let b = if bias { bv.to_f32() } else { vec![0.0; ws[1]] };
        let want: Vec<f32> = conv_transpose_scatter(&x, &w, &b, stride, pad)
            .into_iter()
            .map(|v| act.apply(v))
            .collect();
        let d = compare(&out[0].to_f32(), &want, 1e-5, 1e-5);
        assert!(d.worst <= 1.0, "{name} vs scatter: {d:?}");
    }
    let mut g = Graph::new();
    let xi = g.input(&[1, 3, 2, 2], DType::F32).unwrap();
    let w = g.constant(Tensor::zeros(&[2, 3, 2, 2])).unwrap();
    assert!(
        g.conv_transpose2d(xi, w, None, [2, 2], [0; 4], Act::Identity)
            .is_err()
    );
    let w = g.constant(Tensor::zeros(&[3, 3, 2, 2])).unwrap();
    assert!(
        g.conv_transpose2d(xi, w, None, [2, 2], [2, 2, 2, 2], Act::Identity)
            .is_err()
    );
}

/// Group norm (SAM's upscaling head uses one group), GPU = CPU and against a naive
/// per-group reference.
#[test]
fn group_norm() {
    let mut rng = Rng::new(17);
    for (shape, groups) in [
        ([1, 4, 3, 5], 1),
        ([2, 6, 4, 4], 3),
        ([1, 64, 32, 32], 1),
        ([1, 8, 1, 1], 8),
    ] {
        let x = rng.tensor(&shape, 3.0);
        let c = shape[1];
        let (gv, bv) = (rng.tensor(&[c], 1.0), rng.tensor(&[c], 0.5));
        let mut g = Graph::new();
        let xi = g.input(&shape, DType::F32).unwrap();
        let gamma = g.constant(gv.clone()).unwrap();
        let beta = g.constant(bv.clone()).unwrap();
        let y = g.group_norm(xi, gamma, beta, groups, 1e-5).unwrap();
        g.output(y).unwrap();
        let name = format!("group norm {shape:?}/{groups}");
        let (out, _) = check(&name, &g, &[Feed::Tensor(&x)], ROW.0, ROW.1);
        let (xv, gv, bv) = (x.to_f32(), gv.to_f32(), bv.to_f32());
        let hw = shape[2] * shape[3];
        let per = c / groups * hw;
        let mut want = vec![0f32; xv.len()];
        for (r, chunk) in xv.chunks(per).enumerate() {
            let m = chunk.iter().map(|&a| a as f64).sum::<f64>() / per as f64;
            let v = chunk.iter().map(|&a| (a as f64 - m).powi(2)).sum::<f64>() / per as f64;
            for (i, &a) in chunk.iter().enumerate() {
                let ch = (r % groups) * (c / groups) + i / hw;
                want[r * per + i] = ((a as f64 - m) / (v + 1e-5).sqrt()) as f32 * gv[ch] + bv[ch];
            }
        }
        let d = compare(&out[0].to_f32(), &want, 1e-6, 1e-5);
        assert!(d.worst <= 1.0, "{name} vs naive: {d:?}");
    }
    let mut g = Graph::new();
    let xi = g.input(&[1, 6, 2, 2], DType::F32).unwrap();
    let gamma = g.constant(Tensor::f32(&[6], vec![1.0; 6])).unwrap();
    assert!(g.group_norm(xi, gamma, gamma, 4, 1e-5).is_err());
}

/// Convolutions with a few output channels per group run on the direct kernel (Big-LaMa's
/// last 7×7, 64 → 3 channels; a grouped one with 2 per group; f16 weights).
#[test]
fn conv2d_few_outputs() {
    for (c, o, groups, k, f16) in [
        (64, 3, 1, 7, false),
        (64, 3, 1, 7, true),
        (12, 4, 1, 3, false),
        (8, 4, 2, 3, false),
    ] {
        conv_case(ConvCase {
            n: 1,
            c,
            h: 21,
            w: 19,
            o,
            k: [k, k],
            p: Conv2d {
                groups,
                ..Conv2d::same(k, 1)
            },
            bias: true,
            f16,
        });
    }
}

/// Large transposes that swap two runs of axes go through tiles: exact, with sizes not a
/// multiple of the tile.
#[test]
fn tiled_transposes_are_exact() {
    let mut rng = Rng::new(12);
    let shape = [2, 37, 45, 41];
    let x = rng.tensor(&shape, 1.0);
    let mut g = Graph::new();
    let xi = g.input(&shape, DType::F32).unwrap();
    for perm in [[0, 3, 1, 2], [0, 2, 3, 1], [2, 3, 0, 1], [0, 1, 3, 2]] {
        let t = g.transpose(xi, &perm).unwrap();
        g.output(t).unwrap();
    }
    check("tiled transposes", &g, &[Feed::Tensor(&x)], 0.0, 0.0);
}

/// A product over 4 G multiply-adds is split along its rows of tiles into several
/// dispatches (each short enough for a submission of its own): same values.
#[test]
fn large_products_split_into_row_pieces() {
    let mut rng = Rng::new(13);
    let (m, k, n) = (1000, 4200, 1030);
    let x = rng.tensor(&[m, k], 1.0);
    let w = rng.tensor(&[k, n], 1.0 / (k as f32).sqrt());
    let mut g = Graph::new();
    let xi = g.input(&[m, k], DType::F32).unwrap();
    let wv = g.constant(w).unwrap();
    let y = g
        .matmul(
            xi,
            wv,
            None,
            MatMul {
                trans_b: false,
                act: Act::Identity,
            },
        )
        .unwrap();
    g.output(y).unwrap();
    check("split product", &g, &[Feed::Tensor(&x)], ACC.0, ACC.1);
    if let Some(c) = ctx() {
        let gg = specular_infer::gpu::GpuGraph::new(c, &g).unwrap();
        assert!(gg.dispatches() >= 2, "{}", gg.dispatches());
    }
}
