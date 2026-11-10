//! Own IPC-2581 reader (revisions A to C, XML): the Ecad section's layers and their
//! functions and sides, the step's profile (the board's shape), each layer's features (pads
//! from the standard and user dictionaries or padstacks, lines, arcs, polylines, contours),
//! drill holes, and components with their reference designators and package outlines.
//!
//! Text features are drawn as strokes: with the glyphs of their embedded font
//! (`DictionaryFont`), else (external fonts such as "Arial", or none) in the built-in stroke
//! font, fitted to their bounding box. Files with no Ecad section (a bill of materials
//! or logistics data only) are refused with the reason.

use std::collections::HashMap;
use std::sync::Arc;

use roxmltree::{Document, Node, ParsingOptions};
use specular_vector::kurbo::{Affine, BezPath, Point, Shape as _};
use specular_vector::{PcbKind, VectorScene};

use crate::excellon::{Drill, Hole, Tool};
use crate::features::{self, Contour, Pen, Prim};
use crate::gerber::{Object, ObjectKind};
use crate::stack::{self, Component, Extra, Plating, Role, Side};
use crate::text::{self as stroke, StrokeFont};
use crate::{Error, Limits, warn};

/// XML nodes a file may have.
const MAX_NODES: u32 = 40_000_000;

/// Whether `head` starts an IPC-2581 file.
pub fn is_ipc2581(head: &[u8]) -> bool {
    let n = head.len().min(4096);
    let h = String::from_utf8_lossy(&head[..n]);
    h.contains("<IPC-2581") || h.contains(":IPC-2581")
}

/// Millimetres per unit of a `units` attribute.
fn unit_mm(u: Option<&str>) -> f64 {
    match u.map(str::to_ascii_uppercase).as_deref() {
        Some("INCH") => 25.4,
        Some("MICRON") => 0.001,
        Some("MILS") | Some("MIL") => 0.0254,
        _ => 1.0,
    }
}

fn f(n: Node<'_, '_>, a: &str) -> Option<f64> {
    n.attribute(a)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn fz(n: Node<'_, '_>, a: &str) -> f64 {
    f(n, a).unwrap_or(0.0)
}

fn is(n: Node<'_, '_>, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| is(*c, name))
}

fn children<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| is(*c, name))
}

fn truthy(v: Option<&str>) -> bool {
    v.is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1")
}

/// An `Xform` child as an affine (rotation counter-clockwise in degrees, mirror in x, scale,
/// offset), scaled by `k` for offsets.
fn xform(n: Node<'_, '_>, k: f64) -> Affine {
    let Some(x) = child(n, "Xform") else {
        return Affine::IDENTITY;
    };
    let mut a = Affine::translate((fz(x, "xOffset") * k, fz(x, "yOffset") * k))
        * Affine::rotate(fz(x, "rotation").to_radians())
        * Affine::scale(f(x, "scale").filter(|s| *s > 0.0).unwrap_or(1.0));
    if truthy(x.attribute("mirror")) {
        a *= Affine::FLIP_X;
    }
    a
}

fn location(n: Node<'_, '_>, k: f64) -> Point {
    child(n, "Location").map_or(Point::ZERO, |l| Point::new(fz(l, "x") * k, fz(l, "y") * k))
}

/// A polygon (`PolyBegin`, `PolyStepSegment`, `PolyStepCurve`) into `c`.
fn polygon(n: Node<'_, '_>, k: f64, c: &mut Contour, hole: bool) {
    for s in n.children().filter(|c| c.is_element()) {
        let p = Point::new(fz(s, "x") * k, fz(s, "y") * k);
        match s.tag_name().name() {
            "PolyBegin" => c.begin(p, hole),
            "PolyStepSegment" => c.line_to(p),
            "PolyStepCurve" => c.arc_to(
                p,
                Point::new(fz(s, "centerX") * k, fz(s, "centerY") * k),
                truthy(s.attribute("clockwise")),
            ),
            _ => {}
        }
    }
    c.end();
}

/// A contour element's polygon and cutouts as one outline.
fn contour_path(n: Node<'_, '_>, k: f64) -> BezPath {
    let mut c = Contour::default();
    if let Some(p) = child(n, "Polygon") {
        polygon(p, k, &mut c, false);
    }
    for cut in children(n, "Cutout") {
        polygon(cut, k, &mut c, true);
    }
    c.end();
    c.path
}

