//! The detail ops: the noise fit against synthetic Poisson–Gaussian truth, the Anscombe
//! inverses, denoise PSNR/SSIM at several ISO-like levels, GPU = CPU for every op, tiles =
//! whole image, no halos from the sharpener, Richardson–Lucy convergence without ringing,
//! preview against full size, and no pipelines per slider move.

use super::capture::{self, Capture};
use super::cpu;
use super::denoise::Denoise;
use super::kernels::band_noise;
use super::noise::{NoiseModel, NoiseSource, RawNoise, gat, gat_inv};
use super::sharpen::Sharpen;
use super::{ImageDetail, with_image};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request, Source};
use crate::graph::tile::Tiling;
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::ops::tone::cpu::Img;
use crate::testgpu::{dev, lock};

use specular_raw::sensor::{BlackLevels, DngColour, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};

const GIB: u64 = 1 << 30;

/// xorshift64* with Box–Muller normals and a Poisson sampler.
struct Rng(u64);

impl Rng {
    fn u(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((v >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (a, b) = (self.u(), self.u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
    fn poisson(&mut self, lambda: f64) -> f64 {
        if lambda <= 0.0 {
            return 0.0;
        }
        if lambda > 60.0 {
            return (lambda + lambda.sqrt() * self.normal()).round().max(0.0);
        }
        let l = (-lambda).exp();
        let mut k = 0.0;
        let mut p = 1.0;
        loop {
            p *= self.u();
            if p <= l {
                return k;
            }
            k += 1.0;
        }
    }
    /// A Poisson–Gaussian sample of mean `x` and variance `a·x + b`.
    fn pg(&mut self, x: f64, a: f64, b: f64) -> f64 {
        a * self.poisson(x.max(0.0) / a) + b.sqrt() * self.normal()
    }
}

/// A photo-like linear scene in 0..1: flat patches at many levels, gradients, an edge, a
/// disc, fine stripes and a coloured texture.
fn scene(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
    let level = |k: f32| 0.01 * 70f32.powf(k);
    let mut c = if v < 0.5 {
        // Patches 8 x 2 at levels from 1 % to 70 %, some coloured.
        let k = ((u * 8.0).floor() + 8.0 * (v * 4.0).floor()) / 15.0;
        let l = level(k);
        let tint = [
            (1.0 + 0.3 * (k * 9.0).sin()),
            1.0,
            (1.0 + 0.3 * (k * 7.0).cos()),
        ];
        tint.map(|t| l * t)
    } else {
        let g = 0.05 + 0.4 * u;
        [g, g * 0.9, g * 0.7]
    };
    let (dx, dy) = (u - 0.7, v - 0.75);
    if dx * dx + dy * dy < 0.012 {
        c = [0.6, 0.25, 0.1];
    }
    if v > 0.55 && u < 0.3 {
        let s = 0.5 + 0.5 * (x as f32 * 0.9).sin() * (y as f32 * 0.35).cos();
        c = [0.1 + 0.3 * s, 0.1 + 0.25 * s, 0.08 + 0.2 * s];
    }
    if u + 0.3 * v > 1.05 {
        c = [0.02, 0.03, 0.05];
    }
    c
}

fn clean(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = scene(x, y, w, h);
            v.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    v
}

/// `img` with Poisson–Gaussian noise of `model` per channel.
fn noisy(img: &[f32], model: &NoiseModel, seed: u64) -> Vec<f32> {
    let mut r = Rng(seed | 1);
    img.chunks_exact(4)
        .flat_map(|p| {
            let c: [f32; 3] = std::array::from_fn(|k| {
                r.pg(
                    f64::from(p[k]),
                    f64::from(model.a[k]),
                    f64::from(model.b[k]),
                ) as f32
            });
            [c[0], c[1], c[2], p[3]]
        })
        .collect()
}

/// ISO-like noise levels in white = 1 units (about ISO 400, 1600, 6400).
fn iso_levels() -> [(&'static str, NoiseModel); 3] {
    let m = |a: f32, b: f32| NoiseModel {
        a: [a * 1.6, a, a * 1.3],
        b: [b * 2.0, b, b * 1.5],
    };
    [
        ("ISO 400", m(1.0e-4, 4.0e-8)),
        ("ISO 1600", m(4.0e-4, 6.0e-7)),
        ("ISO 6400", m(1.6e-3, 1.0e-5)),
    ]
}

/// A display-like encoding for quality measures.
fn enc(v: f32) -> f64 {
    f64::from(v.clamp(0.0, 1.0)).powf(1.0 / 2.2)
}

fn psnr(a: &[f32], b: &[f32]) -> f64 {
    let mut se = 0.0;
    let mut n = 0.0;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        for k in 0..3 {
            let d = enc(p[k]) - enc(q[k]);
            se += d * d;
            n += 1.0;
        }
    }
    10.0 * (n / se).log10()
}

/// Mean SSIM of the encoded luma over 8x8 windows (stride 4).
fn ssim(a: &[f32], b: &[f32], w: u32, h: u32) -> f64 {
    let y = |v: &[f32], x: u32, yy: u32| {
        let i = ((yy * w + x) * 4) as usize;
        0.2627 * enc(v[i]) + 0.678 * enc(v[i + 1]) + 0.0593 * enc(v[i + 2])
    };
    let (c1, c2) = (0.01f64.powi(2), 0.03f64.powi(2));
    let mut total = 0.0;
    let mut n = 0.0;
    for by in (0..h - 8).step_by(4) {
        for bx in (0..w - 8).step_by(4) {
            let (mut ma, mut mb, mut va, mut vb, mut cov) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for j in 0..8 {
                for i in 0..8 {
                    ma += y(a, bx + i, by + j);
                    mb += y(b, bx + i, by + j);
                }
            }
            ma /= 64.0;
            mb /= 64.0;
            for j in 0..8 {
                for i in 0..8 {
                    let (p, q) = (y(a, bx + i, by + j) - ma, y(b, bx + i, by + j) - mb);
                    va += p * p;
                    vb += q * q;
                    cov += p * q;
                }
            }
            va /= 63.0;
            vb /= 63.0;
            cov /= 63.0;
            total += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1.0;
        }
    }
    total / n
}

fn engine() -> Option<Engine> {
    let d = dev()?;
    Some(Engine::new(&d.device, &d.queue, Budget::fixed(2 * GIB)))
}

fn module(kind: ModuleKind, f: Fields) -> ModuleInstance {
    let mut m = ModuleInstance::new(Registry::builtin(), kind, 0);
    let mut all = m.params.to_fields();
    all.overlay(&f);
    m.params = Params::from_fields(kind, all);
    m
}

fn edit(mods: Vec<ModuleInstance>) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    for m in mods {
        e.insert(Registry::builtin(), m);
    }
    e
}

fn dn_fields(l: f32, c: f32, model: &NoiseModel) -> Fields {
    let mut f = Fields::new()
        .with("luminance", Value::Float(l))
        .with("colour", Value::Float(c));
    model.write(&mut f);
    f
}

fn render(en: &mut Engine, g: &Graph, src: &Source, req: &Request) -> Vec<f32> {
    let r = en.render(g, src, req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

fn whole(w: u32, h: u32, scale: f32) -> Request {
    Request {
        precision: Precision::F32,
        checkpoints: false,
        scale,
        ..Request::new(Roi::full(w, h))
    }
}

fn gpu_op(en: &mut Engine, img: &[f32], w: u32, h: u32, m: ModuleInstance, scale: f32) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let g = Graph::build(&edit(vec![m]), OpRegistry::builtin(), None).expect("graph");
    render(en, &g, &src, &whole(w, h, scale))
}

/// (worst |gpu − cpu|, pixels over `tol`).
fn diff(a: &[f32], b: &[f32], tol: f32) -> (f32, usize) {
    assert_eq!(a.len(), b.len());
    let mut worst = 0f32;
    let mut over = 0;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = (0..4).map(|k| (p[k] - q[k]).abs()).fold(0f32, f32::max);
        assert!(d.is_finite(), "non-finite: {p:?} vs {q:?}");
        worst = worst.max(d);
        over += usize::from(d > tol);
    }
    (worst, over)
}

// ---- Noise model -------------------------------------------------------------------

/// A Bayer sensor (RGGB, black 512, white 15000) of the scene with Poisson–Gaussian noise
/// `a·x + b` per CFA colour in normalised units.
pub(crate) fn bayer_sensor(w: u32, h: u32, a: [f64; 3], b: [f64; 3], seed: u64) -> SensorData {
    let (black, white) = (512.0f64, 15000.0f64);
    let layout = Layout::Bayer([0, 1, 1, 2]);
    let mut r = Rng(seed | 1);
    let mut samples = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let col = usize::from(layout.color(x, y));
            // Raw values are the scene before white balance (green strongest).
            let wbi = [1.0 / 1.9, 1.0, 1.0 / 1.4][col];
            let v = f64::from(scene(x, y, w, h)[col]) * wbi;
            let n = r.pg(v, a[col], b[col]);
            samples.push((black + n * (white - black)).round().clamp(0.0, 65535.0) as u16);
        }
    }
    SensorData {
        width: w,
        height: h,
        cpp: 1,
        samples,
        float: None,
        crop: Rect { x: 0, y: 0, w, h },
        meta: SensorMeta {
            layout,
            black: BlackLevels::simple([black as f32; 4], 1),
            white: white as f32,
            clip_white: white as f32,
            linearization: None,
            opcodes: Default::default(),
            opcodes_unknown: Vec::new(),
            wb: [1.9, 1.0, 1.4],
            wb_source: WbSource::AsShot,
            rgb_cam: [[1.6, -0.4, -0.2], [-0.2, 1.4, -0.2], [0.0, -0.5, 1.5]],
            matrix_source: MatrixSource::Generic,
            dng: None,
            profile: None,
            orientation: 1,
            truncated: false,
            lens: None,
            default_scale: None,
        },
    }
}

#[test]
fn noise_fit_matches_synthetic_truth() {
    let truths = [
        ([2.0e-4, 1.0e-4, 1.5e-4], [2e-7, 1e-7, 1.5e-7]),
        ([8.0e-4, 4.0e-4, 6.0e-4], [4e-6, 2e-6, 3e-6]),
        ([3.0e-3, 1.6e-3, 2.4e-3], [4e-5, 2e-5, 3e-5]),
    ];
    let mut worst = 0f64;
    for (i, (a, b)) in truths.into_iter().enumerate() {
        let s = bayer_sensor(1024, 768, a, b, 7 + i as u64);
        let fit = RawNoise::estimate(&s).expect("a fit");
        assert_eq!(fit.source, NoiseSource::Estimated);
        for k in 0..3 {
            let ea = (fit.a[k] - a[k]).abs() / a[k];
            // Variance at signal levels from deep shadow to highlights.
            let ev = [0.01, 0.05, 0.2, 0.5]
                .iter()
                .map(|&x| {
                    let t = a[k] * x + b[k];
                    ((fit.a[k] * x + fit.b[k]) - t).abs() / t
                })
                .fold(0f64, f64::max);
            eprintln!(
                "truth {i} colour {k}: a {:.3e} (truth {:.3e}, {:+.1} %), b {:.2e} (truth {:.2e}), \
                 worst variance error {:.1} %",
                fit.a[k],
                a[k],
                100.0 * (fit.a[k] - a[k]) / a[k],
                fit.b[k],
                b[k],
                100.0 * ev
            );
            worst = worst.max(ea).max(ev);
        }
    }
    assert!(worst < 0.10, "noise fit off by {:.1} %", 100.0 * worst);
}

#[test]
fn a_noise_profile_wins_over_the_fit() {
    let mut s = bayer_sensor(256, 192, [1e-3; 3], [1e-6; 3], 3);
    s.meta.dng = Some(DngColour {
        noise_profile: vec![[2e-4, 3e-7]],
        ..Default::default()
    });
    let n = RawNoise::of_sensor(&s);
    assert_eq!(n.source, NoiseSource::Profile);
    assert_eq!(n.a, [2e-4; 3]);
    assert_eq!(n.b, [3e-7; 3]);
    // Per plane.
    let p = RawNoise::from_profile(&[[1e-4, 1e-7], [2e-4, 2e-7], [3e-4, 3e-7]]).unwrap();
    assert_eq!(p.a, [1e-4, 2e-4, 3e-4]);
    assert!(RawNoise::from_profile(&[[1e-4, 1e-7], [2e-4, 2e-7]]).is_none());
}

#[test]
fn the_rgb_fit_finds_working_space_noise() {
    let (w, h) = (512, 384);
    let model = iso_levels()[1].1;
    let img = noisy(&clean(w, h), &model, 11);
    let fit = super::noise::estimate_rgb(w, h, &img).expect("fit");
    for k in 0..3 {
        let x = 0.2;
        let t = model.a[k] * x + model.b[k];
        let e = ((fit.a[k] * x + fit.b[k]) - t).abs() / t;
        assert!(e < 0.1, "channel {k}: {fit:?} vs {model:?}");
    }
}

#[test]
fn anscombe_inverses() {
    // Algebraic: exact round trip, also for negative and tiny values.
    for &(a, b) in &[(1e-3f32, 1e-6f32), (1e-4, 0.0), (5e-3, 1e-4)] {
        let (ia, s2) = (1.0 / a, b / (a * a));
        for &z in &[-0.01f32, -1e-4, 0.0, 1e-4, 0.003, 0.05, 0.5, 2.0] {
            let back = gat_inv(gat(z, ia, s2), ia, s2, 0.0);
            assert!(
                (back - z).abs() <= 2e-6 + 2e-5 * z.abs(),
                "a {a} b {b} z {z} → {back}"
            );
        }
    }
    // Exact unbiased: the inverse of E[gat(z)] is the mean, where the algebraic one is
    // biased (Mäkitalo and Foi 2013).
    let mut r = Rng(99);
    for &(lambda, sigma) in &[(2.0f64, 0.0f64), (5.0, 0.5), (20.0, 1.0), (100.0, 2.0)] {
        let n = 200_000;
        let s2 = (sigma * sigma) as f32;
        let mean: f64 = (0..n)
            .map(|_| f64::from(gat(r.pg(lambda, 1.0, sigma * sigma) as f32, 1.0, s2)))
            .sum::<f64>()
            / f64::from(n);
        let ub = f64::from(gat_inv(mean as f32, 1.0, s2, 1.0));
        let alg = f64::from(gat_inv(mean as f32, 1.0, s2, 0.0));
        eprintln!("λ {lambda} σ {sigma}: unbiased {ub:.3}, algebraic {alg:.3}");
        assert!(
            (ub - lambda).abs() < 0.03 * lambda + 0.05,
            "λ {lambda}: {ub}"
        );
        assert!((ub - lambda).abs() < (alg - lambda).abs());
    }
}

#[test]
fn band_noise_is_the_known_b3_series() {
    // Starck and Murtagh's table for the 2-D B3-spline à-trous transform.
    let known = [0.8907, 0.2007, 0.0856, 0.0413, 0.0205];
    for (j, (sd, _)) in band_noise(5).into_iter().enumerate() {
        assert!((sd - known[j]).abs() < 0.02 * known[j], "level {j}: {sd}");
    }
}

// ---- Denoise -----------------------------------------------------------------------

#[test]
fn denoise_gains_psnr_and_matches_the_cpu_reference() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (256, 192);
    let truth = clean(w, h);
    for (i, (name, model)) in iso_levels().into_iter().enumerate() {
        let img = noisy(&truth, &model, 100 + i as u64);
        let m = module(ModuleKind::DENOISE, dn_fields(0.6, 0.6, &model));
        let gpu = gpu_op(&mut en, &img, w, h, m.clone(), 1.0);
        let d = Denoise::from_fields(&m.params.to_fields());
        let full = Roi::full(w, h);
        let cref = cpu::denoise(&Img { w, h, px: &img }, &d, full, full, 1.0);
        let (worst, over) = diff(&gpu, &cref, 1e-3);
        let (p0, p1) = (psnr(&img, &truth), psnr(&gpu, &truth));
        let (s0, s1) = (ssim(&img, &truth, w, h), ssim(&gpu, &truth, w, h));
        eprintln!(
            "{name}: PSNR {p0:.2} → {p1:.2} dB, SSIM {s0:.4} → {s1:.4}; GPU vs CPU worst \
             {worst:.2e}, {over} over 1e-3"
        );
        assert!(
            over == 0,
            "{name}: GPU differs from the CPU reference by {worst}"
        );
        assert!(p1 > p0 + 3.0, "{name}: PSNR {p0} → {p1}");
        assert!(s1 > s0, "{name}: SSIM {s0} → {s1}");
    }
}

