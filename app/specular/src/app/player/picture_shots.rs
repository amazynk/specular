//! Screenshot tests of a movie's picture while it plays: a frame turned, zoomed in and
//! adjusted (brightness, contrast, saturation, gamma, hue) by the image renderer, with a
//! subtitle where the picture sits at fit (it stays put while zoomed). In English over a
//! dark frame turned right, and in Hebrew over a light frame mirrored, at 1× and 1.5×.
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular player::picture_shots`, then
//! review and commit the rewritten PNGs.

use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
use specular_gfx::Rect;
use specular_render::{ImageRenderer, PictureAdjust, PxRect, RenderOptions, View};
use specular_settings::Settings;
use specular_shots::{Goldens, Harness};

use super::{picture, subs};

/// Logical window size.
const SIZE: (f32, f32) = (480.0, 320.0);
/// The frame's stored size.
const FRAME: (u32, u32) = (320, 180);

/// A synthetic frame: sky, ground, a sun at the top right and an arrow pointing right, so a
/// turn or a mirror shows. Dark (a night street) or light (a beach).
fn frame(light: bool) -> Raster {
    let (w, h) = FRAME;
    let (sky, ground, sun, arrow): ([u8; 3], [u8; 3], [u8; 3], [u8; 3]) = if light {
        (
            [158, 204, 245],
            [245, 230, 184],
            [255, 247, 217],
            [200, 40, 50],
        )
    } else {
        ([20, 26, 51], [41, 36, 33], [242, 230, 179], [230, 120, 40])
    };
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let mut c = if y < h * 6 / 10 { sky } else { ground };
            let (dx, dy) = (fx - w as f32 * 0.78, fy - h as f32 * 0.25);
            if dx * dx + dy * dy < 22.0 * 22.0 {
                c = sun;
            }
            // The arrow: a shaft and a head pointing right, on the ground.
            let (ax, ay) = (fx - 60.0, fy - 140.0);
            let shaft = (0.0..90.0).contains(&ax) && ay.abs() < 5.0;
            let head = (90.0..120.0).contains(&ax) && ay.abs() < (120.0 - ax) * 0.7;
            if shaft || head {
                c = arrow;
            }
            data.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Raster {
        width: w,
        height: h,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: false,
    }
}

struct Case {
    name: &'static str,
    light: bool,
    he: bool,
    /// The user's turn (on top of the file's upright picture).
    turn: Orientation,
    /// Zoom over fit, and the stored point at the centre.
    zoom: f32,
    centre: (f32, f32),
    adjust: PictureAdjust,
    cue: &'static str,
}

fn shoot(h: &mut Harness, goldens: &mut Goldens, c: &Case, scale: f32, suffix: &str) {
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, FRAME.0, FRAME.1, None);
    renderer.upload_overview(&h.gpu, &mut img, &frame(c.light));
    let size_px = (
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let o = picture::apply(Orientation::NORMAL, c.turn);
    let (dw, dh) = o.display_size(FRAME.0, FRAME.1);
    let fit = (size_px.0 as f32 / dw as f32).min(size_px.1 as f32 / dh as f32);
    let view = View::fixed(
        FRAME,
        o,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: size_px.0 as f32,
            h: size_px.1 as f32,
        },
        fit * c.zoom,
        c.centre,
    );
    // The subtitle sits where the picture is at fit (logical px).
    let (x, y, w, hh) = picture::fit_rect(
        (dw as f32, dh as f32),
        (0.0, 0.0, SIZE.0, SIZE.1),
        fit / scale,
    );
    let pic = Rect::new(x, y, w, hh);
    let look = subs::look(&Settings::default());
    let cue = specular_subs::markup::parse(c.cue);
    let adjust = c.adjust;
    let mut render = |h: &mut Harness| {
        h.render_gpu(
            SIZE,
            scale,
            |gpu, enc, target, size| {
                let opts = RenderOptions {
                    picture: Some(adjust),
                    ..RenderOptions::default()
                };
                renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
            },
            |dl, text| {
                let mut stack = specular_widgets::subtitles::Stack::new(pic, look.position);
                specular_widgets::subtitles::draw_text(dl, text, &cue, pic, &look, &mut stack);
            },
        )
    };
    let shot = if c.he {
        specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || render(h))
    } else {
        render(h)
    };
    goldens.check(&format!("{}-{suffix}", c.name), &shot);
}

#[test]
fn turned_zoomed_adjusted_frames_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-video-picture",
    );
    let cases = [
        Case {
            name: "video-picture-turned-en-dark",
            light: false,
            he: false,
            turn: Orientation::NORMAL.rotated(1),
            zoom: 1.6,
            centre: (90.0, 120.0),
            adjust: PictureAdjust {
                brightness: 0.1,
                contrast: 1.2,
                saturation: 1.4,
                gamma: 1.1,
                hue: 0.0,
            },
            cue: "Bob, a boa? 12 o'clock.",
        },
        Case {
            name: "video-picture-mirrored-he-light",
            light: true,
            he: true,
            turn: Orientation::NORMAL.flipped_h(),
            zoom: 1.4,
            centre: (200.0, 100.0),
            adjust: PictureAdjust {
                brightness: -0.05,
                contrast: 1.1,
                saturation: 0.6,
                gamma: 0.9,
                hue: 40.0,
            },
            cue: "שלום Bob! הסרט מתחיל ב־12.",
        },
    ];
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        for c in &cases {
            shoot(&mut h, &mut goldens, c, scale, suffix);
        }
    }
    goldens.finish();
}
