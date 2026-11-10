//! Annotations as QuPath-compatible GeoJSON: a `FeatureCollection` of features in level-0
//! pixel coordinates with `properties.objectType = "annotation"`, `properties.name` and
//! `properties.classification = {name, color: [r, g, b]}` (QuPath 0.3 and later; 0.2's
//! `colorRGB`, a packed ARGB integer, is read too).
//!
//! Areas are `Polygon`s (holes as inner rings) or `MultiPolygon`s, a rectangle a closed
//! five-point `Polygon` (read back as a rectangle when axis-aligned), points `Point` or
//! `MultiPoint` (QuPath's points annotation) and lines `LineString`s. The reader takes a
//! feature collection, a single feature or an array of features (QuPath's older export).

use serde_json::Value;

use crate::annot::{Annotation, Class, Geometry, Poly, Pt, open_ring};
use crate::json::{self, num, round};

fn coord(p: Pt) -> String {
    format!("[{},{}]", num(round(p.0, 3)), num(round(p.1, 3)))
}

fn ring(r: &[Pt]) -> String {
    let mut s = String::from("[");
    for p in r {
        s.push_str(&coord(*p));
        s.push(',');
    }
    // Closed, as RFC 7946 asks.
    s.push_str(&coord(r[0]));
    s.push(']');
    s
}

fn poly(p: &Poly) -> String {
    let mut rings = vec![ring(&p.exterior)];
    rings.extend(p.holes.iter().filter(|h| h.len() >= 3).map(|h| ring(h)));
    format!("[{}]", rings.join(","))
}

fn list(v: &[Pt]) -> String {
    let c: Vec<String> = v.iter().map(|p| coord(*p)).collect();
    format!("[{}]", c.join(","))
}

fn geometry(g: &Geometry) -> String {
    match g {
        Geometry::Area(ps) if ps.len() == 1 => {
            format!("{{\"type\":\"Polygon\",\"coordinates\":{}}}", poly(&ps[0]))
        }
        Geometry::Area(ps) => {
            let parts: Vec<String> = ps.iter().map(poly).collect();
            format!(
                "{{\"type\":\"MultiPolygon\",\"coordinates\":[{}]}}",
                parts.join(",")
            )
        }
        Geometry::Rect { .. } => {
            format!(
                "{{\"type\":\"Polygon\",\"coordinates\":{}}}",
                poly(&g.polys()[0])
            )
        }
        Geometry::Point(p) => format!("{{\"type\":\"Point\",\"coordinates\":{}}}", coord(*p)),
        Geometry::Points(v) => format!("{{\"type\":\"MultiPoint\",\"coordinates\":{}}}", list(v)),
        Geometry::Line(v) => format!("{{\"type\":\"LineString\",\"coordinates\":{}}}", list(v)),
    }
}

/// The GeoJSON for `items`; invalid shapes are left out (counted).
pub fn write(items: &[Annotation]) -> (String, usize) {
    let mut s = String::from("{\"type\":\"FeatureCollection\",\"features\":[");
    let mut n = 0;
    let mut skipped = 0;
    for a in items {
        if !a.geometry.is_valid() {
            skipped += 1;
            continue;
        }
        if n > 0 {
            s.push(',');
        }
        n += 1;
        s.push_str("{\"type\":\"Feature\",\"geometry\":");
        s.push_str(&geometry(&a.geometry));
        s.push_str(",\"properties\":{\"objectType\":\"annotation\"");
        if !a.name.is_empty() {
            s.push_str(&format!(",\"name\":{}", json::string(&a.name)));
        }
        if let (Some(c), Some(name)) = (&a.class, a.class_name()) {
            s.push_str(&format!(
                ",\"classification\":{{\"name\":{}",
                json::string(name)
            ));
            if let Some(col) = c.colour {
                s.push_str(&format!(",\"color\":{}", json::rgb(col)));
            }
            s.push('}');
        }
        if matches!(a.geometry, Geometry::Rect { .. }) {
            // Not QuPath's, but harmless: our own rectangles come back as rectangles.
            s.push_str(",\"specularShape\":\"rectangle\"");
        }
        s.push_str(",\"isLocked\":false}}");
    }
    s.push_str("]}\n");
    (s, skipped)
}

/// What [`read`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Read {
    pub items: Vec<Annotation>,
    /// Features without a usable geometry (or of a type with no annotation form).
    pub skipped: usize,
    /// Detections and other non-annotation objects, read as annotations.
    pub detections: usize,
}

/// Why a file could not be read.
#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    /// Valid JSON that is not GeoJSON features.
    NotGeoJson,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Json(e) => write!(f, "{e}"),
            Error::NotGeoJson => f.write_str("not GeoJSON features"),
        }
    }
}

