//! Borrowed masks bit-exact, the details mask across scales, refine steps, output
//! ranges with refinement, pressure, no kernels when the new values move, and the brush
//! budget with pressure.

use super::*;
use crate::model::mask::{Details, PressureUse};

fn grey_of(px: &[f32]) -> Vec<f32> {
    px.chunks_exact(4).map(|p| p[0]).collect()
}

/// Two exposure instances: `src` (instance 0) with `blend`, the user (instance 1)
/// borrowing its mask; `user_first` puts the user before the source.
fn borrowing(blend: Blend, user_first: bool) -> Edit {
    let mut e = Edit::empty(Default::default());
    let src = exposure(1.0, Some(blend));
    let mut user = exposure(
        -0.5,
        Some(Blend {
            mask: MaskExpr::FromModule {
                kind: ModuleKind::EXPOSURE,
                instance: 0,
            },
            ..Blend::default()
        }),
    );
    user.instance = 1;
    if user_first {
        e.modules.push(user);
        e.modules.push(src);
    } else {
        e.modules.push(src);
        e.modules.push(user);
    }
    e.masks = shapes();
    e
}

#[test]
fn a_borrowed_mask_is_the_source_mask_bit_for_bit() {
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
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    let s = (ModuleKind::EXPOSURE, 0);
    let u = (ModuleKind::EXPOSURE, 1);
    let mut cases = 0;
    for (name, expr) in exprs() {
        // Ranges test each module's own input; the rest must agree exactly.
        let mut ranges = false;
        fn has_range(e: &MaskExpr, out: &mut bool) {
            match e {
                MaskExpr::Range(_) | MaskExpr::ColourRange(_) => *out = true,
                MaskExpr::Invert(a) | MaskExpr::Opacity(_, a) => has_range(a, out),
                MaskExpr::Combine(_, a, b) => {
                    has_range(a, out);
                    has_range(b, out);
                }
                _ => {}
            }
        }
        has_range(&expr, &mut ranges);
        if ranges {
            continue;
        }
        for (invert, user_first) in [(false, false), (true, false), (true, true)] {
            let e = borrowing(
                Blend {
                    mask: expr.clone(),
                    invert,
                    ..Blend::default()
                },
                user_first,
            );
            let (a, _) = run.render(&e, &src, geometry, Some((s, Overlay::Grey)));
            let (b, _) = run.render(&e, &src, geometry, Some((u, Overlay::Grey)));
            let diff = grey_of(&a)
                .iter()
                .zip(grey_of(&b))
                .filter(|(x, y)| x.to_bits() != y.to_bits())
                .count();
            assert_eq!(diff, 0, "{name} invert {invert} user first {user_first}");
            cases += 1;
        }
    }
    eprintln!("borrowed masks: {cases} cases, 0 pixels differ");
    assert!(cases >= 6);
}

