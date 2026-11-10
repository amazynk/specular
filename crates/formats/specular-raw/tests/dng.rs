//! Synthetic DNG round trips: parse → decode_raw (bit exact) → develop (sane values).

mod common;

use common::{DngCodec, DngSpec, XYZ_TO_SRGB, patch_tag, test_pattern, write_dng};
use specular_raw::{
    DecodeOptions, MatrixSource, OutputSize, RawFormat, Rect, SensorCompression, WbSource,
};

fn check_roundtrip(spec: &DngSpec) {
    let cpp = if spec.linear_raw { 3 } else { 1 };
    let data = test_pattern(
        spec.width as usize,
        spec.height as usize,
        cpp,
        spec.bits,
        spec.width,
    );
    let file = write_dng(spec, &data);
    let raw = specular_raw::parse(&file).expect("parse");
    let info = raw.info();
    assert_eq!(info.format, Some(RawFormat::Dng));
    assert_eq!(info.make, "Specular");
    assert_eq!(info.orientation, spec.orientation);
    let s = info.sensor.as_ref().expect("sensor");
    assert!(s.decodable, "{:?}", s.unsupported_reason);
    assert_eq!(
        (s.width, s.height, s.samples_per_pixel),
        (spec.width, spec.height, cpp as u32)
    );
    let want_comp = match spec.codec {
        DngCodec::U16 | DngCodec::Packed(_) => SensorCompression::Uncompressed,
        _ => SensorCompression::LosslessJpeg,
    };
    assert_eq!(s.compression, want_comp);
    for parallel in [false, true] {
        let opts = DecodeOptions {
            parallel,
            ..Default::default()
        };
        let img = raw.decode_raw(&file, &opts).expect("decode_raw");
        assert!(!img.truncated);
        assert_eq!(
            (img.width, img.height, img.cpp),
            (spec.width, spec.height, cpp as u32)
        );
        assert!(img.data == data, "{:?}: sample mismatch", spec.codec);
        for size in [OutputSize::Full, OutputSize::Half] {
            let o = DecodeOptions { size, ..opts };
            let rgb = specular_raw::develop(&img, &o).expect("develop");
            let (cw, ch) = (img.crop.w, img.crop.h);
            let (ew, eh) = match size {
                OutputSize::Full => (cw, ch),
                OutputSize::Half => (cw / 2, ch / 2),
            };
            assert_eq!((rgb.width, rgb.height), (ew, eh));
            assert_eq!(rgb.data.len(), (ew * eh * 3) as usize);
            assert_eq!(rgb.wb_source, WbSource::AsShot);
            // The pattern is mid-grey on average.
            let mean = rgb.data.iter().map(|&v| v as f64).sum::<f64>() / rgb.data.len() as f64;
            assert!(mean > 5000.0 && mean < 60000.0, "mean {mean}");
        }
    }
}

#[test]
fn uncompressed_16() {
    check_roundtrip(&DngSpec::new(64, 40, DngCodec::U16));
}

#[test]
fn packed_bits() {
    for bits in [8, 10, 12, 14] {
        let mut s = DngSpec::new(62, 30, DngCodec::Packed(bits));
        s.bits = bits;
        s.white = (1 << bits) - 1;
        check_roundtrip(&s);
    }
}

#[test]
fn lj92_strip() {
    let mut s = DngSpec::new(60, 34, DngCodec::Lj92Strip);
    s.bits = 14;
    s.white = 16383;
    check_roundtrip(&s);
}

#[test]
fn lj92_tiles_with_partial_edges() {
    for (w, h) in [(64, 64), (100, 70), (33, 17)] {
        let mut s = DngSpec::new(w, h, DngCodec::Lj92Tiles(32));
        s.bits = 12;
        s.white = 4095;
        s.orientation = 6;
        check_roundtrip(&s);
    }
}

#[test]
fn lj92_tile_frame_shapes_and_scans() {
    for (wide, predictor, restart) in [(true, 1, 0), (true, 6, 0), (false, 1, 3), (true, 1, 5)] {
        for (w, h) in [(64, 64), (50, 37)] {
            let codec = DngCodec::Lj92TilesFrame {
                tile: 16,
                wide,
                predictor,
                restart,
            };
            let mut s = DngSpec::new(w, h, codec);
            s.bits = 14;
            s.white = 16383;
            check_roundtrip(&s);
        }
    }
}

#[test]
fn linear_raw_lj92_tiles() {
    let mut s = DngSpec::new(48, 40, DngCodec::Lj92Tiles(16));
    s.linear_raw = true;
    check_roundtrip(&s);
}

#[test]
fn other_cfa_layouts() {
    for cfa in [[2, 1, 1, 0], [1, 0, 2, 1], [1, 2, 0, 1]] {
        let mut s = DngSpec::new(40, 24, DngCodec::U16);
        s.cfa = cfa;
        check_roundtrip(&s);
    }
}

fn uniform(spec: &DngSpec, v: u16) -> specular_raw::RgbImage {
    let cpp = if spec.linear_raw { 3 } else { 1 };
    let data = vec![v; (spec.width * spec.height) as usize * cpp];
    let file = write_dng(spec, &data);
    specular_raw::parse(&file)
        .unwrap()
        .decode(&file, &DecodeOptions::default())
        .unwrap()
}

