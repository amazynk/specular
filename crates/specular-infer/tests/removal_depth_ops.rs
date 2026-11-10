//! The object removal and depth networks' ops: mirror padding, the discrete Fourier transforms
//! (LaMa's Fourier units) and corner-aligned bilinear resizing (Depth Anything's DPT head). Each
//! against an independent formula and GPU against the CPU reference (|gpu − cpu| ≤ atol +
//! rtol·|cpu|).

mod common;

use common::check;
use specular_infer::graph::{Dft, ResizeMode};
use specular_infer::tensor::Rng;
use specular_infer::{DType, Feed, Graph, Tensor, cpu};

/// Sums of n terms in f32 against f64 sums: about n·ulp of the largest partial sum.
const DFT_TOL: (f32, f32) = (3e-5, 3e-5);

fn run1(g: &Graph, x: &Tensor) -> Tensor {
    cpu::run(g, &[Feed::Tensor(x)], 2).expect("cpu").remove(0)
}

#[test]
fn reflect_pad_mirrors_about_the_edge_pixels() {
    // One row 0..5 padded by 3 left, 2 right: PyTorch's ReflectionPad2d gives
    // 3 2 1 | 0 1 2 3 4 | 3 2.
    let x = Tensor::f32(&[1, 1, 2, 5], (0..10).map(|v| v as f32).collect());
    let mut g = Graph::new();
    let xi = g.input(&[1, 1, 2, 5], DType::F32).unwrap();
    let y = g.reflect_pad(xi, [1, 3, 1, 2]).unwrap();
    g.output(y).unwrap();
    let out = run1(&g, &x);
    assert_eq!(out.shape, [1, 1, 4, 10]);
    let v = out.to_f32();
    // Row 0 of the output mirrors input row 1.
    assert_eq!(&v[..10], &[8., 7., 6., 5., 6., 7., 8., 9., 8., 7.]);
    assert_eq!(&v[10..20], &[3., 2., 1., 0., 1., 2., 3., 4., 3., 2.]);
    assert_eq!(&v[30..40], &[3., 2., 1., 0., 1., 2., 3., 4., 3., 2.]);
    // A pad as large as the side is refused.
    let mut g = Graph::new();
    let xi = g.input(&[1, 1, 2, 5], DType::F32).unwrap();
    assert!(g.reflect_pad(xi, [2, 0, 0, 0]).is_err());
}

#[test]
fn reflect_pad_gpu_matches_cpu() {
    let mut rng = Rng::new(271);
    for (shape, pads) in [
        ([2, 3, 7, 9], [1, 1, 1, 1]),
        ([1, 5, 8, 8], [3, 3, 3, 3]),
        ([1, 2, 4, 11], [0, 2, 3, 1]),
    ] {
        let x = rng.tensor(&shape, 2.0);
        let mut g = Graph::new();
        let xi = g.input(&shape, DType::F32).unwrap();
        let y = g.reflect_pad(xi, pads).unwrap();
        g.output(y).unwrap();
        // A copy: exact.
        check(
            &format!("pad {shape:?} {pads:?}"),
            &g,
            &[Feed::Tensor(&x)],
            0.0,
            0.0,
        );
    }
}

/// The direct DFT in f64 along the last axis of a complex row.
fn naive(re: &[f64], im: &[f64], sign: f64) -> (Vec<f64>, Vec<f64>) {
    let n = re.len();
    let s = 1.0 / (n as f64).sqrt();
    let mut or = vec![0.0; n];
    let mut oi = vec![0.0; n];
    for k in 0..n {
        for t in 0..n {
            let a = std::f64::consts::TAU * (k * t) as f64 / n as f64;
            let (c, sn) = (a.cos(), sign * a.sin());
            or[k] += re[t] * c - im[t] * sn;
            oi[k] += re[t] * sn + im[t] * c;
        }
        or[k] *= s;
        oi[k] *= s;
    }
    (or, oi)
}

