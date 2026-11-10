//! The draw list: widgets push primitives in logical pixels; everything is converted to
//! physical pixels and linear premultiplied colour here, ready for one instanced draw call
//! per texture binding.

use crate::mark::{Mark, MarkRef};
use bytemuck::{Pod, Zeroable};
use specular_theme::Color;

/// A rectangle in logical pixels (or physical, where stated).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn from_min_max(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect {
            x: x0.min(x1),
            y: y0.min(y1),
            w: (x1 - x0).abs(),
            h: (y1 - y0).abs(),
        }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn contains(&self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.0 < self.right() && p.1 >= self.y && p.1 < self.bottom()
    }
    pub fn inset(&self, d: f32) -> Rect {
        Rect {
            x: self.x + d,
            y: self.y + d,
            w: (self.w - 2.0 * d).max(0.0),
            h: (self.h - 2.0 * d).max(0.0),
        }
    }
    pub fn inset_xy(&self, dx: f32, dy: f32) -> Rect {
        Rect {
            x: self.x + dx,
            y: self.y + dy,
            w: (self.w - 2.0 * dx).max(0.0),
            h: (self.h - 2.0 * dy).max(0.0),
        }
    }
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0.0),
            h: (y1 - y0).max(0.0),
        }
    }
    pub fn scale(&self, s: f32) -> Rect {
        Rect {
            x: self.x * s,
            y: self.y * s,
            w: self.w * s,
            h: self.h * s,
        }
    }
    /// Centres a `w`×`h` rectangle inside this one.
    pub fn centered(&self, w: f32, h: f32) -> Rect {
        Rect {
            x: self.x + (self.w - w) * 0.5,
            y: self.y + (self.h - h) * 0.5,
            w,
            h,
        }
    }
    /// Mirrors the rectangle horizontally inside a container of width `width` (RTL layout).
    pub fn mirror_x(&self, width: f32) -> Rect {
        Rect {
            x: width - self.x - self.w,
            ..*self
        }
    }
}

/// One instance of the UI uber-shader. 128 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Prim {
    /// Quad to rasterise (physical px): x, y, w, h.
    pub rect: [f32; 4],
    /// Shape: rect x, y, w, h; line x0, y0, x1, y1; arc cx, cy, r, 0.
    pub shape: [f32; 4],
    /// Corner radii (tl, tr, br, bl); line: half width; arc: start, sweep, half width.
    pub radii: [f32; 4],
    /// Fill colour, linear premultiplied.
    pub color: [f32; 4],
    /// Border colour, linear premultiplied.
    pub color2: [f32; 4],
    /// Border width, shadow sigma, kind, gamma / opacity.
    pub params: [f32; 4],
    /// Texture source rect: glyph atlas texels, or normalised uv for images.
    pub uv: [f32; 4],
    /// Clip rect x0, y0, x1, y1 (physical px).
    pub clip: [f32; 4],
}

pub mod kind {
    pub const RECT: f32 = 0.0;
    pub const SHADOW: f32 = 1.0;
    pub const GLYPH: f32 = 2.0;
    pub const LINE: f32 = 3.0;
    pub const ARC: f32 = 4.0;
    pub const GLASS: f32 = 5.0;
    pub const IMAGE: f32 = 6.0;
}

/// What shows through the transparent parts of an image (see [`DrawList::image_over`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Backdrop {
    /// Nothing: whatever was drawn underneath.
    None,
    Solid(Color),
    /// A checkerboard fixed to the screen, `cell` logical px square, `a` in the cells whose
    /// column + row is even (the viewer's pattern, so both line up at the same scale).
    Checker {
        a: Color,
        b: Color,
        cell: f32,
    },
}

/// Identifies a texture registered with [`crate::UiRenderer::register_texture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TexId(pub u32);

#[derive(Clone, Copy, Debug)]
pub struct Batch {
    pub image: Option<TexId>,
    pub start: u32,
    pub end: u32,
}

