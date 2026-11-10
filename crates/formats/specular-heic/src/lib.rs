//! HEIC / HEIF decoding for Specular.
//!
//! A direct per-tile driver over in-tree forks: [`heif`] (`oxideav-heif`,
//! the container), [`h265`] (`oxideav-h265`) and [`hevc`] (the heic-rs
//! HEVC core), each in its own sub-crate with its licence files. The HEVC
//! decoder sits behind the [`HevcCore`] boundary (`hvcC` + tile bytes →
//! planes), so it can be swapped; the container side checks every item's
//! SPS against its `ispe` and the limits before a core sees it. Items of
//! other codecs ([`Codec`]) decode through Specular's own decoders: AVC
//! (`avc1`) through `specular-h264`, VVC (`vvc1`) through `specular-vvc`,
//! JPEG (`jpeg`) through
//! `specular-jpeg`, and uncompressed items (`unci`, ISO/IEC 23001-17:
//! monochrome, YCbCr and RGB components of 1 to 16 bits, every
//! interleaving but multi-Y, deflate / zlib / Brotli generic compression)
//! here. Tiled images (`tili`, in-file tiles) and colour format
//! enhancement images (`cfen`) are composed like grids; `cexg` extents
//! decode as tiles. Image sequences play through [`seq::SeqPlayer`],
//! inter-coded ones included. The flow is parse → layout → decode tile(s)
//! → planes:
//!
//! * [`HeicFile::open`] parses the container and the primary item's
//!   derivation graph, and reports [`Info`] (size, layout, colour) without
//!   decoding anything.
//! * [`HeicFile::decode_tile`] decodes one coded tile of a grid (or the
//!   single coded image) to YCbCr planes; [`HeicFile::decode`] decodes
//!   and composes the whole output image (grid / overlay / identity
//!   derivations, `clap` / `irot` / `imir`, alpha), tiles in parallel on
//!   the current rayon pool when asked.
//! * [`Colour`] exposes `nclx`, ICC and VUI signalling unresolved, plus
//!   the YCbCr → RGB defaults; [`Image::to_rgba8`] / [`Image::to_rgba16`]
//!   are reference conversions.
//!
//! Damaged input is concealed where that is reasonable: a grid tile that
//! fails to decode is drawn neutral grey and reported in
//! [`Image::warnings`]; broken metadata never fails a decode. A core that
//! conceals inside the picture ([`HeicRs`]) reports how many CTUs it
//! concealed there too.
//!
//! **Not supported**, each refused by name: VVC (`vvc1`) pictures that
//! `specular-vvc` refuses (inter-coded items, 12- and 16-bit range-extension
//! tools), JPEG 2000 (`j2k1`), AV1 items without the
//! AVIF brand (the AVIF decoder reads those), HEVC tile (`hvt1`) and
//! layered (`lhv1`) items; 12-bit, CMYK and odd-sampled JPEG items; AVC
//! 4:4:4 and the other streams `specular-h264` refuses; `unci` block
//! packing, multi-Y interleaving, 4:1:1 sampling, signed / float / complex
//! samples and depth, disparity, palette, colour filter array (raw sensor)
//! and CMYK components; `tili` images with extra dimensions or with tiles
//! in separate files; packed `cfen` inputs and depth / K channels; `cexg`
//! with an `ExtentDecoderConfigurationRecord`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use heif::compose::{
    GridCanvas, OverlayInput, apply_transforms_owned, attach_alpha, composite_overlay,
};
use heif::meta::{ITEM_TYPE_HEV1, ITEM_TYPE_HVC1};
use heif::{Chroma, HeifFile, HeifFrame, ImageKind, ImageNode};

mod codec;
mod colour;
mod core;
pub mod planes;
pub mod rgba;
pub mod seq;
pub mod sps;
mod unci;

pub use crate::codec::Codec;
pub use crate::core::{
    BatchSink, CoreError, CoreInput, CoreOutput, DEFAULT_CORE, Fallback, HeicRs, HevcCore, Oxideav,
    decode_each, pack_frame,
};
pub use colour::{Cicp, Colour, VuiColour, Ycbcr};
pub use h265;
pub use heif;
pub use hevc;
pub use rgba::{ConvertOptions, Rgba};

/// Most coded items decoded for one image (grid tiles, alpha, inputs).
pub const MAX_ITEM_DECODES: usize = 4096;
/// Largest output image accepted, in pixels (libheif's default limit).
pub const MAX_PIXELS: u64 = 32768 * 32768;

/// Decode errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Container parse / composition error.
    #[error(transparent)]
    Heif(#[from] heif::HeifError),
    /// HEVC bitstream error of one coded item.
    #[error("item {item}: {source}")]
    Hevc {
        /// The coded item.
        item: u32,
        /// The core's error.
        source: CoreError,
    },
    /// The file uses something this decoder does not handle.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// A size / count limit was exceeded.
    #[error("limit exceeded: {0}")]
    Limit(String),
    /// Under the limits, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    /// The cancellation flag was raised.
    #[error("cancelled")]
    Cancelled,
}

