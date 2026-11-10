//! COLLADA (`.dae`, and `.zae` packages): the visual scene's node tree with its transforms
//! (`matrix`, `translate`, `rotate`, `scale`; `instance_node` too), geometries instanced in
//! it (`triangles`, `polylist`, `polygons`, `trifans`, `tristrips`; positions, normals,
//! texture coordinates and colours with their offsets and sets), materials bound per
//! primitive (an effect's diffuse colour or texture, through samplers and surfaces to an
//! image), the asset's up axis, unit, title and authoring tool. Lines are skipped.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use crate::builder::{Affine, Builder};
use crate::poly::{Attr, Poly, emit};
use crate::xml::{Node, parse};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Unit, Up, bad};

const MAX_NODES: usize = 20_000_000;

struct Doc<'a> {
    ids: HashMap<&'a str, &'a Node>,
    resolver: &'a dyn Resolver,
    materials: HashMap<String, Option<u32>>,
    images: HashMap<String, Option<u32>>,
    /// Float arrays already parsed, by source id.
    floats: HashMap<String, std::rc::Rc<(Vec<f32>, usize)>>,
    /// Nodes visited so far ([`MAX_NODE_VISITS`]).
    visits: usize,
}

/// Node visits per file, at most: `instance_node` may name a node instancing the next one
/// twice (or itself), which would otherwise visit exponentially many nodes.
const MAX_NODE_VISITS: usize = 1 << 20;

fn index<'a>(n: &'a Node, ids: &mut HashMap<&'a str, &'a Node>) {
    if let Some(id) = n.attr("id") {
        ids.entry(id).or_insert(n);
    }
    for c in &n.children {
        index(c, ids);
    }
}

fn nums<T: std::str::FromStr>(s: &str) -> Vec<T> {
    s.split_ascii_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect()
}

fn floats(s: &str) -> Vec<f32> {
    s.split_ascii_whitespace()
        .map(|t| {
            t.parse::<f32>().unwrap_or_else(|_| {
                // Exporters that wrote numbers in their locale ("21,92").
                if t.contains(',') && !t.contains('.') {
                    t.replace(',', ".").parse().unwrap_or(f32::NAN)
                } else {
                    f32::NAN
                }
            })
        })
        .collect()
}

