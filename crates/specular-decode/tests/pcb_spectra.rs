//! Circuit boards and spectra through the registry: the format crates' fixtures open as the
//! right format, by content and by folder, with pages, facts and pixels; refusals say why.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::{DecodeError, DecodeRequest, Format, PageLabel, Registry, Session};

fn data(krate: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats")
        .join(krate)
        .join("tests/data")
        .join(name)
}

fn open(p: &Path) -> Result<Arc<dyn Session>, DecodeError> {
    let s = Source::open(p).map_err(DecodeError::Io)?;
    Registry::default().open(Arc::new(s))
}

fn bytes_as(p: &Path, name: &str) -> Result<Arc<dyn Session>, DecodeError> {
    let b = std::fs::read(p).expect("fixture");
    Registry::default().open(Arc::new(Source::from_vec(name, b)))
}

/// Pixels that are not the paper's white, at 1/4.
fn ink(s: &dyn Session) -> usize {
    let r = s
        .decode(&DecodeRequest::full(4), &CancelToken::new())
        .expect("decodes");
    r.planes[0]
        .data
        .chunks(4)
        .filter(|p| p[0] < 200 || p[1] < 200 || p[2] < 200)
        .count()
}

#[test]
fn a_folder_of_gerbers_opens_as_one_board() {
    let dir = data("specular-pcb", "board");
    assert!(specular_core::is_gerber_folder(&dir));
    let s = open(&dir).expect("opens");
    assert_eq!(s.info().format, Format::GerberSet);
    assert_eq!(s.page_count(), 3);
    use specular_decode::drawing::specular_vector::Generated;
    assert_eq!(
        s.page_label(),
        Some(PageLabel::Generated(Generated::BoardLayers))
    );
    let top = s.open_page(1).expect("top");
    assert_eq!(
        top.page_label(),
        Some(PageLabel::Generated(Generated::BoardTop))
    );
    let d = top.drawing().expect("a drawing");
    let sheet = &d.prepared.scene.sheets[1];
    assert!(
        sheet
            .layer_opacity
            .as_ref()
            .is_some_and(|o| o.iter().all(|&a| a == 1.0))
    );
    let pcb = d.prepared.scene.pcb.as_ref().expect("board facts");
    assert!(pcb.size_mm.is_some() && pcb.copper_layers >= 2, "{pcb:?}");
    assert!(ink(top.as_ref()) > 500);
}

