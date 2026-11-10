//! Turning ShapeSheets into display-list operations: cell inheritance (instance, master
//! shape, style sheets), transforms, geometry, fills, lines, arrowheads, images and text.

use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::sync::Arc;

use specular_drawpage::kurbo::{self, Affine, BezPath, Point, Rect, Shape, Stroke, Vec2};
use specular_drawpage::{
    HAlign, Image, ImageDecoder, Op, Paint, Paragraph, Rgba, Run, Stop, TextBlock, VAlign,
};

use crate::sheet::{Cell, Piece, Row, Section, Sheet};
use crate::theme::{FillStyle, Theme};

/// Deepest style-sheet chain followed.
const MAX_STYLE_DEPTH: usize = 32;
/// Deepest shape nesting drawn (master shapes included).
const MAX_DRAW_DEPTH: usize = 48;
/// Most shapes visited per document (masters' shapes counted per instance): a master whose
/// shapes instance the master again fans out exponentially within the depth cap.
const MAX_SHAPE_VISITS: u64 = 1_000_000;
/// Points per inch.
const PT: f64 = 72.0;

/// The 24 colours of Visio's default palette (cells that hold an index).
const PALETTE: [[u8; 3]; 24] = [
    [0, 0, 0],
    [255, 255, 255],
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [255, 0, 255],
    [0, 255, 255],
    [128, 0, 0],
    [0, 128, 0],
    [0, 0, 128],
    [128, 128, 0],
    [128, 0, 128],
    [0, 128, 128],
    [192, 192, 192],
    [230, 230, 230],
    [205, 205, 205],
    [179, 179, 179],
    [154, 154, 154],
    [128, 128, 128],
    [102, 102, 102],
    [77, 77, 77],
    [51, 51, 51],
    [26, 26, 26],
];

/// A master: its top-level shapes and the relationships of its part (images).
pub(crate) struct Master {
    pub shapes: Vec<Sheet>,
    pub rels: HashMap<String, String>,
}

/// What was left out, counted for the info panel.
#[derive(Debug, Default, Clone)]
pub(crate) struct Skipped {
    pub images: u64,
    pub geometry: u64,
    pub limited: bool,
}

/// The document-wide state drawing needs.
pub(crate) struct Ctx<'a> {
    pub styles: &'a HashMap<u32, Sheet>,
    pub masters: &'a HashMap<u32, Master>,
    pub theme: &'a Theme,
    /// FaceName ID → family (older files name fonts by number).
    pub faces: &'a HashMap<u32, String>,
    pub palette: &'a [[u8; 3]],
    pub media: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    pub decode: Option<&'a ImageDecoder>,
    pub images: HashMap<String, Option<Arc<Image>>>,
    pub image_pixels: u64,
    pub skipped: Skipped,
    pub shapes: u64,
    pub ops: usize,
}

/// Page-level settings shapes resolve through.
pub(crate) struct PageEnv<'a> {
    pub variant: usize,
    pub var_style: usize,
    /// Layer index → visible.
    pub layers: HashMap<u32, bool>,
    /// Relationships of the page part (id → part).
    pub rels: &'a HashMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Line,
    Fill,
    Text,
}

fn kind_of(cell: &str) -> Kind {
    if cell.starts_with("Line")
        || cell.starts_with("BeginArrow")
        || cell.starts_with("EndArrow")
        || cell == "Rounding"
        || cell == "CompoundType"
    {
        Kind::Line
    } else if cell.starts_with("Fill") || cell.starts_with("Shdw") || cell.starts_with("ShapeShdw")
    {
        Kind::Fill
    } else {
        Kind::Text
    }
}

/// An instance and the master shapes it inherits from, most specific first.
#[derive(Clone)]
pub(crate) struct Chain<'a> {
    pub levels: Vec<&'a Sheet>,
}

impl<'a> Chain<'a> {
    fn style_id(&self, k: Kind) -> Option<u32> {
        self.levels.iter().find_map(|s| match k {
            Kind::Line => s.line_style,
            Kind::Fill => s.fill_style,
            Kind::Text => s.text_style,
        })
    }
}

