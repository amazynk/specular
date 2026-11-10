//! A dataset as a drawing: a `specular-vector` scene in the dataset's own coordinates (y
//! up), a scene layer per dataset layer, and a hit-test index over the features.
//!
//! Styles go by geometry type: polygons are a pale fill with an outline in the layer's
//! colour, lines a stroke, points a dot. All polygons are drawn first, then the lines, then
//! the points, so nothing small hides under an area.

use crate::crs::CrsKind;
use crate::model::{Dataset, GeomClass, Geometry, Pt};
use crate::shp::ring_contains;
use kurbo::{BezPath, Point, Rect};
use specular_vector::{
    Color, Generated, Item, Layer as VLayer, Shape, Sheet, SheetKind, Units, VectorScene,
};
use std::sync::Arc;

/// Layer colours: distinct, readable on white and on dark grey.
const PALETTE: [(u8, u8, u8); 8] = [
    (31, 119, 180),
    (214, 39, 40),
    (44, 160, 44),
    (148, 103, 189),
    (255, 127, 14),
    (23, 190, 207),
    (140, 86, 75),
    (227, 119, 194),
];

/// The colour of layer `i` and its pale fill.
pub fn layer_colors(i: usize) -> (Color, Color) {
    let (r, g, b) = PALETTE[i % PALETTE.len()];
    let pale = |c: u8| (u16::from(c) * 35 / 100 + 255 * 65 / 100) as u8;
    (Color::Rgb(r, g, b), Color::Rgb(pale(r), pale(g), pale(b)))
}

/// Point dot radius in CSS pixels.
pub const DOT_RADIUS: f32 = 3.0;

/// Lineweights in hundredths of a millimetre (drawn when lineweights are on).
const LINE_WEIGHT: i16 = 35;
const OUTLINE_WEIGHT: i16 = 18;

fn path_of(rings: &[Vec<Pt>], close: bool) -> BezPath {
    let mut p = BezPath::new();
    for r in rings {
        let mut it = r.iter().filter(|q| q[0].is_finite() && q[1].is_finite());
        let Some(f) = it.next() else { continue };
        p.move_to((f[0], f[1]));
        for q in it {
            p.line_to((q[0], q[1]));
        }
        if close {
            p.close_path();
        }
    }
    p
}

/// Builds the scene.
pub fn to_scene(ds: &Dataset) -> VectorScene {
    let mut layers = Vec::new();
    let mut passes: [Vec<Item>; 3] = Default::default();
    let mut counts = std::collections::BTreeMap::<&'static str, u64>::new();
    for (li, l) in ds.layers.iter().enumerate() {
        let (color, fill) = layer_colors(li);
        // S-57 charts in chart colours by object class.
        let chart = ds.format == "S-57";
        let mut n = 0u64;
        for f in &l.features {
            let Some(g) = &f.geometry else { continue };
            *counts.entry(g.type_name()).or_default() += 1;
            let st = chart
                .then(|| crate::s57::style(&l.name, l.fields(), &f.values))
                .flatten()
                .unwrap_or_default();
            let rgb = |c: [u8; 4]| Color::Rgb(c[0], c[1], c[2]);
            let ink = Ink {
                stroke: st.stroke.map_or(color, rgb),
                fill: match st.fill {
                    Some(c) if c[3] == 0 => None,
                    Some(c) => Some(rgb(c)),
                    None => Some(fill),
                },
                point: st.marker.map_or(color, rgb),
            };
            n += emit(g, li as u32, ink, &mut passes);
        }
        let generated = l.name.is_empty().then_some(Generated::Layer(li as u32 + 1));
        layers.push(VLayer {
            name: generated.map_or_else(|| l.name.clone(), Generated::english),
            generated,
            color,
            linetype: None,
            lineweight: LINE_WEIGHT,
            on: true,
            frozen: false,
            plot: true,
            items: n,
        });
    }
    let mut sheet = Sheet::generated(Generated::Map, SheetKind::Model);
    let [polys, lines, points] = passes;
    sheet.items = polys;
    sheet.items.extend(lines);
    sheet.items.extend(points);
    sheet.extents = ds.bounds().map(|b| Rect::new(b[0], b[1], b[2], b[3]));
    let units = match &ds.crs {
        Some(c) if c.kind == CrsKind::Projected => {
            match c.unit.as_ref().map_or(0.0, |u| u.metres) {
                m if (m - 1.0).abs() < 1e-12 => 6,
                m if (m - 0.3048).abs() < 1e-9 => 2,
                m if (m - 1200.0 / 3937.0).abs() < 1e-9 => 21,
                _ => 0,
            }
        }
        _ => 0,
    };
    VectorScene {
        layer_opacity: Vec::new(),
        pcb: None,
        survey: None,
        format: ds.format.to_string(),
        version: String::new(),
        release: None,
        units: Units { code: units },
        layers,
        linetypes: Vec::new(),
        fonts: vec![specular_vector::FontSpec::default()],
        sheets: vec![sheet],
        counts: counts
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        warnings: ds.warnings.clone(),
        lineweight_display: false,
        point_mode: 0,
        point_size: 0.0,
        codepage: ds.encoding.clone(),
        dot_radius: DOT_RADIUS,
        plot: None,
        layout: None,
    }
}

