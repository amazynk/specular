//! Screenshot tests of the export recipes sheet at 1× and 1.5×: the list (also in Hebrew),
//! the editor with a text watermark, the delete and replace questions, a run and the end
//! report with failures. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`; see `specular-shots` for the tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular export_recipes::shots`, then
//! review and commit the rewritten PNGs.

use super::*;
use specular_gfx::Rect;
use specular_shots::{Goldens, Harness};
use specular_theme::Color;

const SIZE: (f32, f32) = (1000.0, 720.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.32, 0.45, 1.0),
    );
}

fn dialog(phase: Phase) -> Dialog {
    let mut recipes = Recipe::starters();
    recipes.push(Recipe {
        name: "Client proofs".into(),
        format: Format::Jxl,
        quality: 85,
        size: Size::Exact {
            width: 1080,
            height: 1350,
        },
        watermark: recipe::Watermark {
            kind: MarkKind::Text("© Ana Silva".into()),
            ..recipe::Watermark::default()
        },
        destination: Destination::Ask,
        ..Recipe::default()
    });
    Dialog {
        sources: vec![(
            Source::Selection,
            (0..24)
                .map(|i| PathBuf::from(format!("/photos/2026/IMG_{i:04}.CR3")))
                .collect(),
        )],
        source: 0,
        checked: vec![true, true, false, false],
        recipes,
        current: 1,
        develop: None,
        phase,
        focus: Some(2),
        drawn: Drawn::default(),
        preedit: String::new(),
        probe: Some((
            PathBuf::from("/photos/2026/IMG_0000.CR3"),
            Some(recipe::Facts {
                size: (6000, 4000),
                date: Some([2026, 7, 14, 18, 3, 9]),
                camera: Some("Canon EOS R5".into()),
                rating: Some(4),
            }),
        )),
        probing: None,
        picking: None,
        run_recipes: Recipe::starters(),
    }
}

fn report() -> RecipeReport {
    let mut results = Vec::new();
    for i in 0..22 {
        results.push((
            PathBuf::from(format!("/photos/IMG_{i:04}.CR3")),
            i % 2,
            Outcome::Converted {
                out: PathBuf::from(format!("/photos/web/IMG_{i:04}.jpg")),
                bytes: 1_000_000,
            },
        ));
    }
    results.push((
        PathBuf::from("/photos/IMG_0100.CR3"),
        0,
        Outcome::Skipped {
            existing: PathBuf::from("/photos/web/IMG_0100.jpg"),
        },
    ));
    results.push((
        PathBuf::from("/photos/scan.tif"),
        1,
        Outcome::Failed(specular_convert::Failure::Decode(
            "truncated strip 12".to_owned(),
        )),
    ));
    RecipeReport {
        results,
        cancelled: false,
        elapsed: std::time::Duration::from_secs(9),
    }
}

