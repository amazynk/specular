//! Screenshot tests of the thermal report dialog: dark left to right with the title field
//! focused, and light right to left (Hebrew) with the notes typed, the photo on and the
//! folder's images added, at 1× and 1.5×. Rendered offscreen on lavapipe and compared with
//! the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular thermal_report::shots`, then
//! review and commit the rewritten PNGs.

use super::view::{a11y, a11y_target, draw};
use super::*;
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};

const SIZE: (f32, f32) = (900.0, 640.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.10, 0.04, 0.16, 1.0),
    );
    dl.rect(
        Rect::new(SIZE.0 * 0.25, SIZE.1 * 0.2, SIZE.0 * 0.5, SIZE.1 * 0.6),
        SIZE.1 * 0.3,
        Color::rgba(0.85, 0.3, 0.1, 1.0),
    );
}

fn dialog(notes: &str, photo: Option<bool>, folder: bool, focus: Part) -> Dialog {
    let mut title = LineEdit::new(&t!("thermal-report-default-title"));
    title.set_caret(title.text().len(), false);
    let mut n = LineEdit::new("");
    n.insert(notes);
    Dialog {
        title,
        notes: n,
        photo,
        folder,
        docx: false,
        kmz: false,
        inspection: false,
        focus,
        preedit: String::new(),
        subtitle: "FLIR0042.jpg".into(),
        drawn: view::Drawn::default(),
    }
}

#[test]
fn thermal_report_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-thermal-report",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let (dark, light) = (Palette::dark(), Palette::light());
    for (scale, suffix) in SCALES {
        for (name, pal, rtl) in [("title", &dark, false), ("he-rtl-light", &light, true)] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let v = if rtl {
                    build_view(
                        &dialog("גג צפוני, ליד המרזב", Some(true), true, Part::Notes),
                        false,
                    )
                } else {
                    build_view(&dialog("", Some(true), false, Part::Title), false)
                };
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, &v, SIZE, rtl, 1.0));
                });
                let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            let n = format!("thermal-report-{name}-{suffix}");
            goldens.check(&n, &shot);
            super::super::super::a11y_audit::expect_clean(&n, &tree, rtl);
        }
    }
    goldens.finish();
}

/// Every part sits in the sheet without overlapping, right to left mirrors, large text fits,
/// a still without a photo has no photo checkbox, and each part has its node.
#[test]
fn parts_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let v = build_view(
        &dialog("Roof, north side", Some(false), true, Part::Save),
        true,
    );
    let no_photo = build_view(&dialog("", None, false, Part::Title), false);
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 2.0));
        out.push(draw(dl, text, &pal, &no_photo, SIZE, false, 1.0));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in &out[..3] {
        let s = d.sheet;
        assert_eq!(d.hits.len(), Part::ORDER.len());
        for (i, (a, ha)) in d.hits.iter().enumerate() {
            assert!(
                a.x >= s.x - 0.01
                    && a.right() <= s.right() + 0.01
                    && a.y >= s.y - 0.01
                    && a.bottom() <= s.bottom() + 0.01,
                "{ha:?} {a:?} outside {s:?}"
            );
            for (b, hb) in &d.hits[i + 1..] {
                let apart = a.right() <= b.x + 0.01
                    || b.right() <= a.x + 0.01
                    || a.bottom() <= b.y + 0.01
                    || b.bottom() <= a.y + 0.01;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
    }
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
    assert!(!out[3].hits.iter().any(|(_, p)| *p == Part::Photo));
    let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
    a11y(&mut tree, &v, l, 1.0);
    assert_eq!(a11y_target(tree.focus.0), Some(Part::Save));
    let mut parts = Vec::new();
    for (id, _) in &tree.nodes {
        parts.extend(a11y_target(id.0));
    }
    for p in Part::ORDER {
        assert!(parts.contains(&p), "{p:?} has no node");
    }
}

/// The report's pieces: values with their unit, NaN as a dash, the palette between the
/// limits, and the whole plane's statistics.
#[test]
fn report_pieces() {
    assert_eq!(temp(36.44, TemperatureUnit::Celsius), "36.4 °C");
    assert_eq!(temp(f64::NAN, TemperatureUnit::Kelvin), "–");
    let p = NdPlane {
        width: 3,
        height: 1,
        values: vec![10.0, 20.0, f32::NAN],
        truncated: false,
    };
    let mut lut = [[0u8; 3]; 256];
    for (i, c) in lut.iter_mut().enumerate() {
        *c = [i as u8, 0, 255 - i as u8];
    }
    let img = colour(&p, &lut, (10.0, 20.0));
    assert_eq!(img.rgb, vec![0, 0, 255, 255, 0, 0, 0, 0, 0]);
    assert_eq!(whole(&p), (10.0, 20.0, 15.0));
}

/// An input method composes in the field with the keys and nowhere else: the composition
/// shows at the caret with the candidate window there; moving the keys to a checkbox drops
/// it.
#[test]
fn the_focused_field_takes_the_composition() {
    let mut s = ReportState {
        dialog: Some(dialog("", None, false, Part::Notes)),
        job: None,
    };
    s.set_preedit("にほ");
    let v = build_view(s.dialog.as_ref().expect("open"), false);
    assert_eq!(v.notes.preedit, "にほ");
    assert_eq!(v.title.preedit, "", "only the focused field composes");
    s.dialog.as_mut().expect("open").focus = Part::Folder;
    s.set_preedit("x");
    assert_eq!(s.dialog.as_ref().expect("open").preedit, "");
    assert_eq!(s.ime_area(), None, "no field has the keys");

    let Some(mut h) = Harness::new() else { return };
    let d = s.dialog.as_mut().expect("open");
    d.focus = Part::Notes;
    d.notes.insert("ab");
    d.notes.set_caret(1, false);
    let plain = build_view(d, false);
    d.preedit = "日本".into();
    let composing = build_view(d, false);
    let pal = Palette::dark();
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &plain, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &composing, SIZE, false, 1.0));
    });
    let caret = out[0].ime.expect("the caret");
    let area = out[1].ime.expect("the composition");
    assert!(area.w > caret.w, "{area:?} spans the composition");
    assert!(
        (area.x - caret.x).abs() < 1.5,
        "{area:?} starts at the caret {caret:?}"
    );
    assert!(out[1].fields[1].1.width > out[0].fields[1].1.width);
    s.dialog.as_mut().expect("open").drawn = out.pop().expect("drawn");
    assert_eq!(s.ime_area(), Some(area));
}

/// The inspection report's dialog (any photo): no photo checkbox, the folder's photos, and
/// the Word and KMZ checkboxes on; left to right and Hebrew right to left.
#[test]
fn inspection_report_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-thermal-report",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let pal = Palette::dark();
    for (name, lang, rtl) in [("ltr", &en, false), ("he-rtl", &he, true)] {
        let (shot, tree, v) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let mut d = dialog("Pier 3, north face", None, true, Part::Docx);
            d.inspection = true;
            d.title = LineEdit::new(&t!("inspection-report-default-title"));
            d.docx = true;
            d.kmz = true;
            d.subtitle = "DJI_0042.JPG".into();
            let v = build_view(&d, false);
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, &pal, &v, SIZE, rtl, 1.0));
            });
            let mut tree = super::super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree, v)
        });
        assert!(!v.parts().contains(&Part::Photo));
        assert!(v.parts().contains(&Part::Docx) && v.parts().contains(&Part::Kmz));
        let n = format!("inspection-report-{name}");
        goldens.check(&n, &shot);
        super::super::super::a11y_audit::expect_clean(&n, &tree, rtl);
    }
    goldens.finish();
}
