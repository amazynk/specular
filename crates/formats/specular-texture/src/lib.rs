//! GPU texture containers for Specular: DDS (DX9 and DX10 headers), KTX2, KTX (version 1),
//! PowerVR PVR v3, Valve VTF 7.0–7.5, the `.astc` file and the Basis Universal `.basis` file,
//! decoded on the CPU to 8-bit RGBA.
//!
//! * Block compression BC1–BC7 (BC6H signed and unsigned), forked from bcdec / bcdec_rs
//!   (see `bcn.rs`), decoded in parallel by block rows; a region decodes only the blocks it
//!   touches.
//! * ETC1, ETC2 (RGB, punch-through, RGBA) and EAC R11 / RG11 (`etc.rs`); ASTC LDR and HDR,
//!   every 2D and 3D footprint (`astc.rs`; a 3D `.astc` file's depth slices are its pages);
//!   PVRTC1 2 and 4 bpp (`pvrtc.rs`); Basis Universal ETC1S (BasisLZ) and UASTC LDR 4×4
//!   (`basis/`), UASTC HDR 4×4 and ASTC HDR 6×6 (KTX2 and `.basis`; their blocks are ASTC HDR
//!   blocks). All own decoders, bit-exact against etcpack, astcenc, the PowerVR SDK and basisu
//!   (corpus and fixture tests).
//! * Basis Universal v2 intermediate codecs (KTX2 schemes 4–6 and the v2.0 BasisLZ layout,
//!   `.basis` formats 4, 5–18 and 33), transcoded image by image: UASTC HDR 6×6i to ASTC HDR
//!   6×6 (`basis/hdr6x6i.rs`), XUASTC LDR in all three profiles to ASTC LDR (`basis/xuastc*`),
//!   XUBC7 to BC7 (`basis/xubc7.rs`); written from Binomial's published specifications, every
//!   block equal to the basisu v2 transcoder's (the opengl_basisu wheel).
//!
//! Refused by name: PVRTC2 (Imagination publishes no specification of its hard-transition
//! and non-interpolated modes, and no decoder outside its proprietary tools exists to check
//! against).
//! * Uncompressed formats: any integer channel layout up to 32 bits per pixel given by masks
//!   (RGBA8, BGRA8, BGRX8, RGB8, BGR8, R8, RG8, L8, LA8, A8, 565, 5551, 4444, 10:10:10:2),
//!   16-bit UNORM, 16- and 32-bit float, R11G11B10F and RGB9E5.
//! * KTX2 supercompression: none, Zstandard (ruzstd), zlib (zlib-rs) and BasisLZ.
//! * Mip levels serve downscaled requests (level k is 1/2^k); other factors are box filtered
//!   in linear light, alpha weighted.
//!
//! Array layers, cube faces and depth slices are images ([`Texture::image_count`]); one is
//! selected with [`Texture::select_image`] and decoded.
//!
//! Conversions to 8 bits (documented, lossy by design):
//! * 16-bit UNORM is rounded to 8 bits.
//! * Float data (half, float, BC6H, ASTC HDR, R11G11B10F, RGB9E5) is taken as linear light,
//!   clamped to [0, 1] and sRGB encoded (no tone mapping); float alpha is clamped and kept
//!   linear.
//! * One-channel formats (R, L, A, BC4, EAC R11) are shown as grey; two-channel formats (RG,
//!   BC5, EAC RG11, Basis RG) as (R, G, 0). Signed (SNORM) data maps −1…1 to 0…255.
//! * 8-bit sRGB and UNORM variants are passed through unchanged (both are displayed as sRGB).

mod astc;
mod astc_file;
mod basis;
mod basis_file;
mod bcn;
mod dds;
mod decode;
mod etc;
mod ftex;
#[cfg(test)]
mod hostile;
mod ktx1;
mod ktx2;
mod pkm;
mod pvr;
mod pvrtc;
mod vtf;

pub use decode::decode_blocks;

