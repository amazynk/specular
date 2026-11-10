//! The fused warp: GPU = CPU reference, DNG warps = the CPU opcode path, straight lines on
//! synthetic distorted grids, orientation, the inverse map, crop helpers, lens params.

use std::sync::Arc;

use specular_raw::sensor::{LensCorrection, LensMaker, OpcodeInfo, PointKind};
use specular_raw::{OutputSize, Rect};

use super::crop::{self, Grab, Guide};
use super::lens::{MakerModel, Manual, Radial};
use super::warp::{Warper, cpu_ref};
use super::*;
use crate::graph::Precision;
use crate::model::params::CropOrientation;
use crate::model::{Edit, ModuleInstance, Registry};
use crate::testgpu::{dev, lock};

pub(super) fn edit(crop: CropOrientation, lens: Option<LensParams>) -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), Default::default(), true);
    let m = e
        .module_mut(ModuleKind::CROP_ORIENTATION, 0)
        .expect("crop module");
    m.params = Params::CropOrientation(crop);
    if let Some(l) = lens {
        let mut mi = ModuleInstance::new(Registry::builtin(), ModuleKind::LENS, 0);
        mi.params = Params::Other(ModuleKind::LENS, l.to_fields());
        e.modules.push(mi);
        e.sort(Registry::builtin());
    }
    e
}

pub(super) fn no_lens() -> LensParams {
    LensParams {
        embedded: false,
        auto_scale: false,
        ..LensParams::default()
    }
}

/// A smooth colourful test image.
pub(super) fn image(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            v.extend_from_slice(&[
                0.2 + 0.7 * (fx * 7.0).sin().abs(),
                0.1 + 0.8 * fy * fy,
                0.5 + 0.4 * ((fx + fy) * 11.0).cos(),
                1.0,
            ]);
        }
    }
    v
}

/// Runs the GPU warp on `src` and reads it back.
pub(super) fn gpu_warp(
    plan: &Plan,
    src: &[f32],
    sw: u32,
    sh: u32,
    ow: u32,
    oh: u32,
    prec: Precision,
) -> Option<Vec<f32>> {
    let d = dev()?;
    let _l = lock();
    let mut w = Warper::new(&d.device, &d.queue);
    let mut engine = crate::gpu::Engine::new(
        &d.device,
        &d.queue,
        crate::gpu::pool::Budget::fixed(1 << 30),
    );
    let s = engine.upload(sw, sh, src).expect("upload");
    let out = w
        .run(plan, s.buffer(), sw, sh, Precision::F32, ow, oh, prec)
        .expect("warp");
    let o = engine
        .source_from_buffer(out, ow, oh, prec)
        .expect("source");
    let r = engine
        .render(
            &crate::graph::Graph::build(
                &crate::model::Edit::empty(Default::default()),
                &crate::graph::OpRegistry::empty(),
                None,
            )
            .expect("graph"),
            &o,
            &crate::gpu::Request {
                precision: Precision::F32,
                ..crate::gpu::Request::new(crate::graph::Roi::full(ow, oh))
            },
        )
        .expect("render");
    Some(engine.read(&r).expect("read"))
}

pub(super) fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

#[test]
fn quarter_turns_and_flips_compose() {
    let base = Orient::from_exif(6);
    let turned = total_orient(
        base,
        &CropOrientation {
            quarter_turns: 1,
            ..Default::default()
        },
    );
    // Two clockwise quarter turns: upside down.
    assert_eq!(turned, Orient::from_exif(3));
    let four = total_orient(
        Orient::default(),
        &CropOrientation {
            quarter_turns: 4,
            ..Default::default()
        },
    );
    assert_eq!(four, Orient::default());
    let flipped = total_orient(
        Orient::default(),
        &CropOrientation {
            flip_horizontal: true,
            ..Default::default()
        },
    );
    assert_eq!(flipped, Orient::from_exif(2));
    // Every composition maps the corners like the plain turn functions.
    for e in 1..=8 {
        for q in 0..4 {
            let p = CropOrientation {
                quarter_turns: q,
                ..Default::default()
            };
            let o = total_orient(Orient::from_exif(e), &p);
            let (w, h) = (5u32, 3u32);
            let mut want = Orient::from_exif(e);
            let (mut ww, mut wh) = want.size(w, h);
            // Turn what `want` shows by q quarter turns, pixel by pixel.
            let mut map: Vec<(u32, u32)> = (0..wh)
                .flat_map(|y| (0..ww).map(move |x| (x, y)))
                .map(|(x, y)| want.source_of(x, y, w, h))
                .collect();
            for _ in 0..q {
                let t = Orient::from_exif(6);
                let (nw, nh) = t.size(ww, wh);
                map = (0..nh)
                    .flat_map(|y| (0..nw).map(move |x| (x, y)))
                    .map(|(x, y)| {
                        let (sx, sy) = t.source_of(x, y, ww, wh);
                        map[(sy * ww + sx) as usize]
                    })
                    .collect();
                (ww, wh) = (nw, nh);
            }
            want = o;
            let got: Vec<(u32, u32)> = (0..wh)
                .flat_map(|y| (0..ww).map(move |x| (x, y)))
                .map(|(x, y)| want.source_of(x, y, w, h))
                .collect();
            assert_eq!(got, map, "exif {e} turns {q}");
        }
    }
}

