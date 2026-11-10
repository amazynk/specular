//! Screenshot tests of the figure and key-image dialogs at 1× and 1.5×: the figure dialog
//! (dark, and in Hebrew right to left), the key-image dialog (light), the teaching file (and
//! in Hebrew), the "Key image" badge, and a figure as the raster writer draws it (scale bar,
//! colour bar, inset, measurement, provenance). Each dialog passes the screen-reader audit.
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular figure::shots`, then review the
//! rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness, Shot};

const SIZE: (f32, f32) = (760.0, 680.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];

/// What stands for a micrograph under the scrim.
fn backdrop(dl: &mut DrawList) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, Color::BLACK);
    dl.rect(
        Rect::new(120.0, 90.0, 520.0, 480.0),
        24.0,
        Color::rgba(0.10, 0.32, 0.18, 1.0),
    );
    dl.rect(
        Rect::new(300.0, 220.0, 160.0, 160.0),
        80.0,
        Color::rgba(0.25, 0.75, 0.42, 1.0),
    );
}

fn avail() -> Avail {
    Avail {
        calibration: Some(App::measure_source_label(
            specular_measure::calib::Source::OmePhysicalSize,
        )),
        legend: Some(t!(
            "figure-legend-map",
            map = "viridis",
            lo = "0",
            hi = "4095",
            unit = ""
        )),
        inset: true,
        split: true,
        marks: 2,
        full_res: true,
        zoomed_in: true,
        medical: false,
        keys: 0,
    }
}

fn figure_view(focus: usize) -> DialogView {
    let c = FigureChoices {
        format: Format::Pdf,
        inset: true,
        split: true,
        title: t!("figure-channels-merged"),
        ..FigureChoices::default()
    };
    DialogView {
        title: t!("figure-dialog-title"),
        rows: figure_rows(
            &c,
            &avail(),
            Some(Path::new("/home/user/Figures/retina-figure.pdf")),
        ),
        note: None,
        status: None,
        busy: false,
        can_export: true,
        focus: Some(focus),
        preedit: String::new(),
    }
}

fn keys_view(teaching: bool, busy: bool) -> DialogView {
    let a = Avail {
        medical: teaching,
        keys: 4,
        ..Avail::default()
    };
    let c = KeyChoices {
        teaching,
        notes: if teaching {
            t!("figure-keys-notes")
        } else {
            String::new()
        },
        ..KeyChoices::default()
    };
    DialogView {
        title: if teaching {
            t!("figure-teaching-title")
        } else {
            t!("figure-keys-title")
        },
        rows: key_rows(&c, &a, None),
        note: teaching.then(|| t!("figure-keys-teaching-note")),
        status: busy.then(|| t!("figure-working")),
        busy,
        can_export: true,
        focus: Some(if teaching { 3 } else { 0 }),
        preedit: String::new(),
    }
}

#[test]
fn dialogs_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    // The file's own name, not a message.
    h.data_text("retina-figure.pdf");
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-figure-dialog",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        let cases: Vec<(&str, DialogView, &Palette, bool)> = vec![
            ("figure", figure_view(0), &pal_dark, false),
            ("keys-light", keys_view(false, false), &pal_light, false),
            ("teaching", keys_view(true, false), &pal_dark, false),
            (
                "figure-he",
                specular_i18n::I18n::with_thread_language(he.clone(), || figure_view(8)),
                &pal_dark,
                true,
            ),
            (
                "teaching-he",
                specular_i18n::I18n::with_thread_language(he.clone(), || keys_view(true, true)),
                &pal_light,
                true,
            ),
        ];
        for (name, v, pal, rtl) in cases {
            let lang = if name.contains("-he") { &he } else { &en };
            let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let mut drawn = None;
                let shot = h.render(SIZE, scale, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, pal, &v, SIZE, rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
                (shot, tree)
            });
            goldens.check(&format!("figure-dialog-{name}-{suffix}"), &shot);
            super::super::a11y_audit::expect_clean(
                &format!("figure-dialog-{name}-{suffix}"),
                &tree,
                rtl,
            );
        }
        let shot = h.render(SIZE, scale, backdrop, |dl, text| {
            let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
            draw_badge(dl, text, &pal_dark, area, false, 1.0);
        });
        goldens.check(&format!("figure-badge-{suffix}"), &shot);
    }
    goldens.finish();
}

/// Every control is inside the sheet, nothing overlaps, and right to left mirrors.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let v = figure_view(0);
    let pal = Palette::dark();
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, (SIZE.0, 460.0), false, 1.6));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.sheet, r.sheet);
    for d in [&out[0], &out[2]] {
        let s = d.sheet;
        let controls: Vec<_> = d
            .hits
            .iter()
            .filter(|(_, h)| !matches!(h, Hit::Row(_)))
            .collect();
        for (i, (a, ha)) in controls.iter().enumerate() {
            assert!(
                a.x >= s.x && a.right() <= s.right() && a.y >= s.y && a.bottom() <= s.bottom(),
                "{ha:?} {a:?} outside {s:?}"
            );
            for (b, hb) in &controls[i + 1..] {
                let apart =
                    a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {b:?}");
            }
        }
    }
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
    // Steppers only where the part is available.
    for rv in &v.rows {
        let steps = l.hits.iter().any(|(_, h)| *h == Hit::Inc(rv.row));
        assert_eq!(
            steps,
            rv.kind == RowKind::Step && rv.enabled,
            "{:?}",
            rv.row
        );
    }
}

/// A figure as written to PNG: the panels, the scale bar, the colour bar, the inset with its
/// outline, a measurement and the provenance, in the viewer's fonts.
#[test]
fn figure_output_matches_golden() {
    use specular_figure::compose::{
        InsetSpec, LegendSpec, Mark, Panel, ScaleSpec, Spec, compose, inset_size,
    };
    use specular_figure::{El, Rgb16};
    let Some(mut h) = Harness::new() else { return };
    let (w, hh) = (360u32, 260u32);
    let cell = |x: u32, y: u32| -> [u8; 3] {
        let dx = x as f32 - 180.0;
        let dy = y as f32 - 120.0;
        let r = (dx * dx + dy * dy).sqrt();
        let v = (128.0 + 110.0 * (r / 9.0).sin() * (-(r / 140.0)).exp()) as u8;
        [v / 3, v, v / 2]
    };
    let rgb: Vec<u8> = (0..hh)
        .flat_map(|y| (0..w).flat_map(move |x| cell(x, y)))
        .collect();
    let main = Rgb16::from_rgb8(w, hh, &rgb);
    let region = [150.0, 90.0, 60.0, 60.0];
    let (iw, ih) = inset_size((w, hh), region);
    let mut inset = Vec::with_capacity((iw * ih * 3) as usize);
    for y in 0..ih {
        for x in 0..iw {
            let sx = 150 + x * 60 / iw;
            let sy = 90 + y * 60 / ih;
            inset.extend(cell(sx, sy));
        }
    }
    let spec = Spec {
        title: Some("Sample 12, cortex".into()),
        main: Some(Panel {
            px: main,
            label: None,
        }),
        inset: Some(InsetSpec {
            px: Rgb16::from_rgb8(iw, ih, &inset),
            region,
        }),
        scale: Some(ScaleSpec {
            per_px: 0.65,
            unit: "µm".into(),
            metres: Some(1e-6),
            approximate: false,
        }),
        legend: Some(LegendSpec {
            ramp: build::ramp(|t| specular_inspect::ColourMap::Viridis.sample(t)),
            lo: 0.0,
            hi: 4095.0,
            unit: "counts".into(),
        }),
        marks: vec![Mark {
            path: vec![El::Move([40.0, 200.0]), El::Line([130.0, 170.0])],
            closed: false,
            label: Some("61.7 µm".into()),
        }],
        meta: vec![("file".into(), "cortex.ome.tif".into())],
        channels: Vec::new(),
        caption: vec![
            "cortex.ome.tif".into(),
            "Calibration: OME physical size: 0.65 × 0.65 µm/px".into(),
        ],
        deep: true,
    };
    let f = compose(&spec, &mut h.text).unwrap();
    let px = specular_figure::raster::render(&f, 1.0);
    let shot = Shot {
        width: px.width,
        height: px.height,
        rgb: px.to_rgb8(),
        language: "en-US".into(),
        strays: Vec::new(),
        signs: Vec::new(),
    };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-figure-output",
    );
    goldens.check("figure-output", &shot);
    goldens.finish();
}
