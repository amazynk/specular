//! Diffuse: renders split into short submissions, 24 MP within
//! the memory budget, scratch let go, and the timings of every preset.

use super::diffuse::{self, Preset};
use super::tests_diffuse::{after_exposure, engine, graph_at, preset_edit, scene};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Error, Request};
use crate::graph::tile::Tiling;
use crate::graph::{Precision, Roi};
use crate::model::ModuleKind;
use crate::testgpu::{dev, lock};

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

fn load() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_owned))
        .unwrap_or_default()
}

fn env_list(name: &str, default: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_else(|_| default.into())
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `cargo test -p specular-develop diffuse_budget_timings -- --ignored --nocapture`
/// (`DIFFUSE_BUDGET_N` best of, `DIFFUSE_BUDGET_SIZES` like `2560x1600,6000x4000`,
/// `DIFFUSE_BUDGET_PRESETS`, `DIFFUSE_BUDGET_VRAM` the engine's budget in GiB (default 2, the
/// tests' engine), `DIFFUSE_BUDGET_MODES` of `old` (one submission, band steps per thread, refined
/// at full size) and `new`, alternating render by render so load hits them alike;
/// `DIFFUSE_BUDGET_KERNELS` prints the learnt GPU ns per thread of each kernel). Each line: GPU
/// time (timestamps), the band steps' share, wall time, dispatches, submissions and the longest
/// one's GPU time, the engine's memory after it, and the load average.
#[test]
#[ignore = "timings; run on an idle machine"]
fn diffuse_budget_timings() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let n: usize = std::env::var("DIFFUSE_BUDGET_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let vram: u64 = std::env::var("DIFFUSE_BUDGET_VRAM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(vram * GIB));
    let sizes = env_list("DIFFUSE_BUDGET_SIZES", "2560x1600,6000x4000");
    let presets = env_list("DIFFUSE_BUDGET_PRESETS", "");
    let modes = env_list("DIFFUSE_BUDGET_MODES", "old,new");
    let mode = |en: &mut Engine, m: &str| {
        let old = m == "old";
        set(&diffuse::NO_BAND_TILES, old);
        set(&diffuse::NO_STRIPES, old);
        en.set_submission_ms(if old {
            f64::INFINITY
        } else {
            crate::gpu::SUBMIT_MS
        });
    };
    for s in sizes {
        let Some((w, h)) = s
            .split_once('x')
            .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
        else {
            continue;
        };
        let img = scene(w, h, 0.03, 3);
        let src = en.upload(w, h, &img).unwrap();
        for p in Preset::ALL {
            let name = format!("{p:?}");
            if !presets.is_empty() && !presets.contains(&name) {
                continue;
            }
            let e = after_exposure(preset_edit(p));
            for draft in [true, false] {
                let focus = draft.then_some((ModuleKind::DIFFUSE, 0));
                let req = Request {
                    profile: en.has_timestamps(),
                    checkpoints: false,
                    precision: Precision::F16,
                    tiling: Tiling::Auto,
                    ..Request::new(Roi::full(w, h))
                };
                // Per mode: best GPU, its band steps, best wall, longest submission, info.
                let mut best: Vec<(f64, f64, f64, f64, String)> = modes
                    .iter()
                    .map(|_| (f64::MAX, 0.0, f64::MAX, 0.0, String::new()))
                    .collect();
                for _ in 0..n {
                    for (m, b) in modes.iter().zip(best.iter_mut()) {
                        mode(&mut en, m);
                        let g = graph_at(&e, w, h, focus);
                        let t = std::time::Instant::now();
                        let r = match en.render(&g, &src, &req) {
                            Ok(r) => r,
                            Err(Error::TooLarge { needs }) => {
                                b.4 = format!("too large: needs {} MiB", needs / MIB);
                                continue;
                            }
                            Err(e) => {
                                b.4 = format!("{e}");
                                // A lost device: wait, do not hammer it.
                                std::thread::sleep(std::time::Duration::from_secs(5));
                                continue;
                            }
                        };
                        let _ = en.read(&r).unwrap();
                        b.2 = b.2.min(t.elapsed().as_secs_f64() * 1e3);
                        let st = &r.stats;
                        if let Some(ms) = &st.gpu_ms {
                            let total: f64 = ms.iter().map(|m| m.1).sum();
                            if total < b.0 {
                                b.0 = total;
                                b.1 = ms
                                    .iter()
                                    .filter(|m| m.0.starts_with("diffuse_bands"))
                                    .map(|m| m.1)
                                    .sum();
                            }
                            let mut i = 0;
                            for k in &st.submissions {
                                let s: f64 = ms[i..i + k].iter().map(|m| m.1).sum();
                                b.3 = f64::max(b.3, s);
                                i += k;
                            }
                        }
                        let (kept, free, live) = en.memory();
                        b.4 = format!(
                            "{} dispatches, {} submissions, {} tile(s); kept {} MiB, free {} MiB, live {} MiB",
                            st.dispatches,
                            st.submissions.len(),
                            st.tiles,
                            kept / MIB,
                            free / MIB,
                            live / MIB
                        );
                        en.recycle(r);
                    }
                }
                for (m, b) in modes.iter().zip(&best) {
                    eprintln!(
                        "{w}x{h} {name} {} {m}: GPU {:.1} ms (bands {:.1}), wall {:.1} ms, longest submission {:.0} ms, {}, load {}",
                        if draft { "draft" } else { "refined" },
                        b.0,
                        b.1,
                        b.2,
                        b.3,
                        b.4,
                        load()
                    );
                }
            }
        }
        en.clear_cache();
    }
    mode(&mut en, "new");
    if std::env::var_os("DIFFUSE_BUDGET_KERNELS").is_some() {
        for l in [
            "diffuse_bands_v2",
            "diffuse_bands_wg_v1",
            "diffuse_level_rows_v1",
        ] {
            eprintln!("    {l}: {:.2} ns/thread", en.estimate_ms(l, 1_000_000));
        }
    }
}

