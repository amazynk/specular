//! Screenshot tests of the convert dialog at 1× and 1.5×: the setup sheet in English (dark),
//! the same right-to-left in Hebrew, a batch running and the end report with failures.
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`;
//! see `specular-shots` for the tolerance.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular convert::shots`, then review and
//! commit the rewritten PNGs.

use super::*;
use specular_convert::Outcome;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (900.0, 620.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// What stands for the viewer under the scrim.
fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.2, 0.32, 0.45, 1.0),
    );
    dl.rect(
        Rect::new(0.0, SIZE.1 * 0.6, SIZE.0, SIZE.1 * 0.4),
        0.0,
        Color::rgba(0.25, 0.38, 0.22, 1.0),
    );
}

/// JPEG at quality 85, long edge 2048, location removed, into "converted".
fn choices() -> Choices {
    Choices::new(Convert {
        format: ConvertFormat::Jpeg,
        jpeg_quality: 85,
        resize: ConvertResize::LongEdge,
        long_edge: 2048,
        metadata: ConvertMetadata::NoLocation,
        subfolder: "converted".to_owned(),
        folder: None,
        pattern: "{name}-{w}x{h}".to_owned(),
        ..Convert::default()
    })
}

fn setup_view(c: &Choices, pattern: &str, focus: Option<usize>) -> DialogView {
    let src = source_label(Source::Selection, 24);
    let first = Path::new("/photos/2026/IMG_2041.HEIC");
    let probe = Some(((4032, 3024), Some([2026, 7, 14, 18, 3, 9]), None));
    DialogView {
        phase: PhaseView::Setup {
            start: row_views(c, &c.start_rows(), &src),
            end: row_views(c, &c.end_rows(), &src),
            name: pattern.to_owned(),
            caret: pattern.len(),
            selection: (pattern.len(), pattern.len()),
            preview: preview(c, pattern, Some(first), probe),
            preedit: String::new(),
        },
        focus,
        export: false,
        delete_preset: None,
    }
}

fn report() -> Report {
    let mut results = Vec::new();
    for i in 0..17 {
        results.push((
            PathBuf::from(format!("/photos/IMG_{i:04}.HEIC")),
            Outcome::Converted {
                out: PathBuf::from(format!("/photos/converted/IMG_{i:04}.jpg")),
                bytes: 1_000_000,
            },
        ));
    }
    results.push((
        PathBuf::from("/photos/IMG_0100.HEIC"),
        Outcome::Skipped {
            existing: PathBuf::from("/photos/converted/IMG_0100.jpg"),
        },
    ));
    results.push((
        PathBuf::from("/photos/scan.tif"),
        Outcome::Failed(Failure::Decode("truncated strip 12".to_owned())),
    ));
    results.push((
        PathBuf::from("/photos/converted/old.jpg"),
        Outcome::Failed(Failure::SameAsSource(PathBuf::from(
            "/photos/converted/old.jpg",
        ))),
    ));
    Report {
        results,
        cancelled: false,
        elapsed: Duration::from_secs(9),
    }
}

fn running_view() -> DialogView {
    let p = Progress {
        done: 7,
        total: 24,
        failed: 0,
        skipped: 0,
        elapsed: Duration::from_secs(4),
        eta: Some(Duration::from_secs(11)),
    };
    DialogView {
        phase: PhaseView::Running {
            status: progress_text(&p),
            progress: 7.0 / 24.0,
            stopping: false,
        },
        // On Hide (Stop comes first).
        focus: Some(1),
        export: false,
        delete_preset: None,
    }
}

fn done_view() -> DialogView {
    let (summary, lines) = report_lines(&report());
    DialogView {
        phase: PhaseView::Done { summary, lines },
        focus: Some(0),
        export: false,
        delete_preset: None,
    }
}

