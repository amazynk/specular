//! Bitmap fonts (`specular-font`): X11 PCF, BDF, Windows FNT and FON, Linux console PSF,
//! shown as a sheet of their glyphs. A `.fon` holding several fonts (sizes) shows each as a
//! page.
//!
//! PCF, PSF and FON have signatures; BDF its "STARTFONT" line; a bare `.fnt` only a
//! plausible version 2 or 3 header, so it is likely, not certain.

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_font::{BitmapFont, BitmapKind};

use crate::whole::{self, Levels};
use crate::{
    Confidence, DecodeError, DecodeRequest, Decoder, Format, HeaderFacts, ImageInfo, PageLabel,
    Result, Session,
};

/// Glyphs drawn on a sheet at most (a Unicode font's first ones).
const MAX_SHOWN: usize = 16_384;

pub struct BitmapFontDecoder;

fn map_err(e: specular_font::Error) -> DecodeError {
    match e {
        specular_font::Error::Unsupported(s) => DecodeError::Unsupported(s),
        e => DecodeError::Malformed(e.to_string()),
    }
}

impl Decoder for BitmapFontDecoder {
    fn format(&self) -> Format {
        Format::BitmapFont
    }
    fn probe(&self, head: &[u8]) -> bool {
        matches!(
            specular_font::sniff_bitmap(head),
            Some(BitmapKind::Pcf | BitmapKind::Bdf | BitmapKind::Psf1 | BitmapKind::Psf2)
        )
    }
    /// A `.fon` needs its NE resource table; a bare FNT only fits a header.
    fn probe_file(&self, data: &[u8]) -> bool {
        specular_font::sniff_bitmap(data).is_some()
    }
    fn confidence(&self, data: &[u8]) -> Option<Confidence> {
        match specular_font::sniff_bitmap(data)? {
            BitmapKind::Fnt => Some(Confidence::Likely),
            _ => Some(Confidence::Certain),
        }
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["pcf", "bdf", "fon", "fnt", "psf", "psfu"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let (kind, fonts) = specular_font::open_bitmap(source.bytes()).map_err(map_err)?;
        let shared = Arc::new(Shared {
            kind,
            fonts,
            source,
        });
        Ok(Arc::new(BitmapFontSession::new(shared, 0)))
    }
}

struct Shared {
    kind: BitmapKind,
    fonts: Vec<BitmapFont>,
    source: Arc<Source>,
}

struct BitmapFontSession {
    shared: Arc<Shared>,
    page: u32,
    info: ImageInfo,
    header: HeaderFacts,
    levels: Levels,
}

/// The font's facts for the Info panel.
fn facts(kind: BitmapKind, f: &BitmapFont) -> Vec<(String, String)> {
    let mut v = vec![("Format".to_owned(), kind.name().to_owned())];
    if !f.family.is_empty() {
        v.push(("Family".into(), f.family.clone()));
    }
    if !f.name.is_empty() && f.name != f.family {
        v.push(("Name".into(), f.name.clone()));
    }
    if f.pixel_size > 0 {
        v.push(("Pixel size".into(), f.pixel_size.to_string()));
    }
    v.push(("Glyphs".into(), f.glyphs.len().to_string()));
    if f.glyphs.len() > MAX_SHOWN {
        v.push(("Glyphs shown".into(), MAX_SHOWN.to_string()));
    }
    if let (Some(a), Some(b)) = (f.glyphs.first(), f.glyphs.last()) {
        v.push(("Characters".into(), format!("{:#X}–{:#X}", a.code, b.code)));
    }
    v.push(("Encoding".into(), f.encoding.clone()));
    if f.ascent != 0 || f.descent != 0 {
        v.push((
            "Ascent / descent".into(),
            format!("{} / {}", f.ascent, f.descent),
        ));
    }
    v.extend(f.facts.iter().cloned());
    v
}

impl BitmapFontSession {
    fn new(shared: Arc<Shared>, page: u32) -> BitmapFontSession {
        let f = &shared.fonts[page as usize];
        let (w, h) = specular_font::sheet_size(f, MAX_SHOWN);
        let info = ImageInfo {
            format: Format::BitmapFont,
            width: w,
            height: h,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 1,
            progressive: false,
            coding: format!(
                "{}, {} glyphs{}",
                shared.kind.name(),
                f.glyphs.len(),
                if f.pixel_size > 0 {
                    format!(", {} px", f.pixel_size)
                } else {
                    String::new()
                }
            ),
            embedded_preview: None,
            fast_regions: false,
            scales: &[1, 2, 4, 8],
        };
        let header = HeaderFacts {
            summary: Vec::new(),
            fields: facts(shared.kind, f),
        };
        BitmapFontSession {
            shared,
            page,
            info,
            header,
            levels: Levels::new(),
        }
    }
}

impl Session for BitmapFontSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("Grey")
    }
    fn header(&self) -> Option<&HeaderFacts> {
        Some(&self.header)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let f = &self.shared.fonts[self.page as usize];
                let (w, h, px) = specular_font::sheet(f, MAX_SHOWN);
                Ok(whole::interleaved(w, h, PixelLayout::Gray, px, false))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let (w, h) = (self.info.width, self.info.height);
        whole::level_bytes(w, h, req.scale.max(1), 1) + whole::level_bytes(w, h, 1, 1)
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn page_count(&self) -> u32 {
        self.shared.fonts.len() as u32
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        (self.shared.fonts.len() > 1).then_some(PageLabel::Image {
            width: self.info.width,
            height: self.info.height,
            bits: Some(1),
        })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.fonts.len() {
            return Err(DecodeError::Malformed("no such font".into()));
        }
        Ok(Arc::new(BitmapFontSession::new(self.shared.clone(), page)))
    }
}
