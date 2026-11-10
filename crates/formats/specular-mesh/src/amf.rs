//! AMF, the Additive Manufacturing File Format (ISO/ASTM 52915): an XML document (plain, or
//! the only file of a ZIP) of objects, each a mesh of vertices and volumes of triangles, with
//! colours on objects, volumes, materials, vertices and triangles, and constellations that
//! place objects (and other constellations). Read streaming. Curved triangles (`edge`
//! tangents) are drawn flat; colour formulas are not evaluated.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};

use crate::builder::{Affine, Builder, srgb_to_linear};
use crate::poly::{Attr, Poly, emit};
use crate::{Error, Limits, Material, Mesh, Result, Unit, Up, bad};

type Rgba = [f32; 4];

#[derive(Default)]
struct Volume {
    material: Option<String>,
    color: Option<Rgba>,
    tris: Vec<[u32; 3]>,
    /// Per triangle; only when any triangle has one.
    tri_colors: Vec<Option<Rgba>>,
}

#[derive(Default)]
struct Object {
    verts: Vec<[f32; 3]>,
    vert_colors: Vec<Option<Rgba>>,
    any_vert_color: bool,
    volumes: Vec<Volume>,
    color: Option<Rgba>,
}

struct Instance {
    object: String,
    delta: [f64; 3],
    rot: [f64; 3],
}

#[derive(Default)]
struct Doc {
    unit: Unit,
    objects: HashMap<String, Object>,
    /// Object ids in file order.
    order: Vec<String>,
    materials: HashMap<String, Rgba>,
    constellations: HashMap<String, Vec<Instance>>,
    title: Option<String>,
    curved: bool,
}

/// The XML of a plain or zipped AMF file, and whether the ZIP holds more than one.
fn document<'a>(bytes: &'a [u8], limits: &Limits) -> Result<(std::borrow::Cow<'a, [u8]>, bool)> {
    if !bytes.starts_with(b"PK") {
        return Ok((std::borrow::Cow::Borrowed(bytes), false));
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Malformed(format!("zipped AMF: {e}")))?;
    let several = zip
        .file_names()
        .flatten()
        .filter(|n| n.to_ascii_lowercase().ends_with(".amf"))
        .count()
        > 1;
    let name = zip
        .file_names()
        .flatten()
        .find(|n| n.to_ascii_lowercase().ends_with(".amf"))
        .or_else(|| zip.file_names().flatten().find(|n| !n.ends_with('/')))
        .map(String::from)
        .ok_or_else(|| Error::Malformed("zipped AMF without a file".into()))?;
    let f = zip
        .by_name(&name)
        .map_err(|e| Error::Malformed(format!("{name}: {e}")))?;
    let mut out = Vec::new();
    f.take(limits.max_unpacked as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::Malformed(format!("{name}: {e}")))?;
    if out.len() > limits.max_unpacked {
        return Err(Error::TooLarge("zipped AMF unpacks too large".into()));
    }
    Ok((std::borrow::Cow::Owned(out), several))
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.local_name().as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).trim().to_owned())
}

/// Where a `<color>` being read belongs.
#[derive(Clone, Copy, PartialEq)]
enum ColorOf {
    Object,
    Volume,
    Vertex,
    Triangle,
    Material,
}

