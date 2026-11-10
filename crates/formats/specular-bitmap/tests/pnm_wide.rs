//! PNM samples deeper than 8 bits: [`Decode::decode_wide`] keeps them at 16 bits in the most
//! compact format, every region and scale agrees with the full decode, and the 8-bit decode is
//! the 16-bit one rounded.

use specular_bitmap::{Decode, Image, Options, PixelFormat, Region, pnm::Pnm};

const NO: &(dyn Fn() -> bool + Sync) = &|| false;

/// Deterministic samples over the whole range (`depth` per pixel, at most `maxval`).
fn samples(w: u32, h: u32, depth: u32, maxval: u32) -> Vec<u32> {
    let mut s = 0x9e37_79b9u32;
    let mut v = Vec::new();
    for y in 0..h {
        for x in 0..w {
            for c in 0..depth {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                let base = (x * 1301 + y * 977 + c * 20011) % (maxval + 1);
                // Alpha: some fully transparent and opaque pixels too.
                let alpha = (depth == 2 || depth == 4) && c == depth - 1;
                v.push(match (alpha, (x + y) % 7) {
                    (true, 0) => 0,
                    (true, 1) => maxval,
                    _ => (base ^ (s & 0x3f)).min(maxval),
                });
            }
        }
    }
    v
}

/// A PNM file: P2/P3 plain, P5/P6 binary, P7 binary with `depth` samples.
fn pnm(w: u32, h: u32, magic: u8, maxval: u32, depth: u32, v: &[u32]) -> Vec<u8> {
    let mut f = if magic == 7 {
        format!("P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH {depth}\nMAXVAL {maxval}\nENDHDR\n").into_bytes()
    } else {
        format!("P{magic}\n{w} {h}\n{maxval}\n").into_bytes()
    };
    if magic == 2 || magic == 3 {
        for (i, s) in v.iter().enumerate() {
            f.extend_from_slice(format!("{s}{}", if i % 9 == 8 { "\n" } else { " " }).as_bytes());
        }
    } else {
        for &s in v {
            f.extend_from_slice(&(s as u16).to_be_bytes());
        }
    }
    f
}

fn scale16(v: u32, maxval: u32) -> u16 {
    ((v as u64 * 65535 + maxval as u64 / 2) / maxval as u64) as u16
}

fn u16s(img: &Image) -> Vec<u16> {
    img.data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect()
}

fn narrow(v: u16) -> u8 {
    ((v as u32 * 255 + 32767) / 65535) as u8
}

/// The 8-bit RGBA or grey pixels a 16-bit image rounds to.
fn narrowed(img: &Image) -> Vec<u8> {
    let s = u16s(img);
    match img.format {
        PixelFormat::Gray16 => s.iter().map(|&g| narrow(g)).collect(),
        PixelFormat::GrayAlpha16 => s
            .chunks_exact(2)
            .flat_map(|p| [narrow(p[0]), narrow(p[0]), narrow(p[0]), narrow(p[1])])
            .collect(),
        _ => s.iter().map(|&v| narrow(v)).collect(),
    }
}

fn crop(img: &Image, r: Region) -> Vec<u8> {
    let bpp = img.format.bytes_per_pixel();
    let mut v = Vec::new();
    for y in r.y..r.y + r.h {
        let o = (y * img.width + r.x) as usize * bpp;
        v.extend_from_slice(&img.data[o..o + r.w as usize * bpp]);
    }
    v
}

