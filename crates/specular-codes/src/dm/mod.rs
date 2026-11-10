//! Data Matrix ECC 200 (ISO/IEC 16022): every square and rectangular size, Reed–Solomon over
//! GF(256) with x⁸+x⁵+x³+x²+1 (0x12D), all encodation modes. Older ECC 000–140 symbols are
//! not read.
//!
//! Finding a symbol: dark regions of the binary picture (the solid L border holds a symbol
//! together) → each region's convex hull → its smallest enclosing rectangle → the four sides
//! fitted to the hull's corners (so perspective is followed, and the corner the timing
//! pattern leaves open is found where the sides meet) → for each of the four turns, mirrored
//! or not, and each size, the finder and timing modules are sampled; the best match is
//! read, falling back to the next when its error correction fails.

pub mod content;
pub(crate) mod detect;
pub mod place;
pub mod tables;

use crate::image::{Binary, Gray, Runs};
use crate::rs::Field;
use tables::Size;

/// A symbol found and read.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub size: Size,
    pub content: content::Content,
    /// Top-left, top-right, bottom-right, bottom-left (as printed: the L at bottom left).
    pub corners: [(f32, f32); 4],
    pub mirrored: bool,
    pub corrected: usize,
}

/// Modules of a symbol, row by row (row 0 at the top), true for dark.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    pub dark: Vec<bool>,
}

impl Grid {
    pub fn get(&self, r: usize, c: usize) -> bool {
        self.dark[r * self.cols + c]
    }
}

/// Why a grid could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    Size,
    Correction,
    Content,
}

fn field() -> Field {
    Field::new(0x12D, 1)
}

/// The data codewords of a grid, corrected; and how many bytes were put right.
pub fn codewords(g: &Grid, size: &Size) -> Result<(Vec<u8>, usize), Fail> {
    if (g.rows, g.cols) != (size.rows, size.cols) {
        return Err(Fail::Size);
    }
    let (nr, nc) = size.mapping();
    let total = size.data + size.ecc;
    let mut cw = vec![0u8; total];
    for (k, m) in place::layout(nr, nc).iter().enumerate() {
        if let Some((w, bit)) = m {
            let (r, c) = size.module_of(k / nc, k % nc);
            if g.get(r, c) {
                cw[*w as usize] |= 0x80 >> bit;
            }
        }
    }
    // Interleaved blocks: codeword i of the data (and of the check bytes) is in block
    // i mod blocks.
    let nb = size.blocks;
    let ecc_per = size.ecc / nb;
    let f = field();
    let mut data = vec![0u8; size.data];
    let mut corrected = 0;
    for b in 0..nb {
        let mut block: Vec<u8> = (b..size.data).step_by(nb).map(|i| cw[i]).collect();
        let nd = block.len();
        block.extend((b..size.ecc).step_by(nb).map(|i| cw[size.data + i]));
        corrected += f.correct(&mut block, ecc_per).ok_or(Fail::Correction)?;
        for (k, i) in (b..size.data).step_by(nb).enumerate() {
            data[i] = block[k];
        }
        debug_assert_eq!(nd, (b..size.data).step_by(nb).count());
    }
    Ok((data, corrected))
}

/// Reads a grid of `size`.
pub fn read(g: &Grid, size: &Size) -> Result<(content::Content, usize), Fail> {
    let (data, corrected) = codewords(g, size)?;
    let c = content::parse(&data).map_err(|_| Fail::Content)?;
    Ok((c, corrected))
}

/// Every Data Matrix symbol in a binary picture (and its dark runs; `grey` the picture it was
/// binarised from), outside the quadrilaterals `skip` (corners in order, in pixels: codes
/// already read there); and whether anything looked like one.
pub(crate) fn find(
    b: &Binary,
    runs: &Runs,
    grey: &Gray,
    skip: &[[(f64, f64); 4]],
) -> (Vec<Symbol>, bool) {
    detect::find(b, runs, grey, skip)
}

