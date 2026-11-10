mod common;

use common::{Comp, Spec, pattern8, pattern16, u16_bytes, write};
use specular_tiff::{DecodeOptions, Decoded, PixelFormat, Region, Tiff, downscale};
use std::io::Cursor;

fn decode(data: &[u8], scale: u32, region: Option<Region>) -> Decoded {
    let t = Tiff::open(data).expect("open");
    t.decode(
        data,
        0,
        &DecodeOptions {
            scale,
            region,
            ..Default::default()
        },
    )
    .expect("decode")
}

/// Decode with the `tiff` crate, returning raw 8-bit or 16-bit samples.
fn oracle(data: &[u8]) -> tiff::decoder::DecodingResult {
    try_oracle(data).expect("oracle decode")
}

fn try_oracle(data: &[u8]) -> tiff::TiffResult<tiff::decoder::DecodingResult> {
    tiff::decoder::Decoder::new(Cursor::new(data))?.read_image()
}

fn to_rgba(samples: &[u8], spp: usize) -> Vec<u8> {
    samples
        .chunks_exact(spp)
        .flat_map(|s| match spp {
            1 => [s[0], s[0], s[0], 255],
            2 => [s[0], s[0], s[0], s[1]],
            3 => [s[0], s[1], s[2], 255],
            _ => [s[0], s[1], s[2], s[3]],
        })
        .collect()
}

fn crop(d: &Decoded, r: Region) -> Vec<u8> {
    let bpp = d.format.bytes_per_pixel();
    let mut v = Vec::new();
    for y in r.y..r.y + r.h {
        let s = y as usize * d.stride + r.x as usize * bpp;
        v.extend_from_slice(&d.data[s..s + r.w as usize * bpp]);
    }
    v
}

#[test]
fn matches_oracle_8bit() {
    let (w, h) = (75u32, 53u32);
    let mut oracle_skips = 0;
    for &(spp, photo) in &[(1u16, 1u16), (3, 2), (4, 2), (2, 1)] {
        for comp in [Comp::None, Comp::Lzw, Comp::Deflate, Comp::PackBits] {
            for tile in [None, Some((16, 32))] {
                for (big, le) in [(false, true), (true, false), (false, false), (true, true)] {
                    for (planar, pred) in [(false, 1), (false, 2), (true, 1), (true, 2)] {
                        let raw = pattern8(w, h, spp, spp as u32 * 7 + 1);
                        let mut s = Spec::new(w, h, spp, 8, photo, raw.clone());
                        s.comp = comp;
                        s.tile = tile;
                        s.rows_per_strip = 7;
                        s.planar = planar && spp > 1;
                        s.predictor = pred;
                        if spp == 4 || spp == 2 {
                            s.extra.push((338, 3, vec![2]));
                        }
                        let file = write(&[s], big, le);
                        let ctx = format!(
                            "spp {spp} {comp:?} tile {tile:?} big {big} le {le} planar {planar} pred {pred}"
                        );
                        // The oracle returns planar images plane by plane; check chunky ones.
                        if !(planar && spp > 1) {
                            match try_oracle(&file) {
                                Ok(tiff::decoder::DecodingResult::U8(want)) => {
                                    assert_eq!(want, raw, "oracle disagrees with source: {ctx}")
                                }
                                // tiff 0.11 rejects some valid tiled 4-sample LZW files
                                // ("no lzw end code found"); our decode is still checked
                                // against the source below.
                                Err(e) if comp == Comp::Lzw && tile.is_some() => {
                                    oracle_skips += 1;
                                    let _ = e;
                                }
                                o => panic!("oracle: {ctx}: {:?}", o.err()),
                            }
                        }
                        for parallel in [true, false] {
                            let t = Tiff::open(&file).unwrap();
                            let got = t
                                .decode(
                                    &file,
                                    0,
                                    &DecodeOptions {
                                        parallel,
                                        ..Default::default()
                                    },
                                )
                                .unwrap();
                            assert!(!got.truncated, "{ctx}");
                            let want = if spp == 1 {
                                assert_eq!(got.format, PixelFormat::Gray8);
                                raw.clone()
                            } else {
                                to_rgba(&raw, spp as usize)
                            };
                            assert_eq!(got.data, want, "{ctx}");
                        }
                    }
                }
            }
        }
    }
    eprintln!("oracle skipped {oracle_skips} files");
    assert!(oracle_skips < 40);
}

