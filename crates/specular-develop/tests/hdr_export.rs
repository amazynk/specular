//! HDR export, read back with our own decoders: PQ and HLG in PNG, AVIF and JPEG XL
//! decode to the HDR rendition; gain-map JPEG, AVIF and JPEG XL rebuild it from the SDR base
//! with the viewer's gain-map maths; the SDR preview equals the SDR export; the HDR
//! rendition reaches the headroom and keeps mid grey.

use std::sync::OnceLock;

use specular_core::CancelToken;
use specular_core::hdr::{GainMapParams, HdrEncoding, Primaries, decode16, mat_mul, to_bt709};
use specular_develop::export::batch;
use specular_develop::export::hdr::{self, HdrOutput};
use specular_develop::export::{Depth, Exporter, FileFormat, Preset, Samples, Space, Transfer};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::params::{ModuleParams, ViewTransform};
use specular_develop::model::{Edit, ModuleKind, Registry, SourceKey};
use specular_develop::ops::output::OutputSpace;
use specular_develop::ops::view;
use specular_develop::preview::{Developer, Input, Orient};

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

const W: u32 = 192;
const H: u32 = 128;

/// A scene with HDR highlights: an exposure ramp across (2^-6 … 2^5 × grey), colour down,
/// smooth enough for lossy codecs.
fn scene() -> Input {
    let mut rgba = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let ev = x as f32 / (W - 1) as f32 * 11.0 - 6.0;
            let l = 0.1845 * ev.exp2();
            let t = y as f32 / (H - 1) as f32;
            let c = [1.0 - 0.5 * t, 0.7 + 0.3 * t, 0.6 + 0.4 * (1.0 - t)];
            rgba.extend_from_slice(&[l * c[0], l * c[1], l * c[2], 1.0]);
        }
    }
    Input::Rgb {
        width: W,
        height: H,
        rgba,
    }
}

/// The default non-raw edit with an HDR limit of `ev`.
fn edit(ev: f32) -> Edit {
    let reg = Registry::builtin();
    let mut e = Edit::default_for(reg, SourceKey::default(), false);
    let m = e.module_mut(ModuleKind::VIEW_TRANSFORM, 0).unwrap();
    let mut v = ViewTransform::from_fields(&m.params.to_fields());
    v.hdr_limit_ev = ev;
    m.params.overlay(&v.to_fields());
    e
}

/// The HDR rendition as linear BT.709 (what the decoders give back).
fn hdr_rendition(ex: &mut Exporter, e: &Edit, space: Space, peak: f32) -> Vec<[f32; 3]> {
    let p = Preset {
        format: FileFormat::Tiff,
        depth: Depth::Float,
        space,
        ..Preset::default()
    };
    let shown = view::for_headroom(e, peak);
    let img = ex
        .render_at(scene(), &shown, Orient::default(), &p, peak, &mut |_| true)
        .unwrap();
    let Samples::Float(v) = img.samples else {
        panic!("float")
    };
    let prim = match space {
        Space::Rec2020 => Primaries::Bt2020,
        Space::DisplayP3 => Primaries::DisplayP3,
        _ => Primaries::Bt709,
    };
    v.chunks(3)
        .map(|c| mat_mul(to_bt709(prim), [c[0], c[1], c[2]]))
        .collect()
}

fn decode(bytes: &[u8], ext: &str) -> (specular_core::Raster, Option<specular_core::GainMap>) {
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, Into::into);
    let p = base.join(format!(
        "specular-hdr-{}-{}.{ext}",
        std::process::id(),
        bytes.len()
    ));
    std::fs::write(&p, bytes).unwrap();
    let reg = specular_decode::Registry::new();
    let s = reg.open_path(&p).unwrap_or_else(|e| panic!("{ext}: {e}"));
    let r = s
        .decode(
            &specular_decode::DecodeRequest::full(1),
            &CancelToken::new(),
        )
        .unwrap();
    // HEIC and AVIF decode to YCbCr planes for the GPU; convert them like export does.
    let r = specular_core::yuv::to_rgb(&r).unwrap_or(r);
    let g = s.gain_map(&CancelToken::new());
    let _ = std::fs::remove_file(&p);
    (r, g)
}

