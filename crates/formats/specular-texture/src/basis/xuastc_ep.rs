//! XUASTC LDR endpoint helpers: ASTC LDR endpoint decoding to 8-bit RGBA, blue-contraction
//! tests, and `convert_endpoints_across_cems()` with its requantisation, written from
//! Binomial's published "XUASTC LDR - Endpoint Conversion for DPCM Prediction" (integer only).

// Index loops follow the specification's pseudo-code.
#![allow(clippy::needless_range_loop)]

use std::sync::OnceLock;

use super::astc_pack::{R256, cem_values, endpoint_quant, levels};
use crate::astc::{bit_transfer_signed, colour_unquant, ldr_endpoints};

/// Endpoint values of range `r`, unquantised to 0…255.
fn dequant(r: u8, e: &[u8]) -> [i32; 8] {
    let t = &colour_unquant()[usize::from(r)];
    let mut v = [0i32; 8];
    for (d, &s) in v.iter_mut().zip(e) {
        *d = i32::from(t[usize::from(s)]);
    }
    v
}

/// The 8-bit RGBA endpoint pair of LDR CEM `cem` from ISE values of range `r`.
pub(super) fn decode_endpoints(cem: u8, e: &[u8], r: u8) -> ([i32; 4], [i32; 4]) {
    let n = cem_values(cem).min(e.len());
    let v = dequant(r, &e[..n]);
    ldr_endpoints(cem, &v[..n.max(2)])
}

pub(super) fn supports_bc(cem: u8) -> bool {
    matches!(cem, 8 | 9 | 12 | 13)
}

fn has_alpha(cem: u8) -> bool {
    matches!(cem, 4 | 10 | 12 | 13)
}

fn base_without_alpha(cem: u8) -> u8 {
    match cem {
        4 => 0,
        10 => 6,
        12 => 8,
        13 => 9,
        c => c,
    }
}

/// Whether endpoints of CEM 8/9/12/13 decode with blue contraction.
pub(super) fn used_bc(cem: u8, e: &[u8], r: u8) -> bool {
    let n = cem_values(cem).min(e.len());
    let v = dequant(r, &e[..n]);
    match cem {
        8 | 12 => v[1] + v[3] + v[5] < v[0] + v[2] + v[4],
        9 | 13 => {
            let (d1, _) = bit_transfer_signed(v[1], v[0]);
            let (d3, _) = bit_transfer_signed(v[3], v[2]);
            let (d5, _) = bit_transfer_signed(v[5], v[4]);
            d1 + d3 + d5 < 0
        }
        _ => false,
    }
}

/// ASTC's `bit_transfer_signed` inverse: (offset byte, base byte) for a delta in −32…31 and
/// an 8-bit base.
fn bit_transfer_enc(delta: i32, base: i32) -> (i32, i32) {
    (((delta & 0x3f) << 1) | (base & 0x80), (base << 1) & 0xff)
}

/// The ISE value one rank up or down (clamped) in range `r`.
pub(super) fn step_rank(r: u8, ise: u8, delta: i32) -> u8 {
    let q = &endpoint_quant()[usize::from(r)];
    let rank = i32::from(q.ise_to_rank[usize::from(ise)]) + delta;
    let rank = rank.clamp(0, levels(r) as i32 - 1);
    q.rank_to_ise[rank as usize]
}

fn quant_preserve2(r: u8, v: u8) -> u8 {
    if r == R256 {
        v
    } else {
        super::hdr6x6i::preserve_tables()[usize::from(r)][0][usize::from(v)]
    }
}

