//! Drawing a parsed page with vello_cpu.

use std::sync::Arc;

use vello_cpu::color::ColorSpaceTag;
use vello_cpu::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::kurbo::{self, Affine, BezPath, Point, Rect, Shape, Stroke};
use vello_cpu::peniko::{self, ColorStop, Extend, Fill, Gradient, ImageQuality, ImageSampler};
use vello_cpu::{Image, ImageSource, Mask, PaintType, Pixmap, RenderContext, Resources};

use crate::Inner;
use crate::model::*;
use crate::page::MAX_DEPTH;

/// Largest side of a visual brush tile, pixels.
const MAX_TILE_SIDE: f64 = 2048.0;
/// Opacity masks and visual tiles rendered per page at most (each is a full or tile-sized
/// render).
const MAX_OFFSCREEN: usize = 256;
/// Most elements drawn per page, offscreen renders included: visual brushes drawing a
/// visual that fills with the next brush twice fan out exponentially within the depth cap.
const MAX_DRAWS: usize = 250_000;

pub struct Renderer<'a> {
    doc: &'a Inner,
    ctx: &'a mut RenderContext,
    w: u16,
    h: u16,
    cancelled: &'a dyn Fn() -> bool,
    count: usize,
    pub stop: bool,
    offscreen: &'a mut usize,
}

fn color(c: Rgba, alpha: f32) -> AlphaColor<Srgb> {
    AlphaColor::new([c[0], c[1], c[2], (c[3] * alpha).clamp(0.0, 1.0)])
}

fn extend(s: Spread) -> Extend {
    match s {
        Spread::Pad => Extend::Pad,
        Spread::Reflect => Extend::Reflect,
        Spread::Repeat => Extend::Repeat,
    }
}

fn kurbo_cap(c: Cap) -> kurbo::Cap {
    match c {
        Cap::Flat => kurbo::Cap::Butt,
        Cap::Square => kurbo::Cap::Square,
        // kurbo has no triangle cap; round is closest in extent.
        Cap::Round | Cap::Triangle => kurbo::Cap::Round,
    }
}

/// The scale an affine applies along x and y.
fn scales(t: Affine) -> (f64, f64) {
    let c = t.as_coeffs();
    (c[0].hypot(c[1]), c[2].hypot(c[3]))
}

fn rect_path(r: [f64; 4]) -> BezPath {
    Rect::new(r[0], r[1], r[0] + r[2], r[1] + r[3]).to_path(0.1)
}

/// Maps the viewbox `vb` onto the viewport `vp`.
fn box_map(vb: [f64; 4], vp: [f64; 4]) -> Option<Affine> {
    if vb[2] == 0.0 || vb[3] == 0.0 || vp[2] == 0.0 || vp[3] == 0.0 {
        return None;
    }
    Some(
        Affine::translate((vp[0], vp[1]))
            * Affine::scale_non_uniform(vp[2] / vb[2], vp[3] / vb[3])
            * Affine::translate((-vb[0], -vb[1])),
    )
}

fn tile_extend(m: TileMode) -> (Extend, Extend) {
    match m {
        TileMode::None => (Extend::Pad, Extend::Pad),
        TileMode::Tile => (Extend::Repeat, Extend::Repeat),
        TileMode::FlipX => (Extend::Reflect, Extend::Repeat),
        TileMode::FlipY => (Extend::Repeat, Extend::Reflect),
        TileMode::FlipXY => (Extend::Reflect, Extend::Reflect),
    }
}

/// What a brush became: the paint is set; `clip` (element space) limits it when the brush
/// does not tile.
struct Prepared {
    clip: Option<BezPath>,
}