/// The decoded 8-bit base as sRGB-coded RGB.
fn base_rgb(r: &specular_core::Raster) -> Vec<[u8; 3]> {
    let target = specular_convert::pixels::Target {
        width: r.width,
        height: r.height,
        gray: false,
        alpha: false,
        sixteen: false,
        transfer: specular_convert::pixels::OutTransfer::Srgb,
        tone_map: false,
    };
    let px = specular_convert::pixels::render(
        r,
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
    v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect()
}

/// Mean and 99th percentile of the relative error (against max(want, 0.05)), and how many
/// values are off by more than `tol`.
fn compare(got: &[[f32; 3]], want: &[[f32; 3]], tol: f32) -> (f32, f32, usize) {
    assert_eq!(got.len(), want.len());
    let mut errs: Vec<f32> = got
        .iter()
        .zip(want)
        .flat_map(|(g, w)| (0..3).map(move |c| (g[c] - w[c]).abs() / w[c].max(0.05)))
        .collect();
    let mean = errs.iter().sum::<f32>() / errs.len() as f32;
    let bad = errs.iter().filter(|e| **e > tol).count();
    errs.sort_by(f32::total_cmp);
    (mean, errs[errs.len() * 99 / 100], bad)
}

/// The file bytes; also written to `SPECULAR_HDR_EXPORT_DIR` when set (for outside tools).
fn file(img: &specular_develop::export::Exported, p: &Preset) -> Vec<u8> {
    let b = batch::encode(img, p, None, None).unwrap();
    if let Some(d) = std::env::var_os("SPECULAR_HDR_EXPORT_DIR") {
        let name = format!("{}-{}.{}", p.hdr.id(), p.format.id(), p.format.ext());
        std::fs::write(std::path::Path::new(&d).join(name), &b).unwrap();
    }
    b
}

#[test]
fn pq_and_hlg_files_decode_to_the_hdr_rendition() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let e = edit(2.0);
    for h in [HdrOutput::Pq, HdrOutput::Hlg] {
        let peak = hdr::export_headroom(&e, h);
        let want = hdr_rendition(&mut ex, &e, Space::Rec2020, peak);
        for (format, tol) in [
            (FileFormat::Png, 0.01),
            (FileFormat::Jxl, 0.08),
            (FileFormat::Avif, 0.08),
        ] {
            let p = Preset {
                format,
                hdr: h,
                quality: 95,
                ..Preset::default()
            }
            .normalised();
            let img =
                hdr::render(&mut ex, scene(), &e, Orient::default(), &p, &mut |_| true).unwrap();
            let bytes = file(&img, &p);
            let (r, _) = decode(&bytes, format.ext());
            let specular_core::PixelLayout::Rgba16(enc) = r.layout else {
                panic!("{h:?} {format:?}: decoded as {:?}", r.layout)
            };
            let tf = match h {
                HdrOutput::Hlg => specular_core::hdr::Transfer::Hlg,
                _ => specular_core::hdr::Transfer::Pq,
            };
            assert_eq!(
                enc,
                HdrEncoding {
                    transfer: tf,
                    primaries: Primaries::Bt2020
                },
                "{h:?} {format:?}"
            );
            let px: Vec<u16> = r.planes[0]
                .data
                .chunks_exact(2)
                .map(|b| u16::from_ne_bytes([b[0], b[1]]))
                .collect();
            let got: Vec<[f32; 3]> = px
                .chunks_exact(4)
                .map(|c| decode16(enc, [c[0], c[1], c[2]]))
                .collect();
            let (mean, p99, bad) = compare(&got, &want, tol);
            let top = got.iter().map(|c| c[1]).fold(0f32, f32::max);
            eprintln!(
                "{h:?} {format:?}: {} bytes, mean {mean:.4}, p99 {p99:.4}, {bad} of {} over {tol}, top {top:.2} (peak {peak:.2})",
                bytes.len(),
                got.len() * 3
            );
            assert!(mean < tol / 4.0 && p99 < tol, "{h:?} {format:?}");
            assert!(bad * 100 < got.len() * 3, "{h:?} {format:?}: {bad} off");
            assert!(top > 0.8 * peak, "{h:?} {format:?}: top {top}");
        }
    }
}

