//! The colour-grading ops: neutrality and hue properties on the CPU twins, identity at rest,
//! GPU = CPU twin, LUT files = `specular-ocio`'s CPU processor, and (ignored, developer
//! run) the LUT path against PyOpenColorIO.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::cpu;
use super::lut::{self, LutSpace};
use crate::colour::oklab;
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::{Graph, NodeKind, Op, OpData, OpRegistry, Precision, Roi};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::ops::cpu_ref;
use crate::testgpu::{dev, lock};

fn module(kind: ModuleKind, f: Fields) -> ModuleInstance {
    let mut m = ModuleInstance::new(Registry::builtin(), kind, 0);
    m.params = Params::from_fields(kind, f);
    m
}

/// An edit of just these modules.
fn edit(mods: Vec<ModuleInstance>) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    for m in mods {
        e.insert(Registry::builtin(), m);
    }
    e
}

fn fields(kv: &[(&str, f32)]) -> Fields {
    let mut f = Fields::new();
    for (k, v) in kv {
        f.set(k, Value::Float(*v));
    }
    f
}

/// The op's uniforms as f32.
fn uniforms(op: &dyn Op, f: &Fields, kind: ModuleKind) -> Vec<f32> {
    let p = Params::from_fields(kind, f.clone());
    let mut w = vec![0u32; op.param_words() as usize];
    op.write_params(&p, 1.0, &mut w);
    w.iter().map(|v| f32::from_bits(*v)).collect()
}

fn lab(c: [f32; 4]) -> [f32; 3] {
    oklab::from_rec2020_f32([c[0], c[1], c[2]])
}

/// Colourful, dark, bright and out-of-gamut pixels, plus greys.
fn samples() -> Vec<[f32; 4]> {
    let mut v = Vec::new();
    for ev in [-8.0f32, -3.0, -1.0, 0.0, 1.5, 4.0] {
        let k = 0.18 * ev.exp2();
        for h in 0..12 {
            let a = h as f32 / 12.0 * std::f32::consts::TAU;
            for s in [0.05f32, 0.3, 0.8] {
                v.push([
                    k * (1.0 + s * a.cos()),
                    k * (1.0 + s * (a - 2.094).cos()),
                    k * (1.0 + s * (a + 2.094).cos()),
                    1.0,
                ]);
            }
        }
        v.push([k, k, k, 0.5]);
    }
    v.push([0.0, 0.0, 0.0, 1.0]);
    v.push([0.9, -0.05, 0.1, 1.0]);
    v
}

fn greys() -> Vec<[f32; 4]> {
    [0.0f32, 1e-4, 0.01, 0.18, 1.0, 6.0, 40.0]
        .iter()
        .map(|&k| [k, k, k, 1.0])
        .collect()
}

fn neutral(c: [f32; 4]) -> bool {
    let m = c[0].abs().max(1e-3);
    (c[0] - c[1]).abs() <= 2e-5 * m && (c[0] - c[2]).abs() <= 2e-5 * m
}

fn hue_err(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = (b[2].atan2(b[1]) - a[2].atan2(a[1])).abs();
    d.min(std::f32::consts::TAU - d)
}

#[test]
fn every_colour_op_is_left_out_at_its_defaults() {
    use super::balance::ColourBalance;
    use super::bw::BlackWhite;
    use super::equalizer::ColourEqualizer;
    use super::point::PointColour;
    let e = edit(vec![
        module(ModuleKind::COLOUR_BALANCE, ColourBalance::defaults()),
        module(ModuleKind::COLOUR_EQUALIZER, ColourEqualizer::defaults()),
        module(ModuleKind::POINT_COLOUR, PointColour::defaults()),
        module(ModuleKind::LUT_3D, lut::Lut3d::defaults()),
        // Empty fields (a module the registry made) are the identity too.
        module(ModuleKind::COLOUR_BALANCE, Fields::new()),
    ]);
    let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
    assert!(g.nodes.is_empty(), "{:?}", g.nodes);
    assert_eq!(g.identity.len(), 5);
    // B&W always changes the picture: on means a node.
    let e = edit(vec![module(
        ModuleKind::BLACK_WHITE,
        BlackWhite::defaults(),
    )]);
    let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
    assert_eq!(g.nodes.len(), 1);
    // Their kernels at zero settings (forced on) are the identity within f32 rounding.
    let zero = Fields::new();
    for (kind, op, run) in [
        (
            ModuleKind::COLOUR_BALANCE,
            &super::balance::ColourBalanceOp as &dyn Op,
            cpu::balance as fn(&[f32], [f32; 4]) -> [f32; 4],
        ),
        (
            ModuleKind::COLOUR_EQUALIZER,
            &super::equalizer::ColourEqualizerOp,
            cpu::equalizer,
        ),
    ] {
        let u = uniforms(op, &zero, kind);
        for c in samples() {
            let o = run(&u, c);
            for k in 0..4 {
                let tol = 1e-6 * c[k].abs().max(1.0) * 4.0;
                assert!((o[k] - c[k]).abs() <= tol, "{kind:?} {c:?} → {o:?}");
            }
        }
    }
}

