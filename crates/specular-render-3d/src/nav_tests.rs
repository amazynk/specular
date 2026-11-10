//! Walking, flying and section caps on synthetic scenes (metres).

use specular_mesh::{Bounds, Limits, Mesh, Unit, from_triangles};

use crate::bvh::NavScene;
use crate::camera::{Camera, Projection};
use crate::nav::{Body, Nav, NavInput, NavMode};
use crate::section::{SectionPlane, Sections, caps, snap_to_cut};

/// Quads (four corners, counter-clockwise from outside) as a mesh in metres.
fn scene(quads: &[[[f32; 3]; 4]]) -> Mesh {
    let mut pos = Vec::new();
    let mut idx = Vec::new();
    for q in quads {
        let b = pos.len() as u32;
        pos.extend_from_slice(q);
        idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    let mut m = from_triangles(pos, idx, None, &Limits::default()).expect("a mesh");
    m.unit = Unit::Metre;
    m
}

/// A closed axis-aligned box from `lo` to `hi`, outward faces.
fn cuboid(lo: [f32; 3], hi: [f32; 3]) -> Vec<[[f32; 3]; 4]> {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    vec![
        [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]], // bottom (−z)
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], // top
        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], // −y
        [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]], // +y
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]], // −x
        [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], // +x
    ]
}

fn floor(z: f32) -> [[f32; 3]; 4] {
    [
        [-20.0, -20.0, z],
        [20.0, -20.0, z],
        [20.0, 20.0, z],
        [-20.0, 20.0, z],
    ]
}

fn body() -> Body {
    Body {
        eye: 1.6,
        radius: 0.25,
        step: 0.4,
        per_metre: 1.0,
        fly: 5.0,
    }
}

/// A walker at feet `p` facing +x.
fn walker(p: [f64; 3]) -> Nav {
    let b = Bounds {
        min: [-20.0, -20.0, -1.0],
        max: [20.0, 20.0, 5.0],
    };
    let cam = Camera::first_person([0.0; 3], 0.0, 0.0, 1.0);
    let mut n = Nav::start(NavMode::Walk, &cam, &b, 100.0, 100.0, body());
    n.pos = p;
    n.yaw = 0.0;
    n.pitch = 0.0;
    n.grounded = false;
    n.vz = 0.0;
    n
}

fn walk(n: &mut Nav, s: &NavScene, input: NavInput, seconds: f64) {
    let dt = 1.0 / 60.0;
    for _ in 0..(seconds / dt) as usize {
        n.step(Some(s), &input, dt);
    }
}

const FORWARD: NavInput = NavInput {
    forward: 1.0,
    strafe: 0.0,
    rise: 0.0,
    turn: 0.0,
    tilt: 0.0,
    look: (0.0, 0.0),
    fast: false,
};

#[test]
fn gravity_lands_on_the_floor_at_eye_height() {
    let s = NavScene::from_mesh(&scene(&[floor(0.0)]));
    let mut n = walker([0.0, 0.0, 3.0]);
    walk(&mut n, &s, NavInput::default(), 0.3);
    assert!(n.pos[2] > 0.5, "falls, not jumps: {}", n.pos[2]);
    walk(&mut n, &s, NavInput::default(), 2.0);
    assert!(n.grounded);
    assert!(n.pos[2].abs() < 1e-9, "on the floor: {}", n.pos[2]);
    assert!((n.eye()[2] - 1.6).abs() < 1e-9);
    // Fall time from 3 m: √(2h/g) ≈ 0.78 s.
    let mut m = walker([0.0, 0.0, 3.0]);
    let mut t = 0.0;
    while !m.grounded && t < 3.0 {
        m.step(Some(&s), &NavInput::default(), 1.0 / 240.0);
        t += 1.0 / 240.0;
    }
    assert!((t - (6.0f64 / 9.81).sqrt()).abs() < 0.03, "fell in {t} s");
}

