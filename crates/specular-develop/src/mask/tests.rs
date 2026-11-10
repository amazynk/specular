//! Masks: GPU = CPU reference for every shape, range, combination and blend mode; feather
//! monotone; masks follow the warp; brush stamping incremental = full; preview vs export;
//! no new kernel when a mask moves; the brush dab budget.

use std::collections::HashMap;

use super::cpu_ref::{self, Ctx};
use super::*;
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request, Source};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::fields::Choice;
use crate::model::mask::{
    Blend, BlendMode, ColourRange, Combine, MaskExpr, MaskShape, PathPoint, Range, RangeChannel,
    Refine, Shape, Stroke,
};
use crate::model::params::{CropOrientation, Exposure, ModuleParams};
use crate::model::{Edit, ModuleInstance, ModuleKind, Params, Registry};
use crate::ops::geometry::{Context, LensParams, Plan};
use crate::preview::Orient;
use crate::testgpu::{dev, lock};

/// A small deterministic generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f32 {
        (lo + (hi - lo) * self.next()) as f32
    }
}

/// Colourful smooth content over many hues and five stops of luminance.
fn image(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let k = 0.03 * (5.0 * t).exp2();
            v.extend_from_slice(&[
                k * (0.6 + 0.5 * (u * 9.0).sin()),
                k * (0.6 + 0.5 * (u * 7.0 + 2.0).sin()),
                k * (0.6 + 0.5 * (u * 5.0 + 4.0).sin()),
                1.0,
            ]);
        }
    }
    v
}

fn shapes() -> Vec<MaskShape> {
    let star: Vec<PathPoint> = (0..10)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 10.0;
            let r = if i % 2 == 0 { 0.3 } else { 0.14 };
            PathPoint {
                x: 0.5 + r * a.cos() / 1.0,
                y: 0.5 + r * a.sin() * 4.0 / 3.0,
                feather: 0.01 * (i % 3) as f32,
            }
        })
        .collect();
    vec![
        MaskShape {
            id: 1,
            shape: Shape::Ellipse {
                centre: [0.4, 0.55],
                radii: [0.25, 0.12],
                angle_deg: 25.0,
                feather: 0.4,
            },
        },
        MaskShape {
            id: 2,
            shape: Shape::Rectangle {
                centre: [0.6, 0.4],
                half: [0.2, 0.1],
                angle_deg: -15.0,
                round: 0.4,
                feather: 0.3,
            },
        },
        MaskShape {
            id: 3,
            shape: Shape::Polygon {
                points: star,
                closed: true,
            },
        },
        MaskShape {
            id: 4,
            shape: Shape::Linear {
                start: [0.2, 0.2],
                end: [0.7, 0.6],
            },
        },
        MaskShape {
            id: 5,
            shape: Shape::Radial {
                centre: [0.5, 0.5],
                inner: [0.08, 0.05],
                outer: [0.35, 0.25],
                angle_deg: 30.0,
            },
        },
        MaskShape {
            id: 6,
            shape: Shape::Brush {
                strokes: vec![
                    Stroke {
                        radius: 0.06,
                        hardness: 0.3,
                        flow: 0.8,
                        erase: false,
                        pressure: crate::model::mask::PressureUse::Flow,
                        points: vec![[0.1, 0.2, 1.0], [0.5, 0.3, 0.8], [0.8, 0.7, 0.6]],
                    },
                    Stroke {
                        radius: 0.03,
                        hardness: 0.8,
                        flow: 1.0,
                        erase: true,
                        pressure: crate::model::mask::PressureUse::Flow,
                        points: vec![[0.3, 0.1, 1.0], [0.4, 0.6, 1.0]],
                    },
                ],
            },
        },
    ]
}

fn sh(id: u32) -> Box<MaskExpr> {
    Box::new(MaskExpr::Shape(id))
}

fn exprs() -> Vec<(&'static str, MaskExpr)> {
    let lum = MaskExpr::Range(Range {
        channel: RangeChannel::Luminance,
        on_output: false,
        limits: [-4.0, -2.5, -0.5, 1.0],
    });
    let hue = MaskExpr::Range(Range {
        channel: RangeChannel::Hue,
        on_output: true,
        limits: [300.0, 330.0, 400.0, 430.0],
    });
    let chroma = MaskExpr::Range(Range {
        channel: RangeChannel::Chroma,
        on_output: false,
        limits: [0.0, 0.0, 0.02, 0.05],
    });
    let green = MaskExpr::Range(Range {
        channel: RangeChannel::Green,
        on_output: false,
        limits: [0.02, 0.05, 0.2, 0.4],
    });
    let colour = MaskExpr::ColourRange(ColourRange {
        colours: vec![
            crate::colour::oklab::from_rec2020_f32([0.1, 0.05, 0.02]),
            crate::colour::oklab::from_rec2020_f32([0.02, 0.06, 0.1]),
        ],
        distance: 0.04,
        falloff: 0.06,
    });
    let tree = MaskExpr::Combine(
        Combine::Intersect,
        Box::new(MaskExpr::Opacity(
            0.7,
            Box::new(MaskExpr::Combine(
                Combine::Subtract,
                Box::new(MaskExpr::Combine(Combine::Union, sh(1), sh(2))),
                Box::new(MaskExpr::Invert(sh(4))),
            )),
        )),
        Box::new(MaskExpr::Combine(
            Combine::Exclude,
            Box::new(lum.clone()),
            sh(5),
        )),
    );
    vec![
        ("all", MaskExpr::All),
        ("ellipse", MaskExpr::Shape(1)),
        ("rectangle", MaskExpr::Shape(2)),
        ("polygon", MaskExpr::Shape(3)),
        ("linear", MaskExpr::Shape(4)),
        ("radial", MaskExpr::Shape(5)),
        ("brush", MaskExpr::Shape(6)),
        ("luminance", lum),
        ("hue", hue),
        ("chroma", chroma),
        ("green", green),
        ("colour", colour),
        ("tree", tree),
        ("missing", MaskExpr::Shape(99)),
    ]
}

