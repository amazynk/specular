//! 3MF: an OPC package (a ZIP) whose root relationship names the 3D model part, an XML
//! document of `resources` (objects with a `mesh` of `vertices` and `triangles`, or
//! `components` that place other objects; `basematerials`; `m:colorgroup`) and a `build` of
//! items with transforms. Objects in other model parts (`p:path`, the production
//! extension) are read too. Base materials become materials, colour groups vertex colours.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};

use crate::builder::{Affine, Builder, srgb_to_linear};
use crate::{Error, Limits, Material, Mesh, Result, Unit, Up, bad};

const NONE: u32 = u32::MAX;

#[derive(Default)]
struct ObjMesh {
    verts: Vec<[f32; 3]>,
    tris: Vec<[u32; 3]>,
    /// Per triangle (pid, p1, p2, p3), `NONE` where absent; only when any triangle has one.
    props: Option<Vec<[u32; 4]>>,
}

struct Component {
    object: u32,
    transform: Affine,
    path: Option<String>,
}

#[derive(Default)]
struct Object {
    mesh: Option<ObjMesh>,
    components: Vec<Component>,
    pid: u32,
    pindex: u32,
}

#[derive(Default)]
struct Model {
    unit: Unit,
    objects: HashMap<u32, Object>,
    /// Base material groups: (name, sRGB RGBA) each.
    bases: HashMap<u32, Vec<(String, [u8; 4])>>,
    /// Colour groups: sRGB RGBA each.
    colors: HashMap<u32, Vec<[u8; 4]>>,
    items: Vec<Component>,
    title: Option<String>,
    generator: Option<String>,
}

struct Package<'a> {
    zip: zip::ZipArchive<Cursor<&'a [u8]>>,
    budget: usize,
    /// The memory budget's share of the parts unpacked (grown by each part's size).
    res: specular_core::Reservation,
}

impl Package<'_> {
    /// A part's bytes, the name matched without case and leading slash.
    fn part(&mut self, name: &str) -> Result<Vec<u8>> {
        let want = name.trim_start_matches('/');
        let found = self
            .zip
            .file_names()
            .flatten()
            .find(|n| n.trim_start_matches('/').eq_ignore_ascii_case(want))
            .map(String::from)
            .ok_or_else(|| Error::Malformed(format!("no part {name}")))?;
        let f = self
            .zip
            .by_name(&found)
            .map_err(|e| Error::Malformed(format!("{found}: {e}")))?;
        // The declared size is the file's word; reserve it (capped) before unpacking, and
        // what was really unpacked after.
        let declared = f.size().min(self.budget as u64 + 1);
        self.res.grow(declared)?;
        let mut out = Vec::new();
        f.take(self.budget as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|e| Error::Malformed(format!("{found}: {e}")))?;
        if out.len() > self.budget {
            return Err(Error::TooLarge("3MF package unpacks too large".into()));
        }
        self.budget -= out.len();
        self.res.grow((out.len() as u64).saturating_sub(declared))?;
        Ok(out)
    }
}

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Malformed(format!("3MF package: {e}")))?;
    let mut pkg = Package {
        zip,
        budget: limits.max_unpacked,
        res: specular_core::Reservation::none(),
    };
    let root = root_part(&mut pkg);
    let mut models: HashMap<String, Model> = HashMap::new();
    let first = pkg.part(&root)?;
    models.insert(root.clone(), parse_model(&first, limits)?);
    drop(first);
    // Other model parts the components and items name.
    let mut pending: Vec<String> = Vec::new();
    let collect = |m: &Model, pending: &mut Vec<String>| {
        let paths = m
            .items
            .iter()
            .chain(m.objects.values().flat_map(|o| &o.components))
            .filter_map(|c| c.path.clone());
        pending.extend(paths);
    };
    collect(&models[&root], &mut pending);
    while let Some(p) = pending.pop() {
        let key = p.trim_start_matches('/').to_owned();
        if models.contains_key(&key) {
            continue;
        }
        if models.len() >= 64 {
            return bad("too many 3MF model parts");
        }
        let model = match pkg.part(&key) {
            Ok(b) => parse_model(&b, limits)?,
            Err(Error::TooLarge(e)) => return Err(Error::TooLarge(e)),
            Err(_) => Model::default(),
        };
        collect(&model, &mut pending);
        models.insert(key, model);
    }
    let mut b = Builder::new(limits);
    b.up = Up::Z;
    b.flat = true;
    let rm = &models[&root];
    b.unit = rm.unit;
    b.title = rm.title.clone();
    b.generator = rm.generator.clone();
    let mut mats: HashMap<(String, u32, u32), u32> = HashMap::new();
    let mut budget = 10_000_000usize;
    for item in &rm.items {
        let path = item
            .path
            .as_deref()
            .map_or(root.as_str(), |p| p.trim_start_matches('/'));
        emit(
            &models,
            path,
            item.object,
            &item.transform,
            0,
            &mut b,
            &mut mats,
            &mut budget,
        )?;
    }
    if budget == 0 {
        b.note("component tree too large; the rest is not shown");
    }
    b.finish()
}

