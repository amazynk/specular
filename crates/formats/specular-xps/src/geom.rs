//! Geometry: the abbreviated path syntax, `PathGeometry` figures and segments, matrices.

use vello_cpu::kurbo::{Affine, Arc, BezPath, PathEl, Point, SvgArc, Vec2};
use vello_cpu::peniko::Fill;

/// Most path elements one geometry may hold.
pub const MAX_ELEMENTS: usize = 4_000_000;

/// A geometry ready to draw: what is filled and what is stroked (they differ when figures are
/// not filled or segments not stroked).
#[derive(Clone, Debug)]
pub struct Geom {
    pub fill: BezPath,
    /// `None`: stroke the fill path.
    pub stroke: Option<BezPath>,
    pub rule: Fill,
}

impl Geom {
    pub fn stroke_path(&self) -> &BezPath {
        self.stroke.as_ref().unwrap_or(&self.fill)
    }

    pub fn transformed(mut self, t: Affine) -> Geom {
        if t != Affine::IDENTITY {
            self.fill.apply_affine(t);
            if let Some(s) = &mut self.stroke {
                s.apply_affine(t);
            }
        }
        self
    }
}

/// Parses `n` comma/space-separated numbers.
pub fn numbers(s: &str) -> Vec<f64> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map_while(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect()
}

/// A `Point` attribute: `x,y`.
pub fn point(s: &str) -> Option<Point> {
    match numbers(s).as_slice() {
        [x, y] => Some(Point::new(*x, *y)),
        _ => None,
    }
}

/// A list of points: `x,y x,y …`.
pub fn points(s: &str) -> Vec<Point> {
    numbers(s)
        .chunks_exact(2)
        .map(|c| Point::new(c[0], c[1]))
        .collect()
}

/// A `Matrix` value: `m11,m12,m21,m22,dx,dy`.
pub fn matrix(s: &str) -> Option<Affine> {
    match numbers(s).as_slice() {
        [a, b, c, d, e, f] => Some(Affine::new([*a, *b, *c, *d, *e, *f])),
        _ => None,
    }
}

/// A `Rect`-like value: `x,y,w,h`.
pub fn rect4(s: &str) -> Option<[f64; 4]> {
    match numbers(s).as_slice() {
        [x, y, w, h] => Some([*x, *y, *w, *h]),
        _ => None,
    }
}

