//! Uncompressed video and the old palette and RLE codecs: what the container says about the
//! pictures ([`PixDesc`], carried in the track's configuration), the codec a four-character
//! code names, and the raw layouts unpacked into the player's planes.
//!
//! - AVI and Matroska VFW: BITMAPINFOHEADER bitmaps (8-bit palette, 16-bit RGB555, 24-bit BGR,
//!   32-bit BGRX), bottom-up unless the height is negative, rows padded to 4 bytes.
//! - QuickTime `raw ` (8-bit palette, 16-bit RGB555, 24-bit RGB, 32-bit ARGB, greyscale),
//!   `2vuy`, `yuvs`, `yuv2`, `24BG`, `BGRA`, `RGBA`, `ABGR`, `L555`, `b48r`, `b64a`.
//! - FourCC layouts in any container (Matroska `V_UNCOMPRESSED` names its own): `I420`,
//!   `IYUV`, `YV12`, `NV12`, `NV21`, `Y42B`, `YUY2`, `YUYV`, `YVYU`, `UYVY`, `HDYC`, `Y800`,
//!   `GREY`, `v210`, `v308`, `v408`, `v410`, `r210`, `R10k`, `Y41P`, `yuv4`.
//! - MXF uncompressed picture essence (SMPTE 384M): 8-bit 4:2:2 as UYVY, 10-bit as v210.
//!
//! YCbCr stays YCbCr; RGB, palettes and greyscale come out as G, B, R planes under the
//! identity matrix; alpha goes beside the picture.

use specular_decode::DecodeError;
use specular_hwdec::video::{Chroma, Signal, VideoFrame};

use crate::mp4::{Classic, Codec};

/// What the container says about a raw or old-codec picture: carried as the track's
/// configuration ([`PixDesc::to_config`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PixDesc {
    /// The four-character code (AVI's compression, QuickTime's sample entry, Matroska's
    /// colour space). `[0; 4]` is an AVI bitmap.
    pub fourcc: [u8; 4],
    /// Bits per pixel (AVI `biBitCount`, QuickTime depth).
    pub depth: u16,
    /// Rows run bottom to top (AVI bitmaps with a positive height).
    pub bottom_up: bool,
    /// The colour table, RGB.
    pub palette: Vec<[u8; 3]>,
}

const MAGIC: &[u8; 4] = b"PXD1";

/// The code MXF gives uncompressed essence in a packing the player doesn't unpack.
pub const MXF_OTHER: &[u8; 4] = b"MXF?";

impl PixDesc {
    pub fn to_config(&self) -> Vec<u8> {
        let mut c = MAGIC.to_vec();
        c.extend(self.fourcc);
        c.extend(self.depth.to_le_bytes());
        c.push(u8::from(self.bottom_up));
        c.extend((self.palette.len() as u16).to_le_bytes());
        for p in &self.palette {
            c.extend(p);
        }
        c
    }

    pub fn from_config(c: &[u8]) -> Option<PixDesc> {
        if c.get(..4)? != MAGIC {
            return None;
        }
        let n = usize::from(u16::from_le_bytes([*c.get(11)?, *c.get(12)?]));
        let pal = c.get(13..13 + 3 * n)?;
        Some(PixDesc {
            fourcc: c[4..8].try_into().ok()?,
            depth: u16::from_le_bytes([c[8], c[9]]),
            bottom_up: c[10] != 0,
            palette: pal.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect(),
        })
    }

