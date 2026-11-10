//! KiCad schematics (`.kicad_sch`, KiCad 6 and later) as a vector scene: a page per sheet of
//! the hierarchy (the root `/`, then each sheet instance by its path, `/power/`), on its
//! paper with a frame and title block, in KiCad's default schematic colours.
//!
//! Symbols are drawn from the schematic's own `lib_symbols` (the copy KiCad embeds), the
//! units and body style a placement asks for, turned and mirrored as placed. The schematic's
//! coordinates run y down and the symbol library's y up; the scene's run y up.

use std::collections::HashMap;

use specular_vector::kurbo::{Affine, BezPath, Circle, Point, Rect, Shape as _, Vec2};
use specular_vector::{
    Color, FontSpec, HAlign, Item, Layer, PcbInfo, PcbKind, Shape, Sheet, SheetKind, Units, VAlign,
    VectorScene,
};

use crate::geom::{self, Effects, TOLERANCE, capsule, outline, pt, text_block};
use crate::sexpr::{self, Node};
use crate::{Error, LEGACY_SCHEMATIC, Limits, legacy_kind, schematic_release, warn};

/// The scene's layers, in drawing order.
pub const LAYERS: [&str; 8] = [
    "Frame", "Graphics", "Sheets", "Symbols", "Wires", "Buses", "Labels", "Text",
];
const FRAME: u32 = 0;
const GRAPHICS: u32 = 1;
const SHEETS: u32 = 2;
const SYMBOLS: u32 = 3;
const WIRES: u32 = 4;
const BUSES: u32 = 5;
const LABELS: u32 = 6;
const TEXT: u32 = 7;

/// KiCad's default schematic colours.
pub mod colour {
    pub const WIRE: [u8; 3] = [0, 150, 0];
    pub const BUS: [u8; 3] = [0, 0, 132];
    pub const NO_CONNECT: [u8; 3] = [0, 0, 132];
    pub const LABEL: [u8; 3] = [15, 15, 15];
    pub const GLOBAL: [u8; 3] = [132, 0, 0];
    pub const HIER: [u8; 3] = [114, 86, 0];
    pub const NOTE: [u8; 3] = [0, 0, 194];
    pub const BODY: [u8; 3] = [132, 0, 0];
    pub const BODY_FILL: [u8; 3] = [255, 255, 194];
    pub const PIN: [u8; 3] = [132, 0, 0];
    pub const PIN_NAME: [u8; 3] = [0, 100, 100];
    pub const PIN_NUMBER: [u8; 3] = [169, 0, 0];
    pub const REFERENCE: [u8; 3] = [0, 100, 100];
    pub const FIELD: [u8; 3] = [132, 0, 132];
    pub const SHEET: [u8; 3] = [132, 0, 0];
    pub const SHEET_NAME: [u8; 3] = [0, 100, 100];
    pub const SHEET_FILE: [u8; 3] = [114, 86, 0];
    pub const FRAME: [u8; 3] = [132, 0, 0];
}

/// Default line widths, millimetres.
const WIRE_W: f64 = 0.1524;
const BUS_W: f64 = 0.3048;
const LINE_W: f64 = 0.1524;
const JUNCTION_D: f64 = 0.9144;
const NO_CONNECT: f64 = 0.6096;
const PIN_GAP: f64 = 0.3048;

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

/// Reads a schematic and the sheets under it, `resolve` giving a sheet file's bytes by the
/// name the schematic uses for it.
pub fn scene(
    bytes: &[u8],
    resolve: &dyn Fn(&str) -> Option<Vec<u8>>,
    limits: &Limits,
) -> Result<VectorScene, Error> {
    if let Some(kind) = legacy_kind(bytes) {
        return Err(if kind.contains("schematic") || kind.contains("library") {
            Error::Unsupported(LEGACY_SCHEMATIC.into())
        } else {
            Error::NotThisFormat
        });
    }
    let text = crate::text_of(bytes);
    let root = sexpr::parse_with_depth(&text, limits.depth)?;
    if !root.is("kicad_sch") {
        return Err(Error::NotThisFormat);
    }
    let version = root.value_num("version").unwrap_or(0.0) as u64;
    let mut w = Walker {
        limits: *limits,
        resolve,
        pages: Vec::new(),
        items: 0,
        truncated: false,
        warnings: Vec::new(),
        counts: Counts::default(),
        instances6: HashMap::new(),
        pages6: HashMap::new(),
        layer_items: [0; LAYERS.len()],
    };
    // KiCad 6 keeps every placement's reference in the root.
    if let Some(si) = root.child("symbol_instances") {
        for p in si.children("path") {
            if let Some(path) = p.arg(0) {
                w.instances6.insert(
                    path.to_owned(),
                    (
                        p.value("reference").unwrap_or("").to_owned(),
                        p.value_num("unit").unwrap_or(1.0) as u32,
                    ),
                );
            }
        }
    }
    if let Some(si) = root.child("sheet_instances") {
        for p in si.children("path") {
            if let (Some(path), Some(page)) = (p.arg(0), p.value("page")) {
                w.pages6.insert(path.to_owned(), page.to_owned());
            }
        }
    }
    let root_uuid = root.value("uuid").unwrap_or("").to_owned();
    let title = root
        .child("title_block")
        .and_then(|t| t.value("title"))
        .map(str::to_owned);
    let mut stack = Vec::new();
    w.page(
        &root,
        Ctx {
            name: "/".into(),
            file: String::new(),
            path7: format!("/{root_uuid}"),
            path6: String::new(),
            page: root
                .child("sheet_instances")
                .and_then(|s| s.child("path"))
                .and_then(|p| p.value("page"))
                .map(str::to_owned),
        },
        &mut stack,
    );
    Ok(w.finish(&root, version, title))
}

/// Where a sheet sits in the hierarchy.
#[derive(Clone)]
struct Ctx {
    /// The page's name: its sheet path ("/", "/power/").
    name: String,
    /// Its file, as the parent names it (empty for the root).
    file: String,
    /// Instance paths: KiCad 7 on (`/root/sheet…`) and KiCad 6 (`/sheet…`).
    path7: String,
    path6: String,
    /// The page number the file gives it.
    page: Option<String>,
}

