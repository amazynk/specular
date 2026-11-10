//! Decoded sensor tiles kept between region decodes ([`TileCache`]), so panning at 1:1 decodes
//! each tile of a tiled sensor (lossless ARW, tiled DNG) once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// One decoded sensor tile: its part inside the image, `row` samples per row.
pub(crate) struct Tile {
    pub data: Vec<u16>,
    pub row: usize,
    pub truncated: bool,
}

impl Tile {
    fn bytes(&self) -> u64 {
        self.data.len() as u64 * 2
    }
}

struct Slot {
    cell: Arc<OnceLock<Arc<Tile>>>,
    used: u64,
}

#[derive(Default)]
struct Inner {
    /// Identifies the sensor layout the tiles belong to.
    sig: u64,
    clock: u64,
    bytes: u64,
    slots: HashMap<usize, Slot>,
}

/// Decoded sensor tiles of one file, least recently used first out once they take more than
/// the given number of bytes. Give the same cache (through [`crate::DecodeOptions::tiles`]) to
/// the region decodes of one file; a different sensor layout empties it. Concurrent decodes
/// that need the same tile wait for one decode of it.
pub struct TileCache {
    max_bytes: u64,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for TileCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TileCache")
            .field("max_bytes", &self.max_bytes)
            .field("bytes", &self.memory_bytes())
            .finish()
    }
}

impl TileCache {
    pub fn new(max_bytes: u64) -> TileCache {
        TileCache {
            max_bytes,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Bytes of the decoded tiles held.
    pub fn memory_bytes(&self) -> u64 {
        self.lock().bytes
    }

    /// Number of decoded tiles held.
    pub fn len(&self) -> usize {
        self.lock()
            .slots
            .values()
            .filter(|s| s.cell.get().is_some())
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        let mut g = self.lock();
        g.slots.clear();
        g.bytes = 0;
    }

    /// Use the cache for the sensor layout `sig`, dropping tiles of another.
    pub(crate) fn bind(&self, sig: u64) {
        let mut g = self.lock();
        if g.sig != sig {
            g.slots.clear();
            g.bytes = 0;
            g.sig = sig;
        }
    }

    /// Tile `i`, decoded by `decode` unless held (or being decoded by another thread, which is
    /// then waited for).
    pub(crate) fn get(&self, i: usize, decode: impl FnOnce() -> Tile) -> Arc<Tile> {
        let cell = {
            let mut g = self.lock();
            g.clock += 1;
            let used = g.clock;
            let slot = g.slots.entry(i).or_insert_with(|| Slot {
                cell: Arc::new(OnceLock::new()),
                used,
            });
            slot.used = used;
            slot.cell.clone()
        };
        let mut fresh = false;
        let t = cell
            .get_or_init(|| {
                fresh = true;
                Arc::new(decode())
            })
            .clone();
        if fresh {
            let mut g = self.lock();
            // Only count it if the slot was not dropped meanwhile (clear, another layout).
            if g.slots.get(&i).is_some_and(|s| Arc::ptr_eq(&s.cell, &cell)) {
                g.bytes += t.bytes();
                self.evict(&mut g, i);
            }
        }
        t
    }

    fn evict(&self, g: &mut Inner, keep: usize) {
        while g.bytes > self.max_bytes {
            let victim = g
                .slots
                .iter()
                .filter(|(k, s)| **k != keep && s.cell.get().is_some())
                .min_by_key(|(_, s)| s.used)
                .map(|(k, _)| *k);
            let Some(k) = victim else { break };
            if let Some(s) = g.slots.remove(&k) {
                g.bytes = g
                    .bytes
                    .saturating_sub(s.cell.get().map_or(0, |t| t.bytes()));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    fn tile(n: usize) -> Tile {
        Tile {
            data: vec![7; n],
            row: n,
            truncated: false,
        }
    }

    #[test]
    fn evicts_least_recently_used_within_budget() {
        let c = TileCache::new(3 * 200);
        c.bind(1);
        for i in 0..3 {
            c.get(i, || tile(100));
        }
        assert_eq!((c.len(), c.memory_bytes()), (3, 600));
        // Touch 0, then a fourth tile pushes out 1 (the least recently used).
        let mut decoded = false;
        c.get(0, || {
            decoded = true;
            tile(100)
        });
        assert!(!decoded);
        c.get(3, || tile(100));
        assert_eq!((c.len(), c.memory_bytes()), (3, 600));
        let mut again = false;
        c.get(1, || {
            again = true;
            tile(100)
        });
        assert!(again);
        // Another layout starts empty.
        c.bind(2);
        assert!(c.is_empty());
        assert_eq!(c.memory_bytes(), 0);
    }

    #[test]
    fn one_decode_for_concurrent_requests() {
        let c = TileCache::new(u64::MAX);
        let n = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    c.get(5, || {
                        n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        tile(10)
                    })
                });
            }
        });
        assert_eq!(n.into_inner(), 1);
        assert_eq!(c.memory_bytes(), 20);
    }
}