fn exposure(ev: f32, blend: Option<Blend>) -> ModuleInstance {
    let mut m = ModuleInstance::with_params(
        Exposure {
            exposure_ev: ev,
            ..Exposure::default()
        },
        0,
    );
    m.blend = blend;
    m
}

fn edit_with(blend: Blend, masks: Vec<MaskShape>) -> Edit {
    let mut e = Edit::empty(Default::default());
    e.modules.push(exposure(1.5, Some(blend)));
    e.masks = masks;
    e
}

fn engine(d: &crate::testgpu::Dev) -> Engine {
    Engine::new(&d.device, &d.queue, Budget::fixed(1 << 30))
}

struct Run<'a> {
    engine: &'a mut Engine,
    cache: &'a mut MaskCache,
}

impl Run<'_> {
    fn render(
        &mut self,
        edit: &Edit,
        src: &Source,
        geometry: Geometry<'_>,
        overlay: Option<((ModuleKind, u16), Overlay)>,
    ) -> (Vec<f32>, crate::gpu::RenderStats) {
        let mut env = MaskEnv {
            geometry,
            image: (src.width, src.height),
            scale: 1.0,
            cache: self.cache,
            sensor_bin: 1.0,
            overlay,
        };
        let g = Graph::build_masked(edit, OpRegistry::builtin(), None, None, Some(&mut env))
            .expect("graph");
        let req = Request {
            precision: Precision::F32,
            ..Request::new(Roi::full(src.width, src.height))
        };
        let r = self.engine.render(&g, src, &req).expect("render");
        let stats = r.stats.clone();
        let px = self.engine.read(&r).expect("read");
        self.engine.recycle(r);
        (px, stats)
    }
}

/// The CPU reference of `edit` (one masked exposure) on `img`.
fn cpu(edit: &Edit, img: &[f32], w: u32, h: u32, cache: &MaskCache) -> Vec<[f64; 3]> {
    let space = Space::of(f64::from(w), f64::from(h));
    let rasters: HashMap<u32, &Raster> = edit
        .masks
        .iter()
        .filter_map(|m| cache.raster(m.id).map(|r| (m.id, r)))
        .collect();
    let photo = super::depth::photo(&edit.source);
    let held: Vec<([u8; 32], std::sync::Arc<Raster>)> = super::ai::rows(edit)
        .into_iter()
        .map(|a| a.key)
        .chain(
            super::depth::models(edit)
                .iter()
                .map(|m| super::depth::key(&photo, m)),
        )
        .filter_map(|k| {
            let r = super::ai::store().lock().unwrap().get(&k)?;
            Some((k, r))
        })
        .collect();
    let cx = Ctx {
        edit,
        space,
        aa: 1.0 / f64::from(w.max(h)),
        rasters,
        ai: held.iter().map(|(k, r)| (*k, &**r)).collect(),
        detail: Default::default(),
    };
    let m = &edit.modules[0];
    let ev = match &m.params {
        Params::Exposure(p) => p.exposure_ev,
        _ => 0.0,
    };
    let g = (f64::from(ev)).exp2() as f32;
    let bl = m.blend.as_ref().expect("blend");
    (0..(w * h) as usize)
        .map(|i| {
            let (x, y) = ((i as u32 % w) as f64, (i as u32 / w) as f64);
            let a = [img[4 * i], img[4 * i + 1], img[4 * i + 2]];
            let b = a.map(|v| v * g);
            let f = ((x + 0.5) / f64::from(w), (y + 0.5) / f64::from(h));
            cpu_ref::blend_pixel(&cx, bl, f, a, b)
        })
        .collect()
}

fn worst(gpu: &[f32], cpu: &[[f64; 3]]) -> (f64, usize) {
    let mut worst = 0f64;
    let mut over = 0;
    for (i, c) in cpu.iter().enumerate() {
        for k in 0..3 {
            let d = (f64::from(gpu[4 * i + k]) - c[k]).abs() / c[k].abs().max(0.05);
            if d > 2e-3 {
                over += 1;
            }
            worst = worst.max(d);
        }
    }
    (worst, over)
}

