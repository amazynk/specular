//! Alembic (`.abc`) archives in the Ogawa container: the object hierarchy with each object's
//! schema, and the first sample of every polygon mesh (`AbcGeom_PolyMesh`, and subdivision
//! surfaces drawn as their control cage) placed by its transforms (`AbcGeom_Xform` scale,
//! translate, rotate and matrix operations). Animation is not played: sample 0 is shown.
//! Archives in the older HDF5 container are refused.
//!
//! Ogawa is a tree of groups (lists of child offsets) and data blocks (a size and bytes); the
//! archive's root group holds the top object, metadata and an indexed-metadata table; each
//! object group holds its property compound, its children and a block of child headers.
//! Written from the Alembic file format documentation and by inspecting files.

use crate::builder::{Affine, Builder};
use crate::poly::{Poly, emit};
use crate::{Error, Limits, Mesh, Result, Up, bad};

/// Most children of one group read.
const MAX_CHILDREN: u64 = 1 << 24;
/// Deepest object nesting.
const MAX_DEPTH: usize = 256;
/// Most objects listed.
const MAX_OBJECTS: usize = 100_000;

/// Whether `head` starts an Ogawa archive.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"Ogawa")
}

/// Whether `head` starts an HDF5 file (Alembic's older container).
pub fn is_hdf5(head: &[u8]) -> bool {
    head.starts_with(b"\x89HDF\r\n\x1a\n")
}

struct Ogawa<'a> {
    d: &'a [u8],
}

const DATA_BIT: u64 = 1 << 63;

impl<'a> Ogawa<'a> {
    fn u64_at(&self, at: u64) -> Result<u64> {
        let at = usize::try_from(at).map_err(|_| truncated())?;
        let b = self.d.get(at..at + 8).ok_or_else(truncated)?;
        Ok(u64::from_le_bytes(b.try_into().map_err(|_| truncated())?))
    }

    /// The raw child entries of the group at `off` (0: an empty group).
    fn children(&self, off: u64) -> Result<Vec<u64>> {
        if off == 0 {
            return Ok(Vec::new());
        }
        let n = self.u64_at(off)?;
        if n > MAX_CHILDREN || n.saturating_mul(8) > self.d.len() as u64 {
            return Err(Error::Malformed(
                "an Ogawa group lists more children than fit".into(),
            ));
        }
        (0..n).map(|i| self.u64_at(off + 8 + i * 8)).collect()
    }

    /// The bytes of data entry `raw` (empty when it is not data or has offset 0).
    fn data(&self, raw: u64) -> Result<&'a [u8]> {
        if raw & DATA_BIT == 0 {
            return Err(Error::Malformed("expected an Ogawa data block".into()));
        }
        let off = raw & !DATA_BIT;
        if off == 0 {
            return Ok(&[]);
        }
        let n = self.u64_at(off)?;
        let s = usize::try_from(off + 8).map_err(|_| truncated())?;
        let e = usize::try_from(n)
            .ok()
            .and_then(|n| s.checked_add(n))
            .ok_or_else(truncated)?;
        self.d.get(s..e).ok_or_else(truncated)
    }

    fn group(raw: u64) -> Result<u64> {
        if raw & DATA_BIT != 0 {
            return Err(Error::Malformed("expected an Ogawa group".into()));
        }
        Ok(raw)
    }
}

fn truncated() -> Error {
    Error::Malformed("the Alembic file is cut short".into())
}

/// A little-endian reader over a header block.
struct Rd<'a> {
    d: &'a [u8],
    at: usize,
}

impl<'a> Rd<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let b = self.d.get(self.at..self.at + n).ok_or_else(truncated)?;
        self.at += n;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// A size of 1, 2 or 4 bytes (`hint` 0, 1, 2).
    fn sized(&mut self, hint: u32) -> Result<u32> {
        Ok(match hint {
            0 => self.u8()? as u32,
            1 => {
                let b = self.bytes(2)?;
                u16::from_le_bytes([b[0], b[1]]) as u32
            }
            _ => self.u32()?,
        })
    }
    fn string(&mut self, n: usize) -> Result<String> {
        Ok(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
}

/// Plain old data types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pod {
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F16,
    F32,
    F64,
    Str,
    WStr,
}

