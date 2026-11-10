//! A binned first source on screen gives way to the full demosaic reduced once
//! nothing is dragged; a warp that resamples samples a larger source.

use super::*;
use crate::model::{Registry, SourceKey, Value};
use crate::ops::raw::tests::{bayer, sensor};
use crate::testgpu::{dev, lock};

fn raw() -> Input {
    Input::Raw(Box::new(sensor(&bayer(400, 300))))
}

fn edit() -> Edit {
    Edit::default_for(Registry::builtin(), SourceKey::default(), true)
}

fn picture(dv: &mut Developer, e: &Edit, focus: Option<(ModuleKind, u16)>) -> Vec<f32> {
    let r = dv.render(e, focus, &OutputSpace::rec709()).unwrap();
    let px = dv.read(&r).unwrap();
    dv.recycle(r);
    px
}

fn developer(d: &crate::testgpu::Dev, draft_first: bool) -> Developer {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.wait_highlight_stats = true;
    dv.draft_first = draft_first;
    dv.set_input(raw()).unwrap();
    dv
}

#[test]
fn binned_first_source_refines_when_nothing_is_dragged() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let e = edit();
    // The refined picture, as a one-off render (a thumbnail) makes it straight away.
    let mut once = developer(d, false);
    assert_eq!(once.prepare(&e, 150).unwrap(), (150, 113));
    assert!(!once.binned_draft && !once.refining());
    let want = picture(&mut once, &e, None);

    let mut dv = developer(d, true);
    assert_eq!(dv.prepare(&e, 150).unwrap(), (150, 113));
    assert!(dv.binned_draft, "the first source on screen is binned");
    let binned = picture(&mut dv, &e, Some((ModuleKind::EXPOSURE, 0)));
    // Dragging: the binned source stays.
    assert!(!dv.refining());
    assert!(!dv.refine_due());
    dv.prepare(&e, 150).unwrap();
    assert!(dv.binned_draft);
    // Let go: the refined source is due, made once.
    let _ = picture(&mut dv, &e, None);
    assert!(dv.refining());
    assert!(dv.refine_due());
    dv.prepare(&e, 150).unwrap();
    assert!(!dv.binned_draft && !dv.refining() && !dv.refine_due());
    let got = picture(&mut dv, &e, None);
    assert!(
        got.iter()
            .zip(&want)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "the refined source equals a one-off refined render"
    );
    let moved = binned
        .iter()
        .zip(&want)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(moved > 1e-3, "binned and refined differ ({moved})");
    // A new size starts binned again.
    dv.prepare(&e, 200).unwrap();
    assert!(dv.binned_draft);
}

#[test]
fn a_resampling_warp_samples_a_larger_source() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let mut e = edit();
    let mut dv = developer(d, false);
    dv.prepare(&e, 150).unwrap();
    assert!(dv.oversampled.is_none(), "no warp: no larger source");
    // Lens distortion resamples: the source is also made larger (here the sensor's size,
    // the least of twice the view and 2048 px).
    let reg = Registry::builtin();
    if e.module_mut(ModuleKind::LENS, 0).is_none() {
        e.insert(
            reg,
            crate::model::ModuleInstance::new(reg, ModuleKind::LENS, 0),
        );
    }
    let m = e.module_mut(ModuleKind::LENS, 0).unwrap();
    let mut f = m.params.to_fields();
    f.set("distortion", Value::Float(15.0));
    m.params.set_fields(f);
    assert_eq!(dv.prepare(&e, 150).unwrap(), (150, 113));
    let (_, big) = dv.oversampled.as_ref().expect("a larger source");
    assert_eq!((big.width, big.height), (400, 300));
    let px = picture(&mut dv, &e, None);
    assert!(px.iter().all(|v| v.is_finite()));
}
