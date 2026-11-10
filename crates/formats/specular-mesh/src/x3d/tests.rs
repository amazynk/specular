use std::collections::HashMap;

use super::*;
use crate::NoResolver;

struct Files(HashMap<&'static str, Vec<u8>>);

impl Resolver for Files {
    fn read(&self, name: &str, _max: usize) -> std::result::Result<Vec<u8>, String> {
        self.0.get(name).cloned().ok_or_else(|| "not found".into())
    }
}

fn vrml(src: &str) -> Result<Mesh> {
    read_classic(src.as_bytes(), &NoResolver, &Limits::default())
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn box_with_material() {
    let m = vrml(
        "#VRML V2.0 utf8\nShape { appearance Appearance { material Material { diffuseColor 1 0 0 \
         transparency 0.25 } } geometry Box { size 2 4 6 } }",
    )
    .unwrap();
    assert_eq!(m.triangle_count(), 12);
    assert!(close(m.bounds.min, [-1.0, -2.0, -3.0]), "{:?}", m.bounds);
    assert!(close(m.bounds.max, [1.0, 2.0, 3.0]));
    assert_eq!(m.up, Up::Y);
    assert_eq!(m.unit, Unit::Metre);
    assert_eq!(m.materials.len(), 1);
    assert_eq!(m.materials[0].base_color, [1.0, 0.0, 0.0, 0.75]);
    // Outward faces: each triangle's normal points away from the centre.
    for t in m.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.positions[i as usize]);
        let n = crate::builder::cross(crate::builder::sub(b, a), crate::builder::sub(c, a));
        let mid = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
        assert!(n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2] > 0.0);
    }
}

#[test]
fn transforms_and_def_use() {
    let m = vrml(
        "#VRML V2.0 utf8
         Transform { translation 10 0 0 scale 2 2 2 children [ DEF B Shape { geometry Box { } } ] }
         Transform { translation 0 0 0 rotation 0 0 1 1.5707963 center 1 0 0 children USE B }",
    )
    .unwrap();
    assert_eq!(m.triangle_count(), 24);
    // First: x 8‥12; second: the unit box rotated 90° about (1,0,0) → x 0‥2, y −2‥0.
    assert!(close(m.bounds.min, [0.0, -2.0, -2.0]), "{:?}", m.bounds);
    assert!(close(m.bounds.max, [12.0, 2.0, 2.0]), "{:?}", m.bounds);
}

#[test]
fn face_sets_with_colours_and_normals() {
    let m = vrml(
        "#VRML V2.0 utf8
         Shape { geometry IndexedFaceSet {
           coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0, 2 0 0 ] }
           coordIndex [ 0 1 2 3 -1 1 4 2 -1 ]
           colorPerVertex FALSE color Color { color [ 1 0 0, 0 0 1 ] } colorIndex [ 1 0 ]
         } }",
    )
    .unwrap();
    assert_eq!(m.triangle_count(), 3);
    let c = m.colors.as_ref().unwrap();
    // The quad is blue, the triangle red; faceted: corners split per face.
    assert_eq!(m.vertex_count(), 7);
    assert_eq!(c[0], [0, 0, 255, 255]);
    assert_eq!(c[4], [255, 0, 0, 255]);
    assert!(m.normals.iter().all(|n| close(*n, [0.0, 0.0, 1.0])));
    // Smooth (crease angle) and clockwise.
    let m = vrml(
        "#VRML V2.0 utf8\nShape { geometry IndexedFaceSet { ccw FALSE creaseAngle 3
           coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0 ] } coordIndex [ 0 1 2 ] } }",
    )
    .unwrap();
    assert_eq!(m.vertex_count(), 3);
    assert!(m.normals.iter().all(|n| close(*n, [0.0, 0.0, -1.0])));
}

