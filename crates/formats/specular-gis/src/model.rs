//! The feature model every vector reader produces: layers of features, each a geometry and
//! a row of attributes.

use crate::crs::Crs;

/// A position (x, y) in the dataset's CRS: easting/northing or longitude/latitude.
pub type Pt = [f64; 2];

#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Point(Pt),
    MultiPoint(Vec<Pt>),
    LineString(Vec<Pt>),
    MultiLineString(Vec<Vec<Pt>>),
    /// Rings: the exterior first, then holes. Drawn and hit-tested even-odd.
    Polygon(Vec<Vec<Pt>>),
    MultiPolygon(Vec<Vec<Vec<Pt>>>),
    Collection(Vec<Geometry>),
}

/// The drawing class of a geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeomClass {
    Polygon,
    Line,
    Point,
}

impl Geometry {
    /// Visit every coordinate.
    pub fn for_each_pt(&self, f: &mut dyn FnMut(Pt)) {
        match self {
            Geometry::Point(p) => f(*p),
            Geometry::MultiPoint(v) | Geometry::LineString(v) => v.iter().for_each(|p| f(*p)),
            Geometry::MultiLineString(v) | Geometry::Polygon(v) => {
                v.iter().flatten().for_each(|p| f(*p))
            }
            Geometry::MultiPolygon(v) => v.iter().flatten().flatten().for_each(|p| f(*p)),
            Geometry::Collection(g) => g.iter().for_each(|g| g.for_each_pt(f)),
        }
    }

    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        self.for_each_pt(&mut |p| {
            if p[0].is_finite() && p[1].is_finite() {
                b[0] = b[0].min(p[0]);
                b[1] = b[1].min(p[1]);
                b[2] = b[2].max(p[0]);
                b[3] = b[3].max(p[1]);
            }
        });
        (b[0] <= b[2]).then_some(b)
    }

    /// The highest class present (polygons rank lowest: drawn first).
    pub fn class(&self) -> GeomClass {
        match self {
            Geometry::Point(_) | Geometry::MultiPoint(_) => GeomClass::Point,
            Geometry::LineString(_) | Geometry::MultiLineString(_) => GeomClass::Line,
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => GeomClass::Polygon,
            Geometry::Collection(g) => g
                .iter()
                .map(|g| g.class())
                .max()
                .unwrap_or(GeomClass::Point),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Geometry::Point(_) => "Point",
            Geometry::MultiPoint(_) => "MultiPoint",
            Geometry::LineString(_) => "LineString",
            Geometry::MultiLineString(_) => "MultiLineString",
            Geometry::Polygon(_) => "Polygon",
            Geometry::MultiPolygon(_) => "MultiPolygon",
            Geometry::Collection(_) => "GeometryCollection",
        }
    }

    pub fn vertex_count(&self) -> usize {
        let mut n = 0;
        self.for_each_pt(&mut |_| n += 1);
        n
    }
}

/// An attribute value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    /// ISO 8601 date or date-time.
    Date(String),
    /// A binary value: its length.
    Blob(usize),
}

impl Value {
    /// The value as display text.
    pub fn text(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    format!("{f:.1}")
                } else {
                    format!("{f}")
                }
            }
            Value::Text(s) | Value::Date(s) => s.clone(),
            Value::Blob(n) => format!("({n} bytes)"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    /// The feature id the file gave (GeoPackage fid, GeoJSON id), else its index.
    pub id: Option<String>,
    pub geometry: Option<Geometry>,
    /// Aligned with the layer's `fields`.
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layer {
    pub name: String,
    fields: Vec<String>,
    pub features: Vec<Feature>,
    /// The geometry type the layer declares (Shapefile, GeoPackage), if any.
    pub declared_type: Option<String>,
    /// `fields` by name, for [`Layer::field`] (a scan per lookup was quadratic in the fields).
    index: FieldIndex,
}

#[cfg(test)]
thread_local! {
    /// Names put in a field index (tests count them).
    static INDEXED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Field positions by name, covering `fields[..covered]`. `fields` changes only through
/// [`Layer`]'s methods, which keep the index right: appends extend it, a rename moves one
/// name, replacing the list starts it afresh. Not part of a layer's value: clones start
/// empty (indexed on first lookup), and two indexes are always equal.
#[derive(Default)]
struct FieldIndex {
    by_name: std::collections::HashMap<String, usize>,
    covered: usize,
    /// Positions whose name an earlier field already has.
    dups: usize,
}

impl Clone for FieldIndex {
    fn clone(&self) -> FieldIndex {
        FieldIndex::default()
    }
}

impl PartialEq for FieldIndex {
    fn eq(&self, _: &FieldIndex) -> bool {
        true
    }
}

impl std::fmt::Debug for FieldIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FieldIndex")
    }
}

