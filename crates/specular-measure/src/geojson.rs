//! Measurements as GeoJSON in QuPath's convention: a `FeatureCollection` of annotation
//! features in pixel coordinates (origin at the top-left corner, y down), with
//! `properties.objectType = "annotation"`, `properties.name` and `properties.measurements`.
//!
//! Rectangles, polygons and freehand outlines are `Polygon`s, an ellipse a `Polygon` of
//! [`ELLIPSE_VERTICES`](crate::geom::ELLIPSE_VERTICES) points, lines and polylines
//! `LineString`s, an angle a three-point `LineString` (arm, vertex, arm), a point a `Point`.
//! `properties.specular` keeps the exact shape, frame and calibration, so Specular reads its
//! own files back without loss; the provenance is the collection's `specular` member (a
//! foreign member, which RFC 7946 §6.1 allows). The reader also takes plain GeoJSON:
//! a feature collection, a feature, an array of features (QuPath's older export) or a bare
//! geometry; polygon holes are dropped and counted.

use std::fmt::Write as _;

use serde_json::Value;

use crate::calib::{Calibration, Source, Unit};
use crate::geom::{P, Shape};
use crate::list::Measurement;
use crate::provenance::Provenance;

/// A JSON string literal.
pub fn string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn num(v: f64) -> String {
    if v.is_finite() {
        // Display never uses an exponent and round-trips.
        format!("{v}")
    } else {
        "null".to_owned()
    }
}

fn coords(p: &P) -> String {
    format!("[{},{}]", num(p.x), num(p.y))
}

fn ring(v: &[P]) -> String {
    let mut pts: Vec<String> = v.iter().map(coords).collect();
    if let Some(first) = v.first() {
        pts.push(coords(first));
    }
    format!("[{}]", pts.join(","))
}

fn geometry(shape: &Shape) -> String {
    match shape {
        Shape::Point(p) => format!("{{\"type\":\"Point\",\"coordinates\":{}}}", coords(p)),
        Shape::Line(..) | Shape::Polyline(_) | Shape::Angle { .. } => {
            let pts: Vec<String> = shape.points().iter().map(coords).collect();
            format!(
                "{{\"type\":\"LineString\",\"coordinates\":[{}]}}",
                pts.join(",")
            )
        }
        _ => format!(
            "{{\"type\":\"Polygon\",\"coordinates\":[{}]}}",
            ring(&shape.outline())
        ),
    }
}

fn measurements(m: &Measurement) -> String {
    let mut fields: Vec<(String, f64)> = Vec::new();
    let unit = m.cal.unit.key();
    if let Some(q) = m.length() {
        fields.push((format!("Length {unit}"), q.value));
    }
    if let Some(q) = m.area() {
        fields.push((format!("Area {unit}^2"), q.value));
    }
    if let Some(q) = m.perimeter() {
        fields.push((format!("Perimeter {unit}"), q.value));
    }
    if let Some(q) = m.angle() {
        fields.push(("Angle deg".to_owned(), q.value));
    }
    for r in m.outcome().map(|o| o.readouts).unwrap_or_default() {
        fields.push((
            format!("{} {}", r.key, r.value.unit_key()),
            r.value.number(),
        ));
    }
    if let Some(stats) = &m.stats {
        for (i, s) in stats.iter().enumerate() {
            let c = m
                .channels
                .get(i)
                .cloned()
                .unwrap_or_else(|| (i + 1).to_string());
            if i == 0 {
                fields.push(("Num pixels".to_owned(), s.count as f64));
            }
            for (k, v) in [
                ("mean", s.mean),
                ("Std.Dev.", s.sd),
                ("Min", s.min),
                ("Max", s.max),
                ("Median", s.median),
                ("Sum", s.sum),
            ] {
                fields.push((format!("{c}: {k}"), v));
            }
        }
    }
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}:{}", string(k), num(*v)))
        .collect();
    format!("{{{}}}", body.join(","))
}

fn calibration(c: &Calibration) -> String {
    format!(
        "{{\"source\":{},\"unit\":{},\"m\":[[{},{}],[{},{}]]}}",
        string(c.source.key()),
        string(c.unit.key()),
        num(c.m[0][0]),
        num(c.m[0][1]),
        num(c.m[1][0]),
        num(c.m[1][1]),
    )
}

