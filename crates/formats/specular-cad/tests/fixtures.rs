//! Hand-made fixtures (tests/data/*.dxf): what each entity becomes.

use kurbo::{Point, Rect, Shape as _};
use specular_cad::{CadError, Limits, is_dxf, parse, parse_header_only};
use specular_vector::{Color, Item, Shape, SheetKind, VectorScene};

const ENTITIES: &[u8] = include_bytes!("data/entities.dxf");
const R12: &[u8] = include_bytes!("data/r12.dxf");
const LAYOUT: &[u8] = include_bytes!("data/layout.dxf");

fn scene(bytes: &[u8]) -> VectorScene {
    parse(bytes, &Limits::default()).unwrap()
}

fn bounds(it: &Item) -> Rect {
    match &it.shape {
        Shape::Stroke { path, .. } | Shape::Fill { path, .. } => path.bounding_box(),
        Shape::Point(p) => Rect::from_points(*p, *p),
        Shape::Text(t) => {
            let o = t.transform * Point::ZERO;
            Rect::from_points(o, o)
        }
    }
}

fn near(a: Rect, b: Rect) -> bool {
    (a.x0 - b.x0).abs() < 1e-6
        && (a.y0 - b.y0).abs() < 1e-6
        && (a.x1 - b.x1).abs() < 1e-6
        && (a.y1 - b.y1).abs() < 1e-6
}

fn count(s: &VectorScene, ty: &str) -> u64 {
    s.counts.iter().find(|c| c.0 == ty).map_or(0, |c| c.1)
}

#[test]
fn sniffing() {
    assert!(is_dxf(ENTITIES));
    assert!(is_dxf(R12));
    assert!(!is_dxf(b"\x89PNG\r\n\x1a\n"));
    assert!(matches!(
        parse(b"hello world", &Limits::default()),
        Err(CadError::NotDxf)
    ));
    assert!(parse(b"", &Limits::default()).is_err());
}

#[test]
fn header() {
    let h = parse_header_only(ENTITIES).unwrap();
    assert_eq!(h.version, "AC1015");
    assert_eq!(h.release.as_deref(), Some("AutoCAD 2000"));
    assert!(!h.binary);
    assert_eq!(h.units.code, 4);
    assert_eq!(h.extents, Some(Rect::new(-10.0, -10.0, 40.0, 20.0)));
    let s = scene(ENTITIES);
    assert_eq!(s.format, "DXF");
    assert_eq!(s.units.code, 4);
}

#[test]
fn counts_are_as_stored() {
    let s = scene(ENTITIES);
    // Block definitions count once; the ATTRIB under an INSERT does not.
    assert_eq!(count(&s, "INSERT"), 5);
    assert_eq!(count(&s, "ARC"), 2);
    assert_eq!(count(&s, "LINE"), 2);
    assert_eq!(count(&s, "ATTDEF"), 1);
    assert_eq!(count(&s, "ATTRIB"), 0);
    assert_eq!(count(&s, "SEQEND"), 0);
}

