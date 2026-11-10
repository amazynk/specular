//! Fujifilm compressed RAF sensor codec (lossless and lossy, X-Trans and Bayer).
//!
//! Independent implementation written from the behaviour of existing decoders (studied, not
//! copied); output is verified bit-exact against LibRaw on real files.
//!
//! Layout: a 16-byte header (`IS` signature, lossless flag, raw type, bits, height, rounded
//! width, width, block width, blocks per row, line groups), a table of per-block byte sizes,
//! per-line quantiser bases (lossy only), then one entropy-coded stream per vertical block
//! (768 pixels wide). Each block is coded independently, so blocks decode in parallel.
//!
//! A block is coded in groups of six sensor rows. The samples of a group are sorted into
//! colour lines (3 red, 6 green, 3 blue; X-Trans packs 768 pixels into 512-sample lines,
//! Bayer into 384), and each line is predicted from the two lines above it of the same colour
//! (gradient-adaptive median-like prediction, Golomb-like codes with adaptive parameter per
//! gradient context). On X-Trans some red / blue / green positions are not coded but
//! interpolated from their neighbours in the colour line.

use rayon::prelude::*;

use crate::bits::BitReader;

/// Parsed compressed-data header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub lossless: bool,
    pub xtrans: bool,
    pub bits: u32,
    pub height: u32,
    pub width: u32,
    pub block: u32,
    pub blocks: u32,
    /// Line groups (6 rows each).
    pub lines: u32,
}

pub(crate) const HEADER_LEN: usize = 16;

/// Parse the 16-byte header at the start of the raw strip; `None` if it is not a (supported)
/// compressed stream.
pub(crate) fn parse_header(b: &[u8]) -> Option<Header> {
    let b = b.get(..HEADER_LEN)?;
    let be16 = |o: usize| u16::from_be_bytes([b[o], b[o + 1]]) as u32;
    let (sig, lossless, raw_type, bits) = (be16(0), b[2], b[3], b[4] as u32);
    let (height, rounded, width, block) = (be16(5), be16(7), be16(9), be16(11));
    let (blocks, lines) = (b[13] as u32, be16(14));
    let ok = sig == 0x4953
        && lossless <= 1
        && (raw_type == 0 || raw_type == 16)
        && matches!(bits, 12 | 14 | 16)
        && (6..=0x4002).contains(&height)
        && height % 6 == 0
        && block == 0x300
        && (0x300..=0x4200).contains(&width)
        && width % 24 == 0
        && rounded <= 0x4200
        && rounded >= block
        && rounded % block == 0
        && rounded - width < block
        && (1..=16).contains(&blocks)
        && blocks == rounded / block
        && (1..=0xaab).contains(&lines)
        && lines == height / 6;
    ok.then_some(Header {
        lossless: lossless == 1,
        xtrans: raw_type == 16,
        bits,
        height,
        width,
        block,
        blocks,
        lines,
    })
}

// Colour line indices inside the 18-line ring of a block.
const R0: usize = 0;
const R2: usize = 2;
const R3: usize = 3;
const R4: usize = 4;
const G0: usize = 5;
const G2: usize = 7;
const G3: usize = 8;
const G4: usize = 9;
const G5: usize = 10;
const G6: usize = 11;
const G7: usize = 12;
const B0: usize = 13;
const B2: usize = 15;
const B3: usize = 16;
const B4: usize = 17;
const NLINES: usize = 18;

/// Even-position handling of a colour line in one pass: bit 0 set = positions with
/// `pos % 4 == 0` are interpolated, bit 1 = positions with `pos % 4 == 2`; the rest are coded.
type EvenMode = u8;
const CODED: EvenMode = 0;
const INTERP: EvenMode = 3;

/// One pass: two colour lines decoded interleaved, their gradient set, X-Trans even modes.
struct Pass {
    a: usize,
    b: usize,
    set: usize,
    xt: [EvenMode; 2],
}

