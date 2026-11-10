//! The tone ops: the guided filter and the fast local Laplacian filter against textbook
//! CPU versions, GPU = CPU, halos on step edges, tiles, scale stability and the engine's
//! no-new-pipeline rule.

use std::sync::Arc;

use super::cpu::{self as tcpu, Img};
use super::equalizer::{self, ToneEq};
use super::local_contrast::{self, LocalContrast};
use super::{curves, kernels};
use crate::gpu::pool::Budget;
use crate::gpu::{Engine, Request, Source};
use crate::graph::tile::Tiling;
use crate::graph::{Graph, NodeKind, Op, OpRegistry, Precision, Program, Roi};
use crate::model::{Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value};
use crate::ops::cpu_ref;
use crate::ops::output::OutputSpace;
use crate::testgpu::{dev, lock};

const GIB: u64 = 1 << 30;

fn engine() -> Option<Engine> {
    let d = dev()?;
    Some(Engine::new(&d.device, &d.queue, Budget::fixed(2 * GIB)))
}

/// A photo-like scene, scene-linear Rec.2020: sky gradient, darker ground, a bright window
/// with sharp edges, coloured patches and fine texture, values from about −10 to +2 EV.
fn photo(w: u32, h: u32) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, t) = (x as f32 / w as f32, y as f32 / h as f32);
            let n = (x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B))
                .wrapping_mul(0xC2B2_AE35);
            let noise = ((n >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.3;
            // Texture with a period of a few percent of the width (scale-free).
            let tex = 0.25 * ((u * 90.0).sin() * (t * 70.0).cos());
            let mut ev = if t < 0.45 {
                -1.0 - 1.5 * t
            } else {
                -5.0 - 4.0 * (t - 0.45) + tex
            };
            if (0.55..0.75).contains(&u) && (0.15..0.4).contains(&t) {
                ev = 1.5;
            }
            if (0.1..0.3).contains(&u) && (0.6..0.85).contains(&t) {
                ev = -8.5 + tex;
            }
            let y_lin = (ev + noise * 0.2).exp2();
            let hue = u * 6.0 + t * 3.0;
            let sat = 0.35;
            let c = [
                y_lin * (1.0 + sat * hue.cos()),
                y_lin * (1.0 + sat * (hue - 2.094).cos()),
                y_lin * (1.0 + sat * (hue + 2.094).cos()),
            ];
            v.extend_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
    }
    v
}

fn module(kind: ModuleKind, f: Fields) -> ModuleInstance {
    let mut m = ModuleInstance::new(Registry::builtin(), kind, 0);
    m.params = Params::from_fields(kind, f);
    m
}

fn edit(mods: Vec<ModuleInstance>) -> Edit {
    let mut e = Edit::empty(SourceKey::default());
    for m in mods {
        e.insert(Registry::builtin(), m);
    }
    e
}

fn te(f: Fields) -> ModuleInstance {
    module(ModuleKind::TONE_EQUALIZER, f)
}

fn lc(texture: f32, clarity: f32) -> ModuleInstance {
    module(
        ModuleKind::LOCAL_CONTRAST,
        Fields::new()
            .with("texture", Value::Float(texture))
            .with("clarity", Value::Float(clarity)),
    )
}

fn set(e: &mut Edit, kind: ModuleKind, name: &str, v: Value) {
    let m = e.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

fn render(en: &mut Engine, g: &Graph, src: &Source, req: &Request) -> Vec<f32> {
    let r = en.render(g, src, req).expect("render");
    let px = en.read(&r).expect("read");
    en.recycle(r);
    px
}

fn f32_whole(w: u32, h: u32) -> Request {
    Request {
        precision: Precision::F32,
        checkpoints: false,
        ..Request::new(Roi::full(w, h))
    }
}

fn graph(e: &Edit, ops: &OpRegistry) -> Graph {
    Graph::build(e, ops, None).expect("graph")
}

fn log_y(c: &[f32]) -> f32 {
    tcpu::dot([c[0], c[1], c[2]]).max(1e-12).log2()
}

/// The textbook guided filter (He, Sun and Tang 2013) in f64: windows of radius `r`
/// clipped at the edges, means over the pixels inside.
fn guided_textbook(i: &[f64], p: &[f64], w: usize, h: usize, r: usize, eps: f64) -> Vec<f64> {
    let mean = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0f64; w * h];
        for y in 0..h {
            for x in 0..w {
                let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
                let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
                let mut s = 0.0;
                for yy in y0..=y1 {
                    for xx in x0..=x1 {
                        s += v[yy * w + xx];
                    }
                }
                out[y * w + x] = s / ((y1 - y0 + 1) * (x1 - x0 + 1)) as f64;
            }
        }
        out
    };
    let mi = mean(i);
    let mp = mean(p);
    let ip: Vec<f64> = i.iter().zip(p).map(|(a, b)| a * b).collect();
    let ii: Vec<f64> = i.iter().map(|a| a * a).collect();
    let (mip, mii) = (mean(&ip), mean(&ii));
    let a: Vec<f64> = (0..w * h)
        .map(|k| (mip[k] - mi[k] * mp[k]) / ((mii[k] - mi[k] * mi[k]).max(0.0) + eps))
        .collect();
    let b: Vec<f64> = (0..w * h).map(|k| mp[k] - a[k] * mi[k]).collect();
    let (ma, mb) = (mean(&a), mean(&b));
    (0..w * h).map(|k| ma[k] * i[k] + mb[k]).collect()
}

