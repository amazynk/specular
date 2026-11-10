//! Linked views on synthetic series: slice choice by position (different thicknesses, gaps,
//! gantry tilt, manual links) and reference-line geometry (orthogonal, oblique, slabs),
//! checked by numbers worked out by hand.

use super::*;
use crate::parse::PIXEL_DATA;
use crate::series::{Instance, assemble};
use crate::tests::{W, file};

const AXIAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
/// Rows along patient y (posterior), columns head to feet.
const SAGITTAL: [f64; 6] = [0.0, 1.0, 0.0, 0.0, 0.0, -1.0];
const CORONAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 0.0, -1.0];

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn axial(z: f64, size: u32, thickness: f64) -> Plane {
    let h = f64::from(size) / 2.0;
    Plane::new(
        [-h + 0.5, -h + 0.5, z],
        AXIAL,
        [1.0, 1.0],
        [size, size],
        thickness,
    )
    .unwrap()
}

/// Thin axial slices 1 mm apart at z = 0…99, and thick ones 5 mm apart centred on 2.5…97.5.
fn thin_and_thick() -> (Stack, Stack) {
    let thin = Stack::from_planes(
        "1.9.1",
        "CT",
        (0..100).map(|k| axial(f64::from(k), 256, 1.0)).collect(),
    );
    let thick = Stack::from_planes(
        "1.9.1",
        "CT",
        (0..20)
            .map(|k| axial(2.5 + 5.0 * f64::from(k), 512, 5.0))
            .collect(),
    );
    (thin, thick)
}

#[test]
fn follows_by_position_across_thicknesses() {
    let (thin, thick) = thin_and_thick();
    assert_eq!(thin.spacing, 1.0);
    assert_eq!(thick.spacing, 5.0);
    // Thin z = 12 lies in the thick slice 10…15 (index 2, centred on 12.5).
    let p = thick.follow(&thin.planes[12], Link::Frame).unwrap();
    assert_eq!(p.slice, 2);
    assert!(close(p.distance, -0.5, 1e-9), "{p:?}");
    assert_eq!(p.fit, Fit::Inside);
    // By index it would have been slice 12 (z = 62.5): 50 mm off.
    assert!(close(thick.planes[12].origin[2] - 12.0, 50.5, 1e-9));
    // The boundary 15.0 goes to whichever is nearer; 15.2 to the next slice.
    let p = thick.follow(&axial(15.2, 256, 1.0), Link::Frame).unwrap();
    assert_eq!((p.slice, p.fit), (3, Fit::Inside));
    // The other way: thick slice 7 (z = 37.5) shows thin z = 37 or 38 (37.5 is between).
    let p = thin.follow(&thick.planes[7], Link::Frame).unwrap();
    assert!(p.slice == 37 || p.slice == 38);
    assert!(close(p.distance.abs(), 0.5, 1e-9));
    // Beyond the thick series' ends.
    let p = thick.follow(&axial(-3.0, 256, 1.0), Link::Frame).unwrap();
    assert_eq!((p.slice, p.fit), (0, Fit::Outside));
    assert!(close(p.distance, -5.5, 1e-9));
    let p = thick.follow(&axial(99.9, 256, 1.0), Link::Frame).unwrap();
    assert_eq!((p.slice, p.fit), (19, Fit::Inside));
}

#[test]
fn gaps_are_flagged() {
    // Thick series with slices 6, 7 and 8 (z = 32.5, 37.5, 42.5) missing.
    let planes: Vec<Plane> = (0..20)
        .filter(|k| !(6..=8).contains(k))
        .map(|k| axial(2.5 + 5.0 * f64::from(k), 512, 5.0))
        .collect();
    let st = Stack::from_planes("1.9.1", "CT", planes);
    assert_eq!(st.spacing, 5.0);
    // z = 30 is still in slice 5 (25…30 is index 5, z 27.5); 37 is 9.5 mm from 27.5 and 10.5
    // from 47.5: a gap.
    let p = st.follow(&axial(37.0, 256, 1.0), Link::Frame).unwrap();
    assert_eq!(p.fit, Fit::Gap);
    assert_eq!(p.slice, 5);
    assert!(close(p.distance, 9.5, 1e-9));
    assert_eq!(st.missing_between(5, 6), 3);
    assert_eq!(st.missing_between(4, 5), 0);
    let p = st.follow(&axial(29.9, 256, 1.0), Link::Frame).unwrap();
    assert_eq!(p.fit, Fit::Inside);
}

