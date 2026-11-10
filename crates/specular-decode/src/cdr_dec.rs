//! CorelDRAW `.cdr` shown by its embedded previews (`specular-cdr`).
//!
//! The vector pages are not drawn: what is shown is the bitmap CorelDRAW stores for file
//! browsers, through the PNG or BMP decoder, and the coding says "embedded preview only" (the
//! info panel then leaves out the preview's bit depth). ZIP packages (X4 and later) have a
//! preview per page: each is a page. RIFF files (3 to X3) have one. CorelDRAW 1 and 2 files
//! and files without a preview are refused with the reason.

use crate::bitmap::{BitmapDecoder, Kind as BitmapKind};
use crate::png_dec::PngDecoder;
use crate::{Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, Result, Session};
use specular_cdr as sc;
use specular_core::{CancelToken, Raster, Source};
use std::sync::Arc;

pub struct CdrDecoder;

fn map_err(e: sc::Error) -> DecodeError {
    match e {
        sc::Error::NotCdr => DecodeError::Malformed("not a CorelDRAW file".into()),
        sc::Error::Unsupported(s) => DecodeError::Unsupported(s),
        sc::Error::Truncated(s) => DecodeError::Truncated(s),
        sc::Error::Malformed(s) => DecodeError::Malformed(s),
    }
}

impl Decoder for CdrDecoder {
    fn format(&self) -> Format {
        Format::Cdr
    }
    fn probe(&self, head: &[u8]) -> bool {
        sc::probe(head)
    }
    /// Also a ZIP package with CorelDRAW content (X4 and later).
    fn probe_file(&self, data: &[u8]) -> bool {
        sc::probe_file(data)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["cdr"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let cdr = sc::parse(source.bytes()).map_err(map_err)?;
        // Page previews when there are any, else the document thumbnail.
        let pages: Vec<sc::Preview> = {
            let p: Vec<_> = cdr
                .previews
                .iter()
                .filter(|p| matches!(p.kind, sc::PreviewKind::Page(_)))
                .cloned()
                .collect();
            if p.is_empty() {
                cdr.previews.clone()
            } else {
                p
            }
        };
        let file = Arc::new(CdrFile {
            source,
            what: cdr
                .version
                .map(|v| format!("{} drawing", sc::version_name(v)))
                .unwrap_or_else(|| "CorelDRAW drawing".into()),
            pages,
            xmp: cdr.xmp,
        });
        open_page(&file, 0)
    }
}

struct CdrFile {
    source: Arc<Source>,
    /// "CorelDRAW X7 drawing".
    what: String,
    pages: Vec<sc::Preview>,
    xmp: Option<Vec<u8>>,
}

fn open_page(file: &Arc<CdrFile>, page: u32) -> Result<Arc<dyn Session>> {
    let p = file
        .pages
        .get(page as usize)
        .ok_or_else(|| DecodeError::Malformed("no such page".into()))?;
    let data = file.source.bytes();
    let src = match &p.bytes {
        sc::Bytes::Range(r) => {
            Source::from_slice(file.source.path(), file.source.clone(), &data[r.clone()])
                .ok_or_else(|| DecodeError::Malformed("preview outside the file".into()))?
        }
        sc::Bytes::Owned(v) => Source::from_vec(file.source.path(), v.clone()),
    };
    let src = Arc::new(src);
    let (inner, kind) = match p.image {
        sc::ImageKind::Png => (PngDecoder.open(src)?, "PNG"),
        sc::ImageKind::Bmp => (BitmapDecoder(BitmapKind::Bmp).open(src)?, "BMP"),
    };
    let mut info = inner.info().clone();
    info.format = Format::Cdr;
    info.coding = format!(
        "{}, embedded preview only ({kind} {})",
        file.what, info.coding
    );
    if file.pages.len() > 1 {
        info.coding += &format!("; page {} of {}", page + 1, file.pages.len());
    }
    info.embedded_preview = None;
    Ok(Arc::new(CdrSession {
        file: file.clone(),
        page,
        info,
        inner,
    }))
}

/// A preview's PNG or BMP session under the drawing's name.
struct CdrSession {
    file: Arc<CdrFile>,
    page: u32,
    info: ImageInfo,
    inner: Arc<dyn Session>,
}

impl Session for CdrSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.file.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.inner.decode(req, cancel)
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        self.inner.decode_memory(req)
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
    fn colour_model(&self) -> Option<&'static str> {
        self.inner.colour_model()
    }
    fn alpha(&self) -> Option<Alpha> {
        self.inner.alpha()
    }
    fn embedded_xmp(&self) -> Option<Vec<u8>> {
        self.file.xmp.clone()
    }
    fn page_count(&self) -> u32 {
        self.file.pages.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_page(&self.file, page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×1 24-bit DIB in a version 9 RIFF's DISP chunk.
    fn riff_cdr() -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[0; 24]);
        dib.extend_from_slice(&[255, 0, 0, 0, 0, 255, 0, 0]); // blue, red (BGR), padded
        let mut disp = 8u32.to_le_bytes().to_vec();
        disp.extend_from_slice(&dib);
        let mut body = b"CDR9".to_vec();
        body.extend_from_slice(b"vrsn\x02\0\0\0\x84\x03");
        body.extend_from_slice(b"DISP");
        body.extend_from_slice(&(disp.len() as u32).to_le_bytes());
        body.extend_from_slice(&disp);
        let mut f = b"RIFF".to_vec();
        f.extend_from_slice(&(body.len() as u32).to_le_bytes());
        f.extend_from_slice(&body);
        f
    }

    #[test]
    fn riff_preview() {
        let d = riff_cdr();
        assert!(CdrDecoder.probe(&d));
        let s = CdrDecoder
            .open(Arc::new(Source::from_vec("a.cdr", d)))
            .unwrap();
        assert_eq!((s.info().width, s.info().height), (2, 1));
        assert_eq!(s.info().format, Format::Cdr);
        assert!(
            s.info()
                .coding
                .starts_with("CorelDRAW 9 drawing, embedded preview only (BMP"),
            "{}",
            s.info().coding
        );
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        let px = r.planes[0].data.clone();
        assert_eq!(r.layout.pixel_bytes(), px.len() / 2);
        assert_eq!(s.page_count(), 1);
    }

    #[test]
    fn refusals_are_plain() {
        let e = CdrDecoder
            .open(Arc::new(Source::from_vec(
                "a.cdr",
                b"WLe\0\x1c\0\0\0".to_vec(),
            )))
            .err()
            .unwrap();
        assert!(e.to_string().contains("CorelDRAW 1 or 2"), "{e}");
    }
}
