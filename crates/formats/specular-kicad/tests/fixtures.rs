//! The synthetic fixtures (tests/gen_fixtures.py): KiCad 6, 8 and 9 boards and schematics,
//! legacy refusals and broken files.

use std::path::Path;

use specular_kicad::{Error, Limits, is_kicad_pcb, is_kicad_sch, pcb, sch};
use specular_vector::kurbo::{Point, Shape as _};
use specular_vector::{Color, PcbKind, Shape, VectorScene};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .expect("fixture")
}

fn board(name: &str) -> VectorScene {
    pcb::scene(&data(name), &Limits::default()).expect("board")
}

fn schematic(name: &str) -> VectorScene {
    let resolve = |f: &str| {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(f),
        )
        .ok()
    };
    sch::scene(&data(name), &resolve, &Limits::default()).expect("schematic")
}

fn layer_items<'a>(s: &'a VectorScene, layer: &str) -> Vec<&'a Shape> {
    let i = s
        .layers
        .iter()
        .position(|l| l.name == layer)
        .unwrap_or_else(|| panic!("no layer {layer}"));
    s.sheets[0]
        .items
        .iter()
        .filter(|it| it.layer == i as u32)
        .map(|it| &it.shape)
        .collect()
}

fn texts(s: &VectorScene, sheet: usize) -> Vec<String> {
    s.sheets[sheet]
        .items
        .iter()
        .filter_map(|it| match &it.shape {
            Shape::Text(t) => Some(t.plain()),
            _ => None,
        })
        .collect()
}

fn count(s: &VectorScene, what: &str) -> u64 {
    s.counts.iter().find(|c| c.0 == what).map_or(0, |c| c.1)
}

#[test]
fn kicad6_board() {
    let s = board("board6.kicad_pcb");
    assert_eq!(s.format, "KiCad");
    assert_eq!(s.release.as_deref(), Some("KiCad 6"));
    assert_eq!(s.units.code, 4);
    let info = s.pcb.as_deref().expect("facts");
    assert_eq!(info.kind, PcbKind::KicadBoard);
    assert_eq!(info.size_mm, Some((50.0, 30.0)));
    assert_eq!(info.thickness_mm, Some(1.6));
    assert_eq!(info.copper_layers, 2);
    assert_eq!(info.components, 1);
    assert_eq!(info.nets, 2);
    assert_eq!(info.tracks, 3);
    assert_eq!(info.vias, 1);
    // Two pad holes, the mounting hole and the via.
    assert_eq!(info.holes, 4);
    assert_eq!(info.drill_sizes, 4);
    assert_eq!(info.project.as_deref(), Some("Fixture six"));
    assert_eq!(count(&s, "pad"), 8);
    // Layers bottom to top, with the user names KiCad shows.
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    let pos = |n: &str| names.iter().position(|x| *x == n).unwrap_or(usize::MAX);
    assert!(pos("B.Cu") < pos("F.Cu"), "{names:?}");
    assert!(pos("F.Cu") < pos("F.Silkscreen"), "{names:?}");
    assert_eq!(names.last(), Some(&"Edge.Cuts"));
    assert_eq!(s.layer_opacity.len(), s.layers.len());
    // The thru-hole pads are on both copper layers; the hidden value is not drawn, the user
    // text took the reference.
    assert_eq!(layer_items(&s, "B.Cu").len(), 2 + 1 + 1 + 1);
    let t = texts(&s, 0);
    assert!(t.contains(&"U1".to_string()), "{t:?}");
    assert_eq!(t.iter().filter(|x| *x == "U1").count(), 2, "{t:?}");
    assert!(!t.contains(&"Pads".to_string()), "{t:?}");
    assert!(t.contains(&"Board six".to_string()));
    // Holes punch through in the background colour.
    assert!(
        layer_items(&s, "Holes").len() == 4
            && s.sheets[0]
                .items
                .iter()
                .filter(|i| i.color == Color::Background)
                .count()
                == 4
    );
    // y is up: the board spans y from -30 to 0.
    let e = s.sheets[0].extents.expect("extents");
    assert!(e.y0 < -29.0 && e.y1 > -0.5, "{e:?}");
    // The footprint at (20, 15) turned 90°: its silkscreen line from (-3,-3) to (3,-3) runs
    // vertically at x = 20 - 3.
    let silk = layer_items(&s, "F.Silkscreen");
    let line = silk
        .iter()
        .find_map(|sh| match sh {
            Shape::Fill { path, .. } => {
                let b = path.bounding_box();
                (b.height() > 5.9 && b.width() < 0.2).then_some(b)
            }
            _ => None,
        })
        .expect("the turned silkscreen line");
    assert!((line.center().x - 17.0).abs() < 1e-6, "{line:?}");
    assert!((line.center().y + 15.0).abs() < 1e-6, "{line:?}");
}

