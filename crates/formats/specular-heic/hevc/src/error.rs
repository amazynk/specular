//! The decoder's own notion of failure.
//!
//! It is deliberately narrower than [`crate::Error`] — the codec knows nothing
//! about boxes, items or pixel ceilings — and it never escapes this module
//! tree: the `From` impl at the bottom is the seam, and it hands every message
//! through unchanged so that a caller still reads what the decoder refused.

/// Every failure mode the decoder can report.
///
/// The decoder never panics and never performs I/O; all failures surface here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The bitstream ended in the middle of a syntax element.
    Truncated,
    /// A syntax element had a value the specification forbids.
    InvalidData(&'static str),
    /// Syntactically valid, but uses a tool this decoder does not implement.
    Unsupported(&'static str),
    /// A required parameter set was not supplied or referred to an unknown id.
    MissingParameterSet(&'static str),
    /// The picture dimensions would not fit in memory addressable by `usize`.
    TooLarge,
    /// No coded slice NAL units were supplied.
    NoSlices,
    /// The caller's cancellation flag was raised.
    Cancelled,
    /// The memory budget cannot hold another picture.
    NeedsMemory(specular_core::NeedsMemory),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Truncated => f.write_str("bitstream truncated"),
            Error::InvalidData(m) => {
                f.write_str("invalid bitstream: ")?;
                f.write_str(m)
            }
            Error::Unsupported(m) => {
                f.write_str("unsupported feature: ")?;
                f.write_str(m)
            }
            Error::MissingParameterSet(m) => {
                f.write_str("missing parameter set: ")?;
                f.write_str(m)
            }
            Error::TooLarge => f.write_str("picture too large"),
            Error::NoSlices => f.write_str("no coded slice NAL units supplied"),
            Error::Cancelled => f.write_str("cancelled"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

/// Shorthand used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;

impl core::error::Error for Error {}
