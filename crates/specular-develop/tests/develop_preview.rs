//! The preview end to end: a synthetic raw through the front end, the basic ops and the
//! present pass; and the slider latency budgets (ignored: a timing report).
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test develop_preview
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test develop_preview -- \
//!       --ignored --nocapture slider_latency

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::sync::OnceLock;
use std::time::Instant;

use common::{DngCodec, DngSpec, write_dng};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::{
    Edit, Fields, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value,
};
use specular_develop::ops::output::OutputSpace;
use specular_develop::preview::{self, Developer, Input, Orient};

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
    name: String,
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
        label: Some("develop preview tests"),
        required_features: features,
        required_limits: limits,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("no device: {e}"))?;
    Ok(Dev {
        device,
        queue,
        name: format!("{} ({:?})", info.name, info.backend),
    })
}

fn set(e: &mut Edit, kind: ModuleKind, name: &str, v: Value) {
    let m = e.module_mut(kind, 0).expect("module");
    let mut f = m.params.to_fields();
    f.set(name, v);
    m.params.set_fields(f);
}

/// A Bayer DNG of a flat 18 % grey, white at 65535, identity colour matrix, neutral
/// as-shot white.
fn grey_dng(w: u32, h: u32) -> Vec<u8> {
    let s = DngSpec::new(w, h, DngCodec::U16);
    let v = (0.18 * 65535.0f32).round() as u16;
    write_dng(&s, &vec![v; (w * h) as usize])
}

#[test]
fn a_raw_develops_to_neutral_mid_grey() {
    let Some(d) = dev() else { return };
    let file = grey_dng(1200, 800);
    let input = preview::decode_raw(&file).expect("raw");
    let Input::Raw(s) = &input else {
        panic!("sensor")
    };
    // AsShotNeutral 1,1,1 through the identity matrix: equal-energy white.
    let xy = preview::as_shot_white(s).expect("as-shot white");
    assert!((xy[0] - 1.0 / 3.0).abs() < 1e-9 && (xy[1] - 1.0 / 3.0).abs() < 1e-9);
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.set_input(input).unwrap();
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    for (n, v) in [
        ("as_shot_x", xy[0]),
        ("as_shot_y", xy[1]),
        ("white_x", xy[0]),
        ("white_y", xy[1]),
    ] {
        set(
            &mut e,
            ModuleKind::COLOUR_CALIBRATION,
            n,
            Value::Float(v as f32),
        );
    }
    // Half size from the binning, then reduced to 500 px.
    assert_eq!(dv.prepare(&e, 500).unwrap(), (500, 333));
    let r = dv.render(&e, None, &OutputSpace::rec709()).unwrap();
    let (w, h, px) = dv.read_rgba8(r, Orient::default()).unwrap();
    assert_eq!((w, h), (500, 333));
    let c = &px[((h / 2 * w + w / 2) * 4) as usize..][..4];
    // Scene 0.18 lands near display 0.1845: sRGB 0.47 → about 119.
    assert!(c[..3].iter().all(|&v| (115..=123).contains(&v)), "{c:?}");
    assert!(
        c[0].abs_diff(c[1]) <= 1 && c[2].abs_diff(c[1]) <= 1,
        "{c:?}"
    );
    // One EV up brightens it; a warmer white makes it bluer.
    set(
        &mut e,
        ModuleKind::EXPOSURE,
        "exposure_ev",
        Value::Float(1.0),
    );
    set(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_source",
        Value::Text("custom".into()),
    );
    set(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_x",
        Value::Float(0.4476),
    );
    set(
        &mut e,
        ModuleKind::COLOUR_CALIBRATION,
        "white_y",
        Value::Float(0.4074),
    );
    let r = dv.render(&e, None, &OutputSpace::rec709()).unwrap();
    let (_, _, px) = dv.read_rgba8(r, Orient::default()).unwrap();
    let c2 = &px[((h / 2 * w + w / 2) * 4) as usize..][..4];
    assert!(c2[1] > c[1] + 20, "{c2:?}");
    assert!(c2[2] > c2[0] + 30, "{c2:?}");
    eprintln!(
        "raw preview on {}: grey {c:?}, +1 EV under tungsten white {c2:?}",
        d.name
    );
}

