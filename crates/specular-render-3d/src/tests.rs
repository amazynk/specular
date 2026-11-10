//! The shaders, the camera and, on lavapipe (skipped without it), the drawing.

use specular_mesh::preview::Texture;
use specular_mesh::{Bounds, Material, Mesh, Part, Up, fixtures};
use specular_shots::{Goldens, Harness, Shot};

use super::*;
use crate::camera::{corners, dot};

#[test]
fn shaders_validate() {
    for (name, src) in [
        ("mesh", mesh_source()),
        ("present", present_source()),
        ("points", points_source()),
        ("edl", edl_source()),
        ("molecule", molecule_source()),
        ("oit", oit_source()),
        ("splats", splats_source()),
        ("splat present", splat_present_source()),
        ("depth fill", crate::points::depth_fill_source(1)),
        ("depth fill msaa", crate::points::depth_fill_source(4)),
    ] {
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}

fn close(a: V3, b: V3) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-9)
}

#[test]
fn orbits_keep_an_orthonormal_frame() {
    let mut c = Camera::default();
    for i in 0..200 {
        c.orbit(0.37 * f64::from(i % 7) - 1.0, 0.11 * f64::from(i % 5) - 0.2);
        assert!((dot(c.right, c.right) - 1.0).abs() < 1e-9);
        assert!((dot(c.down, c.down) - 1.0).abs() < 1e-9);
        assert!(dot(c.right, c.down).abs() < 1e-9);
    }
    // A turn about the up axis keeps the right vector level.
    let mut c = Camera::default();
    c.orbit(0.5, 0.0);
    assert!(c.right[2].abs() < 1e-12);
}

#[test]
fn standard_views_look_where_they_say() {
    let b = Bounds {
        min: [0.0; 3],
        max: [1.0; 3],
    };
    let mut c = Camera::default();
    c.look(StandardView::Front, &b, 100.0, 100.0);
    assert!(close(c.forward(), [0.0, 1.0, 0.0]));
    c.look(StandardView::Top, &b, 100.0, 100.0);
    assert!(close(c.forward(), [0.0, 0.0, -1.0]));
    c.look(StandardView::Side, &b, 100.0, 100.0);
    assert!(close(c.forward(), [-1.0, 0.0, 0.0]));
    c.look(StandardView::Iso, &b, 100.0, 100.0);
    let s = 1.0 / 3f64.sqrt();
    let f = c.forward();
    assert!(close(
        f.map(|v| (v * 1e6).round()),
        [-s, s, -s].map(|v| (v * 1e6).round())
    ));
}

/// Fitting brings every corner of the box into the view, filling most of it.
#[test]
fn fit_frames_the_box() {
    let b = Bounds {
        min: [-3.0, 10.0, 0.0],
        max: [5.0, 12.0, 1.0],
    };
    let (w, h) = (320.0, 200.0);
    for projection in [Projection::Orthographic, Projection::Perspective] {
        for v in StandardView::ALL {
            let mut c = Camera {
                projection,
                ..Camera::default()
            };
            c.look(v, &b, w, h);
            let p = c.project(&b, w, h, 0.0);
            let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for k in corners(&b) {
                let (x, y, z) = p.to_px(k, w, h).expect("in front");
                assert!((0.0..=1.0).contains(&z), "{v:?} {projection:?} depth {z}");
                (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
            }
            assert!(
                x0 >= 0.0 && y0 >= 0.0 && x1 <= w && y1 <= h,
                "{v:?} {projection:?}"
            );
            let fill = ((x1 - x0) / w).max((y1 - y0) / h);
            assert!(fill > 0.6, "{v:?} {projection:?} fills {fill}");
        }
    }
}

#[test]
fn nearer_is_deeper_and_pan_follows_the_pointer() {
    let b = Bounds {
        min: [-1.0; 3],
        max: [1.0; 3],
    };
    for projection in [Projection::Orthographic, Projection::Perspective] {
        let mut c = Camera {
            projection,
            ..Camera::default()
        };
        c.look(StandardView::Front, &b, 100.0, 100.0);
        let p = c.project(&b, 100.0, 100.0, 0.0);
        let near = p.to_px([0.0, -1.0, 0.0], 100.0, 100.0).unwrap();
        let far = p.to_px([0.0, 1.0, 0.0], 100.0, 100.0).unwrap();
        assert!(near.2 > far.2, "reversed depth");
        let up = p.to_px([0.0, 0.0, 1.0], 100.0, 100.0).unwrap();
        assert!(up.1 < 50.0, "up is up");
        let right = p.to_px([1.0, 0.0, 0.0], 100.0, 100.0).unwrap();
        assert!(right.0 > 50.0, "x is right from the front");
        // Panning by 10 px moves the centre 10 px.
        let before = p.to_px([0.0; 3], 100.0, 100.0).unwrap();
        c.pan_px(10.0, -4.0, p.units_per_px);
        let after = c
            .project(&b, 100.0, 100.0, 0.0)
            .to_px([0.0; 3], 100.0, 100.0)
            .unwrap();
        assert!((after.0 - before.0 - 10.0).abs() < 1e-6);
        assert!((after.1 - before.1 + 4.0).abs() < 1e-6);
    }
}

#[test]
fn grid_steps_are_one_two_five() {
    let at = |r: f32| {
        grid_step(&Bounds {
            min: [0.0; 3],
            max: [r, 0.0, 0.0],
        })
    };
    // The diameter is the box's diagonal; a twentieth of it, rounded up.
    assert_eq!(at(20.0), 1.0);
    assert_eq!(at(30.0), 2.0);
    assert_eq!(at(70.0), 5.0);
    assert_eq!(at(150.0), 10.0);
    assert!((at(0.03) - 0.002).abs() < 1e-12);
}

#[test]
fn octahedral_normals_round_trip() {
    let decode = |e: u32| {
        let s = |v: u16| f32::from(v as i16).max(-32767.0) / 32767.0;
        let (x, y) = (s(e as u16), s((e >> 16) as u16));
        let mut n = [x, y, 1.0 - x.abs() - y.abs()];
        let t = (-n[2]).max(0.0);
        n[0] += if n[0] >= 0.0 { -t } else { t };
        n[1] += if n[1] >= 0.0 { -t } else { t };
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n.map(|c| c / l)
    };
    let mut worst = 0f32;
    for i in 0..2000 {
        let a = i as f32 * 0.61;
        let z = (i as f32 / 1000.0) - 1.0;
        let r = (1.0 - z * z).max(0.0).sqrt();
        let n = [r * a.cos(), r * a.sin(), z];
        let d = decode(upload::oct(n));
        let dot = n[0] * d[0] + n[1] * d[1] + n[2] * d[2];
        worst = worst.max(dot.clamp(-1.0, 1.0).acos());
    }
    assert!(worst < 1e-3, "worst {worst} rad");
}

const SIZE: (u32, u32) = (160, 120);

fn knot() -> Mesh {
    fixtures::torus_knot(160, 24)
}

fn shot(h: &mut Harness, mesh: &Mesh, textures: &[Option<Texture>], v: View) -> Shot {
    shot_with(h, mesh, textures, v, CHUNK, true)
}

fn shot_with(
    h: &mut Harness,
    mesh: &Mesh,
    textures: &[Option<Texture>],
    v: View,
    chunk: usize,
    share: bool,
) -> Shot {
    let mut r = MeshRenderer::new(&h.gpu);
    let gpu_mesh = r.upload_with(&h.gpu, mesh, textures, chunk, share, false);
    h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, target, size| r.draw(gpu, enc, target, size, &gpu_mesh, &v),
        |_, _| {},
    )
}

