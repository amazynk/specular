//! Screenshot tests of Develop's panel (sliders, a switch off, colour wheels, the curve, the
//! keyboard on a slider) and the speed-edit readout over a synthetic photo: English dark
//! left to right and Hebrew light right to left, at 1× and 1.5×. Rendered offscreen on
//! lavapipe and compared with the PNGs in `tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular-widgets --test develop_shots`, then review and
//! commit the rewritten PNGs.

use specular_gfx::{DrawList, Rect};
use specular_i18n::{I18n, LanguageIdentifier, langid, t};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};
use specular_widgets::choice::Choice;
use specular_widgets::curve::Curve;
use specular_widgets::develop_panel::{self, Body, Card, Control, DevelopPanel, Item};
use specular_widgets::slider::{Slider, SliderSpec, Unit};
use specular_widgets::wheels::Wheel;

const SIZE: (f32, f32) = (960.0, 820.0);

fn slider(label: String, min: f32, max: f32, default: f32, value: f32) -> Slider {
    let mut s = Slider::new(SliderSpec::new(label, min, max, default));
    s.set(value);
    s
}

/// The panel as an edit in progress leaves it.
fn panel() -> DevelopPanel {
    let mut ev = Slider::new(
        SliderSpec::new(t!("develop-exposure"), -5.0, 5.0, 0.0)
            .steps(0.05, 0.5)
            .unit(Unit::Ev, 2)
            .signed(),
    );
    ev.set(0.7);
    let mut black = Slider::new(
        SliderSpec::new(t!("develop-black-level"), -0.05, 0.05, 0.0)
            .unit(Unit::Plain, 3)
            .signed(),
    );
    black.set(-0.004);
    let wheels = [
        (
            "develop-wheel-shadows",
            "develop-luminance-shadows",
            220.0,
            0.35,
            -0.1,
        ),
        (
            "develop-wheel-midtones",
            "develop-luminance-midtones",
            0.0,
            0.0,
            0.0,
        ),
        (
            "develop-wheel-highlights",
            "develop-luminance-highlights",
            40.0,
            0.25,
            0.15,
        ),
    ]
    .iter()
    .map(|(name, lum, hue, s, l)| {
        let mut w = Wheel::new(I18n::global().tr(name));
        w.set(*hue, *s);
        let mut sl = Slider::new(
            SliderSpec::new(I18n::global().tr(lum), -1.0, 1.0, 0.0)
                .unit(Unit::Percent, 0)
                .signed()
                .compact(),
        );
        sl.set(*l);
        (w, sl)
    })
    .collect();
    let mut curve = Curve::identity(t!("develop-curve"));
    curve.set_points(&[[0.0, 0.04], [0.3, 0.24], [0.7, 0.78], [1.0, 1.0]]);
    let collapsed = |id: u32, title: &str, on: Option<bool>| Card {
        id,
        title: I18n::global().tr(title),
        enabled: on,
        expanded: false,
        controls: vec![Control {
            id: id * 10,
            body: Body::Slider(slider(t!("develop-contrast"), 0.0, 1.0, 0.5, 0.5)),
        }],
    };
    let cards = vec![
        collapsed(5, "develop-module-crop", None),
        Card {
            id: 2,
            title: t!("develop-module-exposure"),
            enabled: Some(true),
            expanded: true,
            controls: vec![
                Control {
                    id: 1,
                    body: Body::Slider(ev),
                },
                Control {
                    id: 2,
                    body: Body::Slider(black),
                },
            ],
        },
        collapsed(3, "develop-module-white-balance", Some(false)),
        Card {
            id: 10,
            title: t!("develop-module-colour"),
            enabled: Some(true),
            expanded: true,
            controls: vec![Control {
                id: 42,
                body: Body::Wheels(wheels),
            }],
        },
        Card {
            id: 12,
            title: t!("develop-module-curve"),
            enabled: Some(true),
            expanded: true,
            controls: vec![Control {
                id: 50,
                body: Body::Curve(curve),
            }],
        },
    ];
    let mut p = DevelopPanel::new(t!("develop-title"), cards);
    p.history = vec![
        t!("develop-history-original"),
        t!("develop-exposure"),
        t!("develop-colour-wheels"),
        t!("develop-curve"),
    ];
    p.history_at = 3;
    p.can_undo = true;
    p.set_focus(Some(Item::Control {
        card: 1,
        control: 0,
        part: 0,
    }));
    p
}

