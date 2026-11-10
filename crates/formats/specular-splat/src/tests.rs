use super::*;
use crate::fixtures::{compressed_ply_file, ply_file, scene, splat_file, spz_file, spz4_file};

fn close(a: &[f32], b: &[f32], tol: f32) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

/// The scene read from its 3DGS PLY, the reference the other formats are checked against.
fn reference() -> Splats {
    read_ply(&ply_file(&scene(600), 3, false), &Limits::default()).unwrap()
}

#[test]
fn ply_reads_every_property() {
    let s = reference();
    assert_eq!((s.len(), s.in_file, s.sh_degree), (600, 600, 3));
    assert_eq!(s.format, SplatFormat::Ply);
    assert_eq!(s.sh.len(), 600 * 45);
    let g = scene(600);
    // Up was −y: (x, y, z) → (x, z, −y).
    let p = g[7].pos;
    assert!(close(&s.positions[7], &[p[0], p[2], -p[1]], 1e-6));
    let c = covariance(g[7].log_scale.map(f32::exp), g[7].rot);
    // The trace survives the frame change.
    let tr = |c: &[f32; 6]| c[0] + c[3] + c[5];
    assert!((tr(&c) - tr(&s.covariances[7])).abs() < 1e-5);
    let want = g[7].dc.map(|d| 0.5 + SH_C0 * d);
    assert!(close(&s.colors[7][..3], &want, 1e-6));
    assert!((s.colors[7][3] - 1.0 / (1.0 + (-2.0f32).exp())).abs() < 1e-6);
    // Channel-major in the file, coefficient-major here.
    assert!((f16_to_f32(s.sh[7 * 45]) - g[7].rest[0]).abs() < 1e-3);
    assert!((f16_to_f32(s.sh[7 * 45 + 4]) - g[7].rest[16]).abs() < 1e-3);
    assert!(!s.bounds.is_empty());
    // ASCII says the same.
    let a = read_ply(&ply_file(&scene(600), 3, true), &Limits::default()).unwrap();
    for i in 0..600 {
        assert!(close(&a.positions[i], &s.positions[i], 1e-5));
        assert!(close(&a.covariances[i], &s.covariances[i], 1e-5));
    }
}

#[test]
fn every_format_reads_the_same_scene() {
    let r = reference();
    let g = scene(600);
    let l = Limits::default();
    let cases: Vec<(&str, Splats, f32, f32)> = vec![
        (
            "splat",
            read_splat(&splat_file(&g), &l).unwrap(),
            1e-6,
            0.004,
        ),
        (
            "compressed",
            read_ply(&compressed_ply_file(&g), &l).unwrap(),
            0.003,
            0.004,
        ),
        (
            "spz2",
            read_spz(&spz_file(&g, 2, 1), &l).unwrap(),
            0.0005,
            0.02,
        ),
        (
            "spz3",
            read_spz(&spz_file(&g, 3, 1), &l).unwrap(),
            0.0005,
            0.02,
        ),
        (
            "spz4",
            read_spz(&spz4_file(&g, 1), &l).unwrap(),
            0.0005,
            0.02,
        ),
    ];
    for (name, s, ptol, ctol) in cases {
        assert_eq!(s.len(), 600, "{name}");
        for i in 0..600 {
            assert!(
                close(&s.positions[i], &r.positions[i], ptol),
                "{name} {i}: {:?} {:?}",
                s.positions[i],
                r.positions[i]
            );
            assert!(
                close(&s.colors[i], &r.colors[i], ctol),
                "{name} {i}: {:?} {:?}",
                s.colors[i],
                r.colors[i]
            );
            // Covariances within the quantisation of scale and rotation.
            let rel = s.covariances[i]
                .iter()
                .zip(&r.covariances[i])
                .map(|(a, b)| (a - b).abs())
                .fold(0f32, f32::max)
                / r.covariances[i][0]
                    .abs()
                    .max(r.covariances[i][3])
                    .max(r.covariances[i][5]);
            // Version 2 keeps x, y, z in 8 bits and derives w: near w = 0 the angle is coarse.
            let tol = if name == "spz2" { 0.3 } else { 0.12 };
            assert!(rel < tol, "{name} {i}: {rel}");
        }
        // The view-dependent colour agrees where the file has harmonics.
        if s.sh_degree > 0 {
            for d in [[0.0f32, 1.0, 0.0], [0.6, 0.0, -0.8], [-0.48, 0.6, 0.64]] {
                for i in [0, 37, 299] {
                    let (a, b) = (s.color_towards(i, d), r.color_towards(i, d));
                    let tol = if name.starts_with("spz") { 0.03 } else { 0.02 };
                    assert!(close(&a, &b, tol), "{name} {i} {d:?}: {a:?} {b:?}");
                }
            }
        }
    }
}