#[derive(Default)]
struct Counts {
    symbols: u64,
    power: u64,
    wires: u64,
    buses: u64,
    junctions: u64,
    labels: u64,
    /// Directive labels and net class flags.
    directives: u64,
    sheets: u64,
    texts: u64,
}

struct Walker<'r> {
    limits: Limits,
    resolve: &'r dyn Fn(&str) -> Option<Vec<u8>>,
    pages: Vec<Sheet>,
    items: usize,
    truncated: bool,
    warnings: Vec<String>,
    counts: Counts,
    instances6: HashMap<String, (String, u32)>,
    /// KiCad 6 page numbers by sheet path, from the root.
    pages6: HashMap<String, String>,
    layer_items: [u64; LAYERS.len()],
}

/// One page being drawn.
struct Page {
    items: Vec<Item>,
}

/// A stroke's width and colour: `(stroke (width w) (type t) (color r g b a))`, the default
/// when `w` is 0 or the colour's alpha is 0.
fn stroke_of(n: &Node<'_>, width: f64, colour: [u8; 3]) -> (f64, [u8; 3]) {
    let s = n.child("stroke");
    let w = s
        .and_then(|s| s.value_num("width"))
        .or_else(|| n.value_num("width"))
        .filter(|w| *w > 0.0)
        .unwrap_or(width);
    (w, own_colour(s.unwrap_or(n)).unwrap_or(colour))
}

/// `(color r g b a)` among `n`'s children, when its alpha is not 0.
fn own_colour(n: &Node<'_>) -> Option<[u8; 3]> {
    let c = n.child("color")?;
    let a = c.num(3).unwrap_or(1.0);
    if a <= 0.0 {
        return None;
    }
    let ch = |i| c.num(i).unwrap_or(0.0).clamp(0.0, 255.0) as u8;
    Some([ch(0), ch(1), ch(2)])
}

/// A symbol-library point (y up), as stored.
fn lib_xy(n: &Node<'_>) -> Option<Point> {
    Some(Point::new(n.num(0)?, n.num(1)?))
}

fn lib_child(n: &Node<'_>, name: &str) -> Option<Point> {
    lib_xy(n.child(name)?)
}

/// The paper's size in millimetres, landscape unless `portrait`.
pub fn paper_size(n: Option<&Node<'_>>) -> (f64, f64) {
    let Some(n) = n else { return (297.0, 210.0) };
    let name = n.arg(0).unwrap_or("A4");
    let (w, h) = match name {
        "A5" => (210.0, 148.0),
        "A4" => (297.0, 210.0),
        "A3" => (420.0, 297.0),
        "A2" => (594.0, 420.0),
        "A1" => (841.0, 594.0),
        "A0" => (1189.0, 841.0),
        "A" | "USLetter" => (279.4, 215.9),
        "B" | "USLedger" => (431.8, 279.4),
        "C" => (558.8, 431.8),
        "D" => (863.6, 558.8),
        "E" => (1117.6, 863.6),
        "USLegal" => (355.6, 215.9),
        "User" => (
            n.num(1).unwrap_or(297.0).max(10.0),
            n.num(2).unwrap_or(210.0).max(10.0),
        ),
        _ => (297.0, 210.0),
    };
    if n.has_flag("portrait") {
        (h, w)
    } else {
        (w, h)
    }
}

