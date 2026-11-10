//! Screenshot tests of the measurement tool: its bar, the list with values, statistics and
//! source badges, and the shapes over an image; left to right, in Hebrew (right to left) and
//! at text scale 2. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular measure_tool::shots`, then review
//! the rewritten PNGs.

use super::*;
use specular_shots::{Goldens, Harness};

const SIZE: (f32, f32) = (900.0, 640.0);

/// What stands for an image: a dark field with a lighter disc.
fn backdrop(dl: &mut DrawList) {
    dl.rect(Rect::new(0.0, 0.0, SIZE.0, SIZE.1), 0.0, Color::BLACK);
    dl.rect(
        Rect::new(80.0, 60.0, 560.0, 520.0),
        0.0,
        Color::rgba(0.25, 0.27, 0.3, 1.0),
    );
    dl.rect(
        Rect::new(220.0, 240.0, 220.0, 220.0),
        110.0,
        Color::rgba(0.6, 0.6, 0.62, 1.0),
    );
}

/// Stored pixels are screen pixels here (zoom 1, no pan).
fn screen(pts: &[P]) -> Vec<(f32, f32)> {
    pts.iter().map(|p| (p.x as f32, p.y as f32)).collect()
}

/// A view with a line, a rectangle with statistics, an ellipse still being read, an angle,
/// and a polygon being drawn; the strings in the thread's language.
fn sample() -> MeasureView {
    let cal = Calibration::dicom(0.25, 0.25, Source::DicomPixelSpacing).unwrap();
    let user = Calibration::dicom(0.1, 0.1, Source::User).unwrap();
    let line = Measurement::new(Shape::Line(P::new(120.0, 120.0), P::new(400.0, 180.0)), cal);
    let mut rect = Measurement::new(
        Shape::Rect {
            x0: 240.0,
            y0: 260.0,
            x1: 420.0,
            y1: 380.0,
        },
        cal,
    );
    let st = ChannelStats {
        count: 21600,
        sum: 21600.0 * 153.2,
        mean: 153.2,
        sd: 12.71,
        min: 98.0,
        max: 201.0,
        sd_sample: 12.71,
        median: 152.0,
        histogram: specular_measure::stats::Histogram {
            lo: 98.0,
            width: 1.0,
            bins: vec![1; 104],
        },
        nan: 0,
    };
    rect.stats = Some(vec![st]);
    rect.channels = vec!["Gray".into()];
    let ellipse = Measurement::new(
        Shape::ellipse_in(P::new(460.0, 300.0), P::new(600.0, 420.0)),
        user,
    );
    let angle = Measurement::new(
        Shape::Angle {
            a: P::new(140.0, 520.0),
            vertex: P::new(220.0, 470.0),
            b: P::new(330.0, 540.0),
        },
        Calibration::uncalibrated(),
    );
    let items = [&line, &rect, &ellipse, &angle]
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let detail = match &m.stats {
                Some(st) => stats_lines(st, &m.channels),
                None if m.shape.is_area() => vec![t!("measure-tool-stats-pending")],
                None => Vec::new(),
            };
            let pts = match &m.shape {
                Shape::Angle { a, vertex, b } => vec![*a, *vertex, *b],
                s => s.outline(),
            };
            ItemView {
                title: t!(
                    "measure-tool-item",
                    n = i as i64 + 1,
                    shape = shape_name(&m.shape)
                ),
                value: value_text(m, None),
                detail,
                badge: source_label(m.cal.source),
                caveat: m.cal.source.is_approximate(),
                outline: screen(&pts),
                closed: m.shape.is_area(),
                handles: Vec::new(),
                guides: Vec::new(),
                plot: Vec::new(),
            }
        })
        .collect();
    let marks = [
        P::new(470.0, 470.0),
        P::new(560.0, 450.0),
        P::new(590.0, 530.0),
    ];
    let mut drawing = screen(&marks);
    drawing.push((500.0, 560.0));
    MeasureView {
        open: true,
        kind: Kind::Polygon,
        items,
        selected: Some(1),
        drawing,
        drawing_closed: false,
        marks: screen(&marks),
        cursor: Some((500.0, 560.0)),
        status: Kind::Polygon.hint(),
        prompt: None,
        can_undo: true,
        focus: Some(8),
        spot: false,
        geometry: false,
        template: Template::Vhs,
        probe: specular_measure::inspect::Probe::Profile,
    }
}

