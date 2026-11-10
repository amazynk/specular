//! Volume geometry against analytic expectations: stacking and refusals, the patient ↔ voxel
//! mapping (tilt and uneven spacing included), reformatted planes of the phantom, the slice
//! table the GPU reads, downsampling plans and the interleaved load order.

use super::phantom::{self, hu};
use super::*;
use crate::Dicom;
use crate::series::{Instance, assemble};

const AXIAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

fn inst(sop: &str, z: f64) -> Instance {
    Instance {
        study_uid: "1.2".into(),
        series_uid: "1.2.3".into(),
        sop_uid: sop.into(),
        position: Some([-10.0, -10.0, z]),
        orientation: Some(AXIAL),
        frames: 1,
        ..Default::default()
    }
}

fn series_of(list: Vec<Instance>) -> Series<usize> {
    let mut s = assemble(list.into_iter().enumerate().collect());
    assert_eq!(s.len(), 1);
    s.remove(0)
}

#[test]
fn stacks_in_order_and_refuses_what_is_not_a_volume() {
    let zs = [4.0, 0.0, 2.0, 8.0, 6.0];
    let s = series_of(
        zs.iter()
            .enumerate()
            .map(|(i, z)| inst(&format!("1.{i}"), *z))
            .collect(),
    );
    let st = stack(&s).unwrap();
    let order: Vec<f64> = st.slices.iter().map(|s| s.position[2]).collect();
    assert_eq!(order, [0.0, 2.0, 4.0, 6.0, 8.0]);
    assert_eq!(st.spacing, 2.0);
    assert!(!st.irregular && st.tilt.is_none());
    assert_eq!(st.normal, [0.0, 0.0, 1.0]);

    // A missing slice.
    let gap = series_of(
        [0.0, 2.0, 4.0, 8.0, 10.0]
            .iter()
            .enumerate()
            .map(|(i, z)| inst(&format!("1.{i}"), *z))
            .collect(),
    );
    assert_eq!(stack(&gap), Err(Refusal::Missing(1)));
    // Two images at one position (a second phase).
    let shared = series_of(
        [0.0, 2.0, 2.0, 4.0]
            .iter()
            .enumerate()
            .map(|(i, z)| inst(&format!("1.{i}"), *z))
            .collect(),
    );
    assert_eq!(stack(&shared), Err(Refusal::SharedPosition(1)));
    // Too few.
    let two = series_of(vec![inst("1.1", 0.0), inst("1.2", 1.0)]);
    assert_eq!(stack(&two), Err(Refusal::TooFew(2)));
    // A slice turned 90° about the normal: the rows no longer agree.
    let mut turned = vec![inst("1.1", 0.0), inst("1.2", 1.0), inst("1.3", 2.0)];
    turned[1].orientation = Some([0.0, 1.0, 0.0, -1.0, 0.0, 0.0]);
    assert_eq!(stack(&series_of(turned)), Err(Refusal::NotParallel));
    // No position.
    let mut lost = vec![inst("1.1", 0.0), inst("1.2", 1.0), inst("1.3", 2.0)];
    lost[2].position = None;
    lost[2].instance_number = Some(3);
    assert!(stack(&series_of(lost)).is_err());
}

#[test]
fn uneven_spacing_is_kept_exact() {
    let zs = [0.0, 1.0, 2.0, 3.3, 4.3, 5.0, 6.0];
    let s = series_of(
        zs.iter()
            .enumerate()
            .map(|(i, z)| inst(&format!("1.{i}"), *z))
            .collect(),
    );
    let st = stack(&s).unwrap();
    assert!(st.irregular);
    let pick: Vec<usize> = (0..st.slices.len()).collect();
    let g = st.geometry(&pick, 8, 8, [0.5, 0.5]);
    for (k, z) in zs.iter().enumerate() {
        let v = g.to_voxel([-10.0, -10.0, *z]);
        assert!((v[2] - k as f64).abs() < 1e-9, "{v:?} {k}");
    }
    // Halfway between 2.0 and 3.3 is halfway between slices 2 and 3.
    assert!((g.to_voxel([-10.0, -10.0, 2.65])[2] - 2.5).abs() < 1e-9);
    // The GPU table agrees with the exact lookup within a hundredth of a slice.
    let t = g.table(g.table_nodes());
    for i in 0..200 {
        let a = -0.5 + 6.5 * i as f64 / 199.0;
        let x = (a - t.start) / t.step;
        let n = x.floor().clamp(0.0, (t.entries.len() - 2) as f64) as usize;
        let f = (x - n as f64) as f32;
        let s = t.entries[n][0] + (t.entries[n + 1][0] - t.entries[n][0]) * f;
        let exact = g.slice_at(a).0 as f32;
        assert!((s - exact).abs() < 0.03, "{a}: {s} vs {exact}");
    }
}

