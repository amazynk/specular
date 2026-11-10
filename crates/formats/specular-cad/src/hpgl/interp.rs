//! The plotter: HP-GL and HP-GL/2 commands applied to a pen, turned into scene items.
//!
//! Everything is computed in plotter units (plu, 40 per millimetre) of the current
//! orientation and converted to millimetres (with `RO` applied) when an item is emitted, so
//! a page's geometry is in true millimetres. User units (`SC`) map onto the scaling points
//! `P1`/`P2` (`IP`, `IR`); relative moves (`PR`) use the linear part of that mapping.

use std::collections::HashMap;
use std::f64::consts::PI;

use kurbo::{Affine, Arc, BezPath, Circle, PathEl, Point, Rect, Shape as _, Vec2};
use specular_vector::{
    Color, Dash, Generated, HAlign, Item, Layer, Linetype, Lineweight, Paragraph, Shape, Span,
    TextBlock, VAlign,
};

use super::lex::{self, Event, Lexer};
use crate::Limits;
use crate::emit::item_bounds;
use crate::hatch::{self, PatternLine};

/// Plotter units per millimetre.
pub(crate) const PLU_PER_MM: f64 = 40.0;
/// Curve flattening tolerance for hatching, plu.
const TOL: f64 = 0.5;
/// Default stick-font character size (`SI`), cm: width and cap height.
const SI_DEFAULT: (f64, f64) = (0.187, 0.269);
/// Most distinct unknown commands named in the warnings.
const MAX_UNKNOWN: usize = 8;

/// Commands understood or deliberately ignored (no warning).
pub(crate) const KNOWN: &[&[u8; 2]] = &[
    b"AA", b"AC", b"AD", b"AF", b"AH", b"AP", b"AR", b"AS", b"AT", b"BL", b"BP", b"BR", b"BZ",
    b"CA", b"CC", b"CF", b"CI", b"CM", b"CO", b"CP", b"CR", b"CS", b"CT", b"CV", b"DC", b"DF",
    b"DI", b"DL", b"DP", b"DR", b"DS", b"DT", b"DV", b"EA", b"EC", b"EP", b"ER", b"ES", b"EW",
    b"FI", b"FN", b"FP", b"FR", b"FS", b"FT", b"GC", b"GM", b"GP", b"IM", b"IN", b"IP", b"IR",
    b"IV", b"IW", b"KY", b"LA", b"LB", b"LM", b"LO", b"LT", b"MC", b"MG", b"MT", b"NP", b"NR",
    b"OA", b"OC", b"OD", b"OE", b"OF", b"OH", b"OI", b"OL", b"OO", b"OP", b"OS", b"OT", b"OW",
    b"PA", b"PB", b"PC", b"PD", b"PE", b"PG", b"PM", b"PP", b"PR", b"PS", b"PT", b"PU", b"PW",
    b"QL", b"RA", b"RF", b"RO", b"RP", b"RR", b"RT", b"SA", b"SB", b"SC", b"SD", b"SG", b"SI",
    b"SL", b"SM", b"SP", b"SR", b"SS", b"ST", b"SV", b"TD", b"TL", b"TR", b"UC", b"UF", b"UL",
    b"VA", b"VN", b"VS", b"WD", b"WG", b"WU", b"XT", b"YT",
];

pub(crate) fn known(mn: &[u8; 2]) -> bool {
    KNOWN.binary_search(&mn).is_ok()
}

/// User-unit scaling (`SC`).
#[derive(Clone, Copy, Debug)]
enum Scale {
    /// Anisotropic (type 0): user rectangle onto P1–P2.
    Aniso { x0: f64, x1: f64, y0: f64, y1: f64 },
    /// Isotropic (type 1): one factor, the slack placed by `left`/`bottom` percent.
    Iso {
        x0: f64,
        x1: f64,
        y0: f64,
        y1: f64,
        left: f64,
        bottom: f64,
    },
    /// Point factor (type 2): plu per user unit.
    Factor { x0: f64, fx: f64, y0: f64, fy: f64 },
}

/// A line pattern in force: the pattern (fractions, dashes positive, gaps negative) and
/// its length in plu.
#[derive(Clone, Debug, PartialEq)]
struct LineType {
    id: i32,
    pattern: Vec<f64>,
    length: f64,
}

/// Fill type (`FT`).
#[derive(Clone, Copy, Debug)]
struct Fill {
    kind: i32,
    /// Hatch spacing in plu (0 = the default) or shading level in percent.
    option: f64,
    /// Hatch angle, degrees.
    angle: f64,
}

/// Font settings of one of the two fonts (`SD`, `AD`).
#[derive(Clone, Copy, Debug)]
struct Font {
    /// Fixed spacing (else proportional).
    fixed: bool,
    /// Characters per inch.
    pitch: f64,
    /// Point size.
    height: f64,
    /// Italic.
    italic: bool,
    /// Bold (stroke weight above 0).
    bold: bool,
}

impl Default for Font {
    fn default() -> Font {
        Font {
            fixed: true,
            pitch: 9.0,
            height: 11.5,
            italic: false,
            bold: false,
        }
    }
}

/// One plotted page.
pub(crate) struct Page {
    pub items: Vec<Item>,
    pub extents: Option<Rect>,
}

/// What the run produced.
pub(crate) struct Output {
    pub pages: Vec<Page>,
    pub layers: Vec<Layer>,
    pub linetypes: Vec<Linetype>,
    pub counts: Vec<(String, u64)>,
    pub warnings: Vec<String>,
    pub hpgl2: bool,
    pub pcl: bool,
    pub pens: Vec<u32>,
    pub paper_mm: Option<(f64, f64)>,
    pub name: Option<String>,
    pub widths_seen: bool,
    /// Commands not understood (all occurrences).
    pub unknown: u64,
    /// Commands understood (all occurrences).
    pub commands: u64,
}

pub(crate) struct Interp<'l> {
    limits: &'l Limits,
    /// Hatch segments generated so far ([`crate::hatch::MAX_TOTAL`]).
    hatched: usize,
    hpgl2: bool,
    // Frame.
    /// Hard-clip size, plu, in the unrotated orientation.
    paper: Vec2,
    /// The size the file states (`PS`, the PCL picture frame).
    paper_stated: Option<Vec2>,
    frame: (Option<f64>, Option<f64>),
    p1: Point,
    p2: Point,
    ip_set: bool,
    ro: i32,
    scale: Option<Scale>,
    /// User units → plu.
    ux: Affine,
    // Pen.
    pos: Point,
    down: bool,
    relative: bool,
    pen: u32,
    pens: u32,
    widths: HashMap<u32, f64>,
    width_default: f64,
    width_relative: bool,
    colors: HashMap<u32, Color>,
    color_range: [f64; 6],
    widths_seen: bool,
    // Lines and fills.
    line: Option<LineType>,
    line_prev: Option<LineType>,
    user_lines: HashMap<i32, Vec<f64>>,
    fill: Fill,
    anchor: Point,
    ticks: (f64, f64),
    // Polygon mode.
    pm: Option<BezPath>,
    pm_new: bool,
    poly: BezPath,
    // Labels.
    si: Option<(f64, f64)>,
    sr: Option<(f64, f64)>,
    standard: Font,
    alternate: Font,
    use_alternate: bool,
    dir: Option<(f64, f64, bool)>,
    slant: f64,
    origin: i32,
    term_printed: Option<u8>,
    symbol: Option<u8>,
    cr_point: Point,
    // Output.
    cur: BezPath,
    cur_end: Option<Point>,
    cur_points: usize,
    items: Vec<Item>,
    pages: Vec<Page>,
    layers: Vec<Layer>,
    pen_layer: HashMap<u32, u32>,
    linetypes: Vec<Linetype>,
    total: usize,
    pub(crate) stop: bool,
    counts: HashMap<[u8; 2], u64>,
    unknown: Vec<String>,
    unknown_total: u64,
    commands: u64,
    warnings: Vec<String>,
    nums: Vec<f64>,
    name: Option<String>,
    pcl: bool,
}