#[test]
fn layers_colours_and_linetypes() {
    let s = scene(ENTITIES);
    let walls = s.layer_index("walls").unwrap();
    let l = &s.layers[walls as usize];
    assert_eq!(l.color, Color::Rgb(255, 0, 0));
    assert_eq!(l.lineweight, 50);
    let off = &s.layers[s.layer_index("Off").unwrap() as usize];
    assert!(!off.on, "a negative colour switches the layer off");
    assert_eq!(off.color, Color::Rgb(0, 255, 0));
    assert!(s.layers[s.layer_index("Frozen").unwrap() as usize].frozen);
    let dashed = s
        .linetypes
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case("DASHED"))
        .unwrap() as u32;
    assert_eq!(s.linetypes[dashed as usize].pattern, vec![0.5, -0.25]);

    // The LINE on Walls: BYLAYER red, the layer's dashes at $LTSCALE.
    let model = &s.sheets[0];
    let line = &model.items[0];
    assert_eq!(line.layer, walls);
    assert_eq!(line.color, Color::Rgb(255, 0, 0));
    match &line.shape {
        Shape::Stroke { dash: Some(d), .. } => {
            assert_eq!(d.linetype, dashed);
            assert!((d.scale - 2.0).abs() < 1e-12);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn curves_and_ocs() {
    let s = scene(ENTITIES);
    let items = &s.sheets[0].items;
    // CIRCLE (5, 5) r 2.
    assert!(near(bounds(&items[1]), Rect::new(3.0, 3.0, 7.0, 7.0)));
    // ARC with extrusion (0, 0, -1): centre (3, 0) lands at (-3, 0), mirrored.
    assert!(near(bounds(&items[2]), Rect::new(-4.0, 0.0, -3.0, 1.0)));
    // LWPOLYLINE with a bulge of 1 (a half circle below) and width 0.2: a fill.
    let Shape::Fill { .. } = &items[3].shape else {
        panic!("{:?}", items[3].shape)
    };
    assert!(near(bounds(&items[3]), Rect::new(-0.1, 7.9, 8.0, 10.1)));
}

#[test]
fn text_and_mtext() {
    let s = scene(ENTITIES);
    let items = &s.sheets[0].items;
    let Shape::Text(t) = &items[4].shape else {
        panic!()
    };
    assert_eq!(t.plain(), "Hello");
    assert!((t.paragraphs[0].spans[0].height - 2.5).abs() < 1e-12);
    let Shape::Text(m) = &items[5].shape else {
        panic!()
    };
    assert_eq!(m.plain(), "Bold\nred text");
    assert_eq!(m.wrap, 20.0);
    let bold = &m.paragraphs[0].spans[0];
    assert!(bold.bold);
    assert_eq!(bold.family.as_deref(), Some("Arial"));
    let red = &m.paragraphs[1].spans[0];
    assert_eq!(red.text, "red");
    assert_eq!(red.color, Some(Color::Rgb(255, 0, 0)));
    assert_eq!(m.paragraphs[1].spans[1].color, None);
}

#[test]
fn solid_hatch() {
    let s = scene(ENTITIES);
    let h = &s.sheets[0].items[6];
    assert_eq!(h.color, Color::Rgb(0, 255, 0));
    let Shape::Fill { .. } = &h.shape else {
        panic!("{:?}", h.shape)
    };
    assert!(near(bounds(h), Rect::new(30.0, 0.0, 34.0, 4.0)));
}

#[test]
fn inserts() {
    let s = scene(ENTITIES);
    let items = &s.sheets[0].items;
    let walls = s.layer_index("Walls").unwrap();
    // Door (base (1, 0)) at (20, 0), two columns 5 apart: layer-0 entities move to Walls,
    // BYBLOCK takes the reference's green.
    for (k, x) in [(7, 20.0), (9, 25.0)] {
        assert_eq!(items[k].layer, walls);
        assert_eq!(items[k].color, Color::Rgb(0, 255, 0));
        assert!(near(bounds(&items[k]), Rect::new(x, 0.0, x + 2.0, 0.0)));
    }
    // The attribute, BYLAYER on Walls.
    let Shape::Text(a) = &items[11].shape else {
        panic!()
    };
    assert_eq!(a.plain(), "D7");
    assert_eq!(items[11].color, Color::Rgb(255, 0, 0));
    // Outer → Door, nested.
    assert!(near(bounds(&items[12]), Rect::new(0.0, -5.0, 2.0, -5.0)));
    // A block that inserts itself is drawn once, with a warning.
    assert!(near(bounds(&items[14]), Rect::new(-6.0, -1.0, -4.0, 1.0)));
    assert!(s.warnings.iter().any(|w| w.contains("contains itself")));
    // POINT on a layer that is off: still an item (the viewer hides the layer).
    assert_eq!(items.len(), 16);
    let Shape::Point(p) = items[15].shape else {
        panic!()
    };
    assert_eq!(p, Point::new(2.0, 2.0));
}

#[test]
fn extents() {
    let s = scene(ENTITIES);
    let m = &s.sheets[0];
    assert_eq!(m.kind, SheetKind::Model);
    assert_eq!(m.file_extents, Some(Rect::new(-10.0, -10.0, 40.0, 20.0)));
    let e = m.extents.unwrap();
    assert!(near(e, Rect::new(-6.0, -5.0, 34.0, 15.0)), "{e:?}");
}

#[test]
fn r12_and_code_page() {
    let s = scene(R12);
    assert_eq!(s.version, "AC1009");
    assert_eq!(s.release.as_deref(), Some("AutoCAD R11/R12"));
    assert_eq!(s.codepage.as_deref(), Some("ANSI_1252"));
    let items = &s.sheets[0].items;
    let Shape::Text(t) = &items[0].shape else {
        panic!()
    };
    assert_eq!(t.plain(), "Café ° ⌀10");
    // Closed POLYLINE: a half circle from (0, 0) to (2, 0) bulging down, then straight.
    assert!(near(bounds(&items[1]), Rect::new(0.0, -1.0, 2.0, 2.0)));
    assert_eq!(count(&s, "POLYLINE"), 1);
    assert_eq!(count(&s, "VERTEX"), 0);
}

#[test]
fn paper_space_and_viewports() {
    let s = scene(LAYOUT);
    assert_eq!(s.sheets.len(), 2);
    let l = &s.sheets[1];
    assert_eq!(l.kind, SheetKind::Layout);
    assert_eq!(l.name, "Layout1");
    assert_eq!(l.paper, Some(Rect::new(0.0, 0.0, 297.0, 210.0)));
    assert_eq!(l.items.len(), 1);
    // The sheet's own viewport (id 1) is not a view; the other shows model (50, 25) at
    // paper (150, 100), 50 model units in 100 mm.
    assert_eq!(l.viewports.len(), 1);
    let v = &l.viewports[0];
    assert!(near(v.paper, Rect::new(50.0, 50.0, 250.0, 150.0)));
    let c = v.transform * Point::new(50.0, 25.0);
    assert!((c - Point::new(150.0, 100.0)).hypot() < 1e-9);
    let one = v.transform * Point::new(51.0, 25.0) - c;
    assert!((one.hypot() - 2.0).abs() < 1e-9);
}

#[test]
fn limits() {
    // Too many items.
    let small = Limits {
        max_items: 3,
        ..Limits::default()
    };
    assert!(matches!(parse(ENTITIES, &small), Err(CadError::TooLarge)));
    // Nesting: Outer → Door is two deep.
    let shallow = Limits {
        max_depth: 1,
        ..Limits::default()
    };
    let s = parse(ENTITIES, &shallow).unwrap();
    assert!(s.warnings.iter().any(|w| w.contains("nested too deeply")));
    assert_eq!(s.sheets[0].items.len(), 14);
    // Arrays: the two-column INSERT is cut to one copy.
    let one = Limits {
        max_array: 1,
        ..Limits::default()
    };
    let s = parse(ENTITIES, &one).unwrap();
    assert!(s.warnings.iter().any(|w| w.contains("cut to 1")));
    assert_eq!(s.sheets[0].items.len(), 14);
}