/// Result alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// `true` when `head` looks like a HEIF file with an HEVC brand.
pub fn probe(head: &[u8]) -> bool {
    if head.len() < 12 || &head[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
    let end = size.clamp(16, head.len());
    let brands = std::iter::once(&head[8..12]).chain(head[16.min(end)..end].chunks_exact(4));
    brands
        .into_iter()
        .any(|b| matches!(b, b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx"))
}

/// How the primary image is assembled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// One coded HEVC item.
    Single,
    /// A `grid` of coded HEVC tiles, row-major.
    Grid {
        /// Tile rows.
        rows: u32,
        /// Tile columns.
        columns: u32,
        /// Width of the first tile.
        tile_width: u32,
        /// Height of the first tile.
        tile_height: u32,
    },
    /// Any other derivation (overlay, identity chains, tone map, …),
    /// composed by [`HeicFile::decode`] only.
    Derived,
}

/// What [`HeicFile::open`] learns without decoding.
#[derive(Clone, Debug)]
pub struct Info {
    /// Coding of the first coded item (a grid's first tile).
    pub codec: Codec,
    /// Output width (after `clap` / `irot` / `iscl`).
    pub width: u32,
    /// Output height.
    pub height: u32,
    /// Luma bit depth of the coded tiles (`hvcC`, `avcC`, the JPEG frame header, `uncC`).
    pub bit_depth: u8,
    /// Chroma format of the coded tiles (4:4:4 for RGB-coded items, see [`Info::rgb`]).
    pub chroma: Chroma,
    /// The coded tiles hold R, G and B samples (an RGB JPEG or `unci` item), not YCbCr.
    pub rgb: bool,
    /// The image has an alpha auxiliary (or alpha samples of its own).
    pub has_alpha: bool,
    /// The alpha is pre-multiplied (`prem`).
    pub premultiplied_alpha: bool,
    /// Assembly of the primary image.
    pub layout: Layout,
    /// Container colour signalling (no VUI yet: see [`Image::colour`]).
    pub colour: Colour,
    /// A `tmap` gain map references the primary image (see [`HeicFile::gain_map`]).
    pub gain_map: bool,
}

/// Apple's HDR gain-map auxiliary image (iOS 14.1 to 17).
pub const APPLE_GAIN_MAP_URN: &str = "urn:com:apple:photo:2020:aux:hdrgainmap";

/// A gain map of the primary image (see [`HeicFile::gain_map`]).
#[derive(Clone, Debug)]
pub struct GainMapItem {
    /// The map image item (coded or a grid), for [`HeicFile::decode_image`].
    pub map: u32,
    pub kind: GainMapKind,
}

#[derive(Clone, Debug)]
pub enum GainMapKind {
    /// An ISO 21496-1 `tmap` derived item.
    Iso {
        /// The `tmap` item body: a version byte, then the ISO 21496-1 metadata.
        metadata: Vec<u8>,
        /// The `tmap` item's colour properties: the alternate (HDR) rendition's space.
        alternate: Colour,
    },
    /// An Apple `hdrgainmap` auxiliary; its parameters are in the map's XMP
    /// ([`HeicFile::item_xmp`]) and the primary's Exif maker notes.
    Apple,
}

/// Decode options.
#[derive(Clone, Copy, Debug, Default)]
pub struct DecodeOptions<'a> {
    /// Decode grid tiles in parallel on the current rayon pool, and give
    /// single-picture items an in-picture worker budget.
    pub parallel: bool,
    /// Cooperative cancellation, polled per item / tile and inside the
    /// core.
    pub cancel: Option<&'a AtomicBool>,
    /// The HEVC core; `None`: [`DEFAULT_CORE`] (heic-rs, falling back
    /// to Oxideav for pictures it does not support).
    pub core: Option<&'a dyn HevcCore>,
}

impl DecodeOptions<'_> {
    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c.load(Ordering::Relaxed))
    }
}

/// One decoded coded picture (a tile or a single image), YCbCr planes,
/// before any transformative property.
#[derive(Clone, Debug)]
pub struct Tile {
    /// The planes: 8-bit samples as bytes, deeper ones as little-endian
    /// `u16` (see [`HeifFrame`]).
    pub frame: HeifFrame,
    /// The SPS VUI colour signalling of the picture.
    pub vui: Option<VuiColour>,
    /// CTUs the core could not decode and concealed (see
    /// [`CoreOutput::concealed_ctus`]); the picture is damaged when this
    /// is not zero.
    pub concealed_ctus: usize,
}

/// A decoded, composed output image.
#[derive(Clone, Debug)]
pub struct Image {
    /// The YCbCr planes (+ alpha) of the output image.
    pub frame: HeifFrame,
    /// Colour signalling (container and VUI).
    pub colour: Colour,
    /// The alpha plane is pre-multiplied.
    pub premultiplied_alpha: bool,
    /// Non-fatal problems: concealed tiles and the like.
    pub warnings: Vec<String>,
}

impl Image {
    /// Output width.
    pub fn width(&self) -> u32 {
        self.frame.width
    }

    /// Output height.
    pub fn height(&self) -> u32 {
        self.frame.height
    }

    /// Reference conversion to 8-bit RGBA with [`Colour::ycbcr`]
    /// (chroma replicated, no dithering; opaque without alpha).
    pub fn to_rgba8(&self) -> Result<Vec<u8>> {
        let colr = self.colour.kernel_colr();
        Ok(heif::rgb::planes_to_rgb8(
            self.frame.width,
            self.frame.height,
            self.frame.format,
            &self.frame.planes,
            Some(&colr),
            true,
        )?)
    }

    /// Reference conversion to RGB(A) at the image's bit depth.
    pub fn to_rgba16(&self) -> Result<heif::RgbImage16> {
        Ok(heif::rgb::to_rgb(
            &self.frame,
            Some(&self.colour.kernel_colr()),
        )?)
    }
}

/// A parsed HEIC file.
pub struct HeicFile {
    file: HeifFile<Arc<[u8]>>,
    root: ImageNode,
    info: Info,
    /// The top-level images, primary first ([`HeicFile::page_count`]), and the one shown.
    pages: Arc<[u32]>,
    page: u32,
}

impl std::fmt::Debug for HeicFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeicFile")
            .field("info", &self.info)
            .finish()
    }
}

impl HeicFile {
    /// Parse the container and the primary item's derivation graph.
    pub fn open(data: impl Into<Arc<[u8]>>) -> Result<Self> {
        let file = HeifFile::from_data(data.into())?;
        let primary = primary_id(&file)?;
        let root = heif::build_graph(&file, primary)?;
        let info = info_of(&file, &root)?;
        let pages = top_level(&file, primary).into();
        Ok(Self {
            file,
            root,
            info,
            pages,
            page: 0,
        })
    }

    /// The images a viewer pages through: the primary, then the further top-level images of
    /// a collection in item order (MIAF's display order: not hidden, not a thumbnail or
    /// auxiliary image, not an input of a derived image such as a grid tile or a gain map,
    /// one image per `altr` group), each only when it opens; 1 for a single image.
    pub fn page_count(&self) -> u32 {
        self.pages.len().max(1) as u32
    }

    /// The page this file shows ([`HeicFile::reopen_page`]).
    pub fn page(&self) -> u32 {
        self.page
    }

    /// Page `page` of the same file, sharing the parsed container and bytes: that image's
    /// own derivation graph, alpha, gain map and properties.
    pub fn reopen_page(&self, page: u32) -> Result<Self> {
        let id = *self
            .pages
            .get(page as usize)
            .ok_or_else(|| Error::Limit(format!("page {page} out of range")))?;
        let root = heif::build_graph(&self.file, id)?;
        let info = info_of(&self.file, &root)?;
        Ok(Self {
            file: self.file.clone(),
            root,
            info,
            pages: self.pages.clone(),
            page,
        })
    }

    /// What the container says about the primary image.
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// The parsed container (items, properties, raw payloads).
    pub fn container(&self) -> &HeifFile<Arc<[u8]>> {
        &self.file
    }

    /// The primary item's derivation graph.
    pub fn graph(&self) -> &ImageNode {
        &self.root
    }

    /// Number of coded tiles [`HeicFile::decode_tile`] addresses: the
    /// grid's tiles, else 1 for a single coded image (0 for other
    /// derivations).
    pub fn tile_count(&self) -> usize {
        match self.info.layout {
            Layout::Single => 1,
            Layout::Grid { .. } => self.root.inputs.len(),
            Layout::Derived => 0,
        }
    }

