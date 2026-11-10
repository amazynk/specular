//! Two versions of an IFC model compared object by object (by GlobalId): what was added,
//! removed and changed. A change is in the geometry (the element's vertices, in the
//! project's frame to the millimetre), its attributes, type, container, material,
//! classification or a property or quantity value. The newer model drawn with the changes
//! in colour: added green, changed amber, removed (taken from the older model) red, the
//! rest ghosted.

use std::collections::{BTreeMap, HashMap};

use specular_mesh::ifc::Element;
use specular_mesh::ifc::data::{IfcData, Object, Value};
use specular_mesh::{Material, Mesh, Part};

use crate::apply::{self, ElementLook};

/// One version of the model.
pub struct Side<'a> {
    pub data: &'a IfcData,
    /// The elements drawn, by first vertex.
    pub elements: &'a [Element],
    pub mesh: &'a Mesh,
    /// A mesh point `p` is at `p * mesh_metres + origin_metres` in the project.
    pub mesh_metres: f64,
    pub origin_metres: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

/// What a field of a changed object is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    Attribute,
    Type,
    Container,
    Material,
    Classification,
    /// A property or quantity (the set is named in [`FieldChange::set`]).
    Property,
}

/// One field that differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
    pub field: Field,
    pub set: Option<String>,
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// One object added, removed or changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub guid: String,
    pub class: String,
    pub name: Option<String>,
    pub kind: ChangeKind,
    /// Its geometry differs.
    pub geometry: bool,
    /// How far its box's centre moved (metres), when it did.
    pub moved_m: Option<f64>,
    pub fields: Vec<FieldChange>,
    /// Its element in the older and the newer model, when drawn.
    pub old_element: Option<usize>,
    pub new_element: Option<usize>,
}

/// A geometry fingerprint: order-free over the vertices, to the millimetre in the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shape {
    sum: u64,
    xor: u64,
    n: u32,
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn shape(s: &Side, e: &Element) -> (Shape, [f64; 3]) {
    let mut h = Shape {
        sum: 0,
        xor: 0,
        n: 0,
    };
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let r = e.first_vertex as usize..(e.first_vertex + e.vertices) as usize;
    for p in s.mesh.positions.get(r).unwrap_or(&[]) {
        let w: [f64; 3] =
            std::array::from_fn(|k| f64::from(p[k]) * s.mesh_metres + s.origin_metres[k]);
        let q = w.map(|x| (x * 1000.0).round() as i64 as u64);
        let v = mix(q[0] ^ mix(q[1] ^ mix(q[2])));
        h.sum = h.sum.wrapping_add(v);
        h.xor ^= v;
        h.n += 1;
        for k in 0..3 {
            lo[k] = lo[k].min(w[k]);
            hi[k] = hi[k].max(w[k]);
        }
    }
    (h, std::array::from_fn(|k| (lo[k] + hi[k]) / 2.0))
}

/// The fields compared, as (field, set, name) → text.
fn fields(d: &IfcData, o: &Object) -> BTreeMap<(Field, Option<String>, String), String> {
    let mut out = BTreeMap::new();
    for (n, v) in &o.attrs {
        if matches!(
            n.as_str(),
            "GlobalId" | "OwnerHistory" | "ObjectPlacement" | "Representation"
        ) {
            continue;
        }
        let refs = match v.plain() {
            Value::Ref(_) => true,
            Value::List(l) => l.iter().any(|x| matches!(x.plain(), Value::Ref(_))),
            _ => false,
        };
        if refs || matches!(v.plain(), Value::Null | Value::Derived) {
            continue;
        }
        out.insert((Field::Attribute, None, n.clone()), v.text());
    }
    let label = |x: &Object| x.name().unwrap_or_else(|| x.global_id.clone());
    if let Some(t) = d.type_of(o) {
        out.insert((Field::Type, None, String::new()), label(t));
    }
    if let Some(c) = o.container.and_then(|c| d.get(c)) {
        out.insert((Field::Container, None, String::new()), label(c));
    }
    if let Some(m) = d.material(o) {
        out.insert(
            (Field::Material, None, String::new()),
            m.values().join(", "),
        );
    }
    let mut codes: Vec<String> = d
        .classifications(o)
        .iter()
        .map(|c| {
            format!(
                "{}{}",
                c.system
                    .as_deref()
                    .map(|s| format!("{s}: "))
                    .unwrap_or_default(),
                c.codes.first().cloned().unwrap_or_default()
            )
        })
        .collect();
    codes.sort();
    if !codes.is_empty() {
        out.insert(
            (Field::Classification, None, String::new()),
            codes.join(", "),
        );
    }
    for ps in d.psets(o) {
        for p in &ps.props {
            out.insert(
                (Field::Property, Some(ps.name.clone()), p.name.clone()),
                p.text(),
            );
        }
    }
    out
}

