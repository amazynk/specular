//! End-to-end colour check: synthetic images in every colour path the viewer has (JPEG
//! Y'CbCr at three subsamplings, CMYK JPEG with a press profile, PNG sRGB / Display P3 ICC /
//! gAMA+cHRM / 16-bit sRGB, P3 ICC and gAMA+cHRM (kept at 16 bits) / grey ICC / straight
//! alpha / 16-bit grey with a grey ICC, gAMA or straight alpha (kept compact at 16 bits),
//! 16-bit TIFF (RGB, grey + grey ICC), PGM / PPM and PSD (P3 ICC, grey + straight alpha), all
//! kept at 16 bits,
//! AVIF BT.709 limited range and BT.2020 PQ,
//! JPEG XL XYB) are decoded by the app's decoders, uploaded and drawn 1:1 by the app's
//! renderer, encoded and dithered by the app's compositor into an 8-bit and a 10-bit SDR
//! swapchain format, read back, and compared with an independent reference: Little CMS for
//! the ICC cases, jxl-oxide for JPEG XL, and plain f64 formulas (BT.601/709/2020 matrices,
//! the PQ EOTF, linear-light compositing) elsewhere.
//!
//! Errors are in 8-bit code values. Three measures, each the max and mean over channels:
//! - patches: the average over the inside of each flat patch (dither averages out), against
//!   the reference's average;
//! - profiles: the ramps averaged across their width (per column of the horizontal ramps, per
//!   row of the vertical strips), which shows banding, offsets and chroma siting;
//! - pixels: every pixel (includes the ±½ LSB dither and the swapchain's quantisation).
//!
//! The GPU test skips without an adapter (CI runs it on lavapipe with SPECULAR_REQUIRE_GPU=1).
//! Fixtures that need external encoders (AVIF, JPEG XL) are committed under
//! tests/data/colour_e2e and made by the ignored `regenerate_fixtures` test (ffmpeg with
//! libaom-av1 and libjxl).

#[path = "colour_e2e/fixtures.rs"]
mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use fixtures::{Cell, H, PngTags, W, cell};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source, hdr};
use specular_decode::{DecodeRequest, ImageInfo, Registry};
use specular_gfx::{Compositor, Gpu, wgpu};
use specular_render::{GpuImage, ImageRenderer, PxRect, RenderOptions, View};

// ---- references ---------------------------------------------------------------------------

fn srgb_encode(l: f64) -> f64 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_decode(e: f64) -> f64 {
    if e <= 0.040_45 {
        e / 12.92
    } else {
        ((e + 0.055) / 1.055).powf(2.4)
    }
}

/// 0‥1 encoded → 8-bit code units (unrounded).
fn codes(e: [f64; 3]) -> [f64; 3] {
    e.map(|v| v.clamp(0.0, 1.0) * 255.0)
}

/// Little CMS, relative colorimetric with black point compensation (the viewer's default
/// policy), floating point, to sRGB. `T` is `[f64; N]` with lcms's float ranges (0‥1, CMYK
/// 0‥100).
fn lcms_to_srgb<T: lcms2::Pod>(
    src: &lcms2::Profile,
    format: lcms2::PixelFormat,
    px: &[T],
) -> Vec<[f64; 3]> {
    let t: lcms2::Transform<T, [f64; 3]> = lcms2::Transform::new_flags(
        src,
        format,
        &lcms2::Profile::new_srgb(),
        lcms2::PixelFormat::RGB_DBL,
        lcms2::Intent::RelativeColorimetric,
        lcms2::Flags::BLACKPOINT_COMPENSATION | lcms2::Flags::NO_CACHE,
    )
    .expect("lcms transform");
    let mut out = vec![[0.0; 3]; px.len()];
    t.transform_pixels(px, &mut out);
    out.into_iter().map(codes).collect()
}

/// libjpeg-turbo's "fancy" upsampling and colour conversion in f64: centred chroma, bilinear
/// between the nearest chroma samples (edges replicated), BT.601 full-range matrix, clipped.
fn ycbcr_reference(r: &Raster) -> Vec<[f64; 3]> {
    assert!(matches!(r.layout, PixelLayout::YCbCr), "{:?}", r.layout);
    let sample = |p: usize, x: u32, y: u32| -> f64 {
        let pl = &r.planes[p];
        let fx = (f64::from(x) + 0.5) / f64::from(pl.h_div) - 0.5;
        let fy = (f64::from(y) + 0.5) / f64::from(pl.v_div) - 0.5;
        let at = |i: i64, j: i64| {
            let i = i.clamp(0, i64::from(pl.width) - 1) as usize;
            let j = j.clamp(0, i64::from(pl.height) - 1) as usize;
            f64::from(pl.data[j * pl.stride + i])
        };
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        (at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx) * (1.0 - ty)
            + (at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx) * ty
    };
    pixels()
        .map(|(x, y)| {
            let yy = sample(0, x, y);
            let cb = sample(1, x, y) - 128.0;
            let cr = sample(2, x, y) - 128.0;
            [
                yy + 1.402 * cr,
                yy - 0.344_136 * cb - 0.714_136 * cr,
                yy + 1.772 * cb,
            ]
            .map(|v| v.clamp(0.0, 255.0))
        })
        .collect()
}

fn pixels() -> impl Iterator<Item = (u32, u32)> {
    (0..H).flat_map(|y| (0..W).map(move |x| (x, y)))
}

/// The SDR palette as 8-bit sRGB codes (what the 8-bit encoders get).
fn sdr8() -> Vec<[u8; 3]> {
    fixtures::sdr_palette()
        .image()
        .into_iter()
        .map(|c| c.map(|v| (v * 255.0).round() as u8))
        .collect()
}

fn as_f64(px: &[[u8; 3]]) -> Vec<[f64; 3]> {
    px.iter().map(|p| p.map(f64::from)).collect()
}

const BT709_TO_BT2020: [[f64; 3]; 3] = [
    [0.627_403_896, 0.329_283_039, 0.043_313_065],
    [0.069_097_289, 0.919_540_395, 0.011_362_316],
    [0.016_391_439, 0.088_013_308, 0.895_595_253],
];

fn mul(m: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2])
}

fn inverse(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    [
        [
            (e * i - f * h) / det,
            (c * h - b * i) / det,
            (b * f - c * e) / det,
        ],
        [
            (f * g - d * i) / det,
            (a * i - c * g) / det,
            (c * d - a * f) / det,
        ],
        [
            (d * h - e * g) / det,
            (b * g - a * h) / det,
            (a * e - b * d) / det,
        ],
    ]
}

const PQ_M1: f64 = 2610.0 / 16384.0;
const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f64 = 3424.0 / 4096.0;
const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

