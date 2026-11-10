//! draw.io / diagrams.net files: `<mxfile>` with one `<diagram>` a page (its model inline,
//! or compressed: base64 of raw DEFLATE of the URI-encoded XML), or a bare
//! `<mxGraphModel>`. Each `mxCell` is a vertex or an edge with a `style` string
//! (`shape;key=value;…`) and an `mxGeometry`; a child's geometry is relative to its parent
//! vertex.

use std::collections::HashMap;
use std::sync::Arc;

use specular_drawpage::kurbo::{Ellipse, RoundedRect};
use specular_drawpage::xml::{self, roxmltree::Node};
use specular_drawpage::{
    Affine, BezPath, HAlign, Image, ImageDecoder, Op, Paint, Point, Rect, Rgba, Shape, Stroke,
    VAlign,
};

use crate::{
    Diagram, EMPTY_BOUNDS, Error, MAX_ELEMENTS, colour, faded, grow, html_text, page, text_block,
};

/// Whether `d` is a draw.io file: an `mxfile` or `mxGraphModel` root near its start.
pub fn is_drawio(d: &[u8]) -> bool {
    let h = String::from_utf8_lossy(&d[..d.len().min(4096)]);
    let t = h.trim_start_matches('\u{feff}').trim_start();
    let t = if t.starts_with("<?xml") {
        t.split_once("?>").map_or("", |x| x.1).trim_start()
    } else {
        t
    };
    t.starts_with("<mxfile") || t.starts_with("<mxGraphModel")
}

/// A style string: the bare words (shape names) and key=value pairs.
struct Style<'a> {
    words: Vec<&'a str>,
    map: HashMap<&'a str, &'a str>,
}

impl<'a> Style<'a> {
    fn parse(s: &'a str) -> Style<'a> {
        let mut words = Vec::new();
        let mut map = HashMap::new();
        for part in s.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                Some((k, v)) => {
                    map.insert(k, v);
                }
                None => words.push(part),
            }
        }
        Style { words, map }
    }
    fn get(&self, k: &str) -> Option<&'a str> {
        self.map.get(k).copied()
    }
    fn num(&self, k: &str, default: f64) -> f64 {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
    fn flag(&self, k: &str) -> bool {
        self.get(k) == Some("1")
    }
    /// The shape: `shape=…`, else the first bare word.
    fn shape(&self) -> &'a str {
        self.get("shape")
            .or_else(|| self.words.first().copied())
            .unwrap_or("rect")
    }
    /// A colour key: `None` for none, the default when unset or "default".
    fn colour(&self, k: &str, default: Option<Rgba>) -> Option<Rgba> {
        match self.get(k) {
            None | Some("default") | Some("inherit") => default,
            Some("none") | Some("") => None,
            Some(v) => colour(v).or(default),
        }
    }
}

struct Cell<'a, 'i> {
    node: Node<'a, 'i>,
    id: String,
    parent: String,
    value: String,
    style: String,
    vertex: bool,
    edge: bool,
    source: Option<String>,
    target: Option<String>,
}

/// Every diagram's model as XML text.
fn models(text: &str) -> Result<Vec<(String, String)>, Error> {
    let doc = xml::parse(text).map_err(|e| Error::Malformed(e.to_string()))?;
    let root = doc.root_element();
    if root.has_tag_name("mxGraphModel") {
        return Ok(vec![("Page-1".into(), text.to_owned())]);
    }
    if !root.has_tag_name("mxfile") {
        return Err(Error::Malformed("not an mxfile".into()));
    }
    let mut out = Vec::new();
    for d in root.children().filter(|n| n.has_tag_name("diagram")) {
        let name = d.attribute("name").unwrap_or("Page").to_owned();
        if let Some(m) = d.children().find(|n| n.has_tag_name("mxGraphModel")) {
            out.push((name, text[m.range()].to_owned()));
            continue;
        }
        let body: String = d.text().unwrap_or("").trim().to_owned();
        if body.is_empty() {
            continue;
        }
        let raw = crate::base64(&body).ok_or_else(|| Error::Malformed("diagram base64".into()))?;
        let inflated = specular_png::inflate_raw(&raw, 64 << 20)
            .map_err(|_| Error::Malformed("compressed diagram".into()))?;
        let xml = crate::percent_decode(&String::from_utf8_lossy(&inflated));
        out.push((name, xml));
    }
    Ok(out)
}

