//! AVIF decoding for Specular.
//!
//! Our own HEIF/ISOBMFF parser (stills, grids, alpha auxiliary images, AVIS sequences) over an
//! AV1 backend: an in-tree fork of rav1d (the Rust port of dav1d), used through its safe API.
//! Output is 8-bit (Gray, RGB or RGBA; PQ/HLG tone mapped to sRGB), optionally box-downscaled
//! by 2, 4 or 8 in the same pass as the YUV→RGB conversion. HDR stills can also be decoded
//! to 16-bit RGBA holding the PQ/HLG signal itself ([`AvifFile::decode_hdr_with`]), or to their
//! YCbCr planes unconverted, for a viewer that converts them where they are shown
//! ([`AvifFile::decode_planes`]). Orientation (`irot`/`imir`) is reported as an EXIF
//! orientation value, never applied to the pixels.
//!
//! **Not supported:** a primary image or grid tiles coded with another codec than AV1 (HEVC,
//! AVC, JPEG, uncompressed, VVC, JPEG 2000) are refused with [`Error::OtherCodec`], which names
//! the codec; Specular's decode layer hands such files to its HEIF decoder.
//!
//! ```no_run
//! let bytes = std::fs::read("photo.avif").unwrap();
//! let file = specular_avif::AvifFile::open(bytes).unwrap();
//! let img = file.decode(2, true).unwrap(); // half size, multithreaded
//! println!("{}x{} {:?}", img.width, img.height, img.layout);
//! ```

#![deny(unsafe_code)]

mod backend;
mod bmff;
mod convert;
mod grid;
mod heif;
#[cfg(test)]
mod hostile;
mod planes;
mod seq;
pub mod video;

use std::sync::Arc;

use backend::{Av1Decoder, Cicp, Input, Layers, Picture, new_decoder, new_decoder_layers};
use bmff::Boxes;
use convert::{Conv, Dst, Region, Target, Tone};
use heif::{Av1Config, Crop, Grid, Meta, Nclx, Prop};
use seq::Sequence;

pub use planes::{YuvImage, YuvPlane};

/// Largest image (width × height) we decode.
pub const MAX_PIXELS: u64 = 1 << 29;

/// Decoding errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not an AVIF file")]
    NotAvif,
    #[error("file is truncated")]
    Truncated,
    #[error("malformed AVIF: {0}")]
    Malformed(String),
    #[error("unsupported AVIF feature: {0}")]
    Unsupported(String),
    /// The primary image (or a grid's tiles) coded with another codec (named: "HEVC",
    /// "JPEG", …), which a HEIF decoder may read.
    #[error("unsupported AVIF feature: image coded as {0}, not AV1")]
    OtherCodec(String),
    #[error("image too large")]
    TooLarge,
    #[error("AV1 decoder error: {0}")]
    Decoder(String),
    #[error("cancelled")]
    Cancelled,
}

/// HDR transfer function signalled by the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrTransfer {
    /// SMPTE ST 2084 (CICP transfer 16).
    Pq,
    /// ARIB STD-B67 hybrid log-gamma (CICP transfer 18).
    Hlg,
}

/// HDR description (tone mapped to sRGB by [`AvifFile::decode`], kept by
/// [`AvifFile::decode_hdr_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hdr {
    pub transfer: HdrTransfer,
    /// CICP colour primaries of the signal (`nclx`).
    pub primaries: u16,
    /// `clli` maximum content light level in nits (0 = unknown).
    pub max_cll: u16,
    /// `clli` maximum frame-average light level in nits (0 = unknown).
    pub max_fall: u16,
}

/// File-level information, available without decoding pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Displayed size before orientation (after the `clap` crop).
    pub width: u32,
    pub height: u32,
    /// EXIF orientation 1..=8 composed from `irot`/`imir`; not applied to decoded pixels.
    pub orientation: u8,
    pub icc: Option<Vec<u8>>,
    pub has_alpha: bool,
    /// The alpha is premultiplied into the colour (a `prem` reference between the colour and
    /// alpha items or tracks); decoded pixels are straight either way.
    pub premultiplied: bool,
    /// Coded bit depth (8, 10 or 12); output is always 8-bit.
    pub bit_depth: u8,
    pub hdr: Option<Hdr>,
    /// A `tmap` gain map item is present (the base image is what [`AvifFile::decode`]
    /// returns; see [`AvifFile::gain_map`]).
    pub gain_map: bool,
    /// Grid layout of the primary image as (columns, rows).
    pub grid: Option<(u32, u32)>,
    /// The file contains an AVIS image sequence.
    pub animated: bool,
    /// Frames in the sequence (1 for stills).
    pub frame_count: u32,
    /// Total plays of the sequence; 0 = loop forever (1 for stills).
    pub loop_count: u32,
    /// Chroma subsampling: "4:2:0", "4:2:2", "4:4:4" or "4:0:0".
    pub chroma: &'static str,
    /// CICP matrix coefficients of the container's `nclx` (0 = RGB stored as is, 8 = YCgCo),
    /// when it has one; otherwise the bitstream's applies.
    pub matrix: Option<u16>,
    /// CICP (colour primaries, transfer characteristics) of the container's `nclx`, when it
    /// has one.
    pub cicp: Option<(u16, u16)>,
}

/// Pixel layout of [`Decoded::data`] (rows tightly packed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// 8 bits.
    Gray,
    /// 8 bits per channel.
    Rgb,
    /// 8 bits per channel.
    Rgba,
    /// 16 bits per channel (native-endian `u16`): the HDR signal of
    /// [`AvifFile::decode_hdr_with`], straight alpha (65535 when opaque).
    Rgba16,
}

impl Layout {
    pub fn channels(self) -> usize {
        match self {
            Layout::Gray => 1,
            Layout::Rgb => 3,
            Layout::Rgba | Layout::Rgba16 => 4,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Layout::Rgba16 => 8,
            l => l.channels(),
        }
    }
}