/// A feature's colours: lines and outlines, polygon fill (none: outline only), points.
#[derive(Clone, Copy)]
struct Ink {
    stroke: Color,
    fill: Option<Color>,
    point: Color,
}

fn emit(g: &Geometry, layer: u32, ink: Ink, passes: &mut [Vec<Item>; 3]) -> u64 {
    let (color, fill) = (ink.stroke, ink.fill);
    let item = |color, shape| Item {
        layer,
        outer: layer,
        color,
        shape,
    };
    let stroke = |path, weight| Shape::Stroke {
        path,
        weight,
        dash: None,
    };
    let polygon = |rings: &[Vec<Pt>], passes: &mut [Vec<Item>; 3]| {
        let p = path_of(rings, true);
        if p.elements().is_empty() {
            return 0;
        }
        let mut n = 1;
        if let Some(fill) = fill {
            passes[0].push(item(
                fill,
                Shape::Fill {
                    path: p.clone(),
                    even_odd: true,
                },
            ));
            n += 1;
        }
        passes[0].push(item(color, stroke(p, OUTLINE_WEIGHT)));
        n
    };
    match g {
        Geometry::Point(p) => {
            passes[2].push(item(ink.point, Shape::Point(Point::new(p[0], p[1]))));
            1
        }
        Geometry::MultiPoint(v) => {
            for p in v {
                passes[2].push(item(ink.point, Shape::Point(Point::new(p[0], p[1]))));
            }
            v.len() as u64
        }
        Geometry::LineString(l) => {
            let p = path_of(std::slice::from_ref(l), false);
            passes[1].push(item(color, stroke(p, LINE_WEIGHT)));
            1
        }
        Geometry::MultiLineString(ls) => {
            let p = path_of(ls, false);
            passes[1].push(item(color, stroke(p, LINE_WEIGHT)));
            1
        }
        Geometry::Polygon(r) => polygon(r, passes),
        Geometry::MultiPolygon(ps) => {
            // One fill for all parts: even-odd over every ring is the same picture.
            let all: Vec<Vec<Pt>> = ps.iter().flatten().cloned().collect();
            polygon(&all, passes)
        }
        Geometry::Collection(gs) => gs.iter().map(|g| emit(g, layer, ink, passes)).sum(),
    }
}

/// What is under a point of the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub layer: usize,
    pub feature: usize,
}

/// A grid over the features' bounding boxes for hit tests.
pub struct FeatureIndex {
    pub dataset: Arc<Dataset>,
    entries: Vec<(Hit, [f64; 4], GeomClass)>,
    bounds: [f64; 4],
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
    /// Entries too large for the grid (they span many cells), tested always.
    big: Vec<u32>,
}

