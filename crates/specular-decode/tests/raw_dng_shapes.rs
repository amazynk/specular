//! Synthetic DNGs through the registry: a four-colour (cyan, magenta, yellow, green) sensor
//! opens on its sensor data (demosaiced by the decode, as LinearRaw), and non-square pixels
//! (`DefaultScale`) show stretched to square ones at every scale and region.

#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod common;

use std::sync::Arc;

use common::{DngCodec, DngSpec, Tag, write_dng};
use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeRequest, Rect, Registry, Session};

fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, bytes)))
        .unwrap()
}

fn decode(s: &dyn Session, scale: u32, region: Option<Rect>) -> Raster {
    let mut req = DecodeRequest::full(scale);
    req.region = region;
    s.decode(&req, &CancelToken::new()).unwrap()
}

/// Sample (x, y) of plane `k`.
fn at(r: &Raster, k: usize, x: u32, y: u32) -> u8 {
    let p = &r.planes[k];
    p.data[y as usize * p.stride + x as usize]
}

#[test]
fn four_colour_dng_opens_on_its_sensor_data() {
    let mut s = DngSpec::new(32, 32, DngCodec::U16);
    s.cfa = [0, 1, 2, 3];
    s.extra.push((0xc616, 1, 4, vec![3, 4, 5, 1]));
    let file = write_dng(&s, &vec![6000u16; 32 * 32]);
    let sess = open("cmyg.dng", file.clone());
    let i = sess.info();
    assert!(!i.coding.contains("embedded preview only"), "{}", i.coding);
    assert_eq!((i.width, i.height), (32, 32));
    assert_eq!(sess.colour_model(), Some("CFA"));
    let r = decode(sess.as_ref(), 1, None);
    assert_eq!((r.width, r.height, r.layout), (32, 32, PixelLayout::Rgb));
    // Develop's source: three samples per pixel, no pattern (as LinearRaw).
    let raw = specular_raw::parse(&file).unwrap();
    let sd = raw
        .decode_sensor(&file, &specular_raw::DecodeOptions::default())
        .unwrap();
    assert_eq!(sd.cpp, 3);
    assert!(matches!(
        sd.meta.layout,
        specular_raw::sensor::Layout::Linear
    ));
}

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

#[test]
fn default_scale_shows_square_pixels() {
    // LinearRaw 40 × 16, pixels twice as tall as wide: shown 40 × 32. Left half dark, right
    // half bright, top and bottom alike.
    let (w, h) = (40u32, 16u32);
    let mut s = DngSpec::new(w, h, DngCodec::U16);
    s.linear_raw = true;
    s.extra.push(tag_rationals(0xc61e, &[1.0, 2.0]));
    let mut data = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            data.extend([if x < w / 2 { 3000u16 } else { 40000 }; 3]);
        }
    }
    let sess = open("tall.dng", write_dng(&s, &data));
    let i = sess.info();
    assert_eq!((i.width, i.height), (40, 32));
    assert!(!i.fast_regions);
    assert!(i.coding.contains("non-square pixels"), "{}", i.coding);
    for scale in [1, 2, 4, 8] {
        let r = decode(sess.as_ref(), scale, None);
        assert_eq!(
            (r.width, r.height),
            (40u32.div_ceil(scale), 32u32.div_ceil(scale)),
            "1/{scale}"
        );
    }
    let full = decode(sess.as_ref(), 1, None);
    assert!(at(&full, 1, 2, 30) < at(&full, 1, 37, 30));
    assert_eq!(at(&full, 1, 2, 1), at(&full, 1, 2, 30));
    // A region is the same pixels of the whole stretched picture (cut from the cached level,
    // so it may start up and left of the request).
    let rect = Rect {
        x: 16,
        y: 20,
        w: 10,
        h: 8,
    };
    let part = decode(sess.as_ref(), 1, Some(rect));
    let (ox, oy) = part.origin;
    assert!(ox <= 16 && oy <= 20 && ox + part.width >= 26 && oy + part.height >= 28);
    for k in 0..3 {
        for y in 0..part.height {
            for x in 0..part.width {
                assert_eq!(at(&part, k, x, y), at(&full, k, ox + x, oy + y));
            }
        }
    }
}
