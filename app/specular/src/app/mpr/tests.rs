//! MPR's geometry (layout, crosshair, rotation, scrolling, slabs) on the analytic phantom,
//! and loading a phantom series from files.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use specular_dicom::series::{self, Series};
use specular_dicom::volume::phantom;
use specular_render::{VoiShape, Window};
use specular_render_volume::{Slab, reference};

use super::draw::{self, layout};
use super::load::{Loading, Source, lock};
use super::views::{Grab, R, V3, Views, cross, dot};

const SIZE: [u32; 3] = [96, 96, 80];
const PIXEL: f64 = 3.2;
const SPACING: f64 = 5.0;

fn window() -> Window {
    Window {
        center: 40.0,
        width: 400.0,
        shape: VoiShape::Linear,
        invert: false,
    }
}

const RECT: R = [10.0, 20.0, 400.0, 300.0];

fn near(a: V3, b: V3, eps: f64) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= eps)
}

#[test]
fn layout_is_a_two_by_two_grid_mirrored_in_rtl() {
    let l = layout([0.0, 0.0, 801.0, 601.0], 1.0, false);
    assert_eq!(l.views[0], [0.0, 0.0, 400.0, 300.0]);
    assert_eq!(l.views[1], [401.0, 0.0, 400.0, 300.0]);
    assert_eq!(l.views[2], [0.0, 301.0, 400.0, 300.0]);
    assert_eq!(l.slot, [401.0, 301.0, 400.0, 300.0]);
    let r = layout([0.0, 0.0, 801.0, 601.0], 1.0, true);
    assert_eq!(r.views[0], l.views[1]);
    assert_eq!(r.views[1], l.views[0]);
    assert_eq!(r.slot, l.views[2]);
    // At 1.5× every edge lands on a whole physical pixel.
    let h = layout([0.0, 0.0, 533.0, 400.0], 1.5, false);
    for r in h.views.iter().chain([&h.slot]) {
        for v in r {
            let px = v * 1.5;
            assert!((px - px.round()).abs() < 1e-9, "{r:?}");
        }
    }
}

#[test]
fn views_start_on_the_anatomical_planes_through_the_centre() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let v = Views::new(&vol.geometry);
    assert!(near(v.c, vol.geometry.centre(), 1e-9));
    assert!(
        near(v.normal(0), [0.0, 0.0, 1.0], 1e-12),
        "axial looks along z"
    );
    assert!(near(v.normal(1).map(f64::abs), [0.0, 1.0, 0.0], 1e-12));
    assert!(near(v.normal(2).map(f64::abs), [1.0, 0.0, 0.0], 1e-12));
    assert!((0..3).all(|i| !v.oblique(i)));
    // Head up in the coronal and sagittal views (superior is +z in LPS).
    assert!(v.frames[1].down[2] < 0.0 && v.frames[2].down[2] < 0.0);
}

#[test]
fn screen_and_patient_round_trip_in_every_view() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    v.rotate(0, 0.4);
    v.frames[1].zoom = 2.5;
    v.frames[1].pan = [7.0, -3.0];
    for i in 0..3 {
        for p in [(30.0, 40.0), (200.0, 170.0), (399.0, 310.0)] {
            let q = v.to_patient(i, RECT, p);
            assert!(v.on_plane(i, q));
            let s = v.to_screen(i, RECT, q);
            assert!((s.0 - p.0).abs() < 1e-9 && (s.1 - p.1).abs() < 1e-9);
        }
    }
}

/// A measurement's ends are grabbed where they are drawn (the nearest within the radius),
/// only in a view whose plane holds both ends, and a dragged end follows the pointer.
#[test]
fn measurement_ends_are_grabbed_where_drawn() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let v = Views::new(&vol.geometry);
    let a = v.to_patient(0, RECT, (100.0, 100.0));
    let b = v.to_patient(0, RECT, (160.0, 100.0));
    let c = v.to_patient(0, RECT, (106.0, 100.0));
    let mut measures = vec![[a, b], [c, b]];
    assert_eq!(
        v.measure_end_at(0, RECT, &measures, (101.0, 101.0), 12.0),
        Some((0, 0))
    );
    assert_eq!(
        v.measure_end_at(0, RECT, &measures, (105.0, 100.0), 12.0),
        Some((1, 0))
    );
    assert_eq!(
        v.measure_end_at(0, RECT, &measures, (159.0, 99.0), 12.0),
        Some((0, 1))
    );
    assert_eq!(
        v.measure_end_at(0, RECT, &measures, (130.0, 100.0), 12.0),
        None
    );
    // Not drawn in a view whose plane misses an end: the sagittal plane holds neither.
    let i = (1..3)
        .find(|&i| !(v.on_plane(i, a) && v.on_plane(i, b)))
        .expect("a view that misses an end");
    let at = v.to_screen(i, RECT, a);
    assert_eq!(v.measure_end_at(i, RECT, &measures[..1], at, 12.0), None);
    // The end moved stays on the view's plane and lands under the pointer.
    measures[0][1] = v.to_patient(0, RECT, (200.0, 140.0));
    assert!(v.on_plane(0, measures[0][1]));
    assert_eq!(
        v.measure_end_at(0, RECT, &measures, (199.0, 141.0), 12.0),
        Some((0, 1))
    );
}

