//! Cesium 3D Tiles 1.0 and 1.1: own reader for tilesets and their contents.
//!
//! A [`Tileset`] is read from its `tileset.json`; external tilesets and implicit trees
//! (quadtrees and octrees with subtree files, binary or JSON) are read only when a traversal
//! reaches them. [`Tileset::select`] picks the tiles a view draws by screen-space error
//! (refinement `ADD` and `REPLACE`, box, region and sphere bounding volumes, viewer request
//! volumes, frustum culling). [`load`] reads a tile's contents: `b3dm` (with the legacy
//! headers), `i3dm` (embedded or referenced glTF; positions plain or quantized, orientations
//! plain, oct-encoded or east-north-up, uniform and non-uniform scale), `pnts` (plain or
//! quantized positions; RGBA, RGB, RGB565 or constant colour), `cmpt` (nested) and glTF
//! (`glb`, `gltf`, triangles and points), placed in a drawing frame near the tileset (turned
//! east-north-up for tilesets on the Earth) in double precision. [`merge`] puts many contents
//! into one mesh or point set.
//!
//! **Refused, by name:** Draco-compressed points or meshes, glTF 1.0 content, vector tile
//! content (`geom`, `vctr`), S2 bounding volumes, implicit trees bounded by spheres.
//!
//! **Safety of input.** Files come through a [`Fetch`]: [`DirFetch`] reads only below the
//! tileset's folder; a remote fetcher only from the tileset's own server ([`fetch::resolve`]
//! refuses other addresses). JSON files, tiles and contents are capped in size and number.

pub mod content;
pub mod fetch;
pub mod implicit;
pub mod math;
pub mod merge;
pub mod select;
pub mod tileset;
pub mod volume;

pub use content::{Content, Place, Points};
pub use fetch::{DirFetch, Fetch};
pub use select::{MAX_SSE, Selection, View};
pub use tileset::{Refine, Tile, Tileset};

/// Bytes of one JSON or subtree file, at most.
pub const MAX_JSON: usize = 64 << 20;
/// Tiles held, at most.
pub const MAX_TILES: usize = 500_000;
/// Points or instances in one content, at most.
pub const MAX_CONTENT: usize = 50_000_000;
/// Bytes of one content file, at most.
pub const MAX_CONTENT_BYTES: usize = 1 << 30;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("not found: {0}")]
    Missing(String),
    #[error("nothing to draw")]
    Empty,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `head` starts a 3D Tiles tileset: a JSON object with `asset` and `root` (and a
/// geometric error) near the start.
pub fn sniff(head: &[u8]) -> bool {
    let h = &head[..head.len().min(64 << 10)];
    let first = h.iter().position(|b| !b.is_ascii_whitespace());
    let has = |n: &[u8]| h.windows(n.len()).any(|w| w == n);
    first.is_some_and(|i| h[i] == b'{')
        && has(b"\"asset\"")
        && has(b"\"geometricError\"")
        && (has(b"\"root\"") || has(b"\"boundingVolume\""))
        && !has(b"\"stac_version\"")
}

/// The contents of tile `i`, each read and placed in the drawing frame; one that fails is
/// left out with its reason.
pub fn load(
    ts: &Tileset,
    i: u32,
    fetch: &dyn Fetch,
    limits: &specular_mesh::Limits,
) -> (Vec<Content>, Vec<String>) {
    load_tile(&ts.tiles[i as usize], &ts.frame_inv, fetch, limits)
}

/// [`load`] of a tile taken out of its tileset (`frame_inv`: [`Tileset::frame_inv`]), so
/// the tileset need not stay locked while its contents are read.
pub fn load_tile(
    t: &Tile,
    frame_inv: &math::M4,
    fetch: &dyn Fetch,
    limits: &specular_mesh::Limits,
) -> (Vec<Content>, Vec<String>) {
    let place = Place {
        to_local: math::mul(frame_inv, &t.world),
        up: t.up,
    };
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for key in &t.contents {
        let r = fetch::load(fetch, key, MAX_CONTENT_BYTES)
            .map_err(|e| Error::Missing(format!("{}: {e}", short(key))))
            .and_then(|b| content::read(&b, key, &place, fetch, limits));
        match r {
            Ok(c) => out.push(c),
            Err(e) => errors.push(format!("{}: {e}", short(key))),
        }
    }
    (out, errors)
}

/// A key shortened for messages (no `data:` payloads).
fn short(key: &str) -> &str {
    if key.starts_with("data:") {
        "embedded content"
    } else {
        key
    }
}

#[cfg(test)]
mod tests;