/// A source with contrast, offset and refinement: borrowed whole, the user's mask is the
/// source's blend in the user's place bit for bit (refinement included); borrowed inside an
/// expression it takes the contrast and offset per pixel; the CPU reference agrees.
#[test]
fn a_borrowed_mask_takes_the_source_contrast_offset_and_refinement() {
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
    let mut run = Run {
        engine: &mut engine,
        cache: &mut cache,
    };
    let s = (ModuleKind::EXPOSURE, 0);
    let u = (ModuleKind::EXPOSURE, 1);
    let level = Refine {
        contrast: 0.6,
        offset: -0.15,
        ..Refine::default()
    };
    let refined = Refine {
        feather: 0.03,
        blur: 0.01,
        edge_aware: 0.7,
        ..level.clone()
    };
    let mut cases = 0;
    for (name, refine) in [("contrast and offset", level.clone()), ("refined", refined)] {
        for invert in [false, true] {
            let e = borrowing(
                Blend {
                    mask: MaskExpr::Shape(1),
                    invert,
                    refine: refine.clone(),
                    ..Blend::default()
                },
                false,
            );
            // The source's blend written out in the user's own place: the borrowed mask is it
            // bit for bit (edge refinement is guided by the user's own input either way).
            // The source's own overlay shows its mask, not all white (a later
            // refine pass no longer overwrites it).
            let (so, _) = run.render(&e, &src, geometry, Some((s, Overlay::Grey)));
            assert!(grey_of(&so).iter().any(|&v| v < 0.5), "{name}: all white");
            let mut own = e.clone();
            own.modules[1].blend = own.modules[0].blend.clone();
            let (a, _) = run.render(&own, &src, geometry, Some((u, Overlay::Grey)));
            let (b, _) = run.render(&e, &src, geometry, Some((u, Overlay::Grey)));
            let diff = grey_of(&a)
                .iter()
                .zip(grey_of(&b))
                .filter(|(x, y)| x.to_bits() != y.to_bits())
                .count();
            assert_eq!(diff, 0, "{name} invert {invert}");
            // The mask did change from the plain one.
            let plain = borrowing(
                Blend {
                    mask: MaskExpr::Shape(1),
                    invert,
                    ..Blend::default()
                },
                false,
            );
            let (c, _) = run.render(&plain, &src, geometry, Some((u, Overlay::Grey)));
            assert!(grey_of(&c) != grey_of(&b), "{name}: no effect");
            cases += 1;
        }
    }
    // Inside an expression: contrast and offset per pixel (no refinement pass).
    let mut e = borrowing(
        Blend {
            mask: MaskExpr::Shape(1),
            invert: true,
            refine: level,
            ..Blend::default()
        },
        false,
    );
    e.modules[1].blend.as_mut().expect("b").mask = MaskExpr::Opacity(
        1.0,
        Box::new(MaskExpr::FromModule {
            kind: ModuleKind::EXPOSURE,
            instance: 0,
        }),
    );
    let (a, _) = run.render(&e, &src, geometry, Some((s, Overlay::Grey)));
    let (b, _) = run.render(&e, &src, geometry, Some((u, Overlay::Grey)));
    let worst = grey_of(&a)
        .iter()
        .zip(grey_of(&b))
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    assert!(worst < 1e-6, "nested: {worst}");
    // The CPU reference: the user's blend weight equals the source's, whole and nested.
    let cx = Ctx {
        edit: &e,
        space: Space::of(f64::from(w), f64::from(h)),
        aa: 1.0 / f64::from(w.max(h)),
        rasters: HashMap::new(),
        ai: HashMap::new(),
        detail: Default::default(),
    };
    let weight = |bl: &Blend, f: (f64, f64)| {
        let one = Blend {
            mode: BlendMode::Normal,
            opacity: 1.0,
            ..bl.clone()
        };
        1.0 - cpu_ref::blend_pixel(&cx, &one, f, [1.0; 3], [0.0; 3])[0]
    };
    let sb = e.modules[0].blend.clone().expect("source");
    let nested = e.modules[1].blend.clone().expect("user");
    let whole = Blend {
        mask: MaskExpr::FromModule {
            kind: ModuleKind::EXPOSURE,
            instance: 0,
        },
        ..Blend::default()
    };
    let mut cpu_worst = 0.0f64;
    for j in 0..24 {
        for i in 0..32 {
            let f = ((f64::from(i) + 0.5) / 32.0, (f64::from(j) + 0.5) / 24.0);
            let want = weight(&sb, f);
            cpu_worst = cpu_worst
                .max((weight(&nested, f) - want).abs())
                .max((weight(&whole, f) - want).abs());
        }
    }
    assert!(cpu_worst < 1e-12, "CPU reference {cpu_worst}");
    eprintln!(
        "borrowed refinement: {cases} whole cases bit-exact, nested GPU worst {worst:.1e}, CPU {cpu_worst:.1e}"
    );
}

