//! DNG `DefaultScale` (non-square pixels) in Develop: the preview and the export show the
//! picture with square pixels, as the viewer does, and crops are fractions of that shape.
//!
//!     SPECULAR_REQUIRE_GPU=1 cargo test -p specular-develop --test default_scale

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::sync::OnceLock;

use common::{DngCodec, DngSpec, Tag, write_dng};
use specular_convert::Resize;
use specular_develop::export::batch;
use specular_develop::export::{Depth, Exporter, FileFormat, Preset, Samples, Space};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::params::CropOrientation;
use specular_develop::model::{Edit, ModuleKind, Params, Registry, SourceKey};
use specular_develop::ops::geometry::{Context, Plan};
use specular_develop::ops::output::OutputSpace;
use specular_develop::preview::{self, Developer, Input, Orient};

/// Stored size; `DefaultScale` 1 × 2 makes it 400 × 300 with square pixels.
const W: u32 = 400;
const H: u32 = 150;
/// The disc's radius in square pixels.
const R: f64 = 100.0;

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

/// RATIONAL values over 1000.
fn tag_rationals(tag: u16, v: &[f64]) -> Tag {
    let d = v
        .iter()
        .flat_map(|x| {
            [
                ((x * 1000.0).round() as u32).to_le_bytes(),
                1000u32.to_le_bytes(),
            ]
        })
        .flatten()
        .collect();
    (tag, 5, v.len() as u32, d)
}

/// A LinearRaw DNG of a bright disc (round with square pixels: half as tall as wide in the
/// stored grid) on a dark ground, `DefaultScale` 1 × 2.
fn disc_dng() -> Vec<u8> {
    let mut s = DngSpec::new(W, H, DngCodec::U16);
    s.linear_raw = true;
    s.extra.push(tag_rationals(0xc61e, &[1.0, 2.0]));
    let (cx, cy) = (f64::from(W) / 2.0, f64::from(H) / 2.0);
    let mut data = Vec::with_capacity((W * H * 3) as usize);
    for y in 0..H {
        for x in 0..W {
            let dx = (f64::from(x) + 0.5 - cx) / R;
            let dy = (f64::from(y) + 0.5 - cy) * 2.0 / R;
            let v: u16 = if dx * dx + dy * dy <= 1.0 {
                30000
            } else {
                2000
            };
            data.extend([v; 3]);
        }
    }
    write_dng(&s, &data)
}

fn edit_for(input: &Input, crop: Option<CropOrientation>) -> Edit {
    let mut e = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    if let Input::Raw(s) = input
        && let Some(xy) = preview::as_shot_white(s)
    {
        batch::set_as_shot(&mut e, xy);
    }
    if let Some(c) = crop {
        e.module_mut(ModuleKind::CROP_ORIENTATION, 0)
            .expect("crop module")
            .params = Params::CropOrientation(c);
    }
    e
}

/// The bright disc's bounding box (x0, y0, x1, y1, exclusive) in a `w × h` picture whose
/// pixel `i` has brightness `lum(i)`; the threshold is halfway between the two levels.
fn disc_box(w: u32, h: u32, lum: impl Fn(usize) -> f64) -> (u32, u32, u32, u32) {
    let v: Vec<f64> = (0..(w * h) as usize).map(lum).collect();
    let lo = v.iter().copied().fold(f64::MAX, f64::min);
    let hi = v.iter().copied().fold(f64::MIN, f64::max);
    let t = (lo + hi) / 2.0;
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if v[(y * w + x) as usize] > t {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x0, y0, x1, y1)
}

fn assert_round(b: (u32, u32, u32, u32), what: &str) {
    let (w, h) = (b.2 - b.0, b.3 - b.1);
    let d = 2.0 * R;
    assert!(
        (f64::from(w) - d).abs() <= 3.0 && (f64::from(h) - d).abs() <= 3.0,
        "{what}: the disc is {w} × {h}, not round ({d})"
    );
}

/// The developed preview of `edit` at its full size: (width, height, linear RGBA).
fn preview(d: &Dev, file: &[u8], edit: &Edit) -> (u32, u32, Vec<f32>) {
    let mut dv = Developer::new(&d.device, &d.queue, Budget::fixed(1 << 28));
    dv.set_input(preview::decode_raw(file).unwrap()).unwrap();
    dv.prepare(edit, W).unwrap();
    let r = dv.render(edit, None, &OutputSpace::rec709()).unwrap();
    let (w, h) = (r.roi.w, r.roi.h);
    let px = dv.read(&r).unwrap();
    dv.recycle(r);
    (w, h, px)
}

/// The export of `edit`, 16-bit sRGB with its long side `long`.
fn export(d: &Dev, file: &[u8], edit: &Edit, long: u32) -> (u32, u32, Vec<u16>) {
    let mut ex = Exporter::new(&d.device, &d.queue, Budget::fixed(1 << 28));
    let preset = Preset {
        format: FileFormat::Png,
        depth: Depth::Sixteen,
        space: Space::Srgb,
        size: Resize::LongEdge(long),
        ..Preset::default()
    };
    let img = ex
        .render(
            preview::decode_raw(file).unwrap(),
            edit,
            Orient::default(),
            &preset,
            &mut |_| true,
        )
        .unwrap();
    let Samples::Sixteen(px) = img.samples else {
        panic!("16-bit")
    };
    (img.width, img.height, px)
}

