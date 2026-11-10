//! A minimal PowerPoint (PresentationML, ECMA-376) writer for key images: one slide per image
//! (16:9), the title at the top, the picture fitted below it, the notes page holding the
//! measurements. Only the parts a reader needs: one master with one blank layout and a
//! theme, a notes master, the slides, their notes and pictures, and the document properties.
//! The container is a ZIP (the `zip` crate); everything else is written here.

use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use crate::xml_escape;

/// Slide size, EMU (13.333 × 7.5 in).
pub const SLIDE: (i64, i64) = (12_192_000, 6_858_000);
const NOTES: (i64, i64) = (6_858_000, 9_144_000);
const MARGIN: i64 = 381_000;
const TITLE_H: i64 = 700_000;

/// One slide.
#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub title: String,
    /// The picture as PNG, and its size in px.
    pub png: Vec<u8>,
    pub size: (u32, u32),
    /// Describes the picture for screen readers.
    pub alt: String,
    /// The notes page, one paragraph per line.
    pub notes: Vec<String>,
}

/// The deck.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Deck {
    pub title: String,
    pub slides: Vec<Slide>,
    /// Black slides with white titles (medical images), else white.
    pub dark: bool,
}

const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                  xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
                  xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const CT: &str = "application/vnd.openxmlformats-officedocument.presentationml";

const GROUP: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/>\
<a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";

const CLR_MAP: &str = "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" \
accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" \
hlink=\"hlink\" folHlink=\"folHlink\"/>";

fn rels(items: &[(&str, &str, String)]) -> String {
    let mut o = format!(
        "{HEAD}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
    );
    for (id, ty, target) in items {
        let ty = if ty.starts_with("http") {
            (*ty).to_owned()
        } else {
            format!("{REL}/{ty}")
        };
        let _ = write!(
            o,
            "<Relationship Id=\"{id}\" Type=\"{ty}\" Target=\"{}\"/>",
            xml_escape(target)
        );
    }
    o.push_str("</Relationships>");
    o
}

/// Whether text reads right to left (its first strong letter is Hebrew or Arabic).
fn is_rtl(s: &str) -> bool {
    s.chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF))
}

fn paragraph(text: &str, size: u32, bold: bool, colour: Option<&str>) -> String {
    let rtl = if is_rtl(text) {
        "<a:pPr rtl=\"1\"/>"
    } else {
        ""
    };
    let fill = colour.map_or(String::new(), |c| {
        format!("<a:solidFill><a:srgbClr val=\"{c}\"/></a:solidFill>")
    });
    format!(
        "<a:p>{rtl}<a:r><a:rPr lang=\"en-US\" sz=\"{size}\"{}>{fill}</a:rPr><a:t>{}</a:t></a:r></a:p>",
        if bold { " b=\"1\"" } else { "" },
        xml_escape(text)
    )
}

fn theme(name: &str) -> String {
    let fill3 = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>".repeat(3);
    let ln3 =
        "<a:ln w=\"9525\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>".repeat(3);
    let fx3 = "<a:effectStyle><a:effectLst/></a:effectStyle>".repeat(3);
    let font =
        |f: &str| format!("<a:latin typeface=\"{f}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>");
    format!(
        "{HEAD}<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"{name}\">\
<a:themeElements><a:clrScheme name=\"Specular\">\
<a:dk1><a:srgbClr val=\"000000\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1>\
<a:dk2><a:srgbClr val=\"1F1F1F\"/></a:dk2><a:lt2><a:srgbClr val=\"EEEEEE\"/></a:lt2>\
<a:accent1><a:srgbClr val=\"2F6FDE\"/></a:accent1><a:accent2><a:srgbClr val=\"E07B39\"/></a:accent2>\
<a:accent3><a:srgbClr val=\"7A7A7A\"/></a:accent3><a:accent4><a:srgbClr val=\"F2C12E\"/></a:accent4>\
<a:accent5><a:srgbClr val=\"4AA3DF\"/></a:accent5><a:accent6><a:srgbClr val=\"5DAE4F\"/></a:accent6>\
<a:hlink><a:srgbClr val=\"2F6FDE\"/></a:hlink><a:folHlink><a:srgbClr val=\"8E44AD\"/></a:folHlink>\
</a:clrScheme><a:fontScheme name=\"Specular\"><a:majorFont>{}</a:majorFont><a:minorFont>{}</a:minorFont></a:fontScheme>\
<a:fmtScheme name=\"Specular\"><a:fillStyleLst>{fill3}</a:fillStyleLst><a:lnStyleLst>{ln3}</a:lnStyleLst>\
<a:effectStyleLst>{fx3}</a:effectStyleLst><a:bgFillStyleLst>{fill3}</a:bgFillStyleLst></a:fmtScheme>\
</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        font("Calibri"),
        font("Calibri")
    )
}

