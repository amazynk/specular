//! Screenshot tests of the Open address dialog at 1× and 1.5×: an address typed with recent
//! ones below in English, the same right to left in Hebrew with a refused address, an input
//! method composing in the field, and the network switched off. Rendered offscreen on
//! lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular address::shots`, then review and
//! commit the rewritten PNGs.

use super::view::{a11y, draw};
use super::*;
use specular_gfx::Rect;
use specular_shots::{Goldens, Harness};
use specular_theme::Color;

const SIZE: (f32, f32) = (720.0, 480.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.22, 0.3, 0.4, 1.0),
    );
}

fn dialog(text: &str, focus: Part) -> Dialog {
    let mut field = LineEdit::new("");
    field.insert(text);
    Dialog {
        field,
        preedit: String::new(),
        focus,
        error: None,
        recent: vec![
            "https://uk1s3.embassy.ebi.ac.uk/idr/zarr/v0.4/idr0062A/6001240.zarr".into(),
            "https://data.example.org/cog/landsat-scene.tif".into(),
            "https://pacs.example.org/orthanc/dicom-web".into(),
        ],
        drawn: Drawn::default(),
        stream: false,
        streams: Vec::new(),
    }
}

/// The Open network stream dialog: recent streams by host and title.
fn stream_dialog(text: &str, focus: Part) -> Dialog {
    use crate::app::stream::Recent;
    let streams = vec![
        Recent {
            url: "https://radio.example.org:8000/jazz".into(),
            title: Some("Jazz FM".into()),
        },
        Recent {
            url: "https://cdn.example.net/live/news/master.m3u8".into(),
            title: None,
        },
        Recent {
            url: "https://media.example.com/talks/2026-keynote.mp4".into(),
            title: Some("Keynote 2026".into()),
        },
    ];
    let mut d = dialog(text, focus);
    d.recent = streams.iter().map(|r| r.url.clone()).collect();
    d.streams = streams;
    d.stream = true;
    d
}

#[test]
fn stream_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-stream",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let typed = specular_i18n::I18n::with_thread_language(en.clone(), || {
        build_view(
            &stream_dialog("https://radio.example.org/listen.pls", Part::Field),
            false,
        )
    });
    let refused = specular_i18n::I18n::with_thread_language(he.clone(), || {
        let mut d = stream_dialog("rtsp://cam.example/live", Part::Recent(0));
        d.error = Some(stream_problem_text(Problem::Scheme));
        build_view(&d, false)
    });
    for (scale, suffix) in SCALES {
        for (name, v, rtl, pal) in [
            ("typed-en-dark", &typed, false, Palette::dark()),
            ("refused-he-light", &refused, true, Palette::light()),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, &pal, v, SIZE, rtl, 1.0));
                });
                let drawn = drawn.expect("drawn");
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn, 1.0);
                (shot, tree)
            });
            goldens.check(&format!("stream-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(&format!("stream-{name}-{suffix}"), &tree, rtl);
        }
    }
    goldens.finish();
}

#[test]
fn address_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-address",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let typed = specular_i18n::I18n::with_thread_language(en.clone(), || {
        build_view(
            &dialog("https://h.example/data/plate.ome.zarr", Part::Field),
            false,
        )
    });
    let refused = specular_i18n::I18n::with_thread_language(he.clone(), || {
        let mut d = dialog("ftp://h.example/scan.tif", Part::Field);
        d.error = Some(problem_text(Problem::Scheme));
        build_view(&d, false)
    });
    let composing = specular_i18n::I18n::with_thread_language(en.clone(), || {
        let mut d = dialog("https://h.example/", Part::Field);
        d.recent.clear();
        d.preedit = "plate.zarr".into();
        build_view(&d, false)
    });
    let off = specular_i18n::I18n::with_thread_language(he.clone(), || {
        build_view(&dialog("", Part::Recent(1)), true)
    });
    let pal = Palette::dark();
    for (scale, suffix) in SCALES {
        for (name, v, rtl) in [
            ("typed-en", &typed, false),
            ("refused-he", &refused, true),
            ("composing-en", &composing, false),
            ("network-off-he", &off, true),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree, drawn) =
                specular_i18n::I18n::with_thread_language(lang.clone(), || {
                    let mut drawn = None;
                    let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                        drawn = Some(draw(dl, text, &pal, v, SIZE, rtl, 1.0));
                    });
                    let drawn = drawn.expect("drawn");
                    let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                    a11y(&mut tree, v, &drawn, 1.0);
                    (shot, tree, drawn)
                });
            goldens.check(&format!("address-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(&format!("address-{name}-{suffix}"), &tree, rtl);
            // Every part inside the sheet; the field's composition opens the candidate
            // window over the field.
            for (r, p) in &drawn.hits {
                assert!(
                    r.x >= drawn.sheet.x - 0.5 && r.right() <= drawn.sheet.right() + 0.5,
                    "{name}: {p:?} outside"
                );
            }
            if v.focus == Some(Part::Field) {
                let ime = drawn.ime.expect("the candidate window's place");
                let (field, _) = drawn
                    .hits
                    .iter()
                    .find(|(_, p)| *p == Part::Field)
                    .expect("field");
                assert!(ime.y >= field.y && ime.bottom() <= field.bottom(), "{name}");
            }
        }
    }
    goldens.finish();
}

/// Synthetic input method events: a composition shows in the field without changing its
/// text, a commit types it at the caret, and an empty composition (cancelled) leaves the
/// text as it was. Away from the field nothing is composed.
#[test]
fn input_method_composes_and_commits() {
    let mut s = AddressState {
        dialog: Some(dialog("https://h/", Part::Field)),
    };
    assert!(s.wants_text());
    s.set_preedit("にほ");
    assert_eq!(s.view().expect("open").preedit, "にほ");
    assert_eq!(s.view().expect("open").text, "https://h/");
    s.set_preedit("");
    assert_eq!(s.view().expect("open").text, "https://h/");
    s.set_preedit("日本");
    s.commit("日本");
    let v = s.view().expect("open");
    assert_eq!(
        (v.text.as_str(), v.preedit.as_str()),
        ("https://h/日本", "")
    );
    assert_eq!(v.caret, v.text.len());
    if let Some(d) = s.dialog.as_mut() {
        d.focus = Part::Open;
    }
    assert!(!s.wants_text());
    s.set_preedit("x");
    assert!(s.view().expect("open").preedit.is_empty());
}
