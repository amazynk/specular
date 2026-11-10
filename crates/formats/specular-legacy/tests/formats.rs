//! Still formats: hand-built files from the test encoders, plus the `pcx` crate as an oracle.

mod common;

use common::*;
use specular_legacy::{Format, PixelFormat, decode, detect, info};

fn ega16() -> Vec<[u8; 3]> {
    (0..16u8).map(|i| [i * 16, 255 - i * 16, i * 7]).collect()
}

/// Pack indices of `bpp` bits MSB-first into `bpl` bytes.
fn pack(idx: &[u8], bpp: usize, bpl: usize) -> Vec<u8> {
    let mut v = vec![0u8; bpl];
    let per = 8 / bpp;
    for (x, &i) in idx.iter().enumerate() {
        v[x / per] |= i << (8 - bpp * (x % per + 1));
    }
    v
}

#[test]
fn pcx_low_depths() {
    let (w, h) = (13usize, 5usize);
    let pal = ega16();
    for (bpp, planes) in [(1u8, 1u8), (1, 2), (1, 3), (1, 4), (2, 1), (4, 1)] {
        let bits = bpp as usize * planes as usize;
        let idx = pattern(w * h, bits as u64, (1u16 << bits) as u8 - 1);
        let bpl = (w * bpp as usize).div_ceil(8).next_multiple_of(2);
        let lines: Vec<Vec<u8>> = idx
            .chunks(w)
            .map(|row| {
                let mut l = Vec::new();
                for p in 0..planes as usize {
                    let plane: Vec<u8> = row
                        .iter()
                        .map(|&i| (i >> (p * bpp as usize)) & ((1 << bpp) - 1))
                        .collect();
                    l.extend(pack(&plane, bpp as usize, bpl));
                }
                l
            })
            .collect();
        for rle in [false, true] {
            let f = pcx(
                w as u16, h as u16, bpp, planes, bpl as u16, rle, &pal, &lines, None,
            );
            assert_eq!(detect(&f), Some(Format::Pcx));
            let img = decode(&f).unwrap();
            assert!(!img.truncated, "{bpp}x{planes}");
            if bits == 1 {
                assert_eq!(img.format, PixelFormat::Gray8);
                let want: Vec<u8> = idx.iter().map(|&i| i * 255).collect();
                assert_eq!(img.data, want);
            } else {
                assert_eq!(img.format, PixelFormat::Rgba8);
                let want: Vec<u8> = idx
                    .iter()
                    .flat_map(|&i| {
                        let [r, g, b] = pal[i as usize];
                        [r, g, b, 255]
                    })
                    .collect();
                assert_eq!(img.data, want, "{bpp}x{planes} rle={rle}");
            }
        }
    }
}

#[test]
fn pcx_vs_oracle() {
    let (w, h) = (37u16, 11u16);
    let n = w as usize * h as usize;
    // 8-bit paletted, written by the pcx crate.
    let idx = pattern(n, 7, 255);
    let pal: Vec<u8> = (0..768).map(|i| (i * 37 % 256) as u8).collect();
    let mut f = Vec::new();
    {
        let mut wr = pcx::WriterPaletted::new(&mut f, (w, h), (72, 72)).unwrap();
        for row in idx.chunks(w as usize) {
            wr.write_row(row).unwrap();
        }
        wr.write_palette(&pal).unwrap();
    }
    let img = decode(&f).unwrap();
    let mut rd = pcx::Reader::from_mem(&f).unwrap();
    let mut oi = vec![0u8; n];
    for row in oi.chunks_mut(w as usize) {
        rd.next_row_paletted(row).unwrap();
    }
    let mut op = vec![0u8; 768];
    rd.read_palette(&mut op).unwrap();
    let want: Vec<u8> = oi
        .iter()
        .flat_map(|&i| {
            let p = &op[i as usize * 3..];
            [p[0], p[1], p[2], 255]
        })
        .collect();
    assert_eq!(img.data, want);

    // 24-bit RGB.
    let rgb = pattern(n * 3, 9, 255);
    let mut f = Vec::new();
    {
        let mut wr = pcx::WriterRgb::new(&mut f, (w, h), (72, 72)).unwrap();
        for row in rgb.chunks(w as usize * 3) {
            wr.write_row(row).unwrap();
        }
        wr.finish().unwrap();
    }
    let img = decode(&f).unwrap();
    let mut rd = pcx::Reader::from_mem(&f).unwrap();
    let mut o = vec![0u8; n * 3];
    for row in o.chunks_mut(w as usize * 3) {
        rd.next_row_rgb(row).unwrap();
    }
    let want: Vec<u8> = o.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    assert_eq!(img.data, want);
    assert_eq!(rgb, o);
}

