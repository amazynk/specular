//! The interpreter: metafile state, attributes and primitives, turned into a display list per
//! picture ([`Picture`]).

use std::collections::{BTreeMap, HashMap};
use std::f64::consts::TAU;
use std::sync::Arc;

use kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Shape, Stroke, Vec2};
use vello_cpu::PixelMetadata;
use vello_cpu::Pixmap;
use vello_cpu::peniko::ImageAlphaType;

use crate::args::{Args, CellColour, Prec, RealPrec};
use crate::list::{HAlign, Op, Paint, Picture, Rgba, TextOp, TextRun, VAlign};

/// Most display-list entries per document.
pub const MAX_OPS: usize = 1 << 21;
/// Most points per document (all primitives together).
pub const MAX_POINTS: usize = 1 << 25;
/// Most cell array and pattern pixels per document.
pub const MAX_CELLS: u64 = 1 << 26;
/// Cells above which a cell array's list must hold at least 1/64 of them.
const SPARSE_CELLS: usize = 1 << 20;
/// Most pictures per document.
pub(crate) const MAX_PICTURES: usize = 10_000;
/// Most characters of one text element (with its appended runs).
const MAX_TEXT: usize = 1 << 16;
/// Largest colour index kept in the colour table.
const MAX_COLOUR_INDEX: u32 = 1 << 16;
/// Longest page side, points (200 inches); larger pictures are scaled down to it.
pub(crate) const MAX_PAGE: f64 = 14_400.0;
/// Shortest page side, points.
pub(crate) const MIN_PAGE: f64 = 1.0;
/// Page long side of an abstract (unscaled) picture: 11 inches.
pub(crate) const ABSTRACT_LONG_SIDE: f64 = 792.0;

/// A colour as an attribute holds it: resolved when a primitive is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Col {
    Index(u32),
    Rgb(Rgba),
}

/// The default colour table beyond the background (0) and foreground (1).
const DEFAULT_TABLE: [Rgba; 8] = [
    [255, 255, 255, 255],
    [0, 0, 0, 255],
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 0, 255],
    [0, 255, 255, 255],
    [255, 0, 255, 255],
];

#[derive(Clone, Debug)]
struct PatternDef {
    nx: usize,
    ny: usize,
    cells: Vec<CellColour>,
}

/// The state a picture starts from (the metafile defaults) and changes.
#[derive(Clone, Debug)]
struct State {
    prec: Prec,
    colour_direct: bool,
    colour_min: [f64; 4],
    colour_max: [f64; 4],
    /// 0 absolute, 1 scaled, 2 fractional, 3 mm.
    line_mode: u8,
    marker_mode: u8,
    edge_mode: u8,
    extent: Option<[f64; 4]>,
    /// Millimetres per VDC unit (metric scaling).
    metric: Option<f64>,
    background: Rgba,
    table: Vec<Rgba>,
    line_type: i64,
    line_width: Option<f64>,
    line_colour: Col,
    line_cap: i64,
    line_join: i64,
    marker_type: i64,
    marker_size: Option<f64>,
    marker_colour: Col,
    font: i64,
    expansion: f64,
    spacing: f64,
    text_colour: Col,
    char_height: Option<f64>,
    up: (f64, f64),
    base: (f64, f64),
    text_path: i64,
    halign: HAlign,
    valign: VAlign,
    interior: i64,
    fill_colour: Col,
    hatch: i64,
    pattern: i64,
    edge_type: i64,
    edge_width: Option<f64>,
    edge_colour: Col,
    edge_visible: bool,
    edge_cap: i64,
    edge_join: i64,
    fill_ref: Option<(f64, f64)>,
    pattern_size: Option<[f64; 4]>,
    patterns: HashMap<i64, PatternDef>,
    /// Negative line types: (repeat length, relative dash elements).
    line_types: HashMap<i64, (f64, Vec<f64>)>,
    transparency: bool,
    aux: Col,
    clip_rect: Option<[f64; 4]>,
    clip_on: bool,
    transparent_cell: Option<Col>,
}

impl Default for State {
    fn default() -> Self {
        State {
            prec: Prec::default(),
            colour_direct: false,
            colour_min: [0.0; 4],
            colour_max: [255.0; 4],
            line_mode: 1,
            marker_mode: 1,
            edge_mode: 1,
            extent: None,
            metric: None,
            background: [255, 255, 255, 255],
            table: DEFAULT_TABLE.to_vec(),
            line_type: 1,
            line_width: None,
            line_colour: Col::Index(1),
            line_cap: 1,
            line_join: 1,
            marker_type: 3,
            marker_size: None,
            marker_colour: Col::Index(1),
            font: 1,
            expansion: 1.0,
            spacing: 0.0,
            text_colour: Col::Index(1),
            char_height: None,
            up: (0.0, 1.0),
            base: (1.0, 0.0),
            text_path: 0,
            halign: HAlign::Normal,
            valign: VAlign::Normal,
            interior: 0,
            fill_colour: Col::Index(1),
            hatch: 1,
            pattern: 1,
            edge_type: 1,
            edge_width: None,
            edge_colour: Col::Index(1),
            edge_visible: false,
            edge_cap: 1,
            edge_join: 1,
            fill_ref: None,
            pattern_size: None,
            patterns: HashMap::new(),
            line_types: HashMap::new(),
            transparency: true,
            aux: Col::Index(0),
            clip_rect: None,
            clip_on: true,
            transparent_cell: None,
        }
    }
}

impl State {
    fn extent(&self) -> [f64; 4] {
        self.extent.unwrap_or(if self.prec.vdc_real {
            [0.0, 0.0, 1.0, 1.0]
        } else {
            [0.0, 0.0, 32767.0, 32767.0]
        })
    }

    fn resolve(&self, c: Col) -> Rgba {
        match c {
            Col::Rgb(c) => c,
            Col::Index(i) => self.table.get(i as usize).copied().unwrap_or(
                // Undefined indexes draw in the foreground colour.
                self.table.get(1).copied().unwrap_or([0, 0, 0, 255]),
            ),
        }
    }

    /// A direct colour's components scaled by the colour value extent.
    fn direct(&self, c: [f64; 4]) -> Rgba {
        let n = |i: usize| {
            let (lo, hi) = (self.colour_min[i], self.colour_max[i]);
            if hi == lo {
                0.0
            } else {
                ((c[i] - lo) / (hi - lo)).clamp(0.0, 1.0)
            }
        };
        let b = |v: f64| (v * 255.0).round() as u8;
        if self.prec.components == 4 {
            // CMYK, naively.
            let k = n(3);
            let f = |v: f64| b((1.0 - v) * (1.0 - k));
            [f(n(0)), f(n(1)), f(n(2)), 255]
        } else {
            [b(n(0)), b(n(1)), b(n(2)), 255]
        }
    }

    fn colour(&self, a: &mut dyn Args) -> Option<Col> {
        if self.colour_direct {
            Some(Col::Rgb(self.direct(a.cd()?)))
        } else {
            Some(Col::Index(a.ci()?))
        }
    }

