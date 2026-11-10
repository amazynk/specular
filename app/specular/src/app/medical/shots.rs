//! Screenshot tests of medical mode: a synthetic CT phantom (generated here) drawn by the
//! image renderer through the GPU window — the file's default, the lung and bone presets,
//! inverted — with the value readout at 1× and 1.5×, left-to-right and right-to-left.
//! The analysis overlays: a two-segment segmentation drawn over the phantom along its plane
//! (the same renderer as MPR), with a time-intensity chart of synthetic points.
//! Rendered offscreen on lavapipe and compared with the PNGs in `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular medical::shots`, then review and
//! commit the rewritten PNGs.

use specular_core::Orientation;
use specular_render::{ImageRenderer, PxRect, RenderOptions, View};
use specular_shots::{Goldens, Harness};

use super::tests::{Spec, dicom, frame_of, open};
use super::*;

const SIZE: (f32, f32) = (320.0, 240.0);
const N: u16 = 128;

/// A 128² axial CT phantom in HU: air, a soft-tissue body with a fat rim, two lungs with a
/// gradient, a contrast-filled vessel and a bony spine.
fn phantom() -> Spec<'static> {
    let n = i32::from(N);
    let inside = |x: f64, y: f64, cx: f64, cy: f64, rx: f64, ry: f64| {
        ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0
    };
    let mut v = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (f64::from(i) + 0.5, f64::from(j) + 0.5);
            let hu = if !inside(x, y, 64.0, 66.0, 58.0, 46.0) {
                -1000.0
            } else if !inside(x, y, 64.0, 66.0, 54.0, 42.0) {
                -100.0
            } else if inside(x, y, 40.0, 60.0, 17.0, 26.0) || inside(x, y, 88.0, 60.0, 17.0, 26.0) {
                -900.0 + 4.0 * (y - 34.0)
            } else if inside(x, y, 64.0, 94.0, 9.0, 9.0) {
                if inside(x, y, 64.0, 94.0, 5.0, 5.0) {
                    300.0
                } else {
                    1200.0
                }
            } else if inside(x, y, 64.0, 56.0, 7.0, 7.0) {
                180.0
            } else {
                40.0 + 0.5 * (x - 64.0)
            };
            v.push(hu.round() as i32 + 1024);
        }
    }
    Spec {
        width: N,
        height: N,
        modality: "CT",
        photometric: "MONOCHROME2",
        signed: true,
        bits_stored: 16,
        rescale: Some((1.0, -1024.0)),
        window: Some((40.0, 400.0)),
        function: None,
        frames: vec![v],
        extra: Vec::new(),
    }
}

#[test]
fn windowed_ct_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical",
    );
    let m = frame_of(&open(dicom(&phantom())));
    let values = m.values_f32().expect("grey");
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, u32::from(N), u32::from(N), None);
    renderer.set_values(&h.gpu, &mut img, &values);
    let default = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    let preset = |k: usize, invert: bool| {
        let (_, width, center) = CT_PRESETS[k];
        WlWindow {
            center,
            width,
            shape: VoiShape::Linear,
            invert,
        }
    };
    let cases = [
        ("medical-ct-default", default, false, 1.0, "C 40  W 400"),
        (
            "medical-ct-lung",
            preset(0, false),
            false,
            1.0,
            "C −600  W 1500",
        ),
        (
            "medical-ct-bone-inverted",
            preset(2, true),
            true,
            1.5,
            "C 400  W 1800 · inverted",
        ),
    ];
    for (name, w, rtl, scale, wl) in cases {
        assert!(img.set_window(Some(w)));
        let lines = ["\u{2212}45 HU".to_owned(), wl.to_owned()];
        let shot = h.render_gpu(
            SIZE,
            scale,
            |gpu, enc, target, size| {
                let view = View::fixed(
                    (u32::from(N), u32::from(N)),
                    Orientation::NORMAL,
                    PxRect {
                        x: 0.0,
                        y: 0.0,
                        w: size.0 as f32,
                        h: size.1 as f32,
                    },
                    size.1 as f32 / f32::from(N),
                    (f32::from(N) / 2.0, f32::from(N) / 2.0),
                );
                let opts = RenderOptions::default();
                renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
            },
            |dl, text| {
                draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), rtl, 1.0);
            },
        );
        goldens.check(&format!("{name}-{}x", scale), &shot);
    }
    goldens.finish();
}

