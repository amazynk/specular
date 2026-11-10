//! Preview = export (improvement 14): for every Develop module and a few
//! realistic stacks, on a synthetic Bayer raw, a synthetic X-Trans raw, a JPEG and a 16-bit
//! TIFF, the full-size export (f32) against
//! - the 1:1 view rendered in tiles (f16): CIEDE2000 p99 < 0.5;
//! - the fit view (a reduced source) against the export area-reduced to its size: p95 < 1
//!   for scale-stable modules; detail modules are reported only, they change
//!   detail a reduced view cannot show (users are told to check at 100 %);
//! - the edited thumbnail against the export reduced to its size: p95 < 1, same
//!   split.
//!
//! The quick test runs a subset; the sweep prints the whole table of
//! `docs/develop-quality.md`:
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test quality_sweep
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test quality_sweep -- \
//!       --ignored --nocapture quality_sweep_table

#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;
mod support;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use common::{DngCodec, DngSpec, tag_ascii, write_dng};
use specular_convert::Resize;
use specular_core::CancelToken;
use specular_develop::export::batch;
use specular_develop::export::{Depth, Exporter, FileFormat, Preset, Samples, Space, Transfer};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::params::CropOrientation;
use specular_develop::model::{
    Blend, Edit, MaskExpr, MaskShape, ModuleInstance, ModuleKind, Params, Registry, Shape,
    SourceKey, Value,
};
use specular_develop::ops::detail::diffuse::Preset as DiffusePreset;
use specular_develop::ops::output::OutputSpace;
use specular_develop::ops::retouch::{Retouch, Spot, SpotMode};
use specular_develop::preview::{self, Developer, Input, Orient};
use specular_develop::thumb::{self, Source, Thumbnailer};
use specular_raw::sensor::{BlackLevels, Layout, SensorMeta};
use specular_raw::{Cfa, MatrixSource, Rect, SensorData, WbSource};
use support::{Stats, de2000, lab_linear_srgb, srgb_decode};

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

