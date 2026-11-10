//! Coded image items other than HEVC: AVC (`avc1`, HEIF Annex E) through
//! the in-tree H.264 decoder, VVC (`vvc1`, Annex L) through `specular-vvc`,
//! JPEG (`jpeg`, Annex H) through `specular-jpeg`, and uncompressed items
//! (`unci`, ISO/IEC 23001-17, see [`crate::unci`]). JPEG 2000 (`j2k1`) and
//! the HEVC tile and layered item types are refused by name.
//!
//! Each decodes to the same [`Tile`] as an HEVC item: YCbCr (or G / B / R
//! for RGB-coded items) planes cropped to the item's `ispe`, with the
//! colour signalling the codec carries.

use heif::meta::{
    ITEM_TYPE_AV01, ITEM_TYPE_AVC1, ITEM_TYPE_HEV1, ITEM_TYPE_HVC1, ITEM_TYPE_HVT1, ITEM_TYPE_J2K1,
    ITEM_TYPE_JPEG, ITEM_TYPE_LHV1, ITEM_TYPE_UNCI, ITEM_TYPE_VVC1,
};
use heif::{Chroma, HeifFile, HeifFrame, HeifPixelFormat, ImageKind, ImageNode, Plane};

use crate::colour::VuiColour;
use crate::unci::raw_property;
use crate::{DecodeOptions, Error, Result, Tile};

/// How a coded image item is compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    /// HEVC (`hvc1`, `hev1`).
    Hevc,
    /// AVC / H.264 (`avc1`).
    Avc,
    /// VVC / H.266 (`vvc1`).
    Vvc,
    /// JPEG (`jpeg`).
    Jpeg,
    /// Uncompressed or generically compressed samples (`unci`).
    Uncompressed,
}

impl Codec {
    /// A short name for the info panel.
    pub fn name(self) -> &'static str {
        match self {
            Codec::Hevc => "HEVC",
            Codec::Avc => "AVC",
            Codec::Vvc => "VVC",
            Codec::Jpeg => "JPEG",
            Codec::Uncompressed => "uncompressed",
        }
    }
}

/// Properties a codec reads itself: never "unrecognised essential" ones.
pub(crate) const CODEC_PROPERTIES: &[[u8; 4]] = &[
    *b"avcC", *b"vvcC", *b"jpgC", *b"uncC", *b"cmpd", *b"cmpC", *b"icef",
];

/// The codec of coded item `node`; other coding formats are refused by name.
pub(crate) fn codec_of(node: &ImageNode) -> Result<Codec> {
    let id = node.item.id;
    let t = match node.kind {
        ImageKind::Coded(t) => t,
        _ => return Err(heif::HeifError::invalid(format!("item {id} is not coded")).into()),
    };
    let refuse = |what: &str| Err(Error::Unsupported(format!("item {id}: {what}")));
    match t {
        ITEM_TYPE_HVC1 | ITEM_TYPE_HEV1 => Ok(Codec::Hevc),
        ITEM_TYPE_AVC1 => Ok(Codec::Avc),
        ITEM_TYPE_JPEG => Ok(Codec::Jpeg),
        ITEM_TYPE_UNCI => Ok(Codec::Uncompressed),
        ITEM_TYPE_VVC1 => Ok(Codec::Vvc),
        ITEM_TYPE_J2K1 => refuse("JPEG 2000 coding in a HEIF file"),
        ITEM_TYPE_AV01 => refuse("AV1 coding in a HEIF file without the AVIF brand"),
        ITEM_TYPE_HVT1 => refuse("HEVC tile items (hvt1)"),
        ITEM_TYPE_LHV1 => refuse("layered HEVC items (lhv1)"),
        t => refuse(&format!("coded type '{}'", heif::boxes::fourcc_str(&t))),
    }
}

/// What a coded item decodes to, read from its configuration alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Format {
    pub codec: Codec,
    pub bit_depth: u8,
    pub chroma: Chroma,
    /// The item carries its own alpha (an `unci` alpha component).
    pub alpha: bool,
    /// The planes are G / B / R (identity matrix), whatever the `nclx` says.
    pub rgb: bool,
}

