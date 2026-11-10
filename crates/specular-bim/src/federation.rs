//! Several IFC models in one scene (a federation): how each one is placed against the first
//! (by their map conversions, else their sites' reference points, else their shared project
//! coordinates), one mesh of them all for drawing and clash tests, selection sets qualified
//! by model, and the BCF header entries that name each model file.
//!
//! - Map conversions: both models name the same CRS (or one names none). A point goes from
//!   the model's project to the map and back into the first model's project
//!   ([`Georef::to_map`], [`Georef::from_map`]); the result is exact up to rounding.
//! - Site reference points: neither has a map conversion but both sites give a latitude and
//!   longitude. The offset between them is taken on a local tangent plane (a sphere of the
//!   Earth's mean radius), the projects turned by their true north. Good to centimetres
//!   over a few hundred metres; a note says it is approximate.
//! - Shared project coordinates: neither model says where it is: both are assumed to use
//!   the same project origin, as discipline models exported from one project do.
//! - Otherwise (one has a map conversion and the other not, or different CRSs) the model
//!   cannot be aligned, and the user may place it at the origin (its project coordinates as
//!   they are).

use std::ops::Range;

use serde::{Deserialize, Serialize};
use specular_mesh::ifc::Element;
use specular_mesh::ifc::georef::Georef;
use specular_mesh::{Bounds, Mesh, Part};

use crate::bcf::HeaderFile;
use crate::clash::Selector;

/// Mean Earth radius (m), for the site reference fallback.
const EARTH_R: f64 = 6_371_008.8;

/// An affine map of project metres (rows `[x y z t]`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement(pub [[f64; 4]; 3]);

impl Default for Placement {
    fn default() -> Placement {
        Placement::IDENTITY
    }
}

impl Placement {
    pub const IDENTITY: Placement = Placement([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    pub fn point(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
    }

    pub fn vector(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2])
    }

    pub fn is_identity(&self) -> bool {
        self.0
            .iter()
            .flatten()
            .zip(Placement::IDENTITY.0.iter().flatten())
            .all(|(a, b)| (a - b).abs() < 1e-12)
    }

    /// The map that sends `f`'s images of the origin and the three unit axes where `f`
    /// sends them (`f` affine).
    fn sampled(f: impl Fn([f64; 3]) -> Option<[f64; 3]>) -> Option<Placement> {
        // 100 m axes keep rounding small against large map coordinates.
        const L: f64 = 100.0;
        let o = f([0.0; 3])?;
        let ax = |k: usize| -> Option<[f64; 3]> {
            let mut p = [0.0; 3];
            p[k] = L;
            let q = f(p)?;
            Some(std::array::from_fn(|r| (q[r] - o[r]) / L))
        };
        let (x, y, z) = (ax(0)?, ax(1)?, ax(2)?);
        let m = Placement(std::array::from_fn(|r| [x[r], y[r], z[r], o[r]]));
        m.0.iter().flatten().all(|v| v.is_finite()).then_some(m)
    }

    /// The rotation about z of the x axis (degrees).
    pub fn angle_deg(&self) -> f64 {
        self.0[1][0].atan2(self.0[0][0]).to_degrees()
    }

    pub fn offset(&self) -> [f64; 3] {
        [self.0[0][3], self.0[1][3], self.0[2][3]]
    }
}

/// How a model was placed against the first.
#[derive(Clone, Debug, PartialEq)]
pub enum Basis {
    /// The first model itself.
    Reference,
    /// By both map conversions (the CRS named, when one is).
    Map { crs: Option<String> },
    /// By the sites' reference latitude and longitude (approximate).
    Site,
    /// Neither says where it is: the same project coordinates.
    Project,
    /// Placed at the origin by the user, after it could not be aligned.
    Origin,
}

/// Why a model cannot be aligned with the first.
#[derive(Clone, Debug, PartialEq)]
pub enum Unaligned {
    /// The two name different coordinate systems.
    DifferentCrs {
        first: String,
        this: String,
    },
    /// Only one of them is georeferenced (map conversion).
    OnlyFirst,
    OnlyThis,
    /// A map conversion that cannot be inverted (zero scale).
    Broken,
}