use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Largest accepted width or height.
pub const MAX_DIMENSION: u32 = 1 << 17;
/// Largest accepted decoded output (RGBA8 bytes).
pub const MAX_OUTPUT_BYTES: u64 = 4 << 30;
/// Largest BC1 top level [`Texture::has_alpha`] scans for transparent texels (32 MiB).
pub const BC1_SCAN_MAX: usize = 32 << 20;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("truncated: {0}")]
    Truncated(&'static str),
    #[error("malformed: {0}")]
    Malformed(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    #[error("cancelled")]
    Cancelled,
    #[error("region outside the image")]
    BadRegion,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Pixel encoding of the texture data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// BC1; `alpha` false for the RGB-only variants (punch-through texels become opaque).
    Bc1 {
        alpha: bool,
    },
    Bc2,
    Bc3,
    Bc4 {
        signed: bool,
    },
    Bc5 {
        signed: bool,
    },
    Bc6h {
        signed: bool,
    },
    Bc7,
    /// Integer UNORM pixels of `bytes` (1–4) little-endian bytes; channels by bit masks
    /// (R, G, B, A). With `grey`, the R mask is a luminance channel.
    Masked {
        bytes: u8,
        masks: [u32; 4],
        grey: bool,
    },
    /// 16-bit UNORM with 1–4 channels (R, RG, RGB, RGBA).
    Unorm16 {
        channels: u8,
    },
    /// 8-bit SNORM with 1, 2 or 4 channels.
    Snorm8 {
        channels: u8,
    },
    /// Half float with 1–4 channels.
    Half {
        channels: u8,
    },
    /// 32-bit float with 1–4 channels.
    Float {
        channels: u8,
    },
    R11G11B10F,
    Rgb9e5,
    /// ETC1 (decoded as the ETC2 RGB superset).
    Etc1,
    Etc2Rgb,
    /// ETC2 RGB with punch-through (1-bit) alpha.
    Etc2Rgba1,
    /// ETC2 RGB with an EAC alpha block.
    Etc2Rgba,
    /// EAC R11 (one 11-bit channel), unsigned or signed.
    EacR11 {
        signed: bool,
    },
    /// EAC RG11 (two 11-bit channels).
    EacRg11 {
        signed: bool,
    },
    /// ASTC with a `w`×`h`(×`d`) footprint (`d` 1: 2D); `hdr` decodes with the HDR
    /// profile (float output). A 3D block holds `d` slices of the image's depth.
    Astc {
        w: u8,
        h: u8,
        d: u8,
        hdr: bool,
    },
    /// PVRTC1 at 2 or 4 bits per pixel; `alpha` false for the RGB variants.
    Pvrtc {
        two_bpp: bool,
        alpha: bool,
    },
    /// Basis Universal ETC1S (KTX2 BasisLZ), as transcoded to ETC1 blocks: 8 bytes, or 16
    /// with an alpha block. `rg` for two-channel data (R in the colour slice, G in the alpha
    /// slice), shown as (R, G, 0).
    Etc1s {
        alpha: bool,
        rg: bool,
    },
    /// Basis Universal UASTC LDR 4×4. `rg` for R in colour and G in alpha (RRRG), shown as
    /// (R, G, 0).
    Uastc {
        alpha: bool,
        rg: bool,
    },
}

impl Format {
    /// (block width, block height, bytes per block); 1×1 for pixel formats.
    pub fn block(self) -> (u32, u32, usize) {
        match self {
            Format::Bc1 { .. } | Format::Bc4 { .. } => (4, 4, 8),
            Format::Bc2 | Format::Bc3 | Format::Bc5 { .. } | Format::Bc6h { .. } | Format::Bc7 => {
                (4, 4, 16)
            }
            Format::Masked { bytes, .. } => (1, 1, bytes as usize),
            Format::Unorm16 { channels } | Format::Half { channels } => {
                (1, 1, 2 * channels as usize)
            }
            Format::Snorm8 { channels } => (1, 1, channels as usize),
            Format::Float { channels } => (1, 1, 4 * channels as usize),
            Format::R11G11B10F | Format::Rgb9e5 => (1, 1, 4),
            Format::Etc1 | Format::Etc2Rgb | Format::Etc2Rgba1 | Format::EacR11 { .. } => (4, 4, 8),
            Format::Etc2Rgba | Format::EacRg11 { .. } => (4, 4, 16),
            Format::Astc { w, h, .. } => (u32::from(w), u32::from(h), 16),
            Format::Pvrtc { two_bpp, .. } => (if two_bpp { 8 } else { 4 }, 4, 8),
            Format::Etc1s { alpha, .. } => (4, 4, if alpha { 16 } else { 8 }),
            Format::Uastc { .. } => (4, 4, 16),
        }
    }

    pub fn is_compressed(self) -> bool {
        self.block().0 > 1
    }

    pub fn has_alpha(self) -> bool {
        match self {
            Format::Bc1 { alpha } => alpha,
            Format::Bc2
            | Format::Bc3
            | Format::Bc7
            | Format::Etc2Rgba1
            | Format::Etc2Rgba
            | Format::Astc { .. } => true,
            Format::Pvrtc { alpha, .. } => alpha,
            Format::Etc1s { alpha, rg } | Format::Uastc { alpha, rg } => alpha && !rg,
            Format::Masked { masks, grey, .. } => masks[3] != 0 && !(grey && masks[0] == 0),
            Format::Unorm16 { channels }
            | Format::Half { channels }
            | Format::Float { channels }
            | Format::Snorm8 { channels } => channels == 4,
            _ => false,
        }
    }

    /// Whether the data is floating point (decoded with sRGB encoding of linear values).
    pub fn is_float(self) -> bool {
        matches!(
            self,
            Format::Bc6h { .. }
                | Format::Half { .. }
                | Format::Float { .. }
                | Format::R11G11B10F
                | Format::Rgb9e5
                | Format::Astc { hdr: true, .. }
        )
    }

    /// Bits per channel of the source data, for display.
    pub fn bit_depth(self) -> u8 {
        match self {
            Format::Unorm16 { .. }
            | Format::Half { .. }
            | Format::Bc6h { .. }
            | Format::Astc { hdr: true, .. } => 16,
            Format::EacR11 { .. } | Format::EacRg11 { .. } => 11,
            Format::Float { .. } => 32,
            Format::Masked { masks, .. } => {
                masks.iter().map(|m| m.count_ones()).max().unwrap_or(8) as u8
            }
            Format::R11G11B10F => 11,
            Format::Rgb9e5 => 9,
            _ => 8,
        }
    }

    pub fn name(self) -> String {
        match self {
            Format::Bc1 { alpha: true } => "BC1".into(),
            Format::Bc1 { alpha: false } => "BC1 RGB".into(),
            Format::Bc2 => "BC2".into(),
            Format::Bc3 => "BC3".into(),
            Format::Bc4 { signed } => format!("BC4{}", if signed { " signed" } else { "" }),
            Format::Bc5 { signed } => format!("BC5{}", if signed { " signed" } else { "" }),
            Format::Bc6h { signed } => format!("BC6H {}", if signed { "SF16" } else { "UF16" }),
            Format::Bc7 => "BC7".into(),
            Format::Masked { bytes, masks, grey } => {
                let names = if grey {
                    ["L", "", "", "A"]
                } else {
                    ["R", "G", "B", "A"]
                };
                let mut parts: Vec<(u32, String)> = masks
                    .iter()
                    .zip(names)
                    .filter(|(m, n)| **m != 0 && !n.is_empty())
                    .map(|(m, n)| (m.trailing_zeros(), format!("{n}{}", m.count_ones())))
                    .collect();
                parts.sort_by_key(|p| std::cmp::Reverse(p.0));
                let s: String = parts.into_iter().map(|p| p.1).collect();
                if s.is_empty() {
                    format!("{}-bit", bytes * 8)
                } else {
                    s
                }
            }
            Format::Unorm16 { channels } => format!("{} 16-bit", channel_names(channels)),
            Format::Snorm8 { channels } => format!("{} 8-bit signed", channel_names(channels)),
            Format::Half { channels } => format!("{} half float", channel_names(channels)),
            Format::Float { channels } => format!("{} float", channel_names(channels)),
            Format::R11G11B10F => "R11G11B10 float".into(),
            Format::Rgb9e5 => "RGB9E5".into(),
            Format::Etc1 => "ETC1".into(),
            Format::Etc2Rgb => "ETC2 RGB".into(),
            Format::Etc2Rgba1 => "ETC2 RGB A1".into(),
            Format::Etc2Rgba => "ETC2 RGBA".into(),
            Format::EacR11 { signed } => format!("EAC R11{}", if signed { " signed" } else { "" }),
            Format::EacRg11 { signed } => {
                format!("EAC RG11{}", if signed { " signed" } else { "" })
            }
            Format::Astc { w, h, d, hdr } => format!(
                "ASTC {w}×{h}{}{}",
                if d > 1 {
                    format!("×{d}")
                } else {
                    String::new()
                },
                if hdr { " HDR" } else { "" }
            ),
            Format::Pvrtc { two_bpp, alpha } => format!(
                "PVRTC {} bpp{}",
                if two_bpp { 2 } else { 4 },
                if alpha { "" } else { " RGB" }
            ),
            Format::Etc1s { alpha, rg } => format!("Basis ETC1S{}", basis_channels(alpha, rg)),
            Format::Uastc { alpha, rg } => format!("Basis UASTC{}", basis_channels(alpha, rg)),
        }
    }
}

fn basis_channels(alpha: bool, rg: bool) -> &'static str {
    match (alpha, rg) {
        (_, true) => " RG",
        (true, false) => " RGBA",
        (false, false) => " RGB",
    }
}