/// A drawing sheet's regions: viewport 1 at 1:50 in metres (PDF number formats with an
/// area label), viewport 2 at 1 in = 4 ft in feet and eighths of an inch.
fn drawing_regions() -> Vec<ScaleRegion> {
    use specular_measure::scale::{Fraction, NumberFormat};
    let m = Calibration::spacing(0.02, 0.02, Unit::M, Source::DrawingScale).unwrap();
    let ft = Calibration::spacing(0.05, 0.05, Unit::Ft, Source::DrawingScale).unwrap();
    vec![
        ScaleRegion {
            bounds: Some([80.0, 60.0, 400.0, 520.0]),
            viewport: Some(1),
            name: Some("Plan".into()),
            ratio: Some("1:50".into()),
            cal: m,
            distance: vec![NumberFormat::new("m", 1.0)],
            area: vec![NumberFormat::new("sq m", 1.0)],
        },
        ScaleRegion {
            bounds: Some([400.0, 60.0, 640.0, 520.0]),
            viewport: Some(2),
            name: None,
            ratio: Some("1 in = 4 ft".into()),
            cal: ft,
            distance: vec![
                NumberFormat::new("ft", 1.0),
                NumberFormat {
                    fraction: Fraction::Fraction,
                    precision: 8,
                    ..NumberFormat::new("in", 12.0)
                },
            ],
            area: Vec::new(),
        },
    ]
}

/// A drawing measured through its own scale: a line and a rectangle in viewport 1, a line
/// in viewport 2, and one after the user's own calibration.
fn sample_drawing() -> MeasureView {
    let regions = drawing_regions();
    let line = Measurement::new(
        Shape::Line(P::new(120.0, 120.0), P::new(370.0, 120.0)),
        regions[0].cal,
    );
    let rect = Measurement::new(
        Shape::Rect {
            x0: 140.0,
            y0: 200.0,
            x1: 340.0,
            y1: 300.0,
        },
        regions[0].cal,
    );
    let detail = Measurement::new(
        Shape::Line(P::new(430.0, 200.0), P::new(535.83, 200.0)),
        regions[1].cal,
    );
    let user = Measurement::new(
        Shape::Line(P::new(120.0, 440.0), P::new(380.0, 440.0)),
        Calibration::spacing(0.0205, 0.0205, Unit::M, Source::User).unwrap(),
    );
    let items = [
        (&line, Some(&regions[0])),
        (&rect, Some(&regions[0])),
        (&detail, Some(&regions[1])),
        (&user, None),
    ]
    .iter()
    .enumerate()
    .map(|(i, (m, r))| ItemView {
        title: t!(
            "measure-tool-item",
            n = i as i64 + 1,
            shape = shape_name(&m.shape)
        ),
        value: value_text(m, *r),
        detail: Vec::new(),
        badge: badge(m.cal.source, *r),
        caveat: false,
        outline: screen(&m.shape.outline()),
        closed: m.shape.is_area(),
        handles: Vec::new(),
        guides: Vec::new(),
        plot: Vec::new(),
    })
    .collect();
    MeasureView {
        open: true,
        kind: Kind::Line,
        items,
        selected: Some(2),
        drawing: Vec::new(),
        drawing_closed: false,
        marks: Vec::new(),
        cursor: None,
        status: Kind::Line.hint(),
        prompt: None,
        can_undo: true,
        focus: None,
        spot: false,
        geometry: false,
        template: Template::Vhs,
        probe: specular_measure::inspect::Probe::Profile,
    }
}