/// How `this` model's project (metres) maps into the `first` model's project (metres).
pub fn align(first: &Georef, this: &Georef) -> Result<(Placement, Basis), Unaligned> {
    match (&first.map, &this.map) {
        (Some(_), Some(_)) => {
            if let (Some(a), Some(b)) = (first.crs_name(), this.crs_name())
                && !a.eq_ignore_ascii_case(b)
            {
                return Err(Unaligned::DifferentCrs {
                    first: a.to_owned(),
                    this: b.to_owned(),
                });
            }
            let p =
                Placement::sampled(|p| first.from_map(this.to_map(p)?)).ok_or(Unaligned::Broken)?;
            let crs = first.crs_name().or(this.crs_name()).map(str::to_owned);
            Ok((p, Basis::Map { crs }))
        }
        (Some(_), None) => Err(Unaligned::OnlyFirst),
        (None, Some(_)) => Err(Unaligned::OnlyThis),
        (None, None) => match (&first.site, &this.site) {
            (Some(a), Some(b)) => {
                // Each project's east and north (unit vectors in its XY plane).
                let axes = |g: &Georef| {
                    let n = g.true_north.unwrap_or([0.0, 1.0]);
                    ([n[1], -n[0]], n)
                };
                let (ea, na) = axes(first);
                let (eb, nb) = axes(this);
                let lat0 = a.latitude.to_radians();
                let de = (b.longitude - a.longitude).to_radians() * lat0.cos() * EARTH_R;
                let dn = (b.latitude - a.latitude).to_radians() * EARTH_R;
                let dz = b.elevation.unwrap_or(0.0) - a.elevation.unwrap_or(0.0);
                let f = |p: [f64; 3]| -> Option<[f64; 3]> {
                    let v = [p[0] - b.origin[0], p[1] - b.origin[1]];
                    // This project to east/north, shifted by the sites' offset.
                    let e = v[0] * eb[0] + v[1] * eb[1] + de;
                    let n = v[0] * nb[0] + v[1] * nb[1] + dn;
                    // East/north into the first project.
                    let x = e * ea[0] + n * na[0];
                    let y = e * ea[1] + n * na[1];
                    Some([
                        x + a.origin[0],
                        y + a.origin[1],
                        p[2] - b.origin[2] + dz + a.origin[2],
                    ])
                };
                Ok((Placement::sampled(f).ok_or(Unaligned::Broken)?, Basis::Site))
            }
            _ => Ok((Placement::IDENTITY, Basis::Project)),
        },
    }
}

/// One model of a federation, in its own frame: a mesh point `p` is at
/// `p * mesh_metres + origin_metres` in its project, and `place` takes that into the scene's
/// project (the first model's).
pub struct Member<'a> {
    pub mesh: &'a Mesh,
    /// Sorted by first vertex.
    pub elements: &'a [Element],
    pub mesh_metres: f64,
    pub origin_metres: [f64; 3],
    pub place: Placement,
}

/// The scene: every member's triangles in one mesh, in the frame of the first member's mesh.
#[derive(Debug, Default)]
pub struct Merged {
    pub mesh: Mesh,
    /// Every member's elements in order, first vertices in the merged mesh.
    pub elements: Vec<Element>,
    /// Each member's elements in [`Merged::elements`].
    pub ranges: Vec<Range<usize>>,
}

impl Merged {
    /// The member an element (index into [`Merged::elements`]) is from.
    pub fn member_of(&self, element: usize) -> Option<usize> {
        self.ranges.iter().position(|r| r.contains(&element))
    }
}

/// A mesh point of one frame in another, through `place`.
fn to_frame(
    p: [f32; 3],
    from: (f64, [f64; 3]),
    place: &Placement,
    to: (f64, [f64; 3]),
) -> [f32; 3] {
    let w: [f64; 3] = std::array::from_fn(|k| f64::from(p[k]) * from.0 + from.1[k]);
    let q = place.point(w);
    std::array::from_fn(|k| ((q[k] - to.1[k]) / to.0.max(1e-12)) as f32)
}

