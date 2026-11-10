//! Drawing a page's display list with vello_cpu.

use kurbo::{Affine, BezPath, Rect, Shape, Stroke};
use vello_cpu::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::peniko::{ColorStop, Extend, Fill, Gradient, ImageQuality, ImageSampler};
use vello_cpu::{
    Image as VImage, ImageSource, PaintType, PixmapMut, RasterizerSettings, RenderContext,
    Resources, TargetInit,
};

use crate::text::layout;
use crate::{MAX_LAYERS, Op, Page, Paint, Rgba, Stop};

/// Thinnest line drawn, output pixels.
const MIN_LINE_PX: f64 = 0.75;
/// Slant of a synthesised italic.
const FAKE_ITALIC: f64 = 0.2;

fn colour(c: Rgba) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c[0], c[1], c[2], c[3])
}

fn stops(s: &[Stop]) -> Vec<ColorStop> {
    s.iter()
        .map(|s| ColorStop {
            offset: s.offset.clamp(0.0, 1.0),
            color: DynamicColor::from_alpha_color(colour(s.color)),
        })
        .collect()
}

/// Sets `paint` on the context; `false` when nothing would be drawn.
fn set_paint(ctx: &mut RenderContext, paint: &Paint) -> bool {
    ctx.reset_paint_transform();
    match paint {
        Paint::Solid(c) => {
            if c[3] == 0 {
                return false;
            }
            ctx.set_paint(colour(*c));
        }
        Paint::Linear {
            start,
            end,
            stops: s,
        } => match s.as_slice() {
            [] => return false,
            [one] => ctx.set_paint(colour(one.color)),
            _ => {
                if (*start - *end).hypot() < 1e-9 {
                    ctx.set_paint(colour(s[s.len() - 1].color));
                } else {
                    let g = Gradient::new_linear(*start, *end)
                        .with_extend(Extend::Pad)
                        .with_stops(stops(s).as_slice());
                    ctx.set_paint(PaintType::Gradient(g));
                }
            }
        },
        Paint::Radial {
            transform,
            stops: s,
        } => match s.as_slice() {
            [] => return false,
            [one] => ctx.set_paint(colour(one.color)),
            _ => {
                if !transform.determinant().is_finite() || transform.determinant() == 0.0 {
                    ctx.set_paint(colour(s[s.len() - 1].color));
                } else {
                    let g = Gradient::new_radial(kurbo::Point::ZERO, 1.0)
                        .with_extend(Extend::Pad)
                        .with_stops(stops(s).as_slice());
                    ctx.set_paint(PaintType::Gradient(g));
                    ctx.set_paint_transform(*transform);
                }
            }
        },
        Paint::Image {
            image,
            transform,
            repeat,
        } => {
            let (kx, ky) = image.pixmap_scale();
            let ext = if *repeat { Extend::Repeat } else { Extend::Pad };
            ctx.set_paint(VImage {
                image: ImageSource::Pixmap(image.pixmap.clone()),
                sampler: ImageSampler {
                    x_extend: ext,
                    y_extend: ext,
                    quality: ImageQuality::Medium,
                    alpha: 1.0,
                },
            });
            ctx.set_paint_transform(*transform * Affine::scale_non_uniform(kx, ky));
        }
    }
    true
}

