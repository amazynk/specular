//! TEXT, MTEXT and dimension text in SHX styles, drawn with the shapes of our own test
//! fonts (specular-vector's tests/data/shx); fonts not found are reported and substituted.

use std::path::PathBuf;
use std::sync::Arc;

use specular_cad::{Limits, codepage_encoding, parse};
use specular_vector::fonts::{Fonts, attach_shx_in, missing_fonts};
use specular_vector::prep::{Kind, PrepLimits, prepare};

fn fonts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../specular-vector/tests/data/shx")
}

fn style(name: &str, file: &str, big: &str) -> String {
    format!("0\nSTYLE\n2\n{name}\n70\n0\n40\n0\n41\n1\n50\n0\n3\n{file}\n4\n{big}\n")
}

fn drawing(styles: &[String]) -> String {
    let mut f = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$DWGCODEPAGE\n3\nANSI_932\n0\nENDSEC\n",
    );
    f += "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nSTYLE\n";
    for s in styles {
        f += s;
    }
    f += "0\nENDTAB\n0\nENDSEC\n";
    // The dimension's text lives in its anonymous block, in the dimension's style.
    f += "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\n*D1\n70\n1\n10\n0\n20\n0\n";
    f += "0\nMTEXT\n8\n0\n10\n0\n20\n-10\n40\n2\n7\nDIM\n1\nTOL\n0\nENDBLK\n0\nENDSEC\n";
    f += "0\nSECTION\n2\nENTITIES\n";
    f += "0\nTEXT\n8\n0\n10\n0\n20\n0\n40\n3\n7\nPLAIN\n1\nHI LOT\n";
    f += "0\nMTEXT\n8\n0\n10\n0\n20\n10\n40\n3\n7\nKANJI\n1\nT\\P\\U+65E5\\U+672C\n";
    f += "0\nDIMENSION\n8\n0\n2\n*D1\n10\n0\n20\n0\n";
    f += "0\nENDSEC\n0\nEOF\n";
    f
}

#[test]
fn shx_styles_draw_their_shapes() {
    let styles = [
        style("PLAIN", "sample-text", ""),
        style("KANJI", "sample-text.shx", "sample-big.shx"),
        style("DIM", "SAMPLE-TEXT.SHX", ""),
    ];
    let mut scene = parse(drawing(&styles).as_bytes(), &Limits::default()).unwrap();
    assert_eq!(scene.fonts[1].bigfont, "sample-big.shx");
    let enc = scene.codepage.as_deref().map(codepage_encoding);
    attach_shx_in(&mut scene, &[fonts_dir()], enc);
    let fonts = Fonts::bundled_only();
    assert!(missing_fonts(&fonts, &scene.fonts).is_empty());
    let p = prepare(Arc::new(scene), &fonts, PrepLimits::default());
    let items = &p.sheets[0].passes[0].geometry.items;
    // TEXT, MTEXT (two lines) and the dimension's MTEXT: strokes only, no filled glyphs.
    assert!(!items.is_empty());
    assert!(items.iter().all(|i| i.kind == Kind::Stroke), "{items:?}");
}

#[test]
fn missing_shx_fonts_are_reported_with_their_styles() {
    let styles = [
        style("PLAIN", "romans.shx", ""),
        style("KANJI", "sample-text.shx", "extfont2.shx"),
        style("DIM", "romans", ""),
    ];
    let mut scene = parse(drawing(&styles).as_bytes(), &Limits::default()).unwrap();
    let enc = scene.codepage.as_deref().map(codepage_encoding);
    attach_shx_in(&mut scene, &[fonts_dir()], enc);
    let fonts = Fonts::bundled_only();
    let m = missing_fonts(&fonts, &scene.fonts);
    assert_eq!(m.len(), 2, "{m:?}");
    assert_eq!(m[0].file, "extfont2.shx");
    assert_eq!(m[0].styles, vec!["KANJI"]);
    assert_eq!(
        m[0].substitute, "",
        "the main font draws; Big Font text falls back"
    );
    assert_eq!(m[1].file, "romans.shx");
    assert_eq!(m[1].styles, vec!["PLAIN", "DIM"]);
    assert!(!m[1].substitute.is_empty());
    let p = prepare(Arc::new(scene), &fonts, PrepLimits::default());
    let items = &p.sheets[0].passes[0].geometry.items;
    // romans substituted: filled glyphs; KANJI's Latin from its SHX: strokes.
    assert!(items.iter().any(|i| i.kind == Kind::Glyph));
    assert!(items.iter().any(|i| i.kind == Kind::Stroke));
    assert_eq!(p.missing_fonts, m);
}