/// `g_base_ofs_nudges[range][negative][ise]`: the nearest ISE value whose base+offset delta
/// moves the asked way without changing the transferred bit.
fn nudges() -> &'static Vec<[Vec<u8>; 2]> {
    static T: OnceLock<Vec<[Vec<u8>; 2]>> = OnceLock::new();
    T.get_or_init(|| {
        (0..21u8)
            .map(|r| {
                if r < 4 {
                    return [Vec::new(), Vec::new()];
                }
                let tab = &endpoint_quant()[usize::from(r)].ise_to_val;
                let n = levels(r);
                let split = |i: usize| bit_transfer_signed(i32::from(tab[i]), 0);
                let dir = |neg: bool| {
                    (0..n)
                        .map(|cur| {
                            let (ca, cb) = split(cur);
                            let mut best = (i32::MAX, cur);
                            for t in 0..n {
                                let (ta, tb) = split(t);
                                if tb != cb || ta == ca || (neg && ta > ca) || (!neg && ta < ca) {
                                    continue;
                                }
                                let e = (ta - ca).abs();
                                if e < best.0 {
                                    best = (e, t);
                                }
                            }
                            best.1 as u8
                        })
                        .collect()
                };
                [dir(false), dir(true)]
            })
            .collect()
    })
}

/// Base+offset delta sum sign after quantisation (blue contraction when negative).
fn base_ofs_bc(r: u8, e: &[u8]) -> bool {
    let v = dequant(r, &e[..6]);
    let (d1, _) = bit_transfer_signed(v[1], v[0]);
    let (d3, _) = bit_transfer_signed(v[3], v[2]);
    let (d5, _) = bit_transfer_signed(v[5], v[4]);
    d1 + d3 + d5 < 0
}

/// `requantize_ise_endpoints()`: from range `from` to range `to`, keeping blue contraction and
/// base+offset MSBs.
pub(super) fn requantize(cem: u8, from: u8, src: &[u8], to: u8, dst: &mut [u8]) -> bool {
    let n = cem_values(cem);
    if src.len() < n || dst.len() < n {
        return false;
    }
    if from == to {
        dst[..n].copy_from_slice(&src[..n]);
        return true;
    }
    let mut v = [0u8; 8];
    for i in 0..n {
        v[i] = if from == R256 {
            src[i]
        } else {
            colour_unquant()[usize::from(from)][usize::from(src[i])]
        };
    }
    if to == R256 {
        dst[..n].copy_from_slice(&v[..n]);
        return true;
    }
    let q = &endpoint_quant()[usize::from(to)];
    let quant = |x: u8| q.val_to_ise[usize::from(x)];
    let deq = |x: u8| u32::from(q.ise_to_val[usize::from(x)]);
    match cem {
        9 | 13 => {
            for i in 0..n {
                dst[i] = if i & 1 == 1 {
                    quant_preserve2(to, v[i])
                } else {
                    quant(v[i])
                };
            }
            let src_bc = used_bc(cem, src, from);
            let mut bc = base_ofs_bc(to, dst);
            if src_bc != bc {
                let neg = !bc;
                let tab = &nudges()[usize::from(to)][usize::from(neg)];
                for rover in (2usize..).take(5) {
                    for j in 0..3 {
                        let idx = 1 + ((rover + j) % 3) * 2;
                        let nv = tab[usize::from(dst[idx])];
                        if nv != dst[idx] {
                            dst[idx] = nv;
                            break;
                        }
                    }
                    bc = base_ofs_bc(to, dst);
                    if src_bc == bc {
                        break;
                    }
                }
            }
        }
        8 | 12 => {
            let s0 = u32::from(v[0]) + u32::from(v[2]) + u32::from(v[4]);
            let s1 = u32::from(v[1]) + u32::from(v[3]) + u32::from(v[5]);
            let orig_bc = s1 < s0;
            for i in 0..n {
                dst[i] = quant(v[i]);
            }
            let d0 = deq(dst[0]) + deq(dst[2]) + deq(dst[4]);
            let d1 = deq(dst[1]) + deq(dst[3]) + deq(dst[5]);
            if orig_bc != (d1 < d0) {
                if d0 == d1 {
                    if d1 != 0 {
                        for i in 0..3 {
                            let nv = step_rank(to, dst[1 + i * 2], -1);
                            if nv != dst[1 + i * 2] {
                                dst[1 + i * 2] = nv;
                                break;
                            }
                        }
                    } else {
                        for i in 0..3 {
                            let nv = step_rank(to, dst[i * 2], 1);
                            if nv != dst[i * 2] {
                                dst[i * 2] = nv;
                                break;
                            }
                        }
                    }
                } else {
                    for i in (0..n).step_by(2) {
                        dst.swap(i, i + 1);
                    }
                }
            }
        }
        _ => {
            for i in 0..n {
                dst[i] = quant(v[i]);
            }
        }
    }
    true
}

