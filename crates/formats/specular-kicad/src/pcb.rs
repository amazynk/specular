//! KiCad boards (`.kicad_pcb`, KiCad 6 and later) as a vector scene: one sheet, one layer per
//! KiCad layer in use in KiCad's default colours, drawn bottom to top (back layers, inner
//! copper, front layers, then vias, drill holes and the board outline). Shapes with a width
//! are filled outlines, so they keep their true width at any zoom.
//!
//! Positions in footprints are relative to the footprint and turned with it; pad and text
//! angles are stored absolute, as KiCad writes them. Back-side footprints are already
//! flipped in the file; their text carries `justify mirror`.

use std::collections::{BTreeSet, HashMap};

use specular_vector::kurbo::{Affine, BezPath, Circle, Point, Rect, Shape as _};
use specular_vector::{
    Color, FontSpec, Item, Layer, PcbInfo, PcbKind, PcbLayer, Shape, Sheet, SheetKind, Units,
    VectorScene,
};

use crate::geom::{
    self, Effects, TOLERANCE, at, capsule, child_xy, outline, place, pt, pts_path, text_block,
};
use crate::sexpr::{self, Node};
use crate::{BOARD_V6_DEV, Error, LEGACY_BOARD, Limits, board_release, legacy_kind, warn};

/// The drill holes' pseudo-layer.
pub const HOLES: &str = "Holes";
/// The vias' pseudo-layer.
pub const VIAS: &str = "Vias";

/// Reads a board.
pub fn scene(bytes: &[u8], limits: &Limits) -> Result<VectorScene, Error> {
    if let Some(kind) = legacy_kind(bytes) {
        return Err(if kind.contains("board") {
            Error::Unsupported(LEGACY_BOARD.into())
        } else {
            Error::NotThisFormat
        });
    }
    let text = crate::text_of(bytes);
    let root = sexpr::parse_with_depth(&text, limits.depth)?;
    if !root.is("kicad_pcb") {
        return Err(Error::NotThisFormat);
    }
    let version = root.value_num("version").unwrap_or(0.0) as u64;
    if version < BOARD_V6_DEV {
        return Err(Error::Unsupported(LEGACY_BOARD.into()));
    }
    let mut b = Board::new(&root, *limits);
    b.read(&root);
    Ok(b.finish(&root, version))
}

/// The copper of every pad and via, by layer name ("F.Cu", "Vias"), without anything else:
/// for checking where tracks end.
pub fn pad_copper(bytes: &[u8]) -> Result<Vec<(String, BezPath)>, Error> {
    let text = crate::text_of(bytes);
    let root = sexpr::parse(&text)?;
    if !root.is("kicad_pcb") {
        return Err(Error::NotThisFormat);
    }
    let mut b = Board::new(&root, Limits::default());
    b.pads_only = true;
    b.read(&root);
    Ok(b.slots
        .into_iter()
        .flat_map(|s| {
            let name = s.name;
            s.items.into_iter().filter_map(move |it| match it.shape {
                Shape::Fill { path, .. } => Some((name.clone(), path)),
                _ => None,
            })
        })
        .collect())
}

/// Where a layer sits in the drawing order (bottom first) and its colour.
fn rank(name: &str) -> i32 {
    if let Some(n) = inner_number(name) {
        return 200 - n as i32;
    }
    match name {
        "B.Fab" => 10,
        "B.CrtYd" => 20,
        "B.Adhes" => 30,
        "B.Paste" => 40,
        "B.SilkS" => 50,
        "B.Mask" => 60,
        "B.Cu" => 100,
        "F.Cu" => 300,
        VIAS => 310,
        "F.Mask" => 320,
        "F.Paste" => 330,
        "F.Adhes" => 340,
        "F.SilkS" => 350,
        "F.CrtYd" => 360,
        "F.Fab" => 370,
        "Dwgs.User" => 400,
        "Cmts.User" => 410,
        "Eco1.User" => 420,
        "Eco2.User" => 430,
        "Margin" => 480,
        HOLES => 490,
        "Edge.Cuts" => 500,
        _ => match name
            .strip_prefix("User.")
            .and_then(|n| n.parse::<i32>().ok())
        {
            Some(n) => 440 + n.min(39),
            None => 470,
        },
    }
}

fn inner_number(name: &str) -> Option<u32> {
    name.strip_prefix("In")?.strip_suffix(".Cu")?.parse().ok()
}

