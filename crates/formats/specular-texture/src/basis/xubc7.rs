//! XUBC7 (Basis Universal v2.50): supercompressed BC7, transcoded to BC7 blocks.
//!
//! Written from Binomial's published "XUBC7 Low-Level Format" and "XUBC7 File Format"
//! documents (Apache-2.0); section numbers (§) below refer to the low-level document.
//! A slice is either a tiny mip (raw BC7 blocks, §11) or a blob container (§3) of
//! per-role streams that rebuild each block's latent BC7 fields: config, partition,
//! endpoints (always exact) and weights (DPCM or a fixed-point DCT). The rebuilt
//! logical blocks are packed to standard 16-byte BC7 blocks (§11.2).

// Texel, channel and stripe indices mirror the document's loops.
#![allow(clippy::needless_range_loop)]

use std::borrow::Cow;
use std::io::Read;

use crate::{Error, Result};

/// §3 rule 9: cap on the declared decompressed size of all compressed blobs.
const ARENA_CAP: u64 = 1 << 30;
/// Upper bound on any per-stripe stream's bytes per block (a valid stream consumes all of
/// its bytes, and no block reads more than this from one stream).
const MAX_BYTES_PER_BLOCK: u64 = 64;

const SHORT: Error = Error::Truncated("XUBC7 stream");

fn bad(m: &'static str) -> Error {
    Error::Malformed(m)
}

/// Transcode one image (`w`×`h` texels) to BC7 blocks (16 bytes each, row-major).
pub(crate) fn transcode(d: &[u8], w: u32, h: u32) -> Result<Vec<u8>> {
    let nbx = w.div_ceil(4) as usize;
    let nby = h.div_ceil(4) as usize;
    match d.first() {
        None => Err(Error::Truncated("XUBC7 slice")),
        Some(0xB7) => Slice::parse(d, nbx, nby)?.decode(),
        Some(0xB8 | 0xB9) => tiny(d, nbx, nby),
        Some(&b) => Err(Error::Unsupported(format!("XUBC7 slice marker {b:#04x}"))),
    }
}

// ---- §1 output model -------------------------------------------------------------------

/// Per mode: subsets, pattern bits, planes, RGB bits, alpha bits, p-bits per subset,
/// weight bits of plane 0 and plane 1 (§1.1).
const MODES: [[u8; 8]; 8] = [
    [3, 4, 1, 4, 0, 2, 3, 0],
    [2, 6, 1, 6, 0, 1, 3, 0],
    [3, 6, 1, 5, 0, 0, 2, 0],
    [2, 6, 1, 7, 0, 2, 2, 0],
    [1, 0, 2, 5, 6, 0, 2, 3],
    [1, 0, 2, 7, 8, 0, 2, 2],
    [1, 0, 1, 7, 7, 2, 4, 0],
    [2, 6, 1, 5, 5, 2, 2, 0],
];

