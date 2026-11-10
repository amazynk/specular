//! Flattening of an OpenRaster layer stack with the blend modes and compositing operators
//! of the OpenRaster specification.
//!
//! Works on the stored (sRGB-encoded) values, as OpenRaster's operators are defined: the
//! canvas is premultiplied 16-bit fixed point; a layer or group drawn with `svg:src-over`
//! takes a fixed-point fast path, any other `composite-op` goes through the W3C formulas of
//! [`specular_blend`] (blend modes such as `svg:multiply` and `svg:hue`, and the
//! Porter-Duff operators `svg:plus`, `svg:dst-in`, `svg:dst-out`, `svg:src-atop`,
//! `svg:dst-atop` and the others W3C names). Krita's own `krita:` operators are mapped
//! through its composite op ids; an operator nobody defines is drawn as `svg:src-over`, as
//! the specification asks. A stack is isolated (drawn into its own buffer, then composited
//! into its parent) when its `isolation` says so, when its opacity is below 1 or its
//! operator is not `svg:src-over`; otherwise its children are drawn straight into the
//! parent.

use specular_blend::{Mode, PorterDuff};

use crate::stack::{Item, Node, PixelLayer, Stack};
use crate::{Document, Error, MAX_CANVAS_PIXELS};

/// A `composite-op`: a blend mode (composited source-over) or a Porter-Duff operator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Op {
    Blend(Mode),
    Porter(PorterDuff),
}

impl Op {
    /// The operator an OpenRaster `composite-op` names; unknown ones are `svg:src-over`.
    pub(crate) fn parse(s: &str) -> Op {
        if let Some(m) = Mode::from_openraster(s) {
            return Op::Blend(m);
        }
        if let Some(p) = PorterDuff::from_openraster(s) {
            return Op::Porter(p);
        }
        match s.strip_prefix("krita:") {
            Some("erase") => Op::Porter(PorterDuff::DestinationOut),
            Some("behind") => Op::Porter(PorterDuff::DestinationOver),
            Some(id) => Op::Blend(Mode::from_krita(id).unwrap_or(Mode::Normal)),
            None => Op::Blend(Mode::Normal),
        }
    }

    fn normal(self) -> bool {
        matches!(
            self,
            Op::Blend(Mode::Normal) | Op::Porter(PorterDuff::SourceOver)
        )
    }

    /// Whether the operator changes the backdrop where the source is transparent (so it
    /// must run over the whole canvas, not just the layer's rectangle).
    fn reaches_outside(self) -> bool {
        match self {
            Op::Blend(_) => false,
            Op::Porter(p) => p.factors(0.0, 1.0).1 != 1.0,
        }
    }
}

/// Composites straight colour `cs` with alpha `a_s` (0..1) into premultiplied 16-bit `d`.
#[inline]
fn put(d: &mut [u16], cs: [f32; 3], a_s: f32, op: Op) {
    let ab = f32::from(d[3]) / 65535.0;
    let cb = if ab > 0.0 {
        [0, 1, 2].map(|c| (f32::from(d[c]) / 65535.0 / ab).min(1.0))
    } else {
        [0.0; 3]
    };
    let (mode, pd) = match op {
        Op::Blend(m) => (m, PorterDuff::SourceOver),
        Op::Porter(p) => (Mode::Normal, p),
    };
    let (c, a) = specular_blend::composite(mode, pd, cb, ab, cs, a_s.clamp(0.0, 1.0));
    for k in 0..3 {
        d[k] = (c[k].clamp(0.0, 1.0) * a * 65535.0 + 0.5) as u16;
    }
    d[3] = (a.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
}

/// Live group buffers (each the canvas size) allowed at once, in canvas
/// pixels, so nested translucent groups stay bounded.
const MAX_LIVE_PIXELS: u64 = 2 * MAX_CANVAS_PIXELS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    /// Straight RGBA8, row-major.
    pub pixels: Vec<u8>,
    /// A layer's PNG data ended early or was corrupt (its missing rows are mid-grey).
    pub truncated: bool,
}

struct Ctx<'a> {
    doc: &'a Document,
    data: &'a [u8],
    w: usize,
    h: usize,
    live: u64,
    cancel: &'a dyn Fn() -> bool,
    truncated: std::cell::Cell<bool>,
}