/// A symbol of `size` holding `data` codewords (padded): for tests.
#[cfg(test)]
pub(crate) fn draw(size: &Size, data: &[u8]) -> Grid {
    let mut d = data.to_vec();
    if d.len() < size.data {
        d.push(129);
        while d.len() < size.data {
            let p = d.len() + 1;
            let r = ((149 * p) % 253 + 1) as u16;
            let v = 129 + r;
            d.push(if v <= 254 { v as u8 } else { (v - 254) as u8 });
        }
    }
    let nb = size.blocks;
    let ecc_per = size.ecc / nb;
    let f = field();
    let mut cw = d.clone();
    cw.resize(size.data + size.ecc, 0);
    for b in 0..nb {
        let block: Vec<u8> = (b..size.data).step_by(nb).map(|i| d[i]).collect();
        let e = f.encode(&block, ecc_per);
        for (k, i) in (b..size.ecc).step_by(nb).enumerate() {
            cw[size.data + i] = e[k];
        }
    }
    let mut g = Grid {
        rows: size.rows,
        cols: size.cols,
        dark: vec![false; size.rows * size.cols],
    };
    for r in 0..size.rows {
        for c in 0..size.cols {
            if let Some(v) = size.fixed(r, c) {
                g.dark[r * size.cols + c] = v;
            }
        }
    }
    let (nr, nc) = size.mapping();
    for (k, m) in place::layout(nr, nc).iter().enumerate() {
        let (mr, mc) = (k / nc, k % nc);
        let v = match m {
            Some((w, bit)) => cw[*w as usize] & (0x80 >> bit) != 0,
            None => place::spare_dark(nr, nc, mr, mc),
        };
        let (r, c) = size.module_of(mr, mc);
        g.dark[r * size.cols + c] = v;
    }
    g
}

/// ASCII codewords for `text` (digit pairs, upper shift): for tests.
#[cfg(test)]
pub(crate) fn ascii(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let b = text[i];
        if b.is_ascii_digit() && text.get(i + 1).is_some_and(u8::is_ascii_digit) {
            out.push(130 + (b - b'0') * 10 + (text[i + 1] - b'0'));
            i += 2;
            continue;
        }
        if b >= 128 {
            out.push(235);
            out.push(b - 128 + 1);
        } else {
            out.push(b + 1);
        }
        i += 1;
    }
    out
}

/// The smallest size holding `n` data codewords (square unless `rect`).
#[cfg(test)]
pub(crate) fn smallest(n: usize, rect: bool) -> Option<Size> {
    tables::SIZES
        .iter()
        .filter(|s| (s.rows != s.cols) == rect && s.data >= n)
        .min_by_key(|s| s.data)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_round_trip_every_size() {
        for s in tables::SIZES {
            let text: Vec<u8> = (0..s.data.saturating_sub(1))
                .map(|i| b'A' + (i % 26) as u8)
                .collect();
            let mut g = draw(&s, &ascii(&text));
            let (c, fixed) = read(&g, &s).unwrap();
            assert_eq!(c.bytes, text, "{s:?}");
            assert_eq!(fixed, 0);
            // Damage: flip a few modules inside the first data region.
            for k in 0..(s.ecc / s.blocks / 2).min(3) {
                let (r, c) = s.module_of(2 + k, 2);
                let i = r * s.cols + c;
                g.dark[i] = !g.dark[i];
            }
            let (c, _) = read(&g, &s).unwrap();
            assert_eq!(c.bytes, text, "{s:?} damaged");
        }
    }

    /// ISO/IEC 16022 annex O: "123456" in a 10×10 symbol is codewords 142 164 186 then
    /// check codewords 114 25 5 88 102.
    #[test]
    fn annex_example() {
        let d = ascii(b"123456");
        assert_eq!(d, vec![142, 164, 186]);
        let e = field().encode(&d, 5);
        assert_eq!(e, vec![114, 25, 5, 88, 102]);
    }
}