/// A standard primitive element (mm with `k`).
fn primitive(n: Node<'_, '_>, k: f64) -> Option<Prim> {
    let w = || f(n, "width").map(|v| v * k);
    let h = || f(n, "height").map(|v| v * k);
    Some(match n.tag_name().name() {
        "Circle" => Prim::Circle(f(n, "diameter")? * k),
        "RectCenter" => Prim::Rect(w()?, h()?),
        "RectRound" => Prim::RoundRect(w()?, h()?, fz(n, "radius") * k),
        "RectCham" => Prim::Chamfer(w()?, h()?, fz(n, "chamfer") * k),
        "Oval" => Prim::Oval(w()?, h()?),
        "Diamond" => Prim::Diamond(w()?, h()?),
        "Octagon" => Prim::Octagon(w()?, h()?, fz(n, "chamfer") * k),
        "Ellipse" => Prim::Ellipse(w()?, h()?),
        "Triangle" => Prim::Triangle(f(n, "base")? * k, h()?),
        "Hexagon" => {
            let l = f(n, "length")? * k;
            Prim::Hexagon(l, l * 3f64.sqrt() / 2.0, l / 4.0)
        }
        "Donut" => {
            let (od, id) = (f(n, "outerDiameter")? * k, fz(n, "innerDiameter") * k);
            if n.attribute("shape")
                .is_some_and(|s| s.eq_ignore_ascii_case("SQUARE"))
            {
                Prim::SquareDonut(od, id)
            } else {
                Prim::Donut(od, id)
            }
        }
        "Thermal" => Prim::Thermal(
            f(n, "outerDiameter")? * k,
            fz(n, "innerDiameter") * k,
            fz(n, "spokeStartAngle"),
            f(n, "spokeCount").unwrap_or(4.0) as u32,
            fz(n, "gap") * k,
        ),
        "RectCorner" => {
            let (x0, y0) = (fz(n, "lowerLeftX") * k, fz(n, "lowerLeftY") * k);
            let (x1, y1) = (fz(n, "upperRightX") * k, fz(n, "upperRightY") * k);
            Prim::Path(crate::geom::orient(
                specular_vector::kurbo::Rect::new(x0, y0, x1, y1).to_path(0.1),
                false,
            ))
        }
        "Contour" => Prim::Path(contour_path(n, k)),
        _ => return None,
    })
}

