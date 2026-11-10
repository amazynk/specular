//! Feature styles a file carries: GeoJSON simplestyle properties (`stroke`, `stroke-width`,
//! `fill`, `marker-color`… as attributes) and KML `Style`/`StyleMap` (by `styleUrl` or
//! inline: `LineStyle`, `PolyStyle`, `IconStyle`). The readers keep features only; a map view
//! asks here for the colours the file chose.

use std::collections::HashMap;

use roxmltree::Node;

use crate::model::Value;

/// One feature's style; `None` where the file says nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FeatureStyle {
    /// Lines and outlines, RGBA.
    pub stroke: Option<[u8; 4]>,
    /// Line width in CSS pixels.
    pub width_px: Option<f32>,
    /// Polygon fills, RGBA (alpha 0 for no fill).
    pub fill: Option<[u8; 4]>,
    /// Points, RGBA.
    pub marker: Option<[u8; 4]>,
    /// Point radius in CSS pixels.
    pub marker_px: Option<f32>,
}

impl FeatureStyle {
    pub fn is_empty(&self) -> bool {
        *self == FeatureStyle::default()
    }
}

/// `#rgb` or `#rrggbb` (the `#` optional).
fn css_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    let v = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
    match h.len() {
        3 => Some([v(0, 1)? * 17, v(1, 1)? * 17, v(2, 1)? * 17]),
        6 => Some([v(0, 2)?, v(2, 2)?, v(4, 2)?]),
        _ => None,
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Text(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn opacity(v: Option<&Value>) -> u8 {
    v.and_then(number)
        .map_or(255, |o| (o.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The simplestyle-spec 1.1 style of a feature, from its attributes (`fields` and `values`
/// aligned as in [`crate::model::Layer`]).
pub fn simplestyle(fields: &[String], values: &[Value]) -> Option<FeatureStyle> {
    let get = |k: &str| {
        fields
            .iter()
            .position(|f| f == k)
            .and_then(|i| values.get(i))
            .filter(|v| !matches!(v, Value::Null))
    };
    let colour = |k: &str| match get(k)? {
        Value::Text(s) => css_hex(s),
        _ => None,
    };
    let mut s = FeatureStyle::default();
    if let Some([r, g, b]) = colour("stroke") {
        s.stroke = Some([r, g, b, opacity(get("stroke-opacity"))]);
    }
    s.width_px = get("stroke-width")
        .and_then(number)
        .filter(|w| w.is_finite() && *w >= 0.0)
        .map(|w| w.min(50.0) as f32);
    if let Some([r, g, b]) = colour("fill") {
        // The spec's default fill opacity is 0.6.
        let a = get("fill-opacity")
            .and_then(number)
            .map_or(153, |o| (o.clamp(0.0, 1.0) * 255.0).round() as u8);
        s.fill = Some([r, g, b, a]);
    }
    if let Some([r, g, b]) = colour("marker-color") {
        s.marker = Some([r, g, b, 255]);
    }
    s.marker_px = match get("marker-size") {
        Some(Value::Text(t)) => match t.trim() {
            "small" => Some(3.0),
            "medium" => Some(4.5),
            "large" => Some(6.5),
            _ => None,
        },
        _ => None,
    };
    (!s.is_empty()).then_some(s)
}

/// KML `aabbggrr` as RGBA.
fn kml_colour(s: &str) -> Option<[u8; 4]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 8 {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some([v(6)?, v(4)?, v(2)?, v(0)?])
}

fn local<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn text<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    local(n, name).and_then(|c| c.text()).map(str::trim)
}

/// A `Style` element's look.
fn style_of(st: Node<'_, '_>) -> FeatureStyle {
    let mut s = FeatureStyle::default();
    if let Some(l) = local(st, "LineStyle") {
        s.stroke = text(l, "color").and_then(kml_colour);
        s.width_px = text(l, "width")
            .and_then(|w| w.parse::<f32>().ok())
            .filter(|w| w.is_finite() && *w >= 0.0)
            .map(|w| w.min(50.0));
    }
    if let Some(p) = local(st, "PolyStyle") {
        let on = |k: &str| text(p, k).is_none_or(|v| v != "0");
        s.fill = if on("fill") {
            text(p, "color").and_then(kml_colour)
        } else {
            Some([0, 0, 0, 0])
        };
        if !on("outline") {
            s.stroke = Some([0, 0, 0, 0]);
        }
    }
    if let Some(i) = local(st, "IconStyle") {
        s.marker = text(i, "color").and_then(kml_colour);
        s.marker_px = text(i, "scale")
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| (v * 4.0).clamp(1.5, 16.0));
    }
    s
}

/// Fields set in `over` win over `base`.
fn merge(base: FeatureStyle, over: FeatureStyle) -> FeatureStyle {
    FeatureStyle {
        stroke: over.stroke.or(base.stroke),
        width_px: over.width_px.or(base.width_px),
        fill: over.fill.or(base.fill),
        marker: over.marker.or(base.marker),
        marker_px: over.marker_px.or(base.marker_px),
    }
}

/// The styles of a KML document's placemarks, keyed by feature id as [`crate::kml::read`]
/// gives them (the placemark's `id`, else its number among the placemarks). Placemarks
/// without a style are left out. Only styles in the file itself are used (never fetched).
pub fn kml_styles(data: &[u8]) -> HashMap<String, FeatureStyle> {
    let mut out = HashMap::new();
    let Ok(src) = std::str::from_utf8(data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data)) else {
        return out;
    };
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    if !specular_limits::xml_within_depth(src.as_bytes(), specular_limits::MAX_XML_DEPTH) {
        return out;
    }
    let Ok(doc) = roxmltree::Document::parse_with_options(src, opts) else {
        return out;
    };
    let is = |n: &Node<'_, '_>, name: &str| n.is_element() && n.tag_name().name() == name;
    let mut styles: HashMap<&str, FeatureStyle> = HashMap::new();
    for st in doc.descendants().filter(|n| is(n, "Style")) {
        if let Some(id) = st.attribute("id") {
            styles.insert(id, style_of(st));
        }
    }
    // StyleMap: its "normal" pair.
    let mut maps: HashMap<&str, &str> = HashMap::new();
    for sm in doc.descendants().filter(|n| is(n, "StyleMap")) {
        let Some(id) = sm.attribute("id") else {
            continue;
        };
        let normal = sm
            .children()
            .filter(|p| is(p, "Pair"))
            .find(|p| text(*p, "key") == Some("normal"))
            .and_then(|p| text(p, "styleUrl"));
        if let Some(u) = normal {
            maps.insert(id, u.trim_start_matches('#'));
        }
    }
    let resolve = |url: &str| {
        let id = url.trim().trim_start_matches('#');
        let id = maps.get(id).copied().unwrap_or(id);
        styles.get(id).copied()
    };
    // The same placemarks, in the same order, as the reader's.
    let placemarks = doc.descendants().filter(|n| {
        is(n, "Placemark")
            && n.ancestors().skip(1).all(|a| {
                !a.is_element() || matches!(a.tag_name().name(), "Folder" | "Document" | "kml")
            })
    });
    for (count, pm) in placemarks.enumerate() {
        let shared = text(pm, "styleUrl").and_then(resolve).unwrap_or_default();
        let inline = local(pm, "Style").map(style_of).unwrap_or_default();
        let s = merge(shared, inline);
        if s.is_empty() {
            continue;
        }
        let id = pm
            .attribute("id")
            .map(str::to_string)
            .unwrap_or(count.to_string());
        out.insert(id, s);
    }
    out
}

/// The styles of the KML inside a KMZ ([`kml_styles`]); empty when it can't be read.
pub fn kmz_styles(data: &[u8]) -> HashMap<String, FeatureStyle> {
    const MAX_KML: u64 = 1 << 30;
    let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(data)) else {
        return HashMap::new();
    };
    let names: Vec<String> = z.file_names().map(str::to_string).collect();
    let Some(pick) = names
        .iter()
        .find(|n| n.eq_ignore_ascii_case("doc.kml"))
        .or_else(|| {
            names
                .iter()
                .find(|n| n.to_ascii_lowercase().ends_with(".kml"))
        })
        .cloned()
    else {
        return HashMap::new();
    };
    let Ok(f) = z.by_name(&pick) else {
        return HashMap::new();
    };
    let mut buf = Vec::new();
    if std::io::Read::read_to_end(&mut std::io::Read::take(f, MAX_KML + 1), &mut buf).is_err()
        || buf.len() as u64 > MAX_KML
    {
        return HashMap::new();
    }
    kml_styles(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplestyle_properties() {
        let fields: Vec<String> = [
            "name",
            "stroke",
            "stroke-width",
            "stroke-opacity",
            "fill",
            "marker-color",
            "marker-size",
        ]
        .map(String::from)
        .to_vec();
        let values = vec![
            Value::Text("a".into()),
            Value::Text("#ff0000".into()),
            Value::Float(3.0),
            Value::Float(0.5),
            Value::Text("#0f0".into()),
            Value::Text("#0000ff".into()),
            Value::Text("large".into()),
        ];
        let s = simplestyle(&fields, &values).unwrap();
        assert_eq!(s.stroke, Some([255, 0, 0, 128]));
        assert_eq!(s.width_px, Some(3.0));
        assert_eq!(s.fill, Some([0, 255, 0, 153]));
        assert_eq!(s.marker, Some([0, 0, 255, 255]));
        assert_eq!(s.marker_px, Some(6.5));
        assert_eq!(simplestyle(&fields[..1], &values[..1]), None);
    }

    #[test]
    fn kml_shared_inline_and_mapped_styles() {
        let kml = br##"<?xml version="1.0"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <Style id="red"><LineStyle><color>ff0000ff</color><width>4</width></LineStyle>
    <PolyStyle><color>7f00ff00</color></PolyStyle></Style>
  <Style id="blue"><IconStyle><color>ffff0000</color><scale>2</scale></IconStyle></Style>
  <StyleMap id="m"><Pair><key>normal</key><styleUrl>#blue</styleUrl></Pair>
    <Pair><key>highlight</key><styleUrl>#red</styleUrl></Pair></StyleMap>
  <Placemark><styleUrl>#red</styleUrl><LineString><coordinates>0,0 1,1</coordinates></LineString></Placemark>
  <Placemark id="p"><styleUrl>#m</styleUrl><Point><coordinates>0,0</coordinates></Point></Placemark>
  <Folder><Placemark><styleUrl>#red</styleUrl>
    <Style><LineStyle><width>1.5</width></LineStyle></Style>
    <LineString><coordinates>0,0 1,1</coordinates></LineString></Placemark></Folder>
  <Placemark><Point><coordinates>0,0</coordinates></Point></Placemark>
</Document></kml>"##;
        let s = kml_styles(kml);
        assert_eq!(s.len(), 3);
        assert_eq!(s["0"].stroke, Some([255, 0, 0, 255]));
        assert_eq!(s["0"].width_px, Some(4.0));
        assert_eq!(s["0"].fill, Some([0, 255, 0, 127]));
        assert_eq!(s["p"].marker, Some([0, 0, 255, 255]));
        assert_eq!(s["p"].marker_px, Some(8.0));
        // Inline over shared.
        assert_eq!(s["2"].width_px, Some(1.5));
        assert_eq!(s["2"].stroke, Some([255, 0, 0, 255]));
        // The ids agree with the reader's.
        let ds = crate::kml::read(kml, "t").unwrap();
        let ids: Vec<_> = ds
            .layers
            .iter()
            .flat_map(|l| &l.features)
            .filter_map(|f| f.id.clone())
            .collect();
        for k in s.keys() {
            assert!(ids.contains(k), "{k} not in {ids:?}");
        }
    }
}
