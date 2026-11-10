//! Core types shared by every Specular crate.
//!
//! * [`Source`]: the bytes of an image file, memory-mapped when possible.
//! * [`Raster`]: decoded planar pixel data (one plane per component, possibly subsampled).
//! * [`JobPool`]: a small prioritised thread pool with per-job cancellation.
//! * [`isolate`]: panic boundaries for jobs, and the local crash report of the UI thread.
//! * [`HardwareProfile`]: what the machine offers (cores, RAM), used for budgets.
//! * [`budget`]: the memory budget decoders reserve large allocations from.
//! * [`work`]: the registry of background jobs, for the "Background work" panel.
//! * [`net`]: the registry of downloads, for the "Downloads" indicator and panel.
//! * [`links`]: the web addresses the app opens in the browser (the Discord invite).

#![feature(portable_simd)]

pub mod budget;
pub mod fill;
pub mod hdr;
mod hw;
pub mod isolate;
mod jobs;
pub mod keys;
pub mod links;
pub mod local_copy;
pub mod nd;
pub mod net;
pub mod paths;
pub mod placement;
mod qos;
mod raster;
mod source;
pub mod work;
pub mod yuv;

pub use budget::{BudgetHandle, NeedsMemory, Reservation, current_budget};
pub use hdr::{GainMap, GainMapParams, HdrEncoding, Primaries, Transfer};
pub use hw::{HardwareProfile, available_memory};
pub use jobs::{CancelToken, JobHandle, JobPool, Priority};
pub use raster::{Orientation, PixelLayout, Raster, RasterPlane};
pub use source::{
    GERBER_EXTENSIONS, MAX_NESTING, NestingGuard, Source, is_aig_folder, is_bruker_folder,
    is_casa_folder, is_casa_head, is_filegdb_folder, is_gerber_folder, is_image_folder,
    is_odb_folder, is_relight_folder, is_relight_head, is_sentinel2_folder, is_tiles3d_folder,
    is_zarr_folder, open_regular,
};
pub use yuv::{YuvEncoding, YuvMatrix};

use std::time::Instant;

/// Record the process start time. Call first thing in `main`; later calls are ignored.
/// (The clock lives in `specular-log`, which stamps log lines with it.)
pub fn mark_process_start() -> Instant {
    specular_log::mark_process_start()
}

/// The instant recorded by [`mark_process_start`] (or the first call to this function).
pub fn process_start() -> Instant {
    mark_process_start()
}

/// Milliseconds since [`process_start`].
pub fn ms_since_start() -> f64 {
    process_start().elapsed().as_secs_f64() * 1000.0
}
