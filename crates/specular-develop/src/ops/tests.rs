//! The basic ops: properties on the CPU references, and GPU = CPU on synthetic images.

use super::cpu_ref;
use super::output::OutputSpace;
use crate::colour::{self, D50_XY, D65_XY};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::params::{ColourCalibration, Exposure, ViewTransform};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

/// The default non-raw stack with the tone equalizer (as the Develop panel adds it).
fn stack() -> Edit {
    let reg = Registry::builtin();
    let mut e = Edit::default_for(reg, SourceKey::default(), false);
    let mut te = ModuleInstance::new(reg, ModuleKind::TONE_EQUALIZER, 0);
    te.params = Params::from_fields(
        ModuleKind::TONE_EQUALIZER,
        Fields::new()
            .with("highlights_ev", Value::Float(0.0))
            .with("shadows_ev", Value::Float(0.0)),
    );
    e.insert(reg, te);
    e
}

fn set(e: &mut Edit, kind: ModuleKind, name: &str, v: Value) {
    let m = e.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

fn edited() -> Edit {
    let mut e = stack();
    set(
        &mut e,
        ModuleKind::EXPOSURE,
        "exposure_ev",
        Value::Float(0.7),
    );
    set(
        &mut e,
        ModuleKind::EXPOSURE,
        "black_offset",
        Value::Float(0.002),
    );
    set(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_source",
        Value::Text("custom".into()),
    );
    set(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_x",
        Value::Float(0.4100),
    );
    set(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_y",
        Value::Float(0.3940),
    );
    set(
        &mut e,
        ModuleKind::TONE_EQUALIZER,
        "highlights_ev",
        Value::Float(-0.8),
    );
    set(
        &mut e,
        ModuleKind::TONE_EQUALIZER,
        "shadows_ev",
        Value::Float(0.6),
    );
    set(
        &mut e,
        ModuleKind::VIEW_TRANSFORM,
        "contrast",
        Value::Float(1.8),
    );
    set(
        &mut e,
        ModuleKind::VIEW_TRANSFORM,
        "white_target",
        Value::Float(1.2),
    );
    set(
        &mut e,
        ModuleKind::VIEW_TRANSFORM,
        "black_target",
        Value::Float(0.01),
    );
    set(
        &mut e,
        ModuleKind::VIEW_TRANSFORM,
        "skew",
        Value::Float(0.3),
    );
    set(
        &mut e,
        ModuleKind::VIEW_TRANSFORM,
        "hue_preservation",
        Value::Float(0.6),
    );
    e
}

/// Ramps, saturated and out-of-gamut colours, darks, highlights far above white.
fn synthetic(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = x as f32 / (w - 1).max(1) as f32;
            let s = y as f32 / (h - 1).max(1) as f32;
            let ev = (t * 18.0 - 12.0).exp2();
            let hue = s * std::f32::consts::TAU;
            let sat = (y % 7) as f32 / 4.0;
            let c = [
                ev * (1.0 + sat * hue.cos()),
                ev * (1.0 + sat * (hue - 2.094).cos()),
                ev * (1.0 + sat * (hue + 2.094).cos()),
            ];
            v.extend_from_slice(&[c[0], c[1], c[2], 0.25 + 0.5 * s]);
        }
    }
    v
}

fn graph(e: &Edit, out: Option<&OutputSpace>) -> Graph {
    Graph::build_for(e, OpRegistry::builtin(), None, out).expect("graph")
}

fn cpu(e: &Edit, out: Option<&OutputSpace>, px: [f32; 4]) -> [f32; 4] {
    let r = cpu_ref::run(&graph(e, out), &px);
    [r[0], r[1], r[2], r[3]]
}

#[test]
fn the_default_stack_fuses_into_one_kernel_and_keeps_mid_grey() {
    let e = stack();
    let g = graph(&e, Some(&OutputSpace::rec709()));
    // Crop and orientation run in the fused warp before the graph.
    assert!(g.skipped.is_empty());
    assert_eq!(g.warp, vec![(ModuleKind::CROP_ORIENTATION, 0)]);
    // The tone equalizer at rest is left out: the default stack is one kernel.
    assert_eq!(g.identity, vec![(ModuleKind::TONE_EQUALIZER, 0)]);
    assert_eq!(g.nodes.len(), 4);
    assert_eq!(g.groups.len(), 1);
    assert!(g.groups[0].fused);
    // In use, it takes statistics on its input, so it starts a second kernel.
    let mut e2 = e.clone();
    set(
        &mut e2,
        ModuleKind::TONE_EQUALIZER,
        "shadows_ev",
        Value::Float(0.5),
    );
    let g = graph(&e2, Some(&OutputSpace::rec709()));
    assert_eq!(g.groups.len(), 2);
    assert!(g.groups.iter().all(|gr| gr.fused));
    assert!(g.stats_node(1).is_some());
    // Scene mid grey lands on display mid grey.
    let o = cpu(&e, None, [0.1845; 4]);
    for c in &o[..3] {
        assert!((c - 0.1845).abs() < 1e-4, "{o:?}");
    }
    // Whatever the contrast, whites and blacks.
    let mut e2 = e.clone();
    set(
        &mut e2,
        ModuleKind::VIEW_TRANSFORM,
        "contrast",
        Value::Float(2.6),
    );
    set(
        &mut e2,
        ModuleKind::VIEW_TRANSFORM,
        "white_target",
        Value::Float(1.6),
    );
    set(
        &mut e2,
        ModuleKind::VIEW_TRANSFORM,
        "black_target",
        Value::Float(-0.05),
    );
    let o = cpu(&e2, None, [0.1845; 4]);
    assert!((o[1] - 0.1845).abs() < 1e-4, "{o:?}");
}