#[test]
fn defaults_are_the_identity() {
    let e = Edit::default_for(Registry::builtin(), Default::default(), true);
    let p = Plan::new(&e, &Context::image(40, 30, Orient::default()), true);
    assert!(p.is_identity());
    assert_eq!(p.output_size(1.0), (40, 30));
    // The base orientation alone is not.
    let p = Plan::new(&e, &Context::image(40, 30, Orient::from_exif(6)), true);
    assert!(!p.is_identity());
    assert_eq!(p.output_size(1.0), (30, 40));
}

#[test]
fn lens_params_round_trip_and_clamp() {
    let l = LensParams {
        distortion: 12.5,
        vignette: -0.5,
        tca_red: 0.05,
        tca_blue: -0.03,
        ca_on: false,
        ..LensParams::default()
    };
    assert_eq!(LensParams::from_fields(&l.to_fields()), l);
    let wild = Fields::new()
        .with("distortion", Value::Float(1e9))
        .with("vignette", Value::Float(f32::NAN));
    let p = LensParams::from_fields(&wild);
    assert_eq!((p.distortion, p.vignette), (100.0, 0.0));
    // No lens module: the file's corrections on; switched off: none.
    let mut e = edit(CropOrientation::default(), None);
    assert_eq!(lens_params(&e), Some(LensParams::default()));
    e = edit(CropOrientation::default(), Some(LensParams::default()));
    e.module_mut(ModuleKind::LENS, 0).unwrap().enabled = false;
    assert_eq!(lens_params(&e), None);
}

/// Rotation, crop, orientation, manual lens with colour fringes: GPU = CPU reference.
#[test]
fn gpu_warp_equals_the_cpu_reference() {
    let (sw, sh) = (96, 64);
    let src = image(sw, sh);
    let cases = [
        (
            "rotate + crop",
            CropOrientation {
                angle_deg: 7.5,
                crop: [0.1, 0.15, 0.8, 0.9],
                ..Default::default()
            },
            no_lens(),
            Orient::default(),
        ),
        (
            "turn + flip + base",
            CropOrientation {
                quarter_turns: 1,
                flip_vertical: true,
                crop: [0.0, 0.2, 0.7, 1.0],
                ..Default::default()
            },
            no_lens(),
            Orient::from_exif(8),
        ),
        (
            "manual lens",
            CropOrientation {
                angle_deg: -3.0,
                ..Default::default()
            },
            LensParams {
                embedded: false,
                distortion: 40.0,
                vignette: 0.8,
                tca_red: 0.3,
                tca_blue: -0.2,
                ..LensParams::default()
            },
            Orient::default(),
        ),
    ];
    for (name, c, l, base) in cases {
        let e = edit(c, Some(l));
        let mut ctx = Context::image(sw, sh, base);
        ctx.camera = Some([[0.8, 0.15, 0.05], [0.1, 0.85, 0.05], [0.02, 0.1, 0.88]]);
        let plan = Plan::new(&e, &ctx, true);
        let (ow, oh) = plan.output_size(1.3);
        let cpu = cpu_ref(&plan, &src, sw, sh, ow, oh);
        for prec in [Precision::F32, Precision::F16] {
            let Some(gpu) = gpu_warp(&plan, &src, sw, sh, ow, oh, prec) else {
                return;
            };
            let d = max_diff(&gpu, &cpu);
            let tol = if prec == Precision::F32 { 2e-4 } else { 2e-3 };
            eprintln!("{name} {prec:?}: {ow}x{oh}, max diff {d:.2e}");
            assert!(d <= tol, "{name} {prec:?}: {d}");
        }
    }
}

/// A synthetic `WarpRectilinear` (three planes, tangential terms) plus a vignette after it,
/// in a crop inside a larger sensor with its own bounds.
fn dng_ops(crop: Rect) -> Vec<OpcodeInfo> {
    let bounds = Rect {
        x: crop.x.saturating_sub(4),
        y: crop.y + 2,
        w: crop.w + 6,
        h: crop.h - 3,
    };
    let full = Rect {
        x: 0,
        y: 0,
        w: crop.x + crop.w + 20,
        h: crop.y + crop.h + 20,
    };
    vec![
        OpcodeInfo::Warp {
            fisheye: false,
            k: [
                [0.985, 0.021, -0.008, 0.0015, 0.0004, -0.0003],
                [0.99, 0.018, -0.006, 0.001, 0.0004, -0.0003],
                [0.995, 0.015, -0.004, 0.0008, 0.0004, -0.0003],
            ],
            centre: (0.48, 0.52),
            bounds,
        },
        OpcodeInfo::Point {
            rect: full,
            plane: 0,
            planes: 3,
            row_pitch: 1,
            col_pitch: 1,
            bounds,
            kind: PointKind::Vignette {
                k: [0.3, -0.1, 0.05, 0.0, 0.0],
                centre: (
                    crop.x as f64 + crop.w as f64 * 0.5,
                    crop.y as f64 + crop.h as f64 * 0.5,
                ),
                inv_m2: 1.0
                    / (crop.w as f64 * crop.w as f64 / 4.0 + crop.h as f64 * crop.h as f64 / 4.0),
            },
        },
    ]
}

