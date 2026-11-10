//! Cross-format alpha grid: one synthetic RGBA reference (tests/alpha/grid.py) encoded into every
//! format with transparency, decoded through the registry and compared with the reference.
//!
//! Decoder output convention: **straight (unassociated) alpha** in every layout (`Rgba`,
//! `Rgba16`, `Rgba16F`; see `specular_core::PixelLayout`). Formats that store premultiplied
//! colour (OpenEXR, TIFF with associated alpha, TGA alpha type 4) are unpremultiplied by the
//! decoder; the GPU premultiplies on upload (`convert.wgsl`). So the reference here is straight,
//! and colour under fully transparent pixels must survive wherever the file stores it.
//!
//! What the comparison catches: lost alpha (opaque output), inverted alpha, premultiplied or
//! doubly unpremultiplied colour (dark or bright fringes), a transfer curve applied to alpha,
//! 16-bit alpha cut to 8 bits, 1-bit alpha that is not exactly 0 or 1, and colour under
//! transparent pixels thrown away. Fixtures are committed; regenerate them with
//! tests/alpha/make_fixtures.sh. Set `SPECULAR_ALPHA_SHEET=out.png` to also write a contact
//! sheet: per format, the image over a checkerboard and, below it, its colour with alpha
//! ignored.

use specular_core::{CancelToken, PixelLayout, Raster, Source, Transfer, hdr};
use specular_decode::{DecodeRequest, Registry};
use std::path::PathBuf;
use std::sync::Arc;

const W: u32 = 32;
const H: u32 = 32;
const COLS: [[u16; 3]; 8] = [
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [0, 255, 255],
    [255, 0, 255],
    [255, 255, 0],
    [255, 255, 255],
    [128, 128, 128],
];

/// The reference pixel, straight, 16 bits a sample (grid.py's `px16`).
fn px16(x: u32, y: u32) -> [u16; 4] {
    let (c, a): ([u16; 3], u32) = match y / 8 {
        0 => (
            if y < 4 { [255, 128, 0] } else { [0, 64, 255] },
            (x * 65535 + 15) / 31,
        ),
        1 => (COLS[(if y < 12 { x / 4 } else { 7 - x / 4 }) as usize], 0),
        2 => (
            COLS[(if y < 20 { x / 4 } else { 7 - x / 4 }) as usize],
            if y < 20 { 32768 } else { 16384 },
        ),
        _ => {
            let (cx, cy) = (x / 4, (y - 24) / 4);
            if (cx + cy) % 2 == 0 {
                (
                    if cx % 4 < 2 {
                        [255, 255, 255]
                    } else {
                        [0, 255, 0]
                    },
                    65535,
                )
            } else {
                ([255, 0, 255], 0)
            }
        }
    };
    [c[0] * 257, c[1] * 257, c[2] * 257, a as u16]
}

fn to8(v: u16) -> u16 {
    ((v as u32 * 255 + 32767) / 65535) as u16
}

/// Which variant of the reference a fixture was made from.
#[derive(Clone, Copy, PartialEq)]
enum Ref {
    /// 8-bit RGBA.
    Rgba8,
    /// 16-bit RGBA (alpha not a multiple of 257).
    Rgba16,
    /// 8-bit RGBA with alpha thresholded at 50%.
    OneBit,
    /// Grey + 8-bit alpha (grey = (2R + 5G + B) / 8).
    Grey8,
    /// Grey + 16-bit alpha.
    Grey16,
    /// The 16-bit values as linear light (float formats).
    Linear,
}

