//! Develop's performance gate: a bench for every budget, run
//! through cargo, with p50/p95 of the best of N rounds, GPU timestamps beside the wall
//! clock, the load average and the adapter recorded.
//!
//! ```text
//! cargo bench -p specular-develop --profile dev --bench develop        # run and report
//! DV_BENCH_GATE=1 cargo bench ... --bench develop                      # also gate (+10 %)
//! SPECULAR_BLESS=1 cargo bench ... --bench develop                     # bless (idle only)
//! SPECULAR_BLESS=provisional cargo bench ... --bench develop           # bless under load
//! ```
//!
//! Settings (environment):
//! - `DV_BENCH_ONLY=slider/point,export/24`: only cases whose names contain one of these.
//! - `DV_BENCH_PROFILE=1`: GPU milliseconds per dispatch label of a profiled render (the
//!   eight largest; `all` for every label; `seq` also every dispatch in order) and an
//!   export's phases.
//! - `DV_BENCH_ROUNDS=3`: rounds per case; the round with the lowest p50 is kept.
//! - `DV_BENCH_SCALE=1`: multiplies the samples per round (0.25 for a quick look).
//! - `DV_BENCH_SIZES=24,45,100`: the synthetic raws' megapixels (export).
//! - `SPECULAR_BENCH_ADAPTER=gpu|software|auto`: the hardware adapter (default), the
//!   software one (lavapipe, WARP: numbers reported apart, never held to the integrated-GPU
//!   budgets, a smaller default case set), or what the PathSelector picks (`Task::Develop`).
//! - `DV_BENCH_MARKDOWN=FILE`: also write the table as Markdown (CI's step summary).
//! - `DV_BENCH_CLEAN=1`: delete the generated raws at the end.
//! - `DV_BENCH_THUMB_RAW_LONG=512`: raw thumbnails developed at this long side, then reduced
//!   to 512 px (`Thumbnailer::raw_long`; default the app's, `thumb::RAW_LONG` = 1024).
//! - `DV_BENCH_INJECT=1.2`: gate this run as if every time were 20 % longer (checks that the
//!   gate fails; never set in CI). `DV_BENCH_FORCE=1`: judge even when overloaded.
//!
//! Results go to `$CARGO_TARGET_DIR/develop-bench/results-<gpu|software>.json`; the
//! baselines are `benches/develop-baseline.json` (hardware) and
//! `benches/develop-baseline-software.json`. The gate is skipped (said so, not passed) when
//! the machine is overloaded, when there is no hardware GPU, or when the baseline was made
//! on another adapter or profile. Exit status: 0 pass or skip, 1 fail.
//!
//! Inputs are generated (`inputs.rs`), never committed; the AI networks run with random
//! weights at their real sizes (`ai.rs`).

#[allow(dead_code, clippy::all)]
#[path = "../../../formats/specular-raw/tests/common/mod.rs"]
mod common;

mod ai;
mod cases;
// Shared with tests/perf_gate.rs, which uses the parts the bench does not.
#[allow(dead_code)]
mod gate;
mod inputs;

use std::collections::BTreeMap;
use std::path::PathBuf;

use cases::Bench;
use gate::{Env, Results, Verdict};
use inputs::{Mosaic, Raw};
use specular_core::paths::{GpuClass, PathSelector};
use specular_develop::paths::{self as develop_paths, Adapter};