/// The acceptance check: the GPU warp reads every plane from where the CPU opcode develop
/// (`specular-raw`) does, within 1e-3 px. A ramp holding its own coordinate shows where
/// each output pixel read (Catmull-Rom reproduces linear functions exactly).
#[test]
fn dng_warp_matches_the_cpu_opcode_path() {
    let crop = Rect {
        x: 12,
        y: 8,
        w: 3000,
        h: 2000,
    };
    for fisheye in [false, true] {
        let mut list3 = dng_ops(crop);
        if fisheye {
            list3.truncate(1);
            if let OpcodeInfo::Warp { fisheye: f, k, .. } = &mut list3[0] {
                *f = true;
                *k = [
                    [1.02, -0.01, 0.002, 0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [0.98, 0.01, -0.002, 0.0, 0.0, 0.0],
                ];
            }
        }
        let ctx = Context {
            frame: [
                f64::from(crop.x),
                f64::from(crop.y),
                f64::from(crop.w),
                f64::from(crop.h),
            ],
            base: Orient::default(),
            lens: None,
            list3: list3.clone(),
            camera: None,
            rgb_ca: None,
            stretch: [1.0, 1.0],
            lensfun: None,
            focal35: None,
        };
        let e = edit(
            CropOrientation::default(),
            Some(LensParams {
                auto_scale: false,
                ..LensParams::default()
            }),
        );
        let plan = Plan::new(&e, &ctx, true);
        assert_eq!(plan.dng.len(), if fisheye { 1 } else { 2 });
        assert!(plan.per_plane());
        let (w, h) = (crop.w, crop.h);
        let mut worst = 0f64;
        let mut n = 0usize;
        for axis in 0..2 {
            let src: Vec<f32> = (0..h)
                // Centred, so f32 keeps the ramp exact to well below 1e-3.
                .flat_map(|y| {
                    (0..w)
                        .map(move |x| [x as f32 - w as f32 / 2.0, y as f32 - h as f32 / 2.0][axis])
                })
                .flat_map(|v| [v, v, v, 1.0])
                .collect();
            // The vignette would scale the ramp: leave it out of this check.
            let mut p = plan.clone();
            p.dng.retain(|s| matches!(s, DngStep::Warp { .. }));
            let Some(out) = gpu_warp(&p, &src, w, h, w, h, Precision::F32) else {
                return;
            };
            for y in (40..h - 40).step_by(7) {
                for x in (40..w - 40).step_by(5) {
                    for plane in 0..3 {
                        let (cx, cy) = list3[0]
                            .warp_source(crop, OutputSize::Full, x as usize, y as usize, plane)
                            .expect("a warp");
                        let want =
                            f64::from([cx, cy][axis]) - [f64::from(w), f64::from(h)][axis] / 2.0;
                        let got = f64::from(out[((y * w + x) * 4) as usize + plane]);
                        worst = worst.max((got - want).abs());
                        n += 1;
                    }
                }
            }
        }
        eprintln!(
            "DNG {}: {n} coordinates, worst {worst:.2e} px",
            if fisheye {
                "WarpFisheye"
            } else {
                "WarpRectilinear"
            }
        );
        assert!(worst <= 1e-3, "worst {worst} px");
    }
}

/// The vignette after the warp is evaluated at the warped position, as the DNG list runs.
#[test]
fn dng_vignette_after_a_warp_uses_warped_positions() {
    let crop = Rect {
        x: 12,
        y: 8,
        w: 300,
        h: 200,
    };
    let list3 = dng_ops(crop);
    let ctx = Context {
        frame: [12.0, 8.0, 300.0, 200.0],
        base: Orient::default(),
        lens: None,
        list3: list3.clone(),
        camera: None,
        rgb_ca: None,
        stretch: [1.0, 1.0],
        lensfun: None,
        focal35: None,
    };
    let plan = Plan::new(&edit(CropOrientation::default(), None), &ctx, true);
    let DngStep::Vignette { k, centre, inv_m2 } = plan.dng[1].clone() else {
        panic!("vignette second");
    };
    let (x, y) = (250.5 + 12.0, 30.5 + 8.0);
    // The vignette runs after the warp in the list, so it is evaluated at the output point.
    let r2 = ((x - centre.0).powi(2) + (y - centre.1).powi(2)) * inv_m2;
    let want: f64 = 1.0 + (0..5).map(|i| k[i] * r2.powi(i as i32 + 1)).sum::<f64>();
    let (_, gain) = plan.corrected_to_source(1, (x - 12.0 - 150.0, y - 8.0 - 100.0));
    assert!((gain - want).abs() < 1e-12, "{gain} vs {want}");
    // The front end leaves both to the warp.
    assert!(warp_takes(&list3, 0) && warp_takes(&list3, 1));
}

fn fuji_like() -> LensCorrection {
    let knots: Vec<f64> = (1..=9).map(|k| (f64::from(k) / 8.0).sqrt()).collect();
    let last = knots[8];
    LensCorrection {
        maker: LensMaker::Fujifilm,
        knots: knots.iter().map(|k| k / last).collect(),
        distortion: Some(vec![
            -1.17, -2.35, -3.46, -4.43, -5.27, -5.92, -6.33, -6.43, -6.43,
        ]),
        vignetting: Some(vec![98.5, 96.9, 94.9, 91.0, 84.7, 78.4, 73.0, 67.5, 67.5]),
        ca_red: Some(vec![
            -3e-5, -9e-5, -1.5e-4, -2.1e-4, -2.7e-4, -3.4e-4, -4.6e-4, -6.1e-4, -6.1e-4,
        ]),
        ca_blue: Some(vec![
            7e-4, 7.9e-4, 8.9e-4, 9.5e-4, 1.04e-3, 1.13e-3, 1.22e-3, 1.43e-3, 1.43e-3,
        ]),
        half_diagonal_px: Some(3749.0),
    }
}

fn sony_like() -> LensCorrection {
    LensCorrection {
        maker: LensMaker::Sony,
        knots: (0..11).map(|i| f64::from(i) / 10.0).collect(),
        distortion: Some(vec![
            29.0, 29.0, 58.0, 144.0, 259.0, 417.0, 633.0, 891.0, 1222.0, 1639.0, 2156.0,
        ]),
        vignetting: Some(vec![0.0; 11]),
        ca_red: Some(vec![-1280.0; 11]),
        ca_blue: Some(vec![-640.0; 11]),
        half_diagonal_px: None,
    }
}

#[test]
fn maker_models_follow_their_published_forms() {
    // Sony: g(ρ) / (largest edge g); the corner reads the corner.
    let s = MakerModel::new(&sony_like(), 6000.0, 4000.0);
    let corner = s.factor(1, 1.0, true, false);
    assert!((corner - 1.0).abs() < 1e-9, "{corner}");
    let centre = s.factor(1, 0.0, true, false);
    assert!((centre - (1.0 + 29.0 / 16384.0) / (1.0 + 2156.0 / 16384.0)).abs() < 1e-12);
    // Red: the green factor times 1 + c / 2^21.
    let red = s.factor(0, 0.5, true, true) / s.factor(1, 0.5, true, true);
    assert!((red - (1.0 - 1280.0 / 2_097_152.0)).abs() < 1e-12);
    assert!(!s.has_vignetting());
    // Fujifilm: ρs / (1 + D(ρs)/100) = ρu, so barrel correction reads nearer the centre.
    let f = MakerModel::new(&fuji_like(), 6240.0, 4160.0);
    for rho in [0.2, 0.5, 0.9, 1.0] {
        let k = f.factor(1, rho, true, false);
        let rs = rho * k;
        assert!(k < 1.0, "{rho}: {k}");
        // Forward again: the source radius divided by 1 + D(ρs)/100 is ρu.
        let back = rs / (1.0 + f.factor_table_distortion(rs) / 100.0);
        assert!((back - rho).abs() < 1e-6, "{rho}: {back}");
    }
    // Vignetting: 100 / V at the knot.
    assert!((f.vignette_gain(1.0) - 100.0 / 67.5).abs() < 1e-9);
    assert!((f.vignette_gain(0.0) - 1.0).abs() < 1e-12);
}

#[test]
fn fill_zoom_leaves_no_empty_edges() {
    let (w, h) = (600.0, 400.0);
    for lens in [fuji_like(), sony_like()] {
        let r = Radial {
            maker: Some(MakerModel::new(&lens, w, h)),
            use_distortion: true,
            use_ca: true,
            use_vignetting: true,
            manual: Manual::default(),
            zoom: 1.0,
        };
        let z = r.fill_zoom(w, h);
        let r = Radial { zoom: z, ..r };
        let rr = (w * w + h * h).sqrt() / 2.0;
        let mut touch = 0f64;
        for i in 0..=100 {
            let t = f64::from(i) / 100.0 - 0.5;
            for (x, y) in [(t * w, h / 2.0), (w / 2.0, t * h)] {
                let rho = (x * x + y * y).sqrt() / rr;
                for p in 0..3 {
                    let k = r.factor(p, rho);
                    assert!((x * k).abs() <= w / 2.0 + 1e-6 && (y * k).abs() <= h / 2.0 + 1e-6);
                    touch = touch
                        .max((x * k).abs() / (w / 2.0))
                        .max((y * k).abs() / (h / 2.0));
                }
            }
        }
        // ... and wastes nothing: some edge point reads the source edge.
        assert!(touch > 0.999, "{:?}: zoom {z}, reach {touch}", lens.maker);
        eprintln!("{:?}: fill zoom {z:.5}", lens.maker);
    }
}

/// Renders ideal vertical lines (every `step` px, Gaussian profile) as a camera with
/// `plan`'s lens would have recorded it, warps it, and measures how straight the lines are.
fn straightness(plan: &Plan, w: u32, h: u32, scale: f64) -> (f64, f64) {
    let step = 40.0;
    let line = |u: f64| {
        let d = (u / step - (u / step).round()).abs() * step;
        if d < 6.0 { (-d * d / 4.5).exp() } else { 0.0 }
    };
    // Source pixel → where it belongs in the output, then the ideal grid there.
    let mut src = vec![0f32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let s = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let v = plan.to_output(s, scale).map_or(0.0, |o| line(o.0));
            let i = ((y * w + x) * 4) as usize;
            src[i..i + 4].copy_from_slice(&[v as f32, v as f32, v as f32, 1.0]);
        }
    }
    let (ow, oh) = plan.output_size(scale);
    let measure = |img: &[f32], iw: u32, ih: u32| {
        // Each vertical line's centroid per row, then the spread around a fitted line.
        let mut worst = 0f64;
        let mut k = step;
        while k < f64::from(iw) - step / 2.0 {
            let (x0, x1) = ((k - 8.0).max(0.0) as u32, ((k + 8.0) as u32).min(iw - 1));
            let pts: Vec<(f64, f64)> = (20..ih - 20)
                .filter_map(|y| {
                    let (mut s, mut sx) = (0.0, 0.0);
                    for x in x0..=x1 {
                        let v = f64::from(img[((y * iw + x) * 4) as usize]);

                        s += v;
                        sx += v * (f64::from(x) + 0.5);
                    }
                    (s > 0.5).then(|| (f64::from(y), sx / s))
                })
                .collect();
            if pts.len() > 10 {
                let n = pts.len() as f64;
                let (my, mx) = (
                    pts.iter().map(|p| p.0).sum::<f64>() / n,
                    pts.iter().map(|p| p.1).sum::<f64>() / n,
                );
                let b = pts.iter().map(|p| (p.0 - my) * (p.1 - mx)).sum::<f64>()
                    / pts.iter().map(|p| (p.0 - my).powi(2)).sum::<f64>();
                for p in &pts {
                    worst = worst.max((p.1 - (mx + b * (p.0 - my))).abs());
                }
            }
            k += step;
        }
        worst
    };
    let before = measure(&src, w, h);
    let out = cpu_ref(plan, &src, w, h, ow, oh);
    (before, measure(&out, ow, oh))
}

