//! Office documents shown by the preview they store, never rendered.
//!
//! - **OOXML** (Word `.docx/.docm/.dotx/.dotm`, Excel `.xlsx/.xlsm/.xltx/.xltm/.xlsb`,
//!   PowerPoint `.pptx/.pptm/.potx/.potm/.ppsx/.ppsm`): the package's thumbnail part (named by
//!   the root relationship, else `docProps/thumbnail.*`), properties from `docProps/core.xml`
//!   and `docProps/app.xml`. A password-protected package (a compound file holding
//!   `EncryptionInfo` and `EncryptedPackage`) is refused by name.
//! - **Office 97–2003** (`.doc`, `.xls`, `.ppt` and their templates): the compound file's
//!   summary information: its `PIDSI_THUMBNAIL` clipboard data (DIB, metafile picture,
//!   enhanced metafile, Macintosh PICT, or a PNG or JPEG) and the summary and document
//!   summary properties.
//! - **iWork** (`.pages`, `.numbers`, `.key`; single-file packages): `preview.jpg` (2013 and
//!   later) or `QuickLook/Thumbnail.jpg` (iWork '09), else the smaller previews.
//! - **OpenDocument** `meta.xml` properties ([`odf_properties`]); the thumbnail itself is read
//!   by `specular-odf`.
//!
//! A document without a preview opens with [`Document::preview`] `None`, so the caller can
//! say so and still show the properties. Every read is bounded; nothing here panics on a
//! hostile file.

mod iwork;
mod odf;
mod ole;
mod ooxml;
mod xml;

#[doc(hidden)]
pub mod fixture;

use std::ops::Range;

pub use odf::odf_properties;

/// Parts read at most (property XML, a property set stream, a preview).
pub const MAX_PART: u64 = 64 << 20;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not an office document")]
    NotOffice,
    /// Password-protected: what it is ("Word document", "Excel workbook", …, or "Office
    /// document" when the package does not say).
    #[error("password-protected {0}: opening it needs the password, which is not supported")]
    Encrypted(&'static str),
    #[error("{0}")]
    Unsupported(String),
    #[error("damaged office document: {0}")]
    Malformed(String),
}

/// Where a document comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Office Open XML packages (Office 2007 and later).
    Ooxml,
    /// Office 97–2003 compound files.
    Legacy,
    Iwork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum App {
    Word,
    Excel,
    PowerPoint,
    Pages,
    Numbers,
    Keynote,
}

impl App {
    /// What a document of this application is called, in English: "Word document".
    pub fn noun(self) -> &'static str {
        match self {
            App::Word => "Word document",
            App::Excel => "Excel workbook",
            App::PowerPoint => "PowerPoint presentation",
            App::Pages => "Pages document",
            App::Numbers => "Numbers spreadsheet",
            App::Keynote => "Keynote presentation",
        }
    }
}

/// The image format of a preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Png,
    Jpeg,
    Gif,
    Tiff,
    Bmp,
    Wmf,
    Emf,
    Pict,
}

impl PreviewKind {
    pub fn name(self) -> &'static str {
        match self {
            PreviewKind::Png => "PNG",
            PreviewKind::Jpeg => "JPEG",
            PreviewKind::Gif => "GIF",
            PreviewKind::Tiff => "TIFF",
            PreviewKind::Bmp => "BMP",
            PreviewKind::Wmf => "WMF",
            PreviewKind::Emf => "EMF",
            PreviewKind::Pict => "PICT",
        }
    }

    /// By the image's own signature.
    pub(crate) fn sniff(b: &[u8]) -> Option<PreviewKind> {
        Some(if b.starts_with(b"\x89PNG") {
            PreviewKind::Png
        } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
            PreviewKind::Jpeg
        } else if b.starts_with(b"GIF8") {
            PreviewKind::Gif
        } else if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") {
            PreviewKind::Tiff
        } else if b.starts_with(b"BM") {
            PreviewKind::Bmp
        } else if b.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A])
            || (b.len() > 6 && matches!(b[0], 1 | 2) && b[1] == 0 && b[2] == 9 && b[3] == 0)
        {
            PreviewKind::Wmf
        } else if b.len() > 44 && b.starts_with(&[1, 0, 0, 0]) && &b[40..44] == b" EMF" {
            PreviewKind::Emf
        } else {
            return None;
        })
    }
}

/// A preview's bytes: in place in the file, or made (inflated, a header added).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bytes {
    Range(Range<usize>),
    Owned(Vec<u8>),
}