/// Appends `src` to `out`, its positions through `place` from frame `from` into `to`.
fn append(
    out: &mut Mesh,
    src: &Mesh,
    from: (f64, [f64; 3]),
    place: &Placement,
    to: (f64, [f64; 3]),
) {
    let same = place.is_identity() && from == to;
    let base = out.positions.len() as u32;
    let tri_base = (out.indices.len() / 3) as u32;
    let mat_base = out.materials.len() as u32;
    if same {
        out.positions.extend_from_slice(&src.positions);
        out.normals.extend_from_slice(&src.normals);
    } else {
        out.positions
            .extend(src.positions.iter().map(|p| to_frame(*p, from, place, to)));
        out.normals.extend(src.normals.iter().map(|n| {
            let v = place.vector(n.map(f64::from));
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
            v.map(|c| (c / l) as f32)
        }));
    }
    // Vertex colours: kept when every part has them, else white for those without.
    match (&mut out.colors, &src.colors) {
        (Some(c), Some(s)) => c.extend_from_slice(s),
        (Some(c), None) => c.extend(std::iter::repeat_n([255; 4], src.positions.len())),
        (None, Some(s)) => {
            let mut c = vec![[255; 4]; base as usize];
            c.extend_from_slice(s);
            out.colors = Some(c);
        }
        (None, None) => {}
    }
    out.indices.extend(src.indices.iter().map(|i| i + base));
    out.materials.extend(src.materials.iter().cloned());
    out.parts.extend(src.parts.iter().map(|p| Part {
        first: p.first + tri_base,
        count: p.count,
        material: p.material.map(|m| m + mat_base),
    }));
    out.notes.extend(src.notes.iter().cloned());
}

fn bounds(mesh: &Mesh) -> Bounds {
    let mut b = Bounds::EMPTY;
    for &i in &mesh.indices {
        if let Some(p) = mesh.positions.get(i as usize) {
            b.add(*p);
        }
    }
    b
}

/// One mesh of every member, in the first member's mesh frame. Members are kept whole and
/// in order; textures are not carried (IFC models have none).
pub fn merge(members: &[Member]) -> Merged {
    let Some(first) = members.first() else {
        return Merged::default();
    };
    let to = (first.mesh_metres, first.origin_metres);
    let mut mesh = Mesh {
        format: first.mesh.format,
        unit: first.mesh.unit,
        up: first.mesh.up,
        flat: first.mesh.flat,
        title: first.mesh.title.clone(),
        generator: first.mesh.generator.clone(),
        ..Mesh::default()
    };
    let mut elements = Vec::new();
    let mut ranges = Vec::new();
    for m in members {
        let base = mesh.positions.len() as u32;
        append(
            &mut mesh,
            m.mesh,
            (m.mesh_metres, m.origin_metres),
            &m.place,
            to,
        );
        let start = elements.len();
        elements.extend(m.elements.iter().map(|e| Element {
            first_vertex: e.first_vertex + base,
            ..e.clone()
        }));
        ranges.push(start..elements.len());
    }
    mesh.bounds = bounds(&mesh);
    Merged {
        mesh,
        elements,
        ranges,
    }
}

/// `mesh` (in frame `from`) moved through `place` into frame `to`: for a member's other
/// version, compared in its own frame and drawn in the scene's.
pub fn place_mesh(
    mesh: &Mesh,
    from: (f64, [f64; 3]),
    place: &Placement,
    to: (f64, [f64; 3]),
) -> Mesh {
    let mut out = Mesh {
        format: mesh.format,
        unit: mesh.unit,
        up: mesh.up,
        flat: mesh.flat,
        images: mesh.images.clone(),
        uvs: mesh.uvs.clone(),
        title: mesh.title.clone(),
        generator: mesh.generator.clone(),
        file_normals: mesh.file_normals,
        ..Mesh::default()
    };
    append(&mut out, mesh, from, place, to);
    out.bounds = bounds(&out);
    out
}

