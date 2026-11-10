use std::collections::HashMap;

use crate::fixtures;
use crate::*;

/// Side files from memory.
struct MapResolver(HashMap<String, Vec<u8>>);

impl Resolver for MapResolver {
    fn read(&self, name: &str, max: usize) -> std::result::Result<Vec<u8>, String> {
        let rel = safe_relative(name).ok_or("unsafe")?;
        let key = rel.to_string_lossy().into_owned();
        match self.0.get(&key) {
            Some(b) if b.len() <= max => Ok(b.clone()),
            Some(_) => Err("too large".into()),
            None => Err("not found".into()),
        }
    }
}

fn map(files: &[(&str, Vec<u8>)]) -> MapResolver {
    MapResolver(
        files
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

fn read_ok(format: MeshFormat, bytes: &[u8], r: &dyn Resolver) -> Mesh {
    match read(format, bytes, r, &Limits::default()) {
        Ok(m) => m,
        Err(e) => panic!("{format:?}: {e}"),
    }
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|c| (a[c] - b[c]).abs() < 1e-4)
}

fn check_invariants(m: &Mesh) {
    let n = m.positions.len();
    assert_eq!(m.normals.len(), n);
    assert!(m.indices.len().is_multiple_of(3));
    assert!(m.indices.iter().all(|&i| (i as usize) < n));
    if let Some(c) = &m.colors {
        assert_eq!(c.len(), n);
    }
    if let Some(u) = &m.uvs {
        assert_eq!(u.len(), n);
    }
    for nm in &m.normals {
        let l = (nm[0] * nm[0] + nm[1] * nm[1] + nm[2] * nm[2]).sqrt();
        assert!((l - 1.0).abs() < 1e-3, "normal {nm:?}");
    }
    let total: u32 = m.parts.iter().map(|p| p.count).sum();
    assert_eq!(total as usize, m.triangle_count());
    let mut next = 0;
    for p in &m.parts {
        assert_eq!(p.first, next);
        next += p.count;
        if let Some(mi) = p.material {
            assert!((mi as usize) < m.materials.len());
        }
    }
    for mat in &m.materials {
        if let Some(t) = mat.texture {
            assert!((t as usize) < m.images.len());
        }
    }
}

#[test]
fn stl_binary_cube() {
    let m = read_ok(MeshFormat::Stl, &fixtures::stl_binary(2.0), &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 12);
    assert_eq!(m.vertex_count(), 8, "corners welded");
    assert!(close(m.bounds.min, [0.0; 3]) && close(m.bounds.max, [2.0; 3]));
    assert!(m.flat);
    assert_eq!(m.unit, Unit::Unknown);
    assert_eq!(m.up, Up::Z);
    assert_eq!(m.title.as_deref(), Some("cube model"));
}

#[test]
fn stl_ascii_cube() {
    let m = read_ok(MeshFormat::Stl, &fixtures::stl_ascii(), &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 12);
    assert_eq!(m.vertex_count(), 8);
    assert_eq!(m.title.as_deref(), Some("cube"));
    assert!(close(m.bounds.max, [1.0; 3]));
}

#[test]
fn stl_binary_starting_with_solid_is_binary() {
    let mut b = fixtures::stl_binary(1.0);
    b[..6].copy_from_slice(b"solid ");
    let m = read_ok(MeshFormat::Stl, &b, &NoResolver);
    assert_eq!(m.triangle_count(), 12);
}

#[test]
fn stl_colours_viscam() {
    let mut b = fixtures::stl_binary(1.0);
    // Face 0: bit 15 set, pure red (VisCAM: red in bits 10..14).
    let at = 84 + 48;
    b[at..at + 2].copy_from_slice(&(0x8000u16 | (31 << 10)).to_le_bytes());
    let m = read_ok(MeshFormat::Stl, &b, &NoResolver);
    check_invariants(&m);
    let c = m.colors.as_ref().unwrap();
    assert_eq!(c[m.indices[0] as usize], [255, 0, 0, 255]);
    assert_eq!(c[m.indices[3] as usize], [255, 255, 255, 255]);
}

#[test]
fn stl_truncated_keeps_whole_triangles() {
    let b = fixtures::stl_binary(1.0);
    let m = read_ok(MeshFormat::Stl, &b[..84 + 50 * 5 + 20], &NoResolver);
    assert_eq!(m.triangle_count(), 5);
    assert!(m.notes.iter().any(|n| n.contains("cut short")));
}

#[test]
fn obj_with_mtl_and_texture() {
    let r = map(&[
        ("cube.mtl", fixtures::mtl()),
        ("tex.png", b"PNGDATA".to_vec()),
    ]);
    let m = read_ok(MeshFormat::Obj, &fixtures::obj(), &r);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 12);
    assert_eq!(m.materials.len(), 2);
    assert_eq!(m.materials[0].name, "red");
    assert!(close(
        [
            m.materials[0].base_color[0],
            m.materials[0].base_color[1],
            m.materials[0].base_color[2]
        ],
        [1.0, 0.0, 0.0]
    ));
    let t = m.materials[1].texture.expect("texture") as usize;
    assert_eq!(
        m.images[t],
        ImageData::Bytes {
            bytes: b"PNGDATA".to_vec(),
            mime: None
        }
    );
    assert!(m.uvs.is_some());
    assert_eq!(m.up, Up::Y);
    assert!(m.flat, "no normals and no smoothing groups");
    assert_eq!(m.parts.len(), 2);
    assert_eq!(m.parts[0].count, 4);
    assert_eq!(m.parts[1].count, 8);
    assert_eq!(m.title.as_deref(), Some("Cube"));
}

