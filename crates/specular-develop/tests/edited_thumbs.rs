//! Edited thumbnails: the photo developed with its edit at 512 px
//! matches a full-resolution export downscaled to the same size (CIEDE2000), turned and
//! cropped the same way; and the 512 px budget (ignored: a timing report).
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test edited_thumbs
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test edited_thumbs -- \
//!       --ignored --nocapture thumbnail_budget

#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::sync::OnceLock;
use std::time::Instant;

use common::{DngCodec, DngSpec, tag_ascii, write_dng};
use specular_convert::Resize;
use specular_develop::export::batch;
use specular_develop::export::{Depth, Exporter, FileFormat, Preset, Samples, Space};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::params::CropOrientation;
use specular_develop::model::{Edit, ModuleKind, Params, Registry, SourceKey, Value};
use specular_develop::preview::{self, Input, Orient};
use specular_develop::thumb::{self, Source, Thumbnailer};

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn dev() -> Option<&'static Dev> {
    static D: OnceLock<Option<Dev>> = OnceLock::new();
    D.get_or_init(|| match batch::open_device() {
        Ok((device, queue, _)) => Some(Dev { device, queue }),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU tests skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

/// A Bayer (RGGB) DNG of smooth colour fields with fine detail.
fn colour_dng(w: u32, h: u32, orientation: u16, codec: DngCodec) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, codec);
    s.orientation = orientation;
    s.extra = vec![tag_ascii(271, "Specular"), tag_ascii(272, "Thumb Camera")];
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let v = match (y % 2, x % 2) {
                (0, 0) => 0.04 + 0.56 * (0.5 + 0.5 * (fx * 5.0).sin()),
                (1, 1) => 0.04 + 0.56 * (0.5 + 0.5 * ((fx + fy) * 4.0).cos()),
                _ => 0.04 + 0.56 * (0.5 + 0.5 * (fy * 3.0 + 1.0).sin()),
            };
            data.push((v * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// The default raw edit with the file's as-shot white, brighter, warmer and cropped.
fn edit_for(input: &Input, crop: bool) -> Edit {
    let reg = Registry::builtin();
    let mut e = Edit::default_for(reg, SourceKey::default(), true);
    if let Input::Raw(s) = input
        && let Some(xy) = preview::as_shot_white(s)
    {
        batch::set_as_shot(&mut e, xy);
    }
    let m = e.module_mut(ModuleKind::EXPOSURE, 0).expect("exposure");
    let mut f = m.params.to_fields();
    f.set("exposure_ev", Value::Float(0.6));
    m.params.set_fields(f);
    if crop {
        let m = e
            .module_mut(ModuleKind::CROP_ORIENTATION, 0)
            .expect("crop module");
        m.params = Params::CropOrientation(CropOrientation {
            angle_deg: 1.5,
            crop: [0.08, 0.1, 0.9, 0.85],
            ..Default::default()
        });
    }
    e
}

fn lab(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb;
    let x = 0.4123908 * r + 0.3575843 * g + 0.1804808 * b;
    let y = 0.2126390 * r + 0.7151687 * g + 0.0721923 * b;
    let z = 0.0193308 * r + 0.1191948 * g + 0.9505322 * b;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.95047), f(y), f(z / 1.08883));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 (Sharma, Wu and Dalal 2005).
fn de2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let (l1, a1, b1) = (a[0], a[1], a[2]);
    let (l2, a2, b2) = (b[0], b[1], b[2]);
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cb = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cb.powi(7) / (cb.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let c1p = (a1p * a1p + b1 * b1).sqrt();
    let c2p = (a2p * a2p + b2 * b2).sqrt();
    let hue = |b: f64, a: f64| {
        if b == 0.0 && a == 0.0 {
            0.0
        } else {
            let h = b.atan2(a);
            if h < 0.0 { h + 2.0 * PI } else { h }
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let mut dh = h2p - h1p;
    if c1p * c2p == 0.0 {
        dh = 0.0;
    } else if dh > PI {
        dh -= 2.0 * PI;
    } else if dh < -PI {
        dh += 2.0 * PI;
    }
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).sin();
    let lb = (l1 + l2) / 2.0;
    let cbp = (c1p + c2p) / 2.0;
    let hb = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= PI {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 2.0 * PI {
        (h1p + h2p + 2.0 * PI) / 2.0
    } else {
        (h1p + h2p - 2.0 * PI) / 2.0
    };
    let t = 1.0 - 0.17 * (hb - PI / 6.0).cos()
        + 0.24 * (2.0 * hb).cos()
        + 0.32 * (3.0 * hb + PI / 30.0).cos()
        - 0.20 * (4.0 * hb - 63.0 * PI / 180.0).cos();
    let dtheta = 30.0 * PI / 180.0 * (-((hb * 180.0 / PI - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cbp.powi(7) / (cbp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lb - 50.0).powi(2) / (20.0 + (lb - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cbp;
    let sh = 1.0 + 0.015 * cbp * t;
    let rt = -(2.0 * dtheta).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh))
        .sqrt()
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn srgb_decode(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The acceptance check: the edited thumbnail matches the export downscaled to its size.
#[test]
fn edited_thumbnail_matches_the_export_downscaled() {
    let Some(d) = dev() else { return };
    let _g = lock();
    // Upright and turned (EXIF 6), with and without a crop.
    for (orientation, crop) in [(1u16, false), (6, true)] {
        let file = colour_dng(2400, 1600, orientation, DngCodec::U16);
        let input = preview::decode_raw(&file).unwrap();
        let edit = edit_for(&input, crop);
        let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
        let (tw, tt, px) = th
            .render(
                Source {
                    input,
                    orientation,
                    frame: None,
                    lensfun: None,
                    focal35: None,
                },
                &edit,
                thumb::LONG,
            )
            .unwrap();
        assert!(tw.max(tt) <= thumb::LONG + 2, "{tw}×{tt}");
        assert!(tw.max(tt) >= thumb::LONG * 3 / 4, "{tw}×{tt}");
        if orientation == 6 && !crop {
            assert!(tt > tw);
        }
        // The export, 16-bit sRGB, at the thumbnail's size.
        let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
        let preset = Preset {
            format: FileFormat::Png,
            depth: Depth::Sixteen,
            space: Space::Srgb,
            size: Resize::LongEdge(tw.max(tt)),
            ..Preset::default()
        };
        let input = preview::decode_raw(&file).unwrap();
        let img = ex
            .render(
                input,
                &edit,
                Orient::from_exif(orientation),
                &preset,
                &mut |_| true,
            )
            .unwrap();
        assert_eq!(
            (img.width, img.height),
            (tw, tt),
            "orientation {orientation}"
        );
        let Samples::Sixteen(ex_px) = &img.samples else {
            panic!("16-bit")
        };
        let mut de = Vec::with_capacity((tw * tt) as usize);
        for i in 0..(tw * tt) as usize {
            let a = [px[i * 4], px[i * 4 + 1], px[i * 4 + 2]]
                .map(|s| srgb_decode(f64::from(s) / 255.0));
            let b = [ex_px[i * 3], ex_px[i * 3 + 1], ex_px[i * 3 + 2]]
                .map(|s| srgb_decode(f64::from(s) / 65535.0));
            de.push(de2000(lab(a), lab(b)));
        }
        let mean = de.iter().sum::<f64>() / de.len() as f64;
        let p50 = percentile(&mut de, 0.5);
        let p95 = percentile(&mut de, 0.95);
        let p99 = percentile(&mut de, 0.99);
        let max = de[de.len() - 1];
        println!(
            "edited thumbnail vs export ({tw}×{tt}, orientation {orientation}{}): ΔE2000 mean {mean:.3}, p50 {p50:.3}, p95 {p95:.3}, p99 {p99:.3}, max {max:.3}",
            if crop { ", straighten and crop" } else { "" }
        );
        assert!(p95 < 1.0, "p95 {p95}");
    }
}

/// An X-Trans (X-T3 layout) DNG of smooth colour fields with fine detail.
fn xtrans_dng(w: u32, h: u32) -> Vec<u8> {
    const XT: [u8; 36] = [
        1, 1, 2, 1, 1, 0, 2, 0, 1, 0, 2, 1, 1, 1, 2, 1, 1, 0, 1, 1, 0, 1, 1, 2, 0, 2, 1, 2, 0, 1,
        1, 1, 0, 1, 1, 2,
    ];
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.orientation = 1;
    s.extra = vec![
        tag_ascii(271, "Specular"),
        tag_ascii(272, "Thumb X-Trans"),
        (
            0x828d,
            3,
            2,
            [6u16, 6].iter().flat_map(|v| v.to_le_bytes()).collect(),
        ),
        (0x828e, 1, 36, XT.to_vec()),
    ];
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let v = match XT[(y as usize % 6) * 6 + x as usize % 6] {
                0 => 0.04 + 0.56 * (0.5 + 0.5 * (fx * 5.0).sin()),
                2 => 0.04 + 0.56 * (0.5 + 0.5 * ((fx + fy) * 4.0).cos()),
                _ => 0.04 + 0.56 * (0.5 + 0.5 * (fy * 3.0 + 1.0).sin()),
            };
            data.push((v * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// A large X-Trans sensor's thumbnail develops from the sensor binned 3×3 into RGB
/// (no demosaic) and still matches the full export downscaled.
#[test]
fn x_trans_thumbnail_is_binned_and_matches_the_export() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let file = xtrans_dng(2400, 1602);
    let input = preview::decode_raw(&file).unwrap();
    let Input::Raw(s) = &input else {
        panic!("a raw")
    };
    assert!(s.crop.w.max(s.crop.h) >= thumb::BIN_FROM);
    assert!(s.half().is_none() && s.third().is_some());
    let edit = edit_for(&input, false);
    let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (tw, tt, px) = th
        .render(
            Source {
                input,
                orientation: 1,
                frame: None,
                lensfun: None,
                focal35: None,
            },
            &edit,
            thumb::LONG,
        )
        .unwrap();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        size: Resize::LongEdge(tw.max(tt)),
        ..Preset::default()
    };
    let img = ex
        .render(
            preview::decode_raw(&file).unwrap(),
            &edit,
            Orient::from_exif(1),
            &preset,
            &mut |_| true,
        )
        .unwrap();
    assert_eq!((img.width, img.height), (tw, tt));
    let Samples::Sixteen(ex_px) = &img.samples else {
        panic!("16-bit")
    };
    let mut de = de_8_16(&px, ex_px);
    let p95 = percentile(&mut de, 0.95);
    let p99 = percentile(&mut de, 0.99);
    println!(
        "X-Trans binned thumbnail vs export ({tw}×{tt}): ΔE2000 p95 {p95:.3}, p99 {p99:.3}, max {:.3}",
        de[de.len() - 1]
    );
    assert!(p95 < 1.0, "p95 {p95}");
}

/// Non-raw photos develop from the decoded image (linear Rec.2020 at about 1024 px).
#[test]
fn edited_thumbnail_of_a_non_raw() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let (w, h) = (1600u32, 1200u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, 128, 255]);
        }
    }
    let input = preview::rgba8_to_input(w, h, &rgba, thumb::SOURCE_LONG);
    let reg = Registry::builtin();
    let plain = Edit::default_for(reg, SourceKey::default(), false);
    let mut dark = plain.clone();
    let m = dark.module_mut(ModuleKind::EXPOSURE, 0).expect("exposure");
    let mut f = m.params.to_fields();
    f.set("exposure_ev", Value::Float(-1.5));
    m.params.set_fields(f);
    let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let mean = |px: &[u8]| px.iter().map(|&v| f64::from(v)).sum::<f64>() / px.len() as f64;
    let (tw, tt, a) = th
        .render(
            Source {
                input: input.clone(),
                orientation: 1,
                frame: None,
                lensfun: None,
                focal35: None,
            },
            &plain,
            thumb::LONG,
        )
        .unwrap();
    assert_eq!((tw, tt), (512, 384));
    let (_, _, b) = th
        .render(
            Source {
                input,
                orientation: 1,
                frame: None,
                lensfun: None,
                focal35: None,
            },
            &dark,
            thumb::LONG,
        )
        .unwrap();
    assert!(mean(&b) < mean(&a) - 20.0, "{} vs {}", mean(&b), mean(&a));
}

/// The budget: an edited thumbnail at 512 px in 150 ms on the reference
/// integrated GPU, raw decode included. Prints p50 and p95 over 20 photos (24 MP,
/// lossless-compressed) with the load average.
#[test]
#[ignore = "timing report; run by hand"]
fn thumbnail_budget() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let dir = base.join(format!("specular-thumb-budget-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut th = Thumbnailer::new(
        &d.device,
        &d.queue,
        Budget::for_device(true, None, Some(32 << 30)),
    );
    for (mp, w, h) in [(24, 6000u32, 4000u32), (12, 4032, 3024)] {
        let src = dir.join(format!("bench{mp}.dng"));
        std::fs::write(&src, colour_dng(w, h, 1, DngCodec::Lj92Tiles(256))).unwrap();
        let mut times = Vec::new();
        let mut decode = Vec::new();
        for i in 0..21 {
            let t0 = Instant::now();
            let s = thumb::read_raw(&src).unwrap().expect("a raw");
            let t1 = Instant::now();
            let edit = edit_for(&s.input, i % 2 == 0);
            let (tw, tt, _) = th.render(s, &edit, thumb::LONG).unwrap();
            assert!(tw.max(tt) <= thumb::LONG + 2);
            // The first builds the pipelines.
            if i > 0 {
                times.push(t0.elapsed().as_secs_f64() * 1e3);
                decode.push((t1 - t0).as_secs_f64() * 1e3);
            }
        }
        let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
        let p50 = percentile(&mut times, 0.5);
        let p95 = percentile(&mut times, 0.95);
        let d50 = percentile(&mut decode, 0.5);
        println!(
            "edited thumbnail {mp} MP → 512 px: p50 {p50:.0} ms, p95 {p95:.0} ms (decode p50 {d50:.0} ms); load {}",
            load.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        );
        let _ = std::fs::remove_file(&src);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Where an edited thumbnail's time goes (24 MP, waiting for the GPU after each phase):
/// read, decode, upload, front end and reduction, graph, read-back. Prints p50 per phase.
#[test]
#[ignore = "timing report; run by hand"]
fn thumbnail_phases() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let dir = base.join(format!("specular-thumb-phases-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("phases.dng");
    std::fs::write(&src, colour_dng(6000, 4000, 1, DngCodec::Lj92Tiles(256))).unwrap();
    let mut dv = preview::Developer::new(
        &d.device,
        &d.queue,
        Budget::for_device(true, None, Some(32 << 30)),
    );
    // As the thumbnailer does.
    dv.wait_highlight_stats = true;
    // DV_NEUTRAL: as before highlight statistics (no statistics, the neutral grid), for comparison.
    if std::env::var_os("DV_NEUTRAL").is_some() {
        dv.wait_highlight_stats = false;
        dv.highlight_stats = false;
    }
    let names = [
        "read",
        "decode",
        "bin",
        "upload",
        "prepare",
        "graph",
        "read-back",
    ];
    let mut t: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    let mut fe_gpu = Vec::new();
    dv.profile = std::env::var_os("DV_PROFILE").is_some();
    for i in 0..11 {
        let mut lap = Vec::new();
        let mut t0 = Instant::now();
        let mut mark = |lap: &mut Vec<f64>| {
            lap.push(t0.elapsed().as_secs_f64() * 1e3);
            t0 = Instant::now();
        };
        let bytes = std::fs::read(&src).unwrap();
        mark(&mut lap);
        let input = preview::decode_raw(&bytes).unwrap();
        mark(&mut lap);
        let edit = edit_for(&input, i % 2 == 0);
        // As the thumbnailer does: a large sensor binned 2×2 (`thumb::BIN_FROM`).
        let (input, bin) = match input {
            preview::Input::Raw(s) if s.crop.w.max(s.crop.h) >= thumb::BIN_FROM => {
                (preview::Input::Raw(Box::new(s.half().unwrap())), 2.0)
            }
            other => (other, 1.0),
        };
        mark(&mut lap);
        dv.set_input(input).unwrap();
        dv.sensor_bin = bin;
        dv.wait().unwrap();
        mark(&mut lap);
        dv.base = Orient::from_exif(1);
        dv.crop_on = true;
        dv.prepare(&edit, thumb::LONG).unwrap();
        dv.wait().unwrap();
        mark(&mut lap);
        fe_gpu.extend(dv.stats.front_end_gpu_ms);
        let r = dv
            .render(
                &edit,
                None,
                &specular_develop::ops::output::OutputSpace::rec709(),
            )
            .unwrap();
        dv.wait().unwrap();
        mark(&mut lap);
        let _ = dv.read_rgba8(r, Orient::default()).unwrap();
        mark(&mut lap);
        if i > 0 {
            for (v, ms) in t.iter_mut().zip(lap) {
                v.push(ms);
            }
        }
    }
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    let line: Vec<String> = names
        .iter()
        .zip(&mut t)
        .map(|(n, v)| format!("{n} {:.1}", percentile(v, 0.5)))
        .collect();
    println!(
        "edited thumbnail 24 MP phases (p50 ms): {}; front end GPU {:.1}; load {}",
        line.join(", "),
        if fe_gpu.is_empty() {
            f64::NAN
        } else {
            percentile(&mut fe_gpu, 0.5)
        },
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A raw that clips in green and red at its bright end, and a blown sun.
fn clipped_dng(w: u32, h: u32) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.orientation = 1;
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let d = ((u - 0.7).powi(2) + (v - 0.3).powi(2)).sqrt();
            let k = (y % 2) * 2 + (x % 2);
            let c = [0.5 + 0.6 * u, 0.6 + 0.7 * u, 0.6 + 0.7 * u, 0.4 + 0.3 * u][k as usize];
            let c = if d < 0.08 { 1.5 } else { c };
            data.push((c.min(1.0) * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// ΔE2000 per pixel between 8-bit sRGB RGBA and 16-bit sRGB RGB rows.
fn de_8_16(a: &[u8], b: &[u16]) -> Vec<f64> {
    (0..a.len() / 4)
        .map(|i| {
            let x =
                [a[i * 4], a[i * 4 + 1], a[i * 4 + 2]].map(|s| srgb_decode(f64::from(s) / 255.0));
            let y =
                [b[i * 3], b[i * 3 + 1], b[i * 3 + 2]].map(|s| srgb_decode(f64::from(s) / 65535.0));
            de2000(lab(x), lab(y))
        })
        .collect()
}

/// A clipped raw's thumbnail uses the photo's own highlight statistics (it waits
/// for them), as the full render does, not the neutral grid. Compared with the export
/// downscaled to the thumbnail's size; the neutral grid's thumbnail is printed beside it.
#[test]
fn clipped_thumbnail_matches_the_export_downscaled() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let file = clipped_dng(2400, 1600);
    let input = preview::decode_raw(&file).unwrap();
    let edit = edit_for(&input, false);
    let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (tw, tt, px) = th
        .render(
            Source {
                input,
                orientation: 1,
                frame: None,
                lensfun: None,
                focal35: None,
            },
            &edit,
            thumb::LONG,
        )
        .unwrap();
    // The same thumbnail with the neutral grid (as before: no statistics).
    let neutral = {
        let input = preview::decode_raw(&file).unwrap();
        let mut dv = preview::Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
        dv.highlight_stats = false;
        dv.set_input(input).unwrap();
        dv.crop_on = true;
        dv.prepare(&edit, thumb::LONG).unwrap();
        let r = dv
            .render(
                &edit,
                None,
                &specular_develop::ops::output::OutputSpace::rec709(),
            )
            .unwrap();
        let (w, h, px) = dv.read_rgba8(r, Orient::default()).unwrap();
        thumb::fit_rgba8(w, h, &px, thumb::LONG).2
    };
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        size: Resize::LongEdge(tw.max(tt)),
        ..Preset::default()
    };
    let img = ex
        .render(
            preview::decode_raw(&file).unwrap(),
            &edit,
            Orient::default(),
            &preset,
            &mut |_| true,
        )
        .unwrap();
    assert_eq!((img.width, img.height), (tw, tt));
    let Samples::Sixteen(ex_px) = &img.samples else {
        panic!("16-bit")
    };
    let mut de = de_8_16(&px, ex_px);
    let mut dn = de_8_16(&neutral, ex_px);
    let p95 = percentile(&mut de, 0.95);
    let p99 = percentile(&mut de, 0.99);
    let n95 = percentile(&mut dn, 0.95);
    let n99 = percentile(&mut dn, 0.99);
    println!(
        "clipped raw thumbnail vs export ({tw}×{tt}): ΔE2000 p95 {p95:.3}, p99 {p99:.3}, max {:.3}; neutral grid p95 {n95:.3}, p99 {n99:.3}, max {:.3}",
        de[de.len() - 1],
        dn[dn.len() - 1]
    );
    assert!(p95 < 1.0, "p95 {p95}");
    assert!(p99 <= n99, "{p99} vs neutral {n99}");
}

/// A Bayer DNG of an even grey field darkened towards the corners by `falloff` (0: even).
fn vignetted_dng(w: u32, h: u32, level: f32, falloff: f32) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.extra = vec![tag_ascii(271, "Specular"), tag_ascii(272, "Thumb Camera")];
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32 - 0.5, y as f32 / h as f32 - 0.5);
            let r2 = (u * u + v * v) * 2.0;
            let v = level * (1.0 - falloff * r2);
            data.push((v * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// The flat frame an edit names is applied to its thumbnail as
/// to the export; without it the thumbnail's corners stay dark.
#[test]
fn thumbnails_take_the_flat_frame() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let dir = base.join(format!("specular-thumb-flat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h) = (1200u32, 800u32);
    let photo = dir.join("photo.dng");
    let flat = dir.join("flat.dng");
    std::fs::write(&photo, vignetted_dng(w, h, 0.3, 0.5)).unwrap();
    std::fs::write(&flat, vignetted_dng(w, h, 0.6, 0.5)).unwrap();
    let s = thumb::read_raw(&photo).unwrap().expect("a raw");
    let mut edit = edit_for(&s.input, false);
    let plain = edit.clone();
    if edit.module(ModuleKind::FLAT_FIELD, 0).is_none() {
        let reg = Registry::builtin();
        edit.modules
            .push(specular_develop::model::ModuleInstance::new(
                reg,
                ModuleKind::FLAT_FIELD,
                0,
            ));
        edit.sort(reg);
    }
    let m = edit
        .module_mut(ModuleKind::FLAT_FIELD, 0)
        .expect("flat-field module");
    let mut f = m.params.to_fields();
    f.set("flat", Value::Text(flat.display().to_string()));
    m.params.set_fields(f);
    m.enabled = true;
    let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (tw, tt, px) = th.render(s, &edit, thumb::LONG).unwrap();
    let s = thumb::read_raw(&photo).unwrap().expect("a raw");
    let (_, _, unflat) = th.render(s, &plain, thumb::LONG).unwrap();
    // The export with the same frame.
    let bytes = std::fs::read(&photo).unwrap();
    let input = preview::decode_raw(&bytes).unwrap();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    if let Input::Raw(s) = &input {
        ex.calibration = specular_develop::ops::correct::calib::for_edit(
            &edit,
            s,
            Some((photo.as_path(), &bytes)),
        )
        .unwrap();
    }
    assert!(ex.calibration.is_some());
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        size: Resize::LongEdge(tw.max(tt)),
        ..Preset::default()
    };
    let img = ex
        .render(input, &edit, Orient::default(), &preset, &mut |_| true)
        .unwrap();
    assert_eq!((img.width, img.height), (tw, tt));
    let Samples::Sixteen(ex_px) = &img.samples else {
        panic!("16-bit")
    };
    let mut de = de_8_16(&px, ex_px);
    let mut du = de_8_16(&unflat, ex_px);
    let p95 = percentile(&mut de, 0.95);
    let u95 = percentile(&mut du, 0.95);
    // Corner over centre: even once the flat divides the falloff out.
    let grey = |px: &[u8], x: u32, y: u32| f64::from(px[((y * tw + x) * 4 + 1) as usize]);
    let ratio = |px: &[u8]| grey(px, 2, 2) / grey(px, tw / 2, tt / 2);
    println!(
        "flat-framed thumbnail vs export ({tw}×{tt}): ΔE2000 p95 {p95:.3} (without the flat {u95:.3}); corner/centre {:.3} (without {:.3})",
        ratio(&px),
        ratio(&unflat)
    );
    assert!(p95 < 1.0, "p95 {p95}");
    assert!(u95 > 3.0, "{u95}");
    assert!((ratio(&px) - 1.0).abs() < 0.05);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A Bayer (RGGB) DNG of colour fields with texture of `tex` at a 40-pixel period (fine enough
/// that a frequency separation's radius matters) and a hard-edged dark square.
fn textured_dng(w: u32, h: u32, tex: f32) -> Vec<u8> {
    let s = DngSpec::new(w, h, DngCodec::U16);
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let t = tex * (x as f32 * 0.15).sin() * (y as f32 * 0.13).cos();
            let base = match (y % 2, x % 2) {
                (0, 0) => 0.3 + 0.2 * (fx * 5.0).sin(),
                (1, 1) => 0.3 + 0.2 * ((fx + fy) * 4.0).cos(),
                _ => 0.3 + 0.2 * (fy * 3.0 + 1.0).sin(),
            };
            let dark = (0.55..0.7).contains(&fx) && (0.2..0.4).contains(&fy);
            let v = if dark { 0.05 } else { base + t };
            data.push((v.clamp(0.0, 1.0) * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// `edit_for` with masked exposure (a feathered ellipse grown by a refine feather, a brush
/// stroke blurred by a refine blur) and retouch spots of each kind (heal, clone, blur, a
/// high-frequency heal through a frequency separation), all sized in the frame.
fn masked_retouched(input: &Input) -> Edit {
    use specular_develop::model::mask::{PressureUse, Stroke};
    use specular_develop::model::{Blend, MaskExpr, MaskShape, ModuleInstance, Refine, Shape};
    use specular_develop::ops::retouch::{Layer, Retouch, Spot, SpotMode};
    let reg = Registry::builtin();
    let mut e = edit_for(input, false);
    e.masks.push(MaskShape {
        id: 1,
        shape: Shape::Ellipse {
            centre: [0.3, 0.45],
            radii: [0.12, 0.08],
            angle_deg: 20.0,
            feather: 0.4,
        },
    });
    e.masks.push(MaskShape {
        id: 2,
        shape: Shape::Brush {
            strokes: vec![Stroke {
                radius: 0.015,
                hardness: 0.4,
                flow: 1.0,
                erase: false,
                pressure: PressureUse::Flow,
                points: (0..40)
                    .map(|i| {
                        let t = i as f32 / 39.0;
                        [0.55 + 0.3 * t, 0.7 + 0.1 * (t * 6.0).sin(), 1.0]
                    })
                    .collect(),
            }],
        },
    });
    for (inst, ev, shape, refine) in [
        (
            1u16,
            1.0f32,
            1u32,
            Refine {
                feather: 0.01,
                ..Refine::default()
            },
        ),
        (
            2,
            -1.2,
            2,
            Refine {
                blur: 0.004,
                ..Refine::default()
            },
        ),
    ] {
        let mut m = ModuleInstance::new(reg, ModuleKind::EXPOSURE, inst);
        let mut f = m.params.to_fields();
        f.set("exposure_ev", Value::Float(ev));
        m.params.set_fields(f);
        m.blend = Some(Blend {
            mask: MaskExpr::Shape(shape),
            refine,
            ..Blend::default()
        });
        e.insert(reg, m);
    }
    let spot = |mode, layer, target: [f32; 2], source: [f32; 2], size: f32| Spot {
        mode,
        layer,
        target,
        source,
        size,
        feather: 0.3,
        blur: 0.006,
        ..Spot::default()
    };
    let r = Retouch {
        split_radius: 6.0,
        spots: vec![
            spot(SpotMode::Heal, Layer::All, [0.62, 0.3], [0.4, 0.2], 0.02),
            spot(SpotMode::Clone, Layer::All, [0.2, 0.75], [0.3, 0.8], 0.015),
            spot(SpotMode::Blur, Layer::All, [0.8, 0.5], [0.8, 0.5], 0.025),
            spot(SpotMode::Heal, Layer::High, [0.45, 0.6], [0.5, 0.75], 0.03),
        ],
    };
    let mut m = ModuleInstance::new(reg, ModuleKind::RETOUCH, 0);
    m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
    e.insert(reg, m);
    e
}

/// A binned thumbnail (2×2 per colour) with masks and retouch spots matches the
/// export downscaled to its size, as an unbinned one does: lengths are fractions of the
/// frame, and the frequency separation's radius (source pixels) stays the sensor's.
#[test]
fn binned_thumbnails_keep_mask_and_spot_sizes() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let tex = std::env::var("THUMB_TEXTURE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.03f32);
    // 2400 px is binned (from 2048), 2000 px is not.
    for (w, h) in [(2400u32, 1600u32), (2000, 1334)] {
        let file = textured_dng(w, h, tex);
        let mut plain = 0.0;
        for masked in [false, true] {
            let input = preview::decode_raw(&file).unwrap();
            let edit = if masked {
                masked_retouched(&input)
            } else {
                edit_for(&input, false)
            };
            let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
            let (tw, tt, px) = th
                .render(
                    Source {
                        input,
                        orientation: 1,
                        frame: None,
                        lensfun: None,
                        focal35: None,
                    },
                    &edit,
                    thumb::LONG,
                )
                .unwrap();
            let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
            let preset = Preset {
                format: FileFormat::Png,
                depth: Depth::Sixteen,
                space: Space::Srgb,
                size: Resize::LongEdge(tw.max(tt)),
                ..Preset::default()
            };
            let img = ex
                .render(
                    preview::decode_raw(&file).unwrap(),
                    &edit,
                    Orient::default(),
                    &preset,
                    &mut |_| true,
                )
                .unwrap();
            assert_eq!((img.width, img.height), (tw, tt));
            let Samples::Sixteen(ex_px) = &img.samples else {
                panic!("16-bit")
            };
            let mut de = de_8_16(&px, ex_px);
            if std::env::var_os("THUMB_WHERE").is_some() {
                let mut worst: Vec<(f64, usize)> = de.iter().copied().zip(0..).collect();
                worst.sort_by(|a, b| b.0.total_cmp(&a.0));
                for (e, i) in worst.iter().take(5) {
                    println!("  ΔE {e:.2} at ({}, {})", *i as u32 % tw, *i as u32 / tw);
                }
            }
            let p95 = percentile(&mut de, 0.95);
            let p99 = percentile(&mut de, 0.99);
            println!(
                "{w}×{h} ({}) {} thumbnail vs export ({tw}×{tt}): ΔE2000 p95 {p95:.3}, p99 {p99:.3}, max {:.3}",
                if w >= 2048 { "binned" } else { "not binned" },
                if masked {
                    "masked and retouched"
                } else {
                    "plain"
                },
                de[de.len() - 1]
            );
            assert!(p95 < 1.0, "p95 {p95}");
            // The masks and spots add next to nothing to the plain edit's difference.
            if masked {
                assert!(p95 <= plain + 0.1, "p95 {p95} vs plain {plain}");
            } else {
                plain = p95;
            }
        }
    }
}

/// A raw developed larger than its thumbnail is developed at a whole multiple of
/// it (768 px rounds to 1024: at 1.5× the second reduction blurs), and the thumbnail keeps
/// its size.
#[test]
fn raws_develop_larger_by_whole_factors() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let file = colour_dng(1800, 1200, 1, DngCodec::U16);
    let input = preview::decode_raw(&file).unwrap();
    let edit = edit_for(&input, false);
    let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    // Sharper raw thumbnails by default (Settings › Gallery): developed at 1024 px.
    assert_eq!(th.raw_long, Some(thumb::RAW_LONG));
    assert_eq!((thumb::raw_long(true), thumb::raw_long(false)), (1024, 512));
    let mut render = |raw_long: Option<u32>| {
        th.raw_long = raw_long;
        th.render(
            Source {
                input: input.clone(),
                orientation: 1,
                frame: None,
                lensfun: None,
                focal35: None,
            },
            &edit,
            thumb::LONG,
        )
        .unwrap()
    };
    let at_512 = render(None);
    let at_768 = render(Some(768));
    let at_1024 = render(Some(1024));
    assert_eq!((at_512.0, at_512.1), (512, 341));
    assert_eq!((at_1024.0, at_1024.1), (512, 341));
    assert!(at_768 == at_1024, "768 develops at 1024");
    assert!(at_512.2 != at_1024.2, "1024 develops larger");
    // Up to 1.49× rounds down to the thumbnail's own size.
    assert!(render(Some(700)) == at_512);
    // The setting's two sizes.
    assert!(render(Some(thumb::raw_long(false))) == at_512);
    assert!(render(Some(thumb::raw_long(true))) == at_1024);
}

/// Only raws depend on the develop size: other images' thumbnails keep their cache keys.
#[test]
fn only_raws_take_the_raw_develop_size() {
    use std::path::Path;
    for p in ["a.jpg", "b.PNG", "c.heic", "d.tif", "e.exr", "f.mov"] {
        assert!(!thumb::may_be_raw(Path::new(p)), "{p}");
    }
    for p in ["a.CR2", "b.nef", "c.dng", "d.raf", "e.crm"] {
        assert!(thumb::may_be_raw(Path::new(p)), "{p}");
    }
}
