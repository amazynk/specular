//! Colour management for Specular.
//!
//! * [`icc`]: ICC v2/v4 parsing — matrix/TRC, gray TRC and LUT-based `A2B`/`B2A` tags
//!   (`lut8`, `lut16`, `lutAtoB`, `lutBtoA`), RGB/gray/CMYK/L*a*b* device spaces, XYZ and
//!   L*a*b* PCS.
//! * [`transform`]: profile-to-profile transforms with all four rendering intents and
//!   black point compensation.
//! * [`gpu`]: per-image source transforms (curves + matrix, or a 3D LUT) and the display
//!   transform, ready for upload.
//! * [`cmyk`]: fast CMYK / L*a*b* → 8-bit sRGB for decoders.
//! * [`policy`]: the process-wide settings and transform caches.
//!
//! Everything is our own code; moxcms is used only as a test oracle.

#![feature(portable_simd)]

pub mod cmyk;
pub mod curve;
pub mod gpu;
pub mod icc;
pub mod lut;
pub mod math;
pub mod policy;
pub mod profiles;
pub mod transform;

pub use cmyk::{CmykConverter, LabCoding, LabConverter};
pub use gpu::{Display, DisplayKind, SourceLut};
pub use icc::{ColorSpace, Error, Profile};
pub use policy::{Policy, policy, set_policy};
pub use transform::{Intent, Transform};