#[test]
fn moving_the_crosshair_moves_every_plane_through_the_point() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    let p = (150.0, 120.0);
    v.move_to(0, RECT, p);
    let at = v.to_screen(0, RECT, v.c);
    assert!((at.0 - p.0).abs() < 1e-9 && (at.1 - p.1).abs() < 1e-9);
    // The other views' lines cross at the crosshair, and it lies on all three planes.
    for i in 0..3 {
        assert!(v.on_plane(i, v.c));
        for l in v.lines(i, RECT) {
            let at = v.to_screen(i, RECT, v.c);
            assert!((l.at.0 - at.0).abs() < 1e-9);
        }
    }
}

#[test]
fn rotation_keeps_the_planes_orthogonal_and_the_rotated_view_still() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    let axial = v.frames[0];
    v.rotate(0, 0.5);
    assert_eq!(v.frames[0], axial);
    assert!(!v.oblique(0) && v.oblique(1) && v.oblique(2));
    for a in 0..3 {
        let f = v.frames[a];
        assert!(dot(f.right, f.down).abs() < 1e-12);
        for b in a + 1..3 {
            assert!(dot(v.normal(a), v.normal(b)).abs() < 1e-12);
        }
    }
    // The coronal plane turned by the angle about z.
    let n = v.normal(1);
    assert!((n[2]).abs() < 1e-12);
    assert!((n[1].abs() - 0.5f64.cos()).abs() < 1e-12);
    // The axial view's lines turned with it.
    let l = v.lines(0, RECT);
    let d = cross(v.normal(0), v.normal(1));
    assert!((l[0].dir.0 - d[0]).abs() < 1e-9 && (l[0].dir.1 - d[1]).abs() < 1e-9);
    // Turning back restores the planes.
    v.rotate(0, -0.5);
    assert!((0..3).all(|i| !v.oblique(i)));
}

#[test]
fn a_line_end_grabs_rotation_and_elsewhere_moves() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let v = Views::new(&vol.geometry);
    let h = v.handles(0, RECT)[0];
    assert!(matches!(
        v.grab(0, RECT, (h.0 + 3.0, h.1), 12.0),
        Grab::Rotate(_)
    ));
    let at = v.to_screen(0, RECT, v.c);
    assert_eq!(v.grab(0, RECT, (at.0 + 5.0, at.1 + 5.0), 12.0), Grab::Move);
}

#[test]
fn scrolling_stays_within_the_volume() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    let z0 = v.position(0);
    v.scroll(0, 1.0);
    assert!((v.position(0) - z0 - v.step).abs() < 1e-9);
    v.scroll(0, 1e6);
    let top = vol
        .geometry
        .corners()
        .iter()
        .map(|c| c[2])
        .fold(f64::MIN, f64::max);
    assert!((v.position(0) - top).abs() < 1e-9);
    // Only the axial plane moved.
    assert!((v.c[0] - vol.geometry.centre()[0]).abs() < 1e-9);
    // Positions read as the patient coordinates (sagittal: x, towards the patient's left),
    // and stepping forward goes up them.
    v.c = [6.0, -4.0, 2.0];
    assert!((v.position(2) - 6.0).abs() < 1e-9);
    assert!((v.position(1) + 4.0).abs() < 1e-9);
    v.scroll(2, 1.0);
    assert!((v.position(2) - 6.0 - v.step).abs() < 1e-9);
}

#[test]
fn zoom_keeps_the_point_under_the_pointer() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    let p = (90.0, 250.0);
    let q = v.to_patient(1, RECT, p);
    v.zoom_at(1, RECT, 3.0, p);
    assert!(near(v.to_patient(1, RECT, p), q, 1e-9));
    assert!((v.mm(1, RECT) * 3.0 - Views::new(&vol.geometry).mm(1, RECT)).abs() < 1e-9);
}

#[test]
fn slabs_cycle_thin_mip_minip_average() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    assert_eq!(v.plane(0, RECT, 1.0, window()).thickness, 0.0);
    let mut seen = Vec::new();
    for _ in 0..4 {
        v.next_slab();
        let p = v.plane(0, RECT, 1.0, window());
        seen.push((p.thickness > 0.0).then_some(p.slab));
    }
    assert_eq!(
        seen,
        [
            Some(Slab::Mip),
            Some(Slab::MinIp),
            Some(Slab::Average),
            None
        ]
    );
    v.set_slab(Some(Slab::Mip));
    v.step_thickness(true);
    assert_eq!(v.thickness, 15.0);
    for _ in 0..20 {
        v.step_thickness(false);
    }
    assert_eq!(v.thickness, 0.0);
    assert_eq!(
        draw::slab_text(&v, super::load::Length::Mm),
        specular_i18n::t!("mpr-thin")
    );
}