/// KiCad's default ("KiCad Default" theme) colour of a layer.
pub fn layer_colour(name: &str) -> [u8; 3] {
    const INNER: [[u8; 3]; 12] = [
        [127, 200, 127],
        [206, 125, 44],
        [79, 203, 203],
        [219, 98, 139],
        [167, 165, 198],
        [40, 204, 217],
        [232, 178, 167],
        [242, 237, 161],
        [141, 203, 129],
        [237, 124, 51],
        [91, 195, 235],
        [247, 111, 142],
    ];
    if let Some(n) = inner_number(name) {
        return INNER[(n.max(1) as usize - 1) % INNER.len()];
    }
    match name {
        "F.Cu" => [200, 52, 52],
        "B.Cu" => [77, 127, 196],
        "F.Adhes" => [132, 0, 132],
        "B.Adhes" => [0, 0, 132],
        "F.Paste" => [180, 160, 154],
        "B.Paste" => [0, 194, 194],
        "F.SilkS" => [242, 237, 161],
        "B.SilkS" => [232, 178, 167],
        "F.Mask" => [216, 100, 255],
        "B.Mask" => [2, 255, 238],
        "Cmts.User" => [89, 148, 220],
        "Eco1.User" => [180, 219, 210],
        "Eco2.User" => [216, 200, 82],
        "Edge.Cuts" => [208, 210, 205],
        "Margin" | "F.CrtYd" => [255, 38, 226],
        "B.CrtYd" => [38, 233, 255],
        "F.Fab" => [175, 175, 175],
        "B.Fab" => [88, 93, 132],
        VIAS => [236, 236, 236],
        HOLES => [227, 183, 46],
        _ => [194, 194, 194],
    }
}

/// How opaque a layer's shapes are (KiCad draws masks translucent).
fn opacity(name: &str) -> f32 {
    if name.ends_with(".Mask") {
        0.4
    } else if name.ends_with(".Cu") {
        0.85
    } else if name.ends_with(".Paste") {
        0.9
    } else {
        1.0
    }
}

/// What a layer is, in English.
pub fn layer_role(name: &str) -> String {
    if let Some(n) = inner_number(name) {
        return format!("Inner copper {n}");
    }
    let side = |s: &str| if s == "F" { "Top" } else { "Bottom" };
    if let Some((s, kind)) = name.split_once('.')
        && (s == "F" || s == "B")
    {
        let what = match kind {
            "Cu" => "copper",
            "SilkS" => "silkscreen",
            "Mask" => "solder mask",
            "Paste" => "solder paste",
            "Adhes" => "adhesive",
            "CrtYd" => "courtyard",
            "Fab" => "fabrication",
            _ => kind,
        };
        return format!("{} {what}", side(s));
    }
    match name {
        "Edge.Cuts" => "Board outline".into(),
        "Margin" => "Margin".into(),
        "Dwgs.User" => "Drawings".into(),
        "Cmts.User" => "Comments".into(),
        "Eco1.User" => "Engineering change 1".into(),
        "Eco2.User" => "Engineering change 2".into(),
        VIAS => "Vias".into(),
        HOLES => "Drill holes".into(),
        _ => match name.strip_prefix("User.") {
            Some(n) => format!("User {n}"),
            None => name.into(),
        },
    }
}

struct Slot {
    name: String,
    items: Vec<Item>,
}

#[derive(Default)]
struct Counts {
    footprints: u64,
    pads: u64,
    segments: u64,
    arcs: u64,
    vias: u64,
    zones: u64,
    texts: u64,
    graphics: u64,
    dimensions: u64,
    holes: u64,
}

struct Board {
    limits: Limits,
    slots: Vec<Slot>,
    index: HashMap<String, usize>,
    /// Copper layers of the stack, front to back as listed.
    copper: Vec<String>,
    /// Names shown for layers that have a user name.
    shown: HashMap<String, String>,
    items: usize,
    truncated: bool,
    warnings: Vec<String>,
    counts: Counts,
    /// Drill diameters in µm (oval holes by their smaller side).
    drills: BTreeSet<u64>,
    /// Centre-line bounds of the outline, and of everything.
    edge: Option<Rect>,
    bounds: Option<Rect>,
    /// Only pads and vias are read ([`pad_copper`]).
    pads_only: bool,
}

fn union(a: Option<Rect>, b: Rect) -> Option<Rect> {
    if !(b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()) {
        return a;
    }
    Some(a.map_or(b, |a| a.union(b)))
}

/// A line width: `(stroke (width w))` (KiCad 7 on) or `(width w)`.
fn width_of(n: &Node<'_>) -> f64 {
    n.child("stroke")
        .and_then(|s| s.value_num("width"))
        .or_else(|| n.value_num("width"))
        .unwrap_or(0.0)
        .max(0.0)
}