/// The phantom with rows 0.5 mm apart and columns 1 mm apart (Pixel Spacing 0.5\1.0):
/// each stored pixel is shown half as tall as wide (the phantom, round in stored pixels,
/// comes out flattened). The aspect comes from the file's summary, as the viewer sets it.
#[test]
fn non_square_pixels_match_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-aspect",
    );
    let mut spec = phantom();
    spec.extra
        .push((0x0028_0030, *b"DS", b"0.5\\1.0 ".to_vec()));
    let session = open(dicom(&spec));
    let aspect = session.medical().expect("a DICOM image").pixel_aspect();
    assert_eq!(aspect, 0.5);
    let m = frame_of(&session);
    let values = m.values_f32().expect("grey");
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, u32::from(N), u32::from(N), None);
    renderer.set_values(&h.gpu, &mut img, &values);
    let w = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    assert!(img.set_window(Some(w)));
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let mut view = View::new();
            view.set_viewport(PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            });
            view.upscale_small = true;
            view.set_image(u32::from(N), u32::from(N), Orientation::NORMAL);
            view.set_pixel_aspect(aspect as f32);
            view.settle();
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |_, _| {},
    );
    goldens.check("medical-aspect-1x", &shot);
    goldens.finish();
}

/// A circular shutter (grey), an overlay plane (a box and a cross) and three measurements
/// over the phantom at 0.7 mm pixels: the shutter and overlay go through the GPU mask, the
/// measurements through `measure::draw`.
#[test]
fn shutters_overlays_and_measurements_match_goldens() {
    use super::measure::{self, Drawn, Tool};
    use specular_dicom::graphics::{OVERLAY, SHUTTER};
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-graphics",
    );
    let mut spec = phantom();
    spec.extra = vec![(0x0028_0030, *b"DS", super::tests::text("0.7\\0.7"))];
    let m = frame_of(&open(dicom(&spec)));
    let values = m.values_f32().expect("grey");
    let n = u32::from(N);
    let mut mask = vec![0u8; (n * n) as usize];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (f64::from(x) + 0.5 - 64.0, f64::from(y) + 0.5 - 66.0);
            let mut b = 0;
            if fx * fx + fy * fy > 60.0 * 60.0 {
                b |= SHUTTER;
            }
            let on_box = (8..=40).contains(&x) && (y == 4 || y == 12)
                || (4..=12).contains(&y) && (x == 8 || x == 40);
            let on_cross =
                (x == 112 && (100..=116).contains(&y)) || (y == 108 && (104..=120).contains(&x));
            if on_box || on_cross {
                b |= OVERLAY;
            }
            mask[(y * n + x) as usize] = b;
        }
    }
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let mut img = renderer.create_image(&h.gpu, 1, n, n, None);
    renderer.set_values(&h.gpu, &mut img, &values);
    renderer.set_graphics(
        &h.gpu,
        &mut img,
        Some(specular_render::ValueMask {
            mask: &mask,
            shutter: [0.25; 3],
            inks: &[[1.0; 3]],
        }),
    );
    assert!(img.has_graphics());
    let default = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    assert!(img.set_window(Some(default)));
    // Stored pixels to logical pixels at 1×: the 128² image fills the height, centred.
    let k = SIZE.1 / f32::from(N);
    let x0 = (SIZE.0 - SIZE.1) / 2.0;
    let to_screen = |p: &(f64, f64)| (x0 + p.0 as f32 * k, p.1 as f32 * k);
    let made = [
        (Tool::Length, vec![(57.0, 94.0), (71.0, 94.0)], false),
        (Tool::Ellipse, vec![(30.0, 50.0), (50.0, 74.0)], false),
        (
            Tool::Angle,
            vec![(100.0, 40.0), (86.0, 30.0), (100.0, 22.0)],
            false,
        ),
        (Tool::Rect, vec![(58.0, 50.0), (66.0, 60.0)], true),
    ];
    let items: Vec<Drawn> = made
        .iter()
        .map(|(tool, pts, draft)| {
            let (outline, closed) = measure::outline(*tool, pts);
            Drawn {
                outline: outline.iter().map(to_screen).collect(),
                closed,
                handles: pts.iter().map(to_screen).collect(),
                lines: if *draft {
                    Vec::new()
                } else {
                    measure::label(*tool, pts, &m, Some("HU"))
                },
                draft: *draft,
            }
        })
        .collect();
    for (name, overlays) in [
        ("medical-ct-graphics", true),
        ("medical-ct-overlays-hidden", false),
    ] {
        img.set_overlays(overlays);
        let shot = h.render_gpu(
            SIZE,
            1.0,
            |gpu, enc, target, size| {
                let view = View::fixed(
                    (n, n),
                    Orientation::NORMAL,
                    PxRect {
                        x: 0.0,
                        y: 0.0,
                        w: size.0 as f32,
                        h: size.1 as f32,
                    },
                    size.1 as f32 / f32::from(N),
                    (f32::from(N) / 2.0, f32::from(N) / 2.0),
                );
                let opts = RenderOptions::default();
                renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
            },
            |dl, text| {
                if overlays {
                    measure::draw(dl, text, &items, false, 1.0);
                }
            },
        );
        goldens.check(&format!("{name}-1x"), &shot);
    }
    goldens.finish();
}