#[test]
fn straight_lines_come_out_straight() {
    let (w, h) = (360u32, 240u32);
    let frame = [0.0, 0.0, f64::from(w), f64::from(h)];
    let cases: Vec<(&str, Context, LensParams)> = vec![
        (
            "manual barrel",
            Context::image(w, h, Orient::default()),
            LensParams {
                embedded: false,
                distortion: 60.0,
                ..LensParams::default()
            },
        ),
        (
            "Fujifilm table",
            Context {
                frame,
                base: Orient::default(),
                lens: Some(Arc::new(fuji_like())),
                list3: Vec::new(),
                camera: None,
                rgb_ca: None,
                stretch: [1.0, 1.0],
                lensfun: None,
                focal35: None,
            },
            LensParams::default(),
        ),
        (
            "Sony table",
            Context {
                frame,
                base: Orient::default(),
                lens: Some(Arc::new(sony_like())),
                list3: Vec::new(),
                camera: None,
                rgb_ca: None,
                stretch: [1.0, 1.0],
                lensfun: None,
                focal35: None,
            },
            LensParams::default(),
        ),
        (
            "DNG WarpRectilinear",
            Context {
                frame,
                base: Orient::default(),
                lens: None,
                list3: dng_ops(Rect { x: 0, y: 0, w, h })[..1].to_vec(),
                camera: None,
                rgb_ca: None,
                stretch: [1.0, 1.0],
                lensfun: None,
                focal35: None,
            },
            LensParams::default(),
        ),
    ];
    for (name, ctx, l) in cases {
        let plan = Plan::new(&edit(CropOrientation::default(), Some(l)), &ctx, true);
        let (before, after) = straightness(&plan, w, h, 1.0);
        eprintln!("{name}: line residual {before:.2} px distorted, {after:.3} px corrected");
        assert!(after < 0.15, "{name}: {after}");
        assert!(
            before > 4.0 * after.max(0.05),
            "{name}: {before} vs {after}"
        );
    }
}