#[test]
fn x3d_xml_encoding() {
    let src = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE X3D PUBLIC "ISO//Web3D//DTD X3D 3.3//EN" "http://www.web3d.org/specifications/x3d-3.3.dtd">
<X3D profile='Interchange' version='3.3'>
 <head><meta name='title' content='Two shapes'/><meta name='generator' content='hand'/></head>
 <Scene>
  <Transform translation='0 1 0'>
   <Shape DEF='T'>
    <Appearance><Material diffuseColor='0 1 0'/></Appearance>
    <IndexedTriangleSet index='0 1 2 0 2 3'>
     <Coordinate point='0 0 0, 1 0 0, 1 1 0, 0 1 0'/>
     <TextureCoordinate point='0 0 1 0 1 1 0 1'/>
    </IndexedTriangleSet>
   </Shape>
  </Transform>
  <Transform translation='5 0 0'><Shape USE='T'/></Transform>
  <Switch whichChoice='1'><Shape><Box size='1 1 1'/></Shape><Shape><Sphere radius='0.5'/></Shape></Switch>
  <LOD><Shape><Cone/></Shape><Shape><Box/></Shape></LOD>
  <ProtoInstance name='Gadget'/>
 </Scene>
</X3D>"#;
    let m = read_xml(src.as_bytes(), &NoResolver, &Limits::default()).unwrap();
    // A sphere of 24 rings × 48 segments; a cone's side and its 48-gon base.
    let (sphere, cone) = (22 * 48 * 2 + 2 * 48, 48 + 46);
    assert_eq!(m.triangle_count(), 4 + sphere + cone);
    assert_eq!(m.title.as_deref(), Some("Two shapes"));
    assert_eq!(m.generator.as_deref(), Some("hand"));
    assert!(m.notes.iter().any(|n| n.contains("PROTO")), "{:?}", m.notes);
    // The USEd shape shares its material.
    assert_eq!(m.materials.len(), 1);
    let uv = m.uvs.as_ref().unwrap();
    assert!(uv.iter().any(|t| (t[1] - 0.0).abs() < 1e-6));
    assert!(close(m.bounds.max, [6.0, 2.0, 1.0]), "{:?}", m.bounds);
}

#[test]
fn primitives_and_generated_geometry() {
    let m = vrml(
        "#VRML V2.0 utf8
         Shape { geometry Sphere { radius 2 } }
         Transform { translation 10 0 0 children Shape { geometry Cylinder { radius 1 height 4 } } }
         Transform { translation 20 0 0 children Shape { geometry Cone { bottomRadius 1 height 2 } } }",
    )
    .unwrap();
    assert!((m.bounds.min[1] + 2.0).abs() < 1e-4 && (m.bounds.max[1] - 2.0).abs() < 1e-4);
    assert!((m.bounds.max[0] - 21.0).abs() < 1e-3, "{:?}", m.bounds);
    // The default extrusion: the 2×1×2 box along +Y, with caps.
    let m = vrml("#VRML V2.0 utf8\nShape { geometry Extrusion { } }").unwrap();
    assert_eq!(m.triangle_count(), 12);
    assert!(close(m.bounds.min, [-1.0, 0.0, -1.0]) && close(m.bounds.max, [1.0, 1.0, 1.0]));
    let mut vol = 0.0f64;
    for t in m.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.positions[i as usize].map(f64::from));
        vol += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    assert!((vol - 4.0).abs() < 1e-4, "outward faces enclose +4: {vol}");
    let m = vrml(
        "#VRML V2.0 utf8\nShape { geometry ElevationGrid { xDimension 3 zDimension 2 \
         xSpacing 0.5 height [ 0 1 2, 3 4 5 ] } }",
    )
    .unwrap();
    assert_eq!(m.triangle_count(), 4);
    assert!(close(m.bounds.max, [1.0, 5.0, 1.0]));
    assert!(m.normals.iter().all(|n| n[1] > 0.0));
}

#[test]
fn strips_fans_and_quads() {
    let m = vrml(
        "#X3D V3.3 utf8
         PROFILE Interchange
         META \"title\" \"sets\"
         Shape { geometry TriangleStripSet { stripCount [ 4 ]
           coord Coordinate { point [ 0 0 0, 0 1 0, 1 0 0, 1 1 0 ] } } }
         Shape { geometry IndexedTriangleFanSet { index [ 0 1 2 3 -1 ]
           coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0 ] } } }
         Shape { geometry QuadSet { coord Coordinate { point [ 0 0 0, 1 0 0, 1 1 0, 0 1 0 ] } } }",
    )
    .unwrap();
    assert_eq!(m.triangle_count(), 6);
    assert_eq!(m.title.as_deref(), Some("sets"));
    // Every triangle faces −z here except the strip's (which winds to −z first).
    let zs: Vec<f32> = m.normals.iter().map(|n| n[2]).collect();
    assert!(zs.iter().all(|z| z.abs() > 0.99), "{zs:?}");
}

