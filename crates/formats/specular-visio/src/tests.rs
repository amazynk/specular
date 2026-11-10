use super::*;
use specular_drawpage::{Op, RenderRequest, render};

fn crc32(d: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
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

/// A stored (uncompressed) ZIP.
fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (name, data) in members {
        let off = out.len() as u32;
        let crc = crc32(data);
        let mut h = Vec::new();
        h.extend_from_slice(&20u16.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(&0u32.to_le_bytes());
        h.extend_from_slice(&crc.to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(data.len() as u32).to_le_bytes());
        h.extend_from_slice(&(name.len() as u16).to_le_bytes());
        h.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&h);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&h);
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&off.to_le_bytes());
        cd.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(members.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

const NS: &str = "xmlns='http://schemas.microsoft.com/office/visio/2012/main' \
                  xmlns:r='http://schemas.openxmlformats.org/officeDocument/2006/relationships'";

fn rels(items: &[(&str, &str, &str)]) -> String {
    let mut s = String::from(
        "<?xml version='1.0'?><Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>",
    );
    for (id, ty, target) in items {
        s.push_str(&format!(
            "<Relationship Id='{id}' Type='http://schemas.microsoft.com/visio/2010/relationships/{ty}' Target='{target}'/>"
        ));
    }
    s.push_str("</Relationships>");
    s
}

fn rect_geometry(w: f64, h: f64) -> String {
    format!(
        "<Section N='Geometry' IX='0'><Cell N='NoFill' V='0'/>\
         <Row T='MoveTo' IX='1'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row>\
         <Row T='LineTo' IX='2'><Cell N='X' V='{w}'/><Cell N='Y' V='0'/></Row>\
         <Row T='LineTo' IX='3'><Cell N='X' V='{w}'/><Cell N='Y' V='{h}'/></Row>\
         <Row T='LineTo' IX='4'><Cell N='X' V='0'/><Cell N='Y' V='{h}'/></Row>\
         <Row T='LineTo' IX='5'><Cell N='X' V='0'/><Cell N='Y' V='0'/></Row></Section>"
    )
}

/// A two-page drawing: page 1 has a master instance (red box, 2×1 in at (2, 9)), a group
/// with a blue ellipse and a text shape; page 2 is empty and uses a background page with a
/// green box.
fn sample() -> Vec<u8> {
    sample_with(None)
}

/// [`sample`] with other contents for master 2.
fn sample_with(master_contents: Option<String>) -> Vec<u8> {
    let doc = format!(
        "<?xml version='1.0'?><VisioDocument {NS}><FaceNames><FaceName ID='1' NameU='Arial'/></FaceNames>\
         <StyleSheets><StyleSheet ID='0' NameU='No Style'><Cell N='LineWeight' V='0.01'/>\
         <Cell N='LineColor' V='0'/><Cell N='LinePattern' V='1'/><Cell N='FillForegnd' V='1'/>\
         <Cell N='FillPattern' V='1'/><Cell N='VerticalAlign' V='1'/>\
         <Section N='Character'><Row IX='0'><Cell N='Font' V='Arial'/><Cell N='Size' V='0.1666'/>\
         <Cell N='Color' V='0'/><Cell N='Style' V='0'/></Row></Section></StyleSheet></StyleSheets></VisioDocument>"
    );
    let masters = format!(
        "<?xml version='1.0'?><Masters {NS}><Master ID='2' NameU='Box'><PageSheet><Cell N='PageWidth' V='2'/><Cell N='PageHeight' V='1'/></PageSheet><Rel r:id='rId1'/></Master></Masters>"
    );
    let master1 = format!(
        "<?xml version='1.0'?><MasterContents {NS}><Shapes><Shape ID='5' Type='Shape' LineStyle='0' FillStyle='0' TextStyle='0'>\
         <Cell N='PinX' V='1'/><Cell N='PinY' V='0.5'/><Cell N='Width' V='2'/><Cell N='Height' V='1'/>\
         <Cell N='FillForegnd' V='#ff0000'/>{}</Shape></Shapes></MasterContents>",
        rect_geometry(2.0, 1.0)
    );
    let master1 = master_contents.unwrap_or(master1);
    let pages = format!(
        "<?xml version='1.0'?><Pages {NS}>\
         <Page ID='0' Name='First'><PageSheet><Cell N='PageWidth' V='8'/><Cell N='PageHeight' V='10'/></PageSheet><Rel r:id='rId1'/></Page>\
         <Page ID='1' Name='Second' BackPage='2'><PageSheet><Cell N='PageWidth' V='8'/><Cell N='PageHeight' V='10'/></PageSheet><Rel r:id='rId2'/></Page>\
         <Page ID='2' Name='Back' Background='1'><PageSheet><Cell N='PageWidth' V='8'/><Cell N='PageHeight' V='10'/></PageSheet><Rel r:id='rId3'/></Page></Pages>"
    );
    let page1 = format!(
        "<?xml version='1.0'?><PageContents {NS}><Shapes>\
         <Shape ID='1' Master='2'><Cell N='PinX' V='3'/><Cell N='PinY' V='9'/></Shape>\
         <Shape ID='2' Type='Group' LineStyle='0' FillStyle='0' TextStyle='0'><Cell N='PinX' V='4'/><Cell N='PinY' V='5'/>\
         <Cell N='Width' V='2'/><Cell N='Height' V='2'/><Shapes>\
         <Shape ID='3' LineStyle='0' FillStyle='0' TextStyle='0'><Cell N='PinX' V='1'/><Cell N='PinY' V='1'/><Cell N='Width' V='2'/><Cell N='Height' V='2'/>\
         <Cell N='FillForegnd' V='4'/><Section N='Geometry' IX='0'><Row T='Ellipse' IX='1'><Cell N='X' V='1'/><Cell N='Y' V='1'/>\
         <Cell N='A' V='2'/><Cell N='B' V='1'/><Cell N='C' V='1'/><Cell N='D' V='2'/></Row></Section></Shape></Shapes></Shape>\
         <Shape ID='4' LineStyle='0' FillStyle='0' TextStyle='0'><Cell N='PinX' V='4'/><Cell N='PinY' V='2'/><Cell N='Width' V='3'/><Cell N='Height' V='1'/>\
         <Section N='Character'><Row IX='0'><Cell N='Size' V='0.3333'/><Cell N='Color' V='#0000ff'/><Cell N='Style' V='1'/></Row></Section>\
         <Text><cp IX='0'/>Hello Visio</Text></Shape>\
         </Shapes></PageContents>"
    );
    let page2 = format!("<?xml version='1.0'?><PageContents {NS}><Shapes/></PageContents>");
    let page3 = format!(
        "<?xml version='1.0'?><PageContents {NS}><Shapes><Shape ID='1' LineStyle='0' FillStyle='0'>\
         <Cell N='PinX' V='4'/><Cell N='PinY' V='5'/><Cell N='Width' V='8'/><Cell N='Height' V='10'/>\
         <Cell N='FillForegnd' V='#00ff00'/>{}</Shape></Shapes></PageContents>",
        rect_geometry(8.0, 10.0)
    );
    let ct = "<?xml version='1.0'?><Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'>\
              <Override PartName='/visio/document.xml' ContentType='application/vnd.ms-visio.drawing.main+xml'/></Types>";
    let root_rels = "<?xml version='1.0'?><Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>\
         <Relationship Id='rId1' Type='http://schemas.microsoft.com/visio/2010/relationships/document' Target='visio/document.xml'/>\
         <Relationship Id='rId2' Type='http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties' Target='docProps/core.xml'/></Relationships>";
    let core = "<?xml version='1.0'?><cp:coreProperties xmlns:cp='http://schemas.openxmlformats.org/package/2006/metadata/core-properties' xmlns:dc='http://purl.org/dc/elements/1.1/'><dc:title>Test drawing</dc:title><dc:creator>Someone</dc:creator></cp:coreProperties>";
    let doc_rels = rels(&[
        ("rId1", "pages", "pages/pages.xml"),
        ("rId2", "masters", "masters/masters.xml"),
    ]);
    let pages_rels = rels(&[
        ("rId1", "page", "page1.xml"),
        ("rId2", "page", "page2.xml"),
        ("rId3", "page", "page3.xml"),
    ]);
    let masters_rels = rels(&[("rId1", "master", "master1.xml")]);
    zip(&[
        ("[Content_Types].xml", ct.as_bytes()),
        ("_rels/.rels", root_rels.as_bytes()),
        ("docProps/core.xml", core.as_bytes()),
        ("visio/document.xml", doc.as_bytes()),
        ("visio/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("visio/pages/pages.xml", pages.as_bytes()),
        ("visio/pages/_rels/pages.xml.rels", pages_rels.as_bytes()),
        ("visio/pages/page1.xml", page1.as_bytes()),
        ("visio/pages/page2.xml", page2.as_bytes()),
        ("visio/pages/page3.xml", page3.as_bytes()),
        ("visio/masters/masters.xml", masters.as_bytes()),
        (
            "visio/masters/_rels/masters.xml.rels",
            masters_rels.as_bytes(),
        ),
        ("visio/masters/master1.xml", master1.as_bytes()),
    ])
}

/// A master whose three shapes are instances of the master itself: each visit fans out
/// three ways to the depth cap (3^48 visits), which the visit cap stops.
#[test]
fn self_instancing_master_is_bounded() {
    let shape = |id: u32| {
        format!("<Shape ID='{id}' Master='2'><Cell N='PinX' V='1'/><Cell N='PinY' V='1'/></Shape>")
    };
    let master = format!(
        "<?xml version='1.0'?><MasterContents {NS}><Shapes>{}{}{}</Shapes></MasterContents>",
        shape(5),
        shape(6),
        shape(7)
    );
    let d = sample_with(Some(master));
    let t = std::time::Instant::now();
    let doc = open_vsdx(&d, None).unwrap();
    let _ = draw(&doc.pages[0], 18.0);
    assert!(t.elapsed().as_secs() < 30);
}

fn draw(p: &Page, dpi: f64) -> (u32, u32, Vec<u8>) {
    let s = dpi / 72.0;
    let (w, h) = ((p.size.0 * s).round() as u32, (p.size.1 * s).round() as u32);
    let req = RenderRequest {
        scale_x: s,
        scale_y: s,
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    (w, h, render(p, req, &|| false).unwrap())
}

fn px(b: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * w + x) * 4) as usize;
    [b[i], b[i + 1], b[i + 2]]
}

#[test]
fn pages_masters_groups_text() {
    let d = sample();
    assert!(probe_vsdx(&d));
    let doc = open_vsdx(&d, None).unwrap();
    assert_eq!(doc.pages.len(), 2);
    assert_eq!(doc.facts.page_names, ["First", "Second"]);
    assert_eq!(doc.facts.background_pages, 1);
    assert_eq!(doc.facts.title.as_deref(), Some("Test drawing"));
    assert_eq!(doc.facts.author.as_deref(), Some("Someone"));
    assert_eq!(doc.describe(0), "Visio drawing (VSDX), page \"First\"");
    let p = &doc.pages[0];
    assert_eq!(p.size, (576.0, 720.0));
    let (w, _h, b) = draw(p, 10.0);
    // Master instance: red box centred at (3, 9) in → pixel (30, 10).
    assert_eq!(px(&b, w, 30, 10), [255, 0, 0]);
    // Outside it: white.
    assert_eq!(px(&b, w, 5, 5), [255, 255, 255]);
    // Ellipse in the group: blue (palette 4) at (4, 5) in → (40, 50).
    assert_eq!(px(&b, w, 40, 50), [0, 0, 255]);
    // Text: blue ink near (4, 2) in → around (40, 80).
    assert!(p.ops.iter().any(|o| matches!(o, Op::Text(_))));
    let (w2, _, b2) = draw(p, 40.0);
    let mut blue = 0;
    for y in 300..340 {
        for x in 100..220 {
            let c = px(&b2, w2, x, y);
            if c[2] > 150 && c[0] < 120 {
                blue += 1;
            }
        }
    }
    assert!(blue > 50, "text ink {blue}");
    // Page 2: the background page's green.
    let (w, _, b) = draw(&doc.pages[1], 10.0);
    assert_eq!(px(&b, w, 40, 50), [0, 255, 0]);
}

#[test]
fn refusals_are_plain() {
    assert!(open_vsdx(b"hello", None).is_err());
    let vdx = b"<?xml version='1.0'?><VisioDocument xmlns='http://schemas.microsoft.com/visio/2003/core'/>";
    let e = open_vsdx(vdx, None).unwrap_err();
    assert!(e.to_string().contains("VDX"), "{e}");
    let empty = zip(&[("visio/document.xml", b"<VisioDocument/>")]);
    let e = open_vsdx(&empty, None).unwrap_err();
    assert_eq!(
        e.to_string(),
        "damaged Visio drawing: Visio drawing without pages"
    );
    assert!(vsd_preview(b"not a compound file").is_err());
    assert!(!probe_vsdx(b"PK\x03\x04"));
}

#[test]
fn hostile_inputs_do_not_panic() {
    let d = sample();
    // Truncations and byte flips of a valid package.
    for cut in (0..d.len()).step_by(97) {
        let _ = open_vsdx(&d[..cut], None);
    }
    let mut x = d.clone();
    for i in (0..x.len()).step_by(53) {
        x[i] ^= 0x5A;
        let _ = open_vsdx(&x, None);
        x[i] ^= 0x5A;
    }
}

#[test]
fn arcs_bow_and_ellipse() {
    // A half-disc from an ArcTo: chord (0,0)→(2,0), bow 1 (to the right of travel: below).
    let mut p = Page::new("p", (200.0, 200.0));
    let rows = vec![
        sheet::Row {
            t: Some("MoveTo".into()),
            key: "1".into(),
            del: false,
            cells: [("X", "0"), ("Y", "1")]
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        sheet::Cell {
                            v: v.to_string(),
                            f: None,
                        },
                    )
                })
                .collect(),
        },
        sheet::Row {
            t: Some("ArcTo".into()),
            key: "2".into(),
            del: false,
            cells: [("X", "2"), ("Y", "1"), ("A", "1")]
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        sheet::Cell {
                            v: v.to_string(),
                            f: None,
                        },
                    )
                })
                .collect(),
        },
    ];
    let bb = {
        let path = draw::test_path(&rows, 2.0, 2.0);
        specular_drawpage::Shape::bounding_box(&path)
    };
    assert!(
        (bb.y0 - 0.0).abs() < 1e-3 && (bb.y1 - 1.0).abs() < 1e-3,
        "{bb:?}"
    );
    p.ops.clear();
}

