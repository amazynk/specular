//! Diffuse or sharpen and surface blur. GPU = CPU reference for every preset and the
//! extremes, no blow-up over iterations (f16 and f32), tiles = whole image, what each
//! preset does (sharper edges without overshoot, bloom, filled highlights, less noise with
//! edges kept), fit view against a downscaled export, drafts, masks and no pipelines per
//! slider move.

use super::cpu_diffuse;
use super::diffuse::{self, ANISOTROPY_FIELDS, Diffuse, LONG_FIELD, Preset, SPEED_FIELDS};
use super::surface::{self, SurfaceBlur};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::tile::Tiling;
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::mask::{Geometry, MaskCache, MaskEnv};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

const GIB: u64 = 1 << 30;

pub(super) fn engine() -> Option<Engine> {
    let d = dev()?;
    Some(Engine::new(&d.device, &d.queue, Budget::fixed(2 * GIB)))
}

/// xorshift64* noise in −0.5 … 0.5.
struct Rng(u64);

impl Rng {
    fn u(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((v >> 40) as f32 / (1u64 << 24) as f32) - 0.5
    }
}

/// A scene with edges, a disc, texture, a gradient, a clipped highlight and some noise.
pub(super) fn scene(w: u32, h: u32, noise: f32, seed: u64) -> Vec<f32> {
    let mut r = Rng(seed | 1);
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut c = if u < 0.5 {
                [0.08, 0.1, 0.12]
            } else {
                [0.45, 0.4, 0.3]
            };
            if t > 0.6 {
                let g = 0.05 + 0.5 * u;
                c = [g, g * 0.8, g * 0.6];
            }
            if t < 0.3 && u > 0.2 && u < 0.45 {
                // Relative to the image, so a reduced copy holds the same structures.
                let s = 0.5 + 0.5 * (u * 160.0).sin() * (t * 70.0).cos();
                c = [0.1 + 0.3 * s, 0.12 + 0.25 * s, 0.1 + 0.2 * s];
            }
            let (dx, dy) = (u - 0.75, t - 0.3);
            if dx * dx + dy * dy < 0.01 {
                c = [1.0, 1.0, 1.0];
            }
            if dx * dx + dy * dy < 0.004 {
                c = [1.3, 1.3, 1.3];
            }
            let n = |r: &mut Rng, c: f32| c + noise * r.u() * (c.max(0.0).sqrt() + 0.05);
            v.extend_from_slice(&[n(&mut r, c[0]), n(&mut r, c[1]), n(&mut r, c[2]), 1.0]);
        }
    }
    v
}

fn fields_of(d: &Diffuse) -> Fields {
    d.to_fields()
}

pub(super) fn module(kind: ModuleKind, f: Fields) -> ModuleInstance {
    let mut m = ModuleInstance::new(Registry::builtin(), kind, 0);
    let mut all = m.params.to_fields();
    all.overlay(&f);
    m.params = Params::from_fields(kind, all);
    m
}

pub(super) fn edit(mods: Vec<ModuleInstance>) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    for m in mods {
        e.insert(Registry::builtin(), m);
    }
    e
}

pub(super) fn whole(w: u32, h: u32, prec: Precision) -> Request {
    Request {
        precision: prec,
        checkpoints: false,
        ..Request::new(Roi::full(w, h))
    }
}

fn gpu(en: &mut Engine, img: &[f32], w: u32, h: u32, e: &Edit, req: &Request) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let g = Graph::build(e, OpRegistry::builtin(), None).expect("graph");
    let r = en.render(&g, &src, req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

pub(super) fn with_long(mut f: Fields, long: u32) -> Fields {
    f.set(LONG_FIELD, Value::Float(long as f32));
    f
}

pub(super) fn diff(a: &[f32], b: &[f32]) -> (f32, f64) {
    assert_eq!(a.len(), b.len());
    let mut worst = 0f32;
    let mut sum = 0f64;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        for k in 0..4 {
            let d = (p[k] - q[k]).abs();
            assert!(d.is_finite(), "non-finite: {p:?} vs {q:?}");
            worst = worst.max(d);
            sum += f64::from(d);
        }
    }
    (worst, sum / a.len() as f64)
}