#[test]
fn drawing_scale_values_are_written_as_drawn() {
    let regions = drawing_regions();
    let v = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
        sample_drawing()
    });
    assert_eq!(v.items[0].value, "5.00 m");
    let plain = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(
        plain(&v.items[0].badge),
        "From the drawing (scale 1:50, viewport 1)"
    );
    assert_eq!(plain(&v.items[1].value), "Area 8.00 sq m");
    assert_eq!(v.items[2].value, "5 ft 3 1/2 in");
    assert_eq!(
        plain(&v.items[2].badge),
        "From the drawing (scale 1 in = 4 ft, viewport 2)"
    );
    // The user's calibration overrides the drawing's.
    assert_eq!(plain(&v.items[3].badge), "Your calibration");
    assert_eq!(plain(&v.items[3].value), "5.330 m");
    // A region picks the smallest viewport under the shape.
    let ds = DrawingScale {
        regions,
        paper_mm_per_px: None,
        ratio: 50.0,
        unit: Unit::M,
    };
    assert_eq!(ds.region_at(450.0, 100.0), Some(1));
    assert_eq!(ds.region_at(10.0, 10.0), None);
}

#[test]
fn drawing_scale_readout_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-measure-tool",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let pal = Palette::dark();
    for (name, lang, rtl) in [("ltr", &en, false), ("he-rtl", &he, true)] {
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let v = sample_drawing();
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, &pal, &v, area, rtl, 1.0));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(&format!("measure-tool-drawing-scale-{name}"), &shot);
        super::super::a11y_audit::expect_clean(
            &format!("measure-tool-drawing-scale-{name}"),
            &tree,
            rtl,
        );
    }
    goldens.finish();
}

fn blank_tree() -> accesskit::TreeUpdate {
    accesskit::TreeUpdate {
        nodes: vec![(
            accesskit::NodeId(1),
            accesskit::Node::new(accesskit::Role::Window),
        )],
        tree: None,
        tree_id: accesskit::TreeId::ROOT,
        focus: accesskit::NodeId(1),
    }
}

#[test]
fn measure_tool_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-measure-tool",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let dark = Palette::dark();
    let light = Palette::light();
    for (name, lang, pal, rtl, ts) in [
        ("ltr", &en, &dark, false, 1.0),
        ("he-rtl", &he, &dark, true, 1.0),
        ("text-scale-2", &en, &light, false, 2.0),
    ] {
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let v = sample();
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, pal, &v, area, rtl, ts));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        goldens.check(&format!("measure-tool-{name}"), &shot);
        super::super::a11y_audit::expect_clean(&format!("measure-tool-{name}"), &tree, rtl);
    }
    goldens.finish();
}

