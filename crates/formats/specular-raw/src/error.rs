use thiserror::Error;

/// Errors returned by the RAW reader. Corrupt or truncated *pixel data* normally yields a
/// (partial) image flagged as truncated rather than an error; errors are reserved for files whose
/// structure cannot be understood at all.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RawError {
    /// Not a camera RAW file this crate recognises.
    #[error("not a camera RAW file")]
    NotRaw,
    /// The container structure is broken (bad offsets, missing mandatory tags, ...).
    #[error("malformed RAW file: {0}")]
    Malformed(&'static str),
    /// Valid, but the sensor data layout / compression is not supported (yet).
    #[error("unsupported RAW: {0}")]
    Unsupported(&'static str),
    /// The file has no embedded preview of the requested kind.
    #[error("no embedded preview")]
    NoPreview,
    /// Output would exceed the configured pixel/byte limit or is implausible for the file size.
    #[error("image too large to decode ({0} samples)")]
    TooLarge(u64),
    /// The cancel flag was raised.
    #[error("decode cancelled")]
    Cancelled,
    /// Embedded JPEG preview could not be decoded.
    #[error("preview JPEG: {0}")]
    Jpeg(#[from] specular_jpeg::JpegError),
}

pub type Result<T> = std::result::Result<T, RawError>;
