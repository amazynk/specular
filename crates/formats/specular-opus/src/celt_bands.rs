//! CELT shape decoding (RFC 6716 section 4.3.4): band splitting, theta, PVQ, spreading,
//! time-frequency changes, stereo, folding and anti-collapse.

use crate::celt_alloc::{
    BITRES, NB_EBANDS, bits2pulses, cache, eb, get_pulses, log_n, pulses2bits,
};
use crate::range::{RangeDecoder, ilog};

const QTHETA_OFFSET: i32 = 4;
const QTHETA_OFFSET_TWOPHASE: i32 = 16;
pub(crate) const SPREAD_NONE: u32 = 0;
pub(crate) const SPREAD_AGGRESSIVE: u32 = 3;
/// Largest band (band 20 at 20 ms).
const MAX_BAND: usize = 176;

#[inline]
pub(crate) fn lcg_rand(seed: u32) -> u32 {
    seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

#[inline]
fn frac_mul16(a: i32, b: i32) -> i32 {
    (16384 + i32::from(a as i16) * i32::from(b as i16)) >> 15
}

/// cos(pi/2 * x / 16384) in Q15, bit-exact.
fn bitexact_cos(x: i32) -> i32 {
    let tmp = (4096 + x * x) >> 13;
    let x2 = tmp;
    let x2 = (32767 - x2) + frac_mul16(x2, -7651 + frac_mul16(x2, 8277 + frac_mul16(-626, x2)));
    1 + x2
}

/// log2(sin/cos) in Q11, bit-exact.
fn bitexact_log2tan(isin: i32, icos: i32) -> i32 {
    let lc = ilog(icos as u32);
    let ls = ilog(isin as u32);
    let icos = icos << (15 - lc);
    let isin = isin << (15 - ls);
    (ls - lc) * (1 << 11) + frac_mul16(isin, frac_mul16(isin, -2597) + 7932)
        - frac_mul16(icos, frac_mul16(icos, -2597) + 7932)
}

fn isqrt32(v: u32) -> u32 {
    let mut r = (v as f64).sqrt() as u32;
    while u64::from(r) * u64::from(r) > u64::from(v) {
        r -= 1;
    }
    while u64::from(r + 1) * u64::from(r + 1) <= u64::from(v) {
        r += 1;
    }
    r
}

fn renormalise(x: &mut [f32], gain: f32) {
    let e: f32 = 1e-15 + x.iter().map(|v| v * v).sum::<f32>();
    let g = gain / e.sqrt();
    for v in x.iter_mut() {
        *v *= g;
    }
}

fn haar1(x: &mut [f32], n0: usize, stride: usize) {
    let n0 = n0 >> 1;
    let s = std::f32::consts::FRAC_1_SQRT_2;
    for i in 0..stride {
        for j in 0..n0 {
            let a = s * x[stride * 2 * j + i];
            let b = s * x[stride * (2 * j + 1) + i];
            x[stride * 2 * j + i] = a + b;
            x[stride * (2 * j + 1) + i] = a - b;
        }
    }
}

const ORDERY: [usize; 30] = [
    1, 0, 3, 0, 2, 1, 7, 0, 4, 3, 6, 1, 5, 2, 15, 0, 8, 7, 12, 3, 11, 4, 14, 1, 9, 6, 13, 2, 10, 5,
];

fn deinterleave_hadamard(x: &mut [f32], n0: usize, stride: usize, hadamard: bool) {
    let n = n0 * stride;
    let mut tmp = [0f32; MAX_BAND];
    if hadamard {
        let ord = &ORDERY[stride - 2..];
        for i in 0..stride {
            for j in 0..n0 {
                tmp[ord[i] * n0 + j] = x[j * stride + i];
            }
        }
    } else {
        for i in 0..stride {
            for j in 0..n0 {
                tmp[i * n0 + j] = x[j * stride + i];
            }
        }
    }
    x[..n].copy_from_slice(&tmp[..n]);
}

fn interleave_hadamard(x: &mut [f32], n0: usize, stride: usize, hadamard: bool) {
    let n = n0 * stride;
    let mut tmp = [0f32; MAX_BAND];
    if hadamard {
        let ord = &ORDERY[stride - 2..];
        for i in 0..stride {
            for j in 0..n0 {
                tmp[j * stride + i] = x[ord[i] * n0 + j];
            }
        }
    } else {
        for i in 0..stride {
            for j in 0..n0 {
                tmp[j * stride + i] = x[i * n0 + j];
            }
        }
    }
    x[..n].copy_from_slice(&tmp[..n]);
}

fn exp_rotation1(x: &mut [f32], len: usize, stride: usize, c: f32, s: f32) {
    let ms = -s;
    if len <= stride {
        return;
    }
    for i in 0..len - stride {
        let x1 = x[i];
        let x2 = x[i + stride];
        x[i + stride] = c * x2 + s * x1;
        x[i] = c * x1 + ms * x2;
    }
    if len > 2 * stride {
        for i in (0..=len - 2 * stride - 1).rev() {
            let x1 = x[i];
            let x2 = x[i + stride];
            x[i + stride] = c * x2 + s * x1;
            x[i] = c * x1 + ms * x2;
        }
    }
}

/// Spreading rotation (decoder direction).
fn exp_rotation(x: &mut [f32], len: usize, stride: usize, k: i32, spread: u32) {
    const FACTOR: [i32; 3] = [15, 10, 5];
    if 2 * k >= len as i32 || spread == SPREAD_NONE {
        return;
    }
    let factor = FACTOR[spread as usize - 1];
    let gain = len as f32 / (len as i32 + factor * k) as f32;
    let theta = 0.5 * gain * gain;
    let c = (0.5 * std::f32::consts::PI * theta).cos();
    let s = (0.5 * std::f32::consts::PI * (1.0 - theta)).cos();
    let mut stride2 = 0usize;
    if len >= 8 * stride {
        stride2 = 1;
        while (stride2 * stride2 + stride2) * stride + (stride >> 2) < len {
            stride2 += 1;
        }
    }
    let l = len / stride;
    for i in 0..stride {
        let xs = &mut x[i * l..(i + 1) * l];
        if stride2 != 0 {
            exp_rotation1(xs, l, stride2, s, c);
        }
        exp_rotation1(xs, l, 1, c, s);
    }
}

/// Decodes a PVQ codeword of `k` pulses in `n` dimensions into `y`; returns the squared norm.
fn decode_pulses(
    y: &mut [i32],
    n: usize,
    k: usize,
    ec: &mut RangeDecoder,
    v: &mut Vec<u64>,
) -> f32 {
    // V(n, k) table: v[nn * (k + 1) + kk].
    let w = k + 1;
    v.clear();
    v.resize((n + 1) * w, 0);
    v[0] = 1;
    for nn in 1..=n {
        v[nn * w] = 1;
        for kk in 1..=k {
            let a = v[(nn - 1) * w + kk];
            let b = v[nn * w + kk - 1];
            let c = v[(nn - 1) * w + kk - 1];
            v[nn * w + kk] = (a + b + c).min(1 << 40);
        }
    }
    let total = v[n * w + k].min(u64::from(u32::MAX)) as u32;
    let mut i = u64::from(ec.uint(total.max(2)));
    let mut kk = k;
    let mut ryy = 0f32;
    for j in 0..n {
        let rem = n - j;
        let mut p = (v[(rem - 1) * w + kk] + v[rem * w + kk]) / 2;
        let sgn = if i < p {
            1
        } else {
            i -= p;
            -1
        };
        let k0 = kk;
        p -= v[(rem - 1) * w + kk];
        while p > i && kk > 0 {
            kk -= 1;
            p -= v[(rem - 1) * w + kk];
        }
        let val = sgn * (k0 - kk) as i32;
        y[j] = val;
        ryy += (val * val) as f32;
        i -= p.min(i);
    }
    ryy
}

fn extract_collapse_mask(iy: &[i32], n: usize, b: usize) -> u32 {
    if b <= 1 {
        return 1;
    }
    let n0 = n / b;
    let mut mask = 0u32;
    for i in 0..b {
        if iy[i * n0..(i + 1) * n0].iter().any(|&v| v != 0) {
            mask |= 1 << i;
        }
    }
    mask
}

/// Shared state of the band decoding loop.
pub(crate) struct BandCtx<'a, 'b> {
    pub(crate) ec: &'a mut RangeDecoder<'b>,
    pub(crate) band: usize,
    pub(crate) intensity: usize,
    pub(crate) spread: u32,
    pub(crate) tf_change: i32,
    pub(crate) remaining_bits: i32,
    pub(crate) seed: u32,
    pub(crate) avoid_split_noise: bool,
    pub(crate) disable_inv: bool,
    pub(crate) vtab: Vec<u64>,
    pub(crate) iy: Vec<i32>,
}

struct Split {
    inv: bool,
    imid: i32,
    iside: i32,
    delta: i32,
    itheta: i32,
    qalloc: i32,
}

fn compute_qn(n: i32, b: i32, offset: i32, pulse_cap: i32, stereo: bool) -> i32 {
    const EXP2_TABLE8: [i32; 8] = [16384, 17866, 19483, 21247, 23170, 25267, 27554, 30048];
    let mut n2 = 2 * n - 1;
    if stereo && n == 2 {
        n2 -= 1;
    }
    let mut qb = (b + n2 * offset) / n2;
    qb = qb.min(b - pulse_cap - (4 << BITRES));
    qb = qb.min(8 << BITRES);
    if qb < (1 << BITRES >> 1) {
        1
    } else {
        let qn = EXP2_TABLE8[(qb & 7) as usize] >> (14 - (qb >> BITRES));
        (qn + 1) >> 1 << 1
    }
}

impl BandCtx<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn compute_theta(
        &mut self,
        n: usize,
        b: &mut i32,
        bb: usize,
        b0: usize,
        lm: i32,
        stereo: bool,
        fill: &mut u32,
    ) -> Split {
        let i = self.band;
        let pulse_cap = log_n(i) + lm * (1 << BITRES);
        let offset = (pulse_cap >> 1)
            - if stereo && n == 2 {
                QTHETA_OFFSET_TWOPHASE
            } else {
                QTHETA_OFFSET
            };
        let mut qn = compute_qn(n as i32, *b, offset, pulse_cap, stereo);
        if stereo && i >= self.intensity {
            qn = 1;
        }
        let tell = self.ec.tell_frac();
        let mut itheta = 0i32;
        let mut inv = false;
        if qn != 1 {
            if stereo && n > 2 {
                let p0 = 3u32;
                let x0 = (qn / 2) as u32;
                let ft = p0 * (x0 + 1) + x0;
                let (fs, ext) = self.ec.decode(ft);
                let x = if fs < (x0 + 1) * p0 {
                    fs / p0
                } else {
                    x0 + 1 + (fs - (x0 + 1) * p0)
                };
                let (fl, fh) = if x <= x0 {
                    (p0 * x, p0 * (x + 1))
                } else {
                    ((x - 1 - x0) + (x0 + 1) * p0, (x - x0) + (x0 + 1) * p0)
                };
                self.ec.update(ext, fl, fh, ft);
                itheta = x as i32;
            } else if b0 > 1 || stereo {
                itheta = self.ec.uint(qn as u32 + 1) as i32;
            } else {
                let half = (qn >> 1) as u32;
                let ft = (half + 1) * (half + 1);
                let (fm, ext) = self.ec.decode(ft);
                let (fl, fs);
                if fm < (half * (half + 1)) >> 1 {
                    itheta = ((isqrt32(8 * fm + 1) - 1) >> 1) as i32;
                    fs = itheta as u32 + 1;
                    fl = (itheta as u32 * (itheta as u32 + 1)) >> 1;
                } else {
                    let q1 = qn as u32 + 1;
                    itheta = ((2 * q1 - isqrt32(8 * (ft - fm - 1) + 1)) >> 1) as i32;
                    fs = q1 - itheta as u32;
                    fl = ft - (((q1 - itheta as u32) * (q1 + 1 - itheta as u32)) >> 1);
                }
                self.ec.update(ext, fl, fl + fs, ft);
            }
            itheta = ((itheta as u32 * 16384) / qn as u32) as i32;
        } else if stereo {
            if *b > 2 << BITRES && self.remaining_bits > 2 << BITRES {
                inv = self.ec.bit_logp(2);
            }
            if self.disable_inv {
                inv = false;
            }
            itheta = 0;
        }
        let qalloc = self.ec.tell_frac() - tell;
        *b -= qalloc;
        let (imid, iside, delta);
        if itheta == 0 {
            imid = 32767;
            iside = 0;
            *fill &= (1u32 << bb) - 1;
            delta = -16384;
        } else if itheta == 16384 {
            imid = 0;
            iside = 32767;
            *fill &= ((1u32 << bb) - 1) << bb;
            delta = 16384;
        } else {
            imid = bitexact_cos(itheta);
            iside = bitexact_cos(16384 - itheta);
            delta = frac_mul16(((n as i32) - 1) << 7, bitexact_log2tan(iside, imid));
        }
        Split {
            inv,
            imid,
            iside,
            delta,
            itheta,
            qalloc,
        }
    }

    /// Decodes the PVQ shape of `x` (alg_unquant).
    fn alg_unquant(&mut self, x: &mut [f32], n: usize, k: usize, b: usize, gain: f32) -> u32 {
        let mut iy = std::mem::take(&mut self.iy);
        iy.clear();
        iy.resize(n, 0);
        let ryy = decode_pulses(&mut iy, n, k, self.ec, &mut self.vtab);
        let g = gain / ryy.sqrt();
        for (xv, &yv) in x[..n].iter_mut().zip(iy.iter()) {
            *xv = g * yv as f32;
        }
        exp_rotation(x, n, b, k as i32, self.spread);
        let mask = extract_collapse_mask(&iy, n, b);
        self.iy = iy;
        mask
    }

    #[allow(clippy::too_many_arguments)]
    fn quant_partition(
        &mut self,
        x: &mut [f32],
        n: usize,
        mut b: i32,
        mut bb: usize,
        lowband: Option<&[f32]>,
        mut lm: i32,
        gain: f32,
        mut fill: u32,
    ) -> u32 {
        let i = self.band;
        let c = cache((lm + 1) as usize, i);
        let b0 = bb;
        if lm != -1 && b > i32::from(c[usize::from(c[0])]) + 12 && n > 2 {
            let n = n >> 1;
            let (xl, yl) = x.split_at_mut(n);
            lm -= 1;
            if bb == 1 {
                fill = (fill & 1) | (fill << 1);
            }
            bb = (bb + 1) >> 1;
            let sp = self.compute_theta(n, &mut b, bb, b0, lm, false, &mut fill);
            let mid = sp.imid as f32 / 32768.0;
            let side = sp.iside as f32 / 32768.0;
            let mut delta = sp.delta;
            if b0 > 1 && (sp.itheta & 0x3fff) != 0 {
                if sp.itheta > 8192 {
                    delta -= delta >> (4 - lm);
                } else {
                    delta = (delta + ((n as i32) << BITRES >> (5 - lm))).min(0);
                }
            }
            let mbits = b.min((b - delta) / 2).max(0);
            let mut sbits = b - mbits;
            let mut mbits = mbits;
            self.remaining_bits -= sp.qalloc;
            let next_lowband2 = lowband.map(|l| &l[n..]);
            let mut rebalance = self.remaining_bits;
            let mut cm;
            if mbits >= sbits {
                cm = self.quant_partition(xl, n, mbits, bb, lowband, lm, gain * mid, fill);
                rebalance = mbits - (rebalance - self.remaining_bits);
                if rebalance > 3 << BITRES && sp.itheta != 0 {
                    sbits += rebalance - (3 << BITRES);
                }
                cm |= self.quant_partition(
                    yl,
                    n,
                    sbits,
                    bb,
                    next_lowband2,
                    lm,
                    gain * side,
                    fill >> bb,
                ) << (b0 >> 1);
            } else {
                cm = self.quant_partition(
                    yl,
                    n,
                    sbits,
                    bb,
                    next_lowband2,
                    lm,
                    gain * side,
                    fill >> bb,
                ) << (b0 >> 1);
                rebalance = sbits - (rebalance - self.remaining_bits);
                if rebalance > 3 << BITRES && sp.itheta != 16384 {
                    mbits += rebalance - (3 << BITRES);
                }
                cm |= self.quant_partition(xl, n, mbits, bb, lowband, lm, gain * mid, fill);
            }
            return cm;
        }
        // No split.
        let mut q = bits2pulses(i, lm, b);
        let mut curr_bits = pulses2bits(i, lm, q);
        self.remaining_bits -= curr_bits;
        while self.remaining_bits < 0 && q > 0 {
            self.remaining_bits += curr_bits;
            q -= 1;
            curr_bits = pulses2bits(i, lm, q);
            self.remaining_bits -= curr_bits;
        }
        if q != 0 {
            let k = get_pulses(q) as usize;
            return self.alg_unquant(x, n, k, bb, gain);
        }
        // No pulse: fill the band anyway.
        let cm_mask = ((1u64 << bb) - 1) as u32;
        fill &= cm_mask;
        if fill == 0 {
            x[..n].fill(0.0);
            return 0;
        }
        let cm;
        match lowband {
            None => {
                for v in x[..n].iter_mut() {
                    self.seed = lcg_rand(self.seed);
                    *v = ((self.seed as i32) >> 20) as f32;
                }
                cm = cm_mask;
            }
            Some(lb) => {
                for (v, &l) in x[..n].iter_mut().zip(lb.iter()) {
                    self.seed = lcg_rand(self.seed);
                    let tmp = if self.seed & 0x8000 != 0 {
                        1.0 / 256.0
                    } else {
                        -1.0 / 256.0
                    };
                    *v = l + tmp;
                }
                cm = fill;
            }
        }
        renormalise(&mut x[..n], gain);
        cm
    }

    /// Decodes one band of one channel (or the mid of a stereo band).
    #[allow(clippy::too_many_arguments)]
    fn quant_band(
        &mut self,
        x: &mut [f32],
        n: usize,
        b: i32,
        mut bb: usize,
        lowband_in: Option<&[f32]>,
        lm: i32,
        lowband_out: Option<&mut [f32]>,
        gain: f32,
        mut fill: u32,
    ) -> u32 {
        let n0 = n;
        let mut n_b = n;
        let b0_in = bb;
        let long_blocks = b0_in == 1;
        let mut tf_change = self.tf_change;
        n_b /= bb;
        if n == 1 {
            return self.quant_band_n1(x, None, lowband_out);
        }
        let recombine = if tf_change > 0 { tf_change as usize } else { 0 };
        let mut lb_buf = [0f32; MAX_BAND];
        let has_lowband = lowband_in.is_some();
        if let Some(l) = lowband_in {
            lb_buf[..n].copy_from_slice(&l[..n]);
        }
        for k in 0..recombine {
            const BIT_INTERLEAVE: [u32; 16] = [0, 1, 1, 1, 2, 3, 3, 3, 2, 3, 3, 3, 2, 3, 3, 3];
            if has_lowband {
                haar1(&mut lb_buf, n >> k, 1 << k);
            }
            fill = BIT_INTERLEAVE[(fill & 0xf) as usize]
                | BIT_INTERLEAVE[(fill >> 4) as usize & 0xf] << 2;
        }
        bb >>= recombine;
        n_b <<= recombine;
        let mut time_divide = 0;
        while n_b & 1 == 0 && tf_change < 0 {
            if has_lowband {
                haar1(&mut lb_buf, n_b, bb);
            }
            fill |= fill << bb;
            bb <<= 1;
            n_b >>= 1;
            time_divide += 1;
            tf_change += 1;
        }
        let b0 = bb;
        let n_b0 = n_b;
        if b0 > 1 && has_lowband {
            deinterleave_hadamard(&mut lb_buf, n_b >> recombine, b0 << recombine, long_blocks);
        }
        let lowband = if has_lowband {
            Some(&lb_buf[..n])
        } else {
            None
        };
        let mut cm = self.quant_partition(x, n, b, bb, lowband, lm, gain, fill);

        if b0 > 1 {
            interleave_hadamard(x, n_b >> recombine, b0 << recombine, long_blocks);
        }
        let mut n_b = n_b0;
        let mut bb = b0;
        for _ in 0..time_divide {
            bb >>= 1;
            n_b <<= 1;
            cm |= cm >> bb;
            haar1(x, n_b, bb);
        }
        for k in 0..recombine {
            const BIT_DEINTERLEAVE: [u32; 16] = [
                0x00, 0x03, 0x0C, 0x0F, 0x30, 0x33, 0x3C, 0x3F, 0xC0, 0xC3, 0xCC, 0xCF, 0xF0, 0xF3,
                0xFC, 0xFF,
            ];
            cm = BIT_DEINTERLEAVE[(cm & 0xf) as usize];
            haar1(x, n0 >> k, 1 << k);
        }
        bb <<= recombine;
        if let Some(out) = lowband_out {
            let s = (n0 as f32).sqrt();
            for (o, &v) in out[..n0].iter_mut().zip(x.iter()) {
                *o = s * v;
            }
        }
        cm & ((1u64 << bb) - 1) as u32
    }

    fn quant_band_n1(
        &mut self,
        x: &mut [f32],
        y: Option<&mut [f32]>,
        lowband_out: Option<&mut [f32]>,
    ) -> u32 {
        let decode_sign = |ctx: &mut Self| -> f32 {
            let mut sign = 0;
            if ctx.remaining_bits >= 1 << BITRES {
                sign = ctx.ec.bits(1);
                ctx.remaining_bits -= 1 << BITRES;
            }
            if sign != 0 { -1.0 } else { 1.0 }
        };
        x[0] = decode_sign(self);
        if let Some(y) = y {
            y[0] = decode_sign(self);
        }
        if let Some(out) = lowband_out {
            out[0] = x[0];
        }
        1
    }

    #[allow(clippy::too_many_arguments)]
    fn quant_band_stereo(
        &mut self,
        x: &mut [f32],
        y: &mut [f32],
        n: usize,
        mut b: i32,
        bb: usize,
        lowband: Option<&[f32]>,
        lm: i32,
        lowband_out: Option<&mut [f32]>,
        mut fill: u32,
    ) -> u32 {
        if n == 1 {
            return self.quant_band_n1(x, Some(y), lowband_out);
        }
        let orig_fill = fill;
        let sp = self.compute_theta(n, &mut b, bb, bb, lm, true, &mut fill);
        let mid = sp.imid as f32 / 32768.0;
        let side = sp.iside as f32 / 32768.0;
        let mut cm;
        if n == 2 {
            let mut mbits = b;
            let mut sbits = 0;
            if sp.itheta != 0 && sp.itheta != 16384 {
                sbits = 1 << BITRES;
            }
            mbits -= sbits;
            let c = sp.itheta > 8192;
            self.remaining_bits -= sp.qalloc + sbits;
            let mut sign = 0i32;
            if sbits != 0 {
                sign = self.ec.bits(1) as i32;
            }
            let sign = (1 - 2 * sign) as f32;
            if c {
                cm = self.quant_band(y, n, mbits, bb, lowband, lm, lowband_out, 1.0, orig_fill);
                x[0] = -sign * y[1];
                x[1] = sign * y[0];
            } else {
                cm = self.quant_band(x, n, mbits, bb, lowband, lm, lowband_out, 1.0, orig_fill);
                y[0] = -sign * x[1];
                y[1] = sign * x[0];
            }
            x[0] *= mid;
            x[1] *= mid;
            y[0] *= side;
            y[1] *= side;
            let t = x[0];
            x[0] = t - y[0];
            y[0] += t;
            let t = x[1];
            x[1] = t - y[1];
            y[1] += t;
        } else {
            let mut mbits = b.min((b - sp.delta) / 2).max(0);
            let mut sbits = b - mbits;
            self.remaining_bits -= sp.qalloc;
            let mut rebalance = self.remaining_bits;
            if mbits >= sbits {
                cm = self.quant_band(x, n, mbits, bb, lowband, lm, lowband_out, 1.0, fill);
                rebalance = mbits - (rebalance - self.remaining_bits);
                if rebalance > 3 << BITRES && sp.itheta != 0 {
                    sbits += rebalance - (3 << BITRES);
                }
                cm |= self.quant_band(y, n, sbits, bb, None, lm, None, side, fill >> bb);
            } else {
                cm = self.quant_band(y, n, sbits, bb, None, lm, None, side, fill >> bb);
                rebalance = sbits - (rebalance - self.remaining_bits);
                if rebalance > 3 << BITRES && sp.itheta != 16384 {
                    mbits += rebalance - (3 << BITRES);
                }
                cm |= self.quant_band(x, n, mbits, bb, lowband, lm, lowband_out, 1.0, fill);
            }
            stereo_merge(&mut x[..n], &mut y[..n], mid);
        }
        if sp.inv {
            for v in y[..n].iter_mut() {
                *v = -*v;
            }
        }
        cm
    }
}

