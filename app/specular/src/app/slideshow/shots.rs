//! Screenshot tests of the slideshow at 1× and 1.5×: the caption (file name and EXIF line) in
//! left-to-right and right-to-left (with the pause badge), and the cross-fade's midpoint
//! between two slides. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`; see `specular-shots` for the tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular slideshow::shots`, then review and
//! commit the rewritten PNGs.
//!
//! The slides are synthetic: flat "photos" (sky, ground and a sun) drawn with the UI shader.

use super::*;
use specular_shots::{Goldens, Harness};

/// Logical window size, as for the chrome's shots.
const SIZE: (f32, f32) = (720.0, 450.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// A 3:2 slide fitted into the window on black, with a sun at `sun` (0–1 across).
fn photo(dl: &mut DrawList, sky: Color, ground: Color, sun: f32) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, Color::BLACK);
    let h = SIZE.1;
    let w = h * 1.5;
    let x = (SIZE.0 - w) * 0.5;
    dl.rect(Rect::new(x, 0.0, w, h * 0.62), 0.0, sky);
    dl.rect(Rect::new(x, h * 0.62, w, h * 0.38), 0.0, ground);
    let r = 46.0;
    dl.rect(
        Rect::new(x + w * sun - r, h * 0.3 - r, 2.0 * r, 2.0 * r),
        r,
        Color::rgba(1.0, 0.86, 0.45, 1.0),
    );
}

fn beach(dl: &mut DrawList) {
    photo(
        dl,
        Color::rgba(0.42, 0.66, 0.9, 1.0),
        Color::rgba(0.93, 0.83, 0.6, 1.0),
        0.72,
    );
}

fn dusk(dl: &mut DrawList) {
    photo(
        dl,
        Color::rgba(0.45, 0.25, 0.5, 1.0),
        Color::rgba(0.12, 0.2, 0.16, 1.0),
        0.25,
    );
}

fn area() -> (f32, f32, f32, f32) {
    (0.0, 0.0, SIZE.0, SIZE.1)
}

#[test]
fn slideshow_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-slideshow",
    );
    let exif = "12 March 2026, 17:42 · Fujifilm X-T5 · 1/250 s · f/8 · ISO 100";
    let ltr = vec!["beach-04.jpg".to_owned(), exif.to_owned()];
    let rtl = vec![
        "חוף-04.jpg".to_owned(),
        "12 במרץ 2026, 17:42 · Fujifilm X-T5 · 1/250 s · f/8 · ISO 100".to_owned(),
    ];
    for (scale, suffix) in SCALES {
        let shot = h.render(SIZE, scale, beach, |dl, text| {
            draw_overlay(dl, text, &ltr, false, area(), false, 1.0);
        });
        goldens.check(&format!("slideshow-caption-ltr-{suffix}"), &shot);
        let shot = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            h.render(SIZE, scale, beach, |dl, text| {
                draw_overlay(dl, text, &rtl, true, area(), true, 1.0);
            })
        });
        goldens.check(&format!("slideshow-caption-rtl-{suffix}"), &shot);
        // The midpoint: half the outgoing slide, half the incoming one (in linear light), the
        // incoming slide's caption over both.
        let names = vec!["dusk-05.jpg".to_owned()];
        let shot = h.render_crossfade(SIZE, scale, beach, dusk, fade_weight(0.5), |dl, text| {
            draw_overlay(dl, text, &names, false, area(), false, 1.0);
        });
        goldens.check(&format!("slideshow-crossfade-{suffix}"), &shot);
    }
    goldens.finish();
}

/// The fade's ends are the two slides themselves.
#[test]
fn crossfade_ends_are_the_slides() {
    let Some(mut h) = Harness::new() else { return };
    let chrome = |_: &mut DrawList, _: &mut Text| {};
    let from = h.render(SIZE, 1.0, beach, chrome);
    let to = h.render(SIZE, 1.0, dusk, chrome);
    let start = h.render_crossfade(SIZE, 1.0, beach, dusk, fade_weight(0.0), chrome);
    let end = h.render_crossfade(SIZE, 1.0, beach, dusk, fade_weight(1.0), chrome);
    assert_eq!(specular_shots::delta(&from, &start).differing, 0);
    assert_eq!(specular_shots::delta(&to, &end).differing, 0);
    let mid = h.render_crossfade(SIZE, 1.0, beach, dusk, 0.5, chrome);
    assert!(specular_shots::delta(&from, &mid).differing > 0);
}

/// The outgoing slide's snapshot is held only while its fade runs.
#[test]
fn crossfade_snapshot_is_released_when_the_fade_ends() {
    let Some(h) = Harness::new() else { return };
    let mut show = Slideshow {
        fade: Some(Fade { to: 2, start: None }),
        ..Slideshow::default()
    };
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    show.before_scene(&h.gpu, &mut enc, &h.comp, Some(1), Some(2), true, 10.0);
    let held = |s: &Slideshow| s.crossfade.as_ref().is_some_and(Crossfade::holds_snapshot);
    assert!(held(&show), "the outgoing slide is kept");
    assert!(show.draw_fade(&mut enc, &h.comp, 10.0 + FADE_S * 0.5));
    assert!(held(&show), "while the fade runs");
    assert!(!show.draw_fade(&mut enc, &h.comp, 10.0 + FADE_S));
    assert!(!held(&show), "freed when it ends");
    h.gpu.queue.submit([enc.finish()]);
    // A fade replaced by a step without one frees it too.
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    show.fade = Some(Fade { to: 3, start: None });
    show.before_scene(&h.gpu, &mut enc, &h.comp, Some(2), Some(3), true, 20.0);
    assert!(held(&show));
    show.fade = None;
    assert!(!show.draw_fade(&mut enc, &h.comp, 20.1));
    assert!(!held(&show));
    h.gpu.queue.submit([enc.finish()]);
}
