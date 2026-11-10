//! Digital pathology tools, independent of the UI.
//!
//! - [`stain`]: colour deconvolution of brightfield stains (Ruifrok and Johnston, 2001) by
//!   optical-density unmixing, stain vectors estimated from a region (Macenko et al., 2009)
//!   and Macenko normalisation to a reference, as 3 × 3 matrices the GPU applies per pixel;
//!   a CPU reference of the same arithmetic for tests and oracles.
//! - [`annot`]: slide annotations (polygons, rectangles, points, lines) with class labels,
//!   in level-0 pixel coordinates, and their exchange: [`coco`] (instances with polygon
//!   segmentations and boxes), [`qupath`] (QuPath-compatible GeoJSON) and [`mask`] (label
//!   images of a region at a level as PNG or TIFF tiles, or an OME-TIFF label pyramid).
//!
//! # Coordinates
//!
//! Annotations use level-0 (full-resolution) pixel coordinates with the origin at the
//! top-left corner of the top-left pixel and y down: pixel `(i, j)` covers
//! `i..i + 1 × j..j + 1`. This is QuPath's and COCO's convention, so shapes round-trip
//! without a half-pixel shift.

pub mod annot;
pub mod coco;
pub mod json;
pub mod mask;
pub mod qupath;
pub mod stain;
mod tiff;

pub use annot::{Annotation, Class, Geometry};
pub use stain::{Estimate, StainSet, Stains};