#[test]
fn to_output_inverts_to_source() {
    let ctx = Context {
        frame: [6.0, 13.0, 624.0, 416.0],
        base: Orient::from_exif(6),
        lens: Some(Arc::new(fuji_like())),
        list3: Vec::new(),
        camera: None,
        rgb_ca: None,
        stretch: [1.0, 1.0],
        lensfun: None,
        focal35: None,
    };
    let e = edit(
        CropOrientation {
            quarter_turns: 1,
            flip_horizontal: true,
            angle_deg: 4.0,
            crop: [0.1, 0.1, 0.9, 0.85],
            ..Default::default()
        },
        Some(LensParams {
            distortion: -20.0,
            ..LensParams::default()
        }),
    );
    let plan = Plan::new(&e, &ctx, true);
    let scale = 0.7;
    let (ow, oh) = plan.output_size(scale);
    let mut worst = 0f64;
    for y in (0..oh).step_by(17) {
        for x in (0..ow).step_by(13) {
            let o = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let (s, _) = plan.to_source(1, o, scale);
            let back = plan.to_output(s, scale).expect("settles");
            worst = worst.max((back.0 - o.0).abs()).max((back.1 - o.1).abs());
        }
    }
    assert!(worst < 1e-4, "{worst}");
}

#[test]
fn rotation_is_clockwise_on_screen() {
    // A point right of the centre moves down when the image turns clockwise: the output
    // point below the centre reads from the source to the right of it.
    let e = edit(
        CropOrientation {
            angle_deg: 90.0,
            ..Default::default()
        },
        Some(no_lens()),
    );
    let plan = Plan::new(&e, &Context::image(100, 100, Orient::default()), true);
    let ((x, y), _) = plan.frame_to_source(1, (50.0, 80.0));
    assert!(
        (x - 80.0).abs() < 1e-9 && (y - 50.0).abs() < 1e-9,
        "{x} {y}"
    );
}

