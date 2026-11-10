//! Denoise's AI mode. The kernel's mix of the network's result against the CPU
//! reference (GPU = CPU), the classic result where the result is missing or not done yet,
//! the key set per image, and (with the oracle set, `tools/corpus/denoise-oracle.py`)
//! PSNR/SSIM of the network against the classic denoise.

use super::ai::{self, AiParams, AiResult};
use super::cpu;
use super::denoise::Denoise;
use super::noise::NoiseModel;
use super::{ImageDetail, with_image};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request};
use crate::graph::{Graph, OpRegistry, Precision, Roi};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::ops::tone::cpu::Img;
use crate::testgpu::{dev, lock};

const GIB: u64 = 1 << 30;

pub(super) fn engine() -> Option<Engine> {
    let d = dev()?;
    Some(Engine::new(&d.device, &d.queue, Budget::fixed(2 * GIB)))
}

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

/// A scene with edges and gradients, RGBA.
fn scene(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let block = if (x / 16 + y / 12) % 2 == 0 {
                0.15
            } else {
                0.0
            };
            v.extend_from_slice(&[0.1 + 0.5 * u + block, 0.2 + 0.4 * t, 0.3 + block, 1.0]);
        }
    }
    v
}

fn noisy(img: &[f32], seed: u64, s: f32) -> Vec<f32> {
    let mut r = Rng(seed | 1);
    img.chunks_exact(4)
        .flat_map(|p| {
            [
                p[0] + s * (r.u() - 0.5),
                p[1] + s * (r.u() - 0.5),
                p[2] + s * (r.u() - 0.5),
                p[3],
            ]
        })
        .collect()
}

const MODEL: NoiseModel = NoiseModel {
    a: [4.0e-4, 3.0e-4, 5.0e-4],
    b: [6.0e-6, 4.0e-6, 6.0e-6],
};

fn denoise_fields(l: f32, c: f32) -> Fields {
    let mut f = Denoise::default_fields()
        .with("luminance", Value::Float(l))
        .with("colour", Value::Float(c));
    MODEL.write(&mut f);
    f
}

pub(super) fn ai_fields(l: f32, c: f32, blend: f32, key: &[u8; 32]) -> Fields {
    let mut f = denoise_fields(l, c)
        .with("mode", Value::Text("ai".into()))
        .with("model", Value::Text("ffdnet-color".into()))
        .with("blend", Value::Float(blend));
    ai::write_hidden(&mut f, Some(key), None);
    f
}

pub(super) fn edit_of(f: Fields) -> Edit {
    let r = Registry::builtin();
    let mut m = ModuleInstance::new(r, ModuleKind::DENOISE, 0);
    m.params = Params::from_fields(ModuleKind::DENOISE, f);
    let mut e = Edit::empty(SourceKey::default());
    e.insert(r, m);
    e
}