#[test]
fn pcx_8bit_grey_and_rgba() {
    let (w, h) = (9usize, 4usize);
    let g = pattern(w * h, 3, 255);
    let lines: Vec<Vec<u8>> = g.chunks(w).map(|r| [r, &[0][..]].concat()).collect();
    let f = pcx(w as u16, h as u16, 8, 1, 10, true, &[], &lines, None);
    let img = decode(&f).unwrap();
    assert_eq!(img.format, PixelFormat::Gray8);
    assert_eq!(img.data, g);
    let px = pattern(w * h * 4, 4, 255);
    let lines: Vec<Vec<u8>> = (0..h)
        .map(|y| {
            (0..4)
                .flat_map(|c| (0..10).map(move |x| (c, x)))
                .map(|(c, x)| if x < w { px[(y * w + x) * 4 + c] } else { 0 })
                .collect()
        })
        .collect();
    let f = pcx(w as u16, h as u16, 8, 4, 10, true, &[], &lines, None);
    let i = info(&f).unwrap();
    assert!(i.has_alpha);
    assert_eq!(decode(&f).unwrap().data, px);
}

#[test]
fn sgi_all() {
    let (w, h) = (11usize, 6usize);
    for z in 1..=4usize {
        for bpc in [1u8, 2] {
            let max = if bpc == 1 { 255 } else { 65535 };
            let chans: Vec<Vec<u16>> = (0..z)
                .map(|c| {
                    pattern(w * h * 2, (z * 10 + c) as u64, 255)
                        .chunks(2)
                        .map(|p| {
                            if bpc == 1 {
                                p[0] as u16
                            } else {
                                u16::from_be_bytes([p[0], p[1]]) & max
                            }
                        })
                        .collect()
                })
                .collect();
            let to8 = |v: u16| {
                if bpc == 1 {
                    v as u8
                } else {
                    ((v as u32 * 255 + 32895) >> 16) as u8
                }
            };
            let want: Vec<u8> = (0..w * h)
                .flat_map(|i| match z {
                    1 => vec![to8(chans[0][i])],
                    2 => {
                        let g = to8(chans[0][i]);
                        vec![g, g, g, to8(chans[1][i])]
                    }
                    _ => vec![
                        to8(chans[0][i]),
                        to8(chans[1][i]),
                        to8(chans[2][i]),
                        if z == 4 { to8(chans[3][i]) } else { 255 },
                    ],
                })
                .collect();
            for rle in [false, true] {
                let f = sgi(w as u16, h as u16, bpc, rle, &chans);
                assert_eq!(detect(&f), Some(Format::Sgi));
                let img = decode(&f).unwrap();
                assert!(!img.truncated);
                assert_eq!(img.data, want, "z={z} bpc={bpc} rle={rle}");
            }
        }
    }
}

#[test]
fn sun_all() {
    let (w, h) = (7usize, 5usize);
    let map: Vec<[u8; 3]> = (0..256)
        .map(|i| [i as u8, (255 - i) as u8, (i * 3) as u8])
        .collect();
    // 8-bit mapped and grey, RLE and not.
    let idx = pattern(w * h, 1, 255);
    let stride8 = w.next_multiple_of(2);
    let raw8: Vec<u8> = idx.chunks(w).flat_map(|r| [r, &[0][..]].concat()).collect();
    assert_eq!(raw8.len(), stride8 * h);
    for kind in [1, 2] {
        let f = sun(w as u32, h as u32, 8, kind, &map, &raw8);
        let img = decode(&f).unwrap();
        let want: Vec<u8> = idx
            .iter()
            .flat_map(|&i| {
                let [r, g, b] = map[i as usize];
                [r, g, b, 255]
            })
            .collect();
        assert_eq!(img.data, want);
        let f = sun(w as u32, h as u32, 8, kind, &[], &raw8);
        assert_eq!(decode(&f).unwrap().data, idx);
    }
    // 24-bit BGR / RGB, 32-bit XBGR.
    let px = pattern(w * h * 3, 2, 255);
    let raw24: Vec<u8> = px
        .chunks(w * 3)
        .flat_map(|r| [r, &[0][..]].concat())
        .collect();
    let f = sun(w as u32, h as u32, 24, 1, &[], &raw24);
    let want: Vec<u8> = px.chunks(3).flat_map(|c| [c[2], c[1], c[0], 255]).collect();
    assert_eq!(decode(&f).unwrap().data, want);
    let f = sun(w as u32, h as u32, 24, 3, &[], &raw24);
    let want_rgb: Vec<u8> = px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    assert_eq!(decode(&f).unwrap().data, want_rgb);
    let raw32: Vec<u8> = px.chunks(3).flat_map(|c| [0, c[0], c[1], c[2]]).collect();
    let f = sun(w as u32, h as u32, 32, 2, &[], &raw32);
    assert_eq!(decode(&f).unwrap().data, want);
    // 1-bit.
    let bits = pattern(w * h, 5, 1);
    let raw1: Vec<u8> = bits
        .chunks(w)
        .flat_map(|r| {
            let mut v = [0u8; 2];
            for (x, &b) in r.iter().enumerate() {
                v[x / 8] |= b << (7 - x % 8);
            }
            v
        })
        .collect();
    let f = sun(w as u32, h as u32, 1, 1, &[], &raw1);
    let want: Vec<u8> = bits.iter().map(|&b| if b == 1 { 0 } else { 255 }).collect();
    assert_eq!(decode(&f).unwrap().data, want);
    // 0x80 escapes in RLE.
    let tricky = vec![0x80u8, 0x80, 0x80, 0x80, 1, 0x80, 2, 2, 2, 2, 0x80, 0];
    let f = sun(12, 1, 8, 2, &[], &tricky);
    assert_eq!(decode(&f).unwrap().data, tricky);
}