fn view(mesh: &Mesh, shading: Shading, projection: Projection) -> View {
    let b = canonical_bounds(mesh);
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.look(StandardView::Iso, &b, f64::from(SIZE.0), f64::from(SIZE.1));
    View {
        rect: [0, 0, SIZE.0, SIZE.1],
        camera,
        shading,
        grid: true,
        scale: 1.0,
    }
}

fn px(s: &Shot, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * s.width + x) * 3) as usize;
    [s.rgb[i], s.rgb[i + 1], s.rgb[i + 2]]
}

/// Pixels brighter than the black background.
fn lit(s: &Shot) -> usize {
    s.rgb
        .chunks_exact(3)
        .filter(|p| p.iter().any(|c| *c > 24))
        .count()
}

#[test]
fn every_shading_draws_something_different() {
    let Some(mut h) = Harness::new() else { return };
    let m = knot();
    let mut shots = Vec::new();
    for s in Shading::ALL {
        let mut v = view(&m, s, Projection::Perspective);
        v.grid = false;
        let shot = shot(&mut h, &m, &[], v);
        let n = lit(&shot);
        eprintln!("{s:?}: {n} lit");
        assert!(n > 1500, "{s:?} draws too little: {n}");
        // The corners stay background.
        assert_eq!(px(&shot, 0, 0), [0, 0, 0], "{s:?}");
        shots.push(shot);
    }
    for i in 0..shots.len() {
        for j in i + 1..shots.len() {
            assert_ne!(shots[i].rgb, shots[j].rgb, "{i} and {j} are alike");
        }
    }
}

/// Chunks with their own (re-indexed) vertices draw what shared vertices do.
#[test]
fn chunks_draw_the_same() {
    let Some(mut h) = Harness::new() else { return };
    let m = knot();
    let v = view(&m, Shading::Shaded, Projection::Orthographic);
    let a = shot(&mut h, &m, &[], v);
    let b = shot_with(&mut h, &m, &[], v, 1000, false);
    let c = shot_with(&mut h, &m, &[], v, 777, true);
    assert_eq!(a.rgb, b.rgb);
    assert_eq!(a.rgb, c.rgb);
}

#[test]
fn the_grid_shows_under_the_model() {
    let Some(mut h) = Harness::new() else { return };
    let m = knot();
    let mut v = view(&m, Shading::Shaded, Projection::Perspective);
    v.grid = false;
    let without = shot(&mut h, &m, &[], v);
    v.grid = true;
    let with = shot(&mut h, &m, &[], v);
    assert!(lit(&with) > lit(&without));
}

/// A quad with a 2 × 2 texture (red, green over blue, white) seen from the front.
#[test]
fn textures_and_y_up() {
    let Some(mut h) = Harness::new() else { return };
    let m = Mesh {
        positions: vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: Some(vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]),
        indices: vec![0, 1, 2, 0, 2, 3],
        parts: vec![Part {
            first: 0,
            count: 2,
            material: Some(0),
        }],
        materials: vec![Material {
            name: "checker".into(),
            base_color: [1.0; 4],
            texture: Some(0),
        }],
        up: Up::Y,
        bounds: Bounds {
            min: [-1.0, -1.0, 0.0],
            max: [1.0, 1.0, 0.0],
        },
        ..Mesh::default()
    };
    let tex = Texture {
        width: 2,
        height: 2,
        rgba: vec![
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 255, 255, 255, 255, 255,
        ],
    };
    let b = canonical_bounds(&m);
    let mut camera = Camera {
        projection: Projection::Orthographic,
        ..Camera::default()
    };
    camera.look(
        StandardView::Front,
        &b,
        f64::from(SIZE.0),
        f64::from(SIZE.1),
    );
    let v = View {
        rect: [0, 0, SIZE.0, SIZE.1],
        camera,
        shading: Shading::Shaded,
        grid: false,
        scale: 1.0,
    };
    let s = shot(&mut h, &m, &[Some(tex)], v);
    let (cx, cy) = (SIZE.0 / 2, SIZE.1 / 2);
    let tl = px(&s, cx - 20, cy - 20);
    let tr = px(&s, cx + 20, cy - 20);
    let bl = px(&s, cx - 20, cy + 20);
    let br = px(&s, cx + 20, cy + 20);
    eprintln!("{tl:?} {tr:?} {bl:?} {br:?}");
    // Blurred across the quad by linear filtering, but each corner leans its own way.
    assert!(tl[0] > tl[1] && tl[0] > tl[2], "red at the top left");
    assert!(tr[1] > tr[0] && tr[1] > tr[2], "green at the top right");
    assert!(bl[2] > bl[0] && bl[2] > bl[1], "blue at the bottom left");
    assert!(br.iter().all(|c| *c > 100), "white at the bottom right");
}