impl Walker<'_> {
    fn put(&mut self, page: &mut Page, layer: u32, colour: [u8; 3], shape: Shape) {
        if self.items >= self.limits.items {
            self.truncated = true;
            return;
        }
        self.items += 1;
        self.layer_items[layer as usize] += 1;
        page.items.push(Item {
            layer,
            outer: layer,
            color: rgb(colour),
            shape,
        });
    }

    fn fill(&mut self, page: &mut Page, layer: u32, colour: [u8; 3], path: BezPath) {
        self.put(
            page,
            layer,
            colour,
            Shape::Fill {
                path,
                even_odd: false,
            },
        );
    }

    fn line(&mut self, page: &mut Page, layer: u32, c: [u8; 3], a: Point, b: Point, w: f64) {
        self.fill(page, layer, c, capsule(a, b, w));
    }

    fn path(&mut self, page: &mut Page, layer: u32, c: [u8; 3], p: &BezPath, w: f64) {
        self.fill(page, layer, c, outline(p, w));
    }

    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        page: &mut Page,
        layer: u32,
        c: [u8; 3],
        s: &str,
        p: Point,
        deg: f64,
        fx: &Effects,
    ) {
        let s = geom::plain(s);
        if s.trim().is_empty() {
            return;
        }
        let tb = text_block(&s, p, deg, fx);
        self.put(page, layer, c, Shape::Text(Box::new(tb)));
    }

    fn page(&mut self, n: &Node<'_>, ctx: Ctx, stack: &mut Vec<String>) {
        if self.pages.len() >= self.limits.sheets {
            warn(
                &mut self.warnings,
                format!(
                    "More than {} sheets: the rest are left out",
                    self.limits.sheets
                ),
            );
            return;
        }
        let mut page = Page { items: Vec::new() };
        let (pw, ph) = paper_size(n.child("paper"));
        self.frame(&mut page, n, &ctx, pw, ph);
        let libs: HashMap<&str, &Node<'_>> = n
            .child("lib_symbols")
            .map(|l| {
                l.children("symbol")
                    .filter_map(|s| Some((s.arg(0)?, s)))
                    .collect()
            })
            .unwrap_or_default();
        let mut subs: Vec<(&Node<'_>, String, String)> = Vec::new();
        for c in n.lists() {
            if self.truncated {
                break;
            }
            match c.head() {
                Some("wire") => {
                    self.counts.wires += 1;
                    let (w, col) = stroke_of(c, WIRE_W, colour::WIRE);
                    self.poly(&mut page, c, WIRES, col, w);
                }
                Some("bus") => {
                    self.counts.buses += 1;
                    let (w, col) = stroke_of(c, BUS_W, colour::BUS);
                    self.poly(&mut page, c, BUSES, col, w);
                }
                Some("bus_entry") => {
                    let (p, _) = geom::at(c);
                    let s = c
                        .child("size")
                        .map(|s| Vec2::new(s.num(0).unwrap_or(0.0), -s.num(1).unwrap_or(0.0)))
                        .unwrap_or_default();
                    let (w, col) = stroke_of(c, WIRE_W, colour::WIRE);
                    self.line(&mut page, WIRES, col, p, p + s, w);
                }
                Some("junction") => {
                    self.counts.junctions += 1;
                    let (p, _) = geom::at(c);
                    let d = c
                        .value_num("diameter")
                        .filter(|d| *d > 0.0)
                        .unwrap_or(JUNCTION_D);
                    let col = own_colour(c).unwrap_or(colour::WIRE);
                    self.fill(
                        &mut page,
                        WIRES,
                        col,
                        Circle::new(p, d / 2.0).to_path(TOLERANCE),
                    );
                }
                Some("no_connect") => {
                    let (p, _) = geom::at(c);
                    let k = NO_CONNECT;
                    let col = colour::NO_CONNECT;
                    self.line(
                        &mut page,
                        WIRES,
                        col,
                        p + Vec2::new(-k, -k),
                        p + Vec2::new(k, k),
                        LINE_W,
                    );
                    self.line(
                        &mut page,
                        WIRES,
                        col,
                        p + Vec2::new(-k, k),
                        p + Vec2::new(k, -k),
                        LINE_W,
                    );
                }
                Some(
                    k @ ("label" | "global_label" | "hierarchical_label" | "directive_label"
                    | "netclass_flag"),
                ) => {
                    if k == "directive_label" || k == "netclass_flag" {
                        self.counts.directives += 1;
                    } else {
                        self.counts.labels += 1;
                    }
                    self.label(&mut page, c, k);
                }
                Some("text") => {
                    self.counts.texts += 1;
                    if let Some(s) = c.arg(0) {
                        let fx = Effects::of(c);
                        if !fx.hidden {
                            let (p, a) = geom::at(c);
                            let col = own_colour_fx(c).unwrap_or(colour::NOTE);
                            self.text(&mut page, TEXT, col, s, p, upright(a), &fx);
                        }
                    }
                }
                Some("text_box") => self.text_box(&mut page, c, None),
                Some("polyline") => {
                    let (w, col) = stroke_of(c, LINE_W, colour::NOTE);
                    self.poly(&mut page, c, GRAPHICS, col, w);
                }
                Some("rectangle" | "circle" | "arc" | "bezier") => {
                    self.shape(
                        &mut page,
                        c,
                        GRAPHICS,
                        Affine::scale_non_uniform(1.0, -1.0),
                        colour::NOTE,
                        false,
                    );
                }
                Some("image") => warn(&mut self.warnings, "Images in schematics are not drawn"),
                Some("symbol") => self.symbol(&mut page, c, &libs, &ctx),
                Some("sheet") => {
                    self.counts.sheets += 1;
                    let (name, file) = self.sheet(&mut page, c);
                    subs.push((c, name, file));
                }
                _ => {}
            }
        }
        let mut sheet = Sheet::new(ctx.name.clone(), SheetKind::Layout);
        sheet.paper = Some(Rect::new(0.0, -ph, pw, 0.0));
        sheet.items = page.items;
        sheet.extents = bounds(&sheet.items).map(|b| b.union(Rect::new(0.0, -ph, pw, 0.0)));
        self.pages.push(sheet);

        // The sheets under this one.
        for (c, name, file) in subs {
            if file.is_empty() {
                continue;
            }
            if stack.contains(&file) || stack.len() > 64 {
                warn(
                    &mut self.warnings,
                    format!("Sheet {file} contains itself: it is shown once"),
                );
                continue;
            }
            let Some(bytes) = (self.resolve)(&file) else {
                warn(
                    &mut self.warnings,
                    format!("Sheet file {file} was not found"),
                );
                continue;
            };
            let text = crate::text_of(&bytes);
            let Ok(sub) = sexpr::parse_with_depth(&text, self.limits.depth) else {
                warn(
                    &mut self.warnings,
                    format!("Sheet file {file} could not be read"),
                );
                continue;
            };
            if !sub.is("kicad_sch") {
                warn(
                    &mut self.warnings,
                    format!("Sheet file {file} is not a KiCad schematic"),
                );
                continue;
            }
            let uuid = c.value("uuid").or(c.value("tstamp")).unwrap_or("");
            let path7 = format!("{}/{uuid}", ctx.path7);
            let page_no = c
                .child("instances")
                .into_iter()
                .flat_map(|i| i.children("project"))
                .flat_map(|p| p.children("path"))
                .find(|p| p.arg(0) == Some(ctx.path7.as_str()))
                .and_then(|p| p.value("page"))
                .map(str::to_owned)
                .or_else(|| self.pages6.get(&format!("{}/{uuid}", ctx.path6)).cloned());
            let child = Ctx {
                name: format!("{}{name}/", ctx.name),
                file: file.clone(),
                path7,
                path6: format!("{}/{uuid}", ctx.path6),
                page: page_no,
            };
            stack.push(file);
            self.page(&sub, child, stack);
            stack.pop();
        }
    }

    /// A wire, bus or polyline through its `(pts …)`.
    fn poly(&mut self, page: &mut Page, n: &Node<'_>, layer: u32, c: [u8; 3], w: f64) {
        let pts: Vec<Point> = n
            .child("pts")
            .map(|p| p.children("xy").filter_map(geom::xy).collect())
            .unwrap_or_default();
        if pts.len() == 2 {
            self.line(page, layer, c, pts[0], pts[1], w);
        } else if pts.len() > 2 {
            let mut p = BezPath::new();
            p.move_to(pts[0]);
            for q in &pts[1..] {
                p.line_to(*q);
            }
            let fill = fill_of(n);
            if let Some(f) = fill.colour(c) {
                let mut closed = p.clone();
                closed.close_path();
                self.fill(page, layer, f, closed);
            }
            self.path(page, layer, c, &p, w);
        }
    }

    /// A rectangle, circle, arc or bezier: in the symbol library (`xf` placing it) or on the
    /// sheet (`xf` flipping y).
    fn shape(
        &mut self,
        page: &mut Page,
        n: &Node<'_>,
        layer: u32,
        xf: Affine,
        c: [u8; 3],
        body: bool,
    ) {
        let (w, col) = stroke_of(n, LINE_W, c);
        let path = match n.head() {
            Some("rectangle") => {
                let (Some(a), Some(b)) = (lib_child(n, "start"), lib_child(n, "end")) else {
                    return;
                };
                let mut p = BezPath::new();
                p.move_to(a);
                p.line_to(Point::new(b.x, a.y));
                p.line_to(b);
                p.line_to(Point::new(a.x, b.y));
                p.close_path();
                p
            }
            Some("circle") => {
                let Some(ctr) = lib_child(n, "center") else {
                    return;
                };
                let r = n.value_num("radius").unwrap_or(0.0);
                Circle::new(ctr, r).to_path(TOLERANCE)
            }
            Some("arc") => {
                match (
                    lib_child(n, "start"),
                    lib_child(n, "mid"),
                    lib_child(n, "end"),
                ) {
                    (Some(a), Some(m), Some(b)) => geom::arc3(a, m, b),
                    _ => return,
                }
            }
            Some("polyline") => {
                let pts: Vec<Point> = n
                    .child("pts")
                    .map(|p| p.children("xy").filter_map(lib_xy).collect())
                    .unwrap_or_default();
                if pts.len() < 2 {
                    return;
                }
                let mut p = BezPath::new();
                p.move_to(pts[0]);
                for q in &pts[1..] {
                    p.line_to(*q);
                }
                p
            }
            Some("bezier") => {
                let pts: Vec<Point> = n
                    .child("pts")
                    .map(|p| p.children("xy").filter_map(lib_xy).collect())
                    .unwrap_or_default();
                if pts.len() != 4 {
                    return;
                }
                let mut p = BezPath::new();
                p.move_to(pts[0]);
                p.curve_to(pts[1], pts[2], pts[3]);
                p
            }
            _ => return,
        };
        let path = xf * path;
        let fill = fill_of(n);
        let fill_c = if body {
            fill.colour(col)
        } else {
            fill.colour(col).filter(|_| fill != Fill::Background)
        };
        if let Some(f) = fill_c {
            let mut closed = path.clone();
            closed.close_path();
            self.fill(page, layer, f, closed);
        }
        self.path(page, layer, col, &path, w);
    }

    fn label(&mut self, page: &mut Page, n: &Node<'_>, kind: &str) {
        let Some(s) = n.arg(0) else { return };
        let s = geom::plain(s);
        let fx = Effects::of(n);
        let (p, a) = geom::at(n);
        let h = fx.size.0;
        let dir = match (a.round() as i64).rem_euclid(360) {
            90 => Vec2::new(0.0, 1.0),
            180 => Vec2::new(-1.0, 0.0),
            270 => Vec2::new(0.0, -1.0),
            _ => Vec2::new(1.0, 0.0),
        };
        let (text_deg, toward) = if dir.x != 0.0 {
            (0.0, dir.x)
        } else {
            (90.0, dir.y)
        };
        let mut tfx = fx;
        tfx.halign = if toward > 0.0 {
            HAlign::Left
        } else {
            HAlign::Right
        };
        let up = if dir.x != 0.0 {
            Vec2::new(0.0, 1.0)
        } else {
            Vec2::new(-1.0, 0.0)
        };
        let shape = n.value("shape").unwrap_or("passive");
        let est = s.chars().count() as f64 * h * 0.75;
        match kind {
            "label" => {
                tfx.valign = VAlign::Bottom;
                let col = own_colour_fx(n).unwrap_or(colour::LABEL);
                self.text(page, LABELS, col, &s, p + up * 0.2, text_deg, &tfx);
            }
            "global_label" => {
                let hh = h * 0.75;
                let len = est + 2.0 * hh;
                let pts = flag(shape, len, hh, true);
                let frame = Affine::new([dir.x, dir.y, -dir.y, dir.x, p.x, p.y]);
                let poly: Vec<Point> = pts.iter().map(|q| frame * *q).collect();
                let col = own_colour_fx(n).unwrap_or(colour::GLOBAL);
                let path = geom::polygon(&poly);
                self.path(page, LABELS, col, &path, LINE_W);
                tfx.valign = VAlign::Middle;
                self.text(page, LABELS, col, &s, p + dir * hh * 1.1, text_deg, &tfx);
            }
            "hierarchical_label" => {
                let hh = h / 2.0;
                let pts = hier_shape(shape, hh);
                let frame = Affine::new([dir.x, dir.y, -dir.y, dir.x, p.x, p.y]);
                let poly: Vec<Point> = pts.iter().map(|q| frame * *q).collect();
                let col = own_colour_fx(n).unwrap_or(colour::HIER);
                self.path(page, LABELS, col, &geom::polygon(&poly), LINE_W);
                tfx.valign = VAlign::Middle;
                self.text(page, LABELS, col, &s, p + dir * h * 1.3, text_deg, &tfx);
            }
            _ => {
                // Directive labels and net class flags: a stem and a dot, then their fields.
                let len = n.value_num("length").unwrap_or(2.54);
                let col = own_colour_fx(n).unwrap_or(colour::LABEL);
                let tip = p + dir * len;
                self.line(page, LABELS, col, p, tip, LINE_W);
                self.fill(
                    page,
                    LABELS,
                    col,
                    Circle::new(tip, h * 0.3).to_path(TOLERANCE),
                );
                if !s.is_empty() {
                    tfx.valign = VAlign::Middle;
                    self.text(page, LABELS, col, &s, tip + dir * h * 0.6, text_deg, &tfx);
                }
            }
        }
        // Shown fields of the label (net class, intersheet references are left out).
        for f in n.children("property") {
            let (Some(k), Some(v)) = (f.arg(0), f.arg(1)) else {
                continue;
            };
            if k == "Intersheetrefs" || k == "Intersheet References" {
                continue;
            }
            let ffx = Effects::of(f);
            if ffx.hidden {
                continue;
            }
            let (fp, fa) = geom::at(f);
            self.text(page, LABELS, colour::FIELD, v, fp, upright(fa), &ffx);
        }
    }

    fn text_box(&mut self, page: &mut Page, n: &Node<'_>, xf: Option<Affine>) {
        let Some(s) = n.arg(0) else { return };
        let (w, col) = stroke_of(n, LINE_W, colour::NOTE);
        let (a, b) = if let Some(size) = n.child("size") {
            let (p, _) = geom::at(n);
            let d = Vec2::new(size.num(0).unwrap_or(0.0), -size.num(1).unwrap_or(0.0));
            (p, p + d)
        } else {
            match (geom::child_xy(n, "start"), geom::child_xy(n, "end")) {
                (Some(a), Some(b)) => (a, b),
                _ => return,
            }
        };
        let (a, b) = match xf {
            // Library text boxes are y up: flip back into the symbol's frame.
            Some(m) => (m * Point::new(a.x, -a.y), m * Point::new(b.x, -b.y)),
            None => (a, b),
        };
        let r = Rect::from_points(a, b);
        let fill = fill_of(n);
        if let Some(f) = fill.colour(col) {
            self.fill(page, GRAPHICS, f, r.to_path(TOLERANCE));
        }
        let border = n
            .child("stroke")
            .and_then(|s| s.value("type"))
            .is_none_or(|t| t != "none")
            && n.child("stroke").and_then(|s| s.value_num("width")) != Some(-1.0);
        if border {
            self.path(page, GRAPHICS, col, &r.to_path(TOLERANCE), w);
        }
        let mut fx = Effects::of(n);
        if fx.hidden {
            return;
        }
        let m = fx.size.0 * 0.4;
        let (p, valign) = (Point::new(r.x0 + m, r.y1 - m), VAlign::Top);
        fx.valign = valign;
        if fx.halign == HAlign::Center {
            fx.halign = HAlign::Left;
        }
        let tc = own_colour_fx(n).unwrap_or(colour::NOTE);
        self.text(page, TEXT, tc, s, p, 0.0, &fx);
    }

    /// A sheet symbol: box, name, file and pins. Returns its name and file.
    fn sheet(&mut self, page: &mut Page, n: &Node<'_>) -> (String, String) {
        let (p, _) = geom::at(n);
        let (w, h) = n
            .child("size")
            .map(|s| (s.num(0).unwrap_or(0.0), s.num(1).unwrap_or(0.0)))
            .unwrap_or((0.0, 0.0));
        let r = Rect::new(p.x, p.y - h, p.x + w, p.y);
        if let Some(f) = n.child("fill").and_then(own_colour) {
            self.fill(page, SHEETS, f, r.to_path(TOLERANCE));
        }
        let (sw, col) = stroke_of(n, LINE_W, colour::SHEET);
        self.path(page, SHEETS, col, &r.to_path(TOLERANCE), sw);
        let mut name = String::new();
        let mut file = String::new();
        for f in n.children("property") {
            let (Some(k), Some(v)) = (f.arg(0), f.arg(1)) else {
                continue;
            };
            let (fc, is_name, is_file) = match k {
                "Sheetname" | "Sheet name" => (colour::SHEET_NAME, true, false),
                "Sheetfile" | "Sheet file" => (colour::SHEET_FILE, false, true),
                _ => (colour::FIELD, false, false),
            };
            if is_name {
                name = v.to_owned();
            }
            if is_file {
                file = v.to_owned();
            }
            let fx = Effects::of(f);
            if fx.hidden {
                continue;
            }
            let (fp, fa) = geom::at(f);
            self.text(page, SHEETS, fc, v, fp, upright(fa), &fx);
        }
        for pin in n.children("pin") {
            let Some(s) = pin.arg(0) else { continue };
            let fx = Effects::of(pin);
            let (pp, a) = geom::at(pin);
            let hh = fx.size.0 / 2.0;
            // The shape points out of the sheet; the name reads inwards.
            let out = match (a.round() as i64).rem_euclid(360) {
                90 => Vec2::new(0.0, 1.0),
                180 => Vec2::new(-1.0, 0.0),
                270 => Vec2::new(0.0, -1.0),
                _ => Vec2::new(1.0, 0.0),
            };
            let shape = pin.arg(1).unwrap_or("passive");
            let pts = hier_shape(shape, hh);
            let frame = Affine::new([-out.x, -out.y, out.y, -out.x, pp.x, pp.y]);
            let poly: Vec<Point> = pts.iter().map(|q| frame * *q + out * hh * 2.0).collect();
            self.path(page, SHEETS, colour::HIER, &geom::polygon(&poly), LINE_W);
            let mut tfx = fx;
            tfx.valign = VAlign::Middle;
            let (deg, toward) = if out.x != 0.0 {
                (0.0, -out.x)
            } else {
                (90.0, -out.y)
            };
            tfx.halign = if toward > 0.0 {
                HAlign::Left
            } else {
                HAlign::Right
            };
            self.text(page, SHEETS, colour::HIER, s, pp - out * hh, deg, &tfx);
        }
        (name, file)
    }

    fn symbol(
        &mut self,
        page: &mut Page,
        n: &Node<'_>,
        libs: &HashMap<&str, &Node<'_>>,
        ctx: &Ctx,
    ) {
        let lib_id = n.value("lib_name").or(n.value("lib_id")).unwrap_or("");
        let lib = libs.get(lib_id).copied();
        let lib = lib.map(|l| resolve_extends(l, libs));
        let power = lib.is_some_and(|l| l.child("power").is_some() || l.has_flag("power"));
        if power {
            self.counts.power += 1;
        } else {
            self.counts.symbols += 1;
        }
        let (p, a) = geom::at(n);
        let mirror = n.value("mirror");
        let rot = Affine::rotate(a.to_radians());
        let mir = match mirror {
            Some("x") => Affine::scale_non_uniform(1.0, -1.0),
            Some("y") => Affine::scale_non_uniform(-1.0, 1.0),
            _ => Affine::IDENTITY,
        };
        let xf = Affine::translate(p.to_vec2()) * mir * rot;
        let uuid = n.value("uuid").or(n.value("tstamp")).unwrap_or("");
        // Reference and unit of this instance.
        let mut reference = n
            .children("property")
            .find(|f| f.arg(0) == Some("Reference"))
            .and_then(|f| f.arg(1))
            .unwrap_or("")
            .to_owned();
        let mut unit = n.value_num("unit").unwrap_or(1.0) as u32;
        let inst7 = n
            .child("instances")
            .into_iter()
            .flat_map(|i| i.children("project"))
            .flat_map(|p| p.children("path"))
            .find(|p| p.arg(0) == Some(ctx.path7.as_str()));
        if let Some(i) = inst7 {
            if let Some(r) = i.value("reference") {
                reference = r.to_owned();
            }
            if let Some(u) = i.value_num("unit") {
                unit = u as u32;
            }
        } else if let Some((r, u)) = self.instances6.get(&format!("{}/{uuid}", ctx.path6)) {
            reference = r.clone();
            unit = *u;
        }
        let style = n
            .value_num("body_style")
            .or(n.value_num("convert"))
            .unwrap_or(1.0) as u32;
        let Some(lib) = lib else {
            warn(
                &mut self.warnings,
                "Some symbols are missing from the schematic's symbol library and are drawn as boxes",
            );
            let r = Rect::from_center_size(p, (2.54, 2.54));
            self.path(page, SYMBOLS, colour::BODY, &r.to_path(TOLERANCE), LINE_W);
            self.fields(page, n, &reference, unit, 1, a);
            return;
        };
        let units = unit_count(lib);
        let pin_names = lib.child("pin_names");
        let names_offset = pin_names
            .and_then(|p| p.value_num("offset"))
            .unwrap_or(0.508);
        let names_hidden = pin_names.is_some_and(|p| p.yes("hide").unwrap_or(false));
        let numbers_hidden = lib
            .child("pin_numbers")
            .is_some_and(|p| p.yes("hide").unwrap_or(false));
        let mut bodies: Vec<&Node<'_>> = Vec::new();
        let mut pins: Vec<&Node<'_>> = Vec::new();
        for sub in lib.children("symbol") {
            let (u, s) = sub.arg(0).and_then(unit_style).unwrap_or((0, 0));
            if (u != 0 && u != unit) || (s != 0 && s != style) {
                continue;
            }
            for g in sub.lists() {
                match g.head() {
                    Some("pin") => pins.push(g),
                    Some(_) => bodies.push(g),
                    None => {}
                }
            }
        }
        // Filled bodies first, then outlines, then pins, as KiCad layers them.
        for g in &bodies {
            match g.head() {
                Some("rectangle" | "circle" | "arc" | "polyline" | "bezier") => {
                    self.shape(page, g, SYMBOLS, xf, colour::BODY, true);
                }
                Some("text") => {
                    let Some(s) = g.arg(0) else { continue };
                    let fx = Effects::of(g);
                    if fx.hidden {
                        continue;
                    }
                    let (tp, mut ta) = g
                        .child("at")
                        .map(|t| {
                            (
                                Point::new(t.num(0).unwrap_or(0.0), t.num(1).unwrap_or(0.0)),
                                t.num(2).unwrap_or(0.0),
                            )
                        })
                        .unwrap_or((Point::ORIGIN, 0.0));
                    if ta.abs() > 360.0 {
                        // Older libraries store tenths of a degree.
                        ta /= 10.0;
                    }
                    let vertical = is_vertical(ta) != turned(a);
                    self.text(
                        page,
                        SYMBOLS,
                        colour::BODY,
                        s,
                        xf * tp,
                        if vertical { 90.0 } else { 0.0 },
                        &fx,
                    );
                }
                Some("text_box") => self.text_box(page, g, Some(xf)),
                _ => {}
            }
        }
        for pin in pins {
            self.pin(page, pin, xf, names_offset, names_hidden, numbers_hidden);
        }
        self.fields(page, n, &reference, unit, units, a);
    }

    /// The placed symbol's shown fields.
    fn fields(
        &mut self,
        page: &mut Page,
        n: &Node<'_>,
        reference: &str,
        unit: u32,
        units: u32,
        a: f64,
    ) {
        for f in n.children("property") {
            let (Some(k), Some(v)) = (f.arg(0), f.arg(1)) else {
                continue;
            };
            let fx = Effects::of(f);
            if fx.hidden {
                continue;
            }
            let (fp, fa) = geom::at(f);
            let (text, col) = match k {
                "Reference" => {
                    let mut r = reference.to_owned();
                    if units > 1 && (1..=26).contains(&unit) {
                        r.push((b'A' + (unit - 1) as u8) as char);
                    }
                    (r, colour::REFERENCE)
                }
                "Value" => (v.to_owned(), colour::REFERENCE),
                _ => (v.to_owned(), colour::FIELD),
            };
            // Field angles are the symbol's; text stays readable (0° or 90°).
            let vertical = is_vertical(fa) != turned(a);
            self.text(
                page,
                SYMBOLS,
                col,
                &text,
                fp,
                if vertical { 90.0 } else { 0.0 },
                &fx,
            );
        }
    }

    fn pin(
        &mut self,
        page: &mut Page,
        pin: &Node<'_>,
        xf: Affine,
        names_offset: f64,
        names_hidden: bool,
        numbers_hidden: bool,
    ) {
        if pin.yes("hide").unwrap_or(false) {
            return;
        }
        let Some(at) = pin.child("at") else { return };
        let p = Point::new(at.num(0).unwrap_or(0.0), at.num(1).unwrap_or(0.0));
        let ang = at.num(2).unwrap_or(0.0).to_radians();
        let len = pin.value_num("length").unwrap_or(2.54);
        let d = Vec2::new(ang.cos(), ang.sin());
        let style = pin.arg(1).unwrap_or("line");
        let (s0, s1) = (xf * p, xf * (p + d * len));
        let dir = (s1 - s0) / (s1 - s0).hypot().max(1e-9);
        let inverted = style.contains("inverted");
        let bubble = 0.381;
        let line_end = if inverted && len > 2.0 * bubble {
            s1 - dir * 2.0 * bubble
        } else {
            s1
        };
        if len > 0.0 {
            self.line(page, SYMBOLS, colour::PIN, s0, line_end, LINE_W);
        }
        if inverted {
            let c = s1 - dir * bubble;
            self.path(
                page,
                SYMBOLS,
                colour::PIN,
                &Circle::new(c, bubble).to_path(TOLERANCE),
                LINE_W,
            );
        }
        if style.contains("clock") {
            // A small triangle inside the body.
            let k = 0.635;
            let side = Vec2::new(-dir.y, dir.x) * k;
            let tri = [s1 + side, s1 + dir * k, s1 - side];
            let mut t = BezPath::new();
            t.move_to(tri[0]);
            t.line_to(tri[1]);
            t.line_to(tri[2]);
            self.path(page, SYMBOLS, colour::PIN, &t, LINE_W);
        }
        let horizontal = dir.x.abs() >= dir.y.abs();
        let deg = if horizontal { 0.0 } else { 90.0 };
        let toward = if horizontal { dir.x } else { dir.y };
        let up = if horizontal {
            Vec2::new(0.0, 1.0)
        } else {
            Vec2::new(-1.0, 0.0)
        };
        let mid = s0.midpoint(s1);
        let name = pin
            .child("name")
            .and_then(|n| Some((n.arg(0)?, Effects::of(n))))
            .filter(|(s, fx)| *s != "~" && !s.is_empty() && !fx.hidden);
        let number = pin
            .child("number")
            .and_then(|n| Some((n.arg(0)?, Effects::of(n))))
            .filter(|(s, fx)| !s.is_empty() && !fx.hidden);
        if let (Some((s, mut fx)), false) = (name, names_hidden) {
            if names_offset > 0.0 {
                fx.halign = if toward > 0.0 {
                    HAlign::Left
                } else {
                    HAlign::Right
                };
                fx.valign = VAlign::Middle;
                self.text(
                    page,
                    SYMBOLS,
                    colour::PIN_NAME,
                    s,
                    s1 + dir * names_offset,
                    deg,
                    &fx,
                );
            } else {
                fx.halign = HAlign::Center;
                fx.valign = VAlign::Bottom;
                self.text(
                    page,
                    SYMBOLS,
                    colour::PIN_NAME,
                    s,
                    mid + up * PIN_GAP,
                    deg,
                    &fx,
                );
            }
        }
        if let (Some((s, mut fx)), false) = (number, numbers_hidden) {
            fx.halign = HAlign::Center;
            if names_offset > 0.0 || names_hidden || name.is_none() {
                fx.valign = VAlign::Bottom;
                self.text(
                    page,
                    SYMBOLS,
                    colour::PIN_NUMBER,
                    s,
                    mid + up * PIN_GAP,
                    deg,
                    &fx,
                );
            } else {
                fx.valign = VAlign::Top;
                self.text(
                    page,
                    SYMBOLS,
                    colour::PIN_NUMBER,
                    s,
                    mid - up * PIN_GAP,
                    deg,
                    &fx,
                );
            }
        }
    }

    /// The paper, its border and the title block (values only).
    fn frame(&mut self, page: &mut Page, n: &Node<'_>, ctx: &Ctx, w: f64, h: f64) {
        let c = colour::FRAME;
        let r = |x0: f64, y0: f64, x1: f64, y1: f64| Rect::new(x0, -y1, x1, -y0).to_path(TOLERANCE);
        self.path(page, FRAME, c, &r(10.0, 10.0, w - 10.0, h - 10.0), LINE_W);
        self.path(page, FRAME, c, &r(12.0, 12.0, w - 12.0, h - 12.0), LINE_W);
        // Title block: bottom right, 110 × 32 mm.
        let (x0, y0, x1, y1) = (w - 12.0 - 110.0, h - 12.0 - 32.0, w - 12.0, h - 12.0);
        self.path(page, FRAME, c, &r(x0, y0, x1, y1), LINE_W);
        for y in [y0 + 8.0, y0 + 16.0, y0 + 24.0] {
            self.line(page, FRAME, c, pt(x0, y), pt(x1, y), LINE_W);
        }
        let tb = n.child("title_block");
        let v = |k: &str| tb.and_then(|t| t.value(k)).unwrap_or("").to_owned();
        let mut fx = Effects {
            halign: HAlign::Left,
            valign: VAlign::Middle,
            size: (1.5, 1.5),
            ..Effects::default()
        };
        let mut rows: Vec<(String, f64)> =
            vec![(v("company"), y0 + 4.0), ([v("title")].join(""), y0 + 12.0)];
        let mut meta = vec![v("date"), v("rev")];
        meta.retain(|s| !s.is_empty());
        rows.push((meta.join("   "), y0 + 20.0));
        let mut last = vec![ctx.name.clone()];
        if !ctx.file.is_empty() {
            last.push(ctx.file.clone());
        }
        if let Some(p) = &ctx.page {
            last.push(format!("#{p}"));
        }
        rows.push((last.join("   "), y0 + 28.0));
        for (i, (s, y)) in rows.into_iter().enumerate() {
            fx.size = if i == 1 { (2.0, 2.0) } else { (1.5, 1.5) };
            fx.bold = i == 1;
            self.text(page, FRAME, c, &s, pt(x0 + 2.0, y), 0.0, &fx);
        }
        // Comments above the title block.
        if let Some(tb) = tb {
            let mut y = y0 - 3.0;
            for cm in tb.children("comment") {
                if let Some(s) = cm.arg(1) {
                    fx.size = (1.27, 1.27);
                    fx.bold = false;
                    self.text(page, FRAME, c, s, pt(x0 + 2.0, y), 0.0, &fx);
                    y -= 2.5;
                }
            }
        }
    }

    fn finish(mut self, root: &Node<'_>, version: u64, title: Option<String>) -> VectorScene {
        if self.truncated {
            warn(
                &mut self.warnings,
                format!(
                    "More than {} shapes: the rest is left out",
                    self.limits.items
                ),
            );
        }
        let layers = LAYERS
            .iter()
            .zip(self.layer_items)
            .map(|(name, items)| {
                let c = match *name {
                    "Frame" => colour::FRAME,
                    "Graphics" | "Text" => colour::NOTE,
                    "Sheets" => colour::SHEET,
                    "Symbols" => colour::BODY,
                    "Wires" => colour::WIRE,
                    "Buses" => colour::BUS,
                    _ => colour::GLOBAL,
                };
                Layer {
                    name: (*name).to_owned(),
                    generated: None,
                    color: rgb(c),
                    linetype: None,
                    lineweight: 0,
                    on: true,
                    frozen: false,
                    plot: true,
                    items,
                }
            })
            .collect();
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
        let c = &self.counts;
        let info = PcbInfo {
            kind: PcbKind::KicadSchematic,
            generator,
            project: title.or_else(|| tbv("company")),
            revision: tbv("rev"),
            date: tbv("date"),
            sheets: self.pages.len() as u32,
            components: c.symbols,
            tracks: c.wires,
            ..PcbInfo::default()
        };
        let mut counts: Vec<(String, u64)> = [
            ("bus", c.buses),
            ("directive label", c.directives),
            ("junction", c.junctions),
            ("label", c.labels),
            ("power symbol", c.power),
            ("sheet", c.sheets),
            ("symbol", c.symbols),
            ("text", c.texts),
            ("wire", c.wires),
        ]
        .into_iter()
        .filter(|e| e.1 > 0)
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        counts.sort();
        VectorScene {
            format: "KiCad".into(),
            version: version.to_string(),
            release: Some(schematic_release(version).into()),
            units: Units { code: 4 },
            layers,
            fonts: vec![FontSpec::default()],
            sheets: self.pages,
            counts,
            warnings: self.warnings,
            pcb: Some(Box::new(info)),
            ..VectorScene::default()
        }
    }
}

