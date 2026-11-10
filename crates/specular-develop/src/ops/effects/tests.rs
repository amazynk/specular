//! The effects: GPU = CPU reference for every op, grain the same in preview and
//! (downscaled) export, dehaze against synthetic haze and its budget, no pipelines when
//! effect sliders move, and the effects' own properties (vignette follows the frame, ND
//! handles round-trip, negatives invert, frames lay out and blend).

use super::*;
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::ops::tone::cpu::Img;
use crate::testgpu::{dev, lock};

const GIB: u64 = 1 << 30;

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

fn fl(pairs: &[(&str, f32)]) -> Fields {
    let mut f = Fields::new();
    for (k, v) in pairs {
        f.set(k, Value::Float(*v));
    }
    f
}

/// A photo-like scene: sky gradient, a sunlit disc, coloured patches, dark ground.
fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut c = if t < 0.45 {
                [0.35 + 0.3 * t, 0.45 + 0.3 * t, 0.7]
            } else {
                let k = ((u * 7.0).floor() + (t * 5.0).floor()) % 4.0;
                let base = [
                    [0.05, 0.12, 0.03],
                    [0.2, 0.1, 0.05],
                    [0.02, 0.03, 0.04],
                    [0.15, 0.15, 0.12],
                ];
                base[k as usize]
            };
            let (dx, dy) = (u - 0.7, t - 0.2);
            if dx * dx + dy * dy < 0.004 {
                c = [6.0, 5.5, 4.5];
            }
            v.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    v
}

fn gpu(en: &mut Engine, img: &[f32], w: u32, h: u32, e: &Edit, scale: f32) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let g = Graph::build(e, OpRegistry::builtin(), None).expect("graph");
    let req = Request {
        precision: Precision::F32,
        checkpoints: false,
        scale,
        ..Request::new(Roi::full(w, h))
    };
    let r = en.render(&g, &src, &req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

/// Worst relative difference (over max(|b|, 0.01)) and the count over `tol`.
fn rel(a: &[f32], b: &[f32], tol: f32) -> (f32, usize) {
    assert_eq!(a.len(), b.len());
    let mut worst = 0f32;
    let mut over = 0;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = (0..4)
            .map(|k| (p[k] - q[k]).abs() / q[k].abs().max(0.01))
            .fold(0f32, f32::max);
        assert!(d.is_finite(), "{p:?} vs {q:?}");
        worst = worst.max(d);
        over += usize::from(d > tol);
    }
    (worst, over)
}

