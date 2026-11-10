//! Custom shapes: `draw:enhanced-geometry` — equations (`draw:formula`), modifiers and the
//! enhanced path command language, evaluated into paths in the shape's box.

use std::f64::consts::PI;

use specular_drawpage::kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};

/// Most equations per shape.
const MAX_EQUATIONS: usize = 4096;
/// Deepest equation reference chain.
const MAX_DEPTH: u32 = 64;
/// Deepest parenthesis, sign and function nesting, over the whole chain of equations being
/// evaluated (each level is a few stack frames).
const MAX_NEST: u32 = 128;
/// Most parse steps per shape, over all its equations.
const MAX_STEPS: u32 = 200_000;
/// Most path commands/parameters per shape.
const MAX_TOKENS: usize = 1 << 16;

/// Values an equation may name.
#[derive(Debug, Clone, Copy, Default)]
pub struct Frame {
    /// The viewBox.
    pub view: Rect,
    /// Shape size in 1/100 mm.
    pub log_w: f64,
    pub log_h: f64,
    pub has_fill: bool,
    pub has_stroke: bool,
}

/// The equations and modifiers of one shape, with memoised values.
pub struct Eval<'a> {
    formulas: &'a [(String, String)],
    modifiers: &'a [f64],
    frame: Frame,
    cache: Vec<Option<f64>>,
    busy: Vec<bool>,
    /// Parse steps taken ([`MAX_STEPS`]) and the current nesting ([`MAX_NEST`]).
    steps: u32,
    nest: u32,
}

impl<'a> Eval<'a> {
    pub fn new(formulas: &'a [(String, String)], modifiers: &'a [f64], frame: Frame) -> Self {
        let n = formulas.len().min(MAX_EQUATIONS);
        Eval {
            formulas: &formulas[..n],
            modifiers,
            frame,
            cache: vec![None; n],
            busy: vec![false; n],
            steps: 0,
            nest: 0,
        }
    }

    fn equation(&mut self, i: usize, depth: u32) -> f64 {
        if i >= self.formulas.len() || depth > MAX_DEPTH || self.busy[i] {
            return 0.0;
        }
        if let Some(v) = self.cache[i] {
            return v;
        }
        self.busy[i] = true;
        let src = self.formulas[i].1.clone();
        let v = Parser {
            s: src.as_bytes(),
            i: 0,
            depth,
        }
        .expr(self)
        .unwrap_or(0.0);
        let v = if v.is_finite() { v } else { 0.0 };
        self.busy[i] = false;
        self.cache[i] = Some(v);
        v
    }

    /// `?name` → the equation's value.
    fn named(&mut self, name: &str, depth: u32) -> f64 {
        // Equations are usually named f0…fN in order; else look the name up.
        let idx = name
            .strip_prefix('f')
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|&k| self.formulas.get(k).is_some_and(|f| f.0 == name))
            .or_else(|| self.formulas.iter().position(|f| f.0 == name));
        idx.map_or(0.0, |k| self.equation(k, depth + 1))
    }

    /// One parse step, within [`MAX_STEPS`].
    fn tick(&mut self) -> Option<()> {
        self.steps += 1;
        (self.steps < MAX_STEPS).then_some(())
    }

    fn constant(&self, name: &str) -> Option<f64> {
        let f = &self.frame;
        Some(match name {
            "pi" => PI,
            "left" => f.view.x0,
            "top" => f.view.y0,
            "right" => f.view.x1,
            "bottom" => f.view.y1,
            "width" => f.view.width(),
            "height" => f.view.height(),
            "xstretch" | "ystretch" => 0.0,
            "hasstroke" => f64::from(u8::from(f.has_stroke)),
            "hasfill" => f64::from(u8::from(f.has_fill)),
            "logwidth" => f.log_w,
            "logheight" => f.log_h,
            _ => return None,
        })
    }

    /// One path or text-area parameter: a number, `?name`, `$n` or a constant.
    pub fn param(&mut self, tok: &str) -> f64 {
        let v = if let Some(n) = tok.strip_prefix('?') {
            self.named(n, 0)
        } else if let Some(n) = tok.strip_prefix('$') {
            n.parse::<usize>()
                .ok()
                .and_then(|k| self.modifiers.get(k).copied())
                .unwrap_or(0.0)
        } else if let Ok(v) = tok.parse::<f64>() {
            v
        } else {
            self.constant(tok).unwrap_or(0.0)
        };
        if v.is_finite() { v } else { 0.0 }
    }
}

