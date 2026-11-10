//! Decoders against independently computed expected pixels (and the `gif` / `qoi` crates as
//! dev-only oracles), plus scaled/region consistency, animation compositing and robustness.

mod common;
use common::*;
use specular_bitmap::{
    Decode, Options, ani::Ani, bmp::Bmp, gif::Animator, gif::Gif, ico::Entry, ico::Ico, pnm::Pnm,
    qoi::Qoi, tga::Tga,
};
use std::sync::Arc;

const NO: &(dyn Fn() -> bool + Sync) = &|| false;

#[test]
fn bmp_variants() {
    let masks565 = [0xf800, 0x7e0, 0x1f, 0];
    let v5 = [0xff0000, 0xff00, 0xff, 0xff000000];
    let cases: Vec<(&str, BmpSpec)> = vec![
        ("1bpp", BmpSpec::new(1)),
        ("4bpp", BmpSpec::new(4)),
        ("8bpp", BmpSpec::new(8)),
        (
            "8bpp core",
            BmpSpec {
                header: 12,
                ..BmpSpec::new(8)
            },
        ),
        ("24bpp", BmpSpec::new(24)),
        (
            "24bpp top-down",
            BmpSpec {
                top_down: true,
                ..BmpSpec::new(24)
            },
        ),
        ("16bpp 555", BmpSpec::new(16)),
        (
            "16bpp 565",
            BmpSpec {
                comp: 3,
                masks: masks565,
                ..BmpSpec::new(16)
            },
        ),
        ("32bpp", BmpSpec::new(32)),
        (
            "32bpp v5 alpha",
            BmpSpec {
                header: 124,
                comp: 3,
                masks: v5,
                ..BmpSpec::new(32)
            },
        ),
        (
            "32bpp v4 alpha top-down",
            BmpSpec {
                header: 108,
                comp: 3,
                masks: v5,
                top_down: true,
                ..BmpSpec::new(32)
            },
        ),
        (
            "rle8",
            BmpSpec {
                comp: 1,
                ..BmpSpec::new(8)
            },
        ),
        (
            "rle4",
            BmpSpec {
                comp: 2,
                ..BmpSpec::new(4)
            },
        ),
    ];
    for (w, h) in [(1, 1), (13, 7), (67, 45)] {
        for (name, s) in &cases {
            let data = bmp(w, h, *s);
            let d = Bmp::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
            check(
                &d,
                &data,
                &bmp_expected(w, h, *s, false),
                0,
                &format!("bmp {name} {w}x{h}"),
            );
        }
    }
}

#[test]
fn ico_bmp_and_png_entries() {
    // 8-bit with AND mask, 32-bit alpha, and a cursor.
    for (bpp, cursor) in [(8u16, false), (32, false), (4, true), (24, false)] {
        let s = BmpSpec {
            masks: [0; 4],
            ..BmpSpec::new(bpp)
        };
        let small = dib(16, 16, s, true);
        let big = dib(48, 40, s, true);
        let data = ico(&[(small, 16, 16, bpp), (big, 48, 40, bpp)], cursor);
        let i = Ico::parse(&data).unwrap();
        assert_eq!(i.cursor, cursor);
        assert_eq!(i.count, 2);
        let Entry::Bmp(b) = i.entry() else {
            panic!("bmp entry")
        };
        assert_eq!((b.info().width, b.info().height), (48, 40));
        check(
            b.as_ref(),
            &data,
            &bmp_expected(48, 40, s, true),
            0,
            &format!("ico {bpp} cursor {cursor}"),
        );
    }
    let p = png(64, 64);
    let data = ico(
        &[
            (dib(16, 16, BmpSpec::new(32), true), 16, 16, 32),
            (p.clone(), 0, 0, 32),
        ],
        false,
    );
    let Entry::Png {
        range,
        width,
        height,
    } = Ico::parse(&data).unwrap().pages.swap_remove(0).entry
    else {
        panic!("png entry")
    };
    assert_eq!((width, height), (64, 64));
    assert_eq!(&data[range], &p[..]);
}