/// A probe: the tone equalizer's mask (Ā·I + B̄) in red, `I` in green.
struct MaskProbe;
const PROBE: ModuleKind = ModuleKind(0xFE20);
impl Op for MaskProbe {
    fn name(&self) -> &'static str {
        "mask_probe_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Global
    }
    fn param_words(&self) -> u32 {
        0
    }
    fn write_params(&self, _: &Params, _: f32, _: &mut [u32]) {}
    fn helpers(&self) -> &'static [&'static str] {
        &[kernels::COMMON_WGSL, kernels::GRID_WGSL]
    }
    fn has_stats(&self, _: &Params) -> bool {
        true
    }
    fn stats(&self, p: &Params, w: u32, h: u32, _: f32) -> Option<Program> {
        Some(equalizer::stats_program(
            &ToneEq::from_fields(&p.to_fields()),
            w,
            h,
        ))
    }
    fn wgsl(&self) -> &'static str {
        "    let n = u32(st(0u)) * u32(st(1u));
    let l = log2(max(dot(c.rgb, DV_LUMA), 1.52587890625e-5));
    return vec4<f32>(dv_grid(16u, px, py) * l + dv_grid(16u + n, px, py), l, 0.0, 1.0);"
    }
}

#[test]
fn guided_filter_matches_the_textbook_reference() {
    let (w, h) = (180u32, 120u32);
    let img = photo(w, h);
    let f = Fields::new()
        .with("smoothing", Value::Float(0.04))
        .with("mask_edges", Value::Float(0.05));
    let t = ToneEq::from_fields(&f);
    let r = t.radius(w, h);
    assert_eq!(r, 7);
    // The program on the CPU.
    let prog = equalizer::stats_program(&t, w, h);
    let s = tcpu::run_program(&prog, Img { w, h, px: &img }, &[], Roi::full(w, h));
    let n = (w * h) as usize;
    let i: Vec<f64> = img
        .chunks_exact(4)
        .map(|c| f64::from(log_y(c).max(-16.0)))
        .collect();
    let want = guided_textbook(&i, &i, w as usize, h as usize, r as usize, 0.05);
    let mut worst = 0f64;
    for k in 0..n {
        let q = f64::from(s[16 + k]) * i[k] + f64::from(s[16 + n + k]);
        worst = worst.max((q - want[k]).abs());
    }
    eprintln!("guided filter: program (CPU) vs textbook, worst {worst:e} EV");
    assert!(worst < 1e-3, "{worst}");
    // And on the GPU, through a kernel reading the statistics.
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let mut ops = OpRegistry::builtin().clone();
    ops.register(PROBE, 1, Arc::new(MaskProbe));
    let mut e = Edit::empty(SourceKey::default());
    e.modules.push(ModuleInstance {
        kind: PROBE,
        version: 1,
        instance: 0,
        label: None,
        enabled: true,
        params: Params::Other(PROBE, f),
        blend: None,
    });
    let src = en.upload(w, h, &img).expect("upload");
    let out = render(&mut en, &graph(&e, &ops), &src, &f32_whole(w, h));
    let mut worst = 0f64;
    let mut bad = 0;
    for k in 0..n {
        let d = (f64::from(out[k * 4]) - want[k]).abs();
        worst = worst.max(d);
        bad += usize::from(d > 1e-3);
    }
    eprintln!("guided filter: GPU vs textbook, worst {worst:e} EV, {bad} of {n} over 1e-3");
    assert_eq!(bad, 0);
}