/// The phantom's view: `scale` screen pixels per image pixel around stored `center`.
fn view_of(o: Orientation, size: (u32, u32), scale: f32, center: (f32, f32)) -> View {
    View::fixed(
        (u32::from(N), u32::from(N)),
        o,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: size.0 as f32,
            h: size.1 as f32,
        },
        scale,
        center,
    )
}

/// Orientation labels over the axial phantom (Image Orientation (Patient) 1\0\0\0\1\0):
/// as stored, and turned a quarter clockwise and mirrored while zoomed in and panned, where
/// they sit on the visible part of the image.
#[test]
fn orientation_labels_match_goldens() {
    use super::labels;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-labels",
    );
    let mut spec = phantom();
    spec.extra = vec![(0x0020_0037, *b"DS", super::tests::text("1\\0\\0\\0\\1\\0"))];
    let m = frame_of(&open(dicom(&spec)));
    let axes = *m.patient_axes().expect("patient axes");
    let values = m.values_f32().expect("grey");
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let n = u32::from(N);
    let mut img = renderer.create_image(&h.gpu, 1, n, n, None);
    renderer.set_values(&h.gpu, &mut img, &values);
    let default = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    assert!(img.set_window(Some(default)));
    let fit = SIZE.1 / f32::from(N);
    let cases = [
        ("medical-ct-labels", Orientation::NORMAL, fit, (64.0, 64.0)),
        (
            "medical-ct-labels-turned",
            Orientation::from_rotation_flip(1, true),
            fit * 2.5,
            (40.0, 50.0),
        ),
    ];
    for (name, o, scale, center) in cases {
        let size = (SIZE.0 as u32, SIZE.1 as u32);
        let view = view_of(o, size, scale, center);
        let edges = labels::edges(&axes, &view, 1.0);
        assert_eq!(edges.len(), 4, "{name}");
        let shot = h.render_gpu(
            SIZE,
            1.0,
            |gpu, enc, target, size| {
                let view = view_of(o, size, scale, center);
                let opts = RenderOptions::default();
                renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
            },
            |dl, text| labels::draw(dl, text, &edges, 1.0),
        );
        goldens.check(&format!("{name}-1x"), &shot);
    }
    goldens.finish();
}

