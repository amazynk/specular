//! The raw front end on the GPU against its CPU reference, bands against a whole-image run,
//! values above white, and the demosaic against the viewer's CPU path on synthetic scenes.
//!
//! Without an adapter the GPU tests are skipped, unless SPECULAR_REQUIRE_GPU is set.

use std::sync::{Arc, Mutex, OnceLock};

use specular_raw::sensor::{BlackLevels, Layout, OpcodeInfo, PointKind, SensorMeta};
use specular_raw::{Cfa, MatrixSource, Rect, SensorData, WbSource};

use super::*;

mod correct;
use crate::graph::{Graph, OpRegistry};
use crate::model::{Edit, ModuleKind};

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn dev() -> Option<&'static Dev> {
    static D: OnceLock<Option<Dev>> = OnceLock::new();
    D.get_or_init(|| match open() {
        Ok(d) => Some(d),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU tests skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn open() -> Result<Dev, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.flags = wgpu::InstanceFlags::empty().with_env();
    desc.backends = wgpu::Backends::PRIMARY.with_env();
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let info = adapter.get_info();
    let a = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(a.clone());
    limits.max_storage_buffer_binding_size = a.max_storage_buffer_binding_size;
    limits.max_buffer_size = a.max_buffer_size;
    let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("raw front end tests"),
        required_features: features,
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("no device: {e}"))?;
    eprintln!(
        "gpu: {} ({:?}), max binding {} MB, max buffer {} MB",
        info.name,
        info.backend,
        a.max_storage_buffer_binding_size >> 20,
        a.max_buffer_size >> 20
    );
    Ok(Dev { device, queue })
}

/// Fujifilm's X-Trans layout.
const XTRANS: [&str; 6] = ["GGRGGB", "GGBGGR", "BRGRBG", "GGBGGR", "GGRGGB", "RBGBRG"];

pub(crate) fn xtrans_cfa() -> Cfa {
    let colors = XTRANS
        .iter()
        .flat_map(|r| {
            r.bytes().map(|b| match b {
                b'R' => 0,
                b'G' => 1,
                _ => 2,
            })
        })
        .collect();
    Cfa::new(6, 6, colors).unwrap()
}

/// A deterministic scene in linear camera RGB (about 0..1.2): gradients, edges at several
/// angles, a zone plate, fine stripes and noise.
fn scene(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
    let mut c = [0.2 + 0.6 * fx, 0.3 + 0.4 * fy, 0.5 - 0.3 * fx * fy];
    // A disc and a slanted edge.
    let (dx, dy) = (fx - 0.3, fy - 0.4);
    if dx * dx + dy * dy < 0.04 {
        c = [0.9, 0.2, 0.1];
    }
    if fx + 0.4 * fy > 0.85 {
        c = [c[2] * 0.5, c[0], 0.95];
    }
    // Zone plate in the lower left.
    if fx < 0.4 && fy > 0.6 {
        let r2 = ((fx - 0.2).powi(2) + (fy - 0.8).powi(2)) * (w as f32).powi(2);
        let z = 0.5 + 0.4 * (r2 * 0.02).cos();
        c = [z, z * 0.9, z * 0.8];
    }
    // Hash noise.
    let n = ((x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) % 1000) as f32 / 1000.0;
    c.map(|v| v + (n - 0.5) * 0.01)
}

pub(crate) struct Spec {
    layout: Layout,
    cpp: u32,
    w: u32,
    h: u32,
    black: f32,
    white: f32,
    /// Scene gain: > 1 puts values above white.
    gain: f32,
}