#[test]
fn wavelets_and_nlm_each_help() {
    let (w, h) = (192, 128);
    let truth = clean(w, h);
    let model = iso_levels()[1].1;
    let img = noisy(&truth, &model, 5);
    let full = Roi::full(w, h);
    let run = |l: f32, c: f32, nlm: bool| {
        let d = Denoise {
            luminance: l,
            colour: c,
            nlm,
            noise: model,
            ..Denoise::default()
        };
        psnr(
            &cpu::denoise(&Img { w, h, px: &img }, &d, full, full, 1.0),
            &truth,
        )
    };
    let (base, chroma, wav, both) = (
        psnr(&img, &truth),
        run(0.0, 0.6, false),
        run(0.6, 0.6, false),
        run(0.6, 0.6, true),
    );
    eprintln!(
        "PSNR: noisy {base:.2}, chroma {chroma:.2}, + luma wavelets {wav:.2}, + NLM {both:.2}"
    );
    assert!(chroma > base && wav > chroma && both > wav);
}

#[test]
fn denoise_strength_zero_is_left_out_and_image_facts_are_set() {
    let model = iso_levels()[0].1;
    let off = module(ModuleKind::DENOISE, dn_fields(0.0, 0.0, &model));
    let g = Graph::build(&edit(vec![off]), OpRegistry::builtin(), None).unwrap();
    assert!(g.nodes.is_empty());
    assert_eq!(g.identity, vec![(ModuleKind::DENOISE, 0)]);
    // The image's noise and lens go on a copy, only for enabled detail modules.
    let e = edit(vec![
        module(ModuleKind::DENOISE, Fields::new()),
        module(ModuleKind::CAPTURE_SHARPENING, Fields::new()),
    ]);
    let info = ImageDetail {
        noise: Some(model),
        capture_sigma: Some(1.1),
        photo: None,
    };
    let w = with_image(&e, &info).expect("set");
    let f = w.module(ModuleKind::DENOISE, 0).unwrap().params.to_fields();
    assert_eq!(NoiseModel::from_fields(&f), Some(model));
    let c = w
        .module(ModuleKind::CAPTURE_SHARPENING, 0)
        .unwrap()
        .params
        .to_fields();
    assert_eq!(Capture::from_fields(&c).auto_sigma, 1.1);
    assert!(with_image(&edit(Vec::new()), &info).is_none());
    // Never stored: the edit itself is untouched.
    assert!(
        NoiseModel::from_fields(&e.module(ModuleKind::DENOISE, 0).unwrap().params.to_fields())
            .is_none()
    );
}