/// A decoded image or animation frame (straight, non-premultiplied alpha).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// The downscale factor applied (1, 2, 4 or 8).
    pub scale: u32,
    pub layout: Layout,
    pub data: Vec<u8>,
    /// Frame duration (animation frames; 0 for stills).
    pub duration_ms: u32,
}

/// An ISO 21496-1 gain map attached to the primary image: a `tmap` item whose `dimg`
/// inputs are the primary (base) image and the map image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GainMapInfo {
    /// The `tmap` item body: a version byte, then the ISO 21496-1 metadata.
    pub metadata: Vec<u8>,
    /// CICP colour primaries of the base image (its `nclx`), when signalled.
    pub base_primaries: Option<u16>,
    /// CICP colour primaries of the alternate rendition (the `tmap` item's `nclx`).
    pub alternate_primaries: Option<u16>,
    /// Size of the map image.
    pub width: u32,
    pub height: u32,
}

/// Returns true if `head` (the first bytes of a file, 12 or more) looks like AVIF.
pub fn probe(head: &[u8]) -> bool {
    if head.len() < 12 || &head[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
    let end = size.min(head.len());
    if size < 16 {
        return is_avif_brand(&head[8..12]);
    }
    is_avif_brand(&head[8..12])
        || head
            .get(16..end)
            .is_some_and(|b| b.chunks_exact(4).any(is_avif_brand))
}

fn is_avif_brand(b: &[u8]) -> bool {
    b == b"avif" || b == b"avis"
}

/// Where the pixels of an image come from.
#[derive(Clone, Debug)]
enum Source {
    Item(u32),
    Grid { grid: Grid, tiles: Vec<u32> },
}

/// How to decode the primary still image.
#[derive(Clone, Debug)]
struct Still {
    color: Source,
    alpha: Option<Source>,
    premultiplied: bool,
    clap: Option<[u32; 8]>,
    nclx: Option<Nclx>,
    max_cll: u16,
    /// Operating point and layer of the colour and alpha items (`a1op`, `lsel`).
    color_layers: Layers,
    alpha_layers: Layers,
}

/// An opened AVIF file. Cheap to clone the underlying bytes (`Arc`); all decoding methods
/// take `&self`.
pub struct AvifFile {
    data: Arc<[u8]>,
    meta: Option<Meta>,
    still: Option<Still>,
    seq: Option<Sequence>,
    info: Info,
    /// The `tmap` item and its map image, when the primary has a gain map.
    gain_map: Option<(u32, u32)>,
    /// Top-level image items (see [`AvifFile::page_count`]) and the one this file shows.
    pages: Vec<u32>,
    page: u32,
}

impl std::fmt::Debug for AvifFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AvifFile")
            .field("info", &self.info)
            .finish()
    }
}

fn hdr_of(nclx: Option<Nclx>, max_cll: u16, max_fall: u16) -> Option<Hdr> {
    let transfer = match nclx?.transfer {
        16 => HdrTransfer::Pq,
        18 => HdrTransfer::Hlg,
        _ => return None,
    };
    Some(Hdr {
        transfer,
        primaries: nclx?.primaries,
        max_cll,
        max_fall,
    })
}

impl AvifFile {
    /// Parses the container (no AV1 decoding happens here).
    pub fn open(data: impl Into<Arc<[u8]>>) -> Result<AvifFile, Error> {
        Self::open_page(data, 0)
    }

    /// Parses the container to show page `page` (0-based, below [`AvifFile::page_count`]):
    /// page 0 is the primary image (and the sequence, if any), the others the further
    /// top-level images of a collection, each with its own alpha, gain map and properties.
    pub fn open_page(data: impl Into<Arc<[u8]>>, page: u32) -> Result<AvifFile, Error> {
        let data: Arc<[u8]> = data.into();
        let mut meta_box = None;
        let mut moov_box = None;
        let mut first = true;
        let mut brand_ok = false;
        let mut tail_err = None;
        for b in Boxes::top_level(&data) {
            let b = match b {
                Ok(b) => b,
                Err(e) => {
                    if first {
                        return Err(if data.len() < 8 || !probe(&data) {
                            if data.len() >= 8 && &data[4..8] == b"ftyp" {
                                Error::Truncated
                            } else {
                                Error::NotAvif
                            }
                        } else {
                            e
                        });
                    }
                    tail_err = Some(e);
                    break;
                }
            };
            if first {
                if &b.typ != b"ftyp" || b.payload.len() < 8 {
                    return Err(Error::NotAvif);
                }
                let p = b.payload;
                brand_ok = matches!(&p[0..4], b"avif" | b"avis")
                    || p[8..].chunks_exact(4).any(is_avif_brand);
                first = false;
                continue;
            }
            match &b.typ {
                b"meta" if meta_box.is_none() => meta_box = Some(b.payload),
                b"moov" if moov_box.is_none() => moov_box = Some(b.payload),
                _ => {}
            }
        }
        if first {
            return Err(if data.is_empty() {
                Error::Truncated
            } else {
                Error::NotAvif
            });
        }
        if !brand_ok {
            return Err(Error::NotAvif);
        }
        let mut meta = match meta_box {
            Some(m) => Some(Meta::parse(m)?),
            None => None,
        };
        // Images that would fail to open are not pages.
        let pages: Vec<u32> = meta.as_ref().map_or_else(Vec::new, |m| {
            m.top_level()
                .into_iter()
                .enumerate()
                .filter(|&(i, id)| i == 0 || source_of(m, &data, id).is_ok())
                .map(|(_, id)| id)
                .collect()
        });
        if page > 0 {
            let id = *pages
                .get(page as usize)
                .ok_or_else(|| Error::Malformed("no such page".into()))?;
            if let Some(m) = &mut meta {
                m.primary = Some(id);
            }
        }
        let still = match &meta {
            Some(m) if m.primary.is_some() => Some(parse_still(m, &data)?),
            _ => None,
        };
        let seq = match moov_box.filter(|_| page == 0) {
            Some(m) => match Sequence::parse(m) {
                Ok(s) => Some(s),
                Err(e) if still.is_none() => return Err(e),
                Err(_) => None,
            },
            None => None,
        };
        if still.is_none() && seq.is_none() {
            return Err(tail_err.unwrap_or_else(|| {
                if meta.is_some() {
                    Error::Malformed("no primary item".into())
                } else {
                    Error::Malformed("no meta or moov box".into())
                }
            }));
        }
        let info = build_info(meta.as_ref(), still.as_ref(), seq.as_ref(), &data)?;
        let gain_map = match (&meta, &still) {
            (Some(m), Some(_)) => find_gain_map(m),
            _ => None,
        };
        Ok(AvifFile {
            data,
            meta,
            still,
            seq,
            info,
            gain_map,
            pages,
            page,
        })
    }