/// Whether the first 64 KiB use HP-GL/2-only commands.
pub(crate) fn looks_hpgl2(b: &[u8]) -> bool {
    let head = &b[..b.len().min(65536)];
    let mut lx = Lexer::new(head);
    let mut n = 0;
    while let Some(e) = lx.next_event() {
        match e {
            Event::EnterHpgl2 => return true,
            Event::Cmd(mn, _) => {
                if matches!(
                    &mn,
                    b"BP"
                        | b"PE"
                        | b"PW"
                        | b"NP"
                        | b"PC"
                        | b"WU"
                        | b"FT"
                        | b"LA"
                        | b"SD"
                        | b"BZ"
                        | b"BR"
                        | b"PS"
                        | b"TR"
                        | b"CR"
                        | b"PG"
                ) {
                    return true;
                }
                n += 1;
                if n > 5000 {
                    break;
                }
            }
            _ => {}
        }
    }
    false
}

impl<'l> Interp<'l> {
    pub(crate) fn new(limits: &'l Limits, hpgl2: bool) -> Interp<'l> {
        let mut it = Interp {
            limits,
            hatched: 0,
            hpgl2,
            paper: Vec2::ZERO,
            paper_stated: None,
            frame: (None, None),
            p1: Point::ZERO,
            p2: Point::ZERO,
            ip_set: false,
            ro: 0,
            scale: None,
            ux: Affine::IDENTITY,
            pos: Point::ZERO,
            down: false,
            relative: false,
            pen: 1,
            pens: 8,
            widths: HashMap::new(),
            width_default: 0.35,
            width_relative: false,
            colors: HashMap::new(),
            color_range: [0.0, 255.0, 0.0, 255.0, 0.0, 255.0],
            widths_seen: false,
            line: None,
            line_prev: None,
            user_lines: HashMap::new(),
            fill: Fill {
                kind: 1,
                option: 0.0,
                angle: 0.0,
            },
            anchor: Point::ZERO,
            ticks: (0.5, 0.5),
            pm: None,
            pm_new: false,
            poly: BezPath::new(),
            si: None,
            sr: None,
            standard: Font::default(),
            alternate: Font::default(),
            use_alternate: false,
            dir: None,
            slant: 0.0,
            origin: 1,
            term_printed: None,
            symbol: None,
            cr_point: Point::ZERO,
            cur: BezPath::new(),
            cur_end: None,
            cur_points: 0,
            items: Vec::new(),
            pages: Vec::new(),
            layers: Vec::new(),
            pen_layer: HashMap::new(),
            linetypes: Vec::new(),
            total: 0,
            stop: false,
            counts: HashMap::new(),
            unknown: Vec::new(),
            unknown_total: 0,
            commands: 0,
            warnings: Vec::new(),
            nums: Vec::new(),
            name: None,
            pcl: false,
        };
        it.set_paper(None);
        it
    }

    /// Runs the whole file.
    pub(crate) fn run(mut self, b: &[u8]) -> Output {
        let mut lx = Lexer::new(b);
        while !self.stop {
            let Some(e) = lx.next_event() else {
                break;
            };
            match e {
                Event::Cmd(mn, args) => self.command(mn, args),
                Event::Label(mn, text) => {
                    self.count(mn);
                    if mn == *b"LB" {
                        self.label(text);
                    }
                }
                Event::Terminator(t, printed) => {
                    self.count(*b"DT");
                    self.term_printed = printed.then_some(t);
                }
                Event::PageEnd => {
                    self.page_end();
                    self.down = false;
                }
                Event::FrameX(v) => {
                    self.frame.0 = Some(v);
                    self.frame_changed();
                }
                Event::FrameY(v) => {
                    self.frame.1 = Some(v);
                    self.frame_changed();
                }
                Event::Orientation(_) | Event::Pcl => {}
                Event::EnterHpgl2 => self.hpgl2 = true,
            }
        }
        self.pcl = lx.wrapped;
        self.page_end();
        self.finish()
    }

    fn count(&mut self, mn: [u8; 2]) {
        *self.counts.entry(mn).or_insert(0) += 1;
    }

    // ----- Frame -----

    /// A PCL picture frame (decipoints) is the paper of the HP-GL/2 picture.
    fn frame_changed(&mut self) {
        if let (Some(x), Some(y)) = self.frame {
            let to_plu = 25.4 / 720.0 * PLU_PER_MM;
            if x > 0.0 && y > 0.0 && x.is_finite() && y.is_finite() {
                self.set_paper(Some(Vec2::new(x * to_plu, y * to_plu)));
            }
        }
    }

    fn set_paper(&mut self, stated: Option<Vec2>) {
        if let Some(s) = stated {
            self.paper_stated = Some(s);
        }
        self.paper = self.paper_stated.unwrap_or(if self.hpgl2 {
            // A4 landscape.
            Vec2::new(297.0 * PLU_PER_MM, 210.0 * PLU_PER_MM)
        } else {
            Vec2::new(10900.0, 7650.0)
        });
        self.default_p1p2();
    }

    /// P1 and P2 at their defaults for the paper and orientation.
    fn default_p1p2(&mut self) {
        let size = if self.ro % 180 != 0 {
            Vec2::new(self.paper.y, self.paper.x)
        } else {
            self.paper
        };
        if self.hpgl2 || self.paper_stated.is_some() {
            self.p1 = Point::ZERO;
            self.p2 = size.to_point();
        } else {
            // The 7475A's A-size limits (its P1/P2 sit inside the hard-clip limits).
            self.p1 = Point::new(250.0, 596.0);
            self.p2 = Point::new(10250.0, 7796.0);
            if self.ro % 180 != 0 {
                self.p1 = Point::new(self.p1.y, self.p1.x);
                self.p2 = Point::new(self.p2.y, self.p2.x);
            }
        }
        self.ip_set = false;
        self.update_scale();
    }

    fn diagonal(&self) -> f64 {
        (self.p2 - self.p1).hypot().max(1.0)
    }

    fn update_scale(&mut self) {
        let (p1, p2) = (self.p1, self.p2);
        let d = p2 - p1;
        self.ux = match self.scale {
            None => Affine::IDENTITY,
            Some(Scale::Aniso { x0, x1, y0, y1 }) => {
                let sx = if x1 != x0 { d.x / (x1 - x0) } else { 1.0 };
                let sy = if y1 != y0 { d.y / (y1 - y0) } else { 1.0 };
                Affine::new([sx, 0.0, 0.0, sy, p1.x - x0 * sx, p1.y - y0 * sy])
            }
            Some(Scale::Iso {
                x0,
                x1,
                y0,
                y1,
                left,
                bottom,
            }) => {
                let fx = if x1 != x0 { d.x / (x1 - x0) } else { 1.0 };
                let fy = if y1 != y0 { d.y / (y1 - y0) } else { 1.0 };
                let f = if fx.abs() < fy.abs() {
                    fx.abs()
                } else {
                    fy.abs()
                };
                let (sx, sy) = (f * fx.signum(), f * fy.signum());
                let slack_x = d.x - (x1 - x0) * sx;
                let slack_y = d.y - (y1 - y0) * sy;
                Affine::new([
                    sx,
                    0.0,
                    0.0,
                    sy,
                    p1.x + slack_x * left / 100.0 - x0 * sx,
                    p1.y + slack_y * bottom / 100.0 - y0 * sy,
                ])
            }
            Some(Scale::Factor { x0, fx, y0, fy }) => {
                Affine::new([fx, 0.0, 0.0, fy, p1.x - x0 * fx, p1.y - y0 * fy])
            }
        };
        let c = self.ux.as_coeffs();
        if !c.iter().all(|v| v.is_finite()) || c[0] == 0.0 || c[3] == 0.0 {
            self.ux = Affine::IDENTITY;
        }
    }

    /// Plu → millimetres of the page.
    fn out(&self) -> Affine {
        Affine::rotate((self.ro as f64).to_radians()) * Affine::scale(1.0 / PLU_PER_MM)
    }

    /// A user-unit point.
    fn user(&self, x: f64, y: f64) -> Point {
        self.ux * Point::new(x, y)
    }

    /// A user-unit displacement.
    fn user_delta(&self, x: f64, y: f64) -> Vec2 {
        let c = self.ux.as_coeffs();
        Vec2::new(c[0] * x + c[2] * y, c[1] * x + c[3] * y)
    }

    /// A user-unit length along x (radii, spacing).
    fn user_len(&self, r: f64) -> f64 {
        let c = self.ux.as_coeffs();
        r * c[0].hypot(c[1])
    }

    /// Whether arcs turn the other way (a mirrored scaling).
    fn mirrored(&self) -> bool {
        self.ux.determinant() < 0.0
    }

    // ----- Pens -----

    fn pen_color(&self, pen: u32) -> Color {
        if let Some(c) = self.colors.get(&pen) {
            return *c;
        }
        default_color(pen)
    }

    fn pen_width(&self) -> f64 {
        let w = self
            .widths
            .get(&self.pen)
            .copied()
            .unwrap_or(self.width_default);
        if self.width_relative {
            w / 100.0 * self.diagonal() / PLU_PER_MM
        } else {
            w
        }
    }

    fn weight(&self) -> Lineweight {
        (self.pen_width() * 100.0)
            .round()
            .clamp(0.0, i16::MAX as f64) as Lineweight
    }

    /// Whether the selected pen draws: `SP0` puts the pen away on a pen plotter.
    fn drawing(&self) -> bool {
        self.hpgl2 || self.pen != 0
    }

    fn layer(&mut self) -> u32 {
        if let Some(&l) = self.pen_layer.get(&self.pen) {
            return l;
        }
        let i = self.layers.len() as u32;
        self.layers.push(Layer {
            name: Generated::Pen(self.pen).english(),
            generated: Some(Generated::Pen(self.pen)),
            color: self.pen_color(self.pen),
            linetype: None,
            lineweight: self.weight(),
            on: true,
            frozen: false,
            plot: true,
            items: 0,
        });
        self.pen_layer.insert(self.pen, i);
        i
    }

    fn dash(&mut self) -> Option<Dash> {
        let lt = self.line.as_ref()?;
        if lt.length <= 0.0 {
            return None;
        }
        let pattern = lt.pattern.clone();
        let length = lt.length / PLU_PER_MM;
        let id = lt.id;
        let i = match self.linetypes.iter().position(|l| l.pattern == pattern) {
            Some(i) => i,
            None => {
                self.linetypes.push(Linetype {
                    name: format!("LT {id}"),
                    description: String::new(),
                    pattern,
                });
                self.linetypes.len() - 1
            }
        };
        Some(Dash {
            linetype: i as u32,
            scale: length,
        })
    }

    /// Adds an item in the current pen.
    fn emit(&mut self, shape: Shape, color: Option<Color>) {
        if self.stop {
            return;
        }
        if self.total >= self.limits.max_items {
            self.stop = true;
            self.warnings.push(format!(
                "the plot was cut short after {} items",
                self.limits.max_items
            ));
            return;
        }
        let layer = self.layer();
        self.layers[layer as usize].items += 1;
        self.total += 1;
        self.items.push(Item {
            layer,
            outer: layer,
            color: color.unwrap_or_else(|| self.pen_color(self.pen)),
            shape,
        });
    }

    /// Emits the polyline being drawn.
    fn flush(&mut self) {
        if self.cur.elements().len() > 1 && self.drawing() {
            let mut path = std::mem::take(&mut self.cur);
            path.apply_affine(self.out());
            let shape = Shape::Stroke {
                path,
                weight: self.weight(),
                dash: self.dash(),
            };
            self.emit(shape, None);
        }
        self.cur.truncate(0);
        self.cur_end = None;
        self.cur_points = 0;
    }

    /// Strokes a path of its own (plu) in the current pen and line type.
    fn stroke(&mut self, mut path: BezPath) {
        if !self.drawing() || path.elements().len() < 2 {
            return;
        }
        self.flush();
        path.apply_affine(self.out());
        let shape = Shape::Stroke {
            path,
            weight: self.weight(),
            dash: self.dash(),
        };
        self.emit(shape, None);
    }

    /// Fills a closed path (plu) with the fill type in force.
    fn fill_path(&mut self, path: BezPath, even_odd: bool) {
        if !self.drawing() || path.elements().len() < 2 {
            return;
        }
        self.flush();
        let pen = self.pen_color(self.pen);
        match self.fill.kind {
            3 | 4 => {
                let spacing = if self.fill.option > 0.0 {
                    self.user_len(self.fill.option)
                } else {
                    0.01 * self.diagonal()
                };
                if !(spacing.is_finite() && spacing > 0.0) {
                    return;
                }
                let segs = hatch::segments(std::slice::from_ref(&path), TOL);
                let bounds = path.bounding_box();
                let mut lines = Vec::new();
                let turns: &[f64] = if self.fill.kind == 4 {
                    &[0.0, 90.0]
                } else {
                    &[0.0]
                };
                for t in turns {
                    let a = (self.fill.angle + t).to_radians();
                    lines.push(PatternLine {
                        angle: a,
                        base: self.anchor,
                        offset: Vec2::new(-a.sin(), a.cos()) * spacing,
                        dashes: Vec::new(),
                    });
                }
                // A few bytes of fill commands can ask for millions of segments each: the
                // file's hatches share one budget, then fill solid.
                let cap = self
                    .limits
                    .max_points
                    .min(hatch::MAX_TOTAL.saturating_sub(self.hatched));
                let (mut hatch_path, cut) = hatch::pattern(&segs, bounds, &lines, cap);
                self.hatched += hatch_path.elements().len();
                if cut {
                    self.warnings
                        .push("a hatch fill was too dense and was drawn solid".into());
                    let mut p = path;
                    p.apply_affine(self.out());
                    self.emit(Shape::Fill { path: p, even_odd }, None);
                    return;
                }
                hatch_path.apply_affine(self.out());
                let shape = Shape::Stroke {
                    path: hatch_path,
                    weight: self.weight(),
                    dash: None,
                };
                self.emit(shape, None);
            }
            10 => {
                let level = self.fill.option.clamp(0.0, 100.0) / 100.0;
                let mut p = path;
                p.apply_affine(self.out());
                let c = shade(pen, level);
                self.emit(Shape::Fill { path: p, even_odd }, Some(c));
            }
            _ => {
                let mut p = path;
                p.apply_affine(self.out());
                self.emit(Shape::Fill { path: p, even_odd }, None);
            }
        }
    }

    // ----- Motion -----

    /// Moves the pen to `to` (plu), drawing when it is down.
    fn go(&mut self, to: Point) {
        if !(to.x.is_finite() && to.y.is_finite()) {
            return;
        }
        if let Some(pm) = &mut self.pm {
            if self.down {
                if self.pm_new || pm.elements().is_empty() {
                    pm.move_to(self.pos);
                    self.pm_new = false;
                }
                pm.line_to(to);
            } else {
                self.pm_new = true;
            }
            self.pos = to;
            return;
        }
        if self.down && self.drawing() {
            if self.cur_end != Some(self.pos) {
                self.flush();
                self.cur.move_to(self.pos);
            }
            self.cur.line_to(to);
            self.cur_end = Some(to);
            self.cur_points += 1;
            if self.cur_points >= self.limits.max_points {
                self.flush();
            }
        }
        self.pos = to;
        self.cr_point = to;
        if let Some(c) = self.symbol {
            self.symbol_at(c);
        }
    }

    /// Continues the current path with curve elements ending at `end` (plu).
    fn trace(&mut self, els: &[PathEl], end: Point) {
        if let Some(pm) = &mut self.pm {
            if self.down {
                if self.pm_new || pm.elements().is_empty() {
                    pm.move_to(self.pos);
                    self.pm_new = false;
                }
                for &e in els {
                    pm.push(e);
                }
            } else {
                self.pm_new = true;
            }
            self.pos = end;
            return;
        }
        if self.down && self.drawing() {
            if self.cur_end != Some(self.pos) {
                self.flush();
                self.cur.move_to(self.pos);
            }
            for &e in els {
                self.cur.push(e);
            }
            self.cur_end = Some(end);
            self.cur_points += els.len();
            if self.cur_points >= self.limits.max_points {
                self.flush();
            }
        }
        self.pos = end;
        self.cr_point = end;
    }

    /// Pairs of coordinates of `PA`, `PR`, `PU`, `PD`.
    fn moves(&mut self, nums: &[f64], relative: bool) {
        for p in nums.chunks_exact(2) {
            let to = if relative {
                self.pos + self.user_delta(p[0], p[1])
            } else {
                self.user(p[0], p[1])
            };
            self.go(to);
            if self.stop {
                break;
            }
        }
    }

    /// An arc about `center` (plu) from the pen through `sweep` degrees.
    fn arc(&mut self, center: Point, sweep: f64) {
        let sweep = if self.mirrored() { -sweep } else { sweep };
        let r = (self.pos - center).hypot();
        if !(r.is_finite() && sweep.is_finite()) {
            return;
        }
        let sweep = sweep.clamp(-3600.0, 3600.0).to_radians();
        let a0 = (self.pos.y - center.y).atan2(self.pos.x - center.x);
        let end = center + Vec2::new((a0 + sweep).cos(), (a0 + sweep).sin()) * r;
        if r == 0.0 || sweep == 0.0 {
            self.go(end);
            return;
        }
        let arc = Arc::new(center, Vec2::new(r, r), a0, sweep, 0.0);
        let els: Vec<PathEl> = arc.append_iter(curve_tol(r)).collect();
        self.trace(&els, end);
    }

    /// A three-point arc from the pen through `mid` to `end` (plu).
    fn arc3(&mut self, mid: Point, end: Point) {
        let a = self.pos;
        let (bx, by) = (mid.x - a.x, mid.y - a.y);
        let (cx, cy) = (end.x - a.x, end.y - a.y);
        let d = 2.0 * (bx * cy - by * cx);
        if d.abs()
            < 1e-9
                * (bx.abs() + by.abs() + cx.abs() + cy.abs())
                    .max(1e-300)
                    .powi(2)
        {
            // Collinear: a line (through `mid` when it lies beyond).
            self.go(end);
            return;
        }
        let ux = (cy * (bx * bx + by * by) - by * (cx * cx + cy * cy)) / d;
        let uy = (bx * (cx * cx + cy * cy) - cx * (bx * bx + by * by)) / d;
        let center = Point::new(a.x + ux, a.y + uy);
        let ang = |p: Point| (p.y - center.y).atan2(p.x - center.x);
        let (t0, tm, t1) = (ang(a), ang(mid), ang(end));
        let norm = |x: f64| x.rem_euclid(2.0 * PI);
        let ccw_mid = norm(tm - t0);
        let ccw_end = norm(t1 - t0);
        let sweep = if ccw_mid <= ccw_end {
            ccw_end
        } else {
            ccw_end - 2.0 * PI
        };
        let r = (a - center).hypot();
        let arc = Arc::new(center, Vec2::new(r, r), t0, sweep, 0.0);
        let els: Vec<PathEl> = arc.append_iter(curve_tol(r)).collect();
        self.trace(&els, end);
    }

    /// A wedge (plu path): pen at the centre, radius, start angle and sweep in degrees.
    fn wedge(&self, r: f64, start: f64, sweep: f64) -> BezPath {
        let c = self.pos;
        let r = self.user_len(r);
        let (start, sweep) = if self.mirrored() {
            (-start, -sweep)
        } else {
            (start, sweep)
        };
        let a0 = start.to_radians();
        let sw = sweep.clamp(-360.0, 360.0).to_radians();
        let mut p = BezPath::new();
        if sw.abs() >= 2.0 * PI - 1e-9 {
            p.extend(Circle::new(c, r.abs()).path_elements(curve_tol(r)));
            return p;
        }
        p.move_to(c);
        p.line_to(c + Vec2::new(a0.cos(), a0.sin()) * r);
        let arc = Arc::new(c, Vec2::new(r, r), a0, sw, 0.0);
        p.extend(arc.append_iter(curve_tol(r)));
        p.close_path();
        p
    }

    /// A rectangle from the pen to `corner` (plu).
    fn rect(&self, corner: Point) -> BezPath {
        let a = self.pos;
        let mut p = BezPath::new();
        p.move_to(a);
        p.line_to((corner.x, a.y));
        p.line_to(corner);
        p.line_to((a.x, corner.y));
        p.close_path();
        p
    }

    /// The circle `CI` draws about the pen.
    fn circle(&mut self, r: f64) {
        let r = self.user_len(r).abs();
        if !(r.is_finite() && r > 0.0) {
            return;
        }
        let path: BezPath = Circle::new(self.pos, r)
            .path_elements(curve_tol(r))
            .collect();
        if let Some(pm) = &mut self.pm {
            pm.extend(path);
            self.pm_new = true;
            return;
        }
        self.stroke(path);
    }

    // ----- Pages -----

    fn page_end(&mut self) {
        self.flush();
        if self.items.is_empty() {
            return;
        }
        let items = std::mem::take(&mut self.items);
        let mut ext: Option<Rect> = None;
        for it in &items {
            if let Some(b) = item_bounds(&it.shape) {
                ext = Some(ext.map_or(b, |e| e.union(b)));
            }
        }
        self.pages.push(Page {
            items,
            extents: ext,
        });
    }

    /// Restores the defaults (`IN`); `DF` keeps the pen, frame and widths.
    fn defaults(&mut self, full: bool) {
        self.flush();
        self.line = None;
        self.line_prev = None;
        self.fill = Fill {
            kind: 1,
            option: 0.0,
            angle: 0.0,
        };
        self.anchor = Point::ZERO;
        self.pm = None;
        self.relative = false;
        self.scale = None;
        self.si = None;
        self.sr = None;
        self.standard = Font::default();
        self.alternate = Font::default();
        self.use_alternate = false;
        self.dir = None;
        self.slant = 0.0;
        self.origin = 1;
        self.term_printed = None;
        self.symbol = None;
        self.ticks = (0.5, 0.5);
        self.user_lines.clear();
        if full {
            self.down = false;
            self.ro = 0;
            self.widths.clear();
            self.width_relative = false;
            self.colors.clear();
            self.color_range = [0.0, 255.0, 0.0, 255.0, 0.0, 255.0];
            self.pos = Point::ZERO;
            self.default_p1p2();
        } else {
            self.update_scale();
        }
    }

    // ----- Commands -----

    fn command(&mut self, mn: [u8; 2], args: &[u8]) {
        if !known(&mn) {
            self.unknown_total += 1;
            let s = String::from_utf8_lossy(&mn).into_owned();
            if self.unknown.len() < MAX_UNKNOWN && !self.unknown.contains(&s) {
                self.unknown.push(s);
            }
            return;
        }
        self.commands += 1;
        self.count(mn);
        if &mn == b"PE" {
            self.encoded(args);
            return;
        }
        if &mn == b"BP" || &mn == b"CO" {
            if &mn == b"BP" {
                self.hpgl2 = true;
                if self.name.is_none() {
                    self.name = quoted(args);
                }
            }
            return;
        }
        if &mn == b"SM" {
            self.symbol = args.first().copied().filter(|c| c.is_ascii_graphic());
            return;
        }
        let mut n = std::mem::take(&mut self.nums);
        lex::numbers(args, &mut n);
        self.dispatch(mn, &n);
        self.nums = n;
    }

    fn dispatch(&mut self, mn: [u8; 2], n: &[f64]) {
        let arg = |i: usize| n.get(i).copied();
        match &mn {
            b"IN" => {
                self.defaults(true);
                // No pen is selected after IN; files that never select one still plot.
                self.pen = 1;
                self.pens = 8;
            }
            b"DF" => self.defaults(false),
            b"SP" => {
                self.flush();
                self.pen = arg(0).map_or(0, |v| v.clamp(0.0, 1e6) as u32);
            }
            b"PU" | b"PD" => {
                self.down = &mn == b"PD";
                self.moves(n, self.relative);
            }
            b"PA" | b"PR" => {
                self.relative = &mn == b"PR";
                self.moves(n, self.relative);
            }
            b"AA" | b"AR" => {
                if let (Some(x), Some(y), Some(s)) = (arg(0), arg(1), arg(2)) {
                    let c = if &mn == b"AR" {
                        self.pos + self.user_delta(x, y)
                    } else {
                        self.user(x, y)
                    };
                    self.arc(c, s);
                }
            }
            b"AT" | b"RT" => {
                if let (Some(a), Some(b), Some(c), Some(d)) = (arg(0), arg(1), arg(2), arg(3)) {
                    let (m, e) = if &mn == b"RT" {
                        (
                            self.pos + self.user_delta(a, b),
                            self.pos + self.user_delta(c, d),
                        )
                    } else {
                        (self.user(a, b), self.user(c, d))
                    };
                    self.arc3(m, e);
                }
            }
            b"BZ" | b"BR" => {
                for g in n.chunks_exact(6) {
                    let pts: Vec<Point> = if &mn == b"BR" {
                        let o = self.pos;
                        (0..3)
                            .map(|k| o + self.user_delta(g[2 * k], g[2 * k + 1]))
                            .collect()
                    } else {
                        (0..3).map(|k| self.user(g[2 * k], g[2 * k + 1])).collect()
                    };
                    if pts.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
                        self.trace(&[PathEl::CurveTo(pts[0], pts[1], pts[2])], pts[2]);
                    }
                }
            }
            b"CI" => {
                if let Some(r) = arg(0) {
                    self.circle(r);
                }
            }
            b"EA" | b"ER" | b"RA" | b"RR" => {
                if let (Some(x), Some(y)) = (arg(0), arg(1)) {
                    let corner = if mn[1] == b'R' {
                        self.pos + self.user_delta(x, y)
                    } else {
                        self.user(x, y)
                    };
                    let p = self.rect(corner);
                    // The shape is left in the polygon buffer (EP, FP redo it).
                    self.poly = p.clone();
                    if mn[0] == b'E' {
                        self.stroke(p);
                    } else {
                        self.fill_path(p, true);
                    }
                }
            }
            b"EW" | b"WG" => {
                if let (Some(r), Some(s), Some(w)) = (arg(0), arg(1), arg(2)) {
                    let p = self.wedge(r, s, w);
                    self.poly = p.clone();
                    if &mn == b"EW" {
                        self.stroke(p);
                    } else {
                        self.fill_path(p, true);
                    }
                }
            }
            b"PM" => match arg(0).map_or(0, |v| v as i32) {
                0 => {
                    self.flush();
                    let mut p = BezPath::new();
                    p.move_to(self.pos);
                    self.pm = Some(p);
                    self.pm_new = false;
                }
                1 => {
                    if let Some(pm) = &mut self.pm {
                        pm.close_path();
                        self.pm_new = true;
                    }
                }
                2 => {
                    if let Some(mut pm) = self.pm.take() {
                        pm.close_path();
                        self.poly = pm;
                    }
                }
                _ => {}
            },
            b"EP" => {
                let p = self.poly.clone();
                self.stroke(p);
            }
            b"FP" => {
                let p = self.poly.clone();
                let nonzero = arg(0).is_some_and(|v| v == 1.0);
                self.fill_path(p, !nonzero);
            }
            b"FT" => {
                let kind = arg(0).map_or(1, |v| v as i32);
                let mut f = Fill {
                    kind,
                    option: 0.0,
                    angle: 0.0,
                };
                if let Some(o) = arg(1) {
                    f.option = o;
                }
                if let Some(a) = arg(2) {
                    f.angle = a;
                }
                if !matches!(kind, 1..=4 | 10 | 11 | 21 | 22) {
                    f.kind = 1;
                }
                if matches!(kind, 11 | 21 | 22) {
                    self.warn("raster and PCL fill patterns are drawn solid");
                }
                self.fill = f;
            }
            b"AC" => {
                self.anchor = match (arg(0), arg(1)) {
                    (Some(x), Some(y)) => self.user(x, y),
                    _ => Point::ZERO,
                };
            }
            b"LT" => {
                self.flush();
                self.line_type(n);
            }
            b"UL" => {
                self.flush();
                if let Some(i) = arg(0) {
                    let i = i as i32;
                    let gaps: Vec<f64> = n.iter().skip(1).copied().take(20).collect();
                    if gaps.is_empty() {
                        self.user_lines.remove(&i);
                    } else {
                        self.user_lines.insert(i, gaps);
                    }
                }
            }
            b"PW" => {
                self.flush();
                self.widths_seen = true;
                let w = arg(0).unwrap_or(if self.width_relative { 0.1 } else { 0.35 });
                let w = if w.is_finite() { w.max(0.0) } else { 0.35 };
                match arg(1) {
                    Some(p) => {
                        self.widths.insert(p.clamp(0.0, 1e6) as u32, w);
                    }
                    None => {
                        self.widths.clear();
                        self.width_default = w;
                    }
                }
            }
            b"WU" => {
                self.flush();
                self.width_relative = arg(0).is_some_and(|v| v == 1.0);
                self.widths.clear();
                self.width_default = if self.width_relative { 0.1 } else { 0.35 };
            }
            b"NP" => {
                self.pens = arg(0).map_or(8, |v| v.clamp(2.0, 32768.0) as u32);
                self.colors.clear();
            }
            b"CR" => {
                if n.len() >= 6 {
                    self.color_range.copy_from_slice(&n[..6]);
                } else {
                    self.color_range = [0.0, 255.0, 0.0, 255.0, 0.0, 255.0];
                }
            }
            b"PC" => {
                self.flush();
                match (arg(0), arg(1), arg(2), arg(3)) {
                    (Some(p), Some(r), Some(g), Some(b)) => {
                        let cr = self.color_range;
                        let ch = |v: f64, lo: f64, hi: f64| -> u8 {
                            let t = if hi != lo { (v - lo) / (hi - lo) } else { 0.0 };
                            (t.clamp(0.0, 1.0) * 255.0).round() as u8
                        };
                        let c = (
                            ch(r, cr[0], cr[1]),
                            ch(g, cr[2], cr[3]),
                            ch(b, cr[4], cr[5]),
                        );
                        let color = match c {
                            (0, 0, 0) => Color::Foreground,
                            (255, 255, 255) => Color::Background,
                            (r, g, b) => Color::Rgb(r, g, b),
                        };
                        let pen = p.clamp(0.0, 1e6) as u32;
                        self.colors.insert(pen, color);
                        if let Some(&l) = self.pen_layer.get(&pen) {
                            // The layer shows the pen's last colour.
                            self.layers[l as usize].color = color;
                        }
                    }
                    (Some(p), None, ..) => {
                        self.colors.remove(&(p.clamp(0.0, 1e6) as u32));
                    }
                    (None, ..) => self.colors.clear(),
                    _ => {}
                }
            }
            b"IP" => {
                self.flush();
                match (arg(0), arg(1)) {
                    (Some(x), Some(y)) => {
                        let p1 = Point::new(x, y);
                        let p2 = match (arg(2), arg(3)) {
                            (Some(a), Some(b)) => Point::new(a, b),
                            _ => p1 + (self.p2 - self.p1),
                        };
                        if p1 != p2 {
                            self.p1 = p1;
                            self.p2 = p2;
                            self.ip_set = true;
                        }
                        self.update_scale();
                    }
                    _ => self.default_p1p2(),
                }
            }
            b"IR" => {
                self.flush();
                let size = if self.ro % 180 != 0 {
                    Vec2::new(self.paper.y, self.paper.x)
                } else {
                    self.paper
                };
                match (arg(0), arg(1)) {
                    (Some(x), Some(y)) => {
                        let p1 = Point::new(x / 100.0 * size.x, y / 100.0 * size.y);
                        let p2 = match (arg(2), arg(3)) {
                            (Some(a), Some(b)) => {
                                Point::new(a / 100.0 * size.x, b / 100.0 * size.y)
                            }
                            _ => p1 + (self.p2 - self.p1),
                        };
                        if p1 != p2 {
                            self.p1 = p1;
                            self.p2 = p2;
                        }
                        self.update_scale();
                    }
                    _ => self.default_p1p2(),
                }
            }
            b"SC" => {
                self.flush();
                self.scale = match (arg(0), arg(1), arg(2), arg(3)) {
                    (Some(a), Some(b), Some(c), Some(d)) => match arg(4).map_or(0, |t| t as i32) {
                        1 => Some(Scale::Iso {
                            x0: a,
                            x1: b,
                            y0: c,
                            y1: d,
                            left: arg(5).unwrap_or(50.0).clamp(0.0, 100.0),
                            bottom: arg(6).unwrap_or(50.0).clamp(0.0, 100.0),
                        }),
                        2 => Some(Scale::Factor {
                            x0: a,
                            fx: b,
                            y0: c,
                            fy: d,
                        }),
                        _ => Some(Scale::Aniso {
                            x0: a,
                            x1: b,
                            y0: c,
                            y1: d,
                        }),
                    },
                    _ => None,
                };
                self.update_scale();
            }
            b"RO" => {
                self.flush();
                let r = arg(0).map_or(0, |v| v as i32).rem_euclid(360);
                let r = (r + 45) / 90 * 90 % 360;
                if r != self.ro {
                    // The picture turns; the pen keeps its place on the paper.
                    let old = self.out();
                    self.ro = r;
                    let p = self.out().inverse() * (old * self.pos);
                    let ip = self.ip_set;
                    self.default_p1p2();
                    self.ip_set = ip;
                    self.pos = p;
                }
            }
            b"PS" => {
                self.flush();
                if let Some(l) = arg(0)
                    && l > 0.0
                    && l.is_finite()
                {
                    let w = arg(1).filter(|w| *w > 0.0 && w.is_finite()).unwrap_or(l);
                    self.set_paper(Some(Vec2::new(l, w)));
                }
            }
            b"PG" | b"FR" | b"AF" | b"AH" => {
                self.page_end();
                self.down = false;
            }
            b"SI" => {
                self.si = match (arg(0), arg(1)) {
                    (Some(w), Some(h)) => Some((w, h)),
                    _ => None,
                };
                self.sr = None;
            }
            b"SR" => {
                self.sr = match (arg(0), arg(1)) {
                    (Some(w), Some(h)) => Some((w, h)),
                    _ => Some((0.75, 1.5)),
                };
                self.si = None;
            }
            b"SD" | b"AD" => {
                let mut f = Font::default();
                for kv in n.chunks_exact(2) {
                    match kv[0] as i32 {
                        2 => f.fixed = kv[1] == 0.0,
                        3 => f.pitch = kv[1],
                        4 => f.height = kv[1],
                        5 => f.italic = kv[1] != 0.0,
                        6 => f.bold = kv[1] > 0.0,
                        _ => {}
                    }
                }
                if !(f.pitch.is_finite() && f.pitch > 0.0) {
                    f.pitch = 9.0;
                }
                if !(f.height.is_finite() && f.height > 0.0) {
                    f.height = 11.5;
                }
                if &mn == b"SD" {
                    self.standard = f;
                    // SD sets the size: SI/SR give way.
                    self.si = None;
                    self.sr = None;
                } else {
                    self.alternate = f;
                }
            }
            b"SS" => self.use_alternate = false,
            b"SA" => self.use_alternate = true,
            b"DI" | b"DR" => {
                self.dir = match (arg(0), arg(1)) {
                    (Some(run), Some(rise)) if run != 0.0 || rise != 0.0 => {
                        Some((run, rise, &mn == b"DR"))
                    }
                    _ => None,
                };
            }
            b"SL" => {
                self.slant = arg(0).map_or(0.0, |t| t.clamp(-50.0, 50.0).atan().to_degrees());
            }
            b"LO" => {
                let o = arg(0).map_or(1, |v| v as i32);
                self.origin = if matches!(o, 1..=9 | 11..=19 | 21) {
                    o
                } else {
                    1
                };
            }
            b"CP" => {
                let (adv, line) = self.cell();
                let d = self.direction();
                let along = Vec2::new(d.cos(), d.sin());
                let down = Vec2::new(d.sin(), -d.cos());
                match (arg(0), arg(1)) {
                    (Some(s), l) => {
                        let lf = down * (-l.unwrap_or(0.0) * line);
                        self.pos += along * (s * adv) + lf;
                        self.cr_point += lf;
                    }
                    _ => {
                        // Carriage return and line feed.
                        let cr = self.cr_point;
                        self.pos = cr + down * ((self.pos - cr).dot(down) + line);
                        self.cr_point = self.pos;
                    }
                }
            }
            b"TL" => {
                self.ticks = (arg(0).unwrap_or(0.5), arg(1).unwrap_or(0.0));
            }
            b"XT" | b"YT" => {
                let d = self.p2 - self.p1;
                let (tp, tn) = self.ticks;
                let (a, b) = if &mn == b"XT" {
                    (
                        self.pos + Vec2::new(0.0, d.y.abs() * tp / 100.0),
                        self.pos - Vec2::new(0.0, d.y.abs() * tn / 100.0),
                    )
                } else {
                    (
                        self.pos + Vec2::new(d.x.abs() * tp / 100.0, 0.0),
                        self.pos - Vec2::new(d.x.abs() * tn / 100.0, 0.0),
                    )
                };
                let mut p = BezPath::new();
                p.move_to(a);
                p.line_to(b);
                self.stroke(p);
            }
            _ => {}
        }
    }

