//! Shapes of a page (`draw:*`) into display-list operations.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f64::consts::PI;
use std::sync::Arc;

use specular_drawpage::kurbo::{
    Affine, BezPath, Ellipse, Line, PathEl, Point, Rect, Shape, Stroke, Vec2,
};
use specular_drawpage::xml::roxmltree::Node;
use specular_drawpage::{
    Cap, HAlign, Image, ImageDecoder, Join, MAX_IMAGE_PIXELS, MAX_OPS, Op, Paint, Paragraph, Run,
    Stop, TextBlock, VAlign,
};

use crate::geometry::{self, Eval, Frame};
use crate::style::{ListLevel, Props, Resolved, Styles};
use crate::units::{angle_deg, color, length, percent};

/// Most shapes drawn per document.
pub const MAX_SHAPES: usize = 200_000;
/// Deepest group nesting followed.
const MAX_DEPTH: u32 = 64;
/// Most hatch lines per shape.
const MAX_HATCH_LINES: usize = 4096;
/// Points per 1/100 mm.
const HMM: f64 = 72.0 / 2540.0;
/// LibreOffice's default fill colour.
const DEFAULT_FILL: [u8; 4] = [0x72, 0x9f, 0xcf, 255];
/// LibreOffice's default line colour.
const DEFAULT_STROKE: [u8; 4] = [0x34, 0x65, 0xa4, 255];

/// Reads a package member (images) by its path.
pub type Member<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

pub struct Ctx<'a> {
    pub styles: &'a Styles,
    pub member: Member<'a>,
    pub decode: Option<&'a ImageDecoder>,
    pub hidden_layers: HashSet<String>,
    pub fields: Fields,
    images: HashMap<String, Option<Arc<Image>>>,
    /// Points per pixel of each image (its stored resolution, else 96 dpi).
    image_pt: HashMap<String, (f64, f64)>,
    image_pixels: u64,
    pub ops: Vec<Op>,
    pub not_drawn: BTreeMap<String, u64>,
    pub shapes: u64,
    pub images_drawn: u32,
    pub limited: bool,
}

/// Presentation page fields: what master-page footers, dates and numbers show.
#[derive(Clone, Debug, Default)]
pub struct Fields {
    pub page_number: usize,
    pub page_count: usize,
    pub footer: Option<String>,
    pub header: Option<String>,
    pub date_time: Option<String>,
    /// Classes of master shapes shown: "footer", "date-time", "page-number", "header".
    pub show: Vec<String>,
}

/// Text style state while walking spans.
#[derive(Clone, Debug)]
struct TextStyle {
    family: String,
    size: f64,
    bold: bool,
    italic: bool,
    color: [u8; 4],
    underline: bool,
    strike: bool,
    letter_spacing: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: "Liberation Sans".into(),
            size: 18.0,
            bold: false,
            italic: false,
            color: [0, 0, 0, 255],
            underline: false,
            strike: false,
            letter_spacing: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ParaStyle {
    align: HAlign,
    line_spacing: f64,
    line_height: Option<f64>,
    space_before: f64,
    space_after: f64,
    indent_left: f64,
    indent_right: f64,
    indent_first: f64,
}

fn apply_text(t: &mut TextStyle, p: &Props, styles: &Styles) {
    if let Some(n) = p.get("font-name") {
        t.family = styles.fonts.get(n).cloned().unwrap_or_else(|| n.clone());
    } else if let Some(f) = p.get("font-family") {
        t.family = f.trim().trim_matches(['\'', '"']).to_string();
    }
    if let Some(s) = p.get("font-size") {
        if let Some(pc) = s.trim().strip_suffix('%') {
            if let Ok(v) = pc.trim().parse::<f64>() {
                t.size *= v / 100.0;
            }
        } else if let Some(v) = length(s).filter(|v| *v > 0.0) {
            t.size = v;
        }
    }
    if let Some(w) = p.get("font-weight") {
        t.bold = w == "bold" || w.parse::<u32>().is_ok_and(|v| v >= 600);
    }
    if let Some(s) = p.get("font-style") {
        t.italic = s == "italic" || s == "oblique";
    }
    if let Some(c) = p.get("color").and_then(|c| color(c)) {
        t.color = c;
    }
    if p.get("use-window-font-color").is_some_and(|v| v == "true") && !p.contains_key("color") {
        t.color = [0, 0, 0, 255];
    }
    if let Some(u) = p.get("text-underline-style") {
        t.underline = u != "none";
    }
    if let Some(u) = p.get("text-line-through-style") {
        t.strike = u != "none";
    }
    if let Some(l) = p.get("letter-spacing") {
        t.letter_spacing = length(l).unwrap_or(0.0);
    }
}

fn apply_para(a: &mut ParaStyle, p: &Props) {
    if let Some(al) = p.get("text-align") {
        a.align = match al.as_str() {
            "center" => HAlign::Centre,
            "end" | "right" => HAlign::Right,
            "justify" => HAlign::Justify,
            _ => HAlign::Left,
        };
    }
    if let Some(lh) = p.get("line-height") {
        if lh.ends_with('%') {
            a.line_spacing = percent(lh).unwrap_or(1.0);
            a.line_height = None;
        } else if lh == "normal" {
            a.line_spacing = 1.0;
            a.line_height = None;
        } else if let Some(v) = length(lh) {
            a.line_height = Some(v);
        }
    }
    let len = |k: &str| p.get(k).and_then(|v| length(v));
    if let Some(v) = len("margin-top") {
        a.space_before = v;
    }
    if let Some(v) = len("margin-bottom") {
        a.space_after = v;
    }
    if let Some(v) = len("margin-left") {
        a.indent_left = v;
    }
    if let Some(v) = len("margin-right") {
        a.indent_right = v;
    }
    if let Some(v) = len("text-indent") {
        a.indent_first = v;
    }
}

fn attr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    specular_drawpage::xml::attr(n, name)
}

fn len_attr(n: Node, name: &str) -> Option<f64> {
    attr(n, name).and_then(length)
}

