use thiserror::Error;

/// Errors returned by the decoder. Never panics on bad input; damaged or truncated image data
/// normally yields a partial image (see [`crate::Image::truncated`]) rather than an error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PngError {
    /// Input does not start with the PNG signature.
    #[error("not a PNG file")]
    NotPng,
    /// The file ends before the image data starts.
    #[error("truncated PNG: {0}")]
    Truncated(&'static str),
    /// Structurally invalid file (bad IHDR, ...).
    #[error("malformed PNG: {0}")]
    Malformed(&'static str),
    /// Valid but not supported.
    #[error("unsupported PNG: {0}")]
    Unsupported(&'static str),
    /// A CRC or Adler-32 check failed (only with [`crate::DecodeOptions::verify_checksums`]).
    #[error("PNG checksum: {0}")]
    Checksum(&'static str),
    /// The image data could not be decoded at all (not even a first row).
    #[error("PNG image data: {0}")]
    Data(crate::inflate::InflateError),
    /// Output would be too large to allocate.
    #[error("image too large to decode ({0} bytes)")]
    TooLarge(u64),
    /// The output fits the caps but not the memory budget's free memory now.
    #[error("{0}")]
    NeedsMemory(specular_core::NeedsMemory),
    /// The cancel flag was raised.
    #[error("decode cancelled")]
    Cancelled,
    /// No such APNG frame.
    #[error("no frame {0}")]
    NoFrame(usize),
}

impl From<specular_core::NeedsMemory> for PngError {
    fn from(n: specular_core::NeedsMemory) -> Self {
        PngError::NeedsMemory(n)
    }
}

pub type Result<T> = std::result::Result<T, PngError>;
