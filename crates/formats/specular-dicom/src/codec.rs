//! Encapsulated JPEG (baseline, extended 12-bit, lossless, and the retired progressive,
//! arithmetic and hierarchical processes), JPEG-LS and JPEG 2000 / HTJ2K frames, through the
//! in-tree decoders.

use std::sync::atomic::AtomicBool;

use specular_jp2 as jp2;
use specular_jpeg as jpeg;
use specular_jpegls as jpegls;

use crate::pixel::{Frame, Grey, Mask, PixelInfo, Pixels, alloc, grey_from, ybr_to_rgb};
use crate::{Error, Result};

fn jpeg_err(e: jpeg::JpegError) -> Error {
    match e {
        jpeg::JpegError::Cancelled => Error::Cancelled,
        jpeg::JpegError::TooLarge(n) => Error::TooLarge(n),
        jpeg::JpegError::Truncated(_) => Error::Truncated,
        jpeg::JpegError::Unsupported(s) => Error::Unsupported(format!("JPEG {s}")),
        e => Error::Codec(e.to_string()),
    }
}

fn jp2_err(e: jp2::Jp2Error) -> Error {
    match e {
        jp2::Jp2Error::Cancelled => Error::Cancelled,
        jp2::Jp2Error::TooLarge(n) => Error::TooLarge(n),
        jp2::Jp2Error::NeedsMemory(n) => Error::NeedsMemory(n),
        jp2::Jp2Error::Truncated(_) => Error::Truncated,
        jp2::Jp2Error::Unsupported(s) => Error::Unsupported(format!("JPEG 2000 {s}")),
        e => Error::Codec(e.to_string()),
    }
}

fn jpegls_err(e: jpegls::Error) -> Error {
    match e {
        jpegls::Error::Cancelled => Error::Cancelled,
        jpegls::Error::TooLarge(n) => Error::TooLarge(n),
        jpegls::Error::Truncated => Error::Truncated,
        jpegls::Error::NeedsMemory(n) => Error::NeedsMemory(n),
        jpegls::Error::Unsupported(s) => Error::Unsupported(format!("JPEG-LS {s}")),
        e => Error::Codec(e.to_string()),
    }
}

fn check_size(info: &PixelInfo, w: u32, h: u32) -> Result<()> {
    if (w, h) != (info.columns, info.rows) {
        return Err(Error::Malformed(
            "codestream size differs from Rows / Columns",
        ));
    }
    Ok(())
}

