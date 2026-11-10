//! Reading from disk: side files from the model's folder tree only, symbolic links that
//! lead out refused; and the large-mesh timing.

use std::path::PathBuf;
use std::time::Instant;

use specular_mesh::{DirResolver, ImageData, Limits, MeshFormat, Resolver, fixtures, read_path};

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("specular-mesh")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn obj_side_files_from_its_folder() {
    let d = scratch("obj");
    std::fs::create_dir_all(d.join("model")).unwrap();
    std::fs::write(d.join("model/cube.obj"), fixtures::obj()).unwrap();
    std::fs::write(d.join("model/cube.mtl"), fixtures::mtl()).unwrap();
    std::fs::write(d.join("model/tex.png"), b"png bytes").unwrap();
    let m = read_path(&d.join("model/cube.obj"), &Limits::default()).unwrap();
    assert_eq!(m.format, Some(MeshFormat::Obj));
    assert_eq!(m.materials.len(), 2);
    assert!(matches!(&m.images[0], ImageData::Bytes { bytes, .. } if bytes == b"png bytes"));
}

#[test]
fn gltf_side_buffer_from_its_folder() {
    let d = scratch("gltf");
    let (g, bin) = fixtures::gltf_side();
    std::fs::write(d.join("cube.gltf"), g).unwrap();
    std::fs::write(d.join("cube.bin"), bin).unwrap();
    let m = read_path(&d.join("cube.gltf"), &Limits::default()).unwrap();
    assert_eq!(m.triangle_count(), 24);
}

#[test]
fn escapes_are_refused() {
    let d = scratch("escape");
    std::fs::create_dir_all(d.join("inner")).unwrap();
    std::fs::write(d.join("secret.txt"), b"secret").unwrap();
    std::fs::write(d.join("inner/ok.txt"), b"ok").unwrap();
    let r = DirResolver::new(d.join("inner"));
    assert_eq!(r.read("ok.txt", 100).unwrap(), b"ok");
    assert!(r.read("ok.txt", 1).is_err(), "over the cap");
    assert!(r.read("../secret.txt", 100).is_err());
    assert!(
        r.read(&d.join("secret.txt").to_string_lossy(), 100)
            .is_err()
    );
    assert!(r.read("missing.txt", 100).is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(d.join("secret.txt"), d.join("inner/link.txt")).unwrap();
        std::os::unix::fs::symlink(&d, d.join("inner/up")).unwrap();
        assert!(
            r.read("link.txt", 100).is_err(),
            "symlink out of the folder"
        );
        assert!(
            r.read("up/secret.txt", 100).is_err(),
            "symlinked folder out"
        );
        std::os::unix::fs::symlink(d.join("inner/ok.txt"), d.join("inner/inside.txt")).unwrap();
        assert_eq!(
            r.read("inside.txt", 100).unwrap(),
            b"ok",
            "symlink within is fine"
        );
    }
}

/// Parse time of a generated 10M-triangle binary STL (run with `--ignored --nocapture`).
#[test]
#[ignore = "timing; writes a 500 MB file"]
fn large_stl_timing() {
    let m = fixtures::torus_knot(2500, 2000);
    let d = scratch("large");
    let mut stl = vec![0u8; 80];
    stl.extend_from_slice(&(m.triangle_count() as u32).to_le_bytes());
    for t in m.indices.chunks_exact(3) {
        stl.extend_from_slice(&[0; 12]);
        for &i in t {
            for c in m.positions[i as usize] {
                stl.extend_from_slice(&c.to_le_bytes());
            }
        }
        stl.extend_from_slice(&[0, 0]);
    }
    let path = d.join("large.stl");
    std::fs::write(&path, &stl).unwrap();
    drop(stl);
    let t = Instant::now();
    let m2 = read_path(&path, &Limits::default()).unwrap();
    eprintln!(
        "parse {} triangles, {} vertices: {:?}",
        m2.triangle_count(),
        m2.vertex_count(),
        t.elapsed()
    );
    assert_eq!(m2.triangle_count(), m.triangle_count());
    assert_eq!(m2.vertex_count(), m.vertex_count());
    let t = Instant::now();
    let _ = specular_mesh::preview::render(&m2, &[], 256, 256);
    eprintln!("preview 256²: {:?}", t.elapsed());
    let _ = std::fs::remove_dir_all(&d);
}