#[test]
fn fast_guided_filter_matches_the_textbook_reference() {
    // A large radius: the coefficients are taken on a grid subsampled by s (He and Sun
    // 2015), upsampled bilinearly and applied to the full-resolution guide.
    let (w, h) = (300u32, 200u32);
    let img = photo(w, h);
    let f = Fields::new()
        .with("smoothing", Value::Float(0.1))
        .with("mask_edges", Value::Float(0.05));
    let t = ToneEq::from_fields(&f);
    let (s, r) = t.subsample(w, h);
    assert_eq!((t.radius(w, h), s, r), (30, 7, 4));
    let (cw, ch) = (w.div_ceil(s) as usize, h.div_ceil(s) as usize);
    let i: Vec<f64> = img
        .chunks_exact(4)
        .map(|c| f64::from(log_y(c).max(-16.0)))
        .collect();
    // Block means, the guided filter's coefficients there, bilinear back.
    let mut ic = vec![0f64; cw * ch];
    for y in 0..ch {
        for x in 0..cw {
            let (x0, y0) = (x * s as usize, y * s as usize);
            let (x1, y1) = (
                (x0 + s as usize).min(w as usize),
                (y0 + s as usize).min(h as usize),
            );
            let mut sum = 0.0;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    sum += i[yy * w as usize + xx];
                }
            }
            ic[y * cw + x] = sum / ((x1 - x0) * (y1 - y0)) as f64;
        }
    }
    // The coefficients on the coarse grid (they give the textbook filter there).
    let qc = guided_textbook(&ic, &ic, cw, ch, r as usize, 0.05);
    let (ma, mb) = guided_coeffs(&ic, cw, ch, r as usize, 0.05);
    for k in 0..cw * ch {
        assert!((ma[k] * ic[k] + mb[k] - qc[k]).abs() < 1e-9);
    }
    let bil = |v: &[f64], x: u32, y: u32| -> f64 {
        let gx = ((x as f64 + 0.5) * cw as f64 / w as f64 - 0.5).clamp(0.0, (cw - 1) as f64);
        let gy = ((y as f64 + 0.5) * ch as f64 / h as f64 - 0.5).clamp(0.0, (ch - 1) as f64);
        let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(cw - 1), (y0 + 1).min(ch - 1));
        let (fx, fy) = (gx - x0 as f64, gy - y0 as f64);
        let top = v[y0 * cw + x0] * (1.0 - fx) + v[y0 * cw + x1] * fx;
        let bot = v[y1 * cw + x0] * (1.0 - fx) + v[y1 * cw + x1] * fx;
        top * (1.0 - fy) + bot * fy
    };
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let mut ops = OpRegistry::builtin().clone();
    ops.register(PROBE, 1, Arc::new(MaskProbe));
    let mut e = Edit::empty(SourceKey::default());
    e.modules.push(ModuleInstance {
        kind: PROBE,
        version: 1,
        instance: 0,
        label: None,
        enabled: true,
        params: Params::Other(PROBE, f),
        blend: None,
    });
    let src = en.upload(w, h, &img).expect("upload");
    let out = render(&mut en, &graph(&e, &ops), &src, &f32_whole(w, h));
    let mut worst = 0f64;
    for y in 0..h {
        for x in 0..w {
            let k = (y * w + x) as usize;
            let want = bil(&ma, x, y) * i[k] + bil(&mb, x, y);
            worst = worst.max((f64::from(out[k * 4]) - want).abs());
        }
    }
    eprintln!("fast guided filter (s = {s}): GPU vs textbook, worst {worst:e} EV");
    assert!(worst < 1e-3, "{worst}");
}

/// The guided filter's mean coefficients Ā, B̄ (self-guided), textbook, in f64.
fn guided_coeffs(i: &[f64], w: usize, h: usize, r: usize, eps: f64) -> (Vec<f64>, Vec<f64>) {
    let mean = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0f64; w * h];
        for y in 0..h {
            for x in 0..w {
                let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
                let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
                let mut s = 0.0;
                for yy in y0..=y1 {
                    for xx in x0..=x1 {
                        s += v[yy * w + xx];
                    }
                }
                out[y * w + x] = s / ((y1 - y0 + 1) * (x1 - x0 + 1)) as f64;
            }
        }
        out
    };
    let mi = mean(i);
    let ii: Vec<f64> = i.iter().map(|a| a * a).collect();
    let mii = mean(&ii);
    let a: Vec<f64> = (0..w * h)
        .map(|k| {
            let var = (mii[k] - mi[k] * mi[k]).max(0.0);
            var / (var + eps)
        })
        .collect();
    let b: Vec<f64> = (0..w * h).map(|k| mi[k] - a[k] * mi[k]).collect();
    (mean(&a), mean(&b))
}

#[test]
fn local_laplacian_matches_the_textbook_reference() {
    // Aubry et al.'s fast local Laplacian, written plainly in f64: the full Gaussian
    // pyramid of every remapped image, Laplacians interpolated by G_l, collapsed.
    let (w, h) = (96u32, 72u32);
    let img = photo(w, h);
    let p = LocalContrast {
        texture: 0.8,
        ..LocalContrast::default()
    };
    // The γ range from the statistics, as the engine takes it.
    let st_prog = local_contrast::stats_program(&p, w, h);
    let st = tcpu::run_program(&st_prog, Img { w, h, px: &img }, &[], Roi::full(w, h));
    let levels = 3;
    let prog = local_contrast::texture_program(&p, Roi::full(w, h), levels);
    let s = tcpu::run_program(&prog, Img { w, h, px: &img }, &st, Roi::full(w, h));
    let (g0, dg) = (f64::from(st[2]), f64::from(st[3]));
    let l0: Vec<f64> = img
        .chunks_exact(4)
        .map(|c| f64::from(log_y(c).max(-16.0)))
        .collect();
    let want = fast_llf(
        &l0,
        w as usize,
        h as usize,
        levels as usize,
        local_contrast::SAMPLES as usize,
        g0,
        dg,
        0.8,
        1.0,
    );
    let n = (w * h) as usize;
    // The program stops at level 1; level 0 is collapsed by the kernel, as here.
    let words = {
        let mut u = vec![0u32; 6];
        let lcp = Params::Other(
            ModuleKind::LOCAL_CONTRAST,
            Fields::new().with("texture", Value::Float(0.8)),
        );
        local_contrast::LocalContrastOp.write_params(&lcp, 1.0, &mut u);
        u
    };
    let mut worst = 0f64;
    for k in 0..n {
        let (x, y) = ((k as u32 % w) as i32, (k as u32 / w) as i32);
        let c = [img[k * 4], img[k * 4 + 1], img[k * 4 + 2], img[k * 4 + 3]];
        let o = tcpu::local_contrast(&words, c, &st, &s, Roi::full(w, h), x, y, w, h);
        // The kernel scales RGB by 2^(out₀ − L₀): out₀ from the luminance it gives.
        let out0 = l0[k] + f64::from((o[1] / c[1]).log2());
        worst = worst.max((out0 - want[k]).abs());
    }
    eprintln!("local Laplacian: program (CPU) vs textbook, worst {worst:e} EV");
    assert!(worst < 1e-3, "{worst}");
    // Small details grew (edges over σ are left alone).
    let small: Vec<usize> = (1..n - 1)
        .filter(|&k| (l0[k + 1] - l0[k - 1]).abs() < 0.3)
        .collect();
    let dev = |v: &[f64]| -> f64 {
        small
            .iter()
            .map(|&k| (v[k] - 0.5 * (v[k - 1] + v[k + 1])).abs())
            .sum::<f64>()
    };
    let gain = dev(&want) / dev(&l0);
    eprintln!("local Laplacian: small detail × {gain:.2}");
    assert!(gain > 1.3, "{gain}");
}

