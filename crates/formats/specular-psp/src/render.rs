//! Rendering: stored composites and thumbnails, single layers, and the layer stack
//! composited in 16-bit straight RGBA.

use crate::blend::blend;
use crate::channel::Plane;
use crate::{
    BlendMode, Channel, Compression, Error, Image, LayerKind, LayerRec, Node, Options, PixelFormat,
    Psp, Stored, StoredBody,
};
use rayon::prelude::*;

/// A canvas of straight RGBA, 16 bits per sample.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u16>,
    _budget: specular_core::Reservation,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Result<Canvas, Error> {
        let n = (w as usize)
            .checked_mul(h as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(Error::TooLarge)?;
        let budget = specular_core::budget::reserve(n as u64 * 2)?;
        Ok(Canvas {
            w: w as usize,
            h: h as usize,
            px: vec![0; n],
            _budget: budget,
        })
    }
}

/// A layer's pixels: straight RGBA16 over its rect.
struct LayerPix {
    x: i64,
    y: i64,
    w: usize,
    h: usize,
    rgba: Vec<u16>,
}

fn grey_level(v: u16, bits: u32) -> u16 {
    match bits {
        1 => v * 65535,
        4 => v * 4369,
        8 => v * 257,
        _ => v,
    }
}

fn find<'c>(chans: &'c [Channel], dibs: &[u16], kind: Option<u16>) -> Option<&'c Channel> {
    chans
        .iter()
        .find(|c| dibs.contains(&c.dib) && kind.is_none_or(|k| c.kind == k))
}

/// Colour planes of a bitmap into straight RGBA16 (alpha from `alpha` or opaque).
#[allow(clippy::too_many_arguments)]
fn bitmap(
    data: &[u8],
    chans: &[Channel],
    colour_dibs: &[u16],
    alpha_dibs: &[u16],
    bit_depth: u16,
    greyscale: bool,
    palette: Option<&[[u8; 3]]>,
    comp: Compression,
    w: usize,
    h: usize,
    truncated: &mut bool,
) -> Result<Vec<u16>, Error> {
    let n = w.checked_mul(h).ok_or(Error::TooLarge)?;
    let _r = specular_core::budget::reserve(n as u64 * 8)?;
    let wide = bit_depth == 48 || bit_depth == 16;
    let rgb = bit_depth >= 24;
    let bits = match bit_depth {
        48 => 16,
        24 => 8,
        b => u32::from(b),
    };
    // The channels to read: R, G, B (or the one index/grey channel), then alpha.
    let colour: Vec<Option<&Channel>> = if rgb {
        (1..=3).map(|k| find(chans, colour_dibs, Some(k))).collect()
    } else {
        vec![find(chans, colour_dibs, Some(0)).or_else(|| find(chans, colour_dibs, None))]
    };
    let alpha = find(chans, alpha_dibs, None);
    let jobs: Vec<(Option<&Channel>, u32)> = colour
        .iter()
        .map(|c| (*c, bits))
        .chain(alpha.map(|a| (Some(a), if wide { 16 } else { 8 })))
        .collect();
    let read = |(c, b): &(Option<&Channel>, u32)| -> Result<Option<Plane>, Error> {
        let Some(c) = c else { return Ok(None) };
        let mut p = Plane::read(data, c, comp, *b, w, h)?;
        if *b == 16 && !p.complete() && alpha.is_some_and(|a| std::ptr::eq(a, *c)) {
            // 16-bit documents may still keep 8-bit transparency.
            p = Plane::read(data, c, comp, 8, w, h)?;
        }
        Ok(Some(p))
    };
    let mut planes: Vec<Option<Plane>> = jobs.par_iter().map(read).collect::<Result<_, _>>()?;
    let alpha_plane = if alpha.is_some() {
        planes.pop().flatten()
    } else {
        None
    };
    *truncated |= planes
        .iter()
        .any(|p| p.as_ref().is_none_or(|p| !p.complete()))
        || alpha_plane.as_ref().is_some_and(|p| !p.complete());
    let pal = palette.filter(|p| !p.is_empty() && !(greyscale && wide));
    let mut out = vec![0u16; n * 4];
    out.par_chunks_mut((w * 4).max(1))
        .enumerate()
        .for_each(|(y, row)| {
            for (x, o) in row.chunks_exact_mut(4).enumerate() {
                if rgb {
                    for (k, p) in planes.iter().enumerate() {
                        o[k] = p.as_ref().map_or(0, |p| p.get16(x, y));
                    }
                } else {
                    let v = planes[0].as_ref().map_or(0, |p| p.get(x, y));
                    let c = match pal {
                        Some(pal) => pal
                            .get(v as usize)
                            .copied()
                            .unwrap_or([0; 3])
                            .map(|c| u16::from(c) * 257),
                        None => [grey_level(v, bits); 3],
                    };
                    o[..3].copy_from_slice(&c);
                }
                o[3] = match &alpha_plane {
                    Some(p) => p.get16(x, y),
                    None => 65535,
                };
            }
        });
    Ok(out)
}

