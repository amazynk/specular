//! Photoshop PSD and PSB reader for Specular.
//!
//! A viewer needs the *merged composite* that Photoshop stores after the layers ("Maximize
//! compatibility"), so that is chiefly what this crate decodes. Layers are listed (name, bounds,
//! opacity, visibility, group structure) and each pixel layer can be decoded on its own onto the
//! document canvas ([`Psd::pixel_layers`], [`Psd::decode_layer_planes`]): its colour channels,
//! transparency, user mask and opacity, without blending or effects.
//!
//! | | Supported |
//! |---|---|
//! | Container | PSD (v1, ≤ 30 000 px) and PSB (v2, ≤ 300 000 px) |
//! | Depth | 1 (Bitmap), 8, 16, 32 (float) bits per sample |
//! | Compression | raw, RLE (PackBits), ZIP, ZIP with prediction |
//! | Colour modes | Bitmap, Grayscale, Indexed, RGB, CMYK, Lab, Duotone (as grey), Multichannel (first 3) |
//! | Resources | thumbnail (1036, old BGR 1033), ICC (1039), resolution (1005), XMP (1060), EXIF (1058/1059), IPTC (1028), version info (1057) |
//!
//! Two decode paths:
//!
//! * [`Psd::decode_rgba8`] — straight to 8-bit [`Image`]. 8-bit RGB / Grey / Indexed / Bitmap /
//!   Duotone / Multichannel are decoded in one fused, row-parallel pass (PackBits rows are
//!   independently addressable, so every row of every channel decodes on its own thread). Other
//!   modes and depths go through [`Psd::decode_planes`] + [`Planes::to_rgba8`], a *naive*
//!   conversion meant for quick previews (no ICC).
//! * [`Psd::decode_planes`] — the composite's channels at native depth (u8 / u16 / f32), planar,
//!   plus [`Info::mode`]; together with [`Psd::icc`] the caller can colour-manage CMYK, Lab and
//!   high-bit-depth images properly.
//! * [`Psd::decode_wide`] — 16-bit RGB and grey-like composites interleaved at 16 bits
//!   ([`PixelFormat::Gray16`], [`PixelFormat::GrayAlpha16`] or [`PixelFormat::Rgba16`]), for
//!   callers that keep the file's precision.
//!
//! Files saved with "Maximize compatibility" off carry no usable composite, nor do files
//! whose stored merged image is a blank placeholder (every sample the same under layers that
//! show something, [`Composite::Blank`]): [`Info::composite`] says so, and the decoders build
//! the composite from the layers instead ([`Psd::composites_layers`]): pixel, fill and shape
//! layers with their strokes; adjustment layers; layer effects; blend modes (every
//! Photoshop mode, through the formulas shared with the other layered formats in
//! `specular-blend`), opacity and fill opacity, clipping, user and vector masks, groups
//! (pass-through included), knockout, Blend If and artboards. A file whose layers draw
//! nothing (adjustment layers alone, empty layers) gives a transparent canvas, as
//! Photoshop shows it; only a file without any layer record and without a merged image
//! returns [`Error::NoComposite`]. [`Psd::compose_layers`] builds the same composite beside
//! a stored merged image, to view the file by its layers. [`Psd::thumbnail`] (a small JPEG)
//! is usually still there and makes a fast preview in every case.
//!
//! Not supported in the composite built from the layers ([`Psd::composite_notes`] names
//! them when a file uses them): Color Lookup adjustments holding a 3D LUT or device-link
//! profile (an empty one is drawn: it changes nothing),
//! noise gradients, layer styles stored only in the pre-CS format, 3D and video layers
//! and smart-object filters (their stored pixels are drawn); Bevel & Emboss, Satin and
//! Blend If are approximations. Text and smart-object layers draw the pixels Photoshop
//! stored for them. Against the merged images Photoshop stored in the corpus (the file's
//! own composite as the oracle, `tests/merged_oracle.rs`), most layered RGB and grey files
//! match within the composite oracle's bar; the rest are named there.
//!
//! Robustness: no input panics; every allocation is bounded by checked header sizes
//! ([`MAX_PIXELS`], per-version dimension caps). Pixel data that ends early or is corrupt yields an
//! image with [`Image::truncated`] set and the missing samples grey.

