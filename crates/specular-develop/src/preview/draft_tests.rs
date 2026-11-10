//! Half-scale drafts and kept refined results.

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

/// A non-raw edit with neighbourhood ops after exposure.
fn edit() -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    with(&mut e, ModuleKind::EXPOSURE, "exposure_ev", 0.3);
    with(&mut e, ModuleKind::LOCAL_CONTRAST, "texture", 0.3);
    with(&mut e, ModuleKind::LOCAL_CONTRAST, "clarity", 0.2);
    with(&mut e, ModuleKind::TONE_EQUALIZER, "shadows_ev", 0.4);
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
    assert_eq!(dv.prepare(&edit(), w).unwrap(), (w, h));
    dv
}

fn rgba8(dv: &mut Developer, e: &Edit, focus: Option<(ModuleKind, u16)>) -> Vec<u8> {
    let r = dv.render(e, focus, &OutputSpace::rec709()).unwrap();
    dv.read_rgba8(r, Orient::default()).unwrap().2
}

/// A draft of a large picture with a neighbourhood op after the dragged module runs at
/// half scale, comes out at the picture's size close to the refined render, and leaves
/// the refined render exactly as it was. Small pictures, modules with their own drafts
/// and `half_drafts` off keep the full scale.
#[test]
fn drafts_of_large_pictures_run_at_half_scale() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (1600u32, 1000u32);
    assert!(u64::from(w * h) >= HALF_DRAFT_PIXELS);
    let mut dv = developer(d, w, h);
    let e = edit();
    let refined = rgba8(&mut dv, &e, None);
    assert!(!dv.stats.half_draft);
    let draft = rgba8(&mut dv, &e, Some((ModuleKind::EXPOSURE, 0)));
    assert!(dv.stats.half_draft);
    assert_eq!(draft.len(), refined.len());
    let n = (w * h) as usize;
    let diff: Vec<i32> = (0..n * 4)
        .filter(|i| i % 4 != 3)
        .map(|i| (i32::from(draft[i]) - i32::from(refined[i])).abs())
        .collect();
    let mean = diff.iter().sum::<i32>() as f64 / diff.len() as f64;
    let mut sorted = diff.clone();
    sorted.sort_unstable();
    let p99 = sorted[sorted.len() * 99 / 100];
    println!("half-scale draft vs refined (8-bit sRGB): mean {mean:.3}, p99 {p99}");
    assert!(mean < 1.5 && p99 <= 12, "mean {mean}, p99 {p99}");
    // Refined again: as before the draft, bit for bit.
    dv.keep_results = false;
    assert_eq!(rgba8(&mut dv, &e, None), refined);
    // A module with a draft of its own stays at full scale.
    let mut diffuse = e.clone();
    with(&mut diffuse, ModuleKind::DIFFUSE, "iterations", 2.0);
    rgba8(&mut dv, &diffuse, Some((ModuleKind::DIFFUSE, 0)));
    assert!(!dv.stats.half_draft);
    // Point ops alone after the focus: one fused pass, full scale.
    let mut late = e.clone();
    with(&mut late, ModuleKind::VIEW_TRANSFORM, "contrast", 1.1);
    rgba8(&mut dv, &late, Some((ModuleKind::VIEW_TRANSFORM, 0)));
    assert!(!dv.stats.half_draft);
    // Switched off.
    dv.half_drafts = false;
    rgba8(&mut dv, &e, Some((ModuleKind::EXPOSURE, 0)));
    assert!(!dv.stats.half_draft);
    // A small picture.
    let mut small = developer(d, 600, 400);
    rgba8(&mut small, &e, Some((ModuleKind::EXPOSURE, 0)));
    assert!(!small.stats.half_draft);
}

/// Undo and redo between two refined edits copy the kept results: the same pixels, no
/// kernel run. Drafts are not kept.
#[test]
fn undo_and_redo_copy_kept_results() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut dv = developer(d, 640, 400);
    let a = edit();
    let mut b = a.clone();
    with(&mut b, ModuleKind::COLOUR_BALANCE, "vibrance", 0.4);
    let out = OutputSpace::rec709();
    let first = rgba8(&mut dv, &a, None);
    let _ = rgba8(&mut dv, &b, None);
    let r = dv.render(&a, None, &out).unwrap();
    assert!(r.stats.result_hit);
    assert_eq!(r.stats.dispatches, 0);
    assert_eq!(dv.read_rgba8(r, Orient::default()).unwrap().2, first);
    let r = dv.render(&b, None, &out).unwrap();
    assert!(r.stats.result_hit);
    dv.recycle(r);
    // A draft renders (and is not kept).
    let mut c = b.clone();
    with(&mut c, ModuleKind::COLOUR_BALANCE, "vibrance", 0.1);
    for _ in 0..2 {
        let r = dv
            .render(&c, Some((ModuleKind::COLOUR_BALANCE, 0)), &out)
            .unwrap();
        assert!(!r.stats.result_hit);
        dv.recycle(r);
    }
    // Without keeping, nothing is copied.
    dv.keep_results = false;
    let r = dv.render(&a, None, &out).unwrap();
    assert!(!r.stats.result_hit);
    assert_eq!(dv.read_rgba8(r, Orient::default()).unwrap().2, first);
}
