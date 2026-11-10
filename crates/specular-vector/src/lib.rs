//! Vector scenes for 2D drawings.
//!
//! [`scene`] is the model a drawing format parses into ([`VectorScene`]); [`prep`] turns
//! it into a display list (dashes applied, text laid out with [`fonts`]); [`draw`] walks
//! that list for a view into a backend: [`raster`] on the CPU, vello on the GPU (in
//! `specular-render`).

pub mod draw;
pub mod fonts;
pub mod prep;
pub mod raster;
pub mod scene;
pub mod shx;

pub use kurbo;
pub use scene::*;