mod adjust;
mod compose;
mod convert;
mod decode;
mod desc;
mod effects;
mod fill;
mod layer;
mod legacy;
mod parse;
mod rle;
mod vector;

pub use convert::{lab_to_srgb8, linear_to_srgb8};

use std::ops::Range;

/// Largest width/height of a PSD (version 1) file.
pub const MAX_DIMENSION_PSD: u32 = 30_000;
/// Largest width/height of a PSB (version 2) file.
pub const MAX_DIMENSION_PSB: u32 = 300_000;
/// Largest pixel count accepted (512 MP: 2 GiB as RGBA8).
pub const MAX_PIXELS: u64 = 1 << 29;

/// The value missing or corrupt samples get.
pub const GREY: u8 = 128;

/// Refuses to allocate for pixels a file this short cannot hold: `have` bytes of input must
/// be at least 1/16 of the `least` the pixels need when fully compressed (`least` = their
/// bytes over the best ratio), so a partial file still shows. Under 16 MiB always passes.
pub(crate) fn proven(have: usize, least: usize, what: &str) -> Result<()> {
    if least > 16 << 20 && have < least / 16 {
        return Err(Error::Truncated(what.into()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not a PSD/PSB file (bad signature or version).
    NotPsd,
    /// A valid file using a feature this decoder does not implement.
    Unsupported(String),
    /// The structure needed to reach the pixels is cut short.
    Truncated(String),
    Malformed(String),
    /// Dimensions over the caps.
    TooLarge,
    /// The file has no usable merged composite; see [`Composite`].
    NoComposite(Composite),
    /// The caller's cancel flag was raised during a decode.
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotPsd => write!(f, "not a PSD/PSB file"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::Truncated(s) => write!(f, "truncated: {s}"),
            Error::Malformed(s) => write!(f, "malformed: {s}"),
            Error::TooLarge => write!(f, "image too large"),
            Error::NoComposite(Composite::NotMaximized) => write!(
                f,
                "no merged image: saved with \"Maximize compatibility\" off (layers only)"
            ),
            Error::NoComposite(Composite::Blank) => {
                write!(f, "the merged image is blank (a placeholder)")
            }
            Error::NoComposite(c) => write!(f, "no merged image ({c:?})"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// PSD (version 1) or PSB "large document" (version 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Psd,
    Psb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// 1-bit; a set bit is black.
    Bitmap,
    Grayscale,
    /// 8-bit indices into [`Info::palette`].
    Indexed,
    Rgb,
    /// Stored inverted: 0 = 100 % ink, max = no ink.
    Cmyk,
    /// Spot-ink channels; shown as RGB from the first three (grey if fewer).
    Multichannel,
    /// Stored as one grey channel (the ink curves are ignored).
    Duotone,
    /// L 0..max ↦ 0..100, a/b offset by half the range.
    Lab,
}

impl ColorMode {
    fn from_u16(v: u16) -> Option<ColorMode> {
        Some(match v {
            0 => ColorMode::Bitmap,
            1 => ColorMode::Grayscale,
            2 => ColorMode::Indexed,
            3 => ColorMode::Rgb,
            4 => ColorMode::Cmyk,
            7 => ColorMode::Multichannel,
            8 => ColorMode::Duotone,
            9 => ColorMode::Lab,
            _ => return None,
        })
    }

    /// Colour channels this mode uses from a file with `file_channels` channels (alpha excluded).
    pub fn color_channels(self, file_channels: u16) -> usize {
        match self {
            ColorMode::Bitmap | ColorMode::Grayscale | ColorMode::Indexed | ColorMode::Duotone => 1,
            ColorMode::Rgb | ColorMode::Lab => 3,
            ColorMode::Cmyk => 4,
            ColorMode::Multichannel => {
                if file_channels >= 3 {
                    3
                } else {
                    1
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Raw,
    /// PackBits per row, with a table of row byte counts (row-parallel).
    Rle,
    /// One zlib stream for the whole image.
    Zip,
    /// zlib with per-row delta prediction (byte-plane shuffled for 32-bit).
    ZipPrediction,
}

/// Whether the file carries a merged composite image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Composite {
    Present,
    /// Saved with "Maximize compatibility" off: version-info resource 1057 says the image data
    /// is not real merged data (Photoshop stores a blank placeholder). Only layers have pixels.
    NotMaximized,
    /// The image-data section is absent or empty.
    Missing,
    /// A merged image is stored, but every sample of each colour channel is the same while
    /// visible layers have pixels: a placeholder some writers store instead of the composite.
    Blank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionUnit {
    PerInch,
    PerCm,
}

/// Resource 1005. Photoshop stores the value in pixels per inch whatever the display unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub h_dpi: f64,
    pub v_dpi: f64,
    /// Unit the user chose for display.
    pub unit: ResolutionUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbnailFormat {
    /// JFIF bytes; decode with any JPEG decoder.
    Jpeg,
    /// Uncompressed 24-bit RGB rows padded to `row_bytes` (rare).
    RawRgb { row_bytes: u32 },
}

/// The embedded preview (resource 1036; Photoshop 4's 1033).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumbnail {
    pub format: ThumbnailFormat,
    pub width: u32,
    pub height: u32,
    /// Byte range of the encoded thumbnail in the file.
    pub data: Range<usize>,
    /// Resource 1033: channels are stored B, G, R — swap red and blue after decoding.
    pub bgr: bool,
}

/// Byte ranges of the interesting image resources.
#[derive(Clone, Debug, Default)]
pub struct Resources {
    pub icc: Option<Range<usize>>,
    pub xmp: Option<Range<usize>>,
    /// Resource 1058: a TIFF-structured EXIF block ("II*\0" / "MM\0*").
    pub exif: Option<Range<usize>>,
    /// Resource 1059 (EXIF data 3).
    pub exif3: Option<Range<usize>>,
    /// Resource 1028 (IPTC-NAA).
    pub iptc: Option<Range<usize>>,
    pub thumbnail: Option<Thumbnail>,
    /// Writer named by resource 1057 (e.g. "Adobe Photoshop").
    pub writer: Option<String>,
    /// Global light angle in degrees (resource 1037), which layer effects may follow.
    pub global_angle: Option<i32>,
    /// Global light altitude in degrees (resource 1049).
    pub global_altitude: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Pixel,
    /// Group folder record (written after its children); `open` = expanded in the UI.
    GroupStart {
        open: bool,
    },
    /// Hidden "</Layer group>" divider that opens a group (written before its children).
    GroupEnd,
    /// A fill or adjustment layer (solid colour, gradient, pattern, levels, curves…): it has
    /// no pixels of its own, only perhaps a mask.
    Adjustment,
}

/// One channel of a layer record and where its stored data is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerChannel {
    /// 0.. colour channels, −1 transparency, −2 user mask, −3 real user mask (when the layer
    /// has a vector mask as well).
    pub id: i16,
    /// The compression word and the data, clipped to the file (empty when missing).
    pub data: Range<usize>,
}

/// A layer's user (pixel) mask: the bounds its channel covers and the value outside them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerMask {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    /// 0 or 255: the mask outside its bounds.
    pub default: u8,
    /// Turned off in Photoshop: it does not hide anything.
    pub disabled: bool,
    /// The channel holding it: −2, or −3 for the "real" user mask beside a vector mask.
    pub channel: i16,
    /// Density 0..255 (255 = as stored): a lower one lets hidden parts show through.
    pub density: u8,
    /// Photoshop drew this mask from other data (the vector mask): it stands for that mask.
    pub rendered: bool,
}

/// A layer record. Bounds are in canvas pixels, `bottom`/`right` exclusive. Layers are listed
/// bottom-most first, as stored; [`Psd::decode_layer_planes`] decodes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub opacity: u8,
    pub visible: bool,
    /// Blend-mode key, e.g. `b"norm"`.
    pub blend_mode: [u8; 4],
    pub kind: LayerKind,
    /// Fill opacity (`iOpa`): like `opacity` but not applied to layer effects; 255 if absent.
    pub fill_opacity: u8,
    /// Clipped to the layer below (its clipping base): shown only where that one is.
    pub clipping: bool,
    /// Channel records in stored order, with the byte ranges of their image data.
    pub channels: Vec<LayerChannel>,
    pub mask: Option<LayerMask>,
    /// A fill (or shape) layer's settings: what it paints and its block's bytes in the file.
    pub fill: Option<(FillKind, Range<usize>)>,
    /// The vector mask's block (`vmsk` / `vsms`) in the file.
    pub vector_mask: Option<Range<usize>>,
    /// The vector mask's density 0..255 (255 = as drawn).
    pub vector_mask_density: u8,
    /// Other blocks the composite reads, as (key, body bytes in the file): adjustment
    /// settings, layer effects (`lfx2`, `lmfx`, `lfxs`, `lrFX`), a shape's stroke (`vstk`), artboard
    /// settings (`artb`, `artd`, `abdd`), knockout (`knko`), advanced blending (`clbl`,
    /// `infx`, `tsly`), the effects reference point (`fxrp`).
    pub blocks: Vec<([u8; 4], Range<usize>)>,
    /// Blend If ranges: grey first, then one per colour channel, each this layer's black
    /// low/high and white low/high, then the underlying layer's (0..255). Empty when they let
    /// everything through.
    pub blend_if: Vec<[u8; 8]>,
}

/// What a fill layer paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillKind {
    /// `SoCo`: one colour.
    Solid,
    /// `GdFl`: a gradient.
    Gradient,
    /// `PtFl`: a tiled pattern.
    Pattern,
}

impl Layer {
    /// The body of block `key` among [`Layer::blocks`].
    pub(crate) fn block(&self, key: &[u8; 4]) -> Option<Range<usize>> {
        self.blocks
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, r)| r.clone())
    }

    /// Whether the layer has pixels of its own to show: a pixel (or text, smart-object…)
    /// layer with a non-empty rectangle and at least one stored channel.
    pub fn has_pixels(&self) -> bool {
        self.kind == LayerKind::Pixel
            && self.bottom > self.top
            && self.right > self.left
            && self
                .channels
                .iter()
                .any(|c| c.id >= 0 && !c.data.is_empty())
    }
}

/// What an artboard shows under its layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtboardBackground {
    White,
    Black,
    Transparent,
    /// A colour of its own (sRGB).
    Color([u8; 3]),
}

/// An artboard: a group drawn as a page of its own, clipped to its rectangle.
#[derive(Clone, Debug, PartialEq)]
pub struct Artboard {
    /// Its group's name.
    pub name: String,
    /// Its rectangle in canvas pixels, `bottom`/`right` exclusive (may reach past the canvas).
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub background: ArtboardBackground,
    /// Index of its group record in [`Psd::layers`].
    pub layer: usize,
}

/// What a composite built from the layers leaves out or draws only approximately, by
/// Photoshop's names for the features ("Color Lookup", "Bevel & Emboss"…), each once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompositeNotes {
    pub not_drawn: Vec<&'static str>,
    pub approximate: Vec<&'static str>,
}

