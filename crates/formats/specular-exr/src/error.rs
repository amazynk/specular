/// Errors from [`crate::read_info`] and [`crate::decode`]. Damaged pixel data is not an error:
/// it is reported in [`crate::ExrImage::damaged_chunks`].
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ExrError {
    #[error("not an OpenEXR file")]
    NotExr,
    #[error("truncated header")]
    Truncated,
    #[error("invalid header: {0}")]
    InvalidHeader(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    #[error("unsupported compression {0}")]
    UnsupportedCompression(&'static str),
    #[error("no such part, level or channel: {0}")]
    NotFound(&'static str),
    #[error("image of {width}x{height} exceeds the configured limits")]
    TooLarge { width: u64, height: u64 },
    #[error("out of memory allocating {0} bytes")]
    OutOfMemory(u64),
    /// Under the limits, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, ExrError>;