/// The [`Format`] of coded item `node`.
pub(crate) fn format_of<D: AsRef<[u8]>>(file: &HeifFile<D>, node: &ImageNode) -> Result<Format> {
    let codec = codec_of(node)?;
    let id = node.item.id;
    let plain = |bit_depth, chroma| Format {
        codec,
        bit_depth,
        chroma,
        alpha: false,
        rgb: false,
    };
    match codec {
        Codec::Hevc => {
            let cfg = node.properties.hvcc().ok_or_else(|| {
                heif::HeifError::invalid(format!("item {id}: hvc1 item without hvcC"))
            })?;
            let chroma = Chroma::from_idc(cfg.chroma_format_idc).ok_or_else(|| {
                heif::HeifError::invalid(format!(
                    "hvcC chroma_format_idc {}",
                    cfg.chroma_format_idc
                ))
            })?;
            Ok(plain(cfg.bit_depth_luma(), chroma))
        }
        Codec::Avc => {
            let avcc = raw_property(node, b"avcC").ok_or_else(|| {
                heif::HeifError::invalid(format!("item {id}: avc1 item without avcC"))
            })?;
            let (chroma, depth) = avcc_format(avcc);
            Ok(plain(depth, chroma))
        }
        Codec::Vvc => {
            let cfg = vvc_config(node)?;
            let (chroma, depth) = match (cfg.chroma_format_idc, cfg.bit_depth) {
                (Some(c), Some(d)) => (c, d),
                _ => {
                    let sets: Vec<&[u8]> = cfg.nal_units.iter().map(Vec::as_slice).collect();
                    let info = specular_vvc::info(&sets).map_err(|e| vvc_error(id, e))?;
                    (vvc_chroma_idc(info.chroma), info.bit_depth)
                }
            };
            let chroma = Chroma::from_idc(chroma).ok_or_else(|| {
                heif::HeifError::invalid(format!("vvcC chroma_format_idc {chroma}"))
            })?;
            Ok(plain(depth, chroma))
        }
        Codec::Jpeg => {
            let data = file.item_data(id)?;
            let info = specular_jpeg::read_info(&data).map_err(|e| jpeg_error(id, e))?;
            let (chroma, rgb) = jpeg_layout(id, &info)?;
            Ok(Format {
                rgb,
                ..plain(8, chroma)
            })
        }
        Codec::Uncompressed => {
            let f = crate::unci::Layout::of(node)?.format();
            Ok(Format {
                codec,
                bit_depth: f.bit_depth,
                chroma: f.chroma,
                alpha: f.alpha,
                rgb: f.rgb,
            })
        }
    }
}

/// Chroma format and depth from an `avcC` record's High-profile extension; 4:2:0 8-bit
/// without one.
fn avcc_format(c: &[u8]) -> (Chroma, u8) {
    let ext = || -> Option<(Chroma, u8)> {
        if !matches!(c.get(1)?, 100 | 110 | 122 | 244) {
            return None;
        }
        let mut p = 5;
        for k in 0..2 {
            let n = usize::from(*c.get(p)? & if k == 0 { 31 } else { 255 });
            p += 1;
            for _ in 0..n {
                let l = usize::from(u16::from_be_bytes([*c.get(p)?, *c.get(p + 1)?]));
                p += 2 + l;
            }
        }
        let chroma = Chroma::from_idc(*c.get(p)? & 3)?;
        let depth = (*c.get(p + 1)? & 7) + 8;
        Some((chroma, depth))
    };
    ext().unwrap_or((Chroma::Yuv420, 8))
}

fn jpeg_layout(id: u32, info: &specular_jpeg::JpegInfo) -> Result<(Chroma, bool)> {
    use specular_jpeg::ColorModel;
    if info.precision != 8 {
        return Err(Error::Unsupported(format!(
            "item {id}: {}-bit JPEG",
            info.precision
        )));
    }
    match info.color {
        ColorModel::Gray => Ok((Chroma::Mono, false)),
        ColorModel::Rgb => Ok((Chroma::Yuv444, true)),
        ColorModel::YCbCr => {
            let c = &info.components;
            let same = c.len() == 3
                && c[1].h_samp == c[2].h_samp
                && c[1].v_samp == c[2].v_samp
                && c[1].h_samp > 0
                && c[1].v_samp > 0;
            let ratio = same.then(|| (c[0].h_samp / c[1].h_samp, c[0].v_samp / c[1].v_samp));
            let exact = same
                && c[0].h_samp.is_multiple_of(c[1].h_samp)
                && c[0].v_samp.is_multiple_of(c[1].v_samp);
            match ratio.filter(|_| exact) {
                Some((1, 1)) => Ok((Chroma::Yuv444, false)),
                Some((2, 1)) => Ok((Chroma::Yuv422, false)),
                Some((2, 2)) => Ok((Chroma::Yuv420, false)),
                _ => Err(Error::Unsupported(format!(
                    "item {id}: JPEG chroma sampling {}",
                    c.iter()
                        .map(|c| format!("{}x{}", c.h_samp, c.v_samp))
                        .collect::<Vec<_>>()
                        .join(",")
                ))),
            }
        }
        ColorModel::Cmyk | ColorModel::Ycck => {
            Err(Error::Unsupported(format!("item {id}: CMYK JPEG")))
        }
    }
}

