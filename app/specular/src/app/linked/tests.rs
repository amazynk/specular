//! Linked views on synthetic series (headers only, no pixels): the layouts, synced scrolling
//! across thicknesses, tilt, gaps and Frames of Reference, manual alignment, reference
//! lines and the 3D cursor on screen, window and zoom sync, and dragging series onto
//! viewports.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use specular_dicom::linked::{Fit, Link};
use specular_dicom::series::{Instance, Series, assemble};
use specular_gfx::Rect;
use specular_i18n::t;
use specular_render::{VoiShape, Window as WlWindow};
use specular_widgets::Button;

use super::draw::{self, GRIDS, Grid, layout};
use super::*;

const AXIAL: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
const SAGITTAL: [f64; 6] = [0.0, 1.0, 0.0, 0.0, 0.0, -1.0];

struct Spec<'a> {
    study: &'a str,
    uid: &'a str,
    frame: &'a str,
    modality: &'a str,
    number: i64,
    orientation: [f64; 6],
    /// Columns, rows; pixel spacing (both ways); thickness.
    size: [u32; 2],
    pixel: f64,
    thickness: f64,
}

fn series(s: &Spec, origins: &[[f64; 3]]) -> Series<PathBuf> {
    let found: Vec<(PathBuf, Instance)> = origins
        .iter()
        .enumerate()
        .map(|(k, o)| {
            (
                PathBuf::from(format!("/linked/{}/{k}", s.uid)),
                Instance {
                    study_uid: s.study.into(),
                    series_uid: s.uid.into(),
                    sop_uid: format!("{}.{k}", s.uid),
                    sop_class: "1.2.840.10008.5.1.4.1.1.2".into(),
                    modality: s.modality.into(),
                    series_number: Some(s.number),
                    series_description: format!("Series {}", s.number),
                    position: Some(*o),
                    orientation: Some(s.orientation),
                    frames: 1,
                    frame_of_reference: s.frame.into(),
                    rows: s.size[1],
                    columns: s.size[0],
                    pixel_spacing: Some([s.pixel, s.pixel]),
                    slice_thickness: Some(s.thickness),
                    ..Instance::default()
                },
            )
        })
        .collect();
    let mut v = assemble(found);
    assert_eq!(v.len(), 1);
    v.remove(0)
}

fn spec<'a>(uid: &'a str, frame: &'a str, modality: &'a str, number: i64) -> Spec<'a> {
    Spec {
        study: "1.9",
        uid,
        frame,
        modality,
        number,
        orientation: AXIAL,
        size: [256, 256],
        pixel: 1.0,
        thickness: 1.0,
    }
}

/// Thin axial CT, 1 mm at z = 0…99, 256 px of 1 mm centred on the axis.
pub(super) fn thin() -> Entry {
    let o: Vec<[f64; 3]> = (0..100).map(|k| [-127.5, -127.5, f64::from(k)]).collect();
    Entry::new(series(&spec("1.9.1", "FOR-A", "CT", 1), &o)).unwrap()
}

/// Thick axial CT of the same frame: 5 mm centred on 2.5…97.5, 512 px of 0.5 mm; `skip`
/// slices left out.
pub(super) fn thick(skip: &[usize]) -> Entry {
    let o: Vec<[f64; 3]> = (0..20)
        .filter(|k| !skip.contains(k))
        .map(|k| [-127.75, -127.75, 2.5 + 5.0 * k as f64])
        .collect();
    let s = Spec {
        size: [512, 512],
        pixel: 0.5,
        thickness: 5.0,
        ..spec("1.9.2", "FOR-A", "CT", 2)
    };
    Entry::new(series(&s, &o)).unwrap()
}

