//! Nodes → items: block references expanded with their transforms, BYLAYER / BYBLOCK
//! resolved, layer-0 entities moved onto the reference's layer, and infinite lines clipped
//! to what the rest of the sheet covers.

use kurbo::{Affine, BezPath, Point, Rect, Shape as _, Vec2};
use specular_vector::{
    Color, Dash, HAlign, LW_DEFAULT, Layer, Lineweight, Shape, TextBlock, VAlign, Viewport,
    bounds_of,
};

use crate::CadError;
use crate::build::{ColorSpec, LtSpec, LwSpec, Node, Props};
use crate::geom::M3;

/// What a block reference passes down to its contents.
#[derive(Clone, Copy)]
struct Ctx {
    xf3: M3,
    /// The plan view of `xf3`.
    xf: Affine,
    /// sqrt(|det|) of `xf`: how much the linetype pattern grows.
    scale: f64,
    /// The layer that layer-0 entities take (inside a reference).
    layer: Option<u32>,
    /// The layer of the outermost reference.
    outer: Option<u32>,
    color: Color,
    lw: Lineweight,
    lt: Option<u32>,
}

impl Ctx {
    fn top() -> Ctx {
        Ctx {
            xf3: M3::IDENTITY,
            xf: Affine::IDENTITY,
            scale: 1.0,
            layer: None,
            outer: None,
            color: Color::Foreground,
            lw: LW_DEFAULT,
            lt: None,
        }
    }
}

struct Resolved {
    layer: u32,
    outer: u32,
    color: Color,
    lw: Lineweight,
    lt: Option<u32>,
    lt_scale: f64,
}

pub(crate) struct Emitter<'n> {
    pub templates: &'n [Vec<Node>],
    pub layers: &'n mut [Layer],
    pub layer0: u32,
    pub ltscale: f64,
    pub max_items: usize,
    pub max_depth: u32,
    /// Items of every sheet so far.
    pub total: usize,
    /// Block copies expanded over all sheets: nested arrays of blocks that draw little
    /// multiply without adding items.
    pub copies: usize,
    /// Path elements emitted over all sheets ([`MAX_ELEMENTS`]): a block of one huge path
    /// placed many times is few items but many elements.
    pub elements: usize,
    pub warnings: Vec<String>,
}

/// Path elements a drawing may emit in all, at most (about 2 GB).
pub(crate) const MAX_ELEMENTS: usize = 32_000_000;

/// One sheet's output.
#[derive(Default)]
pub(crate) struct Out {
    pub items: Vec<specular_vector::Item>,
    pub viewports: Vec<(Rect, Affine, Vec<u64>, Option<u64>)>,
    infinite: Vec<(Resolved, Point, Vec2, bool)>,
}

impl<'n> Emitter<'n> {
    fn warn(&mut self, w: &str) {
        if !self.warnings.iter().any(|x| x == w) {
            self.warnings.push(w.to_string());
        }
    }

    fn resolve(&self, p: &Props, ctx: &Ctx) -> Resolved {
        let layer = match ctx.layer {
            Some(l) if p.layer == self.layer0 => l,
            _ => p.layer,
        };
        let lay = self.layers.get(layer as usize);
        let color = match p.color {
            ColorSpec::ByLayer => lay.map_or(Color::Foreground, |l| l.color),
            ColorSpec::ByBlock => ctx.color,
            ColorSpec::Is(c) => c,
        };
        let lw = match p.lw {
            LwSpec::ByLayer => lay.map_or(LW_DEFAULT, |l| l.lineweight),
            LwSpec::ByBlock => ctx.lw,
            LwSpec::Is(w) => w,
        };
        let lt = match p.lt {
            LtSpec::ByLayer => lay.and_then(|l| l.linetype),
            LtSpec::ByBlock => ctx.lt,
            LtSpec::Continuous => None,
            LtSpec::Is(i) => Some(i),
        };
        Resolved {
            layer,
            outer: ctx.outer.unwrap_or(layer),
            color,
            lw,
            lt,
            lt_scale: self.ltscale * p.lt_scale * ctx.scale,
        }
    }

    fn push(&mut self, out: &mut Out, r: &Resolved, shape: Shape) -> Result<(), CadError> {
        self.total += 1;
        self.elements += match &shape {
            specular_vector::Shape::Stroke { path, .. }
            | specular_vector::Shape::Fill { path, .. } => path.elements().len(),
            _ => 1,
        };
        if self.total > self.max_items || self.elements > MAX_ELEMENTS {
            return Err(CadError::TooLarge);
        }
        if let Some(l) = self.layers.get_mut(r.layer as usize) {
            l.items += 1;
        }
        out.items.push(specular_vector::Item {
            layer: r.layer,
            outer: r.outer,
            color: r.color,
            shape,
        });
        Ok(())
    }

