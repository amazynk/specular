//! Differential tests against the `png` crate on generated images: every colour type, bit
//! depth, interlace mode and filter type, with and without tRNS, split IDATs, odd sizes.

mod common;

use common::*;
use specular_png::{DecodeOptions, Layout, Output, Parallelism, decode, decode_adam7_preview};

fn opts(output: Output) -> DecodeOptions<'static> {
    DecodeOptions {
        output,
        parallelism: Parallelism::Single,
        ..Default::default()
    }
}

fn rgba16_of(img: &specular_png::Image) -> Vec<u16> {
    assert_eq!(img.layout, Layout::Rgba16);
    img.planes[0]
        .data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect()
}

/// Any 16-bit image (`Auto16` output) as interleaved RGBA16.
fn expand16(img: &specular_png::Image) -> Vec<u16> {
    let v: Vec<u16> = img.planes[0]
        .data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect();
    match img.layout {
        Layout::Rgba16 => v,
        Layout::Gray16 => v.iter().flat_map(|&g| [g, g, g, 0xffff]).collect(),
        Layout::GrayAlpha16 => v
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        l => panic!("not 16-bit: {l:?}"),
    }
}

/// Grey images: `Auto16` gives the compact layout, the same samples as `Rgba16` at full size,
/// and the same downscaled levels (grey averaged as stored, alpha-weighted).
fn check_auto16(data: &[u8], what: &str) {
    let info = specular_png::read_info(data).unwrap();
    if !matches!(
        info.color_type,
        specular_png::ColorType::Gray | specular_png::ColorType::GrayAlpha
    ) {
        return;
    }
    for scale in [1u32, 2, 3, 8] {
        let o = |output| DecodeOptions {
            scale,
            ..opts(output)
        };
        let wide = decode(data, &o(Output::Rgba16)).unwrap();
        let auto = decode(data, &o(Output::Auto16)).unwrap();
        assert_eq!(auto.layout, info.auto_layout16(), "{what}");
        assert_eq!(
            expand16(&auto),
            rgba16_of(&wide),
            "{what}: Auto16 at 1/{scale}"
        );
    }
}

/// Decode `data` every way and compare with the oracle.
fn check(data: &[u8], what: &str) {
    let o = &oracle(data)[0];
    let want8: Vec<u8> = o.rgba16.iter().map(|&v| r8(v)).collect();

    let img = decode(data, &opts(Output::Rgba8)).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(!img.truncated, "{what}");
    assert_eq!(
        (img.width as usize, img.height as usize),
        (o.w, o.h),
        "{what}"
    );
    assert_eq!(img.planes[0].data, want8, "{what}: Rgba8");

    let auto = decode(data, &opts(Output::Auto8)).unwrap();
    assert_eq!(auto.to_rgba8(), want8, "{what}: Auto8 {:?}", auto.layout);

    let img16 = decode(data, &opts(Output::Rgba16)).unwrap();
    assert_eq!(rgba16_of(&img16), o.rgba16, "{what}: Rgba16");
    check_auto16(data, what);

    let threaded = decode(
        data,
        &DecodeOptions {
            parallelism: Parallelism::Threads,
            ..opts(Output::Auto8)
        },
    )
    .unwrap();
    assert_eq!(threaded, auto, "{what}: threaded");

    let checked = decode(
        data,
        &DecodeOptions {
            verify_checksums: true,
            ..opts(Output::Auto8)
        },
    )
    .unwrap();
    assert_eq!(checked, auto, "{what}: verified");
}

