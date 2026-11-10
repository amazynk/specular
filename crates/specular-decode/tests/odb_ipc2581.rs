//! ODB++ jobs and IPC-2581 files through the registry: a job folder, the same job as a
//! `.tgz`, `.tar` and `.zip`, and an IPC-2581 file named `.xml` all open as boards with their
//! layer, top and bottom pages; other archives stay archives; refusals say why.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::Source;
use specular_decode::{DecodeError, Format, Registry, Session};

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-pcb/tests/data")
        .join(rel)
}

fn open(p: &Path) -> Result<Arc<dyn Session>, DecodeError> {
    let s = Source::open(p).map_err(DecodeError::Io)?;
    Registry::default().open(Arc::new(s))
}

/// The job folder's files, relative paths with a top folder `job/`.
fn job_files() -> Vec<(String, Vec<u8>)> {
    let root = data("odb/job");
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((format!("job/{rel}"), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// A ustar archive of `files`.
fn tar(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, b) in files {
        let mut h = [0u8; 512];
        h[..name.len()].copy_from_slice(name.as_bytes());
        h[100..107].copy_from_slice(b"0000644");
        h[124..135].copy_from_slice(format!("{:011o}", b.len()).as_bytes());
        h[136..147].copy_from_slice(b"00000000000");
        h[156] = b'0';
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        h[148..156].copy_from_slice(b"        ");
        let sum: u32 = h.iter().map(|&c| u32::from(c)).sum();
        h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(b);
        out.resize(out.len().div_ceil(512) * 512, 0);
    }
    out.extend_from_slice(&[0u8; 1024]);
    out
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-odb-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn check_board(s: &dyn Session, format: Format) {
    assert_eq!(s.info().format, format);
    assert_eq!(s.page_count(), 3);
    let d = s.drawing().expect("a drawing");
    let p = d.prepared.scene.pcb.as_ref().expect("board facts");
    assert_eq!(p.copper_layers, 2);
    assert_eq!(p.components, 1);
    let (w, h) = p.size_mm.unwrap();
    assert!(
        (w - 40.0).abs() < 1e-9 && (h - 25.0).abs() < 1e-9,
        "{w} {h}"
    );
}

#[test]
fn a_job_folder_opens_as_a_board() {
    let dir = data("odb/job");
    assert!(specular_core::is_odb_folder(&dir));
    assert!(specular_core::is_image_folder(&dir));
    let s = open(&dir).expect("opens");
    check_board(s.as_ref(), Format::Odb);
}

#[test]
fn job_archives_open_as_boards() {
    let files = job_files();
    let t = tar(&files);
    let dir = tmp_dir("archives");
    // .tgz
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&t).unwrap();
    let tgz = dir.join("board.tgz");
    std::fs::write(&tgz, gz.finish().unwrap()).unwrap();
    assert!(specular_decode::is_odb_archive(&tgz));
    check_board(open(&tgz).expect("tgz opens").as_ref(), Format::Odb);
    // .tar
    let tp = dir.join("board.tar");
    std::fs::write(&tp, &t).unwrap();
    assert!(specular_decode::is_odb_archive(&tp));
    check_board(open(&tp).expect("tar opens").as_ref(), Format::Odb);
    // An ordinary .tgz of pictures stays an archive.
    let other = tar(&[("pics/a.txt".into(), b"hello".to_vec())]);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&other).unwrap();
    let op = dir.join("pics.tgz");
    std::fs::write(&op, gz.finish().unwrap()).unwrap();
    assert!(!specular_decode::is_odb_archive(&op));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn landxml_opens_by_content() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-landxml/tests/data/site.xml");
    let s = open(&p).expect("opens");
    assert_eq!(s.info().format, Format::LandXml);
    assert_eq!(s.page_count(), 3, "plan, 3D model and a profile");
    let m = s
        .open_page(specular_decode::LANDXML_MODEL_PAGE)
        .expect("the 3D page");
    assert!(m.mesh().is_some() && m.drawing().is_none());
    let profile = s.open_page(2).expect("the profile");
    assert!(
        profile
            .drawing()
            .is_some_and(|d| d.sheet().is_some_and(|s| s.name != "Plan"))
    );
    let d = s.drawing().expect("a drawing");
    let survey = d.prepared.scene.survey.as_ref().expect("survey facts");
    assert_eq!(survey.surfaces.len(), 1);
    assert_eq!(specular_decode::station_label(1234.5, false), "1+234.50");
    let none = br#"<?xml version="1.0"?><LandXML version="1.2"><Units/></LandXML>"#;
    match Registry::default().open(Arc::new(Source::from_vec("empty.xml", none.to_vec()))) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("no TIN surface"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn ipc2581_opens_by_content() {
    let b = std::fs::read(data("ipc2581/board.xml")).unwrap();
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("design.xml", b)))
        .expect("opens");
    assert_eq!(s.info().format, Format::Ipc2581);
    assert_eq!(s.page_count(), 3);
    let only_bom = br#"<?xml version="1.0"?><IPC-2581 revision="C"><Bom name="x"/></IPC-2581>"#;
    match Registry::default().open(Arc::new(Source::from_vec("bom.xml", only_bom.to_vec()))) {
        Err(DecodeError::Unsupported(m)) => assert!(m.contains("bill of materials"), "{m}"),
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}