impl Bytes {
    pub fn get<'a>(&'a self, file: &'a [u8]) -> &'a [u8] {
        match self {
            Bytes::Range(r) => file.get(r.clone()).unwrap_or(&[]),
            Bytes::Owned(v) => v,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub kind: PreviewKind,
    pub bytes: Bytes,
    /// Where it was found: "docProps/thumbnail.jpeg", "SummaryInformation", "preview.jpg".
    pub part: String,
}

/// Document properties. Dates are ISO 8601 as stored (OOXML, OpenDocument) or made from
/// the FILETIME (Office 97–2003), in UTC.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Properties {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
    pub keywords: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub last_modified_by: Option<String>,
    pub revision: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub printed: Option<String>,
    /// The application that saved it, with its version where given.
    pub application: Option<String>,
    pub company: Option<String>,
    pub template: Option<String>,
    pub pages: Option<u64>,
    pub words: Option<u64>,
    pub characters: Option<u64>,
    pub paragraphs: Option<u64>,
    pub lines: Option<u64>,
    pub slides: Option<u64>,
    pub notes: Option<u64>,
    pub hidden_slides: Option<u64>,
    /// Worksheets (Excel: from the parts' titles), tables (OpenDocument).
    pub sheets: Option<u64>,
    /// Total editing time, in minutes.
    pub edit_minutes: Option<u64>,
}

impl Properties {
    pub fn is_empty(&self) -> bool {
        *self == Properties::default()
    }
}

/// An office document: what it is, its preview if it has one, and its properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub family: Family,
    pub app: App,
    /// A template (`.dotx`, `.xltx`, `.potx`, …).
    pub template: bool,
    /// Macro-enabled (`.docm`, `.xlsm`, `.pptm`, …; a VBA project in a 97–2003 file).
    pub macros: bool,
    /// Excel's binary workbook (`.xlsb`).
    pub binary: bool,
    pub preview: Option<Preview>,
    pub props: Properties,
}

/// What a file is, from its bytes: `None` for anything that is not an office document of a
/// kind read here. A password-protected OOXML package is `Some((Ooxml, app))`, the app from
/// its encryption metadata where it says (else Word).
pub fn probe(data: &[u8]) -> Option<(Family, App)> {
    if specular_cfb::sniff(data) {
        return ole::probe(data);
    }
    if data.starts_with(b"PK\x03\x04") {
        let zip = specular_ora::Zip::parse(data).ok()?;
        if let Some(app) = ooxml::probe(data, &zip) {
            return Some((Family::Ooxml, app));
        }
        if let Some(app) = iwork::probe(&zip) {
            return Some((Family::Iwork, app));
        }
    }
    None
}

/// Reads the preview and properties. `hint`: the application the file name says (from its
/// extension), used where the content cannot tell (iWork packages).
pub fn open(data: &[u8], hint: Option<App>) -> Result<Document, Error> {
    if specular_cfb::sniff(data) {
        return ole::open(data, hint);
    }
    if !data.starts_with(b"PK\x03\x04") {
        return Err(Error::NotOffice);
    }
    let zip = specular_ora::Zip::parse(data).map_err(zip_err)?;
    if ooxml::probe(data, &zip).is_some() {
        return ooxml::open(data, &zip);
    }
    if iwork::probe(&zip).is_some() {
        return iwork::open(data, &zip, hint);
    }
    Err(Error::NotOffice)
}

/// The application an extension names (`docx` → Word, `key` → Keynote, …).
pub fn app_of_extension(ext: &str) -> Option<App> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "doc" | "dot" | "docx" | "docm" | "dotx" | "dotm" => App::Word,
        "xls" | "xlt" | "xlsx" | "xlsm" | "xltx" | "xltm" | "xlsb" => App::Excel,
        "ppt" | "pot" | "pps" | "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => {
            App::PowerPoint
        }
        "pages" => App::Pages,
        "numbers" => App::Numbers,
        "key" => App::Keynote,
        _ => return None,
    })
}

pub(crate) fn zip_err(e: specular_ora::Error) -> Error {
    Error::Malformed(e.to_string())
}

/// A ZIP member as preview bytes: in place when stored, else inflated (bounded).
pub(crate) fn member_bytes(
    data: &[u8],
    zip: &specular_ora::Zip,
    e: &specular_ora::Entry,
) -> Result<Bytes, Error> {
    if let Some(r) = zip.stored_range(data, e).map_err(zip_err)? {
        return Ok(Bytes::Range(r));
    }
    Ok(Bytes::Owned(
        zip.read(data, e, MAX_PART).map_err(zip_err)?.into_owned(),
    ))
}

/// A number property, as stored ("12", " 3 ").
pub(crate) fn count(s: Option<String>) -> Option<u64> {
    s?.trim().parse().ok()
}

/// "Microsoft Office Word" + "16.0000" → "Microsoft Office Word 16.0000".
pub(crate) fn with_version(app: Option<String>, version: Option<String>) -> Option<String> {
    match (app, version) {
        (Some(a), Some(v)) => Some(format!("{a} {v}")),
        (a, _) => a,
    }
}

#[cfg(test)]
mod tests;