/// Text colour from `(effects (font (color …)))`.
fn own_colour_fx(n: &Node<'_>) -> Option<[u8; 3]> {
    own_colour(n.child("effects")?.child("font")?)
}

/// Schematic text angles shown upright: 0 or 90.
fn upright(a: f64) -> f64 {
    if is_vertical(a) { 90.0 } else { 0.0 }
}

fn is_vertical(a: f64) -> bool {
    matches!((a.round() as i64).rem_euclid(180), 45..=135)
}

/// Whether a placement turns its symbol by a quarter (90° or 270°).
fn turned(a: f64) -> bool {
    is_vertical(a)
}

/// A library symbol's parent when it `extends` one and has no drawing of its own.
fn resolve_extends<'n, 'a>(lib: &'n Node<'a>, libs: &HashMap<&str, &'n Node<'a>>) -> &'n Node<'a> {
    let mut cur = lib;
    for _ in 0..8 {
        let Some(parent) = cur.value("extends") else {
            break;
        };
        if cur.child("symbol").is_some() {
            break;
        }
        let name = cur.arg(0).unwrap_or("");
        let full = match name.split_once(':') {
            Some((lib_name, _)) => format!("{lib_name}:{parent}"),
            None => parent.to_owned(),
        };
        match libs.get(full.as_str()).or(libs.get(parent)) {
            Some(p) => cur = p,
            None => break,
        }
    }
    cur
}

