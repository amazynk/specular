//! The fixtures (built by `fixtures/make.py`) read against ttf-parser, and WOFF2 against
//! fontTools' own decompression (`fixtures/expected/*.woff2.sfnt`).

mod common;

use std::path::{Path, PathBuf};

use specular_font::{Container, Flavor, Limits, OutlineKind, Permission, open, sniff};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn read(name: &str) -> Vec<u8> {
    std::fs::read(dir().join(name)).unwrap()
}

fn fixtures() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir())
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| {
            [".ttf", ".otf", ".ttc", ".otc", ".woff", ".woff2"]
                .iter()
                .any(|e| n.ends_with(e))
        })
        .collect();
    v.sort();
    v
}

#[test]
fn every_fixture_matches_the_oracle() {
    let mut t = common::Tally::default();
    let mut faces = 0;
    for name in fixtures() {
        let f = open(&read(&name), Limits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        for (i, face) in f.faces.iter().enumerate() {
            faces += 1;
            common::compare(face, &format!("{name}#{i}"), &mut t);
        }
    }
    eprintln!(
        "fixtures: {faces} faces, {} checks, {} mismatches",
        t.checked, t.mismatches
    );
    assert_eq!(t.mismatches, 0, "{:#?}", t.notes);
    assert!(faces >= 20);
}

#[test]
fn containers_are_sniffed() {
    let cases = [
        ("sans.ttf", Container::TrueType),
        ("serif.otf", Container::OpenTypeCff),
        ("pair.ttc", Container::Collection),
        ("pair.otc", Container::Collection),
        ("sans.woff", Container::Woff(Flavor::TrueType)),
        ("serif.woff", Container::Woff(Flavor::Cff)),
        ("sans.woff2", Container::Woff2(Flavor::TrueType)),
        ("serif.woff2", Container::Woff2(Flavor::Cff)),
        ("pair.woff2", Container::Woff2(Flavor::Collection)),
    ];
    for (n, c) in cases {
        assert_eq!(sniff(&read(n)), Some(c), "{n}");
    }
    assert_eq!(sniff(b"\0\x01\0\0\0\0"), None);
    assert_eq!(sniff(b"GIF89a"), None);
}

#[test]
fn facts_of_the_sans() {
    let f = open(&read("sans.ttf"), Limits::default()).unwrap();
    let face = &f.faces[0];
    assert_eq!(face.names.family, "Spec Sans");
    assert_eq!(face.names.designer, "Test Designer");
    assert!(face.names.licence.starts_with("This Font Software"));
    assert_eq!(face.names.licence_url, "https://openfontlicense.org");
    assert_eq!(face.names.version, "Version 1.002");
    assert_eq!(face.names.sample_text, "Aa Bb Oo 12");
    assert_eq!(face.outlines, OutlineKind::TrueType);
    assert_eq!(face.glyph_count, 12);
    assert_eq!(face.metrics.units_per_em, 1000);
    assert_eq!(face.metrics.cap_height, Some(700));
    assert_eq!(face.embedding.unwrap().permission, Permission::Installable);
    assert_eq!(face.glyph_index('A'), Some(2));
    assert_eq!(face.glyph_index('\u{1F600}'), Some(10));
    assert_eq!(face.glyph_name(9).as_deref(), Some("Aacute"));
    assert!(face.tables.iter().any(|t| t.tag_str() == "glyf"));
    assert!(!face.colour.any());
    // The composite: A and the accent.
    let a = face.outline(2).unwrap();
    let aa = face.outline(9).unwrap();
    assert!(aa.path.len() > a.path.len());
}

#[test]
fn embedding_flags() {
    let f = open(&read("locked.ttf"), Limits::default()).unwrap();
    let e = f.faces[0].embedding.unwrap();
    assert_eq!(e.permission, Permission::Restricted);
    assert!(e.no_subsetting && e.bitmap_only);
    assert_eq!(e.raw, 0x0302);
    let f = open(&read("edit.otf"), Limits::default()).unwrap();
    assert_eq!(
        f.faces[0].embedding.unwrap().permission,
        Permission::Editable
    );
    let f = open(&read("serif.otf"), Limits::default()).unwrap();
    assert_eq!(
        f.faces[0].embedding.unwrap().permission,
        Permission::PreviewPrint
    );
    assert_eq!(f.faces[0].outlines, OutlineKind::Cff);
}

#[test]
fn variable_fonts() {
    for (n, kind) in [
        ("var.ttf", OutlineKind::TrueType),
        ("var-cff2.otf", OutlineKind::Cff2),
    ] {
        let f = open(&read(n), Limits::default()).unwrap();
        let face = &f.faces[0];
        assert_eq!(face.outlines, kind, "{n}");
        assert_eq!(face.axes.len(), 1, "{n}");
        let a = &face.axes[0];
        assert_eq!(&a.tag, b"wght");
        assert_eq!(a.name, "Weight");
        assert_eq!((a.min, a.default, a.max), (300.0, 300.0, 700.0));
        let names: Vec<_> = face.instances.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["Light", "Regular", "Bold"], "{n}");
        assert_eq!(face.instances[2].coords, [700.0]);
        assert_eq!(
            face.instances[1].postscript.as_deref(),
            Some("SpecVar-Regular")
        );
    }
    assert!(open(&read("var.ttf"), Limits::default()).unwrap().faces[0].avar);
}

