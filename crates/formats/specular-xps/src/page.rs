//! FixedPage markup → [`Page`]: elements, property elements, resources and markup
//! compatibility, read into the owned model with every reference resolved.

use std::collections::HashMap;
use std::sync::Arc;

use roxmltree::Node as X;
use vello_cpu::kurbo::{Affine, Point, Vec2};
use vello_cpu::peniko::Fill;

use crate::font::Font;
use crate::geom::{self, Builder, Geom};
use crate::model::*;
use crate::xml;
use crate::{Inner, RenderError};

/// Most elements read per page (and per remote dictionary).
pub const MAX_ELEMENTS: usize = 1_000_000;
/// Deepest element nesting followed (canvases, visual brushes in visual brushes…).
pub const MAX_DEPTH: u32 = 64;
/// Most glyphs in one Glyphs element.
pub const MAX_GLYPHS: usize = 200_000;
/// Most characters of page text kept.
const MAX_TEXT: usize = 4 << 20;

/// A resource of a dictionary.
#[derive(Clone, Debug)]
pub enum Res {
    Brush(Arc<Brush>),
    Geom(Arc<Geom>),
    Visual(Arc<Node>),
    Transform(Affine),
}

pub type Dict = HashMap<String, Res>;

enum Scope {
    Local(Dict),
    Shared(Arc<Dict>),
}

impl Scope {
    fn get(&self, k: &str) -> Option<&Res> {
        match self {
            Scope::Local(d) => d.get(k),
            Scope::Shared(d) => d.get(k),
        }
    }
}

pub struct Parser<'d> {
    doc: &'d Inner,
    /// The part URIs are relative to.
    base: String,
    scopes: Vec<Scope>,
    pub elements: usize,
    pub text: String,
    last_y: Option<f64>,
    cancelled: &'d dyn Fn() -> bool,
    stop: bool,
}

