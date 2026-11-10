//! Design documents shown by an image they embed: Clip Studio Paint (`specular-clip`, the
//! merged canvas), Figma (`.fig` exports, the thumbnail), OpenDocument presentations,
//! spreadsheets and text documents (`Thumbnails/thumbnail.png`), binary Visio (`.vsd`, the
//! summary-information preview) and Sketch files whose pages cannot be drawn (the preview).
//!
//! The embedded image is opened through the registry by its own signature (PNG, JPEG, WMF,
//! EMF, BMP) and shown under the document's format; the coding says what the image is
//! ("embedded preview only", or "merged canvas" for Clip Studio, whose stored composite is
//! the full picture).

use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, DocumentFacts, Format, ImageInfo, Result, Session,
};
use specular_core::{CancelToken, Raster, Source};
use std::sync::Arc;

/// Opens `image` (named neutrally, so the registry goes by content) under `format`, with
/// `what` before the image's own coding: "{what} ({inner format} {inner coding})".
pub(crate) fn open_embedded(
    source: Arc<Source>,
    image: Source,
    format: Format,
    what: &str,
) -> Result<Arc<dyn Session>> {
    open_with(source, image, format, what, None)
}

/// [`open_embedded`] for an office document, with its facts ([`Session::document`]).
pub(crate) fn open_embedded_document(
    source: Arc<Source>,
    image: Source,
    format: Format,
    what: &str,
    facts: DocumentFacts,
) -> Result<Arc<dyn Session>> {
    open_with(source, image, format, what, Some(facts))
}

fn open_with(
    source: Arc<Source>,
    image: Source,
    format: Format,
    what: &str,
    document: Option<DocumentFacts>,
) -> Result<Arc<dyn Session>> {
    let inner = crate::svg::registry()
        .open(Arc::new(image))
        .map_err(|e| match e {
            DecodeError::Cancelled => e,
            e => DecodeError::Malformed(format!("{} preview: {e}", format.name())),
        })?;
    let mut info = inner.info().clone();
    info.coding = format!("{what} ({} {})", info.format.name(), info.coding);
    info.format = format;
    info.embedded_preview = None;
    Ok(Arc::new(EmbeddedSession {
        source,
        info,
        inner,
        document,
    }))
}

/// The embedded image's session under the document's name.
struct EmbeddedSession {
    source: Arc<Source>,
    info: ImageInfo,
    inner: Arc<dyn Session>,
    document: Option<DocumentFacts>,
}

impl Session for EmbeddedSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode_sampled(&self, long_side: u32, cancel: &CancelToken) -> Option<Result<Raster>> {
        self.inner.decode_sampled(long_side, cancel)
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn prepare_regions(&self, cancel: &CancelToken) -> Result<()> {
        self.inner.prepare_regions(cancel)
    }
    fn regions_ready(&self) -> bool {
        self.inner.regions_ready()
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
    }
    fn open_memory(&self, levels: &[u32]) -> u64 {
        self.inner.open_memory(levels)
    }
    fn memory_bytes(&self) -> usize {
        self.inner.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.inner.trim_memory()
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.inner.scaled_decode_cheap()
    }
    fn want_levels(&self, scales: &[u32]) {
        self.inner.want_levels(scales)
    }
    fn embedded_icc(&self) -> Option<&[u8]> {
        self.inner.embedded_icc()
    }
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<Alpha> {
        self.inner.alpha()
    }
    fn colour_space(&self) -> Option<&'static str> {
        self.inner.colour_space()
    }
    fn document(&self) -> Option<&DocumentFacts> {
        self.document.as_ref()
    }
}

// ----- Clip Studio Paint -----------------------------------------------------------------

pub struct ClipDecoder;

impl Decoder for ClipDecoder {
    fn format(&self) -> Format {
        Format::Clip
    }
    /// "CSFCHUNK".
    fn probe(&self, head: &[u8]) -> bool {
        specular_clip::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["clip"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use specular_clip::Error as E;
        let clip = specular_clip::parse(source.bytes()).map_err(|e| match e {
            E::NotClip => DecodeError::Malformed("not a Clip Studio Paint file".into()),
            E::Unsupported(s) => DecodeError::Unsupported(s),
            E::Malformed(s) => DecodeError::Malformed(s),
        })?;
        let what = format!("{}, merged canvas", clip.describe());
        // A neutral name: the registry must pick the decoder by content, not by `.clip`.
        let image = Source::from_vec("clip-preview", clip.preview.bytes);
        open_embedded(source, image, Format::Clip, &what)
    }
}