/// A CT with 20° gantry tilt: the table moves 2 mm along z per slice, the images' columns
/// lean (0, cos 20°, sin 20°), so slices are 2·cos 20° = 1.879 mm apart along their normal.
fn tilted() -> Stack {
    let t = 20f64.to_radians();
    let o = [1.0, 0.0, 0.0, 0.0, t.cos(), t.sin()];
    Stack::from_planes(
        "1.9.1",
        "CT",
        (0..60)
            .map(|k| {
                Plane::new(
                    [-99.5, -99.5, 2.0 * f64::from(k)],
                    o,
                    [1.0, 1.0],
                    [200, 200],
                    2.0,
                )
                .unwrap()
            })
            .collect(),
    )
}

#[test]
fn follows_a_tilted_stack_at_the_view_centre() {
    let st = tilted();
    let t = 20f64.to_radians();
    assert!(close(st.spacing, 2.0 * t.cos(), 1e-9));
    // Each slice's centre is 99.5 mm down its leaning columns: z = 2k + 99.5·sin 20° =
    // 2k + 34.03, y = −99.5 + 99.5·cos 20° = −6.0.
    let lift = 99.5 * t.sin();
    assert!(close(st.planes[13].centre()[2], 26.0 + lift, 1e-9));
    // An axial slice at z = 60 meets the tilted stack's centre line at slice 13
    // (2·13 + 34.03 = 60.03): measured where the views' centres are, not at the corner.
    let src = axial(60.0, 256, 1.0);
    let p = st.follow(&src, Link::Frame).unwrap();
    assert_eq!(p.slice, 13);
    assert_eq!(p.fit, Fit::Inside);
    // The point is on the source plane straight along slice 13's normal from its centre:
    // 0.03 mm above z = 60, so 0.03·cos 20° mm from the slice along its normal.
    assert!(
        close(p.distance, -(26.0 + lift - 60.0) / t.cos(), 1e-6),
        "{p:?}"
    );
    // Ignoring the tilt (nearest by z of the first pixel) would pick slice 30.
    let naive = (0..60)
        .min_by(|a, b| {
            let za = (st.planes[*a].origin[2] - 60.0).abs();
            let zb = (st.planes[*b].origin[2] - 60.0).abs();
            za.total_cmp(&zb)
        })
        .unwrap();
    assert_eq!(naive, 30);
}

#[test]
fn crossing_series_do_not_scroll_together() {
    let (thin, _) = thin_and_thick();
    let sag = Plane::new([0.0, -127.5, 99.0], SAGITTAL, [1.0, 1.0], [256, 100], 1.0).unwrap();
    assert!(close(angle_deg(&thin.planes[0], &sag), 90.0, 1e-9));
    assert_eq!(thin.follow(&sag, Link::Frame), None);
    // 30° apart still follows; 40° does not.
    let turn = |deg: f64| {
        let a = deg.to_radians();
        Plane::new(
            [-127.5, -127.5, 50.0],
            [1.0, 0.0, 0.0, 0.0, a.cos(), a.sin()],
            [1.0, 1.0],
            [256, 256],
            1.0,
        )
        .unwrap()
    };
    assert!(thin.follow(&turn(30.0), Link::Frame).is_some());
    assert!(thin.follow(&turn(40.0), Link::Frame).is_none());
}