#[test]
fn steps_up_to_the_limit_and_no_higher() {
    let low = scene(
        &[floor(0.0)]
            .into_iter()
            .chain(cuboid([2.0, -5.0, 0.0], [8.0, 5.0, 0.3]))
            .collect::<Vec<_>>(),
    );
    let s = NavScene::from_mesh(&low);
    let mut n = walker([0.0, 0.0, 0.0]);
    n.settle(Some(&s));
    walk(&mut n, &s, FORWARD, 3.0);
    assert!(n.pos[0] > 3.0, "walked onto the step: {:?}", n.pos);
    assert!((n.pos[2] - 0.3).abs() < 1e-6, "on top of it: {:?}", n.pos);
    // Down again off the far side (a drop within the step height).
    walk(&mut n, &s, FORWARD, 3.0);
    assert!(n.pos[0] > 8.0 && n.pos[2].abs() < 1e-6, "{:?}", n.pos);

    let high = scene(
        &[floor(0.0)]
            .into_iter()
            .chain(cuboid([2.0, -5.0, 0.0], [8.0, 5.0, 0.6]))
            .collect::<Vec<_>>(),
    );
    let s = NavScene::from_mesh(&high);
    let mut n = walker([0.0, 0.0, 0.0]);
    n.settle(Some(&s));
    walk(&mut n, &s, FORWARD, 3.0);
    assert!(n.pos[0] < 2.0 - 0.2, "stopped at the ledge: {:?}", n.pos);
    assert!(n.pos[2].abs() < 1e-6);
}

#[test]
fn walls_stop_the_capsule_even_fast() {
    // A thin wall (one quad, both faces) across the way.
    let wall = [
        [3.0, -10.0, 0.0],
        [3.0, 10.0, 0.0],
        [3.0, 10.0, 3.0],
        [3.0, -10.0, 3.0],
    ];
    let s = NavScene::from_mesh(&scene(&[floor(0.0), wall]));
    for fast in [false, true] {
        let mut n = walker([0.0, 0.0, 0.0]);
        n.settle(Some(&s));
        let input = NavInput { fast, ..FORWARD };
        // Coarse frames (10 fps) move further than the radius each.
        for _ in 0..60 {
            n.step(Some(&s), &input, 0.1);
        }
        assert!(n.pos[0] < 3.0, "never through the wall: {:?}", n.pos);
        assert!(
            (n.pos[0] - (3.0 - 0.25)).abs() < 0.02,
            "against it: {:?}",
            n.pos
        );
    }
    // Sliding along it: walking diagonally keeps the along-wall motion.
    let mut n = walker([2.6, 0.0, 0.0]);
    n.settle(Some(&s));
    n.yaw = std::f64::consts::FRAC_PI_4;
    walk(&mut n, &s, FORWARD, 1.0);
    assert!(n.pos[1] > 0.8, "slid along the wall: {:?}", n.pos);
    assert!(n.pos[0] < 3.0);
}

#[test]
fn walks_off_a_ledge_and_falls() {
    let s = NavScene::from_mesh(&scene(
        &[floor(0.0)]
            .into_iter()
            .chain(cuboid([-10.0, -5.0, 0.0], [2.0, 5.0, 3.0]))
            .collect::<Vec<_>>(),
    ));
    let mut n = walker([0.0, 0.0, 3.0]);
    n.settle(Some(&s));
    assert!((n.pos[2] - 3.0).abs() < 1e-6);
    walk(&mut n, &s, FORWARD, 3.0);
    assert!(n.pos[0] > 2.0);
    assert!(
        n.grounded && n.pos[2].abs() < 1e-6,
        "landed below: {:?}",
        n.pos
    );
}

#[test]
fn flying_goes_anywhere_at_model_speed() {
    let wall = [
        [3.0, -10.0, 0.0],
        [3.0, 10.0, 0.0],
        [3.0, 10.0, 3.0],
        [3.0, -10.0, 3.0],
    ];
    let s = NavScene::from_mesh(&scene(&[floor(0.0), wall]));
    let cam = Camera::first_person([0.0, 0.0, 1.0], 0.0, 0.0, 1.0);
    let b = Bounds {
        min: [-20.0, -20.0, 0.0],
        max: [20.0, 20.0, 3.0],
    };
    let mut n = Nav::start(NavMode::Fly, &cam, &b, 100.0, 100.0, body());
    n.yaw = 0.0;
    n.pitch = 0.0;
    let start = n.pos;
    walk(&mut n, &s, FORWARD, 1.0);
    assert!((n.pos[0] - start[0] - 5.0).abs() < 0.1, "{:?}", n.pos);
    let rise = NavInput {
        rise: 1.0,
        fast: true,
        ..NavInput::default()
    };
    walk(&mut n, &s, rise, 1.0);
    assert!(
        (n.pos[2] - start[2] - 15.0).abs() < 0.3,
        "Shift is 3×: {:?}",
        n.pos
    );
    // Model-sized: a body for a model in millimetres is 1000× bigger.
    let mm = Body::for_model(&b, Some(0.001));
    assert!((mm.eye - 1600.0).abs() < 1e-9 && (mm.step - 400.0).abs() < 1e-9);
}

