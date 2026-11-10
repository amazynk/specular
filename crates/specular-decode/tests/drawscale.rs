//! Drawing scales: PDF viewports with rectilinear measures and CAD units and layout
//! viewports give calibrations by region; measured lengths and areas match the drawn
//! geometry.

use std::sync::Arc;

use specular_core::Source;
use specular_decode::drawing::specular_measure::calib::{Source as CalSource, Unit};
use specular_decode::drawing::specular_measure::geom::{P, Shape};
use specular_decode::drawing::specular_measure::scale::format_chain;
use specular_decode::drawing::specular_vector::kurbo::{Affine, Point};
use specular_decode::{Registry, Session};

fn open(name: &str, data: Vec<u8>) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, data)))
        .expect("opens")
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1e-12)
}

/// A minimal DXF: header variables, then entities (group code, value pairs).
fn dxf(header: &[(&str, &str, &str)], entities: &str) -> Vec<u8> {
    let mut s = String::from("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n");
    for (name, code, value) in header {
        s.push_str(&format!("9\n{name}\n{code}\n{value}\n"));
    }
    s.push_str("0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    s.push_str(entities);
    s.push_str("0\nENDSEC\n0\nEOF\n");
    s.into_bytes()
}

fn line(x0: f64, y0: f64, x1: f64, y1: f64, paper: bool) -> String {
    let space = if paper { "67\n1\n" } else { "" };
    format!("0\nLINE\n8\n0\n{space}10\n{x0}\n20\n{y0}\n30\n0\n11\n{x1}\n21\n{y1}\n31\n0\n")
}

/// A paper-space viewport: centre and size on the paper, the model point at its centre and
/// the model height it shows.
fn viewport(id: u32, c: (f64, f64), size: (f64, f64), view: (f64, f64), vh: f64) -> String {
    format!(
        "0\nVIEWPORT\n8\n0\n67\n1\n10\n{}\n20\n{}\n30\n0\n40\n{}\n41\n{}\n68\n1\n69\n{id}\n\
         12\n{}\n22\n{}\n45\n{vh}\n",
        c.0, c.1, size.0, size.1, view.0, view.1
    )
}

/// The sheet-to-stored map of a drawing session.
fn to_stored(s: &dyn Session) -> Affine {
    s.drawing().expect("a drawing").to_stored
}

#[test]
fn pdf_viewports_measure_at_their_scales() {
    let s = open("sheet.pdf", specular_pdf::fixture::drawing_scales());
    let ds = s.drawing_scale().expect("a drawing scale");
    assert_eq!(ds.regions.len(), 2, "the GEO viewport is not a scale");
    // 800 pt wide at the session's resolution.
    let k = f64::from(s.info().width) / 800.0;
    let h = f64::from(s.info().height);
    let stored = |x: f64, y: f64| P::new(x * k, h - y * k);

    // Viewport 1 at 1:50: the 100 mm line is 5 m.
    let a = stored(100.0, 100.0);
    let b = stored(383.4646, 100.0);
    let i = ds.region_at(a.x, a.y).expect("in viewport 1");
    let r = &ds.regions[i];
    assert_eq!((r.viewport, r.ratio.as_deref()), (Some(1), Some("1:50")));
    assert_eq!(r.name.as_deref(), Some("Plan"));
    assert_eq!(
        (r.cal.unit, r.cal.source),
        (Unit::M, CalSource::DrawingScale)
    );
    let len = r.cal.distance(a, b);
    assert!(near(len, 5.0, 1e-4), "{len}");
    assert_eq!(format_chain(len, &r.distance).unwrap(), "5.00 m");
    // A 100 × 100 mm square on the paper is 25 m² drawn, written per the A array.
    let sq = Shape::Rect {
        x0: a.x,
        y0: a.y - 283.4646 * k,
        x1: b.x,
        y1: a.y,
    };
    let area = sq.area(&r.cal).unwrap().value;
    assert!(near(area, 25.0, 1e-4), "{area}");
    assert_eq!(format_chain(area, &r.area).unwrap(), "25.00 sq m");

    // Viewport 2 at 1 in = 4 ft: the 1.25 in line is 5 ft, written in feet and inches.
    let a = stored(520.0, 320.0);
    let b = stored(610.0, 320.0);
    let i = ds.region_at(a.x, a.y).expect("in viewport 2");
    let r = &ds.regions[i];
    assert_eq!((r.viewport, r.ratio.as_deref()), (Some(2), Some("1:48")));
    assert_eq!(r.cal.unit, Unit::Ft);
    let len = r.cal.distance(a, b);
    assert!(near(len, 5.0, 1e-4), "{len}");
    assert_eq!(format_chain(len, &r.distance).unwrap(), "5 ft 0 in");
    assert_eq!(
        format_chain(5.0 + 3.5 / 12.0, &r.distance).unwrap(),
        "5 ft 3 1/2 in"
    );

    // Outside both viewports: no drawing scale (the title block is paper).
    let t = stored(470.0, 20.0);
    assert_eq!(ds.region_at(t.x, t.y), None);

    // The sheet prints as drawn at 1:50 (its largest viewport), a point per 1/72 in.
    assert!(near(ds.ratio, 50.0, 1e-9));
    assert_eq!(ds.unit, Unit::M);
    let mm = ds.paper_mm_per_px.unwrap() * f64::from(s.info().width);
    assert!(near(mm, 800.0 * 25.4 / 72.0, 1e-9), "{mm}");
}

#[test]
fn a_plain_pdf_prints_at_actual_size() {
    let pdf = specular_pdf::fixture::build(
        &[specular_pdf::fixture::Page {
            width: 595.0,
            height: 842.0,
            content: "",
        }],
        "",
        None,
        "",
        &[],
    );
    let s = open("a4.pdf", pdf);
    let ds = s.drawing_scale().unwrap();
    assert!(ds.regions.is_empty());
    assert_eq!(ds.ratio, 1.0);
    let mm = ds.paper_mm_per_px.unwrap() * f64::from(s.info().width);
    assert!(near(mm, 595.0 * 25.4 / 72.0, 1e-9), "{mm}");
}

#[test]
fn dxf_model_space_in_millimetres() {
    let s = open(
        "mm.dxf",
        dxf(
            &[("$INSUNITS", "70", "4"), ("$LUPREC", "70", "1")],
            &line(0.0, 0.0, 5000.0, 0.0, false),
        ),
    );
    let ds = s.drawing_scale().unwrap();
    assert_eq!(ds.regions.len(), 1);
    let r = &ds.regions[0];
    assert_eq!((r.bounds, r.viewport), (None, None));
    assert_eq!(
        (r.cal.unit, r.cal.source),
        (Unit::Mm, CalSource::DrawingUnits)
    );
    let t = to_stored(s.as_ref());
    let p = |x: f64, y: f64| {
        let q = t * Point::new(x, y);
        P::new(q.x, q.y)
    };
    let len = r.cal.distance(p(0.0, 0.0), p(5000.0, 0.0));
    assert!(near(len, 5000.0, 1e-9), "{len}");
    assert_eq!(format_chain(len, &r.distance).unwrap(), "5000.0 mm");
    // Without a dimension scale model space prints at 1:1.
    assert_eq!(ds.ratio, 1.0);
    let mm_per_px = ds.paper_mm_per_px.unwrap();
    assert!(near(
        mm_per_px * (p(5000.0, 0.0).x - p(0.0, 0.0).x),
        5000.0,
        1e-9
    ));
}

#[test]
fn dxf_imperial_by_measurement_in_architectural_style() {
    // No $INSUNITS: $MEASUREMENT 0 says inches; $LUNITS 4 writes feet and inches.
    let s = open(
        "arch.dxf",
        dxf(
            &[
                ("$MEASUREMENT", "70", "0"),
                ("$LUNITS", "70", "4"),
                ("$LUPREC", "70", "3"),
                ("$DIMSCALE", "40", "48"),
            ],
            &line(0.0, 0.0, 63.5, 0.0, false),
        ),
    );
    let ds = s.drawing_scale().unwrap();
    let r = &ds.regions[0];
    assert_eq!(r.cal.unit, Unit::Inch);
    let t = to_stored(s.as_ref());
    let (a, b) = (t * Point::new(0.0, 0.0), t * Point::new(63.5, 0.0));
    let len = r.cal.distance(P::new(a.x, a.y), P::new(b.x, b.y));
    assert!(near(len, 63.5, 1e-9));
    assert_eq!(format_chain(len, &r.distance).unwrap(), "5'-3 1/2\"");
    // $DIMSCALE 48: drawn for a quarter inch to the foot.
    assert_eq!(ds.ratio, 48.0);
    let paper_in = ds.paper_mm_per_px.unwrap() * (b.x - a.x) / 25.4;
    assert!(near(paper_in, 63.5 / 48.0, 1e-9), "{paper_in}");
}

#[test]
fn dxf_unitless_has_no_scale() {
    let s = open("none.dxf", dxf(&[], &line(0.0, 0.0, 10.0, 0.0, false)));
    assert!(s.drawing_scale().is_none());
}

#[test]
fn dxf_layout_viewports_measure_model_space() {
    // Metres in model space; an A3 layout in millimetres with a 1:50 and a 1:100 viewport.
    let mut ents = line(0.0, 0.0, 4.0, 0.0, false);
    ents.push_str(&line(10.0, 10.0, 410.0, 10.0, true));
    ents.push_str(&viewport(
        2,
        (150.0, 150.0),
        (200.0, 200.0),
        (2.0, 0.0),
        10.0,
    ));
    ents.push_str(&viewport(3, (330.0, 150.0), (120.0, 80.0), (2.0, 0.0), 8.0));
    let file = dxf(
        &[
            ("$INSUNITS", "70", "6"),
            ("$MEASUREMENT", "70", "1"),
            ("$PLIMMIN", "10", "0.0\n20\n0.0"),
            ("$PLIMMAX", "10", "420.0\n20\n297.0"),
        ],
        &ents,
    );
    let model = open("plan.dxf", file);
    let s = model.open_page(1).expect("the layout");
    let ds = s.drawing_scale().unwrap();
    assert_eq!(ds.regions.len(), 3, "two viewports and the paper");
    let sheet = s.drawing().unwrap();
    let scene_sheet = &sheet.prepared.scene.sheets[sheet.sheet];
    let t = sheet.to_stored;
    for (vi, ratio) in [(0, "1:50"), (1, "1:100")] {
        let vp = &scene_sheet.viewports[vi];
        // The 4 m model line seen through the viewport.
        let a = t * (vp.transform * Point::new(0.0, 0.0));
        let b = t * (vp.transform * Point::new(4.0, 0.0));
        let c = t * (vp.transform * Point::new(2.0, 0.0));
        let i = ds.region_at(c.x, c.y).expect("inside the viewport");
        let r = &ds.regions[i];
        assert_eq!(r.viewport, Some(vi + 1));
        assert_eq!(r.ratio.as_deref(), Some(ratio));
        assert_eq!(
            (r.cal.unit, r.cal.source),
            (Unit::M, CalSource::DrawingScale)
        );
        let len = r.cal.distance(P::new(a.x, a.y), P::new(b.x, b.y));
        assert!(near(len, 4.0, 1e-9), "viewport {}: {len}", vi + 1);
        // A 2 × 1 m rectangle is 2 m².
        let (p, q) = (
            t * (vp.transform * Point::new(1.0, 0.0)),
            t * (vp.transform * Point::new(3.0, 1.0)),
        );
        let rect = Shape::Rect {
            x0: p.x.min(q.x),
            y0: p.y.min(q.y),
            x1: p.x.max(q.x),
            y1: p.y.max(q.y),
        };
        let area = rect.area(&r.cal).unwrap().value;
        assert!(near(area, 2.0, 1e-9), "{area}");
    }
    // The paper outside the viewports measures in millimetres.
    let a = t * Point::new(10.0, 10.0);
    let b = t * Point::new(410.0, 10.0);
    let i = ds.region_at(a.x, a.y).unwrap();
    let r = &ds.regions[i];
    assert_eq!((r.viewport, r.cal.unit), (None, Unit::Mm));
    let len = r.cal.distance(P::new(a.x, a.y), P::new(b.x, b.y));
    assert!(near(len, 400.0, 1e-9), "{len}");
    // The layout prints as drawn at its largest viewport's scale.
    assert!(near(ds.ratio, 50.0, 1e-9));
    assert_eq!(ds.unit, Unit::M);
    let paper = ds.paper_mm_per_px.unwrap() * (b.x - a.x);
    assert!(near(paper, 400.0, 1e-9), "{paper}");
}

/// A layout with its own plot scale (1 mm = 0.05 m, 1:50): paper space is drawn in model
/// space's metres, measures in metres, and prints 1:50.
#[test]
fn dxf_layout_plot_scale() {
    let mut s = String::from("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n");
    s.push_str("9\n$INSUNITS\n70\n6\n9\n$MEASUREMENT\n70\n1\n0\nENDSEC\n");
    s.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nBLOCK_RECORD\n");
    s.push_str(
        "0\nBLOCK_RECORD\n5\n1F\n2\n*Model_Space\n0\nBLOCK_RECORD\n5\n20\n2\n*Paper_Space\n",
    );
    s.push_str("0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");
    s.push_str(&line(0.0, 0.0, 4.0, 0.0, false));
    s.push_str(&line(0.0, 0.0, 10.0, 0.0, true));
    s.push_str(&line(0.0, 5.0, 10.0, 5.0, true));
    s.push_str("0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n");
    s.push_str(
        "0\nLAYOUT\n5\n30\n100\nAcDbPlotSettings\n44\n420.0\n45\n297.0\n72\n1\n142\n1.0\n\
         143\n0.05\n70\n0\n100\nAcDbLayout\n1\nSheet 1\n71\n1\n330\n20\n",
    );
    s.push_str("0\nENDSEC\n0\nEOF\n");
    let model = open("scaled.dxf", s.into_bytes());
    let p = model.open_page(1).expect("the layout");
    let ds = p.drawing_scale().unwrap();
    let t = to_stored(p.as_ref());
    let (a, b) = (t * Point::new(0.0, 0.0), t * Point::new(10.0, 0.0));
    let r = &ds.regions[ds.region_at(a.x, a.y).unwrap()];
    assert_eq!((r.viewport, r.cal.unit), (None, Unit::M));
    let len = r.cal.distance(P::new(a.x, a.y), P::new(b.x, b.y));
    assert!(near(len, 10.0, 1e-9), "{len}");
    assert!(near(ds.ratio, 50.0, 1e-9), "{}", ds.ratio);
    // 10 m at 1:50 is 200 mm of paper.
    let paper = ds.paper_mm_per_px.unwrap() * (b.x - a.x).abs();
    assert!(near(paper, 200.0, 1e-9), "{paper}");
}
