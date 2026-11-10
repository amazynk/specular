//! Reading a QR symbol's modules: format and version information, the function patterns,
//! the mask, the codewords in their zigzag order, and the blocks they interleave.

use std::sync::OnceLock;

use super::tables::{self, Level};
use crate::rs::Field;

/// A square grid of modules, dark = true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub size: usize,
    pub dark: Vec<bool>,
}

impl Grid {
    pub fn new(size: usize) -> Grid {
        Grid {
            size,
            dark: vec![false; size * size],
        }
    }

    /// Module at column `x`, row `y`.
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.size + x]
    }

    pub fn set(&mut self, x: usize, y: usize, v: bool) {
        self.dark[y * self.size + x] = v;
    }

    /// The grid mirrored about its main diagonal (a symbol seen from behind).
    pub fn transposed(&self) -> Grid {
        let mut g = Grid::new(self.size);
        for y in 0..self.size {
            for x in 0..self.size {
                g.set(y, x, self.get(x, y));
            }
        }
        g
    }
}

/// Why a grid did not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    Format,
    Version,
    Correction,
    Content,
}

fn bch_remainder(value: u32, poly: u32, poly_bits: u32) -> u32 {
    let deg = poly_bits - 1;
    let mut v = value << deg;
    for i in (deg..32).rev() {
        if v & (1 << i) != 0 && i >= deg {
            v ^= poly << (i - deg);
        }
    }
    v
}

/// The 15-bit format word for 5 data bits.
pub(crate) fn format_word(data: u32) -> u32 {
    ((data << 10) | bch_remainder(data, 0x537, 11)) ^ 0x5412
}

/// The 18-bit version word for versions 7..=40.
pub(crate) fn version_word(v: u32) -> u32 {
    (v << 12) | bch_remainder(v, 0x1F25, 13)
}

/// The closest of the 32 format words to either read copy (at most 3 bits apart).
fn decode_format(a: u32, b: u32) -> Option<(Level, u8)> {
    let mut best = (u32::MAX, 0u32);
    for d in 0..32 {
        let w = format_word(d);
        let dist = (w ^ a).count_ones().min((w ^ b).count_ones());
        if dist < best.0 {
            best = (dist, d);
        }
    }
    (best.0 <= 3).then(|| (Level::from_bits((best.1 >> 3) as u8), (best.1 & 7) as u8))
}

fn decode_version(a: u32, b: u32) -> Option<usize> {
    let mut best = (u32::MAX, 0usize);
    for v in 7..=40u32 {
        let w = version_word(v);
        let dist = (w ^ a).count_ones().min((w ^ b).count_ones());
        if dist < best.0 {
            best = (dist, v as usize);
        }
    }
    (best.0 <= 3).then_some(best.1)
}

/// The two copies of the format information.
fn read_format(g: &Grid) -> (u32, u32) {
    let n = g.size;
    let bit = |x: usize, y: usize| u32::from(g.get(x, y));
    let mut a = 0u32;
    // bit i of the word, by position (column, row)
    let mut first: Vec<(usize, usize)> = (0..=5).map(|i| (8, i)).collect();
    first.extend([(8, 7), (8, 8), (7, 8)]);
    first.extend((9..15).map(|i| (14 - i, 8)));
    for (i, &(x, y)) in first.iter().enumerate() {
        a |= bit(x, y) << i;
    }
    let mut b = 0u32;
    let mut second: Vec<(usize, usize)> = (0..8).map(|i| (n - 1 - i, 8)).collect();
    second.extend((8..15).map(|i| (8, n - 15 + i)));
    for (i, &(x, y)) in second.iter().enumerate() {
        b |= bit(x, y) << i;
    }
    (a, b)
}