/// SMPTE ST 2084 inverse EOTF: cd/m² → signal.
fn pq_encode(nits: f64) -> f64 {
    let y = (nits / 10_000.0).clamp(0.0, 1.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * y) / (1.0 + PQ_C3 * y)).powf(PQ_M2)
}

/// SMPTE ST 2084 EOTF: signal → cd/m².
fn pq_decode(e: f64) -> f64 {
    let p = e.clamp(0.0, 1.0).powf(1.0 / PQ_M2);
    10_000.0 * ((p - PQ_C1).max(0.0) / (PQ_C2 - PQ_C3 * p)).powf(1.0 / PQ_M1)
}

/// The 10-bit BT.2020 PQ limited-range codes of the HDR palette.
fn pq_codes() -> Vec<[u16; 3]> {
    fixtures::hdr_palette()
        .image()
        .into_iter()
        .map(|lin709| {
            let lin2020 = mul(&BT709_TO_BT2020, lin709);
            let e = lin2020.map(|v| pq_encode(v.max(0.0) * 203.0));
            fixtures::ycbcr_limited(e, fixtures::BT2020, 10)
        })
        .collect()
}

/// The 8-bit BT.709 limited-range codes of the SDR palette.
fn bt709_codes() -> Vec<[u16; 3]> {
    fixtures::sdr_palette()
        .image()
        .into_iter()
        .map(|c| fixtures::ycbcr_limited(c, fixtures::BT709, 8))
        .collect()
}

// ---- the images -----------------------------------------------------------------------------

fn interleave(px: &[[u8; 3]]) -> Vec<u8> {
    px.iter().flatten().copied().collect()
}

fn jpeg(px: &[[u8; 3]], sampling: jpeg_encoder::SamplingFactor) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut out, 100);
    e.set_sampling_factor(sampling);
    e.encode(
        &interleave(px),
        W as u16,
        H as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .expect("jpeg");
    out
}

/// CMYK ink amounts as 8-bit codes (255 = full ink).
fn cmyk8() -> Vec<[u8; 4]> {
    fixtures::cmyk_palette()
        .image()
        .into_iter()
        .map(|c| c.map(|v| (v * 255.0).round() as u8))
        .collect()
}

fn cmyk_jpeg(px: &[[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut out, 100);
    e.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_4_4);
    e.add_icc_profile(&fixtures::cmyk_icc()).expect("icc");
    let data: Vec<u8> = px.iter().flatten().copied().collect();
    e.encode(&data, W as u16, H as u16, jpeg_encoder::ColorType::Cmyk)
        .expect("jpeg");
    out
}

fn png8(px: &[[u8; 3]], tags: &PngTags) -> Vec<u8> {
    let s: Vec<u16> = px.iter().flatten().map(|&v| u16::from(v)).collect();
    fixtures::png(3, 8, &s, tags)
}

/// 16-bit samples of the SDR palette: full precision, so the 16-bit path is checked beyond
/// 8-bit steps.
fn sdr16() -> Vec<[u16; 3]> {
    fixtures::sdr_palette()
        .image()
        .into_iter()
        .map(|c| c.map(|v| (v * 65535.0).round() as u16))
        .collect()
}

fn png16(px: &[[u16; 3]], tags: &PngTags) -> Vec<u8> {
    let s: Vec<u16> = px.iter().flatten().copied().collect();
    fixtures::png(3, 16, &s, tags)
}

/// 16-bit SDR PNGs reach the renderer at 16 bits, sRGB-coded (no narrowing in the decoder).
fn assert_wide(d: &Decoded) {
    assert_eq!(
        d.raster.layout,
        PixelLayout::Rgba16(hdr::HdrEncoding::SRGB),
        "16-bit PNG narrowed or re-encoded"
    );
    assert_eq!(d.content_peak, 1.0, "SDR samples tone-mapped");
}

fn p3_icc() -> Vec<u8> {
    moxcms::ColorProfile::new_display_p3()
        .encode()
        .expect("P3 profile")
}

/// gAMA 1/1.8 and Adobe RGB (1998) primaries with a D65 white in cHRM, no ICC profile.
const GAMA_CHRM: (u32, [f64; 8]) = (55_556, [0.3127, 0.3290, 0.64, 0.33, 0.21, 0.71, 0.15, 0.06]);

fn gama_chrm_profile() -> lcms2::Profile {
    let (g, c) = GAMA_CHRM;
    let xyy = |x: f64, y: f64| lcms2::CIExyY { x, y, Y: 1.0 };
    let curve = lcms2::ToneCurve::new(100_000.0 / f64::from(g));
    lcms2::Profile::new_rgb(
        &xyy(c[0], c[1]),
        &lcms2::CIExyYTRIPLE {
            Red: xyy(c[2], c[3]),
            Green: xyy(c[4], c[5]),
            Blue: xyy(c[6], c[7]),
        },
        &[&curve, &curve, &curve],
    )
    .expect("gAMA/cHRM profile")
}

fn gray_profile() -> lcms2::Profile {
    lcms2::Profile::new_gray(
        &lcms2::CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        },
        &lcms2::ToneCurve::new(1.8),
    )
    .expect("grey profile")
}

fn gray8() -> Vec<u8> {
    fixtures::sdr_palette()
        .image()
        .into_iter()
        .map(|c| ((0.25 * c[0] + 0.6 * c[1] + 0.15 * c[2]) * 255.0).round() as u8)
        .collect()
}

/// 16-bit grey of the SDR palette (the weights of [`gray8`], full precision).
fn gray16() -> Vec<u16> {
    fixtures::sdr_palette()
        .image()
        .into_iter()
        .map(|c| ((0.25 * c[0] + 0.6 * c[1] + 0.15 * c[2]) * 65535.0).round() as u16)
        .collect()
}

/// 16-bit grey PNGs reach the renderer compact (2 or 4 bytes a pixel), not widened to RGBA.
fn assert_grey16(d: &Decoded, alpha: bool) {
    let want = if alpha {
        PixelLayout::GrayAlpha16
    } else {
        PixelLayout::Gray16
    };
    assert_eq!(d.raster.layout, want, "16-bit grey widened or narrowed");
    assert_eq!(d.content_peak, 1.0, "SDR samples tone-mapped");
}

