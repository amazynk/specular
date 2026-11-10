//! Screenshot tests of the Merge to HDR sheet (three bracketed photos, one left out,
//! the options and the float depth), and of the Merge to panorama and Focus stack sheets. English dark left to right and Hebrew light right to
//! left, at 1× and 1.5×. Compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular develop::merge_shots`, then review
//! and commit the rewritten PNGs.

use std::path::PathBuf;

use specular_develop::merge::Shot;
use specular_gfx::{DrawList, Rect};
use specular_shots::{Goldens, Harness};
use specular_theme::Palette;

use crate::app::fileops::{Part, draw_sheet};

fn shots() -> Vec<Shot> {
    [
        ("IMG_2041.CR3", 1.0 / 500.0),
        ("IMG_2042.CR3", 1.0 / 125.0),
        ("IMG_2043.CR3", 1.0 / 30.0),
    ]
    .iter()
    .map(|(n, t)| Shot {
        path: PathBuf::from(format!("/photos/{n}")),
        exposure_time: Some(*t),
        f_number: Some(8.0),
        iso: Some(100.0),
        ..Shot::default()
    })
    .collect()
}

#[test]
fn merge_sheet_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-develop-merge",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut s = super::merge::sheet_for_shots(&shots());
                s.toggle(2);
                let v = s.view(Part::Row(0));
                let size = (720.0, 640.0);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas)
                    },
                    |dl, text| {
                        draw_sheet(dl, text, &pal, &v, size, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("develop-merge-sheet-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

/// Renders `sheet` (English dark, Hebrew light; 1× and 1.5×) against goldens `prefix-*`.
fn check_sheet(set: &str, prefix: &str, sheet: &dyn Fn() -> super::sheet::DevelopSheet) {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"), set);
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    for (name, lang, rtl) in [("en-dark", en, false), ("he-light", he, true)] {
        h.text.set_locale(Some(lang.language.as_str().to_owned()));
        let pal = if rtl {
            Palette::light()
        } else {
            Palette::dark()
        };
        for (scale, suffix) in [(1.0f32, "1x"), (1.5, "1.5x")] {
            let shot = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let v = sheet().view(Part::Row(0));
                let size = (720.0, 640.0);
                h.render(
                    size,
                    scale,
                    |dl: &mut DrawList| {
                        dl.rect(Rect::new(0.0, 0.0, size.0, size.1), 0.0, pal.canvas)
                    },
                    |dl, text| {
                        draw_sheet(dl, text, &pal, &v, size, rtl, 1.0);
                    },
                )
            });
            goldens.check(&format!("{prefix}-{name}-{suffix}"), &shot);
        }
    }
    h.text.set_locale(None);
    goldens.finish();
}

fn named(names: &[&str]) -> Vec<Shot> {
    names
        .iter()
        .map(|n| Shot {
            path: PathBuf::from(format!("/photos/{n}")),
            ..Shot::default()
        })
        .collect()
}

#[test]
fn pano_sheet_matches_goldens() {
    check_sheet("specular-develop-pano", "develop-pano-sheet", &|| {
        let mut s = super::pano::sheet_for_shots(&named(&[
            "DSC_0410.JPG",
            "DSC_0411.JPG",
            "DSC_0412.JPG",
            "DSC_0413.JPG",
        ]));
        s.toggle(3);
        s
    });
}

#[test]
fn stack_sheet_matches_goldens() {
    check_sheet("specular-develop-stack", "develop-stack-sheet", &|| {
        super::stack::sheet_for_shots(&named(&[
            "P1010201.ORF",
            "P1010202.ORF",
            "P1010203.ORF",
            "P1010204.ORF",
            "P1010205.ORF",
        ]))
    });
}
