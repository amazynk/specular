//! Aztec Code (ISO/IEC 24778): compact symbols of 1–4 layers, full-range symbols of 1–32
//! layers and runes; Reed–Solomon over GF(16) for the mode message and GF(64), GF(256),
//! GF(1024) or GF(4096) for the data, by layer count; every character mode (upper, lower,
//! mixed, punctuation, digit, binary shift), FLG(n) for FNC1 and ECI, and structured append.
//!
//! Finding a symbol: the bull's-eye (dark rings one module wide around a dark centre) shows
//! in a row of dark runs as 1:1:1:1:1:1:1 → confirmed down and across → its innermost
//! closed ring traced and its sides fitted (perspective) → the four turns, mirrored or not,
//! checked against the orientation marks → the mode message (layer count, data codewords)
//! → the whole grid, its transform refined with the symbol's outline and, for full-range
//! symbols, with the reference grid's crossings → bits in layer order → Reed–Solomon →
//! bit unstuffing → characters.

pub mod content;
mod detect;

use crate::image::Binary;
use crate::rs::Field;
use std::sync::OnceLock;

/// A symbol found and read.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub compact: bool,
    /// Data layers (0 for a rune).
    pub layers: usize,
    /// Modules a side.
    pub size: usize,
    /// A rune's value.
    pub rune: Option<u8>,
    pub content: content::Content,
    /// Top-left, top-right, bottom-right, bottom-left as printed.
    pub corners: [(f32, f32); 4],
    pub mirrored: bool,
    pub corrected: usize,
}

/// Modules of a symbol, row by row, true for dark.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub size: usize,
    pub dark: Vec<bool>,
}

impl Grid {
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.dark[y * self.size + x]
    }
}

/// Why a grid could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    Size,
    Correction,
    Content,
}

/// Bits a codeword has, by layer count.
pub(crate) fn word_bits(layers: usize) -> usize {
    match layers {
        0..=2 => 6,
        3..=8 => 8,
        9..=22 => 10,
        _ => 12,
    }
}

/// The Galois field of `bits`-bit codewords (4 for the mode message).
pub(crate) fn field(bits: usize) -> &'static Field {
    static F: [OnceLock<Field>; 5] = [const { OnceLock::new() }; 5];
    let (i, poly) = match bits {
        4 => (0, 0x13),
        6 => (1, 0x43),
        8 => (2, 0x12D),
        10 => (3, 0x409),
        _ => (4, 0x1069),
    };
    F[i].get_or_init(|| Field::new(poly, 1))
}

/// Modules a side.
pub(crate) fn size_of(compact: bool, layers: usize) -> usize {
    if compact {
        11 + 4 * layers
    } else {
        let base = 14 + 4 * layers;
        base + 1 + 2 * ((base / 2 - 1) / 15)
    }
}

/// Bits in the data layers.
pub(crate) fn total_bits(compact: bool, layers: usize) -> usize {
    (if compact { 88 } else { 112 } + 16 * layers) * layers
}

/// The most layers a symbol can have.
#[cfg(test)]
pub(crate) fn max_layers(compact: bool) -> usize {
    if compact { 4 } else { 32 }
}

/// Module positions (x, y) of the data layers without the reference grid, in placement
/// order: base coordinates to the symbol's.
fn alignment_map(compact: bool, layers: usize) -> Vec<usize> {
    if compact {
        return (0..size_of(true, layers)).collect();
    }
    let base = 14 + 4 * layers;
    let size = size_of(false, layers);
    let (oc, c) = (base / 2, size / 2);
    let mut map = vec![0; base];
    for i in 0..oc {
        let off = i + i / 15;
        map[oc - i - 1] = c - off - 1;
        map[oc + i] = c + off + 1;
    }
    map
}