#[test]
fn preview_matches_the_reduced_full_size_render() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (384, 256);
    let truth = clean(w, h);
    let model = iso_levels()[2].1;
    let img = noisy(&truth, &model, 21);
    let fields = dn_fields(0.6, 0.6, &model);
    let full = gpu_op(
        &mut en,
        &img,
        w,
        h,
        module(ModuleKind::DENOISE, fields),
        1.0,
    );
    let half = |v: &[f32]| -> Vec<f32> {
        let mut o = Vec::with_capacity(v.len() / 4);
        for y in 0..h / 2 {
            for x in 0..w / 2 {
                for k in 0..4 {
                    let at =
                        |dx: u32, dy: u32| v[(((2 * y + dy) * w + 2 * x + dx) * 4 + k) as usize];
                    o.push(0.25 * (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)));
                }
            }
        }
        o
    };
    let (hw, hh) = (w / 2, h / 2);
    // The preview source: the noisy image reduced, with its noise reduced 4x.
    let small = half(&img);
    let pm = module(
        ModuleKind::DENOISE,
        dn_fields(0.6, 0.6, &model.scaled(0.25)),
    );
    let preview = gpu_op(&mut en, &small, hw, hh, pm, 0.5);
    let reduced = half(&full);
    let (p_pre, p_noisy) = (psnr(&preview, &reduced), psnr(&small, &reduced));
    let p_truth = (psnr(&preview, &half(&truth)), psnr(&reduced, &half(&truth)));
    eprintln!(
        "preview vs reduced full: {p_pre:.2} dB (noisy preview source {p_noisy:.2} dB); \
         vs truth: preview {:.2}, reduced full {:.2}",
        p_truth.0, p_truth.1
    );
    assert!(p_pre > 40.0 && p_pre > p_noisy + 3.0);
}

