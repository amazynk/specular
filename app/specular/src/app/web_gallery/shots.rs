//! Screenshot tests of the client gallery and client picks dialogs: dark left to right and
//! light right to left (Hebrew), at 1× and 1.5×, and the picks dialog with names matched,
//! missing and ambiguous. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular web_gallery::shots`, then review
//! and commit the rewritten PNGs.

use super::view::{a11y, a11y_target, draw};
use super::*;
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::{Color, Palette};

const SIZE: (f32, f32) = (900.0, 760.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.08, 0.10, 0.14, 1.0),
    );
    for i in 0..4 {
        dl.rect(
            Rect::new(60.0 + i as f32 * 200.0, 80.0, 170.0, 120.0),
            6.0,
            Color::rgba(0.25 + 0.15 * i as f32, 0.35, 0.5, 1.0),
        );
    }
}

fn photos(dir: &str, n: usize) -> Vec<PathBuf> {
    (1..=n)
        .map(|i| PathBuf::from(format!("/photos/{dir}/IMG_{i:04}.CR3")))
        .collect()
}

fn gallery_dialog(title: &str, watermark: &str, focus: Part) -> GalleryDialog {
    let mut t = LineEdit::new(title);
    t.set_caret(t.text().len(), false);
    GalleryDialog {
        sources: vec![
            (
                Source::Selection,
                t!("webgallery-source-selection"),
                photos("smith", 24),
            ),
            (
                Source::Shown,
                t!("webgallery-source-album", name = "Smith wedding"),
                photos("smith", 180),
            ),
        ],
        source: 0,
        title: t,
        watermark: LineEdit::new(watermark),
        focus,
        preedit: String::new(),
        current: None,
    }
}

fn picks_dialog(text: &str) -> PicksDialog {
    let mut all = photos("smith", 30);
    all.push(PathBuf::from("/photos/other/IMG_0007.CR3"));
    let mut d = PicksDialog {
        paste: LineEdit::new(text),
        from: None,
        photos: all,
        matched: Matched::default(),
        names: 0,
        focus: Part::ApplyAs,
        preedit: String::new(),
        error: None,
        choosing: None,
    };
    d.rematch();
    d
}

fn render_check(
    h: &mut Harness,
    goldens: &mut Goldens,
    name: &str,
    scale: f32,
    pal: &Palette,
    rtl: bool,
    v: &FormView,
) {
    let mut drawn = None;
    let shot = h.render(SIZE, scale, backdrop, |dl, text| {
        drawn = Some(draw(dl, text, pal, v, SIZE, rtl, 1.0));
    });
    let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
    a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
    goldens.check(name, &shot);
    super::super::a11y_audit::expect_clean(name, &tree, rtl);
}

#[test]
fn client_gallery_dialogs_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-web-gallery",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let (dark, light) = (Palette::dark(), Palette::light());
    let choices = Choices {
        theme: Theme::Dark,
        exif: true,
        ..Choices::default()
    };
    for (scale, suffix) in SCALES {
        for (name, pal, rtl) in [("ltr-dark", &dark, false), ("he-rtl-light", &light, true)] {
            let lang = if rtl { &he } else { &en };
            specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let d = if rtl {
                    gallery_dialog("חתונה, יוני 2026", "© סטודיו", Part::Watermark)
                } else {
                    gallery_dialog("Smith wedding", "", Part::Title)
                };
                let v = gallery_view(&d, &choices, None);
                let n = format!("web-gallery-{name}-{suffix}");
                render_check(&mut h, &mut goldens, &n, scale, pal, rtl, &v);
            });
        }
    }
    // Writing: the progress, Stop, and Write gallery… off.
    specular_i18n::I18n::with_thread_language(en.clone(), || {
        let mut d = gallery_dialog("Smith wedding", "PROOF", Part::Cancel);
        d.source = 1;
        let p = Progress {
            done: 42,
            total: 180,
            fraction: 0.3,
        };
        let v = gallery_view(&d, &choices, Some(&p));
        assert_eq!(v.cancel, t!("webgallery-stop"));
        render_check(
            &mut h,
            &mut goldens,
            "web-gallery-busy",
            1.0,
            &dark,
            false,
            &v,
        );
    });
    // The picks: two found, one missing, one in two folders.
    for (name, lang, pal, rtl) in [
        ("client-picks-ltr-dark", &en, &dark, false),
        ("client-picks-he-rtl-light", &he, &light, true),
    ] {
        specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let d = picks_dialog("IMG_0003.CR3, IMG_0012.jpg, IMG_0099.CR3, IMG_0007.CR3");
            let c = Choices {
                apply: 1,
                ..Choices::default()
            };
            let v = picks_view(&d, &c);
            assert!(v.ok_enabled);
            render_check(&mut h, &mut goldens, name, 1.0, pal, rtl, &v);
        });
    }
    goldens.finish();
}

