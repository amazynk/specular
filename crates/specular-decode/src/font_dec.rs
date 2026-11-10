//! Fonts: TrueType and OpenType (CFF, CFF2) files, TTC/OTC collections, WOFF and WOFF2,
//! PostScript Type 1 (`.pfb`, `.pfa`; rebuilt as OpenType CFF)
//! (`specular-font`), one face per session page.
//!
//! The session's raster is a small preview drawn on the CPU from the face's own outlines
//! ("Aa", or the first characters it has): thumbnails and the first frame show it. The
//! viewer draws the specimen itself on the GPU with the text stack from [`FontModel`].

use std::sync::Arc;

use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_font::{Container, FontFile, OutlineKind};

use crate::{
    DecodeError, DecodeRequest, Decoder, FontModel, Format, ImageInfo, PageLabel, Rect, Result,
    Session,
};

/// The preview's size (4:3).
const PREVIEW: (u32, u32) = (1024, 768);

/// Which containers a decoder takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    TrueType,
    OpenType,
    Collection,
    Woff,
    Woff2,
    Type1,
}

impl Kind {
    fn takes(self, c: Container) -> bool {
        matches!(
            (self, c),
            (Kind::TrueType, Container::TrueType)
                | (Kind::OpenType, Container::OpenTypeCff)
                | (Kind::Collection, Container::Collection)
                | (Kind::Woff, Container::Woff(_))
                | (Kind::Woff2, Container::Woff2(_))
                | (Kind::Type1, Container::Type1)
        )
    }
}

pub struct FontDecoder(pub Kind);

impl Decoder for FontDecoder {
    fn format(&self) -> Format {
        match self.0 {
            Kind::TrueType => Format::TrueType,
            Kind::OpenType => Format::OpenType,
            Kind::Collection => Format::FontCollection,
            Kind::Woff => Format::Woff,
            Kind::Woff2 => Format::Woff2,
            Kind::Type1 => Format::Type1Font,
        }
    }
    fn probe(&self, head: &[u8]) -> bool {
        specular_font::sniff(head).is_some_and(|c| self.0.takes(c))
    }
    fn extensions(&self) -> &'static [&'static str] {
        match self.0 {
            Kind::TrueType => &["ttf"],
            Kind::OpenType => &["otf"],
            Kind::Collection => &["ttc", "otc"],
            Kind::Woff => &["woff"],
            Kind::Woff2 => &["woff2"],
            Kind::Type1 => &["pfb", "pfa"],
        }
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        use specular_font::Error as E;
        let file =
            specular_font::open(source.bytes(), Default::default()).map_err(|e| match e {
                E::TooLarge => DecodeError::TooLarge,
                E::Type1 | E::UnknownTransform | E::WoffCollection => {
                    DecodeError::Unsupported(e.to_string())
                }
                e => DecodeError::Malformed(e.to_string()),
            })?;
        if file.faces.is_empty() {
            return Err(DecodeError::Malformed(
                "the collection holds no fonts".into(),
            ));
        }
        let shared = Arc::new(Shared {
            file: Arc::new(file),
            source,
            format: self.format(),
        });
        Ok(Arc::new(FontSession::new(shared, 0)))
    }
}

struct Shared {
    file: Arc<FontFile>,
    source: Arc<Source>,
    format: Format,
}

struct FontSession {
    shared: Arc<Shared>,
    page: u32,
    info: ImageInfo,
    model: FontModel,
}