const PASSES: [Pass; 6] = [
    Pass {
        a: R2,
        b: G2,
        set: 0,
        xt: [INTERP, CODED],
    },
    Pass {
        a: G3,
        b: B2,
        set: 1,
        xt: [CODED, INTERP],
    },
    Pass {
        a: R3,
        b: G4,
        set: 2,
        xt: [1, INTERP],
    },
    Pass {
        a: G5,
        b: B3,
        set: 0,
        xt: [CODED, 2],
    },
    Pass {
        a: R4,
        b: G6,
        set: 1,
        xt: [2, CODED],
    },
    Pass {
        a: G7,
        b: B4,
        set: 2,
        xt: [INTERP, 1],
    },
];

/// Quantisation table for gradient contexts.
#[derive(Clone)]
struct QTable {
    /// Indexed by `max_value + difference`.
    lut: Vec<i8>,
    q_base: i32,
    max_grad: i32,
    grad_mult: i32,
    total_values: i32,
    raw_bits: u32,
}

/// Bit length of `val - 1` (at least 1 for `val >= 1`).
fn log2ceil(val: i32) -> u32 {
    if val <= 0 {
        0
    } else {
        (32 - ((val - 1) as u32).leading_zeros()).max(1)
    }
}

fn qlut(qp: [i32; 5]) -> Vec<i8> {
    (-qp[4]..=qp[4])
        .map(|v| {
            if v <= -qp[3] {
                -4
            } else if v <= -qp[2] {
                -3
            } else if v <= -qp[1] {
                -2
            } else if v < -qp[0] {
                -1
            } else if v <= qp[0] {
                0
            } else if v < qp[1] {
                1
            } else if v < qp[2] {
                2
            } else if v < qp[3] {
                3
            } else {
                4
            }
        })
        .collect()
}

fn main_qtable(max_value: i32, q_base: i32) -> QTable {
    let lim = max_value + 1;
    let mut qp = [
        q_base,
        3 * q_base + 0x12,
        5 * q_base + 0x43,
        7 * q_base + 0x114,
        max_value,
    ];
    if qp[1] >= lim || qp[1] < q_base + 1 {
        qp[1] = q_base + 1;
    }
    if qp[2] < qp[1] || qp[2] >= lim {
        qp[2] = qp[1];
    }
    if qp[3] < qp[2] || qp[3] >= lim {
        qp[3] = qp[2];
    }
    let total_values = (qp[4] + 2 * q_base) / (2 * q_base + 1) + 1;
    QTable {
        lut: qlut(qp),
        q_base,
        max_grad: 0,
        grad_mult: 9,
        total_values,
        raw_bits: log2ceil(total_values),
    }
}

/// The three fixed tables used by lossy files for flat areas.
fn lossy_qtables(max_value: i32) -> [QTable; 3] {
    let mk = |q_base: i32, max_grad: i32, div: i32, t: [i32; 3]| {
        let mut qp = [q_base, 0, 0, 0, max_value];
        qp[1] = if max_value >= t[0] { t[0] } else { q_base + 1 };
        qp[2] = if max_value >= t[1] { t[1] } else { qp[1] };
        qp[3] = if max_value >= t[2] { t[2] } else { qp[2] };
        let total_values = (max_value + div - 1) / div + 1;
        QTable {
            lut: qlut(qp),
            q_base,
            max_grad,
            grad_mult: 3,
            total_values,
            raw_bits: log2ceil(total_values),
        }
    };
    [
        mk(0, 5, 1, [0x12, 0x43, 0x114]),
        mk(1, 6, 3, [0x15, 0x48, 0x11b]),
        mk(2, 7, 5, [0x18, 0x4d, 0x122]),
    ]
}

/// Adaptive code parameter state: (sum of magnitudes, count) per gradient context.
#[derive(Clone, Copy)]
struct Grads {
    main: [[i32; 2]; 41],
    lossy: [[[i32; 2]; 5]; 3],
}