#[test]
fn manual_alignment_offsets_along_the_normal() {
    let (thin, _) = thin_and_thick();
    // A prior: the same anatomy 37.25 mm further along z, shifted 20 mm sideways in x
    // (patient lay elsewhere on the table), in another Frame of Reference.
    let prior = Stack::from_planes(
        "1.9.2",
        "CT",
        (0..40)
            .map(|k| axial(37.25 + 2.5 * f64::from(k), 256, 2.5).shifted([20.0, 0.0, 0.0]))
            .collect(),
    );
    // The user aligns thin slice 10 with prior slice 4 (z 47.25): 37.25 mm along z only;
    // the sideways shift is in-plane and does not move slices.
    let link = Link::aligned(&thin.planes[10], &prior.planes[4]);
    let Link::Offset(d) = link else {
        panic!("{link:?}")
    };
    assert!(close(d[0], 0.0, 1e-9) && close(d[1], 0.0, 1e-9) && close(d[2], 37.25, 1e-9));
    // Thin slice 30 (z 30) then shows prior z 67.25 (index 12) exactly.
    let p = prior.follow(&thin.planes[30], link).unwrap();
    assert_eq!(p.slice, 12);
    assert!(close(p.distance, 0.0, 1e-9));
    // And back: prior slice 12 shows thin slice 30.
    let p = thin.follow(&prior.planes[12], link.inverse()).unwrap();
    assert_eq!(p.slice, 30);
    // Linked by position instead, thin z 30 would be before the prior's first slice.
    let p = prior.follow(&thin.planes[30], Link::Position).unwrap();
    assert_eq!((p.slice, p.fit), (0, Fit::Outside));
}

fn sagittal() -> Plane {
    // x = 0, columns along y from −127.5, rows down from z = 100: 256 × 200 pixels of 1 mm.
    Plane::new([0.0, -127.5, 100.0], SAGITTAL, [1.0, 1.0], [256, 200], 1.0).unwrap()
}

#[test]
fn reference_line_of_an_axial_slice_on_a_sagittal() {
    let dst = sagittal();
    let src = axial(10.0, 256, 1.0);
    let l = reference_line(&src, &dst, Link::Frame, None).unwrap();
    let c = l.centre.unwrap();
    // z = 10 is row 90 of the sagittal; the axial image spans y −128…128, the sagittal's
    // columns −0.5…255.5.
    let (a, b) = if c.a.0 < c.b.0 {
        (c.a, c.b)
    } else {
        (c.b, c.a)
    };
    assert!(close(a.0, -0.5, 1e-9) && close(a.1, 90.0, 1e-9), "{c:?}");
    assert!(close(b.0, 255.5, 1e-9) && close(b.1, 90.0, 1e-9), "{c:?}");
    // A 1 mm slice on 1 mm pixels draws no slab faces; a 5 mm slab does, 2.5 mm each side.
    assert_eq!(l.slab, [None, None]);
    let l = reference_line(&src, &dst, Link::Frame, Some(5.0)).unwrap();
    let rows: Vec<f64> = l.slab.iter().map(|s| s.unwrap().a.1).collect();
    assert!(
        close(rows[0], 92.5, 1e-9) && close(rows[1], 87.5, 1e-9),
        "{rows:?}"
    );
    // A smaller axial image (field of view y −64…64) gives a shorter line.
    let small = axial(10.0, 128, 1.0);
    let c = cut(&small, &dst, Link::Frame).unwrap();
    assert!(close(c.length(), 128.0, 1e-9), "{c:?}");
    // Parallel planes have none; an axial below the sagittal's bottom edge has none.
    assert_eq!(cut(&src, &axial(20.0, 256, 1.0), Link::Frame), None);
    assert_eq!(cut(&axial(-150.0, 256, 1.0), &dst, Link::Frame), None);
    // Through a manual link: the axial 10 mm higher in the other series' coordinates.
    let c = cut(&src, &dst, Link::Offset([0.0, 0.0, 10.0])).unwrap();
    assert!(close(c.a.1, 80.0, 1e-9));
}

