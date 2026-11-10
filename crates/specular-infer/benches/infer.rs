//! GPU vs CPU throughput of the inference runtime: gemm, conv2d and a ViT-Ti/16 at 224².
//!
//! `cargo bench -p specular-infer [-- <filter>] [-- --profile] [-- --cpu | --gpu]`
//! (SPECULAR_BENCH_THREADS sets the CPU threads, default all; SPECULAR_BENCH_SPIN_US how long
//! idle CPU workers spin before sleeping; `--cpu` / `--gpu` time one engine only: on an
//! integrated GPU the CPU's heat lowers the GPU's clocks). GPU times are whole runs after a
//! warm-up: feed upload, every dispatch, readback of the outputs. `--profile` also times
//! every dispatch of the ViT and the CNN with GPU timestamps and sums them per op. `--idle`
//! instead measures the CPU time of the CPU engine: per run back to back and 250 ms apart,
//! and of the idle workers afterwards. Prints the load averages: numbers taken under load
//! are not comparable with idle ones.

use std::time::Instant;

use specular_infer::gpu::{Context, Features, GpuGraph};
use specular_infer::graph::{Conv2d, MatMul};
use specular_infer::models::{Block, CnnConfig, RandomWeights, VitConfig, cnn, image_batch, vit};
use specular_infer::pool::Pool;
use specular_infer::tensor::Rng;
use specular_infer::{Act, DType, Feed, Graph, Tensor, cpu};

