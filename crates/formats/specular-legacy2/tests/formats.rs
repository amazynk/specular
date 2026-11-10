//! Hand-built files for every variant, checked against pixels computed here.

use specular_legacy2::{Error, Model, Pixels, miff, tim, xwd};

fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }
}

// ----- XWD -----------------------------------------------------------------------------

struct X11 {
    format: u32,
    depth: u32,
    w: u32,
    h: u32,
    msb_bytes: bool,
    unit: u32,
    msb_bits: bool,
    bpp: u32,
    bpl: u32,
    visual: u32,
    masks: [u32; 3],
    cmap: Vec<(u32, [u16; 3])>,
    be_header: bool,
    name: &'static str,
}

fn x11(x: &X11, pixels: &[u8]) -> Vec<u8> {
    let name = format!("{}\0", x.name);
    let hs = 100 + name.len() as u32;
    let words = [
        hs,
        7,
        x.format,
        x.depth,
        x.w,
        x.h,
        0,
        x.msb_bytes as u32,
        x.unit,
        x.msb_bits as u32,
        32,
        x.bpp,
        x.bpl,
        x.visual,
        x.masks[0],
        x.masks[1],
        x.masks[2],
        8,
        x.cmap.len() as u32,
        x.cmap.len() as u32,
        x.w,
        x.h,
        3,
        4,
        0,
    ];
    let w32 = |v: u32| {
        if x.be_header {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let w16 = |v: u16| {
        if x.be_header {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        }
    };
    let mut f: Vec<u8> = words.iter().flat_map(|&v| w32(v)).collect();
    f.extend(name.as_bytes());
    for (p, c) in &x.cmap {
        f.extend(w32(*p));
        for v in c {
            f.extend(w16(*v));
        }
        f.extend([7, 0]);
    }
    f.extend(pixels);
    f
}

/// Packs bit rows: `bits(y, x)` into rows of `bpl` bytes in units of `unit` bits.
fn pack_bits(
    w: usize,
    h: usize,
    bpl: usize,
    unit: usize,
    msb_bytes: bool,
    msb_bits: bool,
    bit: impl Fn(usize, usize) -> bool,
) -> Vec<u8> {
    let mut out = vec![0u8; bpl * h];
    let ub = unit / 8;
    for y in 0..h {
        for x in 0..w {
            if !bit(y, x) {
                continue;
            }
            let u = x / unit;
            let b = x % unit;
            let sig = if msb_bits { unit - 1 - b } else { b };
            let byte = if msb_bytes { ub - 1 - sig / 8 } else { sig / 8 };
            out[y * bpl + u * ub + byte] |= 1 << (sig % 8);
        }
    }
    out
}

#[test]
fn xwd_truecolor_32_both_orders() {
    let (w, h) = (7usize, 5usize);
    let mut r = rng(1);
    let px: Vec<[u8; 3]> = (0..w * h)
        .map(|_| [r() as u8, r() as u8, r() as u8])
        .collect();
    for (msb, be) in [(true, true), (false, false), (true, false), (false, true)] {
        let mut data = Vec::new();
        for p in &px {
            let v = (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32;
            data.extend(if msb {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            });
        }
        let f = x11(
            &X11 {
                format: 2,
                depth: 24,
                w: w as u32,
                h: h as u32,
                msb_bytes: msb,
                unit: 32,
                msb_bits: msb,
                bpp: 32,
                bpl: w as u32 * 4,
                visual: 4,
                masks: [0xff0000, 0xff00, 0xff],
                cmap: vec![],
                be_header: be,
                name: "xterm",
            },
            &data,
        );
        assert!(xwd::probe(&f));
        let i = xwd::info(&f, 0).unwrap();
        assert_eq!((i.width, i.height, i.model), (7, 5, Model::Rgb));
        assert_eq!(i.summary, vec![("window", "xterm".to_string())]);
        let img = xwd::decode(&f, 0).unwrap();
        let want: Vec<u8> = px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        assert_eq!(img.pixels, Pixels::Rgba8(want), "msb {msb} be {be}");
    }
}

#[test]
fn xwd_truecolor_565_and_24bpp() {
    let (w, h) = (5usize, 3usize);
    let mut r = rng(2);
    let vals: Vec<u16> = (0..w * h).map(|_| r() as u16).collect();
    for msb in [true, false] {
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let v = vals[y * w + x];
                data.extend(if msb {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                });
            }
            data.extend([0, 0]); // padding to 12 bytes
        }
        let f = x11(
            &X11 {
                format: 2,
                depth: 16,
                w: w as u32,
                h: h as u32,
                msb_bytes: msb,
                unit: 16,
                msb_bits: true,
                bpp: 16,
                bpl: 12,
                visual: 4,
                masks: [0xf800, 0x07e0, 0x001f],
                cmap: vec![],
                be_header: true,
                name: "",
            },
            &data,
        );
        let Pixels::Rgba8(got) = xwd::decode(&f, 0).unwrap().pixels else {
            panic!()
        };
        for (i, &v) in vals.iter().enumerate() {
            let s = |c: u32, bits: u32| {
                let m = (1 << bits) - 1;
                ((c * 255 + m / 2) / m) as u8
            };
            let want = [
                s((v >> 11) as u32, 5),
                s(((v >> 5) & 63) as u32, 6),
                s((v & 31) as u32, 5),
                255,
            ];
            assert_eq!(&got[i * 4..i * 4 + 4], &want);
        }
    }
    // 24 bits per pixel, LSB first: bytes are b, g, r.
    let data: Vec<u8> = (0..w * h)
        .flat_map(|i| [i as u8, 100 + i as u8, 200 - i as u8])
        .collect();
    let f = x11(
        &X11 {
            format: 2,
            depth: 24,
            w: w as u32,
            h: h as u32,
            msb_bytes: false,
            unit: 32,
            msb_bits: false,
            bpp: 24,
            bpl: w as u32 * 3,
            visual: 4,
            masks: [0xff0000, 0xff00, 0xff],
            cmap: vec![],
            be_header: false,
            name: "",
        },
        &data,
    );
    let Pixels::Rgba8(got) = xwd::decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    assert_eq!(&got[4..8], &[200 - 1, 101, 1, 255]);
}

fn cmap16() -> Vec<(u32, [u16; 3])> {
    (0..16u32)
        .map(|i| {
            (
                i,
                [
                    (i * 4000) as u16,
                    (65535 - i * 4000) as u16,
                    (i * 1234) as u16,
                ],
            )
        })
        .collect()
}

#[test]
fn xwd_pseudocolor_2bit() {
    let (w, h) = (7usize, 3usize);
    let idx: Vec<u8> = (0..w * h).map(|i| ((i * 3) % 4) as u8).collect();
    let cm = cmap16();
    let want: Vec<u8> = idx
        .iter()
        .flat_map(|&i| {
            let c = cm[i as usize].1;
            let e = |v: u16| ((v as u32 * 255 + 32895) >> 16) as u8;
            [e(c[0]), e(c[1]), e(c[2]), 255]
        })
        .collect();
    for msb in [true, false] {
        let bpl = 4;
        let mut data = vec![0u8; bpl * h];
        for y in 0..h {
            for x in 0..w {
                let shift = if msb { 6 - 2 * (x % 4) } else { 2 * (x % 4) };
                data[y * bpl + x / 4] |= idx[y * w + x] << shift;
            }
        }
        let f = x11(
            &X11 {
                format: 2,
                depth: 2,
                w: w as u32,
                h: h as u32,
                msb_bytes: msb,
                unit: 8,
                msb_bits: msb,
                bpp: 2,
                bpl: bpl as u32,
                visual: 3,
                masks: [0; 3],
                cmap: cm.clone(),
                be_header: true,
                name: "",
            },
            &data,
        );
        assert_eq!(
            xwd::decode(&f, 0).unwrap().pixels,
            Pixels::Rgba8(want.clone()),
            "msb {msb}"
        );
    }
}

/// Found against ImageMagick (oracles round 4, Sembiance's 1-bit `.dmp` screen dumps): colour
/// map entries whose pixel fields are all 0 go by position, not by pixel value (which kept
/// only the last entry and drew the bitmap with the wrong colours).
#[test]
fn xwd_colormap_with_repeated_pixel_fields() {
    let (w, h) = (8usize, 2usize);
    let data = [0b1010_0000u8, 0b0101_1111];
    let cmap = vec![(0u32, [0u16, 0, 0]), (0u32, [0xFFFF, 0xFFFF, 0xFFFF])];
    let f = x11(
        &X11 {
            format: 2,
            depth: 1,
            w: w as u32,
            h: h as u32,
            msb_bytes: true,
            unit: 8,
            msb_bits: true,
            bpp: 1,
            bpl: 1,
            visual: 2,
            masks: [0; 3],
            cmap,
            be_header: true,
            name: "",
        },
        &data,
    );
    let Pixels::Rgba8(px) = xwd::decode(&f, 0).unwrap().pixels else {
        panic!("not RGBA8");
    };
    let grey: Vec<u8> = px.chunks_exact(4).map(|p| p[0]).collect();
    let bits = |b: u8| (0..8).map(move |k| if b >> (7 - k) & 1 == 1 { 255 } else { 0 });
    let want: Vec<u8> = bits(data[0]).chain(bits(data[1])).collect();
    assert_eq!(grey, want);
}

#[test]
fn xwd_pseudocolor_4bit_and_xypixmap() {
    let (w, h) = (9usize, 4usize);
    let idx: Vec<u8> = (0..w * h).map(|i| ((i * 7) % 16) as u8).collect();
    let cm = cmap16();
    let want: Vec<u8> = idx
        .iter()
        .flat_map(|&i| {
            let c = cm[i as usize].1;
            let e = |v: u16| ((v as u32 * 255 + 32895) >> 16) as u8;
            [e(c[0]), e(c[1]), e(c[2]), 255]
        })
        .collect();
    // ZPixmap, 4 bits per pixel, both nibble orders.
    for msb in [true, false] {
        let bpl = 8;
        let mut data = vec![0u8; bpl * h];
        for y in 0..h {
            for x in 0..w {
                let v = idx[y * w + x];
                let hi = (x % 2 == 0) == msb;
                data[y * bpl + x / 2] |= if hi { v << 4 } else { v };
            }
        }
        let f = x11(
            &X11 {
                format: 2,
                depth: 4,
                w: w as u32,
                h: h as u32,
                msb_bytes: msb,
                unit: 8,
                msb_bits: msb,
                bpp: 4,
                bpl: bpl as u32,
                visual: 3,
                masks: [0; 3],
                cmap: cm.clone(),
                be_header: true,
                name: "",
            },
            &data,
        );
        assert_eq!(xwd::info(&f, 0).unwrap().model, Model::Palette);
        assert_eq!(
            xwd::decode(&f, 0).unwrap().pixels,
            Pixels::Rgba8(want.clone())
        );
    }
    // XYPixmap: four planes, most significant first, every unit size and bit order.
    for unit in [8usize, 16, 32] {
        for (mb, mbits) in [(true, true), (false, false), (true, false), (false, true)] {
            let bpl = 8;
            let mut data = Vec::new();
            for p in (0..4).rev() {
                data.extend(pack_bits(w, h, bpl, unit, mb, mbits, |y, x| {
                    idx[y * w + x] >> p & 1 != 0
                }));
            }
            let f = x11(
                &X11 {
                    format: 1,
                    depth: 4,
                    w: w as u32,
                    h: h as u32,
                    msb_bytes: mb,
                    unit: unit as u32,
                    msb_bits: mbits,
                    bpp: 1,
                    bpl: bpl as u32,
                    visual: 3,
                    masks: [0; 3],
                    cmap: cm.clone(),
                    be_header: false,
                    name: "",
                },
                &data,
            );
            assert_eq!(
                xwd::decode(&f, 0).unwrap().pixels,
                Pixels::Rgba8(want.clone()),
                "unit {unit} {mb} {mbits}"
            );
        }
    }
}

#[test]
fn xwd_bitmap_and_grey() {
    let (w, h) = (19usize, 3usize);
    let on = |y: usize, x: usize| (x * 3 + y).is_multiple_of(5);
    let data = pack_bits(w, h, 4, 32, false, false, on);
    let f = x11(
        &X11 {
            format: 0,
            depth: 1,
            w: w as u32,
            h: h as u32,
            msb_bytes: false,
            unit: 32,
            msb_bits: false,
            bpp: 1,
            bpl: 4,
            visual: 0,
            masks: [0; 3],
            cmap: vec![],
            be_header: true,
            name: "",
        },
        &data,
    );
    assert_eq!(xwd::info(&f, 0).unwrap().model, Model::Bilevel);
    let want: Vec<u8> = (0..h)
        .flat_map(|y| (0..w).map(move |x| if on(y, x) { 0 } else { 255 }))
        .collect();
    assert_eq!(xwd::decode(&f, 0).unwrap().pixels, Pixels::Gray8(want));
    // 6-bit StaticGray without a colour map: scaled to white.
    let data: Vec<u8> = (0..12u8).map(|i| i * 5).collect();
    let f = x11(
        &X11 {
            format: 2,
            depth: 6,
            w: 4,
            h: 3,
            msb_bytes: true,
            unit: 8,
            msb_bits: true,
            bpp: 8,
            bpl: 4,
            visual: 0,
            masks: [0; 3],
            cmap: vec![],
            be_header: true,
            name: "",
        },
        &data,
    );
    let want: Vec<u8> = data
        .iter()
        .map(|&v| ((v as u32 * 255 + 31) / 63) as u8)
        .collect();
    assert_eq!(xwd::decode(&f, 0).unwrap().pixels, Pixels::Gray8(want));
}

#[test]
fn xwd_directcolor() {
    // 3 bits per component; each component indexes the map's own channel.
    let cm: Vec<(u32, [u16; 3])> = (0..8u32)
        .map(|i| {
            (
                i,
                [
                    (i * 9000) as u16,
                    (i * 8000) as u16,
                    (60000 - i * 7000) as u16,
                ],
            )
        })
        .collect();
    let vals: Vec<u8> = (0..16u8).map(|i| i.wrapping_mul(37) & 0x3f).collect();
    // masks: red 0b110000? use r = bits 4-5 (2 bits), g = bits 2-3, b = bits 0-1
    let f = x11(
        &X11 {
            format: 2,
            depth: 6,
            w: 4,
            h: 4,
            msb_bytes: true,
            unit: 8,
            msb_bits: true,
            bpp: 8,
            bpl: 4,
            visual: 5,
            masks: [0x30, 0x0c, 0x03],
            cmap: cm.clone(),
            be_header: true,
            name: "",
        },
        &vals,
    );
    let Pixels::Rgba8(got) = xwd::decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    let e = |v: u16| ((v as u32 * 255 + 32895) >> 16) as u8;
    for (i, &v) in vals.iter().enumerate() {
        let (r, g, b) = ((v >> 4) & 3, (v >> 2) & 3, v & 3);
        let want = [
            e(cm[r as usize].1[0]),
            e(cm[g as usize].1[1]),
            e(cm[b as usize].1[2]),
            255,
        ];
        assert_eq!(&got[i * 4..i * 4 + 4], &want);
    }
}

#[test]
fn xwd_x10() {
    // X10, little-endian, 4 planes in Z format (one byte per pixel) with a colour map.
    let (w, h) = (6u32, 2u32);
    let cm = cmap16();
    for be in [false, true] {
        let w32 = |v: u32| if be { v.to_be_bytes() } else { v.to_le_bytes() };
        let w16 = |v: u16| if be { v.to_be_bytes() } else { v.to_le_bytes() };
        let name = b"x10 win\0";
        let mut f = Vec::new();
        for v in [40 + name.len() as u32, 6, 0, 4, 1, w, h] {
            f.extend(w32(v));
        }
        for v in [w as u16, h as u16, 0, 0, 0, 16] {
            f.extend(w16(v));
        }
        f.extend(name);
        for (p, c) in &cm {
            f.extend(w32(*p));
            for v in c {
                f.extend(w16(*v));
            }
            f.extend([0, 0]);
        }
        let idx: Vec<u8> = (0..w * h).map(|i| (i * 5 % 16) as u8).collect();
        f.extend(&idx);
        assert!(xwd::probe(&f));
        let i = xwd::info(&f, 0).unwrap();
        assert!(i.coding.starts_with("X10"), "{}", i.coding);
        assert_eq!(i.summary, vec![("window", "x10 win".to_string())]);
        let Pixels::Rgba8(got) = xwd::decode(&f, 0).unwrap().pixels else {
            panic!()
        };
        let e = |v: u16| ((v as u32 * 255 + 32895) >> 16) as u8;
        for (k, &v) in idx.iter().enumerate() {
            let c = cm[v as usize].1;
            assert_eq!(&got[k * 4..k * 4 + 3], &[e(c[0]), e(c[1]), e(c[2])]);
        }
    }
    // X10 XY bitmap: 16-bit units, LSB first.
    let mut f = Vec::new();
    for v in [40u32, 6, 0, 1, 0, 20, 2] {
        f.extend(v.to_le_bytes());
    }
    for v in [20u16, 2, 0, 0, 0, 0] {
        f.extend(v.to_le_bytes());
    }
    let on = |y: usize, x: usize| (x + y).is_multiple_of(3);
    f.extend(pack_bits(20, 2, 4, 16, false, false, on));
    let want: Vec<u8> = (0..2)
        .flat_map(|y| (0..20).map(move |x| if on(y, x) { 0 } else { 255 }))
        .collect();
    assert_eq!(xwd::decode(&f, 0).unwrap().pixels, Pixels::Gray8(want));
}

#[test]
fn xwd_refusals_and_truncation() {
    let mut x = X11 {
        format: 2,
        depth: 12,
        w: 4,
        h: 4,
        msb_bytes: true,
        unit: 8,
        msb_bits: true,
        bpp: 12,
        bpl: 8,
        visual: 0,
        masks: [0; 3],
        cmap: vec![],
        be_header: true,
        name: "",
    };
    let f = x11(&x, &[0; 32]);
    assert!(matches!(xwd::decode(&f, 0), Err(Error::Unsupported(_))));
    x.bpp = 16;
    let f = x11(&x, &[9; 20]);
    let img = xwd::decode(&f, 0).unwrap();
    assert!(img.truncated);
    assert!(!xwd::probe(b"\0\0\0\x64\0\0\0\x08"));
    // Every prefix of a valid file: no panic.
    let f = x11(&x, &[9; 32]);
    for n in 0..f.len() {
        let _ = xwd::decode(&f[..n], 0);
        let _ = xwd::info(&f[..n], 0);
    }
}

// ----- TIM -----------------------------------------------------------------------------

fn tim_block(w: u16, h: u16, data: &[u8]) -> Vec<u8> {
    let mut b = (12 + data.len() as u32).to_le_bytes().to_vec();
    for v in [0u16, 0, w, h] {
        b.extend(v.to_le_bytes());
    }
    b.extend(data);
    b
}

fn c15(r: u8, g: u8, b: u8, stp: bool) -> u16 {
    (r >> 3) as u16 | ((g >> 3) as u16) << 5 | ((b >> 3) as u16) << 10 | (stp as u16) << 15
}

fn e5(v: u8) -> u8 {
    let v = v >> 3;
    (v << 3) | (v >> 2)
}

#[test]
fn tim_all_modes() {
    // 4-bit, two CLUTs (the first is shown).
    let cl: Vec<u16> = (0..32u16)
        .map(|i| {
            if i == 3 {
                0
            } else {
                c15((i * 8) as u8, 255 - (i * 8) as u8, 77, i % 2 == 0)
            }
        })
        .collect();
    let cl_bytes: Vec<u8> = cl.iter().flat_map(|v| v.to_le_bytes()).collect();
    let idx: Vec<u8> = (0..8 * 3).map(|i| (i * 5 % 16) as u8).collect();
    let data: Vec<u8> = idx.chunks(2).map(|p| p[0] | p[1] << 4).collect();
    let mut f = vec![0x10, 0, 0, 0, 8, 0, 0, 0];
    f.extend(tim_block(16, 2, &cl_bytes));
    f.extend(tim_block(2, 3, &data));
    assert!(tim::probe(&f));
    let i = tim::info(&f, 0).unwrap();
    assert_eq!(
        (i.width, i.height, i.model, i.has_alpha),
        (8, 3, Model::Palette, true)
    );
    assert!(i.coding.contains("first of 2"), "{}", i.coding);
    let Pixels::Rgba8(got) = tim::decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    for (k, &v) in idx.iter().enumerate() {
        let c = cl[v as usize];
        let want = [
            e5(((c & 31) << 3) as u8),
            e5((((c >> 5) & 31) << 3) as u8),
            e5((((c >> 10) & 31) << 3) as u8),
            if c == 0 { 0 } else { 255 },
        ];
        assert_eq!(&got[k * 4..k * 4 + 4], &want);
    }
    // 8-bit.
    let cl_bytes: Vec<u8> = (0..256u16)
        .flat_map(|i| c15(i as u8, i as u8, 0, false).to_le_bytes())
        .collect();
    let idx: Vec<u8> = (0..4 * 2).map(|i| i as u8 * 30).collect();
    let mut f = vec![0x10, 0, 0, 0, 9, 0, 0, 0];
    f.extend(tim_block(256, 1, &cl_bytes));
    f.extend(tim_block(2, 2, &idx));
    let Pixels::Rgba8(got) = tim::decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    assert_eq!(&got[4..8], &[e5(30), e5(30), 0, 255]);
    // 16-bit.
    let px: Vec<u16> = vec![
        0,
        c15(255, 0, 0, true),
        c15(0, 255, 0, false),
        c15(8, 8, 8, false),
    ];
    let data: Vec<u8> = px.iter().flat_map(|v| v.to_le_bytes()).collect();
    let mut f = vec![0x10, 0, 0, 0, 2, 0, 0, 0];
    f.extend(tim_block(2, 2, &data));
    let Pixels::Rgba8(got) = tim::decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    assert_eq!(
        got,
        vec![0, 0, 0, 0, 255, 0, 0, 255, 0, 255, 0, 255, 8, 8, 8, 255]
    );
    // 24-bit: 2 pixels per 3 units.
    let data: Vec<u8> = (0..2 * 2 * 3).map(|i| i as u8 * 20).collect();
    let mut f = vec![0x10, 0, 0, 0, 3, 0, 0, 0];
    f.extend(tim_block(3, 2, &data));
    let img = tim::decode(&f, 0).unwrap();
    assert_eq!((img.width, img.height), (2, 2));
    let Pixels::Rgba8(got) = img.pixels else {
        panic!()
    };
    assert_eq!(&got[4..8], &[60, 80, 100, 255]);
    // Mixed mode: the frame buffer's 16-bit colours; truncation: flagged; prefixes never
    // panic.
    let mut m = f.clone();
    m[4] = 4;
    let i = tim::info(&m, 0).unwrap();
    assert!(i.coding.starts_with("mixed"), "{}", i.coding);
    let mut sixteen = f.clone();
    sixteen[4] = 2;
    assert_eq!(tim::decode(&m, 0), tim::decode(&sixteen, 0));
    assert_eq!(tim::decode(&m, 0).unwrap().width, 3);
    let img = tim::decode(&f[..f.len() - 4], 0).unwrap();
    assert!(img.truncated);
    for n in 0..f.len() {
        let _ = tim::decode(&f[..n], 0);
    }
}

// ----- MIFF ----------------------------------------------------------------------------

fn miff_file(header: &str, binary: &[u8]) -> Vec<u8> {
    let mut f = format!("id=ImageMagick version=1.0\n{header}\n\x0c\n:\x1a").into_bytes();
    f.extend(binary);
    f
}

#[test]
fn miff_direct_rle_and_pages() {
    // 3×2 RGB, 8-bit, RLE: runs that span rows are allowed when reading.
    let data = [10, 20, 30, 3, 1, 2, 3, 1];
    let a = miff_file(
        "class=DirectClass alpha-trait=Undefined number-channels=3\ncolumns=3 rows=2 depth=8 colorspace=sRGB compression=RLE\n{a comment}",
        &data,
    );
    let i = miff::info(&a, 0).unwrap();
    assert_eq!(i.summary[0], ("comment", "a comment".to_string()));
    let img = miff::decode(&a, 0).unwrap();
    assert_eq!(
        img.pixels,
        Pixels::Rgba8(vec![
            10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255, 1, 2, 3, 255, 1, 2,
            3, 255
        ])
    );
    // A second image, 16-bit grey LSB-first, uncompressed.
    let mut f = a.clone();
    f.extend(miff_file(
        "class=DirectClass alpha-trait=Undefined columns=2 rows=1 depth=16 colorspace=Gray endian=LSB",
        &[0x34, 0x12, 0xff, 0xff],
    ));
    assert_eq!(miff::info(&f, 0).unwrap().pages, 2);
    assert_eq!(miff::info(&f, 1).unwrap().model, Model::Grey);
    assert_eq!(
        miff::decode(&f, 1).unwrap().pixels,
        Pixels::Gray16(vec![0x1234, 0xffff])
    );
    assert!(miff::decode(&f, 2).is_err());
}

#[test]
fn miff_palette_alpha_float_cmyk() {
    // PseudoClass, 16-bit map, 16-bit indices, with alpha.
    let mut bin = Vec::new();
    for c in [[0u16, 0x8000, 0xffff], [0x1111, 0x2222, 0x3333]] {
        for v in c {
            bin.extend(v.to_be_bytes());
        }
    }
    for (i, a) in [(1u16, 0xffffu16), (0, 0x0102)] {
        bin.extend(i.to_be_bytes());
        bin.extend(a.to_be_bytes());
    }
    let f = miff_file(
        "class=PseudoClass colors=2 alpha-trait=Blend columns=2 rows=1 depth=16 colorspace=sRGB",
        &bin,
    );
    let i = miff::info(&f, 0).unwrap();
    assert!(i.has_alpha && i.model == Model::Palette);
    assert_eq!(
        miff::decode(&f, 0).unwrap().pixels,
        Pixels::Rgba16(vec![
            0x1111, 0x2222, 0x3333, 0xffff, 0, 0x8000, 0xffff, 0x0102
        ])
    );
    // 32-bit floats, sRGB-coded → linear.
    let mut bin = Vec::new();
    for v in [0.5f32, 1.0, 0.0] {
        bin.extend(v.to_be_bytes());
    }
    let f = miff_file(
        "class=DirectClass alpha-trait=Undefined columns=1 rows=1 depth=32 colorspace=sRGB quantum:format=floating-point",
        &bin,
    );
    let Pixels::RgbaF32(p) = miff::decode(&f, 0).unwrap().pixels else {
        panic!()
    };
    assert!((p[0] - 0.214_041).abs() < 1e-4 && p[1] == 1.0 && p[2] == 0.0 && p[3] == 1.0);
    // CMYK 8-bit.
    let f = miff_file(
        "class=DirectClass alpha-trait=Undefined columns=1 rows=1 depth=8 colorspace=CMYK",
        &[255, 0, 0, 0],
    );
    assert_eq!(
        miff::decode(&f, 0).unwrap().pixels,
        Pixels::Rgba8(vec![0, 255, 255, 255])
    );
}

/// bzip2 has no useful expansion bound: a 2048 × 4096 blank grey image (8 MiB) packs into
/// 48 bytes, which a fixed 50 000:1 ratio refused as truncated. Judged by what the stream
/// decodes to: that image decodes, a header claiming 16384 × 16384 16-bit RGB (1.5 GiB)
/// over the same bytes is refused, and an image the memory budget cannot hold is a memory refusal.
#[test]
fn miff_bzip_judged_by_what_it_decodes() {
    // bzip2 of 8 MiB of zeros (python3 bz2.compress, level 9), as one chunk.
    let bz: &[u8] = &[
        66, 90, 104, 57, 49, 65, 89, 38, 83, 89, 162, 61, 75, 88, 0, 64, 64, 64, 128, 192, 0, 0, 4,
        0, 8, 32, 0, 48, 204, 5, 41, 166, 1, 0, 216, 128, 128, 120, 187, 146, 41, 194, 132, 133,
        17, 234, 90, 192,
    ];
    let mut chunk = (bz.len() as u32).to_be_bytes().to_vec();
    chunk.extend_from_slice(bz);
    let file = |w: u32, h: u32, depth: u32, space: &str| {
        miff_file(
            &format!(
                "class=DirectClass columns={w} rows={h} depth={depth} colorspace={space} \
                 compression=BZip"
            ),
            &chunk,
        )
    };
    let f = file(2048, 4096, 8, "Gray");
    let img = miff::decode(&f, 0).unwrap();
    assert!(!img.truncated);
    match &img.pixels {
        Pixels::Gray8(p) => assert!(p.len() == 8 << 20 && p.iter().all(|&v| v == 0)),
        p => panic!("{:?}", std::mem::discriminant(p)),
    }
    assert!(matches!(
        miff::decode(&file(16_384, 16_384, 16, "sRGB"), 0),
        Err(Error::Truncated(_))
    ));
    let r = specular_core::budget::with_limit(4 << 20, || miff::decode(&f, 0).map(|_| ()));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
}

#[test]
fn miff_refusals_and_robustness() {
    let f = miff_file(
        "class=DirectClass columns=2 rows=2 depth=8 compression=LZMA",
        &[0; 8],
    );
    assert_eq!(
        miff::decode(&f, 0),
        Err(Error::Unsupported(
            "MIFF images compressed with LZMA are not supported".into()
        ))
    );
    let f = miff_file(
        "class=DirectClass columns=2 rows=2 depth=8 colorspace=Jzazbz",
        &[0; 12],
    );
    assert!(matches!(miff::info(&f, 0), Err(Error::Unsupported(_))));
    assert!(!miff::probe(b"id=Other"));
    assert!(miff::probe(b"{c}\n id=ImageMagick"));
    let f = miff_file(
        "class=DirectClass columns=4 rows=4 depth=8 compression=RLE",
        &[1, 2, 3, 200],
    );
    for n in 0..f.len() {
        let _ = miff::decode(&f[..n], 0);
        let _ = miff::info(&f[..n], 0);
    }
    let img = miff::decode(&f[..f.len() - 1], 0).unwrap();
    assert!(img.truncated);
}

#[test]
fn committed_fixtures() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    for (name, w, h) in [
        ("rose-zip-pal.miff", 70, 46),
        ("rose-bzip-16.miff", 70, 46),
        ("rose.xwd", 70, 46),
        ("tim8.tim", 32, 17),
    ] {
        let d = std::fs::read(format!("{dir}/{name}")).unwrap();
        let (i, img) = match name.rsplit('.').next().unwrap() {
            "miff" => (miff::info(&d, 0).unwrap(), miff::decode(&d, 0).unwrap()),
            "xwd" => (xwd::info(&d, 0).unwrap(), xwd::decode(&d, 0).unwrap()),
            _ => (tim::info(&d, 0).unwrap(), tim::decode(&d, 0).unwrap()),
        };
        assert_eq!((i.width, i.height), (w, h), "{name}");
        assert!(!img.truncated, "{name}");
        // Against the 8-bit RGBA ImageMagick gave (expect/<name>.rgba).
        let want = std::fs::read(format!("{dir}/{name}.rgba")).unwrap();
        let got: Vec<u8> = match img.pixels {
            Pixels::Rgba8(v) => v,
            Pixels::Rgba16(v) => v
                .iter()
                .map(|&x| ((x as u32 * 255 + 32767) / 65535) as u8)
                .collect(),
            Pixels::Gray8(v) => v.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            _ => panic!("{name}"),
        };
        assert_eq!(got.len(), want.len(), "{name}");
        let bad = got
            .iter()
            .zip(&want)
            .enumerate()
            .filter(|(k, (a, b))| a != b && !(name.ends_with(".tim") && k % 4 == 3))
            .count();
        assert_eq!(bad, 0, "{name}");
    }
}

