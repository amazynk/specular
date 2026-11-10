//! Units that are whole images in another format: WebP (`specular-webp`), JPEG XL
//! (`specular-jxl`) and JPEG 2000 codestreams (`specular-jp2`). Each is decoded and written
//! back as the raw chunky samples the IFD describes (8 or 16 bits, file byte order; signed
//! JPEG 2000 samples as two's complement; JPEG XL float samples as 16- or 32-bit floats), so
//! the usual conversion, region and scaling paths apply.

use crate::{Compression, Ifd};
use std::sync::atomic::AtomicBool;

/// Decoded pixels in one of the shapes the codecs give.
struct Pixels {
    w: usize,
    h: usize,
    /// Channels per pixel: 1 grey, 2 grey + alpha, 3 RGB, 4 RGBA.
    ch: usize,
    data: Samples,
}

enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl Pixels {
    #[inline]
    fn get16(&self, i: usize) -> u16 {
        match &self.data {
            Samples::U8(v) => v[i] as u16 * 257,
            Samples::U16(v) => v[i],
            Samples::F32(v) => (v[i].clamp(0.0, 1.0) * 65535.0).round() as u16,
        }
    }
}

/// f32 → IEEE half, rounding to nearest even (exact for values that came from halves).
fn f32_to_f16(f: f32) -> u16 {
    let b = f.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let man = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = m >> shift;
        let rem = m & ((1 << shift) - 1);
        let mid = 1 << (shift - 1);
        let r = half + u32::from(rem > mid || (rem == mid && half & 1 == 1));
        return sign | r as u16;
    }
    let half = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    let r = half + u32::from(rem > 0x1000 || (rem == 0x1000 && half & 1 == 1));
    sign | r as u16
}

/// Decode unit `src` (a `uw`-wide unit) into `out` (rows of `uw` pixels of the IFD's samples,
/// `le` byte order). Returns the bytes written (complete rows), 0 if it does not decode.
pub(crate) fn decode(
    src: &[u8],
    ifd: &Ifd,
    le: bool,
    uw: u32,
    out: &mut [u8],
    cancel: Option<&AtomicBool>,
) -> usize {
    let Some(px) = (match ifd.compression {
        Compression::Webp => webp(src),
        Compression::Jxl => jxl(src, ifd.sample_format == crate::SampleFormat::Float),
        Compression::Jpeg2000(code) => jp2(src, ifd.bits > 8, code == 33003, cancel),
        _ => None,
    }) else {
        return 0;
    };
    let spp = ifd.samples as usize;
    if let Samples::F32(v) = &px.data {
        return floats(v, &px, ifd, le, uw, out);
    }
    // Signed samples: the codec's values are offset by half the range.
    let flip = if ifd.sample_format == crate::SampleFormat::Int {
        0x8000
    } else {
        0
    };
    let bps = if ifd.bits > 8 { 2 } else { 1 };
    let rb = uw as usize * spp * bps;
    if rb == 0 {
        return 0;
    }
    let rows = (out.len() / rb).min(px.h);
    let cols = (uw as usize).min(px.w);
    let mut sample = [0u16; 4];
    for y in 0..rows {
        let row = &mut out[y * rb..(y + 1) * rb];
        for x in 0..cols {
            let at = (y * px.w + x) * px.ch;
            // The decoded pixel as grey/RGB plus alpha.
            let (c, a) = match px.ch {
                1 => ([px.get16(at); 3], 65535),
                2 => ([px.get16(at); 3], px.get16(at + 1)),
                3 => ([px.get16(at), px.get16(at + 1), px.get16(at + 2)], 65535),
                _ => (
                    [px.get16(at), px.get16(at + 1), px.get16(at + 2)],
                    px.get16(at + 3),
                ),
            };
            match spp {
                1 => sample[0] = c[0],
                2 => sample[..2].copy_from_slice(&[c[0], a]),
                3 => sample[..3].copy_from_slice(&c),
                _ => sample = [c[0], c[1], c[2], a],
            }
            let d = &mut row[x * spp * bps..(x + 1) * spp * bps];
            for s in &mut sample {
                *s ^= flip;
            }
            if bps == 1 {
                for (d, s) in d.iter_mut().zip(&sample) {
                    *d = (*s >> 8) as u8;
                }
            } else {
                for (d, s) in d.chunks_exact_mut(2).zip(&sample) {
                    d.copy_from_slice(&if le { s.to_le_bytes() } else { s.to_be_bytes() });
                }
            }
        }
    }
    rows * rb
}

fn webp(src: &[u8]) -> Option<Pixels> {
    let img = specular_webp::WebpImage::parse(src).ok()?;
    // Lossy planes to RGBA as libwebp converts them (studio-range BT.601, fancy chroma).
    let d = specular_webp::image::to_rgba(img.decode(1, false).ok()?);
    let specular_webp::Pixels::Rgba(p) = d.pixels else {
        return None;
    };
    let (w, h) = (p.width as usize, p.height as usize);
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        data.extend_from_slice(p.data.get(y * p.stride..y * p.stride + w * 4)?);
    }
    Some(Pixels {
        w,
        h,
        ch: 4,
        data: Samples::U8(data),
    })
}

