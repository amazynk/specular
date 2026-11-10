//! Screenshot tests of the password prompt: dark, left to right, the password
//! hidden; and light, right to left (Hebrew), shown, after a wrong password. Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular password::shots`, then review and
//! commit the rewritten PNGs.

use super::view::{a11y, a11y_target, draw};
use super::*;
use specular_gfx::Rect;
use specular_shots::{Goldens, Harness};
use specular_theme::Color;

const SIZE: (f32, f32) = (900.0, 640.0);
const SCALES: [(f32, &str); 1] = [(1.0, "1x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.12, 0.12, 0.13, 1.0),
    );
    dl.rect(
        Rect::new(SIZE.0 * 0.3, SIZE.1 * 0.2, SIZE.0 * 0.4, SIZE.1 * 0.6),
        SIZE.1 * 0.3,
        Color::rgba(0.45, 0.45, 0.47, 1.0),
    );
}

fn prompt(show: bool, wrong: bool, focus: Part) -> Prompt {
    let mut d = Prompt {
        archive: PathBuf::from("/home/me/Comics/Les Cités obscures 01.cbz"),
        field: SecretField::new(),
        show,
        wrong,
        focus,
        drawn: Drawn::default(),
    };
    d.field.insert("correct horse");
    d
}

#[test]
fn password_prompt_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-password-prompt",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let hidden = build_view(&prompt(false, false, Part::Field));
    let shown_he = specular_i18n::I18n::with_thread_language(he.clone(), || {
        build_view(&prompt(true, true, Part::Show))
    });
    // Hebrew, the caret inside the text: dots counted from the right, and a shown
    // password of Hebrew, Latin and digits with the caret in its Latin word.
    let caret_he = specular_i18n::I18n::with_thread_language(he.clone(), || {
        let mut d = prompt(false, false, Part::Field);
        d.field.caret_at_char(4);
        build_view(&d)
    });
    let mixed_he = specular_i18n::I18n::with_thread_language(he.clone(), || {
        let mut d = prompt(true, false, Part::Field);
        d.field.clear();
        d.field.insert("\u{5e9}\u{5dc}\u{5d5}\u{5dd} abc 12");
        d.field.caret_at_char(6);
        build_view(&d)
    });
    let (dark, light) = (Palette::dark(), Palette::light());
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl) in [
            ("hidden", &hidden, &dark, false),
            ("wrong-he-rtl-light", &shown_he, &light, true),
            ("caret-he", &caret_he, &dark, true),
            ("mixed-he", &mixed_he, &dark, true),
        ] {
            let lang = if rtl { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, v, SIZE, rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("password-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("password-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
    }
    goldens.finish();
}

/// Right to left, the caret moves through the field as the text does: through the dots
/// from the right edge leftwards, and a click puts it back where it was drawn.
#[test]
fn rtl_carets_follow_the_text() {
    let Some(mut h) = Harness::new() else { return };
    let he = specular_i18n::langid!("he");
    let pal = Palette::dark();
    let mut xs = Vec::new();
    for n in 0..=4 {
        let mut d = prompt(false, false, Part::Field);
        d.field.clear();
        d.field.insert("abcd");
        d.field.caret_at_char(n);
        let v = specular_i18n::I18n::with_thread_language(he.clone(), || build_view(&d));
        let mut drawn = None;
        let _ = h.render(SIZE, 1.0, backdrop, |dl, text| {
            drawn = Some(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        });
        let (block, x0, mirrored) = drawn.expect("drawn").field_text.expect("text");
        assert!(mirrored);
        let x = block.width - block.caret_x(v.caret);
        xs.push(x0 + x);
        // A click there gives the same byte.
        assert_eq!(block.hit_byte(block.width - x), v.caret, "char {n}");
    }
    assert!(xs.windows(2).all(|w| w[1] < w[0]), "{xs:?}");
}

/// Every part sits in the sheet without overlapping, right to left mirrors, large text
/// fits, and the screen reader gets dots, never the password, while it is hidden.
#[test]
fn parts_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let v = build_view(&prompt(false, true, Part::Open));
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, false, 2.0));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in [&out[0], &out[2]] {
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
    let mut tree = accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    a11y(&mut tree, &v, l, 1.0);
    assert_eq!(a11y_target(tree.focus.0), Some(Part::Open));
    let mut parts = Vec::new();
    let mut text = String::new();
    for (id, n) in &tree.nodes {
        text += &format!("{:?} {:?} {:?} ", n.label(), n.value(), n.description());
        parts.extend(a11y_target(id.0));
        if n.role() == accesskit::Role::PasswordInput {
            assert_eq!(n.label(), Some(v.label.as_str()));
        }
    }
    for p in Part::ORDER {
        assert!(parts.contains(&p), "{p:?} has no node");
    }
    assert!(
        !text.contains("correct"),
        "the password reached the a11y tree"
    );
    assert!(text.contains(&t!("password-wrong")));
}
