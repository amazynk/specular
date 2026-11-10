//! QuickTime image codecs inside CompressedQuickTime opcodes, other than JPEG: PNG
//! (`specular-png`), TIFF (`specular-tiff`), BMP (`WRLE`: a bottom-up DIB without its
//! headers, given to `specular-bitmap` with headers built from the image description),
//! uncompressed (`raw `: 16-bit x-5-5-5, 24-bit RGB, 32-bit ARGB, rows padded alike), and
//! the QuickTime video codecs as one key frame (`specular-oldvideo`): Animation (`rle `),
//! Graphics (`smc `), Apple Video (`rpza`) and Cinepak (`cvid`). Indexed pictures take the
//! colour table that follows the image description (colour table ID 0), else the Macintosh
//! system palette; depths 33-40 are grey.

use specular_bitmap::Decode;
use specular_oldvideo::{Codec, Decoder, Pixels};

/// Decoded straight RGBA8 and its size.
pub(crate) type Rgba = (u32, u32, Vec<u8>);

/// Whether this module decodes `codec`.
pub(crate) fn known(codec: &[u8; 4]) -> bool {
    matches!(codec, b"png " | b"tiff" | b"WRLE" | b"raw ") || video_codec(codec).is_some()
}

fn video_codec(codec: &[u8; 4]) -> Option<Codec> {
    Some(match codec {
        b"rle " => Codec::QtRle,
        b"smc " => Codec::Smc,
        b"rpza" => Codec::Rpza,
        b"cvid" => Codec::Cinepak,
        _ => return None,
    })
}

/// Why a QuickTime video codec at a depth is not drawn, in words (`None`: it is).
pub(crate) fn refusal(codec: &[u8; 4], depth: i16) -> Option<String> {
    let c = video_codec(codec)?;
    specular_oldvideo::refusal(c, depth as u16)
}

/// Decodes one image: `w`×`h` and `depth` from its image description `desc` (with the
/// colour table after it, when there is one).
pub(crate) fn decode(
    codec: &[u8; 4],
    data: &[u8],
    w: u32,
    h: u32,
    depth: i16,
    desc: &[u8],
) -> Option<Rgba> {
    if let Some(c) = video_codec(codec) {
        return video(c, data, w, h, depth, desc);
    }
    match codec {
        b"png " => {
            let img = specular_png::decode(data, &Default::default()).ok()?;
            Some((img.width, img.height, img.to_rgba8()))
        }
        b"tiff" => {
            let t = specular_tiff::Tiff::open(data).ok()?;
            let d = t.decode(data, 0, &Default::default()).ok()?;
            let px = match d.format {
                specular_tiff::PixelFormat::Gray8 => {
                    rows(&d.data, d.stride, d.width as usize, 1, |s| {
                        [s[0], s[0], s[0], 255]
                    })
                }
                specular_tiff::PixelFormat::Rgba8 => {
                    rows(&d.data, d.stride, d.width as usize, 4, |s| {
                        [s[0], s[1], s[2], s[3]]
                    })
                }
                _ => return None,
            };
            Some((d.width, d.height, px))
        }
        b"WRLE" => bmp(data, w, h, depth),
        b"raw " => raw(data, w, h, depth),
        _ => None,
    }
}

/// One key frame of a QuickTime video codec.
fn video(c: Codec, data: &[u8], w: u32, h: u32, depth: i16, desc: &[u8]) -> Option<Rgba> {
    let depth = depth as u16;
    let mut dec = Decoder::new(c, w, h, depth).ok()?;
    let px = dec.decode(data).ok()?;
    let out: Vec<u8> = match px {
        Pixels::Pal8(v) => {
            let pal = palette(desc, depth);
            v.iter()
                .flat_map(|&i| {
                    let [r, g, b] = pal[usize::from(i)];
                    [r, g, b, 255]
                })
                .collect()
        }
        Pixels::Rgb555(v) => v
            .iter()
            .flat_map(|&p| {
                let c = |sh: u16| {
                    let c = ((p >> sh) & 31) as u8;
                    (c << 3) | (c >> 2)
                };
                [c(10), c(5), c(0), 255]
            })
            .collect(),
        Pixels::Rgb24(v) => v
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        // QuickDraw draws the picture opaque.
        Pixels::Argb(v) => v
            .chunks_exact(4)
            .flat_map(|p| [p[1], p[2], p[3], 255])
            .collect(),
    };
    Some((w, h, out))
}