/// The glyphs of a font entry (`FontDefEmbedded` with `Glyph charCode` children holding
/// lines, polylines and arcs), with the width of their strokes, in mm by `k`; `None` for a
/// font defined outside the file (`FontDefExternal`) or one without glyphs.
fn embedded_font(e: Node<'_, '_>, k: f64) -> Option<(Arc<StrokeFont>, f64)> {
    let def = e.descendants().find(|c| is(*c, "FontDefEmbedded"))?;
    let mut glyphs = HashMap::new();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, 0.0f64, 0.0f64);
    let mut pen = 0.0f64;
    for g in children(def, "Glyph") {
        let Some(code) = g.attribute("charCode") else {
            continue;
        };
        // hexBinary ("41"); a single other character as itself.
        let c = u32::from_str_radix(code.trim(), 16)
            .ok()
            .filter(|_| code.trim().len() >= 2)
            .and_then(char::from_u32)
            .or_else(|| {
                let mut it = code.chars();
                it.next().filter(|_| it.next().is_none())
            });
        let Some(c) = c else { continue };
        let (lx, ly, ux, uy) = (
            fz(g, "lowerLeftX") * k,
            fz(g, "lowerLeftY") * k,
            fz(g, "upperRightX") * k,
            fz(g, "upperRightY") * k,
        );
        x0 = x0.min(lx);
        y0 = y0.min(ly);
        x1 = x1.max(ux - lx);
        y1 = y1.max(uy - ly);
        let mut strokes = Vec::new();
        let mut seg = |a: Point, b: Point, square: bool| {
            strokes.push(stroke::Stroke {
                a,
                b,
                square,
                clear: false,
            })
        };
        for s in g.children().filter(|c| c.is_element()) {
            let p = |x: &str, y: &str| Point::new(fz(s, x) * k, fz(s, y) * k);
            let ld = child(s, "LineDesc").map(|d| line_desc(d, k));
            if let Some(d) = ld {
                pen = pen.max(d.width());
            }
            let square = matches!(ld, Some(Pen::Square(_)));
            match s.tag_name().name() {
                "Line" => seg(p("startX", "startY"), p("endX", "endY"), square),
                "Arc" => {
                    let (a, b, c) = (
                        p("startX", "startY"),
                        p("endX", "endY"),
                        p("centerX", "centerY"),
                    );
                    for (q, r) in arc_points(a, b, c, truthy(s.attribute("clockwise")))
                        .windows(2)
                        .map(|w| (w[0], w[1]))
                    {
                        seg(q, r, square);
                    }
                }
                "Polyline" => {
                    let mut at: Option<Point> = None;
                    for v in s.children().filter(|v| v.is_element()) {
                        let q = Point::new(fz(v, "x") * k, fz(v, "y") * k);
                        match (v.tag_name().name(), at) {
                            ("PolyBegin", _) => {}
                            ("PolyStepSegment", Some(o)) => seg(o, q, square),
                            ("PolyStepCurve", Some(o)) => {
                                let c = Point::new(fz(v, "centerX") * k, fz(v, "centerY") * k);
                                for w in
                                    arc_points(o, q, c, truthy(v.attribute("clockwise"))).windows(2)
                                {
                                    seg(w[0], w[1], square);
                                }
                            }
                            _ => continue,
                        }
                        at = Some(q);
                    }
                }
                _ => {}
            }
        }
        glyphs.insert(c, strokes);
    }
    if glyphs.is_empty() || x1 <= 0.0 || y1 <= 0.0 {
        return None;
    }
    let font = StrokeFont {
        xsize: x1,
        ysize: y1,
        offset: 0.0,
        origin: Point::new(x0, y0),
        glyphs,
    };
    Some((Arc::new(font), pen))
}

/// Points along an arc from `a` to `b` about `c` (at most 10° apart), `a` and `b` included.
fn arc_points(a: Point, b: Point, c: Point, cw: bool) -> Vec<Point> {
    let r = (a - c).hypot();
    let a0 = (a - c).atan2();
    let mut sweep = (b - c).atan2() - a0;
    let tau = std::f64::consts::TAU;
    if cw {
        while sweep >= 0.0 {
            sweep -= tau;
        }
    } else {
        while sweep <= 0.0 {
            sweep += tau;
        }
    }
    let n = ((sweep.abs() / 10f64.to_radians()).ceil() as usize).clamp(1, 36);
    (0..=n)
        .map(|i| {
            if i == n {
                return b;
            }
            let t = a0 + sweep * i as f64 / n as f64;
            c + specular_vector::kurbo::Vec2::new(t.cos(), t.sin()) * r
        })
        .collect()
}

/// A line description: width and end.
fn line_desc(n: Node<'_, '_>, k: f64) -> Pen {
    let w = fz(n, "lineWidth") * k;
    match n
        .attribute("lineEnd")
        .map(str::to_ascii_uppercase)
        .as_deref()
    {
        Some("SQUARE") => Pen::Square(w),
        _ => Pen::Round(w),
    }
}

/// The content dictionaries.
#[derive(Default)]
struct Dict {
    std: HashMap<String, Prim>,
    user: HashMap<String, Arc<Vec<Object>>>,
    lines: HashMap<String, Pen>,
    /// Embedded stroke fonts by id, with their stroke width (font units, 0 when unstated);
    /// `None` for fonts defined outside the file.
    fonts: HashMap<String, Option<(Arc<StrokeFont>, f64)>>,
}

struct Reader {
    dict: Dict,
    /// Ecad units → mm.
    k: f64,
    texts: u64,
    pads: u64,
    lines: u64,
    arcs: u64,
    contours: u64,
    items: usize,
    max_items: usize,
    unknown: Vec<String>,
    /// Characters no font had.
    missing_chars: u64,
    /// Text features with neither a bounding box nor a font size.
    no_size: u64,
    /// Fonts named outside the file ("Arial"): text drawn in the built-in font.
    outside_fonts: Vec<String>,
}

