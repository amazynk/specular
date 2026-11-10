//! OpenStreetMap data: OSM XML (`.osm`, streamed) and PBF (`.osm.pbf`: zlib, raw or
//! Zstandard blobs, dense nodes). Nodes, ways and relations become the five layers GDAL's
//! OSM driver makes with its default configuration, so files read the same way in both:
//!
//! - `points`: nodes with a significant tag;
//! - `lines`: ways that are not areas (and untagged ways that no relation uses);
//! - `multilinestrings`: relations of type multilinestring and route;
//! - `multipolygons`: closed ways with an area tag, and multipolygon and boundary relations
//!   (member ways joined into rings; inner rings placed in the outer that holds them);
//! - `other_relations`: the other relations, as collections of their members.
//!
//! Each layer has `osm_id` (multipolygons from ways have `osm_way_id`), the common keys as
//! fields, and every other tag in `other_tags` as `"key"=>"value"` pairs.

use std::collections::HashMap;

use crate::model::{Dataset, Geometry, Layer, MAX_FEATURES, MAX_VERTICES, Pt, Value};

type Tags = Vec<(String, String)>;

struct Way {
    id: i64,
    refs: Vec<i64>,
    tags: Tags,
}

struct Rel {
    id: i64,
    /// (0 node, 1 way, 2 relation; id; role).
    members: Vec<(u8, i64, String)>,
    tags: Tags,
}

#[derive(Default)]
struct Data {
    /// Bytes of tag text copied out of string tables so far ([`MAX_TAG_BYTES`]).
    tag_bytes: usize,
    nodes: Vec<(i64, Pt)>,
    tagged: Vec<(i64, Pt, Tags)>,
    ways: Vec<Way>,
    rels: Vec<Rel>,
    warnings: Vec<String>,
}

pub fn sniff_xml(head: &[u8]) -> bool {
    let s = String::from_utf8_lossy(&head[..head.len().min(4096)]);
    s.contains("<osm")
        && (s.contains("<node") || s.contains("<bounds") || s.contains("version=\"0.6\""))
}

/// A PBF starts with a BlobHeader of type "OSMHeader".
pub fn sniff_pbf(head: &[u8]) -> bool {
    head.len() >= 20 && head[4] == 0x0A && head[5] == 9 && &head[6..15] == b"OSMHeader"
}

// --------------------------------------------------------------------------- protobuf

struct Pb<'a> {
    b: &'a [u8],
    i: usize,
}

enum F<'a> {
    V(u64),
    B(&'a [u8]),
}

impl<'a> Pb<'a> {
    fn new(b: &'a [u8]) -> Self {
        Pb { b, i: 0 }
    }
    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..70).step_by(7) {
            let x = *self.b.get(self.i).ok_or("PBF data is truncated")?;
            self.i += 1;
            if shift < 64 {
                v |= u64::from(x & 0x7F) << shift;
            }
            if x & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("PBF has a malformed number".into())
    }
    fn next(&mut self) -> Result<Option<(u32, F<'a>)>, String> {
        if self.i >= self.b.len() {
            return Ok(None);
        }
        let key = self.varint()?;
        let f = match key & 7 {
            0 => F::V(self.varint()?),
            1 => {
                self.i += 8;
                F::V(0)
            }
            2 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "bad length")?;
                let s = self
                    .b
                    .get(self.i..self.i.checked_add(n).ok_or("bad length")?)
                    .ok_or("PBF data is truncated")?;
                self.i += n;
                F::B(s)
            }
            5 => {
                self.i += 4;
                F::V(0)
            }
            w => return Err(format!("PBF wire type {w}")),
        };
        if self.i > self.b.len() {
            return Err("PBF data is truncated".into());
        }
        Ok(Some((u32::try_from(key >> 3).unwrap_or(0), f)))
    }
}

fn zz(v: u64) -> i64 {
    (v >> 1) as i64 ^ -((v & 1) as i64)
}

fn packed(b: &[u8], signed: bool) -> Result<Vec<i64>, String> {
    let mut p = Pb::new(b);
    let mut out = Vec::new();
    while p.i < b.len() {
        let v = p.varint()?;
        out.push(if signed { zz(v) } else { v as i64 });
    }
    Ok(out)
}

fn delta(v: Vec<i64>) -> Vec<i64> {
    let mut acc = 0i64;
    v.into_iter()
        .map(|d| {
            acc = acc.wrapping_add(d);
            acc
        })
        .collect()
}

fn blob(b: &[u8]) -> Result<Vec<u8>, String> {
    let mut p = Pb::new(b);
    let mut raw_size = 0usize;
    let mut data: Option<(u32, &[u8])> = None;
    while let Some((k, f)) = p.next()? {
        match (k, f) {
            (2, F::V(v)) => raw_size = usize::try_from(v).unwrap_or(0),
            (k @ (1 | 3 | 4 | 5 | 6 | 7), F::B(s)) => data = Some((k, s)),
            _ => {}
        }
    }
    let limit = 64 << 20; // the format caps a blob at 32 MiB uncompressed
    match data {
        Some((1, s)) => Ok(s.to_vec()),
        Some((3, s)) => crate::codec::inflate(s, raw_size.clamp(1, limit)),
        Some((7, s)) => crate::codec::zstd(s, raw_size.clamp(1, limit)),
        Some((6, s)) => {
            let mut out = Vec::new();
            crate::codec::lz4_block_into(s, &mut out, raw_size.clamp(1, limit))?;
            Ok(out)
        }
        Some((4, _)) => Err("LZMA-compressed PBF blocks are not supported".into()),
        Some((5, _)) => Err("bzip2-compressed PBF blocks are not supported".into()),
        _ => Ok(Vec::new()),
    }
}