#[test]
fn identity_ops_at_their_defaults() {
    // Away from the gamut boundary (compression starts at distance 0.8).
    let px = [0.3, 0.2, 0.12, 1.0];
    for (name, p) in [
        ("exposure_v1", Params::Exposure(Exposure::default())),
        (
            "colour_calibration_v1",
            Params::ColourCalibration(ColourCalibration::default()),
        ),
    ] {
        let u = cpu_ref::uniforms_of(name, &p);
        let o = cpu_ref::apply(name, &u, px);
        for i in 0..4 {
            assert!((o[i] - px[i]).abs() < 1e-6, "{name}: {o:?}");
        }
    }
}

#[test]
fn exposure_is_a_gain_after_the_black_offset() {
    let p = Params::Exposure(Exposure {
        exposure_ev: 1.0,
        black_offset: 0.01,
        ..Exposure::default()
    });
    let u = cpu_ref::uniforms_of("exposure_v1", &p);
    let o = cpu_ref::apply("exposure_v1", &u, [0.11, 0.51, 4.01, 0.5]);
    assert_eq!(o, [0.2, 1.0, 8.0, 0.5]);
}

#[test]
fn white_balance_makes_a_grey_card_under_the_chosen_light_neutral() {
    // A grey card lit by `white`, captured balanced for `shot`: in the working space it is
    // CAT(shot → D65) of the light's XYZ. After the op it must be neutral.
    for (white, shot) in [
        ([0.4476, 0.4074], D65_XY), // tungsten, image balanced for daylight
        ([0.3101, 0.3162], D50_XY), // shade-ish, D50 placeholder
        ([0.3805, 0.3768], [0.3457, 0.3585]),
    ] {
        let c = ColourCalibration {
            white_x: white[0] as f32,
            white_y: white[1] as f32,
            as_shot_x: shot[0] as f32,
            as_shot_y: shot[1] as f32,
            ..ColourCalibration::default()
        };
        let card = colour::mulv(
            &colour::adaptation_xyz(
                colour::Cat::Cat16,
                colour::xy_to_xyz(shot),
                colour::xy_to_xyz(D65_XY),
            ),
            colour::xy_to_xyz([f64::from(c.white_x), f64::from(c.white_y)]),
        );
        let rgb = colour::mulv(&colour::xyz_to_rec2020(), card).map(|v| v as f32 * 0.18);
        let u = cpu_ref::uniforms_of("colour_calibration_v1", &Params::ColourCalibration(c));
        let o = cpu_ref::apply("colour_calibration_v1", &u, [rgb[0], rgb[1], rgb[2], 1.0]);
        assert!(
            (o[0] - o[1]).abs() < 2e-5 && (o[2] - o[1]).abs() < 2e-5,
            "{white:?}: {o:?}"
        );
        assert!((o[1] - 0.18).abs() < 1e-3, "{o:?}");
    }
}

#[test]
fn gamut_compression_lifts_negative_channels() {
    let p = Params::ColourCalibration(ColourCalibration::default());
    let u = cpu_ref::uniforms_of("colour_calibration_v1", &p);
    let o = cpu_ref::apply("colour_calibration_v1", &u, [1.0, -0.1, 0.4, 1.0]);
    assert!(o[1] >= 0.0 && o[1] < 0.05, "{o:?}");
    assert_eq!(o[0], 1.0);
}

#[test]
fn the_sigmoid_is_monotone_bounded_and_keeps_hue() {
    let v = ViewTransform {
        inset: 0.0,
        outset: 0.0,
        ..ViewTransform::default()
    };
    let c = super::view::Curve::new(&v);
    let mut last = -1.0;
    for i in 0..400 {
        let x = (i as f64 / 20.0 - 14.0).exp2();
        let y = c.eval(x);
        assert!(y > last && y < 1.0, "{x} → {y}");
        last = y;
    }
    assert!(c.eval(1e6) > 0.999);
    assert_eq!(c.eval(0.0), 0.0);
    // Full hue preservation keeps the middle channel's place between the others.
    let p = Params::ViewTransform(v);
    let u = cpu_ref::uniforms_of("view_v1", &p);
    for px in [[2.0, 0.5, 0.1], [0.05, 0.2, 0.9], [8.0, 7.0, 0.3]] {
        let o = cpu_ref::apply("view_v1", &u, [px[0], px[1], px[2], 1.0]);
        let r_in = (px[1] - px[2]) / (px[0] - px[2]);
        let (mx, mn) = if px[0] > px[2] { (0, 2) } else { (2, 0) };
        let r_in = if mx == 0 {
            r_in
        } else {
            (px[1] - px[0]) / (px[2] - px[0])
        };
        let r_out = (o[1] - o[mn]) / (o[mx] - o[mn]);
        assert!((r_in - r_out).abs() < 1e-4, "{px:?}: {r_in} {r_out}");
    }
}