#[test]
fn gain_maps_rebuild_the_hdr_rendition_in_our_viewer() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let e = edit(2.0);
    let peak = hdr::export_headroom(&e, HdrOutput::GainMap);
    assert_eq!(peak, 4.0);
    let want = hdr_rendition(&mut ex, &e, Space::Srgb, peak);
    for format in [FileFormat::Jpeg, FileFormat::Avif, FileFormat::Jxl] {
        let p = Preset {
            format,
            hdr: HdrOutput::GainMap,
            quality: 95,
            ..Preset::default()
        }
        .normalised();
        let img = hdr::render(&mut ex, scene(), &e, Orient::default(), &p, &mut |_| true).unwrap();
        let bytes = file(&img, &p);
        let (r, g) = decode(&bytes, format.ext());
        let g = g.unwrap_or_else(|| panic!("{format:?}: no gain map read back"));
        let params: GainMapParams = g.params;
        assert!(!params.base_is_hdr, "{format:?}");
        assert!((params.alt_headroom - 2.0).abs() < 1e-4, "{format:?}");
        assert_eq!((g.map.width, g.map.height), (W, H), "{format:?}");
        let base = base_rgb(&r);
        let weight = params.weight(peak);
        let map = &g.map.planes[0];
        let got: Vec<[f32; 3]> = base
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let (x, y) = (i % W as usize, i / W as usize);
                let m = &map.data[y * map.stride + x * 4..];
                let lin = b.map(|v| Transfer::Srgb.decode(f64::from(v) / 255.0) as f32);
                let gs = [m[0], m[1], m[2]].map(|v| f32::from(v) / 255.0);
                params.apply_rgb(lin, gs, weight)
            })
            .collect();
        let tol = 0.1;
        let (mean, p99, bad) = compare(&got, &want, tol);
        eprintln!(
            "gain map {format:?}: {} bytes, mean {mean:.4}, p99 {p99:.4}, {bad} of {} over {tol}",
            bytes.len(),
            got.len() * 3
        );
        assert!(mean < 0.03 && p99 < tol, "{format:?}");
        // Shown at headroom 1 it is the SDR base.
        assert_eq!(params.weight(1.0), 0.0);
    }
}

/// The SDR preview (headroom 1) is the SDR export: the same graph, the same numbers.
#[test]
fn the_sdr_preview_equals_the_sdr_export() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let e = edit(3.0);
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let export = hdr_rendition(&mut ex, &e, Space::Srgb, 1.0);
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    dv.set_input(scene()).unwrap();
    let h = view::headroom(&e, 1.0);
    assert_eq!(h, 1.0);
    let shown = view::for_headroom(&e, h);
    assert!(matches!(shown, std::borrow::Cow::Borrowed(_)));
    assert_eq!(dv.prepare(&shown, W).unwrap(), (W, H));
    let r = dv.render(&shown, None, &OutputSpace::rec709()).unwrap();
    let v = dv.read(&r).unwrap();
    dv.recycle(r);
    let mut worst = 0f32;
    for (i, w) in export.iter().enumerate() {
        for c in 0..3 {
            worst = worst.max((v[i * 4 + c] - w[c]).abs());
        }
    }
    eprintln!("SDR preview vs SDR export: worst {worst:e}");
    // The preview stores f16: within its rounding.
    assert!(worst < 2e-3, "{worst}");
}