fn decode_layer(
    psp: &Psp,
    data: &[u8],
    rec: &LayerRec,
    truncated: &mut bool,
) -> Result<LayerPix, Error> {
    let (x, y, w, h) = rec.public.rect;
    let (w, h) = (w as usize, h as usize);
    if (w as u64) * (h as u64) > crate::MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let i = &psp.info;
    // No channel can expand more than DEFLATE's ~1032:1 (RLE: 64:1): a layer claiming more
    // pixels than its stored bytes could hold is damaged, and is not allocated.
    let stored: u64 = rec.channels.iter().map(|c| c.data.len() as u64).sum();
    let ratio = match i.compression {
        Compression::Lz77 => 1100,
        Compression::Rle => 70,
        _ => 1,
    };
    if (w as u64) * (h as u64) > (stored + 64) * ratio * 8 {
        return Err(Error::Malformed(format!(
            "layer of {w}×{h} pixels with {stored} bytes of data"
        )));
    }
    let mut rgba = bitmap(
        data,
        &rec.channels,
        &[0],
        &[1],
        i.bit_depth,
        i.greyscale,
        psp.palette.as_deref(),
        i.compression,
        w,
        h,
        truncated,
    )?;
    let (mx, my, mw, mh) = rec.mask_rect;
    if rec.public.has_mask
        && !rec.mask_disabled
        && let Some(c) = find(&rec.channels, &[2], None)
    {
        let p = Plane::read(data, c, i.compression, 8, mw as usize, mh as usize)?;
        *truncated |= !p.complete();
        for ly in 0..h {
            for lx in 0..w {
                let cx = x as i64 + lx as i64 - mx as i64;
                let cy = y as i64 + ly as i64 - my as i64;
                // Outside the stored mask the mask shows the layer.
                let m = if cx >= 0 && cy >= 0 && (cx as u32) < mw && (cy as u32) < mh {
                    p.get(cx as usize, cy as usize)
                } else {
                    255
                };
                let m = if rec.mask_invert { 255 - m } else { m };
                let a = &mut rgba[(ly * w + lx) * 4 + 3];
                *a = ((u32::from(*a) * u32::from(m) + 127) / 255) as u16;
            }
        }
    }
    Ok(LayerPix {
        x: x.into(),
        y: y.into(),
        w,
        h,
        rgba,
    })
}

/// A deterministic per-pixel threshold for Dissolve.
fn noise(x: usize, y: usize) -> f32 {
    let mut v = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    v ^= v >> 15;
    v = v.wrapping_mul(0x2C1B_3C6D);
    v ^= v >> 12;
    (v >> 8) as f32 / (1u32 << 24) as f32
}