const W2: [u8; 4] = [0, 21, 43, 64];
const W3: [u8; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const W4: [u8; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn dq(w: u8, n: u8) -> u8 {
    match n {
        2 => W2[(w & 3) as usize],
        3 => W3[(w & 7) as usize],
        _ => W4[(w & 15) as usize],
    }
}

/// A logical BC7 block (§1).
#[derive(Clone, Copy, Default)]
struct Blk {
    mode: u8,
    parts: u8,
    pattern_bits: u8,
    pattern: u8,
    planes: u8,
    rot: u8,
    sel: u8,
    ep_bits: [u8; 2],
    /// [subset][lo/hi][channel], quantized field values.
    ep: [[[u8; 4]; 2]; 3],
    wbits: [u8; 2],
    w: [[u8; 16]; 2],
    npbits: u8,
    shared: bool,
    pbits: [u8; 6],
}

impl Blk {
    fn new(mode: u8) -> Blk {
        let m = MODES[mode as usize];
        Blk {
            mode,
            parts: m[0],
            pattern_bits: m[1],
            planes: m[2],
            ep_bits: [m[3], m[4]],
            wbits: [m[6], m[7]],
            npbits: m[0] * m[5],
            shared: m[5] == 1,
            ..Blk::default()
        }
    }

    fn dual(&self) -> bool {
        self.planes == 2
    }

    fn comps(&self) -> usize {
        if self.ep_bits[1] != 0 { 4 } else { 3 }
    }

    /// The stored field that really holds alpha (dual-plane modes only).
    fn ccs(&self) -> usize {
        (self.rot as usize + 3) & 3
    }

    fn chan_plane(&self, c: usize) -> usize {
        if !self.dual() {
            0
        } else if c == 3 {
            1 - self.sel as usize
        } else {
            self.sel as usize
        }
    }

    fn pbits_per_subset(&self) -> usize {
        (self.npbits / self.parts) as usize
    }

    fn subset_of(&self, i: usize) -> usize {
        let p = self.pattern as usize;
        match self.parts {
            2 => ((PART2[p] >> i) & 1) as usize,
            3 => ((PART3[p] >> (2 * i)) & 3) as usize,
            _ => 0,
        }
    }

    /// §1.3: a subset's two endpoints expanded to 8 bits (alpha 255 when the mode has none).
    fn unpack_endpoints(&self, s: usize) -> [[u8; 4]; 2] {
        let mut ep = [[0, 0, 0, 255]; 2];
        for (e, out) in ep.iter_mut().enumerate() {
            for c in 0..self.comps() {
                let bits = u32::from(self.ep_bits[usize::from(c == 3)]);
                let v = u32::from(self.ep[s][e][c]);
                out[c] = if self.npbits != 0 {
                    let p = self.pbits[if self.shared { s } else { s * 2 + e }];
                    let total = bits + 1;
                    let v = ((v << 1) | u32::from(p)) << (8 - total);
                    (v | (v >> total)) as u8
                } else {
                    let v = v << (8 - bits);
                    (v | (v >> bits)) as u8
                };
            }
        }
        ep
    }

    /// §9 decode_texel.
    fn texel(&self, i: usize) -> [u8; 4] {
        let ep = self.unpack_endpoints(self.subset_of(i));
        let interp = |l: u8, h: u8, w: u8, n: u8| {
            let w = u32::from(dq(w, n));
            ((u32::from(l) * (64 - w) + u32::from(h) * w + 32) >> 6) as u8
        };
        let vp = self.sel as usize;
        let mut r = [0u8; 4];
        for c in 0..3 {
            r[c] = interp(ep[0][c], ep[1][c], self.w[vp][i], self.wbits[vp]);
        }
        r[3] = 255;
        if self.comps() == 4 {
            let sp = if self.dual() { 1 - vp } else { 0 };
            r[3] = interp(ep[0][3], ep[1][3], self.w[sp][i], self.wbits[sp]);
        }
        if self.rot != 0 {
            r.swap(3, self.rot as usize - 1);
        }
        r
    }

    /// §8.2.2 get_max_span_len, as a raw Q15.16 value.
    fn max_span(&self, plane: usize) -> i64 {
        let sq = |a: u8, b: u8| (i64::from(b) - i64::from(a)).pow(2);
        let ssq = if self.dual() {
            let ep = self.unpack_endpoints(0);
            (0..4)
                .filter(|&c| self.chan_plane(c) == plane)
                .map(|c| sq(ep[0][c], ep[1][c]))
                .sum()
        } else {
            (0..self.parts as usize)
                .map(|s| {
                    let ep = self.unpack_endpoints(s);
                    (0..4).map(|c| sq(ep[0][c], ep[1][c])).sum::<i64>()
                })
                .max()
                .unwrap_or(0)
        };
        let x = (ssq as u64) << 32;
        let mut f = x.isqrt();
        if x - f * f > f {
            f += 1;
        }
        f as i64
    }
}

// ---- §7.5.4 endpoint quantization -------------------------------------------------------

fn quant_endpoint(v8: u8, nbits: u8) -> u8 {
    let maxv = (1u32 << nbits) - 1;
    ((u32::from(v8) * maxv * 2 + 255) / 510) as u8
}

fn quant_endpoint_pbit(v8: u8, p: u32, iscalep: u32) -> u32 {
    let k = (u32::from(v8) * iscalep + 255 - 255 * p) / 510;
    (k * 2 + p).clamp(p, iscalep - 1 + p)
}

/// Quantize an 8-bit RGBA pair into `mode`'s endpoint fields and p-bits.
fn pack_endpoints_int(mode: u8, src: &[[u8; 4]; 2]) -> ([[u8; 4]; 2], [u8; 2]) {
    let m = MODES[mode as usize];
    let (rgb, a, pps) = (m[3], m[4], m[5]);
    let nc = if a != 0 { 4 } else { 3 };
    let mut dst = [[0u8; 4]; 2];
    let mut dp = [0u8; 2];
    if pps == 0 {
        for e in 0..2 {
            for c in 0..4 {
                let bits = if c == 3 { a } else { rgb };
                dst[e][c] = if bits != 0 {
                    quant_endpoint(src[e][c], bits)
                } else {
                    0
                };
            }
        }
        return (dst, dp);
    }
    let total = u32::from(rgb) + 1;
    let iscalep = (1u32 << total) - 1;
    let (mut best0, mut best1) = (u64::MAX, u64::MAX);
    for p in 0..2u32 {
        let mut q = [[0u32; 4]; 2];
        let mut err = [0u64; 2];
        for e in 0..2 {
            for c in 0..4 {
                q[e][c] = quant_endpoint_pbit(src[e][c], p, iscalep);
                let mut s = q[e][c] << (8 - total);
                s |= s >> total;
                if c < nc {
                    err[e] += (i64::from(s as i32) - i64::from(src[e][c])).pow(2) as u64;
                }
            }
        }
        if pps == 1 {
            let e = err[0] + err[1];
            if e < best0 {
                best0 = e;
                dp = [p as u8; 2];
                for j in 0..4 {
                    dst[0][j] = (q[0][j] >> 1) as u8;
                    dst[1][j] = (q[1][j] >> 1) as u8;
                }
            }
        } else {
            if err[0] < best0 {
                best0 = err[0];
                dp[0] = p as u8;
                for j in 0..4 {
                    dst[0][j] = (q[0][j] >> 1) as u8;
                }
            }
            if err[1] < best1 {
                best1 = err[1];
                dp[1] = p as u8;
                for j in 0..4 {
                    dst[1][j] = (q[1][j] >> 1) as u8;
                }
            }
        }
    }
    (dst, dp)
}

/// §7.5.3: add the residuals to `pred`'s subset `ps`, requantized into `b`'s mode.
fn endpoint_dpcm(pred: &Blk, ps: usize, b: &mut Blk, s: usize, res: &[u8; 8], rp: &[u8; 2]) {
    let mut pe = pred.unpack_endpoints(ps);
    if pred.dual() {
        let i = pred.ccs();
        pe[0].swap(i, 3);
        pe[1].swap(i, 3);
    }
    if b.dual() {
        let i = b.ccs();
        pe[0].swap(i, 3);
        pe[1].swap(i, 3);
    }
    let (packed, ppb) = pack_endpoints_int(b.mode, &pe);
    let (mut g, mut a) = (1, 3);
    if b.dual() {
        a = b.ccs();
        if a == 1 {
            g = 3;
        }
    }
    let nc = b.comps();
    let mut t = *res;
    for c in 0..nc {
        if c == g || c == a {
            continue;
        }
        t[c * 2] = t[c * 2].wrapping_add(t[g * 2]);
        t[c * 2 + 1] = t[c * 2 + 1].wrapping_add(t[g * 2 + 1]);
    }
    for c in 0..nc {
        let mask = (1u32 << b.ep_bits[usize::from(c == 3)]) - 1;
        for e in 0..2 {
            b.ep[s][e][c] = ((u32::from(t[c * 2 + e]) + u32::from(packed[e][c])) & mask) as u8;
        }
    }
    let pps = b.pbits_per_subset();
    for p in 0..pps {
        b.pbits[s * pps + p] = (rp[p] + ppb[p]) & 1;
    }
}

// ---- tables built once per slice -------------------------------------------------------

struct Tables {
    /// §1.2 quant_weight for n = 2, 3, 4 over [0, 64].
    qw: [[u8; 65]; 3],
    /// §9.1 mode-5 (lo, hi) per 8-bit target at weight index 1.
    solid: [(u8, u8); 256],
}

impl Tables {
    fn new() -> Tables {
        let mut qw = [[0u8; 65]; 3];
        for (k, row) in qw.iter_mut().enumerate() {
            let n = k as u8 + 2;
            for (v, out) in row.iter_mut().enumerate() {
                let mut best = u32::MAX;
                for idx in 0..1u8 << n {
                    let err = (v as i32 - i32::from(dq(idx, n))).unsigned_abs();
                    if err < best {
                        best = err;
                        *out = idx;
                    }
                }
            }
        }
        // The first (l, h) in ascending order reaching each interpolated value k; the
        // reference search's strict tie rule then picks, among the nearest reachable k,
        // the one whose first pair comes earliest.
        let mut first = [u16::MAX; 256];
        for l in 0..128u16 {
            let low = (l << 1) | (l >> 6);
            for h in 0..128u16 {
                let high = (h << 1) | (h >> 6);
                let k = ((low * 43 + high * 21 + 32) >> 6) as usize;
                if first[k] == u16::MAX {
                    first[k] = l * 128 + h;
                }
            }
        }
        let mut solid = [(0u8, 0u8); 256];
        for (c, out) in solid.iter_mut().enumerate() {
            let mut pick = u16::MAX;
            for d in 0..256usize {
                for k in [c.wrapping_sub(d), c + d] {
                    if k < 256 && first[k] < pick {
                        pick = first[k];
                    }
                }
                if pick != u16::MAX {
                    break;
                }
            }
            *out = ((pick >> 7) as u8, (pick & 127) as u8);
        }
        Tables { qw, solid }
    }

    fn quant_weight(&self, v: i32, n: u8) -> u8 {
        self.qw[(n - 2) as usize][v.clamp(0, 64) as usize]
    }
}

// ---- §8.2 fixed-point DCT --------------------------------------------------------------

const ONE: i64 = 1 << 16;

fn rshift(x: i64, bits: u32) -> i64 {
    let half = 1i64 << (bits - 1);
    if x >= 0 {
        (x + half) >> bits
    } else {
        -((-x + half) >> bits)
    }
}

fn fmul(a: i64, b: i64) -> i64 {
    rshift(a * b, 16)
}

fn fdiv(a: i64, b: i64) -> i64 {
    rshift((a << 17) / b, 1)
}

fn idiv(a: i64, n: i64) -> i64 {
    let r = (a.abs() + n.abs() / 2) / n.abs();
    if (a < 0) != (n < 0) { -r } else { r }
}

/// Raw Q15.16 base quantization matrix (§8.2.2).
const BASE_QUANT: [i64; 16] = [
    65536, 229376, 1572864, 3342336, //
    229376, 786432, 2621440, 5111808, //
    1572864, 2621440, 4456448, 6750208, //
    3342336, 5111808, 6750208, 7864320,
];
const SCALE_STEPS: [i64; 3] = [88859, 81640, 75649];
const ZIGZAG: [(u8, u8); 16] = [
    (0, 0),
    (1, 0),
    (0, 1),
    (0, 2),
    (1, 1),
    (2, 0),
    (3, 0),
    (2, 1),
    (1, 2),
    (0, 3),
    (1, 3),
    (2, 2),
    (3, 1),
    (3, 2),
    (2, 3),
    (3, 3),
];

/// §8.2.2: the 16 coefficient steps for one plane.
fn quant_table(q: i64, span: i64, n: u8) -> [i64; 16] {
    let mut tab = [1i64; 16];
    if q >= 100 * ONE {
        return tab;
    }
    let qc = q.clamp(ONE, 100 * ONE);
    let level = if qc < 50 * ONE {
        fdiv(5000 * ONE, qc)
    } else {
        200 * ONE - qc * 2
    };
    let level = idiv(level, 100);
    let adaptive = fdiv(64 * ONE, span.max(14 * ONE));
    let adaptive = fmul(adaptive, SCALE_STEPS[(n - 2) as usize]);
    let scale = fmul(level, adaptive);
    for y in 0..4 {
        for x in (if y != 0 { y } else { 1 })..4 {
            let mut s = rshift(BASE_QUANT[x + y * 4] * scale, 32).max(1);
            if x + y == 1 {
                s = s.min(73);
            }
            tab[x + y * 4] = s;
            tab[y + x * 4] = s;
        }
    }
    tab
}

/// §8.2.3 AC dequantization to a raw Q15.16 value.
fn dequant_ac(coef: i64, l: i64, x: u8, y: u8) -> i64 {
    const SAT: i64 = 2048 * ONE;
    if x + y == 1 {
        return (coef * l * ONE).clamp(-SAT, SAT);
    }
    if coef == 0 || l <= 0 {
        return 0;
    }
    let mag = 32768 * l + coef.abs() * l * ONE;
    (if coef < 0 { -mag } else { mag }).clamp(-SAT, SAT)
}

fn inverse_ortho(y: [i64; 4]) -> [i64; 4] {
    const HALF: i64 = 32768;
    const C1: i64 = 42813;
    const C3: i64 = 17734;
    let b0 = y[0] * HALF + y[2] * HALF;
    let b1 = y[0] * HALF - y[2] * HALF;
    let t0 = y[1] * C1 + y[3] * C3;
    let t1 = y[1] * C3 - y[3] * C1;
    [
        rshift(b0 + t0, 16),
        rshift(b1 + t1, 16),
        rshift(b1 - t1, 16),
        rshift(b0 - t0, 16),
    ]
}

fn inverse_2d(dct: &[i64; 16]) -> [i64; 16] {
    let mut t = [0i64; 16];
    for x in 0..4 {
        let o = inverse_ortho([dct[x], dct[x + 4], dct[x + 8], dct[x + 12]]);
        for (k, v) in o.into_iter().enumerate() {
            t[x + k * 4] = v;
        }
    }
    let mut out = [0i64; 16];
    for y in 0..4 {
        let o = inverse_ortho([t[y * 4], t[y * 4 + 1], t[y * 4 + 2], t[y * 4 + 3]]);
        out[y * 4..y * 4 + 4].copy_from_slice(&o);
    }
    out
}

// ---- §7.7 weight predictors ------------------------------------------------------------

const XY_DELTAS: [(i8, i8); 32] = [
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

fn fw(b: &Blk, plane: usize, i: usize) -> i32 {
    let sp = if b.dual() { plane } else { 0 };
    i32::from(dq(b.w[sp][i], b.wbits[sp]))
}

/// The causal neighbourhood of one block, gated by its stripe.
struct Nb<'a> {
    blocks: &'a [Blk],
    nbx: usize,
    y0: usize,
    y1: usize,
    bx: usize,
    by: usize,
}

impl<'a> Nb<'a> {
    fn at(&self, dx: isize, dy: isize) -> Option<&'a Blk> {
        let x = self.bx.checked_add_signed(dx)?;
        let y = self.by.checked_add_signed(dy)?;
        (x < self.nbx && y >= self.y0 && y <= self.y1).then(|| &self.blocks[y * self.nbx + x])
    }
}

fn need(b: Option<&Blk>) -> Result<&Blk> {
    b.ok_or(bad("XUBC7 prediction from a block outside the stripe"))
}

/// §7.7: 16 predicted weights in [0, 64] for candidate `cand` (1..49) on `plane`.
fn predict(nb: &Nb, cand: u8, plane: usize) -> Result<[i32; 16]> {
    let p = plane;
    let mut w = [0i32; 16];
    let left = || need(nb.at(-1, 0));
    let upper = || need(nb.at(0, -1));
    let ld = || need(nb.at(-1, -1));
    let rd = || need(nb.at(1, -1));
    let edges = |l: &Blk, u: &Blk| {
        let lv: [i32; 4] = std::array::from_fn(|y| fw(l, p, 3 + y * 4));
        let uv: [i32; 4] = std::array::from_fn(|x| fw(u, p, x + 12));
        (lv, uv)
    };
    match cand {
        1 => {
            let l = left()?;
            for i in 0..16 {
                w[i] = fw(l, p, 3 + (i & !3));
            }
        }
        2 => {
            let u = upper()?;
            for i in 0..16 {
                w[i] = fw(u, p, (i & 3) + 12);
            }
        }
        3 | 6 | 7 => {
            let (lv, uv) = edges(left()?, upper()?);
            for y in 0..4 {
                for x in 0..4 {
                    let (l, u) = (lv[y], uv[x]);
                    w[x + y * 4] = match cand {
                        6 => (l + u + 1) >> 1,
                        _ => {
                            let (mut wl, mut wu) = (4 - x as i32, 4 - y as i32);
                            if cand == 7 {
                                wl *= wl;
                                wu *= wu;
                            }
                            let den = wl + wu;
                            (wl * l + wu * u + den / 2) / den
                        }
                    };
                }
            }
        }
        4 => {
            let l = left()?;
            for y in 0..4 {
                for x in 0..4 {
                    w[x + y * 4] = fw(l, p, (3 - x) + y * 4);
                }
            }
        }
        5 => {
            let u = upper()?;
            for y in 0..4 {
                for x in 0..4 {
                    w[x + y * 4] = fw(u, p, x + (3 - y) * 4);
                }
            }
        }
        8 | 9 | 13 | 14 => {
            let (lv, uv) = edges(left()?, upper()?);
            let c = fw(ld()?, p, 15);
            for y in 0..4 {
                for x in 0..4 {
                    let (l, u) = (lv[y], uv[x]);
                    w[x + y * 4] = match cand {
                        8 => (l + u - c).clamp(0, 64),
                        9 => {
                            let grad = (l + u - c).clamp(0, 64);
                            let (wl, wu) = (4 - x as i32, 4 - y as i32);
                            let den = wl + wu;
                            let blend = (wl * l + wu * u + den / 2) / den;
                            (grad + blend + 1) >> 1
                        }
                        13 => {
                            let (mn, mx) = (l.min(u), l.max(u));
                            let v = if c >= mx {
                                mn
                            } else if c <= mn {
                                mx
                            } else {
                                l + u - c
                            };
                            v.clamp(0, 64)
                        }
                        _ => {
                            let (wl, wu) = ((l - c).abs() + 1, (u - c).abs() + 1);
                            let den = wl + wu;
                            (wl * l + wu * u + den / 2) / den
                        }
                    };
                }
            }
        }
        10 => {
            let (a, b) = (ld()?, rd()?);
            for i in 0..16 {
                w[i] = (fw(a, p, i) + fw(b, p, i) + 1) >> 1;
            }
        }
        11 | 12 => {
            let (a, b) = (ld()?, rd()?);
            let uv: Option<[i32; 4]> = if cand == 12 {
                let u = upper()?;
                Some(std::array::from_fn(|x| fw(u, p, x + 12)))
            } else {
                None
            };
            for y in 0..4 {
                let (l, r) = (fw(a, p, 3 + y * 4), fw(b, p, y * 4));
                for x in 0..4 {
                    let xi = x as i32;
                    let diag = ((3 - xi) * l + xi * r + 1) / 3;
                    w[x + y * 4] = match uv {
                        None => diag,
                        Some(uv) => {
                            let (wu, wd) = (4 - y as i32, 1 + y as i32);
                            (wu * uv[x] + wd * diag + 2) / 5
                        }
                    };
                }
            }
        }
        15 => {
            let (lv, uv) = edges(left()?, upper()?);
            let sum_u: i32 = uv.iter().sum();
            let sum_l: i32 = lv.iter().sum();
            let gx = -3 * uv[0] - uv[1] + uv[2] + 3 * uv[3];
            let gy = -3 * lv[0] - lv[1] + lv[2] + 3 * lv[3];
            let base = 5 * (sum_u + sum_l);
            for y in 0..4i32 {
                for x in 0..4i32 {
                    let t = base + gx * (4 * x - 1) + gy * (4 * y - 1) + 20;
                    let v = if t >= 0 { t / 40 } else { -((-t + 39) / 40) };
                    w[(x + y * 4) as usize] = v.clamp(0, 64);
                }
            }
        }
        16 => {
            let (u, r) = (upper()?, rd()?);
            let t: [i32; 8] = std::array::from_fn(|k| {
                if k < 4 {
                    fw(u, p, k + 12)
                } else {
                    fw(r, p, k - 4 + 12)
                }
            });
            for y in 0..4 {
                for x in 0..4 {
                    let d = x + y;
                    w[x + y * 4] = if d == 6 {
                        (t[6] + 3 * t[7] + 2) >> 2
                    } else {
                        (t[d] + 2 * t[d + 1] + t[d + 2] + 2) >> 2
                    };
                }
            }
        }
        17 => {
            let (l, u, c) = (left()?, upper()?, ld()?);
            let mut a = [0i32; 9];
            for y in 0..4 {
                a[3 - y] = fw(l, p, 3 + y * 4);
            }
            a[4] = fw(c, p, 15);
            for x in 0..4 {
                a[5 + x] = fw(u, p, x + 12);
            }
            for y in 0..4 {
                for x in 0..4 {
                    let d = 4 + x - y;
                    w[x + y * 4] = (a[d - 1] + 2 * a[d] + a[d + 1] + 2) >> 2;
                }
            }
        }
        _ => {
            let (dx, dy) = XY_DELTAS[(cand - 18) as usize];
            let r = need(nb.at(dx as isize, dy as isize))?;
            for (i, v) in w.iter_mut().enumerate() {
                *v = fw(r, p, i);
            }
        }
    }
    Ok(w)
}

/// §7.7.1: scale or flip the prediction about its mean.
fn amplitude(w: &mut [i32; 16], amp: u8) {
    if amp == 0 {
        return;
    }
    let mean = (w.iter().sum::<i32>() + 8) >> 4;
    for v in w.iter_mut() {
        let f = if amp == 2 {
            *v
        } else {
            (2 * mean - *v).clamp(0, 64)
        };
        *v = if amp == 1 { f } else { (f + mean + 1) >> 1 };
    }
}

// ---- streams ---------------------------------------------------------------------------

/// A stripe's range of one stream; positions are bytes, or bits for the bit streams.
#[derive(Clone, Copy, Default)]
struct Cur<'a> {
    d: &'a [u8],
    pos: u64,
    end: u64,
}

