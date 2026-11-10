//! Registry wiring of the specular-bitmap formats: probing, extension fallback (TGA), scaled and
//! region decodes, and animation (GIF, ANI). Every fixture is generated here.

#![allow(clippy::type_complexity)]

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Format, PageLabel, Rect, Registry};
use std::sync::Arc;
use std::time::Duration;

fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// GIF image data with min code size 2, a clear code before every pixel (so codes stay 3 bits).
fn lzw(px: &[u8]) -> Vec<u8> {
    let mut codes = Vec::new();
    for &p in px {
        codes.extend([4u32, p as u32]);
    }
    codes.push(5);
    let (mut bits, mut n, mut out) = (0u32, 0, Vec::new());
    for c in codes {
        bits |= c << n;
        n += 3;
        while n >= 8 {
            out.push(bits as u8);
            bits >>= 8;
            n -= 8;
        }
    }
    if n > 0 {
        out.push(bits as u8);
    }
    let mut v = vec![2u8];
    for c in out.chunks(255) {
        v.push(c.len() as u8);
        v.extend_from_slice(c);
    }
    v.push(0);
    v
}

/// 4×4 GIF, 4-colour palette, frames: (left, top, w, h, disposal, transparent, delay cs, index).
fn gif(frames: &[(u16, u16, u16, u16, u8, Option<u8>, u16, u8)], loops: Option<u16>) -> Vec<u8> {
    let mut v = b"GIF89a".to_vec();
    le16(&mut v, 4);
    le16(&mut v, 4);
    v.extend_from_slice(&[0x81, 0, 0]);
    v.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    if let Some(l) = loops {
        v.extend_from_slice(b"\x21\xff\x0bNETSCAPE2.0\x03\x01");
        le16(&mut v, l);
        v.push(0);
    }
    for &(l, t, w, h, disp, tr, delay, idx) in frames {
        v.extend_from_slice(&[0x21, 0xf9, 4, disp << 2 | tr.is_some() as u8]);
        le16(&mut v, delay);
        v.extend_from_slice(&[tr.unwrap_or(0), 0, 0x2c]);
        for x in [l, t, w, h] {
            le16(&mut v, x);
        }
        v.push(0);
        v.extend(lzw(&vec![idx; w as usize * h as usize]));
    }
    v.push(0x3b);
    v
}

fn open(name: &str, data: Vec<u8>) -> Arc<dyn specular_decode::Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec(name, data)))
        .unwrap()
}

fn px(r: &specular_core::Raster, x: u32, y: u32) -> [u8; 4] {
    let p = &r.planes[0];
    let i = y as usize * p.stride + x as usize * 4;
    p.data[i..i + 4].try_into().unwrap()
}

#[test]
fn gif_animation_through_registry() {
    let data = gif(
        &[
            (0, 0, 4, 4, 1, None, 0, 0),     // red, keep
            (1, 1, 2, 2, 2, None, 5, 1),     // green square, then background
            (0, 0, 2, 2, 3, Some(2), 10, 2), // fully transparent pixels, restore previous
            (2, 2, 2, 2, 1, None, 7, 3),     // white
        ],
        Some(2),
    );
    let s = open("a.gif", data);
    assert_eq!(s.info().format, Format::Gif);
    let a = s.animation().expect("animated");
    assert_eq!((a.frame_count, a.loop_count), (Some(4), 3));
    let still = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(px(&still, 3, 3), [255, 0, 0, 255]);
    let mut f = s.frames(1).unwrap();
    let c = CancelToken::new();
    let mut got = Vec::new();
    while let Some(fr) = f.next_frame(&c) {
        let fr = fr.unwrap();
        got.push((
            fr.index,
            fr.delay,
            px(&fr.raster, 1, 1),
            px(&fr.raster, 3, 3),
        ));
    }
    let ms = Duration::from_millis;
    assert_eq!(
        got,
        [
            (0, ms(0), [255, 0, 0, 255], [255, 0, 0, 255]),
            (1, ms(50), [0, 255, 0, 255], [255, 0, 0, 255]),
            (2, ms(100), [0, 0, 0, 0], [255, 0, 0, 255]),
            (3, ms(70), [0, 0, 0, 0], [255, 255, 255, 255]),
        ]
    );
    f.rewind().unwrap();
    assert_eq!(f.next_frame(&c).unwrap().unwrap().index, 0);
    // Scaled frames.
    let mut f = s.frames(2).unwrap();
    let r = f.next_frame(&c).unwrap().unwrap().raster;
    assert_eq!((r.width, r.height, r.scale), (2, 2, 2));
    // A cancelled token stops frame decoding.
    let cancelled = CancelToken::new();
    cancelled.cancel();
    assert!(f.next_frame(&cancelled).unwrap().is_err());
}

