//! Refined pictures after a change leave sharpening out; the whole one follows.

use super::*;
use crate::model::{ModuleInstance, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

/// Linear Rec.2020: gradients, fine texture and a bright patch.
fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let tex = 0.02 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
            let hi = if (0.6..0.7).contains(&fx) && fy < 0.2 {
                1.5
            } else {
                0.0
            };
            v.extend_from_slice(&[
                0.03 + 0.5 * fx + tex + hi,
                0.04 + 0.4 * fy + tex + hi,
                0.05 + 0.3 * (1.0 - fx) + tex + hi,
                1.0,
            ]);
        }
    }
    v
}

fn with(e: &mut Edit, kind: ModuleKind, name: &str, v: f32) {
    if e.module_mut(kind, 0).is_none() {
        let reg = Registry::builtin();
        let m = ModuleInstance::new(reg, kind, 0);
        e.insert(reg, m);
    }
    let m = e.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, Value::Float(v));
    m.params.set_fields(f);
}

/// Exposure, the tone equalizer and local contrast, colour balance and sharpening.
fn edit() -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    with(&mut e, ModuleKind::EXPOSURE, "exposure_ev", 0.3);
    with(&mut e, ModuleKind::TONE_EQUALIZER, "shadows_ev", 0.4);
    with(&mut e, ModuleKind::LOCAL_CONTRAST, "clarity", 0.2);
    with(&mut e, ModuleKind::COLOUR_BALANCE, "vibrance", 0.2);
    with(&mut e, ModuleKind::SHARPEN, "amount", 0.6);
    e
}

fn developer(d: &crate::testgpu::Dev, w: u32, h: u32, defer: bool) -> Developer {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.defer_sharpen = defer;
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: scene(w, h),
    })
    .unwrap();
    assert_eq!(dv.prepare(&edit(), w).unwrap(), (w, h));
    dv
}

/// The picture's values (bits) and the render's stats.
fn render(dv: &mut Developer, e: &Edit) -> (Vec<u32>, crate::gpu::RenderStats) {
    let r = dv.render(e, None, &OutputSpace::rec709()).unwrap();
    let px = dv.read(&r).unwrap().iter().map(|v| v.to_bits()).collect();
    let st = r.stats.clone();
    dv.recycle(r);
    (px, st)
}

/// After a change before sharpening, the refined picture leaves it out and asks to refine
/// (`refine_due`); the next render is the whole picture, bit for bit as without deferring,
/// resumed at sharpening. A change at sharpening or later, a kept result (undo) and a
/// developer that does not defer render the whole picture at once.
#[test]
fn refined_pictures_leave_sharpening_out_then_finish_exactly() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (1200u32, 800u32);
    let mut dv = developer(d, w, h, true);
    let mut plain = developer(d, w, h, false);
    let mut e = edit();
    let mut first = None;
    for ev in [0.3f32, 0.6, -0.2] {
        with(&mut e, ModuleKind::EXPOSURE, "exposure_ev", ev);
        let want = render(&mut plain, &e).0;
        let mut without = e.clone();
        without
            .module_mut(ModuleKind::SHARPEN, 0)
            .expect("sharpen")
            .enabled = false;
        let unsharpened = render(&mut plain, &without).0;
        let (quick, _) = render(&mut dv, &e);
        assert!(
            dv.stats.sharpen_deferred,
            "exposure {ev}: sharpening left out"
        );
        assert_eq!(
            quick, unsharpened,
            "exposure {ev}: the picture without sharpening"
        );
        assert_ne!(quick, want);
        assert!(dv.refine_due(), "the whole picture is due");
        let (whole, st) = render(&mut dv, &e);
        assert!(!dv.stats.sharpen_deferred);
        assert!(st.cache_hits > 0, "resumed from a checkpoint");
        assert_eq!(whole, want, "exposure {ev}: the whole picture, bit for bit");
        assert!(!dv.refine_due());
        first.get_or_insert((e.clone(), want));
    }
    // Sharpening itself changed: the picture resumes at it, whole at once.
    with(&mut e, ModuleKind::SHARPEN, "amount", 0.9);
    let (px, _) = render(&mut dv, &e);
    assert!(!dv.stats.sharpen_deferred);
    assert_eq!(px, render(&mut plain, &e).0);
    // Undo to a picture kept whole: copied, not deferred.
    let (a, want) = first.expect("a first edit");
    let (px, st) = render(&mut dv, &a);
    assert!(!dv.stats.sharpen_deferred);
    assert!(st.result_hit);
    assert_eq!(px, want);
    // Not deferring: never.
    with(&mut e, ModuleKind::EXPOSURE, "exposure_ev", 0.1);
    render(&mut plain, &e);
    assert!(!plain.stats.sharpen_deferred);
    // A drag never defers (its own draft).
    let r = dv
        .render(&e, Some((ModuleKind::EXPOSURE, 0)), &OutputSpace::rec709())
        .unwrap();
    dv.recycle(r);
    assert!(!dv.stats.sharpen_deferred);
}