impl Reader {
    fn pen_of(&self, n: Node<'_, '_>) -> Pen {
        if let Some(d) = child(n, "LineDesc") {
            return line_desc(d, self.k);
        }
        child(n, "LineDescRef")
            .and_then(|r| r.attribute("id"))
            .and_then(|id| self.dict.lines.get(id).copied())
            .unwrap_or(Pen::Round(0.0))
    }

    fn push(&mut self, out: &mut Vec<Object>, o: Object) {
        if self.items < self.max_items {
            self.items += 1;
            out.push(o);
        }
    }

    fn missing(&mut self, id: &str) {
        if self.unknown.len() < 32 && !self.unknown.iter().any(|u| u == id) {
            self.unknown.push(id.to_string());
        }
    }

    /// A primitive placed by `a`: a reference to the dictionaries or one written inline.
    fn place(&mut self, n: Node<'_, '_>, a: Affine, dark: bool, out: &mut Vec<Object>) -> bool {
        for c in n.children().filter(|c| c.is_element()) {
            let name = c.tag_name().name();
            if name == "StandardPrimitiveRef" {
                let id = c.attribute("id").unwrap_or("");
                match self.dict.std.get(id).cloned() {
                    Some(p) => self.flash(&p, a, dark, out),
                    None => self.missing(id),
                }
                return true;
            }
            if name == "UserPrimitiveRef" {
                let id = c.attribute("id").unwrap_or("");
                match self.dict.user.get(id).cloned() {
                    Some(objs) => {
                        for o in objs.iter() {
                            let mut q = o.clone();
                            q.path = crate::geom::transformed(&o.path, a);
                            q.segment = o.segment.map(|s| s.transformed(a));
                            q.dark = o.dark == dark;
                            self.push(out, q);
                        }
                    }
                    None => self.missing(id),
                }
                return true;
            }
            // A contour among features is a region (see `shapes`), not a pad.
            if name != "Contour"
                && let Some(p) = primitive(c, self.k)
            {
                self.flash(&p, a, dark, out);
                return true;
            }
        }
        false
    }

    /// A `Text` feature placed by `a`: `textString` laid out from its `BoundingBox`'s
    /// lower-left corner, as high as the box (else `fontSize`), stretched to the box's width,
    /// then turned, mirrored and scaled by its `Xform`.
    fn text(&mut self, n: Node<'_, '_>, a: Affine, dark: bool) -> Vec<Object> {
        let k = self.k;
        let s = n.attribute("textString").unwrap_or("");
        if s.trim().is_empty() {
            return Vec::new();
        }
        let bbox = child(n, "BoundingBox").map(|b| {
            (
                Point::new(fz(b, "lowerLeftX") * k, fz(b, "lowerLeftY") * k),
                Point::new(fz(b, "upperRightX") * k, fz(b, "upperRightY") * k),
            )
        });
        let (ll, size) = match bbox {
            Some((p, q)) => (
                Point::new(p.x.min(q.x), p.y.min(q.y)),
                ((q.x - p.x).abs(), (q.y - p.y).abs()),
            ),
            None => (Point::ZERO, (0.0, 0.0)),
        };
        let h = if size.1 > 0.0 {
            size.1
        } else {
            fz(n, "fontSize") * k
        };
        if h <= 0.0 {
            self.no_size += 1;
            return Vec::new();
        }
        let id = n
            .children()
            .find(|c| is(*c, "FontRef") || is(*c, "Font"))
            .and_then(|c| c.attribute("id").or_else(|| c.attribute("name")));
        let found = id.and_then(|id| self.dict.fonts.get(id).cloned());
        let (own, pen) = match found {
            Some(Some((f, pen))) => (Some(f), pen),
            other => {
                if let (Some(id), Some(None)) = (id, other)
                    && !self.outside_fonts.iter().any(|f| f == id)
                    && self.outside_fonts.len() < 8
                {
                    self.outside_fonts.push(id.to_string());
                }
                (None, 0.0)
            }
        };
        let font = own.as_deref().unwrap_or(stroke::builtin());
        // Cells as high as the text, as wide as the font's are for that height.
        let ch = h;
        let mut cw = font.xsize * h / font.ysize;
        let mut gap = font.offset * h / font.ysize;
        let natural = StrokeFont::advance(s, cw, gap);
        if size.0 > 0.0 && natural > 0.0 {
            let f = size.0 / natural;
            cw *= f;
            gap *= f;
        }
        let width = if pen > 0.0 {
            pen * h / font.ysize
        } else {
            h / 8.0
        };
        let place = a * xform(n, k) * Affine::translate(ll.to_vec2());
        font.draw(s, cw, ch, gap, width, place, dark, &mut self.missing_chars)
    }