/// The module (x, y) of every data bit, in bit order: each layer two modules deep, from the
/// outermost in, round its four sides (down the left, along the bottom, up the right,
/// back along the top), a pair of bits across the layer at a time.
pub(crate) fn data_positions(compact: bool, layers: usize) -> Vec<(usize, usize)> {
    let map = alignment_map(compact, layers);
    let base = map.len();
    let mut out = vec![(0, 0); total_bits(compact, layers)];
    let mut off = 0;
    for i in 0..layers {
        let row = (layers - i) * 4 + if compact { 9 } else { 12 };
        let (low, high) = (2 * i, base - 1 - 2 * i);
        for j in 0..row {
            for k in 0..2 {
                out[off + 2 * j + k] = (map[low + k], map[low + j]);
                out[off + 2 * row + 2 * j + k] = (map[low + j], map[high - k]);
                out[off + 4 * row + 2 * j + k] = (map[high - k], map[high - j]);
                out[off + 6 * row + 2 * j + k] = (map[high - j], map[low + k]);
            }
        }
        off += 8 * row;
    }
    out
}

/// The mode message's modules, relative to the centre module, in bit order: clockwise from
/// the top left, seven (compact) or ten (full-range, skipping the reference grid's middle
/// line) a side.
pub(crate) fn mode_positions(compact: bool) -> Vec<(i32, i32)> {
    let mut out = Vec::with_capacity(40);
    if compact {
        for side in 0..4 {
            for i in 0..7i32 {
                out.push(match side {
                    0 => (-3 + i, -5),
                    1 => (5, -3 + i),
                    2 => (3 - i, 5),
                    _ => (-5, 3 - i),
                });
            }
        }
    } else {
        for side in 0..4 {
            for i in 0..10i32 {
                let o = -5 + i + i / 5;
                out.push(match side {
                    0 => (o, -7),
                    1 => (7, o),
                    2 => (-o, 7),
                    _ => (-7, -o),
                });
            }
        }
    }
    out
}

/// The orientation marks, relative to the centre: (x, y, dark). The top-left corner has
/// three dark modules, the top-right two, the bottom-right one, the bottom-left none.
pub(crate) fn orientation_marks(compact: bool) -> [(i32, i32, bool); 12] {
    let r = if compact { 5 } else { 7 };
    [
        (-r, -r, true),
        (-r + 1, -r, true),
        (-r, -r + 1, true),
        (r, -r, true),
        (r - 1, -r, false),
        (r, -r + 1, true),
        (r, r, false),
        (r, r - 1, true),
        (r - 1, r, false),
        (-r, r, false),
        (-r + 1, r, false),
        (-r, r - 1, false),
    ]
}

fn nibbles(bits: &[bool]) -> Vec<u16> {
    bits.chunks(4)
        .map(|c| c.iter().fold(0u16, |a, &b| a << 1 | u16::from(b)))
        .collect()
}

/// The mode message's bits: (layers, data codewords, corrected nibbles).
pub(crate) fn decode_mode(bits: &[bool], compact: bool) -> Option<(usize, usize, usize)> {
    let mut w = nibbles(bits);
    let (data, ecc) = if compact { (2, 5) } else { (4, 6) };
    if w.len() != data + ecc {
        return None;
    }
    let fixed = field(4).correct_words(&mut w, ecc)?;
    let v = w[..data]
        .iter()
        .fold(0usize, |a, &n| a << 4 | usize::from(n));
    let (layers, words) = if compact {
        ((v >> 6) + 1, (v & 0x3F) + 1)
    } else {
        ((v >> 11) + 1, (v & 0x7FF) + 1)
    };
    // the data must fit
    let total = total_bits(compact, layers) / word_bits(layers);
    (words < total).then_some((layers, words, fixed))
}

/// A rune's value from a compact mode message (its bits inverted every other one).
pub(crate) fn decode_rune(bits: &[bool]) -> Option<(u8, usize)> {
    let flipped: Vec<bool> = bits
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ (i % 2 == 0))
        .collect();
    let mut w = nibbles(&flipped);
    if w.len() != 7 {
        return None;
    }
    let fixed = field(4).correct_words(&mut w, 5)?;
    Some(((w[0] << 4 | w[1]) as u8, fixed))
}