pub(crate) fn sensor(sp: &Spec) -> SensorData {
    let (bw, bh) = (sp.w + 6, sp.h + 4);
    let crop = Rect {
        x: 2,
        y: 2,
        w: sp.w,
        h: sp.h,
    };
    let mut samples = Vec::with_capacity((bw * bh * sp.cpp) as usize);
    for y in 0..bh {
        for x in 0..bw {
            let c = scene(x, y, bw, bh);
            for ch in 0..sp.cpp {
                let col = match sp.cpp {
                    3 => ch as usize,
                    _ => usize::from(sp.layout.color(x, y)),
                };
                let v = sp.black + c[col.min(2)] * sp.gain * (sp.white - sp.black);
                samples.push(v.clamp(0.0, 65535.0) as u16);
            }
        }
    }
    SensorData {
        width: bw,
        height: bh,
        cpp: sp.cpp,
        samples,
        float: None,
        crop,
        meta: SensorMeta {
            layout: sp.layout.clone(),
            black: BlackLevels::simple([sp.black; 4], sp.cpp),
            white: sp.white,
            clip_white: sp.white,
            linearization: None,
            opcodes: Default::default(),
            opcodes_unknown: Vec::new(),
            wb: [1.9, 1.0, 1.4],
            wb_source: WbSource::AsShot,
            rgb_cam: [[1.6, -0.4, -0.2], [-0.2, 1.4, -0.2], [0.0, -0.5, 1.5]],
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

pub(crate) fn bayer(w: u32, h: u32) -> Spec {
    Spec {
        layout: Layout::Bayer([0, 1, 1, 2]),
        cpp: 1,
        w,
        h,
        black: 512.0,
        white: 15000.0,
        gain: 1.0,
    }
}

/// Every point opcode, a bad-pixel fix and black deltas.
fn with_opcodes(mut s: SensorData) -> SensorData {
    let full = Rect {
        x: 0,
        y: 0,
        w: s.width,
        h: s.height,
    };
    let point = |kind: PointKind, plane: u32, pitch: u32| OpcodeInfo::Point {
        rect: full,
        plane,
        planes: 1,
        row_pitch: pitch,
        col_pitch: pitch,
        bounds: s.crop,
        kind,
    };
    // Bad pixels at a stored value no other sample has.
    for i in [57usize, 300, 901] {
        s.samples[i] = 7;
    }
    s.meta.opcodes[0] = vec![
        OpcodeInfo::FixBadPixelsConstant {
            value: 7,
            bounds: full,
        },
        point(PointKind::MapPolynomial(vec![0.001, 0.98, 0.02]), 0, 1),
    ];
    s.meta.opcodes[1] = vec![
        point(
            PointKind::GainMap {
                points_v: 3,
                points_h: 4,
                spacing_v: 0.5,
                spacing_h: 1.0 / 3.0,
                origin_v: 0.0,
                origin_h: 0.0,
                planes: 1,
                gains: vec![
                    1.0, 1.1, 1.2, 1.3, 0.9, 1.0, 1.05, 1.1, 1.2, 1.15, 1.0, 0.95,
                ],
            },
            0,
            2,
        ),
        point(
            PointKind::DeltaPerColumn((0..s.width).map(|i| (i % 5) as f32 * 1e-3).collect()),
            0,
            1,
        ),
        point(
            PointKind::ScalePerRow((0..s.height).map(|i| 1.0 + (i % 3) as f32 * 0.01).collect()),
            0,
            1,
        ),
    ];
    s.meta.opcodes[2] = vec![
        point(
            PointKind::Vignette {
                k: [0.3, 0.1, 0.0, 0.0, 0.0],
                centre: (
                    s.crop.x as f64 + s.crop.w as f64 * 0.5,
                    s.crop.y as f64 + s.crop.h as f64 * 0.5,
                ),
                inv_m2: 1.0 / ((s.crop.w as f64 * 0.5).powi(2) + (s.crop.h as f64 * 0.5).powi(2)),
            },
            0,
            1,
        ),
        OpcodeInfo::Warp {
            fisheye: false,
            k: [[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]; 3],
            centre: (0.5, 0.5),
            bounds: s.crop,
        },
    ];
    s.meta.black.per_column = (0..s.width).map(|i| (i % 4) as f32).collect();
    s.meta.black.per_row = (0..s.height).map(|i| (i % 3) as f32 * 2.0).collect();
    s.meta.linearization = Some(Arc::from(
        (0..=16383u32)
            .map(|v| (v + v * v / 65536) as u16)
            .collect::<Vec<u16>>(),
    ));
    s
}

fn opts(q: Quality) -> Options {
    Options {
        precision: Precision::F32,
        ..Options::new(q)
    }
}

/// (max |diff| relative to max(1, |ref|), pixels over `tol`).
fn compare(gpu: &[f32], cpu: &[f32], tol: f32) -> (f32, usize) {
    assert_eq!(gpu.len(), cpu.len());
    let mut worst = 0f32;
    let mut over = 0;
    for (p, q) in gpu.chunks_exact(4).zip(cpu.chunks_exact(4)) {
        let d = (0..3)
            .map(|i| (p[i] - q[i]).abs() / q[i].abs().max(1.0))
            .fold(0f32, f32::max);
        assert!(d.is_finite(), "non-finite output: {p:?} vs {q:?}");
        worst = worst.max(d);
        if d > tol {
            over += 1;
        }
    }
    (worst, over)
}

fn gpu_run(fe: &mut FrontEnd, s: &SensorData, o: &Options) -> (FrontEndOutput, Vec<f32>) {
    let g = fe.upload(s.clone()).unwrap();
    let out = fe.run(&g, o).unwrap();
    let v = fe.read(&out).unwrap();
    (out, v)
}

#[test]
fn gpu_matches_the_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let xt = Spec {
        layout: Layout::Pattern(xtrans_cfa()),
        ..bayer(96, 66)
    };
    let linear = Spec {
        layout: Layout::Linear,
        cpp: 3,
        ..bayer(40, 30)
    };
    let mono = Spec {
        layout: Layout::Mono,
        ..bayer(40, 30)
    };
    let cases: Vec<(&str, SensorData, Quality, f32, f64)> = vec![
        (
            "bayer half",
            with_opcodes(sensor(&bayer(98, 70))),
            Quality::Half,
            2e-5,
            0.002,
        ),
        (
            "bayer fast",
            with_opcodes(sensor(&bayer(98, 70))),
            Quality::Fast,
            2e-5,
            0.002,
        ),
        (
            "bayer full",
            with_opcodes(sensor(&bayer(98, 70))),
            Quality::Full,
            1e-4,
            0.002,
        ),
        (
            "x-trans half",
            with_opcodes(sensor(&xt)),
            Quality::Half,
            2e-5,
            0.002,
        ),
        // With the linearization table, a sample on a rounding boundary may pick the next
        // entry (f32 contraction differs); homogeneity choices may flip on near-ties.
        (
            "x-trans full",
            with_opcodes(sensor(&xt)),
            Quality::Full,
            1e-3,
            0.002,
        ),
        (
            "linear full",
            with_opcodes(sensor(&linear)),
            Quality::Full,
            2e-5,
            0.002,
        ),
        ("linear half", sensor(&linear), Quality::Half, 2e-5, 0.0),
        ("mono full", sensor(&mono), Quality::Full, 2e-5, 0.0),
    ];
    for (name, s, q, tol, frac) in cases {
        for output in [Output::Rec2020, Output::CameraRgb, Output::Mosaic] {
            let o = Options { output, ..opts(q) };
            let (out, gpu) = gpu_run(&mut fe, &s, &o);
            let (w, h, cpu) = cpu_ref::develop(&s, &o);
            assert_eq!((out.width, out.height), (w, h), "{name}");
            let (worst, over) = compare(&gpu, &cpu, tol);
            let n = (w * h) as usize;
            eprintln!("{name} {output:?}: max rel diff {worst:.2e}, {over}/{n} over {tol:e}");
            assert!(
                over as f64 <= frac * n as f64,
                "{name} {output:?}: {over} of {n} pixels differ by more than {tol}"
            );
        }
    }
}

#[test]
fn warps_and_bad_pixel_lists_are_reported() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let s = with_opcodes(sensor(&bayer(40, 30)));
    let g = fe.upload(s).unwrap();
    let out = fe.run(&g, &opts(Quality::Half)).unwrap();
    assert_eq!(out.skipped, vec!["WarpRectilinear"]);
}

#[test]
fn bands_equal_a_whole_image_run() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let xt = Spec {
        layout: Layout::Pattern(xtrans_cfa()),
        ..bayer(60, 90)
    };
    for (name, s) in [
        ("bayer", with_opcodes(sensor(&bayer(64, 90)))),
        ("x-trans", with_opcodes(sensor(&xt))),
    ] {
        for q in [Quality::Half, Quality::Fast, Quality::Full] {
            let whole = Options {
                band_rows: 1 << 16,
                ..opts(q)
            };
            let (wo, a) = gpu_run(&mut fe, &s, &whole);
            assert_eq!(wo.stats.bands, 1);
            for rows in [1, 5, 16, 33] {
                let o = Options {
                    band_rows: rows,
                    ..opts(q)
                };
                let (bo, b) = gpu_run(&mut fe, &s, &o);
                assert!(bo.stats.bands > 1);
                let same = a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits());
                assert!(
                    same,
                    "{name} {q:?}: bands of {rows} rows differ from one band"
                );
            }
        }
    }
}

