//! Office and design drawings drawn as vector pages through `specular-drawpage`: Visio
//! (`specular-visio`: VSDX packages drawn; binary VSD by its embedded preview), WordPerfect
//! Graphics (`specular-wpg`), OpenDocument Graphics (`specular-odf`) and Sketch
//! (`specular-sketch`). Pages render like XPS and CGM pages ([`crate::docpage_dec`]): every
//! scale and region straight from the display list.

use crate::docpage_dec::{DrawDoc, doc_xmp, open_drawing};
use crate::embedded_dec::open_embedded;
use crate::{DecodeError, Decoder, Format, Result, Session};
use specular_core::Source;
use std::sync::Arc;

/// Embedded images of drawings, through the registry.
fn image_decoder() -> specular_drawpage::ImageDecoder {
    Arc::new(|d: &[u8]| crate::svg::decode_rgba(d))
}

/// A BMP file around a packed DIB (header, colour table, pixels).
pub(crate) fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(dib.get(o..o + 4)?.try_into().ok()?));
    let header = u32_at(0)? as usize;
    if !(12..=124).contains(&header) || dib.len() < header {
        return None;
    }
    let (bits, compression, used) = if header == 12 {
        (u16::from_le_bytes(dib.get(10..12)?.try_into().ok()?), 0, 0)
    } else {
        (
            u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?),
            u32_at(16)?,
            u32_at(32)?,
        )
    };
    let entry = if header == 12 { 3 } else { 4 };
    let colours = if used > 0 {
        used as usize
    } else if bits <= 8 {
        1usize << bits
    } else {
        0
    };
    // BI_BITFIELDS with a 40-byte header: three masks follow it.
    let masks = if header == 40 && compression == 3 {
        12
    } else {
        0
    };
    let offset = 14 + header + masks + colours.min(256) * entry;
    let size = 14 + dib.len();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&u32::try_from(size).ok()?.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&u32::try_from(offset).ok()?.to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

// ----- Visio -------------------------------------------------------------------------------

pub struct VisioDecoder;

fn visio_error(e: specular_visio::Error) -> DecodeError {
    use specular_visio::Error as E;
    match e {
        E::NotVisio => DecodeError::Malformed(e.to_string()),
        E::Unsupported(s) => DecodeError::Unsupported(s),
        e @ E::Malformed(_) => DecodeError::Malformed(e.to_string()),
    }
}

impl Decoder for VisioDecoder {
    fn format(&self) -> Format {
        Format::Visio
    }
    /// A ZIP or compound file like any other from its first bytes: the directory decides.
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_visio::probe_vsdx(data) || specular_visio::probe_vsd_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[
            "vsdx", "vsdm", "vstx", "vstm", "vssx", "vssm", "vsd", "vss", "vdx",
        ]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        if data.starts_with(b"PK") {
            let mut doc =
                specular_visio::open_vsdx(data, Some(image_decoder())).map_err(visio_error)?;
            let f = doc.facts.clone();
            let xmp = doc_xmp(
                f.title.clone(),
                f.author.clone(),
                f.subject.clone().or(f.description.clone()),
                f.keywords.clone(),
            );
            // The description reads only the facts.
            let pages = std::mem::take(&mut doc.pages);
            let draw = DrawDoc {
                pages,
                describe: Box::new(move |p| doc.describe(p)),
                xmp,
            };
            return open_drawing(draw, source, Format::Visio);
        }
        if specular_visio::probe_vsd(data) {
            use specular_visio::PreviewKind as K;
            let p = specular_visio::vsd_preview(data).map_err(visio_error)?;
            let bytes = match p.kind {
                K::Dib => dib_to_bmp(&p.bytes)
                    .ok_or_else(|| DecodeError::Malformed("damaged Visio preview".into()))?,
                _ => p.bytes,
            };
            let image = Source::from_vec("visio-preview", bytes);
            return open_embedded(
                source,
                image,
                Format::Visio,
                "Visio drawing (VSD), embedded preview only",
            );
        }
        let vsd = source
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("vsd") || e.eq_ignore_ascii_case("vss"));
        if vsd && !data.starts_with(b"\xd0\xcf\x11\xe0") {
            return Err(DecodeError::Unsupported(
                "binary Visio drawing in an old or unknown container (not a compound file): \
                 not supported"
                    .into(),
            ));
        }
        if data.trim_ascii_start().starts_with(b"<") {
            return Err(DecodeError::Unsupported(
                "Visio XML drawing (VDX, Visio 2003–2010): not supported; save it as VSDX".into(),
            ));
        }
        Err(DecodeError::Malformed("not a Visio drawing".into()))
    }
}

