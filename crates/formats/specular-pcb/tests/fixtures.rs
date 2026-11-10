//! The synthetic fixtures (tests/gen_fixtures.py) read as they were written.

use std::f64::consts::PI;
use std::path::Path;

use specular_pcb::excellon::{self, Drill};
use specular_pcb::gerber::{self, Gerber, ObjectKind};
use specular_pcb::stack::{self, Plating, Role, Side};
use specular_pcb::{Error, Limits};
use specular_vector::kurbo::Shape as _;
use specular_vector::{Color, PcbKind};

fn data(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn gbr(name: &str) -> Gerber {
    let b = data(name);
    assert!(gerber::is_gerber(&b), "{name} sniffs");
    gerber::parse(&b, &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn drl(name: &str) -> Drill {
    let b = data(name);
    assert!(excellon::is_excellon(&b), "{name} sniffs");
    excellon::parse(&b, &Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn area(g: &Gerber, i: usize) -> f64 {
    g.objects[i].path.area()
}

#[test]
fn apertures() {
    let g = gbr("apertures.gbr");
    // Eight flashes, two circle draws, one rectangle draw, one hairline.
    assert_eq!(g.objects.len(), 12);
    assert_eq!(g.apertures, 9);
    let want = [
        PI * 0.25,
        PI * (0.5625 - 0.0625),
        2.0,
        3.0 - PI * 0.09,
        2.0 + PI * 0.25,
        2.0 + PI * 0.25 - PI * 0.04,
    ];
    for (i, w) in want.iter().enumerate() {
        assert!(
            (area(&g, i) - w).abs() < 2e-3,
            "flash {i}: {} vs {w}",
            area(&g, i)
        );
    }
    // A regular pentagon of circumradius 1.
    let pent = 2.5 * (2.0 * PI / 5.0).sin();
    assert!((area(&g, 6) - pent).abs() < 1e-9);
    assert_eq!(g.objects[11].kind, ObjectKind::Hairline);
    assert!(g.warnings.is_empty(), "{:?}", g.warnings);
    assert_eq!(
        stack::role_of("apertures.gbr", g.file_function().as_deref()),
        Role::TopCopper
    );
}

#[test]
fn macros() {
    let g = gbr("macros.gbr");
    assert_eq!(g.macros, 8);
    assert_eq!(g.objects.len(), 8);
    assert!(g.warnings.is_empty(), "{:?}", g.warnings);
    // CIRCLES: a 2 mm disc with a 1 mm hole.
    assert!((area(&g, 0) - PI * 0.75).abs() < 2e-3);
    // CALC: $4 = 2, rectangle 2 × 1 plus a circle of diameter 0.75 above it, overlapping.
    assert!(area(&g, 6) > 2.0);
    // ROUNDRECT: 3 × 2 with 0.5 mm corners (overlapping parts, union area).
    let b = g.objects[7].path.bounding_box();
    assert!(
        (b.width() - 3.0).abs() < 1e-4 && (b.height() - 2.0).abs() < 1e-4,
        "{b:?}"
    );
    assert_eq!(
        stack::role_of("x", g.file_function().as_deref()),
        Role::InnerCopper(1)
    );
    let l = gbr("macros-legacy.gbr");
    assert_eq!(l.objects.len(), 3);
    assert!((area(&l, 0) - 1.5).abs() < 1e-9);
    assert!((area(&l, 1) - 2.0).abs() < 1e-9);
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
}

#[test]
fn arcs_and_regions() {
    let g = gbr("arcs.gbr");
    assert_eq!(g.objects.len(), 5);
    let r = gbr("regions.gbr");
    assert_eq!(r.objects.len(), 3);
    assert!(r.objects.iter().all(|o| o.kind == ObjectKind::Region));
    // Two contours: 100 − (4 − π) plus 25.
    assert!(
        (area(&r, 0) - (125.0 - 4.0 + PI)).abs() < 1e-3,
        "{}",
        area(&r, 0)
    );
    assert!((area(&r, 1) - 84.0).abs() < 1e-9);
    assert!((area(&r, 2) - PI * 25.0).abs() < 1e-2);
}

#[test]
fn polarity_steps_and_blocks() {
    let g = gbr("polarity.gbr");
    let dark: Vec<bool> = g.objects.iter().map(|o| o.dark).collect();
    assert_eq!(dark, [true, false, true, true, true, true, true, false]);
    // Mirrored, rotated 30° and scaled 1.5: the rectangle is 3 × 1.5 turned.
    let b = g.objects[3].path.bounding_box();
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    assert!((b.width() - (3.0 * c + 1.5 * s)).abs() < 1e-6, "{b:?}");
    let sr = gbr("stepnrepeat.gbr");
    assert_eq!(sr.objects.len(), 4 * 3 * 2 + 1);
    let last = sr.objects[22].path.bounding_box().center();
    assert!(
        (last.x - 9.0).abs() < 1e-6 && (last.y - 5.0).abs() < 1e-6,
        "{last:?}"
    );
    let bl = gbr("blocks.gbr");
    // Block 100: a square, a clear circle and the nested block (one flash): 3 objects,
    // flashed four times.
    assert_eq!(bl.objects.len(), 12);
    assert!(!bl.objects[6].dark && bl.objects[7].dark);
    assert!(bl.warnings.is_empty(), "{:?}", bl.warnings);
}

#[test]
fn attributes_and_old_files() {
    let g = gbr("x2.gbr");
    assert_eq!(g.version(), "RS-274X X3");
    assert_eq!(g.generator().as_deref(), Some("Specular gen_fixtures 1"));
    assert_eq!(g.date().as_deref(), Some("2026-10-19T12:00:00+02:00"));
    assert_eq!(
        g.attribute("SameCoordinates").map(|v| v.join(",")),
        Some("Original".into())
    );
    let d = gbr("deprecated.gbr");
    assert!(d.inch && d.deprecated.len() >= 3, "{:?}", d.deprecated);
    assert_eq!(d.objects.len(), 4);
    assert!(d.warnings.iter().any(|w| w.contains("M02")));
    // Offset 0.1 in along x: "X01" with trailing zeros omitted is 01000, 1.0 in.
    let c = d.objects[0].path.bounding_box();
    assert!((c.x0 - (1.1 * 25.4 - 0.025 * 25.4)).abs() < 1e-6, "{c:?}");
    match gerber::parse(&data("rs274d.gbr"), &Limits::default()) {
        Err(Error::Unsupported(m)) => assert!(m.contains("RS-274-D")),
        r => panic!("{:?}", r.map(|g| g.objects.len())),
    }
    let n = gbr("negative.gbr");
    assert_eq!(n.objects.len(), 3);
    assert!(n.objects[0].dark && !n.objects[1].dark);
}

#[test]
fn drills() {
    let k = drl("kicad.drl");
    assert_eq!((k.holes.len(), k.slots.len(), k.tools.len()), (7, 1, 3));
    assert_eq!(k.tools[2].plated, Some(false));
    assert_eq!(k.generator.as_deref(), Some("Kicad Pcbnew 8.0.4"));
    let a = drl("altium-inch-lz.txt");
    assert_eq!(a.holes.len(), 3);
    assert!(
        (a.holes[1].at.x - 1.5 * 25.4).abs() < 1e-9,
        "{:?}",
        a.holes[1]
    );
    assert_eq!(a.plated, Some(true));
    let e = drl("eagle.drd");
    // M72 inches, no zeros stated, integers: trailing zeros kept (leading suppressed) 2.4.
    assert!(e.guessed);
    assert!(
        (e.holes[0].at.x - 0.3 * 25.4).abs() < 1e-9,
        "{:?}",
        e.holes[0]
    );
    let r = drl("routed.drl");
    assert_eq!(r.slots.len(), 4);
    assert_eq!(r.holes.len(), 5);
    assert!((r.holes[4].at.x - 9.0).abs() < 1e-9);
}

#[test]
fn a_board_folder() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/board");
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    files.sort();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(stack::looks_like_board(&names));
    let s = stack::board_scene(&files, &Limits::default()).unwrap();
    let info = s.pcb.as_ref().unwrap();
    assert_eq!(info.kind, PcbKind::GerberSet);
    assert_eq!(info.layers.len(), 9);
    assert_eq!(info.copper_layers, 2);
    assert_eq!(info.holes, 6);
    assert_eq!(info.drill_sizes, 3);
    assert_eq!(info.thickness_mm, Some(1.6));
    let (w, h) = info.size_mm.unwrap();
    assert!(
        (w - 30.0).abs() < 1e-6 && (h - 20.0).abs() < 1e-6,
        "{w} {h}"
    );
    let roles: Vec<&str> = info.layers.iter().map(|l| l.role.as_str()).collect();
    assert_eq!(roles[0], "Bottom solder mask");
    assert_eq!(roles[roles.len() - 1], "Plated drill");
    assert!(roles.contains(&"Non-plated drill"));
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    // Top view: substrate, copper, the mask (openings erased), silkscreen, holes.
    let top = &s.sheets[1];
    assert!(top.items.iter().any(|i| i.color == Color::Clear));
    assert!(top.items.iter().any(|i| i.color == Color::Background));
    // Role names for the board-colour views.
    assert_eq!(Role::SolderMask(Side::Top).english(), "Top solder mask");
    assert_eq!(Role::Drill(Plating::Plated).english(), "Plated drill");
}