/// Item 3: kept scratch stays within its cap, a turned-off diffuse lets its scratch go, and
/// a new source (or memory pressure) gives every free buffer back.
#[test]
fn scratch_is_let_go() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (640, 400);
    let img = scene(w, h, 0.03, 5);
    let src = en.upload(w, h, &img).unwrap();
    let e = preset_edit(Preset::Bloom);
    let req = Request::new(Roi::full(w, h));
    for focus in [Some((ModuleKind::DIFFUSE, 0)), None] {
        let g = graph_at(&e, w, h, focus);
        let r = en.render(&g, &src, &req).unwrap();
        en.recycle(r);
    }
    let (kept, free, _) = en.memory();
    assert!(kept > 0, "the draft and refined results are kept");
    assert!(kept <= en.budget().kept());
    assert!(free <= en.budget().free());
    // Diffuse off: its scratch goes.
    let off = after_exposure(crate::model::Edit::empty(Default::default()));
    let g = graph_at(&off, w, h, None);
    let r = en.render(&g, &src, &req).unwrap();
    en.recycle(r);
    assert_eq!(en.memory().0, 0);
    // A small cap keeps nothing.
    let mut small = Engine::new(
        &dev().unwrap().device,
        &dev().unwrap().queue,
        Budget::fixed(8 * MIB),
    );
    let src2 = small.upload(64, 40, &scene(64, 40, 0.03, 5)).unwrap();
    let g = graph_at(&e, 64, 40, None);
    let r = small
        .render(&g, &src2, &Request::new(Roi::full(64, 40)))
        .unwrap();
    small.recycle(r);
    assert!(small.memory().0 <= small.budget().kept());
    // Memory pressure (or a new photo): nothing held but what was handed out.
    en.trim_memory();
    let (kept, free, live) = en.memory();
    assert_eq!((kept, free), (0, 0));
    assert!(live <= src.buffer().size() + MIB, "{live}");
}