/// The extremes the stability test runs besides the presets.
pub(super) fn extremes() -> Vec<(&'static str, Diffuse)> {
    let base = Diffuse {
        iterations: diffuse::MAX_ITERATIONS,
        radius: 0.005,
        span: 0.005,
        edges: 1.0,
        threshold: 0.0,
        ..Diffuse::default()
    };
    vec![
        (
            "all sharpen, along edges",
            Diffuse {
                speed: [-1.0; 4],
                anisotropy: [1.0; 4],
                ..base.clone()
            },
        ),
        (
            "all sharpen, across edges",
            Diffuse {
                speed: [-1.0; 4],
                anisotropy: [-1.0; 4],
                edges: 0.0,
                ..base.clone()
            },
        ),
        (
            "all diffuse",
            Diffuse {
                speed: [1.0; 4],
                anisotropy: [0.0; 4],
                ..base.clone()
            },
        ),
        (
            "mixed, fine",
            Diffuse {
                speed: [1.0, -1.0, 1.0, -1.0],
                anisotropy: [1.0, -1.0, -1.0, 1.0],
                radius: 0.0,
                span: 0.0005,
                ..base.clone()
            },
        ),
        (
            "fine sharpen, coarse window",
            Diffuse {
                speed: [-1.0, -1.0, -1.0, -1.0],
                radius: 0.01,
                span: 0.01,
                ..base
            },
        ),
    ]
}

pub(super) fn presets() -> Vec<(String, Diffuse)> {
    Preset::ALL
        .iter()
        .map(|p| (format!("{p:?}"), p.params()))
        .collect()
}

#[test]
fn diffuse_gpu_matches_cpu() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (96, 64);
    let img = scene(w, h, 0.05, 3);
    let mut cases = presets();
    cases.extend(extremes().into_iter().map(|(n, d)| (n.to_string(), d)));
    // A 4000-pixel long side could run in stripes (bit for bit the same but for
    // the last column and row at even sides): off, the whole-picture program as the CPU's.
    let half = &super::diffuse::NO_STRIPES;
    half.store(true, std::sync::atomic::Ordering::Relaxed);
    for (name, mut d) in cases {
        d.iterations = d.iterations.min(6);
        d.long = 4000.0;
        let f = with_long(fields_of(&d), 4000);
        let d = Diffuse::from_fields(&f);
        let e = edit(vec![module(ModuleKind::DIFFUSE, f)]);
        let g = gpu(&mut en, &img, w, h, &e, &whole(w, h, Precision::F32));
        let c = cpu_diffuse::diffuse(&img, w, h, &d);
        let (worst, mean) = diff(&g, &c);
        eprintln!("{name}: GPU vs CPU worst {worst:.2e} mean {mean:.2e}");
        assert!(worst <= 2e-4, "{name}: {worst}");
    }
    half.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn surface_gpu_matches_cpu() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (120, 80);
    let img = scene(w, h, 0.08, 5);
    for (radius, edges) in [(0.005f32, 0.1f32), (0.015, 0.04), (0.03, 0.12), (0.05, 0.5)] {
        let s = SurfaceBlur {
            amount: 1.0,
            radius,
            edges,
            long: 400.0,
        };
        let f = with_long(
            Fields::new()
                .with("amount", Value::Float(1.0))
                .with("radius", Value::Float(radius))
                .with("edges", Value::Float(edges)),
            400,
        );
        let e = edit(vec![module(ModuleKind::SURFACE_BLUR, f)]);
        let g = gpu(&mut en, &img, w, h, &e, &whole(w, h, Precision::F32));
        let c = cpu_diffuse::surface_blur(&img, w, h, &s);
        let (worst, mean) = diff(&g, &c);
        eprintln!(
            "surface {radius} ({:?}), edges {edges}: worst {worst:.2e} mean {mean:.2e}",
            surface::plan(&s)
        );
        assert!(worst <= 1e-4, "{radius}/{edges}: {worst}");
    }
}

/// Variance about the input of each channel, over the image.
fn spread(px: &[f32]) -> f64 {
    let n = px.len() / 4;
    let mut v = 0.0;
    for k in 0..3 {
        let m: f64 = px.chunks_exact(4).map(|p| f64::from(p[k])).sum::<f64>() / n as f64;
        v += px
            .chunks_exact(4)
            .map(|p| (f64::from(p[k]) - m).powi(2))
            .sum::<f64>()
            / n as f64;
    }
    v
}

