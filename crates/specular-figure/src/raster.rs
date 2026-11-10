//! Raster output: the figure drawn into 16-bit RGB at a scale (1: one figure px per output
//! px). Pictures placed at their own size are copied sample for sample at scale 1; scaled ones
//! are sampled nearest (image pixels) or bilinear (colour ramps). Items are rasterised by zeno
//! (non-zero fill, round-joined strokes) with anti-aliased coverage and blended in the encoded
//! values, as SVG renderers do.

use zeno::{Cap, Command, Fill, Join, Mask, Stroke, Style, Transform, Vector};

use crate::{El, Figure, Rgb16, Rgba};

fn commands(p: &[El]) -> Vec<Command> {
    let v = |a: [f32; 2]| Vector::new(a[0], a[1]);
    p.iter()
        .map(|e| match *e {
            El::Move(a) => Command::MoveTo(v(a)),
            El::Line(a) => Command::LineTo(v(a)),
            El::Quad(c, a) => Command::QuadTo(v(c), v(a)),
            El::Cubic(c1, c2, a) => Command::CurveTo(v(c1), v(c2), v(a)),
            El::Close => Command::Close,
        })
        .collect()
}

fn to16(c: Rgba) -> [f32; 3] {
    [
        f32::from(c.r) * 257.0,
        f32::from(c.g) * 257.0,
        f32::from(c.b) * 257.0,
    ]
}

/// Blends `colour` into `out` through a coverage mask placed at (`left`, `top`).
fn blend(out: &mut Rgb16, mask: &[u8], left: i32, top: i32, mw: u32, mh: u32, colour: Rgba) {
    let c = to16(colour);
    let (w, h) = (out.width as i32, out.height as i32);
    for my in 0..mh as i32 {
        let y = top + my;
        if y < 0 || y >= h {
            continue;
        }
        for mx in 0..mw as i32 {
            let x = left + mx;
            if x < 0 || x >= w {
                continue;
            }
            let cov = mask[(my * mw as i32 + mx) as usize];
            if cov == 0 {
                continue;
            }
            let a = f32::from(cov) / 255.0 * colour.a;
            let i = (y as usize * out.width as usize + x as usize) * 3;
            for (o, ck) in out.data[i..i + 3].iter_mut().zip(c) {
                let d = f32::from(*o);
                *o = (d + (ck - d) * a).round().clamp(0.0, 65535.0) as u16;
            }
        }
    }
}

fn draw_picture(out: &mut Rgb16, p: &crate::Picture, k: f32) {
    let src = &p.px;
    if src.width == 0 || src.height == 0 {
        return;
    }
    if k == 1.0 && p.is_exact() {
        let (x0, y0) = (p.x as i64, p.y as i64);
        for sy in 0..src.height as i64 {
            let y = y0 + sy;
            if y < 0 || y >= i64::from(out.height) {
                continue;
            }
            for sx in 0..src.width as i64 {
                let x = x0 + sx;
                if x < 0 || x >= i64::from(out.width) {
                    continue;
                }
                let si = (sy as usize * src.width as usize + sx as usize) * 3;
                let di = (y as usize * out.width as usize + x as usize) * 3;
                out.data[di..di + 3].copy_from_slice(&src.data[si..si + 3]);
            }
        }
        return;
    }
    let (ox0, oy0) = ((p.x * k).round() as i64, (p.y * k).round() as i64);
    let (ox1, oy1) = (
        ((p.x + p.w) * k).round() as i64,
        ((p.y + p.h) * k).round() as i64,
    );
    let (sw, sh) = (src.width as f32, src.height as f32);
    let (pw, ph) = ((ox1 - ox0).max(1) as f32, (oy1 - oy0).max(1) as f32);
    for y in oy0.max(0)..oy1.min(i64::from(out.height)) {
        let v = ((y - oy0) as f32 + 0.5) / ph * sh;
        for x in ox0.max(0)..ox1.min(i64::from(out.width)) {
            let u = ((x - ox0) as f32 + 0.5) / pw * sw;
            let px = if p.smooth {
                bilinear(src, u - 0.5, v - 0.5)
            } else {
                let sx = (u as u32).min(src.width - 1);
                let sy = (v as u32).min(src.height - 1);
                src.px(sx, sy)
            };
            let di = (y as usize * out.width as usize + x as usize) * 3;
            out.data[di..di + 3].copy_from_slice(&px);
        }
    }
}

fn bilinear(src: &Rgb16, u: f32, v: f32) -> [u16; 3] {
    let x0 = u.floor().clamp(0.0, (src.width - 1) as f32) as u32;
    let y0 = v.floor().clamp(0.0, (src.height - 1) as f32) as u32;
    let x1 = (x0 + 1).min(src.width - 1);
    let y1 = (y0 + 1).min(src.height - 1);
    let fx = (u - x0 as f32).clamp(0.0, 1.0);
    let fy = (v - y0 as f32).clamp(0.0, 1.0);
    let (a, b, c, d) = (
        src.px(x0, y0),
        src.px(x1, y0),
        src.px(x0, y1),
        src.px(x1, y1),
    );
    let mut o = [0u16; 3];
    for i in 0..3 {
        let top = f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * fx;
        let bot = f32::from(c[i]) + (f32::from(d[i]) - f32::from(c[i])) * fx;
        o[i] = (top + (bot - top) * fy).round() as u16;
    }
    o
}

/// The figure at `scale` output px per figure px.
pub fn render(f: &Figure, scale: f32) -> Rgb16 {
    let k = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = ((f.width as f32 * k).round() as u32).max(1);
    let h = ((f.height as f32 * k).round() as u32).max(1);
    let bg = to16(f.background).map(|v| v as u16);
    let mut out = Rgb16::filled(w, h, bg);
    for p in &f.pictures {
        draw_picture(&mut out, p, k);
    }
    let t = Transform::scale(k, k);
    for it in &f.items {
        if it.path.is_empty() {
            continue;
        }
        let cmds = commands(&it.path);
        let mut styles: Vec<(Style, Rgba)> = Vec::new();
        if let Some(c) = it.fill {
            styles.push((Style::Fill(Fill::NonZero), c));
        }
        if let Some(s) = it.stroke {
            let mut st = Stroke::new(s.width);
            st.join(Join::Round).cap(Cap::Round);
            styles.push((Style::Stroke(st), s.colour));
        }
        for (style, colour) in styles {
            let (mask, place) = Mask::new(&cmds[..])
                .style(style)
                .transform(Some(t))
                .render();
            if place.width == 0 || place.height == 0 {
                continue;
            }
            blend(
                &mut out,
                &mask,
                place.left,
                place.top,
                place.width,
                place.height,
                colour,
            );
        }
    }
    out
}
