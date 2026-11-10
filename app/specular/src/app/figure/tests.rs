//! Figures and key images without a GPU: the dialog's rows, the scale bar through rotations,
//! and a teaching file checked for patient details and burned-in text.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use specular_dicom::dict::Vr;
use specular_dicom::mask::Rect as Area;
use specular_dicom::write::{self, FileMeta, Object};
use specular_figure::Rgb16;
use specular_measure::calib::{Calibration, Source};

use super::build::{self, KeyImage, Measured};
use super::capture;
use super::*;

const NAME: &str = "DOE^JANE";
const PID: &str = "PID-12345";
const W: u32 = 96;
const H: u32 = 64;
/// The burned-in text: a bright block, top left.
const TEXT: Area = Area {
    x: 4,
    y: 3,
    w: 40,
    h: 9,
};

fn grey(x: u32, y: u32) -> u8 {
    if in_text(x, y) && !(x + y).is_multiple_of(3) {
        250
    } else {
        (40 + (x * 3 + y * 2) % 60) as u8
    }
}

fn in_text(x: u32, y: u32) -> bool {
    (TEXT.x..TEXT.x + TEXT.w).contains(&x) && (TEXT.y..TEXT.y + TEXT.h).contains(&y)
}

/// A CT image with the patient's name and id, and its burned-in text declared.
fn dicom(burned_in: bool) -> Vec<u8> {
    let mut o = Object::default();
    o.put_text(0x0008_0016, Vr::UI, "1.2.840.10008.5.1.4.1.1.2");
    o.put_text(0x0008_0018, Vr::UI, "1.2.826.0.1.3680043.10.99.1");
    o.put_text(0x0008_0060, Vr::CS, "CT");
    o.put_text(0x0010_0010, Vr::PN, NAME);
    o.put_text(0x0010_0020, Vr::LO, PID);
    o.put_text(0x0010_0030, Vr::DA, "19700101");
    o.put_text(0x0020_000D, Vr::UI, "1.2.826.0.1.3680043.10.99.2");
    o.put_text(0x0020_000E, Vr::UI, "1.2.826.0.1.3680043.10.99.3");
    if burned_in {
        o.put_text(0x0028_0301, Vr::CS, "YES");
    }
    o.put_bytes(0x0028_0002, Vr::US, 1u16.to_le_bytes());
    o.put_text(0x0028_0004, Vr::CS, "MONOCHROME2");
    o.put_bytes(0x0028_0010, Vr::US, (H as u16).to_le_bytes());
    o.put_bytes(0x0028_0011, Vr::US, (W as u16).to_le_bytes());
    o.put_bytes(0x0028_0100, Vr::US, 8u16.to_le_bytes());
    o.put_bytes(0x0028_0101, Vr::US, 8u16.to_le_bytes());
    o.put_bytes(0x0028_0102, Vr::US, 7u16.to_le_bytes());
    o.put_bytes(0x0028_0103, Vr::US, 0u16.to_le_bytes());
    let px: Vec<u8> = (0..H)
        .flat_map(|y| (0..W).map(move |x| grey(x, y)))
        .collect();
    o.put_bytes(0x7FE0_0010, Vr::OB, px);
    write::part10(&o, &FileMeta::of(&o, write::EXPLICIT_LE)).unwrap()
}

