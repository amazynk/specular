//! World coastlines for an overlay on georeferenced images: Natural Earth's 1:110m
//! coastlines (public domain, naturalearthdata.com), packed by `gen/make_coastlines.py` as
//! varint deltas in thousandths of a degree (20 KB).

use std::sync::OnceLock;

const DATA: &[u8] = include_bytes!("coastline110.bin");

/// Each coastline as a polyline of (longitude, latitude) in degrees, −180..180 and
/// −90..90, WGS 84.
pub fn coastlines() -> &'static [Vec<[f32; 2]>] {
    static LINES: OnceLock<Vec<Vec<[f32; 2]>>> = OnceLock::new();
    LINES.get_or_init(|| parse(DATA).unwrap_or_default())
}

fn parse(b: &[u8]) -> Option<Vec<Vec<[f32; 2]>>> {
    let mut pos = 4;
    let mut next = || -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *b.get(pos)?;
            pos += 1;
            v |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    };
    if b.get(..4)? != b"NECL" || next()? != 1 {
        return None;
    }
    let unzig = |v: u64| (v >> 1) as i64 ^ -((v & 1) as i64);
    let count = next()? as usize;
    let mut lines = Vec::with_capacity(count.min(1 << 16));
    for _ in 0..count {
        let n = next()? as usize;
        let mut line = Vec::with_capacity(n.min(1 << 20));
        let (mut x, mut y) = (0i64, 0i64);
        for _ in 0..n {
            x += unzig(next()?);
            y += unzig(next()?);
            line.push([(x as f64 / 1e3) as f32, (y as f64 / 1e3) as f32]);
        }
        lines.push(line);
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_earth_110m() {
        let c = coastlines();
        assert_eq!(c.len(), 134);
        assert_eq!(c.iter().map(Vec::len).sum::<usize>(), 5128);
        assert!(
            c.iter()
                .flatten()
                .all(|p| p[0].abs() <= 180.0 && p[1].abs() <= 90.0)
        );
        // Some line passes the Strait of Gibraltar.
        assert!(
            c.iter()
                .flatten()
                .any(|p| (p[0] + 5.6).abs() < 0.5 && (p[1] - 36.0).abs() < 0.5)
        );
        assert!(parse(b"NECL\x02\x00").is_none());
        assert!(parse(b"NECL\x01\x05\x02").is_none());
    }
}
