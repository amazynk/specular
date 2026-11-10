//! Gerber RS-274X (the 2024 specification, with X2 and X3 attributes and the deprecated
//! commands older files use) into outlines in millimetres, y up.
//!
//! Every graphical object becomes a closed outline (flashes, draws swept along their path,
//! regions) or, for a round aperture under 10 µm, a hairline; dark and clear polarity stay
//! in draw order. Step-and-repeat and block apertures are expanded (capped by
//! [`Limits::copies`]).

mod macros;

use std::collections::HashMap;
use std::f64::consts::PI;

use specular_vector::kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};

use crate::geom;
use crate::{Error, Limits};
use macros::Macro;

/// Round apertures narrower than this (mm) draw hairlines.
pub const HAIRLINE_MM: f64 = 0.01;

/// What made an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// D03 with an aperture.
    Flash,
    /// D01 with an aperture: the aperture swept along a line or arc.
    Draw,
    /// A G36/G37 region.
    Region,
    /// A draw with a zero-size aperture: `path` is the centre line, drawn one pixel wide.
    Hairline,
}

/// The centre line of a draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Line(Point, Point),
    /// Radius and angles in radians; `sweep` is positive counter-clockwise.
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        sweep: f64,
    },
}

impl Segment {
    pub fn start(&self) -> Point {
        match *self {
            Segment::Line(a, _) => a,
            Segment::Arc {
                center,
                radius,
                start,
                ..
            } => center + geom::unit(start) * radius,
        }
    }

    pub fn end(&self) -> Point {
        match *self {
            Segment::Line(_, b) => b,
            Segment::Arc {
                center,
                radius,
                start,
                sweep,
            } => center + geom::unit(start + sweep) * radius,
        }
    }

    pub(crate) fn transformed(&self, a: Affine) -> Segment {
        match *self {
            Segment::Line(p, q) => Segment::Line(a * p, a * q),
            Segment::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                // Similarity transforms only (Gerber's): the start point fixes the angle.
                let c = a * center;
                let s = a * (center + geom::unit(start) * radius);
                let k = a.determinant().abs().sqrt();
                let d = s - c;
                let flip = if a.determinant() < 0.0 { -1.0 } else { 1.0 };
                Segment::Arc {
                    center: c,
                    radius: radius * k,
                    start: d.y.atan2(d.x),
                    sweep: sweep * flip,
                }
            }
        }
    }
}

/// One graphical object.
#[derive(Clone, Debug)]
pub struct Object {
    /// Closed outlines filled non-zero (holes run the other way), or a hairline's centre
    /// line.
    pub path: BezPath,
    /// Dark polarity; clear objects erase what the layer drew before them.
    pub dark: bool,
    pub kind: ObjectKind,
    /// The centre line of a draw (board outlines are built from these).
    pub segment: Option<Segment>,
    /// The flash had a hole or the macro had an exposure-off part (keep it separate when
    /// merging objects).
    pub holes: bool,
}

impl Object {
    pub(crate) fn transformed(&self, a: Affine) -> Object {
        Object {
            path: geom::transformed(&self.path, a),
            dark: self.dark,
            kind: self.kind,
            segment: self.segment.map(|s| s.transformed(a)),
            holes: self.holes,
        }
    }
}

/// A Gerber file.
#[derive(Clone, Debug, Default)]
pub struct Gerber {
    /// In draw order.
    pub objects: Vec<Object>,
    /// File attributes (`%TF`, and `G04 #@! TF` comments): name without the dot, values.
    pub attributes: Vec<(String, Vec<String>)>,
    /// Object and aperture attributes were seen (`TO`, `TA`).
    pub x2: bool,
    /// Component attributes (`TO.C`) were seen: an X3 file.
    pub x3: bool,
    /// The file's units were inches.
    pub inch: bool,
    pub apertures: u32,
    pub macros: u32,
    /// Bounds of every object, millimetres.
    pub bounds: Option<Rect>,
    /// Deprecated commands the file uses, once each ("image polarity (IP)").
    pub deprecated: Vec<String>,
    pub warnings: Vec<String>,
}