#[test]
fn neutrals_stay_neutral_and_chroma_changes_keep_hue() {
    // Colour balance without wheel strengths: every slider, wheel luminance.
    let bal = fields(&[
        ("vibrance", 0.6),
        ("chroma", -0.3),
        ("saturation", 0.5),
        ("brilliance", -0.4),
        ("shadows_luminance", 0.5),
        ("midtones_luminance", -0.3),
        ("highlights_luminance", 0.7),
        ("global_luminance", 0.2),
        ("shadows_hue", 200.0),
    ]);
    let ub = uniforms(
        &super::balance::ColourBalanceOp,
        &bal,
        ModuleKind::COLOUR_BALANCE,
    );
    let mut eq = Fields::new();
    for (i, b) in super::BANDS.iter().enumerate() {
        eq.set(
            &format!("saturation_{b}"),
            Value::Float(0.1 * i as f32 - 0.3),
        );
        eq.set(
            &format!("brightness_{b}"),
            Value::Float(0.4 - 0.1 * i as f32),
        );
    }
    let ue = uniforms(
        &super::equalizer::ColourEqualizerOp,
        &eq,
        ModuleKind::COLOUR_EQUALIZER,
    );
    for c in samples() {
        let (b, e) = (cpu::balance(&ub, c), cpu::equalizer(&ue, c));
        if neutral(c) {
            assert!(neutral(b), "balance {c:?} → {b:?}");
            assert!(neutral(e), "equalizer {c:?} → {e:?}");
            continue;
        }
        let (l0, lb, le) = (lab(c), lab(b), lab(e));
        if l0[1].hypot(l0[2]) / l0[0].abs() > 0.02 {
            assert!(hue_err(l0, lb) < 2e-4, "balance hue {c:?} {l0:?} {lb:?}");
            assert!(hue_err(l0, le) < 2e-4, "equalizer hue {c:?} {l0:?} {le:?}");
        }
    }
    // Hue shifts, point colour and wheels with strength still leave greys grey where they
    // must: the equalizer and point colour never tint a neutral.
    let mut eqh = Fields::new();
    for b in super::BANDS {
        eqh.set(&format!("hue_{b}"), Value::Float(30.0));
    }
    let uh = uniforms(
        &super::equalizer::ColourEqualizerOp,
        &eqh,
        ModuleKind::COLOUR_EQUALIZER,
    );
    let pc = fields(&[
        ("target_hue", 30.0),
        ("target_saturation", 0.3),
        ("target_lightness", 0.6),
        ("shift_hue", 40.0),
        ("shift_saturation", 0.8),
        ("shift_lightness", 0.5),
        ("range", 1.0),
    ])
    .with("picked", Value::Bool(true));
    let up = uniforms(&super::point::PointColourOp, &pc, ModuleKind::POINT_COLOUR);
    let mut bw = Fields::new();
    for (i, b) in super::BANDS.iter().enumerate() {
        bw.set(
            &format!("mix_{b}"),
            Value::Float(if i % 2 == 0 { 0.8 } else { -0.6 }),
        );
    }
    let uw = uniforms(&super::bw::BlackWhiteOp, &bw, ModuleKind::BLACK_WHITE);
    for g in greys() {
        assert!(neutral(cpu::equalizer(&uh, g)));
        let p = cpu::point(&up, g);
        assert!(neutral(p), "{g:?} {p:?}");
        let w = cpu::black_white(&uw, g);
        assert!((w[0] - g[0]).abs() <= 1e-6 * g[0].max(1.0), "{g:?} {w:?}");
    }
}