// ---- Sharpen -----------------------------------------------------------------------

/// A grey step from `lo` to `hi` at x = w/2, plus a flat noisy strip at the bottom.
fn step_image(w: u32, h: u32, lo: f32, hi: f32, noise: f32) -> Vec<f32> {
    let mut r = Rng(5);
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let mut g = if x < w / 2 { lo } else { hi };
            if y >= h * 3 / 4 {
                g = 0.2 + noise * r.normal() as f32;
            }
            v.extend_from_slice(&[g, g, g, 1.0]);
        }
    }
    v
}

#[test]
fn sharpen_makes_no_halos_on_step_edges() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (96, 64);
    let (lo, hi) = (0.1f32, 0.5f32);
    let img = step_image(w, h, lo, hi, 0.01);
    let fields = |detail: f32, masking: f32| {
        Fields::new()
            .with("amount", Value::Float(1.5))
            .with("radius", Value::Float(1.5))
            .with("detail", Value::Float(detail))
            .with("masking", Value::Float(masking))
    };
    let edge_rows = |v: &[f32]| -> (f32, f32) {
        let (mut mn, mut mx) = (f32::MAX, f32::MIN);
        for y in 0..h / 2 {
            for x in 0..w {
                let g = v[((y * w + x) * 4 + 1) as usize];
                mn = mn.min(g);
                mx = mx.max(g);
            }
        }
        (mn, mx)
    };
    for (detail, masking) in [(0.0, 0.0), (0.0, 0.6), (0.25, 0.6)] {
        let m = module(ModuleKind::SHARPEN, fields(detail, masking));
        let gpu = gpu_op(&mut en, &img, w, h, m.clone(), 1.0);
        let s = Sharpen::from_fields(&m.params.to_fields());
        let full = Roi::full(w, h);
        let cref = cpu::sharpen(&Img { w, h, px: &img }, &s, full, full, 1.0);
        let (worst, over) = diff(&gpu, &cref, 1e-4);
        let (mn, mx) = edge_rows(&gpu);
        let over_pct = 100.0 * (mx - hi).max(lo - mn).max(0.0) / (hi - lo);
        eprintln!(
            "detail {detail} masking {masking}: overshoot {over_pct:.2} % of the step; GPU vs \
             CPU worst {worst:.2e}"
        );
        assert_eq!(over, 0);
        assert!(over_pct <= 100.0 * 0.2 * detail + 0.01, "halo {over_pct} %");
        // It did sharpen: the edge pixels moved apart.
        let at = |x: u32| gpu[((10 * w + x) * 4 + 1) as usize];
        assert!(at(w / 2) - at(w / 2 - 1) > (hi - lo) * 1.0 - 1e-4);
    }
    // A plain unsharp mask would ring: the clamp is doing the work.
    let s = Sharpen {
        amount: 1.5,
        radius: 1.5,
        detail: 1.0,
        masking: 0.0,
    };
    let full = Roi::full(w, h);
    let loose = cpu::sharpen(&Img { w, h, px: &img }, &s, full, full, 1.0);
    let (_, mx) = edge_rows(&loose);
    assert!(mx - hi > 0.04 * (hi - lo));
    // Masking leaves the flat noisy strip alone.
    let strip = |v: &[f32]| {
        let mut d = 0.0;
        for y in h * 3 / 4 + 3..h - 3 {
            for x in 3..w - 3 {
                let i = ((y * w + x) * 4 + 1) as usize;
                d += (v[i] - img[i]).abs();
            }
        }
        d
    };
    let masked = gpu_op(
        &mut en,
        &img,
        w,
        h,
        module(ModuleKind::SHARPEN, fields(0.25, 1.0)),
        1.0,
    );
    let unmasked = gpu_op(
        &mut en,
        &img,
        w,
        h,
        module(ModuleKind::SHARPEN, fields(0.25, 0.0)),
        1.0,
    );
    assert!(strip(&masked) < 0.25 * strip(&unmasked));
}