/// The expected straight pixel, 0..1, colour sRGB-encoded (linear references are encoded
/// here, so every comparison is in display steps).
fn reference(r: Ref, x: u32, y: u32) -> [f32; 4] {
    let p = px16(x, y);
    let n8 = |v: u16| to8(v) as f32 / 255.0;
    let n16 = |v: u16| v as f32 / 65535.0;
    let grey = || {
        let g = (2 * to8(p[0]) + 5 * to8(p[1]) + to8(p[2]) + 4) / 8;
        g as f32 / 255.0
    };
    match r {
        Ref::Rgba8 => [n8(p[0]), n8(p[1]), n8(p[2]), n8(p[3])],
        Ref::Rgba16 => [n16(p[0]), n16(p[1]), n16(p[2]), n16(p[3])],
        Ref::Linear => {
            let e = |v: u16| linear_to_srgb(n16(v));
            [e(p[0]), e(p[1]), e(p[2]), n16(p[3])]
        }
        Ref::OneBit => [
            n8(p[0]),
            n8(p[1]),
            n8(p[2]),
            if p[3] >= 32768 { 1.0 } else { 0.0 },
        ],
        Ref::Grey8 => [grey(), grey(), grey(), n8(p[3])],
        Ref::Grey16 => [grey(), grey(), grey(), n16(p[3])],
    }
}

struct Case {
    file: &'static str,
    reference: Ref,
    /// Largest alpha error, in 8-bit steps.
    tol_a: f32,
    /// Largest colour error (premultiplied, and straight where alpha >= 0.2), in 8-bit steps.
    tol_c: f32,
    /// The file stores colour under fully transparent pixels, so it must come back.
    hidden: bool,
    /// Colour is stored premultiplied, quantised to this many 8-bit steps either way (0.5 for
    /// 8-bit samples): unpremultiplying adds up to `premul / alpha` steps.
    premul: f32,
    /// Lossy colour (4:2:0 chroma, transform ringing): colour is checked only inside the 4×4
    /// cells, where it is flat; their edges blend with the neighbours by design. Alpha is
    /// still checked everywhere.
    lossy: bool,
    /// The file is opaque, or the decoder ignores its alpha on purpose (a documented policy):
    /// expect opaque pixels with the reference colour, and `ImageInfo::has_alpha` false (it
    /// must be true for every other case).
    opaque: bool,
}

const fn case(file: &'static str, reference: Ref, tol_a: f32, tol_c: f32, hidden: bool) -> Case {
    Case {
        file,
        reference,
        tol_a,
        tol_c,
        hidden,
        premul: 0.0,
        lossy: false,
        opaque: false,
    }
}

/// A lossy case: see [`Case::lossy`].
const fn lossy(file: &'static str, reference: Ref, tol_a: f32, tol_c: f32, hidden: bool) -> Case {
    Case {
        lossy: true,
        ..case(file, reference, tol_a, tol_c, hidden)
    }
}

/// An opaque file: see [`Case::opaque`].
const fn opaque(file: &'static str, reference: Ref, tol_c: f32) -> Case {
    Case {
        opaque: true,
        ..case(file, reference, 0.5, tol_c, false)
    }
}