pub(super) fn render(en: &mut Engine, img: &[f32], w: u32, h: u32, e: &Edit) -> Vec<f32> {
    let src = en.upload(w, h, img).expect("upload");
    let g = Graph::build(e, OpRegistry::builtin(), None).expect("graph");
    let req = Request {
        precision: Precision::F32,
        checkpoints: false,
        scale: 1.0,
        ..Request::new(Roi::full(w, h))
    };
    let r = en.render(&g, &src, &req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

/// A result over the whole `w × h` frame from a "network" that returns `out` (linear,
/// RGBA), done over the cell rows `rows`.
fn result(w: u32, h: u32, input: &[f32], out: &[f32], rows: std::ops::Range<u32>) -> AiResult {
    let mut r = AiResult::new(w, h, 16);
    let n = (w * h) as usize;
    let mut enc = vec![0f32; 3 * n];
    for i in 0..n {
        let e = ai::encode([out[4 * i], out[4 * i + 1], out[4 * i + 2]]);
        for c in 0..3 {
            enc[c * n + i] = e[c];
        }
    }
    // Hand the rows to the result as the tiler would: a band of planes per cell row.
    for cy in rows {
        let y0 = cy * 16;
        let rh = 16.min(h - y0);
        let m = (w * rh) as usize;
        let mut band = vec![0f32; 3 * m];
        for c in 0..3 {
            for k in 0..m {
                band[c * m + k] = enc[c * n + (y0 * w) as usize + k];
            }
        }
        r.put((0, y0, w, rh), &band, input, &ai::Encoding::Srgb);
    }
    r
}

/// (worst difference, pixels over `tol`).
fn diff(a: &[f32], b: &[f32], tol: f32) -> (f32, usize) {
    let mut worst = 0f32;
    let mut over = 0;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = (0..3).map(|k| (p[k] - q[k]).abs()).fold(0f32, f32::max);
        worst = worst.max(d);
        over += usize::from(d > tol);
    }
    (worst, over)
}

#[test]
fn ai_mode_matches_the_cpu_reference() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (96, 64);
    let clean = scene(w, h);
    let img = noisy(&clean, 3, 0.08);
    let key = [24u8; 32];
    // The "network" returns the clean scene; half of the cell rows are done.
    let r = result(w, h, &img, &clean, 0..2);
    ai::store()
        .lock()
        .unwrap()
        .update(&key, (w, h, 16), |x| *x = r.clone());
    let blend = 0.7;
    let e = edit_of(ai_fields(0.4, 0.5, blend, &key));
    let gpu = render(&mut en, &img, w, h, &e);
    // CPU: the classic result, mixed with the result's samples where it is done.
    let d = Denoise::from_fields(&denoise_fields(0.4, 0.5));
    let full = Roi::full(w, h);
    let classic = cpu::denoise(&Img { w, h, px: &img }, &d, full, full, 1.0);
    let table = ai::store()
        .lock()
        .unwrap()
        .table(&key, &ai::identity_grid())
        .unwrap();
    let mut want = classic.clone();
    let mut mixed = 0;
    for y in 0..h {
        for x in 0..w {
            if let Some(a) = ai::sample(&table.words, (w, h), x as i32, y as i32) {
                let i = ((y * w + x) * 4) as usize;
                for c in 0..3 {
                    want[i + c] = classic[i + c] + blend * (a[c] - classic[i + c]);
                }
                mixed += 1;
            }
        }
    }
    assert!(
        mixed > (w * h / 4) as usize && mixed < (w * h) as usize,
        "{mixed}"
    );
    let (worst, over) = diff(&gpu, &want, 1e-3);
    eprintln!("AI mode: GPU vs CPU worst {worst:.2e}, {over} over 1e-3, {mixed} pixels mixed");
    assert_eq!(over, 0, "worst {worst}");
    // Rows not done yet are the classic result (the draft).
    let gpu_classic = render(&mut en, &img, w, h, &edit_of(denoise_fields(0.4, 0.5)));
    let tail = (40 * w * 4) as usize;
    let (worst, _) = diff(&gpu[tail..], &gpu_classic[tail..], 0.0);
    assert_eq!(worst, 0.0, "cells not done must show the classic result");
    // Mixed rows moved towards the clean scene.
    let err = |v: &[f32]| -> f64 {
        v[..(16 * w * 4) as usize]
            .iter()
            .zip(&clean)
            .map(|(a, b)| f64::from((a - b).abs()))
            .sum()
    };
    assert!(err(&gpu) < err(&gpu_classic));
    ai::store().lock().unwrap().remove(&key);
}

#[test]
fn without_a_result_the_classic_result_shows() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (64, 48);
    let img = noisy(&scene(w, h), 5, 0.06);
    let missing = [99u8; 32];
    let a = render(
        &mut en,
        &img,
        w,
        h,
        &edit_of(ai_fields(0.5, 0.5, 1.0, &missing)),
    );
    let b = render(&mut en, &img, w, h, &edit_of(denoise_fields(0.5, 0.5)));
    assert_eq!(diff(&a, &b, 0.0).0, 0.0);
    // Classic strengths at 0, AI on without a key (no photo): left out entirely.
    let mut f = ai_fields(0.0, 0.0, 1.0, &missing);
    f.remove("ai_key");
    let op = OpRegistry::builtin()
        .get(ModuleKind::DENOISE, 1)
        .unwrap()
        .clone();
    assert!(op.identity(&Params::from_fields(ModuleKind::DENOISE, f.clone())));
    // With a key it runs (the result may arrive).
    let keyed = ai_fields(0.0, 0.0, 1.0, &missing);
    assert!(!op.identity(&Params::from_fields(ModuleKind::DENOISE, keyed)));
    // Blend 0 is the classic mode.
    let off = ai_fields(0.0, 0.0, 0.0, &missing);
    assert!(op.identity(&Params::from_fields(ModuleKind::DENOISE, off)));
}