#[test]
fn obj_missing_side_files_are_noted() {
    let m = read_ok(MeshFormat::Obj, &fixtures::obj(), &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 12);
    assert!(m.notes.iter().any(|n| n.contains("cube.mtl")));
}

#[test]
fn obj_texture_outside_folder_is_refused() {
    let mtl = b"newmtl a\nmap_Kd ../../etc/passwd\nnewmtl b\nmap_Kd /etc/passwd\nnewmtl c\nmap_Kd http://example.com/x.png\n";
    let obj = b"mtllib m.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl a\nf 1 2 3\nusemtl b\nf 1 2 3\nusemtl c\nf 1 2 3\n";
    let r = map(&[("m.mtl", mtl.to_vec()), ("etc/passwd", b"x".to_vec())]);
    let m = read_ok(MeshFormat::Obj, obj, &r);
    check_invariants(&m);
    assert_eq!(m.images.len(), 3);
    assert!(
        m.images
            .iter()
            .all(|i| matches!(i, ImageData::Missing { .. }))
    );
}

#[test]
fn obj_normals_and_colours() {
    let obj = b"v 0 0 0 1 0 0\nv 1 0 0 0 1 0\nv 0 1 0 0 0 1\nvn 0 0 2\nf 1//1 2//1 3//1\n";
    let m = read_ok(MeshFormat::Obj, obj, &NoResolver);
    check_invariants(&m);
    assert!(m.file_normals);
    assert!(!m.flat);
    assert!(close(m.normals[0], [0.0, 0.0, 1.0]));
    assert_eq!(m.colors.as_ref().unwrap()[1], [0, 255, 0, 255]);
}

#[test]
fn obj_bad_indices_skip_faces() {
    let obj = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 9\nf 0 1 2\nf -4 1 2\nf a b c\n";
    let m = read_ok(MeshFormat::Obj, obj, &NoResolver);
    assert_eq!(m.triangle_count(), 1);
    assert!(m.notes.iter().any(|n| n.contains("4 unreadable")));
}

#[test]
fn ply_all_encodings() {
    for f in ["ascii", "binary_little_endian", "binary_big_endian"] {
        let m = read_ok(MeshFormat::Ply, &fixtures::ply(f), &NoResolver);
        check_invariants(&m);
        assert_eq!(m.triangle_count(), 12, "{f}");
        assert_eq!(m.vertex_count(), 8, "{f}");
        assert!(close(m.bounds.max, [1.0; 3]), "{f}");
        assert_eq!(m.colors.as_ref().unwrap()[1], [30, 128, 225, 255], "{f}");
    }
}