#[test]
fn crop_helpers() {
    let (w, h) = (600.0, 400.0);
    let a = 5f64.to_radians();
    // The largest 3:2 crop inside a frame turned by 5°: corners inside, fills one way.
    let c = crop::largest(Some(1.5), w, h, a);
    assert!(crop::crop_inside(c, w, h, a));
    let grown = [c[0] - 0.005, c[1] - 0.005, c[2] + 0.005, c[3] + 0.005];
    assert!(!crop::crop_inside(grown, w, h, a));
    assert!((((c[2] - c[0]) * w) / ((c[3] - c[1]) * h) - 1.5).abs() < 1e-6);
    // Straighten by a line: a line 3° below level asks for −3°.
    let ang = crop::level_angle((0.0, 0.0), (100.0, 100.0 * 3f64.to_radians().tan()))
        .expect("long enough");
    assert!((ang + 3.0).abs() < 1e-9, "{ang}");
    let plumb = crop::level_angle((0.0, 0.0), (5.0, 100.0)).expect("long enough");
    assert!((plumb - (90.0 - (100f64).atan2(5.0).to_degrees())).abs() < 1e-9);
    assert_eq!(crop::level_angle((0.0, 0.0), (1.0, 1.0)), None);
    // Ratio keeps the area's centre.
    let r = crop::with_ratio([0.2, 0.2, 0.8, 0.8], 1.0, w, h, 0.0);
    assert!((((r[2] - r[0]) * w) - ((r[3] - r[1]) * h)).abs() < 1e-6);
    assert!(((r[0] + r[2]) / 2.0 - 0.5).abs() < 1e-9);
    // Grabbing and dragging a corner with a fixed 1:1 ratio.
    assert_eq!(crop::grab(0.0, 0.0, (0.05, 0.05)), Some(Grab::Handle(3)));
    assert_eq!(crop::grab(0.5, 0.5, (0.05, 0.05)), Some(Grab::Move));
    assert_eq!(crop::grab(1.5, 0.5, (0.05, 0.05)), None);
    let d = crop::drag(r, Grab::Handle(12), 0.1, 0.05, Some(1.0), w, h, 0.0);
    assert!(
        (((d[2] - d[0]) * w) - ((d[3] - d[1]) * h)).abs() < 1e-6,
        "{d:?}"
    );
    assert_eq!((d[0], d[1]), (r[0], r[1]));
    // Moving stays inside the frame.
    let m = crop::drag(r, Grab::Move, 0.9, 0.0, None, w, h, 0.0);
    assert!((m[2] - 1.0).abs() < 1e-9);
    // Guides: thirds at 1/3 and 2/3; diagonals at 45° in pixels.
    assert_eq!(Guide::Thirds.lines(1.5).len(), 4);
    for l in Guide::Diagonal.lines(1.5) {
        let (dx, dy) = ((l[1].0 - l[0].0) * 1.5, l[1].1 - l[0].1);
        assert!((dx.abs() - dy.abs()).abs() < 1e-9);
    }
    assert_eq!(Guide::Grid.next(), Guide::Thirds);
    assert!(crop::ASPECTS.contains(&"16:9"));
    assert_eq!(aspect_ratio("16:9", 1.0, 1.0), Some(16.0 / 9.0));
    assert_eq!(aspect_ratio("free", 1.0, 1.0), None);
    assert_eq!(aspect_ratio("original", 6.0, 4.0), Some(1.5));
    assert_eq!(aspect_ratio("0:3", 1.0, 1.0), None);
}

#[test]
fn crop_off_shows_the_whole_straightened_frame() {
    let e = edit(
        CropOrientation {
            angle_deg: 10.0,
            crop: [0.2, 0.2, 0.6, 0.6],
            ..Default::default()
        },
        Some(no_lens()),
    );
    let ctx = Context::image(300, 200, Orient::default());
    assert_eq!(Plan::new(&e, &ctx, true).output_size(1.0), (120, 80));
    assert_eq!(Plan::new(&e, &ctx, false).output_size(1.0), (300, 200));
}

#[test]
fn warper_upscales() {
    let Some(d) = dev() else { return };
    let (sw, sh) = (64, 48);
    let src = image(sw, sh);
    let e = edit(
        CropOrientation {
            angle_deg: 2.0,
            ..Default::default()
        },
        Some(no_lens()),
    );
    let plan = Plan::new(&e, &Context::image(sw, sh, Orient::default()), true);
    let (ow, oh) = (640, 480);
    let cpu = cpu_ref(&plan, &src, sw, sh, ow, oh);
    let gpu = gpu_warp(&plan, &src, sw, sh, ow, oh, Precision::F32).expect("gpu");
    assert!(max_diff(&gpu, &cpu) < 2e-4);
    let _ = d;
}