/// `src` (`w × h` RGBA) area-reduced to `dw × dh` with the preview's weights.
fn area_reduce(src: &[f32], w: u32, h: u32, dw: u32, dh: u32) -> Vec<f32> {
    let (sx, sy) = (f64::from(w) / f64::from(dw), f64::from(h) / f64::from(dh));
    let mut out = vec![0f32; (dw * dh * 4) as usize];
    for oy in 0..dh {
        let (y0, y1) = (f64::from(oy) * sy, f64::from(oy + 1) * sy);
        for ox in 0..dw {
            let (x0, x1) = (f64::from(ox) * sx, f64::from(ox + 1) * sx);
            let mut acc = [0f64; 4];
            let mut ws = 0.0;
            for y in y0.floor() as u32..(y1.ceil() as u32).min(h) {
                let wy = f64::from(y + 1).min(y1) - f64::from(y).max(y0);
                for x in x0.floor() as u32..(x1.ceil() as u32).min(w) {
                    let wx = f64::from(x + 1).min(x1) - f64::from(x).max(x0);
                    let i = ((y * w + x) * 4) as usize;
                    for (a, v) in acc.iter_mut().zip(&src[i..i + 4]) {
                        *a += f64::from(*v) * wx * wy;
                    }
                    ws += wx * wy;
                }
            }
            let o = ((oy * dw + ox) * 4) as usize;
            for (d, a) in out[o..o + 4].iter_mut().zip(acc) {
                *d = (a / ws) as f32;
            }
        }
    }
    out
}