fn read_version(g: &Grid) -> (u32, u32) {
    let n = g.size;
    let (mut a, mut b) = (0u32, 0u32);
    for i in 0..18 {
        let (p, q) = (n - 11 + i % 3, i / 3);
        a |= u32::from(g.get(p, q)) << i; // top right: columns n-11..n-8
        b |= u32::from(g.get(q, p)) << i; // bottom left: rows n-11..n-8
    }
    (a, b)
}

/// The version a grid's version information names (either copy, mirrored or not), when
/// readable: for symbols whose size was misjudged.
pub(crate) fn version_hint(g: &Grid) -> Option<usize> {
    if g.size < 45 {
        return None;
    }
    let (a, b) = read_version(g);
    decode_version(a, b).or_else(|| {
        let (a, b) = read_version(&g.transposed());
        decode_version(a, b)
    })
}

/// Which modules are function patterns (not data) for a version.
pub(crate) fn function_modules(version: usize) -> Vec<bool> {
    let n = tables::size(version);
    let mut f = vec![false; n * n];
    let mut mark = |x: usize, y: usize| f[y * n + x] = true;
    // finders with separators, and the format areas next to them
    for y in 0..9 {
        for x in 0..9 {
            mark(x, y);
        }
    }
    for y in 0..9 {
        for x in n - 8..n {
            mark(x, y);
        }
    }
    for y in n - 8..n {
        for x in 0..9 {
            mark(x, y);
        }
    }
    // timing
    for i in 0..n {
        mark(6, i);
        mark(i, 6);
    }
    // alignment patterns
    let al = tables::alignment(version);
    let last = al.len().saturating_sub(1);
    for (i, &cy) in al.iter().enumerate() {
        for (j, &cx) in al.iter().enumerate() {
            if (i == 0 && j == 0) || (i == 0 && j == last) || (i == last && j == 0) {
                continue;
            }
            for y in cy - 2..=cy + 2 {
                for x in cx - 2..=cx + 2 {
                    mark(x, y);
                }
            }
        }
    }
    // version information
    if version >= 7 {
        for i in 0..6 {
            for j in n - 11..n - 8 {
                mark(j, i);
                mark(i, j);
            }
        }
    }
    f
}

fn mask(m: u8, x: usize, y: usize) -> bool {
    let (i, j) = (y, x);
    match m {
        0 => (i + j) % 2 == 0,
        1 => i % 2 == 0,
        2 => j % 3 == 0,
        3 => (i + j) % 3 == 0,
        4 => (i / 2 + j / 3) % 2 == 0,
        5 => (i * j) % 2 + (i * j) % 3 == 0,
        6 => ((i * j) % 2 + (i * j) % 3) % 2 == 0,
        _ => ((i + j) % 2 + (i * j) % 3) % 2 == 0,
    }
}

/// The order data modules are read in: two columns at a time from the right, upwards and
/// downwards in turn, skipping the vertical timing column.
pub(crate) fn data_positions(version: usize) -> Vec<(usize, usize)> {
    let n = tables::size(version);
    let func = function_modules(version);
    let mut out = Vec::with_capacity(tables::raw_modules(version));
    let mut right = n as isize - 1;
    while right >= 1 {
        if right == 6 {
            right = 5;
        }
        let r = right as usize;
        let upward = (r + 1) & 2 == 0;
        for vert in 0..n {
            for j in 0..2 {
                let x = r - j;
                let y = if upward { n - 1 - vert } else { vert };
                if !func[y * n + x] {
                    out.push((x, y));
                }
            }
        }
        right -= 2;
    }
    out
}

/// [`data_positions`] of every version, made once.
fn positions(version: usize) -> &'static [(u8, u8)] {
    static P: OnceLock<Vec<Vec<(u8, u8)>>> = OnceLock::new();
    &P.get_or_init(|| {
        (1..=40)
            .map(|v| {
                data_positions(v)
                    .into_iter()
                    .map(|(x, y)| (x as u8, y as u8))
                    .collect()
            })
            .collect()
    })[version - 1]
}