    fn cell(&self, c: CellColour) -> Rgba {
        match c {
            CellColour::Index(i) => self.resolve(Col::Index(i)),
            CellColour::Direct(d) => self.direct(d),
        }
    }
}

/// Document-level facts gathered while parsing.
#[derive(Clone, Debug, Default)]
pub(crate) struct Meta {
    pub version: Option<i64>,
    pub description: Option<String>,
    pub fonts: Vec<String>,
    pub vdc_real: bool,
    pub direct_colour: bool,
    pub indexed_colour: bool,
    pub elements: u64,
    /// Elements that draw something but are not drawn, by name.
    pub skipped: BTreeMap<&'static str, u64>,
    pub truncated: bool,
    pub limited: bool,
    pub scaled_down: bool,
    pub metric: bool,
}

/// A picture under construction.
struct Builder {
    name: String,
    to_page: Affine,
    /// Page length per VDC length (geometric mean of the axes).
    len_scale: f64,
    size: (f64, f64),
    /// Points per millimetre on this page (less than 72/25.4 when scaled down).
    pt_per_mm: f64,
    nominal: f64,
    ops: Vec<Op>,
    /// Effective clip last emitted.
    clip: Option<Option<Rect>>,
    /// A TEXT element waiting for its APPEND TEXT runs.
    pending: Option<TextOp>,
    /// Colour index 0 at BEGIN PICTURE BODY: the background (later changes to entry 0 do
    /// not repaint it).
    background: Rgba,
}

pub(crate) struct Interp {
    st: State,
    defaults: State,
    pub meta: Meta,
    pic: Option<Builder>,
    /// Picture name between BEGIN PICTURE and BEGIN PICTURE BODY.
    pic_name: Option<String>,
    pub pictures: Vec<Picture>,
    ops: usize,
    points: usize,
    cells: u64,
    /// Inside a METAFILE DEFAULTS REPLACEMENT.
    in_mdr: bool,
    pub ended: bool,
    pub began: bool,
    /// Character height of the pending text's first run.
    first_height: f64,
    /// Inside BEGIN FIGURE: the boundary collected so far (VDC) and whether its last
    /// subpath is still open (line primitives continue it).
    figure: Option<(BezPath, bool)>,
}

/// Bytes as text: ISO 8859-1 (CGM's default G0/G1 sets), control characters dropped.
pub(crate) fn latin1(b: &[u8]) -> String {
    b.iter()
        .map(|&c| char::from(c))
        .filter(|c| !c.is_control() || *c == '\t')
        .collect()
}

fn pt(p: (f64, f64)) -> Point {
    Point::new(p.0, p.1)
}

impl Interp {
    pub fn new() -> Self {
        Interp {
            st: State::default(),
            defaults: State::default(),
            meta: Meta::default(),
            pic: None,
            pic_name: None,
            pictures: Vec::new(),
            ops: 0,
            points: 0,
            cells: 0,
            in_mdr: false,
            ended: false,
            began: false,
            first_height: 0.0,
            figure: None,
        }
    }

    pub fn prec(&self) -> &Prec {
        &self.st.prec
    }

    fn skip(&mut self, name: &'static str) {
        *self.meta.skipped.entry(name).or_insert(0) += 1;
    }

    fn push(&mut self, op: Op) {
        if self.ops >= MAX_OPS {
            self.meta.limited = true;
            return;
        }
        if let Some(b) = &mut self.pic {
            self.ops += 1;
            b.ops.push(op);
        }
    }

    /// Reads up to the remaining points of a list, within the document's point budget.
    fn points(&mut self, a: &mut dyn Args) -> Vec<(f64, f64)> {
        let mut v = Vec::new();
        while a.more() {
            if self.points >= MAX_POINTS {
                self.meta.limited = true;
                break;
            }
            match a.point() {
                Some(p) if p.0.is_finite() && p.1.is_finite() => {
                    self.points += 1;
                    v.push(p);
                }
                Some(_) => {}
                None => break,
            }
        }
        v
    }

    /// One element. `class`/`id` per ISO/IEC 8632-1 (the binary encoding's numbering).
    pub fn element(&mut self, class: u8, id: u8, a: &mut dyn Args) {
        self.meta.elements += 1;
        if self.ended {
            return;
        }
        match class {
            0 => self.delimiter(id, a),
            1 => self.descriptor(id, a),
            2 => self.picture_descriptor(id, a),
            3 => self.control(id, a),
            4 => {
                if self.pic.is_some() {
                    self.primitive(id, a);
                }
            }
            5 => self.attribute(id, a),
            // Escape, external, segments, application structures: nothing to draw.
            6..=9 => {}
            _ => self.skip("unknown element"),
        }
        if self.pic.is_none() && self.pic_name.is_none() || self.in_mdr {
            // Outside pictures, everything sets the defaults pictures start from.
            self.defaults = self.st.clone();
        }
    }

    fn delimiter(&mut self, id: u8, a: &mut dyn Args) {
        match id {
            1 => self.began = true,
            2 => {
                self.end_picture();
                self.ended = true;
            }
            3 => {
                self.end_picture();
                self.st = self.defaults.clone();
                let name = a.string().map(|s| latin1(&s)).unwrap_or_default();
                self.pic_name = Some(name);
            }
            4 => self.begin_body(),
            // BEGIN FIGURE: line and area primitives build one boundary until END FIGURE,
            // which fills it by the interior style (gaps closed by straight lines).
            8 => {
                if self.pic.is_some() {
                    self.figure = Some((BezPath::new(), false));
                }
            }
            9 => {
                if let Some((mut path, open)) = self.figure.take() {
                    if open {
                        path.close_path();
                    }
                    if !path.elements().is_empty() {
                        self.fill_area(path, None);
                    }
                }
            }
            5 => {
                self.end_picture();
                // Attributes set in a picture do not carry over to the next.
                self.st = self.defaults.clone();
            }
            // Tile arrays (WebCGM 2.0 compressed tiles) are not drawn yet.
            19 => self.skip("tile array"),
            _ => {}
        }
    }

