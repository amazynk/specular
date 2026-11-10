//! Retouch: params round trip, the multigrid's convergence, the GPU's Poisson residual,
//! GPU = CPU reference for every mode and layer, tiles = whole, no seams, spots that follow
//! the warp, no pipelines per spot move, preview against export, and drag latency.

use super::cpu_ref::{self, Grid, Pic};
use super::layout::Layout;
use super::search::{View, Want, best_source};
use super::{Layer, Placed, PlacedSpot, Retouch, Spot, SpotMode};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request, Source};
use crate::graph::tile::Tiling;
use crate::graph::{Graph, OpRegistry, Placement, Precision, Roi};
use crate::mask::{Geometry, MaskCache, MaskEnv};
use crate::model::{Edit, ModuleInstance, ModuleKind, Params, Registry, SourceKey};
use crate::testgpu::{dev, lock};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((v >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
}

/// A photo-like linear scene: a diagonal gradient, a coloured texture, fine noise, a few
/// dark blemishes and an edge.
fn scene(w: u32, h: u32, seed: u64) -> Vec<f32> {
    let mut rng = Rng(seed);
    let mut v = vec![0f32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let tex = 0.04 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
            let n = 0.02 * (rng.next() as f32 - 0.5);
            let mut c = [
                0.15 + 0.35 * u + 0.1 * t + tex + n,
                0.2 + 0.25 * u + 0.2 * t + 0.5 * tex + n,
                0.3 + 0.1 * u + 0.15 * t - tex + n,
            ];
            if u > 0.8 {
                c = c.map(|k| k * 0.5);
            }
            for (bx, by) in [(0.3, 0.4), (0.55, 0.6), (0.7, 0.3)] {
                let d = ((u - bx) * w as f32).hypot((t - by) * h as f32);
                if d < 0.02 * w as f32 {
                    c = c.map(|k| k * 0.25);
                }
            }
            let i = ((y * w + x) * 4) as usize;
            v[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    v
}

fn engine(d: &crate::testgpu::Dev) -> Engine {
    Engine::new(&d.device, &d.queue, Budget::fixed(1 << 30))
}

fn spot(mode: SpotMode, layer: Layer, target: [f32; 2], source: [f32; 2], size: f32) -> Spot {
    Spot {
        mode,
        layer,
        target,
        source,
        size,
        feather: 0.3,
        opacity: 1.0,
        blur: 0.01,
        path: Vec::new(),
        model: String::new(),
    }
}

fn retouch_edit(r: &Retouch) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    let mut m = ModuleInstance::new(Registry::builtin(), ModuleKind::RETOUCH, 0);
    m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
    e.insert(Registry::builtin(), m);
    e
}

fn frame(w: u32, h: u32) -> Geometry<'static> {
    Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    }
}

fn graph(e: &Edit, src: &Source, cache: &mut MaskCache, focus: Option<(ModuleKind, u16)>) -> Graph {
    let mut env = MaskEnv {
        geometry: frame(src.width, src.height),
        image: (src.width, src.height),
        scale: 1.0,
        cache,
        sensor_bin: 1.0,
        overlay: None,
    };
    Graph::build_masked(e, OpRegistry::builtin(), focus, None, Some(&mut env)).expect("graph")
}