#[test]
fn deep_samples_stay_sixteen_bit() {
    let (w, h) = (53u32, 37u32);
    for (magic, maxval, depth) in [
        (5u8, 65535u32, 1u32),
        (5, 1023, 1),
        (2, 4095, 1),
        (6, 65535, 3),
        (3, 1000, 3),
        (7, 65535, 1),
        (7, 65535, 2),
        (7, 4095, 3),
        (7, 65535, 4),
    ] {
        let what = format!("P{magic} maxval {maxval} depth {depth}");
        let v = samples(w, h, depth, maxval);
        let data = pnm(w, h, magic, maxval, depth, &v);
        let d = Pnm::parse(&data).unwrap();
        let fmt = match depth {
            1 => PixelFormat::Gray16,
            2 => PixelFormat::GrayAlpha16,
            _ => PixelFormat::Rgba16,
        };
        assert_eq!(d.wide_format(), Some(fmt), "{what}");
        assert_eq!(d.info().bit_depth, 16, "{what}");
        let full = d.decode_wide(&data, &Options::default(), NO).unwrap();
        assert_eq!(
            (full.format, full.width, full.height),
            (fmt, w, h),
            "{what}"
        );
        assert!(!full.truncated);
        let want: Vec<u16> = v
            .chunks_exact(depth as usize)
            .flat_map(|p| {
                let mut px: Vec<u16> = p.iter().map(|&s| scale16(s, maxval)).collect();
                if depth == 3 {
                    px.push(65535);
                }
                px
            })
            .collect();
        assert_eq!(u16s(&full), want, "{what}");
        // The 8-bit decode rounds the source directly; the 16-bit value rounded again is at
        // most one step away.
        let eight = d.decode(&data, &Options::default(), NO).unwrap();
        let n = narrowed(&full);
        assert_eq!(n.len(), eight.data.len(), "{what}");
        assert!(
            n.iter().zip(&eight.data).all(|(a, b)| a.abs_diff(*b) <= 1),
            "{what}: 8-bit and 16-bit decodes disagree"
        );

        let r = Region {
            x: 7,
            y: 11,
            w: 30,
            h: 20,
        };
        let part = d
            .decode_wide(
                &data,
                &Options {
                    region: Some(r),
                    ..Options::default()
                },
                NO,
            )
            .unwrap();
        assert_eq!(part.data, crop(&full, r), "{what} region");
        for f in [2u32, 3, 4] {
            let scaled = d.decode_wide(&data, &Options::scaled(f), NO).unwrap();
            assert_eq!(
                (scaled.width, scaled.height),
                (w.div_ceil(f), h.div_ceil(f))
            );
            let r = Region {
                x: 2,
                y: 3,
                w: 5,
                h: 4,
            };
            let part = d
                .decode_wide(
                    &data,
                    &Options {
                        scale: f,
                        region: Some(r),
                        parallel: false,
                    },
                    NO,
                )
                .unwrap();
            assert_eq!(part.data, crop(&scaled, r), "{what} /{f} region");
            // Both paths average in linear light with alpha weighting: within rounding.
            let e = d.decode(&data, &Options::scaled(f), NO).unwrap();
            let n = narrowed(&scaled);
            let px = if depth == 1 { 1 } else { 4 };
            for (i, (a, b)) in n.chunks_exact(px).zip(e.data.chunks_exact(px)).enumerate() {
                if px == 4 && b[3] < 8 {
                    continue; // nearly transparent: colour is ill-defined at 8 bits
                }
                let md = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
                assert!(md <= 1, "{what} /{f} px {i}: {a:?} vs {b:?}");
            }
        }
    }
}

#[test]
fn eight_bit_and_bitmaps_have_no_wide_format() {
    for (magic, maxval) in [(5u8, 255u32), (5, 100), (2, 255)] {
        let v = samples(4, 3, 1, maxval);
        let mut data = format!("P{magic}\n4 3\n{maxval}\n").into_bytes();
        if magic == 2 {
            for s in &v {
                data.extend_from_slice(format!("{s} ").as_bytes());
            }
        } else {
            data.extend(v.iter().map(|&s| s as u8));
        }
        let d = Pnm::parse(&data).unwrap();
        assert_eq!(d.wide_format(), None);
        let a = d.decode_wide(&data, &Options::default(), NO).unwrap();
        let b = d.decode(&data, &Options::default(), NO).unwrap();
        assert_eq!((a.format, a.data), (b.format, b.data));
    }
    let data = b"P4\n8 1\n\x0f".to_vec();
    let d = Pnm::parse(&data).unwrap();
    assert_eq!(d.wide_format(), None);
}

#[test]
fn truncated_rows_are_grey() {
    let (w, h) = (6u32, 4u32);
    for (magic, depth) in [(5u8, 1u32), (6, 3), (7, 2), (2, 1)] {
        let v = samples(w, h, depth, 65535);
        let mut data = pnm(w, h, magic, 65535, depth, &v);
        // Cut in the middle of the third row's second pixel.
        let keep = if magic == 2 {
            data.len() * 5 / 12
        } else {
            data.len() - (w as usize * 2 - 1) * depth as usize * 2 - 3
        };
        data.truncate(keep);
        let d = Pnm::parse(&data).unwrap();
        let img = d.decode_wide(&data, &Options::default(), NO).unwrap();
        assert!(img.truncated, "P{magic}");
        let s = u16s(&img);
        let ch = img.format.channels();
        let last = &s[s.len() - ch..];
        let grey: Vec<u16> = (0..ch)
            .map(|c| if c + 1 == ch && ch > 1 { 65535 } else { 32768 })
            .collect();
        assert_eq!(last, &grey[..], "P{magic}");
        // The first row is intact.
        assert_eq!(s[0], scale16(v[0], 65535), "P{magic}");
    }
}
