//! Quantity take-off: each element's count, length, area and volume, and their sums by
//! class, storey or type.
//!
//! Quantities come from the file's quantity sets (`IfcElementQuantity`, the `Qto_` sets)
//! when it has them, else they are computed from the element's mesh: the volume of a
//! closed mesh; for walls the area of one side (net of openings, when one direction
//! holds most of the wall's vertical faces); for slabs, roofs, coverings, plates and spaces
//! the area facing up; for linear elements and walls the length along the principal axis.
//! Every value says where it came from. Values are in metres, square metres and cubic
//! metres.

use std::collections::BTreeMap;

use specular_mesh::Mesh;
use specular_mesh::ifc::Element;
use specular_mesh::ifc::data::{IfcData, PropKind, SetKind};

use crate::export::Cell;

/// A quantity and where it came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Q {
    pub value: f64,
    pub from_file: bool,
}

/// One element's quantities.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub element: usize,
    pub guid: String,
    pub name: String,
    pub class: String,
    pub type_name: String,
    pub storey: String,
    pub length: Option<Q>,
    pub area: Option<Q>,
    pub volume: Option<Q>,
}

/// How to group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By {
    Class,
    Storey,
    Type,
}

/// A group's sums.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub key: String,
    pub count: usize,
    pub length: f64,
    pub area: f64,
    pub volume: f64,
    /// How many of the values summed came from the file and how many were computed.
    pub from_file: usize,
    pub computed: usize,
}

const LENGTH: &[&str] = &["Length", "NetLength", "GrossLength"];
const AREA: &[&str] = &[
    "NetSideArea",
    "NetArea",
    "NetFloorArea",
    "NetFootprintArea",
    "NetSurfaceArea",
    "GrossSideArea",
    "GrossArea",
    "GrossFloorArea",
    "GrossFootprintArea",
    "OuterSurfaceArea",
    "Area",
];
const VOLUME: &[&str] = &["NetVolume", "GrossVolume", "Volume"];

fn linear_class(c: &str) -> bool {
    matches!(
        c,
        "IFCBEAM"
            | "IFCBEAMSTANDARDCASE"
            | "IFCCOLUMN"
            | "IFCCOLUMNSTANDARDCASE"
            | "IFCMEMBER"
            | "IFCMEMBERSTANDARDCASE"
            | "IFCPILE"
            | "IFCRAILING"
            | "IFCREINFORCINGBAR"
            | "IFCTENDON"
    ) || c.ends_with("SEGMENT")
        || is_wall(c)
}

fn is_wall(c: &str) -> bool {
    c.starts_with("IFCWALL") || c == "IFCCURTAINWALL"
}

fn up_area_class(c: &str) -> bool {
    c.starts_with("IFCSLAB")
        || matches!(
            c,
            "IFCROOF"
                | "IFCCOVERING"
                | "IFCPLATE"
                | "IFCPLATESTANDARDCASE"
                | "IFCSPACE"
                | "IFCFOOTING"
        )
}

/// Quantities computed from an element's triangles (mesh units → SI by `metres`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Computed {
    pub length: Option<f64>,
    pub area: Option<f64>,
    pub volume: Option<f64>,
}

/// Computes what can be from one element's triangles.
pub fn compute(tris: &[[[f64; 3]; 3]], class: &str, closed: bool, metres: f64) -> Computed {
    let mut c = Computed::default();
    if tris.is_empty() {
        return c;
    }
    let m2 = metres * metres;
    if closed {
        let v: f64 = tris
            .iter()
            .map(|t| {
                let [a, b, d] = *t;
                (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0])
                    + a[2] * (b[0] * d[1] - b[1] * d[0]))
                    / 6.0
            })
            .sum();
        c.volume = Some(v.abs() * m2 * metres);
    }
    let area_n = |t: &[[f64; 3]; 3]| {
        let u: [f64; 3] = std::array::from_fn(|k| t[1][k] - t[0][k]);
        let w: [f64; 3] = std::array::from_fn(|k| t[2][k] - t[0][k]);
        let n = [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        (l / 2.0, if l > 0.0 { n.map(|x| x / l) } else { n })
    };
    if is_wall(class) {
        // One side: the vertical faces facing the commonest horizontal direction.
        let mut bins: BTreeMap<i32, f64> = BTreeMap::new();
        let mut vertical = 0.0;
        for t in tris {
            let (a, n) = area_n(t);
            if n[2].abs() > 0.2 || a <= 0.0 {
                continue;
            }
            vertical += a;
            let ang = n[1].atan2(n[0]).to_degrees();
            let bin = ((ang + 180.0) / 5.0).round() as i32 % 72;
            *bins.entry(bin).or_insert(0.0) += a;
        }
        if let Some((&b, &best)) = bins.iter().max_by(|x, y| x.1.total_cmp(y.1)) {
            let opposite = bins.get(&((b + 36) % 72)).copied().unwrap_or(0.0);
            if vertical > 0.0 && (best + opposite) / vertical > 0.8 {
                c.area = Some(best.max(opposite) * m2);
            } else if let Some(side) = curved_side(tris, vertical) {
                c.area = Some(side * m2);
            }
        }
    } else if up_area_class(class) {
        let up: f64 = tris
            .iter()
            .map(area_n)
            .filter(|(_, n)| n[2] > 0.7)
            .map(|(a, _)| a)
            .sum();
        if up > 0.0 {
            c.area = Some(up * m2);
        }
    }
    if linear_class(class) {
        // A wall's length runs along the floor.
        c.length = principal_extent(tris, is_wall(class)).map(|l| l * metres);
    }
    c
}