/// Sony vignetting, the proposed reading: knot gains `2^(1 − 2^(v/8192))`, linear
/// in ρ between knots, 1 where the table is 0.
#[test]
fn sony_vignetting_table_gains() {
    use super::lens::sony_vignette_gain;
    assert_eq!(sony_vignette_gain(0.0), 1.0);
    // v = 8192 halves (1 − 2 = −1); v = −8192 gives 2^(1/2).
    assert!((sony_vignette_gain(8192.0) - 0.5).abs() < 1e-12);
    assert!((sony_vignette_gain(-8192.0) - 2f64.sqrt()).abs() < 1e-12);
    let mut l = sony_like();
    let v: Vec<f64> = (0..11).map(|i| -150.0 * f64::from(i)).collect();
    l.vignetting = Some(v.clone());
    let m = MakerModel::new(&l, 6000.0, 4000.0);
    // Evaluated, not applied (see the lens module's documentation).
    assert!(!m.has_vignetting());
    for (i, &vi) in v.iter().enumerate() {
        let rho = i as f64 / 10.0;
        assert!(
            (m.vignette_gain(rho) - sony_vignette_gain(vi)).abs() < 1e-12,
            "knot {i}"
        );
    }
    // Halfway between knots 7 and 8: the mean of their gains.
    let mid = 0.5 * (sony_vignette_gain(v[7]) + sony_vignette_gain(v[8]));
    assert!((m.vignette_gain(0.75) - mid).abs() < 1e-12);
    assert!(
        m.vignette_gain(1.0) > 1.0,
        "negative values brighten the corner"
    );
    // An all-zero table is the identity.
    let z = MakerModel::new(&sony_like(), 6000.0, 4000.0);
    assert!((0..=10).all(|i| z.vignette_gain(f64::from(i) / 10.0) == 1.0));
}

/// A Lensfun lookup with one lens per profile given; `pick` as the lookup decided.
fn lensfun_shot(
    pick: specular_lensfun::lookup::Pick,
    profiles: &[specular_lensfun::model::Profile],
) -> Arc<lensfun::Shot> {
    Arc::new(lensfun::Shot {
        lens_name: Some("Test 24mm".into()),
        candidates: profiles
            .iter()
            .enumerate()
            .map(|(i, p)| lensfun::Candidate {
                key: format!("lens{i}"),
                name: format!("Lens {i}"),
                profile: Arc::new(p.clone()),
            })
            .collect(),
        pick,
        tied: if pick == specular_lensfun::lookup::Pick::Ambiguous {
            profiles.len()
        } else {
            1
        },
    })
}

/// A full-frame 3:2 lens on a full-frame body: strong barrel distortion, red/blue TCA, a
/// 1-stop corner falloff.
fn lensfun_profile() -> specular_lensfun::model::Profile {
    use specular_lensfun::model::{Distortion, Profile, Tca};
    Profile::new(
        Some(Distortion::PtLens {
            a: 0.0,
            b: -0.06,
            c: 0.0,
        }),
        Some(Tca {
            red: [0.0004, 0.0, 1.0002],
            blue: [-0.0003, 0.0, 0.9998],
        }),
        Some([-0.5, 0.0, 0.0]),
        1.0,
        1.5,
        1.0,
    )
}

fn lensfun_ctx(w: u32, h: u32, shot: Option<Arc<lensfun::Shot>>) -> Context {
    Context {
        lensfun: shot,
        ..Context::image(w, h, Orient::default())
    }
}

/// Phase 3 item 11: the back-map of a Lensfun profile is the published formula.
#[test]
fn lensfun_backmap_is_the_formula() {
    use specular_lensfun::lookup::Pick;
    let (w, h) = (3000u32, 2000u32);
    let ctx = lensfun_ctx(w, h, Some(lensfun_shot(Pick::One, &[lensfun_profile()])));
    let lp = LensParams {
        auto_scale: false,
        ..LensParams::default()
    };
    let plan = Plan::new(&edit(CropOrientation::default(), Some(lp)), &ctx, true);
    // A point on the horizontal axis, 1200 px right of the centre: ρ = 1200 / 1802.78,
    // r_u = ρ·√3.25 (half the short side of a 3:2 frame is 1), r_d = r_u (1 − 0.06 r_u² + 0.06).
    let half_diag = (3000f64 * 3000.0 + 2000.0 * 2000.0).sqrt() / 2.0;
    let rho = 1200.0 / half_diag;
    let ru = rho * 3.25f64.sqrt();
    let k = -0.06 * ru * ru + 1.06;
    let ((sx, sy), gain) = plan.to_source(1, (1500.0 + 1200.0, 1000.0), 1.0);
    assert!((sx - (1500.0 + 1200.0 * k)).abs() < 1e-6, "{sx}");
    assert!((sy - 1000.0).abs() < 1e-9);
    // Vignetting at the source radius (the calibration's corner is this body's corner).
    let rs = 1200.0 * k / half_diag;
    assert!((gain - 1.0 / (1.0 - 0.5 * rs * rs)).abs() < 1e-9, "{gain}");
    // Red after distortion, on the distorted radius.
    let rd = ru * k;
    let kr = k * (0.0004 * rd * rd + 1.0002);
    let ((rx, _), _) = plan.to_source(0, (1500.0 + 1200.0, 1000.0), 1.0);
    assert!((rx - (1500.0 + 1200.0 * kr)).abs() < 1e-6, "{rx}");
}