/// A reduced run (the full demosaic in stripes, each area-reduced into the
/// output) equals the full-size run area-reduced afterwards, whatever the stripes and
/// bands, with and without the corrections' mosaic.
#[test]
fn reduced_runs_equal_a_full_run_reduced() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let xt = Spec {
        layout: Layout::Pattern(xtrans_cfa()),
        ..bayer(60, 90)
    };
    let mut worst = 0f32;
    for (name, s) in [
        ("bayer", with_opcodes(sensor(&bayer(64, 90)))),
        ("x-trans", with_opcodes(sensor(&xt))),
    ] {
        let (w, h) = (s.crop.w, s.crop.h);
        for q in [Quality::Fast, Quality::Full] {
            for hot in [None, Some(0.5)] {
                let mut o = opts(q);
                o.corrections.hot = hot;
                let (_, full) = gpu_run(&mut fe, &s, &o);
                for (dw, dh) in [(32, 45), (23, 31), (w / 3, h / 3)] {
                    let want = area_reduce(&full, w, h, dw, dh);
                    for (stripe, band) in [(None, 0), (Some(4), 0), (Some(9), 5)] {
                        fe.stripe_rows = stripe;
                        let r = Options {
                            reduce_to: Some((dw, dh)),
                            band_rows: band,
                            ..o
                        };
                        let (out, got) = gpu_run(&mut fe, &s, &r);
                        assert_eq!((out.width, out.height), (dw, dh));
                        let (err, over) = compare(&got, &want, 1e-4);
                        worst = worst.max(err);
                        assert_eq!(
                            over, 0,
                            "{name} {q:?} hot {hot:?} to {dw}x{dh}, stripes {stripe:?}: \
                             worst {err}"
                        );
                    }
                    fe.stripe_rows = None;
                }
            }
        }
    }
    eprintln!("reduced run vs full run reduced: worst relative difference {worst:e}");
}

#[test]
fn values_above_white_survive() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    // The scene reaches 1.2 x 3 = 3.6 times the white level.
    let sp = Spec {
        gain: 3.0,
        white: 4000.0,
        black: 0.0,
        ..bayer(64, 48)
    };
    let s = sensor(&sp);
    for q in [Quality::Half, Quality::Fast, Quality::Full] {
        for output in [Output::Mosaic, Output::CameraRgb, Output::Rec2020] {
            let o = Options { output, ..opts(q) };
            let (_, v) = gpu_run(&mut fe, &s, &o);
            let max = v
                .chunks_exact(4)
                .flat_map(|p| p[..3].to_vec())
                .fold(0f32, f32::max);
            eprintln!("{q:?} {output:?}: max {max}");
            assert!(max > 2.0, "{q:?} {output:?}: values clipped (max {max})");
        }
    }
    // f16 storage keeps them too.
    let o = Options::new(Quality::Full);
    let (_, v) = gpu_run(&mut fe, &s, &o);
    assert!(v.iter().copied().fold(0f32, f32::max) > 2.0);
}

