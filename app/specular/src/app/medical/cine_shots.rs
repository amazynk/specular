//! Screenshot test of DICOM video: a colour H.264 Video Endoscopic object (synthetic, made
//! here with the in-tree encoder) decoded by the cine path, its fifth frame drawn by the image
//! renderer under the chrome with the transport bar's readout (frame 5 of 9, playing), left to
//! right in English and in Hebrew (the transport keeps its order; its text is translated).
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular medical::cine_shots`, then review
//! and commit the rewritten PNGs.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, Source};
use specular_h264enc as enc;
use specular_input::{ActionState, ImageState, Keymap};
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_settings::SettingsStore;
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;
use specular_widgets::{Chrome, HudStats, Overlays, Playback, ViewerState};

use crate::app::Settings;

const SIZE: (f32, f32) = (480.0, 300.0);
const W: u32 = 176;
const H: u32 = 112;
const FRAMES: u32 = 9;

/// BT.709 limited-range Y, Cb, Cr of an RGB colour.
fn ycbcr(rgb: [f32; 3]) -> [u8; 3] {
    let [r, g, b] = rgb.map(|v| v / 255.0);
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let cb = (b - y) / 1.8556;
    let cr = (r - y) / 1.5748;
    [
        (16.0 + 219.0 * y).round() as u8,
        (128.0 + 224.0 * cb).round() as u8,
        (128.0 + 224.0 * cr).round() as u8,
    ]
}

/// Frame `t`: seven colour bars with a white disc moving across them.
fn picture(t: u32) -> [Vec<u8>; 3] {
    const BARS: [[f32; 3]; 7] = [
        [190.0, 190.0, 190.0],
        [190.0, 190.0, 0.0],
        [0.0, 190.0, 190.0],
        [0.0, 190.0, 0.0],
        [190.0, 0.0, 190.0],
        [190.0, 0.0, 0.0],
        [0.0, 0.0, 190.0],
    ];
    let (cx, cy) = (24.0 + 16.0 * t as f32, 56.0);
    let mut y = vec![0u8; (W * H) as usize];
    let (cw, ch) = (W / 2, H / 2);
    let mut u = vec![0u8; (cw * ch) as usize];
    let mut v = vec![0u8; (cw * ch) as usize];
    let at = |x: u32, yy: u32| {
        let (fx, fy) = (x as f32 + 0.5, yy as f32 + 0.5);
        if (fx - cx).powi(2) + (fy - cy).powi(2) < 18.0 * 18.0 {
            [235.0, 235.0, 235.0]
        } else {
            BARS[(x * 7 / W) as usize]
        }
    };
    for r in 0..H {
        for c in 0..W {
            y[(r * W + c) as usize] = ycbcr(at(c, r))[0];
        }
    }
    for r in 0..ch {
        for c in 0..cw {
            let [_, b, rr] = ycbcr(at(2 * c, 2 * r));
            u[(r * cw + c) as usize] = b;
            v[(r * cw + c) as usize] = rr;
        }
    }
    [y, u, v]
}

fn video() -> Vec<u8> {
    let mut e = enc::Encoder::new(enc::Config::new(W, H, 25, 1, 14)).unwrap();
    let packets: Vec<_> = (0..FRAMES)
        .map(|t| {
            let [y, u, v] = picture(t);
            let f = enc::Frame {
                y: &y,
                u: &u,
                v: &v,
                y_stride: W as usize,
                c_stride: W as usize / 2,
            };
            e.encode(&f, false)
        })
        .collect();
    let stream = enc::annex_b(&e.sps(), &e.pps(), &packets);
    specular_dicom::fixture::video(
        specular_dicom::fixture::H264_HIGH_41,
        &stream,
        0,
        W as u16,
        H as u16,
        Some(FRAMES),
        40.0,
        8,
    )
}

#[test]
fn dicom_video_cine_frame_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-dicom-video",
    );
    let s = specular_decode::Registry::default()
        .open(Arc::new(Source::from_vec("endoscopy.dcm", video())))
        .expect("DICOM video opens");
    assert_eq!(s.page_count(), FRAMES);
    assert!(s.animation().is_some());
    assert!(s.info().coding.contains("H.264"), "{}", s.info().coding);
    let mut frames = s.frames(1).expect("cine");
    let cancel = CancelToken::new();
    let frame = (0..5)
        .map(|_| {
            frames
                .next_frame(&cancel)
                .expect("a frame")
                .expect("decodes")
        })
        .last()
        .unwrap();
    assert_eq!(frame.index, 4);
    assert_eq!(frame.delay.as_millis(), 40);

    let settings = Settings::default();
    let store = SettingsStore::in_memory();
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, W, H, None);
    renderer.upload_overview(&h.gpu, &mut img, &frame.raster);
    let st = ViewerState {
        actions: ActionState {
            image: ImageState::Ready,
            count: 1,
            animated: true,
            video: false,
            sound: false,
            sound_file: false,
            dicom: true,
            ..ActionState::default()
        },
        has_image: true,
        index: 0,
        count: 1,
        zoom: 2.0,
        at_fit: true,
        fullscreen: false,
        file_name: Some("endoscopy.dcm"),
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
        playback: Some(Playback {
            frame: frame.index as usize,
            count: Some(FRAMES),
            playing: true,
            speed: 1.0,
            video: None,
        }),
        motion: None,
        spread: None,
        pages: None,
    };
    let pal = Palette::dark();
    for (name, he, scale) in [
        ("dicom-video-cine", false, 1.0),
        ("dicom-video-cine", false, 1.5),
        ("dicom-video-cine-he", true, 1.0),
    ] {
        let lang = if he {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        h.text
            .set_locale(Some(if he { "he" } else { "en" }.to_owned()));
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let mut c = Chrome::new();
            c.reduce_motion = true;
            c.set_size(SIZE.0, SIZE.1);
            c.poke(1.0);
            h.render_gpu(
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
                        size.0 as f32 / W as f32 * 0.8,
                        (W as f32 / 2.0, H as f32 / 2.0),
                    );
                    let opts = RenderOptions::default();
                    renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
                },
                |dl, text| {
                    let mut warm = specular_gfx::DrawList::new(scale);
                    c.draw(&mut warm, text, &st, &pal, 1.0);
                    c.draw(dl, text, &st, &pal, 1.0);
                },
            )
        });
        goldens.check(&format!("{name}-{scale}x"), &shot);
    }
    goldens.finish();
}