#[test]
fn inline_files_same_folder_only() {
    let part = b"#VRML V2.0 utf8\nShape { geometry Box { } }".to_vec();
    let loop_ = b"#VRML V2.0 utf8\nInline { url \"loop.wrl\" }".to_vec();
    let files = Files(HashMap::from([("part.wrl", part), ("loop.wrl", loop_)]));
    let src = "#VRML V2.0 utf8
        Transform { translation 5 0 0 children Inline { url [ \"http://example.com/x.wrl\" \"part.wrl\" ] } }
        Inline { url \"../up.wrl\" }
        Inline { url \"loop.wrl\" }
        Shape { geometry Box { } }";
    let m = read_classic(src.as_bytes(), &files, &Limits::default()).unwrap();
    assert_eq!(m.triangle_count(), 24);
    assert!((m.bounds.max[0] - 6.0).abs() < 1e-5);
    assert!(
        m.notes
            .iter()
            .any(|n| n.contains("outside the model's folder")),
        "{:?}",
        m.notes
    );
    assert!(m.notes.iter().any(|n| n.contains("includes itself")));
}

#[test]
fn gzip_and_textures() {
    let src =
        b"#VRML V2.0 utf8\nShape { appearance Appearance { texture ImageTexture { url \"t.png\" } \
        textureTransform TextureTransform { scale 2 2 } } geometry Box { } }";
    let gz = gzip_stored(src);
    let files = Files(HashMap::from([("t.png", b"PNG?".to_vec())]));
    let m = read_classic(&gz, &files, &Limits::default()).unwrap();
    assert_eq!(m.triangle_count(), 12);
    assert_eq!(m.images.len(), 1);
    assert_eq!(m.materials[0].texture, Some(0));
    let uv = m.uvs.as_ref().unwrap();
    assert!(uv.iter().any(|t| (t[0] - 2.0).abs() < 1e-6), "{uv:?}");
    // The XML entry takes the classic encoding too (a .x3dz may hold either).
    assert!(read_xml(&gz, &files, &Limits::default()).is_ok());
}

#[test]
fn refusals() {
    let e = vrml("#VRML V1.0 ascii\nSeparator { Cube { } }")
        .unwrap_err()
        .to_string();
    assert!(e.contains("VRML 1.0"), "{e}");
    let e = vrml(
        "#VRML V2.0 utf8\nShape { geometry IndexedLineSet { coord Coordinate { point [0 0 0, 1 1 1] } coordIndex [0 1] } }\nShape { geometry Text { string \"hi\" } }",
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("no shapes to show") && e.contains("lines and points") && e.contains("text"),
        "{e}"
    );
    let e = vrml("#VRML V2.0 utf8\n").unwrap_err().to_string();
    assert!(e.contains("no shapes"), "{e}");
    let e = read_xml(b"<html/>", &NoResolver, &Limits::default())
        .unwrap_err()
        .to_string();
    assert!(e.contains("not an X3D document"), "{e}");
}

#[test]
fn hostile_input_does_not_panic() {
    let cases: [&[u8]; 9] = [
        b"#VRML V2.0 utf8\nShape { geometry IndexedFaceSet { coord Coordinate { point [ 0 0 0 ] } coordIndex [ 0 5 99999999999 -1 ] colorIndex [ 7 ] color Color { color [ 1 ] } } }",
        b"#VRML V2.0 utf8\nShape { geometry ElevationGrid { xDimension 100000 zDimension 100000 height [1] } }",
        b"#VRML V2.0 utf8\nShape { geometry Extrusion { spine [ 0 0 0, 0 0 0 ] crossSection [ 0 0 ] } }",
        b"#VRML V2.0 utf8\nDEF A Group { children [ USE A ] }",
        b"#VRML V2.0 utf8\nShape { geometry TriangleFanSet { fanCount [ -3 99 ] coord Coordinate { point [ 0 0 0 1 1 1 ] } } }",
        b"<X3D><Scene><Transform USE='nothing'/><Shape><IndexedFaceSet coordIndex='a b c'><Coordinate point='x'/></IndexedFaceSet></Shape></Scene></X3D>",
        b"\x1f\x8b\x08\x00garbage",
        b"<X3D><Scene><Group DEF='g'><Group USE='g'/></Group></Scene>",
        b"#VRML V2.0 utf8\nTransform { scale 0 0 0 children Shape { geometry Box { } } }",
    ];
    for c in cases {
        let _ = read_classic(c, &NoResolver, &Limits::default());
        let _ = read_xml(c, &NoResolver, &Limits::default());
    }
}

/// A gzip member of stored deflate blocks.
fn gzip_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    let chunks: Vec<&[u8]> = data.chunks(65_535).collect();
    for (i, c) in chunks.iter().enumerate() {
        out.push(u8::from(i + 1 == chunks.len()));
        let n = c.len() as u16;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&(!n).to_le_bytes());
        out.extend_from_slice(c);
    }
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    out.extend_from_slice(&(!crc).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

/// `cargo test -p specular-mesh x3d_timing -- --ignored --nocapture`
#[test]
#[ignore]
fn x3d_timing() {
    let n = 1000usize;
    let mut pts = String::new();
    let mut idx = String::new();
    for j in 0..n {
        for i in 0..n {
            pts.push_str(&format!("{} {} {:.4}, ", i, j, ((i * j) as f64).sin()));
        }
    }
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let a = j * n + i;
            idx.push_str(&format!("{} {} {} {} -1 ", a, a + 1, a + n + 1, a + n));
        }
    }
    let classic = format!(
        "#VRML V2.0 utf8\nShape {{ geometry IndexedFaceSet {{ creaseAngle 1 coord Coordinate {{ point [ {pts} ] }} coordIndex [ {idx} ] }} }}"
    );
    let xml = format!(
        "<X3D><Scene><Shape><IndexedFaceSet creaseAngle='1' coordIndex='{idx}'><Coordinate point='{pts}'/></IndexedFaceSet></Shape></Scene></X3D>"
    );
    for (what, src) in [("classic", classic), ("xml", xml)] {
        let t0 = std::time::Instant::now();
        let m = read_classic(src.as_bytes(), &NoResolver, &Limits::default()).unwrap();
        eprintln!(
            "{what}: {} triangles, {} MB in {:.0} ms",
            m.triangle_count(),
            src.len() >> 20,
            t0.elapsed().as_secs_f64() * 1e3
        );
    }
}

fn corpus() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("SPECULAR_MODELS2_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../tools/corpus/models2"
            ))
        });
    if dir.join("oracle/x3d.tsv").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no X3D corpus oracle at {}", dir.display());
        None
    }
}

