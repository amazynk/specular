use crate::fixture::{self, Prop};
use crate::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-not-a-real-png";
const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0-not-a-real-jpeg";

fn types(main: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/x" ContentType="{main}"/></Types>"#
    )
}

const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail" Target="/docProps/thumbnail.png"/></Relationships>"#;

const CORE: &str = r#"<cp:coreProperties xmlns:cp="c" xmlns:dc="d" xmlns:dcterms="t"><dc:title>Quarterly &amp; plan</dc:title><dc:creator>Ann</dc:creator><cp:lastModifiedBy>Bob</cp:lastModifiedBy><cp:revision>7</cp:revision><dcterms:created xsi:type="dcterms:W3CDTF">2024-01-02T03:04:05Z</dcterms:created><dcterms:modified>2024-02-03T04:05:06Z</dcterms:modified></cp:coreProperties>"#;

const APP: &str = r#"<Properties><Application>Microsoft Office Word</Application><AppVersion>16.0000</AppVersion><Pages>3</Pages><Words>412</Words><Company>ACME</Company><TotalTime>25</TotalTime></Properties>"#;

#[test]
fn word_with_a_thumbnail_and_properties() {
    let t =
        types("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml");
    let z = fixture::zip(&[
        ("[Content_Types].xml", t.as_bytes()),
        ("_rels/.rels", RELS.as_bytes()),
        ("word/document.xml", b"<w:document/>"),
        ("docProps/core.xml", CORE.as_bytes()),
        ("docProps/app.xml", APP.as_bytes()),
        ("docProps/thumbnail.png", PNG),
    ]);
    assert_eq!(probe(&z), Some((Family::Ooxml, App::Word)));
    let d = open(&z, None).unwrap();
    assert_eq!((d.app, d.template, d.macros), (App::Word, false, false));
    let p = d.preview.unwrap();
    assert_eq!(p.kind, PreviewKind::Png);
    assert_eq!(p.bytes.get(&z), PNG);
    assert_eq!(p.part, "docProps/thumbnail.png");
    let props = d.props;
    assert_eq!(props.title.as_deref(), Some("Quarterly & plan"));
    assert_eq!(props.author.as_deref(), Some("Ann"));
    assert_eq!(props.last_modified_by.as_deref(), Some("Bob"));
    assert_eq!(props.created.as_deref(), Some("2024-01-02T03:04:05Z"));
    assert_eq!(props.pages, Some(3));
    assert_eq!(props.words, Some(412));
    assert_eq!(props.edit_minutes, Some(25));
    assert_eq!(
        props.application.as_deref(),
        Some("Microsoft Office Word 16.0000")
    );
}

#[test]
fn templates_macros_and_no_preview() {
    let t = types("application/vnd.ms-excel.template.macroEnabled.main+xml");
    let wb = br#"<workbook><sheets><sheet name="A" sheetId="1"/><sheet name="B" sheetId="2"/></sheets></workbook>"#;
    let z = fixture::zip(&[
        ("[Content_Types].xml", t.as_bytes()),
        ("xl/workbook.xml", wb),
        ("xl/vbaProject.bin", b"x"),
    ]);
    let d = open(&z, None).unwrap();
    assert_eq!((d.app, d.template, d.macros), (App::Excel, true, true));
    assert_eq!(d.props.sheets, Some(2));
    assert_eq!(d.preview, None);
}

#[test]
fn powerpoint_thumbnail_by_name_and_slides_counted() {
    let t =
        types("application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml");
    let z = fixture::zip(&[
        ("[Content_Types].xml", t.as_bytes()),
        ("ppt/presentation.xml", b"<p/>"),
        ("ppt/slides/slide1.xml", b"<p/>"),
        ("ppt/slides/slide2.xml", b"<p/>"),
        ("ppt/slides/_rels/slide1.xml.rels", b"<r/>"),
        ("docProps/thumbnail.jpeg", JPEG),
    ]);
    let d = open(&z, None).unwrap();
    assert_eq!(d.app, App::PowerPoint);
    assert_eq!(d.props.slides, Some(2));
    assert_eq!(d.preview.unwrap().kind, PreviewKind::Jpeg);
}

#[test]
fn other_packages_are_not_office() {
    // VSDX, XPS and 3MF are OPC packages too.
    for main in [
        "visio/document.xml",
        "Documents/1/FixedDocument.fdoc",
        "3D/3dmodel.model",
    ] {
        let z = fixture::zip(&[
            ("[Content_Types].xml", types("application/xml").as_bytes()),
            (main, b"<x/>"),
        ]);
        assert_eq!(probe(&z), None, "{main}");
        assert_eq!(open(&z, None), Err(Error::NotOffice));
    }
    assert_eq!(probe(b"plain text"), None);
}

fn legacy(main: &str, summary: &[(u32, Prop)], doc: &[(u32, Prop)]) -> Vec<u8> {
    let s = fixture::property_set(fixture::FMTID_SUMMARY, summary);
    let d = fixture::property_set(fixture::FMTID_DOC_SUMMARY, doc);
    let body = vec![0u8; 5000];
    fixture::cfb(&[
        (main, &body[..]),
        ("\u{5}SummaryInformation", &s),
        ("\u{5}DocumentSummaryInformation", &d),
    ])
}

