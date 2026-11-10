//! Colours: surface styles of styled items (on geometric items, or on materials through
//! their definition representations), the materials associated with elements and their
//! types, and a fallback colour per kind of element.

use std::collections::{HashMap, HashSet};

/// Materials gathered for one element, at most.
const MAX_MATERIALS: usize = 4096;

use super::Ctx;
use super::step::{Step, V};

/// A look: sRGB colour (0‥1) with opacity, and the name it came with.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Look {
    pub rgba: [f32; 4],
    pub name: String,
}

pub(crate) fn to_srgb8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The colour of an `IfcColourRgb` (or a pre-defined colour name).
fn colour(s: &Step, id: u32) -> Option<[f32; 3]> {
    let a = s.args(id)?;
    match s.ty(id)? {
        "IFCCOLOURRGB" => {
            let c = |k: usize| {
                a.get(k)
                    .and_then(V::as_f64)
                    .map(|v| v.clamp(0.0, 1.0) as f32)
            };
            Some([c(1)?, c(2)?, c(3)?])
        }
        "IFCDRAUGHTINGPREDEFINEDCOLOUR" => {
            let n = a.first()?.as_str()?.to_ascii_lowercase();
            Some(match n.as_str() {
                "black" => [0.0, 0.0, 0.0],
                "red" => [1.0, 0.0, 0.0],
                "green" => [0.0, 1.0, 0.0],
                "blue" => [0.0, 0.0, 1.0],
                "yellow" => [1.0, 1.0, 0.0],
                "magenta" => [1.0, 0.0, 1.0],
                "cyan" => [0.0, 1.0, 1.0],
                _ => [1.0, 1.0, 1.0],
            })
        }
        _ => None,
    }
}

/// The look of a presentation style (or an assignment of them), if it has a surface colour.
pub(crate) fn style_look(s: &Step, id: u32, depth: usize) -> Option<Look> {
    style_look_in(s, id, depth, &mut HashSet::new())
}

/// [`style_look`], visiting each style once (assignments may share them).
fn style_look_in(s: &Step, id: u32, depth: usize, seen: &mut HashSet<u32>) -> Option<Look> {
    if depth > 8 || !seen.insert(id) {
        return None;
    }
    let a = s.args(id)?;
    match s.ty(id)? {
        "IFCSURFACESTYLE" => {
            // (Name, Side, Styles)
            let name = a.first().and_then(V::as_str).unwrap_or_default();
            for el in a.get(2).map(V::refs).unwrap_or_default() {
                let Some(ea) = s.args(el) else { continue };
                match s.ty(el) {
                    Some("IFCSURFACESTYLESHADING" | "IFCSURFACESTYLERENDERING") => {
                        let Some(c) = ea.first().and_then(V::as_ref).and_then(|c| colour(s, c))
                        else {
                            continue;
                        };
                        let t = ea.get(1).and_then(V::as_f64).unwrap_or(0.0).clamp(0.0, 1.0);
                        return Some(Look {
                            rgba: [c[0], c[1], c[2], (1.0 - t) as f32],
                            name,
                        });
                    }
                    _ => continue,
                }
            }
            None
        }
        "IFCPRESENTATIONSTYLEASSIGNMENT" => a
            .first()
            .map(V::refs)
            .unwrap_or_default()
            .into_iter()
            .find_map(|st| style_look_in(s, st, depth + 1, seen)),
        _ => None,
    }
}

/// The look of an `IfcStyledItem`'s styles.
fn styled_item_look(s: &Step, a: &[V]) -> Option<Look> {
    let name = a.get(2).and_then(V::as_str);
    let mut look = a
        .get(1)
        .map(V::refs)
        .unwrap_or_default()
        .into_iter()
        .find_map(|st| style_look(s, st, 0))?;
    if look.name.trim().is_empty()
        && let Some(n) = name
    {
        look.name = n;
    }
    Some(look)
}