/// Points for each geometry template, laid out roughly as on a radiograph or a lateral
/// cephalogram in the 900 × 640 view (screen = stored pixels).
fn demo_points(t: Template) -> Vec<P> {
    let p = |v: &[(f64, f64)]| v.iter().map(|&(x, y)| P::new(x, y)).collect::<Vec<_>>();
    let spine: Vec<(f64, f64)> = (0..7)
        .map(|i| (330.0 + 42.0 * f64::from(i), 150.0 - 4.0 * f64::from(i)))
        .collect();
    let rim = |cx: f64, cy: f64, r: f64| {
        [10.0f64, 130.0, 250.0].map(|d| {
            let a = d.to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
    };
    match t {
        Template::Vhs => {
            let mut v = vec![
                (300.0, 260.0),
                (430.0, 520.0),
                (250.0, 420.0),
                (470.0, 360.0),
            ];
            v.extend(&spine);
            p(&v)
        }
        Template::Vlas => {
            let mut v = vec![(300.0, 260.0), (420.0, 300.0)];
            v.extend(&spine);
            p(&v)
        }
        Template::Norberg => {
            let mut v = rim(250.0, 340.0, 45.0).to_vec();
            v.extend(rim(470.0, 340.0, 45.0));
            v.extend([(232.0, 275.0), (488.0, 276.0)]);
            p(&v)
        }
        Template::Tpa => p(&[
            (360.0, 200.0),
            (380.0, 560.0),
            (420.0, 205.0),
            (300.0, 230.0),
        ]),
        Template::Distraction => {
            let mut v = rim(330.0, 350.0, 60.0).to_vec();
            v.extend(rim(345.0, 340.0, 66.0));
            p(&v)
        }
        Template::FemoralHead => p(&rim(330.0, 330.0, 70.0)),
        Template::Offset => {
            let mut v = rim(270.0, 230.0, 55.0).to_vec();
            v.extend([(400.0, 330.0), (440.0, 560.0)]);
            p(&v)
        }
        Template::LegLength => p(&[
            (180.0, 260.0),
            (560.0, 270.0),
            (230.0, 420.0),
            (520.0, 400.0),
        ]),
        Template::Acetabular => p(&[(260.0, 420.0), (520.0, 425.0), (170.0, 300.0)]),
        Template::Steiner | Template::Downs => {
            let at = |k: &str| -> (f64, f64) {
                match k {
                    "s" => (230.0, 220.0),
                    "n" => (480.0, 190.0),
                    "a" => (480.0, 380.0),
                    "b" => (460.0, 500.0),
                    "pog" => (462.0, 545.0),
                    "go" => (250.0, 470.0),
                    "gn" => (450.0, 570.0),
                    "me" => (430.0, 580.0),
                    "u1-tip" => (492.0, 448.0),
                    "u1-apex" => (455.0, 375.0),
                    "l1-tip" => (485.0, 444.0),
                    "l1-apex" => (440.0, 515.0),
                    "occlusal-front" => (486.0, 446.0),
                    "occlusal-back" => (330.0, 420.0),
                    "po" => (170.0, 290.0),
                    _ => (440.0, 280.0),
                }
            };
            p(&t.landmarks().iter().map(|l| at(l.key)).collect::<Vec<_>>())
        }
    }
}

/// The tool open on a geometry template's finished measurement, its points asked for again.
fn sample_geometry(t: Template) -> MeasureView {
    let cal = Calibration::dicom(0.2, 0.2, Source::DicomPixelSpacing).unwrap();
    let mut m = Measurement::clinical(t, demo_points(t), cal);
    m.id = 1;
    let item = geometry::item(&m, |p| (p.x as f32, p.y as f32));
    MeasureView {
        open: true,
        kind: Kind::Geometry,
        items: vec![item],
        selected: Some(0),
        drawing: Vec::new(),
        drawing_closed: false,
        marks: Vec::new(),
        cursor: None,
        status: geometry::prompt(t, 0),
        prompt: None,
        can_undo: true,
        focus: None,
        spot: false,
        geometry: true,
        template: t,
        probe: specular_measure::inspect::Probe::Profile,
    }
}

#[test]
fn geometry_templates_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-measure-tool",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let pal = Palette::dark();
    for t in Template::ALL {
        for (name, lang, rtl) in [("ltr", &en, false), ("he-rtl", &he, true)] {
            let (shot, tree, v) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
                let v = sample_geometry(t);
                let mut drawn = None;
                let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                    drawn = Some(draw(dl, text, &pal, &v, area, rtl, 1.0));
                });
                let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
                a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
                (shot, tree, v)
            });
            // Every readout shows: no degenerate demo.
            assert!(!v.items[0].guides.is_empty(), "{t:?}");
            let id = format!("measure-geometry-{}-{name}", t.key());
            goldens.check(&id, &shot);
            super::super::a11y_audit::expect_clean(&id, &tree, rtl);
        }
    }
    goldens.finish();
}

