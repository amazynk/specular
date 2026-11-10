//! BIM review.
//!
//! - [`bcf`]: BCF 2.1 and 3.0 archives (`.bcfzip`, `.bcf`) read into one model of topics,
//!   comments and viewpoints; [`write`] writes them back as either version, and [`check`]
//!   holds what is written to the buildingSMART schemas' rules (element order, required
//!   parts, GUID, date and colour patterns, field-of-view ranges).
//! - [`views`]: named camera views saved next to the model (`<model>.specular-views.json`).
//! - [`federation`]: several IFC models in one scene, placed by their georeferencing, for
//!   cross-model clash tests and BCF topics that name every model involved.
//! - [`apply`]: a BCF viewpoint's component selection, visibility and colours on a building
//!   model's mesh, element by element (by IFC GUID).
//!
//! The model file is never written: only the sidecar and the archives the user exports.

pub mod apply;
pub mod bcf;
pub mod check;
pub mod clash;
pub mod diff;
pub mod export;
pub mod federation;
pub mod ids;
pub mod idscheck;
pub mod qto;
pub mod views;
pub mod write;
pub mod xsd;

pub use bcf::{
    Archive, BcfCamera, BcfError, CameraKind, ClippingPlane, Coloring, Comment, Component,
    Components, Topic, Version, Viewpoint, VisInfo, read,
};
pub use write::write;