/// Composites `src` (straight RGBA16 over `sw`×`sh` at `sx`,`sy` on the canvas) with
/// `opacity` (0..=1) and `mode`.
#[allow(clippy::too_many_arguments)]
fn composite(
    dst: &mut Canvas,
    src: &[u16],
    (sx, sy, sw, sh): (i64, i64, usize, usize),
    opacity: f32,
    mode: BlendMode,
    opts: &Options,
) -> Result<(), Error> {
    let cw = dst.w;
    let x0 = sx.max(0) as usize;
    let x1 = (sx + sw as i64).clamp(0, cw as i64) as usize;
    let y0 = sy.max(0) as usize;
    let y1 = (sy + sh as i64).clamp(0, dst.h as i64) as usize;
    if x0 >= x1 || y0 >= y1 || opacity <= 0.0 {
        return Ok(());
    }
    let row = |(cy, drow): (usize, &mut [u16])| {
        if opts.cancelled() {
            return;
        }
        let ly = (cy as i64 - sy) as usize;
        for cx in x0..x1 {
            let lx = (cx as i64 - sx) as usize;
            let s = &src[(ly * sw + lx) * 4..(ly * sw + lx) * 4 + 4];
            let mut a_s = f32::from(s[3]) / 65535.0 * opacity;
            if mode == BlendMode::Dissolve {
                a_s = if noise(cx, cy) < a_s { 1.0 } else { 0.0 };
            }
            if a_s <= 0.0 {
                continue;
            }
            let d = &mut drow[cx * 4..cx * 4 + 4];
            let a_b = f32::from(d[3]) / 65535.0;
            let cs = [s[0], s[1], s[2]].map(|v| f32::from(v) / 65535.0);
            let cb = [d[0], d[1], d[2]].map(|v| f32::from(v) / 65535.0);
            let bl = if a_b > 0.0 { blend(mode, cb, cs) } else { cs };
            let a_o = a_s + a_b * (1.0 - a_s);
            for k in 0..3 {
                let co = a_s * (1.0 - a_b) * cs[k] + a_s * a_b * bl[k] + (1.0 - a_s) * a_b * cb[k];
                d[k] = (co / a_o * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16;
            }
            d[3] = (a_o * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16;
        }
    };
    let stride = cw * 4;
    let rows = dst.px[y0 * stride..y1 * stride].chunks_mut(stride);
    if opts.parallel {
        rows.collect::<Vec<_>>()
            .into_par_iter()
            .enumerate()
            .map(|(i, r)| (y0 + i, r))
            .for_each(row);
    } else {
        rows.enumerate().map(|(i, r)| (y0 + i, r)).for_each(row);
    }
    if opts.cancelled() {
        return Err(Error::Cancelled);
    }
    Ok(())
}

/// Applies a mask layer to what is below it in its group: alpha times the mask (blended with
/// the mask layer's opacity).
fn apply_mask(dst: &mut Canvas, m: &LayerPix, opacity: f32) {
    let (w, h) = (dst.w, dst.h);
    for y in 0..h {
        for x in 0..w {
            let lx = x as i64 - m.x;
            let ly = y as i64 - m.y;
            let v = if lx >= 0 && ly >= 0 && (lx as usize) < m.w && (ly as usize) < m.h {
                let i = (ly as usize * m.w + lx as usize) * 4;
                // Grey mask: its luminance, times its own transparency.
                let g = f32::from(m.rgba[i]) / 65535.0;
                let a = f32::from(m.rgba[i + 3]) / 65535.0;
                g * a
            } else {
                0.0
            };
            let f = 1.0 - opacity * (1.0 - v);
            let a = &mut dst.px[(y * w + x) * 4 + 3];
            *a = (f32::from(*a) * f + 0.5) as u16;
        }
    }
}

fn composite_nodes(
    psp: &Psp,
    data: &[u8],
    nodes: &[Node],
    canvas: &mut Canvas,
    opts: &Options,
    truncated: &mut bool,
) -> Result<(), Error> {
    for n in nodes {
        if opts.cancelled() {
            return Err(Error::Cancelled);
        }
        let rec = &psp.recs[n.rec];
        let l = &rec.public;
        if !l.visible {
            continue;
        }
        let opacity = f32::from(l.opacity) / 255.0;
        match l.kind {
            LayerKind::Group => {
                let mut g = Canvas::new(canvas.w as u32, canvas.h as u32)?;
                composite_nodes(psp, data, &n.children, &mut g, opts, truncated)?;
                let (w, h) = (canvas.w, canvas.h);
                let px = std::mem::take(&mut g.px);
                composite(canvas, &px, (0, 0, w, h), opacity, l.blend, opts)?;
            }
            LayerKind::Mask if l.has_pixels => {
                let m = decode_layer(psp, data, rec, truncated)?;
                apply_mask(canvas, &m, opacity);
            }
            LayerKind::Vector if rec.vector.as_ref().is_some_and(|v| !v.list.is_empty()) => {
                let px = vector_layer(rec, canvas.w, canvas.h)?;
                let (w, h) = (canvas.w, canvas.h);
                composite(canvas, &px, (0, 0, w, h), opacity, l.blend, opts)?;
            }
            _ if l.has_pixels => {
                let p = decode_layer(psp, data, rec, truncated)?;
                composite(
                    canvas,
                    &p.rgba,
                    (p.x, p.y, p.w, p.h),
                    opacity,
                    l.blend,
                    opts,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// A vector layer's shapes drawn over the canvas (straight RGBA16).
fn vector_layer(rec: &LayerRec, w: usize, h: usize) -> Result<Vec<u16>, Error> {
    let _r = specular_core::budget::reserve((w * h * 12) as u64)?;
    Ok(rec.vector.as_ref().map_or_else(
        || vec![0; w * h * 4],
        |v| crate::vector::raster(&v.list, w, h),
    ))
}

/// Packs a canvas into the output format.
fn finish(c: &Canvas, wide: bool, grey: bool, truncated: bool) -> Result<Image, Error> {
    let n = c.w * c.h;
    let opaque = c.px.chunks_exact(4).all(|p| p[3] == 65535);
    let (format, bpp) = if wide {
        (PixelFormat::Rgba16, 8)
    } else if grey && opaque {
        (PixelFormat::Gray8, 1)
    } else {
        (PixelFormat::Rgba8, 4)
    };
    let _r = specular_core::budget::reserve((n * bpp) as u64)?;
    let to8 = |v: u16| ((u32::from(v) * 255 + 32767) / 65535) as u8;
    let data = match format {
        PixelFormat::Rgba16 => c.px.iter().flat_map(|v| v.to_ne_bytes()).collect(),
        PixelFormat::Gray8 => c.px.chunks_exact(4).map(|p| to8(p[0])).collect(),
        PixelFormat::Rgba8 => c.px.iter().map(|&v| to8(v)).collect(),
    };
    Ok(Image {
        width: c.w as u32,
        height: c.h as u32,
        format,
        data,
        truncated,
    })
}

fn decode_stored(psp: &Psp, data: &[u8], s: &Stored) -> Result<Image, Error> {
    if u64::from(s.width) * u64::from(s.height) > crate::MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    match &s.body {
        StoredBody::Jpeg(r) => {
            let j = data.get(r.clone()).unwrap_or(&[]);
            let opts = specular_jpeg::DecodeOptions::default();
            let p = specular_jpeg::decode(j, &opts)
                .map_err(|e| Error::Malformed(format!("JPEG composite: {e}")))?;
            let rgba = specular_jpeg::planes_to_rgba(&p);
            Ok(Image {
                width: p.width,
                height: p.height,
                format: PixelFormat::Rgba8,
                data: rgba,
                truncated: p.truncated,
            })
        }
        StoredBody::Channels { palette, channels } => {
            let mut truncated = false;
            let (w, h) = (s.width as usize, s.height as usize);
            let bytes: u64 = channels.iter().map(|c| c.data.len() as u64).sum();
            if (w as u64) * (h as u64) > (bytes + 64) * 1100 * 8 {
                return Err(Error::Malformed(format!(
                    "composite of {w}×{h} pixels with {bytes} bytes of data"
                )));
            }
            let (colour, alpha): (&[u16], &[u16]) = if s.kind == 1 {
                (&[5], &[6])
            } else {
                (&[8], &[9])
            };
            let px = bitmap(
                data,
                channels,
                colour,
                alpha,
                s.bit_depth,
                psp.info.greyscale && s.bit_depth <= 8,
                palette.as_deref().or(psp.palette.as_deref()),
                s.compression,
                w,
                h,
                &mut truncated,
            )?;
            let c = Canvas {
                w,
                h,
                px,
                _budget: specular_core::Reservation::none(),
            };
            finish(&c, s.bit_depth == 48, psp.info.greyscale, truncated)
        }
        StoredBody::Missing => Err(Error::Truncated("composite image".into())),
    }
}

pub(crate) fn render(psp: &Psp, data: &[u8], opts: &Options) -> Result<Image, Error> {
    let i = &psp.info;
    let full = |s: &&Stored| {
        s.kind == 0
            && s.width == i.width
            && s.height == i.height
            && !matches!(s.body, StoredBody::Missing)
    };
    let drawable = psp
        .recs
        .iter()
        .any(|r| r.public.has_pixels || r.vector.as_ref().is_some_and(|v| !v.list.is_empty()));
    if opts.prefer_composite {
        // The lossless composite first; a JPEG one only when the layers cannot show the
        // image (they hold vector, adjustment or art media content, or nothing drawable).
        let lossless = psp
            .stored
            .iter()
            .filter(full)
            .find(|s| matches!(s.body, StoredBody::Channels { .. }));
        let jpeg = psp
            .stored
            .iter()
            .filter(full)
            .find(|s| matches!(s.body, StoredBody::Jpeg(_)));
        if let Some(s) = lossless {
            return decode_stored(psp, data, s);
        }
        if let Some(s) = jpeg
            && (i.not_drawn.is_some() || !drawable)
        {
            return decode_stored(psp, data, s);
        }
    }
    if !drawable {
        return Err(Error::Unsupported(match &i.not_drawn {
            Some(r) => format!("PaintShop Pro file without a composite image: {r}"),
            None => "PaintShop Pro file without raster layers".into(),
        }));
    }
    let mut canvas = Canvas::new(i.width, i.height)?;
    let mut truncated = false;
    composite_nodes(psp, data, &psp.tree, &mut canvas, opts, &mut truncated)?;
    finish(&canvas, i.bit_depth == 48, i.greyscale, truncated)
}

pub(crate) fn render_layer(
    psp: &Psp,
    data: &[u8],
    layer: usize,
    opts: &Options,
) -> Result<Image, Error> {
    let rec = psp
        .order
        .get(layer)
        .map(|&r| &psp.recs[r])
        .ok_or_else(|| Error::Malformed("no such layer".into()))?;
    let i = &psp.info;
    let mut canvas = Canvas::new(i.width, i.height)?;
    let mut truncated = false;
    if rec.public.kind == LayerKind::Vector && rec.vector.is_some() {
        let px = vector_layer(rec, canvas.w, canvas.h)?;
        let (w, h) = (canvas.w, canvas.h);
        let opacity = f32::from(rec.public.opacity) / 255.0;
        composite(
            &mut canvas,
            &px,
            (0, 0, w, h),
            opacity,
            BlendMode::Normal,
            opts,
        )?;
    } else if rec.public.has_pixels {
        let p = decode_layer(psp, data, rec, &mut truncated)?;
        let opacity = f32::from(rec.public.opacity) / 255.0;
        composite(
            &mut canvas,
            &p.rgba,
            (p.x, p.y, p.w, p.h),
            opacity,
            BlendMode::Normal,
            opts,
        )?;
    }
    let mut img = finish(&canvas, i.bit_depth == 48, false, truncated)?;
    if img.format == PixelFormat::Gray8 {
        img.format = PixelFormat::Rgba8;
    }
    Ok(img)
}

pub(crate) fn thumbnail(psp: &Psp, data: &[u8]) -> Option<Result<Image, Error>> {
    let s = psp
        .stored
        .iter()
        .filter(|s| s.kind == 1 && !matches!(s.body, StoredBody::Missing))
        .max_by_key(|s| u64::from(s.width) * u64::from(s.height))?;
    Some(decode_stored(psp, data, s))
}