#[test]
fn tone_moves_shadows_and_highlights_only() {
    // On a flat image the guided mask is the pixel's own luminance: the point form.
    let mut e = Edit::empty(SourceKey::default());
    let reg = Registry::builtin();
    let mut te = ModuleInstance::new(reg, ModuleKind::TONE_EQUALIZER, 0);
    te.params = Params::from_fields(
        ModuleKind::TONE_EQUALIZER,
        Fields::new()
            .with("shadows_ev", Value::Float(1.0))
            .with("highlights_ev", Value::Float(-1.0)),
    );
    e.insert(reg, te);
    let g = cpu(&e, None, [0.18; 4]);
    assert!((g[0] - 0.18).abs() < 1e-5, "{g:?}");
    let d = cpu(&e, None, [0.18 / 32.0; 4]);
    assert!((d[0] / (0.18 / 32.0) - 2.0).abs() < 1e-4, "{d:?}");
    let b = cpu(&e, None, [0.18 * 16.0; 4]);
    assert!((b[0] / (0.18 * 16.0) - 0.5).abs() < 1e-4, "{b:?}");
}

#[test]
fn output_keeps_rec709_colours_and_clips_to_the_peak() {
    let o = OutputSpace::rec709();
    let p = o.params();
    let u = cpu_ref::uniforms_of("output_v1", &p);
    let to2020 = colour::rec709_to_rec2020();
    let c = colour::mulv(&to2020, [0.6, 0.3, 0.75]).map(|v| v as f32);
    let r = cpu_ref::apply("output_v1", &u, [c[0], c[1], c[2], 1.0]);
    for (a, b) in r.iter().zip([0.6, 0.3, 0.75]) {
        assert!((a - b).abs() < 1e-5, "{r:?}");
    }
    let r = cpu_ref::apply("output_v1", &u, [3.0, -0.5, 0.2, 1.0]);
    assert!(r[..3].iter().all(|v| (0.0..=1.0).contains(v)), "{r:?}");
}

#[test]
fn gpu_matches_the_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (w, h) = (97, 61);
    let img = synthetic(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let out = OutputSpace::rec709();
    let method = |m: &str| {
        let mut e = edited();
        set(
            &mut e,
            ModuleKind::VIEW_TRANSFORM,
            "method",
            Value::Text(m.into()),
        );
        e
    };
    let hdr = super::view::for_headroom(&method("filmic"), 4.0).into_owned();
    for (label, e, o) in [
        ("default", stack(), None),
        ("edited", edited(), None),
        ("edited + output", edited(), Some(&out)),
        ("filmic", method("filmic"), None),
        ("agx", method("agx"), None),
        ("filmic, headroom 4", hdr, None),
    ] {
        let g = graph(&e, o);
        let req = Request {
            precision: Precision::F32,
            ..Request::new(Roi::full(w, h))
        };
        let r = en.render(&g, &src, &req).expect("render");
        let gpu = en.read(&r).expect("read");
        en.recycle(r);
        let cpu = cpu_ref::run_image(&g, w, h, &img);
        let mut worst = 0f32;
        let mut bad = 0;
        for (a, b) in gpu.iter().zip(&cpu) {
            let err = (a - b).abs() / b.abs().max(1.0);
            worst = worst.max(err);
            if err > 1e-5 {
                bad += 1;
            }
        }
        eprintln!("{label}: worst relative error {worst:e} on {}", d.name);
        assert_eq!(bad, 0, "{label}: {bad} values off, worst {worst:e}");
    }
}

#[test]
fn a_slider_change_builds_no_kernel() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let img = synthetic(64, 32);
    let src = en.upload(64, 32, &img).expect("upload");
    let mut e = stack();
    let out = OutputSpace::rec709();
    let req = Request::new(Roi::full(64, 32));
    let r = en
        .render(&graph(&e, Some(&out)), &src, &req)
        .expect("render");
    en.recycle(r);
    let before = en.counters();
    for i in 0..10 {
        set(
            &mut e,
            ModuleKind::EXPOSURE,
            "exposure_ev",
            Value::Float(i as f32 * 0.1),
        );
        set(
            &mut e,
            ModuleKind::VIEW_TRANSFORM,
            "contrast",
            Value::Float(1.0 + i as f32 * 0.1),
        );
        // Switching the method is a uniform too.
        let m = ["sigmoid", "filmic", "agx"][i % 3];
        set(
            &mut e,
            ModuleKind::VIEW_TRANSFORM,
            "method",
            Value::Text(m.into()),
        );
        let r = en
            .render(&graph(&e, Some(&out)), &src, &req)
            .expect("render");
        en.recycle(r);
    }
    let after = en.counters();
    assert_eq!(after.pipelines, before.pipelines);
    assert_eq!(after.buffers, before.buffers);
}