/// Whether a shape is filled: `(fill solid)`, `(fill yes)`, `(fill (type …))`.
fn filled(n: &Node<'_>) -> bool {
    let Some(f) = n.child("fill") else {
        return false;
    };
    match f.arg(0) {
        Some("solid" | "yes" | "true") => true,
        Some(_) => false,
        None => f
            .value("type")
            .is_some_and(|t| t != "none" && !t.is_empty()),
    }
}

impl Board {
    fn new(root: &Node<'_>, limits: Limits) -> Board {
        let mut copper = Vec::new();
        let mut shown = HashMap::new();
        if let Some(layers) = root.child("layers") {
            for l in layers.lists() {
                // (0 "F.Cu" signal ["user name"])
                let Some(name) = l.arg(0) else { continue };
                if name.ends_with(".Cu") {
                    copper.push(name.to_owned());
                }
                if let Some(user) = l.arg(2)
                    && user != name
                {
                    shown.insert(name.to_owned(), user.to_owned());
                }
            }
        }
        if copper.is_empty() {
            copper = vec!["F.Cu".into(), "B.Cu".into()];
        }
        Board {
            limits,
            slots: Vec::new(),
            index: HashMap::new(),
            copper,
            shown,
            items: 0,
            truncated: false,
            warnings: Vec::new(),
            counts: Counts::default(),
            drills: BTreeSet::new(),
            edge: None,
            bounds: None,
            pads_only: false,
        }
    }

    /// The layers a name stands for: `*.Cu` every copper layer, `F&B.Cu` both outer ones,
    /// `*.Mask` both masks.
    fn expand(&self, name: &str) -> Vec<String> {
        if name == "*.Cu" {
            return self.copper.clone();
        }
        if let Some(kind) = name
            .strip_prefix("*.")
            .or_else(|| name.strip_prefix("F&B."))
        {
            return vec![format!("F.{kind}"), format!("B.{kind}")];
        }
        vec![name.to_owned()]
    }

    fn put(&mut self, layer: &str, color: Option<Color>, shape: Shape) {
        if self.items >= self.limits.items {
            self.truncated = true;
            return;
        }
        let b = match &shape {
            Shape::Fill { path, .. } | Shape::Stroke { path, .. } => path.bounding_box(),
            Shape::Text(t) => {
                let p = t.transform * Point::ORIGIN;
                Rect::from_points(p, p)
            }
            Shape::Point(p) => Rect::from_points(*p, *p),
        };
        self.bounds = union(self.bounds, b);
        self.items += 1;
        let slot = match self.index.get(layer) {
            Some(&i) => i,
            None => {
                self.slots.push(Slot {
                    name: layer.to_owned(),
                    items: Vec::new(),
                });
                self.index.insert(layer.to_owned(), self.slots.len() - 1);
                self.slots.len() - 1
            }
        };
        let [r, g, bl] = layer_colour(layer);
        self.slots[slot].items.push(Item {
            layer: slot as u32,
            outer: slot as u32,
            color: color.unwrap_or(Color::Rgb(r, g, bl)),
            shape,
        });
    }

    fn fill(&mut self, layer: &str, path: BezPath) {
        self.put(
            layer,
            None,
            Shape::Fill {
                path,
                even_odd: false,
            },
        );
    }

    /// `path` drawn `width` wide (a hairline when 0), and filled when `fill`.
    fn draw(&mut self, layer: &str, path: BezPath, width: f64, fill: bool) {
        if layer == "Edge.Cuts" {
            self.edge = union(self.edge, path.bounding_box());
        }
        if fill {
            if width > 0.0 {
                self.fill(layer, outline(&path, width));
            }
            self.fill(layer, path);
        } else if width > 0.0 {
            self.fill(layer, outline(&path, width));
        } else {
            self.put(
                layer,
                None,
                Shape::Stroke {
                    path,
                    weight: 0,
                    dash: None,
                },
            );
        }
    }

    fn line(&mut self, layer: &str, a: Point, b: Point, width: f64) {
        if layer == "Edge.Cuts" {
            self.edge = union(self.edge, Rect::from_points(a, b));
        }
        if width > 0.0 {
            self.fill(layer, capsule(a, b, width));
        } else {
            let mut p = BezPath::new();
            p.move_to(a);
            p.line_to(b);
            self.draw(layer, p, 0.0, false);
        }
    }