/// A zlib stream of one stored block.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01, 0x01];
    z.extend((data.len() as u16).to_le_bytes());
    z.extend((!(data.len() as u16)).to_le_bytes());
    z.extend(data);
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    z
}

/// Listing pages skips an unchunked Zip image by inflating it: a 16000×16000 page holding a
/// 6-byte stream allocated its 768 MB image just to find the next page. It now inflates
/// into a small scratch buffer; the next page is still found.
#[test]
fn miff_zip_page_skipped_without_its_image() {
    // No `version`: an unchunked stream (version 0).
    let miff_file = |h: &str, bin: &[u8]| {
        let mut f = format!("id=ImageMagick\n{h}\n\x0c\n:\x1a").into_bytes();
        f.extend(bin);
        f
    };
    let mut f = miff_file(
        "class=DirectClass alpha-trait=Undefined number-channels=3\ncolumns=16000 rows=16000 depth=8 colorspace=sRGB compression=Zip",
        &zlib_stored(&[1, 2, 3, 4, 5, 6]),
    );
    let small = miff_file(
        "class=DirectClass alpha-trait=Undefined number-channels=3\ncolumns=2 rows=1 depth=8 colorspace=sRGB compression=Zip",
        &zlib_stored(&[1, 2, 3, 4, 5, 6]),
    );
    f.extend(&small);
    assert_eq!(miff::info(&f, 0).unwrap().pages, 2);
    assert_eq!(
        miff::decode(&f, 1).unwrap().pixels,
        Pixels::Rgba8(vec![1, 2, 3, 255, 4, 5, 6, 255])
    );
}
