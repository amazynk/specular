//! GIMP XCF reader for Specular.
//!
//! XCF stores no merged image, so this crate composites the visible layer stack itself, the
//! way GIMP's projection does: layer by layer in float, in the colour space each layer mode
//! asks for, honouring opacity, offsets, masks, groups and pass-through groups.
//!
//! | | Supported |
//! |---|---|
//! | Versions | 0 (GIMP 0.99) to 3.x (format 20+, layer effects listed but not rendered) |
//! | Offsets | 32-bit (≤ v10) and 64-bit (v11+) |
//! | Tiles | uncompressed, RLE, zlib; little-endian (≤ v11) and big-endian (v12+) samples |
//! | Precision | 8/16/32-bit integer, half, float, double; linear, non-linear and perceptual |
//! | Base types | RGB, greyscale, indexed (colormap; v0's broken maps as grey), each ± alpha |
//! | Layer modes | all 64 GIMP modes: legacy (≤ 2.8), 2.10+ blend modes with composite modes and spaces, dissolve, erase, merge, split, replace, anti-erase |
//! | Structure | opacity, offsets, visibility, layer and group masks, groups, pass-through, visible channels (colour overlays); group and image projections stored in the image precision as GIMP does |
//! | Parasites | `icc-profile` |
//! | Containers | `.xcf.gz` (gzip), `.xcf.bz2` (own bzip2), `.xcf.xz` (LZMA2 in an own xz reader) |
//!
//! The separable blend formulas are the ones shared with the other layered formats
//! (`specular-blend`, in their unclamped float forms); GIMP's HSV, LCh, luma and
//! colour-erase modes and the 2.8 legacy rounding stay here.
//!
//! GIMP 3 layer effects (non-destructive filters) are read with their arguments; the common
//! colour and pixel operations are drawn (see `filters.rs`). Layers are drawn without any
//! other operation, which [`Info::undrawn_filters`] names (XCF stores no merged image to fall
//! back on).
//!
//! Compositing runs in parallel bands of rows; layer tiles are decoded (in parallel) only for
//! the rows a group of bands needs, so memory stays near one canvas-sized output plus a few
//! bands of every layer.
//!
//! Robustness: no input panics; allocations are bounded by [`MAX_PIXELS`] and by the tiles
//! present in the file. Tiles that are missing or corrupt leave the layer transparent there
//! and set [`Image::truncated`].

mod blend;
mod bzip2;
mod container;
mod filters;
mod legacy;
mod parse;
mod pixel;
mod read;
mod render;
mod rng;
mod tile;

pub use container::{Container, bunzip2, container, decompress, decompress_reserved};
pub use parse::{BaseType, Component, Compression, Precision};

use std::ops::Range;