#[test]
fn multi_frame_files_stack_by_frame_positions() {
    let mut m = inst("1.1", 0.0);
    m.frames = 4;
    m.frame_positions = vec![
        [0.0, 0.0, 3.0],
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.5],
        [0.0, 0.0, 4.5],
    ];
    let st = stack(&series_of(vec![m])).unwrap();
    let frames: Vec<u32> = st.slices.iter().map(|s| s.frame).collect();
    assert_eq!(frames, [1, 2, 0, 3]);
    assert_eq!(st.spacing, 1.5);
}

#[test]
fn patient_and_voxel_coordinates_invert() {
    // Oblique orientation, tilt and uneven spacing at once.
    let row = normalise([0.9, 0.1, 0.2]).unwrap();
    let col0 = [0.0, 1.0, -0.3];
    let col = normalise(sub(col0, row.map(|c| c * dot(col0, row)))).unwrap();
    let n = normalise(cross(row, col)).unwrap();
    let shear = [0.3 * col[0], 0.3 * col[1], 0.3 * col[2]];
    let mut along = 0.0;
    let positions: Vec<[f64; 3]> = (0..9)
        .map(|k| {
            along += if k % 3 == 0 { 1.3 } else { 1.0 };
            [0, 1, 2].map(|c| 5.0 + along * n[c] + k as f64 * shear[c])
        })
        .collect();
    let g = Geometry::new(&positions, row, col, [0.7, 0.8], [20, 30]);
    for v in [
        [0.0, 0.0, 0.0],
        [3.5, 7.25, 2.5],
        [19.0, 29.0, 8.0],
        [1.0, 2.0, 6.75],
    ] {
        let p = g.to_patient(v);
        let back = g.to_voxel(p);
        for c in 0..3 {
            assert!((back[c] - v[c]).abs() < 1e-9, "{v:?} → {p:?} → {back:?}");
        }
    }
    // Each slice's first pixel is its Image Position.
    for (k, p) in positions.iter().enumerate() {
        let q = g.to_patient([0.0, 0.0, k as f64]);
        assert!(dot(sub(q, *p), sub(q, *p)) < 1e-18);
    }
}

/// Distance from `p` to the nearest boundary of the phantom, mm.
fn edge_distance(p: [f64; 3]) -> f64 {
    let [x, y, z] = p;
    let mut d = (BODY_RADIUS_F - (x * x + y * y).sqrt()).abs();
    d = d.min((phantom::BODY_HALF_LENGTH - z.abs()).abs());
    for (c, r, _) in phantom::SPHERES {
        let q = sub(p, c);
        d = d.min((dot(q, q).sqrt() - r).abs());
    }
    let rr = (x * x + (z - phantom::ROD_Z).powi(2)).sqrt();
    d = d.min((rr - phantom::ROD_RADIUS).abs());
    d.min((y.abs() - 120.0).abs())
}

const BODY_RADIUS_F: f64 = phantom::BODY_RADIUS;

/// Samples the plane through `centre` spanned by `u` and `v` (unit, mm) on a grid, comparing
/// with the analytic phantom wherever the trilinear footprint stays inside one region.
fn check_plane(vol: &Volume, centre: [f64; 3], u: [f64; 3], v: [f64; 3], margin: f64) -> usize {
    let mut checked = 0;
    for a in -40..=40 {
        for b in -40..=40 {
            let p = [0, 1, 2].map(|c| centre[c] + a as f64 * 4.0 * u[c] + b as f64 * 4.0 * v[c]);
            if edge_distance(p) < margin {
                continue;
            }
            let Some(x) = vol.sample_patient(p) else {
                continue;
            };
            assert!((x - hu(p)).abs() < 0.01, "at {p:?}: {x} vs {}", hu(p));
            checked += 1;
        }
    }
    checked
}

