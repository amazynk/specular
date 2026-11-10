//! The setup header (spec 6.4): loop filter limits, quantisation matrices, token codebooks.

use crate::bits::Bits;
use crate::{Error, malformed};

/// Width in bits of one lookup level of a token codebook.
const LUT_BITS: u32 = 8;
/// Entry flag: the entry points to a further lookup level.
const SUB: u32 = 0x8000_0000;

pub(crate) struct Setup {
    pub(crate) lflims: [u8; 64],
    /// Quantisation matrices in natural order, indexed `(qti * 3 + pli) * 64 + qi`.
    pub(crate) qmat: Vec<[u16; 64]>,
    pub(crate) huff: Huffman,
}

/// The 80 DCT token codebooks as multi-level lookup tables.
pub(crate) struct Huffman {
    entries: Vec<u32>,
    roots: [(u32, u32); 80],
}

impl Huffman {
    /// Decodes one token with codebook `t` (0..80).
    #[inline(always)]
    pub(crate) fn token(&self, br: &mut Bits, t: usize) -> u32 {
        let (mut off, mut w) = self.roots[t];
        loop {
            let i = off as usize + (u64::from(br.peek32()) >> (32 - w)) as usize;
            let e = self.entries.get(i).copied().unwrap_or(0);
            if e & SUB == 0 {
                br.skip(e >> 8);
                return e & 31;
            }
            br.skip(w);
            w = (e >> 24) & 15;
            off = e & 0x00ff_ffff;
        }
    }
}

const LEAF: u32 = 0x8000_0000;

fn ilog(a: i32) -> u32 {
    if a <= 0 {
        0
    } else {
        32 - (a as u32).leading_zeros()
    }
}

pub(crate) fn parse(data: &[u8]) -> Result<Setup, Error> {
    if data.len() < 7 || data[0] != 0x82 || &data[1..7] != b"theora" {
        return Err(malformed("not a Theora setup header"));
    }
    let mut br = Bits::new(&data[7..]);
    // Loop filter limits (6.4.1).
    let mut lflims = [0u8; 64];
    let nbits = br.read(3);
    for v in lflims.iter_mut() {
        *v = br.read(nbits) as u8;
    }
    let qmat = parse_quant(&mut br)?;
    let huff = parse_huffman(&mut br)?;
    if br.overrun() {
        return Err(malformed("setup header is cut short"));
    }
    Ok(Setup { lflims, qmat, huff })
}