#[test]
fn the_balance_ranges_act_where_they_should() {
    // A highlights tint moves bright pixels, not dark ones; the global wheel moves both.
    let u = |f: Fields| {
        uniforms(
            &super::balance::ColourBalanceOp,
            &f,
            ModuleKind::COLOUR_BALANCE,
        )
    };
    let hi = u(fields(&[
        ("highlights_hue", 220.0),
        ("highlights_chroma", 0.8),
    ]));
    let dark = [0.005, 0.005, 0.005, 1.0];
    let bright = [3.0, 3.0, 3.0, 1.0];
    let ab = |c: [f32; 4]| {
        let l = lab(c);
        l[1].hypot(l[2]) / l[0]
    };
    assert!(ab(cpu::balance(&hi, dark)) < 0.01);
    assert!(ab(cpu::balance(&hi, bright)) > 0.08);
    // The tint points to the wheel's hue: 220° on the wheel is a blue.
    let t = lab(cpu::balance(&hi, bright));
    let want = super::hsv_direction(220.0);
    let cos = (t[1] * want[0] as f32 + t[2] * want[1] as f32) / t[1].hypot(t[2]);
    assert!(cos > 0.999, "{cos}");
    // A tint is proportional to L: exposure does not change the tinted saturation.
    let g = u(fields(&[("global_hue", 40.0), ("global_chroma", 0.5)]));
    let s1 = ab(cpu::balance(&g, [0.05, 0.05, 0.05, 1.0]));
    let s2 = ab(cpu::balance(&g, [5.0, 5.0, 5.0, 1.0]));
    assert!((s1 - s2).abs() < 1e-4 && s1 > 0.05, "{s1} {s2}");
    // Vibrance lifts dull colours more than vivid ones.
    let v = u(fields(&[("vibrance", 0.8)]));
    let gain = |c: [f32; 4]| ab(cpu::balance(&v, c)) / ab(c);
    assert!(gain([0.2, 0.18, 0.17, 1.0]) > gain([0.5, 0.05, 0.02, 1.0]) + 0.3);
}

#[test]
fn equalizer_bands_are_local_and_point_colour_follows_its_range() {
    let mut f = Fields::new();
    f.set("saturation_blue", Value::Float(-1.0));
    f.set("hue_red", Value::Float(20.0));
    let u = uniforms(
        &super::equalizer::ColourEqualizerOp,
        &f,
        ModuleKind::COLOUR_EQUALIZER,
    );
    let at = |hsv: f64| {
        let d = super::hsv_direction(hsv);
        let lab = [0.6f32, 0.15 * d[0] as f32, 0.15 * d[1] as f32];
        let c = oklab::to_rec2020_f32(lab);
        [c[0], c[1], c[2], 1.0]
    };
    let blue = lab(cpu::equalizer(&u, at(240.0)));
    assert!(blue[1].hypot(blue[2]) < 0.01, "{blue:?}");
    let green = lab(cpu::equalizer(&u, at(120.0)));
    assert!((green[1].hypot(green[2]) - 0.15).abs() < 0.01);
    let red_in = lab(at(0.0));
    let red = lab(cpu::equalizer(&u, at(0.0)));
    let turned = hue_err(red_in, red).to_degrees();
    assert!((turned - 20.0).abs() < 4.0, "{turned}");
    // Point colour: the target moves fully, a far colour not at all.
    let target = at(30.0);
    let t = super::point::target_of(target.map(f64::from)[..3].try_into().unwrap());
    let pf = fields(&[
        ("target_hue", t[0]),
        ("target_saturation", t[1]),
        ("target_lightness", t[2]),
        ("shift_hue", 30.0),
    ])
    .with("picked", Value::Bool(true));
    let up = uniforms(&super::point::PointColourOp, &pf, ModuleKind::POINT_COLOUR);
    let moved = hue_err(lab(target), lab(cpu::point(&up, target))).to_degrees();
    assert!((moved - 30.0).abs() < 0.01, "{moved}");
    let far = at(200.0);
    let o = cpu::point(&up, far);
    assert!(
        (0..3).all(|k| (o[k] - far[k]).abs() < 1e-5),
        "{far:?} {o:?}"
    );
    // The overlay keeps the target's colour and greys the far colour.
    let ov = uniforms(
        &super::point::PointColourOp,
        &pf.clone().with("show_range", Value::Bool(true)),
        ModuleKind::POINT_COLOUR,
    );
    assert!(neutral(cpu::point(&ov, far)));
    let o = cpu::point(&ov, target);
    assert!((0..3).all(|k| (o[k] - target[k]).abs() < 1e-6), "{o:?}");
}

/// A smooth non-linear function of 0–1 RGB (the fixture tables' content).
fn look(c: [f64; 3]) -> [f64; 3] {
    [
        0.9 * c[0].max(0.0).powf(0.8) + 0.05 * c[1],
        0.5 * (c[1] + c[2] * c[2]) + 0.1 * c[0],
        0.7 * c[2].max(0.0).sqrt() + 0.2 * c[0] * c[1],
    ]
}

