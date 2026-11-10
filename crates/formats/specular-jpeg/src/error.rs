use thiserror::Error;

/// Errors returned by the decoder. Never panics on bad input; corrupt entropy data
/// normally yields a (partial) image rather than an error, see [`crate::DecodedPlanes::truncated`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JpegError {
    /// Input does not start with an SOI marker.
    #[error("not a JPEG file")]
    NotJpeg,
    /// The headers end before the first scan, or the file is far too short for the declared size.
    #[error("truncated JPEG: {0}")]
    Truncated(&'static str),
    /// Structurally invalid file (bad marker lengths, bad tables, ...).
    #[error("malformed JPEG: {0}")]
    Malformed(&'static str),
    /// Valid but not supported by this decoder (or the fallback decoders).
    #[error("unsupported JPEG: {0}")]
    Unsupported(&'static str),
    /// Output would be too large to allocate.
    #[error("image too large to decode ({0} bytes)")]
    TooLarge(u64),
    /// The cancel flag was raised.
    #[error("decode cancelled")]
    Cancelled,
    /// A [`crate::RowIndex`] was used with data it was not built from.
    #[error("row index does not match this file")]
    IndexMismatch,
    /// Error reported by a fallback decoder (arithmetic/12-bit/lossless paths).
    #[error("fallback decoder: {0}")]
    Fallback(String),
}

pub type Result<T> = std::result::Result<T, JpegError>;
