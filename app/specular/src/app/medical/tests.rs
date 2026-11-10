//! Medical mode: the window the GPU draws against the decoder's own VOI rendering, drags,
//! presets and the readout's numbers. Every DICOM file is generated here.

use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::{DecodeRequest, Registry, Session};

use super::*;

/// What a synthetic grey DICOM holds.
pub(super) struct Spec<'a> {
    pub width: u16,
    pub height: u16,
    pub modality: &'a str,
    pub photometric: &'a str,
    pub signed: bool,
    pub bits_stored: u16,
    pub rescale: Option<(f64, f64)>,
    pub window: Option<(f64, f64)>,
    pub function: Option<&'a str>,
    /// Stored values, frame after frame.
    pub frames: Vec<Vec<i32>>,
    /// More elements (tag, VR, value).
    pub extra: Vec<(u32, [u8; 2], Vec<u8>)>,
}

/// An explicit VR little-endian Part 10 file for `s`.
pub(super) fn dicom(s: &Spec) -> Vec<u8> {
    fn el(out: &mut Vec<u8>, tag: u32, vr: &[u8; 2], val: &[u8]) {
        out.extend(((tag >> 16) as u16).to_le_bytes());
        out.extend((tag as u16).to_le_bytes());
        out.extend(vr);
        if matches!(vr, b"OB" | b"OW" | b"SQ" | b"UN" | b"UT") {
            out.extend([0, 0]);
            out.extend((val.len() as u32).to_le_bytes());
        } else {
            out.extend((val.len() as u16).to_le_bytes());
        }
        out.extend(val);
    }
    let mut meta = Vec::new();
    el(&mut meta, 0x0002_0010, b"UI", b"1.2.840.10008.1.2.1\0");
    let mut f = vec![0u8; 128];
    f.extend(b"DICM");
    el(
        &mut f,
        0x0002_0000,
        b"UL",
        &(meta.len() as u32).to_le_bytes(),
    );
    f.extend(&meta);
    // The data set, in tag order with the extra elements.
    let mut els: Vec<(u32, [u8; 2], Vec<u8>)> = vec![
        (0x0008_0060, *b"CS", text(s.modality)),
        (0x0028_0002, *b"US", 1u16.to_le_bytes().to_vec()),
        (0x0028_0004, *b"CS", text(s.photometric)),
        (0x0028_0010, *b"US", s.height.to_le_bytes().to_vec()),
        (0x0028_0011, *b"US", s.width.to_le_bytes().to_vec()),
        (0x0028_0100, *b"US", 16u16.to_le_bytes().to_vec()),
        (0x0028_0101, *b"US", s.bits_stored.to_le_bytes().to_vec()),
        (
            0x0028_0102,
            *b"US",
            (s.bits_stored - 1).to_le_bytes().to_vec(),
        ),
        (
            0x0028_0103,
            *b"US",
            u16::from(s.signed).to_le_bytes().to_vec(),
        ),
    ];
    if s.frames.len() > 1 {
        els.push((0x0028_0008, *b"IS", text(&s.frames.len().to_string())));
    }
    if let Some((c, w)) = s.window {
        els.push((0x0028_1050, *b"DS", text(&c.to_string())));
        els.push((0x0028_1051, *b"DS", text(&w.to_string())));
    }
    if let Some((slope, intercept)) = s.rescale {
        els.push((0x0028_1052, *b"DS", text(&intercept.to_string())));
        els.push((0x0028_1053, *b"DS", text(&slope.to_string())));
    }
    if let Some(func) = s.function {
        els.push((0x0028_1056, *b"CS", text(func)));
    }
    els.extend(s.extra.iter().cloned());
    let px: Vec<u8> = s
        .frames
        .iter()
        .flatten()
        .flat_map(|&v| {
            if s.signed {
                (v as i16).to_le_bytes()
            } else {
                (v as u16).to_le_bytes()
            }
        })
        .collect();
    els.push((0x7FE0_0010, *b"OW", px));
    els.sort_by_key(|e| e.0);
    for (tag, vr, val) in &els {
        el(&mut f, *tag, vr, val);
    }
    f
}

