//! Non-raw sources for Develop: a decoded image through its embedded
//! ICC profile into linear Rec.2020, unclipped, with its transparency kept.
//!
//! - **Profiles.** RGB and grey profiles (matrix/shaper or LUT-based) convert the file's
//!   values to linear light ([`specular_color::gpu::source_lut_for`]); untagged files and
//!   sRGB-like profiles take the sRGB curve. A Display P3 or Adobe RGB photo keeps its
//!   colours outside sRGB (they come out as Rec.2020 values, negative in BT.709 terms
//!   before the matrix).
//! - **HDR files** keep their light above SDR white (the view transform maps it).
//! - **CMYK and Lab** files were turned into RGB by their decoder (through their profile);
//!   [`Note::Cmyk`] says so in words.
//! - **Transparency** is straight alpha in the fourth value; colour is unpremultiplied, so
//!   the edit works on the colours as they are.

use specular_convert::pixels::{ToLinear, linear_rgba};
use specular_core::{CancelToken, PixelLayout, Raster};

use crate::preview::Input;

/// Something to tell the user about a source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// A CMYK (or YCCK) file: edited in RGB, converted with its profile on opening.
    Cmyk,
}

impl Note {
    /// The note for a decoder's colour model (`specular_decode`'s `colour_model`).
    pub fn of_colour_model(model: Option<&str>) -> Option<Note> {
        matches!(model, Some("CMYK" | "YCCK")).then_some(Note::Cmyk)
    }

    /// The note for a source: its decoder's colour model, else its embedded profile's
    /// colour space, else (JPEG) four components in its frame header. `head` is the start
    /// of the file.
    pub fn of_source(model: Option<&str>, icc: Option<&[u8]>, head: &[u8]) -> Option<Note> {
        let cmyk_icc = icc.is_some_and(|b| b.get(16..20) == Some(b"CMYK"));
        (Self::of_colour_model(model).is_some() || cmyk_icc || jpeg_components(head) == Some(4))
            .then_some(Note::Cmyk)
    }
}

/// The component count of a JPEG's frame header, when `d` starts a JPEG and holds it.
fn jpeg_components(d: &[u8]) -> Option<u8> {
    if d.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut at = 2;
    loop {
        if *d.get(at)? != 0xFF {
            return None;
        }
        let m = *d.get(at + 1)?;
        if m == 0xFF {
            at += 1;
            continue;
        }
        let len = usize::from(u16::from_be_bytes([*d.get(at + 2)?, *d.get(at + 3)?]));
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            return d.get(at + 9).copied();
        }
        if m == 0xDA {
            return None;
        }
        at += 2 + len;
    }
}

/// How the raster's values become linear light: its embedded profile when it is an RGB or
/// grey one that applies to the stored samples (not HDR-coded ones), else the sRGB curve.
pub fn to_linear(raster: &Raster, icc: Option<&[u8]>) -> ToLinear {
    let applies = match raster.layout {
        PixelLayout::Rgba16(e) => e == specular_core::hdr::HdrEncoding::SRGB,
        PixelLayout::Rgba16F(_) => false,
        _ => true,
    };
    icc.filter(|_| applies)
        .and_then(|b| specular_color::Profile::parse(b).ok())
        .and_then(|p| {
            let pol = specular_color::policy();
            specular_color::gpu::source_lut_for(&p, pol.intent, pol.bpc)
        })
        .map_or(ToLinear::Srgb, ToLinear::Lut)
}

/// The raster as Develop's input: linear Rec.2020 RGBA, straight alpha, area-reduced to a
/// long side of at most `long` when given. `None` when cancelled.
pub fn input(
    raster: &Raster,
    icc: Option<&[u8]>,
    long: Option<u32>,
    cancel: &CancelToken,
) -> Option<Input> {
    let lin = linear_rgba(raster, &to_linear(raster, icc), false, cancel).ok()?;
    let (w, h) = (raster.width, raster.height);
    let (lin, w, h) = match long {
        Some(l) if w.max(h) > l => reduce(&lin, w, h, l),
        _ => (lin, w, h),
    };
    Some(Input::Rgb {
        width: w,
        height: h,
        rgba: to_working(lin),
    })
}

/// Premultiplied linear BT.709 RGBA → straight Rec.2020 RGBA (in place).
fn to_working(mut px: Vec<f32>) -> Vec<f32> {
    use rayon::prelude::*;
    let m = crate::ops::m3_f32(&crate::colour::rec709_to_rec2020());
    px.par_chunks_mut(4).for_each(|p| {
        let a = p[3];
        let k = if a > 0.0 { 1.0 / a } else { 0.0 };
        let v = [p[0] * k, p[1] * k, p[2] * k].map(|v| if v.is_finite() { v } else { 0.0 });
        for i in 0..3 {
            p[i] = m[i * 3] * v[0] + m[i * 3 + 1] * v[1] + m[i * 3 + 2] * v[2];
        }
    });
    px
}

/// Box (area) reduction of premultiplied RGBA to a long side of `long`.
fn reduce(px: &[f32], w: u32, h: u32, long: u32) -> (Vec<f32>, u32, u32) {
    use rayon::prelude::*;
    let k = f64::from(long) / f64::from(w.max(h));
    let dw = ((f64::from(w) * k).round() as u32).max(1);
    let dh = ((f64::from(h) * k).round() as u32).max(1);
    let (sx, sy) = (w as f32 / dw as f32, h as f32 / dh as f32);
    let mut out = vec![0f32; dw as usize * dh as usize * 4];
    out.par_chunks_mut(dw as usize * 4)
        .enumerate()
        .for_each(|(oy, row)| {
            let y0 = oy as f32 * sy;
            let y1 = y0 + sy;
            for (ox, o) in row.chunks_exact_mut(4).enumerate() {
                let x0 = ox as f32 * sx;
                let x1 = x0 + sx;
                let mut acc = [0f32; 4];
                let mut ws = 0f32;
                for y in (y0.floor() as u32)..(y1.ceil() as u32).min(h) {
                    let wy = (y as f32 + 1.0).min(y1) - (y as f32).max(y0);
                    for x in (x0.floor() as u32)..(x1.ceil() as u32).min(w) {
                        let wx = (x as f32 + 1.0).min(x1) - (x as f32).max(x0);
                        let wgt = wx * wy;
                        if wgt <= 0.0 {
                            continue;
                        }
                        let i = (y as usize * w as usize + x as usize) * 4;
                        for c in 0..4 {
                            acc[c] += px[i + c] * wgt;
                        }
                        ws += wgt;
                    }
                }
                for c in 0..4 {
                    o[c] = acc[c] / ws.max(1e-12);
                }
            }
        });
    (out, dw, dh)
}

#[cfg(test)]
mod tests;