/// Fixture LUT files in a fresh folder: name, interpolation, exact on the GPU.
fn fixtures() -> (PathBuf, Vec<(&'static str, bool, bool)>) {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("dv8-luts-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let grid = |n: usize, f: &dyn Fn(usize, usize, usize) -> String| {
        let mut s = String::new();
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    s += &f(r, g, b);
                }
            }
        }
        s
    };
    let t = |i: usize, n: usize| i as f64 / (n - 1) as f64;
    let fmt = |v: [f64; 3]| format!("{:.7} {:.7} {:.7}\n", v[0], v[1], v[2]);
    // a.cube: 17³.
    let n = 17;
    let a = grid(n, &|r, g, b| fmt(look([t(r, n), t(g, n), t(b, n)])));
    std::fs::write(
        dir.join("a.cube"),
        format!("TITLE \"a\"\nLUT_3D_SIZE {n}\n{a}"),
    )
    .unwrap();
    // b.cube: a square-root 1D shaper over 0–4, then a 9³ table.
    let mut b = String::from("LUT_1D_SIZE 64\nLUT_1D_INPUT_RANGE 0 4\nLUT_3D_SIZE 9\n");
    for i in 0..64 {
        let x = (t(i, 64) * 4.0).sqrt() / 2.0;
        b += &fmt([x, x, x]);
    }
    b += &grid(9, &|r, g, bb| fmt(look([t(r, 9), t(g, 9), t(bb, 9)])));
    std::fs::write(dir.join("b.cube"), b).unwrap();
    // c.3dl: 17-point 10-bit mesh, 12-bit codes, blue fastest.
    let mut c: String = (0..n)
        .map(|i| (i * 1023 / (n - 1)).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    c.push('\n');
    for r in 0..n {
        for g in 0..n {
            for bb in 0..n {
                let v = look([t(r, n), t(g, n), t(bb, n)])
                    .map(|v| (v.clamp(0.0, 1.0) * 4095.0).round() as i64);
                c += &format!("{} {} {}\n", v[0], v[1], v[2]);
            }
        }
    }
    std::fs::write(dir.join("c.3dl"), c).unwrap();
    // d.spi3d: 9³.
    let mut d = String::from("SPILUT 1.0\n3 3\n9 9 9\n");
    for r in 0..9 {
        for g in 0..9 {
            for bb in 0..9 {
                let v = look([t(r, 9), t(g, 9), t(bb, 9)]);
                d += &format!("{r} {g} {bb} {:.7} {:.7} {:.7}\n", v[0], v[1], v[2]);
            }
        }
    }
    std::fs::write(dir.join("d.spi3d"), d).unwrap();
    // e.clf: matrix with offset, 1D table, range, tetrahedral 3D table.
    let mut lut3 = String::new();
    for r in 0..9 {
        for g in 0..9 {
            for bb in 0..9 {
                let v = look([t(r, 9), t(g, 9), t(bb, 9)]);
                lut3 += &format!("{:.7} {:.7} {:.7}\n", v[0], v[1], v[2]);
            }
        }
    }
    let mut lut1 = String::new();
    for i in 0..32 {
        let x = t(i, 32);
        lut1 += &format!("{:.7} {:.7} {:.7}\n", x * x, x.powf(0.7), x);
    }
    let body = |extra: &str| {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <ProcessList id=\"dv8\" compCLFversion=\"3.0\">\n{extra}\
             <Matrix inBitDepth=\"32f\" outBitDepth=\"32f\"><Array dim=\"3 4\">\
             0.9 0.08 0.02 0.01 0.05 0.9 0.05 0.0 0.02 0.1 0.88 -0.01</Array></Matrix>\n\
             <LUT1D inBitDepth=\"32f\" outBitDepth=\"32f\"><Array dim=\"32 3\">\n{lut1}</Array></LUT1D>\n\
             <Range inBitDepth=\"32f\" outBitDepth=\"32f\"><minInValue>0</minInValue>\
             <maxInValue>1</maxInValue><minOutValue>0</minOutValue><maxOutValue>1</maxOutValue></Range>\n\
             <LUT3D inBitDepth=\"32f\" outBitDepth=\"32f\" interpolation=\"tetrahedral\">\
             <Array dim=\"9 9 9 3\">\n{lut3}</Array></LUT3D>\n</ProcessList>\n"
        )
    };
    std::fs::write(dir.join("e.clf"), body("")).unwrap();
    // f.clf: an exponent and a moncurve first.
    std::fs::write(
        dir.join("f.clf"),
        body(
            "<Exponent inBitDepth=\"32f\" outBitDepth=\"32f\" style=\"basicMirrorFwd\">\
             <ExponentParams exponent=\"1.8\"/></Exponent>\n\
             <Exponent inBitDepth=\"32f\" outBitDepth=\"32f\" style=\"moncurveRev\">\
             <ExponentParams exponent=\"2.4\" offset=\"0.055\"/></Exponent>\n",
        ),
    )
    .unwrap();
    // g.clf: a camera log and an ASC CDL first.
    std::fs::write(
        dir.join("g.clf"),
        body(
            "<Log inBitDepth=\"32f\" outBitDepth=\"32f\" style=\"cameraLinToLog\">\
             <LogParams base=\"2\" linSideSlope=\"1\" linSideOffset=\"0.01\" \
             logSideSlope=\"0.08\" logSideOffset=\"0.7\" linSideBreak=\"0.005\"/></Log>\n\
             <ASC_CDL inBitDepth=\"32f\" outBitDepth=\"32f\" style=\"Fwd\"><SOPNode>\
             <Slope>1.1 0.95 1.0</Slope><Offset>0.01 -0.02 0.0</Offset>\
             <Power>1.2 1.0 0.9</Power></SOPNode><SatNode><Saturation>0.8</Saturation>\
             </SatNode></ASC_CDL>\n",
        ),
    )
    .unwrap();
    // h.ctf: an inverted 1D table (baked on the GPU).
    std::fs::write(
        dir.join("h.ctf"),
        body(&format!(
            "<InverseLUT1D inBitDepth=\"32f\" outBitDepth=\"32f\">\
             <Array dim=\"32 3\">\n{lut1}</Array></InverseLUT1D>\n"
        ))
        .replace("compCLFversion=\"3.0\"", "version=\"1.7\""),
    )
    .unwrap();
    let files = vec![
        ("a.cube", true, true),
        ("a.cube", false, true),
        ("b.cube", true, true),
        ("c.3dl", true, true),
        ("d.spi3d", false, true),
        ("e.clf", true, true),
        ("f.clf", true, true),
        ("g.clf", true, true),
        ("h.ctf", true, false),
    ];
    (dir, files)
}