    fn flash(&mut self, p: &Prim, a: Affine, dark: bool, out: &mut Vec<Object>) {
        self.pads += 1;
        let mut o = features::flash(p, Point::ZERO, 0.0, false, dark);
        o.path = crate::geom::transformed(&o.path, a);
        self.push(out, o);
    }

    /// The drawable children of a `Features`, `UserSpecial` or `Set` element.
    fn shapes(&mut self, n: Node<'_, '_>, a: Affine, dark: bool, out: &mut Vec<Object>) {
        let k = self.k;
        for c in n.children().filter(|c| c.is_element()) {
            let pt = |x: &str, y: &str| a * Point::new(fz(c, x) * k, fz(c, y) * k);
            match c.tag_name().name() {
                "Line" => {
                    self.lines += 1;
                    let pen = self.pen_of(c);
                    let o = features::line(pt("startX", "startY"), pt("endX", "endY"), pen, dark);
                    self.push(out, o);
                }
                "Arc" => {
                    self.arcs += 1;
                    let pen = self.pen_of(c);
                    let mut cw = truthy(c.attribute("clockwise"));
                    if a.determinant() < 0.0 {
                        cw = !cw;
                    }
                    let o = features::arc(
                        pt("startX", "startY"),
                        pt("endX", "endY"),
                        pt("centerX", "centerY"),
                        cw,
                        pen,
                        dark,
                    );
                    self.push(out, o);
                }
                "Polyline" => {
                    self.lines += 1;
                    let pen = self.pen_of(c);
                    let mut at: Option<Point> = None;
                    for s in c.children().filter(|s| s.is_element()) {
                        let p = a * Point::new(fz(s, "x") * k, fz(s, "y") * k);
                        match (s.tag_name().name(), at) {
                            ("PolyBegin", _) => at = Some(p),
                            ("PolyStepSegment", Some(q)) => {
                                let o = features::line(q, p, pen, dark);
                                self.push(out, o);
                                at = Some(p);
                            }
                            ("PolyStepCurve", Some(q)) => {
                                let cc = a * Point::new(fz(s, "centerX") * k, fz(s, "centerY") * k);
                                let mut cw = truthy(s.attribute("clockwise"));
                                if a.determinant() < 0.0 {
                                    cw = !cw;
                                }
                                let o = features::arc(q, p, cc, cw, pen, dark);
                                self.push(out, o);
                                at = Some(p);
                            }
                            _ => {}
                        }
                    }
                }
                "Contour" => {
                    self.contours += 1;
                    let p = contour_path(c, k);
                    if !p.elements().is_empty() {
                        let o = Object {
                            path: crate::geom::transformed(&p, a),
                            dark,
                            kind: ObjectKind::Region,
                            segment: None,
                            holes: true,
                        };
                        self.push(out, o);
                    }
                }
                "Features" | "UserSpecial" => {
                    let b = a * Affine::translate(location(c, k).to_vec2()) * xform(c, k);
                    if !self.place(c, b, dark, out) {
                        self.shapes(c, b, dark, out);
                    }
                }
                "Text" => {
                    self.texts += 1;
                    for o in self.text(c, a, dark) {
                        self.push(out, o);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Reads an IPC-2581 file into a board scene.
pub fn scene(bytes: &[u8], limits: &Limits) -> Result<VectorScene, Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Malformed("the file is not UTF-8 XML".into()))?;
    let opt = ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..ParsingOptions::default()
    };
    let doc = Document::parse_with_options(text, opt).map_err(|e| match e {
        roxmltree::Error::NodesLimitReached => Error::TooLarge,
        e => Error::Malformed(format!("damaged XML: {e}")),
    })?;
    let root = doc.root_element();
    if root.tag_name().name() != "IPC-2581" {
        return Err(Error::NotThisFormat);
    }
    let revision = root.attribute("revision").unwrap_or("").to_string();
    let Some(ecad) = child(root, "Ecad") else {
        return Err(Error::Unsupported(
            "This IPC-2581 file has no board data (no Ecad section): it holds only a bill of \
             materials or logistics data."
                .into(),
        ));
    };
    let k = unit_mm(child(ecad, "CadHeader").and_then(|h| h.attribute("units")));
    let mut warnings = Vec::new();

    // Dictionaries.
    let mut dict = Dict::default();
    if let Some(content) = child(root, "Content") {
        for d in children(content, "DictionaryStandard") {
            let dk = unit_mm(d.attribute("units"));
            for e in children(d, "EntryStandard") {
                let Some(id) = e.attribute("id") else {
                    continue;
                };
                if let Some(p) = e
                    .children()
                    .filter(|c| c.is_element())
                    .find_map(|c| primitive(c, dk))
                {
                    dict.std.insert(id.to_string(), p);
                }
            }
        }
        for d in children(content, "DictionaryFont") {
            let dk = unit_mm(d.attribute("units"));
            for e in children(d, "EntryFont") {
                let Some(id) = e.attribute("id") else {
                    continue;
                };
                dict.fonts.insert(id.to_string(), embedded_font(e, dk));
            }
        }
        for d in children(content, "DictionaryLineDesc") {
            let dk = unit_mm(d.attribute("units"));
            for e in children(d, "EntryLineDesc") {
                if let (Some(id), Some(ld)) = (e.attribute("id"), child(e, "LineDesc")) {
                    dict.lines.insert(id.to_string(), line_desc(ld, dk));
                }
            }
        }
    }
    let mut r = Reader {
        dict,
        k,
        texts: 0,
        pads: 0,
        lines: 0,
        arcs: 0,
        contours: 0,
        items: 0,
        max_items: limits.items,
        unknown: Vec::new(),
        missing_chars: 0,
        no_size: 0,
        outside_fonts: Vec::new(),
    };
    if let Some(content) = child(root, "Content") {
        for d in children(content, "DictionaryUser") {
            let dk = unit_mm(d.attribute("units"));
            for e in children(d, "EntryUser") {
                let Some(id) = e.attribute("id") else {
                    continue;
                };
                let mut objs = Vec::new();
                let saved = r.k;
                r.k = dk;
                r.shapes(e, Affine::IDENTITY, true, &mut objs);
                r.k = saved;
                r.dict.user.insert(id.to_string(), Arc::new(objs));
            }
        }
    }
    // Counts are of the board's features, not the dictionaries'.
    (r.pads, r.lines, r.arcs, r.contours, r.texts, r.items) = (0, 0, 0, 0, 0, 0);

    let cad = child(ecad, "CadData")
        .ok_or_else(|| Error::Malformed("the Ecad section has no CadData".into()))?;
    let steps: Vec<Node<'_, '_>> = children(cad, "Step").collect();
    let step = steps
        .iter()
        .find(|s| {
            s.attribute("type")
                .is_none_or(|t| !t.eq_ignore_ascii_case("PANEL"))
        })
        .or(steps.first())
        .copied()
        .ok_or_else(|| Error::Malformed("the file has no Step".into()))?;
    if steps.len() > 1 {
        warn(
            &mut warnings,
            format!(
                "The file has {} steps; showing {}.",
                steps.len(),
                step.attribute("name").unwrap_or("the first")
            ),
        );
    }

    // Layers in file order with their roles.
    struct L<'a> {
        name: &'a str,
        role: Role,
    }
    let mut inner = 0;
    let layers: Vec<L<'_>> = children(cad, "Layer")
        .filter_map(|l| {
            let name = l.attribute("name")?;
            let func = l
                .attribute("layerFunction")
                .unwrap_or("")
                .to_ascii_uppercase();
            let side = match l.attribute("side").map(str::to_ascii_uppercase).as_deref() {
                Some("BOTTOM") => Side::Bottom,
                _ => Side::Top,
            };
            let internal = l
                .attribute("side")
                .is_some_and(|s| s.eq_ignore_ascii_case("INTERNAL"));
            let role = match func.as_str() {
                "CONDUCTOR" | "SIGNAL" | "PLANE" | "MIXED" | "CONDFILM" | "CONDFOIL" => {
                    if internal {
                        inner += 1;
                        Role::InnerCopper(inner)
                    } else if side == Side::Bottom {
                        Role::BottomCopper
                    } else {
                        Role::TopCopper
                    }
                }
                "SOLDERMASK" => Role::SolderMask(side),
                "SILKSCREEN" | "LEGEND" => Role::Silkscreen(side),
                "SOLDERPASTE" | "PASTEMASK" => Role::Paste(side),
                "DRILL" => Role::Drill(Plating::Mixed),
                "BOARD_OUTLINE" | "PROFILE" => Role::Outline,
                "ROUT" | "V_CUT" | "SCORE" | "DOCUMENT" | "ASSEMBLY" | "BOARDFAB" => {
                    Role::Mechanical
                }
                _ => Role::Other,
            };
            Some(L { name, role })
        })
        .collect();

    // Padstacks: pads per layer, for pads that name only their padstack.
    let mut padstacks: HashMap<&str, Vec<(&str, Node<'_, '_>)>> = HashMap::new();
    for ps in children(step, "PadStackDef") {
        let Some(name) = ps.attribute("name") else {
            continue;
        };
        let pads = children(ps, "PadstackPadDef")
            .filter_map(|p| Some((p.attribute("layerRef")?, p)))
            .collect();
        padstacks.insert(name, pads);
    }

    let mut out = Vec::new();
    let mut nets = std::collections::HashSet::new();
    for lf in children(step, "LayerFeature") {
        let Some(lref) = lf.attribute("layerRef") else {
            continue;
        };
        let Some(l) = layers.iter().find(|l| l.name == lref) else {
            continue;
        };
        if l.role.is_drill() {
            let mut d = Drill::default();
            let mut plated = (0, 0);
            for set in children(lf, "Set") {
                for h in children(set, "Hole") {
                    let dia = fz(h, "diameter") * k;
                    let at = Point::new(fz(h, "x") * k, fz(h, "y") * k);
                    let p = h.attribute("platingStatus").map(str::to_ascii_uppercase);
                    match p.as_deref() {
                        Some("NONPLATED") => plated.1 += 1,
                        _ => plated.0 += 1,
                    }
                    let key = (dia * 1e4).round();
                    let tool = match d
                        .tools
                        .iter()
                        .position(|t| (t.diameter * 1e4).round() == key)
                    {
                        Some(i) => i,
                        None => {
                            d.tools.push(Tool {
                                number: d.tools.len() as u32 + 1,
                                diameter: dia,
                                plated: None,
                            });
                            d.tools.len() - 1
                        }
                    };
                    d.holes.push(Hole { at, tool });
                }
            }
            let role = match plated {
                (0, n) if n > 0 => Role::Drill(Plating::NonPlated),
                (n, 0) if n > 0 => Role::Drill(Plating::Plated),
                _ => Role::Drill(Plating::Mixed),
            };
            if !d.holes.is_empty() {
                out.push(stack::drill_layer(lref.to_string(), role, d));
            }
            continue;
        }
        let mut objs = Vec::new();
        for set in children(lf, "Set") {
            if let Some(n) = set.attribute("net") {
                nets.insert(n.to_string());
            }
            let dark = !set
                .attribute("polarity")
                .is_some_and(|p| p.eq_ignore_ascii_case("NEGATIVE"));
            for c in set.children().filter(|c| c.is_element()) {
                match c.tag_name().name() {
                    "Pad" => {
                        let a = Affine::translate(location(c, k).to_vec2()) * xform(c, k);
                        if !r.place(c, a, dark, &mut objs)
                            && let Some(ps) = c.attribute("padstackDefRef")
                            && let Some(pads) = padstacks.get(ps)
                        {
                            for (_, pd) in pads.iter().filter(|(lr, _)| *lr == lref) {
                                let b = a
                                    * Affine::translate(location(*pd, k).to_vec2())
                                    * xform(*pd, k);
                                r.place(*pd, b, dark, &mut objs);
                            }
                        }
                    }
                    "Features" => {
                        let a = Affine::translate(location(c, k).to_vec2()) * xform(c, k);
                        if !r.place(c, a, dark, &mut objs) {
                            r.shapes(c, a, dark, &mut objs);
                        }
                    }
                    _ => {}
                }
            }
        }
        let b = features::bounds(&objs);
        out.push(stack::gerber_layer(lref.to_string(), l.role, objs, b));
    }
    if out.is_empty() {
        return Err(Error::Unsupported(
            "This IPC-2581 file draws nothing: it has no layer features (only a bill of \
             materials, a stack-up or other data)."
                .into(),
        ));
    }

    // The board's shape.
    let shape = child(step, "Profile").map(|p| {
        let mut c = Contour::default();
        if let Some(poly) = child(p, "Polygon") {
            polygon(poly, k, &mut c, false);
        }
        for cut in children(p, "Cutout") {
            polygon(cut, k, &mut c, true);
        }
        c.end();
        c.path
    });

    // Components.
    let layer_side: HashMap<&str, Side> = children(cad, "Layer")
        .filter_map(|l| {
            let side = match l.attribute("side").map(str::to_ascii_uppercase).as_deref() {
                Some("BOTTOM") => Side::Bottom,
                _ => Side::Top,
            };
            Some((l.attribute("name")?, side))
        })
        .collect();
    let outlines: HashMap<&str, BezPath> = children(step, "Package")
        .filter_map(|p| {
            let o = child(p, "Outline")?;
            let path = contour_path(o, k);
            Some((p.attribute("name")?, path))
        })
        .collect();
    let mut comps = Vec::new();
    for c in children(step, "Component") {
        let at = location(c, k);
        let a = Affine::translate(at.to_vec2()) * xform(c, k);
        let rotation = child(c, "Xform").map_or(0.0, |x| fz(x, "rotation"));
        let side = c
            .attribute("layerRef")
            .and_then(|l| layer_side.get(l).copied())
            .unwrap_or(Side::Top);
        let outline = c
            .attribute("packageRef")
            .and_then(|p| outlines.get(p))
            .filter(|p| !p.elements().is_empty())
            .map(|p| a * p.clone());
        comps.push(Component {
            refdes: c.attribute("refDes").unwrap_or("").to_string(),
            at,
            rotation,
            side,
            outline,
        });
    }
    let nets = children(step, "LogicalNet").count().max(nets.len()) as u64;

    if !r.unknown.is_empty() {
        warn(
            &mut warnings,
            format!(
                "Primitives not in the dictionaries: {}",
                r.unknown.join(", ")
            ),
        );
    }
    if !r.outside_fonts.is_empty() {
        warn(
            &mut warnings,
            format!(
                "Fonts not in the file, text drawn in the built-in font: {}",
                r.outside_fonts.join(", ")
            ),
        );
    }
    if r.no_size > 0 {
        warn(
            &mut warnings,
            format!(
                "{} text features have no size and are not drawn.",
                r.no_size
            ),
        );
    }
    if r.missing_chars > 0 {
        warn(
            &mut warnings,
            format!(
                "{} characters of text are in no font and are left out.",
                r.missing_chars
            ),
        );
    }
    if r.items >= limits.items {
        warn(&mut warnings, "The board has more features than are drawn.");
    }
    let software = root
        .descendants()
        .find(|n| is(*n, "SoftwarePackage"))
        .and_then(|s| {
            let name = s.attribute("name")?;
            Some(match s.attribute("revision") {
                Some(v) => format!("{name} {v}"),
                None => name.to_string(),
            })
        });
    let counts = [
        ("Pad", r.pads),
        ("Line", r.lines),
        ("Arc", r.arcs),
        ("Contour", r.contours),
        ("Text", r.texts),
    ]
    .into_iter()
    .filter(|(_, n)| *n > 0)
    .map(|(k, n)| (k.to_string(), n))
    .collect();
    let thickness = child(cad, "Stackup")
        .and_then(|s| f(s, "overallThickness"))
        .map(|t| t * k);
    let extra = Extra {
        format: "IPC-2581".into(),
        version: if revision.is_empty() {
            String::new()
        } else {
            format!("rev {revision}")
        },
        kind: Some(PcbKind::Ipc2581),
        shape: shape.filter(|s| s.area().abs() > 0.0),
        components: comps,
        generator: software,
        project: ecad.attribute("name").map(Into::into),
        nets,
        counts,
        thickness,
        ..Extra::default()
    };
    Ok(stack::board_of(out, warnings, extra))
}

#[cfg(test)]
mod tests;
