use thiserror::Error;

/// Errors returned by the decoder. Never panics on bad input; corrupt or truncated packet data
/// normally yields a (partial) image instead of an error, see [`crate::Image::truncated`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Jp2Error {
    /// Neither a JP2 file nor a raw JPEG 2000 codestream.
    #[error("not a JPEG 2000 file")]
    NotJp2,
    /// The data ends inside the headers.
    #[error("truncated JPEG 2000 data: {0}")]
    Truncated(&'static str),
    /// Structurally invalid headers.
    #[error("malformed JPEG 2000 data: {0}")]
    Malformed(&'static str),
    /// Valid but not supported (JPX/JPM-only files, Part 2 extensions, ...).
    #[error("unsupported JPEG 2000 feature: {0}")]
    Unsupported(&'static str),
    /// The decode would exceed an allocation cap (or the allocator refused).
    #[error("image too large to decode ({0} bytes)")]
    TooLarge(u64),
    /// Under the caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
    /// The cancel flag was raised.
    #[error("decode cancelled")]
    Cancelled,
    /// Bad [`crate::DecodeOptions`] (region outside the image, too many discarded levels, ...).
    #[error("invalid decode request: {0}")]
    InvalidRequest(&'static str),
}

pub type Result<T> = std::result::Result<T, Jp2Error>;
