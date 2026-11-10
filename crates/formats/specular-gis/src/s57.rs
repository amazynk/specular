//! S-57 electronic navigational charts (ENC cells, `.000`): ISO 8211 records of vector
//! primitives (isolated and connected nodes, edges; soundings as 3-D node clouds) and of
//! features (an object class, attributes, and pointers to the primitives they are built
//! from). Each object class is a layer named by its acronym (`DEPARE`, `LNDARE`,
//! `SOUNDG`…), as GDAL lays them out, with the dataset's own facts in a `DSID` layer.
//! Lines are chained from their edges, areas closed into rings from theirs. Update files
//! (`.001`…) are not applied, and S-52 symbology is not drawn.

use std::collections::HashMap;

use crate::crs;
use crate::iso8211::{self, Module, Record, Sub, UT};
use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value};
use crate::s57_catalogue::{ATTRIBUTES, OBJECTS};
use crate::style::FeatureStyle;

/// Whether the bytes start an S-57 cell: an ISO 8211 descriptive record (`3LE1`) whose
/// directory names the dataset identification field.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 64
        && head[..5].iter().all(u8::is_ascii_digit)
        && &head[5..9] == b"3LE1"
        && head[24..head.len().min(1024)]
            .windows(4)
            .any(|w| w == b"DSID")
}

/// An object class's acronym.
pub fn object_acronym(code: u16) -> Option<&'static str> {
    OBJECTS
        .binary_search_by_key(&code, |o| o.0)
        .ok()
        .map(|i| OBJECTS[i].1)
}

/// An object class's name.
pub fn object_name(code: u16) -> Option<&'static str> {
    OBJECTS
        .binary_search_by_key(&code, |o| o.0)
        .ok()
        .map(|i| OBJECTS[i].2)
}

fn attribute(code: u16) -> Option<(&'static str, u8)> {
    ATTRIBUTES
        .binary_search_by_key(&code, |a| a.0)
        .ok()
        .map(|i| (ATTRIBUTES[i].1, ATTRIBUTES[i].3))
}

/// A record name: (RCNM, RCID) from a 5-byte `B(40)`.
fn name(b: &[u8]) -> Option<(u8, u32)> {
    (b.len() == 5).then(|| (b[0], u32::from_le_bytes([b[1], b[2], b[3], b[4]])))
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// A vector primitive.
#[derive(Default)]
struct Vector {
    /// x, y and (soundings) depth.
    pts: Vec<[f64; 3]>,
    /// An edge's begin and end nodes.
    begin: Option<(u8, u32)>,
    end: Option<(u8, u32)>,
}

struct Reader<'m> {
    module: &'m Module,
    comf: f64,
    somf: f64,
    vectors: HashMap<(u8, u32), Vector>,
}