impl Cur<'_> {
    fn byte(&mut self) -> Result<u8> {
        if self.pos >= self.end {
            return Err(SHORT);
        }
        let b = self.d[self.pos as usize];
        self.pos += 1;
        Ok(b)
    }

    fn bits(&mut self, n: u8) -> Result<u8> {
        if self.pos > self.end || u64::from(n) > self.end - self.pos {
            return Err(SHORT);
        }
        let mut v = 0u8;
        for k in 0..n {
            let p = self.pos + u64::from(k);
            v |= ((self.d[(p >> 3) as usize] >> (p & 7)) & 1) << k;
        }
        self.pos += u64::from(n);
        Ok(v)
    }
}

fn is_bit_stream(id: usize) -> bool {
    matches!(id, 9 | 10 | 19)
}

const ID_CMD: usize = 1;
const ID_CONFIG: usize = 2;
const ID_PART2: usize = 3;
const ID_PART3: usize = 4;
const ID_WPRED: usize = 5;
const ID_DC: usize = 6;
const ID_AC: usize = 8;
const ID_SIGNS: usize = 9;
const ID_PBITS: usize = 10;
const ID_EP_FINE: usize = 11;
const ID_EP_COARSE: usize = 15;
const ID_EP_RAW: usize = 19;
const ID_EP_INDEX: usize = 20;
const ID_RAW_WEIGHTS: usize = 21;
const ID_SOLID: usize = 22;
const ID_DPCM_WEIGHTS: usize = 23;
const ID_SEEK: usize = 26;

