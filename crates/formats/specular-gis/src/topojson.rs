//! TopoJSON: geometries sharing arcs. Shared, possibly quantized (delta-encoded integers
//! with a `transform`), arcs are decoded once; lines and rings are put together from arc
//! indices (`~i`, that is `-i - 1`, runs arc `i` backwards). Each member of `objects` is a
//! layer; a GeometryCollection's geometries are its features, anything else one feature.

use crate::crs;
use crate::geojson::{J, value};
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value};

/// Whether the bytes look like TopoJSON: an object whose own top-level `type` is `Topology`
/// (a GeoJSON feature with a `"type": "Topology"` property is not).
pub fn sniff(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    let t = s.trim_start_matches('\u{feff}').trim_start();
    if !t.starts_with('{') {
        return false;
    }
    // Walk the top-level object's members: strings at depth 1 alternate key, value.
    let b = t.as_bytes();
    let (mut i, mut depth) = (0, 0usize);
    let mut key: Option<&[u8]> = None;
    let mut after_colon = false;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                let Some(text) = b.get(start..i) else {
                    return false;
                };
                if depth == 1 {
                    if after_colon {
                        if key == Some(b"type") {
                            return text == b"Topology";
                        }
                        after_colon = false;
                        key = None;
                    } else {
                        key = Some(text);
                    }
                }
            }
            b':' if depth == 1 => after_colon = true,
            b',' if depth == 1 => {
                after_colon = false;
                key = None;
            }
            b'{' | b'[' => {
                if depth == 1 && after_colon && key == Some(b"type") {
                    return false;
                }
                depth += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

struct Topo {
    arcs: Vec<Vec<Pt>>,
    /// Quantization: scale and translate.
    transform: Option<([f64; 2], [f64; 2])>,
    vertices: usize,
}

impl Topo {
    fn position(&self, v: &J) -> Option<Pt> {
        let a = v.as_array()?;
        let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
        Some(match self.transform {
            Some((s, t)) => [x * s[0] + t[0], y * s[1] + t[1]],
            None => [x, y],
        })
    }

    /// A line from arc indices.
    fn line(&mut self, v: &J) -> Option<Vec<Pt>> {
        let mut out: Vec<Pt> = Vec::new();
        for i in v.as_array()? {
            let i = match i {
                J::Int(i) => *i,
                J::Num(f) if f.fract() == 0.0 => *f as i64,
                _ => return None,
            };
            let (k, rev) = if i < 0 { (!i, true) } else { (i, false) };
            let arc = self.arcs.get(usize::try_from(k).ok()?)?;
            self.vertices += arc.len();
            if self.vertices > MAX_VERTICES {
                return None;
            }
            let pts: Box<dyn Iterator<Item = &Pt>> = if rev {
                Box::new(arc.iter().rev())
            } else {
                Box::new(arc.iter())
            };
            for (n, p) in pts.enumerate() {
                // Arcs after the first share their first point with the last one's end.
                if n == 0 && !out.is_empty() {
                    continue;
                }
                out.push(*p);
            }
        }
        Some(out)
    }

    fn lines(&mut self, v: &J) -> Option<Vec<Vec<Pt>>> {
        v.as_array()?.iter().map(|l| self.line(l)).collect()
    }

    fn geometry(&mut self, g: &J, depth: usize) -> Option<Geometry> {
        if depth > 32 {
            return None;
        }
        Some(match g.get("type")?.as_str()? {
            "Point" => Geometry::Point(self.position(g.get("coordinates")?)?),
            "MultiPoint" => Geometry::MultiPoint(
                g.get("coordinates")?
                    .as_array()?
                    .iter()
                    .map(|p| self.position(p))
                    .collect::<Option<_>>()?,
            ),
            "LineString" => Geometry::LineString(self.line(g.get("arcs")?)?),
            "MultiLineString" => Geometry::MultiLineString(self.lines(g.get("arcs")?)?),
            "Polygon" => Geometry::Polygon(self.lines(g.get("arcs")?)?),
            "MultiPolygon" => Geometry::MultiPolygon(
                g.get("arcs")?
                    .as_array()?
                    .iter()
                    .map(|p| self.lines(p))
                    .collect::<Option<_>>()?,
            ),
            "GeometryCollection" => Geometry::Collection(
                g.get("geometries")?
                    .as_array()?
                    .iter()
                    .filter_map(|g| self.geometry(g, depth + 1))
                    .collect(),
            ),
            _ => return None,
        })
    }
}

/// Decodes the arcs: quantized ones are delta-encoded.
fn arcs(v: &J, transform: Option<([f64; 2], [f64; 2])>) -> Result<Vec<Vec<Pt>>, String> {
    let list = v.as_array().ok_or("arcs is not a list")?;
    let mut total = 0usize;
    list.iter()
        .map(|a| {
            // An arc that is not a list of positions is empty (lines using it lose it).
            let Some(pts) = a.as_array() else {
                return Ok(Vec::new());
            };
            total += pts.len();
            if total > MAX_VERTICES {
                return Err("too many vertices".to_string());
            }
            let mut out = Vec::with_capacity(pts.len());
            let (mut x, mut y) = (0.0, 0.0);
            for p in pts {
                let Some((dx, dy)) = p.as_array().and_then(|a| {
                    a.first()
                        .and_then(J::as_f64)
                        .zip(a.get(1).and_then(J::as_f64))
                }) else {
                    return Ok(Vec::new());
                };
                out.push(match transform {
                    Some((s, t)) => {
                        x += dx;
                        y += dy;
                        [x * s[0] + t[0], y * s[1] + t[1]]
                    }
                    None => [dx, dy],
                });
            }
            Ok(out)
        })
        .collect()
}

/// JSON text without commas just before a closing bracket or brace (outside strings).
fn without_trailing_commas(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let (mut in_str, mut esc) = (false, false);
    for (i, &b) in data.iter().enumerate() {
        if in_str {
            match (esc, b) {
                (true, _) => esc = false,
                (false, b'\\') => esc = true,
                (false, b'"') => in_str = false,
                _ => {}
            }
        } else if b == b'"' {
            in_str = true;
        } else if b == b',' {
            let next = data[i + 1..].iter().find(|c| !c.is_ascii_whitespace());
            if matches!(next, Some(b']' | b'}')) {
                continue;
            }
        }
        out.push(b);
    }
    out
}

pub fn read(data: &[u8]) -> Result<Dataset, String> {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    // Trailing commas (`[1, 2,]`) are let through, as GDAL does.
    let v: J = match serde_json::from_slice(data) {
        Ok(v) => v,
        Err(e) => serde_json::from_slice(&without_trailing_commas(data))
            .map_err(|_| format!("not valid JSON: {e}"))?,
    };
    if v.get("type").and_then(J::as_str) != Some("Topology") {
        return Err("not a TopoJSON topology".into());
    }
    let pair = |j: Option<&J>| -> Option<[f64; 2]> {
        let a = j?.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
    };
    let transform = v
        .get("transform")
        .and_then(|t| Some((pair(t.get("scale"))?, pair(t.get("translate"))?)));
    let mut topo = Topo {
        arcs: match v.get("arcs") {
            Some(a) => arcs(a, transform)?,
            None => Vec::new(),
        },
        transform,
        vertices: 0,
    };
    let mut ds = Dataset {
        format: "TopoJSON",
        ..Dataset::default()
    };
    let crs_ref = v
        .get("crs")
        .and_then(|c| c.get("properties"))
        .and_then(|p| p.get("name"))
        .and_then(J::as_str);
    match crs_ref {
        Some(r) => match crs::epsg_from_ref(r).and_then(crs::from_epsg) {
            Some(c) => ds.crs = Some(c),
            None => ds.crs_note = Some(r.to_string()),
        },
        None => ds.crs = crs::from_epsg(4326),
    }
    // Objects by name (an array in some old files: a collection's `id` names it).
    let objects: Vec<(String, &J)> = match v.get("objects") {
        Some(J::Obj(m)) => m.iter().map(|(k, o)| (k.clone(), o)).collect(),
        Some(J::Arr(a)) => a
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let name = match o.get("id") {
                    Some(J::Str(s)) => s.clone(),
                    _ => format!("object {}", i + 1),
                };
                (name, o)
            })
            .collect(),
        _ => return Err("no objects".into()),
    };
    // As GDAL lays it out: each GeometryCollection is a layer of its geometries; the other
    // objects are the features of one layer, "TopoJSON", their names as `id`.
    let mut others = Layer::new("TopoJSON");
    let mut n = 0usize;
    let mut full = false;
    let mut feature =
        |layer: &mut Layer, g: &J, id_field: Option<String>, i: usize, topo: &mut Topo| {
            n += 1;
            if n > MAX_FEATURES || topo.vertices > MAX_VERTICES {
                full = true;
                return;
            }
            let geom = topo.geometry(g, 0);
            let id = g.get("id").map(|id| match id {
                J::Str(s) => s.clone(),
                other => other.to_json(),
            });
            let mut attrs = Vec::new();
            if let Some(name) = id_field {
                attrs.push(("id".to_owned(), Value::Text(name)));
            } else if let Some(id) = &id {
                attrs.push(("id".to_owned(), Value::Text(id.clone())));
            }
            if let Some(J::Obj(p)) = g.get("properties") {
                for (k, v) in p {
                    if k == "id" {
                        attrs.retain(|(a, _)| a != "id");
                    }
                    attrs.push((k.clone(), value(v)));
                }
            }
            layer.push(id.or(Some(i.to_string())), geom, attrs);
        };
    for (name, o) in objects {
        if !matches!(o, J::Obj(_)) {
            continue;
        }
        if o.get("type").and_then(J::as_str) == Some("GeometryCollection") {
            let mut layer = Layer::new(name);
            if let Some(g) = o.get("geometries").and_then(J::as_array) {
                for (i, g) in g.iter().enumerate() {
                    feature(&mut layer, g, None, i, &mut topo);
                }
            }
            ds.layers.push(layer);
        } else {
            let i = others.features.len();
            feature(&mut others, o, Some(name), i, &mut topo);
        }
    }
    if full {
        ds.warn("too many features or vertices; the rest are not shown");
    }
    if !others.features.is_empty() {
        ds.layers.push(others);
    }
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantized_shared_arcs() {
        assert!(!sniff(
            br#"{"type":"Feature","properties":{"type":"Topology"}}"#
        ));
        assert!(!sniff(
            br#"{"objects":{"a":{"type":"Topology"}},"type":"FeatureCollection"}"#
        ));
        assert!(sniff(br#"{ "arcs": [[[0,0]]], "type" : "Topology" }"#));
        let t = br#"{"type":"Topology",
          "transform":{"scale":[0.5,0.25],"translate":[100,10]},
          "objects":{
            "areas":{"type":"GeometryCollection","geometries":[
              {"type":"Polygon","arcs":[[0,1]],"id":"a","properties":{"name":"left"}},
              {"type":"Polygon","arcs":[[-1,2]],"properties":{"name":"right"}}]},
            "spot":{"type":"Point","coordinates":[2,4]},
            "road":{"type":"LineString","arcs":[~0]}},
          "arcs":[[[2,0],[0,4]],[[2,4],[-2,0],[0,-4],[2,0]],[[2,0],[2,0],[0,4],[-2,0]]]}"#;
        let t = String::from_utf8_lossy(t).replace("~0", "-1");
        assert!(sniff(t.as_bytes()));
        let d = read(t.as_bytes()).unwrap();
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.layers[1].name, "TopoJSON");
        assert_eq!(d.layers[0].name, "areas");
        // Arc 0 decodes to (101, 10) → (101, 11); arc 1 closes the left square.
        let Some(Geometry::Polygon(r)) = &d.layers[0].features[0].geometry else {
            panic!()
        };
        assert_eq!(
            r[0],
            [
                [101.0, 10.0],
                [101.0, 11.0],
                [100.0, 11.0],
                [100.0, 10.0],
                [101.0, 10.0]
            ]
        );
        let Some(Geometry::Polygon(r)) = &d.layers[0].features[1].geometry else {
            panic!()
        };
        assert_eq!(r[0][0], [101.0, 11.0]);
        assert_eq!(r[0].last(), Some(&[101.0, 11.0]));
        assert_eq!(d.layers[0].features[0].id.as_deref(), Some("a"));
        assert_eq!(
            d.layers[1].features[0].geometry,
            Some(Geometry::Point([101.0, 11.0]))
        );
        assert_eq!(d.layers[1].attributes(0)[0].1, Value::Text("spot".into()));
        assert_eq!(
            d.layers[1].features[1].geometry,
            Some(Geometry::LineString(vec![[101.0, 11.0], [101.0, 10.0]]))
        );
        assert_eq!(d.crs.as_ref().unwrap().epsg, Some(4326));
        // Trailing commas pass.
        let trailing = t.replace("[-2,0]]]}", "[-2,0]],]}");
        assert_ne!(trailing, t);
        assert!(read(trailing.as_bytes()).is_ok());
        // A bad arc index leaves that feature without a geometry.
        let bad = t.replace("[[-1,2]]", "[[-1,9]]");
        assert!(
            read(bad.as_bytes()).unwrap().layers[0].features[1]
                .geometry
                .is_none()
        );
    }
}