/// Item 1: a refined diffuse render goes in several short submissions, bit for bit the
/// picture of one submission.
#[test]
fn diffuse_in_short_submissions_is_bit_identical() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (1280, 800);
    let img = scene(w, h, 0.03, 9);
    let src = en.upload(w, h, &img).unwrap();
    for p in [Preset::Sharpen, Preset::Bloom, Preset::InpaintHighlights] {
        let g = graph_at(&after_exposure(preset_edit(p)), w, h, None);
        let req = Request {
            checkpoints: false,
            precision: Precision::F32,
            ..Request::new(Roi::full(w, h))
        };
        let mut out = Vec::new();
        for ms in [f64::INFINITY, 1.0, 0.0] {
            en.set_submission_ms(ms);
            let r = en.render(&g, &src, &req).unwrap();
            let k = r.stats.submissions.len();
            match ms {
                0.0 => assert_eq!(k, r.stats.dispatches),
                1.0 => assert!(k > 1 && k <= r.stats.dispatches),
                _ => assert_eq!(k, 1),
            }
            out.push(
                en.read(&r)
                    .unwrap()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
            );
            en.recycle(r);
        }
        assert!(out[0] == out[1] && out[0] == out[2], "{p:?}");
    }
    en.set_submission_ms(crate::gpu::SUBMIT_MS);
}

fn set(flag: &std::sync::atomic::AtomicBool, on: bool) {
    flag.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The peak working set (bytes) of a whole refined render of `p` at `w`×`h`, f16, and
/// whether it fits a working set of `working` bytes.
fn peak(p: Preset, w: u32, h: u32, working: u64) -> (u64, bool) {
    use crate::graph::tile::{Limits, Shape, plan};
    let g = graph_at(&after_exposure(preset_edit(p)), w, h, None);
    let shape = Shape {
        out: Roi::full(w, h),
        width: w,
        height: h,
        scale: 1.0,
        precision: Precision::F16,
        source_bpp: 16,
    };
    let limits = Limits {
        max_binding: u64::MAX,
        offset_align: 256,
        working,
    };
    match plan(&g, &shape, Tiling::Auto, &limits) {
        Ok(t) => (t.peak, true),
        Err(t) => (t.needs, false),
    }
}

/// 24 MP refined Bloom, Local contrast and Fill highlights fit the tests'
/// engine (1 GiB working set) by running in stripes; a 2560 preview still runs on whole
/// pictures.
#[test]
fn refined_24mp_fits_the_budget() {
    let _l = lock();
    for p in [
        Preset::Bloom,
        Preset::LocalContrast,
        Preset::InpaintHighlights,
    ] {
        set(&diffuse::NO_STRIPES, true);
        let (before, ok) = peak(p, 6000, 4000, GIB);
        set(&diffuse::NO_STRIPES, false);
        assert!(!ok, "{p:?} fitted on whole pictures");
        let (after, ok) = peak(p, 6000, 4000, GIB);
        let mut d = p.params();
        d.long = 6000.0;
        let pl = diffuse::plan(&d);
        eprintln!(
            "{p:?} 24 MP refined: {} → {} MiB (estimate {} MiB of scratch)",
            before / MIB,
            after / MIB,
            pl.scratch_bytes(6000.0) / MIB
        );
        assert!(pl.striped && !pl.half, "{p:?} at 6000");
        assert!(ok, "{p:?} needs {} MiB", after / MIB);
        d.long = 2560.0;
        assert!(!diffuse::plan(&d).striped, "{p:?} at 2560");
    }
}

/// The refined render in stripes against whole pictures at 24 MP, f32 (`DIFFUSE_BUDGET_PRESETS`
/// picks presets, `DIFFUSE_BUDGET_SIZE` like `6000x4000`). Heavy: the whole-picture render holds up
/// to 2.6 GiB of scratch. Prints ΔE 2000 and the GPU times of both.
#[test]
#[ignore = "heavy: 24 MP, several GiB of video memory"]
fn stripes_stay_close_at_24mp() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(14 * GIB));
    let only = std::env::var("DIFFUSE_BUDGET_PRESETS").ok();
    let (w, h) = std::env::var("DIFFUSE_BUDGET_SIZE")
        .ok()
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((6000, 4000));
    let img = scene(w, h, 0.02, 12);
    let src = en.upload(w, h, &img).unwrap();
    for p in [
        Preset::Bloom,
        Preset::LocalContrast,
        Preset::InpaintHighlights,
    ] {
        if only
            .as_ref()
            .is_some_and(|o| !o.contains(&format!("{p:?}")))
        {
            continue;
        }
        let req = Request {
            checkpoints: false,
            precision: Precision::F32,
            profile: en.has_timestamps(),
            ..Request::new(Roi::full(w, h))
        };
        let mut out = Vec::new();
        let mut ms = Vec::new();
        for whole in [true, false] {
            set(&diffuse::NO_STRIPES, whole);
            let g = graph_at(&preset_edit(p), w, h, None);
            let t = std::time::Instant::now();
            let r = en.render(&g, &src, &req);
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("{p:?} whole {whole}: {e}");
                    break;
                }
            };
            let px = en.read(&r).unwrap();
            ms.push((
                r.stats.gpu_total_ms().unwrap_or(f64::NAN),
                t.elapsed().as_secs_f64() * 1e3,
            ));
            out.push(px);
            en.recycle(r);
            en.clear_cache();
        }
        set(&diffuse::NO_STRIPES, false);
        if out.len() < 2 {
            continue;
        }
        let de = super::tests_diffuse_speed::delta_es(&out[1], &out[0]);
        let q = |f: f64| de[(de.len() as f64 * f) as usize];
        let over = de.iter().filter(|e| **e >= 1.0).count();
        eprintln!(
            "{w}x{h} {p:?}: stripes vs whole ΔE2000 p95 {:.4} p99 {:.4} max {:.4} ({over} ≥ 1); GPU whole {:.0} ms, stripes {:.0} ms; wall {:.0} / {:.0} ms; load {}",
            q(0.95),
            q(0.99),
            de[de.len() - 1],
            ms[0].0,
            ms[1].0,
            ms[0].1,
            ms[1].1,
            load()
        );
        assert!(q(0.99) <= 0.1, "{p:?}");
    }
}