#[test]
fn word_97_with_a_dib_thumbnail() {
    let ft = (1_000_000_000u64 + 11_644_473_600) * 10_000_000;
    let dib = fixture::dib_2x2();
    let f = legacy(
        "WordDocument",
        &[
            (2, Prop::Str("Caf\u{e9} notes")),
            (4, Prop::Str("Ann")),
            (12, Prop::Time(ft)),
            (14, Prop::I4(5)),
            (17, Prop::Clip(-1, 8, &dib)),
            (18, Prop::Str("Microsoft Word 8.0")),
        ],
        &[(15, Prop::Str("ACME"))],
    );
    assert_eq!(probe(&f), Some((Family::Legacy, App::Word)));
    let d = open(&f, None).unwrap();
    assert_eq!(d.props.title.as_deref(), Some("Caf\u{e9} notes"));
    assert_eq!(d.props.author.as_deref(), Some("Ann"));
    assert_eq!(d.props.created.as_deref(), Some("2001-09-09T01:46:40Z"));
    assert_eq!(d.props.pages, Some(5));
    assert_eq!(d.props.company.as_deref(), Some("ACME"));
    let p = d.preview.unwrap();
    assert_eq!(p.kind, PreviewKind::Bmp);
    let bmp = p.bytes.get(&f);
    assert!(bmp.starts_with(b"BM"));
    assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
    assert_eq!(&bmp[14..], &dib[..]);
}

#[test]
fn powerpoint_97_with_a_metafile_thumbnail_and_slides() {
    // A METAFILEPICT header (mapping mode, extents, handle), then a WMF header.
    let mut mfp = vec![8, 0, 0x10, 0x27, 0x20, 0x1C, 0, 0];
    mfp.extend_from_slice(&[1, 0, 9, 0, 0, 3, 0x20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    mfp.extend_from_slice(&[3, 0, 0, 0, 0, 0]);
    let f = legacy(
        "PowerPoint Document",
        &[(17, Prop::Clip(-1, 3, &mfp))],
        &[(7, Prop::I4(12)), (8, Prop::I4(2))],
    );
    let d = open(&f, None).unwrap();
    assert_eq!(d.app, App::PowerPoint);
    assert_eq!(d.props.slides, Some(12));
    assert_eq!(d.props.notes, Some(2));
    let p = d.preview.unwrap();
    assert_eq!(p.kind, PreviewKind::Wmf);
    assert_eq!(p.bytes.get(&f), &mfp[8..]);
}

#[test]
fn excel_97_without_a_preview() {
    let f = legacy("Workbook", &[(2, Prop::Str("Budget"))], &[]);
    let d = open(&f, None).unwrap();
    assert_eq!(d.app, App::Excel);
    assert_eq!(d.preview, None);
    assert_eq!(d.props.title.as_deref(), Some("Budget"));
}

#[test]
fn password_protected_packages_are_refused_by_name() {
    let f = fixture::cfb(&[
        ("EncryptionInfo", &[4, 0, 4, 0, 0x40, 0, 0, 0][..]),
        ("EncryptedPackage", &[0u8; 64][..]),
    ]);
    assert_eq!(probe(&f).map(|p| p.0), Some(Family::Ooxml));
    let e = open(&f, Some(App::Excel)).unwrap_err();
    assert_eq!(e, Error::Encrypted("Excel workbook"));
    assert!(
        e.to_string()
            .starts_with("password-protected Excel workbook")
    );
    assert_eq!(
        open(&f, None).unwrap_err(),
        Error::Encrypted("Office document")
    );
}

#[test]
fn iwork_previews() {
    let z = fixture::zip(&[
        ("Index/Document.iwa", b"x"),
        ("Index/Slide-1.iwa", b"x"),
        ("preview-micro.jpg", JPEG),
        ("preview.jpg", JPEG),
    ]);
    assert_eq!(probe(&z), Some((Family::Iwork, App::Keynote)));
    let d = open(&z, Some(App::Pages)).unwrap();
    assert_eq!(d.app, App::Keynote);
    assert_eq!(d.preview.unwrap().part, "preview.jpg");
    // iWork '09 with its QuickLook folder, zipped with the bundle folder in front.
    let z = fixture::zip(&[
        ("Report.pages/index.xml", b"<sl:document/>"),
        ("Report.pages/QuickLook/Thumbnail.jpg", JPEG),
    ]);
    let d = open(&z, Some(App::Pages)).unwrap();
    assert_eq!((d.family, d.app), (Family::Iwork, App::Pages));
    assert_eq!(d.preview.unwrap().part, "QuickLook/Thumbnail.jpg");
    let z = fixture::zip(&[
        ("Index/Document.iwa", b"x"),
        ("Index/CalculationEngine.iwa", b"x"),
    ]);
    let d = open(&z, None).unwrap();
    assert_eq!((d.app, d.preview), (App::Numbers, None));
    // A plain index.xml without QuickLook is not iWork.
    assert_eq!(probe(&fixture::zip(&[("index.xml", b"<x/>")])), None);
}

#[test]
fn compound_fixture_reads_back() {
    let big = vec![7u8; 9000];
    let f = fixture::cfb(&[("Small", b"abc"), ("Big", &big)]);
    let c = specular_cfb::Cfb::open(&f).unwrap();
    assert_eq!(&c.read("Small").unwrap()[..], b"abc");
    assert_eq!(&c.read("Big").unwrap()[..], &big[..]);
}

#[test]
fn hostile_files_do_not_panic() {
    let t =
        types("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml");
    let z = fixture::zip(&[
        ("[Content_Types].xml", t.as_bytes()),
        ("_rels/.rels", RELS.as_bytes()),
        ("word/document.xml", b"<w:document/>"),
        ("docProps/thumbnail.png", PNG),
    ]);
    let dib = fixture::dib_2x2();
    let l = legacy("WordDocument", &[(17, Prop::Clip(-1, 8, &dib))], &[]);
    for f in [&z, &l] {
        for cut in (0..f.len()).step_by(7) {
            let _ = probe(&f[..cut]);
            let _ = open(&f[..cut], None);
        }
        let mut flip = f.clone();
        for i in (0..flip.len()).step_by(13) {
            flip[i] ^= 0x5A;
            let _ = open(&flip, None);
        }
    }
}