/// One side of a wall curved in plan (an arc swept up): the area of its curved surface,
/// not of the chord. The vertical faces' normals all point through the arc's centre, found
/// as the point nearest (least squares, by area) to the lines through each face along its
/// normal. Faces whose normal points away from it are the outer side, towards it the inner;
/// the larger is the side (as for a straight wall). `None` unless the two sides hold most
/// of the vertical area and the centre is well defined.
fn curved_side(tris: &[[[f64; 3]; 3]], vertical: f64) -> Option<f64> {
    let mut faces = Vec::new();
    // Σ a (I − n nᵀ) and Σ a (I − n nᵀ) p, in plan.
    let (mut m, mut r) = ([0.0f64; 3], [0.0f64; 2]);
    for t in tris {
        let u: [f64; 3] = std::array::from_fn(|k| t[1][k] - t[0][k]);
        let w: [f64; 3] = std::array::from_fn(|k| t[2][k] - t[0][k]);
        let n = [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l <= 0.0 || (n[2] / l).abs() > 0.2 {
            continue;
        }
        let h = (n[0] * n[0] + n[1] * n[1]).sqrt();
        let (nx, ny) = (n[0] / h, n[1] / h);
        let a = l / 2.0;
        let p = [
            (t[0][0] + t[1][0] + t[2][0]) / 3.0,
            (t[0][1] + t[1][1] + t[2][1]) / 3.0,
        ];
        let (pxx, pxy, pyy) = (1.0 - nx * nx, -nx * ny, 1.0 - ny * ny);
        m[0] += a * pxx;
        m[1] += a * pxy;
        m[2] += a * pyy;
        r[0] += a * (pxx * p[0] + pxy * p[1]);
        r[1] += a * (pxy * p[0] + pyy * p[1]);
        faces.push((a, p, [nx, ny]));
    }
    let det = m[0] * m[2] - m[1] * m[1];
    // Normals spread over a real angle (a straight wall leaves the system singular).
    if det <= 1e-6 * (m[0] + m[2]).powi(2) {
        return None;
    }
    let c = [
        (m[2] * r[0] - m[1] * r[1]) / det,
        (m[0] * r[1] - m[1] * r[0]) / det,
    ];
    let (mut outer, mut inner) = (0.0, 0.0);
    for (a, p, n) in faces {
        let d = [p[0] - c[0], p[1] - c[1]];
        let dl = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if dl <= 0.0 {
            continue;
        }
        let cos = (d[0] * n[0] + d[1] * n[1]) / dl;
        if cos > 0.98 {
            outer += a;
        } else if cos < -0.98 {
            inner += a;
        }
    }
    ((outer + inner) / vertical > 0.8).then(|| outer.max(inner))
}

/// The extent of the triangles' corners along their principal axis (in plan when `flat`).
fn principal_extent(tris: &[[[f64; 3]; 3]], flat: bool) -> Option<f64> {
    let mut pts: Vec<[f64; 3]> = tris
        .iter()
        .flatten()
        .map(|p| if flat { [p[0], p[1], 0.0] } else { *p })
        .collect();
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    pts.dedup();
    let n = pts.len() as f64;
    if n < 2.0 {
        return None;
    }
    let mean: [f64; 3] = std::array::from_fn(|k| pts.iter().map(|p| p[k]).sum::<f64>() / n);
    let mut cov = [[0.0; 3]; 3];
    for p in &pts {
        let d: [f64; 3] = std::array::from_fn(|k| p[k] - mean[k]);
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    let mut v = [1.0, 0.7, 0.3];
    for _ in 0..64 {
        let w: [f64; 3] = std::array::from_fn(|i| (0..3).map(|j| cov[i][j] * v[j]).sum());
        let l = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
        if l <= 0.0 {
            return None;
        }
        v = w.map(|x| x / l);
    }
    let proj = pts
        .iter()
        .map(|p| (0..3).map(|k| (p[k] - mean[k]) * v[k]).sum::<f64>());
    let (lo, hi) = proj.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
        (a.min(x), b.max(x))
    });
    (hi > lo).then_some(hi - lo)
}