/// Every part sits in the sheet without overlapping, right to left mirrors, large text fits,
/// and each part has its node.
#[test]
fn parts_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let v = gallery_view(
        &gallery_dialog("Smith wedding", "PROOF", Part::Ok),
        &Choices::default(),
        None,
    );
    let p = picks_view(&picks_dialog("IMG_0001.CR3"), &Choices::default());
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.4));
        out.push(draw(dl, text, &pal, &p, SIZE, false, 1.0));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in &out {
        let s = d.sheet;
        assert!(s.y >= 0.0 && s.bottom() <= SIZE.1, "{s:?} fits the window");
        // The rows' own hits (a choice's arrows sit inside its row).
        let rows: Vec<_> = d
            .hits
            .iter()
            .filter(|(_, h)| matches!(h, Hit::Part(_)))
            .collect();
        for (i, (a, ha)) in rows.iter().enumerate() {
            assert!(
                a.x >= s.x - 0.01
                    && a.right() <= s.right() + 0.01
                    && a.y >= s.y - 0.01
                    && a.bottom() <= s.bottom() + 0.01,
                "{ha:?} {a:?} outside {s:?}"
            );
            for (b, hb) in &rows[i + 1..] {
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
    for (view, d) in [(&v, l), (&p, &out[3])] {
        let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
        a11y(&mut tree, view, d, 1.0);
        assert_eq!(a11y_target(tree.focus.0), Some(view.focus));
        let mut parts = Vec::new();
        for (id, _) in &tree.nodes {
            parts.extend(a11y_target(id.0));
        }
        for part in view.parts() {
            assert!(parts.contains(&part), "{part:?} has no node");
        }
    }
}

/// The password note is in the dialog, the location is off by default, and a picks list
/// marks only what matched, in one undoable change of the marks.
#[test]
fn defaults_note_and_marks() {
    let v = gallery_view(
        &gallery_dialog("Smith", "", Part::Title),
        &Choices::default(),
        None,
    );
    let notes: Vec<&String> = v
        .rows
        .iter()
        .filter_map(|r| match r {
            Row::Note(t, true) => Some(t),
            _ => None,
        })
        .collect();
    assert!(notes[0].contains(&t!("webgallery-note-private")));
    assert!(
        v.rows
            .iter()
            .any(|r| matches!(r, Row::Check(Part::Location, _, false)))
    );
    let m = Mark {
        rating: 2,
        label: 0,
        flag: 0,
    };
    assert_eq!(ApplyAs::Pick.apply(m).flag, 1);
    assert_eq!(ApplyAs::Stars(5).apply(m).rating, 5);
    assert_eq!(ApplyAs::Label(3).apply(m).label, 3);
    assert_eq!(ApplyAs::Label(3).apply(m).rating, 2);
    // Pasted lines become one line of names.
    let mut d = picks_dialog("");
    d.focus = Part::Paste;
    assert_eq!(d.names, 0);
    let mut w = WebGallery {
        picks: Some(d),
        ..WebGallery::default()
    };
    if let Some(f) = w.field_mut() {
        f.paste("IMG_0001.CR3, IMG_0002.CR3, nope.jpg");
    }
    w.field_changed();
    let d = w.picks.as_ref().expect("open");
    assert_eq!(d.names, 3);
    assert_eq!(d.matched.photos().len(), 2);
    assert_eq!(d.matched.missing, ["nope.jpg"]);
    let v = picks_view(d, &Choices::default());
    assert!(v.ok_enabled);
}
