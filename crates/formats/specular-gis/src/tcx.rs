//! Garmin Training Center XML (TCX v1/v2): activities (one track per activity, its laps'
//! times, distance, calories and heart rate summed up), courses (one track each) and
//! course points (a points layer). Trackpoints without a position are counted, not drawn.

use crate::crs;
use crate::fit::{SUMMARY_FIELDS, Summary, parse_iso, summary_layer};
use crate::model::{Dataset, Geometry, MAX_FEATURES, Pt, Value};
use roxmltree::Node;

pub fn sniff(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    s.contains("<TrainingCenterDatabase")
}

fn el<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn els<'a, 'i: 'a>(n: Node<'a, 'i>, name: &'a str) -> impl Iterator<Item = Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn text(n: Node<'_, '_>, name: &str) -> Option<String> {
    let s = el(n, name)?.text()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn num(n: Node<'_, '_>, name: &str) -> Option<f64> {
    text(n, name)?.parse().ok()
}

/// A `<HeartRateBpm><Value>n</Value></HeartRateBpm>`-style value.
fn bpm(n: Node<'_, '_>, name: &str) -> Option<f64> {
    num(el(n, name)?, "Value")
}

/// Trackpoints of a Track into the summary; returns the positions.
fn track_points(track: Node<'_, '_>, s: &mut Summary, unplaced: &mut usize) -> Vec<Pt> {
    let mut pts = Vec::new();
    for tp in els(track, "Trackpoint") {
        if let Some(t) = text(tp, "Time").and_then(|t| parse_iso(&t)) {
            s.time(t);
        }
        if let Some(v) = bpm(tp, "HeartRateBpm") {
            s.hr.add(v);
        }
        if let Some(v) = num(tp, "Cadence") {
            s.cad.add(v);
        }
        let pos = el(tp, "Position")
            .and_then(|p| Some([num(p, "LongitudeDegrees")?, num(p, "LatitudeDegrees")?]));
        match pos {
            Some(p) if p[0].is_finite() && p[1].is_finite() => pts.push(p),
            _ => *unplaced += 1,
        }
    }
    pts
}

fn line(parts: Vec<Vec<Pt>>) -> Option<Geometry> {
    let mut parts: Vec<Vec<Pt>> = parts.into_iter().filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0 => None,
        1 if parts[0].len() == 1 => Some(Geometry::Point(parts[0][0])),
        1 => Some(Geometry::LineString(parts.remove(0))),
        _ => Some(Geometry::MultiLineString(parts)),
    }
}

fn add(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a + b),
        (a, b) => a.or(b),
    }
}