/// The picture's place on a slide: fitted under the title, centred, aspect kept.
pub fn picture_box(size: (u32, u32)) -> (i64, i64, i64, i64) {
    let (aw, ah) = (SLIDE.0 - 2 * MARGIN, SLIDE.1 - 2 * MARGIN - TITLE_H);
    let (w, h) = (i64::from(size.0.max(1)), i64::from(size.1.max(1)));
    let (cx, cy) = if aw * h <= ah * w {
        (aw, aw * h / w)
    } else {
        (ah * w / h, ah)
    };
    ((SLIDE.0 - cx) / 2, MARGIN + TITLE_H + (ah - cy) / 2, cx, cy)
}

fn slide_xml(s: &Slide, dark: bool) -> String {
    let bg = if dark {
        "<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"000000\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"
    } else {
        ""
    };
    let (x, y, cx, cy) = picture_box(s.size);
    format!(
        "{HEAD}<p:sld {NS}><p:cSld>{bg}<p:spTree>{GROUP}\
<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{MARGIN}\" y=\"{MARGIN}\"/><a:ext cx=\"{}\" cy=\"{TITLE_H}\"/></a:xfrm>\
<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>{}</p:txBody></p:sp>\
<p:pic><p:nvPicPr><p:cNvPr id=\"3\" name=\"Picture\" descr=\"{}\"/>\
<p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
<p:blipFill><a:blip r:embed=\"rId2\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
<p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>\
</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
        SLIDE.0 - 2 * MARGIN,
        paragraph(&s.title, 2400, true, dark.then_some("FFFFFF")),
        xml_escape(&s.alt)
    )
}

fn notes_xml(s: &Slide) -> String {
    let mut body: String = s
        .notes
        .iter()
        .map(|l| paragraph(l, 1200, false, None))
        .collect();
    if body.is_empty() {
        body = "<a:p><a:endParaRPr lang=\"en-US\"/></a:p>".into();
    }
    format!(
        "{HEAD}<p:notes {NS}><p:cSld><p:spTree>{GROUP}\
<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr>\
<p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"685800\" y=\"4400550\"/><a:ext cx=\"5486400\" cy=\"3600450\"/></a:xfrm></p:spPr>\
<p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp>\
</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"
    )
}

