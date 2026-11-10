//! The spline's geometry, and straightening against analytic phantoms: a tilted cylinder and
//! a bent tube straighten into straight bands of their diameter, their cross-sections are
//! discs, and the GPU draws the columns as the CPU samples them.

use specular_dicom::volume::{Geometry, Volume, VoxelKind};
use specular_render::VoiShape;

use super::*;
use crate::reference;

const WIDE: Window = Window {
    center: 500.0,
    width: 1000.0,
    shape: VoiShape::Linear,
    invert: false,
};

/// A volume of `f` at the voxel centres of axial slices centred on the origin.
pub(crate) fn volume_of(dims: [u32; 3], pixel: f64, spacing: f64, f: impl Fn(V3) -> f32) -> Volume {
    let half = |n: u32, s: f64| (f64::from(n) - 1.0) * s / 2.0;
    let pos: Vec<V3> = (0..dims[2])
        .map(|k| {
            [
                -half(dims[0], pixel),
                -half(dims[1], pixel),
                f64::from(k) * spacing - half(dims[2], spacing),
            ]
        })
        .collect();
    let g = Geometry::new(
        &pos,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [pixel, pixel],
        [dims[0], dims[1]],
    );
    let mut v = Volume::new(g, VoxelKind::I16, 1).expect("small volume");
    let mut buf = vec![0f32; (dims[0] * dims[1]) as usize];
    for k in 0..dims[2] as usize {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                let p = v
                    .geometry
                    .to_patient([f64::from(i), f64::from(j), k as f64]);
                buf[(j * dims[0] + i) as usize] = f(p);
            }
        }
        v.put(k, &buf, dims[0], dims[1]).expect("slice fits");
    }
    v
}

fn dist_to_line(p: V3, a: V3, d: V3) -> f64 {
    let q = sub(p, a);
    let t = dot(q, d);
    len(sub(q, d.map(|c| c * t)))
}

#[test]
fn passes_through_its_points() {
    let pts = [
        [0.0, 0.0, 0.0],
        [10.0, 3.0, 1.0],
        [25.0, -4.0, 6.0],
        [31.0, 9.0, 2.0],
    ];
    let s = Spline::new(&pts).unwrap();
    for (i, p) in pts.iter().enumerate() {
        let q = s.at(s.knot(i));
        assert!(len(sub(*p, q)) < 1e-9, "point {i}: {q:?}");
    }
}

#[test]
fn collinear_points_give_their_distance() {
    // Uneven spacing: centripetal knots do not overshoot, so the curve is the segment.
    let d = unit([1.0, 2.0, -0.5]).unwrap();
    let pts: Vec<V3> = [0.0, 3.0, 4.0, 20.0, 21.0, 40.0]
        .iter()
        .map(|&t| d.map(|c| c * t))
        .collect();
    let s = Spline::new(&pts).unwrap();
    assert!((s.length() - 40.0).abs() < 1e-6, "{}", s.length());
    for p in s.samples() {
        assert!(dist_to_line(*p, [0.0; 3], d) < 1e-9);
    }
}

#[test]
fn arc_length_of_a_sampled_circle() {
    // 13 points over a half circle of radius 40: π·40 within 0.2 %.
    let r = 40.0;
    let pts: Vec<V3> = (0..13)
        .map(|i| {
            let a = std::f64::consts::PI * f64::from(i) / 12.0;
            [r * a.cos(), r * a.sin(), 5.0]
        })
        .collect();
    let s = Spline::new(&pts).unwrap();
    let want = std::f64::consts::PI * r;
    assert!(
        (s.length() - want).abs() / want < 0.002,
        "{} vs {want}",
        s.length()
    );
    // Every sample stays near the circle.
    for p in s.samples() {
        let e = (p[0].hypot(p[1]) - r).abs();
        // The end segments continue straight, so they bow out a little more.
        let (at, _) = s.closest(*p);
        let inner = at > s.knot(1) && at < s.knot(11);
        let limit = if inner { 0.05 } else { 0.3 };
        assert!(e < limit, "{p:?} is {e} mm off");
    }
    // The closest point of a point on the circle is itself.
    let p = [r * 0.3f64.cos(), r * 0.3f64.sin(), 5.0];
    let (at, d) = s.closest(p);
    assert!(d < 0.05 && (at - 0.3 * r).abs() < 0.1, "{at} {d}");
}