#[test]
fn ico_pages_largest_first() {
    // Directory order: 16×16 8-bit, 32×32 4-bit, 32×32 32-bit, 16×16 8-bit again, a PNG.
    let e = |w: u32, h: u32, bpp: u16| {
        let s = BmpSpec {
            masks: [0; 4],
            ..BmpSpec::new(bpp)
        };
        (dib(w, h, s, true), w as u8, h as u8, bpp)
    };
    let data = ico(
        &[
            e(16, 16, 8),
            e(32, 32, 4),
            e(32, 32, 32),
            e(16, 16, 8),
            (png(24, 24), 0, 0, 32),
        ],
        false,
    );
    let i = Ico::parse(&data).unwrap();
    assert_eq!(i.count, 5);
    let sizes: Vec<_> = i
        .pages
        .iter()
        .map(|p| (p.width, p.height, p.bits))
        .collect();
    // PNG bits come from IHDR (8-bit RGBA).
    assert_eq!(
        sizes,
        [
            (32, 32, 32),
            (32, 32, 4),
            (24, 24, 32),
            (16, 16, 8),
            (16, 16, 8)
        ]
    );
    assert!(matches!(i.entry(), Entry::Bmp(b) if b.info().width == 32));
    assert!(matches!(i.pages[2].entry, Entry::Png { width: 24, .. }));
    // Each BMP entry decodes as itself.
    for p in &i.pages {
        if let Entry::Bmp(b) = &p.entry {
            assert_eq!((b.info().width, b.info().height), (p.width, p.height));
        }
    }
    // An unusable entry is left out rather than failing the file.
    let mut bad = ico(&[e(16, 16, 8), e(32, 32, 8)], false);
    let off = u32::from_le_bytes(bad[6 + 16 + 12..6 + 16 + 16].try_into().unwrap()) as usize;
    bad[off..off + 4].copy_from_slice(&[1, 0, 0, 0]);
    let i = Ico::parse(&bad).unwrap();
    assert_eq!(i.pages.len(), 1);
    assert_eq!((i.pages[0].width, i.count), (16, 2));
}

#[test]
fn tga_variants() {
    for (w, h) in [(1, 1), (17, 9), (70, 33)] {
        for ty in [1u8, 2, 3, 9, 10, 11] {
            let depths: &[u8] = match ty & 7 {
                1 => &[8],
                2 => &[16, 24, 32],
                _ => &[8, 16],
            };
            for &depth in depths {
                for origin in [0u8, 0x20, 0x10, 0x30] {
                    let data = tga(w, h, ty, depth, origin);
                    let what = format!("tga ty{ty} {depth}bit origin{origin:x} {w}x{h}");
                    let d = Tga::parse(&data).unwrap_or_else(|e| panic!("{what}: {e}"));
                    check(&d, &data, &tga_expected(w, h, ty, depth), 0, &what);
                }
            }
        }
    }
}

