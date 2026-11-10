//! What can go wrong while decoding.

/// Every failure the decoder reports. It never panics on bad input and does no I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The bitstream ended in the middle of a syntax element.
    Truncated,
    /// A syntax element had a value the Recommendation forbids.
    InvalidData(&'static str),
    /// Valid, but uses something this decoder does not implement; the text says what, in
    /// plain words.
    Unsupported(&'static str),
    /// A parameter set the picture needs was not supplied.
    MissingParameterSet(&'static str),
    /// The picture is too large to address.
    TooLarge,
    /// No coded slice NAL units were supplied.
    NoSlices,
    /// The caller's cancellation flag was raised.
    Cancelled,
    /// The memory budget cannot hold the picture.
    NeedsMemory(specular_core::NeedsMemory),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Truncated => f.write_str("VVC bitstream truncated"),
            Error::InvalidData(m) => write!(f, "invalid VVC bitstream: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported VVC feature: {m}"),
            Error::MissingParameterSet(m) => write!(f, "missing VVC parameter set: {m}"),
            Error::TooLarge => f.write_str("VVC picture too large"),
            Error::NoSlices => f.write_str("no coded VVC slices"),
            Error::Cancelled => f.write_str("cancelled"),
            Error::NeedsMemory(n) => write!(f, "{n}"),
        }
    }
}

impl core::error::Error for Error {}

/// Shorthand used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;