/// Reads a draw.io file's pages.
pub fn open_drawio(d: &[u8], images: Option<ImageDecoder>) -> Result<Diagram, Error> {
    let text = String::from_utf8_lossy(d);
    let text = text.trim_start_matches('\u{feff}');
    let mut out = Diagram {
        pages: Vec::new(),
        shapes: 0,
        connectors: 0,
        missing_pictures: 0,
        approximated: 0,
    };
    for (name, model) in models(text)? {
        let doc = xml::parse(&model).map_err(|e| Error::Malformed(e.to_string()))?;
        let m = doc.root_element();
        let background = m
            .attribute("background")
            .and_then(colour)
            .unwrap_or([255, 255, 255, 255]);
        let mut cells: Vec<Cell> = Vec::new();
        let root = m.children().find(|n| n.has_tag_name("root"));
        for n in root
            .into_iter()
            .flat_map(|r| r.children())
            .filter(|n| n.is_element())
        {
            if cells.len() >= MAX_ELEMENTS {
                break;
            }
            // `<UserObject>`/`<object>` wrap a cell and carry its label.
            let (cell, label) = if n.has_tag_name("mxCell") {
                (n, n.attribute("value"))
            } else if let Some(c) = n.children().find(|c| c.has_tag_name("mxCell")) {
                (c, n.attribute("label"))
            } else {
                continue;
            };
            cells.push(Cell {
                node: cell,
                id: n
                    .attribute("id")
                    .or(cell.attribute("id"))
                    .unwrap_or("")
                    .to_owned(),
                parent: cell.attribute("parent").unwrap_or("").to_owned(),
                value: label.unwrap_or("").to_owned(),
                style: cell.attribute("style").unwrap_or("").to_owned(),
                vertex: cell.attribute("vertex") == Some("1"),
                edge: cell.attribute("edge") == Some("1"),
                source: cell.attribute("source").map(str::to_owned),
                target: cell.attribute("target").map(str::to_owned),
            });
        }
        let index: HashMap<&str, usize> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.as_str(), i))
            .collect();
        let geom = |c: &Cell| -> Option<(f64, f64, f64, f64, bool)> {
            let g = c.node.children().find(|n| n.has_tag_name("mxGeometry"))?;
            let f = |k: &str| {
                g.attribute(k)
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0)
            };
            Some((
                f("x"),
                f("y"),
                f("width"),
                f("height"),
                g.attribute("relative") == Some("1"),
            ))
        };
        // Absolute boxes of the vertices (parents' offsets added).
        let mut boxes: Vec<Option<(f64, f64, f64, f64)>> = vec![None; cells.len()];
        for i in 0..cells.len() {
            if !cells[i].vertex {
                continue;
            }
            let mut depth = 0;
            let (mut x, mut y, w, h) = match geom(&cells[i]) {
                Some((x, y, w, h, rel)) if !rel => (x, y, w, h),
                Some((x, y, w, h, _)) => {
                    // Relative to the parent's size.
                    let p = index
                        .get(cells[i].parent.as_str())
                        .and_then(|&p| geom(&cells[p]));
                    match p {
                        Some((_, _, pw, ph, _)) => (x * pw, y * ph, w, h),
                        None => (x, y, w, h),
                    }
                }
                None => continue,
            };
            let mut p = cells[i].parent.clone();
            while let Some(&pi) = index.get(p.as_str()) {
                if !cells[pi].vertex || depth > 64 {
                    break;
                }
                if let Some((px, py, ..)) = geom(&cells[pi]) {
                    x += px;
                    y += py;
                }
                p = cells[pi].parent.clone();
                depth += 1;
            }
            boxes[i] = Some((x, y, w, h));
        }
        let mut ops = Vec::new();
        let mut bounds = EMPTY_BOUNDS;
        for (i, c) in cells.iter().enumerate() {
            let st = Style::parse(&c.style);
            if c.vertex
                && let Some(b) = boxes[i]
            {
                out.shapes += 1;
                grow(&mut bounds, b.0, b.1);
                grow(&mut bounds, b.0 + b.2, b.1 + b.3);
                vertex(&mut ops, c, &st, b, images.as_ref(), &mut out);
            } else if c.edge {
                out.connectors += 1;
                edge(&mut ops, c, &st, &cells, &index, &boxes, &mut bounds);
            }
        }
        out.pages.push(page(name, bounds, ops, background));
    }
    if out.pages.is_empty() {
        return Err(Error::Malformed("no diagrams".into()));
    }
    Ok(out)
}