#[test]
fn convert_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-convert-dialog",
    );
    let c = choices();
    let pal = Palette::dark();
    let mut shots = Vec::new();
    for (lang, rtl) in [("en", false), ("he", true)] {
        let id = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        specular_i18n::I18n::with_thread_language(id, || {
            // Focus on the name field, so its caret and ring show.
            let mut v = setup_view(&c, "{name}-{w}x{h}", None);
            v.focus = v.focusables().iter().position(|h| *h == Hit::Name);
            shots.push((format!("setup-{lang}"), v, rtl));
            if rtl {
                // A Hebrew name with digits and Latin, the caret inside the Hebrew word and
                // part of it selected: caret and selection where the letters are.
                let pattern = "{name}-\u{5e9}\u{5dc}\u{5d5}\u{5dd}-12";
                let mut v = setup_view(&c, pattern, None);
                v.focus = v.focusables().iter().position(|h| *h == Hit::Name);
                if let PhaseView::Setup {
                    caret, selection, ..
                } = &mut v.phase
                {
                    let at = pattern.find('\u{5dc}').expect("lamed");
                    *caret = at;
                    *selection = (at, at + 4);
                }
                shots.push(("hebrew-name-he".to_owned(), v, true));
            }
            if !rtl {
                let mut bad = setup_view(&c, "{name}-{size}", Some(1));
                bad.focus = Some(1);
                shots.push(("pattern-error-en".to_owned(), bad, false));
                // The tallest start column: lossy JPEG XL fitted within a box.
                let mut jxl = c.clone();
                jxl.s.format = ConvertFormat::Jxl;
                jxl.s.jxl_lossless = false;
                jxl.s.resize = ConvertResize::Fit;
                shots.push((
                    "setup-jxl-en".to_owned(),
                    setup_view(&jxl, "{name}", None),
                    false,
                ));
                shots.push(("running-en".to_owned(), running_view(), false));
                shots.push(("done-en".to_owned(), done_view(), false));
            }
        });
    }
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
                    drawn = Some(draw(dl, text, &pal, v, SIZE, *rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("convert-dialog-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("convert-dialog-{name}-{suffix}"),
                &tree,
                *rtl,
            );
        }
    }
    goldens.finish();
}

