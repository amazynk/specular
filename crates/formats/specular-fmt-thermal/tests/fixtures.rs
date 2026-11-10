//! Structure of the synthetic fixtures (gen/make_fixtures.py): kinds, records, metadata.

use std::path::Path;

use specular_fmt_thermal::{Error, Kind, Pixels, read, sniff};

fn file(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

#[test]
fn kinds() {
    for (name, kind) in [
        ("fff_raw_le.fff", Kind::FlirFff),
        ("fff_png.fff", Kind::FlirFff),
        ("rjpeg.jpg", Kind::FlirJpeg),
        ("flir_noraw.jpg", Kind::FlirJpeg),
        ("irg_c201.irg", Kind::InfirayIrg),
        ("irg_p200.irg", Kind::InfirayIrg),
        ("irb_c2.irb", Kind::InfratecIrb),
    ] {
        assert_eq!(sniff(&file(name)), Some(kind), "{name}");
    }
    assert_eq!(read(&file("flir_noraw.jpg")), Err(Error::NoRawData));
}

#[test]
fn flir_records() {
    let t = read(&file("rjpeg.jpg")).unwrap();
    assert_eq!(t.kind, Kind::FlirJpeg);
    assert_eq!((t.width, t.height), (16, 12));
    assert_eq!(t.camera.model, "Synthetic FLIR");
    assert_eq!(t.camera.lens, "FOL7");
    assert_eq!(t.camera.serial, "12345678");
    assert_eq!(t.camera.field_of_view, Some(45.0));
    // exiftool: 2023:11:14 23:13:20.250+01:00
    assert_eq!(
        t.camera.date.as_deref(),
        Some("2023-11-14 23:13:20.250 +01:00")
    );
    let (lo, hi) = t.camera.range.unwrap();
    assert!((lo + 20.0).abs() < 1e-3 && (hi - 150.0).abs() < 1e-3);
    let p = t.palette.as_ref().unwrap();
    assert_eq!(p.name, "Test");
    assert_eq!(p.colours.len(), 16);
    // Black to yellow-white through red, within YCrCb rounding.
    let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) <= 3);
    assert!(near(p.colours[0], [0, 0, 0]), "{:?}", p.colours[0]);
    assert!(near(p.colours[15], [255, 255, 255]), "{:?}", p.colours[15]);
    assert!(
        p.colours[7][0] > 200 && p.colours[7][2] < 30,
        "{:?}",
        p.colours[7]
    );
    let v = t.visible.as_ref().unwrap();
    assert!(v.bytes.starts_with(&[0xff, 0xd8]));
    assert_eq!((v.width, v.height), (24, 18));
    let pip = t.pip.unwrap();
    assert_eq!(
        (pip.real_to_ir, pip.offset_x, pip.offset_y, pip.x2, pip.y2),
        (1.5, 3, -2, 15, 11)
    );
    assert!(t.adjustable());
    let fff = read(&file("fff_png.fff")).unwrap();
    assert_eq!(fff.pixels, t.pixels);
}

#[test]
fn every_raw_layout_reads_the_same_counts() {
    let base = read(&file("fff_raw_le.fff")).unwrap();
    for name in [
        "fff_raw_be.fff",
        "fff_png.fff",
        "fff_tiff_le.fff",
        "fff_tiff_be.fff",
    ] {
        assert_eq!(read(&file(name)).unwrap().pixels, base.pixels, "{name}");
    }
    assert_eq!(base.raw_range().map(|(lo, hi)| lo < hi), Some(true));
}

#[test]
fn irg_and_irb_metadata() {
    let g = read(&file("irg_c201.irg")).unwrap();
    assert_eq!(g.conditions.emissivity, Some(0.95));
    assert_eq!(g.conditions.distance, Some(2.5));
    assert!(
        g.visible
            .as_ref()
            .is_some_and(|v| v.bytes.starts_with(&[0xff, 0xd8]))
    );
    assert!(read(&file("irg_p200.irg")).unwrap().visible.is_none());

    let b = read(&file("irb_c0.irb")).unwrap();
    assert!(matches!(b.pixels, Pixels::Celsius(_)));
    assert_eq!(b.camera.model, "VarioTEST");
    assert_eq!(b.camera.serial, "SN0042");
    assert_eq!(b.camera.lens, "OPTIC 30 30 mm");
    assert_eq!(b.camera.wavelength, Some(9.5));
    assert!((b.conditions.emissivity.unwrap() - 0.93).abs() < 1e-6);
    assert!((b.conditions.reflected.unwrap() - 23.0).abs() < 1e-3);
    // OLE date 45000.5 = 2023-03-15 12:00.
    assert_eq!(b.camera.date.as_deref(), Some("2023-03-15 12:00:00.123"));
    let c2 = read(&file("irb_c2.irb")).unwrap();
    assert!(matches!(c2.pixels, Pixels::Counts { .. }));
}