/// Sagittal MR of the same frame: x = −126…126 every 4 mm, 256 × 200 px of 1 mm, rows down
/// from z = 100.
pub(super) fn sagittal() -> Entry {
    let o: Vec<[f64; 3]> = (0..64)
        .map(|k| [-126.0 + 4.0 * f64::from(k), -127.5, 100.0])
        .collect();
    let s = Spec {
        orientation: SAGITTAL,
        size: [256, 200],
        thickness: 4.0,
        ..spec("1.9.3", "FOR-A", "MR", 3)
    };
    Entry::new(series(&s, &o)).unwrap()
}

/// A prior: another study and frame, the same anatomy 37.25 mm further along z.
pub(super) fn prior() -> Entry {
    let o: Vec<[f64; 3]> = (0..40)
        .map(|k| [-127.5, -127.5, 37.25 + 2.5 * f64::from(k)])
        .collect();
    let s = Spec {
        study: "2.7",
        thickness: 2.5,
        ..spec("2.7.1", "FOR-B", "CT", 1)
    };
    Entry::new(series(&s, &o)).unwrap()
}

fn session(entries: Vec<Entry>) -> Session {
    let mut s = Session::new(entries, 0, load::Loader::start(Arc::new(|| {})));
    s.layout = Some(layout(
        [0.0, 0.0, 1000.0, 700.0],
        1.0,
        false,
        s.grid,
        34.0,
        32.0,
    ));
    s
}

fn none() -> HashMap<(String, String), Link> {
    HashMap::new()
}

#[test]
fn layouts_split_the_area_and_mirror_in_rtl() {
    let l = layout(
        [0.0, 0.0, 900.0, 640.0],
        1.0,
        false,
        Grid::new(3, 3),
        34.0,
        32.0,
    );
    assert_eq!(l.bar, [0.0, 0.0, 900.0, 34.0]);
    assert_eq!(l.strip, Some([0.0, 34.0, 900.0, 32.0]));
    assert_eq!(l.ports.len(), 9);
    // (900 − 2) / 3 = 299 wide, (574 − 2) / 3 = 190 tall; the last column and row take the
    // rest.
    assert_eq!(l.ports[0], [0.0, 66.0, 299.0, 190.0]);
    assert_eq!(l.ports[1], [300.0, 66.0, 299.0, 190.0]);
    assert_eq!(l.ports[2], [600.0, 66.0, 300.0, 190.0]);
    assert_eq!(l.ports[8], [600.0, 448.0, 300.0, 192.0]);
    let r = layout(
        [0.0, 0.0, 900.0, 640.0],
        1.0,
        true,
        Grid::new(3, 3),
        34.0,
        32.0,
    );
    assert_eq!(r.ports[0], [600.0, 66.0, 300.0, 190.0]);
    assert_eq!(r.ports[2], [0.0, 66.0, 299.0, 190.0]);
    // At 1.5× everything is on whole physical pixels.
    let l = layout(
        [0.0, 0.0, 801.0, 601.0],
        1.5,
        false,
        Grid::new(2, 2),
        34.0,
        0.0,
    );
    assert_eq!(l.strip, None);
    for p in &l.ports {
        for v in p {
            assert!(((v * 1.5) - (v * 1.5).round()).abs() < 1e-9, "{p:?}");
        }
    }
    // Every grid up to 3 × 3, once.
    let mut all: Vec<(u8, u8)> = GRIDS.iter().map(|g| (g.cols, g.rows)).collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 9);
    assert_eq!(draw::grid_for(2), Grid::new(2, 1));
    assert_eq!(draw::grid_for(4), Grid::new(2, 2));
    assert_eq!(draw::grid_for(9), Grid::new(3, 3));
}