/// A clash set side, in a federation: a selection within one model, or within all.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Scoped {
    /// Index of the member; None: every model.
    pub model: Option<usize>,
    pub set: Selector,
}

impl Scoped {
    /// The elements chosen (indices into the merged elements); `member` gives an element's
    /// model and `storey` its storey's name.
    pub fn pick(
        &self,
        elements: &[Element],
        member: &dyn Fn(usize) -> Option<usize>,
        storey: &dyn Fn(usize) -> Option<String>,
    ) -> Vec<usize> {
        self.set
            .pick(elements, storey)
            .into_iter()
            .filter(|&i| self.model.is_none() || member(i) == self.model)
            .collect()
    }
}

/// A clash's sidecar key for an element of a federation: the model file and the GUID, so
/// the same GUID in two models stays two keys.
pub fn qualified(file: &str, guid: &str) -> String {
    format!("{file}#{guid}")
}

/// A BCF header entry naming a model file: its name, the IFC project's GUID and the file's
/// date (the STEP header's time stamp), as BCF 2.1 and 3.0 `Header/File(s)` hold them.
pub fn header_file(file: &str, project_guid: Option<&str>, date: Option<&str>) -> HeaderFile {
    HeaderFile {
        ifc_project: project_guid.map(str::to_owned).filter(|g| !g.is_empty()),
        ifc_spatial: None,
        is_external: true,
        filename: Some(file.to_owned()),
        date: date.map(str::to_owned).filter(|d| !d.is_empty()),
        reference: Some(file.to_owned()),
    }
}

