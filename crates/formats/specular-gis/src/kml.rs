//! KML 2.2 and KMZ: placemarks with Point, LineString, LinearRing, Polygon, MultiGeometry
//! and gx:Track/gx:MultiTrack geometry, their name, description and ExtendedData. A layer
//! per Folder (or Document) that holds placemarks directly. Styles are not applied; map
//! layers are drawn in the viewer's own colours. Network links are never followed.
//! gx:Track placemarks also get track_start, track_end and track_points attributes.
//! GroundOverlays are collected into [`Dataset::overlays`] (box, rotation or gx:LatLonQuad
//! and the image reference); [`kmz_member`] reads a referenced image out of a KMZ.

use crate::crs;
use crate::fit::parse_iso;
use crate::model::{Dataset, Geometry, GroundOverlay, Layer, MAX_FEATURES, Pt, Value};
use roxmltree::Node;

/// Does this look like KML?
pub fn sniff(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    s.contains("<kml") && (s.contains("opengis.net/kml") || s.contains("earth.google.com/kml"))
}

/// Is this a KMZ (a ZIP with a .kml inside)? Checks the first local header's name.
pub fn sniff_kmz(head: &[u8]) -> bool {
    if !head.starts_with(b"PK\x03\x04") || head.len() < 30 {
        return false;
    }
    let n = usize::from(u16::from_le_bytes([head[26], head[27]]));
    head.get(30..30 + n)
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(b".kml"))
}

/// The largest KML read out of a KMZ.
const MAX_KML: u64 = 1 << 30;

pub fn read_kmz(data: &[u8], layer_name: &str) -> Result<Dataset, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|e| format!("not a KMZ archive: {e}"))?;
    let names: Vec<String> = z.file_names().flatten().map(String::from).collect();
    let pick = names
        .iter()
        .find(|n| n.eq_ignore_ascii_case("doc.kml"))
        .or_else(|| {
            names
                .iter()
                .find(|n| n.to_ascii_lowercase().ends_with(".kml"))
        })
        .ok_or("the KMZ holds no .kml")?
        .clone();
    let f = z.by_name(&pick).map_err(|e| e.to_string())?;
    if f.size() > MAX_KML {
        return Err("the KML inside the KMZ is too large".into());
    }
    let reserve = f.size().min(f.compressed_size().saturating_mul(1032));
    let buf = read_capped(f, reserve, MAX_KML)?;
    let mut ds = read(&buf, layer_name)?;
    ds.format = "KMZ";
    Ok(ds)
}

/// Reads an archive member of at most `cap` bytes. Its stated size is not trusted: a
/// forged one with a deflate bomb behind it inflates (up to ~1032:1) only until the cap,
/// and `reserve` is what the compressed data can expand to.
/// The largest KMZ member [`kmz_member`] reads.
const MAX_MEMBER: u64 = 256 << 20;

/// A KMZ path normalised: backslashes as slashes, `.` and `..` resolved, lower case.
fn norm_path(s: &str) -> String {
    let s = s.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for p in s.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/").to_ascii_lowercase()
}

/// A member of a KMZ by the path a KML gives (case-insensitive; `./`, `../` and
/// backslashes normalised), at most 256 MB.
pub fn kmz_member(data: &[u8], name: &str) -> Option<Vec<u8>> {
    let want = norm_path(name);
    if want.is_empty() {
        return None;
    }
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).ok()?;
    let pick = z
        .file_names()
        .flatten()
        .find(|n| norm_path(n) == want)
        .map(String::from)?;
    let f = z.by_name(&pick).ok()?;
    if f.size() > MAX_MEMBER {
        return None;
    }
    let reserve = f.size().min(f.compressed_size().saturating_mul(1032));
    read_capped(f, reserve, MAX_MEMBER).ok()
}

fn read_capped(r: impl std::io::Read, reserve: u64, cap: u64) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(reserve.min(cap) as usize);
    std::io::Read::read_to_end(&mut r.take(cap + 1), &mut buf).map_err(|e| e.to_string())?;
    if buf.len() as u64 > cap {
        return Err("the KML inside the KMZ is too large".into());
    }
    Ok(buf)
}

