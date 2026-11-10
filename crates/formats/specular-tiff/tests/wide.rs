//! Wide (16-bit) output: integer grey and RGB(A) deeper than 8 bits keep their precision when
//! [`DecodeOptions::wide`] is set, in the most compact 16-bit format, and agree with the 8-bit
//! output within rounding.

mod common;

use common::{Comp, Spec, pattern16, u16_bytes, write};
use specular_tiff::{DecodeOptions, Decoded, PixelFormat, Region, Tiff, downscale};

fn decode(data: &[u8], scale: u32, region: Option<Region>, wide: bool) -> Decoded {
    let t = Tiff::open(data).expect("open");
    t.decode(
        data,
        0,
        &DecodeOptions {
            scale,
            region,
            wide,
            ..Default::default()
        },
    )
    .expect("decode")
}

/// Native-endian samples of a wide decode.
fn samples(d: &Decoded) -> Vec<u16> {
    assert!(d.format.is_wide());
    d.data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect()
}

/// The 8-bit rounding of the 8-bit path.
fn narrow(v: u16) -> u8 {
    ((v as u32 * 255 + 32767) / 65535) as u8
}

/// The 16-bit pixels a wide decode should give for `spp` samples of `photometric` (alpha last
/// when the sample count says so).
fn expect(v: &[u16], spp: u16, photometric: u16) -> (PixelFormat, Vec<u16>) {
    let grey = photometric < 2;
    let fmt = match (grey, spp) {
        (true, 1) => PixelFormat::Gray16,
        (true, _) => PixelFormat::GrayAlpha16,
        _ => PixelFormat::Rgba16,
    };
    let px = v
        .chunks_exact(spp as usize)
        .flat_map(|s| match (grey, spp) {
            (true, 1) => vec![s[0]],
            (true, _) => vec![s[0], s[1]],
            (false, 3) => vec![s[0], s[1], s[2], 65535],
            _ => vec![s[0], s[1], s[2], s[3]],
        })
        .collect();
    (fmt, px)
}

#[test]
fn sixteen_bit_stays_wide_in_every_layout() {
    let (w, h) = (45u32, 29u32);
    for &(spp, photo) in &[(1u16, 1u16), (2, 1), (3, 2), (4, 2)] {
        for comp in [Comp::None, Comp::Lzw, Comp::Deflate] {
            for tile in [None, Some((16, 16))] {
                for le in [true, false] {
                    for (planar, pred) in [(false, 1), (false, 2), (true, 1)] {
                        let v = pattern16(w, h, spp, spp as u32 + 5);
                        let mut s = Spec::new(w, h, spp, 16, photo, u16_bytes(&v, le));
                        s.comp = comp;
                        s.tile = tile;
                        s.rows_per_strip = 5;
                        s.planar = planar && spp > 1;
                        s.predictor = pred;
                        if spp == 2 || spp == 4 {
                            s.extra.push((338, 3, vec![2]));
                        }
                        let file = write(&[s], false, le);
                        let ctx = format!("spp {spp} {comp:?} {tile:?} le {le} planar {planar}");
                        let t = Tiff::open(&file).unwrap();
                        let (fmt, want) = expect(&v, spp, photo);
                        assert_eq!(t.pages()[0].wide_format(), Some(fmt), "{ctx}");
                        let got = decode(&file, 1, None, true);
                        assert_eq!(got.format, fmt, "{ctx}");
                        assert_eq!(got.stride, w as usize * fmt.bytes_per_pixel(), "{ctx}");
                        assert_eq!(samples(&got), want, "{ctx}");
                        // The 8-bit decode is the 16-bit one rounded.
                        let eight = decode(&file, 1, None, false);
                        let want8: Vec<u8> = match fmt {
                            PixelFormat::Gray16 => want.iter().map(|&g| narrow(g)).collect(),
                            PixelFormat::GrayAlpha16 => want
                                .chunks_exact(2)
                                .flat_map(|p| {
                                    let g = narrow(p[0]);
                                    [g, g, g, narrow(p[1])]
                                })
                                .collect(),
                            _ => want.iter().map(|&x| narrow(x)).collect(),
                        };
                        assert_eq!(eight.data, want8, "{ctx}");
                    }
                }
            }
        }
    }
}