#[test]
fn the_photo_keys_the_ai_result_on_a_copy() {
    let mut f = Denoise::default_fields()
        .with("luminance", Value::Float(0.3))
        .with("mode", Value::Text("ai".into()))
        .with("model", Value::Text("ffdnet-color".into()));
    f.set("blend", Value::Float(0.6));
    let e = edit_of(f);
    // No photo: nothing to key.
    assert!(with_image(&e, &ImageDetail::default()).is_none());
    let photo = [3u8; 32];
    let info = ImageDetail {
        photo: Some(photo),
        ..ImageDetail::default()
    };
    let w = with_image(&e, &info).expect("keyed");
    let p = AiParams::from_fields(&w.module(ModuleKind::DENOISE, 0).unwrap().params.to_fields());
    assert_eq!(p.key, Some(ai::key(&photo, &e, "ffdnet-color")));
    assert_eq!(p.blend, 0.6);
    // The stored edit never has the key.
    let stored = e.module(ModuleKind::DENOISE, 0).unwrap().params.to_fields();
    assert!(stored.get("ai_key").is_none());
    // Classic mode: no key.
    let mut c = e.clone();
    let m = c
        .modules
        .iter_mut()
        .find(|m| m.kind == ModuleKind::DENOISE)
        .unwrap();
    let mut f = m.params.to_fields();
    f.set("mode", Value::Text("classic".into()));
    m.params.set_fields(f);
    assert!(with_image(&c, &info).is_none());
}

// -- Quality against the classic denoise (oracle set only) ---------------------------------

pub(super) fn read(p: &std::path::Path) -> Option<(usize, usize, Vec<f32>)> {
    let b = std::fs::read(p).ok()?;
    let w = u32::from_le_bytes(b[0..4].try_into().ok()?) as usize;
    let h = u32::from_le_bytes(b[4..8].try_into().ok()?) as usize;
    let v: Vec<f32> = b[8..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    (v.len() == 3 * w * h).then_some((w, h, v))
}

/// Planar linear sRGB → RGBA linear Rec.2020.
fn to_rgba(w: usize, h: usize, v: &[f32]) -> Vec<f32> {
    let n = w * h;
    let mut out = Vec::with_capacity(4 * n);
    for i in 0..n {
        let s = [v[i], v[n + i], v[2 * n + i]];
        let m = &ai::FROM_SRGB;
        for row in m {
            out.push(row[0] * s[0] + row[1] * s[1] + row[2] * s[2]);
        }
        out.push(1.0);
    }
    out
}

fn enc(v: f32) -> f64 {
    f64::from(v.clamp(0.0, 1.0)).powf(1.0 / 2.2)
}

pub(super) fn psnr(a: &[f32], b: &[f32]) -> f64 {
    let mut se = 0.0;
    let mut n = 0.0;
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        for k in 0..3 {
            se += (enc(p[k]) - enc(q[k])).powi(2);
            n += 1.0;
        }
    }
    10.0 * (n / se).log10()
}

/// Mean SSIM of the encoded luma over 8×8 windows (stride 4), as the classic denoise's check.
pub(super) fn ssim(a: &[f32], b: &[f32], w: usize, h: usize) -> f64 {
    let y = |v: &[f32], x: usize, yy: usize| {
        let i = (yy * w + x) * 4;
        0.2627 * enc(v[i]) + 0.678 * enc(v[i + 1]) + 0.0593 * enc(v[i + 2])
    };
    let (c1, c2) = (0.01f64.powi(2), 0.03f64.powi(2));
    let (mut total, mut n) = (0.0, 0.0);
    for by in (0..h - 8).step_by(4) {
        for bx in (0..w - 8).step_by(4) {
            let (mut ma, mut mb, mut va, mut vb, mut cov) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for j in 0..8 {
                for i in 0..8 {
                    ma += y(a, bx + i, by + j);
                    mb += y(b, bx + i, by + j);
                }
            }
            ma /= 64.0;
            mb /= 64.0;
            for j in 0..8 {
                for i in 0..8 {
                    let (p, q) = (y(a, bx + i, by + j) - ma, y(b, bx + i, by + j) - mb);
                    va += p * p;
                    vb += q * q;
                    cov += p * q;
                }
            }
            let (va, vb, cov) = (va / 63.0, vb / 63.0, cov / 63.0);
            total += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1.0;
        }
    }
    total / n
}