#[test]
fn ply_point_cloud_is_refused() {
    let b = b"ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n";
    assert!(matches!(
        read(MeshFormat::Ply, b, &NoResolver, &Limits::default()),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn ply_huge_count_is_refused_without_allocating() {
    let b = b"ply\nformat binary_little_endian 1.0\nelement vertex 4000000000\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n";
    assert!(read(MeshFormat::Ply, b, &NoResolver, &Limits::default()).is_err());
}

fn check_two_cubes(m: &Mesh) {
    check_invariants(m);
    assert_eq!(m.triangle_count(), 24);
    // Cube 1 at x 2..3; cube 2 scaled 2, turned 90° about z: x -2..0, y 0..2.
    assert!(close(m.bounds.min, [-2.0, 0.0, 0.0]), "{:?}", m.bounds);
    assert!(close(m.bounds.max, [3.0, 2.0, 2.0]), "{:?}", m.bounds);
    assert_eq!(m.unit, Unit::Metre);
    assert_eq!(m.up, Up::Y);
    assert!(m.flat, "no normals: flat");
    assert_eq!(m.materials.len(), 1);
    assert_eq!(m.materials[0].name, "Blue");
    assert!((m.materials[0].base_color[2] - 0.9).abs() < 1e-6);
    assert_eq!(m.title.as_deref(), Some("Cubes"));
    assert_eq!(m.generator.as_deref(), Some("specular tests"));
}

#[test]
fn gltf_embedded_buffer() {
    let m = read_ok(MeshFormat::Gltf, &fixtures::gltf_embedded(), &NoResolver);
    check_two_cubes(&m);
}

#[test]
fn gltf_side_buffer() {
    let (g, bin) = fixtures::gltf_side();
    let m = read_ok(MeshFormat::Gltf, &g, &map(&[("cube.bin", bin)]));
    check_two_cubes(&m);
    // Without the side file there is nothing to show.
    assert!(read(MeshFormat::Gltf, &g, &NoResolver, &Limits::default()).is_err());
}

#[test]
fn glb_container() {
    let b = fixtures::glb();
    assert_eq!(MeshFormat::sniff(&b), Some(MeshFormat::Glb));
    let m = read_ok(MeshFormat::Glb, &b, &NoResolver);
    check_two_cubes(&m);
}

#[test]
fn glb_without_chunk_padding() {
    // The fixture with its JSON padding taken out and an odd length (as some writers do).
    let b = fixtures::glb();
    let jlen = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
    let mut json = b[20..20 + jlen].to_vec();
    while json.last() == Some(&b' ') {
        json.pop();
    }
    if json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let rest = &b[20 + jlen..];
    let total = 20 + json.len() + rest.len();
    let mut out = b[..8].to_vec();
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&b[16..20]);
    out.extend_from_slice(&json);
    out.extend_from_slice(rest);
    let m = read_ok(MeshFormat::Glb, &out, &NoResolver);
    check_two_cubes(&m);
}

#[test]
fn obj_missing_uv_and_normal_indices_drop_the_attribute() {
    let obj = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1\nf 1/1/5 3/3/5 2/2/5\n";
    let m = read_ok(MeshFormat::Obj, obj, &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 2);
    assert!(m.uvs.is_none());
}

#[test]
fn obj_line_continuation() {
    let obj = b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\r\nf 1 2 \\\r\n3 \\\n4\nf 1 \\x 2 3\n";
    let m = read_ok(MeshFormat::Obj, obj, &NoResolver);
    check_invariants(&m);
    // The quad joined (2 triangles); the stray backslash mid-line spoils only its face.
    assert_eq!(m.triangle_count(), 2);
}

#[test]
fn gltf_draco_required_is_refused() {
    let j = br#"{"asset":{"version":"2.0"},"extensionsRequired":["KHR_draco_mesh_compression"]}"#;
    assert!(matches!(
        read(MeshFormat::Gltf, j, &NoResolver, &Limits::default()),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn gltf_network_uri_never_fetched() {
    let j = br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"https://example.com/a.bin","byteLength":4}]}"#;
    struct Panics;
    impl Resolver for Panics {
        fn read(&self, name: &str, _: usize) -> std::result::Result<Vec<u8>, String> {
            assert!(safe_relative(name).is_none(), "{name}");
            Err("refused".into())
        }
    }
    assert!(read(MeshFormat::Gltf, j, &Panics, &Limits::default()).is_err());
}

#[test]
fn gltf_meshopt_fallback_buffer_is_neither_read_nor_noted() {
    // The embedded fixture plus a fallback buffer that no view uses (as gltfpack writes it).
    let mut doc: serde_json::Value = serde_json::from_slice(&fixtures::gltf_embedded()).unwrap();
    doc["buffers"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "uri": "fallback.bin", "byteLength": 4096,
            "extensions": {"EXT_meshopt_compression": {"fallback": true}}
        }));
    struct Panics;
    impl Resolver for Panics {
        fn read(&self, name: &str, _: usize) -> std::result::Result<Vec<u8>, String> {
            panic!("{name} read");
        }
    }
    let m = read_ok(
        MeshFormat::Gltf,
        &serde_json::to_vec(&doc).unwrap(),
        &Panics,
    );
    check_two_cubes(&m);
    assert!(m.notes.is_empty(), "{:?}", m.notes);
}