/// Most bytes of tag text a file's elements may copy from string tables, together: every
/// element may name the same 30 MiB string, so the copies are counted, not the table.
const MAX_TAG_BYTES: usize = 1 << 30;

/// Counts a tag of `k` and `v` against `spent`; false (and nothing counted) past the cap.
fn spend(spent: &mut usize, k: &str, v: &str) -> bool {
    // Two `String`s' headers besides their text.
    let n = k.len() + v.len() + 48;
    if spent.saturating_add(n) > MAX_TAG_BYTES {
        return false;
    }
    *spent += n;
    true
}

fn tags_of(keys: &[i64], vals: &[i64], st: &[String], spent: &mut usize) -> Tags {
    keys.iter()
        .zip(vals)
        .filter_map(|(&k, &v)| {
            let k = st.get(usize::try_from(k).ok()?)?;
            let v = st.get(usize::try_from(v).ok()?)?;
            spend(spent, k, v).then(|| (k.clone(), v.clone()))
        })
        .collect()
}

fn primitive_block(b: &[u8], d: &mut Data) -> Result<(), String> {
    let mut p = Pb::new(b);
    let mut st: Vec<String> = Vec::new();
    let mut groups: Vec<&[u8]> = Vec::new();
    let (mut gran, mut lat_off, mut lon_off) = (100i64, 0i64, 0i64);
    while let Some((k, f)) = p.next()? {
        match (k, f) {
            (1, F::B(s)) => {
                let mut q = Pb::new(s);
                while let Some((k, f)) = q.next()? {
                    if let (1, F::B(x)) = (k, f) {
                        st.push(String::from_utf8_lossy(x).into_owned());
                    }
                }
            }
            (2, F::B(s)) => groups.push(s),
            (17, F::V(v)) => gran = v as i64,
            (19, F::V(v)) => lat_off = v as i64,
            (20, F::V(v)) => lon_off = v as i64,
            _ => {}
        }
    }
    let coord = |lat: i64, lon: i64| -> Pt {
        [
            1e-9 * (lon_off + gran * lon) as f64,
            1e-9 * (lat_off + gran * lat) as f64,
        ]
    };
    for g in groups {
        let mut q = Pb::new(g);
        while let Some((k, f)) = q.next()? {
            let F::B(s) = f else { continue };
            match k {
                1 => {
                    let mut r = Pb::new(s);
                    let (mut id, mut lat, mut lon) = (0i64, 0i64, 0i64);
                    let (mut keys, mut vals) = (Vec::new(), Vec::new());
                    while let Some((k, f)) = r.next()? {
                        match (k, f) {
                            (1, F::V(v)) => id = zz(v),
                            (2, F::B(x)) => keys = packed(x, false)?,
                            (3, F::B(x)) => vals = packed(x, false)?,
                            (8, F::V(v)) => lat = zz(v),
                            (9, F::V(v)) => lon = zz(v),
                            _ => {}
                        }
                    }
                    let pt = coord(lat, lon);
                    d.nodes.push((id, pt));
                    let t = tags_of(&keys, &vals, &st, &mut d.tag_bytes);
                    if !t.is_empty() {
                        d.tagged.push((id, pt, t));
                    }
                }
                2 => {
                    let mut r = Pb::new(s);
                    let (mut ids, mut lats, mut lons, mut kv) =
                        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                    while let Some((k, f)) = r.next()? {
                        match (k, f) {
                            (1, F::B(x)) => ids = delta(packed(x, true)?),
                            (8, F::B(x)) => lats = delta(packed(x, true)?),
                            (9, F::B(x)) => lons = delta(packed(x, true)?),
                            (10, F::B(x)) => kv = packed(x, false)?,
                            _ => {}
                        }
                    }
                    if lats.len() != ids.len() || lons.len() != ids.len() {
                        return Err("PBF dense nodes are inconsistent".into());
                    }
                    let mut kvi = 0usize;
                    for i in 0..ids.len() {
                        let pt = coord(lats[i], lons[i]);
                        d.nodes.push((ids[i], pt));
                        let mut t: Tags = Vec::new();
                        while kvi < kv.len() {
                            let k = kv[kvi];
                            kvi += 1;
                            if k == 0 {
                                break;
                            }
                            let v = kv.get(kvi).copied().unwrap_or(0);
                            kvi += 1;
                            if let (Some(k), Some(v)) = (
                                st.get(usize::try_from(k).unwrap_or(usize::MAX)),
                                st.get(usize::try_from(v).unwrap_or(usize::MAX)),
                            ) && spend(&mut d.tag_bytes, k, v)
                            {
                                t.push((k.clone(), v.clone()));
                            }
                        }
                        if !t.is_empty() {
                            d.tagged.push((ids[i], pt, t));
                        }
                    }
                }
                3 => {
                    let mut r = Pb::new(s);
                    let (mut id, mut keys, mut vals, mut refs) = (0i64, vec![], vec![], vec![]);
                    while let Some((k, f)) = r.next()? {
                        match (k, f) {
                            (1, F::V(v)) => id = v as i64,
                            (2, F::B(x)) => keys = packed(x, false)?,
                            (3, F::B(x)) => vals = packed(x, false)?,
                            (8, F::B(x)) => refs = delta(packed(x, true)?),
                            _ => {}
                        }
                    }
                    d.ways.push(Way {
                        id,
                        refs,
                        tags: tags_of(&keys, &vals, &st, &mut d.tag_bytes),
                    });
                }
                4 => {
                    let mut r = Pb::new(s);
                    let (mut id, mut keys, mut vals) = (0i64, vec![], vec![]);
                    let (mut roles, mut mids, mut types) = (vec![], vec![], vec![]);
                    while let Some((k, f)) = r.next()? {
                        match (k, f) {
                            (1, F::V(v)) => id = v as i64,
                            (2, F::B(x)) => keys = packed(x, false)?,
                            (3, F::B(x)) => vals = packed(x, false)?,
                            (8, F::B(x)) => roles = packed(x, false)?,
                            (9, F::B(x)) => mids = delta(packed(x, true)?),
                            (10, F::B(x)) => types = packed(x, false)?,
                            _ => {}
                        }
                    }
                    let members = mids
                        .iter()
                        .enumerate()
                        .map(|(i, &m)| {
                            let role = roles
                                .get(i)
                                .and_then(|&r| st.get(usize::try_from(r).ok()?))
                                .cloned()
                                .unwrap_or_default();
                            (types.get(i).copied().unwrap_or(0) as u8, m, role)
                        })
                        .collect();
                    d.rels.push(Rel {
                        id,
                        members,
                        tags: tags_of(&keys, &vals, &st, &mut d.tag_bytes),
                    });
                }
                _ => {}
            }
        }
        if d.nodes.len() > MAX_VERTICES {
            return Err("the OSM file has too many nodes".into());
        }
    }
    Ok(())
}

