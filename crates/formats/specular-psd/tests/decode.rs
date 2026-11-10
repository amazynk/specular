mod common;

use common::*;
use specular_psd::*;

const SEQ: Options = Options { parallel: false };

fn rgb_planes(w: u32, h: u32, bps: usize, n: u32) -> Vec<Vec<u8>> {
    (0..n).map(|c| pattern(w, h, c + 1, bps)).collect()
}

fn decode_both(file: &[u8]) -> (Psd, Image) {
    let psd = parse(file).unwrap();
    let a = psd.decode_rgba8(file, &Options::default()).unwrap();
    let b = psd.decode_rgba8(file, &SEQ).unwrap();
    assert_eq!(a.data, b.data, "parallel and sequential differ");
    (psd, a)
}

#[test]
fn rgb8_all_compressions_vs_source_and_oracle() {
    let (w, h) = (67, 41);
    for channels in [3u32, 4] {
        let planes = rgb_planes(w, h, 1, channels);
        for comp in 0..4u16 {
            let mut s = Spec::new(w, h, 8, 3, planes.clone());
            s.compression = comp;
            let f = write(&s);
            let (psd, img) = decode_both(&f);
            assert_eq!(psd.info().compression.map(|c| c as u16), Some(comp));
            // No layer section and no alpha-name resource: the 4th channel is alpha.
            assert_eq!(psd.info().has_alpha, channels == 4);
            assert_eq!(img.format, PixelFormat::Rgba8);
            assert!(!img.truncated);
            for (i, px) in img.data.chunks_exact(4).enumerate() {
                let a = if channels == 4 { planes[3][i] } else { 255 };
                assert_eq!(
                    px,
                    &[planes[0][i], planes[1][i], planes[2][i], a],
                    "comp {comp} px {i}"
                );
            }
            // Differential: the `psd` crate (raw and RLE only, 8-bit RGB).
            if comp < 2 {
                let o = psd::Psd::from_bytes(&f).unwrap();
                let rgba = o.rgba();
                assert_eq!(rgba.len(), img.data.len());
                for (i, (a, b)) in rgba.chunks(4).zip(img.data.chunks(4)).enumerate() {
                    assert_eq!(a[..3], b[..3], "oracle px {i} comp {comp}");
                    if channels == 4 {
                        assert_eq!(a[3], b[3]);
                    }
                }
            }
            // Planes give the stored bytes back.
            let p = psd.decode_planes(&f, &Options::default()).unwrap();
            match &p.samples {
                Samples::U8(v) => assert_eq!(v, &planes.concat()),
                _ => panic!(),
            }
            assert_eq!(p.to_rgba8().data, img.data);
        }
    }
}

#[test]
fn rgb16_planes_and_preview() {
    let (w, h) = (33, 17);
    let planes = rgb_planes(w, h, 2, 3);
    for comp in 0..4u16 {
        let mut s = Spec::new(w, h, 16, 3, planes.clone());
        s.compression = comp;
        let f = write(&s);
        let (psd, img) = decode_both(&f);
        let p = psd.decode_planes(&f, &SEQ).unwrap();
        let Samples::U16(v) = &p.samples else {
            panic!()
        };
        let want: Vec<u16> = planes.iter().flat_map(|p| be_u16s(p)).collect();
        assert_eq!(v, &want, "comp {comp}");
        for (i, px) in img.data.chunks(4).enumerate() {
            for c in 0..3 {
                let exp = (want[c * (w * h) as usize + i] as f64 / 257.0).round() as i32;
                assert!((px[c] as i32 - exp).abs() <= 1);
            }
        }
        assert_eq!(p.to_rgba8().data, img.data);
    }
}

#[test]
fn float32_gray_and_rgb() {
    let (w, h) = (19, 5);
    let vals: Vec<f32> = (0..w * h).map(|i| i as f32 / (w * h) as f32).collect();
    for (mode, n) in [(1u16, 1), (3, 3)] {
        for comp in [0u16, 1, 2, 3] {
            let planes: Vec<Vec<u8>> = (0..n).map(|_| f32_plane(&vals)).collect();
            let mut s = Spec::new(w, h, 32, mode, planes);
            s.compression = comp;
            let f = write(&s);
            let (psd, img) = decode_both(&f);
            let p = psd.decode_planes(&f, &SEQ).unwrap();
            let Samples::F32(v) = &p.samples else {
                panic!()
            };
            assert_eq!(&v[..vals.len()], &vals[..], "mode {mode} comp {comp}");
            let stride = if mode == 1 { 1 } else { 4 };
            assert_eq!(img.format.channels(), stride);
            for (i, &x) in vals.iter().enumerate() {
                assert_eq!(img.data[i * stride], linear_to_srgb8(x));
            }
        }
    }
}