/// Test inputs for a table over 0–1 (and a little outside it).
fn lut_inputs(n: usize) -> Vec<[f32; 4]> {
    let mut v = Vec::with_capacity(n);
    let mut s = 0x9e37_79b9u32;
    let mut r = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        (s as f32 / u32::MAX as f32) * 1.1 - 0.05
    };
    for _ in 0..n {
        v.push([r(), r(), r(), 1.0]);
    }
    v
}

/// What `specular-ocio`'s CPU processor (f64) gives for the module on linear Rec.2020.
fn ocio_reference(path: &Path, tet: bool, space: LutSpace, strength: f64, c: [f64; 3]) -> [f64; 3] {
    let ops = specular_ocio::files::read(path, if tet { "tetrahedral" } else { "linear" }).unwrap();
    let p = specular_ocio::Processor { ops };
    let to = space.to_primaries();
    let from = crate::colour::inv(&to).unwrap();
    let m = crate::colour::mulv(&to, c);
    let enc = |x: f64| match space {
        LutSpace::Acescct if x <= 0.0078125 => 10.540_237_741_654_5 * x + 0.072_905_534_195_835_5,
        LutSpace::Acescct => (x.log2() + 9.72) / 17.52,
        LutSpace::Srgb if x <= 0.003_130_8 => 12.92 * x,
        LutSpace::Srgb => 1.055 * x.powf(1.0 / 2.4) - 0.055,
        LutSpace::Rec709 => x.signum() * x.abs().powf(1.0 / 2.4),
        _ => x,
    };
    let dec = |y: f64| match space {
        LutSpace::Acescct if y <= 0.155_251_141_552_511 => {
            (y - 0.072_905_534_195_835_5) / 10.540_237_741_654_5
        }
        LutSpace::Acescct => (y * 17.52 - 9.72).exp2().min(65504.0),
        LutSpace::Srgb if y <= 0.04045 => y / 12.92,
        LutSpace::Srgb => ((y + 0.055) / 1.055).powf(2.4),
        LutSpace::Rec709 => y.signum() * y.abs().powf(2.4),
        _ => y,
    };
    let o = crate::colour::mulv(&from, p.apply(m.map(enc)).map(dec));
    std::array::from_fn(|k| c[k] + (o[k] - c[k]) * strength)
}

fn lut_module(path: &Path, tet: bool, space: LutSpace, strength: f32) -> ModuleInstance {
    module(
        ModuleKind::LUT_3D,
        Fields::new()
            .with("path", Value::Text(path.to_string_lossy().into_owned()))
            .with("space", space.into())
            .with(
                "interpolation",
                Value::Text(if tet { "tetrahedral" } else { "trilinear" }.into()),
            )
            .with("strength", Value::Float(strength)),
    )
}

