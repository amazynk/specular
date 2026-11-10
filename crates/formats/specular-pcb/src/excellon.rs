//! Excellon drill and rout files (NC drill): the header's units, zeros and number format,
//! tools, hits, G85 slots, routed paths (G00/G01/G02/G03 between M15 and M16) and repeats.
//! Positions in millimetres, y up.

use specular_vector::kurbo::{Point, Rect, Vec2};

use crate::gerber::Segment;
use crate::{Error, Limits, geom};

/// A drill tool.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub number: u32,
    /// Millimetres.
    pub diameter: f64,
    /// Plated (`Some(true)`), not plated, or not said.
    pub plated: Option<bool>,
}

/// One drilled hole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    pub at: Point,
    /// Index into [`Drill::tools`].
    pub tool: usize,
}

/// A slot or routed path: the tool's centre line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub segment: Segment,
    pub tool: usize,
}

/// A drill file.
#[derive(Clone, Debug, Default)]
pub struct Drill {
    pub tools: Vec<Tool>,
    pub holes: Vec<Hole>,
    pub slots: Vec<Slot>,
    /// File attributes from `; #@! TF.` comments.
    pub attributes: Vec<(String, Vec<String>)>,
    /// The file says its holes are plated (`Some(true)`) or not.
    pub plated: Option<bool>,
    pub inch: bool,
    /// Coordinates without a decimal point keep their leading zeros (`LZ`).
    pub leading_zeros: bool,
    /// Units, zeros or number format were not stated and were guessed.
    pub guessed: bool,
    /// The program that wrote it, when a comment says.
    pub generator: Option<String>,
    /// Bounds of the holes and slots, millimetres.
    pub bounds: Option<Rect>,
    pub warnings: Vec<String>,
}

impl Drill {
    /// `.FileFunction` as written.
    pub fn file_function(&self) -> Option<String> {
        self.attributes
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("FileFunction"))
            .map(|(_, v)| v.join(","))
    }

    /// Bounds of the holes and slots with their tools' radii.
    fn measure(&self) -> Option<Rect> {
        let mut bounds: Option<Rect> = None;
        let rad = |t: usize| self.tools.get(t).map_or(0.0, |t| t.diameter / 2.0);
        for h in &self.holes {
            let rr = rad(h.tool);
            let b = Rect::new(h.at.x - rr, h.at.y - rr, h.at.x + rr, h.at.y + rr);
            bounds = Some(bounds.map_or(b, |x| x.union(b)));
        }
        for s in &self.slots {
            let rr = rad(s.tool);
            for p in [s.segment.start(), s.segment.end()] {
                let b = Rect::new(p.x - rr, p.y - rr, p.x + rr, p.y + rr);
                bounds = Some(bounds.map_or(b, |x| x.union(b)));
            }
        }
        bounds
    }

    /// Sizes the tools the file selects but never defines from `table` ((tool number,
    /// diameter mm), e.g. [`allegro_log_tools`]). Returns how many were sized.
    pub fn size_undefined_tools(&mut self, table: &[(u32, f64)]) -> usize {
        let mut sized = Vec::new();
        for t in &mut self.tools {
            if t.diameter > 0.0 {
                continue;
            }
            if let Some(&(_, d)) = table.iter().find(|(k, _)| *k == t.number) {
                t.diameter = d;
                sized.push(format!("Tool T{} is used but never defined.", t.number));
            }
        }
        if !sized.is_empty() {
            self.bounds = self.measure();
            self.warnings.retain(|w| !sized.contains(w));
        }
        sized.len()
    }

    /// Distinct tool diameters that drilled something.
    pub fn drill_sizes(&self) -> u32 {
        let mut used: Vec<u64> = self
            .holes
            .iter()
            .map(|h| h.tool)
            .chain(self.slots.iter().map(|s| s.tool))
            .filter_map(|t| self.tools.get(t))
            .map(|t| (t.diameter * 1e4).round() as u64)
            .collect();
        used.sort_unstable();
        used.dedup();
        used.len() as u32
    }
}

