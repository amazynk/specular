//! JPM files (JPEG 2000 Part 6, ITU-T T.805): compound pages drawn by
//! `specular_jp2::decode_jpm_page`, reached through the JPEG 2000 decoder.
//!
//! * **Pages.** Every page box is a page of the file, in file order.
//! * **Scales.** 1/2/4/8 draw the page at that size (`reduce`): JPEG 2000 objects decode at the
//!   coarsest resolution that still covers their share of the page.
//! * **Other coders.** Objects coded with something other than JPEG 2000 go through the
//!   foreign-coder hook to Specular's own decoders: JBIG2 (`specular-jbig2`, embedded streams
//!   or whole files), fax T.4 / T.6 (MH, MR, MMR: the CCITT decoder of `specular-tiff`, fed a
//!   one-strip TIFF made here), JPEG (`specular-jpeg`), JBIG (`specular-jbig`) and JPEG-LS
//!   (`specular-jpegls`). One they cannot decode refuses the page with the coder's name.

use crate::whole::{self, Levels};
use crate::{Alpha, DecodeError, DecodeRequest, Format, ImageInfo, Result, Session};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, Source};
use specular_jp2::{ForeignObject, Image, JpmCoder, JpmOptions, PixelFormat, Pixels};
use std::sync::Arc;

/// Opens page `page` (0-based) of a JPM file.
pub(crate) fn open_page(source: Arc<Source>, page: u32) -> Result<Arc<dyn Session>> {
    let data = source.bytes();
    let info = specular_jp2::read_jpm_info(data).map_err(crate::jp2_dec::map_err)?;
    let pages = info.pages.len() as u32;
    let p = *info
        .pages
        .get(page as usize)
        .ok_or_else(|| DecodeError::Malformed("no such page".into()))?;
    // Orientations 2 and 4 turn the page a quarter.
    let (width, height) = if matches!(p.orientation, 2 | 4) {
        (p.height, p.width)
    } else {
        (p.width, p.height)
    };
    let mut coding = format!(
        "JPM compound page, {} layout object{}",
        p.layout_objects,
        if p.layout_objects == 1 { "" } else { "s" }
    );
    if pages > 1 {
        coding += &format!(", page {} of {pages}", page + 1);
    }
    let transparent = page_colour(data, page as usize) == Some(0);
    let info = ImageInfo {
        format: Format::Jpeg2000,
        width,
        height,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: transparent,
        bit_depth: 8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: false,
        scales: &[1, 2, 4, 8],
    };
    Ok(Arc::new(JpmSession {
        source,
        info,
        page,
        pages,
        levels: Levels::new(),
    }))
}

/// The page colour field (T.805 Table B.11: 0 transparent, 1 white, 2 black, 255 a base
/// colour) of page `page`'s header, read from the top-level page boxes.
fn page_colour(data: &[u8], page: usize) -> Option<u16> {
    let pbox = top_boxes(data.get(12..)?)
        .filter(|(t, _)| t == b"page")
        .nth(page)?
        .1;
    let (_, ph) = top_boxes(pbox).find(|(t, _)| t == b"phdr")?;
    Some(u16::from_be_bytes(ph.get(12..14)?.try_into().ok()?))
}

/// The `(type, contents)` boxes of a box list (plain and extended lengths; a length of 0 runs
/// to the end).
fn top_boxes(mut d: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    std::iter::from_fn(move || {
        if d.len() < 8 {
            return None;
        }
        let len = u32::from_be_bytes(d[..4].try_into().ok()?) as u64;
        let t: [u8; 4] = d[4..8].try_into().ok()?;
        let (head, len) = match len {
            0 => (8, d.len() as u64),
            1 => (16, u64::from_be_bytes(d.get(8..16)?.try_into().ok()?)),
            l => (8, l),
        };
        if len < head || len > d.len() as u64 {
            return None;
        }
        let (b, rest) = d.split_at(len as usize);
        d = rest;
        Some((t, &b[head as usize..]))
    })
}

struct JpmSession {
    source: Arc<Source>,
    info: ImageInfo,
    page: u32,
    pages: u32,
    levels: Levels,
}

impl JpmSession {
    fn draw(&self, scale: u32, cancel: &CancelToken) -> Result<Raster> {
        let reduce = scale.max(1).trailing_zeros().min(31) as u8;
        let img = specular_jp2::decode_jpm_page(
            self.source.bytes(),
            self.page as usize,
            &JpmOptions {
                reduce,
                cancel: Some(cancel.flag()),
                foreign: Some(&foreign),
                ..Default::default()
            },
        )
        .map_err(crate::jp2_dec::map_err)?;
        let Pixels::U8(px) = img.pixels else {
            return Err(DecodeError::Malformed(
                "JPM page of an unexpected depth".into(),
            ));
        };
        Ok(whole::interleaved(
            img.width,
            img.height,
            PixelLayout::Rgba,
            px,
            img.truncated,
        ))
    }
}

