//! Palettes that change from row to row: Sliced HAM (`SHAM`), Dynamic HiRes / Dynamic Mode
//! (`CTBL`) and Palette Change (`PCHG`).
//!
//! `SHAM` holds a version word, then 16 colours (`0x0RGB` words) for every row; `CTBL` the
//! same without the version word. A table with fewer rows than the picture (an interlaced
//! picture sliced every second row) applies each row's colours to the rows it spans.
//!
//! `PCHG` lists changes to palette registers, row by row from `StartLine` (which may be
//! negative): a header, a bit mask of the rows that change (one bit a row, 32-bit words, most
//! significant bit first), then per changed row either 12-bit changes to registers 0-31
//! (counts for 0-15 and 16-31, each change a word `RRRR rrrr gggg bbbb`: register nibble
//! and colour) or 32-bit changes to any register (count word, then register word and alpha,
//! red, blue, green bytes). Changes last until changed again.
//!
//! Huffman-compressed `PCHG` (compression 1), from the PCHG specification (Vigna and others,
//! 1994, "Compression"): after the header, the compressed size of the tree and the unpacked
//! size (two longwords), the tree, then the packed bits (longwords, most significant bit
//! first). The tree is an array of words read from its end: on a 0 bit step one word left
//! and stop at a word with bit 8 set (its low byte is the value); on a 1 bit take the word
//! there, a value when not negative, else a byte offset to the right subtree.

use crate::be16;

/// Which line palette a picture has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinePalette {
    Sham,
    Ctbl,
    Pchg,
}

impl LinePalette {
    pub fn name(self) -> &'static str {
        match self {
            LinePalette::Sham => "Sliced HAM",
            LinePalette::Ctbl => "Dynamic HiRes",
            LinePalette::Pchg => "palette changes",
        }
    }
}

/// A 4-bit `0x0RGB` colour to 8-bit RGB.
fn rgb4(w: u16) -> [u8; 3] {
    let n = |s: u16| ((w >> s) & 15) as u8 * 17;
    [n(8), n(4), n(0)]
}

/// The per-row changes: for row `y`, (register, colour) pairs applied before it is drawn.
pub(crate) struct Lines {
    /// Changes by row; changes before row 0 (`PCHG` starting above the picture) at index 0.
    rows: Vec<Vec<(u16, [u8; 3])>>,
}

impl Lines {
    /// Parses a line palette chunk for a picture `height` rows high; `None` when it holds
    /// nothing usable.
    pub(crate) fn parse(kind: LinePalette, d: &[u8], height: usize) -> Option<Lines> {
        if height == 0 {
            return None;
        }
        let mut rows: Vec<Vec<(u16, [u8; 3])>> = vec![Vec::new(); height];
        match kind {
            LinePalette::Sham | LinePalette::Ctbl => {
                let d = if kind == LinePalette::Sham {
                    d.get(2..)?
                } else {
                    d
                };
                let count = d.len() / 32;
                if count == 0 {
                    return None;
                }
                for (y, row) in rows.iter_mut().enumerate() {
                    // One table row per picture row, or each spanning several.
                    let t = if count >= height {
                        y
                    } else {
                        y * count / height
                    };
                    let at = t * 32;
                    for r in 0..16u16 {
                        row.push((r, rgb4(be16(d, at + 2 * usize::from(r)))));
                    }
                }
            }
            LinePalette::Pchg => {
                if d.len() < 20 || be16(d, 0) > 1 {
                    return None;
                }
                let unpacked;
                let d = if be16(d, 0) == 1 {
                    unpacked = unhuff_chunk(d)?;
                    &unpacked[..]
                } else {
                    d
                };
                let flags = be16(d, 2);
                let start = be16(d, 4) as i16 as i64;
                let count = usize::from(be16(d, 6));
                let mask_words = count.div_ceil(32);
                let mut p = 20 + mask_words * 4;
                let mask = d.get(20..p)?;
                for i in 0..count {
                    if mask[i / 8] & (0x80 >> (i % 8)) == 0 {
                        continue;
                    }
                    let y = start + i as i64;
                    if y >= height as i64 {
                        break;
                    }
                    let row = &mut rows[y.max(0) as usize];
                    if flags & 1 != 0 {
                        let (n16, n32) = (*d.get(p)?, *d.get(p + 1)?);
                        p += 2;
                        for k in 0..usize::from(n16) + usize::from(n32) {
                            if p + 2 > d.len() {
                                return Some(Lines { rows });
                            }
                            let w = be16(d, p);
                            p += 2;
                            let base = if k < usize::from(n16) { 0 } else { 16 };
                            row.push((base + (w >> 12), rgb4(w & 0xfff)));
                        }
                    } else if flags & 2 != 0 {
                        let n = usize::from(u16::from_be_bytes([*d.get(p)?, *d.get(p + 1)?]));
                        p += 2;
                        for _ in 0..n {
                            let c = d.get(p..p + 6)?;
                            p += 6;
                            let reg = u16::from_be_bytes([c[0], c[1]]);
                            // Alpha, red, blue, green.
                            row.push((reg, [c[3], c[5], c[4]]));
                        }
                    } else {
                        return None;
                    }
                }
            }
        }
        Some(Lines { rows })
    }

