//! glTF 2.0: `.gltf` (JSON with buffers embedded as `data:` URIs or in files beside it) and
//! `.glb` (the binary container: a JSON chunk and a BIN chunk). The default scene's node
//! tree is walked with its transforms; triangle primitives (lists, strips, fans) give
//! positions, normals, `TEXCOORD_0` and `COLOR_0`; materials give `baseColorFactor` and the
//! base-colour texture. Points and lines are skipped, morph targets and skins are shown in
//! their rest pose. Meshopt-compressed buffer views (`EXT_meshopt_compression`,
//! `KHR_meshopt_compression`, [`crate::meshopt`]) are decoded up front; Draco is refused with a
//! clear reason.

use serde_json::Value;

use crate::builder::{Affine, Builder, apply3, linear_to_srgb8};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Unit, Up, bad};

const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;
/// Elements an accessor without a buffer view (all zeros) may declare, at most.
const MAX_ZERO_ELEMENTS: usize = 1 << 16;
/// Nodes visited (or waiting to be) per scene, at most.
const MAX_NODE_VISITS: usize = 1_000_000;

pub(crate) fn read_glb(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let (json, bin) = glb_chunks(bytes)?;
    read_doc(json, bin, resolver, limits, &Affine::IDENTITY, None)
}

/// The JSON and BIN chunks of a GLB.
fn glb_chunks(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>)> {
    let u32_at = |o: usize| -> Option<u32> {
        bytes
            .get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    if bytes.len() < 20 || !bytes.starts_with(b"glTF") {
        return bad("not a GLB file");
    }
    let version = u32_at(4).unwrap_or(0);
    if version != 2 {
        return Err(Error::Unsupported(format!("glTF version {version}")));
    }
    let total = (u32_at(8).unwrap_or(0) as usize).min(bytes.len());
    let mut off = 12;
    let mut json = None;
    let mut bin = None;
    while off + 8 <= total {
        let len = u32_at(off).unwrap_or(0) as usize;
        let kind = u32_at(off + 4).unwrap_or(0);
        let start = off + 8;
        let Some(data) = start.checked_add(len).and_then(|end| bytes.get(start..end)) else {
            return bad("GLB chunk runs past the end of the file");
        };
        match kind {
            JSON_CHUNK if json.is_none() => json = Some(data),
            BIN_CHUNK if bin.is_none() => bin = Some(data),
            _ => {}
        }
        // Chunks are 4-byte aligned; some writers leave out the padding, so a chunk header
        // right after the data wins over the aligned position.
        let next = start + len;
        let unpadded = !next.is_multiple_of(4)
            && matches!(u32_at(next + 4), Some(JSON_CHUNK | BIN_CHUNK))
            && u32_at(next).is_some_and(|l| next + 8 + l as usize <= total);
        off = if unpadded { next } else { next.div_ceil(4) * 4 };
    }
    let json = json.ok_or_else(|| Error::Malformed("GLB without a JSON chunk".into()))?;
    Ok((json, bin))
}

pub(crate) fn read_gltf(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    read_doc(bytes, None, resolver, limits, &Affine::IDENTITY, None)
}

/// The point primitives (mode 0) of a glTF scene, with node transforms applied.
#[derive(Clone, Debug, Default)]
pub struct GltfPoints {
    pub positions: Vec<[f32; 3]>,
    /// sRGB RGBA per point, when the primitives have `COLOR_0`.
    pub colors: Option<Vec<[u8; 4]>>,
}

/// A glTF scene as tile content: its triangles (`None` when it has none) and its points.
#[derive(Clone, Debug, Default)]
pub struct GltfScene {
    pub mesh: Option<Mesh>,
    pub points: GltfPoints,
}

/// Reads a glTF (`glb`: the binary container, else JSON) with every node under `root`, a
/// row-major 3 × 4 affine applied in double precision before positions become `f32` (so a
/// far-away placement can be brought near the origin without losing precision). Point
/// primitives are kept; the mesh says `up` Z when `root` is not the identity.
pub fn read_scene(
    bytes: &[u8],
    glb: bool,
    root: [[f64; 4]; 3],
    resolver: &dyn Resolver,
    limits: &Limits,
) -> Result<GltfScene> {
    let (json, bin) = if glb {
        glb_chunks(bytes)?
    } else {
        (bytes, None)
    };
    let mut points = GltfPoints::default();
    let mesh = match read_doc(
        json,
        bin,
        resolver,
        limits,
        &Affine(root),
        Some(&mut points),
    ) {
        Ok(m) => Some(m),
        Err(Error::Empty) => None,
        Err(e) => return Err(e),
    };
    if mesh.is_none() && points.positions.is_empty() {
        return Err(Error::Empty);
    }
    Ok(GltfScene { mesh, points })
}

/// A field as an index.
fn idx(v: &Value, key: &str) -> Option<usize> {
    v.get(key)?.as_u64().map(|i| i as usize)
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

struct Doc<'a> {
    json: &'a Value,
    buffers: Vec<std::result::Result<Vec<u8>, String>>,
    glb_bin: Option<&'a [u8]>,
    limits: Limits,
    /// Meshopt-compressed views, decoded: (bytes, stride) by view index.
    decoded: Vec<Option<Decoded>>,
}

type Decoded = std::result::Result<(Vec<u8>, Option<usize>), String>;

fn read_doc(
    json: &[u8],
    glb_bin: Option<&[u8]>,
    resolver: &dyn Resolver,
    limits: &Limits,
    root: &Affine,
    points: Option<&mut GltfPoints>,
) -> Result<Mesh> {
    let doc: Value =
        serde_json::from_slice(json).map_err(|e| Error::Malformed(format!("glTF JSON: {e}")))?;
    let version = doc
        .pointer("/asset/version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !version.starts_with('2') {
        return Err(Error::Unsupported(format!(
            "glTF version {}",
            if version.is_empty() {
                "unknown"
            } else {
                version
            }
        )));
    }
    let mut b = Builder::new(limits);
    b.unit = Unit::Metre;
    b.up = Up::Y;
    b.generator = doc
        .pointer("/asset/generator")
        .and_then(Value::as_str)
        .map(str::to_owned);
    for ext in arr(&doc, "extensionsRequired") {
        let e = ext.as_str().unwrap_or("");
        match e {
            "KHR_draco_mesh_compression" => {
                return Err(Error::Unsupported(format!("{e} (compressed geometry)")));
            }
            "EXT_meshopt_compression" | "KHR_meshopt_compression" => {}
            "KHR_mesh_quantization" | "KHR_texture_transform" | "KHR_materials_unlit" => {}
            _ => b.note(format!("required extension {e} not applied")),
        }
    }
    // Buffers, read up front; a failure only fails the primitives that use it.
    let mut buffers = Vec::new();
    let mut side_total = 0usize;
    for (i, buf) in arr(&doc, "buffers").iter().enumerate() {
        // A meshopt fallback buffer is for readers without meshopt: only compressed views
        // name it, and they are decoded from their own source. Not read (nor fetched), and
        // nothing to note about it.
        let fallback = ["KHR_meshopt_compression", "EXT_meshopt_compression"]
            .iter()
            .any(|e| {
                buf.pointer(&format!("/extensions/{e}/fallback"))
                    .and_then(Value::as_bool)
                    == Some(true)
            });
        if fallback {
            buffers.push(Err("meshopt fallback buffer".to_string()));
            continue;
        }
        let r = match buf.get("uri").and_then(Value::as_str) {
            None if i == 0 => glb_bin
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "no binary chunk".to_string()),
            None => Err("buffer without data".to_string()),
            Some(uri) => load_uri(uri, resolver, limits.max_side_file),
        };
        if let Ok(d) = &r {
            side_total += d.len();
            if side_total > limits.max_unpacked {
                return Err(Error::TooLarge("glTF buffers".into()));
            }
            if let Some(n) = idx(buf, "byteLength")
                && d.len() < n
            {
                b.note(format!("buffer {i} is shorter than it says"));
            }
        }
        if let Err(e) = &r {
            b.note(format!("buffer {i}: {e}"));
        }
        buffers.push(r);
    }
    let mut d = Doc {
        json: &doc,
        buffers,
        glb_bin,
        limits: *limits,
        decoded: Vec::new(),
    };
    d.decoded = decode_meshopt(&d, &mut b)?;
    let scene = idx(&doc, "scene")
        .or_else(|| (!arr(&doc, "scenes").is_empty()).then_some(0))
        .and_then(|s| arr(&doc, "scenes").get(s));
    b.title = scene
        .and_then(|s| s.get("name"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let nodes = arr(&doc, "nodes");
    let roots: Vec<usize> = match scene {
        Some(s) => arr(s, "nodes")
            .iter()
            .filter_map(|n| n.as_u64().map(|n| n as usize))
            .collect(),
        None => {
            // No scene: every node that is nobody's child.
            let mut child = vec![false; nodes.len()];
            for n in nodes {
                for c in arr(n, "children") {
                    if let Some(c) = c.as_u64().and_then(|c| child.get_mut(c as usize)) {
                        *c = true;
                    }
                }
            }
            (0..nodes.len()).filter(|&i| !child[i]).collect()
        }
    };
    let mut state = State {
        materials: vec![None; arr(&doc, "materials").len()],
        images: vec![None; arr(&doc, "images").len()],
        any_normals: false,
        missing_normals: false,
        skipped_modes: false,
        points,
    };
    let mut drawn = false;
    if nodes.is_empty() && scene.is_none() {
        // Meshes but no nodes: each mesh once, untransformed.
        for m in 0..arr(&doc, "meshes").len() {
            mesh(&d, &mut b, &mut state, resolver, m, root)?;
            drawn = true;
        }
    }
    // Depth-first with an explicit stack; visits are capped so cycles cannot loop.
    let mut stack: Vec<(usize, Affine, u32)> =
        roots.into_iter().rev().map(|r| (r, *root, 0)).collect();
    let mut visits = 0usize;
    while let Some((n, parent, depth)) = stack.pop() {
        visits += 1;
        if visits > MAX_NODE_VISITS || depth > 128 {
            b.note_once("node tree too deep or cyclic; the rest is not shown");
            break;
        }
        let Some(node) = nodes.get(n) else { continue };
        let world = parent.then_after(&local(node));
        if let Some(m) = idx(node, "mesh") {
            mesh(&d, &mut b, &mut state, resolver, m, &world)?;
            drawn = true;
        }
        // Children waiting count against the visits too: a node listing itself a million
        // times would otherwise fill the stack faster than visits empty it.
        let kids = arr(node, "children");
        if visits + stack.len() + kids.len() > MAX_NODE_VISITS {
            b.note_once("node tree too deep or cyclic; the rest is not shown");
            break;
        }
        for c in kids.iter().rev() {
            if let Some(c) = c.as_u64() {
                stack.push((c as usize, world, depth + 1));
            }
        }
    }
    let _ = drawn;
    if let Some(p) = state.points.as_deref_mut()
        && let Some(c) = &mut p.colors
    {
        c.resize(p.positions.len(), [255; 4]);
    }
    if !root.is_identity() {
        b.up = Up::Z;
    }
    if state.skipped_modes {
        b.note("points and lines are not shown");
    }
    if state.missing_normals && !state.any_normals {
        b.flat = true;
    }
    b.finish()
}

struct State<'p> {
    /// Where point primitives go (`None`: they are skipped).
    points: Option<&'p mut GltfPoints>,
    /// glTF material → ours.
    materials: Vec<Option<u32>>,
    /// glTF image → ours (`Some(None)`: tried, over the cap).
    images: Vec<Option<Option<u32>>>,
    any_normals: bool,
    missing_normals: bool,
    skipped_modes: bool,
}

/// A node's own transform: `matrix` (column-major) or translation · rotation · scale.
fn local(node: &Value) -> Affine {
    let nums = |k: &str| -> Option<Vec<f64>> {
        let a = node.get(k)?.as_array()?;
        a.iter().map(Value::as_f64).collect()
    };
    if let Some(m) = nums("matrix").filter(|m| m.len() == 16) {
        return Affine([
            [m[0], m[4], m[8], m[12]],
            [m[1], m[5], m[9], m[13]],
            [m[2], m[6], m[10], m[14]],
        ]);
    }
    let t = nums("translation")
        .filter(|v| v.len() == 3)
        .unwrap_or(vec![0.0; 3]);
    let q = nums("rotation")
        .filter(|v| v.len() == 4)
        .unwrap_or(vec![0.0, 0.0, 0.0, 1.0]);
    let s = nums("scale")
        .filter(|v| v.len() == 3)
        .unwrap_or(vec![1.0; 3]);
    let ql = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    let [x, y, z, w] = if ql > 0.0 {
        [q[0] / ql, q[1] / ql, q[2] / ql, q[3] / ql]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    let r = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - z * w),
            2.0 * (x * z + y * w),
        ],
        [
            2.0 * (x * y + z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - x * w),
        ],
        [
            2.0 * (x * z - y * w),
            2.0 * (y * z + x * w),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ];
    Affine([0, 1, 2].map(|i| [r[i][0] * s[0], r[i][1] * s[1], r[i][2] * s[2], t[i]]))
}

