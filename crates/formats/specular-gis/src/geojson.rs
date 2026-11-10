//! GeoJSON (RFC 7946, and the 2008 draft's `crs` member): FeatureCollection, Feature or a
//! bare geometry, GeometryCollection, any nesting of properties (objects and arrays shown
//! as JSON text).

use crate::crs;
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, Pt, Value};

/// A JSON value that keeps object members in file order, so attributes show in the order
/// the file lists them.
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub(crate) fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&Vec<J>> {
        match self {
            J::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            J::Int(i) => Some(*i as f64),
            J::Num(f) => Some(*f),
            _ => None,
        }
    }

    /// Compact JSON text.
    pub(crate) fn to_json(&self) -> String {
        let quote = |s: &str| serde_json::to_string(s).unwrap_or_default();
        match self {
            J::Null => "null".into(),
            J::Bool(b) => b.to_string(),
            J::Int(i) => i.to_string(),
            J::Num(f) => f.to_string(),
            J::Str(s) => quote(s),
            J::Arr(a) => format!(
                "[{}]",
                a.iter().map(J::to_json).collect::<Vec<_>>().join(",")
            ),
            J::Obj(m) => format!(
                "{{{}}}",
                m.iter()
                    .map(|(k, v)| format!("{}:{}", quote(k), v.to_json()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

impl<'de> serde::Deserialize<'de> for J {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<J, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_none<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_bool<E>(self, b: bool) -> Result<J, E> {
                Ok(J::Bool(b))
            }
            fn visit_i64<E>(self, i: i64) -> Result<J, E> {
                Ok(J::Int(i))
            }
            fn visit_u64<E>(self, u: u64) -> Result<J, E> {
                Ok(i64::try_from(u).map_or(J::Num(u as f64), J::Int))
            }
            fn visit_f64<E>(self, f: f64) -> Result<J, E> {
                Ok(J::Num(f))
            }
            fn visit_str<E>(self, s: &str) -> Result<J, E> {
                Ok(J::Str(s.to_string()))
            }
            fn visit_string<E>(self, s: String) -> Result<J, E> {
                Ok(J::Str(s))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut v = Vec::with_capacity(a.size_hint().unwrap_or(0).min(4096));
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(J::Arr(v))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut v = Vec::new();
                while let Some((k, x)) = a.next_entry::<String, J>()? {
                    v.push((k, x));
                }
                Ok(J::Obj(v))
            }
        }
        d.deserialize_any(V)
    }
}

/// Does this look like GeoJSON? (A JSON object with a GeoJSON `type` near the start.)
pub fn sniff(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    let t = s.trim_start_matches('\u{feff}').trim_start();
    t.starts_with('{')
        && [
            "\"FeatureCollection\"",
            "\"Feature\"",
            "\"Polygon\"",
            "\"MultiPolygon\"",
            "\"Point\"",
            "\"LineString\"",
            "\"MultiLineString\"",
            "\"MultiPoint\"",
            "\"GeometryCollection\"",
        ]
        .iter()
        .any(|k| s.contains(k))
        && s.contains("\"type\"")
}

pub fn read(data: &[u8], layer_name: &str) -> Result<Dataset, String> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let v: J = serde_json::from_slice(data).map_err(|e| format!("not valid JSON: {e}"))?;
    let mut ds = Dataset {
        format: "GeoJSON",
        ..Dataset::default()
    };
    // RFC 7946: WGS 84 longitude/latitude unless an old-style `crs` member says otherwise.
    let crs_ref = v
        .get("crs")
        .and_then(|c| c.get("properties"))
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str());
    match crs_ref {
        Some(r) => match crs::epsg_from_ref(r).and_then(crs::from_epsg) {
            Some(c) => ds.crs = Some(c),
            None => ds.crs_note = Some(r.to_string()),
        },
        None => ds.crs = crs::from_epsg(4326),
    }
    let mut layer = Layer::new(
        v.get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(layer_name)
            .to_string(),
    );
    match v.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => {
            let feats = v
                .get("features")
                .and_then(|f| f.as_array())
                .ok_or("FeatureCollection without features")?;
            for (i, f) in feats.iter().enumerate() {
                if layer.features.len() >= MAX_FEATURES {
                    ds.warn("too many features; the rest are not shown");
                    break;
                }
                feature(&mut layer, f, i, &mut ds.warnings);
            }
        }
        Some("Feature") => feature(&mut layer, &v, 0, &mut ds.warnings),
        Some(_) => {
            let g = geometry(&v, 0).ok_or("not a GeoJSON object")?;
            layer.push(Some("0".into()), Some(g), Vec::new());
        }
        None => return Err("not a GeoJSON object".into()),
    }
    ds.layers.push(layer);
    Ok(ds)
}