    fn read(&mut self, root: &Node<'_>) {
        for n in root.lists() {
            if self.truncated {
                break;
            }
            if self.pads_only && !matches!(n.head(), Some("footprint" | "module" | "via")) {
                continue;
            }
            match n.head() {
                Some("footprint" | "module") => self.footprint(n),
                Some("segment") => {
                    self.counts.segments += 1;
                    let (Some(a), Some(b), Some(l)) =
                        (child_xy(n, "start"), child_xy(n, "end"), n.value("layer"))
                    else {
                        continue;
                    };
                    let w = n.value_num("width").unwrap_or(0.0);
                    self.line(l, a, b, w);
                }
                Some("arc") => {
                    self.counts.arcs += 1;
                    let (Some(a), Some(m), Some(b), Some(l)) = (
                        child_xy(n, "start"),
                        child_xy(n, "mid"),
                        child_xy(n, "end"),
                        n.value("layer"),
                    ) else {
                        continue;
                    };
                    let w = n.value_num("width").unwrap_or(0.0);
                    self.draw(l, geom::arc3(a, m, b), w, false);
                }
                Some("via") => self.via(n),
                Some("zone") => self.zone(n),
                Some("dimension") => self.dimension(n, Affine::IDENTITY),
                Some(h) if h.starts_with("gr_") => self.graphic(n, Affine::IDENTITY, None),
                Some("image" | "target") => warn(
                    &mut self.warnings,
                    "Images and targets on the board are not drawn",
                ),
                _ => {}
            }
        }
    }

    fn via(&mut self, n: &Node<'_>) {
        self.counts.vias += 1;
        let (p, _) = at(n);
        let size = n.value_num("size").unwrap_or(0.0);
        if size > 0.0 {
            self.fill(VIAS, Circle::new(p, size / 2.0).to_path(TOLERANCE));
        }
        if let Some(d) = n.value_num("drill")
            && d > 0.0
        {
            self.hole(p, 0.0, d, d);
        }
    }

    /// A drill hole `w` × `h` (an oval when they differ) at `p`, turned `deg`.
    fn hole(&mut self, p: Point, deg: f64, w: f64, h: f64) {
        self.counts.holes += 1;
        self.drills.insert((w.min(h) * 1000.0).round() as u64);
        let path = place(p, deg) * geom::oval(w, h);
        self.put(
            HOLES,
            Some(Color::Background),
            Shape::Fill {
                path,
                even_odd: false,
            },
        );
    }

    fn zone(&mut self, n: &Node<'_>) {
        self.counts.zones += 1;
        let layers: Vec<String> = match n.child("layers") {
            Some(ls) => ls
                .args()
                .iter()
                .filter_map(Node::atom)
                .flat_map(|l| self.expand(l))
                .collect(),
            None => n.value("layer").map(|l| self.expand(l)).unwrap_or_default(),
        };
        let mut any = false;
        for fp in n.children("filled_polygon") {
            let Some(pts) = fp.child("pts") else { continue };
            let path = pts_path(pts, true);
            match fp.value("layer") {
                Some(l) => self.fill(l, path),
                None => {
                    for l in &layers {
                        self.fill(l, path.clone());
                    }
                }
            }
            any = true;
        }
        if !any && let Some(pts) = n.child("polygon").and_then(|p| p.child("pts")) {
            // Unfilled zones and rule areas: their outline.
            let path = pts_path(pts, true);
            for l in &layers {
                self.draw(l, path.clone(), 0.0, false);
            }
        }
    }

    fn footprint(&mut self, n: &Node<'_>) {
        if n.is("module") {
            warn(
                &mut self.warnings,
                "KiCad 5 footprints (module) in the board are drawn as KiCad 6 ones",
            );
        }
        self.counts.footprints += 1;
        let (p, a) = at(n);
        let xf = place(p, a);
        // Field values for ${REFERENCE} and ${VALUE}.
        let mut fields: Vec<(String, String)> = Vec::new();
        for c in n.lists() {
            match (c.head(), c.arg(0), c.arg(1)) {
                (Some("property"), Some(k), Some(v)) => fields.push((k.to_uppercase(), v.into())),
                (Some("fp_text"), Some(k @ ("reference" | "value")), Some(v)) => {
                    fields.push((k.to_uppercase(), v.into()));
                }
                _ => {}
            }
        }
        for c in n.lists() {
            if self.truncated {
                return;
            }
            if self.pads_only && !c.is("pad") {
                continue;
            }
            match c.head() {
                Some("pad") => self.pad(c, xf),
                Some("zone") => self.zone(c),
                Some("dimension") => self.dimension(c, xf),
                Some("property") => {
                    // Shown fields carry a layer; KiCad 8 and later.
                    if c.child("layer").is_some()
                        && let Some(v) = c.arg(1)
                    {
                        self.text(c, v, xf, &fields);
                    }
                }
                Some("fp_text") => {
                    if let Some(v) = c.arg(1) {
                        self.text(c, v, xf, &fields);
                    }
                }
                Some(h) if h.starts_with("fp_") => self.graphic(c, xf, None),
                _ => {}
            }
        }
    }