/// Neutral input stays neutral, and black/white levels map to 0..65535.
#[test]
fn grey_levels_and_white_balance() {
    let mut s = DngSpec::new(32, 32, DngCodec::Packed(14));
    s.bits = 14;
    s.black = 1000;
    s.white = 15000;
    s.color_matrix = XYZ_TO_SRGB;
    let img = uniform(&s, 8000); // exactly half way
    assert_eq!(img.matrix_source, MatrixSource::DngColorMatrix);
    for px in img.data.chunks_exact(3) {
        for &c in px {
            assert!((c as i32 - 32768).abs() < 300, "{px:?}");
        }
    }
    // Below black clips to 0, above white to 65535.
    assert!(uniform(&s, 500).data.iter().all(|&v| v == 0));
    assert!(uniform(&s, 16000).data.iter().all(|&v| v == 65535));
}

/// AsShotNeutral is applied: a camera neutral of (0.5, 1, 0.8) renders grey.
#[test]
fn as_shot_neutral() {
    let mut s = DngSpec::new(32, 32, DngCodec::U16);
    s.color_matrix = XYZ_TO_SRGB;
    s.neutral = [0.5, 1.0, 0.8];
    let mut data = vec![0u16; 32 * 32];
    for y in 0..32 {
        for x in 0..32 {
            let c = s.cfa[(y & 1) * 2 + (x & 1)];
            data[y * 32 + x] = (20000.0 * [0.5, 1.0, 0.8][c as usize]) as u16;
        }
    }
    let file = write_dng(&s, &data);
    let img = specular_raw::parse(&file)
        .unwrap()
        .decode(&file, &DecodeOptions::default())
        .unwrap();
    for px in img.data.chunks_exact(3) {
        let (r, g, b) = (px[0] as i32, px[1] as i32, px[2] as i32);
        assert!((r - g).abs() < 300 && (b - g).abs() < 300, "{px:?}");
    }
}

/// LinearRaw with an sRGB "camera" is (nearly) the identity.
#[test]
fn linear_raw_identity_colour() {
    let mut s = DngSpec::new(16, 8, DngCodec::U16);
    s.linear_raw = true;
    s.color_matrix = XYZ_TO_SRGB;
    let data = test_pattern(16, 8, 3, 16, 9);
    let file = write_dng(&s, &data);
    let img = specular_raw::parse(&file)
        .unwrap()
        .decode(&file, &DecodeOptions::default())
        .unwrap();
    for (a, b) in img.data.iter().zip(&data) {
        assert!((*a as i32 - *b as i32).abs() < 200, "{a} vs {b}");
    }
}

#[test]
fn linearization_table() {
    let mut s = DngSpec::new(16, 16, DngCodec::Packed(8));
    s.bits = 8;
    s.white = 1020;
    s.color_matrix = XYZ_TO_SRGB;
    s.linearization = Some((0..256).map(|v| v * 4).collect());
    let img = uniform(&s, 128); // 512 / 1020 ≈ 0.502
    for &c in &img.data {
        assert!((c as i32 - 32896).abs() < 400, "{c}");
    }
}

#[test]
fn active_area_crop() {
    let mut s = DngSpec::new(40, 30, DngCodec::U16);
    s.active_area = Some([2, 4, 28, 36]); // top, left, bottom, right
    let data = test_pattern(40, 30, 1, 16, 1);
    let file = write_dng(&s, &data);
    let raw = specular_raw::parse(&file).unwrap();
    assert_eq!(
        raw.info().sensor.as_ref().unwrap().crop,
        Rect {
            x: 4,
            y: 2,
            w: 32,
            h: 26
        }
    );
    let img = raw.decode(&file, &DecodeOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (32, 26));
}

#[test]
fn huge_dimensions_are_refused_without_allocating() {
    let s = DngSpec::new(16, 16, DngCodec::U16);
    let mut file = write_dng(&s, &vec![100; 256]);
    assert!(patch_tag(&mut file, 0x0100, 60000));
    assert!(patch_tag(&mut file, 0x0101, 60000));
    if let Ok(raw) = specular_raw::parse(&file) {
        assert!(raw.decode_raw(&file, &DecodeOptions::default()).is_err());
    }
    // An explicit limit is honoured too.
    let file = write_dng(&s, &vec![100; 256]);
    let raw = specular_raw::parse(&file).unwrap();
    assert!(
        raw.decode_raw(
            &file,
            &DecodeOptions {
                max_pixels: 100,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn cancel_flag() {
    let s = DngSpec::new(64, 64, DngCodec::Lj92Tiles(32));
    let file = write_dng(&s, &test_pattern(64, 64, 1, 16, 2));
    let stop = std::sync::atomic::AtomicBool::new(true);
    let raw = specular_raw::parse(&file).unwrap();
    let r = raw.decode_raw(
        &file,
        &DecodeOptions {
            cancel: Some(&stop),
            ..Default::default()
        },
    );
    assert!(matches!(r, Err(specular_raw::RawError::Cancelled)));
}