impl FieldIndex {
    /// Indexes the fields appended since the last call.
    fn sync(&mut self, fields: &[String]) {
        for (i, f) in fields.iter().enumerate().skip(self.covered) {
            // The first of equal names wins, as a scan would find it.
            match self.by_name.entry(f.clone()) {
                std::collections::hash_map::Entry::Occupied(_) => self.dups += 1,
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(i);
                }
            }
            #[cfg(test)]
            INDEXED.with(|n| n.set(n.get() + 1));
        }
        self.covered = fields.len();
    }
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Layer {
        Layer {
            name: name.into(),
            ..Layer::default()
        }
    }

    /// The field names; feature values are aligned with them.
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Replaces the field names (feature values are not touched).
    pub fn set_fields(&mut self, fields: Vec<String>) {
        self.fields = fields;
        self.index = FieldIndex::default();
    }

    /// Renames field `i` (nothing when there is no such field).
    pub fn rename_field(&mut self, i: usize, name: impl Into<String>) {
        let name = name.into();
        let Some(old) = self.fields.get(i) else {
            return;
        };
        if *old == name {
            return;
        }
        self.index.sync(&self.fields);
        let old = std::mem::replace(&mut self.fields[i], name.clone());
        let ix = &mut self.index;
        // The old name: gone, or now found at its next occurrence.
        if ix.by_name.get(&old) == Some(&i) {
            ix.by_name.remove(&old);
            if ix.dups > 0
                && let Some(j) = self.fields[i + 1..].iter().position(|f| *f == old)
            {
                ix.by_name.insert(old, i + 1 + j);
                ix.dups -= 1;
            }
        } else {
            ix.dups -= 1;
        }
        // The new name: here, unless an earlier field has it.
        match ix.by_name.get_mut(&name) {
            Some(j) if *j < i => ix.dups += 1,
            Some(j) => {
                *j = i;
                ix.dups += 1;
            }
            None => {
                ix.by_name.insert(name, i);
            }
        }
    }

    /// Index of field `name`, adding it (and padding earlier rows) if new.
    pub fn field(&mut self, name: &str) -> usize {
        self.index.sync(&self.fields);
        if let Some(&i) = self.index.by_name.get(name) {
            return i;
        }
        self.fields.push(name.to_string());
        let i = self.fields.len() - 1;
        self.index.sync(&self.fields);
        i
    }

    /// Push a feature from (field, value) pairs.
    pub fn push(
        &mut self,
        id: Option<String>,
        geometry: Option<Geometry>,
        attrs: Vec<(String, Value)>,
    ) {
        // Rows are padded to every field only while a layer has few; past that a row holds
        // the fields up to the last it sets (readers treat missing values as null). A file
        // naming 50 000 distinct keys would otherwise make every row 50 000 values long.
        let mut values = vec![Value::Null; dense_width(self.fields.len())];
        for (k, v) in attrs {
            let i = self.field(&k);
            if values.len() <= i {
                values.resize(i + 1, Value::Null);
            }
            values[i] = v;
        }
        self.features.push(Feature {
            id,
            geometry,
            values,
        });
    }

    /// A feature's attributes as (field, value) pairs.
    pub fn attributes(&self, i: usize) -> Vec<(String, Value)> {
        let Some(f) = self.features.get(i) else {
            return Vec::new();
        };
        self.fields
            .iter()
            .enumerate()
            .map(|(k, n)| (n.clone(), f.values.get(k).cloned().unwrap_or(Value::Null)))
            .collect()
    }
}

/// Raster tiles in a GeoPackage: one tile pyramid.
#[derive(Clone, Debug, PartialEq)]
pub struct TileSet {
    pub table: String,
    pub identifier: Option<String>,
    pub srs_id: i64,
    /// Matrix set bounds (min x, min y, max x, max y).
    pub bounds: [f64; 4],
    pub levels: Vec<TileLevel>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileLevel {
    pub zoom: i64,
    pub matrix_w: i64,
    pub matrix_h: i64,
    pub tile_w: u32,
    pub tile_h: u32,
    pub pixel_x: f64,
    pub pixel_y: f64,
}

/// What a vector file holds.
#[derive(Clone, Debug, Default)]
pub struct Dataset {
    /// "GeoJSON", "Shapefile", "GeoPackage", "KML", "KMZ", "GPX".
    pub format: &'static str,
    pub layers: Vec<Layer>,
    pub crs: Option<Crs>,
    /// The CRS reference as given when no definition was found (e.g. an unknown EPSG code).
    pub crs_note: Option<String>,
    /// Text encoding of the attributes, when the format names one (Shapefile).
    pub encoding: Option<String>,
    pub tiles: Vec<TileSet>,
    pub warnings: Vec<String>,
    /// KML ground overlays: images placed on the map.
    pub overlays: Vec<GroundOverlay>,
}

/// A KML GroundOverlay: an image laid over a longitude/latitude box (rotated
/// counter-clockwise by `rotation` degrees about its centre) or a quadrilateral.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundOverlay {
    pub name: Option<String>,
    /// The image as the file gives it (a KMZ member path or a URL; never fetched).
    pub href: String,
    pub north: f64,
    pub south: f64,
    pub east: f64,
    pub west: f64,
    pub rotation: f64,
    /// gx:LatLonQuad corners (lon, lat), counter-clockwise from the lower left.
    pub quad: Option<[Pt; 4]>,
}