impl Grads {
    const ZERO: Grads = Grads {
        main: [[0; 2]; 41],
        lossy: [[[0; 2]; 5]; 3],
    };
}

struct Common {
    max_value: i32,
    max_bits: u32,
    lw: usize,
    lossless: bool,
    xtrans: bool,
    lossy: [QTable; 3],
}

/// Decoder state of one vertical block.
struct Block<'a> {
    br: BitReader<'a>,
    q_bases: &'a [u8],
    q0: QTable,
    lines: Vec<u16>,
    stride: usize,
    even: [Grads; 3],
    odd: [Grads; 3],
    /// Output column range of this block.
    x0: usize,
    width: usize,
    /// Data ran out or was corrupt: the remaining rows are left at zero.
    dead: bool,
}

const MIN_VALUE: i32 = 0x40;

impl Block<'_> {
    fn init_main_grads(&mut self) {
        let md = ((self.q0.total_values + 0x20) >> 6).max(2);
        for g in self.even.iter_mut().chain(self.odd.iter_mut()) {
            g.main = [[md, 1]; 41];
        }
    }

    #[inline(always)]
    fn interpolate_even(&mut self, line: usize, pos: usize) {
        let s = self.stride;
        let c = line * s + pos + 1;
        let l = &self.lines;
        let rb = l[c - s] as i32;
        let rc = l[c - s - 1] as i32;
        let rd = l[c - s + 1] as i32;
        let rf = l[c - 2 * s] as i32;
        let (dcb, dfb, ddb) = ((rc - rb).abs(), (rf - rb).abs(), (rd - rb).abs());
        let v = if dcb > dfb && dcb > ddb {
            rf + rd + 2 * rb
        } else if ddb > dcb && ddb > dfb {
            rf + rc + 2 * rb
        } else {
            rd + rc + 2 * rb
        };
        self.lines[c] = (v >> 2) as u16;
    }

    /// Decode one coded sample at `pos` of colour line `line`.
    #[inline(always)]
    fn sample(&mut self, cm: &Common, line: usize, pos: usize, set: usize, odd: bool) {
        let s = self.stride;
        let c = line * s + pos + 1;
        let l = &self.lines;
        let rb = l[c - s] as i32;
        let rc = l[c - s - 1] as i32;
        let rd = l[c - s + 1] as i32;
        let (flat, v1, v2, interp) = if odd {
            let ra = l[c - 1] as i32;
            let rg = l[c + 1] as i32;
            let interp = if (rb > rc && rb > rd) || (rb < rc && rb < rd) {
                (rg + ra + 2 * rb) >> 2
            } else {
                (ra + rg) >> 1
            };
            ((rb - rc).abs() + (rc - ra).abs(), rb - rc, rc - ra, interp)
        } else {
            let rf = l[c - 2 * s] as i32;
            let (dcb, dfb, ddb) = ((rc - rb).abs(), (rf - rb).abs(), (rd - rb).abs());
            let sum = if dcb > dfb && dcb > ddb {
                rf + rd + 2 * rb
            } else if ddb > dcb && ddb > dfb {
                rf + rc + 2 * rb
            } else {
                rd + rc + 2 * rb
            };
            (dfb + dcb, rb - rf, rc - rb, sum >> 2)
        };
        let gs = if odd {
            &mut self.odd[set]
        } else {
            &mut self.even[set]
        };
        let mut qt = &self.q0;
        let mut grads: &mut [[i32; 2]] = &mut gs.main;
        if !cm.lossless {
            for i in 1..=(self.q0.q_base.min(3) as usize) {
                if flat <= cm.lossy[i - 1].max_grad {
                    qt = &cm.lossy[i - 1];
                    grads = &mut gs.lossy[i - 1];
                    break;
                }
            }
        }
        let mx = cm.max_value;
        let grad =
            qt.grad_mult * qt.lut[(mx + v1) as usize] as i32 + qt.lut[(mx + v2) as usize] as i32;
        let gi = grad.unsigned_abs() as usize;

        // Unary prefix: count of zero bits before a one.
        let br = &mut self.br;
        let mut zeros = 0u32;
        loop {
            br.refill();
            let lz = br.peek(32).leading_zeros().min(br.nbits());
            if lz < br.nbits().min(32) {
                zeros += lz;
                br.consume(lz + 1);
                break;
            }
            let n = br.nbits().min(32);
            zeros += n;
            br.consume(n);
            if zeros > cm.max_bits || br.overrun() {
                self.dead = true;
                return;
            }
        }
        let g = &mut grads[gi];
        let mut code = if zeros + qt.raw_bits + 1 < cm.max_bits {
            let (a, b) = (g[0], g[1]);
            let d = if b >= a {
                0
            } else {
                let mut d = b.leading_zeros() - a.leading_zeros();
                if ((b as u64) << d) < a as u64 {
                    d += 1;
                }
                d.min(15)
            };
            br.refill();
            (br.bits(d) + (zeros << d)) as i32
        } else {
            br.refill();
            br.bits(qt.raw_bits) as i32 + 1
        };
        code = if code & 1 != 0 {
            -1 - code / 2
        } else {
            code / 2
        };
        g[0] += code.abs();
        if g[1] == MIN_VALUE {
            g[0] >>= 1;
            g[1] >>= 1;
        }
        g[1] += 1;
        let step = 2 * qt.q_base + 1;
        let mut v = if grad < 0 {
            interp - code * step
        } else {
            interp + code * step
        };
        if v < -qt.q_base {
            v += qt.total_values * step;
        } else if v > qt.q_base + mx {
            v -= qt.total_values * step;
        }
        self.lines[c] = v.clamp(0, mx) as u16;
    }

    #[inline(always)]
    fn even(&mut self, cm: &Common, line: usize, pos: usize, set: usize, mode: EvenMode) {
        let bit = if pos & 3 == 0 { 1 } else { 2 };
        if mode & bit != 0 {
            self.interpolate_even(line, pos);
        } else {
            self.sample(cm, line, pos, set, false);
        }
    }

    fn extend(&mut self, from: usize, to: usize) {
        let (s, lw) = (self.stride, self.stride - 2);
        for i in from..=to {
            self.lines[i * s] = self.lines[(i - 1) * s + 1];
            self.lines[i * s + lw + 1] = self.lines[(i - 1) * s + lw];
        }
    }

    /// Decode one group of six rows into `rows` (this block's columns of each row).
    fn decode_group(
        &mut self,
        cm: &Common,
        group: usize,
        rows: &mut [&mut [u16]],
        cfa: &[[u8; 6]; 6],
    ) {
        if self.dead {
            return;
        }
        if !cm.lossless {
            let qb = self.q_bases.get(group).copied().unwrap_or(0) as i32;
            if group == 0 || qb != self.q0.q_base {
                self.q0 = main_qtable(cm.max_value, qb);
                self.init_main_grads();
            }
        }
        let lw = cm.lw;
        for (k, p) in PASSES.iter().enumerate() {
            let (ma, mb) = if cm.xtrans {
                (p.xt[0], p.xt[1])
            } else {
                (CODED, CODED)
            };
            let (mut ev, mut od) = (0usize, 1usize);
            while ev < lw || od < lw {
                if ev < lw {
                    self.even(cm, p.a, ev, p.set, ma);
                    self.even(cm, p.b, ev, p.set, mb);
                    ev += 2;
                }
                if ev > 8 {
                    self.sample(cm, p.a, od, p.set, true);
                    self.sample(cm, p.b, od, p.set, true);
                    od += 2;
                }
            }
            if self.dead {
                return;
            }
            if k % 2 == 0 {
                self.extend(R2, R4);
                self.extend(G2, G7);
            } else {
                self.extend(G2, G7);
                self.extend(B2, B4);
            }
        }
        let s = self.stride;
        // The last two lines of each colour become the context of the next group.
        for (dst, src) in [
            (R0, R3),
            (R0 + 1, R4),
            (G0, G6),
            (G0 + 1, G7),
            (B0, B3),
            (B0 + 1, B4),
        ] {
            self.lines.copy_within(src * s..(src + 1) * s, dst * s);
        }
        self.store(cm, rows, cfa);
        for (first, n) in [(R2, 3), (G2, 6), (B2, 3)] {
            self.lines[first * s..(first + n) * s].fill(0);
            self.lines[first * s] = self.lines[(first - 1) * s + 1];
            self.lines[first * s + lw + 1] = self.lines[(first - 1) * s + lw];
        }
        if self.br.overrun() {
            self.dead = true;
        }
    }

    fn store(&self, cm: &Common, rows: &mut [&mut [u16]], cfa: &[[u8; 6]; 6]) {
        let s = self.stride;
        let line_of = |row: usize, color: u8| match color {
            0 => R2 + (row >> 1),
            2 => B2 + (row >> 1),
            _ => G2 + row,
        };
        for (row, out) in rows.iter_mut().enumerate() {
            let out = &mut out[..self.width];
            if cm.xtrans {
                // Pixel 3k + j of the block sits at colour-line index 2k + (j != 0).
                let src: [usize; 6] = std::array::from_fn(|j| {
                    line_of(row, cfa[row][(self.x0 + j) % 6]) * s + 1 + [0, 1, 1, 2, 3, 3][j]
                });
                for (m, o) in out.chunks_mut(6).enumerate() {
                    for (j, v) in o.iter_mut().enumerate() {
                        *v = self.lines[src[j] + 4 * m];
                    }
                }
            } else {
                let src: [usize; 2] =
                    std::array::from_fn(|j| line_of(row, cfa[row & 1][(self.x0 + j) & 1]) * s + 1);
                for (m, o) in out.chunks_mut(2).enumerate() {
                    for (j, v) in o.iter_mut().enumerate() {
                        *v = self.lines[src[j] + m];
                    }
                }
            }
        }
    }
}