/// The template button follows the tool and steps through the templates.
#[test]
fn geometry_controls() {
    let v = sample_geometry(Template::Norberg);
    let f = v.focusables();
    assert!(f.contains(&Hit::Tool(Kind::Geometry)) && f.contains(&Hit::Template));
    let mut off = sample();
    assert!(!off.focusables().contains(&Hit::Tool(Kind::Geometry)));
    off.geometry = true;
    assert!(off.focusables().contains(&Hit::Tool(Kind::Geometry)));
    assert!(!off.focusables().contains(&Hit::Template));
    let s = specular_i18n::I18n::with_thread_language(specular_i18n::langid!("en-US"), || {
        geometry::prompt(Template::Vhs, 4)
    });
    assert_eq!(
        s.replace(['\u{2068}', '\u{2069}'], ""),
        "Vertebral heart score (VHS), point 5 of 11: cranial end of T4"
    );
}

/// The buttons sit in the bar without overlapping, the list in its panel on the end side,
/// and right to left mirrors both.
#[test]
fn controls_sit_in_the_bar_and_mirror() {
    let Some(mut h) = Harness::new() else { return };
    let v = sample();
    let pal = Palette::dark();
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let mut out = Vec::new();
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        out.push(draw(dl, text, &pal, &v, area, false, 1.0));
        out.push(draw(dl, text, &pal, &v, area, true, 1.0));
        out.push(draw(dl, text, &pal, &v, area, false, 2.0));
    });
    let (l, r) = (&out[0], &out[1]);
    assert_eq!(l.bar, r.bar, "the bar is centred");
    assert!(l.panel.x > SIZE.0 / 2.0, "the list is on the right in LTR");
    assert!(r.panel.right() < SIZE.0 / 2.0, "and on the left in RTL");
    for d in &out {
        let buttons: Vec<_> = d
            .hits
            .iter()
            .filter(|(_, h)| !matches!(h, Hit::Item(_)))
            .collect();
        assert_eq!(buttons.len(), 13);
        for (i, (a, ha)) in buttons.iter().enumerate() {
            let b = d.bar;
            assert!(
                a.x >= b.x && a.right() <= b.right() && a.y >= b.y && a.bottom() <= b.bottom(),
                "{ha:?} {a:?} outside {b:?}"
            );
            for (o, hb) in &buttons[i + 1..] {
                let apart =
                    a.right() <= o.x || o.right() <= a.x || a.bottom() <= o.y || o.bottom() <= a.y;
                assert!(apart, "{ha:?} {a:?} overlaps {hb:?} {o:?}");
            }
        }
        for (a, ha) in d.hits.iter().filter(|(_, h)| matches!(h, Hit::Item(_))) {
            let p = d.panel;
            assert!(a.x >= p.x && a.right() <= p.right() && a.y >= p.y, "{ha:?}");
            assert!(a.bottom() <= p.bottom() + 0.5, "{ha:?} {a:?} below {p:?}");
        }
    }
    // Every row fits at text scale 1; the selected one at least at text scale 2.
    assert_eq!(
        l.hits
            .iter()
            .filter(|(_, h)| matches!(h, Hit::Item(_)))
            .count(),
        4
    );
    assert!(out[2].hits.iter().any(|(_, h)| *h == Hit::Item(1)));
    for ((a, ha), (b, hb)) in l.hits.iter().zip(&r.hits) {
        assert_eq!(ha, hb);
        assert!((a.x - (SIZE.0 - b.right())).abs() < 0.01, "{ha:?}");
    }
}