fn parse(xml: &[u8], limits: &Limits) -> Result<Doc> {
    let mut r = quick_xml::Reader::from_reader(xml);
    let mut doc = Doc {
        unit: Unit::Millimetre,
        ..Doc::default()
    };
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut text = String::new();
    let mut obj: Option<(String, Object)> = None;
    let mut vol: Option<Volume> = None;
    let mut vert = [0f32; 3];
    let mut vert_color: Option<Rgba> = None;
    let mut tri = [u32::MAX; 3];
    let mut tri_color: Option<Rgba> = None;
    let mut mat: Option<(String, Option<Rgba>)> = None;
    let mut cons: Option<(String, Vec<Instance>)> = None;
    let mut inst: Option<Instance> = None;
    let mut color = [f32::NAN, f32::NAN, f32::NAN, 1.0];
    let mut color_of: Option<ColorOf> = None;
    let mut meta_name: Option<String> = None;
    let mut seen_root = false;
    let (mut verts, mut tris) = (0usize, 0usize);
    loop {
        let ev = r
            .read_event()
            .map_err(|e| Error::Malformed(format!("AMF XML: {e}")))?;
        let (start, empty) = match &ev {
            Event::Start(e) => (Some(e.clone()), false),
            Event::Empty(e) => (Some(e.clone()), true),
            _ => (None, false),
        };
        if let Some(e) = start {
            let name = e.local_name().as_ref().to_vec();
            text.clear();
            let parent = stack.last().map(Vec::as_slice);
            match name.as_slice() {
                b"amf" => {
                    seen_root = true;
                    doc.unit = match attr(&e, b"unit").as_deref().map(str::to_ascii_lowercase) {
                        Some(u) => match u.as_str() {
                            "inch" => Unit::Inch,
                            "feet" | "foot" => Unit::Foot,
                            "meter" | "metre" => Unit::Metre,
                            "micron" | "micrometer" | "micrometre" => Unit::Micron,
                            _ => Unit::Millimetre,
                        },
                        None => Unit::Millimetre,
                    };
                }
                b"object" => {
                    obj = Some((attr(&e, b"id").unwrap_or_default(), Object::default()));
                }
                b"vertex" if obj.is_some() => {
                    vert = [0.0; 3];
                    vert_color = None;
                }
                b"volume" if obj.is_some() => {
                    vol = Some(Volume {
                        material: attr(&e, b"materialid"),
                        ..Volume::default()
                    });
                }
                b"triangle" if vol.is_some() => {
                    tri = [u32::MAX; 3];
                    tri_color = None;
                }
                b"edge" => doc.curved = true,
                b"material" => mat = Some((attr(&e, b"id").unwrap_or_default(), None)),
                b"constellation" => {
                    cons = Some((attr(&e, b"id").unwrap_or_default(), Vec::new()));
                }
                b"instance" if cons.is_some() => {
                    inst = Some(Instance {
                        object: attr(&e, b"objectid").unwrap_or_default(),
                        delta: [0.0; 3],
                        rot: [0.0; 3],
                    });
                }
                b"color" => {
                    color = [f32::NAN, f32::NAN, f32::NAN, 1.0];
                    color_of = match parent {
                        Some(b"vertex") => Some(ColorOf::Vertex),
                        Some(b"triangle") => Some(ColorOf::Triangle),
                        Some(b"volume") => Some(ColorOf::Volume),
                        Some(b"object") => Some(ColorOf::Object),
                        Some(b"material") => Some(ColorOf::Material),
                        _ => None,
                    };
                }
                b"metadata" => meta_name = attr(&e, b"type").or_else(|| attr(&e, b"name")),
                _ => {}
            }
            if !empty {
                if stack.len() > 64 {
                    return bad("AMF nested too deeply");
                }
                stack.push(name);
            }
            continue;
        }
        match ev {
            Event::Text(t) => text.push_str(&t.decode().unwrap_or_default()),
            Event::CData(t) => text.push_str(&String::from_utf8_lossy(&t)),
            Event::End(_) => {
                let Some(name) = stack.pop() else {
                    return bad("AMF XML: unbalanced end tag");
                };
                let parent = stack.last().map(Vec::as_slice);
                let num = || text.trim().parse::<f64>().ok().filter(|v| v.is_finite());
                match (name.as_slice(), parent) {
                    (b"x" | b"y" | b"z", Some(b"coordinates")) => {
                        let k = (name[0] - b'x') as usize;
                        vert[k] = num().unwrap_or(f64::NAN) as f32;
                    }
                    (b"vertex", _) => {
                        if let Some((_, o)) = obj.as_mut() {
                            verts += 1;
                            if verts > limits.max_vertices {
                                return Err(Error::TooLarge(format!(
                                    "more than {} vertices",
                                    limits.max_vertices
                                )));
                            }
                            o.verts.push(vert);
                            o.any_vert_color |= vert_color.is_some();
                            o.vert_colors.push(vert_color);
                        }
                    }
                    (b"v1" | b"v2" | b"v3", Some(b"triangle")) => {
                        let k = (name[1] - b'1') as usize;
                        tri[k] = text.trim().parse::<u32>().unwrap_or(u32::MAX);
                    }
                    (b"triangle", _) => {
                        if let Some(v) = vol.as_mut() {
                            tris += 1;
                            if tris > limits.max_triangles {
                                return Err(Error::TooLarge(format!(
                                    "more than {} triangles",
                                    limits.max_triangles
                                )));
                            }
                            v.tris.push(tri);
                            if tri_color.is_some() && v.tri_colors.len() < v.tris.len() {
                                v.tri_colors.resize(v.tris.len() - 1, None);
                            }
                            if tri_color.is_some() || !v.tri_colors.is_empty() {
                                v.tri_colors.push(tri_color);
                            }
                        }
                    }
                    (b"volume", _) => {
                        if let (Some(v), Some((_, o))) = (vol.take(), obj.as_mut()) {
                            o.volumes.push(v);
                        }
                    }
                    (b"object", _) => {
                        if let Some((id, o)) = obj.take() {
                            if !doc.objects.contains_key(&id) {
                                doc.order.push(id.clone());
                            }
                            doc.objects.insert(id, o);
                        }
                    }
                    (b"r" | b"g" | b"b" | b"a", Some(b"color")) => {
                        let k = match name[0] {
                            b'r' => 0,
                            b'g' => 1,
                            b'b' => 2,
                            _ => 3,
                        };
                        if let Some(v) = num() {
                            color[k] = v.clamp(0.0, 1.0) as f32;
                        }
                    }
                    (b"color", _) => {
                        let c = color.iter().all(|v| v.is_finite()).then_some(color);
                        match color_of {
                            Some(ColorOf::Vertex) => vert_color = c,
                            Some(ColorOf::Triangle) => tri_color = c,
                            Some(ColorOf::Volume) => {
                                if let Some(v) = vol.as_mut() {
                                    v.color = c;
                                }
                            }
                            Some(ColorOf::Object) => {
                                if let Some((_, o)) = obj.as_mut() {
                                    o.color = c;
                                }
                            }
                            Some(ColorOf::Material) => {
                                if let Some((_, m)) = mat.as_mut() {
                                    *m = c;
                                }
                            }
                            None => {}
                        }
                        color_of = None;
                    }
                    (b"material", _) => {
                        if let Some((id, Some(c))) = mat.take() {
                            doc.materials.insert(id, c);
                        }
                    }
                    (b"deltax" | b"deltay" | b"deltaz", Some(b"instance")) => {
                        if let Some(i) = inst.as_mut() {
                            i.delta[(name[5] - b'x') as usize] = num().unwrap_or(0.0);
                        }
                    }
                    (b"rx" | b"ry" | b"rz", Some(b"instance")) => {
                        if let Some(i) = inst.as_mut() {
                            i.rot[(name[1] - b'x') as usize] = num().unwrap_or(0.0);
                        }
                    }
                    (b"instance", _) => {
                        if let (Some(i), Some((_, c))) = (inst.take(), cons.as_mut()) {
                            c.push(i);
                        }
                    }
                    (b"constellation", _) => {
                        if let Some((id, c)) = cons.take() {
                            doc.constellations.insert(id, c);
                        }
                    }
                    (b"metadata", Some(b"amf"))
                        if meta_name
                            .as_deref()
                            .is_some_and(|n| n.eq_ignore_ascii_case("name"))
                            && !text.trim().is_empty() =>
                    {
                        doc.title = Some(text.trim().to_owned());
                    }
                    _ => {}
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root {
        return bad("not an AMF document (no <amf> element)");
    }
    Ok(doc)
}

/// Rotation about x, then y, then z (degrees), then the translation.
fn placement(i: &Instance) -> Affine {
    let [a, b, c] = i.rot.map(f64::to_radians);
    let rx = [
        [1.0, 0.0, 0.0],
        [0.0, a.cos(), -a.sin()],
        [0.0, a.sin(), a.cos()],
    ];
    let ry = [
        [b.cos(), 0.0, b.sin()],
        [0.0, 1.0, 0.0],
        [-b.sin(), 0.0, b.cos()],
    ];
    let rz = [
        [c.cos(), -c.sin(), 0.0],
        [c.sin(), c.cos(), 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mul = |p: [[f64; 3]; 3], q: [[f64; 3]; 3]| {
        let mut m = [[0.0; 3]; 3];
        for (r, row) in m.iter_mut().enumerate() {
            for (k, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|j| p[r][j] * q[j][k]).sum();
            }
        }
        m
    };
    let m = mul(rz, mul(ry, rx));
    Affine([0, 1, 2].map(|r| [m[r][0], m[r][1], m[r][2], i.delta[r]]))
}

/// Deepest nesting of constellations.
const MAX_NESTING: usize = 16;

/// Constellation instances visited per file, at most (empty constellations add nothing to
/// the output, so the output's size alone does not bound the work).
const MAX_VISITS: usize = 1_000_000;

/// Every (object, placement) the constellations put in the build; `visits` counts the
/// instances looked at.
fn expand(
    doc: &Doc,
    id: &str,
    xf: &Affine,
    depth: usize,
    out: &mut Vec<(String, Affine)>,
    visits: &mut usize,
) -> Result<()> {
    if depth > MAX_NESTING {
        return bad("AMF constellations nested too deeply");
    }
    *visits += 1;
    if out.len() > 1_000_000 || *visits > MAX_VISITS {
        return Err(Error::TooLarge("too many AMF instances".into()));
    }
    for i in doc.constellations.get(id).into_iter().flatten() {
        let x = xf.then_after(&placement(i));
        if doc.constellations.contains_key(&i.object) {
            expand(doc, &i.object, &x, depth + 1, out, visits)?;
        } else {
            out.push((i.object.clone(), x));
        }
    }
    Ok(())
}

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Mesh> {
    let (xml, several) = document(bytes, limits)?;
    let doc = parse(&xml, limits)?;
    drop(xml);
    // The build: the constellations no other constellation places, else every object.
    let mut placed: Vec<(String, Affine)> = Vec::new();
    if doc.constellations.is_empty() {
        placed.extend(doc.order.iter().map(|o| (o.clone(), Affine::IDENTITY)));
    } else {
        let inner: std::collections::HashSet<&str> = doc
            .constellations
            .values()
            .flatten()
            .map(|i| i.object.as_str())
            .collect();
        let mut roots: Vec<&String> = doc
            .constellations
            .keys()
            .filter(|k| !inner.contains(k.as_str()))
            .collect();
        roots.sort();
        let mut visits = 0;
        for r in roots {
            expand(&doc, r, &Affine::IDENTITY, 0, &mut placed, &mut visits)?;
        }
    }
    let mut b = Builder::new(limits);
    b.unit = doc.unit;
    b.up = Up::Z;
    b.flat = true;
    b.title = doc.title.clone();
    let mut mats: HashMap<[u32; 4], u32> = HashMap::new();
    let mut counts: Vec<u32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut corner_colors: Vec<[u8; 4]> = Vec::new();
    let to8 = |c: Rgba| c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
    for (id, xf) in &placed {
        let Some(o) = doc.objects.get(id) else {
            b.note_once("a constellation places an object the file does not have");
            continue;
        };
        for v in &o.volumes {
            let base = v
                .color
                .or_else(|| {
                    v.material
                        .as_ref()
                        .and_then(|m| doc.materials.get(m))
                        .copied()
                })
                .or(o.color);
            counts.clear();
            counts.resize(v.tris.len(), 3);
            idx.clear();
            idx.extend(v.tris.iter().flatten());
            let mut p = Poly::new(&o.verts, &counts, &idx);
            let per_corner = o.any_vert_color || !v.tri_colors.is_empty();
            if per_corner {
                corner_colors.clear();
                for (t, tri) in v.tris.iter().enumerate() {
                    let tc = v.tri_colors.get(t).copied().flatten();
                    for &k in tri {
                        let vc = o.vert_colors.get(k as usize).copied().flatten();
                        corner_colors.push(to8(tc.or(vc).or(base).unwrap_or([1.0; 4])));
                    }
                }
                p.colors = Attr::Corner(&corner_colors);
            } else if let Some(c) = base {
                let key = c.map(f32::to_bits);
                let m = *mats.entry(key).or_insert_with(|| {
                    b.add_material(Material {
                        name: v.material.clone().unwrap_or_default(),
                        base_color: [
                            srgb_to_linear(c[0]),
                            srgb_to_linear(c[1]),
                            srgb_to_linear(c[2]),
                            c[3],
                        ],
                        texture: None,
                    })
                });
                p.material = Some(m);
            }
            emit(&mut b, &p, xf)?;
        }
    }
    if several {
        b.note("the package holds several AMF files; the first is shown");
    }
    if doc.curved {
        b.note("curved triangle edges are drawn flat");
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<amf unit="inch" version="1.1">
  <metadata type="name">Two cubes</metadata>
  <material id="2"><color><r>1</r><g>0</g><b>0</b></color></material>
  <object id="1">
    <mesh>
      <vertices>
        <vertex><coordinates><x>0</x><y>0</y><z>0</z></coordinates></vertex>
        <vertex><coordinates><x>1</x><y>0</y><z>0</z></coordinates></vertex>
        <vertex><coordinates><x>1</x><y>1</y><z>0</z></coordinates></vertex>
        <vertex><coordinates><x>0</x><y>1</y><z>0</z></coordinates></vertex>
        <vertex><coordinates><x>0</x><y>0</y><z>1</z></coordinates></vertex>
        <vertex><coordinates><x>1</x><y>0</y><z>1</z></coordinates></vertex>
        <vertex><coordinates><x>1</x><y>1</y><z>1</z></coordinates></vertex>
        <vertex><coordinates><x>0</x><y>1</y><z>1</z></coordinates></vertex>
      </vertices>
      <volume materialid="2">
        <triangle><v1>0</v1><v2>2</v2><v3>1</v3></triangle>
        <triangle><v1>0</v1><v2>3</v2><v3>2</v3></triangle>
        <triangle><v1>4</v1><v2>5</v2><v3>6</v3></triangle>
        <triangle><v1>4</v1><v2>6</v2><v3>7</v3></triangle>
        <triangle><v1>0</v1><v2>1</v2><v3>5</v3></triangle>
        <triangle><v1>0</v1><v2>5</v2><v3>4</v3></triangle>
      </volume>
      <volume>
        <color><r>0</r><g>0</g><b>1</b><a>0.5</a></color>
        <triangle><v1>1</v1><v2>2</v2><v3>6</v3></triangle>
        <triangle><v1>1</v1><v2>6</v2><v3>5</v3></triangle>
        <triangle><v1>2</v1><v2>3</v2><v3>7</v3></triangle>
        <triangle><v1>2</v1><v2>7</v2><v3>6</v3></triangle>
        <triangle><v1>3</v1><v2>0</v2><v3>4</v3></triangle>
        <triangle><v1>3</v1><v2>4</v2><v3>7</v3></triangle>
      </volume>
    </mesh>
  </object>
  <constellation id="10">
    <instance objectid="1"><deltax>0</deltax><deltay>0</deltay><deltaz>0</deltaz></instance>
    <instance objectid="1"><deltax>5</deltax><rz>90</rz></instance>
  </constellation>
</amf>"#;

    #[test]
    fn objects_volumes_colours_and_constellations() {
        let m = read(CUBE.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(m.triangle_count(), 24);
        assert_eq!(m.unit, Unit::Inch);
        assert_eq!(m.title.as_deref(), Some("Two cubes"));
        assert_eq!(m.materials.len(), 2);
        assert_eq!(m.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(m.materials[1].base_color[3], 0.5);
        // The second instance turned 90° about z and moved 5 along x: x 4‥5, y 0‥1.
        let b = m.bounds;
        assert!(
            (b.min[0] - 0.0).abs() < 1e-5 && (b.max[0] - 5.0).abs() < 1e-5,
            "{b:?}"
        );
        assert!((b.max[1] - 1.0).abs() < 1e-5 && (b.max[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn vertex_and_triangle_colours() {
        let src = r#"<amf><object id="0"><mesh><vertices>
          <vertex><coordinates><x>0</x><y>0</y><z>0</z></coordinates><color><r>0</r><g>1</g><b>0</b></color></vertex>
          <vertex><coordinates><x>1</x><y>0</y><z>0</z></coordinates></vertex>
          <vertex><coordinates><x>0</x><y>1</y><z>0</z></coordinates></vertex>
          <vertex><coordinates><x>0</x><y>0</y><z>1</z></coordinates></vertex>
        </vertices><volume>
          <triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle>
          <triangle><color><r>1</r><g>0</g><b>0</b></color><v1>0</v1><v2>1</v2><v3>3</v3></triangle>
          <triangle><v1>0</v1><v2>1</v2><v3>9</v3><edge><v1>0</v1><dx1>0</dx1></edge></triangle>
        </volume></mesh></object></amf>"#;
        let m = read(src.as_bytes(), &Limits::default()).unwrap();
        assert_eq!(m.unit, Unit::Millimetre);
        assert_eq!(m.triangle_count(), 2);
        let c = m.colors.as_ref().unwrap();
        assert!(c.contains(&[0, 255, 0, 255]) && c.contains(&[255, 0, 0, 255]));
        assert!(
            m.notes.iter().any(|n| n.contains("curved")),
            "{:?}",
            m.notes
        );
        assert!(
            m.notes.iter().any(|n| n.contains("bad indices")),
            "{:?}",
            m.notes
        );
    }

    #[test]
    fn zipped() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            w.start_file("cube.amf", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut w, CUBE.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        let m = read(buf.get_ref(), &Limits::default()).unwrap();
        assert_eq!(m.triangle_count(), 24);
    }

    /// The AMF corpus against the oracle's small reader and assimp (gen/amf_oracle.py); the
    /// zipped copies of the 20 mm box (which neither reads) against the plain one.
    #[test]
    fn corpus_against_the_oracles() {
        let dir = std::env::var_os("SPECULAR_MODELS_EXTRA_CORPUS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../tools/corpus/models-extra"
                ))
            });
        let Ok(tsv) = std::fs::read_to_string(dir.join("oracle/amf.tsv")) else {
            eprintln!("skipped: no AMF oracle under {}", dir.display());
            return;
        };
        let mut facts: HashMap<String, HashMap<String, String>> = HashMap::new();
        for l in tsv.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() == 3 {
                facts
                    .entry(f[0].to_owned())
                    .or_default()
                    .insert(f[1].to_owned(), f[2].to_owned());
            }
        }
        let v3 = |s: &str| -> Vec<f32> { s.split(',').map(|x| x.parse().unwrap()).collect() };
        let near = |a: [f32; 3], b: &[f32]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
        let read_file =
            |rel: &str| read(&std::fs::read(dir.join(rel)).unwrap(), &Limits::default());
        let mut mismatches = Vec::new();
        let mut compared = 0;
        let mut rels: Vec<&String> = facts.keys().collect();
        rels.sort();
        for rel in rels {
            let f = &facts[rel];
            let Some(tris) = f.get("tris").filter(|t| *t != "0") else {
                continue;
            };
            let m = match read_file(rel) {
                Ok(m) => m,
                Err(e) => {
                    mismatches.push(format!("{rel}: {e}"));
                    continue;
                }
            };
            // The small reader always; assimp where it reads the file whole.
            let mut against = vec![("", tris)];
            if f.get("assimp_comparable").is_some_and(|c| c == "1")
                && let Some(t) = f.get("assimp_tris")
            {
                against.push(("assimp_", t));
            }
            for (key, tris) in against {
                compared += 1;
                let (lo, hi) = (&f[&format!("{key}min")], &f[&format!("{key}max")]);
                let ok = m.triangle_count().to_string() == *tris
                    && near(m.bounds.min, &v3(lo))
                    && near(m.bounds.max, &v3(hi));
                if !ok {
                    mismatches.push(format!(
                        "{rel}: {} triangles {:?}; {key}oracle {tris} {lo} {hi}",
                        m.triangle_count(),
                        m.bounds,
                    ));
                }
            }
        }
        let plain = read_file("amf/slic3r-20mmbox.amf").unwrap();
        for z in [
            "amf/slic3r-20mmbox_deflated.amf",
            "amf/slic3r-20mmbox_deflated-in_directories.amf",
            "amf/slic3r-20mmbox_deflated-mult_files.amf",
        ] {
            compared += 1;
            match read_file(z) {
                Ok(m)
                    if m.triangle_count() == plain.triangle_count() && m.bounds == plain.bounds => {
                }
                other => mismatches.push(format!("{z}: {:?}", other.map(|m| m.bounds))),
            }
        }
        assert!(read_file("amf/slic3r-5061-malicious.amf").is_err());
        let r = read_file("amf/slic3r-read-amf.amf").unwrap();
        assert_eq!((r.triangle_count(), r.unit), (2, Unit::Inch));
        eprintln!(
            "amf corpus: {compared} compared, {} mismatches",
            mismatches.len()
        );
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    #[test]
    fn junk_is_an_error() {
        for src in [
            &b"<html/>"[..],
            b"<amf><object id='1'><mesh><vertices></vertices></mesh></object></amf>",
            b"<amf><constellation id='a'><instance objectid='a'/></constellation></amf>",
            b"<amf><object",
            b"PK\x03\x04junk",
        ] {
            assert!(read(src, &Limits::default()).is_err());
        }
    }
}