/// A presentation state over the phantom: its lung window and a quarter turn clockwise
/// applied to the frame and the view, with annotations in two coloured layers — a filled
/// circle, an ellipse, a polyline, an interpolated curve and points in image pixels, and
/// text in a display-unit box anchored to an image point.
#[test]
fn presentation_state_annotations_match_goldens() {
    use super::present;
    use specular_dicom::Modality;
    use specular_dicom::gsps::{
        Annotation, Graphic, GraphicKind, ImageRef, Justify, Layer, PresentationState, SoftcopyVoi,
        TextObject, Units,
    };
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-gsps",
    );
    let mut spec = phantom();
    spec.extra = vec![(0x0008_0018, *b"UI", super::tests::text("1.2.3.4.5"))];
    let m = frame_of(&open(dicom(&spec)));
    let sop = "1.2.3.4.5";
    let all = vec![ImageRef {
        sop_uid: sop.to_owned(),
        frames: Vec::new(),
    }];
    let g = |kind, points: &[(f64, f64)], filled| Graphic {
        units: Units::Pixel,
        kind,
        points: points.to_vec(),
        filled,
        style: Default::default(),
        compound: None,
    };
    let ps = PresentationState {
        sop_uid: "9.8.7".to_owned(),
        label: "LUNGS".to_owned(),
        description: None,
        created: "20261001120000".to_owned(),
        references: all.clone(),
        modality: Some(Modality::Rescale {
            slope: 1.0,
            intercept: -1024.0,
        }),
        rescale_type: Some("HU".to_owned()),
        vois: vec![SoftcopyVoi {
            refs: Vec::new(),
            voi: Voi::Window {
                center: -600.0,
                width: 1500.0,
                function: VoiFunction::Linear,
            },
        }],
        shape: None,
        plut: None,
        rotation: 90,
        flip: false,
        areas: Vec::new(),
        layers: vec![
            Layer {
                name: "FINDINGS".to_owned(),
                order: 2,
                colour: Some([255, 80, 60]),
                grey: None,
                description: None,
            },
            Layer {
                name: "OUTLINES".to_owned(),
                order: 1,
                colour: Some([90, 220, 120]),
                grey: None,
                description: None,
            },
        ],
        annotations: vec![
            Annotation {
                layer: "OUTLINES".to_owned(),
                refs: Vec::new(),
                graphics: vec![
                    g(
                        GraphicKind::Ellipse,
                        &[(40.0, 34.0), (40.0, 86.0), (23.0, 60.0), (57.0, 60.0)],
                        false,
                    ),
                    g(
                        GraphicKind::Interpolated,
                        &[(76.0, 90.0), (88.0, 82.0), (100.0, 88.0), (104.0, 100.0)],
                        false,
                    ),
                    g(
                        GraphicKind::Polyline,
                        &[(54.0, 104.0), (64.0, 110.0), (74.0, 104.0)],
                        false,
                    ),
                ],
                texts: Vec::new(),
                compounds: Vec::new(),
            },
            Annotation {
                layer: "FINDINGS".to_owned(),
                refs: all,
                graphics: vec![
                    g(GraphicKind::Circle, &[(64.0, 56.0), (72.0, 56.0)], true),
                    g(GraphicKind::Point, &[(88.0, 50.0), (92.0, 66.0)], false),
                ],
                texts: vec![TextObject {
                    lines: vec!["Vessel".to_owned(), "180 HU".to_owned()],
                    bbox: Some((Units::Display, (0.05, 0.05), (0.32, 0.2))),
                    justify: Justify::Left,
                    anchor: Some((Units::Pixel, (64.0, 56.0), true)),
                    colour: None,
                    valign: Default::default(),
                    compound: None,
                }],
                compounds: Vec::new(),
            },
        ],
        ..PresentationState::default()
    };
    let m = Arc::new((*m).clone().presented(&ps, 0));
    assert_eq!(m.presentation(), Some("9.8.7"));
    assert!(!baked(&m));
    let values = m.values_f32().expect("grey");
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let n = u32::from(N);
    let mut img = renderer.create_image(&h.gpu, 1, n, n, None);
    renderer.set_values(&h.gpu, &mut img, &values);
    let w = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    assert_eq!((w.center, w.width), (-600.0, 1500.0));
    assert!(img.set_window(Some(w)));
    let (q, flip) = ps.rotation_flip();
    let o = present::gsps_orientation(q, flip);
    let size = (SIZE.0 as u32, SIZE.1 as u32);
    let scale = SIZE.1 / f32::from(N);
    let view = view_of(o, size, scale, (64.0, 64.0));
    let scene = present::scene(&ps, sop, 0, &view, 1.0);
    assert_eq!((scene.marks.len(), scene.notes.len()), (5, 1));
    let shot = h.render_gpu(
        SIZE,
        1.0,
        |gpu, enc, target, size| {
            let view = view_of(o, size, scale, (64.0, 64.0));
            let opts = RenderOptions::default();
            renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
        },
        |dl, text| present::draw(dl, text, &scene, 1.0),
    );
    goldens.check("medical-gsps-annotations-1x", &shot);
    goldens.finish();
}