/// Draws into its rectangle only, over what was there.
#[test]
fn stays_in_its_rectangle() {
    let Some(mut h) = Harness::new() else { return };
    let m = knot();
    let mut v = view(&m, Shading::Shaded, Projection::Perspective);
    v.rect = [40, 30, 80, 60];
    v.camera.fit(&canonical_bounds(&m), 80.0, 60.0);
    let s = shot(&mut h, &m, &[], v);
    for y in 0..SIZE.1 {
        for x in 0..SIZE.0 {
            let inside = (40..120).contains(&x) && (30..90).contains(&y);
            if !inside {
                assert_eq!(px(&s, x, y), [0, 0, 0], "{x},{y}");
            }
        }
    }
    assert!(lit(&s) > 400);
}

/// The renderer's views of a generated mesh, as goldens (`SPECULAR_BLESS=1` to update).
#[test]
fn goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut g = Goldens::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"), "mesh");
    let m = knot();
    for s in Shading::ALL {
        let v = view(&m, s, Projection::Perspective);
        g.check(&format!("knot-{}", s.id()), &shot(&mut h, &m, &[], v));
    }
    let mut v = view(&m, Shading::Shaded, Projection::Orthographic);
    v.camera
        .look(StandardView::Top, &canonical_bounds(&m), 160.0, 120.0);
    g.check("knot-top-ortho", &shot(&mut h, &m, &[], v));
    g.finish();
}

/// Upload and frame time of a 10M-triangle model on the machine's own GPU at 1920×1080
/// (run with `--ignored --nocapture`; needs a GPU, uses about 1 GB of host memory).
#[test]
#[ignore = "timing; needs a hardware GPU"]
fn large_mesh_gpu_timing() {
    use std::time::Instant;
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("no GPU: {e}");
            return;
        }
    };
    eprintln!("adapter: {} ({:?})", gpu.info.name, gpu.info.device_type);
    let wait = |gpu: &Gpu| {
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    };
    let t = Instant::now();
    let mesh = fixtures::torus_knot(2500, 2000);
    eprintln!(
        "generated {} triangles, {} vertices in {:?}",
        mesh.triangle_count(),
        mesh.vertex_count(),
        t.elapsed()
    );
    let mut r = MeshRenderer::new(&gpu);
    let t = Instant::now();
    let gm = r.upload(&gpu, &mesh, &[]);
    wait(&gpu);
    eprintln!(
        "upload: {:?} ({} MB on the GPU)",
        t.elapsed(),
        gm.bytes >> 20
    );
    let size = (1920u32, 1080u32);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timing target"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let b = canonical_bounds(&mesh);
    for shading in [
        Shading::Shaded,
        Shading::Edges,
        Shading::Wireframe,
        Shading::XRay,
    ] {
        let mut camera = Camera::default();
        camera.look(StandardView::Iso, &b, f64::from(size.0), f64::from(size.1));
        let mut v = View {
            rect: [0, 0, size.0, size.1],
            camera,
            shading,
            grid: true,
            scale: 1.0,
        };
        let mut frame = |v: &View| {
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            r.draw(&gpu, &mut enc, &tv, size, &gm, v);
            gpu.queue.submit([enc.finish()]);
            wait(&gpu);
        };
        // Warm up (pipelines, depth target), then time orbiting frames.
        frame(&v);
        let n = 20;
        let t = Instant::now();
        for _ in 0..n {
            v.camera.orbit(ORBIT_KEY, 0.0);
            frame(&v);
        }
        eprintln!(
            "{shading:?}: {:.1} ms a frame",
            t.elapsed().as_secs_f64() * 1e3 / f64::from(n)
        );
    }
}

// Point clouds.

/// The generated lidar scene (60 m square), read through the progressive path.
fn terrain(density: f64) -> std::sync::Arc<specular_points::Loading> {
    let bytes = specular_points::fixtures::terrain_las(60.0, density);
    let (loading, job) = specular_points::open(
        specular_points::PointFormat::Las,
        &bytes,
        &specular_points::Limits::default(),
    )
    .unwrap();
    if let Some(job) = job {
        job.run(&bytes, &loading);
    }
    loading
}

/// Uploads all of `loading`, a frame's worth at a time.
fn upload_all(r: &PointRenderer, gpu: &Gpu, loading: &specular_points::Loading) -> PointsGpu {
    let mut g = PointsGpu::new(loading);
    let mut frames = 0;
    while !g.complete() {
        g.feed(r, gpu, loading);
        frames += 1;
        assert!(frames < 1000, "upload never finished");
    }
    g
}

fn point_view(b: &Bounds, colour: PointColour, edl: bool, projection: Projection) -> PointView {
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.look(StandardView::Iso, b, f64::from(SIZE.0), f64::from(SIZE.1));
    PointView {
        rect: [0, 0, SIZE.0, SIZE.1],
        camera,
        grid: true,
        scale: 1.0,
        colour,
        size: 2.0,
        world: false,
        edl,
        budget: 10_000_000,
    }
}

fn point_shot(h: &mut Harness, loading: &specular_points::Loading, v: &PointView) -> (Shot, u64) {
    let mut r = PointRenderer::new(&h.gpu);
    let mut g = upload_all(&r, &h.gpu, loading);
    let shot = h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, target, size| r.draw(gpu, enc, target, size, &mut g, v),
        |_, _| {},
    );
    (shot, g.drawn)
}

fn brightness(s: &Shot) -> u64 {
    s.rgb.iter().map(|&c| u64::from(c)).sum()
}

