//! FBX, binary, version 7 (7.0 to 7.7), geometry only: the node records (32- or 64-bit
//! offsets, zlib arrays), `Geometry` meshes (vertices, polygon vertex indices with the
//! negative end marker, normals, UVs and material slots by layer element with their mapping
//! and reference types), `Model` transforms (local translation, rotation with its order,
//! pre- and post-rotation, pivots and offsets, scaling, geometric transforms) through the
//! connection graph, `Material` diffuse colours and textures (files or embedded `Video`
//! content), and the global settings' up axis and unit. Animation, skinning, cameras and
//! lights are not read. ASCII FBX and binary versions before 7 are refused.
//!
//! Autodesk's SDK is not used (its licence rules out shipping it); the format is read from
//! its record structure as the open readers document it.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::builder::{Affine, Builder};
use crate::poly::{Attr, Poly, emit};
use crate::{Error, ImageData, Limits, Material, Mesh, Resolver, Result, Unit, Up, bad};

const MAGIC: &[u8] = b"Kaydara FBX Binary  \0";

#[derive(Clone, Debug)]
enum P {
    I(i64),
    F(f64),
    S(String),
    Raw(Vec<u8>),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
}

impl P {
    fn int(&self) -> Option<i64> {
        match self {
            P::I(v) => Some(*v),
            P::F(v) => Some(*v as i64),
            _ => None,
        }
    }

    fn num(&self) -> Option<f64> {
        match self {
            P::I(v) => Some(*v as f64),
            P::F(v) => Some(*v),
            _ => None,
        }
    }

    fn str(&self) -> Option<&str> {
        match self {
            P::S(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Default, Debug)]
struct Node {
    name: String,
    props: Vec<P>,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, n: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == n)
    }

    fn all<'a>(&'a self, n: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == n)
    }

    fn floats(&self, n: &str) -> Option<Cow<'_, [f64]>> {
        match self.child(n)?.props.first()? {
            P::Floats(f) => Some(Cow::Borrowed(f)),
            P::Ints(i) => Some(Cow::Owned(i.iter().map(|&v| v as f64).collect())),
            _ => None,
        }
    }

    fn ints(&self, n: &str) -> Option<Cow<'_, [i64]>> {
        match self.child(n)?.props.first()? {
            P::Ints(f) => Some(Cow::Borrowed(f)),
            P::Floats(f) => Some(Cow::Owned(f.iter().map(|&v| v as i64).collect())),
            _ => None,
        }
    }

    fn str_of(&self, n: &str) -> Option<&str> {
        self.child(n)?.props.first()?.str()
    }

    /// A `Properties70` entry's values (after name, type, label, flags).
    fn prop70(&self, name: &str) -> Option<&[P]> {
        self.child("Properties70")?
            .all("P")
            .find(|p| p.props.first().and_then(P::str) == Some(name))
            .map(|p| p.props.get(4..).unwrap_or(&[]))
    }

    fn vec3(&self, name: &str) -> Option<[f64; 3]> {
        let v = self.prop70(name)?;
        Some([v.first()?.num()?, v.get(1)?.num()?, v.get(2)?.num()?])
    }
}

struct Reader<'a> {
    b: &'a [u8],
    wide: bool,
    budget: usize,
    nodes: usize,
}

fn bad_fbx<T>(m: &str) -> Result<T> {
    Err(Error::Malformed(format!("FBX: {m}")))
}

