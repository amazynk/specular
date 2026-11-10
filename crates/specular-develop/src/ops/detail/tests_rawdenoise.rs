//! Raw luminance denoise against the same noise through the non-raw path.
//!
//! A synthetic RGGB sensor of flat patches (grey and coloured, 3–70 % of white) with
//! Poisson–Gaussian noise at ISO 1600 and 6400 levels is developed by our front end with
//! and without denoise; the noise left (luma, against the noise-free development) is
//! measured inside the patches. The noise model the raw path carries to denoise is checked
//! against the variance measured in the developed patches. The non-raw path gets the noisy
//! development as its picture (its own model, fitted on the image) and the same edit.

use super::noise::NoiseModel;
use super::tests_realnoise::{LEVELS, sensor};
use super::{ImageDetail, with_image};
use crate::gpu::pool::Budget;
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::preview::{Developer, Input};
use crate::testgpu::{dev, lock};

const W: usize = 512;
const H: usize = 512;
/// Patches per side and the margin left out at their edges (pixels).
const P: usize = 4;
const EDGE: usize = 12;

/// Planar linear camera RGB: a 4×4 grid of flat patches.
fn chart() -> Vec<f32> {
    let n = W * H;
    let mut v = vec![0f32; 3 * n];
    for y in 0..H {
        for x in 0..W {
            let k = (y * P / H) * P + x * P / W;
            let l = 0.03 + 0.67 * (k as f32 / 15.0).powf(1.5);
            let tint = match k % 4 {
                0 => [1.0, 1.0, 1.0],
                1 => [1.2, 1.0, 0.7],
                2 => [0.7, 1.0, 1.2],
                _ => [1.0, 0.85, 1.0],
            };
            for c in 0..3 {
                v[c * n + y * W + x] = (l * tint[c]).min(0.95);
            }
        }
    }
    v
}

fn denoise_edit(luminance: f32) -> Edit {
    let r = Registry::builtin();
    let mut m = ModuleInstance::new(r, ModuleKind::DENOISE, 0);
    let f = Fields::new()
        .with("luminance", Value::Float(luminance))
        .with("colour", Value::Float(0.0));
    m.params = Params::from_fields(ModuleKind::DENOISE, f);
    m.enabled = luminance > 0.0;
    let mut e = Edit::empty(SourceKey::default());
    e.insert(r, m);
    e
}

/// The developed picture after denoise (linear Rec.2020 RGBA) and the model it used.
fn develop(dv: &mut Developer, input: Input, luminance: f32) -> (Vec<f32>, Option<NoiseModel>) {
    let e = denoise_edit(luminance);
    dv.set_input(input).unwrap();
    dv.prepare(&e, W as u32).unwrap();
    let model = dv.noise_model();
    let info = ImageDetail {
        noise: model,
        ..ImageDetail::default()
    };
    let e = with_image(&e, &info).unwrap_or(e);
    let (w, h, px) = dv.scene_pixels(&e, ModuleKind::DEFRINGE).unwrap();
    assert_eq!((w as usize, h as usize), (W, H));
    (px, model)
}

fn luma(p: &[f32]) -> f64 {
    0.2627 * f64::from(p[0]) + 0.6780 * f64::from(p[1]) + 0.0593 * f64::from(p[2])
}

/// Inside each patch: the RMS luma difference of `a` from `clean`.
fn luma_rms(a: &[f32], clean: &[f32]) -> f64 {
    let (mut s, mut k) = (0.0, 0usize);
    for_patches(|i, _| {
        let d = luma(&a[i * 4..]) - luma(&clean[i * 4..]);
        s += d * d;
        k += 1;
    });
    (s / k as f64).sqrt()
}

fn for_patches(mut f: impl FnMut(usize, usize)) {
    let (pw, ph) = (W / P, H / P);
    for py in 0..P {
        for px in 0..P {
            for y in py * ph + EDGE..(py + 1) * ph - EDGE {
                for x in px * pw + EDGE..(px + 1) * pw - EDGE {
                    f(y * W + x, py * P + px);
                }
            }
        }
    }
}

/// Mean over patches of (measured noise variance) / (model's `a·m + b`), green channel.
fn model_ratio(noisy: &[f32], clean: &[f32], m: &NoiseModel) -> f64 {
    let mut acc = vec![(0.0f64, 0.0f64, 0usize); P * P];
    for_patches(|i, k| {
        let d = f64::from(noisy[i * 4 + 1] - clean[i * 4 + 1]);
        acc[k].0 += d * d;
        acc[k].1 += f64::from(clean[i * 4 + 1]);
        acc[k].2 += 1;
    });
    let r: Vec<f64> = acc
        .iter()
        .map(|(s, mean, n)| {
            let (var, mean) = (s / *n as f64, mean / *n as f64);
            var / (f64::from(m.a[1]) * mean + f64::from(m.b[1]))
        })
        .collect();
    r.iter().sum::<f64>() / r.len() as f64
}

#[test]
fn raw_luminance_denoise_matches_the_rgb_path() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let v = chart();
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.highlight_stats = false;
    let raw = |a: f64, b: f64| Input::Raw(Box::new(sensor(W, H, &v, a, b, 7)));
    let (clean, _) = develop(&mut dv, raw(0.0, 0.0), 0.0);
    for (name, a, b) in LEVELS.into_iter().skip(1) {
        let (noisy, model) = develop(&mut dv, raw(a, b), 0.0);
        let model = model.expect("a raw has a model");
        let (raw_dn, _) = develop(&mut dv, raw(a, b), 1.0);
        let rgb = || Input::Rgb {
            width: W as u32,
            height: H as u32,
            rgba: noisy.clone(),
        };
        let (_, rgb_model) = develop(&mut dv, rgb(), 0.0);
        let (rgb_dn, _) = develop(&mut dv, rgb(), 1.0);
        let before = luma_rms(&noisy, &clean);
        let after_raw = luma_rms(&raw_dn, &clean);
        let after_rgb = luma_rms(&rgb_dn, &clean);
        let ratio = model_ratio(&noisy, &clean, &model);
        let rgb_ratio = rgb_model.map_or(f64::NAN, |m| model_ratio(&noisy, &clean, &m));
        let max_change = raw_dn
            .iter()
            .zip(&noisy)
            .map(|(p, q)| (p - q).abs())
            .fold(0.0f32, f32::max);
        eprintln!(
            "{name}: luma noise {before:.5} → raw {after_raw:.5} ({:.2}×), rgb {after_rgb:.5} ({:.2}×); max change {max_change:.4}; measured / model variance raw {ratio:.2}, rgb {rgb_ratio:.2}; raw model a {:.2e} b {:.2e}",
            before / after_raw,
            before / after_rgb,
            model.a[1],
            model.b[1],
        );
        // The model the raw path carries is the developed noise's (within a factor of 2,
        // the demosaic's correlation aside), and full strength takes most of it away.
        assert!((0.5..2.0).contains(&ratio), "{name}: model off by {ratio}");
        assert!(before / after_raw > 2.0, "{name}: {before} → {after_raw}");
    }
}