#[test]
fn gpu_masks_equal_the_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (256u32, 192u32);
    let img = image(w, h);
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    for (name, e) in exprs() {
        let edit = edit_with(
            Blend {
                mask: e,
                ..Blend::default()
            },
            shapes(),
        );
        let mut run = Run {
            engine: &mut engine,
            cache: &mut cache,
        };
        let (gpu, _) = run.render(&edit, &src, geometry, None);
        let want = cpu(&edit, &img, w, h, &cache);
        let (worst, over) = worst(&gpu, &want);
        eprintln!("mask {name}: worst relative {worst:.2e}, {over} values over 2e-3");
        assert_eq!(over, 0, "{name}: worst {worst}");
    }
}

#[test]
fn gpu_blend_modes_equal_the_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (160u32, 120u32);
    let img = image(w, h);
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    for (i, (mode, _)) in <BlendMode as crate::model::fields::Choice>::ALL
        .iter()
        .enumerate()
    {
        let edit = edit_with(
            Blend {
                mode: *mode,
                opacity: 0.8,
                mask: MaskExpr::Shape(1),
                invert: i % 2 == 1,
                refine: Refine {
                    contrast: 0.3,
                    offset: -0.1,
                    ..Refine::default()
                },
            },
            shapes(),
        );
        let mut run = Run {
            engine: &mut engine,
            cache: &mut cache,
        };
        let (gpu, _) = run.render(&edit, &src, geometry, None);
        let want = cpu(&edit, &img, w, h, &cache);
        let (worst, over) = worst(&gpu, &want);
        eprintln!("blend {}: worst relative {worst:.2e}", mode.name());
        assert_eq!(over, 0, "{}: worst {worst}", mode.name());
    }
}

/// Along every ray out of a shape's centre the mask never rises, and more feather never
/// adds mask.
#[test]
fn feather_is_monotone() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let space = Space::of(3.0, 2.0);
    for case in 0..400 {
        let c = [rng.range(0.2, 0.8), rng.range(0.2, 0.8)];
        let feather = rng.range(0.0, 1.0);
        let angle = rng.range(-180.0, 180.0);
        let (r0, r1) = (rng.range(0.02, 0.3), rng.range(0.02, 0.3));
        let sides = 3 + (case % 7);
        let shape = |feather: f32| match case % 5 {
            0 => Shape::Ellipse {
                centre: c,
                radii: [r0, r1],
                angle_deg: angle,
                feather,
            },
            1 => Shape::Rectangle {
                centre: c,
                half: [r0, r1],
                angle_deg: angle,
                round: rng_round(case),
                feather,
            },
            2 => Shape::Polygon {
                // A regular polygon: convex, its centre the farthest point from the edges.
                points: (0..sides)
                    .map(|i| {
                        let a =
                            (angle.to_radians()) + i as f32 * std::f32::consts::TAU / sides as f32;
                        PathPoint {
                            x: c[0] + r0 * a.cos() / space.ax as f32,
                            y: c[1] + r0 * a.sin() / space.ay as f32,
                            feather: feather * r0 * 0.5,
                        }
                    })
                    .collect(),
                closed: true,
            },
            3 => Shape::Radial {
                centre: c,
                inner: [r0 * 0.3 * (1.0 - feather), r1 * 0.3 * (1.0 - feather)],
                outer: [r0, r1],
                angle_deg: angle,
            },
            _ => Shape::Linear {
                start: c,
                end: [c[0] + r0, c[1] + r1 * (1.0 + feather)],
            },
        };
        let s = shape(feather);
        let s2 = shape((feather + 0.3).min(1.0));
        let cc = space.pos(c);
        for k in 0..16 {
            let a = k as f64 * std::f64::consts::TAU / 16.0;
            let dir = if case % 5 == 4 {
                // A linear gradient falls along start → end.
                let e = space.pos(match &s {
                    Shape::Linear { end, .. } => *end,
                    _ => c,
                });
                let (dx, dy) = (e.0 - cc.0, e.1 - cc.1);
                let n = dx.hypot(dy);
                (dx / n, dy / n)
            } else {
                (a.cos(), a.sin())
            };
            let mut last = f64::INFINITY;
            for i in 0..300 {
                let t = i as f64 * 0.002;
                let p = (cc.0 + dir.0 * t, cc.1 + dir.1 * t);
                let f = (p.0 / space.ax, p.1 / space.ay);
                let m = cpu_ref::shape_mask(&s, space, f, 1e-3, None);
                assert!(
                    m <= last + 1e-12,
                    "case {case} ray {k} step {i}: {m} > {last}"
                );
                if case % 5 != 3 && case % 5 != 4 {
                    let m2 = cpu_ref::shape_mask(&s2, space, f, 1e-3, None);
                    assert!(m2 <= m + 1e-12, "case {case}: more feather added mask");
                }
                last = m;
            }
        }
    }
}