fn median_ms(reps: usize, mut f: impl FnMut()) -> f64 {
    // Warm up (clocks ramp, caches fill) for at least 100 ms.
    let s = Instant::now();
    f();
    while s.elapsed().as_secs_f64() < 0.1 {
        f();
    }
    let mut t: Vec<f64> = (0..reps)
        .map(|_| {
            let s = Instant::now();
            f();
            s.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    t.sort_by(f64::total_cmp);
    t[t.len() / 2]
}

struct Case {
    name: String,
    graph: Graph,
    input: Tensor,
    /// Multiply-adds per run, for GFLOP/s (2 flops each).
    macs: f64,
    cpu_reps: usize,
}

fn gemm(m: usize, k: usize, n: usize, f16: bool) -> Case {
    let mut rng = Rng::new(1);
    let mut g = Graph::new();
    let a = g.input(&[m, k], DType::F32).unwrap();
    let mut w = rng.tensor(&[n, k], 0.05);
    if f16 {
        w = w.to_f16();
    }
    let w = g.constant(w).unwrap();
    let y = g
        .matmul(
            a,
            w,
            None,
            MatMul {
                trans_b: true,
                act: Act::Identity,
            },
        )
        .unwrap();
    g.output(y).unwrap();
    Case {
        name: format!("gemm {m}x{k}x{n}{}", if f16 { " f16 w" } else { "" }),
        graph: g,
        input: rng.tensor(&[m, k], 1.0),
        macs: (m * k * n) as f64,
        cpu_reps: 7,
    }
}

fn conv(c: usize, hw: usize, o: usize, k: usize, groups: usize, stride: usize) -> Case {
    let mut rng = Rng::new(2);
    let mut g = Graph::new();
    let x = g.input(&[1, c, hw, hw], DType::F32).unwrap();
    let w = g.constant(rng.tensor(&[o, c / groups, k, k], 0.1)).unwrap();
    let b = g.constant(rng.tensor(&[o], 0.1)).unwrap();
    let y = g
        .conv2d(
            x,
            w,
            Some(b),
            Conv2d::same(k, stride).groups(groups).act(Act::Relu),
        )
        .unwrap();
    g.output(y).unwrap();
    let kind = if groups == c && groups > 1 {
        "depthwise "
    } else if k == 1 {
        "pointwise "
    } else {
        ""
    };
    Case {
        name: format!(
            "conv2d {kind}{c}x{hw}² → {o}, {k}×{k}{}",
            if stride > 1 {
                format!(" /{stride}")
            } else {
                String::new()
            }
        ),
        graph: g,
        input: rng.tensor(&[1, c, hw, hw], 1.0),
        macs: (o * (c / groups) * k * k * (hw / stride).pow(2)) as f64,
        cpu_reps: 7,
    }
}

fn vit_tiny(f16: bool) -> Case {
    let cfg = VitConfig::tiny();
    let mut g = Graph::new();
    let x = image_batch(&mut g, 1, cfg.image).unwrap();
    let y = vit(&mut g, x, &cfg, &mut RandomWeights { seed: 1, f16 }).unwrap();
    g.output(y).unwrap();
    Case {
        name: format!("ViT-Ti/16 224²{}", if f16 { " f16 weights" } else { "" }),
        graph: g,
        input: Rng::new(3).tensor(&[1, 3, cfg.image, cfg.image], 1.0),
        // ~1.26 GMACs for ViT-Ti/16 at 224² (+ the 512-d head).
        macs: 1.26e9,
        cpu_reps: 7,
    }
}

/// MobileNetV2 (width 1.0) at 224²: 0.30 GMACs.
fn mobilenet_v2(f16: bool) -> Case {
    let b = |expand, out, stride| Block {
        expand,
        out,
        stride,
    };
    let mut blocks = Vec::new();
    for (t, c, n, s) in [
        (1, 16, 1, 1),
        (6, 24, 2, 2),
        (6, 32, 3, 2),
        (6, 64, 4, 2),
        (6, 96, 3, 1),
        (6, 160, 3, 2),
        (6, 320, 1, 1),
    ] {
        for i in 0..n {
            blocks.push(b(t, c, if i == 0 { s } else { 1 }));
        }
    }
    let cfg = CnnConfig {
        image: 224,
        stem: 32,
        blocks,
        last: 1280,
        classes: 1000,
        bn_eps: 1e-5,
    };
    let mut g = Graph::new();
    let x = image_batch(&mut g, 1, cfg.image).unwrap();
    let y = cnn(&mut g, x, &cfg, &mut RandomWeights { seed: 1, f16 }).unwrap();
    g.output(y).unwrap();
    Case {
        name: format!("MobileNetV2 224²{}", if f16 { " f16 weights" } else { "" }),
        graph: g,
        input: Rng::new(3).tensor(&[1, 3, cfg.image, cfg.image], 1.0),
        macs: 0.30e9,
        cpu_reps: 7,
    }
}

/// The adapter specular-gfx picks, opened again with the optional features the inference
/// kernels use (and timestamps for `--profile`).
fn device(gpu: &specular_gfx::Gpu) -> (wgpu::Device, wgpu::Queue) {
    let features = gpu.adapter.features() & Features::wanted();
    pollster::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("infer bench"),
        required_features: features,
        required_limits: gpu.limits.clone(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .expect("device")
}

/// Per-dispatch GPU times of a run, summed per op kind (the label up to its shape).
fn profile(ctx: &Context, c: &Case) {
    let feeds = [Feed::Tensor(&c.input)];
    let mut gg = GpuGraph::new(ctx, &c.graph).unwrap();
    gg.run(&feeds).unwrap();
    // The median of 7 profiles, per step.
    let runs: Vec<Vec<(String, f64)>> = (0..7).map(|_| gg.profile(&feeds).unwrap()).collect();
    let mut steps: Vec<(String, f64)> = runs[0].clone();
    for (i, st) in steps.iter_mut().enumerate() {
        let mut t: Vec<f64> = runs.iter().map(|r| r[i].1).collect();
        t.sort_by(f64::total_cmp);
        st.1 = t[t.len() / 2];
    }
    let total: f64 = steps.iter().map(|s| s.1).sum();
    println!(
        "  {}: {} dispatches, {total:.3} ms of GPU time",
        c.name,
        steps.len()
    );
    let mut per: Vec<(String, usize, f64)> = Vec::new();
    for (label, ms) in &steps {
        let kind = label.split(" [").next().unwrap_or(label).to_owned();
        match per.iter_mut().find(|p| p.0 == kind) {
            Some(p) => {
                p.1 += 1;
                p.2 += ms;
            }
            None => per.push((kind, 1, *ms)),
        }
    }
    per.sort_by(|a, b| b.2.total_cmp(&a.2));
    for (kind, n, ms) in per {
        println!(
            "    {kind:<28} {n:>4}× {ms:>8.3} ms {:>5.1}%",
            100.0 * ms / total
        );
    }
    if std::env::var_os("SPECULAR_BENCH_STEPS").is_some() {
        for (label, ms) in &steps {
            println!("      {label:<48} {ms:>8.4} ms");
        }
    }
}

/// CPU time (ns) the process's threads named `ai-infer-*` (the CPU pool) and all its threads
/// have run, from /proc/self/task/*/schedstat (Linux; 0 elsewhere).
fn cpu_ns() -> (u64, u64) {
    let (mut pool, mut all) = (0, 0);
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return (0, 0);
    };
    for t in tasks.flatten() {
        let p = t.path();
        let ns = std::fs::read_to_string(p.join("schedstat"))
            .ok()
            .and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok())
            .unwrap_or(0);
        all += ns;
        if std::fs::read_to_string(p.join("comm")).is_ok_and(|c| c.starts_with("ai-infer-")) {
            pool += ns;
        }
    }
    (pool, all)
}

/// User and system CPU time (ms) of the process, from /proc/self/stat (Linux, 100 Hz ticks
/// assumed; 0 elsewhere).
fn user_sys_ms() -> (f64, f64) {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // Fields after the parenthesised command name; utime and stime are fields 14 and 15.
    let rest = s.rsplit_once(')').map_or("", |r| r.1);
    let f: Vec<f64> = rest
        .split_whitespace()
        .skip(11)
        .take(2)
        .map(|x| x.parse::<f64>().unwrap_or(0.0) * 10.0)
        .collect();
    (
        f.first().copied().unwrap_or(0.0),
        f.get(1).copied().unwrap_or(0.0),
    )
}

/// `--idle`: what the CPU pool costs beyond the work. Back-to-back runs (wall and CPU time
/// per run), runs 250 ms apart (as an app classifying one image at a time), then the pool's
/// CPU time over 3 s after the last run.
fn idle_cost(c: &Case, threads: usize) {
    let feeds = [Feed::Tensor(&c.input)];
    let ms = |ns: u64| ns as f64 / 1e6;
    // Warm-up: the pool starts, weights are converted.
    for _ in 0..3 {
        cpu::run(&c.graph, &feeds, threads).unwrap();
    }
    let reps = 30;
    let (p0, a0) = cpu_ns();
    let (u0, s0) = user_sys_ms();
    let mut wall: Vec<f64> = (0..reps)
        .map(|_| {
            let s = Instant::now();
            cpu::run(&c.graph, &feeds, threads).unwrap();
            s.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let (p1, a1) = cpu_ns();
    let (u1, s1) = user_sys_ms();
    wall.sort_by(f64::total_cmp);
    println!(
        "  {}: back-to-back median {:.2} ms wall, {:.1} ms CPU per run ({:.1} ms on the pool; \
         user {:.1}, system {:.1})",
        c.name,
        wall[reps / 2],
        ms(a1 - a0) / reps as f64,
        ms(p1 - p0) / reps as f64,
        (u1 - u0) / reps as f64,
        (s1 - s0) / reps as f64,
    );
    let spaced = 8;
    let mut wall: Vec<f64> = Vec::new();
    let (p0, a0) = cpu_ns();
    for _ in 0..spaced {
        let s = Instant::now();
        cpu::run(&c.graph, &feeds, threads).unwrap();
        wall.push(s.elapsed().as_secs_f64() * 1e3);
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let (p1, a1) = cpu_ns();
    wall.sort_by(f64::total_cmp);
    println!(
        "  {}: 250 ms apart median {:.2} ms wall, {:.1} ms CPU per run and gap ({:.1} ms on the pool)",
        c.name,
        wall[spaced / 2],
        ms(a1 - a0) / spaced as f64,
        ms(p1 - p0) / spaced as f64,
    );
    cpu::run(&c.graph, &feeds, threads).unwrap();
    let (p0, _) = cpu_ns();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let (p1, _) = cpu_ns();
    std::thread::sleep(std::time::Duration::from_millis(2900));
    let (p2, _) = cpu_ns();
    println!(
        "  {}: idle pool after a run: {:.2} ms CPU in the first 100 ms, {:.2} ms in the next 2.9 s",
        c.name,
        ms(p1 - p0),
        ms(p2 - p1),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list") {
        return;
    }
    // SPECULAR_BENCH_SPIN_US: how long idle CPU workers spin before sleeping.
    if let Some(us) = std::env::var("SPECULAR_BENCH_SPIN_US")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        let n = std::thread::available_parallelism().map_or(1, |n| n.get());
        Pool::init_global(n, std::time::Duration::from_micros(us));
        println!("CPU workers spin {us} µs before sleeping");
    }
    if args.iter().any(|a| a == "--idle") {
        let threads = std::env::var("SPECULAR_BENCH_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
        let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
        println!(
            "load average {}, CPU {threads} threads; CPU pool cost:",
            load.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        );
        for c in [mobilenet_v2(false), vit_tiny(false)] {
            idle_cost(&c, threads);
        }
        return;
    }
    let want_profile = args.iter().any(|a| a == "--profile");
    let want_cpu = !args.iter().any(|a| a == "--gpu");
    let want_gpu = !args.iter().any(|a| a == "--cpu");
    let filter: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let threads = std::env::var("SPECULAR_BENCH_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
    println!(
        "load average {} (machine shared: numbers are indicative), CPU {threads} threads",
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );
    let gpu = if want_gpu || want_profile {
        specular_gfx::Gpu::new().ok()
    } else {
        None
    };
    let dev = gpu.as_ref().map(device);
    let ctx = gpu.as_ref().zip(dev.as_ref()).map(|(g, (d, q))| {
        let c = Context::new(d, q);
        println!(
            "GPU {} ({:?}), {:?}",
            g.info.name,
            g.info.backend,
            c.features()
        );
        c
    });
    let cases = [
        gemm(512, 512, 512, false),
        gemm(1024, 1024, 1024, false),
        gemm(1024, 1024, 1024, true),
        gemm(197, 192, 768, false),
        conv(64, 56, 64, 3, 1, 1),
        conv(32, 112, 32, 3, 32, 1),
        conv(96, 56, 24, 1, 1, 1),
        conv(3, 224, 32, 3, 1, 1),
        // MobileNetV2's stem.
        conv(3, 224, 32, 3, 1, 2),
        vit_tiny(false),
        vit_tiny(true),
        mobilenet_v2(false),
        mobilenet_v2(true),
    ];
    let cases: Vec<Case> = cases
        .into_iter()
        .filter(|c| filter.is_empty() || filter.iter().any(|f| c.name.contains(f.as_str())))
        .collect();
    println!(
        "{:<36} {:>10} {:>9} {:>10} {:>9} {:>8}",
        "case", "cpu ms", "GFLOP/s", "gpu ms", "GFLOP/s", "speedup"
    );
    for c in &cases {
        let feeds = [Feed::Tensor(&c.input)];
        let cpu_ms = if want_cpu {
            median_ms(c.cpu_reps, || {
                cpu::run(&c.graph, &feeds, threads).unwrap();
            })
        } else {
            f64::NAN
        };
        let gpu_ms = ctx.as_ref().filter(|_| want_gpu).map(|ctx| {
            let mut gg = GpuGraph::new(ctx, &c.graph).unwrap();
            median_ms(25, || {
                gg.run(&feeds).unwrap();
            })
        });
        let gf = |ms: f64| 2.0 * c.macs / (ms * 1e6);
        match gpu_ms {
            Some(g) => println!(
                "{:<36} {:>10.2} {:>9.1} {:>10.2} {:>9.1} {:>7.1}×",
                c.name,
                cpu_ms,
                gf(cpu_ms),
                g,
                gf(g),
                cpu_ms / g
            ),
            None => println!(
                "{:<36} {:>10.2} {:>9.1} {:>10} {:>9} {:>8}",
                c.name,
                cpu_ms,
                gf(cpu_ms),
                "-",
                "-",
                "-"
            ),
        }
    }
    if want_profile && let Some(ctx) = &ctx {
        if !ctx.features().timestamps {
            println!("no timestamp queries on this device: no profile");
            return;
        }
        println!("GPU time per op (median of 7 profiled runs, one pass per dispatch):");
        for c in &cases {
            if c.name.starts_with("ViT") || c.name.starts_with("MobileNet") {
                profile(ctx, c);
            }
        }
    }
}