#[test]
fn regions_and_scales() {
    let (w, h) = (130u32, 97u32);
    for &(spp, photo) in &[(1u16, 1u16), (2, 1), (4, 2)] {
        let v = pattern16(w, h, spp, 11);
        let mut s = Spec::new(w, h, spp, 16, photo, u16_bytes(&v, true));
        s.tile = Some((32, 32));
        if spp != 1 {
            s.extra.push((338, 3, vec![2]));
        }
        let file = write(&[s], false, true);
        let full = decode(&file, 1, None, true);
        let bpp = full.format.bytes_per_pixel();
        let r = Region {
            x: 17,
            y: 40,
            w: 50,
            h: 33,
        };
        let part = decode(&file, 1, Some(r), true);
        let mut want = Vec::new();
        for y in r.y..r.y + r.h {
            let o = y as usize * full.stride + r.x as usize * bpp;
            want.extend_from_slice(&full.data[o..o + r.w as usize * bpp]);
        }
        assert_eq!(part.data, want, "spp {spp} region");
        for scale in [2u32, 4] {
            let scaled = decode(&file, scale, None, true);
            assert_eq!(scaled.format, full.format);
            assert_eq!(
                (scaled.width, scaled.height),
                (w.div_ceil(scale), h.div_ceil(scale))
            );
            // The public downscale of a full decode is the same filter.
            assert_eq!(
                scaled.data,
                downscale(&full, scale).data,
                "spp {spp} /{scale}"
            );
            // Colour agrees with the 8-bit scaled decode within rounding (both average in
            // linear light, alpha-weighted).
            let eight = decode(&file, scale, None, false);
            let wide = samples(&scaled);
            let wide_px: Vec<Vec<u16>> = match spp {
                1 => wide.iter().map(|&g| vec![g]).collect(),
                2 => wide
                    .chunks_exact(2)
                    .map(|p| vec![p[0], p[0], p[0], p[1]])
                    .collect(),
                _ => wide.chunks_exact(4).map(|p| p.to_vec()).collect(),
            };
            let n8 = eight.format.bytes_per_pixel();
            for (i, (p, e)) in wide_px.iter().zip(eight.data.chunks_exact(n8)).enumerate() {
                let a = p.last().copied().filter(|_| p.len() == 4).unwrap_or(65535);
                if a < 2048 {
                    continue; // near-transparent colour is not well defined at 8 bits
                }
                for (c, (&x, &y)) in p.iter().zip(e).enumerate() {
                    assert!(
                        narrow(x).abs_diff(y) <= 1,
                        "spp {spp} /{scale} px {i} c {c}: {x} vs {y}"
                    );
                }
            }
            let r = Region {
                x: 3,
                y: 5,
                w: 9,
                h: 7,
            };
            let part = decode(&file, scale, Some(r), true);
            let mut want = Vec::new();
            for y in r.y..r.y + r.h {
                let o = y as usize * scaled.stride + r.x as usize * bpp;
                want.extend_from_slice(&scaled.data[o..o + r.w as usize * bpp]);
            }
            assert_eq!(part.data, want, "spp {spp} /{scale} region");
        }
    }
}

/// 12-bit samples packed MSB first.
fn pack12(v: &[u16], w: usize, spp: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for row in v.chunks_exact(w * spp) {
        let mut bits = 0u32;
        let mut n = 0;
        for &s in row {
            bits = (bits << 12) | (s as u32 & 0xfff);
            n += 12;
            while n >= 8 {
                out.push((bits >> (n - 8)) as u8);
                n -= 8;
            }
        }
        if n > 0 {
            out.push((bits << (8 - n)) as u8);
        }
    }
    out
}