impl Session for JpmSession {
    fn info(&self) -> &ImageInfo {
        &self.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.source
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some("RGB")
    }
    fn alpha(&self) -> Option<Alpha> {
        self.info.has_alpha.then_some(Alpha::Straight)
    }
    fn decode_preview(&self) -> Option<Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> Result<Raster> {
        self.levels.decode(
            req,
            (self.info.width, self.info.height),
            |s| s,
            |s| self.draw(s, cancel),
            cancel,
        )
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        // The page, its rotation and the objects drawn on it at about the page's size.
        whole::level_bytes(self.info.width, self.info.height, req.scale.max(1), 4) * 3
    }
    fn memory_bytes(&self) -> usize {
        self.levels.memory_bytes()
    }
    fn trim_memory(&self) -> usize {
        self.levels.clear()
    }
    fn scaled_decode_cheap(&self) -> bool {
        true
    }
    fn page_count(&self) -> u32 {
        self.pages
    }
    fn page_index(&self) -> u32 {
        self.page
    }
    fn open_page(&self, page: u32) -> Result<Arc<dyn Session>> {
        if self.pages < 2 {
            return Err(DecodeError::Unsupported("no pages".into()));
        }
        if page >= self.pages {
            return Err(DecodeError::Malformed("no such page".into()));
        }
        open_page(self.source.clone(), page)
    }
}

// ---------------------------------------------------------------------------
// Objects of the other coders
// ---------------------------------------------------------------------------

/// The foreign-coder hook: `o` decoded by Specular's own decoder for its coder, as the hook
/// contract asks (8-bit grey or RGBA of the object's size; 1-bit data 255 where a bit is set,
/// that is black).
fn foreign(o: &ForeignObject<'_>) -> Option<Image> {
    let r = match o.coder {
        JpmCoder::Mh | JpmCoder::Mr | JpmCoder::Mmr => fax(o),
        JpmCoder::Jbig2 => jbig2(o),
        JpmCoder::Jbig => jbig(o),
        JpmCoder::Jpeg => jpeg(o),
        JpmCoder::JpegLs => jpeg_ls(o),
        _ => None,
    };
    if r.is_none() {
        specular_log::debug!(target: specular_log::DECODE, "jpm: {:?} object {}×{} not decoded", o.coder, o.width, o.height);
    }
    r
}

fn gray(width: u32, height: u32, px: Vec<u8>, truncated: bool) -> Image {
    Image {
        width,
        height,
        origin: (0, 0),
        format: PixelFormat::Gray8,
        pixels: Pixels::U8(px),
        truncated,
        reduce: 0,
    }
}

/// A bi-level decode that gives white as 255 and black as 0, turned round to the hook's 1-bit
/// convention (a set bit, black, as 255).
fn bilevel(width: u32, height: u32, px: Vec<u8>, truncated: bool) -> Image {
    gray(
        width,
        height,
        px.into_iter().map(|v| 255 - v).collect(),
        truncated,
    )
}

/// JBIG2: an embedded stream (segments only, as T.805 stores it) is given a sequential file
/// header of one page; a whole JBIG2 file is read as it is.
fn jbig2(o: &ForeignObject<'_>) -> Option<Image> {
    let file;
    let data = if o.data.starts_with(&specular_jbig2::MAGIC) {
        o.data
    } else {
        let mut f = specular_jbig2::MAGIC.to_vec();
        f.push(0x01); // sequential, page count known
        f.extend_from_slice(&1u32.to_be_bytes());
        f.extend_from_slice(o.data);
        file = f;
        &file
    };
    let img = specular_jbig2::decode(data, 0).ok()?;
    let specular_jbig2::Pixels::Gray8(px) = img.pixels else {
        return None;
    };
    Some(bilevel(img.width, img.height, px, img.truncated))
}

/// JBIG (T.82): a bi-level image entity.
fn jbig(o: &ForeignObject<'_>) -> Option<Image> {
    let img = specular_jbig::decode(o.data, 0).ok()?;
    let specular_jbig::Pixels::Gray8(px) = img.pixels else {
        return None;
    };
    if o.bit_depth == 1 {
        Some(bilevel(img.width, img.height, px, img.truncated))
    } else {
        // Grey levels: 0 black, as the samples read.
        Some(gray(img.width, img.height, px, img.truncated))
    }
}

