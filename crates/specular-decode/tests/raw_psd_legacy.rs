//! Camera RAW, Photoshop and the legacy formats through the registry (synthetic files only).

#[path = "../../formats/specular-legacy/tests/common/mod.rs"]
mod lg;
#[path = "../../formats/specular-raw/tests/common/mod.rs"]
mod rw;

use std::path::Path;
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Raster, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, Rect, Registry, Session};

fn try_open(name: &str, bytes: Vec<u8>) -> specular_decode::Result<Arc<dyn Session>> {
    Registry::default().open(Arc::new(Source::from_vec(name, bytes)))
}

fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn Session> {
    try_open(name, bytes).unwrap()
}

fn px(r: &Raster, x: u32, y: u32) -> &[u8] {
    let p = &r.planes[0];
    let bpp = p.bytes_per_pixel();
    let (x, y) = ((x - r.origin.0) as usize, (y - r.origin.1) as usize);
    &p.data[y * p.stride + x * bpp..][..bpp]
}

/// Full decode, half decode, and a region that matches the full decode.
fn check_levels(s: &dyn Session) -> Raster {
    let c = CancelToken::new();
    let (w, h) = (s.info().width, s.info().height);
    let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!((full.width, full.height), (w, h));
    for scale in [2, 8] {
        let l = s.decode(&DecodeRequest::full(scale), &c).unwrap();
        assert_eq!(
            (l.width, l.height, l.scale),
            (w.div_ceil(scale), h.div_ceil(scale), scale)
        );
    }
    let r = Rect {
        x: 1,
        y: 1,
        w: w - 2,
        h: h - 2,
    };
    let part = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(1)
            },
            &c,
        )
        .unwrap();
    // The result covers the region (it may start earlier, as JPEG crops do).
    assert!(part.origin.0 <= r.x && part.origin.1 <= r.y);
    assert!(part.origin.0 + part.width >= r.x + r.w && part.origin.1 + part.height >= r.y + r.h);
    if full.layout != PixelLayout::Rgb {
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                assert_eq!(px(&part, x, y), px(&full, x, y));
            }
        }
    }
    assert!(s.decode_memory(&DecodeRequest::full(1)) > 0);
    full
}

// ---- TIFF fixtures ---------------------------------------------------------------------------