#[test]
fn degenerate_curves_are_refused() {
    assert!(Spline::new(&[]).is_none());
    assert!(Spline::new(&[[1.0, 2.0, 3.0]]).is_none());
    assert!(Spline::new(&[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]).is_none());
    assert!(Spline::new(&[[0.0; 3], [f64::NAN, 0.0, 0.0]]).is_none());
}

#[test]
fn frames_are_orthonormal_and_follow_up() {
    let pts = [[0.0, 0.0, 0.0], [20.0, 10.0, 0.0], [40.0, 0.0, 0.0]];
    let s = Spline::new(&pts).unwrap();
    let at: Vec<f64> = (0..20).map(|i| s.length() * f64::from(i) / 19.0).collect();
    for f in s.frames([0.0, 0.0, 1.0], 0.0, &at) {
        assert!(dot(f.tangent, f.down).abs() < 1e-9);
        assert!((len(f.down) - 1.0).abs() < 1e-9);
        // In the axial plane: down is inferior (−z).
        assert!((f.down[2] + 1.0).abs() < 1e-9, "{:?}", f.down);
        assert!(dot(f.side, f.tangent).abs() < 1e-9 && dot(f.side, f.down).abs() < 1e-9);
    }
    // A curve running along up keeps its frame from the previous one.
    let s = Spline::new(&[[0.0; 3], [0.0, 0.0, 30.0], [10.0, 0.0, 60.0]]).unwrap();
    let at: Vec<f64> = (0..64).map(|i| s.length() * f64::from(i) / 63.0).collect();
    let fs = s.frames([0.0, 0.0, 1.0], 0.0, &at);
    for w in fs.windows(2) {
        assert!(dot(w[0].down, w[1].down) > 0.9, "flipped: {w:?}");
    }
}

/// Rows of column `x` of `cpr` at or above half the phantom's value.
fn band(vol: &Volume, cpr: &Cpr, planes: &[Plane], x: u32) -> Vec<u32> {
    let px = f64::from(cpr.rect[0] + x) + 0.5;
    let plane = planes
        .iter()
        .find(|p| p.rect[0] <= cpr.rect[0] + x && cpr.rect[0] + x < p.rect[0] + p.rect[2])
        .expect("a plane per column");
    (0..cpr.rect[3])
        .filter(|&y| {
            reference(vol, plane, px, f64::from(cpr.rect[1] + y) + 0.5).is_some_and(|v| v >= 500.0)
        })
        .collect()
}