    /// Emits top-level nodes.
    pub fn sheet(&mut self, nodes: &[Node], out: &mut Out) -> Result<(), CadError> {
        let mut stack = Vec::new();
        self.nodes(nodes, &Ctx::top(), out, &mut stack)
    }

    fn nodes(
        &mut self,
        nodes: &[Node],
        ctx: &Ctx,
        out: &mut Out,
        stack: &mut Vec<usize>,
    ) -> Result<(), CadError> {
        for n in nodes {
            match n {
                Node::Draw {
                    props,
                    plane,
                    shapes,
                } => {
                    let r = self.resolve(props, ctx);
                    let xf = if ctx.xf3.is_flat() {
                        ctx.xf
                    } else {
                        // A tilted reference: back from the plan view to the entity's
                        // plane, then through the reference in 3D.
                        let p = M3::ocs(plane.n, plane.elev);
                        let flat = p.plan();
                        if flat.determinant().abs() > 1e-12 {
                            (ctx.xf3 * p).plan() * flat.inverse()
                        } else {
                            ctx.xf
                        }
                    };
                    for s in shapes {
                        let shape = transform(s, xf, &r);
                        self.push(out, &r, shape)?;
                    }
                }
                Node::Insert {
                    props,
                    block,
                    xfs,
                    attribs,
                } => {
                    let r = self.resolve(props, ctx);
                    if stack.contains(block) {
                        self.warn("a block that contains itself was drawn once");
                    } else if stack.len() as u32 >= self.max_depth {
                        self.warn("blocks nested too deeply were not drawn");
                    } else if let Some(t) = self.templates.get(*block) {
                        stack.push(*block);
                        for x in xfs {
                            self.copies += 1;
                            if self.copies > self.max_items {
                                stack.pop();
                                return Err(CadError::TooLarge);
                            }
                            let xf3 = ctx.xf3 * *x;
                            let xf = xf3.plan();
                            let inner = Ctx {
                                xf3,
                                xf,
                                scale: xf.determinant().abs().sqrt(),
                                layer: Some(r.layer),
                                outer: Some(r.outer),
                                color: r.color,
                                lw: r.lw,
                                lt: r.lt,
                            };
                            let res = self.nodes(t, &inner, out, stack);
                            if res.is_err() {
                                stack.pop();
                                return res;
                            }
                        }
                        stack.pop();
                    }
                    self.nodes(attribs, ctx, out, stack)?;
                }
                Node::Infinite {
                    props,
                    origin,
                    dir,
                    ray,
                } => {
                    let r = self.resolve(props, ctx);
                    let o = ctx.xf * *origin;
                    let d = (ctx.xf * (*origin + *dir)) - o;
                    if d.hypot() > 0.0 {
                        self.total += 1;
                        if self.total > self.max_items {
                            return Err(CadError::TooLarge);
                        }
                        out.infinite.push((r, o, d, *ray));
                    }
                }
                Node::Viewport(v) => {
                    if stack.is_empty() {
                        out.viewports
                            .push((v.paper, v.transform, v.frozen.clone(), v.clip));
                    }
                }
            }
        }
        Ok(())
    }

    /// Extents of the items, then the infinite lines clipped to them (grown by a tenth).
    pub fn finish(&mut self, out: &mut Out) -> Result<Option<Rect>, CadError> {
        let mut ext: Option<Rect> = None;
        for it in &out.items {
            if let Some(b) = item_bounds(&it.shape) {
                ext = Some(ext.map_or(b, |e| e.union(b)));
            }
        }
        let inf = std::mem::take(&mut out.infinite);
        if inf.is_empty() {
            return Ok(ext);
        }
        let area = ext.unwrap_or_else(|| bounds_of(inf.iter().map(|i| i.1)).unwrap_or(Rect::ZERO));
        let pad = (area.width().max(area.height()) * 0.1).max(1.0);
        let clip = area.inflate(pad, pad);
        for (r, o, d, ray) in inf {
            if let Some((a, b)) = clip_line(o, d, ray, clip) {
                let mut p = BezPath::new();
                p.move_to(a);
                p.line_to(b);
                let shape = Shape::Stroke {
                    path: p,
                    weight: r.lw,
                    dash: r.lt.map(|linetype| Dash {
                        linetype,
                        scale: r.lt_scale,
                    }),
                };
                self.push(out, &r, shape)?;
            }
        }
        Ok(ext)
    }
}

fn transform(s: &Shape, xf: Affine, r: &Resolved) -> Shape {
    let id = xf == Affine::IDENTITY;
    match s {
        Shape::Stroke { path, .. } => {
            let mut path = path.clone();
            if !id {
                path.apply_affine(xf);
            }
            Shape::Stroke {
                path,
                weight: r.lw,
                dash: r.lt.map(|linetype| Dash {
                    linetype,
                    scale: r.lt_scale,
                }),
            }
        }
        Shape::Fill { path, even_odd } => {
            let mut path = path.clone();
            if !id {
                path.apply_affine(xf);
            }
            Shape::Fill {
                path,
                even_odd: *even_odd,
            }
        }
        Shape::Text(t) => {
            let mut t = t.clone();
            t.transform = xf * t.transform;
            Shape::Text(t)
        }
        Shape::Point(p) => Shape::Point(xf * *p),
    }
}