fn tga(w: u16, h: u16) -> Vec<u8> {
    let mut v = vec![0u8, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    le16(&mut v, w);
    le16(&mut v, h);
    v.extend_from_slice(&[24, 0x20]);
    for y in 0..h {
        for x in 0..w {
            v.extend_from_slice(&[x as u8, y as u8, 200]);
        }
    }
    v
}

#[test]
fn tga_by_extension_and_regions() {
    let reg = Registry::default();
    assert!(reg.supports_extension("x.TGA".as_ref()));
    for e in [
        "gif", "bmp", "dib", "ico", "cur", "ani", "qoi", "pnm", "pbm", "pgm", "ppm", "pam",
    ] {
        assert!(reg.supports_extension(format!("f.{e}").as_ref()), "{e}");
    }
    let data = tga(40, 30);
    // Without the extension nothing recognises it.
    assert!(
        reg.open(Arc::new(Source::from_vec("x.bin", data.clone())))
            .is_err()
    );
    let s = open("x.tga", data);
    assert_eq!(s.info().format, Format::Tga);
    assert!(s.info().fast_regions);
    let c = CancelToken::new();
    let full = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(full.layout, PixelLayout::Rgba);
    assert_eq!(px(&full, 5, 7), [200, 7, 5, 255]);
    let req = DecodeRequest {
        scale: 1,
        region: Some(Rect {
            x: 5,
            y: 7,
            w: 10,
            h: 3,
        }),
        parallel: true,
        prepare_regions: false,
    };
    let r = s.decode(&req, &c).unwrap();
    assert_eq!((r.origin, r.width, r.height), ((5, 7), 10, 3));
    assert_eq!(px(&r, 0, 0), [200, 7, 5, 255]);
    let half = s.decode(&DecodeRequest::full(2), &c).unwrap();
    assert_eq!((half.width, half.height, half.scale), (20, 15, 2));
    assert!(s.open_memory(&[8, 2, 1]) > 0);
}

#[test]
fn pnm_gray_and_sequential_cache() {
    let mut data = b"P2\n3 2\n255\n".to_vec();
    data.extend_from_slice(b"0 128 255\n10 20 30\n");
    let s = open("g.pgm", data);
    assert!(!s.info().fast_regions);
    let c = CancelToken::new();
    let r = s.decode(&DecodeRequest::full(1), &c).unwrap();
    assert_eq!(r.layout, PixelLayout::Gray);
    assert_eq!(r.planes[0].data, [0, 128, 255, 10, 20, 30]);
    let req = DecodeRequest {
        scale: 1,
        region: Some(Rect {
            x: 1,
            y: 1,
            w: 2,
            h: 1,
        }),
        parallel: false,
        prepare_regions: false,
    };
    let reg = s.decode(&req, &c).unwrap();
    // Crops of cached levels are aligned to 16 (origin says where).
    assert_eq!(reg.origin, (0, 0));
    assert!(s.memory_bytes() >= 6);
}

fn ico(w: u8, h: u8, color: [u8; 4]) -> Vec<u8> {
    let mut v = vec![0, 0, 2, 0, 1, 0, w, h, 0, 0, 0, 0, 32, 0];
    let mut dib = Vec::new();
    le32(&mut dib, 40);
    le32(&mut dib, w as u32);
    le32(&mut dib, h as u32 * 2);
    le16(&mut dib, 1);
    le16(&mut dib, 32);
    dib.extend_from_slice(&[0; 24]);
    for _ in 0..w as usize * h as usize {
        dib.extend_from_slice(&[color[2], color[1], color[0], color[3]]);
    }
    dib.extend(vec![0u8; (w as usize).div_ceil(32) * 4 * h as usize]);
    le32(&mut v, dib.len() as u32);
    le32(&mut v, 22);
    v.extend(dib);
    v
}

#[test]
fn ani_through_registry() {
    fn chunk(id: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        le32(&mut v, body.len() as u32);
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }
    let mut anih = Vec::new();
    for x in [36u32, 2, 3, 0, 0, 0, 0, 30, 1] {
        le32(&mut anih, x);
    }
    let mut fram = b"fram".to_vec();
    fram.extend(chunk(b"icon", &ico(8, 8, [255, 0, 0, 255])));
    fram.extend(chunk(b"icon", &ico(8, 8, [0, 0, 255, 128])));
    let seq: Vec<u8> = [1u32, 0, 1].iter().flat_map(|x| x.to_le_bytes()).collect();
    let mut body = b"ACON".to_vec();
    body.extend(chunk(b"anih", &anih));
    body.extend(chunk(b"seq ", &seq));
    body.extend(chunk(b"LIST", &fram));
    let s = open("busy.ani", chunk(b"RIFF", &body));
    let a = s.animation().unwrap();
    assert_eq!((a.frame_count, a.loop_count), (Some(3), 0));
    assert_eq!((s.info().width, s.info().height), (8, 8));
    let still = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(px(&still, 0, 0), [0, 0, 255, 128]);
    let mut f = s.frames(1).unwrap();
    let c = CancelToken::new();
    let mut got = Vec::new();
    while let Some(fr) = f.next_frame(&c) {
        let fr = fr.unwrap();
        got.push((fr.delay, px(&fr.raster, 7, 7)));
    }
    assert_eq!(
        got,
        [
            (Duration::from_millis(500), [0, 0, 255, 128]),
            (Duration::from_millis(500), [255, 0, 0, 255]),
            (Duration::from_millis(500), [0, 0, 255, 128]),
        ]
    );
}

#[test]
fn malformed_inputs_do_not_panic() {
    let files = [
        gif(
            &[(0, 0, 4, 4, 1, None, 0, 0), (1, 1, 2, 2, 3, None, 5, 1)],
            None,
        ),
        tga(9, 9),
        ico(4, 4, [1, 2, 3, 4]),
    ];
    let reg = Registry::default();
    for (f, name) in files.iter().zip(["a.gif", "a.tga", "a.ico"]) {
        for n in 0..f.len() {
            let Ok(s) = reg.open(Arc::new(Source::from_vec(name, f[..n].to_vec()))) else {
                continue;
            };
            let c = CancelToken::new();
            let _ = s.decode(&DecodeRequest::full(2), &c);
            if let Ok(mut fr) = s.frames(1) {
                while let Some(Ok(_)) = fr.next_frame(&c) {}
            }
        }
    }
}

#[path = "common/trim.rs"]
mod trim;

/// Trimming drops a sequential format's cached level; the next request decodes it again.
#[test]
fn sequential_level_trimmed() {
    let mut data = b"P2\n3 2\n255\n".to_vec();
    data.extend_from_slice(b"0 128 255\n10 20 30\n");
    let s = open("g.pgm", data);
    let req = DecodeRequest {
        region: Some(Rect {
            x: 1,
            y: 1,
            w: 2,
            h: 1,
        }),
        ..DecodeRequest::full(1)
    };
    trim::assert_trims(&*s, &req);
}

/// Trimming drops a GIF's first-frame indices (1 byte per pixel, counted in `memory_bytes`);
/// the next region runs the frame's LZW again, and playback is unaffected.
#[test]
fn gif_first_frame_trimmed() {
    let data = gif(
        &[(0, 0, 4, 4, 1, None, 0, 1), (1, 1, 2, 2, 1, None, 5, 2)],
        None,
    );
    let s = open("t.gif", data);
    let req = DecodeRequest {
        region: Some(Rect {
            x: 1,
            y: 0,
            w: 3,
            h: 2,
        }),
        ..DecodeRequest::full(1)
    };
    trim::assert_trims(&*s, &req);
    assert_eq!(s.memory_bytes(), 16);
    s.trim_memory();
    let c = CancelToken::new();
    let mut f = s.frames(1).unwrap();
    let a = f.next_frame(&c).unwrap().unwrap().raster;
    let b = f.next_frame(&c).unwrap().unwrap().raster;
    assert_eq!(
        (px(&a, 1, 1), px(&b, 1, 1)),
        ([0, 255, 0, 255], [0, 0, 255, 255])
    );
    assert_eq!(s.memory_bytes(), 0, "playback decodes its own frames");
}

/// A 32- or 24-bit icon entry of one colour, with its AND mask.
fn ico_dib(w: u8, h: u8, bpp: u16, color: [u8; 4]) -> Vec<u8> {
    let mut dib = Vec::new();
    le32(&mut dib, 40);
    le32(&mut dib, w as u32);
    le32(&mut dib, h as u32 * 2);
    le16(&mut dib, 1);
    le16(&mut dib, bpp);
    dib.extend_from_slice(&[0; 24]);
    let row = (w as usize * bpp as usize / 8).div_ceil(4) * 4;
    for _ in 0..h {
        let mut r = Vec::new();
        for _ in 0..w {
            r.extend_from_slice(&[color[2], color[1], color[0]]);
            if bpp == 32 {
                r.push(color[3]);
            }
        }
        r.resize(row, 0);
        dib.extend(r);
    }
    dib.extend(vec![0u8; (w as usize).div_ceil(32) * 4 * h as usize]);
    dib
}

/// An icon file of `entries` in directory order.
fn ico_file(entries: &[(u8, u8, Vec<u8>)]) -> Vec<u8> {
    let mut v = vec![0, 0, 1, 0];
    le16(&mut v, entries.len() as u16);
    let mut off = 6 + 16 * entries.len();
    for (w, h, d) in entries {
        v.extend_from_slice(&[*w, *h, 0, 0, 1, 0, 32, 0]);
        le32(&mut v, d.len() as u32);
        le32(&mut v, off as u32);
        off += d.len();
    }
    for (.., d) in entries {
        v.extend_from_slice(d);
    }
    v
}

#[test]
fn ico_entries_are_pages() {
    let f = ico_file(&[
        (8, 8, ico_dib(8, 8, 32, [255, 0, 0, 255])),
        (16, 16, ico_dib(16, 16, 24, [0, 255, 0, 255])),
        (16, 16, ico_dib(16, 16, 32, [0, 0, 255, 128])),
    ]);
    let s = open("a.ico", f);
    assert_eq!(s.info().format, Format::Ico);
    assert_eq!((s.page_count(), s.page_index()), (3, 0));
    let label = |width, height, bits| {
        Some(PageLabel::Image {
            width,
            height,
            bits: Some(bits),
        })
    };
    // Largest first, deepest first among equals: the default image is page 0.
    assert_eq!(s.page_label(), label(16, 16, 32));
    let full = |s: &dyn specular_decode::Session| {
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap()
    };
    let r = full(&*s);
    assert_eq!(
        (r.width, r.height, px(&r, 3, 3)),
        (16, 16, [0, 0, 255, 128])
    );
    let p1 = s.open_page(1).unwrap();
    assert_eq!((p1.page_index(), p1.page_count()), (1, 3));
    assert_eq!(p1.page_label(), label(16, 16, 24));
    assert_eq!(px(&full(&*p1), 5, 5), [0, 255, 0, 255]);
    // Stepping from any page reaches any other.
    let p2 = p1.open_page(2).unwrap();
    assert_eq!(p2.page_label(), label(8, 8, 32));
    let r = full(&*p2);
    assert_eq!((r.width, px(&r, 0, 0)), (8, [255, 0, 0, 255]));
    assert_eq!(p2.info().coding, s.info().coding);
    assert!(p2.open_page(0).unwrap().page_label() == label(16, 16, 32));
    assert!(s.open_page(3).is_err());
    // One entry: one page. An animated cursor has frames, not pages.
    let one = open("b.ico", ico(4, 4, [1, 2, 3, 4]));
    assert_eq!((one.page_count(), one.page_label()), (1, label(4, 4, 32)));
}

fn conformance() -> std::path::PathBuf {
    std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/conformance")
        })
}