/// A little-endian, uncompressed, single-strip TIFF of `spp` 16-bit samples per pixel
/// (`photometric` 1 grey or 2 RGB; a second / fourth sample is straight alpha).
fn tiff16(spp: u16, photometric: u16, samples: &[u16], icc: Option<&[u8]>) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    // (tag, type, count, value bytes): SHORT 3, LONG 4, UNDEFINED 7.
    let short = |v: u16| v.to_le_bytes().to_vec();
    let long = |v: u32| v.to_le_bytes().to_vec();
    let mut e: Vec<(u16, u16, u32, Vec<u8>)> = vec![
        (256, 4, 1, long(W)),
        (257, 4, 1, long(H)),
        (
            258,
            3,
            u32::from(spp),
            (0..spp).flat_map(|_| 16u16.to_le_bytes()).collect(),
        ),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(photometric)),
        (273, 4, 1, long(0)), // patched below
        (277, 3, 1, short(spp)),
        (278, 4, 1, long(H)),
        (279, 4, 1, long(data.len() as u32)),
        (284, 3, 1, short(1)),
    ];
    if spp == 2 || spp == 4 {
        e.push((338, 3, 1, short(2)));
    }
    if let Some(icc) = icc {
        e.push((34675, 7, icc.len() as u32, icc.to_vec()));
    }
    let ifd_len = 2 + 12 * e.len() + 4;
    let mut blobs = Vec::new();
    let mut ifd = (e.len() as u16).to_le_bytes().to_vec();
    let blob_at = 8 + ifd_len;
    let blob_total: usize = e
        .iter()
        .filter(|x| x.3.len() > 4)
        .map(|x| x.3.len().next_multiple_of(2))
        .sum();
    let data_at = (blob_at + blob_total) as u32;
    for (tag, ty, n, mut v) in e {
        if tag == 273 {
            v = long(data_at);
        }
        ifd.extend(tag.to_le_bytes());
        ifd.extend(ty.to_le_bytes());
        ifd.extend(n.to_le_bytes());
        if v.len() > 4 {
            ifd.extend(((blob_at + blobs.len()) as u32).to_le_bytes());
            blobs.extend_from_slice(&v);
            if v.len() % 2 == 1 {
                blobs.push(0);
            }
        } else {
            v.resize(4, 0);
            ifd.extend(v);
        }
    }
    ifd.extend(0u32.to_le_bytes());
    let mut f = b"II*\0".to_vec();
    f.extend(8u32.to_le_bytes());
    f.extend(ifd);
    f.extend(blobs);
    f.extend(data);
    f
}

/// A binary PGM (1 sample a pixel) or PPM (3) with maxval 65535.
fn pnm16(channels: usize, samples: &[u16]) -> Vec<u8> {
    let magic = if channels == 1 { 5 } else { 6 };
    let mut f = format!("P{magic}\n{W} {H}\n65535\n").into_bytes();
    f.extend(samples.iter().flat_map(|v| v.to_be_bytes()));
    f
}

/// An uncompressed 16-bit PSD (`mode` 1 grey or 3 RGB) of interleaved `samples`, `planes`
/// per pixel (a plane past the colour ones is alpha: no layer section), with an optional
/// ICC resource. Not written by Photoshop, so the colour is not matted.
fn psd16(mode: u16, planes: usize, samples: &[u16], icc: Option<&[u8]>) -> Vec<u8> {
    let mut f = b"8BPS".to_vec();
    f.extend(1u16.to_be_bytes());
    f.extend([0; 6]);
    f.extend((planes as u16).to_be_bytes());
    f.extend(H.to_be_bytes());
    f.extend(W.to_be_bytes());
    f.extend(16u16.to_be_bytes());
    f.extend(mode.to_be_bytes());
    f.extend(0u32.to_be_bytes()); // colour mode data
    let mut res = Vec::new();
    if let Some(icc) = icc {
        res.extend(b"8BIM");
        res.extend(1039u16.to_be_bytes());
        res.extend([0, 0]); // empty name, padded
        res.extend((icc.len() as u32).to_be_bytes());
        res.extend(icc);
        if icc.len() % 2 == 1 {
            res.push(0);
        }
    }
    f.extend((res.len() as u32).to_be_bytes());
    f.extend(res);
    f.extend(0u32.to_be_bytes()); // layer and mask information
    f.extend(0u16.to_be_bytes()); // raw
    for c in 0..planes {
        f.extend(
            samples
                .iter()
                .skip(c)
                .step_by(planes)
                .flat_map(|v| v.to_be_bytes()),
        );
    }
    f
}

/// 16-bit RGB of the SDR palette as references in 8-bit code units.
fn want16(s16: &[[u16; 3]]) -> Vec<[f64; 3]> {
    s16.iter()
        .map(|c| c.map(|v| f64::from(v) * 255.0 / 65535.0))
        .collect()
}

/// 16-bit grey with the straight alpha ramp, and its reference blended over [`BG`].
fn grey_alpha16() -> (Vec<u16>, Vec<[f64; 3]>) {
    let s: Vec<u16> = gray16()
        .iter()
        .zip(pixels())
        .flat_map(|(&v, (x, y))| [v, u16::from(fixtures::alpha_at(x, y)) * 257])
        .collect();
    let bg = srgb_decode(BG);
    let want = s
        .chunks_exact(2)
        .map(|p| {
            let a = f64::from(p[1]) / 65535.0;
            let l = srgb_decode(f64::from(p[0]) / 65535.0);
            [srgb_encode(l * a + bg * (1.0 - a)) * 255.0; 3]
        })
        .collect();
    (s, want)
}

/// Cases for the other formats that keep 16 bits (TIFF, PGM/PPM, PSD).
fn wide_format_cases(v: &mut Vec<Case>) {
    let check = |label: &'static str, want: &Vec<[f64; 3]>| Check {
        label,
        want: want.clone(),
        tol: EXACT,
        profiles: true,
    };
    let s16 = sdr16();
    let flat: Vec<u16> = s16.iter().flatten().copied().collect();
    let want = want16(&s16);
    for (name, file, bytes) in [
        ("TIFF 16-bit RGB", "rgb16.tif", tiff16(3, 2, &flat, None)),
        ("PPM 16-bit", "rgb16.ppm", pnm16(3, &flat)),
    ] {
        let want = want.clone();
        v.push(Case {
            name,
            file,
            bytes,
            bg: BG,
            checks: Box::new(move |d| {
                assert_wide(d);
                vec![check("vs source", &want)]
            }),
        });
    }
    {
        let icc = p3_icc();
        let p = lcms2::Profile::new_icc(&icc).expect("P3");
        let px: Vec<[f64; 3]> = s16
            .iter()
            .map(|c| c.map(|v| f64::from(v) / 65535.0))
            .collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::RGB_DBL, &px);
        v.push(Case {
            name: "PSD 16-bit RGB + Display P3 ICC",
            file: "p3-16.psd",
            bytes: psd16(3, 3, &flat, Some(&icc)),
            bg: BG,
            checks: Box::new(move |d| {
                assert_wide(d);
                vec![check("vs lcms2", &want)]
            }),
        });
    }
    let g = gray16();
    {
        let icc = gray_profile().icc().expect("grey icc");
        let px: Vec<f64> = g.iter().map(|&v| f64::from(v) / 65535.0).collect();
        let want = lcms_to_srgb(&gray_profile(), lcms2::PixelFormat::GRAY_DBL, &px);
        v.push(Case {
            name: "TIFF 16-bit grey + grey ICC (γ 1.8)",
            file: "grey16.tif",
            bytes: tiff16(1, 1, &g, Some(&icc)),
            bg: BG,
            checks: Box::new(move |d| {
                assert_grey16(d, false);
                vec![check("vs lcms2", &want)]
            }),
        });
    }
    {
        let want: Vec<[f64; 3]> = g
            .iter()
            .map(|&v| [f64::from(v) * 255.0 / 65535.0; 3])
            .collect();
        v.push(Case {
            name: "PGM 16-bit",
            file: "grey16.pgm",
            bytes: pnm16(1, &g),
            bg: BG,
            checks: Box::new(move |d| {
                assert_grey16(d, false);
                vec![check("vs source", &want)]
            }),
        });
    }
    let (ga, want) = grey_alpha16();
    for (name, file, bytes) in [
        (
            "TIFF 16-bit grey straight alpha over grey",
            "grey-alpha16.tif",
            tiff16(2, 1, &ga, None),
        ),
        (
            "PSD 16-bit grey straight alpha over grey",
            "grey-alpha16.psd",
            psd16(1, 2, &ga, None),
        ),
    ] {
        let want = want.clone();
        v.push(Case {
            name,
            file,
            bytes,
            bg: BG,
            checks: Box::new(move |d| {
                assert_grey16(d, true);
                vec![check("vs linear-light blend", &want)]
            }),
        });
    }
}

