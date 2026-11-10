//! JPEG XR (ITU-T T.832 / ISO/IEC 29199-2) decoder.
//!
//! Reads the TIFF-like container (`.jxr`, `.wdp`, `.hdp`) or a bare codestream: spatial and
//! frequency mode, all three overlap modes, every band combination, hard and soft tiling,
//! per-tile and per-macroblock quantisers, YONLY / YUV420 / YUV422 / YUV444 / YUVK / N-component
//! internal colour, interleaved (second plane) and planar (second codestream) alpha, and every
//! output bit depth: BD1, BD5, BD565, BD8, BD10, BD16, BD16S (fixed point), BD16F (half), BD32S
//! (fixed point), BD32F (float) and RGBE.
//!
//! [`decode_native`] returns the samples in the stored layout of the pixel format, bit-exact
//! with the reference software; [`decode`] converts them for display (8/16-bit integers or
//! linear `f32` for the fixed-point, half, float and RGBE formats).
//!
//! CMYK: 32bppCMYK, 64bppCMYK, 40bppCMYKAlpha and 80bppCMYKAlpha (YUVK coded) and the
//! CMYKDIRECT formats are decoded to [`Kind::Cmyk`].
//!
//! YCC pixel formats (12bppYCC420 … 64bppYCC444AlphaFixedPoint, OUTPUT_CLR_FMT YUV420/422/444):
//! the native samples are Y, Cb, Cr (and alpha) per pixel with the stored bias, chroma upsampled
//! to full resolution the way the RGB formats are (jxrlib cannot output these formats, so the
//! layout is ours). [`decode`] converts them to RGB as full-range BT.601 YCbCr (JFIF), since the
//! container has no colour description for them.
//!
//! # Not supported
//! * Codestream versions above 1 (none are defined).
//! * Display of N-component images other than 1 or 3 channels (and alpha): [`decode_native`]
//!   still returns their samples.

mod bits;
mod container;
mod header;
mod output;
mod parse;
mod recon;
mod transform;
mod vlc;

use std::sync::atomic::{AtomicBool, Ordering};

pub use container::probe;
pub use header::{Bands, BitDepth, ClrFmt, OutFmt};
pub use output::Samples;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("not a JPEG XR file")]
    NotJxr,
    #[error("truncated {0}")]
    Truncated(String),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

#[derive(Default, Clone, Copy)]
pub struct DecodeOptions<'a> {
    pub cancel: Option<&'a AtomicBool>,
    /// Largest accepted width × height × channels; 0 means 2^31.
    pub max_samples: u64,
    /// Decode on the calling thread only.
    pub serial: bool,
}

/// What the headers say.
#[derive(Clone, Debug)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// Pixel format name from the container.
    pub pixel_format: Option<&'static str>,
    pub internal: ClrFmt,
    pub output: OutFmt,
    pub bit_depth: BitDepth,
    pub bands: Bands,
    pub overlap: u8,
    pub frequency: bool,
    /// Tile columns and rows.
    pub tiles: (u32, u32),
    pub has_alpha: bool,
    pub planar_alpha: bool,
    pub premultiplied: bool,
    /// Exif-style orientation from the container (1 = as stored).
    pub orientation: u8,
    pub icc: Option<Vec<u8>>,
}

/// Samples in the stored layout of the pixel format (see [`Samples`]), alpha last.
#[derive(Clone, Debug)]
pub struct Native {
    pub width: u32,
    pub height: u32,
    pub channels: usize,
    pub alpha: bool,
    pub output: OutFmt,
    pub bit_depth: BitDepth,
    /// Packed BD5/BD565/BD10 RGB stores red in the low bits (else blue).
    pub rb_not_swapped: bool,
    pub premultiplied: bool,
    pub samples: Samples,
    /// Some data was missing; the rest decoded as zero coefficients.
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gray,
    Rgb,
    Cmyk,
}

/// Display samples: integer formats stay integer, the rest become linear `f32` (1.0 = white).
#[derive(Clone, Debug, PartialEq)]
pub enum Pixels {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub kind: Kind,
    /// An alpha channel follows the colour channels.
    pub alpha: bool,
    pub premultiplied: bool,
    pub pixels: Pixels,
    pub truncated: bool,
}