pub(crate) fn run(
    doc: &Document,
    data: &[u8],
    stack: &Stack,
    cancel: &dyn Fn() -> bool,
) -> Result<Rgba, Error> {
    let px = u64::from(stack.width) * u64::from(stack.height);
    if px > MAX_CANVAS_PIXELS {
        return Err(Error::TooLarge(format!(
            "{}×{} canvas",
            stack.width, stack.height
        )));
    }
    let (w, h) = (stack.width as usize, stack.height as usize);
    let mut ctx = Ctx {
        doc,
        data,
        w,
        h,
        live: px,
        cancel,
        truncated: std::cell::Cell::new(false),
    };
    let mut canvas = vec![0u16; w * h * 4];
    let mut any = false;
    draw(&mut ctx, &stack.root, 0, 0, &mut canvas, &mut any)?;
    if !any {
        return Err(Error::NoImage(crate::Kind::Ora));
    }
    let mut pixels = vec![0u8; w * h * 4];
    for (o, p) in pixels.chunks_exact_mut(4).zip(canvas.chunks_exact(4)) {
        let a = u32::from(p[3]);
        if a == 0 {
            continue;
        }
        for c in 0..3 {
            o[c] = ((u32::from(p[c]) * 255 + a / 2) / a).min(255) as u8;
        }
        o[3] = ((a * 255 + 32767) / 65535) as u8;
    }
    Ok(Rgba {
        width: stack.width,
        height: stack.height,
        pixels,
        truncated: ctx.truncated.get(),
    })
}

/// `dst = src + dst * (1 - src_alpha)` on premultiplied 16-bit values.
#[inline]
fn over(dst: &mut [u16], src: [u32; 4]) {
    // Opaque source or empty destination: a plain copy.
    if src[3] >= 65535 || dst[3] == 0 {
        for c in 0..4 {
            dst[c] = src[c].min(65535) as u16;
        }
        return;
    }
    let inv = 65535 - src[3];
    for c in 0..4 {
        dst[c] = (src[c] + (u32::from(dst[c]) * inv + 32767) / 65535).min(65535) as u16;
    }
}

/// Draws `items` (first on top) into `dst`, offset by `(ox, oy)`.
fn draw(
    ctx: &mut Ctx<'_>,
    items: &[Item],
    ox: i64,
    oy: i64,
    dst: &mut [u16],
    any: &mut bool,
) -> Result<(), Error> {
    for it in items.iter().rev() {
        if !it.visible || it.opacity <= 0.0 {
            continue;
        }
        if (ctx.cancel)() {
            return Err(Error::Cancelled);
        }
        let op = Op::parse(&it.composite_op);
        let opacity = (f64::from(it.opacity) * 65535.0).round() as u32;
        match &it.node {
            Node::Layer(l) => {
                if l.src.is_empty() {
                    return Err(Error::Unsupported(
                        "OpenRaster text and filter layers".into(),
                    ));
                }
                layer(ctx, &l.src, ox + l.x, oy + l.y, opacity, op, dst)?;
                *any = true;
            }
            Node::Stack(children) => {
                let (gx, gy) = (ox + it.x, oy + it.y);
                if opacity >= 65535 && op.normal() && !it.isolate {
                    draw(ctx, children, gx, gy, dst, any)?;
                    continue;
                }
                let px = (ctx.w * ctx.h) as u64;
                if ctx.live + px > MAX_LIVE_PIXELS {
                    return Err(Error::TooLarge(
                        "nesting of translucent layer groups".into(),
                    ));
                }
                ctx.live += px;
                let mut group = vec![0u16; ctx.w * ctx.h * 4];
                let r = draw(ctx, children, gx, gy, &mut group, any);
                ctx.live -= px;
                r?;
                let k = opacity as f32 / 65535.0;
                let outside = op.reaches_outside();
                for (d, s) in dst.chunks_exact_mut(4).zip(group.chunks_exact(4)) {
                    if s[3] == 0 && !outside {
                        continue;
                    }
                    if op.normal() {
                        let src = [0, 1, 2, 3].map(|c| (u32::from(s[c]) * opacity + 32767) / 65535);
                        over(d, src);
                        continue;
                    }
                    let a = f32::from(s[3]) / 65535.0;
                    let cs = if a > 0.0 {
                        [0, 1, 2].map(|c| (f32::from(s[c]) / 65535.0 / a).min(1.0))
                    } else {
                        [0.0; 3]
                    };
                    put(d, cs, a * k, op);
                }
            }
        }
    }
    Ok(())
}

/// Decodes layer member `src` to straight RGBA8.
fn decode_png(doc: &Document, data: &[u8], src: &str) -> Result<specular_png::Image, Error> {
    let e = doc.zip.find(src).ok_or_else(|| {
        Error::Unsupported(format!("OpenRaster layer {src} (missing from the file)"))
    })?;
    let png = doc.read(data, e)?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::Unsupported(format!(
            "OpenRaster layer {src} (only PNG layers)"
        )));
    }
    let info =
        specular_png::read_info(&png).map_err(|err| Error::Png(src.into(), err.to_string()))?;
    if u64::from(info.width) * u64::from(info.height) > MAX_CANVAS_PIXELS {
        return Err(Error::TooLarge(format!("layer {src}")));
    }
    specular_png::decode(
        &png,
        &specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            ..Default::default()
        },
    )
    .map_err(|err| Error::Png(src.into(), err.to_string()))
}