fn pt(v: &Value) -> Option<Pt> {
    let a = v.as_array()?;
    let x = a.first()?.as_f64()?;
    let y = a.get(1)?.as_f64()?;
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn pts(v: &Value) -> Option<Vec<Pt>> {
    v.as_array()?.iter().map(pt).collect()
}

fn read_poly(v: &Value) -> Option<Poly> {
    let rings = v.as_array()?;
    let mut it = rings.iter().map(|r| pts(r).map(open_ring));
    let exterior = it.next()??;
    let holes = it.collect::<Option<Vec<_>>>()?;
    Some(Poly {
        exterior,
        holes: holes.into_iter().filter(|h| h.len() >= 3).collect(),
    })
}

/// An axis-aligned four-corner ring as a rectangle.
fn as_rect(p: &Poly) -> Option<Geometry> {
    if !p.holes.is_empty() || p.exterior.len() != 4 {
        return None;
    }
    let e = &p.exterior;
    let axis = (0..4).all(|i| {
        let (a, b) = (e[i], e[(i + 1) % 4]);
        a.0 == b.0 || a.1 == b.1
    });
    let r = Geometry::rect(e[0], e[2]);
    (axis && r.area() > 0.0).then_some(r)
}

fn read_geometry(g: &Value, rect_hint: bool) -> Option<Geometry> {
    let c = g.get("coordinates")?;
    let out = match g.get("type")?.as_str()? {
        "Polygon" => {
            let p = read_poly(c)?;
            match as_rect(&p).filter(|_| rect_hint) {
                Some(r) => r,
                None => Geometry::Area(vec![p]),
            }
        }
        "MultiPolygon" => {
            Geometry::Area(c.as_array()?.iter().map(read_poly).collect::<Option<_>>()?)
        }
        "Point" => Geometry::Point(pt(c)?),
        "MultiPoint" => {
            let v = pts(c)?;
            if v.len() == 1 {
                Geometry::Point(v[0])
            } else {
                Geometry::Points(v)
            }
        }
        "LineString" => Geometry::Line(pts(c)?),
        _ => return None,
    };
    out.is_valid().then_some(out)
}

fn class(props: &Value) -> Option<Class> {
    let c = props.get("classification")?;
    let name = c.get("name")?.as_str()?.to_owned();
    let colour = c
        .get("color")
        .and_then(|a| {
            let a = a.as_array()?;
            let ch = |i: usize| a.get(i)?.as_u64().and_then(|v| u8::try_from(v).ok());
            Some([ch(0)?, ch(1)?, ch(2)?])
        })
        .or_else(|| {
            // QuPath 0.2: packed (A)RGB, often negative.
            let v = c.get("colorRGB")?.as_i64()? as u32;
            Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
        });
    Some(Class { name, colour })
}

/// Reads QuPath (or any) GeoJSON features.
pub fn read(text: &str) -> Result<Read, Error> {
    let v: Value = serde_json::from_str(text).map_err(Error::Json)?;
    let features: Vec<&Value> = match (&v, v.get("type").and_then(Value::as_str)) {
        (_, Some("FeatureCollection")) => v
            .get("features")
            .and_then(Value::as_array)
            .ok_or(Error::NotGeoJson)?
            .iter()
            .collect(),
        (_, Some("Feature")) => vec![&v],
        (Value::Array(a), _) => a.iter().collect(),
        _ => return Err(Error::NotGeoJson),
    };
    let mut out = Read::default();
    for f in features {
        let props = f.get("properties").cloned().unwrap_or(Value::Null);
        let rect = props.get("specularShape").and_then(Value::as_str) == Some("rectangle");
        let Some(geometry) = f.get("geometry").and_then(|g| read_geometry(g, rect)) else {
            out.skipped += 1;
            continue;
        };
        if props
            .get("objectType")
            .and_then(Value::as_str)
            .is_some_and(|t| t != "annotation")
        {
            out.detections += 1;
        }
        out.items.push(Annotation {
            name: props
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            class: class(&props),
            geometry,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_exact() {
        let items = crate::coco::tests::sample();
        let (text, skipped) = write(&items);
        assert_eq!(skipped, 0);
        let back = read(&text).unwrap();
        assert_eq!(back.items, items);
        let v: Value = serde_json::from_str(&text).unwrap();
        let f0 = &v["features"][0];
        assert_eq!(f0["properties"]["objectType"], "annotation");
        assert_eq!(f0["properties"]["classification"]["name"], "Tumor");
        assert_eq!(
            f0["properties"]["classification"]["color"],
            serde_json::json!([200, 0, 0])
        );
        // Rings are closed.
        let ring = f0["geometry"]["coordinates"][0].as_array().unwrap();
        assert_eq!(ring.first(), ring.last());
        assert_eq!(
            v["features"][2]["geometry"]["coordinates"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(v["features"][4]["geometry"]["type"], "MultiPoint");
    }

    #[test]
    fn reads_qupath_exports() {
        // QuPath 0.4-style feature with an id, a detection, and a 0.2-style colour.
        let text = r#"[
          {"type":"Feature","id":"9b1c","geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]},
           "properties":{"objectType":"annotation","classification":{"name":"Tumor","color":[200,0,0]},"isLocked":false}},
          {"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[5,0],[0,5],[0,0]]]},
           "properties":{"objectType":"detection","classification":{"name":"Other","colorRGB":-16711936}}},
          {"type":"Feature","geometry":{"type":"GeometryCollection","geometries":[]},"properties":{}},
          {"type":"Feature","geometry":{"type":"MultiPolygon","coordinates":[[[[0,0],[1,0],[1,1],[0,0]]],[[[5,5],[6,5],[6,6],[5,5]]]]},"properties":{}}
        ]"#;
        let r = read(text).unwrap();
        assert_eq!(r.items.len(), 3);
        assert_eq!(r.skipped, 1);
        assert_eq!(r.detections, 1);
        // A QuPath square stays a polygon (no hint); ours come back as rectangles.
        assert!(matches!(r.items[0].geometry, Geometry::Area(_)));
        assert_eq!(r.items[1].class.as_ref().unwrap().colour, Some([0, 255, 0]));
        assert!(matches!(&r.items[2].geometry, Geometry::Area(p) if p.len() == 2));
        assert!(matches!(
            read("{\"type\":\"Topology\"}"),
            Err(Error::NotGeoJson)
        ));
    }
}