/// A 2×1 top-down true-colour TGA with `pixels` and, when `attr` is given, a TGA 2.0
/// extension area carrying that attribute type.
fn tga_alpha(depth: u8, alpha_bits: u8, pixels: &[u8], attr: Option<u8>) -> Vec<u8> {
    // No id, no colour map, uncompressed true colour; 2×1 at the origin; top-down.
    let mut v = [
        &[0u8, 0, 2][..],
        &[0; 9],
        &[2, 0, 1, 0, depth, alpha_bits | 0x20],
    ]
    .concat();
    v.extend_from_slice(pixels);
    if let Some(attr) = attr {
        let ext = v.len() as u32;
        let mut area = vec![0u8; 495];
        area[0..2].copy_from_slice(&495u16.to_le_bytes());
        area[494] = attr;
        v.extend_from_slice(&area);
        v.extend_from_slice(&ext.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(b"TRUEVISION-XFILE.\0");
    }
    v
}

#[test]
fn tga_16bit_attribute_bit_is_alpha() {
    // Descriptor declares 1 alpha bit: white with the bit set, red with it clear.
    let data = tga_alpha(16, 1, &[0xff, 0xff, 0x00, 0x7c], None);
    let d = Tga::parse(&data).unwrap();
    let img = d.decode(&data, &Options::default(), NO).unwrap();
    assert_eq!(img.data, [255, 255, 255, 255, 255, 0, 0, 0]);
    // The bit clear everywhere means a writer that ignores it: opaque.
    let data = tga_alpha(16, 1, &[0xff, 0x7f, 0x00, 0x7c], None);
    let d = Tga::parse(&data).unwrap();
    let img = d.decode(&data, &Options::default(), NO).unwrap();
    assert_eq!(img.data[3], 255);
    assert_eq!(img.data[7], 255);
}

#[test]
fn tga_premultiplied_attribute_is_divided_out() {
    // BGRA: orange (255, 128, 0) at alpha 128 stored premultiplied, then transparent.
    let px = [0, 64, 128, 128, 0, 0, 0, 0];
    let data = tga_alpha(32, 8, &px, Some(4));
    let d = Tga::parse(&data).unwrap();
    let img = d.decode(&data, &Options::default(), NO).unwrap();
    assert_eq!(img.data, [255, 128, 0, 128, 0, 0, 0, 0]);
    // Attribute type 3 (straight) is left alone.
    let data = tga_alpha(32, 8, &px, Some(3));
    let d = Tga::parse(&data).unwrap();
    let img = d.decode(&data, &Options::default(), NO).unwrap();
    assert_eq!(img.data, [128, 64, 0, 128, 0, 0, 0, 0]);
}

#[test]
fn pnm_variants() {
    for (w, h) in [(1, 1), (19, 11), (64, 40)] {
        for (magic, maxval, depth) in [
            (1u8, 1u32, 1u32),
            (4, 1, 1),
            (2, 255, 1),
            (2, 1000, 1),
            (3, 255, 3),
            (3, 15, 3),
            (5, 255, 1),
            (5, 100, 1),
            (5, 65535, 1),
            (6, 255, 3),
            (6, 4095, 3),
            (7, 255, 1),
            (7, 255, 2),
            (7, 255, 3),
            (7, 255, 4),
            (7, 65535, 4),
        ] {
            let data = pnm(w, h, magic, maxval, depth);
            let what = format!("P{magic} maxval {maxval} depth {depth} {w}x{h}");
            let d = Pnm::parse(&data).unwrap_or_else(|e| panic!("{what}: {e}"));
            check(
                &d,
                &data,
                &pnm_expected(w, h, magic, maxval, depth),
                0,
                &what,
            );
        }
    }
}

#[test]
fn qoi_matches_oracle() {
    for (w, h, ch) in [(1u32, 1u32, 4u8), (37, 21, 3), (130, 77, 4), (300, 5, 3)] {
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                // Runs, index hits, small diffs and full colours.
                let p = if (x / 6) % 3 == 0 {
                    pat(x / 6, y)
                } else {
                    pat(x, y / 4)
                };
                px.extend_from_slice(&p[..ch as usize]);
            }
        }
        let data = qoi::encode_to_vec(&px, w, h).unwrap();
        let what = format!("qoi {w}x{h}x{ch}");
        let d = Qoi::parse(&data).unwrap();
        let (_, raw) = qoi::decode_to_vec(&data).unwrap();
        let want: Vec<u8> = if ch == 4 {
            raw
        } else {
            raw.chunks(3)
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect()
        };
        let mine = rgba(&d.decode(&data, &Options::default(), NO).unwrap());
        assert_eq!(mine, want, "{what} vs qoi crate");
        check(&d, &data, &want, 0, &what);
    }
}

fn frame(left: u16, top: u16, w: u16, h: u16, dispose: gif::DisposalMethod, seed: u32) -> GifFrame {
    GifFrame {
        left,
        top,
        w,
        h,
        dispose,
        transparent: None,
        interlaced: false,
        local_palette: false,
        delay: 0,
        seed,
    }
}