/// R' = 2R − B, G' = 2G − B, clamped (flagging the clamp).
fn blue_contract_enc(c: [i32; 4], clamped: &mut bool, b: i32) -> [i32; 4] {
    let (r, g) = (c[0] * 2 - b, c[1] * 2 - b);
    if !(0..=255).contains(&r) || !(0..=255).contains(&g) {
        *clamped = true;
    }
    [r.clamp(0, 255), g.clamp(0, 255), c[2], c[3]]
}

fn pack_base_offset(cem: u8, to: u8, out: &mut [u8], l: [i32; 4], h: [i32; 4], bc: bool) -> bool {
    let (mut pl, mut ph) = (l, h);
    let mut clamped = false;
    if bc {
        let el = blue_contract_enc(pl, &mut clamped, pl[2]);
        let eh = blue_contract_enc(ph, &mut clamped, ph[2]);
        ph = el;
        pl = eh;
    }
    let (mut dr, mut dg, mut db, mut da) = (0, 0, 0, 0);
    let mut low = -32;
    for pass in 0..4 {
        dr = (ph[0] - pl[0]).clamp(low, 31);
        dg = (ph[1] - pl[1]).clamp(low, 31);
        db = (ph[2] - pl[2]).clamp(low, 31);
        da = (ph[3] - pl[3]).clamp(low, 31);
        let s = dr + dg + db;
        if (s < 0) == bc {
            break;
        }
        if s == 0 {
            if db > -32 {
                db -= 1;
            } else if dr > -32 {
                dr -= 1;
            } else if dg > -32 {
                dg -= 1;
            }
            break;
        }
        if pass == 3 {
            break;
        }
        if pass == 1 {
            low = -31;
        }
        std::mem::swap(&mut pl, &mut ph);
    }
    let (v1, v0) = bit_transfer_enc(dr, pl[0]);
    let (v3, v2) = bit_transfer_enc(dg, pl[1]);
    let (v5, v4) = bit_transfer_enc(db, pl[2]);
    let mut e8 = [
        v0 as u8, v1 as u8, v2 as u8, v3 as u8, v4 as u8, v5 as u8, 0, 0,
    ];
    if has_alpha(cem) {
        let (v7, v6) = bit_transfer_enc(da, pl[3]);
        e8[6] = v6 as u8;
        e8[7] = v7 as u8;
    }
    requantize(cem, R256, &e8, to, out)
}

