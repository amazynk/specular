//! JPEG XR (`.jxr`, `.wdp`, `.hdp`, ITU-T T.832) through `specular-jxr`.
//!
//! The image is decoded whole. 8-bit grey stays `Gray`, 16-bit grey `Gray16`; 8-bit RGB (and
//! the packed 5/6-bit formats) becomes `Rgba`, 16-bit and 10-bit RGB `Rgba16` (sRGB-coded, the
//! embedded profile applies). Half, float, fixed-point and RGBE are scene-referred scRGB:
//! linear BT.709 with 1.0 = SDR white, so they become `Rgba16F` and take the HDR path. CMYK is
//! converted to sRGB here through the colour policy's CMYK converter (embedded profile, else
//! the generic model). Premultiplied formats are unpremultiplied. Orientation comes from the
//! container's transformation tag; Exif and XMP are found by the shared TIFF-style locators.

use std::sync::Arc;

use specular_core::{
    CancelToken, HdrEncoding, Orientation, PixelLayout, Primaries, Raster, Source,
};
use specular_jxr as jxr;

use crate::linear;
use crate::whole::{self, Levels};
use crate::{Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};

pub struct JxrDecoder;

fn map_err(e: jxr::Error) -> DecodeError {
    match e {
        jxr::Error::NotJxr => DecodeError::UnsupportedFormat,
        jxr::Error::Truncated(s) => DecodeError::Truncated(format!("JPEG XR {s}")),
        jxr::Error::Malformed(s) => DecodeError::Malformed(s),
        jxr::Error::Unsupported(s) => DecodeError::Unsupported(format!("JPEG XR {s}")),
        jxr::Error::TooLarge => DecodeError::TooLarge,
        jxr::Error::Cancelled => DecodeError::Cancelled,
        jxr::Error::NeedsMemory(n) => DecodeError::NeedsMemory(n),
    }
}

/// What the display raster will be.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Out {
    Gray8,
    Rgba8,
    Gray16,
    GrayAlpha16,
    Rgba16,
    Float,
}

impl Out {
    fn bytes(self) -> u64 {
        match self {
            Out::Gray8 => 1,
            Out::Gray16 => 2,
            Out::Rgba8 | Out::GrayAlpha16 => 4,
            Out::Rgba16 | Out::Float => 8,
        }
    }
}

