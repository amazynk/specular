//! A gallery thumbnail of a face: "Aa" (or its first glyphs) in black on white, drawn from
//! the face's own outlines.

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

use crate::face::Face;
use crate::outline::{Outline, PathEl};

/// The glyphs shown: "A" and "a" when the face has them, else its first printable glyphs
/// with outlines (or, in a bitmap-only face, with colour bitmaps).
fn sample(face: &Face) -> Vec<(u16, Outline)> {
    let o = Shape {
        face,
        outliner: face.outliner(),
    };
    let pick: Vec<(u16, Outline)> = ['A', 'a']
        .iter()
        .filter_map(|&c| {
            let g = face.glyph_index(c)?;
            Some((g, o.outline(g)?))
        })
        .collect();
    if pick.len() == 2 {
        return pick;
    }
    let mut out = Vec::new();
    for &(c, g) in face.chars() {
        let printable = char::from_u32(c).is_some_and(|c| !c.is_control() && !c.is_whitespace());
        if printable && let Some(ol) = o.outline(g) {
            out.push((g, ol));
            if out.len() == 2 {
                break;
            }
        }
    }
    if out.is_empty() {
        // A face without a character map: its first glyphs with outlines.
        for g in 1..face.glyph_count.min(64) {
            if let Some(ol) = o.outline(g) {
                out.push((g, ol));
                if out.len() == 2 {
                    break;
                }
            }
        }
    }
    out
}

/// A glyph's outline, or for a glyph drawn only as a colour bitmap (`sbix`, `CBDT`), an
/// empty path with the bitmap's box in font units (so it is placed and scaled like ink).
struct Shape<'a> {
    face: &'a Face,
    outliner: crate::face::Outliner<'a>,
}

impl Shape<'_> {
    fn outline(&self, g: u16) -> Option<Outline> {
        if let Some(ol) = self.outliner.outline(g) {
            return Some(ol);
        }
        let b = sbix_png(self.face, g).or_else(|| cbdt_png(self.face, g))?;
        let (w, h) = png_size(&b.0)?;
        let k = f32::from(self.face.metrics.units_per_em) / f32::from(b.3.max(1));
        let (x0, y0) = (f32::from(b.1) * k, f32::from(b.2) * k);
        Some(Outline {
            path: Vec::new(),
            bbox: [x0, y0, x0 + w as f32 * k, y0 + h as f32 * k],
        })
    }
}

/// Width and height from a PNG's IHDR.
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    (png.get(12..16)? == b"IHDR").then_some(())?;
    Some((be32(png, 16)?, be32(png, 20)?))
}

fn add_path(pb: &mut PathBuilder, ol: &Outline, x0: f32, s: f32, base: f32) {
    let p = |x: f32, y: f32| (x0 + x * s, base - y * s);
    for el in &ol.path {
        match *el {
            PathEl::MoveTo(x, y) => {
                let (x, y) = p(x, y);
                pb.move_to(x, y);
            }
            PathEl::LineTo(x, y) => {
                let (x, y) = p(x, y);
                pb.line_to(x, y);
            }
            PathEl::QuadTo(a, b, x, y) => {
                let (a, b) = p(a, b);
                let (x, y) = p(x, y);
                pb.quad_to(a, b, x, y);
            }
            PathEl::CubicTo(a, b, c, d, x, y) => {
                let (a, b) = p(a, b);
                let (c, d) = p(c, d);
                let (x, y) = p(x, y);
                pb.cubic_to(a, b, c, d, x, y);
            }
            PathEl::Close => pb.close(),
        }
    }
}