/// Every column inside the curve shows one band `2r` wide centred on the middle row, within
/// the voxel size `voxel` (the band's edges are where trilinear sampling crosses half), and
/// every pixel's patient point lies `|y − cy|` mm from the centreline `axis_distance`
/// measures, exactly (the resampling itself, without the voxels).
fn assert_straight_band(
    vol: &Volume,
    cpr: &Cpr,
    r: f64,
    margin: u32,
    voxel: f64,
    axis_distance: &dyn Fn(V3) -> f64,
) {
    let planes = cpr.planes(0.0, Slab::Average, WIDE, [0.0; 4]);
    let mut checked = 0;
    let want = 2.0 * r / cpr.mm;
    let slack = voxel / cpr.mm;
    let n = cpr.columns.len() as u32;
    for x in margin..n - margin {
        if cpr.columns[x as usize].is_none() {
            continue;
        }
        let rows = band(vol, cpr, &planes, x);
        assert!(!rows.is_empty(), "column {x} has no band");
        let (lo, hi) = (rows[0], *rows.last().unwrap());
        assert_eq!(rows.len() as u32, hi - lo + 1, "column {x}: band broken");
        let width = f64::from(hi - lo + 1);
        assert!(
            (width - want).abs() <= slack + 1.5,
            "column {x}: {width} rows, want {want:.1}"
        );
        let mid = (f64::from(lo) + f64::from(hi) + 1.0) / 2.0 + f64::from(cpr.rect[1]);
        assert!(
            (mid - cpr.cy).abs() <= slack / 2.0 + 1.0,
            "column {x}: centre {mid}, want {}",
            cpr.cy
        );
        let px = f64::from(cpr.rect[0] + x) + 0.5;
        for y in 0..cpr.rect[3] {
            let py = f64::from(cpr.rect[1] + y) + 0.5;
            let p = cpr.to_patient(px, py).unwrap();
            let e = axis_distance(p) - (py - cpr.cy).abs() * cpr.mm;
            if (py - cpr.cy).abs() * cpr.mm <= 2.0 * r {
                assert!(e.abs() < 0.05, "column {x} row {y}: {e} mm off");
            }
        }
        checked += 1;
    }
    assert!(checked > n / 2, "{checked} of {n} columns checked");
}

#[test]
fn a_tilted_cylinder_straightens_into_a_straight_band() {
    let d = unit([0.5, 0.35, 1.0]).unwrap();
    let r = 6.0;
    let vol = volume_of([64, 64, 60], 1.5, 1.5, |p| {
        if dist_to_line(p, [0.0; 3], d) <= r {
            1000.0
        } else {
            0.0
        }
    });
    // Unevenly spaced points along the axis.
    let pts: Vec<V3> = [-30.0, -18.0, -2.0, 5.0, 28.0]
        .iter()
        .map(|&t| d.map(|c| c * t))
        .collect();
    let s = Spline::new(&pts).unwrap();
    assert!((s.length() - 58.0).abs() < 1e-6);
    for kind in [CprKind::Straightened, CprKind::Stretched] {
        let rect = [7, 3, 160, 60];
        let mut view = CprView::fit(&s, [0.0, 0.0, 1.0], kind, rect);
        view.mm = 0.5;
        view.along = match kind {
            CprKind::Straightened => s.length() / 2.0,
            CprKind::Stretched => view.along,
        };
        let cpr = Cpr::new(&s, &view, rect);
        match kind {
            // The straightened image is isometric: a straight cylinder stays a band.
            CprKind::Straightened => {
                assert_straight_band(&vol, &cpr, r, 4, 1.5, &|p| dist_to_line(p, [0.0; 3], d))
            }
            // Stretched along z: the band runs diagonally (the curve's own height), its
            // vertical extent 2r / sin(angle to z).
            CprKind::Stretched => {
                let planes = cpr.planes(0.0, Slab::Average, WIDE, [0.0; 4]);
                let sin = (1.0 - d[2] * d[2]).sqrt();
                let cos = d[2];
                let x = cpr.columns.len() as u32 / 2;
                let rows = band(&vol, &cpr, &planes, x);
                // Vertical chord through a cylinder of radius r tilted θ from the
                // vertical: 2r / sin θ.
                let chord = 2.0 * r / sin;
                let got = rows.len() as f64 * cpr.mm;
                assert!(
                    (got - chord).abs() <= 2.0 * cpr.mm + 0.05 * chord,
                    "stretched chord {got} vs {chord} (cos {cos})"
                );
            }
        }
    }
}