impl Pod {
    fn from_code(v: u32) -> Option<Pod> {
        use Pod::*;
        [
            Bool, U8, I8, U16, I16, U32, I32, U64, I64, F16, F32, F64, Str, WStr,
        ]
        .get(v as usize)
        .copied()
    }
    fn size(self) -> usize {
        match self {
            Pod::Bool | Pod::U8 | Pod::I8 | Pod::Str | Pod::WStr => 1,
            Pod::U16 | Pod::I16 | Pod::F16 => 2,
            Pod::U32 | Pod::I32 | Pod::F32 => 4,
            Pod::U64 | Pod::I64 | Pod::F64 => 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Compound,
    Scalar,
    Array,
}

#[derive(Clone, Debug)]
struct Property {
    kind: Kind,
    name: String,
    pod: Option<Pod>,
    samples: u32,
    raw: u64,
}

/// One object: its name, schema and where its group is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    /// The full path, `/parent/child`.
    pub path: String,
    /// The schema (`AbcGeom_PolyMesh_v1`, `AbcGeom_Xform_v3`…), empty for none.
    pub schema: String,
}

/// What an archive holds.
#[derive(Clone, Debug, Default)]
pub struct Archive {
    pub objects: Vec<Object>,
    /// The archive's metadata (`_ai_Application`, `_ai_DateWritten`…), as stored.
    pub metadata: Vec<(String, String)>,
}

fn metadata_pairs(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

struct Reader<'a> {
    o: Ogawa<'a>,
    /// Indexed metadata strings (index 0 is empty).
    indexed: Vec<String>,
}

impl<'a> Reader<'a> {
    fn open(d: &'a [u8]) -> Result<(Reader<'a>, u64, String)> {
        if is_hdf5(d) {
            return Err(Error::Unsupported(
                "this Alembic file uses the older HDF5 container; only Ogawa archives are read. Re-export it from the program that made it (Ogawa is the default)."
                    .into(),
            ));
        }
        if !probe(d) || d.len() < 16 {
            return Err(Error::Malformed("not an Alembic (Ogawa) file".into()));
        }
        let o = Ogawa { d };
        let root = o.u64_at(8)?;
        let kids = o.children(Ogawa::group(root)?)?;
        if kids.len() < 3 {
            return Err(Error::Malformed(
                "the Alembic archive has no top object".into(),
            ));
        }
        let top = Ogawa::group(kids[2])?;
        let meta = kids
            .get(3)
            .and_then(|r| o.data(*r).ok())
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        let mut indexed = vec![String::new()];
        if let Some(r) = kids.get(5)
            && let Ok(b) = o.data(*r)
        {
            let mut rd = Rd { d: b, at: 0 };
            while rd.at < b.len() && indexed.len() < 256 {
                let Ok(n) = rd.u8() else { break };
                let Ok(s) = rd.string(n as usize) else { break };
                indexed.push(s);
            }
        }
        Ok((Reader { o, indexed }, top, meta))
    }

    /// The child objects of the object group `g`: (name, metadata, group).
    fn child_objects(&self, g: u64) -> Result<Vec<(String, String, u64)>> {
        let kids = self.o.children(g)?;
        if kids.len() < 2 {
            return Ok(Vec::new());
        }
        let headers = self.o.data(kids[kids.len() - 1])?;
        let body = &headers[..headers.len().saturating_sub(32)];
        let mut rd = Rd { d: body, at: 0 };
        let mut out = Vec::new();
        let mut i = 1;
        while rd.at < body.len() && i < kids.len() - 1 {
            let n = rd.u32()? as usize;
            let name = rd.string(n)?;
            let mi = rd.u8()?;
            let meta = if mi == 0xFF {
                let n = rd.u32()? as usize;
                rd.string(n)?
            } else {
                self.indexed.get(mi as usize).cloned().unwrap_or_default()
            };
            out.push((name, meta, Ogawa::group(kids[i])?));
            i += 1;
        }
        Ok(out)
    }

    /// The properties of the compound property group `g`.
    fn properties(&self, g: u64) -> Result<Vec<Property>> {
        let kids = self.o.children(g)?;
        let Some(last) = kids.last() else {
            return Ok(Vec::new());
        };
        let h = self.o.data(*last)?;
        let mut rd = Rd { d: h, at: 0 };
        let mut out = Vec::new();
        while rd.at < h.len() && out.len() < kids.len() - 1 {
            let info = rd.u32()?;
            let kind = match info & 3 {
                0 => Kind::Compound,
                1 => Kind::Scalar,
                _ => Kind::Array,
            };
            let hint = (info >> 2) & 3;
            let (mut pod, mut samples) = (None, 0);
            if kind != Kind::Compound {
                pod = Pod::from_code((info >> 4) & 15);
                samples = rd.sized(hint)?;
                if info & 0x200 != 0 {
                    rd.sized(hint)?;
                    rd.sized(hint)?;
                }
                if info & 0x100 != 0 {
                    rd.sized(hint)?;
                }
            }
            let n = rd.sized(hint)? as usize;
            let name = rd.string(n)?;
            if (info >> 20) & 0xFF == 0xFF {
                let n = rd.sized(hint)? as usize;
                rd.bytes(n)?;
            }
            out.push(Property {
                kind,
                name,
                pod,
                samples,
                raw: kids[out.len()],
            });
        }
        Ok(out)
    }