/// The key image of `path` as the viewer would capture it at 1:1 (the window is identity).
fn key(path: &Path) -> KeyImage {
    let rgb: Vec<u8> = (0..H)
        .flat_map(|y| (0..W).flat_map(move |x| [grey(x, y); 3]))
        .collect();
    KeyImage {
        path: path.to_owned(),
        frame: 0,
        title: format!("{NAME} · CT HEAD · Image 1"),
        anon_title: "CT".into(),
        px: Rgb16::from_rgb8(W, H, &rgb),
        from_stored: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        stored: (W, H),
        measured: vec![Measured {
            outline: vec![(50.0, 40.0), (90.0, 40.0)],
            closed: false,
            label: "20.0 mm".into(),
            lines: vec!["1. Line: 20.0 mm".into()],
        }],
        dicom: true,
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/test-tmp/figure")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

fn png_grey(png: &[u8]) -> (u32, u32, Vec<u8>) {
    let img = specular_png::decode(png, &specular_png::DecodeOptions::default()).unwrap();
    let rgba = img.to_rgba8();
    (
        img.width,
        img.height,
        rgba.chunks_exact(4).map(|p| p[0]).collect(),
    )
}

#[test]
fn rows_say_where_values_come_from_and_why_parts_are_off() {
    let a = Avail {
        calibration: Some("DICOM Pixel Spacing".into()),
        legend: None,
        inset: false,
        split: true,
        marks: 2,
        full_res: true,
        zoomed_in: true,
        medical: false,
        keys: 0,
    };
    let rows = figure_rows(&FigureChoices::default(), &a, None);
    let row = |r: Row| rows.iter().find(|v| v.row == r).unwrap();
    assert!(row(Row::ScaleBar).value.contains("DICOM Pixel Spacing"));
    assert!(row(Row::ScaleBar).enabled);
    assert!(!row(Row::ColourBar).enabled);
    assert_eq!(row(Row::ColourBar).value, t!("figure-colour-bar-none"));
    assert!(!row(Row::Inset).enabled);
    assert_eq!(row(Row::Title).kind, RowKind::Text);
    assert_eq!(row(Row::File).kind, RowKind::Action);
    let none = Avail {
        calibration: None,
        ..a.clone()
    };
    let rows = figure_rows(&FigureChoices::default(), &none, None);
    let sb = rows.iter().find(|v| v.row == Row::ScaleBar).unwrap();
    assert!(!sb.enabled);
    assert_eq!(sb.value, t!("figure-scale-bar-none"));
    // Key images: the teaching rows only for medical images, and only with the preset.
    let k = KeyChoices {
        teaching: true,
        ..KeyChoices::default()
    };
    let med = Avail {
        medical: true,
        keys: 3,
        ..Avail::default()
    };
    let rows = key_rows(&k, &med, None);
    assert!(
        rows.iter()
            .any(|r| r.row == Row::Notes && r.kind == RowKind::Text)
    );
    assert!(rows.iter().any(|r| r.row == Row::Masks));
    let rows = key_rows(
        &k,
        &Avail {
            keys: 3,
            ..Avail::default()
        },
        None,
    );
    assert!(
        !rows
            .iter()
            .any(|r| matches!(r.row, Row::Notes | Row::Masks | Row::Preset))
    );
    // Info rows are not in the focus order.
    let v = DialogView {
        title: String::new(),
        rows: key_rows(&k, &med, None),
        note: None,
        status: None,
        busy: false,
        can_export: true,
        focus: None,
        preedit: String::new(),
    };
    assert!(!v.focusables().contains(&Hit::Row(Row::Images)));
}

#[test]
fn typed_text_is_kept_short_and_printable() {
    let mut s = String::new();
    push_text(&mut s, "Fig. 1\u{7}\n — שלום");
    assert_eq!(s, "Fig. 1 — שלום");
    push_text(&mut s, &"x".repeat(1000));
    assert_eq!(s.chars().count(), MAX_TEXT);
}

/// The scale bar measures along the panel's horizontal whatever the rotation: rows 0.5 mm
/// apart, columns 0.25 mm; turned a quarter, the panel's horizontal runs along the rows.
#[test]
fn scale_bar_follows_the_orientation() {
    let cal = Calibration::dicom(0.5, 0.25, Source::DicomPixelSpacing).unwrap();
    let upright: build::Map = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let s = build::scale_spec(&cal, &upright).unwrap();
    assert!((s.per_px - 0.25).abs() < 1e-12 && s.unit == "mm" && !s.approximate);
    // Panel (x, y) → stored (y, W − x): a quarter turn.
    let turned: build::Map = [0.0, 1.0, 0.0, -1.0, 0.0, 512.0];
    let s = build::scale_spec(&cal, &turned).unwrap();
    assert!((s.per_px - 0.5).abs() < 1e-12);
    // At 2× the panel has two px per stored px.
    let zoomed: build::Map = [0.5, 0.0, 0.0, 0.0, 0.5, 0.0];
    assert!((build::scale_spec(&cal, &zoomed).unwrap().per_px - 0.125).abs() < 1e-12);
    assert!(build::scale_spec(&Calibration::uncalibrated(), &upright).is_none());
    // The inverse map round-trips.
    let m: build::Map = [0.0, 2.0, 5.0, -2.0, 0.0, 9.0];
    let p = build::apply(&build::invert(&m), build::apply(&m, (3.0, 7.0)));
    assert!((p.0 - 3.0).abs() < 1e-12 && (p.1 - 7.0).abs() < 1e-12);
}

#[test]
fn marks_map_into_the_panel() {
    let ms = vec![Measured {
        outline: vec![(10.0, 10.0), (20.0, 10.0), (20.0, 20.0)],
        closed: true,
        label: "1 cm²".into(),
        lines: Vec::new(),
    }];
    let m = build::marks(&ms, &[2.0, 0.0, -5.0, 0.0, 2.0, 1.0]);
    assert_eq!(m.len(), 1);
    assert_eq!(
        m[0].path,
        vec![
            specular_figure::El::Move([15.0, 21.0]),
            specular_figure::El::Line([35.0, 21.0]),
            specular_figure::El::Line([35.0, 41.0]),
            specular_figure::El::Close,
        ]
    );
    assert_eq!(m[0].label.as_deref(), Some("1 cm²"));
}

/// A teaching file has nothing about the patient in it, and the burned-in text is black in
/// its pictures and in its DICOM copies.
#[test]
fn teaching_files_leave_the_patient_out() {
    let dir = scratch("teaching");
    let src = dir.join(format!("{NAME}.dcm"));
    std::fs::write(&src, dicom(true)).unwrap();
    let k = key(&src);
    let session = specular_dicom::anon::Session::new();
    let user = ((W, H), vec![TEXT]);
    let (areas, copy) = capture::teaching_one(&k, &user, &session, true).expect("kept");
    assert_eq!(areas, vec![TEXT]);
    let copy = copy.expect("an anonymised copy");
    // The copy: no name or id, the text zeroed.
    assert!(!contains(&copy, NAME) && !contains(&copy, PID));
    let d = specular_dicom::Dicom::parse(&copy).unwrap();
    let pd = d.dataset.get(0x7FE0_0010).unwrap();
    let px = &copy[pd.value.clone()];
    for y in TEXT.y..TEXT.y + TEXT.h {
        for x in TEXT.x..TEXT.x + TEXT.w {
            assert_eq!(px[(y * W + x) as usize], 0, "copy {x},{y}");
        }
    }
    // Pictures: the text area black, the rest as it was.
    let mut kk = k.clone();
    kk.anon_title = "CT · Key image 1".into();
    let with = vec![(&kk, areas.clone())];
    let files = build::folder(&with, true, true, "Classic case.\nLook at the left side.").unwrap();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["key-01.png", "notes.txt"]);
    for (n, data) in &files {
        assert!(!contains(data, "DOE") && !contains(data, PID), "{n}");
    }
    let notes = String::from_utf8(files[1].1.clone()).unwrap();
    assert!(notes.starts_with("Classic case."), "{notes}");
    assert!(notes.contains("key-01.png: CT · Key image 1") && notes.contains("1. Line: 20.0 mm"));
    let (w, h, g) = png_grey(&files[0].1);
    assert_eq!((w, h), (W, H));
    for y in 0..H {
        for x in 0..W {
            let v = g[(y * W + x) as usize];
            if in_text(x, y) {
                assert_eq!(v, 0, "png {x},{y}");
            } else if y < 30 {
                // Away from the drawn measurement, the image itself.
                let near = x + 1 >= TEXT.x
                    && x <= TEXT.x + TEXT.w
                    && y + 1 >= TEXT.y
                    && y <= TEXT.y + TEXT.h;
                if !near {
                    assert_eq!(v, grey(x, y), "png {x},{y}");
                }
            }
        }
    }
    // The deck.
    let deck = build::deck(&with, true, true, "Classic case.", "Teaching file").unwrap();
    let bytes = specular_figure::pptx::write(&deck).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut slides = 0;
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().unwrap().into_owned();
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        assert!(!contains(&data, "DOE") && !contains(&data, PID), "{name}");
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slides += 1;
            assert!(contains(&data, "CT · Key image 1"));
        }
        if name == "ppt/notesSlides/notesSlide1.xml" {
            assert!(contains(&data, "Classic case.") && contains(&data, "1. Line: 20.0 mm"));
        }
        if name == "ppt/media/image1.png" {
            let (_, _, g) = png_grey(&data);
            assert_eq!(g[(TEXT.y * W + TEXT.x + 3) as usize], 0);
        }
    }
    assert_eq!(slides, 1);
    // Standard export keeps the title as shown.
    let deck = build::deck(&[(&k, Vec::new())], false, false, "", "Key images").unwrap();
    assert_eq!(deck.slides[0].title, k.title);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Burned-in text declared and nowhere to be found: the image is left out of a teaching