#[test]
fn a_bent_tube_straightens_into_a_straight_band() {
    // A quarter torus in the axial plane: centre radius 40, tube radius 5.
    let (big, r) = (40.0, 5.0);
    let vol = volume_of([80, 80, 24], 1.25, 1.25, |p| {
        let rho = p[0].hypot(p[1]) - big;
        let inside = p[0] >= -1.0 && p[1] >= -1.0;
        if inside && rho.hypot(p[2]) <= r {
            1000.0
        } else {
            0.0
        }
    });
    let pts: Vec<V3> = (0..6)
        .map(|i| {
            let a = std::f64::consts::FRAC_PI_2 * (0.05 + 0.9 * f64::from(i) / 5.0);
            [big * a.cos(), big * a.sin(), 0.0]
        })
        .collect();
    let s = Spline::new(&pts).unwrap();
    // The distance to the spline itself (it bows off the circle by up to 0.2 mm).
    let torus = |p: V3| s.closest(p).1;
    let rect = [0, 0, 140, 50];
    // Up along z: the tube lies in the axial plane, so straightened and stretched agree.
    for kind in [CprKind::Straightened, CprKind::Stretched] {
        let mut view = CprView::fit(&s, [0.0, 0.0, 1.0], kind, rect);
        view.mm = 0.45;
        let cpr = Cpr::new(&s, &view, rect);
        assert_straight_band(&vol, &cpr, r, 4, 1.25, &torus);
    }
    // Turned 90° about the curve, the band is the tube's in-plane width, also 2r.
    let mut view = CprView::fit(&s, [0.0, 0.0, 1.0], CprKind::Straightened, rect);
    view.mm = 0.45;
    view.angle = std::f64::consts::FRAC_PI_2;
    let cpr = Cpr::new(&s, &view, rect);
    assert_straight_band(&vol, &cpr, r, 4, 1.25, &torus);
}

#[test]
fn image_and_patient_points_round_trip() {
    let pts = [
        [0.0, 0.0, 0.0],
        [20.0, 10.0, 3.0],
        [40.0, 0.0, 8.0],
        [55.0, -12.0, 4.0],
    ];
    let s = Spline::new(&pts).unwrap();
    let rect = [10, 20, 200, 90];
    for kind in [CprKind::Straightened, CprKind::Stretched] {
        let mut view = CprView::fit(&s, [0.0, 0.0, 1.0], kind, rect);
        view.offset = 3.0;
        let cpr = Cpr::new(&s, &view, rect);
        for (x, y) in [(40.5, 30.5), (110.5, 64.5), (180.5, 70.5)] {
            let Some(p) = cpr.to_patient(x, y) else {
                continue;
            };
            // The point's nearest curve point may lie in another column for points far
            // off the curve: only near points (within 8 mm) are checked.
            if s.closest(p).1 > 8.0 {
                continue;
            }
            let (x2, y2) = cpr.to_image(&s, p).unwrap();
            assert!(
                (x2 - x).abs() <= 2.5 && (y2 - y).abs() <= 1.0,
                "{kind:?}: ({x}, {y}) → ({x2}, {y2})"
            );
        }
        // The curve itself shows on its column at the middle row moved by the offset
        // (straightened), or at its own height (stretched).
        let p = s.at(s.length() / 3.0);
        let (_, y) = cpr.to_image(&s, p).unwrap();
        if kind == CprKind::Straightened {
            assert!((y - (cpr.cy + 3.0 / cpr.mm)).abs() < 0.5, "{y}");
        }
    }
}

#[test]
fn cross_sections_of_the_tilted_cylinder_are_discs() {
    let d = unit([0.5, 0.35, 1.0]).unwrap();
    let r = 6.0;
    let vol = volume_of([64, 64, 60], 1.5, 1.5, |p| {
        if dist_to_line(p, [0.0; 3], d) <= r {
            1000.0
        } else {
            0.0
        }
    });
    let pts: Vec<V3> = [-30.0, -10.0, 12.0, 28.0]
        .iter()
        .map(|&t| d.map(|c| c * t))
        .collect();
    let s = Spline::new(&pts).unwrap();
    let mm = 0.25;
    for at in [10.0, 29.0, 45.0] {
        let plane = cross_section(&s, [0.0, 0.0, 1.0], 0.0, at, [0, 0, 80, 80], mm, WIDE);
        assert!(dot(plane.normal(), s.tangent(at)) > 1.0 - 1e-9);
        let mut n = 0;
        for y in 0..80 {
            for x in 0..80 {
                if reference(&vol, &plane, f64::from(x) + 0.5, f64::from(y) + 0.5)
                    .is_some_and(|v| v >= 500.0)
                {
                    n += 1;
                }
            }
        }
        let area = f64::from(n) * mm * mm;
        let want = std::f64::consts::PI * r * r;
        assert!(
            (area - want).abs() / want < 0.08,
            "at {at}: {area} vs {want}"
        );
    }
}

