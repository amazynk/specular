//! End-to-end tests: sources written with the tree's encoders into a private folder under
//! `TMPDIR`, converted by [`run`], and read back through the registry and our own decoders.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use super::*;
use specular_core::{Orientation, Raster};
use std::fs;

const PROPHOTO: &[u8] = include_bytes!(
    "../../formats/specular-jxl/src/color/icc/test-profiles/prophoto-gamma18-rel.icc"
);

fn tmpdir(tag: &str) -> PathBuf {
    let base = std::env::var_os("TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let d = base.join(format!("specular-convert-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// A big-endian camera Exif with orientation `o`, a capture date and a GPS position.
fn camera_exif(o: u16) -> Vec<u8> {
    let mut t = exif::tests::sample_be();
    // The sample's Orientation is IFD0's third entry (value 6), inline at 8 + 2 + 2·12 + 8.
    let at = 8 + 2 + 2 * 12 + 8;
    t[at..at + 2].copy_from_slice(&o.to_be_bytes());
    t
}

const XMP: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:exif="http://ns.adobe.com/exif/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" exif:GPSLatitude="48,51.5N" dc:format="image/png"/></rdf:RDF></x:xmpmeta>"#;

/// An RGBA gradient PNG with half-transparent pixels, Exif (with GPS) and XMP.
fn rgba_png(path: &Path, w: u32, h: u32, opaque: bool) -> Vec<u8> {
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let a = if opaque || x % 4 != 0 { 255 } else { 128 };
            px.extend([(x * 255 / w) as u8, (y * 255 / h) as u8, 90, a]);
        }
    }
    let exif = camera_exif(1);
    let bytes = specular_png::encode_with(
        w,
        h,
        specular_png::ColorType::Rgba,
        &px,
        &specular_png::EncodeOptions {
            exif: Some(&exif),
            xmp: Some(XMP.as_bytes()),
            ..Default::default()
        },
    )
    .unwrap();
    fs::write(path, bytes).unwrap();
    px
}

fn rt() -> (Arc<Registry>, JobPool) {
    (
        Arc::new(Registry::new()),
        JobPool::background(2, "convert-test"),
    )
}

fn convert(inputs: Vec<PathBuf>, opts: Options) -> Report {
    convert_with(inputs, opts, &CancelToken::new())
}

fn convert_with(inputs: Vec<PathBuf>, opts: Options, cancel: &CancelToken) -> Report {
    let (reg, pool) = rt();
    let batch = Arc::new(Batch::new(inputs, opts).unwrap());
    run(batch, reg, &pool, 2, cancel, Arc::new(|_: &Progress| {}))
}

fn outputs(r: &Report) -> Vec<PathBuf> {
    r.results
        .iter()
        .map(|(src, o)| match o {
            Outcome::Converted { out, .. } => out.clone(),
            o => panic!("{}: {o:?}", src.display()),
        })
        .collect()
}