#[test]
fn every_effect_on_the_gpu_matches_its_cpu_reference() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (320u32, 200u32);
    let img = scene(w, h);
    let im = Img { w, h, px: &img };
    let cases: Vec<(&str, ModuleInstance, Vec<f32>)> = vec![
        {
            let f = fl(&[
                ("amount", -1.5),
                ("roundness", 0.4),
                ("shape", 0.3),
                ("protect", 0.7),
            ])
            .with("centre", Value::Floats(vec![0.1, -0.05]));
            let v = vignette::Vignette::from_fields(&f);
            (
                "vignette",
                module(ModuleKind::VIGNETTE, f),
                cpu::vignette(&v, im),
            )
        },
        {
            let f = fl(&[
                ("amount", -2.0),
                ("angle", 20.0),
                ("softness", 0.3),
                ("tint_amount", 0.5),
            ]);
            let g = graduated::Graduated::from_fields(&f);
            (
                "graduated",
                module(ModuleKind::GRADUATED_ND, f),
                cpu::graduated(&g, im),
            )
        },
        {
            let f = fl(&[("amount", 0.8), ("size", 0.5), ("bias", 0.2)])
                .with("colour", Value::Bool(true))
                .with("seed", Value::Int(1234));
            let g = grain::Grain::from_fields(&f);
            (
                "grain colour",
                module(ModuleKind::GRAIN, f),
                cpu::grain(&g, im),
            )
        },
        {
            // Fine grain: the per-pixel substitute (footprint over 2 cells).
            let f = fl(&[("amount", 0.8), ("size", 0.0)]).with("seed", Value::Int(7));
            let g = grain::Grain::from_fields(&f);
            (
                "grain fine",
                module(ModuleKind::GRAIN, f),
                cpu::grain(&g, im),
            )
        },
        {
            let f = fl(&[("d_max", 1.8), ("exposure", 0.5), ("grade", 3.0)])
                .with("base", Value::Floats(vec![0.9, 0.6, 0.4]));
            let n = negative::Negative::from_fields(&f);
            (
                "negative",
                module(ModuleKind::NEGATIVE, f),
                cpu::negative(&n, im),
            )
        },
        {
            let f = fl(&[("amount", 0.6), ("radius", 0.03), ("threshold", 1.0)]);
            let g = glow::Glow::from_fields(&f);
            ("glow bloom", module(ModuleKind::GLOW, f), cpu::glow(&g, im))
        },
        {
            let f = fl(&[("amount", 0.5), ("radius", 0.05), ("threshold", -2.0)])
                .with("mode", Value::Text("soften".into()));
            let g = glow::Glow::from_fields(&f);
            (
                "glow soften",
                module(ModuleKind::GLOW, f),
                cpu::glow(&g, im),
            )
        },
        {
            let f = fl(&[("strength", 0.8)]);
            let d = dehaze::Dehaze::from_fields(&f);
            (
                "dehaze",
                module(ModuleKind::DEHAZE, f),
                cpu::dehaze(&d, im).0,
            )
        },
        {
            let f = fl(&[("strength", -0.6)]);
            let d = dehaze::Dehaze::from_fields(&f);
            (
                "add haze",
                module(ModuleKind::DEHAZE, f),
                cpu::dehaze(&d, im).0,
            )
        },
    ];
    for (name, m, want) in cases {
        let got = gpu(&mut en, &img, w, h, &edit(vec![m]), 1.0);
        let (worst, over) = rel(&got, &want, 1e-3);
        eprintln!(
            "{name}: GPU vs CPU worst {worst:.2e}, {over} of {} over 1e-3",
            w * h
        );
        assert!(
            over <= (w * h / 1000) as usize && worst < 2e-2,
            "{name}: {worst} {over}"
        );
    }
}

/// Log2 of the grain (output over input) per pixel, green channel.
fn grain_log(out: &[f32], inp: f32) -> Vec<f64> {
    out.chunks_exact(4)
        .map(|p| (f64::from(p[1]) / f64::from(inp)).log2())
        .collect()
}

fn std(v: &[f64]) -> f64 {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt()
}

/// Lag-1 horizontal correlation.
fn corr1(v: &[f64], w: usize) -> f64 {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var: f64 = v.iter().map(|x| (x - m) * (x - m)).sum();
    let mut c = 0.0;
    for r in v.chunks_exact(w) {
        for k in 1..w {
            c += (r[k] - m) * (r[k - 1] - m);
        }
    }
    c / var
}

#[test]
fn grain_is_scale_stable() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (1536u32, 1024u32);
    let grey = 0.18f32;
    let img: Vec<f32> = (0..w * h).flat_map(|_| [grey, grey, grey, 1.0]).collect();
    for size in [0.3f32, 0.75] {
        let e = edit(vec![module(
            ModuleKind::GRAIN,
            fl(&[("amount", 0.6), ("size", size)]).with("seed", Value::Int(99)),
        )]);
        let full = gpu(&mut en, &img, w, h, &e, 1.0);
        let again = gpu(&mut en, &img, w, h, &e, 1.0);
        assert_eq!(full, again, "deterministic");
        let cell = grain::grain_cell(size) * w as f32;
        for k in [2u32, 4, 8] {
            let (pw, ph) = (w / k, h / k);
            // The preview's source is the photo reduced to the view (the engine renders it at
            // the view's scale).
            let small: Vec<f32> = (0..pw * ph).flat_map(|_| [grey, grey, grey, 1.0]).collect();
            let prev = gpu(&mut en, &small, pw, ph, &e, 1.0 / k as f32);
            assert_eq!(prev.len(), (pw * ph * 4) as usize);
            // The export reduced by k in linear light, as a resized export is.
            let mut down = vec![0f32; (pw * ph * 4) as usize];
            for y in 0..ph {
                for x in 0..pw {
                    let mut acc = 0f32;
                    for j in 0..k {
                        for i in 0..k {
                            acc += full[(((y * k + j) * w + x * k + i) * 4 + 1) as usize];
                        }
                    }
                    down[((y * pw + x) * 4 + 1) as usize] = acc / (k * k) as f32;
                }
            }
            let (a, b) = (grain_log(&prev, grey), grain_log(&down, grey));
            let (sa, sb) = (std(&a), std(&b));
            let (ca, cb) = (corr1(&a, pw as usize), corr1(&b, pw as usize));
            eprintln!(
                "grain cell {cell:.2} px, 1/{k}: preview σ {sa:.4}, export σ {sb:.4} ({:+.1} %), lag-1 {ca:.3} vs {cb:.3}",
                (sa / sb - 1.0) * 100.0
            );
            assert!((sa / sb - 1.0).abs() < 0.12, "σ {sa} vs {sb}");
            assert!((ca - cb).abs() < 0.12, "corr {ca} vs {cb}");
        }
    }
}