    fn descriptor(&mut self, id: u8, a: &mut dyn Args) {
        let st = &mut self.st;
        match id {
            1 => self.meta.version = a.int(),
            2 => self.meta.description = a.string().map(|s| latin1(&s)),
            3 => {
                st.prec.vdc_real = a.enumv(&["INTEGER", "REAL"]) == Some(1);
                self.meta.vdc_real = st.prec.vdc_real;
            }
            4 => {
                if let Some(v) = a.int().and_then(int_bits) {
                    st.prec.int = v;
                }
            }
            5 => {
                if let (Some(f), Some(x), Some(y)) =
                    (a.enumv(&["FLOATING", "FIXED"]), a.int(), a.int())
                    && let Some(r) = RealPrec::from_parts(f, x, y)
                {
                    st.prec.real = r;
                }
            }
            6 => {
                if let Some(v) = a.int().and_then(int_bits) {
                    st.prec.index = v;
                }
            }
            7 => {
                if let Some(v) = a.int().and_then(int_bits) {
                    st.prec.colour = v;
                }
            }
            8 => {
                if let Some(v) = a.int().and_then(int_bits) {
                    st.prec.colour_index = v;
                }
            }
            10 => {
                // Colour value extent: minimum and maximum direct colours.
                if let (Some(lo), Some(hi)) = (a.cd(), a.cd()) {
                    st.colour_min = lo;
                    st.colour_max = hi;
                }
            }
            12 => {
                // METAFILE DEFAULTS REPLACEMENT: elements inside the parameter list; the
                // binary reader hands them over through `defaults_replacement`.
            }
            13 => {
                while a.more() && self.meta.fonts.len() < 1024 {
                    match a.string() {
                        Some(s) => self.meta.fonts.push(latin1(&s)),
                        None => break,
                    }
                }
            }
            19 => {
                // Colour model: 1 RGB, 2 CIELAB, 3 CIELUV, 4 CMYK, 5 RGB-related.
                if let Some(m) = a.index() {
                    st.prec.components = if m == 4 { 4 } else { 3 };
                    if m == 4 && st.colour_max == [255.0; 4] {
                        st.colour_max = [255.0; 4];
                    }
                }
            }
            _ => {}
        }
    }

    /// Starts or ends a METAFILE DEFAULTS REPLACEMENT.
    pub fn set_mdr(&mut self, on: bool) {
        self.in_mdr = on;
    }

    fn picture_descriptor(&mut self, id: u8, a: &mut dyn Args) {
        let st = &mut self.st;
        let mode = |a: &mut dyn Args| {
            a.enumv(&["ABSTRACT", "SCALED", "FRACTIONAL", "MM"])
                .map(|m| m.clamp(0, 3) as u8)
        };
        match id {
            1 => {
                let m = a.enumv(&["ABSTRACT", "METRIC"]);
                let f = a.real();
                st.metric = match (m, f) {
                    (Some(1), Some(f)) if f.is_finite() && f > 0.0 => Some(f),
                    _ => None,
                };
            }
            2 => {
                st.colour_direct = a.enumv(&["INDEXED", "DIRECT"]) == Some(1);
                if st.colour_direct {
                    self.meta.direct_colour = true;
                } else {
                    self.meta.indexed_colour = true;
                }
            }
            3 => {
                if let Some(m) = mode(a) {
                    st.line_mode = m;
                }
            }
            4 => {
                if let Some(m) = mode(a) {
                    st.marker_mode = m;
                }
            }
            5 => {
                if let Some(m) = mode(a) {
                    st.edge_mode = m;
                }
            }
            6 => {
                if let (Some(p), Some(q)) = (a.point(), a.point())
                    && [p.0, p.1, q.0, q.1].iter().all(|v| v.is_finite())
                {
                    st.extent = Some([p.0, p.1, q.0, q.1]);
                }
            }
            7 => {
                if let Some(c) = a.cd() {
                    let c = st.direct(c);
                    st.background = c;
                    if let Some(t) = st.table.first_mut() {
                        *t = c;
                    }
                }
            }
            16 => {
                // LINE AND EDGE TYPE DEFINITION: type, repeat length, dash elements.
                if let (Some(t), Some(len)) = (a.index(), a.vdc()) {
                    let mut dashes = Vec::new();
                    while a.more() && dashes.len() < 64 {
                        match a.int() {
                            Some(d) => dashes.push(d.max(0) as f64),
                            None => break,
                        }
                    }
                    if t < 0 && dashes.iter().sum::<f64>() > 0.0 {
                        st.line_types.insert(t, (len.abs(), dashes));
                    }
                }
            }
            17 => self.skip("hatch style definition"),
            18 => self.skip("geometric pattern definition"),
            _ => {}
        }
    }

    fn control(&mut self, id: u8, a: &mut dyn Args) {
        let st = &mut self.st;
        match id {
            1 => {
                if let Some(v) = a.int().and_then(int_bits) {
                    st.prec.vdc_int = v;
                }
            }
            2 => {
                if let (Some(f), Some(x), Some(y)) =
                    (a.enumv(&["FLOATING", "FIXED"]), a.int(), a.int())
                    && let Some(r) = RealPrec::from_parts(f, x, y)
                {
                    st.prec.vdc_real_prec = r;
                }
            }
            3 => {
                if let Some(c) = st.colour(a) {
                    st.aux = c;
                }
            }
            4 => st.transparency = a.enumv(&["OFF", "ON"]) != Some(0),
            5 => {
                if let (Some(p), Some(q)) = (a.point(), a.point()) {
                    st.clip_rect = Some([p.0, p.1, q.0, q.1]);
                }
            }
            6 => st.clip_on = a.enumv(&["OFF", "ON"]) != Some(0),
            20 => {
                let on = a.enumv(&["OFF", "ON"]) == Some(1);
                let c = st.colour(a);
                st.transparent_cell = if on { c } else { None };
            }
            _ => {}
        }
        if matches!(id, 5 | 6) {
            self.update_clip();
        }
    }

