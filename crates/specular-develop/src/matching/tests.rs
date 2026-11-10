use super::*;
use crate::model::params::D50_XY;
use crate::model::{Registry, SourceKey};

/// A small deterministic generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A scene as it should look (linear Rec.2020, D65 white): independent pixels, each channel
/// uniform, so the scene averages to grey.
fn scene(seed: u64, n: usize) -> Vec<[f64; 3]> {
    let mut r = Rng(seed);
    (0..n)
        .map(|_| [0.0; 3].map(|_: f64| 0.01 + 0.21 * r.next()))
        .collect()
}

/// The camera's data for `scene` lit by `kelvin`, arriving balanced for D50 and `ev` stops
/// darker: what colour calibration with white = the illuminant turns back into the scene.
fn shot(scene: &[[f64; 3]], kelvin: f64, ev: f64, clipped: usize) -> Vec<f32> {
    let white = planck_xy(kelvin);
    let shot = [f64::from(D50_XY[0]), f64::from(D50_XY[1])];
    let m = colour::adaptation_rec2020(Cat::Cat16, shot, white);
    let k = (-ev).exp2();
    let mut out = Vec::with_capacity(scene.len() * 4);
    for (i, p) in scene.iter().enumerate() {
        let v = colour::mulv(&m, *p).map(|c| (c * k) as f32);
        // Some blown highlights: clipped values that must not count.
        let v = if i < clipped { [1.0, 1.0, 0.97] } else { v };
        out.extend_from_slice(&[v[0], v[1], v[2], 1.0]);
    }
    out
}

fn edit_with(white: [f64; 2], ev: f32) -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    let s = Solved {
        exposure_ev: ev,
        white_xy: [white[0] as f32, white[1] as f32],
    };
    e = applied(&e, &s);
    e
}

#[test]
fn planck_and_cct_agree() {
    for t in [2000.0, 2856.0, 4000.0, 5003.0, 6504.0, 9000.0, 15000.0] {
        let back = cct(planck_xy(t));
        assert!((back - t).abs() < t * 1e-3, "{t} K came back as {back} K");
    }
    // D65 is about 6504 K (it sits a little off the locus).
    let d65 = cct(colour::D65_XY);
    assert!((d65 - 6504.0).abs() < 30.0, "D65 = {d65} K");
}

/// Synthetic exposure and white-balance shifted pairs: the target's exposure and white
/// come back within 0.1 EV and 100 K (acceptance).
#[test]
fn recovers_exposure_and_white_on_shifted_pairs() {
    let mut worst_ev = 0f64;
    let mut worst_k = 0f64;
    let mut n = 0;
    for (i, &(ref_k, ref_ev)) in [(5000.0, 0.0), (6500.0, 0.4)].iter().enumerate() {
        let ref_scene = scene(11 + i as u64, 128 * 128);
        let ref_px = shot(&ref_scene, ref_k, 0.0, 400);
        let reference = Scene::from_rgba(&ref_px, 0.99).unwrap();
        let ref_edit = edit_with(planck_xy(ref_k), ref_ev);
        for (j, &(t_k, t_ev)) in [(3000.0, -1.5), (4300.0, 0.7), (7500.0, 2.0), (5600.0, -0.3)]
            .iter()
            .enumerate()
        {
            // Different content from the same kind of scene, darker or brighter by `t_ev`.
            let t_scene = scene(101 + (i * 10 + j) as u64, 96 * 160);
            let t_px = shot(&t_scene, t_k, t_ev, 250);
            let target = Scene::from_rgba(&t_px, 0.99).unwrap();
            let t_edit = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
            let s = solve(&reference, &ref_edit, &target, &t_edit).unwrap();
            let k = cct([f64::from(s.white_xy[0]), f64::from(s.white_xy[1])]);
            // The target must end where the reference does: its own shift undone, plus the
            // reference's exposure.
            let ev_err = (f64::from(s.exposure_ev) - (t_ev + f64::from(ref_ev))).abs();
            let k_err = (k - t_k).abs();
            println!(
                "ref {ref_k} K {ref_ev:+} EV → target {t_k} K {t_ev:+} EV: solved {k:.0} K {:+.3} EV (errors {k_err:.1} K, {ev_err:.4} EV)",
                s.exposure_ev
            );
            worst_ev = worst_ev.max(ev_err);
            worst_k = worst_k.max(k_err);
            n += 1;
            // Written as ordinary params.
            let e = applied(&t_edit, &s);
            let c = calibration(&e);
            assert_eq!(c.white_source, WhiteSource::Custom);
            assert_eq!(exposure(&e).exposure_ev, s.exposure_ev);
        }
    }
    println!("match to reference: {n} pairs, worst {worst_k:.1} K, {worst_ev:.4} EV");
    assert!(worst_ev <= 0.1, "exposure off by {worst_ev} EV");
    assert!(worst_k <= 100.0, "white off by {worst_k} K");
}