#[test]
fn reference_line_of_an_oblique_slice() {
    let dst = sagittal();
    // An axial tilted 30° about x, centred on (0, 0, 10): its columns point
    // (0, cos 30°, sin 30°), so it meets the sagittal along z = 10 + y·tan 30°.
    let a = 30f64.to_radians();
    let o = [1.0, 0.0, 0.0, 0.0, a.cos(), a.sin()];
    let mut src = Plane::new([0.0; 3], o, [1.0, 1.0], [256, 256], 1.0).unwrap();
    let c0 = src.at(127.5, 127.5);
    src = src.shifted([-c0[0], -c0[1], 10.0 - c0[2]]);
    let c = cut(&src, &dst, Link::Frame).unwrap();
    // On the sagittal, column x = y + 127.5 and row = 100 − z.
    for p in [c.a, c.b] {
        let y = p.0 - 127.5;
        let z = 100.0 - p.1;
        assert!(close(z, 10.0 + y * a.tan(), 1e-9), "{p:?}");
    }
    // The oblique image is 256 mm tall along its columns: y spans ±128·cos 30° = ±110.85.
    let (lo, hi) = if c.a.0 < c.b.0 {
        (c.a.0, c.b.0)
    } else {
        (c.b.0, c.a.0)
    };
    assert!(close(lo, 127.5 - 128.0 * a.cos(), 1e-9), "{lo}");
    assert!(close(hi, 127.5 + 128.0 * a.cos(), 1e-9), "{hi}");
    // A coronal at y = 0 crosses the oblique along z = 10 (y = 0 there), on its row 90.
    let cor = Plane::new([-127.5, 0.0, 100.0], CORONAL, [1.0, 1.0], [256, 200], 1.0).unwrap();
    let c = cut(&src, &cor, Link::Frame).unwrap();
    assert!(
        close(c.a.1, 90.0, 1e-9) && close(c.b.1, 90.0, 1e-9),
        "{c:?}"
    );
    // 5° from parallel draws nothing (the line would run off far away).
    let b = 5f64.to_radians();
    let near = Plane::new(
        [0.0, -127.5, 100.0],
        [0.0, 1.0, 0.0, b.sin(), 0.0, -b.cos()],
        [1.0, 1.0],
        [256, 200],
        1.0,
    )
    .unwrap();
    assert!(angle_deg(&near, &dst) < LINE_MIN_DEG);
    assert_eq!(cut(&near, &dst, Link::Frame), None);
}

#[test]
fn cursor_lands_on_the_slice_and_pixel() {
    let (thin, thick) = thin_and_thick();
    // Shift+click at pixel (200, 64) of thin slice 42: patient (72.5, −63.5, 42).
    let p = thin.planes[42].at(200.0, 64.0);
    assert!(close(p[0], 72.5, 1e-9) && close(p[1], -63.5, 1e-9) && close(p[2], 42.0, 1e-9));
    // The thick series (512 px from −255.5): slice 8 (40…45), pixel (328, 192).
    let l = thick.locate(p).unwrap();
    assert_eq!(l.pick.slice, 8);
    assert!(
        close(l.pixel.0, 328.0, 1e-9) && close(l.pixel.1, 192.0, 1e-9),
        "{l:?}"
    );
    assert!(l.on_image);
    // In the tilted stack: the slice whose plane passes nearest, at the projected pixel.
    let st = tilted();
    let l = st.locate(p).unwrap();
    let pl = &st.planes[l.pick.slice];
    let (_, _, d) = pl.project(p);
    assert!(d.abs() <= st.spacing / 2.0 + 1e-9, "{d}");
    let back = pl.at(l.pixel.0, l.pixel.1);
    let off = [p[0] - back[0], p[1] - back[1], p[2] - back[2]];
    let n = pl.normal();
    // What is left is along the normal only.
    let along = off[0] * n[0] + off[1] * n[1] + off[2] * n[2];
    assert!(close(off[0], along * n[0], 1e-9) && close(off[1], along * n[1], 1e-9));
    // A sagittal series: the point's x picks the slice.
    let sag = Stack::from_planes(
        "1.9.1",
        "MR",
        (0..64)
            .map(|k| sagittal().shifted([-126.0 + 4.0 * f64::from(k), 0.0, 0.0]))
            .collect(),
    );
    let l = sag.locate(p).unwrap();
    // x = 72.5 lies between slices 49 (x 70) and 50 (x 74): 50, 1.5 mm away.
    assert_eq!(l.pick.slice, 50);
    assert!(close(l.pick.distance.abs(), 1.5, 1e-9));
    // Column y + 127.5 = 64, row 100 − z = 58.
    assert!(close(l.pixel.0, 64.0, 1e-9) && close(l.pixel.1, 58.0, 1e-9));
}

