//! Per-module GPU against CPU reference in CIEDE2000 (`docs/develop-quality.md`): one
//! photo-like scene, each module alone at a typical setting, rendered in f32 and by its CPU
//! reference. The per-op tests hold the tight float tolerances; this one gives every module
//! the same perceptual measure for the report.
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --lib \
//!       ops::tests_quality -- --nocapture

use super::cpu_ref;
use super::detail::capture::Capture;
use super::detail::denoise::Denoise;
use super::detail::diffuse::{Diffuse, LONG_FIELD, Preset};
use super::detail::sharpen::Sharpen;
use super::detail::surface::SurfaceBlur;
use super::detail::{NoiseModel, cpu as detail_cpu, cpu_diffuse};
use super::effects::{cpu as fx_cpu, dehaze, glow, graduated, grain, negative, vignette};
use super::tone::cpu::Img;
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

const W: u32 = 256;
const H: u32 = 170;

/// Linear Rec.2020 RGBA: sky gradient, coloured patches, skin, an edge, fine stripes, noise
/// and a highlight above white.
fn scene() -> Vec<f32> {
    let mut v = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let (u, t) = (x as f32 / W as f32, y as f32 / H as f32);
            let mut c = if t < 0.4 {
                [0.25 + 0.3 * t, 0.35 + 0.3 * t, 0.6]
            } else {
                let k = ((u * 7.0).floor() + (t * 5.0).floor()) % 5.0;
                [
                    [0.05, 0.12, 0.03],
                    [0.3, 0.12, 0.06],
                    [0.02, 0.03, 0.04],
                    [0.15, 0.15, 0.12],
                    [0.45, 0.3, 0.22],
                ][k as usize]
            };
            if u > 0.8 && t > 0.6 {
                let s = if (x / 2) % 2 == 0 { 0.5 } else { 0.1 };
                c = [s, s, s];
            }
            let (dx, dy) = (u - 0.7, t - 0.2);
            if dx * dx + dy * dy < 0.003 {
                c = [3.0, 2.8, 2.4];
            }
            let n = ((x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) % 1000) as f32
                / 1000.0
                - 0.5;
            v.extend_from_slice(&[
                (c[0] + 0.01 * n).max(0.0),
                (c[1] + 0.01 * n).max(0.0),
                (c[2] + 0.01 * n).max(0.0),
                1.0,
            ]);
        }
    }
    v
}

fn module(kind: ModuleKind, f: Fields) -> ModuleInstance {
    let mut m = ModuleInstance::new(Registry::builtin(), kind, 0);
    let mut all = m.params.to_fields();
    all.overlay(&f);
    m.params = Params::from_fields(kind, all);
    m
}

fn fl(pairs: &[(&str, f32)]) -> Fields {
    let mut f = Fields::new();
    for (k, v) in pairs {
        f.set(k, Value::Float(*v));
    }
    f
}

fn gpu(en: &mut Engine, img: &[f32], m: &ModuleInstance) -> (Graph, Vec<f32>) {
    let mut e = Edit::empty(SourceKey::default());
    e.insert(Registry::builtin(), m.clone());
    let src = en.upload(W, H, img).expect("upload");
    let g = Graph::build(&e, OpRegistry::builtin(), None).expect("graph");
    let req = Request {
        precision: Precision::F32,
        checkpoints: false,
        ..Request::new(Roi::full(W, H))
    };
    let r = en.render(&g, &src, &req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    (g, px)
}

/// Linear Rec.2020 → CIELAB (D65), negatives clipped.
fn lab(c: &[f32]) -> [f64; 3] {
    let [r, g, b] = [c[0], c[1], c[2]].map(|v| f64::from(v).max(0.0));
    let x = 0.636_958 * r + 0.144_617 * g + 0.168_881 * b;
    let y = 0.262_700 * r + 0.677_998 * g + 0.059_302 * b;
    let z = 0.0 * r + 0.028_073 * g + 1.060_985 * b;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.95047), f(y), f(z / 1.08883));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 (Sharma, Wu and Dalal 2005).
