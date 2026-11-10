//! The colour model, chroma subsampling and stored bit depth each decoder reports for the
//! info panel, from the fixtures of the format crates.

#[path = "../../formats/specular-legacy/tests/common/mod.rs"]
mod lg;
#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod rw;

use specular_core::Source;
use specular_decode::{Registry, Session};
use std::path::Path;
use std::sync::Arc;

fn open(rel: &str) -> Arc<dyn Session> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel);
    Registry::default()
        .open_path(&path)
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// `(file under crates/, colour model, subsampling, bit depth)`.
type Case = (&'static str, Option<&'static str>, Option<&'static str>, u8);

fn check(cases: &[Case]) {
    let mut wrong = Vec::new();
    for &(rel, model, sub, depth) in cases {
        let s = open(rel);
        let got = (s.colour_model(), s.chroma_subsampling(), s.info().bit_depth);
        if got != (model, sub, depth) {
            wrong.push(format!(
                "{rel}: got {got:?}, want {:?}",
                (model, sub, depth)
            ));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

#[test]
fn psd_modes() {
    check(&[
        (
            "formats/specular-psd/fuzz/seeds/bitmap1.psd",
            Some("Greyscale"),
            None,
            1,
        ),
        (
            "formats/specular-psd/fuzz/seeds/gray8.psd",
            Some("Greyscale"),
            None,
            8,
        ),
        (
            "formats/specular-psd/fuzz/seeds/indexed.psd",
            Some("Palette"),
            None,
            8,
        ),
        (
            "formats/specular-psd/fuzz/seeds/rgb8_raw.psd",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-psd/fuzz/seeds/rgb16.psd",
            Some("RGB"),
            None,
            16,
        ),
        (
            "formats/specular-psd/fuzz/seeds/cmyk.psd",
            Some("CMYK"),
            None,
            8,
        ),
        (
            "formats/specular-psd/fuzz/seeds/multichannel.psd",
            Some("Multichannel"),
            None,
            8,
        ),
        (
            "formats/specular-psd/fuzz/seeds/duotone.psd",
            Some("Duotone"),
            None,
            8,
        ),
        (
            "formats/specular-psd/fuzz/seeds/lab8.psd",
            Some("Lab"),
            None,
            8,
        ),
    ]);
}

#[test]
fn png_and_webp() {
    check(&[
        (
            "specular-decode/tests/alpha/rgba8.png",
            Some("RGB"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/rgba16.png",
            Some("RGB"),
            None,
            16,
        ),
        (
            "specular-decode/tests/alpha/la8.png",
            Some("Greyscale"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/palette-trns.png",
            Some("Palette"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/lossy.webp",
            Some("YCbCr"),
            Some("4:2:0"),
            8,
        ),
        (
            "specular-decode/tests/alpha/lossless.webp",
            Some("RGB"),
            None,
            8,
        ),
    ]);
}

#[test]
fn bitmap_formats() {
    check(&[
        (
            "formats/specular-bitmap/fuzz/seeds/pal4.bmp",
            Some("Palette"),
            None,
            4,
        ),
        (
            "formats/specular-bitmap/fuzz/seeds/rle8.bmp",
            Some("Palette"),
            None,
            8,
        ),
        // Bits per sample, not per pixel.
        (
            "specular-decode/tests/alpha/rgba32-bitfields.bmp",
            Some("RGB"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/argb1555-bitfields.bmp",
            Some("RGB"),
            None,
            5,
        ),
        // GIF: the colour table's index bits.
        (
            "formats/specular-bitmap/fuzz/seeds/a.gif",
            Some("Palette"),
            None,
            6,
        ),
        (
            "formats/specular-bitmap/fuzz/seeds/a.pbm",
            Some("Greyscale"),
            None,
            1,
        ),
        (
            "formats/specular-bitmap/fuzz/seeds/a.pgm",
            Some("Greyscale"),
            None,
            8,
        ),
        (
            "formats/specular-bitmap/fuzz/seeds/a.ppm",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-bitmap/fuzz/seeds/d16.ppm",
            Some("RGB"),
            None,
            16,
        ),
        (
            "formats/specular-bitmap/fuzz/seeds/a.qoi",
            Some("RGB"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/bgra32.tga",
            Some("RGB"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/argb1555.tga",
            Some("RGB"),
            None,
            5,
        ),
        (
            "specular-decode/tests/alpha/bgra32.ico",
            Some("RGB"),
            None,
            8,
        ),
        // A PNG inside an ICO reports the PNG's.
        ("specular-decode/tests/alpha/png.ico", Some("RGB"), None, 8),
    ]);
}

#[test]
fn legacy_formats() {
    check(&[
        (
            "formats/specular-legacy/fuzz/seeds/a.pcx",
            Some("Palette"),
            None,
            8,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/pal.pcx",
            Some("Palette"),
            None,
            8,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.sgi",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.ras",
            Some("Palette"),
            None,
            8,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/pal.ras",
            Some("Palette"),
            None,
            8,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.ff",
            Some("RGB"),
            None,
            16,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.xbm",
            Some("Greyscale"),
            None,
            1,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.wbmp",
            Some("Greyscale"),
            None,
            1,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.xpm",
            Some("Palette"),
            None,
            8,
        ),
        (
            "formats/specular-legacy/fuzz/seeds/a.fli",
            Some("Palette"),
            None,
            8,
        ),
    ]);
}

/// True-colour PCX and Sun raster (no fixture): RGB, 8 bits per sample, not 24 per pixel.
#[test]
fn legacy_true_colour() {
    let open = |name: &str, bytes| {
        Registry::default()
            .open(Arc::new(Source::from_vec(name, bytes)))
            .unwrap()
    };
    let (w, h) = (4u16, 2u16);
    let lines: Vec<Vec<u8>> = (0..h * 3).map(|i| vec![i as u8 * 30; w as usize]).collect();
    let pcx = open("t.pcx", lg::pcx(w, h, 8, 3, w, false, &[], &lines, None));
    assert_eq!((pcx.colour_model(), pcx.info().bit_depth), (Some("RGB"), 8));
    let raw = lg::pattern(w as usize * h as usize * 3, 3, 255);
    let sun = open("t.ras", lg::sun(w.into(), h.into(), 24, 1, &[], &raw));
    assert_eq!((sun.colour_model(), sun.info().bit_depth), (Some("RGB"), 8));
    let grey = lg::pattern(w as usize * h as usize, 4, 255);
    let sun = open("g.ras", lg::sun(w.into(), h.into(), 8, 1, &[], &grey));
    assert_eq!(
        (sun.colour_model(), sun.info().bit_depth),
        (Some("Greyscale"), 8)
    );
}

#[test]
fn jpeg2000_and_jpeg_xl() {
    check(&[
        (
            "formats/specular-jp2/tests/data/palette.jp2",
            Some("Palette"),
            None,
            8,
        ),
        // Subsampled components, but tagged sRGB: subsampling is named for YCbCr only.
        (
            "formats/specular-jp2/tests/data/rgb_sub.jp2",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-jp2/tests/data/gray_rev.jp2",
            Some("Greyscale"),
            None,
            8,
        ),
        (
            "formats/specular-jp2/tests/data/gray16_rev.j2k",
            Some("Greyscale"),
            None,
            16,
        ),
        (
            "formats/specular-jp2/tests/data/rgb_rev.j2k",
            Some("RGB"),
            None,
            8,
        ),
        (
            "specular-decode/tests/alpha/rgba8.jp2",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-jxl/tests/data/gray.jxl",
            Some("Greyscale"),
            None,
            8,
        ),
        (
            "formats/specular-jxl/tests/data/vardct.jxl",
            Some("XYB"),
            None,
            8,
        ),
        (
            "formats/specular-jxl/tests/data/modular.jxl",
            Some("RGB"),
            None,
            8,
        ),
    ]);
}

#[test]
fn heif_formats() {
    check(&[
        (
            "formats/specular-avif/tests/data/gray.avif",
            Some("Greyscale"),
            None,
            8,
        ),
        // Lossless: RGB coded as is (matrix 0).
        (
            "specular-decode/tests/alpha/avif8-lossless.avif",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-avif/tests/data/rgb8_420.avif",
            Some("YCbCr"),
            Some("4:2:0"),
            8,
        ),
        (
            "formats/specular-avif/tests/data/rgb8_444.avif",
            Some("YCbCr"),
            Some("4:4:4"),
            8,
        ),
        (
            "formats/specular-avif/tests/data/rgb10_420.avif",
            Some("YCbCr"),
            Some("4:2:0"),
            10,
        ),
        (
            "formats/specular-heic/fuzz/seeds/c420.heic",
            Some("YCbCr"),
            Some("4:2:0"),
            8,
        ),
        (
            "formats/specular-heic/fuzz/seeds/c422_10.heic",
            Some("YCbCr"),
            Some("4:2:2"),
            10,
        ),
        (
            "formats/specular-heic/fuzz/seeds/c444.heic",
            Some("YCbCr"),
            Some("4:4:4"),
            8,
        ),
        (
            "formats/specular-heic/fuzz/seeds/mono.heic",
            Some("Greyscale"),
            None,
            8,
        ),
    ]);
}

#[test]
fn float_and_texture_formats() {
    check(&[
        (
            "formats/specular-exr/fuzz/seeds/half_none.exr",
            Some("RGB"),
            None,
            16,
        ),
        (
            "formats/specular-exr/fuzz/seeds/luma.exr",
            Some("Greyscale"),
            None,
            16,
        ),
        (
            "formats/specular-exr/fuzz/seeds/ycc.exr",
            Some("YC"),
            None,
            16,
        ),
        (
            "formats/specular-radiance/fuzz/seeds/rle.hdr",
            Some("RGBE"),
            None,
            32,
        ),
        (
            "formats/specular-radiance/fuzz/seeds/xyze.hdr",
            Some("XYZE"),
            None,
            32,
        ),
        (
            "formats/specular-radiance/fuzz/seeds/rgb.pfm",
            Some("RGB"),
            None,
            32,
        ),
        (
            "formats/specular-radiance/fuzz/seeds/gray_be.pfm",
            Some("Greyscale"),
            None,
            32,
        ),
        (
            "formats/specular-texture/fuzz/seeds/bc4.dds",
            Some("R"),
            None,
            8,
        ),
        (
            "formats/specular-texture/fuzz/seeds/bc5.dds",
            Some("RG"),
            None,
            8,
        ),
        (
            "formats/specular-texture/fuzz/seeds/bc7.dds",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-texture/fuzz/seeds/bc7.ktx2",
            Some("RGB"),
            None,
            8,
        ),
        (
            "formats/specular-texture/fuzz/seeds/rgb9e5.dds",
            Some("RGBE"),
            None,
            9,
        ),
        (
            "formats/specular-texture/fuzz/seeds/r5g6b5.dds",
            Some("RGB"),
            None,
            6,
        ),
    ]);
}

#[test]
fn camera_raw_layout() {
    let (w, h) = (64, 48);
    let open = |spec: &rw::DngSpec, cpp| {
        let data = rw::test_pattern(w as usize, h as usize, cpp, 16, 7);
        Registry::default()
            .open(Arc::new(Source::from_vec(
                "x.dng",
                rw::write_dng(spec, &data),
            )))
            .unwrap()
    };
    let bayer = rw::DngSpec::new(w, h, rw::DngCodec::U16);
    assert_eq!(open(&bayer, 1).colour_model(), Some("Bayer"));
    let linear = rw::DngSpec {
        linear_raw: true,
        ..rw::DngSpec::new(w, h, rw::DngCodec::U16)
    };
    assert_eq!(open(&linear, 3).colour_model(), Some("LinearRaw"));
}

/// What the libheif fallback's session reports comes from the container alone.
#[test]
fn heif_header_coding_for_the_host_fallback() {
    let read = |n: &str| {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/alpha")
            .join(n);
        std::fs::read(p).unwrap()
    };
    let c = specular_heic::header_coding(&read("heic8.heic")).expect("parses");
    assert!(c.chroma.is_some(), "{c:?}");
    assert!(c.has_alpha && !c.premultiplied_alpha, "{c:?}");
    assert!(specular_heic::header_coding(b"not a heif file").is_none());
}

/// A raw whose sensor data cannot be decoded shows its embedded preview, and still reports
/// the sensor's layout.
#[test]
fn camera_raw_preview_reports_the_sensor_layout() {
    let (w, h) = (64, 48);
    let jpeg = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../specular-metadata/tests/fixtures/ocean.jpg"),
    )
    .unwrap();
    let data = rw::test_pattern(w as usize, h as usize, 1, 16, 7);
    let write = |at: u32| {
        let spec = rw::DngSpec {
            extra: vec![
                // A compression no decoder knows.
                (259, 3, 1, 65000u16.to_le_bytes().to_vec()),
                rw::tag_longs(0x0201, &[at]),
                rw::tag_longs(0x0202, &[jpeg.len() as u32]),
            ],
            ..rw::DngSpec::new(w, h, rw::DngCodec::U16)
        };
        rw::write_dng(&spec, &data)
    };
    // The preview goes at the end; the offset does not change the file's size.
    let mut file = write(0);
    file = write(file.len() as u32);
    file.extend_from_slice(&jpeg);
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("x.dng", file)))
        .unwrap();
    let coding = &s.info().coding;
    assert!(coding.contains("embedded preview only"), "{coding}");
    assert_eq!(s.colour_model(), Some("Bayer"));
}

/// How each family stores transparency, from its header: an alpha channel (straight or
/// premultiplied), or a transparent colour, index or mask; nothing for opaque images.
#[test]
fn alpha_per_family() {
    use specular_decode::Alpha::{Premultiplied as P, Straight as S, Transparency as T};
    let cases = [
        // PNG: colour type, tRNS; APNG; WebP lossless and lossy (ALPH).
        ("rgba8.png", Some(S)),
        ("rgba16.png", Some(S)),
        ("la8.png", Some(S)),
        ("palette-trns.png", Some(T)),
        ("rgb-trns.png", Some(T)),
        ("apng.png", Some(S)),
        ("lossless.webp", Some(S)),
        ("lossy.webp", Some(S)),
        // GIF and XPM: a transparent index.
        ("index.gif", Some(T)),
        ("index.xpm", Some(T)),
        ("opaque-unused-none.xpm", None),
        // TIFF ExtraSamples: associated (premultiplied) or unassociated.
        ("rgba8-assoc.tif", Some(P)),
        ("rgba16-assoc.tif", Some(P)),
        ("rgba32f-assoc.tif", Some(P)),
        ("rgba8-unassoc.tif", Some(S)),
        ("la8-unassoc.tif", Some(S)),
        ("opaque-palette.tif", None),
        // PSD: an alpha channel, or an indexed image's transparent index.
        ("rgba8.psd", Some(S)),
        ("rgba16.psd", Some(S)),
        ("index-alpha.psd", Some(S)),
        ("index-trns.psd", Some(T)),
        // HEIF: an alpha auxiliary image (premultiplied when `prem` says so).
        ("avif8.avif", Some(S)),
        ("avif8-lossless.avif", Some(S)),
        ("heic8.heic", Some(S)),
        // JPEG XL extra channels; JPEG 2000 cdef.
        ("jxl8.jxl", Some(S)),
        ("jxl8-lossy.jxl", Some(S)),
        ("rgba8.jp2", Some(S)),
        ("rgba16-r1.jp2", Some(S)),
        // EXR: premultiplied by definition.
        ("half.exr", Some(P)),
        ("float.exr", Some(P)),
        // BMP, ICO/CUR (an AND mask is transparency), TGA (attribute type 4), QOI and the rest.
        ("argb32-bitfields.bmp", Some(S)),
        ("opaque-argb32-bitfields.bmp", None),
        ("bgr24-mask.ico", Some(T)),
        ("opaque-bgr24-mask.ico", None),
        ("bgra32.ico", Some(S)),
        ("bgra32.cur", Some(S)),
        ("png.ico", Some(S)),
        ("bgra32-premul.tga", Some(P)),
        ("bgra32-straight.tga", Some(S)),
        ("opaque-bgra32.tga", None),
        ("rgba8.qoi", Some(S)),
        ("rgba8.pam", Some(S)),
        ("rgba8.sgi", Some(S)),
        ("rgba16.ff", Some(S)),
        ("opaque.ff", None),
        // DDS (DXT2/DXT4 are premultiplied) and KTX2.
        ("dxt2.dds", Some(P)),
        ("dxt4.dds", Some(P)),
        ("bc3.dds", Some(S)),
        ("rgba8-dx10.dds", Some(S)),
        ("opaque-bc1.dds", None),
        ("rgba8.ktx2", Some(S)),
        ("bc7.ktx2", Some(S)),
    ];
    let mut wrong = Vec::new();
    for (name, want) in cases {
        let got = open(&format!("specular-decode/tests/alpha/{name}")).alpha();
        if got != want {
            wrong.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    // Opaque fixtures of the format crates.
    for rel in [
        "formats/specular-psd/fuzz/seeds/gray8.psd",
        "formats/specular-texture/fuzz/seeds/bc4.dds",
    ] {
        assert_eq!(open(rel).alpha(), None, "{rel}");
    }
}