/// A presentation state's shutter, activated overlays and compound graphics over the
/// phantom, through the app's own path (the frame presented by the state, its mask to the
/// GPU): a circular shutter in a CIELab blue, two overlay planes in their layers' colour
/// and grey; an arrow, a labelled axis, a crosshair with a gap, a turned filled rectangle,
/// a dashed ruler, and text right- and bottom-justified in its box. Shown flipped and
/// turned a quarter (the text upright, its sides swapped), and as stored with a 2:1
/// Presentation Pixel Aspect Ratio (rows twice as tall).
#[test]
fn presentation_state_shutters_overlays_and_compounds_match_goldens() {
    use super::present;
    use specular_dicom::graphics::{OverlayPlane, Shutter, ShutterShape};
    use specular_dicom::gsps::{
        Annotation, Compound, CompoundKind, DisplayedArea, ImageRef, Justify, Layer,
        PresentationState, SizeMode, Style, TextObject, Tick, TickAlign, Units, VAlign,
    };
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-gsps-compounds",
    );
    let mut spec = phantom();
    spec.extra = vec![(0x0008_0018, *b"UI", super::tests::text("1.2.3.4.6"))];
    let m = frame_of(&open(dicom(&spec)));
    let sop = "1.2.3.4.6";
    let all = vec![ImageRef {
        sop_uid: sop.to_owned(),
        frames: Vec::new(),
    }];
    let n = u32::from(N);
    // Overlay planes over the whole image: a box (6000) and a cross (6002).
    let plane = |group: u16, on: &dyn Fn(u32, u32) -> bool| {
        let mut data = vec![0u8; (n * n).div_ceil(8) as usize];
        for y in 0..n {
            for x in 0..n {
                if on(x, y) {
                    let i = (y * n + x) as usize;
                    data[i / 8] |= 1 << (i % 8);
                }
            }
        }
        OverlayPlane {
            group,
            rows: n,
            cols: n,
            origin: (0, 0),
            first_frame: 0,
            frames: 1,
            data,
        }
    };
    let boxed = |x: u32, y: u32| {
        (14..=44).contains(&x) && (y == 16 || y == 26)
            || (16..=26).contains(&y) && (x == 14 || x == 44)
    };
    let cross = |x: u32, y: u32| {
        (x == 100 && (92..=112).contains(&y)) || (y == 102 && (90..=110).contains(&x))
    };
    let layer = |name: &str, order, colour, grey| Layer {
        name: name.to_owned(),
        order,
        colour,
        grey,
        description: None,
    };
    let compound = |kind, points: &[(f64, f64)]| Compound {
        units: Units::Pixel,
        kind,
        points: points.to_vec(),
        rotation: 0.0,
        pivot: None,
        gap: None,
        visible: None,
        ticks: Vec::new(),
        tick_align: TickAlign::Center,
        show_labels: true,
        labels_below: false,
        filled: false,
        style: Style::default(),
        id: None,
    };
    let red = Style {
        colour: Some([255, 70, 60]),
        thickness: Some(1.5),
        ..Style::default()
    };
    let compounds = vec![
        Compound {
            style: red,
            ..compound(CompoundKind::Arrow, &[(64.0, 56.0), (96.0, 30.0)])
        },
        Compound {
            ticks: [("0", 0.0), ("5", 0.5), ("10", 1.0)]
                .map(|(l, at)| Tick {
                    at,
                    label: Some(l.to_owned()),
                })
                .to_vec(),
            tick_align: TickAlign::Top,
            ..compound(CompoundKind::Axis, &[(24.0, 116.0), (104.0, 116.0)])
        },
        Compound {
            gap: Some(0.04),
            visible: Some(0.16),
            ..compound(CompoundKind::Crosshair, &[(40.0, 60.0)])
        },
        Compound {
            rotation: 30.0,
            filled: true,
            style: Style {
                fill: Some([120, 180, 255]),
                opacity: Some(0.35),
                ..Style::default()
            },
            ..compound(CompoundKind::Rectangle, &[(76.0, 76.0), (100.0, 90.0)])
        },
        Compound {
            style: Style {
                dashed: true,
                thickness: Some(1.0),
                ..Style::default()
            },
            ..compound(CompoundKind::Ruler, &[(116.0, 30.0), (116.0, 90.0)])
        },
    ];
    let ps = |rotation, flip, aspect: f64| PresentationState {
        sop_uid: "9.8.6".to_owned(),
        label: "MARKUP".to_owned(),
        created: "20261010120000".to_owned(),
        references: all.clone(),
        rotation,
        flip,
        areas: vec![DisplayedArea {
            refs: Vec::new(),
            tlhc: (1, 1),
            brhc: (128, 128),
            mode: SizeMode::ScaleToFit,
            aspect,
        }],
        layers: vec![
            layer("OVERLAY", 1, Some([255, 200, 0]), None),
            layer("CROSS", 2, None, Some(40000)),
            layer("FINDINGS", 3, Some([90, 220, 120]), None),
        ],
        annotations: vec![Annotation {
            layer: "FINDINGS".to_owned(),
            refs: Vec::new(),
            graphics: Vec::new(),
            texts: vec![TextObject {
                lines: vec!["Right".to_owned(), "bottom".to_owned()],
                bbox: Some((Units::Pixel, (60.0, 4.0), (124.0, 26.0))),
                justify: Justify::Right,
                anchor: None,
                colour: Some([120, 230, 255]),
                valign: VAlign::Bottom,
                compound: None,
            }],
            compounds: compounds.clone(),
        }],
        shutter: Some(Shutter {
            shapes: vec![ShutterShape::Circle {
                row: 66.0,
                col: 64.5,
                radius: 60.0,
            }],
            bitmap: None,
            grey: 0,
            colour: Some([40, 60, 110]),
        }),
        overlays: vec![plane(0x6000, &boxed), plane(0x6002, &cross)],
        activations: vec![(0x6000, "OVERLAY".to_owned()), (0x6002, "CROSS".to_owned())],
        ..PresentationState::default()
    };
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let cases = [
        ("medical-gsps-compounds-turned", 90, true, 1.0),
        ("medical-gsps-compounds-aspect", 0, false, 2.0),
    ];
    for (name, rotation, flip, aspect) in cases {
        let ps = ps(rotation, flip, aspect);
        let pm = Arc::new((*m).clone().presented(&ps, 0));
        let g = pm.graphics().expect("a mask");
        assert_eq!(g.inks, vec![[255, 200, 0], [156; 3]], "{name}");
        let (shutter, inks) = super::mask_colours(g);
        let values = pm.values_f32().expect("grey");
        let mut img = renderer.create_image(&h.gpu, 1, n, n, None);
        renderer.set_values(&h.gpu, &mut img, &values);
        renderer.set_graphics(
            &h.gpu,
            &mut img,
            Some(specular_render::ValueMask {
                mask: &g.mask,
                shutter,
                inks: &inks,
            }),
        );
        let w = window_of(&pm.default_voi().0, pm.inverted()).expect("a window");
        assert!(img.set_window(Some(w)));
        let (q, f) = ps.rotation_flip();
        let o = present::gsps_orientation(q, f);
        let view_for = |size: (u32, u32)| {
            let mut v = View::new();
            v.set_viewport(PxRect {
                x: 0.0,
                y: 0.0,
                w: size.0 as f32,
                h: size.1 as f32,
            });
            v.upscale_small = true;
            v.set_image(n, n, o);
            v.set_pixel_aspect(ps.areas[0].aspect as f32);
            v.settle();
            v
        };
        let view = view_for((SIZE.0 as u32, SIZE.1 as u32));
        let scene = present::scene(&ps, sop, 0, &view, 1.0);
        // Arrow (line, head), axis (line, 3 ticks), crosshair (4 arms), rectangle, ruler
        // (line, 11 ticks).
        assert_eq!(
            (scene.marks.len(), scene.notes.len(), scene.tags.len()),
            (2 + 4 + 4 + 1 + 12, 1, 3),
            "{name}"
        );
        let shot = h.render_gpu(
            SIZE,
            1.0,
            |gpu, enc, target, size| {
                let view = view_for(size);
                let opts = RenderOptions::default();
                renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
            },
            |dl, text| present::draw(dl, text, &scene, 1.0),
        );
        goldens.check(&format!("{name}-1x"), &shot);
    }
    goldens.finish();
}

