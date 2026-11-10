//! Optical and NMR spectra files read with our own parsers into the `specular-spectra`
//! model:
//!
//! * [`jcamp`]: JCAMP-DX 4.24–6.0 (IR, Raman, UV-Vis, NMR, MS): `##XYDATA=` in AFFN and
//!   ASDF (SQZ, DIF, DUP), `##XYPOINTS=`, `##PEAK TABLE=`, NTUPLES pages, LINK blocks.
//! * [`spc`]: Galactic/Thermo SPC, old and new formats, single and multi-file, X arrays.

pub mod jcamp;
pub mod spc;

pub use specular_spectra::Error;