/// The part of the line `o + t·d` (t ≥ 0 for a ray) inside `r`.
fn clip_line(o: Point, d: Vec2, ray: bool, r: Rect) -> Option<(Point, Point)> {
    let mut t0 = if ray { 0.0 } else { f64::NEG_INFINITY };
    let mut t1 = f64::INFINITY;
    for (p, dp, lo, hi) in [(o.x, d.x, r.x0, r.x1), (o.y, d.y, r.y0, r.y1)] {
        if dp.abs() < 1e-300 {
            if p < lo || p > hi {
                return None;
            }
        } else {
            let (a, b) = ((lo - p) / dp, (hi - p) / dp);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    (t0 < t1 && t0.is_finite() && t1.is_finite()).then(|| (o + d * t0, o + d * t1))
}

/// Bounds of one item (text by an estimate of its box: 0.6 × height per character).
pub(crate) fn item_bounds(s: &Shape) -> Option<Rect> {
    let b = match s {
        Shape::Stroke { path, .. } | Shape::Fill { path, .. } => {
            if path.elements().is_empty() {
                return None;
            }
            path.bounding_box()
        }
        Shape::Point(p) => Rect::from_points(*p, *p),
        Shape::Text(t) => return text_bounds(t),
    };
    (b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()).then_some(b)
}

fn text_bounds(t: &TextBlock) -> Option<Rect> {
    let mut lines: Vec<(f64, f64)> = Vec::new(); // (width, height)
    for p in &t.paragraphs {
        let mut w = 0.0f64;
        let mut h = 0.0f64;
        for s in &p.spans {
            for (k, part) in s.text.split('\n').enumerate() {
                if k > 0 {
                    lines.push((w, h));
                    w = 0.0;
                }
                w += part.chars().count() as f64 * 0.6 * s.height * s.width_factor;
                h = h.max(s.height);
            }
        }
        lines.push((w, h));
    }
    let mut width = lines.iter().map(|l| l.0).fold(0.0, f64::max);
    if t.wrap > 0.0 {
        width = width.min(t.wrap);
    }
    if let Some((len, _)) = t.fit {
        width = len;
    }
    let first = lines.first().map_or(0.0, |l| l.1);
    let height = first
        + lines
            .iter()
            .skip(1)
            .map(|l| l.1 * 1.667 * t.line_spacing)
            .sum::<f64>();
    if width <= 0.0 && height <= 0.0 {
        return None;
    }
    let x0 = match t.halign {
        HAlign::Left | HAlign::Fit => 0.0,
        HAlign::Center => -width / 2.0,
        HAlign::Right => -width,
    };
    let (y0, y1) = match t.valign {
        VAlign::Baseline | VAlign::Bottom if lines.len() <= 1 => (0.0, first),
        VAlign::Baseline | VAlign::Bottom => (0.0, height),
        VAlign::Middle => (-height / 2.0, height / 2.0),
        VAlign::Top => (-height, 0.0),
    };
    let corners = [
        Point::new(x0, y0),
        Point::new(x0 + width, y0),
        Point::new(x0, y1),
        Point::new(x0 + width, y1),
    ];
    bounds_of(corners.iter().map(|&c| t.transform * c))
}

/// A sheet's viewports with frozen-layer handles mapped to layer indices and the clip
/// entity looked up among the sheet's own items.
pub(crate) fn viewports(
    raw: Vec<(Rect, Affine, Vec<u64>, Option<u64>)>,
    layer_of_handle: &dyn Fn(u64) -> Option<u32>,
    clip_of_handle: &dyn Fn(u64) -> Option<BezPath>,
) -> Vec<Viewport> {
    raw.into_iter()
        .map(|(paper, transform, frozen, clip)| Viewport {
            paper,
            transform,
            frozen_layers: frozen.into_iter().filter_map(layer_of_handle).collect(),
            clip: clip.and_then(clip_of_handle),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xline_clips_both_ways_and_ray_one_way() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let (a, b) = clip_line(Point::new(5.0, 5.0), Vec2::new(1.0, 0.0), false, r).unwrap();
        assert_eq!((a.x, b.x), (0.0, 10.0));
        let (a, b) = clip_line(Point::new(5.0, 5.0), Vec2::new(1.0, 0.0), true, r).unwrap();
        assert_eq!((a.x, b.x), (5.0, 10.0));
        assert!(clip_line(Point::new(5.0, 20.0), Vec2::new(1.0, 0.0), false, r).is_none());
    }
}