/// The 3D model part the package's root relationships name.
fn root_part(pkg: &mut Package) -> String {
    let fallback = "3D/3dmodel.model".to_owned();
    let Ok(rels) = pkg.part("_rels/.rels") else {
        return fallback;
    };
    let mut r = quick_xml::Reader::from_reader(rels.as_slice());
    loop {
        match r.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) if e.local_name().as_ref() == b"Relationship" => {
                let ty = attr(&e, b"Type").unwrap_or_default();
                if ty.ends_with(b"/3dmodel")
                    && let Some(t) = attr(&e, b"Target")
                {
                    return String::from_utf8_lossy(&t)
                        .trim_start_matches('/')
                        .to_owned();
                }
            }
            Ok(Event::Eof) | Err(_) => return fallback,
            _ => {}
        }
    }
}

fn attr<'a>(e: &'a BytesStart, key: &[u8]) -> Option<Cow<'a, [u8]>> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.local_name().as_ref() == key)
        .map(|a| a.value)
}

fn num<T: std::str::FromStr>(e: &BytesStart, key: &[u8]) -> Option<T> {
    let v = attr(e, key)?;
    std::str::from_utf8(&v).ok()?.trim().parse().ok()
}

fn color(s: &[u8]) -> Option<[u8; 4]> {
    let s = std::str::from_utf8(s).ok()?.trim().strip_prefix('#')?;
    let h = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    match s.len() {
        6 => Some([h(0)?, h(2)?, h(4)?, 255]),
        8 => Some([h(0)?, h(2)?, h(4)?, h(6)?]),
        _ => None,
    }
}

fn transform(e: &BytesStart) -> Affine {
    let Some(t) = attr(e, b"transform") else {
        return Affine::IDENTITY;
    };
    let v: Vec<f64> = String::from_utf8_lossy(&t)
        .split_ascii_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect();
    if v.len() != 12 || v.iter().any(|x| !x.is_finite()) {
        return Affine::IDENTITY;
    }
    // Row vectors: p' = p · M; as columns, row r of ours is column r of theirs.
    Affine([
        [v[0], v[3], v[6], v[9]],
        [v[1], v[4], v[7], v[10]],
        [v[2], v[5], v[8], v[11]],
    ])
}