    fn warn(&mut self, w: &str) {
        if !self.warnings.iter().any(|x| x == w) {
            self.warnings.push(w.to_owned());
        }
    }

    fn line_type(&mut self, n: &[f64]) {
        let Some(&id) = n.first() else {
            if self.line.is_some() {
                self.line_prev = self.line.take();
            }
            return;
        };
        let id = id as i32;
        if id == 99 {
            if self.line.is_none() {
                self.line = self.line_prev.take();
            }
            return;
        }
        let pattern: Vec<f64> = match self.user_lines.get(&id.abs()) {
            Some(g) => g
                .iter()
                .enumerate()
                .map(|(i, &v)| if i % 2 == 0 { v.abs() } else { -v.abs() })
                .collect(),
            None => match id.abs() {
                0 | 1 => vec![0.0, -100.0],
                2 => vec![50.0, -50.0],
                3 => vec![70.0, -30.0],
                4 => vec![80.0, -10.0, 0.0, -10.0],
                5 => vec![70.0, -10.0, 10.0, -10.0],
                6 => vec![50.0, -10.0, 10.0, -10.0, 10.0, -10.0],
                7 => vec![70.0, -10.0, 0.0, -10.0, 0.0, -10.0],
                8 => vec![50.0, -10.0, 0.0, -10.0, 10.0, -10.0, 0.0, -10.0],
                _ => {
                    self.line = None;
                    return;
                }
            },
        };
        let sum: f64 = pattern.iter().map(|v| v.abs()).sum();
        if !(sum.is_finite() && sum > 0.0) {
            self.line = None;
            return;
        }
        let pattern: Vec<f64> = pattern.iter().map(|v| v / sum).collect();
        let relative = !self.hpgl2 || n.get(2).is_none_or(|&m| m == 0.0);
        let len = n.get(1).copied().unwrap_or(4.0);
        let length = if relative {
            len / 100.0 * self.diagonal()
        } else {
            len * PLU_PER_MM
        };
        if !(length.is_finite() && length > 0.0) {
            return;
        }
        if let Some(l) = self.line.take() {
            self.line_prev = Some(l);
        }
        self.line = Some(LineType {
            id,
            pattern,
            length,
        });
    }