fn jpeg_error(id: u32, e: specular_jpeg::JpegError) -> Error {
    use specular_jpeg::JpegError as J;
    match e {
        J::Cancelled => Error::Cancelled,
        J::Unsupported(m) => Error::Unsupported(format!("item {id}: JPEG {m}")),
        J::TooLarge(n) => Error::Limit(format!("item {id}: JPEG of {n} bytes")),
        e => heif::HeifError::invalid(format!("item {id}: {e}")).into(),
    }
}

fn avc_error(id: u32, e: specular_h264::Error) -> Error {
    match e {
        specular_h264::Error::Unsupported(m) => Error::Unsupported(format!("item {id}: H.264 {m}")),
        specular_h264::Error::NeedsMemory(n) => Error::NeedsMemory(n),
        e => heif::HeifError::invalid(format!("item {id}: {e}")).into(),
    }
}

/// Decode coded item `node` of a codec other than HEVC (or one `cexg` extent / `tili`
/// tile of it: `payload`), cropped to its `ispe`.
pub(crate) fn decode<D: AsRef<[u8]>>(
    file: &HeifFile<D>,
    node: &ImageNode,
    payload: Option<&[u8]>,
    threads: usize,
    opts: &DecodeOptions<'_>,
) -> Result<Tile> {
    if opts.cancelled() {
        return Err(Error::Cancelled);
    }
    let id = node.item.id;
    let data = match payload {
        Some(p) => std::borrow::Cow::Borrowed(p),
        None => file.item_data(id)?,
    };
    if data.is_empty() {
        return Err(heif::HeifError::invalid(format!("item {id}: empty payload")).into());
    }
    let ispe = node.ispe();
    if let Some((w, h)) = ispe
        && u64::from(w) * u64::from(h) > crate::MAX_PIXELS
    {
        return Err(Error::Limit(format!("{w}x{h} item {id}")));
    }
    let tile = match codec_of(node)? {
        Codec::Avc => decode_avc(node, &data)?,
        Codec::Vvc => decode_vvc(node, &data, threads, opts)?,
        Codec::Jpeg => decode_jpeg(node, &data, threads, opts)?,
        Codec::Uncompressed => {
            let (w, h) = ispe.ok_or_else(|| {
                heif::HeifError::invalid(format!("item {id}: unci item without ispe"))
            })?;
            let frame = crate::unci::Layout::of(node)?.decode(id, &data, w, h)?;
            Tile {
                frame,
                // H.273 "unspecified" signalling at full range, like libheif's default.
                vui: Some(VuiColour {
                    full_range: true,
                    description: None,
                }),
                concealed_ctus: 0,
            }
        }
        Codec::Hevc => {
            return Err(heif::HeifError::invalid(format!("item {id}: HEVC here")).into());
        }
    };
    if opts.cancelled() {
        return Err(Error::Cancelled);
    }
    Ok(Tile {
        frame: crop_to(id, tile.frame, ispe)?,
        ..tile
    })
}

/// `frame` cropped to `ispe` (top-left); an error when the `ispe` is larger.
fn crop_to(id: u32, frame: HeifFrame, ispe: Option<(u32, u32)>) -> Result<HeifFrame> {
    match ispe {
        None => Ok(frame),
        Some((w, h)) if (w, h) == (frame.width, frame.height) => Ok(frame),
        Some((w, h)) if w <= frame.width && h <= frame.height && w > 0 && h > 0 => {
            Ok(heif::compose::crop(&frame, 0, 0, w, h)?)
        }
        Some((w, h)) => Err(heif::HeifError::invalid(format!(
            "item {id}: ispe {w}x{h} does not fit the decoded {}x{} picture",
            frame.width, frame.height
        ))
        .into()),
    }
}

