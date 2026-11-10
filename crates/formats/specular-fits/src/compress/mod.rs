//! Tile-compression codecs of the FITS tiled image convention (FITS 4.0 §10.4; Pence et al.,
//! "Tiled image convention for storing compressed images in FITS binary tables"): `RICE_1`,
//! `HCOMPRESS_1` and `PLIO_1`. Each decodes one tile's bytes as they are stored in the binary
//! table's heap; `GZIP_1` / `GZIP_2` are plain gzip and live with the tile reader.
//!
//! All three are written from the published descriptions (Rice, Yeh & Miller 1993; White 1992;
//! IRAF's PLIO line lists) and checked against astropy's codecs (which wrap cfitsio) on the
//! fixtures in `tests/data/codec_*.tsv` (`gen/codec_fixtures.py`). Corrupt or truncated
//! input returns [`CodecError`] and never panics; nothing is allocated beyond the tile's size.

mod hcompress;
mod plio;
mod rice;

pub use hcompress::{Hdecoded, hcompress};
pub use plio::plio;
pub use rice::rice;

/// A tile that cannot be decoded.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CodecError {
    #[error("corrupt compressed tile: {0}")]
    Corrupt(&'static str),
}

#[cfg(test)]
mod tests;