/// Every control is inside the sheet, the controls do not overlap, and right-to-left mirrors
/// them.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let pal = Palette::dark();
    let mut c = choices();
    // The most rows the start column shows: JPEG's two, the depth-less size with Fit's two.
    c.s.resize = ConvertResize::Fit;
    for v in [
        setup_view(&c, "{name}", Some(0)),
        running_view(),
        done_view(),
    ] {
        let mut ltr = None;
        let mut rtl = None;
        h.render(SIZE, 1.0, backdrop, |dl, text| {
            ltr = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
            rtl = Some(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        });
        let (l, r) = (ltr.unwrap(), rtl.unwrap());
        assert_eq!(l.sheet, r.sheet);
        let s = l.sheet;
        let controls: Vec<_> = l
            .hits
            .iter()
            .filter(|(_, h)| !matches!(h, Hit::Row(_)))
            .collect();
        for (i, (a, ha)) in controls.iter().enumerate() {
            assert!(
                a.x >= s.x && a.right() <= s.right() && a.y >= s.y && a.bottom() <= s.bottom(),
                "{ha:?} {a:?} is outside {s:?}"
            );
            for (b, hb) in &controls[i + 1..] {
                let apart =
                    a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
        for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
            assert_eq!(ha, hb);
            assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
        }
        // Every focusable control can be clicked.
        for f in v.focusables() {
            assert!(l.hits.iter().any(|(_, h)| *h == f), "{f:?} has no rect");
        }
    }
}

#[test]
fn choices_step_and_follow_the_format() {
    let mut c = choices();
    assert_eq!(
        c.start_rows(),
        [
            Row::Images,
            Row::Format,
            Row::Quality,
            Row::Chroma,
            Row::Resize,
            Row::LongEdge,
            Row::Upscale
        ]
    );
    assert!(c.step(Row::Quality, 1));
    assert_eq!(c.s.jpeg_quality, 90);
    c.s.jpeg_quality = 100;
    assert!(!c.step(Row::Quality, 1), "quality stops at 100");
    c.s.jpeg_quality = 3;
    assert!(c.step(Row::Quality, -1));
    assert_eq!(c.s.jpeg_quality, 1);
    assert!(c.step(Row::LongEdge, 1));
    assert_eq!(c.s.long_edge, 2160);
    assert!(c.step(Row::LongEdge, -1));
    assert!(c.step(Row::LongEdge, -1));
    assert_eq!(c.s.long_edge, 1920);
    c.s.long_edge = 8192;
    assert!(!c.step(Row::LongEdge, 1));
    // PNG: effort and bit depth instead of quality.
    assert!(c.step(Row::Format, 1));
    assert_eq!(c.s.format, ConvertFormat::Png);
    let rows = c.start_rows();
    assert!(rows.contains(&Row::Effort) && rows.contains(&Row::Depth));
    assert!(!rows.contains(&Row::Quality));
    c.s.png_effort = 1;
    assert!(!c.step(Row::Effort, -1));
    assert!(c.step(Row::Effort, 1));
    assert_eq!(c.s.png_effort, 2);
    // WebP: lossless (no effort) above quality 100, and no 16 bits.
    c.step(Row::Format, 1);
    assert_eq!(c.s.format, ConvertFormat::Webp);
    assert!(c.s.webp_lossless);
    let rows = c.start_rows();
    assert!(rows.contains(&Row::Quality) && !rows.contains(&Row::Effort));
    assert!(!rows.contains(&Row::Depth));
    assert_eq!(c.options().webp_quality, None);
    assert!(!c.step(Row::Quality, 1), "lossless is the top");
    assert!(c.step(Row::Quality, -1));
    assert!(!c.s.webp_lossless);
    assert_eq!(c.s.webp_quality, 100);
    assert!(c.start_rows().contains(&Row::Effort));
    assert!(c.step(Row::Quality, -1));
    assert_eq!(c.options().webp_quality, Some(95));
    assert!(c.step(Row::Effort, 1));
    assert_eq!(c.options().webp_effort, 6);
    assert!(c.step(Row::Quality, 1));
    assert!(c.step(Row::Quality, 1));
    assert!(c.s.webp_lossless);
    // JPEG XL: the same quality row, its own effort, and 16 bits.
    c.step(Row::Format, 1);
    assert_eq!(c.s.format, ConvertFormat::Jxl);
    let rows = c.start_rows();
    assert!(rows.contains(&Row::Quality) && rows.contains(&Row::Effort));
    assert!(rows.contains(&Row::Depth));
    assert!(c.step(Row::Quality, -1));
    assert_eq!(c.options().jxl_quality, Some(100));
    assert!(c.step(Row::Effort, 1));
    assert_eq!(c.options().jxl_effort, 2);
    // AVIF: quality and effort, 8 bits.
    c.step(Row::Format, 1);
    assert_eq!(c.s.format, ConvertFormat::Avif);
    let rows = c.start_rows();
    assert!(rows.contains(&Row::Quality) && rows.contains(&Row::Effort));
    assert!(!rows.contains(&Row::Depth));
    assert!(c.step(Row::Quality, 1));
    assert_eq!(c.options().avif_quality, 85);
    assert!(!c.step(Row::Effort, -1), "effort 1 is the fastest");
    // Every format and size fits the start column.
    for f in ConvertFormat::ALL {
        for r in ConvertResize::ALL {
            c.s.format = *f;
            c.s.resize = *r;
            assert!(c.start_rows().len() <= START_ROWS);
        }
    }
    // The folder cycles subfolder → same → (chosen once picked) and keeps the name.
    assert_eq!(c.folder_mode(), FolderMode::Subfolder);
    assert!(c.step(Row::Folder, 1));
    assert_eq!(c.folder_mode(), FolderMode::Same);
    assert!(c.step(Row::Folder, 1));
    assert_eq!(c.folder_mode(), FolderMode::Subfolder);
    assert_eq!(c.s.subfolder, "converted");
    c.choose(PathBuf::from("/out/web"));
    assert_eq!(c.folder_mode(), FolderMode::Chosen);
    assert_eq!(
        c.options().output,
        OutputDir::Folder(PathBuf::from("/out/web"))
    );
    c.step(Row::Folder, 1);
    assert_eq!(c.folder_mode(), FolderMode::Subfolder);
    c.step(Row::Folder, -1);
    assert_eq!(c.folder_mode(), FolderMode::Chosen);
}

#[test]
fn options_follow_the_choices() {
    let c = choices();
    let o = c.options();
    assert_eq!(o.format, Format::Jpeg);
    assert_eq!(o.jpeg_quality, 85);
    assert_eq!(o.resize, Resize::LongEdge(2048));
    assert_eq!(o.metadata, MetaMode::NoLocation);
    assert_eq!(o.output, OutputDir::Subfolder("converted".to_owned()));
    assert!(!o.upscale);
}

#[test]
fn preview_shows_the_new_name_or_the_error() {
    let c = choices();
    let first = Path::new("/photos/IMG_1.heic");
    let probe = Some(((4000, 3000), Some([2026, 7, 14, 18, 3, 9]), None));
    let ok = preview(&c, "{name}-{w}x{h}", Some(first), probe.clone()).unwrap();
    let sep = std::path::MAIN_SEPARATOR;
    assert!(
        ok.contains(&format!("converted{sep}IMG_1-2048x1536.jpg")),
        "{ok}"
    );
    let dated = preview(&c, "{date:%Y-%m-%d}_{n:03}", Some(first), probe.clone()).unwrap();
    assert!(dated.contains("2026-07-14_001.jpg"), "{dated}");
    let err = preview(&c, "{name}-{size}", Some(first), probe.clone()).unwrap_err();
    assert!(err.contains("{size}"), "{err}");
    assert!(preview(&c, "{name", Some(first), probe.clone()).is_err());
    assert!(preview(&c, "", Some(first), probe.clone()).is_err());
    // The same folder: just the name.
    let mut same = c.clone();
    same.step(Row::Folder, 1);
    let ok = preview(&same, "{name}", Some(first), probe.clone()).unwrap();
    assert!(!ok.contains(sep), "{ok}");
}

#[test]
fn report_lists_failures_and_counts() {
    let (summary, lines) = report_lines(&report());
    assert!(summary.contains("17"), "{summary}");
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(lines[2].contains("scan.tif"));
    let mut many = report();
    for i in 0..12 {
        many.results.push((
            PathBuf::from(format!("/x/{i}.png")),
            Outcome::Failed(Failure::Write("disk full".to_owned())),
        ));
    }
    let (_, lines) = report_lines(&many);
    // Skipped, failed, eight failures, "and 6 more".
    assert_eq!(lines.len(), 2 + MAX_FAILURES + 1);
    assert!(lines.last().unwrap().contains('6'));
    assert!(eta_text(Duration::from_secs(11)).contains("15"));
    assert!(eta_text(Duration::from_secs(150)).contains('3'));
}

#[test]
fn a11y_nodes_map_back_to_controls() {
    let Some(mut h) = Harness::new() else { return };
    let c = choices();
    let mut v = setup_view(&c, "{name}", None);
    v.focus = v.focusables().iter().position(|h| *h == Hit::Name);
    let pal = Palette::dark();
    let mut d = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        d = Some(draw(dl, text, &pal, &v, SIZE, false, 1.0));
    });
    let d = d.unwrap();
    let mut tree = accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    };
    a11y(&mut tree, &v, &d, 1.0);
    assert_eq!(a11y_target(&v, tree.focus.0), Some(Hit::Name));
    let focused = tree.nodes.iter().find(|(id, _)| *id == tree.focus).unwrap();
    assert_eq!(focused.1.role(), accesskit::Role::TextInput);
    let spin = tree
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == accesskit::Role::SpinButton)
        .count();
    assert_eq!(spin, c.start_rows().len() + c.end_rows().len());
    assert_eq!(a11y_target(&v, A11Y_BASE + 1), Some(Hit::Row(Row::Images)));
    assert_eq!(tree.nodes[0].1.children(), &[accesskit::NodeId(A11Y_BASE)]);
    // Every node's id is unique.
    let mut ids: Vec<_> = tree.nodes.iter().map(|(id, _)| id.0).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), tree.nodes.len());
}