fn rng_round(case: u32) -> f32 {
    (case % 4) as f32 * 0.3
}

/// A brush dab is monotone around its centre too, and stamping point by point equals
/// stamping the whole stroke.
#[test]
fn brush_incremental_equals_full() {
    let strokes = match &shapes()[5].shape {
        Shape::Brush { strokes } => strokes.clone(),
        _ => unreachable!(),
    };
    let mut id = 0;
    let mut inc = brush::BrushEntry::new(&mut id, 300, 200, 300);
    let mut partial: Vec<Stroke> = Vec::new();
    for s in &strokes {
        partial.push(Stroke {
            points: Vec::new(),
            ..s.clone()
        });
        for p in &s.points {
            partial.last_mut().expect("stroke").points.push(*p);
            inc.update(&partial, 300, 200, 300, &mut id);
            assert!(
                inc.raster.dirty.is_some(),
                "an appended dab uploads rows only"
            );
        }
    }
    let mut full = brush::BrushEntry::new(&mut id, 300, 200, 300);
    full.update(&strokes, 300, 200, 300, &mut id);
    assert_eq!(inc.raster.data, full.raster.data);
    // Undo (a shorter list) restamps everything.
    inc.update(&strokes[..1], 300, 200, 300, &mut id);
    assert!(inc.raster.dirty.is_none());
    // One dab falls off from its centre.
    let r = &full.raster;
    let mut single = brush::BrushEntry::new(&mut id, 300, 200, 300);
    single.update(
        &[Stroke {
            radius: 0.1,
            hardness: 0.2,
            flow: 1.0,
            erase: false,
            pressure: crate::model::mask::PressureUse::Flow,
            points: vec![[0.5, 0.5, 1.0]],
        }],
        300,
        200,
        300,
        &mut id,
    );
    let s = &single.raster;
    for x in 150..185 {
        assert!(s.texel(x + 1, 100) <= s.texel(x, 100));
    }
    assert!(r.data.iter().any(|&v| v > 0));
}

#[test]
fn codec_and_hash_keep_the_new_fields() {
    let mut e = edit_with(
        Blend {
            mode: BlendMode::SoftLight,
            opacity: 0.5,
            mask: MaskExpr::Opacity(0.25, sh(2)),
            invert: true,
            refine: Refine::default(),
        },
        shapes(),
    );
    let b = crate::model::codec::encode_edit(&e);
    let (back, _) = crate::model::codec::decode_edit(&b, Registry::builtin()).expect("decode");
    assert_eq!(back, e);
    let x = crate::sidecar::to_xmp(&e, None);
    let back = crate::sidecar::from_xmp(&x, Registry::builtin()).expect("sidecar");
    assert_eq!(back.edit.modules, e.modules);
    assert_eq!(back.edit.masks, e.masks);
    let h0 = crate::model::hash::edit_hash(&e);
    e.modules[0].blend.as_mut().expect("blend").invert = false;
    assert_ne!(h0, crate::model::hash::edit_hash(&e));
    // A rectangle moving changes the hash of the module that uses it.
    let h1 = crate::model::hash::edit_hash(&e);
    if let Shape::Rectangle { centre, .. } = &mut e.masks[1].shape {
        centre[0] += 0.01;
    }
    assert_ne!(h1, crate::model::hash::edit_hash(&e));
}

fn warp_edit(blend: Option<Blend>) -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), Default::default(), false);
    let m = e
        .module_mut(ModuleKind::CROP_ORIENTATION, 0)
        .expect("crop module");
    m.params = Params::CropOrientation(CropOrientation {
        angle_deg: 7.0,
        crop: [0.08, 0.1, 0.9, 0.85],
        ..CropOrientation::default()
    });
    let mut lens = ModuleInstance::new(Registry::builtin(), ModuleKind::LENS, 0);
    lens.params = Params::Other(
        ModuleKind::LENS,
        LensParams {
            embedded: false,
            auto_scale: false,
            distortion: 40.0,
            ..LensParams::default()
        }
        .to_fields(),
    );
    e.modules.push(lens);
    // Only the warp and one masked exposure.
    e.modules
        .retain(|m| matches!(m.kind, ModuleKind::CROP_ORIENTATION | ModuleKind::LENS));
    e.modules.push(exposure(-4.0, blend));
    e.sort(Registry::builtin());
    e.masks = vec![MaskShape {
        id: 1,
        shape: Shape::Ellipse {
            centre: [0.3, 0.4],
            radii: [10.0 / 600.0, 10.0 / 600.0],
            angle_deg: 0.0,
            feather: 0.0,
        },
    }];
    e
}