    fn attribute(&mut self, id: u8, a: &mut dyn Args) {
        let st = &mut self.st;
        let size = |a: &mut dyn Args, mode: u8| {
            if mode == 0 { a.vdc() } else { a.real() }
        };
        match id {
            2 => st.line_type = a.index().unwrap_or(1),
            3 => st.line_width = size(a, st.line_mode).filter(|v| v.is_finite()),
            4 => st.line_colour = st.colour(a).unwrap_or(st.line_colour),
            6 => st.marker_type = a.index().unwrap_or(3),
            7 => st.marker_size = size(a, st.marker_mode).filter(|v| v.is_finite()),
            8 => st.marker_colour = st.colour(a).unwrap_or(st.marker_colour),
            10 => st.font = a.index().unwrap_or(1),
            12 => {
                st.expansion = a
                    .real()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(1.0)
            }
            13 => st.spacing = a.real().filter(|v| v.is_finite()).unwrap_or(0.0),
            14 => st.text_colour = st.colour(a).unwrap_or(st.text_colour),
            15 => st.char_height = a.vdc().filter(|v| v.is_finite()),
            16 => {
                if let (Some(ux), Some(uy), Some(bx), Some(by)) =
                    (a.vdc(), a.vdc(), a.vdc(), a.vdc())
                    && [ux, uy, bx, by].iter().all(|v| v.is_finite())
                {
                    st.up = (ux, uy);
                    st.base = (bx, by);
                }
            }
            17 => st.text_path = a.enumv(&["RIGHT", "LEFT", "UP", "DOWN"]).unwrap_or(0),
            18 => {
                let h = a.enumv(&["NORMHORIZ", "LEFT", "CTR", "RIGHT", "CONTHORIZ"]);
                let v = a.enumv(&[
                    "NORMVERT", "TOP", "CAP", "HALF", "BASE", "BOTTOM", "CONTVERT",
                ]);
                let ch = a.real().unwrap_or(0.0);
                let cv = a.real().unwrap_or(0.0);
                st.halign = match h {
                    Some(1) => HAlign::Left,
                    Some(2) => HAlign::Centre,
                    Some(3) => HAlign::Right,
                    Some(4) => HAlign::Continuous(ch),
                    _ => HAlign::Normal,
                };
                st.valign = match v {
                    Some(1) => VAlign::Top,
                    Some(2) => VAlign::Cap,
                    Some(3) => VAlign::Half,
                    Some(4) => VAlign::Base,
                    Some(5) => VAlign::Bottom,
                    Some(6) => VAlign::Continuous(cv),
                    _ => VAlign::Normal,
                };
            }
            22 => {
                st.interior = a
                    .enumv(&[
                        "HOLLOW", "SOLID", "PAT", "HATCH", "EMPTY", "GEOPAT", "INTERP",
                    ])
                    .unwrap_or(0)
            }
            23 => st.fill_colour = st.colour(a).unwrap_or(st.fill_colour),
            24 => st.hatch = a.index().unwrap_or(1),
            25 => st.pattern = a.index().unwrap_or(1),
            27 => st.edge_type = a.index().unwrap_or(1),
            28 => st.edge_width = size(a, st.edge_mode).filter(|v| v.is_finite()),
            29 => st.edge_colour = st.colour(a).unwrap_or(st.edge_colour),
            30 => st.edge_visible = a.enumv(&["OFF", "ON"]) == Some(1),
            31 => st.fill_ref = a.point(),
            32 => self.pattern_table(a),
            33 => {
                if let (Some(a0), Some(a1), Some(a2), Some(a3)) =
                    (a.vdc(), a.vdc(), a.vdc(), a.vdc())
                {
                    st.pattern_size = Some([a0, a1, a2, a3]);
                }
            }
            34 => {
                let Some(start) = a.ci() else { return };
                let mut i = start;
                while a.more() && i < MAX_COLOUR_INDEX {
                    let Some(c) = a.cd() else { break };
                    let c = st.direct(c);
                    let i_us = i as usize;
                    if st.table.len() <= i_us {
                        st.table.resize(i_us + 1, [0, 0, 0, 255]);
                    }
                    st.table[i_us] = c;
                    i += 1;
                }
            }
            37 => {
                st.line_cap = a.index().unwrap_or(1);
            }
            38 => st.line_join = a.index().unwrap_or(1),
            44 => st.edge_cap = a.index().unwrap_or(1),
            45 => st.edge_join = a.index().unwrap_or(1),
            _ => {}
        }
    }

    fn pattern_table(&mut self, a: &mut dyn Args) {
        let (Some(idx), Some(nx), Some(ny), Some(lp)) = (a.index(), a.int(), a.int(), a.int())
        else {
            return;
        };
        let (Ok(nx), Ok(ny)) = (usize::try_from(nx), usize::try_from(ny)) else {
            return;
        };
        if nx == 0 || ny == 0 || nx > 1024 || ny > 1024 {
            return;
        }
        self.cells += (nx * ny) as u64;
        if self.cells > MAX_CELLS {
            self.meta.limited = true;
            return;
        }
        let indexed = !self.st.colour_direct;
        let mut cells = Vec::with_capacity(nx * ny);
        let ok = a.cells(nx, ny, lp, 1, indexed, &mut |c| cells.push(c));
        if ok.is_some() {
            self.st.patterns.insert(idx, PatternDef { nx, ny, cells });
        }
    }

