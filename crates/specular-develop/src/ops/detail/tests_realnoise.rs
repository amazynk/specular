//! AI denoise on realistic noise: clean photos (the oracle set's
//! `lin/*.clean.f32`, CC0/CC BY from `tools/corpus/fetch-denoise.py`) mosaicked into an RGGB
//! sensor, Poisson–Gaussian noise added on the mosaic at ISO-like levels, developed by our
//! front end (demosaic), then denoised by the classic and the AI mode. Scored against the
//! noise-free mosaic developed the same way (so the demosaic's own error is not counted).
//! The demosaic correlates the noise; FFDNet expects white noise, so the level it is told
//! is also tried scaled up.
//!
//! Runs with `SPECULAR_DENOISE_CLEAN` (a folder holding `*.clean.f32`) and
//! `SPECULAR_DENOISE_MODELS` (holding `ffdnet_color_deepinv.f16.safetensors`).

use specular_raw::sensor::{BlackLevels, Layout, SensorMeta};
use specular_raw::{MatrixSource, Rect, SensorData, WbSource};

use super::ai::{self, AiResult};
use super::noise::NoiseModel;
use super::tests_ai_denoise::{ai_fields, edit_of, engine, psnr, read, render, ssim};
use crate::gpu::pool::Budget;
use crate::model::{Edit, Registry, SourceKey, Value};
use crate::preview::{Developer, Input};
use crate::testgpu::{dev, lock};

/// Mosaic-domain noise levels (normalised sensor units: variance `a·x + b`).
pub(super) const LEVELS: [(&str, f64, f64); 3] = [
    ("iso400", 1.0e-4, 4.0e-8),
    ("iso1600", 4.0e-4, 6.0e-7),
    ("iso6400", 1.6e-3, 1.0e-5),
];

/// Normal deviates (xorshift64* and Box–Muller).
struct Gauss(u64);

