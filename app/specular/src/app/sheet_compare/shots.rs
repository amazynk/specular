//! Screenshot tests of sheet compare: two revisions of a small drawing overlaid in red and
//! cyan with the changes clouded, and the fade slider; left to right and in Hebrew.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular sheet_compare::shots`, then review
//! the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (900.0, 620.0);
/// Logical px per pixel of the compared copies.
const CELL: f32 = 3.0;
const ORIGIN: (f32, f32) = (60.0, 150.0);

/// A 260 × 140 drawing; the revision moves a wall and adds a door.
fn sheets() -> (Grey, Grey) {
    let (w, h) = (260usize, 140usize);
    let mut a = vec![255u8; w * h];
    let line = |px: &mut Vec<u8>, x0: usize, y0: usize, x1: usize, y1: usize| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                px[y * w + x] = 0;
            }
        }
    };
    line(&mut a, 5, 5, 254, 6);
    line(&mut a, 5, 133, 254, 134);
    line(&mut a, 5, 5, 6, 134);
    line(&mut a, 253, 5, 254, 134);
    line(&mut a, 20, 30, 120, 31);
    line(&mut a, 20, 30, 21, 110);
    line(&mut a, 20, 110, 200, 111);
    line(&mut a, 160, 30, 161, 110);
    let mut b = a.clone();
    // Moved wall: 160 → 180; a door at the top.
    for y in 30..=110 {
        b[y * w + 160] = 255;
        b[y * w + 161] = 255;
    }
    line(&mut b, 180, 30, 181, 110);
    for y in 30..=31 {
        for x in 62..=80 {
            b[y * w + x] = 255;
        }
    }
    line(&mut b, 60, 12, 61, 29);
    (Grey::new(w, h, a), Grey::new(w, h, b))
}

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.2, 0.22, 1.0),
    );
    let (a, b) = sheets();
    dl.rect(
        Rect::new(ORIGIN.0, ORIGIN.1, a.w as f32 * CELL, a.h as f32 * CELL),
        0.0,
        Color::WHITE,
    );
    let rgba = compose(&a, &b, View::Overlay);
    draw_rgba(dl, &rgba, a.w, a.h, ORIGIN, CELL);
}

fn sample(mode: Mode) -> CompareView {
    let (a, b) = sheets();
    let found = changes(&a, &b, 1, 6, 4);
    let regions = found
        .iter()
        .map(|r| {
            Rect::from_min_max(
                ORIGIN.0 + r[0] as f32 * CELL,
                ORIGIN.1 + r[1] as f32 * CELL,
                ORIGIN.0 + r[2] as f32 * CELL,
                ORIGIN.1 + r[3] as f32 * CELL,
            )
        })
        .collect();
    CompareView {
        other: Some("A-101 rev C.pdf".to_owned()),
        page: Some((0, 3)),
        mode,
        blend: 0.35,
        blink: false,
        aligned: true,
        changes: found.len(),
        status: t!(
            "sheets-aligned",
            scale = "1.0000".to_owned(),
            angle = "0.00".to_owned(),
            error = "0.08".to_owned(),
            changes = found.len() as i64
        ),
        focus: Some(1),
        regions,
    }
}

#[test]
fn sheet_compare_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-sheet-compare",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let pal = Palette::dark();
    for (name, lang, rtl, mode) in [
        ("ltr", &en, false, Mode::Overlay),
        ("he-rtl", &he, true, Mode::Overlay),
        ("fade", &en, false, Mode::Blend),
    ] {
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let v = sample(mode);
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, &pal, &v, area, rtl, 1.0));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(&format!("sheet-compare-{name}"), &shot);
        super::super::a11y_audit::expect_clean(&format!("sheet-compare-{name}"), &tree, rtl);
    }
    goldens.finish();
}

/// The two changes (the moved wall, the door) are found, each node maps back to its
/// control, and the slider is there only when fading.
#[test]
fn changes_and_controls() {
    let v = sample(Mode::Overlay);
    assert!(v.changes >= 2, "{}", v.changes);
    for f in v.focusables() {
        assert_eq!(target(A11Y_BASE + node(f), &v), Some(f));
    }
    assert!(!v.focusables().contains(&Hit::Slider));
    assert!(sample(Mode::Blend).focusables().contains(&Hit::Slider));
    let none = CompareView {
        other: None,
        ..sample(Mode::Overlay)
    };
    assert!(!none.enabled(Hit::Auto));
    assert!(none.enabled(Hit::Other));
}
