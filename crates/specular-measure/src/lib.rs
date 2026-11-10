//! Measurements on rasters, independent of the UI (DESIGN.md §6.8).
//!
//! # Coordinates
//!
//! Everything is in *stored pixel coordinates* with the origin at the top-left corner of the
//! top-left pixel and y down: pixel `(i, j)` covers `i..i + 1 × j..j + 1` and its centre is
//! `(i + 0.5, j + 0.5)`. This is the convention of QuPath's GeoJSON and of ImageJ's sub-pixel
//! ROIs, so shapes round-trip through both without shifting.
//!
//! # Pixel coverage (which pixels a region's statistics include)
//!
//! A pixel belongs to a region when its **centre** is inside the region, with these tie rules
//! for centres exactly on the outline ([`cover`]):
//!
//! * rectangle `x0..x1 × y0..y1`: half-open, `x0 <= cx < x1` and `y0 <= cy < y1`, so a
//!   rectangle on integer bounds covers exactly `(x1 - x0) × (y1 - y0)` pixels and two
//!   rectangles sharing an edge never share a pixel;
//! * ellipse: closed, `((cx - x)/rx)² + ((cy - y)/ry)² <= 1` (the DICOM viewer's rule);
//! * polygon and freehand: the even-odd rule with half-open crossings — an edge crosses the
//!   scanline through the centre when exactly one of its ends has `y <= cy`, and a centre on a
//!   left edge is in, on a right edge out. An axis-aligned polygon therefore covers the same
//!   pixels as the equivalent rectangle, and self-intersecting freehand loops alternate;
//! * point: the one pixel containing it.
//!
//! The *geometric* area (the shape's own area, exact for rectangles, ellipses and simple
//! polygons) and the *pixel count* are both reported; they differ along the outline.
//!
//! # Calibration
//!
//! Every length, area and angle is a [`Quantity`] that carries the unit and the
//! [`Source`] of its [`Calibration`], down to "uncalibrated (pixels)".

pub mod calib;
pub mod clinical;
pub mod cover;
pub mod csv;
pub mod geojson;
pub mod geom;
pub mod imagej;
pub mod inspect;
pub mod list;
pub mod profile;
pub mod provenance;
pub mod raster;
pub mod scale;
pub mod stats;

pub use calib::{CalibError, Calibration, Kind, Quantity, Source, Unit};
pub use geom::{P, Shape};
pub use list::{List, Measurement};
pub use profile::Profile;
pub use provenance::Provenance;
pub use raster::{Samples, View};
pub use stats::{ChannelStats, Histogram};