/// Float samples (JPEG XL) as the IFD's 16- or 32-bit floats; grey or RGB only.
fn floats(v: &[f32], px: &Pixels, ifd: &Ifd, le: bool, uw: u32, out: &mut [u8]) -> usize {
    let spp = ifd.samples as usize;
    let bps = if ifd.bits == 16 { 2 } else { 4 };
    let rb = uw as usize * spp * bps;
    if rb == 0 || spp != px.ch {
        return 0;
    }
    let rows = (out.len() / rb).min(px.h);
    let cols = (uw as usize).min(px.w);
    for y in 0..rows {
        let row = &mut out[y * rb..(y + 1) * rb];
        for x in 0..cols * spp {
            let f = v[y * px.w * px.ch + x];
            let d = &mut row[x * bps..(x + 1) * bps];
            if bps == 2 {
                let h = f32_to_f16(f);
                d.copy_from_slice(&if le { h.to_le_bytes() } else { h.to_be_bytes() });
            } else {
                d.copy_from_slice(&if le { f.to_le_bytes() } else { f.to_be_bytes() });
            }
        }
    }
    rows * rb
}

fn jxl(src: &[u8], float: bool) -> Option<Pixels> {
    let f = specular_jxl::JxlFile::open(src).ok()?;
    if float {
        let s = f.decode_float_samples().ok()?;
        return Some(Pixels {
            w: s.width as usize,
            h: s.height as usize,
            ch: s.channels as usize,
            data: Samples::F32(s.data),
        });
    }
    let d = f.decode(1, false).ok()?;
    let ch = match d.layout {
        specular_jxl::Layout::Gray => 1,
        specular_jxl::Layout::Rgb => 3,
        specular_jxl::Layout::Rgba => 4,
        _ => return None,
    };
    Some(Pixels {
        w: d.width as usize,
        h: d.height as usize,
        ch,
        data: Samples::U8(d.data),
    })
}

/// The multiple component transform flag of a codestream's COD marker.
fn codestream_mct(cs: &[u8]) -> Option<u8> {
    if !cs.starts_with(&[0xFF, 0x4F]) {
        return None;
    }
    let mut p = 2usize;
    while p + 4 <= cs.len() && cs[p] == 0xFF {
        let m = cs[p + 1];
        let len = u16::from_be_bytes([cs[p + 2], cs[p + 3]]) as usize;
        if m == 0x52 {
            return cs.get(p + 8).copied();
        }
        if m == 0x90 || m == 0x93 {
            return None;
        }
        p += 2 + len;
    }
    None
}

fn jp2(src: &[u8], wide: bool, aperio_ycbcr: bool, cancel: Option<&AtomicBool>) -> Option<Pixels> {
    let opts = specular_jp2::DecodeOptions {
        depth: if wide {
            specular_jp2::Depth::Sixteen
        } else {
            specular_jp2::Depth::Eight
        },
        parallelism: specular_jp2::Parallelism::Single,
        cancel,
        ..Default::default()
    };
    let img = specular_jp2::decode(src, &opts).ok()?;
    let ch = img.format.channels();
    let mut px = Pixels {
        w: img.width as usize,
        h: img.height as usize,
        ch,
        data: match img.pixels {
            specular_jp2::Pixels::U8(v) => Samples::U8(v),
            specular_jp2::Pixels::U16(v) => Samples::U16(v),
        },
    };
    // Aperio's 33003 codestreams hold YCbCr without the component transform.
    if aperio_ycbcr && ch >= 3 && codestream_mct(src) == Some(0) {
        ycbcr_to_rgb(&mut px);
    }
    Some(px)
}

/// Full-range BT.601 YCbCr to RGB in place (alpha untouched).
fn ycbcr_to_rgb(px: &mut Pixels) {
    let ch = px.ch;
    fn conv(y: f32, cb: f32, cr: f32, max: f32) -> [f32; 3] {
        let half = (max + 1.0) / 2.0;
        let (cb, cr) = (cb - half, cr - half);
        [
            (y + 1.402 * cr).round().clamp(0.0, max),
            (y - 0.344_136 * cb - 0.714_136 * cr)
                .round()
                .clamp(0.0, max),
            (y + 1.772 * cb).round().clamp(0.0, max),
        ]
    }
    match &mut px.data {
        Samples::U8(v) => {
            for p in v.chunks_exact_mut(ch) {
                let c = conv(p[0] as f32, p[1] as f32, p[2] as f32, 255.0);
                for k in 0..3 {
                    p[k] = c[k] as u8;
                }
            }
        }
        Samples::U16(v) => {
            for p in v.chunks_exact_mut(ch) {
                let c = conv(p[0] as f32, p[1] as f32, p[2] as f32, 65535.0);
                for k in 0..3 {
                    p[k] = c[k] as u16;
                }
            }
        }
        // Only JPEG 2000 units are converted, and they are integers.
        Samples::F32(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mct_flag() {
        // SOC, SIZ (length 6, dummy), COD with MCT = 1.
        let cs = [
            0xFF, 0x4F, 0xFF, 0x51, 0, 6, 0, 0, 0, 0, 0xFF, 0x52, 0, 12, 0, 0, 0, 1, 1, 5, 4, 4, 0,
            0,
        ];
        assert_eq!(codestream_mct(&cs), Some(1));
        assert_eq!(codestream_mct(&cs[..5]), None);
    }

    #[test]
    fn ycbcr_grey() {
        let mut px = Pixels {
            w: 1,
            h: 1,
            ch: 3,
            data: Samples::U8(vec![100, 128, 128]),
        };
        ycbcr_to_rgb(&mut px);
        let Samples::U8(v) = px.data else {
            unreachable!()
        };
        assert_eq!(v, [100, 100, 100]);
    }
}