/// The HDR rendition goes up to the headroom and keeps mid grey and the shadows of the SDR
/// one; every limit, every method.
#[test]
fn the_hdr_rendition_reaches_the_headroom() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    for method in ["sigmoid", "filmic", "agx"] {
        let mut e = edit(2.0);
        let m = e.module_mut(ModuleKind::VIEW_TRANSFORM, 0).unwrap();
        m.params
            .overlay(&specular_develop::model::Fields::new().with(
                "method",
                specular_develop::model::Value::Text(method.into()),
            ));
        let sdr = hdr_rendition(&mut ex, &e, Space::Rec2020, 1.0);
        for peak in [2.0f32, 4.0] {
            let hdr = hdr_rendition(&mut ex, &e, Space::Rec2020, peak);
            let lum = |c: &[f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            let top = hdr.iter().map(lum).fold(0f32, f32::max);
            let sdr_top = sdr.iter().map(lum).fold(0f32, f32::max);
            eprintln!("{method} peak {peak}: HDR top {top:.3}, SDR top {sdr_top:.3}");
            // AgX shows a longer scene range: its top of this +5 EV scene is lower.
            assert!(
                top <= peak * 1.001 && top > 1.3 * sdr_top,
                "{method} {peak}: {top}"
            );
            assert!(sdr_top <= 1.001);
            // The darkest third barely moves.
            for i in (0..hdr.len()).filter(|i| (i % W as usize) < W as usize / 3) {
                let (a, b) = (lum(&hdr[i]), lum(&sdr[i]));
                assert!(
                    (a - b).abs() <= 0.25 * b + 1e-3,
                    "{method} {peak}: {a} vs {b}"
                );
            }
        }
    }
}

