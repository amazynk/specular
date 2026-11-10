//! The offline base map: Natural Earth (public domain) 1:50m land, 1:110m lakes and 1:50m
//! country borders, in Web Mercator world units quantised to 1/2^[`BITS`]. Made by
//! `gen/world.py`, which documents the format; about 290 KiB in the binary.

use std::ops::Range;
use std::sync::OnceLock;

/// Quantisation of the world coordinates (the world is 2^BITS units wide).
pub const BITS: u32 = 20;
/// The world's width in quantised units.
pub const Q: u32 = 1 << BITS;

const DATA: &[u8] = include_bytes!("../data/world.bin");
const MAGIC: &[u8; 8] = b"SPMAPW01";

/// The decoded base map. Points of all layers live in one array; rings and lines are ranges
/// into it.
#[derive(Debug, Default)]
pub struct World {
    pub points: Vec<[u32; 2]>,
    /// Land polygons' rings (outer rings and holes; filled even-odd).
    pub land: Vec<Range<u32>>,
    /// Lake rings (cut out of the land, even-odd).
    pub lakes: Vec<Range<u32>>,
    /// Country borders on land (open lines).
    pub borders: Vec<Range<u32>>,
}

impl World {
    /// The built-in base map, decoded once.
    pub fn get() -> &'static World {
        static W: OnceLock<World> = OnceLock::new();
        W.get_or_init(|| World::decode(DATA).expect("the built-in base map decodes"))
    }

    /// Decodes the `gen/world.py` format; `None` when it is malformed.
    pub fn decode(data: &[u8]) -> Option<World> {
        let mut r = Reader {
            data: data.strip_prefix(MAGIC.as_slice())?,
        };
        let mut w = World::default();
        let mut layers: [Vec<Range<u32>>; 3] = Default::default();
        for layer in &mut layers {
            let n = r.varint()?;
            let (mut x, mut y) = (0i64, 0i64);
            for _ in 0..n {
                let k = usize::try_from(r.varint()?).ok()?;
                // Each point takes at least two bytes: refuse counts the data can't hold.
                if k > r.data.len() / 2 {
                    return None;
                }
                let start = u32::try_from(w.points.len()).ok()?;
                w.points.reserve(k);
                for _ in 0..k {
                    x += r.zigzag()?;
                    y += r.zigzag()?;
                    let q = i64::from(Q);
                    if !(0..=q).contains(&x) || !(0..=q).contains(&y) {
                        return None;
                    }
                    w.points.push([x as u32, y as u32]);
                }
                layer.push(start..u32::try_from(w.points.len()).ok()?);
            }
        }
        if !r.data.is_empty() {
            return None;
        }
        let [land, lakes, borders] = layers;
        w.land = land;
        w.lakes = lakes;
        w.borders = borders;
        Some(w)
    }

    /// The points of a ring or line.
    pub fn of(&self, r: &Range<u32>) -> &[[u32; 2]] {
        &self.points[r.start as usize..r.end as usize]
    }
}

struct Reader<'a> {
    data: &'a [u8],
}

impl Reader<'_> {
    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let (&b, rest) = self.data.split_first()?;
            self.data = rest;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }

    fn zigzag(&mut self) -> Option<i64> {
        let v = self.varint()?;
        Some((v >> 1) as i64 ^ -((v & 1) as i64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proj::project;

    #[test]
    fn built_in_map_decodes() {
        let w = World::get();
        assert!(w.land.len() > 1000 && w.lakes.len() > 10 && w.borders.len() > 300);
        assert!(w.points.len() > 70_000);
        assert!(w.land.iter().all(|r| r.end - r.start >= 3));
        assert!(w.borders.iter().all(|r| r.end - r.start >= 2));
    }

    /// Even-odd point in polygon over all land and lake rings.
    fn on_land(w: &World, lat: f64, lon: f64) -> bool {
        let (x, y) = project(lat, lon);
        let (px, py) = (x * f64::from(Q), y * f64::from(Q));
        let mut inside = false;
        for r in w.land.iter().chain(&w.lakes) {
            let p = w.of(r);
            let mut j = p.len() - 1;
            for i in 0..p.len() {
                let (xi, yi) = (f64::from(p[i][0]), f64::from(p[i][1]));
                let (xj, yj) = (f64::from(p[j][0]), f64::from(p[j][1]));
                if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
        }
        inside
    }

    #[test]
    fn land_is_where_it_should_be() {
        let w = World::get();
        for (name, lat, lon) in [
            ("Madrid", 40.4168, -3.7038),
            ("Kansas", 38.5, -98.0),
            ("Mongolia", 46.8, 103.0),
            ("Brazil", -10.0, -55.0),
            ("Australia", -25.0, 134.0),
        ] {
            assert!(on_land(w, lat, lon), "{name} is land");
        }
        for (name, lat, lon) in [
            ("Atlantic", 30.0, -40.0),
            ("Pacific", 0.0, -150.0),
            ("Mediterranean", 35.0, 18.0),
            ("Lake Superior", 47.7, -87.5),
            ("Caspian", 42.0, 50.5),
        ] {
            assert!(!on_land(w, lat, lon), "{name} is water");
        }
    }

    #[test]
    fn malformed_data_is_refused() {
        assert!(World::decode(b"").is_none());
        assert!(World::decode(b"SPMAPW02\0\0\0").is_none());
        // Truncated: a ring of 3 points with none given.
        assert!(World::decode(b"SPMAPW01\x01\x03").is_none());
        // Trailing bytes.
        assert!(World::decode(b"SPMAPW01\0\0\0\0").is_none());
        // Out of the world.
        assert!(World::decode(b"SPMAPW01\x01\x02\x01\x01\x01\x01\0\0").is_none());
        let ok = World::decode(b"SPMAPW01\0\0\x01\x02\x02\x02\x02\x02").unwrap();
        assert_eq!(ok.points, vec![[1, 1], [2, 2]]);
        assert_eq!(ok.borders, vec![0..2]);
    }
}