/// The file's quantity of the first name found, in SI.
fn from_file(d: &IfcData, o: &specular_mesh::ifc::data::Object, names: &[&str]) -> Option<f64> {
    let sets: Vec<_> = d
        .psets(o)
        .into_iter()
        .filter(|s| s.kind == SetKind::Quantities)
        .collect();
    for n in names {
        for s in &sets {
            if let Some(p) = s.get(n)
                && p.kind == PropKind::Quantity
                && let Some(v) = p.si_values().first().and_then(|v| v.as_f64())
            {
                return Some(v);
            }
        }
    }
    None
}

/// Every drawn element's quantities. `metres` is the mesh's metres per unit.
pub fn rows(d: &IfcData, elements: &[Element], mesh: &Mesh, metres: f64) -> Vec<Row> {
    let mut tris: Vec<Vec<[[f64; 3]; 3]>> = vec![Vec::new(); elements.len()];
    let p = |i: u32| mesh.positions[i as usize].map(f64::from);
    for t in mesh.indices.chunks_exact(3) {
        if t.iter().any(|&i| i as usize >= mesh.positions.len()) {
            continue;
        }
        if let Some(e) = crate::apply::element_of(elements, t[0]) {
            tris[e].push([p(t[0]), p(t[1]), p(t[2])]);
        }
    }
    let mut out = Vec::with_capacity(elements.len());
    for (i, e) in elements.iter().enumerate() {
        let o = d.by_guid(&e.global_id);
        let c = compute(&tris[i], &e.kind, e.volume.is_some(), metres);
        let pick = |names: &[&str], computed: Option<f64>| -> Option<Q> {
            o.and_then(|o| from_file(d, o, names))
                .map(|value| Q {
                    value,
                    from_file: true,
                })
                .or(computed.map(|value| Q {
                    value,
                    from_file: false,
                }))
        };
        out.push(Row {
            element: i,
            guid: e.global_id.clone(),
            name: e.name.clone(),
            class: e.kind.clone(),
            type_name: o
                .and_then(|o| d.type_of(o))
                .and_then(|t| t.name())
                .unwrap_or_default(),
            storey: o
                .and_then(|o| d.storey(o))
                .and_then(|s| s.name())
                .unwrap_or_default(),
            length: pick(LENGTH, c.length),
            area: pick(AREA, c.area),
            volume: pick(VOLUME, c.volume),
        });
    }
    out
}

/// Sums by `by`, sorted by key.
pub fn groups(rows: &[Row], by: By) -> Vec<Group> {
    let mut g: BTreeMap<String, Group> = BTreeMap::new();
    for r in rows {
        let key = match by {
            By::Class => r.class.clone(),
            By::Storey => r.storey.clone(),
            By::Type => r.type_name.clone(),
        };
        let e = g.entry(key.clone()).or_insert_with(|| Group {
            key,
            count: 0,
            length: 0.0,
            area: 0.0,
            volume: 0.0,
            from_file: 0,
            computed: 0,
        });
        e.count += 1;
        for (q, sum) in [
            (r.length, &mut e.length),
            (r.area, &mut e.area),
            (r.volume, &mut e.volume),
        ] {
            if let Some(q) = q {
                *sum += q.value;
                if q.from_file {
                    e.from_file += 1;
                } else {
                    e.computed += 1;
                }
            }
        }
    }
    g.into_values().collect()
}

