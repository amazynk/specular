//! Screenshot test of the colour-vision-safe display aids on a real GPU image (DESIGN.md
//! §6.2): a synthetic 12-bit greyscale scan auto-ranged on upload, shown in false colour
//! (viridis) with HiLo marking the pixels at 4095 and at 0, under the chrome's colour bar and
//! badges. The same chrome right to left is in the widget goldens (`colour-bar-he-*`; the UI
//! language is process-wide, and this binary's tests run in parallel).
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular inspect::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_input::{ActionState, ImageState, Keymap};
use specular_render::ImageRenderer;
use specular_settings::SettingsStore;
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::{Chrome, HudStats, Overlays, ViewerState};

use super::*;

const SIZE: (f32, f32) = (480.0, 300.0);
const W: u32 = 160;
const H: u32 = 100;

/// A 12-bit scan widened to 16 bits as the TIFF decoder does: a ramp from 300 to 3800, a
/// saturated block at 4095 (top right) and a black one at 0 (bottom left).
fn scan() -> Raster {
    let widen = |v: u32| (v * 65535 / 4095) as u16;
    let mut data = Vec::with_capacity((W * H * 2) as usize);
    for y in 0..H {
        for x in 0..W {
            let v = if x >= 120 && y < 30 {
                4095
            } else if x < 40 && y >= 70 {
                0
            } else {
                300 + 3500 * x / (W - 1)
            };
            data.extend_from_slice(&widen(v).to_le_bytes());
        }
    }
    Raster {
        width: W,
        height: H,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Gray16,
        planes: vec![RasterPlane {
            width: W,
            height: H,
            stride: W as usize * 2,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

#[test]
fn hilo_and_false_colour_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-display-aids",
    );
    let settings = Settings::default();
    let store = SettingsStore::in_memory();
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, W, H, None);
    renderer.set_auto_range(&h.gpu, &mut img, Some(12));
    renderer.upload_overview(&h.gpu, &mut img, &scan());
    let _ = h
        .gpu
        .device
        .poll(specular_gfx::wgpu::PollType::wait_indefinitely());
    assert_eq!(img.auto_range(), Some([0, 65535]));
    let st = ViewerState {
        actions: ActionState {
            image: ImageState::Ready,
            count: 1,
            animated: false,
            video: false,
            sound: false,
            sound_file: false,
            in_archive: false,
            has_alpha: false,
            gallery_views: None,
            medical: None,
            dicom: false,
            series: 0,
            presentation_states: 0,
            pages: 0,
            ..ActionState::default()
        },
        has_image: true,
        index: 0,
        count: 1,
        zoom: 2.0,
        at_fit: true,
        fullscreen: false,
        file_name: Some("scan-12bit.tif"),
        dims: Some((W, H)),
        loading: false,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: Overlays::default(),
        playback: None,
        motion: None,
        spread: None,
        pages: None,
    };
    let pal = Palette::dark();
    for (name, false_colour, scale) in [("hilo-false-colour", true, 1.0), ("hilo-grey", false, 1.5)]
    {
        let mut c = Chrome::new();
        c.reduce_motion = true;
        c.set_size(SIZE.0, SIZE.1);
        c.poke(1.0);
        if false_colour {
            assert!(c.handle_action(Action::ToggleFalseColour));
        }
        assert!(c.handle_action(Action::ToggleHiLo));
        // The legend `update_legend` sets for a 12-bit image under auto-range.
        c.inspect.legend = Some(specular_inspect::Legend {
            map: specular_inspect::ColourMap::Viridis,
            lo: 0.0,
            hi: 4095.0,
            unit: String::new(),
        });
        let mut opts = RenderOptions::default();
        apply_display_aids(&mut opts, &c, &settings);
        assert!(opts.hilo.is_some());
        let shot = h.render_gpu(
            SIZE,
            scale,
            |gpu, enc, target, size| {
                let view = View::fixed(
                    (W, H),
                    Orientation::NORMAL,
                    PxRect {
                        x: 0.0,
                        y: 0.0,
                        w: size.0 as f32,
                        h: size.1 as f32,
                    },
                    size.1 as f32 / H as f32 * 0.8,
                    (W as f32 / 2.0, H as f32 / 2.0),
                );
                renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
            },
            |dl, text| {
                let mut warm = specular_gfx::DrawList::new(scale);
                c.draw(&mut warm, text, &st, &pal, 1.0);
                c.draw(dl, text, &st, &pal, 1.0);
            },
        );
        goldens.check(&format!("{name}-{scale}x"), &shot);
    }
    goldens.finish();
}