/// Little-endian single-strip TIFF: 1 sample of `bits` (8 or 16) per pixel, BlackIsZero.
fn grey_tiff(w: u32, h: u32, bits: u16) -> Vec<u8> {
    let mut d = b"II*\0\0\0\0\0".to_vec();
    let data_off = d.len() as u32;
    let bpp = u32::from(bits / 8);
    for i in 0..w * h * bpp {
        d.push((i * 7) as u8);
    }
    if d.len() % 2 == 1 {
        d.push(0);
    }
    let ifd = d.len() as u32;
    d[4..8].copy_from_slice(&ifd.to_le_bytes());
    let tags: [(u16, u16, u32); 9] = [
        (256, 4, w),
        (257, 4, h),
        (258, 3, u32::from(bits)),
        (259, 3, 1),
        (262, 3, 1),
        (273, 4, data_off),
        (277, 3, 1),
        (278, 4, h),
        (279, 4, w * h * bpp),
    ];
    d.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for (tag, ty, v) in tags {
        d.extend_from_slice(&tag.to_le_bytes());
        d.extend_from_slice(&ty.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        if ty == 3 {
            d.extend_from_slice(&(v as u16).to_le_bytes());
            d.extend_from_slice(&[0, 0]);
        } else {
            d.extend_from_slice(&v.to_le_bytes());
        }
    }
    d.extend_from_slice(&[0; 4]);
    d
}

// ---- Camera RAW ------------------------------------------------------------------------------

#[test]
fn dng_develops_through_registry() {
    let (w, h) = (64, 48);
    let spec = rw::DngSpec::new(w, h, rw::DngCodec::U16);
    let data = rw::test_pattern(w as usize, h as usize, 1, 16, 7);
    let s = open("x.dng", rw::write_dng(&spec, &data));
    let i = s.info();
    assert_eq!(i.format, Format::Raw);
    assert_eq!((i.width, i.height), (w, h));
    assert!(i.coding.contains("DNG"), "{}", i.coding);
    let cam = s.camera().expect("camera metadata");
    assert!(cam.camera.contains("Specular"), "{cam:?}");
    let full = check_levels(&*s);
    assert_eq!(full.layout, PixelLayout::Rgb);
    // Cancelled before the level exists.
    let c = CancelToken::new();
    c.cancel();
    let s2 = open("y.dng", rw::write_dng(&spec, &data));
    assert!(matches!(
        s2.decode(&DecodeRequest::full(4), &c),
        Err(DecodeError::Cancelled)
    ));
}

/// Sample `(x, y)` of every plane of a planar raster (image coordinates).
fn planar_px(r: &Raster, x: u32, y: u32) -> Vec<u8> {
    let (x, y) = ((x - r.origin.0) as usize, (y - r.origin.1) as usize);
    r.planes.iter().map(|p| p.data[y * p.stride + x]).collect()
}

/// 1/1 regions of a lossless ARW develop from the sensor tiles behind them, equal to the same
/// pixels of the full develop, and cost a fraction of it.
#[test]
fn arw_regions_develop_from_tiles() {
    let mut spec = rw::ArwSpec::new(200, 136, 32);
    spec.wb = Some([2400, 1024, 1024, 1700]);
    let data = rw::test_pattern(200, 136, 1, 14, 3);
    let file = rw::write_arw(&spec, &data);
    let s = open("x.arw", file.clone());
    let c = CancelToken::new();
    let full_req = DecodeRequest::full(1);
    let regions = [
        Rect {
            x: 37,
            y: 29,
            w: 40,
            h: 33,
        },
        Rect {
            x: 0,
            y: 0,
            w: 5,
            h: 5,
        },
        Rect {
            x: 150,
            y: 100,
            w: 50,
            h: 36,
        },
    ];
    let mut parts = Vec::new();
    for r in regions {
        let req = DecodeRequest {
            region: Some(r),
            ..DecodeRequest::full(1)
        };
        assert!(s.decode_memory(&req) * 4 < s.decode_memory(&full_req));
        let part = s.decode(&req, &c).unwrap();
        assert_eq!(
            (part.origin, part.width, part.height, part.scale),
            ((r.x, r.y), r.w, r.h, 1)
        );
        parts.push((r, part));
    }
    assert!(s.memory_bytes() > 0, "tiles held");
    // The full develop (another session, so nothing is shared).
    let full = open("y.arw", file).decode(&full_req, &c).unwrap();
    for (r, part) in &parts {
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                assert_eq!(planar_px(part, x, y), planar_px(&full, x, y), "{x},{y}");
            }
        }
    }
    // Cancelled region decodes stop.
    let stop = CancelToken::new();
    stop.cancel();
    let s2 = open("z.arw", rw::write_arw(&spec, &data));
    let req = DecodeRequest {
        region: Some(regions[0]),
        ..DecodeRequest::full(1)
    };
    assert!(matches!(
        s2.decode(&req, &stop),
        Err(DecodeError::Cancelled)
    ));
}

