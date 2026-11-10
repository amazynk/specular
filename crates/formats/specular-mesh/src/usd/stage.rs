//! A USD stage, composed as far as a viewer needs: the root layer's sublayers, references and
//! payloads (to other layers or within one), variant selections, and from it the scene:
//! `Mesh` prims (points, faces, normals, texture coordinates and display colours with their
//! interpolations and indices, left-handed winding, material subsets), the implicit shapes
//! (`Cube`, `Sphere`, `Cylinder`, `Cone`, `Capsule`, `Plane`), `PointInstancer` instances,
//! transforms from `xformOpOrder`, material bindings to `UsdPreviewSurface` base colours and
//! `UsdUVTexture` files. Prims that are inactive, invisible, guides or proxies are left out;
//! curves, points, NURBS and volumes are counted and named in a note.
//!
//! Composition is simplified: arcs are expanded depth first per contributing spec (local,
//! variants, references, payloads), stronger opinions first; inherits and specializes are not
//! followed. Values are taken at the earliest time: a spec's first sample, else its default.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::value::{Layer, V, child_path, parent_path};
use crate::builder::{Affine, Builder, linear_to_srgb8};
use crate::poly::{Attr, Poly, emit};
use crate::{Error, ImageData, Material, Result, Unit, Up};

/// Where layers and textures come from.
pub(crate) trait Files {
    /// The bytes of `id` (a normalised path relative to the package or the root's folder).
    fn read(&self, id: &str, max: usize) -> std::result::Result<Vec<u8>, String>;
}

/// One opinion on a prim: a spec in a layer, and how its paths map to the stage's.
#[derive(Clone, Debug)]
struct Node {
    layer: usize,
    path: String,
    /// (prefix in the layer, prefix on the stage) for paths authored in this layer.
    map: Option<(String, String)>,
}

pub(crate) struct Stage<'a> {
    files: &'a dyn Files,
    max_file: usize,
    layers: Vec<Layer>,
    /// Layer ids (relative paths), by index.
    ids: Vec<String>,
    by_id: HashMap<String, Option<usize>>,
    /// Layer stacks: a layer and its sublayers, strongest first.
    stacks: HashMap<usize, Rc<Vec<usize>>>,
    root_stack: Rc<Vec<usize>>,
    prims: HashMap<String, Rc<Vec<Node>>>,
    missing_layers: HashSet<String>,
    materials: HashMap<String, Option<u32>>,
    images: HashMap<String, Option<u32>>,
    skipped: HashMap<String, usize>,
    visited: usize,
}

const MAX_PRIMS: usize = 2_000_000;
const MAX_DEPTH: usize = 64;

/// Strips variant selections: `/A{v=x}B` → `/A/B`.
fn strip_variants(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    let mut depth = 0;
    for c in p.chars() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                // A child follows a selection without a slash.
                out.push('/');
            }
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("//", "/");
    let out = out.trim_end_matches('/');
    if out.is_empty() {
        "/".to_owned()
    } else {
        out.to_owned()
    }
}

/// `rel` relative to the folder of layer `id`, normalised (leading `..` kept: the files
/// decide how far up they may go); `None` for absolute paths and URLs.
fn join(id: &str, rel: &str) -> Option<String> {
    let rel = rel.trim();
    if rel.is_empty() || rel.contains("://") {
        return None;
    }
    let base = match id.rfind('/') {
        Some(i) => &id[..i],
        None => "",
    };
    let full = if base.is_empty() {
        rel.to_owned()
    } else {
        format!("{base}/{rel}")
    };
    let (up, p) = crate::resolve::relative_climbing(&full, 64)?;
    let mut parts: Vec<String> = vec!["..".to_owned(); up];
    parts.extend(
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(parts.join("/"))
}

/// A 4×4 matrix, column vectors (`m[row][col]`).
type M4 = [[f64; 4]; 4];

const IDENTITY: M4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut m = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            m[r][c] = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

fn affine(m: &M4) -> Affine {
    Affine([m[0], m[1], m[2]])
}

fn translate(t: [f64; 3]) -> M4 {
    let mut m = IDENTITY;
    m[0][3] = t[0];
    m[1][3] = t[1];
    m[2][3] = t[2];
    m
}

fn scale(s: [f64; 3]) -> M4 {
    let mut m = IDENTITY;
    m[0][0] = s[0];
    m[1][1] = s[1];
    m[2][2] = s[2];
    m
}

/// A rotation of `deg` degrees about axis `a` (0 x, 1 y, 2 z).
fn rot(a: usize, deg: f64) -> M4 {
    let (s, c) = deg.to_radians().sin_cos();
    let (i, j) = match a {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let mut m = IDENTITY;
    m[i][i] = c;
    m[i][j] = -s;
    m[j][i] = s;
    m[j][j] = c;
    m
}

/// The rotation of a quaternion (w, x, y, z).
fn quat(q: &[f64]) -> M4 {
    let l = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    let [w, x, y, z] = if l > 0.0 && l.is_finite() {
        [q[0] / l, q[1] / l, q[2] / l, q[3] / l]
    } else {
        [1.0, 0.0, 0.0, 0.0]
    };
    let mut m = IDENTITY;
    m[0] = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y - w * z),
        2.0 * (x * z + w * y),
        0.0,
    ];
    m[1] = [
        2.0 * (x * y + w * z),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z - w * x),
        0.0,
    ];
    m[2] = [
        2.0 * (x * z - w * y),
        2.0 * (y * z + w * x),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
    ];
    m
}

