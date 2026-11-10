//! Watermarks: a line of text (drawn through the SVG renderer, white with a soft dark
//! outline) or an image with transparency, placed in one of nine positions with a margin
//! of 3 % of the shorter side, at a share of the picture's width and an opacity.

use std::path::Path;

use super::{MarkKind, Watermark};

/// A drawn watermark: straight-alpha sRGB RGBA8 rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// Why a watermark can't be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkError {
    /// The image file can't be read or decoded.
    Image(String),
    /// The text did not draw (no font, or nothing but spaces).
    Text,
}

impl std::fmt::Display for MarkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarkError::Image(e) => write!(f, "the watermark image can't be used: {e}"),
            MarkError::Text => write!(f, "the watermark text can't be drawn"),
        }
    }
}

/// Width text watermarks are drawn at before they are scaled to each picture.
const TEXT_WIDTH: u32 = 2048;

/// Draws the watermark once (text at 2048 px wide, an image at its own size); `None` when
/// the recipe has none.
pub fn prepare(
    w: &Watermark,
    registry: Option<&specular_decode::Registry>,
) -> Result<Option<Mark>, MarkError> {
    match &w.kind {
        MarkKind::None => Ok(None),
        MarkKind::Text(t) => text_mark(t, TEXT_WIDTH).map(Some),
        MarkKind::Image(p) => image_mark(p, registry).map(Some),
    }
}

fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if c.is_control() => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

/// The text drawn about `width` px wide.
pub fn text_mark(text: &str, width: u32) -> Result<Mark, MarkError> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err(MarkError::Text);
    }
    let n = text.chars().count().max(1) as f32;
    // About 0.6 em a character; the font size follows from the width.
    let size = (width as f32 / (0.6 * n + 0.4)).clamp(8.0, 1000.0);
    let w = (size * (0.6 * n + 0.4)).round().max(1.0);
    let h = (size * 1.3).round().max(1.0);
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\
         <text x=\"50%\" y=\"{base:.0}\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"{size:.0}\" \
         fill=\"#ffffff\" stroke=\"#000000\" stroke-opacity=\"0.35\" stroke-width=\"{sw:.1}\" paint-order=\"stroke\">{t}</text></svg>",
        base = size * 1.0,
        sw = (size * 0.04).max(1.0),
        t = xml_escape(&text),
    );
    let (w0, h0, px) = specular_decode::decode_rgba8(svg.as_bytes()).ok_or(MarkError::Text)?;
    if px.chunks_exact(4).all(|p| p[3] == 0) {
        return Err(MarkError::Text);
    }
    Ok(Mark {
        w: w0,
        h: h0,
        rgba: px,
    })
}

/// An image file as a mark (SVG through the SVG renderer, anything else through the
/// registry, transparency kept).
fn image_mark(p: &Path, registry: Option<&specular_decode::Registry>) -> Result<Mark, MarkError> {
    let is_svg = p
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg") || e.eq_ignore_ascii_case("svgz"));
    if is_svg {
        let bytes = std::fs::read(p).map_err(|e| MarkError::Image(e.to_string()))?;
        let (w, h, rgba) = specular_decode::decode_rgba8(&bytes)
            .ok_or_else(|| MarkError::Image("the SVG did not draw".into()))?;
        return Ok(Mark { w, h, rgba });
    }
    let registry = registry.ok_or_else(|| MarkError::Image("no decoders".into()))?;
    let session = registry
        .open_path(p)
        .map_err(|e| MarkError::Image(e.to_string()))?;
    let info = session.info();
    let raster = session
        .decode(
            &specular_decode::DecodeRequest::full(1),
            &specular_core::CancelToken::new(),
        )
        .map_err(|e| MarkError::Image(e.to_string()))?;
    let target = crate::pixels::Target {
        width: raster.width,
        height: raster.height,
        gray: false,
        alpha: true,
        sixteen: false,
        transfer: crate::pixels::OutTransfer::Srgb,
        tone_map: true,
    };
    let px = crate::pixels::render(
        &raster,
        &crate::pixels::ToLinear::Srgb,
        &target,
        info.orientation,
        &specular_core::CancelToken::new(),
    )
    .map_err(|_| MarkError::Image("stopped".into()))?;
    let crate::pixels::Samples::Eight(v) = px.samples else {
        return Err(MarkError::Image("unexpected samples".into()));
    };
    let rgba = match px.channels {
        4 => v,
        3 => v
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        c => return Err(MarkError::Image(format!("{c} channels"))),
    };
    Ok(Mark {
        w: px.width,
        h: px.height,
        rgba,
    })
}