fn varint(d: &[u8], pos: &mut usize) -> Result<u64> {
    let mut v = 0u64;
    for k in 0..5 {
        let b = *d
            .get(*pos)
            .ok_or(Error::Truncated("XUBC7 blob directory"))?;
        *pos += 1;
        if k == 4 && b > 0x0F {
            return Err(bad("XUBC7 blob size over 32 bits"));
        }
        v |= u64::from(b & 0x7F) << (7 * k);
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(bad("XUBC7 blob size over 5 bytes"))
}

struct Entry<'a> {
    compressed: bool,
    size: u64,
    data: &'a [u8],
}

fn inflate(e: &Entry, cap: u64) -> Result<Vec<u8>> {
    if e.size > cap {
        return Err(bad("XUBC7 blob larger than the image needs"));
    }
    let dec = ruzstd::decoding::StreamingDecoder::new(e.data)
        .map_err(|_| bad("XUBC7 blob is not valid Zstandard"))?;
    let mut out = Vec::new();
    dec.take(e.size + 1)
        .read_to_end(&mut out)
        .map_err(|_| bad("XUBC7 blob is not valid Zstandard"))?;
    if out.len() as u64 != e.size {
        return Err(bad("XUBC7 blob inflates to the wrong size"));
    }
    Ok(out)
}

struct Slice<'a> {
    nbx: usize,
    nby: usize,
    q: i64,
    alpha: bool,
    /// (first row, rows) per stripe.
    stripes: Vec<(usize, usize)>,
    blobs: Vec<Cow<'a, [u8]>>,
    /// seek[id][stripe], with the end sentinel at index `stripes.len()`.
    seek: Vec<Vec<u64>>,
}

