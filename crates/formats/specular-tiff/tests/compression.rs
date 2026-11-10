//! The wider compressions, one small file each: Zstandard, LZMA (with libtiff's Delta filter),
//! Deflate with the floating-point predictor, LERC (v2.4, v5, Deflate- and Zstandard-wrapped,
//! NaN masks), WebP, JPEG XL, JPEG 2000 (Aperio YCbCr too), CCITT and old-style JPEG.
//!
//! The fixtures in `tests/fixtures/compression` are 37×23 images written by tifffile /
//! imagecodecs / Pillow (`tools/corpus/tiff-test-fixtures.py`) from formulas recomputed here,
//! so each lossless decode is checked exactly. The old-style JPEG files are built here.

mod common;

use common::{Comp, Spec, write};
use specular_tiff::{DecodeOptions, Decoded, PixelFormat, Tiff};

const W: u32 = 37;
const H: u32 = 23;

fn fixture(name: &str) -> Vec<u8> {
    let p = format!(
        "{}/tests/fixtures/compression/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn decode(data: &[u8], wide: bool) -> Decoded {
    let t = Tiff::open(data).expect("open");
    t.decode(
        data,
        0,
        &DecodeOptions {
            wide,
            ..Default::default()
        },
    )
    .expect("decode")
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

/// `f(x, y)` for every pixel in raster order.
fn image<T>(f: impl Fn(u32, u32) -> Vec<T>) -> Vec<T> {
    let f = &f;
    (0..H)
        .flat_map(|y| (0..W).flat_map(move |x| f(x, y)))
        .collect()
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
    match e {
        0 => s * m * 2f32.powi(-24),
        31 => s * f32::INFINITY,
        e => s * (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

fn halves(d: &Decoded) -> Vec<f32> {
    assert_eq!(d.format, PixelFormat::Rgba16F);
    ne16(d).into_iter().map(half_to_f32).collect()
}

fn check(name: &str, wide: bool, format: PixelFormat, want: &[u8]) {
    let d = decode(&fixture(name), wide);
    assert_eq!((d.width, d.height, d.format), (W, H, format), "{name}");
    assert!(d.data == want, "{name}: pixels differ");
}

fn check16(name: &str, format: PixelFormat, want: &[u16]) {
    let d = decode(&fixture(name), true);
    assert_eq!((d.width, d.height, d.format), (W, H, format), "{name}");
    assert!(ne16(&d) == want, "{name}: pixels differ");
}

fn rgb8(x: u32, y: u32) -> Vec<u8> {
    vec![u8v(x, y, 0), u8v(x, y, 1), u8v(x, y, 2), 255]
}

#[test]
fn zstd_and_lzma() {
    check16(
        "zstd-u16-pred2-tiled.tif",
        PixelFormat::Gray16,
        &image(|x, y| vec![u16v(x, y, 0)]),
    );
    check(
        "lzma-u8-gray.tif",
        false,
        PixelFormat::Gray8,
        &image(|x, y| vec![u8v(x, y, 0)]),
    );
    // xz blocks with a Delta filter (distance 3) before LZMA2, as libtiff writes them.
    check(
        "lzma-delta-u8-rgb.tif",
        false,
        PixelFormat::Rgba8,
        &image(rgb8),
    );
}

#[test]
fn floating_point_predictor_keeps_hdr() {
    // 32-bit big-endian RGB and 16-bit little-endian grey, predictor 3: linear, unclamped.
    for (name, c, tol) in [
        ("deflate-f4-rgb-pred3-be.tif", 3, 2e-3),
        ("deflate-f2-gray-pred3-le.tif", 1, 4e-3),
    ] {
        let d = decode(&fixture(name), true);
        let got = halves(&d);
        let want = image(|x, y| {
            let v: Vec<f32> = (0..3)
                .map(|k| f32v(x, y, if c == 1 { 0 } else { k }))
                .collect();
            vec![v[0], v[1], v[2], 1.0]
        });
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            assert!(
                (g - w).abs() <= tol * w.abs().max(1.0),
                "{name} [{i}]: {g} vs {w}"
            );
        }
        assert!(got.iter().any(|&v| v > 2.0) && got.iter().any(|&v| v < 0.0));
        // 8-bit output clamps and sRGB-encodes.
        let n = decode(&fixture(name), false);
        let px = n.format.bytes_per_pixel();
        assert_eq!(n.data[0], 0, "{name}: negative is black");
        let last = n.data.len() - px;
        assert_eq!(n.data[last], 255, "{name}: above 1.0 is white");
    }
}

#[test]
fn lerc() {
    check16(
        "lerc-u2-tiled.tif",
        PixelFormat::Gray16,
        &image(|x, y| vec![u16v(x, y, 0)]),
    );
    // v5 blobs: the block header's column check moves up a bit.
    check16(
        "lerc-v5-u2-rgb.tif",
        PixelFormat::Rgba16,
        &image(|x, y| vec![u16v(x, y, 0), u16v(x, y, 1), u16v(x, y, 2), 65535]),
    );
    check(
        "lerc-u1-rgb-deflate.tif",
        false,
        PixelFormat::Rgba8,
        &image(rgb8),
    );
    // Float with a NaN (invalid) block, Zstandard-wrapped.
    let d = decode(&fixture("lerc-f4-nan-zstd.tif"), true);
    let got = halves(&d);
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let nan = (3..9).contains(&y) && (4..20).contains(&x);
            let want = if nan { 0.0 } else { f32v(x, y, 0) };
            assert!(
                (got[i] - want).abs() <= 2e-3 * want.abs().max(1.0),
                "({x},{y}): {} vs {want}",
                got[i]
            );
        }
    }
}

fn mean_max(a: &[u8], b: &[u8]) -> (f64, u8) {
    assert_eq!(a.len(), b.len());
    let sum: u64 = a.iter().zip(b).map(|(&p, &q)| p.abs_diff(q) as u64).sum();
    let max = a
        .iter()
        .zip(b)
        .map(|(&p, &q)| p.abs_diff(q))
        .max()
        .unwrap_or(0);
    (sum as f64 / a.len() as f64, max)
}

#[test]
fn image_codecs() {
    check(
        "webp-lossless-rgba.tif",
        false,
        PixelFormat::Rgba8,
        &image(|x, y| {
            vec![
                u8v(x, y, 0),
                u8v(x, y, 1),
                u8v(x, y, 2),
                u8v(x, y, 0) | 0x80,
            ]
        }),
    );
    check(
        "jxl-lossless-rgb.tif",
        false,
        PixelFormat::Rgba8,
        &image(rgb8),
    );
    check16(
        "j2k-lossless-u16-gray.tif",
        PixelFormat::Gray16,
        &image(|x, y| vec![u16v(x, y, 0)]),
    );
    // Lossy WebP (4:2:0) and Aperio 33003 (YCbCr rounded to 8 bits before coding).
    for (name, mean, max) in [
        ("webp-lossy-rgb.tif", 4.0, 40),
        ("aperio-33003.tif", 1.0, 3),
    ] {
        let d = decode(&fixture(name), false);
        assert_eq!((d.width, d.height, d.format), (W, H, PixelFormat::Rgba8));
        let (m, x) = mean_max(&d.data, &image(rgb8));
        assert!(m <= mean && x <= max, "{name}: mean {m:.2} max {x}");
    }
}

#[test]
fn ccitt() {
    let want = image(|x, y| {
        let white = (x / 3 + y / 2) % 5 == 0 || (x * y) % 11 == 0;
        vec![if white { 255 } else { 0 }]
    });
    for name in [
        "ccitt-group4.tif",
        "ccitt-group3.tif",
        "ccitt-tiff_ccitt.tif",
    ] {
        check(name, false, PixelFormat::Gray8, &want);
    }
}

// ---------------------------------------------------------------------------- old-style JPEG

/// Value of a classic little-endian file's first-IFD entry `tag` (one inline LONG / SHORT).
fn set_tag(file: &mut [u8], tag: u16, v: u32) {
    let ifd = u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize;
    let n = u16::from_le_bytes([file[ifd], file[ifd + 1]]) as usize;
    for i in 0..n {
        let e = ifd + 2 + 12 * i;
        if u16::from_le_bytes([file[e], file[e + 1]]) == tag {
            file[e + 8..e + 12].copy_from_slice(&v.to_le_bytes());
            return;
        }
    }
    panic!("no tag {tag}");
}

/// Start of the entropy-coded data (after the SOS segment) and the offset of SOS's Se byte.
fn scan_start(j: &[u8]) -> (usize, usize) {
    let mut p = 2;
    loop {
        let len = u16::from_be_bytes([j[p + 2], j[p + 3]]) as usize;
        if j[p + 1] == 0xDA {
            return (p + 2 + len, p + 2 + len - 2);
        }
        p += 2 + len;
    }
}

/// A grey-ish ramp (R = G = B, so YCbCr chroma is neutral).
fn grey_ramp(w: u32, h: u32) -> Vec<u8> {
    (0..h)
        .flat_map(|y| (0..w).flat_map(move |x| [(20 + x * 3 + y * 2) as u8; 3]))
        .collect()
}

/// An old-style JPEG file: a single strip pointing a few bytes into the scan header of the
/// `JPEGInterchangeFormat` stream (whose SOS ends the spectral range at 0, as some writers
/// leave it), with an under-reported byte count, and optional ReferenceBlackWhite.
fn ojpeg_file(px: &[u8], w: u32, h: u32, rbw: Option<[u32; 6]>) -> Vec<u8> {
    let mut j = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut j, 95);
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    enc.encode(px, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    let (scan, se) = scan_start(&j);
    j[se] = 0;
    let mut s = Spec::new(w, h, 3, 8, 6, Vec::new());
    s.comp = Comp::OldJpeg;
    s.rows_per_strip = h;
    s.extra.push((512, 3, vec![1]));
    // The unit is written right after the 8-byte header.
    s.extra.push((513, 4, vec![8]));
    s.extra.push((514, 4, vec![j.len() as u64]));
    s.extra.push((530, 3, vec![1, 1]));
    if let Some(r) = rbw {
        s.extra
            .push((532, 5, r.iter().map(|&v| ((v as u64) << 32) | 1).collect()));
    }
    s.pre = Some(vec![j.clone()]);
    let mut file = write(&[s], false, true);
    set_tag(&mut file, 273, (8 + scan - 3) as u32);
    set_tag(&mut file, 279, ((j.len() - scan) / 2) as u32);
    file
}

#[test]
fn old_style_jpeg_interchange_stream() {
    let (w, h) = (48u32, 32u32);
    let src = grey_ramp(w, h);
    let rgba: Vec<u8> = src
        .chunks_exact(3)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    let d = decode(&ojpeg_file(&src, w, h, None), false);
    assert_eq!((d.width, d.height, d.format), (w, h, PixelFormat::Rgba8));
    let (m, x) = mean_max(&d.data, &rgba);
    assert!(m <= 1.0 && x <= 6, "mean {m:.2} max {x}");

    // Studio-range ReferenceBlackWhite: Y is stretched from 16..235 (chroma stays neutral).
    let d = decode(
        &ojpeg_file(&src, w, h, Some([16, 235, 128, 240, 128, 240])),
        false,
    );
    let want: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| {
            let v = ((p[0] as f64 - 16.0) * 255.0 / 219.0)
                .round()
                .clamp(0.0, 255.0) as u8;
            [v, v, v, 255]
        })
        .collect();
    let (m, x) = mean_max(&d.data, &want);
    assert!(
        m <= 1.5 && x <= 8,
        "ReferenceBlackWhite: mean {m:.2} max {x}"
    );
}