struct Parser<'s> {
    s: &'s [u8],
    i: usize,
    depth: u32,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }

    /// Runs one nested level of the grammar, within [`MAX_NEST`] and [`MAX_STEPS`].
    fn nested(
        &mut self,
        e: &mut Eval,
        f: impl FnOnce(&mut Self, &mut Eval) -> Option<f64>,
    ) -> Option<f64> {
        e.steps += 1;
        if e.steps >= MAX_STEPS || e.nest >= MAX_NEST {
            return None;
        }
        e.nest += 1;
        let v = f(self, e);
        e.nest -= 1;
        v
    }

    fn expr(&mut self, e: &mut Eval) -> Option<f64> {
        self.nested(e, Self::expr_level)
    }

    fn expr_level(&mut self, e: &mut Eval) -> Option<f64> {
        let mut v = self.term(e)?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.i += 1;
                    v += self.term(e)?;
                }
                Some(b'-') => {
                    self.i += 1;
                    v -= self.term(e)?;
                }
                _ => return Some(v),
            }
        }
    }

    fn term(&mut self, e: &mut Eval) -> Option<f64> {
        e.tick()?;
        let mut v = self.unary(e)?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.i += 1;
                    v *= self.unary(e)?;
                }
                Some(b'/') => {
                    self.i += 1;
                    let d = self.unary(e)?;
                    v = if d == 0.0 { 0.0 } else { v / d };
                }
                _ => return Some(v),
            }
        }
    }

    fn unary(&mut self, e: &mut Eval) -> Option<f64> {
        self.nested(e, Self::unary_level)
    }

    fn unary_level(&mut self, e: &mut Eval) -> Option<f64> {
        match self.peek()? {
            b'-' => {
                self.i += 1;
                Some(-self.unary(e)?)
            }
            b'+' => {
                self.i += 1;
                self.unary(e)
            }
            _ => self.primary(e),
        }
    }

    fn primary(&mut self, e: &mut Eval) -> Option<f64> {
        e.tick()?;
        let c = self.peek()?;
        if c == b'(' {
            self.i += 1;
            let v = self.expr(e)?;
            if self.peek() == Some(b')') {
                self.i += 1;
            }
            return Some(v);
        }
        if c.is_ascii_digit() || c == b'.' {
            let st = self.i;
            while self
                .s
                .get(self.i)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
            {
                self.i += 1;
            }
            // Exponent.
            if matches!(self.s.get(self.i), Some(b'e' | b'E'))
                && self
                    .s
                    .get(self.i + 1)
                    .is_some_and(|b| b.is_ascii_digit() || *b == b'-' || *b == b'+')
            {
                self.i += 2;
                while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
                    self.i += 1;
                }
            }
            return std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok();
        }
        if c == b'?' || c == b'$' {
            self.i += 1;
            let st = self.i;
            while self.s.get(self.i).is_some_and(u8::is_ascii_alphanumeric) {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[st..self.i]).ok()?;
            return Some(if c == b'?' {
                e.named(name, self.depth)
            } else {
                e.param(&format!("${name}"))
            });
        }
        if c.is_ascii_alphabetic() {
            let st = self.i;
            while self.s.get(self.i).is_some_and(u8::is_ascii_alphanumeric) {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[st..self.i])
                .ok()?
                .to_ascii_lowercase();
            if self.peek() == Some(b'(') {
                self.i += 1;
                let mut args = Vec::new();
                loop {
                    if self.peek() == Some(b')') {
                        self.i += 1;
                        break;
                    }
                    args.push(self.expr(e)?);
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b')') => {
                            self.i += 1;
                            break;
                        }
                        _ => return None,
                    }
                    if args.len() > 8 {
                        return None;
                    }
                }
                let a = |k: usize| args.get(k).copied().unwrap_or(0.0);
                return Some(match name.as_str() {
                    "abs" => a(0).abs(),
                    "sqrt" => a(0).max(0.0).sqrt(),
                    "sin" => a(0).sin(),
                    "cos" => a(0).cos(),
                    "tan" => a(0).tan(),
                    "atan" => a(0).atan(),
                    "atan2" => a(0).atan2(a(1)),
                    "min" => a(0).min(a(1)),
                    "max" => a(0).max(a(1)),
                    "if" => {
                        if a(0) > 0.0 {
                            a(1)
                        } else {
                            a(2)
                        }
                    }
                    _ => 0.0,
                });
            }
            return Some(e.constant(&name).unwrap_or(0.0));
        }
        None
    }
}