impl FeatureIndex {
    pub fn new(dataset: Arc<Dataset>) -> FeatureIndex {
        let mut entries = Vec::new();
        for (li, l) in dataset.layers.iter().enumerate() {
            for (fi, f) in l.features.iter().enumerate() {
                if let Some(g) = &f.geometry
                    && let Some(b) = g.bounds()
                {
                    entries.push((
                        Hit {
                            layer: li,
                            feature: fi,
                        },
                        b,
                        g.class(),
                    ));
                }
            }
        }
        let bounds = dataset.bounds().unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let side = ((entries.len() as f64).sqrt().ceil() as usize).clamp(1, 512);
        let (nx, ny) = (side, side);
        let mut cells = vec![Vec::new(); nx * ny];
        let mut big = Vec::new();
        let w = (bounds[2] - bounds[0]).max(1e-12);
        let h = (bounds[3] - bounds[1]).max(1e-12);
        let cx = |x: f64| {
            (((x - bounds[0]) / w * nx as f64) as isize).clamp(0, nx as isize - 1) as usize
        };
        let cy = |y: f64| {
            (((y - bounds[1]) / h * ny as f64) as isize).clamp(0, ny as isize - 1) as usize
        };
        for (i, (_, b, _)) in entries.iter().enumerate() {
            let (x0, x1, y0, y1) = (cx(b[0]), cx(b[2]), cy(b[1]), cy(b[3]));
            if (x1 - x0 + 1) * (y1 - y0 + 1) > 64 {
                big.push(i as u32);
                continue;
            }
            for y in y0..=y1 {
                for x in x0..=x1 {
                    cells[y * nx + x].push(i as u32);
                }
            }
        }
        FeatureIndex {
            dataset,
            entries,
            bounds,
            nx,
            ny,
            cells,
            big,
        }
    }

    /// The topmost feature at `p` (map coordinates) within `tol` map units: points before
    /// lines before polygons, later features before earlier ones (drawn on top).
    pub fn hit(&self, p: Pt, tol: f64) -> Option<Hit> {
        let b = self.bounds;
        let w = (b[2] - b[0]).max(1e-12);
        let h = (b[3] - b[1]).max(1e-12);
        let cell = |v: f64, lo: f64, ext: f64, n: usize| {
            ((((v - lo) / ext) * n as f64).floor() as isize).clamp(0, n as isize - 1) as usize
        };
        let mut cand: Vec<u32> = self.big.clone();
        if p[0] >= b[0] - tol && p[0] <= b[2] + tol && p[1] >= b[1] - tol && p[1] <= b[3] + tol {
            let (x0, x1) = (
                cell(p[0] - tol, b[0], w, self.nx),
                cell(p[0] + tol, b[0], w, self.nx),
            );
            let (y0, y1) = (
                cell(p[1] - tol, b[1], h, self.ny),
                cell(p[1] + tol, b[1], h, self.ny),
            );
            for y in y0..=y1.min(y0 + 8) {
                for x in x0..=x1.min(x0 + 8) {
                    cand.extend_from_slice(&self.cells[y * self.nx + x]);
                }
            }
        }
        cand.sort_unstable();
        cand.dedup();
        let mut best: Option<(GeomClass, u32)> = None;
        for &i in &cand {
            let (hit, bb, class) = self.entries[i as usize];
            if p[0] < bb[0] - tol || p[0] > bb[2] + tol || p[1] < bb[1] - tol || p[1] > bb[3] + tol
            {
                continue;
            }
            if let Some((bc, bi)) = best
                && (class < bc || class == bc && i < bi)
            {
                continue;
            }
            let g = self.dataset.layers[hit.layer].features[hit.feature]
                .geometry
                .as_ref();
            if g.is_some_and(|g| touches(g, p, tol)) {
                best = Some((class, i));
            }
        }
        best.map(|(_, i)| self.entries[i as usize].0)
    }
}