#[test]
fn diffusion_stays_bounded() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (128, 96);
    let img = scene(w, h, 0.1, 9);
    let (lo, hi) = img
        .chunks_exact(4)
        .flat_map(|p| p[..3].to_vec())
        .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let v0 = spread(&img);
    let mut cases = presets();
    cases.extend(extremes().into_iter().map(|(n, d)| (n.to_string(), d)));
    for (i, (name, d)) in cases.into_iter().enumerate() {
        // The presets level off; the extremes are only held within their bound.
        let preset = i < Preset::ALL.len();
        for prec in [Precision::F32, Precision::F16] {
            let mut seen = Vec::new();
            for it in [1, 5, 10, 20] {
                let d = Diffuse {
                    iterations: it,
                    ..d.clone()
                };
                let f = with_long(fields_of(&d), 1000);
                let e = edit(vec![module(ModuleKind::DIFFUSE, f)]);
                let out = gpu(&mut en, &img, w, h, &e, &whole(w, h, prec));
                let v = spread(&out);
                let (olo, ohi) = out
                    .chunks_exact(4)
                    .flat_map(|p| p[..3].to_vec())
                    .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
                assert!(out.iter().all(|v| v.is_finite()), "{name}");
                let span = hi - lo;
                let bound = (2.0 * diffuse::MARGIN).exp() * span;
                assert!(
                    olo >= lo - bound && ohi <= hi + bound,
                    "{name} {prec:?} {it}: {olo}..{ohi} (in {lo}..{hi})"
                );
                // Sharpening raises the variance, but it levels off: no growth without end.
                assert!(
                    !preset || v <= 4.0 * v0,
                    "{name} {prec:?} {it}: variance {v} vs {v0}"
                );
                if it == 20 && preset {
                    let v10: f64 = seen[2];
                    assert!(
                        v <= 1.5 * v10,
                        "{name} {prec:?}: {v} after 20 vs {v10} after 10"
                    );
                }
                seen.push(v);
            }
            let r: Vec<String> = seen.iter().map(|v| format!("{:.2}", v / v0)).collect();
            eprintln!(
                "{name} {prec:?}: variance / input after 1, 5, 10, 20: {}",
                r.join(" ")
            );
        }
    }
}

pub(super) fn graph_at(e: &Edit, w: u32, h: u32, focus: Option<(ModuleKind, u16)>) -> Graph {
    let mut cache = MaskCache::default();
    let mut env = MaskEnv {
        geometry: Geometry::Frame {
            w: f64::from(w),
            h: f64::from(h),
        },
        image: (w, h),
        scale: 1.0,
        cache: &mut cache,
        sensor_bin: 1.0,
        overlay: None,
    };
    Graph::build_masked(e, OpRegistry::builtin(), focus, None, Some(&mut env)).expect("graph")
}

pub(super) fn render_g(
    en: &mut Engine,
    g: &Graph,
    img: &[f32],
    w: u32,
    h: u32,
    req: &Request,
) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let r = en.render(g, &src, req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

/// `e` after a slight exposure, so the op reads an intermediate picture at the render's
/// precision (as after the raw front end), not the uploaded f32 source.
pub(super) fn after_exposure(mut e: Edit) -> Edit {
    let f = Fields::new().with("exposure_ev", Value::Float(0.01));
    e.modules.insert(0, module(ModuleKind::EXPOSURE, f));
    e
}

pub(super) fn preset_edit(p: Preset) -> Edit {
    edit(vec![module(ModuleKind::DIFFUSE, p.params().to_fields())])
}

fn surface_edit(amount: f32, radius: f32, edges: f32) -> Edit {
    edit(vec![module(
        ModuleKind::SURFACE_BLUR,
        Fields::new()
            .with("amount", Value::Float(amount))
            .with("radius", Value::Float(radius))
            .with("edges", Value::Float(edges)),
    )])
}

#[test]
fn placement_sets_the_long_side_and_draft() {
    let e = preset_edit(Preset::Bloom);
    let g = graph_at(&e, 300, 200, None);
    let d = Diffuse::of(&g.nodes[0].params);
    assert_eq!(d.long, 300.0);
    assert!(!d.draft);
    let g = graph_at(&e, 3000, 2000, Some((ModuleKind::DIFFUSE, 0)));
    let d = Diffuse::of(&g.nodes[0].params);
    assert!(d.draft);
    let p = diffuse::plan(&d);
    assert_eq!(p.iterations, Preset::Bloom.params().iterations.div_ceil(2));
    assert!(p.half);
    // The stored fields never carry them.
    let f = e.module(ModuleKind::DIFFUSE, 0).unwrap().params.to_fields();
    assert!(f.get(LONG_FIELD).is_none());
    assert_eq!(Preset::matching(&f), Some(Preset::Bloom));
}

#[test]
fn tiles_equal_the_whole_image() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (300, 200);
    let img = scene(w, h, 0.05, 4);
    let mut cases: Vec<(String, Edit)> = Preset::ALL
        .iter()
        .map(|p| (format!("{p:?}"), preset_edit(*p)))
        .collect();
    cases.push(("surface direct".into(), surface_edit(1.0, 0.01, 0.1)));
    cases.push(("surface grid".into(), surface_edit(1.0, 0.04, 0.1)));
    for (name, e) in cases {
        let g = graph_at(&e, w, h, None);
        let whole_px = render_g(&mut en, &g, &img, w, h, &whole(w, h, Precision::F32));
        let tiled = Request {
            tiling: Tiling::Side(64),
            ..whole(w, h, Precision::F32)
        };
        let px = render_g(&mut en, &g, &img, w, h, &tiled);
        let (worst, _) = diff(&px, &whole_px);
        eprintln!("{name}: tiled vs whole worst {worst:.2e}");
        assert!(worst <= 1e-6, "{name}: {worst}");
    }
}