    /// The images a viewer pages through: the primary and the further top-level images of a
    /// collection (not thumbnails, alpha or depth images, grid tiles or gain maps); 1 for a
    /// single image or a sequence.
    pub fn page_count(&self) -> u32 {
        (self.pages.len() as u32).max(1)
    }

    /// Page `page` of the same file, from the bytes already held (no copy).
    pub fn reopen_page(&self, page: u32) -> Result<AvifFile, Error> {
        Self::open_page(self.data.clone(), page)
    }

    /// The page this file shows ([`AvifFile::open_page`]).
    pub fn page(&self) -> u32 {
        self.page
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    /// The primary image's gain map, if any (container metadata only; nothing is decoded).
    pub fn gain_map(&self) -> Option<GainMapInfo> {
        let (tmap, map) = self.gain_map?;
        let meta = self.meta.as_ref()?;
        let nclx = |id| {
            meta.props_of(id).find_map(|p| match p {
                Prop::Nclx(n) => Some(n.primaries),
                _ => None,
            })
        };
        let (width, height) = source_size(meta, &self.data, map).ok()?;
        Some(GainMapInfo {
            metadata: meta.item_data(&self.data, tmap).ok()?.into_owned(),
            base_primaries: nclx(meta.primary.unwrap_or(0)),
            alternate_primaries: nclx(tmap),
            width,
            height,
        })
    }

    /// Decodes the gain map image at full size. The samples are the map's code values
    /// (YUV to RGB through the map's own matrix, no transfer function or gamut change),
    /// 8 bits per channel: [`Layout::Gray`] for a single-channel map, else [`Layout::Rgb`].
    pub fn decode_gain_map(
        &self,
        parallel: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Decoded, Error> {
        let (_, map) = self
            .gain_map
            .ok_or_else(|| Error::Malformed("no gain map".into()))?;
        let meta = self
            .meta
            .as_ref()
            .ok_or_else(|| Error::Malformed("no gain map".into()))?;
        let src = source_of(meta, &self.data, map)?;
        let (w, h) = source_size(meta, &self.data, map)?;
        check_size(w, h)?;
        let layout = if is_mono(meta, &src) {
            Layout::Gray
        } else {
            Layout::Rgb
        };
        let ch = layout.channels();
        let mut dec = new_decoder(if parallel { 0 } else { 1 })?;
        let map_nclx = meta.props_of(map).find_map(|p| match p {
            Prop::Nclx(n) => Some(*n),
            _ => None,
        });
        let conv_for = move |p: &Picture_| {
            Conv::new(
                map_nclx.map_or(p.cicp.matrix, |n| n.matrix),
                map_nclx.map_or(p.cicp.full_range, |n| n.full_range),
                1,
                p.bits,
                Tone::Sdr,
            )
        };
        let (w, h) = (w as usize, h as usize);
        let mut buf = vec![0u8; w * h * ch];
        let mut dst = Dst {
            data: &mut buf,
            width: w,
            height: h,
            channels: ch,
            wide: false,
            x: 0,
            y: 0,
        };
        render_source(
            &self.data,
            meta,
            &src,
            dec.as_mut(),
            &conv_for,
            None,
            1,
            &mut dst,
            Target::Color,
            parallel,
            cancel,
        )?;
        Ok(Decoded {
            width: w as u32,
            height: h as u32,
            scale: 1,
            layout,
            data: buf,
            duration_ms: 0,
        })
    }

    /// Estimated peak memory in bytes for [`AvifFile::decode`] at `scale`: the output buffer
    /// plus the decoder's YUV frame buffers.
    pub fn decode_memory(&self, scale: u32) -> u64 {
        self.memory(scale, false)
    }

    /// [`AvifFile::decode_memory`] for [`AvifFile::decode_hdr_with`].
    pub fn decode_hdr_memory(&self, scale: u32) -> u64 {
        self.memory(scale, true)
    }

    fn memory(&self, scale: u32, wide: bool) -> u64 {
        let s = u64::from(norm_scale(scale));
        let (w, h) = (u64::from(self.info.width), u64::from(self.info.height));
        let ch = if wide {
            8
        } else if self.info.has_alpha {
            4
        } else if self.info.chroma == "4:0:0" {
            1
        } else {
            3
        };
        let out = w.div_ceil(s) * h.div_ceil(s) * ch;
        let bps = if self.info.bit_depth > 8 { 2 } else { 1 };
        let plane_factor = match self.info.chroma {
            "4:0:0" => 1.0,
            "4:2:0" => 1.5,
            "4:2:2" => 2.0,
            _ => 3.0,
        };
        let tiles = match self.info.grid {
            // Tiles are decoded one at a time; a misaligned grid needs a full-size canvas.
            Some(_) => w * h * ch,
            // So does reduced straight alpha (see `decode_still`).
            None if self.info.has_alpha && s > 1 => w * h * ch,
            None => 0,
        };
        let mut yuv = ((w * h) as f64 * plane_factor) as u64 * bps;
        if self.info.has_alpha {
            yuv += w * h * bps;
        }
        out + yuv + tiles
    }

    /// Decodes the primary image (or the first frame of a sequence-only file) at 1/`scale`
    /// (1, 2, 4 or 8; other values round down to one of these).
    pub fn decode(&self, scale: u32, parallel: bool) -> Result<Decoded, Error> {
        self.decode_with(scale, parallel, &|| false)
    }

    /// [`AvifFile::decode`] with a cancellation callback, polled between tiles and row bands.
    pub fn decode_with(
        &self,
        scale: u32,
        parallel: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Decoded, Error> {
        let scale = norm_scale(scale);
        match (&self.still, &self.meta) {
            (Some(still), Some(meta)) => {
                decode_still(&self.data, meta, still, scale, parallel, false, cancel)
            }
            _ => {
                let mut f = self.frames(scale, parallel)?;
                match f.next_with(cancel) {
                    Some(r) => r.map(|mut d| {
                        d.duration_ms = 0;
                        d
                    }),
                    None => Err(Error::Malformed("empty sequence".into())),
                }
            }
        }
    }

    /// Plays the AVIS sequence frame by frame (a still file yields its single image).
    /// Decodes the primary still image of an HDR file ([`Info::hdr`]) like
    /// [`AvifFile::decode_with`], but to [`Layout::Rgba16`] holding the PQ or HLG signal in
    /// the file's primaries ([`Hdr::primaries`]), full range, without tone mapping: for a
    /// renderer that linearises and tone maps for the display itself, and for gain maps on
    /// an HDR base. Box downscaling averages the signal, as the 8-bit path does.
    pub fn decode_hdr_with(
        &self,
        scale: u32,
        parallel: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Decoded, Error> {
        let scale = norm_scale(scale);
        match (&self.still, &self.meta, self.info.hdr) {
            (Some(still), Some(meta), Some(_)) => {
                decode_still(&self.data, meta, still, scale, parallel, true, cancel)
            }
            _ => Err(Error::Unsupported("not an HDR still image".into())),
        }
    }

    pub fn frames(&self, scale: u32, parallel: bool) -> Result<Frames, Error> {
        let scale = norm_scale(scale);
        let threads = if parallel { 0 } else { 1 };
        match &self.seq {
            Some(seq) => {
                let (w, h) = (seq.color.width, seq.color.height);
                if w == 0 || h == 0 {
                    return Err(Error::Malformed("sequence has no size".into()));
                }
                check_size(w, h)?;
                Ok(Frames {
                    data: self.data.clone(),
                    kind: FramesKind::Seq {
                        seq: Box::new(seq.clone()),
                        color: new_decoder(threads)?,
                        alpha: match seq.alpha {
                            Some(_) => Some(new_decoder(threads)?),
                            None => None,
                        },
                        max_cll: self.info.hdr.map_or(0, |h| h.max_cll),
                    },
                    index: 0,
                    scale,
                    parallel,
                })
            }
            None => Ok(Frames {
                data: self.data.clone(),
                kind: FramesKind::Still(Box::new(self.decode(scale, parallel)?)),
                index: 0,
                scale,
                parallel,
            }),
        }
    }
}

fn norm_scale(s: u32) -> u32 {
    match s {
        0 | 1 => 1,
        2 | 3 => 2,
        4..=7 => 4,
        _ => 8,
    }
}

fn check_size(w: u32, h: u32) -> Result<(), Error> {
    if w == 0 || h == 0 {
        return Err(Error::Malformed("zero image size".into()));
    }
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    Ok(())
}

const ALPHA_URNS: [&[u8]; 2] = [
    b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha",
    b"urn:mpeg:hevc:2015:auxid:1",
];

fn source_of(meta: &Meta, data: &[u8], id: u32) -> Result<Source, Error> {
    let item = meta
        .item(id)
        .ok_or_else(|| Error::Malformed(format!("item {id} missing")))?;
    match &item.typ {
        b"av01" => Ok(Source::Item(id)),
        b"grid" => {
            // The descriptor is at most 12 bytes: the rest of the item is never read.
            let grid = Grid::parse(&meta.item_prefix(data, id, Grid::MAX_LEN)?)?;
            let tiles = meta.refs_from(id, b"dimg");
            if tiles.len() as u64 != u64::from(grid.rows) * u64::from(grid.cols) {
                return Err(Error::Malformed("grid tile count mismatch".into()));
            }
            for &t in &tiles {
                match meta.item(t) {
                    Some(it) if &it.typ == b"av01" => {}
                    Some(it) => return Err(Error::OtherCodec(codec_name(&it.typ))),
                    None => return Err(Error::Malformed(format!("grid tile {t} missing"))),
                }
            }
            Ok(Source::Grid { grid, tiles })
        }
        t @ (b"hvc1" | b"hev1" | b"avc1" | b"jpeg" | b"unci" | b"vvc1" | b"j2k1") => {
            Err(Error::OtherCodec(codec_name(t)))
        }
        t => Err(Error::Unsupported(format!(
            "primary item type '{}'",
            bmff::fourcc_str(t)
        ))),
    }
}

/// What a HEIF item type is coded with, in words.
fn codec_name(t: &[u8; 4]) -> String {
    match t {
        b"hvc1" | b"hev1" => "HEVC".into(),
        b"avc1" => "AVC".into(),
        b"jpeg" => "JPEG".into(),
        b"unci" => "uncompressed samples".into(),
        b"vvc1" => "VVC".into(),
        b"j2k1" => "JPEG 2000".into(),
        b"grid" | b"iovl" | b"iden" => "a derived image".into(),
        t => format!("'{}'", bmff::fourcc_str(t)),
    }
}

/// The `tmap` item derived from the primary image and the map image it applies:
/// `dimg` inputs `[primary, map]` (ISO/IEC 23008-12 tone map derivation).
fn find_gain_map(meta: &Meta) -> Option<(u32, u32)> {
    let primary = meta.primary?;
    meta.items
        .iter()
        .filter(|it| &it.typ == b"tmap")
        .find_map(|it| match meta.refs_from(it.id, b"dimg")[..] {
            [base, map] if base == primary && map != primary => Some((it.id, map)),
            _ => None,
        })
}

/// Size of an image source: the grid canvas or the item's `ispe`.
fn source_size(meta: &Meta, data: &[u8], id: u32) -> Result<(u32, u32), Error> {
    match source_of(meta, data, id)? {
        Source::Grid { grid, .. } => Ok((grid.width, grid.height)),
        Source::Item(_) => meta
            .props_of(id)
            .find_map(|p| match p {
                Prop::Ispe(w, h) => Some((*w, *h)),
                _ => None,
            })
            .ok_or_else(|| Error::Malformed("missing ispe".into())),
    }
}

/// Whether the coded item behind `src` (the first tile of a grid) is monochrome.
/// The `a1op` and `lsel` of an item, or else of a grid's first tile (AVIF 1.1: the operating
/// point to decode and the layer to output; 0xFFFF and no `lsel` mean the highest layer).
fn layers_of(meta: &Meta, id: u32, src: &Source) -> Layers {
    let of = |id: u32| {
        let mut l = Layers::default();
        let mut found = false;
        for p in meta.props_of(id) {
            match p {
                Prop::A1op(op) => (l.op, found) = (*op, true),
                Prop::Lsel(k) => (l.layer, found) = ((*k != 0xffff).then_some(*k), true),
                _ => {}
            }
        }
        found.then_some(l)
    };
    of(id)
        .or_else(|| match src {
            Source::Grid { tiles, .. } => tiles.first().and_then(|&t| of(t)),
            Source::Item(_) => None,
        })
        .unwrap_or_default()
}

fn is_mono(meta: &Meta, src: &Source) -> bool {
    let coded = match src {
        Source::Item(id) => *id,
        Source::Grid { tiles, .. } => tiles.first().copied().unwrap_or(0),
    };
    meta.props_of(coded)
        .any(|p| matches!(p, Prop::Av1C(c) if c.monochrome))
}

fn parse_still(meta: &Meta, data: &[u8]) -> Result<Still, Error> {
    let primary = meta.primary.unwrap_or(0);
    let color = source_of(meta, data, primary)?;
    let mut alpha_id = None;
    for id in meta.refs_to(primary, b"auxl") {
        let is_alpha = meta.props_of(id).any(|p| match p {
            Prop::AuxC(urn) => ALPHA_URNS.contains(&urn.as_slice()),
            _ => false,
        });
        if is_alpha {
            alpha_id = Some(id);
            break;
        }
    }
    let alpha = match alpha_id {
        Some(id) => Some(source_of(meta, data, id)?),
        None => None,
    };
    let color_layers = layers_of(meta, primary, &color);
    let alpha_layers = match (alpha_id, &alpha) {
        (Some(id), Some(src)) => layers_of(meta, id, src),
        _ => Layers::default(),
    };
    let premultiplied = alpha_id.is_some_and(|a| {
        meta.refs_from(primary, b"prem").contains(&a)
            || meta.refs_from(a, b"prem").contains(&primary)
    });
    let mut clap = None;
    let mut nclx = None;
    let mut max_cll = 0;
    for p in meta.props_of(primary) {
        match p {
            Prop::Clap(c) => clap = Some(*c),
            Prop::Nclx(n) if nclx.is_none() => nclx = Some(*n),
            Prop::Clli(c, _) => max_cll = *c,
            _ => {}
        }
    }
    Ok(Still {
        color,
        alpha,
        premultiplied,
        clap,
        nclx,
        max_cll,
        color_layers,
        alpha_layers,
    })
}

fn build_info(
    meta: Option<&Meta>,
    still: Option<&Still>,
    seq: Option<&Sequence>,
    data: &[u8],
) -> Result<Info, Error> {
    let _ = data;
    let mut info = Info {
        width: 0,
        height: 0,
        orientation: 1,
        icc: None,
        has_alpha: false,
        premultiplied: false,
        bit_depth: 8,
        hdr: None,
        gain_map: false,
        grid: None,
        animated: false,
        frame_count: 1,
        loop_count: 1,
        chroma: "4:2:0",
        matrix: None,
        cicp: None,
    };
    if let (Some(meta), Some(still)) = (meta, still) {
        let primary = meta.primary.unwrap_or(0);
        let mut ispe = None;
        let mut max_fall = 0;
        for p in meta.props_of(primary) {
            match p {
                Prop::Ispe(w, h) => ispe = Some((*w, *h)),
                Prop::Icc(icc) if info.icc.is_none() => info.icc = Some(icc.clone()),
                Prop::Clli(_, f) => max_fall = *f,
                _ => {}
            }
        }
        // The av1C of the item (or of the first grid tile).
        let coded = match &still.color {
            Source::Item(id) => *id,
            Source::Grid { tiles, .. } => tiles.first().copied().unwrap_or(0),
        };
        let cfg: Option<Av1Config> = meta.props_of(coded).find_map(|p| match p {
            Prop::Av1C(c) => Some(*c),
            _ => None,
        });
        if let Some(c) = cfg {
            info.bit_depth = c.bit_depth;
            info.chroma = c.chroma();
        }
        let (w, h) = match &still.color {
            Source::Grid { grid, .. } => {
                info.grid = Some((grid.cols, grid.rows));
                (grid.width, grid.height)
            }
            Source::Item(_) => ispe.ok_or_else(|| Error::Malformed("missing ispe".into()))?,
        };
        let crop = still.clap.and_then(|c| heif::clap_crop(&c, w, h));
        (info.width, info.height) = crop.map_or((w, h), |c| (c.w, c.h));
        info.orientation = heif::orientation(meta.props_of(primary));
        info.has_alpha = still.alpha.is_some();
        info.premultiplied = still.premultiplied;
        info.hdr = hdr_of(still.nclx, still.max_cll, max_fall);
        info.matrix = still.nclx.map(|n| n.matrix);
        info.cicp = still.nclx.map(|n| (n.primaries, n.transfer));
        info.gain_map = meta.items.iter().any(|it| &it.typ == b"tmap");
    }
    if let Some(seq) = seq {
        info.animated = true;
        info.frame_count = u32::try_from(seq.color.samples.len()).unwrap_or(u32::MAX);
        info.loop_count = seq.loop_count;
        if still.is_none() {
            info.width = seq.color.width;
            info.height = seq.color.height;
            info.has_alpha = seq.alpha.is_some();
            info.premultiplied = seq.alpha.as_ref().is_some_and(|at| {
                at.prem.contains(&seq.color.id) || seq.color.prem.contains(&at.id)
            });
            if let Some(c) = seq.color.config {
                info.bit_depth = c.bit_depth;
                info.chroma = c.chroma();
            }
            info.icc = seq.color.icc.clone();
            info.hdr = hdr_of(seq.color.nclx, 0, 0);
            info.matrix = seq.color.nclx.map(|n| n.matrix);
            info.cicp = seq.color.nclx.map(|n| (n.primaries, n.transfer));
        }
    }
    Ok(info)
}

/// Builds the decoder input for item `id`, sharing the file bytes when possible.
fn item_input(meta: &Meta, data: &Arc<[u8]>, id: u32) -> Result<Input, Error> {
    let bytes = meta.item_data(data, id)?;
    let base = data.as_ptr() as usize;
    let p = bytes.as_ptr() as usize;
    if p >= base && p + bytes.len() <= base + data.len() {
        let start = p - base;
        if data.get(start..start + bytes.len()) == Some(&bytes[..]) {
            return Ok(Input::Shared(data.clone(), start..start + bytes.len()));
        }
    }
    Ok(Input::Owned(bytes.into_owned()))
}

/// Colour conversion parameters: the container's `nclx` wins over the bitstream, the range
/// flag too (as libavif and libheif read it: Microsoft's test files say full range in `colr`
/// and limited in the AV1 header, and both decode them as full; fidelity round 4).
fn color_conv(nclx: Option<Nclx>, pic: Cicp, bits: u8, max_cll: u16) -> Conv {
    let (primaries, transfer, matrix, full_range) = match nclx {
        Some(n) => (n.primaries, n.transfer, n.matrix, n.full_range),
        None => (pic.primaries, pic.transfer, pic.matrix, pic.full_range),
    };
    let tone = match transfer {
        16 => Tone::Pq {
            peak: if max_cll > 0 {
                f32::from(max_cll)
            } else {
                1000.0
            },
        },
        18 => Tone::Hlg,
        _ => Tone::Sdr,
    };
    Conv::new(matrix, full_range, primaries, bits, tone)
}

/// Decodes one image source (single item or grid) and converts it into `dst`.
#[allow(clippy::too_many_arguments)]
fn render_source(
    data: &Arc<[u8]>,
    meta: &Meta,
    src: &Source,
    dec: &mut dyn Av1Decoder,
    conv_for: &dyn Fn(&Picture_) -> Conv,
    crop: Option<Crop>,
    scale: usize,
    dst: &mut Dst<'_>,
    target: Target,
    parallel: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<(), Error> {
    match src {
        Source::Item(id) => {
            let pic = decode_one(dec, item_input(meta, data, *id)?)?;
            let p = pic.planes();
            let (w, h) = (p.width as usize, p.height as usize);
            let region = match crop {
                Some(c) => Region {
                    x: c.x as usize,
                    y: c.y as usize,
                    w: (c.w as usize).min(w.saturating_sub(c.x as usize)),
                    h: (c.h as usize).min(h.saturating_sub(c.y as usize)),
                    scale,
                },
                None => Region {
                    x: 0,
                    y: 0,
                    w,
                    h,
                    scale,
                },
            };
            if region.w == 0 || region.h == 0 {
                return Err(Error::Malformed("crop outside the image".into()));
            }
            let conv = conv_for(&p);
            convert::convert(&p, &conv, region, dst, target, parallel, cancel)
        }
        Source::Grid { grid, tiles } => {
            let (gw, gh) = (grid.width as usize, grid.height as usize);
            if cancel() {
                return Err(Error::Cancelled);
            }
            let first = decode_one(dec, item_input(meta, data, tiles[0])?)?;
            let (tile_w, tile_h, stitch) = {
                let p = first.planes();
                (
                    p.width as usize,
                    p.height as usize,
                    grid::stitches(grid, &p, target),
                )
            };
            if tile_w * (grid.cols as usize) < gw || tile_h * (grid.rows as usize) < gh {
                return Err(Error::Malformed(
                    "grid tiles do not cover the canvas".into(),
                ));
            }
            let mut decode = |i: usize| decode_one(dec, item_input(meta, data, tiles[i])?);
            if stitch {
                return grid::render_stitched(
                    grid,
                    first,
                    &mut decode,
                    conv_for,
                    scale,
                    dst,
                    parallel,
                    cancel,
                );
            }
            let mut first = Some(first);
            for i in 0..tiles.len() {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                let pic = match first.take() {
                    Some(p) => p,
                    None => decode(i)?,
                };
                let p = pic.planes();
                if p.width as usize != tile_w || p.height as usize != tile_h {
                    return Err(Error::Malformed("grid tiles differ in size".into()));
                }
                let (r, c) = (i / grid.cols as usize, i % grid.cols as usize);
                let (x0, y0) = (c * tile_w, r * tile_h);
                if x0 >= gw || y0 >= gh {
                    continue;
                }
                let region = Region {
                    x: 0,
                    y: 0,
                    w: tile_w.min(gw - x0),
                    h: tile_h.min(gh - y0),
                    scale,
                };
                let conv = conv_for(&p);
                let (dx, dy) = (dst.x, dst.y);
                dst.x = dx + x0 / scale;
                dst.y = dy + y0 / scale;
                let r = convert::convert(&p, &conv, region, dst, target, parallel, cancel);
                dst.x = dx;
                dst.y = dy;
                r?;
            }
            Ok(())
        }
    }
}

type Picture_<'a> = backend::Planes<'a>;

fn decode_one(dec: &mut dyn Av1Decoder, input: Input) -> Result<Box<dyn Picture>, Error> {
    dec.decode(input)?
        .ok_or_else(|| Error::Decoder("no picture decoded".into()))
}

/// The size of the primary still before its crop: the grid canvas or the item's `ispe`.
fn still_size(meta: &Meta, still: &Still) -> Result<(u32, u32), Error> {
    let primary = meta.primary.unwrap_or(0);
    let ispe = meta.props_of(primary).find_map(|p| match p {
        Prop::Ispe(w, h) => Some((*w, *h)),
        _ => None,
    });
    match &still.color {
        Source::Grid { grid, .. } => Ok((grid.width, grid.height)),
        Source::Item(_) => ispe.ok_or_else(|| Error::Malformed("missing ispe".into())),
    }
}

/// The still's clean-aperture crop of a `w` × `h` image, if it has one.
fn clap_of(still: &Still, w: u32, h: u32) -> Option<Crop> {
    still.clap.and_then(|c| heif::clap_crop(&c, w, h))
}

/// The `av1C` of the coded item behind `src` (the first tile of a grid).
fn coded_config(meta: &Meta, src: &Source) -> Option<Av1Config> {
    let coded = match src {
        Source::Item(id) => *id,
        Source::Grid { tiles, .. } => tiles.first().copied()?,
    };
    meta.props_of(coded).find_map(|p| match p {
        Prop::Av1C(c) => Some(*c),
        _ => None,
    })
}

fn decode_still(
    data: &Arc<[u8]>,
    meta: &Meta,
    still: &Still,
    scale: u32,
    parallel: bool,
    wide: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Decoded, Error> {
    let (full_w, full_h) = still_size(meta, still)?;
    check_size(full_w, full_h)?;
    let crop = clap_of(still, full_w, full_h);
    let mono = is_mono(meta, &still.color);
    let layout = match (still.alpha.is_some(), mono) {
        _ if wide => Layout::Rgba16,
        (true, _) => Layout::Rgba,
        (false, true) => Layout::Gray,
        (false, false) => Layout::Rgb,
    };
    let ch = layout.channels();
    let bpp = layout.bytes_per_pixel();
    let s = scale as usize;
    let threads = if parallel { 0 } else { 1 };
    let mut dec = new_decoder_layers(threads, still.color_layers)?;
    let nclx = still.nclx;
    let max_cll = still.max_cll;
    let color_conv_for = move |p: &Picture_| {
        let c = color_conv(nclx, p.cicp, p.bits, max_cll);
        if wide { c.with_tone(Tone::Wide) } else { c }
    };
    let alpha_conv_for = |p: &Picture_| Conv::alpha(p.cicp.full_range, p.bits);

    // A grid needs an intermediate full-size canvas when a crop applies or the tiles do not
    // align with the scale factor; we only know tile sizes after decoding, so take the
    // direct path only when the grid's tile size (from the first tile's ispe) is aligned.
    // Straight alpha reduced needs it too: colour and alpha are separate pictures, and
    // colour must be averaged weighted by alpha (no fringes from colour under transparency).
    let weighted = still.alpha.is_some() && !still.premultiplied && s > 1;
    let alpha = still.alpha.is_some();
    let direct = !weighted && match &still.color {
        Source::Item(_) => true,
        Source::Grid { tiles, .. } => {
            crop.is_none()
                && tiles.first().is_some_and(|&t| {
                    meta.props_of(t).any(|p| {
                        matches!(p, Prop::Ispe(w, h) if (*w as usize).is_multiple_of(s) && (*h as usize).is_multiple_of(s))
                    })
                })
        }
    };
    let (out_w, out_h, work_s) = if direct {
        let (cw, chh) = crop.map_or((full_w, full_h), |c| (c.w, c.h));
        ((cw as usize).div_ceil(s), (chh as usize).div_ceil(s), s)
    } else {
        (full_w as usize, full_h as usize, 1)
    };
    // The wide layout always has alpha: opaque (all ones) unless an alpha plane follows.
    let mut buf = vec![if wide { 0xff } else { 0 }; out_w * out_h * bpp];
    {
        let mut dst = Dst {
            data: &mut buf,
            width: out_w,
            height: out_h,
            channels: ch,
            wide,
            x: 0,
            y: 0,
        };
        let item_crop = if direct { crop } else { None };
        render_source(
            data,
            meta,
            &still.color,
            dec.as_mut(),
            &color_conv_for,
            item_crop,
            work_s,
            &mut dst,
            Target::Color,
            parallel,
            cancel,
        )?;
        if let Some(alpha) = &still.alpha {
            dec = new_decoder_layers(threads, still.alpha_layers)?;
            render_source(
                data,
                meta,
                alpha,
                dec.as_mut(),
                &alpha_conv_for,
                item_crop,
                work_s,
                &mut dst,
                Target::Alpha,
                parallel,
                cancel,
            )?;
        }
    }
    let (mut w, mut h) = (out_w, out_h);
    if !direct {
        let c = crop.unwrap_or(Crop {
            x: 0,
            y: 0,
            w: full_w,
            h: full_h,
        });
        let region = Region {
            x: c.x as usize,
            y: c.y as usize,
            w: c.w as usize,
            h: c.h as usize,
            scale: s,
        };
        buf = if wide {
            convert::downscale16(&buf, out_w, ch, region, weighted, parallel)
        } else {
            convert::downscale(&buf, out_w, ch, region, weighted, parallel)
        };
        (w, h) = (region.out_w(), region.out_h());
    }
    if still.premultiplied && alpha {
        if wide {
            convert::unpremultiply16(&mut buf, parallel);
        } else {
            convert::unpremultiply(&mut buf, parallel);
        }
    }
    Ok(Decoded {
        width: w as u32,
        height: h as u32,
        scale,
        layout,
        data: buf,
        duration_ms: 0,
    })
}

enum FramesKind {
    Still(Box<Decoded>),
    Seq {
        seq: Box<Sequence>,
        color: Box<dyn Av1Decoder>,
        alpha: Option<Box<dyn Av1Decoder>>,
        max_cll: u16,
    },
}

/// Sequential frame decoder for AVIS sequences: one AV1 decoder per track, one frame in
/// memory at a time. Each frame is a full canvas.
pub struct Frames {
    data: Arc<[u8]>,
    kind: FramesKind,
    index: usize,
    scale: u32,
    parallel: bool,
}

impl Frames {
    /// Number of frames.
    pub fn len(&self) -> usize {
        match &self.kind {
            FramesKind::Still(_) => 1,
            FramesKind::Seq { seq, .. } => seq.color.samples.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Restarts playback from the first frame.
    pub fn rewind(&mut self) {
        self.index = 0;
        if let FramesKind::Seq { color, alpha, .. } = &mut self.kind {
            color.reset();
            if let Some(a) = alpha {
                a.reset();
            }
        }
    }

    /// Decodes the next frame, polling `cancel` during conversion.
    pub fn next_with(&mut self, cancel: &dyn Fn() -> bool) -> Option<Result<Decoded, Error>> {
        if self.index >= self.len() {
            return None;
        }
        let i = self.index;
        self.index += 1;
        let r = match &mut self.kind {
            FramesKind::Still(d) => Ok((**d).clone()),
            FramesKind::Seq {
                seq,
                color,
                alpha,
                max_cll,
            } => seq_frame(
                &self.data,
                seq,
                color.as_mut(),
                alpha.as_mut().map(|a| a.as_mut() as &mut dyn Av1Decoder),
                *max_cll,
                i,
                self.scale,
                self.parallel,
                cancel,
            ),
        };
        if r.is_err() {
            // A broken frame ends playback (later frames depend on it).
            self.index = self.len();
        }
        Some(r)
    }
}

impl Iterator for Frames {
    type Item = Result<Decoded, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_with(&|| false)
    }
}

fn sample_input(data: &Arc<[u8]>, s: &seq::Sample) -> Result<Input, Error> {
    let start = usize::try_from(s.offset).map_err(|_| Error::Truncated)?;
    let end = start
        .checked_add(s.size as usize)
        .filter(|&e| e <= data.len())
        .ok_or(Error::Truncated)?;
    Ok(Input::Shared(data.clone(), start..end))
}

#[allow(clippy::too_many_arguments)]
fn seq_frame(
    data: &Arc<[u8]>,
    seq: &Sequence,
    color: &mut dyn Av1Decoder,
    alpha: Option<&mut dyn Av1Decoder>,
    max_cll: u16,
    i: usize,
    scale: u32,
    parallel: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Decoded, Error> {
    let t = &seq.color;
    let sample = t.samples[i];
    let mono = t.config.is_some_and(|c| c.monochrome);
    let alpha_track = seq.alpha.as_ref().filter(|_| alpha.is_some());
    let layout = match (alpha_track.is_some(), mono) {
        (true, _) => Layout::Rgba,
        (false, true) => Layout::Gray,
        (false, false) => Layout::Rgb,
    };
    let ch = layout.channels();
    let s = scale as usize;
    let (cw, chh) = (t.width as usize, t.height as usize);
    let (out_w, out_h) = (cw.div_ceil(s), chh.div_ceil(s));
    let mut buf = vec![0u8; out_w * out_h * ch];
    if layout == Layout::Rgba {
        // Frames whose alpha sample is missing stay opaque.
        buf.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    let mut dst = Dst {
        data: &mut buf,
        width: out_w,
        height: out_h,
        channels: ch,
        wide: false,
        x: 0,
        y: 0,
    };
    let pic = decode_one(color, sample_input(data, &sample)?)?;
    let p = pic.planes();
    let region = Region {
        x: 0,
        y: 0,
        w: (p.width as usize).min(cw),
        h: (p.height as usize).min(chh),
        scale: s,
    };
    let conv = color_conv(t.nclx, p.cicp, p.bits, max_cll);
    convert::convert(&p, &conv, region, &mut dst, Target::Color, parallel, cancel)?;
    drop(pic);
    let mut premultiplied = false;
    if let (Some(at), Some(adec)) = (alpha_track, alpha)
        && let Some(asample) = at.samples.get(i)
    {
        let apic = decode_one(adec, sample_input(data, asample)?)?;
        let ap = apic.planes();
        let region = Region {
            x: 0,
            y: 0,
            w: (ap.width as usize).min(cw),
            h: (ap.height as usize).min(chh),
            scale: s,
        };
        let conv = Conv::alpha(ap.cicp.full_range, ap.bits);
        convert::convert(
            &ap,
            &conv,
            region,
            &mut dst,
            Target::Alpha,
            parallel,
            cancel,
        )?;
        premultiplied = at.prem.contains(&t.id) || t.prem.contains(&at.id);
    }
    if premultiplied {
        convert::unpremultiply(&mut buf, parallel);
    }
    let duration_ms = (u64::from(sample.duration) * 1000 + u64::from(t.timescale) / 2)
        / u64::from(t.timescale.max(1));
    Ok(Decoded {
        width: out_w as u32,
        height: out_h as u32,
        scale,
        layout,
        data: buf,
        duration_ms: u32::try_from(duration_ms).unwrap_or(u32::MAX),
    })
}