#[test]
fn colourings_follow_the_channels() {
    let ch = specular_points::Channels {
        rgb: false,
        intensity: true,
        classification: false,
        returns: false,
    };
    assert_eq!(PointColour::default_for(&ch), PointColour::Elevation);
    let seen: Vec<PointColour> =
        std::iter::successors(Some(PointColour::Elevation), |c| Some(c.next_for(&ch)))
            .take(4)
            .collect();
    assert!(seen.iter().all(|c| c.available(&ch)), "{seen:?}");
    assert!(seen.contains(&PointColour::Intensity));
    assert!(!seen.contains(&PointColour::Rgb));
}

#[test]
fn points_draw_under_the_budget() {
    let Some(mut h) = Harness::new() else { return };
    let loading = terrain(20.0);
    let b = loading.wait().unwrap().bounds;
    let v = point_view(&b, PointColour::Elevation, false, Projection::Perspective);
    let (full, n) = point_shot(&mut h, &loading, &v);
    // What fills a 160 × 120 view with 2 px splats: far fewer than the 72000 points.
    assert!((5_000..30_000).contains(&n), "drew {n}");
    let v = PointView { budget: 2_000, ..v };
    let (thin, n) = point_shot(&mut h, &loading, &v);
    assert!(n <= 2_000 && n > 1_000, "drew {n}");
    assert!(lit(&thin) < lit(&full));
    assert!(lit(&thin) > 400, "{}", lit(&thin));
}

#[test]
fn colourings_and_eye_dome_lighting_differ() {
    let Some(mut h) = Harness::new() else { return };
    let loading = terrain(20.0);
    let b = loading.wait().unwrap().bounds;
    let mut shots = Vec::new();
    for colour in PointColour::ALL {
        let v = point_view(&b, colour, false, Projection::Perspective);
        shots.push(point_shot(&mut h, &loading, &v).0);
    }
    for i in 0..shots.len() {
        for j in 0..i {
            assert_ne!(shots[i].rgb, shots[j].rgb, "{i} and {j} look the same");
        }
    }
    let v = point_view(&b, PointColour::Elevation, true, Projection::Perspective);
    let (edl, _) = point_shot(&mut h, &loading, &v);
    let plain = brightness(&shots[2]);
    assert!(brightness(&edl) < plain, "EDL darkens");
    assert!(brightness(&edl) * 2 > plain, "but keeps the picture");
}

#[test]
fn points_load_progressively() {
    let Some(h) = Harness::new() else { return };
    let bytes = specular_points::fixtures::terrain_las(60.0, 40.0);
    let (loading, job) = specular_points::open(
        specular_points::PointFormat::Las,
        &bytes,
        &specular_points::Limits::default(),
    )
    .unwrap();
    let r = PointRenderer::new(&h.gpu);
    let mut g = PointsGpu::new(&loading);
    // The header's box before any point.
    assert!(!g.bounds.is_empty());
    assert!(!g.feed(&r, &h.gpu, &loading));
    assert!(g.is_empty() && !g.complete());
    job.unwrap().run(&bytes, &loading);
    let mut frames = 0;
    while !g.complete() {
        g.feed(&r, &h.gpu, &loading);
        frames += 1;
        assert!(frames < 1000);
    }
    assert_eq!(g.len(), 144_000);
}

/// The point views of the generated scene, as goldens (`SPECULAR_BLESS=1` to update).
#[test]
fn point_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut g = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "points",
    );
    let loading = terrain(20.0);
    let b = loading.wait().unwrap().bounds;
    let cases = [
        ("terrain-elevation-edl", PointColour::Elevation, true),
        ("terrain-classes", PointColour::Classification, false),
        ("terrain-rgb", PointColour::Rgb, false),
    ];
    for (name, colour, edl) in cases {
        let v = point_view(&b, colour, edl, Projection::Perspective);
        g.check(name, &point_shot(&mut h, &loading, &v).0);
    }
    let mut v = point_view(&b, PointColour::Intensity, true, Projection::Orthographic);
    v.camera
        .look(StandardView::Top, &b, f64::from(SIZE.0), f64::from(SIZE.1));
    v.world = true;
    v.size = 0.6;
    g.check(
        "terrain-intensity-top-ortho",
        &point_shot(&mut h, &loading, &v).0,
    );
    g.finish();
}

/// Loading and orbiting frame time of a 10M-point cloud at a 10M budget on the machine's
/// own GPU at 1920×1080 (run with `--ignored --nocapture`; needs a GPU and about 1 GB).
#[test]
#[ignore = "timing; needs a hardware GPU"]
#[allow(clippy::disallowed_methods)]
fn large_cloud_gpu_timing() {
    use std::time::Instant;
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("no GPU: {e}");
            return;
        }
    };
    eprintln!("adapter: {} ({:?})", gpu.info.name, gpu.info.device_type);
    let wait = |gpu: &Gpu| {
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    };
    let t = Instant::now();
    let bytes = specular_points::fixtures::terrain_las(1000.0, 10.0);
    eprintln!(
        "generated {} MB of LAS in {:?}",
        bytes.len() >> 20,
        t.elapsed()
    );
    let size = (1920u32, 1080u32);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timing target"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let mut r = PointRenderer::new(&gpu);
    let t = Instant::now();
    let (loading, job) = specular_points::open(
        specular_points::PointFormat::Las,
        &bytes,
        &specular_points::Limits::default(),
    )
    .unwrap();
    let mut g = PointsGpu::new(&loading);
    let mut camera = Camera::default();
    camera.look(
        StandardView::Iso,
        &g.bounds,
        f64::from(size.0),
        f64::from(size.1),
    );
    let mut v = PointView {
        rect: [0, 0, size.0, size.1],
        camera,
        grid: true,
        scale: 1.0,
        colour: PointColour::Elevation,
        size: 2.0,
        world: false,
        edl: true,
        budget: 10_000_000,
    };
    let frame = |r: &mut PointRenderer, v: &PointView, g: &mut PointsGpu| {
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        r.draw(&gpu, &mut enc, &tv, size, g, v);
        gpu.queue.submit([enc.finish()]);
        wait(&gpu);
    };
    // Decoding on a worker while this thread feeds and draws, as the UI does.
    let mut first_view = None;
    let mut worst = 0f64;
    let mut feeds = 0u32;
    std::thread::scope(|s| {
        if let Some(job) = job {
            let (bytes, loading) = (&bytes, &loading);
            s.spawn(move || job.run(bytes, loading));
        }
        while !g.complete() {
            let f0 = Instant::now();
            g.feed(&r, &gpu, &loading);
            worst = worst.max(f0.elapsed().as_secs_f64());
            feeds += 1;
            if first_view.is_none() && !g.is_empty() {
                frame(&mut r, &v, &mut g);
                first_view = Some(t.elapsed());
            }
        }
    });
    eprintln!(
        "loaded {} points in {:?} over {feeds} feeds (slowest {:.1} ms); first view after {:?}",
        g.len(),
        t.elapsed(),
        worst * 1e3,
        first_view.unwrap_or_default()
    );
    // 1 px splats fill the view with about four times the points of 2 px ones.
    for (edl, size) in [(false, 2.0), (true, 2.0), (true, 1.0)] {
        v.edl = edl;
        v.size = size;
        frame(&mut r, &v, &mut g);
        let n = 20;
        let t = Instant::now();
        for _ in 0..n {
            v.camera.orbit(ORBIT_KEY, 0.0);
            frame(&mut r, &v, &mut g);
        }
        eprintln!(
            "orbiting, EDL {edl}, {size} px: {:.1} ms a frame, {} points drawn",
            t.elapsed().as_secs_f64() * 1e3 / f64::from(n),
            g.drawn
        );
    }
}