/// Frames composited by the reference (over the `gif` crate) and by the [`Animator`] must be
/// identical.
fn compare_animation(data: &[u8], what: &str) {
    let (cw, chh, want) = gif_reference(data);
    let g = Arc::new(Gif::parse(data).unwrap());
    assert_eq!(g.animation().shown.len(), want.len(), "{what}: frame count");
    let mut a = Animator::new(g.clone());
    for pass in 0..2 {
        for (i, (wb, wdelay)) in want.iter().enumerate() {
            let (canvas, delay, trunc) = a.next_frame(data, NO).unwrap().unwrap();
            assert!(!trunc);
            assert_eq!(delay, *wdelay, "{what}: delay {i}");
            assert_eq!((cw, chh), (g.info().width, g.info().height));
            if canvas != wb.as_slice() {
                let p = canvas.iter().zip(wb).position(|(a, b)| a != b).unwrap() / 4;
                panic!(
                    "{what} pass {pass}: frame {i} differs at ({}, {}): {:?} vs {:?}",
                    p as u32 % cw,
                    p as u32 / cw,
                    &canvas[p * 4..p * 4 + 4],
                    &wb[p * 4..p * 4 + 4]
                );
            }
            if i == 0 && pass == 0 {
                // The still decode equals the first composited frame.
                let still = g.decode(data, &Options::default(), NO).unwrap();
                assert_eq!(still.data, canvas, "{what}: still image");
            }
        }
        assert!(a.next_frame(data, NO).is_none());
        a.rewind();
    }
}

#[test]
fn gif_animation_all_disposals() {
    use gif::DisposalMethod::{Any, Background, Keep, Previous};
    let mut frames = vec![
        frame(0, 0, 40, 30, Keep, 0),
        frame(5, 4, 20, 10, Background, 1),
        frame(10, 8, 25, 15, Previous, 2),
        frame(0, 0, 12, 12, Keep, 3),
        frame(30, 20, 10, 10, Previous, 4),
        frame(28, 18, 12, 12, Any, 5),
        frame(2, 2, 36, 26, Background, 6),
        frame(1, 1, 5, 5, Keep, 7),
    ];
    frames[1].transparent = Some(7);
    frames[2].transparent = Some(0);
    frames[2].interlaced = true;
    frames[3].local_palette = true;
    frames[4].transparent = Some(14);
    frames[6].interlaced = true;
    frames[6].local_palette = true;
    frames[6].transparent = Some(3);
    for (i, f) in frames.iter_mut().enumerate() {
        f.delay = [0, 5, 10, 2, 0, 100, 7, 1][i];
    }
    let data = gif_file(40, 30, &frames, Some(0));
    let g = Gif::parse(&data).unwrap();
    assert_eq!(g.animation().loop_count, Some(0));
    assert_eq!(
        g.animation().frames[0].delay_ms(),
        0,
        "delay 0 kept as stored"
    );
    compare_animation(&data, "disposals");

    // Finite loop count, first frame not covering the canvas, every interlace pass height.
    for h in 1..=9u16 {
        let mut fs = vec![
            frame(3, 2, 7, h, Keep, h as u32),
            frame(0, 0, 4, 4, Background, 1),
        ];
        fs[0].interlaced = true;
        fs[0].transparent = Some(2);
        let data = gif_file(12, 12, &fs, Some(3));
        assert_eq!(Gif::parse(&data).unwrap().animation().loop_count, Some(3));
        compare_animation(&data, &format!("interlace h{h}"));
    }
    let data = gif_file(8, 8, &[frame(0, 0, 8, 8, Keep, 0)], None);
    assert_eq!(Gif::parse(&data).unwrap().animation().loop_count, None);
}