    // ----- Polyline encoded -----

    /// `PE`: coordinates packed into base-64 (or base-32) digits.
    fn encoded(&mut self, b: &[u8]) {
        let mut i = 0;
        let mut seven = false;
        let mut frac = 0u32;
        let mut up_next = false;
        let mut abs_next = false;
        let read = |i: &mut usize, seven: bool| -> Option<i64> {
            let mut v: i64 = 0;
            let mut shift = 0u32;
            while *i < b.len() {
                let c = b[*i];
                *i += 1;
                let (digit, last) = if seven {
                    match c {
                        63..=94 => (c - 63, false),
                        95..=126 => (c - 95, true),
                        _ => continue,
                    }
                } else {
                    match c {
                        63..=126 => (c - 63, false),
                        191..=254 => (c - 191, true),
                        _ => continue,
                    }
                };
                if shift < 62 {
                    v |= (digit as i64) << shift;
                }
                shift += if seven { 5 } else { 6 };
                if last {
                    let mag = v >> 1;
                    return Some(if v & 1 == 1 { -mag } else { mag });
                }
            }
            None
        };
        while i < b.len() && !self.stop {
            match b[i] {
                b'7' => {
                    seven = true;
                    i += 1;
                }
                b':' => {
                    i += 1;
                    if let Some(p) = read(&mut i, seven) {
                        self.flush();
                        self.pen = p.clamp(0, 1_000_000) as u32;
                    }
                }
                b'>' => {
                    i += 1;
                    if let Some(f) = read(&mut i, seven) {
                        frac = f.clamp(0, 26) as u32;
                    }
                }
                b'<' => {
                    up_next = true;
                    i += 1;
                }
                b'=' => {
                    abs_next = true;
                    i += 1;
                }
                c if c >= 63 => {
                    let Some(x) = read(&mut i, seven) else {
                        break;
                    };
                    let Some(y) = read(&mut i, seven) else {
                        break;
                    };
                    let f = (1u64 << frac) as f64;
                    let (x, y) = (x as f64 / f, y as f64 / f);
                    let to = if abs_next {
                        self.user(x, y)
                    } else {
                        self.pos + self.user_delta(x, y)
                    };
                    self.down = !up_next;
                    self.go(to);
                    up_next = false;
                    abs_next = false;
                }
                _ => i += 1,
            }
        }
    }