#[test]
fn gray_bitmap_duotone_multichannel() {
    let (w, h) = (21, 9);
    let g = pattern(w, h, 3, 1);
    for mode in [1u16, 8] {
        let f = write(&Spec::new(w, h, 8, mode, vec![g.clone()]));
        let (_, img) = decode_both(&f);
        assert_eq!(img.format, PixelFormat::Gray8);
        assert_eq!(img.data, g);
    }
    // Bitmap: 1 = black.
    let rb = (w as usize).div_ceil(8);
    let bits: Vec<u8> = (0..rb * h as usize).map(|i| (i * 37) as u8).collect();
    for comp in [0u16, 1] {
        let mut s = Spec::new(w, h, 1, 0, vec![bits.clone()]);
        s.compression = comp;
        let f = write(&s);
        let (_, img) = decode_both(&f);
        assert_eq!(img.format, PixelFormat::Gray8);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let set = bits[y * rb + x / 8] & (0x80 >> (x % 8)) != 0;
                assert_eq!(img.data[y * w as usize + x], if set { 0 } else { 255 });
            }
        }
    }
    // Multichannel: first three channels as RGB, the 4th ignored.
    let planes = rgb_planes(w, h, 1, 4);
    let mut s = Spec::new(w, h, 8, 7, planes.clone());
    s.layer_count = 0;
    s.resources.push((1006, vec![0]));
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    assert!(!psd.info().has_alpha);
    assert_eq!(
        img.data[4..8],
        [planes[0][1], planes[1][1], planes[2][1], 255]
    );
}

#[test]
fn indexed_with_transparency() {
    let (w, h) = (16, 4);
    let idx: Vec<u8> = (0..w * h).map(|i| (i * 7) as u8).collect();
    let mut s = Spec::new(w, h, 8, 2, vec![idx.clone()]);
    s.color_data = (0..768).map(|i| (i * 3 % 256) as u8).collect();
    s.resources.push((1047, vec![0, 14]));
    s.compression = 1;
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    let pal = psd.info().palette.as_ref().unwrap();
    for (i, &k) in idx.iter().enumerate() {
        let [r, g, b] = pal[k as usize];
        let a = if k == 14 { 0 } else { 255 };
        assert_eq!(img.data[i * 4..i * 4 + 4], [r, g, b, a]);
    }
}

#[test]
fn cmyk_and_lab_naive() {
    let (w, h) = (2, 1);
    // CMYK stored inverted: 255 = no ink. Pixel 0: no ink → white. Pixel 1: full K → black.
    let s = Spec::new(
        w,
        h,
        8,
        4,
        vec![vec![255, 255], vec![255, 255], vec![255, 128], vec![255, 0]],
    );
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    assert_eq!(psd.info().mode, ColorMode::Cmyk);
    assert_eq!(img.data, [255, 255, 255, 255, 0, 0, 0, 255]);
    let p = psd.decode_planes(&f, &SEQ).unwrap();
    assert_eq!(p.color_channels, 4);
    // Lab: L=100 a=b=0 → white; L=0 → black.
    let s = Spec::new(
        w,
        h,
        8,
        9,
        vec![vec![255, 0], vec![128, 128], vec![128, 128]],
    );
    let (_, img) = decode_both(&write(&s));
    assert!(
        img.data[..3].iter().all(|&v| v >= 254),
        "{:?}",
        &img.data[..4]
    );
    assert!(img.data[4..7].iter().all(|&v| v <= 1));
    // 16-bit Lab mid grey.
    let s = Spec::new(
        1,
        1,
        16,
        9,
        vec![vec![0x80, 0], vec![0x80, 0], vec![0x80, 0]],
    );
    let (_, img) = decode_both(&write(&s));
    let [r, g, b] = lab_to_srgb8(50.2, 0.0, 0.0);
    assert!((img.data[0] as i32 - r as i32).abs() <= 2 && g == b);
}

#[test]
fn psb_rle_and_layers() {
    let (w, h) = (40, 12);
    let planes = rgb_planes(w, h, 1, 4);
    let mut s = Spec::new(w, h, 8, 3, planes.clone());
    s.psb = true;
    s.compression = 1;
    s.layer_count = -3;
    s.layers = vec![
        LayerSpec {
            name: "</Layer group>".into(),
            bounds: [0, 0, 0, 0],
            opacity: 255,
            hidden: false,
            section: 3,
            ..Default::default()
        },
        LayerSpec {
            name: "Ünïcode layer ✓".into(),
            bounds: [1, 2, 11, 30],
            opacity: 128,
            hidden: true,
            section: 0,
            ..Default::default()
        },
        LayerSpec {
            name: "Group".into(),
            bounds: [0, 0, 0, 0],
            opacity: 255,
            hidden: false,
            section: 1,
            ..Default::default()
        },
    ];
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    let info = psd.info();
    assert_eq!(info.version, Version::Psb);
    assert!(info.has_alpha);
    assert_eq!(info.layer_count, 3);
    let l = psd.layers();
    assert_eq!(l[0].kind, LayerKind::GroupEnd);
    assert_eq!(l[1].name, "Ünïcode layer ✓");
    assert_eq!(
        (l[1].top, l[1].left, l[1].bottom, l[1].right),
        (1, 2, 11, 30)
    );
    assert_eq!(l[1].opacity, 128);
    assert!(!l[1].visible);
    assert_eq!(l[2].kind, LayerKind::GroupStart { open: true });
    assert_eq!(img.data[3], planes[3][0]);
}

