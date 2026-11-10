//! JPEG XL transfer syntaxes (Supplement 232) and Real World Value Mapping against
//! tests/data/jxl_rwvm_oracle.tsv (tools/corpus/dicomwsi-oracle.py --dicom: libjxl through
//! imagecodecs for the frames, highdicom's RealWorldValueMapping for the values). The fixtures
//! are written by tools/corpus/dicomwsi-fixtures.py.

use specular_dicom::{Dicom, Pixels, RealWorldFunction};

fn file(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

fn oracle(kind: &str) -> Vec<Vec<&'static str>> {
    include_str!("data/jxl_rwvm_oracle.tsv")
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .filter(|f| f[0] == kind)
        .collect()
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

#[test]
fn jpeg_xl_frames_match_libjxl() {
    let lines = oracle("jxl");
    assert_eq!(lines.len(), 3);
    for f in lines {
        let name = f[1];
        let data = file(name);
        let d = Dicom::parse(&data).unwrap();
        assert!(d.transfer_syntax.name.starts_with("JPEG XL"), "{name}");
        d.check_supported().unwrap();
        let fr = d.decode_frame(&data, 0, None).unwrap();
        let (w, h, ch, bits): (u32, u32, usize, u32) = (
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
            f[4].parse().unwrap(),
            f[5].parse().unwrap(),
        );
        assert_eq!((fr.width, fr.height), (w, h), "{name}");
        assert!(!fr.truncated);
        let want = hex(f[6]);
        let want: Vec<i64> = if bits == 16 {
            want.chunks_exact(2)
                .map(|c| i64::from(u16::from_le_bytes([c[0], c[1]])))
                .collect()
        } else {
            want.iter().map(|&b| i64::from(b)).collect()
        };
        let got: Vec<i64> = match &fr.pixels {
            Pixels::Grey(g) => {
                assert_eq!(ch, 1);
                (0..(w * h) as usize).map(|i| g.get(i).unwrap()).collect()
            }
            Pixels::Rgb8(p) => {
                assert_eq!(ch, 3);
                p.iter().map(|&b| i64::from(b)).collect()
            }
            Pixels::Rgb16(p) => p.iter().map(|&b| i64::from(b)).collect(),
        };
        let mismatches = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        assert_eq!(got.len(), want.len(), "{name}");
        // Both lossless and lossy decode to libjxl's samples (the lossy one is
        // decoded the same way by any conforming decoder, within rounding).
        let tolerance = if name.contains("lossy") { 1 } else { 0 };
        let far = got
            .iter()
            .zip(&want)
            .filter(|(a, b)| a.abs_diff(**b) > tolerance)
            .count();
        eprintln!("{name}: {mismatches} samples differ, {far} by more than {tolerance}");
        assert_eq!(far, 0, "{name}");
    }
}

#[test]
fn jpeg_xl_grey_keeps_twelve_bits() {
    let data = file("jxl-lossless-grey12.dcm");
    let d = Dicom::parse(&data).unwrap();
    let fr = d.decode_frame(&data, 0, None).unwrap();
    assert_eq!(fr.bits, 12);
    let Pixels::Grey(g) = &fr.pixels else {
        panic!("grey expected");
    };
    // The fixture formula at (5, 3).
    let (x, y) = (5i64, 3i64);
    assert_eq!(
        g.get(3 * 24 + 5),
        Some((x * 157 + y * 263 + (x * y) % 13 * 31) % 4096)
    );
}

#[test]
fn real_world_values_match_highdicom() {
    let data = file("rwvm.dcm");
    let d = Dicom::parse(&data).unwrap();
    let m = d.modality_frame(&data, 0, None).unwrap();
    let maps = m.real_world();
    assert_eq!(maps.len(), 3);
    assert_eq!((maps[0].first, maps[0].last), (-1000.0, -1.0));
    assert_eq!(
        maps[0].function,
        RealWorldFunction::Linear {
            slope: 0.5,
            intercept: 2.5
        }
    );
    assert!(matches!(&maps[1].function, RealWorldFunction::Lut(l) if l.len() == 8));
    let mut checked = 0;
    for f in oracle("rwvm") {
        let (x, y): (u32, u32) = (f[2].parse().unwrap(), f[3].parse().unwrap());
        let stored: i64 = f[4].parse().unwrap();
        assert_eq!(m.value_at(x, y), Some(stored as f64), "stored at {x},{y}");
        let got = m.real_world_at(x, y);
        if f[5] == "-" {
            assert!(got.is_none(), "{x},{y}");
            continue;
        }
        let want: f64 = f[5].parse().unwrap();
        let got = got.unwrap_or_else(|| panic!("no value at {x},{y}"));
        assert!(
            (got.value - want).abs() <= 1e-9 * want.abs().max(1.0),
            "{x},{y}: {} vs {want}",
            got.value
        );
        // Label and unit per item: LUT Label and UCUM; the quantity and no unit ("1"); the
        // unit's code meaning when it is not UCUM.
        let (label, unit) = match stored {
            -1000..=-1 => (Some("ADC"), Some("um2/s")),
            0..=7 => (Some("Ratio"), None),
            _ => (None, Some("second")),
        };
        assert_eq!(got.label.as_deref(), label, "{x},{y}");
        assert_eq!(got.unit.as_deref(), unit, "{x},{y}");
        checked += 1;
    }
    assert_eq!(checked, 32);
}

#[test]
fn no_mapping_no_real_world_values() {
    let data = file("jxl-lossless-grey12.dcm");
    let d = Dicom::parse(&data).unwrap();
    let m = d.modality_frame(&data, 0, None).unwrap();
    assert!(m.real_world().is_empty());
    assert!(m.real_world_at(0, 0).is_none());
}