/// The noise model of a level (the script's, in sRGB primaries), carried to Rec.2020.
fn level(name: &str) -> NoiseModel {
    let (a, b): ([f32; 3], [f32; 3]) = match name {
        "iso400" => ([1.6e-4, 1.0e-4, 1.3e-4], [8.0e-8, 4.0e-8, 6.0e-8]),
        "iso1600" => ([6.4e-4, 4.0e-4, 5.2e-4], [1.2e-6, 6.0e-7, 9.0e-7]),
        _ => ([2.56e-3, 1.6e-3, 2.08e-3], [2.0e-5, 1.0e-5, 1.5e-5]),
    };
    let m = &ai::FROM_SRGB;
    let mix = |v: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|r| (0..3).map(|k| m[r][k] * m[r][k] * v[k]).sum::<f32>())
    };
    NoiseModel {
        a: mix(a),
        b: mix(b),
    }
}

/// PSNR/SSIM of AI denoise (installed f16 weights, tiled, through Develop's kernel) against
/// the best classic denoise on the oracle set. Runs with `SPECULAR_DENOISE_ORACLE` (the
/// prepared set) and `SPECULAR_DENOISE_MODELS` (with the converted `….f16.safetensors`
/// files the specular-infer oracle writes); `SPECULAR_DENOISE_NET` picks the network.
#[test]
fn ai_denoise_against_the_classic_on_the_oracle_set() {
    use specular_infer::ffdnet::{self, FfdConfig};
    use specular_infer::gpu::{Context, GpuGraph};
    use specular_infer::nafnet::{self, NafConfig};
    use specular_infer::safetensors::SafeTensorsFile;
    use specular_infer::tiles::{Planar, Rows, Tiling};
    use specular_infer::{Feed, Tensor};

    let (Some(set), Some(models)) = (
        std::env::var_os("SPECULAR_DENOISE_ORACLE").map(std::path::PathBuf::from),
        std::env::var_os("SPECULAR_DENOISE_MODELS").map(std::path::PathBuf::from),
    ) else {
        eprintln!("SPECULAR_DENOISE_ORACLE / SPECULAR_DENOISE_MODELS not set: skipped");
        return;
    };
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let d = dev().unwrap();
    let ctx = Context::new(&d.device, &d.queue);
    // SPECULAR_DENOISE_NET: ffdnet (the default model), nafnet32 or nafnet64.
    let which = std::env::var("SPECULAR_DENOISE_NET").unwrap_or_else(|_| "ffdnet".into());
    let ffd = which == "ffdnet";
    let file = match which.as_str() {
        "nafnet64" => "NAFNet-SIDD-width64.f16.safetensors",
        "nafnet32" => "NAFNet-SIDD-width32.f16.safetensors",
        _ => "ffdnet_color_deepinv.f16.safetensors",
    };
    let mut st = SafeTensorsFile::open(&models.join(file)).expect("converted weights");
    let (g, tiling) = if ffd {
        let c = FfdConfig::from_header(&st.header).unwrap();
        let t = Tiling::FFDNET;
        (ffdnet::graph(&mut st, &c, (t.size, t.size)).unwrap(), t)
    } else {
        let c = NafConfig::from_header(&st.header).unwrap();
        let t = Tiling::for_width(c.width);
        (nafnet::graph(&mut st, &c, (t.size, t.size)).unwrap(), t)
    };
    let mut net = GpuGraph::new(&ctx, &g).unwrap();
    let sigma = |e: &ai::Encoding| Tensor::f32(&[1, 1, 1, 1], vec![e.sigma()]);
    let mut names: Vec<String> = std::fs::read_dir(set.join("lin"))
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".noisy.f32").map(str::to_owned))
        .collect();
    names.sort();
    // Per level: noisy, best classic, AI, AI half blended — PSNR and SSIM sums.
    let mut sums: std::collections::BTreeMap<String, [(f64, f64); 4]> = Default::default();
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    let mut ms = Vec::new();
    for id in &names {
        let lvl = id.rsplit('-').next().unwrap().to_owned();
        let (w, h, clean) = read(&set.join("lin").join(format!("{id}.clean.f32"))).unwrap();
        let (_, _, noisy) = read(&set.join("lin").join(format!("{id}.noisy.f32"))).unwrap();
        let (clean, img) = (to_rgba(w, h, &clean), to_rgba(w, h, &noisy));
        let (wu, hu) = (w as u32, h as u32);
        let model = level(&lvl);
        // The best classic setting (a small sweep, the true noise model).
        let mut best = (0.0, 0.0, Vec::new());
        for s in [0.3f32, 0.5, 0.7, 0.9, 1.0] {
            let mut f = denoise_fields(s, s);
            model.write(&mut f);
            let out = render(&mut en, &img, wu, hu, &edit_of(f));
            let p = psnr(&out, &clean);
            if p > best.0 {
                best = (p, s, out);
            }
        }
        // The network through the tiler, into a result, through the kernel.
        let enc = if ffd {
            ai::Encoding::gat(&model)
        } else {
            ai::Encoding::Srgb
        };
        let input = ai::net_input(w, h, &img, &enc);
        let key = [0x24u8; 32];
        let t0 = std::time::Instant::now();
        let mut res = AiResult::new(wu, hu, tiling.stride() as u32);
        let src = Planar { w, h, data: &input };
        let all = tiling.cells_of((w, h), 0, 0, w, h);
        tiling
            .run(
                &src,
                all,
                &mut |t: &Tensor| {
                    let sg = sigma(&enc);
                    let feeds: Vec<Feed> = if ffd {
                        vec![Feed::Tensor(t), Feed::Tensor(&sg)]
                    } else {
                        vec![Feed::Tensor(t)]
                    };
                    Ok(net.run(&feeds)?.remove(0))
                },
                &mut |r: Rows<'_>| {
                    res.put(
                        (r.x as u32, r.y as u32, r.w as u32, r.h as u32),
                        r.data,
                        &img,
                        &enc,
                    )
                },
                &mut |_, _| true,
            )
            .unwrap();
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        assert!(res.complete());
        // The result as stored, read on the CPU, against what the kernel shows.
        let stored: Vec<f32> = (0..hu)
            .flat_map(|y| (0..wu).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let p = res.pixel(x, y).unwrap();
                [p[0], p[1], p[2], 1.0]
            })
            .collect();
        ai::store()
            .lock()
            .unwrap()
            .update(&key, (wu, hu, res.cell), |x| *x = res.clone());
        let mut f = ai_fields(0.0, 0.0, 1.0, &key);
        model.write(&mut f);
        let ai_out = render(&mut en, &img, wu, hu, &edit_of(f));
        let mut f = ai_fields(best.1, best.1, 0.5, &key);
        model.write(&mut f);
        let half = render(&mut en, &img, wu, hu, &edit_of(f));
        ai::store().lock().unwrap().remove(&key);
        let (dk, _) = diff(&ai_out, &stored, 2e-3);
        assert!(
            dk < 2e-3,
            "{id}: the kernel shows the stored result (worst {dk})"
        );
        let row = [
            (psnr(&img, &clean), ssim(&img, &clean, w, h)),
            (best.0, ssim(&best.2, &clean, w, h)),
            (psnr(&ai_out, &clean), ssim(&ai_out, &clean, w, h)),
            (psnr(&half, &clean), ssim(&half, &clean, w, h)),
        ];
        eprintln!(
            "{id}: noisy {:.2} dB, classic {:.2} dB (at {:.1}), AI {:.2} dB, AI ½ {:.2} dB; SSIM {:.4} / {:.4} / {:.4} / {:.4}",
            row[0].0, row[1].0, best.1, row[2].0, row[3].0, row[0].1, row[1].1, row[2].1, row[3].1
        );
        let s = sums.entry(lvl.clone()).or_insert([(0.0, 0.0); 4]);
        for (acc, v) in s.iter_mut().zip(row) {
            acc.0 += v.0;
            acc.1 += v.1;
        }
        *counts.entry(lvl).or_default() += 1;
    }
    for (lvl, s) in &sums {
        let n = counts[lvl] as f64;
        eprintln!(
            "{lvl} ({n} images) mean PSNR noisy {:.2}, classic {:.2}, AI {:.2}, AI ½ {:.2} dB; SSIM {:.4}, {:.4}, {:.4}, {:.4}",
            s[0].0 / n,
            s[1].0 / n,
            s[2].0 / n,
            s[3].0 / n,
            s[0].1 / n,
            s[1].1 / n,
            s[2].1 / n,
            s[3].1 / n
        );
        // Half the AI result beats the best classic setting on average.
        if ffd {
            assert!(
                s[3].0 > s[1].0,
                "{lvl}: AI ½ {} vs classic {} dB",
                s[3].0 / n,
                s[1].0 / n
            );
        }
    }
    ms.sort_by(f64::total_cmp);
    if !ms.is_empty() {
        eprintln!(
            "{which} tiled 512×384 on the GPU: median {:.0} ms",
            ms[ms.len() / 2]
        );
    }
}