fn render(en: &mut Engine, g: &Graph, src: &Source, req: &Request) -> Vec<f32> {
    let r = en.render(g, src, req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

fn exact(w: u32, h: u32) -> Request {
    Request {
        precision: Precision::F32,
        checkpoints: false,
        ..Request::new(Roi::full(w, h))
    }
}

fn placed(r: &Retouch, w: u32, h: u32) -> Placed {
    Placed::new(
        r,
        &Placement {
            geometry: frame(w, h),
            image: (w, h),
            draft: false,
            sensor_bin: 1.0,
            photo: [0; 32],
        },
    )
}

#[test]
fn fields_round_trip_and_short_columns_read() {
    let mut s = spot(SpotMode::Fill, Layer::High, [0.2, 0.3], [0.5, 0.6], 0.03);
    s.path = vec![[0.1, 0.1], [0.2, 0.15], [0.25, 0.3]];
    let r = Retouch {
        split_radius: 6.5,
        spots: vec![
            spot(SpotMode::Heal, Layer::All, [0.4, 0.4], [0.6, 0.4], 0.02),
            s,
            spot(SpotMode::Blur, Layer::Low, [0.7, 0.7], [0.7, 0.7], 0.05),
        ],
    };
    let f = r.to_fields();
    assert_eq!(Retouch::from_fields(&f), r);
    // Through the generic params and the catalogue codec too.
    let p = Params::from_fields(ModuleKind::RETOUCH, f.clone());
    assert_eq!(Retouch::of(&p), r);
    // A short column gives defaults; an unknown mode reads as heal.
    let mut short = f.clone();
    short.set(
        "spot_mode",
        crate::model::Value::Text("clone sparkle".into()),
    );
    short.set("spot_size", crate::model::Value::Floats(vec![0.04]));
    short.set("spot_points", crate::model::Value::Floats(vec![]));
    let back = Retouch::from_fields(&short);
    assert_eq!(back.spots.len(), 2);
    assert_eq!(back.spots[1].mode, SpotMode::Heal);
    assert_eq!(back.spots[0].size, 0.04);
    assert_eq!(back.spots[1].size, Spot::default().size);
    assert!(back.spots.iter().all(|s| s.path.is_empty()));
    // Defaults are the identity; a spot at opacity 0 too.
    assert!(Retouch::default().is_identity());
    let mut off = r.clone();
    for s in &mut off.spots {
        s.opacity = 0.0;
    }
    assert!(off.is_identity());
    // The placed form round-trips.
    let pl = placed(&r, 640, 480);
    assert_eq!(Placed::from_fields(&pl.to_fields()), Some(pl.clone()));
    assert_eq!(pl.spots.len(), 3);
    assert_eq!(pl.spots[0].offset, [128, 0]);
}

/// The f64 copy of the GPU's multigrid reaches a residual far below 1e-4 on noisy edge data
/// over round, long and brushed domains.
#[test]
fn cpu_multigrid_converges() {
    let mut rng = Rng(7);
    let mut worst = 0f64;
    for (w, h, shape) in [
        (12, 12, 0),
        (40, 40, 0),
        (130, 130, 0),
        (321, 321, 0),
        (300, 80, 1),
        (200, 200, 2),
    ] {
        let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
        let r = (w.min(h) as f64 / 2.0) - 1.5;
        let mut g = Grid {
            w,
            h,
            free: vec![false; w * h],
            u: vec![[0.0; 3]; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = (x as f64 - cx, y as f64 - cy);
                let inside = match shape {
                    0 => dx.hypot(dy) < r,
                    1 => dy.abs() < r && dx.abs() < w as f64 / 2.0 - 2.0,
                    _ => (dx.hypot(dy) < r) && !(dx > 0.0 && dy.abs() < 20.0),
                };
                let border = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
                let i = y * w + x;
                if inside && !border {
                    g.free[i] = true;
                } else {
                    let base = 0.4 + 0.3 * (x as f64 * 0.05).sin();
                    g.u[i] = [0, 1, 2].map(|k| base + 0.2 * (rng.next() - 0.5) + 0.1 * k as f64);
                }
            }
        }
        let start = g.residual();
        g.solve_mg();
        let res = g.residual();
        eprintln!("multigrid {w}×{h} shape {shape}: residual {start:.2e} → {res:.2e}");
        worst = worst.max(res / start.max(1e-12));
    }
    assert!(worst < 1e-5, "{worst}");
}

/// Heal on the GPU (f32): `Δ(out − source) = 0` inside the spot within 1e-4.
#[test]
fn gpu_heal_poisson_residual() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (360u32, 260u32);
    let img = scene(w, h, 3);
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut worst_all = 0f64;
    for (r_px, path) in [(70.0f32, false), (12.0, false), (20.0, true)] {
        let mut s = spot(
            SpotMode::Heal,
            Layer::All,
            [0.3, 0.4],
            [0.62, 0.45],
            r_px / w as f32,
        );
        s.feather = 0.0;
        if path {
            s.path = vec![[0.2, 0.3], [0.3, 0.45], [0.4, 0.4]];
        }
        let r = Retouch {
            spots: vec![s],
            ..Retouch::default()
        };
        let pl = placed(&r, w, h);
        let ps = &pl.spots[0];
        let mut cache = MaskCache::default();
        let g = graph(&retouch_edit(&r), &src, &mut cache, None);
        let out = render(&mut en, &g, &src, &exact(w, h));
        let at = |v: &[f32], x: i64, y: i64, k: usize| {
            f64::from(v[4 * (y as usize * w as usize + x as usize) + k])
        };
        let lay = Layout::new(&pl);
        let g0 = lay.chunks[0].g0.unwrap();
        let mut worst = 0f64;
        let mut n = 0;
        // Pixels whose neighbours are all fully inside (mask 1: one pixel in from the edge).
        let dist = |x: i64, y: i64| cpu_ref_dist(ps, x as f64, y as f64);
        for y in i64::from(g0.oy) + 1..i64::from(g0.oy) + i64::from(g0.h) - 1 {
            for x in i64::from(g0.ox) + 1..i64::from(g0.ox) + i64::from(g0.w) - 1 {
                if [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(a, b)| dist(x + a, y + b) > f64::from(ps.radius) - 1.0)
                {
                    continue;
                }
                n += 1;
                for k in 0..3 {
                    let g = |x: i64, y: i64| {
                        at(&out, x, y, k)
                            - at(
                                &img,
                                x + i64::from(ps.offset[0]),
                                y + i64::from(ps.offset[1]),
                                k,
                            )
                    };
                    let lap = g(x - 1, y) + g(x + 1, y) + g(x, y - 1) + g(x, y + 1) - 4.0 * g(x, y);
                    worst = worst.max(lap.abs());
                }
            }
        }
        eprintln!("GPU heal r {r_px} px (path {path}): Poisson residual {worst:.2e} over {n} px");
        assert!(n > 50);
        worst_all = worst_all.max(worst);
    }
    assert!(worst_all < 1e-4, "{worst_all}");
}

fn cpu_ref_dist(s: &PlacedSpot, x: f64, y: f64) -> f64 {
    let p = [x, y];
    let d = |a: [f32; 2]| (p[0] - f64::from(a[0])).hypot(p[1] - f64::from(a[1]));
    if s.path.is_empty() {
        return d(s.centre);
    }
    s.path
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = [f64::from(b[0] - a[0]), f64::from(b[1] - a[1])];
            let ap = [p[0] - f64::from(a[0]), p[1] - f64::from(a[1])];
            let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12))
                .clamp(0.0, 1.0);
            (ap[0] - ab[0] * t).hypot(ap[1] - ab[1] * t)
        })
        .fold(f64::MAX, f64::min)
}