fn seg_dist2(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (x, y) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    x * x + y * y
}

fn near_line(l: &[Pt], p: Pt, tol: f64) -> bool {
    let t2 = tol * tol;
    match l {
        [a] => seg_dist2(p, *a, *a) <= t2,
        _ => l.windows(2).any(|w| seg_dist2(p, w[0], w[1]) <= t2),
    }
}

fn in_polygon(rings: &[Vec<Pt>], p: Pt) -> bool {
    rings.iter().filter(|r| ring_contains(r, p)).count() % 2 == 1
}

fn touches(g: &Geometry, p: Pt, tol: f64) -> bool {
    match g {
        Geometry::Point(q) => seg_dist2(p, *q, *q) <= tol * tol,
        Geometry::MultiPoint(v) => v.iter().any(|q| seg_dist2(p, *q, *q) <= tol * tol),
        Geometry::LineString(l) => near_line(l, p, tol),
        Geometry::MultiLineString(ls) => ls.iter().any(|l| near_line(l, p, tol)),
        Geometry::Polygon(r) => in_polygon(r, p) || r.iter().any(|r| near_line(r, p, tol)),
        Geometry::MultiPolygon(ps) => ps
            .iter()
            .any(|r| in_polygon(r, p) || r.iter().any(|r| near_line(r, p, tol))),
        Geometry::Collection(gs) => gs.iter().any(|g| touches(g, p, tol)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Layer;

    fn ds() -> Dataset {
        let mut a = Layer::new("areas");
        a.push(
            Some("0".into()),
            Some(Geometry::Polygon(vec![
                vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
                vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]],
            ])),
            vec![],
        );
        let mut p = Layer::new("pts");
        p.push(Some("0".into()), Some(Geometry::Point([2.0, 2.0])), vec![]);
        p.push(
            Some("1".into()),
            Some(Geometry::LineString(vec![[0.0, 20.0], [10.0, 20.0]])),
            vec![],
        );
        Dataset {
            format: "GeoJSON",
            layers: vec![a, p],
            ..Dataset::default()
        }
    }

    /// Names from the file stay as they are; an unnamed layer and the map's sheet get names
    /// Specular made up, marked so the app shows them translated.
    #[test]
    fn made_up_names_are_marked() {
        let mut d = ds();
        d.layers[1].name.clear();
        let s = to_scene(&d);
        assert_eq!(s.layers[0].name, "areas");
        assert_eq!(s.layers[0].generated, None);
        assert_eq!(s.layers[1].name, "Layer 2");
        assert_eq!(s.layers[1].generated, Some(Generated::Layer(2)));
        assert_eq!(s.sheets[0].name, "Map");
        assert_eq!(s.sheets[0].generated, Some(Generated::Map));
    }

    #[test]
    fn scene_order_and_hits() {
        let d = ds();
        let s = to_scene(&d);
        assert_eq!(s.layers.len(), 2);
        let items = &s.sheets[0].items;
        assert!(matches!(items[0].shape, Shape::Fill { even_odd: true, .. }));
        assert!(matches!(items.last().unwrap().shape, Shape::Point(_)));
        let idx = FeatureIndex::new(Arc::new(d));
        assert_eq!(
            idx.hit([2.05, 2.0], 0.1),
            Some(Hit {
                layer: 1,
                feature: 0
            })
        );
        assert_eq!(
            idx.hit([8.0, 8.0], 0.1),
            Some(Hit {
                layer: 0,
                feature: 0
            })
        );
        assert_eq!(idx.hit([5.0, 5.0], 0.1), None); // the hole
        assert_eq!(
            idx.hit([5.0, 20.05], 0.1),
            Some(Hit {
                layer: 1,
                feature: 1
            })
        );
        assert_eq!(idx.hit([50.0, 50.0], 0.1), None);
    }
}
