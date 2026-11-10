//! Drawing a laid-out page on the CPU: glyph outlines from skrifa filled by tiny-skia,
//! pictures decoded at the size they are drawn.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{GlyphId, MetadataProvider};
use tiny_skia::{
    FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

use crate::layout::{GlyphRun, Item, Pages};
use crate::{Error, Image, MAX_BINARY_PART};

/// Largest page raster (pixels).
const MAX_PIXELS: u64 = 400 << 20;
/// Glyph outlines cached per book.
const MAX_OUTLINES: usize = 100_000;
/// Rasterised glyphs cached per book.
const MAX_MASKS: usize = 30_000;
/// Decoded pictures kept per book.
const MAX_PICTURES: usize = 6;

type OutlineKey = (u32, u32, u64);
/// Resource, width, height.
type PictureKey = (u32, u32, u32);

#[derive(Default)]
pub(crate) struct Cache {
    outlines: Mutex<HashMap<OutlineKey, Option<Arc<tiny_skia::Path>>>>,
    masks: Mutex<HashMap<MaskKey, Option<Arc<GlyphMask>>>>,
    /// (resource, width, height) → premultiplied picture, most recent last.
    pictures: Mutex<Vec<(PictureKey, Arc<Pixmap>)>>,
}

struct Pen(PathBuilder);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

fn hash_coords(c: &[i16]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    c.hash(&mut h);
    h.finish()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Pages {
    fn outline(&self, run: &GlyphRun, gid: u32) -> Option<Arc<tiny_skia::Path>> {
        let key = (run.font, gid, hash_coords(&run.coords));
        if let Some(p) = lock(&self.cache.outlines).get(&key) {
            return p.clone();
        }
        let entry = self.fonts.get(run.font as usize)?;
        let made = skrifa::FontRef::from_index(entry.data.data.data(), entry.data.index)
            .ok()
            .and_then(|font| {
                let coords: Vec<NormalizedCoord> = run
                    .coords
                    .iter()
                    .map(|&c| NormalizedCoord::from_bits(c))
                    .collect();
                let glyphs = font.outline_glyphs();
                let g = glyphs.get(GlyphId::new(gid))?;
                let mut pen = Pen(PathBuilder::new());
                g.draw(
                    DrawSettings::unhinted(Size::unscaled(), LocationRef::new(&coords)),
                    &mut pen,
                )
                .ok()?;
                pen.0.finish().map(Arc::new)
            });
        let mut c = lock(&self.cache.outlines);
        if c.len() >= MAX_OUTLINES {
            c.clear();
        }
        c.insert(key, made.clone());
        made
    }

    /// Picture `res` decoded for drawing at `w` × `h` pixels.
    fn picture(&self, res: u32, w: u32, h: u32) -> Option<Arc<Pixmap>> {
        // Sizes bucketed so a slightly different scale reuses the decode.
        let bw = w.next_power_of_two().max(16);
        let bh = h.next_power_of_two().max(16);
        let key = (res, bw, bh);
        {
            let mut c = lock(&self.cache.pictures);
            if let Some(i) = c.iter().position(|e| e.0 == key) {
                let e = c.remove(i);
                let p = e.1.clone();
                c.push(e);
                return Some(p);
            }
        }
        let r = self.images.get(res as usize)?;
        let read;
        let bytes: &[u8] = match &r.inline {
            Some(b) => b.as_slice(),
            None => {
                read = self.inner.read(&r.path, MAX_BINARY_PART)?;
                &read
            }
        };
        let img = self.decoder.decode(bytes, &r.media, bw, bh)?;
        let pm = Arc::new(to_pixmap(img)?);
        let mut c = lock(&self.cache.pictures);
        if c.len() >= MAX_PICTURES {
            c.remove(0);
        }
        c.push((key, pm.clone()));
        Some(pm)
    }

    /// Page `i` drawn at `scale` (pixels per CSS px), or the `region` (x, y, w, h in those
    /// pixels) of it: opaque RGBA8.
    pub fn render(
        &self,
        i: usize,
        scale: f32,
        region: Option<(u32, u32, u32, u32)>,
    ) -> Result<Image, Error> {
        let page = self.pages.get(i).ok_or(Error::NoSuchPage(i))?;
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let full_w = (page.size.0 * scale).ceil().max(1.0) as u32;
        let full_h = (page.size.1 * scale).ceil().max(1.0) as u32;
        let (rx, ry, rw, rh) = match region {
            Some((x, y, w, h)) => {
                let x = x.min(full_w.saturating_sub(1));
                let y = y.min(full_h.saturating_sub(1));
                (x, y, w.min(full_w - x).max(1), h.min(full_h - y).max(1))
            }
            None => (0, 0, full_w, full_h),
        };
        if u64::from(rw) * u64::from(rh) > MAX_PIXELS {
            return Err(Error::TooLarge(format!("a page of {rw}×{rh} pixels")));
        }
        let mut pm = Pixmap::new(rw, rh)
            .ok_or_else(|| Error::TooLarge(format!("a page of {rw}×{rh} pixels")))?;
        pm.fill(tiny_skia::Color::WHITE);
        let base = Transform::from_translate(-(rx as f32), -(ry as f32)).pre_scale(scale, scale);
        // The visible band in page px, for culling.
        let (vx0, vy0) = (rx as f32 / scale, ry as f32 / scale);
        let (vx1, vy1) = ((rx + rw) as f32 / scale, (ry + rh) as f32 / scale);
        for it in &page.items {
            match it {
                Item::Rect { x, y, w, h, color } => {
                    if x + w < vx0 || *x > vx1 || y + h < vy0 || *y > vy1 {
                        continue;
                    }
                    if let Some(r) = Rect::from_xywh(*x, *y, w.max(0.01), h.max(0.01)) {
                        pm.fill_rect(r, &paint(*color), base, None);
                    }
                }
                Item::Image { res, x, y, w, h } => {
                    if x + w < vx0 || *x > vx1 || y + h < vy0 || *y > vy1 {
                        continue;
                    }
                    let tw = (w * scale).ceil().max(1.0) as u32;
                    let th = (h * scale).ceil().max(1.0) as u32;
                    if let Some(src) = self.picture(*res, tw, th) {
                        let t = base
                            .pre_translate(*x, *y)
                            .pre_scale(w / src.width() as f32, h / src.height() as f32);
                        let pp = PixmapPaint {
                            quality: FilterQuality::Bilinear,
                            ..Default::default()
                        };
                        pm.draw_pixmap(0, 0, src.as_ref().as_ref(), &pp, t, None);
                    } else if let Some(r) = Rect::from_xywh(*x, *y, w.max(1.0), h.max(1.0)) {
                        pm.fill_rect(r, &paint([230, 230, 230, 255]), base, None);
                    }
                }
                Item::Glyphs(run) => self.draw_run(&mut pm, run, scale, (rx, ry)),
            }
        }
        Ok(Image {
            width: rw,
            height: rh,
            rgba: pm.take(),
        })
    }

    /// The coverage of glyph `gid` of `run` at `px` pixels per em, shifted right by
    /// `sub`/4 px, rasterised once.
    fn glyph_mask(&self, run: &GlyphRun, gid: u32, px: f32, sub: u8) -> Option<Arc<GlyphMask>> {
        let key = MaskKey {
            font: run.font,
            gid,
            coords: hash_coords(&run.coords),
            size: (px * 16.0).round() as u32,
            sub,
            skew: run.skew.to_bits(),
            bold: run.bold,
        };
        if let Some(m) = lock(&self.cache.masks).get(&key) {
            return m.clone();
        }
        let made = self.make_mask(run, gid, px, sub).map(Arc::new);
        let mut c = lock(&self.cache.masks);
        if c.len() >= MAX_MASKS {
            c.clear();
        }
        c.insert(key, made.clone());
        made
    }

    fn make_mask(&self, run: &GlyphRun, gid: u32, px: f32, sub: u8) -> Option<GlyphMask> {
        let entry = self.fonts.get(run.font as usize)?;
        let path = self.outline(run, gid)?;
        let k = px / entry.upem;
        let skew = if run.skew != 0.0 {
            run.skew.to_radians().tan()
        } else {
            0.0
        };
        let glyph = Transform::from_scale(k, -k)
            .pre_concat(Transform::from_row(1.0, 0.0, skew, 1.0, 0.0, 0.0));
        let stroke = run.bold.then(|| (px / 40.0).max(0.3));
        let pad = 2.0 + stroke.unwrap_or(0.0);
        let b = (*path).clone().transform(glyph)?.bounds();
        let left = (b.left() - pad).floor();
        let top = (b.top() - pad).floor();
        let w = ((b.right() + pad + 1.0).ceil() - left) as u32;
        let h = ((b.bottom() + pad).ceil() - top) as u32;
        if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16 << 20 {
            return None;
        }
        let mut pm = Pixmap::new(w, h)?;
        let t = Transform::from_translate(-left + f32::from(sub) / 4.0, -top).pre_concat(glyph);
        let p = paint([0, 0, 0, 255]);
        pm.fill_path(&path, &p, FillRule::Winding, t, None);
        if let Some(sw) = stroke {
            let s = Stroke {
                width: sw / k,
                ..Default::default()
            };
            pm.stroke_path(&path, &p, &s, t, None);
        }
        let data = pm.data().chunks_exact(4).map(|c| c[3]).collect();
        Some(GlyphMask {
            left: left as i32,
            top: top as i32,
            w,
            h,
            data,
        })
    }

    fn draw_run(&self, pm: &mut Pixmap, run: &GlyphRun, scale: f32, origin: (u32, u32)) {
        let px = run.size * scale;
        if px.is_nan() || px <= 0.0 || px > 4000.0 {
            return;
        }
        let (pw, ph) = (pm.width() as i32, pm.height() as i32);
        let reach = (px * 2.0) as i32 + 4;
        let c = run.color;
        let alpha = u32::from(c[3]);
        if alpha == 0 {
            return;
        }
        let data = pm.data_mut();
        for &(gid, x, y) in &run.glyphs {
            let xd = x * scale - origin.0 as f32;
            let yd = y * scale - origin.1 as f32;
            let ix = xd.floor();
            let mut sub = ((xd - ix) * 4.0).round() as i32;
            let mut ix = ix as i32;
            if sub == 4 {
                sub = 0;
                ix += 1;
            }
            let iy = yd.round() as i32;
            if ix + reach < 0 || ix - reach > pw || iy + reach < 0 || iy - reach > ph {
                continue;
            }
            let Some(m) = self.glyph_mask(run, gid, px, sub as u8) else {
                continue;
            };
            let x0 = ix + m.left;
            let y0 = iy + m.top;
            for row in 0..m.h as i32 {
                let dy = y0 + row;
                if dy < 0 || dy >= ph {
                    continue;
                }
                let src = &m.data[(row as usize) * m.w as usize..(row as usize + 1) * m.w as usize];
                for (col, &cov) in src.iter().enumerate() {
                    if cov == 0 {
                        continue;
                    }
                    let dx = x0 + col as i32;
                    if dx < 0 || dx >= pw {
                        continue;
                    }
                    let a = u32::from(cov) * alpha / 255;
                    let o = (dy as usize * pw as usize + dx as usize) * 4;
                    let d = &mut data[o..o + 4];
                    for ch in 0..3 {
                        d[ch] = ((u32::from(c[ch]) * a + u32::from(d[ch]) * (255 - a) + 127) / 255)
                            as u8;
                    }
                    d[3] = (a + u32::from(d[3]) * (255 - a) / 255).min(255) as u8;
                }
            }
        }
    }
}

/// A rasterised glyph: coverage, and its top-left relative to the pen position.
struct GlyphMask {
    left: i32,
    top: i32,
    w: u32,
    h: u32,
    data: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MaskKey {
    font: u32,
    gid: u32,
    coords: u64,
    /// Pixels per em × 16.
    size: u32,
    sub: u8,
    skew: u32,
    bold: bool,
}

fn paint(c: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], c[3]);
    p.anti_alias = true;
    p
}

fn to_pixmap(img: Image) -> Option<Pixmap> {
    let n = (img.width as usize).checked_mul(img.height as usize)?;
    if img.rgba.len() < n * 4 || n == 0 {
        return None;
    }
    let mut data = img.rgba;
    data.truncate(n * 4);
    for px in data.chunks_exact_mut(4) {
        let a = u16::from(px[3]);
        if a < 255 {
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(img.width, img.height)?)
}