/// A synthetic photo: sky, sun, hills and a lake.
fn scene(dl: &mut DrawList, pal: &Palette, image: Rect) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas);
    let r = image.centered(image.w - 48.0, (image.w - 48.0) * 0.66);
    dl.push_clip(r);
    dl.rect(r, 0.0, Color::hex(0x6fa8dc));
    dl.circle(
        (r.x + r.w * 0.7, r.y + r.h * 0.3),
        r.h * 0.1,
        Color::hex(0xffd966),
    );
    dl.circle(
        (r.x + r.w * 0.25, r.y + r.h * 1.1),
        r.h * 0.6,
        Color::hex(0x38761d),
    );
    dl.rect(
        Rect::new(r.x, r.y + r.h * 0.85, r.w, r.h * 0.15),
        0.0,
        Color::hex(0x1c4587),
    );
    dl.pop_clip();
}

#[test]
fn develop_panel_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-widgets",
    );
    let cases: [(&str, LanguageIdentifier, bool); 2] = [
        ("develop-en-dark", langid!("en-US"), false),
        ("develop-he-light", langid!("he"), true),
    ];
    for (name, lang, rtl) in cases {
        I18n::global().set_language(lang.clone());
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for scale in [1.0f32, 1.5] {
            let mut p = panel();
            let window = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
            let pw = DevelopPanel::width(SIZE.0, 1.0);
            let image = Rect::new(if rtl { pw } else { 0.0 }, 0.0, SIZE.0 - pw, SIZE.1);
            let hud = {
                let mut s = Slider::new(
                    SliderSpec::new(t!("develop-exposure"), -5.0, 5.0, 0.0)
                        .unit(Unit::Ev, 2)
                        .signed(),
                );
                s.set(0.7);
                s
            };
            let shot = h.render(
                SIZE,
                scale,
                |dl| scene(dl, &pal, image),
                |dl, text| {
                    p.draw(dl, text, &pal, window, rtl, 1.0);
                    develop_panel::speed_hud(dl, text, &pal, image, &hud, rtl, 1.0);
                },
            );
            goldens.check(&format!("{name}-{scale}x"), &shot);
        }
    }
    I18n::global().set_language(langid!("en-US"));
    goldens.finish();
}

/// Buttons whose labels mix Hebrew and Latin (style, snapshot and version names put into
/// messages) keep each name's word order right to left, and a panel too narrow for the
/// title beside its buttons gives the title a row of its own.
#[test]
fn mixed_direction_buttons_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-widgets",
    );
    // This thread only: the other test here switches the global language.
    I18n::with_thread_language(langid!("he"), || mixed_buttons(&mut h, &mut goldens));
    goldens.finish();
}

fn mixed_buttons(h: &mut Harness, goldens: &mut Goldens) {
    h.text.set_locale(Some("he".to_owned()));
    let pal = Palette::light();
    let size = (480.0, 420.0);
    let button = |id: u32, label: String| Control {
        id,
        body: Body::Choice(Choice::button(label)),
    };
    for scale in [1.0f32, 1.5] {
        let cards = vec![Card {
            id: 1,
            title: t!("develop-snapshots"),
            enabled: None,
            expanded: true,
            controls: vec![
                button(1, "Wedding warm!".to_owned()),
                button(2, t!("develop-snapshot-comparing", name = "Warm look 2")),
                button(3, t!("develop-version-open", name = "Square crop")),
                button(4, "Look חם 2.0".to_owned()),
                Control {
                    id: 5,
                    body: Body::Choice(Choice::new(
                        "סגנון".to_owned(),
                        vec!["Warm look!".to_owned(), "גוון חם".to_owned()],
                    )),
                },
            ],
        }];
        let mut p = DevelopPanel::new(t!("develop-title"), cards);
        p.can_undo = true;
        let window = Rect::new(0.0, 0.0, size.0, size.1);
        let shot = h.render(
            size,
            scale,
            |dl| dl.rect(window, 0.0, pal.canvas),
            |dl, text| p.draw(dl, text, &pal, window, true, 1.0),
        );
        goldens.check(&format!("develop-bidi-buttons-he-light-{scale}x"), &shot);
    }
}