/// A text value, padded to even length.
pub(super) fn text(v: &str) -> Vec<u8> {
    let mut b = v.as_bytes().to_vec();
    if b.len() % 2 == 1 {
        b.push(b' ');
    }
    b
}

/// A single-file NIfTI-1 of signed 16-bit values (`values` in stored order, i fastest):
/// `dims` (i, j, k), the sform rows (RAS mm, as NIfTI states them), `cal` (min, max) as the
/// default window and a description.
pub(in crate::app) fn nifti_i16(
    dims: [u16; 3],
    srow: [[f32; 4]; 3],
    cal: (f32, f32),
    description: &str,
    values: &[i16],
) -> Vec<u8> {
    let mut h = vec![0u8; 352];
    let put = |h: &mut Vec<u8>, at: usize, b: &[u8]| h[at..at + b.len()].copy_from_slice(b);
    put(&mut h, 0, &348i32.to_le_bytes());
    for (n, d) in [3, dims[0], dims[1], dims[2], 1, 1, 1, 1]
        .iter()
        .enumerate()
    {
        put(&mut h, 40 + 2 * n, &(*d as i16).to_le_bytes());
    }
    put(&mut h, 70, &4i16.to_le_bytes());
    put(&mut h, 72, &16i16.to_le_bytes());
    let len = |r: [f32; 4]| (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let col = |c: usize| [srow[0][c], srow[1][c], srow[2][c], 0.0];
    for (n, p) in [1.0, len(col(0)), len(col(1)), len(col(2))]
        .iter()
        .enumerate()
    {
        put(&mut h, 76 + 4 * n, &f32::to_le_bytes(*p));
    }
    put(&mut h, 108, &352f32.to_le_bytes());
    put(&mut h, 112, &1f32.to_le_bytes());
    put(&mut h, 124, &cal.1.to_le_bytes());
    put(&mut h, 128, &cal.0.to_le_bytes());
    put(
        &mut h,
        148,
        &description.as_bytes()[..description.len().min(79)],
    );
    put(&mut h, 254, &1i16.to_le_bytes());
    for (r, row) in srow.iter().enumerate() {
        for (c, x) in row.iter().enumerate() {
            put(&mut h, 280 + 16 * r + 4 * c, &x.to_le_bytes());
        }
    }
    put(&mut h, 344, b"n+1\0");
    assert_eq!(
        values.len(),
        dims.iter().map(|&d| usize::from(d)).product::<usize>()
    );
    h.extend(values.iter().flat_map(|v| v.to_le_bytes()));
    h
}

pub(super) fn open(data: Vec<u8>) -> Arc<dyn Session> {
    Registry::default()
        .open(Arc::new(Source::from_vec("synthetic.dcm", data)))
        .expect("synthetic DICOM opens")
}

pub(super) fn frame_of(s: &Arc<dyn Session>) -> Arc<ModalityFrame> {
    s.modality_frame(&CancelToken::new())
        .expect("a DICOM session")
        .expect("values")
}

/// A signed 16-bit CT ramp over every 12-bit stored value, HU = stored − 1024.
fn ct(window: Option<(f64, f64)>, function: Option<&str>) -> Spec<'static> {
    let (w, h) = (64u16, 64u16);
    let v = (0..i32::from(w) * i32::from(h)).map(|i| i - 2048).collect();
    Spec {
        width: w,
        height: h,
        modality: "CT",
        photometric: "MONOCHROME2",
        signed: true,
        bits_stored: 12,
        rescale: Some((1.0, -1024.0)),
        window,
        function: function.map(|f| match f {
            "LINEAR_EXACT" => "LINEAR_EXACT",
            "SIGMOID" => "SIGMOID",
            _ => "LINEAR",
        }),
        frames: vec![v],
        extra: Vec::new(),
    }
}