#[test]
fn first_person_cameras_round_trip_through_poses() {
    let b = Bounds {
        min: [-1.0; 3],
        max: [1.0; 3],
    };
    for proj in [Projection::Perspective, Projection::Orthographic] {
        let mut c = Camera {
            projection: proj,
            ..Camera::default()
        };
        c.fit(&b, 400.0, 300.0);
        c.orbit(0.4, -0.2);
        let p = c.pose(&b, 400.0, 300.0);
        let back = Camera::from_pose(&p, &b, 400.0, 300.0);
        let q = back.pose(&b, 400.0, 300.0);
        for k in 0..3 {
            assert!((p.eye[k] - q.eye[k]).abs() < 1e-9, "{proj:?} {p:?} {q:?}");
            assert!((p.direction[k] - q.direction[k]).abs() < 1e-9);
            assert!((p.up[k] - q.up[k]).abs() < 1e-9);
        }
        assert!((p.height - q.height).abs() < 1e-9);
    }
    // Inside the box, looking away from its centre: first person.
    let c = Camera::first_person([0.5, 0.0, 0.0], 0.0, 0.1, 0.9);
    let p = c.pose(&b, 400.0, 300.0);
    let back = Camera::from_pose(&p, &b, 400.0, 300.0);
    assert!(back.eye.is_some());
    let (y, t) = back.yaw_pitch();
    assert!(y.abs() < 1e-9 && (t - 0.1).abs() < 1e-9);
}