/// Writes a synthetic scene as a Bayer DNG for screenshots (no licence questions): a sky
/// gradient, colour patches, a grey ramp and a bright sun clipping in one channel, with a
/// warm as-shot white. `SPECULAR_SHOT_RAW=<path.dng>`.
#[test]
#[ignore = "writes a file for screenshots"]
fn write_shot_raw() {
    let Some(path) = std::env::var_os("SPECULAR_SHOT_RAW") else {
        return;
    };
    let (w, h) = (3000u32, 2000u32);
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.color_matrix = common::XYZ_TO_SRGB;
    // Shot under a warm light: blue reads low, the neutral says so.
    s.neutral = [0.9, 1.0, 0.6];
    let patches: [[f32; 3]; 12] = [
        [0.40, 0.23, 0.17],
        [0.75, 0.50, 0.40],
        [0.25, 0.33, 0.50],
        [0.22, 0.28, 0.12],
        [0.35, 0.35, 0.60],
        [0.30, 0.60, 0.55],
        [0.80, 0.40, 0.08],
        [0.18, 0.20, 0.55],
        [0.70, 0.22, 0.25],
        [0.20, 0.10, 0.25],
        [0.55, 0.70, 0.15],
        [0.85, 0.60, 0.10],
    ];
    let mut data = vec![0u16; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut c = if v < 0.45 {
                // Sky: deeper blue at the top, a sun at the upper right.
                let k = v / 0.45;
                let sky = [0.10 + 0.25 * k, 0.20 + 0.25 * k, 0.55 + 0.1 * k];
                let d = ((u - 0.78).powi(2) + (v - 0.15).powi(2)).sqrt();
                let sun =
                    (1.0 - d / 0.06).max(0.0) * 30.0 + (1.0 - d / 0.25).max(0.0).powi(2) * 2.0;
                [sky[0] + sun, sky[1] + sun * 0.9, sky[2] + sun * 0.7]
            } else if v < 0.85 {
                // Two rows of six patches with dark gaps.
                let (px, py) = (u * 6.0, (v - 0.45) / 0.2);
                let (i, j) = (px as usize, py as usize);
                if px.fract() < 0.08 || py.fract() < 0.08 {
                    [0.03, 0.03, 0.03]
                } else {
                    patches[(j.min(1) * 6 + i.min(5)) % 12]
                }
            } else {
                // A grey ramp over 10 EV.
                let g = 0.002 * (u * 10.0).exp2();
                [g, g, g]
            };
            // Camera RGB = linear sRGB under the warm light, exposed so white sits at 0.25.
            for (k, n) in c.iter_mut().zip(s.neutral) {
                *k *= 0.25 * n as f32;
            }
            let ch = match (y % 2, x % 2) {
                (0, 0) => 0,
                (1, 1) => 2,
                _ => 1,
            };
            data[(y * w + x) as usize] = (c[ch] * 65535.0).clamp(0.0, 65535.0) as u16;
        }
    }
    std::fs::write(path, write_dng(&s, &data)).unwrap();
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// A 33³ .cube for the slider bench (a gentle warm look), written once under the temp dir.
fn bench_lut() -> String {
    let p = std::env::temp_dir().join("dv8-bench-33.cube");
    let n = 33;
    let mut s = format!("LUT_3D_SIZE {n}\n");
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let t = |i: usize| i as f64 / (n - 1) as f64;
                let (r, g, b) = (t(r), t(g), t(b));
                s += &format!(
                    "{:.6} {:.6} {:.6}\n",
                    (r * 1.05 + 0.02).min(1.0),
                    g,
                    b * 0.92 + 0.01 * r
                );
            }
        }
    }
    std::fs::write(&p, s).unwrap();
    p.to_string_lossy().into_owned()
}

