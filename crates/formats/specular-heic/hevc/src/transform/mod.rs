//! Inverse transforms (clause 8.6.4).
mod dct;
pub use dct::{inverse_transform, inverse_transform_ext, transform_skip, transform_skip_ext};

mod dequant;
pub use dequant::scale_range;

mod extent;