/// A density map's or cube file's lengths are Ångström: the slab, positions and measured
/// lengths say Å, never mm.
#[test]
fn angstrom_grids_say_angstrom() {
    let g = specular_dicom::volume::Geometry::new(
        &[[0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0],
        [4, 4],
    );
    let mut v = Views::new(&g);
    v.set_slab(Some(Slab::Mip));
    v.step_thickness(true);
    let a = draw::slab_text(&v, super::load::Length::Angstrom);
    assert!(a.ends_with('Å') && !a.contains("mm"), "{a}");
    assert!(draw::slab_text(&v, super::load::Length::Mm).ends_with("mm"));
    assert!(
        super::length_text(12.5, super::load::Length::Angstrom).contains("12.5")
            && super::length_text(12.5, super::load::Length::Angstrom).ends_with('Å')
    );
    assert!(super::length_text(12.5, super::load::Length::Mm).ends_with("mm"));
}

/// In a small window the 3D view's readout keeps its first lines and never grows over the
/// controls; with room it shows every line, and with none it is left out.
#[test]
fn readout_drops_lines_that_would_cover_the_controls() {
    let lines = [16.0; 5];
    // 2 × 6 padding + 5 × 16 + 4 × 2 gaps = 100.
    assert_eq!(draw::readout_lines_that_fit(&lines, 2.0, 6.0, 100.0), 5);
    assert_eq!(draw::readout_lines_that_fit(&lines, 2.0, 6.0, 99.0), 4);
    assert_eq!(draw::readout_lines_that_fit(&lines, 2.0, 6.0, 46.0), 2);
    assert_eq!(draw::readout_lines_that_fit(&lines, 2.0, 6.0, 20.0), 0);
    assert_eq!(draw::readout_lines_that_fit(&lines, 2.0, 6.0, -5.0), 0);
}

/// The views sample the phantom where they say they are: the bone sphere's centre in every
/// plane, straight and oblique, and the CPU reference matches the analytic phantom away
/// from edges.
#[test]
fn planes_sample_the_phantom_where_the_crosshair_is() {
    let vol = phantom::volume(SIZE, PIXEL, SPACING, 0.0);
    let mut v = Views::new(&vol.geometry);
    let (bone, _, hu) = phantom::SPHERES[0];
    v.c = bone;
    for turn in [0.0, 0.35] {
        v.rotate(1, turn);
        for i in 0..3 {
            let plane = v.plane(i, RECT, 1.5, window());
            let at = v.to_screen(i, RECT, v.c);
            let x = reference(&vol, &plane, at.0 * 1.5, at.1 * 1.5).expect("inside");
            assert!((x - hu).abs() < 1.0, "view {i} turn {turn}: {x}");
            // Elsewhere on the plane: agrees with the analytic phantom where it is flat
            // (more than a voxel from any edge).
            for p in [(60.0, 80.0), (250.0, 200.0), (330.0, 100.0)] {
                let q = v.to_patient(i, RECT, p);
                let edge = [-2.0 * SPACING, 0.0, 2.0 * SPACING].iter().any(|&o| {
                    [0, 1, 2].iter().any(|&a| {
                        let mut r = q;
                        r[a] += o;
                        phantom::hu(r) != phantom::hu(q)
                    })
                });
                let Some(x) = reference(&vol, &plane, p.0 * 1.5 + 0.5, p.1 * 1.5 + 0.5) else {
                    continue;
                };
                if !edge {
                    let q = plane.at(p.0 * 1.5 + 0.5, p.1 * 1.5 + 0.5);
                    assert_eq!(x, phantom::hu(q), "view {i} at {p:?}");
                }
            }
        }
    }
}

/// Gantry tilt: the volume keeps the tilted slices exact (sheared), so the views still find
/// the bone sphere at its patient position.
#[test]
fn tilted_stacks_sample_at_patient_positions() {
    let vol = phantom::volume(SIZE, PIXEL, SPACING, 15.0);
    let mut v = Views::new(&vol.geometry);
    let (bone, _, hu) = phantom::SPHERES[0];
    v.c = bone;
    for i in 0..3 {
        let plane = v.plane(i, RECT, 1.0, window());
        let at = v.to_screen(i, RECT, v.c);
        let x = reference(&vol, &plane, at.0, at.1).expect("inside");
        assert!((x - hu).abs() < 1.0, "view {i}: {x}");
    }
}

/// The phantom series written to `dir` and assembled as series mode would.
pub(super) fn phantom_series(
    dir: &std::path::Path,
    size: [u32; 3],
    skip: Option<usize>,
) -> Series<PathBuf> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    let mut items = Vec::new();
    for (name, bytes) in phantom::series(size, PIXEL, SPACING, 0.0) {
        if skip.is_some_and(|k| name == format!("slice-{k:04}.dcm")) {
            continue;
        }
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        let inst = series::read_header(&p).unwrap().unwrap();
        items.push((p, inst));
    }
    let mut sets = series::assemble(items);
    assert_eq!(sets.len(), 1);
    sets.remove(0)
}