/// GPU tests one at a time.
fn lock() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn tmpdir() -> PathBuf {
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let d = base.join(format!("specular-quality-sweep-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------------------------------------------------------------------------------------
// The scene

/// Linear camera RGB (0 … about 1.1) at `(x, y)` of a `w × h` frame: smooth colour fields, a
/// saturated disc, hard edges at several angles, fine stripes, a near-white patch that
/// clips and glows, skin- and sky-like patches, and a little noise.
fn scene(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
    let mut c = [
        0.06 + 0.5 * (0.5 + 0.5 * (fx * 5.0).sin()),
        0.06 + 0.45 * (0.5 + 0.5 * (fy * 3.0 + 1.0).sin()),
        0.06 + 0.5 * (0.5 + 0.5 * ((fx + fy) * 4.0).cos()),
    ];
    let (dx, dy) = (fx - 0.3, (fy - 0.4) * h as f32 / w as f32);
    if dx * dx + dy * dy < 0.012 {
        c = [0.75, 0.12, 0.08];
    }
    // A slanted edge.
    if fx + 0.35 * fy > 0.82 && fy < 0.5 {
        c = [0.08, 0.22, 0.65];
    }
    // Fine stripes (period 12 px) in the lower left (kept at the earlier period so the
    // tables compare).
    if fx < 0.25 && fy > 0.7 {
        let s = if (x / 6).is_multiple_of(2) {
            0.55
        } else {
            0.12
        };
        c = [s, s * 0.95, s * 0.9];
    }
    // Skin-like and sky-like patches.
    if (0.45..0.6).contains(&fx) && (0.62..0.85).contains(&fy) {
        c = [0.55, 0.36, 0.27];
    }
    if (0.65..0.95).contains(&fx) && (0.6..0.72).contains(&fy) {
        c = [0.3, 0.45, 0.7];
    }
    // A bright patch near and above white.
    if (0.7..0.8).contains(&fx) && (0.8..0.92).contains(&fy) {
        c = [1.1, 1.05, 0.95];
    }
    let mut k = u64::from(y) << 32 | u64::from(x) | 1;
    k ^= k >> 12;
    k ^= k << 25;
    k ^= k >> 27;
    let u = (k.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32;
    c.map(|v| (v + 0.012 * (u - 0.5)).max(0.0))
}

/// A Bayer (RGGB) DNG of [`scene`], white at 65535.
fn bayer_dng(w: u32, h: u32) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.extra = vec![tag_ascii(271, "Specular"), tag_ascii(272, "Test Camera")];
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = scene(x, y, w, h);
            let v = match (y % 2, x % 2) {
                (0, 0) => c[0],
                (1, 1) => c[2],
                _ => c[1],
            };
            data.push((v.clamp(0.0, 1.0) * 65535.0).round() as u16);
        }
    }
    write_dng(&s, &data)
}

/// The long side the app reads non-raws at for Develop (`app/develop/preview.rs` `SIDE`).
const APP_SIDE: u32 = 2560;

/// Fujifilm's X-Trans layout.
const XTRANS: [&str; 6] = ["GGRGGB", "GGBGGR", "BRGRBG", "GGBGGR", "GGRGGB", "RBGBRG"];

/// An X-Trans sensor of [`scene`] (black 512, white 15000, a camera matrix and as-shot
/// white like a Fujifilm's).
fn xtrans_sensor(w: u32, h: u32) -> SensorData {
    let colours: Vec<u8> = XTRANS
        .iter()
        .flat_map(|r| {
            r.bytes().map(|b| match b {
                b'R' => 0,
                b'G' => 1,
                _ => 2,
            })
        })
        .collect();
    let layout = Layout::Pattern(Cfa::new(6, 6, colours).unwrap());
    let (black, white) = (512.0f32, 15000.0f32);
    let wb = [1.9f32, 1.0, 1.4];
    let mut samples = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = scene(x, y, w, h);
            // The camera sees the scene through its white balance's inverse.
            let ch = usize::from(layout.color(x, y)).min(2);
            let v = black + c[ch] / wb[ch] * (white - black);
            samples.push(v.clamp(0.0, 65535.0) as u16);
        }
    }
    SensorData {
        width: w,
        height: h,
        cpp: 1,
        samples,
        float: None,
        crop: Rect { x: 0, y: 0, w, h },
        meta: SensorMeta {
            layout,
            black: BlackLevels::simple([black; 4], 1),
            white,
            clip_white: white,
            linearization: None,
            opcodes: Default::default(),
            opcodes_unknown: Vec::new(),
            wb,
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

fn srgb_encode(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// [`scene`] as sRGB-coded 8-bit RGB.
fn scene_rgb8(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in scene(x, y, w, h) {
                v.push((srgb_encode(c * 0.85) * 255.0).round() as u8);
            }
        }
    }
    v
}

/// [`scene`] as an sRGB-coded 16-bit RGB baseline TIFF (uncompressed, one strip).
fn scene_tiff16(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 6) as usize);
    for y in 0..h {
        for x in 0..w {
            for c in scene(x, y, w, h) {
                let v = (srgb_encode(c * 0.85) * 65535.0).round() as u16;
                px.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    let n_tags: u16 = 10;
    let ifd = 8u32;
    let after_ifd = ifd + 2 + u32::from(n_tags) * 12 + 4;
    let bps_at = after_ifd;
    let data_at = bps_at + 6;
    let mut o = Vec::with_capacity(data_at as usize + px.len());
    o.extend_from_slice(b"II*\0");
    o.extend_from_slice(&ifd.to_le_bytes());
    o.extend_from_slice(&n_tags.to_le_bytes());
    let tag = |o: &mut Vec<u8>, t: u16, typ: u16, count: u32, val: u32| {
        o.extend_from_slice(&t.to_le_bytes());
        o.extend_from_slice(&typ.to_le_bytes());
        o.extend_from_slice(&count.to_le_bytes());
        if typ == 3 && count == 1 {
            o.extend_from_slice(&(val as u16).to_le_bytes());
            o.extend_from_slice(&[0, 0]);
        } else {
            o.extend_from_slice(&val.to_le_bytes());
        }
    };
    tag(&mut o, 256, 4, 1, w); // ImageWidth
    tag(&mut o, 257, 4, 1, h); // ImageLength
    tag(&mut o, 258, 3, 3, bps_at); // BitsPerSample
    tag(&mut o, 259, 3, 1, 1); // Compression: none
    tag(&mut o, 262, 3, 1, 2); // RGB
    tag(&mut o, 273, 4, 1, data_at); // StripOffsets
    tag(&mut o, 277, 3, 1, 3); // SamplesPerPixel
    tag(&mut o, 278, 4, 1, h); // RowsPerStrip
    tag(&mut o, 279, 4, 1, px.len() as u32); // StripByteCounts
    tag(&mut o, 284, 3, 1, 1); // Chunky
    o.extend_from_slice(&0u32.to_le_bytes());
    for _ in 0..3 {
        o.extend_from_slice(&16u16.to_le_bytes());
    }
    assert_eq!(o.len(), data_at as usize);
    o.extend_from_slice(&px);
    o
}

/// A mild warming .cube (9³) in the LUT module's default space.
fn warm_cube(dir: &Path) -> PathBuf {
    let p = dir.join("warm.cube");
    let mut s = String::from("TITLE \"warm\"\nLUT_3D_SIZE 9\n");
    for b in 0..9 {
        for g in 0..9 {
            for r in 0..9 {
                let f = |i: u32, k: f32| (i as f32 / 8.0).powf(0.92) * k;
                s.push_str(&format!(
                    "{:.6} {:.6} {:.6}\n",
                    f(r, 1.0).min(1.0),
                    f(g, 0.98),
                    f(b, 0.9)
                ));
            }
        }
    }
    std::fs::write(&p, s).unwrap();
    p
}

// ---------------------------------------------------------------------------------------------
// The inputs

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Bayer,
    XTrans,
    Jpeg,
    Tiff,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Bayer => "Bayer",
            Kind::XTrans => "X-Trans",
            Kind::Jpeg => "JPEG",
            Kind::Tiff => "TIFF 16",
        }
    }

    fn raw(self) -> bool {
        matches!(self, Kind::Bayer | Kind::XTrans)
    }
}