fn label_text(c: &Cell, st: &Style) -> String {
    if st.get("html") == Some("1") || c.value.contains('<') {
        html_text(&c.value)
    } else {
        crate::entities(&c.value)
    }
}

fn font_ops(
    ops: &mut Vec<Op>,
    text: &str,
    st: &Style,
    rect: (f64, f64, f64, f64),
    rotation: f64,
    top: bool,
) {
    let fs = st.num("fontStyle", 0.0) as u32;
    let align = match st.get("align") {
        Some("left") => HAlign::Left,
        Some("right") => HAlign::Right,
        _ => HAlign::Centre,
    };
    let valign = match st.get("verticalAlign") {
        Some("top") => VAlign::Top,
        Some("bottom") => VAlign::Bottom,
        None if top => VAlign::Top,
        _ => VAlign::Middle,
    };
    let colour = st
        .colour("fontColor", Some([0, 0, 0, 255]))
        .unwrap_or([0, 0, 0, 255]);
    let opacity = st.num("textOpacity", 100.0) / 100.0;
    let pad = 2.0;
    if let Some(op) = text_block(
        text,
        (
            rect.0 + pad,
            rect.1 + pad,
            (rect.2 - 2.0 * pad).max(4.0),
            (rect.3 - 2.0 * pad).max(4.0),
        ),
        st.get("fontFamily").unwrap_or("Helvetica"),
        st.num("fontSize", 12.0),
        faded(colour, opacity),
        fs & 1 != 0,
        fs & 2 != 0,
        align,
        valign,
        rotation,
    ) {
        ops.push(op);
    }
}