/// Decode a compressed stream. `d` starts right after the 16-byte header; `cfa[r][c]` is the
/// colour (0 R, 1 G, 2 B) at raw row `r % 6`, column `c % 6`. `out` is `width x height`.
/// Returns whether the data was truncated or corrupt (affected rows are left at zero).
pub(crate) fn decode(
    h: &Header,
    d: &[u8],
    cfa: &[[u8; 6]; 6],
    out: &mut [u16],
    parallel: bool,
) -> Result<bool, &'static str> {
    let (w, ht) = (h.width as usize, h.height as usize);
    if out.len() < w * ht {
        return Err("Fujifilm output buffer");
    }
    let nb = h.blocks as usize;
    let mut off = (4 * nb + 15) & !15;
    let lines = h.lines as usize;
    let q_stride = (lines + 15) & !15;
    let q_all = if h.lossless {
        &[][..]
    } else {
        let q = d
            .get(off..off + nb * q_stride)
            .ok_or("Fujifilm quantiser table")?;
        off += nb * q_stride;
        q
    };
    let max_value = (1i32 << h.bits) - 1;
    let lw = if h.xtrans {
        (h.block as usize * 2) / 3
    } else {
        h.block as usize / 2
    };
    let cm = Common {
        max_value,
        max_bits: 4 * log2ceil(max_value + 1),
        lw,
        lossless: h.lossless,
        xtrans: h.xtrans,
        lossy: lossy_qtables(max_value),
    };
    let mut truncated = false;
    let mut states = Vec::with_capacity(nb);
    for i in 0..nb {
        let size = d
            .get(4 * i..4 * i + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap_or_default()) as usize)
            .ok_or("Fujifilm block table")?;
        let end = off.saturating_add(size);
        if end > d.len() {
            truncated = true;
        }
        let data = d.get(off.min(d.len())..end.min(d.len())).unwrap_or(&[]);
        off = end;
        let x0 = i * h.block as usize;
        let width = (w.saturating_sub(x0)).min(h.block as usize);
        let mut b = Block {
            br: BitReader::new(data, false),
            q_bases: q_all.get(i * q_stride..(i + 1) * q_stride).unwrap_or(&[]),
            q0: main_qtable(max_value, 0),
            lines: vec![0; NLINES * (lw + 2)],
            stride: lw + 2,
            even: [Grads::ZERO; 3],
            odd: [Grads::ZERO; 3],
            x0,
            width,
            dead: width == 0,
        };
        if h.lossless {
            b.init_main_grads();
        } else {
            for k in 0..3 {
                let md = ((cm.lossy[k].total_values + 0x20) >> 6).max(2);
                for g in b.even.iter_mut().chain(b.odd.iter_mut()) {
                    g.lossy[k] = [[md, 1]; 5];
                }
            }
        }
        states.push(b);
    }

    // Every block owns its columns of every row; blocks decode independently.
    let mut per_block: Vec<Vec<&mut [u16]>> = (0..nb).map(|_| Vec::with_capacity(ht)).collect();
    for row in out[..w * ht].chunks_mut(w) {
        let mut rest = row;
        for pb in per_block.iter_mut() {
            let n = (h.block as usize).min(rest.len());
            let (a, b) = std::mem::take(&mut rest).split_at_mut(n);
            pb.push(a);
            rest = b;
        }
    }
    let run = |(b, mut rows): (&mut Block, Vec<&mut [u16]>)| {
        for (k, g) in rows.chunks_mut(6).enumerate() {
            b.decode_group(&cm, k, g, cfa);
        }
    };
    if parallel {
        states.par_iter_mut().zip(per_block).for_each(run);
    } else {
        states.iter_mut().zip(per_block).for_each(run);
    }
    Ok(truncated || states.iter().any(|b| b.dead && b.width > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_validation() {
        let mut h = [0u8; 16];
        h[..2].copy_from_slice(&0x4953u16.to_be_bytes());
        h[2] = 1;
        h[3] = 16;
        h[4] = 14;
        h[5..7].copy_from_slice(&4038u16.to_be_bytes());
        h[7..9].copy_from_slice(&6144u16.to_be_bytes());
        h[9..11].copy_from_slice(&6048u16.to_be_bytes());
        h[11..13].copy_from_slice(&768u16.to_be_bytes());
        h[13] = 8;
        h[14..16].copy_from_slice(&673u16.to_be_bytes());
        let p = parse_header(&h).unwrap();
        assert!(p.lossless && p.xtrans && p.bits == 14 && p.blocks == 8);
        h[13] = 7;
        assert!(parse_header(&h).is_none());
    }

    #[test]
    fn log2ceil_matches_definition() {
        assert_eq!(log2ceil(0), 0);
        assert_eq!(log2ceil(1), 1);
        assert_eq!(log2ceil(2), 1);
        assert_eq!(log2ceil(5), 3);
        assert_eq!(log2ceil(16384), 14);
        assert_eq!(log2ceil(16385), 15);
    }

    #[test]
    fn garbage_does_not_panic() {
        let h = Header {
            lossless: false,
            xtrans: true,
            bits: 14,
            height: 12,
            width: 768,
            block: 768,
            blocks: 1,
            lines: 2,
        };
        let mut d = vec![0u8; 64];
        d[3] = 40;
        for (i, b) in d.iter_mut().enumerate().skip(48) {
            *b = (i * 37) as u8;
        }
        let mut out = vec![0u16; 768 * 12];
        let cfa = [[1u8; 6]; 6];
        assert!(decode(&h, &d, &cfa, &mut out, false).is_ok());
    }
}
