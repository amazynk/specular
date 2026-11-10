//! The seam for raster base-map tiles.
//!
//! The map view draws the offline vector base map ([`crate::World`]) and nothing else. A
//! [`TileSource`] could add raster tiles over it (z/x/y in the usual web map scheme), but
//! there is no implementation and no default provider: whether Specular may fetch map tiles
//! at all, and from whom, is an open question.
//!
//! TODO(DESIGN §17 Q8): decide on online map tiles. If they come, the HTTP goes through the
//! future `crates/specular-remote` client (one place for proxies, consent and offline mode),
//! never from here; this module stays network-free.

use std::sync::Arc;

/// A tile as the renderer would take it.
#[derive(Clone, Debug)]
pub enum TileState {
    /// Not available (and not coming): the vector base map shows through.
    Missing,
    /// Asked for; draw the base map for now and ask again next frame.
    Pending,
    /// Straight sRGB RGBA8, `size` × `size` pixels.
    Ready { size: u32, rgba: Arc<[u8]> },
}

/// Where raster tiles would come from. Implementations must not block: `tile` answers from
/// memory and starts any fetch elsewhere.
pub trait TileSource: Send {
    /// Tile `x`, `y` at zoom `z` (0 ≤ x, y < 2^z).
    fn tile(&mut self, z: u8, x: u32, y: u32) -> TileState;
    /// The credit line the map must show while tiles from here are on screen.
    fn attribution(&self) -> &str;
    /// The deepest zoom with tiles.
    fn max_zoom(&self) -> u8;
}

/// The tiles covering a view: (z, x, y) at zoom level `z` for the world rectangle
/// `x0..x1` × `y0..y1` (x may go past 0 and 1 for world copies; returned x are wrapped).
pub fn covering(z: u8, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<(u8, u32, u32)> {
    let n = 1u32 << z.min(30);
    let nf = f64::from(n);
    let tx0 = (x0 * nf).floor() as i64;
    let tx1 = (x1 * nf).ceil() as i64;
    let ty0 = ((y0.clamp(0.0, 1.0)) * nf).floor() as i64;
    let ty1 = ((y1.clamp(0.0, 1.0)) * nf).ceil() as i64;
    let mut out = Vec::new();
    for ty in ty0..ty1.min(i64::from(n)) {
        for tx in tx0..tx1.min(tx0 + i64::from(n)) {
            out.push((z, tx.rem_euclid(i64::from(n)) as u32, ty as u32));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covering_wraps_and_clamps() {
        assert_eq!(covering(0, 0.0, 0.0, 1.0, 1.0), vec![(0, 0, 0)]);
        assert_eq!(covering(1, 0.6, 0.1, 1.2, 0.4), vec![(1, 1, 0), (1, 0, 0)]);
        assert_eq!(covering(1, 0.0, -0.5, 0.4, 0.2).len(), 1);
    }
}