fn qr_field() -> &'static Field {
    static F: OnceLock<Field> = OnceLock::new();
    F.get_or_init(|| Field::new(0x11D, 0))
}

/// What a grid holds before its content is parsed.
#[derive(Clone, Debug)]
pub struct Codewords {
    pub version: usize,
    pub level: Level,
    pub mask: u8,
    pub data: Vec<u8>,
    /// Bytes corrected by Reed–Solomon.
    pub corrected: usize,
}

/// Reads and corrects a grid's data codewords.
pub fn read(g: &Grid) -> Result<Codewords, Fail> {
    if g.size < 21 || !(g.size - 17).is_multiple_of(4) || g.size > 177 {
        return Err(Fail::Version);
    }
    let (fa, fb) = read_format(g);
    let (level, m) = decode_format(fa, fb).ok_or(Fail::Format)?;
    let mut version = (g.size - 17) / 4;
    if version >= 7 {
        let (va, vb) = read_version(g);
        // trust the size when the version information is unreadable
        if let Some(v) = decode_version(va, vb)
            && v != version
        {
            return Err(Fail::Version);
        }
    }
    if version == 0 {
        version = 1;
    }
    let pos = positions(version);
    let total = tables::raw_modules(version) / 8;
    let mut raw = vec![0u8; total];
    for (k, &(x, y)) in pos.iter().take(total * 8).enumerate() {
        let (x, y) = (usize::from(x), usize::from(y));
        if g.get(x, y) ^ mask(m, x, y) {
            raw[k / 8] |= 0x80 >> (k % 8);
        }
    }
    let blocks = tables::blocks(version, level);
    let longest = blocks.iter().map(|b| b.data).max().unwrap_or(0);
    let mut per: Vec<Vec<u8>> = blocks
        .iter()
        .map(|b| Vec::with_capacity(b.data + b.ecc))
        .collect();
    let mut at = 0;
    for i in 0..longest {
        for (k, b) in blocks.iter().enumerate() {
            if i < b.data {
                per[k].push(raw[at]);
                at += 1;
            }
        }
    }
    let ecc = blocks[0].ecc;
    for _ in 0..ecc {
        for p in per.iter_mut() {
            p.push(raw[at]);
            at += 1;
        }
    }
    let f = qr_field();
    let mut data = Vec::new();
    let mut corrected = 0;
    for (k, b) in blocks.iter().enumerate() {
        corrected += f.correct(&mut per[k], b.ecc).ok_or(Fail::Correction)?;
        data.extend_from_slice(&per[k][..b.data]);
    }
    Ok(Codewords {
        version,
        level,
        mask: m,
        data,
        corrected,
    })
}