#[test]
fn scrolls_together_by_position_across_thicknesses() {
    let mut s = session(vec![thin(), thick(&[])]);
    assert_eq!((s.ports[0].set, s.ports[1].set), (Some(0), Some(1)));
    assert_eq!(s.link(0, 1, &none()), Some(Link::Frame));
    // Thin z = 12 is in thick slice 2 (10…15), not slice 12.
    s.go_to(0, 12, &none());
    assert_eq!(s.ports[1].slice, 2);
    assert_eq!(s.ports[1].fit.map(|f| f.0), Some(Fit::Inside));
    // Stepping 4 thin slices (z = 16) moves to thick slice 3.
    s.step(0, 4, &none());
    assert_eq!(s.ports[1].slice, 3);
    // The other way: thick slice 10 (z = 52.5) brings thin to 52 or 53.
    s.active = 1;
    s.go_to(1, 10, &none());
    assert!(matches!(s.ports[0].slice, 52 | 53), "{}", s.ports[0].slice);
    // The first and last slices clamp.
    s.go_to(1, 0, &none());
    assert!(!s.step(1, -1, &none()));
    assert_eq!(s.ports[1].slice, 0);
}

#[test]
fn gaps_and_ends_are_flagged() {
    // Thick slices 6–8 (z 32.5–42.5) are missing.
    let mut s = session(vec![thin(), thick(&[6, 7, 8])]);
    s.go_to(0, 37, &none());
    let (fit, d) = s.ports[1].fit.unwrap();
    assert_eq!(fit, Fit::Gap);
    assert!((d.abs() - 9.5).abs() < 1e-9, "{d}");
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    assert_eq!(
        sc.ports[1].flag,
        Some(t!("linked-in-gap", distance = "9.5"))
    );
    assert_eq!(sc.ports[0].flag, None);
    // Back inside a slice: no flag.
    s.go_to(0, 50, &none());
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    assert_eq!(sc.ports[1].flag, None);
}

#[test]
fn another_frame_follows_once_aligned_and_the_alignment_is_kept() {
    let mut aligned = none();
    let mut s = session(vec![thin(), prior()]);
    assert_eq!(s.link(0, 1, &aligned), None);
    s.go_to(0, 10, &aligned);
    let sc = s.scene(&aligned, false, &|_| None).unwrap();
    assert_eq!(sc.ports[1].flag, Some(t!("linked-not-linked")));
    // The user scrolls the prior to the same anatomy (slice 4, z 47.25) and aligns.
    s.ports[1].slice = 4;
    assert_eq!(s.align(&mut aligned), 1);
    let key = ("1.9".to_owned(), "2.7".to_owned());
    match aligned.get(&key) {
        Some(Link::Offset(d)) => assert!((d[2] - 37.25).abs() < 1e-9, "{d:?}"),
        l => panic!("{l:?}"),
    }
    // Thin slice 30 now shows prior z 67.25 (slice 12), exactly.
    s.go_to(0, 30, &aligned);
    assert_eq!(s.ports[1].slice, 12);
    assert_eq!(s.ports[1].fit.map(|f| f.0), Some(Fit::Inside));
    // Driven from the prior, the other way round.
    s.active = 1;
    s.go_to(1, 20, &aligned);
    assert_eq!(s.ports[0].slice, 50);
    // A new comparison of the same studies keeps the alignment (for the session).
    let mut s = session(vec![prior(), thin()]);
    s.go_to(0, 12, &aligned);
    assert_eq!(s.ports[1].slice, 30);
    // Aligning series of one frame does nothing.
    let mut s = session(vec![thin(), thick(&[])]);
    assert_eq!(s.align(&mut none()), 0);
}

#[test]
fn follow_modes() {
    let mut s = session(vec![thin(), prior()]);
    // By patient position, as stored: thin z 40 is prior slice 1 (z 39.75).
    s.ports[1].follow = Follow::Position;
    assert_eq!(s.link(0, 1, &none()), Some(Link::Position));
    s.go_to(0, 40, &none());
    assert_eq!(s.ports[1].slice, 1);
    // Off: nothing follows, even the same frame.
    let mut s = session(vec![thin(), thick(&[])]);
    s.ports[1].follow = Follow::Off;
    let before = s.ports[1].slice;
    s.go_to(0, 12, &none());
    assert_eq!(s.ports[1].slice, before);
    assert_ne!(before, 2);
    assert_eq!(s.link(0, 1, &none()), None);
}