// ---- Capture sharpening ------------------------------------------------------------

/// Bars and a step, blurred by a Gaussian of `sigma`: (sharp, blurred).
fn blurred_bars(w: u32, h: u32, sigma: f32) -> (Vec<f32>, Vec<f32>) {
    let sharp: Vec<f32> = (0..h)
        .flat_map(|y| {
            (0..w).flat_map(move |x| {
                let g = if x < w / 3 {
                    if (x / 3) % 2 == 0 { 0.6 } else { 0.15 }
                } else if x < 2 * w / 3 {
                    0.1
                } else {
                    0.5
                } + if y % 16 == 8 { 0.2 } else { 0.0 };
                [g, g * 0.9, g * 0.8, 1.0]
            })
        })
        .collect();
    let r = (3.0 * sigma).ceil() as i32;
    let taps = super::kernels::gauss_taps(sigma, r as u32);
    let blur = |v: &[f32], horiz: bool| -> Vec<f32> {
        let mut o = vec![0f32; v.len()];
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                for k in 0..4 {
                    let mut s = 0.0;
                    for t in -r..=r {
                        let (sx, sy) = if horiz {
                            ((x + t).clamp(0, w as i32 - 1), y)
                        } else {
                            (x, (y + t).clamp(0, h as i32 - 1))
                        };
                        s += taps[t.unsigned_abs() as usize]
                            * v[((sy * w as i32 + sx) * 4 + k) as usize];
                    }
                    o[((y * w as i32 + x) * 4 + k) as usize] = if k == 3 { 1.0 } else { s };
                }
            }
        }
        o
    };
    let b = blur(&blur(&sharp, true), false);
    (sharp, b)
}

fn rms(a: &[f32], b: &[f32], w: u32, h: u32, border: u32) -> f64 {
    let mut s = 0.0;
    let mut n = 0.0;
    for y in border..h - border {
        for x in border..w - border {
            let i = ((y * w + x) * 4 + 1) as usize;
            s += f64::from(a[i] - b[i]).powi(2);
            n += 1.0;
        }
    }
    (s / n).sqrt()
}

