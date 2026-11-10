//! The `cmap` table: the best Unicode subtable (formats 0, 4, 6, 12, 13) as a sorted list
//! of (code point, glyph).

use crate::read::{Reader, u16_at, u32_at};

/// Most code points listed (all of Unicode).
const MAX_CHARS: u32 = 0x11_0000;

/// How good a subtable is: full-repertoire Unicode first, then BMP Unicode, then the rest.
fn rank(platform: u16, encoding: u16, format: u16) -> Option<u8> {
    let unicode_full = matches!((platform, encoding), (3, 10) | (0, 4) | (0, 6));
    let unicode_bmp = matches!((platform, encoding), (3, 1) | (0, 0..=3));
    Some(match format {
        12 if unicode_full || unicode_bmp => 0,
        4 if unicode_bmp || unicode_full => 1,
        6 | 0 if unicode_bmp || unicode_full => 2,
        13 if unicode_full || unicode_bmp => 3,
        4 | 6 | 0 if platform == 3 && encoding == 0 => 4,
        0 | 6 if platform == 1 && encoding == 0 => 5,
        _ => return None,
    })
}

/// The mapped characters of `cmap`, sorted by code point, glyph 0 left out.
pub(crate) fn parse(cmap: &[u8], num_glyphs: u16) -> Vec<(u32, u16)> {
    let Some(n) = u16_at(cmap, 2) else {
        return Vec::new();
    };
    let mut best: Option<(u8, usize)> = None;
    for i in 0..n as usize {
        let mut r = Reader::at(cmap, 4 + i * 8);
        let (Some(p), Some(e), Some(off)) = (r.u16(), r.u16(), r.u32()) else {
            break;
        };
        let Some(format) = u16_at(cmap, off as usize) else {
            continue;
        };
        if let Some(k) = rank(p, e, format)
            && best.is_none_or(|b| k < b.0)
        {
            best = Some((k, off as usize));
        }
    }
    let Some((_, off)) = best else {
        return Vec::new();
    };
    let sub = &cmap[off..];
    let mut out = Vec::new();
    let mut push = |c: u32, g: u32| {
        if g != 0 && g < num_glyphs as u32 && c < MAX_CHARS {
            out.push((c, g as u16));
        }
    };
    match u16_at(sub, 0) {
        Some(0) => {
            for c in 0..256u32 {
                if let Some(&g) = sub.get(6 + c as usize) {
                    push(c, g as u32);
                }
            }
        }
        Some(4) => format4(sub, &mut push),
        Some(6) => {
            let (Some(first), Some(count)) = (u16_at(sub, 6), u16_at(sub, 8)) else {
                return Vec::new();
            };
            for i in 0..count as u32 {
                if let Some(g) = u16_at(sub, 10 + i as usize * 2) {
                    push(first as u32 + i, g as u32);
                }
            }
        }
        Some(f @ (12 | 13)) => {
            let n = u32_at(sub, 12).unwrap_or(0) as usize;
            let mut total = 0u32;
            for i in 0..n {
                let at = 16 + i * 12;
                let (Some(s), Some(e), Some(g)) =
                    (u32_at(sub, at), u32_at(sub, at + 4), u32_at(sub, at + 8))
                else {
                    break;
                };
                if s > e || e >= MAX_CHARS {
                    continue;
                }
                total = total.saturating_add(e - s + 1);
                if total > MAX_CHARS {
                    break;
                }
                for c in s..=e {
                    push(c, if f == 12 { g.wrapping_add(c - s) } else { g });
                }
            }
        }
        _ => {}
    }
    out.sort_unstable_by_key(|p| p.0);
    out.dedup_by_key(|p| p.0);
    out
}

fn format4(sub: &[u8], push: &mut impl FnMut(u32, u32)) {
    let Some(seg2) = u16_at(sub, 6) else { return };
    let segs = seg2 as usize / 2;
    let ends = 14;
    let starts = ends + seg2 as usize + 2;
    let deltas = starts + seg2 as usize;
    let ranges = deltas + seg2 as usize;
    // Segments cover each 16-bit code at most once: overlapping ones (32767 segments of
    // 65535 codes each) would push two billion mappings.
    let mut total = 0u32;
    for s in 0..segs {
        let (Some(end), Some(start), Some(delta), Some(ro)) = (
            u16_at(sub, ends + s * 2),
            u16_at(sub, starts + s * 2),
            u16_at(sub, deltas + s * 2),
            u16_at(sub, ranges + s * 2),
        ) else {
            return;
        };
        if start > end {
            continue;
        }
        total += u32::from(end - start) + 1;
        if total > 0x1_0000 {
            return;
        }
        for c in start as u32..=end as u32 {
            if c == 0xFFFF {
                break;
            }
            let g = if ro == 0 {
                (c + delta as u32) & 0xFFFF
            } else {
                let at = ranges + s * 2 + ro as usize + 2 * (c - start as u32) as usize;
                match u16_at(sub, at) {
                    Some(0) | None => 0,
                    Some(g) => (g as u32 + delta as u32) & 0xFFFF,
                }
            };
            push(c, g);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2000 format-4 segments each covering every code: they used to push 131 million
    /// mappings (two billion with 32767 segments).
    #[test]
    fn overlapping_format4_segments() {
        let segs = 2000usize;
        let mut sub = Vec::new();
        let be = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_be_bytes());
        for x in [4u16, 0, 0, (segs * 2) as u16, 0, 0, 0] {
            be(&mut sub, x);
        }
        for _ in 0..segs {
            be(&mut sub, 0xFFFE); // end
        }
        be(&mut sub, 0);
        for _ in 0..segs {
            be(&mut sub, 0); // start
        }
        for _ in 0..segs {
            be(&mut sub, 1); // delta
        }
        for _ in 0..segs {
            be(&mut sub, 0); // range offset
        }
        let mut cmap = Vec::new();
        for x in [0u16, 1, 3, 1] {
            be(&mut cmap, x);
        }
        cmap.extend_from_slice(&12u32.to_be_bytes());
        cmap.extend_from_slice(&sub);
        let t = std::time::Instant::now();
        let m = parse(&cmap, u16::MAX);
        assert_eq!(m.len(), 0xFFFE);
        assert!(t.elapsed().as_secs() < 5);
    }
}
