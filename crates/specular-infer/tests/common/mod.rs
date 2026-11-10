//! Shared by the GPU-vs-CPU tests: one device per test process, and the comparison.
//! Every GPU check runs twice: with the device's optional features (f16 weights) and
//! with the plain kernels.
//!
//! Without an adapter the GPU half is skipped (the CPU reference still runs) unless
//! SPECULAR_REQUIRE_GPU is set.

#![allow(dead_code)]

use std::sync::OnceLock;

use specular_infer::gpu::{Context, Features, GpuGraph};
use specular_infer::tensor::{Diff, compare};
use specular_infer::{Feed, Graph, Tensor, cpu};

/// The device's contexts: with its optional features, then (when it has any) without.
pub fn contexts() -> &'static [Context] {
    static C: OnceLock<Option<(specular_gfx::Gpu, Vec<Context>)>> = OnceLock::new();
    C.get_or_init(|| match specular_gfx::Gpu::new() {
        Ok(g) => {
            let c = Context::new(&g.device, &g.queue);
            eprintln!(
                "gpu: {} ({:?}), {:?}",
                g.info.name,
                g.info.backend,
                c.features()
            );
            let mut cs = vec![c.clone()];
            if c.features() != Features::none() {
                cs.push(Context::with_features(
                    &g.device,
                    &g.queue,
                    Features::none(),
                ));
            }
            Some((g, cs))
        }
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU halves skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
    .map_or(&[], |(_, c)| c)
}

pub fn ctx() -> Option<&'static Context> {
    contexts().first()
}

/// Runs `g` on the CPU (2 threads) and the GPU (twice: the second run must repeat the
/// first bit for bit) and checks every output within |gpu − cpu| ≤ atol + rtol·|cpu|.
/// Returns the CPU outputs and the worst difference seen.
pub fn check(name: &str, g: &Graph, feeds: &[Feed], atol: f32, rtol: f32) -> (Vec<Tensor>, Diff) {
    check_with(name, contexts(), g, feeds, atol, rtol)
}

/// As [`check`], on the given contexts.
pub fn check_with(
    name: &str,
    contexts: &[Context],
    g: &Graph,
    feeds: &[Feed],
    atol: f32,
    rtol: f32,
) -> (Vec<Tensor>, Diff) {
    let want = cpu::run(g, feeds, 2).unwrap_or_else(|e| panic!("{name}: cpu: {e}"));
    let one = cpu::run(g, feeds, 1).unwrap_or_else(|e| panic!("{name}: cpu: {e}"));
    assert_eq!(
        one, want,
        "{name}: the CPU result depends on the thread count"
    );
    let mut worst = Diff {
        cosine: 1.0,
        ..Diff::default()
    };
    for c in contexts {
        let name = format!("{name} ({:?})", c.features());
        worst = check_on(&name, c, g, feeds, &want, atol, rtol, worst);
    }
    (want, worst)
}

#[allow(clippy::too_many_arguments)]
fn check_on(
    name: &str,
    c: &Context,
    g: &Graph,
    feeds: &[Feed],
    want: &[Tensor],
    atol: f32,
    rtol: f32,
    mut worst: Diff,
) -> Diff {
    let mut gg = GpuGraph::new(c, g).unwrap_or_else(|e| panic!("{name}: gpu build: {e}"));
    let got = gg
        .run(feeds)
        .unwrap_or_else(|e| panic!("{name}: gpu run: {e}"));
    assert_eq!(got.len(), want.len(), "{name}: output count");
    for (i, (a, b)) in got.iter().zip(want).enumerate() {
        assert_eq!(a.shape, b.shape, "{name}: output {i} shape");
        assert_eq!(a.dtype(), b.dtype(), "{name}: output {i} dtype");
        let d = compare(&a.to_f32(), &b.to_f32(), atol, rtol);
        assert!(
            d.worst <= 1.0,
            "{name}: output {i} off by {} at {} (gpu {}, cpu {}; atol {atol}, rtol {rtol})",
            d.max_abs,
            d.at,
            a.to_f32()[d.at],
            b.to_f32()[d.at],
        );
        if d.worst >= worst.worst {
            worst = Diff {
                cosine: d.cosine.min(worst.cosine),
                ..d
            };
        }
    }
    let again = gg
        .run(feeds)
        .unwrap_or_else(|e| panic!("{name}: gpu rerun: {e}"));
    assert_eq!(again, got, "{name}: a second GPU run differs");
    worst
}
