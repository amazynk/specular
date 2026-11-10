//! Small generated models in every format, for tests here and in the crates that show
//! meshes (no fixture files are kept in the tree).

use std::fmt::Write as _;
use std::io::Write as _;

/// A unit cube's eight corners and twelve counter-clockwise (outward) triangles.
pub fn cube() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let v = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let t = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    (v, t)
}

/// Binary STL of a cube scaled by `s`.
pub fn stl_binary(s: f32) -> Vec<u8> {
    let (v, t) = cube();
    let mut out = vec![0u8; 80];
    out[..10].copy_from_slice(b"cube model");
    out.extend_from_slice(&(t.len() as u32).to_le_bytes());
    for tri in &t {
        out.extend_from_slice(&[0u8; 12]);
        for &i in tri {
            for c in v[i as usize] {
                out.extend_from_slice(&(c * s).to_le_bytes());
            }
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

pub fn stl_ascii() -> Vec<u8> {
    let (v, t) = cube();
    let mut s = String::from("solid cube\n");
    for tri in &t {
        s.push_str("  facet normal 0 0 0\n    outer loop\n");
        for &i in tri {
            let p = v[i as usize];
            let _ = writeln!(s, "      vertex {} {} {}", p[0], p[1], p[2]);
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    s.push_str("endsolid cube\n");
    s.into_bytes()
}

/// OBJ of a cube as quads with texture coordinates and a material from `cube.mtl`.
pub fn obj() -> Vec<u8> {
    let (v, _) = cube();
    let mut s = String::from("# cube\nmtllib cube.mtl\no Cube\n");
    for p in &v {
        let _ = writeln!(s, "v {} {} {}", p[0], p[1], p[2]);
    }
    s.push_str("vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n");
    s.push_str("usemtl red\n");
    for q in [[1, 4, 3, 2], [5, 6, 7, 8]] {
        let _ = writeln!(s, "f {}/1 {}/2 {}/3 {}/4", q[0], q[1], q[2], q[3]);
    }
    s.push_str("usemtl textured\n");
    // Negative indices count back from the last vertex (8).
    for q in [[1, 2, 6, 5], [2, 3, 7, 6], [3, 4, 8, 7], [4, 1, 5, 8]] {
        let r = q.map(|i: i32| i - 9);
        let _ = writeln!(s, "f {}/1 {}/2 {}/3 {}/4", r[0], r[1], r[2], r[3]);
    }
    s.into_bytes()
}

pub fn mtl() -> Vec<u8> {
    b"newmtl red\nKd 1 0 0\nd 1\n\nnewmtl textured\nKd 1 1 1\nmap_Kd -s 1 1 1 tex.png\n".to_vec()
}

/// PLY of a cube with vertex colours: `ascii`, `binary_little_endian` or
/// `binary_big_endian`.
pub fn ply(format: &str) -> Vec<u8> {
    let (v, t) = cube();
    let mut out = format!(
        "ply\nformat {format} 1.0\ncomment made by tests\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face {}\nproperty list uchar int vertex_indices\nelement extra 1\nproperty list uchar float stuff\nend_header\n",
        v.len(),
        t.len()
    )
    .into_bytes();
    let col = |i: usize| [(i * 30) as u8, 128, (255 - i * 30) as u8];
    let be = format == "binary_big_endian";
    match format {
        "ascii" => {
            let mut s = String::new();
            for (i, p) in v.iter().enumerate() {
                let c = col(i);
                let _ = writeln!(s, "{} {} {} {} {} {}", p[0], p[1], p[2], c[0], c[1], c[2]);
            }
            for tri in &t {
                let _ = writeln!(s, "3 {} {} {}", tri[0], tri[1], tri[2]);
            }
            s.push_str("2 0.5 1.5\n");
            out.extend_from_slice(s.as_bytes());
        }
        _ => {
            let f = |x: f32| if be { x.to_be_bytes() } else { x.to_le_bytes() };
            let n = |x: u32| if be { x.to_be_bytes() } else { x.to_le_bytes() };
            for (i, p) in v.iter().enumerate() {
                for c in p {
                    out.extend_from_slice(&f(*c));
                }
                out.extend_from_slice(&col(i));
            }
            for tri in &t {
                out.push(3);
                for &i in tri {
                    out.extend_from_slice(&n(i));
                }
            }
            out.push(2);
            out.extend_from_slice(&f(0.5));
            out.extend_from_slice(&f(1.5));
        }
    }
    out
}

fn glb_bin() -> (Vec<u8>, usize, usize) {
    let (v, t) = cube();
    let mut bin = Vec::new();
    for p in &v {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    let pos_len = bin.len();
    for tri in &t {
        for &i in tri {
            bin.extend_from_slice(&(i as u16).to_le_bytes());
        }
    }
    let idx_len = bin.len() - pos_len;
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    (bin, pos_len, idx_len)
}

fn gltf_json(buffer_uri: Option<&str>, bin_len: usize, pos_len: usize, idx_len: usize) -> String {
    let uri = buffer_uri.map_or(String::new(), |u| format!("\"uri\": \"{u}\", "));
    format!(
        r#"{{
  "asset": {{"version": "2.0", "generator": "specular tests"}},
  "scene": 0,
  "scenes": [{{"name": "Cubes", "nodes": [0, 1]}}],
  "nodes": [
    {{"mesh": 0, "translation": [2, 0, 0]}},
    {{"children": [2], "scale": [2, 2, 2]}},
    {{"mesh": 0, "rotation": [0, 0, 0.7071068, 0.7071068]}}
  ],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0}}, "indices": 1, "material": 0}}]}}],
  "materials": [{{"name": "Blue", "pbrMetallicRoughness": {{"baseColorFactor": [0.1, 0.2, 0.9, 1.0]}}}}],
  "buffers": [{{{uri}"byteLength": {bin_len}}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": {pos_len}}},
    {{"buffer": 0, "byteOffset": {pos_len}, "byteLength": {idx_len}}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 8, "type": "VEC3", "min": [0,0,0], "max": [1,1,1]}},
    {{"bufferView": 1, "componentType": 5123, "count": 36, "type": "SCALAR"}}
  ]
}}"#
    )
}