#[test]
fn threemf_package() {
    let b = fixtures::threemf();
    let m = read_ok(MeshFormat::ThreeMf, &b, &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 24);
    assert_eq!(m.unit, Unit::Centimetre);
    assert_eq!(m.up, Up::Z);
    assert!(close(m.bounds.min, [0.0; 3]));
    assert!(close(m.bounds.max, [4.0, 1.0, 1.0]));
    assert_eq!(m.title.as_deref(), Some("Cube & Co"));
    assert_eq!(m.materials.len(), 1);
    assert_eq!(m.materials[0].name, "Red");
    let colors = m
        .colors
        .as_ref()
        .expect("colour group gives vertex colours");
    assert!(colors.contains(&[0, 255, 0, 255]));
    assert!(colors.contains(&[0, 0, 255, 255]));
}

#[test]
fn safe_relative_paths() {
    for bad in [
        "",
        "/etc/passwd",
        "\\\\server\\share\\x.png",
        "C:\\x.png",
        "c:/x.png",
        "http://example.com/x.png",
        "file:///etc/passwd",
        "data:image/png;base64,AAAA",
        "../x.png",
        "a/../../x.png",
        "..",
        ".",
    ] {
        assert!(safe_relative(bad).is_none(), "{bad:?}");
    }
    for (ok, want) in [
        ("x.png", "x.png"),
        ("tex/x.png", "tex/x.png"),
        ("tex\\x.png", "tex/x.png"),
        ("./a/../x.png", "x.png"),
        ("my file.png", "my file.png"),
    ] {
        // Compared as paths: joined with the platform's separator.
        assert_eq!(
            safe_relative(ok),
            Some(std::path::PathBuf::from(want)),
            "{ok:?}"
        );
    }
}

#[test]
fn caps_are_enforced() {
    let limits = Limits {
        max_triangles: 5,
        ..Limits::default()
    };
    for (f, b) in [
        (MeshFormat::Stl, fixtures::stl_binary(1.0)),
        (MeshFormat::Stl, fixtures::stl_ascii()),
        (MeshFormat::Ply, fixtures::ply("ascii")),
        (MeshFormat::Glb, fixtures::glb()),
        (MeshFormat::ThreeMf, fixtures::threemf()),
    ] {
        assert!(
            matches!(read(f, &b, &NoResolver, &limits), Err(Error::TooLarge(_))),
            "{f:?}"
        );
    }
}

#[test]
fn torus_knot_and_preview() {
    let m = fixtures::torus_knot(64, 12);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 64 * 12 * 2);
    let img = preview::render(&m, &[], 64, 48);
    assert_eq!(img.len(), 64 * 48 * 4);
    let opaque = img.chunks_exact(4).filter(|p| p[3] == 255).count();
    assert!(opaque > 64 * 48 / 6, "{opaque}");
    // Corners are background.
    assert_eq!(img[3], 0);
}

#[test]
fn preview_of_every_fixture() {
    let r = map(&[("cube.mtl", fixtures::mtl())]);
    for (f, b) in all_fixtures() {
        let m = read_ok(f, &b, &r);
        let img = preview::render(&m, &[], 32, 32);
        assert!(img.chunks_exact(4).any(|p| p[3] == 255), "{f:?}");
    }
}

fn all_fixtures() -> Vec<(MeshFormat, Vec<u8>)> {
    vec![
        (MeshFormat::Stl, fixtures::stl_binary(1.0)),
        (MeshFormat::Stl, fixtures::stl_ascii()),
        (MeshFormat::Obj, fixtures::obj()),
        (MeshFormat::Ply, fixtures::ply("ascii")),
        (MeshFormat::Ply, fixtures::ply("binary_little_endian")),
        (MeshFormat::Ply, fixtures::ply("binary_big_endian")),
        (MeshFormat::Gltf, fixtures::gltf_embedded()),
        (MeshFormat::Glb, fixtures::glb()),
        (MeshFormat::ThreeMf, fixtures::threemf()),
        (MeshFormat::Usd, fixtures::usda()),
        (MeshFormat::Usdz, fixtures::usdz()),
        (MeshFormat::Dae, fixtures::dae()),
        (MeshFormat::ThreeDs, fixtures::three_ds()),
        (MeshFormat::Lwo, fixtures::lwo()),
        (MeshFormat::Fbx, fixtures::fbx()),
        (MeshFormat::Fbx, fixtures::fbx_ascii()),
    ]
}