/// The inverse of an affine 4×4 (the identity when singular).
fn invert(m: &M4) -> M4 {
    let a = affine(m);
    let det = a.det();
    if det.abs() < 1e-300 || !det.is_finite() {
        return IDENTITY;
    }
    let n = a.normal_matrix();
    // The inverse of the linear part is the cofactor matrix transposed over the determinant.
    let mut inv = IDENTITY;
    for r in 0..3 {
        for c in 0..3 {
            inv[r][c] = n[c][r] / det;
        }
    }
    let t = [m[0][3], m[1][3], m[2][3]];
    for row in inv.iter_mut().take(3) {
        row[3] = -(row[0] * t[0] + row[1] * t[1] + row[2] * t[2]);
    }
    inv
}

/// A primvar: its values, interpolation and indices.
struct Primvar {
    value: V,
    interpolation: String,
    indices: Option<Vec<i64>>,
}

impl<'a> Stage<'a> {
    pub fn open(files: &'a dyn Files, root_id: &str, root: Layer, max_file: usize) -> Stage<'a> {
        let mut st = Stage {
            files,
            max_file,
            layers: vec![root],
            ids: vec![root_id.to_owned()],
            by_id: HashMap::new(),
            stacks: HashMap::new(),
            root_stack: Rc::new(Vec::new()),
            prims: HashMap::new(),
            missing_layers: HashSet::new(),
            materials: HashMap::new(),
            images: HashMap::new(),
            skipped: HashMap::new(),
            visited: 0,
        };
        st.by_id.insert(root_id.to_owned(), Some(0));
        st.root_stack = st.stack(0, 0);
        st
    }

    fn load(&mut self, id: &str) -> Option<usize> {
        if let Some(i) = self.by_id.get(id) {
            return *i;
        }
        // Guard against cycles and floods of layers.
        self.by_id.insert(id.to_owned(), None);
        if self.layers.len() > 4096 {
            return None;
        }
        let bytes = match self.files.read(id, self.max_file) {
            Ok(b) => b,
            Err(_) => {
                self.missing_layers.insert(id.to_owned());
                return None;
            }
        };
        let layer = match super::parse_layer(&bytes) {
            Ok(l) => l,
            Err(_) => {
                self.missing_layers.insert(id.to_owned());
                return None;
            }
        };
        self.layers.push(layer);
        self.ids.push(id.to_owned());
        let i = self.layers.len() - 1;
        self.by_id.insert(id.to_owned(), Some(i));
        Some(i)
    }

    /// Layer `l` and its sublayers, strongest first.
    fn stack(&mut self, l: usize, depth: usize) -> Rc<Vec<usize>> {
        if let Some(s) = self.stacks.get(&l) {
            return s.clone();
        }
        let mut out = vec![l];
        // In progress: a layer listing itself (directly or further down) gets just itself,
        // and one listed many times is expanded once.
        self.stacks.insert(l, Rc::new(out.clone()));
        if depth < 16 {
            let subs = self.layers[l]
                .meta("subLayers")
                .and_then(V::strs)
                .unwrap_or_default();
            for s in subs {
                let Some(id) = join(&self.ids[l], &s) else {
                    self.missing_layers.insert(s);
                    continue;
                };
                if let Some(sl) = self.load(&id) {
                    for x in self.stack(sl, depth + 1).iter() {
                        if !out.contains(x) {
                            out.push(*x);
                        }
                    }
                }
            }
        }
        let r = Rc::new(out);
        self.stacks.insert(l, r.clone());
        r
    }

    /// The prim (or variant, or root) spec of a node.
    fn spec(&self, n: &Node) -> Option<&super::value::Spec> {
        self.layers[n.layer].specs.get(&n.path).filter(|s| {
            !matches!(
                s.kind,
                super::value::SpecKind::Attribute | super::value::SpecKind::Relationship
            )
        })
    }

    /// The stage path a node's prim stands for.
    fn stage_path(&self, n: &Node) -> String {
        let p = strip_variants(&n.path);
        self.map_path(n, &p)
    }

    /// A path authored in `n`'s layer, on the stage.
    fn map_path(&self, n: &Node, p: &str) -> String {
        let p = strip_variants(p);
        match &n.map {
            Some((from, to)) => {
                let from = strip_variants(from);
                if p == from {
                    to.clone()
                } else if let Some(rest) = p.strip_prefix(&from)
                    && (rest.starts_with('/') || rest.starts_with('.'))
                {
                    if to == "/" && rest.starts_with('/') {
                        rest.to_owned()
                    } else {
                        format!("{to}{rest}")
                    }
                } else {
                    p
                }
            }
            None => p,
        }
    }

    /// Expands `n` with its variants and arcs, appending to `out`.
    fn expand(
        &mut self,
        n: Node,
        sel: &mut Vec<(String, String)>,
        out: &mut Vec<Node>,
        depth: usize,
    ) {
        if depth > MAX_DEPTH || out.iter().any(|o| o.layer == n.layer && o.path == n.path) {
            return;
        }
        let Some(spec) = self.spec(&n).cloned() else {
            return;
        };
        out.push(n.clone());
        if let Some(V::Dict(d)) = spec.get("variantSelection") {
            for (k, v) in d {
                if let Some(v) = v.as_str()
                    && !sel.iter().any(|(s, _)| s == k)
                {
                    sel.push((k.clone(), v.to_owned()));
                }
            }
        }
        for set in spec
            .get("variantSetNames")
            .and_then(V::strs)
            .unwrap_or_default()
        {
            let Some((_, choice)) = sel.iter().find(|(s, _)| *s == set).cloned() else {
                continue;
            };
            let vp = format!("{}{{{set}={choice}}}", n.path);
            if self.layers[n.layer].specs.contains_key(&vp) {
                self.expand(
                    Node {
                        layer: n.layer,
                        path: vp,
                        map: n.map.clone(),
                    },
                    sel,
                    out,
                    depth + 1,
                );
            }
        }
        let here = self.stage_path(&n);
        let mut arcs = Vec::new();
        for f in ["references", "payload"] {
            if let Some(V::Arcs(a)) = spec.get(f) {
                arcs.extend(a.iter().cloned());
            }
        }
        for arc in arcs {
            let stack = if arc.asset.is_empty() {
                self.stack(n.layer, 0)
            } else {
                let Some(id) = join(&self.ids[n.layer], &arc.asset) else {
                    self.missing_layers.insert(arc.asset.clone());
                    continue;
                };
                match self.load(&id) {
                    Some(l) => self.stack(l, 0),
                    None => continue,
                }
            };
            let target = if arc.prim.is_empty() {
                let l = &self.layers[stack[0]];
                match l.meta("defaultPrim").and_then(V::as_str) {
                    Some(d) if !d.is_empty() => format!("/{}", d.trim_start_matches('/')),
                    _ => match l
                        .root()
                        .and_then(|r| r.get("primChildren"))
                        .and_then(V::strs)
                        .and_then(|c| c.into_iter().next())
                    {
                        Some(c) => format!("/{c}"),
                        None => continue,
                    },
                }
            } else {
                arc.prim.clone()
            };
            for &l in stack.iter() {
                if self.layers[l].specs.contains_key(&target) {
                    self.expand(
                        Node {
                            layer: l,
                            path: target.clone(),
                            map: Some((target.clone(), here.clone())),
                        },
                        sel,
                        out,
                        depth + 1,
                    );
                }
            }
        }
    }

    /// The composed opinions on the prim at stage path `p`, strongest first.
    fn prim(&mut self, p: &str) -> Rc<Vec<Node>> {
        if let Some(n) = self.prims.get(p) {
            return n.clone();
        }
        let direct: Vec<Node> = if p == "/" {
            self.root_stack
                .clone()
                .iter()
                .filter(|&&l| self.layers[l].specs.contains_key("/"))
                .map(|&l| Node {
                    layer: l,
                    path: "/".into(),
                    map: None,
                })
                .collect()
        } else {
            let parent = self.prim(parent_path(p));
            let name = &p[p.rfind('/').map_or(0, |i| i + 1)..];
            parent
                .iter()
                .map(|n| Node {
                    layer: n.layer,
                    path: child_path(&n.path, name),
                    map: n.map.clone(),
                })
                .filter(|n| self.layers[n.layer].specs.contains_key(&n.path))
                .collect()
        };
        let mut out = Vec::new();
        let mut sel = Vec::new();
        for n in direct {
            self.expand(n, &mut sel, &mut out, 0);
        }
        let r = Rc::new(out);
        self.prims.insert(p.to_owned(), r.clone());
        r
    }

    fn field<'s>(&'s self, nodes: &[Node], name: &str) -> Option<&'s V> {
        nodes
            .iter()
            .find_map(|n| self.spec(n).and_then(|s| s.get(name)))
    }

    /// Children names, merged in order.
    fn children(&self, nodes: &[Node]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for n in nodes {
            if let Some(c) = self
                .spec(n)
                .and_then(|s| s.get("primChildren"))
                .and_then(V::strs)
            {
                for c in c {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        out
    }

    /// An attribute's value (the earliest sample, else the default) and the node that gave it.
    fn attr_from(&self, nodes: &[Node], name: &str) -> Option<(V, usize)> {
        for (k, n) in nodes.iter().enumerate() {
            let Some(s) = self.layers[n.layer]
                .specs
                .get(&format!("{}.{name}", n.path))
            else {
                continue;
            };
            // At the earliest time: a spec's samples win over its default.
            if let Some(v) = s.get("timeSamples") {
                let v = v.first_sample();
                if *v != V::None {
                    return Some((v.clone(), k));
                }
            }
            if let Some(v) = s.get("default").filter(|v| **v != V::None) {
                return Some((v.clone(), k));
            }
        }
        None
    }

    fn attr(&self, nodes: &[Node], name: &str) -> Option<V> {
        self.attr_from(nodes, name).map(|(v, _)| v)
    }

    /// A field on a property spec (`interpolation`, `elementSize`), strongest first.
    fn prop_field(&self, nodes: &[Node], name: &str, field: &str) -> Option<V> {
        nodes.iter().find_map(|n| {
            self.layers[n.layer]
                .specs
                .get(&format!("{}.{name}", n.path))
                .and_then(|s| s.get(field))
                .cloned()
        })
    }

    /// The stage paths a relationship or a connection targets.
    fn targets(&self, nodes: &[Node], name: &str, field: &str) -> Vec<String> {
        for n in nodes {
            let Some(s) = self.layers[n.layer]
                .specs
                .get(&format!("{}.{name}", n.path))
            else {
                continue;
            };
            if let Some(v) = s.get(field) {
                let t = v.paths();
                if !t.is_empty() {
                    return t.iter().map(|p| self.map_path(n, p)).collect();
                }
            }
        }
        Vec::new()
    }

    fn primvar(&self, nodes: &[Node], name: &str) -> Option<Primvar> {
        let attr = format!("primvars:{name}");
        let value = self.attr(nodes, &attr)?;
        let interpolation = self
            .prop_field(nodes, &attr, "interpolation")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "constant".into());
        let indices = self
            .attr(nodes, &format!("{attr}:indices"))
            .and_then(|v| v.nums().map(|n| n.iter().map(|&x| x as i64).collect()));
        Some(Primvar {
            value,
            interpolation,
            indices,
        })
    }

    /// The prim's local transform and whether it resets the parent's.
    fn xform(&self, nodes: &[Node]) -> (M4, bool) {
        let Some(order) = self.attr(nodes, "xformOpOrder").and_then(|v| v.strs()) else {
            // Older files: a bare `xformOp:transform` or none.
            return (IDENTITY, false);
        };
        let mut m = IDENTITY;
        let mut reset = false;
        for op in order {
            if op == "!resetXformStack!" {
                reset = true;
                m = IDENTITY;
                continue;
            }
            let (invert_op, name) = match op.strip_prefix("!invert!") {
                Some(n) => (true, n.to_owned()),
                None => (false, op.clone()),
            };
            let Some(v) = self.attr(nodes, &name) else {
                continue;
            };
            let Some(d) = v.nums() else { continue };
            let kind = name
                .strip_prefix("xformOp:")
                .unwrap_or(&name)
                .split(':')
                .next()
                .unwrap_or("");
            let v3 = |d: &[f64], fill: f64| [0, 1, 2].map(|k| d.get(k).copied().unwrap_or(fill));
            let opm = match kind {
                "translate" => translate(v3(&d, 0.0)),
                "scale" => {
                    if d.len() == 1 {
                        scale([d[0]; 3])
                    } else {
                        scale(v3(&d, 1.0))
                    }
                }
                "rotateX" => rot(0, d[0]),
                "rotateY" => rot(1, d[0]),
                "rotateZ" => rot(2, d[0]),
                "orient" if d.len() >= 4 => quat(&d[..4]),
                "transform" if d.len() >= 16 => {
                    // Stored row by row for row vectors: the transpose of ours.
                    let mut t = IDENTITY;
                    for r in 0..4 {
                        for c in 0..4 {
                            t[r][c] = d[c * 4 + r];
                        }
                    }
                    t
                }
                k if k.starts_with("rotate") && k.len() == 9 => {
                    // rotateXYZ: X first, then Y, then Z.
                    let mut r = IDENTITY;
                    for ax in k[6..].chars() {
                        let a = match ax {
                            'X' => 0,
                            'Y' => 1,
                            _ => 2,
                        };
                        r = mul(&rot(a, d.get(a).copied().unwrap_or(0.0)), &r);
                    }
                    r
                }
                _ => continue,
            };
            let opm = if invert_op { invert(&opm) } else { opm };
            m = mul(&m, &opm);
        }
        (m, reset)
    }

    /// Reads the scene into `b`.
    pub fn build(&mut self, b: &mut Builder) -> Result<()> {
        let root = self.prim("/");
        for c in self.children(&root) {
            self.walk(&child_path("/", &c), &IDENTITY, None, b, 0)?;
        }
        if !self.missing_layers.is_empty() {
            let mut m: Vec<_> = self.missing_layers.iter().cloned().collect();
            m.sort();
            b.note(format!(
                "{} layers not found or unreadable: {}",
                m.len(),
                m.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        let mut skipped: Vec<_> = self.skipped.iter().collect();
        skipped.sort();
        if !skipped.is_empty() {
            let list: Vec<String> = skipped.iter().map(|(k, n)| format!("{n} {k}")).collect();
            if b.triangle_count() == 0 {
                return Err(Error::Unsupported(format!(
                    "USD geometry of kinds not shown ({})",
                    list.join(", ")
                )));
            }
            b.note(format!("not shown: {}", list.join(", ")));
        }
        Ok(())
    }

    fn walk(
        &mut self,
        path: &str,
        parent: &M4,
        bound: Option<String>,
        b: &mut Builder,
        depth: usize,
    ) -> Result<()> {
        self.visited += 1;
        if self.visited > MAX_PRIMS || depth > 256 {
            b.note_once("too many prims; the rest are not shown");
            return Ok(());
        }
        let nodes = self.prim(path);
        if nodes.is_empty() {
            return Ok(());
        }
        // The strongest def or class decides (overs only refine); a class is never drawn,
        // nor a prim of overs alone.
        let specifier = nodes.iter().find_map(|n| {
            self.spec(n)
                .and_then(|s| s.get("specifier"))
                .and_then(V::as_str)
                .filter(|s| *s != "over")
        });
        if specifier != Some("def") {
            return Ok(());
        }
        if self.field(&nodes, "active").and_then(V::as_bool) == Some(false) {
            return Ok(());
        }
        if self.attr(&nodes, "visibility").as_ref().and_then(V::as_str) == Some("invisible") {
            return Ok(());
        }
        if matches!(
            self.attr(&nodes, "purpose").as_ref().and_then(V::as_str),
            Some("guide" | "proxy")
        ) {
            return Ok(());
        }
        let type_name = self
            .field(&nodes, "typeName")
            .and_then(V::as_str)
            .unwrap_or("")
            .to_owned();
        let (local, reset) = self.xform(&nodes);
        let world = if reset { local } else { mul(parent, &local) };
        let bound = self
            .targets(&nodes, "material:binding", "targetPaths")
            .into_iter()
            .next()
            .or(bound);
        match type_name.as_str() {
            "Mesh" => self.mesh(path, &nodes, &world, bound.as_deref(), b)?,
            "Cube" | "Sphere" | "Cylinder" | "Cone" | "Capsule" | "Plane" => {
                self.implicit(&type_name, &nodes, &world, bound.as_deref(), b)?
            }
            "PointInstancer" => return self.instancer(&nodes, &world, bound, b, depth),
            "BasisCurves" | "NurbsCurves" | "HermiteCurves" | "Points" | "NurbsPatch"
            | "TetMesh" | "Volume" | "OpenVDBAsset" | "Field3DAsset" => {
                *self.skipped.entry(type_name.clone()).or_default() += 1;
            }
            _ => {}
        }
        for c in self.children(&nodes) {
            self.walk(&child_path(path, &c), &world, bound.clone(), b, depth + 1)?;
        }
        Ok(())
    }

    fn instancer(
        &mut self,
        nodes: &[Node],
        world: &M4,
        bound: Option<String>,
        b: &mut Builder,
        depth: usize,
    ) -> Result<()> {
        let protos = self.targets(nodes, "prototypes", "targetPaths");
        let ids = self
            .attr(nodes, "protoIndices")
            .and_then(|v| v.nums().map(|n| n.into_owned()))
            .unwrap_or_default();
        let pos = self
            .attr(nodes, "positions")
            .and_then(|v| v.nums().map(|n| n.into_owned()))
            .unwrap_or_default();
        let ori = self
            .attr(nodes, "orientations")
            .and_then(|v| v.nums().map(|n| n.into_owned()));
        let scl = self
            .attr(nodes, "scales")
            .and_then(|v| v.nums().map(|n| n.into_owned()));
        let n = ids.len().min(pos.len() / 3);
        let budget = b.limits.max_triangles / 2;
        for i in 0..n {
            if b.triangle_count() > budget {
                b.note(format!("only {i} of {n} instances shown (the cap)"));
                break;
            }
            let Some(p) = protos.get(ids[i] as usize) else {
                continue;
            };
            let mut m = translate([pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]]);
            if let Some(o) = ori.as_ref().and_then(|o| o.get(i * 4..i * 4 + 4)) {
                m = mul(&m, &quat(o));
            }
            if let Some(s) = scl.as_ref().and_then(|s| s.get(i * 3..i * 3 + 3)) {
                m = mul(&m, &scale([s[0], s[1], s[2]]));
            }
            let m = mul(world, &m);
            // The prototype's own transform applies under the instance's; its parent's does
            // not (prototypes sit under the instancer).
            self.walk(&p.clone(), &m, bound.clone(), b, depth + 1)?;
        }
        Ok(())
    }

    /// The material bound at `path` (stage path), added to the builder once.
    fn material(&mut self, path: &str, b: &mut Builder) -> Option<u32> {
        if let Some(m) = self.materials.get(path) {
            return *m;
        }
        self.materials.insert(path.to_owned(), None);
        let nodes = self.prim(path);
        if nodes.is_empty() {
            return None;
        }
        let name = path.rsplit('/').next().unwrap_or("").to_owned();
        let mut mat = Material {
            name,
            ..Material::default()
        };
        let surface = self
            .targets(&nodes, "outputs:surface", "connectionPaths")
            .into_iter()
            .next();
        if let Some(out) = surface {
            let shader = out.split('.').next().unwrap_or("").to_owned();
            let sn = self.prim(&shader);
            let id = self.attr(&sn, "info:id");
            if id.as_ref().and_then(V::as_str) == Some("UsdPreviewSurface") {
                match self.input(&sn, "inputs:diffuseColor", 0) {
                    Some(Input::Value(v)) => {
                        if let Some(d) = v.nums()
                            && d.len() >= 3
                        {
                            mat.base_color[..3].copy_from_slice(&[
                                d[0] as f32,
                                d[1] as f32,
                                d[2] as f32,
                            ]);
                        }
                    }
                    Some(Input::Shader(tex)) => {
                        mat.texture = self.texture(&tex, b);
                        // A texture's own fallback colour is not used: the image is there or
                        // its absence is noted.
                    }
                    None => {}
                }
                if let Some(Input::Value(v)) = self.input(&sn, "inputs:opacity", 0)
                    && let Some(o) = v.as_f64()
                {
                    mat.base_color[3] = o.clamp(0.0, 1.0) as f32;
                }
            }
        }
        let i = b.add_material(mat);
        self.materials.insert(path.to_owned(), Some(i));
        Some(i)
    }

    /// An input's value or the shader prim it is connected to, through material interface
    /// inputs.
    fn input(&mut self, nodes: &[Node], name: &str, depth: usize) -> Option<Input> {
        if depth > 8 {
            return None;
        }
        if let Some(t) = self
            .targets(nodes, name, "connectionPaths")
            .into_iter()
            .next()
        {
            let (prim, prop) = t.split_once('.')?;
            let pn = self.prim(prim);
            if prop.starts_with("outputs:") {
                return Some(Input::Shader(prim.to_owned()));
            }
            return self.input(&pn, prop, depth + 1);
        }
        self.attr(nodes, name).map(Input::Value)
    }

    /// The image of a `UsdUVTexture` shader at `path`.
    fn texture(&mut self, path: &str, b: &mut Builder) -> Option<u32> {
        let nodes = self.prim(path);
        let (file, k) = self.attr_from(&nodes, "inputs:file")?;
        let name = file.as_str()?.to_owned();
        let layer = nodes[k].layer;
        let id = join(&self.ids[layer], &name);
        let key = id.clone().unwrap_or_else(|| name.clone());
        if let Some(i) = self.images.get(&key) {
            return *i;
        }
        let data = match id {
            Some(id) => match self.files.read(&id, b.limits.max_side_file) {
                Ok(bytes) => ImageData::Bytes {
                    bytes,
                    mime: mime_of(&id).map(str::to_owned),
                },
                Err(reason) => ImageData::Missing {
                    name: name.clone(),
                    reason,
                },
            },
            None => ImageData::Missing {
                name: name.clone(),
                reason: "not a relative path inside the model's folder".into(),
            },
        };
        let i = b.add_image(data);
        self.images.insert(key, i);
        i
    }

    /// The texture-coordinate primvar a material reads (its primvar reader's `varname`).
    fn uv_name(&mut self, material: &str) -> Option<String> {
        let nodes = self.prim(material);
        let out = self
            .targets(&nodes, "outputs:surface", "connectionPaths")
            .into_iter()
            .next()?;
        let shader = out.split('.').next()?.to_owned();
        let sn = self.prim(&shader);
        let Some(Input::Shader(tex)) = self.input(&sn, "inputs:diffuseColor", 0) else {
            return None;
        };
        let tn = self.prim(&tex);
        let Some(Input::Shader(reader)) = self.input(&tn, "inputs:st", 0) else {
            return None;
        };
        let rn = self.prim(&reader);
        match self.input(&rn, "inputs:varname", 0) {
            Some(Input::Value(v)) => v.as_str().map(str::to_owned),
            _ => None,
        }
    }

    fn mesh(
        &mut self,
        path: &str,
        nodes: &[Node],
        world: &M4,
        bound: Option<&str>,
        b: &mut Builder,
    ) -> Result<()> {
        let nums = |v: Option<V>| v.and_then(|v| v.nums().map(|n| n.into_owned()));
        let Some(pts) = nums(self.attr(nodes, "points")) else {
            return Ok(());
        };
        let counts: Vec<u32> = nums(self.attr(nodes, "faceVertexCounts"))
            .unwrap_or_default()
            .iter()
            .map(|&c| c.max(0.0) as u32)
            .collect();
        let indices: Vec<u32> = nums(self.attr(nodes, "faceVertexIndices"))
            .unwrap_or_default()
            .iter()
            .map(|&c| if c < 0.0 { u32::MAX } else { c as u32 })
            .collect();
        if counts.is_empty() || pts.len() < 3 {
            return Ok(());
        }
        let positions: Vec<[f32; 3]> = pts
            .chunks_exact(3)
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect();
        let corners: usize = counts.iter().map(|&c| c as usize).sum();
        let faces = counts.len();
        let nv = positions.len();
        let material = bound.and_then(|m| self.material(m, b));
        let uv_name = bound.and_then(|m| self.uv_name(m));
        let left =
            self.attr(nodes, "orientation").as_ref().and_then(V::as_str) == Some("leftHanded");
        // Normals: the primvar wins over the attribute.
        let normals = match self.primvar(nodes, "normals") {
            Some(p) => Some(p),
            None => self.attr(nodes, "normals").map(|value| Primvar {
                value,
                interpolation: self
                    .prop_field(nodes, "normals", "interpolation")
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "vertex".into()),
                indices: None,
            }),
        };
        let normal_vals = normals
            .as_ref()
            .and_then(|p| expand3(p, nv, corners, faces));
        let uv = [
            uv_name.as_deref(),
            Some("st"),
            Some("st0"),
            Some("UVMap"),
            Some("uv"),
        ]
        .into_iter()
        .flatten()
        .find_map(|n| self.primvar(nodes, n));
        let uv_vals = uv.as_ref().and_then(|p| expand2(p, nv, corners, faces));
        let colour = self.primvar(nodes, "displayColor");
        let opacity = self.primvar(nodes, "displayOpacity");
        let mut mat = material;
        let mut colour_vals: Option<(Kind, Vec<[u8; 4]>)> = None;
        if let Some(c) = &colour {
            let rgb = expand3(c, nv, corners, faces);
            match rgb {
                Some((Kind::Constant, v)) if material.is_none() => {
                    let a = opacity
                        .as_ref()
                        .and_then(|o| o.value.nums().and_then(|d| d.first().copied()))
                        .unwrap_or(1.0) as f32;
                    let m = Material {
                        name: String::new(),
                        base_color: [v[0][0], v[0][1], v[0][2], a],
                        texture: None,
                    };
                    mat = Some(b.add_material(m));
                }
                Some((k, v)) if material.is_none() && k != Kind::Constant => {
                    colour_vals = Some((
                        k,
                        v.iter()
                            .map(|c| {
                                [
                                    linear_to_srgb8(c[0]),
                                    linear_to_srgb8(c[1]),
                                    linear_to_srgb8(c[2]),
                                    255,
                                ]
                            })
                            .collect(),
                    ))
                }
                _ => {}
            }
        }
        // Material subsets.
        let mut face_mat: Option<Vec<Option<u32>>> = None;
        let mesh_nodes = self.prim(path);
        for c in self.children(&mesh_nodes) {
            let cp = child_path(path, &c);
            let cn = self.prim(&cp);
            if self.field(&cn, "typeName").and_then(V::as_str) != Some("GeomSubset") {
                continue;
            }
            let family = self.attr(&cn, "familyName");
            if family
                .as_ref()
                .and_then(V::as_str)
                .is_some_and(|f| f != "materialBind")
            {
                continue;
            }
            let Some(target) = self
                .targets(&cn, "material:binding", "targetPaths")
                .into_iter()
                .next()
            else {
                continue;
            };
            let m = self.material(&target, b);
            let fm = face_mat.get_or_insert_with(|| vec![None; faces]);
            for f in nums(self.attr(&cn, "indices")).unwrap_or_default() {
                if let Some(slot) = fm.get_mut(f as usize) {
                    *slot = m;
                }
            }
        }
        let flip_v: Option<(Kind, Vec<[f32; 2]>)> =
            uv_vals.map(|(k, v)| (k, v.into_iter().map(|t| [t[0], 1.0 - t[1]]).collect()));
        let mut p = Poly::new(&positions, &counts, &indices);
        p.normals = attr_of(&normal_vals);
        p.uvs = attr_of(&flip_v);
        p.colors = attr_of(&colour_vals);
        p.material = mat;
        p.face_materials = face_mat.as_deref();
        p.clockwise = left;
        if self
            .attr(nodes, "subdivisionScheme")
            .as_ref()
            .and_then(V::as_str)
            .is_some_and(|s| s != "none")
        {
            b.note_once("subdivision surfaces are shown as their control cage");
        }
        emit(b, &p, &affine(world))
    }

    fn implicit(
        &mut self,
        kind: &str,
        nodes: &[Node],
        world: &M4,
        bound: Option<&str>,
        b: &mut Builder,
    ) -> Result<()> {
        let num =
            |s: &Self, n: &str, d: f64| s.attr(nodes, n).and_then(|v| v.as_f64()).unwrap_or(d);
        let axis = match self.attr(nodes, "axis").as_ref().and_then(V::as_str) {
            Some("X") => 0,
            Some("Y") => 1,
            _ => 2,
        };
        let (pos, counts, idx) = match kind {
            "Cube" => shapes::cube(num(self, "size", 2.0)),
            "Sphere" => shapes::sphere(num(self, "radius", 1.0)),
            "Cylinder" => shapes::cylinder(
                num(self, "radius", 1.0),
                num(self, "radius", 1.0),
                num(self, "height", 2.0),
                axis,
            ),
            "Cone" => shapes::cylinder(
                num(self, "radius", 1.0),
                0.0,
                num(self, "height", 2.0),
                axis,
            ),
            "Capsule" => shapes::capsule(num(self, "radius", 0.5), num(self, "height", 1.0), axis),
            _ => shapes::plane(num(self, "width", 2.0), num(self, "length", 2.0), axis),
        };
        let mut mat = bound.and_then(|m| self.material(m, b));
        if mat.is_none()
            && let Some(c) = self
                .primvar(nodes, "displayColor")
                .and_then(|p| p.value.nums().map(|n| n.into_owned()))
            && c.len() >= 3
        {
            mat = Some(b.add_material(Material {
                name: String::new(),
                base_color: [c[0] as f32, c[1] as f32, c[2] as f32, 1.0],
                texture: None,
            }));
        }
        let mut p = Poly::new(&pos, &counts, &idx);
        p.material = mat;
        emit(b, &p, &affine(world))
    }
}

enum Input {
    Value(V),
    Shader(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Constant,
    Vertex,
    Corner,
    Face,
}

fn attr_of<T: Copy>(v: &Option<(Kind, Vec<T>)>) -> Attr<'_, T> {
    match v {
        None => Attr::None,
        Some((Kind::Constant, d)) => d.first().map_or(Attr::None, |c| Attr::Constant(*c)),
        Some((Kind::Vertex, d)) => Attr::Vertex(d),
        Some((Kind::Corner, d)) => Attr::Corner(d),
        Some((Kind::Face, d)) => Attr::Face(d),
    }
}

/// A primvar's values of `n` components each, indices applied, with where they belong;
/// `None` when the count does not fit the interpolation.
fn expand<const N: usize>(
    p: &Primvar,
    vertices: usize,
    corners: usize,
    faces: usize,
) -> Option<(Kind, Vec<[f32; N]>)> {
    let d = p.value.nums()?;
    let vals: Vec<[f32; N]> = d
        .chunks_exact(N)
        .map(|c| std::array::from_fn(|k| c[k] as f32))
        .collect();
    let vals = match &p.indices {
        Some(ix) => ix
            .iter()
            .map(|&i| vals.get(i as usize).copied().unwrap_or([0.0; N]))
            .collect(),
        None => vals,
    };
    let (kind, want) = match p.interpolation.as_str() {
        "constant" => (Kind::Constant, 1),
        "uniform" => (Kind::Face, faces),
        "vertex" | "varying" => (Kind::Vertex, vertices),
        "faceVarying" => (Kind::Corner, corners),
        _ => return None,
    };
    (vals.len() >= want && want > 0).then_some((kind, vals))
}

fn expand3(p: &Primvar, v: usize, c: usize, f: usize) -> Option<(Kind, Vec<[f32; 3]>)> {
    expand::<3>(p, v, c, f)
}

fn expand2(p: &Primvar, v: usize, c: usize, f: usize) -> Option<(Kind, Vec<[f32; 2]>)> {
    expand::<2>(p, v, c, f)
}

fn mime_of(name: &str) -> Option<&'static str> {
    let l = name.to_ascii_lowercase();
    Some(if l.ends_with(".png") {
        "image/png"
    } else if l.ends_with(".jpg") || l.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        return None;
    })
}

