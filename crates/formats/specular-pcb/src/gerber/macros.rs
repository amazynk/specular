//! Aperture macros (`%AM`): primitives, variables and arithmetic expressions.
//!
//! A macro instance is one outline path: primitives with exposure on are outlines, those
//! with exposure off are holes, so a non-zero fill shows the aperture (an off primitive
//! punches through the on primitives it lies on; where two on primitives overlap it does
//! not, which Gerber files in the wild do not do).

use std::f64::consts::PI;

use specular_vector::kurbo::{Affine, BezPath, Point, Vec2};

use crate::geom;

/// An arithmetic expression of a macro body.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Expr {
    Num(f64),
    Var(usize),
    Neg(Box<Expr>),
    Bin(Box<Expr>, u8, Box<Expr>),
}

impl Expr {
    fn eval(&self, vars: &[f64]) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::Var(i) => vars.get(*i).copied().unwrap_or(0.0),
            Expr::Neg(e) => -e.eval(vars),
            Expr::Bin(a, op, b) => {
                let (a, b) = (a.eval(vars), b.eval(vars));
                match op {
                    b'+' => a + b,
                    b'-' => a - b,
                    b'x' => a * b,
                    _ => {
                        if b == 0.0 {
                            0.0
                        } else {
                            a / b
                        }
                    }
                }
            }
        }
    }
}

struct ExprParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl ExprParser<'_> {
    fn peek(&mut self) -> Option<u8> {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
        self.s.get(self.i).copied()
    }

    fn sum(&mut self) -> Option<Expr> {
        let mut a = self.product()?;
        while let Some(c @ (b'+' | b'-')) = self.peek() {
            self.i += 1;
            let b = self.product()?;
            a = Expr::Bin(Box::new(a), c, Box::new(b));
        }
        Some(a)
    }

    fn product(&mut self) -> Option<Expr> {
        let mut a = self.unary()?;
        while let Some(c @ (b'x' | b'X' | b'/')) = self.peek() {
            self.i += 1;
            let b = self.unary()?;
            let op = if c == b'/' { b'/' } else { b'x' };
            a = Expr::Bin(Box::new(a), op, Box::new(b));
        }
        Some(a)
    }

    fn unary(&mut self) -> Option<Expr> {
        match self.peek()? {
            b'-' => {
                self.i += 1;
                Some(Expr::Neg(Box::new(self.unary()?)))
            }
            b'+' => {
                self.i += 1;
                self.unary()
            }
            b'(' => {
                self.i += 1;
                let e = self.sum()?;
                if self.peek() == Some(b')') {
                    self.i += 1;
                }
                Some(e)
            }
            b'$' => {
                self.i += 1;
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                let n: usize = std::str::from_utf8(&self.s[st..self.i])
                    .ok()?
                    .parse()
                    .ok()?;
                Some(Expr::Var(n))
            }
            c if c.is_ascii_digit() || c == b'.' => {
                let st = self.i;
                while self.i < self.s.len()
                    && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.')
                {
                    self.i += 1;
                }
                let v: f64 = std::str::from_utf8(&self.s[st..self.i])
                    .ok()?
                    .parse()
                    .ok()?;
                Some(Expr::Num(v))
            }
            _ => None,
        }
    }
}

/// Parses one expression; `None` when it is not one.
pub(crate) fn parse_expr(s: &str) -> Option<Expr> {
    let mut p = ExprParser {
        s: s.as_bytes(),
        i: 0,
    };
    let e = p.sum()?;
    (p.peek().is_none()).then_some(e)
}

#[derive(Clone, Debug, PartialEq)]
enum Stmt {
    /// `$n=expr`.
    Assign(usize, Expr),
    /// A primitive code and its parameters.
    Prim(u32, Vec<Expr>),
}

/// A macro definition.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Macro {
    body: Vec<Stmt>,
}

impl Macro {
    /// Parses the statements after `AMname*` (each without its `*`). Statements that are not
    /// understood are skipped and returned as warnings.
    pub(crate) fn parse(stmts: &[&str]) -> (Macro, Vec<String>) {
        let mut body = Vec::new();
        let mut warns = Vec::new();
        for s in stmts {
            // Primitive 0 is a comment.
            if s.trim_start().starts_with('0') {
                continue;
            }
            let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            if s.is_empty() {
                continue;
            }
            if let Some(rest) = s.strip_prefix('$')
                && let Some((n, e)) = rest.split_once('=')
            {
                match (n.parse::<usize>(), parse_expr(e)) {
                    (Ok(n), Some(e)) => body.push(Stmt::Assign(n, e)),
                    _ => warns.push(format!("aperture macro statement not understood: {s}")),
                }
                continue;
            }
            let mut parts = s.split(',');
            let code = parts.next().and_then(|c| c.parse::<u32>().ok());
            let params: Option<Vec<Expr>> = parts.map(parse_expr).collect();
            match (code, params) {
                (Some(c), Some(p)) => body.push(Stmt::Prim(c, p)),
                _ => warns.push(format!("aperture macro statement not understood: {s}")),
            }
        }
        (Macro { body }, warns)
    }