/// Region requests at every level (1/1 to 1/8) equal the same pixels of the whole level: from
/// the sensor tiles behind them (as-shot white balance), or, for a file whose develop needs
/// global statistics (grey-world white balance), from the sensor data decoded once and held.
#[test]
fn raw_regions_at_every_level() {
    let data = rw::test_pattern(200, 136, 1, 14, 7);
    for as_shot in [true, false] {
        let mut spec = rw::ArwSpec::new(200, 136, 32);
        spec.crop = Some([2, 1, 195, 133]);
        if as_shot {
            spec.wb = Some([2400, 1024, 1024, 1700]);
        }
        let file = rw::write_arw(&spec, &data);
        let s = open("x.arw", file.clone());
        assert_eq!(s.info().fast_regions, as_shot);
        let (w, h) = (s.info().width, s.info().height);
        let c = CancelToken::new();
        let fresh = open("y.arw", file);
        let first = Rect {
            x: 37,
            y: 29,
            w: 40,
            h: 33,
        };
        if !as_shot {
            // The first 1/1 region decodes the sensor and holds it; later ones cost their pixels.
            let req = DecodeRequest {
                region: Some(first),
                ..DecodeRequest::full(1)
            };
            s.decode(&req, &c).unwrap();
            let sensor = 200 * 136 * 2;
            assert!(s.memory_bytes() >= sensor, "{}", s.memory_bytes());
            assert!(s.decode_memory(&req) < sensor as u64 / 4);
        }
        for scale in [1u32, 2, 4, 8] {
            let whole = fresh.decode(&DecodeRequest::full(scale), &c).unwrap();
            let (lw, lh) = (whole.width, whole.height);
            let regions = [
                first,
                Rect {
                    x: 0,
                    y: 0,
                    w: 3,
                    h: 2,
                },
                Rect {
                    x: lw.saturating_sub(7),
                    y: lh.saturating_sub(5),
                    w: 20,
                    h: 20,
                },
                Rect {
                    x: 1,
                    y: 3,
                    w: lw,
                    h: lh,
                },
            ];
            for r in regions {
                let req = DecodeRequest {
                    region: Some(r),
                    parallel: false,
                    ..DecodeRequest::full(scale)
                };
                let part = s.decode(&req, &c).unwrap();
                assert_eq!(part.scale, scale);
                let (x1, y1) = ((r.x + r.w).min(lw), (r.y + r.h).min(lh));
                assert!(part.origin.0 <= r.x.min(lw) && part.origin.1 <= r.y.min(lh));
                assert!(part.origin.0 + part.width >= x1 && part.origin.1 + part.height >= y1);
                for y in r.y.min(lh)..y1 {
                    for x in r.x.min(lw)..x1 {
                        assert_eq!(
                            planar_px(&part, x, y),
                            planar_px(&whole, x, y),
                            "as shot {as_shot}, 1/{scale} {r:?}: {x},{y}"
                        );
                    }
                }
            }
        }
        // Whole levels are unchanged by the held data (and by regions decoded before them).
        for scale in [2u32, 1] {
            let a = s.decode(&DecodeRequest::full(scale), &c).unwrap();
            let b = fresh.decode(&DecodeRequest::full(scale), &c).unwrap();
            assert_eq!((a.width, a.height), (b.width, b.height));
            assert!(
                a.planes
                    .iter()
                    .zip(&b.planes)
                    .all(|(p, q)| p.data == q.data)
            );
        }
        assert_eq!((w, h), (195, 133));
    }
}

/// Pixels of `r` (clipped to `whole`) in `part` and `whole` are the same.
fn same_pixels(part: &Raster, whole: &Raster, r: Rect, what: &str) {
    let (x1, y1) = ((r.x + r.w).min(whole.width), (r.y + r.h).min(whole.height));
    for y in r.y..y1 {
        for x in r.x..x1 {
            assert_eq!(
                planar_px(part, x, y),
                planar_px(whole, x, y),
                "{what}: {x},{y}"
            );
        }
    }
}