#[test]
fn the_plan_works_in_square_pixels() {
    let file = disc_dng();
    let Input::Raw(s) = preview::decode_raw(&file).unwrap() else {
        panic!("raw")
    };
    let ctx = Context::raw(&s, Orient::default());
    assert_eq!(ctx.stretch, [1.0, 2.0]);
    let e = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    let plan = Plan::new(&e, &ctx, true);
    assert!(!plan.is_identity() && !plan.orientation_only());
    assert_eq!(plan.output_size(1.0), (W, 2 * H));
    // The disc's top in the stored grid is its top on screen, twice as far down.
    let top = (f64::from(W) / 2.0, f64::from(H) / 2.0 - R / 2.0);
    let o = plan.to_output(top, 1.0).expect("maps");
    assert!(
        (o.0 - 200.0).abs() < 1e-6 && (o.1 - 50.0).abs() < 1e-6,
        "{o:?}"
    );
    let (back, _) = plan.to_source(1, o, 1.0);
    assert!((back.0 - top.0).abs() < 1e-6 && (back.1 - top.1).abs() < 1e-6);
    // A quarter turn: the shown frame is 300 × 400.
    let mut c = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    c.module_mut(ModuleKind::CROP_ORIENTATION, 0)
        .unwrap()
        .params = Params::CropOrientation(CropOrientation {
        quarter_turns: 1,
        ..Default::default()
    });
    assert_eq!(Plan::new(&c, &ctx, true).output_size(1.0), (2 * H, W));
}

#[test]
fn develop_shows_square_pixels_as_the_viewer_does() {
    let file = disc_dng();
    // The viewer: the decode stretched to square pixels.
    let f = specular_raw::parse(&file).unwrap();
    assert_eq!(f.info().sensor.as_ref().unwrap().square_size, (W, 2 * H));
    let v = f
        .decode(&file, &specular_raw::DecodeOptions::default())
        .unwrap();
    assert_eq!((v.width, v.height), (W, 2 * H));
    let viewer = disc_box(v.width, v.height, |i| f64::from(v.data[i * 3 + 1]));
    assert_round(viewer, "viewer");

    let Some(d) = dev() else { return };
    let _g = lock();
    let input = preview::decode_raw(&file).unwrap();
    let edit = edit_for(&input, None);

    let (pw, ph, px) = preview(d, &file, &edit);
    assert_eq!((pw, ph), (W, 2 * H), "preview size");
    let shown = disc_box(pw, ph, |i| f64::from(px[i * 4 + 1]));
    assert_round(shown, "preview");

    let (ew, eh, ex) = export(d, &file, &edit, W);
    assert_eq!((ew, eh), (W, 2 * H), "export size");
    let exported = disc_box(ew, eh, |i| f64::from(ex[i * 3 + 1]));
    assert_round(exported, "export");
    // All three put the disc in the same place (within the filters' half pixel).
    for b in [shown, exported] {
        let d = [
            b.0.abs_diff(viewer.0),
            b.1.abs_diff(viewer.1),
            b.2.abs_diff(viewer.2),
            b.3.abs_diff(viewer.3),
        ];
        assert!(
            d.iter().all(|&e| e <= 1),
            "{b:?} vs the viewer's {viewer:?}"
        );
    }
}

#[test]
fn crops_are_fractions_of_the_shown_picture() {
    let Some(d) = dev() else { return };
    let _g = lock();
    let file = disc_dng();
    let input = preview::decode_raw(&file).unwrap();
    // Turned a quarter (300 × 400 shown), the middle 80 %: 240 × 320.
    let crop = CropOrientation {
        quarter_turns: 1,
        crop: [0.1, 0.1, 0.9, 0.9],
        ..Default::default()
    };
    let edit = edit_for(&input, Some(crop));
    let (pw, ph, px) = preview(d, &file, &edit);
    assert_eq!((pw, ph), (240, 320), "preview size");
    let b = disc_box(pw, ph, |i| f64::from(px[i * 4 + 1]));
    assert_round(b, "cropped preview");
    // Centred: 20 px either side across, 60 px above and below.
    assert!(b.0.abs_diff(20) <= 1 && b.1.abs_diff(60) <= 1, "{b:?}");

    let (ew, eh, ex) = export(d, &file, &edit, 320);
    assert_eq!((ew, eh), (240, 320), "export size");
    let e = disc_box(ew, eh, |i| f64::from(ex[i * 3 + 1]));
    assert_round(e, "cropped export");
    assert!(
        e.0.abs_diff(b.0) <= 1 && e.1.abs_diff(b.1) <= 1,
        "{e:?} vs {b:?}"
    );
}
