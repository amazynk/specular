//! Adaptive preview while dragging ([`crate::drag`]): the levels a module in focus is
//! drafted at.

use super::*;
use crate::drag::Level;
use crate::model::{ModuleInstance, Registry, SourceKey, Value};
use crate::testgpu::{dev, lock};

fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let tex = 0.02 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
            v.extend_from_slice(&[
                0.03 + 0.5 * fx + tex,
                0.04 + 0.4 * fy + tex,
                0.05 + 0.3 * (1.0 - fx) + tex,
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

/// Point ops only: exposure and white balance were never drafted before.
fn plain() -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    with(&mut e, ModuleKind::EXPOSURE, "exposure_ev", 0.3);
    e
}

fn developer(d: &crate::testgpu::Dev, w: u32, h: u32) -> Developer {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba: scene(w, h),
    })
    .unwrap();
    assert_eq!(dv.prepare(&plain(), w).unwrap(), (w, h));
    dv
}

fn rgba8(dv: &mut Developer, e: &Edit, focus: Option<(ModuleKind, u16)>) -> Vec<u8> {
    let r = dv.render(e, focus, &OutputSpace::rec709()).unwrap();
    dv.read_rgba8(r, Orient::default()).unwrap().2
}

fn mean_diff(a: &[u8], b: &[u8]) -> f64 {
    let d: u64 = a
        .iter()
        .zip(b)
        .enumerate()
        .filter(|(i, _)| i % 4 != 3)
        .map(|(_, (x, y))| u64::from(x.abs_diff(*y)))
        .sum();
    d as f64 / (a.len() / 4 * 3) as f64
}

/// A point slider (exposure) drafts at the level asked for, at the picture's size and
/// close to the refined render; the refined render is the same bit for bit afterwards.
/// Without a level it keeps the old rule (full scale: nothing reads neighbours).
#[test]
fn point_sliders_draft_at_the_level_asked_for() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (1600u32, 1000u32);
    let mut dv = developer(d, w, h);
    dv.keep_results = false;
    let e = plain();
    let focus = (ModuleKind::EXPOSURE, 0);
    let refined = rgba8(&mut dv, &e, None);
    assert_eq!(dv.stats.draft_divisor, 1);
    assert_eq!(dv.drag_default(&e, focus), Level::Full);
    // The old rule: full scale.
    let _ = rgba8(&mut dv, &e, Some(focus));
    assert!(!dv.stats.half_draft);
    for (level, divisor, bar) in [
        (Level::Full, 1, 0.01),
        (Level::Half, 2, 1.0),
        (Level::Quarter, 4, 2.0),
        (Level::Pause, 4, 2.0),
    ] {
        dv.drag_level = Some(level);
        let draft = rgba8(&mut dv, &e, Some(focus));
        assert_eq!(dv.stats.draft_divisor, divisor, "{level:?}");
        assert_eq!(dv.stats.half_draft, divisor > 1);
        assert_eq!(draft.len(), refined.len());
        let m = mean_diff(&draft, &refined);
        println!("{level:?} draft vs refined (8-bit sRGB): mean {m:.3}");
        assert!(m <= bar, "{level:?}: mean {m}");
    }
    // Released: the refined picture, bit for bit.
    assert_eq!(rgba8(&mut dv, &e, None), refined);
    assert_eq!(dv.stats.draft_divisor, 1);
}

/// "Always sharp" (full) keeps a neighbourhood edit at full scale where the old rule
/// halved it; modules with drafts of their own are never reduced; a small picture is
/// reduced only down to `MIN_DRAFT_LONG`.
#[test]
fn levels_respect_exempt_modules_and_small_pictures() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = developer(d, 1600, 1000);
    let mut e = plain();
    with(&mut e, ModuleKind::LOCAL_CONTRAST, "clarity", 0.3);
    let focus = (ModuleKind::EXPOSURE, 0);
    assert_eq!(dv.drag_default(&e, focus), Level::Half);
    let _ = rgba8(&mut dv, &e, Some(focus));
    assert_eq!(dv.stats.draft_divisor, 2, "the old rule halves");
    dv.drag_level = Some(Level::Full);
    let _ = rgba8(&mut dv, &e, Some(focus));
    assert_eq!(dv.stats.draft_divisor, 1);
    dv.drag_level = Some(Level::Quarter);
    let _ = rgba8(&mut dv, &e, Some(focus));
    assert_eq!(dv.stats.draft_divisor, 4);
    let mut diffuse = e.clone();
    with(&mut diffuse, ModuleKind::DIFFUSE, "iterations", 2.0);
    let _ = rgba8(&mut dv, &diffuse, Some((ModuleKind::DIFFUSE, 0)));
    assert_eq!(dv.stats.draft_divisor, 1, "own drafts");
    // Small: 600 px long, a quarter would be 150 px: a half (300 px) instead.
    let mut small = developer(d, 600, 400);
    small.drag_level = Some(Level::Quarter);
    let _ = rgba8(&mut small, &e, Some(focus));
    assert_eq!(small.stats.draft_divisor, 2);
    assert_eq!(fit_divisor(4, 400, 300), 1);
    assert_eq!(fit_divisor(4, 2048, 100), 4);
}

/// Frame times arrive once the GPU is done, once each.
#[test]
fn frames_are_timed_when_asked() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = developer(d, 640, 400);
    let e = plain();
    let _ = rgba8(&mut dv, &e, None);
    assert_eq!(dv.take_frame_ms(), None, "off by default");
    dv.time_frames(true);
    let r = dv
        .render(&e, Some((ModuleKind::EXPOSURE, 0)), &OutputSpace::rec709())
        .unwrap();
    dv.recycle(r);
    let t = std::time::Instant::now();
    let ms = loop {
        if let Some(ms) = dv.take_frame_ms() {
            break ms;
        }
        assert!(t.elapsed().as_secs() < 30, "no frame time");
        std::thread::yield_now();
    };
    assert!(ms > 0.0 && ms < 30_000.0, "{ms}");
    assert_eq!(dv.take_frame_ms(), None, "taken once");
}