impl Reader<'_> {
    fn groups<'a>(&'a self, rec: &Record<'a>, tag: &str) -> Vec<Vec<(&'a str, Sub<'a>)>> {
        match (self.module.fields.get(tag), rec.field(tag)) {
            (Some(d), Some(f)) => d.groups(f, UT),
            _ => Vec::new(),
        }
    }

    fn vector(&mut self, rec: &Record<'_>) {
        let Some(g) = self.groups(rec, "VRID").into_iter().next() else {
            return;
        };
        let get = |k: &str| g.iter().find(|(l, _)| *l == k).and_then(|(_, v)| v.int());
        let (Some(rcnm), Some(rcid)) = (get("RCNM"), get("RCID")) else {
            return;
        };
        // Update instructions (delete 2, modify 3) are not applied: what a base cell holds
        // stays.
        let key = (rcnm as u8, rcid as u32);
        if get("RUIN").is_some_and(|u| u != 1)
            && (get("RUIN") == Some(2) || self.vectors.contains_key(&key))
        {
            return;
        }
        let mut v = Vector::default();
        for g in self.groups(rec, "SG2D") {
            let y = g.first().and_then(|(_, v)| v.real());
            let x = g.get(1).and_then(|(_, v)| v.real());
            if let (Some(x), Some(y)) = (x, y) {
                v.pts.push([x / self.comf, y / self.comf, f64::NAN]);
            }
        }
        for g in self.groups(rec, "SG3D") {
            let c = |i: usize| g.get(i).and_then(|(_, v)| v.real());
            if let (Some(y), Some(x), Some(z)) = (c(0), c(1), c(2)) {
                v.pts.push([x / self.comf, y / self.comf, z / self.somf]);
            }
        }
        for g in self.groups(rec, "VRPT") {
            let n = g
                .iter()
                .find(|(l, _)| *l == "NAME")
                .and_then(|(_, v)| name(v.bytes()));
            let topi = g
                .iter()
                .find(|(l, _)| *l == "TOPI")
                .and_then(|(_, v)| v.int());
            match topi {
                Some(1) => v.begin = n,
                Some(2) => v.end = n,
                _ => {}
            }
        }
        if self.vectors.len() < MAX_VERTICES {
            self.vectors.insert(key, v);
        }
    }

    /// An edge's points from begin to end node.
    fn edge(&self, key: (u8, u32)) -> Option<Vec<Pt>> {
        let e = self.vectors.get(&key)?;
        let node = |k: Option<(u8, u32)>| {
            self.vectors
                .get(&k?)
                .and_then(|n| n.pts.first())
                .map(|p| [p[0], p[1]])
        };
        let mut out = Vec::with_capacity(e.pts.len() + 2);
        out.extend(node(e.begin));
        out.extend(e.pts.iter().map(|p| [p[0], p[1]]));
        out.extend(node(e.end));
        (out.len() >= 2).then_some(out)
    }

    /// The feature's spatial pointers: (name, reversed, usage).
    fn pointers(&self, rec: &Record<'_>) -> Vec<((u8, u32), bool, i64)> {
        self.groups(rec, "FSPT")
            .into_iter()
            .filter_map(|g| {
                let get = |k: &str| g.iter().find(|(l, _)| *l == k).map(|(_, v)| v.clone());
                let n = name(get("NAME")?.bytes())?;
                let rev = get("ORNT").and_then(|v| v.int()) == Some(2);
                let usage = get("USAG").and_then(|v| v.int()).unwrap_or(1);
                Some((n, rev, usage))
            })
            .collect()
    }

    fn geometry(&self, prim: i64, rec: &Record<'_>) -> (Option<Geometry>, Vec<f64>) {
        let ptrs = self.pointers(rec);
        match prim {
            1 => {
                let mut pts = Vec::new();
                let mut depths = Vec::new();
                for (k, _, _) in &ptrs {
                    if let Some(v) = self.vectors.get(k) {
                        for p in &v.pts {
                            pts.push([p[0], p[1]]);
                            if p[2].is_finite() {
                                depths.push(p[2]);
                            }
                        }
                    }
                }
                let g = match pts.len() {
                    0 => None,
                    1 if depths.is_empty() => Some(Geometry::Point(pts[0])),
                    _ => Some(Geometry::MultiPoint(pts)),
                };
                (g, depths)
            }
            2 => {
                let mut parts: Vec<Vec<Pt>> = Vec::new();
                for (k, rev, _) in &ptrs {
                    let Some(mut e) = self.edge(*k) else { continue };
                    if *rev {
                        e.reverse();
                    }
                    match parts.last_mut() {
                        Some(last) if last.last() == e.first() => last.extend(&e[1..]),
                        _ => parts.push(e),
                    }
                }
                let g = match parts.len() {
                    0 => None,
                    1 => parts.pop().map(Geometry::LineString),
                    _ => Some(Geometry::MultiLineString(parts)),
                };
                (g, Vec::new())
            }
            3 => {
                let edges: Vec<Vec<Pt>> = ptrs
                    .iter()
                    .filter_map(|(k, rev, _)| {
                        let mut e = self.edge(*k)?;
                        if *rev {
                            e.reverse();
                        }
                        Some(e)
                    })
                    .collect();
                let mut rings = rings(edges);
                if rings.is_empty() {
                    return (None, Vec::new());
                }
                // The largest ring is the outside; the others are holes.
                let big = (0..rings.len())
                    .max_by(|&a, &b| area(&rings[a]).abs().total_cmp(&area(&rings[b]).abs()))
                    .unwrap_or(0);
                let outer = rings.remove(big);
                rings.insert(0, outer);
                (Some(Geometry::Polygon(rings)), Vec::new())
            }
            _ => (None, Vec::new()),
        }
    }
}