/// Outward faces: every triangle's normal points away from its cube's centre.
fn outward(m: &Mesh, centre_of: impl Fn([f32; 3]) -> [f32; 3]) -> bool {
    m.indices.chunks_exact(3).all(|t| {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.positions[i as usize]);
        let n = crate::builder::cross(crate::builder::sub(b, a), crate::builder::sub(c, a));
        let mid = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
        let o = centre_of(mid);
        let d = [0, 1, 2].map(|k| mid[k] - o[k]);
        n[0] * d[0] + n[1] * d[1] + n[2] * d[2] > 0.0
    })
}

#[test]
fn scene_formats_read_their_fixtures() {
    // USD: two referenced cubes (the second moved by the selected variant), the invisible
    // mesh left out, the class not drawn; z up, millimetres; the material's colour.
    for (f, b) in [
        (MeshFormat::Usd, fixtures::usda()),
        (MeshFormat::Usdz, fixtures::usdz()),
    ] {
        let m = read_ok(f, &b, &NoResolver);
        check_invariants(&m);
        assert_eq!(m.triangle_count(), 24, "{f:?}");
        assert_eq!(m.bounds.min, [0.0; 3]);
        assert_eq!(m.bounds.max, [4.0, 1.0, 1.0]);
        assert_eq!((m.up, m.unit), (Up::Z, Unit::Millimetre));
        assert_eq!(m.generator.as_deref(), Some("fixture writer"));
        assert_eq!(m.title.as_deref(), Some("World"));
        assert_eq!(m.materials.len(), 1);
        assert_eq!(m.materials[0].name, "Red");
        assert_eq!(m.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]);
        assert!(outward(&m, |p| [
            if p[0] > 2.0 { 3.5 } else { 0.5 },
            0.5,
            0.5
        ]));
    }
    // COLLADA: the cube twice (the second translated and doubled), its normals, a material.
    let m = read_ok(MeshFormat::Dae, &fixtures::dae(), &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 24);
    assert_eq!(m.bounds.max, [5.0, 2.0, 2.0]);
    assert_eq!((m.up, m.unit), (Up::Z, Unit::Centimetre));
    assert_eq!(m.title.as_deref(), Some("Cubes"));
    assert_eq!(m.generator.as_deref(), Some("fixture writer"));
    assert!(m.file_normals);
    assert_eq!(m.materials[0].name, "Blue");
    assert_eq!(m.materials[0].base_color, [0.0, 0.5, 1.0, 1.0]);
    assert!(outward(&m, |p| if p[0] > 2.0 {
        [4.0, 1.0, 1.0]
    } else {
        [0.5; 3]
    }));
    // 3DS: moved up by its keyframer node, its material.
    let m = read_ok(MeshFormat::ThreeDs, &fixtures::three_ds(), &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 12);
    assert_eq!(m.bounds.min, [0.0, 0.0, 2.0]);
    assert_eq!(m.materials[0].name, "Green");
    assert!(outward(&m, |_| [0.5, 0.5, 2.5]));
    // LightWave: mirrored in z, the clockwise faces still outward.
    let m = read_ok(MeshFormat::Lwo, &fixtures::lwo(), &NoResolver);
    check_invariants(&m);
    assert_eq!(m.triangle_count(), 12);
    assert_eq!(m.bounds.min, [0.0, 0.0, -1.0]);
    assert_eq!(m.materials[0].name, "Orange");
    assert!(outward(&m, |_| [0.5, 0.5, -0.5]));
    // FBX, binary and ASCII: rotated 90° about z and moved by 5 in x; metres.
    for b in [fixtures::fbx(), fixtures::fbx_ascii()] {
        let m = read_ok(MeshFormat::Fbx, &b, &NoResolver);
        check_invariants(&m);
        assert_eq!(m.triangle_count(), 12);
        for k in 0..3 {
            let want_min = [4.0, 0.0, 0.0][k];
            let want_max = [5.0, 1.0, 1.0][k];
            assert!((m.bounds.min[k] - want_min).abs() < 1e-5, "{:?}", m.bounds);
            assert!((m.bounds.max[k] - want_max).abs() < 1e-5, "{:?}", m.bounds);
        }
        assert_eq!((m.up, m.unit), (Up::Y, Unit::Metre));
        assert_eq!(m.generator.as_deref(), Some("fixture writer"));
        assert_eq!(m.materials[0].name, "Steel");
        assert!(outward(&m, |_| [4.5, 0.5, 0.5]));
    }
}