#[test]
fn f16_output_is_close_to_f32() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let s = sensor(&bayer(64, 48));
    let (_, a) = gpu_run(&mut fe, &s, &opts(Quality::Full));
    let (o16, b) = gpu_run(&mut fe, &s, &Options::new(Quality::Full));
    assert_eq!(o16.precision, Precision::F16);
    let (worst, _) = compare(&b, &a, 1e-3);
    assert!(worst < 1e-3, "f16 differs by {worst}");
}

#[test]
fn raw_stage_modules_go_to_the_front_end() {
    let reg = crate::model::Registry::builtin();
    let e = Edit::default_for(reg, crate::model::SourceKey::default(), true);
    let g = Graph::build(&e, &OpRegistry::empty(), None).unwrap();
    assert_eq!(g.front_end.len(), 1);
    assert_eq!(g.front_end[0].0.0, ModuleKind::RAW_FRONT_END);
    assert!(!g.skipped.iter().any(|k| k.0 == ModuleKind::RAW_FRONT_END));
    assert!(g.skipped.contains(&(ModuleKind::EXPOSURE, 0)));
    let crate::model::Params::RawFrontEnd(p) = &g.front_end[0].1 else {
        panic!("typed params")
    };
    let o = Options::from_params(p, 1.0);
    assert_eq!(
        (o.quality, o.opcodes, o.profile),
        (Quality::Full, true, true)
    );
    assert_eq!(Options::from_params(p, 0.5).quality, Quality::Half);
}

// ---- Demosaic quality on synthetic scenes ----------------------------------------------------

/// Synthetic test images in the spirit of the Kodak set (natural-looking content with
/// fences, foliage-like texture, saturated edges, smooth skies), made procedurally.
fn kodak_like(k: u32, w: u32, h: u32) -> Vec<[f32; 3]> {
    let mut v = Vec::with_capacity((w * h) as usize);
    let hash = |x: u32, y: u32, s: u32| {
        let mut a = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ s;
        a ^= a >> 15;
        a = a.wrapping_mul(0x2C1B_3C6D);
        a ^= a >> 12;
        (a & 0xffff) as f32 / 65535.0
    };
    // Value noise: smooth random texture.
    let vnoise = |x: f32, y: f32, s: u32| {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let g = |i: f32, j: f32| hash((x0 + i) as u32, (y0 + j) as u32, s);
        let a = g(0.0, 0.0) + (g(1.0, 0.0) - g(0.0, 0.0)) * sx;
        let b = g(0.0, 1.0) + (g(1.0, 1.0) - g(0.0, 1.0)) * sx;
        a + (b - a) * sy
    };
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let c = match k {
                // Lighthouse-like: sky gradient, a white fence of thin slats.
                0 => {
                    let sky = [0.35 + 0.2 * fy / h as f32, 0.55, 0.85];
                    if fy > h as f32 * 0.55 {
                        let slat = ((fx * 0.45).sin() > 0.2) as u8 as f32;
                        let g = 0.15 + 0.7 * slat;
                        [g, g * 0.97, g * 0.92]
                    } else {
                        sky
                    }
                }
                // Foliage: multi-octave value noise in greens and browns.
                1 => {
                    let n = 0.5 * vnoise(fx / 6.0, fy / 6.0, 1)
                        + 0.3 * vnoise(fx / 3.0, fy / 3.0, 2)
                        + 0.2 * vnoise(fx / 12.0, fy / 12.0, 3);
                    [0.15 + 0.4 * n, 0.25 + 0.6 * n, 0.08 + 0.2 * n]
                }
                // Texture finer than the CFA can carry (1.5 px value noise): reported only.
                4 => {
                    let n =
                        0.6 * vnoise(fx / 1.5, fy / 1.5, 4) + 0.4 * vnoise(fx / 3.0, fy / 3.0, 5);
                    [0.15 + 0.4 * n, 0.25 + 0.6 * n, 0.08 + 0.2 * n]
                }
                // Saturated objects with sharp edges at many angles (parrots, boats).
                2 => {
                    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
                    let a = (fy - cy).atan2(fx - cx);
                    let sector =
                        ((a + std::f32::consts::PI) / (std::f32::consts::PI / 6.0)).floor() as u32;
                    match sector % 4 {
                        0 => [0.85, 0.1, 0.08],
                        1 => [0.1, 0.7, 0.15],
                        2 => [0.95, 0.85, 0.1],
                        _ => [0.1, 0.2, 0.8],
                    }
                }
                // Zone plate (aliasing stress) over a soft gradient.
                _ => {
                    let (dx, dy) = (fx - w as f32 * 0.5, fy - h as f32 * 0.5);
                    let z = 0.5 + 0.4 * ((dx * dx + dy * dy) * 0.0025).cos();
                    [z, z, z * 0.9]
                }
            };
            // Sensor-like noise, so no flat area looks like a clipped pile-up to the CPU
            // path's highlight detection.
            let n = (hash(x, y, 99 + k) - 0.5) * 0.008;
            v.push(c.map(|x| (x + n).clamp(0.02, 0.98)));
        }
    }
    v
}

