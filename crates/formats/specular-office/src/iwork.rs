//! Apple iWork single-file packages (ZIP): Pages, Numbers and Keynote 2013 and later
//! (`Index/Document.iwa` with `preview.jpg`, `preview-web.jpg`, `preview-micro.jpg`) and
//! iWork '09 (`index.xml` / `index.apxl`, with `QuickLook/Thumbnail.jpg`). Only the preview
//! is read: the documents themselves are Apple's undocumented archive formats.

use specular_ora::Zip;

use crate::{App, Document, Error, Family, Preview, PreviewKind, Properties, member_bytes};

/// Previews, largest first.
const PREVIEWS: &[&str] = &[
    "preview.jpg",
    "QuickLook/Thumbnail.jpg",
    "QuickLook/Thumbnail.png",
    "preview-web.jpg",
    "preview-micro.jpg",
];

/// The member names inside a package, with any single top folder taken off (a bundle zipped
/// by the Finder keeps `Name.pages/` in front).
fn names(zip: &Zip) -> impl Iterator<Item = &str> {
    zip.entries.iter().map(|e| names_one(&e.name))
}

pub(crate) fn probe(zip: &Zip) -> Option<App> {
    let mut iwa = false;
    let mut old = None;
    let mut slides = false;
    let mut tables = false;
    for n in names(zip) {
        match n {
            "Index/Document.iwa" | "Index.zip" => iwa = true,
            "index.apxl" | "index.apxl.gz" => old = Some(App::Keynote),
            "index.xml" | "index.xml.gz" if old.is_none() => old = Some(App::Pages),
            _ => {}
        }
        slides |= n.starts_with("Index/Slide") || n.starts_with("Index/TemplateSlide");
        tables |= n.starts_with("Index/CalculationEngine") || n.starts_with("Index/Tables/");
    }
    if iwa {
        return Some(if slides {
            App::Keynote
        } else if tables {
            App::Numbers
        } else {
            App::Pages
        });
    }
    // iWork '09 also needs its QuickLook folder: plain `index.xml` is far too common.
    let quicklook = names(zip).any(|n| n.starts_with("QuickLook/"));
    old.filter(|_| quicklook)
}

pub(crate) fn open(data: &[u8], zip: &Zip, hint: Option<App>) -> Result<Document, Error> {
    let found = probe(zip).ok_or(Error::NotOffice)?;
    // The content tells Keynote reliably; Pages from Numbers only by the extension.
    let app = match hint {
        Some(h @ (App::Pages | App::Numbers | App::Keynote)) if found != App::Keynote => h,
        _ => found,
    };
    let mut preview = None;
    'look: for want in PREVIEWS {
        for e in &zip.entries {
            let short = names_one(&e.name);
            if short != *want || e.size == 0 {
                continue;
            }
            let bytes = member_bytes(data, zip, e)?;
            if let Some(kind) = PreviewKind::sniff(bytes.get(data)) {
                preview = Some(Preview {
                    kind,
                    bytes,
                    part: short.to_owned(),
                });
                break 'look;
            }
        }
    }
    Ok(Document {
        family: Family::Iwork,
        app,
        template: false,
        macros: false,
        binary: false,
        preview,
        props: Properties::default(),
    })
}

/// A member's name without a bundle folder in front.
fn names_one(n: &str) -> &str {
    match n.split_once('/') {
        Some((top, rest))
            if top.ends_with(".pages") || top.ends_with(".numbers") || top.ends_with(".key") =>
        {
            rest
        }
        _ => n,
    }
}
