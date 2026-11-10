//! The DICOM test corpus (`tools/corpus/fetch-dicom.sh`): read in place from
//! `SPECULAR_DICOM_CORPUS`, else `tools/corpus/dicom` of this checkout; skipped when absent.
//! Every file must parse or fail cleanly, and every frame we accept must decode and render.
//! Pixel-exact comparisons against pydicom live in the oracle script, not here; this pins a
//! few values that oracle confirmed.

use std::path::{Path, PathBuf};

use specular_dicom::{Dicom, Error, Pixels, Rendered, VoiSource};

fn corpus() -> Option<PathBuf> {
    let p = std::env::var_os("SPECULAR_DICOM_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/dicom")
        });
    p.is_dir().then_some(p)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dcm")) {
            out.push(p);
        }
    }
}

fn find(root: &Path, name: &str) -> Option<Vec<u8>> {
    let mut all = Vec::new();
    walk(root, &mut all);
    all.into_iter()
        .find(|p| p.file_name().is_some_and(|n| n == name))
        .and_then(|p| std::fs::read(p).ok())
}

#[test]
fn every_file_parses_or_fails_cleanly() {
    let Some(root) = corpus() else {
        eprintln!("DICOM corpus absent; skipping");
        return;
    };
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let (mut decoded, mut refused) = (0, 0);
    for p in &files {
        let Ok(data) = std::fs::read(p) else { continue };
        let Ok(d) = Dicom::parse(&data) else { continue };
        let _ = d.summary(&data);
        let _ = d.icon(&data);
        let n = d.frame_count();
        for k in [0, n.saturating_sub(1)].into_iter().take(n.min(2) as usize) {
            match d.modality_frame(&data, k, None) {
                Ok(m) => {
                    let img = m
                        .render_default()
                        .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                    assert_eq!(
                        (img.width, img.height),
                        (m.width(), m.height()),
                        "{}",
                        p.display()
                    );
                    decoded += 1;
                }
                Err(
                    Error::Unsupported(_)
                    | Error::Codec(_)
                    | Error::Malformed(_)
                    | Error::Truncated,
                ) => refused += 1,
                Err(e) => panic!("{}: unexpected {e}", p.display()),
            }
        }
    }
    eprintln!(
        "{} files, {decoded} frames decoded, {refused} refused",
        files.len()
    );
    assert!(files.is_empty() || decoded > 0);
}

#[test]
fn known_values() {
    let Some(root) = corpus() else { return };
    // CT, signed 16-bit, rescale to HU, no window (min/max).
    if let Some(data) = find(&root, "CT_small.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let s = d.summary(&data);
        assert_eq!(s.modality.as_deref(), Some("CT"));
        assert_eq!((s.columns, s.rows), (128, 128));
        let m = d.modality_frame(&data, 0, None).unwrap();
        assert_eq!(m.unit(), Some("HU"));
        assert_eq!(m.stored_at(0, 0), Some(175));
        assert_eq!(m.value_at(0, 0), Some(175.0 - 1024.0));
        assert_eq!(m.default_voi().1, VoiSource::MinMax);
        assert!(matches!(
            m.render_default().unwrap().pixels,
            Rendered::Gray16(_)
        ));
    }
    // Multi-frame RLE colour, frame access through the offset table.
    if let Some(data) = find(&root, "SC_rgb_rle_2frame.dcm") {
        let d = Dicom::parse(&data).unwrap();
        assert_eq!(d.frame_count(), 2);
        let f = d.decode_frame(&data, 1, None).unwrap();
        let Pixels::Rgb8(v) = &f.pixels else {
            panic!("not RGB8")
        };
        // Frame 2 is the colour inverse of frame 1: its first row is cyan-ish.
        assert_eq!(&v[..3], &[0, 255, 255]);
    }
    // JPEG-LS: signed lossless, multi-frame, and near-lossless sample-interleaved RGB (values
    // from pydicom + CharLS).
    if let Some(data) = find(&root, "MR_small_jpeg_ls_lossless.dcm") {
        let d = Dicom::parse(&data).unwrap();
        d.check_supported().unwrap();
        let m = d.modality_frame(&data, 0, None).unwrap();
        assert_eq!(m.stored_at(0, 0), Some(905));
        assert_eq!(m.stored_at(63, 63), Some(862));
    }
    if let Some(data) = find(&root, "emri_small_jpeg_ls_lossless.dcm") {
        let d = Dicom::parse(&data).unwrap();
        assert_eq!(d.frame_count(), 10);
        let m = d.modality_frame(&data, 9, None).unwrap();
        assert_eq!(m.stored_at(63, 63), Some(147));
    }
    // HTJ2K (.201 lossless RCT, .203 lossy ICT), values from OpenJPEG; the lossy frame is
    // within 1 of it.
    let htj2k = [
        (
            "HTJ2KLossless_08_RGB.dcm",
            0,
            [[160, 121, 53], [168, 136, 79]],
        ),
        ("HTJ2K_08_RGB.dcm", 1, [[159, 120, 51], [168, 135, 78]]),
    ];
    for (name, tol, want) in htj2k {
        let Some(data) = find(&root, name) else {
            continue;
        };
        let d = Dicom::parse(&data).unwrap();
        d.check_supported().unwrap();
        let f = d.decode_frame(&data, 0, None).unwrap();
        assert_eq!((f.width, f.height), (640, 480), "{name}");
        assert_eq!(f.bits, 8, "{name}");
        let v: Vec<u16> = match &f.pixels {
            Pixels::Rgb8(v) => v.iter().map(|&x| x as u16).collect(),
            Pixels::Rgb16(v) => v.clone(),
            Pixels::Grey(_) => panic!("{name}: not RGB"),
        };
        for (i, want) in [0, 240 * 640 + 320].into_iter().zip(want) {
            for (c, w) in want.into_iter().enumerate() {
                let got = v[i * 3 + c];
                assert!(
                    got.abs_diff(w) <= tol,
                    "{name} sample {i}.{c}: {got} vs {w}"
                );
            }
        }
    }
    if let Some(data) = find(&root, "SC_rgb_jls_lossy_sample.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let f = d.decode_frame(&data, 0, None).unwrap();
        let Pixels::Rgb8(v) = &f.pixels else {
            panic!("not RGB8")
        };
        assert_eq!(&v[..4], &[255, 0, 0, 255]);
    }
}