    /// Text `value` of `n` (`fp_text`, `property`, `gr_text`): its position is turned with
    /// the footprint (`xf`), its angle is absolute.
    fn text(&mut self, n: &Node<'_>, value: &str, xf: Affine, fields: &[(String, String)]) {
        self.text_on(n, value, xf, fields, None);
    }

    /// [`Board::text`] on `layer` when the text names none of its own.
    fn text_on(
        &mut self,
        n: &Node<'_>,
        value: &str,
        xf: Affine,
        fields: &[(String, String)],
        layer: Option<&str>,
    ) {
        let fx = Effects::of(n);
        if fx.hidden || n.has_flag("hide") {
            return;
        }
        let Some(layer) = n.value("layer").or(layer).map(str::to_owned) else {
            return;
        };
        let (p, deg) = at(n);
        let mut s = value.to_owned();
        for (k, v) in fields {
            s = s.replace(&format!("${{{k}}}"), v);
        }
        let s = geom::plain(&s);
        if s.trim().is_empty() {
            return;
        }
        self.counts.texts += 1;
        let tb = text_block(&s, xf * p, deg, &fx);
        self.put(&layer, None, Shape::Text(Box::new(tb)));
    }

    /// `fp_*` and `gr_*` graphics, `xf` placing them (identity on the board).
    fn graphic(&mut self, n: &Node<'_>, xf: Affine, layer: Option<&str>) {
        let Some(head) = n.head() else { return };
        let kind = head
            .strip_prefix("fp_")
            .or_else(|| head.strip_prefix("gr_"))
            .unwrap_or(head);
        let Some(layer) = layer.or_else(|| n.value("layer")).map(str::to_owned) else {
            return;
        };
        let w = width_of(n);
        let fill = filled(n);
        self.counts.graphics += 1;
        match kind {
            "line" => {
                if let (Some(a), Some(b)) = (child_xy(n, "start"), child_xy(n, "end")) {
                    self.line(&layer, xf * a, xf * b, w);
                }
            }
            "rect" => {
                if let (Some(a), Some(b)) = (child_xy(n, "start"), child_xy(n, "end")) {
                    let pts = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
                    let path = xf * geom::polygon(&pts);
                    self.draw(&layer, path, w, fill);
                }
            }
            "circle" => {
                if let (Some(c), Some(e)) = (child_xy(n, "center"), child_xy(n, "end")) {
                    let r = (e - c).hypot();
                    let path = xf * Circle::new(c, r).to_path(TOLERANCE);
                    self.draw(&layer, path, w, fill);
                }
            }
            "arc" => {
                let path = match (child_xy(n, "start"), child_xy(n, "mid"), child_xy(n, "end")) {
                    (Some(a), Some(m), Some(b)) => geom::arc3(a, m, b),
                    (Some(c), None, Some(s)) => {
                        // KiCad 5 form: centre, start, angle (clockwise on screen).
                        geom::arc_angle(c, s, -n.value_num("angle").unwrap_or(0.0))
                    }
                    _ => return,
                };
                self.draw(&layer, xf * path, w, false);
            }
            "poly" => {
                if let Some(pts) = n.child("pts") {
                    let path = xf * pts_path(pts, true);
                    self.draw(&layer, path, w, fill || n.child("fill").is_none());
                }
            }
            "curve" => {
                let pts: Vec<Point> = n
                    .child("pts")
                    .map(|p| p.children("xy").filter_map(geom::xy).collect())
                    .unwrap_or_default();
                if pts.len() == 4 {
                    let mut p = BezPath::new();
                    p.move_to(pts[0]);
                    p.curve_to(pts[1], pts[2], pts[3]);
                    self.draw(&layer, xf * p, w, false);
                }
            }
            "text" => {
                self.counts.graphics -= 1;
                if let Some(v) = n.arg(if head == "fp_text" { 1 } else { 0 }) {
                    self.text(n, v, xf, &[]);
                }
            }
            "text_box" => {
                let Some(v) = n.arg(0) else { return };
                let corners: Vec<Point> = match n.child("pts") {
                    Some(p) => p.children("xy").filter_map(geom::xy).collect(),
                    None => [child_xy(n, "start"), child_xy(n, "end")]
                        .into_iter()
                        .flatten()
                        .collect(),
                };
                let Some(&first) = corners.first() else {
                    return;
                };
                if n.yes("border").unwrap_or(true) && corners.len() == 2 {
                    let (a, b) = (corners[0], corners[1]);
                    let pts = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
                    self.draw(&layer, xf * geom::polygon(&pts), w, false);
                } else if corners.len() == 4 {
                    self.draw(&layer, xf * geom::polygon(&corners), w, false);
                }
                let mut fx = Effects::of(n);
                if fx.hidden {
                    return;
                }
                fx.halign = specular_vector::HAlign::Left;
                fx.valign = specular_vector::VAlign::Top;
                let m = fx.size.0 * 0.4;
                let s = geom::plain(v);
                self.counts.texts += 1;
                let tb = text_block(
                    &s,
                    xf * (first + specular_vector::kurbo::Vec2::new(m, -m)),
                    0.0,
                    &fx,
                );
                self.put(&layer, None, Shape::Text(Box::new(tb)));
            }
            "bbox" => {}
            _ => {
                self.counts.graphics -= 1;
            }
        }
    }