    // ----- Labels -----

    /// The cap height and character body width, plu.
    fn char_size(&self) -> (f64, f64) {
        if let Some((w, h)) = self.si {
            return (w * 10.0 * PLU_PER_MM, h * 10.0 * PLU_PER_MM);
        }
        let d = self.p2 - self.p1;
        if let Some((w, h)) = self.sr {
            return (w / 100.0 * d.x.abs(), h / 100.0 * d.y.abs());
        }
        if self.hpgl2 {
            let f = if self.use_alternate {
                self.alternate
            } else {
                self.standard
            };
            let cap = f.height * 25.4 / 72.0 * 0.7 * PLU_PER_MM;
            if !f.fixed {
                // Proportional: the font's own widths.
                return (cap * SI_DEFAULT.0 / SI_DEFAULT.1, cap);
            }
            let adv = 25.4 / f.pitch * PLU_PER_MM;
            return (adv / 1.5, cap);
        }
        (
            SI_DEFAULT.0 * 10.0 * PLU_PER_MM,
            SI_DEFAULT.1 * 10.0 * PLU_PER_MM,
        )
    }

    /// Character advance and line advance, plu.
    fn cell(&self) -> (f64, f64) {
        let (w, h) = self.char_size();
        (1.5 * w, 2.0 * h)
    }