fn feature(m: &Measurement) -> String {
    let pts: Vec<String> = m.shape.points().iter().map(coords).collect();
    let mut props = vec![
        "\"objectType\":\"annotation\"".to_owned(),
        format!("\"name\":{}", string(&m.label)),
        format!("\"measurements\":{}", measurements(m)),
    ];
    let mut spec = vec![
        format!("\"shape\":{}", string(m.shape.key())),
        format!("\"points\":[{}]", pts.join(",")),
        format!("\"calibration\":{}", calibration(&m.cal)),
    ];
    if let Some(f) = m.frame {
        spec.push(format!("\"frame\":{f}"));
    }
    if m.approximate {
        spec.push("\"approximate\":true".to_owned());
    }
    if let Some(t) = m.template {
        spec.push(format!("\"template\":{}", string(t.key())));
    }
    props.push(format!("\"specular\":{{{}}}", spec.join(",")));
    format!(
        "{{\"type\":\"Feature\",\"id\":{},\"geometry\":{},\"properties\":{{{}}}}}",
        string(&m.id.to_string()),
        geometry(&m.shape),
        props.join(",")
    )
}

/// The list as a GeoJSON feature collection, one feature per line.
pub fn write(items: &[Measurement], prov: &Provenance) -> String {
    let prov: Vec<String> = prov
        .fields()
        .into_iter()
        .map(|(k, v)| format!("{}:{}", string(k), string(&v)))
        .collect();
    let features: Vec<String> = items.iter().map(feature).collect();
    format!(
        "{{\"type\":\"FeatureCollection\",\"specular\":{{{}}},\"features\":[\n{}\n]}}\n",
        prov.join(","),
        features.join(",\n")
    )
}

/// What a GeoJSON file held.
#[derive(Clone, Debug, Default)]
pub struct Read {
    /// Ids are 0 (the list assigns them); calibration is the file's when it recorded one,
    /// else uncalibrated.
    pub items: Vec<Measurement>,
    pub provenance: Option<Provenance>,
    /// Geometries that are not measurements (unsupported types, too few points, non-finite).
    pub skipped: usize,
    /// Polygon holes left out.
    pub holes: usize,
}

/// Why a file could not be read at all.
#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    /// Valid JSON but no GeoJSON object.
    NotGeoJson,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Json(e) => write!(f, "not JSON: {e}"),
            Error::NotGeoJson => f.write_str("not GeoJSON"),
        }
    }
}

impl std::error::Error for Error {}

/// Reads measurements from GeoJSON text.
pub fn read(text: &str) -> Result<Read, Error> {
    let v: Value = serde_json::from_str(text).map_err(Error::Json)?;
    let mut out = Read::default();
    match &v {
        Value::Array(items) => {
            for f in items {
                read_object(f, &mut out);
            }
        }
        Value::Object(_) if v.get("type").and_then(Value::as_str).is_some() => {
            if let Some(Value::Object(p)) = v.get("specular") {
                out.provenance = Some(Provenance::from_fields(
                    p.iter()
                        .filter_map(|(k, v)| Some((k.as_str(), v.as_str()?))),
                ));
            }
            read_object(&v, &mut out);
        }
        _ => return Err(Error::NotGeoJson),
    }
    Ok(out)
}

fn read_object(v: &Value, out: &mut Read) {
    match v.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => {
            for f in v
                .get("features")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                read_object(f, out);
            }
        }
        Some("Feature") => {
            let props = v.get("properties");
            let label = props
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let spec = props.and_then(|p| p.get("specular"));
            if let Some(m) = spec.and_then(|s| from_specular(s, &label)) {
                out.items.push(m);
                return;
            }
            let n = out.items.len();
            if let Some(g) = v.get("geometry") {
                read_geometry(g, out);
            } else {
                out.skipped += 1;
            }
            for m in &mut out.items[n..] {
                m.label.clone_from(&label);
            }
        }
        Some(_) => read_geometry(v, out),
        None => out.skipped += 1,
    }
}

fn point(v: &Value) -> Option<P> {
    let a = v.as_array()?;
    let p = P::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?);
    p.is_finite().then_some(p)
}