    /// A dimension: its text and the line between its points.
    fn dimension(&mut self, n: &Node<'_>, xf: Affine) {
        self.counts.dimensions += 1;
        let Some(layer) = n.value("layer").map(str::to_owned) else {
            return;
        };
        let w = n
            .child("style")
            .and_then(|s| s.value_num("thickness"))
            .unwrap_or(0.0);
        if let Some(pts) = n.child("pts") {
            let ps: Vec<Point> = pts.children("xy").filter_map(geom::xy).collect();
            if ps.len() >= 2 {
                let (a, b) = (xf * ps[0], xf * ps[1]);
                let h = n.value_num("height").unwrap_or(0.0);
                let d = b - a;
                let len = d.hypot();
                if len > 0.0 && h != 0.0 {
                    // KiCad's height runs to the left of a→b on screen (y down).
                    let off = specular_vector::kurbo::Vec2::new(d.y, -d.x) / len * h;
                    self.line(&layer, a, a + off, w);
                    self.line(&layer, b, b + off, w);
                    self.line(&layer, a + off, b + off, w);
                } else {
                    self.line(&layer, a, b, w);
                }
            }
        }
        if let Some(t) = n.child("gr_text")
            && let Some(v) = t.arg(0)
        {
            self.text_on(t, v, xf, &[], Some(&layer));
        }
    }