// Molecules.

fn molecule(name: &str) -> specular_mol::Molecule {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../formats/specular-mol/tests/data/"
    );
    let data = std::fs::read(format!("{path}{name}")).expect("fixture");
    specular_mol::open(&data, None, &specular_mol::Limits::default()).expect(name)
}

/// The scene's view: iso, fitted on its box.
fn mol_view(s: &specular_mol::Scene, projection: Projection) -> View {
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.look(
        StandardView::Iso,
        &s.bounds,
        f64::from(SIZE.0),
        f64::from(SIZE.1),
    );
    View {
        rect: [0, 0, SIZE.0, SIZE.1],
        camera,
        shading: Shading::Shaded,
        grid: false,
        scale: 1.0,
    }
}

fn mol_shot(
    h: &mut Harness,
    s: &specular_mol::Scene,
    overlay: &[specular_mol::Ball],
    v: View,
) -> Shot {
    let mut r = MeshRenderer::new(&h.gpu);
    let mut mol = r.upload_molecule(&h.gpu, &s.balls, &s.sticks, s.bounds);
    r.set_overlay(&h.gpu, &mut mol, overlay, &[]);
    let cartoon = s.cartoon.as_ref().map(|c| r.upload(&h.gpu, c, &[]));
    h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, target, size| {
            r.draw_scene(gpu, enc, target, size, cartoon.as_ref(), Some(&mol), &v);
        },
        |_, _| {},
    )
}

/// A click on an atom's projected centre picks that atom, in both projections.
#[test]
fn clicks_pick_the_atom_under_them() {
    use specular_mol::{Colouring, Style, scene};
    let m = molecule("caffeine.xyz");
    for style in [Style::BallStick, Style::Spacefill] {
        let s = scene::build(&m, style, Colouring::Element);
        for projection in [Projection::Orthographic, Projection::Perspective] {
            let v = mol_view(&s, projection);
            let (w, h) = (f64::from(SIZE.0), f64::from(SIZE.1));
            let p = v.camera.project(&s.bounds, w, h, 0.0);
            let depth = |i: usize| {
                p.to_px(m.atoms[i].pos.map(f64::from), w, h)
                    .expect("in front")
                    .2
            };
            // The atom nearest the viewer: nothing covers its centre.
            let near = (0..m.atoms.len())
                .max_by(|&a, &b| depth(a).total_cmp(&depth(b)))
                .expect("atoms");
            let (x, y, _) = p
                .to_px(m.atoms[near].pos.map(f64::from), w, h)
                .expect("in front");
            let (o, d) = v.camera.ray(&s.bounds, x, y, w, h);
            let hit = s.pick(&m, o, d).map(|(i, _)| i);
            assert_eq!(hit, Some(near), "{style:?} {projection:?}");
            // Far outside: nothing.
            let (o, d) = v.camera.ray(&s.bounds, -500.0, -500.0, w, h);
            assert_eq!(s.pick(&m, o, d), None);
        }
    }
}

/// Every style draws, and they differ; the overlay shows over the molecule.
#[test]
fn molecule_styles_draw() {
    use specular_mol::{Colouring, Style, scene};
    let Some(mut h) = Harness::new() else { return };
    let m = molecule("1crn.pdb");
    let mut shots = Vec::new();
    for style in Style::ALL {
        let s = scene::build(&m, style, Colouring::Element);
        let shot = mol_shot(&mut h, &s, &[], mol_view(&s, Projection::Perspective));
        assert!(lit(&shot) > 400, "{style:?} lit {}", lit(&shot));
        shots.push(shot);
    }
    assert_ne!(shots[0].rgb, shots[1].rgb);
    assert_ne!(shots[1].rgb, shots[2].rgb);
    // A big yellow ball at the centre over the space-filling model.
    let s = scene::build(&m, Style::Spacefill, Colouring::Element);
    let ball = specular_mol::Ball {
        centre: s.bounds.centre(),
        radius: 9.0,
        colour: [255, 220, 0, 255],
    };
    let shot = mol_shot(&mut h, &s, &[ball], mol_view(&s, Projection::Orthographic));
    let [r, g, b] = px(&shot, SIZE.0 / 2, SIZE.1 / 2);
    assert!(r > 150 && g > 120 && b < 80, "centre {r} {g} {b}");
}

