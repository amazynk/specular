//! Diffuse speed: timings of every preset, refined and draft, at
//! 2560×1600 and 24 MP, with dispatch counts, nominal bytes and the GPU time per band.

use super::diffuse::{self, Diffuse, Preset};
use super::tests_diffuse::{
    after_exposure, edit, engine, extremes, graph_at, module, preset_edit, presets, render_g,
    scene, whole,
};
use crate::gpu::Request;
use crate::graph::tile::Tiling;
use crate::graph::{Precision, Roi, Step};
use crate::model::{Edit, ModuleKind};
use crate::testgpu::lock;

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_owned))
        .unwrap_or_default()
}

/// Nominal bytes a step moves (each picture word read once and written once per pixel).
fn bytes(s: &Step, half: bool) -> u64 {
    let px = u64::from(s.threads[0]) * u64::from(s.threads[1]);
    // f32 iterate pixel and cascade pixel (packed f16 in f16 renders).
    let (it, cas) = (12, if half { 8 } else { 12 });
    let per = diffuse::step_bytes(s, it, cas);
    px * per
}

/// One mode's best render of a case.
#[derive(Default)]
struct Best {
    gpu: f64,
    wall: f64,
    dispatches: usize,
    tiles: usize,
    per_band: Vec<(String, f64)>,
}