fn channel_names(n: u8) -> &'static str {
    ["R", "R", "RG", "RGB", "RGBA"][n.min(4) as usize]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Dds,
    Ktx1,
    Ktx2,
    /// PowerVR PVR version 3.
    Pvr,
    /// Valve Texture Format.
    Vtf,
    /// The `.astc` file of astcenc (one 2D image).
    Astc,
    /// The `.basis` file of Basis Universal.
    Basis,
    /// Ericsson's PKM (ETC1, ETC2, EAC).
    Pkm,
    /// Firaxis FTEX (`.ftc`, `.ftu`).
    Ftex,
}

impl Container {
    pub fn name(self) -> &'static str {
        match self {
            Container::Dds => "DDS",
            Container::Ktx1 => "KTX",
            Container::Ktx2 => "KTX2",
            Container::Pvr => "PVR",
            Container::Vtf => "VTF",
            Container::Astc => "ASTC",
            Container::Basis => "Basis",
            Container::Pkm => "PKM",
            Container::Ftex => "Firaxis",
        }
    }
}

/// Where the images (array layers, cube faces, depth slices) of a level are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
    /// Each level holds all images, ordered layer, face, slice (KTX, KTX2, PVR, VTF); a
    /// level's `range` covers them all.
    PerLevel,
    /// Each layer and face stores its whole mip chain, `chain` bytes apart (DDS); a level's
    /// `range` covers the slices of the first chain.
    PerChain { chain: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Supercompression {
    None,
    Zstd,
    Zlib,
    /// BasisLZ (ETC1S); the global data is in [`Texture::basis`].
    BasisLz,
    /// A Basis Universal v2 intermediate codec, transcoded to the format's blocks; each image's
    /// bytes are found through [`Texture::seek`].
    Basis2(Basis2),
}

/// The Basis Universal intermediate codecs transcoded image by image to GPU blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Basis2 {
    /// UASTC HDR 6×6 intermediate → ASTC HDR 6×6.
    Hdr6x6i,
    /// XUASTC LDR → ASTC LDR of the same footprint.
    Xuastc,
    /// XUBC7 → BC7.
    Xubc7,
}

