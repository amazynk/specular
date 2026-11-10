//! Codings outside the libjpeg-turbo ±1 comparison of `tests/djpeg.rs` (which covers 4:4:4,
//! 4:2:0, 4:2:2, 4:4:0, 4:1:1, greyscale, RGB, restart and progressive variants) and
//! `tests/cmyk.rs` (CMYK and YCCK):
//!
//! * 12-bit data (the installed djpeg is 8-bit only, so there is no libjpeg reference): an
//!   8-bit stream relabelled as 12-bit has the same coefficients, so its samples must be
//!   `128 + (s8 - 128) / 16` (±1) of the 8-bit decode `s8` wherever that is not clipped.
//!   Huffman and arithmetic coding (8-bit arithmetic coding is checked in `tests/arith.rs`).

mod common;

use common::*;
use specular_jpeg::*;

const SCALES: [Scale; 4] = [Scale::Full, Scale::Half, Scale::Quarter, Scale::Eighth];

fn region_opts(scale: Scale) -> DecodeOptions<'static> {
    DecodeOptions {
        scale,
        // Output coordinates: the same part of the image at every scale.
        region: Some(Region {
            x: 80 / scale.factor(),
            y: 64 / scale.factor(),
            w: 120 / scale.factor(),
            h: 80 / scale.factor(),
        }),
        ..Default::default()
    }
}

/// Relabels the frame of an 8-bit JPEG as 12-bit (SOF0 becomes SOF1; the progressive and
/// arithmetic frame types allow both precisions).
fn as_12bit(jpg: &[u8]) -> Vec<u8> {
    let mut v = jpg.to_vec();
    let sof = v
        .windows(2)
        .position(|w| w[0] == 0xFF && matches!(w[1], 0xC0 | 0xC2 | 0xC9 | 0xCA))
        .expect("SOF");
    if v[sof + 1] == 0xC0 {
        v[sof + 1] = 0xC1;
    }
    assert_eq!(v[sof + 4], 8);
    v[sof + 4] = 12;
    v
}

#[test]
fn twelve_bit_matches_scaled_8bit_decode() {
    if !tools_available() {
        eprintln!("skipping: cjpeg/djpeg/magick not installed");
        return;
    }
    let src = source_ppm(301, 203, 12);
    for (name, args) in [
        ("t12_420", &["-quality", "90", "-sample", "2x2"][..]),
        (
            "t12_422_rst",
            &["-quality", "90", "-sample", "2x1", "-restart", "2"],
        ),
        ("t12_gray", &["-quality", "90", "-grayscale"]),
        (
            "t12_444_prog",
            &["-quality", "90", "-sample", "1x1", "-progressive"],
        ),
        (
            "t12_420_arith_rst",
            &[
                "-quality",
                "90",
                "-sample",
                "2x2",
                "-arithmetic",
                "-restart",
                "3",
            ],
        ),
        (
            "t12_444_prog_arith",
            &[
                "-quality",
                "90",
                "-sample",
                "1x1",
                "-progressive",
                "-arithmetic",
            ],
        ),
    ] {
        let j8 = cjpeg(&src, name, args);
        let j12 = as_12bit(&j8);
        assert_eq!(read_info(&j12).unwrap().precision, 12, "{name}");
        let full = DecodeOptions {
            parallelism: Parallelism::Single,
            ..Default::default()
        };
        let a = decode(&j8, &full).unwrap();
        let b = decode(&j12, &full).unwrap();
        assert!(!b.truncated, "{name}");
        assert_eq!(a.planes.len(), b.planes.len(), "{name}");
        let mut compared = 0usize;
        for (pa, pb) in a.planes.iter().zip(&b.planes) {
            assert_eq!((pa.width, pa.height), (pb.width, pb.height), "{name}");
            for y in 0..pa.height as usize {
                for x in 0..pa.width as usize {
                    let s8 = pa.data[y * pa.stride + x] as i32;
                    if s8 == 0 || s8 == 255 {
                        continue;
                    }
                    let want = 128.0 + (s8 - 128) as f64 / 16.0;
                    let got = pb.data[y * pb.stride + x] as f64;
                    assert!(
                        (got - want).abs() <= 1.5,
                        "{name} ({x},{y}): 12-bit {got}, expected {want:.2}"
                    );
                    compared += 1;
                }
            }
        }
        assert!(compared > 50_000, "{name}: {compared}");
        // Every scale, regions, threads and an index decode: no panic, sizes as for 8 bits.
        for scale in SCALES {
            for o in [
                DecodeOptions {
                    scale,
                    parallelism: Parallelism::Threads(4),
                    ..Default::default()
                },
                region_opts(scale),
            ] {
                let a = decode(&j8, &o).unwrap();
                let b = decode(&j12, &o).unwrap();
                assert_eq!((a.width, a.height, a.origin), (b.width, b.height, b.origin));
            }
        }
        if let Ok(ix) = build_index(&j12) {
            let o = region_opts(Scale::Full);
            assert_eq!(
                decode_with_index(&j12, &ix, &o).unwrap(),
                decode(&j12, &o).unwrap(),
                "{name}"
            );
        }
    }
}