    /// Label direction in the plu frame, radians.
    fn direction(&self) -> f64 {
        match self.dir {
            None => 0.0,
            Some((run, rise, false)) => rise.atan2(run),
            Some((run, rise, true)) => {
                let d = self.p2 - self.p1;
                (rise * d.y / 100.0).atan2(run * d.x / 100.0)
            }
        }
    }

    fn label(&mut self, raw: &[u8]) {
        let mut text: Vec<u8> = raw.to_vec();
        if let Some(t) = self.term_printed {
            text.push(t);
        }
        self.flush();
        if let Some(pm) = &self.pm
            && !pm.elements().is_empty()
        {
            // Labels are not part of a polygon.
            self.pm_new = true;
        }
        let (adv, line) = self.cell();
        let d = self.direction();
        let along = Vec2::new(d.cos(), d.sin());
        let down = Vec2::new(d.sin(), -d.cos());
        let start = self.pos;
        let mut cr = self.cr_point;
        let mut seg = String::new();
        let mut seg_origin = self.pos;
        let mut n = 0usize;
        let left = matches!(self.origin, 1..=3 | 11..=13 | 21);
        for &c in &text {
            match c {
                b'\r' | b'\n' | 0x08 => {
                    self.text_line(&seg, seg_origin);
                    let p = if left {
                        seg_origin + along * (n as f64 * adv)
                    } else {
                        seg_origin
                    };
                    seg.clear();
                    n = 0;
                    self.pos = match c {
                        // Back to the carriage-return point, on this line.
                        b'\r' => cr + down * (p - cr).dot(down),
                        b'\n' => {
                            cr += down * line;
                            p + down * line
                        }
                        _ => p - along * adv,
                    };
                    seg_origin = self.pos;
                }
                0..=0x1f | 0x7f => {}
                _ => {
                    seg.push(char::from(c));
                    n += 1;
                }
            }
        }
        self.text_line(&seg, seg_origin);
        self.pos = if left {
            seg_origin + along * (n as f64 * adv)
        } else {
            start
        };
        self.cr_point = cr;
    }