/// The BasisLZ global data of a KTX2 file, shared by clones of its [`Texture`].
#[derive(Clone)]
pub(crate) struct BasisGlobal(pub(crate) Arc<basis::etc1s::Etc1sGlobal>);

impl std::fmt::Debug for BasisGlobal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BasisGlobal")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Level {
    pub width: u32,
    pub height: u32,
    /// Bytes of the level in the file (compressed when supercompressed).
    pub range: Range<usize>,
    /// Size after supercompression is undone.
    pub uncompressed_len: usize,
}

/// A parsed texture file.
#[derive(Clone, Debug)]
pub struct Texture {
    pub container: Container,
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// Array layers (1 for a plain texture).
    pub layers: u32,
    /// 6 for a cube map.
    pub faces: u32,
    /// The data is tagged sRGB (8-bit formats are decoded unchanged either way).
    pub srgb: bool,
    /// Rows are stored bottom-up (KTX2 `KTXorientation` "ru"): show flipped vertically.
    pub flip_y: bool,
    /// Colour is stored premultiplied by alpha (DDS DXT2/DXT4, a DX10 header with alpha
    /// mode 2 or a KTX2 DFD with KHR_DF_FLAG_ALPHA_PREMULTIPLIED, on an integer format). Decoding divides it out: the output is straight alpha.
    pub premultiplied: bool,
    pub(crate) levels: Vec<Level>,
    pub(crate) supercompression: Supercompression,
    pub(crate) layout: Layout,
    /// The image shown, 0..[`Texture::image_count`].
    pub(crate) image: u32,
    /// Uncompressed rows are padded to a multiple of this many bytes (4 in KTX).
    pub(crate) row_align: u32,
    /// BasisLZ global data (codebooks, image descriptors) when supercompressed so.
    pub(crate) basis: Option<BasisGlobal>,
    /// [`Supercompression::Basis2`]: per image (level-major, then layer, face), its bytes'
    /// offset from the level's `range` start and length.
    pub(crate) seek: Option<Arc<[(usize, usize)]>>,
}