/// Real icons and cursors: every page of a file that opens opens too, labelled with the size
/// it decodes to.
#[test]
fn real_ico_pages() {
    let base = conformance();
    let names = [
        "image-rs/tests/images/ico/images/multiple_entries_with_different_bit_depth.ico",
        "image-rs/tests/images/ico/images/two-entry-order-test.ico",
        "pillow/Tests/images/hopper.ico",
        "pillow/Tests/images/python.ico",
        "sdl_image/test/sample.ico",
        "sdl_image/test/sample.cur",
    ];
    let mut multi = 0;
    for n in names {
        let path = base.join(n);
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let s = open(n, data);
        let count = s.page_count();
        multi += usize::from(count > 1);
        let mut last = u64::MAX;
        for i in 0..count {
            let p = s.open_page(i).unwrap();
            let Some(PageLabel::Image {
                width,
                height,
                bits: Some(bits),
            }) = p.page_label()
            else {
                panic!("{n} page {i}: no label")
            };
            assert!(matches!(bits, 1 | 4 | 8 | 16 | 24 | 32 | 64), "{n}: {bits}");
            let px = u64::from(width) * u64::from(height);
            assert!(px <= last, "{n}: pages not largest first");
            last = px;
            let r = p
                .decode(&DecodeRequest::full(1), &CancelToken::new())
                .unwrap();
            assert_eq!((r.width, r.height), (width, height), "{n} page {i}");
            assert_eq!((p.info().width, p.info().height), (width, height));
        }
        eprintln!("{n}: {count} pages");
    }
    if multi == 0 {
        eprintln!("no multi-image icons in the conformance corpus; skipped");
    }
}