/// Every cap outline closes and the fill has the section's area.
#[test]
fn caps_are_watertight() {
    let cube = scene(&cuboid([0.0, 0.0, 0.0], [2.0, 3.0, 4.0]));
    let nav = NavScene::from_mesh(&cube);
    for (plane, area) in [
        (SectionPlane::through([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]), 6.0),
        (
            SectionPlane::through([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
            12.0,
        ),
        // A diagonal: the rectangle 2 × √(3² + 4²)... through two opposite edges.
        (
            SectionPlane::through([0.0, 0.0, 0.0], [0.0, 4.0, -3.0]),
            10.0,
        ),
    ] {
        let s = Sections {
            planes: vec![plane],
            section_box: None,
        };
        for scene in [None, Some(&nav)] {
            let c = caps(&cube, scene, &s);
            assert!(!c.loops.is_empty());
            assert!(c.loops.iter().all(|l| l.closed), "{plane:?}");
            assert!((c.area[0] - area).abs() < 1e-4, "{plane:?}: {}", c.area[0]);
            assert!(c.mesh.is_some());
        }
    }
    // A closed, curved surface at many heights.
    let knot = specular_mesh::fixtures::torus_knot(120, 16);
    let kn = NavScene::from_mesh(&knot);
    let b = knot.bounds;
    for i in 1..10 {
        let z = b.min[2] + (b.max[2] - b.min[2]) * i as f32 / 10.0;
        let s = Sections {
            planes: vec![SectionPlane::through(
                [0.0, 0.0, f64::from(z)],
                [0.0, 0.0, 1.0],
            )],
            section_box: None,
        };
        let c = caps(&knot, Some(&kn), &s);
        assert!(c.loops.iter().all(|l| l.closed), "z = {z}");
        assert!(c.area[0] > 0.0);
        // Twice-counted: the hatched fill has as many corners as triangles need.
        let m = c.mesh.expect("a fill");
        assert_eq!(m.indices.len() % 3, 0);
    }
}

#[test]
fn a_section_box_caps_each_face_inside_the_others() {
    let cube = scene(&cuboid([0.0, 0.0, 0.0], [4.0, 4.0, 4.0]));
    let s = Sections {
        planes: Vec::new(),
        section_box: Some(Bounds {
            min: [1.0, 1.0, 1.0],
            max: [3.0, 3.0, 2.0],
        }),
    };
    let c = caps(&cube, None, &s);
    assert_eq!(c.area.len(), 6);
    // Each face of the box crosses the cube in a 4 × 4 square, clipped by the others to
    // the box's face.
    let mut faces: Vec<f64> = c.area.clone();
    faces.sort_by(f64::total_cmp);
    for (got, want) in faces.iter().zip([2.0, 2.0, 2.0, 2.0, 4.0, 4.0]) {
        assert!((got - want).abs() < 1e-6, "{faces:?}");
    }
    assert!(s.keeps([2.0, 2.0, 1.5], 0.0));
    assert!(!s.keeps([2.0, 2.0, 2.5], 0.0));
}

#[test]
fn measures_snap_to_the_cut() {
    let cube = scene(&cuboid([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]));
    let s = Sections {
        planes: vec![SectionPlane::through([0.0, 0.0, 1.0], [0.0, 0.0, 1.0])],
        section_box: None,
    };
    let c = caps(&cube, None, &s);
    let a = snap_to_cut(&c, [0.05, 1.0, 1.0], 0.1).expect("near the outline");
    let b = snap_to_cut(&c, [1.97, 1.0, 1.0], 0.1).expect("near the outline");
    assert!(a[0].abs() < 1e-9 && (b[0] - 2.0).abs() < 1e-9);
    let width = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    assert!((width - 2.0).abs() < 1e-9);
    assert!(
        snap_to_cut(&c, [1.0, 1.0, 1.0], 0.1).is_none(),
        "the middle is no edge"
    );
}

/// A million triangles: the hierarchy is built once; a walking step and a cut stay well
/// inside a 60 fps frame (timings printed; the bound is loose for loaded machines).
#[test]
fn big_models_walk_and_cut_within_a_frame() {
    let mut knot = specular_mesh::fixtures::torus_knot(2000, 250);
    knot.unit = Unit::Metre;
    let t0 = std::time::Instant::now();
    let s = NavScene::from_mesh(&knot);
    let build = t0.elapsed();
    let b = knot.bounds;
    let mut n = walker([f64::from(b.min[0]), 0.0, f64::from(b.max[2])]);
    n.body = Body::for_model(&b, None);
    let t1 = std::time::Instant::now();
    let steps = 600u32;
    for _ in 0..steps {
        n.step(Some(&s), &FORWARD, 1.0 / 60.0);
    }
    let step = t1.elapsed() / steps;
    let sections = Sections {
        planes: vec![SectionPlane::through(
            b.centre().map(f64::from),
            [0.3, 0.2, 1.0],
        )],
        section_box: None,
    };
    let t2 = std::time::Instant::now();
    let c = caps(&knot, Some(&s), &sections);
    let cut = t2.elapsed();
    eprintln!(
        "{} triangles: BVH {:.0} ms, walk step {:.3} ms, cap {:.1} ms ({} outlines)",
        s.len(),
        build.as_secs_f64() * 1e3,
        step.as_secs_f64() * 1e3,
        cut.as_secs_f64() * 1e3,
        c.loops.len()
    );
    assert!(step.as_secs_f64() < 0.004, "a step took {step:?}");
    assert!(c.loops.iter().all(|l| l.closed));
}

/// A scene of triangles and points: rays and pushes meet both.
#[test]
fn mixed_scene_meets_triangles_and_points() {
    let floor = specular_mesh::Mesh {
        positions: vec![
            [-5.0, -5.0, 0.0],
            [5.0, -5.0, 0.0],
            [5.0, 5.0, 0.0],
            [-5.0, 5.0, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        up: specular_mesh::Up::Z,
        ..specular_mesh::Mesh::default()
    };
    // A post of points at x = 2 standing on the floor.
    let post: Vec<[f32; 3]> = (0..20).map(|i| [2.0, 0.0, i as f32 * 0.1]).collect();
    let s = crate::bvh::NavScene::from_mesh_and_points(&floor, &post, 0.05);
    assert_eq!(s.len(), 2 + 20);
    // Down onto the floor.
    let d = s.ray([0.0, 0.0, 1.0], [0.0, 0.0, -1.0], 10.0).unwrap();
    assert!((d - 1.0).abs() < 1e-6, "{d}");
    // Along x into the post.
    let d = s.ray([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 10.0).unwrap();
    assert!((d - 1.95).abs() < 1e-3, "{d}");
    // A capsule touching the post is pushed back along -x.
    let p = s
        .push_out([1.8, 0.0, 0.5], [1.8, 0.0, 1.5], 0.3)
        .expect("touches the post");
    assert!(p[0] < 0.0 && p[2].abs() < 1e-9, "{p:?}");
    // Points alone and triangles alone still work.
    assert!(
        crate::bvh::NavScene::from_points(&post, 0.05)
            .ray([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 10.0)
            .is_some()
    );
    assert!(
        crate::bvh::NavScene::from_mesh(&floor)
            .ray([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], 10.0)
            .is_none()
    );
}