/// Region in output (scaled) coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct DecodeOptions<'a> {
    /// Downscale factor (≥ 1); output is ceil(width / scale) × ceil(height / scale).
    pub scale: u32,
    pub region: Option<Region>,
    pub cancel: Option<&'a AtomicBool>,
    pub parallel: bool,
}

impl Default for DecodeOptions<'_> {
    fn default() -> Self {
        DecodeOptions {
            scale: 1,
            region: None,
            cancel: None,
            parallel: true,
        }
    }
}

/// Decoded RGBA8 pixels.
#[derive(Clone, Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// Top-left of the pixels in the scaled image.
    pub origin: (u32, u32),
    /// Bytes per row (width × 4).
    pub stride: usize,
    pub data: Vec<u8>,
    /// Some blocks were missing from the file (shown as transparent).
    pub truncated: bool,
}

/// Whether the bytes start like a DDS file.
pub fn is_dds(head: &[u8]) -> bool {
    head.starts_with(b"DDS ")
}

/// Whether the bytes start like a KTX2 file.
pub fn is_ktx2(head: &[u8]) -> bool {
    head.starts_with(&ktx2::IDENTIFIER)
}

/// Whether the bytes start like a KTX (version 1) file.
pub fn is_ktx1(head: &[u8]) -> bool {
    head.starts_with(&ktx1::IDENTIFIER)
}

/// Whether the bytes start like a PVR version 3 file (either byte order).
pub fn is_pvr(head: &[u8]) -> bool {
    head.starts_with(b"PVR\x03") || head.starts_with(b"\x03RVP")
}

/// Whether the bytes start like a Valve Texture Format file.
pub fn is_vtf(head: &[u8]) -> bool {
    head.starts_with(b"VTF\0")
}

/// Whether the bytes start like an `.astc` file.
pub fn is_astc(head: &[u8]) -> bool {
    head.starts_with(&astc_file::MAGIC)
}

/// Whether the bytes start like a Basis Universal `.basis` file.
pub fn is_basis(head: &[u8]) -> bool {
    basis_file::is_basis(head)
}

/// Whether the bytes start like a PKM (ETC) file.
pub fn is_pkm(head: &[u8]) -> bool {
    pkm::is_pkm(head)
}

/// Whether the bytes start like a Firaxis FTEX file.
pub fn is_ftex(head: &[u8]) -> bool {
    head.starts_with(ftex::MAGIC)
}

/// Whether the bytes start like any container this crate reads.
pub fn is_texture(head: &[u8]) -> bool {
    is_dds(head)
        || is_ktx1(head)
        || is_ktx2(head)
        || is_pvr(head)
        || is_vtf(head)
        || is_astc(head)
        || is_basis(head)
        || is_pkm(head)
        || is_ftex(head)
}

impl Texture {
    /// Parse a DDS, KTX, KTX2, PVR, VTF, `.astc` or `.basis` header and level index. The first image is
    /// selected.
    pub fn open(data: &[u8]) -> Result<Texture> {
        let t = if is_dds(data) {
            dds::parse(data)?
        } else if is_ktx2(data) {
            ktx2::parse(data)?
        } else if is_ktx1(data) {
            ktx1::parse(data)?
        } else if is_pvr(data) {
            pvr::parse(data)?
        } else if is_vtf(data) {
            vtf::parse(data)?
        } else if is_astc(data) {
            astc_file::parse(data)?
        } else if is_basis(data) {
            basis_file::parse(data)?
        } else if is_pkm(data) {
            pkm::parse(data)?
        } else if is_ftex(data) {
            ftex::parse(data)?
        } else {
            return Err(Error::Malformed("not a texture file"));
        };
        if t.width == 0 || t.height == 0 {
            return Err(Error::Malformed("zero size"));
        }
        if t.width > MAX_DIMENSION || t.height > MAX_DIMENSION {
            return Err(Error::TooLarge);
        }
        if t.levels.is_empty() {
            return Err(Error::Truncated("no image data"));
        }
        if let Format::Astc { w, h, d, .. } = t.format
            && !astc::valid_footprint_3d(w.into(), h.into(), d.into())
        {
            return Err(Error::Malformed("ASTC footprint"));
        }
        Ok(t)
    }