/// Largest canvas (and layer) width or height GIMP creates.
pub const MAX_DIMENSION: u32 = 524_288;
/// Largest canvas pixel count accepted (512 MP).
pub const MAX_PIXELS: u64 = 1 << 29;
/// Largest decompressed size of an `.xcf.gz/.bz2/.xz` accepted.
pub const MAX_DECOMPRESSED: u64 = 1 << 34;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not an XCF file (or a compressed container without one inside).
    NotXcf,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The structure needed to reach the pixels is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over the caps.
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    NeedsMemory(specular_core::NeedsMemory),
    /// [`Options::cancel`] returned true.
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotXcf => write!(f, "not a GIMP XCF file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

impl From<specular_core::NeedsMemory> for Error {
    fn from(n: specular_core::NeedsMemory) -> Self {
        Error::NeedsMemory(n)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

const MAGIC: &[u8; 9] = b"gimp xcf ";

/// Whether `head` starts like an uncompressed XCF file (14-byte magic: `gimp xcf file` or
/// `gimp xcf vNNN`, NUL-terminated).
pub fn probe(head: &[u8]) -> bool {
    if head.len() < 14 || &head[..9] != MAGIC || head[13] != 0 {
        return false;
    }
    let v = &head[9..13];
    v == b"file" || (v[0] == b'v' && v[1..].iter().all(u8::is_ascii_digit))
}

/// Whether `data` is an XCF file, plain or in a gzip, bzip2 or xz container (whose start is
/// decompressed to look).
pub fn probe_file(data: &[u8]) -> bool {
    probe(data) || container::probe_compressed(data)
}

/// The image header and what the layer stack holds.
#[derive(Clone, Debug)]
pub struct Info {
    /// File format version (0 for `gimp xcf file`).
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub base: BaseType,
    pub precision: Precision,
    pub compression: Compression,
    /// Leaf layers (groups not counted).
    pub layer_count: usize,
    pub group_count: usize,
    /// Live (non-destructive) filters attached to layers: GIMP 3 renders them, this crate
    /// does not.
    pub live_filters: usize,
    /// The operations of visible layer effects this crate does not draw (GEGL names such as
    /// `gegl:emboss`; `unreadable` for a record it could not read), each once.
    pub undrawn_filters: Vec<String>,
    /// Whether the composite can have transparent pixels (a conservative estimate made from
    /// the layer stack before decoding).
    pub has_alpha: bool,
}

/// One layer or group as listed in the file, top of the stack first.
#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    /// Nesting depth (0 = top level).
    pub depth: u32,
    pub group: bool,
    pub visible: bool,
    /// GIMP layer mode number (`GimpLayerMode`).
    pub mode: u32,
    /// 0..=1.
    pub opacity: f32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub has_mask: bool,
}

/// A parsed XCF file: header, properties and the layer tree with tile offsets. Holds no pixel
/// data; [`Xcf::render`] reads the tiles from the same bytes.
#[derive(Clone, Debug)]
pub struct Xcf {
    info: Info,
    layers: Vec<Layer>,
    icc: Option<Range<usize>>,
    doc: parse::Doc,
}

/// Parses an uncompressed XCF file (see [`decompress`] for `.xcf.gz` and friends).
pub fn parse(data: &[u8]) -> Result<Xcf> {
    parse::parse(data)
}

impl Xcf {
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// Every layer and group, top of the stack first (the order GIMP's Layers dialog shows).
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The `icc-profile` parasite: the image's colour profile.
    pub fn icc<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        data.get(self.icc.clone()?)
    }

    /// The pixel format [`Xcf::render`] produces for this file.
    pub fn output_format(&self) -> PixelFormat {
        render::output_format(&self.info, self.icc.is_some(), self.info.has_alpha)
    }

    /// Composites the visible layers into the image.
    pub fn render(&self, data: &[u8], opts: &Options) -> Result<Image> {
        render::render(self, data, opts)
    }

    /// The leaf layers that hold pixels, top of the stack first, as indices into
    /// [`Xcf::layers`] (hidden ones too; groups and empty layers are left out).
    pub fn pixel_layers(&self) -> Vec<usize> {
        render::pixel_layers(&self.doc)
    }

    /// The pixel format [`Xcf::render_layer`] produces: [`Xcf::output_format`] with alpha.
    pub fn layer_format(&self) -> PixelFormat {
        render::output_format(&self.info, self.icc.is_some(), true)
    }

    /// Layer `index` (into [`Xcf::layers`]; not a group) alone on the canvas at its offsets,
    /// with its transparency, applied layer mask and opacity; its blend mode, the groups
    /// above it and the visible channels are not applied. Only that layer's tiles are read.
    /// Missing or corrupt tiles leave it transparent there and set [`Image::truncated`].
    pub fn render_layer(&self, data: &[u8], index: usize, opts: &Options) -> Result<Image> {
        render::render_layer(self, data, index, opts)
    }
}

/// How [`Xcf::render`] runs.
#[derive(Clone, Copy, Default)]
pub struct Options<'a> {
    /// Use the rayon pool (tiles and bands in parallel).
    pub parallel: bool,
    /// Polled between bands; returning true aborts with [`Error::Cancelled`].
    pub cancel: Option<&'a (dyn Fn() -> bool + Sync)>,
}

impl Options<'_> {
    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c())
    }
}

/// Layout of [`Image::data`]. Sixteen-bit formats are native-endian `u16`; alpha is straight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// Opaque 8-bit grey, sRGB-coded (or in the embedded profile's TRC).
    Gray8,
    /// 8-bit RGBA, sRGB-coded (or in the embedded profile's TRC).
    Rgba8,
    /// Opaque 16-bit grey, sRGB-coded (or in the embedded profile's TRC).
    Gray16,
    /// 16-bit grey and alpha, sRGB-coded (or in the embedded profile's TRC).
    GrayAlpha16,
    /// 16-bit RGBA, sRGB-coded (or in the embedded profile's TRC).
    Rgba16,
    /// IEEE half RGBA, linear light with sRGB primaries; values beyond 0..1 are kept.
    Rgba16F,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Gray16 => 2,
            PixelFormat::Rgba8 | PixelFormat::GrayAlpha16 => 4,
            PixelFormat::Rgba16 | PixelFormat::Rgba16F => 8,
        }
    }
}

/// A composited image.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    /// Some tiles were missing or corrupt (those areas of their layers are transparent).
    pub truncated: bool,
}

/// Converts an `f32` to IEEE half bits (round to nearest even; overflow to infinity).
pub fn f32_to_f16(v: f32) -> u16 {
    pixel::f32_to_f16(v)
}

/// Converts IEEE half bits to `f32`.
pub fn f16_to_f32(h: u16) -> f32 {
    pixel::f16_to_f32(h)
}