/// An opaque RGBA8 thumbnail of `face`, `width` × `height` pixels.
pub fn preview(face: &Face, width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width.max(1), height.max(1));
    let Some(mut pm) = Pixmap::new(w, h) else {
        return vec![255; w as usize * h as usize * 4];
    };
    pm.fill(tiny_skia::Color::WHITE);
    let glyphs = sample(face);
    let mut paint = Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    if glyphs.is_empty() {
        // Bitmap-only or empty: a glyph box.
        let side = w.min(h) as f32 * 0.5;
        let r = Rect::from_xywh((w as f32 - side) / 2.0, (h as f32 - side) / 2.0, side, side);
        if let Some(r) = r {
            let path = PathBuilder::from_rect(r);
            let stroke = tiny_skia::Stroke {
                width: (side / 24.0).max(1.0),
                ..Default::default()
            };
            pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        return pm.take();
    }
    // Ink extents of the run, glyphs side by side at their advances.
    let gap = face.metrics.units_per_em as f32 * 0.04;
    let mut x = 0.0f32;
    let mut placed = Vec::new();
    let (mut y0, mut y1) = (f32::MAX, f32::MIN);
    for (g, ol) in &glyphs {
        let adv = face
            .advance(*g)
            .map(|a| a as f32)
            .filter(|&a| a > 0.0)
            .unwrap_or(ol.bbox[2].max(1.0));
        placed.push((x, ol));
        y0 = y0.min(ol.bbox[1]);
        y1 = y1.max(ol.bbox[3]);
        x += adv + gap;
    }
    let ink_x0 = placed
        .iter()
        .map(|(px, ol)| px + ol.bbox[0])
        .fold(f32::MAX, f32::min);
    let ink_x1 = placed
        .iter()
        .map(|(px, ol)| px + ol.bbox[2])
        .fold(f32::MIN, f32::max);
    let iw = (ink_x1 - ink_x0).max(1.0);
    let ih = (y1 - y0).max(1.0);
    let s = (w as f32 * 0.8 / iw).min(h as f32 * 0.7 / ih);
    let left = (w as f32 - iw * s) / 2.0 - ink_x0 * s;
    let base = (h as f32 + ih * s) / 2.0 + y0 * s;
    // Colour glyphs as the specimen shows them: COLR version 1 paint graphs and version 0
    // layers in the first palette, sbix bitmaps; the outline in black otherwise (the
    // thumbnails were black; the visual review; version 1 in fidelity round 6).
    let colr = colr_v0(face);
    let palette = face.colour.palettes.first();
    let v1 = crate::colr::ColrV1::of(face);
    let v1_ctx = v1.as_ref().map(|c| crate::colr::Ctx {
        face,
        colr: c,
        palette: palette.map(Vec::as_slice),
        foreground: [0, 0, 0, 255],
    });
    let o = face.outliner();
    let mut plain = PathBuilder::new();
    for ((px, ol), (g, _)) in placed.iter().zip(&glyphs) {
        let x0 = left + px * s;
        if let Some(png) = sbix_png(face, *g).or_else(|| cbdt_png(face, *g))
            && draw_bitmap(&mut pm, &png, face, x0, base, s)
        {
            continue;
        }
        if let Some(ctx) = &v1_ctx
            && crate::colr::draw(
                ctx,
                *g,
                &mut pm,
                Transform::from_row(s, 0.0, 0.0, -s, x0, base),
            )
        {
            continue;
        }
        if let Some(layers) = colr.as_ref().and_then(|c| c.layers(*g)) {
            for (lg, pi) in layers {
                let Some(lol) = o.outline(lg) else { continue };
                let mut pb = PathBuilder::new();
                add_path(&mut pb, &lol, x0, s, base);
                let c = palette
                    .and_then(|p| p.get(usize::from(pi)))
                    .copied()
                    .unwrap_or([0, 0, 0, 255]);
                let mut lp = paint.clone();
                lp.set_color_rgba8(c[0], c[1], c[2], c[3]);
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &lp, FillRule::Winding, Transform::identity(), None);
                }
            }
            continue;
        }
        add_path(&mut plain, ol, x0, s, base);
    }
    if let Some(path) = plain.finish() {
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    pm.take()
}

/// `COLR` version 0 base glyph and layer records.
struct ColrV0<'a> {
    base: &'a [u8],
    layers: &'a [u8],
}

