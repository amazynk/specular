//! Unit tests on hand-written streams.

use specular_vector::kurbo::Point;

use super::*;
use crate::records as r;

/// Writes records.
#[derive(Default)]
struct W(Vec<u8>);

fn real8(v: f64) -> [u8; 8] {
    if v == 0.0 {
        return [0; 8];
    }
    let sign = if v < 0.0 { 0x80u8 } else { 0 };
    let mut m = v.abs();
    let mut e = 64i32;
    while m >= 1.0 {
        m /= 16.0;
        e += 1;
    }
    while m < 1.0 / 16.0 {
        m *= 16.0;
        e -= 1;
    }
    let mant = (m * 2f64.powi(56)).round() as u64;
    let mut out = [0u8; 8];
    out[0] = sign | e as u8;
    out[1..].copy_from_slice(&mant.to_be_bytes()[1..]);
    out
}

impl W {
    fn rec(&mut self, rtype: u8, dtype: u8, data: &[u8]) -> &mut Self {
        let len = (4 + data.len()) as u16;
        self.0.extend_from_slice(&len.to_be_bytes());
        self.0.push(rtype);
        self.0.push(dtype);
        self.0.extend_from_slice(data);
        self
    }
    fn i16(&mut self, rtype: u8, v: &[i16]) -> &mut Self {
        let d: Vec<u8> = v.iter().flat_map(|x| x.to_be_bytes()).collect();
        self.rec(rtype, r::DT_I16, &d)
    }
    fn i32(&mut self, rtype: u8, v: &[i32]) -> &mut Self {
        let d: Vec<u8> = v.iter().flat_map(|x| x.to_be_bytes()).collect();
        self.rec(rtype, r::DT_I32, &d)
    }
    fn real(&mut self, rtype: u8, v: &[f64]) -> &mut Self {
        let d: Vec<u8> = v.iter().flat_map(|&x| real8(x)).collect();
        self.rec(rtype, r::DT_REAL8, &d)
    }
    fn str(&mut self, rtype: u8, s: &str) -> &mut Self {
        let mut d = s.as_bytes().to_vec();
        if d.len() % 2 == 1 {
            d.push(0);
        }
        self.rec(rtype, r::DT_ASCII, &d)
    }
    fn none(&mut self, rtype: u8) -> &mut Self {
        self.rec(rtype, r::DT_NONE, &[])
    }
    fn head(&mut self) -> &mut Self {
        self.i16(r::HEADER, &[600])
            .i16(r::BGNLIB, &[2026, 1, 2, 3, 4, 5, 2026, 1, 2, 3, 4, 5])
            .str(r::LIBNAME, "LIB")
            .real(r::UNITS, &[1e-3, 1e-9])
    }
    fn cell(&mut self, name: &str) -> &mut Self {
        self.i16(r::BGNSTR, &[0; 12]).str(r::STRNAME, name)
    }
    fn rect(&mut self, layer: i16, dt: i16, x0: i32, y0: i32, x1: i32, y1: i32) -> &mut Self {
        self.none(r::BOUNDARY)
            .i16(r::LAYER, &[layer])
            .i16(r::DATATYPE, &[dt])
            .i32(r::XY, &[x0, y0, x1, y0, x1, y1, x0, y1, x0, y0])
            .none(r::ENDEL)
    }
    fn sref(&mut self, name: &str, x: i32, y: i32) -> &mut Self {
        self.none(r::SREF)
            .str(r::SNAME, name)
            .i32(r::XY, &[x, y])
            .none(r::ENDEL)
    }
    fn end_cell(&mut self) -> &mut Self {
        self.none(r::ENDSTR)
    }
    fn end(&mut self) -> Vec<u8> {
        self.none(r::ENDLIB);
        std::mem::take(&mut self.0)
    }
}

fn lib(bytes: &[u8]) -> Library {
    parse(bytes, Limits::default()).unwrap()
}

#[test]
fn reads_units_cells_and_layers() {
    let b = W::default()
        .head()
        .cell("TOP")
        .rect(5, 1, 0, 0, 100, 50)
        .rect(2, 0, 0, 0, 10, 10)
        .sref("SUB", 1000, 0)
        .end_cell()
        .cell("SUB")
        .rect(5, 1, 0, 0, 10, 10)
        .end_cell()
        .end();
    assert!(is_gds(&b));
    let l = lib(&b);
    assert_eq!(l.name, "LIB");
    assert_eq!(l.version, 600);
    assert!((l.db_unit_m - 1e-9).abs() < 1e-22);
    assert_eq!(l.layers, vec![(2, 0), (5, 1)]);
    assert_eq!(l.cells.len(), 2);
    assert_eq!(l.top, vec![0]);
    assert_eq!(l.cells[0].shapes[0].layer, 1);
    // Closing point dropped.
    assert_eq!(l.cells[0].shape_points(&l.cells[0].shapes[0]).len(), 4);
    assert_eq!(l.cells[0].refs[0].cell, Some(1));
    let h = hier::hulls(&l);
    let bb = hier::hull_bounds(&h[0]).unwrap();
    assert_eq!((bb.x0, bb.y0, bb.x1, bb.y1), (0.0, 0.0, 1010.0, 50.0));
    assert_eq!(hier::flat_counts(&l), vec![3, 1]);
    assert_eq!(hier::flat_layer_counts(&l, 0), vec![1, 2]);
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
}