#[test]
fn grain_is_seeded_by_the_photo_not_the_time() {
    let (w, h) = (64u32, 48u32);
    let img: Vec<f32> = (0..w * h)
        .flat_map(|_| [0.18f32, 0.18, 0.18, 1.0])
        .collect();
    let im = Img { w, h, px: &img };
    let g = |seed: u32| grain::Grain {
        amount: 0.5,
        seed,
        ..grain::Grain::default()
    };
    assert_eq!(cpu::grain(&g(5), im), cpu::grain(&g(5), im));
    assert_ne!(cpu::grain(&g(5), im), cpu::grain(&g(6), im));
    assert_ne!(grain::seed_for("IMG_1.CR2"), grain::seed_for("IMG_2.CR2"));
    assert_eq!(grain::seed_for("IMG_1.CR2"), grain::seed_for("IMG_1.CR2"));
    // Midtones carry the grain; black does not.
    let u = g(5).uniforms();
    let dark = grain::cpu_pixel(&u, [1e-5, 1e-5, 1e-5, 1.0], 3, 3, w, h);
    assert!((dark[1] / 1e-5 - 1.0).abs() < 1e-3);
}

#[test]
fn box_sigma_matches_the_sampled_field() {
    // The substitute's σ against many box means of the real field, footprint 3 cells.
    let f = 3.0f32;
    let mut means = Vec::new();
    for b in 0..4000 {
        let (bx, by) = ((b % 63) as f32 * 3.7 * f, (b / 63) as f32 * 3.3 * f);
        let mut acc = 0f64;
        let n = 24;
        for j in 0..n {
            for i in 0..n {
                let x = bx + (i as f32 + 0.5) / n as f32 * f;
                let y = by + (j as f32 + 0.5) / n as f32 * f;
                acc += f64::from(grain::field(x, y, 77));
            }
        }
        means.push(acc / f64::from(n * n));
    }
    let s = std(&means);
    eprintln!(
        "box σ sampled {s:.4}, closed form {:.4}",
        grain::box_sigma(f)
    );
    assert!((s / f64::from(grain::box_sigma(f)) - 1.0).abs() < 0.08);
    // The field has unit variance.
    let pts: Vec<f64> = (0..20000)
        .map(|i| f64::from(grain::field(i as f32 * 0.731, (i / 97) as f32 * 1.37, 3)))
        .collect();
    let sd = std(&pts);
    assert!((sd - 1.0).abs() < 0.05, "{sd}");
}