/// Held sensor data is released by `trim_memory` (counted in `memory_bytes` before, not after)
/// and decoded again, transparently, by the next region.
#[test]
fn held_sensor_released_on_trim() {
    let data = rw::test_pattern(200, 136, 1, 14, 7);
    let mut spec = rw::ArwSpec::new(200, 136, 32);
    spec.crop = Some([2, 1, 195, 133]);
    // No white balance: grey world needs the whole sensor, so regions hold it.
    let file = rw::write_arw(&spec, &data);
    let s = open("x.arw", file.clone());
    assert!(!s.info().fast_regions);
    let whole = open("y.arw", file)
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let c = CancelToken::new();
    let r = Rect {
        x: 37,
        y: 29,
        w: 40,
        h: 33,
    };
    let req = DecodeRequest {
        region: Some(r),
        ..DecodeRequest::full(1)
    };
    assert_eq!(s.memory_bytes(), 0);
    assert_eq!(s.trim_memory(), 0);
    let a = s.decode(&req, &c).unwrap();
    let sensor = 200 * 136 * 2;
    let held = s.memory_bytes();
    assert!(held >= sensor, "{held}");
    assert_eq!(s.trim_memory(), held);
    assert_eq!(s.memory_bytes(), 0);
    // The next 1/1 region decodes and holds the sensor again; the same pixels.
    let b = s.decode(&req, &c).unwrap();
    assert!(s.memory_bytes() >= sensor);
    assert!(
        a.planes
            .iter()
            .zip(&b.planes)
            .all(|(p, q)| p.data == q.data)
    );
    same_pixels(&b, &whole, r, "after trim");
    // A 1/2 region after a trim develops from a whole half level instead, equal too.
    s.trim_memory();
    let half = open("z.arw", rw::write_arw(&spec, &data))
        .decode(&DecodeRequest::full(2), &c)
        .unwrap();
    let h = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(2)
            },
            &c,
        )
        .unwrap();
    same_pixels(&h, &half, r, "1/2 after trim");
}

/// Developed whole levels are released by `trim_memory` too (with any sensor data the level
/// decode kept); the next request develops them again, the same pixels, and a region whose
/// level is gone develops alone.
#[test]
fn developed_levels_released_on_trim() {
    let data = rw::test_pattern(200, 136, 1, 14, 5);
    let mut spec = rw::ArwSpec::new(200, 136, 32);
    spec.crop = Some([2, 1, 195, 133]);
    let s = open("x.arw", rw::write_arw(&spec, &data));
    let c = CancelToken::new();
    let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
    let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
    let held = s.memory_bytes();
    // At least the two developed 8-bit RGB levels.
    let levels = (full.width * full.height + half.width * half.height) as usize * 3;
    assert!(held >= levels, "{held} < {levels}");
    assert_eq!(s.trim_memory(), held);
    assert_eq!(s.memory_bytes(), 0);
    assert_eq!(s.trim_memory(), 0);
    let r = Rect {
        x: 31,
        y: 17,
        w: 50,
        h: 40,
    };
    let part = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(2)
            },
            &c,
        )
        .unwrap();
    same_pixels(&part, &half, r, "1/2 region after trim");
    let again = s.decode(&DecodeRequest::full(2), &c).unwrap();
    assert!(
        again
            .planes
            .iter()
            .zip(&half.planes)
            .all(|(p, q)| p.data == q.data)
    );
    assert!(s.memory_bytes() >= (half.width * half.height) as usize * 3);
}