/// One JPEG frame (any process DICOM still allows; JPEG-LS has its own).
pub(crate) fn jpeg(data: &[u8], info: &PixelInfo, cancel: Option<&AtomicBool>) -> Result<Frame> {
    let head = jpeg::read_info(data).map_err(jpeg_err)?;
    check_size(info, head.width, head.height)?;
    if head.precision <= 8 && head.coding != jpeg::Coding::Lossless {
        return jpeg8(data, info, dct_colour_transform(&head, info), cancel);
    }
    let img = jpeg::decode_wide(data, cancel).map_err(jpeg_err)?;
    let (w, h) = (img.width, img.height);
    let n = w as usize * h as usize;
    let mask = Mask::new(
        info.bits_stored.min(img.precision as u16),
        info.bits_stored.min(img.precision as u16).saturating_sub(1),
        info.signed,
    );
    if img.planes.len() == 1 || info.samples == 1 {
        let p = &img.planes[0];
        let grey = grey_from(n, info, |i| {
            mask.apply(p.data.get(i).copied().unwrap_or(0) as u32)
        })?;
        return Ok(Frame {
            width: w,
            height: h,
            pixels: Pixels::Grey(grey),
            bits: info.bits_stored,
            signed: info.signed,
            truncated: img.truncated,
        });
    }
    if img.planes.len() < 3 {
        return Err(Error::Malformed("colour JPEG with fewer than 3 components"));
    }
    let bits = (img.precision as u16).min(16);
    let sample = |c: usize, x: usize, y: usize| -> u16 {
        let p = &img.planes[c];
        // Nearest upsampling of subsampled components.
        let sx = x * p.width as usize / w as usize;
        let sy = y * p.height as usize / h as usize;
        p.data.get(sy * p.width as usize + sx).copied().unwrap_or(0)
    };
    let mut frame = Frame {
        width: w,
        height: h,
        pixels: Pixels::Rgb8(Vec::new()),
        bits,
        signed: false,
        truncated: img.truncated,
    };
    if bits <= 8 {
        let mut out = alloc::<u8>(n * 3)?;
        for (i, px) in out.chunks_exact_mut(3).enumerate() {
            let (x, y) = (i % w as usize, i / w as usize);
            for (c, o) in px.iter_mut().enumerate() {
                *o = sample(c, x, y) as u8;
            }
        }
        frame.pixels = Pixels::Rgb8(out);
    } else {
        let mut out = alloc::<u16>(n * 3)?;
        for (i, px) in out.chunks_exact_mut(3).enumerate() {
            let (x, y) = (i % w as usize, i / w as usize);
            for (c, o) in px.iter_mut().enumerate() {
                *o = sample(c, x, y);
            }
        }
        frame.pixels = Pixels::Rgb16(out);
    }
    // Lossless and 12-bit frames carry the components as the photometric names them.
    if img.lossless {
        ybr_to_rgb(&mut frame, &info.photometric);
    } else if dct_colour_transform(&head, info) {
        ybr_to_rgb(&mut frame, &crate::pixel::Photometric::YbrFull);
    }
    Ok(frame)
}

/// Whether a three-component DCT frame holds YCbCr. The Adobe marker decides when present,
/// then JFIF, then component ids `R`,`G`,`B`. Without markers an `RGB` photometric with
/// unsubsampled components means the encoder skipped the transform (PS3.5 8.2.1); subsampled
/// components are always YCbCr, whatever the label says.
fn dct_colour_transform(head: &jpeg::JpegInfo, info: &PixelInfo) -> bool {
    if let Some(t) = head.adobe_transform {
        return t != 0;
    }
    if head.jfif {
        return true;
    }
    if head.components.iter().map(|c| c.id).eq(*b"RGB") {
        return false;
    }
    let subsampled = head
        .components
        .iter()
        .any(|c| (c.h_samp, c.v_samp) != (head.max_h, head.max_v));
    subsampled || info.photometric != crate::pixel::Photometric::Rgb
}

/// 8-bit DCT through the fast decoder.
fn jpeg8(
    data: &[u8],
    info: &PixelInfo,
    transform: bool,
    cancel: Option<&AtomicBool>,
) -> Result<Frame> {
    let opts = jpeg::DecodeOptions {
        cancel,
        ..Default::default()
    };
    let p = jpeg::decode(data, &opts).map_err(jpeg_err)?;
    let (w, h) = (p.width, p.height);
    let n = w as usize * h as usize;
    if p.layout == jpeg::PlanesLayout::Gray || info.samples == 1 {
        let pl = &p.planes[0];
        let mask = Mask::new(
            info.bits_stored.min(8),
            info.bits_stored.min(8) - 1,
            info.signed,
        );
        let grey = grey_from(n, info, |i| {
            let (x, y) = (i % w as usize, i / w as usize);
            mask.apply(pl.data.get(y * pl.stride + x).copied().unwrap_or(0) as u32)
        })?;
        return Ok(Frame {
            width: w,
            height: h,
            pixels: Pixels::Grey(grey),
            bits: info.bits_stored.min(8),
            signed: info.signed,
            truncated: p.truncated,
        });
    }
    let mut out = alloc::<u8>(n * 3)?;
    if !transform && p.layout == jpeg::PlanesLayout::YCbCr && p.planes.len() >= 3 {
        // The decoder guessed YCbCr; the components are R, G, B as coded.
        for (i, o) in out.chunks_exact_mut(3).enumerate() {
            let (x, y) = (i % w as usize, i / w as usize);
            for (c, v) in o.iter_mut().enumerate() {
                let pl = &p.planes[c];
                let (sx, sy) = (
                    x * pl.width as usize / w as usize,
                    y * pl.height as usize / h as usize,
                );
                *v = pl.data.get(sy * pl.stride + sx).copied().unwrap_or(0);
            }
        }
        return Ok(Frame {
            width: w,
            height: h,
            pixels: Pixels::Rgb8(out),
            bits: 8,
            signed: false,
            truncated: p.truncated,
        });
    }
    let rgba = jpeg::planes_to_rgba(&p);
    for (o, i) in out.chunks_exact_mut(3).zip(rgba.chunks_exact(4)) {
        o.copy_from_slice(&i[..3]);
    }
    Ok(Frame {
        width: w,
        height: h,
        pixels: Pixels::Rgb8(out),
        bits: 8,
        signed: false,
        truncated: p.truncated,
    })
}