impl<'a> Ctx<'a> {
    /// The style sheets of `kind` from `id` up its parents.
    fn style_chain(&self, id: Option<u32>, k: Kind) -> Vec<&'a Sheet> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut cur = id;
        while let Some(i) = cur {
            if !seen.insert(i) || out.len() >= MAX_STYLE_DEPTH {
                break;
            }
            let Some(s) = self.styles.get(&i) else { break };
            out.push(s);
            cur = match k {
                Kind::Line => s.line_style,
                Kind::Fill => s.fill_style,
                Kind::Text => s.text_style,
            };
        }
        out
    }

    fn cell<'c>(&self, ch: &Chain<'c>, name: &str) -> Option<&'c Cell>
    where
        'a: 'c,
    {
        for s in &ch.levels {
            if let Some(c) = s.cells.get(name) {
                return Some(c);
            }
        }
        let kinds: &[Kind] = if name.starts_with("QuickStyle") {
            &[Kind::Fill, Kind::Line, Kind::Text]
        } else {
            match kind_of(name) {
                Kind::Line => &[Kind::Line],
                Kind::Fill => &[Kind::Fill],
                Kind::Text => &[Kind::Text],
            }
        };
        for &k in kinds {
            for s in self.style_chain(ch.style_id(k), k) {
                if let Some(c) = s.cells.get(name) {
                    return Some(c);
                }
            }
        }
        None
    }

    fn num(&self, ch: &Chain, name: &str) -> Option<f64> {
        self.cell(ch, name).and_then(Cell::num)
    }

    fn numd(&self, ch: &Chain, name: &str, d: f64) -> f64 {
        self.num(ch, name).unwrap_or(d)
    }

    fn themed(&self, ch: &Chain, name: &str) -> bool {
        self.cell(ch, name).is_some_and(|c| c.v == "Themed")
    }

    /// Rows of a named section merged from the style chain (`k`), master shapes and the
    /// instance, by row key; deleted rows dropped. In key order of first appearance.
    fn rows(&self, ch: &Chain, section: &str, ix: Option<u32>, k: Option<Kind>) -> Vec<Row> {
        let mut levels: Vec<&Section> = Vec::new();
        if let Some(k) = k {
            for s in self.style_chain(ch.style_id(k), k).iter().rev() {
                if let Some(sec) = s.section(section, ix) {
                    levels.push(sec);
                }
            }
        }
        for s in ch.levels.iter().rev() {
            if let Some(sec) = s.section(section, ix) {
                if sec.del {
                    levels.clear();
                } else {
                    levels.push(sec);
                }
            }
        }
        let mut out: Vec<Row> = Vec::new();
        for sec in levels {
            for r in &sec.rows {
                match out.iter_mut().position(|o| o.key == r.key) {
                    Some(i) => {
                        if r.del {
                            out.remove(i);
                        } else {
                            let o = &mut out[i];
                            if r.t.is_some() {
                                o.t = r.t.clone();
                            }
                            for (n, c) in &r.cells {
                                o.cells.insert(n.clone(), c.clone());
                            }
                        }
                    }
                    None if !r.del => out.push(r.clone()),
                    None => {}
                }
            }
        }
        out
    }

    fn connector(&self, ch: &Chain) -> bool {
        self.num(ch, "ObjType") == Some(2.0)
            || ch.levels.iter().any(|s| s.cells.contains_key("BeginX"))
    }

    fn scheme(&self, ch: &Chain) -> &crate::theme::Scheme {
        if self.connector(ch) && !self.theme.connectors.lines.is_empty() {
            &self.theme.connectors
        } else {
            &self.theme.shapes
        }
    }

    /// A colour cell: `#rrggbb`, a palette index, `RGB(r,g,b)`, or "Themed" (through the
    /// quick-style cells and the theme). With the cell's `…Trans` transparency.
    fn color(&self, ch: &Chain, env: &PageEnv, name: &str, themed: Themed) -> Option<Rgba> {
        let c = self.cell(ch, name)?;
        let mut rgba = self.parse_color(&c.v).or_else(|| {
            if c.v == "Themed" {
                Some(self.theme_color(ch, env, themed))
            } else {
                c.f.as_deref().and_then(|f| self.parse_color(f))
            }
        })?;
        let trans = match name {
            "Color" => "ColorTrans".to_string(),
            n => format!("{n}Trans"),
        };
        if let Some(t) = self.num(ch, &trans) {
            rgba[3] = ((1.0 - t.clamp(0.0, 1.0)) * f64::from(rgba[3])).round() as u8;
        }
        Some(rgba)
    }

    fn parse_color(&self, v: &str) -> Option<Rgba> {
        let v = v.trim();
        if let Some(c) = specular_drawpage::hex_color(v) {
            return Some(c);
        }
        if let Ok(i) = v.parse::<usize>() {
            return self
                .palette
                .get(i)
                .or_else(|| PALETTE.get(i))
                .map(|c| [c[0], c[1], c[2], 255]);
        }
        let up = v.to_ascii_uppercase();
        if let Some(rest) = up.strip_prefix("RGB(") {
            let n: Vec<u8> = rest
                .trim_end_matches(')')
                .split(',')
                .filter_map(|x| x.trim().parse::<f64>().ok())
                .map(|x| x.clamp(0.0, 255.0) as u8)
                .collect();
            if n.len() == 3 {
                return Some([n[0], n[1], n[2], 255]);
            }
        }
        None
    }

    fn quick(&self, ch: &Chain, name: &str, d: i64) -> i64 {
        self.num(ch, name).map_or(d, |v| v as i64)
    }

    fn theme_color(&self, ch: &Chain, env: &PageEnv, t: Themed) -> Rgba {
        let th = self.theme;
        let scheme = self.scheme(ch);
        let (cname, mname, fallback) = match t {
            Themed::Fill | Themed::FillBack => (
                "QuickStyleFillColor",
                "QuickStyleFillMatrix",
                [255, 255, 255],
            ),
            Themed::Line => ("QuickStyleLineColor", "QuickStyleLineMatrix", [0, 0, 0]),
            Themed::Font => ("QuickStyleFontColor", "QuickStyleFontMatrix", [0, 0, 0]),
        };
        let ph = th
            .quick_color(self.quick(ch, cname, 100), env.variant)
            .unwrap_or(fallback);
        let m = self.quick(ch, mname, 100);
        match t {
            Themed::Fill | Themed::FillBack => {
                let i = th.matrix(m, env.var_style, |v| v.fill);
                match i.checked_sub(1).and_then(|i| scheme.fills.get(i)) {
                    Some(FillStyle::Solid(s)) => th.resolve(s, ph),
                    Some(FillStyle::Grad(stops, _)) => {
                        let pick = if t == Themed::Fill {
                            stops.first()
                        } else {
                            stops.last()
                        };
                        pick.map_or([ph[0], ph[1], ph[2], 255], |s| th.resolve(&s.1, ph))
                    }
                    _ => [ph[0], ph[1], ph[2], 255],
                }
            }
            Themed::Line => {
                let i = th.matrix(m, env.var_style, |v| v.line);
                match i.checked_sub(1).and_then(|i| scheme.lines.get(i)) {
                    Some(l) => match &l.fill {
                        FillStyle::Solid(s) => th.resolve(s, ph),
                        FillStyle::Grad(stops, _) => stops
                            .first()
                            .map_or([ph[0], ph[1], ph[2], 255], |s| th.resolve(&s.1, ph)),
                        FillStyle::None => [ph[0], ph[1], ph[2], 255],
                    },
                    None => [ph[0], ph[1], ph[2], 255],
                }
            }
            Themed::Font => {
                let i = th.matrix(m, env.var_style, |v| v.font);
                match i
                    .checked_sub(1)
                    .and_then(|i| scheme.fonts.get(i))
                    .and_then(Option::as_ref)
                {
                    Some(s) => th.resolve(s, ph),
                    None if th.colors.is_empty() => [0, 0, 0, 255],
                    None => [ph[0], ph[1], ph[2], 255],
                }
            }
        }
    }

    /// The themed fill style (for a themed FillPattern), if any.
    fn theme_fill_style(&self, ch: &Chain, env: &PageEnv) -> Option<&FillStyle> {
        let m = self.quick(ch, "QuickStyleFillMatrix", 100);
        let i = self.theme.matrix(m, env.var_style, |v| v.fill);
        i.checked_sub(1).and_then(|i| self.scheme(ch).fills.get(i))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Themed {
    Fill,
    FillBack,
    Line,
    Font,
}

/// The chain of a shape: itself, then the master shape it names (or that its master's only
/// top-level shape is), recursively bounded.
pub(crate) fn chain_of<'a>(
    ctx: &Ctx<'a>,
    s: &'a Sheet,
    master: Option<u32>,
) -> (Chain<'a>, Option<u32>) {
    let mut levels = vec![s];
    let m = s.master.or(master);
    let mut cur = s;
    let mut mid = m;
    for _ in 0..4 {
        let Some(id) = mid else { break };
        let Some(mas) = ctx.masters.get(&id) else {
            break;
        };
        let next = match cur.master_shape {
            Some(sid) => mas.shapes.iter().find_map(|t| t.find(sid)),
            None if cur.master.is_some() && mas.shapes.len() == 1 => mas.shapes.first(),
            None => None,
        };
        let Some(next) = next else { break };
        if levels.iter().any(|l| std::ptr::eq(*l, next)) {
            break;
        }
        levels.push(next);
        cur = next;
        mid = next.master;
    }
    (Chain { levels }, m)
}

/// The transform of a shape's local coordinates (inches, y up) into its parent's.
fn local_transform(ctx: &Ctx, ch: &Chain) -> Affine {
    let w = ctx.numd(ch, "Width", 0.0);
    let h = ctx.numd(ch, "Height", 0.0);
    let pin_x = ctx.numd(ch, "PinX", 0.0);
    let pin_y = ctx.numd(ch, "PinY", 0.0);
    let loc_x = ctx.numd(ch, "LocPinX", w / 2.0);
    let loc_y = ctx.numd(ch, "LocPinY", h / 2.0);
    let angle = ctx.numd(ch, "Angle", 0.0);
    let fx = if ctx.numd(ch, "FlipX", 0.0) != 0.0 {
        -1.0
    } else {
        1.0
    };
    let fy = if ctx.numd(ch, "FlipY", 0.0) != 0.0 {
        -1.0
    } else {
        1.0
    };
    Affine::translate((pin_x, pin_y))
        * Affine::rotate(angle)
        * Affine::scale_non_uniform(fx, fy)
        * Affine::translate((-loc_x, -loc_y))
}

