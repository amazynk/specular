//! Screenshot tests of subtitles over a movie's frame at 1× and 1.5×: the default look,
//! moved up (a cue the file puts at the top stays), large yellow text on a box, and another
//! font (what it lacks falls back), in English over a dark frame and in Hebrew over a light
//! one; and roll-up closed captions on their grid rows. Rendered offscreen on lavapipe and
//! compared with the PNGs in `app/specular/tests/shots/`; see `specular-shots` for the
//! tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular player::shots`, then review and
//! commit the rewritten PNGs.

use specular_gfx::{DrawList, Rect};
use specular_settings::{Settings, SubtitleBackground, SubtitleColor};
use specular_shots::{Goldens, Harness};
use specular_subs::{Body, Cue};
use specular_theme::Color;

use super::subs;

/// Logical window size; the 16:9 frame fills its width.
const SIZE: (f32, f32) = (720.0, 450.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn pic() -> Rect {
    let h = SIZE.0 * 9.0 / 16.0;
    Rect::new(0.0, (SIZE.1 - h) * 0.5, SIZE.0, h)
}

/// A synthetic frame: sky, ground and a sun; dark (a night street) or light (a beach).
fn frame(dl: &mut DrawList, light: bool) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, Color::BLACK);
    let p = pic();
    let (sky, ground, sun) = if light {
        (
            Color::rgba(0.62, 0.8, 0.96, 1.0),
            Color::rgba(0.96, 0.9, 0.72, 1.0),
            Color::rgba(1.0, 0.97, 0.85, 1.0),
        )
    } else {
        (
            Color::rgba(0.08, 0.1, 0.2, 1.0),
            Color::rgba(0.16, 0.14, 0.13, 1.0),
            Color::rgba(0.95, 0.9, 0.7, 1.0),
        )
    };
    dl.rect(Rect::new(p.x, p.y, p.w, p.h * 0.6), 0.0, sky);
    dl.rect(Rect::new(p.x, p.y + p.h * 0.6, p.w, p.h * 0.4), 0.0, ground);
    let r = 34.0;
    dl.rect(
        Rect::new(p.x + p.w * 0.75 - r, p.y + p.h * 0.28 - r, 2.0 * r, 2.0 * r),
        r,
        sun,
    );
}

fn cue(text: &str) -> Cue {
    Cue {
        start_us: 0,
        end_us: 1_000_000,
        body: Body::Text(specular_subs::markup::parse(text)),
    }
}

/// A cue the file puts at the top (ASS `\an8`).
fn top(text: &str) -> Cue {
    let mut c = cue(text);
    if let Body::Text(t) = &mut c.body {
        t.align = 8;
    }
    c
}

#[test]
fn subtitles_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-subtitles",
    );
    // A font of few letters (A B a o 1 2): the rest falls back to the interface's fonts.
    let serif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/formats/specular-font/fixtures/serif.otf"
    ))
    .expect("serif fixture");
    let family = h.text.add_font(serif).pop().expect("a family");

    let mut moved = Settings::default();
    moved.viewing.subtitle_position = 30;
    let mut yellow = Settings::default();
    yellow.viewing.subtitle_size = 160;
    yellow.viewing.subtitle_color = SubtitleColor::Yellow;
    yellow.viewing.subtitle_background = SubtitleBackground::Box;
    let mut font = Settings::default();
    font.viewing.subtitle_font = family;
    let looks = [
        ("default", Settings::default()),
        ("moved-up", moved),
        ("large-yellow-box", yellow),
        ("font", font),
    ];

    let en = [
        cue("Bob, a boa? 12 o'clock.\n<i>Meet me by the old road.</i>"),
        top("ABBA 21"),
    ];
    let he = [
        cue("שלום Bob! הסרט מתחיל ב־12.\n<i>נתראה שם, Ana</i>"),
        top("ABBA 21"),
    ];
    for (scale, suffix) in SCALES {
        for (name, settings) in &looks {
            let look = subs::look(settings);
            let p = pic();
            // The cue the file puts at the top shows in the moved-up case only, to show it stays.
            let n = if *name == "moved-up" { 2 } else { 1 };
            let shot = h.render(
                SIZE,
                scale,
                |dl| frame(dl, false),
                |dl, text| draw_text_cues(dl, text, &en[..n], p, &look),
            );
            goldens.check(&format!("subtitles-{name}-en-dark-{suffix}"), &shot);
            let shot =
                specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
                    h.render(
                        SIZE,
                        scale,
                        |dl| frame(dl, true),
                        |dl, text| draw_text_cues(dl, text, &he[..n], p, &look),
                    )
                });
            goldens.check(&format!("subtitles-{name}-he-light-{suffix}"), &shot);
        }
    }
    // Roll-up closed captions (CEA-608 CC1, three rows, the last with a red word), decoded
    // from line 21 bytes; the same in a right-to-left interface.
    let captions = rollup_captions();
    let look = subs::look(&Settings::default());
    for (scale, suffix) in SCALES {
        let p = pic();
        let shot = h.render(
            SIZE,
            scale,
            |dl| frame(dl, false),
            |dl, text| draw_text_cues(dl, text, &captions, p, &look),
        );
        goldens.check(
            &format!("subtitles-captions-rollup-en-dark-{suffix}"),
            &shot,
        );
        let shot = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
            h.render(
                SIZE,
                scale,
                |dl| frame(dl, true),
                |dl, text| draw_text_cues(dl, text, &captions, p, &look),
            )
        });
        goldens.check(
            &format!("subtitles-captions-rollup-he-light-{suffix}"),
            &shot,
        );
    }
    goldens.finish();
}

/// The screen of a roll-up caption stream after its third row.
fn rollup_captions() -> Vec<Cue> {
    use specular_subs::cea608::fixture::{ctrl, frames, pac, text};
    let pairs = [
        ctrl(0x14, 0x26),
        pac(15, 0, false),
        text(">> NEWS AT NINE."),
        ctrl(0x14, 0x2D),
        text("STORMS MOVE EAST"),
        ctrl(0x14, 0x2D),
        text("TONIGHT,"),
        // Mid-row: red (shown as a space), then a music note.
        ctrl(0x11, 0x28),
        text("FLOODS "),
        ctrl(0x11, 0x37),
    ]
    .concat();
    let f = frames(0, &pairs, 0, 33_367);
    let t = specular_subs::cea608::decode(1, f.iter().map(|(t, d)| (*t, d.as_slice())));
    let last = t.cues.last().map_or(0, |c| c.start_us);
    t.active(last).cloned().collect()
}

/// The text cues as the player draws them (bitmap cues need textures; their placement has
/// unit tests).
fn draw_text_cues(
    dl: &mut DrawList,
    text: &mut specular_gfx::Text,
    cues: &[Cue],
    pic: Rect,
    look: &specular_widgets::subtitles::Look,
) {
    let mut stack = specular_widgets::subtitles::Stack::new(pic, look.position);
    for c in cues {
        if let Body::Text(t) = &c.body {
            specular_widgets::subtitles::draw_text(dl, text, t, pic, look, &mut stack);
        }
    }
}