/// Unit and body style of a sub-symbol named `Name_unit_style`.
fn unit_style(name: &str) -> Option<(u32, u32)> {
    let mut it = name.rsplitn(3, '_');
    let style = it.next()?.parse().ok()?;
    let unit = it.next()?.parse().ok()?;
    Some((unit, style))
}

/// How many units a library symbol has.
fn unit_count(lib: &Node<'_>) -> u32 {
    lib.children("symbol")
        .filter_map(|s| s.arg(0).and_then(unit_style))
        .map(|(u, _)| u)
        .max()
        .unwrap_or(1)
        .max(1)
}

/// `(fill (type …))`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fill {
    None,
    Outline,
    Background,
    Colour([u8; 3]),
}

impl Fill {
    fn colour(self, stroke: [u8; 3]) -> Option<[u8; 3]> {
        match self {
            Fill::None => None,
            Fill::Outline => Some(stroke),
            Fill::Background => Some(colour::BODY_FILL),
            Fill::Colour(c) => Some(c),
        }
    }
}

fn fill_of(n: &Node<'_>) -> Fill {
    let Some(f) = n.child("fill") else {
        return Fill::None;
    };
    match f.value("type").or(f.arg(0)) {
        Some("outline") => Fill::Outline,
        Some("background") => Fill::Background,
        Some("color") => own_colour(f).map_or(Fill::None, Fill::Colour),
        _ => Fill::None,
    }
}