/// Twice the signed area of a ring.
fn area(r: &[Pt]) -> f64 {
    r.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum()
}

/// Closes edges into rings: each ring follows edges end to start (an edge may be run
/// backwards), until it comes back to its first point.
fn rings(mut edges: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    let mut out = Vec::new();
    while let Some(first) = edges.first().cloned() {
        edges.swap_remove(0);
        let mut ring = first;
        while ring.first() != ring.last() {
            let end = *ring.last().unwrap_or(&[0.0, 0.0]);
            let next = edges
                .iter()
                .position(|e| e.first() == Some(&end))
                .map(|i| (i, false))
                .or_else(|| {
                    edges
                        .iter()
                        .position(|e| e.last() == Some(&end))
                        .map(|i| (i, true))
                });
            let Some((i, rev)) = next else {
                // Not closed by any edge: close it straight.
                let start = ring[0];
                ring.push(start);
                break;
            };
            let mut e = edges.swap_remove(i);
            if rev {
                e.reverse();
            }
            ring.extend(&e[1..]);
        }
        if ring.len() >= 4 {
            out.push(ring);
        }
    }
    out
}

/// A text attribute value of national (lexical level 2: UCS-2) or plain (Latin-1) text.
fn natf(d: &[u8], ucs2: bool) -> Vec<(u16, String)> {
    let mut out = Vec::new();
    let mut p = 0;
    while p + 2 <= d.len() {
        let code = u16::from_le_bytes([d[p], d[p + 1]]);
        p += 2;
        let text = if ucs2 {
            let mut units = Vec::new();
            while p + 2 <= d.len() {
                let u = u16::from_le_bytes([d[p], d[p + 1]]);
                p += 2;
                if u == 0x001F {
                    break;
                }
                units.push(u);
            }
            String::from_utf16_lossy(&units)
        } else {
            let end = d[p..]
                .iter()
                .position(|&b| b == UT)
                .map_or(d.len(), |e| p + e);
            let t = latin1(&d[p..end]);
            p = end + 1;
            t
        };
        out.push((code, text));
    }
    out
}

/// A value of an attribute of type `t` (`E`, `L`, `F`, `I`, `A`, `S`).
fn typed(t: u8, s: &str) -> Value {
    let s = s.trim_end_matches('\0');
    // DEL (0x7F) marks a value an update deletes.
    if s.is_empty() || s == "\u{7f}" {
        return Value::Null;
    }
    match t {
        b'E' | b'I' => s.trim().parse().map_or(Value::Text(s.into()), Value::Int),
        b'F' => s.trim().parse().map_or(Value::Text(s.into()), Value::Float),
        _ => Value::Text(s.into()),
    }
}

