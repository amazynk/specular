//! Denoise before the warp ([`crate::prewarp`]) in the preview.

use super::*;
use crate::model::params::CropOrientation;
use crate::model::{Fields, ModuleInstance, Registry, SourceKey, Value};
use crate::ops::detail::denoise::Denoise;
use crate::ops::geometry::LensParams;
use crate::testgpu::{dev, lock};

/// xorshift64* in 0..1.
struct Rng(u64);

impl Rng {
    fn u(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Edges and gradients with uniform noise, linear RGBA.
fn noisy(w: u32, h: u32, seed: u64) -> Vec<f32> {
    let mut r = Rng(seed | 1);
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let block = if (x / 16 + y / 12) % 2 == 0 {
                0.15
            } else {
                0.0
            };
            let c = [0.1 + 0.5 * u + block, 0.2 + 0.4 * t, 0.3 + block];
            for k in c {
                v.push(k + 0.08 * (r.u() - 0.5));
            }
            v.push(1.0);
        }
    }
    v
}

fn input(w: u32, h: u32, rgba: Vec<f32>) -> Input {
    Input::Rgb {
        width: w,
        height: h,
        rgba,
    }
}

/// Denoise (strong), and with `geometry` a straighten, crop and lens correction.
fn edit(denoise: bool, geometry: bool) -> Edit {
    let r = Registry::builtin();
    let mut e = Edit::empty(SourceKey::default());
    if denoise {
        let mut m = ModuleInstance::new(r, ModuleKind::DENOISE, 0);
        let mut f: Fields = Denoise::default_fields()
            .with("luminance", Value::Float(0.8))
            .with("colour", Value::Float(0.8));
        // The noise as made (a fixed model: every developer here uses the same).
        crate::ops::detail::NoiseModel {
            a: [1.0e-5; 3],
            b: [5.3e-4; 3],
        }
        .write(&mut f);
        m.params = Params::from_fields(ModuleKind::DENOISE, f);
        e.insert(r, m);
    }
    if geometry {
        let mut c = ModuleInstance::new(r, ModuleKind::CROP_ORIENTATION, 0);
        c.params = Params::CropOrientation(CropOrientation {
            angle_deg: 3.0,
            crop: [0.12, 0.1, 0.8, 0.85],
            ..Default::default()
        });
        e.insert(r, c);
        let mut l = ModuleInstance::new(r, ModuleKind::LENS, 0);
        let lp = LensParams {
            distortion: 25.0,
            tca_red: 0.1,
            ..LensParams::default()
        };
        l.params = Params::Other(ModuleKind::LENS, lp.to_fields());
        e.insert(r, l);
    }
    e
}

fn worst(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q).abs())
        .fold(0.0, f32::max)
}

/// With a warp, the picture is the warp of the denoised unwarped picture: denoise sees
/// the noise as the sensor made it. Only the crop's part of the source is denoised, with
/// the same pixels there as a whole-frame denoise.
#[test]
fn denoise_runs_on_the_unwarped_picture() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (320u32, 240u32);
    let img = noisy(w, h, 7);
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.set_input(input(w, h, img.clone())).unwrap();
    let full = edit(true, true);
    dv.prepare(&full, w).unwrap();
    let (pw, ph, warped) = dv.scene_pixels(&full, ModuleKind::DEFRINGE).unwrap();
    assert!(pw < w && ph < h, "cropped: {pw}×{ph}");
    // The reference: denoise alone, then the same geometry on its result.
    let plain = edit(true, false);
    let (_, _, den) = dv.scene_pixels(&plain, ModuleKind::DEFRINGE).unwrap();
    let mut rf = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    rf.set_input(input(w, h, den)).unwrap();
    let geo = edit(false, true);
    rf.prepare(&geo, w).unwrap();
    let (rw, rh, want) = rf.scene_pixels(&geo, ModuleKind::DEFRINGE).unwrap();
    assert_eq!((rw, rh), (pw, ph));
    let e = worst(&warped, &want);
    // Denoise after the warp (the old order) is far from it.
    let mut old = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    old.set_input(input(w, h, img)).unwrap();
    old.prepare(&geo, w).unwrap();
    let (_, _, noisy_warped) = old.scene_pixels(&geo, ModuleKind::DEFRINGE).unwrap();
    let mut o2 = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    o2.set_input(input(pw, ph, noisy_warped)).unwrap();
    o2.prepare(&plain, pw.max(ph)).unwrap();
    let (_, _, after) = o2.scene_pixels(&plain, ModuleKind::DEFRINGE).unwrap();
    let mean = |a: &[f32], b: &[f32]| {
        a.iter()
            .zip(b)
            .map(|(p, q)| f64::from((p - q).abs()))
            .sum::<f64>()
            / a.len() as f64
    };
    let old_mean = mean(&after, &want);
    eprintln!(
        "denoise before the warp: worst {e:.2e} vs the reference; the old order differs by {old_mean:.2e} on average"
    );
    assert!(e < 1e-4, "{e}");
    assert!(old_mean > 1e-3, "{old_mean}");
}