/// Luminance of a pixel.
fn yl(px: &[f32], w: u32, x: u32, y: u32) -> f32 {
    let i = ((y * w + x) * 4) as usize;
    0.2627 * px[i] + 0.678 * px[i + 1] + 0.0593 * px[i + 2]
}

/// A soft vertical step edge (dark left, bright right) of Gaussian blur `sigma`.
fn soft_edge(w: u32, h: u32, sigma: f32) -> Vec<f32> {
    let erf = |x: f32| {
        // Abramowitz–Stegun 7.1.26.
        let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
        let y = 1.0
            - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_7) * t - 0.284_496_74) * t
                + 0.254_829_6)
                * t
                * (-x * x).exp();
        if x < 0.0 { -y } else { y }
    };
    let mut v = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            let t = 0.5 + 0.5 * erf((x as f32 + 0.5 - w as f32 / 2.0) / (sigma * 2f32.sqrt()));
            let c = 0.1 + 0.4 * t;
            v.extend_from_slice(&[c, c, c, 1.0]);
        }
    }
    v
}

/// 10–90 % rise distance of a row's luminance, and the overshoot past the plateaus as a
/// share of the step.
fn edge_stats(px: &[f32], w: u32, y: u32) -> (f32, f32) {
    let row: Vec<f32> = (0..w).map(|x| yl(px, w, x, y)).collect();
    let (lo, hi) = (row[2], row[w as usize - 3]);
    let at = |f: f32| {
        let t = lo + f * (hi - lo);
        let i = row.iter().position(|v| *v >= t).unwrap_or(0);
        let (a, b) = (row[i.saturating_sub(1)], row[i]);
        i as f32 - 1.0 + if b > a { (t - a) / (b - a) } else { 1.0 }
    };
    let max = row.iter().copied().fold(f32::MIN, f32::max);
    let min = row.iter().copied().fold(f32::MAX, f32::min);
    let over = ((max - hi).max(0.0) + (lo - min).max(0.0)) / (hi - lo);
    (at(0.9) - at(0.1), over)
}

