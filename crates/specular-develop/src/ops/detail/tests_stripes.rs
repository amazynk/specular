//! Refined diffuse renders in stripes (one iterate updated in place, the
//! fine bands' levels stripe by stripe) give the whole-picture program's picture.

use std::sync::atomic::{AtomicBool, Ordering};

use super::diffuse::{self, Diffuse, Preset};
use super::tests_diffuse::{
    after_exposure, edit, engine, extremes, graph_at, module, presets, render_g, scene,
};
use crate::gpu::Request;
use crate::graph::tile::Tiling;
use crate::graph::{Precision, Roi};
use crate::model::ModuleKind;
use crate::testgpu::lock;

fn set(flag: &AtomicBool, on: bool) {
    flag.store(on, Ordering::Relaxed);
}

/// Sets the test switches for a render and puts them back after (also on a panic).
struct Mode;

impl Mode {
    /// Stripes on or off (`rows` their height, 0 the plan's); `coarse`: highlights put
    /// their coarse bands on reduced grids on whole pictures too.
    fn new(stripes: bool, rows: u32, coarse: bool) -> Mode {
        set(&diffuse::FORCE_STRIPES, stripes);
        set(&diffuse::NO_STRIPES, !stripes);
        set(&diffuse::COARSE_HIGHLIGHTS, coarse);
        diffuse::STRIPE_ROWS.store(rows, Ordering::Relaxed);
        Mode
    }
}

impl Drop for Mode {
    fn drop(&mut self) {
        set(&diffuse::FORCE_STRIPES, false);
        set(&diffuse::NO_STRIPES, false);
        set(&diffuse::COARSE_HIGHLIGHTS, false);
        diffuse::STRIPE_ROWS.store(0, Ordering::Relaxed);
    }
}

/// A scene with blown discs (Fill highlights has something to fill).
fn clipped_scene(w: u32, h: u32) -> Vec<f32> {
    let mut img = scene(w, h, 0.03, 9);
    for (cx, cy, r) in [
        (0.3f32, 0.25f32, 0.06f32),
        (0.7, 0.6, 0.1),
        (0.45, 0.85, 0.03),
    ] {
        for y in 0..h {
            for x in 0..w {
                let dx = x as f32 / w as f32 - cx;
                let dy = (y as f32 - cy * h as f32) / w as f32;
                if dx * dx + dy * dy < r * r {
                    let i = ((y * w + x) * 4) as usize;
                    img[i] = 1.4;
                    img[i + 1] = 1.2;
                    img[i + 2] = 0.97;
                }
            }
        }
    }
    img
}

fn diffuse_of(g: &crate::graph::Graph) -> Diffuse {
    let node = g.nodes.iter().find(|n| n.id.0 == ModuleKind::DIFFUSE);
    let mut d = Diffuse::from_fields(&node.expect("diffuse").params.to_fields());
    d.draft = false;
    d
}

/// Every preset and the extremes, f16 and f32, whole and tiled, on a tall picture cut in
/// stripes of 16 rows (or the reach of `u`): bit for bit the whole-picture program's
/// picture. Highlights put their coarse bands on reduced grids on both sides here (the
/// stripes always do).
///
/// Odd sides: the first reduced grid then has a sample on the last column and row, where
/// the cascade's stencils clamp. With an even side, the fine levels made on that grid for
/// the coarse ones clamp one pixel short there ([`stripes_at_even_sides_stay_close`]).
#[test]
fn stripes_equal_whole_pictures() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (241, 421);
    let img = clipped_scene(w, h);
    let mut cases = presets();
    cases.extend(extremes().into_iter().map(|(n, d)| (n.to_string(), d)));
    let mut checked = 0usize;
    let mut cut = 0usize;
    for (name, mut d) in cases {
        d.iterations = d.iterations.min(4);
        let e = after_exposure(edit(vec![module(ModuleKind::DIFFUSE, d.to_fields())]));
        let g = graph_at(&e, w, h, None);
        for prec in [Precision::F32, Precision::F16] {
            for tiling in [Tiling::Auto, Tiling::Side(160)] {
                let req = Request {
                    checkpoints: false,
                    precision: prec,
                    tiling,
                    ..Request::new(Roi::full(w, h))
                };
                let mut out = Vec::new();
                for on in [false, true] {
                    let _m = Mode::new(on, 16, true);
                    if on && tiling == Tiling::Auto && prec == Precision::F32 {
                        let dd = diffuse_of(&g);
                        assert!(diffuse::plan(&dd).striped, "{name}");
                        let prog = diffuse::program(&dd, Roi::full(w, h));
                        cut += prog
                            .steps
                            .iter()
                            .filter(|s| s.kernel.name == diffuse::DF_COPY.name)
                            .count();
                    }
                    let px = render_g(&mut en, &g, &img, w, h, &req);
                    out.push(px.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
                }
                let differ = out[0].iter().zip(&out[1]).filter(|(a, b)| a != b).count();
                assert_eq!(differ, 0, "{name} {prec:?} {tiling:?}");
                checked += out[0].len();
            }
        }
    }
    assert!(cut > 0, "no case ran in more than one stripe");
    eprintln!("{checked} values, 0 differ ({cut} stripe copies)");
}