/// One input at its three sizes: the full-size source (1:1 view and export), the fit
/// view's source and the thumbnail's.
struct Photo {
    kind: Kind,
    full: Input,
    fit: Input,
    thumb: Input,
    /// A non-raw's thumbnail source when the edit's warp resamples ([`thumb::source_long`]).
    thumb_warped: Option<Input>,
    /// The long side the fit view is prepared at.
    fit_long: u32,
    as_shot: Option<[f64; 2]>,
}

fn photo(kind: Kind, w: u32, h: u32, dir: &Path) -> Photo {
    let fit_long = w.max(h) / 2;
    match kind {
        Kind::Bayer => {
            let f = bayer_dng(w, h);
            let input = preview::decode_raw(&f).unwrap();
            let as_shot = match &input {
                Input::Raw(s) => preview::as_shot_white(s),
                Input::Rgb { .. } => None,
            };
            Photo {
                kind,
                full: input.clone(),
                fit: input.clone(),
                thumb: input,
                thumb_warped: None,
                fit_long,
                as_shot,
            }
        }
        Kind::XTrans => {
            let input = Input::Raw(Box::new(xtrans_sensor(w, h)));
            Photo {
                kind,
                full: input.clone(),
                fit: input.clone(),
                thumb: input,
                thumb_warped: None,
                fit_long,
                as_shot: None,
            }
        }
        Kind::Jpeg | Kind::Tiff => {
            let path = if kind == Kind::Jpeg {
                let p = dir.join("scene.jpg");
                let mut out = Vec::new();
                let enc = jpeg_encoder::Encoder::new(&mut out, 95);
                enc.encode(
                    &scene_rgb8(w, h),
                    w as u16,
                    h as u16,
                    jpeg_encoder::ColorType::Rgb,
                )
                .unwrap();
                std::fs::write(&p, out).unwrap();
                p
            } else {
                let p = dir.join("scene.tif");
                std::fs::write(&p, scene_tiff16(w, h)).unwrap();
                p
            };
            // As export reads it (full size), as Develop is given it (the app reads
            // non-raws at up to 2560 px: this one at its real size; the developer reduces
            // it to the view), and as a thumbnail starts.
            let reg = specular_decode::Registry::new();
            let full = batch::read_input(&path, &reg).ok().unwrap().input;
            let s = reg.open_path(&path).unwrap();
            let cancel = CancelToken::new();
            let raster = s
                .decode(&specular_decode::DecodeRequest::full(1), &cancel)
                .unwrap();
            let fit =
                specular_develop::source::input(&raster, None, Some(APP_SIDE), &cancel).unwrap();
            let thumb =
                specular_develop::source::input(&raster, None, Some(thumb::SOURCE_LONG), &cancel)
                    .unwrap();
            let thumb_warped = specular_develop::source::input(
                &raster,
                None,
                Some(thumb::SOURCE_LONG_WARPED),
                &cancel,
            );
            Photo {
                kind,
                full,
                fit,
                thumb,
                thumb_warped,
                fit_long,
                as_shot: None,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The cases

/// How a case's fit view and thumbnail are held to the export.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scale {
    /// Scale-stable: fit view and thumbnail p95 < 1.
    Stable,
    /// Changes detail finer than a reduced view shows (check at 100 %): reported only.
    Detail,
}

struct Case {
    name: &'static str,
    scale: Scale,
    /// Raws only (sensor corrections, demosaic).
    raw_only: bool,
    apply: fn(&mut Edit, &Path),
}

fn module(e: &mut Edit, kind: ModuleKind, instance: u16) -> &mut ModuleInstance {
    let r = Registry::builtin();
    if e.module_mut(kind, instance).is_none() {
        e.insert(r, ModuleInstance::new(r, kind, instance));
    }
    let m = e.module_mut(kind, instance).unwrap();
    m.enabled = true;
    m
}

fn set(e: &mut Edit, kind: ModuleKind, fields: &[(&str, Value)]) {
    set_in(e, kind, 0, fields);
}

fn set_in(e: &mut Edit, kind: ModuleKind, instance: u16, fields: &[(&str, Value)]) {
    let m = module(e, kind, instance);
    let mut f = m.params.to_fields();
    for (n, v) in fields {
        f.set(n, v.clone());
    }
    m.params = Params::from_fields(kind, f);
}

fn fl(v: f32) -> Value {
    Value::Float(v)
}

fn text(v: &str) -> Value {
    Value::Text(v.to_owned())
}

fn diffuse(e: &mut Edit, p: DiffusePreset) {
    let m = module(e, ModuleKind::DIFFUSE, 0);
    m.params = Params::from_fields(ModuleKind::DIFFUSE, p.params().to_fields());
}

fn crop(e: &mut Edit, angle: f32, rect: [f32; 4]) {
    let m = module(e, ModuleKind::CROP_ORIENTATION, 0);
    m.params = Params::CropOrientation(CropOrientation {
        angle_deg: angle,
        crop: rect,
        ..Default::default()
    });
}

fn retouch(e: &mut Edit) {
    let spots = vec![
        Spot {
            mode: SpotMode::Heal,
            target: [0.3, 0.4],
            source: [0.4, 0.3],
            size: 0.03,
            ..Spot::default()
        },
        Spot {
            mode: SpotMode::Clone,
            target: [0.6, 0.3],
            source: [0.5, 0.2],
            size: 0.025,
            ..Spot::default()
        },
        Spot {
            mode: SpotMode::Blur,
            target: [0.52, 0.72],
            source: [0.52, 0.72],
            size: 0.05,
            blur: 0.004,
            ..Spot::default()
        },
    ];
    let r = Retouch {
        spots,
        ..Retouch::default()
    };
    let m = module(e, ModuleKind::RETOUCH, 0);
    m.params = Params::from_fields(ModuleKind::RETOUCH, r.to_fields());
}

/// An ellipse-masked second exposure instance (a brightened subject).
fn masked_exposure(e: &mut Edit) {
    e.masks.push(MaskShape {
        id: 1,
        shape: Shape::Ellipse {
            centre: [0.5, 0.55],
            radii: [0.22, 0.18],
            angle_deg: 10.0,
            feather: 0.5,
        },
    });
    set_in(e, ModuleKind::EXPOSURE, 1, &[("exposure_ev", fl(0.6))]);
    module(e, ModuleKind::EXPOSURE, 1).blend = Some(Blend {
        mask: MaskExpr::Shape(1),
        ..Blend::default()
    });
}

fn cases() -> Vec<Case> {
    use ModuleKind as K;
    use Scale::{Detail, Stable};
    let c = |name, scale, raw_only, apply| Case {
        name,
        scale,
        raw_only,
        apply,
    };
    vec![
        c("default", Stable, false, |_, _| {}),
        c("exposure +0.7 EV", Stable, false, |e, _| {
            set(e, K::EXPOSURE, &[("exposure_ev", fl(0.7))]);
        }),
        c("auto exposure", Stable, false, |e, _| {
            set(e, K::EXPOSURE, &[("auto", Value::Bool(true))]);
        }),
        c("white balance 4300 K-ish", Stable, false, |e, _| {
            set(
                e,
                K::COLOUR_CALIBRATION,
                &[
                    ("white_source", text("custom")),
                    ("white_x", fl(0.365)),
                    ("white_y", fl(0.37)),
                ],
            );
        }),
        c("look: filmic", Stable, false, |e, _| {
            set(
                e,
                K::VIEW_TRANSFORM,
                &[("method", text("filmic")), ("contrast", fl(1.8))],
            );
        }),
        c("look: AgX", Stable, false, |e, _| {
            set(e, K::VIEW_TRANSFORM, &[("method", text("agx"))]);
        }),
        c("crop and straighten", Stable, false, |e, _| {
            crop(e, 2.5, [0.1, 0.12, 0.85, 0.9]);
        }),
        c(
            "lens: distortion, TCA, vignetting",
            Stable,
            false,
            |e, _| {
                set(
                    e,
                    K::LENS,
                    &[
                        ("distortion", fl(15.0)),
                        ("tca_red", fl(0.05)),
                        ("vignette", fl(20.0)),
                    ],
                );
            },
        ),
        c("tone equalizer", Stable, false, |e, _| {
            set(
                e,
                K::TONE_EQUALIZER,
                &[("highlights_ev", fl(-1.0)), ("shadows_ev", fl(1.0))],
            );
        }),
        c("local contrast: clarity", Stable, false, |e, _| {
            set(e, K::LOCAL_CONTRAST, &[("clarity", fl(0.6))]);
        }),
        c("local contrast: texture", Detail, false, |e, _| {
            set(e, K::LOCAL_CONTRAST, &[("texture", fl(0.6))]);
        }),
        c("colour balance and wheels", Stable, false, |e, _| {
            set(
                e,
                K::COLOUR_BALANCE,
                &[
                    ("vibrance", fl(0.4)),
                    ("saturation", fl(-0.2)),
                    ("brilliance", fl(0.3)),
                    ("shadows_hue", fl(210.0)),
                    ("shadows_chroma", fl(0.4)),
                    ("highlights_hue", fl(40.0)),
                    ("highlights_chroma", fl(0.3)),
                ],
            );
        }),
        c("colour mixer", Stable, false, |e, _| {
            set(
                e,
                K::COLOUR_EQUALIZER,
                &[
                    ("hue_red", fl(15.0)),
                    ("saturation_blue", fl(0.5)),
                    ("brightness_green", fl(-0.4)),
                ],
            );
        }),
        c("point colour", Stable, false, |e, _| {
            set(
                e,
                K::POINT_COLOUR,
                &[
                    ("picked", Value::Bool(true)),
                    ("target_hue", fl(30.0)),
                    ("target_saturation", fl(0.4)),
                    ("target_lightness", fl(0.5)),
                    ("shift_hue", fl(-20.0)),
                    ("shift_saturation", fl(0.3)),
                ],
            );
        }),
        c("black and white", Stable, false, |e, _| {
            set(
                e,
                K::BLACK_WHITE,
                &[("mix_red", fl(0.3)), ("mix_blue", fl(-0.3))],
            );
        }),
        c("curve and levels", Stable, false, |e, _| {
            set(
                e,
                K::CURVES,
                &[(
                    "points",
                    Value::Floats(vec![0.0, 0.0, 0.25, 0.2, 0.75, 0.82, 1.0, 1.0]),
                )],
            );
        }),
        c("LUT (.cube)", Stable, false, |e, dir| {
            let p = warm_cube(dir);
            set(
                e,
                K::LUT_3D,
                &[("path", text(&p.to_string_lossy())), ("strength", fl(1.0))],
            );
        }),
        c("noise reduction", Detail, false, |e, _| {
            set(
                e,
                K::DENOISE,
                &[("luminance", fl(0.6)), ("colour", fl(0.6))],
            );
        }),
        c("sharpening", Detail, false, |e, _| {
            set(e, K::SHARPEN, &[("amount", fl(1.0))]);
        }),
        c("capture sharpening", Detail, false, |e, _| {
            set(e, K::CAPTURE_SHARPENING, &[("amount", fl(1.0))]);
        }),
        c("diffuse: sharpen", Detail, false, |e, _| {
            diffuse(e, DiffusePreset::Sharpen);
        }),
        c("diffuse: local contrast", Stable, false, |e, _| {
            diffuse(e, DiffusePreset::LocalContrast);
        }),
        c("diffuse: bloom", Stable, false, |e, _| {
            diffuse(e, DiffusePreset::Bloom);
        }),
        c("diffuse: fill highlights", Stable, false, |e, _| {
            diffuse(e, DiffusePreset::InpaintHighlights);
        }),
        c("surface blur", Stable, false, |e, _| {
            set(
                e,
                K::SURFACE_BLUR,
                &[("amount", fl(0.6)), ("radius", fl(0.004))],
            );
        }),
        c("hot pixels", Detail, true, |e, _| {
            set(e, K::HOT_PIXELS, &[("threshold", fl(1.0))]);
        }),
        c("chromatic aberration (auto)", Stable, true, |e, _| {
            set(e, K::CHROMATIC_ABERRATION, &[("auto", Value::Bool(true))]);
        }),
        c("defringe", Stable, false, |e, _| {
            set(e, K::DEFRINGE, &[("amount", fl(0.6))]);
        }),
        c("demosaic: dual", Detail, true, |e, _| {
            set(e, K::RAW_FRONT_END, &[("demosaic", text("dual"))]);
        }),
        c("retouch: heal, clone, blur", Stable, false, |e, _| {
            retouch(e)
        }),
        c("negative", Stable, false, |e, _| {
            set(e, K::NEGATIVE, &[("d_max", fl(1.6)), ("grade", fl(2.5))]);
        }),
        c("dehaze", Stable, false, |e, _| {
            set(e, K::DEHAZE, &[("strength", fl(0.5))]);
        }),
        c("graduated filter", Stable, false, |e, _| {
            set(
                e,
                K::GRADUATED_ND,
                &[
                    ("amount", fl(-1.5)),
                    ("angle", fl(15.0)),
                    ("tint_amount", fl(0.3)),
                ],
            );
        }),
        c("vignette", Stable, false, |e, _| {
            set(
                e,
                K::VIGNETTE,
                &[("amount", fl(-1.0)), ("roundness", fl(0.4))],
            );
        }),
        c("glow: bloom", Stable, false, |e, _| {
            set(e, K::GLOW, &[("amount", fl(0.6)), ("threshold", fl(0.5))]);
        }),
        c("glow: soften", Stable, false, |e, _| {
            set(
                e,
                K::GLOW,
                &[
                    ("amount", fl(0.5)),
                    ("threshold", fl(-2.0)),
                    ("mode", text("soften")),
                ],
            );
        }),
        c("grain", Detail, false, |e, _| {
            set(
                e,
                K::GRAIN,
                &[("amount", fl(0.4)), ("seed", Value::Int(1234))],
            );
        }),
        c("masked exposure (ellipse)", Stable, false, |e, _| {
            masked_exposure(e)
        }),
        c("stack: portrait", Detail, false, |e, _| {
            set(e, K::EXPOSURE, &[("exposure_ev", fl(0.4))]);
            set(
                e,
                K::TONE_EQUALIZER,
                &[("highlights_ev", fl(-0.6)), ("shadows_ev", fl(0.5))],
            );
            set(e, K::LOCAL_CONTRAST, &[("clarity", fl(-0.2))]);
            set(e, K::COLOUR_BALANCE, &[("vibrance", fl(0.25))]);
            set(e, K::DENOISE, &[("luminance", fl(0.3))]);
            set(e, K::SHARPEN, &[("amount", fl(0.5))]);
            set(e, K::VIGNETTE, &[("amount", fl(-0.5))]);
            masked_exposure(e);
        }),
        c("stack: landscape", Stable, false, |e, _| {
            crop(e, -1.5, [0.05, 0.05, 0.95, 0.9]);
            set(e, K::LENS, &[("distortion", fl(-10.0))]);
            set(e, K::DEHAZE, &[("strength", fl(0.3))]);
            set(e, K::GRADUATED_ND, &[("amount", fl(-1.0))]);
            set(
                e,
                K::COLOUR_EQUALIZER,
                &[("saturation_blue", fl(0.3)), ("hue_green", fl(-10.0))],
            );
            set(
                e,
                K::CURVES,
                &[(
                    "points",
                    Value::Floats(vec![0.0, 0.0, 0.3, 0.27, 0.7, 0.75, 1.0, 1.0]),
                )],
            );
            set(e, K::VIEW_TRANSFORM, &[("contrast", fl(1.7))]);
        }),
        c("stack: black-and-white film", Detail, false, |e, _| {
            set(e, K::BLACK_WHITE, &[("mix_orange", fl(0.3))]);
            set(
                e,
                K::GLOW,
                &[
                    ("amount", fl(0.3)),
                    ("threshold", fl(-2.0)),
                    ("mode", text("soften")),
                ],
            );
            set(
                e,
                K::GRAIN,
                &[("amount", fl(0.3)), ("seed", Value::Int(99))],
            );
            set(e, K::VIEW_TRANSFORM, &[("method", text("filmic"))]);
        }),
    ]
}

// ---------------------------------------------------------------------------------------------
// The renders

/// Linear BT.709 RGB of a render read back (alpha dropped), clipped as a display does.
fn rgb_of_view(px: &[f32]) -> Vec<[f64; 3]> {
    px.chunks_exact(4)
        .map(|p| [p[0], p[1], p[2]].map(|v| f64::from(v).clamp(0.0, 1.0)))
        .collect()
}

/// The export (16-bit sRGB-coded) in linear light.
fn rgb_of_export(s: &Samples, n: usize) -> Vec<[f64; 3]> {
    let Samples::Sixteen(px) = s else {
        panic!("16-bit")
    };
    let ch = px.len() / n;
    px.chunks_exact(ch)
        .map(|p| [p[0], p[1], p[2]].map(|v| Transfer::Srgb.decode(f64::from(v) / 65535.0)))
        .collect()
}

/// Area reduction in linear light of `w × h` to `dw × dh`.
fn reduce(src: &[[f64; 3]], w: u32, h: u32, dw: u32, dh: u32) -> Vec<[f64; 3]> {
    let (sx, sy) = (f64::from(w) / f64::from(dw), f64::from(h) / f64::from(dh));
    let mut out = Vec::with_capacity((dw * dh) as usize);
    for oy in 0..dh {
        let y0 = f64::from(oy) * sy;
        let y1 = y0 + sy;
        for ox in 0..dw {
            let x0 = f64::from(ox) * sx;
            let x1 = x0 + sx;
            let mut acc = [0f64; 3];
            let mut ws = 0f64;
            for y in (y0.floor() as u32)..(y1.ceil() as u32).min(h) {
                let wy = (f64::from(y) + 1.0).min(y1) - f64::from(y).max(y0);
                for x in (x0.floor() as u32)..(x1.ceil() as u32).min(w) {
                    let wx = (f64::from(x) + 1.0).min(x1) - f64::from(x).max(x0);
                    let k = wx * wy;
                    if k <= 0.0 {
                        continue;
                    }
                    let p = src[(y * w + x) as usize];
                    for c in 0..3 {
                        acc[c] += p[c] * k;
                    }
                    ws += k;
                }
            }
            out.push(acc.map(|a| a / ws.max(1e-12)));
        }
    }
    out
}

/// Writes linear RGB as an sRGB-coded binary PPM (`SPECULAR_SWEEP_DUMP`, for looking).
fn dump(path: &Path, w: u32, h: u32, px: &[[f64; 3]]) {
    let mut o = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in px {
        for c in p {
            o.push((srgb_encode(*c as f32) * 255.0).round() as u8);
        }
    }
    let _ = std::fs::write(path, o);
}

fn compare(a: &[[f64; 3]], b: &[[f64; 3]]) -> Stats {
    assert_eq!(a.len(), b.len());
    Stats::of(
        a.iter()
            .zip(b)
            .map(|(p, q)| de2000(lab_linear_srgb(*p), lab_linear_srgb(*q)))
            .collect(),
    )
}

struct Row {
    input: Kind,
    case: &'static str,
    scale: Scale,
    one: Stats,
    tiles: usize,
    fit: Stats,
    thumb: Stats,
}

fn edit_for(p: &Photo) -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), p.kind.raw());
    if let Some(xy) = p.as_shot {
        batch::set_as_shot(&mut e, xy);
    }
    e
}