/// What the colours of a file are made of, found once.
#[derive(Default)]
pub(crate) struct Styles {
    /// Geometric item → look.
    pub item: HashMap<u32, u32>,
    /// Material → look.
    pub material: HashMap<u32, u32>,
    /// Object (element or type) → its associated material definition.
    pub assoc: HashMap<u32, u32>,
    /// Element → its type object.
    pub typ: HashMap<u32, u32>,
    pub looks: Vec<Look>,
}

impl Styles {
    fn add(&mut self, l: Look) -> u32 {
        if let Some(k) = self.looks.iter().position(|x| *x == l) {
            return k as u32;
        }
        self.looks.push(l);
        (self.looks.len() - 1) as u32
    }

    pub fn find(s: &Step) -> Styles {
        let mut st = Styles::default();
        for id in s.of_types(&["IFCSTYLEDITEM"]) {
            let Some(a) = s.args(id) else { continue };
            let Some(item) = a.first().and_then(V::as_ref) else {
                continue;
            };
            if let Some(l) = styled_item_look(s, &a) {
                let k = st.add(l);
                st.item.entry(item).or_insert(k);
            }
        }
        for id in s.of_types(&["IFCMATERIALDEFINITIONREPRESENTATION"]) {
            // (Name, Description, Representations, RepresentedMaterial)
            let Some(a) = s.args(id) else { continue };
            let Some(mat) = a.get(3).and_then(V::as_ref) else {
                continue;
            };
            let mut found = None;
            'reps: for rep in a.get(2).map(V::refs).unwrap_or_default() {
                let Some(ra) = s.args(rep) else { continue };
                for it in ra.get(3).map(V::refs).unwrap_or_default() {
                    if s.ty(it) != Some("IFCSTYLEDITEM") {
                        continue;
                    }
                    if let Some(l) = s.args(it).and_then(|ia| styled_item_look(s, &ia)) {
                        found = Some(l);
                        break 'reps;
                    }
                }
            }
            if let Some(mut l) = found {
                if l.name.trim().is_empty() {
                    l.name = material_name(s, mat).unwrap_or_default();
                }
                let k = st.add(l);
                st.material.insert(mat, k);
            }
        }
        for id in s.of_types(&["IFCRELASSOCIATESMATERIAL"]) {
            // (GlobalId, OwnerHistory, Name, Description, RelatedObjects, RelatingMaterial)
            let Some(a) = s.args(id) else { continue };
            let Some(m) = a.get(5).and_then(V::as_ref) else {
                continue;
            };
            for o in a.get(4).map(V::refs).unwrap_or_default() {
                st.assoc.insert(o, m);
            }
        }
        for id in s.of_types(&["IFCRELDEFINESBYTYPE"]) {
            let Some(a) = s.args(id) else { continue };
            let Some(t) = a.get(5).and_then(V::as_ref) else {
                continue;
            };
            for o in a.get(4).map(V::refs).unwrap_or_default() {
                st.typ.insert(o, t);
            }
        }
        st
    }
}

fn material_name(s: &Step, m: u32) -> Option<String> {
    let a = s.args(m)?;
    a.first()
        .and_then(V::as_str)
        .filter(|n| !n.trim().is_empty())
}