/// The molecule views, as goldens (`SPECULAR_BLESS=1` to update).
#[test]
fn molecule_goldens() {
    use specular_mol::{Colouring, Style, scene};
    let Some(mut h) = Harness::new() else { return };
    let mut g = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "molecule",
    );
    for (name, file, style, colouring) in [
        (
            "mol-cartoon-chain",
            "1bna.pdb",
            Style::Cartoon,
            Colouring::Chain,
        ),
        ("mol-cartoon-ss", "1crn.pdb", Style::Cartoon, Colouring::Ss),
        (
            "mol-ballstick-element",
            "aspirin.sdf",
            Style::BallStick,
            Colouring::Element,
        ),
        (
            "mol-spacefill",
            "1crn.pdb",
            Style::Spacefill,
            Colouring::Element,
        ),
        (
            "mol-unit-cell",
            "cod_1011176.cif",
            Style::BallStick,
            Colouring::Element,
        ),
    ] {
        let m = molecule(file);
        let s = scene::build(&m, style, colouring);
        let v = mol_view(&s, Projection::Perspective);
        g.check(name, &mol_shot(&mut h, &s, &[], v));
    }
    g.finish();
}

// Translucent sticks (streamlines below full opacity).

fn stick(a: [f32; 3], b: [f32; 3], radius: f32, colour: [u8; 4]) -> specular_mol::Stick {
    specular_mol::Stick {
        a,
        b,
        radius,
        colours: [colour; 2],
    }
}

/// Sticks alone (no balls) at `opacity`, seen from `from`.
fn sticks_shot(
    h: &mut Harness,
    sticks: &[specular_mol::Stick],
    opacity: f32,
    from: StandardView,
    projection: Projection,
) -> Shot {
    let mut b = Bounds::EMPTY;
    for s in sticks {
        b.add(s.a);
        b.add(s.b);
    }
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.look(from, &b, f64::from(SIZE.0), f64::from(SIZE.1));
    let v = View {
        rect: [0, 0, SIZE.0, SIZE.1],
        camera,
        shading: Shading::Shaded,
        grid: false,
        scale: 1.0,
    };
    let mut r = MeshRenderer::new(&h.gpu);
    let mut mol = r.upload_molecule(&h.gpu, &[], sticks, b);
    mol.opacity = opacity;
    h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, target, size| r.draw_scene(gpu, enc, target, size, None, Some(&mol), &v),
        |_, _| {},
    )
}

fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A translucent stick covers its pixels once, at its opacity (not twice: the far face of
/// its box is left out); where a nearer red stick crosses a farther blue one both show,
/// the nearer more.
#[test]
fn translucent_sticks_blend_once_and_nearer_weighs_more() {
    let Some(mut h) = Harness::new() else { return };
    let red = [255, 40, 40, 255];
    let blue = [40, 40, 255, 255];
    let pair = [
        stick([-10.0, 0.0, 2.0], [10.0, 0.0, 2.0], 1.5, red),
        stick([0.0, -10.0, -2.0], [0.0, 10.0, -2.0], 1.5, blue),
    ];
    for projection in [Projection::Orthographic, Projection::Perspective] {
        let opaque = sticks_shot(&mut h, &pair, 1.0, StandardView::Top, projection);
        let half = sticks_shot(&mut h, &pair, 0.5, StandardView::Top, projection);
        let (cx, cy) = (SIZE.0 / 2, SIZE.1 / 2);
        // The crossing: red alone when opaque.
        let [r, g, b] = px(&opaque, cx, cy);
        assert!(
            r > 150 && b < 120,
            "{projection:?} opaque crossing {r} {g} {b}"
        );
        let [r, _, b] = px(&half, cx, cy);
        assert!(
            b > 60 && r > b,
            "{projection:?} translucent crossing {r} {b}"
        );
        // On the blue stick alone (between the centre and the edge): half its light.
        let (x, y) = (0..cy)
            .map(|d| (cx, cy - d))
            .find(|&(x, y)| {
                let [r, _, b] = px(&opaque, x, y);
                b > 120 && r < 120 && {
                    let [r, _, b] = px(&opaque, x, y.saturating_sub(4));
                    b > 120 && r < 120
                }
            })
            .map(|(x, y)| (x, y.saturating_sub(6)))
            .expect("blue alone");
        let o = linear(px(&opaque, x, y)[2]);
        let t = linear(px(&half, x, y)[2]);
        assert!(
            (t / o - 0.5).abs() < 0.06,
            "{projection:?} at ({x}, {y}): {t:.3} of {o:.3}"
        );
        // Nothing where there are no sticks.
        assert_eq!(px(&half, 2, 2), px(&opaque, 2, 2));
    }
}

/// A bundle of streamline-like curves, translucent, as a golden (`SPECULAR_BLESS=1`).
#[test]
fn translucent_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut sticks = Vec::new();
    for k in 0..24 {
        let phase = k as f32 * 0.26;
        let mut prev: Option<[f32; 3]> = None;
        for i in 0..=40 {
            let t = i as f32 / 40.0;
            let p = [
                -30.0 + 60.0 * t,
                8.0 * (t * 6.0 + phase).sin() + (k as f32 - 12.0) * 0.8,
                12.0 * (t * std::f32::consts::PI).sin() + (k % 5) as f32,
            ];
            if let Some(q) = prev {
                let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let c = d.map(|x| (x.abs() / l * 255.0) as u8);
                sticks.push(stick(q, p, 0.6, [c[0], c[1], c[2], 255]));
            }
            prev = Some(p);
        }
    }
    let mut g = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "translucent",
    );
    for (name, opacity) in [("tracts-opaque", 1.0), ("tracts-translucent-35", 0.35)] {
        let shot = sticks_shot(
            &mut h,
            &sticks,
            opacity,
            StandardView::Iso,
            Projection::Perspective,
        );
        g.check(name, &shot);
    }
    g.finish();
}

// Gaussian splats.