    /// The aperture for these parameters, in the macro's units scaled by `scale` (to mm).
    pub(crate) fn instance(&self, params: &[f64], scale: f64, warns: &mut Vec<String>) -> BezPath {
        let mut vars = vec![0.0; 1];
        vars.extend_from_slice(params);
        let mut out = BezPath::new();
        for st in &self.body {
            match st {
                Stmt::Assign(n, e) => {
                    let v = e.eval(&vars);
                    if *n < 10_000 {
                        if vars.len() <= *n {
                            vars.resize(n + 1, 0.0);
                        }
                        vars[*n] = v;
                    }
                }
                Stmt::Prim(code, ps) => {
                    let v: Vec<f64> = ps.iter().map(|e| e.eval(&vars)).collect();
                    primitive(*code, &v, scale, &mut out, warns);
                }
            }
        }
        out
    }
}

fn arg(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

/// Adds primitive `code` with values `v` to `out`.
fn primitive(code: u32, v: &[f64], k: f64, out: &mut BezPath, warns: &mut Vec<String>) {
    // Rotation about the macro's origin, degrees counter-clockwise.
    let rot = |i: usize| Affine::rotate(arg(v, i).to_radians());
    let p = |x: f64, y: f64| Point::new(x * k, y * k);
    let (hole, shape) = match code {
        1 => {
            let s = geom::circle(p(arg(v, 2), arg(v, 3)), arg(v, 1) * k / 2.0);
            (arg(v, 0) == 0.0, geom::transformed(&s, rot(4)))
        }
        2 | 20 => {
            let (a, b) = (p(arg(v, 2), arg(v, 3)), p(arg(v, 4), arg(v, 5)));
            let w = arg(v, 1) * k / 2.0;
            let d = b - a;
            let len = d.hypot();
            if len < geom::EPS || w <= 0.0 {
                return;
            }
            let n = Vec2::new(-d.y, d.x) / len * w;
            let s = geom::polygon(&[a + n, b + n, b - n, a - n]);
            (arg(v, 0) == 0.0, geom::transformed(&s, rot(6)))
        }
        21 => {
            let s = geom::rect(p(arg(v, 3), arg(v, 4)), arg(v, 1) * k, arg(v, 2) * k);
            (arg(v, 0) == 0.0, geom::transformed(&s, rot(5)))
        }
        22 => {
            let (w, h) = (arg(v, 1) * k, arg(v, 2) * k);
            let ll = p(arg(v, 3), arg(v, 4));
            let s = geom::rect(ll + Vec2::new(w / 2.0, h / 2.0), w, h);
            (arg(v, 0) == 0.0, geom::transformed(&s, rot(5)))
        }
        4 => {
            let n = arg(v, 1).max(0.0) as usize;
            if n < 2 || v.len() < 2 + 2 * (n + 1) {
                warns.push("an outline macro primitive has too few points".into());
                return;
            }
            let pts: Vec<Point> = (0..=n).map(|i| p(v[2 + 2 * i], v[3 + 2 * i])).collect();
            let s = geom::polygon(&pts);
            (
                arg(v, 0) == 0.0,
                geom::transformed(&s, rot(2 + 2 * (n + 1))),
            )
        }
        5 => {
            let n = arg(v, 1).max(3.0) as usize;
            let s = geom::regular(p(arg(v, 2), arg(v, 3)), arg(v, 4) * k, n, 0.0);
            (arg(v, 0) == 0.0, geom::transformed(&s, rot(5)))
        }
        6 => {
            moire(v, k, out);
            return;
        }
        7 => {
            thermal(v, k, out);
            return;
        }
        _ => {
            warns.push(format!(
                "aperture macro primitive {code} is not a Gerber primitive"
            ));
            return;
        }
    };
    // An exposure-off primitive erases only what the macro drew before it: where it lies on
    // nothing it would add a hole of its own (negative winding), so one wholly off the
    // primitives drawn so far is left out.
    if hole && !covers_any(out, &shape) {
        return;
    }
    geom::push(out, shape, hole);
}

/// Whether some point inside `shape` (its vertices drawn a little towards its centre, and
/// the centre) lies on what `drawn` fills.
fn covers_any(drawn: &BezPath, shape: &BezPath) -> bool {
    use specular_vector::kurbo::Shape;
    if drawn.elements().is_empty() {
        return false;
    }
    let pts = geom::sample(shape);
    if pts.is_empty() {
        return false;
    }
    let n = pts.len() as f64;
    let c = pts.iter().fold(Point::ORIGIN, |a, p| a + p.to_vec2() / n);
    std::iter::once(c)
        .chain(pts.iter().map(|p| c + (*p - c) * 0.9))
        .any(|q| drawn.winding(q) != 0)
}

/// Primitive 6 (deprecated): rings and a cross hair.
fn moire(v: &[f64], k: f64, out: &mut BezPath) {
    let c = Point::new(arg(v, 0) * k, arg(v, 1) * k);
    let (outer, ring, gap) = (arg(v, 2) * k, arg(v, 3) * k, arg(v, 4) * k);
    let max = arg(v, 5).clamp(0.0, 100.0) as usize;
    let (cross_w, cross_l) = (arg(v, 6) * k, arg(v, 7) * k);
    let rot = Affine::rotate(arg(v, 8).to_radians());
    let mut d = outer;
    for _ in 0..max {
        if d <= 0.0 {
            break;
        }
        let mut r = BezPath::new();
        geom::push(&mut r, geom::circle(c, d / 2.0), false);
        let inner = d - 2.0 * ring;
        if inner > 0.0 {
            geom::push(&mut r, geom::circle(c, inner / 2.0), true);
        }
        for el in geom::transformed(&r, rot).elements() {
            out.push(*el);
        }
        d = inner - 2.0 * gap;
    }
    if cross_w > 0.0 && cross_l > 0.0 {
        for (w, h) in [(cross_l, cross_w), (cross_w, cross_l)] {
            geom::push(out, geom::transformed(&geom::rect(c, w, h), rot), false);
        }
    }
}

/// Primitive 7: a ring cut into four by a cross-shaped gap.
fn thermal(v: &[f64], k: f64, out: &mut BezPath) {
    let c = Point::new(arg(v, 0) * k, arg(v, 1) * k);
    let ro = arg(v, 2) * k / 2.0;
    let ri = (arg(v, 3) * k / 2.0).max(0.0);
    let g = arg(v, 4) * k / 2.0;
    let rot = Affine::rotate(arg(v, 5).to_radians());
    if ro <= g || ro <= ri {
        return;
    }
    for q in 0..4 {
        // The first quadrant's piece, turned to quadrant q about the centre.
        let mut s = BezPath::new();
        let a0 = (g / ro).asin();
        let outer_start = Point::new(ro * a0.cos(), g);
        s.move_to(outer_start);
        let arc = specular_vector::kurbo::Arc::new(
            Point::ORIGIN,
            Vec2::new(ro, ro),
            a0,
            PI / 2.0 - 2.0 * a0,
            0.0,
        );
        for el in arc.append_iter(geom::FLAT * 0.1) {
            s.push(el);
        }
        if ri > g {
            let b0 = (g / ri).asin();
            s.line_to(Point::new(g, ri * b0.cos()));
            let arc = specular_vector::kurbo::Arc::new(
                Point::ORIGIN,
                Vec2::new(ri, ri),
                PI / 2.0 - b0,
                -(PI / 2.0 - 2.0 * b0),
                0.0,
            );
            for el in arc.append_iter(geom::FLAT * 0.1) {
                s.push(el);
            }
        } else {
            s.line_to(Point::new(g, g));
        }
        s.close_path();
        let a = rot * Affine::translate(c.to_vec2()) * Affine::rotate(PI / 2.0 * f64::from(q));
        geom::push(out, geom::transformed(&s, a), false);
    }
}

#[cfg(test)]
mod tests {
    use specular_vector::kurbo::Shape;

    use super::*;

    #[test]
    fn expressions_follow_precedence() {
        let e = parse_expr("1+2x3").unwrap();
        assert_eq!(e.eval(&[]), 7.0);
        let e = parse_expr("-$1x2+($2-1)/2").unwrap();
        assert_eq!(e.eval(&[0.0, 3.0, 5.0]), -4.0);
        let e = parse_expr("$1X$1").unwrap();
        assert_eq!(e.eval(&[0.0, 1.5]), 2.25);
        assert!(parse_expr("1+").is_none());
        assert!(parse_expr("1 2").is_none());
    }

    #[test]
    fn donut_and_variables() {
        let (m, w) = Macro::parse(&["0 a donut", "$3=$1x0.5", "1,1,$1,0,0", "1,0,$3,0,0"]);
        assert!(w.is_empty(), "{w:?}");
        let p = m.instance(&[2.0], 1.0, &mut Vec::new());
        let want = PI * (1.0 - 0.25);
        assert!((p.area() - want).abs() < 1e-3, "{}", p.area());
    }

    #[test]
    fn primitives_have_their_areas() {
        let area = |body: &str, params: &[f64]| {
            let (m, w) = Macro::parse(&[body]);
            assert!(w.is_empty(), "{w:?}");
            m.instance(params, 1.0, &mut Vec::new()).area()
        };
        assert!((area("20,1,0.5,0,0,4,0,30", &[]) - 2.0).abs() < 1e-9);
        assert!((area("21,1,2,3,5,5,45", &[]) - 6.0).abs() < 1e-9);
        assert!((area("22,1,2,3,0,0,0", &[]) - 6.0).abs() < 1e-9);
        assert!((area("4,1,3,0,0,1,0,1,1,0,0,0", &[]) - 0.5).abs() < 1e-9);
        assert!((area("5,1,4,0,0,2,0", &[]) - 2.0).abs() < 1e-9);
        // Thermal: ring 1..2 radius minus a 0.2-wide cross.
        let t = area("7,0,0,4,2,0.2,0", &[]);
        assert!(t > 7.0 && t < PI * 3.0, "{t}");
    }
}