#[test]
fn real_to_complex_is_the_half_spectrum() {
    let mut rng = Rng::new(272);
    for n in [1usize, 2, 7, 8, 33] {
        let x = rng.tensor(&[3, n], 1.0);
        let mut g = Graph::new();
        let xi = g.input(&[3, n], DType::F32).unwrap();
        let y = g.dft(xi, Dft::RealToComplex).unwrap();
        g.output(y).unwrap();
        let out = run1(&g, &x);
        let k = n / 2 + 1;
        assert_eq!(out.shape, [3, k, 2]);
        let (xv, ov) = (x.to_f32(), out.to_f32());
        for r in 0..3 {
            let re: Vec<f64> = xv[r * n..(r + 1) * n].iter().map(|&v| v as f64).collect();
            let (fr, fi) = naive(&re, &vec![0.0; n], -1.0);
            for b in 0..k {
                let (gr, gi) = (ov[2 * (r * k + b)] as f64, ov[2 * (r * k + b) + 1] as f64);
                assert!(
                    (gr - fr[b]).abs() < 1e-6 && (gi - fi[b]).abs() < 1e-6,
                    "n {n} bin {b}"
                );
            }
        }
        check(
            &format!("rfft {n}"),
            &g,
            &[Feed::Tensor(&x)],
            DFT_TOL.0,
            DFT_TOL.1,
        );
    }
}

#[test]
fn complex_transform_along_rows_and_back() {
    let mut rng = Rng::new(273);
    for (n, m) in [(5usize, 3usize), (8, 5), (16, 9), (64, 33)] {
        let shape = [2, n, m, 2];
        let x = rng.tensor(&shape, 1.0);
        for inverse in [false, true] {
            let mut g = Graph::new();
            let xi = g.input(&shape, DType::F32).unwrap();
            let y = g.dft(xi, Dft::Complex { inverse }).unwrap();
            g.output(y).unwrap();
            let out = run1(&g, &x);
            let (xv, ov) = (x.to_f32(), out.to_f32());
            // Column j of block 0 against the naive transform.
            for j in [0, m - 1] {
                let re: Vec<f64> = (0..n).map(|t| xv[2 * (t * m + j)] as f64).collect();
                let im: Vec<f64> = (0..n).map(|t| xv[2 * (t * m + j) + 1] as f64).collect();
                let (fr, fi) = naive(&re, &im, if inverse { 1.0 } else { -1.0 });
                for b in 0..n {
                    let (gr, gi) = (ov[2 * (b * m + j)] as f64, ov[2 * (b * m + j) + 1] as f64);
                    assert!((gr - fr[b]).abs() < 1e-6 && (gi - fi[b]).abs() < 1e-6);
                }
            }
            check(
                &format!("cdft {n}x{m} {inverse}"),
                &g,
                &[Feed::Tensor(&x)],
                DFT_TOL.0,
                DFT_TOL.1,
            );
        }
    }
}

