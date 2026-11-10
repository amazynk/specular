//! Screenshot tests of the annotation export dialog at 1× and 1.5×: GeoJSON, an OME-TIFF
//! label pyramid of the whole slide, PNG tiles of the last drawn region on the light
//! palette, an export under way, and the dialog in Hebrew (right to left). Rendered
//! offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular annot_export::shots`, then review
//! the rewritten PNGs.

use specular_pathology::annot::{Class, Geometry};
use specular_shots::{Goldens, Harness};

use super::*;

const SIZE: (f32, f32) = (720.0, 560.0);
const SCALES: [(f32, &str); 2] = [(1.0, "1x"), (1.5, "1.5x")];
const LEVELS: [(u32, u32); 4] = [
    (98_304, 71_680),
    (24_576, 17_920),
    (6_144, 4_480),
    (1_536, 1_120),
];

/// What stands for a slide under the scrim.
fn backdrop(dl: &mut DrawList) {
    dl.rect(
        Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
        0.0,
        Color::rgba(0.93, 0.9, 0.92, 1.0),
    );
    for (x, y, r) in [
        (160.0, 140.0, 90.0),
        (420.0, 300.0, 140.0),
        (600.0, 120.0, 60.0),
    ] {
        dl.rect(
            Rect::new(x - r, y - r, 2.0 * r, 2.0 * r),
            r,
            Color::rgba(0.86, 0.62, 0.76, 1.0),
        );
    }
}

fn items() -> Vec<Annotation> {
    let class = |n: &str, c: [u8; 3]| {
        Some(Class {
            name: n.into(),
            colour: Some(c),
        })
    };
    let mut v = Vec::new();
    for i in 0..9 {
        let x = f64::from(i) * 400.0;
        v.push(Annotation {
            name: String::new(),
            class: class(if i % 3 == 0 { "Tumor" } else { "Stroma" }, [200, 0, 0]),
            geometry: Geometry::rect((x, 0.0), (x + 300.0, 200.0)),
        });
    }
    v.push(Annotation {
        name: "nucleus".into(),
        class: class("Immune cells", [0, 0, 255]),
        geometry: Geometry::Points(vec![(10.0, 10.0), (20.0, 20.0)]),
    });
    v.push(Annotation::new(Geometry::Line(vec![
        (0.0, 0.0),
        (50.0, 50.0),
    ])));
    v
}

fn view(c: &Choices) -> DialogView {
    let (summary, note) = summary(&items());
    DialogView {
        title: t!("annot-export-title"),
        rows: row_views(c, &LEVELS),
        summary,
        note,
        status: None,
        busy: false,
        can_export: true,
        focus: Some(0),
    }
}

#[test]
fn export_dialog_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-annot-export-dialog",
    );
    let geo = view(&Choices::default());
    let ome = Choices {
        format: Format::Masks,
        output: Output::Ome,
        level: 1,
        ..Choices::default()
    };
    let mut ome_v = view(&ome);
    ome_v.focus = Some(3);
    let png = Choices {
        format: Format::Masks,
        output: Output::Png,
        area: Area::Drawn,
        tile: 2,
        skip_empty: false,
        ..Choices::default()
    };
    let png_v = view(&png);
    let mut running = view(&ome);
    running.busy = true;
    running.can_export = false;
    running.status = Some(t!("annot-export-writing"));
    running.focus = Some(running.rows.len());
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let ome_he = specular_i18n::I18n::with_thread_language(he.clone(), || view(&ome));
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    for (scale, suffix) in SCALES {
        for (name, v, pal, rtl) in [
            ("geojson", &geo, &pal_dark, false),
            ("ome", &ome_v, &pal_dark, false),
            ("png-light", &png_v, &pal_light, false),
            ("running", &running, &pal_dark, false),
            ("ome-he", &ome_he, &pal_dark, true),
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
            let n = format!("annot-export-{name}-{suffix}");
            goldens.check(&n, &shot);
            super::super::a11y_audit::expect_clean(&n, &tree, rtl);
        }
    }
    goldens.finish();
}

/// Every control is inside the sheet, nothing overlaps, and right to left mirrors.
#[test]
fn controls_sit_in_the_sheet_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let c = Choices {
        format: Format::Masks,
        output: Output::Tiff,
        ..Choices::default()
    };
    let mut v = view(&c);
    v.status = Some(t!("annot-export-writing"));
    let pal = Palette::dark();
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, SIZE, false, 1.0));
        out.push(draw(dl, text, &pal, &v, SIZE, true, 1.0));
        out.push(draw(dl, text, &pal, &v, (SIZE.0, 420.0), false, 1.6));
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
    assert_eq!(v.focusables()[v.rows.len()..], [Hit::Cancel, Hit::Export]);
    // Six rows for tiles, five for the pyramid (no empty-tile row), one for GeoJSON.
    assert_eq!(c.rows().len(), 6);
    assert_eq!(
        Choices {
            output: Output::Ome,
            ..c.clone()
        }
        .rows()
        .len(),
        5
    );
    assert_eq!(Choices::default().rows(), [Row::Format]);
}

#[test]
fn choices_step_and_wrap() {
    let mut c = Choices::default();
    assert!(c.step(Row::Format, 1, 4, false));
    assert_eq!(c.format, Format::Masks);
    assert!(c.step(Row::Format, 1, 4, false));
    assert_eq!(c.format, Format::Coco);
    // Without a drawn region the region skips "last drawn".
    c.area = Area::View;
    c.step(Row::Region, 1, 4, false);
    assert_eq!(c.area, Area::Slide);
    c.step(Row::Region, -1, 4, true);
    assert_eq!(c.area, Area::Drawn);
    c.step(Row::Level, -1, 4, false);
    assert_eq!(c.level, 3);
    assert!(!Choices::default().clone().step(Row::Level, 1, 1, false));
}

#[test]
fn jobs_write_every_format() {
    let dir = std::env::temp_dir().join(format!("specular-annot-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut job = Job {
        items: items(),
        choices: Choices::default(),
        size: (4000, 300),
        region: Region {
            x: 0.0,
            y: 0.0,
            w: 4000.0,
            h: 300.0,
        },
        downsample: 4.0,
        file: "slide.svs".into(),
        stem: "slide".into(),
        pixel_um: Some(0.25),
    };
    assert_eq!(write(&job, &dir.join("a.geojson")), Ok(1));
    job.choices.format = Format::Coco;
    assert_eq!(write(&job, &dir.join("a.json")), Ok(1));
    let back = coco::read(&std::fs::read_to_string(dir.join("a.json")).unwrap()).unwrap();
    assert_eq!(back.items.len(), 10);
    job.choices.format = Format::Masks;
    job.choices.output = Output::Ome;
    assert_eq!(write(&job, &dir.join("a.ome.tif")), Ok(1));
    job.choices.output = Output::Png;
    job.choices.tile = 0;
    let tiles = dir.join("tiles");
    std::fs::create_dir_all(&tiles).unwrap();
    // 1000 × 75 at 1/4 in 256 tiles: 4 × 1, all labelled, and the class table.
    assert_eq!(write(&job, &tiles), Ok(5));
    std::fs::remove_dir_all(&dir).unwrap();
}
