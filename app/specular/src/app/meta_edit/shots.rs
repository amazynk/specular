//! Screenshot tests of the metadata editor at 1× and 1.5×: two photos that agree on some
//! fields and differ on others, with two templates, the keyboard in a field (dark, English);
//! and a template being edited, right to left in Hebrew (light). Rendered offscreen on
//! lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular meta_edit::shots`, then review and
//! commit the rewritten PNGs.

use super::tests::panel;
use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (720.0, 560.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn photos() -> MetaEdit {
    let r = |title: &str, kws: &[&str]| {
        let mut r = Record::new();
        r.set(Field::Title, vec![title.to_owned()]);
        r.set(Field::Copyright, vec!["© 2026 Ann Smith".to_owned()]);
        r.set(Field::Creator, vec!["Ann Smith".to_owned()]);
        r.set(
            Field::Keywords,
            kws.iter().map(|k| (*k).to_owned()).collect(),
        );
        r
    };
    let mut m = panel(&[
        (
            "/shots/harbour-01.jpg",
            r("Morning boats", &["sea", "Places|Lisbon"]),
        ),
        ("/shots/harbour-02.jpg", r("Nets drying", &["sea"])),
    ]);
    let mut t = Template::new("Agency defaults");
    t.set(Field::Copyright, vec!["© 2026 Ann Smith".into()]);
    m.templates.push(t);
    m.templates.push(Template::new("Lisbon assignment"));
    m.current = Some(0);
    m.status = None;
    m.rows = m.build_rows();
    let k = m
        .rows
        .iter()
        .position(|r| *r == Row::Field(Field::Headline))
        .unwrap();
    m.focus_row(Some(k));
    m.slot_mut(Field::Headline)
        .edit
        .insert("Fishing fleet returns");
    m
}

fn template() -> MetaEdit {
    let mut m = panel(&[("/shots/harbour-01.jpg", Record::new())]);
    let mut t = Template::new("סוכנות");
    t.set(Field::Copyright, vec!["© 2026 Ann Smith".into()]);
    t.set(
        Field::Keywords,
        vec!["חדשות".into(), "Places|Lisbon".into()],
    );
    t.keyword_mode = KeywordMode::Replace;
    m.templates.push(t);
    m.edit_template(0);
    m.focus = None;
    m.editing = false;
    m
}

fn draw_panel(
    h: &mut Harness,
    m: &mut MetaEdit,
    pal: &Palette,
    scale: f32,
    rtl: bool,
) -> specular_shots::Shot {
    h.render(
        SIZE,
        scale,
        |dl| dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, pal.canvas),
        |dl, text| {
            let w = side_panel::width(SIZE.0, 1.0);
            let x = if rtl { 0.0 } else { SIZE.0 - w };
            m.draw(dl, text, pal, Rect::new(x, 0.0, w, SIZE.1), 1.0, rtl, None);
        },
    )
}

#[test]
fn meta_edit_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-metaedit",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (scale, suffix) in SCALES {
        for (name, rtl, pal) in [
            ("photos-en-dark", false, Palette::dark()),
            ("template-he-light", true, Palette::light()),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut m = if rtl { template() } else { photos() };
                let shot = draw_panel(&mut h, &mut m, &pal, scale, rtl);
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                m.a11y(&mut tree, 1.0);
                (shot, tree)
            });
            goldens.check(&format!("metaedit-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("metaedit-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
    }
    goldens.finish();
}
