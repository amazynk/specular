//! DNG `DefaultScale` (non-square pixels): the whole decode comes out with square pixels,
//! regions and `develop` keep the stored grid; and which sensors the decode demosaics itself.

mod common;

use common::{DngCodec, DngSpec, Tag, write_dng};
use specular_raw::{DecodeOptions, OutputSize, Rect};

/// RATIONAL values over 1000.
fn tag_rationals(tag: u16, v: &[f64]) -> Tag {
    let d = v
        .iter()
        .flat_map(|x| {
            [
                ((x * 1000.0).round() as u32).to_le_bytes(),
                1000u32.to_le_bytes(),
            ]
        })
        .flatten()
        .collect();
    (tag, 5, v.len() as u32, d)
}

/// A LinearRaw DNG of a horizontal ramp, `w × h`, with `scale` as its DefaultScale.
fn ramp_dng(w: u32, h: u32, scale: Option<[f64; 2]>) -> Vec<u8> {
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.linear_raw = true;
    if let Some(k) = scale {
        s.extra.push(tag_rationals(0xc61e, &k));
    }
    let mut data = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            data.extend([(2000 + x * 1500) as u16; 3]);
        }
    }
    write_dng(&s, &data)
}

#[test]
fn default_scale_gives_square_pixels() {
    let (w, h) = (24, 10);
    let plain = ramp_dng(w, h, None);
    let tall = ramp_dng(w, h, Some([1.0, 2.0]));
    // The same shape from the halved horizontal factor: the stored detail is kept.
    let half = ramp_dng(w, h, Some([0.5, 1.0]));
    let info = |f: &[u8]| {
        specular_raw::parse(f)
            .unwrap()
            .info()
            .sensor
            .clone()
            .unwrap()
    };
    assert_eq!(info(&plain).square_size, (w, h));
    assert_eq!(info(&tall).square_size, (w, 2 * h));
    assert_eq!(info(&half).square_size, (w, 2 * h));

    let opts = DecodeOptions::default();
    let dec = |f: &[u8], o: &DecodeOptions| specular_raw::parse(f).unwrap().decode(f, o).unwrap();
    let a = dec(&plain, &opts);
    let b = dec(&tall, &opts);
    assert_eq!((a.width, a.height), (w, h));
    assert_eq!((b.width, b.height), (w, 2 * h));
    // Every row of the stretched picture is a row of the plain one (the ramp runs across).
    let row = (w * 3) as usize;
    for y in 0..(2 * h) as usize {
        assert_eq!(&b.data[y * row..][..row], &a.data[..row], "row {y}");
    }
    assert_eq!(dec(&half, &opts).data, b.data);
    // Half size: half the square size.
    let small = dec(
        &tall,
        &DecodeOptions {
            size: OutputSize::Half,
            ..Default::default()
        },
    );
    assert_eq!((small.width, small.height), (w / 2, h));
    // Regions are pixels of the stored grid, as `develop` gives them.
    let r = dec(
        &tall,
        &DecodeOptions {
            region: Some(Rect {
                x: 0,
                y: 0,
                w: 8,
                h: 4,
            }),
            ..Default::default()
        },
    );
    assert_eq!((r.width, r.height), (8, 4));
}

#[test]
fn demosaiced_on_decode_is_reported() {
    let sensor = |s: &DngSpec| {
        let f = write_dng(s, &vec![4000u16; (s.width * s.height) as usize]);
        specular_raw::parse(&f)
            .unwrap()
            .info()
            .sensor
            .clone()
            .unwrap()
    };
    // RGGB: the develop demosaics it.
    let bayer = DngSpec::new(16, 16, DngCodec::U16);
    assert!(!sensor(&bayer).demosaiced);
    // Cyan, magenta, yellow, green: camera RGB on decode.
    let mut cmyg = DngSpec::new(16, 16, DngCodec::U16);
    cmyg.cfa = [0, 1, 2, 3];
    cmyg.extra.push((0xc616, 1, 4, vec![3, 4, 5, 1]));
    let s = sensor(&cmyg);
    assert!(s.demosaiced && s.decodable, "{s:?}");
    // A staggered Bayer layout too.
    let mut stag = DngSpec::new(16, 16, DngCodec::U16);
    stag.extra.push((0xc617, 3, 1, 2u16.to_le_bytes().to_vec()));
    assert!(sensor(&stag).demosaiced);
}