/// "TrueType outlines, 2 axes, colour (COLR), WOFF2": the coding line.
fn coding(file: &FontFile, face: &specular_font::Face) -> String {
    let mut parts = vec![
        match face.outlines {
            OutlineKind::TrueType => "TrueType outlines",
            OutlineKind::Cff => "CFF outlines",
            OutlineKind::Cff2 => "CFF2 outlines",
            OutlineKind::BitmapOnly => "bitmaps only",
            OutlineKind::None => "no outlines",
        }
        .to_owned(),
    ];
    if !face.axes.is_empty() {
        parts.push(format!("variable, {} axes", face.axes.len()));
    }
    let c = &face.colour;
    if let Some((v, _)) = c.colr {
        parts.push(format!("colour (COLR v{v})"));
    }
    if !c.sbix.is_empty() {
        parts.push("colour bitmaps (sbix)".into());
    }
    if !c.cbdt.is_empty() {
        parts.push("colour bitmaps (CBDT)".into());
    }
    if c.svg {
        parts.push("SVG glyphs".into());
    }
    match file.container {
        Container::Woff(_) => parts.push("WOFF".into()),
        Container::Woff2(_) => parts.push("WOFF2".into()),
        Container::Type1 => parts.push("PostScript Type 1 (hints not used)".into()),
        _ => {}
    }
    parts.join(", ")
}

impl FontSession {
    fn new(shared: Arc<Shared>, page: u32) -> FontSession {
        let face = &shared.file.faces[page as usize];
        let info = ImageInfo {
            format: shared.format,
            width: PREVIEW.0,
            height: PREVIEW.1,
            orientation: Orientation::NORMAL,
            icc_profile: None,
            has_alpha: false,
            bit_depth: 8,
            progressive: false,
            coding: coding(&shared.file, face),
            embedded_preview: None,
            fast_regions: true,
            scales: &[1, 2, 4, 8],
        };
        let model = FontModel {
            file: shared.file.clone(),
            face: page as usize,
        };
        FontSession {
            shared,
            page,
            info,
            model,
        }
    }
}

impl Session for FontSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn alpha(&self) -> Option<crate::Alpha> {
        None
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        if cancel.is_cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let scale = req.scale.clamp(1, 8);
        let (sw, sh) = (
            self.info.width.div_ceil(scale),
            self.info.height.div_ceil(scale),
        );
        let (x, y, w, h) = match req.region {
            Some(Rect { x, y, w, h }) => {
                let x = x.min(sw);
                let y = y.min(sh);
                (x, y, w.min(sw - x), h.min(sh - y))
            }
            None => (0, 0, sw, sh),
        };
        if w == 0 || h == 0 {
            return Err(DecodeError::Unsupported("empty region".into()));
        }
        let face = &self.shared.file.faces[self.page as usize];
        let full = specular_font::preview(face, sw, sh);
        let data = if (x, y, w, h) == (0, 0, sw, sh) {
            full
        } else {
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for row in y..y + h {
                let start = (row as usize * sw as usize + x as usize) * 4;
                out.extend_from_slice(&full[start..start + w as usize * 4]);
            }
            out
        };
        Ok(Raster {
            width: w,
            height: h,
            origin: (x, y),
            scale,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        })
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = u64::from(req.scale.clamp(1, 8));
        // The whole preview at the scale, and the region cut from it.
        2 * 4 * u64::from(self.info.width).div_ceil(s) * u64::from(self.info.height).div_ceil(s)
    }
    fn memory_bytes(&self) -> usize {
        // The unwrapped SFNT data of every face, shared by the pages.
        let mut seen: Vec<*const Vec<u8>> = Vec::new();
        let mut n = 0;
        for f in &self.shared.file.faces {
            let d = f.sfnt();
            let p = Arc::as_ptr(&d);
            if !seen.contains(&p) {
                seen.push(p);
                n += d.len();
            }
        }
        n
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn page_count(&self) -> u32 {
        u32::try_from(self.shared.file.faces.len()).unwrap_or(u32::MAX)
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        if self.shared.file.faces.len() < 2 {
            return None;
        }
        let n = &self.shared.file.faces[self.page as usize].names;
        let name = [&n.full_name, &n.postscript, &n.family]
            .into_iter()
            .map(|s| s.trim())
            .find(|s| !s.is_empty())?;
        Some(PageLabel::Face { name: name.into() })
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if page as usize >= self.shared.file.faces.len() {
            return Err(DecodeError::Unsupported(format!("no face {page}")));
        }
        Ok(Arc::new(FontSession::new(self.shared.clone(), page)))
    }
    fn font(&self) -> Option<&FontModel> {
        Some(&self.model)
    }
}