#[test]
fn reformatted_planes_of_the_phantom_match_analytic_values() {
    let vol = phantom::volume([96, 96, 80], 3.5, 3.0, 0.0);
    let c = vol.geometry.centre();
    assert!(dot(c, c) < 1e-12, "centred: {c:?}");
    // A sample is exact when every voxel it blends lies inside one region: 1.5 voxels.
    let margin = 1.5 * (3.5f64 * 3.5 * 2.0 + 9.0).sqrt();
    let x = [1.0, 0.0, 0.0];
    let y = [0.0, 1.0, 0.0];
    let z = [0.0, 0.0, 1.0];
    // Axial through the bone sphere, sagittal through the rod, coronal through the contrast.
    let axial = check_plane(&vol, [0.0, 0.0, 0.0], x, y, margin);
    let sagittal = check_plane(&vol, [0.0, 0.0, 0.0], y, z, margin);
    let coronal = check_plane(&vol, [0.0, 70.0, 0.0], x, z, margin);
    assert!(axial > 2000 && sagittal > 1000 && coronal > 1000);
    // The planes cross the features they should.
    let near = |p: [f64; 3], v: f32| vol.sample_patient(p).is_some_and(|x| (x - v).abs() < 0.01);
    assert!(near([60.0, 0.0, 0.0], 1000.0));
    assert!(near([0.0, 0.0, 100.0], phantom::ROD));
    assert!(near([0.0, 70.0, -60.0], 300.0));
    assert!(near([-60.0, -30.0, 40.0], -100.0));
    // Outside the volume: nothing.
    assert_eq!(vol.sample_patient([0.0, 0.0, 200.0]), None);
}

#[test]
fn oblique_planes_match_the_phantom_and_the_affine() {
    let vol = phantom::volume([96, 96, 80], 3.5, 3.0, 0.0);
    let margin = 1.5 * (3.5f64 * 3.5 * 2.0 + 9.0).sqrt();
    let u = normalise([0.8, 0.3, 0.52]).unwrap();
    let w0 = [-0.3, 0.9, 0.1];
    let v = normalise(sub(w0, u.map(|c| c * dot(w0, u)))).unwrap();
    assert!(check_plane(&vol, [10.0, -5.0, 3.0], u, v, margin) > 1500);
    // For evenly spaced slices the 4×4 affine and the exact mapping agree.
    let m = vol.geometry.affine();
    for t in [[0.0, 0.0, 0.0], [10.5, 20.25, 30.75], [95.0, 95.0, 79.0]] {
        let p = vol.geometry.to_patient(t);
        for r in 0..3 {
            let q = m[r][0] * t[0] + m[r][1] * t[1] + m[r][2] * t[2] + m[r][3];
            assert!((q - p[r]).abs() < 1e-9);
        }
    }
}