#[test]
fn collections_have_a_face_each() {
    for n in ["pair.ttc", "pair.otc", "pair.woff2"] {
        let f = open(&read(n), Limits::default()).unwrap();
        assert_eq!(f.faces.len(), 2, "{n}");
        assert_ne!(f.faces[0].names.family, f.faces[1].names.family, "{n}");
    }
    let f = open(&read("pair.ttc"), Limits::default()).unwrap();
    assert_eq!(f.faces[1].index(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &f.faces[0].sfnt(),
        &f.faces[1].sfnt()
    ));
}

#[test]
fn colour_tables() {
    let c = open(&read("colr0.ttf"), Limits::default()).unwrap().faces[0]
        .colour
        .clone();
    assert_eq!(c.colr, Some((0, 2)));
    assert_eq!(c.palettes.len(), 2);
    assert_eq!(c.palettes[0][0], [255, 0, 0, 255]);
    assert_eq!(c.palettes[0][2], [0, 0, 255, 128]);
    let c = open(&read("colr1.ttf"), Limits::default()).unwrap().faces[0]
        .colour
        .clone();
    assert_eq!(c.colr.map(|c| c.0), Some(1));
    assert_eq!(c.colr.map(|c| c.1), Some(2));
    let c = open(&read("sbix.ttf"), Limits::default()).unwrap().faces[0]
        .colour
        .clone();
    assert_eq!(c.sbix, [20, 40]);
    assert!(c.any());
}

#[test]
fn woff_metadata_and_private_data() {
    let f = open(&read("sans.woff"), Limits::default()).unwrap();
    assert!(f.woff_metadata.unwrap().contains("Specular"));
    assert_eq!(f.woff_private_len, 8);
}

/// Our WOFF2 decompression against fontTools': every table byte-equal except those WOFF2
/// rebuilds (`glyf`, `loca`, `hmtx`: compared as outlines, advances and side bearings by
/// the oracle) and the two `head` fields fontTools rewrites when it saves.
#[test]
fn woff2_matches_fonttools_decompression() {
    let mut tables = 0;
    let mut mismatches = 0;
    for n in ["sans.woff2", "sans-hmtx.woff2", "serif.woff2", "var.woff2"] {
        let ours = open(&read(n), Limits::default()).unwrap();
        let ours = &ours.faces[0];
        let theirs = open(&read(&format!("expected/{n}.sfnt")), Limits::default()).unwrap();
        let theirs = &theirs.faces[0];
        assert_eq!(ours.tables.len(), theirs.tables.len(), "{n}");
        for t in &theirs.tables {
            tables += 1;
            let a = ours.table(&t.tag).unwrap();
            let b = theirs.table(&t.tag).unwrap();
            let same = match &t.tag {
                b"glyf" | b"loca" => a.len() <= b.len() + b.len() / 2 + 64,
                b"hmtx" => a == b,
                // fontTools rewrites the checksum adjustment and the modified time.
                b"head" => {
                    a.len() == b.len()
                        && a[..8] == b[..8]
                        && a[12..28] == b[12..28]
                        && a[36..] == b[36..]
                }
                _ => a == b,
            };
            if !same {
                mismatches += 1;
                eprintln!("{n}: {} differs", t.tag_str());
            }
        }
        // Rebuilt glyphs: same outlines and side bearings.
        let (fa, fb) = (ours.sfnt(), theirs.sfnt());
        let a = ttf_parser::Face::parse(&fa, 0).unwrap();
        let b = ttf_parser::Face::parse(&fb, 0).unwrap();
        for g in 0..ours.glyph_count {
            let gid = ttf_parser::GlyphId(g);
            let pa = common::oracle_outline(&a, g);
            let pb = common::oracle_outline(&b, g);
            let same_outline = match (&pa, &pb) {
                (Some(x), Some(y)) => common::same_path(x, y),
                (None, None) => true,
                _ => false,
            };
            if !same_outline || a.glyph_hor_side_bearing(gid) != b.glyph_hor_side_bearing(gid) {
                mismatches += 1;
                eprintln!("{n}: glyph {g} differs");
            }
        }
    }
    eprintln!("woff2: {tables} tables, {mismatches} mismatches");
    assert_eq!(mismatches, 0);
}