pub fn read(data: &[u8], layer_name: &str) -> Result<Dataset, String> {
    let text = std::str::from_utf8(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data))
        .map_err(|_| "the KML is not UTF-8")?;
    let opts = || roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    if !specular_limits::xml_within_depth(text.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return Err("the XML nests too deeply".into());
    }
    // Content after the document's closing tag (written by some tools; GDAL reads these):
    // the document up to it, with a warning.
    let (doc, trailing) = match roxmltree::Document::parse_with_options(text, opts()) {
        Ok(d) => (d, false),
        Err(e) => {
            let whole = text
                .find("</kml>")
                .map(|i| &text[..i + "</kml>".len()])
                .filter(|t| t.len() < text.trim_end().len())
                .and_then(|t| roxmltree::Document::parse_with_options(t, opts()).ok());
            (whole.ok_or_else(|| format!("not valid XML: {e}"))?, true)
        }
    };
    let mut ds = Dataset {
        format: "KML",
        crs: crs::from_epsg(4326),
        ..Dataset::default()
    };
    if trailing {
        ds.warn("content after the end of the KML document was ignored");
    }
    let mut layers: Vec<Layer> = Vec::new();
    // The container node of each layer: folders of the same name stay apart.
    let mut containers: Vec<Option<roxmltree::NodeId>> = Vec::new();
    // Placemarks inside a geometry (broken files) are not features of their own.
    let placemarks = doc.descendants().filter(|n| {
        n.has_tag_name_local("Placemark")
            && n.ancestors().skip(1).all(|a| {
                !a.is_element() || matches!(a.tag_name().name(), "Folder" | "Document" | "kml")
            })
    });
    for (count, pm) in placemarks.enumerate() {
        if count >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        // The layer: the nearest Folder or Document.
        let container = pm
            .ancestors()
            .skip(1)
            .find(|a| a.has_tag_name_local("Folder") || a.has_tag_name_local("Document"));
        let key = container.map(|c| c.id());
        let li = match containers.iter().position(|&k| k == key) {
            Some(i) => i,
            None => {
                let base = container
                    .and_then(|c| child_text(c, "name"))
                    .unwrap_or_else(|| layer_name.to_string());
                let mut lname = base.clone();
                let mut n = 1;
                while layers.iter().any(|l| l.name == lname) {
                    n += 1;
                    lname = format!("{base} (#{n})");
                }
                containers.push(key);
                let mut l = Layer::new(lname);
                l.field("Name");
                l.field("Description");
                layers.push(l);
                layers.len() - 1
            }
        };
        let mut attrs = vec![
            ("Name".to_string(), text_value(child_text(pm, "name"))),
            (
                "Description".to_string(),
                text_value(child_markup(pm, "description")),
            ),
        ];
        if let Some(ext) = child(pm, "ExtendedData") {
            for d in ext.descendants() {
                if d.has_tag_name_local("Data") {
                    let v = child_text(d, "value");
                    if let Some(n) = d.attribute("name") {
                        attrs.push((n.to_string(), text_value(v)));
                    }
                } else if d.has_tag_name_local("SimpleData")
                    && let Some(n) = d.attribute("name")
                {
                    attrs.push((n.to_string(), text_value(d.text().map(str::to_string))));
                }
            }
        }
        for k in ["TimeStamp", "TimeSpan"] {
            if let Some(t) = child(pm, k) {
                for (tag, field) in [("when", "Time"), ("begin", "Begin"), ("end", "End")] {
                    if let Some(v) = child_text(t, tag) {
                        attrs.push((field.to_string(), Value::Date(v)));
                    }
                }
            }
        }
        // gx:Track summaries: when the track starts and ends, and its point count.
        let tracks: Vec<Node<'_, '_>> = pm
            .descendants()
            .filter(|d| d.has_tag_name_local("Track"))
            .collect();
        if !tracks.is_empty() {
            let mut first: Option<(f64, String)> = None;
            let mut last: Option<(f64, String)> = None;
            let mut n = 0usize;
            for t in &tracks {
                n += t
                    .children()
                    .filter(|c| c.has_tag_name_local("coord"))
                    .count();
                for w in t.children().filter(|c| c.has_tag_name_local("when")) {
                    let Some(s) = w.text().map(str::trim) else {
                        continue;
                    };
                    let Some(v) = parse_iso(s) else { continue };
                    if first.as_ref().is_none_or(|f| v < f.0) {
                        first = Some((v, s.to_string()));
                    }
                    if last.as_ref().is_none_or(|l| v > l.0) {
                        last = Some((v, s.to_string()));
                    }
                }
            }
            let date = |x: Option<(f64, String)>| x.map_or(Value::Null, |x| Value::Date(x.1));
            attrs.push(("track_start".to_string(), date(first)));
            attrs.push(("track_end".to_string(), date(last)));
            attrs.push(("track_points".to_string(), Value::Int(n as i64)));
        }
        let geoms: Vec<Geometry> = pm.children().filter_map(|c| geometry(c, 0)).collect();
        let geom = match geoms.len() {
            0 => None,
            1 => geoms.into_iter().next(),
            _ => Some(Geometry::Collection(geoms)),
        };
        let id = pm
            .attribute("id")
            .map(str::to_string)
            .unwrap_or(count.to_string());
        layers[li].push(Some(id), geom, attrs);
    }
    if layers.is_empty() {
        layers.push(Layer::new(layer_name));
    }
    if doc
        .descendants()
        .any(|n| n.has_tag_name_local("NetworkLink"))
    {
        ds.warn("network links are not followed");
    }
    for go in doc
        .descendants()
        .filter(|n| n.has_tag_name_local("GroundOverlay"))
    {
        if ds.overlays.len() >= 4096 {
            break;
        }
        if let Some(o) = ground_overlay(go) {
            ds.overlays.push(o);
        }
    }
    if doc
        .descendants()
        .any(|n| n.has_tag_name_local("GroundOverlay"))
    {
        ds.warn("ground overlays are not shown");
    }
    ds.layers = layers;
    Ok(ds)
}