#[test]
fn gantry_tilt_is_corrected_by_the_shear() {
    let tilt = 20.0;
    let vol = phantom::volume([96, 96, 80], 3.5, 3.0, tilt);
    let st = stack_of(
        phantom::positions([96, 96, 80], 3.5, 3.0, tilt)
            .into_iter()
            .enumerate()
            .map(|(k, p)| Slice {
                key: k,
                frame: 0,
                position: p,
            })
            .collect(),
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    )
    .unwrap();
    assert!((st.tilt.unwrap() - tilt).abs() < 1e-6);
    // The affine's slice column leans by the tilt.
    let m = vol.geometry.affine();
    let lean = (m[1][2] / m[2][2]).atan().to_degrees();
    assert!((lean - tilt).abs() < 1e-6, "{lean}");
    // Corrected: the spheres are where they are, round in the sagittal plane.
    let margin = 1.5 * (3.5f64 * 3.5 * 2.0 + 9.0).sqrt() + 1.0;
    assert!(
        check_plane(
            &vol,
            [60.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            margin
        ) > 900
    );
    // The contrast sphere's edges along y, from the centre, are at its radius.
    let (c, r, v) = phantom::SPHERES[2];
    let edge = |dir: f64| {
        let mut d = 0.0;
        while vol
            .sample_patient([c[0], c[1] + dir * d, c[2]])
            .is_some_and(|x| x > 170.0)
        {
            d += 0.05;
        }
        d
    };
    assert!(vol.sample_patient(c).is_some_and(|x| (x - v).abs() < 0.01));
    for dir in [-1.0, 1.0] {
        assert!(
            (edge(dir) - r).abs() < 3.5,
            "edge {} vs radius {r}",
            edge(dir)
        );
    }
    // Ignoring the tilt (stacking straight up) misplaces it by z·tan(tilt).
    let straight = Geometry {
        shift: vec![[0.0, 0.0]; vol.geometry.slices()],
        ..vol.geometry.clone()
    };
    let naive = Volume {
        geometry: straight,
        ..vol.clone()
    };
    let p = [0.0, 70.0, -60.0];
    assert!(
        naive
            .sample_patient(p)
            .is_some_and(|x| (x - 300.0).abs() > 100.0)
    );
}

#[test]
fn loading_slices_blend_their_loaded_neighbours() {
    let full = phantom::volume([16, 16, 9], 20.0, 20.0, 0.0);
    let mut part = Volume::new(full.geometry.clone(), VoxelKind::I16, 1).unwrap();
    assert_eq!(part.sample([4.0, 4.0, 4.0]), None);
    let take = |part: &mut Volume, k: usize| {
        let vals: Vec<f32> = (0..256).map(|i| full.voxels.get(k * 256 + i)).collect();
        part.put(k, &vals, 16, 16).unwrap();
    };
    take(&mut part, 0);
    take(&mut part, 8);
    assert_eq!(part.neighbours()[4], [0, 8]);
    let (a, b) = (full.value(8, 8, 0), full.value(8, 8, 8));
    let mid = part.sample([8.0, 8.0, 4.0]).unwrap();
    assert!((mid - (a + (b - a) * 0.5)).abs() < 1e-3);
    for k in 1..8 {
        take(&mut part, k);
    }
    assert!(part.complete());
    assert_eq!(part.sample([8.0, 8.0, 4.0]), Some(full.value(8, 8, 4)));
}

#[test]
fn voxels_promote_when_a_slice_needs_it() {
    let g = Geometry::new(
        &[[0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, 2.0]],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0],
        [2, 2],
    );
    let mut v = Volume::new(g, VoxelKind::I16, 1).unwrap();
    assert!(!v.put(0, &[-1024.0, 0.0, 1.0, 3071.0], 2, 2).unwrap());
    assert!(v.put(1, &[0.5, 0.0, 0.0, 0.0], 2, 2).unwrap());
    assert_eq!(v.voxels.kind(), VoxelKind::F32);
    assert_eq!(v.value(0, 0, 0), -1024.0);
    assert_eq!(v.value(0, 0, 1), 0.5);
    assert_eq!(v.put(2, &[0.0; 4], 3, 2), Err(Refusal::Mismatch));
}

#[test]
fn plans_fit_the_texture_limit_and_the_budget() {
    let p = Plan::new([512, 512, 500], VoxelKind::I16, 0.7, 1.0, 2048, 1 << 30).unwrap();
    assert_eq!((p.step_xy, p.step_z, p.dims), (1, 1, [512, 512, 500]));
    assert_eq!(p.bytes, 512 * 512 * 500 * 2);
    // Over the side limit: halved in-plane, every other slice.
    let p = Plan::new([4096, 4096, 3000], VoxelKind::I16, 0.1, 0.1, 2048, u64::MAX).unwrap();
    assert_eq!((p.step_xy, p.step_z), (2, 2));
    // Over the budget: the finer axis first (in-plane 0.5 mm against 2 mm slices).
    let p = Plan::new([512, 512, 200], VoxelKind::I16, 0.5, 2.0, 2048, 40 << 20).unwrap();
    assert_eq!((p.step_xy, p.step_z), (2, 1));
    assert!(p.bytes <= 40 << 20);
    assert_eq!(p.pick(7).len(), 7);
    let q = Plan::new([64, 64, 9], VoxelKind::F32, 1.0, 1.0, 4, u64::MAX).unwrap();
    assert_eq!(q.dims, [4, 4, 3]);
    assert_eq!(q.pick(9), [0, 3, 6]);
    assert!(matches!(
        Plan::new([512, 512, 500], VoxelKind::I16, 0.7, 1.0, 2048, 1000),
        Err(Refusal::TooLarge { .. })
    ));
}

