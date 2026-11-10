//! Sony lossless compressed ARW (LJ92 tiles): synthetic round trips, damaged files, refused
//! layouts, and the 7R V sample from the large corpus when present.

mod common;

use common::{ArwSpec, test_pattern, write_arw};
use specular_raw::{DecodeOptions, OutputSize, RawFormat, SensorCompression};

fn pattern(s: &ArwSpec) -> Vec<u16> {
    test_pattern(
        s.width as usize,
        s.height as usize,
        1,
        s.precision as u32,
        s.width,
    )
}

fn roundtrip(s: &ArwSpec) {
    let data = pattern(s);
    let file = write_arw(s, &data);
    let raw = specular_raw::parse(&file).expect("parse");
    let info = raw.info();
    assert_eq!(info.format, Some(RawFormat::Arw));
    let sensor = info.sensor.as_ref().expect("sensor");
    assert!(sensor.decodable, "{:?}", sensor.unsupported_reason);
    assert_eq!(sensor.compression, SensorCompression::LosslessJpeg);
    assert_eq!((sensor.width, sensor.height), (s.width, s.height));
    for parallel in [false, true] {
        let opts = DecodeOptions {
            parallel,
            ..Default::default()
        };
        let img = raw.decode_raw(&file, &opts).expect("decode_raw");
        assert!(!img.truncated);
        assert!(
            img.data == data,
            "{}x{} tile {} quad {}: sample mismatch",
            s.width,
            s.height,
            s.tile,
            s.quad
        );
        assert_eq!(img.black, [s.black as f32; 4]);
        assert_eq!(img.white, s.white as f32);
        if let Some([x, y, w, h]) = s.crop {
            assert_eq!(
                (img.crop.x, img.crop.y, img.crop.w, img.crop.h),
                (x, y, w, h)
            );
        }
        for size in [OutputSize::Full, OutputSize::Half] {
            let rgb = specular_raw::develop(&img, &DecodeOptions { size, ..opts }).unwrap();
            let (ew, eh) = match size {
                OutputSize::Full => (img.crop.w, img.crop.h),
                OutputSize::Half => (img.crop.w / 2, img.crop.h / 2),
            };
            assert_eq!((rgb.width, rgb.height), (ew, eh));
        }
    }
}

#[test]
fn lossless_quad_tiles_with_partial_edges() {
    for (w, h, t) in [
        (64, 64, 32),
        (100, 70, 32),
        (33, 18, 16),
        (16, 16, 16),
        (40, 2, 16),
    ] {
        let mut s = ArwSpec::new(w, h, t);
        if w > 20 {
            s.crop = Some([2, 0, w - 4, h]);
        }
        roundtrip(&s);
    }
}

#[test]
fn lossless_other_frame_shapes_and_scans() {
    // Two-component frames, as DNG converters write them.
    let mut s = ArwSpec::new(70, 40, 32);
    s.quad = false;
    roundtrip(&s);
    // Restart intervals and other predictors take the general decoder.
    for (quad, predictor, restart) in [(true, 1, 5), (true, 6, 0), (false, 4, 7)] {
        let mut s = ArwSpec::new(50, 36, 16);
        s.quad = quad;
        s.predictor = predictor;
        s.restart = restart;
        roundtrip(&s);
    }
    // 12-bit data.
    let mut s = ArwSpec::new(48, 32, 16);
    s.precision = 12;
    s.white = 4095;
    s.black = 128;
    roundtrip(&s);
}

/// Offset of the raw SubIFD's entry for `tag`.
fn raw_entry(file: &[u8], tag: u16) -> Option<usize> {
    let rd16 = |p: usize| u16::from_le_bytes([file[p], file[p + 1]]);
    let rd32 = |p: usize| u32::from_le_bytes(file[p..p + 4].try_into().unwrap());
    let ifd0 = rd32(4) as usize;
    let sub = (0..rd16(ifd0) as usize)
        .map(|i| ifd0 + 2 + i * 12)
        .find(|&e| rd16(e) == 0x014a)?;
    let raw = rd32(sub + 8) as usize;
    (0..rd16(raw) as usize)
        .map(|i| raw + 2 + i * 12)
        .find(|&e| rd16(e) == tag)
}