fn stereo_merge(x: &mut [f32], y: &mut [f32], mid: f32) {
    let mut xp = 0f32;
    let mut side = 0f32;
    for (a, b) in x.iter().zip(y.iter()) {
        xp += b * a;
        side += b * b;
    }
    xp *= mid;
    let mid2 = mid;
    let el = mid2 * mid2 + side - 2.0 * xp;
    let er = mid2 * mid2 + side + 2.0 * xp;
    if er < 6e-4 || el < 6e-4 {
        y.copy_from_slice(x);
        return;
    }
    let lgain = 1.0 / el.sqrt();
    let rgain = 1.0 / er.sqrt();
    for (a, b) in x.iter_mut().zip(y.iter_mut()) {
        let l = mid * *a;
        let r = *b;
        *a = lgain * (l - r);
        *b = rgain * (l + r);
    }
}

/// Decodes all bands (quant_all_bands, decoder side). `x` and `y` hold `n` = 120 << lm
/// coefficients per channel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_all_bands(
    ec: &mut RangeDecoder,
    start: usize,
    end: usize,
    x: &mut [f32],
    mut y: Option<&mut [f32]>,
    collapse_masks: &mut [u8],
    pulses: &[i32; NB_EBANDS],
    short_blocks: bool,
    spread: u32,
    mut dual_stereo: bool,
    intensity: usize,
    tf_res: &[i32; NB_EBANDS],
    total_bits: i32,
    mut balance: i32,
    lm: usize,
    coded_bands: usize,
    seed: &mut u32,
    disable_inv: bool,
) {
    let m = 1i32 << lm;
    let bb = if short_blocks { m as usize } else { 1 };
    let c = if y.is_some() { 2 } else { 1 };
    let norm_offset = (m * eb(start)) as usize;
    let norm_len = (m * eb(NB_EBANDS - 1)) as usize - norm_offset;
    let mut norm = vec![0f32; norm_len];
    let mut norm2 = vec![0f32; if c == 2 { norm_len } else { 0 }];
    let mut ctx = BandCtx {
        ec,
        band: start,
        intensity,
        spread,
        tf_change: 0,
        remaining_bits: 0,
        seed: *seed,
        avoid_split_noise: bb > 1,
        disable_inv,
        vtab: Vec::new(),
        iy: Vec::new(),
    };
    let mut lowband_offset = 0usize;
    let mut update_lowband = true;
    let mut lb_copy = [0f32; MAX_BAND];
    for i in start..end {
        ctx.band = i;
        let last = i == end - 1;
        let xs = (m * eb(i)) as usize;
        let n = (m * eb(i + 1) - m * eb(i)) as usize;
        let tell = ctx.ec.tell_frac();
        if i != start {
            balance -= tell;
        }
        let remaining_bits = total_bits - tell - 1;
        ctx.remaining_bits = remaining_bits;
        let b = if i < coded_bands {
            let curr_balance = balance / (3.min(coded_bands as i32 - i as i32));
            0.max(16383.min((remaining_bits + 1).min(pulses[i] + curr_balance)))
        } else {
            0
        };
        if ((m * eb(i)) as usize >= n + norm_offset || i == start + 1)
            && (update_lowband || lowband_offset == 0)
        {
            lowband_offset = i;
        }
        if i == start + 1 {
            // Duplicate enough of the first band to fold the second one (RFC 8251 section 9).
            let n1 = (m * (eb(start + 1) - eb(start))) as usize;
            let n2 = (m * (eb(start + 2) - eb(start + 1))) as usize;
            if n2 > n1 {
                norm.copy_within(2 * n1 - n2..n1, n1);
                if dual_stereo {
                    norm2.copy_within(2 * n1 - n2..n1, n1);
                }
            }
        }
        ctx.tf_change = tf_res[i];
        let mut effective_lowband: Option<usize> = None;
        let (x_cm, y_cm);
        if lowband_offset != 0 && (spread != SPREAD_AGGRESSIVE || bb > 1 || ctx.tf_change < 0) {
            let el = (m * eb(lowband_offset) - norm_offset as i32 - n as i32).max(0) as usize;
            effective_lowband = Some(el);
            let mut fold_start = lowband_offset;
            loop {
                fold_start -= 1;
                if (m * eb(fold_start)) as usize <= el + norm_offset {
                    break;
                }
            }
            let mut fold_end = lowband_offset - 1;
            loop {
                fold_end += 1;
                if !(fold_end < i && ((m * eb(fold_end)) as usize) < el + norm_offset + n) {
                    break;
                }
            }
            let mut xc = 0u32;
            let mut yc = 0u32;
            let mut fi = fold_start;
            loop {
                xc |= u32::from(collapse_masks[fi * c]);
                yc |= u32::from(collapse_masks[fi * c + c - 1]);
                fi += 1;
                if fi >= fold_end {
                    break;
                }
            }
            x_cm = xc;
            y_cm = yc;
        } else {
            x_cm = (1u32 << bb) - 1;
            y_cm = x_cm;
        }
        if dual_stereo && i == intensity {
            dual_stereo = false;
            let lim = (m * eb(i)) as usize - norm_offset;
            for j in 0..lim {
                norm[j] = 0.5 * (norm[j] + norm2[j]);
            }
        }
        let out_off = (m * eb(i)) as usize - norm_offset;
        let xb = &mut x[xs..xs + n];
        let (cm_x, cm_y);
        if dual_stereo {
            let y = y.as_deref_mut().expect("stereo");
            let yb = &mut y[xs..xs + n];
            let lb = effective_lowband.map(|e| {
                lb_copy[..n].copy_from_slice(&norm[e..e + n]);
                &lb_copy[..n]
            });
            let lb_owned: Option<Vec<f32>> = lb.map(|s| s.to_vec());
            let out = if last {
                None
            } else {
                Some(&mut norm[out_off..out_off + n])
            };
            cm_x = ctx.quant_band(
                xb,
                n,
                b / 2,
                bb,
                lb_owned.as_deref(),
                lm as i32,
                out,
                1.0,
                x_cm,
            );
            let lb2: Option<Vec<f32>> = effective_lowband.map(|e| norm2[e..e + n].to_vec());
            let out2 = if last {
                None
            } else {
                Some(&mut norm2[out_off..out_off + n])
            };
            cm_y = ctx.quant_band(yb, n, b / 2, bb, lb2.as_deref(), lm as i32, out2, 1.0, y_cm);
        } else {
            let lb: Option<Vec<f32>> = effective_lowband.map(|e| norm[e..e + n].to_vec());
            let out = if last {
                None
            } else {
                Some(&mut norm[out_off..out_off + n])
            };
            if let Some(y) = y.as_deref_mut() {
                let yb = &mut y[xs..xs + n];
                cm_x = ctx.quant_band_stereo(
                    xb,
                    yb,
                    n,
                    b,
                    bb,
                    lb.as_deref(),
                    lm as i32,
                    out,
                    x_cm | y_cm,
                );
            } else {
                cm_x = ctx.quant_band(
                    xb,
                    n,
                    b,
                    bb,
                    lb.as_deref(),
                    lm as i32,
                    out,
                    1.0,
                    x_cm | y_cm,
                );
            }
            cm_y = cm_x;
        }
        collapse_masks[i * c] = cm_x as u8;
        collapse_masks[i * c + c - 1] = cm_y as u8;
        balance += pulses[i] + tell;
        update_lowband = b > (n as i32) << BITRES;
        ctx.avoid_split_noise = false;
    }
    *seed = ctx.seed;
}