impl Decoder for JxrDecoder {
    fn format(&self) -> Format {
        Format::JpegXr
    }
    fn probe(&self, head: &[u8]) -> bool {
        jxr::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["jxr", "wdp", "hdp"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use jxr::{BitDepth as B, OutFmt as O};
        let i = jxr::read_info(source.bytes()).map_err(map_err)?;
        if i.width == 0 || i.height == 0 {
            return Err(DecodeError::Malformed("empty image".into()));
        }
        let cmyk = matches!(i.output, O::Cmyk | O::CmykDirect);
        let gray = i.output == O::YOnly;
        let float =
            matches!(i.bit_depth, B::Bd16S | B::Bd16F | B::Bd32S | B::Bd32F) || i.output == O::Rgbe;
        let wide = matches!(i.bit_depth, B::Bd16 | B::Bd10);
        let out = match () {
            _ if float => Out::Float,
            _ if gray && wide && i.has_alpha => Out::GrayAlpha16,
            _ if gray && wide => Out::Gray16,
            _ if gray && !i.has_alpha => Out::Gray8,
            _ if wide && !cmyk => Out::Rgba16,
            _ => Out::Rgba8,
        };
        let model = match i.output {
            O::YOnly if matches!(i.bit_depth, B::Bd1White1 | B::Bd1Black1) => "Black and white",
            O::YOnly => "Greyscale",
            O::Cmyk | O::CmykDirect => "CMYK",
            O::Rgbe => "RGBE",
            _ if float => "scRGB",
            _ => "RGB",
        };
        let bit_depth = match i.bit_depth {
            B::Bd1White1 | B::Bd1Black1 => 1,
            B::Bd5 => 5,
            B::Bd565 => 6,
            B::Bd8 => 8,
            B::Bd10 => 10,
            B::Bd16 | B::Bd16S | B::Bd16F => 16,
            B::Bd32S | B::Bd32F => 32,
        };
        let info = ImageInfo {
            format: Format::JpegXr,
            width: i.width,
            height: i.height,
            orientation: Orientation::from_exif(i.orientation),
            // CMYK is converted with its profile here; scRGB is defined without one.
            icc_profile: i.icc.clone().filter(|_| !cmyk && !float).map(Arc::new),
            has_alpha: i.has_alpha,
            bit_depth,
            progressive: false,
            coding: coding(&i),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        Ok(Arc::new(JxrSession {
            source,
            info,
            model,
            out,
            cmyk_icc: i.icc.filter(|_| cmyk),
            premultiplied: i.premultiplied && i.has_alpha,
            levels: Levels::new(),
        }))
    }
}

fn coding(i: &jxr::Info) -> String {
    use jxr::ClrFmt as C;
    let mut s = i.pixel_format.unwrap_or("codestream").to_string();
    match i.internal {
        C::Yuv420 => s += ", 4:2:0",
        C::Yuv422 => s += ", 4:2:2",
        _ => {}
    }
    s += [", no overlap", ", overlap 1", ", overlap 2", ", overlap 3"][i.overlap.min(3) as usize];
    s += if i.frequency {
        ", frequency order"
    } else {
        ", spatial order"
    };
    match i.bands {
        jxr::Bands::All => {}
        jxr::Bands::NoFlex => s += ", no flexbits",
        jxr::Bands::NoHp => s += ", no highpass",
        jxr::Bands::DcOnly => s += ", DC only",
    }
    if i.tiles != (1, 1) {
        s += &format!(", {}×{} tiles", i.tiles.0, i.tiles.1);
    }
    if i.has_alpha {
        s += if i.planar_alpha {
            ", planar alpha"
        } else {
            ", interleaved alpha"
        };
    }
    s
}

struct JxrSession {
    source: Arc<Source>,
    info: ImageInfo,
    model: &'static str,
    out: Out,
    /// The profile CMYK is converted with (the generic model without one).
    cmyk_icc: Option<Vec<u8>>,
    premultiplied: bool,
    levels: Levels,
}

/// Straight from premultiplied 8- or 16-bit samples, rounded.
fn unpremultiply_int(c: u32, a: u32, max: u32) -> u32 {
    if a == 0 || a >= max {
        c
    } else {
        ((c * max + a / 2) / a).min(max)
    }
}

impl JxrSession {
    fn decode_full(&self, cancel: &CancelToken, parallel: bool) -> Result<Raster> {
        let opts = jxr::DecodeOptions {
            cancel: Some(cancel.flag()),
            serial: !parallel,
            ..Default::default()
        };
        let img = jxr::decode(self.source.bytes(), &opts).map_err(map_err)?;
        self.to_raster(img, parallel)
    }

    fn to_raster(&self, mut img: jxr::Image, parallel: bool) -> Result<Raster> {
        // 16-bit CMYK goes through the 8-bit converter.
        if img.kind == jxr::Kind::Cmyk
            && let jxr::Pixels::U16(p) = &img.pixels
        {
            img.pixels = jxr::Pixels::U8(
                p.iter()
                    .map(|&v| ((v as u32 * 255 + 32767) / 65535) as u8)
                    .collect(),
            );
        }
        let (w, h) = (img.width, img.height);
        let n = w as usize * h as usize;
        let ch = img.channels();
        let colour = ch - img.alpha as usize;
        let reserve = |len: usize| -> Result<Vec<u8>> {
            let mut v = Vec::new();
            v.try_reserve_exact(len)
                .map_err(|_| DecodeError::TooLarge)?;
            Ok(v)
        };
        let bad = || DecodeError::Malformed("JPEG XR sample count".into());
        match (&img.pixels, self.out) {
            (jxr::Pixels::U8(p), Out::Gray8) if p.len() == n => Ok(whole::interleaved(
                w,
                h,
                PixelLayout::Gray,
                p.clone(),
                img.truncated,
            )),
            (jxr::Pixels::U8(p), Out::Rgba8) if p.len() == n * ch => {
                let mut out = reserve(n * 4)?;
                if img.kind == jxr::Kind::Cmyk {
                    out.resize(n * 4, 0);
                    let conv = specular_color::policy::cmyk_converter(self.cmyk_icc.as_deref());
                    let mut ink = vec![0u8; w as usize * 4];
                    for (src, dst) in p
                        .chunks_exact(w as usize * ch)
                        .zip(out.chunks_exact_mut(w as usize * 4))
                    {
                        for (i, s) in ink.chunks_exact_mut(4).zip(src.chunks_exact(ch)) {
                            i.copy_from_slice(&s[..4]);
                        }
                        conv.convert_row(&ink, dst);
                        if img.alpha {
                            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(ch)) {
                                d[3] = s[4];
                            }
                        }
                    }
                } else {
                    for s in p.chunks_exact(ch) {
                        let a = if img.alpha { s[colour] } else { 255 };
                        let c = |k: usize| {
                            let v = s[if colour == 1 { 0 } else { k }] as u32;
                            if self.premultiplied {
                                unpremultiply_int(v, a as u32, 255) as u8
                            } else {
                                v as u8
                            }
                        };
                        out.extend([c(0), c(1), c(2), a]);
                    }
                }
                Ok(whole::interleaved(
                    w,
                    h,
                    PixelLayout::Rgba,
                    out,
                    img.truncated,
                ))
            }
            (jxr::Pixels::U16(p), Out::Gray16 | Out::GrayAlpha16 | Out::Rgba16)
                if p.len() == n * ch =>
            {
                let (layout, per) = match self.out {
                    Out::Gray16 => (PixelLayout::Gray16, 1),
                    Out::GrayAlpha16 => (PixelLayout::GrayAlpha16, 2),
                    _ => (PixelLayout::Rgba16(HdrEncoding::SRGB), 4),
                };
                let mut out = reserve(n * per * 2)?;
                for s in p.chunks_exact(ch) {
                    let a = if img.alpha { s[colour] } else { 65535 };
                    let c = |k: usize| {
                        let v = s[if colour == 1 { 0 } else { k }] as u32;
                        if self.premultiplied {
                            unpremultiply_int(v, a as u32, 65535) as u16
                        } else {
                            v as u16
                        }
                    };
                    let px: &[u16] = match per {
                        1 => &[c(0)],
                        2 => &[c(0), a],
                        _ => &[c(0), c(1), c(2), a],
                    };
                    for v in px {
                        out.extend_from_slice(&v.to_ne_bytes());
                    }
                }
                Ok(whole::interleaved(w, h, layout, out, img.truncated))
            }
            (jxr::Pixels::F32(p), Out::Float) if p.len() == n * ch => {
                // Grey is spread to RGB; the decoder gives RGB(A) or grey(+alpha).
                let spread;
                let (data, channels): (&[f32], usize) = if colour == 1 {
                    let mut v = Vec::new();
                    v.try_reserve_exact(n * (3 + img.alpha as usize))
                        .map_err(|_| DecodeError::TooLarge)?;
                    for s in p.chunks_exact(ch) {
                        v.extend([s[0], s[0], s[0]]);
                        if img.alpha {
                            v.push(s[1]);
                        }
                    }
                    spread = v;
                    (&spread, 3 + img.alpha as usize)
                } else {
                    (p, ch)
                };
                let mut r = linear::f32_to_raster(
                    w,
                    h,
                    channels,
                    data,
                    None,
                    Primaries::Bt709,
                    img.truncated,
                    parallel,
                );
                if self.premultiplied {
                    linear::unpremultiply(&mut r, parallel);
                }
                Ok(r)
            }
            _ => Err(bad()),
        }
    }
}

impl Session for JxrSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(self.model)
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(if self.premultiplied {
            Alpha::Premultiplied
        } else {
            Alpha::Straight
        })
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| self.decode_full(cancel, req.parallel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        let s = req.scale.max(1);
        let bpp = self.out.bytes();
        let mut m = whole::level_bytes(w, h, s, bpp);
        if !self.levels.has(s) && !self.levels.has(1) {
            // The decoder's native and display samples (f32 for the float formats), then the
            // raster.
            let native = if self.out == Out::Float {
                16 + 16
            } else {
                2 * 8
            };
            m += whole::level_bytes(w, h, 1, native + bpp);
        }
        m
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    /// Drops the cached levels.
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str) -> Arc<dyn Session> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../formats/specular-jxr/tests/vectors")
            .join(name);
        let src = Arc::new(Source::from_vec(name, std::fs::read(path).unwrap()));
        JxrDecoder.open(src).unwrap()
    }

    #[test]
    fn layouts() {
        let cancel = CancelToken::new();
        for (name, layout, alpha) in [
            ("gray8-q.jxr", PixelLayout::Gray, false),
            ("rgb24-q.jxr", PixelLayout::Rgba, false),
            ("bgra32-q-a3.jxr", PixelLayout::Rgba, true),
            ("gray16-q.jxr", PixelLayout::Gray16, false),
            ("rgb48-q.jxr", PixelLayout::Rgba16(HdrEncoding::SRGB), false),
            (
                "rgb128f-q.jxr",
                PixelLayout::Rgba16F(Primaries::Bt709),
                false,
            ),
            (
                "gray32f-q.jxr",
                PixelLayout::Rgba16F(Primaries::Bt709),
                false,
            ),
            ("rgbe-q.jxr", PixelLayout::Rgba16F(Primaries::Bt709), false),
            ("cmyk32-lossless.jxr", PixelLayout::Rgba, false),
            ("cmyka40-lossless-a2.jxr", PixelLayout::Rgba, true),
            ("bw1-lossless.jxr", PixelLayout::Gray, false),
            ("bgr565-q.jxr", PixelLayout::Rgba, false),
            (
                "bgr101010-q.jxr",
                PixelLayout::Rgba16(HdrEncoding::SRGB),
                false,
            ),
        ] {
            let s = open(name);
            assert_eq!(s.info().format, Format::JpegXr);
            assert_eq!(s.info().has_alpha, alpha, "{name}");
            let r = s.decode(&DecodeRequest::full(1), &cancel).unwrap();
            assert_eq!(r.layout, layout, "{name}");
            assert_eq!(
                (r.width, r.height),
                (s.info().width, s.info().height),
                "{name}"
            );
            assert_eq!(
                r.planes[0].data.len(),
                r.width as usize * r.height as usize * layout.pixel_bytes(),
                "{name}"
            );
            let half = s.decode(&DecodeRequest::full(2), &cancel).unwrap();
            assert_eq!(half.width, r.width.div_ceil(2), "{name}");
        }
    }

    #[test]
    fn rejects_garbage() {
        let src = Arc::new(Source::from_vec(
            "t.jxr",
            b"II\xBC\x01\x08\0\0\0\xFF\xFF".to_vec(),
        ));
        assert!(JxrDecoder.open(src).is_err());
    }
}