/// One large compressed strip is converted in parallel row bands: the same pixels as the
/// serial decode, at full size, reduced and for regions, chunky and planar, 8 and 16 bits.
#[test]
fn single_strip_converts_in_row_bands() {
    // About 3 × 10⁵ pixels: several bands.
    let (w, h) = (1003u32, 301u32);
    let naive = |s: &[u8]| -> Vec<u8> {
        s.chunks_exact(4)
            .flat_map(|p| {
                let k = 255 - p[3] as u32;
                let f = |c: u8| (((255 - c as u32) * k + 127) / 255) as u8;
                [f(p[0]), f(p[1]), f(p[2]), 255]
            })
            .collect()
    };
    for (spp, bits, photo) in [(4u16, 8u16, 5u16), (3, 8, 2), (3, 16, 2)] {
        for comp in [Comp::Lzw, Comp::Deflate, Comp::PackBits] {
            for (planar, pred) in [(false, 2), (true, 1)] {
                if bits == 16 && comp == Comp::PackBits {
                    continue;
                }
                let raw = if bits == 8 {
                    pattern8(w, h, spp, 9)
                } else {
                    u16_bytes(&pattern16(w, h, spp, 9), true)
                };
                let mut s = Spec::new(w, h, spp, bits, photo, raw.clone());
                s.rows_per_strip = h;
                s.comp = comp;
                s.predictor = pred;
                s.planar = planar;
                let file = write(&[s], false, true);
                let ctx = format!("{spp}×{bits} {comp:?} planar {planar}");
                let t = Tiff::open(&file).unwrap();
                let run = |scale, region, parallel| {
                    let d = t
                        .decode(
                            &file,
                            0,
                            &DecodeOptions {
                                scale,
                                region,
                                parallel,
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    assert!(!d.truncated, "{ctx}");
                    d.data
                };
                let full = run(1, None, true);
                assert_eq!(full, run(1, None, false), "{ctx}");
                if bits == 8 {
                    let want = if photo == 5 {
                        naive(&raw)
                    } else {
                        to_rgba(&raw, 3)
                    };
                    assert_eq!(full, want, "{ctx}");
                }
                let r = Some(Region {
                    x: 17,
                    y: 33,
                    w: 900,
                    h: 250,
                });
                for scale in [1, 2] {
                    assert_eq!(run(scale, r, true), run(scale, r, false), "{ctx} {scale}");
                }
                assert_eq!(run(4, None, true), run(4, None, false), "{ctx}");
            }
        }
    }
}

#[test]
fn sixteen_bit_to_eight() {
    let (w, h) = (64u32, 40u32);
    for le in [true, false] {
        for comp in [Comp::None, Comp::Deflate, Comp::Lzw] {
            let v = pattern16(w, h, 3, 3);
            let mut s = Spec::new(w, h, 3, 16, 2, u16_bytes(&v, le));
            s.comp = comp;
            s.predictor = 2;
            s.tile = Some((32, 16));
            let file = write(&[s], false, le);
            let tiff::decoder::DecodingResult::U16(o) = oracle(&file) else {
                panic!()
            };
            assert_eq!(o, v);
            let got = decode(&file, 1, None);
            let want: Vec<u8> = to_rgba(
                &v.iter()
                    .map(|&x| ((x as u32 * 255 + 32767) / 65535) as u8)
                    .collect::<Vec<_>>(),
                3,
            );
            assert_eq!(got.data, want);
        }
    }
}

#[test]
fn tiled_regions_match_full_decode() {
    let (w, h) = (300u32, 211u32);
    let raw = pattern8(w, h, 3, 9);
    let mut s = Spec::new(w, h, 3, 8, 2, raw);
    s.comp = Comp::Lzw;
    s.tile = Some((64, 48));
    let file = write(&[s], false, true);
    let mut seed = 77u32;
    let mut rnd = |n: u32| {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) % n
    };
    for scale in [1u32, 2, 4, 8] {
        let full = decode(&file, scale, None);
        assert_eq!(
            (full.width, full.height),
            (w.div_ceil(scale), h.div_ceil(scale))
        );
        if scale > 1 {
            // Decoding at a scale is the box filter of the full decode.
            let ds = downscale(&decode(&file, 1, None), scale);
            assert_eq!(ds.data, full.data, "scale {scale}");
        }
        for _ in 0..40 {
            let x = rnd(full.width);
            let y = rnd(full.height);
            let r = Region {
                x,
                y,
                w: 1 + rnd(full.width - x),
                h: 1 + rnd(full.height - y),
            };
            let got = decode(&file, scale, Some(r));
            assert_eq!(got.origin, (r.x, r.y));
            assert_eq!((got.width, got.height), (r.w, r.h));
            assert_eq!(got.data, crop(&full, r), "scale {scale} region {r:?}");
        }
    }
    // Oversized regions are clipped.
    let r = decode(
        &file,
        1,
        Some(Region {
            x: 290,
            y: 200,
            w: 100,
            h: 100,
        }),
    );
    assert_eq!((r.width, r.height), (10, 11));
}

#[test]
fn strips_scaled_with_odd_rows_per_strip() {
    let (w, h) = (101u32, 77u32);
    let raw = pattern8(w, h, 1, 5);
    for rps in [1u32, 3, 5, 77] {
        let mut s = Spec::new(w, h, 1, 8, 1, raw.clone());
        s.rows_per_strip = rps;
        s.comp = Comp::Deflate;
        let file = write(&[s], false, true);
        let full = decode(&file, 1, None);
        assert_eq!(full.data, raw);
        for scale in [2, 4, 8] {
            let got = decode(&file, scale, None);
            assert_eq!(
                got.data,
                downscale(&full, scale).data,
                "rps {rps} scale {scale}"
            );
            let r = Region {
                x: 3,
                y: 2,
                w: 5,
                h: 4,
            };
            let part = decode(&file, scale, Some(r));
            assert_eq!(part.data, crop(&got, r));
        }
    }
}

#[test]
fn tall_uncompressed_strips_become_small_units() {
    // One strip per plane (or very tall strips) of raw data: read as ~1 MiB virtual strips.
    let (w, h) = (1500u32, 700u32);
    let raw = pattern8(w, h, 3, 9);
    for (rps, planar, pred) in [
        (700u32, false, 1u16),
        (384, false, 2),
        (700, true, 1),
        (300, false, 1),
    ] {
        let mut s = Spec::new(w, h, 3, 8, 2, raw.clone());
        s.rows_per_strip = rps;
        s.planar = planar;
        s.predictor = pred;
        let file = write(&[s], false, true);
        let t = Tiff::open(&file).expect("open");
        let uh = t.pages()[0].ifd().unit_h;
        // 300 rows per strip has no power-of-two divisor of 8 or more: left as is.
        // ~1 MiB units: 128 rows of 4500 bytes, 256 rows of 1500-byte planar rows.
        let want_uh = match (rps, planar) {
            (300, _) => 300,
            (_, true) => 256,
            _ => 128,
        };
        assert_eq!(uh, want_uh, "rps {rps} planar {planar}");
        let full = decode(&file, 1, None);
        let want = to_rgba(&raw, 3);
        let bad = want.iter().zip(&full.data).position(|(a, b)| a != b);
        assert_eq!(
            bad,
            None,
            "rps {rps} planar {planar} pred {pred}: first bad pixel (x, y) {:?}",
            bad.map(|i| (i / 4 % w as usize, i / 4 / w as usize))
        );
        let r = Region {
            x: 333,
            y: 250,
            w: 401,
            h: 300,
        };
        assert_eq!(decode(&file, 1, Some(r)).data, crop(&full, r));
        for scale in [2, 8] {
            let got = decode(&file, scale, None);
            assert_eq!(
                got.data,
                downscale(&full, scale).data,
                "rps {rps} scale {scale}"
            );
        }
        // Cut inside the pixel data: the start still decodes, the rest is marked missing.
        let cut = &file[..file.len() * 3 / 5];
        if let Ok(t) = Tiff::open(cut) {
            let d = t
                .decode(cut, 0, &DecodeOptions::default())
                .expect("truncated decode");
            assert!(d.truncated, "rps {rps}");
        }
    }
}

#[test]
fn pages_levels_orientation_icc() {
    let (w, h) = (128u32, 96u32);
    let icc: Vec<u64> = (0..300u64).map(|i| i % 251).collect();
    let mut p0 = Spec::new(w, h, 3, 8, 2, pattern8(w, h, 3, 1));
    p0.extra.push((274, 3, vec![6]));
    p0.extra.push((34675, 7, icc.clone()));
    p0.tile = Some((32, 32));
    // Reduced levels with recognisable flat content, one as a SubIFD and one in the chain.
    let mut sub = Spec::new(64, 48, 3, 8, 2, vec![10; 64 * 48 * 3]);
    sub.subfile = 1;
    sub.tile = Some((16, 16));
    p0.sub_ifds.push(sub);
    let mut chain_level = Spec::new(32, 24, 3, 8, 2, vec![200; 32 * 24 * 3]);
    chain_level.subfile = 1;
    let p1 = Spec::new(50, 40, 1, 8, 1, pattern8(50, 40, 1, 2));
    let mut mask = Spec::new(50, 40, 1, 1, 4, vec![0xff; 7 * 40]);
    mask.subfile = 4;
    let p2 = Spec::new(10, 10, 3, 8, 2, pattern8(10, 10, 3, 3));
    for big in [false, true] {
        let file = write(
            &[
                p0.clone(),
                chain_level.clone(),
                p1.clone(),
                mask.clone(),
                p2.clone(),
            ],
            big,
            true,
        );
        let t = Tiff::open(&file).unwrap();
        assert_eq!(t.pages().len(), 3);
        let p = &t.pages()[0];
        assert_eq!(p.orientation(), 6);
        let want_icc: Vec<u8> = icc.iter().map(|&v| v as u8).collect();
        assert_eq!(p.icc_profile(&file).unwrap(), &want_icc[..]);
        assert_eq!(p.level_sizes(), vec![(128, 96), (64, 48), (32, 24)]);
        // 1/2 comes from the SubIFD, 1/4 from the chain level, 1/8 is 1/2 of the 1/4 level.
        let d2 = decode(&file, 2, None);
        assert!(d2.data.chunks(4).all(|p| p == [10, 10, 10, 255]));
        let d4 = decode(&file, 4, None);
        assert!(d4.data.chunks(4).all(|p| p == [200, 200, 200, 255]));
        let d8 = decode(&file, 8, None);
        assert_eq!((d8.width, d8.height), (16, 12));
        assert!(d8.data.chunks(4).all(|p| p == [200, 200, 200, 255]));
        let g = t.decode(&file, 1, &DecodeOptions::default()).unwrap();
        assert_eq!(g.data, pattern8(50, 40, 1, 2));
        assert!(t.decode(&file, 3, &DecodeOptions::default()).is_err());
    }
}

#[test]
fn level_one_pixel_smaller_is_padded() {
    // Page 101×61; its "half" level is stored as 50×30 (floor) instead of 51×31.
    let mut p = Spec::new(101, 61, 1, 8, 1, pattern8(101, 61, 1, 4));
    let mut sub = Spec::new(50, 30, 1, 8, 1, vec![99; 50 * 30]);
    sub.subfile = 1;
    p.sub_ifds.push(sub);
    let file = write(&[p], false, true);
    let d = decode(&file, 2, None);
    assert_eq!((d.width, d.height), (51, 31));
    assert!(d.data.iter().all(|&v| v == 99));
    let d = decode(
        &file,
        4,
        Some(Region {
            x: 20,
            y: 10,
            w: 10,
            h: 10,
        }),
    );
    assert_eq!((d.width, d.height), (6, 6));
    assert!(d.data.iter().all(|&v| v == 99));
}

#[test]
fn bilevel_palette_cmyk_float_signed() {
    // 1-bit min-is-white.
    let (w, h) = (21u32, 9u32);
    let rb = w.div_ceil(8) as usize;
    let mut raw = vec![0u8; rb * h as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            if (x + y) % 3 == 0 {
                raw[y * rb + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    let mut s = Spec::new(w, h, 1, 1, 0, raw);
    s.comp = Comp::PackBits;
    let d = decode(&write(&[s], false, true), 1, None);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let want = if (x + y) % 3 == 0 { 0 } else { 255 };
            assert_eq!(d.data[y * w as usize + x], want);
        }
    }
    // 4-bit palette.
    let mut raw = Vec::new();
    for y in 0..8u8 {
        for x in 0..4u8 {
            raw.push(((x * 2 + y) & 15) << 4 | ((x * 2 + 1 + y) & 15));
        }
    }
    let mut map = vec![0u64; 48];
    for i in 0..16u64 {
        map[i as usize] = i * 4096;
        map[16 + i as usize] = 65535 - i * 4096;
        map[32 + i as usize] = 0x8000;
    }
    let mut s = Spec::new(8, 8, 1, 4, 3, raw);
    s.extra.push((320, 3, map));
    let d = decode(&write(&[s], false, false), 1, None);
    assert_eq!(d.format, PixelFormat::Rgba8);
    for y in 0..8usize {
        for x in 0..8usize {
            let i = (x + y) & 15;
            assert_eq!(
                &d.data[(y * 8 + x) * 4..][..4],
                &[(i * 16) as u8, (255 - i * 16) as u8, 128, 255]
            );
        }
    }
    // CMYK.
    let raw = vec![0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 255, 0, 128, 255, 0];
    let mut s = Spec::new(2, 2, 4, 8, 5, raw);
    s.extra.push((332, 3, vec![1]));
    let d = decode(&write(&[s], false, true), 1, None);
    assert_eq!(
        d.data,
        vec![
            255, 255, 255, 255, 0, 255, 255, 255, 0, 0, 0, 255, 255, 127, 0, 255
        ]
    );
    // Float32 grey, linear → sRGB; and floating-point predictor via the oracle-free path.
    let vals = [0.0f32, 1.0, 0.5, 2.0, -1.0, f32::NAN];
    let raw: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut s = Spec::new(6, 1, 1, 32, 1, raw);
    s.sample_format = 3;
    let d = decode(&write(&[s], false, true), 1, None);
    assert_eq!(d.data, vec![0, 255, 188, 255, 0, 0]);
    // Signed 16-bit: offset to unsigned.
    let raw: Vec<u8> = [-32768i16, 0, 32767]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect();
    let mut s = Spec::new(3, 1, 1, 16, 1, raw);
    s.sample_format = 2;
    let d = decode(&write(&[s], false, false), 1, None);
    assert_eq!(d.data, vec![0, 128, 255]);
}

#[test]
fn associated_alpha_is_unpremultiplied() {
    let raw = vec![100, 50, 0, 128, 0, 0, 0, 0];
    let mut s = Spec::new(2, 1, 4, 8, 2, raw);
    s.extra.push((338, 3, vec![1]));
    let d = decode(&write(&[s], false, true), 1, None);
    assert_eq!(d.data, vec![199, 100, 0, 128, 0, 0, 0, 0]);

    // 16-bit: divided at full precision, not after the cut to 8 bits (which gives 203).
    // Straight 0.8 at alpha 10000/65535 is stored as 8000.
    let raw: Vec<u8> = [8000u16, 8000, 8000, 10000]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut s = Spec::new(1, 1, 4, 16, 2, raw);
    s.extra.push((338, 3, vec![1]));
    let d = decode(&write(&[s], false, true), 1, None);
    assert_eq!(d.data, vec![204, 204, 204, 39]);

    // Float: divided in linear light, before the sRGB encoding. Linear 0.5 at alpha 0.25.
    let raw: Vec<u8> = [0.125f32, 0.125, 0.0, 0.25]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut s = Spec::new(1, 1, 4, 32, 2, raw);
    s.sample_format = 3;
    s.extra.push((338, 3, vec![1]));
    let d = decode(&write(&[s], false, true), 1, None);
    assert_eq!(d.data, vec![188, 188, 0, 64]);
}

#[test]
fn malformed_inputs_never_panic() {
    let (w, h) = (90u32, 70u32);
    let mut files = Vec::new();
    for (comp, tile) in [
        (Comp::Lzw, Some((32, 32))),
        (Comp::Deflate, None),
        (Comp::PackBits, Some((16, 16))),
        (Comp::None, None),
    ] {
        let mut s = Spec::new(w, h, 3, 8, 2, pattern8(w, h, 3, 11));
        s.comp = comp;
        s.tile = tile;
        s.predictor = 2;
        let mut sub = Spec::new(45, 35, 3, 8, 2, pattern8(45, 35, 3, 1));
        sub.subfile = 1;
        s.sub_ifds.push(sub);
        files.push(write(&[s.clone(), s], comp == Comp::Deflate, true));
    }
    let try_all = |f: &[u8]| {
        if let Ok(t) = Tiff::open(f) {
            for page in 0..t.pages().len() {
                for scale in [1, 2, 8] {
                    let _ = t.decode(
                        f,
                        page,
                        &DecodeOptions {
                            scale,
                            ..Default::default()
                        },
                    );
                    let _ = t.decode(
                        f,
                        page,
                        &DecodeOptions {
                            scale,
                            region: Some(Region {
                                x: 5,
                                y: 5,
                                w: 20,
                                h: 20,
                            }),
                            parallel: false,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    };
    let mut seed = 1u32;
    for f in &files {
        // Truncations.
        for cut in (0..f.len()).step_by(37) {
            try_all(&f[..cut]);
        }
        // Truncated pixel data but intact header: decodes with `truncated`.
        // Byte flips.
        for _ in 0..400 {
            let mut g = f.clone();
            for _ in 0..4 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let i = (seed >> 8) as usize % g.len();
                g[i] ^= (seed >> 3) as u8 | 1;
            }
            try_all(&g);
        }
    }
    assert!(Tiff::open(b"II*\0\x08\0\0\0").is_err());
    assert!(Tiff::open(b"").is_err());
}

#[test]
fn absurd_dimensions_are_refused() {
    let mut s = Spec::new(16, 16, 3, 8, 2, vec![0; 16 * 16 * 3]);
    s.rows_per_strip = 16;
    let mut f = write(&[s], false, true);
    // Patch ImageWidth / ImageLength (LONG, inline) to 4M × 4M.
    let ifd = u32::from_le_bytes(f[4..8].try_into().unwrap()) as usize;
    let n = u16::from_le_bytes([f[ifd], f[ifd + 1]]) as usize;
    for i in 0..n {
        let e = ifd + 2 + i * 12;
        let tag = u16::from_le_bytes([f[e], f[e + 1]]);
        if tag == 256 || tag == 257 {
            f[e + 8..e + 12].copy_from_slice(&(1u32 << 22).to_le_bytes());
        }
    }
    if let Ok(t) = Tiff::open(&f) {
        {
            // Parses (too few strips are a decode-time matter), but the decode is refused.
            let r = t.decode(&f, 0, &DecodeOptions::default());
            assert!(r.is_err());
            // A small region still works (missing strips are grey).
            let r = t
                .decode(
                    &f,
                    0,
                    &DecodeOptions {
                        region: Some(Region {
                            x: 0,
                            y: 0,
                            w: 8,
                            h: 8,
                        }),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(r.truncated);
        }
    }
}

#[test]
fn cancel_stops_decode() {
    let (w, h) = (256u32, 256u32);
    let mut s = Spec::new(w, h, 3, 8, 2, pattern8(w, h, 3, 1));
    s.tile = Some((16, 16));
    let f = write(&[s], false, true);
    let t = Tiff::open(&f).unwrap();
    let c = std::sync::atomic::AtomicBool::new(true);
    let r = t.decode(
        &f,
        0,
        &DecodeOptions {
            cancel: Some(&c),
            ..Default::default()
        },
    );
    assert!(matches!(r, Err(specular_tiff::Error::Cancelled)));
}

fn jpeg(px: &[u8], w: u32, h: u32, spp: u16) -> Vec<u8> {
    let mut out = Vec::new();
    let ct = if spp == 1 {
        jpeg_encoder::ColorType::Luma
    } else {
        jpeg_encoder::ColorType::Rgb
    };
    jpeg_encoder::Encoder::new(&mut out, 95)
        .encode(px, w as u16, h as u16, ct)
        .unwrap();
    out
}

/// Split a JPEG into (tables-only stream, the image stream without its DQT/DHT segments).
fn split_tables(j: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut tables = vec![0xff, 0xd8];
    let mut image = vec![0xff, 0xd8];
    let mut p = 2;
    while p + 4 <= j.len() {
        let m = j[p + 1];
        if m == 0xda {
            image.extend_from_slice(&j[p..]);
            break;
        }
        let len = u16::from_be_bytes([j[p + 2], j[p + 3]]) as usize;
        let seg = &j[p..p + 2 + len];
        if m == 0xdb || m == 0xc4 {
            tables.extend_from_slice(seg);
        } else {
            image.extend_from_slice(seg);
        }
        p += 2 + len;
    }
    tables.extend_from_slice(&[0xff, 0xd9]);
    (tables, image)
}

fn smooth(w: u32, h: u32, spp: u16) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            for c in 0..spp as u32 {
                // Triangle wave: no sharp edges for JPEG to ring on.
                let t = (x * 2 + y + c * 60) % 400;
                v.push((t.min(400 - t) + 20) as u8);
            }
        }
    }
    v
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(&x, &y)| x.abs_diff(y))
        .max()
        .unwrap_or(0)
}

#[test]
fn jpeg_in_tiff() {
    let (w, h) = (100u32, 70u32);
    for (spp, photo) in [(3u16, 6u16), (1, 1)] {
        let src = smooth(w, h, spp);
        let want = if spp == 1 {
            src.clone()
        } else {
            to_rgba(&src, 3)
        };
        // One JPEG per strip of 32 rows (the last strip is short).
        let strips: Vec<Vec<u8>> = (0..h.div_ceil(32))
            .map(|i| {
                let rows = 32.min(h - i * 32);
                let a = (i * 32 * w) as usize * spp as usize;
                jpeg(
                    &src[a..a + (rows * w) as usize * spp as usize],
                    w,
                    rows,
                    spp,
                )
            })
            .collect();
        let mut s = Spec::new(w, h, spp, 8, photo, Vec::new());
        s.comp = Comp::Jpeg;
        s.rows_per_strip = 32;
        s.pre = Some(strips);
        let f = write(&[s], false, true);
        let d = decode(&f, 1, None);
        assert!(!d.truncated);
        assert!(
            max_diff(&d.data, &want) < 16,
            "strips spp {spp}: {} {:?} {:?}",
            max_diff(&d.data, &want),
            &d.data[..12],
            &want[..12]
        );

        // Tiles of 32×32 (edge tiles padded), tables shared through JPEGTables.
        let (tw, th) = (32u32, 32u32);
        let mut tables = Vec::new();
        let mut tiles = Vec::new();
        for ty in 0..h.div_ceil(th) {
            for tx in 0..w.div_ceil(tw) {
                let mut t = Vec::new();
                for y in 0..th {
                    for x in 0..tw {
                        let (sx, sy) = ((tx * tw + x).min(w - 1), (ty * th + y).min(h - 1));
                        let i = (sy * w + sx) as usize * spp as usize;
                        t.extend_from_slice(&src[i..i + spp as usize]);
                    }
                }
                let (tb, img) = split_tables(&jpeg(&t, tw, th, spp));
                tables = tb;
                tiles.push(img);
            }
        }
        let mut s = Spec::new(w, h, spp, 8, photo, Vec::new());
        s.comp = Comp::Jpeg;
        s.tile = Some((tw, th));
        s.pre = Some(tiles);
        s.extra
            .push((347, 7, tables.iter().map(|&b| b as u64).collect()));
        let f = write(&[s], true, false);
        let full = decode(&f, 1, None);
        assert!(!full.truncated);
        assert!(max_diff(&full.data, &want) < 16, "tiles spp {spp}");
        let r = Region {
            x: 30,
            y: 20,
            w: 41,
            h: 33,
        };
        assert_eq!(decode(&f, 1, Some(r)).data, crop(&full, r));
        // DCT-scaled tile decode is close to the box filter of the full decode.
        for scale in [2, 4, 8] {
            let d = decode(&f, scale, None);
            let ds = downscale(&full, scale);
            assert_eq!((d.width, d.height), (ds.width, ds.height));
            assert!(max_diff(&d.data, &ds.data) < 24, "scale {scale} spp {spp}");
            let r = Region {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            };
            assert_eq!(decode(&f, scale, Some(r)).data, crop(&d, r));
        }
    }
}

/// RGB plus alpha in JPEG strips (GDAL's `stefan_full_rgba_jpeg_contig.tif`): the four JPEG
/// components are R, G, B and alpha as stored, not CMYK. Read as CMYK, the colour came out
/// multiplied by alpha and the alpha was lost (the visual review sheets).
/// Planar JPEG (one single-component JPEG per sample and strip, as libtiff writes it): RGB
/// and RGB with an alpha plane, each plane matching its own JPEG decoded alone.
#[test]
fn jpeg_planar() {
    let (w, h) = (45u32, 70u32);
    for spp in [3u16, 4] {
        let src = smooth(w, h, spp);
        let n = (w * h) as usize;
        let planes: Vec<Vec<u8>> = (0..spp as usize)
            .map(|c| (0..n).map(|i| src[i * spp as usize + c]).collect())
            .collect();
        let mut units = Vec::new();
        let mut want = vec![0u8; n * 4];
        for (c, p) in planes.iter().enumerate() {
            let mut whole = Vec::new();
            for i in 0..h.div_ceil(32) {
                let rows = 32.min(h - i * 32);
                let a = (i * 32 * w) as usize;
                let j = jpeg(&p[a..a + (rows * w) as usize], w, rows, 1);
                let d = specular_jpeg::decode(&j, &Default::default()).unwrap();
                let pl = &d.planes[0];
                for y in 0..rows as usize {
                    whole.extend_from_slice(&pl.data[y * pl.stride..y * pl.stride + w as usize]);
                }
                units.push(j);
            }
            for (k, v) in whole.iter().enumerate() {
                want[k * 4 + c] = *v;
            }
        }
        if spp == 3 {
            want.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        }
        let mut s = Spec::new(w, h, spp, 8, 2, Vec::new());
        s.comp = Comp::Jpeg;
        s.planar = true;
        s.rows_per_strip = 32;
        s.pre = Some(units);
        if spp == 4 {
            s.extra.push((338, 3, vec![2]));
        }
        let f = write(&[s], false, true);
        let t = Tiff::open(&f).unwrap();
        t.pages()[0].check_supported().unwrap();
        let d = decode(&f, 1, None);
        assert!(!d.truncated);
        assert_eq!(
            max_diff(&d.data, &want),
            0,
            "planar JPEG with {spp} samples"
        );
    }
}

#[test]
fn jpeg_rgb_with_alpha() {
    let (w, h) = (48u32, 40u32);
    let src = smooth(w, h, 4);
    // jpeg-encoder stores Adobe-inverted CMYK: invert so the components hold `src`.
    let inverted: Vec<u8> = src.iter().map(|&v| 255 - v).collect();
    let mut j = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut j, 98);
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    enc.encode(&inverted, w as u16, h as u16, jpeg_encoder::ColorType::Cmyk)
        .unwrap();
    for associated in [false, true] {
        let mut s = Spec::new(w, h, 4, 8, 2, Vec::new());
        s.comp = Comp::Jpeg;
        s.rows_per_strip = h;
        s.pre = Some(vec![j.clone()]);
        s.extra.push((338, 3, vec![if associated { 1 } else { 2 }]));
        let d = decode(&write(&[s], false, true), 1, None);
        assert!(!d.truncated);
        assert_eq!(d.data.len(), src.len());
        for (i, (got, want)) in d.data.chunks(4).zip(src.chunks(4)).enumerate() {
            let a = want[3];
            assert!(got[3].abs_diff(a) < 8, "alpha at {i}: {got:?} {want:?}");
            let want_c = |c: u8| {
                if associated {
                    (u32::from(c) * 255 / u32::from(a)).min(255) as u8
                } else {
                    c
                }
            };
            if a >= 128 {
                for c in 0..3 {
                    assert!(
                        got[c].abs_diff(want_c(want[c])) < 16,
                        "assoc {associated} colour at {i}: {got:?} {want:?}"
                    );
                }
            }
        }
    }
}