/// Float and Double Float Pixel Data (parametric maps): values from pydicom's `pixel_array`.
#[test]
fn float_pixel_data() {
    let Some(root) = corpus() else { return };
    let cases = [
        (
            "parametric_map_float.dcm",
            [
                0.920127809047699,
                0.12003651261329651,
                0.5851209759712219,
                0.4815152883529663,
            ],
            0.9415791630744934,
        ),
        (
            "parametric_map_double_float.dcm",
            [
                0.9201277955271565,
                0.12003651300775897,
                0.5851209493382017,
                0.4815152898219991,
            ],
            0.9415791875855773,
        ),
    ];
    for (name, want, max) in cases {
        let Some(data) = find(&root, name) else {
            continue;
        };
        let d = Dicom::parse(&data).unwrap();
        assert!(d.pixel.as_ref().unwrap().float, "{name}");
        let m = d.modality_frame(&data, 0, None).unwrap();
        assert_eq!((m.width(), m.height()), (128, 128));
        for (&(x, y), w) in [(0, 0), (64, 64), (127, 127), (32, 42)].iter().zip(want) {
            assert_eq!(m.value_at(x, y), Some(w), "{name} at {x},{y}");
        }
        assert_eq!(m.value_range(), Some((0.0, max)), "{name}");
        // The file's window 0.5 / 1 (LINEAR, read as LINEAR_EXACT for floats) spans 0..1.
        let (voi, src) = m.default_voi();
        assert_eq!(src, VoiSource::Window);
        assert_eq!(
            m.voi_function(),
            specular_dicom::display::VoiFunction::LinearExact
        );
        let img = m.render(&voi).unwrap();
        let Rendered::Gray16(p) = &img.pixels else {
            panic!("{name}: not grey")
        };
        let expect = (0.920127809047699 * 65535.0) as i64;
        assert!((p[0] as i64 - expect).abs() < 8, "{name}: {}", p[0]);
        let f = m.values_f32().unwrap();
        assert_eq!(f[64 * 128 + 64], 0.120_036_51_f32, "{name}");
    }
}