const NO_CLIP: [f32; 4] = [-1e9, -1e9, 1e9, 1e9];

/// Collected primitives for one frame.
#[derive(Debug)]
pub struct DrawList {
    /// Logical → physical pixels.
    pub scale: f32,
    pub prims: Vec<Prim>,
    pub batches: Vec<Batch>,
    clips: Vec<[f32; 4]>,
    alpha: Vec<f32>,
    current_image: Option<TexId>,
    /// The marks drawn this frame, whose atlas positions the renderer fills in.
    pub marks: Vec<MarkRef>,
}

impl Default for DrawList {
    fn default() -> Self {
        DrawList::new(1.0)
    }
}

fn lin(c: Color, a: f32) -> [f32; 4] {
    let mut v = c.to_linear_premul();
    for x in &mut v {
        *x *= a;
    }
    v
}

impl DrawList {
    pub fn new(scale: f32) -> DrawList {
        DrawList {
            scale,
            prims: Vec::new(),
            batches: Vec::new(),
            clips: Vec::new(),
            alpha: Vec::new(),
            current_image: None,
            marks: Vec::new(),
        }
    }

    pub fn clear(&mut self, scale: f32) {
        self.scale = scale;
        self.prims.clear();
        self.batches.clear();
        self.clips.clear();
        self.alpha.clear();
        self.current_image = None;
        self.marks.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.prims.is_empty()
    }

    fn clip(&self) -> [f32; 4] {
        self.clips.last().copied().unwrap_or(NO_CLIP)
    }

    /// Current opacity multiplier.
    pub fn opacity(&self) -> f32 {
        self.alpha.last().copied().unwrap_or(1.0)
    }

    /// Multiplies the opacity of everything drawn until [`pop_opacity`](Self::pop_opacity).
    pub fn push_opacity(&mut self, a: f32) {
        let o = self.opacity() * a.clamp(0.0, 1.0);
        self.alpha.push(o);
    }

    pub fn pop_opacity(&mut self) {
        self.alpha.pop();
    }

    pub fn push_clip(&mut self, r: Rect) {
        let p = r.scale(self.scale);
        let c = self.clip();
        self.clips.push([
            p.x.max(c[0]),
            p.y.max(c[1]),
            p.right().min(c[2]),
            p.bottom().min(c[3]),
        ]);
    }

    pub fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn push(&mut self, mut p: Prim, image: Option<TexId>) {
        p.clip = self.clip();
        // Cull primitives entirely outside the clip.
        if p.rect[0] > p.clip[2]
            || p.rect[1] > p.clip[3]
            || p.rect[0] + p.rect[2] < p.clip[0]
            || p.rect[1] + p.rect[3] < p.clip[1]
        {
            return;
        }
        let idx = self.prims.len() as u32;
        let need_new = match self.batches.last() {
            None => true,
            Some(b) => image.is_some() && b.image != image,
        };
        if need_new {
            let img = image.or(self.current_image);
            self.batches.push(Batch {
                image: img,
                start: idx,
                end: idx,
            });
        }
        if image.is_some() {
            self.current_image = image;
        }
        self.prims.push(p);
        if let Some(b) = self.batches.last_mut() {
            b.end = idx + 1;
        }
    }

    /// A filled rounded rectangle.
    pub fn rect(&mut self, r: Rect, radius: f32, fill: Color) {
        self.rect_full(r, [radius; 4], fill, Color::TRANSPARENT, 0.0);
    }

    /// A rounded rectangle with a border drawn inside its edge.
    pub fn rect_border(&mut self, r: Rect, radius: f32, fill: Color, border: Color, width: f32) {
        self.rect_full(r, [radius; 4], fill, border, width);
    }

    /// Only a border.
    pub fn stroke_rect(&mut self, r: Rect, radius: f32, border: Color, width: f32) {
        self.rect_full(r, [radius; 4], Color::TRANSPARENT, border, width);
    }