#[test]
fn scene_refusals_are_plain() {
    let e = read(
        MeshFormat::Lwo,
        b"FORM\0\0\0\x04LXOB",
        &NoResolver,
        &Limits::default(),
    );
    assert!(matches!(e, Err(Error::Unsupported(m)) if m.contains("Modo")));
    let old = b"; FBX 6.1.0 project file\nFBXHeaderExtension:  {\n FBXVersion: 6100\n}\n";
    let e = read(MeshFormat::Fbx, old, &NoResolver, &Limits::default());
    assert!(matches!(e, Err(Error::Unsupported(m)) if m.contains("6.1")));
    let mut b = fixtures::fbx();
    b[23..27].copy_from_slice(&6100u32.to_le_bytes());
    let e = read(MeshFormat::Fbx, &b, &NoResolver, &Limits::default());
    assert!(matches!(e, Err(Error::Unsupported(_))));
    // A USD stage of curves only.
    let curves = b"#usda 1.0\ndef BasisCurves \"C\" { }\n";
    let e = read(MeshFormat::Usd, curves, &NoResolver, &Limits::default());
    assert!(matches!(e, Err(Error::Unsupported(m)) if m.contains("BasisCurves")));
}

/// Every truncation and many byte flips of every fixture: errors are fine, panics are not.
#[test]
fn robust_to_truncation_and_corruption() {
    let r = map(&[("cube.mtl", fixtures::mtl()), ("tex.png", vec![1, 2, 3])]);
    let limits = Limits {
        max_triangles: 100_000,
        max_vertices: 100_000,
        max_unpacked: 1 << 24,
        ..Limits::default()
    };
    for (f, b) in all_fixtures() {
        for n in 0..b.len() {
            let _ = read(f, &b[..n], &r, &limits);
        }
        let mut seed = 0x1234_5678_9abc_def0u64 ^ b.len() as u64;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..1500 {
            let mut c = b.clone();
            for _ in 0..1 + rnd() % 4 {
                let i = (rnd() % c.len() as u64) as usize;
                c[i] = match rnd() % 4 {
                    0 => c[i] ^ (1 << (rnd() % 8)),
                    1 => 0xFF,
                    2 => 0,
                    _ => b"0123456789-.e /\n"[(rnd() % 16) as usize],
                };
            }
            if let Ok(m) = read(f, &c, &r, &limits) {
                check_invariants(&m);
                let _ = preview::render(&m, &[], 8, 8);
            }
        }
    }
}

#[test]
fn weld_merges_exact_positions_only() {
    let corners = vec![
        [0.0, 0.0, 0.0],
        [-0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1e-7],
        [1.0, 0.0, 0.0],
    ];
    let (u, r) = crate::builder::weld(&corners);
    assert_eq!(u.len(), 3);
    assert_eq!(r, vec![0, 0, 1, 2, 1]);
}

#[test]
fn base64_round_trip() {
    let data: Vec<u8> = (0..=255).collect();
    let j = fixtures::gltf_embedded();
    assert!(!j.is_empty());
    for n in [0, 1, 2, 3, 4, 5, 255] {
        let s = {
            const A: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            for c in data[..n].chunks(3) {
                let v = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | u32::from(*c.get(2).unwrap_or(&0));
                for k in 0..=c.len() {
                    s.push(A[(v >> (18 - 6 * k) & 63) as usize] as char);
                }
            }
            s
        };
        assert_eq!(crate::gltf::base64(&s).unwrap(), &data[..n]);
    }
}

// ------------------------------------------------------------------- hostile references

/// Reads `bytes` as `format` on a small stack; must finish quickly (before the caps, these
/// files looped, took exponential time or allocated gigabytes).
fn read_hostile(format: MeshFormat, bytes: &[u8]) -> Result<Mesh> {
    let t = std::time::Instant::now();
    let r = specular_thread::on_stack(256 * 1024, || {
        read(format, bytes, &NoResolver, &Limits::default())
    });
    assert!(t.elapsed().as_secs() < 60, "took {:?}", t.elapsed());
    r
}