/// A global label's outline for a body `len` long and `2 hh` high, pointing along +x from
/// the connection point at the origin.
fn flag(shape: &str, len: f64, hh: f64, _global: bool) -> Vec<Point> {
    let p = Point::new;
    match shape {
        "input" => vec![p(0.0, 0.0), p(hh, hh), p(len, hh), p(len, -hh), p(hh, -hh)],
        "output" => vec![
            p(0.0, hh),
            p(len - hh, hh),
            p(len, 0.0),
            p(len - hh, -hh),
            p(0.0, -hh),
        ],
        "bidirectional" | "tri_state" => vec![
            p(0.0, 0.0),
            p(hh, hh),
            p(len - hh, hh),
            p(len, 0.0),
            p(len - hh, -hh),
            p(hh, -hh),
        ],
        _ => vec![p(0.0, hh), p(len, hh), p(len, -hh), p(0.0, -hh)],
    }
}

/// A hierarchical label's small shape, `2 hh` across, from the origin along +x.
fn hier_shape(shape: &str, hh: f64) -> Vec<Point> {
    let p = Point::new;
    let w = 2.0 * hh;
    match shape {
        "input" => vec![p(0.0, hh), p(hh, hh), p(w, 0.0), p(hh, -hh), p(0.0, -hh)],
        "output" => vec![p(0.0, 0.0), p(hh, hh), p(w, hh), p(w, -hh), p(hh, -hh)],
        "bidirectional" | "tri_state" => vec![p(0.0, 0.0), p(hh, hh), p(w, 0.0), p(hh, -hh)],
        _ => vec![p(0.0, hh), p(w, hh), p(w, -hh), p(0.0, -hh)],
    }
}

/// Bounds of a page's items (text by its insertion point).
fn bounds(items: &[Item]) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    for it in items {
        let b = match &it.shape {
            Shape::Fill { path, .. } | Shape::Stroke { path, .. } => path.bounding_box(),
            Shape::Text(t) => {
                let p = t.transform * Point::ORIGIN;
                Rect::from_points(p, p)
            }
            Shape::Point(p) => Rect::from_points(*p, *p),
        };
        if b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite() {
            r = Some(r.map_or(b, |r| r.union(b)));
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_styles_come_from_names() {
        assert_eq!(unit_style("R_0_1"), Some((0, 1)));
        assert_eq!(unit_style("74xx_LS00_2_1"), Some((2, 1)));
        assert_eq!(unit_style("R"), None);
    }

    #[test]
    fn papers() {
        let n = sexpr::parse("(paper \"A3\" portrait)").unwrap();
        assert_eq!(paper_size(Some(&n)), (297.0, 420.0));
        let n = sexpr::parse("(paper \"User\" 100 50)").unwrap();
        assert_eq!(paper_size(Some(&n)), (100.0, 50.0));
    }
}