/// A segmentation on the phantom's pixel grid (1 mm, three slices around z = 0): segment 1
/// over the contrast vessel, segment 2 over the spine.
fn phantom_segmentation() -> specular_dicom::labels::Labels {
    use specular_dicom::labels::{LabelSource, Labels, Layer, Segment};
    use specular_dicom::volume::{Geometry, Volume, Voxels};
    let n = u32::from(N);
    let positions = [[0.0, 0.0, -1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let g = Geometry::new(
        &positions,
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0],
        [n, n],
    );
    let inside =
        |x: f64, y: f64, cx: f64, cy: f64, r: f64| (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    let mut ids = vec![0u16; (n * n * 3) as usize];
    for k in 0..3 {
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (f64::from(i) + 0.5, f64::from(j) + 0.5);
                let idx = ((k * n + j) * n + i) as usize;
                if inside(x, y, 64.0, 56.0, 7.0) {
                    ids[idx] = 1;
                } else if inside(x, y, 64.0, 94.0, 9.0) {
                    ids[idx] = 2;
                }
            }
        }
    }
    let segment = |number, label: &str, colour| Segment {
        number,
        label: label.to_owned(),
        colour,
        algorithm: "MANUAL".to_owned(),
        fractional: false,
        ml: 0.0,
        voxels: 0,
    };
    Labels {
        source: LabelSource::Seg,
        segments: vec![
            segment(1, "Vessel", [230, 80, 60]),
            segment(2, "Spine", [60, 160, 255]),
        ],
        layers: vec![Layer {
            volume: Volume {
                geometry: g,
                voxels: Voxels::U16(ids),
                loaded: vec![true; 3],
                step_xy: 1,
            },
            fraction: None,
        }],
        frame_of_reference: "1.2.3".to_owned(),
        referenced_series: Vec::new(),
        title: "Phantom".to_owned(),
    }
}