/// Colour fonts thumbnail in colour, as the specimen shows them: COLR v0 layers in the first
/// palette, sbix bitmaps (they were black; the visual review) and CBDT bitmaps,
/// also in a face whose glyphs have no outlines (fidelity round 8).
#[test]
fn colour_previews_are_in_colour() {
    for n in [
        "colr0.ttf",
        "sbix.ttf",
        "colr1.ttf",
        "colr1-paints.ttf",
        "cbdt.ttf",
        "cbdt-only.ttf",
    ] {
        let f = open(&read(n), Limits::default()).unwrap();
        let px = specular_font::preview(&f.faces[0], 128, 96);
        let coloured = px
            .chunks_exact(4)
            .filter(|p| {
                let (mx, mn) = (p[0].max(p[1]).max(p[2]), p[0].min(p[1]).min(p[2]));
                mx - mn > 60
            })
            .count();
        assert!(coloured > 200, "{n}: {coloured} coloured pixels");
    }
}

/// COLR version 1 paint graphs (they thumbnailed black; fidelity round 6), `colr1-paints.ttf`
/// from fixtures/make.py: "A" a radial gradient from a red centre to blue under a rotation;
/// "a" a translated copy of "o", whose layer is a sweep gradient (red at 0°, blue at 360°,
/// counter-clockwise) with the "one" bar cut out of it by a DEST_OUT composite.
#[test]
fn colr_v1_paint_graphs() {
    let f = open(&read("colr1-paints.ttf"), Limits::default()).unwrap();
    let (w, h) = (256usize, 192usize);
    let px = specular_font::preview(&f.faces[0], w as u32, h as u32);
    let at = |x: usize, y: usize| -> [u8; 3] {
        let p = &px[(y * w + x) * 4..][..3];
        [p[0], p[1], p[2]]
    };
    let white = |p: [u8; 3]| p.iter().all(|&v| v > 235);
    let red = |p: [u8; 3]| p[0] > 150 && p[2] < 110 && p[1] < 80;
    let blue = |p: [u8; 3]| p[2] > 150 && p[0] < 110 && p[1] < 80;
    // The ink box of each half (the "A" left, the "a" right).
    let bbox = |x0: usize, x1: usize| {
        let (mut b, mut any) = ([usize::MAX, usize::MAX, 0, 0], false);
        for y in 0..h {
            for x in x0..x1 {
                if !white(at(x, y)) {
                    b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
                    any = true;
                }
            }
        }
        assert!(any, "no ink in {x0}..{x1}");
        b
    };
    // "A": red near the gradient's centre (the crossbar), blue at the feet.
    let a = bbox(0, w / 2);
    let reds = (a[1]..=a[3])
        .flat_map(|y| (a[0]..=a[2]).map(move |x| (x, y)))
        .filter(|&(x, y)| red(at(x, y)))
        .count();
    assert!(reds > 30, "radial centre: {reds} red pixels");
    assert!(blue(at(a[0] + 2, a[3] - 2)), "{:?}", at(a[0] + 2, a[3] - 2));
    // "a": the sweep, red just above the centre on the right, blue just below; the bar cut
    // out in the middle.
    let o = bbox(w / 2, w);
    let (cx, cy) = ((o[0] + o[2]) / 2, (o[1] + o[3]) / 2);
    let dx = (o[2] - o[0]) * 3 / 8;
    let dy = (o[3] - o[1]) / 8;
    assert!(red(at(cx + dx, cy - dy)), "{:?}", at(cx + dx, cy - dy));
    assert!(blue(at(cx + dx, cy + dy)), "{:?}", at(cx + dx, cy + dy));
    // The bar (x 240..340 of the moved "o", 80..540) is centred 20 units left of it.
    let bx = cx - (o[2] - o[0]) * 20 / 460;
    let hole = (o[1] + 4..o[3] - 4).filter(|&y| white(at(bx, y))).count();
    assert!(
        hole > (o[3] - o[1]) / 2,
        "the cut-out bar: {hole} white rows"
    );
}

#[test]
fn previews_draw_ink() {
    for n in ["sans.ttf", "serif.otf", "var-cff2.otf", "sans.woff2"] {
        let f = open(&read(n), Limits::default()).unwrap();
        let px = specular_font::preview(&f.faces[0], 64, 48);
        assert_eq!(px.len(), 64 * 48 * 4);
        let dark = px.chunks_exact(4).filter(|p| p[0] < 128).count();
        assert!(dark > 100 && dark < 64 * 48 / 2, "{n}: {dark} dark pixels");
        assert!(px.chunks_exact(4).all(|p| p[3] == 255));
    }
}