fn main() {
    let adapter = std::env::var("SPECULAR_BENCH_ADAPTER").unwrap_or_default();
    let gate_on =
        std::env::var_os("DV_BENCH_GATE").is_some() || std::env::args().any(|a| a == "--gate");
    let bless = std::env::var("SPECULAR_BLESS").ok();
    let load_start = gate::load();
    let cores = std::thread::available_parallelism().map_or(1, usize::from);

    let pick = match adapter.as_str() {
        "software" => Some(true),
        "gpu" | "" => Some(false),
        "auto" => None,
        other => {
            eprintln!("SPECULAR_BENCH_ADAPTER={other}: use gpu, software or auto");
            std::process::exit(2);
        }
    };
    // Task::Develop: the hardware path is there when a hardware device opens.
    let sel = PathSelector::new();
    let hardware = open_gpu();
    develop_paths::register(&sel, hardware.as_ref().ok().map(|h| h.4));
    let opened = match pick {
        Some(true) => open_software(),
        Some(false) => hardware,
        None => {
            let a = develop_paths::choose(&sel, 24_000_000);
            eprintln!("PathSelector (Task::Develop) picks the {a:?} adapter for 24 MP");
            if a == Adapter::Software {
                drop(hardware);
                open_software()
            } else {
                hardware
            }
        }
    };
    let (device, queue, name, software, _) = match opened {
        Ok(d) => d,
        Err(e) => {
            say_skip(&format!("no GPU adapter ({e})"));
            return;
        }
    };
    let timestamps = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let class = if software { "software" } else { "gpu" };
    eprintln!(
        "adapter: {name} [{class}], timestamps {timestamps}, profile {}, load {:.1} {:.1} {:.1} on {cores} cores",
        profile(),
        load_start[0],
        load_start[1],
        load_start[2]
    );

    let only: Vec<String> = match std::env::var("DV_BENCH_ONLY") {
        Ok(s) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        // The software adapter runs the interactive rows and the thumbnail by default.
        Err(_) if software => [
            "slider/point/exposure",
            "slider/neighbourhood/tone",
            "undo",
            "pan-1to1",
            "mask/",
            "thumbnail/512/24mp-bayer",
            "open/next-prefetched/24mp-bayer",
        ]
        .map(String::from)
        .to_vec(),
        Err(_) => Vec::new(),
    };
    let mut b = Bench {
        device,
        queue,
        timestamps,
        software,
        rounds: env_num("DV_BENCH_ROUNDS", 3.0) as usize,
        scale: env_num("DV_BENCH_SCALE", 1.0),
        only,
        results: BTreeMap::new(),
    };
    let sizes: Vec<u32> = std::env::var("DV_BENCH_SIZES")
        .unwrap_or_else(|_| "24,45,100".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let bayer24 = Raw::new(24, Mosaic::Bayer);
    let xtrans24 = Raw::new(24, Mosaic::XTrans);
    let small = [bayer24, xtrans24];
    let mut big: Vec<Raw> = sizes
        .iter()
        .map(|&mp| Raw::new(mp, Mosaic::Bayer))
        .collect();
    big.push(xtrans24);

    if b.wants_prefix("slider/") || b.wants_prefix("drag/") || b.wants("undo") || b.wants("scopes")
    {
        let bytes = std::fs::read(bayer24.path()).expect("read the 24 MP raw");
        let input = specular_develop::preview::decode_raw(&bytes).expect("decode");
        cases::sliders(&mut b, &input);
        cases::drags(&mut b, &input);
    }
    cases::open(&mut b, &small);
    cases::tiles(&mut b);
    cases::masks(&mut b, &bayer24);
    cases::retouch(&mut b);
    cases::diffuse(&mut b);
    ai::networks(&mut b);
    cases::thumbnails(&mut b, &small);
    cases::exports(&mut b, &big);

    let results = Results {
        schema: gate::SCHEMA,
        provisional: false,
        note: String::new(),
        env: Env {
            adapter: name,
            class: class.into(),
            profile: profile().into(),
            timestamps,
            load_start,
            load_end: gate::load(),
            cores,
            unix_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        },
        cases: b.results,
    };
    let out = inputs::out_dir().join(format!("results-{class}.json"));
    results.write(&out).expect("write the results");
    eprintln!("results: {}", out.display());
    let table = gate::table(&results);
    if let Ok(md) = std::env::var("DV_BENCH_MARKDOWN") {
        let _ = std::fs::write(md, &table);
    }
    print!("{table}");

    // The PathSelector learns this run's numbers (one task, two adapters).
    let adapter = if software {
        Adapter::Software
    } else {
        Adapter::Hardware
    };
    for (mp, ms) in results_mpx(&results) {
        let px = (f64::from(mp) * 1e6) as u64;
        let before = develop_paths::estimate_ms(&sel, adapter, px);
        develop_paths::record(&sel, adapter, px, f64::from(ms));
        eprintln!(
            "PathSelector Task::Develop {adapter:?} at {mp} MP: predicted {before:.0} ms, measured {ms:.0} ms"
        );
    }

    let base_path = gate::baseline_path(software);
    if let Some(mode) = bless {
        bless_baseline(&results, &base_path, &mode);
    } else if gate_on {
        // `DV_BENCH_FORCE=1` judges under load too (said so): checking the gate on a busy machine.
        let busy = gate::overloaded().filter(|_| std::env::var_os("DV_BENCH_FORCE").is_none());
        let verdict = match (busy, Results::read(&base_path)) {
            (Some(why), _) => Verdict::Skip(why),
            (None, Err(e)) => Verdict::Skip(format!("no baseline ({e})")),
            (None, Ok(base)) => {
                // Checking the gate itself: `DV_BENCH_INJECT=1.2` gates a run made 20 % slower.
                let f = env_num("DV_BENCH_INJECT", 1.0);
                gate::compare(&base, &results.scaled(f))
            }
        };
        match verdict {
            Verdict::Pass { compared, notes } => {
                for n in notes {
                    eprintln!("note: {n}");
                }
                eprintln!(
                    "GATE PASSED: {compared} cases within +{:.0} %",
                    gate::TOLERANCE * 100.0
                );
            }
            Verdict::Skip(why) => say_skip(&why),
            Verdict::Fail(lines) => {
                for l in &lines {
                    eprintln!("FAIL {l}");
                }
                eprintln!("GATE FAILED: {} over the +10 % rule", lines.len());
                std::process::exit(1);
            }
        }
    }
    if std::env::var_os("DV_BENCH_CLEAN").is_some() {
        inputs::clean();
    }
}

/// Export wall per megapixel, for the PathSelector.
fn results_mpx(r: &Results) -> Vec<(f32, f32)> {
    r.cases
        .iter()
        .filter(|(k, _)| k.starts_with("export/") && !k.contains("batch") && k.ends_with("bayer"))
        .filter_map(|(k, c)| {
            let mp: f32 = k
                .trim_start_matches("export/")
                .split("mp")
                .next()?
                .parse()
                .ok()?;
            Some((mp, c.wall.p50 as f32))
        })
        .collect()
}

fn bless_baseline(results: &Results, path: &std::path::Path, mode: &str) {
    let overloaded = gate::overloaded();
    let provisional = mode == "provisional";
    if let Some(why) = &overloaded
        && !provisional
    {
        eprintln!(
            "not blessed: {why}. Bless from an idle run, or with SPECULAR_BLESS=provisional \
             (keeps each case's best run and marks the baseline provisional)."
        );
        std::process::exit(1);
    }
    let mut r = if provisional {
        match Results::read(path) {
            Ok(old) if old.provisional => results.best_with(&old),
            _ => results.clone(),
        }
    } else {
        results.clone()
    };
    r.provisional = provisional && overloaded.is_some();
    r.note = if r.provisional {
        format!(
            "PROVISIONAL, not from an idle run: {}. Each case keeps its best run; re-bless when idle",
            overloaded.unwrap_or_default()
        )
    } else {
        "blessed from an idle run".into()
    };
    r.write(path).expect("write the baseline");
    eprintln!(
        "blessed {} ({} cases){}",
        path.display(),
        r.cases.len(),
        if r.provisional { ", provisional" } else { "" }
    );
}

fn say_skip(why: &str) {
    eprintln!("GATE SKIPPED (not passed): {why}");
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        println!("::warning title=Develop performance gate skipped::{why}");
    }
}

fn env_num(name: &str, d: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f64| *v > 0.0)
        .unwrap_or(d)
}

/// The cargo profile this bench was built with.
fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    }
}

type Opened = (wgpu::Device, wgpu::Queue, String, bool, GpuClass);

fn open_gpu() -> Result<Opened, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::PRIMARY.with_env();
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu {
        return Err(format!("only a software adapter ({})", info.name));
    }
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    let features = adapter.features()
        & (wgpu::Features::TIMESTAMP_QUERY | specular_infer::gpu::Features::wanted());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("develop bench"),
        required_features: features,
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("no device: {e}"))?;
    let class = if info.device_type == wgpu::DeviceType::DiscreteGpu {
        GpuClass::Discrete
    } else {
        GpuClass::Integrated
    };
    Ok((
        device,
        queue,
        format!("{} ({:?})", info.name, info.backend),
        false,
        class,
    ))
}

fn open_software() -> Result<Opened, String> {
    let (device, queue) = specular_develop::preview::open_software_device()?;
    Ok((
        device,
        queue,
        "software adapter".into(),
        true,
        GpuClass::Software,
    ))
}

#[allow(dead_code)]
fn unused(_: PathBuf) {}