/// A haze-free scene, a depth map, and the scene behind haze.
fn hazy(w: u32, h: u32, a: [f32; 3]) -> (Vec<f32>, Vec<f32>) {
    let mut clear = Vec::new();
    let mut hz = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            // Saturated patches (each has a dark channel near 0), finer at the bottom.
            let k = ((u * 12.0).floor() as i32 + (v * 8.0).floor() as i32 * 5) % 6;
            let pal = [
                [0.6, 0.15, 0.02],
                [0.05, 0.4, 0.1],
                [0.08, 0.1, 0.5],
                [0.5, 0.45, 0.03],
                [0.02, 0.3, 0.35],
                [0.3, 0.02, 0.25],
            ];
            let j = pal[k as usize];
            // Far at the top, near at the bottom.
            let depth = 2.0 * (1.0 - v);
            let t = (-0.9 * depth).exp();
            clear.extend_from_slice(&[j[0], j[1], j[2], 1.0]);
            hz.extend_from_slice(&[
                j[0] * t + a[0] * (1.0 - t),
                j[1] * t + a[1] * (1.0 - t),
                j[2] * t + a[2] * (1.0 - t),
                1.0,
            ]);
        }
    }
    // A bright sky band at the very top (where the atmosphere is read).
    for y in 0..h / 12 {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            hz[i..i + 3].copy_from_slice(&a);
            clear[i..i + 3].copy_from_slice(&a);
        }
    }
    (clear, hz)
}

fn rms_rgb(a: &[f32], b: &[f32], w: u32, from_row: u32) -> f64 {
    let mut s = 0.0;
    let mut n = 0.0;
    for (i, (p, q)) in a.chunks_exact(4).zip(b.chunks_exact(4)).enumerate() {
        if (i as u32) / w < from_row {
            continue;
        }
        for k in 0..3 {
            let d = f64::from(p[k] - q[k]);
            s += d * d;
            n += 1.0;
        }
    }
    (s / n).sqrt()
}

#[test]
fn dehaze_recovers_a_synthetic_hazy_scene() {
    let (w, h) = (480u32, 320u32);
    let a = [0.85f32, 0.88, 0.92];
    let (clear, hz) = hazy(w, h, a);
    let im = Img { w, h, px: &hz };
    let (out, st) = cpu::dehaze(&dehaze::Dehaze { strength: 1.0 }, im);
    eprintln!("dehaze: A {:?} (truth {a:?})", &st[4..7]);
    for k in 0..3 {
        assert!((st[4 + k] / a[k] - 1.0).abs() < 0.05, "A {:?}", &st[4..7]);
    }
    let before = rms_rgb(&hz, &clear, w, h / 12);
    let after = rms_rgb(&out, &clear, w, h / 12);
    eprintln!(
        "dehaze: A {:?}, RMS to the clear scene {before:.4} → {after:.4}",
        &st[4..7]
    );
    assert!(after < 0.45 * before, "{before} → {after}");
    // Adding haze moves away from the clear scene and towards A.
    let (more, _) = cpu::dehaze(&dehaze::Dehaze { strength: -1.0 }, im);
    assert!(rms_rgb(&more, &clear, w, h / 12) > before);
}

#[test]
fn effect_sliders_make_no_pipelines() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (256u32, 160u32);
    let img = scene(w, h);
    let src = en.upload(w, h, &img).unwrap();
    let mut e = edit(vec![
        module(ModuleKind::EXPOSURE, Fields::new()),
        module(ModuleKind::NEGATIVE, Fields::new()),
        module(ModuleKind::DEHAZE, fl(&[("strength", 0.3)])),
        module(ModuleKind::GRADUATED_ND, fl(&[("amount", -1.0)])),
        module(ModuleKind::VIGNETTE, fl(&[("amount", -0.5)])),
        module(ModuleKind::GLOW, fl(&[("amount", 0.3)])),
        module(ModuleKind::GRAIN, fl(&[("amount", 0.3)])),
    ]);
    let req = Request {
        precision: Precision::F16,
        scale: 0.5,
        ..Request::new(Roi::full(w, h))
    };
    let moves: [(ModuleKind, &str, Value); 12] = [
        (ModuleKind::DEHAZE, "strength", Value::Float(0.7)),
        (ModuleKind::DEHAZE, "strength", Value::Float(-0.4)),
        (ModuleKind::GRADUATED_ND, "angle", Value::Float(30.0)),
        (ModuleKind::GRADUATED_ND, "tint_amount", Value::Float(0.4)),
        (ModuleKind::VIGNETTE, "midpoint", Value::Float(0.3)),
        (ModuleKind::VIGNETTE, "shape", Value::Float(0.6)),
        (ModuleKind::GLOW, "radius", Value::Float(0.05)),
        (ModuleKind::GLOW, "threshold", Value::Float(0.0)),
        (ModuleKind::GRAIN, "size", Value::Float(0.8)),
        (ModuleKind::GRAIN, "bias", Value::Float(-0.5)),
        (ModuleKind::NEGATIVE, "grade", Value::Float(4.0)),
        (ModuleKind::NEGATIVE, "gloss", Value::Float(0.1)),
    ];
    for (kind, name, v) in moves {
        // The first value may meet a new focus split; the second must build nothing.
        for (pass, v) in [(0, Value::Float(0.123)), (1, v)] {
            let m = e.module_mut(kind, 0).unwrap();
            let mut f = m.params.to_fields();
            f.set(name, v);
            m.params.set_fields(f);
            let g = Graph::build(&e, OpRegistry::builtin(), Some((kind, 0))).unwrap();
            let r = en.render(&g, &src, &req).unwrap();
            if pass == 1 {
                assert_eq!(r.stats.pipelines_created, 0, "{name}");
            }
            en.recycle(r);
        }
    }
}

