//! Screenshot tests of what Local AI's faces add to the viewer: "Show faces" boxes
//! (named people, a face with closed eyes) on a synthetic photo, and cull mode's strip with
//! the culling flags; English dark and Hebrew light, at 1× and 1.5×.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular faces_shots`.

use specular_gfx::{DrawList, Rect};
use specular_i18n::t;
use specular_library::ai::{EyeFace, Scores};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};

use super::cull::{Strip, draw_strip};
use super::faces_view::{Shown, flags, placed};
use super::gallery::Mark;

const SIZE: (f32, f32) = (720.0, 450.0);

/// A 3:2 photo fitted into the window, with three pale "faces".
fn photo(dl: &mut DrawList, pal: &Palette) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas);
    let (x, w, h) = photo_rect();
    dl.rect(
        Rect::new(x, 0.0, w, h),
        0.0,
        Color::rgba(0.42, 0.56, 0.7, 1.0),
    );
    for f in faces() {
        let r = Rect::new(
            x + f.rect[0] * w,
            f.rect[1] * h,
            f.rect[2] * w,
            f.rect[3] * h,
        );
        dl.rect(r, r.w * 0.5, Color::rgba(0.93, 0.78, 0.68, 1.0));
    }
}

fn photo_rect() -> (f32, f32, f32) {
    let h = SIZE.1;
    let w = h * 1.5;
    ((SIZE.0 - w) * 0.5, w, h)
}

fn faces() -> Vec<Shown> {
    vec![
        Shown {
            rect: [0.15, 0.25, 0.14, 0.2],
            label: "Ana".into(),
            closed: false,
            roll: 0.0,
        },
        Shown {
            rect: [0.45, 0.2, 0.12, 0.18],
            label: "Person #12".into(),
            closed: true,
            roll: 0.0,
        },
        Shown {
            // A face lying on its side: its box turned with it.
            rect: [0.72, 0.3, 0.1, 0.15],
            label: "Bo".into(),
            closed: false,
            roll: std::f32::consts::FRAC_PI_2,
        },
    ]
}

fn scores() -> Scores {
    Scores {
        sharpness: Some(0.2),
        eyes_open: Some(0.1),
        exposure: Some(0.08),
        faces: vec![
            EyeFace {
                closed: 0.9,
                ..Default::default()
            },
            EyeFace::default(),
            EyeFace::default(),
        ],
        ..Default::default()
    }
}

fn strip(pal: Palette) -> Strip {
    Strip {
        title: "DSC_0412.NEF".into(),
        position: (12, 240),
        exposure: Some("1/60 s · f/2.8 · ISO 3200 · 35 mm".into()),
        flags: flags(&scores()),
        mark: Mark::default(),
        burst: None,
        flash: None,
        hover: None,
        tip: None,
        pal,
    }
}

#[test]
fn faces_and_cull_flags_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-faces",
    );
    for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
        for (he, pal) in [(false, Palette::dark()), (true, Palette::light())] {
            let lang = if he {
                specular_i18n::langid!("he")
            } else {
                specular_i18n::langid!("en-US")
            };
            let name = if he { "he-light" } else { "dark" };
            let (boxes, cull) = specular_i18n::I18n::with_thread_language(lang, || {
                let (x, w, ph) = photo_rect();
                let mut fs = faces();
                if he {
                    fs[0].label = "אנה".into();
                }
                let shapes = placed(&fs, (w, ph), scale, |d| ((d.0 + x) * scale, d.1 * scale));
                let boxes = h.render(
                    SIZE,
                    scale,
                    |dl| photo(dl, &pal),
                    |dl, text| {
                        super::script::draw(
                            dl,
                            text,
                            &shapes,
                            Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                            1.0,
                        );
                    },
                );
                let s = strip(pal.clone());
                let cull = h.render(
                    SIZE,
                    scale,
                    |dl| photo(dl, &pal),
                    |dl, text| {
                        draw_strip(dl, text, &s, (0.0, 0.0, SIZE.0, SIZE.1), he, 1.0);
                    },
                );
                (boxes, cull)
            });
            goldens.check(&format!("face-boxes-{name}-{suffix}"), &boxes);
            goldens.check(&format!("cull-flags-{name}-{suffix}"), &cull);
        }
    }
    goldens.finish();
}

#[test]
fn flags_are_words_of_the_language() {
    let f = flags(&scores());
    assert_eq!(f.len(), 3);
    assert_eq!(
        f[0],
        t!("cull-flag-eyes-closed-some", closed = 1, faces = 3)
    );
}