/// glTF with its buffer embedded as a base64 `data:` URI: two cubes, one moved by +2 in x,
/// one scaled by 2 and turned 90° about z.
pub fn gltf_embedded() -> Vec<u8> {
    let (bin, p, i) = glb_bin();
    let uri = format!("data:application/octet-stream;base64,{}", base64(&bin));
    gltf_json(Some(&uri), bin.len(), p, i).into_bytes()
}

/// The same with the buffer in `cube.bin` beside it: (gltf, bin).
pub fn gltf_side() -> (Vec<u8>, Vec<u8>) {
    let (bin, p, i) = glb_bin();
    (
        gltf_json(Some("cube.bin"), bin.len(), p, i).into_bytes(),
        bin,
    )
}

pub fn glb() -> Vec<u8> {
    let (bin, p, i) = glb_bin();
    let mut json = gltf_json(None, bin.len(), p, i).into_bytes();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                s.push(A[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// 3MF of a cube in centimetres placed twice (the second moved +3 in x), with a red base
/// material on the cube and a colour group on two triangles.
pub fn threemf() -> Vec<u8> {
    let (v, t) = cube();
    let mut model = String::from(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="centimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:m="http://schemas.microsoft.com/3dmanufacturing/material/2015/02">
 <metadata name="Title">Cube &amp; Co</metadata>
 <resources>
  <basematerials id="1"><base name="Red" displaycolor="#FF0000"/></basematerials>
  <m:colorgroup id="2"><m:color color="#00FF00"/><m:color color="#0000FFFF"/></m:colorgroup>
  <object id="3" type="model" pid="1" pindex="0"><mesh><vertices>
"##,
    );
    for p in &v {
        let _ = writeln!(
            model,
            "   <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
            p[0], p[1], p[2]
        );
    }
    model.push_str("  </vertices><triangles>\n");
    for (k, tri) in t.iter().enumerate() {
        let extra = if k < 2 {
            " pid=\"2\" p1=\"0\" p2=\"1\" p3=\"0\""
        } else {
            ""
        };
        let _ = writeln!(
            model,
            "   <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"{extra}/>",
            tri[0], tri[1], tri[2]
        );
    }
    model.push_str(
        r#"  </triangles></mesh></object>
  <object id="4" type="model"><components><component objectid="3" transform="1 0 0 0 1 0 0 0 1 3 0 0"/></components></object>
 </resources>
 <build><item objectid="3"/><item objectid="4"/></build>
</model>
"#,
    );
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>
"#;
    let types = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>
"#;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in [
        ("[Content_Types].xml", types),
        ("_rels/.rels", rels),
        ("3D/3dmodel.model", model.as_str()),
    ] {
        let _ = zip.start_file(name, opts);
        let _ = zip.write_all(data.as_bytes());
    }
    zip.finish().map(|c| c.into_inner()).unwrap_or_default()
}

/// A colourful torus knot, `segments` around and `sides` around the tube: a mesh with
/// curvature, self-occlusion and vertex colours for screenshots and timings.
pub fn torus_knot(segments: usize, sides: usize) -> crate::Mesh {
    use std::f32::consts::TAU;
    let (p, q) = (2.0f32, 3.0f32);
    let centre = |t: f32| {
        let r = 2.0 + (q * t).cos();
        [r * (p * t).cos(), r * (p * t).sin(), -(q * t).sin()]
    };
    let segments = segments.max(3);
    let sides = sides.max(3);
    let mut positions = Vec::with_capacity(segments * sides);
    let mut colors = Vec::with_capacity(segments * sides);
    for i in 0..segments {
        let t = i as f32 / segments as f32 * TAU;
        let c = centre(t);
        let d = crate::builder::sub(centre(t + 1e-3), centre(t - 1e-3));
        let dl = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let tang = d.map(|x| x / dl);
        // A normal frame: any vector not along the tangent.
        let up = if tang[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let n1 = crate::builder::cross(tang, up);
        let l1 = (n1[0] * n1[0] + n1[1] * n1[1] + n1[2] * n1[2]).sqrt();
        let n1 = n1.map(|x| x / l1);
        let n2 = crate::builder::cross(tang, n1);
        let hue = i as f32 / segments as f32;
        let rgb = [0.0f32, 1.0 / 3.0, 2.0 / 3.0]
            .map(|o| (((hue + o) * TAU).cos() * 0.5 + 0.5) * 200.0 + 40.0);
        for j in 0..sides {
            let a = j as f32 / sides as f32 * TAU;
            let (s, co) = a.sin_cos();
            let r = 0.45;
            positions.push([0, 1, 2].map(|k| c[k] + r * (co * n1[k] + s * n2[k])));
            colors.push([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 255]);
        }
    }
    let mut indices = Vec::with_capacity(segments * sides * 6);
    for i in 0..segments {
        let i2 = (i + 1) % segments;
        for j in 0..sides {
            let j2 = (j + 1) % sides;
            let a = (i * sides + j) as u32;
            let b = (i2 * sides + j) as u32;
            let c = (i2 * sides + j2) as u32;
            let d = (i * sides + j2) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    let mut b = crate::builder::Builder::new(&crate::Limits::default());
    b.positions = positions;
    b.colors = Some(colors);
    b.indices = indices;
    b.unit = crate::Unit::Millimetre;
    b.up = crate::Up::Z;
    b.normals_given(false);
    let mut m = b.finish().unwrap_or_default();
    m.title = Some("Torus knot".into());
    m
}

/// The cube as quads: (corners, faces of four, counter-clockwise outward).
fn cube_quads() -> (Vec<[f32; 3]>, Vec<[u32; 4]>) {
    let (v, _) = cube();
    let q = vec![
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    (v, q)
}

/// A USDA stage: a cube mesh defined in a class and referenced twice (the second through a
/// variant that moves it by 3 in x), a material bound with a diffuse colour, z up, mm.
pub fn usda() -> Vec<u8> {
    let (v, q) = cube_quads();
    let pts: Vec<String> = v
        .iter()
        .map(|p| format!("({}, {}, {})", p[0], p[1], p[2]))
        .collect();
    let idx: Vec<String> = q.iter().flatten().map(u32::to_string).collect();
    format!(
        r#"#usda 1.0
(
    defaultPrim = "World"
    metersPerUnit = 0.001
    upAxis = "Z"
    customLayerData = {{
        string creator = "fixture writer"
    }}
)

class "Proto"
{{
    def Mesh "Box"
    {{
        point3f[] points = [{}]
        int[] faceVertexCounts = [4, 4, 4, 4, 4, 4]
        int[] faceVertexIndices = [{}]
        rel material:binding = </World/Looks/Red>
    }}
}}

def Xform "World"
{{
    def Scope "Looks"
    {{
        def Material "Red"
        {{
            token outputs:surface.connect = </World/Looks/Red/S.outputs:surface>
            def Shader "S"
            {{
                uniform token info:id = "UsdPreviewSurface"
                color3f inputs:diffuseColor = (1, 0, 0)
                token outputs:surface
            }}
        }}
    }}
    def "A" (
        prepend references = </Proto>
    )
    {{
    }}
    def Xform "B" (
        variants = {{
            string place = "moved"
        }}
        prepend variantSets = "place"
    )
    {{
        variantSet "place" = {{
            "moved" {{
                double3 xformOp:translate = (3, 0, 0)
                uniform token[] xformOpOrder = ["xformOp:translate"]
            }}
            "home" {{
            }}
        }}
        def "C" (
            prepend references = </Proto>
        )
        {{
        }}
    }}
    def Mesh "Hidden"
    {{
        token visibility = "invisible"
        point3f[] points = [(9, 9, 9), (10, 9, 9), (9, 10, 9)]
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
    }}
}}
"#,
        pts.join(", "),
        idx.join(", ")
    )
    .into_bytes()
}

/// The USDA stage in a USDZ package.
pub fn usdz() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let _ = zip.start_file("scene.usda", opts);
    let _ = zip.write_all(&usda());
    zip.finish().map(|c| c.into_inner()).unwrap_or_default()
}

/// A COLLADA document: the cube as a polylist with normals per corner, a phong material,
/// instanced twice (the second translated by 3 in x and scaled by 2), Z up, centimetres.
pub fn dae() -> Vec<u8> {
    let (v, q) = cube_quads();
    let pos: Vec<String> = v.iter().flatten().map(|c| c.to_string()).collect();
    let mut p = String::new();
    for (f, quad) in q.iter().enumerate() {
        for &i in quad {
            let _ = write!(p, "{i} {f} ");
        }
    }
    let normals = "0 0 -1 0 0 1 0 -1 0 1 0 0 0 1 0 -1 0 0";
    format!(
        r##"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <asset><contributor><authoring_tool>fixture writer</authoring_tool></contributor>
    <title>Cubes</title><unit name="centimeter" meter="0.01"/><up_axis>Z_UP</up_axis></asset>
  <library_effects><effect id="fx"><profile_COMMON><technique sid="t"><phong>
    <diffuse><color>0 0.5 1 1</color></diffuse></phong></technique></profile_COMMON></effect></library_effects>
  <library_materials><material id="blue" name="Blue"><instance_effect url="#fx"/></material></library_materials>
  <library_geometries><geometry id="cube"><mesh>
    <source id="pos"><float_array id="pa" count="24">{}</float_array>
      <technique_common><accessor source="#pa" count="8" stride="3"/></technique_common></source>
    <source id="nrm"><float_array id="na" count="18">{normals}</float_array>
      <technique_common><accessor source="#na" count="6" stride="3"/></technique_common></source>
    <vertices id="v"><input semantic="POSITION" source="#pos"/></vertices>
    <polylist count="6" material="m">
      <input semantic="VERTEX" source="#v" offset="0"/>
      <input semantic="NORMAL" source="#nrm" offset="1"/>
      <vcount>4 4 4 4 4 4</vcount><p>{p}</p></polylist>
  </mesh></geometry></library_geometries>
  <library_visual_scenes><visual_scene id="s">
    <node id="a"><instance_geometry url="#cube"><bind_material><technique_common>
      <instance_material symbol="m" target="#blue"/></technique_common></bind_material></instance_geometry></node>
    <node id="b"><translate>3 0 0</translate><scale>2 2 2</scale><instance_geometry url="#cube"/></node>
  </visual_scene></library_visual_scenes>
  <scene><instance_visual_scene url="#s"/></scene>
</COLLADA>
"##,
        pos.join(" ")
    )
    .into_bytes()
}

fn chunk3ds(id: u16, body: &[u8]) -> Vec<u8> {
    let mut out = id.to_le_bytes().to_vec();
    out.extend_from_slice(&(body.len() as u32 + 6).to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A 3DS file: the cube object (world-space vertices, one smoothing group, a material
/// with a diffuse colour) and a keyframer node moving it by 2 in z.
pub fn three_ds() -> Vec<u8> {
    let (v, t) = cube();
    let mut verts = (v.len() as u16).to_le_bytes().to_vec();
    for p in &v {
        for c in p {
            verts.extend_from_slice(&c.to_le_bytes());
        }
    }
    let mut faces = (t.len() as u16).to_le_bytes().to_vec();
    for tri in &t {
        for &i in tri {
            faces.extend_from_slice(&(i as u16).to_le_bytes());
        }
        faces.extend_from_slice(&0u16.to_le_bytes());
    }
    let mut fm = b"Green\0".to_vec();
    fm.extend_from_slice(&(t.len() as u16).to_le_bytes());
    for k in 0..t.len() as u16 {
        fm.extend_from_slice(&k.to_le_bytes());
    }
    faces.extend(chunk3ds(0x4130, &fm));
    faces.extend(chunk3ds(0x4150, &[1u8, 0, 0, 0].repeat(t.len())));
    let mut frame = Vec::new();
    for f in [
        1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
    ] {
        frame.extend_from_slice(&f.to_le_bytes());
    }
    let mesh = [
        chunk3ds(0x4110, &verts),
        chunk3ds(0x4120, &faces),
        chunk3ds(0x4160, &frame),
    ]
    .concat();
    let mut obj = b"Box\0".to_vec();
    obj.extend(chunk3ds(0x4100, &mesh));
    let mat = [
        chunk3ds(0xA000, b"Green\0"),
        chunk3ds(0xA020, &chunk3ds(0x0011, &[0, 255, 0])),
    ]
    .concat();
    let editor = [chunk3ds(0xAFFF, &mat), chunk3ds(0x4000, &obj)].concat();
    // Keyframer: the object node, position key (0, 0, 2).
    let mut hdr = b"Box\0".to_vec();
    hdr.extend_from_slice(&[0, 0, 0, 0, 0xff, 0xff]);
    let mut track = vec![0u8; 10];
    track.extend_from_slice(&1u32.to_le_bytes());
    track.extend_from_slice(&0u32.to_le_bytes());
    track.extend_from_slice(&0u16.to_le_bytes());
    for c in [0.0f32, 0.0, 2.0] {
        track.extend_from_slice(&c.to_le_bytes());
    }
    let node = [
        chunk3ds(0xB030, &0u16.to_le_bytes()),
        chunk3ds(0xB010, &hdr),
        chunk3ds(0xB020, &track),
    ]
    .concat();
    let kf = chunk3ds(0xB002, &node);
    chunk3ds(
        0x4D4D,
        &[chunk3ds(0x3D3D, &editor), chunk3ds(0xB000, &kf)].concat(),
    )
}

fn iff(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

/// A LightWave LWO2 object: the cube's quads, clockwise as LightWave winds them, a surface
/// with a colour.
pub fn lwo() -> Vec<u8> {
    let (v, q) = cube_quads();
    let mut pnts = Vec::new();
    for p in &v {
        for c in p {
            pnts.extend_from_slice(&c.to_be_bytes());
        }
    }
    let mut pols = b"FACE".to_vec();
    for quad in &q {
        pols.extend_from_slice(&4u16.to_be_bytes());
        // Clockwise: reversed; LightWave's z is mirrored too, which turns them back.
        for &i in quad.iter().rev() {
            pols.extend_from_slice(&(i as u16).to_be_bytes());
        }
    }
    let mut ptag = b"SURF".to_vec();
    for f in 0..q.len() as u16 {
        ptag.extend_from_slice(&f.to_be_bytes());
        ptag.extend_from_slice(&0u16.to_be_bytes());
    }
    let mut colr = b"COLR".to_vec();
    colr.extend_from_slice(&14u16.to_be_bytes());
    for c in [1.0f32, 0.5, 0.0] {
        colr.extend_from_slice(&c.to_be_bytes());
    }
    colr.extend_from_slice(&0u16.to_be_bytes());
    let mut surf = b"Orange\0\0\0\0".to_vec();
    surf.extend_from_slice(&colr);
    let mut layr = vec![0u8; 16];
    layr.extend_from_slice(b"Layer\0");
    let body = [
        b"LWO2".to_vec(),
        iff(b"TAGS", b"Orange\0\0"),
        iff(b"LAYR", &layr),
        iff(b"PNTS", &pnts),
        iff(b"POLS", &pols),
        iff(b"PTAG", &ptag),
        iff(b"SURF", &surf),
    ]
    .concat();
    iff(b"FORM", &body)
}

/// An FBX property list item.
fn fbx_prop(out: &mut Vec<u8>, p: &FbxP) {
    match p {
        FbxP::I(v) => {
            out.push(b'I');
            out.extend_from_slice(&v.to_le_bytes());
        }
        FbxP::L(v) => {
            out.push(b'L');
            out.extend_from_slice(&v.to_le_bytes());
        }
        FbxP::D(v) => {
            out.push(b'D');
            out.extend_from_slice(&v.to_le_bytes());
        }
        FbxP::S(s) => {
            out.push(b'S');
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        FbxP::Ds(v) => {
            out.push(b'd');
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&(v.len() as u32 * 8).to_le_bytes());
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        FbxP::Is(v) => {
            out.push(b'i');
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&(v.len() as u32 * 4).to_le_bytes());
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
    }
}

enum FbxP {
    I(i32),
    L(i64),
    D(f64),
    S(String),
    Ds(Vec<f64>),
    Is(Vec<i32>),
}

struct FbxNode {
    name: &'static str,
    props: Vec<FbxP>,
    children: Vec<FbxNode>,
}

fn fbx_node(name: &'static str, props: Vec<FbxP>, children: Vec<FbxNode>) -> FbxNode {
    FbxNode {
        name,
        props,
        children,
    }
}

/// Writes a node record (version 7400: 32-bit offsets) at `base`.
fn fbx_write(n: &FbxNode, base: usize) -> Vec<u8> {
    let mut props = Vec::new();
    for p in &n.props {
        fbx_prop(&mut props, p);
    }
    let head = 13 + n.name.len();
    let mut body = Vec::new();
    let mut at = base + head + props.len();
    for c in &n.children {
        let b = fbx_write(c, at);
        at += b.len();
        body.extend(b);
    }
    if !n.children.is_empty() {
        body.extend_from_slice(&[0; 13]);
    }
    let end = base + head + props.len() + body.len();
    let mut out = (end as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&(n.props.len() as u32).to_le_bytes());
    out.extend_from_slice(&(props.len() as u32).to_le_bytes());
    out.push(n.name.len() as u8);
    out.extend_from_slice(n.name.as_bytes());
    out.extend(props);
    out.extend(body);
    out
}

fn p70(name: &str, kind: &str, v: [f64; 3]) -> FbxNode {
    fbx_node(
        "P",
        vec![
            FbxP::S(name.into()),
            FbxP::S(kind.into()),
            FbxP::S(String::new()),
            FbxP::S("A".into()),
            FbxP::D(v[0]),
            FbxP::D(v[1]),
            FbxP::D(v[2]),
        ],
        vec![],
    )
}

/// A binary FBX 7.4 file: the cube geometry under a model translated by 5 in x and rotated
/// 90° about z, a material with a diffuse colour, y up, metres (unit scale 100).
pub fn fbx() -> Vec<u8> {
    let (v, q) = cube_quads();
    let verts: Vec<f64> = v.iter().flatten().map(|&c| f64::from(c)).collect();
    let mut pvi = Vec::new();
    for quad in &q {
        for (k, &i) in quad.iter().enumerate() {
            pvi.push(if k == 3 { !(i as i32) } else { i as i32 });
        }
    }
    let unit = fbx_node(
        "P",
        vec![
            FbxP::S("UnitScaleFactor".into()),
            FbxP::S("double".into()),
            FbxP::S("Number".into()),
            FbxP::S(String::new()),
            FbxP::D(100.0),
        ],
        vec![],
    );
    let up = fbx_node(
        "P",
        vec![
            FbxP::S("UpAxis".into()),
            FbxP::S("int".into()),
            FbxP::S("Integer".into()),
            FbxP::S(String::new()),
            FbxP::I(1),
        ],
        vec![],
    );
    let top = vec![
        fbx_node(
            "FBXHeaderExtension",
            vec![],
            vec![fbx_node("FBXVersion", vec![FbxP::I(7400)], vec![])],
        ),
        fbx_node("Creator", vec![FbxP::S("fixture writer".into())], vec![]),
        fbx_node(
            "GlobalSettings",
            vec![],
            vec![fbx_node("Properties70", vec![], vec![unit, up])],
        ),
        fbx_node(
            "Objects",
            vec![],
            vec![
                fbx_node(
                    "Geometry",
                    vec![
                        FbxP::L(10),
                        FbxP::S("Box\0\u{1}Geometry".into()),
                        FbxP::S("Mesh".into()),
                    ],
                    vec![
                        fbx_node("Vertices", vec![FbxP::Ds(verts)], vec![]),
                        fbx_node("PolygonVertexIndex", vec![FbxP::Is(pvi)], vec![]),
                    ],
                ),
                fbx_node(
                    "Model",
                    vec![
                        FbxP::L(20),
                        FbxP::S("Box\0\u{1}Model".into()),
                        FbxP::S("Mesh".into()),
                    ],
                    vec![fbx_node(
                        "Properties70",
                        vec![],
                        vec![
                            p70("Lcl Translation", "Lcl Translation", [5.0, 0.0, 0.0]),
                            p70("Lcl Rotation", "Lcl Rotation", [0.0, 0.0, 90.0]),
                        ],
                    )],
                ),
                fbx_node(
                    "Material",
                    vec![
                        FbxP::L(30),
                        FbxP::S("Steel\0\u{1}Material".into()),
                        FbxP::S(String::new()),
                    ],
                    vec![fbx_node(
                        "Properties70",
                        vec![],
                        vec![p70("DiffuseColor", "Color", [0.5, 0.5, 0.6])],
                    )],
                ),
            ],
        ),
        fbx_node(
            "Connections",
            vec![],
            vec![
                fbx_node(
                    "C",
                    vec![FbxP::S("OO".into()), FbxP::L(10), FbxP::L(20)],
                    vec![],
                ),
                fbx_node(
                    "C",
                    vec![FbxP::S("OO".into()), FbxP::L(30), FbxP::L(20)],
                    vec![],
                ),
                fbx_node(
                    "C",
                    vec![FbxP::S("OO".into()), FbxP::L(20), FbxP::L(0)],
                    vec![],
                ),
            ],
        ),
    ];
    let mut out = b"Kaydara FBX Binary  \0\x1a\0".to_vec();
    out.extend_from_slice(&7400u32.to_le_bytes());
    for n in &top {
        let b = fbx_write(n, out.len());
        out.extend(b);
    }
    out.extend_from_slice(&[0; 13]);
    out
}

/// The same scene as ASCII FBX 7.4.
pub fn fbx_ascii() -> Vec<u8> {
    let (v, q) = cube_quads();
    let verts: Vec<String> = v.iter().flatten().map(|c| c.to_string()).collect();
    let mut pvi = Vec::new();
    for quad in &q {
        for (k, &i) in quad.iter().enumerate() {
            pvi.push(if k == 3 {
                (!(i as i32)).to_string()
            } else {
                i.to_string()
            });
        }
    }
    format!(
        r#"; FBX 7.4.0 project file
FBXHeaderExtension:  {{
	FBXVersion: 7400
	Creator: "fixture writer"
}}
GlobalSettings:  {{
	Properties70:  {{
		P: "UpAxis", "int", "Integer", "",1
		P: "UnitScaleFactor", "double", "Number", "",100
	}}
}}
Objects:  {{
	Geometry: 10, "Geometry::Box", "Mesh" {{
		Vertices: *{} {{
			a: {}
		}}
		PolygonVertexIndex: *{} {{
			a: {}
		}}
	}}
	Model: 20, "Model::Box", "Mesh" {{
		Properties70:  {{
			P: "Lcl Translation", "Lcl Translation", "", "A",5,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,90
		}}
	}}
	Material: 30, "Material::Steel", "" {{
		Properties70:  {{
			P: "DiffuseColor", "Color", "", "A",0.5,0.5,0.6
		}}
	}}
}}
Connections:  {{
	C: "OO",10,20
	C: "OO",30,20
	C: "OO",20,0
}}
"#,
        verts.len(),
        verts.join(","),
        pvi.len(),
        pvi.join(",")
    )
    .into_bytes()
}

/// The cube as an X3D (XML) scene, red.
pub fn x3d() -> Vec<u8> {
    let (v, t) = cube();
    let pts: Vec<String> = v
        .iter()
        .map(|p| format!("{} {} {}", p[0], p[1], p[2]))
        .collect();
    let idx: Vec<String> = t
        .iter()
        .map(|t| format!("{} {} {} -1", t[0], t[1], t[2]))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<X3D profile=\"Interchange\" version=\"3.3\"><Scene>\
         <Shape><Appearance><Material diffuseColor=\"1 0 0\"/></Appearance>\
         <IndexedFaceSet coordIndex=\"{}\"><Coordinate point=\"{}\"/></IndexedFaceSet></Shape>\
         </Scene></X3D>\n",
        idx.join(" "),
        pts.join(", ")
    )
    .into_bytes()
}

/// The cube as a VRML 97 scene, blue.
pub fn vrml() -> Vec<u8> {
    let (v, t) = cube();
    let pts: Vec<String> = v
        .iter()
        .map(|p| format!("{} {} {}", p[0], p[1], p[2]))
        .collect();
    let idx: Vec<String> = t
        .iter()
        .map(|t| format!("{}, {}, {}, -1", t[0], t[1], t[2]))
        .collect();
    format!(
        "#VRML V2.0 utf8\nShape {{\n appearance Appearance {{ material Material {{ diffuseColor 0 0 1 }} }}\n \
         geometry IndexedFaceSet {{\n  coord Coordinate {{ point [ {} ] }}\n  coordIndex [ {} ]\n }}\n}}\n",
        pts.join(", "),
        idx.join(", ")
    )
    .into_bytes()
}

/// The cube as an AMF document in millimetres.
pub fn amf() -> Vec<u8> {
    let (v, t) = cube();
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<amf unit=\"millimeter\"><object id=\"1\"><mesh><vertices>",
    );
    for p in &v {
        let _ = write!(
            s,
            "<vertex><coordinates><x>{}</x><y>{}</y><z>{}</z></coordinates></vertex>",
            p[0], p[1], p[2]
        );
    }
    s.push_str("</vertices><volume>");
    for t in &t {
        let _ = write!(
            s,
            "<triangle><v1>{}</v1><v2>{}</v2><v3>{}</v3></triangle>",
            t[0], t[1], t[2]
        );
    }
    s.push_str("</volume></mesh></object></amf>\n");
    s.into_bytes()
}

/// The cube as an IFC4 building element proxy (a triangulated face set) in metres.
pub fn ifc() -> Vec<u8> {
    let (v, t) = cube();
    let pts: Vec<String> = v
        .iter()
        .map(|p| format!("({:.1},{:.1},{:.1})", p[0], p[1], p[2]))
        .collect();
    let idx: Vec<String> = t
        .iter()
        .map(|t| format!("({},{},{})", t[0] + 1, t[1] + 1, t[2] + 1))
        .collect();
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView]'),'2;1');\n\
         FILE_NAME('cube.ifc','2024-01-01T00:00:00',('A'),('B'),'','fixture','');\n\
         FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
         #1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Cube',$,$,$,$,(#2),#3);\n\
         #2=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#4,$);\n\
         #3=IFCUNITASSIGNMENT((#5));\n#4=IFCAXIS2PLACEMENT3D(#6,$,$);\n\
         #5=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n#6=IFCCARTESIANPOINT((0.,0.,0.));\n\
         #10=IFCBUILDINGELEMENTPROXY('1YvctVUKr0kugbFTf53O9L',$,'Cube',$,$,#11,#12,$,$);\n\
         #11=IFCLOCALPLACEMENT($,#4);\n#12=IFCPRODUCTDEFINITIONSHAPE($,$,(#13));\n\
         #13=IFCSHAPEREPRESENTATION(#2,'Body','Tessellation',(#14));\n\
         #14=IFCTRIANGULATEDFACESET(#15,$,.T.,({}),$);\n#15=IFCCARTESIANPOINTLIST3D(({}));\n\
         ENDSEC;\nEND-ISO-10303-21;\n",
        idx.join(","),
        pts.join(",")
    )
    .into_bytes()
}

/// An Alembic (Ogawa) archive: a unit quad under a transform moving it 10 along x.
pub fn alembic() -> Vec<u8> {
    crate::alembic::fixture::quad_archive()
}
