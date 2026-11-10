//! Mapbox Vector Tiles (MVT 2.x): protobuf tiles of named layers whose features carry a
//! geometry in tile coordinates (commands MoveTo, LineTo, ClosePath with zigzag deltas)
//! and attributes as indices into the layer's key and value tables.
//!
//! [`decode_into`] reads a tile at z/x/y into the feature model with coordinates in Web
//! Mercator metres (EPSG:3857), merging layers of the same name across tiles.

use std::collections::HashMap;

use crate::model::{Geometry, Layer, Pt, Value};
use crate::tiles;

/// Features read from one tile at most.
const MAX_FEATURES: usize = 1 << 20;
/// Commands one geometry may hold.
const MAX_GEOMETRY: usize = 1 << 24;

struct Pb<'a> {
    b: &'a [u8],
    i: usize,
}

enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed64(u64),
    Fixed32(u32),
}

impl<'a> Pb<'a> {
    fn new(b: &'a [u8]) -> Self {
        Pb { b, i: 0 }
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *self.b.get(self.i).ok_or("vector tile truncated")?;
            self.i += 1;
            v |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("vector tile has a malformed number".into())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self
            .i
            .checked_add(n)
            .and_then(|e| self.b.get(self.i..e))
            .ok_or("vector tile truncated")?;
        self.i += n;
        Ok(s)
    }

    fn next(&mut self) -> Result<Option<(u64, Field<'a>)>, String> {
        if self.i >= self.b.len() {
            return Ok(None);
        }
        let key = self.varint()?;
        let f = match key & 7 {
            0 => Field::Varint(self.varint()?),
            1 => Field::Fixed64(u64::from_le_bytes(
                self.take(8)?.try_into().expect("8 bytes"),
            )),
            2 => {
                let n = usize::try_from(self.varint()?).map_err(|_| "vector tile length")?;
                Field::Bytes(self.take(n)?)
            }
            5 => Field::Fixed32(u32::from_le_bytes(
                self.take(4)?.try_into().expect("4 bytes"),
            )),
            w => return Err(format!("vector tile wire type {w}")),
        };
        Ok(Some((key >> 3, f)))
    }
}

fn packed(b: &[u8], out: &mut Vec<u32>) -> Result<(), String> {
    let mut p = Pb::new(b);
    while p.i < b.len() {
        if out.len() >= MAX_GEOMETRY {
            return Err("vector tile geometry too large".into());
        }
        out.push(p.varint()? as u32);
    }
    Ok(())
}

fn value(b: &[u8]) -> Result<Value, String> {
    let mut p = Pb::new(b);
    let mut v = Value::Null;
    while let Some((tag, f)) = p.next()? {
        v = match (tag, f) {
            (1, Field::Bytes(s)) => Value::Text(String::from_utf8_lossy(s).into_owned()),
            (2, Field::Fixed32(x)) => Value::Float(f64::from(f32::from_bits(x))),
            (3, Field::Fixed64(x)) => Value::Float(f64::from_bits(x)),
            (4, Field::Varint(x)) => Value::Int(x as i64),
            (5, Field::Varint(x)) => Value::Int(i64::try_from(x).unwrap_or(i64::MAX)),
            (6, Field::Varint(x)) => Value::Int(((x >> 1) as i64) ^ -((x & 1) as i64)),
            (7, Field::Varint(x)) => Value::Bool(x != 0),
            _ => continue,
        };
    }
    Ok(v)
}