impl Image {
    pub fn channels(&self) -> usize {
        let c = match self.kind {
            Kind::Gray => 1,
            Kind::Rgb => 3,
            Kind::Cmyk => 4,
        };
        c + self.alpha as usize
    }
}

fn slice(d: &[u8], (s, e): (usize, usize)) -> &[u8] {
    &d[s..e]
}

pub fn read_info(data: &[u8]) -> Result<Info, Error> {
    let c = container::parse(data)?;
    let (ih, ph, ah) = parse::read_headers(slice(data, c.image))?;
    let premultiplied = ih.premultiplied || c.pixel_format.is_some_and(container::premultiplied);
    Ok(Info {
        width: ih.width,
        height: ih.height,
        pixel_format: c.pixel_format.map(container::pixel_format_name),
        internal: ph.clr,
        output: ih.out_fmt,
        bit_depth: ih.bit_depth,
        bands: ph.bands,
        overlap: ih.overlap,
        frequency: ih.frequency,
        tiles: (ih.tile_cols.len() as u32 - 1, ih.tile_rows.len() as u32 - 1),
        has_alpha: ah.is_some() || c.alpha.is_some(),
        planar_alpha: ah.is_none() && c.alpha.is_some(),
        premultiplied,
        orientation: c.orientation,
        icc: c.icc.map(|r| slice(data, r).to_vec()),
    })
}

fn check_supported(ih: &header::ImageHeader, ph: &header::PlaneHeader) -> Result<(), Error> {
    use ClrFmt as C;
    use OutFmt as O;
    let ok = match (ph.clr, ih.out_fmt) {
        (C::YOnly, O::YOnly | O::Rgb | O::Rgbe) => true,
        (C::Yuv420 | C::Yuv422 | C::Yuv444, O::Rgb | O::Rgbe) => true,
        (C::Yuvk, O::Cmyk | O::CmykDirect) => true,
        (C::NComp, O::NComp) => true,
        // YCC pixel formats: the components are output as stored, chroma upsampled when the
        // output is denser than the coded chroma (never downsampled).
        (C::Yuv420, O::Yuv420 | O::Yuv422 | O::Yuv444)
        | (C::Yuv422, O::Yuv422 | O::Yuv444)
        | (C::Yuv444, O::Yuv444) => true,
        _ => false,
    };
    let bd_ok = match ih.bit_depth {
        BitDepth::Bd5 | BitDepth::Bd565 => ih.out_fmt == O::Rgb,
        BitDepth::Bd1White1 | BitDepth::Bd1Black1 => ih.out_fmt == O::YOnly,
        _ => true,
    };
    if ok && bd_ok {
        Ok(())
    } else {
        Err(Error::Malformed(format!(
            "{:?} coded as {:?} at {:?}",
            ih.out_fmt, ph.clr, ih.bit_depth
        )))
    }
}

/// Decodes one codestream into reconstructed planes (i32 samples, reserved from the memory
/// budget for as long as the planes live).
fn decode_cs(
    data: &[u8],
    opts: &DecodeOptions,
) -> Result<(parse::Codestream, specular_core::Reservation), Error> {
    let (ih, ph, _) = parse::read_headers(data)?;
    check_supported(&ih, &ph)?;
    let max = if opts.max_samples == 0 {
        1 << 31
    } else {
        opts.max_samples
    };
    let samples =
        ih.ext_width() as u64 * ih.ext_height() as u64 * (ph.nc as u64 + ih.alpha_plane as u64);
    if samples > max {
        return Err(Error::TooLarge);
    }
    let res = specular_core::budget::reserve_n(samples, 4)?;
    let cancel = || opts.cancel.is_some_and(|c| c.load(Ordering::Relaxed));
    let mut cs = parse::parse_codestream(data, &cancel)?;
    for p in &mut cs.planes {
        if cancel() {
            return Err(Error::Cancelled);
        }
        recon::reconstruct(p, &cs.ih, !opts.serial);
    }
    Ok((cs, res))
}