#[test]
fn lr16_layers_and_alpha() {
    let (w, h) = (8, 8);
    let planes = rgb_planes(w, h, 2, 4);
    let mut s = Spec::new(w, h, 16, 3, planes);
    s.layer_count = -1;
    s.lr16 = true;
    s.layers = vec![LayerSpec {
        name: "deep".into(),
        bounds: [0, 0, 8, 8],
        opacity: 255,
        hidden: false,
        section: 0,
        ..Default::default()
    }];
    let f = write(&s);
    let psd = parse(&f).unwrap();
    assert!(psd.info().has_alpha);
    assert_eq!(psd.layers()[0].name, "deep");
    let p = psd.decode_planes(&f, &SEQ).unwrap();
    assert_eq!(p.plane_count(), 4);
}

#[test]
fn alpha_channel_is_not_transparency_with_layers() {
    // A layered file with a positive layer count: the extra channel is a saved selection.
    let planes = rgb_planes(4, 4, 1, 4);
    let mut s = Spec::new(4, 4, 8, 3, planes);
    s.layer_count = 1;
    s.layers = vec![LayerSpec {
        name: "bg".into(),
        bounds: [0, 0, 4, 4],
        opacity: 255,
        hidden: false,
        section: 0,
        ..Default::default()
    }];
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    assert!(!psd.info().has_alpha);
    assert!(img.data.chunks(4).all(|p| p[3] == 255));
}

#[test]
fn photoshop_white_matte_is_removed() {
    // Colour 50 % red over transparent, flattened on white: (255, 128, 128) @ a = 128.
    let mut s = Spec::new(1, 1, 8, 3, vec![vec![255], vec![127], vec![127], vec![128]]);
    s.resources
        .push((1057, version_info(true, "Adobe Photoshop")));
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    assert!(psd.info().matted_on_white);
    assert_eq!(img.data, [255, 0, 0, 128]);
}

/// Fidelity round 5 (psd-tools' `transparentbg.psd`, 32-bit): the merged image's
/// transparency signalled by a "saving merged transparency" block (`Mt16`/`Mt32`/`Mtrn`)
/// next to a positive layer count was read as an alpha channel, so the transparent
/// background showed opaque.
#[test]
fn merged_transparency_block_marks_alpha() {
    let (w, h) = (4, 4);
    let mut s = Spec::new(w, h, 16, 3, rgb_planes(w, h, 2, 4));
    s.layer_count = 1;
    s.lr16 = true;
    s.layers = vec![LayerSpec {
        name: "deep".into(),
        bounds: [0, 0, 4, 4],
        opacity: 255,
        ..Default::default()
    }];
    let f = write(&s);
    assert!(
        !parse(&f).unwrap().info().has_alpha,
        "an alpha channel only"
    );
    s.blocks.push((*b"Mt16", Vec::new()));
    let f = write(&s);
    let psd = parse(&f).unwrap();
    assert!(psd.info().has_alpha);
    assert_eq!(psd.layers()[0].name, "deep");
}

/// Fidelity round 5 (psd-tools' `broken-groups.psd`, rewritten without its version-info
/// resource): with no writer named, the white matte is undone when the samples show one
/// (every colour at least 255 − alpha), and left alone when they do not.
#[test]
fn white_matte_found_in_the_samples_without_a_writer() {
    let file = |r: u8| {
        write(&Spec::new(
            2,
            1,
            8,
            3,
            vec![vec![255, r], vec![127, 0], vec![127, 0], vec![128, 255]],
        ))
    };
    let f = file(9);
    let (psd, img) = decode_both(&f);
    assert!(psd.info().matte_unknown && !psd.info().matted_on_white);
    assert_eq!(img.data, [255, 0, 0, 128, 9, 0, 0, 255]);
    // A dark pixel under partial transparency: not matted, kept as stored.
    let f = write(&Spec::new(
        2,
        1,
        8,
        3,
        vec![vec![255, 10], vec![127, 0], vec![127, 0], vec![128, 100]],
    ));
    let (_, img) = decode_both(&f);
    assert_eq!(img.data, [255, 127, 127, 128, 10, 0, 0, 100]);
}