/// The background of the straight-alpha case: sRGB grey 50 % (the viewer's solid backdrop
/// option), as encoded sRGB.
const BG: f64 = 0.5;

fn alpha_px() -> Vec<[u8; 4]> {
    sdr8()
        .into_iter()
        .zip(pixels())
        .map(|(c, (x, y))| [c[0], c[1], c[2], fixtures::alpha_at(x, y)])
        .collect()
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/colour_e2e")
}

fn fixture(name: &str) -> Vec<u8> {
    let p = data_dir().join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e} (run regenerate_fixtures)", p.display()))
}

// ---- harness --------------------------------------------------------------------------------

/// The output tag of a drawing from tiles. Tiles store 8-bit sRGB, so a tiled output is
/// held to [`ROUNDED`] at least.
const TILED: &str = " tiles";

/// What the decoder gave the renderer.
struct Decoded {
    info: ImageInfo,
    raster: Raster,
    content_peak: f32,
    /// Readback per output format, 8-bit code units (10-bit output scaled by 255/1023).
    outputs: Vec<(String, Vec<[f64; 3]>)>,
}

struct Harness {
    gpu: Gpu,
    renderer: ImageRenderer,
    registry: Registry,
    outputs: Vec<(&'static str, wgpu::TextureFormat, Compositor)>,
    next_id: u64,
}

impl Harness {
    fn new(gpu: Gpu) -> Harness {
        let mut renderer = ImageRenderer::new(&gpu, 32 << 20);
        // An SDR display, as the app sets it up (app.rs: set_display_peak(1.0), set_output
        // with no HDR peak); the display profile is sRGB (no display transform).
        renderer.set_display_peak(1.0);
        let outputs = [
            ("8-bit", wgpu::TextureFormat::Rgba8Unorm, 8),
            ("10-bit", wgpu::TextureFormat::Rgb10a2Unorm, 10),
        ]
        .into_iter()
        .map(|(name, fmt, bits)| {
            let mut c = Compositor::new(&gpu, fmt, bits, W, H);
            c.set_output(&gpu, fmt, None);
            (name, fmt, c)
        })
        .collect();
        Harness {
            gpu,
            renderer,
            registry: Registry::new(),
            outputs,
            next_id: 1,
        }
    }

    /// Opens, decodes and uploads like the app's loader and `App::upload`, then draws one
    /// frame per output format like `App::render` (image pass into the scene, compositor
    /// copy, final encode + dither into the swapchain image).
    fn run(&mut self, file: &str, bytes: Vec<u8>, bg: f64) -> Decoded {
        let src = Arc::new(Source::from_vec(file, bytes));
        let session = self
            .registry
            .open(src)
            .unwrap_or_else(|e| panic!("{file}: open: {e}"));
        let info = session.info().clone();
        assert_eq!((info.width, info.height), (W, H), "{file}");
        let lut = info
            .icc_profile
            .as_ref()
            .and_then(|p| specular_render::icc::build_lut(p));
        let raster = session
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap_or_else(|e| panic!("{file}: decode: {e}"));
        let lin = srgb_decode(bg) as f32;
        let solid = [lin, lin, lin, 1.0];
        // The solid backdrop (app/inspect.rs `apply_backdrop`): both checker colours equal.
        let opts = RenderOptions {
            hq: true,
            canvas: solid,
            checker_a: solid,
            checker_b: solid,
            checker_cell: 1e6,
            ..RenderOptions::default()
        };
        // Y'CbCr has two conversion paths (the path selector picks one per GPU and image
        // size): both are drawn. Wide SDR rasters (16-bit PNG) are drawn from the half-float
        // overview and, as when zoomed in, from 8-bit tiles over a half-size overview.
        let wide_sdr = raster.layout.is_wide() && !raster.layout.is_hdr();
        let paths: &[(Option<usize>, &str)] = if matches!(raster.layout, PixelLayout::YCbCr) {
            &[(Some(0), " cpu"), (Some(1), " gpu")]
        } else if wide_sdr {
            &[(None, ""), (None, TILED)]
        } else {
            &[(None, "")]
        };
        let mut outputs = Vec::new();
        let mut content_peak = 0.0;
        for &(path, tag) in paths {
            self.renderer.force_colour_path(path);
            let id = self.next_id;
            self.next_id += 1;
            let mut img =
                self.renderer
                    .create_image(&self.gpu, id, info.width, info.height, lut.as_ref());
            if tag == TILED {
                let half = session
                    .decode(&DecodeRequest::full(2), &CancelToken::new())
                    .unwrap_or_else(|e| panic!("{file}: decode 1/2: {e}"));
                self.renderer.upload_overview(&self.gpu, &mut img, &half);
                let stored = self
                    .renderer
                    .upload_tiles(&self.gpu, &mut img, &raster, 0, &[(0, 0)]);
                assert_eq!(stored, 1, "{file}: tile not stored");
            } else {
                self.renderer.upload_overview(&self.gpu, &mut img, &raster);
            }
            for i in 0..self.outputs.len() {
                let (name, fmt, _) = self.outputs[i];
                outputs.push((format!("{name}{tag}"), self.draw(i, fmt, &img, &opts)));
            }
            content_peak = img.content_peak;
            self.renderer.release_image(id);
        }
        self.renderer.force_colour_path(None);
        Decoded {
            info,
            raster,
            content_peak,
            outputs,
        }
    }