#[test]
fn kicad8_board() {
    let s = board("board8.kicad_pcb");
    assert_eq!(s.release.as_deref(), Some("KiCad 8"));
    let info = s.pcb.as_deref().unwrap();
    assert_eq!(info.generator.as_deref(), Some("pcbnew 8.0"));
    assert_eq!(info.copper_layers, 4);
    assert_eq!(info.size_mm, Some((40.0, 20.0)));
    assert_eq!(info.thickness_mm, Some(1.2));
    assert_eq!(info.components, 2);
    let t = texts(&s, 0);
    for want in ["R7", "10k", "H1", "Hole", "40.00 mm", "Notes\nline two"] {
        assert!(t.contains(&want.to_string()), "{want} in {t:?}");
    }
    // The hidden footprint field is left out.
    assert!(!t.contains(&"Fixture:R".to_string()), "{t:?}");
    // Back-side text reads mirrored.
    let r7 = s.sheets[0]
        .items
        .iter()
        .find_map(|it| match &it.shape {
            Shape::Text(tb) if tb.plain() == "R7" => Some(tb.transform),
            _ => None,
        })
        .unwrap();
    assert!(r7.determinant() < 0.0);
    // Inner layers sit between the outer ones, In2 below In1.
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
    assert!(
        pos("B.Cu") < pos("In2.Cu") && pos("In2.Cu") < pos("In1.Cu") && pos("In1.Cu") < pos("F.Cu")
    );
    // The thru-hole pad of H1 is on all four copper layers.
    assert_eq!(layer_items(&s, "In2.Cu").len(), 2);
    assert_eq!(count(&s, "dimension"), 1);
}

#[test]
fn kicad9_board() {
    let s = board("board9.kicad_pcb");
    assert_eq!(s.release.as_deref(), Some("KiCad 9"));
    let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(
        names.contains(&"F.Courtyard") && names.contains(&"User.1"),
        "{names:?}"
    );
    let t = texts(&s, 0);
    assert_eq!(t, vec!["C3".to_string()]);
    // The outline with an arc in its points bulges to x = 22.
    let info = s.pcb.as_deref().unwrap();
    let (w, h) = info.size_mm.unwrap();
    assert!(
        (w - 22.0).abs() < 1e-3 && (h - 10.0).abs() < 1e-3,
        "{w} {h}"
    );
    // The keepout is an outline (a hairline), not a fill.
    assert!(
        layer_items(&s, "F.Cu")
            .iter()
            .any(|sh| matches!(sh, Shape::Stroke { .. }))
    );
}

#[test]
fn kicad6_schematic_with_a_sheet() {
    let s = schematic("sch6.kicad_sch");
    assert_eq!(s.release.as_deref(), Some("KiCad 6"));
    assert_eq!(s.sheets.len(), 2);
    assert_eq!(s.sheets[0].name, "/");
    assert_eq!(s.sheets[1].name, "/power/");
    let info = s.pcb.as_deref().unwrap();
    assert_eq!(info.kind, PcbKind::KicadSchematic);
    assert_eq!(info.sheets, 2);
    // R1, U1 and X1 on the root, R5 below; the power symbol counted apart.
    assert_eq!(info.components, 4);
    assert_eq!(count(&s, "power symbol"), 1);
    let t = texts(&s, 0);
    // References from the root's symbol_instances, the unit letter for the second unit.
    for want in [
        "R1",
        "10k",
        "U1B",
        "GND",
        "NET1",
        "CLK",
        "OUT",
        "A note",
        "power",
        "sub6.kicad_sch",
        "VIN",
        "Fixture schematic",
        "X1",
        "&",
    ] {
        assert!(t.contains(&want.to_string()), "{want} in {t:?}");
    }
    assert!(
        !t.iter().any(|x| x == "#PWR01" || x == "R?" || x == "U?"),
        "{t:?}"
    );
    // The pins of R are numbered but the numbers are hidden; NAND's names show.
    assert!(!t.contains(&"1".to_string()));
    assert!(t.contains(&"C".to_string()), "{t:?}");
    let sub = texts(&s, 1);
    assert!(sub.contains(&"R5".to_string()), "{sub:?}");
    assert!(
        sub.contains(&"/power/   sub6.kicad_sch   #2".to_string()),
        "{sub:?}"
    );
    // The paper: A4 landscape, A5 below.
    let p = s.sheets[0].paper.unwrap();
    assert!((p.width() - 297.0).abs() < 1e-9 && (p.height() - 210.0).abs() < 1e-9);
    let p = s.sheets[1].paper.unwrap();
    assert!((p.width() - 210.0).abs() < 1e-9);
    // The R at (50.8, 64.77): its body runs vertically around it (y up in the scene).
    let body = s.sheets[0]
        .items
        .iter()
        .filter(|it| it.layer == 3)
        .find_map(|it| match &it.shape {
            Shape::Fill { path, .. } => {
                let b = path.bounding_box();
                (b.contains(Point::new(50.8, -64.77)) && b.height() > 5.0 && b.width() < 2.5)
                    .then_some(b)
            }
            _ => None,
        });
    assert!(body.is_some());
}