impl<'a> Slice<'a> {
    fn parse(d: &'a [u8], nbx: usize, nby: usize) -> Result<Slice<'a>> {
        if d.len() < 3 {
            return Err(Error::Truncated("XUBC7 slice"));
        }
        let mut dir: Vec<Option<Entry>> = (0..128).map(|_| None).collect();
        let mut pos = 2usize;
        let mut arena = 0u64;
        for _ in 0..d[1] {
            let b = *d.get(pos).ok_or(Error::Truncated("XUBC7 blob directory"))?;
            pos += 1;
            let compressed = b & 0x80 != 0;
            let size = varint(d, &mut pos)?;
            if size == 0 {
                return Err(bad("XUBC7 empty blob"));
            }
            let stored = if compressed {
                let s = varint(d, &mut pos)?;
                if s == 0 || s >= size {
                    return Err(bad("XUBC7 compressed blob not smaller than its data"));
                }
                arena += size;
                if arena > ARENA_CAP {
                    return Err(bad("XUBC7 blobs over 1 GiB"));
                }
                s
            } else {
                size
            };
            let end = pos as u64 + stored;
            if end > d.len() as u64 {
                return Err(Error::Truncated("XUBC7 blob"));
            }
            let slot = &mut dir[(b & 0x7F) as usize];
            if slot.is_some() {
                return Err(bad("XUBC7 blob id repeated"));
            }
            *slot = Some(Entry {
                compressed,
                size,
                data: &d[pos..end as usize],
            });
            pos = end as usize;
        }
        match d.get(pos) {
            None => return Err(Error::Truncated("XUBC7 end marker")),
            Some(&0x6A) if pos + 1 == d.len() => {}
            Some(_) => return Err(bad("XUBC7 end marker misplaced")),
        }

        // §4 header.
        let hdr = dir[0].as_ref().ok_or(bad("XUBC7 header blob missing"))?;
        if hdr.size != 7 {
            return Err(bad("XUBC7 header blob size"));
        }
        let hb: Cow<[u8]> = if hdr.compressed {
            Cow::Owned(inflate(hdr, 7)?)
        } else {
            Cow::Borrowed(hdr.data)
        };
        let width = u16::from_le_bytes([hb[0], hb[1]]);
        let height = u16::from_le_bytes([hb[2], hb[3]]);
        let (dct_q, flags, num_stripes) = (hb[4], hb[5], hb[6] as usize);
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return Err(bad("XUBC7 image size"));
        }
        if !(1..=100).contains(&dct_q) {
            return Err(bad("XUBC7 quality"));
        }
        if flags & !1 != 0 {
            return Err(Error::Unsupported(format!(
                "XUBC7 header flags {flags:#04x}"
            )));
        }
        if usize::from(width).div_ceil(4) != nbx || usize::from(height).div_ceil(4) != nby {
            return Err(bad("XUBC7 image size differs from the container's"));
        }
        if num_stripes == 0 || num_stripes > nby || num_stripes > 16 {
            return Err(bad("XUBC7 stripe count"));
        }
        let total = (nbx * nby) as u64;
        match &dir[ID_CMD] {
            Some(e) if e.size == total => {}
            _ => return Err(bad("XUBC7 command stream size")),
        }

        // Every stream this decoder reads (blob 7 is reserved and unread, §6).
        let cap = total * MAX_BYTES_PER_BLOCK + 64;
        let mut blobs: Vec<Cow<[u8]>> = Vec::with_capacity(27);
        for (id, e) in dir.iter().enumerate().take(27) {
            let b = match e {
                Some(e) if id != 0 && id != 7 => {
                    let cap = if id == ID_SEEK { 16 * 100 } else { cap };
                    if e.compressed {
                        Cow::Owned(inflate(e, cap)?)
                    } else if e.size > cap {
                        return Err(bad("XUBC7 blob larger than the image needs"));
                    } else {
                        Cow::Borrowed(e.data)
                    }
                }
                _ => Cow::Borrowed(&[][..]),
            };
            blobs.push(b);
        }

        // §5.1 stripe geometry.
        let (base, extra) = (nby / num_stripes, nby % num_stripes);
        let mut stripes = Vec::with_capacity(num_stripes);
        let mut row = 0;
        for i in 0..num_stripes {
            let n = base + usize::from(i < extra);
            stripes.push((row, n));
            row += n;
        }

        // §5.3 seek table.
        let mut seek = vec![vec![0u64; num_stripes + 1]; 26];
        for (id, s) in seek.iter_mut().enumerate().skip(1) {
            let len = blobs[id].len() as u64;
            s[num_stripes] = if is_bit_stream(id) { len * 8 } else { len };
        }
        if num_stripes > 1 {
            let t = &blobs[ID_SEEK];
            let n = num_stripes * 25;
            if t.len() != n * 4 {
                return Err(bad("XUBC7 stripe seek table size"));
            }
            for (id, s) in seek.iter_mut().enumerate().skip(1) {
                let end = s[num_stripes];
                let mut running = 0u64;
                for st in 0..num_stripes {
                    let e = st * 25 + id - 1;
                    let delta = u64::from(u32::from_le_bytes([
                        t[e],
                        t[n + e],
                        t[2 * n + e],
                        t[3 * n + e],
                    ]));
                    if st == 0 && delta != 0 {
                        return Err(bad("XUBC7 stripe seek table"));
                    }
                    running += delta;
                    if running > end {
                        return Err(bad("XUBC7 stripe seek table"));
                    }
                    s[st] = running;
                }
            }
        }
        Ok(Slice {
            nbx,
            nby,
            q: i64::from(dct_q) * ONE,
            alpha: flags & 1 != 0,
            stripes,
            blobs,
            seek,
        })
    }

    fn decode(&self) -> Result<Vec<u8>> {
        let tabs = Tables::new();
        let mut blocks = vec![Blk::default(); self.nbx * self.nby];
        for (s, &(y0, rows)) in self.stripes.iter().enumerate() {
            let mut cur = [Cur::default(); 26];
            for (id, c) in cur.iter_mut().enumerate().skip(1) {
                *c = Cur {
                    d: &self.blobs[id],
                    pos: self.seek[id][s],
                    end: self.seek[id][s + 1],
                };
            }
            let y1 = y0 + rows - 1;
            for by in y0..=y1 {
                for bx in 0..self.nbx {
                    let nb = Nb {
                        blocks: &blocks,
                        nbx: self.nbx,
                        y0,
                        y1,
                        bx,
                        by,
                    };
                    let b = self.block(&nb, &tabs, &mut cur)?;
                    blocks[by * self.nbx + bx] = b;
                }
            }
            // §5.5: every cursor consumed (bit streams may leave up to 7 pad bits).
            for (id, c) in cur.iter().enumerate().skip(1) {
                let done = if is_bit_stream(id) {
                    c.pos <= c.end && c.end - c.pos < 8
                } else {
                    id == 7 || c.pos == c.end
                };
                if !done {
                    return Err(bad(
                        "XUBC7 stream not fully consumed at the end of a stripe",
                    ));
                }
            }
        }
        let mut out = vec![0u8; blocks.len() * 16];
        for (b, o) in blocks.iter().zip(out.chunks_exact_mut(16)) {
            o.copy_from_slice(&pack(b));
        }
        Ok(out)
    }