fn mesh(
    d: &Doc,
    b: &mut Builder,
    st: &mut State,
    resolver: &dyn Resolver,
    m: usize,
    xf: &Affine,
) -> Result<()> {
    let Some(mesh) = arr(d.json, "meshes").get(m) else {
        b.note_once("a node names a mesh that does not exist");
        return Ok(());
    };
    let flip = xf.det() < 0.0;
    let nm = xf.normal_matrix();
    let identity = xf.is_identity();
    for prim in arr(mesh, "primitives") {
        let mode = prim.get("mode").and_then(Value::as_u64).unwrap_or(4);
        if mode == 0 && st.points.is_some() {
            match point_primitive(d, st, prim, xf) {
                Ok(()) => {}
                Err(Error::TooLarge(e)) => return Err(Error::TooLarge(e)),
                Err(e) => b.note(format!("points of mesh {m} skipped: {e}")),
            }
            continue;
        }
        if !matches!(mode, 4..=6) {
            st.skipped_modes = true;
            continue;
        }
        let position_view = prim
            .pointer("/attributes/POSITION")
            .and_then(Value::as_u64)
            .and_then(|a| arr(d.json, "accessors").get(a as usize))
            .and_then(|a| a.get("bufferView"));
        if prim
            .pointer("/extensions/KHR_draco_mesh_compression")
            .is_some()
            && position_view.is_none()
        {
            b.note_once("Draco-compressed primitives are not shown");
            continue;
        }
        match primitive(d, b, st, resolver, prim, mode, xf, &nm, identity, flip) {
            Ok(()) => {}
            Err(Error::TooLarge(e)) => return Err(Error::TooLarge(e)),
            Err(e) => b.note(format!("a primitive of mesh {m} skipped: {e}")),
        }
    }
    Ok(())
}

