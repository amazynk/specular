//! Drawing a picture's display list with vello_cpu, and its text with the drawing stack's
//! fonts.

use std::sync::Arc;

use kurbo::{Affine, Rect, Shape, Stroke};
use specular_vector::fonts::{Face, Fonts};
use specular_vector::scene::FontSpec;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::peniko::{Extend, Fill, ImageQuality, ImageSampler};
use vello_cpu::{
    Image, ImageSource, PixmapMut, RasterizerSettings, RenderContext, Resources, TargetInit,
};

use crate::list::{HAlign, Op, Paint, Picture, Rgba, TextOp, VAlign};

/// Thinnest line drawn, output pixels.
const MIN_LINE_PX: f64 = 1.0;

fn colour(c: Rgba) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c[0], c[1], c[2], c[3])
}

/// Draws `pic` into `w`×`h` straight RGBA8 (opaque): page point `p` lands on pixel
/// `scale · p − (x, y)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(
    pic: &Picture,
    faces: &[Arc<Face>],
    fonts: &Fonts,
    scale: (f64, f64),
    origin: (u32, u32),
    size: (u16, u16),
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<u8>> {
    let (w, h) = size;
    let base = Affine::translate((-f64::from(origin.0), -f64::from(origin.1)))
        * Affine::scale_non_uniform(scale.0, scale.1);
    let px_per_pt = (scale.0 * scale.1).abs().sqrt();
    let mut ctx = RenderContext::new(w, h);
    ctx.set_transform(base);
    ctx.set_paint(colour(pic.background));
    ctx.fill_rect(&Rect::new(0.0, 0.0, pic.size.0, pic.size.1));
    let mut clipped = false;
    for (i, op) in pic.ops.iter().enumerate() {
        if i % 256 == 0 && cancelled() {
            return None;
        }
        ctx.set_transform(base);
        match op {
            Op::Clip(r) => {
                if clipped {
                    ctx.pop_layer();
                    clipped = false;
                }
                if let Some(r) = r {
                    ctx.set_fill_rule(Fill::NonZero);
                    ctx.push_clip_layer(&r.to_path(0.1));
                    clipped = true;
                }
            }
            Op::Fill {
                path,
                even_odd,
                paint,
            } => {
                ctx.set_fill_rule(if *even_odd {
                    Fill::EvenOdd
                } else {
                    Fill::NonZero
                });
                match paint {
                    Paint::Solid(c) => ctx.set_paint(colour(*c)),
                    Paint::Pattern { pixmap, transform } => {
                        ctx.set_paint(Image {
                            image: ImageSource::Pixmap(pixmap.clone()),
                            sampler: ImageSampler {
                                x_extend: Extend::Repeat,
                                y_extend: Extend::Repeat,
                                quality: ImageQuality::Low,
                                alpha: 1.0,
                            },
                        });
                        ctx.set_paint_transform(*transform);
                    }
                }
                ctx.fill_path(path);
                ctx.reset_paint_transform();
            }
            Op::Stroke {
                path,
                style,
                colour: c,
            } => {
                ctx.set_paint(colour(*c));
                ctx.set_stroke(thin(style, px_per_pt));
                ctx.stroke_path(path);
            }
            Op::ClippedStroke {
                clip,
                even_odd,
                lines,
                width,
                colour: c,
            } => {
                ctx.set_fill_rule(if *even_odd {
                    Fill::EvenOdd
                } else {
                    Fill::NonZero
                });
                ctx.push_clip_layer(clip);
                ctx.set_paint(colour(*c));
                ctx.set_stroke(thin(&Stroke::new(*width), px_per_pt));
                ctx.stroke_path(lines);
                ctx.pop_layer();
            }
            Op::Image { pixmap, transform } => {
                ctx.set_transform(base * *transform);
                ctx.set_paint(Image {
                    image: ImageSource::Pixmap(pixmap.clone()),
                    sampler: ImageSampler {
                        x_extend: Extend::Pad,
                        y_extend: Extend::Pad,
                        quality: ImageQuality::Low,
                        alpha: 1.0,
                    },
                });
                ctx.set_fill_rule(Fill::NonZero);
                ctx.fill_rect(&Rect::new(
                    0.0,
                    0.0,
                    f64::from(pixmap.width()),
                    f64::from(pixmap.height()),
                ));
            }
            Op::Text(t) => draw_text(&mut ctx, base, t, faces, fonts),
        }
    }
    if clipped {
        ctx.pop_layer();
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

/// A stroke no thinner than [`MIN_LINE_PX`] at `px_per_pt`.
fn thin(s: &Stroke, px_per_pt: f64) -> Stroke {
    let min = MIN_LINE_PX / px_per_pt.max(1e-9);
    if s.width >= min {
        s.clone()
    } else {
        let mut s = s.clone();
        s.width = min;
        s
    }
}

/// The faces of the document's font list, in order (index 0 is font 1).
pub(crate) fn resolve_faces(names: &[String], fonts: &Fonts) -> Vec<Arc<Face>> {
    names.iter().map(|n| resolve_face(n, fonts)).collect()
}

/// The face for a CGM font name ("Helvetica-Bold", "Times-Roman", "Courier", "Hershey:…").
pub(crate) fn resolve_face(name: &str, fonts: &Fonts) -> Arc<Face> {
    let lower = name.to_ascii_lowercase();
    let bold = lower.contains("bold") || lower.contains("heavy") || lower.contains("black");
    let italic = lower.contains("italic") || lower.contains("oblique");
    // The family part: up to a style suffix.
    let family = name
        .split(['-', ',', ':'])
        .next()
        .unwrap_or(name)
        .trim()
        .to_string();
    let spec = FontSpec::default();
    if !family.is_empty() {
        let r = fonts.resolve(&spec, Some(&family), bold, italic);
        if !r.substituted {
            return r.face;
        }
    }
    let generic =
        if lower.contains("courier") || lower.contains("mono") || lower.contains("typewriter") {
            "Courier New"
        } else if (lower.contains("times") || lower.contains("roman") || lower.contains("serif"))
            && !lower.contains("sans")
        {
            "Times New Roman"
        } else {
            "Arial"
        };
    fonts.resolve(&spec, Some(generic), bold, italic).face
}

/// The cap height of a face in font units: the top of its "H" (the metrics' value is missing,
/// or a default, in many fonts).
fn cap_height(f: &Face) -> f64 {
    f.glyph('H')
        .map(|g| g.path.bounding_box())
        .filter(|b| b.y1 > 0.0 && b.y1.is_finite())
        .map_or(f64::from(f.cap_height.max(1.0)), |b| b.y1)
}

/// One placed glyph: its face, character and position in text units.
struct Placed<'a> {
    face: &'a Arc<Face>,
    c: char,
    x: f64,
    y: f64,
    /// Font units → text units, horizontally and vertically.
    sx: f64,
    sy: f64,
    colour: Rgba,
}

fn draw_text(
    ctx: &mut RenderContext,
    base: Affine,
    t: &TextOp,
    faces: &[Arc<Face>],
    fonts: &Fonts,
) {
    let default = fonts.default_face(false);
    let mut fallbacks: Vec<Arc<Face>> = Vec::new();
    // Lay the runs out along the baseline, in text units (one unit = cap height).
    struct G {
        face: Arc<Face>,
        c: char,
        adv: f64,
        sx: f64,
        sy: f64,
        colour: Rgba,
    }
    let mut glyphs: Vec<G> = Vec::new();
    let mut asc: f64 = 0.0;
    let mut desc: f64 = 0.0;
    for r in &t.runs {
        let face = faces.get(r.font).unwrap_or(&default).clone();
        for &c in &r.text {
            let mut f = face.clone();
            if !f.has(c)
                && let Some(fb) = fonts.fallback_for(&f, c)
            {
                if !fallbacks.iter().any(|x| Arc::ptr_eq(x, &fb)) {
                    fallbacks.push(fb.clone());
                }
                f = fb;
            }
            let cap = cap_height(&f);
            let sy = r.height / cap;
            let sx = sy * r.expansion;
            let adv = f
                .glyph(c)
                .map_or(f64::from(f.units_per_em) * 0.5, |g| f64::from(g.advance));
            asc = asc.max(f64::from(f.ascent) * sy);
            desc = desc.min(f64::from(f.descent) * sy);
            glyphs.push(G {
                adv: adv * sx + r.spacing * r.height,
                face: f,
                c,
                sx,
                sy,
                colour: r.colour,
            });
        }
    }
    if glyphs.is_empty() {
        return;
    }
    let vertical = t.path >= 2;
    let line = (asc - desc).max(1.0);
    let mut placed: Vec<(usize, f64, f64)> = Vec::with_capacity(glyphs.len());
    let (width, height);
    if vertical {
        // Characters stacked, centred on the path.
        let n = glyphs.len() as f64;
        for (i, g) in glyphs.iter().enumerate() {
            let step = i as f64 * line;
            let y = if t.path == 2 { step } else { -step };
            placed.push((i, -(g.adv) / 2.0, y));
        }
        width = 0.0;
        height = n * line;
    } else {
        let total: f64 = glyphs.iter().map(|g| g.adv).sum();
        let mut x = 0.0;
        for (i, g) in glyphs.iter().enumerate() {
            if t.path == 1 {
                // Leftwards: the first character at the reference point.
                placed.push((i, -x - g.adv, 0.0));
            } else {
                placed.push((i, x, 0.0));
            }
            x += g.adv;
        }
        width = total;
        height = 1.0;
    }
    // Restricted text is scaled, keeping its proportions, to fit its box (WebCGM's
    // boxed-cap: larger or smaller as the box says).
    let (kx, ky) = match t.restrict {
        Some((bw, bh)) => {
            let fit_w = (!vertical && width > 0.0 && bw > 0.0).then(|| bw / width);
            let fit_h = (bh > 0.0 && height > 0.0).then(|| bh / height);
            let k = match (fit_w, fit_h) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => 1.0,
            };
            let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
            (k, k)
        }
        None => (1.0, 1.0),
    };
    // Alignment offsets (text units, before the squeeze).
    let dx = if vertical {
        match t.halign {
            HAlign::Left => width / 2.0,
            HAlign::Right => -width / 2.0,
            _ => 0.0,
        }
    } else {
        // The run spans [x0, x0 + w]: leftwards text ends at the reference point. Normal
        // alignment is left for rightwards text and right for leftwards text.
        let w = width;
        let x0 = if t.path == 1 { -w } else { 0.0 };
        match t.halign {
            HAlign::Normal if t.path == 1 => -x0 - w,
            HAlign::Normal | HAlign::Left => -x0,
            HAlign::Centre => -x0 - w / 2.0,
            HAlign::Right => -x0 - w,
            HAlign::Continuous(f) => -x0 - f * w,
        }
    };
    let dy = if vertical {
        let top = if t.path == 3 { -asc } else { 0.0 };
        match t.valign {
            VAlign::Normal => {
                if t.path == 3 {
                    top
                } else {
                    0.0
                }
            }
            VAlign::Top => -asc,
            VAlign::Cap => -1.0,
            VAlign::Half => {
                if t.path == 3 {
                    height / 2.0 - asc
                } else {
                    -height / 2.0
                }
            }
            VAlign::Base => 0.0,
            VAlign::Bottom => -desc,
            VAlign::Continuous(f) => -f,
        }
    } else {
        match t.valign {
            VAlign::Normal | VAlign::Base => 0.0,
            VAlign::Top => -asc,
            VAlign::Cap => -1.0,
            VAlign::Half => -0.5,
            VAlign::Bottom => -desc,
            VAlign::Continuous(f) => -f,
        }
    };
    let text = base * t.transform * Affine::scale_non_uniform(kx, ky) * Affine::translate((dx, dy));
    ctx.set_fill_rule(Fill::NonZero);
    for &(i, x, y) in &placed {
        let g = &glyphs[i];
        let p = Placed {
            face: &g.face,
            c: g.c,
            x,
            y,
            sx: g.sx,
            sy: g.sy,
            colour: g.colour,
        };
        let Some(glyph) = p.face.glyph(p.c) else {
            continue;
        };
        if glyph.path.elements().is_empty() {
            continue;
        }
        let m = text * Affine::new([p.sx, 0.0, 0.0, p.sy, p.x, p.y]);
        if !m.determinant().is_finite() || m.determinant() == 0.0 {
            continue;
        }
        ctx.set_transform(m);
        ctx.set_paint(colour(p.colour));
        ctx.fill_path(&glyph.path);
    }
}