/// Header facts.
#[derive(Clone, Debug)]
pub struct Info {
    pub version: Version,
    pub width: u32,
    pub height: u32,
    /// Channels in the file's image data (colour + alpha + spot/alpha masks).
    pub channels: u16,
    /// Bits per sample: 1, 8, 16 or 32.
    pub depth: u8,
    pub mode: ColorMode,
    /// `None` when there is no image-data section.
    pub compression: Option<Compression>,
    /// The composite has a transparency channel (right after the colour channels).
    pub has_alpha: bool,
    /// Colour of transparent composite pixels was flattened onto white (Photoshop does this);
    /// [`Planes::to_rgba8`] / [`Psd::decode_rgba8`] undo it.
    pub matted_on_white: bool,
    /// The composite has transparency but no version-info resource names its writer (tools
    /// that rewrite Photoshop files drop it): the decoders then undo a white matte when the
    /// samples show one ([`looks_matted`]).
    pub matte_unknown: bool,
    pub composite: Composite,
    pub layer_count: usize,
    pub resolution: Option<Resolution>,
    /// Indexed mode: 256 RGB entries.
    pub palette: Option<Vec<[u8; 3]>>,
    /// Indexed mode: palette entry that is transparent (resource 1047).
    pub transparent_index: Option<u8>,
}