/// `convert_endpoints_across_cems()` (no forced repack, no automatic blue-contraction
/// disabling): the predictor's endpoints (`prev_cem`, range `prev_r`) as `dst_cem` in range
/// `dst_r`.
pub(super) fn convert(
    prev_cem: u8,
    prev_r: u8,
    prev: &[u8],
    dst_cem: u8,
    dst_r: u8,
    out: &mut [u8],
    bc: bool,
) -> bool {
    let n = cem_values(dst_cem);
    if prev.len() < cem_values(prev_cem) || out.len() < n {
        return false;
    }
    if prev_cem == dst_cem {
        return requantize(prev_cem, prev_r, prev, dst_r, out);
    }
    let (pb, db) = (base_without_alpha(prev_cem), base_without_alpha(dst_cem));
    let q = &endpoint_quant()[usize::from(dst_r)];
    if pb == db && !has_alpha(dst_cem) {
        return requantize(pb, prev_r, prev, dst_r, out);
    }
    if pb == db {
        if !requantize(pb, prev_r, prev, dst_r, out) {
            return false;
        }
        let a = if dst_r == R256 {
            255
        } else {
            q.val_to_ise[255]
        };
        match dst_cem {
            4 => out[2..4].fill(a),
            12 => out[6..8].fill(a),
            10 => out[4..6].fill(a),
            13 => {
                out[6] = a;
                out[7] = if dst_r == R256 {
                    128
                } else {
                    q.val_to_ise[128]
                };
            }
            _ => return false,
        }
        return true;
    }
    let (l, h) = decode_endpoints(prev_cem, prev, prev_r);
    let mut e8 = [0u8; 8];
    let lum = |c: [i32; 4]| ((c[0] + c[1] + c[2] + 1) / 3) as u8;
    match dst_cem {
        0 | 4 => {
            e8[0] = lum(l);
            e8[1] = lum(h);
            if dst_cem == 4 {
                e8[2] = l[3] as u8;
                e8[3] = h[3] as u8;
            }
            if !matches!(prev_cem, 0 | 4) && e8[0] > e8[1] {
                e8.swap(0, 1);
                e8.swap(2, 3);
            }
            requantize(dst_cem, R256, &e8, dst_r, out)
        }
        8 | 12 => {
            for c in 0..3 {
                e8[c * 2] = l[c] as u8;
                e8[c * 2 + 1] = h[c] as u8;
            }
            if dst_cem == 12 {
                e8[6] = l[3] as u8;
                e8[7] = h[3] as u8;
            }
            if bc {
                let rq = |b: i32| {
                    if dst_r == R256 {
                        b
                    } else {
                        i32::from(q.ise_to_val[usize::from(q.val_to_ise[b as usize])])
                    }
                };
                let mut clamped = false;
                let el = blue_contract_enc(l, &mut clamped, rq(l[2]));
                let eh = blue_contract_enc(h, &mut clamped, rq(h[2]));
                for c in 0..3 {
                    e8[c * 2] = eh[c] as u8;
                    e8[c * 2 + 1] = el[c] as u8;
                }
                if dst_cem == 12 {
                    e8[6] = h[3] as u8;
                    e8[7] = l[3] as u8;
                }
            }
            let s0 = u32::from(e8[0]) + u32::from(e8[2]) + u32::from(e8[4]);
            let s1 = u32::from(e8[1]) + u32::from(e8[3]) + u32::from(e8[5]);
            if (s1 < s0) != bc {
                if s0 == s1 {
                    if s1 != 0 {
                        for i in 0..3 {
                            if e8[1 + i * 2] > 0 {
                                e8[1 + i * 2] -= 1;
                                break;
                            }
                        }
                    } else {
                        for i in 0..3 {
                            if e8[i * 2] < 255 {
                                e8[i * 2] += 1;
                                break;
                            }
                        }
                    }
                } else {
                    for i in (0..n).step_by(2) {
                        e8.swap(i, i + 1);
                    }
                }
            }
            requantize(dst_cem, R256, &e8, dst_r, out)
        }
        6 | 10 => {
            let (mut lc, mut hc) = (l, h);
            if !matches!(prev_cem, 6 | 10) && lc[0] + lc[1] + lc[2] > hc[0] + hc[1] + hc[2] {
                std::mem::swap(&mut lc, &mut hc);
            }
            e8[0] = hc[0] as u8;
            e8[1] = hc[1] as u8;
            e8[2] = hc[2] as u8;
            let id = lc[0] * hc[0] + lc[1] * hc[1] + lc[2] * hc[2];
            let nrm = hc[0] * hc[0] + hc[1] * hc[1] + hc[2] * hc[2];
            const IMAX: i32 = (1024 * 255) / 256;
            let s = if nrm > 0 { (id * 1024) / nrm } else { IMAX };
            let s = (s.clamp(0, IMAX) + 2) >> 2;
            e8[3] = s.clamp(0, 255) as u8;
            if dst_cem == 10 {
                e8[4] = lc[3] as u8;
                e8[5] = hc[3] as u8;
                if !matches!(prev_cem, 6 | 10) && e8[4] > e8[5] {
                    e8.swap(4, 5);
                }
            }
            requantize(dst_cem, R256, &e8, dst_r, out)
        }
        9 | 13 => pack_base_offset(dst_cem, dst_r, out, l, h, bc),
        _ => false,
    }
}