/// One JPEG 2000 (or HTJ2K) frame.
pub(crate) fn jpeg2000(
    data: &[u8],
    info: &PixelInfo,
    cancel: Option<&AtomicBool>,
) -> Result<Frame> {
    let head = jp2::read_info(data).map_err(jp2_err)?;
    check_size(info, head.width, head.height)?;
    let c0 = head
        .components
        .first()
        .copied()
        .ok_or(Error::Malformed("no components"))?;
    let opts = jp2::DecodeOptions {
        depth: jp2::Depth::Sixteen,
        cancel,
        ..Default::default()
    };
    let img = jp2::decode(data, &opts).map_err(jp2_err)?;
    let (w, h) = (img.width, img.height);
    // Without a multi-component transform the components are what the photometric names: a
    // YBR_FULL(_422) codestream (some whole-slide converters write one, the chroma subsampled
    // in SIZ) still needs its YCbCr converted (PS3.5 8.2.4 asks for YBR_ICT / YBR_RCT / RGB).
    let ybr = img.format != jp2::PixelFormat::Gray8
        && img.format != jp2::PixelFormat::Gray16
        && codestream_mct(data) == Some(0);
    let n = w as usize * h as usize;
    let prec = c0.precision.clamp(1, 16) as u32;
    let vmax = (1u32 << prec) - 1;
    // 16-bit output is scaled to 0..65535; undo it.
    let unscale = |v: u16| -> u32 { (v as u32 * vmax + 32767) / 65535 };
    match img.pixels {
        jp2::Pixels::U8(px) if img.format == jp2::PixelFormat::Gray8 => {
            grey_frame(info, &head, c0, prec, n, (w, h), img.truncated, |i| {
                px.get(i).copied().unwrap_or(0) as u32
            })
        }
        jp2::Pixels::U16(px) if img.format == jp2::PixelFormat::Gray16 => {
            grey_frame(info, &head, c0, prec, n, (w, h), img.truncated, |i| {
                unscale(px.get(i).copied().unwrap_or(0))
            })
        }
        jp2::Pixels::U8(px) => {
            let mut out = alloc::<u8>(n * 3)?;
            for (o, i) in out.chunks_exact_mut(3).zip(px.chunks_exact(4)) {
                o.copy_from_slice(&i[..3]);
            }
            let mut frame = Frame {
                width: w,
                height: h,
                pixels: Pixels::Rgb8(out),
                bits: 8,
                signed: false,
                truncated: img.truncated,
            };
            if ybr {
                ybr_to_rgb(&mut frame, &info.photometric);
            }
            Ok(frame)
        }
        jp2::Pixels::U16(px) => {
            let mut out = alloc::<u16>(n * 3)?;
            for (o, i) in out.chunks_exact_mut(3).zip(px.chunks_exact(4)) {
                for (o, &v) in o.iter_mut().zip(i) {
                    *o = unscale(v) as u16;
                }
            }
            let mut frame = Frame {
                width: w,
                height: h,
                pixels: Pixels::Rgb16(out),
                bits: prec as u16,
                signed: false,
                truncated: img.truncated,
            };
            if ybr {
                ybr_to_rgb(&mut frame, &info.photometric);
            }
            Ok(frame)
        }
    }
}