impl Info {
    /// The composite can have transparent pixels: an alpha channel, or (indexed mode) a
    /// transparent palette entry. [`Info::has_alpha`] alone is about the channel.
    pub fn is_transparent(&self) -> bool {
        self.has_alpha || (self.mode == ColorMode::Indexed && self.transparent_index.is_some())
    }
}

/// Interleaved pixel layout of an [`Image`]. The 16-bit formats hold native-endian `u16`
/// samples (only [`Psd::decode_wide`] and [`Planes::to_wide`] produce them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    /// Straight (not premultiplied) alpha.
    Rgba8,
    Gray16,
    /// Grey, then straight alpha.
    GrayAlpha16,
    /// Straight alpha.
    Rgba16,
}

impl PixelFormat {
    /// Samples per pixel.
    pub fn channels(self) -> usize {
        match self {
            PixelFormat::Gray8 | PixelFormat::Gray16 => 1,
            PixelFormat::GrayAlpha16 => 2,
            PixelFormat::Rgba8 | PixelFormat::Rgba16 => 4,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Gray8 | PixelFormat::Rgba8 => self.channels(),
            _ => self.channels() * 2,
        }
    }
}

/// Decoded pixels: `width × height` tightly packed rows.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
    /// Pixel data ended early or was corrupt; the missing part is grey.
    pub truncated: bool,
}