/// file. Found by the detector: masked without the user's help.
#[test]
fn declared_burned_in_text_needs_masks() {
    let dir = scratch("burned");
    let src = dir.join("im.dcm");
    std::fs::write(&src, dicom(true)).unwrap();
    let session = specular_dicom::anon::Session::new();
    let mut k = key(&src);
    // A flat picture: nothing to detect.
    k.px = Rgb16::filled(W, H, [20_000; 3]);
    assert!(capture::teaching_one(&k, &((W, H), Vec::new()), &session, false).is_none());
    // Masks for another size do not apply.
    assert!(capture::teaching_one(&k, &((W + 1, H), vec![TEXT]), &session, false).is_none());
    // Not declared: kept, nothing masked.
    std::fs::write(&src, dicom(false)).unwrap();
    let (areas, _) = capture::teaching_one(&k, &((W, H), Vec::new()), &session, false).unwrap();
    assert!(areas.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn masks_land_on_the_captured_pixels() {
    let p = PathBuf::from("x.dcm");
    let mut k = key(&p);
    // Captured at half size: stored (x, y) → (x / 2, y / 2).
    k.px = Rgb16::filled(W / 2, H / 2, [65_535; 3]);
    k.from_stored = [0.5, 0.0, 0.0, 0.0, 0.5, 0.0];
    let m = build::mask(&k, &[Area::new(10, 10, 20, 10)]);
    assert_eq!(m.px(10, 7), [0; 3]);
    assert_eq!(m.px(4, 4), [0; 3], "one pixel of margin");
    assert_eq!(m.px(20, 20), [65_535; 3]);
    assert_eq!(m.px(3, 3), [65_535; 3]);
}

#[test]
fn key_images_tile_for_the_clipboard() {
    let p = PathBuf::from("x.png");
    let a = key(&p);
    let mut b = key(&p);
    b.px = Rgb16::filled(30, 20, [0, 65_535, 0]);
    b.measured.clear();
    let px = build::tiled(&[(&a, Vec::new()), (&b, Vec::new())], false, 1 << 30);
    assert_eq!((px.width, px.height), (W + 8 + 30, H));
    assert_eq!(px.px(W + 8, 0), [0, 65_535, 0]);
    assert_eq!(px.px(0, 0), a.px.px(0, 0));
    let rgba = build::rgba8(&px);
    assert_eq!(rgba.len() as u32, px.width * px.height * 4);
    // Too many pixels: scaled down.
    let small = build::tiled(&[(&a, Vec::new())], false, 1000);
    assert!(u64::from(small.width) * u64::from(small.height) <= 1100);
}

#[test]
fn dates_for_files() {
    assert_eq!(build::rfc3339(0), "1970-01-01T00:00:00Z");
    assert_eq!(build::rfc3339(1_792_843_200), "2026-10-24T12:00:00Z");
    assert_eq!(build::pdf_date(1_792_843_200), "D:20261024120000Z");
}

/// Raster figures carry their provenance: XMP in the PNG and TIFF, the info dictionary and
/// XMP in the PDF.
#[test]
fn figure_files_carry_the_provenance() {
    use specular_figure::compose::{Panel, Spec, compose};
    let spec = Spec {
        title: Some("Figure 1".into()),
        main: Some(Panel {
            px: Rgb16::filled(64, 48, [30_000; 3]),
            label: None,
        }),
        meta: vec![
            ("file".into(), "cells.tif".into()),
            (
                "calibration".into(),
                "TIFF resolution: 0.5 × 0.5 µm/px".into(),
            ),
        ],
        deep: true,
        ..Spec::default()
    };
    let f = compose(&spec, &mut specular_figure::text::Boxes).unwrap();
    for format in [Format::Svg, Format::Pdf, Format::Png, Format::Tiff] {
        let b = capture::figure_bytes(&f, format, 1_792_843_200).unwrap();
        assert!(contains(&b, "cells.tif"), "{format:?}");
        assert!(contains(&b, "TIFF resolution"), "{format:?}");
        match format {
            Format::Png => {
                assert!(b.starts_with(b"\x89PNG"));
                // 16 bits per sample.
                assert_eq!(b[24], 16);
            }
            Format::Tiff => assert!(b.starts_with(b"II*\0")),
            Format::Pdf => assert!(contains(&b, "/CreationDate (D:20261024120000Z)")),
            Format::Svg => assert!(contains(&b, "<svg")),
        }
    }
}

/// The panels as the viewer reads them back: the visible area at 1:1 holds the source
/// pixels where the view shows them (upright and turned a quarter), and an inset holds the
/// same pixels as its region on the panel, enlarged.
#[test]
fn panels_and_insets_read_back_through_the_display_path() {
    use specular_core::{Orientation, PixelLayout, Raster, RasterPlane};
    use specular_render::{PxRect, RenderOptions, View};
    let Some(h) = specular_shots::Harness::new() else {
        return;
    };
    let gpu = &h.gpu;
    let (w, hh) = (120u32, 80u32);
    let src = |x: u32, y: u32| -> [u8; 3] {
        let v = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
        [(v >> 24) as u8, (v >> 16) as u8, (x * 2) as u8]
    };
    let planes = (0..3)
        .map(|c| RasterPlane {
            width: w,
            height: hh,
            stride: w as usize,
            data: (0..hh)
                .flat_map(|y| (0..w).map(move |x| src(x, y)[c]))
                .collect(),
            h_div: 1,
            v_div: 1,
        })
        .collect();
    let raster = Raster {
        width: w,
        height: hh,
        origin: (0, 0),
        scale: 1,
        layout: PixelLayout::Rgb,
        planes,
        truncated: false,
    };
    let mut r = specular_render::ImageRenderer::new(gpu, 64 << 20);
    let mut img = r.create_image(gpu, 1, w, hh, None);
    r.upload_overview(gpu, &mut img, &raster);
    let opts = RenderOptions {
        nearest_from: 1.0,
        ..Default::default()
    };
    for orient in [Orientation::NORMAL, Orientation(6)] {
        // The viewer at 2× on part of the image.
        let viewer = View::fixed(
            (w, hh),
            orient,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 70.0,
            },
            2.0,
            (37.0, 41.5),
        );
        let shot = capture::main_shot(&viewer, Region::Visible, Size::Pixels).unwrap();
        assert_eq!(shot.zoom, 1.0);
        assert_eq!(shot.size, (50, 35), "{orient:?}");
        let main = specular_figure::render::read(gpu, &mut r, &img, &shot, &opts).unwrap();
        let p2s = build::invert(&shot.view().screen_from_stored_f64());
        for y in 0..main.height {
            for x in 0..main.width {
                let s = build::apply(&p2s, (f64::from(x) + 0.5, f64::from(y) + 0.5));
                let want = src(s.0.floor() as u32, s.1.floor() as u32);
                let got = main.px(x, y).map(specular_figure::to8);
                assert_eq!(got, want, "{orient:?} panel {x},{y}");
            }
        }
        // An inset of a stored rectangle inside the panel.
        let s2p = shot.view().screen_from_stored_f64();
        let c = build::apply(&p2s, (25.0, 17.0));
        let rect = (c.0 - 6.0, c.1 - 5.0, c.0 + 6.0, c.1 + 5.0);
        let (region, ishot) = capture::inset_shot(&shot, &s2p, rect);
        let inset = specular_figure::render::read(gpu, &mut r, &img, &ishot, &opts).unwrap();
        assert_eq!((inset.width, inset.height), ishot.size);
        assert!(inset.height == main.height || inset.width == main.width);
        let (mut off, mut n) = (0, 0);
        for y in 0..inset.height {
            for x in 0..inset.width {
                let px = region[0] + (x as f32 + 0.5) * region[2] / inset.width as f32;
                let py = region[1] + (y as f32 + 0.5) * region[3] / inset.height as f32;
                n += 1;
                if inset.px(x, y) != main.px(px as u32, py as u32) {
                    off += 1;
                }
            }
        }
        // Pixels on the edges between source pixels may fall either way.
        assert!(off * 20 < n, "{orient:?}: {off} of {n} inset pixels differ");
    }
}

/// A teaching case folder: PNGs, anonymised DICOM, a "For Teaching" manifest that references
/// them, case.txt and case.json, nothing about the patient; the quiz reads it back. With
/// `SPECULAR_PYDICOM` (a Python with pydicom), pydicom reads the manifest too.
#[test]
fn teaching_case_folder_for_tce() {
    let dir = scratch("tce");
    let src = dir.join(format!("{NAME}.dcm"));
    std::fs::write(&src, dicom(true)).unwrap();
    let k = key(&src);
    let session = specular_dicom::anon::Session::new();
    let user = ((W, H), vec![TEXT]);
    let (areas, copy) = capture::teaching_one(&k, &user, &session, true).expect("kept");
    let mut kk = k.clone();
    kk.anon_title = "CT · Key image 1".into();
    let with = vec![(&kk, areas)];
    let texts = build::CaseTexts {
        readme: "Teaching case.".into(),
        notes: "Case notes".into(),
        findings: "Findings".into(),
        none: "None".into(),
    };
    let copies = vec![(0, copy.unwrap())];
    let files = build::tce(
        &with,
        &copies,
        true,
        "Classic case.",
        &texts,
        ("20261025", "101500"),
    )
    .unwrap();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "images/key-01.png",
            "DICOM/key-01.dcm",
            "DICOM/teaching-manifest.dcm",
            "case.json",
            "case.txt",
            "README.txt"
        ]
    );
    for (n, data) in &files {
        assert!(!contains(data, "DOE") && !contains(data, PID), "{n}");
    }
    // The manifest: Key Object Selection, "For Teaching", the notes, the copy referenced.
    let kos = &files[2].1;
    let d = specular_dicom::Dicom::parse(kos).unwrap();
    let v = d.values(kos);
    let text = |ds: &specular_dicom::DataSet, tag| {
        String::from_utf8_lossy(v.text_bytes(ds.get(tag).unwrap()))
            .trim_end_matches(['\0', ' '])
            .to_string()
    };
    assert_eq!(
        text(&d.dataset, 0x0008_0016),
        specular_dicom::kos::KEY_OBJECT_SELECTION
    );
    let title = d.dataset.item(0x0040_A043).unwrap();
    assert_eq!(text(title, 0x0008_0100), "113004");
    let copy = specular_dicom::kos::source(&files[1].1).unwrap();
    let content = &d.dataset.get(0x0040_A730).unwrap().items;
    assert_eq!(text(&content[0], 0x0040_A160), "Classic case.");
    let image = content[1].item(0x0008_1199).unwrap();
    assert_eq!(text(image, 0x0008_1155), copy.reference.sop_instance);
    assert_eq!(text(&d.dataset, 0x0020_000D), copy.reference.study);
    assert_eq!(text(&d.dataset, 0x0010_0010), copy.patient_name);
    // Written to disk, read back as quiz cases.
    let out = dir.join("case");
    std::fs::create_dir_all(&out).unwrap();
    for (n, data) in &files {
        let p = out.join(n);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }
    let case = build::read_case(&out).unwrap();
    assert_eq!(case.notes, "Classic case.");
    assert_eq!(case.images.len(), 1);
    assert_eq!(case.images[0].title, "CT · Key image 1");
    assert_eq!(case.images[0].findings, ["1. Line: 20.0 mm"]);
    // The quiz opens the anonymised instance, at the frame marked.
    assert_eq!(
        case.images[0].dicom.as_deref(),
        Some(out.join("DICOM/key-01.dcm").as_path())
    );
    assert_eq!(case.images[0].frame, k.frame);
    let (cards, _) = crate::app::quiz::cards_of_case(&out).unwrap();
    assert_eq!(cards[0].path, out.join("DICOM/key-01.dcm"));
    // pydicom, when available, reads the manifest as a valid KOS.
    if let Ok(py) = std::env::var("SPECULAR_PYDICOM") {
        let script = "import sys, pydicom\n\
            d = pydicom.dcmread(sys.argv[1])\n\
            assert d.SOPClassUID == '1.2.840.10008.5.1.4.1.1.88.59'\n\
            assert d.ConceptNameCodeSequence[0].CodeValue == '113004'\n\
            assert d.ContentSequence[0].TextValue == 'Classic case.'\n\
            ref = d.CurrentRequestedProcedureEvidenceSequence[0].ReferencedSeriesSequence[0].ReferencedSOPSequence[0]\n\
            assert ref.ReferencedSOPInstanceUID == d.ContentSequence[1].ReferencedSOPSequence[0].ReferencedSOPInstanceUID\n\
            print('ok')\n";
        let o = std::process::Command::new(py)
            .args(["-I", "-c", script])
            .arg(out.join("DICOM/teaching-manifest.dcm"))
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    } else {
        eprintln!("skipped the pydicom check: SPECULAR_PYDICOM is not set");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