#[test]
fn other_depths_and_models() {
    // 12-bit grey: scaled to the full 16-bit range.
    let v: Vec<u16> = vec![0, 1, 2048, 4095, 1234];
    let s = Spec::new(5, 1, 1, 12, 1, pack12(&v, 5, 1));
    let d = decode(&write(&[s], false, true), 1, None, true);
    let want: Vec<u16> = v
        .iter()
        .map(|&x| ((x as u32 * 65535 + 2047) / 4095) as u16)
        .collect();
    assert_eq!(samples(&d), want);
    assert_eq!(d.format, PixelFormat::Gray16);

    // MinIsWhite 16-bit: inverted.
    let s = Spec::new(3, 1, 1, 16, 0, u16_bytes(&[0, 1000, 65535], false));
    let d = decode(&write(&[s], false, false), 1, None, true);
    assert_eq!(samples(&d), vec![65535, 64535, 0]);

    // Signed 16-bit: offset to unsigned.
    let s16 = |v: i16| v.to_be_bytes();
    let raw: Vec<u8> = [i16::MIN, 0, i16::MAX].into_iter().flat_map(s16).collect();
    let mut s = Spec::new(3, 1, 1, 16, 1, raw);
    s.sample_format = 2;
    let d = decode(&write(&[s], false, false), 1, None, true);
    assert_eq!(samples(&d), vec![0, 32768, 65535]);

    // Associated alpha: divided out at 16 bits. Straight 0.8 at alpha 10000 is stored as 8000.
    let raw = u16_bytes(&[8000, 8000, 8000, 10000, 0, 0, 0, 0], true);
    let mut s = Spec::new(2, 1, 4, 16, 2, raw);
    s.extra.push((338, 3, vec![1]));
    let d = decode(&write(&[s], false, true), 1, None, true);
    assert_eq!(samples(&d), vec![52428, 52428, 52428, 10000, 0, 0, 0, 0]);
    let raw = u16_bytes(&[8000, 10000], true);
    let mut s = Spec::new(1, 1, 2, 16, 1, raw);
    s.extra.push((338, 3, vec![1]));
    let d = decode(&write(&[s], false, true), 1, None, true);
    assert_eq!(samples(&d), vec![52428, 10000]);
    assert_eq!(d.format, PixelFormat::GrayAlpha16);

    // 8-bit and palette images have no wide format: `wide` changes nothing.
    let s = Spec::new(2, 1, 1, 8, 1, vec![7, 9]);
    let f = write(&[s], false, true);
    assert_eq!(Tiff::open(&f).unwrap().pages()[0].wide_format(), None);
    assert_eq!(decode(&f, 1, None, true).data, vec![7, 9]);
    // Float is HDR: linear half-float RGBA, unclamped (2.5 = 0x4100, 1.0 = 0x3c00).
    let mut s = Spec::new(1, 1, 1, 32, 1, 2.5f32.to_le_bytes().to_vec());
    s.sample_format = 3;
    let f = write(&[s], false, true);
    let d = decode(&f, 1, None, true);
    assert_eq!(d.format, PixelFormat::Rgba16F);
    let h: Vec<u16> = d
        .data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(h, vec![0x4100, 0x4100, 0x4100, 0x3c00]);
    assert_eq!(decode(&f, 1, None, false).format, PixelFormat::Gray8);
}

#[test]
fn missing_data_is_grey_and_estimates_count_wide_pixels() {
    let (w, h) = (16u32, 16u32);
    let v = pattern16(w, h, 3, 2);
    let mut s = Spec::new(w, h, 3, 16, 2, u16_bytes(&v, true));
    s.rows_per_strip = 4;
    let file = write(&[s], false, true);
    let t = Tiff::open(&file).unwrap();
    let opts = |wide| DecodeOptions {
        wide,
        parallel: false,
        ..Default::default()
    };
    let (e8, e16) = (
        t.memory_estimate(0, &opts(false)),
        t.memory_estimate(0, &opts(true)),
    );
    assert!(e16 >= e8 + (w * h * 4) as u64, "{e8} vs {e16}");
    // The last strip is missing: its rows are mid grey, opaque.
    let mut s = Spec::new(w, h, 3, 16, 2, u16_bytes(&v, true));
    s.rows_per_strip = 4;
    let strip = (w * 4 * 6) as usize;
    let raw = u16_bytes(&v, true);
    s.pre = Some(
        (0..3)
            .map(|i| raw[i * strip..(i + 1) * strip].to_vec())
            .collect(),
    );
    let file = write(&[s], false, true);
    let d = Tiff::open(&file)
        .unwrap()
        .decode(&file, 0, &opts(true))
        .unwrap();
    assert!(d.truncated);
    let px = samples(&d);
    assert_eq!(
        &px[..12 * w as usize * 4],
        &expect(&v, 3, 2).1[..12 * w as usize * 4]
    );
    assert!(
        px[12 * w as usize * 4..]
            .chunks_exact(4)
            .all(|p| p == [32768, 32768, 32768, 65535])
    );
}