/// Planar samples at native depth: plane `c` is `[c * w * h .. (c + 1) * w * h]`.
#[derive(Clone, Debug)]
pub enum Samples {
    /// 8-bit, and 1-bit Bitmap expanded to 0 (black) / 255 (white).
    U8(Vec<u8>),
    U16(Vec<u16>),
    /// 32-bit float, linear light (gamma 1.0) as Photoshop stores it.
    F32(Vec<f32>),
}

/// The composite's channels at native depth.
#[derive(Clone, Debug)]
pub struct Planes {
    pub width: u32,
    pub height: u32,
    pub mode: ColorMode,
    /// Colour channels (see [`ColorMode::color_channels`]).
    pub color_channels: usize,
    /// One more plane after the colour planes holds transparency.
    pub alpha: bool,
    pub samples: Samples,
    pub truncated: bool,
    /// Copied from [`Info`] so [`Planes::to_rgba8`] is self-contained.
    pub palette: Option<Vec<[u8; 3]>>,
    pub transparent_index: Option<u8>,
    pub matted_on_white: bool,
}

/// Whether planar colour samples (`cc` planes of `n`, then the transparency plane) were
/// flattened onto white: a matted pixel's colour is at least `max − alpha` (it is
/// `c·a + max·(1 − a)`), which unmatted colour under partial transparency rarely is
/// everywhere. True only when some pixel is not opaque and none breaks the rule (one code of
/// rounding allowed). 32-bit samples are not judged (false).
pub fn looks_matted(samples: &Samples, cc: usize, n: usize) -> bool {
    fn judge<T: Copy + Into<i64>>(s: &[T], cc: usize, n: usize, max: i64, tol: i64) -> bool {
        if s.len() < (cc + 1) * n || n == 0 {
            return false;
        }
        let alpha = &s[cc * n..(cc + 1) * n];
        let mut seen = false;
        for (i, &a) in alpha.iter().enumerate() {
            let a: i64 = a.into();
            if a >= max {
                continue;
            }
            seen = true;
            if (0..cc).any(|c| {
                let v: i64 = s[c * n + i].into();
                v < max - a - tol
            }) {
                return false;
            }
        }
        seen
    }
    match samples {
        Samples::U8(s) => judge(s, cc, n, 255, 1),
        Samples::U16(s) => judge(s, cc, n, 65535, 257),
        Samples::F32(_) => false,
    }
}

impl Planes {
    pub fn plane_count(&self) -> usize {
        self.color_channels + self.alpha as usize
    }

    /// The format [`Planes::to_wide`] gives: 16-bit RGB and grey-like (grey, duotone,
    /// multichannel) samples only.
    pub fn wide_format(&self) -> Option<PixelFormat> {
        if !matches!(self.samples, Samples::U16(_)) {
            return None;
        }
        convert::wide_format(self.mode, self.color_channels, self.alpha)
    }
}

/// Decode settings.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Allow several threads (rayon).
    pub parallel: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { parallel: true }
    }
}