/// `draw:transform`: operations applied in the order written.
pub fn parse_transform(s: &str) -> Affine {
    let mut m = Affine::IDENTITY;
    let mut rest = s;
    for _ in 0..64 {
        let Some(open) = rest.find('(') else { break };
        let Some(close) = rest[open..].find(')').map(|c| open + c) else {
            break;
        };
        let name = rest[..open].trim().to_ascii_lowercase();
        let args: Vec<&str> = rest[open + 1..close]
            .split([' ', ','])
            .filter(|a| !a.is_empty())
            .collect();
        let num = |k: usize| args.get(k).and_then(|a| a.parse::<f64>().ok());
        let lenk = |k: usize| args.get(k).and_then(|a| length(a));
        let op = match name.as_str() {
            "rotate" => num(0).map(|a| Affine::rotate(-a)),
            "translate" => lenk(0).map(|x| Affine::translate((x, lenk(1).unwrap_or(0.0)))),
            "scale" => num(0).map(|x| Affine::scale_non_uniform(x, num(1).unwrap_or(x))),
            "skewx" => num(0).map(|a| Affine::skew(-a.tan(), 0.0)),
            "skewy" => num(0).map(|a| Affine::skew(0.0, -a.tan())),
            "matrix" => match (num(0), num(1), num(2), num(3), lenk(4), lenk(5)) {
                (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) => {
                    Some(Affine::new([a, b, c, d, e, f]))
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(op) = op
            && op.determinant().is_finite()
        {
            m = op * m;
        }
        rest = &rest[close + 1..];
    }
    m
}

/// `draw:points="x,y x,y …"`.
fn points(s: &str) -> Vec<Point> {
    s.split_whitespace()
        .take(1 << 20)
        .filter_map(|p| {
            let (x, y) = p.split_once(',')?;
            Some(Point::new(x.parse().ok()?, y.parse().ok()?))
        })
        .collect()
}

/// `svg:viewBox="x y w h"`.
fn view_box(n: Node) -> Option<Rect> {
    let v: Vec<f64> = attr(n, "viewBox")?
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    (v.len() == 4 && v[2] != 0.0 && v[3] != 0.0)
        .then(|| Rect::new(v[0], v[1], v[0] + v[2], v[1] + v[3]))
}

fn vb_to_box(vb: Rect, w: f64, h: f64) -> Affine {
    Affine::scale_non_uniform(w / vb.width(), h / vb.height()) * Affine::translate((-vb.x0, -vb.y0))
}

/// Points per pixel from an image's stored resolution (JPEG JFIF density, PNG pHYs), else
/// 96 dpi.
fn image_resolution(b: &[u8]) -> (f64, f64) {
    let dflt = (0.75, 0.75);
    let be16 = |o: usize| {
        b.get(o..o + 2)
            .map(|s| f64::from(u16::from_be_bytes([s[0], s[1]])))
    };
    let be32 = |o: usize| {
        b.get(o..o + 4)
            .map(|s| f64::from(u32::from_be_bytes([s[0], s[1], s[2], s[3]])))
    };
    let ok = |d: f64| (10.0..=10_000.0).contains(&d);
    if b.starts_with(&[0xff, 0xd8, 0xff, 0xe0]) && b.get(6..11) == Some(b"JFIF\0") {
        let unit = b.get(13).copied().unwrap_or(0);
        if let (Some(x), Some(y)) = (be16(14), be16(16)) {
            let (dx, dy) = match unit {
                1 => (x, y),
                2 => (x * 2.54, y * 2.54),
                _ => return dflt,
            };
            if ok(dx) && ok(dy) {
                return (72.0 / dx, 72.0 / dy);
            }
        }
        return dflt;
    }
    if b.starts_with(b"\x89PNG") {
        let mut at = 8;
        for _ in 0..64 {
            let (Some(len), Some(ty)) = (be32(at), b.get(at + 4..at + 8)) else {
                break;
            };
            if ty == b"pHYs" && b.get(at + 16) == Some(&1) {
                if let (Some(x), Some(y)) = (be32(at + 8), be32(at + 12)) {
                    let (dx, dy) = (x * 0.0254, y * 0.0254);
                    if ok(dx) && ok(dy) {
                        return (72.0 / dx, 72.0 / dy);
                    }
                }
                break;
            }
            if ty == b"IDAT" {
                break;
            }
            at += 12 + len as usize;
        }
    }
    dflt
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The geometry of a shape in its own box and where the box sits.
struct Placed {
    /// Box space → page.
    xf: Affine,
    w: f64,
    h: f64,
}

impl<'a> Ctx<'a> {
    pub fn new(styles: &'a Styles, member: Member<'a>, decode: Option<&'a ImageDecoder>) -> Self {
        Ctx {
            styles,
            member,
            decode,
            hidden_layers: HashSet::new(),
            images: HashMap::new(),
            image_pt: HashMap::new(),
            image_pixels: 0,
            ops: Vec::new(),
            not_drawn: BTreeMap::new(),
            shapes: 0,
            images_drawn: 0,
            limited: false,
            fields: Fields::default(),
        }
    }

    fn skip(&mut self, what: &str) {
        *self.not_drawn.entry(what.to_string()).or_default() += 1;
    }

    fn push(&mut self, op: Op) {
        if self.ops.len() < MAX_OPS {
            self.ops.push(op);
        } else {
            self.limited = true;
        }
    }

    fn image(&mut self, href: &str) -> Option<Arc<Image>> {
        if let Some(i) = self.images.get(href) {
            return i.clone();
        }
        let decode = self.decode?;
        let name = href.trim_start_matches("./");
        let bytes = (self.member)(name);
        if let Some(b) = &bytes {
            self.image_pt.insert(href.to_string(), image_resolution(b));
        }
        let img = bytes
            .and_then(|b| decode(&b))
            .and_then(|(w, h, px)| {
                let n = u64::from(w) * u64::from(h);
                if self.image_pixels + n > MAX_IMAGE_PIXELS {
                    self.limited = true;
                    return None;
                }
                self.image_pixels += n;
                Image::from_rgba(w, h, px)
            })
            .map(Arc::new);
        self.images.insert(href.to_string(), img.clone());
        img
    }

    fn image_of(&mut self, n: Node) -> Option<Arc<Image>> {
        if let Some(h) = attr(n, "href") {
            return self.image(h);
        }
        let data = n
            .children()
            .find(|c| c.tag_name().name() == "binary-data")?;
        let bytes = base64(&specular_drawpage::xml::all_text(data))?;
        let decode = self.decode?;
        let (w, h, px) = decode(&bytes)?;
        let px_n = u64::from(w) * u64::from(h);
        if self.image_pixels + px_n > MAX_IMAGE_PIXELS {
            self.limited = true;
            return None;
        }
        self.image_pixels += px_n;
        Image::from_rgba(w, h, px).map(Arc::new)
    }

    /// The graphic style of a shape (draw:style-name or presentation:style-name).
    fn shape_style(&self, n: Node) -> Resolved {
        let mut r = Resolved::default();
        self.styles.apply(&mut r, "graphic", None);
        if let Some(s) = n.attributes().find(|a| {
            a.name() == "style-name" && a.namespace().is_some_and(|ns| ns.contains("presentation"))
        }) {
            self.styles.apply(&mut r, "presentation", Some(s.value()));
        }
        if let Some(s) = n.attributes().find(|a| {
            a.name() == "style-name" && a.namespace().is_some_and(|ns| ns.contains("drawing"))
        }) {
            self.styles.apply(&mut r, "graphic", Some(s.value()));
        }
        r
    }

    fn placed(n: Node, parent: Affine) -> Placed {
        let x = len_attr(n, "x").unwrap_or(0.0);
        let y = len_attr(n, "y").unwrap_or(0.0);
        let w = len_attr(n, "width").unwrap_or(0.0).max(0.0);
        let h = len_attr(n, "height").unwrap_or(0.0).max(0.0);
        let t = attr(n, "transform").map_or(Affine::IDENTITY, parse_transform);
        Placed {
            xf: parent * t * Affine::translate((x, y)),
            w,
            h,
        }
    }

    /// Walks the shapes under `n` (a page, master page or group).
    pub fn walk(&mut self, n: Node, parent: Affine, depth: u32, master: bool) {
        if depth > MAX_DEPTH {
            self.limited = true;
            return;
        }
        for c in n.children().filter(Node::is_element) {
            if self.shapes as usize >= MAX_SHAPES {
                self.limited = true;
                return;
            }
            if let Some(l) = attr(c, "layer")
                && self.hidden_layers.contains(l)
            {
                continue;
            }
            if master
                && let Some(class) = c
                    .attributes()
                    .find(|a| a.name() == "class")
                    .map(|a| a.value())
            {
                // Title and outline placeholders are never drawn from the master; footers,
                // dates, page numbers and headers when the page shows them.
                if !self.fields.show.iter().any(|s| s == class) {
                    continue;
                }
            }
            if attr(c, "placeholder") == Some("true") {
                continue;
            }
            let name = c.tag_name().name();
            let ns = c.tag_name().namespace().unwrap_or("");
            if !ns.contains(":drawing:") && !ns.contains(":dr3d:") {
                if name == "notes" || name == "forms" || name == "animations" {
                    continue;
                }
                continue;
            }
            self.shapes += 1;
            match name {
                "g" => {
                    let t = attr(c, "transform").map_or(Affine::IDENTITY, parse_transform);
                    self.walk(c, parent * t, depth + 1, master);
                }
                "a" => self.walk(c, parent, depth + 1, master),
                "rect" => {
                    let p = Self::placed(c, parent);
                    let r = Rect::new(0.0, 0.0, p.w, p.h);
                    let rad = len_attr(c, "corner-radius").unwrap_or(0.0);
                    let path = if rad > 0.0 {
                        r.to_rounded_rect(rad.min(p.w / 2.0).min(p.h / 2.0))
                            .to_path(0.05)
                    } else {
                        r.to_path(0.05)
                    };
                    self.shape(c, &p, vec![(path, true, true)], None, false);
                }
                "ellipse" | "circle" => {
                    let p = Self::placed(c, parent);
                    let path = ellipse(c, p.w, p.h);
                    let closed = attr(c, "kind") != Some("arc");
                    self.shape(c, &p, vec![(path, closed, true)], None, false);
                }
                "line" | "connector" | "measure" => {
                    let mut path = BezPath::new();
                    let d = (name == "connector").then(|| attr(c, "d")).flatten();
                    if let Some(d) = d.and_then(|d| BezPath::from_svg(d).ok()) {
                        path = d;
                        path.apply_affine(Affine::scale(HMM));
                    } else {
                        let p0 = Point::new(
                            len_attr(c, "x1").unwrap_or(0.0),
                            len_attr(c, "y1").unwrap_or(0.0),
                        );
                        let p1 = Point::new(
                            len_attr(c, "x2").unwrap_or(0.0),
                            len_attr(c, "y2").unwrap_or(0.0),
                        );
                        path.move_to(p0);
                        path.line_to(p1);
                    }
                    let t = attr(c, "transform").map_or(Affine::IDENTITY, parse_transform);
                    let p = Placed {
                        xf: parent * t,
                        w: 0.0,
                        h: 0.0,
                    };
                    self.shape(c, &p, vec![(path, false, true)], None, false);
                }
                "polyline" | "polygon" => {
                    let p = Self::placed(c, parent);
                    let pts = points(attr(c, "points").unwrap_or(""));
                    let vb = view_box(c).unwrap_or(Rect::new(0.0, 0.0, p.w.max(1.0), p.h.max(1.0)));
                    let m = vb_to_box(vb, p.w, p.h);
                    let mut path = BezPath::new();
                    for (i, q) in pts.iter().enumerate() {
                        if i == 0 {
                            path.move_to(m * *q);
                        } else {
                            path.line_to(m * *q);
                        }
                    }
                    let closed = name == "polygon";
                    if closed && !pts.is_empty() {
                        path.close_path();
                    }
                    self.shape(c, &p, vec![(path, closed, true)], None, false);
                }
                "regular-polygon" => {
                    let p = Self::placed(c, parent);
                    let path = regular_polygon(c, p.w, p.h);
                    self.shape(c, &p, vec![(path, true, true)], None, false);
                }
                "path" => {
                    let p = Self::placed(c, parent);
                    let Some(mut path) = attr(c, "d").and_then(|d| BezPath::from_svg(d).ok())
                    else {
                        self.skip("path");
                        continue;
                    };
                    let vb = view_box(c).unwrap_or(Rect::new(0.0, 0.0, p.w.max(1.0), p.h.max(1.0)));
                    path.apply_affine(vb_to_box(vb, p.w, p.h));
                    let closed = path
                        .elements()
                        .iter()
                        .any(|e| matches!(e, PathEl::ClosePath));
                    self.shape(c, &p, vec![(path, closed, true)], None, false);
                }
                "custom-shape" => self.custom(c, parent),
                "frame" => self.frame(c, parent),
                "caption" => {
                    let p = Self::placed(c, parent);
                    let path = Rect::new(0.0, 0.0, p.w, p.h).to_path(0.05);
                    self.shape(c, &p, vec![(path, true, true)], None, false);
                }
                "page-thumbnail" => self.skip("page thumbnail"),
                "control" => self.skip("form control"),
                "scene" => self.skip("3D scene"),
                "glue-point" | "layer-set" => {}
                other => self.skip(other),
            }
        }
    }

    fn custom(&mut self, c: Node, parent: Affine) {
        let p = Self::placed(c, parent);
        let style = self.shape_style(c);
        let Some(geo) = c
            .children()
            .find(|g| g.tag_name().name() == "enhanced-geometry")
        else {
            let path = Rect::new(0.0, 0.0, p.w, p.h).to_path(0.05);
            self.shape(c, &p, vec![(path, true, true)], None, false);
            return;
        };
        if attr(geo, "text-path") == Some("true") {
            // Fontwork: text bent along the path; neither is drawn as such.
            self.skip("Fontwork text");
            return;
        }
        if attr(geo, "extrusion") == Some("true") {
            // Drawn flat: the front face only.
            self.skip("3D extrusion");
        }
        let vb = view_box(geo).unwrap_or(Rect::new(0.0, 0.0, 21600.0, 21600.0));
        let formulas: Vec<(String, String)> = geo
            .children()
            .filter(|e| e.tag_name().name() == "equation")
            .filter_map(|e| {
                Some((
                    attr(e, "name")?.to_string(),
                    attr(e, "formula")?.to_string(),
                ))
            })
            .collect();
        let modifiers: Vec<f64> = attr(geo, "modifiers")
            .unwrap_or("")
            .split_whitespace()
            .take(64)
            .map(|m| m.parse().unwrap_or(0.0))
            .collect();
        let frame = Frame {
            view: vb,
            log_w: p.w / HMM,
            log_h: p.h / HMM,
            has_fill: style.g("fill") != Some("none"),
            has_stroke: style.g("stroke") != Some("none"),
        };
        let mut ev = Eval::new(&formulas, &modifiers, frame);
        let mut to_box = vb_to_box(vb, p.w, p.h);
        let centre = Vec2::new(p.w / 2.0, p.h / 2.0);
        // LibreOffice mirrors on the page, across the lines through the shape's centre,
        // after rotating (which turns the rotation the other way); as a box-space
        // transform that is xf⁻¹ · flip · xf.
        let (fx, fy) = (
            attr(geo, "mirror-horizontal") == Some("true"),
            attr(geo, "mirror-vertical") == Some("true"),
        );
        let mut mirror = Affine::IDENTITY;
        if fx || fy {
            let k =
                Affine::scale_non_uniform(if fx { -1.0 } else { 1.0 }, if fy { -1.0 } else { 1.0 });
            let c = p.xf * centre.to_point();
            let flip = Affine::translate(c.to_vec2()) * k * Affine::translate(-c.to_vec2());
            let det = p.xf.determinant();
            mirror = if det.is_finite() && det.abs() > 1e-12 {
                p.xf.inverse() * flip * p.xf
            } else {
                Affine::translate(centre) * k * Affine::translate(-centre)
            };
        }
        to_box = mirror * to_box;
        let parts: Vec<(BezPath, bool, bool)> = match attr(geo, "enhanced-path") {
            Some(ep) => geometry::build(ep, &mut ev, to_box)
                .into_iter()
                .map(|part| (part.path, part.fill, part.stroke))
                .collect(),
            None => {
                let kind = attr(geo, "type").unwrap_or("rectangle");
                match geometry::preset(kind, p.w, p.h) {
                    Some(mut path) => {
                        path.apply_affine(mirror);
                        vec![(path, true, true)]
                    }
                    None => {
                        self.skip(&format!("custom shape \"{kind}\""));
                        vec![(Rect::new(0.0, 0.0, p.w, p.h).to_path(0.05), true, true)]
                    }
                }
            }
        };
        let area = attr(geo, "text-areas")
            .and_then(|a| geometry::text_area(a, &mut ev))
            .map(|r| vb_to_box(vb, p.w, p.h).transform_rect_bbox(r));
        self.shape(c, &p, parts, area, false);
    }

    fn frame(&mut self, c: Node, parent: Affine) {
        let p = Self::placed(c, parent);
        // The first child with content wins: an image (the first one that decodes: an SVG
        // usually comes with a PNG fallback after it), a text box, an object's replacement.
        let kids: Vec<Node> = c.children().filter(Node::is_element).collect();
        let mut drew = false;
        for k in &kids {
            match k.tag_name().name() {
                "table" => {
                    self.table(*k, &p);
                    drew = true;
                    break;
                }
                "text-box" => {
                    // The frame's style, with the text box's content.
                    let r = Rect::new(0.0, 0.0, p.w, p.h).to_path(0.05);
                    let style = self.shape_style(c);
                    self.paint_shape(&style, &p, &[(r, true, true)], true);
                    self.text(c, Some(*k), &style, &p, None, true);
                    drew = true;
                    break;
                }
                "image" => {
                    if let Some(img) = self.image_of(*k) {
                        let style = self.shape_style(c);
                        let r = Rect::new(0.0, 0.0, p.w, p.h).to_path(0.05);
                        self.paint_shape(&style, &p, &[(r, true, true)], true);
                        let (iw, ih) = (f64::from(img.width), f64::from(img.height));
                        let opacity = style
                            .g("image-opacity")
                            .or(style.g("opacity"))
                            .and_then(percent)
                            .unwrap_or(1.0) as f32;
                        self.push(Op::Image {
                            transform: p.xf * Affine::scale_non_uniform(p.w / iw, p.h / ih),
                            image: img,
                            opacity,
                        });
                        self.images_drawn += 1;
                        drew = true;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !drew {
            // Text boxes were handled above; an object or an image that did not decode.
            let what = if kids.iter().any(|k| k.tag_name().name() == "table") {
                "table"
            } else if kids
                .iter()
                .any(|k| k.tag_name().name() == "object" || k.tag_name().name() == "object-ole")
            {
                "embedded object"
            } else if kids.iter().any(|k| k.tag_name().name() == "image") {
                "image"
            } else {
                "frame"
            };
            self.skip(what);
        }
    }

    /// A table (`table:table` in a frame): columns, rows, cell fills from the cell styles and
    /// the table template, borders, and cell text.
    fn table(&mut self, t: Node, p: &Placed) {
        const MAX_CELLS: usize = 100_000;
        let mut widths: Vec<f64> = Vec::new();
        for c in t
            .children()
            .filter(|c| c.tag_name().name() == "table-column")
        {
            let n = attr(c, "number-columns-repeated")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .min(1024);
            let w = attr(c, "style-name")
                .map(|s| self.styles.apply_layout("table-column", s))
                .and_then(|l| l.get("column-width").and_then(|v| length(v)))
                .filter(|w| *w > 0.0);
            for _ in 0..n {
                if widths.len() < 1024 {
                    widths.push(w.unwrap_or(0.0));
                }
            }
        }
        let rows: Vec<Node> = t
            .descendants()
            .filter(|r| r.tag_name().name() == "table-row")
            .take(4096)
            .collect();
        let is_cell = |c: &Node| matches!(c.tag_name().name(), "table-cell" | "covered-table-cell");
        let ncols = rows
            .iter()
            .map(|r| r.children().filter(is_cell).count())
            .max()
            .unwrap_or(0)
            .max(widths.len())
            .min(1024);
        if ncols == 0 || rows.is_empty() {
            self.skip("table");
            return;
        }
        widths.resize(ncols, 0.0);
        let known: f64 = widths.iter().sum();
        let unknown = widths.iter().filter(|w| **w <= 0.0).count();
        if unknown > 0 {
            let each = ((p.w - known) / unknown as f64).max(p.w / ncols as f64 / 4.0);
            for w in &mut widths {
                if *w <= 0.0 {
                    *w = each;
                }
            }
        }
        let mut heights: Vec<f64> = rows
            .iter()
            .map(|r| {
                attr(*r, "style-name")
                    .map(|s| self.styles.apply_layout("table-row", s))
                    .and_then(|l| {
                        l.get("row-height")
                            .or(l.get("min-row-height"))
                            .and_then(|v| length(v))
                    })
                    .unwrap_or(0.0)
            })
            .collect();
        let known: f64 = heights.iter().sum();
        let unknown = heights.iter().filter(|h| **h <= 0.0).count();
        if unknown > 0 {
            let each = ((p.h - known) / unknown as f64).max(12.0);
            for h in &mut heights {
                if *h <= 0.0 {
                    *h = each;
                }
            }
        }
        let template: Vec<(String, String)> = attr(t, "template-name")
            .and_then(|n| self.styles.templates.get(n))
            .cloned()
            .unwrap_or_default();
        let flag = |k: &str| attr(t, k) == Some("true");
        let part = |name: &str| {
            template
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        };
        let nrows = rows.len();
        let col_x: Vec<f64> = widths
            .iter()
            .scan(0.0, |x, w| {
                let v = *x;
                *x += w;
                Some(v)
            })
            .collect();
        let row_y: Vec<f64> = heights
            .iter()
            .scan(0.0, |y, h| {
                let v = *y;
                *y += h;
                Some(v)
            })
            .collect();
        let mut cells = 0;
        let mut texts: Vec<(Node, Resolved, Placed)> = Vec::new();
        let mut borders: Vec<(BezPath, f64, [u8; 4])> = Vec::new();
        for (ri, row) in rows.iter().enumerate() {
            let row_default = attr(*row, "default-cell-style-name");
            let mut ci = 0;
            for cell in row.children().filter(is_cell) {
                let covered = cell.tag_name().name() == "covered-table-cell";
                let repeat = attr(cell, "number-columns-repeated")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 1024);
                for _ in 0..repeat {
                    if ci >= ncols || cells >= MAX_CELLS {
                        break;
                    }
                    cells += 1;
                    let this = ci;
                    ci += 1;
                    if covered {
                        continue;
                    }
                    let span = |k: &str, max: usize| {
                        attr(cell, k)
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .clamp(1, max)
                    };
                    let span_c = span("number-columns-spanned", ncols - this);
                    let span_r = span("number-rows-spanned", nrows - ri);
                    let x0 = col_x[this];
                    let x1 = col_x[this + span_c - 1] + widths[this + span_c - 1];
                    let y0 = row_y[ri];
                    let y1 = row_y[ri + span_r - 1] + heights[ri + span_r - 1];
                    // Style layers: template body, banding, first/last column, first/last
                    // row, then the cell's own (or the row's default) style.
                    let mut layers: Vec<String> = Vec::new();
                    layers.extend(part("body"));
                    if flag("use-banding-rows-styles") {
                        // Banding counts body rows: after the header row when it is styled.
                        let k = ri - usize::from(flag("use-first-row-styles") && ri > 0);
                        layers.extend(part(if k % 2 == 0 { "odd-rows" } else { "even-rows" }));
                    }
                    if flag("use-banding-columns-styles") {
                        let k = this - usize::from(flag("use-first-column-styles") && this > 0);
                        layers.extend(part(if k % 2 == 0 {
                            "odd-columns"
                        } else {
                            "even-columns"
                        }));
                    }
                    if flag("use-first-column-styles") && this == 0 {
                        layers.extend(part("first-column"));
                    }
                    if flag("use-last-column-styles") && this + span_c == ncols {
                        layers.extend(part("last-column"));
                    }
                    if flag("use-first-row-styles") && ri == 0 {
                        layers.extend(part("first-row"));
                    }
                    if flag("use-last-row-styles") && ri + span_r == nrows {
                        layers.extend(part("last-row"));
                    }
                    if let Some(s) = attr(cell, "style-name").or(row_default) {
                        layers.push(s.to_string());
                    }
                    let mut r = Resolved::default();
                    self.styles.apply(&mut r, "graphic", None);
                    // Text defaults stay; fill comes from the cell.
                    r.graphic.insert("fill".into(), "none".into());
                    for l in &layers {
                        self.styles.apply_cell(&mut r, l);
                    }
                    // Fills overlap their neighbours by a hair so no seams show between cells.
                    let mut path = Rect::new(x0, y0, x1 + 0.3, y1 + 0.3).to_path(0.05);
                    path.apply_affine(p.xf);
                    let bg = r
                        .g("background-color")
                        .filter(|c| *c != "transparent")
                        .and_then(color);
                    let fill = bg.or_else(|| {
                        (r.g("fill") == Some("solid"))
                            .then(|| r.g("fill-color").and_then(color).unwrap_or(DEFAULT_FILL))
                    });
                    if let Some(c) = fill {
                        self.push(Op::Fill {
                            path,
                            even_odd: false,
                            paint: Paint::Solid(c),
                        });
                    }
                    for (side, a, b) in [
                        ("border-top", Point::new(x0, y0), Point::new(x1, y0)),
                        ("border-bottom", Point::new(x0, y1), Point::new(x1, y1)),
                        ("border-left", Point::new(x0, y0), Point::new(x0, y1)),
                        ("border-right", Point::new(x1, y0), Point::new(x1, y1)),
                    ] {
                        if let Some((w, c)) = r
                            .g(side)
                            .or(r.g("border"))
                            .or(r.paragraph.get(side).map(String::as_str))
                            .or(r.paragraph.get("border").map(String::as_str))
                            .and_then(border)
                        {
                            let mut l = BezPath::new();
                            l.move_to(p.xf * a);
                            l.line_to(p.xf * b);
                            borders.push((l, w, c));
                        }
                    }
                    texts.push((
                        cell,
                        r,
                        Placed {
                            xf: p.xf * Affine::translate((x0, y0)),
                            w: x1 - x0,
                            h: y1 - y0,
                        },
                    ));
                }
            }
        }
        for (path, w, c) in borders {
            self.push(Op::Stroke {
                path,
                stroke: Stroke::new(w),
                paint: Paint::Solid(c),
            });
        }
        for (cell, r, cp) in texts {
            self.text(cell, None, &r, &cp, None, true);
        }
    }

    /// Fills, strokes and the text of a shape. `parts`: (path in box space, filled,
    /// stroked).
    fn shape(
        &mut self,
        n: Node,
        p: &Placed,
        parts: Vec<(BezPath, bool, bool)>,
        text_area: Option<Rect>,
        frame: bool,
    ) {
        if parts.is_empty() {
            return;
        }
        let style = self.shape_style(n);
        self.paint_shape(&style, p, &parts, frame);
        self.text(n, None, &style, p, text_area, frame);
    }

    fn paint_shape(
        &mut self,
        style: &Resolved,
        p: &Placed,
        parts: &[(BezPath, bool, bool)],
        frame: bool,
    ) {
        let page_parts: Vec<(BezPath, bool, bool)> = parts
            .iter()
            .map(|(path, f, s)| {
                let mut q = path.clone();
                q.apply_affine(p.xf);
                (q, *f, *s)
            })
            .collect();
        let fill_mode = match style.g("fill") {
            Some(f) => f.to_string(),
            None if frame => "none".into(),
            None => "solid".into(),
        };
        let stroke = self.stroke_of(style, p);
        // Shadow under the shape.
        if style.g("shadow") == Some("visible") {
            let dx = style
                .g("shadow-offset-x")
                .and_then(length)
                .unwrap_or(0.2 * 72.0 / 2.54);
            let dy = style
                .g("shadow-offset-y")
                .and_then(length)
                .unwrap_or(0.2 * 72.0 / 2.54);
            let mut c = style
                .g("shadow-color")
                .and_then(color)
                .unwrap_or([128, 128, 128, 255]);
            let o = style.g("shadow-opacity").and_then(percent).unwrap_or(1.0)
                * style.g("opacity").and_then(percent).unwrap_or(1.0);
            c[3] = (o.clamp(0.0, 1.0) * f64::from(c[3])).round() as u8;
            let t = Affine::translate((dx, dy));
            for (path, f, s) in &page_parts {
                let mut q = path.clone();
                q.apply_affine(t);
                if *f && fill_mode != "none" {
                    self.push(Op::Fill {
                        path: q.clone(),
                        even_odd: true,
                        paint: Paint::Solid(c),
                    });
                }
                if *s && let Some((st, _)) = &stroke {
                    self.push(Op::Stroke {
                        path: q,
                        stroke: st.clone(),
                        paint: Paint::Solid(c),
                    });
                }
            }
        }
        let opacity = style.g("opacity").and_then(percent).unwrap_or(1.0);
        for (i, (path, f, _)) in page_parts.iter().enumerate() {
            if !*f || fill_mode == "none" {
                continue;
            }
            // Gradients and bitmaps span the shape's box; each part of a multi-part custom
            // shape gets its own (as LibreOffice draws them).
            let bbox_local = if parts.len() == 1 && p.w > 0.0 && p.h > 0.0 {
                Rect::new(0.0, 0.0, p.w, p.h)
            } else {
                parts[i].0.bounding_box()
            };
            let mut paint = self.fill_paint(style, &fill_mode, p, bbox_local);
            let mut opacity = opacity;
            // A transparency gradient: folded into the paint's alpha where the geometry
            // allows (solid fills, gradients of the same kind and angle), else its mean.
            if let Some(og) = style
                .g("opacity-name")
                .and_then(|n| self.styles.opacities.get(n))
                && let Some(pt) = paint.take()
            {
                let (folded, mean) = fold_opacity(pt, &og.attrs, p.xf, bbox_local, style);
                paint = Some(folded);
                opacity *= mean;
            }
            let wrap = opacity < 0.999;
            if wrap {
                self.push(Op::PushOpacity(opacity.clamp(0.0, 1.0) as f32));
            }
            if fill_mode == "hatch" {
                self.hatch(style, p, path, bbox_local);
            } else if let Some(paint) = paint {
                self.push(Op::Fill {
                    path: path.clone(),
                    even_odd: true,
                    paint,
                });
            }
            if wrap {
                self.push(Op::Pop);
            }
        }
        if let Some((st, c)) = stroke {
            for (path, _, s) in &page_parts {
                if *s {
                    self.push(Op::Stroke {
                        path: path.clone(),
                        stroke: st.clone(),
                        paint: Paint::Solid(c),
                    });
                }
            }
            // Markers at the ends of the first and last subpaths.
            if let Some(first) = page_parts.iter().find(|q| q.2) {
                self.markers(style, &first.0, &st, c, true);
            }
            if let Some(last) = page_parts.iter().rev().find(|q| q.2) {
                self.markers(style, &last.0, &st, c, false);
            }
        }
    }

    fn stroke_of(&self, style: &Resolved, p: &Placed) -> Option<(Stroke, [u8; 4])> {
        let mode = style.g("stroke").unwrap_or("solid");
        if mode == "none" {
            return None;
        }
        let _ = p;
        let w = style
            .g("stroke-width")
            .and_then(length)
            .unwrap_or(0.0)
            .max(0.0);
        let mut c = style
            .g("stroke-color")
            .and_then(color)
            .unwrap_or(DEFAULT_STROKE);
        if let Some(o) = style.g("stroke-opacity").and_then(percent) {
            c[3] = (o.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        let mut st = Stroke::new(w);
        st.join = match style.g("stroke-linejoin") {
            Some("round") => Join::Round,
            Some("bevel") => Join::Bevel,
            _ => Join::Miter,
        };
        let cap = match style.g("stroke-linecap") {
            Some("round") => Cap::Round,
            Some("square") => Cap::Square,
            _ => Cap::Butt,
        };
        st.start_cap = cap;
        st.end_cap = cap;
        if mode == "dash"
            && let Some(d) = style
                .g("stroke-dash")
                .and_then(|n| self.styles.dashes.get(n))
        {
            let lw = w.max(0.5);
            let seg = |k: &str| -> Option<f64> {
                let v = d.attrs.get(k)?;
                if v.ends_with('%') {
                    percent(v).map(|f| f * lw)
                } else {
                    length(v)
                }
            };
            let n1 = d
                .attrs
                .get("dots1")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .min(32);
            let n2 = d
                .attrs
                .get("dots2")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0)
                .min(32);
            let l1 = seg("dots1-length").filter(|v| *v > 0.0).unwrap_or(lw);
            let l2 = seg("dots2-length").filter(|v| *v > 0.0).unwrap_or(lw);
            let gap = seg("distance").filter(|v| *v > 0.0).unwrap_or(lw);
            let mut pat = Vec::new();
            for _ in 0..n1 {
                pat.push(l1);
                pat.push(gap);
            }
            for _ in 0..n2 {
                pat.push(l2);
                pat.push(gap);
            }
            if d.attrs.get("style").is_some_and(|s| s == "round") {
                st.start_cap = Cap::Round;
                st.end_cap = Cap::Round;
            }
            if pat.iter().sum::<f64>() > 1e-6 {
                st = st.with_dashes(0.0, pat);
            }
        }
        Some((st, c))
    }

    fn markers(&mut self, style: &Resolved, path: &BezPath, st: &Stroke, c: [u8; 4], start: bool) {
        let key = if start { "marker-start" } else { "marker-end" };
        let Some(name) = style.g(key) else { return };
        // A marker the document names but does not define (LibreOffice's built-in arrows):
        // a plain arrowhead.
        let fallback;
        let m = match self.styles.markers.get(name) {
            Some(m) => m,
            None if name.to_ascii_lowercase().contains("arrow") => {
                let mut f = crate::style::Named::default();
                f.attrs.insert("viewBox".into(), "0 0 20 30".into());
                f.attrs.insert("d".into(), "M10 0l-10 30h20z".into());
                fallback = f;
                &fallback
            }
            None => return,
        };
        let Some(mut shape) = m.attrs.get("d").and_then(|d| BezPath::from_svg(d).ok()) else {
            return;
        };
        let vb: Vec<f64> = m
            .attrs
            .get("viewBox")
            .map(|v| {
                v.split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        let vb = if vb.len() == 4 && vb[2] > 0.0 && vb[3] > 0.0 {
            Rect::new(vb[0], vb[1], vb[0] + vb[2], vb[1] + vb[3])
        } else {
            shape.bounding_box()
        };
        if vb.width() <= 0.0 {
            return;
        }
        let wkey = if start {
            "marker-start-width"
        } else {
            "marker-end-width"
        };
        let mw = style
            .g(wkey)
            .and_then(length)
            .unwrap_or(st.width * 3.0 + 3.0)
            .max(0.1);
        let k = mw / vb.width();
        // Tip and direction at the end.
        let els = path.elements();
        let pts: Vec<Point> = els
            .iter()
            .filter_map(|e| match e {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(*p),
                PathEl::QuadTo(_, b) => Some(*b),
                PathEl::CurveTo(_, _, p) => Some(*p),
                PathEl::ClosePath => None,
            })
            .collect();
        if pts.len() < 2 {
            return;
        }
        // Direction from the neighbouring control point.
        let (tip, from) = if start {
            let tip = pts[0];
            let next = match els.get(1) {
                Some(PathEl::CurveTo(a, _, _)) | Some(PathEl::QuadTo(a, _)) => *a,
                _ => pts[1],
            };
            (tip, next)
        } else {
            let tip = pts[pts.len() - 1];
            let prev = match els.last() {
                Some(PathEl::CurveTo(_, b, _)) => *b,
                Some(PathEl::QuadTo(a, _)) => *a,
                _ => pts[pts.len() - 2],
            };
            (tip, prev)
        };
        let d = tip - from;
        if d.hypot() < 1e-9 {
            return;
        }
        // Marker space: tip at (centre x, min y), pointing up (−y). Rotate so −y points
        // along `d`.
        let angle = d.y.atan2(d.x) + PI / 2.0;
        let centre = m
            .attrs
            .get(if start {
                "marker-start-center"
            } else {
                "marker-end-center"
            })
            .is_some_and(|v| v == "true");
        let anchor = Point::new(vb.center().x, if centre { vb.center().y } else { vb.y0 });
        shape.apply_affine(
            Affine::translate(tip.to_vec2())
                * Affine::rotate(angle)
                * Affine::scale(k)
                * Affine::translate(-anchor.to_vec2()),
        );
        self.push(Op::Fill {
            path: shape,
            even_odd: false,
            paint: Paint::Solid(c),
        });
    }

    fn fill_paint(
        &mut self,
        style: &Resolved,
        mode: &str,
        p: &Placed,
        bbox: Rect,
    ) -> Option<Paint> {
        let solid = || {
            Paint::Solid(
                style
                    .g("fill-color")
                    .and_then(color)
                    .unwrap_or(DEFAULT_FILL),
            )
        };
        match mode {
            "solid" => Some(solid()),
            "gradient" => {
                let g = style
                    .g("fill-gradient-name")
                    .and_then(|n| self.styles.gradients.get(n));
                match g {
                    Some(g) => Some(gradient(&g.attrs, &g.stops, p.xf, bbox)),
                    None => Some(solid()),
                }
            }
            "bitmap" => {
                let href = style
                    .g("fill-image-name")
                    .and_then(|n| self.styles.fill_images.get(n))
                    .and_then(|f| f.attrs.get("href").cloned());
                let img = href.as_ref().and_then(|h| self.image(h));
                let Some(img) = img else {
                    return Some(solid());
                };
                let (iw, ih) = (f64::from(img.width), f64::from(img.height));
                let (kx, ky) = href
                    .as_ref()
                    .and_then(|h| self.image_pt.get(h))
                    .copied()
                    .unwrap_or((0.75, 0.75));
                let repeat = style.g("repeat").unwrap_or("repeat");
                let m = if repeat == "stretch" {
                    Affine::translate((bbox.x0, bbox.y0))
                        * Affine::scale_non_uniform(bbox.width() / iw, bbox.height() / ih)
                } else {
                    let tw = style
                        .g("fill-image-width")
                        .and_then(length)
                        .filter(|v| *v > 0.0)
                        .unwrap_or(iw * kx);
                    let th = style
                        .g("fill-image-height")
                        .and_then(length)
                        .filter(|v| *v > 0.0)
                        .unwrap_or(ih * ky);
                    // The tiling is anchored at the reference point, offset by fractions of
                    // a tile.
                    let rp = style.g("fill-image-ref-point").unwrap_or("top-left");
                    let fx = if rp.ends_with("right") {
                        1.0
                    } else if rp.ends_with("left") {
                        0.0
                    } else {
                        0.5
                    };
                    let fy = if rp.starts_with("top") {
                        0.0
                    } else if rp.starts_with("bottom") {
                        1.0
                    } else {
                        0.5
                    };
                    let ox = style
                        .g("fill-image-ref-point-x")
                        .and_then(percent)
                        .unwrap_or(0.0);
                    let oy = style
                        .g("fill-image-ref-point-y")
                        .and_then(percent)
                        .unwrap_or(0.0);
                    let x0 = bbox.x0 + bbox.width() * fx - tw * fx + tw * ox;
                    let y0 = bbox.y0 + bbox.height() * fy - th * fy + th * oy;
                    Affine::translate((x0, y0)) * Affine::scale_non_uniform(tw / iw, th / ih)
                };
                Some(Paint::Image {
                    image: img,
                    transform: p.xf * m,
                    repeat: repeat == "repeat",
                })
            }
            "hatch" => None,
            _ => Some(solid()),
        }
    }

    fn hatch(&mut self, style: &Resolved, p: &Placed, path: &BezPath, bbox: Rect) {
        if style.g("fill-hatch-solid") == Some("true") {
            self.push(Op::Fill {
                path: path.clone(),
                even_odd: true,
                paint: Paint::Solid(
                    style
                        .g("fill-color")
                        .and_then(color)
                        .unwrap_or([255, 255, 255, 255]),
                ),
            });
        }
        let Some(h) = style
            .g("fill-hatch-name")
            .and_then(|n| self.styles.hatches.get(n))
        else {
            return;
        };
        let c = h
            .attrs
            .get("color")
            .and_then(|c| color(c))
            .unwrap_or([0, 0, 0, 255]);
        let dist = h
            .attrs
            .get("distance")
            .and_then(|d| length(d))
            .filter(|d| *d > 0.05)
            .unwrap_or(5.0);
        let base = h
            .attrs
            .get("rotation")
            .map(|a| gradient_angle(a))
            .unwrap_or(0.0);
        let n = match h.attrs.get("style").map(String::as_str) {
            Some("double") => 2,
            Some("triple") => 3,
            _ => 1,
        };
        let mut lines = BezPath::new();
        let centre = bbox.center();
        let r = (bbox.width().hypot(bbox.height())) / 2.0 + dist;
        let mut count = 0;
        for k in 0..n {
            let a = (base + [0.0, 90.0, 45.0][k]).to_radians();
            let dir = Vec2::new(a.cos(), -a.sin());
            let nrm = Vec2::new(-dir.y, dir.x);
            let steps = (r / dist).ceil() as i64;
            for s in -steps..=steps {
                if count >= MAX_HATCH_LINES {
                    break;
                }
                count += 1;
                let o = centre + nrm * (s as f64 * dist);
                let l = Line::new(o - dir * r, o + dir * r);
                lines.move_to(p.xf * l.p0);
                lines.line_to(p.xf * l.p1);
            }
        }
        self.push(Op::PushClip {
            path: path.clone(),
            even_odd: true,
        });
        self.push(Op::Stroke {
            path: lines,
            stroke: Stroke::new(0.0),
            paint: Paint::Solid(c),
        });
        self.push(Op::Pop);
    }

    /// The text of a shape (its `text:p`/`text:list` children, or those of `content`).
    fn text(
        &mut self,
        shape: Node,
        content: Option<Node>,
        style: &Resolved,
        p: &Placed,
        area: Option<Rect>,
        frame: bool,
    ) {
        let holder = content.unwrap_or(shape);
        if !holder
            .children()
            .any(|c| matches!(c.tag_name().name(), "p" | "h" | "list"))
        {
            return;
        }
        // Base text and paragraph styles: the graphic chain, then draw:text-style-name.
        let mut ts = TextStyle::default();
        let mut ps = ParaStyle {
            line_spacing: 1.0,
            ..ParaStyle::default()
        };
        apply_text(&mut ts, &style.text, self.styles);
        apply_para(&mut ps, &style.paragraph);
        if let Some(n) = attr(shape, "text-style-name") {
            let mut r = Resolved::default();
            self.styles.apply_text(&mut r, "paragraph", Some(n));
            apply_text(&mut ts, &r.text, self.styles);
            apply_para(&mut ps, &r.paragraph);
        }
        let mut paras = Vec::new();
        let list = n_list(self.styles, shape);
        self.paragraphs(holder, &ts, &ps, 0, list, &mut None, &mut paras);
        if paras
            .iter()
            .all(|q: &Paragraph| q.runs.iter().all(|r| r.text.trim().is_empty()))
        {
            return;
        }
        let pad = |k: &str| {
            style
                .g(k)
                .or(style.g("padding"))
                .and_then(length)
                .unwrap_or(if frame { 0.0 } else { 0.25 * 72.0 / 2.54 })
        };
        let mut r = area.unwrap_or(Rect::new(0.0, 0.0, p.w, p.h));
        if p.w <= 0.0 && p.h <= 0.0 {
            // Lines: text is not drawn.
            self.skip("text on a line");
            return;
        }
        r = Rect::new(
            r.x0 + pad("padding-left"),
            r.y0 + pad("padding-top"),
            r.x1 - pad("padding-right"),
            r.y1 - pad("padding-bottom"),
        );
        let valign = match style
            .g("textarea-vertical-align")
            .or(style.g("vertical-align"))
        {
            Some("middle") => VAlign::Middle,
            Some("bottom") => VAlign::Bottom,
            Some("top") | Some("justify") => VAlign::Top,
            _ if frame => VAlign::Top,
            _ => VAlign::Middle,
        };
        let mut t = TextBlock::new(
            p.xf * Affine::translate((r.x0, r.y0)),
            r.width().max(1.0),
            r.height().max(0.0),
        );
        t.valign = valign;
        // Word wrap is a custom-shape setting; other text grows sideways only when asked.
        t.wrap = if shape.tag_name().name() == "custom-shape" {
            style.g("wrap-option") != Some("no-wrap")
        } else {
            style.g("auto-grow-width") != Some("true")
        };
        t.paragraphs = paras;
        let fit = style.g("fit-to-size").unwrap_or("false");
        if matches!(fit, "true" | "all") {
            // Fit to frame: the text, unwrapped, stretched to fill the box.
            let (w, h) = natural_size(&t.paragraphs, None);
            if w > 0.0 && h > 0.0 {
                let (bw, bh) = (t.width, t.height.max(1.0));
                t.transform *= Affine::scale_non_uniform(bw / w, bh / h);
                t.width = w;
                t.height = h;
                t.wrap = false;
                t.valign = VAlign::Top;
            }
        } else if matches!(fit, "shrink-to-fit" | "autofit")
            || style.g("shrink-to-fit") == Some("true")
        {
            // Shrink on overflow: the largest scale (down to 1/10) whose wrapped text fits.
            let need = |k: f64| natural_size(&t.paragraphs, Some((t.width, k))).1 * k;
            if need(1.0) > t.height {
                let (mut lo, mut hi) = (0.1, 1.0);
                for _ in 0..12 {
                    let mid = (lo + hi) / 2.0;
                    if need(mid) > t.height {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                for p in &mut t.paragraphs {
                    p.space_before *= lo;
                    p.space_after *= lo;
                    if let Some(h) = &mut p.line_height {
                        *h *= lo;
                    }
                    for r in &mut p.runs {
                        r.size *= lo;
                        r.letter_spacing *= lo;
                    }
                }
            }
        }
        let cols = style
            .g("column-count")
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 16);
        if cols > 1 && t.wrap && t.height > 1.0 {
            // Columns: the text flows down one column-wide strip; column k shows its k-th
            // box-high slice.
            let gap = style
                .g("column-gap")
                .and_then(length)
                .unwrap_or(0.0)
                .max(0.0);
            let cw = ((t.width - gap * (cols - 1) as f64) / cols as f64).max(1.0);
            let h = t.height;
            let base = t.transform;
            for k in 0..cols {
                let x = k as f64 * (cw + gap);
                let mut clip = Rect::new(x, 0.0, x + cw, h).to_path(0.05);
                clip.apply_affine(base);
                self.push(Op::PushClip {
                    path: clip,
                    even_odd: false,
                });
                let mut tk = t.clone();
                tk.width = cw;
                tk.valign = VAlign::Top;
                tk.transform = base * Affine::translate((x, -(k as f64) * h));
                self.push(Op::Text(Box::new(tk)));
                self.push(Op::Pop);
            }
            return;
        }
        self.push(Op::Text(Box::new(t)));
    }

    #[allow(clippy::too_many_arguments)]
    fn paragraphs(
        &self,
        holder: Node,
        ts: &TextStyle,
        ps: &ParaStyle,
        level: usize,
        list: Option<&Vec<ListLevel>>,
        label: &mut Option<String>,
        out: &mut Vec<Paragraph>,
    ) {
        for c in holder.children().filter(Node::is_element) {
            if out.len() > 10_000 {
                return;
            }
            match c.tag_name().name() {
                "p" | "h" => {
                    let mut ts2 = ts.clone();
                    let mut ps2 = ps.clone();
                    if let Some(n) = attr(c, "style-name") {
                        for s in self.own_chain("paragraph", n) {
                            apply_text(&mut ts2, &s.0, self.styles);
                            apply_para(&mut ps2, &s.1);
                        }
                    }
                    let mut runs = Vec::new();
                    self.runs(c, &ts2, &mut runs, 0);
                    let mut q = Paragraph {
                        runs,
                        align: ps2.align,
                        line_spacing: ps2.line_spacing,
                        line_height: ps2.line_height,
                        space_before: ps2.space_before,
                        space_after: ps2.space_after,
                        indent_left: ps2.indent_left,
                        indent_right: ps2.indent_right,
                        indent_first: ps2.indent_first,
                        bullet: None,
                    };
                    if level > 0 {
                        let lv = list.and_then(|l| l.get(level - 1));
                        let (before, width) = match lv {
                            Some(lv) => (lv.space_before, lv.label_width),
                            None => (0.6 * 72.0 / 2.54 * (level - 1) as f64, 0.6 * 72.0 / 2.54),
                        };
                        q.indent_left += before + width;
                        q.indent_first = 0.0;
                        if let Some(lab) = label.take().filter(|b| !b.is_empty()) {
                            // The label as runs in front, its last character spaced out so the
                            // text starts at the label width (the list's tab position).
                            let base = q.runs.first().cloned().unwrap_or_else(|| run_of("", &ts2));
                            let rel = lv.map_or(1.0, |l| {
                                if l.number.is_some() {
                                    1.0
                                } else {
                                    l.size.clamp(0.2, 2.0)
                                }
                            });
                            let mut r = base.clone();
                            r.size = base.size * rel;
                            r.underline = false;
                            r.strike = false;
                            r.letter_spacing = 0.0;
                            let mut chars: Vec<char> = lab.chars().collect();
                            let last = chars.pop().unwrap_or(' ');
                            let mut head = r.clone();
                            head.text = chars.into_iter().collect();
                            let mut tail = r;
                            tail.text = last.to_string();
                            let used = crate::measure::width(&head) + crate::measure::width(&tail);
                            tail.letter_spacing = if used < width - 0.5 {
                                width - used
                            } else {
                                base.size * 0.3
                            };
                            q.indent_first = -width;
                            q.runs.insert(0, tail);
                            if !head.text.is_empty() {
                                q.runs.insert(0, head);
                            }
                        }
                    }
                    if q.runs.is_empty() {
                        // An empty paragraph still takes a line.
                        q.runs.push(run_of("", &ts2));
                    }
                    out.push(q);
                }
                "list" => {
                    if level >= 10 {
                        continue;
                    }
                    let own = attr(c, "style-name").and_then(|n| self.styles.lists.get(n));
                    let list = own.or(list);
                    let lv = list.and_then(|l| l.get(level));
                    let mut n = lv.and_then(|l| l.number.as_ref()).map_or(1, |x| x.3);
                    for item in c
                        .children()
                        .filter(|i| matches!(i.tag_name().name(), "list-item" | "list-header"))
                    {
                        let mut lab = if item.tag_name().name() == "list-header" {
                            None
                        } else {
                            if let Some(v) = attr(item, "start-value").and_then(|v| v.parse().ok())
                            {
                                n = v;
                            }
                            let l = list_label(lv, n);
                            n = n.saturating_add(1);
                            Some(l)
                        };
                        self.paragraphs(item, ts, ps, level + 1, list, &mut lab, out);
                    }
                }
                _ => {}
            }
        }
    }

    /// (text, paragraph) props of a style's own chain, most general first.
    fn own_chain(&self, family: &str, name: &str) -> Vec<(Props, Props)> {
        let mut v = Vec::new();
        let mut cur = Some(name.to_string());
        while let Some(n) = cur {
            if v.len() >= 32 {
                break;
            }
            let Some(s) = self.styles.get(family, &n) else {
                break;
            };
            v.push((s.text.clone(), s.paragraph.clone()));
            cur = s.parent.clone();
        }
        v.reverse();
        v
    }

    fn runs(&self, n: Node, ts: &TextStyle, out: &mut Vec<Run>, depth: u32) {
        if depth > 16 {
            return;
        }
        for c in n.children() {
            if out.len() > 20_000 {
                return;
            }
            if c.is_text() {
                if let Some(t) = c.text() {
                    // Whitespace runs collapse to one space, as XML text in ODF does.
                    let mut s = String::with_capacity(t.len());
                    let mut ws = false;
                    for ch in t.chars() {
                        if ch.is_whitespace() {
                            if !ws {
                                s.push(' ');
                            }
                            ws = true;
                        } else {
                            s.push(ch);
                            ws = false;
                        }
                    }
                    out.push(run_of(&s, ts));
                }
                continue;
            }
            if !c.is_element() {
                continue;
            }
            match c.tag_name().name() {
                "span" | "a" => {
                    let mut ts2 = ts.clone();
                    if let Some(sn) = attr(c, "style-name") {
                        for s in self.own_chain("text", sn) {
                            apply_text(&mut ts2, &s.0, self.styles);
                        }
                    }
                    self.runs(c, &ts2, out, depth + 1);
                }
                "s" => {
                    let k = attr(c, "c")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .min(256);
                    out.push(run_of(&"\u{a0}".repeat(k), ts));
                }
                "tab" => out.push(run_of("\t", ts)),
                "line-break" => out.push(run_of("\n", ts)),
                "note" | "annotation" | "bookmark" | "bookmark-start" | "bookmark-end"
                | "soft-page-break" => {}
                "page-number" => out.push(run_of(&self.fields.page_number.to_string(), ts)),
                "page-count" => out.push(run_of(&self.fields.page_count.to_string(), ts)),
                "footer" | "header" | "date-time"
                    if c.tag_name()
                        .namespace()
                        .is_some_and(|n| n.contains("presentation")) =>
                {
                    let v = match c.tag_name().name() {
                        "footer" => &self.fields.footer,
                        "header" => &self.fields.header,
                        _ => &self.fields.date_time,
                    };
                    if let Some(v) = v {
                        out.push(run_of(v, ts));
                    }
                }
                // Other fields (dates, names, …): their stored text.
                _ => self.runs(c, ts, out, depth + 1),
            }
        }
    }
}

/// The natural size of paragraphs, points: unwrapped (`wrap` `None`), or wrapped at
/// `width` with every font scaled by `k` (height returned unscaled, in units of `k`).
fn natural_size(paras: &[Paragraph], wrap: Option<(f64, f64)>) -> (f64, f64) {
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for p in paras {
        let spacing = if p.line_spacing > 0.0 {
            p.line_spacing
        } else {
            1.0
        };
        let lh = p.line_height.unwrap_or_else(|| {
            p.runs
                .iter()
                .map(crate::measure::line_height)
                .fold(0.0, f64::max)
                * spacing
        });
        // Hard line breaks split the paragraph into lines.
        let mut lines = vec![0.0f64];
        for r in &p.runs {
            for (i, part) in r.text.split('\n').enumerate() {
                if i > 0 {
                    lines.push(0.0);
                }
                let mut q = r.clone();
                q.text = part.to_string();
                if let Some(l) = lines.last_mut() {
                    *l += crate::measure::width(&q);
                }
            }
        }
        let indent = p.indent_left + p.indent_right;
        let count: f64 = match wrap {
            None => lines.len() as f64,
            Some((bw, k)) => lines
                .iter()
                .map(|l| {
                    ((l * k + indent * k) / (bw - indent * k).max(1.0))
                        .ceil()
                        .max(1.0)
                })
                .sum(),
        };
        w = w.max(lines.iter().cloned().fold(0.0, f64::max) + indent);
        h += count * lh + p.space_before + p.space_after;
    }
    (w, h)
}

/// The list style a shape's text uses by default (its presentation or graphic style's).
fn n_list<'s>(styles: &'s Styles, shape: Node) -> Option<&'s Vec<ListLevel>> {
    let by = |ns: &str| {
        shape
            .attributes()
            .find(|a| a.name() == "style-name" && a.namespace().is_some_and(|x| x.contains(ns)))
            .map(|a| a.value())
    };
    by("presentation")
        .and_then(|n| styles.list_of("presentation", Some(n)))
        .or_else(|| by("drawing").and_then(|n| styles.list_of("graphic", Some(n))))
}

/// The label of item `n` at a list level: its bullet (large bullets drawn as the smaller
/// kind they show as, since labels take the text's size) or its number.
fn list_label(lv: Option<&ListLevel>, n: u32) -> String {
    let Some(lv) = lv else {
        return "\u{2022}".into();
    };
    if let Some((fmt, pre, suf, _)) = &lv.number {
        let num = match fmt.as_str() {
            "" => return format!("{pre}{suf}"),
            "a" => alpha(n, b'a'),
            "A" => alpha(n, b'A'),
            "i" => roman(n).to_lowercase(),
            "I" => roman(n),
            _ => n.to_string(),
        };
        return format!("{pre}{num}{suf}");
    }
    let b = lv.bullet.clone().unwrap_or_default();
    if lv.size < 0.75 {
        match b.as_str() {
            "\u{25cf}" | "\u{2b24}" | "\u{f0b7}" | "\u{e00c}" => return "\u{2022}".into(),
            "\u{25a0}" | "\u{f0a7}" => return "\u{25aa}".into(),
            _ => {}
        }
    }
    // Private-use symbol-font bullets have no glyph in text fonts.
    if b.chars().any(|c| ('\u{e000}'..='\u{f8ff}').contains(&c)) {
        return "\u{2022}".into();
    }
    b
}

fn alpha(mut n: u32, base: u8) -> String {
    let mut s = Vec::new();
    while n > 0 && s.len() < 8 {
        n -= 1;
        s.push(base + (n % 26) as u8);
        n /= 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn roman(mut n: u32) -> String {
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    let t = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (v, r) in t {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    s
}

fn run_of(text: &str, ts: &TextStyle) -> Run {
    Run {
        text: text.to_string(),
        family: ts.family.clone(),
        bold: ts.bold,
        italic: ts.italic,
        size: ts.size,
        color: ts.color,
        underline: ts.underline,
        strike: ts.strike,
        letter_spacing: ts.letter_spacing,
    }
}

/// `draw:ellipse`/`draw:circle` with `draw:kind` and angles, in a w×h box.
fn ellipse(n: Node, w: f64, h: f64) -> BezPath {
    let r = Rect::new(0.0, 0.0, w, h);
    let kind = attr(n, "kind").unwrap_or("full");
    let a0 = attr(n, "start-angle").and_then(angle_deg);
    let a1 = attr(n, "end-angle").and_then(angle_deg);
    if kind == "full" || (a0.is_none() && a1.is_none()) {
        return Ellipse::from_rect(r).to_path(0.05);
    }
    let (a0, a1) = (
        a0.unwrap_or(0.0).to_radians(),
        a1.unwrap_or(360.0).to_radians(),
    );
    let mut sweep = a1 - a0;
    while sweep <= 0.0 {
        sweep += 2.0 * PI;
    }
    let c = r.center();
    let rad = Vec2::new(w / 2.0, h / 2.0);
    let mut p = BezPath::new();
    // LibreOffice runs an open arc from its end angle back to its start (where the end
    // marker goes); closed kinds look the same either way.
    let (from, sweep) = if kind == "arc" {
        (a1, -sweep)
    } else {
        (a0, sweep)
    };
    let start = Point::new(c.x + rad.x * from.cos(), c.y - rad.y * from.sin());
    p.move_to(start);
    let arc = specular_drawpage::kurbo::Arc::new(c, rad, -from, -sweep, 0.0);
    arc.to_cubic_beziers(0.05, |a, b, d| p.curve_to(a, b, d));
    match kind {
        "section" => {
            p.line_to(c);
            p.close_path();
        }
        "cut" => p.close_path(),
        _ => {}
    }
    p
}

fn regular_polygon(n: Node, w: f64, h: f64) -> BezPath {
    let corners = attr(n, "corners")
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(3)
        .clamp(3, 1000);
    let concave = attr(n, "concave") == Some("true");
    let sharp = attr(n, "sharpness")
        .and_then(percent)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let c = Point::new(w / 2.0, h / 2.0);
    let mut p = BezPath::new();
    let steps = if concave { corners * 2 } else { corners };
    for i in 0..steps {
        let a = -PI / 2.0 + 2.0 * PI * i as f64 / steps as f64;
        let k = if concave && i % 2 == 1 {
            1.0 - sharp
        } else {
            1.0
        };
        let q = Point::new(c.x + w / 2.0 * k * a.cos(), c.y + h / 2.0 * k * a.sin());
        if i == 0 {
            p.move_to(q);
        } else {
            p.line_to(q);
        }
    }
    p.close_path();
    p
}

/// A gradient angle: degrees, with a bare number in tenths of a degree (ODF 1.2).
fn gradient_angle(s: &str) -> f64 {
    let t = s.trim();
    if t.bytes()
        .all(|b| b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        t.parse::<f64>().unwrap_or(0.0) / 10.0
    } else {
        angle_deg(t).unwrap_or(0.0)
    }
}

fn intensity(c: [u8; 4], k: f64) -> [u8; 4] {
    let k = k.clamp(0.0, 1.0);
    [
        (f64::from(c[0]) * k).round() as u8,
        (f64::from(c[1]) * k).round() as u8,
        (f64::from(c[2]) * k).round() as u8,
        c[3],
    ]
}

/// A `draw:gradient` over `bbox` (box space), as a drawpage paint in page space.
fn gradient(a: &Props, stops: &[(f64, String)], xf: Affine, bbox: Rect) -> Paint {
    let get = |k: &str| a.get(k).map(String::as_str);
    let start = intensity(
        get("start-color").and_then(color).unwrap_or([0, 0, 0, 255]),
        get("start-intensity").and_then(percent).unwrap_or(1.0),
    );
    let end = intensity(
        get("end-color")
            .and_then(color)
            .unwrap_or([255, 255, 255, 255]),
        get("end-intensity").and_then(percent).unwrap_or(1.0),
    );
    let border = get("border")
        .and_then(percent)
        .unwrap_or(0.0)
        .clamp(0.0, 0.99) as f32;
    let angle = get("angle").map(gradient_angle).unwrap_or(0.0).to_radians();
    let style = get("style").unwrap_or("linear");
    let st = |o: f32, c: [u8; 4]| Stop {
        offset: o,
        color: c,
    };
    // Multi-colour stops (LibreOffice 7.6+), start to end.
    let multi: Vec<Stop> = stops
        .iter()
        .filter_map(|(o, c)| Some(st(*o as f32, color(c)?)))
        .collect();
    let (w, h) = (bbox.width(), bbox.height());
    let c = bbox.center();
    match style {
        "linear" | "axial" => {
            // Angle 0: start at the top, end at the bottom; rotated counter-clockwise.
            let d = Vec2::new(angle.sin(), angle.cos());
            let half = (w / 2.0 * angle.sin()).abs() + (h / 2.0 * angle.cos()).abs();
            let (p0, p1) = (c - d * half, c + d * half);
            let s: Vec<Stop> = if style == "linear" {
                if multi.len() >= 2 {
                    multi
                        .iter()
                        .map(|s| st(border + s.offset * (1.0 - border), s.color))
                        .collect()
                } else {
                    vec![st(0.0, start), st(border, start), st(1.0, end)]
                }
            } else {
                // Axial: the start colour in the middle, the end colour at both edges.
                let b = border / 2.0;
                vec![
                    st(0.0, end),
                    st(b, end),
                    st(0.5, start),
                    st(1.0 - b, end),
                    st(1.0, end),
                ]
            };
            Paint::Linear {
                start: xf * p0,
                end: xf * p1,
                stops: s,
            }
        }
        _ => {
            // Radial-like: end colour in the centre, start colour outside.
            let cx = get("cx").and_then(percent).unwrap_or(0.5);
            let cy = get("cy").and_then(percent).unwrap_or(0.5);
            let centre = Point::new(bbox.x0 + w * cx, bbox.y0 + h * cy);
            let (rx, ry) = match style {
                "radial" => {
                    let r = w.hypot(h) / 2.0;
                    (r, r)
                }
                "square" => {
                    let r = w.max(h) / 2.0 * std::f64::consts::SQRT_2;
                    (r, r)
                }
                _ => (
                    w / 2.0 * std::f64::consts::SQRT_2,
                    h / 2.0 * std::f64::consts::SQRT_2,
                ),
            };
            let s = vec![st(0.0, end), st(1.0 - border, start), st(1.0, start)];
            Paint::Radial {
                transform: xf
                    * Affine::translate(centre.to_vec2())
                    * Affine::rotate(-angle)
                    * Affine::scale_non_uniform(rx.max(1e-6), ry.max(1e-6)),
                stops: s,
            }
        }
    }
}

/// Applies a `draw:opacity` gradient to `paint`: returns the paint with alpha folded in,
/// and a uniform factor for what could not be folded (1 when folded).
fn fold_opacity(
    paint: Paint,
    og: &Props,
    xf: Affine,
    bbox: Rect,
    style: &Resolved,
) -> (Paint, f64) {
    let start = og
        .get("start")
        .and_then(|v| percent(v))
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let end = og
        .get("end")
        .and_then(|v| percent(v))
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let mean = (start + end) / 2.0;
    // The opacity gradient as a grey gradient: grey level = opacity.
    let grey = |v: f64| {
        let g = (v * 255.0).round() as u8;
        format!("#{g:02x}{g:02x}{g:02x}")
    };
    let mut ga = og.clone();
    ga.insert("start-color".into(), grey(start));
    ga.insert("end-color".into(), grey(end));
    ga.remove("start-intensity");
    ga.remove("end-intensity");
    let alpha_paint = gradient(&ga, &[], xf, bbox);
    match (paint, alpha_paint) {
        (
            Paint::Solid(c),
            Paint::Linear {
                start: a,
                end: b,
                stops,
            },
        ) => (
            Paint::Linear {
                start: a,
                end: b,
                stops: stops
                    .iter()
                    .map(|s| Stop {
                        offset: s.offset,
                        color: [
                            c[0],
                            c[1],
                            c[2],
                            ((u32::from(c[3]) * u32::from(s.color[0])) / 255) as u8,
                        ],
                    })
                    .collect(),
            },
            1.0,
        ),
        (Paint::Solid(c), Paint::Radial { transform, stops }) => (
            Paint::Radial {
                transform,
                stops: stops
                    .iter()
                    .map(|s| Stop {
                        offset: s.offset,
                        color: [
                            c[0],
                            c[1],
                            c[2],
                            ((u32::from(c[3]) * u32::from(s.color[0])) / 255) as u8,
                        ],
                    })
                    .collect(),
            },
            1.0,
        ),
        (
            Paint::Linear {
                start: a,
                end: b,
                stops,
            },
            Paint::Linear {
                start: oa,
                end: ob,
                stops: os,
            },
        ) if (b - a).normalize().dot((ob - oa).normalize()) > std::f64::consts::FRAC_1_SQRT_2 => {
            // Axes within 45°: both ramps along the opacity's axis (exact when they match).
            // Same axis: sample the alpha ramp at every colour stop and every alpha stop.
            let mut offs: Vec<f32> = stops
                .iter()
                .map(|s| s.offset)
                .chain(os.iter().map(|s| s.offset))
                .collect();
            offs.sort_by(f32::total_cmp);
            offs.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            let at = |v: &[Stop], o: f32| -> [f32; 4] {
                let k = v.iter().position(|s| s.offset >= o).unwrap_or(v.len() - 1);
                if k == 0 {
                    return v[0].color.map(f32::from);
                }
                let (p, q) = (&v[k - 1], &v[k]);
                let t = if q.offset > p.offset {
                    (o - p.offset) / (q.offset - p.offset)
                } else {
                    0.0
                };
                let mut out = [0.0; 4];
                for (i, o) in out.iter_mut().enumerate() {
                    *o = f32::from(p.color[i]) * (1.0 - t) + f32::from(q.color[i]) * t;
                }
                out
            };
            let stops = offs
                .iter()
                .map(|&o| {
                    let c = at(&stops, o);
                    let a = at(&os, o)[0] / 255.0;
                    Stop {
                        offset: o,
                        color: [c[0] as u8, c[1] as u8, c[2] as u8, (c[3] * a).round() as u8],
                    }
                })
                .collect();
            let _ = (a, b);
            (
                Paint::Linear {
                    start: oa,
                    end: ob,
                    stops,
                },
                1.0,
            )
        }
        (other, _) => {
            let _ = style;
            (other, mean)
        }
    }
}

/// `fo:border` ("0.5pt solid #000000", "none") → (width, colour).
fn border(v: &str) -> Option<(f64, [u8; 4])> {
    if v.trim() == "none" || v.contains("hidden") {
        return None;
    }
    let mut w = 0.75;
    let mut c = [0, 0, 0, 255];
    for t in v.split_whitespace() {
        if let Some(x) = color(t) {
            c = x;
        } else if let Some(x) = length(t) {
            w = x;
        } else if t == "none" {
            return None;
        }
    }
    (w > 0.0).then_some((w, c))
}

/// The page background from drawing-page properties.
pub fn background(ctx: &mut Ctx, props: &Props, r: Rect) {
    let size = (r.width(), r.height());
    let res = Resolved {
        graphic: props.clone(),
        ..Resolved::default()
    };
    let mode = props.get("fill").map(String::as_str).unwrap_or("none");
    if mode == "none" {
        return;
    }
    let p = Placed {
        xf: Affine::translate((r.x0, r.y0)),
        w: size.0,
        h: size.1,
    };
    let path = r.to_path(0.05);
    let local = Rect::new(0.0, 0.0, size.0, size.1);
    if mode == "hatch" {
        ctx.hatch(&res, &p, &path, local);
        return;
    }
    if let Some(paint) = ctx.fill_paint(&res, mode, &p, local) {
        ctx.push(Op::Fill {
            path,
            even_odd: false,
            paint,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms() {
        let m = parse_transform("rotate (1.5707963267949) translate (2cm 1cm)");
        let p = m * Point::new(10.0, 0.0);
        // Rotated counter-clockwise on screen (y down): +x goes to −y, then moved.
        assert!((p.x - 2.0 * 72.0 / 2.54).abs() < 1e-6, "{p:?}");
        assert!((p.y - (72.0 / 2.54 - 10.0)).abs() < 1e-6, "{p:?}");
        assert_eq!(parse_transform("bogus (1) rotate ("), Affine::IDENTITY);
        let s = parse_transform("scale (2 3)");
        assert_eq!(s * Point::new(1.0, 1.0), Point::new(2.0, 3.0));
    }

    #[test]
    fn b64() {
        assert_eq!(base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64("aGVs\nbG8h").unwrap(), b"hello!");
        assert!(base64("a*b").is_none());
    }

    #[test]
    fn angles() {
        assert_eq!(gradient_angle("300"), 30.0);
        assert_eq!(gradient_angle("30deg"), 30.0);
    }

    #[test]
    fn labels_and_borders() {
        let lv = |f: &str| ListLevel {
            number: Some((f.into(), "(".into(), ")".into(), 1)),
            size: 1.0,
            ..ListLevel::default()
        };
        assert_eq!(list_label(Some(&lv("A")), 28), "(AB)");
        assert_eq!(list_label(Some(&lv("I")), 1994), "(MCMXCIV)");
        assert_eq!(list_label(Some(&lv("1")), 7), "(7)");
        let big = ListLevel {
            bullet: Some("\u{25cf}".into()),
            size: 0.45,
            ..ListLevel::default()
        };
        assert_eq!(list_label(Some(&big), 1), "\u{2022}");
        assert_eq!(border("0.5pt solid #ff0000"), Some((0.5, [255, 0, 0, 255])));
        assert_eq!(border("none"), None);
        assert_eq!(roman(0), "0");
    }

    #[test]
    fn image_resolutions() {
        let mut jfif = vec![0xff, 0xd8, 0xff, 0xe0, 0, 16];
        jfif.extend_from_slice(b"JFIF\0\x01\x01\x01\x01\x2c\x01\x2c");
        assert_eq!(image_resolution(&jfif), (72.0 / 300.0, 72.0 / 300.0));
        assert_eq!(image_resolution(b"junk"), (0.75, 0.75));
    }
}