/// Column names and the words for where values came from, in the user's language.
#[derive(Clone, Debug)]
pub struct Labels {
    /// GlobalId, Name, Class, Type, Storey, Count, Length (m), Area (m²), Volume (m³),
    /// Source; for groups the first column's name is given per grouping.
    pub columns: [String; 10],
    pub from_file: String,
    pub computed: String,
    pub mixed: String,
}

fn src(q: &[Option<Q>], l: &Labels) -> String {
    let f = q.iter().flatten().filter(|q| q.from_file).count();
    let c = q.iter().flatten().filter(|q| !q.from_file).count();
    match (f, c) {
        (0, 0) => String::new(),
        (_, 0) => l.from_file.clone(),
        (0, _) => l.computed.clone(),
        _ => l.mixed.clone(),
    }
}

/// The element table: a header, then one row per element.
pub fn element_table(
    rows: &[Row],
    l: &Labels,
    class_name: &dyn Fn(&str) -> String,
) -> Vec<Vec<Cell>> {
    let mut t = vec![l.columns.iter().map(|c| Cell::from(c.as_str())).collect()];
    for r in rows {
        t.push(vec![
            Cell::from(r.guid.as_str()),
            Cell::from(r.name.as_str()),
            Cell::from(class_name(&r.class)),
            Cell::from(r.type_name.as_str()),
            Cell::from(r.storey.as_str()),
            Cell::Number(1.0),
            Cell::from(r.length.map(|q| q.value)),
            Cell::from(r.area.map(|q| q.value)),
            Cell::from(r.volume.map(|q| q.value)),
            Cell::from(src(&[r.length, r.area, r.volume], l)),
        ]);
    }
    t
}