/// The profile straightens a synthetic grid it distorted; the GPU warp equals the CPU one.
#[test]
fn lensfun_profile_straightens_a_grid() {
    use specular_lensfun::lookup::Pick;
    let (w, h) = (360u32, 240u32);
    let ctx = lensfun_ctx(w, h, Some(lensfun_shot(Pick::One, &[lensfun_profile()])));
    let plan = Plan::new(
        &edit(CropOrientation::default(), Some(LensParams::default())),
        &ctx,
        true,
    );
    assert!(plan.radial.maker.is_some());
    let (before, after) = straightness(&plan, w, h, 1.0);
    eprintln!("Lensfun ptlens: line residual {before:.2} px distorted, {after:.3} px corrected");
    assert!(after < 0.15, "{after}");
    assert!(before > 4.0 * after.max(0.05), "{before} vs {after}");
    let (sw, sh) = (96, 64);
    let src = image(sw, sh);
    let ctx = lensfun_ctx(sw, sh, Some(lensfun_shot(Pick::One, &[lensfun_profile()])));
    let plan = Plan::new(
        &edit(CropOrientation::default(), Some(LensParams::default())),
        &ctx,
        true,
    );
    let (ow, oh) = plan.output_size(1.0);
    let cpu = cpu_ref(&plan, &src, sw, sh, ow, oh);
    if let Some(gpu) = gpu_warp(&plan, &src, sw, sh, ow, oh, Precision::F32) {
        let d = max_diff(&gpu, &cpu);
        eprintln!("Lensfun GPU vs CPU: max diff {d:.2e}");
        assert!(d <= 2e-4, "{d}");
    }
}

/// The lens module's `lensfun` field picks; ambiguity and data of the file's own win.
#[test]
fn lensfun_choice_follows_the_field() {
    use specular_lensfun::lookup::Pick;
    let (w, h) = (600u32, 400u32);
    let p0 = lensfun_profile();
    let mut p1 = lensfun_profile();
    p1.distortion = None;
    let applied = |shot: Arc<lensfun::Shot>, setting: &str| -> Option<bool> {
        let lp = LensParams {
            lensfun: setting.into(),
            ..LensParams::default()
        };
        let plan = Plan::new(
            &edit(CropOrientation::default(), Some(lp)),
            &lensfun_ctx(w, h, Some(shot)),
            true,
        );
        plan.radial.maker.as_ref().map(|m| m.has_distortion())
    };
    let one = lensfun_shot(Pick::One, &[p0.clone(), p1.clone()]);
    assert_eq!(applied(one.clone(), ""), Some(true), "automatic: the match");
    assert_eq!(
        applied(one.clone(), "lens1"),
        Some(false),
        "the second lens"
    );
    assert_eq!(applied(one.clone(), lensfun::NONE), None, "none");
    assert_eq!(applied(one, "gone"), None, "a lens no longer offered");
    // Two equally good matches: nothing until one is picked.
    let amb = lensfun_shot(Pick::Ambiguous, &[p0.clone(), p1]);
    assert_eq!(applied(amb.clone(), ""), None);
    assert_eq!(applied(amb, "lens0"), Some(true));
    let none = lensfun_shot(Pick::NoMatch, std::slice::from_ref(&p0));
    assert_eq!(applied(none.clone(), ""), None);
    assert_eq!(applied(none, "lens0"), Some(true));
    // The field round-trips and is left out while automatic.
    let lp = LensParams {
        lensfun: "lens1".into(),
        ..LensParams::default()
    };
    assert_eq!(LensParams::from_fields(&lp.to_fields()), lp);
    assert_eq!(LensParams::default().to_fields().get("lensfun"), None);
    // The file's own data wins: maker tables, or DNG warps.
    let one = lensfun_shot(Pick::One, &[p0]);
    let mut ctx = lensfun_ctx(w, h, Some(one.clone()));
    ctx.lens = Some(Arc::new(sony_like()));
    let plan = Plan::new(
        &edit(CropOrientation::default(), Some(LensParams::default())),
        &ctx,
        true,
    );
    assert!(
        plan.radial
            .maker
            .as_ref()
            .is_some_and(|m| m.maker().is_some())
    );
    let mut ctx = lensfun_ctx(w, h, Some(one));
    ctx.list3 = dng_ops(Rect { x: 0, y: 0, w, h })[..1].to_vec();
    let plan = Plan::new(
        &edit(CropOrientation::default(), Some(LensParams::default())),
        &ctx,
        true,
    );
    assert!(plan.radial.maker.is_none());
}