/// Fidelity round 5 (psd-tools' `transparentbg.psd`): a 32-bit composite is matted onto
/// white in linear light, so the matte comes off before the sRGB curve, not after it.
#[test]
fn float32_white_matte_is_removed_in_linear_light() {
    // Linear 0.6 at alpha 0.5, matted: 0.6 · 0.5 + 0.5 = 0.8.
    let mut s = Spec::new(
        1,
        1,
        32,
        3,
        vec![
            f32_plane(&[0.8]),
            f32_plane(&[0.8]),
            f32_plane(&[1.0]),
            f32_plane(&[0.5]),
        ],
    );
    s.resources
        .push((1057, version_info(true, "Adobe Photoshop")));
    let f = write(&s);
    let (psd, img) = decode_both(&f);
    assert!(psd.info().matted_on_white);
    // sRGB of linear 0.6 is code 203, of 1.0 is 255 (un-matting the 8-bit codes gave 207).
    assert_eq!(img.data, [203, 203, 255, 128]);
}

#[test]
fn resources() {
    let mut s = Spec::new(4, 4, 8, 1, vec![vec![9; 16]]);
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9, 0x00];
    let mut thumb = Vec::new();
    for v in [1u32, 160, 120, 480, 57600, jpeg.len() as u32] {
        thumb.extend(v.to_be_bytes());
    }
    thumb.extend([0, 24, 0, 1]);
    thumb.extend(&jpeg);
    let mut res = Vec::new();
    res.extend((300u32 << 16).to_be_bytes());
    res.extend([0, 1, 0, 1]);
    res.extend((150u32 << 16).to_be_bytes());
    res.extend([0, 2, 0, 1]);
    s.resources = vec![
        (1005, res),
        (1033, thumb.clone()),
        (1036, thumb),
        (1039, b"icc-profile".to_vec()),
        (1058, b"MM\0*exif".to_vec()),
        (1060, b"<x:xmpmeta/>".to_vec()),
        (1028, b"\x1c\x02".to_vec()),
    ];
    let f = write(&s);
    let psd = parse(&f).unwrap();
    let (t, bytes) = psd.thumbnail(&f).unwrap();
    assert_eq!((t.width, t.height, t.bgr), (160, 120, false));
    assert_eq!(t.format, ThumbnailFormat::Jpeg);
    assert_eq!(bytes, &jpeg[..]);
    assert_eq!(psd.icc(&f).unwrap(), b"icc-profile");
    assert_eq!(psd.exif(&f).unwrap(), b"MM\0*exif");
    assert_eq!(psd.xmp(&f).unwrap(), b"<x:xmpmeta/>");
    assert_eq!(psd.iptc(&f).unwrap(), b"\x1c\x02");
    let r = psd.info().resolution.unwrap();
    assert_eq!(
        (r.h_dpi, r.v_dpi, r.unit),
        (300.0, 150.0, ResolutionUnit::PerInch)
    );
}

#[test]
fn maximize_compatibility_off() {
    let mut s = Spec::new(4, 4, 8, 3, rgb_planes(4, 4, 1, 3));
    s.resources
        .push((1057, version_info(false, "Adobe Photoshop")));
    let f = write(&s);
    let psd = parse(&f).unwrap();
    assert_eq!(psd.info().composite, Composite::NotMaximized);
    let e = psd.decode_rgba8(&f, &SEQ).unwrap_err();
    assert_eq!(e, Error::NoComposite(Composite::NotMaximized));
    assert!(e.to_string().contains("Maximize compatibility"));
    // Image data section cut off entirely.
    let s = Spec::new(4, 4, 8, 1, vec![vec![0; 16]]);
    let f = write(&s);
    let cut = &f[..f.len() - 18];
    let psd = parse(cut).unwrap();
    assert_eq!(psd.info().composite, Composite::Missing);
    assert!(matches!(
        psd.decode_planes(cut, &SEQ),
        Err(Error::NoComposite(Composite::Missing))
    ));
}

#[test]
fn caps_and_garbage() {
    assert_eq!(parse(b"nope").unwrap_err(), Error::NotPsd);
    let mut s = Spec::new(4, 4, 8, 1, vec![vec![0; 16]]);
    let mut f = write(&s);
    f[14..18].copy_from_slice(&40_000u32.to_be_bytes()); // height > PSD cap
    assert_eq!(parse(&f).unwrap_err(), Error::TooLarge);
    s.psb = true;
    let mut f = write(&s);
    f[14..18].copy_from_slice(&40_000u32.to_be_bytes());
    f[18..22].copy_from_slice(&40_000u32.to_be_bytes()); // 1.6 GP > MAX_PIXELS
    assert_eq!(parse(&f).unwrap_err(), Error::TooLarge);
}