/// `cargo test -p specular-develop diffuse_speed_timings -- --ignored --nocapture`
/// (`DIFFUSE_SPEED_N` best of, `DIFFUSE_SPEED_SIZES` like `2560x1600,6000x4000`,
/// `DIFFUSE_SPEED_PRESETS` names, `DIFFUSE_SPEED_MODES` of `old` ([`super::diffuse_legacy`]),
/// `full` (every band on the full grid), `new`; the modes alternate render by render, so load hits them alike;
/// `DIFFUSE_SPEED_BANDS` prints the GPU time per step kind).
#[test]
#[ignore = "timings; run on an idle machine"]
fn diffuse_speed_timings() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let n: usize = std::env::var("DIFFUSE_SPEED_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let sizes: Vec<(u32, u32)> = std::env::var("DIFFUSE_SPEED_SIZES")
        .unwrap_or_else(|_| "2560x1600,6000x4000".into())
        .split(',')
        .filter_map(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect();
    let only = std::env::var("DIFFUSE_SPEED_PRESETS").ok();
    let modes: Vec<String> = std::env::var("DIFFUSE_SPEED_MODES")
        .unwrap_or_else(|_| "old,full,new".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    let mode = |m: &str| {
        set(&diffuse::LEGACY, m == "old");
        set(&diffuse::FULL_GRID, m == "full");
    };
    for (w, h) in sizes {
        let img = scene(w, h, 0.03, 3);
        let src = en.upload(w, h, &img).unwrap();
        for p in Preset::ALL {
            let name = format!("{p:?}");
            if only
                .as_ref()
                .is_some_and(|o| !o.split(',').any(|q| q == name))
            {
                continue;
            }
            let e = after_exposure(preset_edit(p));
            for (draft, prec) in [
                (true, Precision::F16),
                (false, Precision::F16),
                (false, Precision::F32),
            ] {
                if prec == Precision::F32 && w < 4000 {
                    continue;
                }
                let focus = draft.then_some((ModuleKind::DIFFUSE, 0));
                let g = graph_at(&e, w, h, focus);
                let node = g
                    .nodes
                    .iter()
                    .find(|nd| nd.id.0 == ModuleKind::DIFFUSE)
                    .unwrap();
                let d = Diffuse::of(&node.params);
                let req = Request {
                    profile: en.has_timestamps(),
                    checkpoints: false,
                    precision: prec,
                    tiling: Tiling::Auto,
                    ..Request::new(Roi::full(w, h))
                };
                let mut best: Vec<Best> = modes
                    .iter()
                    .map(|_| Best {
                        gpu: f64::MAX,
                        wall: f64::MAX,
                        ..Best::default()
                    })
                    .collect();
                for _ in 0..n {
                    for (m, b) in modes.iter().zip(best.iter_mut()) {
                        mode(m);
                        let t = std::time::Instant::now();
                        let r = match en.render(&g, &src, &req) {
                            Ok(r) => r,
                            Err(e) => {
                                b.wall = f64::NAN;
                                eprintln!("{name} {m}: {e:?}");
                                continue;
                            }
                        };
                        let _ = en.read(&r).unwrap();
                        b.wall = b.wall.min(t.elapsed().as_secs_f64() * 1e3);
                        let total = r.stats.gpu_total_ms().unwrap_or(f64::NAN);
                        b.dispatches = r.stats.dispatches;
                        b.tiles = r.stats.tiles;
                        if total < b.gpu {
                            b.gpu = total;
                            b.per_band.clear();
                            if b.tiles == 1
                                && let Some(ms) = &r.stats.gpu_ms
                            {
                                let prog = diffuse::program(&d, Roi::full(w, h));
                                let first = prog.steps[0].kernel.name;
                                if let Some(i) = ms.iter().position(|(l, _)| l == first) {
                                    for (s, (_, t)) in prog.steps.iter().zip(&ms[i..]) {
                                        let k = diffuse::step_label(s);
                                        match b.per_band.iter_mut().find(|(n, _)| *n == k) {
                                            Some(e) => e.1 += t,
                                            None => b.per_band.push((k, *t)),
                                        }
                                    }
                                }
                            }
                        }
                        en.recycle(r);
                    }
                }
                for (m, b) in modes.iter().zip(&best) {
                    mode(m);
                    let prog = diffuse::program(&d, Roi::full(w, h));
                    let f16 = prec == Precision::F16;
                    let mb: u64 = prog.steps.iter().map(|s| bytes(s, f16)).sum::<u64>() >> 20;
                    eprintln!(
                        "{w}x{h} {name} {} {prec:?} {m}: GPU {:.1} ms, wall {:.1} ms, {} dispatches ({} program steps), {} tile(s), ≈{mb} MiB, scratch {} MiB, load {}",
                        if draft { "draft" } else { "refined" },
                        b.gpu,
                        b.wall,
                        b.dispatches,
                        prog.steps.len(),
                        b.tiles,
                        (prog.words * 4) >> 20,
                        load()
                    );
                    if std::env::var_os("DIFFUSE_SPEED_BANDS").is_some() {
                        for (k, t) in &b.per_band {
                            eprintln!("    {k}: {t:.1} ms");
                        }
                    }
                }
            }
        }
    }
    mode("new");
}

fn set(flag: &std::sync::atomic::AtomicBool, on: bool) {
    flag.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The edits the exactness tests run: every preset and the extremes (iterations held to 6).
fn cases() -> Vec<(String, Edit)> {
    let mut v: Vec<(String, Diffuse)> = presets();
    v.extend(extremes().into_iter().map(|(n, d)| (n.to_string(), d)));
    v.into_iter()
        .map(|(n, mut d)| {
            d.iterations = d.iterations.min(6);
            let e = after_exposure(edit(vec![module(ModuleKind::DIFFUSE, d.to_fields())]));
            (n, e)
        })
        .collect()
}

/// Items 1 and 3: the level steps and one band step per iteration (the change summed in
/// registers, the first iteration reading the input where it lies) give the pictures of
/// the old program (a level and an update per band, the change summed in a picture) bit
/// for bit, with every band on the full grid: f16 and f32, refined and draft, whole and
/// tiled.
#[test]
fn band_passes_equal_the_old_steps() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (640u32, 400u32);
    let img = scene(w, h, 0.03, 9);
    set(&diffuse::FULL_GRID, true);
    let mut checked = 0;
    for (name, e) in cases() {
        for focus in [None, Some((ModuleKind::DIFFUSE, 0))] {
            let g = graph_at(&e, w, h, focus);
            for prec in [Precision::F16, Precision::F32] {
                for tiling in [Tiling::Auto, Tiling::Side(256)] {
                    let req = Request {
                        tiling,
                        ..whole(w, h, prec)
                    };
                    let mut out = Vec::new();
                    for legacy in [true, false] {
                        set(&diffuse::LEGACY, legacy);
                        out.push(render_g(&mut en, &g, &img, w, h, &req));
                    }
                    set(&diffuse::LEGACY, false);
                    let differ = out[0]
                        .iter()
                        .zip(&out[1])
                        .filter(|(a, b)| a.to_bits() != b.to_bits())
                        .count();
                    if cfg!(target_os = "macos") {
                        // Metal fuses multiply-adds as it sees fit, differently in the two
                        // programs: not bit for bit there, only within rounding. A few ulp per
                        // step, which the sharpening presets (diffusion run backwards) grow over
                        // the iterations: measured up to 1.2e-4 in F32 and 1.4e-2 in F16 (near
                        // black, the 1e-3 floor), far below an 8-bit step; a wrong band or
                        // update is off by orders of magnitude more.
                        let tol = match prec {
                            Precision::F32 => 5e-4,
                            _ => 3e-2,
                        };
                        let rel = out[0]
                            .iter()
                            .zip(&out[1])
                            .map(|(a, b)| (a - b).abs() / a.abs().max(1e-3))
                            .fold(0f32, f32::max);
                        assert!(
                            rel <= tol,
                            "{name} {focus:?} {prec:?} {tiling:?}: {differ} differ, by {rel:e}"
                        );
                    } else {
                        assert_eq!(differ, 0, "{name} {focus:?} {prec:?} {tiling:?}");
                    }
                    checked += out[0].len();
                }
            }
        }
    }
    set(&diffuse::FULL_GRID, false);
    eprintln!("{checked} values, 0 differ");
}

/// ΔE 2000 of every pixel (clipped to 0–1), sorted.
pub(super) fn delta_es(a: &[f32], b: &[f32]) -> Vec<f64> {
    use crate::ops::tone::tests::{de2000, lab};
    let c = |p: &[f32]| lab([p[0], p[1], p[2]].map(|v| v.clamp(0.0, 1.0)));
    let mut d: Vec<f64> = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .map(|(p, q)| de2000(c(p), c(q)))
        .collect();
    d.sort_by(f64::total_cmp);
    d
}

/// Item 4: coarse bands on reduced grids against every band on the full grid, at a full
/// HD export (f32): ΔE 2000 p95 and max per preset.
#[test]
fn coarse_grids_stay_close_to_full_grids() {
    coarse_vs_full(1920, 1080);
}

/// As [`coarse_grids_stay_close_to_full_grids`] at a 4K export (several seconds of GPU per
/// preset; run when the GPU is otherwise idle).
#[test]
#[ignore = "heavy: a 4K export twice per preset"]
fn coarse_grids_stay_close_to_full_grids_4k() {
    coarse_vs_full(3840, 2160);
}

fn coarse_vs_full(w: u32, h: u32) {
    // Every band of a 4K Bloom on the full grid holds nine levels at once.
    let Some(d) = crate::testgpu::dev() else {
        return;
    };
    let budget = crate::gpu::pool::Budget::fixed(6 << 30);
    let mut en = crate::gpu::Engine::new(&d.device, &d.queue, budget);
    let _l = lock();
    let img = scene(w, h, 0.02, 12);
    let src = en.upload(w, h, &img).unwrap();
    for p in Preset::ALL {
        let e = preset_edit(p);
        let g = graph_at(&e, w, h, None);
        let req = whole(w, h, Precision::F32);
        let mut out = Vec::new();
        for full in [true, false] {
            set(&diffuse::FULL_GRID, full);
            let r = en.render(&g, &src, &req).unwrap();
            out.push(en.read(&r).unwrap());
            en.recycle(r);
        }
        set(&diffuse::FULL_GRID, false);
        let d = delta_es(&out[1], &out[0]);
        let p95 = d[(d.len() as f64 * 0.95) as usize];
        let max = d[d.len() - 1];
        let effect = delta_es(&img, &out[0]);
        let effect = effect[(effect.len() as f64 * 0.95) as usize];
        eprintln!(
            "{w}x{h} {p:?}: reduced vs full grids ΔE p95 {p95:.3} max {max:.3} (effect p95 {effect:.2})"
        );
        assert!(p95 < 0.1 && max < 1.0, "{p:?}: {p95} {max}");
    }
}

/// Diffuse's program steps in a profiled render (by their kernels' labels).
fn steps_run(r: &crate::gpu::RenderStats) -> usize {
    let ms = r.gpu_ms.as_ref().expect("a profiled render");
    ms.iter()
        .filter(|(l, _)| l.starts_with("diffuse_") && !l.starts_with("diffuse_v"))
        .count()
}

/// Item 2: an edit after diffuse resumes from its checkpoint, and a change of diffuse's
/// own blend (mask, opacity; the draft while dragging, then the refined render) reuses the
/// kept result: no diffuse step runs, and the pictures equal fresh renders bit for bit. An
/// edit before it or of its own params runs it again.
#[test]
fn edits_after_diffuse_reuse_it() {
    use crate::model::{Blend, Fields, MaskExpr, MaskShape, Registry, Shape, Value};
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (640u32, 400u32);
    let img = scene(w, h, 0.03, 4);
    let src = en.upload(w, h, &img).unwrap();
    let mut e = after_exposure(preset_edit(Preset::Bloom));
    let vig = Fields::new().with("amount", Value::Float(-0.5));
    e.insert(Registry::builtin(), module(ModuleKind::VIGNETTE, vig));
    e.masks = vec![MaskShape {
        id: 1,
        shape: Shape::Linear {
            start: [0.3, 0.5],
            end: [0.7, 0.5],
        },
    }];
    if !en.has_timestamps() {
        eprintln!("no timestamp queries: steps cannot be counted");
        return;
    }
    let req = Request {
        profile: true,
        ..Request::new(Roi::full(w, h))
    };
    let df = (ModuleKind::DIFFUSE, 0);
    let vg = (ModuleKind::VIGNETTE, 0);
    // (picture, program steps run, diffuse's kernel ran) of a render with the engine's
    // caches, checked against a fresh engine.
    let mut fresh = engine().unwrap();
    let mut run = |e: &Edit, focus: Option<(ModuleKind, u16)>| {
        let g = graph_at(e, w, h, focus);
        let r = en.render(&g, &src, &req).unwrap();
        let px = en.read(&r).unwrap();
        let steps = steps_run(&r.stats);
        let ran = r.stats.nodes_run.contains(&df);
        en.recycle(r);
        let want = render_g(&mut fresh, &g, &img, w, h, &req);
        let same = px
            .iter()
            .zip(&want)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "{focus:?}: differs from a fresh render");
        (steps, ran)
    };
    let set_f = |e: &mut Edit, k: (ModuleKind, u16), name: &str, v: f32| {
        let m = e.module_mut(k.0, k.1).unwrap();
        let mut f = m.params.to_fields();
        f.set(name, Value::Float(v));
        m.params.set_fields(f);
    };
    let (steps, _) = run(&e, None);
    assert!(steps > 0);
    // Vignette dragged, then refined: diffuse is not run.
    for (v, focus) in [(-0.3, Some(vg)), (-0.2, Some(vg)), (-0.2, None)] {
        set_f(&mut e, vg, "amount", v);
        let (steps, ran) = run(&e, focus);
        assert!(!ran && steps == 0, "vignette {v}: {steps} steps, ran {ran}");
    }
    // Diffuse's mask and opacity dragged: the first draft runs, the next reuse it; the
    // refined render after the drag reuses the first refined result.
    let mut drafts = Vec::new();
    for (k, op) in [0.5f32, 0.6, 0.7, 0.8].iter().enumerate() {
        e.module_mut(df.0, df.1).unwrap().blend = Some(Blend {
            mask: MaskExpr::Shape(1),
            opacity: *op,
            ..Blend::default()
        });
        let (steps, ran) = run(&e, Some(df));
        assert!(ran);
        drafts.push(steps);
        if k > 0 {
            assert_eq!(steps, 0, "opacity {op}");
        }
    }
    assert!(drafts[0] > 0);
    let (steps, _) = run(&e, None);
    assert_eq!(steps, 0, "refined after the blend drag");
    // Its own params and an edit before it run it again.
    set_f(&mut e, df, "speed_1", 0.9);
    assert!(run(&e, None).0 > 0);
    set_f(&mut e, (ModuleKind::EXPOSURE, 0), "exposure_ev", 0.2);
    assert!(run(&e, None).0 > 0);
}