    /// §7: one block.
    fn block(&self, nb: &Nb, tabs: &Tables, cur: &mut [Cur; 26]) -> Result<Blk> {
        let cb = cur[ID_CMD].byte()?;
        if cb & 0x80 != 0 {
            return Err(Error::Unsupported("XUBC7 P-frame blocks".into()));
        }
        let cmd = cb & 7;
        if cmd <= 2 && cb != cmd {
            return Err(bad("XUBC7 command byte"));
        }
        let left = nb.at(-1, 0);
        let upper = nb.at(0, -1);
        match cmd {
            0 => {
                return left
                    .copied()
                    .ok_or(bad("XUBC7 repeat of a missing left block"));
            }
            1 => {
                return upper
                    .copied()
                    .ok_or(bad("XUBC7 repeat of a missing upper block"));
            }
            2 => return self.solid(left, upper, cur, tabs),
            _ => {}
        }
        // §7.3 config.
        let mut b = if cmd == 3 {
            let c = cur[ID_CONFIG].byte()?;
            if c & 0xC0 != 0 {
                return Err(Error::Unsupported(format!("XUBC7 block config {c:#04x}")));
            }
            let (rot, sel) = ((c >> 3) & 3, (c >> 5) & 1);
            let mut b = Blk::new(c & 7);
            if b.dual() {
                b.rot = rot;
            } else if rot != 0 {
                return Err(bad("XUBC7 rotation on a single-plane mode"));
            }
            if b.mode == 4 {
                b.sel = sel;
            } else if sel != 0 {
                return Err(bad("XUBC7 index selector outside mode 4"));
            }
            b
        } else {
            let src = match cmd {
                4 => left,
                5 => upper,
                6 => nb.at(-1, -1),
                _ => nb.at(1, -1),
            };
            let src = src.ok_or(bad("XUBC7 config reused from a missing block"))?;
            let mut b = Blk::new(src.mode);
            b.rot = src.rot;
            b.sel = src.sel;
            b
        };
        // §7.4 partition.
        match b.parts {
            2 => {
                b.pattern = cur[ID_PART2].byte()?;
                if b.pattern >= 64 {
                    return Err(bad("XUBC7 partition index"));
                }
            }
            3 => {
                b.pattern = cur[ID_PART3].byte()?;
                if b.pattern >= 1 << b.pattern_bits {
                    return Err(bad("XUBC7 partition index"));
                }
            }
            _ => {}
        }
        // §7.5 endpoints.
        let ep_mode = (cb >> 3) & 7;
        if ep_mode == 0 {
            let r = &mut cur[ID_EP_RAW];
            for s in 0..b.parts as usize {
                for c in 0..b.comps() {
                    for e in 0..2 {
                        b.ep[s][e][c] = r.bits(b.ep_bits[usize::from(c == 3)])?;
                    }
                }
            }
            for p in 0..b.npbits as usize {
                b.pbits[p] = r.bits(1)?;
            }
        } else {
            let (pred, ps) = match ep_mode {
                1 => (left, 0),
                2 => (upper, 0),
                3 => (nb.at(-1, -1), 0),
                4 => (nb.at(1, -1), 0),
                5 => {
                    let i = cur[ID_EP_INDEX].byte()?;
                    if i >= 32 {
                        return Err(bad("XUBC7 endpoint block index"));
                    }
                    let (dx, dy) = XY_DELTAS[i as usize];
                    (nb.at(dx as isize, dy as isize), 0)
                }
                6 => (left, 1),
                _ => (upper, 1),
            };
            let pred = pred.ok_or(bad("XUBC7 endpoint prediction from a missing block"))?;
            if ps == 1 && pred.parts < 2 {
                return Err(bad("XUBC7 endpoint prediction from a missing subset"));
            }
            let pred = *pred;
            let fine = b.ep_bits[0] >= 6;
            let no_alpha6 = !self.alpha && b.mode == 6;
            let nres = if no_alpha6 { 6 } else { b.comps() * 2 };
            let pps = b.pbits_per_subset();
            for s in 0..b.parts as usize {
                let mut res = [0u8; 8];
                for i in (0..nres).step_by(2) {
                    let ch = i >> 1;
                    let c = &mut cur[if fine { ID_EP_FINE } else { ID_EP_COARSE } + ch];
                    res[i] = c.byte()?;
                    res[i + 1] = c.byte()?;
                }
                let mut rp = [0u8; 2];
                for v in rp.iter_mut().take(pps) {
                    *v = cur[ID_PBITS].bits(1)?;
                }
                endpoint_dpcm(&pred, ps, &mut b, s, &res, &rp);
                if no_alpha6 {
                    b.ep[0][0][3] = 127;
                    b.ep[0][1][3] = 127;
                }
            }
        }
        // §7.6 weight predictor.
        let pred_byte = cur[ID_WPRED].byte()?;
        if pred_byte >= 200 {
            return Err(bad("XUBC7 weight predictor"));
        }
        let (cand, amp) = (pred_byte % 50, pred_byte / 50);
        if cand == 0 && amp != 0 {
            return Err(bad("XUBC7 weight predictor"));
        }
        let dct = cb & 0x40 != 0;
        for p in 0..b.planes as usize {
            let mut pred = [0i32; 16];
            if cand != 0 {
                pred = predict(nb, cand, p)?;
                amplitude(&mut pred, amp);
            }
            let n = b.wbits[p];
            if dct {
                self.dct_plane(&mut b, p, &pred, pred_byte != 0, cur, tabs)?;
            } else {
                let mask = (1u8 << n) - 1;
                let c = &mut cur[if cand == 0 {
                    ID_RAW_WEIGHTS
                } else {
                    ID_DPCM_WEIGHTS + n as usize - 2
                }];
                let mut syms = [0u8; 16];
                if n == 2 {
                    for k in 0..4 {
                        let v = c.byte()?;
                        for j in 0..4 {
                            syms[k * 4 + j] = (v >> (2 * j)) & 3;
                        }
                    }
                } else {
                    for k in 0..8 {
                        let v = c.byte()?;
                        let (lo, hi) = (v & 15, v >> 4);
                        if n == 3 && (lo > 7 || hi > 7) {
                            return Err(bad("XUBC7 3-bit weight out of range"));
                        }
                        syms[k * 2] = lo;
                        syms[k * 2 + 1] = hi;
                    }
                }
                for i in 0..16 {
                    let pi = if cand != 0 {
                        tabs.quant_weight(pred[i], n)
                    } else {
                        0
                    };
                    b.w[p][i] = (syms[i] + pi) & mask;
                }
            }
        }
        Ok(b)
    }

    /// §7.9 + §8.2.4: one DCT-coded weight plane.
    fn dct_plane(
        &self,
        b: &mut Blk,
        p: usize,
        pred: &[i32; 16],
        signed_dc: bool,
        cur: &mut [Cur; 26],
        tabs: &Tables,
    ) -> Result<()> {
        let n = b.wbits[p];
        let mut dc = i64::from(cur[ID_DC].byte()?);
        if signed_dc && cur[ID_SIGNS].bits(1)? != 0 {
            dc = -dc;
        }
        let tab = quant_table(self.q, b.max_span(p), n);
        let mut dct = [0i64; 16];
        dct[0] = dc * (1 << (6 - n)) * ONE;
        let mut zig = 1usize;
        while zig < 16 {
            let r = cur[ID_AC].byte()?;
            if r == 0xFF {
                break;
            }
            let at = zig + r as usize;
            if at > 15 {
                return Err(bad("XUBC7 AC run past the block"));
            }
            let mag = cur[ID_AC].byte()?;
            if mag == 0 {
                return Err(bad("XUBC7 zero AC coefficient"));
            }
            let coef = if cur[ID_SIGNS].bits(1)? != 0 {
                -i64::from(mag)
            } else {
                i64::from(mag)
            };
            let (x, y) = ZIGZAG[at];
            let k = (x + y * 4) as usize;
            dct[k] = dequant_ac(coef, tab[k], x, y);
            zig = at + 1;
        }
        let out = inverse_2d(&dct);
        for i in 0..16 {
            let v = rshift(out[i] + i64::from(pred[i]) * ONE, 16);
            b.w[p][i] = tabs.quant_weight(v.clamp(0, 64) as i32, n);
        }
        Ok(())
    }

    /// §9: a solid block predicted from the neighbours' edge texels.
    fn solid(
        &self,
        left: Option<&Blk>,
        upper: Option<&Blk>,
        cur: &mut [Cur; 26],
        tabs: &Tables,
    ) -> Result<Blk> {
        let mut preds = [0u32; 4];
        let mut num = 0;
        if let Some(l) = left {
            for y in 0..4 {
                let px = l.texel(3 + y * 4);
                for c in 0..4 {
                    preds[c] += u32::from(px[c]);
                }
            }
            num += 4;
        }
        if let Some(u) = upper {
            for x in 0..4 {
                let px = u.texel(x + 12);
                for c in 0..4 {
                    preds[c] += u32::from(px[c]);
                }
            }
            num += 4;
        }
        // No neighbour: the prediction is zero.
        for v in preds.iter_mut() {
            *v = (*v + num / 2).checked_div(num).unwrap_or(0);
        }
        let mut color = [0u8, 0, 0, 255];
        for c in 0..if self.alpha { 4 } else { 3 } {
            color[c] = cur[ID_SOLID].byte()?.wrapping_add(preds[c] as u8);
        }
        let mut b = Blk::new(5);
        for c in 0..3 {
            let (lo, hi) = tabs.solid[color[c] as usize];
            b.ep[0][0][c] = lo;
            b.ep[0][1][c] = hi;
        }
        b.ep[0][0][3] = color[3];
        b.ep[0][1][3] = color[3];
        b.w[0] = [1; 16];
        Ok(b)
    }
}

// ---- §11 tiny mips and BC7 packing -------------------------------------------------------

fn tiny(d: &[u8], nbx: usize, nby: usize) -> Result<Vec<u8>> {
    if d.len() < 3 {
        return Err(Error::Truncated("XUBC7 tiny mip"));
    }
    let (tx, ty) = (d[1] as usize, d[2] as usize);
    if tx == 0 || ty == 0 {
        return Err(bad("XUBC7 tiny mip size"));
    }
    if d.len() as u64 != 3 + (tx * ty * 16) as u64 {
        return Err(Error::Truncated("XUBC7 tiny mip"));
    }
    if tx != nbx || ty != nby {
        return Err(bad("XUBC7 image size differs from the container's"));
    }
    let mut out = Vec::with_capacity(tx * ty * 16);
    for blk in d[3..].chunks_exact(16) {
        let b = unpack(blk.try_into().unwrap_or([0; 16]))?;
        out.extend_from_slice(&pack(&b));
    }
    Ok(out)
}