/// Fax T.4 one- or two-dimensional (MH, MR) or T.6 (MMR) data, decoded by `specular-tiff`'s
/// CCITT decoder from a one-strip TIFF made around it. BlackIsZero makes a set bit (black in
/// fax coding) come out as 255, the hook's convention.
fn fax(o: &ForeignObject<'_>) -> Option<Image> {
    if o.components != 1 || o.width == 0 || o.height == 0 {
        return None;
    }
    let (compression, t4) = match o.coder {
        JpmCoder::Mh => (3u16, Some(0u32)),
        JpmCoder::Mr => (3, Some(1)),
        _ => (4, None),
    };
    let tiff = fax_tiff(o.width, o.height, compression, t4, o.data);
    let t = specular_tiff::Tiff::open(&tiff).ok()?;
    let d = t
        .decode(
            &tiff,
            0,
            &specular_tiff::DecodeOptions {
                parallel: false,
                ..Default::default()
            },
        )
        .ok()?;
    if d.format != specular_tiff::PixelFormat::Gray8 || d.width != o.width {
        return None;
    }
    let row = d.width as usize;
    let px: Vec<u8> = d
        .data
        .chunks(d.stride)
        .take(d.height as usize)
        .flat_map(|r| &r[..row])
        .copied()
        .collect();
    Some(gray(d.width, d.height, px, d.truncated))
}

/// A little-endian TIFF of one 1-bit BlackIsZero strip holding `data`.
fn fax_tiff(w: u32, h: u32, compression: u16, t4: Option<u32>, data: &[u8]) -> Vec<u8> {
    // (tag, type, value): type 3 SHORT, 4 LONG; every value fits the entry.
    let mut tags: Vec<(u16, u16, u32)> = vec![
        (256, 4, w),
        (257, 4, h),
        (258, 3, 1),
        (259, 3, u32::from(compression)),
        (262, 3, 1),
        (273, 4, 0), // strip offset, set below
        (277, 3, 1),
        (278, 4, h),
        (279, 4, data.len() as u32),
    ];
    if let Some(v) = t4 {
        tags.push((292, 4, v));
    }
    let ifd_len = 2 + tags.len() * 12 + 4;
    let strip = (8 + ifd_len) as u32;
    let mut f = Vec::with_capacity(strip as usize + data.len());
    f.extend_from_slice(b"II*\0");
    f.extend_from_slice(&8u32.to_le_bytes());
    f.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for (tag, ty, v) in tags {
        let v = if tag == 273 { strip } else { v };
        f.extend_from_slice(&tag.to_le_bytes());
        f.extend_from_slice(&ty.to_le_bytes());
        f.extend_from_slice(&1u32.to_le_bytes());
        if ty == 3 {
            f.extend_from_slice(&(v as u16).to_le_bytes());
            f.extend_from_slice(&[0, 0]);
        } else {
            f.extend_from_slice(&v.to_le_bytes());
        }
    }
    f.extend_from_slice(&0u32.to_le_bytes());
    f.extend_from_slice(data);
    f
}

/// JPEG: RGBA (grey replicated, CMYK through the colour-managed hook).
fn jpeg(o: &ForeignObject<'_>) -> Option<Image> {
    let planes = specular_jpeg::decode(o.data, &specular_jpeg::DecodeOptions::default()).ok()?;
    Some(Image {
        width: planes.width,
        height: planes.height,
        origin: (0, 0),
        format: PixelFormat::Rgba8,
        pixels: Pixels::U8(specular_jpeg::planes_to_rgba(&planes)),
        truncated: planes.truncated,
        reduce: 0,
    })
}

/// JPEG-LS: grey or RGB scaled to 8 bits (no mapping tables).
fn jpeg_ls(o: &ForeignObject<'_>) -> Option<Image> {
    let img = specular_jpegls::decode(o.data, &specular_jpegls::DecodeOptions::default()).ok()?;
    if img.planes.iter().any(|p| p.table.is_some()) {
        return None;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    if img
        .planes
        .iter()
        .any(|p| p.width as usize != w || p.height as usize != h)
    {
        return None;
    }
    // MAXVAL, which a JPEG-LS preset may set below 2^P - 1.
    let max = specular_jpegls::read_info(o.data)
        .map_or((1u32 << img.precision.clamp(1, 16)) - 1, |i| {
            u32::from(i.maxval)
        })
        .max(1);
    let to8 = |v: u16| ((u32::from(v).min(max) * 255 + max / 2) / max) as u8;
    let at = |p: &specular_jpegls::Plane, i: usize| to8(p.data.get(i).copied().unwrap_or(0));
    let (format, px) = match img.planes.len() {
        1 => (
            PixelFormat::Gray8,
            (0..w * h).map(|i| at(&img.planes[0], i)).collect(),
        ),
        3 => {
            let [r, g, b] = [&img.planes[0], &img.planes[1], &img.planes[2]];
            let mut px = Vec::with_capacity(w * h * 4);
            for i in 0..w * h {
                px.extend_from_slice(&[at(r, i), at(g, i), at(b, i), 255]);
            }
            (PixelFormat::Rgba8, px)
        }
        _ => return None,
    };
    Some(Image {
        width: img.width,
        height: img.height,
        origin: (0, 0),
        format,
        pixels: Pixels::U8(px),
        truncated: img.truncated,
        reduce: 0,
    })
}
