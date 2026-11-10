//! Perspective: the kernel against the CPU reference, the inverse map, identity, upright on
//! synthetic scenes, guided lines, LSD, the crop region, the mask grid and the sidecar.

use super::crop::{self, Frame};
use super::persp::{self, Mode, PerspParams, apply, inverse, keystone};
use super::tests::{edit, gpu_warp, image, max_diff, no_lens};
use super::upright::{self, Goal};
use super::warp::{cpu_ref, params};
use super::*;
use crate::graph::Precision;
use crate::model::params::CropOrientation;
use crate::model::{Edit, ModuleInstance, Registry};

fn with_persp(mut e: Edit, p: &PerspParams) -> Edit {
    let mut m = ModuleInstance::new(Registry::builtin(), ModuleKind::PERSPECTIVE, 0);
    m.params = Params::Other(ModuleKind::PERSPECTIVE, p.to_fields());
    e.modules.push(m);
    e.sort(Registry::builtin());
    e
}

fn keystoned() -> PerspParams {
    PerspParams {
        vertical: 40.0,
        horizontal: -25.0,
        rotate: 2.0,
        aspect: 15.0,
        offset_x: 6.0,
        ..PerspParams::default()
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn gpu_warp_equals_the_cpu_reference_with_perspective() {
    let (sw, sh) = (96, 64);
    let src = image(sw, sh);
    let cases = [
        ("keystone", CropOrientation::default(), no_lens()),
        (
            "keystone + straighten + crop + lens",
            CropOrientation {
                angle_deg: -4.0,
                crop: [0.05, 0.1, 0.9, 0.95],
                quarter_turns: 1,
                ..Default::default()
            },
            LensParams {
                embedded: false,
                distortion: 35.0,
                tca_red: 0.2,
                ..LensParams::default()
            },
        ),
    ];
    for (name, c, l) in cases {
        let e = with_persp(edit(c, Some(l)), &keystoned());
        let mut ctx = Context::image(sw, sh, Orient::from_exif(6));
        ctx.camera = Some([[0.8, 0.15, 0.05], [0.1, 0.85, 0.05], [0.02, 0.1, 0.88]]);
        let plan = Plan::new(&e, &ctx, true);
        assert!(plan.persp.is_some(), "{name}");
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

#[test]
fn to_output_inverts_to_source_with_keystone_lens_and_stretch() {
    let mut ctx = Context::image(600, 400, Orient::from_exif(8));
    ctx.stretch = [1.5, 1.0];
    let e = with_persp(
        edit(
            CropOrientation {
                angle_deg: 3.0,
                crop: [0.1, 0.1, 0.9, 0.85],
                ..Default::default()
            },
            Some(LensParams {
                embedded: false,
                distortion: -25.0,
                ..LensParams::default()
            }),
        ),
        &keystoned(),
    );
    let plan = Plan::new(&e, &ctx, true);
    assert!(plan.persp.is_some());
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

/// Off, missing, neutral or switched off: the plan, the kernel's words and the pixels are
/// what they were without the module.
#[test]
fn identity_params_change_nothing() {
    let (sw, sh) = (80u32, 60u32);
    let src = image(sw, sh);
    let c = CropOrientation {
        angle_deg: 5.0,
        crop: [0.1, 0.05, 0.95, 0.9],
        ..Default::default()
    };
    let base = edit(c, Some(no_lens()));
    let ctx = Context::image(sw, sh, Orient::default());
    let plain = Plan::new(&base, &ctx, true);
    let (ow, oh) = plain.output_size(1.0);
    let want = cpu_ref(&plain, &src, sw, sh, ow, oh);
    let words = params(&plain, sw, sh, Precision::F32, ow, oh, Precision::F32);
    let mut off = with_persp(base.clone(), &keystoned());
    off.module_mut(ModuleKind::PERSPECTIVE, 0).unwrap().enabled = false;
    let guides_only = PerspParams {
        guides: vec![[0.1, 0.1, 0.1, 0.9]],
        ..PerspParams::default()
    };
    for (name, e) in [
        ("default", with_persp(base.clone(), &PerspParams::default())),
        ("off", off),
        ("guides, mode off", with_persp(base.clone(), &guides_only)),
    ] {
        let p = Plan::new(&e, &ctx, true);
        assert!(p.persp.is_none(), "{name}");
        assert_eq!(format!("{p:?}"), format!("{plain:?}"), "{name}");
        assert_eq!(
            params(&p, sw, sh, Precision::F32, ow, oh, Precision::F32),
            words,
            "{name}"
        );
        let got = cpu_ref(&p, &src, sw, sh, ow, oh);
        assert!(
            got.iter()
                .zip(&want)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "{name}"
        );
    }
}

/// The synthetic scene: a facade (lines every 48 px), photographed by a camera turned by
/// `truth` (pitch, yaw, roll) through a barrel lens.
struct Scene {
    w: u32,
    h: u32,
    f: f64,
    /// Oriented frame (centred) → facade.
    to_facade: persp::M3,
    /// The lens alone.
    lens: LensParams,
    src: Vec<f32>,
    base: Plan,
}

const SPACING: f64 = 48.0;

fn facade(q: (f64, f64)) -> f64 {
    let d = |v: f64| {
        let t = (v / SPACING - (v / SPACING).round()).abs() * SPACING;
        (-t * t / 2.0).exp()
    };
    0.8 - 0.6 * d(q.0).max(d(q.1))
}

fn scene(truth: [f64; 3]) -> Scene {
    let (w, h) = (480u32, 320u32);
    let lens = LensParams {
        embedded: false,
        auto_scale: false,
        distortion: 30.0,
        ..LensParams::default()
    };
    let ctx = Context::image(w, h, Orient::default());
    let base = Plan::new(
        &edit(CropOrientation::default(), Some(lens.clone())),
        &ctx,
        false,
    );
    let f = persp::focal_px(&PerspParams::default(), None, f64::from(w), f64::from(h));
    let [p, y, r] = truth.map(f64::to_radians);
    let to_facade = keystone(f, p, y, r);
    let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
    let mut src = vec![0f32; (w * h * 4) as usize];
    for py in 0..h {
        for px in 0..w {
            let s = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            let v = base
                .to_output(s, 1.0)
                .and_then(|o| apply(&to_facade, (o.0 - cx, o.1 - cy)))
                .map_or(0.0, facade);
            let i = ((py * w + px) * 4) as usize;
            src[i..i + 4].copy_from_slice(&[v as f32, v as f32, v as f32, 1.0]);
        }
    }
    Scene {
        w,
        h,
        f,
        to_facade,
        lens,
        src,
        base,
    }
}

impl Scene {
    /// Detected segments on the lens-corrected picture (centred oriented pixels).
    fn lines(&self) -> Vec<upright::Line> {
        let (w, h) = (self.w, self.h);
        let pic = cpu_ref(&self.base, &self.src, w, h, w, h);
        let lum: Vec<f32> = pic.chunks_exact(4).map(|p| p[0]).collect();
        let t = std::time::Instant::now();
        let segs = lsd::detect(&lum, w as usize, h as usize, 255.0);
        eprintln!(
            "LSD: {} segments on {w}×{h} in {:.0} ms",
            segs.len(),
            t.elapsed().as_secs_f64() * 1e3
        );
        let (cx, cy) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
        segs.iter()
            .filter(|s| s.len() > 0.015 * f64::from(w).hypot(f64::from(h)))
            .map(|s| upright::Line::new((s.a.0 - cx, s.a.1 - cy), (s.b.0 - cx, s.b.1 - cy), 1.0))
            .collect()
    }

    /// The worst distance (output pixels) of the facade's vertical lines from plumb after
    /// `plan`, over the lines that show.
    fn plumb_error(&self, plan: &Plan) -> f64 {
        let back = inverse(&self.to_facade).unwrap();
        let (cx, cy) = (f64::from(self.w) / 2.0, f64::from(self.h) / 2.0);
        let mut worst = 0f64;
        for k in -6..=6 {
            let xs: Vec<f64> = (-6..=6)
                .filter_map(|j| {
                    let q = (f64::from(k) * SPACING, f64::from(j) * 20.0);
                    let o = apply(&back, q)?;
                    let o = (o.0 + cx, o.1 + cy);
                    let inside =
                        o.0 > 2.0 && o.1 > 2.0 && o.0 < cx * 2.0 - 2.0 && o.1 < cy * 2.0 - 2.0;
                    inside.then_some(())?;
                    let (s, _) = self.base.frame_to_source(1, o);
                    plan.to_output(s, 1.0).map(|p| p.0)
                })
                .collect();
            if xs.len() >= 4 {
                let m = xs.iter().sum::<f64>() / xs.len() as f64;
                worst = xs.iter().fold(worst, |a, x| a.max((x - m).abs()));
            }
        }
        worst
    }

    fn plan(&self, p: &PerspParams) -> Plan {
        let e = with_persp(edit(CropOrientation::default(), Some(self.lens.clone())), p);
        Plan::new(&e, &Context::image(self.w, self.h, Orient::default()), true)
    }
}

#[test]
fn auto_upright_recovers_a_known_camera() {
    let truth = [12.0, -8.0, 2.0];
    let sc = scene(truth);
    let lines = sc.lines();
    let (w, h) = (f64::from(sc.w), f64::from(sc.h));
    for goal in [Goal::Full, Goal::Auto] {
        let s = upright::auto(goal, &lines, w, h, Some(sc.f), 0.0, 0.0).expect("solves");
        let got = [s.pitch, s.yaw, s.roll].map(f64::to_degrees);
        let err: Vec<f64> = got.iter().zip(truth).map(|(a, b)| (a - b).abs()).collect();
        eprintln!("{goal:?}: {got:?} (error {err:?})");
        assert!(err.iter().all(|e| *e < 0.3), "{goal:?}: {got:?}");
        let p = PerspParams {
            mode: if goal == Goal::Full {
                Mode::Full
            } else {
                Mode::Auto
            },
            solved: Some(got.map(|v| v as f32)),
            ..PerspParams::default()
        };
        let plumb = sc.plumb_error(&sc.plan(&p));
        eprintln!("{goal:?}: verticals within {plumb:.3} px of plumb");
        assert!(plumb < 0.5, "{goal:?}: {plumb}");
    }
    // Vertical alone fixes pitch and roll; the verticals come out plumb all the same.
    let s = upright::auto(Goal::Vertical, &lines, w, h, Some(sc.f), 0.0, 0.0).unwrap();
    assert_eq!(s.yaw, 0.0);
    let p = PerspParams {
        mode: Mode::Vertical,
        solved: Some([s.pitch, s.yaw, s.roll].map(|v| v.to_degrees() as f32)),
        ..PerspParams::default()
    };
    let plumb = sc.plumb_error(&sc.plan(&p));
    assert!(plumb < 0.5, "vertical: {plumb}");
    // Uncorrected, they lean.
    assert!(sc.plumb_error(&sc.plan(&PerspParams::default())) > 5.0);
}

#[test]
fn too_few_lines_refuse() {
    let (w, h) = (400.0, 300.0);
    let one = [upright::Line::new((0.0, -100.0), (3.0, 100.0), 1.0)];
    assert_eq!(
        upright::auto(Goal::Vertical, &one, w, h, None, 0.0, 0.0),
        Err(upright::Refusal::FewLines)
    );
    assert_eq!(
        upright::auto(Goal::Full, &[], w, h, None, 0.0, 0.0),
        Err(upright::Refusal::FewLines)
    );
}

#[test]
fn guided_lines_come_out_plumb() {
    let sc = scene([9.0, 5.0, -1.5]);
    let back = inverse(&sc.to_facade).unwrap();
    let (cx, cy) = (f64::from(sc.w) / 2.0, f64::from(sc.h) / 2.0);
    // Two facade verticals, as source fractions (as the tool stores them).
    let frac = |q: (f64, f64)| {
        let o = apply(&back, q).unwrap();
        let (s, _) = sc.base.frame_to_source(1, (o.0 + cx, o.1 + cy));
        [
            (s.0 / f64::from(sc.w)) as f32,
            (s.1 / f64::from(sc.h)) as f32,
        ]
    };
    let guide = |x: f64| {
        let a = frac((x, -100.0));
        let b = frac((x, 100.0));
        [a[0], a[1], b[0], b[1]]
    };
    let p = PerspParams {
        mode: Mode::Guided,
        guides: vec![guide(-2.0 * SPACING), guide(3.0 * SPACING)],
        ..PerspParams::default()
    };
    let plan = sc.plan(&p);
    assert!(plan.persp.is_some());
    let plumb = sc.plumb_error(&plan);
    eprintln!("guided: verticals within {plumb:.3} px of plumb");
    assert!(plumb < 0.5, "{plumb}");
}

/// Polygons with anti-aliased edges: LSD finds nearly every edge longer than 30 px.
#[test]
fn lsd_finds_polygon_edges() {
    let (w, h) = (400usize, 300usize);
    let polys: Vec<(Vec<(f64, f64)>, f32)> = vec![
        (
            vec![(20.0, 20.0), (150.0, 30.0), (130.0, 120.0), (30.0, 110.0)],
            0.9,
        ),
        (vec![(200.0, 20.0), (380.0, 40.0), (290.0, 130.0)], 0.1),
        (vec![(30.0, 160.0), (180.0, 150.0), (170.0, 280.0)], 0.75),
        (
            vec![
                (230.0, 170.0),
                (330.0, 160.0),
                (380.0, 230.0),
                (300.0, 285.0),
                (220.0, 250.0),
            ],
            0.15,
        ),
    ];
    let inside = |p: &[(f64, f64)], x: f64, y: f64| {
        let n = p.len();
        (0..n).all(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0) >= 0.0
        })
    };
    let mut img = vec![0.45f32; w * h];
    for y in 0..h {
        for x in 0..w {
            // 4 × 4 supersampling.
            let mut acc = 0.0;
            for j in 0..4 {
                for i in 0..4 {
                    let (sx, sy) = (
                        x as f64 + (f64::from(i) + 0.5) / 4.0,
                        y as f64 + (f64::from(j) + 0.5) / 4.0,
                    );
                    let mut v = 0.45;
                    for (p, g) in &polys {
                        if inside(p, sx, sy) {
                            v = *g;
                        }
                    }
                    acc += v;
                }
            }
            img[y * w + x] = acc / 16.0;
        }
    }
    let t = std::time::Instant::now();
    let segs = lsd::detect(&img, w, h, 255.0);
    eprintln!(
        "LSD: {} segments in {:.0} ms",
        segs.len(),
        t.elapsed().as_secs_f64() * 1e3
    );
    let (mut edges, mut found) = (0, 0);
    for (p, _) in &polys {
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            if len <= 30.0 {
                continue;
            }
            edges += 1;
            let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
            let dist = |q: (f64, f64)| ((q.0 - a.0) * uy - (q.1 - a.1) * ux).abs();
            let along = |q: (f64, f64)| (q.0 - a.0) * ux + (q.1 - a.1) * uy;
            // Coverage of the edge by segments lying on it.
            let mut cover = vec![false; len as usize + 1];
            for s in &segs {
                if dist(s.a) < 2.0 && dist(s.b) < 2.0 {
                    let (t0, t1) = (along(s.a).min(along(s.b)), along(s.a).max(along(s.b)));
                    for (k, c) in cover.iter_mut().enumerate() {
                        let t = k as f64;
                        if t >= t0 && t <= t1 {
                            *c = true;
                        }
                    }
                }
            }
            let frac = cover.iter().filter(|c| **c).count() as f64 / cover.len() as f64;
            if frac >= 0.6 {
                found += 1;
            } else {
                eprintln!("missed edge {a:?}–{b:?}: {frac:.2} covered");
            }
        }
    }
    let recall = f64::from(found) / f64::from(edges);
    eprintln!("LSD recall {found}/{edges} = {recall:.3}");
    assert!(recall >= 0.95, "{recall}");
}

/// The a-contrario rule: noise gives no segments.
#[test]
fn lsd_sees_nothing_in_noise() {
    let (w, h) = (256usize, 256usize);
    let mut rng = Rng(77);
    let img: Vec<f32> = (0..w * h).map(|_| rng.next() as f32).collect();
    let segs = lsd::detect(&img, w, h, 255.0);
    assert!(segs.is_empty(), "{segs:?}");
    // Flat grey: nothing either.
    assert!(lsd::detect(&vec![0.5; w * h], w, h, 255.0).is_empty());
}

/// The largest crop of a keystoned and turned frame keeps all of itself on the image.
#[test]
fn largest_crop_stays_inside_the_warped_frame() {
    let mut rng = Rng(4242);
    let (w, h) = (600u32, 400u32);
    let ctx = Context::image(w, h, Orient::default());
    for i in 0..60 {
        let r = |rng: &mut Rng, k: f64| (rng.next() * 2.0 - 1.0) * k;
        let p = PerspParams {
            vertical: r(&mut rng, 100.0) as f32,
            horizontal: r(&mut rng, 100.0) as f32,
            rotate: r(&mut rng, 10.0) as f32,
            aspect: r(&mut rng, 50.0) as f32,
            auto_scale: i % 2 == 0,
            ..PerspParams::default()
        };
        let c = CropOrientation {
            angle_deg: r(&mut rng, 20.0) as f32,
            ..Default::default()
        };
        let plan = Plan::new(&with_persp(edit(c, Some(no_lens())), &p), &ctx, true);
        let fr = plan.crop_frame();
        let ratio = [None, Some(1.0), Some(16.0 / 9.0)][i % 3];
        let cr = crop::largest_in(ratio, &fr);
        assert!(valid_crop(&cr), "{i}: {cr:?}");
        // Every point of its edges reads the image, not only the corners.
        let (x0, y0, x1, y1) = (cr[0], cr[1], cr[2], cr[3]);
        for k in 0..=40 {
            let t = f64::from(k) / 40.0;
            for (u, v) in [
                (x0 + t * (x1 - x0), y0),
                (x0 + t * (x1 - x0), y1),
                (x0, y0 + t * (y1 - y0)),
                (x1, y0 + t * (y1 - y0)),
            ] {
                let (px, py) = (u * plan.oriented.0, v * plan.oriented.1);
                assert!(fr.inside(px, py), "{i}: ({u}, {v}) outside");
                let (s, _) = plan.frame_to_source(1, (px, py));
                assert!(
                    s.0 >= -1e-6
                        && s.1 >= -1e-6
                        && s.0 <= f64::from(w) + 1e-6
                        && s.1 <= f64::from(h) + 1e-6,
                    "{i}: {s:?}"
                );
            }
        }
    }
}

#[test]
fn fit_inside_a_turned_and_keystoned_frame() {
    let (w, h) = (600u32, 400u32);
    let p = PerspParams {
        vertical: 70.0,
        horizontal: 30.0,
        auto_scale: false,
        ..PerspParams::default()
    };
    let c = CropOrientation {
        angle_deg: 6.0,
        ..Default::default()
    };
    let plan = Plan::new(
        &with_persp(edit(c, Some(no_lens())), &p),
        &Context::image(w, h, Orient::default()),
        true,
    );
    let fr = plan.crop_frame();
    let full = [0.0, 0.0, 1.0, 1.0];
    assert!(!fr.crop_inside(full));
    let fit = crop::fit_inside_in(full, &fr);
    assert!(fr.crop_inside(fit));
    // As large as it can be: a little more is outside.
    let (cx, cy) = ((fit[0] + fit[2]) / 2.0, (fit[1] + fit[3]) / 2.0);
    let k = 1.01;
    let more = [
        cx - (cx - fit[0]) * k,
        cy - (cy - fit[1]) * k,
        cx + (fit[2] - cx) * k,
        cy + (fit[3] - cy) * k,
    ];
    assert!(!fr.crop_inside(more));
    // The angle wrappers still describe the turn alone.
    let turned = Frame::turned(600.0, 400.0, 6f64.to_radians());
    assert_eq!(
        crop::fit_inside(full, 600.0, 400.0, 6f64.to_radians()),
        crop::fit_inside_in(full, &turned)
    );
    // The outline is the warped frame's corners.
    let q = fr.outline().unwrap();
    for (x, y) in q {
        let (s, _) = plan.frame_to_source(1, (x, y));
        let corner = [(0.0, 0.0), (600.0, 0.0), (600.0, 400.0), (0.0, 400.0)]
            .iter()
            .any(|c| (c.0 - s.0).abs() < 1e-6 && (c.1 - s.1).abs() < 1e-6);
        assert!(corner, "{s:?}");
    }
}

/// Masks' coordinate grid (bilinear over 65 × 65 points) at the strongest keystone.
#[test]
fn mask_grid_tracks_the_strongest_keystone() {
    use crate::mask::{Geometry, grid_side, make_grid};
    let p = PerspParams {
        vertical: 100.0,
        horizontal: 100.0,
        rotate: 10.0,
        ..PerspParams::default()
    };
    let e = with_persp(
        edit(
            CropOrientation {
                angle_deg: 5.0,
                ..Default::default()
            },
            Some(no_lens()),
        ),
        &p,
    );
    let ctx = Context::image(6000, 4000, Orient::default());
    let plan = Plan::new(&e, &ctx, true);
    assert!(plan.persp.is_some());
    let g = Geometry::Warp(&plan);
    let grid = make_grid(&g);
    let n = grid_side(&g) as usize;
    let mut rng = Rng(9);
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
        let d = ((lerp(0) - exact.0) * 6000.0).hypot((lerp(1) - exact.1) * 4000.0);
        worst = worst.max(d);
    }
    eprintln!("grid vs exact keystone: worst {worst:.4} px at 6000 px");
    assert!(worst < 0.1, "{worst}");
}

#[test]
fn the_sidecar_keeps_perspective_and_old_edits_stay() {
    let r = Registry::builtin();
    let base = edit(CropOrientation::default(), Some(no_lens()));
    let p = PerspParams {
        mode: Mode::Guided,
        vertical: 12.5,
        focal_mm35: 24.0,
        guides: vec![[0.1, 0.2, 0.12, 0.9], [0.8, 0.1, 0.85, 0.95]],
        solved: Some([3.0, -1.0, 0.5]),
        ..PerspParams::default()
    };
    let e = with_persp(base.clone(), &p);
    let text = crate::sidecar::to_xmp(&e, None);
    assert!(text.contains("spd:module=\"perspective\""));
    let back = crate::sidecar::from_xmp(&text, r).unwrap();
    assert_eq!(back.edit, e);
    assert_eq!(
        persp::persp_params(&back.edit),
        Some(PerspParams::from_fields(&p.to_fields()))
    );
    // An edit without the module writes as before and has no perspective.
    let old = crate::sidecar::to_xmp(&base, None);
    assert!(!old.contains("perspective"));
    let back = crate::sidecar::from_xmp(&old, r).unwrap();
    assert_eq!(persp::persp_params(&back.edit), None);
    // Geometry: left out of styles.
    assert!(r.get(ModuleKind::PERSPECTIVE).unwrap().geometry);
}

/// The kernel's extra cost: a keystoned render against the same render turned only.
#[test]
fn keystone_costs_little_on_the_gpu() {
    let (sw, sh) = (2000u32, 1500u32);
    let src = image(sw, sh);
    let c = CropOrientation {
        angle_deg: 3.0,
        ..Default::default()
    };
    let ctx = Context::image(sw, sh, Orient::default());
    let turned = Plan::new(&edit(c.clone(), Some(no_lens())), &ctx, true);
    let keyed = Plan::new(
        &with_persp(edit(c, Some(no_lens())), &keystoned()),
        &ctx,
        true,
    );
    let Some(d) = crate::testgpu::dev() else {
        return;
    };
    let _l = crate::testgpu::lock();
    let mut engine = crate::gpu::Engine::new(
        &d.device,
        &d.queue,
        crate::gpu::pool::Budget::fixed(1 << 30),
    );
    let s = engine.upload(sw, sh, &src).expect("upload");
    let mut w = warp::Warper::new(&d.device, &d.queue);
    let mut time = |plan: &Plan| {
        let mut best = f64::MAX;
        for _ in 0..8 {
            let t = std::time::Instant::now();
            let out = w
                .run(
                    plan,
                    s.buffer(),
                    sw,
                    sh,
                    Precision::F32,
                    sw,
                    sh,
                    Precision::F16,
                )
                .expect("warp");
            let _ = d.device.poll(wgpu::PollType::wait_indefinitely());
            best = best.min(t.elapsed().as_secs_f64());
            w.give_back(out);
        }
        best
    };
    let a = time(&turned);
    let b = time(&keyed);
    eprintln!(
        "warp {sw}×{sh}: turned {:.2} ms, keystoned {:.2} ms ({:+.1} %)",
        a * 1e3,
        b * 1e3,
        (b / a - 1.0) * 100.0
    );
}
