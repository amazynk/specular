//! Image rendering for Specular.
//!
//! - [`view`]: the view model (log zoom anchored at the cursor, springs, momentum,
//!   rubber-band, fit/fill/100 %, rectangle zoom, EXIF orientation in the transform).
//! - [`tiles`]: geometry and LRU of the 512 px detail-tile pyramid.
//! - [`icc`]: embedded ICC profiles → source transforms (curves + matrix or 3D LUT) via
//!   specular-color.
//! - [`renderer`]: GPU upload and conversion (YCbCr planes → linear light, centred chroma),
//!   overview with gamma-correct Lanczos mips, and the image shader (Lanczos when settled,
//!   bilinear while moving, nearest-neighbour + pixel grid at high zoom, checkerboard), and
//!   the inspection passes (clipping / focus-peaking overlay, histogram, pixel probe).

pub mod cpu_colour;
pub mod icc;
pub mod picture;
pub mod renderer;
pub mod tiles;
pub mod vector;
pub mod view;

pub use picture::PictureAdjust;
pub use renderer::compare::{
    Comparer, ComparerBuild, DiffParams, METRIC_SIZE, MetricImage, MetricPair, MetricResult,
    MetricsReadback, Place,
};
pub use renderer::inspect::{
    HistogramRequest, Inspector, InspectorBuild, OverlayOptions, Readback,
};
pub use renderer::medical::{
    ASPECT_MAP, ChannelLook, FLAT_GREY, Hillshade, Isotherm, MAX_CHANNELS, Relief, SLOPE_MAP,
    SLOPE_MAX, ScienceLook, ValueMask, VoiShape, Window,
};
pub use renderer::{
    COLOUR_STEPS, GpuImage, GpuImageCache, Gsdf, HiLo, ImageRenderer, OcioLut, Overview,
    RenderOptions, StainView, TILE_BYTES, ycbcr_variant,
};
pub use view::{Affine, PxRect, View, ZoomMode};
