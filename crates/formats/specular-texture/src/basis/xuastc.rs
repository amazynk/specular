//! XUASTC LDR (Basis Universal v2): supercompressed ASTC LDR, transcoded to ASTC LDR blocks of
//! the same footprint. Written from Binomial's published "XUASTC LDR Specification v1.0" and
//! its "Arithmetic Profile", "Range Coding", "Weight Grid DCT" and "Endpoint Conversion"
//! companions (basis_universal wiki). All three profiles: full arithmetic (0), hybrid
//! arithmetic + Zstandard (1) and full Zstandard (2).
//!
//! Blocks come in raster order as runs, solid colours, reuses of a neighbour's configuration
//! and endpoints, or new blocks whose configuration ("trial mode") indexes a table of 10,311
//! ASTC configurations; endpoints are coded directly or predicted from a nearby block through
//! a cross-CEM conversion; weight grids are DPCM or DCT coded.

// Index loops follow the specification's pseudo-code.
#![allow(clippy::needless_range_loop)]

use std::io::Read;
use std::sync::OnceLock;

use super::astc_pack::{LogBlock, cem_values, endpoint_quant, levels, pack, weight_quant};
use super::hdr6x6i::{Bits, decode_values};
use super::xuastc_arith::{Arith, BitModel, DataModel, Gamma};
use super::xuastc_dct::{dc_levels, decode_plane};
use super::xuastc_ep::{convert, decode_endpoints, supports_bc, used_bc};
use super::xuastc_tables::{PART2, PART3};
use crate::{Error, Result};

const BLOCK_SIZES: [(u8, u8); 14] = [
    (4, 4),
    (5, 4),
    (5, 5),
    (6, 5),
    (6, 6),
    (8, 5),
    (8, 6),
    (10, 5),
    (10, 6),
    (8, 8),
    (10, 8),
    (10, 10),
    (12, 10),
    (12, 12),
];
const CFG: &[u8] = include_bytes!("xuastc_cfg.bin");
const UNIQUE_CEMS: [u8; 6] = [0, 4, 6, 8, 10, 12];
/// Reuse "delta" codes (shared with UASTC HDR 6×6i).
const REUSE: [(i8, i8); 32] = [
    (-1, 0),
    (-2, 0),
    (-3, 0),
    (-4, 0),
    (3, -1),
    (2, -1),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-2, -1),
    (-3, -1),
    (-4, -1),
    (3, -2),
    (2, -2),
    (1, -2),
    (0, -2),
    (-1, -2),
    (-2, -2),
    (-3, -2),
    (-4, -2),
    (3, -3),
    (2, -3),
    (1, -3),
    (0, -3),
    (-1, -3),
    (-2, -3),
    (-3, -3),
    (-4, -3),
    (3, -4),
    (2, -4),
    (1, -4),
    (0, -4),
];
const PART_HASH: usize = 64;
const TM_HASH: usize = 128;
const GROUPS: usize = 14 * 3 * 5 * 2 * 3;
const FINAL_SYNC: u32 = 0xaf;
const EOB: u32 = 64;

fn bad(what: &'static str) -> Error {
    Error::Malformed(what)
}

/// A configuration ("trial mode").
#[derive(Clone, Copy, Debug)]
struct Tm {
    gw: u8,
    gh: u8,
    cem: u8,
    /// Dual-plane component, or −1.
    ccs: i8,
    er: u8,
    wr: u8,
    parts: u8,
}

struct Modes {
    tms: Vec<Tm>,
    /// Trial modes per (CEM, subsets − 1, CCS + 1, large grid, anisotropy) bucket.
    groups: Vec<Vec<u32>>,
}

fn group(cem: usize, subset: usize, ccs: usize, size: usize, aniso: usize) -> usize {
    (((cem * 3 + subset) * 5 + ccs) * 2 + size) * 3 + aniso
}

fn modes(size: usize) -> &'static Modes {
    static T: OnceLock<Vec<Modes>> = OnceLock::new();
    &T.get_or_init(|| {
        BLOCK_SIZES
            .iter()
            .map(|&(bw, bh)| {
                let mut m = Modes {
                    tms: Vec::new(),
                    groups: vec![Vec::new(); GROUPS],
                };
                for e in CFG.chunks_exact(3) {
                    let v = u32::from(e[0]) | u32::from(e[1]) << 8 | u32::from(e[2]) << 16;
                    let grid = v >> 17;
                    let (gw, gh) = ((grid / 11 + 2) as u8, (grid % 11 + 2) as u8);
                    let cem = (v >> 14) & 7;
                    if gw > bw || gh > bh || cem >= 6 {
                        continue;
                    }
                    let ccs1 = (v >> 9) & 7;
                    let tm = Tm {
                        gw,
                        gh,
                        cem: UNIQUE_CEMS[cem as usize],
                        ccs: ccs1 as i8 - 1,
                        er: ((v & 31) + 4) as u8,
                        wr: ((v >> 5) & 15) as u8,
                        parts: ((v >> 12) & 3) as u8 + 1,
                    };
                    if tm.er > 20 || tm.wr > 11 || tm.parts > 3 || ccs1 > 4 {
                        continue;
                    }
                    let large = gw + 1 >= bw && gh + 1 >= bh;
                    let (l, r) = (u32::from(gw) * u32::from(bh), u32::from(gh) * u32::from(bw));
                    let aniso = if l == r {
                        0
                    } else if l >= r {
                        1
                    } else {
                        2
                    };
                    let g = group(
                        usize::from(tm.cem),
                        usize::from(tm.parts - 1),
                        ccs1 as usize,
                        usize::from(large),
                        aniso,
                    );
                    m.groups[g].push(m.tms.len() as u32);
                    m.tms.push(tm);
                }
                m
            })
            .collect::<Vec<_>>()
    })[size]
}