#[test]
fn collada_instancing_its_own_node_twice_is_bounded() {
    // Node k instances node k+1 twice, 40 levels: 2^40 visits before.
    let mut nodes = String::new();
    for k in 0..40 {
        let n = k + 1;
        nodes += &format!(
            "<node id=\"n{k}\"><instance_node url=\"#n{n}\"/><instance_node url=\"#n{n}\"/></node>"
        );
    }
    nodes += "<node id=\"n40\"/>";
    let dae = String::from_utf8(fixtures::dae()).unwrap().replace(
        "<library_visual_scenes>",
        &format!("<library_nodes>{nodes}</library_nodes><library_visual_scenes>"),
    );
    let dae = dae.replace(
        "<node id=\"b\">",
        "<node id=\"c\"><instance_node url=\"#n0\"/></node><node id=\"b\">",
    );
    let m = read_hostile(MeshFormat::Dae, dae.as_bytes()).unwrap();
    assert!(
        m.notes
            .iter()
            .any(|n| n.contains("instanced too many times")),
        "{:?}",
        m.notes
    );
}

#[test]
fn x3d_use_twice_per_level_is_bounded() {
    // Group k uses group k+1 twice, 40 levels: 2^40 visits before.
    let mut groups = String::from("<Group DEF=\"g40\"/>");
    for k in (0..40).rev() {
        let n = k + 1;
        groups +=
            &format!("<Group DEF=\"g{k}\"><Group USE=\"g{n}\"/><Group USE=\"g{n}\"/></Group>");
    }
    let x3d = String::from_utf8(fixtures::x3d())
        .unwrap()
        .replace("</Scene>", &format!("{groups}</Scene>"));
    let m = read_hostile(MeshFormat::X3d, x3d.as_bytes()).unwrap();
    assert!(
        m.notes.iter().any(|n| n.contains("used too many times")),
        "{:?}",
        m.notes
    );
}

#[test]
fn amf_constellations_of_empties_are_bounded() {
    // Constellation k places k+1 four times, 16 levels, the last empty: 4^16 visits before
    // (no output, so the old output cap never fired).
    let mut c = String::from("<constellation id=\"c16\"></constellation>");
    for k in 0..16 {
        let n = k + 1;
        c += &format!("<constellation id=\"c{k}\">");
        for _ in 0..4 {
            c += &format!("<instance objectid=\"c{n}\"><deltax>0</deltax></instance>");
        }
        c += "</constellation>";
    }
    let amf = String::from_utf8(fixtures::amf())
        .unwrap()
        .replace("</amf>", &format!("{c}</amf>"));
    let e = read_hostile(MeshFormat::Amf, amf.as_bytes()).unwrap_err();
    assert!(e.to_string().contains("too many AMF instances"), "{e}");
}

#[test]
fn gltf_node_listing_itself_is_bounded() {
    // Node 0 lists itself 600 000 times: each visit pushed them all again (gigabytes of
    // stack entries) before.
    let kids = vec!["0"; 600_000].join(",");
    let json = String::from_utf8(fixtures::gltf_embedded())
        .unwrap()
        .replace(
            "{\"children\": [2], ",
            &format!("{{\"children\": [2,{kids}], "),
        );
    let json = json.replace("\"nodes\": [0, 1]", "\"nodes\": [1]");
    let json = json.replace(
        "{\"mesh\": 0, \"translation\": [2, 0, 0]}",
        &format!("{{\"children\": [{kids}]}}"),
    );
    let m = read_hostile(MeshFormat::Gltf, json.as_bytes()).unwrap();
    assert!(
        m.notes.iter().any(|n| n.contains("too deep or cyclic")),
        "{:?}",
        m.notes
    );
}

#[test]
fn gltf_accessor_without_data_is_refused() {
    // 100 million positions with no buffer view: 1.2 GB of zeros before.
    let json = String::from_utf8(fixtures::gltf_embedded())
        .unwrap()
        .replace(
            "{\"bufferView\": 0, \"componentType\": 5126, \"count\": 8,",
            "{\"componentType\": 5126, \"count\": 100000000,",
        );
    let e = read_hostile(MeshFormat::Gltf, json.as_bytes()).unwrap_err();
    assert!(e.to_string().contains("without data"), "{e}");
}