/// Overlays, ultrasound regions, spacing kinds and frame timing (values from pydicom's
/// `overlay_array` and data elements; only structure, never patient attributes).
#[test]
fn graphics_calibration_and_timing() {
    use specular_dicom::{SpacingKind, graphics::OVERLAY};
    let Some(root) = corpus() else { return };
    if let Some(data) = find(&root, "MR-SIEMENS-DICOM-WithOverlays.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let m = d.modality_frame(&data, 0, None).unwrap();
        let g = m.graphics().expect("an overlay");
        assert_eq!((g.overlays, g.shutter), (1, false));
        let set: Vec<usize> = (0..g.mask.len())
            .filter(|&i| g.mask[i] & OVERLAY != 0)
            .collect();
        assert_eq!(set.len(), 323);
        // First set pixel: row 136, column 420.
        assert_eq!(set[0], 136 * 484 + 420);
        // Burnt in white; without overlays the pixel keeps its grey.
        let voi = m.default_voi().0;
        let (Rendered::Gray16(on), Rendered::Gray16(off)) = (
            m.render(&voi).unwrap().pixels,
            m.render_with(&voi, false).unwrap().pixels,
        ) else {
            panic!("grey")
        };
        assert_eq!(on[set[0]], 65535);
        assert_ne!(off[set[0]], 65535);
    }
    // Overlay planes without rows / columns are ignored.
    if let Some(data) = find(&root, "JPEGLosslessP14SV1_1s_1f_u_16_16.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let m = d.modality_frame(&data, 0, None).unwrap();
        assert!(m.graphics().is_none());
    }
    if let Some(data) = find(&root, "OBXXXX1A.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let m = d.modality_frame(&data, 0, None).unwrap();
        let c = m.calibration();
        assert_eq!(c.spacing, None);
        assert_eq!(c.regions.len(), 2);
        let r = c.regions[0];
        assert_eq!((r.x0, r.y0, r.x1, r.y1, r.format), (120, 60, 800, 518, 1));
        let (dx, dy) = r.spacing.unwrap();
        assert!((dx - 0.262_287_876_619_699_8).abs() < 1e-12 && dx == dy);
        // Region 2 is a spectral trace (units 4 / 0): no spatial spacing.
        assert_eq!(c.regions[1].spacing, None);
        let s = c.for_points(&[(200.0, 100.0), (300.0, 400.0)]).unwrap();
        assert_eq!(s.kind, SpacingKind::Region);
        assert!(c.for_points(&[(200.0, 100.0), (300.0, 540.0)]).is_err());
    }
    if let Some(data) = find(&root, "color3d_jpeg_baseline.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let t = d.frame_durations(&data).unwrap();
        assert_eq!(t.len(), 120);
        assert_eq!(t[0], std::time::Duration::from_micros(33_333));
    }
    if let Some(data) = find(&root, "JPEGLosslessP14SV1_3s_2f_u_08_08.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let t = d.frame_durations(&data).unwrap();
        // Frame Time wins over Cine Rate.
        assert_eq!(t[1].as_micros(), 31_965);
    }
    if let Some(data) = find(&root, "emri_small.dcm") {
        let d = Dicom::parse(&data).unwrap();
        assert_eq!(d.frame_durations(&data), None);
    }
    // DX with Pixel Spacing equal to Imager Pixel Spacing: the detector's.
    if let Some(data) = find(&root, "966.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let s = d
            .modality_frame(&data, 0, None)
            .unwrap()
            .calibration()
            .spacing;
        let s = s.unwrap();
        assert_eq!((s.x, s.y, s.kind), (0.16, 0.16, SpacingKind::Detector));
    }
    // Imager Pixel Spacing only.
    if let Some(data) = find(&root, "JPEG2KLossless_1s_1f_u_16_16.dcm") {
        let d = Dicom::parse(&data).unwrap();
        let s = d
            .modality_frame(&data, 0, None)
            .unwrap()
            .calibration()
            .spacing;
        assert_eq!(s.map(|s| (s.x, s.kind)), Some((0.4, SpacingKind::Detector)));
    }
}

