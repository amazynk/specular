//! PaintShop Pro (`.psp`, `.pspimage`, `.tub`, `.msk`) reader for Specular.
//!
//! The PSP format is a tree of `~BK\0` blocks after a 36-byte header. File-format major
//! version 3 (Paint Shop Pro 5) has 14-byte block headers with an "initial chunk length" and
//! fixed-size chunks; versions 4 (PSP 6) and later have 10-byte headers and chunks that start
//! with their own size, so fields added by later releases (up to the 2023 releases, major
//! version 14 and later) are skipped by size. Blocks this reader does not know are skipped.
//!
//! | | Supported |
//! |---|---|
//! | Versions | major 3 (PSP 5) to 14+ (PaintShop Pro 2020–2023) |
//! | Colour | 1/4/8-bit palette, 8-bit grey, 24-bit RGB; 48-bit RGB (16 bits per channel) |
//! | Channels | uncompressed, RLE, LZ77 (zlib); rows of 1/4-bit channels padded to 4 bytes |
//! | Layers | raster and floating-selection layers with transparency, user masks (disabled, inverted), opacity, visibility, offsets, every blend mode; groups (isolated); mask layers |
//! | Composites | the composite image bank (lossless or JPEG, the full-size composite and the thumbnail) and PSP 5's thumbnail block |
//! | Vector layers | polyline shapes: fill and stroke in a colour (round joins and caps) |
//! | Not drawn | vector text, adjustment and art media layers without pixels (named in [`Info::not_drawn`]; the file's composite shows them) |
//!
//! Blend formulas are the ones shared with the other layered formats (`specular-blend`).
//!
//! [`Psp::render`] shows the file's own lossless composite when there is one (fast and exactly
//! what PaintShop Pro showed), else composites the layers; a JPEG composite is used only when
//! the layers hold something this reader cannot draw. Layers are composited in 16-bit
//! straight RGBA, band-parallel.
//!
//! Robustness: no input panics; canvases and layers are bounded by [`MAX_PIXELS`] and reserved
//! from the memory budget; channel data that is missing or corrupt leaves those pixels
//! transparent and sets [`Image::truncated`].

mod blend;
mod channel;
mod parse;
mod render;
mod vector;

use std::ops::Range;

/// Largest canvas (or layer) pixel count accepted (512 MP).
pub const MAX_PIXELS: u64 = 1 << 29;
/// Largest canvas width or height accepted.
pub const MAX_DIMENSION: u32 = 1 << 20;

/// The 32-byte file signature.
pub const SIGNATURE: &[u8; 32] = b"Paint Shop Pro Image File\n\x1a\0\0\0\0\0";

/// Whether `head` starts with the PSP signature.
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(SIGNATURE)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a PaintShop Pro file.
    NotPsp,
    /// A valid file using a feature this reader does not implement.
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
            Error::NotPsp => write!(f, "not a PaintShop Pro file"),
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

/// How channel data is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Rle,
    Lz77,
    /// Only composites and thumbnails.
    Jpeg,
}

/// A layer's blend mode, as PaintShop Pro numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Darken,
    Lighten,
    /// "Hue (Legacy)": HSL hue.
    LegacyHue,
    LegacySaturation,
    LegacyColor,
    LegacyLuminosity,
    Multiply,
    Screen,
    Dissolve,
    Overlay,
    HardLight,
    SoftLight,
    Difference,
    Dodge,
    Burn,
    Exclusion,
    /// "Hue" of PSP 8 and later: the non-separable luminosity-preserving modes.
    TrueHue,
    TrueSaturation,
    TrueColor,
    TrueLightness,
    /// Adjustment layers' marker mode.
    Adjust,
    /// A mode number this reader does not know (drawn as Normal).
    Other(u8),
}

impl BlendMode {
    pub(crate) fn from_u8(v: u8) -> BlendMode {
        use BlendMode::*;
        match v {
            0 => Normal,
            1 => Darken,
            2 => Lighten,
            3 => LegacyHue,
            4 => LegacySaturation,
            5 => LegacyColor,
            6 => LegacyLuminosity,
            7 => Multiply,
            8 => Screen,
            9 => Dissolve,
            10 => Overlay,
            11 => HardLight,
            12 => SoftLight,
            13 => Difference,
            14 => Dodge,
            15 => Burn,
            16 => Exclusion,
            17 => TrueHue,
            18 => TrueSaturation,
            19 => TrueColor,
            20 => TrueLightness,
            255 => Adjust,
            n => Other(n),
        }
    }
}

/// What a layer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    /// Raster layers and floating raster selections.
    Raster,
    Vector,
    ArtMedia,
    Adjustment,
    Group,
    Mask,
    /// A layer type number this reader does not know.
    Other(u16),
}

/// One layer as the file describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    pub kind: LayerKind,
    pub visible: bool,
    /// 0..=255.
    pub opacity: u8,
    pub blend: BlendMode,
    /// x, y (may be negative), width, height on the canvas of the stored pixels.
    pub rect: (i32, i32, u32, u32),
    pub has_mask: bool,
    /// Whether the layer has colour channels this reader draws.
    pub has_pixels: bool,
    /// Group nesting: 0 at the top level.
    pub depth: u32,
}