/// Tool sizes from a Cadence Allegro NC log written beside its drill and route files, whose
/// tapes often select tools they never define: `ncdrill.log` lists drill sizes in mils,
/// `ncroute.log` (`route`) router bits in inches. Returns (tool number, diameter mm); empty
/// for metric logs (no sample to confirm their units) or text that is not such a log.
pub fn allegro_log_tools(text: &str, route: bool) -> Vec<(u32, f64)> {
    let english = text.lines().any(|l| {
        let mut w = l.split_whitespace();
        w.next() == Some("OUTPUT-UNITS") && w.next() == Some("ENGLISH")
    });
    if !english {
        return Vec::new();
    }
    let unit = if route { 25.4 } else { 0.0254 };
    let mut out: Vec<(u32, f64)> = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let tool = words.iter().position(|w| {
            w.len() > 1 && w.starts_with('T') && w[1..].bytes().all(|b| b.is_ascii_digit())
        });
        let (Some(t), Some(size)) = (tool, words.first().and_then(|w| w.parse::<f64>().ok()))
        else {
            continue;
        };
        // "size [P|N] Tnn [tolerances]": the size first, the tool second or third.
        if !(1..=2).contains(&t) || !size.is_finite() || size <= 0.0 {
            continue;
        }
        let Ok(n) = words[t][1..].parse::<u32>() else {
            continue;
        };
        if !out.iter().any(|(k, _)| *k == n) {
            out.push((n, size * unit));
        }
    }
    out
}

/// Whether `head` is an Excellon file: an `M48` header, or tool selections and hits with
/// an end code and no Gerber `*`.
pub fn is_excellon(head: &[u8]) -> bool {
    let head = &head[..head.len().min(16 * 1024)];
    if head.is_empty() || head.contains(&0) {
        return false;
    }
    let text = String::from_utf8_lossy(head);
    let mut m48 = false;
    let mut tools = 0;
    let mut hits = 0;
    let mut stars = 0;
    for line in text.lines().take(400) {
        let l = line.trim();
        if l.is_empty() || l.starts_with(';') {
            continue;
        }
        if l.contains('*') {
            stars += 1;
        }
        if l == "M48" {
            m48 = true;
        }
        let b = l.as_bytes();
        if b[0] == b'T' && b.get(1).is_some_and(u8::is_ascii_digit) {
            tools += 1;
        }
        let pos = l
            .strip_prefix("G00")
            .or_else(|| l.strip_prefix("G01"))
            .unwrap_or(l)
            .as_bytes();
        if (pos.first() == Some(&b'X') || pos.first() == Some(&b'Y'))
            && pos
                .get(1)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'-' || *c == b'.')
        {
            hits += 1;
        }
    }
    stars == 0 && (m48 || (tools > 0 && hits > 0 && text.contains('%')))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NumFormat {
    inch: bool,
    /// Leading zeros kept (`LZ`): digits count from the left.
    leading: bool,
    int: u32,
    dec: u32,
    /// The file writes decimal numbers: one without a point is a whole number.
    decimal: bool,
}

impl NumFormat {
    fn value(&self, s: &str) -> Option<f64> {
        let unit = if self.inch { 25.4 } else { 1.0 };
        if s.contains('.') || self.decimal {
            return s.parse::<f64>().ok().map(|v| v * unit);
        }
        let (neg, d) = match s.as_bytes().first()? {
            b'-' => (true, &s[1..]),
            b'+' => (false, &s[1..]),
            _ => (false, s),
        };
        if d.is_empty() || !d.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let v = if self.leading {
            let mut p = d.to_string();
            while p.len() < (self.int + self.dec) as usize {
                p.push('0');
            }
            p.parse::<f64>().ok()? / 10f64.powi(self.dec as i32)
        } else {
            d.parse::<f64>().ok()? / 10f64.powi(self.dec as i32)
        };
        Some(if neg { -v } else { v } * unit)
    }
}

/// What the header states.
#[derive(Clone, Copy, Debug, Default)]
struct Stated {
    units: Option<bool>,
    zeros: Option<bool>,
    digits: Option<(u32, u32)>,
    /// Some coordinate has no decimal point.
    integers: bool,
    /// Some coordinate has one: the file writes decimals (and "X0" is just 0).
    decimals: bool,
}

/// Reads a drill file.
pub fn parse(bytes: &[u8], limits: &Limits) -> Result<Drill, Error> {
    parse_with_hint(bytes, limits, None)
}