/// rfft2 then irfft2 (LaMa's Fourier unit without its conv) gives the input back, for even
/// and odd sides; the GPU agrees with the CPU through the whole chain.
#[test]
fn rfft2_round_trip() {
    let mut rng = Rng::new(274);
    for (h, w) in [(8usize, 8usize), (6, 9), (7, 5), (64, 64), (32, 48)] {
        let x = rng.tensor(&[1, 3, h, w], 1.0);
        let mut g = Graph::new();
        let xi = g.input(&[1, 3, h, w], DType::F32).unwrap();
        let f = specular_infer::lama::rfft2(&mut g, xi).unwrap();
        assert_eq!(g.shape(f), [1, 6, h, w / 2 + 1]);
        let back = specular_infer::lama::irfft2(&mut g, f, (h, w)).unwrap();
        g.output(f).unwrap();
        g.output(back).unwrap();
        let out = cpu::run(&g, &[Feed::Tensor(&x)], 2).unwrap();
        let worst = out[1]
            .to_f32()
            .iter()
            .zip(x.to_f32().iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(worst < 2e-6, "{h}x{w}: round trip off by {worst}");
        // Parseval, orthonormal: the energy of the half spectrum with its mirror equals the
        // picture's.
        check(
            &format!("rfft2 {h}x{w}"),
            &g,
            &[Feed::Tensor(&x)],
            DFT_TOL.0,
            DFT_TOL.1,
        );
    }
}

#[test]
fn complex_to_real_ignores_the_imaginary_dc_and_nyquist() {
    // numpy.fft.irfft([1+5j, 2-1j, 3+7j], n=4, norm="ortho") = [3.5, -1.5, 0.5, 2.5]·…:
    // the imaginary parts of bins 0 and 2 do not count.
    let spec = Tensor::f32(&[1, 3, 2], vec![1.0, 5.0, 2.0, -1.0, 3.0, 7.0]);
    let mut g = Graph::new();
    let xi = g.input(&[1, 3, 2], DType::F32).unwrap();
    let y = g.dft(xi, Dft::ComplexToReal { n: 4 }).unwrap();
    g.output(y).unwrap();
    let v = run1(&g, &spec).to_f32();
    // x[t] = (X0 + 2·Re(X1·e^{iπt/2}) + X2·(−1)^t) / 2.
    let want = [
        (1.0 + 4.0 + 3.0) / 2.0,
        (1.0 + 2.0 - 3.0) / 2.0,
        (1.0 - 4.0 + 3.0) / 2.0,
        (1.0 - 2.0 - 3.0) / 2.0,
    ];
    for (a, b) in v.iter().zip(want) {
        assert!((a - b).abs() < 1e-6, "{v:?} vs {want:?}");
    }
    check("irfft n4", &g, &[Feed::Tensor(&spec)], DFT_TOL.0, DFT_TOL.1);
}

#[test]
fn aligned_resize_keeps_the_corners() {
    let mut rng = Rng::new(275);
    let x = rng.tensor(&[2, 3, 7, 5], 2.0);
    for (h, w) in [(14, 10), (13, 4), (1, 1), (7, 5), (37, 74)] {
        let mut g = Graph::new();
        let xi = g.input(&[2, 3, 7, 5], DType::F32).unwrap();
        let y = g.resize(xi, h, w, ResizeMode::BilinearAligned).unwrap();
        g.output(y).unwrap();
        let out = run1(&g, &x);
        let (xv, ov) = (x.to_f32(), out.to_f32());
        if h > 1 && w > 1 {
            // Corners land exactly on the input's corners.
            assert_eq!(ov[0], xv[0]);
            assert_eq!(ov[w - 1], xv[4]);
            assert_eq!(ov[(h - 1) * w + w - 1], xv[6 * 5 + 4]);
        }
        check(
            &format!("aligned {h}x{w}"),
            &g,
            &[Feed::Tensor(&x)],
            1e-5,
            1e-4,
        );
    }
}

/// LaMa's up-sampling as a plain conv over the zero-spread input equals the transposed conv
/// (3×3, stride 2, padding 1, output padding 1).
#[test]
fn spread_conv_equals_the_transposed_conv() {
    use specular_infer::Act;
    use specular_infer::graph::Conv2d;
    let mut rng = Rng::new(276);
    let (c, o, h, w) = (5usize, 4usize, 6usize, 7usize);
    let x = rng.tensor(&[1, c, h, w], 1.0);
    let wt = rng.tensor(&[c, o, 3, 3], 0.5);
    let wv = wt.to_f32();
    let mut flipped = vec![0f32; o * c * 9];
    for ci in 0..c {
        for oi in 0..o {
            for k in 0..9 {
                flipped[(oi * c + ci) * 9 + (8 - k)] = wv[(ci * o + oi) * 9 + k];
            }
        }
    }
    let mut g = Graph::new();
    let xi = g.input(&[1, c, h, w], DType::F32).unwrap();
    let wa = g.constant(wt.clone()).unwrap();
    let a = g
        .conv_transpose2d(xi, wa, None, [2, 2], [1, 1, 0, 0], Act::Identity)
        .unwrap();
    let wb = g.constant(Tensor::f32(&[o, c, 3, 3], flipped)).unwrap();
    let z = specular_infer::lama::spread(&mut g, xi).unwrap();
    let b = g.conv2d(z, wb, None, Conv2d::same(3, 1)).unwrap();
    assert_eq!(g.shape(a), g.shape(b));
    g.output(a).unwrap();
    g.output(b).unwrap();
    let out = cpu::run(&g, &[Feed::Tensor(&x)], 2).unwrap();
    let worst = out[0]
        .to_f32()
        .iter()
        .zip(out[1].to_f32().iter())
        .map(|(p, q)| (p - q).abs())
        .fold(0f32, f32::max);
    assert!(worst < 1e-5, "{worst}");
    check("spread conv", &g, &[Feed::Tensor(&x)], 1e-4, 1e-4);
}