    /// A rectangle with per-corner radii (top-left, top-right, bottom-right, bottom-left).
    pub fn rect_full(&mut self, r: Rect, radii: [f32; 4], fill: Color, border: Color, width: f32) {
        let s = self.scale;
        let a = self.opacity();
        let p = r.scale(s);
        if p.w <= 0.0 || p.h <= 0.0 {
            return;
        }
        let q = p.inset(-1.0);
        self.push(
            Prim {
                rect: [q.x, q.y, q.w, q.h],
                shape: [p.x, p.y, p.w, p.h],
                radii: radii.map(|v| v * s),
                color: lin(fill, a),
                color2: lin(border, a),
                params: [width * s, 0.0, kind::RECT, 1.0],
                ..Default::default()
            },
            None,
        );
    }

    /// A soft drop shadow of a rounded rectangle; `sigma` is the Gaussian blur radius.
    pub fn shadow(&mut self, r: Rect, radius: f32, color: Color, sigma: f32, dy: f32) {
        let s = self.scale;
        let a = self.opacity();
        let p = r.translate(0.0, dy).scale(s);
        let sg = (sigma * s).max(0.5);
        let q = p.inset(-3.0 * sg - 1.0);
        self.push(
            Prim {
                rect: [q.x, q.y, q.w, q.h],
                shape: [p.x, p.y, p.w, p.h],
                radii: [radius * s; 4],
                color: lin(color, a),
                params: [0.0, sg, kind::SHADOW, 1.0],
                ..Default::default()
            },
            None,
        );
    }

    /// Frosted glass: the blurred image behind `r`, tinted, with an optional hairline border.
    pub fn glass(&mut self, r: Rect, radius: f32, tint: Color, border: Color, width: f32) {
        let s = self.scale;
        let a = self.opacity();
        let p = r.scale(s);
        let q = p.inset(-1.0);
        self.push(
            Prim {
                rect: [q.x, q.y, q.w, q.h],
                shape: [p.x, p.y, p.w, p.h],
                radii: [radius * s; 4],
                color: lin(tint, 1.0),
                color2: lin(border, 1.0),
                params: [width * s, 0.0, kind::GLASS, a],
                ..Default::default()
            },
            None,
        );
    }

    /// A line with round caps.
    pub fn line(&mut self, p0: (f32, f32), p1: (f32, f32), width: f32, color: Color) {
        let s = self.scale;
        let a = self.opacity();
        let hw = width * s * 0.5;
        let (x0, y0, x1, y1) = (p0.0 * s, p0.1 * s, p1.0 * s, p1.1 * s);
        let q = Rect::from_min_max(x0, y0, x1, y1).inset(-hw - 1.0);
        self.push(
            Prim {
                rect: [q.x, q.y, q.w, q.h],
                shape: [x0, y0, x1, y1],
                radii: [hw, 0.0, 0.0, 0.0],
                color: lin(color, a),
                params: [0.0, 0.0, kind::LINE, 1.0],
                ..Default::default()
            },
            None,
        );
    }

    /// A circular arc from angle `start` (radians, clockwise from +x since y points down)
    /// sweeping `sweep` radians, stroked with round caps.
    pub fn arc(
        &mut self,
        c: (f32, f32),
        radius: f32,
        start: f32,
        sweep: f32,
        width: f32,
        color: Color,
    ) {
        let s = self.scale;
        let a = self.opacity();
        let hw = width * s * 0.5;
        let (cx, cy, r) = (c.0 * s, c.1 * s, radius * s);
        let q = Rect::new(cx - r, cy - r, 2.0 * r, 2.0 * r).inset(-hw - 1.0);
        self.push(
            Prim {
                rect: [q.x, q.y, q.w, q.h],
                shape: [cx, cy, r, 0.0],
                radii: [start, sweep, hw, 0.0],
                color: lin(color, a),
                params: [0.0, 0.0, kind::ARC, 1.0],
                ..Default::default()
            },
            None,
        );
    }