/// Every fixture. Exact formats get half a step of slack for the float comparison.
const CASES: &[Case] = &[
    // PNG
    case("rgba8.png", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba16.png", Ref::Rgba16, 0.02, 0.02, true),
    case("palette-trns.png", Ref::Rgba8, 0.5, 0.5, true),
    case("rgb-trns.png", Ref::OneBit, 0.5, 0.5, false),
    case("la8.png", Ref::Grey8, 0.5, 0.5, true),
    case("la16.png", Ref::Grey16, 0.02, 0.02, true),
    case("apng.png", Ref::Rgba8, 0.5, 0.5, true),
    // GIF, XPM: one transparent index
    case("index.gif", Ref::OneBit, 0.5, 0.5, false),
    case("index.xpm", Ref::OneBit, 0.5, 0.5, false),
    // WebP
    case("lossless.webp", Ref::Rgba8, 0.5, 0.5, true),
    lossy("lossy.webp", Ref::Rgba8, 0.5, 12.0, false),
    // AVIF and HEIC: alpha as an auxiliary image
    lossy("avif8.avif", Ref::Rgba8, 2.0, 8.0, true),
    lossy("avif10.avif", Ref::Rgba16, 2.0, 8.0, true),
    lossy("avif12.avif", Ref::Rgba16, 2.0, 8.0, true),
    case("avif8-lossless.avif", Ref::Rgba8, 0.5, 0.5, true),
    lossy("heic8.heic", Ref::Rgba8, 2.0, 8.0, true),
    lossy("heic10.heic", Ref::Rgba16, 2.0, 8.0, true),
    // JPEG XL
    case("jxl8.jxl", Ref::Rgba8, 0.5, 0.5, true),
    case("jxl16.jxl", Ref::Rgba16, 0.02, 0.02, true),
    // Distance 1 on saturated 4-pixel cells rings by up to 74 steps; libjxl (through ffmpeg)
    // decodes the same file within 1 step of us, so this checks alpha and gross errors.
    lossy("jxl8-lossy.jxl", Ref::Rgba8, 2.0, 80.0, false),
    // TIFF: ExtraSamples 2 (unassociated) and 1 (associated), 8/16-bit and float
    case("rgba8-unassoc.tif", Ref::Rgba8, 0.5, 0.5, true),
    Case {
        premul: 0.5,
        ..case("rgba8-assoc.tif", Ref::Rgba8, 0.5, 0.75, false)
    },
    case("rgba16-unassoc.tif", Ref::Rgba16, 0.02, 0.02, true),
    case("rgba16-assoc.tif", Ref::Rgba16, 0.02, 0.05, false),
    case("rgba32f-unassoc.tif", Ref::Linear, 0.2, 0.3, true),
    case("rgba32f-assoc.tif", Ref::Linear, 0.2, 0.3, false),
    case("la8-unassoc.tif", Ref::Grey8, 0.5, 0.5, true),
    case("magick-unassoc.tif", Ref::Rgba8, 0.5, 0.5, true),
    opaque("opaque-palette.tif", Ref::Rgba8, 0.5),
    // Photoshop: the merged image
    case("rgba8.psd", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba16.psd", Ref::Rgba16, 0.02, 0.02, true),
    // Indexed: a transparent palette entry (resource 1047), and a merged alpha channel
    case("index-trns.psd", Ref::OneBit, 0.5, 0.5, false),
    case("index-alpha.psd", Ref::Rgba8, 0.5, 0.5, true),
    // OpenEXR: premultiplied by definition, so no colour under alpha 0
    case("half.exr", Ref::Linear, 0.2, 0.3, false),
    case("float.exr", Ref::Linear, 0.2, 0.3, false),
    // TGA
    case("bgra32.tga", Ref::Rgba8, 0.5, 0.5, true),
    case("bgra32-rle.tga", Ref::Rgba8, 0.5, 0.5, true),
    case("bgra32-straight.tga", Ref::Rgba8, 0.5, 0.5, true),
    Case {
        premul: 0.5,
        ..case("bgra32-premul.tga", Ref::Rgba8, 0.5, 0.75, false)
    },
    case("argb1555.tga", Ref::OneBit, 0.5, 4.5, true),
    case("magick.tga", Ref::Rgba8, 0.5, 0.5, true),
    opaque("opaque-bgra32.tga", Ref::Rgba8, 0.5),
    // BMP
    case("argb32-bitfields.bmp", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba32-bitfields.bmp", Ref::Rgba8, 0.5, 0.5, true),
    case("argb4444-bitfields.bmp", Ref::Rgba8, 8.5, 8.5, true),
    case("argb1555-bitfields.bmp", Ref::OneBit, 0.5, 4.5, true),
    // BITMAPINFOHEADER BI_RGB: the fourth byte is not alpha (bmp.rs policy)
    Case {
        opaque: true,
        ..case("argb32-birgb.bmp", Ref::Rgba8, 0.5, 0.5, false)
    },
    case("magick.bmp", Ref::Rgba8, 0.5, 0.5, true),
    opaque("opaque-argb32-bitfields.bmp", Ref::Rgba8, 0.5),
    // ICO / CUR: 32-bit, 24-bit with the AND mask, PNG inside
    case("bgra32.ico", Ref::Rgba8, 0.5, 0.5, true),
    case("bgra32.cur", Ref::Rgba8, 0.5, 0.5, true),
    case("bgr24-mask.ico", Ref::OneBit, 0.5, 0.5, false),
    case("png.ico", Ref::Rgba8, 0.5, 0.5, true),
    case("magick.ico", Ref::Rgba8, 0.5, 0.5, true),
    opaque("opaque-bgr24-mask.ico", Ref::Rgba8, 0.5),
    // DDS / KTX2: BC1 1-bit (transparent texels are black), BC2/BC3/BC7, uncompressed
    case("bc1.dds", Ref::OneBit, 0.5, 8.5, false),
    case("bc2.dds", Ref::Rgba8, 8.5, 8.5, true),
    case("bc3.dds", Ref::Rgba8, 2.5, 8.5, true),
    case("bc7.dds", Ref::Rgba8, 1.5, 1.5, true),
    case("magick-dxt5.dds", Ref::Rgba8, 2.5, 8.5, true),
    // DXT2 / DXT4: BC2 / BC3 with colour premultiplied (in 5:6:5, so up to ~4.5 steps each way)
    Case {
        premul: 4.5,
        ..case("dxt2.dds", Ref::Rgba8, 8.5, 8.5, false)
    },
    Case {
        premul: 4.5,
        ..case("dxt4.dds", Ref::Rgba8, 2.5, 8.5, false)
    },
    opaque("opaque-bc1.dds", Ref::Rgba8, 8.5),
    case("bgra8.dds", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba8-dx10.dds", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba16-dx10.dds", Ref::Rgba16, 0.02, 0.02, true),
    case("rgba16f-dx10.dds", Ref::Linear, 0.6, 0.6, true),
    case("rgba8.ktx2", Ref::Rgba8, 0.5, 0.5, true),
    case("bc7.ktx2", Ref::Rgba8, 1.5, 1.5, true),
    // Simple and legacy formats
    case("rgba8.qoi", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba8.pam", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba16.pam", Ref::Rgba16, 0.02, 0.02, true),
    case("rgba8.sgi", Ref::Rgba8, 0.5, 0.5, true),
    case("rgba16.sgi", Ref::Rgba16, 0.02, 0.02, true),
    case("rgba16.ff", Ref::Rgba16, 0.02, 0.02, true),
    opaque("opaque.ff", Ref::Rgba16, 0.02),
    opaque("opaque-unused-none.xpm", Ref::Rgba8, 0.5),
    case("rgba8.jp2", Ref::Rgba8, 0.5, 0.5, true),
    // 16-bit, no wavelet levels: reduced decodes are box-filtered (`downscale_wide`)
    case("rgba16-r1.jp2", Ref::Rgba16, 0.02, 0.02, true),
];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/alpha")
}

fn open(file: &str) -> Arc<dyn specular_decode::Session> {
    let data = std::fs::read(dir().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    Registry::default()
        .open(Arc::new(Source::from_vec(file, data)))
        .unwrap_or_else(|e| panic!("{file}: open: {e}"))
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Decoded pixels as 0..1 RGBA as stored (straight by convention), colour sRGB-encoded (linear
/// layouts are encoded here). Alpha is never converted. Layouts without alpha come back
/// opaque, which the comparison reports as lost alpha.
fn pixels(r: &Raster) -> Vec<[f32; 4]> {
    // YCbCr planes (HEIC) through the CPU twin of the viewer's conversion.
    if let PixelLayout::Yuv(_) = r.layout {
        return pixels(&specular_core::yuv::to_rgb(r).expect("planes"));
    }
    let n = (r.width * r.height) as usize;
    let p = &r.planes[0];
    let row = |i: usize| (i / r.width as usize) * p.stride + (i % r.width as usize);
    // Whether the samples are linear light.
    let (linear, px): (bool, Vec<[f32; 4]>) = match r.layout {
        PixelLayout::Rgba => (
            false,
            (0..n)
                .map(|i| {
                    let o = (i / r.width as usize) * p.stride + (i % r.width as usize) * 4;
                    std::array::from_fn(|c| p.data[o + c] as f32 / 255.0)
                })
                .collect(),
        ),
        PixelLayout::Rgba16(enc) => {
            assert!(
                matches!(enc.transfer, Transfer::Srgb | Transfer::Linear),
                "unexpected transfer {enc:?}"
            );
            (
                enc.transfer == Transfer::Linear,
                (0..n)
                    .map(|i| {
                        let o = (i / r.width as usize) * p.stride + (i % r.width as usize) * 8;
                        std::array::from_fn(|c| {
                            u16::from_ne_bytes([p.data[o + 2 * c], p.data[o + 2 * c + 1]]) as f32
                                / 65535.0
                        })
                    })
                    .collect(),
            )
        }
        PixelLayout::Rgba16F(_) => (
            true,
            (0..n)
                .map(|i| {
                    let o = (i / r.width as usize) * p.stride + (i % r.width as usize) * 8;
                    std::array::from_fn(|c| {
                        hdr::f16_to_f32(u16::from_ne_bytes([
                            p.data[o + 2 * c],
                            p.data[o + 2 * c + 1],
                        ]))
                    })
                })
                .collect(),
        ),
        PixelLayout::Gray16 | PixelLayout::GrayAlpha16 => (
            false,
            (0..n)
                .map(|i| {
                    let bpp = r.layout.pixel_bytes();
                    let o = (i / r.width as usize) * p.stride + (i % r.width as usize) * bpp;
                    let v = |c: usize| {
                        u16::from_ne_bytes([p.data[o + 2 * c], p.data[o + 2 * c + 1]]) as f32
                            / 65535.0
                    };
                    let a = if bpp == 4 { v(1) } else { 1.0 };
                    [v(0), v(0), v(0), a]
                })
                .collect(),
        ),
        PixelLayout::Gray => (
            false,
            (0..n)
                .map(|i| {
                    let g = p.data[row(i)] as f32 / 255.0;
                    [g, g, g, 1.0]
                })
                .collect(),
        ),
        PixelLayout::Rgb => (
            false,
            (0..n)
                .map(|i| {
                    let s = |k: usize| {
                        let q = &r.planes[k];
                        q.data[(i / r.width as usize) * q.stride + i % r.width as usize] as f32
                            / 255.0
                    };
                    [s(0), s(1), s(2), 1.0]
                })
                .collect(),
        ),
        PixelLayout::Yuv(_) => unreachable!("converted above"),
        PixelLayout::YCbCr => (
            false,
            (0..n)
                .map(|i| {
                    let y = p.data[row(i)] as f32 / 255.0;
                    [y, y, y, 1.0]
                })
                .collect(),
        ),
    };
    if !linear {
        return px;
    }
    let f = |v: f32| linear_to_srgb(v.max(0.0));
    px.into_iter()
        .map(|[r, g, b, a]| [f(r), f(g), f(b), a])
        .collect()
}

/// Names the likely bug when a comparison fails: tries each known mistake on the reference
/// and reports the one that explains the decoded pixels.
fn diagnose(c: &Case, got: &[[f32; 4]]) -> String {
    let mut out = Vec::new();
    let refs: Vec<[f32; 4]> = (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .map(|(x, y)| reference(c.reference, x, y))
        .collect();
    let explains = |f: &dyn Fn(&[f32; 4], &[f32; 4]) -> bool| {
        refs.iter()
            .zip(got)
            .filter(|(r, _)| r[3] > 0.1 && r[3] < 0.9)
            .all(|(r, g)| f(r, g))
    };
    let near = |a: f32, b: f32| (a - b).abs() < 3.0 / 255.0;
    if got.iter().all(|p| p[3] == 1.0) {
        out.push("alpha lost (opaque output)");
    } else if explains(&|r, g| near(g[3], 1.0 - r[3])) {
        out.push("alpha inverted");
    } else if explains(&|r, g| near(g[3], srgb_to_linear(r[3])) || near(g[3], linear_to_srgb(r[3])))
    {
        out.push("transfer curve applied to alpha");
    } else if c.reference == Ref::OneBit && got.iter().any(|p| p[3] != 0.0 && p[3] != 1.0) {
        out.push("1-bit alpha not 0/1");
    }
    // Premultiplication happens on the stored values: linear light for float formats.
    let lin = c.reference == Ref::Linear;
    let scaled = |v: f32, s: f32| {
        if lin {
            linear_to_srgb((srgb_to_linear(v) * s).min(1.0))
        } else {
            (v * s).min(1.0)
        }
    };
    if explains(&|r, g| (0..3).all(|k| near(g[k], scaled(r[k], r[3])))) {
        out.push("colour premultiplied (dark fringes)");
    } else if explains(&|r, g| (0..3).all(|k| near(g[k], scaled(r[k], 1.0 / r[3])))) {
        out.push("colour divided by alpha twice (bright fringes)");
    }
    if matches!(c.reference, Ref::Rgba16 | Ref::Grey16)
        && explains(&|r, g| {
            (g[3] * 65535.0 - ((r[3] * 65535.0) as u32 >> 8 << 8) as f32).abs() < 1.0
                || (g[3] * 255.0 - (r[3] * 255.0).round()).abs() < 0.01
        })
    {
        out.push("16-bit alpha cut to 8 bits");
    }
    out.join(", ")
}

/// Compares one fixture; `Err` describes every failed check.
fn check(c: &Case, r: &Raster) -> Result<(), String> {
    if (r.width, r.height) != (W, H) {
        return Err(format!("size {}x{}", r.width, r.height));
    }
    let got = pixels(r);
    let (mut ea, mut epm, mut est, mut ehid) = (0f32, 0f32, 0f32, 0f32);
    let mut worst = [(0, 0); 4];
    let mut non_binary = false;
    for y in 0..H {
        for x in 0..W {
            let mut want = reference(c.reference, x, y);
            if c.opaque {
                want[3] = 1.0;
            }
            let g = got[(y * W + x) as usize];
            let mut upd = |e: &mut f32, v: f32, k: usize| {
                if v > *e {
                    *e = v;
                    worst[k] = (x, y);
                }
            };
            upd(&mut ea, (g[3] - want[3]).abs(), 0);
            if c.lossy && (x % 4 == 0 || x % 4 == 3 || y % 4 == 0 || y % 4 == 3) {
                continue;
            }
            for k in 0..3 {
                // |g·ga − w·wa| <= |g − w|·ga + w·|ga − wa|: both tolerances add up.
                upd(
                    &mut epm,
                    (g[k] * g[3] - want[k] * want[3]).abs() - (c.tol_a / 255.0),
                    1,
                );
                if want[3] >= 0.2 {
                    let slack = c.premul / 255.0 / want[3];
                    upd(&mut est, ((g[k] - want[k]).abs() - slack).max(0.0), 2);
                }
                if want[3] == 0.0 && c.hidden {
                    upd(&mut ehid, (g[k] - want[k]).abs(), 3);
                }
            }
            non_binary |= c.reference == Ref::OneBit && g[3] != 0.0 && g[3] != 1.0;
        }
    }
    let steps = |e: f32| e * 255.0;
    // 8-bit output of a deeper source rounds: half a step. Truncation (`>> 8`) shows as a full
    // step at the ramp's x = 3, 7, 24 and 28.
    let floor = if r.layout == PixelLayout::Rgba && c.reference != Ref::Rgba8 {
        0.51
    } else {
        0.0
    };
    let mut errs = Vec::new();
    for (name, e, tol, k) in [
        ("alpha", ea, c.tol_a, 0),
        ("premultiplied colour", epm, c.tol_c, 1),
        ("straight colour", est, c.tol_c, 2),
        ("colour under alpha 0", ehid, c.tol_c, 3),
    ] {
        let tol = tol.max(floor);
        if steps(e) > tol {
            errs.push(format!(
                "{name} off by {:.2} steps (tolerance {tol}) at {:?}",
                steps(e),
                worst[k]
            ));
        }
    }
    if non_binary {
        errs.push("1-bit alpha has values between 0 and 1".into());
    }
    if errs.is_empty() {
        Ok(())
    } else {
        let d = diagnose(c, &got);
        if !d.is_empty() {
            errs.push(format!("likely: {d}"));
        }
        Err(errs.join("; "))
    }
}

#[test]
fn every_format_matches_the_reference() {
    let mut failures = Vec::new();
    let mut sheet = Vec::new();
    for c in CASES {
        let s = open(c.file);
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap_or_else(|e| panic!("{}: decode: {e}", c.file));
        let mut res = check(c, &r);
        if s.info().has_alpha == c.opaque {
            let e = format!("ImageInfo::has_alpha is {}", s.info().has_alpha);
            res = Err(match res {
                Ok(()) => e,
                Err(f) => format!("{f}; {e}"),
            });
        }
        eprintln!(
            "{:<24} {:<10} {}",
            c.file,
            format!("{:?}", r.layout).split('(').next().unwrap_or(""),
            match &res {
                Ok(()) => "ok".to_string(),
                Err(e) => e.clone(),
            }
        );
        if let Err(e) = res {
            failures.push(format!("{}: {e}", c.file));
        }
        sheet.push((c.file, pixels(&r)));
    }
    if let Ok(path) = std::env::var("SPECULAR_ALPHA_SHEET") {
        contact_sheet::write(&PathBuf::from(path), &sheet);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Reduced decodes (1/8: 4×4 pixels) of every fixture average with alpha weighting, as
/// `whole::downscale` and the decoders' own reduced paths promise: transparent pixels add
/// nothing to the colour. Checked where it is unambiguous: alpha everywhere (the block mean),
/// colour on the bottom row, where each 8×8 block holds one opaque colour (white or green) and
/// transparent magenta, so unweighted averaging shows as pink. Gamma-space and linear-light
/// averaging agree there, as only one colour has weight. Lossy colour is not checked (the cell
/// edges blend, see [`Case::lossy`]); wavelet resolution levels are skipped (see below).
#[test]
fn reduced_decodes_are_alpha_weighted() {
    const S: u32 = 8;
    let mut failures = Vec::new();
    // JPEG 2000 with enough wavelet levels returns the LL band: a lowpass of each channel on
    // its own, so colour under transparent pixels leaks in. That is the codec's reduced image,
    // not a box filter; `rgba16-r1.jp2` covers the JP2 box-filter path.
    const WAVELET: &[&str] = &["rgba8.jp2"];
    for c in CASES
        .iter()
        .filter(|c| !c.opaque && !WAVELET.contains(&c.file))
    {
        let s = open(c.file);
        let r = match s.decode(&DecodeRequest::full(S), &CancelToken::new()) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: 1/{S}: {e}", c.file));
                continue;
            }
        };
        if (r.width, r.height) != (W / S, H / S) {
            failures.push(format!("{}: 1/{S} is {}x{}", c.file, r.width, r.height));
            continue;
        }
        let got = pixels(&r);
        let (mut ea, mut ec) = (0f32, 0f32);
        for oy in 0..H / S {
            for ox in 0..W / S {
                let block: Vec<[f32; 4]> = (0..S * S)
                    .map(|i| reference(c.reference, ox * S + i % S, oy * S + i / S))
                    .collect();
                let a = block.iter().map(|p| p[3]).sum::<f32>() / (S * S) as f32;
                let g = got[(oy * (W / S) + ox) as usize];
                ea = ea.max((g[3] - a).abs());
                if oy == H / S - 1 && !c.lossy {
                    let want = block
                        .iter()
                        .find(|p| p[3] == 1.0)
                        .copied()
                        .unwrap_or([0.0; 4]);
                    for k in 0..3 {
                        ec = ec.max((g[k] - want[k]).abs());
                    }
                }
            }
        }
        // Rounding at two levels, and lossy formats ring.
        let (tol_a, tol_c) = (c.tol_a + 1.0, c.tol_c.max(1.0) + 1.0);
        let line = format!(
            "{}: 1/{S} alpha off by {:.2} steps (tolerance {tol_a}), colour by {:.2} \
             (tolerance {tol_c})",
            c.file,
            ea * 255.0,
            ec * 255.0
        );
        eprintln!("{line}");
        if ea * 255.0 > tol_a || ec * 255.0 > tol_c {
            failures.push(line);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// APNG frame 1 draws the reference OVER frame 0 (the reference): straight-alpha blending
/// keeps the colour and raises alpha to a + a(1 - a).
#[test]
fn apng_over_blend_is_straight() {
    let s = open("apng.png");
    let mut f = s.frames(1).unwrap();
    let cancel = CancelToken::new();
    f.next_frame(&cancel).unwrap().unwrap();
    let r = f.next_frame(&cancel).unwrap().unwrap().raster;
    let got = pixels(&r);
    for y in 0..H {
        for x in 0..W {
            let w = reference(Ref::Rgba8, x, y);
            let g = got[(y * W + x) as usize];
            let a = w[3] + w[3] * (1.0 - w[3]);
            assert!(
                (g[3] - a).abs() <= 1.5 / 255.0,
                "alpha at ({x}, {y}): {} vs {a}",
                g[3]
            );
            if w[3] > 0.0 {
                for k in 0..3 {
                    assert!(
                        (g[k] - w[k]).abs() <= 1.5 / 255.0,
                        "colour at ({x}, {y}): {g:?} vs {w:?}"
                    );
                }
            }
        }
    }
}

/// Every fixture in tests/alpha is in [`CASES`] (so a new one cannot be forgotten).
#[test]
fn every_fixture_is_checked() {
    for e in std::fs::read_dir(dir()).unwrap() {
        let name = e.unwrap().file_name().into_string().unwrap();
        if name.ends_with(".py") || name.ends_with(".sh") || name == "rgba8-1bit.png" {
            continue;
        }
        assert!(
            CASES.iter().any(|c| c.file == name),
            "{name} is not in CASES"
        );
    }
}

/// The contact sheet, as an RGB PNG.
mod contact_sheet {
    use super::{H, W};
    use std::io::Write;
    use std::path::Path;

    const ZOOM: u32 = 4;
    const PER_ROW: u32 = 8;
    const PAD: u32 = 6;

    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut t = tag.to_vec();
        t.extend_from_slice(body);
        out.extend_from_slice(&t);
        out.extend_from_slice(&crc32(&t).to_be_bytes());
    }

    fn png(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
        let mut raw = Vec::new();
        for y in 0..h as usize {
            raw.push(0);
            raw.extend_from_slice(&rgb[y * w as usize * 3..(y + 1) * w as usize * 3]);
        }
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(&raw).unwrap();
        let z = enc.finish().unwrap();
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }

    /// Each format: over a checkerboard on top, colour with alpha ignored below. Files in
    /// order, left to right, top to bottom; the order is also written next to the sheet.
    pub fn write(path: &Path, tiles: &[(&str, Vec<[f32; 4]>)]) {
        let tw = W * ZOOM + PAD;
        let th = 2 * H * ZOOM + PAD;
        let rows = (tiles.len() as u32).div_ceil(PER_ROW);
        let (sw, sh) = (PER_ROW * tw + PAD, rows * th + PAD);
        let mut img = vec![40u8; (sw * sh * 3) as usize];
        for (i, (_, px)) in tiles.iter().enumerate() {
            let (ox, oy) = (PAD + i as u32 % PER_ROW * tw, PAD + i as u32 / PER_ROW * th);
            for y in 0..2 * H * ZOOM {
                for x in 0..W * ZOOM {
                    let p = px[((y / ZOOM % H) * W + x / ZOOM) as usize];
                    let rgb: [f32; 3] = if y < H * ZOOM {
                        let bg = if (x / 8 + y / 8) % 2 == 0 { 0.8 } else { 0.55 };
                        std::array::from_fn(|k| p[k] * p[3] + bg * (1.0 - p[3]))
                    } else {
                        [p[0], p[1], p[2]]
                    };
                    let o = (((oy + y) * sw + ox + x) * 3) as usize;
                    for k in 0..3 {
                        img[o + k] = (rgb[k].clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
            }
        }
        std::fs::write(path, png(sw, sh, &img)).unwrap();
        let names: Vec<&str> = tiles.iter().map(|t| t.0).collect();
        std::fs::write(path.with_extension("txt"), names.join("\n") + "\n").unwrap();
    }
}