    /// Sample 0 of a scalar or array property, as its raw values.
    fn sample0(&self, p: &Property) -> Result<&'a [u8]> {
        if p.samples == 0 {
            return Ok(&[]);
        }
        let kids = self.o.children(Ogawa::group(p.raw)?)?;
        let Some(first) = kids.first() else {
            return Ok(&[]);
        };
        let d = self.o.data(*first)?;
        // Each sample starts with its 16-byte digest.
        Ok(d.get(16..).unwrap_or(&[]))
    }

    fn f32s(&self, p: &Property) -> Result<Vec<f32>> {
        let d = self.sample0(p)?;
        Ok(match p.pod {
            Some(Pod::F32) => d
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
            Some(Pod::F64) => d
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or([0; 8])) as f32)
                .collect(),
            _ => return bad("positions are not floating point"),
        })
    }

    fn f64s(&self, p: &Property) -> Result<Vec<f64>> {
        let d = self.sample0(p)?;
        Ok(match p.pod {
            Some(Pod::F32) => d
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64)
                .collect(),
            Some(Pod::F64) => d
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap_or([0; 8])))
                .collect(),
            _ => Vec::new(),
        })
    }

    fn ints(&self, p: &Property) -> Result<Vec<i64>> {
        let d = self.sample0(p)?;
        let Some(pod) = p.pod else {
            return Ok(Vec::new());
        };
        let s = pod.size();
        Ok(d.chunks_exact(s)
            .map(|c| match pod {
                Pod::U8 | Pod::Bool => c[0] as i64,
                Pod::I8 => c[0] as i8 as i64,
                Pod::U16 => u16::from_le_bytes([c[0], c[1]]) as i64,
                Pod::I16 => i16::from_le_bytes([c[0], c[1]]) as i64,
                Pod::U32 => u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i64,
                Pod::I32 => i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i64,
                _ => i64::from_le_bytes(c.try_into().unwrap_or([0; 8])),
            })
            .collect())
    }
}

