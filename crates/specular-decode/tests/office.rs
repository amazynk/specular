//! Office documents through the registry: shown by their stored preview, with their
//! properties; without one, a session that says so; password-protected packages refused by
//! name. Fixtures from `specular_office::fixture`; the corpus (tools/corpus/gen-office.py)
//! when SPECULAR_OFFICE_CORPUS or tools/corpus/office has it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, Source};
use specular_decode::office_dec::NO_PREVIEW;
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};
use specular_office::fixture::{self, Prop};

fn open(name: &str, bytes: Vec<u8>) -> Result<Arc<dyn Session>, DecodeError> {
    Registry::default().open(Arc::new(Source::from_vec(name, bytes)))
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h)
        .flat_map(|i| [(i * 7) as u8, 90, 200, 255])
        .collect();
    specular_png::encode(w, h, specular_png::ColorType::Rgba, &px).unwrap()
}

/// The decoded pixels as RGBA (an 8-bit RGBA or RGB raster).
fn rgba(s: &dyn Session) -> (u32, u32, Vec<u8>) {
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .expect("decode");
    let p = &r.planes[0];
    let bpp = p.bytes_per_pixel();
    let mut out = Vec::new();
    for y in 0..p.height as usize {
        let row = &p.data[y * p.stride..];
        for x in 0..p.width as usize {
            let px = &row[x * bpp..x * bpp + bpp];
            out.extend_from_slice(&[px[0], px[1], px[2], if bpp == 4 { px[3] } else { 255 }]);
        }
    }
    (r.width, r.height, out)
}

fn docx(thumb: Option<&[u8]>, main: &str) -> Vec<u8> {
    let types = format!(
        r#"<Types><Default Extension="png" ContentType="image/png"/><Override PartName="/m" ContentType="{main}"/></Types>"#
    );
    let rels = r#"<Relationships><Relationship Id="r1" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail" Target="docProps/thumbnail.png"/></Relationships>"#;
    let core = r#"<cp:coreProperties><dc:title>Plan</dc:title><dc:creator>Ann</dc:creator></cp:coreProperties>"#;
    let app = r#"<Properties><Pages>4</Pages><Slides>9</Slides></Properties>"#;
    let main_part = if main.contains("word") {
        "word/document.xml"
    } else if main.contains("sheet") {
        "xl/workbook.xml"
    } else {
        "ppt/presentation.xml"
    };
    let mut files: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        (main_part, b"<x/>"),
        ("docProps/core.xml", core.as_bytes()),
        ("docProps/app.xml", app.as_bytes()),
    ];
    if let Some(t) = thumb {
        files.push(("docProps/thumbnail.png", t));
    }
    fixture::zip(&files)
}

const WORD: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const EXCEL: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";

#[test]
fn word_package_shows_its_thumbnail_with_properties() {
    let thumb = png(24, 32);
    for name in ["plan.docx", "plan.bin"] {
        let s = open(name, docx(Some(&thumb), WORD)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let info = s.info();
        assert_eq!(info.format, Format::Ooxml, "{name}");
        assert_eq!((info.width, info.height), (24, 32));
        assert!(
            info.coding
                .starts_with("Word document, embedded preview only"),
            "{}",
            info.coding
        );
        let d = s.document().expect("facts");
        assert_eq!((d.kind, d.preview), ("word", Some("PNG")));
        assert_eq!(d.props.title.as_deref(), Some("Plan"));
        assert_eq!(d.props.pages, Some(4));
        let (w, h, px) = rgba(s.as_ref());
        assert_eq!((w, h), (24, 32));
        assert_eq!(&px[..4], &[0, 90, 200, 255]);
    }
}

#[test]
fn a_document_without_a_preview_says_so_and_keeps_its_properties() {
    let s = open("budget.xlsx", docx(None, EXCEL)).unwrap();
    assert_eq!(s.info().format, Format::Ooxml);
    let d = s.document().unwrap();
    assert_eq!((d.kind, d.preview), ("excel", None));
    assert_eq!(d.props.author.as_deref(), Some("Ann"));
    match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
        Err(DecodeError::Unsupported(m)) => assert_eq!(m, NO_PREVIEW),
        r => panic!("{:?}", r.map(|_| ())),
    }
}

