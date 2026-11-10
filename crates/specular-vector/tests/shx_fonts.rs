//! The SHX reader on our own test fonts (tests/data/shx, written by
//! tools/corpus/shx-fixtures.py) against the independent reader in tools/corpus/shx-oracle.py
//! (its output is tests/data/shx/oracle.json), and SHX text through layout and preparation.

use std::path::PathBuf;
use std::sync::Arc;

use kurbo::{Affine, Shape as _};
use specular_vector::fonts::{Fonts, attach_shx_in, missing_fonts};
use specular_vector::prep::{Kind, PrepLimits, layout, prepare};
use specular_vector::shx::{ShxFont, ShxKind};
use specular_vector::{
    Color, FontSpec, Item, Paragraph, Shape, Sheet, SheetKind, Span, TextBlock, VectorScene,
};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/shx")
}

fn font(name: &str) -> ShxFont {
    ShxFont::parse(&std::fs::read(dir().join(name)).unwrap()).unwrap()
}

/// Every shape of every test font: the pen's end and the strokes' bounds as the oracle
/// computes them (arcs sampled every degree there, exact here).
#[test]
fn matches_the_oracle() {
    let oracle: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir().join("oracle.json")).unwrap()).unwrap();
    let mut shapes = 0;
    let mut mismatches = Vec::new();
    for (file, glyphs) in oracle.as_object().unwrap() {
        let f = font(file);
        assert_eq!(f.len(), glyphs.as_object().unwrap().len(), "{file}");
        for (code, want) in glyphs.as_object().unwrap() {
            shapes += 1;
            let code: u32 = code.parse().unwrap();
            let (path, end) = f.render(code).expect("shape");
            let num = |v: &serde_json::Value| v.as_f64().unwrap();
            let we = want["end"].as_array().unwrap();
            if (end.x - num(&we[0])).abs() > 1e-6 || (end.y - num(&we[1])).abs() > 1e-6 {
                mismatches.push(format!("{file} {code}: end {end:?} vs {we:?}"));
            }
            match want["bbox"].as_array() {
                None => {
                    if !path.elements().is_empty() {
                        mismatches.push(format!("{file} {code}: strokes where none"));
                    }
                }
                Some(b) => {
                    let bb = path.bounding_box();
                    let got = [bb.x0, bb.y0, bb.x1, bb.y1];
                    if got.iter().zip(b).any(|(g, w)| (g - num(w)).abs() > 0.01) {
                        mismatches.push(format!("{file} {code}: bbox {got:?} vs {b:?}"));
                    }
                }
            }
        }
    }
    assert!(shapes >= 25, "{shapes}");
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn kinds_and_metrics() {
    let t = font("sample-text.shx");
    assert_eq!((t.kind, t.above, t.below), (ShxKind::Shapes, 6, 2));
    assert_eq!(t.name, "Specular test text");
    let b = font("sample-big.shx");
    assert_eq!(b.kind, ShxKind::Bigfont);
    assert!(b.is_lead(0x93) && b.is_lead(0xE5) && !b.is_lead(0x41));
    assert_eq!(b.extended, None);
    let e = font("sample-extbig.shx");
    assert_eq!(e.extended, Some((16, 16)));
    let u = font("sample-uni.shx");
    assert_eq!(u.kind, ShxKind::Unifont);
    assert!(u.has(0x3A9) && u.has(0x2205));
}

fn spec(file: &str, big: &str) -> FontSpec {
    FontSpec {
        style: format!("S-{file}"),
        file: file.into(),
        width_factor: 1.0,
        bigfont: big.into(),
        ..FontSpec::default()
    }
}

fn text_item(s: &str, font: u32) -> Item {
    Item {
        layer: 0,
        outer: 0,
        color: Color::Foreground,
        shape: Shape::Text(Box::new(TextBlock {
            transform: Affine::IDENTITY,
            halign: Default::default(),
            valign: Default::default(),
            wrap: 0.0,
            fit: None,
            line_spacing: 1.0,
            paragraphs: vec![Paragraph {
                spans: vec![Span::new(s, font, 3.0)],
                align: None,
            }],
        })),
    }
}

