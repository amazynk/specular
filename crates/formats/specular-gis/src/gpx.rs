//! GPX 1.0/1.1: waypoints, routes and tracks as three layers (the layout GDAL uses).
//! Tracks and routes also carry a summary of their points: count, start, end and duration,
//! and from Garmin's TrackPointExtension (v1/v2) and GpxExtensions v3 the heart rate,
//! cadence and temperature.

use crate::crs;
use crate::fit::{Summary, parse_iso};
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, Pt, Value};
use roxmltree::Node;

pub fn sniff(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    s.contains("<gpx") && s.contains("topografix.com/GPX")
}

fn tag(n: &Node<'_, '_>, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name
}

fn text(n: Node<'_, '_>, name: &str) -> Option<String> {
    let c = n.children().find(|c| tag(c, name))?;
    let s = c.text()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn pt(n: Node<'_, '_>) -> Option<Pt> {
    let lon = n.attribute("lon")?.trim().parse::<f64>().ok()?;
    let lat = n.attribute("lat")?.trim().parse::<f64>().ok()?;
    Some([lon, lat])
}

const POINT_FIELDS: &[&str] = &["ele", "time", "name", "cmt", "desc", "src", "sym", "type"];
const LINE_FIELDS: &[&str] = &["name", "cmt", "desc", "src", "number", "type"];

/// Summary fields added to tracks and routes.
const TRACK_SUMMARY: &[&str] = &[
    "points",
    "start",
    "end",
    "duration_s",
    "avg_hr",
    "max_hr",
    "avg_cadence",
    "min_temp",
    "avg_temp",
    "max_temp",
];
const ROUTE_SUMMARY: &[&str] = &["points", "start", "end", "duration_s"];

/// A point's time and extension values into a summary.
fn summarise(p: Node<'_, '_>, s: &mut Summary) {
    if let Some(t) = text(p, "time").and_then(|t| parse_iso(&t)) {
        s.time(t);
    }
    let Some(ext) = p.children().find(|c| tag(c, "extensions")) else {
        return;
    };
    for d in ext.descendants().filter(Node::is_element) {
        let v = || d.text()?.trim().parse::<f64>().ok();
        match d.tag_name().name() {
            "hr" | "HeartRate" | "heartrate" => {
                if let Some(v) = v() {
                    s.hr.add(v);
                }
            }
            "cad" | "Cadence" | "cadence" => {
                if let Some(v) = v() {
                    s.cad.add(v);
                }
            }
            "atemp" | "Temperature" | "temp" => {
                if let Some(v) = v() {
                    s.temp.add(v);
                }
            }
            _ => {}
        }
    }
}

fn attrs(n: Node<'_, '_>, fields: &[&str]) -> Vec<(String, Value)> {
    fields
        .iter()
        .map(|f| {
            let v = text(n, f);
            let v = match (*f, v) {
                (_, None) => Value::Null,
                ("ele", Some(s)) => s.parse().map_or(Value::Text(s), Value::Float),
                ("number", Some(s)) => s.parse().map_or(Value::Text(s), Value::Int),
                ("time", Some(s)) => Value::Date(s),
                (_, Some(s)) => Value::Text(s),
            };
            (f.to_string(), v)
        })
        .collect()
}

pub fn read(data: &[u8]) -> Result<Dataset, String> {
    let text = std::str::from_utf8(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data))
        .map_err(|_| "the GPX is not UTF-8")?;
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err("the XML nests too deeply".into());
    }
    let doc = roxmltree::Document::parse(text).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "gpx" {
        return Err("not a GPX document".into());
    }
    let mut ds = Dataset {
        format: "GPX",
        crs: crs::from_epsg(4326),
        ..Dataset::default()
    };
    let new = |name: &str, fields: &[&str], ty: &str| {
        let mut l = Layer::new(name);
        for f in fields {
            l.field(f);
        }
        l.declared_type = Some(ty.to_string());
        l
    };
    let mut wpt = new("waypoints", POINT_FIELDS, "Point");
    let mut rte = new("routes", LINE_FIELDS, "LineString");
    let mut trk = new("tracks", LINE_FIELDS, "MultiLineString");
    for f in ROUTE_SUMMARY {
        rte.field(f);
    }
    for f in TRACK_SUMMARY {
        trk.field(f);
    }
    let mut n = 0usize;
    for c in root.children().filter(Node::is_element) {
        if n >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        match c.tag_name().name() {
            "wpt" => {
                let id = wpt.features.len().to_string();
                wpt.push(Some(id), pt(c).map(Geometry::Point), attrs(c, POINT_FIELDS));
            }
            "rte" => {
                let mut s = Summary::default();
                let pts: Vec<Pt> = c
                    .children()
                    .filter(|p| tag(p, "rtept"))
                    .inspect(|p| summarise(*p, &mut s))
                    .filter_map(pt)
                    .collect();
                s.points = pts.len();
                let id = rte.features.len().to_string();
                let g = (!pts.is_empty()).then_some(Geometry::LineString(pts));
                let mut a = attrs(c, LINE_FIELDS);
                a.extend(s.attrs(ROUTE_SUMMARY));
                rte.push(Some(id), g, a);
            }
            "trk" => {
                let mut sum = Summary::default();
                let segs: Vec<Vec<Pt>> = c
                    .children()
                    .filter(|s| tag(s, "trkseg"))
                    .map(|s| {
                        s.children()
                            .filter(|p| tag(p, "trkpt"))
                            .inspect(|p| summarise(*p, &mut sum))
                            .filter_map(pt)
                            .collect()
                    })
                    .collect();
                sum.points = segs.iter().map(Vec::len).sum();
                let id = trk.features.len().to_string();
                let g = (!segs.is_empty()).then_some(Geometry::MultiLineString(segs));
                let mut a = attrs(c, LINE_FIELDS);
                a.extend(sum.attrs(TRACK_SUMMARY));
                trk.push(Some(id), g, a);
            }
            _ => continue,
        }
        n += 1;
    }
    ds.layers = vec![wpt, rte, trk];
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document nested 100 000 elements deep is refused before the XML parser (which
    /// recurses per level) sees it. The same check guards every XML reader of the crate.
    #[test]
    fn deep_xml_refused() {
        let d = 100_000;
        let x = format!("<gpx>{}{}</gpx>", "<a>".repeat(d), "</a>".repeat(d));
        let r = specular_thread::on_stack(256 * 1024, || read(x.as_bytes()));
        assert!(r.unwrap_err().contains("nests too deeply"));
    }

    #[test]
    fn layers() {
        let g = br#"<?xml version="1.0"?>
<gpx version="1.1" creator="t" xmlns="http://www.topografix.com/GPX/1/1">
 <wpt lat="47.5" lon="8.25"><ele>410.5</ele><name>Hut</name></wpt>
 <rte><name>R</name><number>3</number><rtept lat="1" lon="2"/><rtept lat="3" lon="4"/></rte>
 <trk><name>T</name><trkseg><trkpt lat="10" lon="20"><time>2024-05-01T10:00:00Z</time></trkpt><trkpt lat="11" lon="21"/></trkseg><trkseg/></trk>
</gpx>"#;
        assert!(sniff(g));
        let d = read(g).unwrap();
        assert_eq!(
            d.layers[0].features[0].geometry,
            Some(Geometry::Point([8.25, 47.5]))
        );
        assert_eq!(d.layers[0].features[0].values[0], Value::Float(410.5));
        assert_eq!(d.layers[0].features[0].values[2], Value::Text("Hut".into()));
        assert_eq!(d.layers[1].features[0].values[4], Value::Int(3));
        assert_eq!(
            d.layers[2].features[0].geometry,
            Some(Geometry::MultiLineString(vec![
                vec![[20.0, 10.0], [21.0, 11.0]],
                vec![]
            ]))
        );
    }

    #[test]
    fn track_point_extensions() {
        let g = br#"<?xml version="1.0"?>
<gpx version="1.1" creator="t" xmlns="http://www.topografix.com/GPX/1/1"
 xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v2"
 xmlns:gpxx="http://www.garmin.com/xmlschemas/GpxExtensions/v3">
 <trk><name>Ride</name><trkseg>
  <trkpt lat="1" lon="2"><time>2024-05-01T10:00:00Z</time><extensions><gpxtpx:TrackPointExtension><gpxtpx:hr>100</gpxtpx:hr><gpxtpx:cad>80</gpxtpx:cad><gpxtpx:atemp>20</gpxtpx:atemp></gpxtpx:TrackPointExtension></extensions></trkpt>
  <trkpt lat="1.1" lon="2.1"><time>2024-05-01T10:01:30Z</time><extensions><gpxtpx:TrackPointExtension><gpxtpx:hr>120</gpxtpx:hr><gpxtpx:cad>90</gpxtpx:cad></gpxtpx:TrackPointExtension><gpxx:TrackPointExtension><gpxx:Temperature>24</gpxx:Temperature></gpxx:TrackPointExtension></extensions></trkpt>
 </trkseg></trk>
</gpx>"#;
        let d = read(g).unwrap();
        let t = &d.layers[2];
        let a: std::collections::HashMap<_, _> = t.attributes(0).into_iter().collect();
        assert_eq!(a["name"], Value::Text("Ride".into()));
        assert_eq!(a["points"], Value::Int(2));
        assert_eq!(a["start"], Value::Date("2024-05-01T10:00:00Z".into()));
        assert_eq!(a["end"], Value::Date("2024-05-01T10:01:30Z".into()));
        assert_eq!(a["duration_s"], Value::Float(90.0));
        assert_eq!(a["avg_hr"], Value::Float(110.0));
        assert_eq!(a["max_hr"], Value::Float(120.0));
        assert_eq!(a["avg_cadence"], Value::Float(85.0));
        assert_eq!(a["min_temp"], Value::Float(20.0));
        assert_eq!(a["max_temp"], Value::Float(24.0));
    }
}