fn points(v: &Value) -> Option<Vec<P>> {
    v.as_array()?.iter().map(point).collect()
}

fn push(out: &mut Read, shape: Option<Shape>) {
    match shape.filter(Shape::is_valid) {
        Some(s) => out
            .items
            .push(Measurement::new(s, Calibration::uncalibrated())),
        None => out.skipped += 1,
    }
}

fn polygon(v: &Value, out: &mut Read) {
    let rings = v.as_array();
    out.holes += rings.map_or(0, |r| r.len().saturating_sub(1));
    let outer = rings.and_then(|r| r.first()).and_then(points).map(|mut p| {
        if p.len() > 1 && p.first() == p.last() {
            p.pop();
        }
        Shape::Polygon(p)
    });
    push(out, outer);
}

fn line(v: &Value, out: &mut Read) {
    let shape = points(v).map(|p| match p.as_slice() {
        [a, b] => Shape::Line(*a, *b),
        _ => Shape::Polyline(p),
    });
    push(out, shape);
}

fn read_geometry(g: &Value, out: &mut Read) {
    let c = g.get("coordinates").unwrap_or(&Value::Null);
    let each = |out: &mut Read, f: &dyn Fn(&Value, &mut Read)| {
        for item in c.as_array().into_iter().flatten() {
            f(item, out);
        }
    };
    match g.get("type").and_then(Value::as_str) {
        Some("Point") => push(out, point(c).map(Shape::Point)),
        Some("MultiPoint") => each(out, &|v, out| push(out, point(v).map(Shape::Point))),
        Some("LineString") => line(c, out),
        Some("MultiLineString") => each(out, &line),
        Some("Polygon") => polygon(c, out),
        Some("MultiPolygon") => each(out, &polygon),
        Some("GeometryCollection") => {
            for item in g
                .get("geometries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                read_geometry(item, out);
            }
        }
        _ => out.skipped += 1,
    }
}

fn from_specular(s: &Value, label: &str) -> Option<Measurement> {
    let pts = points(s.get("points")?)?;
    let shape = match (s.get("shape")?.as_str()?, pts.as_slice()) {
        ("point", [p]) => Shape::Point(*p),
        ("line", [a, b]) => Shape::Line(*a, *b),
        ("polyline", _) => Shape::Polyline(pts),
        ("angle", [a, v, b]) => Shape::Angle {
            a: *a,
            vertex: *v,
            b: *b,
        },
        ("rect", [a, b]) => Shape::Rect {
            x0: a.x,
            y0: a.y,
            x1: b.x,
            y1: b.y,
        },
        ("ellipse", [a, b]) => Shape::ellipse_in(*a, *b),
        ("polygon", _) => Shape::Polygon(pts),
        ("freehand", _) => Shape::Freehand(pts),
        _ => return None,
    };
    if !shape.is_valid() {
        return None;
    }
    let mut m = Measurement::new(shape, Calibration::uncalibrated());
    m.label = label.to_owned();
    m.frame = s
        .get("frame")
        .and_then(Value::as_u64)
        .and_then(|f| u32::try_from(f).ok());
    m.approximate = s
        .get("approximate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // A template's measurement keeps its template when its point count fits.
    m.template = s
        .get("template")
        .and_then(Value::as_str)
        .and_then(crate::clinical::Template::from_key)
        .filter(|t| t.landmarks().len() == m.shape.points().len());
    if let Some(c) = s.get("calibration") {
        let row = |i: usize| -> Option<[f64; 2]> {
            let r = c.get("m")?.get(i)?;
            Some([r.get(0)?.as_f64()?, r.get(1)?.as_f64()?])
        };
        if let (Some(source), Some(unit), Some(r0), Some(r1)) = (
            c.get("source")
                .and_then(Value::as_str)
                .and_then(Source::from_key),
            c.get("unit")
                .and_then(Value::as_str)
                .and_then(Unit::from_key),
            row(0),
            row(1),
        ) {
            let cal = Calibration {
                m: [r0, r1],
                unit,
                source,
            };
            if cal.pixel_area().is_finite() && cal.pixel_area() > 0.0 {
                m.cal = cal;
            }
        }
    }
    Some(m)
}
