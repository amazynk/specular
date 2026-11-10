//! TIFF, DDS, KTX and KTX2 through the registry (fixtures built here).

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Source};
use specular_decode::{
    DecodeRequest, Format, PageLabel, Rect, Registry, open_texture_page, open_tiff_page,
};

/// Little-endian classic TIFF, uncompressed RGB8 strips of `rps` rows; one IFD per page.
fn tiff(pages: &[(u32, u32)], rps: u32, orientation: u16) -> Vec<u8> {
    tiff_with(pages, rps, orientation, false)
}

/// As [`tiff`], optionally PackBits-compressed (literal runs, row by row).
fn tiff_with(pages: &[(u32, u32)], rps: u32, orientation: u16, packbits: bool) -> Vec<u8> {
    let mut d = b"II*\0\0\0\0\0".to_vec();
    let mut prev_next = 4usize;
    for (pi, &(w, h)) in pages.iter().enumerate() {
        // Pixel data first, strip by strip.
        let strips = h.div_ceil(rps);
        let mut spans = Vec::new();
        for s in 0..strips {
            let start = d.len();
            for y in s * rps..(s * rps + rps).min(h) {
                let row: Vec<u8> = (0..w)
                    .flat_map(|x| [(x * 3 + pi as u32 * 50) as u8, (y * 5) as u8, (x ^ y) as u8])
                    .collect();
                if packbits {
                    for run in row.chunks(128) {
                        d.push(run.len() as u8 - 1);
                        d.extend_from_slice(run);
                    }
                } else {
                    d.extend_from_slice(&row);
                }
            }
            spans.push((start as u32, (d.len() - start) as u32));
        }
        let offs_off = d.len();
        for &(o, _) in &spans {
            d.extend_from_slice(&o.to_le_bytes());
        }
        let counts_off = d.len();
        for &(_, c) in &spans {
            d.extend_from_slice(&c.to_le_bytes());
        }
        let bps_off = d.len();
        d.extend_from_slice(&[8, 0, 8, 0, 8, 0]);
        if d.len() % 2 == 1 {
            d.push(0);
        }
        let ifd = d.len();
        d[prev_next..prev_next + 4].copy_from_slice(&(ifd as u32).to_le_bytes());
        let (so, sc) = if strips == 1 {
            spans[0]
        } else {
            (offs_off as u32, counts_off as u32)
        };
        let tags: [(u16, u16, u32, u32); 10] = [
            (256, 4, 1, w),
            (257, 4, 1, h),
            (258, 3, 3, bps_off as u32),
            (259, 3, 1, if packbits { 32773 } else { 1 }),
            (262, 3, 1, 2),
            (273, 4, strips, so),
            (274, 3, 1, u32::from(orientation)),
            (277, 3, 1, 3),
            (278, 4, 1, rps),
            (279, 4, strips, sc),
        ];
        d.extend_from_slice(&(tags.len() as u16).to_le_bytes());
        for (tag, ty, n, v) in tags {
            d.extend_from_slice(&tag.to_le_bytes());
            d.extend_from_slice(&ty.to_le_bytes());
            d.extend_from_slice(&n.to_le_bytes());
            if ty == 3 && n == 1 {
                d.extend_from_slice(&(v as u16).to_le_bytes());
                d.extend_from_slice(&[0, 0]);
            } else {
                d.extend_from_slice(&v.to_le_bytes());
            }
        }
        prev_next = d.len();
        d.extend_from_slice(&[0; 4]);
    }
    d
}

fn open(name: &str, bytes: Vec<u8>) -> Arc<dyn specular_decode::Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, bytes)))
        .unwrap()
}

fn rgba_px(r: &specular_core::Raster, x: u32, y: u32) -> &[u8] {
    let p = &r.planes[0];
    let bpp = p.bytes_per_pixel();
    let (x, y) = ((x - r.origin.0) as usize, (y - r.origin.1) as usize);
    &p.data[y * p.stride + x * bpp..][..bpp]
}