/// Item 4: the band step per workgroup (taps shared in workgroup memory) gives the
/// pictures of the band step per thread bit for bit: every preset and the extremes, f16
/// and f32, refined and draft, odd sizes (partial workgroups).
#[test]
fn band_tiles_equal_band_threads() {
    use super::tests_diffuse::{edit, extremes, module, presets};
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (333, 217);
    let img = scene(w, h, 0.03, 4);
    let src = en.upload(w, h, &img).unwrap();
    let mut cases = presets();
    cases.extend(extremes().into_iter().map(|(n, d)| (n.to_string(), d)));
    let mut checked = 0usize;
    for (name, mut d) in cases {
        d.iterations = d.iterations.min(4);
        let e = after_exposure(edit(vec![module(ModuleKind::DIFFUSE, d.to_fields())]));
        for (focus, prec) in [
            (None, Precision::F32),
            (None, Precision::F16),
            (Some((ModuleKind::DIFFUSE, 0)), Precision::F16),
        ] {
            let g = graph_at(&e, w, h, focus);
            let req = Request {
                checkpoints: false,
                precision: prec,
                ..Request::new(Roi::full(w, h))
            };
            let mut out = Vec::new();
            for threads in [true, false] {
                set(&diffuse::NO_BAND_TILES, threads);
                let r = en.render(&g, &src, &req).unwrap();
                out.push(
                    en.read(&r)
                        .unwrap()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                );
                en.recycle(r);
            }
            set(&diffuse::NO_BAND_TILES, false);
            assert!(out[0] == out[1], "{name} {focus:?} {prec:?}");
            checked += out[0].len();
        }
    }
    eprintln!("{checked} values, 0 differ");
}