/// Even sides: the presets in stripes against whole pictures, f32 (ΔE 2000; they differ
/// within the coarse bands' reach of the last column and row).
#[test]
fn stripes_at_even_sides_stay_close() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (640, 426);
    let img = clipped_scene(w, h);
    for (name, d) in presets() {
        let e = after_exposure(edit(vec![module(ModuleKind::DIFFUSE, d.to_fields())]));
        let g = graph_at(&e, w, h, None);
        let req = Request {
            checkpoints: false,
            precision: Precision::F32,
            ..Request::new(Roi::full(w, h))
        };
        let mut out = Vec::new();
        for on in [false, true] {
            let _m = Mode::new(on, 0, true);
            out.push(render_g(&mut en, &g, &img, w, h, &req));
        }
        let de = super::tests_diffuse_speed::delta_es(&out[1], &out[0]);
        let differ = out[0].iter().zip(&out[1]).filter(|(a, b)| a != b).count();
        eprintln!(
            "{name} {w}x{h}: stripes vs whole ΔE2000 p99 {:.5} max {:.4} ({differ} values differ)",
            de[(de.len() as f64 * 0.99) as usize],
            de[de.len() - 1]
        );
        assert!(de[de.len() - 1] < 0.5, "{name}");
    }
}

/// Fill highlights in stripes (coarse bands on reduced grids, gated at each pixel) against
/// every band on the full grid (the whole-picture program), f32: ΔE 2000.
#[test]
fn stripe_highlights_stay_close_to_full_grids() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (900, 600);
    let img = clipped_scene(w, h);
    let e = after_exposure(edit(vec![module(
        ModuleKind::DIFFUSE,
        Preset::InpaintHighlights.params().to_fields(),
    )]));
    let g = graph_at(&e, w, h, None);
    let req = Request {
        checkpoints: false,
        precision: Precision::F32,
        ..Request::new(Roi::full(w, h))
    };
    let mut out = Vec::new();
    for on in [false, true] {
        let _m = Mode::new(on, 0, false);
        out.push(render_g(&mut en, &g, &img, w, h, &req));
    }
    // The picture without the fill, to tell the pixels it changes.
    let plain = {
        let e = after_exposure(edit(Vec::new()));
        render_g(&mut en, &graph_at(&e, w, h, None), &img, w, h, &req)
    };
    let d = super::tests_diffuse_speed::delta_es(&out[1], &out[0]);
    let q = |p: f64| d[((d.len() as f64 * p) as usize).min(d.len() - 1)];
    let over = d.iter().filter(|e| **e >= 1.0).count();
    // Over the pixels the fill changes (the rest stay as they are on both sides).
    let mut dc: Vec<f64> = out[1]
        .chunks_exact(4)
        .zip(out[0].chunks_exact(4))
        .zip(plain.chunks_exact(4))
        .filter(|((_, a), i)| (a[0] - i[0]).abs() > 1e-4)
        .map(|((s, f), _)| {
            use crate::ops::tone::tests::{de2000, lab};
            let c = |p: &[f32]| lab([p[0], p[1], p[2]].map(|v| v.clamp(0.0, 1.0)));
            de2000(c(s), c(f))
        })
        .collect();
    dc.sort_by(f64::total_cmp);
    let changed = dc.len();
    let qc = |p: f64| dc[((dc.len() as f64 * p) as usize).min(dc.len().saturating_sub(1))];
    eprintln!(
        "Fill highlights {w}x{h}, stripes vs full grids: ΔE2000 p95 {:.4} p99 {:.4} max {:.3}, {over} ≥ 1; over the {changed} changed pixels p95 {:.4} p99 {:.4}",
        q(0.95),
        q(0.99),
        d[d.len() - 1],
        qc(0.95),
        qc(0.99),
    );
    assert!(changed > 1000, "{changed}");
    assert!(
        q(0.99) <= 0.1 && qc(0.99) <= 1.0,
        "{} {}",
        q(0.99),
        qc(0.99)
    );
}