/// The materials of a material definition, in order (layers, profiles, constituents,
/// lists).
fn materials_of(s: &Step, def: u32, out: &mut Vec<u32>, depth: usize, seen: &mut HashSet<u32>) {
    // Each definition once: sets may list the same layers many times over.
    if depth > 6 || out.len() >= MAX_MATERIALS || !seen.insert(def) {
        return;
    }
    let Some(a) = s.args(def) else { return };
    let refs = |k: usize| a.get(k).map(V::refs).unwrap_or_default();
    match s.ty(def).unwrap_or("") {
        "IFCMATERIAL" => out.push(def),
        "IFCMATERIALLAYERSETUSAGE"
        | "IFCMATERIALPROFILESETUSAGE"
        | "IFCMATERIALPROFILESETUSAGETAPERING" => {
            if let Some(set) = a.first().and_then(V::as_ref) {
                materials_of(s, set, out, depth + 1, seen);
            }
        }
        "IFCMATERIALLAYERSET" | "IFCMATERIALLIST" => {
            for l in refs(0) {
                materials_of(s, l, out, depth + 1, seen);
            }
        }
        "IFCMATERIALLAYER" | "IFCMATERIALLAYERWITHOFFSETS" => {
            if let Some(m) = a.first().and_then(V::as_ref) {
                out.push(m);
            }
        }
        "IFCMATERIALPROFILESET" | "IFCMATERIALCONSTITUENTSET" => {
            for p in refs(2) {
                materials_of(s, p, out, depth + 1, seen);
            }
        }
        "IFCMATERIALPROFILE" | "IFCMATERIALPROFILEWITHOFFSETS" | "IFCMATERIALCONSTITUENT" => {
            if let Some(m) = a.get(2).and_then(V::as_ref) {
                out.push(m);
            }
        }
        _ => {}
    }
}

impl Ctx<'_> {
    /// The look of an element from its material (or its type's), if a material has one.
    pub fn material_look(&self, element: u32) -> Option<u32> {
        let def = self.styles.assoc.get(&element).or_else(|| {
            self.styles
                .typ
                .get(&element)
                .and_then(|t| self.styles.assoc.get(t))
        })?;
        let mut mats = Vec::new();
        materials_of(&self.s, *def, &mut mats, 0, &mut HashSet::new());
        mats.iter()
            .find_map(|m| self.styles.material.get(m).copied())
    }
}

/// The colour drawn for an element without one of its own, by its kind (sRGB, opacity).
pub(crate) fn kind_colour(ty: &str) -> [f32; 4] {
    let t = ty.strip_prefix("IFC").unwrap_or(ty);
    let c: [f32; 4] = match t {
        _ if t.starts_with("WALL") || t == "CURTAINWALL" => [0.88, 0.86, 0.82, 1.0],
        _ if t.starts_with("SLAB") || t == "COVERING" => [0.72, 0.72, 0.72, 1.0],
        "ROOF" => [0.62, 0.36, 0.30, 1.0],
        _ if t.starts_with("WINDOW") => [0.62, 0.78, 0.90, 0.45],
        _ if t.starts_with("DOOR") => [0.62, 0.47, 0.32, 1.0],
        "PLATE" | "PLATESTANDARDCASE" => [0.70, 0.80, 0.88, 0.6],
        "COLUMN" | "COLUMNSTANDARDCASE" | "BEAM" | "BEAMSTANDARDCASE" | "MEMBER"
        | "MEMBERSTANDARDCASE" | "FOOTING" | "PILE" => [0.70, 0.71, 0.75, 1.0],
        _ if t.starts_with("STAIR") || t.starts_with("RAMP") || t == "RAILING" => {
            [0.78, 0.74, 0.68, 1.0]
        }
        "FURNISHINGELEMENT" | "FURNITURE" | "SYSTEMFURNITUREELEMENT" => [0.80, 0.66, 0.48, 1.0],
        "SITE" | "GEOGRAPHICELEMENT" | "EARTHWORKSFILL" | "EARTHWORKSCUT" => {
            [0.56, 0.66, 0.46, 1.0]
        }
        "REINFORCINGBAR" | "REINFORCINGMESH" | "TENDON" => [0.55, 0.35, 0.30, 1.0],
        _ if t.starts_with("FLOW")
            || t.ends_with("SEGMENT")
            || t.ends_with("FITTING")
            || t.ends_with("TERMINAL")
            || t.starts_with("DISTRIBUTION") =>
        {
            [0.60, 0.72, 0.62, 1.0]
        }
        _ => [0.80, 0.80, 0.80, 1.0],
    };
    c
}