fn all_kinds() -> Retouch {
    let mut brush = spot(SpotMode::Heal, Layer::All, [0.45, 0.2], [0.45, 0.75], 0.025);
    brush.path = vec![[0.38, 0.18], [0.45, 0.22], [0.52, 0.2]];
    Retouch {
        split_radius: 3.0,
        spots: vec![
            spot(SpotMode::Heal, Layer::All, [0.3, 0.4], [0.3, 0.75], 0.06),
            spot(SpotMode::Clone, Layer::All, [0.55, 0.6], [0.4, 0.6], 0.04),
            spot(SpotMode::Fill, Layer::All, [0.7, 0.3], [0.7, 0.6], 0.05),
            spot(SpotMode::Blur, Layer::All, [0.15, 0.7], [0.15, 0.7], 0.05),
            spot(SpotMode::Heal, Layer::Low, [0.85, 0.5], [0.6, 0.85], 0.05),
            spot(SpotMode::Clone, Layer::High, [0.2, 0.2], [0.1, 0.4], 0.04),
            spot(
                SpotMode::Fill,
                Layer::High,
                [0.62, 0.15],
                [0.62, 0.45],
                0.03,
            ),
            spot(SpotMode::Blur, Layer::Low, [0.3, 0.85], [0.3, 0.85], 0.04),
            spot(
                SpotMode::Blur,
                Layer::High,
                [0.85, 0.85],
                [0.85, 0.85],
                0.04,
            ),
            brush,
        ],
    }
}

/// Every mode on every layer, overlapping, against the f64 reference.
#[test]
fn gpu_equals_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (240u32, 180u32);
    let img = scene(w, h, 11);
    let r = all_kinds();
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut cache = MaskCache::default();
    let g = graph(&retouch_edit(&r), &src, &mut cache, None);
    let gpu = render(&mut en, &g, &src, &exact(w, h));
    let cpu = cpu_ref::retouch(&Pic { w, h, px: &img }, &placed(&r, w, h));
    let (mut worst, mut at, mut changed) = (0f64, 0, 0);
    for i in 0..(w * h) as usize {
        for k in 0..3 {
            let e = f64::from((gpu[4 * i + k] - cpu[4 * i + k]).abs());
            if e > worst {
                worst = e;
                at = i;
            }
        }
        changed += usize::from((cpu[4 * i] - img[4 * i]).abs() > 1e-4);
    }
    eprintln!(
        "retouch GPU vs CPU: worst {worst:.2e} at ({}, {}); {changed} px changed",
        at as u32 % w,
        at as u32 / w
    );
    assert!(changed > 3000);
    assert!(worst < 1e-3, "{worst}");
}

/// Tiles equal the whole picture (every tile solves the whole spot the same way).
#[test]
fn tiles_equal_the_whole_image() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (240u32, 180u32);
    let img = scene(w, h, 5);
    let r = all_kinds();
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut cache = MaskCache::default();
    let g = graph(&retouch_edit(&r), &src, &mut cache, None);
    let whole = render(&mut en, &g, &src, &exact(w, h));
    let tiled_req = Request {
        tiling: Tiling::Side(64),
        ..exact(w, h)
    };
    let r = en.render(&g, &src, &tiled_req).unwrap();
    assert!(r.stats.tiles > 1);
    let tiled = en.read(&r).unwrap();
    en.recycle(r);
    let worst = whole
        .iter()
        .zip(&tiled)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    eprintln!("retouch tiled vs whole: worst {worst:.2e}");
    assert!(worst <= 1e-6, "{worst}");
}

/// Heal and fill leave no seam: across the spot's edge the picture changes no more than it
/// does elsewhere, while clone (no blending at a hard edge) shows one.
#[test]
fn heal_and_fill_have_no_seams() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (320u32, 240u32);
    // A smooth gradient with fine texture; the source is much brighter than the target.
    let mut img = vec![0f32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = 0.1 + 0.6 * x as f32 / w as f32 + 0.01 * ((x * 7 + y * 13) % 5) as f32;
            let i = ((y * w + x) * 4) as usize;
            img[i..i + 4].copy_from_slice(&[v, v * 0.9, v * 0.8, 1.0]);
        }
    }
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut edge_jump = |mode: SpotMode| {
        let mut s = spot(mode, Layer::All, [0.25, 0.5], [0.75, 0.5], 0.12);
        s.feather = 0.0;
        let r = Retouch {
            spots: vec![s],
            ..Retouch::default()
        };
        let pl = placed(&r, w, h);
        let ps = pl.spots[0].clone();
        let mut cache = MaskCache::default();
        let g = graph(&retouch_edit(&r), &src, &mut cache, None);
        let out = render(&mut en, &g, &src, &exact(w, h));
        let lum = |x: u32, y: u32| out[4 * (y * w + x) as usize + 1];
        // The largest step between neighbours at the edge, against the largest elsewhere.
        let (mut edge, mut away) = (0f32, 0f32);
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let dd = cpu_ref_dist(&ps, f64::from(x), f64::from(y)) - f64::from(ps.radius);
                let step = (lum(x + 1, y) - lum(x, y))
                    .abs()
                    .max((lum(x, y + 1) - lum(x, y)).abs());
                if dd.abs() < 2.0 {
                    edge = edge.max(step);
                } else if dd > 6.0 {
                    away = away.max(step);
                }
            }
        }
        (edge, away)
    };
    for mode in [SpotMode::Heal, SpotMode::Fill, SpotMode::Clone] {
        let (edge, away) = edge_jump(mode);
        eprintln!("{mode:?}: step at the edge {edge:.4}, elsewhere {away:.4}");
        if mode == SpotMode::Clone {
            assert!(edge > 3.0 * away, "clone should show its edge");
        } else {
            assert!(edge <= 1.25 * away, "{mode:?} seam: {edge} vs {away}");
        }
    }
}

