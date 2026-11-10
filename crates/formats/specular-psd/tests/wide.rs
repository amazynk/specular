//! 16-bit composites at 16 bits: [`Psd::decode_wide`] keeps RGB and grey-like samples in the
//! most compact 16-bit format, and agrees with [`Psd::decode_rgba8`] within rounding.

mod common;

use common::*;
use specular_psd::*;

fn u16s(img: &Image) -> Vec<u16> {
    img.data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect()
}

fn narrow(v: u16) -> u8 {
    ((v as u32 * 255 + 32767) / 65535) as u8
}

/// The 8-bit pixels (grey or RGBA, as `decode_rgba8` lays them out) of a wide image.
fn narrowed(img: &Image) -> Vec<u8> {
    let s = u16s(img);
    match img.format {
        PixelFormat::GrayAlpha16 => s
            .chunks_exact(2)
            .flat_map(|p| [narrow(p[0]), narrow(p[0]), narrow(p[0]), narrow(p[1])])
            .collect(),
        _ => s.iter().map(|&v| narrow(v)).collect(),
    }
}

#[test]
fn sixteen_bit_rgb_and_grey_stay_wide() {
    let (w, h) = (37u32, 23u32);
    // (mode, planes, expected format)
    for (mode, n, fmt) in [
        (3u16, 3u32, PixelFormat::Rgba16),
        (3, 4, PixelFormat::Rgba16),
        (1, 1, PixelFormat::Gray16),
        (1, 2, PixelFormat::GrayAlpha16),
        (8, 1, PixelFormat::Gray16),
        (7, 3, PixelFormat::Rgba16),
    ] {
        for compression in [0u16, 1, 2, 3] {
            for psb in [false, true] {
                let planes: Vec<Vec<u8>> = (0..n).map(|c| pattern(w, h, c + 3, 2)).collect();
                let mut s = Spec::new(w, h, 16, mode, planes.clone());
                s.compression = compression;
                s.psb = psb;
                let f = write(&s);
                let what = format!("mode {mode} planes {n} comp {compression} psb {psb}");
                let psd = parse(&f).unwrap();
                assert_eq!(psd.info().has_alpha, n == 2 || n == 4, "{what}");
                assert_eq!(psd.wide_format(), Some(fmt), "{what}");
                let img = psd
                    .decode_wide(&f, &Options::default())
                    .unwrap()
                    .expect("wide");
                let seq = psd
                    .decode_wide(&f, &Options { parallel: false })
                    .unwrap()
                    .unwrap();
                assert_eq!(img.data, seq.data, "{what}: parallel and sequential differ");
                assert_eq!((img.format, img.width, img.height), (fmt, w, h), "{what}");
                assert_eq!(img.data.len(), (w * h) as usize * fmt.bytes_per_pixel());
                // Exact file samples, interleaved; RGB without alpha is opaque.
                let p: Vec<Vec<u16>> = planes.iter().map(|p| be_u16s(p)).collect();
                let want: Vec<u16> = (0..(w * h) as usize)
                    .flat_map(|i| {
                        let mut px: Vec<u16> = p.iter().map(|c| c[i]).collect();
                        if fmt == PixelFormat::Rgba16 && px.len() == 3 {
                            px.push(65535);
                        }
                        px
                    })
                    .collect();
                assert_eq!(u16s(&img), want, "{what}");
                // The 8-bit decode is the same pixels rounded.
                let eight = psd.decode_rgba8(&f, &Options::default()).unwrap();
                let nw = narrowed(&img);
                assert_eq!(nw.len(), eight.data.len(), "{what}");
                assert!(
                    nw.iter().zip(&eight.data).all(|(a, b)| a.abs_diff(*b) <= 1),
                    "{what}: 8-bit and 16-bit differ"
                );
            }
        }
    }
}

#[test]
fn white_matte_is_removed_at_sixteen_bits() {
    // 50 % red over transparent, flattened on white at a = 0x8000.
    let a = 0x8000u16;
    let r = 65535u16;
    let gb = 65535 - a; // 0 straight colour, matted: c' = c·a + (1 − a)
    let be = |v: u16| v.to_be_bytes().to_vec();
    let mut s = Spec::new(1, 1, 16, 3, vec![be(r), be(gb), be(gb), be(a)]);
    s.resources
        .push((1057, version_info(true, "Adobe Photoshop")));
    let f = write(&s);
    let psd = parse(&f).unwrap();
    assert!(psd.info().matted_on_white);
    let img = psd.decode_wide(&f, &Options::default()).unwrap().unwrap();
    assert_eq!(u16s(&img), [65535, 0, 0, a]);
    let eight = psd.decode_rgba8(&f, &Options::default()).unwrap();
    assert_eq!(eight.data, [255, 0, 0, 128]);
}

#[test]
fn other_depths_and_modes_have_no_wide_format() {
    let (w, h) = (5u32, 4u32);
    for (depth, mode, n) in [
        (8u16, 3u16, 3u32),
        (8, 1, 1),
        (16, 4, 4),
        (16, 9, 3),
        (32, 3, 3),
    ] {
        let bps = depth as usize / 8;
        let planes: Vec<Vec<u8>> = (0..n).map(|c| pattern(w, h, c + 1, bps)).collect();
        let f = write(&Spec::new(w, h, depth, mode, planes));
        let psd = parse(&f).unwrap();
        assert_eq!(psd.wide_format(), None, "depth {depth} mode {mode}");
        assert!(psd.decode_wide(&f, &Options::default()).unwrap().is_none());
        assert!(
            psd.decode_planes(&f, &Options::default())
                .unwrap()
                .to_wide(true)
                .is_none()
        );
    }
}

#[test]
fn truncated_data_is_grey() {
    let (w, h) = (8u32, 6u32);
    let planes: Vec<Vec<u8>> = (0..3).map(|c| pattern(w, h, c + 1, 2)).collect();
    let mut f = write(&Spec::new(w, h, 16, 3, planes));
    // Drop the last half of the blue plane.
    let cut = (w * h) as usize;
    f.truncate(f.len() - cut);
    let psd = parse(&f).unwrap();
    let img = psd.decode_wide(&f, &Options::default()).unwrap().unwrap();
    assert!(img.truncated);
    let s = u16s(&img);
    let last = &s[s.len() - 4..];
    // Missing samples are the decoder's mid grey, as in the 8-bit decode.
    assert_eq!(narrow(last[2]), GREY);
    assert_eq!(last[3], 65535);
}
