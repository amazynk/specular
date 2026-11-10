//! Tile bookkeeping for the detail pyramid: geometry, which tiles a view needs, and an LRU
//! of GPU array layers.
//!
//! Level `L` is the image downscaled by `2^L` (the JPEG decoder's native scales, so a level
//! is exactly what a scaled decode produces). Tiles are 512² of level pixels plus an 8-pixel
//! border of real neighbouring pixels (edge-replicated at the image boundary), so every
//! Lanczos tap of a pixel comes from the tile that contains the pixel.

use std::collections::HashMap;

pub const TILE: u32 = 512;
pub const BORDER: u32 = 8;
pub const SLOT: u32 = TILE + 2 * BORDER;
/// Coarsest level of an ordinary image (JPEG scales 1, 2, 4, 8).
pub const MAX_LEVEL: u8 = 3;
/// Coarsest level any image may have (a whole-slide pyramid, scale 65536): the shaders
/// keep the levels present in 17 bits.
pub const LEVEL_CAP: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub image: u64,
    pub level: u8,
    pub tx: u32,
    pub ty: u32,
}

/// Size of a level in pixels.
pub fn level_size(w: u32, h: u32, level: u8) -> (u32, u32) {
    let f = 1u32 << level;
    (w.div_ceil(f), h.div_ceil(f))
}

/// Tile grid of a level.
pub fn level_grid(w: u32, h: u32, level: u8) -> (u32, u32) {
    let (lw, lh) = level_size(w, h, level);
    (lw.div_ceil(TILE), lh.div_ceil(TILE))
}

/// Level that is sharp at `scale` (screen px per full-res px): the coarsest level whose
/// pixels are not larger than screen pixels, at most `max`.
pub fn level_for_scale(scale: f32, max: u8) -> u8 {
    if scale >= 1.0 {
        return 0;
    }
    ((1.0 / scale).log2().floor().max(0.0) as u8).min(max)
}

/// The tiles of `level` covering the stored-pixel rectangle `r` (x0, y0, x1, y1), plus a
/// margin of `margin` tiles, nearest to the centre first.
pub fn tiles_covering(
    w: u32,
    h: u32,
    level: u8,
    r: (f32, f32, f32, f32),
    margin: u32,
) -> Vec<(u32, u32)> {
    let f = (1u32 << level) as f32;
    let (cols, rows) = level_grid(w, h, level);
    if cols == 0 || rows == 0 || r.2 <= r.0 || r.3 <= r.1 {
        return Vec::new();
    }
    let t = TILE as f32;
    let tx0 = ((r.0 / f / t).floor().max(0.0) as u32).saturating_sub(margin);
    let ty0 = ((r.1 / f / t).floor().max(0.0) as u32).saturating_sub(margin);
    let tx1 = (((r.2 / f / t).ceil() as u32) + margin).min(cols);
    let ty1 = (((r.3 / f / t).ceil() as u32) + margin).min(rows);
    let (cx, cy) = ((r.0 + r.2) * 0.5 / f / t, (r.1 + r.3) * 0.5 / f / t);
    let mut v: Vec<(u32, u32)> = (ty0..ty1)
        .flat_map(|y| (tx0..tx1).map(move |x| (x, y)))
        .collect();
    v.sort_by(|a, b| {
        let da = (a.0 as f32 + 0.5 - cx).powi(2) + (a.1 as f32 + 0.5 - cy).powi(2);
        let db = (b.0 as f32 + 0.5 - cx).powi(2) + (b.1 as f32 + 0.5 - cy).powi(2);
        da.total_cmp(&db)
    });
    v
}