#[test]
fn xbm_x10_x11() {
    let (w, h) = (19usize, 4usize);
    let bits: Vec<bool> = pattern(w * h, 8, 1).iter().map(|&b| b == 1).collect();
    let want: Vec<u8> = bits.iter().map(|&b| if b { 0 } else { 255 }).collect();
    for x10 in [false, true] {
        let f = xbm(w, h, &bits, x10);
        assert_eq!(detect(&f), Some(Format::Xbm));
        let img = decode(&f).unwrap();
        assert_eq!(img.data, want);
        assert!(!img.truncated);
    }
}

#[test]
fn xpm_colors() {
    let f = br#"/* XPM */
static char *x[] = {
/* comment */
"4 2 6 2 ",
"  c None",
"a. c #f00",
"b. c #00FF00 m white",
"c. c navy blue",
"d. g grey50",
"e. c #ffff0000ffff",
"  a.b.c.",
"d.e.a.  "};
"#;
    assert_eq!(detect(f), Some(Format::Xpm));
    let img = decode(f).unwrap();
    assert!(!img.truncated);
    let px: Vec<&[u8]> = img.data.chunks(4).collect();
    assert_eq!(px[0], [0, 0, 0, 0]);
    assert_eq!(px[1], [255, 0, 0, 255]);
    assert_eq!(px[2], [0, 255, 0, 255]);
    assert_eq!(px[3], [0, 0, 128, 255]);
    assert_eq!(px[4], [127, 127, 127, 255]);
    assert_eq!(px[5], [255, 0, 255, 255]);
    // Unknown key → truncated flag, not a panic.
    let bad = b"/* XPM */\n\"2 1 1 1\",\"a c red\",\"ab\"";
    let img = decode(bad).unwrap();
    assert!(img.truncated);
    assert_eq!(&img.data[..4], [255, 0, 0, 255]);
    // cpp 3 path.
    let f3 = b"/* XPM */\n\"2 1 2 3\",\"abc c #102030\",\"xyz c white\",\"xyzabc\"";
    assert_eq!(
        decode(f3).unwrap().data,
        [255, 255, 255, 255, 16, 32, 48, 255]
    );
}

#[test]
fn x11_names() {
    use specular_legacy::xpm::named_color;
    assert_eq!(named_color("red"), Some([255, 0, 0]));
    assert_eq!(named_color("Light Slate Grey"), Some([119, 136, 153]));
    assert_eq!(named_color("DarkOliveGreen3"), Some([162, 205, 90]));
    assert_eq!(named_color("gray0"), Some([0, 0, 0]));
    assert_eq!(named_color("grey100"), Some([255, 255, 255]));
    assert_eq!(named_color("gray41"), Some([105, 105, 105]));
    assert_eq!(named_color("gray101"), None);
    assert_eq!(named_color("nosuchcolour"), None);
}

#[test]
fn wbmp_and_farbfeld() {
    let (w, h) = (130u32, 3u32);
    let white: Vec<bool> = pattern((w * h) as usize, 4, 1)
        .iter()
        .map(|&b| b == 1)
        .collect();
    let f = wbmp(w, h, &white);
    assert_eq!(detect(&f), Some(Format::Wbmp));
    let want: Vec<u8> = white.iter().map(|&b| if b { 255 } else { 0 }).collect();
    assert_eq!(decode(&f).unwrap().data, want);
    // Missing bytes → not recognised (no magic) rather than garbage.
    assert_eq!(detect(&f[..f.len() - 1]), None);

    let (w, h) = (5u32, 3u32);
    let px: Vec<u16> = pattern((w * h * 8) as usize, 6, 255)
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    let f = farbfeld(w, h, &px);
    let img = decode(&f).unwrap();
    let want: Vec<u8> = px
        .iter()
        .map(|&v| ((v as u32 * 255 + 32895) >> 16) as u8)
        .collect();
    assert_eq!(img.data, want);
    assert!(info(&f).unwrap().has_alpha);
}

#[test]
fn caps_and_garbage() {
    // Huge declared sizes with a tiny body: refused, not allocated.
    let mut f = farbfeld(1, 1, &[0; 4]);
    f[8..12].copy_from_slice(&60000u32.to_be_bytes());
    f[12..16].copy_from_slice(&60000u32.to_be_bytes());
    assert!(decode(&f).is_err());
    let mut f = sgi(1, 1, 1, false, &[vec![0]]);
    f[6..10].copy_from_slice(&[0xFF, 0xFF, 0x0F, 0xFF]);
    assert!(decode(&f).is_err());
    let f = pcx(65535, 4000, 8, 1, 65535, true, &[], &[], None);
    assert!(decode(&f).is_err());
    assert!(decode(b"").is_err());
    assert!(decode(&[0u8; 300]).is_err());
}