#[test]
fn presets_do_what_they_say() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let req = |w, h| whole(w, h, Precision::F32);
    // Sharpen and lens blur: a 1.5 px blurred edge (on a 3000 px long side) gets steeper
    // with little overshoot.
    let (w, h) = (64, 16);
    let edge = soft_edge(w, h, 1.5);
    let (r0, _) = edge_stats(&edge, w, 8);
    for p in [Preset::Sharpen, Preset::Deblur] {
        let mut f = p.params().to_fields();
        f.set(LONG_FIELD, Value::Float(3000.0));
        let e = edit(vec![module(ModuleKind::DIFFUSE, f)]);
        let out = gpu(&mut en, &edge, w, h, &e, &req(w, h));
        let (r, over) = edge_stats(&out, w, 8);
        eprintln!(
            "{p:?}: 10-90 rise {r0:.2} -> {r:.2} px, overshoot {:.1} %",
            100.0 * over
        );
        assert!(r < 0.85 * r0, "{p:?}: {r} vs {r0}");
        assert!(over < 0.12, "{p:?}: overshoot {over}");
    }
    // Bloom: light spreads into the dark next to the highlight.
    let (w, h) = (200, 140);
    let img = scene(w, h, 0.0, 1);
    let e = preset_edit(Preset::Bloom);
    let g = graph_at(&e, w, h, None);
    let out = render_g(&mut en, &g, &img, w, h, &req(w, h));
    // Just outside the disc (centre 150, 42; radius 0.1·200 = 20 px).
    let (bx, by) = (150 + 24, 42);
    let (b0, b1) = (yl(&img, w, bx, by), yl(&out, w, bx, by));
    eprintln!("bloom beside the highlight {b0:.3} -> {b1:.3}");
    assert!(b1 > b0 + 0.02);
    // Fill clipped highlights: a flat clipped patch inside texture takes on structure from
    // its surroundings, and nothing well below the threshold moves.
    let (w, h) = (96, 96);
    let mut img = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let v = 0.6 + 0.3 * ((x as f32) * 0.3).sin() * ((y as f32) * 0.2).cos();
            let (dx, dy) = (x as f32 - 48.0, y as f32 - 48.0);
            let c = if dx * dx + dy * dy < 100.0 { 1.0 } else { v };
            img.extend_from_slice(&[c, c * 0.95, c * 0.9, 1.0]);
        }
    }
    let mut f = Preset::InpaintHighlights.params().to_fields();
    f.set(LONG_FIELD, Value::Float(1000.0));
    let e = edit(vec![module(ModuleKind::DIFFUSE, f)]);
    let out = gpu(&mut en, &img, w, h, &e, &req(w, h));
    let centre = yl(&out, w, 48, 48);
    let below: f32 = (0..w * h)
        .filter(|i| img[(*i * 4) as usize] < 0.7)
        .map(|i| (out[(i * 4) as usize] - img[(i * 4) as usize]).abs())
        .fold(0.0, f32::max);
    eprintln!("inpaint: centre {centre:.3} (was 0.96), largest change below 0.7: {below:.2e}");
    assert!(centre < 0.95);
    assert!(below < 1e-6);
    // Smooth: noise falls in flat areas, the main edge stays.
    let (w, h) = (128, 64);
    let clean = soft_edge(w, h, 0.5);
    let mut r = Rng(77);
    let noisy: Vec<f32> = clean
        .chunks_exact(4)
        .flat_map(|p| {
            let n = 0.04 * r.u();
            [p[0] + n, p[1] + n, p[2] + n, 1.0]
        })
        .collect();
    let mut f = Preset::Smooth.params().to_fields();
    f.set(LONG_FIELD, Value::Float(4000.0));
    let e = edit(vec![module(ModuleKind::DIFFUSE, f)]);
    let out = gpu(&mut en, &noisy, w, h, &e, &req(w, h));
    let rms = |a: &[f32]| {
        let s: f64 = (8..h - 8)
            .flat_map(|y| (8..40).map(move |x| (x, y)))
            .map(|(x, y)| f64::from(yl(a, w, x, y) - yl(&clean, w, x, y)).powi(2))
            .sum();
        (s / f64::from(32 * (h - 16))).sqrt()
    };
    let (n0, n1) = (rms(&noisy), rms(&out));
    let (e0, _) = edge_stats(&clean, w, 32);
    let (e1, _) = edge_stats(&out, w, 32);
    eprintln!("smooth: noise {n0:.4} -> {n1:.4}; edge rise {e0:.2} -> {e1:.2} px");
    assert!(n1 < 0.6 * n0);
    assert!(e1 < e0 + 1.5);
}

#[test]
fn surface_blur_keeps_edges() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (128, 64);
    let clean = soft_edge(w, h, 0.5);
    let mut r = Rng(5);
    let noisy: Vec<f32> = clean
        .chunks_exact(4)
        .flat_map(|p| {
            let n = 0.03 * r.u();
            [p[0] + n, p[1] + n, p[2] + n, 1.0]
        })
        .collect();
    for radius in [0.02f32, 0.08] {
        let f = Fields::new()
            .with("amount", Value::Float(1.0))
            .with("radius", Value::Float(radius))
            .with("edges", Value::Float(0.08));
        let e = edit(vec![module(ModuleKind::SURFACE_BLUR, with_long(f, w))]);
        let out = gpu(&mut en, &noisy, w, h, &e, &whole(w, h, Precision::F32));
        let sd = |a: &[f32]| {
            let v: Vec<f32> = (8..h - 8).map(|y| yl(a, w, 20, y)).collect();
            let m = v.iter().sum::<f32>() / v.len() as f32;
            (v.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / v.len() as f32).sqrt()
        };
        let (e1, _) = edge_stats(&out, w, 32);
        eprintln!(
            "surface {radius}: noise {:.4} -> {:.4}, edge rise {e1:.2} px",
            sd(&noisy),
            sd(&out)
        );
        assert!(sd(&out) < 0.4 * sd(&noisy));
        assert!(e1 < 3.0, "edge kept: {e1}");
    }
}

/// ΔE2000 p95 between two renders (as display-referred, clipped).
pub(super) fn delta_e_p95(a: &[f32], b: &[f32]) -> f64 {
    use crate::ops::tone::tests::{de2000, lab};
    let c = |p: &[f32]| lab([p[0], p[1], p[2]].map(|v| v.clamp(0.0, 1.0)));
    let mut d: Vec<f64> = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .map(|(p, q)| de2000(c(p), c(q)))
        .collect();
    d.sort_by(f64::total_cmp);
    d[(d.len() as f64 * 0.95) as usize]
}