/// Anti-collapse: fills short blocks that got no pulses with noise (RFC 6716 section 4.3.5).
#[allow(clippy::too_many_arguments)]
pub(crate) fn anti_collapse(
    x: &mut [f32],
    collapse_masks: &[u8],
    lm: usize,
    c: usize,
    size: usize,
    start: usize,
    end: usize,
    log_e: &[f32],
    prev1: &[f32],
    prev2: &[f32],
    pulses: &[i32; NB_EBANDS],
    mut seed: u32,
) {
    for i in start..end {
        let n0 = (eb(i + 1) - eb(i)) as usize;
        let depth = (((1 + pulses[i]) as u32 / (n0 as u32)) >> lm) as i32;
        let thresh = 0.5 * (-0.125 * depth as f32).exp2();
        let sqrt_1 = 1.0 / ((n0 << lm) as f32).sqrt();
        for ch in 0..c {
            let mut p1 = prev1[ch * NB_EBANDS + i];
            let mut p2 = prev2[ch * NB_EBANDS + i];
            if c == 1 {
                p1 = p1.max(prev1[NB_EBANDS + i]);
                p2 = p2.max(prev2[NB_EBANDS + i]);
            }
            let ediff = (log_e[ch * NB_EBANDS + i] - p1.min(p2)).max(0.0);
            let mut r = 2.0 * (-ediff).exp2();
            if lm == 3 {
                r *= std::f32::consts::SQRT_2;
            }
            r = r.min(thresh) * sqrt_1;
            let xb = &mut x[ch * size + ((eb(i) as usize) << lm)..];
            let mut renorm = false;
            for k in 0..1usize << lm {
                if collapse_masks[i * c + ch] & (1 << k) == 0 {
                    for j in 0..n0 {
                        seed = lcg_rand(seed);
                        xb[(j << lm) + k] = if seed & 0x8000 != 0 { r } else { -r };
                    }
                    renorm = true;
                }
            }
            if renorm {
                renormalise(&mut xb[..n0 << lm], 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitexact_helpers() {
        // 1 + x2 with x2 = 32767 (theta 0 never reaches the 16-bit wrap).
        assert_eq!(bitexact_cos(0), 32768);
        assert_eq!(bitexact_log2tan(16384, 16384), 0);
        assert_eq!(isqrt32(17), 4);
        assert_eq!(isqrt32(16), 4);
    }

    #[test]
    fn pvq_count() {
        // V(2, 2) = 8 codewords: decode all of them via a fake index is covered by the vectors.
        assert_eq!(compute_qn(4, 0, 0, 0, false), 1);
    }
}