/// The deck as a `.pptx` file.
pub fn write(deck: &Deck) -> Result<Vec<u8>, String> {
    let n = deck.slides.len();
    let mut parts: Vec<(String, Vec<u8>, bool)> = Vec::new();
    let mut add =
        |name: &str, data: Vec<u8>, deflate: bool| parts.push((name.to_owned(), data, deflate));

    // [Content_Types].xml
    let mut ct = format!(
        "{HEAD}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Default Extension=\"png\" ContentType=\"image/png\"/>\
<Override PartName=\"/ppt/presentation.xml\" ContentType=\"{CT}.presentation.main+xml\"/>\
<Override PartName=\"/ppt/presProps.xml\" ContentType=\"{CT}.presProps+xml\"/>\
<Override PartName=\"/ppt/viewProps.xml\" ContentType=\"{CT}.viewProps+xml\"/>\
<Override PartName=\"/ppt/tableStyles.xml\" ContentType=\"{CT}.tableStyles+xml\"/>\
<Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"{CT}.slideMaster+xml\"/>\
<Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"{CT}.slideLayout+xml\"/>\
<Override PartName=\"/ppt/notesMasters/notesMaster1.xml\" ContentType=\"{CT}.notesMaster+xml\"/>\
<Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>\
<Override PartName=\"/ppt/theme/theme2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>"
    );
    for i in 1..=n {
        let _ = write!(
            ct,
            "<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"{CT}.slide+xml\"/>\
<Override PartName=\"/ppt/notesSlides/notesSlide{i}.xml\" ContentType=\"{CT}.notesSlide+xml\"/>"
        );
    }
    ct.push_str("</Types>");
    add("[Content_Types].xml", ct.into_bytes(), true);
    add(
        "_rels/.rels",
        rels(&[
            ("rId1", "officeDocument", "ppt/presentation.xml".into()),
            (
                "rId2",
                "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties",
                "docProps/core.xml".into(),
            ),
            ("rId3", "extended-properties", "docProps/app.xml".into()),
        ])
        .into_bytes(),
        true,
    );
    add(
        "docProps/core.xml",
        format!(
            "{HEAD}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title>\
<dc:creator>Specular</dc:creator></cp:coreProperties>",
            xml_escape(&deck.title)
        )
        .into_bytes(),
        true,
    );
    add(
        "docProps/app.xml",
        format!(
            "{HEAD}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">\
<Application>Specular</Application><Slides>{n}</Slides><Notes>{n}</Notes></Properties>"
        )
        .into_bytes(),
        true,
    );

    // The presentation.
    let mut ids = String::new();
    for i in 0..n {
        let _ = write!(ids, "<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 256 + i, 10 + i);
    }
    let sld_lst = if n > 0 {
        format!("<p:sldIdLst>{ids}</p:sldIdLst>")
    } else {
        String::new()
    };
    add(
        "ppt/presentation.xml",
        format!(
            "{HEAD}<p:presentation {NS} saveSubsetFonts=\"1\">\
<p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
<p:notesMasterIdLst><p:notesMasterId r:id=\"rId2\"/></p:notesMasterIdLst>{sld_lst}\
<p:sldSz cx=\"{}\" cy=\"{}\"/><p:notesSz cx=\"{}\" cy=\"{}\"/></p:presentation>",
            SLIDE.0, SLIDE.1, NOTES.0, NOTES.1
        )
        .into_bytes(),
        true,
    );
    let mut pr = vec![
        (
            "rId1".to_owned(),
            "slideMaster",
            "slideMasters/slideMaster1.xml".to_owned(),
        ),
        (
            "rId2".to_owned(),
            "notesMaster",
            "notesMasters/notesMaster1.xml".to_owned(),
        ),
        ("rId3".to_owned(), "theme", "theme/theme1.xml".to_owned()),
        ("rId4".to_owned(), "presProps", "presProps.xml".to_owned()),
        ("rId5".to_owned(), "viewProps", "viewProps.xml".to_owned()),
        (
            "rId6".to_owned(),
            "tableStyles",
            "tableStyles.xml".to_owned(),
        ),
    ];
    for i in 0..n {
        pr.push((
            format!("rId{}", 10 + i),
            "slide",
            format!("slides/slide{}.xml", i + 1),
        ));
    }
    let pr: Vec<(&str, &str, String)> = pr
        .iter()
        .map(|(a, b, c)| (a.as_str(), *b, c.clone()))
        .collect();
    add(
        "ppt/_rels/presentation.xml.rels",
        rels(&pr).into_bytes(),
        true,
    );
    add(
        "ppt/presProps.xml",
        format!("{HEAD}<p:presentationPr {NS}/>").into_bytes(),
        true,
    );
    add(
        "ppt/viewProps.xml",
        format!("{HEAD}<p:viewPr {NS}/>").into_bytes(),
        true,
    );
    add(
        "ppt/tableStyles.xml",
        format!(
            "{HEAD}<a:tblStyleLst xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
def=\"{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}\"/>"
        )
        .into_bytes(),
        true,
    );

    // Master, layout, notes master, themes.
    add(
        "ppt/slideMasters/slideMaster1.xml",
        format!(
            "{HEAD}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
<p:spTree>{GROUP}</p:spTree></p:cSld>{CLR_MAP}\
<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
<p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"
        )
        .into_bytes(),
        true,
    );
    add(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        rels(&[
            (
                "rId1",
                "slideLayout",
                "../slideLayouts/slideLayout1.xml".into(),
            ),
            ("rId2", "theme", "../theme/theme1.xml".into()),
        ])
        .into_bytes(),
        true,
    );
    add(
        "ppt/slideLayouts/slideLayout1.xml",
        format!(
            "{HEAD}<p:sldLayout {NS} type=\"blank\" preserve=\"1\"><p:cSld name=\"Blank\">\
<p:spTree>{GROUP}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
        )
        .into_bytes(),
        true,
    );
    add(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        rels(&[(
            "rId1",
            "slideMaster",
            "../slideMasters/slideMaster1.xml".into(),
        )])
        .into_bytes(),
        true,
    );
    add(
        "ppt/notesMasters/notesMaster1.xml",
        format!(
            "{HEAD}<p:notesMaster {NS}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
<p:spTree>{GROUP}</p:spTree></p:cSld>{CLR_MAP}</p:notesMaster>"
        )
        .into_bytes(),
        true,
    );
    add(
        "ppt/notesMasters/_rels/notesMaster1.xml.rels",
        rels(&[("rId1", "theme", "../theme/theme2.xml".into())]).into_bytes(),
        true,
    );
    add("ppt/theme/theme1.xml", theme("Specular").into_bytes(), true);
    add(
        "ppt/theme/theme2.xml",
        theme("Specular Notes").into_bytes(),
        true,
    );

    // The slides.
    for (i, s) in deck.slides.iter().enumerate() {
        let k = i + 1;
        add(
            &format!("ppt/slides/slide{k}.xml"),
            slide_xml(s, deck.dark).into_bytes(),
            true,
        );
        add(
            &format!("ppt/slides/_rels/slide{k}.xml.rels"),
            rels(&[
                (
                    "rId1",
                    "slideLayout",
                    "../slideLayouts/slideLayout1.xml".into(),
                ),
                ("rId2", "image", format!("../media/image{k}.png")),
                (
                    "rId3",
                    "notesSlide",
                    format!("../notesSlides/notesSlide{k}.xml"),
                ),
            ])
            .into_bytes(),
            true,
        );
        add(&format!("ppt/media/image{k}.png"), s.png.clone(), false);
        add(
            &format!("ppt/notesSlides/notesSlide{k}.xml"),
            notes_xml(s).into_bytes(),
            true,
        );
        add(
            &format!("ppt/notesSlides/_rels/notesSlide{k}.xml.rels"),
            rels(&[
                (
                    "rId1",
                    "notesMaster",
                    "../notesMasters/notesMaster1.xml".into(),
                ),
                ("rId2", "slide", format!("../slides/slide{k}.xml")),
            ])
            .into_bytes(),
            true,
        );
    }

    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data, deflate) in parts {
        let method = if deflate {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let opts = zip::write::SimpleFileOptions::default().compression_method(method);
        z.start_file(name, opts).map_err(|e| e.to_string())?;
        z.write_all(&data).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}