#[test]
fn planned_geometry_centres_the_averaged_blocks() {
    let pos = phantom::positions([40, 30, 9], 2.0, 3.0, 0.0);
    let slices: Vec<Slice<usize>> = pos
        .iter()
        .enumerate()
        .map(|(k, p)| Slice {
            key: k,
            frame: 0,
            position: *p,
        })
        .collect();
    let st = stack_of(slices, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
    let plan = Plan {
        step_xy: 4,
        step_z: 2,
        dims: [10, 8, 5],
        bytes: 0,
    };
    let g = st.planned(&plan, 40, 30, [2.0, 2.0]);
    assert_eq!(g.dims, [10, 8, 5]);
    assert_eq!(g.pixel, [8.0, 8.0]);
    // Voxel (0, 0) averages pixels 0–3: its centre is 1.5 pixels in.
    let p = g.to_patient([0.0, 0.0, 1.0]);
    let want = [pos[2][0] + 3.0, pos[2][1] + 3.0, pos[2][2]];
    for c in 0..3 {
        assert!((p[c] - want[c]).abs() < 1e-9, "{p:?} vs {want:?}");
    }
    // Filled from full-resolution slices, a voxel samples as its block's mean at its centre.
    let mut v = Volume::new(g, VoxelKind::F32, 4).unwrap();
    let full = phantom::volume([40, 30, 9], 2.0, 3.0, 0.0);
    let n = 40 * 30;
    for (k, &i) in plan.pick(9).iter().enumerate() {
        let vals: Vec<f32> = (0..n).map(|x| full.voxels.get(i * n + x)).collect();
        v.put(k, &vals, 40, 30).unwrap();
    }
    let mut mean = 0.0;
    for j in 12..16 {
        for i in 16..20 {
            mean += full.voxels.get(4 * n + j * 40 + i) / 16.0;
        }
    }
    let q = v.geometry.to_patient([4.0, 3.0, 2.0]);
    let got = v.sample_patient(q).unwrap();
    assert!((got - mean).abs() < 1e-3, "{got} vs {mean}");
    let centre = full.geometry.to_patient([17.5, 13.5, 4.0]);
    for c in 0..3 {
        assert!((q[c] - centre[c]).abs() < 1e-9);
    }
}

#[test]
fn downsampled_slices_average_their_blocks() {
    let g = Geometry::new(
        &[[0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, 2.0]],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 2.0],
        [2, 2],
    );
    let mut v = Volume::new(g, VoxelKind::F32, 2).unwrap();
    let vals: Vec<f32> = (0..12).map(|i| i as f32).collect(); // 4 × 3
    v.put(0, &vals, 4, 3).unwrap();
    assert_eq!(v.value(0, 0, 0), (0.0 + 1.0 + 4.0 + 5.0) / 4.0);
    assert_eq!(v.value(1, 1, 0), (10.0 + 11.0) / 2.0);
}

#[test]
fn load_order_is_coarse_first_and_complete() {
    for n in [1, 2, 3, 17, 100, 500] {
        let o = load_order(n);
        let mut s = o.clone();
        s.sort();
        assert_eq!(s, (0..n).collect::<Vec<_>>(), "n = {n}");
        assert_eq!(o[0], 0);
    }
    let o = load_order(500);
    // The first pass spans the volume evenly (every 32nd slice and the last).
    assert_eq!(&o[..4], [0, 32, 64, 96]);
    assert_eq!(o[16], 499);
    assert!(o[..17].iter().all(|k| k % 32 == 0 || *k == 499));
}

#[test]
fn phantom_files_load_into_the_same_volume() {
    let size = [24, 20, 7];
    let files = phantom::series(size, 12.0, 25.0, 0.0);
    let found: Vec<(usize, Instance)> = files
        .iter()
        .enumerate()
        .map(|(i, (_, d))| (i, Instance::read(d, true).unwrap().unwrap()))
        .collect();
    let s = series_of_found(found);
    let st = stack(&s).unwrap();
    assert_eq!(st.slices.len(), 7);
    let first = Dicom::parse(&files[st.slices[0].key].1).unwrap();
    let mf = first
        .modality_frame(&files[st.slices[0].key].1, 0, None)
        .unwrap();
    assert_eq!(VoxelKind::of(&mf), VoxelKind::I16);
    let sp = mf.calibration().spacing.unwrap();
    let pick: Vec<usize> = (0..7).collect();
    let g = st.geometry(&pick, mf.frame.width, mf.frame.height, [sp.x, sp.y]);
    let mut vol = Volume::new(g, VoxelKind::of(&mf), 1).unwrap();
    for k in load_order(7) {
        let d = &files[st.slices[k].key].1;
        let f = Dicom::parse(d).unwrap().modality_frame(d, 0, None).unwrap();
        vol.put_frame(k, &f).unwrap();
    }
    let reference = phantom::volume(size, 12.0, 25.0, 0.0);
    assert_eq!(vol.geometry, reference.geometry);
    assert_eq!(vol.voxels, reference.voxels);
}

fn series_of_found(found: Vec<(usize, Instance)>) -> Series<usize> {
    let mut s = assemble(found);
    assert_eq!(s.len(), 1);
    s.remove(0)
}