/// A point primitive's positions and colours, into `st.points`.
fn point_primitive(d: &Doc, st: &mut State, prim: &Value, xf: &Affine) -> Result<()> {
    if prim
        .pointer("/extensions/KHR_draco_mesh_compression")
        .is_some()
    {
        return Err(Error::Unsupported("Draco-compressed points".into()));
    }
    let attrs = prim
        .get("attributes")
        .ok_or_else(|| Error::Malformed("no attributes".into()))?;
    let pos_i = idx(attrs, "POSITION").ok_or_else(|| Error::Malformed("no POSITION".into()))?;
    let pos = accessor(d, pos_i, 3)?;
    let n = pos.count;
    let colors = match idx(attrs, "COLOR_0") {
        Some(i) => Some(accessor(d, i, 0)?).filter(|a| a.count == n && a.comps >= 3),
        None => None,
    };
    let Some(out) = st.points.as_deref_mut() else {
        return Ok(());
    };
    if out.positions.len() + n > d.limits.max_vertices {
        return Err(Error::TooLarge("points".into()));
    }
    let before = out.positions.len();
    out.positions.extend(
        (0..n).map(|i| xf.point([pos.data[i * 3], pos.data[i * 3 + 1], pos.data[i * 3 + 2]])),
    );
    if let Some(a) = colors {
        let c = out.colors.get_or_insert_with(Vec::new);
        c.resize(before, [255; 4]);
        let k = a.comps;
        c.extend((0..n).map(|i| {
            let ch = |j: usize| linear_to_srgb8(a.data[i * k + j]);
            let alpha = if k == 4 {
                (a.data[i * k + 3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            } else {
                255
            };
            [ch(0), ch(1), ch(2), alpha]
        }));
    } else if let Some(c) = &mut out.colors {
        c.resize(out.positions.len(), [255; 4]);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn primitive(
    d: &Doc,
    b: &mut Builder,
    st: &mut State,
    resolver: &dyn Resolver,
    prim: &Value,
    mode: u64,
    xf: &Affine,
    nm: &[[f64; 3]; 3],
    identity: bool,
    flip: bool,
) -> Result<()> {
    let attrs = prim
        .get("attributes")
        .ok_or_else(|| Error::Malformed("no attributes".into()))?;
    let pos_i = idx(attrs, "POSITION").ok_or_else(|| Error::Malformed("no POSITION".into()))?;
    let pos = accessor(d, pos_i, 3)?;
    let n = pos.count;
    let normals = match idx(attrs, "NORMAL") {
        Some(i) => Some(accessor(d, i, 3)?).filter(|a| a.count == n),
        None => None,
    };
    let uvs = match idx(attrs, "TEXCOORD_0") {
        Some(i) => Some(accessor(d, i, 2)?).filter(|a| a.count == n),
        None => None,
    };
    let colors = match idx(attrs, "COLOR_0") {
        Some(i) => Some(accessor(d, i, 0)?).filter(|a| a.count == n && a.comps >= 3),
        None => None,
    };
    let indices: Vec<u32> = match idx(prim, "indices") {
        Some(i) => index_accessor(d, i)?,
        None => (0..n as u32).collect(),
    };
    if normals.is_some() {
        st.any_normals = true;
    } else {
        st.missing_normals = true;
    }
    let material = match idx(prim, "material") {
        Some(mi) => material(d, b, st, resolver, mi),
        None => None,
    };
    b.reserve_vertices(n)?;
    let base = b.vertex_count();
    for i in 0..n {
        let p = [pos.data[i * 3], pos.data[i * 3 + 1], pos.data[i * 3 + 2]];
        let p = if identity { p } else { xf.point(p) };
        let nrm = normals.as_ref().map(|a| {
            let v = [a.data[i * 3], a.data[i * 3 + 1], a.data[i * 3 + 2]];
            if identity { v } else { apply3(nm, v) }
        });
        let c = colors.as_ref().map(|a| {
            let k = a.comps;
            let ch = |j: usize| linear_to_srgb8(a.data[i * k + j]);
            let alpha = if k == 4 {
                (a.data[i * k + 3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            } else {
                255
            };
            [ch(0), ch(1), ch(2), alpha]
        });
        let t = uvs.as_ref().map(|a| [a.data[i * 2], a.data[i * 2 + 1]]);
        b.vertex(p, nrm, c, t)?;
    }
    b.material(material);
    let mut tri = |a: u32, c1: u32, c2: u32| -> Result<()> {
        if a as usize >= n || c1 as usize >= n || c2 as usize >= n {
            return Ok(());
        }
        let (c1, c2) = if flip { (c2, c1) } else { (c1, c2) };
        b.triangle(base + a, base + c1, base + c2)
    };
    match mode {
        4 => {
            for t in indices.chunks_exact(3) {
                tri(t[0], t[1], t[2])?;
            }
        }
        5 => {
            for k in 2..indices.len() {
                let (a, c1, c2) = (indices[k - 2], indices[k - 1], indices[k]);
                if k % 2 == 0 {
                    tri(a, c1, c2)?;
                } else {
                    tri(c1, a, c2)?;
                }
            }
        }
        _ => {
            for k in 2..indices.len() {
                tri(indices[0], indices[k - 1], indices[k])?;
            }
        }
    }
    Ok(())
}

fn material(
    d: &Doc,
    b: &mut Builder,
    st: &mut State,
    resolver: &dyn Resolver,
    mi: usize,
) -> Option<u32> {
    if let Some(Some(m)) = st.materials.get(mi) {
        return Some(*m);
    }
    let mat = arr(d.json, "materials").get(mi)?;
    let pbr = mat.get("pbrMetallicRoughness");
    let mut base_color = [1.0f32; 4];
    if let Some(f) = pbr
        .and_then(|p| p.get("baseColorFactor"))
        .and_then(Value::as_array)
        && f.len() == 4
    {
        for (o, v) in base_color.iter_mut().zip(f) {
            *o = v.as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32;
        }
    }
    let mut texture = None;
    if let Some(info) = pbr.and_then(|p| p.get("baseColorTexture")) {
        if idx(info, "texCoord").unwrap_or(0) != 0 {
            b.note_once("base-colour textures on a second texture coordinate set are not shown");
        } else if let Some(tex) = idx(info, "index").and_then(|t| arr(d.json, "textures").get(t)) {
            // `source`, or an extension's (WebP, KTX2) when the plain one is absent.
            let src = idx(tex, "source").or_else(|| {
                tex.get("extensions")?
                    .as_object()?
                    .values()
                    .find_map(|e| idx(e, "source"))
            });
            if let Some(src) = src {
                texture = image(d, b, st, resolver, src);
            }
        }
    }
    let m = b.add_material(Material {
        name: mat
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        base_color,
        texture,
    });
    if let Some(slot) = st.materials.get_mut(mi) {
        *slot = Some(m);
    }
    Some(m)
}

fn image(
    d: &Doc,
    b: &mut Builder,
    st: &mut State,
    resolver: &dyn Resolver,
    src: usize,
) -> Option<u32> {
    if let Some(Some(done)) = st.images.get(src) {
        return *done;
    }
    let img = arr(d.json, "images").get(src)?;
    let mime = img
        .get("mimeType")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let name = img
        .get("uri")
        .and_then(Value::as_str)
        .map(|u| {
            if u.starts_with("data:") {
                "embedded image"
            } else {
                u
            }
        })
        .unwrap_or("embedded image")
        .to_owned();
    let data = if let Some(uri) = img.get("uri").and_then(Value::as_str) {
        load_uri(uri, resolver, d.limits.max_side_file)
    } else if let Some(v) = idx(img, "bufferView") {
        view_bytes(d, v).map(<[u8]>::to_vec)
    } else {
        Err("no data".into())
    };
    let data = match data {
        Ok(bytes) => ImageData::Bytes { bytes, mime },
        Err(reason) => ImageData::Missing { name, reason },
    };
    let ours = b.add_image(data);
    if let Some(slot) = st.images.get_mut(src) {
        *slot = Some(ours);
    }
    ours
}

/// Decodes every meshopt-compressed buffer view, within the unpacked-bytes cap. A view that
/// fails only fails the primitives that use it.
fn decode_meshopt(d: &Doc<'_>, b: &mut Builder) -> Result<Vec<Option<Decoded>>> {
    use crate::meshopt::{Filter, Mode};
    let views = arr(d.json, "bufferViews");
    let mut out = Vec::with_capacity(views.len());
    let mut total = 0usize;
    for (i, view) in views.iter().enumerate() {
        let ext = view
            .pointer("/extensions/KHR_meshopt_compression")
            .or_else(|| view.pointer("/extensions/EXT_meshopt_compression"));
        let Some(ext) = ext else {
            out.push(None);
            continue;
        };
        let count = idx(ext, "count").unwrap_or(0);
        let stride = idx(ext, "byteStride").unwrap_or(0);
        total = total.saturating_add(count.saturating_mul(stride));
        if total > d.limits.max_unpacked {
            return Err(Error::TooLarge("meshopt-compressed buffer views".into()));
        }
        let r = (|| {
            let mode = ext
                .get("mode")
                .and_then(Value::as_str)
                .and_then(Mode::parse)
                .ok_or("unknown meshopt mode")?;
            let filter = match ext.get("filter").and_then(Value::as_str) {
                None => Filter::None,
                Some(f) => Filter::parse(f).ok_or("unknown meshopt filter")?,
            };
            let src = view_range(d, ext)?;
            let bytes = crate::meshopt::decode(src, count, stride, mode, filter)?;
            let own = idx(view, "byteStride").filter(|&s| s > 0);
            let stride = match mode {
                Mode::Attributes => own.or(Some(stride)),
                _ => own,
            };
            Ok((bytes, stride))
        })();
        if let Err(e) = &r {
            b.note(format!("compressed buffer view {i}: {e}"));
        }
        out.push(Some(r));
    }
    Ok(out)
}

/// A buffer view's bytes and stride.
fn view<'a>(d: &'a Doc<'_>, v: usize) -> std::result::Result<(&'a [u8], Option<usize>), String> {
    if let Some(Some(r)) = d.decoded.get(v) {
        return match r {
            Ok((bytes, stride)) => Ok((bytes, *stride)),
            Err(e) => Err(e.clone()),
        };
    }
    let view = arr(d.json, "bufferViews")
        .get(v)
        .ok_or("no such buffer view")?;
    let bytes = view_range(d, view)?;
    Ok((bytes, idx(view, "byteStride").filter(|&s| s > 0)))
}

/// The bytes a view (or a meshopt extension object) names: `buffer`, `byteOffset`,
/// `byteLength`.
fn view_range<'a>(d: &'a Doc<'_>, view: &Value) -> std::result::Result<&'a [u8], String> {
    let buf = idx(view, "buffer").ok_or("buffer view without a buffer")?;
    let data: &[u8] = match d.buffers.get(buf) {
        Some(Ok(b)) => b,
        Some(Err(e)) => return Err(e.clone()),
        None => match (buf, d.glb_bin) {
            (0, Some(bin)) => bin,
            _ => return Err("no such buffer".into()),
        },
    };
    let off = idx(view, "byteOffset").unwrap_or(0);
    let len = idx(view, "byteLength").ok_or("buffer view without a length")?;
    let bytes = off
        .checked_add(len)
        .and_then(|end| data.get(off..end))
        .ok_or("buffer view past the end of its buffer")?;
    Ok(bytes)
}

fn view_bytes<'a>(d: &'a Doc<'_>, v: usize) -> std::result::Result<&'a [u8], String> {
    view(d, v).map(|(b, _)| b)
}

struct Accessor {
    count: usize,
    comps: usize,
    /// `count × comps` values, normalised integers mapped to 0..=1 or -1..=1.
    data: Vec<f32>,
}

/// Reads accessor `i` to floats. `want`: the components expected (0 = any of 3 or 4).
fn accessor(d: &Doc, i: usize, want: usize) -> Result<Accessor> {
    let a = arr(d.json, "accessors")
        .get(i)
        .ok_or_else(|| Error::Malformed(format!("no accessor {i}")))?;
    let count = idx(a, "count").ok_or_else(|| Error::Malformed("accessor without count".into()))?;
    let comps = match a.get("type").and_then(Value::as_str).unwrap_or("") {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        t => return Err(Error::Malformed(format!("accessor type {t}"))),
    };
    if want != 0 && comps != want || want == 0 && comps < 3 {
        return Err(Error::Malformed(format!(
            "accessor with {comps} components"
        )));
    }
    let ct = a.get("componentType").and_then(Value::as_u64).unwrap_or(0);
    let size = match ct {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err(Error::Malformed(format!("component type {ct}"))),
    };
    let normalized = a
        .get("normalized")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        // Colours stored as integers are always normalised.
        || want == 0;
    if count > d.limits.max_vertices.max(d.limits.max_triangles * 3) {
        return Err(Error::TooLarge(format!("accessor of {count} elements")));
    }
    let elem = size * comps;
    let source = match idx(a, "bufferView") {
        Some(v) => Some(strided(d, a, v, count, elem)?),
        None => None,
    };
    // Without a buffer view the accessor is zeros (plus its sparse values): its size is
    // proven by no data, so it must stay near what the sparse part supplies.
    if source.is_none() {
        let sparse = a.get("sparse").and_then(|s| idx(s, "count")).unwrap_or(0);
        if count > MAX_ZERO_ELEMENTS.max(sparse.saturating_mul(16)) {
            return Err(Error::TooLarge(format!(
                "accessor of {count} elements without data"
            )));
        }
    }
    let mut data = vec![0f32; count * comps];
    if let Some((bytes, off, stride)) = source {
        for (e, out) in data.chunks_exact_mut(comps).enumerate() {
            let at = off + e * stride;
            for (c, o) in out.iter_mut().enumerate() {
                *o = component(&bytes[at + c * size..], ct, normalized);
            }
        }
    }
    if let Some(sparse) = a.get("sparse") {
        let n = idx(sparse, "count").unwrap_or(0);
        if n > count {
            return bad("sparse count larger than the accessor");
        }
        let si = sparse
            .get("indices")
            .ok_or_else(|| Error::Malformed("sparse".into()))?;
        let sv = sparse
            .get("values")
            .ok_or_else(|| Error::Malformed("sparse".into()))?;
        let ict = si.get("componentType").and_then(Value::as_u64).unwrap_or(0);
        let isz = match ict {
            5121 => 1,
            5123 => 2,
            5125 => 4,
            _ => return bad("sparse index type"),
        };
        let ib =
            view_bytes(d, idx(si, "bufferView").unwrap_or(usize::MAX)).map_err(Error::Malformed)?;
        let ib = ib.get(idx(si, "byteOffset").unwrap_or(0)..).unwrap_or(&[]);
        let vb =
            view_bytes(d, idx(sv, "bufferView").unwrap_or(usize::MAX)).map_err(Error::Malformed)?;
        let vb = vb.get(idx(sv, "byteOffset").unwrap_or(0)..).unwrap_or(&[]);
        if ib.len() < n * isz || vb.len() < n * elem {
            return bad("sparse data past the end of its buffer view");
        }
        for k in 0..n {
            let at = component(&ib[k * isz..], ict, false) as usize;
            if at >= count {
                return bad("sparse index out of range");
            }
            for c in 0..comps {
                data[at * comps + c] = component(&vb[k * elem + c * size..], ct, normalized);
            }
        }
    }
    Ok(Accessor { count, comps, data })
}

/// An accessor's view bytes, offset and stride, checked to hold `count` elements.
fn strided<'a>(
    d: &'a Doc<'_>,
    a: &Value,
    v: usize,
    count: usize,
    elem: usize,
) -> Result<(&'a [u8], usize, usize)> {
    let (bytes, stride) = view(d, v).map_err(Error::Malformed)?;
    let stride = stride.unwrap_or(elem);
    if stride < elem {
        return bad("buffer view stride shorter than its element");
    }
    let off = idx(a, "byteOffset").unwrap_or(0);
    if count > 0 {
        let need = stride
            .checked_mul(count - 1)
            .and_then(|x| x.checked_add(off))
            .and_then(|x| x.checked_add(elem));
        if need.is_none_or(|n| n > bytes.len()) {
            return bad("accessor past the end of its buffer view");
        }
    }
    Ok((bytes, off, stride))
}

/// Reads an index accessor exactly (unsigned byte, short or int scalars).
fn index_accessor(d: &Doc, i: usize) -> Result<Vec<u32>> {
    let a = arr(d.json, "accessors")
        .get(i)
        .ok_or_else(|| Error::Malformed(format!("no accessor {i}")))?;
    let count = idx(a, "count").ok_or_else(|| Error::Malformed("accessor without count".into()))?;
    if a.get("type").and_then(Value::as_str) != Some("SCALAR") {
        return bad("index accessor is not scalar");
    }
    if a.get("sparse").is_some() {
        return bad("sparse index accessor");
    }
    let ct = a.get("componentType").and_then(Value::as_u64).unwrap_or(0);
    let size = match ct {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        _ => return bad(format!("index component type {ct}")),
    };
    if count > d.limits.max_triangles.saturating_mul(3) {
        return Err(Error::TooLarge(format!("{count} indices")));
    }
    let Some(v) = idx(a, "bufferView") else {
        if count > MAX_ZERO_ELEMENTS {
            return Err(Error::TooLarge(format!("{count} indices without data")));
        }
        return Ok(vec![0; count]);
    };
    let (bytes, off, stride) = strided(d, a, v, count, size)?;
    Ok((0..count)
        .map(|e| {
            let b = &bytes[off + e * stride..];
            match size {
                1 => u32::from(b[0]),
                2 => u32::from(u16::from_le_bytes([b[0], b[1]])),
                _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            }
        })
        .collect())
}

/// One component at the start of `b` (checked long enough by the caller).
fn component(b: &[u8], ct: u64, normalized: bool) -> f32 {
    match ct {
        5120 => {
            let v = f32::from(b[0] as i8);
            if normalized { (v / 127.0).max(-1.0) } else { v }
        }
        5121 => {
            let v = f32::from(b[0]);
            if normalized { v / 255.0 } else { v }
        }
        5122 => {
            let v = f32::from(i16::from_le_bytes([b[0], b[1]]));
            if normalized {
                (v / 32767.0).max(-1.0)
            } else {
                v
            }
        }
        5123 => {
            let v = f32::from(u16::from_le_bytes([b[0], b[1]]));
            if normalized { v / 65535.0 } else { v }
        }
        5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
        _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
    }
}

/// A buffer or image URI: `data:` (base64 or percent-encoded) or a relative file beside
/// the model, never a network URL.
fn load_uri(
    uri: &str,
    resolver: &dyn Resolver,
    max: usize,
) -> std::result::Result<Vec<u8>, String> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (meta, data) = rest.split_once(',').ok_or("malformed data URI")?;
        if data.len() / 4 * 3 > max {
            return Err("too large".into());
        }
        return if meta.ends_with(";base64") {
            base64(data).ok_or_else(|| "malformed base64".into())
        } else {
            Ok(percent_decode(data))
        };
    }
    let name = String::from_utf8(percent_decode(uri)).map_err(|_| "malformed URI".to_string())?;
    resolver.read(&name, max)
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (
                b.get(i + 1).and_then(|&c| hex(c)),
                b.get(i + 2).and_then(|&c| hex(c)),
            )
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

/// Standard or URL-safe base64, padding optional, white space ignored.
pub(crate) fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}