    /// One line of a label at `origin` (plu).
    fn text_line(&mut self, s: &str, origin: Point) {
        if s.is_empty() || !self.drawing() || self.pm.is_some() {
            return;
        }
        let (w, h) = self.char_size();
        if !(h.is_finite() && h.abs() > 1e-6 && w.is_finite()) {
            return;
        }
        let o = self.origin;
        let base = if o >= 11 { o - 10 } else { o };
        let base = if base == 11 { 1 } else { base };
        let halign = match base {
            1..=3 => HAlign::Left,
            4..=6 => HAlign::Center,
            _ => HAlign::Right,
        };
        let valign = match base % 3 {
            1 => VAlign::Baseline,
            2 => VAlign::Middle,
            _ => VAlign::Top,
        };
        let mut shift = Vec2::ZERO;
        if (11..=19).contains(&o) {
            let (adv, _) = self.cell();
            shift.x = match halign {
                HAlign::Left => adv / 2.0,
                HAlign::Right => -adv / 2.0,
                _ => 0.0,
            };
            shift.y = match valign {
                VAlign::Baseline => h / 2.0,
                VAlign::Top => -h / 2.0,
                _ => 0.0,
            };
        }
        let d = self.direction();
        let (sx, sy) = (
            if w < 0.0 { -1.0 } else { 1.0 },
            if h < 0.0 { -1.0 } else { 1.0 },
        );
        let transform = self.out()
            * Affine::translate(origin.to_vec2())
            * Affine::rotate(d)
            * Affine::translate(shift)
            * Affine::scale_non_uniform(sx * PLU_PER_MM, sy * PLU_PER_MM);
        let height = h.abs() / PLU_PER_MM;
        let mut span = Span::new(s, 0, height);
        span.width_factor = ((w.abs() / h.abs()) / (SI_DEFAULT.0 / SI_DEFAULT.1)).clamp(0.25, 4.0);
        span.oblique = self.slant;
        if self.hpgl2 && self.si.is_none() && self.sr.is_none() {
            let f = if self.use_alternate {
                self.alternate
            } else {
                self.standard
            };
            span.italic = f.italic;
            span.bold = f.bold;
        }
        let block = TextBlock {
            transform,
            halign,
            valign,
            wrap: 0.0,
            fit: None,
            line_spacing: 1.0,
            paragraphs: vec![Paragraph {
                spans: vec![span],
                align: None,
            }],
        };
        self.emit(Shape::Text(Box::new(block)), None);
    }