fn splat_scene() -> std::sync::Arc<specular_splat::Splats> {
    let g = specular_splat::fixtures::scene(1200);
    let bytes = specular_splat::fixtures::ply_file(&g, 3, false);
    std::sync::Arc::new(
        specular_splat::read_ply(&bytes, &specular_splat::Limits::default()).unwrap(),
    )
}

fn splat_view(s: &specular_splat::Splats, projection: Projection) -> SplatView {
    let mut camera = Camera {
        projection,
        ..Camera::default()
    };
    camera.look(
        StandardView::Iso,
        &s.bounds,
        f64::from(SIZE.0),
        f64::from(SIZE.1),
    );
    SplatView {
        rect: [0, 0, SIZE.0, SIZE.1],
        camera,
        grid: false,
    }
}

fn splat_shot(
    h: &mut Harness,
    s: &std::sync::Arc<specular_splat::Splats>,
    v: &SplatView,
) -> (Shot, u32) {
    let mut r = SplatRenderer::new(&h.gpu);
    let mut g = SplatsGpu::new(&h.gpu, s.clone());
    let mut frames = 0;
    while !g.complete() {
        g.feed(&h.gpu);
        frames += 1;
        assert!(frames < 100);
    }
    let shot = h.render_gpu(
        (SIZE.0 as f32, SIZE.1 as f32),
        1.0,
        |gpu, enc, target, size| r.draw(gpu, enc, target, size, &mut g, v),
        |_, _| {},
    );
    (shot, g.drawn)
}

#[test]
fn splats_sort_back_to_front() {
    let p: Vec<[f32; 3]> = (0..1000)
        .map(|i| {
            let t = i as f32 * 0.37;
            [t.sin() * 5.0, t.cos() * 5.0, (i % 17) as f32 - 8.0]
        })
        .collect();
    let f = [0.0, 0.6, 0.8];
    let o = [0.0, -20.0, -20.0];
    let order = sort_back_to_front(&p, 1, p.len(), o, f, false);
    let depth = |i: u32| {
        let q = p[i as usize];
        (q[1] - o[1]) * f[1] + (q[2] - o[2]) * f[2]
    };
    assert_eq!(order.len(), 1000);
    assert!(order.windows(2).all(|w| depth(w[0]) >= depth(w[1])));
    // Behind the eye is left out in perspective, kept in orthographic.
    let o = [0.0, 0.0, 0.0];
    let order = sort_back_to_front(&p, 1, p.len(), o, f, false);
    assert!(order.len() < 1000 && order.iter().all(|&i| depth(i) > 0.0));
    let all = sort_back_to_front(&p, 2, 500, o, f, true);
    assert_eq!(all.len(), 500);
}

/// The GPU draws what the CPU rasteriser draws, for the same orthographic view.
#[test]
fn splats_match_the_cpu_rasteriser() {
    let Some(mut h) = Harness::new() else { return };
    let s = splat_scene();
    let v = splat_view(&s, Projection::Orthographic);
    let (shot, drawn) = splat_shot(&mut h, &s, &v);
    assert_eq!(drawn, 1200);
    let p = v
        .camera
        .project(&s.bounds, f64::from(SIZE.0), f64::from(SIZE.1), 0.0);
    let cpu_view = specular_splat::preview::View {
        right: v.camera.right.map(|c| c as f32),
        down: v.camera.down.map(|c| c as f32),
        forward: p.forward.map(|c| c as f32),
        eye: None,
        target: p.target.map(|c| c as f32),
        focal: (1.0 / p.units_per_px) as f32,
    };
    let cpu = specular_splat::preview::render_view(&s, &cpu_view, SIZE.0, SIZE.1);
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let enc = |c: f64| {
        if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    let mut se = 0f64;
    for (a, b) in cpu.chunks_exact(4).zip(shot.rgb.chunks_exact(3)) {
        let alpha = f64::from(a[3]) / 255.0;
        for c in 0..3 {
            let want = enc(lin(f64::from(a[c]) / 255.0) * alpha) * 255.0;
            let d = want - f64::from(b[c]);
            se += d * d;
        }
    }
    let mse = se / f64::from(SIZE.0 * SIZE.1 * 3);
    let psnr = 10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10();
    eprintln!("GPU against CPU: {psnr:.1} dB");
    assert!(psnr > 30.0, "{psnr:.1} dB");
    assert!(lit(&shot) > 2000, "{}", lit(&shot));
}

/// The splat views of the generated scene, as goldens (`SPECULAR_BLESS=1` to update).
#[test]
fn splat_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut g = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "splats",
    );
    let s = splat_scene();
    let mut v = splat_view(&s, Projection::Perspective);
    v.grid = true;
    g.check("splats-ring-iso", &splat_shot(&mut h, &s, &v).0);
    // From the far side the ring's view-dependent colour differs.
    let mut back = v;
    back.camera.orbit(std::f64::consts::PI, 0.0);
    let (b, _) = splat_shot(&mut h, &s, &back);
    assert_ne!(b.rgb, splat_shot(&mut h, &s, &v).0.rgb);
    g.check("splats-ring-back", &b);
    let mut top = splat_view(&s, Projection::Orthographic);
    top.camera.look(
        StandardView::Top,
        &s.bounds,
        f64::from(SIZE.0),
        f64::from(SIZE.1),
    );
    g.check("splats-ring-top-ortho", &splat_shot(&mut h, &s, &top).0);
    g.finish();
}