/// A developer as the app has one, waiting for the highlight statistics (no provisional
/// first picture).
fn developer(d: &Dev, budget: u64) -> Developer {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(budget));
    dv.wait_highlight_stats = true;
    dv
}

type View = (u32, u32, Vec<f32>, usize);

fn view(dv: &mut Developer, input: Input, edit: &Edit, long: u32) -> View {
    dv.set_input(input).unwrap();
    dv.prepare(edit, long).unwrap();
    let r = dv.render(edit, None, &OutputSpace::rec709()).unwrap();
    let (w, h) = (r.roi.w, r.roi.h);
    let tiles = r.stats.tiles;
    let px = dv.read(&r).unwrap();
    dv.recycle(r);
    (w, h, px, tiles)
}

/// The 1:1 view in tiles of 256 pixels, so the tile seams are part of what is compared.
fn tiled_view(d: &Dev, input: &Input, edit: &Edit, long: u32) -> View {
    let mut dv = developer(d, 1 << 30);
    dv.tile_side = Some(256);
    view(&mut dv, input.clone(), edit, long)
}

fn run(d: &Dev, p: &Photo, case: &Case, dir: &Path) -> Row {
    let mut edit = edit_for(p);
    (case.apply)(&mut edit, dir);

    // The export at full size, 16-bit sRGB.
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        size: Resize::None,
        ..Preset::default()
    };
    let img = ex
        .render(
            p.full.clone(),
            &edit,
            Orient::default(),
            &preset,
            &mut |_| true,
        )
        .unwrap();
    let (ew, eh) = (img.width, img.height);
    let export = rgb_of_export(&img.samples, (ew * eh) as usize);
    drop(img);

    // The 1:1 view in tiles: a budget small enough that the engine tiles it.
    let (long, _) = (ew.max(eh), ());
    let source_long = match &p.full {
        Input::Raw(s) => s.crop.w.max(s.crop.h),
        Input::Rgb { width, height, .. } => (*width).max(*height),
    };
    let (vw, vh, px, tiles) = tiled_view(d, &p.full, &edit, source_long.max(long));
    assert_eq!((vw, vh), (ew, eh), "{}: 1:1 view size", case.name);
    let one = compare(&rgb_of_view(&px), &export);
    drop(px);

    // The fit view.
    let mut dv = developer(d, 1 << 30);
    let (fw, fh, px, _) = view(&mut dv, p.fit.clone(), &edit, p.fit_long);
    drop(dv);
    let fit_view = rgb_of_view(&px);
    let reduced = reduce(&export, ew, eh, fw, fh);
    let fit = compare(&fit_view, &reduced);
    if let Some(dir) = std::env::var_os("SPECULAR_SWEEP_DUMP") {
        let tag = format!("{}-{}", p.kind.name(), case.name).replace([' ', ':', ',', '/'], "_");
        dump(
            &Path::new(&dir).join(format!("{tag}-fit.ppm")),
            fw,
            fh,
            &fit_view,
        );
        dump(
            &Path::new(&dir).join(format!("{tag}-export.ppm")),
            fw,
            fh,
            &reduced,
        );
    }

    // The edited thumbnail.
    let mut th = Thumbnailer::new(&d.device, &d.queue, Budget::fixed(1 << 30));
    // `SPECULAR_SWEEP_THUMB_RAW_LONG=512`: raws developed at this size, then reduced
    // (default the app's, `thumb::RAW_LONG`).
    if let Some(v) = std::env::var("SPECULAR_SWEEP_THUMB_RAW_LONG")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        th.raw_long = Some(v);
    }
    let (tw, tt, px) = th
        .render(
            Source {
                // As the app gives it (`thumb::source_long`).
                input: match &p.thumb_warped {
                    Some(i) if thumb::source_long(&edit) == thumb::SOURCE_LONG_WARPED => i.clone(),
                    _ => p.thumb.clone(),
                },
                orientation: 1,
                frame: None,
                lensfun: None,
                focal35: None,
            },
            &edit,
            thumb::LONG,
        )
        .unwrap();
    let t: Vec<[f64; 3]> = px
        .chunks_exact(4)
        .map(|q| [q[0], q[1], q[2]].map(|v| srgb_decode(f64::from(v) / 255.0)))
        .collect();
    let reduced_t = reduce(&export, ew, eh, tw, tt);
    let thumb = compare(&t, &reduced_t);
    if let Some(dir) = std::env::var_os("SPECULAR_SWEEP_DUMP") {
        let tag = format!("{}-{}", p.kind.name(), case.name).replace([' ', ':', ',', '/'], "_");
        dump(
            &Path::new(&dir).join(format!("{tag}-thumb.ppm")),
            tw,
            tt,
            &t,
        );
        dump(
            &Path::new(&dir).join(format!("{tag}-thumb-export.ppm")),
            tw,
            tt,
            &reduced_t,
        );
    }

    Row {
        input: p.kind,
        case: case.name,
        scale: case.scale,
        one,
        tiles,
        fit,
        thumb,
    }
}