/// §3.9: dehaze on a fit view of a 2560×1600 window and at 24 MP full size. Run on an idle
/// machine: `cargo test -p specular-develop dehaze_timings -- --ignored --nocapture`.
#[test]
#[ignore = "timings; run on an idle machine"]
fn dehaze_timings() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    for (label, w, h, scale, prec) in [
        (
            "fit 2560x1600 of 6000x4000",
            2400u32,
            1600u32,
            0.4f32,
            Precision::F16,
        ),
        ("24 MP full", 6000, 4000, 1.0, Precision::F32),
    ] {
        let (_, img) = hazy(w, h, [0.85, 0.88, 0.92]);
        let src = en.upload(w, h, &img).unwrap();
        let mut e = edit(vec![
            module(ModuleKind::EXPOSURE, Fields::new()),
            module(ModuleKind::DEHAZE, fl(&[("strength", 0.5)])),
        ]);
        let req = Request {
            precision: prec,
            scale,
            profile: true,
            ..Request::new(Roi::full(w, h))
        };
        let mut run = |e: &Edit, focus: ModuleKind| {
            let g = Graph::build(e, OpRegistry::builtin(), Some((focus, 0))).unwrap();
            let t = std::time::Instant::now();
            let r = en.render(&g, &src, &req).unwrap();
            en.wait().unwrap();
            let wall = t.elapsed().as_secs_f64() * 1e3;
            let gpu = r.stats.gpu_total_ms().unwrap_or(0.0);
            en.recycle(r);
            (gpu, wall)
        };
        run(&e, ModuleKind::DEHAZE);
        let mut slider = Vec::new();
        let mut upstream = Vec::new();
        for i in 0..8 {
            for (kind, name, v) in [
                (ModuleKind::DEHAZE, "strength", 0.3 + 0.05 * i as f32),
                (ModuleKind::EXPOSURE, "exposure_ev", 0.1 * i as f32),
            ] {
                let m = e.module_mut(kind, 0).unwrap();
                let mut f = m.params.to_fields();
                f.set(name, Value::Float(v));
                m.params.set_fields(f);
                let r = run(&e, kind);
                if kind == ModuleKind::DEHAZE {
                    slider.push(r);
                } else {
                    upstream.push(r);
                }
            }
        }
        for (what, mut v) in [
            ("dehaze slider", slider),
            ("exposure under dehaze", upstream),
        ] {
            v.sort_by(|a, b| a.1.total_cmp(&b.1));
            let p50 = v[v.len() / 2];
            let p95 = v[v.len() - 1];
            eprintln!(
                "{label}, {what}: wall p50 {:.1} ms (GPU {:.1}), worst {:.1} ms",
                p50.1, p50.0, p95.1
            );
        }
    }
}