/// The grid of the fused warp against the exact map, in output pixels of a 6000 px wide
/// export.
#[test]
fn coordinate_grid_tracks_the_warp() {
    let e = warp_edit(None);
    let ctx = Context::image(6000, 4000, Orient::default());
    let plan = Plan::new(&e, &ctx, true);
    let g = Geometry::Warp(&plan);
    let grid = make_grid(&g);
    let n = GRID as usize;
    let (ow, oh) = plan.output_size(1.0);
    let mut rng = Rng(12345);
    let mut worst = 0f64;
    for _ in 0..5000 {
        let (u, v) = (rng.next(), rng.next());
        let (fx, fy) = (u * (n - 1) as f64, v * (n - 1) as f64);
        let (i0, j0) = ((fx as usize).min(n - 2), (fy as usize).min(n - 2));
        let (tx, ty) = (fx - i0 as f64, fy - j0 as f64);
        let at = |i: usize, j: usize, k: usize| f64::from(grid[2 * (j * n + i) + k]);
        let lerp = |k: usize| {
            let top = at(i0, j0, k) * (1.0 - tx) + at(i0 + 1, j0, k) * tx;
            let bot = at(i0, j0 + 1, k) * (1.0 - tx) + at(i0 + 1, j0 + 1, k) * tx;
            top * (1.0 - ty) + bot * ty
        };
        let exact = g.frame_point(u, v);
        // In source pixels (≈ output pixels at scale 1).
        let d = ((lerp(0) - exact.0) * 6000.0).hypot((lerp(1) - exact.1) * 4000.0);
        worst = worst.max(d);
    }
    eprintln!("grid vs exact warp: worst {worst:.4} px at {ow}×{oh}");
    assert!(worst < 0.25, "{worst}");
}

/// A shape placed on a feature stays on it after rotate, crop and lens correction.
#[test]
fn masks_follow_the_warp() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (sw, sh) = (600u32, 400u32);
    let mut img = vec![0f32; (sw * sh * 4) as usize];
    for y in 0..sh {
        for x in 0..sw {
            let (dx, dy) = (
                x as f32 + 0.5 - 0.3 * sw as f32,
                y as f32 + 0.5 - 0.4 * sh as f32,
            );
            let v = if dx.hypot(dy) < 10.0 { 1.0 } else { 0.18 };
            let i = ((y * sw + x) * 4) as usize;
            img[i..i + 4].copy_from_slice(&[v, v, v, 1.0]);
        }
    }
    let mut engine = engine(d);
    let mut warper = crate::ops::geometry::warp::Warper::new(&d.device, &d.queue);
    let src = engine.upload(sw, sh, &img).expect("upload");
    let blend = Blend {
        mask: MaskExpr::Shape(1),
        ..Blend::default()
    };
    let masked = warp_edit(Some(blend));
    let mut plain = warp_edit(None);
    for m in &mut plain.modules {
        if m.kind == ModuleKind::EXPOSURE {
            m.params = exposure(0.0, None).params;
        }
    }
    let ctx = Context::image(sw, sh, Orient::default());
    let plan = Plan::new(&masked, &ctx, true);
    let (ow, oh) = plan.output_size(1.0);
    let out = warper
        .run(
            &plan,
            src.buffer(),
            sw,
            sh,
            Precision::F32,
            ow,
            oh,
            Precision::F32,
        )
        .expect("warp");
    let warped = engine
        .source_from_buffer(out, ow, oh, Precision::F32)
        .expect("source");
    let mut cache = MaskCache::default();
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    let (a, _) = run.render(&plain, &warped, Geometry::Warp(&plan), None);
    let (b, _) = run.render(&masked, &warped, Geometry::Warp(&plan), None);
    let centroid = |wt: &dyn Fn(usize) -> f64| {
        let (mut sx, mut sy, mut s) = (0.0, 0.0, 0.0);
        for i in 0..(ow * oh) as usize {
            let w = wt(i);
            sx += w * ((i as u32 % ow) as f64 + 0.5);
            sy += w * ((i as u32 / ow) as f64 + 0.5);
            s += w;
        }
        (sx / s, sy / s)
    };
    // The feature: brighter than the grey around it.
    let feat = centroid(&|i| (f64::from(a[4 * i + 1]) - 0.2).max(0.0));
    // The mask: what the masked exposure took away (weighted off the feature's grey).
    let mask = centroid(&|i| {
        f64::from(a[4 * i + 1] - b[4 * i + 1]).max(0.0) / f64::from(a[4 * i + 1]).max(1e-6)
    });
    let expect = plan
        .to_output((0.3 * f64::from(sw), 0.4 * f64::from(sh)), 1.0)
        .expect("inverse");
    let d1 = (feat.0 - mask.0).hypot(feat.1 - mask.1);
    let d2 = (expect.0 - mask.0).hypot(expect.1 - mask.1);
    eprintln!(
        "feature at {:.2},{:.2}; mask at {:.2},{:.2}; to_output {:.2},{:.2}: {d1:.3} / {d2:.3} px",
        feat.0, feat.1, mask.0, mask.1, expect.0, expect.1
    );
    assert!(d1 < 0.5 && d2 < 1.0);
}