/// Draws a symbol (for tests): codewords already interleaved with their check bytes.
#[cfg(test)]
pub(crate) fn draw(version: usize, level: Level, mask_no: u8, data: &[u8]) -> Grid {
    let n = tables::size(version);
    let mut g = Grid::new(n);
    let f = qr_field();
    // blocks with check bytes, interleaved
    let blocks = tables::blocks(version, level);
    let mut per = Vec::new();
    let mut at = 0;
    for b in &blocks {
        let d = data[at..at + b.data].to_vec();
        at += b.data;
        let e = f.encode(&d, b.ecc);
        per.push((d, e));
    }
    let longest = blocks.iter().map(|b| b.data).max().unwrap();
    let mut raw = Vec::new();
    for i in 0..longest {
        for (d, _) in &per {
            if i < d.len() {
                raw.push(d[i]);
            }
        }
    }
    for i in 0..blocks[0].ecc {
        for (_, e) in &per {
            raw.push(e[i]);
        }
    }
    // finders
    let finder = |g: &mut Grid, ox: usize, oy: usize| {
        for dy in 0..7 {
            for dx in 0..7 {
                let edge = dx == 0 || dy == 0 || dx == 6 || dy == 6;
                let centre = (2..=4).contains(&dx) && (2..=4).contains(&dy);
                g.set(ox + dx, oy + dy, edge || centre);
            }
        }
    };
    finder(&mut g, 0, 0);
    finder(&mut g, n - 7, 0);
    finder(&mut g, 0, n - 7);
    for i in 8..n - 8 {
        g.set(6, i, i % 2 == 0);
        g.set(i, 6, i % 2 == 0);
    }
    let al = tables::alignment(version);
    let last = al.len().saturating_sub(1);
    for (i, &cy) in al.iter().enumerate() {
        for (j, &cx) in al.iter().enumerate() {
            if (i == 0 && j == 0) || (i == 0 && j == last) || (i == last && j == 0) {
                continue;
            }
            for dy in 0..5 {
                for dx in 0..5 {
                    let d = (dx as isize - 2).abs().max((dy as isize - 2).abs());
                    g.set(cx - 2 + dx, cy - 2 + dy, d != 1);
                }
            }
        }
    }
    let lv = match level {
        Level::L => 1,
        Level::M => 0,
        Level::Q => 3,
        Level::H => 2,
    };
    let fw = format_word((lv << 3) | u32::from(mask_no));
    let mut first: Vec<(usize, usize)> = (0..=5).map(|i| (8, i)).collect();
    first.extend([(8, 7), (8, 8), (7, 8)]);
    first.extend((9..15).map(|i| (14 - i, 8)));
    let mut second: Vec<(usize, usize)> = (0..8).map(|i| (n - 1 - i, 8)).collect();
    second.extend((8..15).map(|i| (8, n - 15 + i)));
    for i in 0..15 {
        let b = fw >> i & 1 != 0;
        g.set(first[i].0, first[i].1, b);
        g.set(second[i].0, second[i].1, b);
    }
    g.set(8, n - 8, true);
    if version >= 7 {
        let vw = version_word(version as u32);
        for i in 0..18 {
            let b = vw >> i & 1 != 0;
            g.set(n - 11 + i % 3, i / 3, b);
            g.set(i / 3, n - 11 + i % 3, b);
        }
    }
    for (k, &(x, y)) in data_positions(version).iter().enumerate() {
        let bit = raw.get(k / 8).is_some_and(|b| b & (0x80 >> (k % 8)) != 0);
        g.set(x, y, bit ^ mask(mask_no, x, y));
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_version_words() {
        // ISO/IEC 18004 Annex C examples: M, mask 5 → 100000011001110
        assert_eq!(format_word(0b00101), 0b100_0000_1100_1110);
        // version 7 → 000111110010010100
        assert_eq!(version_word(7), 0b00_0111_1100_1001_0100);
        assert_eq!(
            decode_format(format_word(13) ^ 0b101, 0),
            Some((Level::L, 5))
        );
    }

    #[test]
    fn data_positions_cover_the_raw_modules() {
        for v in [1, 2, 6, 7, 14, 40] {
            assert_eq!(
                data_positions(v).len(),
                tables::raw_modules(v),
                "version {v}"
            );
        }
    }

    #[test]
    fn drawn_symbols_read_back_with_errors() {
        for (v, level) in [
            (1, Level::L),
            (5, Level::Q),
            (7, Level::M),
            (21, Level::H),
            (40, Level::L),
        ] {
            let n: usize = tables::blocks(v, level).iter().map(|b| b.data).sum();
            let data: Vec<u8> = (0..n).map(|i| (i * 31 + v) as u8).collect();
            for m in [0u8, 3, 7] {
                let mut g = draw(v, level, m, &data);
                // a few damaged modules
                for k in 0..6 {
                    let at = (k * 97 + 50) % g.dark.len();
                    g.dark[at] = !g.dark[at];
                }
                let c = read(&g).unwrap();
                assert_eq!((c.version, c.level, c.mask), (v, level, m));
                assert_eq!(c.data, data);
            }
        }
    }
}
