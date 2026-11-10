//! Apple icons (ICNS) through the in-tree `specular-icns` container reader.
//!
//! Every icon in the file is a page, largest first (as the ICO decoder shows its largest
//! entry); the session shows the first. PNG and JPEG 2000 payloads are opened zero-copy by
//! the PNG and JPEG 2000 decoders; the legacy run-length, `ARGB` and 1-bit icons are decoded
//! whole to RGBA and kept as levels. Other pages open with [`open_icns_page`].

use crate::whole::{self, Levels};
use crate::{
    Alpha, DecodeError, DecodeRequest, Decoder, Format, ImageInfo, PageLabel, Result, Session,
};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_icns as ic;
use std::sync::Arc;

pub struct IcnsDecoder;

fn map_err(e: ic::Error) -> DecodeError {
    match e {
        ic::Error::NotIcns => DecodeError::UnsupportedFormat,
        ic::Error::Truncated => DecodeError::Truncated("ICNS file".into()),
        ic::Error::NoIcons => DecodeError::Unsupported("ICNS file without a supported icon".into()),
        e => DecodeError::Malformed(e.to_string()),
    }
}

impl Decoder for IcnsDecoder {
    fn format(&self) -> Format {
        Format::Icns
    }
    fn probe(&self, head: &[u8]) -> bool {
        ic::detect(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["icns"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        open_icns_page(source, 0)
    }
}

/// The page labels of an ICNS file, in page order (largest first): the icon's size in
/// points, with `@2x` for Retina variants, e.g. "512×512 @2x".
pub fn icns_page_labels(data: &[u8]) -> Vec<String> {
    ic::parse(data).map_or_else(
        |_| Vec::new(),
        |f| f.icons.iter().map(ic::Icon::label).collect(),
    )
}

/// Open page `page` (0-based, largest icon first) of an ICNS file.
pub fn open_icns_page(source: Arc<Source>, page: usize) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let file = ic::parse(data).map_err(map_err)?;
    let pages = file.icons.len();
    let icon = file
        .icons
        .get(page)
        .cloned()
        .ok_or(DecodeError::Malformed("no such page".into()))?;
    let s = u32::from(icon.scale.max(1));
    let page_label = PageLabel::Icon {
        width: icon.width / s,
        height: icon.height / s,
        density: s,
    };
    let label = icon.label();
    let tail = if pages > 1 {
        format!(", {label}, page {} of {pages}", page + 1)
    } else {
        format!(", {label}")
    };
    let sub = || {
        Source::from_slice(source.path(), source.clone(), &data[icon.data.clone()])
            .map(Arc::new)
            .ok_or(DecodeError::Malformed("ICNS icon outside the file".into()))
    };
    let (inner, info) = match icon.kind {
        ic::Kind::Png | ic::Kind::Jp2 => {
            let s = if icon.kind == ic::Kind::Png {
                crate::png_dec::PngDecoder.open(sub()?)?
            } else {
                crate::jp2_dec::Jp2Decoder.open(sub()?)?
            };
            let mut i = s.info().clone();
            i.format = Format::Icns;
            let what = if icon.kind == ic::Kind::Png {
                "PNG"
            } else {
                "JPEG 2000"
            };
            i.coding = format!("{} ({what} {}){tail}", icon.name(), i.coding);
            (Inner::Session(s), i)
        }
        ic::Kind::Argb | ic::Kind::Rgb24 | ic::Kind::Mono => {
            let what = match icon.kind {
                ic::Kind::Argb => "ARGB run-length",
                ic::Kind::Rgb24 if icon.mask.is_some() => "24-bit with 8-bit mask",
                ic::Kind::Rgb24 => "24-bit",
                _ => "1-bit with mask",
            };
            let info = ImageInfo {
                format: Format::Icns,
                width: icon.width,
                height: icon.height,
                orientation: Orientation::NORMAL,
                icc_profile: None,
                has_alpha: icon.kind != ic::Kind::Rgb24 || icon.mask.is_some(),
                bit_depth: if icon.kind == ic::Kind::Mono { 1 } else { 8 },
                progressive: false,
                coding: format!("{} ({what}){tail}", icon.name()),
                embedded_preview: None,
                fast_regions: false,
                scales: &[1, 2, 4, 8],
            };
            (Inner::Own(icon), info)
        }
    };
    if file.truncated {
        specular_log::debug!(target: specular_log::DECODE, "{}: ICNS file ends early", source.path().display());
    }
    Ok(Arc::new(IcnsSession {
        source,
        info,
        page: page as u32,
        label: page_label,
        pages: pages as u32,
        inner,
        levels: Levels::new(),
    }))
}

enum Inner {
    /// A PNG or JPEG 2000 icon.
    Session(Arc<dyn Session>),
    /// Decoded here.
    Own(ic::Icon),
}

struct IcnsSession {
    source: Arc<Source>,
    info: ImageInfo,
    page: u32,
    label: PageLabel,
    pages: u32,
    inner: Inner,
    levels: Levels,
}

impl Session for IcnsSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        match &self.inner {
            Inner::Session(s) => s.colour_model(),
            Inner::Own(i) if i.kind == ic::Kind::Mono => Some("Greyscale"),
            Inner::Own(_) => Some("RGB"),
        }
    }
    fn alpha(&self) -> Option<Alpha> {
        match &self.inner {
            Inner::Session(s) => s.alpha(),
            Inner::Own(_) => self.info.has_alpha.then_some(Alpha::Straight),
        }
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        let icon = match &self.inner {
            Inner::Session(s) => return s.decode(req, cancel),
            Inner::Own(i) => i,
        };
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |_| 1,
            |_| {
                let img = ic::decode(self.source.bytes(), icon).map_err(map_err)?;
                Ok(whole::interleaved(
                    img.width,
                    img.height,
                    PixelLayout::Rgba,
                    img.pixels,
                    img.truncated,
                ))
            },
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        match &self.inner {
            Inner::Session(s) => s.decode_memory(req),
            Inner::Own(_) => {
                let (w, h) = (self.info.width, self.info.height);
                // The level plus the full image and its planar channels.
                whole::level_bytes(w, h, req.scale.max(1), 4) + whole::level_bytes(w, h, 1, 8)
            }
        }
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
            + match &self.inner {
                Inner::Session(s) => s.memory_bytes(),
                Inner::Own(_) => 0,
            }
    }
    /// Drops the cached levels (an embedded PNG / JPEG 2000 session's own).
    fn trim_memory(&self) -> usize {
        self.levels.clear()
            + match &self.inner {
                Inner::Session(s) => s.trim_memory(),
                Inner::Own(_) => 0,
            }
    }
    fn scaled_decode_cheap(&self) -> bool {
        match &self.inner {
            Inner::Session(s) => s.scaled_decode_cheap(),
            Inner::Own(_) => false,
        }
    }
    fn want_levels(&self, scales: &[u32]) {
        if let Inner::Session(s) = &self.inner {
            s.want_levels(scales);
        }
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn page_label(&self) -> Option<PageLabel> {
        Some(self.label.clone())
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        open_icns_page(self.source.clone(), page as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trim_check::assert_trims;

    /// One `is32` 16×16 icon: red run-length channel, constant green and blue, with a mask.
    fn tiny() -> Vec<u8> {
        let mut rgb = Vec::new();
        for v in [200u8, 10, 30] {
            // Two runs of 128 (0xfd = 0x80 + 125, repeated 125 + 3 times).
            rgb.extend_from_slice(&[0xfd, v, 0xfd, v]);
        }
        let mask = vec![128u8; 256];
        let mut body = Vec::new();
        for (cc, d) in [(b"is32", &rgb), (b"s8mk", &mask)] {
            body.extend_from_slice(cc);
            body.extend_from_slice(&((d.len() + 8) as u32).to_be_bytes());
            body.extend_from_slice(d);
        }
        let mut f = b"icns".to_vec();
        f.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
        f.extend_from_slice(&body);
        f
    }

    #[test]
    fn rle_icon_decodes_and_trims() {
        let s = IcnsDecoder
            .open(Arc::new(Source::from_vec("t.icns", tiny())))
            .unwrap();
        assert_eq!((s.info().width, s.info().height), (16, 16));
        assert_eq!(s.page_count(), 1);
        let r = s
            .decode(&DecodeRequest::full(1), &CancelToken::new())
            .unwrap();
        assert_eq!(&r.planes[0].data[..4], &[200, 10, 30, 128]);
        assert_trims(s.as_ref(), &DecodeRequest::full(1));
    }
}