/// Lists the objects of an Alembic archive and its metadata.
pub fn archive(bytes: &[u8]) -> Result<Archive> {
    let (r, top, meta) = Reader::open(bytes)?;
    let mut out = Archive {
        objects: Vec::new(),
        metadata: metadata_pairs(&meta),
    };
    let mut stack = vec![(top, String::new(), 0usize)];
    while let Some((g, path, depth)) = stack.pop() {
        if depth > MAX_DEPTH || out.objects.len() >= MAX_OBJECTS {
            break;
        }
        let kids = r.child_objects(g)?;
        for (name, meta, cg) in kids.into_iter().rev() {
            let p = format!("{path}/{name}");
            let schema = metadata_pairs(&meta)
                .into_iter()
                .find(|(k, _)| k == "schema")
                .map(|(_, v)| v)
                .unwrap_or_default();
            out.objects.push(Object {
                path: p.clone(),
                schema,
            });
            stack.push((cg, p, depth + 1));
        }
    }
    // Depth-first order as in the file.
    out.objects.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// An `.xform` compound's local transform (sample 0) and whether it inherits its parent's.
fn xform(r: &Reader, props: &[Property]) -> Result<(Affine, bool)> {
    let Some(x) = props
        .iter()
        .find(|p| p.name == ".xform" && p.kind == Kind::Compound)
    else {
        return Ok((Affine::IDENTITY, true));
    };
    let xp = r.properties(Ogawa::group(x.raw)?)?;
    let inherits = xp
        .iter()
        .find(|p| p.name == ".inherits")
        .map(|p| r.ints(p).map(|v| v.first().copied().unwrap_or(1) != 0))
        .transpose()?
        .unwrap_or(true);
    let ops = match xp.iter().find(|p| p.name == ".ops") {
        Some(p) => r.ints(p)?,
        None => return Ok((Affine::IDENTITY, inherits)),
    };
    let vals = match xp.iter().find(|p| p.name == ".vals") {
        Some(p) => r.f64s(p)?,
        None => Vec::new(),
    };
    let mut m = Affine::IDENTITY;
    let mut at = 0usize;
    let v = |n: usize, at: &mut usize| -> Vec<f64> {
        let s: Vec<f64> = (0..n)
            .map(|i| vals.get(*at + i).copied().unwrap_or(0.0))
            .collect();
        *at += n;
        s
    };
    for op in ops {
        let a = match (op >> 4) & 15 {
            0 => {
                let s = v(3, &mut at);
                Affine([
                    [s[0], 0.0, 0.0, 0.0],
                    [0.0, s[1], 0.0, 0.0],
                    [0.0, 0.0, s[2], 0.0],
                ])
            }
            1 => {
                let t = v(3, &mut at);
                Affine([
                    [1.0, 0.0, 0.0, t[0]],
                    [0.0, 1.0, 0.0, t[1]],
                    [0.0, 0.0, 1.0, t[2]],
                ])
            }
            2 => {
                let r = v(4, &mut at);
                rotation([r[0], r[1], r[2]], r[3])
            }
            3 => {
                // Imath row-vector matrix, row major: transposed into column form.
                let e = v(16, &mut at);
                Affine([
                    [e[0], e[4], e[8], e[12]],
                    [e[1], e[5], e[9], e[13]],
                    [e[2], e[6], e[10], e[14]],
                ])
            }
            4 => rotation([1.0, 0.0, 0.0], v(1, &mut at)[0]),
            5 => rotation([0.0, 1.0, 0.0], v(1, &mut at)[0]),
            6 => rotation([0.0, 0.0, 1.0], v(1, &mut at)[0]),
            _ => return bad("unknown transform operation"),
        };
        // The first operation is the outermost.
        m = m.then_after(&a);
    }
    Ok((m, inherits))
}

/// A rotation of `deg` degrees about `axis`.
fn rotation(axis: [f64; 3], deg: f64) -> Affine {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if l == 0.0 || !l.is_finite() {
        return Affine::IDENTITY;
    }
    let [x, y, z] = axis.map(|v| v / l);
    let (s, c) = deg.to_radians().sin_cos();
    let t = 1.0 - c;
    Affine([
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0.0],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0.0],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0.0],
    ])
}

/// One polygon mesh as stored (sample 0), with its placement in the archive.
#[derive(Clone, Debug)]
pub struct RawMesh {
    pub path: String,
    pub positions: Vec<[f32; 3]>,
    pub counts: Vec<u32>,
    pub indices: Vec<u32>,
    /// Object to archive transform: `out[r] = Σ m[r][c]·in[c] + m[r][3]`.
    pub transform: [[f64; 4]; 3],
}