/// Reads a drill file; `board` (millimetres) helps to guess units and zeros the file does
/// not state: the reading whose holes fit the board best is taken.
pub fn parse_with_hint(bytes: &[u8], limits: &Limits, board: Option<Rect>) -> Result<Drill, Error> {
    if bytes.contains(&0) {
        return Err(Error::NotThisFormat);
    }
    let text = String::from_utf8_lossy(bytes);
    let stated = scan(&text);
    let inch = stated.units.unwrap_or(true);
    let default_digits = |inch: bool| if inch { (2, 4) } else { (3, 3) };
    let fixed =
        stated.units.is_some() && stated.zeros.is_some() || stated.decimals || !stated.integers;
    if fixed {
        let (int, dec) = stated.digits.unwrap_or(default_digits(inch));
        let f = NumFormat {
            inch,
            leading: stated.zeros.unwrap_or(false),
            int,
            dec,
            decimal: stated.decimals,
        };
        if stated.units.is_none() && (stated.decimals || stated.integers) {
            let mut d = run(&text, f, limits)?;
            crate::warn(
                &mut d.warnings,
                "The drill file does not state its units; inches assumed.",
            );
            d.guessed = true;
            return Ok(d);
        }
        return run(&text, f, limits);
    }
    // Guess: every reading, scored against the board (else the usual defaults).
    let mut candidates = Vec::new();
    for inch in [true, false] {
        if stated.units.is_some_and(|u| u != inch) {
            continue;
        }
        for leading in [false, true] {
            if stated.zeros.is_some_and(|z| z != leading) {
                continue;
            }
            let digits: Vec<(u32, u32)> = match stated.digits {
                Some(d) => vec![d],
                None if inch => vec![(2, 4), (2, 3), (3, 3)],
                None => vec![(3, 3), (3, 2), (4, 2), (4, 3), (2, 4)],
            };
            for (int, dec) in digits {
                candidates.push(NumFormat {
                    inch,
                    leading,
                    int,
                    dec,
                    decimal: false,
                });
            }
        }
    }
    let first = candidates[0];
    let mut best: Option<(f64, Drill)> = None;
    match board {
        Some(b) => {
            for f in candidates {
                let d = run(&text, f, limits)?;
                let s = fit_score(&d, b);
                if best.as_ref().is_none_or(|(bs, _)| s > *bs) {
                    best = Some((s, d));
                }
            }
        }
        None => best = Some((0.0, run(&text, first, limits)?)),
    }
    let (_, mut d) = best.expect("a reading");
    d.guessed = true;
    let what = match (stated.units, stated.zeros) {
        (None, None) => "units and zero suppression",
        (None, Some(_)) => "units",
        _ => "zero suppression",
    };
    let how = if board.is_some() {
        "guessed from the board's size"
    } else {
        "assumed"
    };
    crate::warn(
        &mut d.warnings,
        format!(
            "The drill file does not state its {what}; {} with {} zeros {how}.",
            if d.inch { "inches" } else { "millimetres" },
            if d.leading_zeros {
                "leading"
            } else {
                "trailing"
            },
        ),
    );
    Ok(d)
}

/// How well the holes of `d` fit `board`: overlap of the bounds, size alike.
fn fit_score(d: &Drill, board: Rect) -> f64 {
    let Some(b) = d.bounds else {
        return -1e9;
    };
    let inter = b.intersect(board);
    let ia = if inter.width() > 0.0 && inter.height() > 0.0 {
        inter.area()
    } else {
        0.0
    };
    let ua = b.union(board).area().max(1e-12);
    ia / ua
}