/// Draws `page` into `w`×`h` straight RGBA8 (opaque): page point `p` lands on pixel
/// `scale · p − origin`.
pub(crate) fn render(
    page: &Page,
    scale: (f64, f64),
    origin: (u32, u32),
    size: (u16, u16),
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<u8>> {
    let (w, h) = size;
    let base = Affine::translate((-f64::from(origin.0), -f64::from(origin.1)))
        * Affine::scale_non_uniform(scale.0, scale.1);
    let px_per_pt = (scale.0 * scale.1).abs().sqrt();
    let view = base
        .inverse()
        .transform_rect_bbox(Rect::new(0.0, 0.0, f64::from(w), f64::from(h)))
        .inflate(2.0, 2.0);
    let mut ctx = RenderContext::new(w, h);
    ctx.set_transform(base);
    if page.background[3] > 0 {
        ctx.set_paint(colour(page.background));
        ctx.fill_rect(&Rect::new(0.0, 0.0, page.size.0, page.size.1));
    }
    // Pushed layers: whether each was really pushed (past MAX_LAYERS they are not).
    let mut layers: Vec<bool> = Vec::new();
    for (i, op) in page.ops.iter().enumerate() {
        if i % 256 == 0 && cancelled() {
            return None;
        }
        ctx.set_transform(base);
        match op {
            Op::PushClip { path, even_odd } => {
                let real = layers.len() < MAX_LAYERS;
                if real {
                    ctx.set_fill_rule(rule(*even_odd));
                    ctx.push_clip_layer(path);
                }
                layers.push(real);
            }
            Op::PushOpacity(a) => {
                let real = layers.len() < MAX_LAYERS;
                if real {
                    ctx.push_opacity_layer(a.clamp(0.0, 1.0));
                }
                layers.push(real);
            }
            Op::Pop => {
                if layers.pop() == Some(true) {
                    ctx.pop_layer();
                }
            }
            Op::Fill {
                path,
                even_odd,
                paint,
            } => {
                if !visible(path, 0.0, &view) || !set_paint(&mut ctx, paint) {
                    continue;
                }
                ctx.set_fill_rule(rule(*even_odd));
                ctx.fill_path(path);
            }
            Op::Stroke {
                path,
                stroke,
                paint,
            } => {
                if !visible(path, stroke.width, &view) || !set_paint(&mut ctx, paint) {
                    continue;
                }
                ctx.set_stroke(thin(stroke, px_per_pt));
                ctx.stroke_path(path);
            }
            Op::Image {
                image,
                transform,
                opacity,
            } => {
                if *opacity <= 0.0 {
                    continue;
                }
                let (kx, ky) = image.pixmap_scale();
                let m = *transform * Affine::scale_non_uniform(kx, ky);
                if !m.determinant().is_finite() || m.determinant() == 0.0 {
                    continue;
                }
                ctx.set_transform(base * m);
                ctx.reset_paint_transform();
                ctx.set_paint(VImage {
                    image: ImageSource::Pixmap(image.pixmap.clone()),
                    sampler: ImageSampler {
                        x_extend: Extend::Pad,
                        y_extend: Extend::Pad,
                        quality: ImageQuality::Medium,
                        alpha: opacity.clamp(0.0, 1.0),
                    },
                });
                ctx.set_fill_rule(Fill::NonZero);
                ctx.fill_rect(&Rect::new(
                    0.0,
                    0.0,
                    f64::from(image.pixmap.width()),
                    f64::from(image.pixmap.height()),
                ));
            }
            Op::Text(t) => {
                let bbox = t
                    .transform
                    .transform_rect_bbox(Rect::new(0.0, 0.0, t.width, t.height))
                    .inflate(t.width.abs() + t.height.abs() + 50.0, t.height.abs() + 50.0);
                if bbox.intersect(view).is_zero_area() {
                    continue;
                }
                draw_text(&mut ctx, base, t);
            }
        }
    }
    while let Some(real) = layers.pop() {
        if real {
            ctx.pop_layer();
        }
    }
    ctx.flush();
    let len = usize::from(w) * usize::from(h) * 4;
    let mut buf = Vec::new();
    buf.try_reserve_exact(len).ok()?;
    buf.resize(len, 0);
    ctx.render_with(
        PixmapMut::new(w, h, &mut buf)?,
        &mut Resources::default(),
        RasterizerSettings {
            target_init: TargetInit::Clear(vello_cpu::color::palette::css::WHITE),
            ..Default::default()
        },
    );
    // Over opaque white premultiplied is straight; pin alpha.
    for px in buf.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Some(buf)
}

fn rule(even_odd: bool) -> Fill {
    if even_odd {
        Fill::EvenOdd
    } else {
        Fill::NonZero
    }
}

/// Whether a path (widened by `width`) can touch the view.
fn visible(path: &BezPath, width: f64, view: &Rect) -> bool {
    if path.elements().is_empty() {
        return false;
    }
    let b = path.bounding_box().inflate(width, width);
    b.x0 <= view.x1 && b.x1 >= view.x0 && b.y0 <= view.y1 && b.y1 >= view.y0
}

/// A stroke no thinner than [`MIN_LINE_PX`] at `px_per_pt`.
fn thin(s: &Stroke, px_per_pt: f64) -> Stroke {
    let min = MIN_LINE_PX / px_per_pt.max(1e-9);
    let mut s = s.clone();
    if s.width.is_nan() || s.width < min {
        s.width = min;
    }
    s
}

fn draw_text(ctx: &mut RenderContext, base: Affine, t: &crate::TextBlock) {
    if t.is_empty() {
        return;
    }
    let (glyphs, lines) = layout(t);
    let m = base * t.transform;
    ctx.set_fill_rule(Fill::NonZero);
    ctx.reset_paint_transform();
    for l in &lines {
        ctx.set_transform(m);
        ctx.set_paint(colour(l.color));
        ctx.fill_rect(&Rect::new(
            l.x0,
            l.y - l.thickness / 2.0,
            l.x1,
            l.y + l.thickness / 2.0,
        ));
    }
    for g in &glyphs {
        let Some(glyph) = g.face.glyph(g.c) else {
            continue;
        };
        if glyph.path.elements().is_empty() {
            continue;
        }
        // Font units (y up) → box space at the glyph's baseline origin.
        let slant = if g.face.fake_italic { FAKE_ITALIC } else { 0.0 };
        let gm = m * Affine::new([g.scale, 0.0, slant * g.scale, -g.scale, g.x, g.y]);
        if !gm.determinant().is_finite() || gm.determinant() == 0.0 {
            continue;
        }
        ctx.set_transform(gm);
        ctx.set_paint(colour(g.color));
        ctx.fill_path(&glyph.path);
    }
}