fn reduce_f64(v: &[f64], w: usize, h: usize) -> (Vec<f64>, usize, usize) {
    let (wd, hd) = (w.div_ceil(2), h.div_ceil(2));
    let bw = [0.0625, 0.25, 0.375, 0.25, 0.0625];
    let at = |x: i64, y: i64| {
        v[y.clamp(0, h as i64 - 1) as usize * w + x.clamp(0, w as i64 - 1) as usize]
    };
    let mut out = vec![0f64; wd * hd];
    for y in 0..hd {
        for x in 0..wd {
            let mut s = 0.0;
            for b in 0..5 {
                for a in 0..5 {
                    s += bw[a]
                        * bw[b]
                        * at(2 * x as i64 + a as i64 - 2, 2 * y as i64 + b as i64 - 2);
                }
            }
            out[y * wd + x] = s;
        }
    }
    (out, wd, hd)
}

fn expand_f64(v: &[f64], w1: usize, h1: usize, w: usize, h: usize) -> Vec<f64> {
    let at = |x: i64, y: i64| {
        v[y.clamp(0, h1 as i64 - 1) as usize * w1 + x.clamp(0, w1 as i64 - 1) as usize]
    };
    // up[i] = 2·Σ_k w(k)·X[(i − k)/2] over even i − k, in each axis.
    let wt = [0.0625, 0.25, 0.375, 0.25, 0.0625];
    let mut out = vec![0f64; w * h];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let mut s = 0.0;
            for kb in -2i64..=2 {
                if (y - kb) % 2 != 0 {
                    continue;
                }
                for ka in -2i64..=2 {
                    if (x - ka) % 2 != 0 {
                        continue;
                    }
                    s += 4.0
                        * wt[(ka + 2) as usize]
                        * wt[(kb + 2) as usize]
                        * at((x - ka) / 2, (y - kb) / 2);
                }
            }
            out[y as usize * w + x as usize] = s;
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn fast_llf(
    l0: &[f64],
    w: usize,
    h: usize,
    levels: usize,
    k: usize,
    g0: f64,
    dg: f64,
    amt: f64,
    sig: f64,
) -> Vec<f64> {
    let pyr = |v: &[f64]| {
        let mut p = vec![(v.to_vec(), w, h)];
        for _ in 0..levels {
            let (pv, pw, ph) = p.last().unwrap();
            p.push(reduce_f64(pv, *pw, *ph));
        }
        p
    };
    let lap = |p: &Vec<(Vec<f64>, usize, usize)>, l: usize| -> Vec<f64> {
        let (v, lw, lh) = &p[l];
        let (v1, w1, h1) = &p[l + 1];
        let e = expand_f64(v1, *w1, *h1, *lw, *lh);
        v.iter().zip(e).map(|(a, b)| a - b).collect()
    };
    let g = pyr(l0);
    let rp: Vec<_> = (0..k)
        .map(|j| {
            let gm = g0 + j as f64 * dg;
            let r: Vec<f64> = l0
                .iter()
                .map(|&v| {
                    let d = v - gm;
                    let u = d / (2.0 * sig);
                    if u.abs() >= 1.0 {
                        v
                    } else {
                        v + amt * d * (1.0 - u * u).powi(2)
                    }
                })
                .collect();
            pyr(&r)
        })
        .collect();
    let mut out = g[levels].0.clone();
    for l in (0..levels).rev() {
        let (gv, lw, lh) = &g[l];
        let laps: Vec<Vec<f64>> = rp.iter().map(|p| lap(p, l)).collect();
        let (w1, h1) = (g[l + 1].1, g[l + 1].2);
        let e = expand_f64(&out, w1, h1, *lw, *lh);
        out = (0..lw * lh)
            .map(|q| {
                let jf = ((gv[q] - g0) / dg).clamp(0.0, (k - 1) as f64);
                let j0 = (jf.floor() as usize).min(k - 2);
                let fr = jf - j0 as f64;
                laps[j0][q] * (1.0 - fr) + laps[j0 + 1][q] * fr + e[q]
            })
            .collect();
    }
    out
}

/// Worst relative difference |a − b| / max(1, |b|) and the count over `tol`.
fn compare(a: &[f32], b: &[f32], tol: f32) -> (f32, usize) {
    let mut worst = 0f32;
    let mut bad = 0;
    for (x, y) in a.iter().zip(b) {
        let e = (x - y).abs() / y.abs().max(1.0);
        worst = worst.max(e);
        bad += usize::from(e > tol);
    }
    (worst, bad)
}

#[test]
fn gpu_matches_the_cpu_reference() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let ops = OpRegistry::builtin();
    // Small enough that the grid is the image, and larger, so the grid is reduced.
    for (w, h) in [(160u32, 110u32), (1500, 900)] {
        let img = photo(w, h);
        let src = en.upload(w, h, &img).expect("upload");
        let cases = [
            (
                "tone equalizer",
                edit(vec![te(Fields::new()
                    .with("shadows_ev", Value::Float(1.2))
                    .with("highlights_ev", Value::Float(-0.7))
                    .with(
                        "zones",
                        Value::Floats(vec![0.3, 0.2, 0.0, -0.1, 0.4, 0.0, 0.0, -0.3, -0.5]),
                    )
                    .with("mask_auto", Value::Bool(true)))]),
            ),
            ("texture", edit(vec![lc(0.7, 0.0)])),
            ("clarity", edit(vec![lc(0.0, 0.6)])),
            (
                "curves",
                edit(vec![module(
                    ModuleKind::CURVES,
                    Fields::new()
                        .with(
                            "points",
                            Value::Floats(vec![0.0, 0.0, 0.25, 0.18, 0.7, 0.8, 1.0, 1.0]),
                        )
                        .with("points_b", Value::Floats(vec![0.0, 0.05, 1.0, 0.95]))
                        .with("levels_black", Value::Float(0.02))
                        .with("levels_grey", Value::Float(0.45)),
                )]),
            ),
            (
                "auto exposure",
                edit(vec![module(
                    ModuleKind::EXPOSURE,
                    Fields::new()
                        .with("auto", Value::Bool(true))
                        .with("exposure_ev", Value::Float(0.3)),
                )]),
            ),
        ];
        for (label, e) in cases {
            let g = graph(&e, ops);
            let gpu = render(&mut en, &g, &src, &f32_whole(w, h));
            let cpu = cpu_ref::run_image(&g, w, h, &img);
            let (worst, bad) = compare(&gpu, &cpu, 2e-4);
            eprintln!("{label} {w}×{h}: GPU vs CPU worst relative {worst:e}");
            assert_eq!(bad, 0, "{label} {w}×{h}: worst {worst:e}");
        }
    }
}