    /// From a BITMAPINFOHEADER and the colour table after it (AVI `strf`, Matroska VFW).
    pub fn from_bitmap_info(bih: &[u8]) -> PixDesc {
        let u32le = |o: usize| {
            bih.get(o..o + 4)
                .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let size = (u32le(0) as usize).clamp(40, bih.len().max(40));
        let height = u32le(8) as i32;
        let depth = bih
            .get(14..16)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        let fourcc: [u8; 4] = bih
            .get(16..20)
            .and_then(|c| c.try_into().ok())
            .unwrap_or_default();
        let used = u32le(32) as usize;
        let mut palette = Vec::new();
        if depth <= 8 && depth > 0 {
            let n = if used > 0 { used.min(256) } else { 1 << depth };
            if let Some(t) = bih.get(size..) {
                palette = t
                    .chunks_exact(4)
                    .take(n)
                    .map(|q| [q[2], q[1], q[0]])
                    .collect();
            }
        }
        // Bitmaps and the RLE codings are bottom-up unless the height is negative.
        let dib = matches!(&fourcc, [0..=3, 0, 0, 0] | b"DIB " | b"RGB " | b"RAW ");
        PixDesc {
            fourcc,
            depth,
            bottom_up: dib && height > 0,
            palette,
        }
    }
}

/// The length of the colour table inside a QuickTime visual sample entry (`body`: the entry
/// after its box header): present when the colour table ID (offset 76) is 0 at a depth of
/// 1–8 bits or greyscale.
pub fn qt_inline_table_len(body: &[u8]) -> usize {
    let be16 = |o: usize| body.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let (Some(depth), Some(id)) = (be16(74), be16(76)) else {
        return 0;
    };
    if id != 0 || !matches!(depth & 31, 1 | 2 | 4 | 8) {
        return 0;
    }
    // Seed, flags, size (count - 1), then 8 bytes per entry.
    be16(84).map_or(0, |n| {
        (8 + 8 * (usize::from(n) + 1)).min(body.len().saturating_sub(78))
    })
}

/// The colour table of a QuickTime visual sample entry: inline (ID 0), else the Mac system
/// palette at 8 bits. Empty when there is none (greyscale builds its own ramp).
pub fn qt_palette(body: &[u8]) -> Vec<[u8; 3]> {
    let be16 = |o: usize| body.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let (Some(depth), Some(id)) = (be16(74), be16(76)) else {
        return Vec::new();
    };
    if depth > 8 {
        return Vec::new();
    }
    if id == 0 && qt_inline_table_len(body) > 0 {
        let n = usize::from(be16(84).unwrap_or(0)) + 1;
        let mut pal = vec![[0u8; 3]; n.min(256)];
        for k in 0..n {
            let at = 86 + 8 * k;
            let (Some(i), Some(r), Some(g), Some(b)) =
                (be16(at), be16(at + 2), be16(at + 4), be16(at + 6))
            else {
                break;
            };
            // Indexed tables give the slot; device tables (flags 0x8000) go in order.
            let slot = if be16(82).unwrap_or(0) & 0x8000 != 0 {
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
    if depth == 8 {
        return mac_palette();
    }
    Vec::new()
}

/// The Macintosh 8-bit system palette: the 6×6×6 cube from white down (without black),
/// ten-step ramps of red, green, blue and grey, then black.
fn mac_palette() -> Vec<[u8; 3]> {
    let levels = [0xFF, 0xCC, 0x99, 0x66, 0x33, 0x00];
    let mut p = Vec::with_capacity(256);
    for r in levels {
        for g in levels {
            for b in levels {
                p.push([r, g, b]);
            }
        }
    }
    p.pop();
    let ramp = [0xEE, 0xDD, 0xBB, 0xAA, 0x88, 0x77, 0x55, 0x44, 0x22, 0x11];
    for k in 0..4 {
        for v in ramp {
            p.push(match k {
                0 => [v, 0, 0],
                1 => [0, v, 0],
                2 => [0, 0, v],
                _ => [v, v, v],
            });
        }
    }
    p.push([0, 0, 0]);
    p
}

/// The codec a four-character code names when it is raw video or one of the old codecs
/// (case matters: `yuvs` and `YUVS` differ).
pub fn codec_of(fourcc: [u8; 4]) -> Option<Codec> {
    Some(match &fourcc {
        [0, 0, 0, 0] | b"DIB " | b"RGB " | b"RAW " | b"raw " => Codec::Raw,
        [1, 0, 0, 0] | [2, 0, 0, 0] | b"mrle" | b"MRLE" | b"RLE8" | b"RLE4" => {
            Codec::Classic(Classic::MsRle)
        }
        b"cvid" | b"CVID" => Codec::Classic(Classic::Cinepak),
        b"MSVC" | b"msvc" | b"CRAM" | b"cram" | b"WHAM" | b"wham" => {
            Codec::Classic(Classic::MsVideo1)
        }
        b"rle " => Codec::Classic(Classic::QtRle),
        b"smc " => Codec::Classic(Classic::Smc),
        b"rpza" | b"azpr" => Codec::Classic(Classic::Rpza),
        _ if layout(fourcc, 0).is_some() => Codec::Raw,
        _ => return None,
    })
}

/// A raw layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    /// AVI bitmap: 8 (palette), 16 (RGB555), 24 (BGR) or 32 (BGRX) bits, little-endian.
    Dib(u16),
    /// QuickTime `raw `: 8 (palette), 16 (RGB555 big-endian), 24 (RGB), 32 (ARGB).
    QtRaw(u16),
    /// Planar 4:2:0, the second plane Cr when swapped (YV12).
    I420 {
        swap: bool,
    },
    /// Luma then interleaved chroma (NV12: Cb first).
    Nv {
        swap: bool,
    },
    /// Planar 4:2:2.
    Y42b,
    /// Packed 4:2:2: byte offsets of Y0, Cb, Y1, Cr in each 4 bytes; chroma stored signed.
    Packed422([usize; 4], bool),
    Grey,
    /// Packed 24-bit: offsets of R, G, B.
    Rgb24([usize; 3]),
    /// Packed 32-bit: offsets of R, G, B and alpha.
    Rgb32([usize; 4]),
    /// RGB555 little-endian.
    L555,
    B48r,
    B64a,
    V210,
    V308,
    V408,
    V410,
    R210,
    R10k,
    Y41p,
    Yuv4,
    /// Planar Y, Cb, Cr (YUV4MPEG2's): chroma subsampled by 2 across (`.0`) and down
    /// (`.1`), samples of `.2` bits (two bytes, little-endian, above 8).
    Planar(bool, bool, u8),
}

/// The four-character code of a planar layout ([`Layout::Planar`]): `Y4` and the chroma
/// (`0` 4:2:0, `2` 4:2:2, `4` 4:4:4), then the bits (`8`, `9`, `A` 10, `C` 12, `G` 16).
pub fn planar_fourcc(chroma: Chroma, bits: u8) -> Option<[u8; 4]> {
    let c = match chroma {
        Chroma::Yuv420 => b'0',
        Chroma::Yuv422 => b'2',
        Chroma::Yuv444 => b'4',
        _ => return None,
    };
    let b = match bits {
        8 => b'8',
        9 => b'9',
        10 => b'A',
        12 => b'C',
        14 => b'E',
        16 => b'G',
        _ => return None,
    };
    Some([b'Y', b'4', c, b])
}

/// The layout of a four-character code at a depth (0: unknown).
fn layout(fourcc: [u8; 4], depth: u16) -> Option<Layout> {
    Some(match &fourcc {
        [0, 0, 0, 0] | b"DIB " | b"RGB " | b"RAW " => Layout::Dib(depth),
        b"raw " => Layout::QtRaw(depth),
        b"I420" | b"IYUV" | b"i420" => Layout::I420 { swap: false },
        b"YV12" => Layout::I420 { swap: true },
        b"NV12" => Layout::Nv { swap: false },
        b"NV21" => Layout::Nv { swap: true },
        b"Y42B" => Layout::Y42b,
        b"YUY2" | b"YUYV" | b"yuvs" | b"YUNV" | b"V422" => Layout::Packed422([0, 1, 2, 3], false),
        b"yuv2" => Layout::Packed422([0, 1, 2, 3], true),
        b"YVYU" => Layout::Packed422([0, 3, 2, 1], false),
        b"UYVY" | b"2vuy" | b"HDYC" | b"UYNV" | b"Y422" => Layout::Packed422([1, 0, 3, 2], false),
        b"Y800" | b"GREY" | b"Y8  " | b"GRAY" => Layout::Grey,
        b"24BG" => Layout::Rgb24([2, 1, 0]),
        b"RGBA" => Layout::Rgb32([0, 1, 2, 3]),
        b"BGRA" => Layout::Rgb32([2, 1, 0, 3]),
        b"ABGR" => Layout::Rgb32([3, 2, 1, 0]),
        b"ARGB" => Layout::Rgb32([1, 2, 3, 0]),
        b"L555" => Layout::L555,
        b"b48r" => Layout::B48r,
        b"b64a" => Layout::B64a,
        b"v210" => Layout::V210,
        b"v308" => Layout::V308,
        b"v408" => Layout::V408,
        b"v410" => Layout::V410,
        b"r210" => Layout::R210,
        b"R10k" | b"R10g" => Layout::R10k,
        b"Y41P" => Layout::Y41p,
        b"yuv4" => Layout::Yuv4,
        [b'Y', b'4', c @ (b'0' | b'2' | b'4'), b] => {
            let bits = match b {
                b'8' => 8,
                b'9' => 9,
                b'A' => 10,
                b'C' => 12,
                b'E' => 14,
                b'G' => 16,
                _ => return None,
            };
            Layout::Planar(*c != b'4', *c == b'0', bits)
        }
        _ => return None,
    })
}

/// The name of a raw layout, for the Info panel ("UYVY 4:2:2", "v210 10-bit 4:2:2").
pub fn describe(d: &PixDesc) -> Option<String> {
    let l = layout(d.fourcc, d.depth)?;
    let code = String::from_utf8_lossy(&d.fourcc).trim().to_owned();
    Some(match l {
        Layout::Dib(8) | Layout::QtRaw(8) => "8-bit palette".into(),
        Layout::Dib(16) | Layout::QtRaw(16) | Layout::L555 => "RGB555".into(),
        Layout::Dib(24) | Layout::QtRaw(24) => "24-bit RGB".into(),
        Layout::Dib(32) => "32-bit RGB".into(),
        Layout::QtRaw(32) => "32-bit ARGB".into(),
        Layout::QtRaw(40) => "8-bit greyscale".into(),
        Layout::Dib(n) | Layout::QtRaw(n) => format!("{n}-bit"),
        Layout::I420 { .. } | Layout::Nv { .. } | Layout::Yuv4 => format!("{code} 4:2:0"),
        Layout::Y42b | Layout::Packed422(..) => format!("{code} 4:2:2"),
        Layout::Grey => format!("{code} greyscale"),
        Layout::V210 => "v210 10-bit 4:2:2".into(),
        Layout::V410 => "v410 10-bit 4:4:4".into(),
        Layout::V308 => "v308 4:4:4".into(),
        Layout::V408 => "v408 4:4:4 with alpha".into(),
        Layout::R210 | Layout::R10k => format!("{code} 10-bit RGB"),
        Layout::B48r => "b48r 16-bit RGB".into(),
        Layout::B64a => "b64a 16-bit RGB with alpha".into(),
        Layout::Y41p => "Y41P 4:1:1".into(),
        Layout::Rgb24(_) | Layout::Rgb32(_) => format!("{code} RGB"),
        Layout::Planar(x, y, bits) => format!(
            "planar {}, {bits}-bit",
            match (x, y) {
                (true, true) => "4:2:0",
                (true, false) => "4:2:2",
                _ => "4:4:4",
            }
        ),
    })
}

/// Why a raw track can't play, in words.
pub fn refusal(d: &PixDesc) -> Option<String> {
    let code = String::from_utf8_lossy(&d.fourcc).trim().to_owned();
    if &d.fourcc == MXF_OTHER {
        return Some(format!(
            "{}-bit uncompressed MXF video in this packing",
            d.depth
        ));
    }
    match layout(d.fourcc, d.depth) {
        None => Some(format!("uncompressed video in the {code:?} layout")),
        Some(Layout::Dib(n)) if !matches!(n, 8 | 16 | 24 | 32) => {
            Some(format!("{n}-bit uncompressed video"))
        }
        Some(Layout::QtRaw(n)) if !matches!(n, 8 | 16 | 24 | 32 | 40) => {
            Some(format!("{n}-bit uncompressed video"))
        }
        Some(Layout::Dib(8) | Layout::QtRaw(8)) if d.palette.is_empty() => {
            Some("palette video without its colour table".into())
        }
        _ => None,
    }
}

/// What a raw frame unpacks to, before it becomes a [`VideoFrame`].
pub struct Unpacked {
    pub frame: VideoFrame,
    /// The alpha plane at the frame's depth, when the layout has one.
    pub alpha: Option<Vec<u8>>,
}

fn rgb_signal() -> Option<Signal> {
    Some(Signal {
        full_range: true,
        primaries: Some(1),
        transfer: Some(13),
        matrix: Some(0),
    })
}

/// G, B, R planes (and alpha) built pixel by pixel, 8 or 16 bits.
struct RgbOut {
    w: usize,
    deep: bool,
    g: Vec<u8>,
    b: Vec<u8>,
    r: Vec<u8>,
    a: Option<Vec<u8>>,
}

impl RgbOut {
    fn new(w: usize, h: usize, deep: bool, alpha: bool) -> RgbOut {
        let n = w * h * if deep { 2 } else { 1 };
        RgbOut {
            w,
            deep,
            g: vec![0; n],
            b: vec![0; n],
            r: vec![0; n],
            a: alpha.then(|| vec![0; n]),
        }
    }

    #[inline(always)]
    fn put8(&mut self, i: usize, r: u8, g: u8, b: u8) {
        self.r[i] = r;
        self.g[i] = g;
        self.b[i] = b;
    }

    #[inline(always)]
    fn put16(&mut self, i: usize, r: u16, g: u16, b: u16, a: u16) {
        self.r[2 * i..2 * i + 2].copy_from_slice(&r.to_le_bytes());
        self.g[2 * i..2 * i + 2].copy_from_slice(&g.to_le_bytes());
        self.b[2 * i..2 * i + 2].copy_from_slice(&b.to_le_bytes());
        if let Some(al) = &mut self.a {
            al[2 * i..2 * i + 2].copy_from_slice(&a.to_le_bytes());
        }
    }

    fn finish(self, h: usize, bits: u8) -> Unpacked {
        let _ = self.deep;
        Unpacked {
            frame: VideoFrame {
                width: self.w as u32,
                height: h as u32,
                bit_depth: bits,
                chroma: Chroma::Yuv444,
                planes: [self.g, self.b, self.r],
                signal: rgb_signal(),
            },
            alpha: self.a,
        }
    }
}

fn yuv(w: usize, h: usize, bits: u8, chroma: Chroma, planes: [Vec<u8>; 3]) -> Unpacked {
    Unpacked {
        frame: VideoFrame {
            width: w as u32,
            height: h as u32,
            bit_depth: bits,
            chroma,
            planes,
            signal: None,
        },
        alpha: None,
    }
}

fn short(what: &str) -> DecodeError {
    DecodeError::Malformed(format!("{what} frame shorter than its picture"))
}

/// The row stride of a packed layout: the frame's size over its height when that is a whole
/// number of bytes at least `min` (padding varies by writer), else `min`.
fn stride(len: usize, h: usize, min: usize) -> usize {
    if h > 0 && len.is_multiple_of(h) && len / h >= min {
        len / h
    } else {
        min
    }
}

/// Unpacks one raw frame.
pub fn unpack(d: &PixDesc, w: u32, h: u32, data: &[u8]) -> Result<Unpacked, DecodeError> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 {
        return Err(DecodeError::Malformed("raw video without a size".into()));
    }
    let l = layout(d.fourcc, d.depth).ok_or_else(|| {
        DecodeError::Unsupported(refusal(d).unwrap_or_else(|| "this raw layout".into()))
    })?;
    let _held = specular_core::budget::reserve((w * h * 8) as u64)?;
    let row_of = |y: usize| if d.bottom_up { h - 1 - y } else { y };
    let (cw2, ch2) = (w.div_ceil(2), h.div_ceil(2));
    Ok(match l {
        Layout::Dib(depth) | Layout::QtRaw(depth) => {
            let qt = matches!(l, Layout::QtRaw(_));
            let bpp = match depth {
                8 | 40 => 1,
                16 => 2,
                24 => 3,
                32 => 4,
                n => {
                    return Err(DecodeError::Unsupported(format!(
                        "{n}-bit uncompressed video"
                    )));
                }
            };
            let min = if qt {
                w * bpp
            } else {
                (w * bpp).div_ceil(4) * 4
            };
            let s = stride(data.len(), h, min);
            if data.len() < s * (h - 1) + w * bpp {
                return Err(short("raw"));
            }
            let alpha = qt && depth == 32;
            let mut o = RgbOut::new(w, h, false, alpha);
            let grey: Vec<[u8; 3]>;
            let pal: &[[u8; 3]] = if depth == 40 {
                // QuickTime's 8-bit greyscale: 0 is white.
                grey = (0..=255u8).map(|i| [255 - i; 3]).collect();
                &grey
            } else {
                &d.palette
            };
            if depth == 24 || depth == 32 && !alpha {
                // The common bitmaps, a row at a time.
                // QuickTime RGB, or the bitmaps' BGR (32-bit: BGRX; QuickTime's ARGB has
                // alpha and goes the long way).
                let (ri, gi, bi) = if qt { (0, 1, 2) } else { (2, 1, 0) };
                let bpp = usize::from(depth / 8);
                for y in 0..h {
                    let src = &data[row_of(y) * s..row_of(y) * s + w * bpp];
                    let rows = (y * w)..(y * w + w);
                    let (g, b, r) = (
                        &mut o.g[rows.clone()],
                        &mut o.b[rows.clone()],
                        &mut o.r[rows],
                    );
                    for (((p, g), b), r) in src.chunks_exact(bpp).zip(g).zip(b).zip(r) {
                        (*r, *g, *b) = (p[ri], p[gi], p[bi]);
                    }
                }
                return Ok(o.finish(h, 8));
            }
            for y in 0..h {
                let src = &data[row_of(y) * s..];
                for x in 0..w {
                    let i = y * w + x;
                    match depth {
                        8 | 40 => {
                            let c = pal.get(usize::from(src[x])).copied().unwrap_or_default();
                            o.put8(i, c[0], c[1], c[2]);
                        }
                        16 => {
                            let v = if qt {
                                u16::from_be_bytes([src[2 * x], src[2 * x + 1]])
                            } else {
                                u16::from_le_bytes([src[2 * x], src[2 * x + 1]])
                            };
                            let five = |c: u16| ((c << 3) | (c >> 2)) as u8;
                            o.put8(i, five(v >> 10 & 31), five(v >> 5 & 31), five(v & 31));
                        }
                        24 => {
                            let p = &src[3 * x..3 * x + 3];
                            if qt {
                                o.put8(i, p[0], p[1], p[2]);
                            } else {
                                o.put8(i, p[2], p[1], p[0]);
                            }
                        }
                        _ => {
                            let p = &src[4 * x..4 * x + 4];
                            if qt {
                                o.put8(i, p[1], p[2], p[3]);
                                if let Some(a) = &mut o.a {
                                    a[i] = p[0];
                                }
                            } else {
                                o.put8(i, p[2], p[1], p[0]);
                            }
                        }
                    }
                }
            }
            o.finish(h, 8)
        }
        Layout::I420 { swap } => {
            let (ys, cs) = (w * h, cw2 * ch2);
            if data.len() < ys + 2 * cs {
                return Err(short("I420"));
            }
            let (a, b) = (
                data[ys..ys + cs].to_vec(),
                data[ys + cs..ys + 2 * cs].to_vec(),
            );
            let (cb, cr) = if swap { (b, a) } else { (a, b) };
            yuv(w, h, 8, Chroma::Yuv420, [data[..ys].to_vec(), cb, cr])
        }
        Layout::Nv { swap } => {
            let ys = w * h;
            let cs = 2 * cw2 * ch2;
            if data.len() < ys + cs {
                return Err(short("NV12"));
            }
            let uv = &data[ys..ys + cs];
            let first: Vec<u8> = uv.iter().step_by(2).copied().collect();
            let second: Vec<u8> = uv.iter().skip(1).step_by(2).copied().collect();
            let (cb, cr) = if swap {
                (second, first)
            } else {
                (first, second)
            };
            yuv(w, h, 8, Chroma::Yuv420, [data[..ys].to_vec(), cb, cr])
        }
        Layout::Planar(sx, sy, bits) => {
            let bytes = if bits > 8 { 2 } else { 1 };
            let cw = if sx { cw2 } else { w };
            let chh = if sy { ch2 } else { h };
            let (ys, cs) = (w * h * bytes, cw * chh * bytes);
            if data.len() < ys + 2 * cs {
                return Err(short("planar YCbCr"));
            }
            let chroma = match (sx, sy) {
                (true, true) => Chroma::Yuv420,
                (true, false) => Chroma::Yuv422,
                _ => Chroma::Yuv444,
            };
            yuv(
                w,
                h,
                bits,
                chroma,
                [
                    data[..ys].to_vec(),
                    data[ys..ys + cs].to_vec(),
                    data[ys + cs..ys + 2 * cs].to_vec(),
                ],
            )
        }
        Layout::Y42b => {
            let (ys, cs) = (w * h, cw2 * h);
            if data.len() < ys + 2 * cs {
                return Err(short("Y42B"));
            }
            yuv(
                w,
                h,
                8,
                Chroma::Yuv422,
                [
                    data[..ys].to_vec(),
                    data[ys..ys + cs].to_vec(),
                    data[ys + cs..ys + 2 * cs].to_vec(),
                ],
            )
        }
        Layout::Packed422([y0, u, y1, v], signed) => {
            let s = stride(data.len(), h, 2 * cw2 * 2);
            if data.len() < s * (h - 1) + 4 * cw2 {
                return Err(short("4:2:2"));
            }
            let flip = if signed { 0x80 } else { 0 };
            let (mut yp, mut cb, mut cr) =
                (vec![0u8; w * h], vec![0u8; cw2 * h], vec![0u8; cw2 * h]);
            for y in 0..h {
                let src = &data[y * s..];
                for k in 0..cw2 {
                    let q = &src[4 * k..4 * k + 4];
                    yp[y * w + 2 * k] = q[y0];
                    if 2 * k + 1 < w {
                        yp[y * w + 2 * k + 1] = q[y1];
                    }
                    cb[y * cw2 + k] = q[u] ^ flip;
                    cr[y * cw2 + k] = q[v] ^ flip;
                }
            }
            yuv(w, h, 8, Chroma::Yuv422, [yp, cb, cr])
        }
        Layout::Grey => {
            if data.len() < w * h {
                return Err(short("greyscale"));
            }
            let g = data[..w * h].to_vec();
            Unpacked {
                frame: VideoFrame {
                    width: w as u32,
                    height: h as u32,
                    bit_depth: 8,
                    chroma: Chroma::Yuv444,
                    planes: [g.clone(), g.clone(), g],
                    signal: rgb_signal(),
                },
                alpha: None,
            }
        }
        Layout::Rgb24([r, g, b]) => {
            let s = stride(data.len(), h, 3 * w);
            if data.len() < s * (h - 1) + 3 * w {
                return Err(short("RGB"));
            }
            let mut o = RgbOut::new(w, h, false, false);
            for y in 0..h {
                let src = &data[y * s..];
                for x in 0..w {
                    let p = &src[3 * x..];
                    o.put8(y * w + x, p[r], p[g], p[b]);
                }
            }
            o.finish(h, 8)
        }
        Layout::Rgb32([r, g, b, a]) => {
            let s = stride(data.len(), h, 4 * w);
            if data.len() < s * (h - 1) + 4 * w {
                return Err(short("RGB"));
            }
            let mut o = RgbOut::new(w, h, false, true);
            for y in 0..h {
                let src = &data[y * s..];
                for x in 0..w {
                    let p = &src[4 * x..];
                    let i = y * w + x;
                    o.put8(i, p[r], p[g], p[b]);
                    if let Some(al) = &mut o.a {
                        al[i] = p[a];
                    }
                }
            }
            o.finish(h, 8)
        }
        Layout::L555 => {
            let d2 = PixDesc {
                fourcc: [0; 4],
                depth: 16,
                bottom_up: false,
                palette: Vec::new(),
            };
            // Unpadded rows: the bitmap path takes the stride from the frame's size.
            let s = stride(data.len(), h, 2 * w);
            let mut v = Vec::with_capacity(h * (2 * w).div_ceil(4) * 4);
            for y in 0..h {
                let row = data
                    .get(y * s..y * s + 2 * w)
                    .ok_or_else(|| short("RGB555"))?;
                v.extend_from_slice(row);
                v.resize(v.len().div_ceil(4) * 4, 0);
            }
            return unpack(&d2, w as u32, h as u32, &v);
        }
        Layout::B48r | Layout::B64a => {
            let alpha = l == Layout::B64a;
            let bpp = if alpha { 8 } else { 6 };
            let s = stride(data.len(), h, bpp * w);
            if data.len() < s * (h - 1) + bpp * w {
                return Err(short("16-bit RGB"));
            }
            let mut o = RgbOut::new(w, h, true, alpha);
            for y in 0..h {
                let src = &data[y * s..];
                for x in 0..w {
                    let p = &src[bpp * x..bpp * x + bpp];
                    let be = |k: usize| u16::from_be_bytes([p[2 * k], p[2 * k + 1]]);
                    if alpha {
                        o.put16(y * w + x, be(1), be(2), be(3), be(0));
                    } else {
                        o.put16(y * w + x, be(0), be(1), be(2), 0);
                    }
                }
            }
            o.finish(h, 16)
        }
        Layout::R210 | Layout::R10k => {
            let s = stride(data.len(), h, 4 * w);
            if data.len() < s * (h - 1) + 4 * w {
                return Err(short("10-bit RGB"));
            }
            let mut o = RgbOut::new(w, h, true, false);
            for y in 0..h {
                let src = &data[y * s..];
                for x in 0..w {
                    let v = u32::from_be_bytes([
                        src[4 * x],
                        src[4 * x + 1],
                        src[4 * x + 2],
                        src[4 * x + 3],
                    ]);
                    let v = if l == Layout::R10k { v >> 2 } else { v };
                    let c = |sh: u32| (v >> sh & 0x3FF) as u16;
                    o.put16(y * w + x, c(20), c(10), c(0), 0);
                }
            }
            o.finish(h, 10)
        }
        Layout::V210 => {
            let min = w.div_ceil(48) * 128;
            let s = stride(data.len(), h, min);
            if data.len() < s * (h - 1) + (w.div_ceil(6) * 16).min(s) {
                return Err(short("v210"));
            }
            let (mut yp, mut cb, mut cr) = (
                vec![0u8; 2 * w * h],
                vec![0u8; 2 * cw2 * h],
                vec![0u8; 2 * cw2 * h],
            );
            let put = |p: &mut [u8], i: usize, v: u32| {
                p[2 * i..2 * i + 2].copy_from_slice(&((v & 0x3FF) as u16).to_le_bytes());
            };
            for y in 0..h {
                let src = &data[y * s..];
                let (mut yi, mut ci) = (0usize, 0usize);
                for blk in src.chunks_exact(16).take(w.div_ceil(6)) {
                    let wd = |k: usize| {
                        u32::from_le_bytes([
                            blk[4 * k],
                            blk[4 * k + 1],
                            blk[4 * k + 2],
                            blk[4 * k + 3],
                        ])
                    };
                    let (a, b, c, e) = (wd(0), wd(1), wd(2), wd(3));
                    // Cb0 Y0 Cr0 | Y1 Cb1 Y2 | Cr1 Y3 Cb2 | Y4 Cr2 Y5
                    let ys = [a >> 10, b, b >> 20, c >> 10, e, e >> 20];
                    let cbs = [a, b >> 10, c >> 20];
                    let crs = [a >> 20, c, e >> 10];
                    for v in ys {
                        if yi < w {
                            put(&mut yp, y * w + yi, v);
                        }
                        yi += 1;
                    }
                    for k in 0..3 {
                        if ci < cw2 {
                            put(&mut cb, y * cw2 + ci, cbs[k]);
                            put(&mut cr, y * cw2 + ci, crs[k]);
                        }
                        ci += 1;
                    }
                }
            }
            yuv(w, h, 10, Chroma::Yuv422, [yp, cb, cr])
        }
        Layout::V410 => {
            let s = stride(data.len(), h, 4 * w);
            if data.len() < s * (h - 1) + 4 * w {
                return Err(short("v410"));
            }
            let mut p = [
                vec![0u8; 2 * w * h],
                vec![0u8; 2 * w * h],
                vec![0u8; 2 * w * h],
            ];
            for y in 0..h {
                let src = &data[y * s..];
                for x in 0..w {
                    let v = u32::from_le_bytes([
                        src[4 * x],
                        src[4 * x + 1],
                        src[4 * x + 2],
                        src[4 * x + 3],
                    ]);
                    let i = 2 * (y * w + x);
                    for (k, sh) in [(0, 12), (1, 2), (2, 22)] {
                        p[k][i..i + 2].copy_from_slice(&((v >> sh & 0x3FF) as u16).to_le_bytes());
                    }
                }
            }
            yuv(w, h, 10, Chroma::Yuv444, p)
        }
        Layout::V308 | Layout::V408 => {
            let alpha = l == Layout::V408;
            let bpp = if alpha { 4 } else { 3 };
            let s = stride(data.len(), h, bpp * w);
            if data.len() < s * (h - 1) + bpp * w {
                return Err(short("packed 4:4:4"));
            }
            let mut p = [vec![0u8; w * h], vec![0u8; w * h], vec![0u8; w * h]];
            let mut a = alpha.then(|| vec![0u8; w * h]);
            // v308: Cr Y Cb; v408: Cb Y Cr A.
            let (yo, uo, vo) = if alpha { (1, 0, 2) } else { (1, 2, 0) };
            for y in 0..h {
                let src = &data[y * s..];
                for x in 0..w {
                    let q = &src[bpp * x..bpp * x + bpp];
                    let i = y * w + x;
                    p[0][i] = q[yo];
                    p[1][i] = q[uo];
                    p[2][i] = q[vo];
                    if let Some(a) = &mut a {
                        a[i] = q[3];
                    }
                }
            }
            let mut u = yuv(w, h, 8, Chroma::Yuv444, p);
            u.alpha = a;
            u
        }
        Layout::Y41p => {
            let groups = w.div_ceil(8);
            let s = stride(data.len(), h, 12 * groups);
            if data.len() < s * (h - 1) + 12 * groups {
                return Err(short("Y41P"));
            }
            let cw = w.div_ceil(4);
            let (mut yp, mut cb, mut cr) = (vec![0u8; w * h], vec![0u8; cw * h], vec![0u8; cw * h]);
            for y in 0..h {
                // Rows run bottom to top.
                let src = &data[(h - 1 - y) * s..];
                for g in 0..groups {
                    let q = &src[12 * g..12 * g + 12];
                    // U0 Y0 V0 Y1 U4 Y2 V4 Y3 Y4 Y5 Y6 Y7
                    let ys = [q[1], q[3], q[5], q[7], q[8], q[9], q[10], q[11]];
                    for (k, &v) in ys.iter().enumerate() {
                        if 8 * g + k < w {
                            yp[y * w + 8 * g + k] = v;
                        }
                    }
                    for (k, (u, v)) in [(q[0], q[2]), (q[4], q[6])].into_iter().enumerate() {
                        if 2 * g + k < cw {
                            cb[y * cw + 2 * g + k] = u;
                            cr[y * cw + 2 * g + k] = v;
                        }
                    }
                }
            }
            yuv(w, h, 8, Chroma::Yuv411, [yp, cb, cr])
        }
        Layout::Yuv4 => {
            if data.len() < 6 * cw2 * ch2 {
                return Err(short("yuv4"));
            }
            let (mut yp, mut cb, mut cr) =
                (vec![0u8; w * h], vec![0u8; cw2 * ch2], vec![0u8; cw2 * ch2]);
            // Each 2×2 block: Cb, Cr (stored signed), then the four luma samples.
            for by in 0..ch2 {
                for bx in 0..cw2 {
                    let q = &data[6 * (by * cw2 + bx)..];
                    cb[by * cw2 + bx] = q[0] ^ 0x80;
                    cr[by * cw2 + bx] = q[1] ^ 0x80;
                    for (k, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                        let (x, y) = (2 * bx + dx, 2 * by + dy);
                        if x < w && y < h {
                            yp[y * w + x] = q[2 + k];
                        }
                    }
                }
            }
            yuv(w, h, 8, Chroma::Yuv420, [yp, cb, cr])
        }
    })
}

/// One raw track's decoder: the description, the size, and the last alpha plane.
pub struct RawDec {
    desc: PixDesc,
    width: u32,
    height: u32,
    alpha: Option<Vec<u8>>,
}

impl RawDec {
    pub fn new(track: &crate::mp4::VideoTrack) -> Result<RawDec, crate::play::PlayError> {
        let desc = PixDesc::from_config(&track.config).ok_or_else(|| {
            crate::play::PlayError::Malformed("raw video without a description".into())
        })?;
        if let Some(why) = refusal(&desc) {
            return Err(crate::play::PlayError::Unsupported(why));
        }
        Ok(RawDec {
            desc,
            width: track.width,
            height: track.height,
            alpha: None,
        })
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Option<VideoFrame>, DecodeError> {
        match unpack(&self.desc, self.width, self.height, data) {
            Ok(u) => {
                self.alpha = u.alpha;
                Ok(Some(u.frame))
            }
            Err(DecodeError::Malformed(m)) => {
                specular_log::debug!(target: specular_log::DECODE, "video: raw frame lost: {m}");
                self.alpha = None;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    pub fn take_alpha(&mut self) -> Option<Vec<u8>> {
        self.alpha.take()
    }
}

/// Whether a codec is raw video or an old codec whose description is a [`PixDesc`].
pub fn described(codec: Codec) -> bool {
    matches!(codec, Codec::Raw | Codec::Classic(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let d = PixDesc {
            fourcc: *b"raw ",
            depth: 8,
            bottom_up: true,
            palette: vec![[1, 2, 3], [250, 251, 252]],
        };
        assert_eq!(PixDesc::from_config(&d.to_config()), Some(d));
        assert_eq!(PixDesc::from_config(b"avcC"), None);
    }

    #[test]
    fn v210_six_pixels() {
        // Cb0 Y0 Cr0 | Y1 Cb1 Y2 | Cr1 Y3 Cb2 | Y4 Cr2 Y5 with distinct samples.
        let w3 = |a: u32, b: u32, c: u32| (a | b << 10 | c << 20).to_le_bytes();
        let mut row = Vec::new();
        row.extend(w3(100, 1, 200));
        row.extend(w3(2, 101, 3));
        row.extend(w3(201, 4, 102));
        row.extend(w3(5, 202, 6));
        row.resize(128, 0);
        let d = PixDesc {
            fourcc: *b"v210",
            ..PixDesc::default()
        };
        let u = unpack(&d, 6, 1, &row).unwrap();
        let s = |p: &[u8]| -> Vec<u16> {
            p.chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect()
        };
        assert_eq!(s(&u.frame.planes[0]), [1, 2, 3, 4, 5, 6]);
        assert_eq!(s(&u.frame.planes[1]), [100, 101, 102]);
        assert_eq!(s(&u.frame.planes[2]), [200, 201, 202]);
    }

    #[test]
    fn bitmap_rows_bottom_up_and_padded() {
        // 2×2 BGR, rows padded to 8 bytes, bottom row first.
        let data = [0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 1, 1, 1, 0, 0];
        let d = PixDesc {
            fourcc: [0; 4],
            depth: 24,
            bottom_up: true,
            palette: Vec::new(),
        };
        let u = unpack(&d, 2, 2, &data).unwrap();
        // Top row: blue then (1,1,1); bottom row: red then green. Planes G, B, R.
        assert_eq!(u.frame.planes[2], [0, 1, 255, 0]);
        assert_eq!(u.frame.planes[1], [255, 1, 0, 0]);
        assert_eq!(u.frame.planes[0], [0, 1, 0, 255]);
    }

    #[test]
    fn short_frames_are_errors_not_panics() {
        for code in [
            *b"v210", *b"UYVY", *b"I420", *b"NV12", *b"r210", *b"b64a", *b"Y41P", *b"yuv4",
            *b"v408", [0; 4],
        ] {
            let d = PixDesc {
                fourcc: code,
                depth: 24,
                ..PixDesc::default()
            };
            for n in [0, 1, 7, 100] {
                assert!(unpack(&d, 33, 17, &vec![0; n]).is_err(), "{code:?} {n}");
            }
        }
    }
}