#[track_caller]
fn reason(file: &[u8]) -> &'static str {
    let raw = specular_raw::parse(file).unwrap();
    let s = raw.info().sensor.as_ref().unwrap();
    assert!(!s.decodable);
    s.unsupported_reason.unwrap()
}

#[test]
fn refused_layouts_say_why() {
    let s = ArwSpec::new(64, 64, 32);
    let file = write_arw(&s, &pattern(&s));
    let set = |f: &mut Vec<u8>, tag: u16, v: u32| {
        let e = raw_entry(f, tag).unwrap();
        f[e + 8..e + 12].copy_from_slice(&v.to_le_bytes());
    };
    // Tiles that do not cover the image.
    let mut f = file.clone();
    set(&mut f, 0x0142, 16);
    assert!(reason(&f).contains("tile count"));
    // Same tile count, but the frames do not match the tile shape.
    let mut f = file.clone();
    set(&mut f, 0x0142, 64);
    set(&mut f, 0x0143, 20);
    assert!(reason(&f).contains("frame layout"));
    // Not tiled.
    let mut f = file.clone();
    for tag in [0x0142, 0x0143] {
        let e = raw_entry(&f, tag).unwrap();
        f[e..e + 2].copy_from_slice(&0x7fffu16.to_le_bytes());
    }
    assert!(reason(&f).contains("not tiled"));
    // Not a lossless JPEG frame.
    let mut f = file.clone();
    let off = raw_entry(&f, 0x0144).unwrap();
    let first = u32::from_le_bytes(f[off + 8..off + 12].try_into().unwrap()) as usize;
    let first = u32::from_le_bytes(f[first..first + 4].try_into().unwrap()) as usize;
    f[first + 1] = 0;
    assert!(reason(&f).contains("tile header"));
}

#[test]
fn damaged_files_never_panic() {
    let s = ArwSpec::new(48, 40, 16);
    let file = write_arw(&s, &pattern(&s));
    let opts = DecodeOptions::default();
    let try_decode = |f: &[u8]| -> Option<bool> {
        let raw = specular_raw::parse(f).ok()?;
        let img = raw.decode_raw(f, &opts).ok()?;
        let _ = specular_raw::develop(&img, &opts);
        Some(img.truncated)
    };
    assert_eq!(try_decode(&file), Some(false));
    // Short tiles (the byte counts cut) are decoded as far as they go and flagged.
    let counts = raw_entry(&file, 0x0145).unwrap();
    let n = u32::from_le_bytes(file[counts + 4..counts + 8].try_into().unwrap()) as usize;
    let at = u32::from_le_bytes(file[counts + 8..counts + 12].try_into().unwrap()) as usize;
    for keep in [3, 5, 9] {
        let mut f = file.clone();
        for i in 0..n {
            let p = at + 4 * i;
            let len = u32::from_le_bytes(f[p..p + 4].try_into().unwrap());
            f[p..p + 4].copy_from_slice(&(len * keep / 10).to_le_bytes());
        }
        assert_eq!(try_decode(&f), Some(true), "keep {keep}/10");
    }
    // The file cut anywhere.
    for cut in (8..file.len()).step_by(37) {
        let _ = try_decode(&file[..cut]);
    }
    // Corrupt bytes in the tile data (entropy-coded segments and headers).
    let data_end = at;
    let mut seed = 12345u32;
    for _ in 0..300 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let mut f = file.clone();
        let p = 8 + seed as usize % (data_end - 8);
        f[p] = (seed >> 8) as u8;
        f[(p * 7) % data_end] = 0xff;
        let _ = try_decode(&f);
    }
}

/// One tile, declared 60000 × 60000 for a 16 × 16 sensor: a tile is decoded at its declared
/// size, so this asked for gigabytes. Refused as a bad tile layout.
#[test]
fn tile_far_larger_than_the_sensor_is_refused() {
    let s = ArwSpec::new(16, 16, 16);
    let mut file = write_arw(&s, &pattern(&s));
    for tag in [0x42u8, 0x43] {
        let entry = [tag, 0x01, 4, 0, 1, 0, 0, 0];
        let at = file.windows(8).position(|w| w == entry).unwrap() + 8;
        file[at..at + 4].copy_from_slice(&60000u32.to_le_bytes());
    }
    let raw = specular_raw::parse(&file).unwrap();
    let sensor = raw.info().sensor.as_ref().unwrap();
    assert!(!sensor.decodable);
    assert!(raw.decode(&file, &DecodeOptions::default()).is_err());
}