fn read_pbf(data: &[u8]) -> Result<Data, String> {
    let mut d = Data::default();
    let mut i = 0usize;
    while i + 4 <= data.len() {
        let hl = u32::from_be_bytes(data[i..i + 4].try_into().unwrap_or_default()) as usize;
        i += 4;
        let h = data
            .get(i..i.checked_add(hl).ok_or("bad header")?)
            .ok_or("the PBF is truncated")?;
        i += hl;
        let mut p = Pb::new(h);
        let (mut ty, mut size) = (Vec::new(), 0usize);
        while let Some((k, f)) = p.next()? {
            match (k, f) {
                (1, F::B(s)) => ty = s.to_vec(),
                (3, F::V(v)) => size = usize::try_from(v).unwrap_or(usize::MAX),
                _ => {}
            }
        }
        let b = data
            .get(i..i.checked_add(size).ok_or("bad blob size")?)
            .ok_or("the PBF is truncated")?;
        i += size;
        match ty.as_slice() {
            b"OSMHeader" => {
                let raw = blob(b)?;
                let mut q = Pb::new(&raw);
                while let Some((k, f)) = q.next()? {
                    if let (4, F::B(s)) = (k, f) {
                        let feat = String::from_utf8_lossy(s);
                        if !matches!(
                            feat.as_ref(),
                            "OsmSchema-V0.6" | "DenseNodes" | "HistoricalInformation"
                        ) {
                            return Err(format!(
                                "the PBF needs a feature not supported here: {feat}"
                            ));
                        }
                    }
                }
            }
            b"OSMData" => primitive_block(&blob(b)?, &mut d)?,
            _ => {}
        }
    }
    Ok(d)
}

// --------------------------------------------------------------------------------- XML