fn vertex(
    ops: &mut Vec<Op>,
    c: &Cell,
    st: &Style,
    (x, y, w, h): (f64, f64, f64, f64),
    images: Option<&ImageDecoder>,
    out: &mut Diagram,
) {
    let shape = st.shape();
    let opacity = st.num("opacity", 100.0) / 100.0;
    let is_text = shape == "text" || shape == "label" && st.get("fillColor").is_none();
    let fill = if is_text {
        None
    } else {
        st.colour("fillColor", Some([255, 255, 255, 255]))
    }
    .map(|f| faded(f, opacity * st.num("fillOpacity", 100.0) / 100.0));
    let stroke_c = if is_text {
        None
    } else {
        st.colour("strokeColor", Some([0, 0, 0, 255]))
    }
    .map(|s| faded(s, opacity * st.num("strokeOpacity", 100.0) / 100.0));
    let width = st.num("strokeWidth", 1.0);
    let rotation = st.num("rotation", 0.0);
    let r = Rect::new(x, y, x + w, y + h);
    let path: Option<BezPath> = match shape {
        "ellipse" | "doubleEllipse" => Some(Ellipse::from_rect(r).to_path(0.1)),
        "rhombus" => {
            let mut p = BezPath::new();
            p.move_to((x + w / 2.0, y));
            p.line_to((x + w, y + h / 2.0));
            p.line_to((x + w / 2.0, y + h));
            p.line_to((x, y + h / 2.0));
            p.close_path();
            Some(p)
        }
        "triangle" => {
            let mut p = BezPath::new();
            match st.get("direction") {
                Some("north") => {
                    p.move_to((x + w / 2.0, y));
                    p.line_to((x + w, y + h));
                    p.line_to((x, y + h));
                }
                Some("south") => {
                    p.move_to((x, y));
                    p.line_to((x + w, y));
                    p.line_to((x + w / 2.0, y + h));
                }
                Some("west") => {
                    p.move_to((x + w, y));
                    p.line_to((x + w, y + h));
                    p.line_to((x, y + h / 2.0));
                }
                _ => {
                    p.move_to((x, y));
                    p.line_to((x + w, y + h / 2.0));
                    p.line_to((x, y + h));
                }
            }
            p.close_path();
            Some(p)
        }
        "hexagon" => {
            let s = (w * 0.25).min(h * 0.5);
            let mut p = BezPath::new();
            p.move_to((x + s, y));
            p.line_to((x + w - s, y));
            p.line_to((x + w, y + h / 2.0));
            p.line_to((x + w - s, y + h));
            p.line_to((x + s, y + h));
            p.line_to((x, y + h / 2.0));
            p.close_path();
            Some(p)
        }
        "cylinder" | "cylinder3" | "datastore" => {
            let e = (h * 0.1).min(15.0);
            let mut p = BezPath::new();
            p.move_to((x, y + e));
            p.curve_to((x, y - e / 3.0), (x + w, y - e / 3.0), (x + w, y + e));
            p.line_to((x + w, y + h - e));
            p.curve_to(
                (x + w, y + h + e / 3.0),
                (x, y + h + e / 3.0),
                (x, y + h - e),
            );
            p.close_path();
            p.move_to((x, y + e));
            p.curve_to(
                (x, y + e + e * 4.0 / 3.0),
                (x + w, y + e + e * 4.0 / 3.0),
                (x + w, y + e),
            );
            Some(p)
        }
        "image" => None,
        "line" => {
            let mut p = BezPath::new();
            p.move_to((x, y + h / 2.0));
            p.line_to((x + w, y + h / 2.0));
            Some(p)
        }
        _ => {
            let known = matches!(
                shape,
                "rect"
                    | "rounded"
                    | "text"
                    | "label"
                    | "swimlane"
                    | "process"
                    | "card"
                    | "note"
                    | "parallelogram"
                    | "document"
                    | "step"
            );
            if !known && !shape.starts_with("rect") {
                out.approximated += 1;
            }
            if st.flag("rounded") || shape == "rounded" {
                let arc = if st.flag("absoluteArcSize") {
                    st.num("arcSize", 10.0) / 2.0
                } else {
                    w.min(h) * st.num("arcSize", 15.0) / 100.0
                };
                Some(RoundedRect::from_rect(r, arc).to_path(0.1))
            } else {
                Some(r.to_path(0.1))
            }
        }
    };
    let rot = Affine::rotate_about(rotation.to_radians(), (x + w / 2.0, y + h / 2.0));
    if let Some(p) = path {
        let p = rot * p;
        if let Some(f) = fill {
            ops.push(Op::Fill {
                path: p.clone(),
                even_odd: false,
                paint: Paint::Solid(f),
            });
        }
        if let Some(s) = stroke_c {
            ops.push(Op::Stroke {
                path: p,
                stroke: dashed(Stroke::new(width), st, width),
                paint: Paint::Solid(s),
            });
        }
        // A swimlane's header line.
        if shape == "swimlane"
            && let Some(s) = stroke_c
        {
            let head = st.num("startSize", 23.0).min(h);
            let mut l = BezPath::new();
            l.move_to((x, y + head));
            l.line_to((x + w, y + head));
            ops.push(Op::Stroke {
                path: rot * l,
                stroke: Stroke::new(width),
                paint: Paint::Solid(s),
            });
        }
    }
    if shape == "image" || st.get("image").is_some() {
        let src = st.get("image").unwrap_or("");
        // `data:image/png,<base64>` in draw.io's style (the `;base64` is dropped there).
        let bytes = src
            .strip_prefix("data:")
            .and_then(|r| r.split_once(','))
            .and_then(|(_, b)| {
                crate::base64(b).or_else(|| Some(crate::percent_decode(b).into_bytes()))
            });
        match (bytes, images) {
            (Some(b), Some(dec)) => match Image::decode(dec, &b) {
                Some(img) => {
                    let (iw, ih) = (f64::from(img.width), f64::from(img.height));
                    let t =
                        rot * Affine::translate((x, y)) * Affine::scale_non_uniform(w / iw, h / ih);
                    ops.push(Op::Image {
                        image: Arc::new(img),
                        transform: t,
                        opacity: opacity as f32,
                    });
                }
                None => out.missing_pictures += 1,
            },
            _ => out.missing_pictures += 1,
        }
    }
    let text = label_text(c, st);
    if !text.is_empty() {
        let rect = if shape == "swimlane" {
            (x, y, w, st.num("startSize", 23.0).min(h))
        } else {
            match (st.get("verticalLabelPosition"), st.get("labelPosition")) {
                (Some("bottom"), _) => (
                    x - w,
                    y + h,
                    3.0 * w,
                    24.0f64.max(st.num("fontSize", 12.0) * 2.0),
                ),
                (Some("top"), _) => (x - w, y - 24.0, 3.0 * w, 24.0),
                (_, Some("right")) => (x + w, y, 2.0 * w.max(60.0), h),
                (_, Some("left")) => (x - 2.0 * w.max(60.0), y, 2.0 * w.max(60.0), h),
                _ => (x, y, w, h),
            }
        };
        // A label below its shape hangs from the top of its box.
        let below = st.get("verticalLabelPosition") == Some("bottom");
        font_ops(ops, &text, st, rect, rotation, below);
    }
}