/// The decoder's 16-bit rendering with the file's default VOI against the window medical mode
/// draws (its f32 values through [`WlWindow::grey`]), pixel by pixel: within one 16-bit step.
fn check_default_matches(spec: &Spec) {
    let s = open(dicom(spec));
    let m = frame_of(&s);
    let (voi, _) = m.default_voi();
    let w = window_of(&voi, m.inverted()).expect("a window");
    // The decoder's own rendering, as the viewer shows it without a window.
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .unwrap();
    assert_eq!(r.layout, PixelLayout::Gray16);
    let cpu: Vec<u16> = r.planes[0]
        .data
        .chunks_exact(2)
        .map(|b| u16::from_ne_bytes([b[0], b[1]]))
        .collect();
    let values = m.values_f32().expect("grey values");
    assert_eq!(values.len(), cpu.len());
    for (i, (&v, &want)) in values.iter().zip(&cpu).enumerate() {
        let got = (w.grey(f64::from(v)) * 65535.0 + 0.5) as i64;
        assert!(
            (got - i64::from(want)).abs() <= 1,
            "{} {:?} pixel {i}: value {v}, window {got}, decoder {want}",
            spec.modality,
            w
        );
    }
}

#[test]
fn linear_window_matches_the_decoder() {
    check_default_matches(&ct(Some((40.0, 400.0)), None));
    // W = 1: LINEAR is a threshold at c − 0.5.
    check_default_matches(&ct(Some((40.0, 1.0)), None));
}

#[test]
fn linear_exact_and_sigmoid_match_the_decoder() {
    check_default_matches(&ct(Some((-600.0, 1500.0)), Some("LINEAR_EXACT")));
    check_default_matches(&ct(Some((50.0, 350.0)), Some("SIGMOID")));
}

#[test]
fn monochrome1_and_unsigned_match_the_decoder() {
    let (w, h) = (32u16, 16u16);
    let v: Vec<i32> = (0..i32::from(w) * i32::from(h)).map(|i| i * 8).collect();
    let dx = Spec {
        width: w,
        height: h,
        modality: "DX",
        photometric: "MONOCHROME1",
        signed: false,
        bits_stored: 14,
        rescale: None,
        window: Some((2000.0, 3000.0)),
        function: None,
        frames: vec![v],
        extra: Vec::new(),
    };
    check_default_matches(&dx);
    let m = frame_of(&open(dicom(&dx)));
    assert!(m.inverted());
    assert!(window_of(&m.default_voi().0, m.inverted()).unwrap().invert);
    // No window in the file: the value range, LINEAR_EXACT.
    let no_window = Spec { window: None, ..dx };
    check_default_matches(&no_window);
}

#[test]
fn every_voi_function_maps_to_the_same_curve() {
    for function in [
        VoiFunction::Linear,
        VoiFunction::LinearExact,
        VoiFunction::Sigmoid,
    ] {
        for (center, width) in [(40.0, 400.0), (0.5, 1.0), (-600.0, 1500.0), (40.0, 1.0)] {
            let voi = Voi::Window {
                center,
                width,
                function,
            };
            let w = window_of(&voi, false).unwrap();
            for x in (-40000..40000).step_by(7).map(f64::from) {
                let (a, b) = (w.apply(x), voi.apply(x));
                assert!((a - b).abs() < 1e-12, "{voi:?} at {x}: {a} vs {b}");
            }
        }
    }
}

#[test]
fn drags_scale_with_the_range() {
    let w = WlWindow {
        center: 40.0,
        width: 400.0,
        shape: VoiShape::Linear,
        invert: false,
    };
    // A CT's range of 4096 HU: a drag across DRAG_SPAN_PX moves by all of it.
    let d = dragged(w, DRAG_SPAN_PX, 0.0, 4096.0);
    assert!((d.width - 4496.0).abs() < 1e-9 && d.center == 40.0);
    let d = dragged(w, 0.0, -DRAG_SPAN_PX / 2.0, 4096.0);
    assert!((d.center - (40.0 - 2048.0)).abs() < 1e-9 && d.width == 400.0);
    // Never narrower than LINEAR's smallest width, keeps shape and inversion.
    let d = dragged(WlWindow { invert: true, ..w }, -1e6, 0.0, 4096.0);
    assert_eq!((d.width, d.shape, d.invert), (1.0, VoiShape::Linear, true));
    // A tiny range (a 0..1 float image) still moves.
    let e = WlWindow {
        shape: VoiShape::LinearExact,
        center: 0.5,
        width: 1.0,
        ..w
    };
    let d = dragged(e, -1e6, 0.0, 1.0);
    assert!(d.width > 0.0 && d.width < 1e-3);
}