impl ColrV0<'_> {
    /// The layers (glyph, palette index; 0xFFFF the text colour) of base glyph `g`.
    fn layers(&self, g: u16) -> Option<Vec<(u16, u16)>> {
        let rec = self.base.chunks_exact(6).find(|r| be16(r, 0) == Some(g))?;
        let (first, n) = (usize::from(be16(rec, 2)?), usize::from(be16(rec, 4)?));
        let out: Vec<(u16, u16)> = (first..first + n)
            .filter_map(|i| {
                let r = self.layers.get(i * 4..i * 4 + 4)?;
                Some((be16(r, 0)?, be16(r, 2)?))
            })
            .collect();
        (!out.is_empty()).then_some(out)
    }
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn colr_v0(face: &Face) -> Option<ColrV0<'_>> {
    let t = face.table(b"COLR")?;
    let nbase = usize::from(be16(t, 2)?);
    let base_at = be32(t, 4)? as usize;
    let layer_at = be32(t, 8)? as usize;
    let nlayers = usize::from(be16(t, 12)?);
    Some(ColrV0 {
        base: t.get(base_at..base_at.checked_add(nbase * 6)?)?,
        layers: t.get(layer_at..layer_at.checked_add(nlayers * 4)?)?,
    })
}

/// Glyph `g`'s PNG in the largest `sbix` strike, with its origin offset.
fn sbix_png(face: &Face, g: u16) -> Option<(Vec<u8>, i16, i16, u16)> {
    let t = face.table(b"sbix")?;
    let n = be32(t, 4)? as usize;
    let mut best: Option<(u16, usize)> = None;
    for i in 0..n.min(64) {
        let off = be32(t, 8 + 4 * i)? as usize;
        let ppem = be16(t, off)?;
        if best.is_none_or(|(p, _)| ppem > p) {
            best = Some((ppem, off));
        }
    }
    let (ppem, strike) = best?;
    let gi = usize::from(g);
    let a = be32(t, strike + 4 + 4 * gi)? as usize;
    let b = be32(t, strike + 4 + 4 * (gi + 1))? as usize;
    let rec = t.get(strike + a..strike + b)?;
    if rec.len() < 8 || &rec[4..8] != b"png " {
        return None;
    }
    let ox = be16(rec, 0)? as i16;
    let oy = be16(rec, 2)? as i16;
    Some((rec[8..].to_vec(), ox, oy, ppem))
}