/// The data codewords of a grid of `layers` with `words` data codewords, corrected, as
/// a bit stream with the stuffed bits taken out; and how many codewords were put right.
pub(crate) fn data_bits(
    g: &Grid,
    compact: bool,
    layers: usize,
    words: usize,
) -> Result<(Vec<bool>, usize), Fail> {
    if g.size != size_of(compact, layers) {
        return Err(Fail::Size);
    }
    let raw: Vec<bool> = data_positions(compact, layers)
        .iter()
        .map(|&(x, y)| g.get(x, y))
        .collect();
    let wb = word_bits(layers);
    // The first (total mod word size) bits are padding.
    let start = raw.len() % wb;
    let mut cw: Vec<u16> = raw[start..]
        .chunks_exact(wb)
        .map(|c| c.iter().fold(0u16, |a, &b| a << 1 | u16::from(b)))
        .collect();
    if words >= cw.len() {
        return Err(Fail::Size);
    }
    let ecc = cw.len() - words;
    let fixed = field(wb)
        .correct_words(&mut cw, ecc)
        .ok_or(Fail::Correction)?;
    let all = (1u16 << wb) - 1;
    let mut bits = Vec::with_capacity(words * wb);
    for &w in &cw[..words] {
        // all zeros or all ones never occur; one less or one more is a stuffed word
        if w == 0 || w == all {
            return Err(Fail::Content);
        }
        let n = if w == 1 || w == all - 1 { wb - 1 } else { wb };
        for i in (wb - n..wb).rev() {
            bits.push(w >> i & 1 == 1);
        }
    }
    Ok((bits, fixed))
}

/// Reads a grid.
pub fn read(
    g: &Grid,
    compact: bool,
    layers: usize,
    words: usize,
) -> Result<(content::Content, usize), Fail> {
    let (bits, fixed) = data_bits(g, compact, layers, words)?;
    let c = content::parse(&bits).map_err(|_| Fail::Content)?;
    Ok((c, fixed))
}

/// Every Aztec symbol in a binary picture (and its dark runs), outside the quadrilaterals
/// `skip` (codes already read there).
pub(crate) fn find(b: &Binary, runs: &crate::image::Runs, skip: &[[(f64, f64); 4]]) -> Vec<Symbol> {
    detect::find(b, runs, skip)
}

/// Test symbols: an encoder for the grid side of the format.
#[cfg(test)]
pub(crate) mod draw {
    use super::*;

