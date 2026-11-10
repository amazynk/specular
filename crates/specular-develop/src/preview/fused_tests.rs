//! Runs of point groups as one kernel, bit for bit as group by group.

use super::*;
use crate::model::{ModuleInstance, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

/// Linear Rec.2020: gradients, fine texture, a bright patch and a little noise.
fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let tex = 0.02 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
            let n = ((x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) % 1000) as f32 * 2e-5;
            let hi = if (0.6..0.7).contains(&fx) && fy < 0.2 {
                1.5
            } else {
                0.0
            };
            v.extend_from_slice(&[
                0.03 + 0.5 * fx + tex + hi + n,
                0.04 + 0.4 * fy + tex + hi,
                0.05 + 0.3 * (1.0 - fx) + tex + hi - n,
                1.0,
            ]);
        }
    }
    v
}

fn with(e: &mut Edit, kind: ModuleKind, name: &str, v: Value) {
    if e.module_mut(kind, 0).is_none() {
        let reg = Registry::builtin();
        let m = ModuleInstance::new(reg, kind, 0);
        e.insert(reg, m);
    }
    let m = e.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

/// Exposure and white balance, then the tone equalizer (statistics: its own group), local
/// contrast, colour balance and curves.
fn edit() -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    with(
        &mut e,
        ModuleKind::EXPOSURE,
        "exposure_ev",
        Value::Float(0.3),
    );
    with(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_source",
        Value::Text("custom".into()),
    );
    with(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_x",
        Value::Float(0.33),
    );
    with(
        &mut e,
        ModuleKind::TONE_EQUALIZER,
        "shadows_ev",
        Value::Float(0.4),
    );
    with(
        &mut e,
        ModuleKind::LOCAL_CONTRAST,
        "texture",
        Value::Float(0.3),
    );
    with(
        &mut e,
        ModuleKind::LOCAL_CONTRAST,
        "clarity",
        Value::Float(0.2),
    );
    with(
        &mut e,
        ModuleKind::COLOUR_BALANCE,
        "vibrance",
        Value::Float(0.2),
    );
    with(
        &mut e,
        ModuleKind::CURVES,
        "points",
        Value::Floats(vec![0.0, 0.0, 0.5, 0.6, 1.0, 1.0]),
    );
    e
}

fn developer(d: &crate::testgpu::Dev, w: u32, h: u32, runs: bool) -> Developer {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.engine.point_runs = runs;
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: scene(w, h),
    })
    .unwrap();
    assert_eq!(dv.prepare(&edit(), w).unwrap(), (w, h));
    dv
}

fn render(dv: &mut Developer, e: &Edit) -> (Vec<u32>, crate::gpu::RenderStats) {
    let r = dv.render(e, None, &OutputSpace::rec709()).unwrap();
    let px = dv.read(&r).unwrap().iter().map(|v| v.to_bits()).collect();
    let st = r.stats.clone();
    dv.recycle(r);
    (px, st)
}

/// Exposure and white balance run in the tone equalizer's kernel (f16 storage rounded in
/// between, as stored): the same pictures bit for bit, fewer dispatches, every node run;
/// later sliders (tone equalizer, colour balance) resume as before.
#[test]
fn point_runs_match_group_by_group() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (640u32, 424u32);
    let mut runs = developer(d, w, h, true);
    let mut plain = developer(d, w, h, false);
    let mut e = edit();
    for (i, (kind, field, v)) in [
        (ModuleKind::EXPOSURE, "exposure_ev", 0.3f32),
        (ModuleKind::EXPOSURE, "exposure_ev", 0.8),
        (ModuleKind::COLOUR_CALIBRATION, "white_x", 0.31),
        (ModuleKind::TONE_EQUALIZER, "shadows_ev", 0.6),
        (ModuleKind::COLOUR_BALANCE, "vibrance", 0.4),
        (ModuleKind::EXPOSURE, "exposure_ev", -0.4),
    ]
    .into_iter()
    .enumerate()
    {
        with(&mut e, kind, field, Value::Float(v));
        let (want, ps) = render(&mut plain, &e);
        let (got, rs) = render(&mut runs, &e);
        assert_eq!(got, want, "change {i}: the same picture");
        // A tone equalizer change resumes before exposure (whose output the run leaves
        // out): no more dispatches.
        assert!(
            rs.dispatches <= ps.dispatches,
            "change {i}: no more dispatches"
        );
        if kind != ModuleKind::TONE_EQUALIZER {
            assert_eq!(rs.nodes_run, ps.nodes_run, "change {i}: the same nodes run");
        }
        if i == 0 || kind == ModuleKind::EXPOSURE {
            assert!(
                rs.dispatches < ps.dispatches,
                "change {i}: {} dispatches, {} group by group",
                rs.dispatches,
                ps.dispatches
            );
        }
        if kind == ModuleKind::COLOUR_BALANCE {
            assert!(rs.cache_hits > 0 && ps.cache_hits > 0, "resumed");
            // Resumed after local contrast: a run never starts at it.
            assert!(
                !rs.nodes_run
                    .iter()
                    .any(|n| n.0 == ModuleKind::LOCAL_CONTRAST)
            );
        }
    }
    // Tiles and a smaller view: the same.
    for runs_on in [true, false] {
        let dv = if runs_on { &mut runs } else { &mut plain };
        dv.tile_side = Some(160);
    }
    with(
        &mut e,
        ModuleKind::EXPOSURE,
        "exposure_ev",
        Value::Float(0.1),
    );
    assert_eq!(render(&mut runs, &e).0, render(&mut plain, &e).0, "tiles");
}
