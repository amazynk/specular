//! Office Open XML packages (ECMA-376 Part 2, Open Packaging Conventions): the main part's
//! content type, the thumbnail relationship and the core and extended properties.

use specular_ora::Zip;

use crate::xml::{self, text};
use crate::{
    App, Document, Error, Family, MAX_PART, Preview, PreviewKind, Properties, count, member_bytes,
    with_version, zip_err,
};

const THUMBNAIL_REL: &str = "/metadata/thumbnail";

/// Word, Excel or PowerPoint, by the package's main folder (`[Content_Types].xml` beside it).
pub(crate) fn probe(data: &[u8], zip: &Zip) -> Option<App> {
    zip.find("[Content_Types].xml")?;
    let has = |n: &str| zip.find(n).is_some();
    if has("word/document.xml") || has("word/glossary/document.xml") {
        Some(App::Word)
    } else if has("xl/workbook.xml") || has("xl/workbook.bin") {
        Some(App::Excel)
    } else if has("ppt/presentation.xml") {
        Some(App::PowerPoint)
    } else {
        // A renamed main part: the content types say.
        let types = part(data, zip, "[Content_Types].xml").ok()?;
        main_type(&types).map(|(app, _)| app)
    }
}

fn part(data: &[u8], zip: &Zip, name: &str) -> Result<Vec<u8>, Error> {
    let e = zip
        .find(name)
        .ok_or_else(|| Error::Malformed(format!("no {name}")))?;
    Ok(zip.read(data, e, MAX_PART).map_err(zip_err)?.into_owned())
}

/// The main document's application and content type, from `[Content_Types].xml`.
fn main_type(types: &[u8]) -> Option<(App, String)> {
    xml::elements(types)
        .iter()
        .filter(|e| e.name == "Override")
        .filter_map(|e| e.attr("ContentType"))
        .find_map(|t| {
            let l = t.to_ascii_lowercase();
            if !l.contains(".main") && !l.ends_with("binary.macroenabled.main") {
                return None;
            }
            let app = if l.contains("wordprocessingml") || l.contains("ms-word") {
                App::Word
            } else if l.contains("spreadsheetml") || l.contains("ms-excel") {
                App::Excel
            } else if l.contains("presentationml") || l.contains("ms-powerpoint") {
                App::PowerPoint
            } else {
                return None;
            };
            Some((app, l))
        })
}

pub(crate) fn open(data: &[u8], zip: &Zip) -> Result<Document, Error> {
    let types = part(data, zip, "[Content_Types].xml")?;
    let main = main_type(&types);
    let app = main
        .as_ref()
        .map(|(a, _)| *a)
        .or_else(|| probe(data, zip))
        .ok_or(Error::NotOffice)?;
    let ct = main.map(|(_, t)| t).unwrap_or_default();
    let template = ct.contains(".template") || ct.contains("macroenabledtemplate");
    let macros = ct.contains("macroenabled")
        || zip
            .entries
            .iter()
            .any(|e| e.name.ends_with("/vbaProject.bin"));
    let binary = ct.contains(".binary.");
    let mut props = Properties::default();
    if let Ok(core) = part(data, zip, "docProps/core.xml") {
        let e = xml::elements(&core);
        props.title = text(&e, "title");
        props.subject = text(&e, "subject");
        props.author = text(&e, "creator");
        props.keywords = text(&e, "keywords");
        props.description = text(&e, "description");
        props.category = text(&e, "category");
        props.last_modified_by = text(&e, "lastModifiedBy");
        props.revision = text(&e, "revision");
        props.created = text(&e, "created");
        props.modified = text(&e, "modified");
        props.printed = text(&e, "lastPrinted");
    }
    if let Ok(ext) = part(data, zip, "docProps/app.xml") {
        let e = xml::elements(&ext);
        props.application = with_version(text(&e, "Application"), text(&e, "AppVersion"));
        props.company = text(&e, "Company");
        props.template = text(&e, "Template");
        props.pages = count(text(&e, "Pages"));
        props.words = count(text(&e, "Words"));
        props.characters = count(text(&e, "Characters"));
        props.paragraphs = count(text(&e, "Paragraphs"));
        props.lines = count(text(&e, "Lines"));
        props.slides = count(text(&e, "Slides"));
        props.notes = count(text(&e, "Notes"));
        props.hidden_slides = count(text(&e, "HiddenSlides"));
        props.edit_minutes = count(text(&e, "TotalTime"));
    }
    if app == App::Excel
        && let Ok(wb) = part(data, zip, "xl/workbook.xml")
    {
        let n = xml::elements(&wb)
            .iter()
            .filter(|e| e.name == "sheet")
            .count();
        props.sheets = Some(n as u64);
    }
    if app == App::PowerPoint && props.slides.is_none() {
        let n = zip
            .entries
            .iter()
            .filter(|e| {
                e.name
                    .strip_prefix("ppt/slides/slide")
                    .is_some_and(|r| r.ends_with(".xml"))
            })
            .count();
        props.slides = Some(n as u64);
    }
    Ok(Document {
        family: Family::Ooxml,
        app,
        template,
        macros,
        binary,
        preview: thumbnail(data, zip)?,
        props,
    })
}

/// The part the package relationship of type `.../metadata/thumbnail` names, else
/// `docProps/thumbnail.*`.
fn thumbnail(data: &[u8], zip: &Zip) -> Result<Option<Preview>, Error> {
    let mut names = Vec::new();
    if let Ok(rels) = part(data, zip, "_rels/.rels") {
        for e in xml::elements(&rels) {
            if e.name == "Relationship"
                && e.attr("Type").is_some_and(|t| t.ends_with(THUMBNAIL_REL))
                && e.attr("TargetMode") != Some("External")
                && let Some(t) = e.attr("Target")
            {
                names.push(t.trim_start_matches('/').to_owned());
            }
        }
    }
    names.extend(
        zip.entries
            .iter()
            .filter(|e| e.name.starts_with("docProps/thumbnail."))
            .map(|e| e.name.clone()),
    );
    for name in names {
        let Some(e) = zip.find(&name) else { continue };
        if e.size == 0 {
            continue;
        }
        let bytes = member_bytes(data, zip, e)?;
        let kind = PreviewKind::sniff(bytes.get(data)).or_else(|| by_extension(&name));
        let Some(kind) = kind else { continue };
        return Ok(Some(Preview {
            kind,
            bytes,
            part: name,
        }));
    }
    Ok(None)
}

fn by_extension(name: &str) -> Option<PreviewKind> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpeg" | "jpg" => PreviewKind::Jpeg,
        "png" => PreviewKind::Png,
        "wmf" => PreviewKind::Wmf,
        "emf" => PreviewKind::Emf,
        "gif" => PreviewKind::Gif,
        "tif" | "tiff" => PreviewKind::Tiff,
        "bmp" => PreviewKind::Bmp,
        _ => return None,
    })
}