/// Every X3D file against the independent reader's counts and bounds
/// (gen/x3d_oracle.py), and each scene's classic encodings (`.x3dv`, `.wrl`) and gzipped
/// copies against its XML one.
#[test]
fn corpus_against_the_oracle_and_across_encodings() {
    let Some(dir) = corpus() else { return };
    let tsv = std::fs::read_to_string(dir.join("oracle/x3d.tsv")).unwrap();
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
    let v3 = |s: &str| -> [f32; 3] {
        let v: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect();
        [v[0], v[1], v[2]]
    };
    let near = |a: [f32; 3], b: [f32; 3], size: f32| {
        a.iter()
            .zip(b)
            .all(|(x, y)| (x - y).abs() <= 1e-4 * size.max(1.0))
    };
    let (mut files, mut compared, mut mismatches) = (0, 0, Vec::new());
    let mut rels: Vec<String> = facts.keys().cloned().collect();
    rels.sort();
    // ORACLE lines start their own line (the harness's "test ... " has none).
    println!();
    for rel in rels {
        let f = &facts[&rel];
        let path = dir.join(&rel);
        let t0 = std::time::Instant::now();
        let m = crate::read_path(&path, &Limits::default());
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        files += 1;
        // One line per file (each encoding of the scene too) for
        // tools/ci/oracle-pass-rates.sh.
        let before = mismatches.len();
        let m = match m {
            Ok(m) => m,
            Err(e) => {
                mismatches.push(format!("{rel}: {e}"));
                println!("ORACLE\tX3D\t{rel}\tfail");
                continue;
            }
        };
        let tris: usize = f["tris"].parse().unwrap();
        let size = m.bounds.radius() * 2.0;
        if f.get("round").map(String::as_str) == Some("0") {
            compared += 1;
            if m.triangle_count() != tris {
                mismatches.push(format!(
                    "{rel}: {} triangles, oracle {tris}",
                    m.triangle_count()
                ));
            }
            if let (Some(lo), Some(hi)) = (f.get("min"), f.get("max"))
                && !(near(m.bounds.min, v3(lo), size) && near(m.bounds.max, v3(hi), size))
            {
                mismatches.push(format!("{rel}: bounds {:?}, oracle {lo} / {hi}", m.bounds));
            }
        } else if m.triangle_count() < tris {
            mismatches.push(format!(
                "{rel}: {} triangles < oracle's {tris}",
                m.triangle_count()
            ));
        }
        eprintln!("{rel}: {} triangles in {ms:.1} ms", m.triangle_count());
        let status = if mismatches.len() > before {
            "fail"
        } else {
            "pass"
        };
        println!("ORACLE\tX3D\t{rel}\t{status}");
        // The same scene in the classic encodings.
        for ext in ["x3dv", "wrl", "x3dz", "x3dvz", "wrz"] {
            let other = path.with_extension(ext);
            if !other.is_file() {
                continue;
            }
            files += 1;
            compared += 1;
            let before = mismatches.len();
            match crate::read_path(&other, &Limits::default()) {
                Ok(o) => {
                    if o.triangle_count() != m.triangle_count()
                        || !(near(o.bounds.min, m.bounds.min, size)
                            && near(o.bounds.max, m.bounds.max, size))
                    {
                        mismatches.push(format!(
                            "{rel} vs .{ext}: {} vs {} triangles, {:?} vs {:?}",
                            m.triangle_count(),
                            o.triangle_count(),
                            m.bounds,
                            o.bounds
                        ));
                    }
                }
                Err(e) => mismatches.push(format!("{rel} .{ext}: {e}")),
            }
            let status = if mismatches.len() > before {
                "fail"
            } else {
                "pass"
            };
            println!(
                "ORACLE\tX3D\t{}\t{status}",
                std::path::Path::new(&rel).with_extension(ext).display()
            );
        }
    }
    eprintln!(
        "x3d corpus: {files} files, {compared} compared, {} mismatches",
        mismatches.len()
    );
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// PixelTexture: an SFImage becomes an RGBA PNG, the file's first row at the bottom, in both
/// encodings and for each component count.
#[test]
fn pixel_textures() {
    let rgba = |m: &Mesh| -> (u32, u32, Vec<u8>) {
        let ImageData::Bytes { bytes, .. } = &m.images[0] else {
            panic!("no image");
        };
        let opts = specular_png::DecodeOptions {
            output: specular_png::Output::Rgba8,
            ..Default::default()
        };
        let img = specular_png::decode(bytes, &opts).unwrap();
        (img.width, img.height, img.to_rgba8())
    };
    let classic = "#VRML V2.0 utf8\nShape { appearance Appearance { texture PixelTexture { \
        image 2 2 3 0xFF0000 0x00FF00 0x0000FF 0xFFFFFF } } geometry Box { } }";
    let m = vrml(classic).unwrap();
    assert_eq!(m.materials[0].texture, Some(0));
    let (w, h, px) = rgba(&m);
    assert_eq!((w, h), (2, 2));
    // Top row: blue, white; bottom row: red, green.
    assert_eq!(
        px,
        [
            0, 0, 255, 255, 255, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255
        ]
    );
    let xml = r#"<X3D><Scene><Shape><Appearance><PixelTexture image="1 1 2 0x80C0"/>
        </Appearance><Box/></Shape></Scene></X3D>"#;
    let m = read_xml(xml.as_bytes(), &NoResolver, &Limits::default()).unwrap();
    assert_eq!(rgba(&m).2, [0x80, 0x80, 0x80, 0xC0]);
    let one = "#VRML V2.0 utf8\nShape { appearance Appearance { texture PixelTexture { \
        image 1 1 4 4278190335 } } geometry Box { } }";
    assert_eq!(rgba(&vrml(one).unwrap()).2, [0xFF, 0, 0, 0xFF]);
    // Short pixel lists and odd component counts are noted, never a panic.
    for bad in [
        "image 4 4 3 1 2",
        "image 1 1 7 0",
        "image 0 0 0",
        "image -3 2 1 0",
    ] {
        let src = format!(
            "#VRML V2.0 utf8\nShape {{ appearance Appearance {{ texture PixelTexture {{ {bad} }} }} geometry Box {{ }} }}"
        );
        let m = vrml(&src).unwrap();
        assert!(m.images.is_empty(), "{bad}");
    }
}