fn scan(text: &str) -> Stated {
    let mut s = Stated::default();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with(';') {
            continue;
        }
        let up = l.to_ascii_uppercase();
        if up.starts_with("METRIC")
            || up.starts_with("INCH")
            || up.starts_with("M71,")
            || up.starts_with("M72,")
        {
            s.units = Some(up.starts_with("INCH") || up.starts_with("M72"));
            for part in up.split(',').skip(1) {
                match part.trim() {
                    "LZ" => s.zeros = Some(true),
                    "TZ" => s.zeros = Some(false),
                    p if p.contains('.') && p.bytes().all(|c| c == b'0' || c == b'.') => {
                        if let Some((a, b)) = p.split_once('.') {
                            s.digits = Some((a.len() as u32, b.len() as u32));
                        }
                    }
                    _ => {}
                }
            }
        } else if up == "M71" {
            s.units = s.units.or(Some(false));
        } else if up == "M72" {
            s.units = s.units.or(Some(true));
        } else if up.starts_with('X') || up.starts_with('Y') || up.contains("G85X") {
            let coord_part = up.split(['T', 'G', 'M']).next().unwrap_or("");
            if coord_part.contains('.') {
                s.decimals = true;
            } else if !coord_part.is_empty() {
                s.integers = true;
            }
        }
    }
    s
}

struct Run<'l> {
    f: NumFormat,
    limits: &'l Limits,
    d: Drill,
    cur: Point,
    tool: Option<usize>,
    incremental: bool,
    rout: bool,
    down: bool,
    seen_m15: bool,
    pending_plated: Option<bool>,
    full: bool,
    /// Holes recorded since `M25` (the pattern), while recording.
    pattern: Option<Vec<Hole>>,
    last_pattern: Vec<Hole>,
}

fn run(text: &str, f: NumFormat, limits: &Limits) -> Result<Drill, Error> {
    let mut r = Run {
        f,
        limits,
        d: Drill {
            inch: f.inch,
            leading_zeros: f.leading,
            ..Drill::default()
        },
        cur: Point::ORIGIN,
        tool: None,
        incremental: false,
        rout: false,
        down: false,
        seen_m15: text.contains("M15"),
        pending_plated: None,
        full: false,
        pattern: None,
        last_pattern: Vec::new(),
    };
    let mut lines = 0usize;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        lines += 1;
        if let Some(c) = l.strip_prefix(';') {
            r.comment(c.trim());
            continue;
        }
        let l: String = l.chars().filter(|c| !c.is_whitespace()).collect();
        if r.line(&l) {
            break;
        }
    }
    if lines == 0 {
        return Err(Error::NotThisFormat);
    }
    r.d.bounds = r.d.measure();
    if r.d.plated.is_none() {
        let tools: Vec<Option<bool>> = r.d.tools.iter().map(|t| t.plated).collect();
        if !tools.is_empty() && tools.iter().all(|p| *p == tools[0]) {
            r.d.plated = tools[0];
        }
    }
    Ok(r.d)
}

/// The value after `letter` in `s` (up to the next letter).
fn field(s: &str, letter: char) -> Option<&str> {
    let i = s.find(letter)?;
    let t = &s[i + 1..];
    let end = t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len());
    Some(&t[..end])
}