fn anchor(b: &Blk, s: usize) -> usize {
    let p = b.pattern as usize;
    match (b.parts, s) {
        (_, 0) => 0,
        (2, _) => ANCHOR2[p] as usize,
        (_, 1) => ANCHOR3_1[p] as usize,
        _ => ANCHOR3_2[p] as usize,
    }
}

fn is_anchor(b: &Blk, i: usize) -> bool {
    (0..b.parts as usize).any(|s| anchor(b, s) == i)
}

/// §11.1: a physical BC7 block to a logical one.
fn unpack(raw: [u8; 16]) -> Result<Blk> {
    if raw[0] == 0 {
        return Err(bad("XUBC7 tiny mip block without a mode"));
    }
    let v = u128::from_le_bytes(raw);
    let mode = raw[0].trailing_zeros() as u8;
    let mut pos = u32::from(mode) + 1;
    let mut fetch = |n: u8| -> Result<u8> {
        let n = u32::from(n);
        if pos + n > 128 {
            return Err(bad("XUBC7 tiny mip block"));
        }
        let r = ((v >> pos) & ((1u128 << n) - 1)) as u8;
        pos += n;
        Ok(r)
    };
    let mut b = Blk::new(mode);
    match mode {
        4 | 5 => {
            b.rot = fetch(2)?;
            b.sel = if mode == 4 { fetch(1)? } else { 0 };
            for c in 0..4 {
                for e in 0..2 {
                    b.ep[0][e][c] = fetch(b.ep_bits[usize::from(c == 3)])?;
                }
            }
            for p in 0..2 {
                for i in 0..16 {
                    b.w[p][i] = fetch(b.wbits[p] - u8::from(i == 0))?;
                }
            }
        }
        6 => {
            for c in 0..4 {
                b.ep[0][0][c] = fetch(7)?;
                b.ep[0][1][c] = fetch(7)?;
            }
            for p in 0..2 {
                b.pbits[p] = fetch(1)?;
            }
            for i in 0..16 {
                b.w[0][i] = fetch(if i == 0 { 3 } else { 4 })?;
            }
        }
        _ => {
            b.pattern = fetch(b.pattern_bits)?;
            for c in 0..b.comps() {
                for s in 0..b.parts as usize {
                    for e in 0..2 {
                        b.ep[s][e][c] = fetch(b.ep_bits[0])?;
                    }
                }
            }
            for p in 0..b.npbits as usize {
                b.pbits[p] = fetch(1)?;
            }
            for i in 0..16 {
                let short = is_anchor(&b, i);
                b.w[0][i] = fetch(b.wbits[0] - u8::from(short))?;
            }
        }
    }
    Ok(b)
}

/// §11.2: canonicalize (anchor weights' MSB clear) and pack a logical block.
fn pack(src: &Blk) -> [u8; 16] {
    let mut b = *src;
    if b.dual() {
        let vp = b.sel as usize;
        for p in [vp, 1 - vp] {
            let n = b.wbits[p];
            if b.w[p][0] & (1 << (n - 1)) != 0 {
                let chans = if p == vp { 0..3 } else { 3..4 };
                for c in chans {
                    let lo = b.ep[0][0][c];
                    b.ep[0][0][c] = b.ep[0][1][c];
                    b.ep[0][1][c] = lo;
                }
                for w in b.w[p].iter_mut() {
                    *w ^= (1 << n) - 1;
                }
            }
        }
    } else {
        let n = b.wbits[0];
        for s in 0..b.parts as usize {
            if b.w[0][anchor(&b, s)] & (1 << (n - 1)) != 0 {
                b.ep[s].swap(0, 1);
                if b.npbits != 0 && !b.shared {
                    b.pbits.swap(s * 2, s * 2 + 1);
                }
                for i in 0..16 {
                    if b.subset_of(i) == s {
                        b.w[0][i] ^= (1 << n) - 1;
                    }
                }
            }
        }
    }
    let mut v = 0u128;
    let mut pos = 0u32;
    let mut put = |x: u8, n: u8| {
        v |= u128::from(x & ((1u16 << n) - 1) as u8) << pos;
        pos += u32::from(n);
    };
    put(1 << b.mode, b.mode + 1);
    match b.mode {
        4 | 5 => {
            put(b.rot, 2);
            if b.mode == 4 {
                put(b.sel, 1);
            }
            for c in 0..4 {
                for e in 0..2 {
                    put(b.ep[0][e][c], b.ep_bits[usize::from(c == 3)]);
                }
            }
            for p in 0..2 {
                for i in 0..16 {
                    put(b.w[p][i], b.wbits[p] - u8::from(i == 0));
                }
            }
        }
        6 => {
            for c in 0..4 {
                put(b.ep[0][0][c], 7);
                put(b.ep[0][1][c], 7);
            }
            put(b.pbits[0], 1);
            put(b.pbits[1], 1);
            for i in 0..16 {
                put(b.w[0][i], if i == 0 { 3 } else { 4 });
            }
        }
        _ => {
            put(b.pattern, b.pattern_bits);
            for c in 0..b.comps() {
                for s in 0..b.parts as usize {
                    for e in 0..2 {
                        put(b.ep[s][e][c], b.ep_bits[0]);
                    }
                }
            }
            for p in 0..b.npbits as usize {
                put(b.pbits[p], 1);
            }
            for i in 0..16 {
                put(b.w[0][i], b.wbits[0] - u8::from(is_anchor(&b, i)));
            }
        }
    }
    v.to_le_bytes()
}