#[test]
fn pnm_images_are_pages() {
    let mut f = b"P6\n2 1\n255\n".to_vec();
    f.extend_from_slice(&[255, 0, 0, 0, 255, 0]);
    f.extend_from_slice(b"\n\nP5 3 1 255\n");
    f.extend_from_slice(&[1, 2, 3]);
    f.extend_from_slice(
        b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
    );
    f.extend_from_slice(&[9, 8, 7, 6]);
    // A plain image ends the scan: what follows it is not looked for.
    f.extend_from_slice(b"P2 1 1 255 77\nP5 1 1 255\n\x05");
    let s = open("m.ppm", f.clone());
    assert_eq!(s.info().format, Format::Pnm);
    assert_eq!((s.page_count(), s.page_index()), (4, 0));
    let label = |width, height| {
        Some(PageLabel::Image {
            width,
            height,
            bits: None,
        })
    };
    assert_eq!(s.page_label(), label(2, 1));
    assert!(
        s.info().coding.contains("image 1 of 4"),
        "{}",
        s.info().coding
    );
    let full = |s: &dyn specular_decode::Session| {
        s.decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap()
    };
    let r = full(&*s);
    assert_eq!(
        (px(&r, 0, 0), px(&r, 1, 0)),
        ([255, 0, 0, 255], [0, 255, 0, 255])
    );
    let p1 = s.open_page(1).unwrap();
    assert_eq!((p1.page_index(), p1.page_label()), (1, label(3, 1)));
    let r = full(&*p1);
    assert_eq!(
        (r.layout, &r.planes[0].data[..3]),
        (PixelLayout::Gray, &[1, 2, 3][..])
    );
    let p2 = p1.open_page(2).unwrap();
    assert_eq!(p2.page_label(), label(1, 1));
    assert_eq!(px(&full(&*p2), 0, 0), [9, 8, 7, 6]);
    let p3 = p2.open_page(3).unwrap();
    assert_eq!(full(&*p3).planes[0].data[0], 77);
    assert!(p3.open_page(4).is_err());
    assert_eq!(p3.open_page(0).unwrap().page_label(), label(2, 1));

    // One image (trailing whitespace or junk is not another): no pages.
    let one = open("o.pgm", b"P5 1 1 255\n\x07\n\n".to_vec());
    assert_eq!((one.page_count(), one.page_label()), (1, None));
    let junk = open("j.pgm", b"P5 1 1 255\n\x07P9 junk".to_vec());
    assert_eq!(junk.page_count(), 1);

    // A last image cut short is a page that decodes as truncated (or fails), never panics;
    // every prefix of the file opens and decodes its pages without panicking.
    let cut = open("c.pgm", b"P5 1 1 255\n\x07P5 4 4 255\n\x01\x02".to_vec());
    assert_eq!(cut.page_count(), 2);
    if let Ok(r) = cut
        .open_page(1)
        .unwrap()
        .decode(&DecodeRequest::full(1), &CancelToken::new())
    {
        assert!(r.truncated);
    }
    for n in 3..f.len() {
        let Ok(s) = Registry::default().open(Arc::new(Source::from_vec("m.ppm", f[..n].to_vec())))
        else {
            continue;
        };
        let _ = s.decode(&DecodeRequest::full(1), &CancelToken::new());
        for page in 1..s.page_count() {
            let p = s.open_page(page).unwrap();
            let _ = p.decode(&DecodeRequest::full(1), &CancelToken::new());
        }
    }

    // Many tiny images: the scan reads headers only and stops at the cap.
    let mut many = Vec::new();
    for i in 0..5000u32 {
        many.extend_from_slice(b"P5 1 1 255\n");
        many.push(i as u8);
    }
    let t = std::time::Instant::now();
    let s = open("many.pgm", many);
    assert_eq!(s.page_count(), 4096);
    assert!(t.elapsed() < Duration::from_secs(1));
    let last = s.open_page(4095).unwrap();
    assert_eq!(full(&*last).planes[0].data[0], (4095 % 256) as u8);
}