/// A ZIP with these entries stored (no compression).
fn stored_zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    fn crc32(d: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in d {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let (mut out, mut cd) = (Vec::new(), Vec::new());
    for (name, body) in files {
        let off = out.len() as u32;
        let (crc, n, len) = (crc32(body), name.len() as u16, body.len() as u32);
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&len.to_le_bytes());
            v.extend_from_slice(&len.to_le_bytes());
            v.extend_from_slice(&n.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(b"PK\x03\x04");
        common(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        cd.extend_from_slice(b"PK\x01\x02\x14\x00");
        common(&mut cd);
        // Comment length, disk, internal and external attributes.
        cd.extend_from_slice(&[0u8; 6]);
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&off.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let (at, size, count) = (out.len() as u32, cd.len() as u32, files.len() as u16);
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[test]
fn a_zip_of_gerbers_opens_as_one_board_not_a_book() {
    let dir = data("specular-pcb", "board");
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| {
            let name = format!("gerbers/{}", e.file_name().to_string_lossy());
            (name, std::fs::read(e.path()).unwrap())
        })
        .collect();
    files.sort();
    let zip = stored_zip(&files);
    let tmp = std::env::temp_dir().join(format!("specular-board-{}.zip", std::process::id()));
    std::fs::write(&tmp, &zip).unwrap();
    assert!(specular_decode::is_board_archive(&tmp));
    let s = open(&tmp).expect("opens");
    assert_eq!(s.info().format, Format::GerberSet);
    assert_eq!(s.page_count(), 3);
    let _ = std::fs::remove_file(&tmp);
    // A ZIP of other things stays an archive.
    let other = stored_zip(&[("a.txt".into(), b"hello".to_vec())]);
    std::fs::write(&tmp, &other).unwrap();
    assert!(!specular_decode::is_board_archive(&tmp));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn single_gerber_and_drill_files_open_alone_by_content() {
    let head = std::fs::read(data("specular-pcb", "x2.gbr")).unwrap();
    assert_eq!(
        Registry::default().probe(&head).map(|d| d.format()),
        Some(Format::Gerber)
    );
    let s = bytes_as(&data("specular-pcb", "x2.gbr"), "layer.bin").expect("gerber");
    assert_eq!(s.info().format, Format::Gerber);
    assert_eq!(s.page_count(), 1);
    assert!(ink(s.as_ref()) > 50);
    let d = open(&data("specular-pcb", "kicad.drl")).expect("drill");
    assert_eq!(d.info().format, Format::Excellon);
    match open(&data("specular-pcb", "rs274d.gbr")) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("RS-274-D"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn kicad_boards_and_schematics_with_sheets_open_and_old_ones_are_refused() {
    let b = open(&data("specular-kicad", "board8.kicad_pcb")).expect("board");
    assert_eq!(b.info().format, Format::KicadPcb);
    assert!(ink(b.as_ref()) > 100);
    let s = open(&data("specular-kicad", "sch8.kicad_sch")).expect("schematic");
    assert_eq!(s.info().format, Format::KicadSch);
    assert!(s.page_count() >= 2, "sub-sheets are pages");
    assert!(matches!(
        s.page_label(),
        Some(PageLabel::Sheet { model: false, .. })
    ));
    match open(&data("specular-kicad", "kicad5.kicad_pcb")) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("KiCad 5"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn spectra_open_as_charts_with_facts() {
    let s = bytes_as(&data("specular-jcamp", "ir_dif.jdx"), "x.txt").expect("jcamp");
    assert_eq!(s.info().format, Format::Jcamp);
    let f = s.spectrum().expect("facts");
    assert_eq!(f.technique, "Infrared");
    assert_eq!(f.x_axis.1, "cm⁻¹");
    let d = s.drawing().expect("a drawing");
    let chart = d.prepared.scene.sheets[0].chart.as_ref().expect("a chart");
    assert!(chart.x.start > chart.x.end, "wavenumbers run right to left");
    assert!(ink(s.as_ref()) > 100);

    let m = open(&data("specular-jcamp", "multi_even.spc")).expect("spc");
    assert_eq!(m.info().format, Format::Spc);
    assert!(m.page_count() > 1);
    let p2 = m.open_page(1).expect("page 2");
    assert_eq!(p2.page_index(), 1);
    assert_eq!(p2.spectrum().unwrap().page, 2);
    match open(&data("specular-jcamp", "shimadzu.spc")) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("Shimadzu"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn mass_spectrometry_runs_page_by_spectrum_and_imaging_files_stay_imzml() {
    let s = open(&data("specular-mzml", "indexed.mzML")).expect("mzml");
    assert_eq!(s.info().format, Format::Mzml);
    assert!(s.page_count() > 2);
    let last = s.open_page(s.page_count() - 1).expect("a spectrum");
    assert!(matches!(last.page_label(), Some(PageLabel::Map { .. })));
    assert!(ink(last.as_ref()) > 20);
    let x = open(&data("specular-mzml", "scans64z.mzXML")).expect("mzxml");
    assert_eq!(x.info().format, Format::Mzxml);
    let i = open(&data("specular-mzml", "imaging.mzML"));
    assert!(!i.is_ok_and(|s| s.info().format == Format::Mzml));
}

#[test]
fn bruker_experiments_open_as_folders_or_by_their_files() {
    let dir = data("specular-nmr", "fid1d");
    let s = open(&dir).expect("experiment");
    assert_eq!(s.info().format, Format::BrukerNmr);
    let f = s.spectrum().expect("facts");
    assert_eq!(f.nucleus.as_deref(), Some("1H"));
    let fid = open(&dir.join("fid")).expect("by its fid");
    assert_eq!(fid.info().format, Format::BrukerNmr);
    // The page made from the FID says it was phased automatically.
    let made = (0..s.page_count())
        .map(|p| s.open_page(p).expect("page"))
        .find(|p| {
            p.spectrum()
                .is_some_and(|f| f.notes.iter().any(|n| n.contains("automatically")))
        });
    assert!(made.is_some());
    let hsqc = open(&data("specular-nmr", "hsqc2d")).expect("2D");
    let d = hsqc.drawing().expect("a drawing");
    assert!(
        d.prepared
            .scene
            .layers
            .iter()
            .any(|l| l.name.contains("contours"))
    );
}

/// A whole-file gzip of an mzML or mzXML run opens as the run itself, by name or by content.
#[test]
fn gzipped_mass_spectrometry_runs_open_as_the_run() {
    use std::io::Write;
    for (name, format) in [
        ("indexed.mzML", Format::Mzml),
        ("scans64z.mzXML", Format::Mzxml),
    ] {
        let path = data("specular-mzml", name);
        let plain = open(&path).expect("plain");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&std::fs::read(&path).unwrap()).unwrap();
        let gz = gz.finish().unwrap();
        for as_name in [format!("{name}.gz"), "run.bin".to_string()] {
            let s = Registry::default()
                .open(Arc::new(Source::from_vec(&as_name, gz.clone())))
                .unwrap_or_else(|e| panic!("{as_name}: {e}"));
            assert_eq!(s.info().format, format, "{as_name}");
            assert_eq!(s.page_count(), plain.page_count(), "{as_name}");
            let (a, b) = (s.spectrum().unwrap(), plain.spectrum().unwrap());
            assert_eq!(a.spectra, b.spectra);
            assert_eq!(a.points, b.points);
        }
    }
}
