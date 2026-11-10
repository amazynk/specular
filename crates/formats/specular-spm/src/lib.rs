//! `specular-spm`: scanning-probe readers for Specular.
//!
//! * **[`gwy`]: Gwyddion `.gwy`.** The `GWYP` object tree (and Gwyddion 1's `GWYO`
//!   container); every `GwyDataField` channel with its physical size, offset, lateral and
//!   value units, title and metadata.
//! * **[`ibw`]: Igor Pro binary waves.** Versions 1, 2, 3 and 5, either byte order; integer,
//!   float and complex (shown as magnitude) waves of up to four dimensions; scaling, units,
//!   extended units, dimension labels and the wave note (`Key: value` lines, as Asylum
//!   Research writes them).
//! * **[`nanoscope`]: Bruker / Veeco Nanoscope** (version 5 and later), **[`sxm`]: Nanonis
//!   `.sxm`**, **[`jpk`]: JPK `.jpk` and `.jpk-qi-image`** and **[`mdt`]: NT-MDT `.mdt`**
//!   scanned frames, read straight into a [`specular_em::dataset::Dataset`] with SI lateral
//!   calibrations and plane units.
//!
//! Each format parses its header first ([`gwy::parse`], [`ibw::parse`]) without converting
//! values; planes are then read one at a time as `f32` in the file's row order. Malformed
//! input never panics.

pub mod gwy;
pub mod ibw;
pub mod jpk;
pub mod mdt;
pub mod nanoscope;
pub mod sxm;
mod units;

/// Errors from the readers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a Gwyddion file")]
    NotGwy,
    #[error("not an Igor binary wave")]
    NotIbw,
    #[error("not a Nanoscope file")]
    NotNanoscope,
    #[error("not a Nanonis SXM file")]
    NotSxm,
    #[error("not a JPK file")]
    NotJpk,
    #[error("not an NT-MDT file")]
    NotMdt,
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
}

pub type Result<T> = std::result::Result<T, Error>;