/// A first-pass-like curve: twelve points 10 s apart.
fn synthetic_curve() -> analysis::Curve {
    let points = (0..12)
        .map(|i| {
            let t = f64::from(i) * 10.0;
            let x = t / 30.0;
            let mean = 40.0 + 180.0 * x * x * (-x).exp() * 2.0;
            specular_dicom::tic::Point {
                t,
                mean,
                max: mean * 1.5 + 20.0,
                sd: 0.0,
                count: 100,
            }
        })
        .collect();
    analysis::Curve {
        points,
        unit: "HU".to_owned(),
    }
}

#[test]
fn segmentation_and_curve_match_goldens() {
    use super::super::mpr::analysis::Set;
    use specular_render_volume::VolumeTexture;
    use specular_theme::Palette;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-analysis",
    );
    let m = frame_of(&open(dicom(&phantom())));
    let values = m.values_f32().expect("grey");
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let n = u32::from(N);
    let mut img = renderer.create_image(&h.gpu, 1, n, n, None);
    renderer.set_values(&h.gpu, &mut img, &values);
    let default = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    assert!(img.set_window(Some(default)));
    let labels = phantom_segmentation();
    let mut tex = VolumeTexture::new(&h.gpu, &labels.layers[0].volume).expect("texture");
    tex.sync(&h.gpu, &labels.layers[0].volume).expect("upload");
    let mut an = analysis::State::default();
    an.geom = Some(analysis::Geom {
        positions: vec![[0.0, 0.0, 0.0]],
        row: [1.0, 0.0, 0.0],
        col: [0.0, 1.0, 0.0],
        series: "1".to_owned(),
        frame_of_reference: "1.2.3".to_owned(),
        modality: "CT".to_owned(),
    });
    an.sets = vec![Set {
        labels,
        textures: vec![tex],
        hidden: vec![false, false],
    }];
    let curve = synthetic_curve();
    let logical = (480.0, 360.0);
    // name, scale, right-to-left, light, the spine hidden
    let cases = [
        ("medical-seg-curve-1x", 1.0, false, false, false),
        ("medical-seg-curve-light-1_5x", 1.5, false, true, true),
        ("medical-seg-curve-he-1x", 1.0, true, false, false),
    ];
    for (name, scale, rtl, light, hide) in cases {
        an.sets[0].hidden[1] = hide;
        // At the vessel: its name in the readout.
        let names = an.names_at([63.5, 55.5, 0.0]);
        assert_eq!(names, ["Vessel"], "{name}");
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let pal = if light {
            Palette::light()
        } else {
            Palette::dark()
        };
        let view_for = |size: (u32, u32)| {
            view_of(
                Orientation::NORMAL,
                size,
                size.1 as f32 / f32::from(N),
                (64.0, 64.0),
            )
        };
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let mut lines = vec!["180 HU".to_owned()];
            lines.extend(names.clone());
            h.render_gpu(
                logical,
                scale,
                |gpu, enc, target, size| {
                    let view = view_for(size);
                    let opts = RenderOptions::default();
                    renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
                    let plane = analysis::plane_of(
                        an.geom.as_ref().expect("geometry"),
                        0,
                        (1.0, 1.0),
                        &view,
                    )
                    .expect("a plane");
                    an.render(gpu, enc, target, size, &plane, f64::from(scale));
                },
                |dl, text| {
                    let area = (0.0, 0.0, logical.0, logical.1);
                    draw_readout(dl, text, &lines, area, rtl, 1.0);
                    analysis::draw_curve(dl, text, &pal, &curve, area, rtl, 1.0);
                },
            )
        });
        goldens.check(name, &shot);
    }
    goldens.finish();
}