pub fn decode_native(data: &[u8], opts: &DecodeOptions) -> Result<Native, Error> {
    let c = container::parse(data)?;
    let (cs, _planes) = decode_cs(slice(data, c.image), opts)?;
    let mut truncated = cs.truncated;
    let planar = match (cs.planes.len(), c.alpha) {
        (1, Some(r)) => {
            let (a, alpha_res) = decode_cs(slice(data, r), opts)?;
            let ok = a.ih.width == cs.ih.width
                && a.ih.height == cs.ih.height
                && a.planes[0].ph.nc == 1
                && a.ih.out_fmt == OutFmt::YOnly
                && a.ih.bit_depth == cs.ih.bit_depth;
            if !ok {
                return Err(Error::Malformed(
                    "planar alpha does not match the image".into(),
                ));
            }
            truncated |= a.truncated;
            Some((a, alpha_res))
        }
        _ => None,
    };
    let alpha = match (&planar, cs.planes.get(1)) {
        (Some((a, _)), _) => Some((&a.ih, &a.planes[0], false)),
        (None, Some(p)) => Some((&cs.ih, p, true)),
        _ => None,
    };
    let (channels, samples) = output::format(&cs.ih, &cs.planes[0], alpha, !opts.serial)?;
    Ok(Native {
        width: cs.ih.width,
        height: cs.ih.height,
        channels,
        alpha: alpha.is_some(),
        output: cs.ih.out_fmt,
        bit_depth: cs.ih.bit_depth,
        rb_not_swapped: cs.ih.rb_not_swapped,
        premultiplied: cs.ih.premultiplied || c.pixel_format.is_some_and(container::premultiplied),
        samples,
        truncated,
    })
}