    fn pad(&mut self, n: &Node<'_>, fp: Affine) {
        self.counts.pads += 1;
        let kind = n.arg(1).unwrap_or("");
        let shape = n.arg(2).unwrap_or("");
        let (p, deg) = at(n);
        let pos = fp * p;
        let (w, h) = n
            .child("size")
            .map(|s| {
                let w = s.num(0).unwrap_or(0.0);
                (w, s.num(1).unwrap_or(w))
            })
            .unwrap_or((0.0, 0.0));
        let drill = n.child("drill");
        let offset = drill
            .and_then(|d| d.child("offset"))
            .map(|o| pt(o.num(0).unwrap_or(0.0), o.num(1).unwrap_or(0.0)))
            .unwrap_or(Point::ORIGIN);
        let pad_xf = place(pos, deg);
        let path = pad_shape(n, shape, w, h);
        let layers: Vec<String> = n
            .child("layers")
            .map(|ls| {
                ls.args()
                    .iter()
                    .filter_map(Node::atom)
                    .flat_map(|l| self.expand(l))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(path) = path {
            let placed = pad_xf * Affine::translate(offset.to_vec2()) * path;
            for l in &layers {
                self.fill(l, placed.clone());
            }
        }
        if let Some(d) = drill {
            let oval = d.has_flag("oval");
            let nums: Vec<f64> = d.args().iter().filter_map(Node::number).collect();
            let dw = nums.first().copied().unwrap_or(0.0);
            let dh = if oval {
                nums.get(1).copied().unwrap_or(dw)
            } else {
                dw
            };
            if dw > 0.0 && kind != "smd" && kind != "connect" {
                self.hole(pos, deg, dw, dh);
            }
        }
    }

    fn finish(mut self, root: &Node<'_>, version: u64) -> VectorScene {
        if self.truncated {
            warn(
                &mut self.warnings,
                format!(
                    "More than {} shapes: the rest of the board is left out",
                    self.limits.items
                ),
            );
        }
        let mut order: Vec<usize> = (0..self.slots.len()).collect();
        order.sort_by_key(|&i| (rank(&self.slots[i].name), self.slots[i].name.clone()));
        let mut layers = Vec::new();
        let mut opacities = Vec::new();
        let mut items = Vec::with_capacity(self.items);
        let mut info_layers = Vec::new();
        for (new, &old) in order.iter().enumerate() {
            let slot = &mut self.slots[old];
            let [r, g, b] = layer_colour(&slot.name);
            let shown = self
                .shown
                .get(&slot.name)
                .cloned()
                .unwrap_or_else(|| slot.name.clone());
            layers.push(Layer {
                name: shown.clone(),
                generated: None,
                color: Color::Rgb(r, g, b),
                linetype: None,
                lineweight: 0,
                on: true,
                frozen: false,
                plot: true,
                items: slot.items.len() as u64,
            });
            opacities.push(opacity(&slot.name));
            info_layers.push(PcbLayer {
                name: shown,
                role: layer_role(&slot.name),
                from_x2: false,
            });
            for mut it in slot.items.drain(..) {
                it.layer = new as u32;
                it.outer = new as u32;
                items.push(it);
            }
        }
        let tb = root.child("title_block");
        let tbv = |k: &str| {
            tb.and_then(|t| t.value(k))
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned)
        };
        let generator = root
            .value("generator")
            .map(|g| match root.value("generator_version") {
                Some(v) => format!("{g} {v}"),
                None => g.to_owned(),
            });
        let nets = root
            .children("net")
            .filter(|n| n.arg(1).is_some_and(|s| !s.is_empty()))
            .count() as u64;
        let c = &self.counts;
        let info = PcbInfo {
            kind: PcbKind::KicadBoard,
            generator,
            project: tbv("title"),
            revision: tbv("rev"),
            date: tbv("date"),
            size_mm: self.edge.or(self.bounds).map(|r| (r.width(), r.height())),
            thickness_mm: root.child("general").and_then(|g| g.value_num("thickness")),
            copper_layers: self.copper.len() as u32,
            layers: info_layers,
            holes: c.holes,
            drill_sizes: self.drills.len() as u32,
            components: c.footprints,
            nets,
            tracks: c.segments + c.arcs,
            vias: c.vias,
            sheets: 0,
            apertures: 0,
            macros: 0,
        };
        let mut counts: Vec<(String, u64)> = [
            ("arc", c.arcs),
            ("dimension", c.dimensions),
            ("footprint", c.footprints),
            ("graphic", c.graphics),
            ("pad", c.pads),
            ("segment", c.segments),
            ("text", c.texts),
            ("via", c.vias),
            ("zone", c.zones),
        ]
        .into_iter()
        .filter(|e| e.1 > 0)
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        counts.sort();
        let mut sheet = Sheet::new("Board", SheetKind::Model);
        sheet.items = items;
        sheet.extents = self.bounds;
        VectorScene {
            format: "KiCad".into(),
            version: version.to_string(),
            release: Some(board_release(version).into()),
            units: Units { code: 4 },
            layers,
            fonts: vec![FontSpec::default()],
            sheets: vec![sheet],
            counts,
            warnings: self.warnings,
            layer_opacity: opacities,
            pcb: Some(Box::new(info)),
            ..VectorScene::default()
        }
    }
}

/// A pad's copper outline centred on the origin, unturned (`None` for no copper).
fn pad_shape(n: &Node<'_>, shape: &str, w: f64, h: f64) -> Option<BezPath> {
    if w <= 0.0 && h <= 0.0 && shape != "custom" {
        return None;
    }
    let chamfer = n.child("chamfer");
    let rr = n.value_num("roundrect_rratio").unwrap_or(0.25);
    Some(match shape {
        "circle" => Circle::new(Point::ORIGIN, w / 2.0).to_path(TOLERANCE),
        "oval" => geom::oval(w, h),
        "roundrect" if chamfer.is_none() => geom::round_rect(w, h, rr * w.min(h)),
        "rect" | "roundrect" => match chamfer {
            Some(c) => chamfered(w, h, n.value_num("chamfer_ratio").unwrap_or(0.2), c),
            None => geom::round_rect(w, h, 0.0),
        },
        "trapezoid" => {
            let (dx, dy) = n
                .child("rect_delta")
                .map(|d| (d.num(0).unwrap_or(0.0) / 2.0, d.num(1).unwrap_or(0.0) / 2.0))
                .unwrap_or((0.0, 0.0));
            let (wx, wy) = (w / 2.0, h / 2.0);
            // KiCad's corners (y down): the delta narrows one end.
            let c = [
                pt(-wx - dy, wy + dx),
                pt(wx + dy, wy - dx),
                pt(wx - dy, -wy + dx),
                pt(-wx + dy, -wy - dx),
            ];
            geom::polygon(&c)
        }
        "custom" => custom(n, w, h),
        _ => geom::round_rect(w, h, 0.0),
    })
}

/// A rectangle with the corners named in `(chamfer top_left …)` cut by `ratio` of its short
/// side.
fn chamfered(w: f64, h: f64, ratio: f64, c: &Node<'_>) -> BezPath {
    let k = ratio.clamp(0.0, 0.5) * w.min(h);
    let has = |s: &str| c.args().iter().any(|a| a.atom() == Some(s));
    let (x, y) = (w / 2.0, h / 2.0);
    let mut pts = Vec::new();
    // Clockwise on screen from the top left (y down), then flipped.
    let corners = [
        ("top_left", (-x, -y), (0.0, k), (k, 0.0)),
        ("top_right", (x, -y), (-k, 0.0), (0.0, k)),
        ("bottom_right", (x, y), (0.0, -k), (-k, 0.0)),
        ("bottom_left", (-x, y), (k, 0.0), (0.0, -k)),
    ];
    for (name, (cx, cy), a, b) in corners {
        if has(name) && k > 0.0 {
            pts.push(pt(cx + a.0, cy + a.1));
            pts.push(pt(cx + b.0, cy + b.1));
        } else {
            pts.push(pt(cx, cy));
        }
    }
    geom::polygon(&pts)
}

/// A custom pad: its anchor and its primitives, in the pad's frame.
fn custom(n: &Node<'_>, w: f64, h: f64) -> BezPath {
    let anchor = n
        .child("options")
        .and_then(|o| o.value("anchor"))
        .unwrap_or("circle");
    let mut out = if w > 0.0 {
        if anchor == "rect" {
            geom::round_rect(w, h, 0.0)
        } else {
            Circle::new(Point::ORIGIN, w / 2.0).to_path(TOLERANCE)
        }
    } else {
        BezPath::new()
    };
    let Some(prims) = n.child("primitives") else {
        return out;
    };
    for p in prims.lists() {
        let wd = width_of(p);
        let fill = filled(p) || p.child("fill").is_none();
        let add = |out: &mut BezPath, path: BezPath| out.extend(path);
        match p.head() {
            Some("gr_poly") => {
                if let Some(pts) = p.child("pts") {
                    let path = pts_path(pts, true);
                    if wd > 0.0 {
                        add(&mut out, outline(&path, wd));
                    }
                    if fill || wd == 0.0 {
                        add(&mut out, path);
                    }
                }
            }
            Some("gr_line") => {
                if let (Some(a), Some(b)) = (child_xy(p, "start"), child_xy(p, "end")) {
                    add(&mut out, capsule(a, b, wd.max(1e-3)));
                }
            }
            Some("gr_circle") => {
                if let (Some(c), Some(e)) = (child_xy(p, "center"), child_xy(p, "end")) {
                    let r = (e - c).hypot();
                    let path = Circle::new(c, r).to_path(TOLERANCE);
                    if filled(p) || wd == 0.0 {
                        add(&mut out, Circle::new(c, r + wd / 2.0).to_path(TOLERANCE));
                    } else {
                        add(&mut out, outline(&path, wd));
                    }
                }
            }
            Some("gr_arc") => {
                if let (Some(a), Some(m), Some(b)) =
                    (child_xy(p, "start"), child_xy(p, "mid"), child_xy(p, "end"))
                {
                    add(&mut out, outline(&geom::arc3(a, m, b), wd.max(1e-3)));
                }
            }
            Some("gr_rect") => {
                if let (Some(a), Some(b)) = (child_xy(p, "start"), child_xy(p, "end")) {
                    let pts = [a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)];
                    let path = geom::polygon(&pts);
                    if wd > 0.0 {
                        add(&mut out, outline(&path, wd));
                    }
                    if filled(p) || wd == 0.0 {
                        add(&mut out, path);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_are_ordered_and_named() {
        let mut v = vec![
            "Edge.Cuts",
            "F.Cu",
            "In2.Cu",
            "B.SilkS",
            "In1.Cu",
            "B.Cu",
            "F.SilkS",
        ];
        v.sort_by_key(|n| rank(n));
        assert_eq!(
            v,
            [
                "B.SilkS",
                "B.Cu",
                "In2.Cu",
                "In1.Cu",
                "F.Cu",
                "F.SilkS",
                "Edge.Cuts"
            ]
        );
        assert_eq!(layer_role("F.Mask"), "Top solder mask");
        assert_eq!(layer_role("In3.Cu"), "Inner copper 3");
        assert_eq!(layer_colour("F.Cu"), [200, 52, 52]);
    }
}