/// The MESA CPI presentation-state test cases (`mesa-cpi` of the corpus, or
/// `SPECULAR_DICOM_MESA`): modality values and VOI outputs of each state applied to its
/// image, from pydicom (the state's Modality LUT / rescale, then its Softcopy VOI LUT or
/// window; LUT first values read as signed for signed input).
#[test]
fn mesa_presentation_states() {
    use specular_dicom::gsps::{PresentationState, Shape};
    let root = std::env::var_os("SPECULAR_DICOM_MESA")
        .map(PathBuf::from)
        .or_else(|| corpus().map(|c| c.join("mesa-cpi")))
        .filter(|p| p.is_dir());
    let Some(root) = root else {
        eprintln!("MESA CPI cases absent; skipping");
        return;
    };
    type Case = (&'static str, &'static [(u32, u32, f64, Option<f64>)]);
    let cases: &[Case] = &[
        (
            "vlut_p02",
            &[
                (0, 0, 127.0, Some(0.49804)),
                (100, 37, 130.0, Some(0.50980)),
                (511, 300, 191.0, Some(0.74902)),
                (17, 480, 0.0, Some(0.0)),
            ],
        ),
        (
            "vlut_p04",
            &[
                (0, 0, 127.0, Some(0.49804)),
                (511, 300, 191.0, Some(0.74902)),
            ],
        ),
        (
            "vlut_p05",
            &[
                (0, 0, 127.0, Some(0.50196)),
                (100, 37, 124.0, Some(0.51373)),
                (511, 300, 63.0, Some(0.75294)),
                (17, 480, 255.0, Some(0.0)),
            ],
        ),
        (
            "vlut_p07",
            &[
                (0, 0, -1.0, Some(0.49988)),
                (100, 37, 40.0, Some(0.50989)),
                (511, 300, 1023.0, Some(0.74994)),
                (17, 480, -2048.0, Some(0.0)),
            ],
        ),
        (
            "vlut_p09",
            &[
                (0, 0, -1.0, Some(0.49987)),
                (100, 37, 40.0, Some(0.50990)),
                (511, 300, 1023.0, Some(0.74994)),
                (17, 480, -2048.0, Some(0.0)),
            ],
        ),
        (
            "vlut_p10",
            &[
                (0, 0, -1.0, Some(0.50013)),
                (100, 37, -42.0, Some(0.51014)),
                (511, 300, -1025.0, Some(0.75019)),
                (17, 480, 2047.0, Some(0.0)),
            ],
        ),
        (
            "xlut_p02",
            &[
                (0, 0, 1.0, Some(0.50018)),
                (100, 37, -81.0, Some(0.49017)),
                (511, 300, -2047.0, Some(0.25015)),
                (17, 480, 4095.0, Some(1.0)),
            ],
        ),
        (
            "xlut_p03",
            &[
                (0, 0, 134.0, Some(0.61176)),
                (100, 37, 67.0, Some(0.4)),
                (511, 300, 255.0, Some(1.0)),
            ],
        ),
        (
            "mlut_p04",
            &[(0, 0, 1023.0, None), (17, 480, -1024.0, None)],
        ),
        ("mlut_p16", &[(0, 0, -0.5, None)]),
        ("mlut_p18", &[(0, 0, 32759.0, None)]),
        ("mlut_p19", &[(0, 0, 32776.0, None)]),
    ];
    let mut checked = 0;
    for (name, points) in cases {
        let (Ok(pre), Ok(img)) = (
            std::fs::read(root.join(format!("{name}.pre"))),
            std::fs::read(root.join(format!("{name}.dcm"))),
        ) else {
            continue;
        };
        let ps = PresentationState::parse(&pre).unwrap_or_else(|e| panic!("{name}: {e}"));
        let d = Dicom::parse(&img).unwrap();
        let m = d.modality_frame(&img, 0, None).unwrap();
        let sop = m.sop_instance_uid().unwrap().to_owned();
        assert!(ps.refers_to(&sop, 0), "{name}");
        let m = m.presented(&ps, 0);
        assert_eq!(m.inverted(), ps.shape == Some(Shape::Inverse), "{name}");
        let voi = m.default_voi().0;
        for &(x, y, value, fraction) in *points {
            let v = m.value_at(x, y).unwrap();
            assert!(
                (v - value).abs() < 1e-9,
                "{name} at {x},{y}: {v} != {value}"
            );
            if let Some(f) = fraction {
                let got = voi.apply(v);
                assert!((got - f).abs() < 2e-3, "{name} at {x},{y}: {got} != {f}");
            }
        }
        m.render_default().unwrap();
        checked += 1;
    }
    // Every state of the set parses, and refers to an image of the set.
    let mut states = 0;
    for e in std::fs::read_dir(&root).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "pre") {
            let data = std::fs::read(&p).unwrap();
            let ps =
                PresentationState::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(!ps.references.is_empty(), "{}", p.display());
            states += 1;
        }
    }
    eprintln!("{checked} MESA cases checked, {states} states parsed");
    assert!(checked > 0);
}