#[test]
fn vignette_darkens_corners_follows_the_frame_and_spares_highlights() {
    let v = vignette::Vignette {
        amount: -1.0,
        ..vignette::Vignette::default()
    };
    let u = v.uniforms();
    for (w, h) in [(600u32, 400u32), (300, 500)] {
        let mid = vignette::cpu_pixel(&u, [0.18; 4], w as i32 / 2, h as i32 / 2, w, h);
        assert!((mid[0] - 0.18).abs() < 1e-6);
        // Every corner of every frame gets the full amount.
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            let c = vignette::cpu_pixel(&u, [0.18; 4], x as i32, y as i32, w, h);
            assert!((c[0] / 0.18 - 0.5).abs() < 0.02, "{w}x{h} corner {c:?}");
        }
    }
    let p = vignette::Vignette { protect: 1.0, ..v }.uniforms();
    let bright = vignette::cpu_pixel(&p, [8.0, 8.0, 8.0, 1.0], 0, 0, 600, 400);
    assert!(bright[0] > 7.9, "{bright:?}");
}

#[test]
fn graduated_handles_round_trip_and_darken_the_top() {
    let mut g = graduated::Graduated {
        amount: -2.0,
        angle: 25.0,
        ..graduated::Graduated::default()
    };
    let (s, e) = g.ends(6000.0, 4000.0);
    let before = g;
    g.set_ends(s, e, 6000.0, 4000.0);
    assert!((g.angle - before.angle).abs() < 1e-3 && (g.softness - before.softness).abs() < 1e-5);
    assert!((g.centre[0] - before.centre[0]).abs() < 1e-5);
    let flat = graduated::Graduated {
        amount: -2.0,
        ..graduated::Graduated::default()
    };
    let u = flat.uniforms();
    let top = graduated::cpu_pixel(&u, [0.4; 4], 300, 0, 600, 400);
    let bottom = graduated::cpu_pixel(&u, [0.4; 4], 300, 399, 600, 400);
    assert!((top[0] / 0.4 - 0.25).abs() < 0.01 && (bottom[0] - 0.4).abs() < 1e-4);
    // The tint has luminance 1.
    let t = graduated::Graduated::default().tint_rgb();
    assert!((luma(t) - 1.0).abs() < 1e-4);
}

#[test]
fn negatives_invert_monotonically() {
    let n = negative::Negative {
        base: [0.8, 0.5, 0.3],
        ..negative::Negative::default()
    };
    let u = n.uniforms();
    // Denser negative (less light through) = brighter scene = brighter print.
    let mut last = 0.0;
    for k in 0..12 {
        let t = 10f32.powf(-0.2 * k as f32);
        let c = negative::cpu_pixel(&u, [0.8 * t, 0.5 * t, 0.3 * t, 1.0]);
        assert!(c[0] > last, "{k}");
        // The base's orange mask is gone: neutral densities print neutral.
        assert!((c[0] - c[1]).abs() < 1e-5 && (c[1] - c[2]).abs() < 1e-5);
        last = c[0];
    }
    // The rebate prints close to the paper's black; mid density near mid grey.
    let rebate = negative::cpu_pixel(&u, [0.8, 0.5, 0.3, 1.0]);
    assert!(rebate[0] < 0.06, "{rebate:?}");
    let t = 10f32.powf(-0.9);
    let mid = negative::cpu_pixel(&u, [0.8 * t, 0.5 * t, 0.3 * t, 1.0]);
    assert!((0.1..0.3).contains(&mid[0]), "{mid:?}");
    let bw = negative::Negative {
        black_white: true,
        ..n
    }
    .uniforms();
    let g = negative::cpu_pixel(&bw, [0.3, 0.2, 0.05, 1.0]);
    assert!(g[0] == g[1] && g[1] == g[2]);
}