impl Gauss {
    fn u(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        let (a, b) = (self.u(), self.u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// An RGGB sensor of planar linear camera RGB `v` (`w`×`h`, 1 = white) with noise of
/// variance `a·x + b` per sample (none when `a = b = 0`).
pub(super) fn sensor(w: usize, h: usize, v: &[f32], a: f64, b: f64, seed: u64) -> SensorData {
    let (black, white) = (256.0f64, 16383.0f64);
    let n = w * h;
    let mut g = Gauss(seed | 1);
    let mut samples = Vec::with_capacity(n);
    for y in 0..h {
        for x in 0..w {
            let c = match (y & 1, x & 1) {
                (0, 0) => 0,
                (1, 1) => 2,
                _ => 1,
            };
            let x0 = f64::from(v[c * n + y * w + x]).clamp(0.0, 1.0) * 0.8;
            let s = (a * x0 + b).max(0.0).sqrt();
            let xv = if s > 0.0 { x0 + s * g.n() } else { x0 };
            let raw = black + xv * (white - black);
            samples.push(raw.round().clamp(0.0, white) as u16);
        }
    }
    SensorData {
        width: w as u32,
        height: h as u32,
        cpp: 1,
        samples,
        float: None,
        crop: Rect {
            x: 0,
            y: 0,
            w: w as u32,
            h: h as u32,
        },
        meta: SensorMeta {
            layout: Layout::Bayer([0, 1, 1, 2]),
            black: BlackLevels::simple([black as f32; 4], 1),
            white: white as f32,
            clip_white: white as f32,
            linearization: None,
            opcodes: Default::default(),
            opcodes_unknown: Vec::new(),
            wb: [1.0, 1.0, 1.0],
            wb_source: WbSource::AsShot,
            rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            matrix_source: MatrixSource::Generic,
            dng: None,
            profile: None,
            orientation: 1,
            lens: None,
            default_scale: None,
            truncated: false,
        },
    }
}

/// The developed (demosaicked) picture before denoise, linear Rec.2020 RGBA, and the noise
/// model the preview carries to it.
fn develop(
    d: &crate::testgpu::Dev,
    s: SensorData,
    edit: &Edit,
    long: u32,
) -> (u32, u32, Vec<f32>, Option<NoiseModel>) {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.highlight_stats = false;
    dv.set_input(Input::Raw(Box::new(s))).unwrap();
    dv.prepare(edit, long).unwrap();
    let noise = dv.noise_model();
    let (w, h, px) = dv
        .scene_pixels(edit, crate::model::ModuleKind::DENOISE)
        .unwrap();
    (w, h, px, noise)
}

/// Correlation of horizontally neighbouring noise values (green), `noisy − clean`.
fn neighbour_correlation(w: usize, h: usize, noisy: &[f32], clean: &[f32]) -> f64 {
    let d = |x: usize, y: usize| f64::from(noisy[(y * w + x) * 4 + 1] - clean[(y * w + x) * 4 + 1]);
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for y in 4..h - 4 {
        for x in 4..w - 5 {
            sxy += d(x, y) * d(x + 1, y);
            sxx += d(x, y) * d(x, y);
        }
    }
    sxy / sxx.max(1e-30)
}

/// FFDNet over the whole picture through the tiler into a result.
fn run_ai(
    net: &mut specular_infer::gpu::GpuGraph,
    tiling: &specular_infer::tiles::Tiling,
    img: &[f32],
    enc: &ai::Encoding,
    (w, h): (usize, usize),
    boost: f32,
) -> AiResult {
    use specular_infer::tiles::{Planar, Rows};
    use specular_infer::{Feed, Tensor};
    let input = ai::net_input(w, h, img, enc);
    let mut res = AiResult::new(w as u32, h as u32, tiling.stride() as u32);
    let src = Planar { w, h, data: &input };
    let all = tiling.cells_of((w, h), 0, 0, w, h);
    tiling
        .run(
            &src,
            all,
            &mut |t: &Tensor| {
                let sg = Tensor::f32(&[1, 1, 1, 1], vec![enc.sigma() * boost]);
                Ok(net.run(&[Feed::Tensor(t), Feed::Tensor(&sg)])?.remove(0))
            },
            &mut |r: Rows<'_>| {
                res.put(
                    (r.x as u32, r.y as u32, r.w as u32, r.h as u32),
                    r.data,
                    img,
                    enc,
                )
            },
            &mut |_, _| true,
        )
        .unwrap();
    res
}

/// A result's pixels as stored (RGBA).
fn stored_rgba(res: &AiResult) -> Vec<f32> {
    (0..res.h)
        .flat_map(|y| (0..res.w).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let p = res.pixel(x, y).unwrap();
            [p[0], p[1], p[2], 1.0]
        })
        .collect()
}

#[test]
fn ai_denoise_on_demosaicked_noise() {
    use specular_infer::ffdnet::{self, FfdConfig};
    use specular_infer::gpu::{Context, GpuGraph};
    use specular_infer::safetensors::SafeTensorsFile;
    use specular_infer::tiles::Tiling;

    let (Some(set), Some(models)) = (
        std::env::var_os("SPECULAR_DENOISE_CLEAN").map(std::path::PathBuf::from),
        std::env::var_os("SPECULAR_DENOISE_MODELS").map(std::path::PathBuf::from),
    ) else {
        eprintln!("SPECULAR_DENOISE_CLEAN / SPECULAR_DENOISE_MODELS not set: skipped");
        return;
    };
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let d = dev().unwrap();
    let ctx = Context::new(&d.device, &d.queue);
    let mut st = SafeTensorsFile::open(&models.join("ffdnet_color_deepinv.f16.safetensors"))
        .expect("converted weights");
    let c = FfdConfig::from_header(&st.header).unwrap();
    let tiling = Tiling::FFDNET;
    let g = ffdnet::graph(&mut st, &c, (tiling.size, tiling.size)).unwrap();
    let mut net = GpuGraph::new(&ctx, &g).unwrap();
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&set)
        .unwrap()
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.to_string_lossy().ends_with(".clean.f32"))
        .collect();
    names.sort();
    if let Some(n) = std::env::var("SPECULAR_DENOISE_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        names.truncate(n);
    }
    // The told level's factors tried.
    const BOOST: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 2.5];
    let plain = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    // Per level: noisy, classic, AI per boost, AI ½ per boost — PSNR/SSIM sums.
    type Row = Vec<(f64, f64)>;
    let mut sums: std::collections::BTreeMap<&str, (Row, usize, f64)> = Default::default();
    for (i, path) in names.iter().enumerate() {
        let (w, h, v) = read(path).unwrap();
        // Even size (whole CFA periods).
        let (w2, h2) = (w & !1, h & !1);
        let n = w * h;
        let mut cut = vec![0f32; 3 * w2 * h2];
        for c in 0..3 {
            for y in 0..h2 {
                for x in 0..w2 {
                    cut[c * w2 * h2 + y * w2 + x] = v[c * n + y * w + x];
                }
            }
        }
        let long = w2.max(h2) as u32;
        let (_, _, clean, _) = develop(d, sensor(w2, h2, &cut, 0.0, 0.0, 1), &plain, long);
        for (li, &(lvl, a, b)) in LEVELS.iter().enumerate() {
            let seed = (i * 7 + li) as u64 + 11;
            let (wu, hu, img, model) = develop(d, sensor(w2, h2, &cut, a, b, seed), &plain, long);
            let model = model.expect("a noise model");
            let rho = neighbour_correlation(w2, h2, &img, &clean);
            // The measured noise against the model, around mid grey (green).
            let (mut se, mut sm, mut k) = (0.0f64, 0.0f64, 0.0f64);
            for (p, q) in img.chunks_exact(4).zip(clean.chunks_exact(4)) {
                if (0.15..0.25).contains(&q[1]) {
                    se += f64::from(p[1] - q[1]).powi(2);
                    sm += f64::from(model.a[1] * q[1] + model.b[1]);
                    k += 1.0;
                }
            }
            eprintln!(
                "  {lvl}: model a {:?} b {:?}; green variance at 0.15-0.25 measured {:.3e}, model {:.3e}",
                model.a,
                model.b,
                se / k.max(1.0),
                sm / k.max(1.0)
            );
            let mut row: Row = vec![(psnr(&img, &clean), ssim(&img, &clean, w2, h2))];
            // The best classic setting.
            let mut best = (0.0, 0.0, Vec::new());
            for s in [0.3f32, 0.5, 0.7, 0.9, 1.0] {
                let mut f = super::denoise::Denoise::default_fields()
                    .with("luminance", Value::Float(s))
                    .with("colour", Value::Float(s));
                model.write(&mut f);
                let out = render(&mut en, &img, wu, hu, &edit_of(f));
                let p = psnr(&out, &clean);
                if p > best.0 {
                    best = (p, s, out);
                }
            }
            row.push((best.0, ssim(&best.2, &clean, w2, h2)));
            let enc = ai::Encoding::gat(&model);
            let mut halves = Vec::new();
            for (bi, boost) in BOOST.iter().enumerate() {
                let key = [0x50 + bi as u8; 32];
                let res = run_ai(&mut net, &tiling, &img, &enc, (w2, h2), *boost);
                if bi == 0 {
                    let stored = stored_rgba(&res);
                    // Control: the same noise model as white noise on the clean picture.
                    let mut g = Gauss(seed ^ 0x77);
                    let white: Vec<f32> = clean
                        .chunks_exact(4)
                        .flat_map(|p| {
                            let mut q = [p[0], p[1], p[2], p[3]];
                            for c in 0..3 {
                                let v = f64::from(model.a[c] * p[c].max(0.0) + model.b[c]);
                                q[c] += (v.sqrt() * g.n()) as f32;
                            }
                            q
                        })
                        .collect();
                    let wr = run_ai(&mut net, &tiling, &white, &enc, (w2, h2), 1.0);
                    let ws = stored_rgba(&wr);
                    eprintln!(
                        "  {lvl}: AI result as stored {:.2} dB; white-noise control: noisy {:.2}, AI {:.2} dB",
                        psnr(&stored, &clean),
                        psnr(&white, &clean),
                        psnr(&ws, &clean)
                    );
                }
                ai::store()
                    .lock()
                    .unwrap()
                    .update(&key, (wu, hu, res.cell), |x| *x = res.clone());
                let mut f = ai_fields(0.0, 0.0, 1.0, &key);
                model.write(&mut f);
                let out = render(&mut en, &img, wu, hu, &edit_of(f));
                let mut f = ai_fields(best.1, best.1, 0.5, &key);
                model.write(&mut f);
                let half = render(&mut en, &img, wu, hu, &edit_of(f));
                ai::store().lock().unwrap().remove(&key);
                row.push((psnr(&out, &clean), ssim(&out, &clean, w2, h2)));
                halves.push((psnr(&half, &clean), ssim(&half, &clean, w2, h2)));
            }
            row.extend(halves);
            let name = path.file_name().unwrap().to_string_lossy();
            eprintln!(
                "{name} {lvl}: ρ {rho:.3}; noisy {:.2}, classic {:.2} (at {:.1}), AI ×{BOOST:?} {:?}, AI ½ {:?} dB",
                row[0].0,
                row[1].0,
                best.1,
                row[2..2 + BOOST.len()]
                    .iter()
                    .map(|r| (r.0 * 100.0).round() / 100.0)
                    .collect::<Vec<_>>(),
                row[2 + BOOST.len()..]
                    .iter()
                    .map(|r| (r.0 * 100.0).round() / 100.0)
                    .collect::<Vec<_>>(),
            );
            let e = sums
                .entry(lvl)
                .or_insert_with(|| (vec![(0.0, 0.0); row.len()], 0, 0.0));
            for (acc, v) in e.0.iter_mut().zip(&row) {
                acc.0 += v.0;
                acc.1 += v.1;
            }
            e.1 += 1;
            e.2 += rho;
        }
    }
    for (lvl, (s, n, rho)) in &sums {
        let k = *n as f64;
        let f = |r: &(f64, f64)| format!("{:.2} dB / {:.4}", r.0 / k, r.1 / k);
        eprintln!(
            "{lvl} ({n} images, mean neighbour correlation {:.3}): noisy {}, classic {}",
            rho / k,
            f(&s[0]),
            f(&s[1])
        );
        for (bi, boost) in BOOST.iter().enumerate() {
            eprintln!(
                "  told ×{boost}: AI {}, AI ½ {}",
                f(&s[2 + bi]),
                f(&s[2 + BOOST.len() + bi])
            );
        }
    }
}