/// A GroundOverlay's image reference and placement; `None` without an image or a place.
fn ground_overlay(n: Node<'_, '_>) -> Option<GroundOverlay> {
    let href = child(n, "Icon").and_then(|i| child_text(i, "href"))?;
    let num = |p: Node<'_, '_>, k: &str| child_text(p, k).and_then(|s| s.parse::<f64>().ok());
    let quad = n
        .children()
        .find(|c| c.has_tag_name_local("LatLonQuad"))
        .map(coords_of)
        .filter(|c| c.len() >= 4)
        .map(|c| [c[0], c[1], c[2], c[3]]);
    let (north, south, east, west, rotation);
    if let Some(b) = child(n, "LatLonBox") {
        north = num(b, "north")?;
        south = num(b, "south")?;
        east = num(b, "east")?;
        west = num(b, "west")?;
        rotation = num(b, "rotation").unwrap_or(0.0);
    } else {
        let q = quad?;
        north = q.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
        south = q.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        east = q.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        west = q.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        rotation = 0.0;
    }
    if ![north, south, east, west, rotation]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    Some(GroundOverlay {
        name: child_text(n, "name"),
        href,
        north,
        south,
        east,
        west,
        rotation,
        quad,
    })
}

trait Local {
    fn has_tag_name_local(&self, n: &str) -> bool;
}

impl Local for Node<'_, '_> {
    fn has_tag_name_local(&self, n: &str) -> bool {
        self.is_element() && self.tag_name().name() == n
    }
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.has_tag_name_local(name))
}