/// Linear Rec.2020 RGBA of `scene` shot as [`shot`] does, `w`×`h`.
fn input(scene: &[[f64; 3]], kelvin: f64, ev: f64, w: u32, h: u32) -> crate::preview::Input {
    crate::preview::Input::Rgb {
        width: w,
        height: h,
        rgba: shot(scene, kelvin, ev, 0),
    }
}

/// The same solve with both photos read back from the GPU engine (the app's path).
#[test]
fn matches_through_the_gpu_engine() {
    use crate::gpu::pool::Budget;
    use crate::preview::Developer;
    let Some(d) = crate::testgpu::dev() else {
        return;
    };
    let _l = crate::testgpu::lock();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (w, h) = (160u32, 120u32);
    let read = |dv: &mut Developer, scene: &[[f64; 3]], k: f64, ev: f64, e: &Edit| {
        dv.set_input(input(scene, k, ev, w, h)).unwrap();
        dv.prepare(e, 256).unwrap();
        let (_, _, px) = dv.scene_pixels(e, ModuleKind::EXPOSURE).unwrap();
        Scene::from_rgba(&px, 0.99).unwrap()
    };
    let ref_edit = edit_with(planck_xy(5500.0), 0.3);
    let t_edit = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    let reference = read(&mut dv, &scene(5, (w * h) as usize), 5500.0, 0.0, &ref_edit);
    let target = read(&mut dv, &scene(6, (w * h) as usize), 3800.0, -1.2, &t_edit);
    let s = solve(&reference, &ref_edit, &target, &t_edit).unwrap();
    let k = cct([f64::from(s.white_xy[0]), f64::from(s.white_xy[1])]);
    println!("GPU path: {k:.0} K (3800), {:+.3} EV (-0.9)", s.exposure_ev);
    assert!((k - 3800.0).abs() <= 100.0);
    assert!((f64::from(s.exposure_ev) - (-1.2 + 0.3)).abs() <= 0.1);
}

/// A style hovered in the Styles list is shown at preview size within 100 ms: the style applied to the edit and the picture developed again.
#[test]
fn style_hover_preview_is_quick() {
    use crate::gpu::pool::Budget;
    use crate::model::ApplyMode;
    use crate::model::style::shipped_styles;
    use crate::ops::output::OutputSpace;
    use crate::preview::Developer;
    use std::time::Instant;
    let Some(d) = crate::testgpu::dev() else {
        return;
    };
    let _l = crate::testgpu::lock();
    let reg = Registry::builtin();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    // A 3:2 photo already at about screen size (the app's non-raw input), shown 1280 px.
    let (w, h) = (2400u32, 1600u32);
    dv.set_input(input(&scene(9, (w * h) as usize), 5000.0, 0.0, w, h))
        .unwrap();
    let base = Edit::default_for(reg, SourceKey::default(), false);
    let out = OutputSpace::rec709();
    dv.prepare(&base, 1280).unwrap();
    let r = dv.render(&base, None, &out).unwrap();
    dv.recycle(r);
    dv.wait().unwrap();
    let styles = shipped_styles(reg);
    let mut times = Vec::new();
    for round in 0..3 {
        for s in &styles {
            let t0 = Instant::now();
            let e = s.applied(&base, reg, ApplyMode::Append);
            dv.prepare(&e, 1280).unwrap();
            let r = dv.render(&e, None, &out).unwrap();
            dv.recycle(r);
            dv.wait().unwrap();
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            if round > 0 {
                times.push(ms);
            } else {
                println!("first hover of {}: {ms:.1} ms (pipelines built)", s.name);
            }
        }
    }
    times.sort_by(f64::total_cmp);
    let p50 = times[times.len() / 2];
    let max = times[times.len() - 1];
    println!("style hover preview at 1280 px: p50 {p50:.1} ms, max {max:.1} ms");
    assert!(max <= 100.0, "hover preview took {max} ms");
}

#[test]
fn too_little_or_black_is_refused() {
    assert!(Scene::from_rgba(&[0.5; 4 * 10], 1.0).is_none());
    let black = Scene::from_rgba(&[0.0; 4 * 1000], 1.0);
    assert!(black.is_none());
}