#[test]
fn a_borrowed_mask_follows_the_source_and_survives_reorder() {
    let mut e = borrowing(
        Blend {
            mask: MaskExpr::Shape(1),
            ..Blend::default()
        },
        false,
    );
    let user = |e: &Edit| crate::model::hash::module_hash(e, &e.modules[1]);
    let before = user(&e);
    if let Shape::Ellipse { centre, .. } = &mut e.masks[0].shape {
        centre[0] += 0.1;
    }
    let moved = user(&e);
    assert_ne!(before, moved, "the user's hash follows the source's shape");
    e.modules[0].blend.as_mut().expect("b").invert = true;
    assert_ne!(moved, user(&e), "and its invert");
    e.modules.swap(0, 1);
    let u = e.module(ModuleKind::EXPOSURE, 1).expect("user").clone();
    assert!(u.blend.expect("b").mask.refers_to(ModuleKind::EXPOSURE, 0));
    // Deleting the source copies its mask in first.
    assert_eq!(
        e.mask_users(ModuleKind::EXPOSURE, 0),
        vec![(ModuleKind::EXPOSURE, 1)]
    );
    e.inline_mask_users(ModuleKind::EXPOSURE, 0);
    let b = e
        .module(ModuleKind::EXPOSURE, 1)
        .and_then(|m| m.blend.clone());
    assert_eq!(
        b.expect("b").mask,
        MaskExpr::Invert(Box::new(MaskExpr::Shape(1)))
    );
    assert!(e.mask_users(ModuleKind::EXPOSURE, 0).is_empty());
    // A copied-in combination lists as one row (a group).
    let group = MaskExpr::Combine(
        Combine::Union,
        Box::new(MaskExpr::Shape(2)),
        Box::new(MaskExpr::Combine(Combine::Subtract, sh(1), sh(3))),
    );
    let rows = group.layers().expect("rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(MaskExpr::from_layers(&rows), group);
}

/// Strong texture on the left (two octaves of hashed noise), smooth on the right.
fn textured(w: u32, h: u32, cell: f32) -> Vec<f32> {
    let hash = |x: i64, y: i64| {
        let mut v = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) as u64;
        v ^= v >> 13;
        v = v.wrapping_mul(0x5bd1_e995);
        v ^= v >> 15;
        (v & 0xFFFF) as f32 / 65535.0
    };
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            // Texture in frame units, so both sizes show the same pattern.
            let (fx, fy) = (u * cell, t * cell * h as f32 / w as f32);
            let n = hash(fx as i64, fy as i64);
            let k = if u < 0.5 {
                0.18 * (2.4 * (n - 0.5)).exp2()
            } else {
                0.05 + 0.3 * u * t
            };
            v.extend_from_slice(&[k, k, k, 1.0]);
        }
    }
    v
}

/// The details mask of `e` on the full image and on it reduced by `f`, compared at the
/// reduced size away from the edges and the texture's border: (IoU of the masks over ½,
/// mean |Δ|, mean on the textured half, mean on the smooth half), the latter two of the
/// reduced render.
fn details_at_scales(
    engine: &mut Engine,
    e: &Edit,
    full: &[f32],
    (w, h): (u32, u32),
    f: u32,
) -> (f64, f64, f64, f64) {
    let small = crate::ops::tone::tests::shrink(full, w, h, f);
    let sf = engine.upload(w, h, full).expect("upload");
    let ss = engine.upload(w / f, h / f, &small).expect("upload");
    let id = (ModuleKind::EXPOSURE, 0);
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let mut render = |s: &crate::gpu::Source| {
        let mut c = MaskCache::default();
        Run {
            engine: &mut *engine,
            cache: &mut c,
        }
        .render(e, s, geometry, Some((id, Overlay::Grey)))
        .0
    };
    let a = render(&sf);
    let b = render(&ss);
    let a = crate::ops::tone::tests::shrink(&a, w, h, f);
    let (qw, qh) = (w / f, h / f);
    let margin = 32 / f;
    let (mut inter, mut uni, mut sum, mut n) = (0u32, 0u32, 0f64, 0u32);
    let (mut left, mut nl, mut right, mut nr) = (0f64, 0u32, 0f64, 0u32);
    for y in margin..qh - margin {
        for x in margin..qw - margin {
            if (x as i32 - qw as i32 / 2).unsigned_abs() < margin {
                continue;
            }
            let i = ((y * qw + x) * 4) as usize;
            let (p, q) = (a[i], b[i]);
            inter += u32::from(p > 0.5 && q > 0.5);
            uni += u32::from(p > 0.5 || q > 0.5);
            sum += f64::from((p - q).abs());
            n += 1;
            if x < qw / 2 {
                left += f64::from(q);
                nl += 1;
            } else {
                right += f64::from(q);
                nr += 1;
            }
        }
    }
    (
        f64::from(inter) / f64::from(uni.max(1)),
        sum / f64::from(n),
        left / f64::from(nl),
        right / f64::from(nr),
    )
}