#[test]
fn ply_kinds_are_told_apart() {
    let g = scene(10);
    assert!(ply_is_splats(&ply_file(&g, 0, false)));
    assert!(ply_is_splats(&compressed_ply_file(&g)));
    let mesh = b"ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";
    assert!(!ply_is_splats(mesh));
    let cloud = b"ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nproperty float opacity\nend_header\n0 0 0 1\n";
    assert!(!ply_is_splats(cloud));
    assert!(read_ply(mesh, &Limits::default()).is_err());
}

#[test]
fn harmonic_degrees_and_caps() {
    let g = scene(1000);
    for d in 0..=3 {
        let s = read_ply(&ply_file(&g, d, false), &Limits::default()).unwrap();
        assert_eq!(s.sh_degree, d);
        assert_eq!(s.sh.len(), 1000 * sh_coeffs(d) * 3);
    }
    let l = Limits {
        max_splats: 300,
        ..Limits::default()
    };
    let s = read_splat(&splat_file(&g), &l).unwrap();
    assert!(s.len() <= 300 && s.len() >= 250, "{}", s.len());
    assert_eq!(s.in_file, 1000);
    assert!(s.notes.iter().any(|n| n.contains("cap")));
}

#[test]
fn junk_is_an_error_not_a_panic() {
    let l = Limits::default();
    assert!(read_splat(&[], &l).is_err());
    assert!(read_splat(&[0; 33], &l).is_err());
    assert!(read_spz(b"NGSP\x04\0\0\0", &l).is_err());
    assert!(read_spz(&fixtures::gzip_stored(b"NGSP"), &l).is_err());
    let mut f = spz_file(&scene(50), 3, 1);
    f.truncate(f.len() / 2);
    assert!(read_spz(&f, &l).is_err());
    let full = ply_file(&scene(50), 3, false);
    for cut in [10, 200, full.len() - 7] {
        assert!(read_ply(&full[..cut], &l).is_err(), "{cut}");
    }
    let mut v4 = spz4_file(&scene(50), 0);
    v4[16] = 0xff;
    assert!(read_spz(&v4, &l).is_err());
    // Truncations and bit flips anywhere: errors or splats, never a panic.
    let files = [
        splat_file(&scene(40)),
        ply_file(&scene(40), 1, false),
        compressed_ply_file(&scene(40)),
        spz_file(&scene(40), 2, 1),
        spz4_file(&scene(40), 1),
    ];
    for f in &files {
        for i in (0..f.len()).step_by(7) {
            let mut b = f.clone();
            b[i] ^= 0x5a;
            let _ = read_splat(&b, &l);
            let _ = read_ply(&b, &l);
            let _ = read_spz(&b, &l);
            let _ = read_ply(&f[..i], &l);
            let _ = read_spz(&f[..i], &l);
        }
    }
    // Non-finite splats are dropped with a note.
    let mut g = scene(20);
    g[3].pos[1] = f32::NAN;
    let s = read_ply(&ply_file(&g, 0, false), &l).unwrap();
    assert_eq!(s.len(), 19);
    assert!(s.notes.iter().any(|n| n.contains("non-finite")));
}

#[test]
fn the_preview_draws_the_scene() {
    let s = reference();
    let (w, h) = (160, 120);
    let px = preview::render(&s, w, h);
    assert_eq!(px.len(), (w * h * 4) as usize);
    let alpha: Vec<u8> = px.chunks_exact(4).map(|p| p[3]).collect();
    let covered = alpha.iter().filter(|&&a| a > 128).count();
    assert!(covered > 1500, "{covered}");
    assert_eq!(alpha[0], 0, "corner empty");
    // The ring's colours vary around it.
    let hues: std::collections::HashSet<[u8; 3]> = px
        .chunks_exact(4)
        .filter(|p| p[3] > 200)
        .map(|p| [p[0] >> 5, p[1] >> 5, p[2] >> 5])
        .collect();
    assert!(hues.len() > 10, "{}", hues.len());
    // From the other side the view-dependent colour differs.
    let mut v = preview::iso_view(&s, w, h);
    let back = preview::render_view(&s, &v, w, h);
    assert_eq!(back, px);
    v.forward = v.forward.map(|c| -c);
    v.right = v.right.map(|c| -c);
    let other = preview::render_view(&s, &v, w, h);
    assert_ne!(other, px);
}