#[test]
fn tiff_pages_regions_scales() {
    let bytes = tiff(&[(70, 50), (20, 10), (9, 9)], 8, 6);
    let s = open("x.tif", bytes.clone());
    let i = s.info();
    assert_eq!(i.format, Format::Tiff);
    assert_eq!((i.width, i.height), (70, 50));
    assert_eq!(i.orientation, Orientation::from_exif(6));
    assert!(i.fast_regions);
    assert_eq!(s.page_count(), 3);
    assert!(i.coding.contains("page 1 of 3"), "{}", i.coding);

    let c = CancelToken::new();
    let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(
        (full.width, full.height, full.layout),
        (70, 50, PixelLayout::Rgba)
    );
    assert_eq!(rgba_px(&full, 10, 7), [30, 35, 10 ^ 7, 255]);
    let r = Rect {
        x: 13,
        y: 9,
        w: 30,
        h: 21,
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
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            assert_eq!(rgba_px(&part, x, y), rgba_px(&full, x, y));
        }
    }
    let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
    assert_eq!((half.width, half.height), (35, 25));
    assert!(s.decode_memory(&DecodeRequest::full(1)) >= 70 * 50 * 4);

    let p2 = open_tiff_page(Arc::new(Source::from_vec("x.tif", bytes)), 1).unwrap();
    assert_eq!((p2.info().width, p2.info().height), (20, 10));
    let d = p2.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(rgba_px(&d, 0, 0), [50, 0, 0, 255]);
    // open_page reuses the parsed IFD chain.
    assert_eq!((s.page_index(), s.page_label()), (0, None));
    let q = s.open_page(1).unwrap();
    assert_eq!((q.page_index(), q.page_count()), (1, 3));
    assert_eq!((q.info().width, q.info().height), (20, 10));
    let d = q.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(rgba_px(&d, 0, 0), [50, 0, 0, 255]);
    assert_eq!(q.open_page(2).unwrap().info().width, 9);
    assert!(s.open_page(3).is_err());
}