#[test]
fn the_details_mask_is_stable_from_preview_to_export() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (2048u32, 1364u32);
    // Cells of 1/160 of the width: 12.8 px here, 6.4 px at half size, 3.2 at a quarter.
    let full = textured(w, h, 160.0);
    let mut engine = engine(d);
    let mask = |d: Details| {
        edit_with(
            Blend {
                mask: MaskExpr::Details(d),
                ..Blend::default()
            },
            Vec::new(),
        )
    };
    // The default mask, and a wide ramp that reads the measure out (0 to 1.2 EV).
    let default = mask(Details::default());
    let readout = mask(Details {
        threshold: 0.6,
        softness: 1.2,
    });
    for f in [2u32, 4] {
        let (iou, mad, tex, smooth) = details_at_scales(&mut engine, &default, &full, (w, h), f);
        let (_, rmad, rtex, _) = details_at_scales(&mut engine, &readout, &full, (w, h), f);
        let (_, _, ftex, _) = details_at_scales(&mut engine, &readout, &full, (w, h), 1);
        eprintln!(
            "details mask, export vs 1/{f} preview: IoU {iou:.4}, mean |Δ| {mad:.4} \
             (textured {tex:.3}, smooth {smooth:.3}); measure read-out mean |Δ| {rmad:.4}, \
             textured {rtex:.3} vs {ftex:.3} at full size"
        );
        assert!(tex > 0.9, "texture selected");
        assert!(smooth < 0.02, "smooth left out");
        if f == 2 {
            assert!(iou > 0.95, "{iou}");
            assert!(mad < 0.05, "{mad}");
            assert!(rmad < 0.08, "{rmad}");
        } else {
            assert!(iou > 0.85, "{iou}");
        }
    }
    // `details` on another mask keeps it to the flat part.
    let mut e = default.clone();
    e.modules[0].blend.as_mut().expect("b").mask = MaskExpr::All;
    e.modules[0].blend.as_mut().expect("b").refine.details = -0.25;
    let (qw, qh) = (w / 2, h / 2);
    let small = crate::ops::tone::tests::shrink(&full, w, h, 2);
    let s = engine.upload(qw, qh, &small).expect("upload");
    let mut c = MaskCache::default();
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let (px, _) = Run {
        engine: &mut engine,
        cache: &mut c,
    }
    .render(
        &e,
        &s,
        geometry,
        Some(((ModuleKind::EXPOSURE, 0), Overlay::Grey)),
    );
    let at = |x: u32, y: u32| px[((y * qw + x) * 4) as usize];
    assert!(at(qw / 8, qh / 2) < 0.1 && at(qw * 7 / 8, qh / 2) > 0.9);
}