/// Upload, sort and frame time of the corpus's largest splat scene (786k splats, degree-3
/// harmonics; tools/corpus/fetch-models.sh) on the machine's own GPU at 1920×1080, orbiting
/// (run with `--ignored --nocapture`).
#[test]
#[ignore = "timing; needs a hardware GPU and the models corpus"]
fn large_splats_gpu_timing() {
    use std::time::Instant;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/corpus/models/splats/hornedlizard.spz");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("no {}", path.display());
        return;
    };
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("no GPU: {e}");
            return;
        }
    };
    eprintln!("adapter: {} ({:?})", gpu.info.name, gpu.info.device_type);
    let wait = |gpu: &Gpu| {
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    };
    let t = Instant::now();
    let s = std::sync::Arc::new(
        specular_splat::read_spz(&bytes, &specular_splat::Limits::default()).unwrap(),
    );
    eprintln!("read {} splats in {:?}", s.len(), t.elapsed());
    let size = (1920u32, 1080u32);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("timing target"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let mut r = SplatRenderer::new(&gpu);
    let mut g = SplatsGpu::new(&gpu, s.clone());
    let t = Instant::now();
    let mut feeds = 0;
    while !g.complete() {
        g.feed(&gpu);
        feeds += 1;
    }
    wait(&gpu);
    eprintln!(
        "uploaded {:.0} MB in {feeds} frames, {:?}",
        g.bytes() as f64 / 1e6,
        t.elapsed()
    );
    let mut camera = Camera::default();
    camera.look(
        StandardView::Iso,
        &s.bounds,
        f64::from(size.0),
        f64::from(size.1),
    );
    let mut v = SplatView {
        rect: [0, 0, size.0, size.1],
        camera,
        grid: true,
    };
    let (mut sort, mut total, n) = (0f64, 0f64, 20);
    for _ in 0..n {
        v.camera.orbit(0.05, 0.0);
        let t = Instant::now();
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        r.draw(&gpu, &mut enc, &tv, size, &mut g, &v);
        gpu.queue.submit([enc.finish()]);
        wait(&gpu);
        total += t.elapsed().as_secs_f64() * 1e3;
        sort += f64::from(g.sort_ms);
    }
    eprintln!(
        "orbit at 1080p: {:.1} ms a frame, of which sorting {:.1} ms ({} drawn)",
        total / f64::from(n),
        sort / f64::from(n),
        g.drawn
    );
    let t = Instant::now();
    let px = specular_splat::preview::render(&s, 256, 256);
    eprintln!(
        "CPU thumbnail 256 px: {:?} ({} bytes)",
        t.elapsed(),
        px.len()
    );
}

/// A grey wall in the plane y = 0 (z up), 4 wide and 2 high.
fn wall() -> Mesh {
    Mesh {
        positions: vec![
            [-2.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 2.0],
            [-2.0, 0.0, 2.0],
        ],
        normals: vec![[0.0, -1.0, 0.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        parts: vec![Part {
            first: 0,
            count: 2,
            material: None,
        }],
        up: Up::Z,
        bounds: Bounds {
            min: [-2.0, 0.0, 0.0],
            max: [2.0, 0.0, 2.0],
        },
        ..Mesh::default()
    }
}

/// Red points on the left half in front of the wall (y = -0.5), on the right half behind
/// it (y = +0.5).
fn sheets() -> std::sync::Arc<specular_points::Loading> {
    let mut pos = Vec::new();
    for j in 0..40 {
        for i in 0..80 {
            let x = -1.9 + f64::from(i) * 0.0475;
            let y = if x < 0.0 { -0.5 } else { 0.5 };
            pos.push([x as f32, y, 0.1 + j as f32 * 0.045]);
        }
    }
    let rgb = vec![[230u8, 30, 30]; pos.len()];
    specular_points::from_points(pos, Some(rgb), [0.0; 3], "test").unwrap()
}

/// Reddish pixels in the left and right halves of `s`.
fn red_halves(s: &Shot) -> (usize, usize) {
    let (mut l, mut r) = (0, 0);
    for y in 0..s.height {
        for x in 0..s.width {
            let [cr, cg, cb] = px(s, x, y);
            if cr > 100 && u32::from(cr) > 2 * u32::from(cg) && u32::from(cr) > 2 * u32::from(cb) {
                if x < s.width / 2 {
                    l += 1;
                } else {
                    r += 1;
                }
            }
        }
    }
    (l, r)
}

/// Points drawn over a mesh share its depth: those behind the wall are hidden, those in
/// front show; both are framed on one box so they line up.
#[test]
fn points_behind_a_mesh_are_hidden() {
    let Some(mut h) = Harness::new() else { return };
    let mesh = wall();
    let loading = sheets();
    let mut frame = canonical_bounds(&mesh);
    frame.add([-2.0, -0.5, 0.0]);
    frame.add([2.0, 0.5, 2.0]);
    for projection in [Projection::Perspective, Projection::Orthographic] {
        let mut camera = Camera {
            projection,
            ..Camera::default()
        };
        camera.look(
            StandardView::Front,
            &frame,
            f64::from(SIZE.0),
            f64::from(SIZE.1),
        );
        let v = View {
            rect: [0, 0, SIZE.0, SIZE.1],
            camera,
            shading: Shading::Shaded,
            grid: false,
            scale: 1.0,
        };
        let pv = PointView {
            rect: v.rect,
            camera,
            grid: false,
            scale: 1.0,
            colour: PointColour::Rgb,
            size: 3.0,
            world: false,
            edl: false,
            budget: 10_000_000,
        };
        for shared in [true, false] {
            let mut mr = MeshRenderer::new(&h.gpu);
            mr.set_frame(Some(frame));
            let gm = mr.upload(&h.gpu, &mesh, &[]);
            let mut pr = PointRenderer::new(&h.gpu);
            pr.set_frame(Some(frame));
            let mut g = upload_all(&pr, &h.gpu, &loading);
            let shot = h.render_gpu(
                (SIZE.0 as f32, SIZE.1 as f32),
                1.0,
                |gpu, enc, target, size| {
                    mr.draw(gpu, enc, target, size, &gm, &v);
                    let under = if shared { mr.depth() } else { None };
                    pr.draw_over(gpu, enc, target, size, &mut g, &pv, under);
                },
                |_, _| {},
            );
            let (l, r) = red_halves(&shot);
            if shared {
                assert!(l > 1500, "{projection:?}: front points show ({l})");
                assert!(r < 20, "{projection:?}: points behind are hidden ({r})");
            } else {
                assert!(
                    l > 1500 && r > 1500,
                    "{projection:?}: unshared draws both ({l}, {r})"
                );
            }
        }
    }
}