// ----- WordPerfect Graphics ----------------------------------------------------------------

pub struct WpgDecoder;

impl Decoder for WpgDecoder {
    fn format(&self) -> Format {
        Format::Wpg
    }
    /// FF "WPC", product 1, file type 0x16.
    fn probe(&self, head: &[u8]) -> bool {
        specular_wpg::probe(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["wpg"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use specular_wpg::Error as E;
        let mut doc = specular_wpg::open(source.bytes()).map_err(|e| match e {
            E::Encrypted | E::Unsupported(_) => DecodeError::Unsupported(e.to_string()),
            e => DecodeError::Malformed(e.to_string()),
        })?;
        let pages = std::mem::take(&mut doc.pages);
        let draw = DrawDoc {
            pages,
            describe: Box::new(move |p| doc.describe(p)),
            xmp: None,
        };
        open_drawing(draw, source, Format::Wpg)
    }
}

// ----- OpenDocument ------------------------------------------------------------------------

pub struct OdfDecoder;

fn odf_error(e: specular_odf::Error) -> DecodeError {
    use specular_odf::Error as E;
    match e {
        E::Encrypted | E::Unsupported(_) => DecodeError::Unsupported(e.to_string()),
        e => DecodeError::Malformed(e.to_string()),
    }
}

impl Decoder for OdfDecoder {
    fn format(&self) -> Format {
        Format::OpenDocument
    }
    /// A ZIP like any other from its first bytes: its `mimetype` member decides.
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_odf::probe_file(data).is_some()
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[
            "odg", "otg", "fodg", "odp", "otp", "fodp", "ods", "ots", "odt", "ott",
        ]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use specular_odf::Kind;
        let data = source.bytes();
        let kind = specular_odf::probe_file(data)
            .ok_or_else(|| DecodeError::Malformed("not an OpenDocument file".into()))?;
        // Drawings and presentations are drawn page by page.
        if matches!(kind, Kind::Graphics | Kind::Presentation) {
            let mut doc =
                specular_odf::open_graphics(data, Some(image_decoder())).map_err(odf_error)?;
            let pages = std::mem::take(&mut doc.pages);
            let f = &doc.facts;
            let xmp = doc_xmp(
                f.title.clone(),
                f.author.clone(),
                f.subject.clone(),
                f.keywords.clone(),
            );
            let draw = DrawDoc {
                pages,
                describe: Box::new(move |p| doc.describe(p)),
                xmp,
            };
            return open_drawing(draw, source, Format::OpenDocument);
        }
        // Text documents, spreadsheets and the rest: the thumbnail the office suite stored,
        // with the properties of `meta.xml`; a package saved without a thumbnail opens with
        // its properties and "no preview inside".
        let zip = data
            .starts_with(b"PK")
            .then(|| specular_ora::Zip::parse(data).ok())
            .flatten();
        let props = zip
            .as_ref()
            .and_then(|z| {
                let e = z.find("meta.xml")?;
                z.read(data, e, 16 << 20).ok()
            })
            .map(|m| specular_office::odf_properties(&m))
            .unwrap_or_default();
        let image = match specular_odf::thumbnail(data) {
            Ok(specular_odf::Thumbnail::Range(r)) => {
                let part = data
                    .get(r)
                    .ok_or_else(|| DecodeError::Malformed("thumbnail outside the file".into()))?;
                Some(
                    Source::from_slice("odf-thumbnail", source.clone(), part).ok_or_else(|| {
                        DecodeError::Malformed("thumbnail outside the file".into())
                    })?,
                )
            }
            Ok(specular_odf::Thumbnail::Owned(v)) => Some(Source::from_vec("odf-thumbnail", v)),
            Err(specular_odf::Error::Unsupported(_)) if zip.is_some() => None,
            Err(e) => return Err(odf_error(e)),
        };
        let facts = crate::DocumentFacts {
            kind: match kind {
                Kind::Text => "odf-text",
                Kind::Spreadsheet => "odf-spreadsheet",
                Kind::Presentation => "odf-presentation",
                Kind::Graphics => "odf-graphics",
                Kind::Other(_) => "odf-other",
            },
            legacy: false,
            template: false,
            macros: false,
            preview: image.as_ref().map(|_| "PNG"),
            props,
            items: None,
        };
        let what = format!("OpenDocument {}", kind.noun());
        crate::office_dec::open_document(source, image, Format::OpenDocument, &what, facts)
    }
}

// ----- draw.io and Excalidraw --------------------------------------------------------------

/// draw.io (`.drawio`, `.dio`) and Excalidraw (`.excalidraw`) diagrams, drawn cleanly.
pub struct DiagramDecoder(pub Format);

impl Decoder for DiagramDecoder {
    fn format(&self) -> Format {
        self.0
    }
    fn probe(&self, head: &[u8]) -> bool {
        match self.0 {
            Format::Drawio => specular_diagram::is_drawio(head),
            _ => specular_diagram::is_excalidraw(head),
        }
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        self.probe(&data[..data.len().min(4096)])
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Format::Drawio => &["drawio", "dio"],
            _ => &["excalidraw"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let d = match self.0 {
            Format::Drawio => specular_diagram::open_drawio(data, Some(image_decoder())),
            _ => specular_diagram::open_excalidraw(data, Some(image_decoder())),
        }
        .map_err(|e| match e {
            specular_diagram::Error::Unsupported(s) => DecodeError::Unsupported(s),
            e => DecodeError::Malformed(e.to_string()),
        })?;
        let what = if self.0 == Format::Drawio {
            "draw.io diagram"
        } else {
            "Excalidraw scene"
        };
        let summary = format!(
            "{what}: {} shapes, {} connectors{}{}",
            d.shapes,
            d.connectors,
            if d.missing_pictures > 0 {
                format!(", {} pictures not shown", d.missing_pictures)
            } else {
                String::new()
            },
            if d.approximated > 0 {
                format!(", {} shapes drawn as boxes", d.approximated)
            } else {
                String::new()
            }
        );
        let names: Vec<String> = d.pages.iter().map(|p| p.name.clone()).collect();
        let draw = DrawDoc {
            pages: d.pages,
            describe: Box::new(move |p| match names.get(p) {
                Some(n) if names.len() > 1 => format!("{summary}; page “{n}”"),
                _ => summary.clone(),
            }),
            xmp: None,
        };
        open_drawing(draw, source, self.0)
    }
}

// ----- Flash ------------------------------------------------------------------------------

/// Flash movies (`.swf`): the first frame drawn; no ActionScript is ever run.
pub struct SwfDecoder;

impl Decoder for SwfDecoder {
    fn format(&self) -> Format {
        Format::Swf
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_swf::is_swf(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["swf"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let images = image_decoder();
        let m = specular_swf::open(source.bytes(), Some(&images)).map_err(|e| match e {
            specular_swf::Error::Unsupported(s) => DecodeError::Unsupported(s),
            e => DecodeError::Malformed(e.to_string()),
        })?;
        let f = m.facts;
        let mut parts = vec![format!(
            "Flash movie (SWF {}, {} compression), first of {} frames at {} fps",
            f.version, f.compression, f.frame_count, f.frame_rate
        )];
        let mut held = Vec::new();
        for (n, what) in [
            (f.shapes, "shapes"),
            (f.bitmaps, "bitmaps"),
            (f.sprites, "sprites"),
            (f.texts, "texts"),
            (f.buttons, "buttons (not drawn)"),
        ] {
            if n > 0 {
                held.push(format!("{n} {what}"));
            }
        }
        if !held.is_empty() {
            parts.push(held.join(", "));
        }
        if !f.videos.is_empty() {
            parts.push(format!("video not played ({})", f.videos.join(", ")));
        }
        if !f.sounds.is_empty() {
            parts.push(format!("sound not played ({})", f.sounds.join(", ")));
        }
        if f.actionscript {
            parts.push("ActionScript never run".into());
        }
        if f.missing_pictures > 0 {
            parts.push(format!("{} bitmaps not shown", f.missing_pictures));
        }
        let summary = parts.join("; ");
        let draw = DrawDoc {
            pages: vec![m.page],
            describe: Box::new(move |_| summary.clone()),
            xmp: None,
        };
        open_drawing(draw, source, Format::Swf)
    }
}

// ----- Sketch and Figma --------------------------------------------------------------------

fn sketch_error(e: specular_sketch::Error) -> DecodeError {
    use specular_sketch::Error as E;
    match e {
        E::Unsupported(s) => DecodeError::Unsupported(s),
        e => DecodeError::Malformed(e.to_string()),
    }
}

/// An embedded image of `source` as a source named neutrally.
fn bytes_source(source: &Arc<Source>, b: specular_sketch::Bytes, name: &str) -> Result<Source> {
    match b {
        specular_sketch::Bytes::Range(r) => {
            let part = source
                .bytes()
                .get(r)
                .ok_or_else(|| DecodeError::Malformed("preview outside the file".into()))?;
            Source::from_slice(name, source.clone(), part)
                .ok_or_else(|| DecodeError::Malformed("preview outside the file".into()))
        }
        specular_sketch::Bytes::Owned(v) => Ok(Source::from_vec(name, v)),
    }
}

pub struct SketchDecoder;

impl Decoder for SketchDecoder {
    fn format(&self) -> Format {
        Format::Sketch
    }
    /// A ZIP like any other from its first bytes: the directory decides.
    fn probe(&self, _head: &[u8]) -> bool {
        false
    }
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_sketch::probe_sketch_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["sketch"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        match specular_sketch::open_sketch(data, Some(image_decoder())) {
            Ok(mut doc) => {
                let pages = std::mem::take(&mut doc.pages);
                let facts = doc.facts;
                let draw = DrawDoc {
                    pages,
                    describe: Box::new(move |p| facts.describe(p)),
                    xmp: None,
                };
                open_drawing(draw, source, Format::Sketch)
            }
            // Nothing drawable (no pages, empty pages): the preview Sketch stored, if any.
            Err(specular_sketch::Error::Unsupported(why))
                if !data.starts_with(b"SQLite format 3") =>
            {
                let Ok(b) = specular_sketch::sketch_preview(data) else {
                    return Err(DecodeError::Unsupported(why));
                };
                let image = bytes_source(&source, b, "sketch-preview")?;
                open_embedded(
                    source,
                    image,
                    Format::Sketch,
                    "Sketch document, embedded preview only",
                )
            }
            Err(e) => Err(sketch_error(e)),
        }
    }
}

pub struct FigmaDecoder;

impl Decoder for FigmaDecoder {
    fn format(&self) -> Format {
        Format::Figma
    }
    /// "fig-kiwi" (a bare canvas, refused with the reason).
    fn probe(&self, head: &[u8]) -> bool {
        specular_sketch::probe_figma(head)
    }
    /// Also a ZIP local copy holding `canvas.fig`.
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_sketch::probe_figma_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["fig"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let data = source.bytes();
        let b = specular_sketch::figma_thumbnail(data).map_err(|e| match e {
            specular_sketch::Error::NotFigma => DecodeError::Malformed("not a Figma file".into()),
            e => sketch_error(e),
        })?;
        let facts = specular_sketch::figma_facts(data);
        let what = match facts.file_name.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() => format!("Figma file \"{n}\", embedded preview only"),
            _ => "Figma file, embedded preview only".to_string(),
        };
        let image = bytes_source(&source, b, "figma-thumbnail")?;
        open_embedded(source, image, Format::Figma, &what)
    }
}