    /// Number of images: array layers × cube faces × depth slices (each shown as a page).
    pub fn image_count(&self) -> u32 {
        self.layers
            .saturating_mul(self.faces)
            .saturating_mul(self.depth)
            .max(1)
    }

    /// The image [`Texture::decode`] shows.
    pub fn image(&self) -> u32 {
        self.image
    }

    /// Show image `i` (0..[`Texture::image_count`]); false when out of range.
    pub fn select_image(&mut self, i: u32) -> bool {
        if i < self.image_count() {
            self.image = i;
            true
        } else {
            false
        }
    }

    /// (layer, face, slice) of the selected image.
    pub fn image_position(&self) -> (u32, u32, u32) {
        let (f, d) = (self.faces.max(1), self.depth.max(1));
        (
            self.image / f.saturating_mul(d),
            (self.image / d) % f,
            self.image % d,
        )
    }

    /// Bytes of an uncompressed row of `w` pixels, with padding.
    pub(crate) fn row_bytes(&self, w: u32) -> u64 {
        let raw = u64::from(w) * self.format.block().2 as u64;
        raw.next_multiple_of(u64::from(self.row_align.max(1)))
    }

    /// Bytes of one image of `w`×`h` pixels as stored (uncompressed rows padded).
    pub(crate) fn image_size(&self, w: u32, h: u32) -> u64 {
        if self.format.is_compressed() {
            image_bytes(self.format, w, h)
        } else {
            self.row_bytes(w).saturating_mul(u64::from(h))
        }
    }

    /// Index of the selected image within level `k` (whose depth is depth >> k).
    pub(crate) fn image_in_level(&self, k: usize) -> u64 {
        let (layer, face, z) = self.image_position();
        let dk = (self.depth >> k).max(1);
        // 3D ASTC: a layer of blocks holds `d` slices.
        let bd = self.slices_per_block();
        let (dk, z) = (dk.div_ceil(bd), (z >> k).min(dk - 1) / bd);
        (u64::from(layer) * u64::from(self.faces.max(1)) + u64::from(face)) * u64::from(dk)
            + u64::from(z)
    }

    /// Depth slices per block: the block depth of 3D ASTC, else 1.
    pub(crate) fn slices_per_block(&self) -> u32 {
        match self.format {
            Format::Astc { d, .. } => u32::from(d.max(1)),
            _ => 1,
        }
    }

    /// The selected slice's index within its block's depth (3D ASTC), at level `k`.
    pub(crate) fn slice_in_block(&self, k: usize) -> usize {
        let dk = (self.depth >> k).max(1);
        let z = (self.image_position().2 >> k).min(dk - 1);
        (z % self.slices_per_block()) as usize
    }

    /// KTX2 levels are zstd / zlib / BasisLZ supercompressed: a region decode inflates (or
    /// transcodes) the whole image.
    pub fn is_supercompressed(&self) -> bool {
        self.supercompression != Supercompression::None
    }

    /// Whether the image has transparency. [`Format::has_alpha`], except that BC1 (DXT1),
    /// whose blocks choose per block between four colours and three plus transparent black,
    /// counts only when some block of the top level uses that transparent texel. The scan
    /// reads 8 bytes per 16 texels, so it runs only up to [`BC1_SCAN_MAX`] bytes (an 8192²
    /// texture) and on levels stored as is; past that the format answers.
    pub fn has_alpha(&self, data: &[u8]) -> bool {
        if self.format != (Format::Bc1 { alpha: true })
            || self.is_supercompressed()
            || self.image != 0
        {
            return self.format.has_alpha();
        }
        let l = &self.levels[0];
        if l.range.len() > BC1_SCAN_MAX {
            return true;
        }
        let Some(level) = data.get(l.range.clone()) else {
            return true;
        };
        // A truncated level decodes its missing blocks as transparent.
        level.len() < image_bytes(self.format, l.width, l.height) as usize
            || level.chunks_exact(8).any(|b| {
                let c0 = u16::from_le_bytes([b[0], b[1]]);
                let c1 = u16::from_le_bytes([b[2], b[3]]);
                let idx = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
                // Three-colour mode, and some 2-bit index is 3.
                c0 <= c1 && idx & (idx >> 1) & 0x5555_5555 != 0
            })
    }

