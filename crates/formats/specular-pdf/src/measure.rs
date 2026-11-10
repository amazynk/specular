//! Drawing scales of a page: ISO 32000 §12.9 viewports (`/VP`) whose `/Measure` is
//! rectilinear (`/Subtype /RL`, the default): the scale ratio text (`/R`), the number format
//! arrays for x, y, distance and area, and the y-to-x conversion (`/CYX`). Read as given;
//! the caller maps the viewport's box onto its pixels.

use hayro::hayro_syntax::object::{Array, Dict, Name, Object};

use crate::Document;

/// One number format dictionary (`/Type /NumberFormat`), as written.
#[derive(Clone, Debug, PartialEq)]
pub struct NumFmt {
    /// `/U`.
    pub unit: String,
    /// `/C`.
    pub factor: f64,
    /// `/F`: `D` decimals, `F` fraction, `R` round, `T` truncate.
    pub fraction: char,
    /// `/D` (100 when absent).
    pub precision: u32,
    /// `/FD`: fractions are not reduced.
    pub no_reduce: bool,
    /// `/RT`, `/RD`, `/PS`, `/SS` (`None` when absent: the defaults apply).
    pub thousands: Option<String>,
    pub decimal: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    /// `/O /P`.
    pub prefix: bool,
}

/// A viewport with a rectilinear measure.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaleViewport {
    /// Its position in the page's `/VP` array (0-based).
    pub index: usize,
    /// Its box in user space (x0, y0, x1, y1), ordered.
    pub bbox: [f64; 4],
    pub name: Option<String>,
    /// `/R`, e.g. `1:50` or `1 in = 4 ft`.
    pub ratio: Option<String>,
    pub x: Vec<NumFmt>,
    pub y: Vec<NumFmt>,
    pub distance: Vec<NumFmt>,
    pub area: Vec<NumFmt>,
    /// `/CYX`.
    pub cyx: Option<f64>,
    /// `/O`: the origin in user space, when given.
    pub origin: Option<[f64; 2]>,
}

fn text(o: &Object<'_>) -> Option<String> {
    match o {
        Object::String(s) => crate::decode_text_string(s),
        Object::Name(n) => Some(n.as_str().to_string()),
        _ => None,
    }
}

fn num(o: &Object<'_>) -> Option<f64> {
    match o {
        Object::Number(n) => Some(n.as_f64()),
        _ => None,
    }
}

fn numfmt(d: &Dict<'_>) -> Option<NumFmt> {
    let factor = d.get::<Object<'_>>(b"C").and_then(|o| num(&o))?;
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let s = |k: &[u8]| d.get::<Object<'_>>(k).and_then(|o| text(&o));
    Some(NumFmt {
        unit: s(b"U").unwrap_or_default(),
        factor,
        fraction: d
            .get::<Name<'_>>(b"F")
            .and_then(|n| n.as_str().chars().next())
            .filter(|c| matches!(c, 'D' | 'F' | 'R' | 'T'))
            .unwrap_or('D'),
        precision: d
            .get::<Object<'_>>(b"D")
            .and_then(|o| num(&o))
            .filter(|v| v.is_finite() && *v >= 1.0 && *v <= 1e9)
            .map_or(100, |v| v as u32),
        no_reduce: d.get::<bool>(b"FD").unwrap_or(false),
        thousands: s(b"RT"),
        decimal: s(b"RD"),
        before: s(b"PS"),
        after: s(b"SS"),
        prefix: d.get::<Name<'_>>(b"O").is_some_and(|n| n.as_str() == "P"),
    })
}

/// A number format array; empty when absent or when any element is unusable.
fn array(m: &Dict<'_>, key: &[u8]) -> Vec<NumFmt> {
    let Some(a) = m.get::<Array<'_>>(key) else {
        return Vec::new();
    };
    let v: Vec<Option<NumFmt>> = a.iter::<Dict<'_>>().map(|d| numfmt(&d)).collect();
    if v.iter().any(Option::is_none) {
        return Vec::new();
    }
    v.into_iter().flatten().collect()
}

fn viewport(index: usize, vp: &Dict<'_>) -> Option<ScaleViewport> {
    let m = vp.get::<Dict<'_>>(b"Measure")?;
    if m.get::<Name<'_>>(b"Subtype")
        .is_some_and(|n| n.as_str() != "RL")
    {
        return None;
    }
    let bbox = vp.get::<Vec<f64>>(b"BBox")?;
    let [a, b, c, d]: [f64; 4] = bbox.try_into().ok()?;
    if ![a, b, c, d].iter().all(|v| v.is_finite()) {
        return None;
    }
    let x = array(&m, b"X");
    if x.is_empty() {
        return None;
    }
    Some(ScaleViewport {
        index,
        bbox: [a.min(c), b.min(d), a.max(c), b.max(d)],
        name: vp.get::<Object<'_>>(b"Name").and_then(|o| text(&o)),
        ratio: m
            .get::<Object<'_>>(b"R")
            .and_then(|o| text(&o))
            .filter(|r| !r.trim().is_empty()),
        y: array(&m, b"Y"),
        distance: array(&m, b"D"),
        area: array(&m, b"A"),
        cyx: m
            .get::<Object<'_>>(b"CYX")
            .and_then(|o| num(&o))
            .filter(|v| v.is_finite() && *v > 0.0),
        origin: m
            .get::<Vec<f64>>(b"O")
            .and_then(|v| <[f64; 2]>::try_from(v).ok()),
        x,
    })
}

impl Document {
    /// The viewports of page `index` with a rectilinear measure, in `/VP` order.
    pub fn scale_viewports(&self, index: usize) -> Vec<ScaleViewport> {
        let Some(page) = self.pdf.pages().get(index) else {
            return Vec::new();
        };
        let Some(vps) = page.raw().get::<Array<'_>>(b"VP") else {
            return Vec::new();
        };
        vps.iter::<Dict<'_>>()
            .enumerate()
            .filter_map(|(i, vp)| viewport(i, &vp))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectilinear_viewports_are_read() {
        let doc = Document::open(std::sync::Arc::new(crate::fixture::drawing_scales())).unwrap();
        let vps = doc.scale_viewports(0);
        // The GEO viewport is not a drawing scale.
        assert_eq!(vps.len(), 2);
        let (a, b) = (&vps[0], &vps[1]);
        assert_eq!((a.index, b.index), (0, 1));
        assert_eq!(a.bbox, [50.0, 50.0, 450.0, 550.0]);
        // Reversed corners come back ordered.
        assert_eq!(b.bbox, [500.0, 300.0, 750.0, 550.0]);
        assert_eq!(a.name.as_deref(), Some("Plan"));
        assert_eq!(a.ratio.as_deref(), Some("1:50"));
        assert_eq!(b.ratio.as_deref(), Some("1 in = 4 ft"));
        assert_eq!(a.x[0].unit, "m");
        assert!((a.x[0].factor - crate::fixture::M_PER_PT_1_50).abs() < 1e-12);
        assert_eq!(a.distance.len(), 1);
        assert_eq!(a.area[0].unit, "sq m");
        assert_eq!(a.x[0].precision, 100);
        assert_eq!(
            (b.x.len(), b.x[1].unit.as_str(), b.x[1].fraction),
            (2, "in", 'F')
        );
        assert_eq!(b.x[1].precision, 8);
        assert!(b.distance.is_empty() && b.y.is_empty());
        // Page 2 does not exist.
        assert!(doc.scale_viewports(1).is_empty());
    }
}
