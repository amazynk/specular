//! TeX font metric (TFM) files: each character's width, height and depth as fractions of the
//! design size (TeX: The Program, §539–§545).

/// A font's character dimensions, as fix_words (20 fractional bits) of the design size.
#[derive(Clone, Debug, PartialEq)]
pub struct Tfm {
    pub checksum: u32,
    /// Design size in points × 2^20.
    pub design_size: i32,
    /// By character code: (width, height, depth).
    pub chars: Vec<Option<(i32, i32, i32)>>,
}

impl Tfm {
    /// Character `c`'s (width, height, depth) in DVI units at scaled size `scale`.
    pub fn dims(&self, c: u32, scale: u32) -> Option<(i64, i64, i64)> {
        let (w, h, d) = (*self.chars.get(c as usize)?)?;
        let s = |f: i32| (f as i64 * scale as i64) >> 20;
        Some((s(w), s(h), s(d)))
    }
}

fn word(d: &[u8], i: usize) -> Option<u32> {
    let b = d.get(i * 4..i * 4 + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parses a TFM file; `None` when it is not one or is cut short.
pub fn parse(d: &[u8]) -> Option<Tfm> {
    let half = |i: usize| -> Option<usize> {
        let b = d.get(i * 2..i * 2 + 2)?;
        Some(u16::from_be_bytes([b[0], b[1]]) as usize)
    };
    let (lf, lh, bc, ec, nw, nh, nd) = (
        half(0)?,
        half(1)?,
        half(2)?,
        half(3)?,
        half(4)?,
        half(5)?,
        half(6)?,
    );
    if lf * 4 > d.len() || lh < 2 || ec > 255 || (bc > ec + 1) {
        return None;
    }
    let chars_at = 6 + lh;
    let n = if bc > ec { 0 } else { ec - bc + 1 };
    let widths_at = chars_at + n;
    let heights_at = widths_at + nw;
    let depths_at = heights_at + nh;
    if depths_at + nd > lf {
        return None;
    }
    let mut chars = vec![None; 256];
    for i in 0..n {
        let ci = word(d, chars_at + i)?;
        let wi = (ci >> 24) as usize;
        if wi == 0 {
            continue;
        }
        let hi = ((ci >> 20) & 15) as usize;
        let di = ((ci >> 16) & 15) as usize;
        let get = |at: usize, idx: usize, count: usize| -> Option<i32> {
            (idx < count)
                .then(|| word(d, at + idx).map(|v| v as i32))
                .flatten()
        };
        chars[bc + i] = Some((
            get(widths_at, wi, nw)?,
            get(heights_at, hi, nh).unwrap_or(0),
            get(depths_at, di, nd).unwrap_or(0),
        ));
    }
    Some(Tfm {
        checksum: word(d, 6)?,
        design_size: word(d, 7)? as i32,
        chars,
    })
}