fn parse_model(xml: &[u8], limits: &Limits) -> Result<Model> {
    let mut m = Model {
        unit: Unit::Millimetre,
        ..Model::default()
    };
    let mut r = quick_xml::Reader::from_reader(xml);
    let mut obj: Option<(u32, Object)> = None;
    let mut group: Option<(u32, bool)> = None; // (id, base materials?)
    let mut meta: Option<Vec<u8>> = None;
    let mut meta_text = String::new();
    let mut in_build = false;
    let mut total_v = 0usize;
    let mut total_t = 0usize;
    loop {
        let ev = r
            .read_event()
            .map_err(|e| Error::Malformed(format!("3MF XML: {e}")))?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                match e.local_name().as_ref() {
                    b"model" => {
                        m.unit = match attr(e, b"unit").as_deref() {
                            Some(b"micron") => Unit::Micron,
                            Some(b"centimeter") => Unit::Centimetre,
                            Some(b"inch") => Unit::Inch,
                            Some(b"foot") => Unit::Foot,
                            Some(b"meter") => Unit::Metre,
                            _ => Unit::Millimetre,
                        };
                    }
                    b"metadata" if !empty => {
                        meta = attr(e, b"name").map(Cow::into_owned);
                        meta_text.clear();
                    }
                    b"object" => {
                        let id = num(e, b"id").unwrap_or(NONE);
                        obj = Some((
                            id,
                            Object {
                                pid: num(e, b"pid").unwrap_or(NONE),
                                pindex: num(e, b"pindex").unwrap_or(0),
                                ..Object::default()
                            },
                        ));
                    }
                    b"mesh" => {
                        if let Some((_, o)) = &mut obj {
                            o.mesh = Some(ObjMesh::default());
                        }
                    }
                    b"vertex" => {
                        if let Some(mesh) = obj.as_mut().and_then(|o| o.1.mesh.as_mut()) {
                            total_v += 1;
                            if total_v > limits.max_vertices {
                                return Err(Error::TooLarge("too many vertices".into()));
                            }
                            let c = |k: &[u8]| num::<f32>(e, k).unwrap_or(f32::NAN);
                            mesh.verts.push([c(b"x"), c(b"y"), c(b"z")]);
                        }
                    }
                    b"triangle" => {
                        if let Some(mesh) = obj.as_mut().and_then(|o| o.1.mesh.as_mut()) {
                            total_t += 1;
                            if total_t > limits.max_triangles {
                                return Err(Error::TooLarge("too many triangles".into()));
                            }
                            let v = |k: &[u8]| num::<u32>(e, k).unwrap_or(NONE);
                            mesh.tris.push([v(b"v1"), v(b"v2"), v(b"v3")]);
                            let p = [v(b"pid"), v(b"p1"), v(b"p2"), v(b"p3")];
                            if p != [NONE; 4] || mesh.props.is_some() {
                                let n = mesh.tris.len();
                                let props = mesh.props.get_or_insert_with(Vec::new);
                                props.resize(n - 1, [NONE; 4]);
                                props.push(p);
                            }
                        }
                    }
                    b"component" => {
                        if let Some((_, o)) = &mut obj {
                            o.components.push(Component {
                                object: num(e, b"objectid").unwrap_or(NONE),
                                transform: transform(e),
                                path: attr(e, b"path")
                                    .map(|p| String::from_utf8_lossy(&p).into_owned()),
                            });
                        }
                    }
                    b"build" => in_build = true,
                    b"item" if in_build => {
                        m.items.push(Component {
                            object: num(e, b"objectid").unwrap_or(NONE),
                            transform: transform(e),
                            path: attr(e, b"path")
                                .map(|p| String::from_utf8_lossy(&p).into_owned()),
                        });
                    }
                    b"basematerials" => {
                        let id = num(e, b"id").unwrap_or(NONE);
                        m.bases.entry(id).or_default();
                        group = Some((id, true));
                    }
                    b"base" => {
                        if let Some((id, true)) = group {
                            let name = attr(e, b"name")
                                .map(|n| String::from_utf8_lossy(&n).into_owned())
                                .unwrap_or_default();
                            let c = attr(e, b"displaycolor")
                                .and_then(|c| color(&c))
                                .unwrap_or([255; 4]);
                            m.bases.entry(id).or_default().push((name, c));
                        }
                    }
                    b"colorgroup" => {
                        let id = num(e, b"id").unwrap_or(NONE);
                        m.colors.entry(id).or_default();
                        group = Some((id, false));
                    }
                    b"color" => {
                        if let Some((id, false)) = group {
                            let c = attr(e, b"color")
                                .and_then(|c| color(&c))
                                .unwrap_or([255; 4]);
                            m.colors.entry(id).or_default().push(c);
                        }
                    }
                    _ => {}
                }
                if empty
                    && e.local_name().as_ref() == b"object"
                    && let Some((id, o)) = obj.take()
                {
                    m.objects.insert(id, o);
                }
            }
            Event::Text(t) => {
                if meta.is_some() && meta_text.len() < 4096 {
                    meta_text.push_str(&t.decode().unwrap_or_default());
                }
            }
            Event::GeneralRef(g) => {
                if meta.is_some() && meta_text.len() < 4096 {
                    let name = g.decode().unwrap_or_default();
                    if let Ok(s) = quick_xml::escape::unescape(&format!("&{name};")) {
                        meta_text.push_str(&s);
                    }
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"object" => {
                    if let Some((id, o)) = obj.take() {
                        m.objects.insert(id, o);
                    }
                }
                b"basematerials" | b"colorgroup" => group = None,
                b"build" => in_build = false,
                b"metadata" => {
                    let text = meta_text.trim().to_owned();
                    match meta.take().as_deref() {
                        Some(b"Title") if !text.is_empty() => m.title = Some(text),
                        Some(b"Application") if !text.is_empty() => m.generator = Some(text),
                        _ => {}
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(m)
}

#[allow(clippy::too_many_arguments)]
fn emit(
    models: &HashMap<String, Model>,
    path: &str,
    id: u32,
    xf: &Affine,
    depth: u32,
    b: &mut Builder,
    mats: &mut HashMap<(String, u32, u32), u32>,
    budget: &mut usize,
) -> Result<()> {
    if depth > 32 || *budget == 0 {
        return Ok(());
    }
    *budget -= 1;
    let Some(model) = models.get(path) else {
        b.note_once("a build item names a model part that is missing");
        return Ok(());
    };
    let Some(obj) = model.objects.get(&id) else {
        b.note_once("a build item names an object that does not exist");
        return Ok(());
    };
    if let Some(mesh) = &obj.mesh {
        emit_mesh(model, path, obj, mesh, xf, b, mats)?;
    }
    for c in &obj.components {
        let child = xf.then_after(&c.transform);
        let p = c
            .path
            .as_deref()
            .map_or(path, |p| p.trim_start_matches('/'));
        emit(models, p, c.object, &child, depth + 1, b, mats, budget)?;
    }
    Ok(())
}

enum Look {
    None,
    Material(u32),
    Colors([[u8; 4]; 3]),
}

fn emit_mesh(
    model: &Model,
    path: &str,
    obj: &Object,
    mesh: &ObjMesh,
    xf: &Affine,
    b: &mut Builder,
    mats: &mut HashMap<(String, u32, u32), u32>,
) -> Result<()> {
    let flip = xf.det() < 0.0;
    let n = mesh.verts.len() as u32;
    let look = |t: usize, b: &mut Builder, mats: &mut HashMap<(String, u32, u32), u32>| -> Look {
        let p = mesh.props.as_ref().map_or([NONE; 4], |p| p[t]);
        let pid = if p[0] == NONE { obj.pid } else { p[0] };
        if pid == NONE {
            return Look::None;
        }
        let p1 = if p[1] == NONE { obj.pindex } else { p[1] };
        let p2 = if p[2] == NONE { p1 } else { p[2] };
        let p3 = if p[3] == NONE { p1 } else { p[3] };
        if let Some(base) = model.bases.get(&pid) {
            let Some((name, c)) = base.get(p1 as usize) else {
                return Look::None;
            };
            let key = (path.to_owned(), pid, p1);
            let m = *mats.entry(key).or_insert_with(|| {
                b.add_material(Material {
                    name: name.clone(),
                    base_color: [
                        srgb_to_linear(f32::from(c[0]) / 255.0),
                        srgb_to_linear(f32::from(c[1]) / 255.0),
                        srgb_to_linear(f32::from(c[2]) / 255.0),
                        f32::from(c[3]) / 255.0,
                    ],
                    texture: None,
                })
            });
            return Look::Material(m);
        }
        if let Some(cg) = model.colors.get(&pid) {
            let get = |i: u32| cg.get(i as usize).copied().unwrap_or([255; 4]);
            return Look::Colors([get(p1), get(p2), get(p3)]);
        }
        Look::None
    };
    let any_colors = mesh.props.is_some() || obj.pid != NONE;
    let colored =
        any_colors && (0..mesh.tris.len()).any(|t| matches!(look(t, b, mats), Look::Colors(_)));
    let order = |t: &[u32; 3]| if flip { [t[0], t[2], t[1]] } else { *t };
    if !colored {
        b.reserve_vertices(mesh.verts.len())?;
        let base = b.vertex_count();
        for &v in &mesh.verts {
            b.vertex(xf.point(v), None, None, None)?;
        }
        b.reserve_triangles(mesh.tris.len())?;
        for (i, t) in mesh.tris.iter().enumerate() {
            if t.iter().any(|&v| v >= n) {
                continue;
            }
            match look(i, b, mats) {
                Look::Material(m) => b.material(Some(m)),
                _ => b.material(None),
            }
            let [a, c1, c2] = order(t);
            b.triangle(base + a, base + c1, base + c2)?;
        }
    } else {
        b.reserve_vertices(mesh.tris.len().saturating_mul(3))?;
        for (i, t) in mesh.tris.iter().enumerate() {
            if t.iter().any(|&v| v >= n) {
                continue;
            }
            let lk = look(i, b, mats);
            let cols = match lk {
                Look::Colors(c) => Some(c),
                _ => None,
            };
            b.material(match lk {
                Look::Material(m) => Some(m),
                _ => None,
            });
            let o = order(t);
            let mut ids = [0u32; 3];
            for (k, &v) in o.iter().enumerate() {
                let corner = t.iter().position(|&x| x == v).unwrap_or(k);
                let c = cols.map(|c| c[corner]);
                ids[k] = b.vertex(xf.point(mesh.verts[v as usize]), None, c, None)?;
            }
            b.triangle(ids[0], ids[1], ids[2])?;
        }
    }
    Ok(())
}