/// The 256 colours of an indexed picture: the image description's own table (colour table
/// ID 0), a grey ramp for depths 33-40, else the Macintosh system palette (white first,
/// black last).
fn palette(desc: &[u8], depth: u16) -> Vec<[u8; 3]> {
    let be16 = |o: usize| desc.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let mut pal = vec![[0u8; 3]; 256];
    if depth > 32 {
        let n = 1usize << (depth - 32).min(8);
        for (k, e) in pal.iter_mut().take(n).enumerate() {
            // Index 0 is white.
            let v = (255 - k * 255 / (n - 1).max(1)) as u8;
            *e = [v, v, v];
        }
        return pal;
    }
    if be16(84) == Some(0) && desc.len() > 86 + 8 {
        // Seed (4), flags (2), entries - 1 (2), then (index, r, g, b) as 16-bit values.
        let flags = be16(90).unwrap_or(0);
        let n = usize::from(be16(92).unwrap_or(0)) + 1;
        for k in 0..n.min(256) {
            let at = 94 + 8 * k;
            let (Some(i), Some(r), Some(g), Some(b)) =
                (be16(at), be16(at + 2), be16(at + 4), be16(at + 6))
            else {
                break;
            };
            let slot = if flags & 0x8000 != 0 {
                k
            } else {
                usize::from(i)
            };
            if let Some(e) = pal.get_mut(slot) {
                *e = [(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8];
            }
        }
        return pal;
    }
    // The system palette: the 6x6x6 cube from white down (black left out), ramps of red,
    // green, blue and grey without the cube's levels, then black.
    let levels = [0xFF, 0xCC, 0x99, 0x66, 0x33, 0x00];
    let mut k = 0;
    for r in levels {
        for g in levels {
            for b in levels {
                if k < 215 {
                    pal[k] = [r, g, b];
                    k += 1;
                }
            }
        }
    }
    let ramp = [0xEE, 0xDD, 0xBB, 0xAA, 0x88, 0x77, 0x55, 0x44, 0x22, 0x11];
    for c in 0..4 {
        for v in ramp {
            pal[k] = match c {
                0 => [v, 0, 0],
                1 => [0, v, 0],
                2 => [0, 0, v],
                _ => [v, v, v],
            };
            k += 1;
        }
    }
    pal[255] = [0, 0, 0];
    pal
}

/// Rows of `stride` bytes, `bpp` bytes a pixel, to RGBA through `f`.
fn rows(d: &[u8], stride: usize, w: usize, bpp: usize, f: impl Fn(&[u8]) -> [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len() / stride.max(1) * w * 4);
    for row in d.chunks(stride.max(1)) {
        for s in row.chunks_exact(bpp).take(w) {
            out.extend_from_slice(&f(s));
        }
    }
    out
}

/// The `WRLE` (Windows BMP) codec: DIB pixel rows, bottom-up, BI_RGB.
fn bmp(data: &[u8], w: u32, h: u32, depth: i16) -> Option<Rgba> {
    let bits = match depth {
        16 | 24 | 32 => depth as u16,
        _ => return None,
    };
    let mut f = Vec::with_capacity(54 + data.len());
    let size = 54 + data.len() as u32;
    f.extend_from_slice(b"BM");
    f.extend_from_slice(&size.to_le_bytes());
    f.extend_from_slice(&[0; 4]);
    f.extend_from_slice(&54u32.to_le_bytes());
    f.extend_from_slice(&40u32.to_le_bytes());
    f.extend_from_slice(&(w as i32).to_le_bytes());
    f.extend_from_slice(&(h as i32).to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&bits.to_le_bytes());
    f.extend_from_slice(&[0; 24]);
    f.extend_from_slice(data);
    let b = specular_bitmap::bmp::Bmp::parse(&f).ok()?;
    let img = b.decode(&f, &Default::default(), &|| false).ok()?;
    let px = match img.format {
        specular_bitmap::PixelFormat::Rgba8 => img.data,
        specular_bitmap::PixelFormat::Gray8 => {
            img.data.iter().flat_map(|&g| [g, g, g, 255]).collect()
        }
        _ => return None,
    };
    // Alpha in 32-bit BMP pixels is unused here.
    let mut px = px;
    for p in px.chunks_exact_mut(4) {
        p[3] = 255;
    }
    Some((img.width, img.height, px))
}

/// The `raw ` codec: big-endian pixels, rows padded to a common length.
fn raw(data: &[u8], w: u32, h: u32, depth: i16) -> Option<Rgba> {
    let bpp = match depth {
        16 => 2,
        24 => 3,
        32 => 4,
        _ => return None,
    };
    let (wu, hu) = (w as usize, h as usize);
    if wu == 0 || hu == 0 {
        return None;
    }
    let stride = (data.len() / hu).max(wu * bpp);
    let px = rows(data, stride, wu, bpp, |s| match bpp {
        2 => {
            let v = u16::from_be_bytes([s[0], s[1]]);
            let c = |sh: u16| {
                let c = ((v >> sh) & 31) as u8;
                (c << 3) | (c >> 2)
            };
            [c(10), c(5), c(0), 255]
        }
        3 => [s[0], s[1], s[2], 255],
        // ARGB; QuickDraw draws it opaque.
        _ => [s[1], s[2], s[3], 255],
    });
    let n = px.len() / 4 / wu;
    Some((w, n.min(hu) as u32, px[..n.min(hu) * wu * 4].to_vec()))
}