fn parse_quant(br: &mut Bits) -> Result<Vec<[u16; 64]>, Error> {
    let mut acscale = [0u32; 64];
    let mut dcscale = [0u32; 64];
    let nbits = br.read(4) + 1;
    for v in acscale.iter_mut() {
        *v = br.read(nbits);
    }
    let nbits = br.read(4) + 1;
    for v in dcscale.iter_mut() {
        *v = br.read(nbits);
    }
    let nbms = br.read(9) as usize + 1;
    if nbms > 384 {
        return Err(malformed("too many base matrices"));
    }
    let mut bms = vec![[0u8; 64]; nbms];
    for m in bms.iter_mut() {
        for v in m.iter_mut() {
            *v = br.read(8) as u8;
        }
    }
    // Quant ranges per (qti, pli): sizes and base matrix indices.
    let mut ranges: Vec<(Vec<u32>, Vec<usize>)> = Vec::with_capacity(6);
    let bmbits = ilog(nbms as i32 - 1);
    for qti in 0..2usize {
        for pli in 0..3usize {
            let newqr = if qti > 0 || pli > 0 { br.bit() } else { 1 };
            if newqr == 0 {
                let rpqr = if qti > 0 { br.bit() } else { 0 };
                let (qtj, plj) = if rpqr == 1 {
                    (qti - 1, pli)
                } else {
                    ((3 * qti + pli - 1) / 3, (pli + 2) % 3)
                };
                let r = ranges[qtj * 3 + plj].clone();
                ranges.push(r);
            } else {
                let mut sizes = Vec::new();
                let mut bmis = Vec::new();
                let mut qi = 0i32;
                let b = br.read(bmbits) as usize;
                if b >= nbms {
                    return Err(malformed("bad base matrix index"));
                }
                bmis.push(b);
                loop {
                    let size = br.read(ilog(62 - qi)) + 1;
                    qi += size as i32;
                    sizes.push(size);
                    let b = br.read(bmbits) as usize;
                    if b >= nbms {
                        return Err(malformed("bad base matrix index"));
                    }
                    bmis.push(b);
                    if qi < 63 {
                        continue;
                    }
                    if qi > 63 {
                        return Err(malformed("bad quant ranges"));
                    }
                    break;
                }
                ranges.push((sizes, bmis));
            }
        }
    }
    // Matrices for every (qti, pli, qi) (6.4.3).
    let mut out = vec![[0u16; 64]; 6 * 64];
    for qti in 0..2usize {
        for pli in 0..3usize {
            let (sizes, bmis) = &ranges[qti * 3 + pli];
            for qi in 0..64u32 {
                let mut start = 0u32;
                let mut qri = 0;
                while qri + 1 < sizes.len() && qi > start + sizes[qri] {
                    start += sizes[qri];
                    qri += 1;
                }
                let size = sizes[qri];
                let end = start + size;
                let (bmi, bmj) = (&bms[bmis[qri]], &bms[bmis[qri + 1]]);
                let m = &mut out[(qti * 3 + pli) * 64 + qi as usize];
                for ci in 0..64 {
                    let bm = (2 * (end - qi) * u32::from(bmi[ci])
                        + 2 * (qi - start) * u32::from(bmj[ci])
                        + size)
                        / (2 * size);
                    let qmin = match (qti, ci) {
                        (0, 0) => 16,
                        (0, _) => 8,
                        (_, 0) => 32,
                        _ => 16,
                    };
                    let scale = if ci == 0 {
                        dcscale[qi as usize]
                    } else {
                        acscale[qi as usize]
                    };
                    m[ci] = ((scale * bm / 100) * 4).clamp(qmin, 4096) as u16;
                }
            }
        }
    }
    Ok(out)
}

fn parse_huffman(br: &mut Bits) -> Result<Huffman, Error> {
    let mut entries = Vec::new();
    let mut roots = [(0u32, 0u32); 80];
    for root in roots.iter_mut() {
        let mut nodes: Vec<[u32; 2]> = Vec::new();
        let mut count = 0;
        let top = parse_tree(br, &mut nodes, 0, &mut count)?;
        *root = build_lut(&nodes, top, &mut entries);
    }
    Ok(Huffman { entries, roots })
}

/// Reads one (sub)tree; returns `LEAF | token` or an index into `nodes`.
fn parse_tree(
    br: &mut Bits,
    nodes: &mut Vec<[u32; 2]>,
    depth: u32,
    count: &mut u32,
) -> Result<u32, Error> {
    if depth > 32 {
        return Err(malformed("token codebook too deep"));
    }
    if br.bit() == 1 {
        if *count == 32 {
            return Err(malformed("token codebook too large"));
        }
        *count += 1;
        return Ok(LEAF | br.read(5));
    }
    let i = nodes.len();
    nodes.push([0, 0]);
    let a = parse_tree(br, nodes, depth + 1, count)?;
    let b = parse_tree(br, nodes, depth + 1, count)?;
    nodes[i] = [a, b];
    Ok(i as u32)
}

fn height(nodes: &[[u32; 2]], n: u32) -> u32 {
    if n & LEAF != 0 {
        return 0;
    }
    let [a, b] = nodes[n as usize];
    1 + height(nodes, a).max(height(nodes, b))
}

/// Builds the lookup level for node `n`; returns its (offset, width).
fn build_lut(nodes: &[[u32; 2]], n: u32, out: &mut Vec<u32>) -> (u32, u32) {
    let w = height(nodes, n).min(LUT_BITS);
    let off = out.len();
    out.resize(off + (1 << w), 0);
    for pat in 0..(1u32 << w) {
        let mut m = n;
        let mut d = 0;
        while d < w && m & LEAF == 0 {
            let bit = (pat >> (w - 1 - d)) & 1;
            m = nodes[m as usize][bit as usize];
            d += 1;
        }
        out[off + pat as usize] = if m & LEAF != 0 {
            (m & 31) | (d << 8)
        } else {
            let (so, sw) = build_lut(nodes, m, out);
            SUB | (sw << 24) | so
        };
    }
    (off as u32, w)
}