#[test]
fn a11y_nodes_map_back_to_controls() {
    let Some(mut h) = Harness::new() else { return };
    let v = sample();
    let pal = Palette::dark();
    let mut d = None;
    h.render(SIZE, 1.0, backdrop, |dl, text| {
        d = Some(draw(
            dl,
            text,
            &pal,
            &v,
            Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
            false,
            1.0,
        ));
    });
    let d = d.unwrap();
    let mut tree = blank_tree();
    a11y(&mut tree, &v, &d, 1.0);
    // Focus 8 is the Delete button (seven tools, Undo, Delete).
    assert_eq!(target(tree.focus.0, &v), Some(Hit::Delete));
    let role = |r| tree.nodes.iter().filter(|(_, n)| n.role() == r).count();
    assert_eq!(role(accesskit::Role::Toolbar), 1);
    assert_eq!(role(accesskit::Role::Button), 13);
    assert_eq!(role(accesskit::Role::ListBox), 1);
    assert_eq!(role(accesskit::Role::ListBoxOption), 4);
    for h in v.focusables() {
        let id = A11Y_BASE + node(h);
        assert_eq!(target(id, &v), Some(h));
        let n = tree
            .nodes
            .iter()
            .find(|(i, _)| i.0 == id)
            .map(|x| &x.1)
            .unwrap();
        assert!(n.label().is_some_and(|l| !l.is_empty()), "{h:?}");
        assert!(n.bounds().is_some(), "{h:?}");
    }
    let sel = tree
        .nodes
        .iter()
        .find(|(i, _)| i.0 == A11Y_BASE + 0x101)
        .unwrap();
    assert_eq!(sel.1.is_selected(), Some(true));
    // The selected rectangle's row carries its value, statistics and source.
    let label = sel.1.label().unwrap();
    assert!(
        label.contains("mm²") && label.contains("153.2") && label.contains("DICOM"),
        "{label}"
    );
}

/// The inspection probes' readings over a generated radiograph strip: a profile with its
/// plot, an SNR region grown to 55 × 20, a duplex IQI, a single wire, a crack with its cuts
/// and a density point (the readings computed, not typed in).
fn sample_probes() -> MeasureView {
    use specular_measure::inspect::{self, Grey, Probe, Reading};
    let cal = Calibration::spacing(0.01, 0.01, Unit::Mm, Source::User).unwrap();
    // Duplex pairs D6–D13 along x at a 1 mm pitch, 10 µm pixels; dark wires on 30000 with a
    // gentle blur.
    let (w, h) = (900u32, 640u32);
    let chord = |x: f64, d: f64| {
        let r = d / 2.0;
        if x.abs() < r {
            2.0 * (r * r - x * x).sqrt()
        } else {
            0.0
        }
    };
    let row: Vec<f64> = (0..w)
        .map(|x| {
            let xm = (f64::from(x) + 0.5) * 0.01;
            let mut t = 0.0;
            for (i, (_, d)) in inspect::DUPLEX[5..13].iter().enumerate() {
                let c = 0.9 + 1.0 * i as f64;
                t += chord(xm - (c - d), *d) + chord(xm - (c + d), *d);
            }
            30000.0 * (-0.6 * t).exp()
        })
        .collect();
    let blurred: Vec<f64> = (0..w as usize)
        .map(|i| {
            let k: Vec<usize> = (i.saturating_sub(1)..=(i + 1).min(w as usize - 1)).collect();
            k.iter().map(|j| row[*j]).sum::<f64>() / k.len() as f64
        })
        .collect();
    let data: Vec<f64> = (0..h)
        .flat_map(|y| {
            let b = &blurred;
            (0..w).map(move |x| {
                let noise = f64::from((x * 7919 + y * 104_729) % 97) - 48.0;
                // A dark crack 6 px wide down x = 600 in the lower half.
                if y > 380 && (557..563).contains(&x) {
                    9000.0 + noise
                } else if (160..240).contains(&y) {
                    b[x as usize] + noise
                } else {
                    24000.0 + noise * 4.0
                }
            })
        })
        .collect();
    let g = Grey::new(0, 0, w, h, data).unwrap();
    let mut items = Vec::new();
    let mut add = |probe: Probe, shape: Shape, reading: Reading| {
        let mut m = Measurement::probed(probe, shape, cal);
        m.id = items.len() as u32 + 1;
        m.reading = Some(reading);
        items.push(probe::item(&m, false, badge(cal.source, None), |p| {
            (p.x as f32, p.y as f32)
        }));
    };
    let (a, b) = (P::new(20.0, 190.0), P::new(860.0, 190.0));
    let (v, step) = inspect::profile(&g, a, b, 1);
    add(
        Probe::Profile,
        Shape::Line(a, b),
        Reading::Profile {
            values: v,
            step,
            density: None,
        },
    );
    let (iv, istep) = inspect::profile(&g, a, b, inspect::IQI_WIDTH);
    let mm = inspect::mm_per_px(&cal, a, b).unwrap() * istep;
    add(
        Probe::Duplex,
        Shape::Line(P::new(20.0, 210.0), P::new(860.0, 210.0)),
        Reading::Duplex(inspect::duplex(&iv, mm).unwrap()),
    );
    let region = inspect::snr_region((120.0, 330.0, 150.0, 340.0), w, h).unwrap();
    let snr = inspect::snr(&g, region, Some(63.0)).unwrap();
    add(
        Probe::Snr,
        Shape::Rect {
            x0: f64::from(region.0),
            y0: f64::from(region.1),
            x1: f64::from(region.2),
            y1: f64::from(region.3),
        },
        Reading::Snr(snr),
    );
    let crack = [P::new(560.0, 400.0), P::new(560.0, 600.0)];
    add(
        Probe::Crack,
        Shape::Line(crack[0], crack[1]),
        Reading::Crack(inspect::crack(&g, &crack, 10, 25.0, &cal).unwrap()),
    );
    add(
        Probe::Density,
        Shape::Point(P::new(300.0, 300.0)),
        Reading::Density {
            grey: 24012.0,
            density: Some(2.41),
        },
    );
    MeasureView {
        open: true,
        kind: Kind::Inspect,
        items,
        selected: Some(1),
        drawing: Vec::new(),
        drawing_closed: false,
        marks: Vec::new(),
        cursor: None,
        status: probe::hint(Probe::Duplex),
        prompt: None,
        can_undo: true,
        focus: None,
        spot: false,
        geometry: false,
        template: Template::Vhs,
        probe: Probe::Duplex,
    }
}