/// A builder that keeps a fill path and, when they diverge, a separate stroke path.
pub struct Builder {
    fill: BezPath,
    stroke: BezPath,
    split: bool,
    filled: bool,
    stroked: bool,
    start: Point,
    cur: Point,
    open: bool,
    count: usize,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            fill: BezPath::new(),
            stroke: BezPath::new(),
            split: false,
            filled: true,
            stroked: true,
            start: Point::ZERO,
            cur: Point::ZERO,
            open: false,
            count: 0,
        }
    }

    fn push_fill(&mut self, el: PathEl) {
        if self.filled {
            self.fill.push(el);
        }
    }

    fn push_stroke(&mut self, el: PathEl) {
        self.stroke.push(el);
    }

    pub fn full(&self) -> bool {
        self.count >= MAX_ELEMENTS
    }

    /// Starts a figure; `filled` false keeps it out of the fill.
    pub fn move_to(&mut self, p: Point, filled: bool) {
        self.count += 1;
        if !filled {
            self.split = true;
        }
        self.filled = filled;
        self.push_fill(PathEl::MoveTo(p));
        self.push_stroke(PathEl::MoveTo(p));
        self.start = p;
        self.cur = p;
        self.open = true;
    }

    fn ensure_open(&mut self) {
        if !self.open {
            let p = self.cur;
            self.move_to(p, self.filled);
        }
    }

    /// Whether the next segments are stroked.
    pub fn set_stroked(&mut self, s: bool) {
        if !s {
            self.split = true;
        }
        self.stroked = s;
    }

    fn seg(&mut self, el: PathEl, end: Point) {
        self.ensure_open();
        self.count += 1;
        self.push_fill(el);
        if self.stroked {
            self.push_stroke(el);
        } else {
            self.push_stroke(PathEl::MoveTo(end));
        }
        self.cur = end;
    }

    pub fn line_to(&mut self, p: Point) {
        self.seg(PathEl::LineTo(p), p);
    }

    pub fn quad_to(&mut self, c: Point, p: Point) {
        self.seg(PathEl::QuadTo(c, p), p);
    }

    pub fn curve_to(&mut self, c1: Point, c2: Point, p: Point) {
        self.seg(PathEl::CurveTo(c1, c2, p), p);
    }

    /// An elliptical arc to `p` (radii `r`, x-axis rotation `rot` degrees).
    pub fn arc_to(&mut self, r: Vec2, rot: f64, large: bool, sweep: bool, p: Point) {
        self.ensure_open();
        let svg = SvgArc {
            from: self.cur,
            to: p,
            radii: Vec2::new(r.x.abs(), r.y.abs()),
            x_rotation: rot.to_radians(),
            large_arc: large,
            sweep,
        };
        match Arc::from_svg_arc(&svg) {
            Some(arc) => {
                let tol = (arc.radii.x.max(arc.radii.y) * 1e-4).max(1e-3);
                let mut els = Vec::new();
                arc.to_cubic_beziers(tol, |a, b, c| els.push(PathEl::CurveTo(a, b, c)));
                for el in els {
                    let end = match el {
                        PathEl::CurveTo(_, _, e) => e,
                        _ => p,
                    };
                    self.seg(el, end);
                }
                // Land exactly on the end point.
                self.cur = p;
            }
            None => self.line_to(p),
        }
    }

    pub fn close(&mut self) {
        if !self.open {
            return;
        }
        self.count += 1;
        self.push_fill(PathEl::ClosePath);
        if self.stroked {
            self.push_stroke(PathEl::ClosePath);
        } else {
            self.push_stroke(PathEl::MoveTo(self.start));
        }
        self.cur = self.start;
        self.open = false;
    }

    pub fn current(&self) -> Point {
        self.cur
    }

    pub fn finish(self, rule: Fill) -> Geom {
        Geom {
            fill: self.fill,
            stroke: self.split.then_some(self.stroke),
            rule,
        }
    }
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',')
        {
            self.i += 1;
        }
    }

    fn command(&mut self) -> Option<u8> {
        self.skip();
        let c = *self.s.get(self.i)?;
        if c.is_ascii_alphabetic() && c != b'e' && c != b'E' {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    fn at_number(&mut self) -> bool {
        self.skip();
        self.s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
    }

    fn number(&mut self) -> Option<f64> {
        self.skip();
        let start = self.i;
        let s = self.s;
        let mut i = self.i;
        if i < s.len() && matches!(s[i], b'+' | b'-') {
            i += 1;
        }
        let mut digits = false;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
            digits = true;
        }
        if i < s.len() && s[i] == b'.' {
            i += 1;
            while i < s.len() && s[i].is_ascii_digit() {
                i += 1;
                digits = true;
            }
        }
        if !digits {
            return None;
        }
        if i < s.len() && matches!(s[i], b'e' | b'E') {
            let mut j = i + 1;
            if j < s.len() && matches!(s[j], b'+' | b'-') {
                j += 1;
            }
            if j < s.len() && s[j].is_ascii_digit() {
                while j < s.len() && s[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        self.i = i;
        std::str::from_utf8(&s[start..i])
            .ok()?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }

    fn pair(&mut self) -> Option<Point> {
        Some(Point::new(self.number()?, self.number()?))
    }
}

/// Parses the abbreviated geometry syntax into `b`; returns the fill rule it names (`F0` even-odd,
/// the default, or `F1` nonzero). Stops quietly at the first error, keeping what was read.
pub fn abbreviated(s: &str, b: &mut Builder) -> Fill {
    let mut lx = Lexer {
        s: s.as_bytes(),
        i: 0,
    };
    let mut rule = Fill::EvenOdd;
    let mut last_ctrl: Option<Point> = None;
    let mut cmd: Option<u8> = None;
    let mut first = true;
    loop {
        if b.full() {
            break;
        }
        let c = match lx.command() {
            Some(c) => c,
            None => {
                // Implicit repetition of the previous command.
                match cmd {
                    Some(c) if lx.at_number() => match c {
                        b'M' => b'L',
                        b'm' => b'l',
                        c => c,
                    },
                    _ => break,
                }
            }
        };
        let rel = c.is_ascii_lowercase();
        let base = if rel {
            b.current().to_vec2()
        } else {
            Vec2::ZERO
        };
        let ok = (|| -> Option<()> {
            match c.to_ascii_uppercase() {
                b'F' if first => {
                    rule = if lx.number()? == 1.0 {
                        Fill::NonZero
                    } else {
                        Fill::EvenOdd
                    };
                    last_ctrl = None;
                }
                b'M' => {
                    let p = lx.pair()? + base;
                    b.move_to(p, true);
                    last_ctrl = None;
                }
                b'L' => {
                    let p = lx.pair()? + base;
                    b.line_to(p);
                    last_ctrl = None;
                }
                b'H' => {
                    let x = lx.number()? + base.x;
                    let y = b.current().y;
                    b.line_to(Point::new(x, y));
                    last_ctrl = None;
                }
                b'V' => {
                    let y = lx.number()? + base.y;
                    let x = b.current().x;
                    b.line_to(Point::new(x, y));
                    last_ctrl = None;
                }
                b'C' => {
                    let c1 = lx.pair()? + base;
                    let c2 = lx.pair()? + base;
                    let p = lx.pair()? + base;
                    b.curve_to(c1, c2, p);
                    last_ctrl = Some(c2);
                }
                b'S' => {
                    let cur = b.current();
                    let c1 = match last_ctrl {
                        Some(c) => cur + (cur - c),
                        None => cur,
                    };
                    let c2 = lx.pair()? + base;
                    let p = lx.pair()? + base;
                    b.curve_to(c1, c2, p);
                    last_ctrl = Some(c2);
                }
                b'Q' => {
                    let c1 = lx.pair()? + base;
                    let p = lx.pair()? + base;
                    b.quad_to(c1, p);
                    last_ctrl = None;
                }
                b'A' => {
                    let r = lx.pair()?.to_vec2();
                    let rot = lx.number()?;
                    let large = lx.number()? != 0.0;
                    let sweep = lx.number()? != 0.0;
                    let p = lx.pair()? + base;
                    b.arc_to(r, rot, large, sweep, p);
                    last_ctrl = None;
                }
                b'Z' => {
                    b.close();
                    last_ctrl = None;
                }
                _ => return None,
            }
            Some(())
        })();
        if ok.is_none() {
            break;
        }
        first = false;
        cmd = if c.eq_ignore_ascii_case(&b'Z') || c.eq_ignore_ascii_case(&b'F') {
            None
        } else {
            Some(c)
        };
    }
    rule
}

/// A geometry from the abbreviated syntax alone.
pub fn parse_abbreviated(s: &str) -> Geom {
    let mut b = Builder::new();
    let rule = abbreviated(s, &mut b);
    b.finish(rule)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviated_syntax() {
        let g = parse_abbreviated("F1 M 10,10 L 20,10 20,20 h -10 z m 5 5 l 1 1");
        assert_eq!(g.rule, Fill::NonZero);
        let els = g.fill.elements();
        assert_eq!(els[0], PathEl::MoveTo(Point::new(10.0, 10.0)));
        assert_eq!(els[2], PathEl::LineTo(Point::new(20.0, 20.0)));
        assert_eq!(els[3], PathEl::LineTo(Point::new(10.0, 20.0)));
        assert_eq!(els[4], PathEl::ClosePath);
        assert_eq!(els[5], PathEl::MoveTo(Point::new(15.0, 15.0)));
        assert_eq!(els[6], PathEl::LineTo(Point::new(16.0, 16.0)));
        assert!(g.stroke.is_none());
        let g = parse_abbreviated("M0,0 A 10,10 0 1 1 20,0 C1,2 3,4 5,6 S 7,8 9,10 Q1,1 2,2");
        assert_eq!(g.rule, Fill::EvenOdd);
        assert!(g.fill.elements().len() > 4);
        let g = parse_abbreviated("M1e1,-2.5E-1L.5.5");
        assert_eq!(
            g.fill.elements()[0],
            PathEl::MoveTo(Point::new(10.0, -0.25))
        );
        assert_eq!(g.fill.elements()[1], PathEl::LineTo(Point::new(0.5, 0.5)));
        // Garbage stops quietly.
        let g = parse_abbreviated("M 0 0 L 1 X 3");
        assert_eq!(g.fill.elements().len(), 1);
    }

    #[test]
    fn matrices_and_points() {
        assert_eq!(
            matrix("1,0,0,1,5,6"),
            Some(Affine::new([1.0, 0.0, 0.0, 1.0, 5.0, 6.0]))
        );
        assert_eq!(matrix("1,0"), None);
        assert_eq!(
            points("1,2 3,4"),
            vec![Point::new(1.0, 2.0), Point::new(3.0, 4.0)]
        );
    }
}