/// log2 of the luminance gain per column, averaged over rows.
fn column_gain(inp: &[f32], out: &[f32], w: u32, h: u32) -> Vec<f32> {
    (0..w as usize)
        .map(|x| {
            (0..h as usize)
                .map(|y| {
                    let i = (y * w as usize + x) * 4;
                    log_y(&out[i..i + 4]) - log_y(&inp[i..i + 4])
                })
                .sum::<f32>()
                / h as f32
        })
        .collect()
}

/// A step edge at x = w/2: dark (−6.5 EV) left, bright (−0.5 EV) right, with fine texture.
fn step_edge(w: u32, h: u32, amp: f32) -> Vec<f32> {
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let base = if x < w / 2 { -6.5 } else { -0.5 };
            let t = amp * (((x * 7 + y * 13) % 5) as f32 / 4.0 - 0.5);
            let l = (base + t).exp2();
            v.extend_from_slice(&[l, l, l, 1.0]);
        }
    }
    v
}

#[test]
fn no_halos_on_step_edges() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let ops = OpRegistry::builtin();
    let (w, h) = (400u32, 64u32);
    let img = step_edge(w, h, 0.2);
    let src = en.upload(w, h, &img).expect("upload");
    let mid = (w / 2) as usize;
    // Tone equalizer: +2 EV shadows. The guided mask keeps the edge: the dark side gets
    // the full gain right up to it, the bright side none.
    let shadows = |edges: f32| {
        edit(vec![te(Fields::new()
            .with("shadows_ev", Value::Float(2.0))
            .with("smoothing", Value::Float(0.05))
            .with("mask_edges", Value::Float(edges)))])
    };
    let out = render(&mut en, &graph(&shadows(0.1), ops), &src, &f32_whole(w, h));
    let g = column_gain(&img, &out, w, h);
    eprintln!("tone equalizer across the edge: {:?}", &g[mid - 4..mid + 4]);
    for (x, v) in g.iter().enumerate().take(mid).skip(mid - 20) {
        assert!(*v > 1.9, "dark side column {x}: {v}");
    }
    for (x, v) in g.iter().enumerate().skip(mid).take(20) {
        assert!(v.abs() < 0.1, "bright side column {x}: {v}");
    }
    // The same with a large ε, closer to a plain blur: the mask bleeds over the edge and
    // the dark side loses its gain next to it (the test sees halos).
    let out = render(&mut en, &graph(&shadows(16.0), ops), &src, &f32_whole(w, h));
    let g = column_gain(&img, &out, w, h);
    assert!(g[mid - 3] < 1.5, "{:?}", &g[mid - 4..mid + 4]);
    // Local contrast: clarity and texture boost the texture, but the 6 EV edge keeps its
    // flat sides: no overshoot or undershoot next to it.
    for (t, c) in [(0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
        let flat = step_edge(w, h, 0.0);
        let src = en.upload(w, h, &flat).expect("upload");
        let out = render(
            &mut en,
            &graph(&edit(vec![lc(t, c)]), ops),
            &src,
            &f32_whole(w, h),
        );
        let g = column_gain(&flat, &out, w, h);
        let (at, worst) =
            g.iter().enumerate().fold(
                (0, 0f32),
                |m, (i, v)| if v.abs() > m.1 { (i, v.abs()) } else { m },
            );
        eprintln!(
            "local contrast t={t} c={c} on a flat step: worst column change {worst:e} EV at {}",
            at as i64 - mid as i64
        );
        // The edge itself may gain a little contrast (the darker side a little darker, the
        // brighter a little brighter), at most 2 % of the 6 EV step per control at full
        // strength, where an unsharp mask of the same strength overshoots by whole EVs; a
        // band of the opposite tone stays under 0.5 %.
        let dark = g[..mid].iter().fold(0f32, |m, v| m.max(*v));
        let bright = g[mid..].iter().fold(0f32, |m, v| m.min(*v));
        assert!(
            dark < 0.03 && bright > -0.03,
            "t={t} c={c}: {dark} {bright}"
        );
        assert!(worst < 0.12 * (t + c), "t={t} c={c}: {worst}");
        // Texture next to the edge grows like texture far from it.
        let tsrc = en_src(&mut en, &img, w, h);
        let out = render(
            &mut en,
            &graph(&edit(vec![lc(t, c)]), ops),
            &tsrc,
            &f32_whole(w, h),
        );
        let amp = |o: &[f32], x: usize| {
            let v: Vec<f32> = (0..h as usize)
                .map(|y| log_y(&o[(y * w as usize + x) * 4..][..4]))
                .collect();
            let m = v.iter().sum::<f32>() / v.len() as f32;
            (v.iter().map(|a| (a - m) * (a - m)).sum::<f32>() / v.len() as f32).sqrt()
        };
        let near = amp(&out, mid + 3) / amp(&img, mid + 3);
        let far = amp(&out, mid + 60) / amp(&img, mid + 60);
        let mean_near = column_gain(&img, &out, w, h)[mid + 3];
        assert!(
            mean_near.abs() < 0.08,
            "t={t} c={c}: mean shift next to the edge {mean_near}"
        );
        if t > 0.0 {
            assert!(
                far > 1.2 && near > 0.8 * far,
                "t={t} c={c}: near {near}, far {far}"
            );
        }
    }
}