/// A parsed PSD/PSB: header, resources and layer list. Holds no borrow of the file; decode
/// methods take the same bytes again.
#[derive(Clone, Debug)]
pub struct Psd {
    info: Info,
    resources: Resources,
    layers: Vec<Layer>,
    /// Image data after the compression word.
    image_data: Range<usize>,
    /// Pattern records (`Patt`, `Pat2`, `Pat3` blocks) that pattern fills name.
    patterns: Vec<Range<usize>>,
}

/// Cheap signature check (needs 6 bytes).
pub fn probe(head: &[u8]) -> bool {
    head.len() >= 6 && &head[..4] == b"8BPS" && matches!(head[4..6], [0, 1] | [0, 2])
}

/// Parse header, resources and layer list.
pub fn parse(data: &[u8]) -> Result<Psd> {
    parse::parse(data)
}

impl Psd {
    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Embedded ICC profile.
    pub fn icc<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        data.get(self.resources.icc.clone()?)
    }

    pub fn xmp<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        data.get(self.resources.xmp.clone()?)
    }

    /// EXIF (resource 1058, else 1059) as a TIFF-structured block.
    pub fn exif<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        data.get(
            self.resources
                .exif
                .clone()
                .or(self.resources.exif3.clone())?,
        )
    }

    pub fn iptc<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        data.get(self.resources.iptc.clone()?)
    }

    /// The embedded preview and its encoded bytes.
    pub fn thumbnail<'a>(&self, data: &'a [u8]) -> Option<(&Thumbnail, &'a [u8])> {
        let t = self.resources.thumbnail.as_ref()?;
        Some((t, data.get(t.data.clone())?))
    }

    /// Indices into [`Psd::layers`] of the layers with pixels ([`Layer::has_pixels`]), top-most
    /// first as Photoshop's Layers panel lists them; hidden layers included. Empty for colour
    /// modes whose layers this crate does not decode (Bitmap, Indexed, Multichannel: Photoshop
    /// keeps those single-layer anyway).
    pub fn pixel_layers(&self) -> Vec<usize> {
        if !layer::mode_supported(self.info.mode) {
            return Vec::new();
        }
        (0..self.layers.len())
            .rev()
            .filter(|&i| self.layers[i].has_pixels())
            .collect()
    }

    /// Decode layer `index` (of [`Psd::layers`]) alone onto the document canvas at native
    /// depth: colour planes plus a transparency plane that is zero outside the layer's
    /// rectangle and carries the layer's transparency channel, user mask, opacity and fill
    /// opacity. Blend mode, clipping, effects and vector masks are ignored. Only this layer's
    /// channels are read; `cancel`, when raised, stops the decode with [`Error::Cancelled`].
    pub fn decode_layer_planes(
        &self,
        data: &[u8],
        index: usize,
        opts: &Options,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<Planes> {
        layer::planes(self, data, index, opts, cancel)
    }

    /// Decode the composite's colour (+ alpha) channels at native depth. Without a merged
    /// image ([`Info::composite`] not [`Composite::Present`]) it is built from the layers
    /// when [`Psd::composites_layers`] says so.
    pub fn decode_planes(&self, data: &[u8], opts: &Options) -> Result<Planes> {
        if self.composites_layers() {
            return compose::composite(self, data, opts, None);
        }
        decode::planes(self, data, opts)
    }

    /// The file has no merged image and its composite is built from the layers instead:
    /// pixel, fill and shape layers with their transparency, user and vector masks, opacity,
    /// blend modes, clipping and groups (with their masks), on a transparent canvas
    /// with their adjustment layers, layer effects and artboards (what is left out:
    /// [`Psd::composite_notes`], [`Psd::undrawn_layers`]).
    pub fn composites_layers(&self) -> bool {
        self.info.composite != Composite::Present && compose::possible(self)
    }

    /// Whether [`Psd::compose_layers`] can build a composite from the layers: a colour mode
    /// whose layers are decoded and at least one layer record.
    pub fn can_compose_layers(&self) -> bool {
        compose::possible(self)
    }

    /// The composite built from the layers, even when a merged image is stored (to compare
    /// the two, or to view the file by its layers). Fails when
    /// [`Psd::can_compose_layers`] is false.
    pub fn compose_layers(&self, data: &[u8], opts: &Options) -> Result<Planes> {
        compose::composite(self, data, opts, None)
    }

    /// Layer `index`'s enabled effects and their settings, one per line: for oracle tests.
    #[doc(hidden)]
    pub fn describe_effects(&self, data: &[u8], index: usize) -> String {
        self.layers
            .get(index)
            .map_or_else(String::new, |l| effects::describe(data, l))
    }

    /// [`Psd::compose_layers`] without the layer effects named in `skip` (Photoshop's
    /// names, as in [`CompositeNotes`]): for oracle tests that measure one effect.
    #[doc(hidden)]
    pub fn compose_layers_without_effects(
        &self,
        data: &[u8],
        opts: &Options,
        skip: &[&str],
    ) -> Result<Planes> {
        let mask = effects::kind_bits(skip);
        compose::composite_skipping(self, data, opts, None, mask)
    }

    /// The artboards, top-most first as the Layers panel lists them (hidden ones too).
    pub fn artboards(&self, data: &[u8]) -> Vec<Artboard> {
        compose::artboards(self, data)
    }

    /// The composite of artboard `a` alone: what [`Psd::decode_planes`] gives, cropped to
    /// its rectangle (the part over the canvas). Built from the layers when the file has
    /// no merged image.
    pub fn decode_artboard_planes(
        &self,
        data: &[u8],
        a: &Artboard,
        opts: &Options,
    ) -> Result<Planes> {
        let p = self.decode_planes(data, opts)?;
        Ok(compose::crop(&p, a))
    }

    /// What the composite built from the layers leaves out or approximates, for features
    /// that are visible.
    pub fn composite_notes(&self, data: &[u8]) -> CompositeNotes {
        compose::notes(self, data)
    }

    /// Shape or fill layer `index` painted from its settings alone (fill, vector mask,
    /// stroke, user mask), even when it stores its pixels: colour planes then transparency,
    /// 0..1, canvas-sized. For comparing with the stored pixels.
    #[doc(hidden)]
    pub fn layer_from_settings(&self, data: &[u8], index: usize) -> Result<Option<Vec<f32>>> {
        compose::from_settings(self, data, index)
    }

    /// Visible adjustment layers a composite built from the layers leaves out.
    pub fn undrawn_layers(&self, data: &[u8]) -> usize {
        compose::skipped(self, data)
    }

    /// Decode the composite to 8-bit. Grey (no alpha) → [`PixelFormat::Gray8`], everything else
    /// → [`PixelFormat::Rgba8`]. 8-bit RGB/Grey/Indexed/Bitmap/Duotone/Multichannel take a fused
    /// fast path; CMYK, Lab, 16- and 32-bit go through the naive [`Planes::to_rgba8`].
    pub fn decode_rgba8(&self, data: &[u8], opts: &Options) -> Result<Image> {
        if self.composites_layers() {
            return Ok(compose::composite(self, data, opts, None)?.to_rgba8());
        }
        decode::rgba8(self, data, opts)
    }

    /// The format [`Psd::decode_wide`] gives, if any: 16-bit RGB → [`PixelFormat::Rgba16`],
    /// 16-bit grey / duotone / single-channel multichannel → [`PixelFormat::Gray16`] (with
    /// alpha [`PixelFormat::GrayAlpha16`]). Other modes and depths have none.
    pub fn wide_format(&self) -> Option<PixelFormat> {
        let i = &self.info;
        if i.depth != 16 {
            return None;
        }
        convert::wide_format(i.mode, i.mode.color_channels(i.channels), i.has_alpha)
    }

    /// Decode a 16-bit composite at 16 bits (see [`Psd::wide_format`]); the same pixels as
    /// [`Psd::decode_rgba8`] before rounding. `Ok(None)` when the file has no wide format.
    pub fn decode_wide(&self, data: &[u8], opts: &Options) -> Result<Option<Image>> {
        if self.wide_format().is_none() {
            return Ok(None);
        }
        Ok(self.decode_planes(data, opts)?.to_wide(opts.parallel))
    }
}

pub(crate) fn check_composite(info: &Info) -> Result<Compression> {
    match (info.composite, info.compression) {
        (Composite::Present, Some(c)) => Ok(c),
        (Composite::Present, None) => Err(Error::NoComposite(Composite::Missing)),
        (c, _) => Err(Error::NoComposite(c)),
    }
}