/// From the fidelity corpus (the pygif suite's `high-color.gif`, `images-combine.gif` and
/// `animation-multi-image.gif`): an image without a graphic control extension is drawn with
/// the next image, and with no extension and no loop all images make one still picture.
#[test]
fn gif_images_without_control_combine() {
    use gif::DisposalMethod::Keep;
    // Four tiles, no control extensions, no looping: one still frame holding all four.
    let mut tiles = vec![
        frame(0, 0, 8, 8, Keep, 1),
        frame(8, 0, 8, 8, Keep, 2),
        frame(0, 8, 8, 8, Keep, 3),
        frame(8, 8, 8, 8, Keep, 4),
    ];
    for t in &mut tiles {
        t.delay = 1;
    }
    let data = strip_controls(&gif_file(16, 16, &tiles, None));
    let g = Gif::parse(&data).unwrap();
    assert_eq!(
        (g.animation().shown.len(), g.animation().shown[0].clone()),
        (1, 0..4)
    );
    let still = g.decode(&data, &Options::default(), NO).unwrap();
    assert!(
        still.data.chunks_exact(4).all(|p| p[3] == 255),
        "every tile is in the still picture"
    );
    compare_animation(&data, "tiles combine");
    // The same with a looping extension: a forced animation, one image per frame.
    let data = strip_controls(&gif_file(16, 16, &tiles, Some(0)));
    assert_eq!(Gif::parse(&data).unwrap().animation().shown.len(), 4);
    compare_animation(&data, "tiles animate");
    // Whole-canvas images drawn over each other (the suite's gif87a-animation.gif): frames.
    let mut over = vec![frame(0, 0, 16, 16, Keep, 1), frame(0, 0, 16, 16, Keep, 2)];
    for t in &mut over {
        t.delay = 1;
    }
    let data = strip_controls(&gif_file(16, 16, &over, None));
    assert_eq!(Gif::parse(&data).unwrap().animation().shown.len(), 2);
    compare_animation(&data, "overlapping images animate");
    // Mixed: images 1 and 3 without an extension join the image after them; an explicit
    // delay of 0 (image 4) stays a frame of its own.
    let mut fs: Vec<_> = (0..6).map(|i| frame(i as u16, 0, 4, 4, Keep, i)).collect();
    for (f, d) in fs.iter_mut().zip([50, 50, 50, 50, 0, 50]) {
        f.delay = d;
    }
    // A transparent index makes the encoder write image 4's extension despite delay 0.
    fs[4].transparent = Some(255);
    let full = gif_file(10, 4, &fs, Some(0));
    let at = gif_controls(&full);
    let mut data = full.clone();
    for i in [3, 1] {
        let p = at[i].unwrap();
        data.drain(p..p + 8);
    }
    assert_eq!(
        Gif::parse(&data).unwrap().animation().shown,
        vec![0..1, 1..3, 3..5, 5..6]
    );
    compare_animation(&data, "images without a control join the next");
}

#[test]
fn gif_small_palettes_and_scaling() {
    // Minimum code sizes 2..=8 through palette sizes.
    for bits in 1..=8u32 {
        let n = 1usize << bits;
        let pal: Vec<u8> = (0..n)
            .flat_map(|i| [(i * 255 / n.max(2)) as u8, (i * 3) as u8, 200])
            .collect();
        let (w, h) = (53u16, 41u16);
        let buf: Vec<u8> = (0..w as usize * h as usize)
            .map(|i| ((i / 3 + i / 97) % n) as u8)
            .collect();
        let mut out = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut out, w, h, &pal).unwrap();
            let mut f = gif::Frame::from_indexed_pixels(w, h, buf, None);
            f.palette = None;
            enc.write_frame(&f).unwrap();
        }
        let g = Gif::parse(&out).unwrap();
        let want = &gif_reference(&out).2[0].0;
        check(&g, &out, want, 0, &format!("gif {bits}-bit palette"));
    }
}