fn views() -> Vec<(String, View, bool)> {
    let mut out = Vec::new();
    for (lang, rtl) in [("en", false), ("he", true)] {
        let id = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(id, || {
            if rtl {
                // The user's own recipes, named in Hebrew (the starters' words are English
                // until translated).
                let mut d = dialog(Phase::List);
                d.recipes = vec![
                    Recipe {
                        name: "\u{5d0}\u{5ea}\u{5e8} 2048".into(),
                        size: Size::LongEdge(2048),
                        destination: Destination::Subfolder("web".into()),
                        ..Recipe::default()
                    },
                    Recipe {
                        name: "\u{5d3}\u{5d5}\u{5d0}\u{5f4}\u{5dc}".into(),
                        size: Size::LongEdge(1280),
                        quality: 75,
                        destination: Destination::Subfolder("mail".into()),
                        ..Recipe::default()
                    },
                ];
                d.checked = vec![true, false];
                d.current = 0;
                out.push((format!("list-{lang}"), d.view(None), rtl));
                return;
            }
            out.push((format!("list-{lang}"), dialog(Phase::List).view(None), rtl));
            let mut ed = Editor::new(Some(3), dialog(Phase::List).recipes[3].clone());
            ed.pattern.set_text("{date:%Y%m%d}_{seq:3}_{text}");
            ed.text.set_text("proof");
            let mut d = dialog(Phase::Edit(Box::new(ed)));
            d.focus = Some(0);
            out.push(("edit-en".into(), d.view(None), false));
            let mut bad = Editor::new(None, Recipe::default());
            bad.pattern.set_text("{name}-{size}");
            let mut d = dialog(Phase::Edit(Box::new(bad)));
            d.focus = Some(0);
            out.push(("edit-error-en".into(), d.view(None), false));
            let mut d = dialog(Phase::ConfirmDelete(1));
            d.focus = Some(0);
            out.push(("delete-en".into(), d.view(None), false));
            let files: Vec<PathBuf> = (0..7)
                .map(|i| PathBuf::from(format!("/photos/web/IMG_{i:04}.jpg")))
                .collect();
            let mut d = dialog(Phase::ConfirmReplace(files));
            d.focus = Some(1);
            out.push(("replace-en".into(), d.view(None), false));
            let mut v = dialog(Phase::Running).view(None);
            v.phase = PhaseView::Running {
                status: progress_text(&Progress {
                    done: 17,
                    total: 48,
                    failed: 0,
                    skipped: 0,
                    elapsed: std::time::Duration::from_secs(6),
                    eta: Some(std::time::Duration::from_secs(12)),
                }),
                progress: 17.0 / 48.0,
                stopping: false,
            };
            v.focus = Some(1);
            out.push(("running-en".into(), v, false));
            let mut d = dialog(Phase::Done(report()));
            d.focus = Some(0);
            out.push(("done-en".into(), d.view(None), false));
        });
    }
    out
}

#[test]
fn export_recipes_sheet_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-export-recipes",
    );
    let pal = Palette::dark();
    let shots = views();
    for (scale, suffix) in SCALES {
        for (name, v, rtl) in &shots {
            let id = if *rtl {
                specular_i18n::langid!("he")
            } else {
                specular_i18n::langid!("en-US")
            };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(id, || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(view::draw(dl, text, &pal, v, SIZE, *rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                view::a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("export-recipes-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("export-recipes-{name}-{suffix}"),
                &tree,
                *rtl,
            );
        }
    }
    goldens.finish();
}

/// Every control sits inside the sheet, none overlap, and right-to-left mirrors them.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    for (name, v, _) in views() {
        let mut ltr = None;
        let mut rtl = None;
        h.render(SIZE, 1.0, backdrop, |dl, text| {
            ltr = Some(view::draw(dl, text, &pal, &v, SIZE, false, 1.0));
            rtl = Some(view::draw(dl, text, &pal, &v, SIZE, true, 1.0));
        });
        let (ltr, rtl) = (ltr.unwrap(), rtl.unwrap());
        let inside = |r: &Rect, s: &Rect| {
            r.x >= s.x - 0.5
                && r.y >= s.y - 0.5
                && r.right() <= s.right() + 0.5
                && r.bottom() <= s.bottom() + 0.5
        };
        for (r, hit) in &ltr.hits {
            assert!(
                inside(r, &ltr.sheet),
                "{name}: {hit:?} {r:?} outside {:?}",
                ltr.sheet
            );
        }
        // Buttons and fields do not overlap each other.
        let solid: Vec<_> = ltr
            .hits
            .iter()
            .filter(|(_, h)| {
                !matches!(
                    h,
                    Hit::Row(_) | Hit::Recipe(_) | Hit::Check(_) | Hit::Dec(_) | Hit::Inc(_)
                )
            })
            .collect();
        for (i, (a, ha)) in solid.iter().enumerate() {
            for (b, hb) in &solid[i + 1..] {
                let apart = a.right() <= b.x + 0.5
                    || b.right() <= a.x + 0.5
                    || a.bottom() <= b.y + 0.5
                    || b.bottom() <= a.y + 0.5;
                assert!(apart, "{name}: {ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
        for ((a, ha), (b, hb)) in ltr.hits.iter().zip(&rtl.hits) {
            assert_eq!(ha, hb);
            assert!(
                ((a.x + a.w) - (SIZE.0 - b.x)).abs() < 1.0,
                "{name}: {ha:?} not mirrored"
            );
        }
    }
}