/// A grouped table: `first` names the key column.
pub fn group_table(
    groups: &[Group],
    first: &str,
    l: &Labels,
    key_name: &dyn Fn(&str) -> String,
) -> Vec<Vec<Cell>> {
    let mut head: Vec<Cell> = vec![Cell::from(first)];
    head.extend(l.columns[5..].iter().map(|c| Cell::from(c.as_str())));
    let mut t = vec![head];
    for g in groups {
        let source = match (g.from_file, g.computed) {
            (0, 0) => String::new(),
            (_, 0) => l.from_file.clone(),
            (0, _) => l.computed.clone(),
            _ => l.mixed.clone(),
        };
        let some = |v: f64| {
            if v > 0.0 {
                Cell::Number(v)
            } else {
                Cell::Empty
            }
        };
        t.push(vec![
            Cell::from(key_name(&g.key)),
            Cell::Number(g.count as f64),
            some(g.length),
            some(g.area),
            some(g.volume),
            Cell::from(source),
        ]);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(lo: [f64; 3], hi: [f64; 3]) -> Vec<[[f64; 3]; 3]> {
        let c = |k: usize| -> [f64; 3] {
            [
                if k & 1 == 0 { lo[0] } else { hi[0] },
                if k & 2 == 0 { lo[1] } else { hi[1] },
                if k & 4 == 0 { lo[2] } else { hi[2] },
            ]
        };
        let faces = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let mut t = Vec::new();
        for f in faces {
            t.push([c(f[0]), c(f[1]), c(f[2])]);
            t.push([c(f[0]), c(f[2]), c(f[3])]);
        }
        t
    }

    /// A wall curved in plan: a quarter of a ring (inner radius `r`, `thick`, `h` high, `n`
    /// segments), closed.
    fn arc_wall(r: f64, thick: f64, h: f64, n: usize) -> Vec<[[f64; 3]; 3]> {
        let ang = |i: usize| std::f64::consts::FRAC_PI_2 * i as f64 / n as f64;
        let pt =
            |rad: f64, i: usize, z: f64| [10.0 + rad * ang(i).cos(), -3.0 + rad * ang(i).sin(), z];
        let ro = r + thick;
        let mut t = Vec::new();
        let mut quad = |a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]| {
            t.push([a, b, c]);
            t.push([a, c, d]);
        };
        for i in 0..n {
            // Outer (facing out), inner (facing in), top, bottom.
            quad(
                pt(ro, i, 0.0),
                pt(ro, i + 1, 0.0),
                pt(ro, i + 1, h),
                pt(ro, i, h),
            );
            quad(
                pt(r, i + 1, 0.0),
                pt(r, i, 0.0),
                pt(r, i, h),
                pt(r, i + 1, h),
            );
            quad(pt(r, i, h), pt(ro, i, h), pt(ro, i + 1, h), pt(r, i + 1, h));
            quad(
                pt(r, i + 1, 0.0),
                pt(ro, i + 1, 0.0),
                pt(ro, i, 0.0),
                pt(r, i, 0.0),
            );
        }
        // The two ends.
        quad(pt(r, 0, 0.0), pt(ro, 0, 0.0), pt(ro, 0, h), pt(r, 0, h));
        quad(pt(ro, n, 0.0), pt(r, n, 0.0), pt(r, n, h), pt(ro, n, h));
        t
    }

    #[test]
    fn a_curved_wall_has_the_area_of_its_curved_side() {
        let (r, thick, h, n) = (5.0, 0.3, 3.0, 48);
        let w = compute(&arc_wall(r, thick, h, n), "IFCWALLSTANDARDCASE", true, 1.0);
        // The outer side as faceted: n chords of the outer arc, times the height (within
        // 0.03 % of the true arc).
        let chord = 2.0 * (r + thick) * (std::f64::consts::FRAC_PI_4 / n as f64).sin();
        let faceted = n as f64 * chord * h;
        let arc = std::f64::consts::FRAC_PI_2 * (r + thick) * h;
        let a = w.area.expect("a side area");
        assert!((a - faceted).abs() < 1e-6, "{a} vs {faceted}");
        assert!((a - arc).abs() / arc < 5e-4, "{a} vs {arc}");
        // Not the straight chord between its ends.
        let straight = std::f64::consts::SQRT_2 * (r + thick) * h;
        assert!(a > straight * 1.1);
    }

    #[test]
    fn computes_wall_slab_and_beam_quantities() {
        // A wall 4 m long, 0.2 m thick, 3 m high, in millimetres.
        let w = compute(
            &boxed([0.0; 3], [4000.0, 200.0, 3000.0]),
            "IFCWALL",
            true,
            0.001,
        );
        assert!((w.volume.unwrap() - 2.4).abs() < 1e-9);
        assert!((w.area.unwrap() - 12.0).abs() < 1e-9, "{:?}", w.area);
        assert!((w.length.unwrap() - 4.0).abs() < 1e-6, "{:?}", w.length);
        let s = compute(&boxed([0.0; 3], [5.0, 4.0, 0.25]), "IFCSLAB", true, 1.0);
        assert!((s.area.unwrap() - 20.0).abs() < 1e-9);
        assert!(s.length.is_none());
        // A diagonal beam: its length along the axis, not the box.
        let mut b = boxed([0.0, -0.1, -0.1], [6.0, 0.1, 0.1]);
        let (c, sn) = (0.6f64, 0.8f64);
        for t in &mut b {
            for p in t.iter_mut() {
                *p = [p[0] * c - p[1] * sn, p[0] * sn + p[1] * c, p[2]];
            }
        }
        let bm = compute(&b, "IFCBEAM", true, 1.0);
        assert!((bm.length.unwrap() - 6.0).abs() < 1e-3, "{:?}", bm.length);
        // Open shells have no volume.
        assert!(compute(&b, "IFCBEAM", false, 1.0).volume.is_none());
    }

    #[test]
    fn groups_sum_and_say_where_from() {
        let row = |class: &str, a: f64, f: bool| Row {
            element: 0,
            guid: String::new(),
            name: String::new(),
            class: class.into(),
            type_name: "T".into(),
            storey: "L1".into(),
            length: None,
            area: Some(Q {
                value: a,
                from_file: f,
            }),
            volume: None,
        };
        let rows = [
            row("IFCWALL", 2.0, true),
            row("IFCWALL", 3.0, false),
            row("IFCSLAB", 5.0, true),
        ];
        let g = groups(&rows, By::Class);
        assert_eq!(g.len(), 2);
        assert_eq!(
            (g[1].key.as_str(), g[1].count, g[1].area),
            ("IFCWALL", 2, 5.0)
        );
        assert_eq!((g[1].from_file, g[1].computed), (1, 1));
        let l = Labels {
            columns: std::array::from_fn(|i| format!("c{i}")),
            from_file: "file".into(),
            computed: "computed".into(),
            mixed: "mixed".into(),
        };
        let t = group_table(&g, "Class", &l, &|k| k.to_owned());
        assert_eq!(t[2][5], Cell::from("mixed"));
        assert_eq!(t[1][5], Cell::from("file"));
        assert_eq!(groups(&rows, By::Storey).len(), 1);
    }
}