fn read_xml(data: &[u8]) -> Result<Data, String> {
    use quick_xml::events::{BytesStart, Event};
    let attr = |e: &BytesStart<'_>, key: &[u8]| -> Option<String> {
        let a = e
            .attributes()
            .with_checks(false)
            .flatten()
            .find(|a| a.key.local_name().as_ref() == key)?;
        let s = String::from_utf8_lossy(&a.value).into_owned();
        Some(match quick_xml::escape::unescape(&s) {
            Ok(u) => u.into_owned(),
            Err(_) => s,
        })
    };
    let num = |e: &BytesStart<'_>, key: &[u8]| -> Option<f64> { attr(e, key)?.trim().parse().ok() };
    let id = |e: &BytesStart<'_>, key: &[u8]| -> Option<i64> { attr(e, key)?.trim().parse().ok() };
    let mut r = quick_xml::Reader::from_reader(data);
    let mut d = Data::default();
    #[derive(PartialEq)]
    enum Cur {
        None,
        Node(i64, Pt),
        Way,
        Rel,
    }
    let mut cur = Cur::None;
    let mut tags: Tags = Vec::new();
    let mut refs: Vec<i64> = Vec::new();
    let mut members: Vec<(u8, i64, String)> = Vec::new();
    let mut cur_id = 0i64;
    let mut root_ok = false;
    loop {
        let ev = r.read_event().map_err(|e| format!("not valid XML: {e}"))?;
        let (e, empty) = match &ev {
            Event::Start(e) => (Some(e.clone()), false),
            Event::Empty(e) => (Some(e.clone()), true),
            Event::End(e) => {
                match e.local_name().as_ref() {
                    b"node" | b"way" | b"relation" => finish(
                        &mut d,
                        &mut cur_id,
                        std::mem::replace(&mut cur, Cur::None),
                        &mut tags,
                        &mut refs,
                        &mut members,
                    ),
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let Some(e) = e else { continue };
        match e.local_name().as_ref() {
            b"osm" | b"osmChange" => root_ok = true,
            b"node" => {
                cur_id = id(&e, b"id").unwrap_or(0);
                let pt = [
                    num(&e, b"lon").unwrap_or(f64::NAN),
                    num(&e, b"lat").unwrap_or(f64::NAN),
                ];
                cur = Cur::Node(cur_id, pt);
                if empty {
                    finish(
                        &mut d,
                        &mut cur_id,
                        std::mem::replace(&mut cur, Cur::None),
                        &mut tags,
                        &mut refs,
                        &mut members,
                    );
                }
            }
            b"way" => {
                cur_id = id(&e, b"id").unwrap_or(0);
                cur = Cur::Way;
                if empty {
                    finish(
                        &mut d,
                        &mut cur_id,
                        std::mem::replace(&mut cur, Cur::None),
                        &mut tags,
                        &mut refs,
                        &mut members,
                    );
                }
            }
            b"relation" => {
                cur_id = id(&e, b"id").unwrap_or(0);
                cur = Cur::Rel;
                if empty {
                    finish(
                        &mut d,
                        &mut cur_id,
                        std::mem::replace(&mut cur, Cur::None),
                        &mut tags,
                        &mut refs,
                        &mut members,
                    );
                }
            }
            b"tag" if cur != Cur::None => {
                if let (Some(k), Some(v)) = (attr(&e, b"k"), attr(&e, b"v")) {
                    tags.push((k, v));
                }
            }
            b"nd" if cur == Cur::Way => {
                if let Some(r) = id(&e, b"ref") {
                    refs.push(r);
                }
            }
            b"member" if cur == Cur::Rel => {
                let t = match attr(&e, b"type").as_deref() {
                    Some("way") => 1,
                    Some("relation") => 2,
                    _ => 0,
                };
                if let Some(r) = id(&e, b"ref") {
                    members.push((t, r, attr(&e, b"role").unwrap_or_default()));
                }
            }
            _ => {}
        }
        if d.nodes.len() > MAX_VERTICES {
            return Err("the OSM file has too many nodes".into());
        }
    }
    if !root_ok {
        return Err("not an OSM XML document".into());
    }
    return Ok(d);

    fn finish(
        d: &mut Data,
        id: &mut i64,
        cur: Cur,
        tags: &mut Tags,
        refs: &mut Vec<i64>,
        members: &mut Vec<(u8, i64, String)>,
    ) {
        match cur {
            Cur::Node(nid, pt) => {
                d.nodes.push((nid, pt));
                if !tags.is_empty() {
                    d.tagged.push((nid, pt, std::mem::take(tags)));
                }
            }
            Cur::Way => d.ways.push(Way {
                id: *id,
                refs: std::mem::take(refs),
                tags: std::mem::take(tags),
            }),
            Cur::Rel => d.rels.push(Rel {
                id: *id,
                members: std::mem::take(members),
                tags: std::mem::take(tags),
            }),
            Cur::None => {}
        }
        tags.clear();
        refs.clear();
        members.clear();
    }
}

// ----------------------------------------------------------------------------- layers

const AREA_KEYS: &[&str] = &[
    "aeroway",
    "amenity",
    "boundary",
    "building",
    "craft",
    "geological",
    "historic",
    "landuse",
    "leisure",
    "military",
    "natural",
    "office",
    "place",
    "shop",
    "sport",
    "tourism",
];
const POINT_FIELDS: &[&str] = &[
    "name", "barrier", "highway", "ref", "address", "is_in", "place", "man_made",
];
const LINE_FIELDS: &[&str] = &[
    "name",
    "highway",
    "waterway",
    "aerialway",
    "barrier",
    "man_made",
    "railway",
];
const POLY_FIELDS: &[&str] = &[
    "name",
    "type",
    "aeroway",
    "amenity",
    "admin_level",
    "barrier",
    "boundary",
    "building",
    "craft",
    "geological",
    "historic",
    "land_area",
    "landuse",
    "leisure",
    "man_made",
    "military",
    "natural",
    "office",
    "place",
    "shop",
    "sport",
    "tourism",
];
const REL_FIELDS: &[&str] = &["name", "type"];
const POINT_UNSIGNIFICANT: &[&str] = &[
    "created_by",
    "converted_by",
    "source",
    "time",
    "ele",
    "attribution",
];

fn ignored(k: &str, area: bool) -> bool {
    matches!(
        k,
        "created_by"
            | "converted_by"
            | "source"
            | "time"
            | "ele"
            | "note"
            | "todo"
            | "fixme"
            | "FIXME"
    ) || k.starts_with("openGeoDB:")
        || (area && k == "area")
}

fn hstore(tags: &[(String, String)]) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    tags.iter()
        .map(|(k, v)| format!("\"{}\"=>\"{}\"", esc(k), esc(v)))
        .collect::<Vec<_>>()
        .join(",")
}

/// A row: osm_id (and osm_way_id), the layer's fields, other_tags.
fn row(
    id_fields: &[(&str, Option<i64>)],
    fields: &[&str],
    tags: &[(String, String)],
    area: bool,
    extra: &[(&str, Value)],
) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = id_fields
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                v.map_or(Value::Null, |v| Value::Text(v.to_string())),
            )
        })
        .collect();
    for f in fields {
        let v = tags.iter().find(|(k, _)| k == f).map(|(_, v)| v.clone());
        out.push((f.to_string(), v.map_or(Value::Null, Value::Text)));
    }
    for (k, v) in extra {
        out.push((k.to_string(), v.clone()));
    }
    let other: Vec<(String, String)> = tags
        .iter()
        .filter(|(k, _)| !fields.contains(&k.as_str()) && !ignored(k, area))
        .cloned()
        .collect();
    out.push((
        "other_tags".into(),
        if other.is_empty() {
            Value::Null
        } else {
            Value::Text(hstore(&other))
        },
    ));
    out
}