#[test]
fn refuses_broken_files() {
    let good = W::default()
        .head()
        .cell("A")
        .rect(1, 0, 0, 0, 1, 1)
        .end_cell()
        .end();
    // Cut anywhere: a clear error, never a panic.
    for n in 0..good.len() {
        match parse(&good[..n], Limits::default()) {
            Err(GdsError::Truncated(_) | GdsError::NotGds | GdsError::Malformed(_)) => {}
            other => panic!("cut at {n}: {other:?}"),
        }
    }
    // Trailing padding after ENDLIB is fine.
    let mut padded = good.clone();
    padded.extend_from_slice(&[0; 300]);
    assert_eq!(lib(&padded).cells.len(), 1);
    // An element without ENDEL.
    let b = W::default()
        .head()
        .cell("A")
        .none(r::BOUNDARY)
        .i16(r::LAYER, &[1])
        .none(r::ENDSTR)
        .end();
    assert!(matches!(
        parse(&b, Limits::default()),
        Err(GdsError::Malformed(_))
    ));
    assert!(matches!(
        parse(b"%SEMI-OASIS\r\n....", Limits::default()),
        Err(GdsError::Oasis)
    ));
    assert!(matches!(
        parse(b"hello", Limits::default()),
        Err(GdsError::NotGds)
    ));
    let small = Limits {
        max_elements: 0,
        ..Limits::default()
    };
    assert!(matches!(parse(&good, small), Err(GdsError::TooLarge(_))));
}

#[test]
fn cuts_cycles_and_warns_of_undefined_cells() {
    let b = W::default()
        .head()
        .cell("A")
        .rect(1, 0, 0, 0, 1, 1)
        .sref("B", 10, 0)
        .end_cell()
        .cell("B")
        .sref("A", 0, 10)
        .sref("NOPE", 0, 0)
        .end_cell()
        .cell("C")
        .sref("C", 5, 5)
        .end_cell()
        .end();
    let l = lib(&b);
    let cut = l
        .cells
        .iter()
        .flat_map(|c| &c.refs)
        .filter(|r| r.cell.is_none())
        .count();
    assert_eq!(cut, 3, "{:?}", l.warnings);
    assert_eq!(l.top, vec![0, 2]);
    assert!(l.warnings.iter().any(|w| w.contains("cycle")));
    assert!(l.warnings.iter().any(|w| w.contains("NOPE")));
    // The order puts children first, and flattening ends.
    assert_eq!(hier::order(&l), vec![1, 0, 2]);
    assert_eq!(hier::flatten(&l, 0, 100).unwrap().len(), 1);
}

#[test]
fn arrays_with_reflection_rotation_and_magnification() {
    let b = W::default()
        .head()
        .cell("TOP")
        .none(r::AREF)
        .str(r::SNAME, "E")
        .i16(r::STRANS, &[0x8000u16 as i16])
        .real(r::MAG, &[2.0])
        .real(r::ANGLE, &[90.0])
        .i16(r::COLROW, &[3, 2])
        .i32(r::XY, &[0, 0, 300, 0, 0, 400])
        .none(r::ENDEL)
        .end_cell()
        .cell("E")
        .rect(1, 0, 0, 0, 10, 20)
        .end_cell()
        .end();
    let l = lib(&b);
    let a = l.cells[0].refs[0].array.unwrap();
    assert_eq!(
        (a.cols, a.rows, a.col, a.row),
        (3, 2, [100.0, 0.0], [0.0, 200.0])
    );
    let s = &l.cells[0].refs[0].strans;
    assert!(s.reflect && s.mag == 2.0 && s.angle == 90.0);
    let flat = hier::flatten(&l, 0, 100).unwrap();
    assert_eq!(flat.len(), 6);
    // (10, 20) reflected (10, -20), doubled (20, -40), turned (40, 20).
    let first = &flat[0].points;
    assert!(first.contains(&Point::new(40.0, 20.0)), "{first:?}");
    let last = &flat[5].points;
    assert!(last.contains(&Point::new(240.0, 220.0)), "{last:?}");
    let bb = hier::hull_bounds(&hier::hulls(&l)[0]).unwrap();
    assert_eq!((bb.x0, bb.y0, bb.x1, bb.y1), (0.0, 0.0, 240.0, 220.0));
    assert_eq!(hier::flat_counts(&l)[0], 6);
    assert!(matches!(hier::flatten(&l, 0, 4), Err(v) if v.len() == 4));
}