    /// Decode one coded tile (row-major grid index; 0 for a single
    /// image) to its planes, cropped to the tile's `ispe`, without the
    /// tile's transformative properties.
    pub fn decode_tile(&self, index: usize, opts: &DecodeOptions<'_>) -> Result<Tile> {
        let node = match self.info.layout {
            Layout::Single if index == 0 => &self.root,
            Layout::Grid { .. } => self
                .root
                .inputs
                .get(index)
                .ok_or_else(|| Error::Limit(format!("tile {index} out of range")))?,
            _ => {
                return Err(Error::Unsupported(format!(
                    "tile {index} of a derived image"
                )));
            }
        };
        let threads = if opts.parallel && self.info.layout == Layout::Single {
            rayon::current_num_threads()
        } else {
            1
        };
        decode_coded(&self.file, node, None, threads, opts)
    }

    /// Decode any coded HEVC item of the file by id (a tile, the alpha
    /// plane, a thumbnail, …) to its planes, cropped to its `ispe`.
    pub fn decode_item(&self, id: u32, opts: &DecodeOptions<'_>) -> Result<Tile> {
        let node = heif::build_graph(&self.file, id)?;
        let threads = if opts.parallel {
            rayon::current_num_threads()
        } else {
            1
        };
        decode_coded(&self.file, &node, None, threads, opts)
    }

    /// The largest thumbnail of the primary image (an image item with a
    /// `thmb` reference to it): its item id and output size. `None` when
    /// there is none (or none whose size can be worked out).
    pub fn thumbnail(&self) -> Option<(u32, u32, u32)> {
        let meta = self.file.meta().ok()?;
        let master = self.root.item.id;
        meta.items
            .iter()
            .filter(|it| it.id != master)
            .filter(|it| {
                meta.references_from(it.id, &heif::meta::reference::THMB)
                    .contains(&master)
            })
            .filter_map(|it| {
                let node = heif::build_graph(&self.file, it.id).ok()?;
                let (w, h) = node.output_size().ok()?;
                (w > 0 && h > 0 && first_coded(&node).is_some()).then_some((it.id, w, h))
            })
            .max_by_key(|&(_, w, h)| u64::from(w) * u64::from(h))
    }

    /// Decode and compose the thumbnail item `id` (see
    /// [`HeicFile::thumbnail`]) like the primary image: transformative
    /// properties and alpha applied, its own colour signalling.
    pub fn decode_thumbnail(&self, id: u32, opts: &DecodeOptions<'_>) -> Result<Image> {
        self.decode_image(id, opts)
    }

    /// Decode and compose any image item `id` (a thumbnail, a gain map, …) like the primary
    /// image: grids assembled, transformative properties and alpha applied, its own colour
    /// signalling.
    pub fn decode_image(&self, id: u32, opts: &DecodeOptions<'_>) -> Result<Image> {
        let node = heif::build_graph(&self.file, id)?;
        let (w, h) = node.output_size()?;
        if u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err(Error::Limit(format!("{w}x{h} image {id}")));
        }
        let colour = Colour::from_props(&colour_node(&node).properties);
        let prem = node.premultiplied_alpha && node.alpha.is_some();
        decode_root(&self.file, &node, colour, prem, opts)
    }

    /// Decode and compose the primary output image.
    pub fn decode(&self, opts: &DecodeOptions<'_>) -> Result<Image> {
        decode_root(
            &self.file,
            &self.root,
            self.info.colour.clone(),
            self.info.premultiplied_alpha,
            opts,
        )
    }

    /// The Exif payload (from the TIFF header on) of the primary image;
    /// `None` when absent or unreadable (never an error).
    pub fn exif(&self) -> Option<Vec<u8>> {
        self.root.metadata.iter().find_map(|m| {
            let body = match m.item_type {
                heif::meta::ITEM_TYPE_EXIF => self.file.item_data_owned(m.id).ok()?,
                heif::meta::ITEM_TYPE_DEXF => inflate(&self.file.item_data(m.id).ok()?)?,
                _ => return None,
            };
            exif_payload(&body)
        })
    }

    /// The primary image's gain map: an ISO 21496-1 `tmap` item deriving from it (or the
    /// primary itself being one), else an Apple `hdrgainmap` auxiliary. `None` when there is
    /// none or it is malformed.
    pub fn gain_map(&self) -> Option<GainMapItem> {
        let meta = self.file.meta().ok()?;
        let tmap = if matches!(self.root.kind, ImageKind::ToneMap(_)) {
            Some(self.root.item.id)
        } else {
            meta.items
                .iter()
                .filter(|it| it.item_type == heif::meta::ITEM_TYPE_TMAP)
                .find(|it| meta.derivation_inputs(it.id).first() == Some(&self.root.item.id))
                .map(|it| it.id)
        };
        if let Some(t) = tmap {
            let inputs = meta.derivation_inputs(t);
            let map = *inputs.get(1)?;
            let metadata = self.file.item_data_owned(t).ok()?;
            let alternate = if t == self.root.item.id {
                Colour::from_props(&self.root.properties)
            } else {
                Colour::from_props(&heif::build_graph(&self.file, t).ok()?.properties)
            };
            return Some(GainMapItem {
                map,
                kind: GainMapKind::Iso {
                    metadata,
                    alternate,
                },
            });
        }
        let base = colour_node(&self.root);
        base.other_auxiliaries
            .iter()
            .find(|a| {
                a.properties
                    .auxc()
                    .is_some_and(|c| c.aux_type.trim_end_matches('\0') == APPLE_GAIN_MAP_URN)
            })
            .map(|a| GainMapItem {
                map: a.item.id,
                kind: GainMapKind::Apple,
            })
    }

    /// The XMP packet describing image item `id` (a `cdsc` metadata item), when present.
    pub fn item_xmp(&self, id: u32) -> Option<Vec<u8>> {
        let node = heif::build_graph(&self.file, id).ok()?;
        xmp_of(&self.file, &node)
    }

    /// The XMP packet of the primary image, when present and readable.
    pub fn xmp(&self) -> Option<Vec<u8>> {
        xmp_of(&self.file, &self.root)
    }
}

fn xmp_of(file: &HeifFile<Arc<[u8]>>, node: &ImageNode) -> Option<Vec<u8>> {
    node.metadata.iter().filter(|m| m.is_xmp()).find_map(|m| {
        let body = file.item_data(m.id).ok()?;
        let deflated = m
            .content_encoding
            .as_deref()
            .is_some_and(|e| e.eq_ignore_ascii_case("deflate"));
        if deflated {
            inflate(&body)
        } else {
            Some(body.into_owned())
        }
    })
}