/// One layer alone on a `w`×`h` canvas (straight RGBA8), its alpha scaled by its opacity.
pub(crate) fn layer_page(
    doc: &Document,
    data: &[u8],
    l: &PixelLayer,
    w: u32,
    h: u32,
    cancel: &dyn Fn() -> bool,
) -> Result<Rgba, Error> {
    if u64::from(w) * u64::from(h) > MAX_CANVAS_PIXELS {
        return Err(Error::TooLarge(format!("{w}×{h} canvas")));
    }
    if cancel() {
        return Err(Error::Cancelled);
    }
    let img = decode_png(doc, data, &l.src)?;
    if cancel() {
        return Err(Error::Cancelled);
    }
    let plane = &img.planes[0];
    let (cw, ch) = (i64::from(w), i64::from(h));
    let (lw, lh) = (i64::from(img.width), i64::from(img.height));
    let (x0, x1) = (l.x.clamp(0, cw), l.x.saturating_add(lw).clamp(0, cw));
    let (y0, y1) = (l.y.clamp(0, ch), l.y.saturating_add(lh).clamp(0, ch));
    let opacity = (f64::from(l.opacity.clamp(0.0, 1.0)) * 65535.0).round() as u32;
    let size = w as usize * h as usize * 4;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(size)
        .map_err(|_| Error::TooLarge(format!("{w}×{h} canvas")))?;
    pixels.resize(size, 0);
    let stride = w as usize * 4;
    for cy in y0..y1 {
        if (cy - y0) % 64 == 63 && cancel() {
            return Err(Error::Cancelled);
        }
        let srow = &plane.data[(cy - l.y) as usize * plane.stride..];
        let drow = &mut pixels[cy as usize * stride..][..stride];
        for cx in x0..x1 {
            let s = &srow[(cx - l.x) as usize * 4..][..4];
            let a = (u32::from(s[3]) * opacity + 32767) / 65535;
            if a != 0 {
                drow[cx as usize * 4..][..4].copy_from_slice(&[s[0], s[1], s[2], a as u8]);
            }
        }
    }
    Ok(Rgba {
        width: w,
        height: h,
        pixels,
        truncated: img.truncated,
    })
}

fn layer(
    ctx: &Ctx<'_>,
    src: &str,
    x: i64,
    y: i64,
    opacity: u32,
    op: Op,
    dst: &mut [u16],
) -> Result<(), Error> {
    let img = decode_png(ctx.doc, ctx.data, src)?;
    if img.truncated {
        ctx.truncated.set(true);
    }
    let plane = &img.planes[0];
    let (lw, lh) = (img.width as i64, img.height as i64);
    let (cw, ch) = (ctx.w as i64, ctx.h as i64);
    let x0 = x.max(0);
    let x1 = (x + lw).min(cw);
    let y0 = y.max(0);
    let y1 = (y + lh).min(ch);
    if !op.normal() {
        let k = opacity as f32 / 65535.0;
        let outside = op.reaches_outside();
        for cy in 0..ch {
            let inside_row = (y0..y1).contains(&cy);
            let drow = &mut dst[cy as usize * ctx.w * 4..(cy as usize + 1) * ctx.w * 4];
            for cx in 0..cw {
                let d = &mut drow[cx as usize * 4..cx as usize * 4 + 4];
                if inside_row && (x0..x1).contains(&cx) {
                    let o = (cy - y) as usize * plane.stride + (cx - x) as usize * 4;
                    let s = &plane.data[o..o + 4];
                    if s[3] == 0 && !outside {
                        continue;
                    }
                    let cs = [0, 1, 2].map(|c| f32::from(s[c]) / 255.0);
                    put(d, cs, f32::from(s[3]) / 255.0 * k, op);
                } else if outside {
                    put(d, [0.0; 3], 0.0, op);
                }
            }
        }
        return Ok(());
    }
    for cy in y0..y1 {
        let srow = &plane.data[(cy - y) as usize * plane.stride..];
        let drow = &mut dst[cy as usize * ctx.w * 4..(cy as usize + 1) * ctx.w * 4];
        for cx in x0..x1 {
            let s = &srow[(cx - x) as usize * 4..(cx - x) as usize * 4 + 4];
            if s[3] == 0 {
                continue;
            }
            // Straight 8-bit to premultiplied 16-bit, with layer opacity.
            let a = (u32::from(s[3]) * 257 * opacity + 32767) / 65535;
            let src = [
                (u32::from(s[0]) * 257 * a + 32767) / 65535,
                (u32::from(s[1]) * 257 * a + 32767) / 65535,
                (u32::from(s[2]) * 257 * a + 32767) / 65535,
                a,
            ];
            over(&mut drow[cx as usize * 4..cx as usize * 4 + 4], src);
        }
    }
    Ok(())
}