/// The polygon meshes and subdivision surfaces (sample 0) of an archive, the schemas of
/// the other objects it holds (transforms excluded), and the archive's metadata.
pub fn meshes(bytes: &[u8]) -> Result<(Vec<RawMesh>, Vec<String>, String)> {
    let (r, top, meta) = Reader::open(bytes)?;
    let mut out = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut stack = vec![(top, Affine::IDENTITY, String::new(), 0usize)];
    let mut visited = 0usize;
    while let Some((g, parent, path, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        for (name, meta, cg) in r.child_objects(g)? {
            visited += 1;
            if visited > MAX_OBJECTS {
                return Err(Error::TooLarge(format!("more than {MAX_OBJECTS} objects")));
            }
            let path = format!("{path}/{name}");
            let schema = metadata_pairs(&meta)
                .into_iter()
                .find(|(k, _)| k == "schema")
                .map(|(_, v)| v)
                .unwrap_or_default();
            let kids = r.o.children(cg)?;
            let props = match kids.first() {
                Some(p) if p & DATA_BIT == 0 => r.properties(*p)?,
                _ => Vec::new(),
            };
            let mut world = parent;
            if schema.starts_with("AbcGeom_Xform") {
                let (local, inherits) = xform(&r, &props)?;
                world = if inherits {
                    parent.then_after(&local)
                } else {
                    local
                };
            } else if schema.starts_with("AbcGeom_PolyMesh") || schema.starts_with("AbcGeom_SubD") {
                if let Some(geom) = props.iter().find(|p| p.name == ".geom") {
                    let gp = r.properties(Ogawa::group(geom.raw)?)?;
                    let find = |n: &str| gp.iter().find(|p| p.name == n);
                    if let (Some(pp), Some(fi), Some(fc)) =
                        (find("P"), find(".faceIndices"), find(".faceCounts"))
                    {
                        let flat = r.f32s(pp)?;
                        let to_u32 = |v: Vec<i64>| -> Vec<u32> {
                            v.into_iter()
                                .map(|x| u32::try_from(x).unwrap_or(u32::MAX))
                                .collect()
                        };
                        out.push(RawMesh {
                            path: path.clone(),
                            positions: flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
                            counts: to_u32(r.ints(fc)?),
                            indices: to_u32(r.ints(fi)?),
                            transform: world.0,
                        });
                    }
                }
            } else if !schema.is_empty() && !skipped.contains(&schema) {
                skipped.push(schema.clone());
            }
            stack.push((cg, world, path, depth + 1));
        }
    }
    Ok((out, skipped, meta))
}

/// Reads sample 0 of every polygon mesh and subdivision surface, placed by its transforms.
pub fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let (meshes, skipped, meta) = meshes(bytes)?;
    if meshes.is_empty() {
        return Err(Error::Unsupported(if skipped.is_empty() {
            "this Alembic archive holds no polygon meshes".into()
        } else {
            format!(
                "this Alembic archive holds no polygon meshes, only {}",
                skipped.join(", ")
            )
        }));
    }
    let mut b = Builder::new(limits);
    for m in &meshes {
        let mut poly = Poly::new(&m.positions, &m.counts, &m.indices);
        // Alembic faces wind clockwise.
        poly.clockwise = true;
        emit(&mut b, &poly, &Affine(m.transform))?;
    }
    for s in skipped {
        b.note(format!("{s} objects are not drawn"));
    }
    let mut m = b.finish()?;
    m.up = Up::Y;
    let meta = metadata_pairs(&meta);
    m.generator = meta
        .iter()
        .find(|(k, _)| k == "_ai_Application")
        .map(|(_, v)| v.clone());
    m.title = meta
        .iter()
        .find(|(k, _)| k == "_ai_Description")
        .map(|(_, v)| v.clone())
        .filter(|s| !s.is_empty() && s != "unknown");
    Ok(m)
}

/// A generated archive for tests elsewhere ([`crate::fixtures::alembic`]).
#[doc(hidden)]
pub mod fixture {
    use super::DATA_BIT;

    /// Writes an Ogawa file: groups and data blocks appended, children as offsets.
    struct W {
        d: Vec<u8>,
    }

    type Prop = (u64, Vec<u8>);

    impl W {
        fn data(&mut self, b: &[u8]) -> u64 {
            if b.is_empty() {
                return DATA_BIT;
            }
            let at = self.d.len() as u64;
            self.d.extend_from_slice(&(b.len() as u64).to_le_bytes());
            self.d.extend_from_slice(b);
            at | DATA_BIT
        }
        fn group(&mut self, kids: &[u64]) -> u64 {
            let at = self.d.len() as u64;
            self.d.extend_from_slice(&(kids.len() as u64).to_le_bytes());
            for k in kids {
                self.d.extend_from_slice(&k.to_le_bytes());
            }
            at
        }
        /// A one-sample property (`kind` 1 scalar, 2 array) of `pod` with `extent`.
        fn prop(&mut self, kind: u32, name: &str, pod: u32, extent: u32, values: &[u8]) -> Prop {
            let mut sample = vec![0u8; 16];
            sample.extend_from_slice(values);
            let s = self.data(&sample);
            let g = if kind == 2 {
                let dims = self.data(&[]);
                self.group(&[s, dims])
            } else {
                self.group(&[s])
            };
            let info: u32 = kind | (pod << 4) | (extent << 12);
            let mut h = info.to_le_bytes().to_vec();
            h.push(1); // samples
            h.push(name.len() as u8);
            h.extend_from_slice(name.as_bytes());
            (g, h)
        }
        fn compound(&mut self, props: &[Prop]) -> u64 {
            let mut headers = Vec::new();
            let mut kids: Vec<u64> = Vec::new();
            for (g, h) in props {
                kids.push(*g);
                headers.extend_from_slice(h);
            }
            kids.push(self.data(&headers));
            self.group(&kids)
        }
        fn compound_header(name: &str) -> Vec<u8> {
            let mut h = 0u32.to_le_bytes().to_vec();
            h.push(name.len() as u8);
            h.extend_from_slice(name.as_bytes());
            h
        }
    }