    fn draw(
        &mut self,
        out: usize,
        fmt: wgpu::TextureFormat,
        img: &GpuImage,
        opts: &RenderOptions,
    ) -> Vec<[f64; 3]> {
        let gpu = &self.gpu;
        let comp = &self.outputs[out].2;
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("colour e2e swapchain stand-in"),
            size: wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: fmt,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let tv = target.create_view(&Default::default());
        // 1:1, the image centred in a viewport of its own size.
        let view = View::fixed(
            (W, H),
            Orientation::NORMAL,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: W as f32,
                h: H as f32,
            },
            1.0,
            (W as f32 * 0.5, H as f32 * 0.5),
        );
        let row = (W * 4).div_ceil(256) * 256;
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colour e2e readback"),
            size: u64::from(row * H),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        self.renderer.render(
            gpu,
            &mut enc,
            &comp.scene.view,
            (W, H),
            Some(img),
            &view,
            opts,
        );
        comp.begin(&mut enc);
        comp.finish(&mut enc, &tv);
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(H),
                },
            },
            wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().expect("mapped");
        pixels()
            .map(|(x, y)| {
                let o = (y * row + x * 4) as usize;
                let b = [data[o], data[o + 1], data[o + 2], data[o + 3]];
                match fmt {
                    wgpu::TextureFormat::Rgb10a2Unorm => {
                        let v = u32::from_le_bytes(b);
                        [0, 10, 20].map(|s| f64::from((v >> s) & 1023) * 255.0 / 1023.0)
                    }
                    _ => [b[0], b[1], b[2]].map(f64::from),
                }
            })
            .collect()
    }
}

// ---- metrics --------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
struct Stat {
    max: f64,
    sum: f64,
    n: usize,
    /// Where the maximum is: x, y (a region's centre or a profile's position), channel,
    /// rendered and reference values.
    worst: (u32, u32, usize, f64, f64),
}

