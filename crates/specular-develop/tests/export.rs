//! Export end to end: a synthetic raw exported through the same graph as
//! the preview, compared with the fit view (CIEDE2000, CPU colour maths), written in every
//! format and read back with our own decoders; and the 24/45/100 MP budgets (ignored: a
//! timing report).
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test export
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test export -- \
//!       --ignored --nocapture export_budgets

#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use common::{DngCodec, DngSpec, op_warp, opcode_list, tag_ascii, write_dng};
use specular_convert::{Conflict, MetaMode, OutputDir, Resize};
use specular_core::CancelToken;
use specular_develop::export::batch::{self, Job, Outcome};
use specular_develop::export::{Depth, Exporter, FileFormat, Preset, Samples, Space, Transfer};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::params::{CropOrientation, ModuleParams};
use specular_develop::model::{
    Edit, ModuleInstance, ModuleKind, Params, Registry, SourceKey, Value,
};
use specular_develop::ops::geometry::LensParams;
use specular_develop::ops::output::OutputSpace;
use specular_develop::preview::{self, Developer, Input, Orient};

struct Dev {
    device: wgpu::Device,
    queue: wgpu::Queue,
    name: String,
}

fn dev() -> Option<&'static Dev> {
    static D: OnceLock<Option<Dev>> = OnceLock::new();
    D.get_or_init(|| match batch::open_device() {
        Ok((device, queue, _)) => {
            let name = "export test device".to_owned();
            Some(Dev {
                device,
                queue,
                name,
            })
        }
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU tests skipped");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    })
    .as_ref()
}

/// GPU tests one at a time.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn tmpdir(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let d = base.join(format!("specular-export-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A Bayer (RGGB) DNG of smooth colour fields: red across, green down, blue on the
/// diagonal, all between 4 % and 60 % of white, with a camera make and model.
fn colour_dng(w: u32, h: u32, orientation: u16) -> Vec<u8> {
    colour_dng_with(w, h, orientation, false, 0.0)
}

/// [`colour_dng`], optionally with a three-plane `WarpRectilinear` in `OpcodeList3`.
/// `noise`: the amplitude of uniform noise added to every sample (fraction of white).
fn colour_dng_with(w: u32, h: u32, orientation: u16, warp: bool, noise: f32) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.orientation = orientation;
    s.extra = vec![tag_ascii(271, "Specular"), tag_ascii(272, "Test Camera")];
    if warp {
        s.extra.push(opcode_list(
            3,
            vec![op_warp(
                &[
                    [0.97, 0.03, -0.01, 0.0, 0.0002, 0.0],
                    [0.975, 0.028, -0.01, 0.0, 0.0002, 0.0],
                    [0.98, 0.026, -0.01, 0.0, 0.0002, 0.0],
                ],
                [0.5, 0.5],
            )],
        ));
    }
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
            // xorshift of the position: uniform in -0.5..0.5.
            let mut k = u64::from(y) << 32 | u64::from(x) | 1;
            k ^= k >> 12;
            k ^= k << 25;
            k ^= k >> 27;
            let u = (k.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32;
            let v = (v + noise * (u - 0.5)).clamp(0.0, 1.0);
            data.push((v * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// The default raw edit with the file's as-shot white.
fn edit_for(input: &Input) -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    if let Input::Raw(s) = input
        && let Some(xy) = preview::as_shot_white(s)
    {
        batch::set_as_shot(&mut e, xy);
    }
    e
}

/// Linear sRGB → CIELAB (D65).
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

/// The published CIEDE2000 test pairs (Sharma et al., table 1, a sample).
#[test]
fn ciede2000_matches_the_published_pairs() {
    for (a, b, want) in [
        ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
        ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
        ([50.0, -1.3802, -84.2814], [50.0, 0.0, -82.7485], 1.0),
        ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
        (
            [60.2574, -34.0099, 36.2677],
            [60.4626, -34.1751, 39.4387],
            1.2644,
        ),
        (
            [22.7233, 20.0904, -46.694],
            [23.0331, 14.973, -42.5619],
            2.0373,
        ),
        (
            [90.9257, -0.5406, -0.9208],
            [88.6381, -0.8985, -0.7239],
            1.5381,
        ),
        ([2.0776, 0.0795, -1.135], [0.9033, -0.0636, -0.5514], 0.9082),
    ] {
        let d = de2000(a, b);
        assert!((d - want).abs() < 1e-4, "{a:?} {b:?}: {d} vs {want}");
    }
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// The acceptance check: a full-resolution export downscaled to the fit view's size matches
/// the fit view within CIEDE2000 p99 < 0.5.
/// The default edit, and one with the DNG's warp, a straighten, a crop and manual lens
/// corrections: export uses the same warp as the view.
#[test]
fn export_matches_the_fit_view() {
    for geometry in [false, true] {
        export_matches_the_fit_view_with(geometry, false);
    }
}

/// [`export_matches_the_fit_view`] on a noisy raw with denoise and the geometry: both
/// denoise before the warp (`specular_develop::prewarp`).
#[test]
fn export_with_denoise_matches_the_fit_view() {
    export_matches_the_fit_view_with(false, true);
    export_matches_the_fit_view_with(true, true);
}

/// `edit_for` with a straighten, a crop and manual lens corrections (`geometry`) and
/// luminance and colour denoise at 0.6 (`denoise`).
fn edit_with(input: &Input, geometry: bool, denoise: bool) -> Edit {
    let mut edit = edit_for(input);
    if geometry {
        let m = edit
            .module_mut(ModuleKind::CROP_ORIENTATION, 0)
            .expect("crop module");
        m.params = Params::CropOrientation(CropOrientation {
            angle_deg: 2.5,
            crop: [0.1, 0.12, 0.85, 0.9],
            ..Default::default()
        });
        let mut l = ModuleInstance::new(Registry::builtin(), ModuleKind::LENS, 0);
        let lp = LensParams {
            distortion: 15.0,
            tca_red: 0.05,
            ..LensParams::default()
        };
        l.params = Params::Other(ModuleKind::LENS, lp.to_fields());
        edit.insert(Registry::builtin(), l);
    }
    if denoise {
        let r = Registry::builtin();
        let mut m = ModuleInstance::new(r, ModuleKind::DENOISE, 0);
        let f = m
            .params
            .to_fields()
            .with("luminance", Value::Float(0.6))
            .with("colour", Value::Float(0.6));
        m.params = Params::from_fields(ModuleKind::DENOISE, f);
        edit.insert(r, m);
    }
    edit
}

fn export_matches_the_fit_view_with(geometry: bool, denoise: bool) {
    let Some(d) = dev() else { return };
    let _g = lock();
    let (w, h) = (1600, 1068);
    let file = colour_dng_with(w, h, 1, geometry, if denoise { 0.06 } else { 0.0 });
    // Denoise at full size (at a reduced view it works on the reduced noise: a different
    // picture by design).
    let long = if denoise { w } else { 800 };

    // The fit view, as Develop shows it (linear BT.709 read back).
    let input = preview::decode_raw(&file).unwrap();
    let edit = edit_with(&input, geometry, denoise);
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.set_input(input).unwrap();
    dv.prepare(&edit, long).unwrap();
    let r = dv.render(&edit, None, &OutputSpace::rec709()).unwrap();
    let (vw, vh) = (r.roi.w, r.roi.h);
    let view = dv.read(&r).unwrap();
    dv.recycle(r);
    let long = vw.max(vh);

    // The export at the same size, 16-bit sRGB.
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        size: Resize::LongEdge(long),
        ..Preset::default()
    };
    let input = preview::decode_raw(&file).unwrap();
    let img = ex
        .render(input, &edit, Orient::default(), &preset, &mut |_| true)
        .unwrap();
    assert_eq!((img.width, img.height), (vw, vh));
    assert!(img.stats.source_f32);
    let Samples::Sixteen(px) = &img.samples else {
        panic!("16-bit")
    };
    let mut de = Vec::with_capacity((vw * vh) as usize);
    for i in 0..(vw * vh) as usize {
        let v = [view[i * 4], view[i * 4 + 1], view[i * 4 + 2]].map(f64::from);
        let e = [px[i * 3], px[i * 3 + 1], px[i * 3 + 2]]
            .map(|s| Transfer::Srgb.decode(f64::from(s) / 65535.0));
        de.push(de2000(lab(v), lab(e)));
    }
    let mean = de.iter().sum::<f64>() / de.len() as f64;
    let p50 = percentile(&mut de, 0.5);
    let p99 = percentile(&mut de, 0.99);
    let max = de[de.len() - 1];
    println!(
        "export vs fit view ({vw}×{vh}, {}{}{}): ΔE2000 mean {mean:.3}, p50 {p50:.3}, p99 {p99:.3}, max {max:.3}",
        d.name,
        if geometry {
            ", warp, straighten, crop, lens"
        } else {
            ""
        },
        if denoise { ", denoise" } else { "" }
    );
    assert!(p99 < 0.5, "p99 {p99}");
}

/// A decoded output: size, straight RGBA8 and its profile.
struct Back {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
    icc: Option<Vec<u8>>,
    orientation: u8,
}

fn read_back(p: &Path) -> Back {
    let reg = specular_decode::Registry::new();
    let s = reg
        .open_path(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let info = s.info();
    let icc = info
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| s.embedded_icc().map(<[u8]>::to_vec));
    let orientation = info.orientation.0;
    let r = s
        .decode(
            &specular_decode::DecodeRequest::full(1),
            &specular_core::CancelToken::new(),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let target = specular_convert::pixels::Target {
        width: r.width,
        height: r.height,
        gray: false,
        alpha: true,
        sixteen: false,
        transfer: specular_convert::pixels::OutTransfer::Srgb,
        tone_map: false,
    };
    let px = specular_convert::pixels::render(
        &r,
        &specular_convert::pixels::ToLinear::Srgb,
        &target,
        specular_core::Orientation::NORMAL,
        &CancelToken::new(),
    )
    .ok()
    .unwrap();
    let specular_convert::pixels::Samples::Eight(v) = px.samples else {
        panic!("8-bit")
    };
    let rgba = match px.channels {
        3 => v.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        _ => v,
    };
    Back {
        w: r.width,
        h: r.height,
        rgba,
        icc,
        orientation,
    }
}

/// Every format, written by the queue (EXIF orientation 6 turned upright, Exif kept with
/// orientation 1), reads back with our own decoders at the turned size, with the space's
/// profile, and with the pixels of the export.
#[test]
fn every_format_reads_back_with_our_decoders() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let dir = tmpdir("formats");
    let src = dir.join("IMG_0001.dng");
    std::fs::write(&src, colour_dng(320, 214, 6)).unwrap();
    let input = preview::decode_raw(&std::fs::read(&src).unwrap()).unwrap();
    let edit = edit_for(&input);
    let reg = specular_decode::Registry::new();
    let cases: Vec<(FileFormat, Depth, Space, bool, u8)> = vec![
        // format, depth, space, lossless, max 8-bit difference from the export
        (FileFormat::Jpeg, Depth::Eight, Space::Srgb, false, 24),
        (FileFormat::Png, Depth::Eight, Space::DisplayP3, true, 0),
        (FileFormat::Png, Depth::Sixteen, Space::Rec2020, true, 1),
        (FileFormat::Avif, Depth::Eight, Space::Srgb, false, 40),
        (FileFormat::Jxl, Depth::Eight, Space::AdobeRgb, false, 24),
        (FileFormat::Jxl, Depth::Sixteen, Space::Srgb, true, 1),
        (FileFormat::Tiff, Depth::Eight, Space::Srgb, true, 0),
        (FileFormat::Tiff, Depth::Sixteen, Space::AdobeRgb, true, 1),
        (FileFormat::Tiff, Depth::Float, Space::Rec2020, true, 2),
        (FileFormat::Exr, Depth::Half, Space::Srgb, true, 2),
        (FileFormat::Exr, Depth::Float, Space::Srgb, true, 2),
    ];
    for (k, &(format, depth, space, lossless, tol)) in cases.iter().enumerate() {
        let preset = Preset {
            format,
            depth,
            space,
            lossless,
            quality: 92,
            pattern: format!("{{name}}-{k}"),
            folder: OutputDir::Folder(dir.join("out")),
            ..Preset::default()
        };
        let jobs = vec![Job {
            source: src.clone(),
            edit: edit.clone(),
            orient: None,
        }];
        let rep = batch::run(
            &d.device,
            &d.queue,
            Budget::fixed(1 << 30),
            jobs,
            &preset,
            &reg,
            &CancelToken::new(),
            &|_| {},
        );
        let out = match &rep.results[0].1 {
            Outcome::Written { out, .. } => out.clone(),
            o => panic!("{format:?} {depth:?}: {o:?}"),
        };
        assert_eq!(
            out.extension().unwrap().to_string_lossy(),
            format.ext(),
            "{format:?}"
        );
        let b = read_back(&out);
        assert_eq!(
            (b.w, b.h),
            (214, 320),
            "{format:?} {depth:?}: turned upright"
        );
        assert!(
            b.orientation <= 1,
            "{format:?}: orientation {}",
            b.orientation
        );
        if format == FileFormat::Jxl {
            // libjxl keeps a known colour encoding, not the bytes: the decoder's profile
            // has the same colorants.
            let got = specular_color::Profile::parse(b.icc.as_deref().expect("profile")).unwrap();
            let want = specular_color::Profile::parse(&space.icc(false)).unwrap();
            let (g, w) = (got.matrix.unwrap(), want.matrix.unwrap());
            assert!(g.max_diff(&w) < 2e-3, "{format:?}: colorants");
        } else if format != FileFormat::Exr {
            assert_eq!(
                b.icc.as_deref(),
                Some(&space.icc(depth.float())[..]),
                "{format:?} {depth:?}: the space's profile"
            );
        }
        // Exif carried where the format holds it, the camera named.
        if matches!(
            format,
            FileFormat::Jpeg | FileFormat::Png | FileFormat::Tiff
        ) {
            let bytes = std::fs::read(&out).unwrap();
            let t = specular_decode::exif_tiff(&bytes).expect("Exif");
            let (facts, _) = specular_decode::exif_facts(&t).expect("facts");
            assert!(facts.camera.contains("Test Camera"), "{}", facts.camera);
        }
        // Pixels: the export's own samples (for sRGB-coded 8-bit files) through a fresh
        // render, compared as the registry shows them (float and wide files are only
        // checked for size and profile above).
        if space == Space::Srgb && !depth.float() {
            let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
            let input = preview::decode_raw(&std::fs::read(&src).unwrap()).unwrap();
            let img = ex
                .render(input, &edit, Orient::from_exif(6), &preset, &mut |_| true)
                .unwrap();
            let want: Vec<u8> = match &img.samples {
                Samples::Eight(v) => v.clone(),
                Samples::Sixteen(v) => v
                    .iter()
                    .map(|s| ((u32::from(*s) * 255 + 32767) / 65535) as u8)
                    .collect(),
                Samples::Float(_) => unreachable!(),
            };
            let mut worst = 0u8;
            for (p, q) in want.chunks(3).zip(b.rgba.chunks(4)) {
                for c in 0..3 {
                    worst = worst.max(p[c].abs_diff(q[c]));
                }
            }
            assert!(worst <= tol, "{format:?} {depth:?}: worst {worst} > {tol}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Names: a taken name gets a suffix or is skipped as the preset says; a name that would
/// be the source is refused; Stop before the start does nothing.
#[test]
fn names_never_replace_files() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let dir = tmpdir("names");
    let src = dir.join("frame.dng");
    std::fs::write(&src, colour_dng(64, 48, 1)).unwrap();
    let input = preview::decode_raw(&std::fs::read(&src).unwrap()).unwrap();
    let edit = edit_for(&input);
    let reg = specular_decode::Registry::new();
    let go = |preset: &Preset, cancel: &CancelToken| {
        batch::run(
            &d.device,
            &d.queue,
            Budget::fixed(1 << 28),
            vec![Job {
                source: src.clone(),
                edit: edit.clone(),
                orient: None,
            }],
            preset,
            &reg,
            cancel,
            &|_| {},
        )
    };
    let mut p = Preset {
        format: FileFormat::Png,
        folder: OutputDir::Subfolder(String::new()),
        metadata: MetaMode::Strip,
        ..Preset::default()
    };
    let a = go(&p, &CancelToken::new());
    assert_eq!(
        a.results[0].1,
        Outcome::Written {
            out: dir.join("frame.png"),
            bytes: std::fs::metadata(dir.join("frame.png")).unwrap().len()
        }
    );
    let b = go(&p, &CancelToken::new());
    assert!(
        matches!(&b.results[0].1, Outcome::Written { out, .. } if out == &dir.join("frame-2.png"))
    );
    p.conflict = Conflict::Skip;
    let c = go(&p, &CancelToken::new());
    assert!(
        matches!(&c.results[0].1, Outcome::Skipped { .. }),
        "{:?}",
        c.results
    );
    // The source's own name: frame.png (an image, not a raw) exported as PNG beside itself.
    let png = dir.join("frame.png");
    let before = std::fs::read(&png).unwrap();
    let e = batch::run(
        &d.device,
        &d.queue,
        Budget::fixed(1 << 28),
        vec![Job {
            source: png.clone(),
            edit: Edit::default_for(Registry::builtin(), SourceKey::default(), false),
            orient: None,
        }],
        &Preset {
            conflict: Conflict::Suffix,
            ..p.clone()
        },
        &reg,
        &CancelToken::new(),
        &|_| {},
    );
    assert!(
        matches!(&e.results[0].1, Outcome::Failed(specular_convert::Failure::SameAsSource(p)) if p == &png),
        "{:?}",
        e.results
    );
    assert_eq!(std::fs::read(&png).unwrap(), before);
    let stop = CancelToken::new();
    stop.cancel();
    let f = go(&p, &stop);
    assert!(f.cancelled && f.results[0].1 == Outcome::Cancelled);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Export budgets: a 24, 45 and 100 MP raw (lossless-compressed DNG) to
/// JPEG q90 at full size, decode included, and the batch throughput at 24 MP.
#[test]
#[ignore = "timing report; run by hand"]
fn export_budgets() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let dir = tmpdir("budgets");
    let reg = specular_decode::Registry::new();
    for (mp, w, h) in [(24, 6000u32, 4000u32), (45, 8256, 5504), (100, 11648, 8736)] {
        let mut s = DngSpec::new(w, h, DngCodec::Lj92Tiles(256));
        s.extra = vec![tag_ascii(271, "Specular"), tag_ascii(272, "Bench")];
        let mut data = Vec::with_capacity((w * h) as usize);
        let mut seed = 0x2545F491u32;
        for y in 0..h {
            for x in 0..w {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let base = 0.1 + 0.4 * ((x as f32 / 300.0).sin() * (y as f32 / 200.0).cos()).abs();
                let n = (seed % 512) as f32 / 65535.0;
                data.push(((base + n) * 60000.0) as u16);
            }
        }
        let src = dir.join(format!("bench{mp}.dng"));
        std::fs::write(&src, write_dng(&s, &data)).unwrap();
        drop(data);
        let input = preview::decode_raw(&std::fs::read(&src).unwrap()).unwrap();
        let edit = edit_for(&input);
        drop(input);
        let preset = Preset {
            folder: OutputDir::Folder(dir.join("out")),
            ..Preset::default()
        };
        let n = if mp == 24 { 4 } else { 2 };
        let jobs: Vec<Job> = (0..n)
            .map(|_| Job {
                source: src.clone(),
                edit: edit.clone(),
                orient: None,
            })
            .collect();
        let rep = batch::run(
            &d.device,
            &d.queue,
            Budget::for_device(true, None, Some(32 << 30)),
            jobs,
            &preset,
            &reg,
            &CancelToken::new(),
            &|_| {},
        );
        assert_eq!(rep.written(), n, "{:?}", rep.results);
        for (i, t) in rep.timings.iter().enumerate() {
            println!(
                "{mp} MP #{i}: read {:.0} ms, source {:.0} ms, render {:.0} ms, encode {:.0} ms, write {:.0} ms; one image {:.2} s",
                t.read_ms,
                t.source_ms,
                t.render_ms,
                t.encode_ms,
                t.write_ms,
                (t.read_ms + t.source_ms + t.render_ms + t.encode_ms + t.write_ms) / 1e3
            );
        }
        println!(
            "{mp} MP: {n} images in {:.2} s, {:.2} images/s",
            rep.elapsed.as_secs_f64(),
            n as f64 / rep.elapsed.as_secs_f64()
        );
        let _ = std::fs::remove_dir_all(dir.join("out"));
        let _ = std::fs::remove_file(&src);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Oracle: LittleCMS 2 (`tificc`, the system's) converts our 16-bit and float TIFFs in every
/// export space, through their embedded profiles, to sRGB; the result must match our own
/// colour maths for a 17³ grid of colours inside sRGB (no gamut clipping anywhere). A
/// mismatch is a sample more than half an 8-bit code away.
#[test]
#[ignore = "oracle; needs LittleCMS's tificc"]
fn profiles_match_littlecms() {
    use specular_develop::colour;
    use specular_develop::export::render::{ExportStats, Exported};
    let dir = tmpdir("lcms");
    let n = 17usize;
    let (w, h) = ((n * n) as u32, n as u32);
    let mut lin = Vec::with_capacity(n * n * n * 3);
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                lin.extend([r, g, b].map(|v| v as f64 / (n - 1) as f64));
            }
        }
    }
    // The pixels are laid out r fastest, rows by b: 289 × 17.
    let to2020 = colour::rec709_to_rec2020();
    let want: Vec<u16> = lin
        .iter()
        .map(|&l| (Transfer::Srgb.encode(l) * 65535.0).round() as u16)
        .collect();
    let mut total = 0usize;
    for space in Space::ALL {
        let m = colour::mul(&space.from_rec2020(), &to2020);
        let mut sixteen = Vec::with_capacity(lin.len());
        let mut float = Vec::with_capacity(lin.len());
        for px in lin.chunks(3) {
            let v = colour::mulv(&m, [px[0], px[1], px[2]]);
            for c in v {
                let c = c.clamp(0.0, 1.0);
                sixteen.push((space.transfer().encode(c) * 65535.0).round() as u16);
                float.push(c as f32);
            }
        }
        for (depth, samples) in [
            (Depth::Sixteen, Samples::Sixteen(sixteen)),
            (Depth::Float, Samples::Float(float)),
        ] {
            let img = Exported {
                width: w,
                height: h,
                samples,
                stats: ExportStats::default(),
                gain_map: None,
                alpha: None,
            };
            let preset = Preset {
                format: FileFormat::Tiff,
                depth,
                space,
                ..Preset::default()
            };
            let bytes = match &img.samples {
                // tificc keeps the input's predictor 3 on its 16-bit output, which TIFF does
                // not define: the float file goes in uncompressed (same pixels and profile).
                Samples::Float(v) => {
                    let icc = space.icc(true);
                    let meta = specular_convert::encode::Meta {
                        icc: Some(&icc),
                        ..Default::default()
                    };
                    specular_convert::encode::tiff_float(w, h, 3, v, &meta, false)
                }
                _ => batch::encode(&img, &preset, None, None).unwrap(),
            };
            let src = dir.join(format!("{}-{}.tif", space.id(), depth.id()));
            let out = dir.join(format!("{}-{}-srgb.tif", space.id(), depth.id()));
            std::fs::write(&src, bytes).unwrap();
            let st = std::process::Command::new("tificc")
                .args(["-t1", "-c0", "-w16"])
                .arg(&src)
                .arg(&out)
                .status()
                .expect("tificc");
            assert!(st.success(), "tificc failed on {}", src.display());
            let reg = specular_decode::Registry::new();
            let s = reg.open_path(&out).unwrap();
            let r = s
                .decode(
                    &specular_decode::DecodeRequest::full(1),
                    &CancelToken::new(),
                )
                .unwrap();
            assert!(
                matches!(r.layout, specular_core::PixelLayout::Rgba16(_)),
                "{:?}",
                r.layout
            );
            let pl = &r.planes[0];
            let mut worst = 0u32;
            let mut bad = 0usize;
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let o = y * pl.stride + x * 8;
                    for c in 0..3 {
                        let got = u16::from_ne_bytes([pl.data[o + 2 * c], pl.data[o + 2 * c + 1]]);
                        let e = want[(y * w as usize + x) * 3 + c];
                        let d = u32::from(got.abs_diff(e));
                        worst = worst.max(d);
                        if d > 128 {
                            bad += 1;
                        }
                    }
                }
            }
            println!(
                "{} {}: worst {worst} of 65535, {bad} of {} samples off by more than half an 8-bit code",
                space.id(),
                depth.id(),
                want.len()
            );
            total += bad;
        }
    }
    assert_eq!(total, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A source with transparency keeps it through the edit's geometry into formats that hold
/// it (PNG 8 and 16-bit, float TIFF), read back with our own decoders; JPEG has none.
#[test]
fn transparency_survives_export() {
    use specular_develop::export::hdr;
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let (w, h) = (96u32, 64u32);
    // Opaque left half, transparent right half, half-covered band in the middle rows.
    let alpha = |x: u32, y: u32| {
        if (28..36).contains(&y) {
            0.5
        } else if x < w / 2 {
            1.0
        } else {
            0.0
        }
    };
    let input = || Input::Rgb {
        width: w,
        height: h,
        rgba: (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| [0.2, 0.1, 0.05, alpha(x, y)]))
            .collect(),
    };
    // A quarter turn: the coverage turns with the photo.
    let reg = Registry::builtin();
    let mut e = Edit::default_for(reg, SourceKey::default(), false);
    let m = e.module_mut(ModuleKind::CROP_ORIENTATION, 0).unwrap();
    let mut c = CropOrientation::from_fields(&m.params.to_fields());
    c.quarter_turns = 1;
    m.params.overlay(&c.to_fields());
    let mut bad = 0;
    for (format, depth) in [
        (FileFormat::Png, Depth::Eight),
        (FileFormat::Png, Depth::Sixteen),
        (FileFormat::Tiff, Depth::Float),
        (FileFormat::Jpeg, Depth::Eight),
    ] {
        let p = Preset {
            format,
            depth,
            ..Preset::default()
        }
        .normalised();
        let img = hdr::render(&mut ex, input(), &e, Orient::default(), &p, &mut |_| true).unwrap();
        assert_eq!((img.width, img.height), (h, w));
        let a = img.alpha.as_ref().expect("coverage");
        let bytes = batch::encode(&img, &p, None, None).unwrap();
        let reg = specular_decode::Registry::new();
        let dir =
            std::env::temp_dir().join(format!("specular-develop-alpha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join(format!("a{depth:?}.{}", format.ext()));
        std::fs::write(&f, &bytes).unwrap();
        let s = reg.open_path(&f).unwrap();
        let has = s.info().has_alpha;
        std::fs::remove_dir_all(&dir).unwrap();
        if format == FileFormat::Jpeg {
            assert!(!has);
            continue;
        }
        assert!(has, "{format:?} {depth:?}: no alpha read back");
        // Turned a quarter clockwise: output (x, y) is source (y, h - 1 - x).
        for oy in 0..w {
            for ox in 0..h {
                let want = alpha(oy, h - 1 - ox);
                let got = a[(oy * h + ox) as usize];
                bad += usize::from((got - want).abs() > 0.02);
            }
        }
    }
    eprintln!("transparency: {bad} coverage values off by more than 0.02");
    assert_eq!(bad, 0);
}

/// A 24 MP export with denoise before the warp (straighten, crop, lens): its
/// time (debug build; with the load average) and the most memory it holds meanwhile, in
/// full-size source buffers. `EXPORT24_SIZE` like `6000x4000`.
#[test]
#[ignore = "timing report; run by hand"]
fn export_24mp_with_denoise_before_the_warp() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let (w, h) = std::env::var("EXPORT24_SIZE")
        .ok()
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((6000u32, 4000u32));
    prewarp_export(d, w, h);
}

/// 8-bit samples coded on the GPU are the CPU's bytes: every integer file's space,
/// upright and turned, one band and two (6000×4000), with denoise before the warp.
#[test]
fn eight_bit_codes_on_the_gpu_equal_the_cpus() {
    let Some(d) = dev() else { return };
    let _g = lock();
    codes_on_the_gpu_equal_the_cpus(d, Depth::Eight, FileFormat::Jpeg);
}

/// 16-bit samples coded on the GPU are the CPU's, as the 8-bit ones.
#[test]
fn sixteen_bit_codes_on_the_gpu_equal_the_cpus() {
    let Some(d) = dev() else { return };
    let _g = lock();
    codes_on_the_gpu_equal_the_cpus(d, Depth::Sixteen, FileFormat::Png);
}

fn codes_on_the_gpu_equal_the_cpus(d: &Dev, depth: Depth, format: FileFormat) {
    let mut checked = 0usize;
    for (w, h, spaces, orientation) in [
        (
            1200u32,
            800u32,
            &[Space::Srgb, Space::Rec2020, Space::AdobeRgb][..],
            1u16,
        ),
        (1200, 800, &[Space::Srgb][..], 6),
        (6000, 4000, &[Space::Srgb][..], 1),
    ] {
        let file = colour_dng_with(w, h, orientation, true, 0.06);
        let input = preview::decode_raw(&file).unwrap();
        let mut edit = edit_with(&input, true, true);
        // Highlights pushed past white, so values above 1 are coded too.
        let mut m = ModuleInstance::new(Registry::builtin(), ModuleKind::EXPOSURE, 1);
        m.params = Params::from_fields(
            ModuleKind::EXPOSURE,
            m.params.to_fields().with("exposure_ev", Value::Float(1.5)),
        );
        edit.insert(Registry::builtin(), m);
        let orient = Orient::from_exif(orientation);
        for &space in spaces {
            let preset = Preset {
                format,
                depth,
                space,
                ..Preset::default()
            };
            let mut out = Vec::new();
            for gpu in [false, true] {
                let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
                ex.gpu_codes = gpu;
                let img = ex
                    .render(input.clone(), &edit, orient, &preset, &mut |_| true)
                    .unwrap();
                assert_eq!(img.stats.gpu_codes, gpu);
                let px: Vec<u16> = match img.samples {
                    Samples::Eight(px) => px.into_iter().map(u16::from).collect(),
                    Samples::Sixteen(px) => px,
                    Samples::Float(_) => panic!("integer samples"),
                };
                out.push((img.width, img.height, img.stats.bands, px));
            }
            let (a, b) = (&out[0], &out[1]);
            assert_eq!((a.0, a.1), (b.0, b.1));
            let differ = a.3.iter().zip(&b.3).filter(|(x, y)| x != y).count();
            println!(
                "{w}×{h} {space:?} orientation {orientation}, {} band(s): {} samples, {differ} differ",
                b.2,
                b.3.len()
            );
            assert_eq!(differ, 0, "{space:?} {w}×{h}");
            assert_eq!(a.3.len(), b.3.len());
            if w == 6000 {
                assert!(b.2 >= 2, "two bands at least");
            }
            checked += b.3.len();
        }
    }
    println!("{checked} samples, 0 differ");
}

/// A resized export's horizontal half on the GPU gives the CPU's values (to float rounding:
/// a fused multiply-add may round differently), at shrinks across one band and several, in
/// both orientations.
#[test]
fn resizing_across_on_the_gpu_matches_the_cpu() {
    let Some(d) = dev() else { return };
    let _g = lock();
    for (w, h, orientation, long) in [(1200u32, 800u32, 1u16, 517u32), (6000, 4000, 6, 2048)] {
        let file = colour_dng_with(w, h, orientation, true, 0.06);
        let input = preview::decode_raw(&file).unwrap();
        let edit = edit_with(&input, true, true);
        let orient = Orient::from_exif(orientation);
        let preset = Preset {
            format: FileFormat::Tiff,
            depth: Depth::Float,
            space: Space::Srgb,
            size: Resize::LongEdge(long),
            ..Preset::default()
        };
        let mut out = Vec::new();
        for gpu in [false, true] {
            let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
            ex.gpu_resize = gpu;
            let img = ex
                .render(input.clone(), &edit, orient, &preset, &mut |_| true)
                .unwrap();
            assert_eq!(img.stats.gpu_resize, gpu);
            assert_eq!(img.width.max(img.height), long);
            let Samples::Float(px) = img.samples else {
                panic!("float samples")
            };
            out.push(px);
        }
        let worst = out[0]
            .iter()
            .zip(&out[1])
            .map(|(a, b)| (a - b).abs() / a.abs().max(1.0))
            .fold(0f32, f32::max);
        println!("{w}×{h} → {long}: worst relative difference {worst:e}");
        assert_eq!(out[0].len(), out[1].len());
        assert!(worst < 1e-5, "{worst}");
    }
}

/// An export with denoise before the warp holds the source, its denoised copy and
/// one band's render at most (2400×1600).
#[test]
fn denoise_before_the_warp_holds_two_sources() {
    let Some(d) = dev() else { return };
    let _g = lock();
    prewarp_export(d, 2400, 1600);
}

fn prewarp_export(d: &Dev, w: u32, h: u32) {
    let file = colour_dng_with(w, h, 1, true, 0.06);
    let input = preview::decode_raw(&file).unwrap();
    let edit = edit_with(&input, true, true);
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        ..Preset::default()
    };
    let load = || {
        std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|s| s.split_whitespace().next().map(str::to_owned))
            .unwrap_or_default()
    };
    let l0 = load();
    let t = std::time::Instant::now();
    let img = ex
        .render(input, &edit, Orient::default(), &preset, &mut |_| true)
        .unwrap();
    let total = t.elapsed().as_secs_f64() * 1e3;
    let st = img.stats;
    assert!(st.prewarp_peak > 0, "denoise ran before the warp");
    // A full-size source at the export's storage (f32 or f16 RGBA).
    let full = u64::from(w) * u64::from(h) * if st.source_f32 { 16 } else { 8 };
    let band = 192u64 << 20;
    println!(
        "{w}×{h} export, denoise before the warp: {:.0} ms of {:.0} ms (source {:.0}, render {:.0}); peak {} MiB = {:.2} full-size sources of {} MiB; load {l0} → {}",
        st.prewarp_ms,
        total,
        st.source_ms,
        st.render_ms,
        st.prewarp_peak >> 20,
        st.prewarp_peak as f64 / full as f64,
        full >> 20,
        load()
    );
    // Two full-size buffers and one band's render (its output, read-back and scratch).
    assert!(
        st.prewarp_peak <= 2 * full + 4 * band,
        "{} MiB",
        st.prewarp_peak >> 20
    );
}

/// Export recipes on a developed raw and a plain JPEG: the raw is rendered through its edit
/// (as Develop's export would), the JPEG decoded as it is; both recipes write both images
/// at their sizes, and the 16-bit ProPhoto TIFF carries its profile.
#[test]
fn recipes_run_on_developed_and_plain_images() {
    use specular_convert::recipe::{self, Bits, Destination, Item, Recipe, Size, Space as RSpace};
    let Some(d) = dev() else { return };
    let _g = lock();
    let dir = tmpdir("recipes");
    let raw = dir.join("raw.dng");
    std::fs::write(&raw, colour_dng(240, 160, 1)).unwrap();
    let jpeg = dir.join("plain.jpg");
    let px = specular_convert::pixels::Pixels {
        width: 120,
        height: 90,
        channels: 3,
        samples: specular_convert::pixels::Samples::Eight([90u8, 140, 200].repeat(120 * 90)),
    };
    std::fs::write(
        &jpeg,
        specular_convert::encode::encode(&Default::default(), &px, &Default::default()).unwrap(),
    )
    .unwrap();
    let registry = std::sync::Arc::new(specular_decode::Registry::new());
    let read = batch::read_input(&raw, &registry).unwrap();
    let mut edits = std::collections::HashMap::new();
    edits.insert(raw.clone(), (edit_for(&read.input), None));
    let r = specular_develop::export::recipes::Developed::new(
        &d.device,
        &d.queue,
        Budget::for_device(true, None, None),
        registry.clone(),
        edits,
    );
    let web = Recipe {
        size: Size::LongEdge(100),
        destination: Destination::Subfolder("web".into()),
        ..Recipe::default()
    };
    let tiff = Recipe {
        format: specular_convert::Format::Tiff,
        bits: Bits::Sixteen,
        space: RSpace::ProPhoto,
        destination: Destination::Subfolder("tiff".into()),
        ..Recipe::default()
    };
    let c = CancelToken::new();
    let p = recipe::plan(
        vec![Item::new(&raw), Item::new(&jpeg)],
        vec![web, tiff],
        &r,
        &c,
    )
    .unwrap();
    let rep = recipe::execute(&p, &r, Some(&registry), 2, &c, &|_| {});
    assert_eq!(rep.written(), 4, "{:?}", rep.results);
    let sizes: Vec<(u32, u32)> = rep
        .results
        .iter()
        .map(|(_, _, o)| {
            let specular_convert::Outcome::Converted { out, .. } = o else {
                unreachable!()
            };
            let s = specular_decode::Registry::new().open_path(out).unwrap();
            s.info().display_size()
        })
        .collect();
    assert_eq!(sizes[0].0.max(sizes[0].1), 100);
    assert!(sizes[1].0 >= 200, "{sizes:?}");
    assert_eq!(sizes[2], (100, 75));
    assert_eq!(sizes[3], (120, 90));
    let _ = std::fs::remove_dir_all(&dir);
}
