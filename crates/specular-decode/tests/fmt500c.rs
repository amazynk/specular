//! Home computer screens, Photo CD, PostScript refusals, Type 1 fonts, DVI and Alembic
//! through the registry: recognised, opened as the right format, drawn, with Info facts.

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};
use std::sync::Arc;

fn open(name: &str, bytes: Vec<u8>) -> specular_decode::Result<Arc<dyn Session>> {
    Registry::new().open(Arc::new(Source::from_vec(name, bytes)))
}

fn decode(s: &dyn Session) -> specular_core::Raster {
    s.decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap()
}

fn field<'a>(s: &'a dyn Session, name: &str) -> Option<&'a str> {
    s.header()?
        .fields
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn screens_by_extension_and_header() {
    let s = open("a.scr", (0..16384u32).map(|i| (i % 7) as u8).collect()).unwrap();
    assert_eq!(s.info().format, Format::CpcScreen);
    assert_eq!(s.page_count(), 3);
    assert_eq!(field(s.as_ref(), "Machine"), Some("Amstrad CPC"));
    let r = decode(s.as_ref());
    assert_eq!((r.width, r.height), (320, 200));
    let p2 = s.open_page(2).unwrap();
    assert_eq!((p2.info().width, p2.info().height), (640, 400));
    // A Windows screen saver named .scr is refused in plain words.
    let mut exe = b"MZ".to_vec();
    exe.resize(16384, 0);
    match open("saver.scr", exe) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("screen saver"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
    // An MSX screen 5 with its BSAVE header, under any name.
    let mut sc5 = vec![0xFE, 0, 0, 0xFF, 0x69, 0, 0];
    sc5.resize(7 + 0x6A00, 0x11);
    let s = open("pic.sc5", sc5).unwrap();
    assert_eq!(s.info().format, Format::MsxScreen);
    assert_eq!(decode(s.as_ref()).width, 256);
    for (name, len, format) in [
        ("a.hgr", 8192, Format::AppleHgr),
        ("a.dhr", 16384, Format::AppleDhr),
        ("a.a2fc", 16384, Format::AppleDhr),
        ("a.bb1", 20480, Format::BbcScreen),
        ("a.bb5", 10240, Format::BbcScreen),
        ("a.mic", 7684, Format::AtariMic),
        ("a.gr8", 7680, Format::AtariGr8),
        ("a.gr9", 7680, Format::AtariGr9),
    ] {
        let s = open(name, vec![0xA5; len]).unwrap();
        assert_eq!(s.info().format, format, "{name}");
        let r = decode(s.as_ref());
        assert_eq!((r.width, r.height), (s.info().width, s.info().height));
        assert!(field(s.as_ref(), "Machine").is_some(), "{name}");
    }
    assert!(matches!(
        open("a.gr8", vec![0; 100]),
        Err(DecodeError::Unsupported(_))
    ));
}

fn photo_cd() -> Vec<u8> {
    let mut d = vec![128u8; 96 * 2048 + 768 * 512 * 3 / 2];
    d[..2048].fill(0);
    d[2048..2055].copy_from_slice(b"PCD_IPI");
    d[0xE02] = 0;
    d
}

#[test]
fn photo_cd_and_point_clouds_share_pcd() {
    let s = open("scan.pcd", photo_cd()).unwrap();
    assert_eq!(s.info().format, Format::PhotoCd);
    assert_eq!((s.info().width, s.info().height), (768, 512));
    let p = s.decode_preview().unwrap().unwrap();
    assert_eq!((p.width, p.height), (192, 128));
    assert!(field(s.as_ref(), "Resolutions").is_some());
    let cloud = b"# .PCD v0.7\nVERSION 0.7\nFIELDS x y z\nSIZE 4 4 4\nTYPE F F F\nCOUNT 1 1 1\nWIDTH 1\nHEIGHT 1\nVIEWPOINT 0 0 0 1 0 0 0\nPOINTS 1\nDATA ascii\n1 2 3\n";
    let s = open("cloud.pcd", cloud.to_vec()).unwrap();
    assert_eq!(s.info().format, Format::Pcd);
}

#[test]
fn postscript_is_refused_with_the_reason() {
    match open("doc.ps", b"%!PS-Adobe-3.0\n%%Pages: 1\nshowpage\n".to_vec()) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("PostScript interpreter"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
    match open("doc.ps", b"just text".to_vec()) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("not PostScript")),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn type1_fonts() {
    for (name, pfb) in [("tiny.pfb", true), ("tiny.pfa", false)] {
        let s = open(name, specular_font::type1_fixture::tiny_font(pfb)).unwrap();
        assert_eq!(s.info().format, Format::Type1Font);
        assert!(s.info().coding.contains("Type 1"), "{}", s.info().coding);
        let face = s.font().unwrap().face();
        assert!(face.glyph_index('A').is_some());
        decode(s.as_ref());
    }
}

#[test]
fn dvi_pages() {
    let s = open("paper.dvi", specular_dvi::fixtures::sample()).unwrap();
    assert_eq!(s.info().format, Format::Dvi);
    assert_eq!(s.page_count(), 2);
    assert_eq!(field(s.as_ref(), "Pages"), Some("2"));
    assert_eq!(field(s.as_ref(), "Comment"), Some("TeX output 2026.11.07"));
    // Letter paper at 200 dpi.
    assert_eq!((s.info().width, s.info().height), (1700, 2200));
    let r = decode(s.as_ref());
    // The rule (50000 sp wide, 1000 sp tall, from the 1 in margin) is drawn in black.
    let p = &r.planes[0];
    let at = |x: usize, y: usize| p.data[y * p.stride + x * 4];
    assert!(at(201, 200) < 64);
    assert_eq!(at(1000, 1000), 255);
    let p2 = s.open_page(1).unwrap();
    assert!(p2.info().coding.contains("page 2"));
    let mut cut = specular_dvi::fixtures::sample();
    cut.truncate(20);
    assert!(open("cut.dvi", cut).is_err());
}

#[test]
fn alembic_meshes() {
    let s = open("scene.abc", specular_mesh::fixtures::alembic()).unwrap();
    assert_eq!(s.info().format, Format::Alembic);
    assert_eq!(s.mesh().unwrap().mesh.triangle_count(), 2);
    assert_eq!(field(s.as_ref(), "/xf/quad"), Some("PolyMesh"));
    match open("old.abc", b"\x89HDF\r\n\x1a\n\0\0\0\0\0\0\0\0".to_vec()) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("HDF5"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
    // ABC music notation is not an Alembic archive.
    assert!(open("tune.abc", b"X:1\nT:Tune\nK:C\nCDEF|\n".to_vec()).is_err());
}

/// A plain TeX page (tools/corpus/dvi-oracle.py) drawn with the system's TeX fonts.
#[test]
#[ignore]
fn dvi_with_system_fonts() {
    let Some(root) = std::env::var_os("SPECULAR_DVI_CORPUS") else {
        eprintln!("SPECULAR_DVI_CORPUS not set");
        return;
    };
    let path = std::path::Path::new(&root).join("plain.dvi");
    let t0 = std::time::Instant::now();
    let s = Registry::new()
        .open(Arc::new(Source::open(&path).unwrap()))
        .unwrap();
    let r = decode(s.as_ref());
    eprintln!(
        "plain.dvi: {}x{}, drawn: {:?}, boxes: {:?}, {:.0} ms",
        r.width,
        r.height,
        field(s.as_ref(), "Fonts drawn"),
        field(s.as_ref(), "Fonts shown as boxes"),
        t0.elapsed().as_secs_f64() * 1e3
    );
    assert!(field(s.as_ref(), "Fonts drawn").is_some_and(|f| f.contains("cmr10")));
    let ink = r.planes[0]
        .data
        .chunks_exact(4)
        .filter(|p| p[0] < 128)
        .count();
    assert!(ink > 500, "{ink}");
}