#[test]
fn richardson_lucy_converges_without_ringing() {
    let (w, h) = (120, 64);
    let sigma = 1.2;
    let (sharp, blurred) = blurred_bars(w, h, sigma);
    let c = Capture {
        amount: 1.0,
        radius: sigma,
        auto_sigma: 0.7,
    };
    let full = Roi::full(w, h);
    let img = Img { w, h, px: &blurred };
    // Residual |P ⊛ u − d| and error to the sharp image after k iterations.
    let mut last = f64::MAX;
    let mut errs = Vec::new();
    for k in [1u32, 2, 5, 10, 20] {
        let prog = capture::program_iterations(&c, full, 1.0, k);
        let s = cpu::run_program(&prog, &img, full);
        let n = (w * h) as usize;
        let u: Vec<f32> = s[n..2 * n].to_vec();
        let (_, r, _) = capture::plan(&c, 1.0);
        let taps = super::kernels::gauss_taps(sigma, r);
        let conv = |v: &[f32], horiz: bool| -> Vec<f32> {
            let mut o = vec![0f32; n];
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let mut acc = 0.0;
                    for t in -(r as i32)..=r as i32 {
                        let (sx, sy) = if horiz {
                            ((x + t).clamp(0, w as i32 - 1), y)
                        } else {
                            (x, (y + t).clamp(0, h as i32 - 1))
                        };
                        acc += taps[t.unsigned_abs() as usize] * v[(sy * w as i32 + sx) as usize];
                    }
                    o[(y * w as i32 + x) as usize] = acc;
                }
            }
            o
        };
        let pu = conv(&conv(&u, true), false);
        let d = &s[..n];
        let residual = (pu
            .iter()
            .zip(d)
            .map(|(p, q)| f64::from(p - q).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        // Ringing: the estimate never leaves the observed local range by more than the margin.
        let mut ring = 0f32;
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for t in -(r as i32)..=r as i32 {
                    for q in -(r as i32)..=r as i32 {
                        let sx = (x as i32 + t).clamp(0, w as i32 - 1) as u32;
                        let sy = (y as i32 + q).clamp(0, h as i32 - 1) as u32;
                        let v = d[(sy * w + sx) as usize];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
                let out = (u[i] - hi).max(lo - u[i]).max(0.0);
                if hi > lo {
                    ring = ring.max(out / (hi - lo));
                }
            }
        }
        let developed = cpu::capture(&img, &c, full, full, 1.0);
        let e = if k == capture::ITERATIONS {
            rms(&developed, &sharp, w, h, 8)
        } else {
            0.0
        };
        eprintln!(
            "RL k {k}: residual {residual:.5}, ringing {:.2} % of the local range",
            100.0 * ring
        );
        assert!(residual <= last * 1.0001, "k {k}: {residual} after {last}");
        assert!(ring <= capture::MARGIN + 1e-5, "k {k}: rings {ring}");
        last = residual;
        if e > 0.0 {
            errs.push(e);
        }
    }
    let before = rms(&blurred, &sharp, w, h, 8);
    eprintln!(
        "RMS error to the sharp image: blurred {before:.4}, deconvolved {:.4}",
        errs[0]
    );
    assert!(errs[0] < 0.8 * before);
}

#[test]
fn capture_sharpening_gpu_matches_cpu() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (120, 64);
    let (_, blurred) = blurred_bars(w, h, 1.0);
    let f = Fields::new()
        .with("amount", Value::Float(0.8))
        .with("radius", Value::Float(1.0));
    let m = module(ModuleKind::CAPTURE_SHARPENING, f);
    let gpu = gpu_op(&mut en, &blurred, w, h, m.clone(), 1.0);
    let c = Capture::from_fields(&m.params.to_fields());
    let full = Roi::full(w, h);
    let cref = cpu::capture(&Img { w, h, px: &blurred }, &c, full, full, 1.0);
    let (worst, over) = diff(&gpu, &cref, 1e-4);
    eprintln!("capture sharpening GPU vs CPU worst {worst:.2e}");
    assert_eq!(over, 0);
    // Too small a PSF at a reduced view does nothing.
    let small = gpu_op(&mut en, &blurred, w, h, m, 0.1);
    assert!(diff(&small, &blurred, 1e-6).1 == 0);
}

// ---- Engine ------------------------------------------------------------------------

fn detail_edit(model: &NoiseModel) -> Edit {
    edit(vec![
        module(ModuleKind::DENOISE, dn_fields(0.5, 0.5, model)),
        module(
            ModuleKind::CAPTURE_SHARPENING,
            Fields::new().with("amount", Value::Float(0.6)),
        ),
        module(
            ModuleKind::SHARPEN,
            Fields::new()
                .with("amount", Value::Float(0.8))
                .with("masking", Value::Float(0.3)),
        ),
    ])
}

#[test]
fn tiles_equal_the_whole_image() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (300, 200);
    let model = iso_levels()[1].1;
    let img = noisy(&clean(w, h), &model, 4);
    let src = en.upload(w, h, &img).unwrap();
    let g = Graph::build(&detail_edit(&model), OpRegistry::builtin(), None).unwrap();
    assert_eq!(g.nodes.len(), 3);
    for scale in [1.0f32, 0.5] {
        let whole_px = render(&mut en, &g, &src, &whole(w, h, scale));
        let tiled = Request {
            tiling: Tiling::Side(64),
            ..whole(w, h, scale)
        };
        let r = en.render(&g, &src, &tiled).unwrap();
        assert!(r.stats.tiles > 1);
        let px = en.read(&r).unwrap();
        en.recycle(r);
        let (worst, _) = diff(&px, &whole_px, 0.0);
        eprintln!("scale {scale}: tiled vs whole worst {worst:.2e}");
        assert!(worst <= 1e-6, "scale {scale}: {worst}");
    }
}

