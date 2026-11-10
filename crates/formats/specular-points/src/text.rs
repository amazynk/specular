//! Points as text columns: `.xyz`/`.txt` (x y z, then optionally intensity and/or r g b,
//! separated by spaces, tabs, commas or semicolons) and Leica `.pts` (a count line, then
//! x y z intensity r g b). The columns' meaning is sniffed from the first lines; lines that
//! are not numbers (headers, comments) are skipped. Parsing runs in parallel over pieces of
//! the file that start at line breaks.

use rayon::prelude::*;

use crate::build;
use crate::{Channels, CloudInfo, Error, Limits, PointCloud, PointFormat, Result};

/// Lines looked at to sniff the columns.
const SNIFF_LINES: usize = 200;

/// Lines longer than this are not point lines.
const MAX_LINE: usize = 4096;

/// One slice's points: positions, packed attributes, and the rows seen.
type Part = (Vec<[f32; 3]>, Vec<[u32; 2]>, u64);

/// What the columns hold.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Columns {
    sep: u8,
    /// The intensity's column, and how to bring it to 16 bits.
    intensity: Option<(usize, Scale)>,
    /// The first of three colour columns.
    rgb: Option<(usize, Scale)>,
    /// Fewest columns a point line has.
    min: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scale {
    /// 0‥1 floats.
    Unit,
    /// 0‥255 integers.
    Byte,
    /// 0‥65535 integers.
    Word,
    /// PTS intensity: −2048‥2047.
    Pts,
}

impl Scale {
    fn to8(self, v: f64) -> u8 {
        match self {
            Scale::Unit => (v * 255.0).round().clamp(0.0, 255.0) as u8,
            Scale::Word => (v / 257.0).round().clamp(0.0, 255.0) as u8,
            _ => v.round().clamp(0.0, 255.0) as u8,
        }
    }

    fn to16(self, v: f64) -> u16 {
        match self {
            Scale::Unit => (v * 65_535.0).round().clamp(0.0, 65_535.0) as u16,
            Scale::Pts => ((v + 2048.0) * 16.0).round().clamp(0.0, 65_535.0) as u16,
            _ => v.round().clamp(0.0, 65_535.0) as u16,
        }
    }
}

fn fields(line: &[u8], sep: u8) -> impl Iterator<Item = &[u8]> {
    line.split(move |&c| c == sep || (sep == b' ' && c == b'\t'))
        .map(|f| f.trim_ascii())
        .filter(move |f| sep != b' ' || !f.is_empty())
}

fn number(f: &[u8]) -> Option<f64> {
    std::str::from_utf8(f).ok()?.parse::<f64>().ok()
}

/// Numbers of a line, or `None` when it is not a point line.
fn numbers(line: &[u8], sep: u8, out: &mut Vec<f64>) -> bool {
    out.clear();
    if line.len() > MAX_LINE {
        return false;
    }
    for f in fields(line, sep) {
        match number(f) {
            Some(v) => out.push(v),
            None => return false,
        }
    }
    out.len() >= 3
}