    fn begin_body(&mut self) {
        let name = self.pic_name.take().unwrap_or_default();
        if self.pictures.len() >= MAX_PICTURES {
            self.meta.limited = true;
            return;
        }
        let st = &self.st;
        let [x0, y0, x1, y1] = st.extent();
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (wv, hv) = (
            if dx.abs() > 0.0 { dx.abs() } else { 1.0 },
            if dy.abs() > 0.0 { dy.abs() } else { 1.0 },
        );
        let mut pt_per_mm = 72.0 / 25.4;
        let (mut w, mut h) = match st.metric {
            Some(mm) => {
                self.meta.metric = true;
                (wv * mm * pt_per_mm, hv * mm * pt_per_mm)
            }
            None => {
                let k = ABSTRACT_LONG_SIDE / wv.max(hv);
                (wv * k, hv * k)
            }
        };
        if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
            (w, h) = (ABSTRACT_LONG_SIDE, ABSTRACT_LONG_SIDE);
        }
        let long = w.max(h);
        if long > MAX_PAGE {
            let k = MAX_PAGE / long;
            w *= k;
            h *= k;
            pt_per_mm *= k;
            self.meta.scaled_down = true;
        } else if long < MIN_PAGE * 36.0 && st.metric.is_some() {
            // A tiny metric picture (under half an inch) is shown at a viewable size.
            let k = ABSTRACT_LONG_SIDE / long;
            w *= k;
            h *= k;
            pt_per_mm *= k;
        }
        w = w.max(MIN_PAGE);
        h = h.max(MIN_PAGE);
        let sx = w / if dx != 0.0 { dx } else { 1.0 };
        let sy = -h / if dy != 0.0 { dy } else { 1.0 };
        let to_page = Affine::new([sx, 0.0, 0.0, sy, -x0 * sx, h - y0 * sy]);
        let len_scale = (sx * sy).abs().sqrt();
        // Nominal line: a thousandth of the page's long side.
        let nominal = w.max(h) / 1000.0;
        self.pic = Some(Builder {
            name,
            to_page,
            len_scale,
            size: (w, h),
            pt_per_mm,
            nominal,
            ops: Vec::new(),
            clip: None,
            pending: None,
            background: self.st.table.first().copied().unwrap_or(self.st.background),
        });
        self.update_clip();
    }

    fn end_picture(&mut self) {
        self.flush_text();
        self.figure = None;
        self.pic_name = None;
        if let Some(b) = self.pic.take() {
            self.pictures.push(Picture {
                name: b.name,
                size: b.size,
                background: b.background,
                ops: b.ops,
            });
        }
    }

    /// Emits a clip change when the effective clip differs from the one in force.
    fn update_clip(&mut self) {
        let Some(b) = &self.pic else { return };
        let want = if self.st.clip_on
            && let Some([x0, y0, x1, y1]) = self.st.clip_rect
        {
            let r = Rect::from_points(
                b.to_page * Point::new(x0, y0),
                b.to_page * Point::new(x1, y1),
            );
            let page = Rect::new(0.0, 0.0, b.size.0, b.size.1);
            if r.contains_rect(page) { None } else { Some(r) }
        } else {
            None
        };
        if b.clip != Some(want) {
            self.flush_text();
            if let Some(b) = &mut self.pic {
                b.clip = Some(want);
            }
            self.push(Op::Clip(want));
        }
    }

    // ----- sizes ------------------------------------------------------------------------

    fn width_pt(&self, mode: u8, v: Option<f64>) -> f64 {
        let Some(b) = &self.pic else { return 1.0 };
        let w = match (mode, v) {
            (_, None) => b.nominal,
            (0, Some(v)) => v.abs() * b.len_scale,
            (1, Some(v)) => v.abs() * b.nominal,
            (2, Some(v)) => v.abs() * b.size.0.max(b.size.1),
            (_, Some(v)) => v.abs() * b.pt_per_mm,
        };
        w.min(b.size.0.max(b.size.1))
    }

    fn stroke_style(&self, width: f64, ty: i64, cap: i64, join: i64) -> Stroke {
        let mut s = Stroke::new(width)
            .with_caps(match cap {
                3 => Cap::Round,
                4 => Cap::Square,
                _ => Cap::Butt,
            })
            .with_join(match join {
                3 => Join::Round,
                4 => Join::Bevel,
                _ => Join::Miter,
            });
        let u = width.max(self.pic.as_ref().map_or(1.0, |b| b.nominal * 2.0));
        let dashes: Vec<f64> = match ty {
            2 => vec![6.0 * u, 3.0 * u],
            3 => vec![u, 2.0 * u],
            4 => vec![6.0 * u, 2.0 * u, u, 2.0 * u],
            5 => vec![6.0 * u, 2.0 * u, u, 2.0 * u, u, 2.0 * u],
            t if t < 0 => match (self.st.line_types.get(&t), &self.pic) {
                (Some((len, d)), Some(b)) => {
                    let sum: f64 = d.iter().sum();
                    let repeat = if self.st.line_mode == 0 {
                        len * b.len_scale
                    } else {
                        len * u
                    };
                    d.iter().map(|x| x / sum * repeat).collect()
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        if !dashes.is_empty()
            && dashes.iter().all(|d| d.is_finite())
            && dashes.iter().sum::<f64>() > 0.0
        {
            s = s.with_dashes(0.0, dashes);
        }
        s
    }

    fn page_path(&self, mut p: BezPath) -> BezPath {
        if let Some(b) = &self.pic {
            p.apply_affine(b.to_page);
        }
        p
    }

    fn stroke_line(&mut self, path: BezPath) {
        if let Some((fig, open)) = &mut self.figure {
            // A boundary piece: continues the open subpath (a straight line closes the gap).
            for el in path.elements() {
                match *el {
                    kurbo::PathEl::MoveTo(p) if *open => fig.line_to(p),
                    kurbo::PathEl::MoveTo(p) => {
                        fig.move_to(p);
                        *open = true;
                    }
                    kurbo::PathEl::ClosePath => {}
                    e => fig.push(e),
                }
            }
            return;
        }
        let st = &self.st;
        let w = self.width_pt(st.line_mode, st.line_width);
        let style = self.stroke_style(w, st.line_type, st.line_cap, st.line_join);
        let colour = st.resolve(st.line_colour);
        let path = self.page_path(path);
        let bb = path.bounding_box();
        if bb.width() == 0.0 && bb.height() == 0.0 && bb.x0.is_finite() && bb.y0.is_finite() {
            // A line of zero length: a dot as wide as the line.
            let dot = kurbo::Circle::new(bb.origin(), w / 2.0).to_path(0.05);
            self.push(Op::Fill {
                path: dot,
                even_odd: false,
                paint: Paint::Solid(colour),
            });
            return;
        }
        self.push(Op::Stroke {
            path,
            style,
            colour,
        });
    }

    /// Fills a closed area (VDC) by the interior style, then draws its edge if visible.
    /// `edges`: the outline to stroke (defaults to the area's boundary).
    fn fill_area(&mut self, area: BezPath, edges: Option<BezPath>) {
        if let Some((fig, open)) = &mut self.figure {
            // A closed region of the figure.
            if *open {
                fig.close_path();
                *open = false;
            }
            fig.extend(area.elements().iter().copied());
            return;
        }
        let page = self.page_path(area);
        let st = &self.st;
        let fill = st.resolve(st.fill_colour);
        match st.interior {
            0 => {
                // Hollow: the boundary in the fill colour.
                let w = self.pic.as_ref().map_or(1.0, |b| b.nominal);
                let path = match &edges {
                    Some(e) => self.page_path(e.clone()),
                    None => page.clone(),
                };
                self.push(Op::Stroke {
                    path,
                    style: Stroke::new(w),
                    colour: fill,
                });
            }
            2 => {
                let paint = self.pattern_paint().unwrap_or(Paint::Solid(fill));
                self.push(Op::Fill {
                    path: page.clone(),
                    even_odd: true,
                    paint,
                });
            }
            3 => self.hatch(&page, fill),
            4 => {}
            _ => self.push(Op::Fill {
                path: page.clone(),
                even_odd: true,
                paint: Paint::Solid(fill),
            }),
        }
        if self.st.edge_visible {
            let st = &self.st;
            let w = self.width_pt(st.edge_mode, st.edge_width);
            let style = self.stroke_style(w, st.edge_type, st.edge_cap, st.edge_join);
            let colour = st.resolve(st.edge_colour);
            let path = match edges {
                Some(e) => self.page_path(e),
                None => page,
            };
            self.push(Op::Stroke {
                path,
                style,
                colour,
            });
        }
    }

    fn hatch(&mut self, page: &BezPath, colour: Rgba) {
        if self.pic.is_none() {
            return;
        }
        if !self.st.transparency {
            let aux = self.st.resolve(self.st.aux);
            self.push(Op::Fill {
                path: page.clone(),
                even_odd: true,
                paint: Paint::Solid(aux),
            });
        }
        let Some(b) = &self.pic else { return };
        let dirs: &[(f64, f64)] = match self.st.hatch {
            2 => &[(0.0, 1.0)],
            3 => &[(1.0, 1.0)],
            4 => &[(1.0, -1.0)],
            5 => &[(1.0, 0.0), (0.0, 1.0)],
            6 => &[(1.0, 1.0), (1.0, -1.0)],
            _ => &[(1.0, 0.0)],
        };
        let bb = page.bounding_box();
        if !(bb.width().is_finite() && bb.height().is_finite()) {
            return;
        }
        let spacing = b.size.0.max(b.size.1) / 100.0;
        let width = b.nominal;
        let mut lines = BezPath::new();
        let centre = bb.center();
        let r = 0.5 * (bb.width().hypot(bb.height())) + spacing;
        let n = (2.0 * r / spacing).ceil().min(20_000.0) as i64;
        for &(dx, dy) in dirs {
            // The direction in VDC (y up) seen on the page.
            let d = b.to_page * Point::new(dx, dy) - b.to_page * Point::ZERO;
            let len = d.hypot();
            if len == 0.0 {
                continue;
            }
            let d = d / len;
            let nrm = Vec2::new(-d.y, d.x);
            for i in 0..=n {
                let off = -r + i as f64 * spacing;
                let c = centre + nrm * off;
                lines.move_to(c - d * r);
                lines.line_to(c + d * r);
            }
        }
        self.push(Op::ClippedStroke {
            clip: page.clone(),
            even_odd: true,
            lines,
            width,
            colour,
        });
    }

    fn pattern_paint(&self) -> Option<Paint> {
        let b = self.pic.as_ref()?;
        let p = self.st.patterns.get(&self.st.pattern)?;
        let mut px = Vec::with_capacity(p.nx * p.ny * 4);
        for &c in &p.cells {
            px.extend_from_slice(&self.st.cell(c));
        }
        let pixmap = pixmap(px, p.nx, p.ny)?;
        let [x0, y0, x1, y1] = self.st.extent();
        let side = (x1 - x0).abs().max((y1 - y0).abs()) / 20.0;
        let [hx, hy, wx, wy] = self.st.pattern_size.unwrap_or([0.0, side, side, 0.0]);
        let (rx, ry) = self.st.fill_ref.unwrap_or((x0, y0));
        // Pattern pixel (u, v) (row 0 at the top) → VDC: rows run down the height vector.
        let m = Affine::new([
            wx / p.nx as f64,
            wy / p.nx as f64,
            -hx / p.ny as f64,
            -hy / p.ny as f64,
            rx + hx,
            ry + hy,
        ]);
        if m.determinant() == 0.0 || !m.determinant().is_finite() {
            return None;
        }
        Some(Paint::Pattern {
            pixmap: Arc::new(pixmap),
            transform: b.to_page * m,
        })
    }

    // ----- primitives -------------------------------------------------------------------

    fn primitive(&mut self, id: u8, a: &mut dyn Args) {
        if !matches!(id, 6) {
            self.flush_text();
        }
        match id {
            1 => {
                let p = self.points(a);
                if p.len() >= 2 {
                    self.stroke_line(polyline(&p, false));
                }
            }
            2 => {
                let p = self.points(a);
                let mut path = BezPath::new();
                for s in p.chunks_exact(2) {
                    path.move_to(pt(s[0]));
                    path.line_to(pt(s[1]));
                }
                if !path.elements().is_empty() {
                    self.stroke_line(path);
                }
            }
            3 => {
                let p = self.points(a);
                self.markers(&p);
            }
            4 => {
                let (Some(p), Some(_fin), Some(s)) =
                    (a.point(), a.enumv(&["NOTFINAL", "FINAL"]), a.string())
                else {
                    return;
                };
                self.text(p, &s, None);
            }
            5 => {
                let (Some(dx), Some(dy), Some(p), Some(_fin), Some(s)) = (
                    a.vdc(),
                    a.vdc(),
                    a.point(),
                    a.enumv(&["NOTFINAL", "FINAL"]),
                    a.string(),
                ) else {
                    return;
                };
                self.text(p, &s, Some((dx, dy)));
            }
            6 => {
                let (Some(_fin), Some(s)) = (a.enumv(&["NOTFINAL", "FINAL"]), a.string()) else {
                    return;
                };
                self.append_text(&s);
            }
            7 => {
                let p = self.points(a);
                if p.len() >= 3 {
                    self.fill_area(polyline(&p, true), None);
                }
            }
            8 => self.polygon_set(a),
            9 => self.cell_array(a),
            10 => self.skip("generalized drawing primitive"),
            11 => {
                let (Some(p), Some(q)) = (a.point(), a.point()) else {
                    return;
                };
                self.fill_area(Rect::from_points(pt(p), pt(q)).to_path(0.1), None);
            }
            12 => {
                let (Some(c), Some(r)) = (a.point(), a.vdc()) else {
                    return;
                };
                if r.is_finite() && r != 0.0 {
                    let path = kurbo::Circle::new(pt(c), r.abs()).to_path(r.abs() * 1e-4);
                    self.fill_area(path, None);
                }
            }
            13 | 14 => {
                let (Some(p1), Some(p2), Some(p3)) = (a.point(), a.point(), a.point()) else {
                    return;
                };
                let close = if id == 14 {
                    a.enumv(&["PIE", "CHORD"])
                } else {
                    None
                };
                self.arc3(pt(p1), pt(p2), pt(p3), close);
            }
            15 | 16 | 20 => {
                let (Some(c), Some(sx), Some(sy), Some(ex), Some(ey), Some(r)) =
                    (a.point(), a.vdc(), a.vdc(), a.vdc(), a.vdc(), a.vdc())
                else {
                    return;
                };
                let close = if id == 16 {
                    a.enumv(&["PIE", "CHORD"])
                } else {
                    None
                };
                let unit = Affine::new([r, 0.0, 0.0, r, c.0, c.1]);
                let a0 = sy.atan2(sx);
                let a1 = ey.atan2(ex);
                self.conic_arc(unit, a0, a1, id != 20, close);
            }
            17 => {
                let (Some(c), Some(p1), Some(p2)) = (a.point(), a.point(), a.point()) else {
                    return;
                };
                let m = conjugate(c, p1, p2);
                if m.determinant().abs() > 0.0 {
                    let path = m * kurbo::Circle::new(Point::ZERO, 1.0).to_path(1e-4);
                    self.fill_area(path, None);
                }
            }
            18 | 19 => {
                let (Some(c), Some(p1), Some(p2)) = (a.point(), a.point(), a.point()) else {
                    return;
                };
                let (Some(sx), Some(sy), Some(ex), Some(ey)) = (a.vdc(), a.vdc(), a.vdc(), a.vdc())
                else {
                    return;
                };
                let close = if id == 19 {
                    a.enumv(&["PIE", "CHORD"])
                } else {
                    None
                };
                let m = conjugate(c, p1, p2);
                let det = m.determinant();
                if det == 0.0 || !det.is_finite() {
                    return;
                }
                // The start and end vectors in the unit circle's space.
                let inv = Affine::new([
                    m.as_coeffs()[0],
                    m.as_coeffs()[1],
                    m.as_coeffs()[2],
                    m.as_coeffs()[3],
                    0.0,
                    0.0,
                ])
                .inverse();
                let s = inv * Point::new(sx, sy);
                let e = inv * Point::new(ex, ey);
                self.conic_arc(m, s.y.atan2(s.x), e.y.atan2(e.x), true, close);
            }
            26 => self.polybezier(a),
            // CONNECTING EDGE: closes the figure's current region with a straight edge; the
            // next primitive starts a new region.
            21 => {
                if let Some((fig, open)) = &mut self.figure {
                    if *open {
                        fig.close_path();
                        *open = false;
                    }
                } else {
                    self.skip("connecting edge");
                }
            }
            22 => self.skip("hyperbolic arc"),
            23 => self.skip("parabolic arc"),
            24 => self.skip("non-uniform B-spline"),
            25 => self.skip("NURB"),
            27 => self.skip("polysymbol"),
            28 => self.skip("bitonal tile"),
            29 => self.skip("tile"),
            _ => self.skip("unknown element"),
        }
    }

    fn markers(&mut self, p: &[(f64, f64)]) {
        let Some(b) = &self.pic else { return };
        let st = &self.st;
        let size = match (st.marker_mode, st.marker_size) {
            (_, None) => b.size.0.max(b.size.1) / 100.0,
            (0, Some(v)) => v.abs() * b.len_scale,
            (1, Some(v)) => v.abs() * b.size.0.max(b.size.1) / 100.0,
            (2, Some(v)) => v.abs() * b.size.0.max(b.size.1),
            (_, Some(v)) => v.abs() * b.pt_per_mm,
        }
        .min(b.size.0.max(b.size.1));
        let colour = st.resolve(st.marker_colour);
        let h = size / 2.0;
        let mut strokes = BezPath::new();
        let mut dots = BezPath::new();
        for &q in p {
            let c = b.to_page * pt(q);
            let line = |path: &mut BezPath, x0: f64, y0: f64, x1: f64, y1: f64| {
                path.move_to((c.x + x0, c.y + y0));
                path.line_to((c.x + x1, c.y + y1));
            };
            match st.marker_type {
                1 => dots
                    .extend(kurbo::Circle::new(c, (size / 8.0).max(b.nominal)).path_elements(0.01)),
                2 => {
                    line(&mut strokes, -h, 0.0, h, 0.0);
                    line(&mut strokes, 0.0, -h, 0.0, h);
                }
                4 => strokes.extend(kurbo::Circle::new(c, h).path_elements(0.01)),
                5 => {
                    line(&mut strokes, -h, -h, h, h);
                    line(&mut strokes, -h, h, h, -h);
                }
                _ => {
                    line(&mut strokes, -h, 0.0, h, 0.0);
                    line(&mut strokes, 0.0, -h, 0.0, h);
                    let d = h * std::f64::consts::FRAC_1_SQRT_2;
                    line(&mut strokes, -d, -d, d, d);
                    line(&mut strokes, -d, d, d, -d);
                }
            }
        }
        let w = b.nominal;
        if !dots.elements().is_empty() {
            self.push(Op::Fill {
                path: dots,
                even_odd: false,
                paint: Paint::Solid(colour),
            });
        }
        if !strokes.elements().is_empty() {
            self.push(Op::Stroke {
                path: strokes,
                style: Stroke::new(w),
                colour,
            });
        }
    }

    fn polygon_set(&mut self, a: &mut dyn Args) {
        let mut area = BezPath::new();
        let mut edges = BezPath::new();
        let mut start: Option<Point> = None;
        let mut prev: Option<(Point, i64)> = None;
        while a.more() {
            if self.points >= MAX_POINTS {
                self.meta.limited = true;
                break;
            }
            let (Some(p), Some(flag)) = (
                a.point(),
                a.enumv(&["INVIS", "VIS", "CLOSEINVIS", "CLOSEVIS"]),
            ) else {
                break;
            };
            self.points += 1;
            let p = pt(p);
            match start {
                None => {
                    area.move_to(p);
                    start = Some(p);
                }
                Some(_) => area.line_to(p),
            }
            if let Some((q, f)) = prev
                && f % 2 == 1
            {
                edges.move_to(q);
                edges.line_to(p);
            }
            prev = Some((p, flag));
            if flag >= 2 {
                // Closes this polygon back to its first point.
                let s = start.take().unwrap_or(p);
                area.close_path();
                if flag == 3 {
                    edges.move_to(p);
                    edges.line_to(s);
                }
                prev = None;
            }
        }
        if let (Some(s), Some((p, f))) = (start, prev) {
            area.close_path();
            if f % 2 == 1 {
                edges.move_to(p);
                edges.line_to(s);
            }
        }
        if area.elements().len() >= 3 {
            self.fill_area(area, Some(edges));
        }
    }

    fn cell_array(&mut self, a: &mut dyn Args) {
        let (Some(p), Some(q), Some(r)) = (a.point(), a.point(), a.point()) else {
            return;
        };
        let (Some(nx), Some(ny), Some(lp)) = (a.int(), a.int(), a.int()) else {
            return;
        };
        let mode = a.cell_mode();
        let (Ok(nx), Ok(ny)) = (u16::try_from(nx), u16::try_from(ny)) else {
            self.skip("cell array larger than 65535 cells a side");
            return;
        };
        if nx == 0 || ny == 0 {
            return;
        }
        let (nx, ny) = (usize::from(nx), usize::from(ny));
        let n = (nx * ny) as u64;
        if self.cells + n > MAX_CELLS {
            self.meta.limited = true;
            return;
        }
        self.cells += n;
        let indexed = !self.st.colour_direct;
        let transparent = self.st.transparent_cell.map(|c| self.st.resolve(c));
        let mut px: Vec<u8> = Vec::new();
        if px.try_reserve_exact(nx * ny * 4).is_err() {
            self.meta.limited = true;
            return;
        }
        let st = &self.st;
        let filled = a.cells(nx, ny, lp, mode, indexed, &mut |c| {
            let c = st.cell(c);
            if Some(c) == transparent {
                px.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                px.extend_from_slice(&c);
            }
        });
        if filled.is_none() {
            // A large array whose list holds almost no cells is a header claiming a size the
            // file does not hold: not drawn, rather than allocated transparent.
            if nx * ny > SPARSE_CELLS && px.len() / 4 * 64 < nx * ny {
                self.meta.limited = true;
                return;
            }
            // A short list: the rest of the cells stay transparent.
            px.resize(nx * ny * 4, 0);
        }
        let Some(pm) = pixmap(px, nx, ny) else { return };
        let Some(b) = &self.pic else { return };
        // First row along P→R, rows from R toward Q.
        let (p, q, r) = (pt(p), pt(q), pt(r));
        let col = (r - p) / nx as f64;
        let row = (q - r) / ny as f64;
        let m = Affine::new([col.x, col.y, row.x, row.y, p.x, p.y]);
        if m.determinant() == 0.0 || !m.determinant().is_finite() {
            return;
        }
        let transform = b.to_page * m;
        self.push(Op::Image {
            pixmap: Arc::new(pm),
            transform,
        });
    }

    fn polybezier(&mut self, a: &mut dyn Args) {
        let ind = a.index().unwrap_or(1);
        let p = self.points(a);
        let mut path = BezPath::new();
        if ind == 2 {
            if p.len() >= 4 {
                path.move_to(pt(p[0]));
                for c in p[1..].chunks_exact(3) {
                    path.curve_to(pt(c[0]), pt(c[1]), pt(c[2]));
                }
            }
        } else {
            for c in p.chunks_exact(4) {
                path.move_to(pt(c[0]));
                path.curve_to(pt(c[1]), pt(c[2]), pt(c[3]));
            }
        }
        if !path.elements().is_empty() {
            self.stroke_line(path);
        }
    }

    /// An arc of the unit circle mapped by `m`, from angle `a0` to `a1`, counter-clockwise
    /// (in the unit circle's space) or clockwise, open or closed (0 pie, 1 chord).
    fn conic_arc(&mut self, m: Affine, a0: f64, a1: f64, ccw: bool, close: Option<i64>) {
        if !(a0.is_finite() && a1.is_finite()) || m.determinant() == 0.0 {
            return;
        }
        let mut sweep = (a1 - a0).rem_euclid(TAU);
        if sweep == 0.0 {
            sweep = TAU;
        }
        if !ccw {
            sweep -= TAU;
            if sweep == 0.0 {
                sweep = -TAU;
            }
        }
        let arc = kurbo::Arc::new(Point::ZERO, (1.0, 1.0), a0, sweep, 0.0);
        let mut path = BezPath::new();
        path.move_to(Point::new(a0.cos(), a0.sin()));
        arc.to_cubic_beziers(1e-5, |p1, p2, p3| path.curve_to(p1, p2, p3));
        let path = m * path;
        match close {
            None => self.stroke_line(path),
            Some(kind) => {
                let mut path = path;
                if kind == 0 {
                    path.line_to(m * Point::ZERO);
                }
                path.close_path();
                self.fill_area(path, None);
            }
        }
    }

    fn arc3(&mut self, p1: Point, p2: Point, p3: Point, close: Option<i64>) {
        let Some(c) = circumcentre(p1, p2, p3) else {
            // Collinear: a straight line.
            let path = polyline(&[(p1.x, p1.y), (p3.x, p3.y)], false);
            match close {
                None => self.stroke_line(path),
                Some(_) => self.fill_area(path, None),
            }
            return;
        };
        let r = (p1 - c).hypot();
        let ang = |p: Point| (p.y - c.y).atan2(p.x - c.x);
        let (a1, a2, a3) = (ang(p1), ang(p2), ang(p3));
        // Counter-clockwise when p2 is met going counter-clockwise from p1 to p3.
        let ccw = (a2 - a1).rem_euclid(TAU) < (a3 - a1).rem_euclid(TAU);
        let m = Affine::new([r, 0.0, 0.0, r, c.x, c.y]);
        self.conic_arc(m, a1, a3, ccw, close);
    }

    // ----- text -------------------------------------------------------------------------

    fn text(&mut self, p: (f64, f64), s: &[u8], restrict: Option<(f64, f64)>) {
        let Some(b) = &self.pic else { return };
        let st = &self.st;
        let [x0, y0, x1, y1] = st.extent();
        let h = st
            .char_height
            .unwrap_or(0.01 * (x1 - x0).abs().max((y1 - y0).abs()))
            .abs();
        if h == 0.0 || !h.is_finite() {
            return;
        }
        let up = Vec2::new(st.up.0, st.up.1);
        let base = Vec2::new(st.base.0, st.base.1);
        let (ul, bl) = (up.hypot(), base.hypot());
        let (up, base, ratio) = if ul > 0.0 && bl > 0.0 && up.cross(base).abs() > 1e-9 * ul * bl {
            (up / ul, base / bl, bl / ul)
        } else {
            (Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0), 1.0)
        };
        // Text space (units of the character height) → VDC.
        let xa = base * (h * ratio);
        let ya = up * h;
        let m = Affine::new([xa.x, xa.y, ya.x, ya.y, p.0, p.1]);
        let transform = b.to_page * m;
        let restrict = restrict.map(|(dx, dy)| (dx.abs() / h, dy.abs() / h));
        let op = TextOp {
            transform,
            halign: st.halign,
            valign: st.valign,
            path: st.text_path.clamp(0, 3) as u8,
            restrict,
            runs: Vec::new(),
        };
        if let Some(b) = &mut self.pic {
            b.pending = Some(op);
        }
        self.text_run(s, h);
        self.first_height = h;
    }

    fn append_text(&mut self, s: &[u8]) {
        if self.pic.as_ref().is_some_and(|b| b.pending.is_some()) {
            let h = self.st.char_height.unwrap_or(self.first_height).abs();
            self.text_run(s, h);
        }
    }

    fn text_run(&mut self, s: &[u8], h: f64) {
        let first = if self.first_height > 0.0 {
            self.first_height
        } else {
            h
        };
        let st = &self.st;
        let run = TextRun {
            font: usize::try_from(st.font.max(1) - 1).unwrap_or(0),
            text: latin1(s).chars().collect(),
            colour: st.resolve(st.text_colour),
            height: if first > 0.0 { h / first } else { 1.0 },
            expansion: st.expansion,
            spacing: st.spacing,
        };
        if let Some(b) = &mut self.pic
            && let Some(t) = &mut b.pending
        {
            let used: usize = t.runs.iter().map(|r| r.text.len()).sum();
            if used + run.text.len() <= MAX_TEXT {
                t.runs.push(run);
            }
        }
    }

    fn flush_text(&mut self) {
        if let Some(b) = &mut self.pic
            && let Some(t) = b.pending.take()
        {
            self.first_height = 0.0;
            if t.runs.iter().any(|r| !r.text.is_empty()) {
                self.push(Op::Text(Box::new(t)));
            }
        }
    }
}

/// Maps the unit circle onto the ellipse with centre `c` and conjugate diameter end points
/// `p1`, `p2`.
fn conjugate(c: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> Affine {
    Affine::new([p1.0 - c.0, p1.1 - c.1, p2.0 - c.0, p2.1 - c.1, c.0, c.1])
}

fn circumcentre(a: Point, b: Point, c: Point) -> Option<Point> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let scale = (b - a).hypot().max((c - a).hypot());
    if d.abs() <= 1e-12 * scale * scale || !d.is_finite() {
        return None;
    }
    let (a2, b2, c2) = (
        a.to_vec2().hypot2(),
        b.to_vec2().hypot2(),
        c.to_vec2().hypot2(),
    );
    let x = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let y = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let p = Point::new(x, y);
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

fn polyline(p: &[(f64, f64)], close: bool) -> BezPath {
    let mut path = BezPath::new();
    for (i, &q) in p.iter().enumerate() {
        if i == 0 {
            path.move_to(pt(q));
        } else {
            path.line_to(pt(q));
        }
    }
    if close {
        path.close_path();
    }
    path
}

/// The bits of an integer precision: 8, 16, 24 or 32.
fn int_bits(v: i64) -> Option<u8> {
    match v {
        8 | 16 | 24 | 32 => Some(v as u8),
        // Colour precisions of 1, 2 and 4 bits exist for packed cells.
        1 | 2 | 4 => Some(v as u8),
        _ => None,
    }
}

/// An opaque-or-not RGBA8 (straight) buffer as a vello pixmap.
pub(crate) fn pixmap(px: Vec<u8>, w: usize, h: usize) -> Option<Pixmap> {
    let (w16, h16) = (u16::try_from(w).ok()?, u16::try_from(h).ok()?);
    if px.len() != w * h * 4 {
        return None;
    }
    let transparent = px.chunks_exact(4).any(|p| p[3] != 255);
    Some(Pixmap::from_parts(
        px,
        w16,
        h16,
        PixelMetadata {
            may_have_transparency: transparent,
            alpha_type: ImageAlphaType::Alpha,
        },
    ))
}