fn en_src(en: &mut Engine, img: &[f32], w: u32, h: u32) -> Source {
    en.upload(w, h, img).expect("upload")
}

#[test]
fn tiles_match_the_whole_image() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let ops = OpRegistry::builtin();
    let (w, h) = (1300u32, 900u32);
    let img = photo(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let mut e = edit(vec![
        te(Fields::new().with("shadows_ev", Value::Float(1.0))),
        lc(0.8, 0.5),
    ]);
    set(
        &mut e,
        ModuleKind::TONE_EQUALIZER,
        "highlights_ev",
        Value::Float(-1.0),
    );
    let g = graph(&e, ops);
    let whole = render(&mut en, &g, &src, &f32_whole(w, h));
    let req = Request {
        tiling: Tiling::Side(300),
        ..f32_whole(w, h)
    };
    let tiled = render(&mut en, &g, &src, &req);
    let (diff, at) = crate::graph::testops::max_abs(&whole, &tiled);
    eprintln!("tone ops 1300×900 tiled (300) vs whole: max |diff| {diff:e}");
    assert!(diff <= 1e-4, "{diff} at {at}");
    // A region alone is the same pixels.
    let roi = Roi::new(611, 203, 150, 97);
    let part = render(
        &mut en,
        &g,
        &src,
        &Request {
            roi,
            ..f32_whole(w, h)
        },
    );
    let mut d = 0f32;
    for y in 0..roi.h {
        for x in 0..roi.w {
            for c in 0..4 {
                let a = part[((y * roi.w + x) * 4 + c) as usize];
                let b = whole[(((roi.y + y) * w + roi.x + x) * 4 + c) as usize];
                d = d.max((a - b).abs());
            }
        }
    }
    assert!(d <= 1e-4, "{d}");
}

/// Area-reduces an RGBA image by an integer factor.
pub fn shrink(img: &[f32], w: u32, h: u32, f: u32) -> Vec<f32> {
    let (dw, dh) = (w / f, h / f);
    let mut v = vec![0f32; (dw * dh * 4) as usize];
    for y in 0..dh {
        for x in 0..dw {
            for c in 0..4 {
                let mut s = 0.0;
                for yy in 0..f {
                    for xx in 0..f {
                        s += img[(((y * f + yy) * w + x * f + xx) * 4 + c) as usize];
                    }
                }
                v[((y * dw + x) * 4 + c) as usize] = s / (f * f) as f32;
            }
        }
    }
    v
}

