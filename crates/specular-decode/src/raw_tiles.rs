//! Developed RAW tiles: the 8-bit sRGB output of a camera RAW develop, cut into a fixed grid of
//! [`DEV_TILE`]-pixel squares per develop size (full and half) and kept, least recently used
//! dropped first, so regions over the same pixels (panning, revisits, overlapping requests)
//! reuse them and only tiles not developed yet are developed.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

/// Edge of a developed tile in output pixels of its develop (a divisor of the viewer's 512-px
/// tiles, so their requests cover whole developed tiles).
pub(crate) const DEV_TILE: u32 = 256;

/// One developed tile: planar 8-bit R, G, B (`w`×`h`, smaller at the level's right and bottom
/// edges).
pub(crate) struct DevTile {
    pub w: u32,
    pub h: u32,
    pub planes: [Vec<u8>; 3],
    /// Developed from truncated sensor data.
    pub truncated: bool,
}

impl DevTile {
    pub fn bytes(&self) -> u64 {
        self.planes.iter().map(|p| p.len() as u64).sum()
    }
}

/// Tile `(tx, ty)` of the half-size (`true`) or full-size develop.
pub(crate) type TileKey = (bool, u32, u32);

#[derive(Default)]
struct Inner {
    /// Each tile with its last use.
    map: HashMap<TileKey, (Arc<DevTile>, u64)>,
    bytes: u64,
    clock: u64,
}

/// A session's developed tiles, at most `max` bytes.
pub(crate) struct DevTiles {
    max: u64,
    inner: Mutex<Inner>,
}

impl DevTiles {
    pub fn new(max: u64) -> DevTiles {
        DevTiles {
            max,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Most bytes kept.
    pub fn max(&self) -> u64 {
        self.max
    }

    /// The tile at `key`, if kept (then the most recently used).
    pub fn get(&self, key: TileKey) -> Option<Arc<DevTile>> {
        let mut g = self.inner.lock();
        g.clock += 1;
        let now = g.clock;
        g.map.get_mut(&key).map(|(t, used)| {
            *used = now;
            t.clone()
        })
    }

    /// Whether all of `keys` are kept.
    pub fn has_all(&self, keys: impl IntoIterator<Item = TileKey>) -> bool {
        let g = self.inner.lock();
        keys.into_iter().all(|k| g.map.contains_key(&k))
    }

    /// Keeps `tile` at `key` as the most recently used, dropping the least recently used
    /// tiles while more than `max` bytes are kept. A tile larger than `max` is not kept.
    pub fn insert(&self, key: TileKey, tile: Arc<DevTile>) {
        let n = tile.bytes();
        if n > self.max {
            return;
        }
        let mut g = self.inner.lock();
        g.clock += 1;
        let now = g.clock;
        if let Some((old, _)) = g.map.insert(key, (tile, now)) {
            g.bytes -= old.bytes();
        }
        g.bytes += n;
        while g.bytes > self.max {
            let Some(oldest) = g.map.iter().min_by_key(|(_, (_, u))| *u).map(|(k, _)| *k) else {
                break;
            };
            if let Some((t, _)) = g.map.remove(&oldest) {
                g.bytes -= t.bytes();
            }
        }
    }

    pub fn memory_bytes(&self) -> u64 {
        self.inner.lock().bytes
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map.len()
    }

    /// Drops every tile; the bytes released.
    pub fn clear(&self) -> u64 {
        let mut g = self.inner.lock();
        g.map.clear();
        std::mem::take(&mut g.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(v: u8) -> Arc<DevTile> {
        Arc::new(DevTile {
            w: 10,
            h: 10,
            planes: std::array::from_fn(|_| vec![v; 100]),
            truncated: false,
        })
    }

    /// Past the bound the least recently used tiles go first; a get counts as a use.
    #[test]
    fn evicts_least_recently_used() {
        let c = DevTiles::new(900);
        for i in 0..3 {
            c.insert((false, i, 0), tile(i as u8));
        }
        assert_eq!((c.len(), c.memory_bytes()), (3, 900));
        assert!(c.get((false, 0, 0)).is_some());
        c.insert((true, 0, 0), tile(9));
        assert_eq!((c.len(), c.memory_bytes()), (3, 900));
        assert!(
            c.get((false, 1, 0)).is_none(),
            "least recently used dropped"
        );
        assert!(c.has_all([(false, 0, 0), (false, 2, 0), (true, 0, 0)]));
        // Replacing a tile keeps the byte count exact.
        c.insert((true, 0, 0), tile(7));
        assert_eq!(c.memory_bytes(), 900);
        assert_eq!(c.get((true, 0, 0)).unwrap().planes[0][0], 7);
        assert_eq!(c.clear(), 900);
        assert_eq!((c.len(), c.memory_bytes()), (0, 0));
        // Larger than the bound: not kept.
        let small = DevTiles::new(100);
        small.insert((false, 0, 0), tile(1));
        assert_eq!((small.len(), small.memory_bytes()), (0, 0));
    }
}
