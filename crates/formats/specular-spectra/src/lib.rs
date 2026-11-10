//! Lab spectra: the model every reader produces ([`model`]) and its plots as chart sheets of
//! a vector scene ([`chart`]). The readers are their own crates: `specular-jcamp` (JCAMP-DX
//! and SPC), `specular-mzml` (mzML and mzXML) and `specular-nmr` (Bruker NMR).

pub mod chart;
pub mod model;

pub use model::*;

/// Why a file could not be read.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum Error {
    /// Not this format at all (the next decoder may try).
    #[error("not this format")]
    NotThisFormat,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("{0}")]
    Malformed(String),
    /// A variant we refuse, said in plain English ("SPC files compressed with ... are not
    /// supported").
    #[error("{0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
}