fn is_area(tags: &[(String, String)]) -> bool {
    if let Some((_, v)) = tags.iter().find(|(k, _)| k == "area") {
        return v == "yes";
    }
    tags.iter().any(|(k, v)| {
        AREA_KEYS.contains(&k.as_str())
            || (k == "highway" && v == "platform")
            || (k == "public_transport" && v == "platform")
    })
}

fn z_order(tags: &[(String, String)]) -> i64 {
    let get = |k: &str| tags.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    let mut z = match get("highway") {
        Some("minor" | "road" | "unclassified" | "residential") => 3,
        Some("tertiary_link" | "tertiary") => 4,
        Some("secondary_link" | "secondary") => 6,
        Some("primary_link" | "primary") => 7,
        Some("trunk_link" | "trunk") => 8,
        Some("motorway_link" | "motorway") => 9,
        _ => 0,
    };
    let yes = |v: Option<&str>| matches!(v, Some("yes" | "true" | "1"));
    if yes(get("bridge")) {
        z += 10;
    }
    if yes(get("tunnel")) {
        z -= 10;
    }
    if get("railway").is_some() {
        z += 5;
    }
    if let Some(l) = get("layer") {
        z += 10 * l.trim().parse::<i64>().unwrap_or(0);
    }
    z
}

/// Joins way segments end to end into closed rings; `None` when they do not close.
fn rings(mut parts: Vec<Vec<Pt>>) -> Option<Vec<Vec<Pt>>> {
    let mut out = Vec::new();
    parts.retain(|p| p.len() >= 2);
    while let Some(mut ring) = parts.pop() {
        let mut guard = 0;
        while ring.first() != ring.last() {
            guard += 1;
            if guard > 100_000 {
                return None;
            }
            let end = *ring.last()?;
            let start = *ring.first()?;
            let pos = parts.iter().position(|p| {
                p.first() == Some(&end)
                    || p.last() == Some(&end)
                    || p.first() == Some(&start)
                    || p.last() == Some(&start)
            })?;
            let mut p = parts.swap_remove(pos);
            if p.first() == Some(&end) {
                ring.extend_from_slice(&p[1..]);
            } else if p.last() == Some(&end) {
                p.reverse();
                ring.extend_from_slice(&p[1..]);
            } else if p.last() == Some(&start) {
                p.pop();
                p.extend_from_slice(&ring);
                ring = p;
            } else {
                p.reverse();
                p.pop();
                p.extend_from_slice(&ring);
                ring = p;
            }
        }
        if ring.len() >= 4 {
            out.push(ring);
        }
    }
    Some(out)
}

/// Rings to polygons: each ring not inside another is an outer; others are holes of the
/// smallest outer holding them.
fn organise(rings: Vec<Vec<Pt>>) -> Vec<Vec<Vec<Pt>>> {
    let area = |r: &[Pt]| crate::shp::signed_area2(r).abs();
    let mut idx: Vec<usize> = (0..rings.len()).collect();
    idx.sort_by(|&a, &b| area(&rings[b]).total_cmp(&area(&rings[a])));
    let mut polys: Vec<(usize, Vec<Vec<Pt>>)> = Vec::new(); // (outer index, rings)
    let mut depth = vec![0usize; rings.len()];
    for &i in &idx {
        // The smallest enclosing ring already placed.
        let probe = rings[i][0];
        let mut parent: Option<usize> = None;
        for &j in &idx {
            if j == i || area(&rings[j]) <= area(&rings[i]) {
                continue;
            }
            if crate::shp::ring_contains(&rings[j], probe)
                && parent.is_none_or(|p| area(&rings[j]) < area(&rings[p]))
            {
                parent = Some(j);
            }
        }
        match parent {
            Some(p) if depth[p].is_multiple_of(2) => {
                depth[i] = depth[p] + 1;
                if let Some(poly) = polys.iter_mut().find(|(o, _)| *o == p) {
                    poly.1.push(rings[i].clone());
                }
            }
            Some(p) => {
                depth[i] = depth[p] + 1;
                polys.push((i, vec![rings[i].clone()]));
            }
            None => polys.push((i, vec![rings[i].clone()])),
        }
    }
    polys.into_iter().map(|(_, r)| r).collect()
}