/// Decodes `path` through the registry to straight RGBA8 (16-bit sRGB samples rounded).
fn decode_rgba(path: &Path) -> (u32, u32, Vec<u8>, Raster) {
    let reg = Registry::new();
    let s = reg.open_path(path).unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    // Video-style YUV (lossy WebP): the CPU reference conversion.
    let r = match r.layout {
        PixelLayout::Yuv(_) => specular_core::yuv::to_rgb(&r).unwrap(),
        _ => r,
    };
    let (w, h) = (r.width as usize, r.height as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    let p = &r.planes;
    let at = |i: usize, x: usize, y: usize| {
        let pl = &p[i];
        let px = (x / pl.h_div.max(1) as usize).min(pl.width as usize - 1);
        let py = (y / pl.v_div.max(1) as usize).min(pl.height as usize - 1);
        pl.data[py * pl.stride + px]
    };
    let u16at = |x: usize, y: usize, k: usize, n: usize| {
        let o = y * p[0].stride + (x * n + k) * 2;
        let v = u16::from_ne_bytes([p[0].data[o], p[0].data[o + 1]]);
        ((v as u32 * 255 + 32767) / 65535) as u8
    };
    for y in 0..h {
        for x in 0..w {
            let px: [u8; 4] = match r.layout {
                PixelLayout::Gray => {
                    let g = at(0, x, y);
                    [g, g, g, 255]
                }
                PixelLayout::Rgb => [at(0, x, y), at(1, x, y), at(2, x, y), 255],
                PixelLayout::Rgba => {
                    let o = y * p[0].stride + x * 4;
                    p[0].data[o..o + 4].try_into().unwrap()
                }
                PixelLayout::YCbCr => {
                    let (yy, cb, cr) = (
                        at(0, x, y) as f32,
                        at(1, x, y) as f32 - 128.0,
                        at(2, x, y) as f32 - 128.0,
                    );
                    let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                    [
                        c(yy + 1.402 * cr),
                        c(yy - 0.344_136 * cb - 0.714_136 * cr),
                        c(yy + 1.772 * cb),
                        255,
                    ]
                }
                PixelLayout::Rgba16(_) => [
                    u16at(x, y, 0, 4),
                    u16at(x, y, 1, 4),
                    u16at(x, y, 2, 4),
                    u16at(x, y, 3, 4),
                ],
                PixelLayout::Gray16 => {
                    let g = u16at(x, y, 0, 1);
                    [g, g, g, 255]
                }
                PixelLayout::GrayAlpha16 => {
                    let g = u16at(x, y, 0, 2);
                    [g, g, g, u16at(x, y, 1, 2)]
                }
                l => panic!("layout {l:?}"),
            };
            out.extend(px);
        }
    }
    (w as u32, h as u32, out, r)
}

/// Every format, and the lossy modes of WebP and JPEG XL: (options, lossless).
fn variants() -> Vec<(Options, bool)> {
    let mut v: Vec<(Options, bool)> = Format::ALL
        .into_iter()
        .map(|f| {
            let o = Options {
                format: f,
                jpeg_quality: 95,
                jpeg_full_chroma: true,
                ..Options::default()
            };
            (o, !matches!(f, Format::Jpeg | Format::Avif))
        })
        .collect();
    v.push((
        Options {
            format: Format::Webp,
            webp_quality: Some(90),
            ..Options::default()
        },
        false,
    ));
    v.push((
        Options {
            format: Format::Jxl,
            jxl_quality: Some(90),
            ..Options::default()
        },
        false,
    ));
    v
}

/// PSNR in dB of two equally long sample sets.
fn psnr(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let se: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| (f64::from(*x) - f64::from(*y)).powi(2))
        .sum();
    let mse = se / a.len() as f64;
    if mse == 0.0 {
        return f64::INFINITY;
    }
    10.0 * (255.0 * 255.0 / mse).log10()
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

#[test]
fn every_format_round_trips_through_our_decoders() {
    let d = tmpdir("roundtrip");
    let src = d.join("grad.png");
    let px = rgba_png(&src, 64, 48, false);
    let opaque_src = d.join("flat.png");
    let opaque = rgba_png(&opaque_src, 64, 48, true);
    for (k, (opts, lossless)) in variants().into_iter().enumerate() {
        let f = opts.format;
        let sub = format!("converted-{k}");
        let opts = Options {
            output: OutputDir::Subfolder(sub.clone()),
            ..opts
        };
        let r = convert(vec![src.clone(), opaque_src.clone()], opts);
        let outs = outputs(&r);
        assert_eq!(outs[0], d.join(&sub).join(format!("grad.{}", f.ext())));
        let bytes = fs::read(&outs[0]).unwrap();
        // Metadata came along (all of it by default, GPS included).
        let exif = specular_decode::exif_tiff(&bytes).unwrap_or_else(|| panic!("{f:?}: no Exif"));
        assert!(specular_decode::exif_position(&exif).is_some(), "{f:?}");
        assert_eq!(specular_decode::exif_orientation(&exif), Some(1), "{f:?}");
        let (w, h, got, _) = decode_rgba(&outs[0]);
        assert_eq!((w, h), (64, 48), "{f:?}");
        if f == Format::Jpeg {
            // Flattened onto white, lossy.
            let (_, _, got_o, _) = decode_rgba(&outs[1]);
            assert!(
                max_diff(&got_o, &opaque) <= 6,
                "{f:?} {}",
                max_diff(&got_o, &opaque)
            );
            let i = 0; // x = 0 is half transparent: over white.
            // Composited in linear light, not by averaging sRGB codes.
            let white = |c: u8| {
                let lin = specular_core::hdr::srgb_to_linear(f32::from(c) / 255.0);
                i32::from(crate::pixels::encode8(0.5 * lin + 0.5))
            };
            for c in 0..3 {
                let (g, e) = (i32::from(got[i + c]), white(px[i + c]));
                assert!((g - e).abs() <= 8, "{c}: {g} vs {e} {:?}", &got[..4]);
            }
        } else {
            let (_, _, got_o, r) = decode_rgba(&outs[1]);
            if lossless {
                assert_eq!(max_diff(&got, &px), 0, "{f:?}");
                assert_eq!(max_diff(&got_o, &opaque), 0, "{f:?}");
            } else {
                // Lossy at quality 80–90 on a smooth gradient: close, alpha included.
                let (a, b) = (psnr(&got, &px), psnr(&got_o, &opaque));
                assert!(a > 34.0 && b > 34.0, "{f:?} {k}: {a:.1} / {b:.1} dB");
            }
            // A fully opaque source loses its alpha channel (some of our decoders hand
            // back RGBA regardless, so ask the file).
            let s = Registry::new().open_path(&outs[1]).unwrap();
            assert!(!s.info().has_alpha, "{f:?} {:?}", r.layout);
            let s = Registry::new().open_path(&outs[0]).unwrap();
            assert!(s.info().has_alpha, "{f:?}");
        }
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn metadata_modes_and_gps() {
    let d = tmpdir("meta");
    let src = d.join("m.png");
    rgba_png(&src, 16, 16, true);
    for (k, (base, _)) in variants().into_iter().enumerate() {
        let f = base.format;
        for (mode, gps, any) in [
            (MetaMode::All, true, true),
            (MetaMode::NoLocation, false, true),
            (MetaMode::Strip, false, false),
        ] {
            let opts = Options {
                metadata: mode,
                output: OutputDir::Folder(d.join(format!("{k}-{f:?}-{mode:?}"))),
                ..base.clone()
            };
            let out = outputs(&convert(vec![src.clone()], opts)).remove(0);
            let bytes = fs::read(&out).unwrap();
            // (A TIFF file is its own Exif structure: look for the capture date.)
            let exif = specular_decode::exif_tiff(&bytes)
                .filter(|e| specular_decode::exif_date_taken(e).is_some());
            assert_eq!(exif.is_some(), any, "{f:?} {mode:?}");
            if let Some(e) = &exif {
                assert_eq!(
                    specular_decode::exif_position(e).is_some(),
                    gps,
                    "{f:?} {mode:?}"
                );
                assert_eq!(
                    specular_decode::exif_date_taken(e).as_deref(),
                    Some("2023:07:01 10:20:30")
                );
            }
            // XMP: present unless stripped, its GPS only with everything kept.
            let text = String::from_utf8_lossy(&bytes);
            assert_eq!(text.contains("dc:format"), any, "{f:?} {mode:?}");
            assert_eq!(text.contains("GPSLatitude"), gps, "{f:?} {mode:?}");
            // And it is where readers look for it (AVIF: a `mime` item; JPEG isn't covered by
            // this locator).
            if f != Format::Jpeg {
                let found = specular_decode::embedded_metadata(&bytes).xmp.is_some();
                assert_eq!(found, any, "{f:?} {mode:?}");
            }
            // No GPS bytes anywhere when location is removed.
            if !gps {
                assert!(!text.contains("48,51.5N"), "{f:?} {mode:?}");
            }
        }
    }
    let _ = fs::remove_dir_all(&d);
}

/// Batch Convert carries the sidecar's keywords, private ones left out, unless metadata is
/// stripped.
#[test]
fn sidecar_keywords_go_into_conversions() {
    use specular_metadata::keywords as kw;
    let d = tmpdir("kw");
    let src = d.join("k.png");
    rgba_png(&src, 16, 16, true);
    let mut v = kw::Vocabulary::new();
    v.add("Secret");
    v.set_private("Secret", true);
    let keywords = ["Places|Lisbon".to_owned(), "Secret".to_owned()];
    fs::write(d.join("k.xmp"), kw::write_xmp(None, &keywords, Some(&v))).unwrap();
    for (mode, kept) in [(MetaMode::All, true), (MetaMode::Strip, false)] {
        let opts = Options {
            format: Format::Png,
            metadata: mode,
            output: OutputDir::Folder(d.join(format!("{mode:?}"))),
            ..Options::default()
        };
        let out = outputs(&convert(vec![src.clone()], opts)).remove(0);
        let bytes = fs::read(&out).unwrap();
        let xmp = specular_decode::embedded_metadata(&bytes).xmp;
        let Some(xmp) = xmp else {
            assert!(!kept, "{mode:?}: no XMP");
            continue;
        };
        let x = specular_metadata::xmp::parse(&xmp);
        let subject = x.get_all("dc:subject").unwrap_or_default();
        if kept {
            assert_eq!(subject, ["Lisbon", "Places"], "{mode:?}");
            assert_eq!(
                x.get_all("lr:hierarchicalSubject").unwrap(),
                ["Places|Lisbon"]
            );
        } else {
            assert!(subject.is_empty(), "{mode:?}: {subject:?}");
        }
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("Secret"), "{mode:?}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn resize_orientation_and_decode_scale() {
    let d = tmpdir("resize");
    // A 400×200 JPEG stored sideways (orientation 6): red block at the stored top-left,
    // which is the displayed top-right.
    let (w, h) = (400u32, 200u32);
    let mut rgb = Vec::new();
    for y in 0..h {
        for x in 0..w {
            rgb.extend(if x < 100 && y < 100 {
                [220, 20, 20]
            } else {
                [30, 60, 200]
            });
        }
    }
    let exif = camera_exif(6);
    let jpg = specular_jpeg::encode::encode(
        w,
        h,
        specular_jpeg::encode::Samples::Rgb(&rgb),
        &specular_jpeg::encode::EncodeOptions {
            quality: 95,
            exif: Some(&exif),
            ..Default::default()
        },
    )
    .unwrap();
    let src = d.join("side.jpg");
    fs::write(&src, jpg).unwrap();
    let opts = Options {
        format: Format::Png,
        resize: Resize::LongEdge(100),
        pattern: "{name}_{w}x{h}".into(),
        ..Options::default()
    };
    let out = outputs(&convert(vec![src.clone()], opts)).remove(0);
    assert_eq!(out.file_name().unwrap(), "side_50x100.png");
    let (ow, oh, px, _) = decode_rgba(&out);
    assert_eq!((ow, oh), (50, 100));
    let at = |x: u32, y: u32| &px[((y * ow + x) * 4) as usize..][..3];
    // Displayed top-right is red, top-left and bottom are blue.
    assert!(at(45, 5)[0] > 180 && at(45, 5)[2] < 60, "{:?}", at(45, 5));
    assert!(at(5, 5)[2] > 160, "{:?}", at(5, 5));
    assert!(at(45, 90)[2] > 160, "{:?}", at(45, 90));
    let bytes = fs::read(&out).unwrap();
    let e = specular_decode::exif_tiff(&bytes).unwrap();
    assert_eq!(specular_decode::exif_orientation(&e), Some(1));

    // Never upscaled unless asked; percentages and boxes.
    assert_eq!(
        output_size(400, 200, Resize::LongEdge(800), false),
        (400, 200)
    );
    assert_eq!(
        output_size(400, 200, Resize::LongEdge(800), true),
        (800, 400)
    );
    assert_eq!(
        output_size(400, 200, Resize::Percent(50), false),
        (200, 100)
    );
    assert_eq!(
        output_size(
            400,
            200,
            Resize::Fit {
                width: 100,
                height: 100
            },
            false
        ),
        (100, 50)
    );
    assert_eq!(
        output_size(
            4000,
            3000,
            Resize::Fit {
                width: 1920,
                height: 1080
            },
            false
        ),
        (1440, 1080)
    );
    assert_eq!(output_size(3, 1000, Resize::LongEdge(10), false), (1, 10));
    // The coarsest decoder level that still covers the target.
    assert_eq!(decode_scale(&[1, 2, 4, 8], (4000, 3000), (1000, 750)), 4);
    assert_eq!(decode_scale(&[1, 2, 4, 8], (4000, 3000), (1001, 750)), 2);
    assert_eq!(decode_scale(&[1], (4000, 3000), (10, 10)), 1);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn names_conflicts_and_never_overwriting() {
    let d = tmpdir("names");
    let a = d.join("a.png");
    let b_dir = d.join("sub");
    fs::create_dir_all(&b_dir).unwrap();
    let b = b_dir.join("a.png");
    rgba_png(&a, 8, 8, true);
    rgba_png(&b, 8, 8, true);
    let out_dir = d.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    fs::write(out_dir.join("a.png"), b"keep me").unwrap();

    // Skip: the existing file and the batch's second claim on the name are both skipped.
    let opts = Options {
        format: Format::Png,
        output: OutputDir::Folder(out_dir.clone()),
        conflict: Conflict::Skip,
        ..Options::default()
    };
    let r = convert(vec![a.clone(), b.clone()], opts.clone());
    assert_eq!(r.skipped(), 2, "{:?}", r.results);
    assert_eq!(fs::read(out_dir.join("a.png")).unwrap(), b"keep me");

    // Suffix: deterministic in batch order.
    let r = convert(
        vec![a.clone(), b.clone()],
        Options {
            conflict: Conflict::Suffix,
            ..opts.clone()
        },
    );
    assert_eq!(
        outputs(&r),
        [out_dir.join("a-2.png"), out_dir.join("a-3.png")]
    );
    assert_eq!(fs::read(out_dir.join("a.png")).unwrap(), b"keep me");

    // Counter, size and date tokens.
    let r = convert(
        vec![a.clone(), b.clone()],
        Options {
            pattern: "{date:%Y%m%d}-{n:03}-{w}x{h}".into(),
            format: Format::Jpeg,
            ..opts.clone()
        },
    );
    assert_eq!(
        outputs(&r),
        [
            out_dir.join("20230701-001-8x8.jpg"),
            out_dir.join("20230701-002-8x8.jpg")
        ]
    );

    // An output that would be its own source is refused, whatever the conflict mode.
    let src_before = fs::read(&a).unwrap();
    for conflict in [Conflict::Skip, Conflict::Suffix] {
        let r = convert(
            vec![a.clone()],
            Options {
                format: Format::Png,
                output: OutputDir::Subfolder(String::new()),
                conflict,
                ..Options::default()
            },
        );
        assert!(
            matches!(&r.results[0].1, Outcome::Failed(Failure::SameAsSource(_))),
            "{:?}",
            r.results
        );
    }
    // Also through another spelling of the same folder.
    let r = convert(
        vec![a.clone()],
        Options {
            format: Format::Png,
            output: OutputDir::Folder(d.join("sub").join("..")),
            ..Options::default()
        },
    );
    assert!(matches!(
        &r.results[0].1,
        Outcome::Failed(Failure::SameAsSource(_))
    ));
    assert_eq!(fs::read(&a).unwrap(), src_before);

    // Bad patterns are refused up front.
    assert!(
        Batch::new(
            vec![],
            Options {
                pattern: "{nope}".into(),
                ..Options::default()
            }
        )
        .is_err()
    );
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn failures_are_reported_and_cancel_stops() {
    let d = tmpdir("fail");
    let good = d.join("good.png");
    rgba_png(&good, 8, 8, true);
    let bad = d.join("bad.png");
    fs::write(&bad, b"not an image").unwrap();
    let missing = d.join("missing.png");
    let r = convert(
        vec![bad.clone(), good.clone(), missing.clone()],
        Options::default(),
    );
    assert_eq!(r.converted(), 1);
    assert_eq!(r.failed(), 2);
    assert_eq!(r.failures().count(), 2);
    assert!(!r.cancelled);

    let c = CancelToken::new();
    c.cancel();
    let r = convert_with(
        vec![good.clone()],
        Options {
            output: OutputDir::Subfolder("cancelled".into()),
            ..Options::default()
        },
        &c,
    );
    assert!(r.cancelled);
    assert!(matches!(r.results[0].1, Outcome::Cancelled));
    assert!(!d.join("cancelled").exists());

    // Progress reaches the total with an estimate once something is done.
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let (reg, pool) = rt();
    let inputs: Vec<PathBuf> = (0..4)
        .map(|i| {
            let p = d.join(format!("p{i}.png"));
            rgba_png(&p, 8, 8, true);
            p
        })
        .collect();
    let batch = Arc::new(
        Batch::new(
            inputs,
            Options {
                output: OutputDir::Subfolder("progress".into()),
                ..Options::default()
            },
        )
        .unwrap(),
    );
    let r = run(
        batch,
        reg,
        &pool,
        2,
        &CancelToken::new(),
        Arc::new(move |p: &Progress| s2.lock().unwrap().push(*p)),
    );
    assert_eq!(r.converted(), 4);
    let seen = seen.lock().unwrap();
    let last = seen.iter().max_by_key(|p| p.done).unwrap();
    assert_eq!((last.done, last.total), (4, 4));
    assert!(last.eta.is_some());
    let _ = fs::remove_dir_all(&d);
}

/// A truncated file converts to what decoded, as the viewer shows it (the rest grey), instead
/// of failing: review finding 12.
#[test]
fn truncated_source_gives_what_decoded() {
    let d = tmpdir("trunc");
    let src = d.join("whole.png");
    let px = rgba_png(&src, 64, 64, true);
    let bytes = fs::read(&src).unwrap();
    let cut = d.join("cut.png");
    // The last part of the image data and IEND are gone.
    fs::write(&cut, &bytes[..bytes.len() - 120]).unwrap();
    let r = convert(
        vec![cut],
        Options {
            format: Format::Png,
            output: OutputDir::Subfolder("out".into()),
            ..Options::default()
        },
    );
    assert_eq!(r.converted(), 1, "{:?}", r.results);
    let out = outputs(&r);
    let (w, h, got, _) = decode_rgba(&out[0]);
    assert_eq!((w, h), (64, 64));
    // The first rows came through unchanged.
    assert_eq!(&got[..64 * 4 * 4], &px[..64 * 4 * 4]);
    let _ = fs::remove_dir_all(&d);
}

/// `png` with a gAMA chunk (decoding exponent 1/`g`·100000) after IHDR.
fn with_gama(png: &[u8], g: u32) -> Vec<u8> {
    fn crc(b: &[u8]) -> u32 {
        let mut c = !0u32;
        for &x in b {
            c ^= u32::from(x);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let at = 8 + 25;
    let mut chunk = b"gAMA".to_vec();
    chunk.extend_from_slice(&g.to_be_bytes());
    let mut out = png[..at].to_vec();
    out.extend_from_slice(&4u32.to_be_bytes());
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&crc(&chunk).to_be_bytes());
    out.extend_from_slice(&png[at..]);
    out
}

/// A grey PNG's gAMA (an RGB matrix/shaper profile, as for any PNG) converts like an RGB
/// one's, as the viewer shows it: PngSuite's linear (gAMA 1.0) grey files exported as if
/// sRGB-coded while their RGB twins were converted (fidelity round 4).
#[test]
fn grey_png_gamma_converts_like_rgb() {
    let d = tmpdir("greygama");
    let grey = specular_png::encode_with(
        8,
        8,
        specular_png::ColorType::Gray,
        &[64u8; 64],
        &Default::default(),
    )
    .unwrap();
    let rgb = specular_png::encode_with(
        8,
        8,
        specular_png::ColorType::Rgb,
        &[64u8; 192],
        &Default::default(),
    )
    .unwrap();
    let (g, c) = (d.join("grey.png"), d.join("rgb.png"));
    fs::write(&g, with_gama(&grey, 100_000)).unwrap();
    fs::write(&c, with_gama(&rgb, 100_000)).unwrap();
    for color in [ColorMode::Srgb, ColorMode::Keep] {
        let out = outputs(&convert(
            vec![g.clone(), c.clone()],
            Options {
                format: Format::Png,
                color,
                output: OutputDir::Subfolder(format!("{color:?}")),
                ..Options::default()
            },
        ));
        let (_, _, grey_px, _) = decode_rgba(&out[0]);
        let (_, _, rgb_px, _) = decode_rgba(&out[1]);
        // Linear 64/255 is sRGB 137.
        assert!(
            grey_px[0].abs_diff(137) <= 1,
            "{color:?}: {:?}",
            &grey_px[..4]
        );
        if color == ColorMode::Srgb {
            assert_eq!(grey_px[..3], rgb_px[..3]);
        }
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn colour_keep_or_convert() {
    let d = tmpdir("colour");
    // Saturated ProPhoto green, tagged.
    let px: Vec<u8> = (0..16 * 16).flat_map(|_| [40u8, 200, 40]).collect();
    let png = specular_png::encode_with(
        16,
        16,
        specular_png::ColorType::Rgb,
        &px,
        &specular_png::EncodeOptions {
            icc: Some(PROPHOTO),
            ..Default::default()
        },
    )
    .unwrap();
    let src = d.join("pp.png");
    fs::write(&src, png).unwrap();

    // Keep: same values (AVIF within its loss), same profile (JPEG XL in its codestream).
    for f in [Format::Png, Format::Jxl, Format::Avif] {
        let out = outputs(&convert(
            vec![src.clone()],
            Options {
                format: f,
                output: OutputDir::Subfolder(format!("keep-{f:?}")),
                ..Options::default()
            },
        ))
        .remove(0);
        let (_, _, got, _) = decode_rgba(&out);
        if f == Format::Avif {
            assert!(max_diff(&got[..3], &[40, 200, 40]) <= 4, "{:?}", &got[..3]);
        } else {
            assert_eq!(&got[..3], &[40, 200, 40], "{f:?}");
        }
        let s = Registry::new().open_path(&out).unwrap();
        let icc = s.info().icc_profile.clone().expect("profile");
        if f == Format::Jxl {
            // libjxl stores ProPhoto as an enumerated colour encoding (primaries, gamma
            // 1.8), and the decoder synthesizes an equivalent profile from it.
            assert!(icc.windows(4).any(|w| w == b"rXYZ"), "{f:?}");
        } else {
            assert_eq!(icc.as_slice(), PROPHOTO, "{f:?}");
        }
    }

    // Convert: sRGB values, no profile; ProPhoto's green is outside sRGB, so it clips to a
    // purer green than the stored triple says.
    for f in [Format::Png, Format::Avif] {
        let out = outputs(&convert(
            vec![src.clone()],
            Options {
                format: f,
                color: ColorMode::Srgb,
                output: OutputDir::Subfolder(format!("srgb-{f:?}")),
                ..Options::default()
            },
        ))
        .remove(0);
        let (_, _, got, _) = decode_rgba(&out);
        assert!(
            got[0] < 20 && got[1] > 200 && got[2] < 20,
            "{f:?} {:?}",
            &got[..3]
        );
        let s = Registry::new().open_path(&out).unwrap();
        assert!(s.info().icc_profile.is_none(), "{f:?}");
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn sixteen_bit_and_hdr() {
    let d = tmpdir("deep");
    // 16-bit RGB gradient → 16-bit TIFF (Deflate + predictor) and JPEG XL, losslessly.
    let (w, h) = (40u32, 30u32);
    let mut be = Vec::new();
    let mut vals = Vec::new();
    for y in 0..h {
        for x in 0..w {
            for c in 0..3u32 {
                let v = ((x * 1601 + y * 977 + c * 12345) % 65536) as u16;
                vals.push(v);
                be.extend(v.to_be_bytes());
            }
        }
    }
    let png = specular_png::encode_with(
        w,
        h,
        specular_png::ColorType::Rgb,
        &be,
        &specular_png::EncodeOptions {
            sixteen_bit: true,
            ..Default::default()
        },
    )
    .unwrap();
    let src = d.join("deep.png");
    fs::write(&src, png).unwrap();
    for (f, deflate) in [
        (Format::Tiff, true),
        (Format::Tiff, false),
        (Format::Jxl, false),
        (Format::Png, false),
    ] {
        let out = outputs(&convert(
            vec![src.clone()],
            Options {
                format: f,
                tiff_deflate: deflate,
                output: OutputDir::Subfolder(format!("{f:?}-{deflate}")),
                ..Options::default()
            },
        ))
        .remove(0);
        let s = Registry::new().open_path(&out).unwrap();
        if f == Format::Jxl {
            // The file is 16-bit; our JXL decoder only keeps wide samples for HDR or
            // linear images, so compare at 8 bits.
            assert_eq!(s.info().bit_depth, 16, "{f:?}");
            let (_, _, got, _) = decode_rgba(&out);
            let worst = got
                .chunks_exact(4)
                .flat_map(|p| p[..3].to_vec())
                .zip(&vals)
                .map(|(a, b)| i32::from(a).abs_diff((u32::from(*b) * 255 + 32767) as i32 / 65535))
                .max()
                .unwrap();
            assert!(worst <= 1, "{f:?}: {worst}");
            continue;
        }
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let PixelLayout::Rgba16(_) = r.layout else {
            panic!("{f:?}: {:?}", r.layout)
        };
        let got: Vec<u16> = r.planes[0]
            .data
            .chunks_exact(2)
            .map(|c| u16::from_ne_bytes([c[0], c[1]]))
            .collect();
        let rgb: Vec<u16> = got.chunks_exact(4).flat_map(|p| p[..3].to_vec()).collect();
        let worst = rgb
            .iter()
            .zip(&vals)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        // The linear-light round trip is within a code or two of 16 bits.
        assert!(worst <= 2, "{f:?} {deflate}: {worst}");
    }

    // HDR: a PQ BT.2020 16-bit PNG stays PQ in PNG and is tone-mapped for JPEG.
    let pq = (0.75_f32 * 65535.0) as u16;
    let hdr: Vec<u8> = (0..8 * 8)
        .flat_map(|_| [pq, pq / 2, pq / 3])
        .flat_map(u16::to_be_bytes)
        .collect();
    let png = specular_png::encode_with(
        8,
        8,
        specular_png::ColorType::Rgb,
        &hdr,
        &specular_png::EncodeOptions {
            sixteen_bit: true,
            cicp: Some([9, 16, 0, 1]),
            ..Default::default()
        },
    )
    .unwrap();
    let src = d.join("hdr.png");
    fs::write(&src, png).unwrap();
    for f in [Format::Png, Format::Jxl] {
        let out = outputs(&convert(
            vec![src.clone()],
            Options {
                format: f,
                output: OutputDir::Subfolder(format!("hdr-keep-{f:?}")),
                ..Options::default()
            },
        ))
        .remove(0);
        let s = Registry::new().open_path(&out).unwrap();
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert!(r.layout.is_hdr(), "{f:?} {:?}", r.layout);
        if f == Format::Png {
            let v = u16::from_ne_bytes([r.planes[0].data[0], r.planes[0].data[1]]);
            assert!(v.abs_diff(pq) < 64, "{v} vs {pq}");
        }
    }
    for (f, hdr_mode) in [
        (Format::Jpeg, HdrMode::Keep),
        (Format::Png, HdrMode::ToneMap),
    ] {
        let out = outputs(&convert(
            vec![src.clone()],
            Options {
                format: f,
                hdr: hdr_mode,
                output: OutputDir::Subfolder(format!("hdr-{f:?}")),
                ..Options::default()
            },
        ))
        .remove(0);
        let (_, _, px, r) = decode_rgba(&out);
        assert!(!r.layout.is_hdr());
        assert!(px[0] > 200, "{f:?} {:?}", &px[..4]);
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn orientation_values_match_raster_rotation() {
    // Belt and braces for the pixel module's orientation against core's decomposition.
    for o in 1..=8u8 {
        let (q, _) = Orientation(o).to_rotation_flip();
        let (w, h, _) = pixels::orient(3, 2, 1, &[0u8; 6], Orientation(o));
        assert_eq!((w, h), if q % 2 == 1 { (2, 3) } else { (3, 2) });
    }
}

/// Encode speed of every format on a 24 MP RGB photo-like image (a smooth gradient with
/// noise). `cargo test --release -p specular-convert encode_speed -- --ignored --nocapture`;
/// `SPECULAR_ENCODE_THREADS=n` runs the encoders on a pool of n threads (default: all), and
/// `SPECULAR_ENCODE_ONLY=text` only the cases whose name contains it.
#[test]
#[ignore]
fn encode_speed() {
    let threads = std::env::var("SPECULAR_ENCODE_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let only = std::env::var("SPECULAR_ENCODE_ONLY").unwrap_or_default();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    println!("{} threads", pool.current_num_threads());
    let (w, h) = (6000u32, 4000u32);
    let mut s = 0x1234_5678u32;
    let mut px = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let n = (s & 15) as i32 - 8;
            let v = |c: u32| (c as i32 + n).clamp(0, 255) as u8;
            px.extend([
                v(x * 255 / w),
                v(y * 255 / h),
                v((x + y) * 128 / (w + h) + 60),
            ]);
        }
    }
    let px = pixels::Pixels {
        width: w,
        height: h,
        channels: 3,
        samples: pixels::Samples::Eight(px),
    };
    let cases = [
        ("JPEG q90 4:2:0", Format::Jpeg, 0u8),
        ("PNG effort 0", Format::Png, 0),
        ("PNG effort 1", Format::Png, 1),
        ("PNG effort 3", Format::Png, 3),
        ("PNG effort 6", Format::Png, 6),
        ("WebP lossless", Format::Webp, 0),
        ("WebP q85 effort 5", Format::Webp, 5),
        ("JPEG XL lossless effort 1", Format::Jxl, 1),
        ("JPEG XL lossless effort 3", Format::Jxl, 3),
        ("JPEG XL q90 effort 3", Format::Jxl, 103),
        ("AVIF q80 effort 1", Format::Avif, 1),
        ("TIFF deflate", Format::Tiff, 0),
    ];
    for (name, format, effort) in cases {
        if !name.contains(only.as_str()) {
            continue;
        }
        let lossy = effort > 100 || (format == Format::Webp && effort > 0);
        let effort = effort % 100;
        let o = Options {
            format,
            jpeg_quality: 90,
            png_effort: effort,
            webp_quality: lossy.then_some(85),
            webp_effort: effort.max(1),
            jxl_quality: lossy.then_some(90),
            jxl_effort: effort.max(1),
            avif_effort: effort.max(1),
            tiff_deflate: true,
            ..Options::default()
        };
        let t = std::time::Instant::now();
        let out = pool
            .install(|| encode::encode(&o, &px, &encode::Meta::default()))
            .unwrap();
        let dt = t.elapsed().as_secs_f64();
        println!(
            "{name}: {:.2} s, {:.1} MP/s, {:.1} MB",
            dt,
            24.0 / dt,
            out.len() as f64 / 1e6
        );
    }
}

/// A linear float gradient with values above 1 and a few tiny ones.
fn float_rgb(w: u32, h: u32, ch: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity((w * h) as usize * ch);
    for y in 0..h {
        for x in 0..w {
            let f = x as f32 / w as f32;
            v.extend([f * 2.5, (y as f32 / h as f32).powi(3), 1e-4 * f]);
            if ch == 4 {
                v.push(1.0 - 0.5 * f);
            }
        }
    }
    v
}

/// Decodes `bytes` (written to `name` in `dir`) through the registry: its float samples.
fn decode_float(dir: &Path, name: &str, bytes: &[u8]) -> (Raster, Vec<f32>) {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    let s = Registry::new().open_path(&p).unwrap();
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert!(
        matches!(r.layout, PixelLayout::Rgba16F(_)),
        "{:?}",
        r.layout
    );
    let pl = &r.planes[0];
    let mut v = Vec::new();
    for y in 0..r.height as usize {
        let row = &pl.data[y * pl.stride..][..r.width as usize * 8];
        v.extend(
            row.chunks_exact(2)
                .map(|b| half::f16::from_ne_bytes([b[0], b[1]]).to_f32()),
        );
    }
    (r, v)
}

/// Worst relative difference (absolute below 1e-3) between `a` (`ch` per pixel) and RGBA `b`.
fn float_err(a: &[f32], ch: usize, b: &[f32]) -> f32 {
    let mut worst = 0f32;
    for (pa, pb) in a.chunks_exact(ch).zip(b.chunks_exact(4)) {
        for c in 0..ch {
            let d = (pa[c] - pb[c]).abs() / pa[c].abs().max(1e-3);
            worst = worst.max(d);
        }
    }
    worst
}

fn icc_of(p: &Path) -> Option<Vec<u8>> {
    let s = Registry::new().open_path(p).ok()?;
    s.info()
        .icc_profile
        .as_ref()
        .map(|a| a.as_ref().clone())
        .or_else(|| s.embedded_icc().map(<[u8]>::to_vec))
}

/// 32-bit float TIFF, with and without Deflate and the floating-point predictor, re-decodes
/// through our TIFF decoder within half precision (the registry's float layout), its ICC
/// profile kept.
#[test]
fn float_tiff_round_trips_through_our_decoder() {
    let dir = tmpdir("ftiff");
    let icc = specular_color::profiles::matrix_para_icc(
        &specular_color::math::rgb_to_pcs(
            [0.64, 0.33],
            [0.30, 0.60],
            [0.15, 0.06],
            [0.3127, 0.3290],
        ),
        &[1.0],
        "linear sRGB",
    );
    for (ch, deflate) in [(3, true), (4, true), (3, false)] {
        let (w, h) = (97, 61);
        let v = float_rgb(w, h, ch);
        let meta = encode::Meta {
            icc: Some(&icc),
            ..Default::default()
        };
        let bytes = encode::tiff_float(w, h, ch, &v, &meta, deflate);
        let name = format!("f{ch}{deflate}.tif");
        let (r, got) = decode_float(&dir, &name, &bytes);
        assert_eq!((r.width, r.height), (w, h));
        let e = float_err(&v, ch, &got);
        assert!(e < 1e-3, "{ch} channels, deflate {deflate}: worst {e}");
        assert_eq!(icc_of(&dir.join(&name)).as_deref(), Some(&icc[..]));
    }
    let _ = fs::remove_dir_all(&dir);
}

/// EXR, half and float, ZIP and none, RGB and RGBA, re-decodes through specular-exr with
/// the chromaticities kept.
#[test]
fn exr_round_trips_through_our_decoder() {
    let dir = tmpdir("exr");
    let p3 = [0.68, 0.32, 0.265, 0.69, 0.15, 0.06, 0.3127, 0.329];
    for (ch, sample, zip) in [
        (3, exr::Sample::Half, true),
        (4, exr::Sample::Float, true),
        (3, exr::Sample::Float, false),
        (4, exr::Sample::Half, false),
    ] {
        // Odd height: a short last ZIP block.
        let (w, h) = (83, 37);
        let v = float_rgb(w, h, ch);
        let o = exr::ExrOptions {
            sample,
            zip,
            chromaticities: Some(p3),
        };
        let bytes = exr::encode(w, h, ch, &v, &o).unwrap();
        let img = specular_exr::decode(&bytes, &specular_exr::DecodeOptions::default())
            .unwrap_or_else(|e| panic!("{ch} {sample:?} {zip}: {e}"));
        assert_eq!((img.width, img.height), (w, h));
        assert_eq!(img.damaged_chunks, 0);
        assert_eq!(img.has_alpha, ch == 4);
        assert_eq!(img.compression == specular_exr::Compression::Zip, zip);
        let c = img.chromaticities.expect("chromaticities");
        assert!((c.red[0] - 0.68).abs() < 1e-6 && (c.white[1] - 0.329).abs() < 1e-6);
        let specular_exr::Pixels::F32(got) = &img.pixels else {
            panic!("f32 asked")
        };
        let e = float_err(&v, ch, got);
        let tol = if sample == exr::Sample::Half {
            1e-3
        } else {
            1e-7
        };
        assert!(e < tol, "{ch} {sample:?} zip {zip}: worst {e}");
        // And through the registry, as the viewer opens it.
        let (r, _) = decode_float(&dir, &format!("e{ch}{zip}.exr"), &bytes);
        assert_eq!((r.width, r.height), (w, h));
    }
    let _ = fs::remove_dir_all(&dir);
}

/// EXR attributes from a photo's Exif and XMP: owner, comments, capture date and offset,
/// position, and the packet, read back by our decoder.
#[test]
fn exr_carries_the_photos_metadata() {
    use crate::exif::{Entry, Exif};
    let rat = |v: &[(u32, u32)]| Entry {
        tag: 0,
        ty: 5,
        count: v.len() as u32,
        data: v
            .iter()
            .flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes()))
            .collect(),
    };
    let with_tag = |mut e: Entry, tag: u16| {
        e.tag = tag;
        e
    };
    let exif = Exif {
        ifd0: vec![
            Entry::ascii(0x010E, "Harbour at dusk"),
            Entry::ascii(0x013B, "Ann Lee"),
            Entry::ascii(0x8298, "\u{a9} Ann Lee"),
        ],
        exif: vec![
            Entry::ascii(0x9003, "2026:05:01 19:30:00"),
            Entry::ascii(0x9011, "+02:00"),
        ],
        gps: vec![
            Entry::ascii(0x0001, "N"),
            with_tag(rat(&[(38, 1), (42, 1), (3000, 100)]), 0x0002),
            Entry::ascii(0x0003, "W"),
            with_tag(rat(&[(9, 1), (8, 1), (0, 1)]), 0x0004),
            Entry::undefined(0x0005, &[0]),
            with_tag(rat(&[(125, 10)]), 0x0006),
        ],
    };
    let xmp = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>";
    let meta = exr::ExrMeta::from_exif(Some(&exif), Some(xmp));
    assert_eq!(meta.owner.as_deref(), Some("\u{a9} Ann Lee"));
    assert_eq!(meta.utc_offset, Some(-7200.0));
    let (w, h) = (8, 4);
    let v = float_rgb(w, h, 3);
    let o = exr::ExrOptions {
        sample: exr::Sample::Half,
        zip: true,
        chromaticities: None,
    };
    let bytes = exr::encode_with(w, h, 3, &v, &o, &meta).unwrap();
    let info = specular_exr::read_info(&bytes).unwrap();
    let part = &info.parts[0];
    let string = |n: &str| String::from_utf8(part.attribute(&bytes, n).unwrap().to_vec()).unwrap();
    let float = |n: &str| {
        let b = part.attribute(&bytes, n).unwrap();
        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
    };
    assert_eq!(string("owner"), "\u{a9} Ann Lee");
    assert_eq!(string("comments"), "Harbour at dusk");
    assert_eq!(string("capDate"), "2026:05:01 19:30:00");
    assert_eq!(float("utcOffset"), -7200.0);
    assert!(
        (float("latitude") - 38.7083).abs() < 1e-3,
        "{}",
        float("latitude")
    );
    assert!(
        (float("longitude") + 9.1333).abs() < 1e-3,
        "{}",
        float("longitude")
    );
    assert_eq!(float("altitude"), 12.5);
    assert_eq!(part.attribute(&bytes, "xmp").unwrap(), xmp);
    // The pixels still decode.
    let img = specular_exr::decode(&bytes, &specular_exr::DecodeOptions::default()).unwrap();
    assert_eq!((img.width, img.height), (w, h));
    // Nothing given: nothing written.
    let plain = exr::encode(w, h, 3, &v, &o).unwrap();
    assert!(
        specular_exr::read_info(&plain).unwrap().parts[0]
            .attribute(&plain, "owner")
            .is_none()
    );
}

/// PQ and HLG AVIF: the AV1 sequence headers (the `av1C` property's and the bitstream's)
/// say BT.2020 and the real transfer, as the `nclx` box does; the file still decodes.
#[test]
fn hdr_avif_sequence_header_says_its_colour() {
    let (w, h) = (16u32, 8u32);
    let v: Vec<u16> = (0..w * h)
        .flat_map(|i| [(i * 400) as u16, 20_000, 9_000])
        .collect();
    let px = crate::pixels::Pixels {
        width: w,
        height: h,
        channels: 3,
        samples: crate::pixels::Samples::Sixteen(v),
    };
    for transfer in [16u8, 18] {
        let opts = Options {
            format: Format::Avif,
            ..Options::default()
        };
        let meta = crate::encode::Meta {
            cicp: Some([9, transfer, 0, 1]),
            ..Default::default()
        };
        let file = crate::encode::encode(&opts, &px, &meta).unwrap();
        // The av1C property's config OBUs, and the item's bitstream (the mdat payload).
        // av1C may carry no config OBUs (ravif writes none); any it has agree.
        let at = file.windows(4).position(|x| x == b"av1C").unwrap();
        let len = u32::from_be_bytes(file[at - 4..at].try_into().unwrap()) as usize;
        let obus = &file[at + 8..at - 4 + len];
        if !obus.is_empty() {
            assert_eq!(
                crate::av1_seq::colour(obus),
                Some((9, transfer, 9)),
                "av1C, transfer {transfer}"
            );
        }
        let md = file.windows(4).position(|x| x == b"mdat").unwrap();
        assert_eq!(
            crate::av1_seq::colour(&file[md + 4..]),
            Some((9, transfer, 9)),
            "bitstream, transfer {transfer}"
        );
        let d = tmpdir(&format!("avif-seq-{transfer}"));
        let p = d.join("hdr.avif");
        fs::write(&p, &file).unwrap();
        let s = Registry::new().open_path(&p).unwrap();
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert!(r.layout.is_hdr(), "{:?}", r.layout);
        let _ = fs::remove_dir_all(&d);
    }
}

/// A sequence header without a colour description is left alone; a described one is set.
#[test]
fn av1_sequence_header_colour_fields() {
    // Reduced still-picture header, profile 0: level 8, 16x16 (4-bit widths of 15), no
    // superres/cdef/restoration, 8-bit, not mono, described: 1/13/6.
    fn header(described: bool) -> Vec<u8> {
        let mut bits: Vec<u8> = Vec::new();
        let mut put = |n: u32, v: u64| {
            for i in (0..n).rev() {
                bits.push(((v >> i) & 1) as u8);
            }
        };
        put(3, 0); // profile
        put(1, 1); // still
        put(1, 1); // reduced
        put(5, 8); // level
        put(4, 3); // width bits - 1
        put(4, 3);
        put(4, 15); // width - 1
        put(4, 15);
        put(3, 0);
        put(3, 0);
        put(1, 0); // high bitdepth
        put(1, 0); // mono
        put(1, u64::from(described));
        if described {
            put(8, 1);
            put(8, 13);
            put(8, 6);
        }
        put(1, 1); // colour range
        bits.resize(bits.len().div_ceil(8) * 8, 0);
        let payload: Vec<u8> = bits
            .chunks(8)
            .map(|c| c.iter().fold(0, |a, b| (a << 1) | b))
            .collect();
        let mut obu = vec![0b0000_1010, payload.len() as u8];
        obu.extend(payload);
        obu
    }
    let mut d = header(true);
    assert_eq!(crate::av1_seq::colour(&d), Some((1, 13, 6)));
    assert_eq!(crate::av1_seq::set_colour(&mut d, 9, 16), 1);
    assert_eq!(crate::av1_seq::colour(&d), Some((9, 16, 6)));
    let mut d = header(false);
    let before = d.clone();
    assert_eq!(crate::av1_seq::set_colour(&mut d, 9, 16), 0);
    assert_eq!(d, before);
}

/// The TIFF and 16-bit PNG writers at 12 MP, and the time of each. Uncompressed TIFFs are
/// the bytes they always were (FNV-1a, taken before the writers stopped copying the
/// picture); the compressed ones matched too, but depend on the zlib version, so are not
/// pinned.
#[test]
fn large_tiff_and_png_files_are_unchanged() {
    let (w, h) = (4000u32, 3000u32);
    let n = (w * h * 3) as usize;
    let mut r = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        r ^= r << 13;
        r ^= r >> 7;
        r ^= r << 17;
        r
    };
    // Smooth gradients with a little noise: compressible as photographs are.
    let v16: Vec<u16> = (0..n)
        .map(|i| {
            let p = i / 3;
            let (x, y) = ((p % w as usize) as u32, (p / w as usize) as u32);
            ((x * 13 + y * 7 + (i % 3) as u32 * 5000) as u16).wrapping_add((next() & 63) as u16)
        })
        .collect();
    let v8: Vec<u8> = v16.iter().map(|&s| (s >> 8) as u8).collect();
    let px = |s: crate::pixels::Samples| crate::pixels::Pixels {
        width: w,
        height: h,
        channels: 3,
        samples: s,
    };
    let (p8, p16) = (
        px(crate::pixels::Samples::Eight(v8)),
        px(crate::pixels::Samples::Sixteen(v16)),
    );
    let fnv = |b: &[u8]| {
        b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| {
            (h ^ u64::from(x)).wrapping_mul(0x0100_0000_01b3)
        })
    };
    let meta = crate::encode::Meta::default();
    let png = Options {
        format: Format::Png,
        ..Options::default()
    };
    type Case<'a> = (&'a str, &'a dyn Fn() -> Vec<u8>, u64);
    let cases: [Case; 5] = [
        (
            "tiff 8",
            &|| crate::encode::tiff(&p8, &meta, false),
            0xd9e9_ee26_bd17_a3df,
        ),
        (
            "tiff 8 deflate",
            &|| crate::encode::tiff(&p8, &meta, true),
            0,
        ),
        (
            "tiff 16",
            &|| crate::encode::tiff(&p16, &meta, false),
            0x341d_a733_343c_69fe,
        ),
        (
            "tiff 16 deflate",
            &|| crate::encode::tiff(&p16, &meta, true),
            0,
        ),
        (
            "png 16",
            &|| crate::encode::encode(&png, &p16, &meta).unwrap(),
            0,
        ),
    ];
    for (name, f, want) in cases {
        let mut best = std::time::Duration::MAX;
        let mut hash = 0;
        for _ in 0..3 {
            let t0 = std::time::Instant::now();
            let file = f();
            best = best.min(t0.elapsed());
            hash = fnv(&file);
        }
        eprintln!("{name}: {best:.1?} (best of 3), fnv {hash:#018x}");
        if want != 0 {
            assert_eq!(hash, want, "{name}");
        }
    }
}