/// Tessellations of the implicit shapes, z up before the axis swap.
mod shapes {
    type Out = (Vec<[f32; 3]>, Vec<u32>, Vec<u32>);

    /// `p` with its z axis turned onto `axis`.
    fn orient(p: [f64; 3], axis: usize) -> [f32; 3] {
        let [x, y, z] = p;
        let q = match axis {
            0 => [z, x, y],
            1 => [y, z, x],
            _ => [x, y, z],
        };
        q.map(|v| v as f32)
    }

    pub fn cube(size: f64) -> Out {
        let h = size / 2.0;
        let pos: Vec<[f32; 3]> = (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { -h } else { h },
                    if i & 2 == 0 { -h } else { h },
                    if i & 4 == 0 { -h } else { h },
                ]
                .map(|v| v as f32)
            })
            .collect();
        let idx = vec![
            0, 2, 3, 1, 4, 5, 7, 6, 0, 1, 5, 4, 2, 6, 7, 3, 0, 4, 6, 2, 1, 3, 7, 5,
        ];
        (pos, vec![4; 6], idx)
    }

    const SEG: usize = 32;
    const RINGS: usize = 16;

    pub fn sphere(r: f64) -> Out {
        let mut pos = vec![[0.0, 0.0, -r as f32]];
        for i in 1..RINGS {
            let t = std::f64::consts::PI * i as f64 / RINGS as f64;
            let (s, c) = t.sin_cos();
            for j in 0..SEG {
                let a = std::f64::consts::TAU * j as f64 / SEG as f64;
                pos.push([
                    (r * s * a.cos()) as f32,
                    (r * s * a.sin()) as f32,
                    (-r * c) as f32,
                ]);
            }
        }
        pos.push([0.0, 0.0, r as f32]);
        let top = (pos.len() - 1) as u32;
        let ring = |i: usize, j: usize| (1 + (i - 1) * SEG + j % SEG) as u32;
        let (mut counts, mut idx) = (Vec::new(), Vec::new());
        for j in 0..SEG {
            counts.push(3);
            idx.extend([0, ring(1, j + 1), ring(1, j)]);
        }
        for i in 1..RINGS - 1 {
            for j in 0..SEG {
                counts.push(4);
                idx.extend([
                    ring(i, j),
                    ring(i, j + 1),
                    ring(i + 1, j + 1),
                    ring(i + 1, j),
                ]);
            }
        }
        for j in 0..SEG {
            counts.push(3);
            idx.extend([top, ring(RINGS - 1, j), ring(RINGS - 1, j + 1)]);
        }
        (pos, counts, idx)
    }

    /// A cylinder or cone frustum: bottom radius `r0`, top `r1`, centred on the origin.
    pub fn cylinder(r0: f64, r1: f64, height: f64, axis: usize) -> Out {
        let h = height / 2.0;
        let mut pos = Vec::new();
        for (r, z) in [(r0, -h), (r1, h)] {
            for j in 0..SEG {
                let a = std::f64::consts::TAU * j as f64 / SEG as f64;
                pos.push(orient([r * a.cos(), r * a.sin(), z], axis));
            }
        }
        pos.push(orient([0.0, 0.0, -h], axis));
        pos.push(orient([0.0, 0.0, h], axis));
        let (b, t) = ((2 * SEG) as u32, (2 * SEG + 1) as u32);
        let (mut counts, mut idx) = (Vec::new(), Vec::new());
        for j in 0..SEG {
            let (a, n) = (j as u32, ((j + 1) % SEG) as u32);
            let s = SEG as u32;
            counts.push(4);
            idx.extend([a, n, s + n, s + a]);
            counts.push(3);
            idx.extend([b, n, a]);
            if r1 > 0.0 {
                counts.push(3);
                idx.extend([t, s + a, s + n]);
            }
        }
        (pos, counts, idx)
    }

    pub fn capsule(r: f64, height: f64, axis: usize) -> Out {
        // A cylinder of `height` with hemispheres of radius `r` at its ends.
        let h = height / 2.0;
        let half = RINGS / 2;
        let mut pos = vec![orient([0.0, 0.0, -h - r], axis)];
        let mut rings = Vec::new();
        for i in 1..=half {
            let t = std::f64::consts::FRAC_PI_2 * i as f64 / half as f64;
            rings.push((r * t.sin(), -h - r * t.cos()));
        }
        for i in 0..half {
            let t = std::f64::consts::FRAC_PI_2 * (half - i) as f64 / half as f64;
            rings.push((r * t.sin(), h + r * t.cos()));
        }
        for &(rr, z) in &rings {
            for j in 0..SEG {
                let a = std::f64::consts::TAU * j as f64 / SEG as f64;
                pos.push(orient([rr * a.cos(), rr * a.sin(), z], axis));
            }
        }
        pos.push(orient([0.0, 0.0, h + r], axis));
        let top = (pos.len() - 1) as u32;
        let n = rings.len();
        let ring = |i: usize, j: usize| (1 + i * SEG + j % SEG) as u32;
        let (mut counts, mut idx) = (Vec::new(), Vec::new());
        for j in 0..SEG {
            counts.push(3);
            idx.extend([0, ring(0, j + 1), ring(0, j)]);
        }
        for i in 0..n - 1 {
            for j in 0..SEG {
                counts.push(4);
                idx.extend([
                    ring(i, j),
                    ring(i, j + 1),
                    ring(i + 1, j + 1),
                    ring(i + 1, j),
                ]);
            }
        }
        for j in 0..SEG {
            counts.push(3);
            idx.extend([top, ring(n - 1, j), ring(n - 1, j + 1)]);
        }
        (pos, counts, idx)
    }

    pub fn plane(w: f64, l: f64, axis: usize) -> Out {
        let (a, b) = (w / 2.0, l / 2.0);
        let pos = [[-a, -b, 0.0], [a, -b, 0.0], [a, b, 0.0], [-a, b, 0.0]]
            .map(|p| orient(p, axis))
            .to_vec();
        (pos, vec![4], vec![0, 1, 2, 3])
    }
}

/// The unit of `meters_per_unit`, when it is a common one.
pub(crate) fn unit_of(m: f64) -> Unit {
    let close = |v: f64| (m - v).abs() <= v * 1e-6;
    if close(0.01) {
        Unit::Centimetre
    } else if close(1.0) {
        Unit::Metre
    } else if close(0.001) {
        Unit::Millimetre
    } else if close(0.0254) {
        Unit::Inch
    } else if close(0.3048) {
        Unit::Foot
    } else if close(1e-6) {
        Unit::Micron
    } else {
        Unit::Unknown
    }
}

/// The up axis a layer states (Y when it says none, USD's fallback).
pub(crate) fn up_of(l: &Layer) -> Up {
    match l.meta("upAxis").and_then(V::as_str) {
        Some("Z") => Up::Z,
        _ => Up::Y,
    }
}