#[test]
fn frames_lay_out_expand_variables_and_blend() {
    use crate::export::Space;
    use crate::export::render::{ExportStats, Exported, Samples};
    let f = frame::Frame {
        border: 0.05,
        aspect: 1.0,
        ..frame::Frame::default()
    };
    let l = f.layout(600, 400, None);
    assert_eq!(l.canvas, (660, 660));
    assert_eq!(l.image, [30, 130, 600, 400]);
    let px = frame::Frame {
        border: 12.0,
        border_px: true,
        watermark: true,
        anchor: 8,
        scale: 0.25,
        margin: 0.0,
        ..frame::Frame::default()
    };
    let l = px.layout(600, 400, Some(2.0));
    assert_eq!(l.canvas, (624, 424));
    assert_eq!(l.mark, Some([12 + 450, 12 + 325, 150, 75]));

    let mut v = frame::Vars::default();
    v.set("title", "Dune <1>");
    v.set("exif.iso", "ISO 100");
    assert_eq!(
        v.expand("$(title), $(exif.iso), $(nope)$$"),
        "Dune <1>, ISO 100, $"
    );
    assert_eq!(v.expand_xml("$(title)"), "Dune &lt;1&gt;");

    // Border colour and a half-transparent mark over mid grey, sRGB 8-bit.
    let mut img = Exported {
        width: 4,
        height: 2,
        samples: Samples::Eight(vec![128; 4 * 2 * 3]),
        stats: ExportStats::default(),
        gain_map: None,
        alpha: None,
    };
    let fr = frame::Frame {
        border: 1.0,
        border_px: true,
        colour: [1.0, 0.0, 0.0],
        watermark: true,
        opacity: 1.0,
        scale: 1.0,
        margin: 0.0,
        ..frame::Frame::default()
    };
    let mark = frame::Mark {
        w: 4,
        h: 2,
        rgba: [255u8, 255, 255, 128].repeat(8),
    };
    frame::apply(&mut img, &fr, Some(&mark), Space::Srgb, false);
    assert_eq!((img.width, img.height), (6, 4));
    let Samples::Eight(s) = &img.samples else {
        panic!()
    };
    assert_eq!(&s[0..3], &[255, 0, 0]);
    let mid = &s[(6 + 1) * 3..(6 + 1) * 3 + 3];
    // Linear 0.216 and 1.0 mixed half and half: about 0.61, sRGB ≈ 203.
    assert!((200..=206).contains(&mid[0]), "{mid:?}");
}

/// A float (HDR) export keeps its highlights under the watermark: linear 4.0 half covered
/// by white gives 2.5, not a value clipped to 1.
#[test]
fn float_exports_keep_highlights_under_the_watermark() {
    use crate::export::Space;
    use crate::export::render::{ExportStats, Exported, Samples};
    let mut img = Exported {
        width: 4,
        height: 2,
        samples: Samples::Float(vec![4.0; 4 * 2 * 3]),
        stats: ExportStats::default(),
        gain_map: None,
        alpha: None,
    };
    let fr = frame::Frame {
        watermark: true,
        opacity: 1.0,
        scale: 1.0,
        margin: 0.0,
        ..frame::Frame::default()
    };
    let mark = frame::Mark {
        w: 4,
        h: 2,
        rgba: [255u8, 255, 255, 128].repeat(8),
    };
    frame::apply(&mut img, &fr, Some(&mark), Space::Srgb, true);
    let Samples::Float(s) = &img.samples else {
        panic!()
    };
    let a = 128.0f32 / 255.0;
    let want = 4.0 * (1.0 - a) + a;
    assert!(s.iter().all(|v| (v - want).abs() < 1e-4), "{:?}", &s[..3]);
}

#[test]
fn an_svg_watermark_draws_at_its_size() {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"100\">\
               <rect x=\"0\" y=\"0\" width=\"100\" height=\"100\" fill=\"#ff0000\"/></svg>";
    let m = frame::render_mark(svg, 300).expect("drawn");
    assert_eq!((m.w, m.h), (300, 150));
    let at =
        |x: u32, y: u32| &m.rgba[((y * m.w + x) * 4) as usize..((y * m.w + x) * 4 + 4) as usize];
    assert_eq!(at(10, 10), &[255, 0, 0, 255]);
    assert_eq!(at(290, 10)[3], 0);
    let big = frame::render_mark(svg, 2400).expect("drawn large");
    assert_eq!((big.w, big.h), (2400, 1200));
}