/// One subpath group of an enhanced path (between `N` commands) and how it is painted.
pub struct Part {
    pub path: BezPath,
    pub fill: bool,
    pub stroke: bool,
}

/// Splits an enhanced path into command letters and parameter tokens.
fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        }
    };
    for ch in s.chars() {
        if out.len() >= MAX_TOKENS {
            break;
        }
        if ch.is_whitespace() || ch == ',' {
            flush(&mut cur, &mut out);
        } else if "MLCZNFSTUABWVXYQ".contains(ch) && !cur.starts_with(['?', '$']) {
            flush(&mut cur, &mut out);
            out.push(ch.to_string());
        } else if ((ch == '?' || ch == '$') && !cur.is_empty())
            || (ch == '-' && !cur.is_empty() && !cur.ends_with(['e', 'E']))
        {
            flush(&mut cur, &mut out);
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn is_command(t: &str) -> bool {
    t.len() == 1 && t.as_bytes()[0].is_ascii_uppercase()
}

/// A point on an ellipse at `deg` (counter-clockwise from 3 o'clock, y down).
fn on_ellipse(c: Point, r: Vec2, rad: f64) -> Point {
    Point::new(c.x + r.x * rad.cos(), c.y - r.y * rad.sin())
}

/// Appends an elliptical arc (centre `c`, radii `r`) from angle `a0` sweeping `sweep`
/// radians (positive counter-clockwise on screen), joined to the current point by a line
/// unless `move_first`.
fn arc(p: &mut BezPath, c: Point, r: Vec2, a0: f64, sweep: f64, move_first: bool) {
    let start = on_ellipse(c, r, a0);
    if move_first || p.elements().is_empty() {
        p.move_to(start);
    } else {
        p.line_to(start);
    }
    // kurbo's Arc sweeps clockwise on screen for positive angles in y-down space; express
    // our counter-clockwise angles by negating.
    let a = specular_drawpage::kurbo::Arc::new(c, r, -a0, -sweep, 0.0);
    a.to_cubic_beziers(0.1, |p1, p2, p3| p.curve_to(p1, p2, p3));
}

/// Angle of point `p` on the ellipse inscribed in `r` (counter-clockwise, y down).
fn angle_on(rect: Rect, p: Point) -> f64 {
    let c = rect.center();
    let (rx, ry) = (rect.width() / 2.0, rect.height() / 2.0);
    let dx = if rx.abs() > 1e-12 {
        (p.x - c.x) / rx
    } else {
        0.0
    };
    let dy = if ry.abs() > 1e-12 {
        (p.y - c.y) / ry
    } else {
        0.0
    };
    (-dy).atan2(dx)
}

/// Evaluates `path` (enhanced path syntax) in viewBox units; parts are mapped by `to_box`.
pub fn build(path: &str, ev: &mut Eval, to_box: Affine) -> Vec<Part> {
    let t = tokens(path);
    let mut parts = Vec::new();
    let mut cur = Part {
        path: BezPath::new(),
        fill: true,
        stroke: true,
    };
    let mut i = 0;
    let mut cmd = 'M';
    let mut last = Point::ZERO;
    let mut start = Point::ZERO;
    let mut x_first = true;
    while i < t.len() {
        if is_command(&t[i]) {
            cmd = t[i].chars().next().unwrap_or('M');
            i += 1;
            match cmd {
                'Z' => {
                    if !cur.path.elements().is_empty() {
                        cur.path.close_path();
                    }
                    last = start;
                    continue;
                }
                'N' => {
                    let done = std::mem::replace(
                        &mut cur,
                        Part {
                            path: BezPath::new(),
                            fill: true,
                            stroke: true,
                        },
                    );
                    if !done.path.elements().is_empty() {
                        parts.push(done);
                    }
                    continue;
                }
                'F' => {
                    cur.fill = false;
                    continue;
                }
                'S' => {
                    cur.stroke = false;
                    continue;
                }
                'X' => x_first = true,
                'Y' => x_first = false,
                _ => {}
            }
        }
        let need = match cmd {
            'M' | 'L' | 'X' | 'Y' => 2,
            'C' => 6,
            'Q' => 4,
            'T' | 'U' => 6,
            'A' | 'B' | 'W' | 'V' => 8,
            _ => {
                i += 1;
                continue;
            }
        };
        if i + need > t.len() || t[i..i + need].iter().any(|s| is_command(s)) {
            // Incomplete parameters: skip to the next command.
            while i < t.len() && !is_command(&t[i]) {
                i += 1;
            }
            continue;
        }
        let v: Vec<f64> = t[i..i + need].iter().map(|s| ev.param(s)).collect();
        i += need;
        let pt = |k: usize| Point::new(v[k], v[k + 1]);
        let p = &mut cur.path;
        match cmd {
            'M' => {
                p.move_to(pt(0));
                start = pt(0);
                last = pt(0);
                // Further pairs are line-tos.
                cmd = 'L';
            }
            'L' => {
                if p.elements().is_empty() {
                    p.move_to(last);
                    start = last;
                }
                p.line_to(pt(0));
                last = pt(0);
            }
            'C' => {
                if p.elements().is_empty() {
                    p.move_to(last);
                    start = last;
                }
                p.curve_to(pt(0), pt(2), pt(4));
                last = pt(4);
            }
            'Q' => {
                if p.elements().is_empty() {
                    p.move_to(last);
                    start = last;
                }
                p.quad_to(pt(0), pt(2));
                last = pt(2);
            }
            'T' | 'U' => {
                let c = pt(0);
                let r = Vec2::new(v[2], v[3]);
                // Angles here run clockwise on screen (y down), unlike the arcs below:
                // negate them for the counter-clockwise helpers.
                let (a0, a1) = (-v[4].to_radians(), -v[5].to_radians());
                let mut sweep = a1 - a0;
                if sweep >= 0.0 {
                    sweep -= 2.0 * PI;
                }
                if (v[5] - v[4]).abs() >= 360.0 {
                    sweep = -2.0 * PI;
                }
                let first = cmd == 'U' || p.elements().is_empty();
                arc(p, c, r, a0, sweep, first);
                if first {
                    start = on_ellipse(c, r, a0);
                }
                last = on_ellipse(c, r, a0 + sweep);
            }
            'A' | 'B' | 'W' | 'V' => {
                let rect = Rect::from_points(pt(0), pt(2));
                let (ps, pe) = (pt(4), pt(6));
                let a0 = angle_on(rect, ps);
                let a1 = angle_on(rect, pe);
                let ccw = matches!(cmd, 'A' | 'B');
                let mut sweep = a1 - a0;
                if ccw {
                    if sweep <= 0.0 {
                        sweep += 2.0 * PI;
                    }
                } else if sweep >= 0.0 {
                    sweep -= 2.0 * PI;
                }
                let first = matches!(cmd, 'B' | 'V') || p.elements().is_empty();
                let c = rect.center();
                let r = Vec2::new(rect.width() / 2.0, rect.height() / 2.0);
                arc(p, c, r, a0, sweep, first);
                if first {
                    start = on_ellipse(c, r, a0);
                }
                last = on_ellipse(c, r, a0 + sweep);
            }
            'X' | 'Y' => {
                if p.elements().is_empty() {
                    p.move_to(last);
                    start = last;
                }
                let to = pt(0);
                // A quarter ellipse: leaves `last` along x (X) or y (Y), arrives at `to`.
                const K: f64 = 0.552_284_749_8;
                let (c1, c2) = if x_first {
                    (
                        Point::new(last.x + (to.x - last.x) * K, last.y),
                        Point::new(to.x, to.y - (to.y - last.y) * K),
                    )
                } else {
                    (
                        Point::new(last.x, last.y + (to.y - last.y) * K),
                        Point::new(to.x - (to.x - last.x) * K, to.y),
                    )
                };
                p.curve_to(c1, c2, to);
                last = to;
                x_first = !x_first;
            }
            _ => {}
        }
    }
    if !cur.path.elements().is_empty() {
        parts.push(cur);
    }
    for p in &mut parts {
        p.path.apply_affine(to_box);
    }
    parts
}

/// The text area (first of `draw:text-areas`), viewBox units.
pub fn text_area(areas: &str, ev: &mut Eval) -> Option<Rect> {
    let t = tokens(areas);
    if t.len() < 4 {
        return None;
    }
    let v: Vec<f64> = t[..4].iter().map(|s| ev.param(s)).collect();
    Some(Rect::new(v[0], v[1], v[2], v[3]).abs())
}

/// The path of a preset shape by `draw:type` for files that store no enhanced path (the
/// common ones; the rest draw as their box).
pub fn preset(kind: &str, w: f64, h: f64) -> Option<BezPath> {
    let r = Rect::new(0.0, 0.0, w, h);
    Some(match kind {
        "rectangle" | "mso-spt1" | "flowchart-process" | "frame" => r.to_path(0.1),
        "ellipse" | "circle" | "mso-spt3" | "flowchart-connector" => {
            specular_drawpage::kurbo::Ellipse::from_rect(r).to_path(0.1)
        }
        "round-rectangle" | "mso-spt2" | "flowchart-alternate-process" => {
            r.to_rounded_rect(w.min(h) / 6.0).to_path(0.1)
        }
        "diamond" | "flowchart-decision" => {
            let mut p = BezPath::new();
            p.move_to((w / 2.0, 0.0));
            p.line_to((w, h / 2.0));
            p.line_to((w / 2.0, h));
            p.line_to((0.0, h / 2.0));
            p.close_path();
            p
        }
        "isosceles-triangle" => {
            let mut p = BezPath::new();
            p.move_to((w / 2.0, 0.0));
            p.line_to((w, h));
            p.line_to((0.0, h));
            p.close_path();
            p
        }
        "right-triangle" => {
            let mut p = BezPath::new();
            p.move_to((0.0, 0.0));
            p.line_to((w, h));
            p.line_to((0.0, h));
            p.close_path();
            p
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            view: Rect::new(0.0, 0.0, 21600.0, 21600.0),
            log_w: 6000.0,
            log_h: 4000.0,
            has_fill: true,
            has_stroke: true,
        }
    }

    #[test]
    fn formulas() {
        let f: Vec<(String, String)> = [
            ("f0", "$0 "),
            ("f1", "21600-$0 "),
            ("f2", "?f1 *2/4+(3-1)*-1"),
            ("f3", "max(?f0 ,?f1 )+abs(-2)+sqrt(16)"),
            ("f4", "if(?f0 -10000,1,2)+width/height+logwidth"),
            ("f5", "?f5 +1"),
            ("f6", "cos(pi)+atan2(0,1)+sin(0)"),
            ("f7", "1/0+right-left"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let m = [5400.0];
        let mut e = Eval::new(&f, &m, frame());
        assert_eq!(e.param("?f0"), 5400.0);
        assert_eq!(e.param("?f1"), 16200.0);
        assert_eq!(e.param("?f2"), 8100.0 - 2.0);
        assert_eq!(e.param("?f3"), 16200.0 + 2.0 + 4.0);
        assert_eq!(e.param("?f4"), 2.0 + 1.0 + 6000.0);
        // A self-reference evaluates to 0 instead of recursing.
        assert_eq!(e.param("?f5"), 1.0);
        assert_eq!(e.param("?f6"), -1.0);
        assert_eq!(e.param("?f7"), 21600.0);
        assert_eq!(e.param("$3"), 0.0);
        assert_eq!(e.param("bottom"), 21600.0);
        assert_eq!(e.param("?nope"), 0.0);
    }

    #[test]
    fn deep_chains_stop() {
        let f: Vec<(String, String)> = (0..3000)
            .map(|i| (format!("f{i}"), format!("?f{} +1", i + 1)))
            .collect();
        let mut e = Eval::new(&f, &[], frame());
        let v = e.param("?f0");
        assert!(v.is_finite() && v <= f64::from(MAX_DEPTH) + 2.0);
    }

    /// 100 000 nested parentheses or signs, alone and through a chain of equations: refused
    /// within [`MAX_NEST`] where they used to overflow the stack.
    #[test]
    fn deep_nesting_stops() {
        specular_thread::on_stack(256 * 1024, || {
            for src in [
                "(".repeat(100_000) + "1" + &")".repeat(100_000),
                "-".repeat(100_000) + "1",
                "sin(".repeat(100_000) + "1" + &")".repeat(100_000),
            ] {
                let f = vec![("f0".to_owned(), src)];
                let mut e = Eval::new(&f, &[], frame());
                assert_eq!(e.param("?f0"), 0.0);
            }
            let f: Vec<(String, String)> = (0..64)
                .map(|i| {
                    (
                        format!("f{i}"),
                        "(".repeat(60) + &format!("?f{}", i + 1) + &")".repeat(60),
                    )
                })
                .collect();
            let mut e = Eval::new(&f, &[], frame());
            assert!(e.param("?f0").is_finite());
        });
    }

    #[test]
    fn paths() {
        let f: Vec<(String, String)> = vec![("f0".into(), "$0".into())];
        let m = [5400.0];
        let mut e = Eval::new(&f, &m, frame());
        let k = 1.0 / 216.0;
        let parts = build(
            "M ?f0 0 L 21600 0 21600 21600 Z N U 10800 10800 10800 10800 0 360 F N",
            &mut e,
            Affine::scale(k),
        );
        assert_eq!(parts.len(), 2);
        assert!(parts[0].fill && parts[0].stroke);
        assert!(!parts[1].fill);
        let b = parts[0].path.bounding_box();
        assert!((b.x0 - 25.0).abs() < 1e-9 && (b.x1 - 100.0).abs() < 1e-9);
        let c = parts[1].path.bounding_box();
        assert!((c.width() - 100.0).abs() < 0.5 && (c.height() - 100.0).abs() < 0.5);
        // Arcs, quadrants, garbage.
        let parts = build(
            "M 0 10800 X 10800 0 Y 21600 10800 A 0 0 21600 21600 21600 10800 0 10800 W 0 0 1 1 Q 1 2 3 4 Z foo 1 2 3",
            &mut e,
            Affine::IDENTITY,
        );
        assert_eq!(parts.len(), 1);
        let b = parts[0].path.bounding_box();
        assert!(b.y0 < 100.0 && b.x1 > 21000.0, "{b:?}");
        assert!(build("C 1 2", &mut e, Affine::IDENTITY).is_empty());
    }

    #[test]
    fn tokenizer() {
        assert_eq!(
            tokens("M0-10L?f1?f2 $0,5 Z"),
            vec!["M", "0", "-10", "L", "?f1", "?f2", "$0", "5", "Z"]
        );
    }
}