/// Twice the signed area in tile coordinates (y down): positive for an exterior ring.
fn area2(r: &[Pt]) -> f64 {
    let n = r.len();
    (0..n)
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

/// A feature's geometry in tile coordinates.
fn geometry(kind: u64, cmds: &[u32]) -> Option<Geometry> {
    let (mut x, mut y) = (0i64, 0i64);
    let mut parts: Vec<Vec<Pt>> = Vec::new();
    let mut i = 0;
    while i < cmds.len() {
        let c = cmds[i];
        i += 1;
        let (id, count) = (c & 7, (c >> 3) as usize);
        match id {
            1 | 2 => {
                for k in 0..count {
                    let (Some(&dx), Some(&dy)) = (cmds.get(i), cmds.get(i + 1)) else {
                        return None;
                    };
                    i += 2;
                    x += i64::from((dx >> 1) as i32 ^ -((dx & 1) as i32));
                    y += i64::from((dy >> 1) as i32 ^ -((dy & 1) as i32));
                    if id == 1 && (kind != 1 || k == 0 || parts.is_empty()) {
                        parts.push(Vec::new());
                    }
                    parts.last_mut()?.push([x as f64, y as f64]);
                }
            }
            7 => {
                if let Some(p) = parts.last_mut()
                    && let Some(&f) = p.first()
                {
                    p.push(f);
                }
            }
            _ => return None,
        }
    }
    parts.retain(|p| !p.is_empty());
    if parts.is_empty() {
        return None;
    }
    Some(match kind {
        1 => {
            let pts: Vec<Pt> = parts.into_iter().flatten().collect();
            if pts.len() == 1 {
                Geometry::Point(pts[0])
            } else {
                Geometry::MultiPoint(pts)
            }
        }
        2 => {
            if parts.len() == 1 {
                Geometry::LineString(parts.pop()?)
            } else {
                Geometry::MultiLineString(parts)
            }
        }
        3 => {
            // An exterior ring (positive area) starts a polygon; negative ones are its holes.
            let mut polys: Vec<Vec<Vec<Pt>>> = Vec::new();
            for r in parts {
                let a = area2(&r);
                if a == 0.0 {
                    continue;
                }
                if a > 0.0 || polys.is_empty() {
                    polys.push(vec![r]);
                } else if let Some(p) = polys.last_mut() {
                    p.push(r);
                }
            }
            match polys.len() {
                0 => return None,
                1 => Geometry::Polygon(polys.pop()?),
                _ => Geometry::MultiPolygon(polys),
            }
        }
        _ => return None,
    })
}

fn map_geometry(g: &mut Geometry, f: &dyn Fn(Pt) -> Pt) {
    let line = |v: &mut Vec<Pt>| v.iter_mut().for_each(|p| *p = f(*p));
    match g {
        Geometry::Point(p) => *p = f(*p),
        Geometry::MultiPoint(v) | Geometry::LineString(v) => line(v),
        Geometry::MultiLineString(v) | Geometry::Polygon(v) => v.iter_mut().for_each(line),
        Geometry::MultiPolygon(v) => v.iter_mut().flatten().for_each(line),
        Geometry::Collection(v) => v.iter_mut().for_each(|g| map_geometry(g, f)),
    }
}

/// Most bytes of attribute text one tile's features may copy from its tables, together.
const MAX_ATTR_BYTES: usize = 256 << 20;

/// Decode tile z/x/y into `layers` (in EPSG:3857 metres), one layer per name; `index`
/// maps layer names to positions in `layers`. Returns the features added.
pub fn decode_into(
    tile: &[u8],
    (z, tx, ty): (u8, u32, u32),
    layers: &mut Vec<Layer>,
    index: &mut HashMap<String, usize>,
) -> Result<usize, String> {
    let b = tiles::tile_bounds(z, tx, ty);
    let mut added = 0usize;
    // Attribute text copied out of the layers' tables: every feature may name the same
    // large value.
    let mut spent = 0usize;
    let mut top = Pb::new(tile);
    while let Some((tag, f)) = top.next()? {
        let (3, Field::Bytes(lb)) = (tag, f) else {
            continue;
        };
        let mut name = String::new();
        let mut keys: Vec<String> = Vec::new();
        let mut values: Vec<Value> = Vec::new();
        let mut features: Vec<&[u8]> = Vec::new();
        let mut extent = 4096u64;
        let mut p = Pb::new(lb);
        while let Some((tag, f)) = p.next()? {
            match (tag, f) {
                (1, Field::Bytes(s)) => name = String::from_utf8_lossy(s).into_owned(),
                (2, Field::Bytes(s)) => features.push(s),
                (3, Field::Bytes(s)) => keys.push(String::from_utf8_lossy(s).into_owned()),
                (4, Field::Bytes(s)) => values.push(value(s)?),
                (5, Field::Varint(e)) if e > 0 => extent = e,
                _ => {}
            }
        }
        let li = *index.entry(name.clone()).or_insert_with(|| {
            layers.push(Layer::new(name.clone()));
            layers.len() - 1
        });
        let ext = extent as f64;
        let to_map = |p: Pt| -> Pt {
            [
                b[0] + p[0] / ext * (b[2] - b[0]),
                b[3] - p[1] / ext * (b[3] - b[1]),
            ]
        };
        let mut cmds = Vec::new();
        let mut tags = Vec::new();
        for fb in features {
            if added >= MAX_FEATURES {
                return Err("a vector tile with over a million features".into());
            }
            let mut id = None;
            let mut kind = 0u64;
            cmds.clear();
            tags.clear();
            let mut p = Pb::new(fb);
            while let Some((tag, f)) = p.next()? {
                match (tag, f) {
                    (1, Field::Varint(v)) => id = Some(v.to_string()),
                    (2, Field::Bytes(s)) => packed(s, &mut tags)?,
                    (3, Field::Varint(v)) => kind = v,
                    (4, Field::Bytes(s)) => packed(s, &mut cmds)?,
                    _ => {}
                }
            }
            let mut g = geometry(kind, &cmds);
            if let Some(g) = g.as_mut() {
                map_geometry(g, &to_map);
            }
            let mut attrs = Vec::new();
            for kv in tags.chunks_exact(2) {
                let (Some(k), Some(v)) = (keys.get(kv[0] as usize), values.get(kv[1] as usize))
                else {
                    continue;
                };
                let text = if let Value::Text(t) = v { t.len() } else { 0 };
                spent = spent.saturating_add(k.len() + text + 64);
                if spent > MAX_ATTR_BYTES {
                    return Err("a vector tile with too much attribute text".into());
                }
                attrs.push((k.clone(), v.clone()));
            }
            layers[li].push(id, g, attrs);
            added += 1;
        }
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 300 features naming one 1 MiB value: refused at 256 MiB of copies, not 300 MiB.
    #[test]
    fn shared_value_copies_bounded() {
        let big = "x".repeat(1 << 20);
        let feats: Vec<Vec<u8>> = (0..300)
            .map(|i| crate::fixtures::pb_feature(i, 1, &[0, 0], &[]))
            .collect();
        let layer = crate::fixtures::pb_layer("l", &feats, &["k"], &[&big]);
        let mut tile = Vec::new();
        tile.push(3 << 3 | 2);
        let mut n = layer.len();
        while n >= 0x80 {
            tile.push((n as u8 & 0x7F) | 0x80);
            n >>= 7;
        }
        tile.push(n as u8);
        tile.extend_from_slice(&layer);
        let mut layers = Vec::new();
        let e = decode_into(&tile, (0, 0, 0), &mut layers, &mut HashMap::new()).unwrap_err();
        assert!(e.contains("too much attribute text"), "{e}");
    }

    #[test]
    fn decodes_points_lines_and_polygons() {
        let tile = crate::fixtures::mvt_tile();
        let mut layers = Vec::new();
        let mut index = HashMap::new();
        let n = decode_into(&tile, (0, 0, 0), &mut layers, &mut index).unwrap();
        assert_eq!(n, 3);
        assert_eq!(layers.len(), 2);
        let places = &layers[index["places"]];
        assert_eq!(places.features.len(), 1);
        let a = places.attributes(0);
        assert!(
            a.iter().any(|(k, v)| k == "name" && v.text() == "Centre"),
            "{a:?}"
        );
        // The tile's centre is (0, 0) in Web Mercator.
        match places.features[0].geometry.as_ref().unwrap() {
            Geometry::Point(p) => assert!(p[0].abs() < 1e-6 && p[1].abs() < 1e-6, "{p:?}"),
            g => panic!("{g:?}"),
        }
        let land = &layers[index["land"]];
        assert!(
            matches!(land.features[0].geometry, Some(Geometry::Polygon(ref r)) if r.len() == 2)
        );
        assert!(matches!(
            land.features[1].geometry,
            Some(Geometry::LineString(_))
        ));
        // Garbage is an error, not a panic.
        assert!(decode_into(&[0x1A, 0xFF], (0, 0, 0), &mut layers, &mut index).is_err());
    }
}