fn feature(layer: &mut Layer, f: &J, i: usize, warnings: &mut Vec<String>) {
    if f.get("type").and_then(|t| t.as_str()) != Some("Feature") {
        if warnings.len() < 32 {
            warnings.push("an entry of features is not a Feature".into());
        }
        return;
    }
    let geom = f.get("geometry").and_then(|g| geometry(g, 0));
    let id = f.get("id").map(|id| match id {
        J::Str(s) => s.clone(),
        other => other.to_json(),
    });
    let mut attrs = Vec::new();
    if let Some(J::Obj(p)) = f.get("properties") {
        for (k, v) in p {
            attrs.push((k.clone(), value(v)));
        }
    }
    layer.push(id.or(Some(i.to_string())), geom, attrs);
}

pub(crate) fn value(v: &J) -> Value {
    match v {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(*b),
        J::Int(i) => Value::Int(*i),
        J::Num(f) => Value::Float(*f),
        J::Str(s) => Value::Text(s.clone()),
        other => Value::Text(other.to_json()),
    }
}

/// A position: two or more numbers (a non-number anywhere makes it invalid, as in GDAL).
fn pt(v: &J) -> Option<Pt> {
    let a = v.as_array()?;
    let x = a.first()?.as_f64()?;
    let y = a.get(1)?.as_f64()?;
    a.iter()
        .skip(2)
        .all(|z| z.as_f64().is_some())
        .then_some([x, y])
}

fn line(v: &J) -> Option<Vec<Pt>> {
    v.as_array()?.iter().map(pt).collect()
}

/// Rings or line parts; invalid ones are left out.
fn rings(v: &J) -> Option<Vec<Vec<Pt>>> {
    let r: Vec<Vec<Pt>> = v
        .as_array()?
        .iter()
        .filter_map(line)
        .filter(|l| !l.is_empty())
        .collect();
    (!r.is_empty()).then_some(r)
}

fn geometry(g: &J, depth: usize) -> Option<Geometry> {
    if depth > 32 {
        return None;
    }
    let c = g.get("coordinates");
    Some(match g.get("type")?.as_str()? {
        "Point" => Geometry::Point(pt(c?)?),
        "MultiPoint" => Geometry::MultiPoint(line(c?)?),
        "LineString" => Geometry::LineString(line(c?)?),
        "MultiLineString" => Geometry::MultiLineString(rings(c?)?),
        "Polygon" => Geometry::Polygon(rings(c?)?),
        "MultiPolygon" => Geometry::MultiPolygon(c?.as_array()?.iter().filter_map(rings).collect()),
        "GeometryCollection" => Geometry::Collection(
            g.get("geometries")?
                .as_array()?
                .iter()
                .filter_map(|g| geometry(g, depth + 1))
                .collect(),
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection() {
        let s = br#"{"type":"FeatureCollection","features":[
            {"type":"Feature","id":7,"properties":{"name":"A","n":3,"x":1.5,"o":{"k":1}},
             "geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]],[[2,2],[4,2],[4,4],[2,2]]]}},
            {"type":"Feature","properties":{"name":"B"},"geometry":{"type":"Point","coordinates":[5,5,1]}},
            {"type":"Feature","properties":null,"geometry":null}]}"#;
        assert!(sniff(s));
        let d = read(s, "t").unwrap();
        let l = &d.layers[0];
        assert_eq!(l.features.len(), 3);
        assert_eq!(l.fields(), ["name", "n", "x", "o"]);
        assert_eq!(l.features[0].id.as_deref(), Some("7"));
        assert_eq!(l.attributes(0)[3].1, Value::Text("{\"k\":1}".into()));
        assert_eq!(d.crs.as_ref().unwrap().epsg, Some(4326));
        assert_eq!(d.bounds(), Some([0.0, 0.0, 10.0, 10.0]));
    }
}