/// Overlay and refine run; the refined mask follows an edge of the image.
#[test]
fn overlay_and_refine() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (200u32, 100u32);
    // Dark left half, bright right half.
    let mut img = vec![0f32; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        let v = if (i as u32 % w) < w / 2 { 0.02 } else { 0.6 };
        img[4 * i..4 * i + 4].copy_from_slice(&[v, v, v, 1.0]);
    }
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let masks = vec![MaskShape {
        id: 1,
        shape: Shape::Linear {
            start: [0.3, 0.5],
            end: [0.7, 0.5],
        },
    }];
    let id = (ModuleKind::EXPOSURE, 0);
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    let raw = edit_with(
        Blend {
            mask: MaskExpr::Shape(1),
            ..Blend::default()
        },
        masks.clone(),
    );
    let (grey, _) = run.render(&raw, &src, geometry, Some((id, Overlay::Grey)));
    let refined = edit_with(
        Blend {
            mask: MaskExpr::Shape(1),
            refine: Refine {
                edge_aware: 1.0,
                ..Refine::default()
            },
            ..Blend::default()
        },
        masks,
    );
    let (rgrey, _) = run.render(&refined, &src, geometry, Some((id, Overlay::Grey)));
    let row = h / 2;
    let at = |v: &[f32], x: u32| v[((row * w + x) * 4) as usize];
    // The raw gradient: 1 at 30 %, 0 at 70 %.
    assert!((at(&grey, 50) - 1.0).abs() < 1e-4);
    assert!(at(&grey, 150) < 1e-4);
    let step = |v: &[f32]| (at(v, w / 2 - 2) - at(v, w / 2 + 1)).abs();
    eprintln!(
        "mask step across the edge: raw {:.3}, refined {:.3}",
        step(&grey),
        step(&rgrey)
    );
    assert!(step(&rgrey) > 1.8 * step(&grey));
    let (tint, _) = run.render(&raw, &src, geometry, Some((id, Overlay::Tint)));
    assert!(tint[((row * w + 50) * 4) as usize] > tint[((row * w + 50) * 4 + 1) as usize]);
}

/// Moving masks, adding shapes, painting and changing modes make no kernel.
#[test]
fn mask_changes_build_no_kernel() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (128u32, 96u32);
    let img = image(w, h);
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let mut e = edit_with(
        Blend {
            mask: MaskExpr::Shape(1),
            ..Blend::default()
        },
        shapes(),
    );
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    run.render(&e, &src, geometry, None);
    type Change = Box<dyn Fn(&mut Edit)>;
    let mut steps: Vec<Change> = vec![
        Box::new(|e| {
            if let Shape::Ellipse { centre, .. } = &mut e.masks[0].shape {
                centre[0] += 0.05;
            }
        }),
        Box::new(|e| e.modules[0].blend.as_mut().expect("b").opacity = 0.4),
        Box::new(|e| e.modules[0].blend.as_mut().expect("b").mode = BlendMode::Overlay),
        Box::new(|e| {
            e.modules[0].blend.as_mut().expect("b").mask =
                MaskExpr::Combine(Combine::Union, sh(1), sh(6));
        }),
        Box::new(|e| {
            if let Shape::Brush { strokes } = &mut e.masks[5].shape {
                strokes[0].points.push([0.9, 0.9, 1.0]);
            }
        }),
        Box::new(|e| {
            if let Shape::Rectangle { half, .. } = &mut e.masks[1].shape {
                half[0] = 0.3;
            }
            e.modules[0].blend.as_mut().expect("b").mask = MaskExpr::Invert(sh(2));
        }),
    ];
    for (i, f) in steps.drain(..).enumerate() {
        f(&mut e);
        let (_, stats) = run.render(&e, &src, geometry, None);
        assert_eq!(stats.pipelines_created, 0, "step {i} built a kernel");
    }
}

/// The mask of a fit-view preview matches the full-size render reduced to its size.
#[test]
fn preview_matches_export_scale() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (1024u32, 682u32);
    let full = image(w, h);
    let half = crate::ops::tone::tests::shrink(&full, w, h, 2);
    let mut engine = engine(d);
    let sf = engine.upload(w, h, &full).expect("upload");
    let sh2 = engine.upload(w / 2, h / 2, &half).expect("upload");
    let e = edit_with(
        Blend {
            mask: MaskExpr::Combine(
                Combine::Union,
                Box::new(MaskExpr::Combine(Combine::Union, sh(1), sh(6))),
                sh(3),
            ),
            ..Blend::default()
        },
        shapes(),
    );
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let mut c1 = MaskCache::default();
    let (a, _) = Run {
        engine: &mut engine,
        cache: &mut c1,
    }
    .render(&e, &sf, geometry, None);
    let mut c2 = MaskCache::default();
    let (b, _) = Run {
        engine: &mut engine,
        cache: &mut c2,
    }
    .render(&e, &sh2, geometry, None);
    let a = crate::ops::tone::tests::shrink(&a, w, h, 2);
    let mut de: Vec<f64> = (0..(w / 2 * h / 2) as usize)
        .map(|i| {
            let p = |v: &[f32]| {
                crate::ops::tone::tests::lab(
                    [v[4 * i], v[4 * i + 1], v[4 * i + 2]].map(|x| x.min(1.0)),
                )
            };
            crate::ops::tone::tests::de2000(p(&a), p(&b))
        })
        .collect();
    de.sort_by(f64::total_cmp);
    let p95 = de[de.len() * 95 / 100];
    let p99 = de[de.len() * 99 / 100];
    eprintln!("preview vs export ΔE2000: p95 {p95:.3}, p99 {p99:.3}");
    assert!(p95 < 1.0, "{p95}");
}