/// Spots stay on their feature after rotate, crop and lens changes.
#[test]
fn spots_follow_the_warp() {
    use crate::model::params::CropOrientation;
    use crate::ops::geometry::{Context, LensParams, Plan};
    use crate::preview::Orient;
    let mut e = Edit::default_for(Registry::builtin(), Default::default(), false);
    let m = e.module_mut(ModuleKind::CROP_ORIENTATION, 0).expect("crop");
    m.params = Params::CropOrientation(CropOrientation {
        angle_deg: 7.0,
        crop: [0.08, 0.1, 0.9, 0.85],
        ..CropOrientation::default()
    });
    let mut lens = ModuleInstance::new(Registry::builtin(), ModuleKind::LENS, 0);
    lens.params = Params::Other(
        ModuleKind::LENS,
        crate::model::params::ModuleParams::to_fields(&LensParams {
            embedded: false,
            auto_scale: false,
            distortion: 40.0,
            ..LensParams::default()
        }),
    );
    e.modules.push(lens);
    e.sort(Registry::builtin());
    let ctx = Context::image(6000, 4000, Orient::default());
    let plan = Plan::new(&e, &ctx, true);
    let (ow, oh) = plan.output_size(1.0);
    for (img, scale) in [((ow, oh), 1.0), ((ow / 4, oh / 4), 0.25)] {
        let at = Placement {
            geometry: Geometry::Warp(&plan),
            image: img,
            draft: false,
            sensor_bin: 1.0,
            photo: [0; 32],
        };
        let s = spot(SpotMode::Heal, Layer::All, [0.3, 0.4], [0.5, 0.45], 0.01);
        let pl = Placed::new(
            &Retouch {
                spots: vec![s],
                ..Retouch::default()
            },
            &at,
        );
        let p = &pl.spots[0];
        let want = plan.to_output((0.3 * 6000.0, 0.4 * 4000.0), 1.0).unwrap();
        let src = plan.to_output((0.5 * 6000.0, 0.45 * 4000.0), 1.0).unwrap();
        let k = f64::from(img.0) / f64::from(ow);
        let err = (f64::from(p.centre[0]) - want.0 * k).hypot(f64::from(p.centre[1]) - want.1 * k);
        let off_err = (f64::from(p.offset[0]) - (src.0 - want.0) * k)
            .hypot(f64::from(p.offset[1]) - (src.1 - want.1) * k);
        // 60 px radius at full size, scaled with the picture and the lens.
        let r_err = (f64::from(p.radius) / (60.0 * scale) - 1.0).abs();
        eprintln!(
            "placed at scale {scale}: centre {err:.3} px, offset {off_err:.3} px, radius {r_err:.3}"
        );
        assert!(err < 0.01 && off_err <= 0.71 && r_err < 0.1);
        // Back to the frame.
        let back = super::image_to_frame(
            &Geometry::Warp(&plan),
            img,
            [f64::from(p.centre[0]), f64::from(p.centre[1])],
        );
        assert!(
            (back[0] - 0.3).abs() < 1e-4 && (back[1] - 0.4).abs() < 1e-4,
            "{back:?}"
        );
    }
}

/// Moving, resizing and retyping spots builds no pipeline.
#[test]
fn spot_changes_build_no_pipeline() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (200u32, 150u32);
    let img = scene(w, h, 9);
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut r = all_kinds();
    let mut cache = MaskCache::default();
    let focus = Some((ModuleKind::RETOUCH, 0));
    let req = Request::new(Roi::full(w, h));
    let g = graph(&retouch_edit(&r), &src, &mut cache, focus);
    let first = en.render(&g, &src, &req).unwrap();
    en.recycle(first);
    let mut rng = Rng(3);
    for i in 0..12 {
        let k = i % r.spots.len();
        r.spots[k].target[0] = 0.1 + 0.8 * rng.next() as f32;
        r.spots[k].source[1] = 0.1 + 0.8 * rng.next() as f32;
        r.spots[k].size = 0.02 + 0.05 * rng.next() as f32;
        r.spots[k].feather = rng.next() as f32;
        if i == 6 {
            r.spots.push(spot(
                SpotMode::Fill,
                Layer::Low,
                [0.5, 0.5],
                [0.2, 0.2],
                0.08,
            ));
        }
        if i == 9 {
            r.spots.remove(0);
        }
        let g = graph(&retouch_edit(&r), &src, &mut cache, focus);
        let out = en.render(&g, &src, &req).unwrap();
        assert_eq!(out.stats.pipelines_created, 0, "move {i} built a pipeline");
        en.recycle(out);
    }
}