/// Draws the shapes of a page (or a background page, a master) into `ops`.
pub(crate) fn draw_shapes<'a>(
    ctx: &mut Ctx<'a>,
    env: &PageEnv,
    shapes: &'a [Sheet],
    to_page: Affine,
    ops: &mut Vec<Op>,
) {
    for s in shapes {
        draw_shape(ctx, env, s, None, to_page, ops, 0, env.rels);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_shape<'a>(
    ctx: &mut Ctx<'a>,
    env: &PageEnv,
    s: &'a Sheet,
    master: Option<u32>,
    parent: Affine,
    ops: &mut Vec<Op>,
    depth: usize,
    rels: &HashMap<String, String>,
) {
    // Guides are drawing aids, not printed.
    if s.del || depth > MAX_DRAW_DEPTH || s.kind == "Guide" {
        return;
    }
    if ctx.ops >= specular_drawpage::MAX_OPS || ctx.shapes >= MAX_SHAPE_VISITS {
        ctx.skipped.limited = true;
        return;
    }
    ctx.shapes += 1;
    let (ch, m) = chain_of(ctx, s, master);
    if hidden(ctx, &ch, env) {
        return;
    }
    let mt = parent * local_transform(ctx, &ch);
    let start = ops.len();
    geometry(ctx, env, &ch, mt, ops);
    foreign(ctx, &ch, mt, ops, rels, m);
    // Members: the instance's own, else the master shape's (or the master's top-level
    // shapes when the instance names none).
    let mut members: Vec<(&'a Sheet, Option<u32>)> = s.children.iter().map(|c| (c, m)).collect();
    if members.is_empty() {
        if let Some(ms) = ch.levels.get(1) {
            members = ms.children.iter().map(|c| (c, None)).collect();
        } else if s.master.is_some()
            && let Some(mas) = m.and_then(|id| ctx.masters.get(&id))
            && mas.shapes.len() > 1
        {
            members = mas.shapes.iter().map(|c| (c, None)).collect();
        }
    }
    let member_rels: HashMap<String, String>;
    let mrels = if s.children.is_empty() && !members.is_empty() {
        member_rels = m
            .and_then(|id| ctx.masters.get(&id))
            .map(|x| x.rels.clone())
            .unwrap_or_default();
        &member_rels
    } else {
        rels
    };
    for (c, cm) in members {
        draw_shape(ctx, env, c, cm, mt, ops, depth + 1, mrels);
    }
    text(ctx, env, &ch, mt, ops);
    ctx.ops += ops.len() - start;
}

fn hidden(ctx: &Ctx, ch: &Chain, env: &PageEnv) -> bool {
    if env.layers.is_empty() {
        return false;
    }
    let Some(c) = ctx.cell(ch, "LayerMember") else {
        return false;
    };
    let ids: Vec<u32> =
        c.v.split(';')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
    !ids.is_empty() && ids.iter().all(|i| env.layers.get(i) == Some(&false))
}

// ----- geometry -------------------------------------------------------------------------

/// One geometry section as a path in local coordinates, with its flags.
struct Geom {
    path: BezPath,
    fill: bool,
    line: bool,
}

fn row_num(r: &Row, n: &str) -> f64 {
    r.num(n).unwrap_or(0.0)
}

fn geoms(ctx: &mut Ctx, ch: &Chain) -> Vec<Geom> {
    let w = ctx.numd(ch, "Width", 0.0);
    let h = ctx.numd(ch, "Height", 0.0);
    // Geometry sections of the instance and its master shapes (not styles), by index.
    let mut ixs: Vec<Option<u32>> = Vec::new();
    for s in ch.levels.iter().rev() {
        for sec in &s.sections {
            if sec.n == "Geometry" && !ixs.contains(&sec.ix) {
                ixs.push(sec.ix);
            }
        }
    }
    let mut out = Vec::new();
    for ix in ixs {
        // Section flags, most specific first.
        let flag = |n: &str| {
            ch.levels
                .iter()
                .find_map(|s| s.section("Geometry", ix).and_then(|sec| sec.cells.get(n)))
                .and_then(Cell::num)
                .unwrap_or(0.0)
                != 0.0
        };
        if ch
            .levels
            .first()
            .and_then(|s| s.section("Geometry", ix))
            .is_some_and(|sec| sec.del)
        {
            continue;
        }
        if flag("NoShow") {
            continue;
        }
        let rows = ctx.rows(ch, "Geometry", ix, None);
        let mut rows = rows;
        rows.sort_by_key(|r| r.key.parse::<u32>().unwrap_or(u32::MAX));
        let path = build_path(&mut ctx.skipped, &rows, w, h);
        if path.elements().is_empty() {
            continue;
        }
        out.push(Geom {
            path,
            fill: !flag("NoFill"),
            line: !flag("NoLine"),
        });
    }
    out
}

fn build_path(skipped: &mut Skipped, rows: &[Row], w: f64, h: f64) -> BezPath {
    let mut p = BezPath::new();
    let mut cur: Option<Point> = None;
    let mut start = Point::ZERO;
    let mut spline: Vec<Point> = Vec::new();
    let flush_spline = |p: &mut BezPath, spline: &mut Vec<Point>| {
        for q in spline.drain(..) {
            p.line_to(q);
        }
    };
    for r in rows {
        let t = r.t.as_deref().unwrap_or("");
        let x = row_num(r, "X");
        let y = row_num(r, "Y");
        let (xx, yy) = if t.starts_with("Rel") {
            (x * w, y * h)
        } else {
            (x, y)
        };
        let pt = Point::new(xx, yy);
        if !matches!(t, "SplineKnot") {
            flush_spline(&mut p, &mut spline);
        }
        let ensure = |p: &mut BezPath, cur: &mut Option<Point>| {
            if cur.is_none() {
                p.move_to(Point::ZERO);
                *cur = Some(Point::ZERO);
            }
        };
        match t {
            "MoveTo" | "RelMoveTo" => {
                p.move_to(pt);
                start = pt;
                cur = Some(pt);
            }
            "LineTo" | "RelLineTo" => {
                ensure(&mut p, &mut cur);
                p.line_to(pt);
                cur = Some(pt);
            }
            "ArcTo" => {
                ensure(&mut p, &mut cur);
                let p0 = cur.unwrap_or(Point::ZERO);
                let a = row_num(r, "A");
                let chord = pt - p0;
                let len = chord.hypot();
                if len < 1e-12 || a.abs() < 1e-12 {
                    p.line_to(pt);
                } else {
                    // Bow: the arc's midpoint is |A| from the chord's, to the right of the
                    // direction of travel for positive A.
                    let n = Vec2::new(chord.y, -chord.x) / len;
                    let mid = p0.midpoint(pt) + n * a;
                    arc3(&mut p, p0, mid, pt, Affine::IDENTITY);
                }
                cur = Some(pt);
            }
            "EllipticalArcTo" | "RelEllipticalArcTo" => {
                ensure(&mut p, &mut cur);
                let p0 = cur.unwrap_or(Point::ZERO);
                let (mut a, mut b) = (row_num(r, "A"), row_num(r, "B"));
                let c = row_num(r, "C");
                let d = r.num("D").unwrap_or(1.0);
                let rel = t.starts_with("Rel");
                if rel {
                    a *= w;
                    b *= h;
                }
                // Normalised space for relative rows: the ratio applies there.
                let norm = if rel && w.abs() > 1e-12 && h.abs() > 1e-12 {
                    Affine::scale_non_uniform(w, h)
                } else {
                    Affine::IDENTITY
                };
                let inv = norm.inverse();
                let (q0, qm, q1) = (inv * p0, inv * Point::new(a, b), inv * pt);
                // Ellipse → circle: rotate the major axis onto x, squeeze it by the ratio.
                let d = if d.is_finite() && d.abs() > 1e-9 {
                    d
                } else {
                    1.0
                };
                let to_circle = Affine::scale_non_uniform(1.0 / d, 1.0) * Affine::rotate(-c);
                let back = norm * to_circle.inverse();
                arc3(&mut p, to_circle * q0, to_circle * qm, to_circle * q1, back);
                cur = Some(pt);
            }
            "Ellipse" => {
                let (a, b) = (row_num(r, "A"), row_num(r, "B"));
                let (c, d) = (row_num(r, "C"), row_num(r, "D"));
                let m = Affine::new([a - xx, b - yy, c - xx, d - yy, xx, yy]);
                let circle = kurbo::Circle::new(Point::ZERO, 1.0).to_path(1e-4);
                let mut e = circle;
                e.apply_affine(m);
                for el in e.elements() {
                    p.push(*el);
                }
                cur = None;
            }
            "RelCubBezTo" => {
                ensure(&mut p, &mut cur);
                let c1 = Point::new(row_num(r, "A") * w, row_num(r, "B") * h);
                let c2 = Point::new(row_num(r, "C") * w, row_num(r, "D") * h);
                p.curve_to(c1, c2, pt);
                cur = Some(pt);
            }
            "RelQuadBezTo" => {
                ensure(&mut p, &mut cur);
                let c1 = Point::new(row_num(r, "A") * w, row_num(r, "B") * h);
                p.quad_to(c1, pt);
                cur = Some(pt);
            }
            "PolylineTo" => {
                ensure(&mut p, &mut cur);
                for q in polyline_points(r, w, h) {
                    p.line_to(q);
                }
                p.line_to(pt);
                cur = Some(pt);
            }
            "NURBSTo" => {
                ensure(&mut p, &mut cur);
                let p0 = cur.unwrap_or(Point::ZERO);
                for q in nurbs_points(r, p0, pt, w, h) {
                    p.line_to(q);
                }
                p.line_to(pt);
                cur = Some(pt);
            }
            "SplineStart" => {
                ensure(&mut p, &mut cur);
                spline.push(pt);
                cur = Some(pt);
            }
            "SplineKnot" => {
                ensure(&mut p, &mut cur);
                spline.push(pt);
                cur = Some(pt);
            }
            "InfiniteLine" => skipped.geometry += 1,
            _ => {}
        }
        let _ = start;
    }
    flush_spline(&mut p, &mut spline);
    p
}

#[cfg(test)]
pub(crate) fn test_path(rows: &[Row], w: f64, h: f64) -> BezPath {
    build_path(&mut Skipped::default(), rows, w, h)
}

/// Appends the circular arc from `p0` through `pm` to `p1` (in a space `back` maps to
/// local coordinates); a line when the points are collinear.
fn arc3(p: &mut BezPath, p0: Point, pm: Point, p1: Point, back: Affine) {
    let (ax, ay) = (p0.x, p0.y);
    let (bx, by) = (pm.x, pm.y);
    let (cx, cy) = (p1.x, p1.y);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 || !d.is_finite() {
        p.line_to(back * p1);
        return;
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    let c = Point::new(ux, uy);
    let r = (p0 - c).hypot();
    if !r.is_finite() || r > 1e7 {
        p.line_to(back * p1);
        return;
    }
    let t0 = (p0.y - c.y).atan2(p0.x - c.x);
    let tm = (pm.y - c.y).atan2(pm.x - c.x);
    let t1 = (p1.y - c.y).atan2(p1.x - c.x);
    // Sweep from t0 to t1 passing tm.
    let norm = |a: f64| a.rem_euclid(2.0 * PI);
    let ccw_to_end = norm(t1 - t0);
    let ccw_to_mid = norm(tm - t0);
    let sweep = if ccw_to_mid <= ccw_to_end {
        ccw_to_end
    } else {
        ccw_to_end - 2.0 * PI
    };
    let arc = kurbo::Arc::new(c, (r, r), t0, sweep, 0.0);
    let mut first = true;
    arc.to_cubic_beziers(1e-4, |c1, c2, e| {
        let _ = first;
        first = false;
        p.curve_to(back * c1, back * c2, back * e);
    });
}

/// The numbers of a `POLYLINE(…)` / `NURBS(…)` formula in a row's `A` or `E` cell.
fn formula_args(r: &Row, cell: &str, func: &str) -> Vec<f64> {
    let Some(c) = r.cells.get(cell) else {
        return Vec::new();
    };
    for src in [Some(c.v.as_str()), c.f.as_deref()].into_iter().flatten() {
        let up = src.to_ascii_uppercase();
        if let Some(i) = up.find(func) {
            let rest = &src[i + func.len()..];
            let rest = rest.trim_start().trim_start_matches('(');
            let end = rest.find(')').unwrap_or(rest.len());
            let v: Vec<f64> = rest[..end]
                .split(',')
                .map(|x| x.trim().parse::<f64>().unwrap_or(0.0))
                .collect();
            if v.len() >= 2 {
                return v;
            }
        }
    }
    Vec::new()
}

fn polyline_points(r: &Row, w: f64, h: f64) -> Vec<Point> {
    let a = formula_args(r, "A", "POLYLINE");
    if a.len() < 2 {
        return Vec::new();
    }
    let (xrel, yrel) = (a[0] == 0.0, a[1] == 0.0);
    a[2..]
        .chunks_exact(2)
        .take(100_000)
        .map(|c| {
            Point::new(
                if xrel { c[0] * w } else { c[0] },
                if yrel { c[1] * h } else { c[1] },
            )
        })
        .collect()
}

/// Samples a NURBS segment: control points from the current point, the formula's points
/// and the row's end point; knots and weights as the row gives them.
fn nurbs_points(r: &Row, p0: Point, p1: Point, w: f64, h: f64) -> Vec<Point> {
    let e = formula_args(r, "E", "NURBS");
    if e.len() < 4 {
        return Vec::new();
    }
    let knot_last = e[0];
    let degree = (e[1].round() as usize).clamp(1, 10);
    let (xrel, yrel) = (e[2] == 0.0, e[3] == 0.0);
    let mut pts = vec![p0];
    let mut knots = vec![r.num("C").unwrap_or(0.0)];
    let mut weights = vec![r.num("D").unwrap_or(1.0)];
    for c in e[4..].chunks_exact(4).take(10_000) {
        pts.push(Point::new(
            if xrel { c[0] * w } else { c[0] },
            if yrel { c[1] * h } else { c[1] },
        ));
        knots.push(c[2]);
        weights.push(c[3]);
    }
    pts.push(p1);
    knots.push(r.num("A").unwrap_or(knot_last));
    weights.push(r.num("B").unwrap_or(1.0));
    let n = pts.len();
    if n <= degree {
        return pts[1..n - 1].to_vec();
    }
    // Clamp: the first knot repeated `degree` more times, the last once more.
    let mut k = vec![knots[0]; degree];
    k.extend_from_slice(&knots);
    while k.len() < n + degree + 1 {
        k.push(*knots.last().unwrap_or(&1.0));
    }
    let (t0, t1) = (k[degree], k[n]);
    if t1.partial_cmp(&t0) != Some(std::cmp::Ordering::Greater)
        || weights.iter().any(|w| !w.is_finite())
    {
        return pts[1..n - 1].to_vec();
    }
    let samples = (n * 12).min(4096);
    let mut out = Vec::with_capacity(samples);
    for i in 1..samples {
        let t = t0 + (t1 - t0) * i as f64 / samples as f64;
        // The knot span.
        let mut s = degree;
        while s + 1 < n && k[s + 1] <= t {
            s += 1;
        }
        // de Boor in homogeneous coordinates.
        let mut d: Vec<(f64, f64, f64)> = (0..=degree)
            .map(|j| {
                let q = pts[s - degree + j];
                let wt = weights[s - degree + j];
                (q.x * wt, q.y * wt, wt)
            })
            .collect();
        for rr in 1..=degree {
            for j in (rr..=degree).rev() {
                let i0 = s - degree + j;
                let den = k[i0 + degree + 1 - rr] - k[i0];
                let a = if den.abs() < 1e-12 {
                    0.0
                } else {
                    (t - k[i0]) / den
                };
                d[j] = (
                    (1.0 - a) * d[j - 1].0 + a * d[j].0,
                    (1.0 - a) * d[j - 1].1 + a * d[j].1,
                    (1.0 - a) * d[j - 1].2 + a * d[j].2,
                );
            }
        }
        let (x, y, wt) = d[degree];
        if wt.abs() > 1e-12 && (x / wt).is_finite() && (y / wt).is_finite() {
            out.push(Point::new(x / wt, y / wt));
        }
    }
    out
}

/// Subpaths of `p` whose ends meet, closed (what Visio fills).
fn closed_part(p: &BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut cur: Vec<kurbo::PathEl> = Vec::new();
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    let bb = p.bounding_box();
    let eps = (bb.width().abs() + bb.height().abs()) * 1e-6 + 1e-9;
    let mut flush = |cur: &mut Vec<kurbo::PathEl>, start: Point, last: Point, closed: bool| {
        if cur.len() > 1 && (closed || (start - last).hypot() <= eps) {
            for el in cur.iter() {
                out.push(*el);
            }
            out.close_path();
        }
        cur.clear();
    };
    for el in p.elements() {
        match *el {
            kurbo::PathEl::MoveTo(q) => {
                flush(&mut cur, start, last, false);
                start = q;
                last = q;
                cur.push(*el);
            }
            kurbo::PathEl::LineTo(q)
            | kurbo::PathEl::QuadTo(_, q)
            | kurbo::PathEl::CurveTo(_, _, q) => {
                last = q;
                cur.push(*el);
            }
            kurbo::PathEl::ClosePath => {
                flush(&mut cur, start, last, true);
                last = start;
            }
        }
    }
    flush(&mut cur, start, last, false);
    out
}

fn dashes(pattern: i64, w: f64) -> Vec<f64> {
    let u = w.max(0.75);
    let v: &[f64] = match pattern {
        2 => &[4.0, 3.0],
        3 => &[1.0, 2.0],
        4 => &[4.0, 2.0, 1.0, 2.0],
        5 => &[4.0, 2.0, 1.0, 2.0, 1.0, 2.0],
        6 => &[8.0, 3.0],
        7 => &[8.0, 3.0, 1.0, 3.0],
        8 => &[8.0, 3.0, 1.0, 3.0, 1.0, 3.0],
        9 => &[2.0, 2.0],
        10 => &[1.0, 1.0],
        11..=23 => &[3.0, 2.0],
        _ => &[],
    };
    v.iter().map(|x| x * u).collect()
}

fn geometry(ctx: &mut Ctx, env: &PageEnv, ch: &Chain, mt: Affine, ops: &mut Vec<Op>) {
    let gs = geoms(ctx, ch);
    if gs.is_empty() {
        return;
    }
    // Fill.
    let pattern = if ctx.themed(ch, "FillPattern") {
        match ctx.theme_fill_style(ch, env) {
            Some(FillStyle::None) => 0,
            Some(FillStyle::Grad(..)) => -1,
            _ => 1,
        }
    } else {
        ctx.num(ch, "FillPattern").map_or(1, |v| v as i64)
    };
    let mut fill = BezPath::new();
    for g in gs.iter().filter(|g| g.fill) {
        for el in closed_part(&g.path).elements() {
            fill.push(*el);
        }
    }
    if pattern != 0 && !fill.elements().is_empty() {
        let local_bb = fill.bounding_box();
        if let Some(paint) = fill_paint(ctx, env, ch, pattern, local_bb, mt) {
            let mut path = fill.clone();
            path.apply_affine(mt);
            ops.push(Op::Fill {
                path,
                even_odd: true,
                paint,
            });
        }
    }
    // Lines.
    let lp = if ctx.themed(ch, "LinePattern") {
        let m = ctx.quick(ch, "QuickStyleLineMatrix", 100);
        let i = ctx.theme.matrix(m, env.var_style, |v| v.line);
        match i
            .checked_sub(1)
            .and_then(|i| ctx.scheme(ch).lines.get(i))
            .and_then(|l| l.dash.as_deref())
        {
            Some("dash" | "sysDash") => 2,
            Some("dot" | "sysDot") => 3,
            Some("dashDot" | "sysDashDot") => 4,
            Some("sysDashDotDot") => 5,
            Some("lgDash") => 6,
            Some("lgDashDot") => 7,
            Some("lgDashDotDot") => 8,
            _ => 1,
        }
    } else {
        ctx.num(ch, "LinePattern").map_or(1, |v| v as i64)
    };
    if lp == 0 {
        return;
    }
    let Some(color) = ctx.color(ch, env, "LineColor", Themed::Line) else {
        return;
    };
    if color[3] == 0 {
        return;
    }
    let weight_in = if ctx.themed(ch, "LineWeight") {
        let m = ctx.quick(ch, "QuickStyleLineMatrix", 100);
        let i = ctx.theme.matrix(m, env.var_style, |v| v.line);
        i.checked_sub(1)
            .and_then(|i| ctx.scheme(ch).lines.get(i))
            .and_then(|l| l.width)
            .map_or(0.75 / PT, |w| w / PT)
    } else {
        ctx.numd(ch, "LineWeight", 0.75 / PT)
    };
    let scale = (mt.determinant().abs()).sqrt();
    let w_pt = weight_in * scale;
    let cap = match ctx.num(ch, "LineCap").map(|v| v as i64) {
        Some(1) | Some(2) => kurbo::Cap::Square,
        _ => kurbo::Cap::Round,
    };
    let mut stroke = Stroke::new(w_pt.max(0.0))
        .with_caps(cap)
        .with_join(kurbo::Join::Round);
    let d = dashes(lp, w_pt);
    if !d.is_empty() {
        stroke = stroke.with_dashes(0.0, d);
    }
    let mut first_last: Option<(BezPath, bool)> = None;
    for g in gs.iter().filter(|g| g.line) {
        let mut path = g.path.clone();
        path.apply_affine(mt);
        if first_last.is_none() {
            first_last = Some((path.clone(), true));
        }
        ops.push(Op::Stroke {
            path,
            stroke: stroke.clone(),
            paint: Paint::Solid(color),
        });
    }
    // Arrowheads on the first stroked section's ends.
    if let Some((path, _)) = first_last {
        let begin = ctx.num(ch, "BeginArrow").map_or(0, |v| v as i64);
        let end = ctx.num(ch, "EndArrow").map_or(0, |v| v as i64);
        let bsize = ctx.numd(ch, "BeginArrowSize", 2.0);
        let esize = ctx.numd(ch, "EndArrowSize", 2.0);
        if begin > 0
            && let Some((tip, dir)) = path_end(&path, false)
        {
            arrow(ops, tip, dir, begin, bsize, w_pt, color);
        }
        if end > 0
            && let Some((tip, dir)) = path_end(&path, true)
        {
            arrow(ops, tip, dir, end, esize, w_pt, color);
        }
    }
}

fn fill_paint(
    ctx: &Ctx,
    env: &PageEnv,
    ch: &Chain,
    pattern: i64,
    bb: Rect,
    mt: Affine,
) -> Option<Paint> {
    let fg = ctx
        .color(ch, env, "FillForegnd", Themed::Fill)
        .unwrap_or([255, 255, 255, 255]);
    let bg = ctx
        .color(ch, env, "FillBkgnd", Themed::FillBack)
        .unwrap_or([255, 255, 255, 255]);
    // Visio 2013 gradients.
    if ctx.num(ch, "FillGradientEnabled").unwrap_or(0.0) != 0.0 {
        let rows = ctx.rows(ch, "FillGradient", None, Some(Kind::Fill));
        let mut stops: Vec<Stop> = rows
            .iter()
            .filter_map(|r| {
                let c = r.cells.get("GradientStopColor")?;
                let mut col = ctx.parse_color(&c.v).or_else(|| {
                    (c.v == "Themed").then(|| ctx.theme_color(ch, env, Themed::Fill))
                })?;
                if let Some(t) = r.num("GradientStopColorTrans") {
                    col[3] = ((1.0 - t.clamp(0.0, 1.0)) * 255.0).round() as u8;
                }
                Some(Stop {
                    offset: r.num("GradientStopPosition").unwrap_or(0.0) as f32,
                    color: col,
                })
            })
            .collect();
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        if stops.len() >= 2 {
            let angle = ctx.numd(ch, "FillGradientAngle", 0.0);
            let dir = ctx.numd(ch, "FillGradientDir", 0.0) as i64;
            return Some(gradient(stops, bb, mt, angle, dir != 0));
        }
    }
    let solid = |c: Rgba| (c[3] > 0).then_some(Paint::Solid(c));
    match pattern {
        -1 => {
            if let Some(FillStyle::Grad(gs, ang)) = ctx.theme_fill_style(ch, env) {
                let ph = ctx
                    .theme
                    .quick_color(ctx.quick(ch, "QuickStyleFillColor", 100), env.variant)
                    .unwrap_or([255, 255, 255]);
                let stops: Vec<Stop> = gs
                    .iter()
                    .map(|(o, s)| Stop {
                        offset: *o,
                        color: ctx.theme.resolve(s, ph),
                    })
                    .collect();
                // DrawingML angles run clockwise in y-down space; local space is y up.
                return Some(gradient(stops, bb, mt, -ang.to_radians(), false));
            }
            solid(fg)
        }
        1 => solid(fg),
        2..=24 => {
            let mix = |a: u8, b: u8| ((u16::from(a) + u16::from(b)) / 2) as u8;
            solid([
                mix(fg[0], bg[0]),
                mix(fg[1], bg[1]),
                mix(fg[2], bg[2]),
                fg[3].max(bg[3]),
            ])
        }
        25..=40 => {
            let stops = vec![
                Stop {
                    offset: 0.0,
                    color: fg,
                },
                Stop {
                    offset: 1.0,
                    color: bg,
                },
            ];
            let (angle, radial) = match pattern {
                25 => (0.0, false),
                26 | 27 => (PI, false),
                28 => (-PI / 2.0, false),
                29 | 30 => (PI / 2.0, false),
                _ => (0.0, true),
            };
            Some(gradient(stops, bb, mt, angle, radial))
        }
        _ => solid(fg),
    }
}

/// A gradient over the local box `bb`, at `angle` (radians, local y-up space), mapped to
/// the page by `mt`.
fn gradient(stops: Vec<Stop>, bb: Rect, mt: Affine, angle: f64, radial: bool) -> Paint {
    let c = bb.center();
    if radial {
        let r = (bb.width().hypot(bb.height()) / 2.0).max(1e-9);
        return Paint::Radial {
            transform: mt * Affine::translate(c.to_vec2()) * Affine::scale(r),
            stops,
        };
    }
    let dir = Vec2::new(angle.cos(), angle.sin());
    let half = (bb.width() * dir.x.abs() + bb.height() * dir.y.abs()) / 2.0;
    Paint::Linear {
        start: mt * (c - dir * half),
        end: mt * (c + dir * half),
        stops,
    }
}

/// An end of a path's first subpath (or the last subpath's end) and the direction into it.
fn path_end(p: &BezPath, end: bool) -> Option<(Point, Vec2)> {
    let segs: Vec<kurbo::PathSeg> = p.segments().collect();
    let s = if end { segs.last()? } else { segs.first()? };
    let (tip, from) = match (*s, end) {
        (kurbo::PathSeg::Line(l), true) => (l.p1, l.p0),
        (kurbo::PathSeg::Line(l), false) => (l.p0, l.p1),
        (kurbo::PathSeg::Quad(q), true) => (q.p2, q.p1),
        (kurbo::PathSeg::Quad(q), false) => (q.p0, q.p1),
        (kurbo::PathSeg::Cubic(c), true) => (c.p3, if c.p2 == c.p3 { c.p1 } else { c.p2 }),
        (kurbo::PathSeg::Cubic(c), false) => (c.p0, if c.p1 == c.p0 { c.p2 } else { c.p1 }),
    };
    let d = tip - from;
    let len = d.hypot();
    (len > 1e-9).then(|| (tip, d / len))
}

fn arrow(ops: &mut Vec<Op>, tip: Point, dir: Vec2, kind: i64, size: f64, w: f64, c: Rgba) {
    let len = (4.0 + 2.0 * size.clamp(0.0, 6.0)) + w * 2.0;
    let half = len * 0.4;
    let n = Vec2::new(-dir.y, dir.x);
    let base = tip - dir * len;
    let mut p = BezPath::new();
    p.move_to(base + n * half);
    p.line_to(tip);
    p.line_to(base - n * half);
    if kind == 1 {
        ops.push(Op::Stroke {
            path: p,
            stroke: Stroke::new(w.max(0.5)).with_join(kurbo::Join::Miter),
            paint: Paint::Solid(c),
        });
    } else {
        p.close_path();
        ops.push(Op::Fill {
            path: p,
            even_odd: false,
            paint: Paint::Solid(c),
        });
    }
}

// ----- images -------------------------------------------------------------------------

fn foreign(
    ctx: &mut Ctx,
    ch: &Chain,
    mt: Affine,
    ops: &mut Vec<Op>,
    rels: &HashMap<String, String>,
    master: Option<u32>,
) {
    // The foreign data of the instance, else its master shape's (in the master's part).
    let mut found: Option<(String, bool)> = None;
    for (i, s) in ch.levels.iter().enumerate() {
        if let Some(r) = &s.foreign {
            found = Some((r.clone(), i > 0));
            break;
        }
    }
    let Some((rid, from_master)) = found else {
        return;
    };
    let part = if from_master {
        master
            .and_then(|m| ctx.masters.get(&m))
            .and_then(|m| m.rels.get(&rid))
            .cloned()
    } else {
        rels.get(&rid).cloned()
    };
    let Some(part) = part else {
        ctx.skipped.images += 1;
        return;
    };
    let img = match ctx.images.get(&part) {
        Some(i) => i.clone(),
        None => {
            let img = (ctx.media)(&part).and_then(|bytes| {
                let dec = ctx.decode?;
                let img = Image::decode(dec, &bytes)?;
                let px = u64::from(img.width) * u64::from(img.height);
                if ctx.image_pixels + px > specular_drawpage::MAX_IMAGE_PIXELS {
                    return None;
                }
                ctx.image_pixels += px;
                Some(Arc::new(img))
            });
            ctx.images.insert(part.clone(), img.clone());
            img
        }
    };
    let Some(img) = img else {
        ctx.skipped.images += 1;
        return;
    };
    let w = ctx.numd(ch, "Width", 0.0);
    let h = ctx.numd(ch, "Height", 0.0);
    let ox = ctx.numd(ch, "ImgOffsetX", 0.0);
    let oy = ctx.numd(ch, "ImgOffsetY", 0.0);
    let iw = ctx.numd(ch, "ImgWidth", w);
    let ih = ctx.numd(ch, "ImgHeight", h);
    let (pw, ph) = (f64::from(img.width), f64::from(img.height));
    // Image pixels (y down) → local (y up).
    let to_local = Affine::new([iw / pw, 0.0, 0.0, -ih / ph, ox, oy + ih]);
    // Clipped to the shape's box (crops).
    let clip = Rect::new(0.0, 0.0, w, h).to_path(1e-6);
    let mut clip_page = clip;
    clip_page.apply_affine(mt);
    ops.push(Op::PushClip {
        path: clip_page,
        even_odd: false,
    });
    ops.push(Op::Image {
        image: img,
        transform: mt * to_local,
        opacity: 1.0,
    });
    ops.push(Op::Pop);
}

// ----- text ---------------------------------------------------------------------------

fn font_name(ctx: &Ctx, v: &str) -> String {
    let v = v.trim();
    let name = if v == "Themed" || v.is_empty() {
        ctx.theme
            .minor_font
            .clone()
            .unwrap_or_else(|| "Calibri".into())
    } else if let Ok(i) = v.parse::<u32>() {
        ctx.faces.get(&i).cloned().unwrap_or_else(|| "Arial".into())
    } else {
        v.to_string()
    };
    sans_stand_in(name)
}

/// Sans families Office themes use whose names read like serif ones ("Franklin Gothic
/// Book"): when not installed, named as a sans so the stand-in is one.
fn sans_stand_in(name: String) -> String {
    let lower = name.to_ascii_lowercase();
    let sans = [
        "gothic",
        "franklin",
        "segoe",
        "verdana",
        "tahoma",
        "corbel",
        "candara",
        "trebuchet",
    ];
    if !sans.iter().any(|s| lower.contains(s)) {
        return name;
    }
    let fonts = specular_vector::fonts::Fonts::shared();
    let spec = specular_vector::scene::FontSpec::default();
    if fonts.resolve(&spec, Some(&name), false, false).substituted {
        "Arial".into()
    } else {
        name
    }
}

fn text(ctx: &mut Ctx, env: &PageEnv, ch: &Chain, mt: Affine, ops: &mut Vec<Op>) {
    let Some(pieces) = ch.levels.iter().find_map(|s| s.text.as_ref()) else {
        return;
    };
    if !pieces.iter().any(|p| match p {
        Piece::Text(t) | Piece::Field(t) => !t.trim().is_empty(),
        _ => false,
    }) {
        return;
    }
    if ctx.numd(ch, "HideText", 0.0) != 0.0 {
        return;
    }
    let chars = ctx.rows(ch, "Character", None, Some(Kind::Text));
    let paras = ctx.rows(ch, "Paragraph", None, Some(Kind::Text));
    let char_row = |i: u32| {
        chars
            .iter()
            .find(|r| r.key == i.to_string())
            .or_else(|| chars.iter().find(|r| r.key == "0"))
    };
    let para_row = |i: u32| {
        paras
            .iter()
            .find(|r| r.key == i.to_string())
            .or_else(|| paras.iter().find(|r| r.key == "0"))
    };
    let style_run = |ctx: &Ctx, i: u32, text: String| -> Run {
        let r = char_row(i);
        let cell = |n: &str| r.and_then(|r| r.cells.get(n));
        let num = |n: &str| cell(n).and_then(Cell::num);
        let family = font_name(ctx, cell("Font").map_or("Themed", |c| c.v.as_str()));
        let size = num("Size").unwrap_or(12.0 / PT) * PT;
        let style = num("Style").unwrap_or(0.0) as i64;
        let mut color = match cell("Color") {
            Some(c) if c.v == "Themed" => ctx.theme_color(ch, env, Themed::Font),
            Some(c) => ctx.parse_color(&c.v).unwrap_or([0, 0, 0, 255]),
            None => [0, 0, 0, 255],
        };
        if let Some(t) = num("ColorTrans") {
            color[3] = ((1.0 - t.clamp(0.0, 1.0)) * 255.0).round() as u8;
        }
        let mut run = Run::new(text, family, size);
        run.bold = style & 1 != 0;
        run.italic = style & 2 != 0;
        run.underline = style & 4 != 0 || num("DoubleUnderline").unwrap_or(0.0) != 0.0;
        run.strike = num("Strikethru").unwrap_or(0.0) != 0.0;
        run.color = color;
        if style & 8 != 0 {
            run.text = run.text.to_uppercase();
        }
        run
    };
    let make_para = |pi: u32| -> Paragraph {
        let r = para_row(pi);
        let num = |n: &str| r.and_then(|r| r.num(n));
        let align = match num("HorzAlign").map(|v| v as i64) {
            Some(0) => HAlign::Left,
            Some(2) => HAlign::Right,
            Some(3) | Some(4) => HAlign::Justify,
            _ => HAlign::Centre,
        };
        let sp = num("SpLine").unwrap_or(-1.2);
        let (line_spacing, line_height) = if sp < 0.0 {
            (-sp, None)
        } else if sp > 0.0 {
            (0.0, Some(sp * PT))
        } else {
            (1.2, None)
        };
        let bullet = match num("Bullet").map(|v| v as i64) {
            Some(b) if b > 0 => Some(
                r.and_then(|r| r.cells.get("BulletStr"))
                    .map(|c| c.v.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "\u{2022}".into()),
            ),
            _ => None,
        };
        Paragraph {
            runs: Vec::new(),
            align,
            line_spacing,
            line_height,
            space_before: num("SpBefore").unwrap_or(0.0) * PT,
            space_after: num("SpAfter").unwrap_or(0.0) * PT,
            indent_left: num("IndLeft").unwrap_or(0.0) * PT,
            indent_right: num("IndRight").unwrap_or(0.0) * PT,
            indent_first: num("IndFirst").unwrap_or(0.0) * PT,
            bullet,
        }
    };
    let (mut cp, mut pp) = (0u32, 0u32);
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut cur = make_para(0);
    let push_text = |ctx: &Ctx,
                     s: &str,
                     cp: u32,
                     pp: &mut u32,
                     cur: &mut Paragraph,
                     paragraphs: &mut Vec<Paragraph>,
                     next_pp: u32| {
        let mut first = true;
        for part in s.split('\n') {
            if !first {
                let done = std::mem::replace(cur, make_para(next_pp.max(*pp)));
                paragraphs.push(done);
            }
            first = false;
            if !part.is_empty() {
                cur.runs
                    .push(style_run(ctx, cp, part.replace('\u{2028}', "\n")));
            }
        }
    };
    for p in pieces {
        match p {
            Piece::Cp(i) => cp = *i,
            Piece::Pp(i) => {
                pp = *i;
                if cur.runs.is_empty() {
                    cur = make_para(pp);
                }
            }
            Piece::Tab => cur.runs.push(style_run(ctx, cp, "\t".into())),
            Piece::Text(t) | Piece::Field(t) => {
                let np = pp;
                push_text(ctx, t, cp, &mut pp, &mut cur, &mut paragraphs, np);
            }
        }
        if paragraphs.len() > 10_000 {
            break;
        }
    }
    if !cur.runs.is_empty() {
        paragraphs.push(cur);
    }
    // Text block geometry (inches, local space).
    let w = ctx.numd(ch, "Width", 0.0);
    let h = ctx.numd(ch, "Height", 0.0);
    let tw = ctx.numd(ch, "TxtWidth", w);
    let th = ctx.numd(ch, "TxtHeight", h);
    let tpx = ctx.numd(ch, "TxtPinX", w / 2.0);
    let tpy = ctx.numd(ch, "TxtPinY", h / 2.0);
    let tlx = ctx.numd(ch, "TxtLocPinX", tw / 2.0);
    let tly = ctx.numd(ch, "TxtLocPinY", th / 2.0);
    let ta = ctx.numd(ch, "TxtAngle", 0.0);
    let ml = ctx.numd(ch, "LeftMargin", 4.0 / PT);
    let mr = ctx.numd(ch, "RightMargin", 4.0 / PT);
    let mtp = ctx.numd(ch, "TopMargin", 4.0 / PT);
    let mb = ctx.numd(ch, "BottomMargin", 4.0 / PT);
    let valign = match ctx.num(ch, "VerticalAlign").map(|v| v as i64) {
        Some(0) => VAlign::Top,
        Some(2) => VAlign::Bottom,
        _ => VAlign::Middle,
    };
    let bw = ((tw - ml - mr) * PT).max(0.0);
    let bh = ((th - mtp - mb) * PT).max(0.0);
    let to_text =
        mt * Affine::translate((tpx, tpy)) * Affine::rotate(ta) * Affine::translate((-tlx, -tly));
    // Box space (points, y down) → text-local (inches, y up).
    let mut m = to_text * Affine::new([1.0 / PT, 0.0, 0.0, -1.0 / PT, ml, th - mtp]);
    // Mirrored (a flipped shape): keep the text readable.
    if m.determinant() < 0.0 {
        m *= Affine::new([-1.0, 0.0, 0.0, 1.0, bw, 0.0]);
    }
    let mut block = TextBlock::new(m, bw, bh);
    block.valign = valign;
    block.wrap = bw > 1.0;
    block.paragraphs = paragraphs;
    if block.is_empty() {
        return;
    }
    // Text background: behind the text's extent.
    if let Some(c) = ctx.cell(ch, "TextBkgnd") {
        let col = if c.v.starts_with('#') {
            ctx.parse_color(&c.v)
        } else {
            c.num()
                .map(|i| i as i64)
                .filter(|&i| (1..=24).contains(&i))
                .and_then(|i| ctx.parse_color(&(i - 1).to_string()))
        };
        if let Some(mut col) = col {
            if let Some(t) = ctx.num(ch, "TextBkgndTrans") {
                col[3] = ((1.0 - t.clamp(0.0, 1.0)) * 255.0).round() as u8;
            }
            if col[3] > 0 {
                let mut path = text_extent(&block).to_path(1e-6);
                path.apply_affine(m);
                ops.push(Op::Fill {
                    path,
                    even_odd: false,
                    paint: Paint::Solid(col),
                });
            }
        }
    }
    ops.push(Op::Text(Box::new(block)));
}

/// An estimate of the laid-out text's box (box space, points): natural line widths from the
/// fonts' advances, wrapped at the box width, placed as the alignment says.
fn text_extent(b: &TextBlock) -> Rect {
    use specular_vector::fonts::Fonts;
    let fonts = Fonts::shared();
    let spec = specular_vector::scene::FontSpec::default();
    let mut lines: Vec<(f64, f64, HAlign)> = Vec::new();
    for p in &b.paragraphs {
        let mut w = 0.0;
        let mut size: f64 = 0.0;
        for r in &p.runs {
            let face = fonts.resolve(&spec, Some(&r.family), r.bold, r.italic).face;
            let k = r.size / f64::from(face.units_per_em.max(1.0));
            size = size.max(r.size);
            for c in r.text.chars() {
                w += face
                    .glyph(c)
                    .map_or(f64::from(face.units_per_em) * 0.5, |g| f64::from(g.advance))
                    * k;
            }
        }
        let lh = size
            * if p.line_spacing > 0.0 {
                p.line_spacing
            } else {
                1.2
            };
        let avail = b.width.max(1.0);
        let n = if b.wrap {
            (w / avail).ceil().max(1.0)
        } else {
            1.0
        };
        lines.push((w.min(avail), lh * n, p.align));
    }
    let total: f64 = lines.iter().map(|l| l.1).sum();
    let width = lines.iter().map(|l| l.0).fold(0.0, f64::max);
    let align = lines.first().map_or(HAlign::Centre, |l| l.2);
    let x0 = match align {
        HAlign::Left | HAlign::Justify => 0.0,
        HAlign::Centre => (b.width - width) / 2.0,
        HAlign::Right => b.width - width,
    };
    let y0 = match b.valign {
        VAlign::Top => 0.0,
        VAlign::Middle => (b.height - total) / 2.0,
        VAlign::Bottom => b.height - total,
    };
    Rect::new(x0, y0, x0 + width, y0 + total)
}
