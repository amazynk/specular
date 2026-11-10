//! The `Task::Inference` hook into the PathSelector (DESIGN.md §12.3.1).
//!
//! [`self_test`] runs one small graph that goes through every GPU kernel (each variant the
//! models use: f16 and f32 weights, the three convolution lowerings, packed causal
//! attention, image preprocessing) on the GPU and on the CPU reference and compares them.
//! [`calibrate`] runs it when the machine is idle: a failure or an error **blocks** the GPU
//! path for inference; a pass only marks it available. Trust is never granted here: a
//! model's GPU path is trusted by `specular-ai`'s `Dispatch` when its own outputs match the
//! CPU on real input. Calibration then times a ViT-Ti/16 at 224² on both engines so the
//! selector starts from measured costs rather than priors.

use std::time::Instant;

use specular_core::paths::{Engine, Task, Trust, selector};

use crate::gpu::{Context, GpuGraph};
use crate::graph::{Act, Conv2d, Crop, Graph, MatMul, Pool2d, PoolKind, Preprocess, ResizeMode};
use crate::models::{RandomWeights, VitConfig, vit};
use crate::tensor::{Rng, Tensor, compare};
use crate::{DType, Error, Feed};

/// Tolerance of the self-test: |gpu − cpu| ≤ ATOL + RTOL·|cpu| for every element.
pub const ATOL: f32 = 2e-3;
pub const RTOL: f32 = 2e-3;

/// Side of the image the calibration times (ViT-Ti/16 at 224²); its key, `(SIDE², 0)`, is
/// where the inference timings are reported.
pub const SIDE: u32 = 224;
/// [`SIDE`]².
pub const PIXELS: u64 = SIDE as u64 * SIDE as u64;

/// Whether the calibration still has to run: the GPU path is not blocked and has fewer than
/// two timed runs at [`PIXELS`].
pub fn missing() -> bool {
    let sel = selector();
    let gpu = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
    let key = sel.calibration(Task::Inference, gpu, PIXELS, 0).key;
    sel.trust(Task::Inference, gpu) != Trust::Blocked && sel.samples(Task::Inference, gpu, key) < 2
}

/// The self-test graph and its feeds: a 61×47 RGBA image and a `[2, 5]` tensor.
pub fn coverage() -> Result<(Graph, Vec<u8>, Tensor), Error> {
    let mut rng = Rng::new(7);
    let mut g = Graph::new();
    let img = g.image_input()?;
    let extra = g.input(&[2, 5], DType::F32)?;
    let x = g.preprocess(
        img,
        Preprocess {
            width: 24,
            height: 20,
            crop: Crop::Center,
            antialias: true,
            mean: [0.48, 0.46, 0.41],
            std: [0.27, 0.26, 0.28],
        },
    )?;
    // General conv (implicit GEMM), depthwise (direct), pointwise with f16 weights (GEMM).
    let w1 = g.constant(rng.tensor(&[8, 3, 3, 3], 0.4))?;
    let b1 = g.constant(rng.tensor(&[8], 0.1))?;
    let x = g.conv2d(x, w1, Some(b1), Conv2d::same(3, 2).act(Act::Silu))?;
    let w2 = g.constant(rng.tensor(&[8, 1, 3, 3], 0.4))?;
    let p = Conv2d {
        dilation: [2, 1],
        pad: [2, 1, 2, 1],
        ..Conv2d::default().groups(8).act(Act::HardSwish)
    };
    let x = g.conv2d(x, w2, None, p)?;
    let w3 = g.constant(rng.tensor(&[12, 8, 1, 1], 0.4).to_f16())?;
    let x = g.conv2d(x, w3, None, Conv2d::default().act(Act::Relu6))?;
    let x = g.pool2d(
        x,
        Pool2d {
            kind: PoolKind::Max,
            kernel: [2, 2],
            stride: [2, 2],
            pad: [0; 4],
        },
    )?;
    let x = g.resize(x, 6, 4, ResizeMode::Bilinear)?;
    // Tokens: [1, 12, 24] → layer norm, MLP, causal attention.
    let t = g.reshape(x, &[1, 12, 24])?;
    let t = g.transpose(t, &[0, 2, 1])?;
    let gamma = g.constant(rng.tensor(&[12], 0.5))?;
    let beta = g.constant(rng.tensor(&[12], 0.1))?;
    let t = g.layer_norm(t, gamma, beta, 1e-5)?;
    let wq = g.constant(rng.tensor(&[36, 12], 0.4).to_f16())?;
    let bq = g.constant(rng.tensor(&[36], 0.1))?;
    let qkv = g.matmul(
        t,
        wq,
        Some(bq),
        MatMul {
            trans_b: true,
            act: Act::Gelu,
        },
    )?;
    let a = g.attention_packed(qkv, 3, None, true)?;
    let s = g.softmax(a)?;
    let first = g.slice(s, &[0, 0, 0], &[1, 24, 12], &[1, 2, 3])?;
    let flat = g.reshape(first, &[12, 4])?;
    let ids = g.constant(Tensor::f32(&[2, 3], vec![0.0, 3.0, 11.0, 5.5, 40.0, 7.0]))?;
    let rows = g.gather(flat, ids)?;
    let rows = g.reshape(rows, &[2, 12])?;
    let both = g.concat(&[rows, extra], 1)?;
    let half = g.unary(both, Act::Tanh, 0.5, 0.25)?;
    let m = g.binary(crate::graph::BinaryOp::Mul, half, both)?;
    let r = g.reduce(m, crate::graph::ReduceOp::Max, 1)?;
    let n = g.l2_norm(m, 1e-6)?;
    let c = g.cast_f16(n)?;
    g.output(r)?;
    g.output(c)?;
    let (w, h) = (61usize, 47usize);
    let image: Vec<u8> = (0..w * h * 4).map(|_| rng.below(256) as u8).collect();
    let feed = rng.tensor(&[2, 5], 1.0);
    Ok((g, image, feed))
}