/// Strip the `exif_tiff_header_offset` word of an `Exif` item body (HEIF
/// A.2.1). Lenient like libheif: a bad offset falls back to searching
/// for the TIFF header (`II*\0` / `MM\0*`) in the body.
pub fn exif_payload(item: &[u8]) -> Option<Vec<u8>> {
    let is_tiff = |b: &[u8]| b.starts_with(b"II*\0") || b.starts_with(b"MM\0*");
    if item.len() >= 4 {
        let off = u32::from_be_bytes([item[0], item[1], item[2], item[3]]) as usize;
        if let Some(rest) = 4usize.checked_add(off).and_then(|s| item.get(s..))
            && is_tiff(rest)
        {
            return Some(rest.to_vec());
        }
    }
    (0..item.len().saturating_sub(4))
        .find(|&i| is_tiff(&item[i..]))
        .map(|i| item[i..].to_vec())
}

/// What the container alone says about the primary image's coding, for files
/// [`HeicFile::open`] rejects (another codec, a derivation it does not compose): no bytes
/// are copied and no pixels decoded. `None` when the item graph does not parse.
pub fn header_coding(data: &[u8]) -> Option<HeaderCoding> {
    let file = HeifFile::from_data(data).ok()?;
    let root = heif::build_graph(&file, primary_id(&file).ok()?).ok()?;
    let format = first_coded(&root).and_then(|n| codec::format_of(&file, &n).ok());
    let colour = Colour::from_props(&colour_node(&root).properties);
    Some(HeaderCoding {
        codec: format.map(|f| f.codec),
        chroma: format.map(|f| f.chroma),
        matrix: colour.nclx.map(|n| n.matrix),
        has_alpha: root.alpha.is_some(),
        premultiplied_alpha: root.premultiplied_alpha && root.alpha.is_some(),
    })
}

/// See [`header_coding`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderCoding {
    /// Coding of the first coded image, when it is one this crate decodes.
    pub codec: Option<Codec>,
    /// Chroma format of the first coded image, from its decoder configuration.
    pub chroma: Option<Chroma>,
    /// CICP matrix coefficients of the `nclx` colour property.
    pub matrix: Option<u16>,
    /// The image has an alpha auxiliary.
    pub has_alpha: bool,
    /// The alpha is pre-multiplied (`prem`).
    pub premultiplied_alpha: bool,
}

fn inflate(body: &[u8]) -> Option<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_with_limit(body, heif::file::MAX_ITEM_BYTES as usize)
        .ok()
}

/// What the container says about the image `root`; an error when it cannot be shown.
fn info_of(file: &HeifFile<Arc<[u8]>>, root: &ImageNode) -> Result<Info> {
    let (width, height) = root.output_size()?;
    if width == 0 || height == 0 {
        return Err(heif::HeifError::invalid("zero-sized image").into());
    }
    if width as u64 * height as u64 > MAX_PIXELS {
        return Err(Error::Limit(format!("{width}x{height} image")));
    }
    let first = first_coded(root).ok_or_else(|| {
        Error::Unsupported(format!(
            "item {}: no coded image in its derivation",
            root.item.id
        ))
    })?;
    let mut format = codec::format_of(file, &first)?;
    if let ImageKind::ColourFormatEnhancement(c) = &root.kind {
        // The layout the channels compose to, not the first channel's.
        let inputs = root
            .inputs
            .iter()
            .map(|i| Ok((i.output_size()?, format.bit_depth)))
            .collect::<Result<Vec<_>>>()?;
        let (fmt, _, _) =
            heif::compose::plan_colour_format_enhancement(c, &inputs, root.properties.nclx())?;
        format.chroma = fmt.chroma;
        format.alpha = fmt.has_alpha;
    }
    let layout = match &root.kind {
        ImageKind::Coded(_) => Layout::Single,
        ImageKind::Grid(g) => {
            let (tw, th) = root
                .inputs
                .first()
                .and_then(ImageNode::ispe)
                .unwrap_or((0, 0));
            Layout::Grid {
                rows: g.rows as u32,
                columns: g.columns as u32,
                tile_width: tw,
                tile_height: th,
            }
        }
        _ => Layout::Derived,
    };
    let meta = file.meta()?;
    let gain_map = matches!(root.kind, ImageKind::ToneMap(_))
        || meta.items.iter().any(|it| {
            it.item_type == heif::meta::ITEM_TYPE_TMAP
                && meta.derivation_inputs(it.id).first() == Some(&root.item.id)
        });
    let mut colour = Colour::from_props(&colour_node(root).properties);
    if format.rgb {
        colour.mark_rgb();
    }
    Ok(Info {
        codec: format.codec,
        width,
        height,
        bit_depth: format.bit_depth,
        chroma: format.chroma,
        rgb: format.rgb,
        has_alpha: root.alpha.is_some() || format.alpha,
        premultiplied_alpha: root.premultiplied_alpha && root.alpha.is_some(),
        layout,
        colour,
        gain_map,
    })
}

/// The top-level images of a collection, `primary` first (see [`HeicFile::page_count`]).
fn top_level(file: &HeifFile<Arc<[u8]>>, primary: u32) -> Vec<u32> {
    let mut out = vec![primary];
    let Ok(meta) = file.meta() else {
        return out;
    };
    // Every derivation input, gathered once: scanning the items and their
    // references per entry was cubic in a hostile item count.
    let items: std::collections::HashSet<u32> = meta.items.iter().map(|it| it.id).collect();
    let inputs: std::collections::HashSet<u32> = meta
        .references
        .iter()
        .filter(|r| {
            r.reference_type == heif::meta::reference::DIMG && items.contains(&r.from_item_id)
        })
        .flat_map(|r| r.to_item_ids.iter().copied())
        .collect();
    let input = |id: u32| inputs.contains(&id);
    for entry in meta.display_order() {
        if entry.contains(&primary) {
            continue;
        }
        // The first alternative that opens.
        let shown = entry.into_iter().find(|&id| {
            meta.item(id)
                .is_some_and(|it| it.item_type != heif::meta::ITEM_TYPE_TMAP)
                && !input(id)
                && heif::build_graph(file, id).is_ok_and(|r| info_of(file, &r).is_ok())
        });
        out.extend(shown);
    }
    out
}

/// The primary item; when `pitm` names a thumbnail (a `thmb` reference
/// to another image, MPEG conformance C005), its master image instead,
/// like libheif.
fn primary_id<D: AsRef<[u8]>>(file: &HeifFile<D>) -> Result<u32> {
    let id = file.primary_item()?.id;
    let meta = file.meta()?;
    Ok(meta
        .references_from(id, &heif::meta::reference::THMB)
        .into_iter()
        .find(|&m| m != id && meta.item(m).is_some())
        .unwrap_or(id))
}