/// Slider latency with each view method, SDR (8-bit texture) and HDR (headroom 4,
/// half-float texture), at 2560×1600: the §3.9 point-op budget (p50 ≤ 16 ms to present,
/// p95 ≤ 33 ms). A timing report: run by hand.
#[test]
#[ignore = "timing report; run by hand"]
fn hdr_slider_latency() {
    use specular_develop::model::{Fields, Value};
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (2560u32, 1600u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = x as f32 / w as f32;
            let s = y as f32 / h as f32;
            rgba.extend_from_slice(&[
                (t * 12.0 - 7.0).exp2(),
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
    let tex = |format| {
        d.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    };
    let sdr_tex = tex(wgpu::TextureFormat::Rgba8UnormSrgb);
    let hdr_tex = tex(wgpu::TextureFormat::Rgba16Float);
    for method in ["sigmoid", "filmic", "agx"] {
        for peak in [1.0f32, 4.0] {
            let mut e = edit(2.0);
            let mut wall = Vec::new();
            dv.prepare(&e, w).unwrap();
            for i in 0..100 {
                let m = e.module_mut(ModuleKind::VIEW_TRANSFORM, 0).unwrap();
                m.params.overlay(
                    &Fields::new()
                        .with("method", Value::Text(method.into()))
                        .with("contrast", Value::Float(1.0 + (i % 40) as f32 * 0.03)),
                );
                let t0 = std::time::Instant::now();
                let hr = view::headroom(&e, peak);
                let shown = view::for_headroom(&e, hr);
                let out = OutputSpace {
                    peak: hr,
                    ..OutputSpace::rec709()
                };
                let r = dv
                    .render(&shown, Some((ModuleKind::VIEW_TRANSFORM, 0)), &out)
                    .unwrap();
                let t = if hr > 1.0 { &hdr_tex } else { &sdr_tex };
                dv.present(r, Orient::default(), t).unwrap();
                dv.wait().unwrap();
                if i >= 20 {
                    wall.push(t0.elapsed().as_secs_f64() * 1e3);
                }
            }
            wall.sort_by(f64::total_cmp);
            let (p50, p95) = (wall[wall.len() / 2], wall[wall.len() * 95 / 100]);
            eprintln!("{method} peak {peak}: contrast slider p50 {p50:.2} ms, p95 {p95:.2} ms");
        }
    }
}

/// `edit(ev)` with modules before the view transform that are worth sharing (clarity on the
/// statistics grid, the tone equalizer).
fn rich_edit(ev: f32) -> Edit {
    use specular_develop::model::{Fields, ModuleInstance, Params, Value};
    let reg = Registry::builtin();
    let mut e = edit(ev);
    for (kind, f) in [
        (
            ModuleKind::LOCAL_CONTRAST,
            Fields::new().with("clarity", Value::Float(0.5)),
        ),
        (
            ModuleKind::TONE_EQUALIZER,
            Fields::new().with("shadows_ev", Value::Float(1.0)),
        ),
    ] {
        let mut m = ModuleInstance::new(reg, kind, 0);
        m.params = Params::from_fields(kind, f);
        e.insert(reg, m);
    }
    e
}

/// The gain-map export renders once: both renditions from one pass equal the two renders
/// made apart (the SDR base to the code, the HDR one to float rounding).
#[test]
fn one_pass_gives_both_gain_map_renditions() {
    use specular_develop::export::Rendition;
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let e = rich_edit(2.0);
    let peak = hdr::export_headroom(&e, HdrOutput::GainMap);
    let sdr_p = Preset {
        format: FileFormat::Jpeg,
        ..Preset::default()
    }
    .normalised();
    let hdr_p = Preset {
        format: FileFormat::Tiff,
        depth: Depth::Float,
        ..Preset::default()
    }
    .normalised();
    let shown = view::for_headroom(&e, peak);
    let o = Orient::default();
    let apart = |ex: &mut Exporter| {
        let a = ex.render(scene(), &e, o, &sdr_p, &mut |_| true).unwrap();
        let b = ex
            .render_at(scene(), &shown, o, &hdr_p, peak, &mut |_| true)
            .unwrap();
        (a, b)
    };
    let list = [
        Rendition {
            edit: &e,
            preset: &sdr_p,
            peak: 1.0,
        },
        Rendition {
            edit: &shown,
            preset: &hdr_p,
            peak,
        },
    ];
    let (a, b) = apart(&mut ex);
    let v = ex
        .render_renditions(scene(), o, &list, &mut |_| true)
        .unwrap();
    let (Samples::Eight(s0), Samples::Eight(s1)) = (&a.samples, &v[0].samples) else {
        panic!("8-bit")
    };
    let off = s0
        .iter()
        .zip(s1)
        .filter(|(x, y)| x.abs_diff(**y) > 1)
        .count();
    let (Samples::Float(h0), Samples::Float(h1)) = (&b.samples, &v[1].samples) else {
        panic!("float")
    };
    let worst = h0
        .iter()
        .zip(h1)
        .map(|(x, y)| (x - y).abs() / x.abs().max(1e-3))
        .fold(0f32, f32::max);
    eprintln!(
        "renditions: SDR {off} of {} codes off by more than 1, HDR worst relative {worst:.2e}",
        s0.len()
    );
    assert_eq!(off, 0);
    assert!(worst < 1e-4, "{worst}");
    // Timing (the machine may be loaded): apart against together.
    let t = std::time::Instant::now();
    for _ in 0..3 {
        apart(&mut ex);
    }
    let two = t.elapsed().as_secs_f64() / 3.0 * 1e3;
    let t = std::time::Instant::now();
    for _ in 0..3 {
        ex.render_renditions(scene(), o, &list, &mut |_| true)
            .unwrap();
    }
    let one = t.elapsed().as_secs_f64() / 3.0 * 1e3;
    eprintln!("gain-map renders {W}x{H}: apart {two:.1} ms, one pass {one:.1} ms");
}

/// Borders and watermark on a gain-map export: on the base and in the map, which has the
/// framed size; the border's gain is nil, the photo's gains are kept.
#[test]
fn gain_map_exports_get_their_border() {
    use specular_develop::model::{ModuleInstance, Params, Value};
    use specular_develop::ops::effects::frame::{Frame, Vars};
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let reg = Registry::builtin();
    let mut e = edit(2.0);
    let mut m = ModuleInstance::new(reg, ModuleKind::FRAME, 0);
    m.params = Params::from_fields(
        ModuleKind::FRAME,
        Frame::default_fields()
            .with("border", Value::Float(0.05))
            .with("colour", Value::Floats(vec![1.0, 1.0, 1.0])),
    );
    m.enabled = true;
    e.insert(reg, m);
    let p = Preset {
        format: FileFormat::Jpeg,
        hdr: HdrOutput::GainMap,
        ..Preset::default()
    }
    .normalised();
    let plain = hdr::render(&mut ex, scene(), &e, Orient::default(), &p, &mut |_| true).unwrap();
    let img = hdr::render_framed(
        &mut ex,
        scene(),
        &e,
        Orient::default(),
        &p,
        Some(&Vars::default()),
        &mut |_| true,
    )
    .unwrap();
    let b = (0.05 * W as f32).round() as u32;
    assert_eq!((img.width, img.height), (W + 2 * b, H + 2 * b));
    let g = img.gain_map.as_ref().expect("gain map");
    assert_eq!((g.width, g.height), (img.width, img.height));
    let gain = |x: u32, y: u32, c: usize| {
        let s = f32::from(g.samples[((y * g.width + x) * 3) as usize + c]) / 255.0;
        g.params.gain_min[c] + s * (g.params.gain_max[c] - g.params.gain_min[c])
    };
    // The border: no gain (within a quantisation step).
    for (x, y) in [
        (0, 0),
        (img.width - 1, img.height / 2),
        (b / 2, img.height - 1),
    ] {
        for c in 0..3 {
            let step = (g.params.gain_max[c] - g.params.gain_min[c]) / 255.0;
            assert!(gain(x, y, c).abs() <= step, "({x}, {y}): {}", gain(x, y, c));
        }
    }
    // Inside: the unframed export's gains, moved by the border.
    let pg = plain.gain_map.as_ref().unwrap();
    let mut worst = 0f32;
    for (x, y) in [(W - 2, H / 2), (W / 2, H / 3), (10, 10)] {
        for c in 0..3 {
            let s = f32::from(pg.samples[((y * pg.width + x) * 3) as usize + c]) / 255.0;
            let want = pg.params.gain_min[c] + s * (pg.params.gain_max[c] - pg.params.gain_min[c]);
            worst = worst.max((gain(x + b, y + b, c) - want).abs());
        }
    }
    eprintln!("framed gain map: inside worst {worst:.3} stops");
    assert!(worst < 0.05, "{worst}");
}

/// Gain maps state the space their gains apply in (the base's, ISO 21496-1
/// `use_base_colour_space`, with the base's profile or `nclx`), and a JPEG's primary lists
/// the map in its XMP container directory even though it carries XMP of its own.
#[test]
fn gain_maps_state_their_space_and_directory() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let e = edit(2.0);
    for (space, want) in [
        (Space::Srgb, Primaries::Bt709),
        (Space::DisplayP3, Primaries::DisplayP3),
    ] {
        for format in [FileFormat::Jpeg, FileFormat::Avif, FileFormat::Jxl] {
            let p = Preset {
                format,
                space,
                hdr: HdrOutput::GainMap,
                quality: 90,
                ..Preset::default()
            }
            .normalised();
            let img =
                hdr::render(&mut ex, scene(), &e, Orient::default(), &p, &mut |_| true).unwrap();
            let bytes = file(&img, &p);
            let (_, g) = decode(&bytes, format.ext());
            let g = g.unwrap_or_else(|| panic!("{format:?}: no gain map"));
            eprintln!("{space:?} {format:?}: gains in {:?}", g.params.space);
            assert_eq!(g.params.space, want, "{space:?} {format:?}");
            if format == FileFormat::Jpeg {
                let text = String::from_utf8_lossy(&bytes);
                assert_eq!(text.matches("<Container:Directory>").count(), 1);
                // The directory's map length is the bytes after the primary.
                let at = text.find("Item:Semantic=\"GainMap\"").expect("map item");
                let len: usize = text[at..]
                    .split("Item:Length=\"")
                    .nth(1)
                    .and_then(|t| t.split('"').next())
                    .and_then(|t| t.parse().ok())
                    .expect("a length");
                assert!(bytes[bytes.len() - len..].starts_with(&[0xFF, 0xD8]));
            }
        }
    }
}
