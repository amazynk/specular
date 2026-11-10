//! Duplicates and similar shots (DESIGN.md §10): finding the same image in another format or
//! size, and the near-same shots of a burst or a re-take, then suggesting which to keep.
//!
//! Everything but [`scan`] is pure logic over small per-image records, so it can be tested
//! and benchmarked without a GPU or a file system, and later run over the catalogue:
//!
//! - [`sig`]: tier 1 (canonical pixel hash) and tier 2 (pHash, dHash, colour moments)
//!   signatures and the quality measures, from the thumbnail the gallery already decodes.
//! - [`ssim`]: the verify step on 256 px thumbnails, for candidate pairs only.
//! - [`index`]: multi-index hashing over the 64-bit hashes for candidates.
//! - [`hints`]: capture time with sub-seconds, serial, burst/bracket ids, XMP document ids,
//!   GPS and focal length from the file head; user data from XMP sidecars.
//! - [`group`]: duplicate pairs and similar-shot clusters with a live sensitivity.
//! - [`pick`]: best-pick suggestions, each with a reason.
//! - [`store`]: signatures and decisions on disk, shaped like the catalogue's
//!   `image_hashes` and `match_groups` tables.
//! - [`analyse`]: from items to the groups the UI shows.
//! - [`live`]: the same groups kept current over a changing set (the Library's catalogue).
//! - [`scan`]: the background scan over the shared thumbnail engine.
//! - [`embed`]: the hook for tier 3 (AI pack image embeddings).
//!
//! Tier 0 (size + BLAKE3 of the file) is [`file_hash`]: only files whose size collides with
//! another are hashed.

pub mod analyse;
pub mod embed;
pub mod focus;
pub mod group;
pub mod hints;
pub mod index;
pub mod live;
pub mod pick;
pub mod scan;
pub mod sig;
pub mod ssim;
pub mod store;

use std::io::Read;
use std::path::Path;

pub use analyse::Group;
pub use group::{Category, Item, Kind, Tier};
pub use hints::Hints;
pub use pick::{Pick, Protect, Reason};
pub use sig::{Luma, Quality, SIG_CLASS, Signature};
pub use store::{Decision, DiscardAction, FileId, Record, Store};

/// Tier 0: BLAKE3 of the whole file, streamed.
pub fn file_hash(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = std::fs::File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 256 << 10];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(*h.finalize().as_bytes())
}

/// The first `n` bytes of a file (for [`hints::read_head`]).
pub fn file_head(path: &Path, n: usize) -> std::io::Result<Vec<u8>> {
    let f = std::fs::File::open(path)?;
    let mut v = Vec::with_capacity(n.min(1 << 20));
    f.take(n as u64).read_to_end(&mut v)?;
    Ok(v)
}