pub fn read(data: &[u8]) -> Result<Dataset, String> {
    let (module, records, damage) = iso8211::parse(data)?;
    let mut r = Reader {
        module: &module,
        // Without DSPM (an update file), GDAL's defaults.
        comf: 1_000_000.0,
        somf: 10.0,
        vectors: HashMap::new(),
    };
    let mut ds = Dataset {
        format: "S-57",
        ..Dataset::default()
    };
    if let Some(e) = damage {
        ds.warn(format!(
            "the file is damaged after record {}: {e}",
            records.len()
        ));
    }
    // The dataset's facts, and its coordinate factors.
    let mut dsid = Layer::new("DSID");
    let mut facts: Vec<(String, Value)> = Vec::new();
    let mut ucs2 = false;
    let mut hdat = None;
    let (mut comf, mut somf) = (None, None);
    // Each in the first record that has it (the parameters follow the identification).
    for tag in ["DSID", "DSSI", "DSPM"] {
        let Some(rec) = records.iter().take(64).find(|r| r.field(tag).is_some()) else {
            continue;
        };
        {
            for g in r.groups(rec, tag).into_iter().take(1) {
                for (l, v) in g {
                    if l == "RCNM" || l == "RCID" {
                        continue;
                    }
                    let val = match &v {
                        Sub::Int(i) => Value::Int(*i),
                        Sub::Real(f) => Value::Float(*f),
                        Sub::Text(t) => Value::Text(latin1(t)),
                        _ => Value::Null,
                    };
                    match (tag, l) {
                        ("DSPM", "COMF") => comf = v.real().filter(|c| *c > 0.0),
                        ("DSPM", "SOMF") => somf = v.real().filter(|c| *c > 0.0),
                        ("DSPM", "HDAT") => hdat = v.int(),
                        ("DSSI", "NALL") => ucs2 = v.int() == Some(2),
                        _ => {}
                    }
                    facts.push((format!("{tag}_{l}"), val));
                }
            }
        }
    }
    // An update (EXPP 2, or an update number) changes a cell; alone it shows only that.
    let update = facts.iter().any(|(k, v)| {
        (k == "DSID_EXPP" && *v == Value::Int(2))
            || (k == "DSID_UPDN"
                && !matches!(v, Value::Text(t) if t.trim().is_empty() || t.trim() == "0"))
    });
    if update {
        ds.warn("this is an update to a chart cell: only what it changes is shown; open the cell's .000 file for the chart");
    }
    r.comf = comf.unwrap_or(r.comf);
    r.somf = somf.unwrap_or(r.somf);
    if !facts.is_empty() {
        dsid.push(Some("DSID".into()), None, facts);
    }
    // HDAT 2 is WGS 84, the datum every ENC uses.
    match hdat {
        Some(2) | None => ds.crs = crs::from_epsg(4326),
        Some(h) => {
            ds.crs = crs::from_epsg(4326);
            ds.crs_note = Some(format!("horizontal datum {h} (S-57), shown as WGS 84"));
        }
    }
    for rec in &records {
        if rec.field("VRID").is_some() {
            r.vector(rec);
        }
    }
    let mut layers: Vec<Layer> = Vec::new();
    let mut by_class: HashMap<u16, usize> = HashMap::new();
    let mut n = 0usize;
    for rec in &records {
        let Some(frid) = r.groups(rec, "FRID").into_iter().next() else {
            continue;
        };
        n += 1;
        if n > MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        let get = |k: &str| {
            frid.iter()
                .find(|(l, _)| *l == k)
                .and_then(|(_, v)| v.int())
        };
        let objl = get("OBJL").unwrap_or(0).clamp(0, i64::from(u16::MAX)) as u16;
        let prim = get("PRIM").unwrap_or(255);
        let li = *by_class.entry(objl).or_insert_with(|| {
            let name = object_acronym(objl).map_or_else(|| format!("OBJL_{objl}"), str::to_owned);
            let mut l = Layer::new(name);
            l.declared_type = None;
            layers.push(l);
            layers.len() - 1
        });
        let mut attrs: Vec<(String, Value)> = Vec::new();
        for k in ["RCID", "PRIM", "GRUP", "OBJL", "RVER"] {
            attrs.push((k.into(), get(k).map_or(Value::Null, Value::Int)));
        }
        if let Some(foid) = r.groups(rec, "FOID").into_iter().next() {
            let f = |k: &str| {
                foid.iter()
                    .find(|(l, _)| *l == k)
                    .and_then(|(_, v)| v.int())
            };
            let (a, i, s) = (f("AGEN"), f("FIDN"), f("FIDS"));
            for (k, v) in [("AGEN", a), ("FIDN", i), ("FIDS", s)] {
                attrs.push((k.into(), v.map_or(Value::Null, Value::Int)));
            }
            if let (Some(a), Some(i), Some(s)) = (a, i, s) {
                attrs.push(("LNAM".into(), Value::Text(format!("{a:04X}{i:08X}{s:04X}"))));
            }
        }
        let mut values: Vec<(u16, String)> = Vec::new();
        for g in r.groups(rec, "ATTF") {
            let code = g.first().and_then(|(_, v)| v.int());
            let text = g.get(1).map(|(_, v)| latin1(v.bytes()));
            if let (Some(c), Some(t)) = (code, text) {
                values.push((c as u16, t));
            }
        }
        if let Some(d) = rec.field("NATF") {
            values.extend(natf(d, ucs2));
        }
        for (code, text) in values {
            let (acr, t) = attribute(code).unwrap_or(("", b'S'));
            let key = if acr.is_empty() || acr == "N/A" {
                format!("ATTL_{code}")
            } else {
                acr.to_owned()
            };
            attrs.push((key, typed(t, &text)));
        }
        let (geom, depths) = r.geometry(prim, rec);
        if !depths.is_empty() {
            let shown: Vec<String> = depths.iter().take(500).map(|d| format!("{d}")).collect();
            attrs.push(("DEPTHS".into(), Value::Text(shown.join(","))));
        }
        let id = get("RCID").map(|i| i.to_string());
        layers[li].push(id, geom, attrs);
    }
    if layers.is_empty() && dsid.features.is_empty() {
        return Err("no features".into());
    }
    // Layers in object class order, the dataset's facts first.
    let mut order: Vec<(u16, usize)> = by_class.into_iter().collect();
    order.sort();
    let mut slots: Vec<Option<Layer>> = layers.into_iter().map(Some).collect();
    if !dsid.features.is_empty() {
        ds.layers.push(dsid);
    }
    for (_, i) in order {
        if let Some(mut l) = slots[i].take() {
            l.features
                .sort_by_key(|f| f.id.as_deref().and_then(|i| i.parse::<u64>().ok()));
            ds.layers.push(l);
        }
    }
    Ok(ds)
}