#[test]
fn tiff_large_strips_cache_and_crop() {
    // A tall uncompressed strip is read as small virtual strips straight from the file.
    let raw = open("y.tiff", tiff(&[(64, 600)], 600, 1));
    assert!(raw.info().fast_regions);
    // A tall compressed strip must be decoded whole: the level is cached, regions cropped.
    let s = open("y.tiff", tiff_with(&[(64, 600)], 600, 1, true));
    assert!(!s.info().fast_regions);
    let c = CancelToken::new();
    let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
    let r = Rect {
        x: 5,
        y: 300,
        w: 20,
        h: 40,
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
    assert_eq!(rgba_px(&part, 7, 333), rgba_px(&full, 7, 333));
    assert!(s.memory_bytes() > 0);
    let c = CancelToken::new();
    let raw_part = raw
        .decode(
            &DecodeRequest {
                region: Some(r),
                ..DecodeRequest::full(1)
            },
            &c,
        )
        .unwrap();
    assert_eq!(rgba_px(&raw_part, 7, 333), rgba_px(&full, 7, 333));
    assert_eq!(raw.memory_bytes(), 0);
}

#[test]
fn dds_and_ktx2_through_registry() {
    // DDS: DX9 RGBA8 masks, 8×4, two mips.
    let mut d = vec![0u8; 128];
    let put = |d: &mut Vec<u8>, p: usize, v: u32| d[p..p + 4].copy_from_slice(&v.to_le_bytes());
    d[..4].copy_from_slice(b"DDS ");
    put(&mut d, 4, 124);
    put(&mut d, 8, 0x2_1007);
    put(&mut d, 12, 4);
    put(&mut d, 16, 8);
    put(&mut d, 28, 2);
    put(&mut d, 76, 32);
    put(&mut d, 80, 0x41);
    put(&mut d, 88, 32);
    for (k, m) in [0xffu32, 0xff00, 0xff_0000, 0xff00_0000].iter().enumerate() {
        put(&mut d, 92 + 4 * k, *m);
    }
    d.extend_from_slice(&[1, 2, 3, 255].repeat(32));
    d.extend_from_slice(&[9, 8, 7, 255].repeat(8));
    let s = open("t.dds", d);
    assert_eq!(s.info().format, Format::Dds);
    assert!(s.info().has_alpha);
    let c = CancelToken::new();
    let r = s.decode(&DecodeRequest::full(2), &c).unwrap();
    assert_eq!((r.width, r.height), (4, 2));
    assert_eq!(rgba_px(&r, 3, 1), [9, 8, 7, 255]);

    // KTX2: R8G8B8A8_UNORM 2×2, bottom-up rows.
    let mut k = vec![
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    let kv = b"KTXorientation\0ru\0";
    let kvd_off = 80 + 24;
    for v in [
        37u32,
        1,
        2,
        2,
        0,
        0,
        1,
        1,
        0,
        0,
        0,
        kvd_off,
        4 + kv.len() as u32,
    ] {
        k.extend_from_slice(&v.to_le_bytes());
    }
    k.extend_from_slice(&[0; 16]);
    let data_off = 128u64;
    for v in [data_off, 16, 16] {
        k.extend_from_slice(&v.to_le_bytes());
    }
    k.extend_from_slice(&(kv.len() as u32).to_le_bytes());
    k.extend_from_slice(kv);
    k.resize(data_off as usize, 0);
    k.extend_from_slice(&[10, 20, 30, 40].repeat(4));
    let s = open("t.ktx2", k);
    assert_eq!(s.info().format, Format::Ktx2);
    assert_eq!(s.info().orientation, Orientation::from_exif(4));
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(rgba_px(&r, 1, 1), [10, 20, 30, 40]);
}

#[test]
fn texture_array_layers_are_pages() {
    // KTX (version 1): RGBA8 2×1, three array layers, one colour each.
    let mut k = vec![
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x31, 0x31, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    for v in [
        0x0403_0201u32,
        0x1401,
        1,
        0x1908,
        0x8058,
        0x1908,
        2,
        1,
        0,
        3,
        1,
        1,
        0,
    ] {
        k.extend_from_slice(&v.to_le_bytes());
    }
    k.extend_from_slice(&24u32.to_le_bytes());
    for layer in 0..3u8 {
        k.extend_from_slice(&[layer * 10, 1, 2, 255].repeat(2));
    }
    let s = open("a.ktx", k.clone());
    assert_eq!(s.info().format, Format::Ktx);
    assert_eq!(s.page_count(), 3);
    let c = CancelToken::new();
    assert_eq!(
        rgba_px(&s.decode(&DecodeRequest::full(1), &c).unwrap(), 1, 0),
        [0, 1, 2, 255]
    );
    let p = open_texture_page(Arc::new(Source::from_vec("a.ktx", k.clone())), 2).unwrap();
    assert!(p.info().coding.contains("layer 3"), "{}", p.info().coding);
    let r = p.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(rgba_px(&r, 1, 0), [20, 1, 2, 255]);
    assert!(open_texture_page(Arc::new(Source::from_vec("a.ktx", k)), 3).is_err());
    let layer = |l| {
        Some(PageLabel::Texture {
            layer: Some(l),
            face: None,
            slice: None,
        })
    };
    assert_eq!((s.page_index(), s.page_label()), (0, layer(0)));
    let q = s.open_page(2).unwrap();
    assert_eq!((q.page_index(), q.page_label()), (2, layer(2)));
    assert_eq!(
        rgba_px(&q.decode(&DecodeRequest::full(1), &c).unwrap(), 1, 0),
        [20, 1, 2, 255]
    );
    assert!(s.open_page(3).is_err());
}

#[path = "common/trim.rs"]
mod trim;

/// zlib stream of stored blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let n = data.len() as u16;
    z.push(1);
    z.extend_from_slice(&n.to_le_bytes());
    z.extend_from_slice(&(!n).to_le_bytes());
    z.extend_from_slice(data);
    let (mut a, mut b) = (1u32, 0u32);
    for &v in data {
        a = (a + u32::from(v)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    z
}

/// Trimming drops the levels cached for regions (a compressed TIFF strip, a zlib-supercompressed
/// KTX2); the next region decodes them again.
#[test]
fn cached_levels_trimmed() {
    let region = |w, h| DecodeRequest {
        region: Some(Rect { x: 0, y: 0, w, h }),
        ..DecodeRequest::full(1)
    };
    let s = open("y.tiff", tiff_with(&[(64, 600)], 600, 1, true));
    trim::assert_trims(&*s, &region(20, 40));

    // KTX2: R8G8B8A8_UNORM 2×2, one zlib (scheme 3) level.
    let pixels = [10u8, 20, 30, 40].repeat(4);
    let level = zlib_stored(&pixels);
    let mut k = vec![
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    for v in [37u32, 1, 2, 2, 0, 0, 1, 1, 3, 0, 0, 0, 0] {
        k.extend_from_slice(&v.to_le_bytes());
    }
    k.extend_from_slice(&[0; 16]);
    let data_off = 128u64;
    for v in [data_off, level.len() as u64, pixels.len() as u64] {
        k.extend_from_slice(&v.to_le_bytes());
    }
    k.resize(data_off as usize, 0);
    k.extend_from_slice(&level);
    let s = open("t.ktx2", k);
    assert!(!s.info().fast_regions);
    trim::assert_trims(&*s, &region(1, 1));
}