fn decode_avc(node: &ImageNode, data: &[u8]) -> Result<Tile> {
    let id = node.item.id;
    let avcc = raw_property(node, b"avcC")
        .ok_or_else(|| heif::HeifError::invalid(format!("item {id}: avc1 item without avcC")))?;
    // The record's body follows the box header (no version / flags).
    let (chroma, _) = avcc_format(avcc);
    let mut d = specular_h264::Decoder::with_avcc(
        avcc,
        specular_h264::Options {
            threads: 1,
            output: specular_h264::OutputOrder::Decode,
        },
    )
    .map_err(|e| avc_error(id, e))?;
    d.decode_sample(data, 0).map_err(|e| avc_error(id, e))?;
    d.flush();
    let pic = d.next_picture(true).ok_or_else(|| {
        heif::HeifError::invalid(format!("item {id}: no picture in the AVC item"))
    })?;
    // Monochrome streams come out as 4:2:0 with grey chroma: only the luma is kept.
    let chroma = match (chroma, pic.chroma) {
        (Chroma::Mono, _) => Chroma::Mono,
        (_, specular_h264::Chroma::Yuv420) => Chroma::Yuv420,
        (_, specular_h264::Chroma::Yuv422) => Chroma::Yuv422,
        (_, specular_h264::Chroma::Yuv444) => Chroma::Yuv444,
    };
    let fmt = HeifPixelFormat::new(chroma, pic.bit_depth.max(8), false)?;
    let bps = fmt.bytes_per_sample();
    let mut planes = Vec::with_capacity(chroma.colour_planes());
    for (i, src) in pic.planes.iter().enumerate().take(chroma.colour_planes()) {
        let (pw, ph) = fmt.plane_dims(i, pic.width, pic.height);
        let stride = pw as usize * bps;
        let need = stride * ph as usize;
        let data = src
            .get(..need)
            .ok_or_else(|| heif::HeifError::invalid(format!("item {id}: short AVC plane")))?
            .to_vec();
        planes.push(Plane { stride, data });
    }
    let frame = HeifFrame {
        width: pic.width,
        height: pic.height,
        format: fmt,
        planes,
    };
    frame.validate()?;
    Ok(Tile {
        frame,
        vui: pic.signal.map(|s| VuiColour {
            full_range: s.full_range,
            description: s.colour,
        }),
        concealed_ctus: 0,
    })
}

/// The parsed `vvcC` of a `vvc1` item.
fn vvc_config(node: &ImageNode) -> Result<specular_vvc::vvcc::VvcConfig> {
    let id = node.item.id;
    let raw = raw_property(node, b"vvcC")
        .ok_or_else(|| heif::HeifError::invalid(format!("item {id}: vvc1 item without vvcC")))?;
    specular_vvc::vvcc::parse(raw).map_err(|e| vvc_error(id, e))
}

fn vvc_chroma_idc(c: specular_vvc::ChromaFormat) -> u8 {
    match c {
        specular_vvc::ChromaFormat::Monochrome => 0,
        specular_vvc::ChromaFormat::Yuv420 => 1,
        specular_vvc::ChromaFormat::Yuv422 => 2,
        specular_vvc::ChromaFormat::Yuv444 => 3,
    }
}

fn vvc_error(id: u32, e: specular_vvc::Error) -> Error {
    match e {
        specular_vvc::Error::Unsupported(m) => {
            Error::Unsupported(format!("item {id}: VVC (H.266) {m}"))
        }
        specular_vvc::Error::Cancelled => Error::Cancelled,
        specular_vvc::Error::NeedsMemory(n) => Error::NeedsMemory(n),
        e => heif::HeifError::invalid(format!("item {id}: {e}")).into(),
    }
}