impl Reader<'_> {
    fn uint(&self, at: usize, n: usize) -> Result<u64> {
        let s = self
            .b
            .get(at..at + n)
            .ok_or_else(|| Error::Malformed("FBX: record cut short".into()))?;
        Ok(if n == 8 {
            u64::from_le_bytes(s.try_into().expect("8"))
        } else {
            u64::from(u32::from_le_bytes(s.try_into().expect("4")))
        })
    }

    /// The node record at `at` (`None` for the null record ending a list) and where the next
    /// one starts.
    fn node(&mut self, at: usize, depth: usize) -> Result<(Option<Node>, usize)> {
        if depth > 64 {
            return bad_fbx("nesting too deep");
        }
        self.nodes += 1;
        if self.nodes > 50_000_000 {
            return Err(Error::TooLarge("FBX with too many records".into()));
        }
        let w = if self.wide { 8 } else { 4 };
        let end = self.uint(at, w)? as usize;
        let nprops = self.uint(at + w, w)? as usize;
        let plen = self.uint(at + 2 * w, w)? as usize;
        let nlen = *self
            .b
            .get(at + 3 * w)
            .ok_or_else(|| Error::Malformed("FBX: record cut short".into()))?
            as usize;
        if end == 0 {
            return Ok((None, at + 3 * w + 1));
        }
        if end <= at || end > self.b.len() {
            return bad_fbx("record end outside the file");
        }
        let name_at = at + 3 * w + 1;
        let name = String::from_utf8_lossy(
            self.b
                .get(name_at..name_at + nlen)
                .ok_or_else(|| Error::Malformed("FBX: name cut short".into()))?,
        )
        .into_owned();
        let mut p = name_at + nlen;
        let props_end = p + plen;
        if props_end > end {
            return bad_fbx("properties past the record");
        }
        let mut props = Vec::with_capacity(nprops.min(64));
        for _ in 0..nprops {
            if p >= props_end {
                break;
            }
            let (v, next) = self.prop(p)?;
            props.push(v);
            p = next;
        }
        let mut children = Vec::new();
        let mut c = props_end;
        while c + 3 * w < end {
            let (n, next) = self.node(c, depth + 1)?;
            match n {
                Some(n) => children.push(n),
                None => break,
            }
            if next <= c {
                break;
            }
            c = next;
        }
        Ok((
            Some(Node {
                name,
                props,
                children,
            }),
            end,
        ))
    }

    fn prop(&mut self, at: usize) -> Result<(P, usize)> {
        let t = *self
            .b
            .get(at)
            .ok_or_else(|| Error::Malformed("FBX: property cut short".into()))?;
        let d = at + 1;
        let get = |n: usize| -> Result<&[u8]> {
            self.b
                .get(d..d + n)
                .ok_or_else(|| Error::Malformed("FBX: property cut short".into()))
        };
        Ok(match t {
            b'Y' => {
                let s = get(2)?;
                (P::I(i64::from(i16::from_le_bytes([s[0], s[1]]))), d + 2)
            }
            b'C' => (P::I(i64::from(get(1)?[0])), d + 1),
            b'I' => {
                let s = get(4)?;
                (
                    P::I(i64::from(i32::from_le_bytes(s.try_into().expect("4")))),
                    d + 4,
                )
            }
            b'F' => {
                let s = get(4)?;
                (
                    P::F(f64::from(f32::from_le_bytes(s.try_into().expect("4")))),
                    d + 4,
                )
            }
            b'D' => {
                let s = get(8)?;
                (P::F(f64::from_le_bytes(s.try_into().expect("8"))), d + 8)
            }
            b'L' => {
                let s = get(8)?;
                (P::I(i64::from_le_bytes(s.try_into().expect("8"))), d + 8)
            }
            b'S' | b'R' => {
                let n = self.uint(d, 4)? as usize;
                let s = self
                    .b
                    .get(d + 4..d + 4 + n)
                    .ok_or_else(|| Error::Malformed("FBX: string cut short".into()))?;
                let v = if t == b'S' {
                    P::S(String::from_utf8_lossy(s).into_owned())
                } else {
                    P::Raw(s.to_vec())
                };
                (v, d + 4 + n)
            }
            b'f' | b'd' | b'l' | b'i' | b'b' | b'c' => {
                let count = self.uint(d, 4)? as usize;
                let enc = self.uint(d + 4, 4)?;
                let clen = self.uint(d + 8, 4)? as usize;
                let src = self
                    .b
                    .get(d + 12..d + 12 + clen)
                    .ok_or_else(|| Error::Malformed("FBX: array cut short".into()))?;
                let size = match t {
                    b'f' | b'i' => 4,
                    b'b' | b'c' => 1,
                    _ => 8,
                };
                let bytes = count
                    .checked_mul(size)
                    .filter(|&n| n <= self.budget)
                    .ok_or_else(|| Error::TooLarge("FBX array too large".into()))?;
                self.budget -= bytes;
                let raw: std::borrow::Cow<[u8]> = match enc {
                    0 => std::borrow::Cow::Borrowed(src),
                    1 => std::borrow::Cow::Owned(
                        specular_png::inflate_zlib(src, bytes)
                            .map_err(|e| Error::Malformed(format!("FBX: zlib array: {e}")))?,
                    ),
                    e => return bad_fbx(&format!("array encoding {e}")),
                };
                if raw.len() < bytes {
                    return bad_fbx("array shorter than its count");
                }
                let raw = &raw[..bytes];
                let v = match t {
                    b'f' => P::Floats(
                        raw.chunks_exact(4)
                            .map(|c| f64::from(f32::from_le_bytes(c.try_into().expect("4"))))
                            .collect(),
                    ),
                    b'd' => P::Floats(
                        raw.chunks_exact(8)
                            .map(|c| f64::from_le_bytes(c.try_into().expect("8")))
                            .collect(),
                    ),
                    b'i' => P::Ints(
                        raw.chunks_exact(4)
                            .map(|c| i64::from(i32::from_le_bytes(c.try_into().expect("4"))))
                            .collect(),
                    ),
                    b'l' => P::Ints(
                        raw.chunks_exact(8)
                            .map(|c| i64::from_le_bytes(c.try_into().expect("8")))
                            .collect(),
                    ),
                    _ => P::Ints(raw.iter().map(|&c| i64::from(c)).collect()),
                };
                (v, d + 12 + clen)
            }
            t => return bad_fbx(&format!("property type {:?}", t as char)),
        })
    }
}