/// Regions developed from held sensor data are assembled from developed 256-px tiles: across
/// tile edges, while panning over tiles developed before, at every level, they equal the same
/// pixels of the whole levels; the tiles count in `memory_bytes` and go on `trim_memory`.
#[test]
fn held_regions_from_developed_tiles() {
    let (w, h) = (1100u32, 800u32);
    let data = rw::test_pattern(w as usize, h as usize, 1, 14, 11);
    let mut spec = rw::ArwSpec::new(w, h, 64);
    spec.crop = Some([3, 2, 1090, 793]);
    // No white balance: grey world, so regions hold the sensor.
    let file = rw::write_arw(&spec, &data);
    let c = CancelToken::new();
    let s = open("x.arw", file.clone());
    assert!(!s.info().fast_regions);
    let fresh = open("y.arw", file);
    let at = |r: Rect, scale: u32| DecodeRequest {
        region: Some(r),
        ..DecodeRequest::full(scale)
    };
    let r0 = Rect {
        x: 201,
        y: 187,
        w: 140,
        h: 120,
    };
    let first = s.decode(&at(r0, 1), &c).unwrap();
    let sensor = (w * h * 2) as usize;
    let after_first = s.memory_bytes();
    // The sensor and the four 256-px tiles around (250, 250).
    assert!(after_first >= sensor + 4 * 256 * 256 * 3, "{after_first}");
    let mut wholes = Vec::new();
    for scale in [1u32, 2, 4, 8] {
        let whole = fresh.decode(&DecodeRequest::full(scale), &c).unwrap();
        let (lw, lh) = (whole.width, whole.height);
        let mut rs = vec![
            r0,
            Rect {
                x: 0,
                y: 0,
                w: 29,
                h: 17,
            },
            Rect {
                x: lw.saturating_sub(37),
                y: lh.saturating_sub(23),
                w: 64,
                h: 64,
            },
            Rect {
                x: 0,
                y: 0,
                w: lw,
                h: lh,
            },
        ];
        // Panning right over tiles developed before.
        rs.extend((0..5).map(|k| Rect {
            x: 90 * k,
            y: 230,
            w: 300,
            h: 70,
        }));
        for r in rs {
            let part = s.decode(&at(r, scale), &c).unwrap();
            assert_eq!(part.scale, scale, "1/{scale} {r:?}");
            same_pixels(&part, &whole, r, &format!("1/{scale} {r:?}"));
        }
        wholes.push(whole);
    }
    // The same pixels again, now from kept tiles.
    let again = s.decode(&at(r0, 1), &c).unwrap();
    assert!(
        again
            .planes
            .iter()
            .zip(&first.planes)
            .all(|(p, q)| p.data == q.data)
    );
    let all = s.memory_bytes();
    assert!(all > after_first, "{all}");
    assert_eq!(s.trim_memory(), all);
    assert_eq!(s.memory_bytes(), 0);
    let part = s.decode(&at(r0, 2), &c).unwrap();
    same_pixels(&part, &wholes[1], r0, "1/2 after trim");
}

/// A whole level's decode keeps the sensor data while memory allows, so the first 1:1 region
/// needs no second sensor decode; it equals the whole develop.
#[test]
fn whole_level_keeps_sensor_for_regions() {
    let data = rw::test_pattern(300, 200, 1, 14, 5);
    let spec = rw::ArwSpec::new(300, 200, 32);
    let file = rw::write_arw(&spec, &data);
    let c = CancelToken::new();
    let s = open("x.arw", file.clone());
    s.decode(&DecodeRequest::full(4), &c).unwrap();
    let sensor = 300 * 200 * 2;
    // Unless memory is tight (a quarter of what is available is under the sensor).
    if specular_core::available_memory().is_some_and(|a| a > 1 << 30) {
        assert!(s.memory_bytes() >= sensor, "{}", s.memory_bytes());
    }
    let r = Rect {
        x: 55,
        y: 40,
        w: 180,
        h: 99,
    };
    let part = s
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(1)
            },
            &c,
        )
        .unwrap();
    let whole = open("y.arw", file)
        .decode(&DecodeRequest::full(1), &c)
        .unwrap();
    same_pixels(&part, &whole, r, "1/1 after a whole level");
}

#[test]
fn dng_found_by_content_under_any_name() {
    let spec = rw::DngSpec::new(16, 16, rw::DngCodec::Lj92Strip);
    let data = rw::test_pattern(16, 16, 1, 16, 3);
    let s = open("noext", rw::write_dng(&spec, &data));
    assert_eq!(s.info().format, Format::Raw);
    // Even when named .tif, it is not claimed by TIFF.
    let s = open("x.tif", rw::write_dng(&spec, &data));
    assert_eq!(s.info().format, Format::Raw);
}

#[test]
fn plain_tiffs_stay_tiff() {
    for bits in [8, 16] {
        let s = open("g.tif", grey_tiff(12, 10, bits));
        assert_eq!(s.info().format, Format::Tiff, "{bits}-bit grey");
        let d = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!((d.width, d.height), (12, 10));
    }
}

// ---- Photoshop -------------------------------------------------------------------------------