    /// Sizes of the stored mip levels, largest first.
    pub fn level_sizes(&self) -> Vec<(u32, u32)> {
        self.levels.iter().map(|l| (l.width, l.height)).collect()
    }

    /// Short technical description, e.g. "DDS BC7, 10 mips".
    pub fn description(&self) -> String {
        let format = self.format.name();
        // "ASTC 4×4", "Basis UASTC RGB": the format already names the container.
        let container = if format.starts_with(self.container.name()) {
            String::new()
        } else {
            format!("{} ", self.container.name())
        };
        let mut s = format!(
            "{container}{format}{}",
            if self.srgb { " sRGB" } else { "" }
        );
        if self.premultiplied {
            s += " premultiplied";
        }
        if self.levels.len() > 1 {
            s += &format!(", {} mips", self.levels.len());
        }
        if self.faces == 6 {
            s += ", cube map";
        }
        if self.layers > 1 {
            s += &format!(", {} layers", self.layers);
        }
        if self.depth > 1 {
            s += &format!(", depth {}", self.depth);
        }
        match self.supercompression {
            Supercompression::Zstd => s += ", zstd",
            Supercompression::Zlib => s += ", zlib",
            Supercompression::BasisLz if self.container != Container::Basis => s += ", BasisLZ",
            Supercompression::BasisLz => {}
            Supercompression::Basis2(Basis2::Hdr6x6i) => s += ", UASTC HDR 6×6 intermediate",
            Supercompression::Basis2(Basis2::Xuastc) => s += ", XUASTC LDR",
            Supercompression::Basis2(Basis2::Xubc7) => s += ", XUBC7",
            Supercompression::None => {}
        }
        s
    }

    /// Decode a scale and optional region to RGBA8.
    pub fn decode(&self, data: &[u8], opts: &DecodeOptions) -> Result<Decoded> {
        decode::decode(self, data, opts)
    }

    /// Estimated peak memory of [`Texture::decode`].
    pub fn memory_estimate(&self, opts: &DecodeOptions) -> u64 {
        decode::memory_estimate(self, opts)
    }
}

/// Bytes of one image (one layer, face and slice) of `w`×`h` pixels.
pub(crate) fn image_bytes(f: Format, w: u32, h: u32) -> u64 {
    let (bw, bh, b) = f.block();
    if let Format::Pvrtc { .. } = f {
        // PVRTC1 levels are at least 2×2 blocks.
        return w.div_ceil(bw).max(2) as u64 * h.div_ceil(bh).max(2) as u64 * b as u64;
    }
    // Saturating: headers are not range-checked yet when this sizes their levels, and
    // 2^32 × 2^32 × 16 does not fit in a u64.
    (w.div_ceil(bw) as u64 * h.div_ceil(bh) as u64).saturating_mul(b as u64)
}

/// `$SPECULAR_LARGE_CORPUS/texture/<sub>` (default `tools/corpus/large`), when fetched.
#[cfg(test)]
pub(crate) fn test_corpus(sub: &str) -> Option<std::path::PathBuf> {
    let base = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
        });
    let d = base.join("texture").join(sub);
    d.is_dir().then_some(d)
}

/// A reference pair of `tools/corpus/texture/blocks.sh`: mode, width, height, input blocks,
/// reference decode.
#[cfg(test)]
pub(crate) type BlockRef = (String, u32, u32, Vec<u8>, Vec<u8>);

/// The `.in` / `.ref` pairs under `blocks/<dir>`, named `<anything>.<mode>.<w>x<h>.in`.
#[cfg(test)]
pub(crate) fn block_refs(dir: &str) -> Vec<BlockRef> {
    let Some(d) = test_corpus(&format!("blocks/{dir}")) else {
        return Vec::new();
    };
    let mut names: Vec<_> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "in"))
        .collect();
    names.sort();
    names
        .into_iter()
        .filter_map(|p| {
            let stem = p.file_stem()?.to_str()?.to_string();
            let mut parts = stem.rsplitn(3, '.');
            let size = parts.next()?;
            let mode = parts.next()?.to_string();
            let (w, h) = size.split_once('x')?;
            let r = std::fs::read(p.with_extension("ref")).ok()?;
            Some((
                mode,
                w.parse().ok()?,
                h.parse().ok()?,
                std::fs::read(&p).ok()?,
                r,
            ))
        })
        .collect()
}