/// The node whose colour properties describe the output: a `tmap`
/// (gain maps are not applied) shows its base image.
fn colour_node(root: &ImageNode) -> &ImageNode {
    match (&root.kind, root.inputs.first()) {
        (ImageKind::ToneMap(_), Some(base)) => base,
        _ => root,
    }
}

/// Decode and compose the output image of `root`.
fn decode_root(
    file: &HeifFile<Arc<[u8]>>,
    root: &ImageNode,
    mut colour: Colour,
    premultiplied_alpha: bool,
    opts: &DecodeOptions<'_>,
) -> Result<Image> {
    let mut uses = HashMap::new();
    count_coded_uses(root, &mut uses);
    let mut s = Session {
        file,
        opts,
        cache: HashMap::new(),
        uses,
        decodes: 0,
        warnings: Vec::new(),
        vui: None,
    };
    let frame = s.output(root)?;
    colour.vui = s.vui;
    if first_coded(root).is_some_and(|n| codec::format_of(file, &n).is_ok_and(|f| f.rgb)) {
        colour.mark_rgb();
    }
    Ok(Image {
        frame,
        colour,
        premultiplied_alpha,
        warnings: s.warnings,
    })
}

/// The first coded picture of `node`'s derivation: a coded item, or a `tili` item's tile.
fn first_coded(node: &ImageNode) -> Option<std::borrow::Cow<'_, ImageNode>> {
    match &node.kind {
        ImageKind::Coded(_) => Some(std::borrow::Cow::Borrowed(node)),
        ImageKind::Tiled(t) => tile_node(node, t).ok().map(std::borrow::Cow::Owned),
        _ => node.inputs.iter().find_map(first_coded),
    }
}

/// The coded item every tile of `tili` item `node` is: the `tilC` tile item type with the
/// tile properties, sized one tile.
fn tile_node(node: &ImageNode, t: &heif::derived::TiledItem) -> Result<ImageNode> {
    let id = node.item.id;
    if !t.config.extra_dimensions.is_empty() {
        return Err(Error::Unsupported(format!(
            "item {id}: tiled image with {} extra dimensions",
            t.config.extra_dimensions.len()
        )));
    }
    let (tile_type, _) = t.config.in_file_tiles.as_ref().ok_or_else(|| {
        Error::Unsupported(format!(
            "item {id}: tiled image whose tiles are separate files"
        ))
    })?;
    if t.config.tile_width == 0 || t.config.tile_height == 0 {
        return Err(heif::HeifError::invalid(format!("item {id}: zero-sized tiles")).into());
    }
    Ok(ImageNode::new(
        node.item.clone(),
        ImageKind::Coded(*tile_type),
        with_ispe(
            &t.tile_properties,
            heif::Ispe::new(t.config.tile_width, t.config.tile_height),
        ),
        Vec::new(),
        None,
        None,
        Vec::new(),
        false,
        Vec::new(),
        Vec::new(),
        node.depth_in_chain,
    ))
}

fn hevc_config(node: &ImageNode) -> Result<&heif::HevcConfig> {
    match node.kind {
        ImageKind::Coded(ITEM_TYPE_HVC1 | ITEM_TYPE_HEV1) => {
            node.properties.hvcc().ok_or_else(|| {
                heif::HeifError::invalid(format!("item {}: hvc1 item without hvcC", node.item.id))
                    .into()
            })
        }
        _ => codec::codec_of(node).and_then(|c| {
            Err(heif::HeifError::invalid(format!(
                "item {}: {} item where HEVC was expected",
                node.item.id,
                c.name()
            ))
            .into())
        }),
    }
}

/// Decode a coded item (or one extent / tile span of it: `payload`).
fn decode_coded(
    file: &HeifFile<Arc<[u8]>>,
    node: &ImageNode,
    payload: Option<&[u8]>,
    threads: usize,
    opts: &DecodeOptions<'_>,
) -> Result<Tile> {
    if opts.cancelled() {
        return Err(Error::Cancelled);
    }
    if codec::codec_of(node)? != Codec::Hevc {
        return codec::decode(file, node, payload, threads, opts);
    }
    let c = prepare_coded(file, node, payload)?;
    // The core's picture and the cropped, packed output: an SPS of up to
    // MAX_PIXELS from a header-only item is charged before it is allocated.
    let _picture = specular_core::budget::reserve(c.picture_bytes.saturating_mul(2))?;
    let core: &dyn HevcCore = opts.core.unwrap_or(&DEFAULT_CORE);
    let mut out = core
        .decode(&c.input(threads, opts))
        .map_err(|e| c.error(e))?;
    out.frame.format.siting = c.siting;
    Ok(Tile {
        frame: out.frame,
        vui: out.vui,
        concealed_ctus: out.concealed_ctus,
    })
}

/// A coded picture checked and ready for a core.
struct Coded<'a> {
    id: u32,
    hvcc: &'a [u8],
    data: std::borrow::Cow<'a, [u8]>,
    ispe: Option<(u32, u32)>,
    /// One decoded picture's bytes, from the largest SPS.
    picture_bytes: u64,
    /// Where the SPS VUI sites subsampled chroma (H.273 location 0 when it does not say).
    siting: heif::ChromaSiting,
}

impl Coded<'_> {
    fn input<'s>(&'s self, threads: usize, opts: &DecodeOptions<'s>) -> CoreInput<'s> {
        CoreInput {
            hvcc: self.hvcc,
            data: &self.data,
            crop: self.ispe,
            threads,
            cancel: opts.cancel,
        }
    }

    fn error(&self, e: CoreError) -> Error {
        match e {
            CoreError::Cancelled => Error::Cancelled,
            source => Error::Hevc {
                item: self.id,
                source,
            },
        }
    }
}

/// The `hvcC`, payload and `ispe` of a coded item (or of one extent / tile
/// span of it: `payload`), checked against the SPS and the limits.
fn prepare_coded<'a>(
    file: &'a HeifFile<Arc<[u8]>>,
    node: &'a ImageNode,
    payload: Option<&'a [u8]>,
) -> Result<Coded<'a>> {
    let id = node.item.id;
    let cfg = hevc_config(node)?;
    let data = match payload {
        Some(p) => std::borrow::Cow::Borrowed(p),
        None => file.item_data(id)?,
    };
    if data.is_empty() {
        return Err(heif::HeifError::invalid(format!("item {id}: empty payload")).into());
    }
    let ispe = node.ispe();
    let picture_bytes = sps::check(cfg, &data, ispe, MAX_PIXELS)
        .map_err(|m| heif::HeifError::invalid(format!("item {id}: {m}")))?;
    Ok(Coded {
        id,
        hvcc: &cfg.raw,
        data,
        ispe,
        picture_bytes,
        siting: sps::chroma_loc(cfg).map_or_else(Default::default, heif::ChromaSiting::from_h273),
    })
}