#[test]
fn word_97_shows_its_dib_thumbnail() {
    let dib = fixture::dib_2x2();
    let s_info = fixture::property_set(
        fixture::FMTID_SUMMARY,
        &[(2, Prop::Str("Old")), (17, Prop::Clip(-1, 8, &dib))],
    );
    let f = fixture::cfb(&[
        ("WordDocument", &[0u8; 5000][..]),
        ("\u{5}SummaryInformation", &s_info),
    ]);
    let s = open("old.doc", f).unwrap();
    assert_eq!(s.info().format, Format::MsOffice);
    assert!(
        s.info()
            .coding
            .starts_with("Word document (97–2003), embedded preview only"),
        "{}",
        s.info().coding
    );
    assert!(s.document().unwrap().legacy);
    let (w, h, px) = rgba(s.as_ref());
    assert_eq!((w, h), (2, 2));
    // Top row red, green; bottom row blue, white.
    assert_eq!(&px[..8], &[255, 0, 0, 255, 0, 255, 0, 255]);
    assert_eq!(&px[8..], &[0, 0, 255, 255, 255, 255, 255, 255]);
}

#[test]
fn password_protected_packages_are_refused_by_name() {
    let f = fixture::cfb(&[
        ("EncryptionInfo", &[4, 0, 4, 0, 0x40, 0, 0, 0][..]),
        ("EncryptedPackage", &[0u8; 64][..]),
    ]);
    match open("secret.pptx", f) {
        Err(DecodeError::Unsupported(m)) => {
            assert!(
                m.starts_with("password-protected PowerPoint presentation"),
                "{m}"
            )
        }
        r => panic!("{:?}", r.map(|s| s.info().format)),
    }
}

#[test]
fn iwork_and_opendocument_previews_and_their_absence() {
    let z = fixture::zip(&[("Index/Document.iwa", b"x"), ("preview.jpg", &png(10, 8))]);
    let s = open("letter.pages", z).unwrap();
    assert_eq!(s.info().format, Format::Iwork);
    assert_eq!(s.document().unwrap().kind, "pages");
    assert_eq!((s.info().width, s.info().height), (10, 8));
    let meta = br#"<office:document-meta><office:meta><dc:title>Notes</dc:title><meta:document-statistic meta:page-count="3"/></office:meta></office:document-meta>"#;
    let odt = |thumb: Option<&[u8]>| {
        let mut f: Vec<(&str, &[u8])> = vec![
            ("mimetype", b"application/vnd.oasis.opendocument.text"),
            ("content.xml", b"<office:document-content/>"),
            ("meta.xml", meta),
        ];
        if let Some(t) = thumb {
            f.push(("Thumbnails/thumbnail.png", t));
        }
        fixture::zip(&f)
    };
    let s = open("notes.odt", odt(Some(&png(5, 7)))).unwrap();
    assert_eq!(s.info().format, Format::OpenDocument);
    let d = s.document().unwrap();
    assert_eq!(
        (d.kind, d.preview, d.props.pages),
        ("odf-text", Some("PNG"), Some(3))
    );
    let s = open("notes.odt", odt(None)).unwrap();
    let d = s.document().unwrap();
    assert_eq!((d.preview, d.props.title.as_deref()), (None, Some("Notes")));
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_OFFICE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus/office"));
    dir.join("oracle.json").is_file().then_some(dir)
}

/// Every corpus file opens as a document or is refused in words; previews decode.
#[test]
fn office_corpus_opens_through_the_registry() {
    let Some(dir) = corpus() else {
        eprintln!("no office corpus; run tools/corpus/gen-office.py");
        return;
    };
    let (mut shown, mut none, mut refused) = (0, 0, 0);
    let mut bad = Vec::new();
    for sub in ["real", "own", "own-legacy"] {
        let Ok(rd) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            let t0 = std::time::Instant::now();
            let s = Registry::default().open(Arc::new(Source::open(&p).unwrap()));
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            match s {
                Ok(s) if s.document().is_some_and(|d| d.preview.is_none()) => none += 1,
                Ok(s) if s.document().is_some() => {
                    match s.decode(&DecodeRequest::full(1), &CancelToken::new()) {
                        Ok(_) => shown += 1,
                        Err(e) => bad.push(format!("{name}: preview: {e}")),
                    }
                }
                Ok(s) => bad.push(format!("{name}: opened as {:?}", s.info().format)),
                Err(DecodeError::Unsupported(m)) if m.starts_with("password-protected") => {
                    refused += 1
                }
                Err(e) => bad.push(format!("{name}: {e}")),
            }
            eprintln!("{name}: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
        }
    }
    eprintln!(
        "office corpus: {shown} previews shown, {none} without a preview, {refused} refused, {} failed",
        bad.len()
    );
    assert!(bad.is_empty(), "{bad:#?}");
}