/// A preview at half size matches the full-size render shrunk (ΔE2000).
#[test]
fn preview_matches_export() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (640u32, 480u32);
    let full = scene(w, h, 21);
    let half = crate::ops::tone::tests::shrink(&full, w, h, 2);
    let mut en = engine(d);
    let r = all_kinds();
    let e = retouch_edit(&r);
    let sf = en.upload(w, h, &full).unwrap();
    let sh = en.upload(w / 2, h / 2, &half).unwrap();
    let render_on = |en: &mut Engine, src: &Source| {
        let mut cache = MaskCache::default();
        let mut env = MaskEnv {
            geometry: frame(w, h),
            image: (src.width, src.height),
            scale: src.width as f32 / w as f32,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        };
        let g = Graph::build_masked(&e, OpRegistry::builtin(), None, None, Some(&mut env)).unwrap();
        render(
            en,
            &g,
            src,
            &Request {
                scale: src.width as f32 / w as f32,
                ..exact(src.width, src.height)
            },
        )
    };
    let a = render_on(&mut en, &sf);
    let b = render_on(&mut en, &sh);
    let a = crate::ops::tone::tests::shrink(&a, w, h, 2);
    let mut de: Vec<f64> = (0..(w / 2 * h / 2) as usize)
        .map(|i| {
            let p = |v: &[f32]| {
                crate::ops::tone::tests::lab(
                    [v[4 * i], v[4 * i + 1], v[4 * i + 2]].map(|x| x.clamp(0.0, 1.0)),
                )
            };
            crate::ops::tone::tests::de2000(p(&a), p(&b))
        })
        .collect();
    de.sort_by(f64::total_cmp);
    let p95 = de[de.len() * 95 / 100];
    let p99 = de[de.len() * 99 / 100];
    eprintln!("retouch preview vs export ΔE2000: p95 {p95:.3}, p99 {p99:.3}");
    assert!(p95 < 1.0, "{p95}");
}

/// Dragging a spot on a 24 MP photo's fit-view preview (2560×1707): place, build and render
/// per move. Timings are printed; the bound is loose (the machine may be loaded).
#[test]
fn spot_drag_latency() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (2560u32, 1707u32);
    let img = scene(w, h, 2);
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut cache = MaskCache::default();
    let mut r = all_kinds();
    let focus = Some((ModuleKind::RETOUCH, 0));
    let mut times = Vec::new();
    for i in 0..60 {
        r.spots[0].source = [0.3 + 0.003 * i as f32, 0.75 - 0.002 * i as f32];
        let t = std::time::Instant::now();
        let mut env = MaskEnv {
            geometry: Geometry::Frame {
                w: 6000.0,
                h: 4000.0,
            },
            image: (w, h),
            scale: 2560.0 / 6000.0,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        };
        let g = Graph::build_masked(
            &retouch_edit(&r),
            OpRegistry::builtin(),
            focus,
            None,
            Some(&mut env),
        )
        .unwrap();
        let out = en
            .render(
                &g,
                &src,
                &Request {
                    scale: 2560.0 / 6000.0,
                    ..Request::new(Roi::full(w, h))
                },
            )
            .unwrap();
        en.wait().unwrap();
        en.recycle(out);
        if i >= 5 {
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    // The refined render after each drag (no focus: every V-cycle; the spots left in place
    // keep their solves).
    let mut refined = Vec::new();
    for i in 0..8 {
        r.spots[0].source = [0.5 + 0.004 * i as f32, 0.7];
        let mut env = MaskEnv {
            geometry: Geometry::Frame {
                w: 6000.0,
                h: 4000.0,
            },
            image: (w, h),
            scale: 2560.0 / 6000.0,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        };
        let g = Graph::build_masked(
            &retouch_edit(&r),
            OpRegistry::builtin(),
            focus,
            None,
            Some(&mut env),
        )
        .unwrap();
        let req = Request {
            scale: 2560.0 / 6000.0,
            ..Request::new(Roi::full(w, h))
        };
        let out = en.render(&g, &src, &req).unwrap();
        en.wait().unwrap();
        en.recycle(out);
        let t = std::time::Instant::now();
        let mut env = MaskEnv {
            geometry: Geometry::Frame {
                w: 6000.0,
                h: 4000.0,
            },
            image: (w, h),
            scale: 2560.0 / 6000.0,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        };
        let g = Graph::build_masked(
            &retouch_edit(&r),
            OpRegistry::builtin(),
            None,
            None,
            Some(&mut env),
        )
        .unwrap();
        let req = Request {
            scale: 2560.0 / 6000.0,
            ..Request::new(Roi::full(w, h))
        };
        let out = en.render(&g, &src, &req).unwrap();
        en.wait().unwrap();
        en.recycle(out);
        refined.push(t.elapsed().as_secs_f64() * 1e3);
    }
    refined.sort_by(f64::total_cmp);
    eprintln!(
        "refined render after the drag: p50 {:.1} ms",
        refined[refined.len() / 2]
    );
    if std::env::var_os("DV_PROFILE").is_some() && en.has_timestamps() {
        // GPU time per kernel of one refined render after a move (each dispatch in its own
        // pass), after a draft as above.
        r.spots[0].source = [0.6, 0.7];
        for f in [focus, None] {
            let mut env = MaskEnv {
                geometry: Geometry::Frame {
                    w: 6000.0,
                    h: 4000.0,
                },
                image: (w, h),
                scale: 2560.0 / 6000.0,
                cache: &mut cache,
                sensor_bin: 1.0,
                overlay: None,
            };
            let g = Graph::build_masked(
                &retouch_edit(&r),
                OpRegistry::builtin(),
                f,
                None,
                Some(&mut env),
            )
            .unwrap();
            if f.is_none() {
                break;
            }
            let req = Request {
                scale: 2560.0 / 6000.0,
                ..Request::new(Roi::full(w, h))
            };
            let out = en.render(&g, &src, &req).unwrap();
            en.wait().unwrap();
            en.recycle(out);
        }
        let mut env = MaskEnv {
            geometry: Geometry::Frame {
                w: 6000.0,
                h: 4000.0,
            },
            image: (w, h),
            scale: 2560.0 / 6000.0,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        };
        let g = Graph::build_masked(
            &retouch_edit(&r),
            OpRegistry::builtin(),
            None,
            None,
            Some(&mut env),
        )
        .unwrap();
        let req = Request {
            scale: 2560.0 / 6000.0,
            profile: true,
            ..Request::new(Roi::full(w, h))
        };
        let out = en.render(&g, &src, &req).unwrap();
        let mut by: Vec<(String, f64, usize)> = Vec::new();
        for (k, ms) in out.stats.gpu_ms.iter().flatten() {
            match by.iter_mut().find(|(n, _, _)| n == k) {
                Some(e) => {
                    e.1 += ms;
                    e.2 += 1;
                }
                None => by.push((k.clone(), *ms, 1)),
            }
        }
        en.recycle(out);
        by.sort_by(|a, b| b.1.total_cmp(&a.1));
        let tot: f64 = by.iter().map(|e| e.1).sum();
        let nd: usize = by.iter().map(|e| e.2).sum();
        eprintln!("    total {tot:.1} ms in {nd} dispatches");
        for (k, ms, n) in by.iter().take(8) {
            eprintln!("    {k}: {ms:.1} ms ({n} dispatches)");
        }
    }
    times.sort_by(f64::total_cmp);
    let p50 = times[times.len() / 2];
    let p95 = times[times.len() * 95 / 100];
    eprintln!(
        "spot drag at 2560×1707 (10 spots) on {}: p50 {p50:.1} ms, p95 {p95:.1} ms",
        d.name
    );
    assert!(p95 < 500.0, "{p95}");
}

/// The source search avoids blemishes and follows the gradient's direction.
#[test]
fn source_search_finds_a_clean_match() {
    let (w, h) = (400u32, 300u32);
    let mut px = vec![0f32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            // Stripes that change left to right; a dark blob sits below the spot.
            let mut v = 0.3 + 0.2 * (x as f32 * 0.15).sin() + 0.4 * x as f32 / w as f32;
            if (x as f32 - 200.0).hypot(y as f32 - 225.0) < 25.0 {
                v *= 0.1;
            }
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[v, v, v, 1.0]);
        }
    }
    let view = View { w, h, px: &px };
    let got = best_source(
        &view,
        &Want {
            centre: [200.0, 150.0],
            radius: 15.0,
            path: &[],
            avoid: &[],
        },
    )
    .expect("a source");
    eprintln!("source for (200, 150): ({:.1}, {:.1})", got[0], got[1]);
    // Same brightness column (straight up or down), not onto the blob.
    assert!((got[0] - 200.0).abs() < 12.0, "{got:?}");
    assert!((got[0] - 200.0).hypot(got[1] - 225.0) > 40.0, "{got:?}");
    // Other spots are avoided.
    let avoid = [(got, 15.0)];
    let other = best_source(
        &view,
        &Want {
            centre: [200.0, 150.0],
            radius: 15.0,
            path: &[],
            avoid: &avoid,
        },
    )
    .unwrap();
    assert!((other[0] - got[0]).hypot(other[1] - got[1]) > 30.0);
}