#[test]
fn feather_grows_blur_softens_and_radius_scales() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (400u32, 100u32);
    let img = vec![0.2f32; (w * h * 4) as usize];
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    // A hard-edged rectangle over the left half.
    let masks = vec![MaskShape {
        id: 1,
        shape: Shape::Rectangle {
            centre: [0.25, 0.5],
            half: [0.25, 1.0],
            angle_deg: 0.0,
            round: 0.0,
            feather: 0.0,
        },
    }];
    let id = (ModuleKind::EXPOSURE, 0);
    let row = |engine: &mut Engine, refine: Refine| {
        let e = edit_with(
            Blend {
                mask: MaskExpr::Shape(1),
                refine,
                ..Blend::default()
            },
            masks.clone(),
        );
        let mut c = MaskCache::default();
        let px = Run {
            engine,
            cache: &mut c,
        }
        .render(&e, &src, geometry, Some((id, Overlay::Grey)))
        .0;
        (0..w)
            .map(|x| px[(((h / 2) * w + x) * 4) as usize])
            .collect::<Vec<_>>()
    };
    let plain = row(&mut engine, Refine::default());
    let fe = row(
        &mut engine,
        Refine {
            feather: 0.05,
            ..Refine::default()
        },
    );
    let bl = row(
        &mut engine,
        Refine {
            blur: 0.05,
            ..Refine::default()
        },
    );
    // 0.05 of the long side is 20 px.
    let edge = 200usize;
    assert!(plain[edge + 5] < 0.01);
    // Feather: the inside stays, a soft edge grows outwards and ends within the radius.
    assert!(fe[edge - 10] > 0.999 && fe[edge - 1] > 0.999);
    assert!(fe[edge + 5] > 0.3 && fe[edge + 5] < 1.0, "{}", fe[edge + 5]);
    assert!(fe[edge + 30] < 0.01);
    // Blur: soft on both sides.
    assert!(bl[edge - 3] < 0.95 && bl[edge + 3] > 0.05);
    assert!((bl[edge - 1] + bl[edge] - 1.0).abs() < 0.1);
    eprintln!(
        "feather at +5 px {:.3}; blur at -3/+3 px {:.3}/{:.3}",
        fe[edge + 5],
        bl[edge - 3],
        bl[edge + 3]
    );
}

#[test]
fn output_ranges_with_refine_see_the_output() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (128u32, 96u32);
    // Input at −0.85 EV; the module adds 1.5 EV.
    let img = vec![0.1f32; (w * h * 4) as usize];
    let mut engine = engine(d);
    let src = engine.upload(w, h, &img).expect("upload");
    let geometry = Geometry::Frame {
        w: f64::from(w),
        h: f64::from(h),
    };
    let id = (ModuleKind::EXPOSURE, 0);
    let mut c = MaskCache::default();
    let mut run = Run {
        engine: &mut engine,
        cache: &mut c,
    };
    for (on_output, want) in [(true, 1.0f32), (false, 0.0)] {
        for refine in [
            Refine::default(),
            Refine {
                blur: 0.02,
                edge_aware: 0.5,
                ..Refine::default()
            },
        ] {
            let e = edit_with(
                Blend {
                    mask: MaskExpr::Range(Range {
                        channel: RangeChannel::Luminance,
                        on_output,
                        limits: [0.2, 0.4, 2.0, 3.0],
                    }),
                    refine: refine.clone(),
                    ..Blend::default()
                },
                Vec::new(),
            );
            let (px, _) = run.render(&e, &src, geometry, Some((id, Overlay::Grey)));
            let m = px[((h / 2 * w + w / 2) * 4) as usize];
            assert!(
                (m - want).abs() < 1e-3,
                "output {on_output} refined {} mask {m}",
                refine.needs_pass()
            );
        }
    }
}