#[test]
fn paths_texts_boxes_and_nodes() {
    let b = W::default()
        .head()
        .cell("T")
        // Pathtype 4 with extensions, absolute width.
        .none(r::PATH)
        .i16(r::LAYER, &[3])
        .i16(r::DATATYPE, &[0])
        .i16(r::PATHTYPE, &[4])
        .i32(r::WIDTH, &[-10])
        .i32(r::BGNEXTN, &[2])
        .i32(r::ENDEXTN, &[7])
        .i32(r::XY, &[0, 0, 100, 0])
        .none(r::ENDEL)
        .none(r::BOX)
        .i16(r::LAYER, &[4])
        .i16(r::BOXTYPE, &[2])
        .i32(r::XY, &[0, 0, 5, 0, 5, 5, 0, 5, 0, 0])
        .none(r::ENDEL)
        .none(r::NODE)
        .i16(r::LAYER, &[1])
        .i16(r::NODETYPE, &[0])
        .i32(r::XY, &[0, 0])
        .none(r::ENDEL)
        .none(r::TEXT)
        .i16(r::LAYER, &[9])
        .i16(r::TEXTTYPE, &[1])
        .i16(r::PRESENTATION, &[0b0101])
        .real(r::MAG, &[0.5])
        .i32(r::XY, &[7, 8])
        .str(r::STRING, "VDD")
        .i16(r::PROPATTR, &[1])
        .str(r::PROPVALUE, "x")
        .none(r::ENDEL)
        .end_cell()
        .end();
    let l = lib(&b);
    let c = &l.cells[0];
    assert_eq!(c.nodes, 1);
    assert_eq!(l.counts.properties, 1);
    assert_eq!(c.texts[0].text, "VDD");
    assert_eq!(l.layer_name(c.texts[0].layer), "9/1");
    assert_eq!(l.layers, vec![(3, 0), (4, 2), (9, 1)]);
    let flat = hier::flatten(&l, 0, 10).unwrap();
    let p = &flat[0].points;
    let bb = specular_vector::bounds_of(p.iter().copied()).unwrap();
    assert_eq!((bb.x0, bb.y0, bb.x1, bb.y1), (-2.0, -5.0, 107.0, 5.0));
    let scene = scene::to_scene(&l);
    assert_eq!(scene.layers.len(), 3);
    assert_eq!(scene.units.symbol(), Some("µm"));
    let layout = scene.layout.as_ref().unwrap();
    assert_eq!(layout.cells[0].items.len(), 3);
    assert_eq!(layout.info.top_cells, vec!["T".to_string()]);
    let e = scene.sheets[0].extents.unwrap();
    assert!(
        (e.x0 + 0.002).abs() < 1e-12 && (e.x1 - 0.107).abs() < 1e-12,
        "{e:?}"
    );
}

#[test]
fn zero_width_paths_are_hairlines() {
    let b = W::default()
        .head()
        .cell("T")
        .none(r::PATH)
        .i16(r::LAYER, &[5])
        .i16(r::DATATYPE, &[0])
        .i32(r::WIDTH, &[0])
        .i32(r::XY, &[0, 0, 100, 0, 100, 50])
        .none(r::ENDEL)
        .end_cell()
        .end();
    let l = lib(&b);
    // Along the spine and back, as gdstk reads it; it still has bounds.
    let flat = hier::flatten(&l, 0, 10).unwrap();
    let pts: Vec<(f64, f64)> = flat[0].points.iter().map(|p| (p.x, p.y)).collect();
    assert_eq!(
        pts,
        vec![(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (100.0, 0.0)]
    );
    let scene = scene::to_scene(&l);
    let items = &scene.layout.as_ref().unwrap().cells[0].items;
    assert!(matches!(
        &items[..],
        [specular_vector::Item {
            shape: specular_vector::Shape::Stroke { weight: 0, .. },
            ..
        }]
    ));
    assert!(scene.sheets[0].extents.is_some());
}

#[test]
fn layer_colours_are_distinct_and_stable() {
    let mut seen = std::collections::HashSet::new();
    for l in 0..64u16 {
        for d in 0..4u16 {
            assert!(seen.insert(scene::layer_color(l, d)), "{l}/{d}");
        }
    }
    assert_eq!(scene::layer_color(1, 0), scene::layer_color(1, 0));
}

mod hostile {
    use super::*;

    /// Forty cells, each placing the next twice, the last empty: 2⁴⁰ placements that
    /// add no shapes.
    #[test]
    fn nested_references_to_cells_that_draw_nothing() {
        let mut b = W::default();
        b.head();
        for k in 0..40 {
            let next = format!("C{}", k + 1);
            b.cell(&format!("C{k}"))
                .sref(&next, 0, 0)
                .sref(&next, 1, 1)
                .end_cell();
        }
        b.cell("C40").end_cell();
        let l = lib(&b.end());
        assert!(hier::flatten(&l, 0, 1000).is_err());
    }
}