/// A URI's path, percent-decoded, without a leading `./`.
fn uri_path(u: &str) -> String {
    let mut out = Vec::new();
    let b = u.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&u[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    let s = String::from_utf8_lossy(&out).into_owned();
    s.strip_prefix("./").map(str::to_owned).unwrap_or(s)
}

impl<'a> Doc<'a> {
    fn by_url(&self, url: &str) -> Option<&'a Node> {
        self.ids.get(url.strip_prefix('#')?).copied()
    }

    /// A source's floats and stride.
    fn source(&mut self, url: &str) -> Option<std::rc::Rc<(Vec<f32>, usize)>> {
        let id = url.strip_prefix('#')?;
        if let Some(f) = self.floats.get(id) {
            return Some(f.clone());
        }
        let n = *self.ids.get(id)?;
        let arr = n.child("float_array")?;
        let data = floats(arr.text_trim());
        let stride = n
            .child("technique_common")
            .and_then(|t| t.child("accessor"))
            .and_then(|a| a.attr("stride"))
            .and_then(|s| s.parse().ok())
            .unwrap_or(1usize)
            .max(1);
        let r = std::rc::Rc::new((data, stride));
        self.floats.insert(id.to_owned(), r.clone());
        Some(r)
    }

    fn image(&mut self, id: &str, b: &mut Builder) -> Option<u32> {
        if let Some(i) = self.images.get(id) {
            return *i;
        }
        let img = self.ids.get(id).copied()?;
        let init = img.child("init_from")?;
        let path = match init.child("ref") {
            Some(r) => r.text_trim().to_owned(),
            None => init.text_trim().to_owned(),
        };
        let path = uri_path(&path);
        let data = match self.resolver.read(&path, b.limits.max_side_file) {
            Ok(bytes) => ImageData::Bytes { bytes, mime: None },
            Err(reason) => ImageData::Missing {
                name: path.clone(),
                reason,
            },
        };
        let i = b.add_image(data);
        self.images.insert(id.to_owned(), i);
        i
    }

    /// The builder material of a `<material>` (by url).
    fn material(&mut self, url: &str, b: &mut Builder) -> Option<u32> {
        if let Some(m) = self.materials.get(url) {
            return *m;
        }
        let mat = self.by_url(url)?;
        let mut out = Material {
            name: mat.attr("name").or(mat.attr("id")).unwrap_or("").to_owned(),
            ..Material::default()
        };
        if let Some(effect) = mat
            .child("instance_effect")
            .and_then(|i| i.attr("url"))
            .and_then(|u| self.by_url(u))
            && let Some(profile) = effect.child("profile_COMMON")
            && let Some(tech) = profile.child("technique")
        {
            let shading = ["phong", "lambert", "blinn", "constant"]
                .iter()
                .find_map(|s| tech.child(s));
            let diffuse = shading.and_then(|s| s.child("diffuse").or(s.child("emission")));
            if let Some(d) = diffuse {
                if let Some(c) = d.child("color") {
                    let v = floats(c.text_trim());
                    if v.len() >= 3 && v.iter().all(|x| x.is_finite()) {
                        out.base_color = [v[0], v[1], v[2], v.get(3).copied().unwrap_or(1.0)];
                    }
                } else if let Some(t) = d.child("texture").and_then(|t| t.attr("texture")) {
                    // texture → sampler2D newparam → its source surface → init_from image
                    // (1.4), or straight to an image (1.5 and lax exporters).
                    let param = |sid: &str| {
                        profile
                            .all("newparam")
                            .chain(tech.all("newparam"))
                            .find(|p| p.attr("sid") == Some(sid))
                    };
                    let image = match param(t).and_then(|p| p.child("sampler2D")) {
                        Some(s) => match s.child("source") {
                            Some(src) => param(src.text_trim())
                                .and_then(|p| p.child("surface"))
                                .and_then(|s| s.child("init_from"))
                                .map(|i| i.text_trim().to_owned()),
                            None => s
                                .child("instance_image")
                                .and_then(|i| i.attr("url"))
                                .map(|u| u.trim_start_matches('#').to_owned()),
                        },
                        None => Some(t.to_owned()),
                    };
                    if let Some(img) = image {
                        out.texture = self.image(&img, b);
                    }
                }
            }
        }
        let i = b.add_material(out);
        self.materials.insert(url.to_owned(), Some(i));
        Some(i)
    }
}

/// A node's local transform from its transform children, in document order.
fn node_transform(n: &Node) -> [[f64; 4]; 4] {
    let mut m = IDENT;
    for c in &n.children {
        let v: Vec<f64> = floats(c.text_trim()).into_iter().map(f64::from).collect();
        let t = match c.name.as_str() {
            "matrix" if v.len() >= 16 => {
                let mut t = IDENT;
                for r in 0..4 {
                    for k in 0..4 {
                        t[r][k] = v[r * 4 + k];
                    }
                }
                t
            }
            "translate" if v.len() >= 3 => {
                let mut t = IDENT;
                t[0][3] = v[0];
                t[1][3] = v[1];
                t[2][3] = v[2];
                t
            }
            "scale" if v.len() >= 3 => {
                let mut t = IDENT;
                t[0][0] = v[0];
                t[1][1] = v[1];
                t[2][2] = v[2];
                t
            }
            "rotate" if v.len() >= 4 => axis_angle([v[0], v[1], v[2]], v[3]),
            _ => continue,
        };
        m = mul(&m, &t);
    }
    m
}