fn child_text(n: Node<'_, '_>, name: &str) -> Option<String> {
    let c = child(n, name)?;
    let s: String = c
        .descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect();
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// An element's content with any markup kept as written: descriptions are HTML, mostly in
/// CDATA but sometimes as plain elements.
fn child_markup(n: Node<'_, '_>, name: &str) -> Option<String> {
    let c = child(n, name)?;
    if !c.children().any(|d| d.is_element()) {
        return child_text(n, name);
    }
    let (first, last) = (c.first_child()?, c.last_child()?);
    let src = c.document().input_text();
    let s = src.get(first.range().start..last.range().end)?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn text_value(s: Option<String>) -> Value {
    s.map_or(Value::Null, Value::Text)
}

/// "lon,lat[,alt] lon,lat[,alt] ..."
fn coords(s: &str) -> Vec<Pt> {
    s.split_ascii_whitespace()
        .filter_map(|t| {
            let mut it = t.split(',');
            let x = it.next()?.parse::<f64>().ok()?;
            let y = it.next()?.parse::<f64>().ok()?;
            Some([x, y])
        })
        .collect()
}

fn coords_of(n: Node<'_, '_>) -> Vec<Pt> {
    child_text(n, "coordinates")
        .map(|s| coords(&s))
        .unwrap_or_default()
}

fn ring_of(n: Node<'_, '_>) -> Option<Vec<Pt>> {
    let lr = child(n, "LinearRing")?;
    let r = coords_of(lr);
    (!r.is_empty()).then_some(r)
}

fn geometry(n: Node<'_, '_>, depth: usize) -> Option<Geometry> {
    if !n.is_element() || depth > 32 {
        return None;
    }
    match n.tag_name().name() {
        "Point" => coords_of(n).first().map(|p| Geometry::Point(*p)),
        "LineString" | "LinearRing" => {
            let c = coords_of(n);
            (!c.is_empty()).then_some(Geometry::LineString(c))
        }
        "Polygon" => {
            let mut rings = Vec::new();
            rings.extend(child(n, "outerBoundaryIs").and_then(ring_of));
            if rings.is_empty() {
                return None;
            }
            for inner in n
                .children()
                .filter(|c| c.has_tag_name_local("innerBoundaryIs"))
            {
                // KML 2.2 puts one ring per innerBoundaryIs; some writers put several.
                for lr in inner
                    .children()
                    .filter(|c| c.has_tag_name_local("LinearRing"))
                {
                    let r = coords_of(lr);
                    if !r.is_empty() {
                        rings.push(r);
                    }
                }
            }
            Some(Geometry::Polygon(rings))
        }
        // MultiPolygon, MultiLineString and MultiPoint are not KML, but writers use them.
        "MultiGeometry" | "MultiTrack" | "MultiPolygon" | "MultiLineString" | "MultiPoint" => {
            let g: Vec<Geometry> = n
                .children()
                .filter_map(|c| geometry(c, depth + 1))
                .collect();
            simplify(g)
        }
        "Track" => {
            let pts: Vec<Pt> = n
                .children()
                .filter(|c| c.has_tag_name_local("coord"))
                .filter_map(|c| {
                    let mut it = c.text()?.split_ascii_whitespace();
                    Some([it.next()?.parse().ok()?, it.next()?.parse().ok()?])
                })
                .collect();
            (!pts.is_empty()).then_some(Geometry::LineString(pts))
        }
        "Model" => {
            let loc = child(n, "Location")?;
            let x = child_text(loc, "longitude")?.parse().ok()?;
            let y = child_text(loc, "latitude")?.parse().ok()?;
            Some(Geometry::Point([x, y]))
        }
        _ => None,
    }
}

/// A homogeneous collection becomes the matching Multi type.
pub fn simplify(g: Vec<Geometry>) -> Option<Geometry> {
    if g.is_empty() {
        return None;
    }
    if g.iter().all(|g| matches!(g, Geometry::Point(_))) {
        return Some(Geometry::MultiPoint(
            g.into_iter()
                .filter_map(|g| match g {
                    Geometry::Point(p) => Some(p),
                    _ => None,
                })
                .collect(),
        ));
    }
    if g.iter().all(|g| matches!(g, Geometry::LineString(_))) {
        return Some(Geometry::MultiLineString(
            g.into_iter()
                .filter_map(|g| match g {
                    Geometry::LineString(p) => Some(p),
                    _ => None,
                })
                .collect(),
        ));
    }
    if g.iter().all(|g| matches!(g, Geometry::Polygon(_))) {
        return Some(Geometry::MultiPolygon(
            g.into_iter()
                .filter_map(|g| match g {
                    Geometry::Polygon(p) => Some(p),
                    _ => None,
                })
                .collect(),
        ));
    }
    Some(Geometry::Collection(g))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
<Document><name>Doc</name>
  <Placemark><name>Top</name><Point><coordinates>8.5,47.3,400</coordinates></Point></Placemark>
  <Folder><name>Paths</name>
    <Placemark id="p1"><name>Walk</name><description><![CDATA[<b>nice</b>]]></description>
      <ExtendedData><Data name="len"><value>12</value></Data>
        <SchemaData schemaUrl="#s"><SimpleData name="kind">trail</SimpleData></SchemaData></ExtendedData>
      <LineString><coordinates>
        8.0,47.0 8.1,47.1
        8.2,47.0</coordinates></LineString></Placemark>
    <Placemark><name>Area</name><MultiGeometry>
      <Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 4,0 4,4 0,4 0,0</coordinates></LinearRing></outerBoundaryIs>
        <innerBoundaryIs><LinearRing><coordinates>1,1 2,1 2,2 1,1</coordinates></LinearRing></innerBoundaryIs></Polygon>
      <Point><coordinates>2,2</coordinates></Point></MultiGeometry></Placemark>
    <Placemark><gx:Track><when>2020-01-01T00:00:00Z</when><when>2020-01-01T00:05:00Z</when><gx:coord>1 2 3</gx:coord><gx:coord>1.5 2.5 3</gx:coord></gx:Track></Placemark>
    <Placemark><gx:MultiTrack><gx:Track><gx:coord>1 2 3</gx:coord><gx:coord>2 2 3</gx:coord></gx:Track><gx:Track><gx:coord>3 2 3</gx:coord><gx:coord>4 2 3</gx:coord></gx:Track></gx:MultiTrack></Placemark>
  </Folder>
  <GroundOverlay><name>Scan</name><Icon><href>files/scan.png</href></Icon>
    <LatLonBox><north>47.5</north><south>47.0</south><east>8.5</east><west>8.0</west><rotation>10</rotation></LatLonBox></GroundOverlay>
  <GroundOverlay><Icon><href>q.jpg</href></Icon>
    <gx:LatLonQuad><coordinates>1,1 3,1 3,4 1,3</coordinates></gx:LatLonQuad></GroundOverlay>
  <GroundOverlay><name>No image</name><LatLonBox><north>1</north><south>0</south><east>1</east><west>0</west></LatLonBox></GroundOverlay>
</Document></kml>"##;

    /// Content after the closing tag is ignored with a warning; a broken document stays an
    /// error.
    #[test]
    fn trailing_content_is_ignored() {
        let clean = read(KML.as_bytes(), "a").unwrap();
        let junk = format!("{KML}\n   <kml>");
        let ds = read(junk.as_bytes(), "a").unwrap();
        assert_eq!(ds.feature_count(), clean.feature_count());
        assert!(
            ds.warnings.iter().any(|w| w.contains("ignored")),
            "{:?}",
            ds.warnings
        );
        assert!(!clean.warnings.iter().any(|w| w.contains("ignored")));
        let cut = &KML[..KML.len() - 30];
        assert!(
            read(cut.as_bytes(), "a")
                .unwrap_err()
                .starts_with("not valid XML")
        );
    }

    #[test]
    fn placemarks() {
        assert!(sniff(KML.as_bytes()));
        let d = read(KML.as_bytes(), "x").unwrap();
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.layers[0].name, "Doc");
        let p = &d.layers[1];
        assert_eq!(p.name, "Paths");
        assert_eq!(p.features.len(), 4);
        assert_eq!(
            p.fields(),
            [
                "Name",
                "Description",
                "len",
                "kind",
                "track_start",
                "track_end",
                "track_points"
            ]
        );
        assert_eq!(p.features[0].values[1], Value::Text("<b>nice</b>".into()));
        assert_eq!(p.features[0].values[3], Value::Text("trail".into()));
        assert_eq!(
            p.features[0].geometry,
            Some(Geometry::LineString(vec![
                [8.0, 47.0],
                [8.1, 47.1],
                [8.2, 47.0]
            ]))
        );
        match p.features[1].geometry.as_ref().unwrap() {
            Geometry::Collection(g) => {
                assert!(matches!(&g[0], Geometry::Polygon(r) if r.len() == 2));
            }
            g => panic!("{g:?}"),
        }
        assert_eq!(
            p.features[2].geometry,
            Some(Geometry::LineString(vec![[1.0, 2.0], [1.5, 2.5]]))
        );
        assert_eq!(
            p.features[2].values[4],
            Value::Date("2020-01-01T00:00:00Z".into())
        );
        assert_eq!(
            p.features[2].values[5],
            Value::Date("2020-01-01T00:05:00Z".into())
        );
        assert_eq!(p.features[2].values[6], Value::Int(2));
        assert!(
            matches!(&p.features[3].geometry, Some(Geometry::MultiLineString(l)) if l.len() == 2)
        );
        assert_eq!(p.features[3].values[6], Value::Int(4));
        assert_eq!(d.overlays.len(), 2);
        let o = &d.overlays[0];
        assert_eq!(o.name.as_deref(), Some("Scan"));
        assert_eq!(o.href, "files/scan.png");
        assert_eq!(
            (o.north, o.south, o.east, o.west, o.rotation),
            (47.5, 47.0, 8.5, 8.0, 10.0)
        );
        assert_eq!(o.quad, None);
        let q = &d.overlays[1];
        assert_eq!(
            q.quad,
            Some([[1.0, 1.0], [3.0, 1.0], [3.0, 4.0], [1.0, 3.0]])
        );
        assert_eq!((q.north, q.south, q.east, q.west), (4.0, 1.0, 3.0, 1.0));
    }

    #[test]
    fn kmz_members() {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("doc.kml", o).unwrap();
        z.write_all(KML.as_bytes()).unwrap();
        z.start_file("Files/Scan.PNG", o).unwrap();
        z.write_all(b"png").unwrap();
        let data = z.finish().unwrap().into_inner();
        let png = Some(&b"png"[..]);
        assert_eq!(kmz_member(&data, "files/scan.png").as_deref(), png);
        assert_eq!(
            kmz_member(&data, "./files/../files/scan.png").as_deref(),
            png
        );
        assert_eq!(kmz_member(&data, "files\\scan.png").as_deref(), png);
        assert!(kmz_member(&data, "missing.png").is_none());
        assert!(kmz_member(b"junk", "a").is_none());
        let d = read_kmz(&data, "x").unwrap();
        assert_eq!(d.overlays.len(), 2);
    }
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A deflate bomb behind a forged size stops at the cap.
    #[test]
    fn kmz_member_inflating_past_the_cap() {
        let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        std::io::Write::write_all(&mut enc, &vec![b' '; 4 << 20]).unwrap();
        let bomb = enc.finish().unwrap();
        let dec = flate2::read::DeflateDecoder::new(&bomb[..]);
        assert!(read_capped(dec, 10, 1 << 20).is_err());
        let dec = flate2::read::DeflateDecoder::new(&bomb[..]);
        assert_eq!(read_capped(dec, 10, 4 << 20).unwrap().len(), 4 << 20);
    }

    #[test]
    fn kmz_garbage() {
        assert!(read_kmz(b"PK\x03\x04 not really a zip", "x").is_err());
    }
}