fn render(en: &mut Engine, g: &Graph, w: u32, h: u32, img: &[f32]) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let req = Request {
        precision: Precision::F32,
        ..Request::new(Roi::full(w, h))
    };
    let r = en.render(g, &src, &req).expect("render");
    let out = en.read(&r).expect("read");
    en.recycle(r);
    out
}

#[test]
fn lut_files_on_the_gpu_match_the_ocio_cpu_processor() {
    let (dir, files) = fixtures();
    let inputs = lut_inputs(4096);
    let flat: Vec<f32> = inputs.iter().flatten().copied().collect();
    let gpu = dev();
    let _l = lock();
    let mut en = gpu.map(|d| Engine::new(&d.device, &d.queue, Budget::fixed(1 << 28)));
    for (name, tet, exact) in files {
        let path = dir.join(name);
        let loaded = lut::load(&path.to_string_lossy(), tet).expect(name);
        assert_eq!(loaded.exact, exact, "{name}");
        let e = edit(vec![lut_module(&path, tet, LutSpace::LinearRec2020, 1.0)]);
        let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
        assert_eq!(g.nodes.len(), 1);
        let twin = cpu_ref::run(&g, &flat);
        let mut worst = (0f64, 0f64);
        for (i, c) in inputs.iter().enumerate() {
            let want = ocio_reference(
                &path,
                tet,
                LutSpace::LinearRec2020,
                1.0,
                c.map(f64::from)[..3].try_into().unwrap(),
            );
            for k in 0..3 {
                worst.0 = worst.0.max((f64::from(twin[i * 4 + k]) - want[k]).abs());
            }
        }
        if let (Some(en), Some(d)) = (en.as_mut(), gpu) {
            let out = render(en, &g, 64, 64, &flat);
            for (a, b) in out.iter().zip(&twin) {
                worst.1 = worst.1.max(f64::from((a - b).abs()));
            }
            eprintln!(
                "{name} ({}): CPU twin vs OCIO CPU {:.2e}, GPU vs twin {:.2e} on {}",
                if tet { "tetrahedral" } else { "trilinear" },
                worst.0,
                worst.1,
                d.name
            );
        }
        // Exact files: f32 table vs f64 processor. The bake: interpolation error.
        let tol = if exact { 1e-5 } else { 1e-2 };
        assert!(worst.0 <= tol, "{name}: twin vs OCIO {}", worst.0);
        assert!(worst.1 <= 1e-5, "{name}: GPU vs twin {}", worst.1);
    }
    // In every declared space, with half strength: GPU vs the f64 composition ≤ 1e-4
    // (relative above 1).
    let path = dir.join("a.cube");
    let mut scene = Vec::new();
    for c in lut_inputs(1024) {
        // Scene-linear: some far above 1, some near black.
        let k = (c[0] * 8.0 - 5.0).exp2();
        scene.push([
            c[1].abs() * k,
            c[2].abs() * k,
            (c[1] + c[2]).abs() * 0.5 * k,
            1.0,
        ]);
    }
    let flat: Vec<f32> = scene.iter().flatten().copied().collect();
    for space in [
        LutSpace::LinearRec2020,
        LutSpace::Acescg,
        LutSpace::Acescct,
        LutSpace::Srgb,
        LutSpace::Rec709,
    ] {
        let e = edit(vec![lut_module(&path, true, space, 0.5)]);
        let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
        let out = match (en.as_mut(), gpu) {
            (Some(en), Some(_)) => render(en, &g, 32, 32, &flat),
            _ => cpu_ref::run(&g, &flat),
        };
        let mut worst = 0f64;
        for (i, c) in scene.iter().enumerate() {
            let want = ocio_reference(
                &path,
                true,
                space,
                0.5,
                c.map(f64::from)[..3].try_into().unwrap(),
            );
            for k in 0..3 {
                let err = (f64::from(out[i * 4 + k]) - want[k]).abs() / want[k].abs().max(1.0);
                worst = worst.max(err);
            }
        }
        eprintln!("a.cube in {space:?} at half strength: worst {worst:.2e}");
        assert!(worst <= 1e-4, "{space:?}: {worst}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// An op that delegates to another but never counts as the identity (to run kernels at
/// zero settings on the GPU).
struct Forced(Arc<dyn Op>);

impl Op for Forced {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn kind(&self) -> NodeKind {
        self.0.kind()
    }
    fn param_words(&self) -> u32 {
        self.0.param_words()
    }
    fn write_params(&self, p: &Params, s: f32, out: &mut [u32]) {
        self.0.write_params(p, s, out)
    }
    fn wgsl(&self) -> &'static str {
        self.0.wgsl()
    }
    fn helpers(&self) -> &'static [&'static str] {
        self.0.helpers()
    }
    fn data(&self, p: &Params) -> Option<Arc<OpData>> {
        self.0.data(p)
    }
}

fn graded() -> Edit {
    let mut eq = fields(&[
        ("hue_red", 15.0),
        ("saturation_blue", 0.5),
        ("brightness_green", -0.4),
        ("hue_aqua", -20.0),
    ]);
    eq.set("saturation_orange", Value::Float(-0.3));
    edit(vec![
        module(
            ModuleKind::COLOUR_BALANCE,
            fields(&[
                ("vibrance", 0.4),
                ("saturation", -0.2),
                ("chroma", 0.1),
                ("brilliance", 0.3),
                ("shadows_hue", 210.0),
                ("shadows_chroma", 0.4),
                ("shadows_luminance", 0.2),
                ("highlights_hue", 40.0),
                ("highlights_chroma", 0.3),
                ("midtones_luminance", -0.1),
                ("global_hue", 120.0),
                ("global_chroma", 0.1),
            ]),
        ),
        module(ModuleKind::COLOUR_EQUALIZER, eq),
        module(
            ModuleKind::POINT_COLOUR,
            fields(&[
                ("target_hue", 60.0),
                ("target_saturation", 0.3),
                ("target_lightness", 0.6),
                ("shift_hue", -25.0),
                ("shift_saturation", 0.5),
                ("shift_lightness", 0.3),
                ("range", 0.8),
            ])
            .with("picked", Value::Bool(true)),
        ),
    ])
}

#[test]
fn gpu_matches_the_cpu_twins_and_sliders_build_nothing() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let px = samples();
    let flat: Vec<f32> = px.iter().flatten().copied().collect();
    let (w, h) = (px.len() as u32, 1);
    let mut bw = Fields::new();
    for (i, b) in super::BANDS.iter().enumerate() {
        bw.set(&format!("mix_{b}"), Value::Float(0.25 * i as f32 - 0.9));
    }
    let mut forced = OpRegistry::builtin().clone();
    for k in [ModuleKind::COLOUR_BALANCE, ModuleKind::COLOUR_EQUALIZER] {
        let op = OpRegistry::builtin().get(k, 1).unwrap().clone();
        forced.register(k, 1, Arc::new(Forced(op)));
    }
    let zero = edit(vec![
        module(ModuleKind::COLOUR_BALANCE, Fields::new()),
        module(ModuleKind::COLOUR_EQUALIZER, Fields::new()),
    ]);
    let cases = [
        ("graded", graded(), OpRegistry::builtin().clone()),
        (
            "balance",
            edit(vec![graded().modules[0].clone()]),
            OpRegistry::builtin().clone(),
        ),
        (
            "equalizer",
            edit(vec![graded().modules[1].clone()]),
            OpRegistry::builtin().clone(),
        ),
        (
            "point",
            edit(vec![graded().modules[2].clone()]),
            OpRegistry::builtin().clone(),
        ),
        (
            "black & white",
            edit(vec![module(ModuleKind::BLACK_WHITE, bw)]),
            OpRegistry::builtin().clone(),
        ),
        ("zero settings", zero, forced),
    ];
    for (label, e, ops) in cases {
        let g = Graph::build(&e, &ops, None).unwrap();
        assert_eq!(g.groups.len(), 1, "{label}: one fused kernel");
        let gpu = render(&mut en, &g, w, h, &flat);
        let cpu = cpu_ref::run(&g, &flat);
        let mut worst = 0f32;
        for (a, b) in gpu.iter().zip(&cpu) {
            worst = worst.max((a - b).abs() / b.abs().max(1.0));
        }
        eprintln!(
            "{label}: GPU vs CPU twin worst relative {worst:e} on {}",
            d.name
        );
        assert!(worst <= 1e-5, "{label}: {worst}");
        if label == "zero settings" {
            let mut id = 0f32;
            for (a, b) in gpu.iter().zip(&flat) {
                id = id.max((a - b).abs() / b.abs().max(1.0));
            }
            eprintln!("zero settings on the GPU vs input: worst relative {id:e}");
            assert!(id <= 1e-6 * 4.0, "{id}");
        }
    }
    // Moving any colour slider (and the LUT strength) makes no kernel and no buffer.
    let (dir, _) = fixtures();
    let lpath = dir.join("a.cube");
    let mut e = graded();
    e.insert(
        Registry::builtin(),
        lut_module(&lpath, true, LutSpace::Acescct, 1.0),
    );
    let focus = Some((ModuleKind::COLOUR_BALANCE, 0));
    let g = Graph::build(&e, OpRegistry::builtin(), focus).unwrap();
    let _ = render(&mut en, &g, w, h, &flat);
    let src = en.upload(w, h, &flat).expect("upload");
    let req = Request::new(Roi::full(w, h));
    let r = en.render(&g, &src, &req).unwrap();
    en.recycle(r);
    let before = en.counters();
    for i in 0..12 {
        let v = i as f32 * 0.05;
        for (kind, name) in [
            (ModuleKind::COLOUR_BALANCE, "vibrance"),
            (ModuleKind::COLOUR_EQUALIZER, "saturation_blue"),
            (ModuleKind::POINT_COLOUR, "shift_lightness"),
            (ModuleKind::LUT_3D, "strength"),
        ] {
            let m = e.module_mut(kind, 0).unwrap();
            let mut f = m.params.to_fields();
            f.set(name, Value::Float(0.2 + v));
            m.params.set_fields(f);
        }
        let g = Graph::build(
            &e,
            OpRegistry::builtin(),
            Some((ModuleKind::COLOUR_BALANCE, 0)),
        )
        .unwrap();
        let r = en.render(&g, &src, &req).unwrap();
        en.recycle(r);
    }
    let after = en.counters();
    assert_eq!(after.pipelines, before.pipelines);
    assert_eq!(after.buffers, before.buffers);
    std::fs::remove_dir_all(&dir).ok();
}