/// What denoise before the warp costs in the preview: a fit view of a 24 MP photo
/// (2560×1707) with lens, straighten and crop, the denoise slider moved each render,
/// against denoise after the warp. Prints p50 per mode; run by hand.
#[test]
#[ignore = "timing report; run by hand"]
fn denoise_before_the_warp_cost() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (2560u32, 1707u32);
    for (name, crop) in [
        ("whole frame", [0.0, 0.0, 1.0, 1.0]),
        ("crop 0.1-0.9", [0.1, 0.1, 0.9, 0.9]),
        ("crop 0.3-0.7", [0.3, 0.3, 0.7, 0.7]),
    ] {
        for after in [true, false] {
            let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
            dv.denoise_after_warp = after;
            dv.set_input(input(w, h, noisy(w, h, 3))).unwrap();
            let mut e = edit(true, true);
            if let Some(m) = e.module_mut(ModuleKind::CROP_ORIENTATION, 0) {
                m.params = Params::CropOrientation(CropOrientation {
                    angle_deg: 3.0,
                    crop,
                    ..Default::default()
                });
            }
            dv.prepare(&e, w).unwrap();
            let mut t = Vec::new();
            for i in 0..9 {
                if let Some(m) = e.module_mut(ModuleKind::DENOISE, 0) {
                    let f = m
                        .params
                        .to_fields()
                        .with("luminance", Value::Float(0.5 + 0.02 * i as f32));
                    m.params = Params::from_fields(ModuleKind::DENOISE, f);
                }
                let t0 = std::time::Instant::now();
                let r = dv
                    .render(
                        &e,
                        Some((ModuleKind::DENOISE, 0)),
                        &crate::ops::output::OutputSpace::rec709(),
                    )
                    .unwrap();
                dv.wait().unwrap();
                if i > 0 {
                    t.push(t0.elapsed().as_secs_f64() * 1e3);
                }
                dv.recycle(r);
            }
            t.sort_by(f64::total_cmp);
            let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
            eprintln!(
                "{name}, denoise {}: render p50 {:.0} ms (min {:.0}); load {}",
                if after {
                    "after the warp"
                } else {
                    "before the warp"
                },
                t[t.len() / 2],
                t[0],
                load.split_whitespace().next().unwrap_or("?")
            );
        }
    }
}

/// The region denoised before the warp covers what the warp reads, and a crop shrinks it.
#[test]
fn the_denoised_region_follows_the_crop() {
    let ctx = crate::ops::geometry::Context::image(1000, 800, Orient::default());
    let whole = Plan::new(&edit(true, true), &ctx, false);
    let wb = crate::prewarp::source_box(&whole, 1000, 800);
    let cropped = Plan::new(&edit(true, true), &ctx, true);
    let b = crate::prewarp::source_box(&cropped, 1000, 800);
    assert!(wb.contains(&b), "{wb:?} {b:?}");
    assert!(b.pixels() < 1000 * 800 * 3 / 4, "{b:?}");
    // Every output pixel's source taps lie inside.
    let (ow, oh) = cropped.output_size(1.0);
    for j in (0..oh).step_by(7) {
        for i in (0..ow).step_by(7) {
            for p in 0..3 {
                let (s, _) = cropped.to_source(p, (f64::from(i) + 0.5, f64::from(j) + 0.5), 1.0);
                let (x, y) = (s.0 - 0.5, s.1 - 0.5);
                assert!(
                    x - 2.0 >= f64::from(b.x) && x + 2.0 <= f64::from(b.right()),
                    "x {x} outside {b:?}"
                );
                assert!(
                    y - 2.0 >= f64::from(b.y) && y + 2.0 <= f64::from(b.bottom()),
                    "y {y} outside {b:?}"
                );
            }
        }
    }
}