    /// The `SM` symbol at the pen.
    fn symbol_at(&mut self, c: u8) {
        let saved = (self.origin, self.pos);
        self.origin = 5;
        self.text_line(&char::from(c).to_string(), self.pos);
        (self.origin, self.pos) = saved;
    }

    // ----- Result -----

    fn finish(mut self) -> Output {
        // Layers in pen order.
        let mut order: Vec<(u32, u32)> = self.pen_layer.iter().map(|(&p, &l)| (p, l)).collect();
        order.sort();
        let mut remap = vec![0u32; self.layers.len()];
        let mut layers = Vec::with_capacity(self.layers.len());
        for (new, &(_, old)) in order.iter().enumerate() {
            remap[old as usize] = new as u32;
            layers.push(self.layers[old as usize].clone());
        }
        for p in &mut self.pages {
            for it in &mut p.items {
                it.layer = remap[it.layer as usize];
                it.outer = it.layer;
            }
        }
        let mut counts: Vec<(String, u64)> = self
            .counts
            .iter()
            .map(|(k, &v)| (String::from_utf8_lossy(k).into_owned(), v))
            .collect();
        counts.sort();
        let mut warnings = self.warnings;
        if !self.unknown.is_empty() {
            warnings.push(format!(
                "unknown commands skipped: {}",
                self.unknown.join(", ")
            ));
        }
        let paper_mm = self.paper_stated.map(|s| {
            let s = s / PLU_PER_MM;
            (s.x, s.y)
        });
        Output {
            pages: self.pages,
            layers,
            linetypes: self.linetypes,
            counts,
            warnings,
            hpgl2: self.hpgl2,
            pcl: self.pcl,
            pens: order.iter().map(|o| o.0).collect(),
            paper_mm,
            name: self.name,
            widths_seen: self.widths_seen,
            unknown: self.unknown_total,
            commands: self.commands,
        }
    }
}

/// The pen colours of the HP-GL/2 default palette, then more for larger carousels.
pub(crate) fn default_color(pen: u32) -> Color {
    const MORE: [(u8, u8, u8); 8] = [
        (255, 128, 0),
        (128, 0, 255),
        (128, 64, 0),
        (128, 128, 128),
        (0, 128, 0),
        (0, 0, 128),
        (128, 0, 0),
        (0, 128, 128),
    ];
    let p = if pen >= 16 { (pen - 1) % 15 + 1 } else { pen };
    match p {
        0 => Color::Background,
        1 => Color::Foreground,
        2 => Color::Rgb(255, 0, 0),
        3 => Color::Rgb(0, 255, 0),
        4 => Color::Rgb(255, 255, 0),
        5 => Color::Rgb(0, 0, 255),
        6 => Color::Rgb(255, 0, 255),
        7 => Color::Rgb(0, 255, 255),
        _ => {
            let (r, g, b) = MORE[(p as usize - 8) % MORE.len()];
            Color::Rgb(r, g, b)
        }
    }
}

/// Flattening tolerance of a curve of radius `r` (plu): fine, but bounded in pieces for
/// absurd radii.
fn curve_tol(r: f64) -> f64 {
    (r.abs() * 1e-6).max(0.1)
}

/// A shading level of the pen colour on white paper.
fn shade(pen: Color, level: f64) -> Color {
    let [r, g, b] = pen.on(false, [255, 255, 255]);
    let mix = |c: u8| (255.0 - (255.0 - c as f64) * level).round() as u8;
    Color::Rgb(mix(r), mix(g), mix(b))
}

/// The first quoted string of a parameter list.
fn quoted(args: &[u8]) -> Option<String> {
    let s = args.iter().position(|&c| c == b'"')? + 1;
    let e = args[s..]
        .iter()
        .position(|&c| c == b'"')
        .map_or(args.len(), |p| s + p);
    let t: String = args[s..e].iter().map(|&c| char::from(c)).collect();
    let t = t.trim().to_owned();
    (!t.is_empty()).then_some(t)
}