/// PyOpenColorIO on the fixture files vs our GPU path (developer run):
///
///     SPECULAR_OCIO_PY=<venv>/bin/python SPECULAR_REQUIRE_GPU=1 \
///       cargo test -p specular-develop --lib lut_matches_pyopencolorio -- --ignored --nocapture
///
/// The venv needs `opencolorio` (`pip install opencolorio`).
#[test]
#[ignore = "needs PyOpenColorIO: SPECULAR_OCIO_PY=<venv>/bin/python"]
fn lut_matches_pyopencolorio() {
    let Some(py) = std::env::var_os("SPECULAR_OCIO_PY") else {
        eprintln!("SPECULAR_OCIO_PY not set: skipped");
        return;
    };
    let Some(d) = dev() else { return };
    let _l = lock();
    let (dir, files) = fixtures();
    let inputs = lut_inputs(4096);
    let flat: Vec<f32> = inputs.iter().flatten().copied().collect();
    let input_file = dir.join("inputs.txt");
    let text: String = inputs
        .iter()
        .map(|c| format!("{:e} {:e} {:e}\n", c[0], c[1], c[2]))
        .collect();
    std::fs::write(&input_file, text).unwrap();
    const SCRIPT: &str = r#"
import sys
import PyOpenColorIO as ocio
path, interp, inputs = sys.argv[1], sys.argv[2], sys.argv[3]
cfg = ocio.Config.CreateRaw()
t = ocio.FileTransform(src=path, interpolation=ocio.INTERP_TETRAHEDRAL if interp == "tetrahedral" else ocio.INTERP_LINEAR)
p = cfg.getProcessor(t).getDefaultCPUProcessor()
for l in open(inputs):
    r = p.applyRGB([float(v) for v in l.split()])
    print("%.9g %.9g %.9g" % tuple(r))
"#;
    let mut en = Engine::new(&d.device, &d.queue, Budget::fixed(1 << 28));
    for (name, tet, exact) in files {
        let path = dir.join(name);
        let out = std::process::Command::new(&py)
            .arg("-I")
            .arg("-c")
            .arg(SCRIPT)
            .arg(&path)
            .arg(if tet { "tetrahedral" } else { "linear" })
            .arg(&input_file)
            .output()
            .expect("python runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let want: Vec<f64> = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(want.len(), inputs.len() * 3);
        let e = edit(vec![lut_module(&path, tet, LutSpace::LinearRec2020, 1.0)]);
        let g = Graph::build(&e, OpRegistry::builtin(), None).unwrap();
        let gpu = render(&mut en, &g, 64, 64, &flat);
        let mut worst = 0f64;
        let mut over = 0;
        for i in 0..inputs.len() {
            for k in 0..3 {
                let err = (f64::from(gpu[i * 4 + k]) - want[i * 3 + k]).abs();
                worst = worst.max(err);
                if err > 1e-4 {
                    over += 1;
                }
            }
        }
        eprintln!(
            "{name} ({}): GPU vs PyOpenColorIO {} worst {worst:.2e}, {over} of {} values over 1e-4",
            if tet { "tetrahedral" } else { "trilinear" },
            ocio_version(&py),
            inputs.len() * 3
        );
        if exact {
            assert_eq!(over, 0, "{name}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

fn ocio_version(py: &std::ffi::OsStr) -> String {
    std::process::Command::new(py)
        .args([
            "-I",
            "-c",
            "import PyOpenColorIO as o; print(o.__version__)",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}