#[test]
fn all_formats() {
    let sizes = [(1, 1), (3, 5), (17, 9), (33, 40)];
    let filters = [
        Filters::Fixed(0),
        Filters::Fixed(1),
        Filters::Fixed(2),
        Filters::Fixed(3),
        Filters::Fixed(4),
        Filters::Mixed,
    ];
    let mut n = 0;
    for ct in [0u8, 2, 3, 4, 6] {
        for &depth in depths(ct) {
            for trns in [false, true] {
                if trns && (ct == 4 || ct == 6) {
                    continue;
                }
                for interlace in [false, true] {
                    for (fi, &f) in filters.iter().enumerate() {
                        for (si, &(w, h)) in sizes.iter().enumerate() {
                            let seed = (n as u64 + 1) * 7919;
                            let content = [Content::Noise, Content::Photo, Content::Screen][si % 3];
                            let s = make(w, h, ct, depth, content, seed);
                            let extra = Extra {
                                palette: (ct == 3).then(|| palette(depth, seed)),
                                trns: if trns { trns_for(&s, seed) } else { None },
                                idat_size: (fi % 2 == 1).then_some(7),
                                level: Some([0, 1, 6, 9][si]),
                            };
                            let data = encode(&s, interlace, f, &extra, seed);
                            check(
                                &data,
                                &format!(
                                    "ct {ct} depth {depth} trns {trns} interlace {interlace} {f:?} {w}x{h}"
                                ),
                            );
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(n > 1000);
}

/// Larger images: pipeline path, many windows of inflate output.
#[test]
fn large_images() {
    for (ct, depth, interlace) in [
        (2u8, 8u8, false),
        (6, 8, false),
        (6, 16, true),
        (0, 8, false),
        (3, 4, true),
    ] {
        let s = make(1200, 900, ct, depth, Content::Photo, 99);
        let extra = Extra {
            palette: (ct == 3).then(|| palette(depth, 3)),
            ..Default::default()
        };
        let data = encode(&s, interlace, Filters::Mixed, &extra, 5);
        check(
            &data,
            &format!("large ct {ct} depth {depth} interlace {interlace}"),
        );
        let auto = decode(&data, &DecodeOptions::default()).unwrap();
        assert_eq!(auto, decode(&data, &opts(Output::Auto8)).unwrap());
    }
}

#[test]
fn idot_parts() {
    for parts in [2, 3, 5] {
        let s = make(301, 257, 6, 8, Content::Screen, parts as u64);
        let data = encode_idot(&s, parts);
        check(&data, &format!("iDOT {parts}"));
        let info = specular_png::read_info(&data).unwrap();
        assert_eq!(info.idot.as_ref().map(|p| p.len()), Some(parts));
        let t = decode(
            &data,
            &DecodeOptions {
                parallelism: Parallelism::Threads,
                ..opts(Output::Rgba8)
            },
        )
        .unwrap();
        assert_eq!(t, decode(&data, &opts(Output::Rgba8)).unwrap());
    }
}

/// Reference box filter in linear light with premultiplied alpha.
fn reference_down(rgba: &[u8], w: usize, h: usize, s: usize) -> Vec<u8> {
    let lin = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let srgb = |l: f64| {
        let c = if l <= 0.0031308 {
            l * 12.92
        } else {
            1.055 * l.powf(1.0 / 2.4) - 0.055
        };
        (c * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let (ow, oh) = (w.div_ceil(s), h.div_ceil(s));
    let mut out = Vec::new();
    for oy in 0..oh {
        for ox in 0..ow {
            let mut acc = [0f64; 4];
            let mut n = 0.0;
            for y in oy * s..((oy + 1) * s).min(h) {
                for x in ox * s..((ox + 1) * s).min(w) {
                    let p = &rgba[(y * w + x) * 4..][..4];
                    let a = f64::from(p[3]) / 255.0;
                    for c in 0..3 {
                        acc[c] += lin(p[c]) * a;
                    }
                    acc[3] += a;
                    n += 1.0;
                }
            }
            for c in 0..3 {
                out.push(if acc[3] > 0.0 {
                    srgb(acc[c] / acc[3])
                } else {
                    0
                });
            }
            out.push((acc[3] / n * 255.0).round() as u8);
        }
    }
    out
}

#[test]
fn scaled() {
    for (ct, depth, interlace) in [
        (6u8, 8u8, false),
        (2, 8, true),
        (0, 8, false),
        (4, 16, false),
    ] {
        let s = make(103, 77, ct, depth, Content::Photo, 11);
        let data = encode(&s, interlace, Filters::Mixed, &Extra::default(), 3);
        let full = decode(&data, &opts(Output::Rgba8)).unwrap().planes[0]
            .data
            .clone();
        for sc in [2usize, 3, 4, 8] {
            let img = decode(
                &data,
                &DecodeOptions {
                    scale: sc as u32,
                    ..opts(Output::Rgba8)
                },
            )
            .unwrap();
            assert_eq!(
                (img.width, img.height),
                (103u32.div_ceil(sc as u32), 77u32.div_ceil(sc as u32))
            );
            let want = reference_down(&full, 103, 77, sc);
            for (i, (a, b)) in img.planes[0].data.iter().zip(&want).enumerate() {
                assert!(
                    a.abs_diff(*b) <= 1,
                    "ct {ct} scale {sc} byte {i}: {a} vs {b}"
                );
            }
            let auto = decode(
                &data,
                &DecodeOptions {
                    scale: sc as u32,
                    ..opts(Output::Auto8)
                },
            )
            .unwrap();
            for (a, b) in auto.to_rgba8().iter().zip(&img.planes[0].data) {
                assert!(a.abs_diff(*b) <= 1);
            }
        }
    }
}

#[test]
fn adam7_preview() {
    let s = make(61, 45, 2, 8, Content::Noise, 5);
    let data = encode(&s, true, Filters::Mixed, &Extra::default(), 3);
    let full = decode(&data, &opts(Output::Rgba8)).unwrap();
    for sc in [8u32, 4, 2] {
        let p = decode_adam7_preview(
            &data,
            &DecodeOptions {
                scale: sc,
                ..opts(Output::Rgba8)
            },
        )
        .unwrap();
        assert!(!p.truncated);
        assert_eq!(
            (p.width, p.height),
            (61u32.div_ceil(sc), 45u32.div_ceil(sc))
        );
        let sc = sc as usize;
        for y in 0..p.height as usize {
            for x in 0..p.width as usize {
                let a = &p.planes[0].data[(y * p.width as usize + x) * 4..][..4];
                let b = &full.planes[0].data[(y * sc * 61 + x * sc) * 4..][..4];
                assert_eq!(a, b, "scale {sc} at {x},{y}");
            }
        }
    }
}

#[test]
fn metadata() {
    let s = make(8, 8, 2, 8, Content::Photo, 1);
    let mut data = encode(&s, false, Filters::Fixed(0), &Extra::default(), 1);
    // Insert chunks after IHDR.
    let mut extra = Vec::new();
    chunk(&mut extra, b"gAMA", &45455u32.to_be_bytes());
    chunk(&mut extra, b"sRGB", &[0]);
    let icc = b"fake icc profile bytes".repeat(10);
    let mut iccp = b"name\0\0".to_vec();
    iccp.extend_from_slice(&zlib(&icc, 6));
    chunk(&mut extra, b"iCCP", &iccp);
    chunk(&mut extra, b"cICP", &[9, 16, 0, 1]);
    chunk(&mut extra, b"eXIf", b"MM\0\x2a\0\0\0\x08\0\0");
    data.splice(33..33, extra);
    let info = specular_png::read_info(&data).unwrap();
    assert_eq!(info.gamma, Some(45455));
    assert_eq!(info.srgb_intent, Some(0));
    assert_eq!(info.icc_profile.as_deref(), Some(&icc[..]));
    assert_eq!(info.icc_name.as_deref(), Some("name"));
    assert!(info.cicp.is_some());
    assert_eq!(info.exif(&data).map(|e| e.len()), Some(10));
    assert_eq!(
        decode(&data, &opts(Output::Rgba8)).unwrap().planes[0].data,
        {
            let o = &oracle(&data)[0];
            o.rgba16.iter().map(|&v| r8(v)).collect::<Vec<_>>()
        }
    );
}