/// 1:1 tiles (f16) against the f32 export.
const ONE_P99: f64 = 0.5;
/// Fit view and thumbnail against the reduced export, scale-stable modules.
const FIT_P95: f64 = 1.0;

fn check(r: &Row) -> Vec<String> {
    let mut bad = Vec::new();
    let what = format!("{} / {}", r.input.name(), r.case);
    if r.one.p99 >= ONE_P99 {
        bad.push(format!("{what}: 1:1 p99 {:.3}", r.one.p99));
    }
    // Reduced raw views demosaic at full size and reduce (no binned mosaic), so
    // every input is held to the same bar.
    if r.scale == Scale::Stable {
        if r.fit.p95 >= FIT_P95 {
            bad.push(format!("{what}: fit p95 {:.3}", r.fit.p95));
        }
        if r.thumb.p95 >= FIT_P95 {
            bad.push(format!("{what}: thumbnail p95 {:.3}", r.thumb.p95));
        }
    }
    bad
}

fn table_row(r: &Row) -> String {
    format!(
        "| {} | {} | {} | {} | {} | {} | {} |",
        r.case,
        r.input.name(),
        r.one.cells(),
        r.tiles,
        r.fit.cells(),
        r.thumb.short(),
        match r.scale {
            Scale::Stable => "stable",
            Scale::Detail => "detail",
        }
    )
}