fn seeds(size: usize, parts: u8) -> &'static [u16] {
    if parts == 2 { PART2[size] } else { PART3[size] }
}

/// Per-block prediction state (`prev_block_state`).
#[derive(Clone, Copy, Default)]
struct State {
    solid: bool,
    dct: bool,
    first_bc: bool,
    reused_full: bool,
    part_hash: bool,
    tm: i32,
    base_cem: u8,
    subset: u8,
    ccs: u8,
    grid_size: u8,
    aniso: u8,
}

struct Header {
    size: usize,
    alpha: bool,
    dct: bool,
    q: f32,
}

/// Blocks across, down, and the rolling rows of logical blocks (8) and states (2).
struct Grid {
    bx: usize,
    bw: u8,
    bh: u8,
    log: Vec<LogBlock>,
    st: Vec<State>,
    out: Vec<u8>,
}

impl Grid {
    fn new(bx: usize, by: usize, bw: u8, bh: u8) -> Self {
        Grid {
            bx,
            bw,
            bh,
            log: vec![LogBlock::default(); bx * 8],
            st: vec![State::default(); bx * 2],
            out: vec![0; bx * by * 16],
        }
    }

    fn log(&self, x: usize, y: usize) -> &LogBlock {
        &self.log[(y & 7) * self.bx + x]
    }

    fn state(&self, x: usize, y: usize) -> State {
        self.st[(y & 1) * self.bx + x]
    }

    fn emit(&mut self, x: usize, y: usize, b: LogBlock, s: State) -> Result<()> {
        let bytes = pack(&b).ok_or(bad("XUASTC LDR block"))?;
        self.out[(y * self.bx + x) * 16..][..16].copy_from_slice(&bytes);
        self.log[(y & 7) * self.bx + x] = b;
        self.st[(y & 1) * self.bx + x] = s;
        Ok(())
    }

    /// The previous block (left, else above).
    fn prev(&self, x: usize, y: usize) -> Option<(LogBlock, State)> {
        if x > 0 {
            Some((*self.log(x - 1, y), self.state(x - 1, y)))
        } else if y > 0 {
            Some((*self.log(x, y - 1), self.state(x, y - 1)))
        } else {
            None
        }
    }

    /// Neighbour 0 left, 1 up, 2 up-left.
    fn neighbour(&self, x: usize, y: usize, k: u32) -> Option<(LogBlock, State)> {
        let (nx, ny) = match k {
            0 => (x.checked_sub(1)?, y),
            1 => (x, y.checked_sub(1)?),
            2 => (x.checked_sub(1)?, y.checked_sub(1)?),
            _ => return None,
        };
        Some((*self.log(nx, ny), self.state(nx, ny)))
    }

    /// The endpoint predictor of reuse code `k`: in the image, not solid.
    fn predictor(&self, x: usize, y: usize, k: u32) -> Result<LogBlock> {
        let &(dx, dy) = REUSE
            .get(k as usize)
            .ok_or(bad("XUASTC LDR endpoint reuse"))?;
        let (px, py) = (x as isize + isize::from(dx), y as isize + isize::from(dy));
        if px < 0 || py < 0 || px >= self.bx as isize {
            return Err(bad("XUASTC LDR endpoint reuse"));
        }
        let p = *self.log(px as usize, py as usize);
        if p.solid {
            return Err(bad("XUASTC LDR endpoint reuse of a solid block"));
        }
        Ok(p)
    }
}

/// The predicted solid colour: the previous block's, or its first endpoint pair's midpoint.
fn solid_prediction(p: &LogBlock) -> [i32; 4] {
    if p.solid {
        p.solid_colour.map(|c| i32::from(c >> 8))
    } else {
        let (l, h) = decode_endpoints(p.cem, &p.endpoints, p.erange);
        [0, 1, 2, 3].map(|c| (l[c] + h[c] + 1) >> 1)
    }
}

fn solid_block(pred: [i32; 4], d: [u32; 4], alpha: bool) -> LogBlock {
    let mut b = LogBlock {
        solid: true,
        ..LogBlock::default()
    };
    for c in 0..4 {
        let v = if c == 3 && !alpha {
            255
        } else {
            ((pred[c] + d[c] as i32) & 0xff) as u16
        };
        b.solid_colour[c] = v | (v << 8);
    }
    b
}

fn fib(x: u32, size: usize) -> usize {
    (x.wrapping_mul(2_654_435_769) as usize) & (size - 1)
}

fn apply_tm(b: &mut LogBlock, tm: &Tm, cem: u8) {
    b.parts = tm.parts;
    b.cem = cem;
    b.dual = tm.ccs >= 0;
    b.ccs = if tm.ccs >= 0 { tm.ccs as u8 } else { 0 };
    b.wrange = tm.wr;
    b.erange = tm.er;
    b.grid_w = tm.gw;
    b.grid_h = tm.gh;
}

/// Endpoints from residual ranks added to predicted ISE values.
fn apply_residuals(
    b: &mut LogBlock,
    pred: &[[u8; 8]; 3],
    mut residual: impl FnMut(usize) -> Result<u32>,
) -> Result<()> {
    let n = cem_values(b.cem);
    let q = &endpoint_quant()[usize::from(b.erange)];
    let lv = levels(b.erange) as u32;
    for p in 0..usize::from(b.parts) {
        for i in 0..n {
            let d = residual(lv as usize)?;
            let r = (d + u32::from(q.ise_to_rank[usize::from(pred[p][i])])) % lv;
            b.endpoints[p * n + i] = q.rank_to_ise[r as usize];
        }
    }
    Ok(())
}