#[test]
fn presets_and_strings() {
    use specular_input::Action;
    for (n, (name, width, level)) in CT_PRESETS.iter().enumerate() {
        let id = Action::WindowPreset(n as u8 + 1).name();
        assert_eq!(
            id,
            specular_i18n::I18n::global().tr(&format!("palette-window-{name}"))
        );
        assert!(*width >= 40.0 && (-600.0..=400.0).contains(level));
    }
    assert_eq!(number(-45.0), "\u{2212}45");
    assert_eq!(number(1500.0), "1500");
    assert_eq!(number(0.126), "0.13");
    assert_eq!(number(-0.5), "\u{2212}0.5");
    let r = range_window((-1024.0, 3071.0), true);
    assert_eq!(
        (r.center, r.width, r.shape, r.invert),
        (1023.5, 4095.0, VoiShape::LinearExact, true)
    );
}

#[test]
fn multi_frame_files_step_through_their_frames() {
    let one = |k: i32| (0..16).map(|i| i * 10 + k).collect::<Vec<i32>>();
    let spec = Spec {
        width: 4,
        height: 4,
        modality: "CT",
        photometric: "MONOCHROME2",
        signed: true,
        bits_stored: 16,
        rescale: Some((1.0, -1024.0)),
        window: Some((40.0, 400.0)),
        function: None,
        frames: vec![one(0), one(1000), one(2000)],
        extra: Vec::new(),
    };
    let s = open(dicom(&spec));
    assert_eq!(s.page_count(), 3);
    let f2 = specular_decode::open_dicom_frame(s.source().clone(), 2).unwrap();
    let m = frame_of(&f2);
    assert_eq!(m.value_at(1, 0), Some(10.0 + 2000.0 - 1024.0));
    assert_eq!(m.values_f32().unwrap()[1], 986.0);
}

/// A 10×10 CT, HU = column + 10 × row, with `extra` elements.
fn small_ct(extra: Vec<(u32, [u8; 2], Vec<u8>)>) -> Arc<ModalityFrame> {
    let v = (0..100).map(|i| i + 1024).collect();
    let spec = Spec {
        width: 10,
        height: 10,
        modality: "CT",
        photometric: "MONOCHROME2",
        signed: true,
        bits_stored: 16,
        rescale: Some((1.0, -1024.0)),
        window: Some((40.0, 400.0)),
        function: None,
        frames: vec![v],
        extra,
    };
    frame_of(&open(dicom(&spec)))
}