fn dashed(s: Stroke, st: &Style, width: f64) -> Stroke {
    if st.flag("dashed") {
        let pattern: Vec<f64> = st
            .get("dashPattern")
            .map(|p| {
                p.split_whitespace()
                    .filter_map(|v| v.parse::<f64>().ok())
                    .map(|v| v * width)
                    .collect()
            })
            .filter(|v: &Vec<f64>| !v.is_empty() && v.iter().all(|x| *x > 0.0))
            .unwrap_or_else(|| vec![3.0 * width, 3.0 * width]);
        s.with_dashes(0.0, pattern)
    } else {
        s
    }
}

/// Where the line from `from` toward the centre of `b` meets its border (the ellipse's for
/// ellipses).
fn border(b: (f64, f64, f64, f64), from: Point, ellipse: bool) -> Point {
    let c = Point::new(b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
    let (dx, dy) = (from.x - c.x, from.y - c.y);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c;
    }
    let (hw, hh) = (b.2 / 2.0, b.3 / 2.0);
    let t = if ellipse {
        1.0 / ((dx / hw.max(1e-9)).powi(2) + (dy / hh.max(1e-9)).powi(2)).sqrt()
    } else {
        (hw / dx.abs().max(1e-9)).min(hh / dy.abs().max(1e-9))
    };
    Point::new(c.x + dx * t.min(1.0), c.y + dy * t.min(1.0))
}