/// A long value widens its column's steppers as far as the longest label still fits beside
/// them; short values keep the usual width.
#[test]
fn long_values_widen_the_steppers_up_to_the_labels() {
    let (col, arrow) = (378.0, 26.0);
    let base = (col * 0.56f32).min(220.0);
    assert_eq!(stepper_width(col, arrow, 120.0, 90.0), base);
    // 190 px of value: 190 + 2 × (26 + 6).
    assert_eq!(stepper_width(col, arrow, 80.0, 190.0), 254.0);
    // Longer still: up to the labels (the rest is cut with "…").
    assert_eq!(
        stepper_width(col, arrow, 80.0, 400.0),
        col - 80.0 - LABEL_GAP
    );
    // Labels taking more than their share leave the usual width.
    assert_eq!(stepper_width(col, arrow, 300.0, 400.0), base);
}

/// An input method composing in the name field (synthetic preedit): shown at the caret, the
/// candidate window's area spans it inside the field; nothing composed, it is the caret.
#[test]
fn name_field_shows_a_composition_at_the_caret() {
    let c = choices();
    let pal = Palette::dark();
    let mut text = specular_gfx::Text::bundled_only();
    let probe = setup_view(&c, "{name}", None);
    let at = probe.focusables().iter().position(|h| *h == Hit::Name);
    let mut v = setup_view(&c, "{name}", at);
    let mut dl = DrawList::new(1.0);
    let plain = draw(&mut dl, &mut text, &pal, &v, SIZE, false, 1.0);
    let field = plain.field.unwrap();
    let caret = plain
        .ime
        .expect("an IME area while the field has the keyboard");
    assert!(caret.w < 4.0, "{caret:?}");
    if let PhaseView::Setup { preedit, .. } = &mut v.phase {
        *preedit = "なまえ".into();
    }
    let mut dl = DrawList::new(1.0);
    let d = draw(&mut dl, &mut text, &pal, &v, SIZE, false, 1.0);
    let area = d.ime.unwrap();
    assert!(
        area.w > 20.0 && area.x >= caret.x - 1.0,
        "{area:?} vs {caret:?}"
    );
    assert!(
        area.right() <= field.right() && area.x >= field.x,
        "{area:?} in {field:?}"
    );
    assert!(d.field_text.is_none());
}