/// A label without Fluent's bidi isolation marks.
fn plain(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|l| l.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn measurements_carry_their_calibration() {
    use measure::{Tool, label};
    let spacing = |tag: u32, v: &str| vec![(tag, *b"DS", text(v))];
    // Pixel Spacing: millimetres in the patient, no caveat.
    let m = small_ct(spacing(0x0028_0030, "0.5\\0.5"));
    let l = plain(label(
        Tool::Length,
        &[(0.0, 0.0), (3.0, 4.0)],
        &m,
        Some("HU"),
    ));
    assert_eq!(l, vec!["2.5 mm"]);
    // A 2×1 rectangle over HU 0 and 1: the area, mean ± SD, range and count.
    let r = plain(label(Tool::Rect, &[(2.0, 1.0), (0.0, 0.0)], &m, Some("HU")));
    assert_eq!(
        r,
        vec!["Area 0.5 mm²", "Mean 0.5 ± 0.5 HU", "Min 0  Max 1  (2 px)"]
    );
    // An ellipse inscribed in 4×4 pixels covers the 12 centres off its corners.
    let e = plain(label(
        Tool::Ellipse,
        &[(0.0, 0.0), (4.0, 4.0)],
        &m,
        Some("HU"),
    ));
    assert!(e[2].ends_with("(12 px)"), "{e:?}");
    let a = plain(label(
        Tool::Angle,
        &[(5.0, 0.0), (0.0, 0.0), (0.0, 5.0)],
        &m,
        None,
    ));
    assert_eq!(a, vec!["90.0°"]);
    // Imager Pixel Spacing only: millimetres at the detector, said so.
    let d = small_ct(spacing(0x0018_1164, "0.2\\0.2"));
    let l = plain(label(Tool::Length, &[(0.0, 0.0), (10.0, 0.0)], &d, None));
    assert_eq!(l[0], "2.0 mm");
    assert!(l[1].contains("detector"), "{l:?}");
    // No spacing: pixels, said so.
    let n = small_ct(Vec::new());
    let l = plain(label(Tool::Length, &[(0.0, 0.0), (3.0, 4.0)], &n, None));
    assert_eq!(l[0], "5.0 px");
    assert!(l[1].contains("No pixel spacing"), "{l:?}");
    let r = plain(label(Tool::Rect, &[(0.0, 0.0), (2.0, 2.0)], &n, Some("HU")));
    assert_eq!(r[0], "Area 4.0 px²");
}

#[test]
fn measuring_tools_and_outlines() {
    use measure::{Tool, outline, roi_of};
    use specular_input::Action;
    assert_eq!(Tool::of(Action::MeasureAngle), Some(Tool::Angle));
    assert_eq!(Tool::of(Action::Cine), None);
    assert_eq!((Tool::Angle.points(), Tool::Rect.points()), (3, 2));
    // From the top-left corner whichever way it was dragged.
    let (o, closed) = outline(Tool::Rect, &[(4.0, 1.0), (0.0, 3.0)]);
    assert!(closed && o == vec![(0.0, 1.0), (4.0, 1.0), (4.0, 3.0), (0.0, 3.0)]);
    let (o, closed) = outline(Tool::Ellipse, &[(0.0, 0.0), (4.0, 2.0)]);
    assert!(closed && o.len() == 72 && o[0] == (4.0, 1.0));
    let (o, closed) = outline(Tool::Angle, &[(0.0, 0.0), (1.0, 1.0)]);
    assert!(!closed && o.len() == 2);
    assert_eq!(
        roi_of(Tool::Ellipse, (4.0, 2.0), (0.0, 0.0)),
        Some(specular_measure::Shape::Ellipse {
            cx: 2.0,
            cy: 1.0,
            rx: 2.0,
            ry: 1.0
        })
    );
    assert_eq!(roi_of(Tool::Length, (0.0, 0.0), (1.0, 1.0)), None);
}

#[test]
fn hidden_overlays_and_cine_timing() {
    let mut m = Medical {
        default: Some(range_window((0.0, 100.0), false)),
        ..Medical::default()
    };
    // Frame 0 at the file's window draws the CPU raster (overlays burnt in) ...
    assert_eq!(m.effective(), None);
    m.graphics = true;
    assert_eq!(m.effective(), None);
    // ... hiding them needs the GPU window.
    m.hide_overlays = true;
    assert_eq!(m.effective(), m.default);
    m.graphics = false;
    assert_eq!(m.effective(), None);
    // Frame times: the file's, else 10 frames a second; a zero time is not stalled on.
    assert_eq!(m.frame_time(0), specular_decode::DEFAULT_CINE_FRAME);
    m.timing = Some(vec![Duration::from_millis(40), Duration::ZERO]);
    assert_eq!(m.frame_time(0), Duration::from_millis(40));
    assert_eq!(m.frame_time(1), specular_decode::DEFAULT_CINE_FRAME);
    assert_eq!(m.frame_time(5), specular_decode::DEFAULT_CINE_FRAME);
}

/// A grey multi-frame file with a frame time and a circular shutter: its timing, and each
/// frame's values carry the shutter mask.
#[test]
fn grey_cine_files_state_their_timing_and_shutters() {
    let one = |k: i32| (0..16).map(|i| i + k).collect::<Vec<i32>>();
    let spec = Spec {
        width: 4,
        height: 4,
        modality: "XA",
        photometric: "MONOCHROME2",
        signed: false,
        bits_stored: 12,
        rescale: None,
        window: None,
        function: None,
        frames: vec![one(0), one(100), one(200)],
        extra: vec![
            (0x0018_1063, *b"DS", text("66.7")),
            (0x0018_1600, *b"CS", text("CIRCULAR")),
            (0x0018_1610, *b"IS", text("2\\2")),
            (0x0018_1612, *b"IS", text("1")),
        ],
    };
    let s = open(dicom(&spec));
    let t = s.cine_timing().expect("frame time");
    assert_eq!(t.len(), 3);
    assert!(t.iter().all(|d| (d.as_secs_f64() - 0.0667).abs() < 1e-6));
    // Grey cine plays in medical mode, not through the animation player.
    assert!(s.animation().is_none());
    let f = s
        .modality_frame_at(2, &CancelToken::new())
        .unwrap()
        .unwrap();
    assert_eq!(f.value_at(0, 0), Some(200.0));
    let g = f.graphics().expect("a shutter");
    assert!(g.shutter && g.at(0, 0) == specular_dicom::graphics::SHUTTER);
    assert_eq!(g.at(1, 1), 0);
}

/// A readout whose corner comes under the window-centred toolbar (the viewer narrowed by
/// the info panel) moves up above it; one beside the bar stays in its corner.
#[test]
fn readout_keeps_clear_of_the_toolbar() {
    let bar = Some(Rect::new(410.0, 738.0, 458.0, 46.0));
    let under = Rect::new(840.0, 740.0, 120.0, 44.0);
    let r = clear_of_bar(under, bar);
    assert_eq!((r.x, r.w, r.h), (under.x, under.w, under.h));
    assert!(r.y + r.h <= 738.0 - TOOLBAR_MARGIN + 0.01, "{r:?}");
    let beside = Rect::new(1140.0, 740.0, 120.0, 44.0);
    assert_eq!(clear_of_bar(beside, bar), beside);
    assert_eq!(clear_of_bar(under, None), under);
}

/// A readout line longer than the image is wide (a long file name in the MPR's streamlines
/// line) is cut with an ellipsis: the box stays inside its area.
#[test]
fn a_long_readout_line_is_cut_to_the_area() {
    let mut text = specular_gfx::Text::new();
    let mut dl = specular_gfx::DrawList::new(1.0);
    let lines = [
        "C 40  W 400".to_owned(),
        format!(
            "Streamlines from {}: 2 mm slab",
            "very-long-name-".repeat(12)
        ),
    ];
    let area = (0.0, 0.0, 320.0, 240.0);
    let r = draw_readout(&mut dl, &mut text, &lines, area, false, 1.0);
    assert!(r.x >= TOOLBAR_MARGIN - 0.01, "{r:?}");
    assert!(r.x + r.w <= 320.0 - TOOLBAR_MARGIN + 0.01, "{r:?}");
    let rtl = draw_readout(&mut dl, &mut text, &lines, area, true, 1.0);
    assert!(rtl.x + rtl.w <= 320.0 - TOOLBAR_MARGIN + 0.01, "{rtl:?}");
}

/// A length's or an angle's points are grabbed where drawn (the nearest within the radius,
/// on the shown frame only; a region's four corners too), and a point dropped on another of
/// its measurement's removes it.
#[test]
fn measurement_points_are_grabbed_and_dropped() {
    use measure::{Measurement, Tool, dropped_on_another, end_at};
    let m = |tool, points: Vec<(f64, f64)>, frame| Measurement {
        tool,
        points,
        frame,
        lines: Vec::new(),
    };
    // Twice the stored size on screen.
    let to_screen = |p: (f64, f64)| (p.0 * 2.0, p.1 * 2.0);
    let items = vec![
        m(Tool::Length, vec![(10.0, 10.0), (40.0, 10.0)], 0),
        m(
            Tool::Angle,
            vec![(12.0, 10.0), (20.0, 30.0), (40.0, 30.0)],
            0,
        ),
        m(Tool::Rect, vec![(50.0, 50.0), (60.0, 60.0)], 0),
        m(Tool::Length, vec![(10.0, 10.0), (40.0, 10.0)], 1),
    ];
    assert_eq!(
        end_at(&items, 0, to_screen, (21.0, 21.0), 12.0),
        Some((0, 0))
    );
    assert_eq!(
        end_at(&items, 0, to_screen, (24.0, 20.0), 12.0),
        Some((1, 0))
    );
    assert_eq!(
        end_at(&items, 0, to_screen, (41.0, 59.0), 12.0),
        Some((1, 1))
    );
    assert_eq!(end_at(&items, 0, to_screen, (50.0, 40.0), 12.0), None);
    // A region's four corners are handles (it had none): the placed two, then the others.
    assert_eq!(
        end_at(&items, 0, to_screen, (100.0, 100.0), 12.0),
        Some((2, 0))
    );
    assert_eq!(
        end_at(&items, 0, to_screen, (101.0, 119.0), 12.0),
        Some((2, 2))
    );
    assert_eq!(
        end_at(&items, 0, to_screen, (119.0, 101.0), 12.0),
        Some((2, 3))
    );
    assert_eq!(end_at(&items, 0, to_screen, (110.0, 110.0), 4.0), None);
    // Dragging a corner keeps the opposite one.
    let mut pts = vec![(50.0, 50.0), (60.0, 60.0)];
    measure::move_handle(Tool::Rect, &mut pts, 2, (45.0, 70.0));
    assert_eq!(pts, [(45.0, 50.0), (60.0, 70.0)]);
    measure::move_handle(Tool::Ellipse, &mut pts, 3, (65.0, 40.0));
    assert_eq!(pts, [(45.0, 40.0), (65.0, 70.0)]);
    measure::move_handle(Tool::Rect, &mut pts, 1, (66.0, 71.0));
    assert_eq!(pts, [(45.0, 40.0), (66.0, 71.0)]);
    // A corner dropped onto a neighbour leaves no width: the region goes.
    let squashed = vec![(45.0, 40.0), (45.5, 71.0)];
    assert!(dropped_on_another(
        &measure::handles(Tool::Rect, &squashed),
        1,
        to_screen,
        4.0
    ));
    assert!(!dropped_on_another(
        &measure::handles(Tool::Rect, &pts),
        1,
        to_screen,
        4.0
    ));
    assert_eq!(
        end_at(&items, 1, to_screen, (21.0, 21.0), 12.0),
        Some((3, 0))
    );
    assert_eq!(end_at(&items, 2, to_screen, (21.0, 21.0), 12.0), None);
    // Dropped on the other end (within 4 px on screen): gone; anywhere else: kept.
    assert!(dropped_on_another(
        &[(40.0, 10.0), (41.0, 10.0)],
        1,
        to_screen,
        4.0
    ));
    assert!(!dropped_on_another(
        &[(40.0, 10.0), (43.0, 10.0)],
        1,
        to_screen,
        4.0
    ));
    assert!(dropped_on_another(
        &[(12.0, 10.0), (20.0, 30.0), (20.5, 30.0)],
        2,
        to_screen,
        4.0
    ));
    assert!(!dropped_on_another(&[(40.0, 10.0)], 3, to_screen, 4.0));
}

/// One file per slice (a CT series in series mode): stepping from slice 40 to 41 and back
/// keeps the window, and slice 40's length comes back with it (41 starts without one).
/// Another series starts from its own default, unless the window is kept by setting.
#[test]
fn window_and_measurements_stay_with_the_series() {
    use std::path::PathBuf;
    let p = |k: u32| PathBuf::from(format!("/ct/IM{k:04}.dcm"));
    let uid = Some("1.2.3".to_owned());
    let window = WlWindow {
        center: 60.0,
        width: 120.0,
        shape: VoiShape::Linear,
        invert: false,
    };
    let length = Measurement {
        tool: Tool::Length,
        points: vec![(1.0, 1.0), (5.0, 4.0)],
        frame: 0,
        lines: Vec::new(),
    };
    let mut m = Medical::default().moved(Some(40), Some(p(40)), uid.clone(), false);
    m.frame = Some(small_ct(Vec::new()));
    m.window = Some(window);
    m.measurements.push(length.clone());
    // To slice 41: same window, no measurements.
    let mut m = m.moved(Some(41), Some(p(41)), uid.clone(), false);
    assert_eq!(m.window, Some(window));
    assert!(m.measurements.is_empty());
    m.frame = Some(small_ct(Vec::new()));
    // Back to 40: its length again.
    let mut m = m.moved(Some(40), Some(p(40)), uid.clone(), false);
    assert_eq!(m.window, Some(window));
    assert_eq!(m.measurements.len(), 1);
    assert_eq!(m.measurements[0].points, length.points);
    m.frame = Some(small_ct(Vec::new()));
    // Another series: the file's default, and slice 40's length is not carried there.
    let mut other = m.moved(Some(7), Some(p(7)), Some("9.9".into()), false);
    assert_eq!(other.window, None);
    assert!(other.measurements.is_empty() && other.kept.is_empty());
    // Outside series mode each file starts afresh, as before; the setting keeps the window.
    other.frame = Some(small_ct(Vec::new()));
    other.window = Some(window);
    let mut alone = other.moved(Some(8), Some(p(8)), None, false);
    assert_eq!(alone.window, None);
    alone.frame = Some(small_ct(Vec::new()));
    alone.window = Some(window);
    assert_eq!(
        alone.moved(Some(9), Some(p(9)), None, true).window,
        Some(window)
    );
}

/// Cine over a series of single-frame files keeps playing from file to file of the series,
/// and stops in another series; a multi-frame file's own cine stops at the next file.
#[test]
fn series_cine_follows_the_files() {
    use std::path::PathBuf;
    let p = |k: u32| PathBuf::from(format!("/ct/IM{k:04}.dcm"));
    let uid = Some("1.2.3".to_owned());
    let due = Instant::now();
    let mut m = Medical::default().moved(Some(1), Some(p(1)), uid.clone(), false);
    m.cine = Some(due);
    m.cine_series = true;
    let mut m = m.moved(Some(2), Some(p(2)), uid.clone(), false);
    assert_eq!((m.cine, m.cine_series), (Some(due), true));
    let other = m.moved(Some(9), Some(p(9)), Some("4.5.6".into()), false);
    assert_eq!((other.cine, other.cine_series), (None, false));
    let mut m = Medical::default().moved(Some(1), Some(p(1)), uid.clone(), false);
    m.cine = Some(due);
    assert_eq!(m.moved(Some(2), Some(p(2)), uid, false).cine, None);
}

/// `Shift+M` keeps what it cleared for the toast's Undo, on that image only.
#[test]
fn cleared_measurements_stay_with_their_image() {
    use std::path::PathBuf;
    let p = |k: u32| PathBuf::from(format!("/ct/IM{k:04}.dcm"));
    let mut m = Medical::default().moved(Some(1), Some(p(1)), Some("1".into()), false);
    m.cleared.push(measure::Measurement {
        tool: measure::Tool::Length,
        points: vec![(1.0, 1.0), (5.0, 4.0)],
        frame: 0,
        lines: Vec::new(),
    });
    assert!(
        m.moved(Some(2), Some(p(2)), Some("1".into()), false)
            .cleared
            .is_empty()
    );
}