/// Interactive renders keep each spot's solve: after spots are dragged (drafts) and a
/// source changes, the refined render equals a fresh engine's, bit for bit; spots left in
/// place are not solved again (fewer solve dispatches do work: their kernels return).
#[test]
fn kept_solves_equal_fresh_ones() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (480u32, 320u32);
    let mut cache = MaskCache::default();
    let mut en = engine(d);
    let req = Request {
        precision: Precision::F32,
        ..Request::new(Roi::full(w, h))
    };
    let mut render =
        |en: &mut Engine, src: &Source, r: &Retouch, focus: Option<(ModuleKind, u16)>| {
            let g = graph(&retouch_edit(r), src, &mut cache, focus);
            let out = en.render(&g, src, &req).unwrap();
            let px = en.read(&out).unwrap();
            en.recycle(out);
            px
        };
    let src = en.upload(w, h, &scene(w, h, 2)).unwrap();
    let mut r = all_kinds();
    render(&mut en, &src, &r, None);
    // Drag the first spot: drafts while dragging, then the refined render.
    for i in 0..3 {
        r.spots[0].source = [0.3 + 0.02 * i as f32, 0.75 - 0.01 * i as f32];
        render(&mut en, &src, &r, Some((ModuleKind::RETOUCH, 0)));
    }
    let kept = render(&mut en, &src, &r, None);
    let mut fresh_en = engine(d);
    let fresh_src = fresh_en.upload(w, h, &scene(w, h, 2)).unwrap();
    let fresh = render(&mut fresh_en, &fresh_src, &r, None);
    assert!(
        kept.iter()
            .zip(&fresh)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    // Another source (the input changed): every spot is solved again.
    let other = en.upload(w, h, &scene(w, h, 5)).unwrap();
    let kept = render(&mut en, &other, &r, None);
    let fresh_other = fresh_en.upload(w, h, &scene(w, h, 5)).unwrap();
    let fresh = render(&mut fresh_en, &fresh_other, &r, None);
    assert!(
        kept.iter()
            .zip(&fresh)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
    // A spot added: the scratch grows, so all are solved in a new one.
    r.spots.push(spot(
        SpotMode::Heal,
        Layer::All,
        [0.5, 0.5],
        [0.6, 0.3],
        0.03,
    ));
    let kept = render(&mut en, &other, &r, None);
    let fresh = render(&mut fresh_en, &fresh_other, &r, None);
    assert!(
        kept.iter()
            .zip(&fresh)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
}

/// The fused solve (tiles in workgroup memory, each spot's small levels in one workgroup,
/// the coarsest on a wavefront) gives the same bits as one dispatch per sweep.
#[test]
fn fused_solve_equals_the_steps() {
    use std::sync::atomic::Ordering;
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (1280u32, 960u32);
    let mut en = engine(d);
    let src = en.upload(w, h, &scene(w, h, 4)).unwrap();
    let req = Request {
        precision: Precision::F32,
        checkpoints: false,
        ..Request::new(Roi::full(w, h))
    };
    let mut r = all_kinds();
    // A large heal on the low layer: four tile levels.
    r.spots.push(spot(
        SpotMode::Heal,
        Layer::Low,
        [0.5, 0.5],
        [0.25, 0.3],
        0.2,
    ));
    // A small one: its whole solve in the tail.
    r.spots.push(spot(
        SpotMode::Fill,
        Layer::All,
        [0.1, 0.1],
        [0.2, 0.1],
        0.01,
    ));
    let p = placed(&r, w, h);
    let lay = Layout::new(&p);
    assert!(lay.chunks.iter().any(|c| c.tail >= 3), "tile levels");
    assert!(
        lay.chunks.iter().any(|c| c.g0.is_some() && c.tail == 0),
        "tail only"
    );
    for focus in [None, Some((ModuleKind::RETOUCH, 0))] {
        let mut out = Vec::new();
        for step in [true, false] {
            super::layout::STEPWISE.store(step, Ordering::Relaxed);
            let mut cache = MaskCache::default();
            let g = graph(&retouch_edit(&r), &src, &mut cache, focus);
            out.push(render(&mut en, &g, &src, &req));
        }
        super::layout::STEPWISE.store(false, Ordering::Relaxed);
        let differ = out[0]
            .iter()
            .zip(&out[1])
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        let worst = out[0]
            .iter()
            .zip(&out[1])
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        eprintln!("fused vs stepwise (focus {focus:?}): {differ} values differ, worst {worst:.2e}");
        assert_eq!(differ, 0, "worst {worst}");
    }
}

/// The tail size against the picture size. Every spot re-solves each run (all
/// sources move); refined (10 V-cycles) and draft (1) wall times, p50 of 7, per tail size.
/// Prints a table; run by hand:
///
///     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --lib tail_size_sweep -- \
///       --ignored --nocapture
#[test]
#[ignore = "timing report; run by hand"]
fn tail_size_sweep() {
    use std::sync::atomic::Ordering;
    let Some(d) = dev() else { return };
    let _l = lock();
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    eprintln!(
        "load {}",
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );
    for (w, h) in [(1280u32, 853u32), (2560, 1707), (5120, 3413)] {
        let img = scene(w, h, 3);
        let mut en = engine(d);
        let src = en.upload(w, h, &img).unwrap();
        for tail in [1024u32, 2048, 4096, 8192, 16384, 32768] {
            super::layout::TAIL_OVERRIDE.store(tail, Ordering::Relaxed);
            let mut r = all_kinds();
            let mut cache = MaskCache::default();
            let mut times = [Vec::new(), Vec::new()];
            for i in 0..9 {
                for s in &mut r.spots {
                    s.source[0] += if i % 2 == 0 { 0.002 } else { -0.002 };
                }
                for (k, focus) in [None, Some((ModuleKind::RETOUCH, 0))]
                    .into_iter()
                    .enumerate()
                {
                    let g = graph(&retouch_edit(&r), &src, &mut cache, focus);
                    let req = Request::new(Roi::full(w, h));
                    let t = std::time::Instant::now();
                    let out = en.render(&g, &src, &req).unwrap();
                    en.wait().unwrap();
                    en.recycle(out);
                    if i >= 2 {
                        times[k].push(t.elapsed().as_secs_f64() * 1e3);
                    }
                }
            }
            let p50 = |v: &mut Vec<f64>| {
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            };
            let (a, b) = (p50(&mut times[0]), p50(&mut times[1]));
            eprintln!("{w}×{h} tail {tail:>5}: refined {a:6.1} ms, draft {b:5.1} ms");
        }
        super::layout::TAIL_OVERRIDE.store(0, Ordering::Relaxed);
    }
}

/// A removal spot heals from its patch. The patch here is the scene without its
/// blemish, 10 % too bright everywhere (as an 8-bit network look could leave it): the heal
/// takes the surroundings' tones at the edge, so no seam shows and the blemish is gone.
/// GPU = the f64 reference; without the patch the spot heals from its source.
#[test]
fn removal_heals_from_its_patch_without_a_seam() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (240u32, 160u32);
    let img = scene(w, h, 3);
    let spot = Spot {
        mode: SpotMode::Remove,
        model: "big-lama".into(),
        target: [0.3, 0.4],
        source: [0.45, 0.4],
        size: 0.03,
        feather: 0.3,
        ..Spot::default()
    };
    let r = Retouch {
        spots: vec![spot.clone()],
        ..Retouch::default()
    };
    let e = retouch_edit(&r);
    // The clean scene (no blemishes), brighter by 10 %, over a frame rectangle.
    let clean = |x: u32, y: u32| {
        let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
        let tex = 0.04 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
        [
            0.15 + 0.35 * u + 0.1 * t + tex,
            0.2 + 0.25 * u + 0.2 * t + 0.5 * tex,
            0.3 + 0.1 * u + 0.15 * t - tex,
        ]
    };
    let (x0, y0, pw, ph) = (36u32, 40u32, 72u32, 48u32);
    let mut rgb = Vec::new();
    for j in 0..ph {
        for i in 0..pw {
            rgb.extend(clean(x0 + i, y0 + j).map(|v| v * 1.1));
        }
    }
    let p = super::patch::Patch {
        rect: [
            x0 as f32 / w as f32,
            y0 as f32 / h as f32,
            (x0 + pw) as f32 / w as f32,
            (y0 + ph) as f32 / h as f32,
        ],
        w: pw,
        h: ph,
        rgb,
    };
    let key = super::patch::key(&crate::mask::depth::photo(&e.source), &spot);
    super::patch::store().lock().unwrap().put(key, p);
    let mut en = engine(d);
    let src = en.upload(w, h, &img).unwrap();
    let mut cache = MaskCache::default();
    let g = graph(&e, &src, &mut cache, None);
    let node = g
        .nodes
        .iter()
        .find(|n| n.id.0 == ModuleKind::RETOUCH)
        .unwrap();
    let pl = Placed::from_fields(&node.params.to_fields()).unwrap();
    assert!(pl.spots[0].patch.is_some(), "the patch is placed");
    assert_eq!(pl.spots[0].mode, SpotMode::Heal);
    let gpu = render(&mut en, &g, &src, &exact(w, h));
    let cpu = cpu_ref::retouch(&Pic { w, h, px: &img }, &pl);
    let worst = gpu
        .iter()
        .zip(&cpu)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    eprintln!("removal GPU vs CPU: worst {worst:.2e}");
    assert!(worst < 1e-3, "{worst}");
    // The blemish is gone: the centre is the clean scene within 2 %, not 10 % off.
    let (cx, cy) = (72u32, 64u32);
    let i = 4 * (cy * w + cx) as usize;
    let want = clean(cx, cy);
    for k in 0..3 {
        let rel = (gpu[i + k] - want[k]).abs() / want[k];
        assert!(rel < 0.02, "channel {k}: {} vs {}", gpu[i + k], want[k]);
    }
    // No seam: across the spot's edge, steps stay within the scene's own.
    let r_px = 0.03 * 240.0;
    let (mut seam, mut own) = (0f32, 0f32);
    for a in 0..64 {
        let th = a as f32 / 64.0 * std::f32::consts::TAU;
        let at = |rad: f32| {
            let x = (cx as f32 + rad * th.cos()).round() as u32;
            let y = (cy as f32 + rad * th.sin()).round() as u32;
            4 * (y * w + x) as usize
        };
        let (p, q) = (at(r_px - 1.0), at(r_px + 1.0));
        seam = seam.max((gpu[p + 1] - gpu[q + 1]).abs());
        let (p, q) = (at(r_px + 2.0), at(r_px + 4.0));
        own = own.max((img[p + 1] - img[q + 1]).abs());
    }
    eprintln!("removal edge step {seam:.4}, the scene's own {own:.4}");
    assert!(seam < 2.0 * own + 0.01, "{seam} vs {own}");
    // Without its patch it heals from its source, like a heal spot.
    super::patch::store().lock().unwrap().remove(&key);
    let g2 = graph(&e, &src, &mut cache, None);
    let heal = Retouch {
        spots: vec![Spot {
            mode: SpotMode::Heal,
            model: String::new(),
            ..spot
        }],
        ..Retouch::default()
    };
    let g3 = graph(&retouch_edit(&heal), &src, &mut cache, None);
    assert_eq!(
        render(&mut en, &g2, &src, &exact(w, h)),
        render(&mut en, &g3, &src, &exact(w, h))
    );
}

/// The frequency separation's radius is in sensor pixels, so a thumbnail of a
/// raw binned 2×2 (its frame half the sensor's) places it as the full sensor does; spot
/// sizes (fractions of the frame) are the same either way.
#[test]
fn a_binned_frame_keeps_the_split_radius() {
    let r = Retouch {
        split_radius: 6.0,
        spots: vec![spot(
            SpotMode::Heal,
            Layer::High,
            [0.3, 0.4],
            [0.5, 0.45],
            0.02,
        )],
    };
    let image = (512, 341);
    let place = |w: f64, h: f64, bin: f64| {
        Placed::new(
            &r,
            &Placement {
                geometry: Geometry::Frame { w, h },
                image,
                draft: false,
                sensor_bin: bin,
                photo: [0; 32],
            },
        )
    };
    let full = place(2400.0, 1600.0, 1.0);
    let binned = place(1200.0, 800.0, 2.0);
    assert!(
        (full.split - binned.split).abs() < 1e-6,
        "{} vs {}",
        full.split,
        binned.split
    );
    assert!((f64::from(full.split) - 6.0 * 512.0 / 2400.0).abs() < 1e-3);
    assert_eq!(full.spots[0].radius, binned.spots[0].radius);
    assert_eq!(full.spots[0].centre, binned.spots[0].centre);
}
