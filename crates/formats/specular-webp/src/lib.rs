//! WebP decoding for Specular.
//!
//! A fork of `image-webp` 0.2.4 (see NOTICE). The upstream decoder is kept close to its
//! original form (`decoder`, `vp8`, `lossless`, ...) so upstream fixes can be merged; Specular's
//! additions live in [`image`]: a whole-file parser over a byte slice, planar YCbCr output for
//! lossy images (no RGB conversion on the CPU), decoding at 1/2/1/4/1/8 scale by box-filtering
//! the planes, parallel conversion, and an animation frame API.

#![forbid(unsafe_code)]
#![feature(portable_simd)]
#![recursion_limit = "256"]

pub use self::decoder::{
    DecodingError, LoopCount, UpsamplingMethod, WebPDecodeOptions, WebPDecoder,
};
pub use self::image::{AnimFrame, Animation, Coding, Decoded, Info, Pixels, Plane, WebpImage};

mod alpha_blending;
mod decoder;
mod extended;
mod huffman;
pub mod image;
mod loop_filter;
mod loop_filter_simd;
mod lossless;
mod lossless_transform;
mod transform;
mod vp8_arithmetic_decoder;
mod yuv;

pub mod vp8;