pub fn lab(rgb709: [f32; 3]) -> [f64; 3] {
    let r = rgb709.map(|v| f64::from(v.max(0.0)));
    let x = 0.412_456_4 * r[0] + 0.357_576_1 * r[1] + 0.180_437_5 * r[2];
    let y = 0.212_672_9 * r[0] + 0.715_152_2 * r[1] + 0.072_175 * r[2];
    let z = 0.019_333_9 * r[0] + 0.119_192 * r[1] + 0.950_304_1 * r[2];
    let f = |t: f64| {
        if t > (6.0f64 / 29.0).powi(3) {
            t.cbrt()
        } else {
            t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 (Sharma, Wu and Dalal 2005).
pub fn de2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (l1, a1, b1) = (a[0], a[1], a[2]);
    let (l2, a2, b2) = (b[0], b[1], b[2]);
    let c1 = a1.hypot(b1);
    let c2 = a2.hypot(b2);
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = (a1 * (1.0 + g), a2 * (1.0 + g));
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hp = |bb: f64, ap: f64| {
        if bb == 0.0 && ap == 0.0 {
            0.0
        } else {
            bb.atan2(ap).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1p, h2p) = (hp(b1, a1p), hp(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh.to_radians() / 2.0).sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hm).to_radians().cos()
        + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(2.0 * dtheta).to_radians().sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh))
        .sqrt()
}

#[test]
fn ciede2000_matches_published_pairs() {
    // Sharma et al.'s test data, pairs 1, 7, 17 and 25.
    for (a, b, want) in [
        ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
        ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
        ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        (
            [60.2574, -34.0099, 36.2677],
            [60.4626, -34.1751, 39.4387],
            1.2644,
        ),
    ] {
        let d = de2000(a, b);
        assert!((d - want).abs() < 1e-4, "{d} vs {want}");
    }
}

#[test]
fn preview_and_full_size_agree() {
    // Acceptance: ΔE2000 p95 < 1 between a preview-size render and the
    // full-size render reduced to the preview size.
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (2400u32, 1600u32);
    let img = photo(w, h);
    let e = {
        let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
        let reg = Registry::builtin();
        for m in [
            te(Fields::new()
                .with("shadows_ev", Value::Float(1.0))
                .with("highlights_ev", Value::Float(-1.0))),
            lc(0.6, 0.5),
            module(
                ModuleKind::CURVES,
                Fields::new().with(
                    "points",
                    Value::Floats(vec![0.0, 0.0, 0.3, 0.25, 0.7, 0.78, 1.0, 1.0]),
                ),
            ),
        ] {
            e.insert(reg, m);
        }
        e
    };
    let out = OutputSpace::rec709();
    let g = Graph::build_for(&e, OpRegistry::builtin(), None, Some(&out)).expect("graph");
    let src = en.upload(w, h, &img).expect("upload");
    let full = render(&mut en, &g, &src, &f32_whole(w, h));
    for f in [2u32, 4] {
        let (pw, ph) = (w / f, h / f);
        let small = shrink(&img, w, h, f);
        let psrc = en.upload(pw, ph, &small).expect("upload");
        let req = Request {
            scale: 1.0 / f as f32,
            ..f32_whole(pw, ph)
        };
        let prev = render(&mut en, &g, &psrc, &req);
        let reduced = shrink(&full, w, h, f);
        let mut de: Vec<f64> = prev
            .chunks_exact(4)
            .zip(reduced.chunks_exact(4))
            .map(|(a, b)| de2000(lab([a[0], a[1], a[2]]), lab([b[0], b[1], b[2]])))
            .collect();
        de.sort_by(f64::total_cmp);
        let p95 = de[(de.len() - 1) * 95 / 100];
        let p50 = de[(de.len() - 1) / 2];
        eprintln!("preview 1/{f} vs full reduced: ΔE2000 p50 {p50:.3}, p95 {p95:.3}");
        assert!(p95 < 1.0, "1/{f}: p95 {p95}");
    }
}

#[test]
fn slider_moves_make_no_pipelines_or_buffers() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (640u32, 400u32);
    let img = photo(w, h);
    let src = en.upload(w, h, &img).expect("upload");
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), false);
    for m in [
        te(Fields::new().with("shadows_ev", Value::Float(0.5))),
        lc(0.3, 0.3),
        module(
            ModuleKind::CURVES,
            Fields::new().with("points", Value::Floats(vec![0.0, 0.0, 0.5, 0.55, 1.0, 1.0])),
        ),
    ] {
        e.insert(Registry::builtin(), m);
    }
    let out = OutputSpace::rec709();
    type Slider = (ModuleKind, &'static str, fn(i32) -> Value);
    let sliders: [Slider; 6] = [
        (ModuleKind::TONE_EQUALIZER, "shadows_ev", |i| {
            Value::Float(0.5 + i as f32 * 0.1)
        }),
        (ModuleKind::LOCAL_CONTRAST, "texture", |i| {
            Value::Float(0.3 + i as f32 * 0.05)
        }),
        (ModuleKind::LOCAL_CONTRAST, "clarity", |i| {
            Value::Float(0.3 - i as f32 * 0.03)
        }),
        (ModuleKind::CURVES, "points", |i| {
            Value::Floats(vec![0.0, 0.0, 0.5, 0.55 + i as f32 * 0.01, 1.0, 1.0])
        }),
        (ModuleKind::CURVES, "levels_grey", |i| {
            Value::Float(0.5 + i as f32 * 0.02)
        }),
        (ModuleKind::EXPOSURE, "exposure_ev", |i| {
            Value::Float(i as f32 * 0.1)
        }),
    ];
    for (kind, name, val) in sliders {
        let focus = Some((kind, 0));
        let req = Request::new(Roi::full(w, h));
        // Warm: the first value twice (the second settles the cache), then two changes (the
        // first to take statistics again binds their buffers, the second settles the pool's
        // order).
        for v in [0, 0, 1, 2] {
            set(&mut e, kind, name, val(v));
            let g = Graph::build_for(&e, OpRegistry::builtin(), focus, Some(&out)).unwrap();
            let r = en.render(&g, &src, &req).expect("render");
            en.recycle(r);
        }
        let before = en.counters();
        for i in 3..10 {
            set(&mut e, kind, name, val(i));
            let g = Graph::build_for(&e, OpRegistry::builtin(), focus, Some(&out)).unwrap();
            let r = en.render(&g, &src, &req).expect("render");
            let st = r.stats.clone();
            en.recycle(r);
            assert_eq!(st.pipelines_created, 0, "{name} step {i}");
            assert_eq!(st.buffers_created, 0, "{name} step {i}");
            assert_eq!(st.bind_groups_created, 0, "{name} step {i}");
        }
        assert_eq!(en.counters().pipelines, before.pipelines, "{name}");
    }
}