/// A crop whose origin plus size overflows 32 bits is ignored (it used to overflow).
#[test]
fn overflowing_crop_is_ignored() {
    let mut s = ArwSpec::new(48, 40, 16);
    s.crop = Some([2, 1, 40, u32::MAX]);
    let file = write_arw(&s, &pattern(&s));
    let raw = specular_raw::parse(&file).unwrap();
    let crop = raw.info().sensor.as_ref().unwrap().crop;
    assert_eq!((crop.x, crop.y, crop.w, crop.h), (0, 0, 48, 40));
    raw.decode(&file, &DecodeOptions::default()).unwrap();
}

#[test]
fn cancel_flag() {
    let s = ArwSpec::new(64, 64, 32);
    let file = write_arw(&s, &pattern(&s));
    let stop = std::sync::atomic::AtomicBool::new(true);
    let raw = specular_raw::parse(&file).unwrap();
    for parallel in [false, true] {
        let r = raw.decode_raw(
            &file,
            &DecodeOptions {
                cancel: Some(&stop),
                parallel,
                ..Default::default()
            },
        );
        assert!(matches!(r, Err(specular_raw::RawError::Cancelled)));
    }
}

/// The ILCE-7RM5 lossless sample of the large corpus (`tools/corpus/large`, fetched, never
/// committed), checked against sums of LibRaw's unpacked mosaic. Skipped when absent.
#[test]
fn corpus_7rm5_matches_reference() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tools/corpus/large/raw/Sony/ILCE-7RM5/7RM5-LosslessCompressedLarge.ARW");
    let Ok(data) = std::fs::read(&p) else {
        eprintln!("{} not present; skipping", p.display());
        return;
    };
    let raw = specular_raw::parse(&data).unwrap();
    let img = raw.decode_raw(&data, &DecodeOptions::default()).unwrap();
    assert!(!img.truncated);
    assert_eq!((img.width, img.height), (9728, 6656));
    assert_eq!(img.black, [512.0; 4]);
    assert_eq!(img.white, 16383.0);
    let (mut sum, mut weighted) = (0u64, 0u64);
    for (i, &v) in img.data.iter().enumerate() {
        sum += v as u64;
        weighted += v as u64 * (i as u64 % 65521 + 1);
    }
    assert_eq!((sum, weighted), (118_231_925_753, 3_872_728_192_615_017));
    let rgb = specular_raw::develop(
        &img,
        &DecodeOptions {
            size: OutputSize::Half,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!((rgb.width, rgb.height), (img.crop.w / 2, img.crop.h / 2));
}

/// With `SPECULAR_RAW_FUZZ_SEEDS=<dir>`, write small synthetic lossless ARWs there as seeds for
/// `fuzz/fuzz_targets/decode.rs`.
#[test]
fn write_fuzz_seeds() {
    let Some(dir) = std::env::var_os("SPECULAR_RAW_FUZZ_SEEDS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, quad, restart) in [
        ("arw-lossless", true, 0),
        ("arw-lossless-rst", true, 4),
        ("arw-lossless-2c", false, 0),
    ] {
        let mut s = ArwSpec::new(40, 24, 16);
        s.quad = quad;
        s.restart = restart;
        std::fs::write(dir.join(format!("{name}.arw")), write_arw(&s, &pattern(&s))).unwrap();
    }
    // With a white balance (SR2), so region decodes read only the tiles behind the region.
    let mut s = ArwSpec::new(72, 40, 16);
    s.wb = Some([2400, 1024, 1024, 1700]);
    s.crop = Some([2, 2, 66, 36]);
    std::fs::write(dir.join("arw-lossless-wb.arw"), write_arw(&s, &pattern(&s))).unwrap();
}