// Standard BC7 partition and anchor tables (BPTC), generated from the tables in bcn.rs:
// PART2 holds one subset bit per texel (raster order), PART3 two bits per texel.
static PART2: [u16; 64] = [
    0xcccc, 0x8888, 0xeeee, 0xecc8, 0xc880, 0xfeec, 0xfec8, 0xec80, 0xc800, 0xffec, 0xfe80, 0xe800,
    0xffe8, 0xff00, 0xfff0, 0xf000, 0xf710, 0x008e, 0x7100, 0x08ce, 0x008c, 0x7310, 0x3100, 0x8cce,
    0x088c, 0x3110, 0x6666, 0x366c, 0x17e8, 0x0ff0, 0x718e, 0x399c, 0xaaaa, 0xf0f0, 0x5a5a, 0x33cc,
    0x3c3c, 0x55aa, 0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc, 0x6996, 0xc33c, 0x9966, 0x0660,
    0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6, 0x639c, 0x9336, 0x9cc6, 0x817e, 0xe718,
    0xccf0, 0x0fcc, 0x7744, 0xee22,
];
static PART3: [u32; 64] = [
    0xaa685050, 0x6a5a5040, 0x5a5a4200, 0x5450a0a8, 0xa5a50000, 0xa0a05050, 0x5555a0a0, 0x5a5a5050,
    0xaa550000, 0xaa555500, 0xaaaa5500, 0x90909090, 0x94949494, 0xa4a4a4a4, 0xa9a59450, 0x2a0a4250,
    0xa5945040, 0x0a425054, 0xa5a5a500, 0x55a0a0a0, 0xa8a85454, 0x6a6a4040, 0xa4a45000, 0x1a1a0500,
    0x0050a4a4, 0xaaa59090, 0x14696914, 0x69691400, 0xa08585a0, 0xaa821414, 0x50a4a450, 0x6a5a0200,
    0xa9a58000, 0x5090a0a8, 0xa8a09050, 0x24242424, 0x00aa5500, 0x24924924, 0x24499224, 0x50a50a50,
    0x500aa550, 0xaaaa4444, 0x66660000, 0xa5a0a5a0, 0x50a050a0, 0x69286928, 0x44aaaa44, 0x66666600,
    0xaa444444, 0x54a854a8, 0x95809580, 0x96969600, 0xa85454a8, 0x80959580, 0xaa141414, 0x96960000,
    0xaaaa1414, 0xa05050a0, 0xa0a5a5a0, 0x96000000, 0x40804080, 0xa9a8a9a8, 0xaaaaaa44, 0x2a4a5254,
];
static ANCHOR2: [u8; 64] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15, 2, 8,
    2, 2, 8, 8, 2, 2, 15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, 6, 2, 6, 8, 15, 15, 2,
    2, 15, 15, 15, 15, 15, 2, 2, 15,
];
static ANCHOR3_1: [u8; 64] = [
    3, 3, 15, 15, 8, 3, 15, 15, 8, 8, 6, 6, 6, 5, 3, 3, 3, 3, 8, 15, 3, 3, 6, 10, 5, 8, 8, 6, 8, 5,
    15, 15, 8, 15, 3, 5, 6, 10, 8, 15, 15, 3, 15, 5, 15, 15, 15, 15, 3, 15, 5, 5, 5, 8, 5, 10, 5,
    10, 8, 13, 15, 12, 3, 3,
];
static ANCHOR3_2: [u8; 64] = [
    15, 8, 8, 3, 15, 15, 3, 8, 15, 15, 15, 15, 15, 15, 15, 8, 15, 8, 15, 3, 15, 8, 15, 8, 3, 15, 6,
    10, 15, 15, 10, 8, 15, 3, 15, 10, 10, 8, 9, 10, 6, 15, 8, 15, 3, 6, 6, 8, 15, 3, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 3, 15, 15, 8,
];

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");

    /// Level `k`'s slice of a single-image KTX2 file, with the level's size.
    fn level(d: &[u8], k: usize) -> (u32, u32, Vec<u8>) {
        let t = crate::ktx2::parse(d).unwrap();
        let l = &t.levels[k];
        let (o, n) = t.seek.as_ref().unwrap()[k];
        (l.width, l.height, d[l.range.clone()][o..o + n].to_vec())
    }

    /// Blocks that differ, and texels whose RGBA8 differ, between two BC7 block arrays.
    fn compare(ours: &[u8], want: &[u8]) -> (usize, usize) {
        assert_eq!(ours.len(), want.len());
        let (mut blocks, mut texels) = (0, 0);
        let (mut p, mut q) = ([0u8; 64], [0u8; 64]);
        for (x, y) in ours.chunks_exact(16).zip(want.chunks_exact(16)) {
            if x == y {
                continue;
            }
            blocks += 1;
            crate::bcn::bc7(x, &mut p, 16);
            crate::bcn::bc7(y, &mut q, 16);
            texels += p
                .chunks_exact(4)
                .zip(q.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
        }
        (blocks, texels)
    }

    /// §1.4's checks on the transcribed BC7 tables.
    #[test]
    fn tables_hold_the_bc7_invariants() {
        for p in 0..64 {
            assert!(ANCHOR2[p] != 0 && ANCHOR3_1[p] != 0 && ANCHOR3_2[p] != 0);
            assert_ne!(ANCHOR3_1[p], ANCHOR3_2[p]);
            assert_eq!(PART2[p] & 1, 0);
            assert_eq!(PART3[p] & 3, 0);
            assert_eq!((PART2[p] >> ANCHOR2[p]) & 1, 1);
            assert_eq!((PART3[p] >> (2 * ANCHOR3_1[p])) & 3, 1);
            assert_eq!((PART3[p] >> (2 * ANCHOR3_2[p])) & 3, 2);
            assert!((0..16).all(|i| (PART3[p] >> (2 * i)) & 3 < 3));
        }
    }

    /// §1.2's tie rule and §13's DC-only identity of the inverse DCT.
    #[test]
    fn weight_quantizer_and_dc_only_idct() {
        let t = Tables::new();
        assert_eq!(t.quant_weight(32, 2), 1);
        for n in 2..=4u8 {
            for dc in -255i64..=255 {
                let mut d = [0i64; 16];
                d[0] = dc * (1 << (6 - n)) * ONE;
                let want = dc * (1 << (6 - n)) * ONE / 4;
                assert!(inverse_2d(&d).iter().all(|&v| v == want), "n {n} dc {dc}");
            }
        }
    }

    /// One blob-container level (139×138, kodim26 from basis_universal v2.50, Apache-2.0) and
    /// one tiny-mip level (17×17): every block equals the opengl_basisu wheel's BC7 transcode.
    #[test]
    fn fixture_blocks_match_the_transcoder() {
        for (name, w, h) in [("xubc7_L3", 139, 138), ("xubc7_L6", 17, 17)] {
            let s = std::fs::read(format!("{DATA}{name}.bin")).unwrap();
            let want = std::fs::read(format!("{DATA}{name}.blocks")).unwrap();
            let ours = transcode(&s, w, h).unwrap();
            assert_eq!(compare(&ours, &want), (0, 0), "{name}");
            assert_eq!(ours, want, "{name}");
        }
    }

    /// Every level of the fetched kodim26 XUBC7 file against the wheel's blocks
    /// (kodim26_xubc7.L<k>.blocks). Skips when absent.
    #[test]
    fn corpus_levels_match_the_transcoder() {
        let Some(base) = std::env::var_os("SPECULAR_LARGE_CORPUS") else {
            return;
        };
        let dir = std::path::Path::new(&base).join("texture/basisu2");
        let Ok(d) = std::fs::read(dir.join("kodim26_xubc7.ktx2")) else {
            return;
        };
        let levels = crate::ktx2::parse(&d).unwrap().levels.len();
        for k in 0..levels {
            let Ok(want) = std::fs::read(dir.join(format!("kodim26_xubc7.L{k}.blocks"))) else {
                continue;
            };
            let (w, h, s) = level(&d, k);
            let t0 = std::time::Instant::now();
            let ours = transcode(&s, w, h).unwrap();
            let el = t0.elapsed();
            let (b, x) = compare(&ours, &want);
            eprintln!(
                "kodim26_xubc7 L{k} {w}x{h} ({:#04x}): {} blocks, {b} differ, {x} texels differ, {el:?}",
                s[0],
                ours.len() / 16
            );
            assert_eq!(x, 0, "level {k}");
        }
    }

    /// Truncated or damaged slices fail with an error, never a panic or a hang.
    #[test]
    fn damaged_slices_fail_cleanly() {
        for (name, w, h) in [("xubc7_L3", 139, 138), ("xubc7_L6", 17, 17)] {
            let s = std::fs::read(format!("{DATA}{name}.bin")).unwrap();
            for n in 0..s.len() {
                assert!(transcode(&s[..n], w, h).is_err(), "{name} cut at {n}");
            }
        }
        for (name, w, h, stride) in [("xubc7_L3", 139, 138, 23), ("xubc7_L6", 17, 17, 1)] {
            let s = std::fs::read(format!("{DATA}{name}.bin")).unwrap();
            for i in (0..s.len()).step_by(stride) {
                for flip in [0x01u8, 0x80, 0xFF] {
                    let mut d = s.clone();
                    d[i] ^= flip;
                    let _ = transcode(&d, w, h);
                }
            }
        }
        // Wrong container sizes, §3's over-read examples, stray markers.
        let s = std::fs::read(format!("{DATA}xubc7_L3.bin")).unwrap();
        assert!(transcode(&s, 4, 4).is_err());
        assert!(transcode(&s, 16384, 16384).is_err());
        assert!(transcode(&[0xB7, 0x02, 0x00, 0x01, 0xAA], 4, 4).is_err());
        assert!(transcode(&[0xB7, 0x01, 0x00, 0xFF], 4, 4).is_err());
        assert!(transcode(&[0xB8], 4, 4).is_err());
        assert!(matches!(
            transcode(&[0x42, 0, 0], 4, 4),
            Err(Error::Unsupported(_))
        ));
    }
}