/// A NIfTI volume in medical mode: three slices of the phantom (the outer two at half the
/// values) stored with i toward the patient's right and j toward the front (RAS), opened by
/// content and shown as its middle slice in the DICOM convention (both in-plane axes
/// flipped), with its cal_min/cal_max window, the orientation labels and the readout;
/// left-to-right in English and right-to-left in Hebrew.
#[test]
fn nifti_slice_matches_goldens() {
    use super::labels;
    use super::tests::nifti_i16;
    let Some(mut h) = Harness::new() else { return };
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-medical-nifti",
    );
    let slice = &phantom().frames[0];
    let mut values: Vec<i16> = Vec::new();
    for k in 0..3 {
        for &v in slice {
            let hu = (v - 1024) as i16;
            values.push(if k == 1 { hu } else { hu / 2 });
        }
    }
    let half = 0.7 * (f32::from(N) - 1.0) / 2.0;
    let data = nifti_i16(
        [N, N, 3],
        [
            [0.7, 0.0, 0.0, -half],
            [0.0, 0.7, 0.0, -half],
            [0.0, 0.0, 2.5, -2.5],
        ],
        (-160.0, 240.0),
        "phantom",
        &values,
    );
    let s = specular_decode::Registry::default()
        .open(std::sync::Arc::new(specular_core::Source::from_vec(
            "phantom.nii",
            data,
        )))
        .expect("NIfTI opens");
    assert_eq!(s.info().format, specular_decode::Format::Nifti);
    assert_eq!((s.page_count(), s.page_index()), (3, 1));
    let m = frame_of(&s);
    let axes = *m.patient_axes().expect("patient axes");
    let values = m.values_f32().expect("grey");
    let mut renderer = ImageRenderer::new(&h.gpu, 32 << 20);
    let n = u32::from(N);
    let mut img = renderer.create_image(&h.gpu, 1, n, n, None);
    renderer.set_values(&h.gpu, &mut img, &values);
    let default = window_of(&m.default_voi().0, m.inverted()).expect("a window");
    assert_eq!((default.center, default.width), (40.0, 400.0));
    assert!(img.set_window(Some(default)));
    let fit = SIZE.1 / f32::from(N);
    for (name, rtl) in [("medical-nifti", false), ("medical-nifti-he", true)] {
        let lang = if rtl {
            specular_i18n::langid!("he")
        } else {
            specular_i18n::langid!("en-US")
        };
        let size = (SIZE.0 as u32, SIZE.1 as u32);
        let view = view_of(Orientation::NORMAL, size, fit, (64.0, 64.0));
        let edges = labels::edges(&axes, &view, 1.0);
        assert_eq!(edges.len(), 4, "{name}");
        let shot = specular_i18n::I18n::with_thread_language(lang, || {
            let lines = [
                specular_i18n::t!("medical-frame", frame = 2i64, count = 3i64),
                specular_i18n::t!(
                    "medical-window",
                    center = number(default.center),
                    width = number(default.width)
                ),
            ];
            h.render_gpu(
                SIZE,
                1.0,
                |gpu, enc, target, size| {
                    let view = view_of(Orientation::NORMAL, size, fit, (64.0, 64.0));
                    let opts = RenderOptions::default();
                    renderer.render(gpu, enc, target, size, Some(&img), &view, &opts);
                },
                |dl, text| {
                    labels::draw(dl, text, &edges, 1.0);
                    draw_readout(dl, text, &lines, (0.0, 0.0, SIZE.0, SIZE.1), rtl, 1.0);
                },
            )
        });
        goldens.check(&format!("{name}-1x"), &shot);
    }
    goldens.finish();
}