fn count_coded_uses(node: &ImageNode, uses: &mut HashMap<u32, usize>) {
    if let ImageKind::Coded(_) = node.kind {
        *uses.entry(node.item.id).or_insert(0) += 1;
    }
    for i in &node.inputs {
        count_coded_uses(i, uses);
    }
    if let Some(a) = &node.alpha {
        count_coded_uses(a, uses);
    }
}

/// One whole-image decode: coded items used more than once are cached
/// until their last use.
struct Session<'a, 'o> {
    file: &'a HeifFile<Arc<[u8]>>,
    opts: &'a DecodeOptions<'o>,
    cache: HashMap<u32, HeifFrame>,
    uses: HashMap<u32, usize>,
    decodes: usize,
    warnings: Vec<String>,
    /// VUI of the first colour picture decoded.
    vui: Option<VuiColour>,
}

impl Session<'_, '_> {
    fn count_decode(&mut self, n: usize) -> Result<()> {
        if self.decodes + n > MAX_ITEM_DECODES {
            return Err(Error::Limit(format!(
                "more than {MAX_ITEM_DECODES} coded items in one image"
            )));
        }
        self.decodes += n;
        Ok(())
    }

    fn note_vui(&mut self, node: &ImageNode, vui: Option<VuiColour>) {
        if self.vui.is_none() && node.depth_in_chain <= 1 && !is_aux(node) {
            self.vui = vui;
        }
    }

    fn coded(&mut self, node: &ImageNode) -> Result<HeifFrame> {
        let id = node.item.id;
        let left = self.uses.get(&id).copied().unwrap_or(1).saturating_sub(1);
        self.uses.insert(id, left);
        if let Some(f) = self.cache.remove(&id) {
            if left > 0 {
                self.cache.insert(id, f.clone());
            }
            return Ok(f);
        }
        let f = match node.properties.cexg() {
            Some(cexg) if cexg.tile_count() > 1 => self.constrained_extents(node, cexg)?,
            _ => {
                self.count_decode(1)?;
                let threads = if self.opts.parallel {
                    rayon::current_num_threads()
                } else {
                    1
                };
                let t = decode_coded(self.file, node, None, threads, self.opts)?;
                self.note_vui(node, t.vui);
                if t.concealed_ctus > 0 {
                    self.warnings
                        .push(format!("item {id}: {} CTUs concealed", t.concealed_ctus));
                }
                t.frame
            }
        };
        if left > 0 {
            self.cache.insert(id, f.clone());
        }
        Ok(f)
    }

    fn reconstruct(&mut self, node: &ImageNode) -> Result<HeifFrame> {
        match &node.kind {
            ImageKind::Coded(_) => self.coded(node),
            ImageKind::Grid(g) => {
                // The canvas: up to four planes of 16-bit samples.
                let _canvas = specular_core::budget::reserve_n(
                    u64::from(g.output_width) * u64::from(g.output_height),
                    8,
                )?;
                let mut canvas = GridCanvas::new(g);
                let plain = node.inputs.iter().all(|t| {
                    matches!(t.kind, ImageKind::Coded(_))
                        && t.alpha.is_none()
                        && t.properties.transformative().next().is_none()
                        && unsupported_essential(t).is_empty()
                        && self.uses.get(&t.item.id).copied().unwrap_or(1) == 1
                });
                if plain {
                    let jobs: Vec<(usize, &ImageNode, Option<&[u8]>)> = node
                        .inputs
                        .iter()
                        .enumerate()
                        .map(|(i, t)| (i, t, None))
                        .collect();
                    self.run_jobs(&jobs, &mut canvas)?;
                } else {
                    for (i, t) in node.inputs.iter().enumerate() {
                        let tile = self.output(t)?;
                        canvas.place(i, &tile)?;
                    }
                }
                Ok(canvas.finish()?)
            }
            ImageKind::Overlay(o) => {
                // The working canvas (three colour planes and opacity, u64 each) and the output
                // (up to four planes of 16-bit samples), before any input is decoded.
                let _canvas = specular_core::budget::reserve_n(
                    u64::from(o.output_width) * u64::from(o.output_height),
                    4 * 8 + 8,
                )?;
                // Every input is held until compositing: each is charged to the budget, so
                // 65 536 inputs of one coded item cannot pile up past it.
                let mut frames = Vec::with_capacity(node.inputs.len().min(64));
                let mut held = Vec::with_capacity(node.inputs.len().min(64));
                for i in &node.inputs {
                    let f = self.output(i)?;
                    let bytes: usize = f.planes.iter().map(|p| p.data.len()).sum();
                    held.push(specular_core::budget::reserve(bytes as u64)?);
                    frames.push((f, i.premultiplied_alpha && i.alpha.is_some()));
                }
                let inputs: Vec<OverlayInput<'_>> = frames
                    .iter()
                    .map(|(f, prem)| OverlayInput::new(f, *prem))
                    .collect();
                Ok(composite_overlay(o, &inputs, node.properties.nclx())?)
            }
            ImageKind::Identity => {
                let input = node.inputs.first().ok_or_else(|| {
                    heif::HeifError::invalid(format!("iden item {} has no input", node.item.id))
                })?;
                self.output(input)
            }
            // Gain maps are not applied: the base rendition.
            ImageKind::ToneMap(_) => {
                let base = node.inputs.first().ok_or_else(|| {
                    heif::HeifError::invalid(format!("tmap item {} has no input", node.item.id))
                })?;
                self.output(base)
            }
            ImageKind::Tiled(t) => self.tiled(node, t),
            ImageKind::ColourFormatEnhancement(c) => {
                // Every input is held until composed, charged to the budget like an overlay's.
                let mut frames = Vec::with_capacity(node.inputs.len().min(64));
                let mut held = Vec::with_capacity(node.inputs.len().min(64));
                for i in &node.inputs {
                    let f = self.output(i)?;
                    let bytes: usize = f.planes.iter().map(|p| p.data.len()).sum();
                    held.push(specular_core::budget::reserve(bytes as u64)?);
                    frames.push(f);
                }
                Ok(heif::compose::composite_colour_format_enhancement(
                    c,
                    &frames,
                    node.properties.nclx(),
                )?)
            }
        }
    }

