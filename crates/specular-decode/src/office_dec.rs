//! Office documents by the preview they store (`specular-office`): Word, Excel and PowerPoint
//! (OOXML and 97–2003) and Apple iWork. The document is never rendered. Its properties come
//! with the session ([`Session::document`]); a file without a preview opens as a
//! [`NoPreviewSession`] the app shows as "No preview inside this file", with the properties
//! still in the info panel. Password-protected OOXML packages are refused by name.
//!
//! OpenDocument text documents and spreadsheets take the same sessions from the ODF decoder
//! ([`open_document`]).

use std::sync::Arc;

use specular_core::{CancelToken, Raster, Source};
use specular_office::{App, Family};

use crate::embedded_dec::open_embedded_document;
use crate::{
    DecodeError, DecodeRequest, Decoder, DocumentFacts, Format, ImageInfo, Orientation, Result,
    Session,
};

/// One decoder per family, so each has its format and extensions.
pub struct OfficeDecoder(pub Family);

fn family_format(f: Family) -> Format {
    match f {
        Family::Ooxml => Format::Ooxml,
        Family::Legacy => Format::MsOffice,
        Family::Iwork => Format::Iwork,
    }
}

fn office_error(e: specular_office::Error) -> DecodeError {
    use specular_office::Error as E;
    match e {
        E::NotOffice => DecodeError::Malformed("not an office document".into()),
        E::Encrypted(_) | E::Unsupported(_) => DecodeError::Unsupported(e.to_string()),
        E::Malformed(m) => DecodeError::Malformed(m),
    }
}

impl Decoder for OfficeDecoder {
    fn format(&self) -> Format {
        family_format(self.0)
    }
    /// ZIP and compound files carry no signature of their own: `probe_file` looks inside.
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_office::probe(data).is_some_and(|(f, _)| f == self.0)
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Family::Ooxml => &[
                "docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "xltx", "xltm", "xlsb", "pptx",
                "pptm", "potx", "potm", "ppsx", "ppsm",
            ],
            Family::Legacy => &["doc", "dot", "xls", "xlt", "ppt", "pot", "pps"],
            Family::Iwork => &["pages", "numbers", "key"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let hint = source
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .and_then(specular_office::app_of_extension);
        let data = source.bytes();
        let doc = specular_office::open(data, hint).map_err(office_error)?;
        // The format is the file's family (an encrypted OOXML package is a compound file).
        let format = family_format(doc.family);
        let mut kind = match (doc.app, doc.template) {
            (App::Word, true) => "Word template".to_owned(),
            (App::Excel, true) => "Excel template".to_owned(),
            (App::PowerPoint, true) => "PowerPoint template".to_owned(),
            (app, _) => app.noun().to_owned(),
        };
        if doc.family == Family::Legacy {
            kind.push_str(" (97–2003)");
        }
        let facts = DocumentFacts {
            kind: app_key(doc.app),
            legacy: doc.family == Family::Legacy,
            template: doc.template,
            macros: doc.macros,
            preview: doc.preview.as_ref().map(|p| p.kind.name()),
            props: doc.props,
            items: None,
        };
        let image = doc.preview.map(|p| match p.bytes {
            specular_office::Bytes::Range(r) => {
                Source::from_slice("office-preview", source.clone(), &data[r])
                    .unwrap_or_else(|| Source::from_vec("office-preview", Vec::new()))
            }
            specular_office::Bytes::Owned(v) => Source::from_vec("office-preview", v),
        });
        open_document(source, image, format, &kind, facts)
    }
}

/// The session of a document shown by its preview (`image`), or the no-preview session.
pub(crate) fn open_document(
    source: Arc<Source>,
    image: Option<Source>,
    format: Format,
    kind: &str,
    facts: DocumentFacts,
) -> Result<Arc<dyn Session>> {
    match image {
        Some(image) => open_embedded_document(
            source,
            image,
            format,
            &format!("{kind}, embedded preview only"),
            facts,
        ),
        None => Ok(Arc::new(NoPreviewSession::new(source, format, kind, facts))),
    }
}

/// The reason a no-preview document has no pixels (the gallery's tooltip, any decode).
pub const NO_PREVIEW: &str = "there is no preview inside this file";

/// A document without a preview: its facts, no pixels. Decodes fail with
/// [`DecodeError::Unsupported`] naming [`NO_PREVIEW`].
pub struct NoPreviewSession {
    source: Arc<Source>,
    info: ImageInfo,
    facts: DocumentFacts,
}

impl NoPreviewSession {
    fn new(source: Arc<Source>, format: Format, kind: &str, facts: DocumentFacts) -> Self {
        Self::with_coding(source, format, format!("{kind}, no preview inside"), facts)
    }

    /// A no-preview session whose coding line is `coding` (a playlist's "M3U playlist, 12
    /// items").
    pub(crate) fn with_coding(
        source: Arc<Source>,
        format: Format,
        coding: String,
        facts: DocumentFacts,
    ) -> Self {
        let info = ImageInfo {
            format,
            width: 1,
            height: 1,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding,
            embedded_preview: None,
            fast_regions: false,
            scales: &[1],
        };
        NoPreviewSession {
            source,
            info,
            facts,
        }
    }
}

impl Session for NoPreviewSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, _req: &DecodeRequest, _cancel: &CancelToken) -> Result<Raster> {
        Err(DecodeError::Unsupported(NO_PREVIEW.into()))
    }
    fn document(&self) -> Option<&DocumentFacts> {
        Some(&self.facts)
    }
}

/// The stable key of what a document is ([`DocumentFacts::kind`]).
fn app_key(app: App) -> &'static str {
    match app {
        App::Word => "word",
        App::Excel => "excel",
        App::PowerPoint => "powerpoint",
        App::Pages => "pages",
        App::Numbers => "numbers",
        App::Keynote => "keynote",
    }
}