fn build(d: Data, ds: &mut Dataset) {
    let mut nodes = d.nodes;
    nodes.sort_unstable_by_key(|n| n.0);
    nodes.dedup_by_key(|n| n.0);
    let loc = |id: i64| -> Option<Pt> {
        nodes
            .binary_search_by_key(&id, |n| n.0)
            .ok()
            .map(|i| nodes[i].1)
            .filter(|p| p[0].is_finite() && p[1].is_finite())
    };
    // The layers and their fields, as GDAL's default configuration makes them.
    let new = |name: &str, ty: &str, ids: &[&str], fields: &[&str], extra: &[&str]| {
        let mut l = Layer::new(name);
        l.declared_type = Some(ty.into());
        for f in ids.iter().chain(fields).chain(extra).chain(&["other_tags"]) {
            l.field(f);
        }
        l
    };
    let mut points = new("points", "Point", &["osm_id"], POINT_FIELDS, &[]);
    let mut lines = new(
        "lines",
        "LineString",
        &["osm_id"],
        LINE_FIELDS,
        &["z_order"],
    );
    let mut mls = new(
        "multilinestrings",
        "MultiLineString",
        &["osm_id"],
        REL_FIELDS,
        &[],
    );
    let mut mps = new(
        "multipolygons",
        "MultiPolygon",
        &["osm_id", "osm_way_id"],
        POLY_FIELDS,
        &[],
    );
    let mut other = new(
        "other_relations",
        "GeometryCollection",
        &["osm_id"],
        REL_FIELDS,
        &[],
    );
    let mut count = 0usize;
    let mut missing = 0usize;
    // Points: nodes with a significant tag.
    for (id, pt, tags) in &d.tagged {
        if !tags
            .iter()
            .any(|(k, _)| !POINT_UNSIGNIFICANT.contains(&k.as_str()))
        {
            continue;
        }
        points.push(
            Some(id.to_string()),
            Some(Geometry::Point(*pt)),
            row(&[("osm_id", Some(*id))], POINT_FIELDS, tags, false, &[]),
        );
        count += 1;
    }
    // Nodes outside the file are dropped from the way (as GDAL does); a way needs two.
    let way_pts = |w: &Way, missing: &mut usize| -> Option<Vec<Pt>> {
        let v: Vec<Pt> = w.refs.iter().filter_map(|&r| loc(r)).collect();
        if v.len() < w.refs.len() {
            *missing += 1;
        }
        (v.len() >= 2).then_some(v)
    };
    let mut way_index: HashMap<i64, usize> = HashMap::with_capacity(d.ways.len());
    for (i, w) in d.ways.iter().enumerate() {
        way_index.insert(w.id, i);
    }
    // Old-style multipolygons: a relation tagged with its type only takes the tags of its
    // one outer way, which is then not reported on its own.
    let significant = |t: &Tags| t.iter().any(|(k, _)| k != "type" && !ignored(k, true));
    let mut inherited: HashMap<i64, Tags> = HashMap::new();
    let mut in_area_rel: std::collections::HashSet<i64> = Default::default();
    for r in &d.rels {
        let ty = r
            .tags
            .iter()
            .find(|(k, _)| k == "type")
            .map(|(_, v)| v.as_str());
        if matches!(ty, Some("multipolygon" | "boundary")) {
            in_area_rel.extend(
                r.members
                    .iter()
                    .filter(|m| m.0 == 1 && m.2 == "outer")
                    .map(|m| m.1),
            );
        }
    }
    let mut consumed: std::collections::HashSet<i64> = Default::default();
    for r in &d.rels {
        let ty = r
            .tags
            .iter()
            .find(|(k, _)| k == "type")
            .map(|(_, v)| v.as_str());
        if ty != Some("multipolygon") || significant(&r.tags) {
            continue;
        }
        let outers: Vec<i64> = r
            .members
            .iter()
            .filter(|m| m.0 == 1 && m.2 == "outer")
            .map(|m| m.1)
            .collect();
        if let [o] = outers.as_slice()
            && let Some(w) = way_index.get(o).map(|&i| &d.ways[i])
            && significant(&w.tags)
        {
            let mut t = r.tags.clone();
            t.extend(w.tags.iter().filter(|(k, _)| k != "type").cloned());
            inherited.insert(r.id, t);
            consumed.insert(*o);
        }
    }
    let mut way_polys = Layer::new("way polygons");
    for w in &d.ways {
        if count >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        let closed = w.refs.len() >= 4 && w.refs.first() == w.refs.last();
        // Untagged ways are not reported, nor the outer ways whose tags a relation took,
        // nor areas that are outer members of a multipolygon or boundary (the relation
        // draws them; inner ones are areas of their own).
        if w.tags.is_empty() || consumed.contains(&w.id) {
            continue;
        }
        if closed && is_area(&w.tags) {
            // An area needs a tag besides "area" and the ignored ones.
            let meaningful = w.tags.iter().any(|(k, _)| !ignored(k, true));
            if in_area_rel.contains(&w.id) || !meaningful {
                continue;
            }
            let Some(p) = way_pts(w, &mut missing) else {
                continue;
            };
            // After the relations' polygons, in GDAL's order.
            way_polys.push(
                Some(w.id.to_string()),
                Some(Geometry::MultiPolygon(vec![vec![p]])),
                row(
                    &[("osm_id", None), ("osm_way_id", Some(w.id))],
                    POLY_FIELDS,
                    &w.tags,
                    true,
                    &[],
                ),
            );
            count += 1;
            continue;
        }
        let Some(p) = way_pts(w, &mut missing) else {
            continue;
        };
        lines.push(
            Some(w.id.to_string()),
            Some(Geometry::LineString(p)),
            row(
                &[("osm_id", Some(w.id))],
                LINE_FIELDS,
                &w.tags,
                false,
                &[("z_order", Value::Int(z_order(&w.tags)))],
            ),
        );
        count += 1;
    }
    for r in &d.rels {
        if count >= MAX_FEATURES {
            ds.warn("too many features; the rest are not shown");
            break;
        }
        let ty = r
            .tags
            .iter()
            .find(|(k, _)| k == "type")
            .map(|(_, v)| v.as_str());
        let member_way = |id: i64, missing: &mut usize| -> Option<Vec<Pt>> {
            way_index
                .get(&id)
                .and_then(|&i| way_pts(&d.ways[i], missing))
        };
        match ty {
            Some("multipolygon" | "boundary") => {
                let mut parts = Vec::new();
                let mut complete = true;
                for m in r.members.iter().filter(|m| m.0 == 1) {
                    match member_way(m.1, &mut missing) {
                        Some(p) => parts.push(p),
                        None => complete = false,
                    }
                }
                if !complete || parts.is_empty() {
                    continue;
                }
                let Some(rs) = rings(parts) else { continue };
                if rs.is_empty() {
                    continue;
                }
                let tags = inherited.get(&r.id).unwrap_or(&r.tags);
                mps.push(
                    Some(r.id.to_string()),
                    Some(Geometry::MultiPolygon(organise(rs))),
                    row(
                        &[("osm_id", Some(r.id)), ("osm_way_id", None)],
                        POLY_FIELDS,
                        tags,
                        true,
                        &[],
                    ),
                );
                count += 1;
            }
            Some("multilinestring" | "route") => {
                let parts: Vec<Vec<Pt>> = r
                    .members
                    .iter()
                    .filter(|m| m.0 == 1)
                    .filter_map(|m| member_way(m.1, &mut missing))
                    .collect();
                if parts.is_empty() {
                    continue;
                }
                mls.push(
                    Some(r.id.to_string()),
                    Some(Geometry::MultiLineString(parts)),
                    row(&[("osm_id", Some(r.id))], REL_FIELDS, &r.tags, true, &[]),
                );
                count += 1;
            }
            _ => {
                let mut geoms = Vec::new();
                for m in &r.members {
                    match m.0 {
                        0 => {
                            if let Some(p) = loc(m.1) {
                                geoms.push(Geometry::Point(p));
                            }
                        }
                        1 => {
                            if let Some(p) = member_way(m.1, &mut missing) {
                                geoms.push(Geometry::LineString(p));
                            }
                        }
                        _ => {}
                    }
                }
                if geoms.is_empty() {
                    continue;
                }
                other.push(
                    Some(r.id.to_string()),
                    Some(Geometry::Collection(geoms)),
                    row(&[("osm_id", Some(r.id))], REL_FIELDS, &r.tags, true, &[]),
                );
                count += 1;
            }
        }
    }
    for f in std::mem::take(&mut way_polys.features) {
        let values = way_polys.fields().iter().cloned().zip(f.values).collect();
        mps.push(f.id, f.geometry, values);
    }
    if missing > 0 {
        ds.warn(format!(
            "{missing} ways refer to nodes outside the file; those nodes are left out"
        ));
    }
    for mut l in [points, lines, mls, mps, other] {
        let n = crate::model::dense_width(l.fields().len());
        for f in &mut l.features {
            if f.values.len() < n {
                f.values.resize(n, Value::Null);
            }
        }
        ds.layers.push(l);
    }
}