#[test]
fn kicad8_schematic_instances_and_missing_sheets() {
    let s = schematic("sch8.kicad_sch");
    assert_eq!(s.release.as_deref(), Some("KiCad 8"));
    // The root and the same file twice, with each instance's own references.
    let names: Vec<&str> = s.sheets.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["/", "/chan/", "/chan2/"]);
    assert!(texts(&s, 0).contains(&"R9".to_string()));
    assert!(
        texts(&s, 1).contains(&"R101".to_string()),
        "{:?}",
        texts(&s, 1)
    );
    assert!(
        texts(&s, 2).contains(&"R201".to_string()),
        "{:?}",
        texts(&s, 2)
    );
    assert!(texts(&s, 0).contains(&"Boxed note".to_string()));
    assert!(
        s.warnings.iter().any(|w| w.contains("missing.kicad_sch")),
        "{:?}",
        s.warnings
    );
    assert_eq!(count(&s, "bus"), 1);
    let p = s.sheets[0].paper.unwrap();
    assert!((p.width() - 420.0).abs() < 1e-9);
}

#[test]
fn a_sheet_that_contains_itself_is_shown_once() {
    let s = schematic("cycle.kicad_sch");
    assert_eq!(s.sheets.len(), 2);
    assert!(
        s.warnings.iter().any(|w| w.contains("contains itself")),
        "{:?}",
        s.warnings
    );
}

#[test]
fn legacy_and_broken_files() {
    let l = Limits::default();
    for name in ["kicad5.kicad_pcb", "pcbnew4.brd"] {
        match pcb::scene(&data(name), &l) {
            Err(Error::Unsupported(m)) => assert!(m.contains("KiCad 5"), "{m}"),
            r => panic!("{name}: {:?}", r.map(|s| s.format)),
        }
    }
    let none = |_: &str| None;
    match sch::scene(&data("legacy.sch"), &none, &l) {
        Err(Error::Unsupported(m)) => assert!(m.contains("KiCad 6 or later"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.format)),
    }
    assert!(matches!(
        pcb::scene(&data("truncated.kicad_pcb"), &l),
        Err(Error::Truncated(_))
    ));
    assert!(matches!(
        pcb::scene(&data("not_kicad.kicad_pcb"), &l),
        Err(Error::NotThisFormat)
    ));
    assert!(matches!(
        sch::scene(&data("board6.kicad_pcb"), &none, &l),
        Err(Error::NotThisFormat)
    ));
    assert!(is_kicad_pcb(&data("board8.kicad_pcb")));
    assert!(is_kicad_sch(&data("sch8.kicad_sch")));
    // Every prefix of a board and a schematic: an error or a scene, never a panic.
    for name in ["board6.kicad_pcb", "sch6.kicad_sch"] {
        let d = data(name);
        for cut in (0..d.len()).step_by(97) {
            let _ = pcb::scene(&d[..cut], &l);
            let _ = sch::scene(&d[..cut], &none, &l);
        }
    }
    // The item cap stops a board early with a warning.
    let small = Limits {
        items: 5,
        ..Limits::default()
    };
    let s = pcb::scene(&data("board6.kicad_pcb"), &small).unwrap();
    assert_eq!(s.sheets[0].items.len(), 5);
    assert!(s.warnings.iter().any(|w| w.contains("left out")));
}