/// Box downscale by `k`.
pub(super) fn down(px: &[f32], w: u32, h: u32, k: u32) -> Vec<f32> {
    let (dw, dh) = (w / k, h / k);
    let mut v = vec![0f32; (dw * dh * 4) as usize];
    for y in 0..dh * k {
        for x in 0..dw * k {
            for c in 0..4 {
                v[(((y / k) * dw + x / k) * 4 + c) as usize] +=
                    px[((y * w + x) * 4 + c) as usize] / (k * k) as f32;
            }
        }
    }
    v
}

/// Improvement 14: a fit view (the graph on a quarter-size image, f16) against the
/// full-size render (f32) downscaled. Presets that miss ΔE p95 < 1 must be the ones whose
/// card says "only at 100 %".
#[test]
fn fit_view_matches_downscaled_export() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h, k) = (2000, 1300, 4);
    let full = scene(w, h, 0.0, 2);
    let small = down(&full, w, h, k);
    let (sw, sh) = (w / k, h / k);
    let mut cases: Vec<(String, Edit, bool)> = Preset::ALL
        .iter()
        .map(|p| (format!("{p:?}"), preset_edit(*p), p.scale_stable()))
        .collect();
    cases.push(("surface".into(), surface_edit(1.0, 0.02, 0.1), true));
    for (name, e, stable) in cases {
        let g = graph_at(&e, w, h, None);
        let big = render_g(&mut en, &g, &full, w, h, &whole(w, h, Precision::F32));
        let g = graph_at(&e, sw, sh, None);
        let view = render_g(&mut en, &g, &small, sw, sh, &whole(sw, sh, Precision::F16));
        let de = delta_e_p95(&view, &down(&big, w, h, k));
        let base = delta_e_p95(&small, &down(&big, w, h, k));
        eprintln!("{name}: fit view vs export ΔE p95 {de:.2} (effect itself {base:.2})");
        if stable {
            assert!(de < 1.0, "{name}: {de}");
        }
    }
}

#[test]
fn draft_is_close_to_refined() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (1280, 832);
    let img = scene(w, h, 0.02, 6);
    for p in Preset::ALL {
        let e = preset_edit(p);
        let req = whole(w, h, Precision::F16);
        let refined = render_g(&mut en, &graph_at(&e, w, h, None), &img, w, h, &req);
        let focus = Some((ModuleKind::DIFFUSE, 0));
        let draft = render_g(&mut en, &graph_at(&e, w, h, focus), &img, w, h, &req);
        let de = delta_e_p95(&draft, &refined);
        let effect = delta_e_p95(&img, &refined);
        eprintln!("{p:?}: draft vs refined ΔE p95 {de:.2} (effect {effect:.2})");
        assert!(de <= effect.max(1.0), "{p:?}");
    }
}

#[test]
fn masks_limit_both_ops() {
    use crate::model::{Blend, MaskExpr, MaskShape, Shape};
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (160, 100);
    let img = scene(w, h, 0.05, 8);
    let req = whole(w, h, Precision::F32);
    for kind in [ModuleKind::DIFFUSE, ModuleKind::SURFACE_BLUR] {
        let mut e = if kind == ModuleKind::DIFFUSE {
            preset_edit(Preset::Bloom)
        } else {
            surface_edit(1.0, 0.03, 0.2)
        };
        let full = render_g(&mut en, &graph_at(&e, w, h, None), &img, w, h, &req);
        e.masks = vec![MaskShape {
            id: 1,
            shape: Shape::Linear {
                start: [0.4, 0.5],
                end: [0.6, 0.5],
            },
        }];
        e.module_mut(kind, 0).unwrap().blend = Some(Blend {
            mask: MaskExpr::Shape(1),
            ..Blend::default()
        });
        let out = render_g(&mut en, &graph_at(&e, w, h, None), &img, w, h, &req);
        let side = |a: &[f32], b: &[f32], x0: u32, x1: u32| {
            let mut m = 0f32;
            for y in 0..h {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    for c in 0..3 {
                        m = m.max((a[i + c] - b[i + c]).abs());
                    }
                }
            }
            m
        };
        let (l_in, l_full) = (side(&out, &img, 0, 50), side(&out, &full, 0, 50));
        let (r_in, r_full) = (side(&out, &img, 110, w), side(&out, &full, 110, w));
        eprintln!(
            "{kind:?}: left vs input {l_in:.2e} / full {l_full:.2e}; right {r_in:.2e} / {r_full:.2e}"
        );
        let one = l_in < 1e-5 && r_full < 1e-5;
        let other = r_in < 1e-5 && l_full < 1e-5;
        assert!(one || other, "{kind:?}");
    }
}

