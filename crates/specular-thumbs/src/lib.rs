//! Thumbnail engine (DESIGN.md §10): everything the gallery, the Library and the OS
//! thumbnailer share, and nothing that needs a window or a GPU.
//!
//! * [`list`]: streamed folder listing (first chunk as soon as it is read).
//! * [`key`] / [`cache`]: a content-keyed disk cache in the platform cache directory, keyed by
//!   size + modification time + a partial content hash, so renamed or copied files hit.
//! * [`generate`]: embedded preview first, then a DCT-scaled / reduced decode through the
//!   [`specular_decode::Registry`] (sandboxed formats go through the decode host there).
//! * [`engine`]: a late-binding priority queue (visible first, then the scroll direction)
//!   with cancellation, a bounded memory LRU, and no work on the caller's thread.
//! * [`layout`]: the grid maths (square, justified and uncropped layouts, details rows,
//!   group headers, hit testing, keyboard navigation, zoom anchoring).
//! * [`sort`], [`watch`]: ordering and live folder updates.

pub mod cache;
pub mod codec;
pub mod engine;
pub mod exif_meta;
pub mod generate;
pub mod key;
pub mod layout;
pub mod list;
pub mod pixels;
pub mod slowio;
pub mod sort;
pub mod watch;

pub use cache::{ASTRO_BLOB, AstroMeta, CacheEntry, Digest, DiskCache, Meta};
pub use engine::{
    Digester, Engine, EngineConfig, KeyMemo, Opened, Opener, Ready, Request, Stats, StreamPos,
    Timing,
};
pub use exif_meta::{ExifMeta, exif_meta};
pub use key::{Stamp, ThumbKey};
pub use list::{Entry, EntryKind};

/// Thumbnail size classes: the long side of a cached thumbnail. The grid picks the smallest
/// class at least as large as the tile in device pixels.
pub const CLASSES: [u32; 3] = [128, 256, 512];

/// The smallest class that covers `px` device pixels (the largest class when none does).
pub fn class_for(px: f32) -> u32 {
    CLASSES
        .iter()
        .copied()
        .find(|&c| c as f32 >= px * 0.9)
        .unwrap_or(CLASSES[CLASSES.len() - 1])
}

#[cfg(test)]
mod tests {
    #[test]
    fn classes() {
        assert_eq!(super::class_for(40.0), 128);
        assert_eq!(super::class_for(130.0), 128);
        assert_eq!(super::class_for(200.0), 256);
        assert_eq!(super::class_for(900.0), 512);
    }
}