impl Gerber {
    /// The values of file attribute `name` ("FileFunction").
    pub fn attribute(&self, name: &str) -> Option<&[String]> {
        self.attributes
            .iter()
            .rev()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_slice())
    }

    /// `.FileFunction` as written, values joined by commas.
    pub fn file_function(&self) -> Option<String> {
        self.attribute("FileFunction").map(|v| v.join(","))
    }

    /// `.GenerationSoftware`: vendor, application and version.
    pub fn generator(&self) -> Option<String> {
        let v = self.attribute("GenerationSoftware")?;
        let s = v
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!s.is_empty()).then_some(s)
    }

    /// `.ProjectId`'s project name.
    pub fn project(&self) -> Option<String> {
        let v = self.attribute("ProjectId")?;
        v.first()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// `.ProjectId`'s revision.
    pub fn revision(&self) -> Option<String> {
        let v = self.attribute("ProjectId")?;
        v.get(2)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// `.CreationDate`.
    pub fn date(&self) -> Option<String> {
        self.attribute("CreationDate")
            .and_then(|v| v.first())
            .cloned()
    }

    /// "RS-274X", "RS-274X X2" or "RS-274X X3".
    pub fn version(&self) -> String {
        if self.x3 {
            "RS-274X X3".into()
        } else if self.x2 || !self.attributes.is_empty() {
            "RS-274X X2".into()
        } else {
            "RS-274X".into()
        }
    }
}

/// Whether `head` (the start of a file) is a Gerber file: text with `*`-terminated
/// statements and a Gerber command (`%FS`, `%MO`, `%AD`, `%TF`, or G04/D-code words).
pub fn is_gerber(head: &[u8]) -> bool {
    let head = &head[..head.len().min(16 * 1024)];
    if head.is_empty() || head.contains(&0) {
        return false;
    }
    let text = String::from_utf8_lossy(head);
    let stars = text.matches('*').count();
    if stars < 2 {
        return false;
    }
    let t = text.trim_start_matches(|c: char| c.is_whitespace() || c == '*');
    let ext = ["%FS", "%MO", "%ADD", "%TF.", "%TA.", "%AM", "%IP", "%LP"];
    let first = t.as_bytes().first().copied().unwrap_or(0);
    if ext.iter().any(|e| text.contains(e)) && matches!(first, b'%' | b'G' | b'D' | b'M' | b'X') {
        return true;
    }
    // RS-274-D: G04 comments and D-code words, nothing else.
    t.starts_with("G04")
        && stars >= 4
        && (text.contains("D01*") || text.contains("D02*") || text.contains("D03*"))
}

/// Coordinate format (`%FS`).
#[derive(Clone, Copy, Debug)]
struct Format {
    dec: u32,
    /// Trailing zeros omitted (deprecated): digits count from the left.
    trailing: bool,
    int: u32,
    incremental: bool,
}

#[derive(Clone, Debug)]
enum Aperture {
    Circle {
        d: f64,
        base: BezPath,
        holes: bool,
    },
    /// Rectangle, obround or polygon: its outline at the origin.
    Shape {
        base: BezPath,
        holes: bool,
    },
    Macro {
        base: BezPath,
        holes: bool,
    },
    Block(Vec<Object>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Interp {
    Linear,
    Cw,
    Ccw,
}

struct StepRepeat {
    nx: u32,
    ny: u32,
    dx: f64,
    dy: f64,
    objects: Vec<Object>,
}

struct Parser<'l> {
    limits: &'l Limits,
    fmt: Option<Format>,
    /// Millimetres per file unit, once known.
    unit: Option<f64>,
    interp: Interp,
    multi: Option<bool>,
    cur: Point,
    aperture: Option<u32>,
    last_op: u32,
    dark: bool,
    mirror: (bool, bool),
    rotation: f64,
    scale: f64,
    region: Option<Region>,
    apertures: HashMap<u32, Aperture>,
    macros: HashMap<String, Macro>,
    blocks: Vec<(u32, Vec<Object>)>,
    sr: Option<StepRepeat>,
    copies: usize,
    full: bool,
    image: ImageTransform,
    negative: bool,
    ended: bool,
    used_apertures: bool,
    out: Gerber,
}

#[derive(Default)]
struct ImageTransform {
    swap: bool,
    mirror: (bool, bool),
    scale: (f64, f64),
    offset: (f64, f64),
    /// Image offset (IO), in the units in force when it was read: moves the finished image.
    image_offset: (f64, f64),
    rotate: f64,
    set: bool,
}

/// A region being built: finished contours and the current one.
#[derive(Default)]
struct Region {
    done: BezPath,
    cur: Option<(BezPath, usize)>,
}

impl Region {
    fn close(&mut self) {
        if let Some((mut p, segs)) = self.cur.take()
            && segs > 0
        {
            p.close_path();
            geom::push(&mut self.done, p, false);
        }
    }
}

/// Reads a Gerber file.
pub fn parse(bytes: &[u8], limits: &Limits) -> Result<Gerber, Error> {
    if bytes.contains(&0) {
        return Err(Error::NotThisFormat);
    }
    let mut p = Parser {
        limits,
        fmt: None,
        unit: None,
        interp: Interp::Linear,
        multi: None,
        cur: Point::ORIGIN,
        aperture: None,
        last_op: 2,
        dark: true,
        mirror: (false, false),
        rotation: 0.0,
        scale: 1.0,
        region: None,
        apertures: HashMap::new(),
        macros: HashMap::new(),
        blocks: Vec::new(),
        sr: None,
        copies: 0,
        full: false,
        image: ImageTransform {
            scale: (1.0, 1.0),
            ..ImageTransform::default()
        },
        negative: false,
        ended: false,
        used_apertures: false,
        out: Gerber::default(),
    };
    let text = String::from_utf8_lossy(bytes);
    let b = text.as_bytes();
    let mut i = 0;
    let mut statements = 0usize;
    while i < b.len() && !p.ended {
        let c = b[i];
        if c == b'%' {
            let j = b[i + 1..]
                .iter()
                .position(|&c| c == b'%')
                .map_or(b.len(), |k| i + 1 + k);
            p.extended(&text[i + 1..j]);
            statements += 1;
            i = j + 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else {
            // A comment runs to its `*` even through a `%` ("G04 #@! %TF…*").
            let comment = b[i..].starts_with(b"G04");
            let j = b[i..]
                .iter()
                .position(|&c| c == b'*' || (c == b'%' && !comment))
                .map_or(b.len(), |k| i + k);
            p.word(&text[i..j]);
            statements += 1;
            i = if j < b.len() && b[j] == b'*' {
                j + 1
            } else {
                j
            };
        }
    }
    if statements == 0 {
        return Err(Error::NotThisFormat);
    }
    if p.used_apertures && p.out.apertures == 0 {
        return Err(Error::Unsupported(
            "This is an RS-274-D Gerber file: it has no aperture definitions of its own and \
             needs the aperture list that came with it. Only RS-274X (extended) Gerber files \
             can be shown."
                .into(),
        ));
    }
    if !p.ended {
        p.warn("The file ends without M02; it may be cut short.");
    }
    if p.region.is_some() {
        p.warn("A region is not closed (G37 missing).");
        p.end_region();
    }
    p.finish();
    Ok(p.out)
}

fn num(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

impl Parser<'_> {
    fn warn(&mut self, w: impl Into<String>) {
        crate::warn(&mut self.out.warnings, w);
    }

    fn deprecated(&mut self, what: &str) {
        if !self.out.deprecated.iter().any(|d| d == what) && self.out.deprecated.len() < 64 {
            self.out.deprecated.push(what.to_string());
        }
    }

    fn unit(&mut self) -> f64 {
        match self.unit {
            Some(u) => u,
            None => {
                self.warn("The file does not state its units; inches assumed.");
                self.unit = Some(25.4);
                self.out.inch = true;
                25.4
            }
        }
    }

    fn set_unit(&mut self, mm: bool) {
        self.unit = Some(if mm { 1.0 } else { 25.4 });
        self.out.inch = !mm;
    }

    /// A coordinate value in file units → mm.
    fn coord(&mut self, s: &str) -> Option<f64> {
        let u = self.unit();
        if s.contains('.') {
            return num(s).map(|v| v * u);
        }
        let fmt = match self.fmt {
            Some(f) => f,
            None => {
                self.warn("The file has no coordinate format (%FS); 2.4 assumed.");
                let f = Format {
                    dec: 4,
                    trailing: false,
                    int: 2,
                    incremental: false,
                };
                self.fmt = Some(f);
                f
            }
        };
        let (neg, digits) = match s.as_bytes().first() {
            Some(b'-') => (true, &s[1..]),
            Some(b'+') => (false, &s[1..]),
            _ => (false, s),
        };
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let v: f64 = if fmt.trailing {
            let width = (fmt.int + fmt.dec) as usize;
            let mut d = digits.to_string();
            while d.len() < width {
                d.push('0');
            }
            d.parse::<f64>().ok()? / 10f64.powi(fmt.dec as i32)
        } else {
            digits.parse::<f64>().ok()? / 10f64.powi(fmt.dec as i32)
        };
        Some(if neg { -v } else { v } * u)
    }

    fn extended(&mut self, block: &str) {
        let stmts: Vec<&str> = block.split('*').collect();
        let mut k = 0;
        while k < stmts.len() {
            let s = stmts[k].trim();
            if s.is_empty() {
                k += 1;
                continue;
            }
            if let Some(name) = s.strip_prefix("AM") {
                let body: Vec<&str> = stmts[k + 1..].to_vec();
                let (m, warns) = Macro::parse(&body);
                for w in warns {
                    self.warn(w);
                }
                self.macros.insert(name.trim().to_string(), m);
                self.out.macros += 1;
                return;
            }
            self.ext_statement(s);
            k += 1;
        }
    }

    fn ext_statement(&mut self, s: &str) {
        let s: String = s.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        let code = s.get(..2).unwrap_or("");
        let rest = s.get(2..).unwrap_or("");
        match code {
            "FS" => self.format_spec(rest),
            "MO" => match rest.trim() {
                "MM" => self.set_unit(true),
                "IN" => self.set_unit(false),
                _ => self.warn(format!("Unknown unit {rest}.")),
            },
            "AD" => self.aperture_def(rest),
            "LP" => self.dark = !rest.trim().starts_with('C'),
            "LM" => {
                let r = rest.trim();
                self.mirror = (r.contains('X'), r.contains('Y'));
            }
            "LR" => self.rotation = num(rest).unwrap_or(0.0),
            "LS" => self.scale = num(rest).unwrap_or(1.0),
            "SR" => self.step_repeat(rest),
            "AB" => self.block(rest),
            "TF" => self.attribute(rest),
            "TA" | "TD" => self.out.x2 = true,
            "TO" => {
                self.out.x2 = true;
                if rest.starts_with(".C") || rest.starts_with(".P,") {
                    self.out.x3 = true;
                }
            }
            "IP" => {
                self.deprecated("image polarity (IP)");
                self.negative = rest.trim().starts_with("NEG");
            }
            "MI" => {
                self.deprecated("mirror image (MI)");
                self.image.set = true;
                self.image.mirror = (rest.contains("A1"), rest.contains("B1"));
            }
            "OF" => {
                self.deprecated("offset (OF)");
                self.image.set = true;
                let (a, b) = ab(rest);
                let u = self.unit();
                self.image.offset = (a.unwrap_or(0.0) * u, b.unwrap_or(0.0) * u);
            }
            "SF" => {
                self.deprecated("scale factor (SF)");
                self.image.set = true;
                let (a, b) = ab(rest);
                self.image.scale = (a.unwrap_or(1.0), b.unwrap_or(1.0));
            }
            "IR" => {
                self.deprecated("image rotation (IR)");
                self.image.set = true;
                self.image.rotate = num(rest).unwrap_or(0.0);
            }
            "AS" => {
                self.deprecated("axis select (AS)");
                self.image.set = true;
                self.image.swap = rest.starts_with("AY");
            }
            "IO" => {
                self.deprecated("image offset (IO)");
                self.image.set = true;
                let (a, b) = ab(rest);
                let u = self.unit();
                self.image.image_offset = (a.unwrap_or(0.0) * u, b.unwrap_or(0.0) * u);
            }
            "IN" | "LN" | "IJ" | "PF" | "KO" | "IC" | "TJ" => {}
            "IF" => self.warn("An included file (IF) is not read."),
            _ => {
                if s.starts_with('G') || s.starts_with('D') || s.starts_with('X') {
                    self.word(&s);
                } else {
                    self.warn(format!("Unknown command %{s}."));
                }
            }
        }
    }

    fn format_spec(&mut self, rest: &str) {
        let r = rest.trim();
        let trailing = r.starts_with('T');
        let incremental = r.get(1..2) == Some("I");
        if trailing {
            self.deprecated("trailing zero omission (FST)");
        }
        if incremental {
            self.deprecated("incremental coordinates (FS…I)");
        }
        let digits = |axis: char| -> Option<(u32, u32)> {
            let i = r.find(axis)?;
            let b = r.as_bytes();
            let a = (*b.get(i + 1)? as char).to_digit(10)?;
            let d = (*b.get(i + 2)? as char).to_digit(10)?;
            Some((a, d))
        };
        match digits('X').or(digits('Y')) {
            Some((int, dec)) => {
                self.fmt = Some(Format {
                    dec,
                    trailing,
                    int,
                    incremental,
                })
            }
            None => self.warn(format!("Coordinate format not understood: FS{r}.")),
        }
    }

    fn aperture_def(&mut self, rest: &str) {
        let r = rest.trim();
        let Some(r) = r.strip_prefix('D') else {
            self.warn(format!("Aperture definition not understood: AD{r}."));
            return;
        };
        let n_end = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        let Ok(d) = r[..n_end].parse::<u32>() else {
            self.warn(format!("Aperture definition not understood: ADD{r}."));
            return;
        };
        let rest = &r[n_end..];
        let (name, params) = match rest.split_once(',') {
            Some((n, p)) => (n.trim(), p),
            None => (rest.trim(), ""),
        };
        let vals: Vec<f64> = params
            .split(['X', 'x'])
            .filter(|s| !s.trim().is_empty())
            .map(|s| num(s).unwrap_or(0.0))
            .collect();
        let u = self.unit();
        let v = |i: usize| vals.get(i).copied().unwrap_or(0.0) * u;
        let mut rect_hole = false;
        let hole = |base: &mut BezPath, i: usize, rect_hole: &mut bool| -> bool {
            let h = vals.get(i).copied().unwrap_or(0.0) * u;
            // Two values make a rectangular hole (deprecated).
            let hy = vals.get(i + 1).copied().unwrap_or(0.0) * u;
            if h > 0.0 && hy > 0.0 {
                *rect_hole = true;
                geom::push(base, geom::rect(Point::ORIGIN, h, hy), true);
                true
            } else if h > 0.0 {
                geom::push(base, geom::circle(Point::ORIGIN, h / 2.0), true);
                true
            } else {
                false
            }
        };
        let ap = match name {
            "C" => {
                let mut base = BezPath::new();
                if v(0) > 0.0 {
                    geom::push(&mut base, geom::circle(Point::ORIGIN, v(0) / 2.0), false);
                }
                let holes = hole(&mut base, 1, &mut rect_hole);
                Aperture::Circle {
                    d: v(0),
                    base,
                    holes,
                }
            }
            "R" | "O" => {
                let mut base = BezPath::new();
                if v(0) > 0.0 && v(1) > 0.0 {
                    let s = if name == "R" {
                        geom::rect(Point::ORIGIN, v(0), v(1))
                    } else {
                        geom::obround(Point::ORIGIN, v(0), v(1))
                    };
                    geom::push(&mut base, s, false);
                }
                let holes = hole(&mut base, 2, &mut rect_hole);
                Aperture::Shape { base, holes }
            }
            "P" => {
                let n = vals.get(1).copied().unwrap_or(3.0).clamp(3.0, 12.0) as usize;
                let rot = vals.get(2).copied().unwrap_or(0.0);
                let mut base = BezPath::new();
                if v(0) > 0.0 {
                    geom::push(&mut base, geom::regular(Point::ORIGIN, v(0), n, rot), false);
                }
                let holes = hole(&mut base, 3, &mut rect_hole);
                Aperture::Shape { base, holes }
            }
            _ => match self.macros.get(name) {
                Some(m) => {
                    let mut warns = Vec::new();
                    let base = m.instance(&vals, u, &mut warns);
                    for w in warns {
                        self.warn(w);
                    }
                    let holes = has_holes(&base);
                    Aperture::Macro { base, holes }
                }
                None => {
                    self.warn(format!("Aperture D{d} uses the undefined macro {name}."));
                    return;
                }
            },
        };
        if rect_hole {
            self.deprecated("rectangular aperture holes");
        }
        self.apertures.insert(d, ap);
        self.out.apertures += 1;
    }

    fn step_repeat(&mut self, rest: &str) {
        self.flush_sr();
        let r = rest.trim();
        if r.is_empty() {
            return;
        }
        let field = |c: char| -> Option<f64> {
            let i = r.find(c)?;
            let tail = &r[i + 1..];
            let end = tail
                .find(|ch: char| ch.is_ascii_alphabetic())
                .unwrap_or(tail.len());
            num(&tail[..end])
        };
        let nx = field('X').unwrap_or(1.0).max(1.0) as u32;
        let ny = field('Y').unwrap_or(1.0).max(1.0) as u32;
        let u = self.unit();
        let dx = field('I').unwrap_or(0.0) * u;
        let dy = field('J').unwrap_or(0.0) * u;
        if nx == 1 && ny == 1 {
            return;
        }
        self.sr = Some(StepRepeat {
            nx,
            ny,
            dx,
            dy,
            objects: Vec::new(),
        });
    }

    fn flush_sr(&mut self) {
        let Some(sr) = self.sr.take() else {
            return;
        };
        for j in 0..sr.ny {
            for i in 0..sr.nx {
                let off = Vec2::new(sr.dx * f64::from(i), sr.dy * f64::from(j));
                let a = Affine::translate(off);
                for o in &sr.objects {
                    if (i, j) != (0, 0) && !self.count_copy() {
                        return;
                    }
                    let o = if (i, j) == (0, 0) {
                        o.clone()
                    } else {
                        o.transformed(a)
                    };
                    self.place(o);
                }
            }
        }
    }

    fn count_copy(&mut self) -> bool {
        self.copies += 1;
        if self.copies > self.limits.copies {
            self.warn(format!(
                "More than {} step-and-repeat or block copies; the rest are left out.",
                self.limits.copies
            ));
            return false;
        }
        true
    }

    fn block(&mut self, rest: &str) {
        let r = rest.trim();
        match r.strip_prefix('D').and_then(|d| d.parse::<u32>().ok()) {
            Some(d) => self.blocks.push((d, Vec::new())),
            None => match self.blocks.pop() {
                Some((d, objs)) => {
                    self.apertures.insert(d, Aperture::Block(objs));
                    self.out.apertures += 1;
                }
                None => self.warn("A block aperture ends that never began."),
            },
        }
    }

    fn attribute(&mut self, rest: &str) {
        let r = rest.trim().trim_start_matches('.');
        let mut parts = r.split(',');
        let name = parts.next().unwrap_or("").trim().to_string();
        if name.is_empty() {
            return;
        }
        let vals: Vec<String> = parts.map(|s| s.trim().to_string()).collect();
        if name.eq_ignore_ascii_case("FileFunction")
            && vals
                .first()
                .is_some_and(|v| v.eq_ignore_ascii_case("Component"))
        {
            self.out.x3 = true;
        }
        self.out.attributes.push((name, vals));
    }

    /// A word command (between `*`), whitespace removed.
    fn word(&mut self, raw: &str) {
        let t = raw.trim();
        if let Some(c) = t.strip_prefix("G04") {
            let c = c.trim();
            if let Some(a) = c.strip_prefix("#@!") {
                let a = a.trim().trim_start_matches('%');
                match a.get(..2) {
                    Some("TF") => self.attribute(&a[2..]),
                    Some("TA" | "TO" | "TD") => {
                        self.out.x2 = true;
                        if a.starts_with("TO.C") {
                            self.out.x3 = true;
                        }
                    }
                    _ => {}
                }
            }
            return;
        }
        let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        if s.is_empty() {
            return;
        }
        let b = s.as_bytes();
        let mut i = 0;
        let mut coords: [Option<&str>; 4] = [None; 4];
        let mut d_code: Option<u32> = None;
        while i < b.len() {
            let letter = b[i];
            let st = i + 1;
            let mut j = st;
            while j < b.len()
                && (b[j].is_ascii_digit() || b[j] == b'.' || b[j] == b'-' || b[j] == b'+')
            {
                j += 1;
            }
            let val = &s[st..j];
            match letter {
                b'G' => {
                    let g: u32 = val.parse().unwrap_or(9999);
                    self.g_code(g);
                    if g == 4 {
                        return;
                    }
                }
                b'X' => coords[0] = Some(val),
                b'Y' => coords[1] = Some(val),
                b'I' => coords[2] = Some(val),
                b'J' => coords[3] = Some(val),
                b'D' => d_code = val.parse().ok(),
                b'M' => {
                    if matches!(val.parse::<u32>(), Ok(0..=2)) {
                        self.ended = true;
                        return;
                    }
                }
                b'N' => {}
                _ => {
                    self.warn(format!("Unknown word {}.", letter as char));
                    return;
                }
            }
            i = j.max(st);
        }
        if let Some(d) = d_code
            && d >= 10
        {
            self.used_apertures = true;
            self.aperture = Some(d);
            if !self.apertures.contains_key(&d) && self.out.apertures > 0 {
                self.warn(format!("Aperture D{d} is used but never defined."));
            }
            return;
        }
        let has_xy = coords[0].is_some() || coords[1].is_some();
        let op = match d_code {
            Some(d @ 1..=3) => d,
            Some(_) => return,
            None if has_xy => {
                if self.last_op == 1 {
                    self.deprecated("coordinate data without an operation code");
                }
                self.last_op
            }
            None => return,
        };
        self.last_op = op;
        let incremental = self.fmt.is_some_and(|f| f.incremental);
        let mut to = self.cur;
        if let Some(x) = coords[0].and_then(|v| self.coord(v)) {
            to.x = if incremental { self.cur.x + x } else { x };
        }
        if let Some(y) = coords[1].and_then(|v| self.coord(v)) {
            to.y = if incremental { self.cur.y + y } else { y };
        }
        let ij = (
            coords[2].and_then(|v| self.coord(v)).unwrap_or(0.0),
            coords[3].and_then(|v| self.coord(v)).unwrap_or(0.0),
        );
        match op {
            1 => self.interpolate(to, ij),
            2 => {
                if let Some(r) = &mut self.region {
                    r.close();
                }
                self.cur = to;
            }
            _ => {
                self.cur = to;
                if self.region.is_some() {
                    self.warn("A flash inside a region is ignored.");
                } else {
                    self.flash(to);
                }
            }
        }
    }

    fn g_code(&mut self, g: u32) {
        match g {
            1 | 10..=12 => self.interp = Interp::Linear,
            2 => self.interp = Interp::Cw,
            3 => self.interp = Interp::Ccw,
            4 => {}
            36 => {
                if self.region.is_none() {
                    self.region = Some(Region::default());
                }
            }
            37 => self.end_region(),
            54 | 55 => {}
            70 => {
                self.deprecated("G70 (inches)");
                self.set_unit(false);
            }
            71 => {
                self.deprecated("G71 (millimetres)");
                self.set_unit(true);
            }
            74 => {
                self.multi = Some(false);
                self.deprecated("single-quadrant arcs (G74)");
            }
            75 => self.multi = Some(true),
            90 => {}
            91 => {
                self.deprecated("incremental coordinates (G91)");
                if let Some(f) = &mut self.fmt {
                    f.incremental = true;
                }
            }
            _ => self.warn(format!("Unknown G code G{g}.")),
        }
    }

    /// Centre, radius, start angle and sweep of an arc from the current point to `to`.
    fn arc(&mut self, to: Point, (i, j): (f64, f64)) -> Option<Segment> {
        let s = self.cur;
        let ccw = self.interp == Interp::Ccw;
        let multi = self.multi.unwrap_or(false);
        let center = if multi {
            s + Vec2::new(i, j)
        } else {
            let mut best: Option<(f64, Point)> = None;
            for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let c = s + Vec2::new(sx * i.abs(), sy * j.abs());
                let a0 = (s - c).y.atan2((s - c).x);
                let a1 = (to - c).y.atan2((to - c).x);
                let sw = geom::sweep(a0, a1, ccw, false);
                if sw.abs() > PI / 2.0 + 1e-6 {
                    continue;
                }
                let err = ((s - c).hypot() - (to - c).hypot()).abs();
                if best.is_none_or(|(e, _)| err < e - 1e-12) {
                    best = Some((err, c));
                }
            }
            best?.1
        };
        // Start and end not quite on one circle (the file's rounding, or a careless writer):
        // the centre moves to the nearest point that is, so the arc still ends where the file
        // says and the next draw starts there. Far off (a centre on the start point: arcs
        // with no offsets, which draw straight) is left alone.
        let chord = to - s;
        let (r0, r1) = ((s - center).hypot(), (to - center).hypot());
        let center = if chord.hypot() > 1e-9
            && r0.min(r1) > geom::EPS
            && (r0 - r1).abs() > 1e-9
            && (r0 - r1).abs() <= 0.1 * r0.max(r1)
        {
            let mid = s.midpoint(to);
            let n = Vec2::new(-chord.y, chord.x) / chord.hypot();
            mid + n * (center - mid).dot(n)
        } else {
            center
        };
        let r = (s - center).hypot();
        if r < geom::EPS {
            return None;
        }
        let a0 = (s - center).y.atan2((s - center).x);
        let a1 = (to - center).y.atan2((to - center).x);
        let full = multi && (s - to).hypot() < 1e-9;
        let sweep = geom::sweep(a0, a1, ccw, full);
        Some(Segment::Arc {
            center,
            radius: r,
            start: a0,
            sweep,
        })
    }

    fn interpolate(&mut self, to: Point, ij: (f64, f64)) {
        let from = self.cur;
        let seg = match self.interp {
            Interp::Linear => Some(Segment::Line(from, to)),
            _ => {
                if self.multi.is_none() {
                    self.multi = Some(false);
                    self.warn("Arcs without G74 or G75; single-quadrant mode assumed.");
                }
                self.arc(to, ij)
            }
        };
        self.cur = to;
        if let Some(r) = &mut self.region {
            let (p, n) = r.cur.get_or_insert_with(|| {
                let mut p = BezPath::new();
                p.move_to(from);
                (p, 0)
            });
            match seg {
                Some(Segment::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                }) => {
                    let arc = specular_vector::kurbo::Arc::new(
                        center,
                        Vec2::new(radius, radius),
                        start,
                        sweep,
                        0.0,
                    );
                    for el in arc.append_iter(geom::FLAT * 0.1) {
                        p.push(el);
                    }
                    p.line_to(to);
                }
                _ => p.line_to(to),
            }
            *n += 1;
            return;
        }
        let Some(seg) = seg else {
            return;
        };
        let Some(d) = self.aperture else {
            self.warn("A draw without an aperture is ignored.");
            return;
        };
        let Some(ap) = self.apertures.get(&d) else {
            return;
        };
        let k = self.scale;
        let mut macro_draw = false;
        let path = match ap {
            // Thinner than any plotted line (Allegro writes 0.1 mil for "no width"): a
            // hairline, so it stays visible at every zoom.
            Aperture::Circle { d, .. } if *d * k < HAIRLINE_MM => {
                let mut p = BezPath::new();
                p.move_to(seg.start());
                match seg {
                    Segment::Line(_, b) => p.line_to(b),
                    Segment::Arc {
                        center,
                        radius,
                        start,
                        sweep,
                    } => {
                        let arc = specular_vector::kurbo::Arc::new(
                            center,
                            Vec2::new(radius, radius),
                            start,
                            sweep,
                            0.0,
                        );
                        for el in arc.append_iter(geom::FLAT * 0.1) {
                            p.push(el);
                        }
                    }
                }
                let o = Object {
                    path: p,
                    dark: self.dark,
                    kind: ObjectKind::Hairline,
                    segment: Some(seg),
                    holes: false,
                };
                self.emit(o);
                return;
            }
            Aperture::Circle { d, .. } => {
                let hw = d * k / 2.0;
                match seg {
                    Segment::Line(a, b) => geom::capsule(a, b, hw),
                    Segment::Arc {
                        center,
                        radius,
                        start,
                        sweep,
                    } => geom::arc_band(center, radius, start, sweep, hw),
                }
            }
            Aperture::Shape { base, .. } | Aperture::Macro { base, .. } => {
                macro_draw = matches!(ap, Aperture::Macro { .. });
                let base = geom::transformed(base, self.aperture_transform(Point::ORIGIN));
                let pts = geom::sample(&base);
                sweep_hull(&pts, &seg)
            }
            Aperture::Block(_) => {
                self.warn("A draw with a block aperture is ignored.");
                return;
            }
        };
        if macro_draw {
            self.warn("Draws with macro apertures are drawn as their convex hull.");
        }
        let o = Object {
            path,
            dark: self.dark,
            kind: ObjectKind::Draw,
            segment: Some(seg),
            holes: false,
        };
        self.emit(o);
    }

    /// Mirroring, then rotation, then scaling, then the move to `at`.
    fn aperture_transform(&self, at: Point) -> Affine {
        let m = Affine::scale_non_uniform(
            if self.mirror.0 { -1.0 } else { 1.0 },
            if self.mirror.1 { -1.0 } else { 1.0 },
        );
        Affine::translate(at.to_vec2())
            * Affine::scale(self.scale)
            * Affine::rotate(self.rotation.to_radians())
            * m
    }

    fn flash(&mut self, at: Point) {
        let Some(d) = self.aperture else {
            self.warn("A flash without an aperture is ignored.");
            return;
        };
        let Some(ap) = self.apertures.get(&d) else {
            return;
        };
        let a = self.aperture_transform(at);
        match ap {
            Aperture::Circle { base, holes, .. }
            | Aperture::Shape { base, holes }
            | Aperture::Macro { base, holes } => {
                if base.elements().is_empty() {
                    return;
                }
                let o = Object {
                    path: geom::transformed(base, a),
                    dark: self.dark,
                    kind: ObjectKind::Flash,
                    segment: None,
                    holes: *holes,
                };
                self.emit(o);
            }
            Aperture::Block(objs) => {
                let objs = objs.clone();
                for o in &objs {
                    if !self.count_copy() {
                        return;
                    }
                    let mut o = o.transformed(a);
                    if !self.dark {
                        o.dark = !o.dark;
                    }
                    self.emit(o);
                }
            }
        }
    }

    fn end_region(&mut self) {
        let Some(mut r) = self.region.take() else {
            return;
        };
        r.close();
        if r.done.elements().is_empty() {
            return;
        }
        let o = Object {
            path: r.done,
            dark: self.dark,
            kind: ObjectKind::Region,
            segment: None,
            holes: false,
        };
        self.emit(o);
    }

    fn emit(&mut self, o: Object) {
        if let Some((_, objs)) = self.blocks.last_mut() {
            objs.push(o);
        } else if let Some(sr) = &mut self.sr {
            sr.objects.push(o);
        } else {
            self.place(o);
        }
    }

    fn place(&mut self, o: Object) {
        if self.out.objects.len() >= self.limits.items {
            if !self.full {
                self.full = true;
                self.warn(format!(
                    "More than {} objects; the rest are left out.",
                    self.limits.items
                ));
            }
            return;
        }
        self.out.objects.push(o);
    }

    fn finish(&mut self) {
        self.flush_sr();
        while let Some((d, objs)) = self.blocks.pop() {
            self.warn("A block aperture is not closed.");
            self.apertures.insert(d, Aperture::Block(objs));
        }
        if self.image.set {
            let im = &self.image;
            let swap = if im.swap {
                Affine::new([0.0, 1.0, 1.0, 0.0, 0.0, 0.0])
            } else {
                Affine::IDENTITY
            };
            let mirror = Affine::scale_non_uniform(
                if im.mirror.0 { -1.0 } else { 1.0 },
                if im.mirror.1 { -1.0 } else { 1.0 },
            );
            let a = Affine::translate(Vec2::new(im.image_offset.0, im.image_offset.1))
                * Affine::rotate(im.rotate.to_radians())
                * Affine::translate(Vec2::new(im.offset.0, im.offset.1))
                * Affine::scale_non_uniform(im.scale.0, im.scale.1)
                * mirror
                * swap;
            if a != Affine::IDENTITY {
                for o in &mut self.out.objects {
                    *o = o.transformed(a);
                }
            }
        }
        let mut bounds: Option<Rect> = None;
        for o in &self.out.objects {
            let b = o.path.bounding_box();
            if b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite() {
                bounds = Some(bounds.map_or(b, |r| r.union(b)));
            }
        }
        if self.negative
            && let Some(b) = bounds
        {
            for o in &mut self.out.objects {
                o.dark = !o.dark;
            }
            let back = Object {
                path: geom::rect(b.center(), b.width() + 2.0, b.height() + 2.0),
                dark: true,
                kind: ObjectKind::Region,
                segment: None,
                holes: false,
            };
            self.out.objects.insert(0, back);
            bounds = Some(b.inflate(1.0, 1.0));
        }
        self.out.bounds = bounds;
    }
}

/// `A<num>B<num>` values.
fn ab(s: &str) -> (Option<f64>, Option<f64>) {
    let a = s.find('A').map(|i| {
        let t = &s[i + 1..];
        num(&t[..t.find('B').unwrap_or(t.len())])
    });
    let b = s.find('B').map(|i| num(&s[i + 1..]));
    (a.flatten(), b.flatten())
}

fn has_holes(p: &BezPath) -> bool {
    // Any subpath with negative area.
    let mut sub = BezPath::new();
    let mut neg = false;
    let mut check = |sub: &mut BezPath| {
        if !sub.elements().is_empty() && sub.area() < 0.0 {
            neg = true;
        }
        *sub = BezPath::new();
    };
    for el in p.elements() {
        if matches!(el, specular_vector::kurbo::PathEl::MoveTo(_)) {
            check(&mut sub);
        }
        sub.push(*el);
    }
    check(&mut sub);
    neg
}

/// A convex aperture (outline points `pts` about the origin) swept along `seg`.
fn sweep_hull(pts: &[Point], seg: &Segment) -> BezPath {
    let along: Vec<Point> = match *seg {
        Segment::Line(a, b) => vec![a, b],
        Segment::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let n = ((sweep.abs() / (PI / 64.0)).ceil() as usize).max(2);
            (0..=n)
                .map(|i| center + geom::unit(start + sweep * i as f64 / n as f64) * radius)
                .collect()
        }
    };
    let mut out = BezPath::new();
    for w in along.windows(2) {
        let mut all: Vec<Point> = pts.iter().map(|p| *p + w[0].to_vec2()).collect();
        all.extend(pts.iter().map(|p| *p + w[1].to_vec2()));
        if let Some(h) = geom::hull(&all) {
            geom::push(&mut out, h, false);
        }
    }
    out
}

#[cfg(test)]
mod tests;