fn psnr(a: &[[f32; 3]], b: &[[f32; 3]], w: u32, h: u32, border: u32) -> f64 {
    let (mut se, mut n) = (0f64, 0u64);
    for y in border..h - border {
        for x in border..w - border {
            let i = (y * w + x) as usize;
            for c in 0..3 {
                let d = f64::from(a[i][c].clamp(0.0, 1.0) - b[i][c].clamp(0.0, 1.0));
                se += d * d;
                n += 1;
            }
        }
    }
    10.0 * (1.0 / (se / n as f64)).log10()
}

/// Scene level at the sensor: half of white, so nothing reaches the range where the CPU
/// path's clip detection (made for real highlights) would act.
const LEVEL: f32 = 0.5;

/// Mosaics `img` with `cfa` (white 65535, black 0) at [`LEVEL`].
fn mosaic(img: &[[f32; 3]], w: u32, h: u32, cfa: &Cfa) -> Vec<u16> {
    let mut v = Vec::with_capacity(img.len());
    for y in 0..h {
        for x in 0..w {
            let c = cfa.color(y, x) as usize;
            v.push((img[(y * w + x) as usize][c] * LEVEL * 65535.0).round() as u16);
        }
    }
    v
}

/// The viewer's CPU develop of the mosaic (no white balance, identity matrix).
fn cpu_path(m: &[u16], w: u32, h: u32, cfa: &Cfa) -> Vec<[f32; 3]> {
    let raw = specular_raw::RawImage {
        width: w,
        height: h,
        cpp: 1,
        data: m.to_vec(),
        window: Rect { x: 0, y: 0, w, h },
        cfa: Some(cfa.clone()),
        crop: Rect { x: 0, y: 0, w, h },
        black: [0.0; 4],
        white: 65535.0,
        linearization: None,
        wb: Some([1.0; 3]),
        rgb_cam: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        matrix_source: MatrixSource::Identity,
        profile: None,
        orientation: 1,
        truncated: false,
        opcodes: None,
    };
    let img = specular_raw::develop(&raw, &specular_raw::DecodeOptions::default()).unwrap();
    img.data
        .chunks_exact(3)
        .map(|p| [0, 1, 2].map(|i| f32::from(p[i]) / 65535.0 / LEVEL))
        .collect()
}

fn gpu_path(
    fe: &mut FrontEnd,
    m: &[u16],
    w: u32,
    h: u32,
    layout: Layout,
    q: Quality,
) -> Vec<[f32; 3]> {
    let mut s = sensor(&Spec {
        layout,
        black: 0.0,
        white: 65535.0,
        ..bayer(w, h)
    });
    s.width = w;
    s.height = h;
    s.crop = Rect { x: 0, y: 0, w, h };
    s.samples = m.to_vec();
    s.meta.wb = [1.0; 3];
    let o = Options {
        output: Output::CameraRgb,
        ..opts(q)
    };
    let (_, v) = gpu_run(fe, &s, &o);
    v.chunks_exact(4)
        .map(|p| [p[0] / LEVEL, p[1] / LEVEL, p[2] / LEVEL])
        .collect()
}