    /// A `tili` tiled image (HEIF Amd 2:2026 §6.11) whose tiles are in this file: the
    /// `deti` offset table locates each tile in the item's data; empty tiles are grey.
    fn tiled(&mut self, node: &ImageNode, t: &heif::derived::TiledItem) -> Result<HeifFrame> {
        let id = node.item.id;
        let (w, h) = node.reconstructed_size()?;
        let tile = tile_node(node, t)?;
        let (cols, rows) = t
            .config
            .tile_grid(w, h)
            .ok_or_else(|| heif::HeifError::invalid(format!("item {id}: zero-sized tiles")))?;
        if cols > u32::from(u16::MAX) || rows > u32::from(u16::MAX) {
            return Err(Error::Limit(format!("item {id}: {cols}x{rows} tiles")));
        }
        let count = u64::from(cols) * u64::from(rows);
        let meta = self.file.meta()?;
        let loc = meta
            .location(id)
            .ok_or_else(|| heif::HeifError::invalid(format!("item {id} has no iloc entry")))?;
        let entry = loc
            .data_reference_index
            .checked_sub(1)
            .and_then(|i| meta.data_references.get(usize::from(i)))
            .filter(|d| &d.entry_type == b"deti")
            .ok_or_else(|| {
                heif::HeifError::invalid(format!("item {id}: tiled image without a deti entry"))
            })?;
        if !entry.self_contained {
            return Err(Error::Unsupported(format!(
                "item {id}: tiled image whose tiles are separate files"
            )));
        }
        let deti = heif::tiled::DataEntryTiledItem::parse(entry)?;
        if deti.external.is_some() {
            return Err(Error::Unsupported(format!(
                "item {id}: tiled image whose tiles are separate files"
            )));
        }
        let data = self.file.item_data(id)?;
        let spans = deti.tile_spans(&data, count)?;
        let _canvas = specular_core::budget::reserve_n(u64::from(w) * u64::from(h), 8)?;
        let desc = heif::GridDescriptor::new(rows as u16, cols as u16, w, h);
        let mut canvas = GridCanvas::new(&desc);
        let jobs: Vec<(usize, &ImageNode, Option<&[u8]>)> = spans
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let (o, l) = (*s)?;
                Some((i, &tile, data.get(o as usize..(o + l) as usize)))
            })
            .collect();
        if jobs.is_empty() {
            return Err(heif::HeifError::invalid(format!("item {id}: every tile is empty")).into());
        }
        self.run_jobs(&jobs, &mut canvas)?;
        for (i, s) in spans.iter().enumerate() {
            if s.is_none() {
                canvas.place_blank(i)?;
            }
        }
        Ok(canvas.finish()?)
    }

    /// Decode coded pictures into `canvas`: HEVC through the core's batch
    /// ([`Session::run_tiles`]), other codecs on the rayon pool.
    fn run_jobs(
        &mut self,
        jobs: &[(usize, &ImageNode, Option<&[u8]>)],
        canvas: &mut GridCanvas,
    ) -> Result<()> {
        let hevc = jobs
            .iter()
            .all(|j| codec::codec_of(j.1).is_ok_and(|c| c == Codec::Hevc));
        if hevc {
            self.run_tiles(jobs, canvas)
        } else {
            self.run_other_tiles(jobs, canvas)
        }
    }

    /// [`Session::run_tiles`] for coded pictures of any codec, one decode per picture on
    /// the current rayon pool when allowed; failures are concealed the same way.
    fn run_other_tiles(
        &mut self,
        jobs: &[(usize, &ImageNode, Option<&[u8]>)],
        canvas: &mut GridCanvas,
    ) -> Result<()> {
        use rayon::prelude::*;
        self.count_decode(jobs.len())?;
        let opts = *self.opts;
        if opts.cancelled() {
            return Err(Error::Cancelled);
        }
        let file = self.file;
        let shared = Mutex::new((
            std::mem::replace(
                canvas,
                GridCanvas::new(&heif::GridDescriptor::new(1, 1, 1, 1)),
            ),
            Vec::<(usize, Error)>::new(),
            None::<VuiColour>,
            0usize,
            Vec::<(usize, usize)>::new(),
            None::<Error>,
        ));
        let one = |&(i, node, payload): &(usize, &ImageNode, Option<&[u8]>)| -> Result<(), ()> {
            let r = decode_coded(file, node, payload, 1, &opts);
            let mut g = shared.lock().unwrap_or_else(|p| p.into_inner());
            match r {
                Ok(t) => {
                    g.2 = g.2.or(t.vui);
                    if let Err(e) = g.0.place(i, &t.frame) {
                        g.5 = Some(e.into());
                        return Err(());
                    }
                    g.3 += 1;
                    if t.concealed_ctus > 0 {
                        g.4.push((i, t.concealed_ctus));
                    }
                    Ok(())
                }
                Err(e @ (Error::Cancelled | Error::NeedsMemory(_) | Error::Limit(_))) => {
                    g.5 = Some(e);
                    Err(())
                }
                Err(e) => {
                    g.1.push((i, e));
                    Ok(())
                }
            }
        };
        let _ = if opts.parallel && jobs.len() > 1 {
            jobs.par_iter().try_for_each(one)
        } else {
            jobs.iter().try_for_each(one)
        };
        let (c, mut failed, vui, ok, mut damaged, stop) =
            shared.into_inner().unwrap_or_else(|p| p.into_inner());
        if let Some(e) = stop {
            return Err(e);
        }
        if opts.cancelled() {
            return Err(Error::Cancelled);
        }
        damaged.sort_unstable();
        for (i, n) in damaged {
            self.warnings
                .push(format!("tile {i}: damaged data concealed ({n})"));
        }
        *canvas = c;
        if let Some(n) = jobs.first() {
            self.note_vui(n.1, vui);
        }
        if ok == 0
            && let Some((_, e)) = failed.drain(..).next()
        {
            return Err(e);
        }
        failed.sort_by_key(|f| f.0);
        for (i, e) in failed {
            canvas.place_blank(i)?;
            self.warnings.push(format!("tile {i} concealed: {e}"));
        }
        Ok(())
    }

    /// A coded item with a `cexg` property (HEIF Amd 1 §6.5.41): every
    /// `iloc` extent is one independently decodable tile.
    fn constrained_extents(
        &mut self,
        node: &ImageNode,
        cexg: &heif::props::Cexg,
    ) -> Result<HeifFrame> {
        let id = node.item.id;
        let (w, h) = node.reconstructed_size()?;
        let extents = self.file.item_extents(id)?;
        let covers = cexg.tile_width as u64 * cexg.columns as u64 >= w as u64
            && cexg.tile_height as u64 * cexg.rows as u64 >= h as u64;
        if extents.len() != cexg.tile_count()
            || cexg.tile_width == 0
            || cexg.tile_height == 0
            || !covers
            || cexg.rows > u16::MAX as u32
            || cexg.columns > u16::MAX as u32
        {
            return Err(heif::HeifError::invalid(format!("item {id}: inconsistent cexg")).into());
        }
        if cexg.extent_config.is_some() {
            return Err(Error::Unsupported(format!(
                "item {id}: cexg with an ExtentDecoderConfigurationRecord"
            )));
        }
        let desc = heif::GridDescriptor::new(cexg.rows as u16, cexg.columns as u16, w, h);
        let mut canvas = GridCanvas::new(&desc);
        let mut tile = node.clone();
        tile.item.id = id;
        let ispe = heif::Ispe::new(cexg.tile_width, cexg.tile_height);
        tile.properties = with_ispe(&node.properties, ispe);
        let jobs: Vec<(usize, &ImageNode, Option<&[u8]>)> = extents
            .iter()
            .enumerate()
            .map(|(i, b)| (i, &tile, Some(*b)))
            .collect();
        self.run_jobs(&jobs, &mut canvas)?;
        Ok(canvas.finish()?)
    }

    /// Decode coded tiles into `canvas`, in parallel on the current rayon
    /// pool when allowed. A tile that fails to decode is concealed as a
    /// neutral grey tile (a warning) unless every tile fails.
    fn run_tiles(
        &mut self,
        jobs: &[(usize, &ImageNode, Option<&[u8]>)],
        canvas: &mut GridCanvas,
    ) -> Result<()> {
        self.count_decode(jobs.len())?;
        let opts = *self.opts;
        if opts.cancelled() {
            return Err(Error::Cancelled);
        }
        // Every tile checked first, then the whole batch to the core (a
        // hardware core keeps several in flight).
        let mut failed = Vec::<(usize, Error)>::new();
        let mut coded = Vec::with_capacity(jobs.len());
        // Payloads copied together from several extents are all held until the batch is
        // decoded: each is charged to the budget as it is read (65 535 tiles each naming the
        // same large extents twice could otherwise copy it 131 070 times).
        let mut payloads = specular_core::Reservation::none();
        // The pictures decoding at once: one per worker, as `decode_coded` charges one.
        let mut picture = 0u64;
        for &(i, node, payload) in jobs {
            match prepare_coded(self.file, node, payload) {
                Ok(c) => {
                    if let std::borrow::Cow::Owned(d) = &c.data {
                        payloads.grow(d.len() as u64)?;
                    }
                    picture = picture.max(c.picture_bytes.saturating_mul(2));
                    coded.push((i, c));
                }
                Err(e) => failed.push((i, e)),
            }
        }
        let workers = if opts.parallel && coded.len() > 1 {
            rayon::current_num_threads().clamp(1, coded.len())
        } else {
            1
        };
        let _pictures = specular_core::budget::reserve_n(workers as u64, picture)?;
        let inputs: Vec<CoreInput<'_>> = coded.iter().map(|(_, c)| c.input(1, &opts)).collect();
        let shared = Mutex::new((
            std::mem::replace(
                canvas,
                GridCanvas::new(&heif::GridDescriptor::new(1, 1, 1, 1)),
            ),
            failed,
            None::<VuiColour>,
            0usize,
            Vec::<(usize, usize)>::new(),
            None::<Error>,
        ));
        let sink = |k: usize, r: std::result::Result<CoreOutput, CoreError>| -> bool {
            let (i, c) = &coded[k];
            let mut g = shared.lock().unwrap_or_else(|p| p.into_inner());
            match r {
                Ok(mut t) => {
                    t.frame.format.siting = c.siting;
                    g.2 = g.2.or(t.vui);
                    if let Err(e) = g.0.place(*i, &t.frame) {
                        g.5 = Some(e.into());
                        return false;
                    }
                    g.3 += 1;
                    if t.concealed_ctus > 0 {
                        g.4.push((*i, t.concealed_ctus));
                    }
                    true
                }
                Err(e) => match c.error(e) {
                    Error::Cancelled => {
                        g.5 = Some(Error::Cancelled);
                        false
                    }
                    e => {
                        g.1.push((*i, e));
                        true
                    }
                },
            }
        };
        let core: &dyn HevcCore = opts.core.unwrap_or(&DEFAULT_CORE);
        core.decode_batch(&inputs, opts.parallel && inputs.len() > 1, &sink);
        let (c, mut failed, vui, ok, mut damaged, stop) =
            shared.into_inner().unwrap_or_else(|p| p.into_inner());
        if let Some(e) = stop {
            return Err(e);
        }
        if opts.cancelled() {
            return Err(Error::Cancelled);
        }
        damaged.sort_unstable();
        for (i, n) in damaged {
            self.warnings.push(format!("tile {i}: {n} CTUs concealed"));
        }
        *canvas = c;
        if let Some(n) = jobs.first() {
            self.note_vui(n.1, vui);
        }
        if ok == 0
            && let Some((_, e)) = failed.drain(..).next()
        {
            return Err(e);
        }
        failed.sort_by_key(|f| f.0);
        for (i, e) in failed {
            canvas.place_blank(i)?;
            self.warnings.push(format!("tile {i} concealed: {e}"));
        }
        Ok(())
    }

    /// Output image of a node (§6.3): reconstruction, transformative
    /// properties, then the alpha auxiliary attached.
    fn output(&mut self, node: &ImageNode) -> Result<HeifFrame> {
        if self.opts.cancelled() {
            return Err(Error::Cancelled);
        }
        let unsupported = unsupported_essential(node);
        if !unsupported.is_empty() {
            return Err(Error::Unsupported(format!(
                "item {}: essential properties not recognised: {}",
                node.item.id,
                unsupported
                    .iter()
                    .map(heif::boxes::fourcc_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let rec = self.reconstruct(node)?;
        let mut out = apply_transforms_owned(rec, node.properties.transformative(), false)?;
        if let Some(a) = &node.alpha {
            // A broken alpha plane is dropped rather than failing the image.
            match self.output(a) {
                Ok(alpha) => match attach_alpha(&out, &alpha) {
                    Ok(f) => out = f,
                    Err(e) => self.warnings.push(format!("alpha dropped: {e}")),
                },
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(e) => self.warnings.push(format!("alpha dropped: {e}")),
            }
        }
        Ok(out)
    }
}

/// Essential properties nothing here interprets (a codec's own configuration boxes are).
fn unsupported_essential(node: &ImageNode) -> Vec<heif::boxes::FourCc> {
    let mut v = node.properties.unsupported_essential();
    v.retain(|t| !codec::CODEC_PROPERTIES.contains(t));
    v
}

fn is_aux(node: &ImageNode) -> bool {
    node.properties.auxc().is_some()
}

/// `props` with its `ispe` replaced, or added (the `cexg` / `tili` tile size).
fn with_ispe(props: &heif::ItemProperties, ispe: heif::Ispe) -> heif::ItemProperties {
    let mut p = props.clone();
    let mut found = false;
    for e in &mut p.entries {
        if let heif::props::Property::Ispe(v) = &mut e.property {
            *v = ispe;
            found = true;
        }
    }
    if !found {
        p.entries.push(heif::PropertyEntry::new(
            0,
            false,
            heif::props::Property::Ispe(ispe),
        ));
    }
    p
}