/// Facts about the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// File-format version: 3 = Paint Shop Pro 5, 4 = PSP 6, 5 = PSP 7, 6 = PSP 8, 7 = PSP 9,
    /// 8 = PSP X, and so on to the 2020-2023 releases (14 and later).
    pub major: u16,
    pub minor: u16,
    /// Bits per pixel of the layers as stored: 1, 4, 8, 24 or 48.
    pub bit_depth: u16,
    pub greyscale: bool,
    pub palette: bool,
    /// Compression of the layer channels.
    pub compression: Compression,
    /// Pixels per inch.
    pub dpi: Option<f64>,
    pub layer_count: usize,
    /// A full-size composite image is stored.
    pub has_composite: bool,
    pub has_thumbnail: bool,
    /// Why some layers are not drawn when the layers are composited (vector, adjustment or art
    /// media layers), in plain words; None when every visible layer is drawn.
    pub not_drawn: Option<String>,
    /// The creating application, when the creator block names it.
    pub creator: Option<String>,
    /// A picture tube (the `.tub` tube data block is present).
    pub tube: bool,
}

/// Pixel layout of an [`Image`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    /// Straight alpha.
    Rgba8,
    /// Native-endian `u16`, straight alpha (48-bit documents).
    Rgba16,
}

impl PixelFormat {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 => 1,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Rgba16 => 8,
        }
    }
}

/// A decoded image.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// Rows top to bottom, `width * format.bytes_per_pixel()` bytes each.
    pub data: Vec<u8>,
    /// Some channel data was missing or corrupt.
    pub truncated: bool,
}

/// How to render.
#[derive(Clone, Copy)]
pub struct Options<'a> {
    /// Composite in parallel row bands.
    pub parallel: bool,
    /// Polled between layers and row bands; when it returns true the render stops with
    /// [`Error::Cancelled`].
    pub cancel: Option<&'a (dyn Fn() -> bool + Sync)>,
    /// Show the file's composite image when it has one (default); false composites the layers
    /// even then (tests).
    pub prefer_composite: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            parallel: true,
            cancel: None,
            prefer_composite: true,
        }
    }
}

impl Options<'_> {
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c())
    }
}

/// A channel sub-block: where its data is and what it is for.
#[derive(Clone, Debug)]
pub(crate) struct Channel {
    pub(crate) data: Range<usize>,
    /// PSPDIBType: 0 colour, 1 transparency, 2 user mask, 5 thumbnail, 8 composite, …
    pub(crate) dib: u16,
    /// PSPChannelType: 0 single channel, 1 red, 2 green, 3 blue.
    pub(crate) kind: u16,
}

/// A stored composite or thumbnail.
#[derive(Clone, Debug)]
pub(crate) struct Stored {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bit_depth: u16,
    pub(crate) compression: Compression,
    /// 0 = composite, 1 = thumbnail.
    pub(crate) kind: u16,
    pub(crate) body: StoredBody,
}

#[derive(Clone, Debug)]
pub(crate) enum StoredBody {
    Channels {
        palette: Option<Vec<[u8; 3]>>,
        channels: Vec<Channel>,
    },
    Jpeg(Range<usize>),
    Missing,
}

/// A layer with what rendering needs, in file order (bottom first).
#[derive(Clone, Debug)]
pub(crate) struct LayerRec {
    pub(crate) public: Layer,
    pub(crate) mask_rect: (i32, i32, u32, u32),
    pub(crate) mask_disabled: bool,
    pub(crate) mask_invert: bool,
    pub(crate) channels: Vec<Channel>,
    /// Direct children of a group layer.
    pub(crate) group_count: Option<u32>,
    /// A vector layer's shapes (its vector extension block).
    pub(crate) vector: Option<vector::Shapes>,
}

/// A layer tree node: an index into the file-order layer list and the children of a group,
/// bottom first.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) rec: usize,
    pub(crate) children: Vec<Node>,
}

/// A parsed file: the block tree reduced to ranges into the file bytes.
#[derive(Clone, Debug)]
pub struct Psp {
    pub(crate) info: Info,
    /// File order (bottom first).
    pub(crate) recs: Vec<LayerRec>,
    /// Top-level nodes, bottom first.
    pub(crate) tree: Vec<Node>,
    /// Public order (top-most first) → index into `recs`.
    pub(crate) order: Vec<usize>,
    pub(crate) layers: Vec<Layer>,
    pub(crate) palette: Option<Vec<[u8; 3]>>,
    pub(crate) stored: Vec<Stored>,
}

/// Parses the block structure (no pixel data is read).
pub fn parse(data: &[u8]) -> Result<Psp, Error> {
    parse::parse(data)
}

impl Psp {
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// The layers, top-most first as PaintShop Pro's Layers palette lists them (a group comes
    /// before its own layers).
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Indices into [`Psp::layers`] of the layers with pixels, top-most first.
    pub fn pixel_layers(&self) -> Vec<usize> {
        (0..self.layers.len())
            .filter(|&i| self.layers[i].has_pixels)
            .collect()
    }

    /// The image: the file's composite when there is one (see the crate docs), else the
    /// visible layers composited.
    pub fn render(&self, data: &[u8], opts: &Options) -> Result<Image, Error> {
        render::render(self, data, opts)
    }

    /// Layer `layer` ([`Psp::layers`] index) alone on the canvas at its offsets, with its
    /// transparency, mask and opacity (its blend mode and visibility are not applied).
    pub fn render_layer(&self, data: &[u8], layer: usize, opts: &Options) -> Result<Image, Error> {
        render::render_layer(self, data, layer, opts)
    }

    /// The stored thumbnail, when the file has one.
    pub fn thumbnail(&self, data: &[u8]) -> Option<Result<Image, Error>> {
        render::thumbnail(self, data)
    }
}