fn finish(d: Data, format: &'static str) -> Dataset {
    let mut ds = Dataset {
        format,
        crs: crate::crs::from_epsg(4326),
        ..Dataset::default()
    };
    if d.tag_bytes + 4096 >= MAX_TAG_BYTES {
        ds.warn("too much tag text; some tags are not shown");
    }
    for w in &d.warnings {
        ds.warn(w.clone());
    }
    build(d, &mut ds);
    ds
}

pub fn read_xml_file(data: &[u8]) -> Result<Dataset, String> {
    Ok(finish(read_xml(data)?, "OSM XML"))
}

pub fn read_pbf_file(data: &[u8]) -> Result<Dataset, String> {
    Ok(finish(read_pbf(data)?, "OSM PBF"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4096 tags naming one 1 MiB string: the copies stop at the byte cap.
    #[test]
    fn shared_string_copies_bounded() {
        let st = vec!["x".repeat(1 << 20)];
        let mut spent = 0;
        let mut n = 0;
        for _ in 0..4096 {
            n += tags_of(&[0], &[0], &st, &mut spent).len();
        }
        assert!(n < 600, "{n}");
        assert!(spent <= MAX_TAG_BYTES);
    }

    const XML: &[u8] = br#"<?xml version="1.0"?>
<osm version="0.6">
 <node id="1" lat="0" lon="0"/><node id="2" lat="0" lon="1"/><node id="3" lat="1" lon="1"/>
 <node id="4" lat="1" lon="0"><tag k="amenity" v="cafe"/><tag k="name" v="Caf&#233; &amp; Co"/></node>
 <node id="5" lat="0.2" lon="0.2"/><node id="6" lat="0.2" lon="0.4"/><node id="7" lat="0.4" lon="0.4"/>
 <node id="8" lat="5" lon="5"><tag k="created_by" v="x"/></node>
 <way id="10"><nd ref="1"/><nd ref="2"/><nd ref="3"/></way>
 <way id="11"><nd ref="3"/><nd ref="4"/><nd ref="1"/></way>
 <way id="12"><nd ref="5"/><nd ref="6"/><nd ref="7"/><nd ref="5"/></way>
 <way id="13"><nd ref="1"/><nd ref="3"/><tag k="highway" v="primary"/><tag k="bridge" v="yes"/><tag k="maxspeed" v="50"/></way>
 <way id="14"><nd ref="1"/><nd ref="2"/><nd ref="3"/><nd ref="1"/><tag k="building" v="yes"/></way>
 <way id="15"><nd ref="1"/><nd ref="99"/><tag k="highway" v="path"/></way>
 <relation id="20"><member type="way" ref="10" role="outer"/><member type="way" ref="11" role="outer"/><member type="way" ref="12" role="inner"/><tag k="type" v="multipolygon"/><tag k="landuse" v="forest"/></relation>
 <relation id="21"><member type="node" ref="4" role=""/><tag k="type" v="site"/></relation>
</osm>"#;

    #[test]
    fn xml_layers() {
        assert!(sniff_xml(XML));
        let d = read_xml_file(XML).unwrap();
        let by = |n: &str| d.layers.iter().find(|l| l.name == n).unwrap();
        let p = by("points");
        assert_eq!(p.features.len(), 1);
        let a = p.attributes(0);
        assert!(
            a.contains(&("name".into(), Value::Text("Café & Co".into()))),
            "{a:?}"
        );
        assert!(a.contains(&(
            "other_tags".into(),
            Value::Text("\"amenity\"=>\"cafe\"".into())
        )));
        let l = by("lines");
        // Way 13 only: 10 and 11 are untagged relation members, 15 is incomplete.
        assert_eq!(l.features.len(), 1);
        let a = l.attributes(0);
        assert!(a.contains(&("z_order".into(), Value::Int(17))), "{a:?}");
        let m = by("multipolygons");
        assert_eq!(m.features.len(), 2);
        // Relations first, then closed ways (GDAL's order).
        let Some(Geometry::MultiPolygon(polys)) = &m.features[0].geometry else {
            panic!()
        };
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0].len(), 2, "outer and hole");
        assert_eq!(by("other_relations").features.len(), 1);
        assert!(d.warnings.iter().any(|w| w.contains("outside the file")));
    }

    #[test]
    fn pbf_blocks() {
        // One OSMHeader blob and one OSMData blob holding dense nodes and a way.
        fn field(k: u32, wire: u8, out: &mut Vec<u8>) {
            let mut v = u64::from(k << 3 | u32::from(wire));
            loop {
                let b = (v & 0x7F) as u8;
                v >>= 7;
                out.push(if v > 0 { b | 0x80 } else { b });
                if v == 0 {
                    break;
                }
            }
        }
        fn varint(mut v: u64, out: &mut Vec<u8>) {
            loop {
                let b = (v & 0x7F) as u8;
                v >>= 7;
                out.push(if v > 0 { b | 0x80 } else { b });
                if v == 0 {
                    break;
                }
            }
        }
        fn bytes(k: u32, b: &[u8], out: &mut Vec<u8>) {
            field(k, 2, out);
            varint(b.len() as u64, out);
            out.extend_from_slice(b);
        }
        fn zig(v: i64) -> u64 {
            ((v << 1) ^ (v >> 63)) as u64
        }
        fn packed_s(v: &[i64]) -> Vec<u8> {
            let mut o = Vec::new();
            for &x in v {
                varint(zig(x), &mut o);
            }
            o
        }
        fn file_block(ty: &[u8], raw: &[u8], out: &mut Vec<u8>) {
            let mut blob = Vec::new();
            bytes(1, raw, &mut blob);
            let mut h = Vec::new();
            bytes(1, ty, &mut h);
            field(3, 0, &mut h);
            varint(blob.len() as u64, &mut h);
            out.extend_from_slice(&(h.len() as u32).to_be_bytes());
            out.extend_from_slice(&h);
            out.extend_from_slice(&blob);
        }
        let mut header = Vec::new();
        bytes(4, b"OsmSchema-V0.6", &mut header);
        bytes(4, b"DenseNodes", &mut header);
        // String table: "", "highway", "path", "name", "Lane".
        let mut st = Vec::new();
        for s in ["", "highway", "path", "name", "Lane"] {
            bytes(1, s.as_bytes(), &mut st);
        }
        let mut dense = Vec::new();
        bytes(1, &packed_s(&[1, 1, 1]), &mut dense); // ids 1, 2, 3
        bytes(8, &packed_s(&[100_000_000, 10_000_000, 0]), &mut dense); // lat 10, 11, 11 (gran 100)
        bytes(9, &packed_s(&[200_000_000, 0, 10_000_000]), &mut dense); // lon 20, 20, 21
        let mut kv = Vec::new();
        for v in [3u64, 4, 0, 0, 0] {
            varint(v, &mut kv);
        }
        bytes(10, &kv, &mut dense);
        let mut way = Vec::new();
        field(1, 0, &mut way);
        varint(7, &mut way);
        let mut keys = Vec::new();
        varint(1, &mut keys);
        let mut vals = Vec::new();
        varint(2, &mut vals);
        bytes(2, &keys, &mut way);
        bytes(3, &vals, &mut way);
        bytes(8, &packed_s(&[1, 1, 1]), &mut way);
        let mut group = Vec::new();
        bytes(2, &dense, &mut group);
        bytes(3, &way, &mut group);
        let mut block = Vec::new();
        bytes(1, &st, &mut block);
        bytes(2, &group, &mut block);
        let mut file = Vec::new();
        file_block(b"OSMHeader", &header, &mut file);
        file_block(b"OSMData", &block, &mut file);
        assert!(sniff_pbf(&file));
        let d = read_pbf_file(&file).unwrap();
        let p = &d.layers[0];
        assert_eq!(p.features.len(), 1);
        assert_eq!(p.features[0].geometry, Some(Geometry::Point([20.0, 10.0])));
        let l = &d.layers[1];
        assert_eq!(
            l.features[0].geometry,
            Some(Geometry::LineString(vec![
                [20.0, 10.0],
                [20.0, 11.0],
                [21.0, 11.0]
            ]))
        );
        assert!(
            l.attributes(0)
                .contains(&("highway".into(), Value::Text("path".into())))
        );
        for cut in [3, 20, file.len() - 2] {
            let _ = read_pbf_file(&file[..cut]);
        }
    }
}