#[test]
fn sliders_make_no_pipelines() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (160, 120);
    let model = iso_levels()[1].1;
    let img = noisy(&clean(w, h), &model, 8);
    let src = en.upload(w, h, &img).unwrap();
    let mut e = detail_edit(&model);
    let req = Request {
        precision: Precision::F16,
        scale: 0.5,
        ..Request::new(Roi::full(w, h))
    };
    let first = Graph::build(&e, OpRegistry::builtin(), Some((ModuleKind::SHARPEN, 0))).unwrap();
    let r = en.render(&first, &src, &req).unwrap();
    en.recycle(r);
    for (kind, name, v) in [
        (ModuleKind::DENOISE, "luminance", 0.7),
        (ModuleKind::DENOISE, "colour", 0.2),
        (ModuleKind::DENOISE, "detail", 0.8),
        (ModuleKind::CAPTURE_SHARPENING, "amount", 0.3),
        (ModuleKind::SHARPEN, "amount", 1.2),
        (ModuleKind::SHARPEN, "masking", 0.7),
        (ModuleKind::SHARPEN, "detail", 0.6),
    ] {
        let m = e.module_mut(kind, 0).unwrap();
        let mut f = m.params.to_fields();
        f.set(name, Value::Float(v));
        m.params.set_fields(f);
        let g = Graph::build(&e, OpRegistry::builtin(), Some((kind, 0))).unwrap();
        // Twice: the first may meet a new focus split; the second must create nothing.
        for pass in 0..2 {
            let r = en.render(&g, &src, &req).unwrap();
            if pass == 1 {
                assert_eq!(r.stats.pipelines_created, 0, "{name}");
            }
            en.recycle(r);
        }
    }
    // Moving a value with the focus fixed. (Each new value's output is a new heavy-node
    // checkpoint, so the nodes after it bind new buffers: bind groups, not pipelines.)
    for v in [0.3f32, 0.45, 0.6] {
        let m = e.module_mut(ModuleKind::DENOISE, 0).unwrap();
        let mut f = m.params.to_fields();
        f.set("luminance", Value::Float(v));
        m.params.set_fields(f);
        let g = Graph::build(&e, OpRegistry::builtin(), Some((ModuleKind::DENOISE, 0))).unwrap();
        let r = en.render(&g, &src, &req).unwrap();
        assert_eq!(r.stats.pipelines_created, 0, "luminance {v}");
        eprintln!(
            "luminance {v}: {} bind groups, {} buffers created",
            r.stats.bind_groups_created, r.stats.buffers_created
        );
        en.recycle(r);
    }
}

/// §3.9 budgets for the heavy detail ops at fit view on a 2560×1600 window and at full
/// size (GPU time from timestamps). Run on an idle machine:
/// `cargo test -p specular-develop detail_timings -- --ignored --nocapture`.
#[test]
#[ignore = "timings; run on an idle machine"]
fn detail_timings() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    if !en.has_timestamps() {
        eprintln!("no timestamps");
        return;
    }
    let model = iso_levels()[1].1;
    for (label, w, h, scale, prec) in [
        ("fit 2560x1600", 2560u32, 1600u32, 0.43f32, Precision::F16),
        (
            "full 4000x3000 (12 MP band)",
            4000,
            3000,
            1.0,
            Precision::F32,
        ),
    ] {
        let img = noisy(&clean(w, h), &model.scaled(scale * scale), 3);
        let src = en.upload(w, h, &img).unwrap();
        for (name, e) in [
            (
                "denoise wavelets",
                edit(vec![module(ModuleKind::DENOISE, {
                    let mut f = dn_fields(0.5, 0.5, &model.scaled(scale * scale));
                    f.set("nlm", Value::Bool(false));
                    f
                })]),
            ),
            (
                "denoise wavelets + NLM",
                edit(vec![module(
                    ModuleKind::DENOISE,
                    dn_fields(0.5, 0.5, &model.scaled(scale * scale)),
                )]),
            ),
            (
                "capture sharpening",
                edit(vec![module(
                    ModuleKind::CAPTURE_SHARPENING,
                    Fields::new().with("amount", Value::Float(0.6)),
                )]),
            ),
            (
                "sharpen",
                edit(vec![module(
                    ModuleKind::SHARPEN,
                    Fields::new().with("amount", Value::Float(0.8)),
                )]),
            ),
            ("all three", detail_edit(&model.scaled(scale * scale))),
        ] {
            let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
            let req = Request {
                precision: prec,
                scale,
                profile: true,
                checkpoints: false,
                ..Request::new(Roi::full(w, h))
            };
            let mut ms = Vec::new();
            for _ in 0..6 {
                let t = std::time::Instant::now();
                let r = en.render(&g, &src, &req).unwrap();
                en.wait().unwrap();
                let wall = t.elapsed().as_secs_f64() * 1e3;
                ms.push((r.stats.gpu_total_ms().unwrap_or(0.0), wall, r.stats.tiles));
                en.recycle(r);
            }
            ms.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (gpu, wall, tiles) = ms[ms.len() / 2];
            eprintln!("{label} {name}: GPU p50 {gpu:.1} ms, wall {wall:.1} ms, {tiles} tiles");
        }
    }
}