fn attr_f(n: X<'_, '_>, name: &str) -> Option<f64> {
    n.attribute(name)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn attr_bool(n: X<'_, '_>, name: &str, default: bool) -> bool {
    match n.attribute(name).map(str::trim) {
        Some(v) if v.eq_ignore_ascii_case("true") => true,
        Some(v) if v.eq_ignore_ascii_case("false") => false,
        _ => default,
    }
}

/// The first element child of the property element `<{owner}.{prop}>`.
fn prop<'a, 'i>(n: X<'a, 'i>, owner: &str, prop: &str) -> Option<X<'a, 'i>> {
    let want_len = owner.len() + 1 + prop.len();
    let p = n.children().find(|c| {
        let name = c.tag_name().name();
        c.is_element()
            && name.len() == want_len
            && name.starts_with(owner)
            && name.as_bytes()[owner.len()] == b'.'
            && name.ends_with(prop)
    })?;
    xml::children(p).into_iter().next()
}

fn cap(v: Option<&str>) -> Cap {
    match v.map(str::trim) {
        Some("Square") => Cap::Square,
        Some("Round") => Cap::Round,
        Some("Triangle") => Cap::Triangle,
        _ => Cap::Flat,
    }
}

fn opacity(n: X<'_, '_>) -> f32 {
    attr_f(n, "Opacity").map_or(1.0, |v| v.clamp(0.0, 1.0) as f32)
}

fn spread(n: X<'_, '_>) -> Spread {
    match n.attribute("SpreadMethod").map(str::trim) {
        Some("Reflect") => Spread::Reflect,
        Some("Repeat") => Spread::Repeat,
        _ => Spread::Pad,
    }
}

fn tile_mode(n: X<'_, '_>) -> TileMode {
    match n.attribute("TileMode").map(str::trim) {
        Some("Tile") => TileMode::Tile,
        Some("FlipX") => TileMode::FlipX,
        Some("FlipY") => TileMode::FlipY,
        Some("FlipXY") => TileMode::FlipXY,
        _ => TileMode::None,
    }
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// The colour of sorted `stops` at `t`.
fn at(stops: &[Stop], t: f32) -> Rgba {
    match stops.iter().position(|s| s.offset >= t) {
        None => stops.last().map_or([0.0; 4], |s| s.color),
        Some(0) => stops[0].color,
        Some(i) => {
            let (a, b) = (&stops[i - 1], &stops[i]);
            let span = b.offset - a.offset;
            if span <= 0.0 {
                b.color
            } else {
                lerp(a.color, b.color, (t - a.offset) / span)
            }
        }
    }
}

/// Sorts stops and fits them into 0..1 (colours at the ends interpolated).
pub fn normalise_stops(mut stops: Vec<Stop>) -> Vec<Stop> {
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    if stops.is_empty() {
        return stops;
    }
    let first = stops[0].offset;
    let last = stops[stops.len() - 1].offset;
    if first >= 0.0 && last <= 1.0 {
        return stops;
    }
    let c0 = at(&stops, 0.0);
    let c1 = at(&stops, 1.0);
    let mut out = vec![Stop {
        offset: 0.0,
        color: c0,
    }];
    out.extend(
        stops
            .iter()
            .filter(|s| s.offset > 0.0 && s.offset < 1.0)
            .cloned(),
    );
    out.push(Stop {
        offset: 1.0,
        color: c1,
    });
    out
}

impl<'d> Parser<'d> {
    pub fn new(doc: &'d Inner, base: &str, cancelled: &'d dyn Fn() -> bool) -> Parser<'d> {
        Parser {
            doc,
            base: base.to_owned(),
            scopes: Vec::new(),
            elements: 0,
            text: String::new(),
            last_y: None,
            cancelled,
            stop: false,
        }
    }

    fn lookup(&self, key: &str) -> Option<Res> {
        self.scopes.iter().rev().find_map(|s| s.get(key)).cloned()
    }

    fn count(&mut self) -> bool {
        self.elements += 1;
        if self.elements >= MAX_ELEMENTS {
            self.stop = true;
        }
        if self.elements.is_multiple_of(512) && (self.cancelled)() {
            self.stop = true;
        }
        !self.stop
    }

    pub fn was_cancelled(&self) -> bool {
        self.stop && (self.cancelled)()
    }

    /// Reads a page part's root `FixedPage`.
    pub fn page(&mut self, root: X<'_, '_>) -> Result<Page, RenderError> {
        if root.tag_name().name() != "FixedPage" {
            return Err(RenderError::Failed("page part is not a FixedPage".into()));
        }
        let pushed = self.resources(root, "FixedPage");
        let nodes = self.children(root, 0);
        self.pop(pushed);
        if self.was_cancelled() {
            return Err(RenderError::Cancelled);
        }
        Ok(Page {
            nodes,
            text: std::mem::take(&mut self.text),
        })
    }

    fn pop(&mut self, n: usize) {
        for _ in 0..n {
            self.scopes.pop();
        }
    }

    /// Pushes the scopes of `<{owner}.Resources>`; returns how many.
    fn resources(&mut self, n: X<'_, '_>, owner: &str) -> usize {
        let Some(dict) = prop(n, owner, "Resources") else {
            return 0;
        };
        if dict.tag_name().name() != "ResourceDictionary" {
            return 0;
        }
        if let Some(src) = dict.attribute("Source") {
            if let Some(part) = crate::opc::resolve(&self.base, src)
                && let Some(d) = self.doc.dict(&part, self.cancelled)
            {
                self.scopes.push(Scope::Shared(d));
                return 1;
            }
            return 0;
        }
        self.scopes.push(Scope::Local(Dict::new()));
        self.fill_dict(dict, 0);
        1
    }

    /// Reads the entries of a `ResourceDictionary` into the innermost (local) scope, so later
    /// entries can refer to earlier ones.
    pub fn fill_dict(&mut self, dict: X<'_, '_>, depth: u32) {
        for c in xml::children(dict) {
            if !self.count() {
                break;
            }
            let Some(k) = xml::key(c) else { continue };
            let res = match c.tag_name().name() {
                "SolidColorBrush"
                | "LinearGradientBrush"
                | "RadialGradientBrush"
                | "ImageBrush"
                | "VisualBrush" => self.brush_elem(c, depth + 1).map(Res::Brush),
                "PathGeometry" => self.geometry_elem(c).map(Res::Geom),
                "MatrixTransform" => c
                    .attribute("Matrix")
                    .and_then(geom::matrix)
                    .map(Res::Transform),
                "Canvas" | "Path" | "Glyphs" => {
                    self.node(c, depth + 1).map(|n| Res::Visual(Arc::new(n)))
                }
                _ => None,
            };
            if let Some(r) = res
                && let Some(Scope::Local(d)) = self.scopes.last_mut()
            {
                d.insert(k.to_owned(), r);
            }
        }
    }

    /// Starts an empty local scope (for reading a remote dictionary).
    pub fn push_local(&mut self) {
        self.scopes.push(Scope::Local(Dict::new()));
    }

    /// The local scope built, taken out.
    pub fn take_local(&mut self) -> Dict {
        match self.scopes.pop() {
            Some(Scope::Local(d)) => d,
            _ => Dict::new(),
        }
    }

    fn children(&mut self, n: X<'_, '_>, depth: u32) -> Vec<Node> {
        let mut out = Vec::new();
        if depth > MAX_DEPTH {
            return out;
        }
        for c in xml::children(n) {
            if self.stop {
                break;
            }
            if let Some(node) = self.node(c, depth) {
                out.push(node);
            }
        }
        out
    }

    fn node(&mut self, n: X<'_, '_>, depth: u32) -> Option<Node> {
        if depth > MAX_DEPTH || !self.count() {
            return None;
        }
        match n.tag_name().name() {
            "Canvas" => {
                let pushed = self.resources(n, "Canvas");
                let common = self.common(n, "Canvas", depth);
                let children = self.children(n, depth + 1);
                self.pop(pushed);
                Some(Node::Canvas(CanvasNode { common, children }))
            }
            "Path" => self.path(n, depth).map(Node::Path),
            "Glyphs" => self.glyphs(n, depth).map(Node::Glyphs),
            _ => None,
        }
    }

    fn transform_value(&self, v: &str) -> Option<Affine> {
        if let Some(k) = xml::static_resource(v) {
            return match self.lookup(k)? {
                Res::Transform(t) => Some(t),
                _ => None,
            };
        }
        geom::matrix(v)
    }

    /// A transform given as attribute `attr` or property element `<owner.prop>`.
    fn transform(&self, n: X<'_, '_>, attr: &str, owner: &str, p: &str) -> Affine {
        if let Some(t) = n.attribute(attr).and_then(|v| self.transform_value(v)) {
            return t;
        }
        prop(n, owner, p)
            .filter(|m| m.tag_name().name() == "MatrixTransform")
            .and_then(|m| m.attribute("Matrix"))
            .and_then(|v| self.transform_value(v))
            .unwrap_or(Affine::IDENTITY)
    }

    fn common(&mut self, n: X<'_, '_>, owner: &str, depth: u32) -> Common {
        let transform = self.transform(n, "RenderTransform", owner, "RenderTransform");
        let clip = match n.attribute("Clip") {
            Some(v) => self.geometry_value(v),
            None => prop(n, owner, "Clip").and_then(|g| self.geometry_elem(g)),
        };
        let mask = match n.attribute("OpacityMask") {
            Some(v) => self.brush_value(v),
            None => prop(n, owner, "OpacityMask").and_then(|b| self.brush_elem(b, depth + 1)),
        };
        Common {
            transform,
            clip,
            opacity: opacity(n),
            mask,
        }
    }

    fn geometry_value(&self, v: &str) -> Option<Arc<Geom>> {
        if let Some(k) = xml::static_resource(v) {
            return match self.lookup(k)? {
                Res::Geom(g) => Some(g),
                _ => None,
            };
        }
        Some(Arc::new(geom::parse_abbreviated(v)))
    }

    fn geometry_elem(&mut self, g: X<'_, '_>) -> Option<Arc<Geom>> {
        if g.tag_name().name() != "PathGeometry" {
            return None;
        }
        let rule = match g.attribute("FillRule").map(str::trim) {
            Some("NonZero") => Fill::NonZero,
            _ => Fill::EvenOdd,
        };
        let mut b = Builder::new();
        if let Some(f) = g.attribute("Figures") {
            geom::abbreviated(f, &mut b);
        }
        for fig in xml::children(g) {
            if fig.tag_name().name() != "PathFigure" || b.full() {
                continue;
            }
            let Some(start) = fig.attribute("StartPoint").and_then(geom::point) else {
                continue;
            };
            b.move_to(start, attr_bool(fig, "IsFilled", true));
            for seg in xml::children(fig) {
                if b.full() {
                    break;
                }
                b.set_stroked(attr_bool(seg, "IsStroked", true));
                match seg.tag_name().name() {
                    "ArcSegment" => {
                        let (Some(p), Some(size)) = (
                            seg.attribute("Point").and_then(geom::point),
                            seg.attribute("Size").and_then(geom::point),
                        ) else {
                            continue;
                        };
                        b.arc_to(
                            Vec2::new(size.x, size.y),
                            attr_f(seg, "RotationAngle").unwrap_or(0.0),
                            attr_bool(seg, "IsLargeArc", false),
                            seg.attribute("SweepDirection").map(str::trim) == Some("Clockwise"),
                            p,
                        );
                    }
                    "PolyLineSegment" => {
                        for p in geom::points(seg.attribute("Points").unwrap_or("")) {
                            b.line_to(p);
                        }
                    }
                    "PolyBezierSegment" => {
                        let pts = geom::points(seg.attribute("Points").unwrap_or(""));
                        for c in pts.chunks_exact(3) {
                            b.curve_to(c[0], c[1], c[2]);
                        }
                    }
                    "PolyQuadraticBezierSegment" => {
                        let pts = geom::points(seg.attribute("Points").unwrap_or(""));
                        for c in pts.chunks_exact(2) {
                            b.quad_to(c[0], c[1]);
                        }
                    }
                    _ => {}
                }
            }
            b.set_stroked(true);
            if attr_bool(fig, "IsClosed", false) {
                b.close();
            }
        }
        let t = self.transform(g, "Transform", "PathGeometry", "Transform");
        Some(Arc::new(b.finish(rule).transformed(t)))
    }

    fn brush_value(&self, v: &str) -> Option<Arc<Brush>> {
        if let Some(k) = xml::static_resource(v) {
            return match self.lookup(k)? {
                Res::Brush(b) => Some(b),
                _ => None,
            };
        }
        color(v).map(|c| Arc::new(Brush::Solid(c)))
    }

    fn stops(&self, n: X<'_, '_>, owner: &str) -> Vec<Stop> {
        let Some(list) = n
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == format!("{owner}.GradientStops"))
        else {
            return Vec::new();
        };
        let stops = xml::children(list)
            .into_iter()
            .filter(|s| s.tag_name().name() == "GradientStop")
            .filter_map(|s| {
                Some(Stop {
                    offset: attr_f(s, "Offset").unwrap_or(0.0) as f32,
                    color: s.attribute("Color").and_then(color)?,
                })
            })
            .take(4096)
            .collect();
        normalise_stops(stops)
    }

    fn tiling(n: X<'_, '_>) -> Option<Tiling> {
        let viewbox = n.attribute("Viewbox").and_then(geom::rect4)?;
        let viewport = n.attribute("Viewport").and_then(geom::rect4)?;
        Some(Tiling {
            viewbox,
            viewport,
            mode: tile_mode(n),
        })
    }

    fn brush_elem(&mut self, b: X<'_, '_>, depth: u32) -> Option<Arc<Brush>> {
        if depth > MAX_DEPTH {
            return None;
        }
        let name = b.tag_name().name();
        let linear_rgb = b.attribute("ColorInterpolationMode").map(str::trim)
            == Some("ScRgbLinearInterpolation");
        let brush = match name {
            "SolidColorBrush" => {
                let mut c = b.attribute("Color").and_then(color)?;
                c[3] *= opacity(b);
                Brush::Solid(c)
            }
            "LinearGradientBrush" => Brush::Linear {
                start: b.attribute("StartPoint").and_then(geom::point)?,
                end: b.attribute("EndPoint").and_then(geom::point)?,
                stops: self.stops(b, name),
                spread: spread(b),
                linear_rgb,
                transform: self.transform(b, "Transform", name, "Transform"),
                opacity: opacity(b),
            },
            "RadialGradientBrush" => Brush::Radial {
                center: b.attribute("Center").and_then(geom::point)?,
                origin: b.attribute("GradientOrigin").and_then(geom::point)?,
                rx: attr_f(b, "RadiusX")?,
                ry: attr_f(b, "RadiusY")?,
                stops: self.stops(b, name),
                spread: spread(b),
                linear_rgb,
                transform: self.transform(b, "Transform", name, "Transform"),
                opacity: opacity(b),
            },
            "ImageBrush" => {
                let src = b.attribute("ImageSource")?.trim();
                // `{ColorConvertedBitmap image profile}`: the image, shown without its profile.
                let uri = match src.strip_prefix('{') {
                    Some(inner) => inner
                        .trim_end_matches('}')
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or(""),
                    None => src,
                };
                Brush::Image {
                    part: crate::opc::resolve(&self.base, uri)?,
                    tiling: Self::tiling(b)?,
                    transform: self.transform(b, "Transform", name, "Transform"),
                    opacity: opacity(b),
                }
            }
            "VisualBrush" => {
                let visual = match b.attribute("Visual") {
                    Some(v) => match self.lookup(xml::static_resource(v)?)? {
                        Res::Visual(n) => n,
                        _ => return None,
                    },
                    None => {
                        let v = prop(b, name, "Visual")?;
                        Arc::new(self.node(v, depth + 1)?)
                    }
                };
                Brush::Visual {
                    visual,
                    tiling: Self::tiling(b)?,
                    transform: self.transform(b, "Transform", name, "Transform"),
                    opacity: opacity(b),
                }
            }
            _ => return None,
        };
        Some(Arc::new(brush))
    }

    /// A brush given as attribute `attr` or property element `<owner.attr>`.
    fn brush(&mut self, n: X<'_, '_>, owner: &str, attr: &str, depth: u32) -> Option<Arc<Brush>> {
        match n.attribute(attr) {
            Some(v) => self.brush_value(v),
            None => prop(n, owner, attr).and_then(|b| self.brush_elem(b, depth + 1)),
        }
    }

    fn path(&mut self, n: X<'_, '_>, depth: u32) -> Option<PathNode> {
        let data = match n.attribute("Data") {
            Some(v) => self.geometry_value(v)?,
            None => {
                let g = prop(n, "Path", "Data")?;
                self.geometry_elem(g)?
            }
        };
        let common = self.common(n, "Path", depth);
        let fill = self.brush(n, "Path", "Fill", depth);
        let stroke = self.brush(n, "Path", "Stroke", depth).map(|b| {
            let join = match n.attribute("StrokeLineJoin").map(str::trim) {
                Some("Bevel") => Join::Bevel,
                Some("Round") => Join::Round,
                _ => Join::Miter,
            };
            let style = StrokeStyle {
                width: attr_f(n, "StrokeThickness").unwrap_or(1.0).abs(),
                dash: n
                    .attribute("StrokeDashArray")
                    .map(geom::numbers)
                    .unwrap_or_default()
                    .into_iter()
                    .map(f64::abs)
                    .take(256)
                    .collect(),
                dash_offset: attr_f(n, "StrokeDashOffset").unwrap_or(0.0),
                dash_cap: cap(n.attribute("StrokeDashCap")),
                start_cap: cap(n.attribute("StrokeStartLineCap")),
                end_cap: cap(n.attribute("StrokeEndLineCap")),
                join,
                miter: attr_f(n, "StrokeMiterLimit").unwrap_or(10.0).max(1.0),
            };
            (b, style)
        });
        if fill.is_none() && stroke.is_none() {
            return None;
        }
        Some(PathNode {
            common,
            data,
            fill,
            stroke,
        })
    }

    fn glyphs(&mut self, n: X<'_, '_>, depth: u32) -> Option<GlyphsNode> {
        let em = attr_f(n, "FontRenderingEmSize")?.abs();
        let origin = Point::new(attr_f(n, "OriginX")?, attr_f(n, "OriginY")?);
        let text = n.attribute("UnicodeString").unwrap_or("");
        let text = text.strip_prefix("{}").unwrap_or(text);
        let uri = n.attribute("FontUri").unwrap_or("");
        let index = uri
            .split_once('#')
            .and_then(|(_, f)| f.trim().parse::<u32>().ok())
            .unwrap_or(0);
        let font = crate::opc::resolve(&self.base, uri)
            .and_then(|p| self.doc.font(&p, index))
            .or_else(Font::fallback)?;
        let rtl = n
            .attribute("BidiLevel")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0)
            % 2
            == 1;
        let sideways = attr_bool(n, "IsSideways", false);
        let sim = n.attribute("StyleSimulations").unwrap_or("None");
        let bold = sim.contains("Bold");
        let italic = sim.contains("Italic");
        let glyphs = layout(
            &font,
            em,
            origin,
            text,
            n.attribute("Indices").unwrap_or(""),
            rtl,
            sideways,
        );
        self.add_text(text, origin.y, em);
        let common = self.common(n, "Glyphs", depth);
        let fill = self.brush(n, "Glyphs", "Fill", depth);
        Some(GlyphsNode {
            common,
            font,
            em,
            glyphs,
            fill,
            bold,
            italic,
            sideways,
        })
    }

    fn add_text(&mut self, text: &str, y: f64, em: f64) {
        if text.trim().is_empty() || self.text.len() >= MAX_TEXT {
            return;
        }
        if let Some(last) = self.last_y {
            self.text.push(if (y - last).abs() > em * 0.5 {
                '\n'
            } else {
                ' '
            });
        }
        self.last_y = Some(y);
        self.text.push_str(text);
    }
}