#[test]
fn sliders_make_no_pipelines() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (160, 120);
    let img = scene(w, h, 0.05, 8);
    let src = en.upload(w, h, &img).unwrap();
    let mut e = edit(vec![
        module(ModuleKind::DIFFUSE, Preset::Sharpen.params().to_fields()),
        module(
            ModuleKind::SURFACE_BLUR,
            Fields::new()
                .with("amount", Value::Float(0.5))
                .with("radius", Value::Float(0.01)),
        ),
    ]);
    let req = Request::new(Roi::full(w, h));
    let r = en.render(&graph_at(&e, w, h, None), &src, &req).unwrap();
    en.recycle(r);
    let mut moves: Vec<(ModuleKind, &str, Value)> = vec![
        (ModuleKind::DIFFUSE, "iterations", Value::Int(7)),
        (ModuleKind::DIFFUSE, "radius", Value::Float(0.004)),
        (ModuleKind::DIFFUSE, "span", Value::Float(0.002)),
        (ModuleKind::DIFFUSE, "edges", Value::Float(0.9)),
        (ModuleKind::DIFFUSE, "threshold", Value::Float(0.4)),
        (ModuleKind::DIFFUSE, "highlights", Value::Float(0.8)),
        (ModuleKind::SURFACE_BLUR, "amount", Value::Float(0.9)),
        (ModuleKind::SURFACE_BLUR, "radius", Value::Float(0.05)),
        (ModuleKind::SURFACE_BLUR, "radius", Value::Float(0.002)),
        (ModuleKind::SURFACE_BLUR, "edges", Value::Float(0.3)),
    ];
    for k in 0..4 {
        moves.push((ModuleKind::DIFFUSE, SPEED_FIELDS[k], Value::Float(-0.7)));
        moves.push((ModuleKind::DIFFUSE, ANISOTROPY_FIELDS[k], Value::Float(0.3)));
    }
    for (kind, name, v) in moves {
        let m = e.module_mut(kind, 0).unwrap();
        let mut f = m.params.to_fields();
        f.set(name, v);
        m.params.set_fields(f);
        for focus in [Some((kind, 0)), None] {
            // Twice: the first may meet a new focus split; the second must create nothing.
            for pass in 0..2 {
                let r = en.render(&graph_at(&e, w, h, focus), &src, &req).unwrap();
                if pass == 1 {
                    assert_eq!(r.stats.pipelines_created, 0, "{name} {focus:?}");
                }
                en.recycle(r);
            }
        }
    }
}

/// §3.9 "slider on a heavy op": draft ≤ 50 ms, refined ≤ 300 ms at fit view on a
/// 2560×1600 window (GPU timestamps and wall clock). Run on an idle machine:
/// `cargo test -p specular-develop diffuse_timings -- --ignored --nocapture`.
#[test]
#[ignore = "timings; run on an idle machine"]
fn diffuse_timings() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (2560u32, 1600u32);
    let img = scene(w, h, 0.03, 3);
    let src = en.upload(w, h, &img).unwrap();
    diffuse::F32_CASCADE.store(
        std::env::var_os("DV_F32_CASCADE").is_some(),
        std::sync::atomic::Ordering::Relaxed,
    );
    let mut cases: Vec<(String, Edit)> = Preset::ALL
        .iter()
        .map(|p| (format!("{p:?}"), after_exposure(preset_edit(*p))))
        .collect();
    cases.push(("surface direct".into(), surface_edit(1.0, 0.002, 0.1)));
    cases.push(("surface grid".into(), surface_edit(1.0, 0.01, 0.1)));
    let req = Request {
        profile: en.has_timestamps(),
        checkpoints: false,
        ..Request::new(Roi::full(w, h))
    };
    for (name, e) in cases {
        for draft in [true, false] {
            let focus = draft.then_some((e.modules.last().unwrap().kind, 0));
            let g = graph_at(&e, w, h, focus);
            let mut gpu_best = f64::MAX;
            let mut wall = f64::MAX;
            let mut by_kernel: Vec<(String, f64, usize)> = Vec::new();
            for _ in 0..3 {
                let t = std::time::Instant::now();
                let r = en.render(&g, &src, &req).unwrap();
                let _ = en.read(&r).unwrap();
                wall = wall.min(t.elapsed().as_secs_f64() * 1e3);
                let total = r.stats.gpu_total_ms().unwrap_or(f64::NAN);
                if total < gpu_best {
                    by_kernel.clear();
                    for (k, ms) in r.stats.gpu_ms.iter().flatten() {
                        match by_kernel.iter_mut().find(|(n, _, _)| n == k) {
                            Some(e) => {
                                e.1 += ms;
                                e.2 += 1;
                            }
                            None => by_kernel.push((k.clone(), *ms, 1)),
                        }
                    }
                }
                gpu_best = gpu_best.min(total);
                en.recycle(r);
            }
            eprintln!("{name} draft={draft}: GPU {gpu_best:.1} ms, wall {wall:.1} ms");
            if std::env::var_os("DV_PROFILE").is_some() {
                by_kernel.sort_by(|a, b| b.1.total_cmp(&a.1));
                for (k, ms, n) in by_kernel.iter().take(4) {
                    eprintln!("    {k}: {ms:.1} ms ({n} dispatches)");
                }
            }
        }
    }
}