pub fn read(data: &[u8], name: &str) -> Result<Dataset, String> {
    let _ = name;
    let text_ = std::str::from_utf8(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data))
        .map_err(|_| "the TCX is not UTF-8")?;
    if !specular_limits::xml_within_depth(text_.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err("the XML nests too deeply".into());
    }
    let doc = roxmltree::Document::parse(text_).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "TrainingCenterDatabase" {
        return Err("not a TCX document".into());
    }
    let mut ds = Dataset {
        format: "TCX",
        crs: crs::from_epsg(4326),
        ..Dataset::default()
    };
    let mut fields: Vec<&str> = vec!["name"];
    fields.extend_from_slice(SUMMARY_FIELDS);
    let mut tracks = summary_layer("tracks", &fields, "MultiLineString");
    let point_fields = ["name", "type", "time", "notes"];
    let mut points = summary_layer("course points", &point_fields, "Point");
    let mut unplaced = 0usize;
    let device = |n: Node<'_, '_>| el(n, "Creator").and_then(|c| text(c, "Name"));
    for acts in els(root, "Activities") {
        for a in els(acts, "Activity") {
            if tracks.features.len() >= MAX_FEATURES {
                break;
            }
            let mut s = Summary {
                sport: a.attribute("Sport").map(str::to_lowercase),
                device: device(a),
                ..Summary::default()
            };
            let mut parts = Vec::new();
            let (mut dur, mut dist, mut cal) = (None, None, None);
            let mut max_hr: Option<f64> = None;
            for lap in els(a, "Lap") {
                if let Some(t) = lap.attribute("StartTime").and_then(parse_iso)
                    && s.start.is_none_or(|s0| t < s0)
                {
                    s.start = Some(t);
                }
                dur = add(dur, num(lap, "TotalTimeSeconds"));
                dist = add(dist, num(lap, "DistanceMeters"));
                cal = add(cal, num(lap, "Calories"));
                if let Some(m) = bpm(lap, "MaximumHeartRateBpm") {
                    max_hr = Some(max_hr.map_or(m, |x: f64| x.max(m)));
                }
                for t in els(lap, "Track") {
                    parts.push(track_points(t, &mut s, &mut unplaced));
                }
            }
            s.duration = dur;
            s.distance = dist;
            s.calories = cal;
            s.max_hr = max_hr;
            s.points = parts.iter().map(Vec::len).sum();
            let mut attrs = vec![(
                "name".to_string(),
                text(a, "Id").map_or(Value::Null, Value::Text),
            )];
            attrs.extend(s.attrs(SUMMARY_FIELDS));
            let id = tracks.features.len().to_string();
            tracks.push(Some(id), line(parts), attrs);
        }
    }
    for cs in els(root, "Courses") {
        for c in els(cs, "Course") {
            if tracks.features.len() >= MAX_FEATURES {
                break;
            }
            let mut s = Summary::default();
            let mut parts = Vec::new();
            for lap in els(c, "Lap") {
                s.duration = add(s.duration, num(lap, "TotalTimeSeconds"));
                s.distance = add(s.distance, num(lap, "DistanceMeters"));
            }
            for t in els(c, "Track") {
                parts.push(track_points(t, &mut s, &mut unplaced));
            }
            s.points = parts.iter().map(Vec::len).sum();
            let mut attrs = vec![(
                "name".to_string(),
                text(c, "Name").map_or(Value::Null, Value::Text),
            )];
            attrs.extend(s.attrs(SUMMARY_FIELDS));
            let id = tracks.features.len().to_string();
            tracks.push(Some(id), line(parts), attrs);
            for cp in els(c, "CoursePoint") {
                let pos = el(cp, "Position")
                    .and_then(|p| Some([num(p, "LongitudeDegrees")?, num(p, "LatitudeDegrees")?]));
                let v = |k: &str| text(cp, k).map_or(Value::Null, Value::Text);
                let attrs = vec![
                    ("name".to_string(), v("Name")),
                    ("type".to_string(), v("PointType")),
                    (
                        "time".to_string(),
                        text(cp, "Time").map_or(Value::Null, Value::Date),
                    ),
                    ("notes".to_string(), v("Notes")),
                ];
                let id = points.features.len().to_string();
                points.push(Some(id), pos.map(Geometry::Point), attrs);
            }
        }
    }
    if tracks.features.iter().all(|f| f.geometry.is_none())
        && points.features.iter().all(|f| f.geometry.is_none())
    {
        return Err("this TCX file has no GPS positions".into());
    }
    ds.layers.push(tracks);
    if !points.features.is_empty() {
        ds.layers.push(points);
    }
    if unplaced > 0 {
        ds.warn(format!(
            "{unplaced} trackpoints have no position and are not drawn"
        ));
    }
    Ok(ds)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TCX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2"
  xmlns:ns3="http://www.garmin.com/xmlschemas/ActivityExtension/v2">
 <Activities><Activity Sport="Running"><Id>2022-07-16T16:08:25.000Z</Id>
  <Lap StartTime="2022-07-16T16:08:25.000Z"><TotalTimeSeconds>60</TotalTimeSeconds>
   <DistanceMeters>200.5</DistanceMeters><Calories>12</Calories>
   <MaximumHeartRateBpm><Value>150</Value></MaximumHeartRateBpm>
   <Track>
    <Trackpoint><Time>2022-07-16T16:08:25Z</Time><Position><LatitudeDegrees>46.5</LatitudeDegrees><LongitudeDegrees>16.1</LongitudeDegrees></Position><HeartRateBpm><Value>100</Value></HeartRateBpm><Cadence>80</Cadence></Trackpoint>
    <Trackpoint><Time>2022-07-16T16:08:35Z</Time><HeartRateBpm><Value>110</Value></HeartRateBpm></Trackpoint>
    <Trackpoint><Time>2022-07-16T16:09:25Z</Time><Position><LatitudeDegrees>46.6</LatitudeDegrees><LongitudeDegrees>16.2</LongitudeDegrees></Position><Extensions><ns3:TPX><ns3:Speed>3</ns3:Speed></ns3:TPX></Extensions></Trackpoint>
   </Track></Lap>
  <Creator><Name>Forerunner</Name></Creator></Activity></Activities>
 <Courses><Course><Name>Loop</Name><Track>
   <Trackpoint><Position><LatitudeDegrees>1</LatitudeDegrees><LongitudeDegrees>2</LongitudeDegrees></Position></Trackpoint>
   <Trackpoint><Position><LatitudeDegrees>3</LatitudeDegrees><LongitudeDegrees>4</LongitudeDegrees></Position></Trackpoint>
  </Track><CoursePoint><Name>Turn</Name><Time>2022-01-01T00:00:00Z</Time><Position><LatitudeDegrees>1</LatitudeDegrees><LongitudeDegrees>2</LongitudeDegrees></Position><PointType>Left</PointType></CoursePoint></Course></Courses>
</TrainingCenterDatabase>"#;

    #[test]
    fn activities_and_courses() {
        assert!(sniff(TCX.as_bytes()));
        let ds = read(TCX.as_bytes(), "x").unwrap();
        assert_eq!(ds.format, "TCX");
        let t = &ds.layers[0];
        assert_eq!(t.features.len(), 2);
        assert_eq!(
            t.features[0].geometry,
            Some(Geometry::LineString(vec![[16.1, 46.5], [16.2, 46.6]]))
        );
        let a: std::collections::HashMap<_, _> = t.attributes(0).into_iter().collect();
        assert_eq!(a["sport"], Value::Text("running".into()));
        assert_eq!(a["start"], Value::Date("2022-07-16T16:08:25Z".into()));
        assert_eq!(a["end"], Value::Date("2022-07-16T16:09:25Z".into()));
        assert_eq!(a["duration_s"], Value::Float(60.0));
        assert_eq!(a["distance_m"], Value::Float(200.5));
        assert_eq!(a["points"], Value::Int(2));
        assert_eq!(a["avg_hr"], Value::Float(105.0));
        assert_eq!(a["max_hr"], Value::Float(150.0));
        assert_eq!(a["calories"], Value::Float(12.0));
        assert_eq!(a["device"], Value::Text("Forerunner".into()));
        let b: std::collections::HashMap<_, _> = t.attributes(1).into_iter().collect();
        assert_eq!(b["name"], Value::Text("Loop".into()));
        assert_eq!(
            ds.layers[1].features[0].geometry,
            Some(Geometry::Point([2.0, 1.0]))
        );
        assert!(ds.warnings.iter().any(|w| w.starts_with("1 trackpoints")));
        for cut in [10, 200, TCX.len() / 2] {
            assert!(read(&TCX.as_bytes()[..cut], "x").is_err());
        }
    }
}