const IDENT: [[f64; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn mul(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            m[r][c] = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    m
}

/// A rotation of `deg` degrees about `axis`.
pub(crate) fn axis_angle(axis: [f64; 3], deg: f64) -> [[f64; 4]; 4] {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if l.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) || !deg.is_finite() {
        return IDENT;
    }
    let [x, y, z] = axis.map(|v| v / l);
    let (s, c) = deg.to_radians().sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0.0],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0.0],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// The inputs of a primitive: (semantic, source url, offset, set).
fn inputs(p: &Node) -> Vec<(String, String, usize, u32)> {
    p.all("input")
        .filter_map(|i| {
            Some((
                i.attr("semantic")?.to_owned(),
                i.attr("source")?.to_owned(),
                i.attr("offset").and_then(|o| o.parse().ok()).unwrap_or(0),
                i.attr("set").and_then(|o| o.parse().ok()).unwrap_or(0),
            ))
        })
        .collect()
}

impl Doc<'_> {
    /// Adds a `<geometry>`'s mesh primitives under `xf`, materials bound by `bind`
    /// (symbol → material url).
    fn geometry(
        &mut self,
        g: &Node,
        xf: &Affine,
        bind: &HashMap<String, String>,
        b: &mut Builder,
    ) -> Result<()> {
        let Some(mesh) = g.child("mesh") else {
            if g.child("convex_mesh").is_some() || g.child("spline").is_some() {
                b.note_once("COLLADA splines and convex meshes are not shown");
            }
            return Ok(());
        };
        // <vertices>: the POSITION source (and maybe NORMAL, TEXCOORD).
        let verts = mesh.child("vertices");
        let vid = verts.and_then(|v| v.attr("id")).unwrap_or("").to_owned();
        let vin = verts.map(inputs).unwrap_or_default();
        let pos_url = vin.iter().find(|i| i.0 == "POSITION").map(|i| i.1.clone());
        let Some(pos) = pos_url.and_then(|u| self.source(&u)) else {
            return Ok(());
        };
        let positions: Vec<[f32; 3]> = pos
            .0
            .chunks(pos.1)
            .filter(|c| c.len() >= 3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        for prim in &mesh.children {
            let kind = prim.name.as_str();
            if !matches!(
                kind,
                "triangles" | "polylist" | "polygons" | "trifans" | "tristrips"
            ) {
                if matches!(kind, "lines" | "linestrips") {
                    b.note_once("COLLADA lines are not shown");
                }
                continue;
            }
            let ins = inputs(prim);
            let stride = ins.iter().map(|i| i.2 + 1).max().unwrap_or(1);
            let vertex_off = ins
                .iter()
                .find(|i| i.0 == "VERTEX" && i.1.trim_start_matches('#') == vid)
                .or_else(|| ins.iter().find(|i| i.0 == "VERTEX"))
                .map(|i| i.2);
            let Some(vertex_off) = vertex_off else {
                continue;
            };
            // Faces as corner lists of index tuples.
            let ps: Vec<Vec<u32>> = prim.all("p").map(|p| nums(p.text_trim())).collect();
            let mut counts: Vec<u32> = Vec::new();
            let mut corner_rows: Vec<usize> = Vec::new(); // start of each corner's tuple in flat
            let mut flat: Vec<u32> = Vec::new();
            match kind {
                "triangles" | "polylist" => {
                    flat = ps.into_iter().flatten().collect();
                    let n = flat.len() / stride;
                    let vc: Vec<u32> = if kind == "polylist" {
                        prim.child("vcount")
                            .map(|v| nums(v.text_trim()))
                            .unwrap_or_default()
                    } else {
                        vec![3; n / 3]
                    };
                    let mut at = 0usize;
                    for c in vc {
                        if at + c as usize > n {
                            break;
                        }
                        counts.push(c);
                        corner_rows.extend((at..at + c as usize).map(|k| k * stride));
                        at += c as usize;
                    }
                }
                "polygons" => {
                    for p in ps {
                        let base = flat.len();
                        let n = p.len() / stride;
                        flat.extend_from_slice(&p);
                        counts.push(n as u32);
                        corner_rows.extend((0..n).map(|k| base + k * stride));
                    }
                }
                _ => {
                    // Fans and strips: one per <p>, as triangles.
                    for p in ps {
                        let base = flat.len();
                        let n = p.len() / stride;
                        flat.extend_from_slice(&p);
                        for k in 2..n {
                            let (a, c, d) = if kind == "trifans" {
                                (0, k - 1, k)
                            } else if k % 2 == 0 {
                                (k - 2, k - 1, k)
                            } else {
                                (k - 1, k - 2, k)
                            };
                            counts.push(3);
                            corner_rows.extend([a, c, d].map(|j| base + j * stride));
                        }
                    }
                }
            }
            if flat.len() < stride {
                continue;
            }
            let at = |row: usize, off: usize| flat.get(row + off).copied().unwrap_or(u32::MAX);
            let indices: Vec<u32> = corner_rows.iter().map(|&r| at(r, vertex_off)).collect();
            // Corner attributes: from the primitive's inputs, or the <vertices> element's
            // (by the position index).
            let corner_attr = |doc: &mut Doc, sem: &str, comps: usize| -> Option<Vec<f32>> {
                let inp = ins
                    .iter()
                    .filter(|i| i.0 == sem)
                    .min_by_key(|i| i.3)
                    .map(|i| (i.1.clone(), i.2))
                    .or_else(|| {
                        vin.iter()
                            .filter(|i| i.0 == sem)
                            .min_by_key(|i| i.3)
                            .map(|i| (i.1.clone(), vertex_off))
                    })?;
                let src = doc.source(&inp.0)?;
                let mut out = Vec::with_capacity(corner_rows.len() * comps);
                for &r in &corner_rows {
                    let k = at(r, inp.1) as usize;
                    for c in 0..comps {
                        let v = src.0.get(k * src.1 + c).copied().unwrap_or(if c == 3 {
                            1.0
                        } else {
                            0.0
                        });
                        out.push(if c >= src.1 { 1.0 } else { v });
                    }
                }
                Some(out)
            };
            let normals: Option<Vec<[f32; 3]>> = corner_attr(self, "NORMAL", 3)
                .map(|v| v.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect());
            let uvs: Option<Vec<[f32; 2]>> = corner_attr(self, "TEXCOORD", 2)
                .map(|v| v.chunks_exact(2).map(|c| [c[0], 1.0 - c[1]]).collect());
            let colors: Option<Vec<[u8; 4]>> = corner_attr(self, "COLOR", 4).map(|v| {
                v.chunks_exact(4)
                    .map(|c| {
                        [
                            crate::builder::linear_to_srgb8(c[0]),
                            crate::builder::linear_to_srgb8(c[1]),
                            crate::builder::linear_to_srgb8(c[2]),
                            (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
                        ]
                    })
                    .collect()
            });
            let material = prim
                .attr("material")
                .and_then(|s| bind.get(s).cloned().or_else(|| Some(format!("#{s}"))))
                .and_then(|u| self.material(&u, b));
            let mut p = Poly::new(&positions, &counts, &indices);
            if let Some(n) = &normals {
                p.normals = Attr::Corner(n);
            }
            if let Some(t) = &uvs {
                p.uvs = Attr::Corner(t);
            }
            if let Some(c) = &colors {
                p.colors = Attr::Corner(c);
            }
            p.material = material;
            emit(b, &p, xf)?;
        }
        Ok(())
    }

    fn node(
        &mut self,
        n: &Node,
        parent: &[[f64; 4]; 4],
        b: &mut Builder,
        depth: usize,
    ) -> Result<()> {
        if depth > 128 {
            b.note_once("node tree too deep; the rest is not shown");
            return Ok(());
        }
        self.visits += 1;
        if self.visits > MAX_NODE_VISITS {
            b.note_once("nodes instanced too many times; the rest is not shown");
            return Ok(());
        }
        let m = mul(parent, &node_transform(n));
        let xf = Affine([m[0], m[1], m[2]]);
        if n.children
            .iter()
            .any(|c| c.name == "lookat" || c.name == "skew")
        {
            b.note_once("COLLADA lookat and skew transforms are not applied");
        }
        for c in &n.children {
            match c.name.as_str() {
                "instance_geometry" => {
                    let Some(g) = c.attr("url").and_then(|u| self.by_url(u)) else {
                        continue;
                    };
                    let mut bind = HashMap::new();
                    let mut im = Vec::new();
                    c.descendants("instance_material", &mut im);
                    for i in im {
                        if let (Some(s), Some(t)) = (i.attr("symbol"), i.attr("target")) {
                            bind.insert(s.to_owned(), t.to_owned());
                        }
                    }
                    self.geometry(g, &xf, &bind, b)?;
                }
                "instance_controller" => {
                    // A skin or morph: its base geometry (the bind pose).
                    let mut url = c.attr("url").map(str::to_owned);
                    let mut shape = IDENT;
                    for _ in 0..8 {
                        let Some(n) = url.as_deref().and_then(|u| self.by_url(u)) else {
                            break;
                        };
                        if n.name == "geometry" {
                            break;
                        }
                        let ctl = n.child("skin").or(n.child("morph"));
                        if let Some(m) = ctl
                            .and_then(|s| s.child("bind_shape_matrix"))
                            .map(|m| nums::<f64>(m.text_trim()))
                            .filter(|v| v.len() >= 16)
                        {
                            let mut t = IDENT;
                            for r in 0..4 {
                                for k in 0..4 {
                                    t[r][k] = m[r * 4 + k];
                                }
                            }
                            shape = mul(&t, &shape);
                        }
                        url = ctl.and_then(|s| s.attr("source")).map(str::to_owned);
                    }
                    let Some(g) = url.as_deref().and_then(|u| self.by_url(u)) else {
                        continue;
                    };
                    if g.name != "geometry" {
                        continue;
                    }
                    let mut bind = HashMap::new();
                    let mut im = Vec::new();
                    c.descendants("instance_material", &mut im);
                    for i in im {
                        if let (Some(s), Some(t)) = (i.attr("symbol"), i.attr("target")) {
                            bind.insert(s.to_owned(), t.to_owned());
                        }
                    }
                    let w = mul(&m, &shape);
                    b.note_once("skinned meshes are shown in their bind pose");
                    self.geometry(g, &Affine([w[0], w[1], w[2]]), &bind, b)?;
                }
                "instance_node" => {
                    if let Some(t) = c.attr("url").and_then(|u| self.by_url(u)) {
                        self.node(t, &m, b, depth + 1)?;
                    }
                }
                "node" => self.node(c, &m, b, depth + 1)?,
                _ => {}
            }
        }
        Ok(())
    }
}

pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let root = parse(bytes, MAX_NODES)?;
    if root.name != "COLLADA" {
        return bad("not a COLLADA document");
    }
    let mut ids = HashMap::new();
    index(&root, &mut ids);
    let mut b = Builder::new(limits);
    let asset = root.child("asset");
    let up = asset
        .and_then(|a| a.child("up_axis"))
        .map(|u| u.text_trim().to_owned())
        .unwrap_or_else(|| "Y_UP".into());
    let mut top = IDENT;
    b.up = match up.as_str() {
        "Z_UP" => Up::Z,
        "X_UP" => {
            // X up: turned so x becomes y.
            top = axis_angle([0.0, 0.0, 1.0], 90.0);
            Up::Y
        }
        _ => Up::Y,
    };
    if let Some(u) = asset
        .and_then(|a| a.child("unit"))
        .and_then(|u| u.attr("meter"))
        .and_then(|m| m.trim().parse::<f64>().ok())
    {
        b.unit = crate::usd::unit_of(u);
        if b.unit == Unit::Unknown {
            b.note(format!("unit: {u} m"));
        }
    }
    b.title = asset
        .and_then(|a| a.child("title"))
        .map(|t| t.text_trim().to_owned())
        .filter(|t| !t.is_empty());
    let mut tools = Vec::new();
    root.descendants("authoring_tool", &mut tools);
    b.generator = tools
        .first()
        .map(|t| t.text_trim().to_owned())
        .filter(|t| !t.is_empty());
    let mut doc = Doc {
        ids,
        resolver,
        materials: HashMap::new(),
        images: HashMap::new(),
        floats: HashMap::new(),
        visits: 0,
    };
    let scene = root
        .child("scene")
        .and_then(|s| s.child("instance_visual_scene"))
        .and_then(|i| i.attr("url"))
        .and_then(|u| doc.by_url(u))
        .or_else(|| {
            root.child("library_visual_scenes")
                .and_then(|l| l.child("visual_scene"))
        });
    match scene {
        Some(s) => {
            for n in s.all("node") {
                doc.node(n, &top, &mut b, 0)?;
            }
        }
        None => {
            // No scene: every geometry where it stands.
            let xf = Affine([top[0], top[1], top[2]]);
            if let Some(lib) = root.child("library_geometries") {
                for g in lib.all("geometry") {
                    doc.geometry(g, &xf, &HashMap::new(), &mut b)?;
                }
            }
        }
    }
    b.finish()
}