fn half_to_f32(h: u16) -> f32 {
    let s = ((h as u32) & 0x8000) << 16;
    let e = (h >> 10) & 0x1F;
    let m = (h & 0x3FF) as u32;
    let bits = match (e, m) {
        (0, 0) => s,
        (0, _) => {
            // Subnormal: renormalise.
            let v = m as f32 * (1.0 / (1 << 24) as f32);
            return if s != 0 { -v } else { v };
        }
        (31, _) => s | 0x7F80_0000 | (m << 13),
        _ => s | ((e as u32 + 112) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// Decodes for display.
pub fn decode(data: &[u8], opts: &DecodeOptions) -> Result<Image, Error> {
    let n = decode_native(data, opts)?;
    let kind = match n.output {
        OutFmt::YOnly => Kind::Gray,
        OutFmt::Rgb | OutFmt::Rgbe | OutFmt::Yuv420 | OutFmt::Yuv422 | OutFmt::Yuv444 => Kind::Rgb,
        OutFmt::Cmyk | OutFmt::CmykDirect => Kind::Cmyk,
        OutFmt::NComp if n.channels - n.alpha as usize == 1 => Kind::Gray,
        OutFmt::NComp if n.channels - n.alpha as usize == 3 => Kind::Rgb,
        _ => return Err(Error::Unsupported(format!("{} channels", n.channels))),
    };
    let swap = !n.rb_not_swapped;
    let mut pixels = match (&n.samples, n.bit_depth) {
        (Samples::U8(v), BitDepth::Bd1White1 | BitDepth::Bd1Black1) => {
            let white = if n.bit_depth == BitDepth::Bd1White1 {
                1
            } else {
                0
            };
            Pixels::U8(
                v.iter()
                    .map(|&b| if b == white { 255 } else { 0 })
                    .collect(),
            )
        }
        (Samples::U8(v), _) if n.output == OutFmt::Rgbe => {
            let mut f = Vec::with_capacity(v.len() / 4 * 3);
            for p in v.chunks_exact(4) {
                let e = p[3] as i32;
                let s = if e == 0 { 0.0 } else { 2f32.powi(e - 136) };
                f.extend([p[0] as f32 * s, p[1] as f32 * s, p[2] as f32 * s]);
            }
            Pixels::F32(f)
        }
        (Samples::U8(v), _) => Pixels::U8(v.clone()),
        (Samples::U16(v), BitDepth::Bd16) => Pixels::U16(v.clone()),
        (Samples::U16(v), BitDepth::Bd16S) => {
            Pixels::F32(v.iter().map(|&x| x as i16 as f32 / 8192.0).collect())
        }
        (Samples::U16(v), BitDepth::Bd16F) => {
            Pixels::F32(v.iter().map(|&x| half_to_f32(x)).collect())
        }
        (Samples::U16(v), BitDepth::Bd5 | BitDepth::Bd565) => {
            let six = n.bit_depth == BitDepth::Bd565;
            let mut o = Vec::with_capacity(v.len() * 3);
            for &p in v {
                let x5 = |x: u16| ((x << 3) | (x >> 2)) as u8;
                let a = x5(p & 31);
                let (b, c) = if six {
                    let g = (p >> 5) & 63;
                    (((g << 2) | (g >> 4)) as u8, x5(p >> 11))
                } else {
                    (x5((p >> 5) & 31), x5((p >> 10) & 31))
                };
                o.extend(if swap { [c, b, a] } else { [a, b, c] });
            }
            Pixels::U8(o)
        }
        (Samples::U16(v), _) => {
            // BD10, unpacked.
            Pixels::U16(v.iter().map(|&x| (x << 6) | (x >> 4)).collect())
        }
        (Samples::U32(v), BitDepth::Bd10) => {
            let mut o = Vec::with_capacity(v.len() * 3);
            for &p in v {
                let c = |s: u32| {
                    let x = ((p >> s) & 1023) as u16;
                    (x << 6) | (x >> 4)
                };
                o.extend(if swap {
                    [c(20), c(10), c(0)]
                } else {
                    [c(0), c(10), c(20)]
                });
            }
            Pixels::U16(o)
        }
        (Samples::U32(v), BitDepth::Bd32S) => Pixels::F32(
            v.iter()
                .map(|&x| x as i32 as f32 / (1 << 24) as f32)
                .collect(),
        ),
        (Samples::U32(v), _) => Pixels::F32(v.iter().map(|&x| f32::from_bits(x)).collect()),
    };
    if matches!(n.output, OutFmt::Yuv420 | OutFmt::Yuv422 | OutFmt::Yuv444) {
        ycc_to_rgb(&mut pixels, n.channels);
    }
    Ok(Image {
        width: n.width,
        height: n.height,
        kind,
        alpha: n.alpha,
        premultiplied: n.premultiplied && n.alpha,
        pixels,
        truncated: n.truncated,
    })
}

/// YCbCr to RGB in place (pixels of `stride` channels, Y Cb Cr first). The container has no
/// colour description for the YCC pixel formats, so they are read as full-range BT.601 YCbCr, as
/// in JFIF: chroma is centred on half the integer range, or on 0 for the fixed-point formats.
fn ycc_to_rgb(p: &mut Pixels, stride: usize) {
    fn conv(y: f32, cb: f32, cr: f32) -> [f32; 3] {
        [
            y + 1.402 * cr,
            y - 0.344_136 * cb - 0.714_136 * cr,
            y + 1.772 * cb,
        ]
    }
    match p {
        Pixels::U8(v) => {
            for px in v.chunks_exact_mut(stride) {
                let c = conv(px[0] as f32, px[1] as f32 - 128.0, px[2] as f32 - 128.0);
                for (d, s) in px.iter_mut().zip(c) {
                    *d = s.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        Pixels::U16(v) => {
            for px in v.chunks_exact_mut(stride) {
                let c = conv(px[0] as f32, px[1] as f32 - 32768.0, px[2] as f32 - 32768.0);
                for (d, s) in px.iter_mut().zip(c) {
                    *d = s.round().clamp(0.0, 65535.0) as u16;
                }
            }
        }
        Pixels::F32(v) => {
            for px in v.chunks_exact_mut(stride) {
                let c = conv(px[0], px[1], px[2]);
                px[..3].copy_from_slice(&c);
            }
        }
    }
}