fn ds(v: &[f64]) -> String {
    v.iter()
        .map(|x| format!("{x}"))
        .collect::<Vec<_>>()
        .join("\\")
}

/// A header with Pixel Spacing and Slice Thickness, 3 × 4 pixels.
fn header(z: f64, sop: &str, thickness: &str) -> Instance {
    let mut w = W::new(true, false);
    w.s(0x0008_0016, b"UI", "1.2.840.10008.5.1.4.1.1.2")
        .s(0x0008_0018, b"UI", sop)
        .s(0x0008_0060, b"CS", "CT")
        .s(0x0018_0050, b"DS", thickness)
        .s(0x0020_000D, b"UI", "1.2")
        .s(0x0020_000E, b"UI", "1.2.3")
        .s(0x0020_0032, b"DS", &ds(&[-1.0, -1.5, z]))
        .s(0x0020_0037, b"DS", &ds(&AXIAL))
        .s(0x0020_0052, b"UI", "1.2.9")
        .us(0x0028_0002, &[1])
        .s(0x0028_0004, b"CS", "MONOCHROME2")
        .us(0x0028_0010, &[4])
        .us(0x0028_0011, &[3])
        .s(0x0028_0030, b"DS", "0.5\\0.75")
        .us(0x0028_0100, &[16])
        .us(0x0028_0101, &[16])
        .us(0x0028_0102, &[15])
        .us(0x0028_0103, &[0])
        .raw(PIXEL_DATA, b"OW", &[0u8; 24]);
    let f = file("1.2.840.10008.1.2.1", &w.out);
    Instance::read(&f, true).unwrap().unwrap()
}

#[test]
fn stack_from_headers() {
    let i = header(4.0, "1.1", "2.5");
    assert_eq!((i.rows, i.columns), (4, 3));
    assert_eq!(i.pixel_spacing, Some([0.5, 0.75]));
    assert_eq!(i.slice_thickness, Some(2.5));
    let found: Vec<(usize, Instance)> =
        [(0.0, "1.0", "2.5"), (8.0, "1.2", ""), (4.0, "1.1", "2.5")]
            .into_iter()
            .enumerate()
            .map(|(k, (z, sop, t))| (k, header(z, sop, t)))
            .collect();
    let s = assemble(found).remove(0);
    let st = Stack::of(&s).unwrap();
    assert_eq!(st.frame_of_reference, "1.2.9");
    assert_eq!(st.planes.len(), 3);
    assert_eq!(st.spacing, 4.0);
    // Display order by position; the file without a thickness takes the spacing.
    let z: Vec<f64> = st.planes.iter().map(|p| p.origin[2]).collect();
    assert_eq!(z, [0.0, 4.0, 8.0]);
    assert_eq!(st.planes[2].thickness, 4.0);
    assert_eq!(st.planes[0].thickness, 2.5);
    // Columns are 0.75 mm apart, rows 0.5 mm: the last pixel's centre.
    let p = st.planes[0].at(2.0, 3.0);
    assert!(close(p[0], 0.5, 1e-12) && close(p[1], 0.0, 1e-12));
    assert_eq!(st.planes[0].extent_mm(), (2.25, 2.0));
    // Each plane points back at its file.
    assert_eq!(st.source, [(0, 0), (1, 0), (2, 0)]);
    assert_eq!(s.items[st.source[1].0].key, 2);
}