/// Compares two versions; only objects that differ are listed, removed first, then added,
/// then changed, each by class and name.
pub fn diff(old: &Side, new: &Side) -> Vec<Change> {
    let comparable = |d: &IfcData, o: &Object| {
        !o.global_id.is_empty() && d.schema_table().is_a(&o.class, "IFCOBJECTDEFINITION")
    };
    let index = |s: &Side| -> HashMap<String, usize> {
        s.elements
            .iter()
            .enumerate()
            .map(|(i, e)| (e.global_id.clone(), i))
            .collect()
    };
    let (oi, ni) = (index(old), index(new));
    let olds: HashMap<&str, &Object> = old
        .data
        .objects
        .iter()
        .filter(|o| comparable(old.data, o))
        .map(|o| (o.global_id.as_str(), o))
        .collect();
    let news: HashMap<&str, &Object> = new
        .data
        .objects
        .iter()
        .filter(|o| comparable(new.data, o))
        .map(|o| (o.global_id.as_str(), o))
        .collect();
    let mut out = Vec::new();
    for (g, o) in &olds {
        if !news.contains_key(g) {
            out.push(Change {
                guid: (*g).to_owned(),
                class: o.class.clone(),
                name: o.name(),
                kind: ChangeKind::Removed,
                geometry: false,
                moved_m: None,
                fields: Vec::new(),
                old_element: oi.get(*g).copied(),
                new_element: None,
            });
        }
    }
    for (g, n) in &news {
        let Some(o) = olds.get(g) else {
            out.push(Change {
                guid: (*g).to_owned(),
                class: n.class.clone(),
                name: n.name(),
                kind: ChangeKind::Added,
                geometry: false,
                moved_m: None,
                fields: Vec::new(),
                old_element: None,
                new_element: ni.get(*g).copied(),
            });
            continue;
        };
        let (fo, fnew) = (fields(old.data, o), fields(new.data, n));
        let mut changes = Vec::new();
        for (k, v) in &fo {
            match fnew.get(k) {
                Some(w) if w == v => {}
                w => changes.push(FieldChange {
                    field: k.0,
                    set: k.1.clone(),
                    name: k.2.clone(),
                    old: Some(v.clone()),
                    new: w.cloned(),
                }),
            }
        }
        for (k, w) in &fnew {
            if !fo.contains_key(k) {
                changes.push(FieldChange {
                    field: k.0,
                    set: k.1.clone(),
                    name: k.2.clone(),
                    old: None,
                    new: Some(w.clone()),
                });
            }
        }
        if o.class != n.class {
            changes.insert(
                0,
                FieldChange {
                    field: Field::Attribute,
                    set: None,
                    name: "Class".into(),
                    old: Some(o.class.clone()),
                    new: Some(n.class.clone()),
                },
            );
        }
        let (oe, ne) = (oi.get(*g).copied(), ni.get(*g).copied());
        let (mut geometry, mut moved_m) = (false, None);
        match (oe, ne) {
            (Some(a), Some(b)) => {
                let (sa, ca) = shape(old, &old.elements[a]);
                let (sb, cb) = shape(new, &new.elements[b]);
                if sa != sb {
                    geometry = true;
                    let d = (0..3).map(|k| (cb[k] - ca[k]).powi(2)).sum::<f64>().sqrt();
                    if d > 0.0005 {
                        moved_m = Some(d);
                    }
                }
            }
            (None, None) => {}
            _ => geometry = true,
        }
        if geometry || !changes.is_empty() {
            out.push(Change {
                guid: (*g).to_owned(),
                class: n.class.clone(),
                name: n.name(),
                kind: ChangeKind::Changed,
                geometry,
                moved_m,
                fields: changes,
                old_element: oe,
                new_element: ne,
            });
        }
    }
    out.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.class.cmp(&b.class))
            .then(a.name.cmp(&b.name))
            .then(a.guid.cmp(&b.guid))
    });
    out
}