impl Dataset {
    pub fn feature_count(&self) -> usize {
        self.layers.iter().map(|l| l.features.len()).sum()
    }

    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut b: Option<[f64; 4]> = None;
        for g in self
            .layers
            .iter()
            .flat_map(|l| &l.features)
            .filter_map(|f| f.geometry.as_ref())
        {
            if let Some(g) = g.bounds() {
                b = Some(match b {
                    None => g,
                    Some(b) => [
                        b[0].min(g[0]),
                        b[1].min(g[1]),
                        b[2].max(g[2]),
                        b[3].max(g[3]),
                    ],
                });
            }
        }
        b
    }

    pub fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) && self.warnings.len() < 32 {
            self.warnings.push(w);
        }
    }
}

/// Limits on what one file may make the readers hold.
pub const MAX_FEATURES: usize = 20_000_000;
/// Rows are padded to all of a layer's fields while it has at most this many.
pub const DENSE_FIELDS: usize = 256;

/// How long a row is padded with nulls when its layer has `fields` fields.
pub fn dense_width(fields: usize) -> usize {
    if fields <= DENSE_FIELDS { fields } else { 0 }
}
pub const MAX_VERTICES: usize = 200_000_000;

/// A circular arc through `p0`, `p1` and `p2` (start, a point on the arc, end) as points,
/// appended to `out` without `p0` (so arcs chain). At most 4° apart, like GDAL's default
/// stroking; collinear points give the straight segment.
pub fn arc_to(p0: Pt, p1: Pt, p2: Pt, out: &mut Vec<Pt>) {
    let (ax, ay) = (p0[0], p0[1]);
    let (bx, by) = (p1[0], p1[1]);
    let (cx, cy) = (p2[0], p2[1]);
    let tau = std::f64::consts::TAU;
    if p0 == p2 && p0 != p1 {
        // A full circle: p1 is the opposite point.
        let (ox, oy) = ((ax + bx) / 2.0, (ay + by) / 2.0);
        let r = ((ax - ox).powi(2) + (ay - oy).powi(2)).sqrt();
        let a0 = (ay - oy).atan2(ax - ox);
        for k in 1..90 {
            let a = a0 + tau * f64::from(k) / 90.0;
            out.push([ox + r * a.cos(), oy + r * a.sin()]);
        }
        out.push(p2);
        return;
    }
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    let scale = [ax, ay, bx, by, cx, cy]
        .iter()
        .fold(0.0f64, |m, v| m.max(v.abs()))
        .max(1.0);
    if !d.is_finite() || d.abs() < 1e-12 * scale * scale {
        out.push(p1);
        out.push(p2);
        return;
    }
    let sq = |x: f64, y: f64| x * x + y * y;
    let ox = (sq(ax, ay) * (by - cy) + sq(bx, by) * (cy - ay) + sq(cx, cy) * (ay - by)) / d;
    let oy = (sq(ax, ay) * (cx - bx) + sq(bx, by) * (ax - cx) + sq(cx, cy) * (bx - ax)) / d;
    let r = sq(ax - ox, ay - oy).sqrt();
    let ang = |x: f64, y: f64| (y - oy).atan2(x - ox);
    let (a0, a1, a2) = (ang(ax, ay), ang(bx, by), ang(cx, cy));
    let norm = |a: f64| a.rem_euclid(tau);
    // Counter-clockwise when the middle point lies on the CCW sweep from start to end.
    let ccw = norm(a1 - a0) < norm(a2 - a0);
    let sweep = if ccw { norm(a2 - a0) } else { -norm(a0 - a2) };
    let steps = ((sweep.abs() / 4f64.to_radians()).ceil() as usize).clamp(2, 3600);
    for k in 1..steps {
        let a = a0 + sweep * k as f64 / steps as f64;
        out.push([ox + r * a.cos(), oy + r * a.sin()]);
    }
    out.push(p2);
}

/// A CircularString's points (arcs of three, chained) as a line.
pub fn stroke_circular(pts: &[Pt]) -> Vec<Pt> {
    let Some(&first) = pts.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    let mut i = 0;
    while i + 2 < pts.len() {
        arc_to(pts[i], pts[i + 1], pts[i + 2], &mut out);
        i += 2;
    }
    out.extend_from_slice(&pts[i + 1..]);
    out
}