#[test]
fn demosaic_psnr_beats_the_cpu_path() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (384, 256);
    let rggb = Cfa::bayer("RGGB").unwrap();
    let xt = xtrans_cfa();
    let (mut sum_cpu, mut sum_rcd, mut sum_mhc, mut sum_xc, mut sum_xg) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for k in 0..5 {
        let img = kodak_like(k, w, h);
        let m = mosaic(&img, w, h, &rggb);
        let cpu = psnr(&cpu_path(&m, w, h, &rggb), &img, w, h, 8);
        let rcd = psnr(
            &gpu_path(
                &mut fe,
                &m,
                w,
                h,
                Layout::Bayer([0, 1, 1, 2]),
                Quality::Full,
            ),
            &img,
            w,
            h,
            8,
        );
        let mhc = psnr(
            &gpu_path(
                &mut fe,
                &m,
                w,
                h,
                Layout::Bayer([0, 1, 1, 2]),
                Quality::Fast,
            ),
            &img,
            w,
            h,
            8,
        );
        let mx = mosaic(&img, w, h, &xt);
        let xc = psnr(&cpu_path(&mx, w, h, &xt), &img, w, h, 8);
        let xg = psnr(
            &gpu_path(
                &mut fe,
                &mx,
                w,
                h,
                Layout::Pattern(xt.clone()),
                Quality::Full,
            ),
            &img,
            w,
            h,
            8,
        );
        eprintln!(
            "image {k}: Bayer CPU path (MHC) {cpu:.2} dB, GPU RCD {rcd:.2} dB, GPU MHC {mhc:.2} dB; \
             X-Trans CPU {xc:.2} dB, GPU {xg:.2} dB"
        );
        if k == 4 {
            continue;
        }
        assert!(
            rcd >= cpu,
            "image {k}: RCD {rcd:.2} dB below the CPU path {cpu:.2} dB"
        );
        assert!(
            mhc >= cpu - 0.05,
            "image {k}: GPU MHC {mhc:.2} vs CPU MHC {cpu:.2}"
        );
        assert!(
            xg >= xc - 0.1,
            "image {k}: X-Trans GPU {xg:.2} vs CPU {xc:.2}"
        );
        sum_cpu += cpu;
        sum_rcd += rcd;
        sum_mhc += mhc;
        sum_xc += xc;
        sum_xg += xg;
    }
    eprintln!(
        "mean: CPU {:.2}, RCD {:.2}, MHC {:.2}, X-Trans CPU {:.2}, X-Trans GPU {:.2} dB",
        sum_cpu / 4.0,
        sum_rcd / 4.0,
        sum_mhc / 4.0,
        sum_xc / 4.0,
        sum_xg / 4.0
    );
}

/// The dual demosaic: GPU = CPU reference; on flat noisy areas it leaves less noise
/// and fewer maze patterns than the directional method alone, and on detail it keeps the
/// directional method's quality.
#[test]
fn dual_demosaic_smooths_flat_noise_and_keeps_detail() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    // GPU against the CPU reference.
    let s = sensor(&bayer(98, 70));
    let o = Options {
        dual: Some(0.2),
        ..opts(Quality::Full)
    };
    let (_, gpu) = gpu_run(&mut fe, &s, &o);
    let (_, _, cpu) = cpu_ref::develop(&s, &o);
    let (worst, over) = compare(&gpu, &cpu, 2e-5);
    eprintln!("dual demosaic GPU vs CPU worst {worst:.2e}");
    assert!(over == 0, "dual: worst {worst}");
    // From the params: dual only when asked.
    let mut p = RawFrontEnd::default();
    assert_eq!(Options::from_params(&p, 1.0).dual, None);
    p.demosaic = Demosaic::Dual;
    assert_eq!(Options::from_params(&p, 1.0).dual, Some(0.2));
    assert_eq!(Options::from_params(&p, 0.4).quality, Quality::Half);

    let (w, h) = (256u32, 192u32);
    let rggb = Cfa::bayer("RGGB").unwrap();
    let run = |fe: &mut FrontEnd, m: &[u16], dual: Option<f32>| -> Vec<[f32; 3]> {
        let mut s = sensor(&Spec {
            black: 0.0,
            white: 65535.0,
            ..bayer(w, h)
        });
        s.width = w;
        s.height = h;
        s.crop = Rect { x: 0, y: 0, w, h };
        s.samples = m.to_vec();
        s.meta.wb = [1.0; 3];
        let o = Options {
            output: Output::CameraRgb,
            dual,
            ..opts(Quality::Full)
        };
        let (_, v) = gpu_run(fe, &s, &o);
        v.chunks_exact(4)
            .map(|p| [p[0] / LEVEL, p[1] / LEVEL, p[2] / LEVEL])
            .collect()
    };
    // Flat grey with noise: less residual noise (and pattern) with the dual blend.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut gauss = move || {
        let mut u = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let (a, b) = (u(), u());
        ((-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()) as f32
    };
    let flat: Vec<u16> = (0..w * h)
        .map(|_| ((0.3 + 0.02 * gauss()) * LEVEL * 65535.0).round() as u16)
        .collect();
    // Green at the red and blue sites: where the directional method guesses.
    let spread = |v: &[[f32; 3]]| {
        let inner: Vec<f32> = (8..h - 8)
            .flat_map(|y| (8..w - 8).map(move |x| (x, y)))
            .filter(|&(x, y)| (x + y) % 2 == 0)
            .map(|(x, y)| v[(y * w + x) as usize][1])
            .collect();
        let m = inner.iter().sum::<f32>() / inner.len() as f32;
        (inner.iter().map(|g| (g - m) * (g - m)).sum::<f32>() / inner.len() as f32).sqrt()
    };
    let rcd_n = spread(&run(&mut fe, &flat, None));
    let imgs: Vec<_> = (0..4).map(|k| kodak_like(k, w, h)).collect();
    let detail = |fe: &mut FrontEnd, dual: Option<f32>| {
        imgs.iter()
            .map(|img| psnr(&run(fe, &mosaic(img, w, h, &rggb), dual), img, w, h, 8) / 4.0)
            .sum::<f64>()
    };
    let rcd_p = detail(&mut fe, None);
    eprintln!("RCD: flat noise σ at red/blue sites {rcd_n:.4}, detail PSNR {rcd_p:.2} dB");
    for t in [0.05f32, 0.1, 0.2, 0.3] {
        let n = spread(&run(&mut fe, &flat, Some(t)));
        let p = detail(&mut fe, Some(t));
        eprintln!("dual {t}: flat noise σ {n:.4}, detail PSNR {p:.2} dB");
        if (t - 0.2).abs() < 1e-6 {
            assert!(n < 0.8 * rcd_n);
            assert!(p > rcd_p - 0.5);
        }
    }
}