/// The MESA CPI shutter and overlay cases against their `.res` reference renderings (8-bit,
/// same geometry as the image: these states neither rotate nor zoom): every display shutter
/// shape and both shutter values, bitmap shutters from the state's own overlay, and image
/// overlays (embedded in pixel bits and in Overlay Data) plus the state's own, activated on
/// graphic layers. Shuttered pixels (away from the shape edges, which renderers round
/// differently) must match exactly, overlay pixels must be lit in the reference (which draws
/// its text anti-aliased), and the rest of the image within one grey level but for the
/// references' own text.
#[test]
fn mesa_shutters_and_overlays_match_reference() {
    use specular_dicom::{graphics::SHUTTER, gsps::PresentationState};
    let root = std::env::var_os("SPECULAR_DICOM_MESA")
        .map(PathBuf::from)
        .or_else(|| corpus().map(|c| c.join("mesa-cpi")))
        .filter(|p| p.is_dir());
    let Some(root) = root else {
        eprintln!("MESA CPI cases absent; skipping");
        return;
    };
    let names = [
        "dish_p01", "dish_p02", "dish_p03", "dish_p04", "dish_p05", "dish_p06", "dish_p07",
        "dish_p08", "dish_p09", "dish_p10", "ovly_p01",
    ];
    let mut checked = 0;
    for name in names {
        let (Ok(pre), Ok(img), Ok(res)) = (
            std::fs::read(root.join(format!("{name}.pre"))),
            std::fs::read(root.join(format!("{name}.dcm"))),
            std::fs::read(root.join(format!("{name}.res"))),
        ) else {
            continue;
        };
        let ps = PresentationState::parse(&pre).unwrap_or_else(|e| panic!("{name}: {e}"));
        let d = Dicom::parse(&img).unwrap();
        let m = d.modality_frame(&img, 0, None).unwrap().presented(&ps, 0);
        let g = m
            .graphics()
            .unwrap_or_else(|| panic!("{name}: no graphics"));
        let out = m.render_default().unwrap();
        let Rendered::Gray16(out) = out.pixels else {
            panic!("{name}: not grey");
        };
        let r = Dicom::parse(&res).unwrap();
        let rf = r.modality_frame(&res, 0, None).unwrap();
        assert_eq!((rf.width(), rf.height()), (m.width(), m.height()), "{name}");
        let (mut shut, mut shut_ok, mut ink, mut ink_ok) = (0u32, 0u32, 0u32, 0u32);
        let (mut plain, mut plain_ok) = (0u32, 0u32);
        // Within two pixels of a shutter edge, where renderers may round the shapes
        // differently (the circle cases put a bright ring right on the radius).
        let edge = |x: u32, y: u32| {
            let here = g.at(x, y) & SHUTTER;
            (-2i32..=2).any(|dy| {
                (-2i32..=2).any(|dx| {
                    let (u, v) = (x as i32 + dx, y as i32 + dy);
                    u >= 0
                        && v >= 0
                        && (u as u32) < m.width()
                        && (v as u32) < m.height()
                        && g.at(u as u32, v as u32) & SHUTTER != here
                })
            })
        };
        // The references' bottom row is a grey ramp, not the image.
        for y in 0..m.height() - 1 {
            for x in 0..m.width() {
                if edge(x, y) {
                    continue;
                }
                let i = (y * m.width() + x) as usize;
                let ours = (u32::from(out[i]) * 255 + 32767) / 65535;
                let theirs = rf.value_at(x, y).unwrap() as u32;
                let m_bits = g.at(x, y);
                if g.ink(m_bits).is_some() {
                    // The references draw the overlay text anti-aliased: an overlay pixel is
                    // white with us and lit there.
                    ink += 1;
                    ink_ok += u32::from(ours == 255 && theirs >= 96);
                } else if m_bits & SHUTTER != 0 {
                    shut += 1;
                    shut_ok += u32::from(ours == theirs);
                } else {
                    plain += 1;
                    plain_ok += u32::from(ours.abs_diff(theirs) <= 1);
                }
            }
        }
        eprintln!(
            "{name}: shutter {shut_ok}/{shut} exact, overlay {ink_ok}/{ink} lit, others \
             {plain_ok}/{plain} within 1"
        );
        assert!(shut + ink > 0, "{name}");
        assert_eq!(shut_ok, shut, "{name}");
        assert!(
            f64::from(ink_ok) >= 0.97 * f64::from(ink),
            "{name}: {ink_ok}/{ink}"
        );
        // The references' own text (a label, or anti-aliased overlay text, a few thousand
        // pixels) sits a pixel or so from the images' copy.
        assert!(plain - plain_ok <= 4000, "{name}: {plain_ok}/{plain}");
        if name == "ovly_p01" {
            // Two overlays in pixel bits, two in Overlay Data, two of the state's own.
            assert_eq!(g.overlays, 6);
        }
        checked += 1;
    }
    eprintln!("{checked} MESA shutter/overlay references compared");
}
