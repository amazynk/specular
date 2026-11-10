//! Test-image writers and the pixels they are expected to decode to (every fixture is
//! generated; nothing is read from disk). The expected outputs are computed from the same
//! pattern the writers encode, independently of the decoders under test.
#![allow(dead_code, clippy::field_reassign_with_default)]

use specular_bitmap::{Decode, Image, Options, PixelFormat};

/// Deterministic test colour of pixel (x, y).
pub fn pat(x: u32, y: u32) -> [u8; 4] {
    let h = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)).rotate_left(7);
    [
        (x * 7 + y * 3) as u8,
        (x * 2 + y * 9 + (h & 15)) as u8,
        ((h >> 8) as u8) | 1,
        if (x / 3 + y / 5) % 4 == 1 {
            0
        } else {
            255 - ((x + y) % 64) as u8
        },
    ]
}

pub fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
pub fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

pub fn rgba(img: &Image) -> Vec<u8> {
    match img.format {
        PixelFormat::Rgba8 => img.data.clone(),
        PixelFormat::Gray8 => img.data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        // 16-bit images narrowed to 8 bits.
        f => {
            let n =
                |b: &[u8]| ((u16::from_ne_bytes([b[0], b[1]]) as u32 * 255 + 32767) / 65535) as u8;
            img.data
                .chunks_exact(f.bytes_per_pixel())
                .flat_map(|p| match f {
                    PixelFormat::Gray16 => [n(p), n(p), n(p), 255],
                    PixelFormat::GrayAlpha16 => [n(p), n(p), n(p), n(&p[2..])],
                    _ => [n(p), n(&p[2..]), n(&p[4..]), n(&p[6..])],
                })
                .collect()
        }
    }
}

/// Colour of fully transparent pixels is irrelevant: set it to 0.
pub fn norm(mut v: Vec<u8>) -> Vec<u8> {
    for p in v.chunks_exact_mut(4) {
        if p[3] == 0 {
            p.fill(0);
        }
    }
    v
}

pub fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len(), "length");
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

/// Full decode must equal the oracle (within `tol`), and every scale/region must equal the
/// reference box filter of the full decode.
pub fn check(d: &dyn Decode, data: &[u8], want: &[u8], tol: u8, what: &str) {
    let full = d.decode(data, &Options::default(), &|| false).expect(what);
    assert!(!full.truncated, "{what}: truncated");
    let w = full.width;
    assert_eq!(
        want.len(),
        (full.width * full.height * 4) as usize,
        "{what}: size"
    );
    let got = norm(rgba(&full));
    let want = norm(want.to_vec());
    let md = max_diff(&got, &want);
    if md > tol {
        let i = got
            .iter()
            .zip(&want)
            .position(|(a, b)| a.abs_diff(*b) > tol)
            .unwrap();
        panic!(
            "{what}: max diff {md} at pixel {} ({},{}): got {:?} want {:?}",
            i / 4,
            (i / 4) as u32 % w,
            (i / 4) as u32 / w,
            &got[i / 4 * 4..i / 4 * 4 + 4],
            &want[i / 4 * 4..i / 4 * 4 + 4]
        );
    }
    check_scales(d, data, &full, what);
}

/// Reference: box filter in linear light, premultiplied, in f64.
pub fn ref_scale(img: &[u8], w: u32, h: u32, f: u32, opaque: bool) -> Vec<u8> {
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let srgb = |l: f64| {
        let c = if l <= 0.0031308 {
            l * 12.92
        } else {
            1.055 * l.powf(1.0 / 2.4) - 0.055
        };
        (c * 255.0 + 0.5).clamp(0.0, 255.0) as u8
    };
    let (sw, sh) = (w.div_ceil(f), h.div_ceil(f));
    let mut out = Vec::with_capacity((sw * sh * 4) as usize);
    for oy in 0..sh {
        for ox in 0..sw {
            let mut acc = [0f64; 4];
            let mut n = 0f64;
            for y in oy * f..((oy + 1) * f).min(h) {
                for x in ox * f..((ox + 1) * f).min(w) {
                    let p = &img[((y * w + x) * 4) as usize..][..4];
                    let a = if opaque { 1.0 } else { p[3] as f64 / 255.0 };
                    for c in 0..3 {
                        acc[c] += lin(p[c]) * a;
                    }
                    acc[3] += a;
                    n += 1.0;
                }
            }
            for c in 0..3 {
                out.push(if acc[3] > 0.0 {
                    srgb(acc[c] / acc[3])
                } else {
                    0
                });
            }
            out.push(if opaque {
                255
            } else {
                (acc[3] / n * 255.0 + 0.5) as u8
            });
        }
    }
    out
}