/// One `Indices` entry: `[(chars[:glyphs])][gid][,[advance][,[u][,v]]]`.
#[derive(Debug, Default, PartialEq)]
struct Entry {
    cluster: Option<(usize, usize)>,
    gid: Option<u32>,
    advance: Option<f64>,
    u: Option<f64>,
    v: Option<f64>,
}

fn entry(s: &str) -> Entry {
    let mut e = Entry::default();
    let mut s = s.trim();
    if let Some(rest) = s.strip_prefix('(')
        && let Some((inside, after)) = rest.split_once(')')
    {
        let mut it = inside.split(':');
        let c = it.next().and_then(|v| v.trim().parse().ok()).unwrap_or(1);
        let g = it.next().and_then(|v| v.trim().parse().ok()).unwrap_or(1);
        e.cluster = Some((c, g));
        s = after;
    }
    let mut parts = s.split(',');
    let num = |v: Option<&str>| {
        v.and_then(|t| t.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    e.gid = parts.next().and_then(|t| t.trim().parse().ok());
    e.advance = num(parts.next());
    e.u = num(parts.next());
    e.v = num(parts.next());
    e
}

/// Lays out a Glyphs element: glyph ids and origins from `Indices`, falling back to the
/// font's character map and advances for whatever it leaves out.
pub fn layout(
    font: &Font,
    em: f64,
    origin: Point,
    text: &str,
    indices: &str,
    rtl: bool,
    sideways: bool,
) -> Vec<Placed> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let Some(face) = font.face() else {
        return Vec::new();
    };
    // The code point starting at unit `i`, and its length in units.
    let cp = |i: usize| -> Option<(u32, usize)> {
        let u = *units.get(i)?;
        if (0xD800..0xDC00).contains(&u)
            && let Some(&l) = units.get(i + 1)
            && (0xDC00..0xE000).contains(&l)
        {
            return Some((
                0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(l) - 0xDC00),
                2,
            ));
        }
        Some((u32::from(u), 1))
    };
    let entries: Vec<Entry> = if indices.trim().is_empty() {
        Vec::new()
    } else {
        indices.split(';').map(entry).collect()
    };
    let mut out = Vec::new();
    let mut pen = origin.x;
    let mut ci = 0usize;
    // Glyphs left in the current cluster, and the units it covers.
    let mut cluster_left = 0usize;
    let mut cluster_units = 0usize;
    let mut k = 0;
    let plain = Entry::default();
    loop {
        if out.len() >= MAX_GLYPHS {
            break;
        }
        let e = match entries.get(k) {
            Some(e) => e,
            None if ci < units.len() && cluster_left == 0 => &plain,
            None => break,
        };
        k += 1;
        let mut ch = None;
        if cluster_left == 0 {
            match e.cluster {
                Some((c, g)) => {
                    cluster_units = c;
                    cluster_left = g.max(1);
                    ch = cp(ci).map(|x| x.0);
                }
                None => {
                    let (c, len) = cp(ci).unwrap_or((0, 1));
                    ch = (ci < units.len()).then_some(c);
                    cluster_units = len;
                    cluster_left = 1;
                }
            }
        }
        let gid = e.gid.unwrap_or_else(|| ch.map_or(0, |c| face.glyph(c)));
        let advance = match e.advance {
            Some(a) => a * em / 100.0,
            // Sideways glyphs advance by their height: one em (no vertical metrics are read).
            None if sideways => em,
            None => face.advance(gid) * em,
        };
        let u = e.u.unwrap_or(0.0) * em / 100.0;
        let v = e.v.unwrap_or(0.0) * em / 100.0;
        if rtl {
            pen -= advance;
            out.push(Placed {
                gid,
                at: Point::new(pen - u, origin.y - v),
            });
        } else {
            out.push(Placed {
                gid,
                at: Point::new(pen + u, origin.y - v),
            });
            pen += advance;
        }
        cluster_left -= 1;
        if cluster_left == 0 {
            ci += cluster_units;
        }
        if k > entries.len() && ci >= units.len() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_entries() {
        assert_eq!(
            entry("(2:1)72,50.5,1,-2"),
            Entry {
                cluster: Some((2, 1)),
                gid: Some(72),
                advance: Some(50.5),
                u: Some(1.0),
                v: Some(-2.0)
            }
        );
        assert_eq!(
            entry(",30"),
            Entry {
                advance: Some(30.0),
                ..Entry::default()
            }
        );
    }

    #[test]
    fn glyph_layout() {
        let f = Font::new(specular_theme::fonts::INTER.to_vec(), 0).unwrap();
        let o = Point::new(10.0, 20.0);
        // Explicit advances, then defaults for the rest of the string.
        let g = layout(&f, 100.0, o, "AB C", ";,50;", false, false);
        assert_eq!(g.len(), 4);
        assert_eq!(g[0].at, o);
        assert!(g[1].at.x > 10.0);
        assert_eq!(g[2].at.x, g[1].at.x + 50.0);
        assert!(g.iter().all(|p| p.at.y == 20.0));
        // A ligature cluster: two characters, one glyph.
        let g = layout(&f, 10.0, o, "fi", "(2:1)5", false, false);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].gid, 5);
        // Right to left runs leftwards.
        let g = layout(&f, 10.0, o, "AB", "", true, false);
        assert!(g[0].at.x < 10.0 && g[1].at.x < g[0].at.x);
        // Indices without text.
        let g = layout(&f, 10.0, o, "", "3;4;5", false, false);
        assert_eq!(g.iter().map(|p| p.gid).collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn stops_fit() {
        let s = normalise_stops(vec![
            Stop {
                offset: 1.5,
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Stop {
                offset: -0.5,
                color: [0.0, 0.0, 0.0, 1.0],
            },
        ]);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].offset, 0.0);
        assert!((s[0].color[0] - 0.25).abs() < 1e-6);
        assert!((s[1].color[0] - 0.75).abs() < 1e-6);
    }
}
