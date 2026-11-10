//! Float JPEG XL and signed JPEG 2000 units, written by tifffile 2026.11.8 + imagecodecs
//! 2026.10.10 (libjxl, OpenJPEG; lossless, 16×16 tiles; `tools/corpus/tiff-codec-float.py`):
//! every band's values as stored (`decode_values`) equal the source samples (`NAME.src`,
//! native order, interleaved).

use std::path::Path;

use specular_tiff::{DecodeOptions, PixelFormat, Tiff};

fn fixture(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codec_float")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn half_to_f32(h: u16) -> f32 {
    let (s, e, m) = ((h >> 15) as u32, ((h >> 10) & 31) as i32, (h & 1023) as f32);
    let v = match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    };
    if s == 1 { -v } else { v }
}

#[test]
fn float_jxl_and_signed_j2k_values_equal_the_source() {
    type Reader = fn(&[u8]) -> Vec<f64>;
    let cases: [(&str, usize, Reader); 5] = [
        ("jxl_f32_gray", 1, |b| {
            b.chunks_exact(4)
                .map(|c| f64::from(f32::from_ne_bytes(c.try_into().unwrap())))
                .collect()
        }),
        ("jxl_f32_rgb", 3, |b| {
            b.chunks_exact(4)
                .map(|c| f64::from(f32::from_ne_bytes(c.try_into().unwrap())))
                .collect()
        }),
        ("jxl_f16_gray", 1, |b| {
            b.chunks_exact(2)
                .map(|c| f64::from(half_to_f32(u16::from_ne_bytes([c[0], c[1]]))))
                .collect()
        }),
        ("j2k_i16_gray", 1, |b| {
            b.chunks_exact(2)
                .map(|c| f64::from(i16::from_ne_bytes([c[0], c[1]])))
                .collect()
        }),
        ("j2k_i8_rgb", 3, |b| {
            b.iter().map(|&v| f64::from(v as i8)).collect()
        }),
    ];
    for (name, spp, read) in cases {
        let data = fixture(&format!("{name}.tif"));
        let src = read(&fixture(&format!("{name}.src")));
        let t = Tiff::open(&data).unwrap();
        t.pages()[0]
            .check_supported()
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut differ = 0;
        for band in 0..spp {
            let d = t
                .decode_values(&data, 0, band, None, &DecodeOptions::default())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(d.format, PixelFormat::GrayF32);
            assert_eq!((d.width, d.height), (45, 37), "{name}");
            for (i, c) in d.data.chunks_exact(4).enumerate() {
                let v = f64::from(f32::from_ne_bytes(c.try_into().unwrap()));
                differ += usize::from(v != src[i * spp + band]);
            }
        }
        eprintln!("{name}: {differ} of {} values differ", src.len());
        assert_eq!(differ, 0, "{name}");
    }
}

/// The display decode of a float JPEG XL tile works too.
#[test]
fn float_jxl_displays() {
    let data = fixture("jxl_f32_rgb.tif");
    let t = Tiff::open(&data).unwrap();
    let d = t
        .decode(
            &data,
            0,
            &DecodeOptions {
                wide: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!d.truncated);
    assert_eq!((d.width, d.height), (45, 37));
}