    /// Bits to codewords of `wb` bits, stuffing a bit where the first wb - 1 are all equal,
    /// padded with ones.
    pub fn stuff(bits: &[bool], wb: usize) -> Vec<u16> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < bits.len() {
            let mut w = 0u16;
            for k in 0..wb - 1 {
                w = w << 1 | u16::from(bits.get(i + k).copied().unwrap_or(true));
            }
            let lead = w;
            let mask = (1u16 << (wb - 1)) - 1;
            if lead == 0 {
                w = w << 1 | 1;
                i += wb - 1;
            } else if lead == mask {
                w <<= 1;
                i += wb - 1;
            } else {
                w = w << 1 | u16::from(bits.get(i + wb - 1).copied().unwrap_or(true));
                i += wb;
            }
            out.push(w);
        }
        out
    }

    fn put_word(out: &mut Vec<bool>, w: u16, n: usize) {
        for i in (0..n).rev() {
            out.push(w >> i & 1 == 1);
        }
    }

    /// The mode message bits for a symbol, or a rune's.
    pub fn mode_bits(compact: bool, layers: usize, words: usize, rune: Option<u8>) -> Vec<bool> {
        let (data, ecc) = if compact { (2, 5) } else { (4, 6) };
        let v = match rune {
            Some(r) => usize::from(r),
            None if compact => (layers - 1) << 6 | (words - 1),
            None => (layers - 1) << 11 | (words - 1),
        };
        let d: Vec<u16> = (0..data)
            .map(|i| (v >> (4 * (data - 1 - i)) & 0xF) as u16)
            .collect();
        let mut all = d.clone();
        all.extend(field(4).encode_words(&d, ecc));
        let mut bits = Vec::new();
        for w in all {
            put_word(&mut bits, w, 4);
        }
        if rune.is_some() {
            for (i, b) in bits.iter_mut().enumerate() {
                *b ^= i % 2 == 0;
            }
        }
        bits
    }

    /// A symbol of `layers` (0 with `rune`) holding the codewords `data`.
    pub fn grid(compact: bool, layers: usize, data: &[u16], rune: Option<u8>) -> Grid {
        let size = if rune.is_some() {
            11
        } else {
            size_of(compact, layers)
        };
        let c = (size / 2) as i32;
        let mut g = Grid {
            size,
            dark: vec![false; size * size],
        };
        let set = |g: &mut Grid, dx: i32, dy: i32, v: bool| {
            let (x, y) = ((c + dx) as usize, (c + dy) as usize);
            g.dark[y * size + x] = v;
        };
        if !compact {
            // reference grid
            for line in (-(c / 16) * 16..=c).step_by(16) {
                for t in -c..=c {
                    set(&mut g, line, t, t % 2 == 0);
                    set(&mut g, t, line, t % 2 == 0);
                }
            }
        }
        // bull's-eye and the mode ring cleared
        let core: i32 = if compact { 5 } else { 7 };
        for dy in -core..=core {
            for dx in -core..=core {
                let d = dx.abs().max(dy.abs());
                set(&mut g, dx, dy, d < core && d % 2 == 0);
            }
        }
        for (x, y, v) in orientation_marks(compact) {
            set(&mut g, x, y, v);
        }
        let words = data.len();
        let mode = mode_bits(compact, layers, words, rune);
        for (&(x, y), &v) in mode_positions(compact).iter().zip(&mode) {
            set(&mut g, x, y, v);
        }
        if rune.is_none() {
            let wb = word_bits(layers);
            let total = total_bits(compact, layers) / wb;
            let mut cw = data.to_vec();
            cw.extend(field(wb).encode_words(data, total - words));
            let mut bits = vec![false; total_bits(compact, layers) % wb];
            for w in cw {
                put_word(&mut bits, w, wb);
            }
            for (&(x, y), &v) in data_positions(compact, layers).iter().zip(&bits) {
                g.dark[y * size + x] = v;
            }
        }
        g
    }

    /// Upper-mode codes for capital letters and spaces, binary shift for anything else:
    /// enough to make test symbols.
    pub fn simple_bits(text: &[u8]) -> Vec<bool> {
        let mut bits = Vec::new();
        let mut i = 0;
        while i < text.len() {
            let ch = text[i];
            if ch.is_ascii_uppercase() || ch == b' ' {
                put_word(
                    &mut bits,
                    if ch == b' ' {
                        1
                    } else {
                        u16::from(ch - b'A') + 2
                    },
                    5,
                );
                i += 1;
                continue;
            }
            let n = text[i..]
                .iter()
                .take_while(|c| !(c.is_ascii_uppercase() || **c == b' '))
                .count()
                .min(31 + 2047);
            put_word(&mut bits, 31, 5);
            if n < 32 {
                put_word(&mut bits, n as u16, 5);
            } else {
                put_word(&mut bits, 0, 5);
                put_word(&mut bits, (n - 31) as u16, 11);
            }
            for &b in &text[i..i + n] {
                put_word(&mut bits, u16::from(b), 8);
            }
            i += n;
        }
        bits
    }

    /// The smallest symbol (compact first) for `bits` with about a third for checking, as
    /// (compact, layers, codewords).
    pub fn fit(bits: &[bool], compact_ok: bool) -> (bool, usize, Vec<u16>) {
        for compact in [true, false] {
            if compact && !compact_ok {
                continue;
            }
            for layers in 1..=max_layers(compact) {
                let wb = word_bits(layers);
                let words = stuff(bits, wb);
                let total = total_bits(compact, layers) / wb;
                if words.len() + total / 3 + 2 <= total {
                    return (compact, layers, words);
                }
            }
        }
        panic!("too long for a test symbol");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_capacities() {
        assert_eq!(size_of(true, 1), 15);
        assert_eq!(size_of(true, 4), 27);
        assert_eq!(size_of(false, 1), 19);
        assert_eq!(size_of(false, 4), 31);
        assert_eq!(size_of(false, 5), 37);
        assert_eq!(size_of(false, 32), 151);
        // full-range 32 layers: 1664 twelve-bit codewords
        assert_eq!(total_bits(false, 32) / word_bits(32), 1664);
        assert_eq!(total_bits(true, 1) / word_bits(1), 17);
        for compact in [true, false] {
            for layers in 1..=max_layers(compact) {
                let pos = data_positions(compact, layers);
                let mut seen = std::collections::HashSet::new();
                let size = size_of(compact, layers);
                let c = size / 2;
                let core: i32 = if compact { 5 } else { 7 };
                for &(x, y) in &pos {
                    assert!(seen.insert((x, y)), "{compact} {layers}: ({x}, {y}) twice");
                    assert!(x.abs_diff(c).max(y.abs_diff(c)) > core as usize);
                    if !compact {
                        assert!((x as i32 - c as i32) % 16 != 0 && (y as i32 - c as i32) % 16 != 0);
                    }
                }
            }
        }
    }

    #[test]
    fn grids_round_trip() {
        for (compact, layers) in [
            (true, 1),
            (true, 4),
            (false, 1),
            (false, 5),
            (false, 12),
            (false, 32),
        ] {
            let wb = word_bits(layers);
            let total = total_bits(compact, layers) / wb;
            let text: Vec<u8> = (0..(total * wb / 2 / 8).min(1500))
                .map(|i| b"AZTEC CODE 24778 "[i % 17])
                .collect();
            let bits = draw::simple_bits(&text);
            let words = draw::stuff(&bits, wb);
            assert!(words.len() < total, "{compact} {layers}");
            let mut g = draw::grid(compact, layers, &words, None);
            let c = (g.size / 2) as i32;
            let mode: Vec<bool> = mode_positions(compact)
                .iter()
                .map(|&(x, y)| g.get((c + x) as usize, (c + y) as usize))
                .collect();
            assert_eq!(decode_mode(&mode, compact), Some((layers, words.len(), 0)));
            let (got, fixed) = read(&g, compact, layers, words.len()).unwrap();
            assert_eq!(got.bytes, text, "{compact} {layers}");
            assert_eq!(fixed, 0);
            // damage some data modules
            let pos = data_positions(compact, layers);
            for k in 0..(total - words.len()) / 2 / 2 {
                let (x, y) = pos[(k * 97 + 13) % pos.len()];
                let i = y * g.size + x;
                g.dark[i] = !g.dark[i];
            }
            let (got, _) = read(&g, compact, layers, words.len()).unwrap();
            assert_eq!(got.bytes, text, "{compact} {layers} damaged");
        }
    }

    #[test]
    fn runes_round_trip() {
        for v in [0u8, 1, 42, 170, 255] {
            let bits = draw::mode_bits(true, 0, 0, Some(v));
            assert_eq!(decode_rune(&bits), Some((v, 0)));
            // a rune is no symbol's mode message, and the other way round
            assert_eq!(decode_mode(&bits, true), None);
            let m = draw::mode_bits(true, 2, 10, None);
            assert_eq!(decode_rune(&m), None);
        }
    }

    #[test]
    fn stuffing_round_trips() {
        let bits: Vec<bool> = [false; 13].into_iter().chain([true; 20]).collect();
        let w = draw::stuff(&bits, 6);
        assert!(w.iter().all(|&x| x != 0 && x != 63));
        assert_eq!(w[0], 1);
        assert_eq!(w[1], 1);
    }
}