#[test]
fn reference_lines_and_cursor_on_screen() {
    let mut s = session(vec![thick(&[]), sagittal()]);
    // Axial and sagittal cross: no scrolling together, a reference line instead.
    s.go_to(0, 4, &none()); // z 22.5
    assert!(s.ports[1].crossing);
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    assert_eq!(sc.ports[1].flag, None);
    let m = s.mapping(1).unwrap();
    assert_eq!(sc.ports[1].refs.len(), 1);
    // z = 22.5 is row 77.5 of the sagittal, across the axial's whole width (y −128…128).
    let l = sc.ports[1].refs[0];
    let y = m.to_screen(0.0, 77.5).1 as f32;
    assert!(
        (l[0].1 - y).abs() < 1e-3 && (l[1].1 - y).abs() < 1e-3,
        "{l:?} {y}"
    );
    let (x0, x1) = (
        m.to_screen(-0.5, 0.0).0 as f32,
        m.to_screen(255.5, 0.0).0 as f32,
    );
    let (a, b) = (l[0].0.min(l[1].0), l[0].0.max(l[1].0));
    assert!((a - x0).abs() < 1e-3 && (b - x1).abs() < 1e-3, "{l:?}");
    // The 5 mm slab's faces at rows 80 and 75.
    assert_eq!(sc.ports[1].faces.len(), 2);
    let fy: Vec<f32> = sc.ports[1].faces.iter().map(|f| f[0].1).collect();
    let want = [
        m.to_screen(0.0, 80.0).1 as f32,
        m.to_screen(0.0, 75.0).1 as f32,
    ];
    assert!(
        (fy[0] - want[0]).abs() < 1e-3 && (fy[1] - want[1]).abs() < 1e-3,
        "{fy:?} {want:?}"
    );
    // None on the active viewport; none at all with the lines off.
    assert!(sc.ports[0].refs.is_empty());
    s.lines = false;
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    assert!(sc.ports[1].refs.is_empty());
    // The 3D cursor: Shift+click at axial pixel (400, 128) is x = 72.25; the sagittal goes
    // to the slice at x = 74 and marks the point there.
    let m0 = s.mapping(0).unwrap();
    let at = m0.to_screen(400.0, 128.0);
    assert_eq!(s.press(at, Button::Primary, true, &none()), None);
    // (Ordered along the sagittal normal, −x: the slice at x = 74 is index 13.)
    let x = s.plane(1).unwrap().origin[0];
    assert_eq!((s.ports[1].slice, x), (13, 74.0));
    assert_eq!(s.ports[1].fit.map(|f| f.0), Some(Fit::Inside));
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    let c0 = sc.ports[0].cursor.unwrap();
    assert!((f64::from(c0.0) - at.0).abs() < 1e-3 && (f64::from(c0.1) - at.1).abs() < 1e-3);
    // On the sagittal: column y + 127.5 = 63.75 (y −63.75), row 100 − 22.5 = 77.5.
    let m1 = s.mapping(1).unwrap();
    let want = m1.to_screen(63.75, 77.5);
    let c1 = sc.ports[1].cursor.unwrap();
    assert!((f64::from(c1.0) - want.0).abs() < 1e-3 && (f64::from(c1.1) - want.1).abs() < 1e-3);
    assert!(sc.cursor.is_some());
    // Orientation letters follow the planes: L/R and A/P on the axial, A/P and S/I on the
    // sagittal.
    let sc = s.scene(&none(), true, &|_| None).unwrap();
    let letters =
        |i: usize| -> Vec<String> { sc.ports[i].edges.iter().map(|e| e.text.clone()).collect() };
    assert_eq!(letters(0), ["R", "L", "A", "P"]);
    assert_eq!(letters(1), ["A", "P", "S", "I"]);
}