/// Chart colours for a feature of object class `class` (S-52's day palette, simplified):
/// depth areas shaded by depth, land in buff, the coastline dark, soundings grey, lights
/// and routeing areas magenta. Areas of other classes are outlined, not filled, so they do
/// not hide the chart; meta classes (`M_…`, `C_…`) are only faintly outlined.
pub fn style(class: &str, fields: &[String], values: &[Value]) -> Option<FeatureStyle> {
    let get = |k: &str| {
        fields
            .iter()
            .position(|f| f == k)
            .and_then(|i| values.get(i))
            .and_then(|v| match v {
                Value::Float(f) => Some(*f),
                Value::Int(i) => Some(*i as f64),
                _ => None,
            })
    };
    let outline = |c: [u8; 4]| FeatureStyle {
        stroke: Some(c),
        fill: Some([0, 0, 0, 0]),
        ..FeatureStyle::default()
    };
    let area = |fill: [u8; 3], stroke: [u8; 3]| FeatureStyle {
        stroke: Some([stroke[0], stroke[1], stroke[2], 255]),
        fill: Some([fill[0], fill[1], fill[2], 255]),
        width_px: Some(0.5),
        ..FeatureStyle::default()
    };
    const MAGENTA: [u8; 4] = [0xC0, 0x2F, 0x9E, 255];
    Some(match class {
        "DEPARE" | "DRGARE" => {
            // Drying, very shallow, shallow, medium and deep water.
            let fill = match get("DRVAL1") {
                Some(d) if d < 0.0 => [0x9F, 0xBF, 0x9A],
                Some(d) if d < 2.0 => [0x73, 0xB6, 0xEF],
                Some(d) if d < 10.0 => [0x98, 0xC5, 0xF2],
                Some(d) if d < 30.0 => [0xBA, 0xD5, 0xE1],
                _ => [0xD4, 0xEA, 0xEE],
            };
            area(fill, [0x9A, 0xA8, 0xB0])
        }
        "LNDARE" => area([0xC9, 0xB9, 0x7A], [0x8C, 0x7B, 0x45]),
        "BUAARE" => area([0xB0, 0x9A, 0x5E], [0x7A, 0x68, 0x38]),
        "UNSARE" => area([0xD8, 0xD8, 0xD8], [0xA0, 0xA0, 0xA0]),
        "COALNE" | "SLCONS" | "PONTON" | "HULKES" => FeatureStyle {
            stroke: Some([0x30, 0x30, 0x30, 255]),
            fill: Some([0x8C, 0x7B, 0x45, 255]),
            width_px: Some(1.2),
            ..FeatureStyle::default()
        },
        "DEPCNT" => outline([0x7A, 0x8A, 0x99, 255]),
        "SOUNDG" => FeatureStyle {
            marker: Some([0x55, 0x66, 0x77, 255]),
            marker_px: Some(1.5),
            ..FeatureStyle::default()
        },
        "LIGHTS" => FeatureStyle {
            marker: Some(MAGENTA),
            ..FeatureStyle::default()
        },
        c if c.starts_with("BOY") || c.starts_with("BCN") => FeatureStyle {
            marker: Some([0xD0, 0x30, 0x30, 255]),
            ..FeatureStyle::default()
        },
        "FAIRWY" | "RESARE" | "ACHARE" | "TSSLPT" | "TSEZNE" | "PRCARE" | "CTNARE" | "MIPARE"
        | "DWRTPT" | "RECTRC" | "NAVLNE" | "TSSBND" => FeatureStyle {
            stroke: Some(MAGENTA),
            fill: Some([0, 0, 0, 0]),
            marker: Some(MAGENTA),
            ..FeatureStyle::default()
        },
        c if c.starts_with("M_") || c.starts_with("C_") => outline([0x90, 0x90, 0x90, 96]),
        _ => FeatureStyle {
            fill: Some([0, 0, 0, 0]),
            ..FeatureStyle::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue() {
        assert_eq!(object_acronym(42), Some("DEPARE"));
        assert_eq!(object_acronym(129), Some("SOUNDG"));
        assert_eq!(object_name(71), Some("Land area"));
        assert_eq!(attribute(87), Some(("DRVAL1", b'F')));
        assert_eq!(attribute(116), Some(("OBJNAM", b'S')));
        assert!(OBJECTS.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(ATTRIBUTES.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn rings_from_edges() {
        // A square from three edges, one of them backwards, and a hole of one edge.
        let e = vec![
            vec![[0.0, 0.0], [4.0, 0.0]],
            vec![[0.0, 4.0], [4.0, 4.0], [4.0, 0.0]],
            vec![[0.0, 4.0], [0.0, 0.0]],
            vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 1.0]],
        ];
        let r = rings(e);
        assert_eq!(r.len(), 2);
        assert_eq!(
            r[0],
            [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]]
        );
        assert_eq!(area(&r[0]), 32.0);
    }

    #[test]
    fn national_text() {
        let mut d = 301u16.to_le_bytes().to_vec();
        for u in "Bërg".encode_utf16() {
            d.extend(u.to_le_bytes());
        }
        d.extend([0x1F, 0x00]);
        d.extend(116u16.to_le_bytes());
        d.extend([b'x', 0x1F, 0x00]);
        let v = natf(&d, true);
        assert_eq!(v[0], (301, "Bërg".to_string()));
        assert_eq!(
            natf(b"\x74\x00Caf\xe9\x1f", false),
            [(116, "Café".to_string())]
        );
        assert_eq!(typed(b'F', "12.5"), Value::Float(12.5));
        assert_eq!(typed(b'E', "3"), Value::Int(3));
        assert_eq!(typed(b'L', "1,2"), Value::Text("1,2".into()));
        assert_eq!(typed(b'S', ""), Value::Null);
    }

    #[test]
    fn chart_colours() {
        let f = vec!["DRVAL1".to_string()];
        let shallow = style("DEPARE", &f, &[Value::Float(1.0)]).unwrap();
        let deep = style("DEPARE", &f, &[Value::Float(50.0)]).unwrap();
        assert_ne!(shallow.fill, deep.fill);
        assert_eq!(style("M_COVR", &[], &[]).unwrap().fill, Some([0, 0, 0, 0]));
        assert!(style("SOUNDG", &[], &[]).unwrap().marker.is_some());
    }
}