/// From the fidelity corpus (BMP Suite's `q/rgb24jpeg.bmp`): BI_JPEG pixels are a whole JPEG file,
/// shown through the JPEG decoder under the BMP's name.
#[test]
fn bmp_with_embedded_jpeg() {
    let rgb: Vec<u8> = [200u8, 40, 90].repeat(16 * 8);
    let mut jpeg = Vec::new();
    jpeg_encoder::Encoder::new(&mut jpeg, 95)
        .encode(&rgb, 16, 8, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    let mut v = b"BM".to_vec();
    le32(&mut v, (14 + 40 + jpeg.len()) as u32);
    le32(&mut v, 0);
    le32(&mut v, 14 + 40);
    le32(&mut v, 40);
    le32(&mut v, 16);
    le32(&mut v, 8);
    le16(&mut v, 1);
    le16(&mut v, 0);
    le32(&mut v, 4); // BI_JPEG
    le32(&mut v, jpeg.len() as u32);
    v.extend_from_slice(&[0; 16]);
    v.extend_from_slice(&jpeg);
    let s = open("j.bmp", v);
    assert_eq!(s.info().format, Format::Bmp);
    assert_eq!((s.info().width, s.info().height), (16, 8));
    assert!(
        s.info().coding.starts_with("BMP with embedded JPEG"),
        "{}",
        s.info().coding
    );
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!((r.width, r.height), (16, 8));
}