/// Runs the coverage graph on the GPU and the CPU; `Err` says why they differ or what
/// failed.
pub fn self_test(ctx: &Context) -> Result<(), String> {
    let (g, image, extra) = coverage().map_err(|e| e.to_string())?;
    let feeds = [
        Feed::Rgba {
            width: 61,
            height: 47,
            data: &image,
        },
        Feed::Tensor(&extra),
    ];
    let want = crate::cpu::run(&g, &feeds, 1).map_err(|e| e.to_string())?;
    let mut gpu = GpuGraph::new(ctx, &g).map_err(|e| e.to_string())?;
    let got = gpu.run(&feeds).map_err(|e| e.to_string())?;
    for (i, (a, b)) in got.iter().zip(&want).enumerate() {
        let d = compare(&a.to_f32(), &b.to_f32(), ATOL, RTOL);
        if d.worst > 1.0 || a.shape != b.shape {
            return Err(format!(
                "output {i} differs from the CPU (max |error| {}, at {})",
                d.max_abs, d.at
            ));
        }
    }
    Ok(())
}

/// The self-test, then timings of a ViT-Ti/16 at 224² on the CPU (`threads`) and the GPU
/// (whole runs, after a warm-up). Blocks the GPU path for inference on any failure.
/// Returns false if cancelled by `stop`.
pub fn calibrate(ctx: &Context, threads: usize, stop: &dyn Fn() -> bool) -> bool {
    let start = Instant::now();
    let sel = selector();
    let gpu = Task::Inference.path_of(Engine::Gpu).unwrap_or(1);
    if sel.trust(Task::Inference, gpu) == Trust::Blocked {
        return true;
    }
    // The self-test builds every kernel: a few hundred milliseconds with a cold shader cache.
    if stop() {
        return false;
    }
    if let Err(why) = self_test(ctx) {
        sel.block(
            Task::Inference,
            Engine::Gpu,
            &format!("inference self-test: {why}"),
        );
        return true;
    }
    sel.set_available(Task::Inference, Engine::Gpu);
    if stop() {
        return false;
    }
    let cfg = VitConfig {
        image: SIDE as usize,
        ..VitConfig::tiny()
    };
    let mut g = Graph::new();
    let build = |g: &mut Graph| -> Result<(), Error> {
        let x = g.input(&[1, 3, cfg.image, cfg.image], DType::F32)?;
        let y = vit(g, x, &cfg, &mut RandomWeights::new(1))?;
        g.output(y)
    };
    if build(&mut g).is_err() {
        return true;
    }
    let input = Rng::new(3).tensor(&[1, 3, cfg.image, cfg.image], 1.0);
    let feeds = [Feed::Tensor(&input)];
    let px = PIXELS;
    let mut gg = match GpuGraph::new(ctx, &g) {
        Ok(gg) => gg,
        Err(e) => {
            sel.block(Task::Inference, Engine::Gpu, &e.to_string());
            return true;
        }
    };
    if let Err(e) = gg.run(&feeds) {
        sel.block(Task::Inference, Engine::Gpu, &e.to_string());
        return true;
    }
    for _ in 0..2 {
        if stop() {
            return false;
        }
        let run = sel.start(&sel.calibration(Task::Inference, gpu, px, 0));
        let ok = gg.run(&feeds).is_ok();
        if ok {
            run.finish();
        }
    }
    if stop() {
        return false;
    }
    let t = Instant::now();
    let run = sel.start(&sel.calibration(Task::Inference, 0, px, 0));
    let ok = crate::cpu::run(&g, &feeds, threads).is_ok();
    if ok {
        run.finish();
    }
    specular_log::info!(target: specular_log::JOBS,
        "inference calibration: {:.0} ms in all, ViT-Ti/16 on the CPU in {:.0} ms",
        start.elapsed().as_secs_f64() * 1e3, t.elapsed().as_secs_f64() * 1e3);
    true
}