    fn le<T: Copy, const N: usize>(v: &[T], f: impl Fn(T) -> [u8; N]) -> Vec<u8> {
        v.iter().flat_map(|x| f(*x)).collect()
    }

    fn object_header(name: &str, schema: &str) -> Vec<u8> {
        let mut h = (name.len() as u32).to_le_bytes().to_vec();
        h.extend_from_slice(name.as_bytes());
        h.push(0xFF);
        let meta = format!("schema={schema}");
        h.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        h.extend_from_slice(meta.as_bytes());
        h.extend_from_slice(&[0u8; 32]);
        h
    }

    /// An archive with `/xf/quad`: a unit quad translated by (10, 0, 0).
    pub fn quad_archive() -> Vec<u8> {
        let mut w = W {
            d: b"Ogawa\xff\x00\x01\0\0\0\0\0\0\0\0".to_vec(),
        };
        let pts = [0f32, 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0.];
        let p = w.prop(2, "P", 10, 3, &le(&pts, f32::to_le_bytes));
        let fi = w.prop(
            2,
            ".faceIndices",
            6,
            1,
            &le(&[0i32, 1, 2, 3], i32::to_le_bytes),
        );
        let fc = w.prop(2, ".faceCounts", 6, 1, &le(&[4i32], i32::to_le_bytes));
        let geom = w.compound(&[p, fi, fc]);
        let quad_props = w.compound(&[(geom, W::compound_header(".geom"))]);
        let quad_headers = w.data(&[0u8; 32]);
        let quad = w.group(&[quad_props, quad_headers]);
        // One translate operation and its three values.
        let ops = w.prop(1, ".ops", 1, 1, &[0x10]);
        let vals = w.prop(1, ".vals", 11, 3, &le(&[10f64, 0.0, 0.0], f64::to_le_bytes));
        let xform = w.compound(&[ops, vals]);
        let xf_props = w.compound(&[(xform, W::compound_header(".xform"))]);
        let xf_headers = w.data(&object_header("quad", "AbcGeom_PolyMesh_v1"));
        let xf = w.group(&[xf_props, quad, xf_headers]);
        let top_props = w.compound(&[]);
        let top_headers = w.data(&object_header("xf", "AbcGeom_Xform_v3"));
        let top = w.group(&[top_props, xf, top_headers]);
        let v = w.data(&1i32.to_le_bytes());
        let lib = w.data(&10703i32.to_le_bytes());
        let meta = w.data(b"_ai_Application=Specular test");
        let ts = w.data(&[]);
        let idx = w.data(&[]);
        let root = w.group(&[v, lib, top, meta, ts, idx]);
        w.d[8..16].copy_from_slice(&root.to_le_bytes());
        w.d
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::quad_archive;
    use super::*;

    #[test]
    fn generated_quad() {
        let d = quad_archive();
        let a = archive(&d).unwrap();
        let objects: Vec<(&str, &str)> = a
            .objects
            .iter()
            .map(|o| (o.path.as_str(), o.schema.as_str()))
            .collect();
        assert_eq!(
            objects,
            vec![
                ("/xf", "AbcGeom_Xform_v3"),
                ("/xf/quad", "AbcGeom_PolyMesh_v1")
            ]
        );
        let (meshes, skipped, _) = meshes(&d).unwrap();
        assert!(skipped.is_empty());
        assert_eq!(meshes[0].counts, vec![4]);
        assert_eq!(meshes[0].transform[0][3], 10.0);
        let m = read(&d, &Limits::default()).unwrap();
        assert_eq!(m.triangle_count(), 2);
        assert_eq!(m.generator.as_deref(), Some("Specular test"));
        assert!(m.positions.iter().all(|p| p[0] >= 10.0 && p[0] <= 11.0));
    }

    #[test]
    fn refusals_and_damage() {
        let hdf5 = b"\x89HDF\r\n\x1a\n\0\0\0\0\0\0\0\0";
        assert!(matches!(
            read(hdf5, &Limits::default()),
            Err(Error::Unsupported(_))
        ));
        let d = quad_archive();
        for n in 0..d.len() {
            let _ = read(&d[..n], &Limits::default());
            let _ = archive(&d[..n]);
        }
        let mut x = d.clone();
        for i in (16..x.len()).step_by(5) {
            x[i] ^= 0xA5;
            let _ = read(&x, &Limits::default());
        }
    }
}