/// Tuning aid: edge rise and overshoot of sharpening candidates.
#[test]
#[ignore = "tuning aid"]
fn diffuse_tune() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (64, 16);
    let edge = soft_edge(w, h, 1.5);
    let (r0, _) = edge_stats(&edge, w, 8);
    eprintln!("rise {r0:.2}");
    for it in [2u32, 4, 8] {
        for span in [0.0003f32, 0.0006] {
            for (sn, sp) in [
                ("hf", [0.0, 0.0, -0.5, -0.25]),
                ("hf1", [0.0, 0.0, -1.0, -0.5]),
                ("lf+hf", [-0.5, 0.0, -0.5, -0.25]),
                ("all", [-0.5, -0.25, -0.5, -0.25]),
            ] {
                for a in [0.0f32, -1.0] {
                    let d = Diffuse {
                        iterations: it,
                        span,
                        speed: sp,
                        anisotropy: [a; 4],
                        threshold: 0.1,
                        ..Diffuse::default()
                    };
                    let e = edit(vec![module(
                        ModuleKind::DIFFUSE,
                        with_long(d.to_fields(), 3000),
                    )]);
                    let out = gpu(&mut en, &edge, w, h, &e, &whole(w, h, Precision::F32));
                    let (r, o) = edge_stats(&out, w, 8);
                    eprintln!(
                        "it {it} span {span} {sn} a {a}: rise {r:.2} over {:.1} %",
                        100.0 * o
                    );
                }
            }
        }
    }
}

/// Filling highlights makes the bands only near pixels that can change: the same picture,
/// bit for bit, as making them everywhere (refined and draft, whole and in tiles).
#[test]
fn highlight_tiles_change_nothing() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (1600u32, 1000u32);
    let img = scene(w, h, 0.03, 3);
    let e = preset_edit(Preset::InpaintHighlights);
    for focus in [None, Some((ModuleKind::DIFFUSE, 0))] {
        for tiling in [Tiling::Auto, Tiling::Side(512)] {
            let g = graph_at(&e, w, h, focus);
            let req = Request {
                tiling,
                ..whole(w, h, Precision::F32)
            };
            let mut out = Vec::new();
            for off in [true, false] {
                super::diffuse::NO_HOT_MASK.store(off, std::sync::atomic::Ordering::Relaxed);
                out.push(render_g(&mut en, &g, &img, w, h, &req));
            }
            super::diffuse::NO_HOT_MASK.store(false, std::sync::atomic::Ordering::Relaxed);
            let same = out[0]
                .iter()
                .zip(&out[1])
                .filter(|(a, b)| a.to_bits() == b.to_bits())
                .count();
            assert_eq!(same, out[0].len(), "{focus:?} {tiling:?}");
        }
    }
}

/// f16 renders keep the cascade at f16 (half the memory traffic): within f16's own
/// rounding of the f32 cascade.
#[test]
fn f16_cascade_stays_close() {
    use std::sync::atomic::Ordering;
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (480u32, 320u32);
    let img = scene(w, h, 0.03, 5);
    for (name, d) in presets() {
        let e = after_exposure(edit(vec![module(ModuleKind::DIFFUSE, d.to_fields())]));
        let mut out = Vec::new();
        for f32c in [true, false] {
            diffuse::F32_CASCADE.store(f32c, Ordering::Relaxed);
            let g = graph_at(&e, w, h, None);
            out.push(render_g(
                &mut en,
                &g,
                &img,
                w,
                h,
                &whole(w, h, Precision::F16),
            ));
        }
        diffuse::F32_CASCADE.store(false, Ordering::Relaxed);
        let worst = out[0]
            .iter()
            .zip(&out[1])
            .map(|(a, b)| (a - b).abs() / a.abs().max(0.05))
            .fold(0f32, f32::max);
        let de = delta_e_p95(&out[0], &out[1]);
        let effect = delta_e_p95(&out[0], &img);
        eprintln!(
            "{name}: f16 vs f32 cascade worst relative {worst:.2e}, ΔE p95 {de:.3} (effect {effect:.2})"
        );
        assert!(de < 0.1 && worst < 0.02, "{name}: {de} {worst}");
    }
}