/// `entry` as a veterinary study (Anatomical Orientation Type QUADRUPED) of `part`.
pub(super) fn quadruped(make: fn() -> Entry, part: &str) -> Entry {
    let mut series = make().series;
    for it in &mut series.items {
        it.instance.anatomy = specular_dicom::Anatomy::of(Some("QUADRUPED"), Some(part));
    }
    Entry::new(series).unwrap()
}

#[test]
fn quadruped_studies_get_veterinary_letters() {
    let mut s = session(vec![
        quadruped(thin, "ABDOMEN"),
        quadruped(sagittal, "HEAD"),
    ]);
    let sc = s.scene(&none(), true, &|_| None).unwrap();
    let letters =
        |i: usize| -> Vec<String> { sc.ports[i].edges.iter().map(|e| e.text.clone()).collect() };
    // Axial trunk: Rt/Le and ventral/dorsal (not R/L, A/P).
    assert_eq!(letters(0), ["Rt", "Le", "V", "D"]);
    // Sagittal head: ventral/dorsal and rostral/caudal (not A/P, S/I).
    assert_eq!(letters(1), ["V", "D", "R", "Cd"]);
    // A human study beside it keeps L/R.
    s = session(vec![thin(), quadruped(sagittal, "ABDOMEN")]);
    let sc = s.scene(&none(), true, &|_| None).unwrap();
    assert_eq!(sc.ports[0].edges[0].text, "R");
    assert_eq!(sc.ports[1].edges[2].text, "Cr");
}

#[test]
fn window_sync_reaches_by_modality_or_all() {
    let mut s = session(vec![thin(), thick(&[]), sagittal()]);
    assert_eq!(s.visible(), 4);
    let w = WlWindow {
        center: 40.0,
        width: 400.0,
        shape: VoiShape::Linear,
        invert: false,
    };
    s.set_window(0, w);
    assert_eq!(s.ports[1].window, Some(w));
    assert_eq!(s.ports[2].window, None);
    s.wsync = WindowSync::All;
    let w2 = WlWindow { center: 300.0, ..w };
    s.set_window(0, w2);
    assert_eq!(s.ports[2].window, Some(w2));
    s.wsync = WindowSync::Off;
    let w3 = WlWindow {
        center: -600.0,
        width: 1500.0,
        ..w
    };
    s.set_window(0, w3);
    assert_eq!(s.ports[0].window, Some(w3));
    assert_eq!(s.ports[1].window, Some(w2));
    s.reset_window(0);
    assert_eq!(s.ports[0].window, None);
    assert_eq!(s.ports[1].window, Some(w2));
}

#[test]
fn zoom_sync_by_physical_size() {
    let mut s = session(vec![thin(), thick(&[])]);
    // Each fits its own viewport (499 and 500 px wide) with its 256 mm field.
    let (a, b) = (s.mapping(0).unwrap(), s.mapping(1).unwrap());
    assert!((a.zoom - 499.0 / 256.0).abs() < 1e-9 && (b.zoom - 500.0 / 256.0).abs() < 1e-9);
    s.ports[0].zoom = Some(3.0);
    assert!((s.mapping(1).unwrap().zoom - 3.0).abs() > 0.1);
    s.zsync = true;
    assert_eq!(s.mapping(1).unwrap().zoom, 3.0);
    // Panning one pans all, in millimetres.
    let r = s.layout.as_ref().unwrap().ports[0];
    let c = (r[0] + 100.0, r[1] + 100.0);
    s.press(c, Button::Middle, false, &none());
    assert_eq!(s.active, 0);
    s.motion((c.0 + 30.0, c.1 - 15.0), &none());
    s.release((c.0 + 30.0, c.1 - 15.0), &none());
    assert_eq!(s.ports[0].pan, (10.0, -5.0));
    assert_eq!(s.mapping(1).unwrap().pan, (10.0, -5.0));
    // Off again: each its own.
    s.zsync = false;
    assert_eq!(s.mapping(1).unwrap().pan, (0.0, 0.0));
    s.fit_zoom(0);
    assert_eq!(s.ports[0].zoom, None);
}