/// The multiple component transformation flag of a raw codestream's main-header COD segment
/// (None for a JP2 file or a malformed header).
pub(crate) fn codestream_mct(data: &[u8]) -> Option<u8> {
    if data.get(..2)? != [0xff, 0x4f] {
        return None;
    }
    let mut p = 2;
    loop {
        let marker = u16::from_be_bytes([*data.get(p)?, *data.get(p + 1)?]);
        if marker == 0xff90 {
            return None; // SOT: the main header has no COD
        }
        let len = usize::from(u16::from_be_bytes([*data.get(p + 2)?, *data.get(p + 3)?]));
        if marker == 0xff52 {
            // Lcod, Scod, progression order, layers (2), then the transform flag.
            return data.get(p + 8).copied();
        }
        if len < 2 {
            return None;
        }
        p += 2 + len;
    }
}

#[allow(clippy::too_many_arguments)]
fn grey_frame(
    info: &PixelInfo,
    _head: &jp2::Jp2Info,
    c0: jp2::ComponentInfo,
    prec: u32,
    n: usize,
    (w, h): (u32, u32),
    truncated: bool,
    at: impl Fn(usize) -> u32,
) -> Result<Frame> {
    // Signed components come out offset by half the range; back to two's complement bits so
    // that Pixel Representation decides the sign (files disagree with their codestreams).
    let half = if c0.signed { 1u32 << (prec - 1) } else { 0 };
    let bits = info.bits_stored.min(prec as u16).max(1);
    let mask = Mask::new(bits, bits - 1, info.signed);
    let pmask = if prec >= 32 {
        u32::MAX
    } else {
        (1u32 << prec) - 1
    };
    let grey: Grey = grey_from(n, info, |i| mask.apply(at(i).wrapping_sub(half) & pmask))?;
    Ok(Frame {
        width: w,
        height: h,
        pixels: Pixels::Grey(grey),
        bits,
        signed: info.signed,
        truncated,
    })
}

/// One JPEG-LS frame (lossless or near-lossless).
pub(crate) fn jpegls(data: &[u8], info: &PixelInfo, cancel: Option<&AtomicBool>) -> Result<Frame> {
    let head = jpegls::read_info(data).map_err(jpegls_err)?;
    check_size(info, head.width, head.height)?;
    if head.mapped {
        return Err(Error::Unsupported("JPEG-LS mapping tables".into()));
    }
    let opts = jpegls::DecodeOptions {
        cancel,
        ..Default::default()
    };
    let img = jpegls::decode(data, &opts).map_err(jpegls_err)?;
    let (w, h) = (img.width, img.height);
    let n = w as usize * h as usize;
    let prec = img.precision as u16;
    if img.planes.len() == 1 || info.samples == 1 {
        let p = &img.planes[0];
        let bits = info.bits_stored.min(prec).max(1);
        let mask = Mask::new(bits, bits - 1, info.signed);
        let grey = grey_from(n, info, |i| {
            mask.apply(p.data.get(i).copied().unwrap_or(0) as u32)
        })?;
        return Ok(Frame {
            width: w,
            height: h,
            pixels: Pixels::Grey(grey),
            bits,
            signed: info.signed,
            truncated: img.truncated,
        });
    }
    if img.planes.len() < 3 {
        return Err(Error::Malformed(
            "colour JPEG-LS with fewer than 3 components",
        ));
    }
    // Nearest upsampling of subsampled components.
    let sample = |c: usize, x: usize, y: usize| -> u16 {
        let p = &img.planes[c];
        let sx = x * p.width as usize / w as usize;
        let sy = y * p.height as usize / h as usize;
        p.data.get(sy * p.width as usize + sx).copied().unwrap_or(0)
    };
    let mut frame = Frame {
        width: w,
        height: h,
        pixels: Pixels::Rgb8(Vec::new()),
        bits: prec,
        signed: false,
        truncated: img.truncated,
    };
    if prec <= 8 {
        let mut out = alloc::<u8>(n * 3)?;
        for (i, px) in out.chunks_exact_mut(3).enumerate() {
            let (x, y) = (i % w as usize, i / w as usize);
            for (c, o) in px.iter_mut().enumerate() {
                *o = sample(c, x, y) as u8;
            }
        }
        frame.pixels = Pixels::Rgb8(out);
    } else {
        let mut out = alloc::<u16>(n * 3)?;
        for (i, px) in out.chunks_exact_mut(3).enumerate() {
            let (x, y) = (i % w as usize, i / w as usize);
            for (c, o) in px.iter_mut().enumerate() {
                *o = sample(c, x, y);
            }
        }
        frame.pixels = Pixels::Rgb16(out);
    }
    // JPEG-LS codes the components as the photometric names them (PS3.5 8.2.3).
    ybr_to_rgb(&mut frame, &info.photometric);
    Ok(frame)
}

