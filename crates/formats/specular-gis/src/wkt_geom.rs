//! Well-known text geometries (OGC simple features, ISO Z/M/ZM, EWKT `SRID=n;` prefix,
//! curves stroked): `POINT (1 2)`, `MULTIPOLYGON (((…)))`, `GEOMETRYCOLLECTION (…)`.

use crate::model::{Geometry, Pt, stroke_circular};

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn word(&mut self) -> String {
        self.ws();
        let st = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[st..self.i]).to_ascii_uppercase()
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(format!("WKT: expected '{}'", c as char))
        }
    }
    fn num(&mut self) -> Result<f64, String> {
        self.ws();
        let st = self.i;
        while self.i < self.s.len()
            && matches!(self.s[self.i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' | b'a'..=b'z' | b'A'..=b'Z')
        {
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.s[st..self.i]).map_err(|_| "WKT: bad number")?;
        t.parse::<f64>()
            .map_err(|_| format!("WKT: bad number {t:?}"))
    }
    /// "EMPTY" or an opening parenthesis: whether the item is empty.
    fn open(&mut self) -> Result<bool, String> {
        let save = self.i;
        if self.word() == "EMPTY" {
            return Ok(true);
        }
        self.i = save;
        self.expect(b'(')?;
        Ok(false)
    }
    fn coord(&mut self) -> Result<Pt, String> {
        let x = self.num()?;
        let y = self.num()?;
        // Further ordinates (Z, M) until a comma or a parenthesis.
        loop {
            self.ws();
            match self.s.get(self.i) {
                Some(b',' | b')') | None => break,
                _ => {
                    self.num()?;
                }
            }
        }
        Ok([x, y])
    }
    fn points(&mut self) -> Result<Vec<Pt>, String> {
        if self.open()? {
            return Ok(Vec::new());
        }
        let mut v = Vec::new();
        loop {
            // MULTIPOINT ((1 2), (3 4)) and MULTIPOINT (1 2, 3 4).
            if self.eat(b'(') {
                v.push(self.coord()?);
                self.expect(b')')?;
            } else {
                v.push(self.coord()?);
            }
            if !self.eat(b',') {
                break;
            }
        }
        self.expect(b')')?;
        Ok(v)
    }
    fn list<T>(
        &mut self,
        mut f: impl FnMut(&mut Self) -> Result<T, String>,
    ) -> Result<Vec<T>, String> {
        if self.open()? {
            return Ok(Vec::new());
        }
        let mut v = Vec::new();
        loop {
            v.push(f(self)?);
            if !self.eat(b',') {
                break;
            }
        }
        self.expect(b')')?;
        Ok(v)
    }
    /// A curve member: a bare point list (a line), or a tagged curve.
    fn curve(&mut self, depth: usize) -> Result<Vec<Pt>, String> {
        self.ws();
        if self.s.get(self.i) == Some(&b'(') {
            return self.points();
        }
        match self.geometry(depth + 1)? {
            Geometry::LineString(l) => Ok(l),
            Geometry::MultiLineString(l) => Ok(l.into_iter().flatten().collect()),
            _ => Err("WKT: not a curve".into()),
        }
    }
    fn geometry(&mut self, depth: usize) -> Result<Geometry, String> {
        if depth > 32 {
            return Err("WKT nests too deep".into());
        }
        let mut name = self.word();
        // ISO dimension suffixes as separate words.
        let save = self.i;
        match self.word().as_str() {
            "Z" | "M" | "ZM" => {}
            _ => self.i = save,
        }
        for suf in ["ZM", "Z", "M"] {
            if name.len() > suf.len()
                && name.ends_with(suf)
                && !matches!(name.as_str(), "POLYHEDRALSURFACE" | "MULTISURFACE")
            {
                let base = &name[..name.len() - suf.len()];
                if KNOWN.contains(&base) {
                    name = base.to_string();
                    break;
                }
            }
        }
        Ok(match name.as_str() {
            "POINT" => {
                let v = self.points()?;
                match v.first() {
                    Some(p) => Geometry::Point(*p),
                    None => Geometry::MultiPoint(Vec::new()),
                }
            }
            "MULTIPOINT" => Geometry::MultiPoint(self.points()?),
            "LINESTRING" => Geometry::LineString(self.points()?),
            "CIRCULARSTRING" => Geometry::LineString(stroke_circular(&self.points()?)),
            "COMPOUNDCURVE" => {
                let parts = self.list(|p| p.curve(depth))?;
                let mut line: Vec<Pt> = Vec::new();
                for c in parts {
                    let skip = usize::from(!line.is_empty() && line.last() == c.first());
                    line.extend_from_slice(&c[skip.min(c.len())..]);
                }
                Geometry::LineString(line)
            }
            "POLYGON" | "TRIANGLE" => Geometry::Polygon(self.list(|p| p.points())?),
            "CURVEPOLYGON" => Geometry::Polygon(self.list(|p| p.curve(depth))?),
            "MULTILINESTRING" | "MULTICURVE" => {
                Geometry::MultiLineString(self.list(|p| p.curve(depth))?)
            }
            "MULTIPOLYGON" | "POLYHEDRALSURFACE" | "TIN" => {
                Geometry::MultiPolygon(self.list(|p| p.list(|q| q.points()))?)
            }
            "MULTISURFACE" => Geometry::MultiPolygon(self.list(|p| {
                p.ws();
                if p.s.get(p.i) == Some(&b'(') {
                    return p.list(|q| q.points());
                }
                match p.geometry(depth + 1)? {
                    Geometry::Polygon(r) => Ok(r),
                    _ => Err("WKT: not a surface".into()),
                }
            })?),
            "GEOMETRYCOLLECTION" => Geometry::Collection(self.list(|p| p.geometry(depth + 1))?),
            n => return Err(format!("WKT: unknown geometry {n:?}")),
        })
    }
}

const KNOWN: &[&str] = &[
    "POINT",
    "MULTIPOINT",
    "LINESTRING",
    "MULTILINESTRING",
    "POLYGON",
    "MULTIPOLYGON",
    "GEOMETRYCOLLECTION",
    "CIRCULARSTRING",
    "COMPOUNDCURVE",
    "CURVEPOLYGON",
    "MULTICURVE",
    "MULTISURFACE",
    "TRIANGLE",
    "TIN",
    "POLYHEDRALSURFACE",
];

/// Parses a WKT geometry.
pub fn parse(text: &str) -> Result<Geometry, String> {
    let t = text.trim();
    let t = match t.find(';') {
        Some(i) if t[..i].trim().to_ascii_uppercase().starts_with("SRID") => &t[i + 1..],
        _ => t,
    };
    let mut p = P {
        s: t.as_bytes(),
        i: 0,
    };
    let g = p.geometry(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err("WKT: text after the geometry".into());
    }
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        assert_eq!(parse("POINT (1 2)").unwrap(), Geometry::Point([1.0, 2.0]));
        assert_eq!(
            parse("point z (1 2 3)").unwrap(),
            Geometry::Point([1.0, 2.0])
        );
        assert_eq!(
            parse("POINTZM (1 2 3 4)").unwrap(),
            Geometry::Point([1.0, 2.0])
        );
        assert_eq!(
            parse("SRID=4326;MULTIPOINT ((1 2), (3 4))").unwrap(),
            Geometry::MultiPoint(vec![[1.0, 2.0], [3.0, 4.0]])
        );
        assert_eq!(
            parse("MULTIPOINT (1 2, 3 4)").unwrap(),
            Geometry::MultiPoint(vec![[1.0, 2.0], [3.0, 4.0]])
        );
        assert_eq!(
            parse("POLYGON ((0 0, 1 0, 1 1, 0 0), (0.2 0.2, 0.3 0.2, 0.3 0.3, 0.2 0.2))").unwrap(),
            Geometry::Polygon(vec![
                vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0]],
                vec![[0.2, 0.2], [0.3, 0.2], [0.3, 0.3], [0.2, 0.2]]
            ])
        );
        let g = parse("MULTIPOLYGON (((0 0, 1 0, 1 1, 0 0)), EMPTY)").unwrap();
        assert!(matches!(g, Geometry::MultiPolygon(ref p) if p.len() == 2));
        let g = parse("GEOMETRYCOLLECTION (POINT (1 2), LINESTRING EMPTY)").unwrap();
        assert!(matches!(g, Geometry::Collection(ref c) if c.len() == 2));
        assert_eq!(parse("POINT EMPTY").unwrap(), Geometry::MultiPoint(vec![]));
        assert!(parse("POINT (1)").is_err());
        assert!(parse("POINT (1 2) x").is_err());
        assert!(parse("BLOB (1 2)").is_err());
        let c = parse("COMPOUNDCURVE ((0 0, 1 0), CIRCULARSTRING (1 0, 2 1, 3 0))").unwrap();
        let Geometry::LineString(l) = c else { panic!() };
        assert_eq!(l[0], [0.0, 0.0]);
        assert_eq!(*l.last().unwrap(), [3.0, 0.0]);
    }
}