// ---- Timings ---------------------------------------------------------------------------------

/// Front-end timings (GPU timestamps and wall clock to GPU idle) for 24, 45 and 100 MP Bayer
/// sensors and a 26 MP X-Trans one, preview (half size) and full quality, f16 output.
///
///     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop raw_front_end_timings -- --ignored --nocapture
#[test]
#[ignore = "timing run: prints front-end times for 24/45/100 MP"]
fn raw_front_end_timings() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let sizes: [(&str, u32, u32, Layout); 4] = [
        ("24 MP Bayer", 6000, 4000, Layout::Bayer([0, 1, 1, 2])),
        ("45 MP Bayer", 8256, 5504, Layout::Bayer([0, 1, 1, 2])),
        ("100 MP Bayer", 11664, 8750, Layout::Bayer([0, 1, 1, 2])),
        ("26 MP X-Trans", 6240, 4160, Layout::Pattern(xtrans_cfa())),
    ];
    for (name, w, h, layout) in sizes {
        let mut s = sensor(&Spec {
            layout,
            ..bayer(16, 16)
        });
        s.width = w;
        s.height = h;
        s.crop = Rect { x: 0, y: 0, w, h };
        s.samples = (0..w as usize * h as usize)
            .map(|i| (512 + (i * 2_654_435_761) % 14000) as u16)
            .collect();
        let g = fe.upload(s).unwrap();
        // Also reduced in stripes to a 2560 px view, and with the default
        // front end's corrections (highlights) as the preview runs it.
        let view = crate::preview::fit(w, h, 2560);
        let runs = [
            (Quality::Half, None, false),
            (Quality::Fast, None, false),
            (Quality::Full, None, false),
            (Quality::Full, Some(view), false),
            (Quality::Full, Some(view), true),
        ];
        for (q, reduce_to, front) in runs {
            let base = if front {
                Options::from_params(&RawFrontEnd::default(), 1.0)
            } else {
                Options::new(q)
            };
            let o = Options {
                profile_gpu: true,
                reduce_to,
                ..base
            };
            let q = format!("{q:?}{}", if front { " (default front end)" } else { "" });
            let q = match reduce_to {
                Some((vw, vh)) => format!("{q} to {vw}x{vh}"),
                None => q,
            };
            // Warm up (pipelines, scratch buffers), then the best of three.
            let _ = fe.run(&g, &o).unwrap();
            let mut best = (f64::MAX, f64::MAX);
            let mut bands = 0;
            for _ in 0..3 {
                let r = fe.run(&g, &o).unwrap();
                best.0 = best.0.min(r.stats.gpu_ms.unwrap_or(f64::NAN));
                best.1 = best.1.min(r.stats.wall_ms.unwrap_or(f64::NAN));
                bands = r.stats.bands;
            }
            eprintln!(
                "{name} {q}: GPU {:.1} ms, wall {:.1} ms ({bands} bands)",
                best.0, best.1
            );
        }
        fe.release_scratch();
    }
}
