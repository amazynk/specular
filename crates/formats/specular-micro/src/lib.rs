//! `specular-micro`: vendor light-microscopy readers for Specular.
//!
//! * **[`lif`]: Leica LIF and LOF.** The UTF-16 XML header and the memory blocks it names
//!   (both block header versions); every image series of the element tree as its own image;
//!   dimensions by `DimID` (X, Y, Z, T, wavelengths, rotation, tile/mosaic positions) with
//!   their origins, lengths and units; channels with their resolution (bit depth), data type,
//!   LUT name and byte offset; samples addressed by each dimension's `BytesInc`, so padded
//!   rows and any interleaving read the same way. LOF (a single image with its own XML) is the
//!   same reader.
//! * **[`nd2`]: Nikon ND2.** The chunk-mapped layout (versions 2 and 3): the chunk map at the
//!   end of the file, CLX Lite binary metadata (or its XML form in version 2 files), the
//!   image attributes, the experiment's loops (time, XY positions, Z stacks, multi-phase time),
//!   channels with names and colours, calibration, text info; raw or zlib-compressed frames
//!   with padded rows and interleaved components. The legacy layout (version 1, a JPEG 2000
//!   box file with a box map at its end) decodes each channel's codestream through
//!   `specular-jp2`.
//!
//! * **[`oir`]: Olympus FluoView OIR.** XML documents (frame and image properties, colour
//!   tables) between raw pixel blocks named by plane (λ, Z, T), channel and band.
//! * **[`ics`]: Image Cytometry Standard 1.0 and 2.0.** The text header with its `.ids`
//!   file, inline data or a source file; gzip or compress; every sample type, byte order and
//!   dimension order; units and scales per dimension.
//! * **[`spe`] and [`sif`]: spectroscopy cameras** (Princeton Instruments SPE 2.x and 3.0,
//!   Andor SIF): frames and regions as [`spectral::Page`]s with their wavelength calibration,
//!   exposure, temperature and gain.
//!
//! Headers parse first without touching the pixels; planes are then read one at a time as
//! `f32` values, rows top-down. Malformed input never panics; sizes are checked before
//! anything is allocated.

pub mod clx;
pub mod ics;
pub mod lif;
pub mod nd2;
pub mod oir;
pub mod sif;
pub mod spe;
pub mod spectral;

/// Largest number of samples in one plane.
pub const MAX_PLANE: u64 = 1 << 30;

/// Errors from the readers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a Leica image file")]
    NotLif,
    #[error("not an ND2 file")]
    NotNd2,
    #[error("truncated: {0}")]
    Truncated(&'static str),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("image too large")]
    TooLarge,
    #[error("no image in the file")]
    NoImage,
    /// The memory budget cannot hold what the file declares.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// How one sample is stored (little-endian).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl Sample {
    pub fn bytes(self) -> usize {
        match self {
            Sample::U8 => 1,
            Sample::U16 => 2,
            Sample::U32 | Sample::F32 => 4,
            Sample::U64 | Sample::F64 => 8,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Sample::F32 | Sample::F64)
    }

    /// The sample at `b[..self.bytes()]`.
    #[inline]
    pub fn read(self, b: &[u8]) -> f32 {
        match self {
            Sample::U8 => f32::from(b[0]),
            Sample::U16 => f32::from(u16::from_le_bytes([b[0], b[1]])),
            Sample::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
            Sample::U64 => u64::from_le_bytes(b[..8].try_into().unwrap_or([0; 8])) as f32,
            Sample::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            Sample::F64 => f64::from_le_bytes(b[..8].try_into().unwrap_or([0; 8])) as f32,
        }
    }
}

/// A display colour from a vendor's colour name (Leica LUT names, ImageJ-like names):
/// display-coded 0..=1, `None` for grey or an unknown name.
pub fn named_colour(name: &str) -> Option<[f32; 3]> {
    let n = name.trim().to_ascii_lowercase();
    Some(match n.as_str() {
        "red" => [1.0, 0.0, 0.0],
        "green" => [0.0, 1.0, 0.0],
        "blue" => [0.0, 0.0, 1.0],
        "cyan" => [0.0, 1.0, 1.0],
        "magenta" => [1.0, 0.0, 1.0],
        "yellow" => [1.0, 1.0, 0.0],
        "orange" => [1.0, 0.5, 0.0],
        "gray" | "grey" | "white" => [1.0, 1.0, 1.0],
        _ => return None,
    })
}

/// Little-endian readers over a byte slice that fail instead of panicking.
pub(crate) fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub(crate) fn le_u64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        b.get(at..at.checked_add(8)?)?.try_into().ok()?,
    ))
}

pub(crate) fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// UTF-16LE code units to a string (lossy), stopping at a NUL.
pub(crate) fn utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}