/// GPU milliseconds of non-local means per thread and per workgroup, 3000×2000,
/// search radius 4, alternating (the best of 7 each; prints).
#[test]
#[ignore = "timing run"]
fn nlm_timing() {
    use super::denoise::PLAIN_NLM;
    use super::tests_diffuse::{edit, module};
    use crate::model::{Fields, Value};
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (3000u32, 2000u32);
    let src = en.upload(w, h, &scene(w, h, 0.05, 9)).unwrap();
    let f = Fields::new().with("luminance", Value::Float(0.8));
    let e = edit(vec![module(ModuleKind::DENOISE, f)]);
    let g = graph_at(&e, w, h, None);
    let req = Request {
        checkpoints: false,
        precision: Precision::F16,
        profile: true,
        ..Request::new(Roi::full(w, h))
    };
    let mut best = [f64::MAX; 2];
    for _ in 0..7 {
        for (k, plain) in [true, false].into_iter().enumerate() {
            set(&PLAIN_NLM, plain);
            let r = en.render(&g, &src, &req).unwrap();
            let ms: f64 = r
                .stats
                .gpu_ms
                .iter()
                .flatten()
                .filter(|(l, _)| l.contains("nlm"))
                .map(|(_, m)| m)
                .sum();
            best[k] = best[k].min(ms);
            en.recycle(r);
        }
    }
    set(&PLAIN_NLM, false);
    eprintln!(
        "NLM 3000x2000 r4: per thread {:.1} ms, per workgroup {:.1} ms",
        best[0], best[1]
    );
}

/// Non-local means per workgroup (`NLM_WG`, a sliding window) equals the per-thread kernel bit for
/// bit: search radii 4 (full size), 3 (reduced) and 1 (draft), f32 and f16, whole and in tiles, odd
/// sizes.
#[test]
fn nlm_tiles_equal_nlm_threads() {
    use super::denoise::PLAIN_NLM;
    use super::tests_diffuse::{edit, module};
    use crate::model::{Fields, Value};
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let mut checked = 0usize;
    for (w, h) in [(333u32, 217u32), (64, 40)] {
        let img = scene(w, h, 0.05, 9);
        let src = en.upload(w, h, &img).unwrap();
        let f = Fields::new()
            .with("luminance", Value::Float(0.8))
            .with("colour", Value::Float(0.5))
            .with("detail", Value::Float(0.3));
        let e = after_exposure(edit(vec![module(ModuleKind::DENOISE, f)]));
        for (focus, scale, prec, tiling) in [
            (None, 1.0, Precision::F32, Tiling::Auto),
            (None, 0.5, Precision::F16, Tiling::Auto),
            (
                Some((ModuleKind::DENOISE, 0)),
                1.0,
                Precision::F16,
                Tiling::Auto,
            ),
            (None, 1.0, Precision::F32, Tiling::Side(96)),
        ] {
            let g = graph_at(&e, w, h, focus);
            let req = Request {
                checkpoints: false,
                precision: prec,
                scale,
                tiling,
                ..Request::new(Roi::full(w, h))
            };
            let mut out = Vec::new();
            for plain in [true, false] {
                set(&PLAIN_NLM, plain);
                let r = en.render(&g, &src, &req).unwrap();
                out.push(
                    en.read(&r)
                        .unwrap()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                );
                en.recycle(r);
            }
            set(&PLAIN_NLM, false);
            let same = out[0].iter().zip(&out[1]).filter(|(a, b)| a == b).count();
            assert_eq!(
                same,
                out[0].len(),
                "{w}x{h} {focus:?} scale {scale} {prec:?} {tiling:?}: {} values differ",
                out[0].len() - same
            );
            checked += same;
        }
    }
    eprintln!("{checked} values, 0 differ");
}