fn scene(fonts: Vec<FontSpec>, items: Vec<Item>) -> VectorScene {
    let mut sheet = Sheet::new("Model", SheetKind::Model);
    sheet.items = items;
    VectorScene {
        fonts,
        sheets: vec![sheet],
        ..VectorScene::default()
    }
}

/// Text in an SHX style is drawn with the font's shapes, as strokes; the Big Font draws the
/// double-byte characters through the drawing's code page.
#[test]
fn shx_text_is_stroked_shapes() {
    let fonts = Fonts::bundled_only();
    let mut sc = scene(
        vec![
            spec("SAMPLE-TEXT", "sample-big.shx"),
            spec("sample-uni.shx", ""),
        ],
        vec![text_item("HI 日本", 0), text_item("AΩ⌀", 1)],
    );
    attach_shx_in(&mut sc, &[dir()], Some(encoding_rs_sjis()));
    let r = sc.fonts[0].shx.as_ref().unwrap();
    assert!(r.main.is_some() && r.big.is_some() && r.missing.is_empty());
    assert!(missing_fonts(&fonts, &sc.fonts).is_empty());

    let Shape::Text(t) = &sc.sheets[0].items[0].shape else {
        unreachable!()
    };
    let laid = layout(t, &sc.fonts, &fonts);
    assert_eq!(laid.glyphs.len(), 5);
    assert!(laid.glyphs.iter().all(|g| g.glyph.stroke));
    // Height 3 on a cap height of 6: H is 6 units wide → 3, I 2 → 1, space 6 → 3; the Big
    // Font's 8-unit cap scales to the main font's 6 (日 10 → 7.5).
    let xs: Vec<f64> = laid
        .glyphs
        .iter()
        .map(|g| (g.place * kurbo::Point::ZERO).x)
        .collect();
    assert_eq!(xs, vec![0.0, 3.0, 4.0, 7.0, 7.0 + 3.75]);

    let Shape::Text(t) = &sc.sheets[0].items[1].shape else {
        unreachable!()
    };
    let laid = layout(t, &sc.fonts, &fonts);
    assert_eq!(laid.glyphs.len(), 3);
    assert!(
        laid.glyphs.iter().all(|g| g.glyph.stroke),
        "⌀ through U+2205"
    );

    // Prepared: strokes, no filled glyphs.
    let p = prepare(Arc::new(sc), &fonts, PrepLimits::default());
    let items = &p.sheets[0].passes[0].geometry.items;
    assert!(items.iter().all(|i| i.kind == Kind::Stroke));
    assert_eq!(items.len(), 2, "one stroked path per text");
    assert!(p.missing_fonts.is_empty());
}

fn encoding_rs_sjis() -> &'static encoding_rs::Encoding {
    encoding_rs::SHIFT_JIS
}

/// Fonts not found: reported with their styles and the substitute, and drawn with it.
#[test]
fn missing_fonts_are_reported_and_substituted() {
    let fonts = Fonts::bundled_only();
    let mut a = spec("romans", "");
    a.style = "Annotation".into();
    let mut b = spec("ROMANS.SHX", "");
    b.style = "Notes".into();
    let c = spec("sample-text.shx", "nothere-big.shx");
    let mut sc = scene(
        vec![a, b, c],
        vec![text_item("A", 0), text_item("B", 1), text_item("日", 2)],
    );
    attach_shx_in(&mut sc, &[dir()], Some(encoding_rs_sjis()));
    let m = missing_fonts(&fonts, &sc.fonts);
    let files: Vec<&str> = m.iter().map(|m| m.file.as_str()).collect();
    assert_eq!(files, vec!["nothere-big.shx", "romans.shx"]);
    assert_eq!(m[1].styles, vec!["Annotation", "Notes"]);
    assert_eq!(m[1].substitute, fonts.default_face(false).family);
    // The main font was found: only the Big Font is missing, no substitute named.
    assert_eq!(m[0].styles, vec!["S-sample-text.shx"]);
    assert_eq!(m[0].substitute, "");
    let Shape::Text(t) = &sc.sheets[0].items[0].shape else {
        unreachable!()
    };
    let laid = layout(t, &sc.fonts, &fonts);
    assert!(laid.glyphs.iter().all(|g| !g.glyph.stroke));
}