fn edge(
    ops: &mut Vec<Op>,
    c: &Cell,
    st: &Style,
    cells: &[Cell],
    index: &HashMap<&str, usize>,
    boxes: &[Option<(f64, f64, f64, f64)>],
    bounds: &mut (f64, f64, f64, f64),
) {
    let g = c.node.children().find(|n| n.has_tag_name("mxGeometry"));
    let point = |n: Node| -> Point {
        let f = |k: &str| {
            n.attribute(k)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        Point::new(f("x"), f("y"))
    };
    let named = |name: &str| {
        g.and_then(|g| {
            g.children()
                .find(|n| n.has_tag_name("mxPoint") && n.attribute("as") == Some(name))
        })
        .map(point)
    };
    // Waypoints are in the edge's parent's space.
    let offset = {
        let (mut ox, mut oy) = (0.0, 0.0);
        if let Some(&pi) = index.get(c.parent.as_str())
            && let Some(b) = boxes[pi]
        {
            ox = b.0;
            oy = b.1;
        }
        (ox, oy)
    };
    let mut way: Vec<Point> = g
        .and_then(|g| {
            g.children()
                .find(|n| n.has_tag_name("Array") && n.attribute("as") == Some("points"))
        })
        .map(|a| {
            a.children()
                .filter(|n| n.has_tag_name("mxPoint"))
                .map(point)
                .collect()
        })
        .unwrap_or_default();
    for p in &mut way {
        p.x += offset.0;
        p.y += offset.1;
    }
    let end_box = |id: &Option<String>| -> Option<((f64, f64, f64, f64), bool)> {
        let i = *index.get(id.as_deref()?)?;
        let ellipse = Style::parse(&cells[i].style).shape() == "ellipse";
        boxes[i].map(|b| (b, ellipse))
    };
    let (sb, tb) = (end_box(&c.source), end_box(&c.target));
    let centre = |b: (f64, f64, f64, f64)| Point::new(b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
    let shift = |p: Point| Point::new(p.x + offset.0, p.y + offset.1);
    let s0 = sb
        .map(|(b, _)| centre(b))
        .or(named("sourcePoint").map(shift));
    let t0 = tb
        .map(|(b, _)| centre(b))
        .or(named("targetPoint").map(shift));
    let (Some(s0), Some(t0)) = (s0, t0) else {
        return;
    };
    let orthogonal = st
        .get("edgeStyle")
        .is_some_and(|e| e.contains("orthogonal") || e.contains("elbow"));
    if way.is_empty() && orthogonal && (s0.x - t0.x).abs() > 1.0 && (s0.y - t0.y).abs() > 1.0 {
        // An elbow: across, then down (or the other way for a vertical elbow).
        if st.get("elbow") == Some("vertical") {
            let my = (s0.y + t0.y) / 2.0;
            way = vec![Point::new(s0.x, my), Point::new(t0.x, my)];
        } else {
            let mx = (s0.x + t0.x) / 2.0;
            way = vec![Point::new(mx, s0.y), Point::new(mx, t0.y)];
        }
    }
    let first_toward = way.first().copied().unwrap_or(t0);
    let last_from = way.last().copied().unwrap_or(s0);
    let s = sb.map_or(s0, |(b, e)| border(b, first_toward, e));
    let t = tb.map_or(t0, |(b, e)| border(b, last_from, e));
    let mut pts = vec![s];
    pts.extend(way);
    pts.push(t);
    let mut p = BezPath::new();
    p.move_to(pts[0]);
    for q in &pts[1..] {
        p.line_to(*q);
        grow(bounds, q.x, q.y);
    }
    grow(bounds, pts[0].x, pts[0].y);
    let colour = st.colour("strokeColor", Some([0, 0, 0, 255]));
    let width = st.num("strokeWidth", 1.0);
    let Some(colour) = colour else { return };
    let colour = faded(colour, st.num("opacity", 100.0) / 100.0);
    let stroke = dashed(Stroke::new(width), st, width);
    let paint = Paint::Solid(colour);
    ops.push(Op::Stroke {
        path: p,
        stroke: stroke.clone(),
        paint: paint.clone(),
    });
    let size = st.num("endSize", 6.0) + width * 2.0;
    let n = pts.len();
    let end = st.get("endArrow").unwrap_or("classic");
    crate::arrowhead(ops, pts[n - 1], pts[n - 2], end, size, &paint, &stroke);
    if let Some(start) = st.get("startArrow") {
        crate::arrowhead(
            ops,
            pts[0],
            pts[1],
            start,
            st.num("startSize", 6.0) + width * 2.0,
            &paint,
            &stroke,
        );
    }
    let text = label_text(c, st);
    if !text.is_empty() {
        let m = pts[n / 2 - usize::from(n % 2 == 0)].midpoint(pts[n / 2]);
        let (w, h) = (160.0, 40.0);
        let lb = st.colour("labelBackgroundColor", None);
        if let Some(bg) = lb {
            let chars = text.lines().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
            let fs = st.num("fontSize", 12.0);
            let (bw, bh) = (
                chars * fs * 0.6 + 4.0,
                text.lines().count() as f64 * fs * 1.25 + 2.0,
            );
            ops.push(Op::Fill {
                path: Rect::new(
                    m.x - bw / 2.0,
                    m.y - bh / 2.0,
                    m.x + bw / 2.0,
                    m.y + bh / 2.0,
                )
                .to_path(0.1),
                even_odd: false,
                paint: Paint::Solid(bg),
            });
        }
        font_ops(
            ops,
            &text,
            st,
            (m.x - w / 2.0, m.y - h / 2.0, w, h),
            0.0,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: &str = r##"<mxfile><diagram name="One"><mxGraphModel><root>
<mxCell id="0"/><mxCell id="1" parent="0"/>
<mxCell id="a" value="Start" style="rounded=1;fillColor=#dae8fc;" vertex="1" parent="1"><mxGeometry x="10" y="10" width="80" height="40" as="geometry"/></mxCell>
<mxCell id="b" value="&lt;b&gt;End&lt;/b&gt;" style="ellipse;html=1;" vertex="1" parent="1"><mxGeometry x="200" y="10" width="80" height="40" as="geometry"/></mxCell>
<mxCell id="e" style="endArrow=classic;" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>
</root></mxGraphModel></diagram></mxfile>"##;

    #[test]
    fn plain_diagram() {
        assert!(is_drawio(PLAIN.as_bytes()));
        let d = open_drawio(PLAIN.as_bytes(), None).unwrap();
        assert_eq!(d.pages.len(), 1);
        assert_eq!((d.shapes, d.connectors), (2, 1));
        let p = &d.pages[0];
        assert_eq!(p.name, "One");
        assert!((p.size.0 - (270.0 + 40.0)).abs() < 1e-6, "{:?}", p.size);
        let texts = p.ops.iter().filter(|o| matches!(o, Op::Text(_))).count();
        assert_eq!(texts, 2);
    }

    #[test]
    fn style_words() {
        let s = Style::parse("ellipse;whiteSpace=wrap;fillColor=none;");
        assert_eq!(s.shape(), "ellipse");
        assert_eq!(s.colour("fillColor", Some([1, 1, 1, 255])), None);
        assert_eq!(
            s.colour("strokeColor", Some([1, 1, 1, 255])),
            Some([1, 1, 1, 255])
        );
    }
}