#[test]
fn series_dragged_onto_a_viewport() {
    let mut s = session(vec![thin(), thick(&[]), sagittal()]);
    s.grid = Grid::new(2, 1);
    s.layout = Some(layout(
        [0.0, 0.0, 1000.0, 700.0],
        1.0,
        false,
        s.grid,
        34.0,
        32.0,
    ));
    s.placed.chips = vec![
        (0, Rect::new(10.0, 40.0, 80.0, 24.0)),
        (2, Rect::new(100.0, 40.0, 80.0, 24.0)),
    ];
    // Drag the sagittal from the strip onto viewport 1.
    assert_eq!(
        s.press((120.0, 50.0), Button::Primary, false, &none()),
        None
    );
    s.motion((300.0, 200.0), &none());
    let r = s.layout.as_ref().unwrap().ports[1];
    let drop = (r[0] + 50.0, r[1] + 50.0);
    s.motion(drop, &none());
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    assert!(sc.ghost.is_some());
    assert_eq!(s.release(drop, &none()), None);
    assert_eq!(s.ports[1].set, Some(2));
    assert_eq!(s.active, 1);
    // A click (no drag) shows the series in the active viewport.
    s.press((20.0, 50.0), Button::Primary, false, &none());
    s.release((21.0, 51.0), &none());
    assert_eq!(s.ports[1].set, Some(0));
    // Dropped outside every viewport: nothing changes.
    s.press((120.0, 50.0), Button::Primary, false, &none());
    s.motion((500.0, 20.0), &none());
    s.release((500.0, 20.0), &none());
    assert_eq!(s.ports[1].set, Some(0));
    // A Studies browser series dropped on viewport 0 comes back to be read.
    s.drag = Some(Drag::Carry(Carry::Study(7), (0.0, 0.0), true));
    let r0 = s.layout.as_ref().unwrap().ports[0];
    assert_eq!(s.release((r0[0] + 5.0, r0[1] + 5.0), &none()), Some((7, 0)));
    // Bar buttons come back to the caller.
    s.placed.controls = vec![(Control::Align, Rect::new(600.0, 4.0, 90.0, 26.0))];
    assert_eq!(
        s.press((610.0, 10.0), Button::Primary, false, &none()),
        Some(Control::Align)
    );
}

#[test]
fn accessible_names_say_which_viewport_is_active() {
    let mut s = session(vec![thin(), thick(&[6, 7, 8])]);
    s.go_to(0, 37, &none());
    let sc = s.scene(&none(), false, &|_| None).unwrap();
    let a = draw::port_label(&sc, 0);
    let b = draw::port_label(&sc, 1);
    assert!(
        a.starts_with(&t!(
            "linked-viewport",
            n = 1i64,
            count = 2i64,
            active = "yes"
        )),
        "{a}"
    );
    assert!(
        b.starts_with(&t!(
            "linked-viewport",
            n = 2i64,
            count = 2i64,
            active = "no"
        )),
        "{b}"
    );
    assert!(b.contains(&t!("linked-in-gap", distance = "9.5")), "{b}");
    // The bar's settings: toggles carry their state, the others name it.
    let find = |c: Control| sc.controls.iter().find(|x| x.0 == c).cloned().unwrap();
    assert_eq!(find(Control::Lines).2, Some(true));
    assert_eq!(find(Control::Zoom).2, Some(false));
    assert_eq!(
        find(Control::Window).1,
        t!("linked-window-sync", mode = "modality")
    );
    assert_eq!(find(Control::Link).1, t!("linked-link", mode = "frame"));
}