impl Stat {
    fn add(&mut self, at: (u32, u32), c: usize, got: f64, want: f64) {
        let d = (got - want).abs();
        if d > self.max {
            self.max = d;
            self.worst = (at.0, at.1, c, got, want);
        }
        self.sum += d;
        self.n += 1;
    }
    fn mean(&self) -> f64 {
        self.sum / self.n.max(1) as f64
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Errors {
    patch: Stat,
    profile: Stat,
    pixel: Stat,
}

/// Inset of the patch and ramp averages from region edges (the reference upsamples chroma
/// exactly, but a source-referenced check should not see edges).
const MARGIN: u32 = 3;

fn mean_over(px: &[[f64; 3]], xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> [f64; 3] {
    let mut s = [0.0; 3];
    let mut n = 0.0;
    for y in ys {
        for x in xs.clone() {
            let p = px[(y * W + x) as usize];
            for c in 0..3 {
                s[c] += p[c];
            }
            n += 1.0;
        }
    }
    s.map(|v| v / n)
}

fn measure(got: &[[f64; 3]], want: &[[f64; 3]], profiles: bool) -> Errors {
    let mut e = Errors::default();
    let diff = |s: &mut Stat, at: (u32, u32), g: [f64; 3], w: [f64; 3]| {
        for c in 0..3 {
            s.add(at, c, g[c], w[c]);
        }
    };
    for i in 0..24u32 {
        let (x0, y0) = (i % 8 * 16, i / 8 * 16);
        let xs = x0 + MARGIN..x0 + 16 - MARGIN;
        let ys = y0 + MARGIN..y0 + 16 - MARGIN;
        diff(
            &mut e.patch,
            (x0 + 8, y0 + 8),
            mean_over(got, xs.clone(), ys.clone()),
            mean_over(want, xs, ys),
        );
    }
    if profiles {
        for band in 0..2u32 {
            let y0 = 48 + band * 16;
            for x in 0..W {
                let ys = y0 + MARGIN..y0 + 16 - MARGIN;
                diff(
                    &mut e.profile,
                    (x, y0 + 8),
                    mean_over(got, x..x + 1, ys.clone()),
                    mean_over(want, x..x + 1, ys),
                );
            }
        }
        for strip in 0..8u32 {
            let x0 = strip * 16;
            for y in 80 + MARGIN..H {
                let xs = x0 + MARGIN..x0 + 16 - MARGIN;
                diff(
                    &mut e.profile,
                    (x0 + 8, y),
                    mean_over(got, xs.clone(), y..y + 1),
                    mean_over(want, xs, y..y + 1),
                );
            }
        }
        for ((g, w), at) in got.iter().zip(want).zip(pixels()) {
            diff(&mut e.pixel, at, *g, *w);
        }
    }
    e
}

/// Allowed errors (8-bit code values): patch-average, profile and per-pixel maxima.
#[derive(Clone, Copy, Debug)]
struct Tol {
    patch: f64,
    profile: f64,
    pixel: f64,
}

impl Tol {
    fn at_least(self, o: Tol) -> Tol {
        Tol {
            patch: self.patch.max(o.patch),
            profile: self.profile.max(o.profile),
            pixel: self.pixel.max(o.pixel),
        }
    }
}

/// An identity path: integer sRGB samples stored unchanged in the 8-bit sRGB layout. What is
/// left is the driver (hardware sRGB decode is not exact: about ±0.3 code at mid grey on Mesa
/// RADV and llvmpipe, and RADV writes half floats rounding toward zero), the ±½ LSB dither,
/// and the swapchain's rounding, so a pixel may be one code off.
const EXACT: Tol = Tol {
    patch: 0.75,
    profile: 0.75,
    pixel: 1.5,
};

/// A path whose reference is fractional and which the app rounds once to 8 bits on the way:
/// every transform into the 8-bit sRGB storage layout (Y'CbCr on the CPU or in convert.wgsl,
/// ICC source transforms, AVIF's CPU conversion). EXACT plus the storage rounding (½ code at most, ¼ on average).
const ROUNDED: Tol = Tol {
    patch: 1.0,
    profile: 1.25,
    pixel: 2.0,
};

/// One comparison of a case.
struct Check {
    label: &'static str,
    want: Vec<[f64; 3]>,
    tol: Tol,
    /// Ramps and pixels compared too (not for references that ignore lossy coding).
    profiles: bool,
}

/// Builds a case's checks once the image is decoded (some references need the decoded planes
/// or the content peak).
type Checks = Box<dyn Fn(&Decoded) -> Vec<Check>>;

struct Case {
    name: &'static str,
    file: &'static str,
    bytes: Vec<u8>,
    bg: f64,
    checks: Checks,
}

fn source_check(want: Vec<[f64; 3]>, tol: f64) -> Check {
    Check {
        label: "vs source",
        want,
        tol: Tol {
            patch: tol,
            profile: f64::INFINITY,
            pixel: f64::INFINITY,
        },
        profiles: false,
    }
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    let src8 = sdr8();
    for (name, file, sampling) in [
        (
            "JPEG 4:4:4",
            "c444.jpg",
            jpeg_encoder::SamplingFactor::R_4_4_4,
        ),
        (
            "JPEG 4:2:2",
            "c422.jpg",
            jpeg_encoder::SamplingFactor::R_4_2_2,
        ),
        (
            "JPEG 4:2:0",
            "c420.jpg",
            jpeg_encoder::SamplingFactor::R_4_2_0,
        ),
    ] {
        let src = as_f64(&src8);
        v.push(Case {
            name,
            file,
            bytes: jpeg(&src8, sampling),
            bg: BG,
            checks: Box::new(move |d| {
                vec![
                    Check {
                        label: "vs f64 BT.601 of decoded planes",
                        want: ycbcr_reference(&d.raster),
                        tol: ROUNDED,
                        profiles: true,
                    },
                    // Quality 100 still rounds Y'CbCr to 8 bits and quantises the DCT.
                    source_check(src.clone(), 1.5),
                ]
            }),
        });
    }
    {
        let want = as_f64(&src8);
        v.push(Case {
            name: "PNG sRGB 8-bit",
            file: "srgb.png",
            bytes: png8(
                &src8,
                &PngTags {
                    srgb: true,
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |_| {
                vec![Check {
                    label: "vs source",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let icc = p3_icc();
        let p = lcms2::Profile::new_icc(&icc).expect("P3");
        let px: Vec<[f64; 3]> = src8
            .iter()
            .map(|c| c.map(|v| f64::from(v) / 255.0))
            .collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::RGB_DBL, &px);
        v.push(Case {
            name: "PNG Display P3 ICC",
            file: "p3.png",
            bytes: png8(
                &src8,
                &PngTags {
                    icc: Some(icc),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |_| {
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: ROUNDED,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let p = gama_chrm_profile();
        let px: Vec<[f64; 3]> = src8
            .iter()
            .map(|c| c.map(|v| f64::from(v) / 255.0))
            .collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::RGB_DBL, &px);
        v.push(Case {
            name: "PNG gAMA 1/1.8 + cHRM Adobe",
            file: "gama.png",
            bytes: png8(
                &src8,
                &PngTags {
                    gamma: Some(GAMA_CHRM.0),
                    chrm: Some(GAMA_CHRM.1),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |_| {
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: ROUNDED,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let s16 = sdr16();
        let want: Vec<[f64; 3]> = s16
            .iter()
            .map(|c| c.map(|v| f64::from(v) * 255.0 / 65535.0))
            .collect();
        v.push(Case {
            name: "PNG 16-bit sRGB",
            file: "srgb16.png",
            bytes: png16(&s16, &PngTags::default()),
            bg: BG,
            checks: Box::new(move |d| {
                assert_wide(d);
                vec![Check {
                    label: "vs source",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let s16 = sdr16();
        let icc = p3_icc();
        let p = lcms2::Profile::new_icc(&icc).expect("P3");
        let px: Vec<[f64; 3]> = s16
            .iter()
            .map(|c| c.map(|v| f64::from(v) / 65535.0))
            .collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::RGB_DBL, &px);
        v.push(Case {
            name: "PNG 16-bit Display P3 ICC",
            file: "p3-16.png",
            bytes: png16(
                &s16,
                &PngTags {
                    icc: Some(icc),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |d| {
                assert_wide(d);
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        // gAMA/cHRM rather than iCCP: the synthesised profile applies to the 16-bit samples.
        let s16 = sdr16();
        let p = gama_chrm_profile();
        let px: Vec<[f64; 3]> = s16
            .iter()
            .map(|c| c.map(|v| f64::from(v) / 65535.0))
            .collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::RGB_DBL, &px);
        v.push(Case {
            name: "PNG 16-bit gAMA + cHRM",
            file: "gama16.png",
            bytes: png16(
                &s16,
                &PngTags {
                    gamma: Some(GAMA_CHRM.0),
                    chrm: Some(GAMA_CHRM.1),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |d| {
                assert_wide(d);
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let ink = cmyk8();
        let icc = fixtures::cmyk_icc();
        let p = lcms2::Profile::new_icc(&icc).expect("CMYK profile");
        let px: Vec<[f64; 4]> = ink.iter().map(|c| c.map(|v| f64::from(v) / 2.55)).collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::CMYK_DBL, &px);
        v.push(Case {
            name: "JPEG CMYK + press ICC",
            file: "cmyk.jpg",
            bytes: cmyk_jpeg(&ink),
            bg: BG,
            checks: Box::new(move |_| {
                // The reference starts from the ink amounts before JPEG coding (the decoder
                // converts to RGB on the CPU, so its CMYK is not visible here). ROUNDED: the
                // decoder's CmykConverter interpolates this profile's own A2B table the way
                // Little CMS does (specular-color tests/cmyk_table.rs: within one code), and
                // quality-100 JPEG coding of the ink stays within the rounding allowance.
                vec![Check {
                    label: "vs lcms2 of source ink",
                    want: want.clone(),
                    tol: ROUNDED,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let g = gray8();
        let icc = gray_profile().icc().expect("grey icc");
        let p = gray_profile();
        let px: Vec<f64> = g.iter().map(|&v| f64::from(v) / 255.0).collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::GRAY_DBL, &px);
        let s: Vec<u16> = g.iter().map(|&v| u16::from(v)).collect();
        v.push(Case {
            name: "PNG grey + grey ICC (γ 1.8)",
            file: "grey.png",
            bytes: fixtures::png(
                1,
                8,
                &s,
                &PngTags {
                    icc: Some(icc),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |_| {
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: ROUNDED,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let g = gray16();
        let icc = gray_profile().icc().expect("grey icc");
        let p = gray_profile();
        let px: Vec<f64> = g.iter().map(|&v| f64::from(v) / 65535.0).collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::GRAY_DBL, &px);
        v.push(Case {
            name: "PNG 16-bit grey + grey ICC (γ 1.8)",
            file: "grey16.png",
            bytes: fixtures::png(
                1,
                16,
                &g,
                &PngTags {
                    icc: Some(icc),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |d| {
                assert_grey16(d, false);
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        // gAMA alone (no cHRM): grey needs only the curve, so the reference is the same γ 1.8
        // grey profile.
        let g = gray16();
        let p = gray_profile();
        let px: Vec<f64> = g.iter().map(|&v| f64::from(v) / 65535.0).collect();
        let want = lcms_to_srgb(&p, lcms2::PixelFormat::GRAY_DBL, &px);
        v.push(Case {
            name: "PNG 16-bit grey gAMA (1/1.8)",
            file: "grey16-gama.png",
            bytes: fixtures::png(
                1,
                16,
                &g,
                &PngTags {
                    gamma: Some(GAMA_CHRM.0),
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |d| {
                assert_grey16(d, false);
                vec![Check {
                    label: "vs lcms2",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let g = gray16();
        let s: Vec<u16> = g
            .iter()
            .zip(pixels())
            .flat_map(|(&v, (x, y))| [v, u16::from(fixtures::alpha_at(x, y)) * 257])
            .collect();
        let bg = srgb_decode(BG);
        let want: Vec<[f64; 3]> = s
            .chunks_exact(2)
            .map(|p| {
                let a = f64::from(p[1]) / 65535.0;
                let l = srgb_decode(f64::from(p[0]) / 65535.0);
                [srgb_encode(l * a + bg * (1.0 - a)) * 255.0; 3]
            })
            .collect();
        v.push(Case {
            name: "PNG 16-bit grey straight alpha over grey",
            file: "grey-alpha16.png",
            bytes: fixtures::png(
                2,
                16,
                &s,
                &PngTags {
                    srgb: true,
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |d| {
                assert_grey16(d, true);
                vec![Check {
                    label: "vs linear-light blend",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    wide_format_cases(&mut v);
    {
        let px = alpha_px();
        let s: Vec<u16> = px.iter().flatten().map(|&v| u16::from(v)).collect();
        let bg = srgb_decode(BG);
        let want: Vec<[f64; 3]> = px
            .iter()
            .map(|p| {
                let a = f64::from(p[3]) / 255.0;
                [0, 1, 2].map(|c| {
                    srgb_encode(srgb_decode(f64::from(p[c]) / 255.0) * a + bg * (1.0 - a)) * 255.0
                })
            })
            .collect();
        v.push(Case {
            name: "PNG straight alpha over grey",
            file: "alpha.png",
            bytes: fixtures::png(
                4,
                8,
                &s,
                &PngTags {
                    srgb: true,
                    ..Default::default()
                },
            ),
            bg: BG,
            checks: Box::new(move |_| {
                vec![Check {
                    label: "vs linear-light blend",
                    want: want.clone(),
                    tol: EXACT,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let want: Vec<[f64; 3]> = bt709_codes()
            .into_iter()
            .map(|c| codes(fixtures::rgb_from_limited(c, fixtures::BT709, 8)))
            .collect();
        v.push(Case {
            name: "AVIF BT.709 limited 4:4:4",
            file: "bt709-limited.avif",
            bytes: fixture("bt709-limited.avif"),
            bg: BG,
            checks: Box::new(move |_| {
                // The decoder converts to 8-bit RGB on the CPU (rounding: ½ code more).
                vec![Check {
                    label: "vs f64 BT.709 of the coded planes",
                    want: want.clone(),
                    tol: ROUNDED,
                    profiles: true,
                }]
            }),
        });
    }
    {
        let to709 = inverse(&BT709_TO_BT2020);
        let lin: Vec<[f64; 3]> = pq_codes()
            .into_iter()
            .map(|c| {
                let e = fixtures::rgb_from_limited(c, fixtures::BT2020, 10);
                mul(&to709, e.map(|v| pq_decode(v) / 203.0)).map(|v| v.max(0.0))
            })
            .collect();
        v.push(Case {
            name: "AVIF BT.2020 PQ → SDR",
            file: "bt2020-pq.avif",
            bytes: fixture("bt2020-pq.avif"),
            bg: BG,
            checks: Box::new(move |d| {
                // Tone mapping (the viewer's own curve, specular_core::hdr::tone_map_scale, a
                // knee at 0.8 and a Reinhard shoulder to the content peak) is a design choice
                // with no external reference; the reference applies the same curve to its own
                // f64 PQ decode. The bound covers 10-bit PQ steps near the peak and the
                // half-float storage of linear light.
                let peak = d.content_peak;
                let want = lin
                    .iter()
                    .map(|&c| {
                        let m = c[0].max(c[1]).max(c[2]);
                        let s = f64::from(hdr::tone_map_scale(m as f32, peak, 1.0));
                        c.map(|v| srgb_encode(v * s) * 255.0)
                    })
                    .collect();
                vec![Check {
                    label: "vs f64 PQ + in-tree tone curve",
                    want,
                    tol: Tol {
                        patch: 1.0,
                        profile: 1.5,
                        pixel: 2.5,
                    },
                    profiles: true,
                }]
            }),
        });
    }
    {
        let bytes = fixture("xyb.jxl");
        let oxide = jxl_reference(&bytes);
        let src = as_f64(&src8);
        v.push(Case {
            name: "JPEG XL XYB (d 0.5)",
            file: "xyb.jxl",
            bytes,
            bg: BG,
            checks: Box::new(move |_| {
                vec![
                    Check {
                        label: "vs jxl-oxide",
                        want: oxide.clone(),
                        tol: ROUNDED,
                        profiles: true,
                    },
                    // A property of the encoder, not of the viewer: libjxl at distance 0.5
                    // moves the darkest patches by up to ~5.5 codes (jxl-oxide agrees with
                    // the viewer, above). Catches gross errors such as a missed XYB inverse.
                    source_check(src.clone(), 6.0),
                ]
            }),
        });
    }
    v
}

/// jxl-oxide's decode to sRGB (f32), in code units.
fn jxl_reference(bytes: &[u8]) -> Vec<[f64; 3]> {
    let mut img = jxl_oxide::JxlImage::builder()
        .read(std::io::Cursor::new(bytes))
        .expect("jxl-oxide");
    img.request_color_encoding(jxl_oxide::EnumColourEncoding::srgb(
        jxl_oxide::RenderingIntent::Relative,
    ));
    let render = img.render_frame(0).expect("jxl-oxide render");
    let mut stream = render.stream();
    let ch = stream.channels() as usize;
    let mut buf = vec![0f32; (W * H) as usize * ch];
    stream.write_to_buffer(&mut buf);
    buf.chunks_exact(ch)
        .map(|p| [0, 1, 2].map(|c| f64::from(p[c]).clamp(0.0, 1.0) * 255.0))
        .collect()
}

// ---- the test -------------------------------------------------------------------------------

#[test]
fn rendered_colours_match_references() {
    let t0 = Instant::now();
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU, skipped: {e}");
            return;
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    eprintln!(
        "adapter: {} ({:?}, {:?}, driver {} {})",
        gpu.info.name,
        gpu.info.device_type,
        gpu.info.backend,
        gpu.info.driver,
        gpu.info.driver_info
    );
    let mut h = Harness::new(gpu);
    let mut failures = Vec::new();
    eprintln!(
        "{:<30} {:<34} {:>10} | {:>14} | {:>13} | {:>14}",
        "case", "reference", "output", "patch max/mean", "ramp max/mean", "pixel max/mean"
    );
    for case in cases() {
        let d = h.run(case.file, case.bytes, case.bg);
        let checks = (case.checks)(&d);
        eprintln!(
            "{} — {} ({:?}, peak {:.2})",
            case.name, d.info.coding, d.raster.layout, d.content_peak
        );
        for check in &checks {
            for (out, got) in &d.outputs {
                let e = measure(got, &check.want, check.profiles);
                let tol = if out.ends_with(TILED) {
                    check.tol.at_least(ROUNDED)
                } else {
                    check.tol
                };
                let show = |s: Stat| {
                    if s.n == 0 {
                        "-".to_owned()
                    } else {
                        format!("{:.2}/{:.2}", s.max, s.mean())
                    }
                };
                let ok = e.patch.max <= tol.patch
                    && e.profile.max <= tol.profile
                    && e.pixel.max <= tol.pixel;
                eprintln!(
                    "  {:<30} {:<34} {:>10} | {:>14} | {:>13} | {:>14} {}",
                    "",
                    check.label,
                    out,
                    show(e.patch),
                    show(e.profile),
                    show(e.pixel),
                    if ok { "ok" } else { "FAIL" }
                );
                if !ok {
                    let w = |s: Stat| {
                        let (x, y, c, g, r) = s.worst;
                        format!(
                            "{:.2} at ({x}, {y}) {} got {g:.2} want {r:.2}",
                            s.max,
                            ["R", "G", "B"][c]
                        )
                    };
                    failures.push(format!(
                        "{} {} {}: patch {}; ramp {}; pixel {}; allowed {:?}",
                        case.name,
                        check.label,
                        out,
                        w(e.patch),
                        w(e.profile),
                        w(e.pixel),
                        tol
                    ));
                }
            }
        }
    }
    eprintln!("colour e2e: {:.2} s", t0.elapsed().as_secs_f64());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- fixtures made with external encoders ---------------------------------------------------

/// Rewrites tests/data/colour_e2e/{bt709-limited.avif, bt2020-pq.avif, xyb.jxl} with ffmpeg
/// (libaom-av1 lossless from raw Y'CbCr planes, so the coded values are known exactly; libjxl
/// at distance 0.5 with XYB). `cargo test -p specular-render --test colour_e2e -- --ignored`.
#[test]
#[ignore = "needs ffmpeg with libaom-av1 and libjxl; rewrites committed fixtures"]
fn regenerate_fixtures() {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("colour_e2e");
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::create_dir_all(data_dir()).unwrap();
    let ffmpeg = |args: &[&str]| {
        let st = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y"])
            .args(args)
            .status()
            .expect("ffmpeg");
        assert!(st.success(), "ffmpeg {args:?}");
    };
    let size = format!("{W}x{H}");
    let planar = |px: &[[u16; 3]], wide: bool| -> Vec<u8> {
        let mut out = Vec::new();
        for c in 0..3 {
            for p in px {
                if wide {
                    out.extend_from_slice(&p[c].to_le_bytes());
                } else {
                    out.push(p[c] as u8);
                }
            }
        }
        out
    };
    let avif = |name: &str, raw: Vec<u8>, pix: &str, space: &[&str]| {
        let yuv = tmp.join(format!("{name}.yuv"));
        let out = tmp.join(format!("{name}.avif"));
        std::fs::write(&yuv, raw).unwrap();
        // The colour description goes on the input (so the frames carry it) and on the
        // output (so the encoder writes it to the sequence header and the `colr` box).
        let mut args = vec![
            "-f",
            "rawvideo",
            "-pix_fmt",
            pix,
            "-s",
            &size,
            "-color_range",
            "tv",
        ];
        args.extend_from_slice(space);
        args.extend_from_slice(&[
            "-i",
            yuv.to_str().unwrap(),
            "-frames:v",
            "1",
            "-color_range",
            "tv",
        ]);
        args.extend_from_slice(space);
        args.extend_from_slice(&[
            "-c:v",
            "libaom-av1",
            "-aom-params",
            "lossless=1",
            "-still-picture",
            "1",
            "-f",
            "avif",
            out.to_str().unwrap(),
        ]);
        ffmpeg(&args);
        std::fs::copy(&out, data_dir().join(format!("{name}.avif"))).unwrap();
    };
    avif(
        "bt709-limited",
        planar(&bt709_codes(), false),
        "yuv444p",
        &[
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
        ],
    );
    avif(
        "bt2020-pq",
        planar(&pq_codes(), true),
        "yuv444p10le",
        &[
            "-colorspace",
            "bt2020nc",
            "-color_primaries",
            "bt2020",
            "-color_trc",
            "smpte2084",
        ],
    );
    let png = tmp.join("xyb-src.png");
    std::fs::write(
        &png,
        png8(
            &sdr8(),
            &PngTags {
                srgb: true,
                ..Default::default()
            },
        ),
    )
    .unwrap();
    let jxl = data_dir().join("xyb.jxl");
    ffmpeg(&[
        "-i",
        png.to_str().unwrap(),
        "-frames:v",
        "1",
        "-c:v",
        "libjxl",
        "-distance",
        "0.5",
        "-xyb",
        "1",
        "-f",
        "image2",
        jxl.to_str().unwrap(),
    ]);
    for n in ["bt709-limited.avif", "bt2020-pq.avif", "xyb.jxl"] {
        let len = std::fs::metadata(data_dir().join(n)).unwrap().len();
        eprintln!("{n}: {len} bytes");
    }
}

#[test]
fn layout_is_consistent() {
    // Patches are MCU-aligned (4:2:0 chroma flat inside), ramps cover their bands.
    assert!(matches!(cell(0, 0), Cell::Patch(0)));
    assert!(matches!(cell(127, 47), Cell::Patch(23)));
    assert!(matches!(cell(0, 48), Cell::HRamp(0, _)));
    assert!(matches!(cell(127, 111), Cell::VRamp(7, _)));
    // The PQ codes round-trip through the f64 reference, within 10-bit limited-range Y'CbCr
    // quantisation: half a PQ step is ~0.6 % of linear light, and R' and B' add the Cr and Cb
    // steps times 1.47 and 1.88, about 1.5 % in all; the BT.2020 → BT.709 matrix (off-diagonal
    // gains up to 0.59) then carries the largest channel's error into the others, so the bound
    // is relative to the colour's largest component.
    for (c, lin) in pq_codes().iter().zip(fixtures::hdr_palette().image()) {
        let e = fixtures::rgb_from_limited(*c, fixtures::BT2020, 10);
        let back = mul(&inverse(&BT709_TO_BT2020), e.map(|v| pq_decode(v) / 203.0));
        let peak = lin[0].max(lin[1]).max(lin[2]).max(0.05);
        for i in 0..3 {
            assert!(
                (back[i] - lin[i]).abs() <= 0.02 * peak,
                "{lin:?} → {back:?}"
            );
        }
    }
}