#[test]
fn lwo_many_uv_map_names_are_bounded() {
    // 200 000 points and 4000 one-entry UV maps of distinct names: each map was as long
    // as the points (2.4 MB each, 9.6 GB in all) before.
    fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = id.to_vec();
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
        out
    }
    let lwo = fixtures::lwo();
    // The fixture's PNTS (8 points) grows to 200 000; its FORM size is fixed up below.
    let at = lwo.windows(4).position(|w| w == b"PNTS").unwrap();
    let n = u32::from_be_bytes(lwo[at + 4..at + 8].try_into().unwrap()) as usize;
    let mut body = lwo[12..at].to_vec();
    let mut pnts = lwo[at + 8..at + 8 + n].to_vec();
    pnts.resize(200_000 * 12, 0);
    body.extend(chunk(b"PNTS", &pnts));
    body.extend_from_slice(&lwo[at + 8 + n..]);
    for k in 0..4000 {
        let mut v = b"TXUV".to_vec();
        v.extend_from_slice(&2u16.to_be_bytes());
        let mut name = format!("m{k}").into_bytes();
        name.push(0);
        if name.len() % 2 == 1 {
            name.push(0);
        }
        v.extend(name);
        v.extend_from_slice(&0u16.to_be_bytes());
        v.extend_from_slice(&0.5f32.to_be_bytes());
        v.extend_from_slice(&0.5f32.to_be_bytes());
        body.extend(chunk(b"VMAP", &v));
    }
    let mut file = b"FORM".to_vec();
    file.extend_from_slice(&(body.len() as u32 + 4).to_be_bytes());
    file.extend_from_slice(b"LWO2");
    file.extend(body);
    read_hostile(MeshFormat::Lwo, &file).unwrap();
}

#[test]
fn usd_layer_listing_itself_is_bounded() {
    // A sublayer listing itself eight times: 8^16 stack walks before.
    let a = format!(
        "#usda 1.0\n(\n    subLayers = [{}]\n)\n",
        ["@a.usda@"; 8].join(", ")
    );
    let root = String::from_utf8(fixtures::usda()).unwrap().replace(
        "defaultPrim = \"World\"",
        "subLayers = [@a.usda@]\n    defaultPrim = \"World\"",
    );
    let files = MapResolver(HashMap::from([("a.usda".to_owned(), a.into_bytes())]));
    let t = std::time::Instant::now();
    let m = specular_thread::on_stack(256 * 1024, || {
        read(MeshFormat::Usd, root.as_bytes(), &files, &Limits::default())
    })
    .unwrap();
    assert!(t.elapsed().as_secs() < 60);
    assert!(m.triangle_count() > 0);
}

#[test]
fn ply_vertices_reserve_from_the_budget() {
    // 1 Mi vertices in 6 MiB of text: the builder's arrays need about 40 MiB.
    let n = 1usize << 20;
    let mut b = format!(
        "ply\nformat ascii 1.0\nelement vertex {n}\nproperty float x\nproperty float y\nproperty float z\nelement face 0\nproperty list uchar int vertex_indices\nend_header\n"
    )
    .into_bytes();
    for _ in 0..n {
        b.extend_from_slice(b"0 0 0\n");
    }
    let r = specular_core::budget::with_limit(16 << 20, || {
        read(MeshFormat::Ply, &b, &NoResolver, &Limits::default())
    });
    assert!(
        matches!(&r, Err(Error::TooLarge(m)) if m.contains("free RAM")),
        "{:?}",
        r.err()
    );
}

#[test]
fn threemf_parts_reserve_their_unpacked_size() {
    // A model part of 64 MiB of spaces that deflates to almost nothing.
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("3D/3dmodel.model", opts).unwrap();
    let spaces = vec![b' '; 1 << 20];
    for _ in 0..64 {
        std::io::Write::write_all(&mut zip, &spaces).unwrap();
    }
    let b = zip.finish().unwrap().into_inner();
    assert!(b.len() < 1 << 20);
    let r = specular_core::budget::with_limit(16 << 20, || {
        read(MeshFormat::ThreeMf, &b, &NoResolver, &Limits::default())
    });
    assert!(
        matches!(&r, Err(Error::TooLarge(m)) if m.contains("free RAM")),
        "{:?}",
        r.err()
    );
}