/// Files of a `.zae` package.
struct ZipFiles(HashMap<String, Vec<u8>>);

impl Resolver for ZipFiles {
    fn read(&self, name: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        let key = crate::resolve::safe_relative(name)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .ok_or("not a path inside the package")?;
        match self.0.get(&key) {
            Some(b) if b.len() <= max => Ok(b.clone()),
            Some(_) => Err("too large".into()),
            None => Err("not in the package".into()),
        }
    }
}

/// Reads a `.zae` package: its manifest's root document, else its first `.dae`.
pub(crate) fn read_zae(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Malformed(format!("ZAE package: {e}")))?;
    let mut files = HashMap::new();
    let mut budget = limits.max_unpacked;
    let mut first_dae = None;
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| Error::Malformed(format!("ZAE package: {e}")))?;
        if !f.is_file() {
            continue;
        }
        let name = f
            .name()
            .map_err(|e| Error::Malformed(format!("ZAE package: {e}")))?
            .trim_start_matches('/')
            .replace('\\', "/");
        let mut data = Vec::new();
        (&mut f)
            .take(budget as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| Error::Malformed(format!("{name}: {e}")))?;
        if data.len() > budget {
            return Err(Error::TooLarge("ZAE package unpacks too large".into()));
        }
        budget -= data.len();
        if first_dae.is_none() && name.to_ascii_lowercase().ends_with(".dae") {
            first_dae = Some(name.clone());
        }
        files.insert(name, data);
    }
    let root = files
        .get("manifest.xml")
        .and_then(|m| parse(m, 1000).ok())
        .and_then(|m| m.find("dae_root").map(|r| uri_path(r.text_trim())))
        .filter(|r| files.contains_key(r))
        .or(first_dae)
        .ok_or_else(|| Error::Malformed("ZAE package without a COLLADA document".into()))?;
    let doc = files[&root].clone();
    let dir = root.rsplit_once('/').map_or("", |(d, _)| d).to_owned();
    // Side files relative to the root document's folder inside the package.
    let files = if dir.is_empty() {
        files
    } else {
        files
            .into_iter()
            .filter_map(|(k, v)| {
                k.strip_prefix(&format!("{dir}/"))
                    .map(|r| (r.to_owned(), v))
            })
            .collect()
    };
    read(&doc, &ZipFiles(files), limits)
}