pub(super) fn wait(l: &mut Loading, timeout: Duration) -> bool {
    let t = Instant::now();
    let mut refused = false;
    while t.elapsed() < timeout {
        let p = l.poll();
        refused |= p.refused.is_some();
        if refused || l.finished() {
            return !refused;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!(
        "loading timed out: {} done, {} pending, {:?}",
        l.done,
        l.pending(),
        l.info
    );
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("specular-mpr-{name}-{}", std::process::id()))
}

#[test]
fn a_series_loads_off_thread_into_the_phantom_volume() {
    let dir = scratch("load");
    let size = [48, 40, 30];
    let set = phantom_series(&dir, size, None);
    let pool = Arc::new(specular_core::JobPool::new(2, "mpr-test"));
    let mut l = Loading::start(Source::Series(set), pool, Arc::new(|| {}), 2048, 1 << 30);
    assert!(wait(&mut l, Duration::from_secs(60)));
    let info = l.info.clone().unwrap();
    assert_eq!(info.slices, 30);
    assert_eq!(info.unit.as_deref(), Some("HU"));
    assert_eq!(l.failed, 0);
    let want = phantom::volume(size, PIXEL, SPACING, 0.0);
    let got = lock(&l.volume);
    assert!(got.complete());
    assert_eq!(got.geometry.dims, want.geometry.dims);
    for p in [
        [0.0, 0.0, 0.0],
        [60.0, 0.0, 0.0],
        [10.0, -20.0, 33.0],
        [-70.0, 60.0, -50.0],
    ] {
        assert_eq!(got.sample_patient(p), want.sample_patient(p), "{p:?}");
    }
    drop(got);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_series_over_budget_loads_downsampled() {
    let dir = scratch("budget");
    let size = [48, 40, 30];
    let set = phantom_series(&dir, size, None);
    let pool = Arc::new(specular_core::JobPool::new(2, "mpr-test"));
    // Room for a quarter of the voxels.
    let budget = u64::from(size[0] * size[1] * size[2]) * 2 / 4;
    let mut l = Loading::start(Source::Series(set), pool, Arc::new(|| {}), 2048, budget);
    assert!(wait(&mut l, Duration::from_secs(60)));
    let info = l.info.clone().unwrap();
    assert!(
        info.plan.step_xy > 1 || info.plan.step_z > 1,
        "{:?}",
        info.plan
    );
    assert!(info.plan.bytes <= budget);
    // Still the phantom: the bone sphere's centre.
    let got = lock(&l.volume);
    let x = got.sample_patient(phantom::SPHERES[0].0).unwrap();
    assert!((x - 1000.0).abs() < 1.0, "{x}");
    drop(got);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_series_with_a_missing_slice_is_refused() {
    let dir = scratch("gap");
    let set = phantom_series(&dir, [16, 16, 12], Some(5));
    let pool = Arc::new(specular_core::JobPool::new(1, "mpr-test"));
    let mut l = Loading::start(Source::Series(set), pool, Arc::new(|| {}), 2048, 1 << 30);
    let t = Instant::now();
    let refused = loop {
        if let Some(r) = l.poll().refused {
            break r;
        }
        assert!(t.elapsed() < Duration::from_secs(30), "no answer");
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(!refused.is_empty());
    assert!(l.info.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_cancels_the_queued_slices() {
    let dir = scratch("cancel");
    let set = phantom_series(&dir, [64, 64, 60], None);
    let pool = Arc::new(specular_core::JobPool::new(1, "mpr-test"));
    // Hold the only worker so nothing past the first slice runs before the cancel.
    let gate = Arc::new(std::sync::Barrier::new(2));
    let g = gate.clone();
    pool.submit_labeled(
        specular_core::Priority::Visible,
        specular_core::CancelToken::new(),
        "gate",
        move |_| {
            g.wait();
        },
    );
    let mut l = Loading::start(
        Source::Series(set),
        pool.clone(),
        Arc::new(|| {}),
        2048,
        1 << 30,
    );
    let t = Instant::now();
    while l.info.is_none() {
        l.poll();
        assert!(t.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    // Every slice past the first is queued.
    while l.pending() < 59 {
        assert!(t.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    l.cancel();
    gate.wait();
    while l.pending() > 0 {
        assert!(t.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(20));
    l.poll();
    assert_eq!(l.done, 1, "only the first slice was written");
    assert_eq!(lock(&l.volume).loaded_count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Time to the first view and to fully loaded for a 512 × 512 × 500 phantom series (about
/// 262 MB of voxels): `cargo test -p specular --bin specular mpr::tests::bench -- --ignored
/// --nocapture`. The files go to `SPECULAR_MPR_BENCH_DIR` (default: the temp dir).
#[test]
#[ignore = "benchmark: writes 262 MB of DICOM"]
fn bench_512_cube() {
    let dir =
        std::env::var_os("SPECULAR_MPR_BENCH_DIR").map_or_else(|| scratch("bench"), PathBuf::from);
    let size = [512, 512, 500];
    let t = Instant::now();
    let set = phantom_series(&dir, size, None);
    eprintln!("series written in {:.1} s", t.elapsed().as_secs_f64());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pool = Arc::new(specular_core::JobPool::new(threads, "dec"));
    let (tx, rx) = crossbeam_channel::unbounded();
    let wake: crate::loader::Wake = Arc::new(move || {
        let _ = tx.send(());
    });
    let gpu = specular_gfx::Gpu::new().ok();
    let max_dim = gpu
        .as_ref()
        .map_or(2048, specular_render_volume::max_dimension);
    let mut l = Loading::start(Source::Series(set), pool, wake, max_dim, 4 << 30);
    let mut tex: Option<specular_render_volume::VolumeTexture> = None;
    let mut first = None;
    while !l.finished() {
        let _ = rx.recv_timeout(Duration::from_millis(50));
        let p = l.poll();
        assert!(p.refused.is_none(), "{:?}", p.refused);
        if let Some(g) = &gpu {
            if p.opened {
                tex =
                    Some(specular_render_volume::VolumeTexture::new(g, &lock(&l.volume)).unwrap());
            }
            if let Some(t) = tex.as_mut()
                && l.dirty
            {
                l.dirty = false;
                t.sync(g, &lock(&l.volume)).unwrap();
                if first.is_none() && t.uploaded() > 0 {
                    g.device
                        .poll(specular_gfx::wgpu::PollType::wait_indefinitely())
                        .ok();
                    first = Some(l.started.elapsed());
                }
            }
        } else if p.opened && first.is_none() {
            first = Some(l.started.elapsed());
        }
    }
    if let (Some(g), Some(t)) = (&gpu, tex.as_mut()) {
        t.sync(g, &lock(&l.volume)).unwrap();
        g.device
            .poll(specular_gfx::wgpu::PollType::wait_indefinitely())
            .ok();
    }
    let all = l.started.elapsed();
    eprintln!(
        "512x512x500: first view {:.0} ms, fully loaded {:.0} ms, {} MiB on the GPU ({}; {} threads)",
        first.unwrap_or_default().as_secs_f64() * 1e3,
        all.as_secs_f64() * 1e3,
        tex.as_ref().map_or(0, |t| t.bytes() >> 20),
        gpu.as_ref().map_or("no GPU", |g| g.info.name.as_str()),
        threads
    );
    drop(l);
    if std::env::var_os("SPECULAR_MPR_BENCH_DIR").is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Writes a 192 × 192 × 120 phantom series to `SPECULAR_MPR_PHANTOM_DIR`, for trying MPR in
/// the viewer.
#[test]
#[ignore = "writes a phantom series for manual checks"]
fn write_phantom_series() {
    let Some(dir) = std::env::var_os("SPECULAR_MPR_PHANTOM_DIR") else {
        return;
    };
    phantom_series(std::path::Path::new(&dir), [192, 192, 120], None);
}

/// A research volume (a sagittal NIfTI stack, the second time point of a 4D one, scaled
/// values, a gzip NRRD) loads into a volume whose voxels sit where the file puts them in the
/// patient; a colour volume is refused.
#[test]
fn research_volumes_load_at_their_patient_positions() {
    use specular_decode::medvol;
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-medvol/tests/data");
    for (name, v) in [
        ("sagittal.nii", 0),
        ("time.nii", 1),
        ("scaled.nii", 0),
        ("enc_gzip.nrrd", 0),
    ] {
        let path = data.join(name);
        let want = medvol::open_path(&path).unwrap();
        let pool = Arc::new(specular_core::JobPool::new(2, "mpr-test"));
        let mut l = Loading::start(
            Source::Research(path.clone(), v, None),
            pool,
            Arc::new(|| {}),
            2048,
            1 << 30,
        );
        assert!(wait(&mut l, Duration::from_secs(60)), "{name}");
        assert_eq!(l.failed, 0, "{name}");
        let got = lock(&l.volume);
        assert!(got.complete(), "{name}");
        let s = want.header.layout.size;
        for k in 0..s[2] {
            for j in 0..s[1] {
                for i in 0..s[0] {
                    let p = want.header.to_patient([i as f64, j as f64, k as f64]);
                    let a = got.sample_patient(p).unwrap();
                    let b = want.value(i, j, k, v, 0) as f32;
                    assert!(
                        (a - b).abs() <= 1e-3 * b.abs().max(1.0),
                        "{name} {i},{j},{k}: {a} vs {b}"
                    );
                }
            }
        }
    }
    let pool = Arc::new(specular_core::JobPool::new(1, "mpr-test"));
    let mut l = Loading::start(
        Source::Research(data.join("rgb.nii"), 0, None),
        pool,
        Arc::new(|| {}),
        2048,
        1 << 30,
    );
    assert!(!wait(&mut l, Duration::from_secs(60)));
}

/// A 3-D seismic volume loads with crosslines across, inlines down and samples stacked
/// bottom up (time down the inline and crossline views), the gain's window about 0, gaps
/// as 0; the views' position lines name the time, the inline and the crossline.
#[test]
fn seismic_volumes_load_as_time_slices() {
    use specular_core::CancelToken;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-segy/tests/data/gaps_volume.sgy");
    let bytes = std::fs::read(&path).unwrap();
    let session = specular_decode::Registry::default()
        .open(Arc::new(specular_core::Source::from_vec(
            "gaps_volume.sgy",
            bytes,
        )))
        .unwrap();
    let seismic = session.science().unwrap().seismic.clone().unwrap();
    assert!(seismic.is_volume());
    let cube = specular_decode::seismic_cube(&path, &CancelToken::new())
        .unwrap()
        .unwrap();
    let pool = Arc::new(specular_core::JobPool::new(1, "mpr-test"));
    let mut l = Loading::start(
        Source::Seismic(path, seismic.clone()),
        pool,
        Arc::new(|| {}),
        2048,
        1 << 30,
    );
    assert!(wait(&mut l, Duration::from_secs(60)));
    assert_eq!(l.failed, 0);
    let info = l.info.clone().unwrap();
    let (nil, nxl, ns) = (cube.inlines, cube.crosslines, cube.samples);
    assert_eq!(info.full, [nxl as u32, nil as u32, ns as u32]);
    let w = info.window.unwrap();
    assert_eq!(w.center, 0.0);
    assert!(w.width > 0.0);
    assert!(info.seismic.is_some());
    let got = lock(&l.volume);
    assert!(got.complete());
    let mut gaps = 0;
    for il in 0..nil {
        for xl in 0..nxl {
            for k in 0..ns {
                let want = cube.values[(il * nxl + xl) * ns + k];
                let a = got.value(xl as u32, il as u32, (ns - 1 - k) as u32);
                if want.is_nan() {
                    gaps += 1;
                    assert_eq!(a, 0.0);
                } else {
                    assert_eq!(a, want, "{il},{xl},{k}");
                }
            }
        }
    }
    assert!(gaps > 0, "the fixture has gaps");
    // The readout's position lines and header numbers.
    let line = super::seismic_position(&seismic, 1, 2.0, ns as u32);
    assert!(line.contains(&seismic.inlines[2].to_string()), "{line}");
    let line = super::seismic_position(&seismic, 2, 0.0, ns as u32);
    assert!(line.contains(&seismic.crosslines[0].to_string()), "{line}");
    let line = super::seismic_position(&seismic, 0, (ns - 1) as f64, ns as u32);
    assert!(
        line.contains(&super::super::science::trimmed(seismic.t0, 3)),
        "{line}"
    );
    assert!(!super::seismic_cell(&seismic, [0.0, 0.0, 0.0], ns as u32).is_empty());
    assert!(super::seismic_cell(&seismic, [-1.0, 0.0, 0.0], ns as u32).is_empty());
}

/// In a small window the fourth slot keeps every control inside it, none over another, and
/// the 3D view below them at a usable height (the controls never cover it, nor it them).
#[test]
fn small_windows_keep_the_controls_clear_of_the_3d_view() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let views = Views::new(&vol.geometry);
    let pal = specular_theme::Palette::dark();
    let mut text = specular_gfx::Text::new();
    for (w, h, ts) in [
        (1024.0, 720.0, 1.0),
        (640.0, 480.0, 1.0),
        (480.0, 360.0, 1.0),
        (400.0, 300.0, 1.0),
        (640.0, 480.0, 1.5),
    ] {
        let l = layout([0.0, 0.0, w, h], 1.0, false);
        let scene = draw::Scene {
            views: &views,
            layout: l,
            readouts: Default::default(),
            labels: false,
            measures: Default::default(),
            panel: vec!["C 40  W 400".into(), "Thin".into()],
            active: None,
            vr: draw::VrLabels {
                preset: "CT bone".into(),
                mode: "Shaded".into(),
                clip: "No clipping".into(),
                smooth: false,
            },
            segments: Vec::new(),
            tracts: Default::default(),
            buttons_first: 0,
            maximised: false,
            curve: Default::default(),
        };
        let mut dl = specular_gfx::DrawList::new(1.0);
        let placed = draw::draw(&mut dl, &mut text, &scene, &pal, false, ts);
        let slot = draw::rect(l.slot);
        let v3 = placed.view3d.expect("the 3D view placed");
        let at = format!("{w}×{h} at text {ts}");
        let mut bottom = slot.y;
        for (i, (c, r, _)) in placed.controls.iter().enumerate() {
            assert!(
                r.x >= slot.x - 0.01
                    && r.y >= slot.y - 0.01
                    && r.x + r.w <= slot.x + slot.w + 0.01
                    && r.y + r.h <= slot.y + slot.h + 0.01,
                "{at}: {c:?} {r:?} outside the slot {slot:?}"
            );
            for (d, o, _) in &placed.controls[i + 1..] {
                let apart =
                    r.x + r.w <= o.x || o.x + o.w <= r.x || r.y + r.h <= o.y || o.y + o.h <= r.y;
                assert!(apart, "{at}: {c:?} over {d:?}");
            }
            bottom = bottom.max(r.y + r.h);
        }
        assert!(
            v3[1] >= f64::from(bottom) - 0.01,
            "{at}: the 3D view {v3:?} over the controls"
        );
        assert!(
            v3[3] >= draw::MIN_3D.min(l.slot[3] / 3.0),
            "{at}: the 3D view {v3:?} too small"
        );
        // A screen reader finds the 3D view where it is drawn, and the screen passes the
        // audit (no control outside the window or under another).
        let mut tree = super::super::a11y_audit::window((w as f32, h as f32), 1.0);
        draw::a11y(&mut tree, &scene, &placed, String::new(), "3D".into(), 1.0);
        let (_, n) = tree
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("3D"))
            .expect("the 3D view's node");
        let b = n.bounds().expect("bounds");
        assert_eq!([b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0], v3, "{at}");
        super::super::a11y_audit::expect_clean(&at, &tree, false);
    }
}

/// In a slot too small for every button, they show a page at a time with More last, and
/// More's pages reach every button (they were dropped); the screen passes the audit.
#[test]
fn a_small_slot_pages_its_buttons() {
    let vol = phantom::volume([8, 8, 8], PIXEL, SPACING, 0.0);
    let views = Views::new(&vol.geometry);
    let pal = specular_theme::Palette::dark();
    let mut text = specular_gfx::Text::new();
    let (w, h) = (300.0, 200.0);
    let l = layout([0.0, 0.0, w, h], 1.0, false);
    let mut seen = std::collections::HashSet::new();
    let mut first = 0;
    let mut pages = 0;
    loop {
        let scene = draw::Scene {
            views: &views,
            layout: l,
            readouts: Default::default(),
            labels: false,
            measures: Default::default(),
            panel: Vec::new(),
            active: None,
            vr: draw::VrLabels {
                preset: "CT bone".into(),
                mode: "Shaded".into(),
                clip: "No clipping".into(),
                smooth: false,
            },
            segments: Vec::new(),
            tracts: Default::default(),
            buttons_first: first,
            maximised: false,
            curve: Default::default(),
        };
        let mut dl = specular_gfx::DrawList::new(1.0);
        let placed = draw::draw(&mut dl, &mut text, &scene, &pal, false, 1.0);
        let slot = draw::rect(l.slot);
        let next = placed.controls.iter().find_map(|(c, r, _)| {
            assert!(r.y + r.h <= slot.y + slot.h + 0.01, "{c:?} {r:?}");
            match c {
                draw::Control::More(n) => Some(*n),
                _ => None,
            }
        });
        let next = next.expect("a More button in a small slot");
        for (c, _, _) in &placed.controls {
            if !matches!(c, draw::Control::More(_)) {
                seen.insert(format!("{c:?}"));
            }
        }
        let mut tree = super::super::a11y_audit::window((w as f32, h as f32), 1.0);
        draw::a11y(&mut tree, &scene, &placed, String::new(), "3D".into(), 1.0);
        super::super::a11y_audit::expect_clean("small slot", &tree, false);
        pages += 1;
        if next <= first || pages > 13 {
            break;
        }
        first = next;
    }
    assert!(pages >= 2, "{pages} pages");
    // The fifteen MPR and 3D buttons, and Curved.
    assert_eq!(seen.len(), 16, "every button on some page: {seen:?}");
}

/// A colour composite's readout lists every shown channel by name, four a line.
#[test]
fn composite_readout_lists_every_channel() {
    let vals: Vec<(String, f32)> = ["DAPI", "GFP", "RFP", "Cy5", "Cy7"]
        .iter()
        .zip([12.0, 340.4, 7.6, 0.0, 1000.0])
        .map(|(n, v)| ((*n).to_owned(), v))
        .collect();
    let lines: Vec<String> = super::channel_lines(&vals)
        .into_iter()
        .map(|l| specular_i18n::strip_isolates(&l))
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    for (name, _) in &vals {
        assert!(
            lines.iter().any(|l| l.contains(name.as_str())),
            "{name}: {lines:?}"
        );
    }
    assert!(
        lines[0].contains("GFP 340") && lines[0].contains("Cy5 0"),
        "{lines:?}"
    );
    assert!(lines[1].contains("Cy7 1"), "{lines:?}");
}

/// A channel's histogram equalisation curve is the science view's table of its values in
/// the window, resampled: rising to 1, and lifting the dark values most voxels hold.
#[test]
fn histeq_curve_equalises_the_window() {
    use specular_dicom::volume::{Geometry, Volume, VoxelKind};
    let pos: Vec<[f64; 3]> = (0..4).map(|k| [0.0, 0.0, f64::from(k)]).collect();
    let g = Geometry::new(&pos, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0], [16, 16]);
    let mut v = Volume::new(g, VoxelKind::F32, 1).unwrap();
    let vals: Vec<f32> = (0..256)
        .map(|i| if i < 192 { (i % 10) as f32 } else { 100.0 })
        .collect();
    for k in 0..4 {
        v.put(k, &vals, 16, 16).unwrap();
    }
    let w = specular_render::Window {
        center: 50.0,
        width: 100.0,
        shape: specular_render::VoiShape::LinearExact,
        invert: false,
    };
    let c = super::histeq_curve(&v, w);
    assert_eq!(c.len(), specular_render_volume::TONE_KNOTS);
    assert!(c.windows(2).all(|p| p[1] >= p[0]));
    assert_eq!(*c.last().unwrap(), 1.0);
    // A tenth of the window (values up to 10) holds three quarters of the voxels.
    let at = |x: f32| c[(x * 255.0) as usize];
    assert!(at(0.1) >= 0.7, "{}", at(0.1));
    assert!(at(0.5) < 0.8, "{}", at(0.5));

    // Made off the UI thread: one job at a time while the window moves, the last curve
    // drawn meanwhile, and the curve for where the window stopped in the end, the same as
    // made at once; the values are sampled once.
    let vol = Arc::new(std::sync::Mutex::new(v));
    let mut tone = super::ToneCurve::default();
    type Job = Box<dyn FnOnce() + Send>;
    let mut jobs: Vec<Job> = Vec::new();
    let at = |center: f64| Window { center, ..w };
    let ask = |tone: &mut super::ToneCurve, center: f64, jobs: &mut Vec<Job>| {
        tone.refresh(&vol, at(center), &mut |j| jobs.push(j))
    };
    assert!(!ask(&mut tone, 50.0, &mut jobs), "asked for");
    assert!(!ask(&mut tone, 40.0, &mut jobs), "one at a time");
    assert!(tone.curve().is_empty(), "linear until made");
    assert_eq!(jobs.len(), 1);
    jobs.pop().unwrap()();
    assert!(ask(&mut tone, 30.0, &mut jobs), "the first made");
    assert_eq!(tone.curve(), c);
    assert!(!ask(&mut tone, 30.0, &mut jobs));
    assert_eq!(jobs.len(), 1, "the window moved meanwhile");
    let sampled = tone.samples.clone().expect("sampled");
    jobs.pop().unwrap()();
    assert!(ask(&mut tone, 30.0, &mut jobs));
    assert!(!ask(&mut tone, 30.0, &mut jobs), "nothing more to make");
    assert!(jobs.is_empty());
    assert_eq!(tone.curve(), super::histeq_curve(&lock(&vol), at(30.0)));
    assert!(Arc::ptr_eq(&sampled, tone.samples.as_ref().expect("kept")));
}

/// A device stick pans a 2D pane like the image view (the content moves against the
/// stick) and zooms it about its centre.
#[test]
fn device_motion_pans_and_zooms_a_pane() {
    let vol = phantom::volume([16, 16, 16], PIXEL, SPACING, 0.0);
    let mut views = Views::new(&vol.geometry);
    let r = [0.0, 0.0, 200.0, 200.0];
    let c = (100.0, 100.0);
    let before = views.to_patient(0, r, c);
    // Stick to the right: what was under the centre is now to its left.
    super::device_pane(&mut views, 0, r, (10.0, 0.0), 1.0);
    let moved = views.to_patient(0, r, (90.0, 100.0));
    for k in 0..3 {
        assert!((moved[k] - before[k]).abs() < 1e-6, "{moved:?} {before:?}");
    }
    // Zoom keeps the centre's point and the others' planes alone.
    let at = views.to_patient(0, r, c);
    let other = views.to_patient(1, r, c);
    super::device_pane(&mut views, 0, r, (0.0, 0.0), 2.0);
    let after = views.to_patient(0, r, c);
    for k in 0..3 {
        assert!((after[k] - at[k]).abs() < 1e-6);
    }
    let far = views.to_patient(0, r, (150.0, 100.0));
    let near = views.to_patient(1, r, c);
    assert_eq!(near, other);
    assert!(far != at);
}