#[test]
fn vsd_property_set_thumbnail() {
    // A property set with one section holding PIDSI_THUMBNAIL as CF_DIB.
    let dib = [40u8, 0, 0, 0, 1, 0, 0, 0];
    let mut prop = Vec::new();
    prop.extend_from_slice(&0x47u32.to_le_bytes());
    prop.extend_from_slice(&((8 + dib.len()) as u32).to_le_bytes());
    prop.extend_from_slice(&(-1i32).to_le_bytes());
    prop.extend_from_slice(&8u32.to_le_bytes());
    prop.extend_from_slice(&dib);
    let mut sec = Vec::new();
    sec.extend_from_slice(&((16 + prop.len()) as u32).to_le_bytes());
    sec.extend_from_slice(&1u32.to_le_bytes());
    sec.extend_from_slice(&0x11u32.to_le_bytes());
    sec.extend_from_slice(&16u32.to_le_bytes());
    sec.extend_from_slice(&prop);
    let mut ps = vec![0xFE, 0xFF, 0, 0, 0, 0, 0, 0];
    ps.extend_from_slice(&[0; 16]);
    ps.extend_from_slice(&1u32.to_le_bytes());
    ps.extend_from_slice(&[0; 16]);
    ps.extend_from_slice(&48u32.to_le_bytes());
    ps.extend_from_slice(&sec);
    let (cf, data) = vsd::thumbnail_property(&ps).unwrap();
    assert_eq!(cf, 8);
    assert_eq!(data, &dib);
    // METAFILEPICT + a WMF header and a SetWindowExt record → placeable WMF.
    let mut mf = Vec::new();
    for v in [8u16, 2540, 1270, 0] {
        mf.extend_from_slice(&v.to_le_bytes());
    }
    for v in [1u16, 9, 0x300, 0, 0, 5, 0, 0, 0] {
        mf.extend_from_slice(&v.to_le_bytes());
    }
    mf.extend_from_slice(&5u32.to_le_bytes());
    for v in [0x020Cu16, 500, 1000] {
        mf.extend_from_slice(&v.to_le_bytes());
    }
    mf.extend_from_slice(&3u32.to_le_bytes());
    mf.extend_from_slice(&0u16.to_le_bytes());
    let w = vsd::placeable_wmf(&mf).unwrap();
    assert_eq!(&w[..4], &0x9AC6_CDD7u32.to_le_bytes());
    assert_eq!(i16::from_le_bytes([w[10], w[11]]), 1000);
    assert_eq!(i16::from_le_bytes([w[12], w[13]]), 500);
    assert_eq!(u16::from_le_bytes([w[14], w[15]]), 1000);
}

fn corpus() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("SPECULAR_VISIO_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tools/corpus/design2/visio")
        });
    dir.is_dir().then_some(dir)
}

/// Every corpus file opens (or is refused) without panicking, and its pages draw.
#[test]
fn corpus_opens() {
    let Some(dir) = corpus() else {
        eprintln!("Visio corpus absent; skipped");
        return;
    };
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let d = std::fs::read(&p).unwrap();
        match ext.as_str() {
            "vsdx" | "vstx" | "vssx" | "vsdm" => {
                let doc = open_vsdx(&d, None).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                for pg in &doc.pages {
                    let _ = draw(pg, 12.0);
                }
                n += 1;
            }
            "vsd" | "vss" | "vst" => {
                let probed = probe_vsd_file(&d);
                match vsd_preview(&d) {
                    Ok(pr) => assert!(probed && !pr.bytes.is_empty()),
                    Err(e) => eprintln!("{}: {e}", p.display()),
                }
                n += 1;
            }
            _ => {}
        }
    }
    assert!(n > 0);
}