/// 8-bit RGB PSD with a raw composite, an optional raw-RGB thumbnail (1036) and an optional
/// version-info resource (1057) saying whether the composite is real.
fn psd(w: u32, h: u32, thumb: bool, real_merged: Option<bool>) -> Vec<u8> {
    let mut f = b"8BPS".to_vec();
    f.extend(1u16.to_be_bytes());
    f.extend([0; 6]);
    f.extend(3u16.to_be_bytes());
    f.extend(h.to_be_bytes());
    f.extend(w.to_be_bytes());
    f.extend(8u16.to_be_bytes());
    f.extend(3u16.to_be_bytes()); // RGB
    f.extend(0u32.to_be_bytes()); // colour mode data
    let mut res = Vec::new();
    let mut resource = |id: u16, body: Vec<u8>| {
        res.extend_from_slice(b"8BIM");
        res.extend(id.to_be_bytes());
        res.extend([0, 0]);
        res.extend((body.len() as u32).to_be_bytes());
        let odd = body.len() % 2 == 1;
        res.extend(body);
        if odd {
            res.push(0);
        }
    };
    if thumb {
        let (tw, th) = (4u32, 3u32);
        let mut b = Vec::new();
        for v in [0, tw, th, tw * 3, tw * th * 3, tw * th * 3, 24] {
            b.extend(v.to_be_bytes());
        }
        b.truncate(26);
        b.extend(1u16.to_be_bytes());
        b.extend((0..tw * th * 3).map(|i| (i * 20) as u8));
        resource(1036, b);
    }
    if let Some(m) = real_merged {
        let mut b = 1u32.to_be_bytes().to_vec();
        b.push(m as u8);
        for _ in 0..2 {
            b.extend(1u32.to_be_bytes());
            b.extend(u16::from(b'x').to_be_bytes());
        }
        b.extend(1u32.to_be_bytes());
        resource(1057, b);
    }
    f.extend((res.len() as u32).to_be_bytes());
    f.extend(res);
    f.extend(0u32.to_be_bytes()); // layer and mask info
    f.extend(0u16.to_be_bytes()); // raw composite
    for c in 0..3u32 {
        for i in 0..w * h {
            f.push((i * (c + 1)) as u8);
        }
    }
    f
}

#[test]
fn psd_composite_and_thumbnail() {
    let (w, h) = (20, 14);
    let s = open("x.psd", psd(w, h, true, Some(true)));
    let i = s.info();
    assert_eq!(i.format, Format::Psd);
    assert_eq!((i.width, i.height), (w, h));
    let full = check_levels(&*s);
    assert_eq!(full.layout, PixelLayout::Rgba);
    assert_eq!(px(&full, 3, 1), [23, 46, 69, 255]);
    let t = s.decode_preview().expect("thumbnail").unwrap();
    assert_eq!((t.width, t.height, t.scale), (4, 3, 0));
    assert_eq!(px(&t, 1, 0), [60, 80, 100, 255]);
    // By content alone.
    assert_eq!(
        open("noext", psd(w, h, false, None)).info().format,
        Format::Psd
    );
}

/// A header claiming 30000 × 17000 (under the pixel cap) over a few bytes: the native planes
/// and the converted page (3.5 GB together) are refused under a 64 MiB budget before either
/// is allocated.
#[test]
fn psd_canvases_within_memory_budget() {
    let mut f = psd(8, 8, false, None);
    f[14..18].copy_from_slice(&17_000u32.to_be_bytes());
    f[18..22].copy_from_slice(&30_000u32.to_be_bytes());
    let s = open("x.psd", f);
    let r = specular_core::budget::with_limit(64 << 20, || {
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
    });
    match r {
        Err(DecodeError::NeedsMemory(n)) => assert!(n.needed >= 3_000_000_000, "{n:?}"),
        r => panic!("{:?}", r.map(|r| (r.width, r.height))),
    }
}

#[test]
fn psd_without_merged_image_explains() {
    let s = open("x.psd", psd(8, 8, true, Some(false)));
    assert!(
        s.info().coding.contains("no merged image"),
        "{}",
        s.info().coding
    );
    assert!(s.decode_preview().is_some());
    match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
        Err(DecodeError::Unsupported(m)) => {
            assert!(m.contains("Maximize compatibility"), "{m}")
        }
        r => panic!("{:?}", r.map(|r| (r.width, r.height))),
    }
}