    pub fn circle(&mut self, c: (f32, f32), radius: f32, fill: Color) {
        self.rect(
            Rect::new(c.0 - radius, c.1 - radius, 2.0 * radius, 2.0 * radius),
            radius,
            fill,
        );
    }

    /// `mark` in `color`, centred in `r` (logical px) at the width a stroke icon takes in the
    /// same box (20 of its 24 units), never mirrored. Its coverage goes into the glyph atlas
    /// when the frame is rendered ([`crate::Text::place_marks`]).
    pub fn mark(&mut self, mark: Mark, r: Rect, color: Color) {
        let s = self.scale;
        let a = self.opacity();
        let (vw, vh) = mark.view_box();
        let w = r.w.min(r.h) * (20.0 / 24.0);
        let h = w * vh / vw;
        let k = w * s / vw;
        // Whole pixels: the glyph shader reads the atlas texel by texel.
        let x = ((r.x + (r.w - w) * 0.5) * s).round();
        let y = ((r.y + (r.h - h) * 0.5) * s).round();
        let (pw, ph) = ((vw * k).ceil(), (vh * k).ceil());
        let n = self.prims.len();
        self.push(
            Prim {
                rect: [x, y, pw, ph],
                shape: [x, y, pw, ph],
                color: lin(color, a),
                params: [0.0, 0.0, kind::GLYPH, 1.0],
                ..Default::default()
            },
            None,
        );
        if self.prims.len() > n {
            self.marks.push(MarkRef {
                prim: n as u32,
                mark,
                k,
                w: pw as u32,
                h: ph as u32,
            });
        }
    }

    /// A registered texture drawn into `r` (logical px), `uv` normalised x, y, w, h.
    pub fn image(&mut self, r: Rect, uv: [f32; 4], tex: TexId, radius: f32) {
        let s = self.scale;
        let a = self.opacity();
        let p = r.scale(s);
        self.push(
            Prim {
                rect: [p.x, p.y, p.w, p.h],
                shape: [p.x, p.y, p.w, p.h],
                radii: [radius * s; 4],
                uv,
                params: [0.0, 0.0, kind::IMAGE, a],
                ..Default::default()
            },
            Some(tex),
        );
    }

    /// [`DrawList::image`] composited over `backdrop` (straight under the image, so the
    /// backdrop never shows outside it or around its rounded corners).
    pub fn image_over(
        &mut self,
        r: Rect,
        uv: [f32; 4],
        tex: TexId,
        radius: f32,
        backdrop: Backdrop,
    ) {
        let s = self.scale;
        let a = self.opacity();
        let p = r.scale(s);
        // Colours unscaled by opacity: the shader applies it to the composite.
        let (ca, cb, cell) = match backdrop {
            Backdrop::None => ([0.0; 4], [0.0; 4], 0.0),
            Backdrop::Solid(c) => (lin(c, 1.0), lin(c, 1.0), 1.0e6),
            Backdrop::Checker { a, b, cell } => {
                (lin(a, 1.0), lin(b, 1.0), (cell * s).round().max(1.0))
            }
        };
        self.push(
            Prim {
                rect: [p.x, p.y, p.w, p.h],
                shape: [p.x, p.y, p.w, p.h],
                radii: [radius * s; 4],
                color: ca,
                color2: cb,
                uv,
                params: [0.0, cell, kind::IMAGE, a],
                ..Default::default()
            },
            Some(tex),
        );
    }

    /// A glyph quad (physical px, already pixel-aligned) from the atlas.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn glyph(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        atlas: [f32; 2],
        color: [f32; 4],
        gamma: f32,
    ) {
        let a = self.opacity();
        self.push(
            Prim {
                rect: [x, y, w, h],
                shape: [x, y, w, h],
                color: color.map(|c| c * a),
                uv: [atlas[0], atlas[1], w, h],
                params: [0.0, 0.0, kind::GLYPH, gamma],
                ..Default::default()
            },
            None,
        );
    }
}