#[test]
fn columns_past_the_ends_show_the_canvas() {
    let s = Spline::new(&[[0.0; 3], [10.0, 0.0, 0.0]]).unwrap();
    let rect = [0, 0, 100, 10];
    let view = CprView {
        kind: CprKind::Straightened,
        up: [0.0, 0.0, 1.0],
        angle: 0.0,
        mm: 0.2,
        along: 5.0,
        offset: 0.0,
    };
    let cpr = Cpr::new(&s, &view, rect);
    let shown = cpr.columns.iter().filter(|c| c.is_some()).count();
    assert_eq!(shown, 50);
    let planes = cpr.planes(0.0, Slab::Average, WIDE, [0.0; 4]);
    // 50 columns and the two runs either side.
    assert_eq!(planes.len(), 52);
    let covered: u32 = planes.iter().map(|p| p.rect[2]).sum();
    assert_eq!(covered, 100);
}

#[test]
fn gpu_columns_match_the_cpu() {
    use specular_shots::Harness;
    let Some(mut h) = Harness::new() else { return };
    let d = unit([0.5, 0.35, 1.0]).unwrap();
    let vol = volume_of([48, 48, 40], 1.5, 1.5, |p| {
        let e = dist_to_line(p, [0.0; 3], d);
        (1000.0 * (-e * e / 60.0).exp()) as f32
    });
    let pts: Vec<V3> = [-24.0, -8.0, 9.0, 24.0]
        .iter()
        .map(|&t| d.map(|c| c * t))
        .collect();
    let s = Spline::new(&pts).unwrap();
    let size = (96u32, 48u32);
    let rect = [0, 0, size.0, size.1];
    let mut view = CprView::fit(&s, [1.0, 0.0, 0.0], CprKind::Straightened, rect);
    view.mm = 0.6;
    let cpr = Cpr::new(&s, &view, rect);
    for (thickness, slab) in [(0.0, Slab::Average), (6.0, Slab::Mip)] {
        let planes = cpr.planes(thickness, slab, WIDE, [0.0, 0.0, 0.0, 1.0]);
        let mut tex = crate::VolumeTexture::new(&h.gpu, &vol).expect("fits");
        tex.sync(&h.gpu, &vol).unwrap();
        let mut r = crate::MprRenderer::new();
        let shot = h.render_gpu(
            (size.0 as f32, size.1 as f32),
            1.0,
            |gpu, enc, target, sz| r.draw(gpu, enc, target, sz, &tex, &planes),
            |_, _| {},
        );
        let mut worst = 0f64;
        for (x, c) in cpr.columns.iter().enumerate() {
            if c.is_none() {
                continue;
            }
            let p = &planes.iter().find(|p| p.rect[0] == x as u32).unwrap();
            for y in 0..size.1 {
                let Some(v) = reference(&vol, p, x as f64 + 0.5, f64::from(y) + 0.5) else {
                    continue;
                };
                let want = srgb8(lin(WIDE.grey(f64::from(v))));
                let got = f64::from(shot.rgb[(y as usize * size.0 as usize + x) * 3]);
                worst = worst.max((got - want).abs());
            }
        }
        assert!(worst <= 2.5, "worst {worst} (thickness {thickness})");
    }
}

fn lin(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb8(linear: f64) -> f64 {
    let c = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    c * 255.0
}