    /// Applies row `y`'s changes to `pal`; whether anything changed.
    pub(crate) fn apply(&self, y: usize, pal: &mut Vec<[u8; 3]>) -> bool {
        let Some(row) = self.rows.get(y) else {
            return false;
        };
        for &(r, c) in row {
            let r = usize::from(r);
            if r >= 256 {
                continue;
            }
            if pal.len() <= r {
                pal.resize(r + 1, [0; 3]);
            }
            pal[r] = c;
        }
        !row.is_empty()
    }
}

/// The header of a Huffman-compressed `PCHG` chunk followed by its unpacked data.
fn unhuff_chunk(d: &[u8]) -> Option<Vec<u8>> {
    let long = |p: usize| Some(u32::from_be_bytes(d.get(p..p + 4)?.try_into().ok()?) as usize);
    let (tree_len, n) = (long(20)?, long(24)?);
    // The unpacked data is a row mask and at most 65535 rows of changes.
    if n > 64 << 20 || !(2..=1022).contains(&tree_len) {
        return None;
    }
    let tree = d.get(28..28 + tree_len)?;
    let packed = d.get(28 + tree_len..)?;
    let mut out = Vec::with_capacity(20 + n);
    out.extend_from_slice(&d[..20]);
    out[0..2].copy_from_slice(&0u16.to_be_bytes());
    unhuff(tree, packed, n, &mut out)?;
    Some(out)
}

/// Decodes `n` bytes of `packed` with the PCHG tree `tree` (big-endian words), appending
/// them to `out`; `None` when the bits or the tree run out or the tree is malformed.
pub(crate) fn unhuff(tree: &[u8], packed: &[u8], n: usize, out: &mut Vec<u8>) -> Option<()> {
    let words = tree.len() / 2;
    let word = |i: usize| i16::from_be_bytes([tree[2 * i], tree[2 * i + 1]]);
    let root = words.checked_sub(1)?;
    let mut bit = 0usize;
    let total = packed.len() * 8;
    for _ in 0..n {
        let mut pos = root;
        // A path is at most one step per tree word.
        let mut steps = 0;
        loop {
            if bit >= total || steps > words {
                return None;
            }
            let b = packed[bit / 8] >> (7 - bit % 8) & 1;
            bit += 1;
            steps += 1;
            if b == 0 {
                pos = pos.checked_sub(1)?;
                let w = word(pos);
                if w >= 0 && w & 0x100 != 0 {
                    out.push(w as u8);
                    break;
                }
            } else {
                let w = word(pos);
                if w >= 0 {
                    out.push(w as u8);
                    break;
                }
                // A byte offset back to the right subtree's root.
                pos = pos.checked_sub(usize::from(w.unsigned_abs()) / 2)?;
            }
        }
    }
    Some(())
}