/// Where the mark goes on a `w × h` picture: x, y, width, height in pixels.
pub fn place(w: u32, h: u32, mark: (u32, u32), wm: &Watermark) -> (u32, u32, u32, u32) {
    let margin = (f64::from(w.min(h)) * 0.03).round() as u32;
    let room_w = w.saturating_sub(2 * margin).max(1);
    let room_h = h.saturating_sub(2 * margin).max(1);
    let aspect = f64::from(mark.0.max(1)) / f64::from(mark.1.max(1));
    let mut mw = (f64::from(w) * f64::from(wm.scale.clamp(1, 100)) / 100.0).round();
    mw = mw.min(f64::from(room_w));
    let mut mh = mw / aspect;
    if mh > f64::from(room_h) {
        mh = f64::from(room_h);
        mw = mh * aspect;
    }
    let (mw, mh) = ((mw.round() as u32).max(1), (mh.round() as u32).max(1));
    let (col, row) = wm.position.cell();
    let x = match col {
        0 => margin,
        1 => (w - mw) / 2,
        _ => w.saturating_sub(mw + margin),
    };
    let y = match row {
        0 => margin,
        1 => (h - mh) / 2,
        _ => h.saturating_sub(mh + margin),
    };
    (x, y, mw, mh)
}

/// The mark scaled to `w × h` (area-averaged when shrinking, bilinear when growing; both
/// premultiplied while mixing).
pub fn scaled(m: &Mark, w: u32, h: u32) -> Vec<u8> {
    if (m.w, m.h) == (w, h) {
        return m.rgba.clone();
    }
    let (w0, h0) = (m.w as usize, m.h as usize);
    let (w, h) = (w as usize, h as usize);
    let px = |x: usize, y: usize| {
        let i = (y.min(h0 - 1) * w0 + x.min(w0 - 1)) * 4;
        let a = f32::from(m.rgba[i + 3]) / 255.0;
        [
            f32::from(m.rgba[i]) * a,
            f32::from(m.rgba[i + 1]) * a,
            f32::from(m.rgba[i + 2]) * a,
            a,
        ]
    };
    let fx = w0 as f32 / w as f32;
    let fy = h0 as f32 / h as f32;
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0f32; 4];
            if fx > 1.0 || fy > 1.0 {
                // Area average over the source footprint.
                let (x0, x1) = (
                    (x as f32 * fx) as usize,
                    ((x + 1) as f32 * fx).ceil() as usize,
                );
                let (y0, y1) = (
                    (y as f32 * fy) as usize,
                    ((y + 1) as f32 * fy).ceil() as usize,
                );
                let mut n = 0.0;
                for sy in y0..y1.max(y0 + 1) {
                    for sx in x0..x1.max(x0 + 1) {
                        let p = px(sx, sy);
                        for k in 0..4 {
                            acc[k] += p[k];
                        }
                        n += 1.0;
                    }
                }
                acc.iter_mut().for_each(|v| *v /= n);
            } else {
                let sx = ((x as f32 + 0.5) * fx - 0.5).max(0.0);
                let sy = ((y as f32 + 0.5) * fy - 0.5).max(0.0);
                let (ix, iy) = (sx as usize, sy as usize);
                let (tx, ty) = (sx - ix as f32, sy - iy as f32);
                let (a, b, c, d) = (
                    px(ix, iy),
                    px(ix + 1, iy),
                    px(ix, iy + 1),
                    px(ix + 1, iy + 1),
                );
                for k in 0..4 {
                    acc[k] = (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty)
                        + (c[k] * (1.0 - tx) + d[k] * tx) * ty;
                }
            }
            let o = (y * w + x) * 4;
            if acc[3] > 1e-6 {
                for k in 0..3 {
                    out[o + k] = (acc[k] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = (acc[3] * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}