/// Glyph `g`'s PNG in the largest `CBDT` strike holding it (image formats 17, 18 and 19;
/// index formats 1 to 5), as (PNG, left, bottom above the baseline, pixels per em) like
/// [`sbix_png`]'s.
fn cbdt_png(face: &Face, g: u16) -> Option<(Vec<u8>, i16, i16, u16)> {
    let loc = face.table(b"CBLC")?;
    let dat = face.table(b"CBDT")?;
    let n = (be32(loc, 4)? as usize).min(64);
    let mut best: Option<(u8, usize)> = None;
    for i in 0..n {
        let rec = 8 + 48 * i;
        let (first, last) = (be16(loc, rec + 40)?, be16(loc, rec + 42)?);
        let ppem = *loc.get(rec + 44)?;
        if (first..=last).contains(&g) && best.is_none_or(|(p, _)| ppem > p) {
            best = Some((ppem, rec));
        }
    }
    let (ppem, rec) = best?;
    let list = be32(loc, rec)? as usize;
    let subs = (be32(loc, rec + 8)? as usize).min(4096);
    for k in 0..subs {
        let r = list + 8 * k;
        let (first, last) = (be16(loc, r)?, be16(loc, r + 2)?);
        if !(first..=last).contains(&g) {
            continue;
        }
        let st = list + be32(loc, r + 4)? as usize;
        let (index_format, image_format) = (be16(loc, st)?, be16(loc, st + 2)?);
        let base = be32(loc, st + 4)? as usize;
        let i = usize::from(g - first);
        // (offset, length) of the glyph's data in CBDT, and where big metrics sit in the
        // index (formats 2 and 5, for image format 19).
        let (at, len, metrics) = match index_format {
            1 => {
                let (a, b) = (be32(loc, st + 8 + 4 * i)?, be32(loc, st + 12 + 4 * i)?);
                (base + a as usize, b.checked_sub(a)? as usize, None)
            }
            3 => {
                let (a, b) = (be16(loc, st + 8 + 2 * i)?, be16(loc, st + 10 + 2 * i)?);
                (base + usize::from(a), usize::from(b.checked_sub(a)?), None)
            }
            2 => {
                let size = be32(loc, st + 8)? as usize;
                (base + i * size, size, Some(st + 12))
            }
            4 => {
                let count = (be32(loc, st + 8)? as usize).min(65536);
                let j = (0..count).find(|&j| be16(loc, st + 12 + 4 * j) == Some(g))?;
                let a = be16(loc, st + 14 + 4 * j)?;
                let b = be16(loc, st + 18 + 4 * j)?;
                (base + usize::from(a), usize::from(b.checked_sub(a)?), None)
            }
            5 => {
                let size = be32(loc, st + 8)? as usize;
                let count = (be32(loc, st + 20)? as usize).min(65536);
                let j = (0..count).find(|&j| be16(loc, st + 24 + 2 * j) == Some(g))?;
                (base + j * size, size, Some(st + 12))
            }
            _ => return None,
        };
        let d = dat.get(at..at.checked_add(len)?)?;
        // Height and bearings in pixels (the top above the baseline), and where the PNG's
        // length sits.
        let (h, bx, by, png_at) = match image_format {
            17 => (*d.first()?, *d.get(2)? as i8, *d.get(3)? as i8, 5),
            18 => (*d.first()?, *d.get(2)? as i8, *d.get(3)? as i8, 8),
            19 => {
                let m = metrics?;
                let m = loc.get(m..m + 8)?;
                (m[0], m[2] as i8, m[3] as i8, 0)
            }
            _ => return None,
        };
        let plen = be32(d, png_at)? as usize;
        let png = d.get(png_at + 4..(png_at + 4).checked_add(plen)?)?;
        let bottom = i16::from(by) - i16::from(h);
        return Some((png.to_vec(), i16::from(bx), bottom, u16::from(ppem)));
    }
    None
}

/// Draws an sbix bitmap at the pen position (`x0`, baseline `base`), `s` pixels per font unit.
fn draw_bitmap(
    pm: &mut Pixmap,
    (png, ox, oy, ppem): &(Vec<u8>, i16, i16, u16),
    face: &Face,
    x0: f32,
    base: f32,
    s: f32,
) -> bool {
    let Ok(img) = specular_png::decode(png, &Default::default()) else {
        return false;
    };
    let (iw, ih) = (img.width, img.height);
    if iw == 0 || ih == 0 {
        return false;
    }
    let rgba = img.to_rgba8();
    // Bitmap pixels per font unit, then to output pixels.
    let k = f32::from(face.metrics.units_per_em) / f32::from((*ppem).max(1)) * s;
    let (w, h) = (pm.width() as i64, pm.height() as i64);
    let left = x0 + f32::from(*ox) * k;
    let top = base - (f32::from(*oy) + ih as f32) * k;
    let data = pm.data_mut();
    for y in 0..(ih as f32 * k).ceil() as i64 {
        let py = top as i64 + y;
        if py < 0 || py >= h {
            continue;
        }
        let sy = ((y as f32 / k) as usize).min(ih as usize - 1);
        for x in 0..(iw as f32 * k).ceil() as i64 {
            let px = left as i64 + x;
            if px < 0 || px >= w {
                continue;
            }
            let sx = ((x as f32 / k) as usize).min(iw as usize - 1);
            let src = &rgba[(sy * iw as usize + sx) * 4..][..4];
            let a = u32::from(src[3]);
            let d = &mut data[((py * w + px) * 4) as usize..][..4];
            // Premultiplied destination (tiny-skia's), straight source.
            for c in 0..3 {
                d[c] = ((u32::from(src[c]) * a + u32::from(d[c]) * (255 - a)) / 255) as u8;
            }
            d[3] = (a + u32::from(d[3]) * (255 - a) / 255) as u8;
        }
    }
    true
}