/// Sliders at fit view of a 2560×1600 window: a point op (exposure,
/// curves; p50 ≤ 8 ms GPU and ≤ 16 ms to present, p95 ≤ 33 ms) and neighbourhood ops (tone
/// equalizer, texture, clarity; p50 ≤ 33 ms, p95 ≤ 50 ms), with the tone modules in
/// the edit; geometry sliders (straighten, lens distortion) re-run the warp and the whole
/// graph after it (held to the neighbourhood budget). Measured: render + present into a texture, to the GPU finishing. The machine
/// may be under load; nothing is asserted.
#[test]
#[ignore = "timing report; run by hand"]
fn slider_latency() {
    let Some(d) = dev() else { return };
    let (w, h) = (2560u32, 1600u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = x as f32 / w as f32;
            let s = y as f32 / h as f32;
            let tex = 0.2 * ((x as f32 * 0.37).sin() * (y as f32 * 0.23).cos());
            rgba.extend_from_slice(&[
                (t * 10.0 - 7.0 + tex).exp2(),
                0.18 * s + 0.01,
                0.3 * (1.0 - t),
                1.0,
            ]);
        }
    }
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.set_input(Input::Rgb {
        width: w,
        height: h,
        rgba,
    })
    .unwrap();
    let reg = Registry::builtin();
    let mut e = Edit::default_for(reg, SourceKey::default(), false);
    for (kind, f) in [
        (
            ModuleKind::TONE_EQUALIZER,
            Fields::new().with("shadows_ev", Value::Float(0.5)),
        ),
        (
            ModuleKind::LOCAL_CONTRAST,
            Fields::new()
                .with("texture", Value::Float(0.3))
                .with("clarity", Value::Float(0.3)),
        ),
        (
            ModuleKind::CURVES,
            Fields::new().with("points", Value::Floats(vec![0.0, 0.0, 0.5, 0.55, 1.0, 1.0])),
        ),
        // Colour balance, the colour mixer, point colour and a 33³ LUT, all active.
        (
            ModuleKind::COLOUR_BALANCE,
            Fields::new()
                .with("vibrance", Value::Float(0.3))
                .with("shadows_chroma", Value::Float(0.2))
                .with("shadows_hue", Value::Float(220.0)),
        ),
        (
            ModuleKind::COLOUR_EQUALIZER,
            Fields::new().with("saturation_blue", Value::Float(0.3)),
        ),
        (
            ModuleKind::POINT_COLOUR,
            Fields::new()
                .with("picked", Value::Bool(true))
                .with("target_hue", Value::Float(40.0))
                .with("target_saturation", Value::Float(0.3))
                .with("shift_hue", Value::Float(10.0)),
        ),
        (
            ModuleKind::LUT_3D,
            Fields::new()
                .with("path", Value::Text(bench_lut()))
                .with("space", Value::Text("acescct".into())),
        ),
        (
            ModuleKind::LENS,
            Fields::new().with("distortion", Value::Float(5.0)),
        ),
    ] {
        let mut m = ModuleInstance::new(reg, kind, 0);
        m.params = Params::from_fields(kind, f);
        e.insert(reg, m);
    }
    dv.prepare(&e, w).unwrap();
    let tex = d.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let out = OutputSpace::rec709();
    type Case = (&'static str, ModuleKind, &'static str, fn(i32) -> Value);
    let cases: [Case; 13] = [
        (
            "colour balance",
            ModuleKind::COLOUR_BALANCE,
            "vibrance",
            |i| Value::Float(0.1 + (i % 40) as f32 * 0.02),
        ),
        (
            "colour mixer",
            ModuleKind::COLOUR_EQUALIZER,
            "saturation_blue",
            |i| Value::Float(0.1 + (i % 40) as f32 * 0.02),
        ),
        ("point colour", ModuleKind::POINT_COLOUR, "shift_hue", |i| {
            Value::Float(5.0 + (i % 40) as f32)
        }),
        ("LUT strength", ModuleKind::LUT_3D, "strength", |i| {
            Value::Float(0.2 + (i % 40) as f32 * 0.02)
        }),
        ("exposure", ModuleKind::EXPOSURE, "exposure_ev", |i| {
            Value::Float((i % 40) as f32 * 0.05)
        }),
        ("curves", ModuleKind::CURVES, "points", |i| {
            Value::Floats(vec![0.0, 0.0, 0.5, 0.5 + (i % 20) as f32 * 0.01, 1.0, 1.0])
        }),
        (
            "tone equalizer",
            ModuleKind::TONE_EQUALIZER,
            "shadows_ev",
            |i| Value::Float(0.2 + (i % 40) as f32 * 0.05),
        ),
        ("texture", ModuleKind::LOCAL_CONTRAST, "texture", |i| {
            Value::Float(0.1 + (i % 40) as f32 * 0.02)
        }),
        (
            "straighten",
            ModuleKind::CROP_ORIENTATION,
            "angle_deg",
            |i| Value::Float(0.5 + (i % 40) as f32 * 0.1),
        ),
        ("lens distortion", ModuleKind::LENS, "distortion", |i| {
            Value::Float(5.0 + (i % 40) as f32)
        }),
        ("clarity", ModuleKind::LOCAL_CONTRAST, "clarity", |i| {
            Value::Float(0.1 + (i % 40) as f32 * 0.02)
        }),
        (
            "exposure, tone modules at rest",
            ModuleKind::EXPOSURE,
            "exposure_ev",
            |i| Value::Float((i % 40) as f32 * 0.05),
        ),
        (
            "straighten, tone modules at rest",
            ModuleKind::CROP_ORIENTATION,
            "angle_deg",
            |i| Value::Float(0.5 + (i % 40) as f32 * 0.1),
        ),
    ];
    for (label, kind, name, val) in cases {
        // `DV_CASE=<part of a label>` times only those cases.
        if std::env::var("DV_CASE").is_ok_and(|f| !label.contains(f.as_str())) {
            continue;
        }
        if label.ends_with("at rest") {
            set(
                &mut e,
                ModuleKind::TONE_EQUALIZER,
                "shadows_ev",
                Value::Float(0.0),
            );
            set(
                &mut e,
                ModuleKind::LOCAL_CONTRAST,
                "texture",
                Value::Float(0.0),
            );
            set(
                &mut e,
                ModuleKind::LOCAL_CONTRAST,
                "clarity",
                Value::Float(0.0),
            );
        }
        let focus = Some((kind, 0));
        let mut wall = Vec::new();
        let mut before = dv.stats.pipelines_created;
        for i in 0..130 {
            set(&mut e, kind, name, val(i));
            let t0 = Instant::now();
            let r = dv.render(&e, focus, &out).unwrap();
            dv.present(r, Orient::default(), &tex).unwrap();
            dv.wait().unwrap();
            if i == 1 {
                before = dv.stats.pipelines_created;
            }
            if i >= 30 {
                wall.push(t0.elapsed().as_secs_f64() * 1e3);
            }
        }
        let made = dv.stats.pipelines_created - before;
        // GPU time of the graph alone (timestamp queries, when the device has them).
        dv.profile = true;
        let mut gpu = Vec::new();
        let mut by_kernel: Vec<(String, f64, usize)> = Vec::new();
        for i in 0..40 {
            set(&mut e, kind, name, val(i));
            let r = dv.render(&e, focus, &out).unwrap();
            if let Some(ms) = dv.stats.render_gpu_ms {
                gpu.push(ms);
            }
            if i == 39 && std::env::var("DV_PROFILE").is_ok_and(|v| v == "2") {
                for (k, ms) in r.stats.gpu_ms.iter().flatten() {
                    eprintln!("      {k}: {ms:.3}");
                }
            }
            if i == 39 {
                for (k, ms) in r.stats.gpu_ms.iter().flatten() {
                    match by_kernel.iter_mut().find(|(n, _, _)| n == k) {
                        Some(e) => {
                            e.1 += ms;
                            e.2 += 1;
                        }
                        None => by_kernel.push((k.clone(), *ms, 1)),
                    }
                }
            }
            dv.recycle(r);
        }
        by_kernel.sort_by(|a, b| b.1.total_cmp(&a.1));
        if std::env::var_os("DV_PROFILE").is_some() {
            for (k, ms, n) in by_kernel.iter().take(12) {
                eprintln!("    {k}: {ms:.2} ms ({n} dispatches)");
            }
        }
        dv.profile = false;
        let (p50, p95) = (percentile(&mut wall, 0.5), percentile(&mut wall, 0.95));
        eprintln!(
            "{label} slider on {} ({w}x{h}): render+present to GPU done p50 {p50:.2} ms, \
             p95 {p95:.2} ms; pipelines made while sliding {made}",
            d.name
        );
        if !gpu.is_empty() {
            eprintln!(
                "  graph GPU time p50 {:.2} ms, p95 {:.2} ms",
                percentile(&mut gpu, 0.5),
                percentile(&mut gpu, 0.95)
            );
        }
    }
}