fn lines(b: &[u8]) -> impl Iterator<Item = &[u8]> {
    b.split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

fn sniff(b: &[u8], pts: bool) -> Result<Columns> {
    let sample: Vec<&[u8]> = lines(b)
        .filter(|l| !l.trim_ascii().is_empty())
        .take(SNIFF_LINES + 1)
        .collect();
    // The separator splitting most lines into ≥ 3 numbers.
    let mut best = None;
    let mut row = Vec::new();
    for sep in *b" ,;\t" {
        let mut rows: Vec<Vec<f64>> = Vec::new();
        for l in &sample {
            if numbers(l, sep, &mut row) {
                rows.push(row.clone());
            }
        }
        if rows.len()
            > best
                .as_ref()
                .map_or(0, |(_, r): &(u8, Vec<Vec<f64>>)| r.len())
        {
            best = Some((sep, rows));
        }
    }
    let Some((sep, rows)) = best else {
        return Err(Error::Malformed("no x y z columns".into()));
    };
    let min = rows.iter().map(Vec::len).min().unwrap_or(3);
    // A triple of columns from `at` that holds colours in all rows, and how they scale.
    let colour_at = |at: usize| -> Option<Scale> {
        if at + 3 > min {
            return None;
        }
        let vals = rows.iter().flat_map(|r| &r[at..at + 3]);
        let ints = vals.clone().all(|v| v.fract() == 0.0);
        let max = vals.clone().fold(0.0f64, |m, &v| m.max(v));
        let neg = vals.clone().any(|&v| v < 0.0);
        if neg {
            return None;
        }
        if ints && max <= 255.0 {
            Some(Scale::Byte)
        } else if ints && max <= 65_535.0 {
            Some(Scale::Word)
        } else if !ints && max <= 1.0 {
            Some(Scale::Unit)
        } else {
            None
        }
    };
    let intensity_at = |at: usize| -> Option<Scale> {
        if at >= min {
            return None;
        }
        if pts {
            return Some(Scale::Pts);
        }
        let vals = rows.iter().map(|r| r[at]);
        let max = vals.clone().fold(0.0f64, f64::max);
        let ints = vals.clone().all(|v| v.fract() == 0.0);
        if vals.clone().any(|v| v < 0.0) {
            return None;
        }
        Some(if !ints && max <= 1.0 {
            Scale::Unit
        } else {
            Scale::Word
        })
    };
    let (mut intensity, mut rgb) = (None, None);
    if pts || min == 7 {
        // x y z i r g b
        if let Some(s) = colour_at(4) {
            rgb = Some((4, s));
        }
        intensity = intensity_at(3).map(|s| (3, s));
    } else if let Some(s) = colour_at(3) {
        // x y z r g b […]
        rgb = Some((3, s));
        if min >= 7 && rows.iter().all(|r| r[6] >= 0.0) {
            intensity = intensity_at(6).map(|s| (6, s));
        }
    } else if min == 4 {
        intensity = intensity_at(3).map(|s| (3, s));
    } else if min >= 9
        && let Some(s) = colour_at(6)
    {
        // x y z nx ny nz r g b
        rgb = Some((6, s));
    }
    Ok(Columns {
        sep,
        intensity,
        rgb,
        min,
    })
}

pub(crate) fn read(format: PointFormat, b: &[u8], limits: &Limits) -> Result<PointCloud> {
    let pts = format == PointFormat::Pts;
    let cols = sniff(b, pts)?;
    // Lines in the file, from the sample's average length.
    let head: Vec<&[u8]> = lines(b).take(SNIFF_LINES).collect();
    let avg = (head.iter().map(|l| l.len() + 1).sum::<usize>() / head.len().max(1)).max(8);
    let expected = (b.len() / avg) as u64 + 1;
    let stride = expected.div_ceil(limits.max_points.max(1)).max(1);
    let mut row = Vec::new();
    let origin = lines(b)
        .find(|l| numbers(l, cols.sep, &mut row) && row.len() >= cols.min)
        .map(|_| [row[0].round(), row[1].round(), row[2].round()])
        .ok_or(Error::Empty)?;

    // Pieces of about 4 MiB starting after a line break; each keeps the lines whose index in
    // the piece is a multiple of the stride (even enough, and needs no global line count).
    let piece = 4 << 20;
    let mut starts = vec![0usize];
    while let Some(&s) = starts.last() {
        let at = s + piece;
        if at >= b.len() {
            break;
        }
        match b[at..].iter().position(|&c| c == b'\n') {
            Some(e) => starts.push(at + e + 1),
            None => break,
        }
    }
    starts.push(b.len());
    let parts: Vec<Part> = starts
        .par_windows(2)
        .map(|w| {
            let mut pos = Vec::new();
            let mut attrs = Vec::new();
            let mut seen = 0u64;
            let mut row = Vec::new();
            for l in lines(&b[w[0]..w[1]]) {
                if !numbers(l, cols.sep, &mut row) || row.len() < cols.min {
                    continue;
                }
                seen += 1;
                if !(seen - 1).is_multiple_of(stride) {
                    continue;
                }
                let p = [row[0], row[1], row[2]];
                if !p.iter().all(|v| v.is_finite()) {
                    continue;
                }
                pos.push([0, 1, 2].map(|i| (p[i] - origin[i]) as f32));
                let rgb = cols.rgb.map_or([0; 3], |(at, s)| {
                    [s.to8(row[at]), s.to8(row[at + 1]), s.to8(row[at + 2])]
                });
                let i = cols.intensity.map_or(0, |(at, s)| s.to16(row[at]));
                attrs.push([
                    u32::from(rgb[0]) | (u32::from(rgb[1]) << 8) | (u32::from(rgb[2]) << 16),
                    u32::from(i),
                ]);
            }
            (pos, attrs, seen)
        })
        .collect();
    let total: u64 = parts.iter().map(|p| p.2).sum();
    let n: usize = parts.iter().map(|p| p.0.len()).sum();
    let mut positions = Vec::with_capacity(n);
    let mut attrs = Vec::with_capacity(n);
    for (p, a, _) in parts {
        positions.extend(p);
        attrs.extend(a);
    }
    // A file much larger than its sample suggested: halve until within the cap.
    while positions.len() as u64 > limits.max_points.saturating_mul(2) {
        let mut i = 0;
        positions.retain(|_| {
            i += 1;
            i % 2 == 1
        });
        let mut i = 0;
        attrs.retain(|_| {
            i += 1;
            i % 2 == 1
        });
    }
    let info = CloudInfo {
        format: Some(format),
        total,
        channels: Channels {
            rgb: cols.rgb.is_some(),
            intensity: cols.intensity.is_some(),
            classification: false,
            returns: false,
        },
        ..CloudInfo::default()
    };
    build::finish(positions, attrs, info, origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_columns() {
        let c = sniff(b"x,y,z,r,g,b\n1.5,2,3,255,0,10\n2,3,4,0,128,7\n", false).unwrap();
        assert_eq!(c.sep, b',');
        assert_eq!(c.rgb, Some((3, Scale::Byte)));
        assert_eq!(c.intensity, None);
        let c = sniff(b"3\n1 2 3 -100 10 20 30\n4 5 6 2000 1 2 3\n", true).unwrap();
        assert_eq!(c.rgb, Some((4, Scale::Byte)));
        assert_eq!(c.intensity, Some((3, Scale::Pts)));
        let c = sniff(b"1 2 3 0.5\n4 5 6 0.25\n", false).unwrap();
        assert_eq!(c.intensity, Some((3, Scale::Unit)));
        assert!(sniff(b"hello world\n", false).is_err());
    }
}