impl<'a> Renderer<'a> {
    pub fn new(
        doc: &'a Inner,
        ctx: &'a mut RenderContext,
        cancelled: &'a dyn Fn() -> bool,
        offscreen: &'a mut usize,
    ) -> Renderer<'a> {
        let (w, h) = (ctx.width(), ctx.height());
        Renderer {
            doc,
            ctx,
            w,
            h,
            cancelled,
            count: 0,
            stop: false,
            offscreen,
        }
    }

    /// Counts one element drawn; false once cancelled or past [`MAX_DRAWS`] (what is drawn
    /// so far is shown).
    fn tick(&mut self) -> bool {
        self.count += 1;
        if self.count >= MAX_DRAWS || (self.count.is_multiple_of(64) && (self.cancelled)()) {
            self.stop = true;
        }
        !self.stop
    }

    pub fn draw_nodes(&mut self, nodes: &[Node], t: Affine, depth: u32) {
        for n in nodes {
            if self.stop {
                return;
            }
            self.draw_node(n, t, depth);
        }
    }

    /// The device rectangle in an element's space (for masks covering everything).
    fn device_rect_in(&self, t: Affine) -> Option<BezPath> {
        let inv = t.inverse();
        if !inv.as_coeffs().iter().all(|v| v.is_finite()) {
            return None;
        }
        let r = Rect::new(0.0, 0.0, f64::from(self.w), f64::from(self.h));
        let mut p = r.to_path(0.1);
        p.apply_affine(inv);
        Some(p)
    }

    fn mask(&mut self, brush: &Brush, t: Affine, depth: u32) -> Option<Mask> {
        if *self.offscreen >= MAX_OFFSCREEN {
            return None;
        }
        *self.offscreen += 1;
        let area = self.device_rect_in(t)?;
        let mut ctx = RenderContext::new(self.w, self.h);
        let mut r = Renderer::new(self.doc, &mut ctx, self.cancelled, self.offscreen);
        r.count = self.count;
        r.fill(&area, Fill::NonZero, brush, t, 1.0, depth + 1);
        self.count = r.count;
        let stop = r.stop;
        ctx.flush();
        let mut pix = Pixmap::new(self.w, self.h);
        ctx.render(&mut pix, &mut Resources::default());
        if stop {
            self.stop = true;
        }
        Some(Mask::new_alpha(&pix))
    }

    fn draw_node(&mut self, node: &Node, parent: Affine, depth: u32) {
        if depth > MAX_DEPTH || !self.tick() {
            return;
        }
        let c = node.common();
        if c.opacity <= 0.0 {
            return;
        }
        let t = parent * c.transform;
        if !t.as_coeffs().iter().all(|v| v.is_finite()) {
            return;
        }
        let mut layers = 0;
        if let Some(clip) = &c.clip {
            self.ctx.set_transform(t);
            self.ctx.set_fill_rule(clip.rule);
            self.ctx.push_clip_layer(&clip.fill);
            layers += 1;
        }
        // Opacity folds into the paint when one paint is drawn and there is no mask.
        let single = match node {
            Node::Path(p) => p.fill.is_none() || p.stroke.is_none(),
            Node::Glyphs(g) => !g.bold,
            Node::Canvas(_) => false,
        };
        let mut alpha = 1.0;
        let mask = c.mask.as_ref().and_then(|m| self.mask(m, t, depth));
        if mask.is_some() || (c.opacity < 1.0 && !single) {
            self.ctx.set_transform(t);
            self.ctx.push_layer(None, None, Some(c.opacity), mask, None);
            layers += 1;
        } else {
            alpha = c.opacity;
        }
        match node {
            Node::Canvas(cv) => self.draw_nodes(&cv.children, t, depth + 1),
            Node::Path(p) => {
                if let Some(f) = &p.fill {
                    self.fill(&p.data.fill, p.data.rule, f, t, alpha, depth);
                }
                if let Some((b, s)) = &p.stroke {
                    self.stroke(p.data.stroke_path(), s, b, t, alpha, depth);
                }
            }
            Node::Glyphs(g) => self.glyphs(g, t, alpha, depth),
        }
        for _ in 0..layers {
            self.ctx.pop_layer();
        }
    }

    fn glyphs(&mut self, g: &GlyphsNode, t: Affine, alpha: f32, depth: u32) {
        let Some(fill) = &g.fill else { return };
        let k = g.em / f64::from(g.font.units_per_em);
        if !k.is_finite() {
            return;
        }
        let mut glyph_t = Affine::scale_non_uniform(k, -k);
        if g.italic {
            // A 20° slant (x moves right as y goes up; y is down here).
            glyph_t = Affine::new([1.0, 0.0, -(20f64.to_radians().tan()), 1.0, 0.0, 0.0]) * glyph_t;
        }
        // Sideways glyphs turn a quarter counter-clockwise, their ascender at the origin's left
        // edge of the cell and their horizontal centre on the baseline.
        let mut side_shift = kurbo::Vec2::ZERO;
        if g.sideways {
            glyph_t = Affine::rotate(-std::f64::consts::FRAC_PI_2) * glyph_t;
            side_shift = kurbo::Vec2::new(g.font.ascent() * g.em, 0.0);
        }
        let mut path = BezPath::new();
        for p in &g.glyphs {
            let o = g.font.outline(p.gid);
            if o.elements().is_empty() || !(p.at.x.is_finite() && p.at.y.is_finite()) {
                continue;
            }
            let mut at = p.at.to_vec2() + side_shift;
            if g.sideways {
                let half = g.font.face().map_or(0.5, |f| f.advance(p.gid)) * g.em / 2.0;
                at.y += half;
            }
            let m = Affine::translate(at) * glyph_t;
            path.extend(o.elements().iter().map(|el| m * *el));
        }
        if path.elements().is_empty() {
            return;
        }
        self.fill(&path, Fill::NonZero, fill, t, alpha, depth);
        if g.bold {
            let s = StrokeStyle {
                width: g.em * 0.02,
                dash: Vec::new(),
                dash_offset: 0.0,
                dash_cap: Cap::Flat,
                start_cap: Cap::Flat,
                end_cap: Cap::Flat,
                join: Join::Round,
                miter: 10.0,
            };
            self.stroke(&path, &s, fill, t, alpha, depth);
        }
    }

    /// Sets the paint for `brush` drawn in element space `t`; `None` when nothing draws.
    fn prepare(&mut self, brush: &Brush, t: Affine, alpha: f32, depth: u32) -> Option<Prepared> {
        self.ctx.set_transform(t);
        self.ctx.set_paint_transform(Affine::IDENTITY);
        match brush {
            Brush::Solid(c) => {
                if c[3] * alpha <= 0.0 {
                    return None;
                }
                self.ctx.set_paint(color(*c, alpha));
                Some(Prepared { clip: None })
            }
            Brush::Linear {
                start,
                end,
                stops,
                spread,
                linear_rgb,
                transform,
                opacity,
            } => {
                let a = alpha * opacity;
                let g = Gradient::new_linear(*start, *end);
                self.gradient(g, stops, *spread, *linear_rgb, *transform, a)
            }
            Brush::Radial {
                center,
                origin,
                rx,
                ry,
                stops,
                spread,
                linear_rgb,
                transform,
                opacity,
            } => {
                if *rx == 0.0 || *ry == 0.0 {
                    return None;
                }
                let a = alpha * opacity;
                let unit = *transform
                    * Affine::translate(center.to_vec2())
                    * Affine::scale_non_uniform(*rx, *ry);
                let o = Point::new((origin.x - center.x) / rx, (origin.y - center.y) / ry);
                let g = Gradient::new_two_point_radial(o, 0.0, Point::ZERO, 1.0);
                self.gradient(g, stops, *spread, *linear_rgb, unit, a)
            }
            Brush::Image {
                part,
                tiling,
                transform,
                opacity,
            } => {
                let img = self.doc.image(part)?;
                let (sx, sy) = (img.dpi.0 / 96.0, img.dpi.1 / 96.0);
                // Viewbox in image pixels.
                let vb = [
                    tiling.viewbox[0] * sx,
                    tiling.viewbox[1] * sy,
                    tiling.viewbox[2] * sx,
                    tiling.viewbox[3] * sy,
                ];
                let map = *transform * box_map(vb, tiling.viewport)?;
                let (dx, dy) = scales(t * map);
                let (pix, rel) = img.level(dx.min(dy));
                let mut pix = pix;
                let mut origin = (0.0, 0.0);
                let full = vb[0].abs() < 0.5
                    && vb[1].abs() < 0.5
                    && (vb[2] - f64::from(img.width)).abs() < 0.5
                    && (vb[3] - f64::from(img.height)).abs() < 0.5;
                if tiling.mode != TileMode::None && !full {
                    // Tiles repeat the viewbox only: crop it.
                    let x0 = (vb[0] * rel).round().max(0.0) as u32;
                    let y0 = (vb[1] * rel).round().max(0.0) as u32;
                    let x1 = ((vb[0] + vb[2]) * rel).round().min(f64::from(pix.width())) as u32;
                    let y1 = ((vb[1] + vb[3]) * rel).round().min(f64::from(pix.height())) as u32;
                    if x1 <= x0 || y1 <= y0 {
                        return None;
                    }
                    pix = Arc::new(crop(&pix, x0, y0, x1 - x0, y1 - y0));
                    origin = (f64::from(x0), f64::from(y0));
                }
                // Level pixels (cropped at `origin`) → full image pixels → element space.
                let paint_t = map * Affine::scale(1.0 / rel) * Affine::translate(origin);
                let (xe, ye) = tile_extend(tiling.mode);
                self.ctx.set_paint(PaintType::Image(Image {
                    image: ImageSource::Pixmap(pix),
                    sampler: ImageSampler {
                        x_extend: xe,
                        y_extend: ye,
                        quality: ImageQuality::Medium,
                        alpha: (alpha * opacity).clamp(0.0, 1.0),
                    },
                }));
                self.ctx.set_paint_transform(paint_t);
                let clip = (tiling.mode == TileMode::None).then(|| {
                    let mut p = rect_path(tiling.viewport);
                    p.apply_affine(*transform);
                    p
                });
                Some(Prepared { clip })
            }
            Brush::Visual {
                visual,
                tiling,
                transform,
                opacity,
            } => {
                if depth > MAX_DEPTH || *self.offscreen >= MAX_OFFSCREEN {
                    return None;
                }
                let vp = tiling.viewport;
                let (dx, dy) = scales(t * *transform);
                let tw = (dx * vp[2]).abs().ceil().clamp(1.0, MAX_TILE_SIDE);
                let th = (dy * vp[3]).abs().ceil().clamp(1.0, MAX_TILE_SIDE);
                let content = box_map(tiling.viewbox, [0.0, 0.0, tw, th])?;
                *self.offscreen += 1;
                let mut ctx = RenderContext::new(tw as u16, th as u16);
                let mut r = Renderer::new(self.doc, &mut ctx, self.cancelled, self.offscreen);
                r.count = self.count;
                r.draw_node(visual, content, depth + 1);
                self.count = r.count;
                let stop = r.stop;
                ctx.flush();
                let mut pix = Pixmap::new(tw as u16, th as u16);
                ctx.render(&mut pix, &mut Resources::default());
                if stop {
                    self.stop = true;
                }
                let paint_t = *transform
                    * Affine::translate((vp[0], vp[1]))
                    * Affine::scale_non_uniform(vp[2] / tw, vp[3] / th);
                let (xe, ye) = tile_extend(tiling.mode);
                self.ctx.set_transform(t);
                self.ctx.set_paint(PaintType::Image(Image {
                    image: ImageSource::Pixmap(Arc::new(pix)),
                    sampler: ImageSampler {
                        x_extend: xe,
                        y_extend: ye,
                        quality: ImageQuality::Medium,
                        alpha: (alpha * opacity).clamp(0.0, 1.0),
                    },
                }));
                self.ctx.set_paint_transform(paint_t);
                let clip = (tiling.mode == TileMode::None).then(|| {
                    let mut p = rect_path(vp);
                    p.apply_affine(*transform);
                    p
                });
                Some(Prepared { clip })
            }
        }
    }

    fn gradient(
        &mut self,
        g: Gradient,
        stops: &[Stop],
        spread: Spread,
        linear_rgb: bool,
        transform: Affine,
        alpha: f32,
    ) -> Option<Prepared> {
        match stops {
            [] => None,
            [one] => {
                self.ctx.set_paint(color(one.color, alpha));
                Some(Prepared { clip: None })
            }
            _ => {
                let cs: Vec<ColorStop> = stops
                    .iter()
                    .map(|s| ColorStop {
                        offset: s.offset.clamp(0.0, 1.0),
                        color: DynamicColor::from_alpha_color(color(s.color, alpha)),
                    })
                    .collect();
                let mut g = g.with_extend(extend(spread)).with_stops(cs.as_slice());
                if linear_rgb {
                    g = g.with_interpolation_cs(ColorSpaceTag::LinearSrgb);
                }
                self.ctx.set_paint(PaintType::Gradient(g));
                self.ctx.set_paint_transform(transform);
                Some(Prepared { clip: None })
            }
        }
    }

    fn fill(
        &mut self,
        path: &BezPath,
        rule: Fill,
        brush: &Brush,
        t: Affine,
        alpha: f32,
        depth: u32,
    ) {
        // A visual brush that does not tile draws its visual directly, at full resolution.
        if let Brush::Visual {
            visual,
            tiling,
            transform,
            opacity,
        } = brush
            && tiling.mode == TileMode::None
        {
            let Some(map) = box_map(tiling.viewbox, tiling.viewport) else {
                return;
            };
            self.ctx.set_transform(t);
            self.ctx.set_fill_rule(rule);
            self.ctx.push_clip_layer(path);
            let mut vp = rect_path(tiling.viewport);
            vp.apply_affine(*transform);
            self.ctx.set_fill_rule(Fill::NonZero);
            self.ctx.push_clip_layer(&vp);
            let a = alpha * opacity;
            if a < 1.0 {
                self.ctx.push_opacity_layer(a);
            }
            self.draw_node(visual, t * *transform * map, depth + 1);
            if a < 1.0 {
                self.ctx.pop_layer();
            }
            self.ctx.pop_layer();
            self.ctx.pop_layer();
            return;
        }
        let Some(p) = self.prepare(brush, t, alpha, depth) else {
            return;
        };
        self.ctx.set_transform(t);
        if let Some(clip) = &p.clip {
            self.ctx.set_fill_rule(Fill::NonZero);
            self.ctx.push_clip_layer(clip);
        }
        self.ctx.set_fill_rule(rule);
        self.ctx.fill_path(path);
        if p.clip.is_some() {
            self.ctx.pop_layer();
        }
    }

    fn stroke(
        &mut self,
        path: &BezPath,
        s: &StrokeStyle,
        brush: &Brush,
        t: Affine,
        alpha: f32,
        depth: u32,
    ) {
        let mut width = s.width;
        if width <= 0.0 {
            return;
        }
        // Keep hairlines visible: at least a quarter device pixel.
        let (dx, dy) = scales(t);
        let dev = dx.min(dy);
        if dev > 0.0 && width * dev < 0.25 {
            width = 0.25 / dev;
        }
        let Some(p) = self.prepare(brush, t, alpha, depth) else {
            return;
        };
        let dashed = s.dash.iter().any(|d| *d > 0.0);
        let mut st = Stroke::new(width)
            .with_join(match s.join {
                Join::Miter => kurbo::Join::Miter,
                Join::Bevel => kurbo::Join::Bevel,
                Join::Round => kurbo::Join::Round,
            })
            .with_miter_limit(s.miter)
            .with_start_cap(kurbo_cap(s.start_cap))
            .with_end_cap(kurbo_cap(s.end_cap));
        if dashed {
            let mut pattern: Vec<f64> = s.dash.iter().map(|d| d * s.width).collect();
            if pattern.len() % 2 == 1 {
                pattern.extend_from_within(..);
            }
            st = st
                .with_dashes(s.dash_offset * s.width, pattern)
                .with_caps(kurbo_cap(s.dash_cap));
        }
        self.ctx.set_stroke(st);
        self.ctx.set_transform(t);
        if let Some(clip) = &p.clip {
            self.ctx.set_fill_rule(Fill::NonZero);
            self.ctx.push_clip_layer(clip);
        }
        self.ctx.stroke_path(path);
        if p.clip.is_some() {
            self.ctx.pop_layer();
        }
    }
}

fn crop(p: &Pixmap, x: u32, y: u32, w: u32, h: u32) -> Pixmap {
    let src = p.data_as_u8_slice();
    let pw = usize::from(p.width());
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for row in y..y + h {
        let start = (row as usize * pw + x as usize) * 4;
        out.extend_from_slice(&src[start..start + w as usize * 4]);
    }
    Pixmap::from_parts(
        out,
        w as u16,
        h as u16,
        vello_cpu::PixelMetadata::new(peniko::ImageAlphaType::AlphaPremultiplied, true),
    )
}