fn jxl_err(e: specular_jxl::Error) -> Error {
    match e {
        specular_jxl::Error::TooLarge => Error::TooLarge(specular_jxl::MAX_PIXELS),
        specular_jxl::Error::Truncated(_) => Error::Truncated,
        specular_jxl::Error::Unsupported(s) => Error::Unsupported(format!("JPEG XL {s}")),
        e => Error::Codec(e.to_string()),
    }
}

/// One JPEG XL frame (Supplement 232: lossless, JPEG recompression, lossy): the codestream's
/// own samples at its bit depth. Colour comes out RGB whatever YBR photometric the file names
/// (a recompressed JPEG decodes to RGB, as the JPEG syntaxes do); grey samples are the stored
/// bits, with Pixel Representation deciding the sign.
pub(crate) fn jpegxl(data: &[u8], info: &PixelInfo, cancel: Option<&AtomicBool>) -> Result<Frame> {
    if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
        return Err(Error::Cancelled);
    }
    let file = specular_jxl::JxlFile::open(data).map_err(jxl_err)?;
    let i = file.info();
    check_size(info, i.width, i.height)?;
    let s = file.decode_samples().map_err(jxl_err)?;
    let (w, h) = (s.width, s.height);
    let n = w as usize * h as usize;
    let prec = u16::from(s.bits).max(1);
    let truncated = !s.complete;
    if s.channels == 1 || info.samples == 1 {
        let ch = s.channels as usize;
        let bits = info.bits_stored.min(prec).max(1);
        let mask = Mask::new(bits, bits - 1, info.signed);
        let grey = grey_from(n, info, |k| {
            mask.apply(s.data.get(k * ch).copied().unwrap_or(0) as u32)
        })?;
        return Ok(Frame {
            width: w,
            height: h,
            pixels: Pixels::Grey(grey),
            bits,
            signed: info.signed,
            truncated,
        });
    }
    let pixels = if prec <= 8 {
        let mut out = alloc::<u8>(n * 3)?;
        for (o, &v) in out.iter_mut().zip(&s.data) {
            *o = v as u8;
        }
        Pixels::Rgb8(out)
    } else {
        let mut out = alloc::<u16>(n * 3)?;
        out.copy_from_slice(&s.data[..n * 3]);
        Pixels::Rgb16(out)
    };
    Ok(Frame {
        width: w,
        height: h,
        pixels,
        bits: prec,
        signed: false,
        truncated,
    })
}
