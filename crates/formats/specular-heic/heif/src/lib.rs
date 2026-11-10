//! HEIF / HEIC container parsing (ISO/IEC 23008-12, ISO/IEC 23000-22).
//!
//! In-tree fork of `oxideav-heif` 0.0.10 (git 3f9908c2f7f8225265eb710ad8b2e1a88cfffe54,
//! MIT, Copyright (c) 2026 Karpelès Lab Inc.; see `LICENSE`). Reduced to the
//! parse side for Specular: the box tree, the `meta` item model, typed
//! properties, derived images (`grid` / `iovl` / `iden` / `tili`), gain-map
//! metadata, pixel composition and the MIAF checks. The encoder, writer,
//! muxer, demuxer, image-sequence tracks, the oxideav framework registry and
//! the AV1 / AVC / layered-HEVC configuration records were removed. Coded
//! HEVC items are decoded by the `specular-heic` driver.
//!
//! # Layers
//!
//! * [`boxes`] — bounds-checked ISOBMFF box reader helpers.
//! * [`ftyp`] — brands and the container probe.
//! * [`meta`] — the `meta` box tree model.
//! * [`file`](mod@file) — [`HeifFile`]: whole-file parse + item payload
//!   resolution across all three `iloc` construction methods.
//! * [`props`] — the typed item-property surface.
//! * [`hvcc`] — the HEVC decoder configuration record.
//! * [`derived`] — `grid` / `iovl` / `iden` descriptors and the bounded
//!   derivation graph ([`derived::build_graph`]).
//! * [`tiled`] — `tili` tiled images.
//! * [`miaf`] — MIAF constraints as typed checks ([`miaf::check`]).
//! * [`image`] — the crate-local planar frame ([`HeifFrame`]).
//! * [`gainmap`] — ISO 21496-1 gain-map metadata.
//! * [`compose`] — pixel composition: grid / overlay / identity
//!   derivations, `clap` / `irot` / `imir`, alpha attachment.
//! * [`rgb`] — reference YCbCr → RGB conversion.
#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod boxes;
pub mod compose;
pub mod derived;
pub mod error;
pub mod file;
pub mod ftyp;
pub mod gainmap;
pub mod hvcc;
pub mod image;
pub mod meta;
pub mod miaf;
pub mod mini;
pub mod props;
pub mod rgb;
pub mod tiled;

pub use derived::{
    build_graph, build_primary_graph, GridDescriptor, ImageKind, ImageNode, OverlayDescriptor,
};
pub use error::{Error, HeifError, Result};
pub use file::{HeifFile, HeifFileRef};
pub use ftyp::{BrandClass, FileType};
pub use gainmap::{apply_gain_map, GainMapMetadata, LinearRgbImage};
pub use hvcc::HevcConfig;
pub use image::{Chroma, ChromaSiting, HeifFrame, HeifPixelFormat, Plane, Site};
pub use meta::{
    EntityGroup, Extent, ItemInfo, ItemLocation, ItemReference, Meta, PropertyAssociation,
    RawProperty,
};
pub use miaf::{MiafProfile, MiafReport, MiafViolation};
pub use props::{
    AuxC, AuxKind, Clap, Colr, CropRect, Imir, Irot, Ispe, ItemProperties, Pasp, Pixi, Property,
    PropertyEntry,
};
pub use rgb::{to_rgb, RgbImage16};

/// Parse a HEIF file held in memory. See [`HeifFile`] for what it exposes.
pub fn parse(bytes: &[u8]) -> Result<HeifFile> {
    HeifFile::parse(bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_input_is_rejected() {
        assert!(super::parse(b"").is_err());
    }
}