type M4 = [[f64; 4]; 4];

const IDENT: M4 = [
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

fn translate(t: [f64; 3]) -> M4 {
    let mut m = IDENT;
    m[0][3] = t[0];
    m[1][3] = t[1];
    m[2][3] = t[2];
    m
}

fn scale(s: [f64; 3]) -> M4 {
    let mut m = IDENT;
    m[0][0] = s[0];
    m[1][1] = s[1];
    m[2][2] = s[2];
    m
}

/// Euler angles (degrees) in FBX rotation order `order` (0 XYZ: X applied first … 5 ZYX).
fn euler(r: [f64; 3], order: i64) -> M4 {
    let axis = |a: usize| {
        crate::collada::axis_angle(
            [
                f64::from(u8::from(a == 0)),
                f64::from(u8::from(a == 1)),
                f64::from(u8::from(a == 2)),
            ],
            r[a],
        )
    };
    let seq: [usize; 3] = match order {
        1 => [0, 2, 1],
        2 => [1, 2, 0],
        3 => [1, 0, 2],
        4 => [2, 0, 1],
        5 => [2, 1, 0],
        _ => [0, 1, 2],
    };
    // The first in the sequence is applied first: R = R3 · R2 · R1.
    mul(&axis(seq[2]), &mul(&axis(seq[1]), &axis(seq[0])))
}

fn invert(m: &M4) -> M4 {
    let a = Affine([m[0], m[1], m[2]]);
    let det = a.det();
    if det.abs() < 1e-300 || !det.is_finite() {
        return IDENT;
    }
    let n = a.normal_matrix();
    let mut inv = IDENT;
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

/// A model's local transform, and its geometric transform (applied to its geometry only).
fn model_transform(m: &Node) -> (M4, M4) {
    let v = |n: &str, d: f64| m.vec3(n).unwrap_or([d; 3]);
    let order = m
        .prop70("RotationOrder")
        .and_then(|p| p.first())
        .and_then(P::int)
        .unwrap_or(0);
    let t = translate(v("Lcl Translation", 0.0));
    let roff = translate(v("RotationOffset", 0.0));
    let rp = translate(v("RotationPivot", 0.0));
    let pre = euler(v("PreRotation", 0.0), 0);
    let post = euler(v("PostRotation", 0.0), 0);
    let r = euler(v("Lcl Rotation", 0.0), order);
    let soff = translate(v("ScalingOffset", 0.0));
    let sp = translate(v("ScalingPivot", 0.0));
    let s = scale(v("Lcl Scaling", 1.0));
    let mut local = IDENT;
    for x in [
        t,
        roff,
        rp,
        pre,
        r,
        invert(&post),
        invert(&rp),
        soff,
        sp,
        s,
        invert(&sp),
    ] {
        local = mul(&local, &x);
    }
    let geo = mul(
        &mul(
            &translate(v("GeometricTranslation", 0.0)),
            &euler(v("GeometricRotation", 0.0), 0),
        ),
        &scale(v("GeometricScaling", 1.0)),
    );
    (local, geo)
}

pub(crate) fn read(bytes: &[u8], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    if !bytes.starts_with(MAGIC) {
        if bytes.starts_with(b"; FBX")
            || bytes.windows(13).take(4096).any(|w| w == b"FBXHeaderExte")
        {
            let top = ascii::parse(bytes, limits)?;
            let version = top
                .iter()
                .find(|n| n.name == "FBXHeaderExtension")
                .and_then(|h| h.child("FBXVersion"))
                .and_then(|v| v.props.first())
                .and_then(P::int)
                .unwrap_or(0);
            if version == 0 {
                return bad("ASCII FBX without a version in its header");
            }
            if !(7000..8000).contains(&version) {
                return Err(Error::Unsupported(format!(
                    "ASCII FBX version {}.{} (FBX 7 is read)",
                    version / 1000,
                    version % 1000 / 100
                )));
            }
            return scene(&top, resolver, limits);
        }
        return bad("not an FBX file");
    }
    if bytes.len() < 27 {
        return bad("FBX header cut short");
    }
    let version = u32::from_le_bytes([bytes[23], bytes[24], bytes[25], bytes[26]]);
    if !(7000..8000).contains(&version) {
        return Err(Error::Unsupported(format!(
            "FBX version {}.{} (binary FBX 7 is read)",
            version / 1000,
            version % 1000 / 100
        )));
    }
    let mut r = Reader {
        b: bytes,
        wide: version >= 7500,
        budget: limits.max_unpacked,
        nodes: 0,
    };
    let mut top = Vec::new();
    let mut at = 27;
    let w = if r.wide { 8 } else { 4 };
    while at + 3 * w < bytes.len() {
        let (n, next) = r.node(at, 0)?;
        match n {
            Some(n) => top.push(n),
            None => break,
        }
        at = next;
    }
    scene(&top, resolver, limits)
}

/// The model of the parsed records.
fn scene(top: &[Node], resolver: &dyn Resolver, limits: &Limits) -> Result<Mesh> {
    let find = |n: &str| top.iter().find(|t| t.name == n);
    let mut b = Builder::new(limits);
    // Global settings.
    let gs = find("GlobalSettings");
    let gi = |n: &str| {
        gs.and_then(|g| g.prop70(n))
            .and_then(|p| p.first())
            .and_then(P::num)
    };
    b.up = match gi("UpAxis") {
        Some(v) if v as i64 == 2 => Up::Z,
        _ => Up::Y,
    };
    let unit = gi("UnitScaleFactor").unwrap_or(1.0);
    b.unit = crate::usd::unit_of(unit / 100.0);
    if b.unit == Unit::Unknown {
        b.note(format!("unit: {unit} cm"));
    }
    b.generator = find("FBXHeaderExtension")
        .and_then(|h| h.child("Creator"))
        .and_then(|c| c.props.first())
        .and_then(P::str)
        .map(str::to_owned)
        .or_else(|| {
            find("Creator")
                .and_then(|c| c.props.first())
                .and_then(P::str)
                .map(str::to_owned)
        });
    let Some(objects) = find("Objects") else {
        return Err(Error::Empty);
    };
    let by_id: HashMap<i64, &Node> = objects
        .children
        .iter()
        .filter_map(|o| Some((o.props.first()?.int()?, o)))
        .collect();
    // Connections: child → parents (object-object), and (texture → material, property).
    let mut parents: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut children: HashMap<i64, Vec<(i64, Option<String>)>> = HashMap::new();
    if let Some(c) = find("Connections") {
        for e in c.all("C") {
            let (Some(kind), Some(ch), Some(pa)) = (
                e.props.first().and_then(P::str),
                e.props.get(1).and_then(P::int),
                e.props.get(2).and_then(P::int),
            ) else {
                continue;
            };
            let prop = e.props.get(3).and_then(P::str).map(str::to_owned);
            if kind == "OO" || kind == "OP" {
                parents.entry(ch).or_default().push(pa);
                children.entry(pa).or_default().push((ch, prop));
            }
        }
    }
    let class = |n: &Node| n.props.get(2).and_then(P::str).unwrap_or("").to_owned();
    let name_of = |n: &Node| {
        n.props
            .get(1)
            .and_then(P::str)
            // Binary: "name\0\x01Class"; ASCII: "Class::name".
            .map(|s| {
                let s = s.split('\0').next().unwrap_or("");
                s.split_once("::").map_or(s, |(_, n)| n).to_owned()
            })
            .unwrap_or_default()
    };
    // World transforms of models.
    let mut worlds: HashMap<i64, M4> = HashMap::new();
    fn world_of(
        id: i64,
        by_id: &HashMap<i64, &Node>,
        parents: &HashMap<i64, Vec<i64>>,
        worlds: &mut HashMap<i64, M4>,
        depth: usize,
    ) -> M4 {
        if let Some(w) = worlds.get(&id) {
            return *w;
        }
        let Some(n) = by_id.get(&id).filter(|n| n.name == "Model") else {
            return IDENT;
        };
        let (local, _) = model_transform(n);
        let parent = parents
            .get(&id)
            .and_then(|p| {
                p.iter()
                    .find(|p| by_id.get(p).is_some_and(|n| n.name == "Model"))
            })
            .copied();
        let w = match parent {
            Some(p) if depth < 128 => mul(&world_of(p, by_id, parents, worlds, depth + 1), &local),
            _ => local,
        };
        worlds.insert(id, w);
        w
    }
    let mut materials: HashMap<i64, u32> = HashMap::new();
    let mut images: HashMap<i64, Option<u32>> = HashMap::new();
    let mut emitted = 0;
    for geo in objects.all("Geometry") {
        if class(geo) != "Mesh" {
            continue;
        }
        let Some(gid) = geo.props.first().and_then(P::int) else {
            continue;
        };
        let (Some(verts), Some(pvi)) = (geo.floats("Vertices"), geo.ints("PolygonVertexIndex"))
        else {
            continue;
        };
        let positions: Vec<[f32; 3]> = verts
            .chunks_exact(3)
            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
            .collect();
        let mut counts = Vec::new();
        let mut indices = Vec::with_capacity(pvi.len());
        let mut n = 0u32;
        for &i in pvi.iter() {
            let (v, end) = if i < 0 { (!i, true) } else { (i, false) };
            indices.push(u32::try_from(v).unwrap_or(u32::MAX));
            n += 1;
            if end {
                counts.push(n);
                n = 0;
            }
        }
        let corners = indices.len();
        let nf = counts.len();
        // Layer elements: normals, UVs, material slots.
        let element =
            |name: &str, data: &str, index: &str, comps: usize| -> Option<(String, Vec<f64>)> {
                let le = geo.child(name)?;
                let d = le.floats(data)?;
                let mapping = le
                    .str_of("MappingInformationType")
                    .unwrap_or("ByPolygonVertex");
                let reference = le.str_of("ReferenceInformationType").unwrap_or("Direct");
                let ix = (reference != "Direct").then(|| le.ints(index)).flatten();
                let ix = ix.as_deref();
                let get = |k: usize| -> Vec<f64> {
                    let k = ix.map_or(Some(k), |ix| {
                        ix.get(k).and_then(|&v| usize::try_from(v).ok())
                    });
                    (0..comps)
                        .map(|c| k.and_then(|k| d.get(k * comps + c)).copied().unwrap_or(0.0))
                        .collect()
                };
                let (kind, n) = match mapping {
                    "ByPolygonVertex" => ("corner", corners),
                    "ByVertice" | "ByVertex" | "ByControlPoint" => ("vertex", positions.len()),
                    "ByPolygon" => ("face", nf),
                    "AllSame" => ("all", 1),
                    _ => return None,
                };
                Some((kind.to_owned(), (0..n).flat_map(get).collect()))
            };
        let normals = element("LayerElementNormal", "Normals", "NormalsIndex", 3);
        let normal_v: Option<(String, Vec<[f32; 3]>)> = normals.map(|(k, d)| {
            (
                k,
                d.chunks_exact(3)
                    .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
                    .collect(),
            )
        });
        let uv = element("LayerElementUV", "UV", "UVIndex", 2);
        let uv_v: Option<(String, Vec<[f32; 2]>)> = uv.map(|(k, d)| {
            (
                k,
                d.chunks_exact(2)
                    .map(|c| [c[0] as f32, 1.0 - c[1] as f32])
                    .collect(),
            )
        });
        // Materials of the model(s) this geometry hangs under, by slot.
        let models: Vec<i64> = parents
            .get(&gid)
            .map(|p| {
                p.iter()
                    .copied()
                    .filter(|p| by_id.get(p).is_some_and(|n| n.name == "Model"))
                    .collect()
            })
            .unwrap_or_default();
        let slot_mats = geo.child("LayerElementMaterial").map(|le| {
            (
                le.str_of("MappingInformationType")
                    .unwrap_or("AllSame")
                    .to_owned(),
                le.ints("Materials")
                    .map(Cow::into_owned)
                    .unwrap_or_default(),
            )
        });
        for model in models
            .iter()
            .copied()
            .chain((models.is_empty()).then_some(0))
        {
            let world = world_of(model, &by_id, &parents, &mut worlds, 0);
            let geo_xf = by_id.get(&model).map_or(IDENT, |m| model_transform(m).1);
            let xf = mul(&world, &geo_xf);
            // The model's materials, in connection order.
            let mut slots: Vec<Option<u32>> = Vec::new();
            for (c, _) in children.get(&model).cloned().unwrap_or_default() {
                let Some(m) = by_id.get(&c).filter(|n| n.name == "Material") else {
                    continue;
                };
                let idx = match materials.get(&c) {
                    Some(&i) => i,
                    None => {
                        let mut mat = Material {
                            name: name_of(m),
                            ..Material::default()
                        };
                        if let Some(d) = m.vec3("DiffuseColor").or_else(|| m.vec3("Diffuse")) {
                            let f = m
                                .prop70("DiffuseFactor")
                                .and_then(|p| p.first())
                                .and_then(P::num)
                                .unwrap_or(1.0);
                            mat.base_color[..3].copy_from_slice(&d.map(|v| (v * f) as f32));
                        }
                        if let Some(o) =
                            m.prop70("Opacity").and_then(|p| p.first()).and_then(P::num)
                        {
                            mat.base_color[3] = o.clamp(0.0, 1.0) as f32;
                        }
                        // A texture on the diffuse colour.
                        let tex = children.get(&c).and_then(|ch| {
                            ch.iter()
                                .find(|(t, p)| {
                                    by_id.get(t).is_some_and(|n| n.name == "Texture")
                                        && p.as_deref().is_none_or(|p| p.starts_with("Diffuse"))
                                })
                                .map(|(t, _)| *t)
                        });
                        if let Some(t) = tex {
                            mat.texture = *images
                                .entry(t)
                                .or_insert_with(|| texture(t, &by_id, &children, resolver, &mut b));
                        }
                        let i = b.add_material(mat);
                        materials.insert(c, i);
                        i
                    }
                };
                slots.push(Some(idx));
            }
            let face_mats: Option<Vec<Option<u32>>> = slot_mats.as_ref().map(|(mapping, ids)| {
                (0..nf)
                    .map(|f| {
                        let k = if mapping == "AllSame" { 0 } else { f };
                        ids.get(k)
                            .and_then(|&s| usize::try_from(s).ok())
                            .and_then(|s| slots.get(s).copied().flatten())
                    })
                    .collect()
            });
            let mut p = Poly::new(&positions, &counts, &indices);
            p.normals = match &normal_v {
                Some((k, v)) if k == "corner" => Attr::Corner(v),
                Some((k, v)) if k == "vertex" => Attr::Vertex(v),
                Some((k, v)) if k == "face" => Attr::Face(v),
                Some((_, v)) => v.first().map_or(Attr::None, |c| Attr::Constant(*c)),
                None => Attr::None,
            };
            p.uvs = match &uv_v {
                Some((k, v)) if k == "corner" => Attr::Corner(v),
                Some((k, v)) if k == "vertex" => Attr::Vertex(v),
                _ => Attr::None,
            };
            p.face_materials = face_mats.as_deref();
            p.material = slots.first().copied().flatten();
            emit(&mut b, &p, &Affine([xf[0], xf[1], xf[2]]))?;
            emitted += 1;
        }
    }
    if emitted == 0 {
        return Err(Error::Empty);
    }
    b.finish()
}

/// A texture's image: embedded `Video` content, else the file it names.
fn texture(
    t: i64,
    by_id: &HashMap<i64, &Node>,
    children: &HashMap<i64, Vec<(i64, Option<String>)>>,
    resolver: &dyn Resolver,
    b: &mut Builder,
) -> Option<u32> {
    let tex = by_id.get(&t)?;
    let embedded = children.get(&t).and_then(|ch| {
        ch.iter().find_map(|(v, _)| {
            let n = by_id.get(v).filter(|n| n.name == "Video")?;
            match n.child("Content")?.props.first()? {
                P::Raw(r) if !r.is_empty() => Some(r.clone()),
                _ => None,
            }
        })
    });
    if let Some(bytes) = embedded {
        return b.add_image(ImageData::Bytes { bytes, mime: None });
    }
    let rel = tex.str_of("RelativeFilename").unwrap_or("").to_owned();
    let abs = tex.str_of("FileName").unwrap_or("").to_owned();
    let base = abs.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
    let max = b.limits.max_side_file;
    let mut reason = "no file named".to_owned();
    for name in [&rel, &base] {
        if name.is_empty() {
            continue;
        }
        match resolver.read(name, max) {
            Ok(bytes) => return b.add_image(ImageData::Bytes { bytes, mime: None }),
            Err(e) => reason = e,
        }
    }
    b.add_image(ImageData::Missing {
        name: if rel.is_empty() { abs } else { rel },
        reason,
    })
}

/// ASCII FBX: `Key: values {children}` records; `*N { a: … }` arrays; `;` comments.
mod ascii {
    use super::{Node, P};
    use crate::{Error, Limits, Result};

    #[derive(Clone, Debug, PartialEq)]
    enum T {
        Key(String),
        Word(String),
        Num(String),
        Str(String),
        Star,
        Comma,
        Open,
        Close,
    }

    fn lex(b: &[u8]) -> Result<Vec<T>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            match c {
                b';' => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
                c if c.is_ascii_whitespace() => i += 1,
                b'{' => {
                    out.push(T::Open);
                    i += 1;
                }
                b'}' => {
                    out.push(T::Close);
                    i += 1;
                }
                b',' => {
                    out.push(T::Comma);
                    i += 1;
                }
                b'*' => {
                    out.push(T::Star);
                    i += 1;
                }
                b'"' => {
                    let start = i + 1;
                    let mut j = start;
                    while j < b.len() && b[j] != b'"' {
                        j += 1;
                    }
                    let s = String::from_utf8_lossy(&b[start..j.min(b.len())]);
                    out.push(T::Str(s.replace("&quot;", "\"")));
                    i = j + 1;
                }
                b'-' | b'+' | b'.' | b'0'..=b'9' => {
                    let start = i;
                    while i < b.len()
                        && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'.' | b'-' | b'+'))
                    {
                        i += 1;
                    }
                    out.push(T::Num(String::from_utf8_lossy(&b[start..i]).into_owned()));
                }
                _ => {
                    let start = i;
                    while i < b.len()
                        && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'|' | b'-'))
                    {
                        i += 1;
                    }
                    if i == start {
                        i += 1;
                        continue;
                    }
                    let w = String::from_utf8_lossy(&b[start..i]).into_owned();
                    if b.get(i) == Some(&b':') {
                        out.push(T::Key(w));
                        i += 1;
                    } else {
                        out.push(T::Word(w));
                    }
                }
            }
            if out.len() > 400_000_000 {
                return Err(Error::TooLarge("ASCII FBX too large".into()));
            }
        }
        Ok(out)
    }

    fn num(s: &str) -> P {
        if let Ok(i) = s.parse::<i64>() {
            P::I(i)
        } else {
            P::F(s.parse::<f64>().unwrap_or(f64::NAN))
        }
    }

    fn b64(s: &str) -> Vec<u8> {
        let val = |c: u8| -> Option<u32> {
            Some(match c {
                b'A'..=b'Z' => u32::from(c - b'A'),
                b'a'..=b'z' => u32::from(c - b'a') + 26,
                b'0'..=b'9' => u32::from(c - b'0') + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return None,
            })
        };
        let mut out = Vec::with_capacity(s.len() * 3 / 4);
        let (mut acc, mut bits) = (0u32, 0);
        for c in s.bytes() {
            let Some(v) = val(c) else { continue };
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }

    struct Parser {
        t: Vec<T>,
        i: usize,
    }

    impl Parser {
        fn nodes(&mut self, depth: usize) -> Result<Vec<Node>> {
            if depth > 64 {
                return Err(Error::Malformed("FBX: nesting too deep".into()));
            }
            let mut out = Vec::new();
            while self.i < self.t.len() {
                match self.t[self.i].clone() {
                    T::Close => {
                        self.i += 1;
                        return Ok(out);
                    }
                    T::Key(k) => {
                        self.i += 1;
                        out.push(self.node(k, depth)?);
                    }
                    _ => self.i += 1,
                }
            }
            Ok(out)
        }

        fn node(&mut self, name: String, depth: usize) -> Result<Node> {
            let mut props = Vec::new();
            let mut array = false;
            loop {
                match self.t.get(self.i) {
                    Some(T::Num(n)) => props.push(num(n)),
                    Some(T::Str(s)) => props.push(P::S(s.clone())),
                    Some(T::Word(w)) => props.push(P::S(w.clone())),
                    Some(T::Star) => array = true,
                    Some(T::Comma) => {}
                    _ => break,
                }
                self.i += 1;
            }
            let mut children = Vec::new();
            if self.t.get(self.i) == Some(&T::Open) {
                self.i += 1;
                children = self.nodes(depth + 1)?;
            }
            if array {
                // `*N { a: values }`.
                let vals = children
                    .iter()
                    .find(|c| c.name == "a")
                    .map(|a| a.props.clone())
                    .unwrap_or_default();
                let ints: Option<Vec<i64>> = vals
                    .iter()
                    .map(|p| match p {
                        P::I(v) => Some(*v),
                        _ => None,
                    })
                    .collect();
                props = vec![match ints {
                    Some(i) => P::Ints(i),
                    None => P::Floats(vals.iter().filter_map(P::num).collect()),
                }];
                children.clear();
            } else if name == "Content" {
                let text: String = props.iter().filter_map(P::str).collect();
                props = vec![P::Raw(b64(&text))];
            }
            Ok(Node {
                name,
                props,
                children,
            })
        }
    }

    pub(super) fn parse(bytes: &[u8], _limits: &Limits) -> Result<Vec<Node>> {
        let mut p = Parser {
            t: lex(bytes)?,
            i: 0,
        };
        p.nodes(0)
    }
}
