//! Segment surfaces against their voxels: closed (watertight) and holding the voxel count ×
//! voxel volume, before and after volume-keeping smoothing and decimation.

use specular_dicom::volume::{Geometry, Volume, VoxelKind};

use crate::iso::{Mesh, label_surface};

/// A U16 label volume, anisotropic voxels, tilted slices: `label(i, j, k)`.
fn labels(dims: [u32; 3], label: impl Fn(i64, i64, i64) -> u16) -> Volume {
    let (px, sp, shear) = (0.8, 1.5, 0.2);
    let pos: Vec<[f64; 3]> = (0..dims[2])
        .map(|k| [3.0, -4.0 + f64::from(k) * shear, f64::from(k) * sp])
        .collect();
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [px, px],
        [dims[0], dims[1]],
    );
    let mut v = Volume::new(g, VoxelKind::U16, 1).unwrap();
    let mut buf = vec![0f32; (dims[0] * dims[1]) as usize];
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                buf[(j * dims[0] + i) as usize] =
                    f32::from(label(i64::from(i), i64::from(j), i64::from(k)));
            }
        }
        v.put(k as usize, &buf, dims[0], dims[1]).unwrap();
    }
    v
}

fn voxel_ml(v: &Volume) -> f64 {
    // The slice spacing along the normal (the shear keeps it 1.5 mm apart in z).
    let g = &v.geometry;
    let vm = g.voxel_mm();
    vm[0] * vm[1] * vm[2]
}

fn count(v: &Volume, l: f32) -> usize {
    let [w, h, d] = v.geometry.dims;
    let mut n = 0;
    for k in 0..d {
        for j in 0..h {
            for i in 0..w {
                n += usize::from(v.value(i, j, k) == l);
            }
        }
    }
    n
}

/// An ellipsoid of label 1 and, touching it, a box of label 2 (the box reaches the volume's
/// edge, so its surface closes there).
fn two_segments() -> Volume {
    labels([48, 44, 30], |i, j, k| {
        let (x, y, z) = (
            (i as f64 - 18.0) / 12.0,
            (j as f64 - 22.0) / 14.0,
            (k as f64 - 15.0) / 9.0,
        );
        if x * x + y * y + z * z <= 1.0 {
            1
        } else if (31..48).contains(&i) && (10..30).contains(&j) && (4..20).contains(&k) {
            2
        } else {
            0
        }
    })
}

#[test]
fn each_segment_is_closed_with_its_voxel_volume() {
    let v = two_segments();
    for (l, tol) in [(1.0f32, 0.03), (2.0, 0.03)] {
        let m = label_surface(&v, &|x| x == l);
        assert!(m.is_closed(), "label {l} not closed");
        let want = count(&v, l) as f64 * voxel_ml(&v);
        let got = m.volume();
        assert!(
            (got - want).abs() / want < tol,
            "label {l}: {got:.0} mm³ vs {want:.0}"
        );
        assert_eq!(m.parts().1, 1, "label {l}: one part");
    }
}

#[test]
fn smoothing_keeps_the_volume_and_the_surface_closed() {
    let v = two_segments();
    let mut m = label_surface(&v, &|x| x == 1.0);
    let before = m.volume();
    let pts = m.vertices.clone();
    m.smooth_keeping_volume(10);
    assert!(m.is_closed());
    assert!(
        (m.volume() - before).abs() / before < 1e-9,
        "{} vs {before}",
        m.volume()
    );
    // It did move the vertices (the staircase is gone).
    let moved = pts
        .iter()
        .zip(&m.vertices)
        .filter(|(a, b)| (0..3).any(|c| (a[c] - b[c]).abs() > 1e-3))
        .count();
    assert!(moved > pts.len() / 2);
}

#[test]
fn smoothing_keeps_each_part_where_it_is() {
    // Two separate blobs of one label: each keeps its own volume and centre.
    let v = labels([40, 20, 12], |i, j, k| {
        let d = |c: f64| {
            ((i as f64 - c).powi(2) + (j as f64 - 10.0).powi(2) + ((k as f64 - 6.0) * 2.0).powi(2))
                .sqrt()
        };
        u16::from(d(10.0) <= 6.0 || d(29.0) <= 4.0)
    });
    let mut m = label_surface(&v, &|x| x == 1.0);
    let (part, n) = m.parts();
    assert_eq!(n, 2);
    let vols = |m: &Mesh| {
        let mut out = [0.0; 2];
        for t in &m.triangles {
            let [a, b, c] = t.map(|i| m.vertices[i as usize]);
            out[part[t[0] as usize] as usize] += crate::dot(a, crate::cross(b, c)) / 6.0;
        }
        out
    };
    let before = vols(&m);
    m.smooth_keeping_volume(8);
    let after = vols(&m);
    for p in 0..2 {
        assert!((before[p] - after[p]).abs() / before[p] < 1e-9, "part {p}");
    }
}