#[test]
fn ani_cursor() {
    fn chunk(id: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        le32(&mut v, body.len() as u32);
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }
    let frames: Vec<Vec<u8>> = (0..3)
        .map(|i| {
            let s = BmpSpec::new(if i == 1 { 32 } else { 8 });
            ico(&[(dib(32, 32, s, true), 32, 32, s.bpp)], true)
        })
        .collect();
    let mut anih = Vec::new();
    for v in [36u32, 3, 5, 0, 0, 0, 0, 6, 1] {
        le32(&mut anih, v);
    }
    let mut fram = b"fram".to_vec();
    for f in &frames {
        fram.extend_from_slice(&chunk(b"icon", f));
    }
    let seq: Vec<u8> = [0u32, 1, 2, 1, 0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let rate: Vec<u8> = [6u32, 12, 6, 3, 60]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut body = b"ACON".to_vec();
    body.extend(chunk(b"LIST", b"INFOINAM\x04\0\0\0abc\0"));
    body.extend(chunk(b"anih", &anih));
    body.extend(chunk(b"rate", &rate));
    body.extend(chunk(b"seq ", &seq));
    body.extend(chunk(b"LIST", &fram));
    let data = chunk(b"RIFF", &body);
    assert!(specular_bitmap::ani::probe(&data));
    let a = Ani::parse(&data).unwrap();
    assert_eq!(a.frames.len(), 3);
    let st: Vec<(usize, u32)> = a.steps.iter().map(|s| (s.frame, s.jiffies)).collect();
    assert_eq!(st, [(0, 6), (1, 12), (2, 6), (1, 3), (0, 60)]);
    #[allow(clippy::needless_range_loop)]
    for i in 0..3 {
        assert_eq!(a.frame_data(&data, i), &frames[i][..]);
        let ic = Ico::parse(a.frame_data(&data, i)).unwrap();
        assert!(ic.cursor);
    }
    // Without seq/rate: one step per frame at the default rate.
    let mut body = b"ACON".to_vec();
    body.extend(chunk(b"anih", &anih));
    body.extend(chunk(b"LIST", &fram));
    let data = chunk(b"RIFF", &body);
    let st: Vec<(usize, u32)> = Ani::parse(&data)
        .unwrap()
        .steps
        .iter()
        .map(|s| (s.frame, s.jiffies))
        .collect();
    assert_eq!(st, [(0, 6), (1, 6), (2, 6)]);
    for n in 0..data.len() {
        let _ = Ani::parse(&data[..n]);
    }
}

#[test]
fn cancel_is_respected() {
    let data = bmp(600, 600, BmpSpec::new(24));
    let d = Bmp::parse(&data).unwrap();
    let r = d.decode(&data, &Options::default(), &|| true);
    assert!(matches!(r, Err(specular_bitmap::Error::Cancelled)));
    let bad = Options {
        scale: 16,
        ..Options::default()
    };
    assert!(d.decode(&data, &bad, NO).is_err());
}

/// Truncations and byte mutations of every format: never panic; truncated pixel data
/// either errors or decodes with `truncated` set.
#[test]
fn robustness() {
    let mut files: Vec<(&str, Vec<u8>)> = vec![
        ("bmp24", bmp(31, 17, BmpSpec::new(24))),
        ("bmp8", bmp(31, 17, BmpSpec::new(8))),
        (
            "rle8",
            bmp(
                31,
                17,
                BmpSpec {
                    comp: 1,
                    ..BmpSpec::new(8)
                },
            ),
        ),
        (
            "rle4",
            bmp(
                31,
                17,
                BmpSpec {
                    comp: 2,
                    ..BmpSpec::new(4)
                },
            ),
        ),
        (
            "bmp565",
            bmp(
                31,
                17,
                BmpSpec {
                    comp: 3,
                    masks: [0xf800, 0x7e0, 0x1f, 0],
                    ..BmpSpec::new(16)
                },
            ),
        ),
        (
            "ico",
            ico(&[(dib(16, 16, BmpSpec::new(8), true), 16, 16, 8)], false),
        ),
        ("tga10", tga(23, 19, 10, 32, 0)),
        ("tga9", tga(23, 19, 9, 8, 0x20)),
        ("tga3", tga(23, 19, 3, 16, 0)),
        ("p6", pnm(23, 19, 6, 255, 3)),
        ("p3", pnm(23, 19, 3, 1000, 3)),
        ("p7", pnm(23, 19, 7, 65535, 4)),
        ("p4", pnm(23, 19, 4, 1, 1)),
    ];
    let px: Vec<u8> = (0..23 * 19)
        .flat_map(|i: u32| pat(i % 23, i / 23))
        .collect();
    files.push(("qoi", qoi::encode_to_vec(&px, 23, 19).unwrap()));
    let mut fs = vec![
        frame(0, 0, 23, 19, gif::DisposalMethod::Previous, 0),
        frame(3, 3, 9, 9, gif::DisposalMethod::Background, 1),
    ];
    fs[1].interlaced = true;
    files.push(("gif", gif_file(23, 19, &fs, Some(0))));

    fn run(data: &[u8]) -> Option<bool> {
        let opts = [
            Options::default(),
            Options {
                scale: 2,
                ..Options::default()
            },
        ];
        let d: Box<dyn Decode> = if let Ok(d) = Bmp::parse(data) {
            Box::new(d)
        } else if let Ok(d) = Tga::parse(data) {
            Box::new(d)
        } else if let Ok(d) = Pnm::parse(data) {
            Box::new(d)
        } else if let Ok(d) = Qoi::parse(data) {
            Box::new(d)
        } else if let Ok(mut i) = Ico::parse(data) {
            match i.pages.swap_remove(0).entry {
                Entry::Bmp(b) => b,
                Entry::Png { .. } => return None,
            }
        } else if let Ok(g) = Gif::parse(data) {
            let g = Arc::new(g);
            {
                let mut a = Animator::new(g.clone());
                while let Some(r) = a.next_frame(data, NO) {
                    if r.is_err() {
                        break;
                    }
                }
            }
            Box::new(Arc::try_unwrap(g).ok()?)
        } else {
            return None;
        };
        let mut trunc = None;
        for o in &opts {
            if let Ok(img) = d.decode(data, o, NO) {
                assert_eq!(
                    img.data.len(),
                    (img.width * img.height) as usize * img.format.bytes_per_pixel()
                );
                trunc = Some(img.truncated);
            }
        }
        trunc
    }
    let mut seed = 0x1234_5678u32;
    for (name, f) in &files {
        assert_eq!(run(f), Some(false), "{name} intact");
        // Truncation inside the pixel data must be reported (or rejected).
        let n = f.len() * 3 / 4;
        assert_ne!(run(&f[..n]), Some(false), "{name} truncated to {n}");
        for n in 0..f.len() {
            run(&f[..n]);
        }
        for _ in 0..300 {
            let mut m = f.clone();
            for _ in 0..1 + seed % 4 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let i = seed as usize % m.len();
                m[i] = (seed >> 8) as u8;
            }
            run(&m);
        }
    }
}