/// Counts by kind.
pub fn counts(changes: &[Change]) -> [usize; 3] {
    let mut c = [0; 3];
    for x in changes {
        c[match x.kind {
            ChangeKind::Added => 0,
            ChangeKind::Removed => 1,
            ChangeKind::Changed => 2,
        }] += 1;
    }
    c
}

/// Straight sRGB colours of added, changed, removed and unchanged (ghosted) elements.
pub const ADDED: [u8; 4] = [46, 160, 67, 255];
pub const CHANGED: [u8; 4] = [230, 160, 20, 255];
pub const REMOVED: [u8; 4] = [210, 50, 45, 255];
pub const GHOST: [u8; 4] = [190, 190, 190, 46];

fn linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The newer model with the changes in colour and the removed elements added from the
/// older one (in red). `selected` (a change's index) is drawn as selected.
pub fn coloured(old: &Side, new: &Side, changes: &[Change], selected: Option<usize>) -> Mesh {
    let mut looks = vec![
        ElementLook {
            colour: Some(GHOST),
            ..ElementLook::default()
        };
        new.elements.len()
    ];
    for (i, c) in changes.iter().enumerate() {
        if let Some(e) = c.new_element {
            looks[e].colour = Some(match c.kind {
                ChangeKind::Added => ADDED,
                _ => CHANGED,
            });
            looks[e].selected = selected == Some(i);
        }
    }
    let mut mesh = apply::restyle(new.mesh, new.elements, &looks);
    // The removed elements, moved into the newer mesh's frame.
    let removed: Vec<(usize, bool)> = changes
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == ChangeKind::Removed)
        .filter_map(|(i, c)| Some((c.old_element?, selected == Some(i))))
        .collect();
    if removed.is_empty() {
        return mesh;
    }
    let to_new = |p: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|k| {
            let w = f64::from(p[k]) * old.mesh_metres + old.origin_metres[k];
            ((w - new.origin_metres[k]) / new.mesh_metres.max(1e-12)) as f32
        })
    };
    let red = mesh.materials.len() as u32;
    mesh.materials.push(Material {
        name: "removed".into(),
        base_color: [
            linear(REMOVED[0]),
            linear(REMOVED[1]),
            linear(REMOVED[2]),
            1.0,
        ],
        texture: None,
    });
    let sel = mesh.materials.len() as u32;
    mesh.materials.push(Material {
        name: "selected".into(),
        base_color: apply::SELECTED,
        texture: None,
    });
    let elements = old.elements;
    let mut by_el: HashMap<usize, bool> = removed.iter().copied().collect();
    let base = mesh.positions.len() as u32;
    let mut remap: HashMap<u32, u32> = HashMap::new();
    let (mut red_tris, mut sel_tris) = (Vec::new(), Vec::new());
    for t in old.mesh.indices.chunks_exact(3) {
        let Some(e) = apply::element_of(elements, t[0]) else {
            continue;
        };
        let Some(&is_sel) = by_el.get(&e) else {
            continue;
        };
        let mut tri = [0u32; 3];
        for (k, &v) in t.iter().enumerate() {
            if let Some(&n) = remap.get(&v) {
                tri[k] = n;
                continue;
            }
            let n = base + remap.len() as u32;
            remap.insert(v, n);
            let p = to_new(old.mesh.positions[v as usize]);
            mesh.positions.push(p);
            mesh.normals.push(
                old.mesh
                    .normals
                    .get(v as usize)
                    .copied()
                    .unwrap_or([0.0, 0.0, 1.0]),
            );
            if let Some(c) = mesh.colors.as_mut() {
                c.push([255; 4]);
            }
            if let Some(u) = mesh.uvs.as_mut() {
                u.push([0.0; 2]);
            }
            mesh.bounds.add(p);
            tri[k] = n;
        }
        if is_sel {
            sel_tris.extend(tri);
        } else {
            red_tris.extend(tri);
        }
    }
    by_el.clear();
    for (tris, m) in [(red_tris, red), (sel_tris, sel)] {
        if tris.is_empty() {
            continue;
        }
        let first = (mesh.indices.len() / 3) as u32;
        mesh.parts.push(Part {
            first,
            count: (tris.len() / 3) as u32,
            material: Some(m),
        });
        mesh.indices.extend(tris);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_mesh::Limits;
    use specular_mesh::ifc::data;

    fn ifc(body: &str) -> IfcData {
        let text = format!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(),(),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{body}\nENDSEC;\nEND-ISO-10303-21;\n"
        );
        data::read(text.as_bytes(), &Limits::default()).unwrap()
    }

    fn mesh(boxes: &[[f32; 3]]) -> (Mesh, Vec<Element>) {
        let mut m = Mesh::default();
        let mut els = Vec::new();
        for (i, o) in boxes.iter().enumerate() {
            let first = m.positions.len() as u32;
            for k in 0..3u32 {
                m.positions.push([o[0] + k as f32, o[1], o[2]]);
                m.normals.push([0.0, 0.0, 1.0]);
            }
            m.indices.extend([first, first + 1, first + 2]);
            els.push(Element {
                global_id: format!("G{i}"),
                first_vertex: first,
                vertices: 3,
                ..Element::default()
            });
        }
        m.parts.push(Part {
            first: 0,
            count: boxes.len() as u32,
            material: None,
        });
        (m, els)
    }

    #[test]
    fn finds_added_removed_and_changed() {
        let a = ifc("#1=IFCWALL('G0',$,'W0',$,$,$,$,$,$);
#2=IFCWALL('G1',$,'W1',$,$,$,$,$,$);
#3=IFCWALL('G2',$,'W2',$,$,$,$,$,$);
#4=IFCPROPERTYSET('P1',$,'Pset_WallCommon',$,(#5));
#5=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('1HR'),$);
#6=IFCRELDEFINESBYPROPERTIES('R1',$,$,$,(#3),#4);");
        let b = ifc("#1=IFCWALL('G0',$,'W0',$,$,$,$,$,$);
#2=IFCWALL('G2',$,'W2',$,$,$,$,$,$);
#3=IFCWALL('G3',$,'W3',$,$,$,$,$,$);
#4=IFCPROPERTYSET('P1',$,'Pset_WallCommon',$,(#5));
#5=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('2HR'),$);
#6=IFCRELDEFINESBYPROPERTIES('R1',$,$,$,(#2),#4);");
        let (ma, ea) = mesh(&[[0.0; 3], [5.0, 0.0, 0.0], [10.0, 0.0, 0.0]]);
        let (mb, mut eb) = mesh(&[[0.0, 0.0, 0.002], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]]);
        eb[1].global_id = "G2".into();
        eb[2].global_id = "G3".into();
        let old = Side {
            data: &a,
            elements: &ea,
            mesh: &ma,
            mesh_metres: 1.0,
            origin_metres: [0.0; 3],
        };
        let new = Side {
            data: &b,
            elements: &eb,
            mesh: &mb,
            mesh_metres: 1.0,
            origin_metres: [0.0; 3],
        };
        let d = diff(&old, &new);
        let got: Vec<(ChangeKind, &str)> = d.iter().map(|c| (c.kind, c.guid.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (ChangeKind::Added, "G3"),
                (ChangeKind::Removed, "G1"),
                (ChangeKind::Changed, "G0"),
                (ChangeKind::Changed, "G2"),
            ]
        );
        // G0 moved 2 mm up.
        assert!(d[2].geometry);
        assert!((d[2].moved_m.unwrap() - 0.002).abs() < 1e-6);
        // G2's fire rating changed.
        assert!(!d[3].geometry);
        assert_eq!(
            d[3].fields,
            vec![FieldChange {
                field: Field::Property,
                set: Some("Pset_WallCommon".into()),
                name: "FireRating".into(),
                old: Some("1HR".into()),
                new: Some("2HR".into()),
            }]
        );
        assert_eq!(counts(&d), [1, 1, 2]);
        let m = coloured(&old, &new, &d, Some(1));
        // The removed wall's triangle is added, selected.
        assert_eq!(m.triangle_count(), 4);
        assert_eq!(m.positions.len(), 12);
        let last = m.parts.last().unwrap();
        assert_eq!(
            m.materials[last.material.unwrap() as usize].name,
            "selected"
        );
    }
}