#[test]
fn decimation_reaches_its_target_and_stays_closed() {
    let v = two_segments();
    let mut m = label_surface(&v, &|x| x == 1.0);
    m.smooth_keeping_volume(10);
    let full = m.triangles.len();
    let before = m.volume();
    let target = full / 10;
    m.decimate(target);
    assert!(
        m.triangles.len() <= target,
        "{} > {target}",
        m.triangles.len()
    );
    assert!(m.is_closed(), "not closed after decimation");
    assert!(
        (m.volume() - before).abs() / before < 0.02,
        "{} vs {before}",
        m.volume()
    );
    assert_eq!(m.parts().1, 1);
    // Unused vertices went.
    let mut used = vec![false; m.vertices.len()];
    for t in &m.triangles {
        for &i in t {
            used[i as usize] = true;
        }
    }
    assert!(used.iter().all(|&u| u));
    eprintln!("decimated {full} → {} triangles", m.triangles.len());
}

#[test]
fn decimation_stops_before_breaking_a_tiny_surface() {
    // One voxel: the smallest closed surface marching tetrahedra makes.
    let v = labels([3, 3, 3], |i, j, k| u16::from(i == 1 && j == 1 && k == 1));
    let mut m = label_surface(&v, &|x| x == 1.0);
    assert!(m.is_closed());
    m.decimate(0);
    assert!(m.is_closed(), "{} triangles", m.triangles.len());
    assert!(m.triangles.len() >= 4);
    assert!(m.volume() > 0.0);
}

#[test]
fn binary_stl_holds_every_triangle_and_the_title() {
    let v = two_segments();
    let m = label_surface(&v, &|x| x == 2.0);
    let mut out = Vec::new();
    m.write_stl_titled(&mut out, "solid kidney (left) — Niere")
        .unwrap();
    assert_eq!(out.len(), 84 + 50 * m.triangles.len());
    assert!(
        !out.starts_with(b"solid"),
        "binary STL must not start with solid"
    );
    assert!(out[..80].starts_with(b"_olid kidney (left) ? Niere"));
    let n = u32::from_le_bytes(out[80..84].try_into().unwrap());
    assert_eq!(n as usize, m.triangles.len());
}

/// Writes the two segments' surfaces (raw, smoothed, smoothed and decimated) and their masks
/// for the Python oracle (`tools/quality/segments_oracle.py`: trimesh and scikit-image):
/// `SPECULAR_ORACLE_DIR=<dir> cargo test -p specular-render-volume dump_for_oracle -- --ignored`.
#[test]
#[ignore = "writes files for the Python oracle"]
fn dump_for_oracle() {
    let Some(dir) = std::env::var_os("SPECULAR_ORACLE_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let v = two_segments();
    let [w, h, d] = v.geometry.dims;
    let vm = v.geometry.voxel_mm();
    for l in [1u16, 2] {
        let lf = f32::from(l);
        let mut mask = Vec::with_capacity((w * h * d) as usize);
        for k in 0..d {
            for j in 0..h {
                for i in 0..w {
                    mask.push(u8::from(v.value(i, j, k) == lf));
                }
            }
        }
        std::fs::write(dir.join(format!("label{l}.mask")), &mask).unwrap();
        std::fs::write(
            dir.join(format!("label{l}.json")),
            format!(
                "{{\"dims\": [{w}, {h}, {d}], \"voxel\": [{}, {}, {}]}}",
                vm[0], vm[1], vm[2]
            ),
        )
        .unwrap();
        let save = |m: &Mesh, name: &str| {
            let mut out = Vec::new();
            m.write_stl_titled(&mut out, name).unwrap();
            std::fs::write(dir.join(format!("label{l}-{name}.stl")), out).unwrap();
        };
        let mut m = label_surface(&v, &|x| x == lf);
        save(&m, "raw");
        m.smooth_keeping_volume(10);
        save(&m, "smooth");
        let n = m.triangles.len() / 10;
        m.decimate(n);
        save(&m, "decimated");
    }
}