/// The probe button follows the Inspect tool.
#[test]
fn probe_controls() {
    let v = sample_probes();
    let f = v.focusables();
    let i = f
        .iter()
        .position(|h| *h == Hit::Tool(Kind::Inspect))
        .unwrap();
    assert_eq!(f[i + 1], Hit::Probe);
    assert!(
        v.label(Hit::Probe)
            .contains(&probe::label(specular_measure::inspect::Probe::Duplex))
    );
    // Every probe's reading shows a value.
    assert!(
        v.items.iter().all(|it| !it.value.is_empty()),
        "{:?}",
        v.items
    );
    assert!(v.items[0].plot.len() > 100);
    assert_eq!(v.items[3].guides.len(), 10);
}

#[test]
fn inspect_probes_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-measure-tool",
    );
    let he = specular_i18n::langid!("he");
    let en = specular_i18n::langid!("en-US");
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let pal = Palette::dark();
    for (name, lang, rtl) in [("ltr", &en, false), ("he-rtl", &he, true)] {
        let (shot, tree) = specular_i18n::I18n::with_thread_language(lang.clone(), || {
            let v = sample_probes();
            let mut drawn = None;
            let shot = h.render(SIZE, 1.0, backdrop, |dl, text| {
                drawn = Some(draw(dl, text, &pal, &v, area, rtl, 1.0));
            });
            let mut tree = super::super::a11y_audit::window(SIZE, 1.0);
            a11y(&mut tree, &v, &drawn.expect("drawn"), 1.0);
            (shot, tree)
        });
        let id = format!("measure-inspect-{name}");
        goldens.check(&id, &shot);
        super::super::a11y_audit::expect_clean(&id, &tree, rtl);
    }
    goldens.finish();
}