fn de2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let c1 = a[1].hypot(a[2]);
    let c2 = b[1].hypot(b[2]);
    let cb = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cb.powi(7) / (cb.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a[1], (1.0 + g) * b[1]);
    let c1p = a1p.hypot(a[2]);
    let c2p = a2p.hypot(b[2]);
    let hue = |y: f64, x: f64| {
        if y == 0.0 && x == 0.0 {
            0.0
        } else {
            let h = y.atan2(x);
            if h < 0.0 { h + 2.0 * PI } else { h }
        }
    };
    let (h1p, h2p) = (hue(a[2], a1p), hue(b[2], a2p));
    let dl = b[0] - a[0];
    let dc = c2p - c1p;
    let mut dh = h2p - h1p;
    if c1p * c2p == 0.0 {
        dh = 0.0;
    } else if dh > PI {
        dh -= 2.0 * PI;
    } else if dh < -PI {
        dh += 2.0 * PI;
    }
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).sin();
    let lb = (a[0] + b[0]) / 2.0;
    let cbp = (c1p + c2p) / 2.0;
    let hb = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= PI {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 2.0 * PI {
        (h1p + h2p + 2.0 * PI) / 2.0
    } else {
        (h1p + h2p - 2.0 * PI) / 2.0
    };
    let t = 1.0 - 0.17 * (hb - PI / 6.0).cos()
        + 0.24 * (2.0 * hb).cos()
        + 0.32 * (3.0 * hb + PI / 30.0).cos()
        - 0.20 * (4.0 * hb - 63.0 * PI / 180.0).cos();
    let dtheta = 30.0 * PI / 180.0 * (-((hb * 180.0 / PI - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lb - 50.0).powi(2) / (20.0 + (lb - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let rt = -(2.0 * dtheta).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh))
        .sqrt()
}

/// (p50, p95, p99, max) of ΔE2000 between two images.
fn stats(a: &[f32], b: &[f32]) -> [f64; 4] {
    assert_eq!(a.len(), b.len());
    let mut v: Vec<f64> = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .map(|(p, q)| {
            assert!(p.iter().all(|v| v.is_finite()), "non-finite {p:?}");
            de2000(lab(p), lab(q))
        })
        .collect();
    v.sort_by(f64::total_cmp);
    let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
    [at(0.5), at(0.95), at(0.99), v[v.len() - 1]]
}

/// A .cube in the module's default space (a mild warm curve), for the LUT case.
fn cube() -> std::path::PathBuf {
    let dir: std::path::PathBuf =
        std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, Into::into);
    let p = dir.join(format!("dv26-warm-{}.cube", std::process::id()));
    let mut s = String::from("LUT_3D_SIZE 9\n");
    for b in 0..9 {
        for g in 0..9 {
            for r in 0..9 {
                let f = |i: u32, k: f32| (i as f32 / 8.0).powf(0.92) * k;
                s.push_str(&format!("{} {} {}\n", f(r, 1.0), f(g, 0.98), f(b, 0.9)));
            }
        }
    }
    std::fs::write(&p, s).expect("write the cube");
    p
}

#[test]
fn every_module_against_its_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(2 << 30));
    let img = scene();
    let im = Img {
        w: W,
        h: H,
        px: &img,
    };
    let full = Roi::full(W, H);
    let long = W.max(H) as f32;
    let cube = cube();
    use ModuleKind as K;

    // Point, tone and colour ops: the graph's own CPU reference.
    let graph_cases: Vec<(&str, ModuleInstance)> = vec![
        ("Exposure", module(K::EXPOSURE, fl(&[("exposure_ev", 0.7)]))),
        (
            "Auto exposure",
            module(
                K::EXPOSURE,
                fl(&[("exposure_ev", 0.3)]).with("auto", Value::Bool(true)),
            ),
        ),
        (
            "White balance (CAT16, gamut compression)",
            module(
                K::COLOUR_CALIBRATION,
                fl(&[("white_x", 0.365), ("white_y", 0.37)])
                    .with("white_source", Value::Text("custom".into())),
            ),
        ),
        ("Look: sigmoid", module(K::VIEW_TRANSFORM, Fields::new())),
        (
            "Look: filmic",
            module(
                K::VIEW_TRANSFORM,
                Fields::new().with("method", Value::Text("filmic".into())),
            ),
        ),
        (
            "Look: AgX",
            module(
                K::VIEW_TRANSFORM,
                Fields::new().with("method", Value::Text("agx".into())),
            ),
        ),
        (
            "Tone equalizer",
            module(
                K::TONE_EQUALIZER,
                fl(&[("highlights_ev", -1.0), ("shadows_ev", 1.0)]),
            ),
        ),
        (
            "Local contrast",
            module(K::LOCAL_CONTRAST, fl(&[("texture", 0.5), ("clarity", 0.5)])),
        ),
        (
            "Curve and levels",
            module(
                K::CURVES,
                Fields::new().with(
                    "points",
                    Value::Floats(vec![0.0, 0.0, 0.25, 0.2, 0.75, 0.82, 1.0, 1.0]),
                ),
            ),
        ),
        (
            "Colour balance and wheels",
            module(
                K::COLOUR_BALANCE,
                fl(&[
                    ("vibrance", 0.4),
                    ("saturation", -0.2),
                    ("brilliance", 0.3),
                    ("shadows_hue", 210.0),
                    ("shadows_chroma", 0.4),
                    ("highlights_hue", 40.0),
                    ("highlights_chroma", 0.3),
                ]),
            ),
        ),
        (
            "Colour mixer",
            module(
                K::COLOUR_EQUALIZER,
                fl(&[
                    ("hue_red", 15.0),
                    ("saturation_blue", 0.5),
                    ("brightness_green", -0.4),
                ]),
            ),
        ),
        (
            "Point colour",
            module(
                K::POINT_COLOUR,
                fl(&[
                    ("target_hue", 60.0),
                    ("target_saturation", 0.3),
                    ("target_lightness", 0.6),
                    ("shift_hue", -25.0),
                    ("shift_saturation", 0.5),
                    ("shift_lightness", 0.3),
                    ("range", 0.8),
                ])
                .with("picked", Value::Bool(true)),
            ),
        ),
        (
            "Black and white",
            module(K::BLACK_WHITE, fl(&[("mix_red", 0.3), ("mix_blue", -0.3)])),
        ),
        (
            "LUT (.cube, tetrahedral)",
            module(
                K::LUT_3D,
                Fields::new().with("path", Value::Text(cube.to_string_lossy().into_owned())),
            ),
        ),
    ];
    let mut rows: Vec<(String, [f64; 4], f64)> = Vec::new();
    for (name, m) in &graph_cases {
        let (g, got) = gpu(&mut en, &img, m);
        let want = cpu_ref::run_image(&g, W, H, &img);
        rows.push((
            (*name).to_owned(),
            stats(&got, &want),
            stats(&img, &want)[3],
        ));
    }
    let _ = std::fs::remove_file(&cube);

    // Effects.
    let fx: Vec<(&str, ModuleInstance, Vec<f32>)> = vec![
        {
            let f = fl(&[("amount", -1.0), ("roundness", 0.4)]);
            let v = vignette::Vignette::from_fields(&f);
            ("Vignette", module(K::VIGNETTE, f), fx_cpu::vignette(&v, im))
        },
        {
            let f = fl(&[("amount", -1.5), ("angle", 15.0), ("tint_amount", 0.3)]);
            let g = graduated::Graduated::from_fields(&f);
            (
                "Graduated filter",
                module(K::GRADUATED_ND, f),
                fx_cpu::graduated(&g, im),
            )
        },
        {
            let f = fl(&[("amount", 0.4), ("size", 0.25)]).with("seed", Value::Int(1234));
            let g = grain::Grain::from_fields(&f);
            ("Grain", module(K::GRAIN, f), fx_cpu::grain(&g, im))
        },
        {
            let f = fl(&[("d_max", 1.6), ("grade", 2.5)]);
            let n = negative::Negative::from_fields(&f);
            ("Negative", module(K::NEGATIVE, f), fx_cpu::negative(&n, im))
        },
        {
            let f = fl(&[("amount", 0.6), ("threshold", 0.5)]);
            let g = glow::Glow::from_fields(&f);
            ("Glow: bloom", module(K::GLOW, f), fx_cpu::glow(&g, im))
        },
        {
            let f = fl(&[("amount", 0.5), ("threshold", -2.0)])
                .with("mode", Value::Text("soften".into()));
            let g = glow::Glow::from_fields(&f);
            ("Glow: soften", module(K::GLOW, f), fx_cpu::glow(&g, im))
        },
        {
            let f = fl(&[("strength", 0.5)]);
            let h = dehaze::Dehaze::from_fields(&f);
            ("Dehaze", module(K::DEHAZE, f), fx_cpu::dehaze(&h, im).0)
        },
    ];
    for (name, m, want) in fx {
        let (_, got) = gpu(&mut en, &img, &m);
        rows.push((name.to_owned(), stats(&got, &want), stats(&img, &want)[3]));
    }

    // Detail.
    let model = NoiseModel {
        a: [3.0e-4, 2.0e-4, 2.6e-4],
        b: [4.0e-7, 2.0e-7, 3.0e-7],
    };
    let mut dn = fl(&[("luminance", 0.6), ("colour", 0.6)]);
    model.write(&mut dn);
    let dm = module(K::DENOISE, dn);
    let dv = Denoise::from_fields(&dm.params.to_fields());
    let sm = module(K::SHARPEN, fl(&[("amount", 1.0)]));
    let sv = Sharpen::from_fields(&sm.params.to_fields());
    let cm = module(
        K::CAPTURE_SHARPENING,
        fl(&[("amount", 0.8), ("radius", 1.0)]),
    );
    let cv = Capture::from_fields(&cm.params.to_fields());
    let detail: Vec<(&str, ModuleInstance, Vec<f32>)> = vec![
        (
            "Noise reduction",
            dm,
            detail_cpu::denoise(&im, &dv, full, full, 1.0),
        ),
        (
            "Sharpening",
            sm,
            detail_cpu::sharpen(&im, &sv, full, full, 1.0),
        ),
        (
            "Capture sharpening",
            cm,
            detail_cpu::capture(&im, &cv, full, full, 1.0),
        ),
    ];
    for (name, m, want) in detail {
        let (_, got) = gpu(&mut en, &img, &m);
        rows.push((name.to_owned(), stats(&got, &want), stats(&img, &want)[3]));
    }
    // Diffuse at a 4000-pixel long side on this small picture, as its own test runs it (the
    // striped refined render stays off: the CPU runs the whole picture at once).
    let half = &super::detail::diffuse::NO_STRIPES;
    half.store(true, std::sync::atomic::Ordering::Relaxed);
    for p in [
        Preset::Sharpen,
        Preset::LocalContrast,
        Preset::Bloom,
        Preset::InpaintHighlights,
    ] {
        let mut d = p.params();
        d.iterations = d.iterations.min(6);
        let mut f = d.to_fields();
        f.set(LONG_FIELD, Value::Float(4000.0));
        let d = Diffuse::from_fields(&f);
        let (_, got) = gpu(&mut en, &img, &module(K::DIFFUSE, f));
        let want = cpu_diffuse::diffuse(&img, W, H, &d);
        rows.push((
            format!("Diffuse: {p:?}"),
            stats(&got, &want),
            stats(&img, &want)[3],
        ));
    }
    half.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut f = fl(&[("amount", 1.0), ("radius", 0.01), ("edges", 0.12)]);
    f.set(LONG_FIELD, Value::Float(long));
    let s = SurfaceBlur::from_fields(&f);
    let (_, got) = gpu(&mut en, &img, &module(K::SURFACE_BLUR, f));
    let want = cpu_diffuse::surface_blur(&img, W, H, &s);
    rows.push((
        "Surface blur".to_owned(),
        stats(&got, &want),
        stats(&img, &want)[3],
    ));

    println!("| Module | GPU vs CPU ΔE2000 p50 / p95 / p99 / max | Change made, max |");
    println!("|---|---|---|");
    let mut bad = Vec::new();
    for (name, [p50, p95, p99, max], change) in &rows {
        println!("| {name} | {p50:.4} / {p95:.4} / {p99:.4} / {max:.4} | {change:.2} |");
        // The setting did something: a GPU = CPU match of two identities proves nothing.
        if *change < 0.5 {
            bad.push(format!("{name}: changes nothing (max {change:.3})"));
        }
        // Invisible: well under one just-noticeable difference everywhere.
        if *max >= 0.5 {
            bad.push(format!("{name}: max {max:.3}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
