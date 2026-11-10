//! The rarer stored layouts: integers of 17, 24 and 64 bits (signed too), subsampled YCbCr
//! outside JPEG, old-style LZW, SGI LogLuv, ThunderScan, LERC 6 lossless float and tiled
//! files that keep their offsets in the strip tags.
//!
//! The fixtures in `tests/fixtures/variants` are 37×23 images written by
//! `tools/corpus/tiff-test-fixtures.py --variants` (LogLuv through libtiff) from the formulas
//! of `tests/compression.rs`, recomputed here. ThunderScan and the tiled file are built here.

mod common;

use common::{Comp, Spec, write};
use specular_tiff::{DecodeOptions, Decoded, PixelFormat, Tiff};

const W: u32 = 37;
const H: u32 = 23;

fn fixture(name: &str) -> Vec<u8> {
    let p = format!(
        "{}/tests/fixtures/variants/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn decode(data: &[u8], wide: bool) -> Decoded {
    let t = Tiff::open(data).expect("open");
    t.pages()[0].check_supported().expect("supported");
    let d = t
        .decode(
            data,
            0,
            &DecodeOptions {
                wide,
                ..Default::default()
            },
        )
        .expect("decode");
    assert!(!d.truncated);
    d
}

fn values(data: &[u8], band: usize) -> Vec<f32> {
    let t = Tiff::open(data).expect("open");
    let d = t
        .decode_values(data, 0, band, None, &DecodeOptions::default())
        .expect("values");
    assert_eq!(d.format, PixelFormat::GrayF32);
    d.data
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn u8v(x: u32, y: u32, c: u32) -> u8 {
    ((x * 5 + y * 3 + c * 60 + (x * y) % 7) % 256) as u8
}

fn u16v(x: u32, y: u32, c: u32) -> u16 {
    ((u8v(x, y, c) as u32 * 251 + x * 17 + y) % 65536) as u16
}

fn f32v(x: u32, y: u32, c: u32) -> f32 {
    (x as f64 * 0.05 + y as f64 * 0.07 + c as f64 * 0.5 - 0.3) as f32
}

fn pixels() -> impl Iterator<Item = (u32, u32)> {
    (0..H).flat_map(|y| (0..W).map(move |x| (x, y)))
}

fn ne16(d: &Decoded) -> Vec<u16> {
    d.data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect()
}

fn half_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

#[test]
fn integers_of_24_17_and_64_bits() {
    // 24-bit little-endian: exact values, and 16-bit display = the top 16 bits.
    let data = fixture("int24-le-gray.tif");
    let v = values(&data, 0);
    let d = decode(&data, true);
    assert_eq!(d.format, PixelFormat::Gray16);
    let g = ne16(&d);
    for (i, (x, y)) in pixels().enumerate() {
        let want = u16v(x, y, 0) as u32 * 256 + u8v(x, y, 0) as u32;
        assert_eq!(v[i], want as f32, "({x},{y})");
        assert!((g[i] as i32 - (want >> 8) as i32).abs() <= 1, "({x},{y})");
    }
    assert!(
        Tiff::open(&data).unwrap().pages()[0]
            .ifd()
            .description()
            .contains("24-bit")
    );
    // 17-bit big-endian RGB, bit-packed.
    let data = fixture("int17-be-rgb.tif");
    let d = decode(&data, true);
    assert_eq!(d.format, PixelFormat::Rgba16);
    let px = ne16(&d);
    for (i, (x, y)) in pixels().enumerate() {
        for c in 0..3 {
            let v17 = u16v(x, y, c) as u32 * 2 + (x & 1);
            let want = (v17 as f64 * 65535.0 / 131071.0).round() as i32;
            assert!(
                (px[i * 4 + c as usize] as i32 - want).abs() <= 1,
                "({x},{y},{c})"
            );
        }
    }
    let v = values(&data, 2);
    assert_eq!(v[5], (u16v(5, 0, 2) as u32 * 2 + 1) as f32);
    // 64-bit signed: values kept (as f32); shown offset to unsigned over the whole 64-bit
    // range, so these (±2^55) stay within 1/256 of mid-grey.
    let data = fixture("int64-signed-gray.tif");
    let v = values(&data, 0);
    let d = decode(&data, true);
    let g = ne16(&d);
    for (i, (x, y)) in pixels().enumerate() {
        let s = u16v(x, y, 0) as i64 - 32768;
        assert_eq!(v[i], (s << 40) as f32, "({x},{y})");
        let want = 32768 + (s >> 8) as i32;
        assert!(
            (g[i] as i32 - want).abs() <= 1,
            "({x},{y}): {} vs {want}",
            g[i]
        );
    }
}

#[test]
fn subsampled_ycbcr() {
    for name in ["ycbcr-21.tif", "ycbcr-22.tif"] {
        let d = decode(&fixture(name), false);
        assert_eq!(d.format, PixelFormat::Rgba8);
        // Chroma is averaged over the block: compare with a tolerance, luma-weighted.
        let mut err = 0f64;
        for (i, (x, y)) in pixels().enumerate() {
            for c in 0..3 {
                err += (d.data[i * 4 + c] as f64 - u8v(x, y, c as u32) as f64).abs();
            }
        }
        let mean = err / (W * H * 3) as f64;
        assert!(mean < 40.0, "{name}: mean error {mean}");
        // Luma is kept per pixel: neighbours in one block differ like the source.
        let y = |i: usize| {
            0.299 * d.data[i * 4] as f64
                + 0.587 * d.data[i * 4 + 1] as f64
                + 0.114 * d.data[i * 4 + 2] as f64
        };
        let src_y = |x: u32, yy: u32| {
            0.299 * u8v(x, yy, 0) as f64
                + 0.587 * u8v(x, yy, 1) as f64
                + 0.114 * u8v(x, yy, 2) as f64
        };
        let mut lerr = 0f64;
        for (i, (x, yy)) in pixels().enumerate() {
            lerr += (y(i) - src_y(x, yy)).abs();
        }
        let lmean = lerr / (W * H) as f64;
        assert!(lmean < 3.0, "{name}: luma error {lmean}");
    }
}

#[test]
fn old_style_lzw() {
    let d = decode(&fixture("lzw-old-rgb.tif"), false);
    for (i, (x, y)) in pixels().enumerate() {
        for c in 0..3 {
            assert_eq!(d.data[i * 4 + c], u8v(x, y, c as u32), "({x},{y},{c})");
        }
    }
}

#[test]
fn logluv() {
    let data = fixture("logluv32.tif");
    let t = Tiff::open(&data).unwrap();
    assert!(t.pages()[0].ifd().description().contains("SGI LogLuv"));
    let d = decode(&data, true);
    assert_eq!(d.format, PixelFormat::Rgba16F);
    let h: Vec<f32> = ne16(&d).into_iter().map(half_to_f32).collect();
    // LogLuv keeps luminance to 0.3% and chromaticity to about 1/410 in u'v'.
    let mut worst = 0f32;
    for (i, (x, y)) in pixels().enumerate() {
        for c in 0..3 {
            let want = f32v(x, y, c) + 1.0;
            worst = worst.max((h[i * 4 + c as usize] - want).abs() / want);
        }
    }
    assert!(worst < 0.03, "worst relative error {worst}");
}

#[test]
fn logluv24() {
    let data = fixture("logluv24.tif");
    let t = Tiff::open(&data).unwrap();
    assert!(t.pages()[0].ifd().description().contains("SGI LogLuv24"));
    let d = decode(&data, true);
    assert_eq!(d.format, PixelFormat::Rgba16F);
    let h: Vec<f32> = ne16(&d).into_iter().map(half_to_f32).collect();
    // LogLuv24 keeps luminance to 0.6% (steps of 2^(1/64)) and chromaticity to a 0.0035
    // cell in u'v'.
    let (mut worst, mut sum) = (0f32, 0f32);
    for (i, (x, y)) in pixels().enumerate() {
        for c in 0..3 {
            let want = f32v(x, y, c) + 1.0;
            let e = (h[i * 4 + c as usize] - want).abs() / want;
            worst = worst.max(e);
            sum += e;
        }
    }
    let mean = sum / (3 * W * H) as f32;
    assert!(
        worst < 0.07 && mean < 0.015,
        "relative error: worst {worst}, mean {mean}"
    );
}

#[test]
fn lerc_lossless_float() {
    let v = values(&fixture("lerc-fpl-f4.tif"), 0);
    for (i, (x, y)) in pixels().enumerate() {
        assert_eq!(v[i].to_bits(), f32v(x, y, 0).to_bits(), "({x},{y})");
    }
    let data = fixture("lerc-fpl-f8-rgb.tif");
    for c in 0..3 {
        let v = values(&data, c);
        for (i, (x, y)) in pixels().enumerate() {
            assert_eq!(v[i], f32v(x, y, c as u32), "({x},{y},{c})");
        }
    }
}

/// ThunderScan coding of one row of 4-bit values: raw values and runs only.
fn thunder_row(row: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < row.len() {
        let v = row[i];
        out.push(0xC0 | v);
        let mut n = 0;
        while i + 1 + n < row.len() && row[i + 1 + n] == v && n < 63 {
            n += 1;
        }
        if n > 0 {
            out.push(n as u8);
        }
        // A 2-bit delta of +1 then a skip and -1 when the next values allow it.
        i += 1 + n;
        if i + 1 < row.len() && row[i] == (v + 1) & 0xf && row[i + 1] == v {
            out.push(0b01_01_10_11);
            i += 2;
        }
    }
    out
}

#[test]
fn thunderscan() {
    let (w, h) = (29u32, 7u32);
    let val = |x: u32, y: u32| ((x / 3 + y) % 16) as u8;
    let mut strip = Vec::new();
    for y in 0..h {
        let row: Vec<u8> = (0..w).map(|x| val(x, y)).collect();
        strip.extend(thunder_row(&row));
    }
    let mut s = Spec::new(w, h, 1, 4, 1, Vec::new());
    s.comp = Comp::Coded(32809);
    s.rows_per_strip = h;
    s.pre = Some(vec![strip]);
    let data = write(&[s], false, true);
    let d = decode(&data, false);
    assert_eq!(d.format, PixelFormat::Gray8);
    for y in 0..h {
        for x in 0..w {
            assert_eq!(d.data[(y * w + x) as usize], val(x, y) * 17, "({x},{y})");
        }
    }
}

#[test]
fn tiles_with_strip_offset_tags() {
    // Tiled, but the offsets and counts in StripOffsets / StripByteCounts.
    let (w, h) = (40u32, 24u32);
    let raw: Vec<u8> = (0..w * h).map(|i| (i * 7 % 251) as u8).collect();
    let mut s = Spec::new(w, h, 1, 8, 1, raw.clone());
    s.tile = Some((16, 16));
    let mut data = write(&[s], false, true);
    // Rename TileOffsets (324) and TileByteCounts (325) to 273 and 279 in the IFD.
    let ifd = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let n = u16::from_le_bytes([data[ifd], data[ifd + 1]]) as usize;
    for e in 0..n {
        let at = ifd + 2 + 12 * e;
        let tag = u16::from_le_bytes([data[at], data[at + 1]]);
        let new = match tag {
            324 => 273u16,
            325 => 279,
            _ => continue,
        };
        data[at..at + 2].copy_from_slice(&new.to_le_bytes());
    }
    // Keep the entries sorted: 273 and 279 now come before 322 and 323.
    let mut entries: Vec<[u8; 12]> = (0..n)
        .map(|e| {
            data[ifd + 2 + 12 * e..ifd + 14 + 12 * e]
                .try_into()
                .unwrap()
        })
        .collect();
    entries.sort_by_key(|e| u16::from_le_bytes([e[0], e[1]]));
    for (e, b) in entries.iter().enumerate() {
        data[ifd + 2 + 12 * e..ifd + 14 + 12 * e].copy_from_slice(b);
    }
    let d = decode(&data, false);
    assert_eq!(d.data, raw);
}