impl Run<'_> {
    fn warn(&mut self, w: impl Into<String>) {
        crate::warn(&mut self.d.warnings, w);
    }

    fn comment(&mut self, c: &str) {
        if let Some(a) = c.strip_prefix("#@!") {
            let a = a.trim();
            if let Some(tf) = a.strip_prefix("TF.") {
                let mut parts = tf.split(',');
                let name = parts.next().unwrap_or("").to_string();
                let vals: Vec<String> = parts.map(|s| s.trim().to_string()).collect();
                if name.eq_ignore_ascii_case("FileFunction") {
                    match vals.first().map(String::as_str) {
                        Some("Plated") => self.d.plated = Some(true),
                        Some("NonPlated") => self.d.plated = Some(false),
                        _ => {}
                    }
                }
                if name.eq_ignore_ascii_case("GenerationSoftware") {
                    self.d.generator = Some(vals.join(" "));
                }
                self.d.attributes.push((name, vals));
            } else if let Some(ta) = a.strip_prefix("TA.AperFunction,") {
                self.pending_plated = if ta.starts_with("NonPlated") {
                    Some(false)
                } else if ta.starts_with("Plated") {
                    Some(true)
                } else {
                    None
                };
            }
            return;
        }
        let up = c.to_ascii_uppercase();
        // Allegro: ";T01 Holesize 1. = 12.000000 Tolerance = … PLATED MILS Quantity = 241".
        if let Some(rest) = up.strip_prefix('T')
            && let Some((num, tail)) = rest.split_once(" HOLESIZE")
            && let Ok(n) = num.trim().parse::<u32>()
            && let Some((_, v)) = tail.split_once('=')
            && let Some(size) = v
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        {
            let unit = if up.contains(" MILS") {
                0.0254
            } else if up.contains(" MM") {
                1.0
            } else if up.contains(" INCH") {
                25.4
            } else if self.f.inch {
                0.0254
            } else {
                1.0
            };
            let plated = if up.contains("NON_PLATED") {
                Some(false)
            } else if up.contains("PLATED") {
                Some(true)
            } else {
                None
            };
            if !self.d.tools.iter().any(|t| t.number == n) {
                self.d.tools.push(Tool {
                    number: n,
                    diameter: size * unit,
                    plated,
                });
            }
            return;
        }
        if up.contains("TYPE=NON_PLATED") || up.contains("NON-PLATED") || up.contains("NPTH") {
            self.pending_plated = Some(false);
            if up.starts_with("TYPE=") {
                self.d.plated = Some(false);
            }
        } else if up.contains("TYPE=PLATED") {
            self.pending_plated = Some(true);
            self.d.plated = Some(true);
        }
        if self.d.generator.is_none() {
            for (key, name) in [
                ("KICAD", "KiCad"),
                ("ALTIUM", "Altium"),
                ("EAGLE", "Eagle"),
                ("DIPTRACE", "DipTrace"),
                ("FRITZING", "Fritzing"),
            ] {
                if up.contains(key) {
                    self.d.generator = Some(name.into());
                }
            }
        }
    }

    fn coord(&self, s: &str) -> Option<f64> {
        self.f.value(s)
    }

    /// Reads `X…Y…` in `s`, updating the current point; whether there were any.
    fn point(&mut self, s: &str) -> Option<Point> {
        let x = field(s, 'X').and_then(|v| self.coord(v));
        let y = field(s, 'Y').and_then(|v| self.coord(v));
        if x.is_none() && y.is_none() {
            return None;
        }
        let mut p = self.cur;
        if self.incremental {
            p += Vec2::new(x.unwrap_or(0.0), y.unwrap_or(0.0));
        } else {
            if let Some(x) = x {
                p.x = x;
            }
            if let Some(y) = y {
                p.y = y;
            }
        }
        Some(p)
    }

    fn define_tool(&mut self, l: &str) {
        let num_end = l[1..]
            .find(|c: char| !c.is_ascii_digit())
            .map_or(l.len(), |i| i + 1);
        let Ok(n) = l[1..num_end].parse::<u32>() else {
            return;
        };
        let rest = &l[num_end..];
        if let Some(c) = field(rest, 'C') {
            let unit = if self.f.inch { 25.4 } else { 1.0 };
            let d = c.parse::<f64>().unwrap_or(0.0) * unit;
            let plated = self.pending_plated.take();
            if let Some(t) = self.d.tools.iter_mut().find(|t| t.number == n) {
                t.diameter = d;
                t.plated = plated.or(t.plated);
            } else {
                self.d.tools.push(Tool {
                    number: n,
                    diameter: d,
                    plated,
                });
            }
        }
        if n == 0 {
            self.tool = None;
            return;
        }
        self.tool = Some(match self.d.tools.iter().position(|t| t.number == n) {
            Some(i) => i,
            None => {
                self.warn(format!("Tool T{n} is used but never defined."));
                self.d.tools.push(Tool {
                    number: n,
                    diameter: 0.0,
                    plated: None,
                });
                self.d.tools.len() - 1
            }
        });
    }

    fn room(&mut self) -> bool {
        if self.d.holes.len() + self.d.slots.len() >= self.limits.items {
            if !self.full {
                self.full = true;
                let n = self.limits.items;
                self.warn(format!("More than {n} holes; the rest are left out."));
            }
            return false;
        }
        true
    }

    fn hole(&mut self, at: Point) {
        let tool = self.tool;
        match tool {
            Some(t) if self.room() => {
                let h = Hole { at, tool: t };
                if let Some(p) = &mut self.pattern {
                    p.push(h);
                }
                self.d.holes.push(h);
            }
            Some(_) => {}
            None => self.warn("A hole without a tool is ignored."),
        }
    }

    fn slot(&mut self, segment: Segment) {
        let tool = self.tool;
        match tool {
            Some(t) if self.room() => self.d.slots.push(Slot { segment, tool: t }),
            Some(_) => {}
            None => self.warn("A slot without a tool is ignored."),
        }
    }

    /// Handles one line; true at the end of the program.
    fn line(&mut self, l: &str) -> bool {
        let up = l.to_ascii_uppercase();
        let l = up.as_str();
        match l {
            "M48" | "%" | "M95" | "G05" | "G90" | "G81" | "M71" | "M72" => {
                match l {
                    "G05" => self.rout = false,
                    "G90" => self.incremental = false,
                    _ => {}
                }
                return false;
            }
            "M30" | "M00" => return true,
            "M15" => {
                self.down = true;
                return false;
            }
            "M25" => {
                self.pattern = Some(Vec::new());
                return false;
            }
            "M01" => {
                if let Some(p) = self.pattern.take() {
                    self.last_pattern = p;
                }
                return false;
            }
            "M16" | "M17" => {
                self.down = false;
                return false;
            }
            "G91" => {
                self.incremental = true;
                return false;
            }
            _ => {}
        }
        if l.starts_with("METRIC") || l.starts_with("INCH") || l.starts_with("FMAT") {
            return false;
        }
        if l.starts_with("ICI") {
            self.incremental = l.contains("ON");
            return false;
        }
        if l.starts_with('T') && l.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
            self.define_tool(l);
            return false;
        }
        if let Some(rest) = l.strip_prefix('R')
            && let Some(n) = rest
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|n| n.parse::<u32>().ok())
        {
            // Repeat: n more holes, each offset by X/Y from the last.
            let dx = field(rest, 'X').and_then(|v| self.coord(v)).unwrap_or(0.0);
            let dy = field(rest, 'Y').and_then(|v| self.coord(v)).unwrap_or(0.0);
            for _ in 0..n.min(100_000) {
                self.cur += Vec2::new(dx, dy);
                self.hole(self.cur);
            }
            return false;
        }
        // Repeat the last pattern, offset.
        if let Some(rest) = l.strip_prefix("M02")
            && (rest.starts_with('X') || rest.starts_with('Y'))
        {
            let dx = field(rest, 'X').and_then(|v| self.coord(v)).unwrap_or(0.0);
            let dy = field(rest, 'Y').and_then(|v| self.coord(v)).unwrap_or(0.0);
            for h in self.last_pattern.clone() {
                if !self.room() {
                    break;
                }
                self.d.holes.push(Hole {
                    at: h.at + Vec2::new(dx, dy),
                    tool: h.tool,
                });
            }
            return false;
        }
        // Routed circles (G32 clockwise, G33 counter-clockwise): centre, radius A or the
        // distance from the current position.
        if let Some(rest) = l.strip_prefix("G32").or_else(|| l.strip_prefix("G33")) {
            let mut c = self.cur;
            if let Some(x) = field(rest, 'X').and_then(|v| self.coord(v)) {
                c.x = x;
            }
            if let Some(y) = field(rest, 'Y').and_then(|v| self.coord(v)) {
                c.y = y;
            }
            let r = field(rest, 'A')
                .and_then(|v| self.coord(v))
                .unwrap_or_else(|| (self.cur - c).hypot());
            if r > geom::EPS {
                let sweep = if l.starts_with("G32") {
                    -std::f64::consts::TAU
                } else {
                    std::f64::consts::TAU
                };
                self.slot(Segment::Arc {
                    center: c,
                    radius: r,
                    start: 0.0,
                    sweep,
                });
            }
            return false;
        }
        if let Some((a, b)) = l.split_once("G85") {
            let s = self.point(a);
            if let Some(s) = s {
                self.cur = s;
            }
            let start = self.cur;
            if let Some(e) = self.point(b) {
                self.cur = e;
                self.slot(Segment::Line(start, e));
            }
            return false;
        }
        let g = l
            .strip_prefix('G')
            .and_then(|r| r.get(..2))
            .and_then(|c| c.parse::<u32>().ok());
        match g {
            Some(0) => {
                self.rout = true;
                self.down = false;
                if let Some(p) = self.point(&l[3..]) {
                    self.cur = p;
                }
            }
            Some(1) => {
                if let Some(p) = self.point(&l[3..]) {
                    if self.down || !self.seen_m15 {
                        let s = self.cur;
                        self.slot(Segment::Line(s, p));
                    }
                    self.cur = p;
                }
            }
            Some(2 | 3) => {
                let rest = &l[3..];
                if let Some(p) = self.point(rest) {
                    if self.down || !self.seen_m15 {
                        let seg = self.arc(p, rest, g == Some(3));
                        if let Some(seg) = seg {
                            self.slot(seg);
                        }
                    }
                    self.cur = p;
                }
            }
            Some(_) => {}
            None => {
                if l.starts_with('X') || l.starts_with('Y') {
                    // A hit, possibly with a tool change after it ("X…Y…T2").
                    let (coords, tool) = match l.find('T') {
                        Some(i) => (&l[..i], Some(&l[i..])),
                        None => (l, None),
                    };
                    if let Some(p) = self.point(coords) {
                        // In rout mode (after G00, until G05) a bare position moves the
                        // tool, cutting while it is down.
                        let s = self.cur;
                        self.cur = p;
                        if !self.rout {
                            self.hole(p);
                        } else if self.down {
                            self.slot(Segment::Line(s, p));
                        }
                    }
                    if let Some(t) = tool {
                        self.define_tool(t);
                    }
                }
            }
        }
        false
    }

    fn arc(&mut self, to: Point, rest: &str, ccw: bool) -> Option<Segment> {
        let from = self.cur;
        let center = if let Some(a) = field(rest, 'A').and_then(|v| self.coord(v)) {
            // Radius: the centre on the side that makes the shorter arc.
            let chord = to - from;
            let len = chord.hypot();
            if len < geom::EPS || a.abs() < len / 2.0 - 1e-9 {
                return Some(Segment::Line(from, to));
            }
            let h = (a * a - len * len / 4.0).max(0.0).sqrt();
            let mid = from.midpoint(to);
            let n = Vec2::new(-chord.y, chord.x) / len;
            if ccw { mid + n * h } else { mid - n * h }
        } else {
            let i = field(rest, 'I').and_then(|v| self.coord(v)).unwrap_or(0.0);
            let j = field(rest, 'J').and_then(|v| self.coord(v)).unwrap_or(0.0);
            from + Vec2::new(i, j)
        };
        let r = (from - center).hypot();
        if r < geom::EPS {
            return None;
        }
        let a0 = (from - center).y.atan2((from - center).x);
        let a1 = (to - center).y.atan2((to - center).x);
        let sweep = geom::sweep(a0, a1, ccw, (from - to).hypot() < 1e-9);
        Some(Segment::Arc {
            center,
            radius: r,
            start: a0,
            sweep,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn sniffs_drill_files() {
        assert!(is_excellon(
            b"M48\n;DRILL file {KiCad 8}\nMETRIC\nT1C0.8\n%\nT1\nX1.0Y2.0\nM30\n"
        ));
        assert!(is_excellon(b"%\nT01\nX001000Y002000\nT02\nX3Y4\nM30\n"));
        assert!(!is_excellon(b"%FSLAX26Y26*%\n%MOMM*%\nX0Y0D03*\n"));
        assert!(!is_excellon(b"just some text\n"));
    }

    #[test]
    fn kicad_decimal_with_slots_and_plating() {
        let src = "M48\n; DRILL file {KiCad 8.0.1} date 2024\n; FORMAT={-:-/ absolute / metric / decimal}\n\
            ; #@! TF.FileFunction,MixedPlating,1,2\nFMAT,2\nMETRIC\n\
            ; #@! TA.AperFunction,Plated,PTH,ComponentDrill\nT1C0.800\n\
            ; #@! TA.AperFunction,NonPlated,NPTH,ComponentDrill\nT2C3.200\n%\nG90\nG05\n\
            T1\nX10.0Y-5.0\nX12.5Y-5.0\nT2\nX0Y0\nT1\nG00X20.0Y-5.0\nM15\nG01X22.0Y-5.0\nM16\nG05\nT0\nM30\n";
        let d = parse(src.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(d.tools.len(), 2);
        assert_eq!(d.tools[0].plated, Some(true));
        assert_eq!(d.tools[1].plated, Some(false));
        assert_eq!(d.holes.len(), 3);
        assert_eq!(d.slots.len(), 1);
        assert_eq!(d.holes[1].at, Point::new(12.5, -5.0));
        assert!(!d.guessed, "{:?}", d.warnings);
        assert_eq!(d.drill_sizes(), 2);
        assert_eq!(d.generator.as_deref(), Some("KiCad"));
    }

    #[test]
    fn zeros_and_formats() {
        // Inch, leading zeros kept, 2.4: "0125" is 012500, 1.25 in.
        let src = "M48\nINCH,LZ\nT1C0.0236\n%\nT1\nX0125Y01\nM30\n";
        let d = parse(src.as_bytes(), &Limits::default()).unwrap();
        let p = d.holes[0].at;
        assert!(
            (p.x - 1.25 * 25.4).abs() < 1e-9 && (p.y - 25.4).abs() < 1e-9,
            "{p:?}"
        );
        // Metric, trailing zeros kept, 3.3: "12500" is 12.5 mm.
        let src = "M48\nMETRIC,TZ,000.000\nT1C0.8\n%\nT1\nX12500Y-500\nM30\n";
        let d = parse(src.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(d.holes[0].at, Point::new(12.5, -0.5));
        // G85 slot and a repeat.
        let src = "M48\nMETRIC\nT1C1.0\n%\nT1\nX1.0Y1.0G85X5.0Y1.0\nX0Y0\nR3X1.0\nM30\n";
        let d = parse(src.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(d.slots.len(), 1);
        assert_eq!(d.holes.len(), 4);
        assert_eq!(d.holes[3].at, Point::new(3.0, 0.0));
    }

    #[test]
    fn guesses_units_against_the_board() {
        // No header units: 2.4 inch trailing-suppressed puts holes at 25.4 mm; metric 3.3 at
        // 1 mm. A 30 mm board picks inches.
        let src = "%\nT1C0.03\nX10000Y10000\nX5000Y5000\nM30\n";
        let board = Rect::new(0.0, 0.0, 30.0, 30.0);
        let d = parse_with_hint(src.as_bytes(), &Limits::default(), Some(board)).unwrap();
        assert!(d.guessed);
        assert!(d.inch);
        assert!((d.holes[0].at.x - 25.4).abs() < 1e-9, "{:?}", d.holes[0]);
        assert!(
            d.warnings.iter().any(|w| w.contains("guessed")),
            "{:?}",
            d.warnings
        );
        let small = Rect::new(0.0, 0.0, 1.2, 1.2);
        let d = parse_with_hint(src.as_bytes(), &Limits::default(), Some(small)).unwrap();
        assert!(!d.inch);
    }

    #[test]
    fn patterns_circles_and_spaces() {
        let src = "M48\nINCH,TZ\nT01C0.050\nT02C0.035\n%\nT01\nM25\nX10000Y10000\nX20000Y10000\n\
                   T02\nX15000Y15000\nM01\nM02X25000Y0\nM02X0Y25000\nT01\nX 1000 Y 1000\n\
                   G32X15000Y10000\nM30\n";
        let d = parse(src.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(d.holes.len(), 3 * 3 + 1);
        assert_eq!(d.holes[4].at, Point::new(4.5 * 25.4, 1.0 * 25.4));
        assert_eq!(d.holes[4].tool, 0);
        assert_eq!(d.holes[5].tool, 1);
        let Segment::Arc { sweep, radius, .. } = d.slots[0].segment else {
            panic!("a circle");
        };
        assert!((sweep + 2.0 * PI).abs() < 1e-12);
        // From (0.1, 0.1) in to the centre (1.5, 1.0) in.
        assert!((radius - 1.4f64.hypot(0.9) * 25.4).abs() < 1e-9, "{radius}");
    }

    #[test]
    fn arcs_by_radius() {
        let src = "M48\nMETRIC\nT1C1.0\n%\nT1\nG00X0Y0\nM15\nG03X10.0Y0A5.0\nM16\nM30\n";
        let d = parse(src.as_bytes(), &Limits::default()).unwrap();
        let Segment::Arc { center, sweep, .. } = d.slots[0].segment else {
            panic!("an arc");
        };
        assert!(center.distance(Point::new(5.0, 0.0)) < 1e-9);
        assert!((sweep.abs() - PI).abs() < 1e-9);
    }
}