fn decode_vvc(
    node: &ImageNode,
    data: &[u8],
    threads: usize,
    opts: &DecodeOptions<'_>,
) -> Result<Tile> {
    let id = node.item.id;
    let cfg = vvc_config(node)?;
    let sets: Vec<&[u8]> = cfg.nal_units.iter().map(Vec::as_slice).collect();
    let units = specular_vvc::nal::split_length_prefixed(data, cfg.length_size)
        .map_err(|e| vvc_error(id, e))?;
    let d = specular_vvc::decode_still_with(
        &sets,
        &units,
        &specular_vvc::Options {
            threads,
            cancel: opts.cancel,
        },
    )
    .map_err(|e| vvc_error(id, e))?;
    let f = &d.frame;
    let chroma = Chroma::from_idc(vvc_chroma_idc(f.chroma))
        .ok_or_else(|| heif::HeifError::invalid(format!("item {id}: VVC chroma format")))?;
    let frame = crate::core::pack_frame(
        chroma,
        (f.bit_depth, f.bit_depth),
        (f.width, f.height),
        None,
        |p, y| {
            let (plane, stride) = match p {
                0 => (&f.y, f.y_stride),
                1 => (&f.cb, f.c_stride),
                _ => (&f.cr, f.c_stride),
            };
            let s = y * stride as usize;
            plane.get(s..).unwrap_or(&[])
        },
    )
    .map_err(|e| heif::HeifError::invalid(format!("item {id}: {e}")))?;
    Ok(Tile {
        frame,
        vui: d.signal.map(|s| VuiColour {
            full_range: s.full_range,
            description: Some((
                s.colour_primaries,
                s.transfer_characteristics,
                s.matrix_coeffs,
            )),
        }),
        concealed_ctus: d.damaged,
    })
}

fn decode_jpeg(
    node: &ImageNode,
    data: &[u8],
    threads: usize,
    opts: &DecodeOptions<'_>,
) -> Result<Tile> {
    use specular_jpeg::PlanesLayout;
    let id = node.item.id;
    let info = specular_jpeg::read_info(data).map_err(|e| jpeg_error(id, e))?;
    let (chroma, rgb) = jpeg_layout(id, &info)?;
    let d = specular_jpeg::decode(
        data,
        &specular_jpeg::DecodeOptions {
            cancel: opts.cancel,
            parallelism: if threads > 1 {
                specular_jpeg::Parallelism::Auto
            } else {
                specular_jpeg::Parallelism::Single
            },
            ..Default::default()
        },
    )
    .map_err(|e| jpeg_error(id, e))?;
    // JFIF sites subsampled chroma midway between the luma samples.
    let fmt = HeifPixelFormat::new(chroma, 8, false)?.with_siting(heif::ChromaSiting::CENTRE);
    // Stored order → frame order: Y Cb Cr, or G B R for RGB.
    let order: &[usize] = match (d.layout, rgb) {
        (PlanesLayout::Gray, _) => &[0],
        (PlanesLayout::YCbCr, false) => &[0, 1, 2],
        (PlanesLayout::Rgb, true) => &[1, 2, 0],
        _ => {
            return Err(Error::Unsupported(format!(
                "item {id}: JPEG decoded as {:?}",
                d.layout
            )));
        }
    };
    let mut planes = Vec::with_capacity(order.len());
    for (i, &k) in order.iter().enumerate() {
        let (pw, ph) = fmt.plane_dims(i, d.width, d.height);
        let p = d
            .planes
            .get(k)
            .filter(|p| p.width >= pw && p.height >= ph)
            .ok_or_else(|| {
                Error::Unsupported(format!("item {id}: JPEG plane {k} of another size"))
            })?;
        let mut out = Vec::with_capacity(pw as usize * ph as usize);
        for y in 0..ph as usize {
            out.extend_from_slice(&p.data[y * p.stride..y * p.stride + pw as usize]);
        }
        planes.push(Plane {
            stride: pw as usize,
            data: out,
        });
    }
    let frame = HeifFrame {
        width: d.width,
        height: d.height,
        format: fmt,
        planes,
    };
    frame.validate()?;
    Ok(Tile {
        frame,
        // JFIF: BT.601 full range (an `nclx` overrides it).
        vui: Some(VuiColour {
            full_range: true,
            description: (!rgb).then_some((2, 2, 6)),
        }),
        concealed_ctus: usize::from(d.truncated),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avcc_high_profile_extension() {
        // Baseline: no extension.
        let base = [1u8, 66, 0, 30, 0xff, 0xe0, 0];
        assert_eq!(avcc_format(&base), (Chroma::Yuv420, 8));
        // High 4:2:2 10-bit, one 2-byte SPS and one 1-byte PPS, then the extension.
        let high = [
            1u8, 122, 0, 30, 0xff, 0xe1, 0, 2, 0x67, 0x7a, 1, 0, 1, 0x68, 0xfe, 0xfa, 0xf8, 0,
        ];
        assert_eq!(avcc_format(&high), (Chroma::Yuv422, 10));
        // Truncated: the default.
        assert_eq!(avcc_format(&high[..12]), (Chroma::Yuv420, 8));
    }
}