/// RLE pixels the data never paints (a delta skip, an early end of line or of the bitmap)
/// are transparent, as browsers show them and BMP Suite prefers (q/pal8rlecut.bmp; fidelity
/// they took palette colour 0). A fully painted RLE bitmap stays opaque.
#[test]
fn rle_holes_are_transparent() {
    let file = |rle: &[u8]| {
        let (w, h) = (4i32, 2i32);
        let mut b = Vec::new();
        let off = 14 + 40 + 8;
        b.extend_from_slice(b"BM");
        b.extend_from_slice(&((off + rle.len()) as u32).to_le_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&(off as u32).to_le_bytes());
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes()); // BI_RLE8
        b.extend_from_slice(&(rle.len() as u32).to_le_bytes());
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]); // red, blue (BGRx)
        b.extend_from_slice(rle);
        b
    };
    // Bottom row: two blue pixels, end of line; then the end of the bitmap.
    let holes = file(&[2, 1, 0, 0, 0, 1]);
    let d = Bmp::parse(&holes).unwrap();
    assert!(d.info().has_alpha);
    let img = d.decode(&holes, &Options::default(), NO).unwrap();
    let px = |x: usize, y: usize| img.data[(y * 4 + x) * 4..][..4].to_vec();
    assert_eq!(px(0, 1), [0, 0, 255, 255]);
    assert_eq!(px(1, 1), [0, 0, 255, 255]);
    assert_eq!(px(2, 1), [0, 0, 0, 0]);
    assert_eq!(px(0, 0), [0, 0, 0, 0]);
    // Every pixel painted (the bottom row red, the top blue): opaque.
    let full = file(&[4, 0, 0, 0, 4, 1, 0, 1]);
    let d = Bmp::parse(&full).unwrap();
    assert!(!d.info().has_alpha);
    let img = d.decode(&full, &Options::default(), NO).unwrap();
    assert!(img.data.chunks_exact(4).all(|p| p[3] == 255));
    assert_eq!(img.data[..4], [0, 0, 255, 255]);
    assert_eq!(img.data[16..20], [255, 0, 0, 255]);
}