/// The noise model carried through the raw front end (white balance, demosaic or binning,
/// matrix) predicts the developed buffer's noise.
#[test]
fn front_end_noise_model_matches_measured() {
    use crate::ops::raw::{FrontEnd, Options, Quality, noise_model};
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (512u32, 384u32);
    let (a, b) = ([4e-4f64, 2e-4, 3e-4], [2e-6f64, 1e-6, 1.5e-6]);
    let mut s = bayer_sensor(w, h, a, b, 17);
    // A flat grey field: refill the samples at one level per colour.
    let mut r = Rng(23);
    let (black, white) = (512.0f64, 15000.0f64);
    for y in 0..h {
        for x in 0..w {
            let col = usize::from(s.meta.layout.color(x, y));
            let v = 0.25 / f64::from(s.meta.wb[col]);
            let n = r.pg(v, a[col], b[col]);
            s.samples[(y * w + x) as usize] = (black + n * (white - black)).round() as u16;
        }
    }
    let raw = RawNoise {
        a,
        b,
        source: NoiseSource::Profile,
    };
    let g = fe.upload(s.clone()).unwrap();
    let mut worst = 0f64;
    for q in [Quality::Half, Quality::Fast, Quality::Full] {
        let o = Options {
            precision: Precision::F32,
            ..Options::new(q)
        };
        let out = fe.run(&g, &o).unwrap();
        let px = fe.read(&out).unwrap();
        let (ow, oh) = (out.width, out.height);
        let model = noise_model(&s, &raw, &o, 1.0);
        let mut line = format!("{q:?}:");
        for c in 0..3 {
            let vals: Vec<f64> = (8..oh - 8)
                .flat_map(|y| (8..ow - 8).map(move |x| (x, y)))
                .map(|(x, y)| f64::from(px[((y * ow + x) * 4) as usize + c]))
                .collect();
            let m = vals.iter().sum::<f64>() / vals.len() as f64;
            let var = vals.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / vals.len() as f64;
            let pred = f64::from(model.a[c]) * m + f64::from(model.b[c]);
            line += &format!(" c{c} measured/model {:.3}", var / pred);
            worst = worst.max((var / pred).ln().abs());
        }
        eprintln!("{line}");
    }
    assert!(
        worst < 0.1f64.ln_1p(),
        "model off by {:.1} %",
        100.0 * worst.exp_m1()
    );
}

/// The noise fit as a plain loop (the kernel reads each block once and runs every lattice in one
/// parallel pass): the same blocks in the same order, so the same model bit for bit.
fn plain_estimate(s: &SensorData) -> Option<RawNoise> {
    use super::noise::{BLOCK, block_stats, fit_blocks};
    let (m, c) = (&s.meta, s.crop);
    let mut lattices = Vec::new();
    match &m.layout {
        Layout::Bayer(_) => {
            for oy in 0..2 {
                for ox in 0..2 {
                    lattices.push((ox, oy, 2, m.layout.color(c.x + ox, c.y + oy)));
                }
            }
        }
        Layout::Pattern(p) => {
            for oy in 0..p.height {
                for ox in 0..p.width {
                    let col = m.layout.color(c.x + ox, c.y + oy);
                    lattices.push((ox, oy, p.width.max(p.height), col));
                }
            }
        }
        _ => unreachable!("CFA sensors only"),
    }
    let value = |x: u32, y: u32| -> f64 {
        let (sx, sy) = (c.x + x, c.y + y);
        let b = f64::from(m.black.at(sx, sy, 0));
        (f64::from(s.sample(sx, sy, 0)) - b) / (f64::from(m.white) - b).max(1.0)
    };
    let clip = {
        let b = f64::from(m.black.max());
        0.9 * (f64::from(m.clip_white) - b) / (f64::from(m.white) - b).max(1.0)
    };
    let mut per: [Vec<(f64, f64)>; 3] = Default::default();
    for (ox, oy, step, col) in lattices {
        let lw = ((c.w - ox).div_ceil(step)) as usize;
        let lh = ((c.h - oy).div_ceil(step)) as usize;
        let (nx, ny) = (lw / BLOCK, lh / BLOCK);
        let stride = ((nx * ny) as f64 / 8000.0).sqrt().ceil().max(1.0) as usize;
        for by in (0..ny).step_by(stride) {
            for bx in (0..nx).step_by(stride) {
                let at = |i: usize, j: usize| {
                    let x = ox + (bx * BLOCK + i) as u32 * step;
                    let y = oy + (by * BLOCK + j) as u32 * step;
                    value(x, y)
                };
                let mut top = f64::MIN;
                for j in 0..BLOCK {
                    for i in 0..BLOCK {
                        top = top.max(at(i, j));
                    }
                }
                if top < clip {
                    per[usize::from(col.min(2))].push(block_stats(at));
                }
            }
        }
    }
    let mut a = [0.0; 3];
    let mut b = [0.0; 3];
    for k in 0..3 {
        let (ak, bk) = fit_blocks(&mut per[k])?;
        a[k] = ak;
        b[k] = bk.max(0.0);
    }
    Some(RawNoise {
        a,
        b,
        source: NoiseSource::Estimated,
    })
}

#[test]
fn the_noise_fit_equals_the_plain_loop() {
    let bayer = bayer_sensor(1536, 1024, [8.0e-4, 4.0e-4, 6.0e-4], [4e-6, 2e-6, 3e-6], 5);
    let mut xtrans = bayer_sensor(1536, 1026, [8.0e-4, 4.0e-4, 6.0e-4], [4e-6, 2e-6, 3e-6], 9);
    xtrans.meta.layout = Layout::Pattern(
        specular_raw::Cfa::new(
            6,
            6,
            vec![
                1, 1, 2, 1, 1, 0, 2, 0, 1, 0, 2, 1, 1, 1, 2, 1, 1, 0, 1, 1, 0, 1, 1, 2, 0, 2, 1, 2,
                0, 1, 1, 1, 0, 1, 1, 2,
            ],
        )
        .expect("a pattern"),
    );
    for s in [&bayer, &xtrans] {
        let fast = RawNoise::estimate(s).expect("a fit");
        let plain = plain_estimate(s).expect("a fit");
        assert_eq!(fast.a.map(f64::to_bits), plain.a.map(f64::to_bits));
        assert_eq!(fast.b.map(f64::to_bits), plain.b.map(f64::to_bits));
    }
}