#[cfg(test)]
pub(crate) mod huff_enc {
    //! A PCHG Huffman encoder for tests, written from the same specification text: any
    //! prefix tree over the bytes used, the tree words as the specification lays them out.

    enum Node {
        Leaf(u8),
        Inner(Box<Node>, Box<Node>),
    }

    /// `code(node)`: the node's words, its own word last (right subtree, then left subtree,
    /// then the node's word: the right child's value or the byte offset to it).
    fn code(n: &Node) -> Vec<i16> {
        match n {
            Node::Leaf(c) => vec![*c as i16 | 0x100],
            Node::Inner(l, r) => {
                let cl = code(l);
                let (mut v, w) = match &**r {
                    Node::Leaf(c) => (Vec::new(), *c as i16),
                    r => (code(r), -((cl.len() as i16 + 1) * 2)),
                };
                v.extend(cl);
                v.push(w);
                v
            }
        }
    }

    fn paths(n: &Node, prefix: &mut Vec<bool>, out: &mut [Vec<bool>; 256]) {
        match n {
            Node::Leaf(c) => out[*c as usize] = prefix.clone(),
            Node::Inner(l, r) => {
                prefix.push(false);
                paths(l, prefix, out);
                prefix.pop();
                prefix.push(true);
                paths(r, prefix, out);
                prefix.pop();
            }
        }
    }

    /// The tree bytes and the packed bits (padded to whole longwords) of `data`.
    pub(crate) fn encode(data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut freq = [0u64; 256];
        for &b in data {
            freq[b as usize] += 1;
        }
        let mut nodes: Vec<(u64, Node)> = (0..256)
            .filter(|&c| freq[c] > 0)
            .map(|c| (freq[c], Node::Leaf(c as u8)))
            .collect();
        // A lone symbol still needs a two-leaf tree.
        if nodes.len() == 1 {
            let other = if data[0] == 0 { 1 } else { 0 };
            nodes.push((0, Node::Leaf(other)));
        }
        while nodes.len() > 1 {
            nodes.sort_by_key(|n| std::cmp::Reverse(n.0));
            let (fa, a) = nodes.pop().unwrap();
            let (fb, b) = nodes.pop().unwrap();
            nodes.push((fa + fb, Node::Inner(Box::new(a), Box::new(b))));
        }
        let root = nodes.pop().unwrap().1;
        let tree: Vec<u8> = code(&root).iter().flat_map(|w| w.to_be_bytes()).collect();
        let mut table: [Vec<bool>; 256] = std::array::from_fn(|_| Vec::new());
        paths(&root, &mut Vec::new(), &mut table);
        let mut bits = Vec::new();
        let (mut acc, mut k) = (0u8, 0);
        for &b in data {
            for &bit in &table[b as usize] {
                acc = acc << 1 | bit as u8;
                k += 1;
                if k == 8 {
                    bits.push(acc);
                    (acc, k) = (0, 0);
                }
            }
        }
        if k > 0 {
            bits.push(acc << (8 - k));
        }
        while bits.len() % 4 != 0 {
            bits.push(0);
        }
        (tree, bits)
    }