/// Predicted endpoints of every partition from predictor `p`.
fn predict(b: &LogBlock, p: &LogBlock, bc: &[bool; 3]) -> Result<[[u8; 8]; 3]> {
    let mut out = [[0u8; 8]; 3];
    for (k, o) in out.iter_mut().enumerate().take(usize::from(b.parts)) {
        if !convert(p.cem, p.erange, &p.endpoints, b.cem, b.erange, o, bc[k]) {
            return Err(bad("XUASTC LDR endpoint prediction"));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Zstandard / raw sections
// ---------------------------------------------------------------------------------------------

/// A section of packed values (1, 2, 4 or 8 bits, LSB first, never crossing bytes).
#[derive(Default)]
struct Section {
    d: Vec<u8>,
    bit: usize,
}

impl Section {
    fn get(&mut self, n: u32) -> Result<u32> {
        let byte = self.bit >> 3;
        let &v = self
            .d
            .get(byte)
            .ok_or(Error::Truncated("XUASTC LDR section"))?;
        let v = (u32::from(v) >> (self.bit & 7)) & ((1 << n) - 1);
        self.bit += n as usize;
        Ok(v)
    }
}

/// Split `d` into sections of the given lengths, inflating the Zstandard ones (at most
/// `limit` bytes each).
fn sections(d: &[u8], lens: &[(usize, bool)], limit: usize) -> Result<Vec<Section>> {
    let mut at = 0usize;
    let mut out = Vec::with_capacity(lens.len());
    for &(len, zstd) in lens {
        let s = at
            .checked_add(len)
            .and_then(|e| d.get(at..e))
            .ok_or(Error::Truncated("XUASTC LDR sections"))?;
        at += len;
        let data = if len == 0 || !zstd {
            s.to_vec()
        } else {
            let mut v = Vec::new();
            ruzstd::decoding::StreamingDecoder::new(s)
                .map_err(|_| bad("XUASTC LDR Zstandard section"))?
                .take(limit as u64)
                .read_to_end(&mut v)
                .map_err(|_| bad("XUASTC LDR Zstandard section"))?;
            v
        };
        out.push(Section { d: data, bit: 0 });
    }
    Ok(out)
}

/// The weight sections of the hybrid and Zstandard profiles.
struct WeightSections {
    mean0: Section,
    mean1: Section,
    run: Section,
    coeff: Section,
    sign: Section,
    w2: Section,
    w3: Section,
    w4: Section,
    w8: Section,
}

impl WeightSections {
    fn from(mut s: Vec<Section>) -> Self {
        let mut take = || s.remove(0);
        WeightSections {
            mean0: take(),
            mean1: take(),
            run: take(),
            coeff: take(),
            sign: take(),
            w2: take(),
            w3: take(),
            w4: take(),
            w8: take(),
        }
    }

    fn decode(&mut self, b: &mut LogBlock, dct: bool, h: &Header, g: &Grid) -> Result<()> {
        let planes = if b.dual { 2 } else { 1 };
        let total = usize::from(b.grid_w) * usize::from(b.grid_h);
        if dct {
            let dcl = dc_levels(b.wrange);
            for plane in 0..planes {
                let dc = if dcl == 33 {
                    self.mean1.get(8)?
                } else {
                    self.mean0.get(4)?
                };
                let mut coeffs = Vec::new();
                let mut z = 1usize;
                while z < total {
                    let run = self.run.get(8)?;
                    if run == EOB {
                        break;
                    }
                    z += run as usize;
                    if z >= total {
                        return Err(bad("XUASTC LDR DCT run"));
                    }
                    let neg = self.sign.get(1)? == 1;
                    let m = self.coeff.get(8)? as i32 + 1;
                    coeffs.push((run, if neg { -m } else { m }));
                    z += 1;
                }
                if !decode_plane(b, plane, h.q, g.bw, g.bh, dc, &coeffs) {
                    return Err(bad("XUASTC LDR DCT block"));
                }
            }
        } else {
            let lv = levels(b.wrange) as u32;
            let r2i = &weight_quant()[usize::from(b.wrange)].rank_to_ise;
            for plane in 0..planes {
                let mut prev = lv / 2;
                for i in 0..total {
                    let r = if lv <= 4 {
                        self.w2.get(2)?
                    } else if lv <= 8 {
                        self.w3.get(4)?
                    } else if lv <= 16 {
                        self.w4.get(4)?
                    } else {
                        self.w8.get(8)?
                    };
                    prev = (prev + r) % lv;
                    b.weights[plane + i * planes] = r2i[prev as usize];
                }
            }
        }
        Ok(())
    }
}

fn read_header(
    b: &mut impl FnMut(u32) -> Result<u32>,
    bw: u8,
    bh: u8,
    w: u32,
    h: u32,
) -> Result<Header> {
    let version = b(5)?;
    if version != 1 {
        return Err(Error::Unsupported(format!(
            "XUASTC LDR stream version {version}"
        )));
    }
    let size = b(4)? as usize;
    let _srgb = b(1)?;
    let (aw, ah) = (b(16)?, b(16)?);
    let alpha = b(1)? == 1;
    let dct = b(1)? == 1;
    let q = if dct { b(8)? as f32 / 2.0 } else { 0.0 };
    if BLOCK_SIZES.get(size) != Some(&(bw, bh)) {
        return Err(bad("XUASTC LDR block size"));
    }
    if (aw, ah) != (w, h) || aw == 0 || ah == 0 {
        return Err(bad("XUASTC LDR image size"));
    }
    if dct && !(1.0..=100.0).contains(&q) {
        return Err(bad("XUASTC LDR DCT quality"));
    }
    Ok(Header {
        size,
        alpha,
        dct,
        q,
    })
}

fn u32_le(d: &[u8], at: usize) -> usize {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]) as usize
}

/// Transcode one image (`w`×`h` texels, ASTC footprint `bw`×`bh`) to ASTC LDR blocks.
pub(crate) fn transcode(d: &[u8], w: u32, h: u32, bw: u8, bh: u8) -> Result<Vec<u8>> {
    if !BLOCK_SIZES.contains(&(bw, bh)) {
        return Err(bad("XUASTC LDR block size"));
    }
    let (bx, by) = (
        w.div_ceil(u32::from(bw)) as usize,
        h.div_ceil(u32::from(bh)) as usize,
    );
    // Generous per-section cap for the inflated streams.
    let limit = bx * by * 160 + 4096;
    match d.first() {
        Some(0) => arith(&d[1..], None, w, h, bw, bh),
        Some(1) => {
            if d.len() < 45 {
                return Err(Error::Truncated("XUASTC LDR hybrid header"));
            }
            let alen = u32_le(d, 1);
            let lens: Vec<(usize, bool)> = (0..9).map(|i| (u32_le(d, 5 + 4 * i), i != 4)).collect();
            let a = d
                .get(45..45usize.saturating_add(alen))
                .ok_or(Error::Truncated("XUASTC LDR arithmetic section"))?;
            let ws = WeightSections::from(sections(&d[45 + alen..], &lens, limit)?);
            arith(a, Some(ws), w, h, bw, bh)
        }
        Some(2) => zstd(d, w, h, bw, bh, limit),
        Some(&s) => Err(Error::Unsupported(format!("XUASTC LDR profile {s}"))),
        None => Err(Error::Truncated("XUASTC LDR stream")),
    }
}

// ---------------------------------------------------------------------------------------------
// Full Zstandard profile
// ---------------------------------------------------------------------------------------------

fn zstd(d: &[u8], w: u32, h: u32, bw: u8, bh: u8, limit: usize) -> Result<Vec<u8>> {
    if d.len() < 85 {
        return Err(Error::Truncated("XUASTC LDR Zstandard header"));
    }
    // raw_bits, mode, solid, reuse indices, BC bits, DPCM 3…8 bits, mean0, mean1, run, coeff,
    // sign (raw), weight 2/3/4/8.
    let lens: Vec<(usize, bool)> = (0..20)
        .map(|i| (u32_le(d, 1 + 4 * i), i != 0 && i != 15))
        .collect();
    if lens[0].0 == 0 || lens[1].0 == 0 {
        return Err(bad("XUASTC LDR Zstandard sections"));
    }
    let mut s = sections(&d[85..], &lens, limit)?;
    let ws = WeightSections::from(s.split_off(11));
    let mut it = s.into_iter();
    let raw = it.next().unwrap_or_default();
    let mut mode = it.next().unwrap_or_default();
    let mut solid = it.next().unwrap_or_default();
    let mut reuse = it.next().unwrap_or_default();
    let mut usebc = it.next().unwrap_or_default();
    let mut dpcm: Vec<Section> = it.collect();
    let mut ws = ws;
    let mut rb = Bits::new(&raw.d);
    let hd = read_header(&mut |n| rb.get(n), bw, bh, w, h)?;
    let m = modes(hd.size);
    let ntm = m.tms.len() as u32;
    let (bx, by) = (
        w.div_ceil(u32::from(bw)) as usize,
        h.div_ceil(u32::from(bh)) as usize,
    );
    let mut g = Grid::new(bx, by, bw, bh);
    let mut tm_hash = [-1i32; TM_HASH];
    let mut p2_hash = [-1i32; PART_HASH];
    let mut p3_hash = [-1i32; PART_HASH];
    for y in 0..by {
        let mut run = 0usize;
        for x in 0..bx {
            if run > 0 {
                let (b, s) = g.prev(x, y).ok_or(bad("XUASTC LDR run"))?;
                g.emit(x, y, b, s)?;
                run -= 1;
                continue;
            }
            let mb = mode.get(8)?;
            let mut st = State::default();
            if mb & 3 == 1 {
                let n = 1 + (mb as usize >> 2);
                if n > bx - x {
                    return Err(bad("XUASTC LDR run length"));
                }
                let (b, s) = g
                    .prev(x, y)
                    .ok_or(bad("XUASTC LDR run at the first block"))?;
                g.emit(x, y, b, s)?;
                run = n - 1;
                continue;
            }
            if mb & 15 == 3 {
                let (p, _) = g
                    .prev(x, y)
                    .ok_or(bad("XUASTC LDR solid at the first block"))?;
                let pred = solid_prediction(&p);
                let mut dl = [0u32; 4];
                for (c, v) in dl.iter_mut().enumerate() {
                    if c < 3 || hd.alpha {
                        *v = solid.get(8)?;
                    }
                }
                st.tm = -1;
                st.solid = true;
                g.emit(x, y, solid_block(pred, dl, hd.alpha), st)?;
                continue;
            }
            let mut b = LogBlock::default();
            if mb & 1 == 0 {
                let cfg = (mb >> 1) & 3;
                let tm_index;
                if cfg < 3 {
                    let (nb, ns) = g
                        .neighbour(x, y, cfg)
                        .ok_or(bad("XUASTC LDR configuration reuse"))?;
                    if ns.tm < 0 || nb.solid {
                        return Err(bad("XUASTC LDR configuration reuse"));
                    }
                    tm_index = ns.tm as u32;
                    b = nb;
                    b.endpoints = [0; 18];
                } else {
                    tm_index = if mb & 0x40 != 0 {
                        let t = tm_hash[rb.get(7)? as usize];
                        if t < 0 {
                            return Err(bad("XUASTC LDR configuration hash"));
                        }
                        t as u32
                    } else {
                        rb.truncated_binary(ntm)?
                    };
                    let tm = *m
                        .tms
                        .get(tm_index as usize)
                        .ok_or(bad("XUASTC LDR configuration"))?;
                    tm_hash[fib(tm_index, TM_HASH)] = tm_index as i32;
                    let mut cem = tm.cem;
                    if mb & 8 != 0 && matches!(cem, 8 | 12) {
                        cem += 1;
                    }
                    apply_tm(&mut b, &tm, cem);
                    if tm.parts > 1 {
                        let table = seeds(hd.size, tm.parts);
                        let hash = if tm.parts == 2 {
                            &mut p2_hash
                        } else {
                            &mut p3_hash
                        };
                        let idx = if mb & 0x10 != 0 {
                            let i = hash[rb.get(6)? as usize];
                            if i < 0 {
                                return Err(bad("XUASTC LDR partition hash"));
                            }
                            i as u32
                        } else {
                            rb.truncated_binary(table.len() as u32)?
                        };
                        let &seed = table.get(idx as usize).ok_or(bad("XUASTC LDR partition"))?;
                        hash[fib(idx, PART_HASH)] = idx as i32;
                        b.seed = seed;
                    }
                }
                st.tm = tm_index as i32;
                let n = cem_values(b.cem) * usize::from(b.parts);
                if mb & 0x20 != 0 {
                    let p = g.predictor(x, y, reuse.get(8)?)?;
                    let mut bc = [false; 3];
                    if supports_bc(b.cem) {
                        for v in bc.iter_mut().take(usize::from(b.parts)) {
                            *v = usebc.get(1)? == 1;
                        }
                    }
                    let pred = predict(&b, &p, &bc)?;
                    apply_residuals(&mut b, &pred, |lv| {
                        let (k, bits) = match lv {
                            0..=8 => (0, 4),
                            9..=16 => (1, 4),
                            17..=32 => (2, 8),
                            33..=64 => (3, 8),
                            65..=128 => (4, 8),
                            _ => (5, 8),
                        };
                        dpcm[k].get(bits)
                    })?;
                } else {
                    decode_values(&mut rb, &mut b.endpoints[..n], b.erange)?;
                }
            } else {
                let k = ((mb >> 2) & 3).wrapping_sub(1);
                let (nb, ns) = g.neighbour(x, y, k).ok_or(bad("XUASTC LDR reuse"))?;
                if ns.tm < 0 || nb.solid {
                    return Err(bad("XUASTC LDR reuse of a solid block"));
                }
                st.tm = ns.tm;
                b = nb;
            }
            ws.decode(&mut b, hd.dct && mb & 0x80 != 0, &hd, &g)?;
            g.emit(x, y, b, st)?;
        }
    }
    Ok(g.out)
}

// ---------------------------------------------------------------------------------------------
// Arithmetic profiles
// ---------------------------------------------------------------------------------------------

struct Models {
    mode: DataModel,
    solid: [DataModel; 4],
    raw_ep: Vec<DataModel>,
    dpcm_ep: Vec<DataModel>,
    is_base_ofs: BitModel,
    use_dct: [BitModel; 4],
    use_dpcm: BitModel,
    cem: Vec<DataModel>,
    subset: Vec<DataModel>,
    ccs: Vec<DataModel>,
    grid_size: Vec<DataModel>,
    aniso: Vec<DataModel>,
    run_len: DataModel,
    coeff: DataModel,
    mean: [DataModel; 2],
    raw_w: Vec<DataModel>,
    submode: Vec<DataModel>,
    use_bc: [BitModel; 4],
    reuse_delta: DataModel,
    cfg_reuse: Vec<DataModel>,
    runs: Gamma,
    use_part_hash: [BitModel; 4],
    p2_hash: DataModel,
    p3_hash: DataModel,
}

impl Models {
    fn new(full: bool) -> Self {
        let dm = |n: u32| DataModel::new(n, false);
        let ep: Vec<DataModel> = (4..21u8).map(|r| dm(levels(r) as u32)).collect();
        Models {
            mode: dm(6),
            solid: [0; 4].map(|_| DataModel::new(256, true)),
            raw_ep: ep.clone(),
            dpcm_ep: ep,
            is_base_ofs: BitModel::default(),
            use_dct: [BitModel::default(); 4],
            use_dpcm: BitModel::default(),
            cem: (0..8).map(|_| dm(14)).collect(),
            subset: (0..3).map(|_| dm(3)).collect(),
            ccs: (0..5).map(|_| dm(5)).collect(),
            grid_size: (0..2).map(|_| dm(2)).collect(),
            aniso: (0..3).map(|_| dm(3)).collect(),
            run_len: if full { dm(65) } else { DataModel::default() },
            coeff: if full { dm(255) } else { DataModel::default() },
            mean: if full {
                [dm(9), dm(33)]
            } else {
                [DataModel::default(), DataModel::default()]
            },
            raw_w: if full {
                (0..12u8).map(|r| dm(levels(r) as u32)).collect()
            } else {
                Vec::new()
            },
            submode: vec![DataModel::default(); GROUPS],
            use_bc: [BitModel::default(); 4],
            reuse_delta: dm(32),
            cfg_reuse: (0..4).map(|_| dm(4)).collect(),
            runs: Gamma::default(),
            use_part_hash: [BitModel::default(); 4],
            p2_hash: DataModel::new(PART_HASH as u32, true),
            p3_hash: DataModel::new(PART_HASH as u32, true),
        }
    }
}

fn ldrcem_index(cem: u8) -> usize {
    match cem {
        0 => 0,
        4 => 1,
        6 => 2,
        8 => 3,
        9 => 4,
        10 => 5,
        12 => 6,
        13 => 7,
        _ => 0,
    }
}

/// Context index from the left and upper states' flag (absent neighbours count as set).
fn ctx2(l: Option<State>, u: Option<State>, f: impl Fn(&State) -> bool) -> usize {
    let a = l.as_ref().is_none_or(&f);
    let b = u.as_ref().is_none_or(&f);
    usize::from(a) | usize::from(b) << 1
}

fn arith(
    d: &[u8],
    mut ws: Option<WeightSections>,
    w: u32,
    h: u32,
    bw: u8,
    bh: u8,
) -> Result<Vec<u8>> {
    let mut ar = Arith::new(d)?;
    let hd = read_header(&mut |n| Ok(ar.get_bits(n)), bw, bh, w, h)?;
    let tms = modes(hd.size);
    let (bx, by) = (
        w.div_ceil(u32::from(bw)) as usize,
        h.div_ceil(u32::from(bh)) as usize,
    );
    let mut g = Grid::new(bx, by, bw, bh);
    let mut md = Models::new(ws.is_none());
    let mut p2_hash = [-1i32; PART_HASH];
    let mut p3_hash = [-1i32; PART_HASH];
    let mut run = 0u32;
    for y in 0..by {
        for x in 0..bx {
            let left = (x > 0).then(|| g.state(x - 1, y));
            let up = (y > 0).then(|| g.state(x, y - 1));
            let diag = (x > 0 && y > 0).then(|| g.state(x - 1, y - 1));
            let pred = left.or(up);
            if run > 0 {
                let (b, s) = g.prev(x, y).ok_or(bad("XUASTC LDR run"))?;
                let s = run_state(s);
                g.emit(x, y, b, s)?;
                run -= 1;
                continue;
            }
            // Stop early on a stream long past its end (zeros decode forever).
            if ar.overrun() > 64 {
                return Err(Error::Truncated("XUASTC LDR arithmetic data"));
            }
            let mode = ar.sym(&mut md.mode);
            let mut st = State::default();
            match mode {
                0 => {
                    let pb = if x > 0 {
                        Some(*g.log(x - 1, y))
                    } else if y > 0 {
                        Some(*g.log(x, y - 1))
                    } else {
                        None
                    };
                    let p = pb.map_or([0; 4], |p| solid_prediction(&p));
                    let mut dl = [0u32; 4];
                    for (c, v) in dl.iter_mut().enumerate() {
                        if c < 3 || hd.alpha {
                            *v = ar.sym(&mut md.solid[c]);
                        }
                    }
                    st.dct = hd.dct;
                    st.first_bc = true;
                    st.solid = true;
                    st.tm = -1;
                    st.base_cem = 8;
                    st.part_hash = true;
                    g.emit(x, y, solid_block(p, dl, hd.alpha), st)?;
                }
                5 => {
                    if x == 0 && y == 0 {
                        return Err(bad("XUASTC LDR run at the first block"));
                    }
                    let n = ar.gamma(&mut md.runs);
                    if n == 0 || n as usize > bx - x {
                        return Err(bad("XUASTC LDR run length"));
                    }
                    let (b, s) = g.prev(x, y).ok_or(bad("XUASTC LDR run"))?;
                    let s = run_state(s);
                    g.emit(x, y, b, s)?;
                    run = n - 1;
                }
                1..=4 => {
                    let mut b = LogBlock::default();
                    let tm_index;
                    let mut cem;
                    if mode != 1 {
                        let k = mode - 2;
                        let (cb, cs) = g.neighbour(x, y, k).ok_or(bad("XUASTC LDR reuse"))?;
                        let cs_some = [left, up, diag][k as usize].is_some();
                        if !cs_some || cs.tm < 0 {
                            return Err(bad("XUASTC LDR reuse of a solid block"));
                        }
                        b = cb;
                        b.weights = [0; 64];
                        st = State {
                            tm: cs.tm,
                            base_cem: cs.base_cem,
                            subset: cs.subset,
                            ccs: cs.ccs,
                            grid_size: cs.grid_size,
                            aniso: cs.aniso,
                            part_hash: cs.part_hash,
                            reused_full: true,
                            ..State::default()
                        };
                        if supports_bc(b.cem) {
                            st.first_bc = used_bc(b.cem, &b.endpoints, b.erange);
                        }
                        let tm = tms.tms[cs.tm as usize];
                        arith_weights(
                            &mut ar,
                            &mut md,
                            ws.as_mut(),
                            &mut b,
                            &mut st,
                            &tm,
                            &hd,
                            &g,
                            left,
                            up,
                        )?;
                        g.emit(x, y, b, st)?;
                        continue;
                    }
                    let ci = ctx2(left, up, |s| s.reused_full);
                    let reuse = ar.sym(&mut md.cfg_reuse[ci]);
                    if reuse < 3 {
                        let (cb, cs) = g
                            .neighbour(x, y, reuse)
                            .ok_or(bad("XUASTC LDR configuration reuse"))?;
                        if [left, up, diag][reuse as usize].is_none() || cs.tm < 0 {
                            return Err(bad("XUASTC LDR configuration reuse"));
                        }
                        tm_index = cs.tm as u32;
                        b.seed = cb.seed;
                        cem = cb.cem;
                        st = State {
                            tm: cs.tm,
                            base_cem: cs.base_cem,
                            subset: cs.subset,
                            ccs: cs.ccs,
                            grid_size: cs.grid_size,
                            aniso: cs.aniso,
                            part_hash: cs.part_hash,
                            reused_full: true,
                            ..State::default()
                        };
                    } else {
                        let p = pred.unwrap_or(State {
                            base_cem: 8,
                            ..State::default()
                        });
                        let ce = ar.sym(&mut md.cem[ldrcem_index(p.base_cem)]) as usize;
                        let su = ar.sym(&mut md.subset[usize::from(p.subset).min(2)]) as usize;
                        let cc = ar.sym(&mut md.ccs[usize::from(p.ccs).min(4)]) as usize;
                        let gs =
                            ar.sym(&mut md.grid_size[usize::from(p.grid_size).min(1)]) as usize;
                        let an = ar.sym(&mut md.aniso[usize::from(p.aniso).min(2)]) as usize;
                        let gi = group(ce, su, cc, gs, an);
                        let cand = &tms.groups[gi];
                        let mut sub = 0usize;
                        if cand.len() > 1 {
                            if md.submode[gi].symbols() == 0 {
                                md.submode[gi] = DataModel::new(cand.len() as u32, true);
                            }
                            sub = ar.sym(&mut md.submode[gi]) as usize;
                        }
                        let &t = cand.get(sub).ok_or(bad("XUASTC LDR configuration"))?;
                        tm_index = t;
                        st.tm = t as i32;
                        st.base_cem = ce as u8;
                        st.subset = su as u8;
                        st.ccs = cc as u8;
                        st.grid_size = gs as u8;
                        st.aniso = an as u8;
                        st.reused_full = false;
                        let tm = tms.tms[t as usize];
                        cem = tm.cem;
                        if matches!(cem, 8 | 12) && ar.bit(&mut md.is_base_ofs) {
                            cem += 1;
                        }
                        if tm.parts > 1 {
                            let table = seeds(hd.size, tm.parts);
                            let pi = ctx2(left, up, |s| s.part_hash);
                            let hash = if tm.parts == 2 {
                                &mut p2_hash
                            } else {
                                &mut p3_hash
                            };
                            let idx = if !ar.bit(&mut md.use_part_hash[pi]) {
                                let i = ar.truncated_binary(table.len() as u32);
                                hash[fib(i, PART_HASH)] = i as i32;
                                st.part_hash = false;
                                i
                            } else {
                                let hm = if tm.parts == 2 {
                                    &mut md.p2_hash
                                } else {
                                    &mut md.p3_hash
                                };
                                let i = hash[ar.sym(hm) as usize];
                                if i < 0 {
                                    return Err(bad("XUASTC LDR partition hash"));
                                }
                                st.part_hash = true;
                                i as u32
                            };
                            b.seed = *table.get(idx as usize).ok_or(bad("XUASTC LDR partition"))?;
                        } else {
                            st.part_hash = true;
                        }
                    }
                    let tm = *tms
                        .tms
                        .get(tm_index as usize)
                        .ok_or(bad("XUASTC LDR configuration"))?;
                    apply_tm(&mut b, &tm, cem);
                    // Endpoints.
                    let n = cem_values(cem);
                    let er = usize::from(b.erange) - 4;
                    if !ar.bit(&mut md.use_dpcm) {
                        for v in &mut b.endpoints[..n * usize::from(b.parts)] {
                            *v = ar.sym(&mut md.raw_ep[er]) as u8;
                        }
                    } else {
                        let p = g.predictor(x, y, ar.sym(&mut md.reuse_delta))?;
                        let bi = ctx2(left, up, |s| s.first_bc);
                        let mut bc = [false; 3];
                        if supports_bc(cem) {
                            for v in bc.iter_mut().take(usize::from(b.parts)) {
                                *v = ar.bit(&mut md.use_bc[bi]);
                            }
                        }
                        let pr = predict(&b, &p, &bc)?;
                        let m = &mut md.dpcm_ep[er];
                        apply_residuals(&mut b, &pr, |_| Ok(ar.sym(m)))?;
                    }
                    if supports_bc(cem) {
                        st.first_bc = used_bc(cem, &b.endpoints, b.erange);
                    }
                    arith_weights(
                        &mut ar,
                        &mut md,
                        ws.as_mut(),
                        &mut b,
                        &mut st,
                        &tm,
                        &hd,
                        &g,
                        left,
                        up,
                    )?;
                    g.emit(x, y, b, st)?;
                }
                _ => return Err(bad("XUASTC LDR block mode")),
            }
        }
        if run != 0 {
            return Err(bad("XUASTC LDR run across rows"));
        }
    }
    if ar.get_bits(8) != FINAL_SYNC {
        return Err(bad("XUASTC LDR end marker"));
    }
    Ok(g.out)
}

#[allow(clippy::too_many_arguments)]
fn arith_weights(
    ar: &mut Arith,
    md: &mut Models,
    ws: Option<&mut WeightSections>,
    b: &mut LogBlock,
    st: &mut State,
    tm: &Tm,
    hd: &Header,
    g: &Grid,
    left: Option<State>,
    up: Option<State>,
) -> Result<()> {
    let dct = if hd.dct {
        let i = ctx2(left, up, |s| s.dct);
        ar.bit(&mut md.use_dct[i])
    } else {
        false
    };
    if dct {
        st.dct = true;
    }
    if let Some(ws) = ws {
        return ws.decode(b, dct, hd, g);
    }
    let planes = if tm.ccs >= 0 { 2 } else { 1 };
    let total = usize::from(tm.gw) * usize::from(tm.gh);
    if dct {
        let mi = usize::from(dc_levels(b.wrange) == 33);
        for plane in 0..planes {
            let dc = ar.sym(&mut md.mean[mi]);
            let mut coeffs = Vec::new();
            let mut z = 1usize;
            while z < total {
                let run = ar.sym(&mut md.run_len);
                if run == EOB {
                    break;
                }
                z += run as usize;
                if z >= total {
                    return Err(bad("XUASTC LDR DCT run"));
                }
                let neg = ar.get_bit() == 1;
                let m = ar.sym(&mut md.coeff) as i32 + 1;
                coeffs.push((run, if neg { -m } else { m }));
                z += 1;
            }
            if !decode_plane(b, plane, hd.q, g.bw, g.bh, dc, &coeffs) {
                return Err(bad("XUASTC LDR DCT block"));
            }
        }
    } else {
        let lv = levels(b.wrange) as u32;
        let r2i = &weight_quant()[usize::from(b.wrange)].rank_to_ise;
        let m = &mut md.raw_w[usize::from(b.wrange)];
        for plane in 0..planes {
            let mut prev = lv / 2;
            for i in 0..total {
                prev = (prev + ar.sym(m)) % lv;
                b.weights[plane + i * planes] = r2i[prev as usize];
            }
        }
    }
    Ok(())
}

/// The state a run copy leaves: the copied block's, counted as a full reuse (not in the
/// published pseudo-code; found against the reference transcoder, without it every block
/// after a run of one desynchronises).
fn run_state(s: State) -> State {
    State {
        reused_full: true,
        ..s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blocks that differ, and texels whose RGBA8 differ, between two arrays of ASTC LDR
    /// `bw`×`bh` blocks.
    pub(crate) fn compare_ldr(
        ours: &[u8],
        want: &[u8],
        bw: usize,
        bh: usize,
        srgb: bool,
    ) -> (usize, usize) {
        assert_eq!(ours.len(), want.len());
        let (mut blocks, mut texels) = (0, 0);
        let (mut p, mut q) = (vec![0u8; bw * bh * 4], vec![0u8; bw * bh * 4]);
        for (x, y) in ours.chunks_exact(16).zip(want.chunks_exact(16)) {
            if x == y {
                continue;
            }
            blocks += 1;
            crate::astc::decode_ldr(x, bw, bh, 1, 0, srgb, &mut p, bw * 4);
            crate::astc::decode_ldr(y, bw, bh, 1, 0, srgb, &mut q, bw * 4);
            texels += p
                .chunks_exact(4)
                .zip(q.chunks_exact(4))
                .filter(|(a, c)| a != c)
                .count();
        }
        (blocks, texels)
    }

    /// Level 3 (128×128, full arithmetic profile) of basisu v2.50's `t6x6.ktx2` (Apache-2.0)
    /// and the opengl_basisu wheel's ASTC 6×6 blocks for it (tmp script in the lane report:
    /// `KTX2File.transcode(3, 0, 0, ASTC_LDR_6X6_RGBA)`): every block equal.
    #[test]
    fn fixture_blocks_match_the_transcoder() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
        let s = std::fs::read(format!("{dir}xuastc6x6_L3.bin")).unwrap();
        let want = std::fs::read(format!("{dir}xuastc6x6_L3.blocks")).unwrap();
        assert_eq!(transcode(&s, 128, 128, 6, 6).unwrap(), want);
        // Wrong size or footprint is refused.
        assert!(transcode(&s, 127, 128, 6, 6).is_err());
        assert!(transcode(&s, 128, 128, 6, 5).is_err());
    }

    /// Damaged streams fail (or decode to something) without panicking or hanging.
    #[test]
    fn damaged_streams_fail_cleanly() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
        let s = std::fs::read(format!("{dir}xuastc6x6_L3.bin")).unwrap();
        for n in (0..s.len()).step_by(37) {
            assert!(transcode(&s[..n], 128, 128, 6, 6).is_err(), "cut at {n}");
        }
        for i in (1..s.len()).step_by(29) {
            for x in [0x01u8, 0x80, 0xff] {
                let mut d = s.clone();
                d[i] ^= x;
                let _ = transcode(&d, 128, 128, 6, 6);
            }
        }
        for p in [1u8, 2, 3] {
            let mut d = vec![p];
            d.extend_from_slice(&[0xff; 120]);
            let _ = transcode(&d, 64, 64, 6, 6);
        }
    }

    /// The fetched corpus (tools/corpus/fetch-textures.sh src_basisu2 oracle_basisu2): every
    /// XUASTC LDR file's top level against the opengl_basisu wheel's ASTC blocks. Skips when
    /// absent.
    #[test]
    fn corpus_blocks_match_the_transcoder() {
        let Some(base) = std::env::var_os("SPECULAR_LARGE_CORPUS") else {
            return;
        };
        let dir = std::path::Path::new(&base).join("texture/basisu2");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut names: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        names.sort();
        let mut bad_files = Vec::new();
        for p in names {
            if p.extension().is_none_or(|e| e != "ktx2") {
                continue;
            }
            let d = std::fs::read(&p).unwrap();
            let Ok(t) = crate::ktx2::parse(&d) else {
                continue;
            };
            if t.supercompression != crate::Supercompression::Basis2(crate::Basis2::Xuastc) {
                continue;
            }
            let crate::Format::Astc { w: bw, h: bh, .. } = t.format else {
                continue;
            };
            let Ok(want) = std::fs::read(p.with_extension("blocks")) else {
                continue;
            };
            let l = &t.levels[0];
            let (o, n) = t.seek.as_ref().unwrap()[0];
            let s = &d[l.range.clone()][o..o + n];
            let t0 = std::time::Instant::now();
            let r = transcode(s, t.width, t.height, bw, bh);
            let el = t0.elapsed();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            match r {
                Ok(ours) => {
                    let (b, x) = compare_ldr(&ours, &want, bw.into(), bh.into(), t.srgb);
                    eprintln!(
                        "{name}: profile {}, {} blocks, {b} differ, {x} texels differ, {el:?}",
                        s[0],
                        ours.len() / 16
                    );
                    if x != 0 {
                        bad_files.push(name);
                    }
                }
                Err(e) => {
                    eprintln!("{name}: profile {}: {e}", s[0]);
                    bad_files.push(name);
                }
            }
        }
        assert!(bad_files.is_empty(), "{bad_files:?}");
    }
}