/// Memory a member holds while it is in the scene: its own mesh, its share of the merged
/// mesh and of the GPU's copy (bytes).
pub fn member_bytes(mesh: &Mesh) -> u64 {
    (mesh.memory_bytes() as u64).saturating_mul(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_mesh::ifc::georef::{Crs, MapConversion, SiteRef};

    fn mapped(e: f64, n: f64, deg: f64, crs: &str) -> Georef {
        let (s, c) = deg.to_radians().sin_cos();
        Georef {
            map: Some(MapConversion {
                eastings: e,
                northings: n,
                height: 2.0,
                abscissa: c,
                ordinate: s,
                scale: 0.001,
                factors: [1.0; 3],
            }),
            crs: Some(Crs {
                name: crs.into(),
                map_unit_metres: Some(1.0),
                ..Crs::default()
            }),
            unit_metres: 0.001,
            ..Georef::default()
        }
    }

    fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }

    #[test]
    fn map_conversions_compose() {
        let a = mapped(
            729_013.348_829_700_4,
            9_063_992.684_697_364,
            60.0,
            "EPSG:32760",
        );
        let b = mapped(
            729_011.225_882_358_4,
            9_063_960.607_644_705,
            0.0,
            "EPSG:32760",
        );
        let (p, basis) = align(&a, &b).unwrap();
        assert!(matches!(basis, Basis::Map { crs: Some(ref c) } if c == "EPSG:32760"));
        // A point of b on the map, and back from a's project: the same place.
        for q in [[0.0, 0.0, 0.0], [12.5, -40.0, 3.0], [-300.0, 250.0, -7.0]] {
            let on_map = b.to_map(q).unwrap();
            let in_a = p.point(q);
            assert!(close(a.to_map(in_a).unwrap(), on_map, 1e-6), "{q:?}");
        }
        assert!((p.angle_deg() + 60.0).abs() < 1e-9);
        // IfcOpenShell's figure for these two (tools/corpus/ifc-federation-oracle.py).
        assert!(close(p.offset(), [-28.841_016_151_7, -14.2, 0.0], 1e-6));
        // From the map and back.
        let q = [5.0, 6.0, 7.0];
        assert!(close(a.from_map(a.to_map(q).unwrap()).unwrap(), q, 1e-6));
    }

    #[test]
    fn what_cannot_be_aligned_says_why() {
        let a = mapped(0.0, 0.0, 0.0, "EPSG:32760");
        let b = mapped(0.0, 0.0, 0.0, "EPSG:25832");
        assert!(matches!(align(&a, &b), Err(Unaligned::DifferentCrs { .. })));
        let none = Georef {
            unit_metres: 1.0,
            ..Georef::default()
        };
        assert_eq!(align(&a, &none), Err(Unaligned::OnlyFirst));
        assert_eq!(align(&none, &a), Err(Unaligned::OnlyThis));
        let (p, basis) = align(&none, &none).unwrap();
        assert_eq!(basis, Basis::Project);
        assert!(p.is_identity());
    }

    #[test]
    fn sites_place_by_latitude_and_longitude() {
        let site = |lat: f64, lon: f64, north: Option<[f64; 2]>| Georef {
            site: Some(SiteRef {
                latitude: lat,
                longitude: lon,
                elevation: Some(10.0),
                origin: [0.0; 3],
            }),
            true_north: north,
            unit_metres: 1.0,
            ..Georef::default()
        };
        let a = site(52.0, 13.0, None);
        // 0.001° north: 111.2 m.
        let b = site(52.001, 13.0, None);
        let (p, basis) = align(&a, &b).unwrap();
        assert_eq!(basis, Basis::Site);
        assert!(close(p.offset(), [0.0, 111.195, 0.0], 0.01));
        // A project turned so that north is its +x: its +x is the first's +y.
        let c = site(52.0, 13.0, Some([1.0, 0.0]));
        let (p, _) = align(&a, &c).unwrap();
        assert!(close(p.point([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0], 1e-9));
    }

    fn element(first: u32, n: u32, guid: &str) -> Element {
        Element {
            global_id: guid.into(),
            first_vertex: first,
            vertices: n,
            ..Element::default()
        }
    }

    #[test]
    fn merging_keeps_members_whole_and_placed() {
        let tri = |dx: f32| Mesh {
            positions: vec![[dx, 0.0, 0.0], [dx + 1.0, 0.0, 0.0], [dx, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
            parts: vec![Part {
                first: 0,
                count: 1,
                material: Some(0),
            }],
            materials: vec![specular_mesh::Material::default()],
            ..Mesh::default()
        };
        let (m1, m2) = (tri(0.0), tri(0.0));
        let e1 = [element(0, 3, "A")];
        let e2 = [element(0, 3, "B")];
        // The second model: in millimetres, turned a quarter about z, 10 m east.
        let place = Placement([
            [0.0, -1.0, 0.0, 10.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]);
        let merged = merge(&[
            Member {
                mesh: &m1,
                elements: &e1,
                mesh_metres: 1.0,
                origin_metres: [0.0; 3],
                place: Placement::IDENTITY,
            },
            Member {
                mesh: &m2,
                elements: &e2,
                mesh_metres: 0.001,
                origin_metres: [0.0; 3],
                place,
            },
        ]);
        assert_eq!(merged.mesh.positions.len(), 6);
        assert_eq!(merged.mesh.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(merged.ranges, vec![0..1, 1..2]);
        assert_eq!(merged.elements[1].first_vertex, 3);
        assert_eq!(merged.member_of(1), Some(1));
        assert_eq!(merged.mesh.parts[1].first, 1);
        assert_eq!(merged.mesh.parts[1].material, Some(1));
        let p = merged.mesh.positions[4];
        assert!(
            (p[0] - 10.0).abs() < 1e-6 && (p[1] - 0.001).abs() < 1e-6,
            "{p:?}"
        );
        assert!(close(
            merged.mesh.normals[4].map(f64::from),
            [0.0, 0.0, 1.0],
            1e-6
        ));
    }

    #[test]
    fn scoped_sets_pick_within_a_model() {
        let els = [element(0, 3, "A"), element(3, 3, "B"), element(6, 3, "C")];
        let member = |i: usize| Some(usize::from(i > 0));
        let s = Scoped {
            model: Some(1),
            set: Selector::All,
        };
        assert_eq!(s.pick(&els, &member, &|_| None), vec![1, 2]);
        let all = Scoped::default();
        assert_eq!(all.pick(&els, &member, &|_| None), vec![0, 1, 2]);
        assert_eq!(qualified("a.ifc", "X"), "a.ifc#X");
    }
}
