//! The AI networks of Develop at their real sizes with fixed-seed random f16 weights (the
//! installed models are f16; the work does not depend on the weights' values): denoise per
//! 512² tile (FFDNet colour), remove per crop (Big-LaMa), depth per photo
//! (Depth Anything V2 Small). No §3.9 budget: gated by the +10 % rule only.

use std::time::Instant;

use specular_infer::depth::{self, DepthConfig};
use specular_infer::ffdnet::{self, FfdConfig};
use specular_infer::gpu::{Context, GpuGraph};
use specular_infer::lama;
use specular_infer::models::RandomWeights;
use specular_infer::tensor::Rng;
use specular_infer::tiles::Tiling;
use specular_infer::{Feed, Graph, Tensor};

use crate::cases::{Bench, Sample};

/// The removal crop timed (the store makes crops of 64–448 px; one submission each).
const REMOVE_SIDE: usize = 384;

pub fn networks(b: &mut Bench) {
    let names = [
        "ai/denoise/tile-512",
        "ai/remove/crop-384",
        "ai/depth/photo-3x2",
    ];
    if !names.iter().any(|n| b.wants(n)) {
        return;
    }
    let ctx = Context::new(&b.device, &b.queue);
    let weights = |seed| RandomWeights { seed, f16: true };
    let mut rng = Rng::new(20);

    // Denoise: one FFDNet tile with its margin, and the noise level.
    let t = Tiling::FFDNET;
    let g = ffdnet::graph(&mut weights(24), &FfdConfig::color(), (t.size, t.size))
        .expect("ffdnet graph");
    let x = rng.tensor(&[1, 3, t.size, t.size], 0.5);
    let sigma = Tensor::f32(&[1, 1, 1, 1], vec![0.03]);
    time(
        b,
        names[0],
        &ctx,
        &g,
        &[Feed::Tensor(&x), Feed::Tensor(&sigma)],
        6,
    );

    // Remove: one crop with a round hole.
    let s = REMOVE_SIDE;
    let g = lama::graph(&mut weights(27), (s, s)).expect("lama graph");
    let rgb: Vec<f32> = (0..3 * s * s).map(|i| (i % 97) as f32 / 97.0).collect();
    let mask: Vec<f32> = (0..s * s)
        .map(|i| {
            let (x, y) = (
                (i % s) as f32 - s as f32 / 2.0,
                (i / s) as f32 - s as f32 / 2.0,
            );
            if x * x + y * y < (s as f32 / 6.0).powi(2) {
                1.0
            } else {
                0.0
            }
        })
        .collect();
    let x = lama::input(&rgb, &mask, (s, s));
    time(b, names[1], &ctx, &g, &[Feed::Tensor(&x)], 3);

    // Depth: a 3:2 photo at the network's input size.
    let (iw, ih) = depth::input_size(6000, 4000, depth::SIDE);
    let g = depth::graph_from_tensor(&mut weights(28), &DepthConfig::small(), (iw, ih))
        .expect("depth graph");
    let x = rng.tensor(&[1, 3, ih, iw], 1.0);
    time(b, names[2], &ctx, &g, &[Feed::Tensor(&x)], 3);
}

/// Wall: upload, one submission, read back. GPU: the dispatches' timestamps summed.
fn time(b: &mut Bench, name: &str, ctx: &Context, g: &Graph, feeds: &[Feed], n: usize) {
    if !b.wants(name) {
        return;
    }
    let mut gg = GpuGraph::new(ctx, g).expect("gpu graph");
    let _ = gg.run(feeds).expect("warm-up run");
    let timestamps = ctx.features().timestamps;
    b.case(
        name,
        "ai",
        true,
        None,
        "random f16 weights at the real size",
        |b| {
            (0..b.count(n))
                .map(|_| {
                    let t = Instant::now();
                    let _ = gg.run(feeds).expect("run");
                    let wall = t.elapsed().as_secs_f64() * 1e3;
                    let gpu = timestamps
                        .then(|| gg.profile(feeds).ok())
                        .flatten()
                        .map(|v| v.iter().map(|(_, ms)| ms).sum());
                    Sample { wall, gpu }
                })
                .collect()
        },
    );
}