/// Level-pixel rectangle (x, y, w, h) a set of tiles needs decoded, borders included.
pub fn decode_rect(w: u32, h: u32, level: u8, tiles: &[(u32, u32)]) -> (u32, u32, u32, u32) {
    let (lw, lh) = level_size(w, h, level);
    let mut r = (u32::MAX, u32::MAX, 0, 0);
    for &(tx, ty) in tiles {
        r.0 = r.0.min((tx * TILE).saturating_sub(BORDER));
        r.1 = r.1.min((ty * TILE).saturating_sub(BORDER));
        r.2 = r.2.max(((tx + 1) * TILE + BORDER).min(lw));
        r.3 = r.3.max(((ty + 1) * TILE + BORDER).min(lh));
    }
    if r.0 >= r.2 {
        return (0, 0, 0, 0);
    }
    (r.0, r.1, r.2 - r.0, r.3 - r.1)
}

/// Groups tiles into batches whose decode rectangles stay compact (one region decode each).
pub fn batches(tiles: &[(u32, u32)], max_tiles: usize) -> Vec<Vec<(u32, u32)>> {
    if tiles.is_empty() {
        return Vec::new();
    }
    // One batch per run of adjacent tiles in a tile row. Entropy-coded data can only be
    // read from the start of each MCU row, so a region decode pays for every column to its
    // left: batches that straddle rows (or split one row into several jobs) decode the same
    // MCU rows twice. The decoder spreads each batch over all cores itself.
    let mut sorted = tiles.to_vec();
    sorted.sort_by_key(|t| (t.1, t.0));
    let mut out: Vec<Vec<(u32, u32)>> = Vec::new();
    for t in sorted {
        match out.last_mut() {
            Some(b)
                if b.len() < max_tiles
                    && b.last().is_some_and(|o| o.1 == t.1 && o.0 + 1 == t.0) =>
            {
                b.push(t)
            }
            _ => out.push(vec![t]),
        }
    }
    out
}

/// LRU allocator of tile slots (array layers). Slots below [`TilePool::wide_from`] are
/// layers of the 8-bit tile array, the rest layers of the 16-bit one (images with wide
/// samples); a tile's slot is in the part its image uses.
#[derive(Debug)]
pub struct TilePool {
    slots: Vec<Option<TileKey>>,
    map: HashMap<TileKey, (u32, u64)>,
    clock: u64,
    wide_from: u32,
}

impl TilePool {
    pub fn new(capacity: u32) -> TilePool {
        TilePool {
            slots: vec![None; capacity as usize],
            map: HashMap::new(),
            clock: 1,
            wide_from: capacity,
        }
    }

    /// `narrow` 8-bit slots, then `wide` 16-bit ones; forgets every tile.
    pub fn split(&mut self, narrow: u32, wide: u32) {
        self.slots = vec![None; (narrow + wide) as usize];
        self.map.clear();
        self.wide_from = narrow;
    }

    /// All slots, both parts.
    pub fn capacity(&self) -> u32 {
        self.slots.len() as u32
    }

    /// The first 16-bit slot (= [`TilePool::capacity`] while there are none).
    pub fn wide_from(&self) -> u32 {
        self.wide_from
    }

    /// Slots of the part `wide` selects.
    pub fn part_capacity(&self, wide: bool) -> u32 {
        if wide {
            self.capacity() - self.wide_from
        } else {
            self.wide_from
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Starts a new frame: tiles touched from now on are protected from eviction.
    pub fn next_frame(&mut self) {
        self.clock += 1;
    }

    pub fn contains(&self, k: &TileKey) -> bool {
        self.map.contains_key(k)
    }

    /// Marks a tile as used this frame.
    pub fn touch(&mut self, k: &TileKey) -> Option<u32> {
        let clock = self.clock;
        self.map.get_mut(k).map(|e| {
            e.1 = clock;
            e.0
        })
    }

    /// Slot for a new 8-bit tile, evicting the least recently used tile not used this frame.
    pub fn alloc(&mut self, k: TileKey) -> Option<u32> {
        self.alloc_in(k, false)
    }

    /// Slot for a new tile in the 8-bit (`wide` false) or 16-bit part.
    pub fn alloc_in(&mut self, k: TileKey, wide: bool) -> Option<u32> {
        if let Some(s) = self.touch(&k) {
            return Some(s);
        }
        let part = if wide {
            self.wide_from..self.capacity()
        } else {
            0..self.wide_from
        };
        let free = self.slots[part.start as usize..part.end as usize]
            .iter()
            .position(|s| s.is_none());
        let slot = if let Some(i) = free {
            part.start + i as u32
        } else {
            let (victim, (slot, _)) = self
                .map
                .iter()
                .filter(|(_, (s, t))| *t < self.clock && part.contains(s))
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, v)| (*k, *v))?;
            self.map.remove(&victim);
            slot
        };
        self.slots[slot as usize] = Some(k);
        self.map.insert(k, (slot, self.clock));
        Some(slot)
    }

