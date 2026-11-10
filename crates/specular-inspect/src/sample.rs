//! Values the pixel inspector shows: the decoded (source) samples and the displayed colour.

use specular_core::{PixelLayout, Raster};

use crate::colour::{srgb_encode, to_byte};

/// A pixel as the decoder produced it, before colour management.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceValue {
    Gray(u8),
    Rgb([u8; 3]),
    /// Straight alpha.
    Rgba([u8; 4]),
    /// The stored Y, Cb, Cr samples and their RGB (BT.601 full range, as the renderer
    /// converts them).
    YCbCr {
        ycc: [u8; 3],
        rgb: [u8; 3],
    },
    /// 16-bit samples (0..=65535) as decoded; sources of 9 to 15 bits are scaled to 16.
    Gray16(u16),
    /// Grey and straight alpha.
    GrayAlpha16([u16; 2]),
    /// Straight alpha (65535 for sources without alpha). The codes of the layout's transfer
    /// (sRGB, linear, PQ or HLG), before any decoding.
    Rgba16([u16; 4]),
}

impl SourceValue {
    /// Bits per sample of the value: 8 or 16.
    pub fn bits(&self) -> u32 {
        match self {
            SourceValue::Gray(_)
            | SourceValue::Rgb(_)
            | SourceValue::Rgba(_)
            | SourceValue::YCbCr { .. } => 8,
            SourceValue::Gray16(_) | SourceValue::GrayAlpha16(_) | SourceValue::Rgba16(_) => 16,
        }
    }
}

/// BT.601 full-range (JFIF) YCbCr to RGB, rounded; the renderer's conversion.
pub fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    let y = y as f32;
    let cb = cb as f32 - 128.0;
    let cr = cr as f32 - 128.0;
    let c = |v: f32| (v.round()).clamp(0.0, 255.0) as u8;
    [
        c(y + 1.402 * cr),
        c(y - 0.344_136 * cb - 0.714_136 * cr),
        c(y + 1.772 * cb),
    ]
}

/// The decoded value of stored pixel (`x`, `y`) (full-resolution coordinates) in `r`, a
/// raster of any scale or region; `None` outside it or for a malformed raster.
pub fn source_pixel(r: &Raster, x: u32, y: u32) -> Option<SourceValue> {
    let scale = r.scale.max(1);
    let sx = (x / scale).checked_sub(r.origin.0)?;
    let sy = (y / scale).checked_sub(r.origin.1)?;
    if sx >= r.width || sy >= r.height {
        return None;
    }
    let at = |i: usize, bpp: usize| -> Option<&[u8]> {
        let p = r.planes.get(i)?;
        let px = (sx / p.h_div.max(1) as u32).min(p.width.saturating_sub(1)) as usize;
        let py = (sy / p.v_div.max(1) as u32).min(p.height.saturating_sub(1)) as usize;
        p.data
            .get(py * p.stride + px * bpp..py * p.stride + (px + 1) * bpp)
    };
    match r.layout {
        PixelLayout::Gray => Some(SourceValue::Gray(at(0, 1)?[0])),
        PixelLayout::YCbCr => {
            let ycc = [at(0, 1)?[0], at(1, 1)?[0], at(2, 1)?[0]];
            Some(SourceValue::YCbCr {
                ycc,
                rgb: ycbcr_to_rgb(ycc[0], ycc[1], ycc[2]),
            })
        }
        PixelLayout::Rgb => Some(SourceValue::Rgb([at(0, 1)?[0], at(1, 1)?[0], at(2, 1)?[0]])),
        PixelLayout::Rgba => {
            let p = at(0, 4)?;
            Some(SourceValue::Rgba([p[0], p[1], p[2], p[3]]))
        }
        PixelLayout::Gray16 => Some(SourceValue::Gray16(u16s::<1>(at(0, 2)?)[0])),
        PixelLayout::GrayAlpha16 => Some(SourceValue::GrayAlpha16(u16s(at(0, 4)?))),
        PixelLayout::Rgba16(_) => Some(SourceValue::Rgba16(u16s(at(0, 8)?))),
        // YCbCr planes (HEIC, lossy WebP, video): the R'G'B' the viewer converts them to.
        PixelLayout::Yuv(e) => {
            let v = specular_core::yuv::pixel(r, sx, sy)?;
            Some(if e.rgb_layout() == PixelLayout::Rgba {
                SourceValue::Rgba(v.map(|c| (c * 255.0 + 0.5) as u8))
            } else {
                SourceValue::Rgba16(v.map(|c| (c * 65535.0 + 0.5) as u16))
            })
        }
        // Half floats are linear light, not codes: the displayed value says it all.
        PixelLayout::Rgba16F(_) => None,
    }
}

/// `N` native-endian 16-bit samples of a pixel's bytes.
fn u16s<const N: usize>(b: &[u8]) -> [u16; N] {
    std::array::from_fn(|i| u16::from_ne_bytes([b[2 * i], b[2 * i + 1]]))
}

/// The colour on screen at a pixel, before display colour management: linear sRGB-primaries
/// light (straight alpha) and its 8-bit sRGB encoding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayValue {
    pub linear: [f32; 4],
    pub srgb: [u8; 4],
    /// Read from full-resolution data (otherwise from a reduced level still loading, so
    /// only approximate).
    pub exact: bool,
}

impl DisplayValue {
    /// From the probe's premultiplied linear texel; `level` 0 is full resolution.
    pub fn from_premultiplied(p: [f32; 4], level: u32) -> DisplayValue {
        let a = p[3].clamp(0.0, 1.0);
        let un = |v: f32| {
            if a > 0.0 {
                (v / a).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let linear = [un(p[0]), un(p[1]), un(p[2]), a];
        DisplayValue {
            linear,
            srgb: [
                to_byte(srgb_encode(linear[0])),
                to_byte(srgb_encode(linear[1])),
                to_byte(srgb_encode(linear[2])),
                to_byte(a),
            ],
            exact: level == 0,
        }
    }
}

/// `#RRGGBB`.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}