// ---- Legacy formats --------------------------------------------------------------------------

#[test]
fn legacy_stills_by_content_and_extension() {
    let (w, h) = (13usize, 9usize);
    let idx = lg::pattern(w * h, 5, 255);
    let bits: Vec<bool> = idx.iter().map(|&v| v & 1 == 1).collect();
    let px16: Vec<u16> = idx
        .iter()
        .flat_map(|&v| [u16::from(v) * 257, 0, 65535 - u16::from(v) * 257, 65535])
        .collect();
    let grey_lines: Vec<Vec<u8>> = idx.chunks(w).map(|r| [r, &[0][..]].concat()).collect();
    let map: Vec<[u8; 3]> = (0..256).map(|i| [i as u8, 0, (255 - i) as u8]).collect();
    let sun_raw: Vec<u8> = idx.chunks(w).flat_map(|r| [r, &[0][..]].concat()).collect();
    let sgi_chan: Vec<u16> = idx.iter().map(|&v| u16::from(v)).collect();
    let xpm = b"/* XPM */\nstatic char *x[] = {\n\"3 2 2 1\",\n\". c #ff0000\",\n\"# c None\",\n\".#.\",\n\"#.#\"};\n".to_vec();
    type Case = (Format, &'static str, Vec<u8>, (usize, usize));
    let cases: Vec<Case> = vec![
        (
            Format::Farbfeld,
            "ff",
            lg::farbfeld(w as u32, h as u32, &px16),
            (w, h),
        ),
        (
            Format::Pcx,
            "pcx",
            lg::pcx(
                w as u16,
                h as u16,
                8,
                1,
                w as u16 + 1,
                true,
                &[],
                &grey_lines,
                None,
            ),
            (w, h),
        ),
        (
            Format::SunRaster,
            "ras",
            lg::sun(w as u32, h as u32, 8, 2, &map, &sun_raw),
            (w, h),
        ),
        (
            Format::Sgi,
            "sgi",
            lg::sgi(w as u16, h as u16, 1, true, &[sgi_chan]),
            (w, h),
        ),
        (Format::Xbm, "xbm", lg::xbm(w, h, &bits, false), (w, h)),
        (Format::Xpm, "xpm", xpm, (3, 2)),
        (
            Format::Wbmp,
            "wbmp",
            lg::wbmp(w as u32, h as u32, &bits),
            (w, h),
        ),
    ];
    let reg = Registry::default();
    for (format, ext, bytes, (cw, ch)) in cases {
        assert!(
            reg.supports_extension(Path::new(&format!("a.{ext}"))),
            "{ext}"
        );
        for name in [format!("a.{ext}"), "noext".to_string()] {
            let s = open(&name, bytes.clone());
            let i = s.info();
            assert_eq!(i.format, format, "{name}");
            assert_eq!((i.width, i.height), (cw as u32, ch as u32), "{name}");
            if cw > 2 && ch > 2 {
                check_levels(&*s);
            }
        }
    }
    // Spot checks of the pixels.
    let s = open("a.ff", lg::farbfeld(w as u32, h as u32, &px16));
    let full = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    let v = idx[w + 2];
    assert_eq!(px(&full, 2, 1), [v, 0, 255 - v, 255]);
    let s = open("a.xpm", b"/* XPM */\nstatic char *x[] = {\n\"2 1 2 1\",\n\". c #ff0000\",\n\"# c None\",\n\".#\"};\n".to_vec());
    let full = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(px(&full, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&full, 1, 0)[3], 0);
}

#[test]
fn new_extensions_are_known() {
    let reg = Registry::default();
    for ext in [
        "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "rwl", "pef",
        "srw", "psd", "psb", "pcx", "sgi", "rgb", "rgba", "bw", "int", "inta", "ras", "sun", "rs",
        "im1", "im8", "im24", "im32", "xbm", "xpm", "wbmp", "ff", "fli", "flc", "NEF", "Psd",
    ] {
        assert!(
            reg.supports_extension(Path::new(&format!("x.{ext}"))),
            "{ext}"
        );
    }
}

fn fli_spec(flc: bool, n: usize) -> (lg::FliSpec, [[u8; 3]; 256]) {
    let (w, h) = (16usize, 10usize);
    let mut pal = [[0u8; 3]; 256];
    for (i, c) in pal.iter_mut().enumerate() {
        *c = [i as u8, (i * 2) as u8, 255 - i as u8];
        if !flc {
            // FLI palettes are 6-bit: use values that survive the round trip.
            for v in c.iter_mut() {
                let q = *v >> 2;
                *v = (q << 2) | (q >> 4);
            }
        }
    }
    let frames = (0..n)
        .map(|k| {
            let img: Vec<u8> = (0..w * h).map(|p| (p + k * 9) as u8).collect();
            (img, pal, 0, false)
        })
        .collect();
    let spec = lg::FliSpec {
        flc,
        w,
        h,
        speed: if flc { 40 } else { 2 },
        frames,
        copy_first: false,
        extras: false,
    };
    (spec, pal)
}

#[test]
fn fli_animation_frames() {
    for (flc, name) in [(false, "a.fli"), (true, "a.flc")] {
        let (spec, pal) = fli_spec(flc, 4);
        let bytes = lg::fli(&spec);
        for name in [name, "noext"] {
            let s = open(name, bytes.clone());
            let i = s.info();
            assert_eq!(i.format, Format::Fli);
            assert_eq!((i.width, i.height), (16, 10));
            let a = s.animation().expect("animated");
            assert_eq!((a.frame_count, a.loop_count), (Some(4), 0));
            // The still decode is the first frame.
            let still = check_levels(&*s);
            assert_eq!(still.planes[0].data, lg::rgba(&spec.frames[0].0, &pal));

            let c = CancelToken::new();
            let mut fr = s.frames(1).unwrap();
            for pass in 0..2 {
                for (k, (img, _, _, _)) in spec.frames.iter().enumerate() {
                    let f = fr.next_frame(&c).unwrap().unwrap();
                    assert_eq!(f.index as usize, k);
                    assert_eq!(f.raster.planes[0].data, lg::rgba(img, &pal), "{pass} {k}");
                    assert!(!f.delay.is_zero());
                }
                assert!(fr.next_frame(&c).is_none());
                fr.rewind().unwrap();
            }
            let mut half = s.frames(2).unwrap();
            let f = half.next_frame(&c).unwrap().unwrap();
            assert_eq!((f.raster.width, f.raster.height, f.raster.scale), (8, 5, 2));
            c.cancel();
            assert!(matches!(
                half.next_frame(&c),
                Some(Err(DecodeError::Cancelled))
            ));
        }
    }
    // A single-frame FLI is a still.
    let (spec, _) = fli_spec(false, 1);
    let s = open("one.fli", lg::fli(&spec));
    assert!(s.animation().is_none());
    assert!(s.frames(1).is_err());
}

#[test]
fn garbage_with_legacy_extension_fails_cleanly() {
    for name in ["x.pcx", "x.wbmp", "x.fli", "x.psd", "x.dng", "x.ff"] {
        assert!(try_open(name, vec![0x5a; 40]).is_err(), "{name}");
    }
}

#[path = "common/trim.rs"]
mod trim;

/// Trimming drops the cached levels of PSD and a legacy format; the next request decodes them
/// again.
#[test]
fn psd_and_legacy_levels_trimmed() {
    let (w, h) = (13usize, 9usize);
    let px16: Vec<u16> = lg::pattern(w * h, 5, 255)
        .iter()
        .flat_map(|&v| [u16::from(v) * 257, 0, 65535 - u16::from(v) * 257, 65535])
        .collect();
    for s in [
        open("x.psd", psd(20, 14, false, Some(true))),
        open("x.ff", lg::farbfeld(w as u32, h as u32, &px16)),
    ] {
        trim::assert_trims(&*s, &DecodeRequest::full(2));
    }
}