#[test]
fn auto_exposure_follows_the_scene() {
    let Some(mut en) = engine() else { return };
    let _l = lock();
    let (w, h) = (300u32, 200u32);
    let img = photo(w, h);
    let e = edit(vec![module(
        ModuleKind::EXPOSURE,
        Fields::new()
            .with("auto", Value::Bool(true))
            .with("auto_percentile", Value::Float(50.0))
            .with("auto_target_ev", Value::Float(-2.5)),
    )]);
    let g = graph(&e, OpRegistry::builtin());
    let a = {
        let src = en.upload(w, h, &img).expect("upload");
        render(&mut en, &g, &src, &f32_whole(w, h))
    };
    // Three stops brighter in, the same out.
    let bright: Vec<f32> = img
        .chunks_exact(4)
        .flat_map(|c| [c[0] * 8.0, c[1] * 8.0, c[2] * 8.0, c[3]])
        .collect();
    let b = {
        let src = en.upload(w, h, &bright).expect("upload");
        render(&mut en, &g, &src, &f32_whole(w, h))
    };
    let (worst, _) = compare(&a, &b, 1e-3);
    assert!(worst < 2e-3, "{worst}");
    // The median lands on the target.
    let mut l: Vec<f32> = a.chunks_exact(4).map(log_y).collect();
    l.sort_by(f32::total_cmp);
    let med = l[l.len() / 2];
    assert!((med + 2.5).abs() < 0.05, "median {med}");
}

#[test]
fn identities_and_curve_shapes() {
    let (w, h) = (64u32, 40u32);
    let img = photo(w, h);
    // At their defaults the three tone modules change nothing.
    let e = edit(vec![
        te(Fields::new()),
        lc(0.0, 0.0),
        module(ModuleKind::CURVES, Fields::new()),
    ]);
    let g = graph(&e, OpRegistry::builtin());
    let out = cpu_ref::run_image(&g, w, h, &img);
    assert_eq!(out, img);
    // A curve passes through its points (on the encoded axis) and never falls.
    let c = curves::Curves::from_fields(&Fields::new().with(
        "points",
        Value::Floats(vec![0.0, 0.1, 0.3, 0.2, 0.6, 0.75, 1.0, 0.9]),
    ));
    assert_eq!(c.master.len(), 4);
    let u = c.uniforms();
    let at = |v: f32| tcpu::curves(&u, [v, v, v, 1.0])[1];
    // (A norm of 0 cannot be scaled: the first point is not checked.)
    for p in &c.master[1..] {
        let v = p[0].powf(2.4);
        let want = p[1].powf(2.4);
        assert!((at(v) - want).abs() < 1e-5, "{p:?}: {}", at(v));
    }
    let mut last = f32::MIN;
    for i in 0..=400 {
        let v = (i as f32 / 100.0).powf(2.4);
        let o = at(v);
        assert!(o >= last - 1e-6, "falls at {v}");
        last = o;
    }
    // A bad point list is the identity.
    let bad = curves::Curves::from_fields(&Fields::new().with(
        "points",
        Value::Floats(vec![0.0, 0.0, 0.7, 0.5, 0.3, 0.2, 1.0, 1.0]),
    ));
    assert_eq!(bad.master, curves::identity());
    // Levels: black to 0, white to 1, grey to the middle.
    let lv = curves::Curves::from_fields(
        &Fields::new()
            .with("levels_black", Value::Float(0.1))
            .with("levels_white", Value::Float(0.9))
            .with("levels_grey", Value::Float(0.3)),
    );
    let u = lv.uniforms();
    let enc = |v: f32| v.powf(1.0 / 2.4);
    let at = |x: f32| enc(tcpu::curves(&u, [x.powf(2.4); 4])[0]);
    assert!(at(0.1).abs() < 1e-5 && (at(0.9) - 1.0).abs() < 1e-5);
    assert!((at(0.1 + 0.3 * 0.8) - 0.5).abs() < 1e-4);
    // Texture levels follow the view scale.
    assert_eq!(local_contrast::texture_levels(1.0), 3);
    assert_eq!(local_contrast::texture_levels(0.5), 2);
    assert_eq!(local_contrast::texture_levels(0.25), 1);
    assert_eq!(local_contrast::texture_levels(0.1), 0);
    assert_eq!(
        local_contrast::aligned(Roi::new(13, 0, 100, 50), 3),
        Roi::new(16, 0, 97, 50)
    );
}