fn sweep(kinds: &[Kind], size: (u32, u32), only: Option<&[&str]>, strict: bool) {
    let Some(d) = dev() else { return };
    let _g = lock();
    let dir = tmpdir();
    let cases = cases();
    let mut bad = Vec::new();
    println!(
        "| Module or stack | Input | 1:1 tiles p50 / p95 / p99 / max | tiles | Fit view p50 / p95 / p99 / max | Thumbnail p95 / max | Scale |"
    );
    println!("|---|---|---|---|---|---|---|");
    for &k in kinds {
        // X-Trans rows and columns are a multiple of 6.
        let (w, h) = if k == Kind::XTrans {
            (size.0 / 6 * 6, size.1 / 6 * 6)
        } else {
            size
        };
        let p = photo(k, w, h, &dir);
        for c in &cases {
            if (c.raw_only && !k.raw()) || only.is_some_and(|o| !o.contains(&c.name)) {
                continue;
            }
            // `SPECULAR_SWEEP_FILTER=X-Trans/default`: only matching rows (looking at one).
            if let Ok(f) = std::env::var("SPECULAR_SWEEP_FILTER")
                && !format!("{}/{}", k.name(), c.name).contains(&f)
            {
                continue;
            }
            let t0 = std::time::Instant::now();
            let r = run(d, &p, c, &dir);
            println!("{}", table_row(&r));
            eprintln!("  ({} ms)", t0.elapsed().as_millis());
            bad.extend(check(&r));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    // The full table holds 1:1 = export everywhere; its fit-view and thumbnail rows over
    // the bar are filed with their reasons in docs/develop-quality.md (`strict` false).
    let shown: Vec<&String> = bad.iter().filter(|b| strict || b.contains("1:1")).collect();
    if !strict && shown.len() < bad.len() {
        println!(
            "Over the fit-view or thumbnail bar (filed):\n{}",
            bad.join("\n")
        );
    }
    assert!(
        shown.is_empty(),
        "over the tolerances:\n{}",
        shown
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A representative subset, every input.
#[test]
fn preview_matches_export_for_a_subset_of_modules() {
    sweep(
        &[Kind::Bayer, Kind::XTrans, Kind::Jpeg, Kind::Tiff],
        (720, 480),
        Some(&[
            "default",
            "crop and straighten",
            "tone equalizer",
            "colour balance and wheels",
            "retouch: heal, clone, blur",
            "glow: bloom",
            "grain",
            "stack: landscape",
        ]),
        true,
    );
}

/// The whole table of `docs/develop-quality.md`.
#[test]
#[ignore = "the full sweep: a report, run by hand"]
fn quality_sweep_table() {
    sweep(
        &[Kind::Bayer, Kind::XTrans, Kind::Jpeg, Kind::Tiff],
        (1800, 1200),
        None,
        false,
    );
}