    /// Forgets every tile of an image.
    pub fn evict_image(&mut self, image: u64) {
        self.map.retain(|k, _| k.image != image);
        for s in &mut self.slots {
            if s.is_some_and(|k| k.image == image) {
                *s = None;
            }
        }
    }

    /// Tiles of an image: (level, tx, ty, slot).
    pub fn tiles_of(&self, image: u64) -> impl Iterator<Item = (u8, u32, u32, u32)> + '_ {
        self.map
            .iter()
            .filter(move |(k, _)| k.image == image)
            .map(|(k, (s, _))| (k.level, k.tx, k.ty, *s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry() {
        assert_eq!(level_size(22000, 4897, 3), (2750, 613));
        assert_eq!(level_grid(22000, 4897, 0), (43, 10));
        assert_eq!(level_for_scale(1.5, MAX_LEVEL), 0);
        assert_eq!(level_for_scale(0.3, MAX_LEVEL), 1);
        assert_eq!(level_for_scale(0.01, MAX_LEVEL), MAX_LEVEL);
        assert_eq!(level_for_scale(0.01, LEVEL_CAP), 6);
        let t = tiles_covering(22000, 4897, 0, (1000.0, 1000.0, 2000.0, 1500.0), 0);
        assert_eq!(t.len(), 3 * 2);
        assert_eq!(decode_rect(22000, 4897, 0, &[(0, 0)]), (0, 0, 520, 520));
        assert_eq!(decode_rect(1000, 600, 0, &[(1, 1)]), (504, 504, 496, 96));
    }

    #[test]
    fn lru_protects_current_frame() {
        let mut p = TilePool::new(2);
        let k = |i| TileKey {
            image: 1,
            level: 0,
            tx: i,
            ty: 0,
        };
        assert!(p.alloc(k(0)).is_some());
        assert!(p.alloc(k(1)).is_some());
        assert!(p.alloc(k(2)).is_none());
        p.next_frame();
        p.touch(&k(1));
        assert_eq!(p.alloc(k(2)), Some(0));
        assert!(!p.contains(&k(0)));
    }

    #[test]
    fn parts_evict_only_their_own_slots() {
        let mut p = TilePool::new(4);
        let k = |image, i| TileKey {
            image,
            level: 0,
            tx: i,
            ty: 0,
        };
        p.split(2, 1);
        assert_eq!(
            (p.capacity(), p.part_capacity(false), p.part_capacity(true)),
            (3, 2, 1)
        );
        assert_eq!(p.alloc(k(1, 0)), Some(0));
        assert_eq!(p.alloc(k(1, 1)), Some(1));
        assert_eq!(p.alloc_in(k(2, 0), true), Some(2));
        p.next_frame();
        // A new 16-bit tile takes the 16-bit slot, never an 8-bit one.
        assert_eq!(p.alloc_in(k(2, 1), true), Some(2));
        assert!(p.contains(&k(1, 0)) && p.contains(&k(1, 1)) && !p.contains(&k(2, 0)));
        p.touch(&k(1, 1));
        assert_eq!(p.alloc(k(1, 2)), Some(0));
    }
}