/// Brush feedback on a 24 MP photo's fit-view preview (2560 px): stamp, upload and render
/// per dab. Timings are printed; the bound here is loose (the machine may be loaded).
#[test]
fn brush_dab_latency() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (2560u32, 1707u32);
    let img = image(w, h);
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: 6000.0,
        h: 4000.0,
    };
    let mut e = edit_with(
        Blend {
            mask: MaskExpr::Shape(1),
            ..Blend::default()
        },
        vec![MaskShape {
            id: 1,
            shape: Shape::Brush {
                strokes: vec![Stroke {
                    radius: 0.02,
                    hardness: 0.5,
                    flow: 0.6,
                    erase: false,
                    pressure: crate::model::mask::PressureUse::Flow,
                    points: vec![[0.2, 0.5, 1.0]],
                }],
            },
        }],
    );
    let focus = Some((ModuleKind::EXPOSURE, 0));
    let mut times = Vec::new();
    for i in 0..80 {
        if let Shape::Brush { strokes } = &mut e.masks[0].shape {
            strokes[0].points.push([
                0.2 + 0.007 * i as f32,
                0.5 + 0.1 * (i as f32 * 0.2).sin(),
                1.0,
            ]);
        }
        let t = std::time::Instant::now();
        let mut env = MaskEnv {
            geometry,
            image: (w, h),
            scale: 2560.0 / 6000.0,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        };
        let g = Graph::build_masked(&e, OpRegistry::builtin(), focus, None, Some(&mut env))
            .expect("graph");
        let r = engine
            .render(
                &g,
                &src,
                &Request {
                    scale: 2560.0 / 6000.0,
                    ..Request::new(Roi::full(w, h))
                },
            )
            .expect("render");
        drop(g);
        engine.wait().expect("wait");
        engine.recycle(r);
        if i >= 5 {
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    times.sort_by(f64::total_cmp);
    let p50 = times[times.len() / 2];
    let p95 = times[times.len() * 95 / 100];
    eprintln!("brush dab at 2560×1707: p50 {p50:.1} ms, p95 {p95:.1} ms");
    assert!(p95 < 250.0, "{p95}");
}

#[test]
fn panel_rows_round_trip() {
    use crate::model::mask::Layer;
    let mut rows = vec![
        Layer::new(MaskExpr::Shape(1)),
        Layer::new(MaskExpr::Shape(2)),
    ];
    rows[1].op = Combine::Subtract;
    rows[1].invert = true;
    rows[1].opacity = 0.5;
    let e = MaskExpr::from_layers(&rows);
    assert_eq!(e.layers().expect("rows"), rows);
    assert_eq!(MaskExpr::All.layers(), Some(Vec::new()));
    assert_eq!(MaskExpr::from_layers(&[]), MaskExpr::All);
    // A combination on the right lists as one row, a group.
    let group = MaskExpr::Combine(
        Combine::Union,
        sh(1),
        Box::new(MaskExpr::Combine(Combine::Union, sh(2), sh(3))),
    );
    assert_eq!(group.layers().map(|r| r.len()), Some(2));
    // A tree the panel cannot list.
    let odd = MaskExpr::Combine(Combine::Union, Box::new(MaskExpr::All), sh(3));
    assert!(odd.layers().is_none());
}

mod refine_brush;

/// An AI row renders its computed raster (here the left 40 % of the frame), combines
/// with other rows like a brush, and masks nothing (with an issue) until it is computed.
#[test]
fn ai_rows_render_their_raster() {
    use crate::model::mask::{AiKind, AiMask};
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (200u32, 120u32);
    let img = image(w, h);
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let mut row = AiMask::new(AiKind::Subject, "isnet-general-use");
    ai::rekey_one(&mut row, &[0x5a; 32]);
    let flat = |e: MaskExpr| {
        edit_with(
            Blend {
                mask: e,
                ..Blend::default()
            },
            shapes(),
        )
    };
    // Not computed: nothing changes and the panel is told.
    ai::store().lock().unwrap().remove(&row.key);
    let edit = flat(MaskExpr::Ai(row.clone()));
    let mut env = MaskEnv {
        geometry,
        image: (w, h),
        scale: 1.0,
        cache: &mut cache,
        sensor_bin: 1.0,
        overlay: None,
    };
    let c = compile(&edit, &mut env).expect("compiled");
    assert!(c.issues.iter().any(|(_, i)| *i == Issue::AiMask));
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    let (none, _) = run.render(&edit, &src, geometry, None);
    let (plain, _) = run.render(&Edit::empty(Default::default()), &src, geometry, None);
    let diff = none
        .iter()
        .zip(&plain)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(
        diff < 1e-6,
        "an uncomputed AI row changed the picture by {diff}"
    );
    // Computed: the left part is exposed, the right part is not.
    let (rw, rh) = (100u32, 60u32);
    let tex: Vec<u16> = (0..rw * rh)
        .map(|i| if i % rw < 40 { 65535 } else { 0 })
        .collect();
    ai::store()
        .lock()
        .unwrap()
        .put(row.key, ai::raster(rw, rh, &tex).unwrap());
    let c = compile(
        &edit,
        &mut MaskEnv {
            geometry,
            image: (w, h),
            scale: 1.0,
            cache: run.cache,
            sensor_bin: 1.0,
            overlay: None,
        },
    )
    .expect("compiled");
    assert!(c.issues.is_empty(), "{:?}", c.issues);
    // The implicit edge refinement is on for an AI row.
    let nb = c.blends.values().next().unwrap();
    assert!(nb.refine.as_ref().is_some_and(|r| r.radius_per_scale > 0.0));
    let (gpu, _) = run.render(&edit, &src, geometry, None);
    let at = |v: &[f32], x: u32, y: u32| v[((y * w + x) * 4) as usize];
    let gain = |x: u32| at(&gpu, x, 60) / at(&plain, x, 60).max(1e-6);
    assert!(gain(20) > 2.5, "left: {}", gain(20));
    assert!((gain(180) - 1.0).abs() < 1e-3, "right: {}", gain(180));
    // Subtracting it from "everywhere" swaps the sides.
    let edit = flat(MaskExpr::Combine(
        Combine::Subtract,
        Box::new(MaskExpr::All),
        Box::new(MaskExpr::Ai(row.clone())),
    ));
    let (sub, _) = run.render(&edit, &src, geometry, None);
    let gain = |x: u32| at(&sub, x, 60) / at(&plain, x, 60).max(1e-6);
    assert!((gain(20) - 1.0).abs() < 1e-3, "left: {}", gain(20));
    assert!(gain(180) > 2.5, "right: {}", gain(180));
    ai::store().lock().unwrap().remove(&row.key);
}

/// A depth row selects its depth range of the photo's depth map (here depth grows
/// from 0 at the left to 1 at the right), combines like any row, and masks nothing (with
/// an issue) until the map is computed.
#[test]
fn depth_rows_select_their_range() {
    use crate::model::mask::DepthRange;
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (200u32, 120u32);
    let img = image(w, h);
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let mut cache = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let flat = |e: MaskExpr| {
        edit_with(
            Blend {
                mask: e,
                ..Blend::default()
            },
            shapes(),
        )
    };
    let near = flat(MaskExpr::Depth(DepthRange::near("depth-test")));
    let key = depth::key(&depth::photo(&near.source), "depth-test");
    ai::store().lock().unwrap().remove(&key);
    let c = compile(
        &near,
        &mut MaskEnv {
            geometry,
            image: (w, h),
            scale: 1.0,
            cache: &mut cache,
            sensor_bin: 1.0,
            overlay: None,
        },
    )
    .expect("compiled");
    assert!(c.issues.iter().any(|(_, i)| *i == Issue::Depth));
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    let (plain, _) = run.render(&Edit::empty(Default::default()), &src, geometry, None);
    let (none, _) = run.render(&near, &src, geometry, None);
    assert_eq!(none, plain, "no map: no change");
    let (rw, rh) = (100u32, 60u32);
    let tex: Vec<u16> = (0..rw * rh)
        .map(|i| ((i % rw) as f32 / (rw - 1) as f32 * 65535.0) as u16)
        .collect();
    ai::store()
        .lock()
        .unwrap()
        .put(key, ai::raster(rw, rh, &tex).unwrap());
    let at = |v: &[f32], x: u32| v[((60 * w + x) * 4) as usize];
    let (gpu, _) = run.render(&near, &src, geometry, None);
    let gain = |x: u32| at(&gpu, x) / at(&plain, x).max(1e-6);
    assert!(gain(20) > 2.5, "near: {}", gain(20));
    assert!((gain(180) - 1.0).abs() < 1e-3, "far: {}", gain(180));
    let far = flat(MaskExpr::Depth(DepthRange::far("depth-test")));
    let (gpu, _) = run.render(&far, &src, geometry, None);
    let gain = |x: u32| at(&gpu, x) / at(&plain, x).max(1e-6);
    assert!((gain(20) - 1.0).abs() < 1e-3, "near: {}", gain(20));
    assert!(gain(180) > 2.5, "far: {}", gain(180));
    // The weight the shader gives a depth equals the model's own.
    let band = DepthRange::band("depth-test");
    assert_eq!(band.weight(0.5), 1.0);
    assert_eq!(band.weight(0.1), 0.0);
    assert!(band.weight(0.25) > 0.0 && band.weight(0.25) < 1.0);
    ai::store().lock().unwrap().remove(&key);
}