    /// `chunk` (an uncompressed PCHG chunk) Huffman-compressed.
    pub(crate) fn compress_chunk(chunk: &[u8]) -> Vec<u8> {
        let (tree, bits) = encode(&chunk[20..]);
        let mut out = chunk[..20].to_vec();
        out[0..2].copy_from_slice(&1u16.to_be_bytes());
        out.extend((tree.len() as u32).to_be_bytes());
        out.extend(((chunk.len() - 20) as u32).to_be_bytes());
        out.extend(tree);
        out.extend(bits);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example_trees() {
        // [a|0x100][b]: 0 -> a, 1 -> b.
        let t: Vec<u8> = [0x161i16, 0x62]
            .iter()
            .flat_map(|w| w.to_be_bytes())
            .collect();
        let mut out = Vec::new();
        unhuff(&t, &[0b0110_0000], 3, &mut out).unwrap();
        assert_eq!(out, b"abb");
        // [d|0x100][e][c|0x100][-4][a|0x100][b][-6]: a 00, b 01, c 10, d 110, e 111.
        let w = [0x164i16, 0x65, 0x163, -4, 0x161, 0x62, -6];
        let t: Vec<u8> = w.iter().flat_map(|w| w.to_be_bytes()).collect();
        let mut out = Vec::new();
        unhuff(&t, &[0b0001_1011, 0b0111_0000], 5, &mut out).unwrap();
        assert_eq!(out, b"abcde");
        // Bits running out, and a tree whose offsets leave it, are refused.
        assert!(unhuff(&t, &[0xFF], 5, &mut Vec::new()).is_none());
        let bad: Vec<u8> = [0x161i16, -40]
            .iter()
            .flat_map(|w| w.to_be_bytes())
            .collect();
        assert!(unhuff(&bad, &[0xFF, 0xFF], 1, &mut Vec::new()).is_none());
    }

    #[test]
    fn huffman_round_trips() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i * i % 251 % 37) as u8).collect();
        let (tree, bits) = huff_enc::encode(&data);
        assert!(tree.len() <= 1022);
        let mut out = Vec::new();
        unhuff(&tree, &bits, data.len(), &mut out).unwrap();
        assert_eq!(out, data);
        for one in [[0u8; 9], [7u8; 9]] {
            let (tree, bits) = huff_enc::encode(&one);
            let mut out = Vec::new();
            unhuff(&tree, &bits, 9, &mut out).unwrap();
            assert_eq!(out, one);
        }
    }

    #[test]
    fn sham_rows_and_interlace() {
        let mut d = vec![0, 0];
        for t in 0..2u16 {
            for r in 0..16u16 {
                d.extend_from_slice(&((t * 0x800) | r).to_be_bytes());
            }
        }
        let l = Lines::parse(LinePalette::Sham, &d, 4).unwrap();
        let mut pal = Vec::new();
        l.apply(1, &mut pal);
        assert_eq!(pal[3], [0, 0, 51]);
        l.apply(2, &mut pal);
        assert_eq!(pal[3], [136, 0, 51]);
    }

    #[test]
    fn pchg_small_and_big() {
        // Rows -1 and 1 of a 3-row picture change: registers 2 and 17 (12-bit).
        let mut d = vec![0u8; 20];
        d[2..4].copy_from_slice(&1u16.to_be_bytes());
        d[4..6].copy_from_slice(&(-1i16).to_be_bytes());
        d[6..8].copy_from_slice(&3u16.to_be_bytes());
        d.extend_from_slice(&[0b1010_0000, 0, 0, 0]);
        d.extend_from_slice(&[1, 0, 0x2f, 0x00]);
        d.extend_from_slice(&[0, 1, 0x10, 0x0f]);
        let l = Lines::parse(LinePalette::Pchg, &d, 3).unwrap();
        let mut pal = vec![[1, 1, 1]; 4];
        assert!(l.apply(0, &mut pal));
        assert_eq!(pal[2], [255, 0, 0]);
        assert!(l.apply(1, &mut pal));
        assert_eq!(pal[17], [0, 0, 255]);
        assert!(!l.apply(2, &mut pal));
        // 32-bit: register 5 to (red 10, green 30, blue 20).
        let mut d = vec![0u8; 20];
        d[2..4].copy_from_slice(&2u16.to_be_bytes());
        d[6..8].copy_from_slice(&1u16.to_be_bytes());
        d.extend_from_slice(&[0x80, 0, 0, 0, 0, 1, 0, 5, 0, 10, 20, 30]);
        let l = Lines::parse(LinePalette::Pchg, &d, 2).unwrap();
        let mut pal = Vec::new();
        l.apply(0, &mut pal);
        assert_eq!(pal[5], [10, 30, 20]);
    }
}