pub fn crop(img: &[u8], w: u32, x: u32, y: u32, cw: u32, ch: u32) -> Vec<u8> {
    let mut v = Vec::new();
    for yy in y..y + ch {
        let s = ((yy * w + x) * 4) as usize;
        v.extend_from_slice(&img[s..s + cw as usize * 4]);
    }
    v
}

pub fn check_scales(d: &dyn Decode, data: &[u8], full: &Image, what: &str) {
    let (w, h) = (full.width, full.height);
    let fr = rgba(full);
    let opaque = !d.info().has_alpha;
    for f in [1u32, 2, 3, 4, 8] {
        let s = d
            .decode(
                data,
                &Options {
                    scale: f,
                    region: None,
                    parallel: f % 2 == 0,
                },
                &|| false,
            )
            .unwrap();
        let (sw, sh) = (w.div_ceil(f), h.div_ceil(f));
        assert_eq!((s.width, s.height), (sw, sh), "{what} 1/{f} size");
        let want = ref_scale(&fr, w, h, f, opaque);
        let got = rgba(&s);
        // Integer pipeline vs f64: ±2 (alpha-weighted averages of nearly transparent pixels
        // differ more, so compare colour only where alpha is visible).
        for (i, (g, r)) in got.chunks(4).zip(want.chunks(4)).enumerate() {
            let tol = if r[3] < 16 { 255 } else { 2 };
            for c in 0..4 {
                assert!(
                    g[c].abs_diff(r[c]) <= tol,
                    "{what} 1/{f} px {i} ch {c}: {g:?} vs {r:?}"
                );
            }
        }
        // Regions equal the crop of the whole scaled image.
        for (x, y, rw, rh) in [
            (0, 0, 1, 1),
            (sw / 3, sh / 2, sw / 2 + 1, sh / 3 + 1),
            (sw - 1, sh - 1, 5, 5),
        ] {
            let r = d
                .decode(
                    data,
                    &Options {
                        scale: f,
                        region: Some(specular_bitmap::Region { x, y, w: rw, h: rh }),
                        parallel: true,
                    },
                    &|| false,
                )
                .unwrap();
            let (cw, ch) = (rw.min(sw - x), rh.min(sh - y));
            assert_eq!((r.width, r.height), (cw, ch), "{what} region size");
            assert_eq!(
                rgba(&r),
                crop(&got, sw, x, y, cw, ch),
                "{what} 1/{f} region {x},{y}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------- BMP

/// Options for [`bmp`].
#[derive(Clone, Copy)]
pub struct BmpSpec {
    /// 1, 2, 4, 8, 16, 24, 32.
    pub bpp: u16,
    /// 0 RGB, 1 RLE8, 2 RLE4, 3 bit fields.
    pub comp: u32,
    pub top_down: bool,
    /// 12, 40, 108 or 124.
    pub header: u32,
    /// Bit-field masks for comp 3 (or V4/V5 masks).
    pub masks: [u32; 4],
}

impl BmpSpec {
    pub fn new(bpp: u16) -> BmpSpec {
        BmpSpec {
            bpp,
            comp: 0,
            top_down: false,
            header: 40,
            masks: [0; 4],
        }
    }
}

pub fn palette(n: usize) -> Vec<[u8; 3]> {
    (0..n)
        .map(|i| {
            [
                (i * 37) as u8,
                (i * 91 + 7) as u8,
                (255usize.wrapping_sub(i * 13)) as u8,
            ]
        })
        .collect()
}

fn index_of(x: u32, y: u32, n: u32) -> u8 {
    (((x / 2) + y * 3 + (x * y) % 5) % n) as u8
}

/// The DIB (header, palette, pixels) of a test bitmap, without the file header.
pub fn dib(w: u32, h: u32, s: BmpSpec, icon_and_mask: bool) -> Vec<u8> {
    let mut v = Vec::new();
    let pal_n = if s.bpp <= 8 { 1usize << s.bpp } else { 0 };
    let pix = bmp_pixels(w, h, s);
    let hh = if icon_and_mask {
        h as i32 * 2
    } else {
        h as i32
    };
    if s.header == 12 {
        le32(&mut v, 12);
        le16(&mut v, w as u16);
        le16(&mut v, hh as u16);
        le16(&mut v, 1);
        le16(&mut v, s.bpp);
    } else {
        le32(&mut v, s.header);
        le32(&mut v, w);
        le32(&mut v, if s.top_down { (-hh) as u32 } else { hh as u32 });
        le16(&mut v, 1);
        le16(&mut v, s.bpp);
        le32(&mut v, s.comp);
        le32(&mut v, pix.len() as u32);
        le32(&mut v, 2835);
        le32(&mut v, 2835);
        le32(&mut v, 0);
        le32(&mut v, 0);
        if s.header >= 52 {
            for m in &s.masks[..if s.header >= 56 { 4 } else { 3 }] {
                le32(&mut v, *m);
            }
        }
        while v.len() < s.header as usize {
            v.push(0);
        }
        if s.header == 40 && s.comp == 3 {
            for m in &s.masks[..3] {
                le32(&mut v, *m);
            }
        }
    }
    for c in palette(pal_n) {
        v.extend_from_slice(&[c[2], c[1], c[0]]);
        if s.header != 12 {
            v.push(0);
        }
    }
    v.extend_from_slice(&pix);
    if icon_and_mask {
        let stride = w.div_ceil(32) * 4;
        for fy in 0..h {
            let y = h - 1 - fy;
            let mut row = vec![0u8; stride as usize];
            for x in 0..w {
                if s.bpp != 32 && (x + y).is_multiple_of(7) {
                    row[(x / 8) as usize] |= 0x80 >> (x % 8);
                }
            }
            v.extend_from_slice(&row);
        }
    }
    v
}

fn bmp_pixels(w: u32, h: u32, s: BmpSpec) -> Vec<u8> {
    let mut out = Vec::new();
    let rows: Vec<u32> = if s.top_down {
        (0..h).collect()
    } else {
        (0..h).rev().collect()
    };
    match s.comp {
        1 | 2 => {
            let four = s.comp == 2;
            for &y in &rows {
                let idx: Vec<u8> = (0..w)
                    .map(|x| index_of(x, y, if four { 16 } else { 256 }))
                    .collect();
                let mut x = 0usize;
                let mut toggle = y % 2 == 0;
                while x < idx.len() {
                    // Alternate encoded runs and absolute runs.
                    let mut run = 1;
                    while x + run < idx.len() && idx[x + run] == idx[x] && run < 255 {
                        run += 1;
                    }
                    if run >= 2 || !toggle {
                        out.push(run as u8);
                        out.push(if four { idx[x] << 4 | idx[x] } else { idx[x] });
                        x += run;
                    } else {
                        let n = (idx.len() - x).min(if four { 20 } else { 40 });
                        if n < 3 {
                            out.push(1);
                            out.push(if four { idx[x] << 4 | idx[x] } else { idx[x] });
                            x += 1;
                            continue;
                        }
                        out.push(0);
                        out.push(n as u8);
                        let mut bytes = Vec::new();
                        if four {
                            for k in (0..n).step_by(2) {
                                let a = idx[x + k];
                                let b = if k + 1 < n { idx[x + k + 1] } else { 0 };
                                bytes.push(a << 4 | b);
                            }
                        } else {
                            bytes.extend_from_slice(&idx[x..x + n]);
                        }
                        if bytes.len() % 2 == 1 {
                            bytes.push(0);
                        }
                        out.extend_from_slice(&bytes);
                        x += n;
                    }
                    toggle = !toggle;
                }
                out.extend_from_slice(&[0, 0]);
            }
            out.extend_from_slice(&[0, 1]);
        }
        _ => {
            let stride = ((w * s.bpp as u32).div_ceil(32) * 4) as usize;
            for &y in &rows {
                let mut row = vec![0u8; stride];
                for x in 0..w {
                    let p = pat(x, y);
                    match s.bpp {
                        1 | 2 | 4 => {
                            let b = s.bpp as u32;
                            let i = index_of(x, y, 1 << b) as u32;
                            let bit = x * b;
                            row[(bit / 8) as usize] |= (i << (8 - b - bit % 8)) as u8;
                        }
                        8 => row[x as usize] = index_of(x, y, 256),
                        16 => {
                            let m = if s.comp == 3 {
                                s.masks
                            } else {
                                [0x7c00, 0x3e0, 0x1f, 0]
                            };
                            let v = enc_field(p[0], m[0])
                                | enc_field(p[1], m[1])
                                | enc_field(p[2], m[2]);
                            row[x as usize * 2..x as usize * 2 + 2]
                                .copy_from_slice(&(v as u16).to_le_bytes());
                        }
                        24 => row[x as usize * 3..x as usize * 3 + 3]
                            .copy_from_slice(&[p[2], p[1], p[0]]),
                        _ => {
                            let m = if s.comp == 3 || s.header >= 56 {
                                s.masks
                            } else {
                                [0xff0000, 0xff00, 0xff, 0xff000000]
                            };
                            let v = enc_field(p[0], m[0])
                                | enc_field(p[1], m[1])
                                | enc_field(p[2], m[2])
                                | enc_field(p[3], m[3]);
                            row[x as usize * 4..x as usize * 4 + 4]
                                .copy_from_slice(&v.to_le_bytes());
                        }
                    }
                }
                out.extend_from_slice(&row);
            }
        }
    }
    out
}

fn enc_field(c: u8, mask: u32) -> u32 {
    if mask == 0 {
        return 0;
    }
    let sh = mask.trailing_zeros();
    let max = (mask >> sh) as u64;
    (((c as u64 * max + 127) / 255) as u32) << sh
}

pub fn bmp(w: u32, h: u32, s: BmpSpec) -> Vec<u8> {
    let d = dib(w, h, s, false);
    let pal_n = if s.bpp <= 8 { 1usize << s.bpp } else { 0 };
    let hdr_len = s.header as usize
        + if s.header == 40 && s.comp == 3 { 12 } else { 0 }
        + pal_n * if s.header == 12 { 3 } else { 4 };
    let mut v = b"BM".to_vec();
    le32(&mut v, (14 + d.len()) as u32);
    le32(&mut v, 0);
    le32(&mut v, (14 + hdr_len) as u32);
    v.extend_from_slice(&d);
    v
}

// ---------------------------------------------------------------------------------------- ICO

/// An icon file from (entry bytes, directory width byte, height byte, bpp).
pub fn ico(entries: &[(Vec<u8>, u8, u8, u16)], cursor: bool) -> Vec<u8> {
    let mut v = Vec::new();
    le16(&mut v, 0);
    le16(&mut v, if cursor { 2 } else { 1 });
    le16(&mut v, entries.len() as u16);
    let mut off = 6 + 16 * entries.len();
    for (data, w, h, bpp) in entries {
        v.extend_from_slice(&[*w, *h, 0, 0]);
        le16(&mut v, if cursor { 3 } else { 1 });
        le16(&mut v, if cursor { 5 } else { *bpp });
        le32(&mut v, data.len() as u32);
        le32(&mut v, off as u32);
        off += data.len();
    }
    for (data, ..) in entries {
        v.extend_from_slice(data);
    }
    v
}

pub fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let px: Vec<u8> = (0..w * h).flat_map(|i| pat(i % w, i / w)).collect();
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
    out
}

// ---------------------------------------------------------------------------------------- TGA

/// A TGA: `ty` 1/2/3 (+8 for RLE), `depth`, origin flags (bit 5 top, bit 4 right).
pub fn tga(w: u32, h: u32, ty: u8, depth: u8, origin: u8) -> Vec<u8> {
    let mapped = ty & 7 == 1;
    let gray = ty & 7 == 3;
    let alpha_bits = match depth {
        32 => 8,
        16 if gray => 8,
        _ => 0,
    };
    let mut v = vec![0u8, mapped as u8, ty];
    le16(&mut v, 0);
    le16(&mut v, if mapped { 256 } else { 0 });
    v.push(if mapped { 24 } else { 0 });
    le16(&mut v, 0);
    le16(&mut v, 0);
    le16(&mut v, w as u16);
    le16(&mut v, h as u16);
    v.push(depth);
    v.push(alpha_bits | origin);
    if mapped {
        for c in palette(256) {
            v.extend_from_slice(&[c[2], c[1], c[0]]);
        }
    }
    let top = origin & 0x20 != 0;
    let right = origin & 0x10 != 0;
    let mut px: Vec<Vec<u8>> = Vec::new();
    for fy in 0..h {
        let y = if top { fy } else { h - 1 - fy };
        for fx in 0..w {
            let x = if right { w - 1 - fx } else { fx };
            // Runs for RLE: repeat colours in blocks of 4 columns half the time.
            let (px_x, px_y) = if (y / 4) % 2 == 0 {
                (x / 4 * 4, y)
            } else {
                (x, y)
            };
            let p = pat(px_x, px_y);
            px.push(match (mapped, gray, depth) {
                (true, ..) => vec![index_of(px_x, px_y, 256)],
                (_, true, 8) => vec![p[0]],
                (_, true, _) => vec![p[0], p[3]],
                (_, _, 16) => {
                    let c = |v: u8| (v as u16 * 31 + 127) / 255;
                    let v = (c(p[0]) << 10) | (c(p[1]) << 5) | c(p[2]) | 0x8000;
                    v.to_le_bytes().to_vec()
                }
                (_, _, 24) => vec![p[2], p[1], p[0]],
                _ => vec![p[2], p[1], p[0], p[3]],
            });
        }
    }
    if ty & 8 == 0 {
        for p in px {
            v.extend_from_slice(&p);
        }
    } else {
        // RLE packets (may cross rows).
        let mut i = 0;
        while i < px.len() {
            let mut run = 1;
            while i + run < px.len() && px[i + run] == px[i] && run < 128 {
                run += 1;
            }
            if run > 1 {
                v.push(0x80 | (run - 1) as u8);
                v.extend_from_slice(&px[i]);
                i += run;
            } else {
                let mut n = 1;
                while i + n < px.len() && n < 128 && px[i + n] != px[i + n - 1] {
                    n += 1;
                }
                v.push((n - 1) as u8);
                for p in &px[i..i + n] {
                    v.extend_from_slice(p);
                }
                i += n;
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------- PNM

/// A Netpbm file: `magic` 1–7, maxval (ignored for P1/P4), PAM depth.
pub fn pnm(w: u32, h: u32, magic: u8, maxval: u32, depth: u32) -> Vec<u8> {
    let mut v = Vec::new();
    let depth = match magic {
        1 | 2 | 4 | 5 => 1,
        3 | 6 => 3,
        _ => depth,
    };
    if magic == 7 {
        let tupl = match depth {
            1 => "GRAYSCALE",
            2 => "GRAYSCALE_ALPHA",
            3 => "RGB",
            _ => "RGB_ALPHA",
        };
        v.extend_from_slice(
            format!("P7\n# comment\nWIDTH {w}\nHEIGHT {h}\nDEPTH {depth}\nMAXVAL {maxval}\nTUPLTYPE {tupl}\nENDHDR\n")
                .as_bytes(),
        );
    } else if magic == 1 || magic == 4 {
        v.extend_from_slice(format!("P{magic}\n# a comment\n{w} {h}\n").as_bytes());
    } else {
        v.extend_from_slice(format!("P{magic} {w}\n{h}\n# c\n{maxval}\n").as_bytes());
    }
    let sample = |x: u32, y: u32, c: usize| -> u32 {
        let p = pat(x, y);
        let s = if depth <= 2 && c == 0 {
            p[1]
        } else if depth == 2 {
            p[3]
        } else {
            p[c]
        };
        (s as u32 * maxval + 127) / 255
    };
    for y in 0..h {
        match magic {
            1 => {
                for x in 0..w {
                    v.push(if pat(x, y)[0] > 128 { b'1' } else { b'0' });
                    if x % 3 == 0 {
                        v.push(b' ');
                    }
                }
                v.push(b'\n');
            }
            4 => {
                let mut row = vec![0u8; w.div_ceil(8) as usize];
                for x in 0..w {
                    if pat(x, y)[0] > 128 {
                        row[(x / 8) as usize] |= 0x80 >> (x % 8);
                    }
                }
                v.extend_from_slice(&row);
            }
            2 | 3 => {
                for x in 0..w {
                    for c in 0..depth as usize {
                        v.extend_from_slice(format!("{} ", sample(x, y, c)).as_bytes());
                    }
                }
                v.push(b'\n');
            }
            _ => {
                for x in 0..w {
                    for c in 0..depth as usize {
                        let s = sample(x, y, c);
                        if maxval > 255 {
                            v.extend_from_slice(&(s as u16).to_be_bytes());
                        } else {
                            v.push(s as u8);
                        }
                    }
                }
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------- GIF

pub struct GifFrame {
    pub left: u16,
    pub top: u16,
    pub w: u16,
    pub h: u16,
    pub dispose: gif::DisposalMethod,
    pub transparent: Option<u8>,
    pub interlaced: bool,
    pub local_palette: bool,
    pub delay: u16,
    pub seed: u32,
}

/// A GIF encoded with the `gif` crate.
pub fn gif_file(w: u16, h: u16, frames: &[GifFrame], loops: Option<u16>) -> Vec<u8> {
    let global: Vec<u8> = palette(256).into_iter().flatten().collect();
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, w, h, &global).unwrap();
        if let Some(l) = loops {
            enc.set_repeat(if l == 0 {
                gif::Repeat::Infinite
            } else {
                gif::Repeat::Finite(l)
            })
            .unwrap();
        }
        for f in frames {
            let mut fr = gif::Frame::default();
            fr.left = f.left;
            fr.top = f.top;
            fr.width = f.w;
            fr.height = f.h;
            fr.dispose = f.dispose;
            fr.transparent = f.transparent;
            fr.interlaced = f.interlaced;
            fr.delay = f.delay;
            if f.local_palette {
                let p: Vec<u8> = (0..64u32)
                    .flat_map(|i| [(i * 4) as u8, (255 - i * 3) as u8, (i * 11) as u8])
                    .collect();
                fr.palette = Some(p);
            }
            let n = if f.local_palette { 64 } else { 256 };
            let buf: Vec<u8> = (0..f.w as u32 * f.h as u32)
                .map(|i| {
                    let (x, y) = (i % f.w as u32, i / f.w as u32);
                    // Blocks of the same index (compressible) with noise.
                    (((x / 5 + y / 3 + f.seed) * 7
                        + if (x + y + f.seed) % 11 == 0 { 3 } else { 0 })
                        % n) as u8
                })
                .collect();
            fr.buffer = std::borrow::Cow::Owned(buf);
            enc.write_frame(&fr).unwrap();
        }
    }
    out
}

// ------------------------------------------------------------------------ expected outputs

/// `q` of `max` expanded to 8 bits (rounded).
pub fn expand(q: u32, max: u32) -> u8 {
    ((q * 255 + max / 2) / max) as u8
}

fn quant(c: u8, max: u32) -> u32 {
    (c as u32 * max + 127) / 255
}

/// What [`bmp`]/[`dib`] encode, as RGBA. `icon`: an ICO entry (alpha of 32-bit BI_RGB used,
/// AND mask applied).
pub fn bmp_expected(w: u32, h: u32, s: BmpSpec, icon: bool) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    let pal = if s.bpp <= 8 {
        palette(1 << s.bpp)
    } else {
        Vec::new()
    };
    for y in 0..h {
        for x in 0..w {
            let p = pat(x, y);
            let mut o = match s.bpp {
                1 | 2 | 4 | 8 => {
                    let c = pal[index_of(x, y, 1 << s.bpp) as usize];
                    [c[0], c[1], c[2], 255]
                }
                16 => {
                    let m = if s.comp == 3 {
                        s.masks
                    } else {
                        [0x7c00, 0x3e0, 0x1f, 0]
                    };
                    let f = |c: u8, m: u32| {
                        let max = m >> m.trailing_zeros();
                        expand(quant(c, max), max)
                    };
                    [f(p[0], m[0]), f(p[1], m[1]), f(p[2], m[2]), 255]
                }
                24 => [p[0], p[1], p[2], 255],
                _ => {
                    let alpha = icon || (s.header >= 56 || s.comp == 3) && s.masks[3] != 0;
                    [p[0], p[1], p[2], if alpha { p[3] } else { 255 }]
                }
            };
            if icon && s.bpp != 32 && (x + y) % 7 == 0 {
                o = [0; 4];
            }
            v.extend_from_slice(&o);
        }
    }
    v
}

/// What [`tga`] encodes, as RGBA.
pub fn tga_expected(w: u32, h: u32, ty: u8, depth: u8) -> Vec<u8> {
    let pal = palette(256);
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = if (y / 4) % 2 == 0 {
                (x / 4 * 4, y)
            } else {
                (x, y)
            };
            let p = pat(px, py);
            v.extend_from_slice(&match (ty & 7, depth) {
                (1, _) => {
                    let c = pal[index_of(px, py, 256) as usize];
                    [c[0], c[1], c[2], 255]
                }
                (3, 8) => [p[0], p[0], p[0], 255],
                (3, _) => [p[0], p[0], p[0], p[3]],
                (_, 16) => {
                    let f = |c: u8| expand(quant(c, 31), 31);
                    [f(p[0]), f(p[1]), f(p[2]), 255]
                }
                (_, 24) => [p[0], p[1], p[2], 255],
                _ => p,
            });
        }
    }
    v
}

/// What [`pnm`] encodes, as RGBA.
pub fn pnm_expected(w: u32, h: u32, magic: u8, maxval: u32, depth: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let p = pat(x, y);
            let s = |c: u8| expand(quant(c, maxval), maxval);
            v.extend_from_slice(&match (magic, depth) {
                (1 | 4, _) => {
                    let g = if p[0] > 128 { 0 } else { 255 };
                    [g, g, g, 255]
                }
                (2 | 5, _) | (7, 1) => [s(p[1]), s(p[1]), s(p[1]), 255],
                (7, 2) => [s(p[1]), s(p[1]), s(p[1]), s(p[3])],
                (3 | 6, _) | (7, 3) => [s(p[0]), s(p[1]), s(p[2]), 255],
                _ => [s(p[0]), s(p[1]), s(p[2]), s(p[3])],
            });
        }
    }
    v
}

/// Reference GIF compositor over the `gif` crate's (independently decoded, de-interlaced)
/// frames: returns every composited canvas and the stored delays.
pub fn gif_reference(data: &[u8]) -> (u32, u32, Vec<(Vec<u8>, u16)>) {
    let mut o = gif::DecodeOptions::new();
    o.set_color_output(gif::ColorOutput::RGBA);
    let mut d = o.read_info(data).unwrap();
    let (cw, ch) = (d.width() as usize, d.height() as usize);
    let mut canvas = vec![0u8; cw * ch * 4];
    let mut out = Vec::new();
    let mut pending: Option<(gif::DisposalMethod, [usize; 4], Option<Vec<u8>>)> = None;
    while let Some(f) = d.read_next_frame().unwrap() {
        if let Some((disp, [x0, y0, x1, y1], saved)) = pending.take() {
            match disp {
                gif::DisposalMethod::Background => {
                    for y in y0..y1 {
                        canvas[(y * cw + x0) * 4..(y * cw + x1) * 4].fill(0);
                    }
                }
                gif::DisposalMethod::Previous => canvas = saved.unwrap(),
                _ => {}
            }
        }
        let (l, t, fw, fh) = (
            f.left as usize,
            f.top as usize,
            f.width as usize,
            f.height as usize,
        );
        let rect = [l.min(cw), t.min(ch), (l + fw).min(cw), (t + fh).min(ch)];
        let saved = (f.dispose == gif::DisposalMethod::Previous).then(|| canvas.clone());
        for y in rect[1]..rect[3] {
            for x in rect[0]..rect[2] {
                let s = &f.buffer[((y - t) * fw + (x - l)) * 4..][..4];
                if s[3] != 0 {
                    canvas[(y * cw + x) * 4..][..4].copy_from_slice(s);
                }
            }
        }
        pending = Some((f.dispose, rect, saved));
        out.push((canvas.clone(), f.delay));
    }
    (cw as u32, ch as u32, shown_frames(out, data))
}

/// For each image of a GIF, where its graphic control extension starts (`None` without one),
/// walking the block structure.
pub fn gif_controls(data: &[u8]) -> Vec<Option<usize>> {
    gif_images(data).into_iter().map(|(c, _)| c).collect()
}

/// For each image: its graphic control extension's offset and its rectangle
/// (left, top, right, bottom).
fn gif_images(data: &[u8]) -> Vec<(Option<usize>, [u32; 4])> {
    let skip = |mut p: usize| {
        while let Some(&n) = data.get(p) {
            p += 1 + n as usize;
            if n == 0 {
                break;
            }
        }
        p
    };
    let mut p = 13;
    if data[10] & 0x80 != 0 {
        p += 3 * (2 << (data[10] & 7));
    }
    let (mut out, mut gce) = (Vec::new(), None);
    while let Some(&b) = data.get(p) {
        match b {
            0x21 => {
                if data.get(p + 1) == Some(&0xF9) {
                    gce = Some(p);
                }
                p = skip(p + 2);
            }
            0x2C => {
                let v = |i: usize| u16::from_le_bytes([data[p + 1 + i], data[p + 2 + i]]) as u32;
                let rect = [v(0), v(2), v(0) + v(4), v(2) + v(6)];
                let fl = data[p + 9];
                p += 10;
                if fl & 0x80 != 0 {
                    p += 3 * (2 << (fl & 7));
                }
                p = skip(p + 1);
                out.push((gce.take(), rect));
            }
            _ => break,
        }
    }
    out
}

/// The GIF without its graphic control extensions (images with neither delay nor disposal).
pub fn strip_controls(data: &[u8]) -> Vec<u8> {
    let mut v = data.to_vec();
    for at in gif_controls(data).into_iter().flatten().rev() {
        v.drain(at..at + 8);
    }
    v
}

/// Per-image canvases to the frames a player shows (as the pygif GIF test suite expects):
/// an image without a graphic control extension joins the next image; with none at all,
/// images that tile the picture in a file that does not loop show as the last canvas, and
/// otherwise each image is a frame.
fn shown_frames(images: Vec<(Vec<u8>, u16)>, data: &[u8]) -> Vec<(Vec<u8>, u16)> {
    let loops = data.windows(11).any(|w| w == b"NETSCAPE2.0");
    let blocks = gif_images(data);
    let controls: Vec<_> = blocks.iter().map(|(c, _)| *c).collect();
    if controls.iter().all(Option::is_none) {
        let overlap = blocks.iter().enumerate().any(|(i, (_, a))| {
            blocks[..i]
                .iter()
                .any(|(_, b)| a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3])
        });
        if loops || overlap {
            return images;
        }
        return images.into_iter().last().into_iter().collect();
    }
    let n = images.len();
    images
        .into_iter()
        .enumerate()
        .filter(|(i, _)| controls.get(*i).is_some_and(Option::is_some) || i + 1 == n)
        .map(|(_, f)| f)
        .collect()
}