/// Moving details, feather, blur, radius and refine values makes no kernel once the
/// refine pass is there; nor does a pressure stroke.
#[test]
fn new_mask_values_build_no_kernel() {
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
            mask: MaskExpr::Combine(
                Combine::Intersect,
                sh(1),
                Box::new(MaskExpr::Details(Details::default())),
            ),
            refine: Refine {
                feather: 0.01,
                blur: 0.01,
                edge_aware: 0.5,
                details: 0.1,
                ..Refine::default()
            },
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
    fn r(e: &mut Edit) -> &mut Refine {
        &mut e.modules[0].blend.as_mut().expect("b").refine
    }
    let steps: Vec<Change> = vec![
        Box::new(move |e| r(e).feather = 0.03),
        Box::new(move |e| r(e).blur = 0.002),
        Box::new(move |e| r(e).radius = 0.05),
        Box::new(move |e| r(e).edge_aware = 0.9),
        Box::new(move |e| r(e).details = -0.4),
        Box::new(|e| {
            e.modules[0].blend.as_mut().expect("b").mask = MaskExpr::Combine(
                Combine::Intersect,
                sh(6),
                Box::new(MaskExpr::Details(Details {
                    threshold: 0.6,
                    softness: 0.05,
                })),
            );
        }),
        Box::new(|e| {
            if let Shape::Brush { strokes } = &mut e.masks[5].shape {
                strokes.push(Stroke {
                    radius: 0.05,
                    hardness: 0.5,
                    flow: 1.0,
                    erase: false,
                    pressure: PressureUse::Both,
                    points: vec![[0.5, 0.5, 0.2], [0.6, 0.5, 0.9]],
                });
            }
        }),
    ];
    for (i, f) in steps.iter().enumerate() {
        f(&mut e);
        let (_, stats) = run.render(&e, &src, geometry, None);
        assert_eq!(stats.pipelines_created, 0, "step {i} built a kernel");
    }
}

#[test]
fn pressure_scales_size_or_flow() {
    let stroke = |p: PressureUse, pressure: f32| Shape::Brush {
        strokes: vec![Stroke {
            radius: 0.05,
            hardness: 1.0,
            flow: 1.0,
            erase: false,
            pressure: p,
            points: vec![[0.5, 0.5, pressure]],
        }],
    };
    let raster = |s: Shape| {
        let mut e = edit_with(
            Blend {
                mask: MaskExpr::Shape(1),
                ..Blend::default()
            },
            vec![MaskShape { id: 1, shape: s }],
        );
        e.modules[0].enabled = true;
        let mut cache = MaskCache::default();
        compile(
            &e,
            &mut MaskEnv {
                geometry: Geometry::Frame { w: 200.0, h: 200.0 },
                image: (200, 200),
                scale: 1.0,
                cache: &mut cache,
                sensor_bin: 1.0,
                overlay: None,
            },
        )
        .expect("compiled");
        cache.raster(1).expect("raster").clone()
    };
    // Size: half pressure, 0.6 of the radius (6 px of 10), full strength inside.
    let r = raster(stroke(PressureUse::Size, 0.5));
    assert!(r.texel(100 + 4, 100) > 0.99 && r.texel(100 + 7, 100) < 0.01);
    // Flow: full size, half strength.
    let r = raster(stroke(PressureUse::Flow, 0.5));
    assert!((r.texel(100 + 7, 100) - 0.5).abs() < 0.01);
    // Both.
    let r = raster(stroke(PressureUse::Both, 0.5));
    assert!((r.texel(100 + 4, 100) - 0.5).abs() < 0.01 && r.texel(100 + 7, 100) < 0.01);
}

/// Brush feedback with pressure on a 24 MP photo's fit-view preview (2560 px), as the brush
/// test, every dab a different pressure. Printed; the bound is loose (the machine may be
/// loaded).
#[test]
fn brush_dab_latency_with_pressure() {
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
                    pressure: PressureUse::Both,
                    points: vec![[0.2, 0.5, 0.5]],
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
                0.5 + 0.5 * (i as f32 * 0.37).sin(),
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
    eprintln!("pressure brush dab at 2560×1707: p50 {p50:.1} ms, p95 {p95:.1} ms");
    assert!(p95 < 250.0, "{p95}");
}