#[cfg(test)]
mod arc_tests {
    use super::*;

    /// One row naming 20 000 distinct keys, then 10 000 rows of one key each: the short
    /// rows stay short (padded, they would hold 200 million values).
    #[test]
    fn wide_layers_stay_sparse() {
        let mut l = Layer::new("t");
        let wide = (0..20_000)
            .map(|k| (format!("k{k}"), Value::Int(k)))
            .collect();
        l.push(None, None, wide);
        for k in 0..10_000 {
            l.push(None, None, vec![("k0".into(), Value::Int(k))]);
        }
        let cells: usize = l.features.iter().map(|f| f.values.len()).sum();
        assert!(cells < 40_000, "{cells}");
        assert_eq!(l.attributes(5).len(), 20_000);
        assert_eq!(l.attributes(5)[1].1, Value::Null);
    }

    #[test]
    fn half_circle() {
        let line = stroke_circular(&[[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]]);
        assert_eq!(line.first(), Some(&[1.0, 0.0]));
        assert_eq!(line.last(), Some(&[-1.0, 0.0]));
        assert_eq!(line.len(), 46);
        for p in &line {
            assert!((p[0].hypot(p[1]) - 1.0).abs() < 1e-12);
            assert!(p[1] >= -1e-12, "{p:?}");
        }
        // Clockwise the other way round.
        let cw = stroke_circular(&[[1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]]);
        assert!(cw[1..cw.len() - 1].iter().all(|p| p[1] < 0.0));
        // Straight when collinear.
        assert_eq!(
            stroke_circular(&[[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]),
            vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]
        );
    }

    /// 100,000 distinct keys, each looked up again: a scan per lookup compared 10¹⁰ names;
    /// the index puts each name in about once (twice across reallocations).
    #[test]
    fn field_lookup_is_linear() {
        let n = 100_000;
        INDEXED.with(|c| c.set(0));
        let mut l = Layer::new("hostile");
        for i in 0..n {
            assert_eq!(l.field(&format!("k{i}")), i);
        }
        for i in (0..n).rev() {
            assert_eq!(l.field(&format!("k{i}")), i);
        }
        // Each name indexed once: appends extend the index, nothing rescans.
        assert_eq!(INDEXED.with(std::cell::Cell::get), n);
        // Renaming every field: each rename touches its two names only.
        for i in 0..n {
            l.rename_field(i, format!("r{i}"));
        }
        assert_eq!(INDEXED.with(std::cell::Cell::get), n);
        for i in (0..n).step_by(997) {
            assert_eq!(l.field(&format!("r{i}")), i);
        }
        assert_eq!(l.field("k5"), n);
        // Replaced and cloned fields are indexed afresh.
        l.set_fields(vec!["b".into(), "a".into()]);
        assert_eq!(l.field("a"), 1);
        l.set_fields(vec!["b".into()]);
        assert_eq!(l.field("a"), 1);
        let mut c = l.clone();
        assert_eq!(c.field("b"), 0);
        assert_eq!(c, l);
    }

    /// Every lookup after any rename answers as a scan would: the first field of that name,
    /// added only when none has it. Checked against a scan over a sequence of renames that
    /// make, move and undo duplicates.
    #[test]
    fn field_index_follows_renames() {
        let mut l = Layer::new("renames");
        for n in ["a", "b", "a", "c", "b"] {
            l.set_fields(l.fields().iter().cloned().chain([n.to_owned()]).collect());
        }
        let names = ["a", "b", "c", "d", "e"];
        let mut seed = 7u32;
        for step in 0..2000 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let i = (seed >> 8) as usize % l.fields().len();
            let to = names[(seed >> 20) as usize % names.len()];
            l.rename_field(i, to);
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let ask = names[(seed >> 16) as usize % names.len()];
            let scan = l.fields().iter().position(|f| f == ask);
            let before = l.fields().len();
            let got = l.field(ask);
            match scan {
                Some(j) => assert_eq!(got, j, "step {step}: {ask} in {:?}", l.fields()),
                None => {
                    assert_eq!(got, before, "step {step}");
                    // Keep the list short: drop the added name again.
                    let mut f = l.fields().to_vec();
                    f.pop();
                    l.set_fields(f);
                }
            }
        }
        // The case round 3 missed: a rename in place to a name looked up next.
        let mut l = Layer::new("x");
        assert_eq!(l.field("a"), 0);
        l.rename_field(0, "b");
        assert_eq!(l.field("b"), 0);
        assert_eq!(l.field("a"), 1);
        assert_eq!(l.fields(), ["b", "a"]);
    }
}
