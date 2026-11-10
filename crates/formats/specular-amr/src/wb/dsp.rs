//! AMR-WB signal-processing pieces (TS 26.190 / 26.173): ISF/ISP conversion, LP
//! synthesis, the 12.8 to 16 kHz resampler, the output filters and the math helpers.

use super::tables as t;
use super::{L_SUBFR, M, M16K};
use crate::basic::*;

pub(crate) fn random(seed: &mut i16) -> i16 {
    *seed = extract_l(l_add(l_shr(l_mult(*seed, 31821), 1), 13849));
    *seed
}

pub(crate) fn scale_sig(x: &mut [i16], exp: i16) {
    for v in x.iter_mut() {
        *v = round(l_shl(l_deposit_h(*v), exp));
    }
}

/// Normalised dot product with its exponent (energy is `result * 2^(exp-31)`).
pub(crate) fn dot_product12(x: &[i16], y: &[i16]) -> (i32, i16) {
    let mut s = 1i32;
    for (&a, &b) in x.iter().zip(y) {
        s = l_mac(s, a, b);
    }
    let sft = norm_l(s);
    (l_shl(s, sft), sub(30, sft))
}

/// 1/sqrt of a normalised fraction with exponent, in place.
pub(crate) fn isqrt_n(frac: &mut i32, exp: &mut i16) {
    if *frac <= 0 {
        *exp = 0;
        *frac = MAX_32;
        return;
    }
    if *exp & 1 == 1 {
        *frac = l_shr(*frac, 1);
    }
    *exp = negate(shr(sub(*exp, 1), 1));
    *frac = l_shr(*frac, 9);
    let i = extract_h(*frac) as usize;
    *frac = l_shr(*frac, 1);
    let a = extract_l(*frac) & 0x7fff;
    let i = i - 16;
    let tb = &t::TABLE_ISQRT;
    *frac = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    *frac = l_msu(*frac, tmp, a);
}

pub(crate) fn isqrt(l_x: i32) -> i32 {
    let e = norm_l(l_x);
    let mut x = l_shl(l_x, e);
    let mut exp = sub(31, e);
    isqrt_n(&mut x, &mut exp);
    l_shl(x, exp)
}

pub(crate) fn pow2(exponent: i16, fraction: i16) -> i32 {
    let l = l_mult(fraction, 32);
    let i = extract_h(l) as usize;
    let l = l_shr(l, 1);
    let a = extract_l(l) & 0x7fff;
    let tb = &t::TABLE_POW2;
    let x = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    l_shr_r(l_msu(x, tmp, a), sub(30, exponent))
}

pub(crate) fn log2(l_x: i32) -> (i16, i16) {
    let e = norm_l(l_x);
    let l = l_shl(l_x, e);
    if l <= 0 {
        return (0, 0);
    }
    let exponent = sub(30, e);
    let l = l_shr(l, 9);
    let i = extract_h(l) as usize;
    let l = l_shr(l, 1);
    let a = extract_l(l) & 0x7fff;
    let i = i - 32;
    let tb = &t::LOG2_TABLE;
    let y = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    (exponent, extract_h(l_msu(y, tmp, a)))
}

pub(crate) fn median5(x: &[i16]) -> i16 {
    let (mut x1, mut x2, mut x3, mut x4, mut x5) = (x[0], x[1], x[2], x[3], x[4]);
    if x2 < x1 {
        std::mem::swap(&mut x1, &mut x2);
    }
    if x3 < x1 {
        std::mem::swap(&mut x1, &mut x3);
    }
    if x4 < x1 {
        std::mem::swap(&mut x1, &mut x4);
    }
    if x5 < x1 {
        x5 = x1;
    }
    if x3 < x2 {
        std::mem::swap(&mut x2, &mut x3);
    }
    if x4 < x2 {
        std::mem::swap(&mut x2, &mut x4);
    }
    if x5 < x2 {
        x5 = x2;
    }
    if x4 < x3 {
        x3 = x4;
    }
    if x5 < x3 {
        x3 = x5;
    }
    x3
}

pub(crate) fn reorder_isf(isf: &mut [i16], min_dist: i16, n: usize) {
    let mut isf_min = min_dist;
    for v in isf.iter_mut().take(n - 1) {
        if *v < isf_min {
            *v = isf_min;
        }
        isf_min = add(*v, min_dist);
    }
}

/// ISF (Hz-like, Q15 of 6.4 kHz) to ISP (cosine domain); `m` is 16 or 20.
pub(crate) fn isf_isp(isf: &[i16], isp: &mut [i16], m: usize) {
    let mut tmp = [0i16; M16K];
    tmp[..m - 1].copy_from_slice(&isf[..m - 1]);
    tmp[m - 1] = shl(isf[m - 1], 1);
    let tb = &t::ISP_ISF_TABLE;
    for i in 0..m {
        let ind = shr(tmp[i], 7) as usize;
        let offset = tmp[i] & 0x7f;
        let l = l_mult(sub(tb[ind + 1], tb[ind]), offset);
        isp[i] = add(tb[ind], extract_l(l_shr(l, 8)));
    }
}

fn get_isp_pol(isp: &[i16], first: usize, n: usize, wide: bool) -> [i32; 11] {
    let mut f = [0i32; 11];
    let (f0, k) = if wide {
        (l_mult(4096, 256), 64)
    } else {
        (l_mult(4096, 1024), 256)
    };
    f[0] = f0;
    f[1] = l_mult(isp[first], -k);
    let mut li = first + 2;
    for i in 2..=n {
        f[i] = f[i - 2];
        let mut p = i;
        for _ in 1..i {
            let (hi, lo) = l_extract(f[p - 1]);
            let t0 = l_shl(mpy_32_16(hi, lo, isp[li]), 1);
            f[p] = l_sub(f[p], t0);
            f[p] = l_add(f[p], f[p - 2]);
            p -= 1;
        }
        f[p] = l_msu(f[p], isp[li], k);
        li += 2;
    }
    f
}

/// ISP to LP coefficients; `m` is 16 or 20 (the high band of 6.60 kbit/s).
pub(crate) fn isp_az(isp: &[i16], a: &mut [i16], m: usize, adaptive_scaling: bool) {
    let nc = m >> 1;
    let wide = nc > 8;
    let mut f1 = get_isp_pol(isp, 0, nc, wide);
    let mut f2 = get_isp_pol(isp, 1, nc - 1, wide);
    if wide {
        for v in f1.iter_mut().take(nc + 1) {
            *v = l_shl(*v, 2);
        }
        for v in f2.iter_mut().take(nc) {
            *v = l_shl(*v, 2);
        }
    }
    for i in (2..nc).rev() {
        f2[i] = l_sub(f2[i], f2[i - 2]);
    }
    let last = isp[m - 1];
    for i in 0..nc {
        let (hi, lo) = l_extract(f1[i]);
        f1[i] = l_add(f1[i], mpy_32_16(hi, lo, last));
        let (hi, lo) = l_extract(f2[i]);
        f2[i] = l_sub(f2[i], mpy_32_16(hi, lo, last));
    }
    a[0] = 4096;
    let mut tmax: i32 = 1;
    let mut j = m - 1;
    for i in 1..nc {
        let t0 = l_add(f1[i], f2[i]);
        tmax |= l_abs(t0);
        a[i] = extract_l(l_shr_r(t0, 12));
        let t0 = l_sub(f1[i], f2[i]);
        tmax |= l_abs(t0);
        a[j] = extract_l(l_shr_r(t0, 12));
        j -= 1;
    }
    let mut q = if adaptive_scaling {
        sub(4, norm_l(tmax))
    } else {
        0
    };
    let q_sug;
    if q > 0 {
        q_sug = add(12, q);
        let mut j = m - 1;
        for i in 1..nc {
            a[i] = extract_l(l_shr_r(l_add(f1[i], f2[i]), q_sug));
            a[j] = extract_l(l_shr_r(l_sub(f1[i], f2[i]), q_sug));
            j -= 1;
        }
        a[0] = shr(a[0], q);
    } else {
        q_sug = 12;
        q = 0;
    }
    let (hi, lo) = l_extract(f1[nc]);
    let t0 = l_add(f1[nc], mpy_32_16(hi, lo, last));
    a[nc] = extract_l(l_shr_r(t0, q_sug));
    a[m] = shr_r(last, add(3, q));
}

/// Interpolates the ISPs for the first three subframes and converts all four.
pub(crate) fn int_isp(isp_old: &[i16], isp_new: &[i16], frac: &[i16; 4], az: &mut [i16]) {
    let mut isp = [0i16; M];
    for k in 0..3 {
        let fac_new = frac[k];
        let fac_old = add(sub(32767, fac_new), 1);
        for i in 0..M {
            let l = l_mult(isp_old[i], fac_old);
            isp[i] = round(l_mac(l, isp_new[i], fac_new));
        }
        isp_az(&isp, &mut az[k * (M + 1)..], M, false);
    }
    isp_az(isp_new, &mut az[3 * (M + 1)..], M, false);
}

pub(crate) fn weight_a(a: &[i16], ap: &mut [i16], gamma: i16, m: usize) {
    ap[0] = a[0];
    let mut fac = gamma;
    for i in 1..m {
        ap[i] = round(l_mult(a[i], fac));
        fac = round(l_mult(fac, gamma));
    }
    ap[m] = round(l_mult(a[m], fac));
}

/// LP synthesis 1/A(z) of order `m`, in place on `x` (output scaled by a[0]).
pub(crate) fn syn_filt(a: &[i16], m: usize, x: &mut [i16], mem: &mut [i16], update: bool) {
    let lg = x.len();
    let mut yy = [0i16; 80 + M16K];
    yy[..m].copy_from_slice(&mem[..m]);
    let s = sub(norm_s(a[0]), 2);
    let a0 = shr(a[0], 1);
    for i in 0..lg {
        let mut l = l_mult(x[i], a0);
        for j in 1..=m {
            l = l_msu(l, a[j], yy[m + i - j]);
        }
        let v = round(l_shl(l, add(3, s)));
        yy[m + i] = v;
        x[i] = v;
    }
    if update {
        mem[..m].copy_from_slice(&yy[lg..lg + m]);
    }
}

/// Double-precision LP synthesis of one subframe; `hi`/`lo` hold M samples of memory
/// before the L_SUBFR outputs.
pub(crate) fn syn_filt_32(a: &[i16], exc: &[i16], q_new: i16, hi: &mut [i16], lo: &mut [i16]) {
    let s = sub(norm_s(a[0]), 2);
    let a0 = shr(a[0], add(4, q_new));
    for i in 0..L_SUBFR {
        let n = M + i;
        let mut l = 0i32;
        for j in 1..=M {
            l = l_msu(l, lo[n - j], a[j]);
        }
        l = l_shr(l, 16 - 4);
        l = l_mac(l, exc[i], a0);
        for j in 1..=M {
            l = l_msu(l, hi[n - j], a[j]);
        }
        l = l_shl(l, add(3, s));
        hi[n] = extract_h(l);
        let l = l_shr(l, 4);
        lo[n] = extract_l(l_msu(l, hi[n], 2048));
    }
}

pub(crate) fn deemph_32(hi: &[i16], lo: &[i16], y: &mut [i16], mu: i16, mem: &mut i16) {
    let fac = shr(mu, 1);
    let mut prev = *mem;
    for i in 0..y.len() {
        let mut l = l_deposit_h(hi[i]);
        l = l_mac(l, lo[i], 8);
        l = l_shl(l, 3);
        l = l_mac(l, prev, fac);
        l = l_shl(l, 1);
        y[i] = round(l);
        prev = y[i];
    }
    *mem = prev;
}

/// Second-order IIR (the 50 Hz and 400 Hz high-passes at 12.8 kHz).
pub(crate) fn hp_iir(
    signal: &mut [i16],
    mem: &mut [i16; 6],
    b: [i16; 3],
    a: [i16; 3],
    out_shift_50: bool,
) {
    let [mut y2_hi, mut y2_lo, mut y1_hi, mut y1_lo, mut x0, mut x1] = *mem;
    for v in signal.iter_mut() {
        let x2 = x1;
        x1 = x0;
        x0 = *v;
        let mut l: i32 = 16384;
        l = l_mac(l, y1_lo, a[1]);
        l = l_mac(l, y2_lo, a[2]);
        l = l_shr(l, 15);
        l = l_mac(l, y1_hi, a[1]);
        l = l_mac(l, y2_hi, a[2]);
        l = l_mac(l, x0, b[0]);
        l = l_mac(l, x1, b[1]);
        l = l_mac(l, x2, b[2]);
        l = l_shl(l, if out_shift_50 { 2 } else { 1 });
        y2_hi = y1_hi;
        y2_lo = y1_lo;
        (y1_hi, y1_lo) = l_extract(l);
        *v = if out_shift_50 {
            round(l_shl(l, 1))
        } else {
            round(l)
        };
    }
    *mem = [y2_hi, y2_lo, y1_hi, y1_lo, x0, x1];
}

/// 31-tap FIR used for the high band (6–7 kHz band-pass, or 7 kHz low-pass).
pub(crate) fn fir31(signal: &mut [i16], mem: &mut [i16; 30], fir: &[i16; 31], pre_shift: i16) {
    let lg = signal.len();
    let mut x = [0i16; 80 + 30];
    x[..30].copy_from_slice(mem);
    for i in 0..lg {
        x[i + 30] = shr(signal[i], pre_shift);
    }
    for i in 0..lg {
        let mut l = 0i32;
        for j in 0..31 {
            l = l_mac(l, x[i + j], fir[j]);
        }
        signal[i] = round(l);
    }
    mem.copy_from_slice(&x[lg..lg + 30]);
}

/// 12.8 kHz to 16 kHz (factor 5/4) for one subframe: 64 samples in, 80 out.
pub(crate) fn oversamp_16k(sig12k8: &[i16], sig16k: &mut [i16], mem: &mut [i16; 24]) {
    const NB_COEF_UP: usize = 12;
    let lg = sig12k8.len();
    let mut signal = [0i16; L_SUBFR + 2 * NB_COEF_UP];
    signal[..2 * NB_COEF_UP].copy_from_slice(mem);
    signal[2 * NB_COEF_UP..2 * NB_COEF_UP + lg].copy_from_slice(sig12k8);
    let lg_up = shl(mult(lg as i16, 20480), 1) as usize;
    let fir = &t::FIR_UP;
    let mut pos: i16 = 0;
    for out in sig16k.iter_mut().take(lg_up) {
        let i = mult(pos, 6554);
        let frac = sub(pos, add(shl(i, 2), i));
        let base = NB_COEF_UP + i as usize + 1 - NB_COEF_UP;
        let mut l = 0i32;
        let mut k = (5 - 1 - frac) as usize;
        for n in 0..2 * NB_COEF_UP {
            l = l_mac(l, signal[base + n], fir[k]);
            k += 5;
        }
        *out = round(l_shl(l, 1));
        pos = add(pos, 4);
    }
    mem.copy_from_slice(&signal[lg..lg + 2 * NB_COEF_UP]);
}

/// Adaptive-codebook excitation by 1/4-sample interpolation of the past excitation.
pub(crate) fn pred_lt4(exc: &mut [i16], base: usize, t0: i16, frac: i16, len: usize) {
    const UP_SAMP: i16 = 4;
    const L_INTERPOL2: isize = 16;
    let mut x = base as isize - t0 as isize;
    let mut frac = negate(frac);
    if frac < 0 {
        frac = add(frac, UP_SAMP);
        x -= 1;
    }
    x = x - L_INTERPOL2 + 1;
    let tb = &t::INTER4_2;
    for j in 0..len {
        let mut l = 0i32;
        let mut k = (UP_SAMP - 1 - frac) as usize;
        let xs = x as usize;
        for i in 0..32 {
            l = l_mac(l, exc[xs + i], tb[k]);
            k += 4;
        }
        exc[base + j] = round(l_shl(l, 1));
        x += 1;
    }
}

pub(crate) fn preemph(x: &mut [i16], mu: i16, mem: &mut i16) {
    let lg = x.len();
    let temp = x[lg - 1];
    for i in (1..lg).rev() {
        x[i] = round(l_msu(l_deposit_h(x[i]), x[i - 1], mu));
    }
    x[0] = round(l_msu(l_deposit_h(x[0]), *mem, mu));
    *mem = temp;
}

pub(crate) fn pit_shrp(x: &mut [i16], pit_lag: i16, sharp: i16) {
    let lag = pit_lag.max(0) as usize;
    for i in lag..x.len() {
        x[i] = round(l_mac(l_deposit_h(x[i]), x[i - lag], sharp));
    }
}

pub(crate) fn voice_factor(
    exc: &[i16],
    q_exc: i16,
    gain_pit: i16,
    code: &[i16],
    gain_code: i16,
) -> i16 {
    let (e1, exp1) = dot_product12(exc, exc);
    let mut ener1 = extract_h(e1);
    let mut exp1 = sub(exp1, add(q_exc, q_exc));
    let l = l_mult(gain_pit, gain_pit);
    let exp = norm_l(l);
    let tmp = extract_h(l_shl(l, exp));
    ener1 = mult(ener1, tmp);
    exp1 = sub(sub(exp1, exp), 10);
    let (e2, exp2) = dot_product12(code, code);
    let mut ener2 = extract_h(e2);
    let exp = norm_s(gain_code);
    let tmp = shl(gain_code, exp);
    let tmp = mult(tmp, tmp);
    ener2 = mult(ener2, tmp);
    let exp2 = sub(exp2, add(exp, exp));
    let i = sub(exp1, exp2);
    if i >= 0 {
        ener1 = shr(ener1, 1);
        ener2 = shr(ener2, add(i, 1));
    } else {
        ener1 = shr(ener1, sub(1, i));
        ener2 = shr(ener2, 1);
    }
    let tmp = sub(ener1, ener2);
    let ener1 = add(add(ener1, ener2), 1);
    if tmp >= 0 {
        div_s(tmp, ener1)
    } else {
        negate(div_s(negate(tmp), ener1))
    }
}

pub(crate) fn agc2(sig_in: &[i16], sig_out: &mut [i16]) {
    let energy = |x: &[i16]| {
        let t = shr(x[0], 2);
        let mut s = l_mult(t, t);
        for &v in &x[1..] {
            let t = shr(v, 2);
            s = l_mac(s, t, t);
        }
        s
    };
    let s = energy(sig_out);
    if s == 0 {
        return;
    }
    let exp = sub(norm_l(s), 1);
    let gain_out = round(l_shl(s, exp));
    let s = energy(sig_in);
    let g0 = if s == 0 {
        0
    } else {
        let i = norm_l(s);
        let gain_in = round(l_shl(s, i));
        let exp = sub(exp, i);
        let s = l_deposit_l(div_s(gain_out, gain_in));
        let s = l_shr(l_shl(s, 7), exp);
        round(l_shl(isqrt(s), 9))
    };
    for v in sig_out.iter_mut() {
        *v = extract_h(l_shl(l_mult(*v, g0), 2));
    }
}

/// Extrapolates the 16 ISFs to the 20 of the 6.4–7 kHz band (6.60 kbit/s), then to ISPs.
pub(crate) fn isf_extrapolation(hf: &mut [i16; M16K]) {
    const INV_LENGTH: i16 = 2731;
    let mut diff = [0i16; M - 2];
    hf[M16K - 1] = hf[M - 1];
    for i in 1..M - 1 {
        diff[i - 1] = sub(hf[i], hf[i - 1]);
    }
    let mut l = 0i32;
    for i in 3..M - 1 {
        l = l_mac(l, diff[i - 1], INV_LENGTH);
    }
    let mut mean = round(l);
    let mut tmp: i16 = 0;
    for &d in &diff {
        if d > tmp {
            tmp = d;
        }
    }
    let exp = norm_s(tmp);
    for d in diff.iter_mut() {
        *d = shl(*d, exp);
    }
    mean = shl(mean, exp);
    let mut corr = [0i32; 3];
    for (c, lag) in corr.iter_mut().zip([2usize, 3, 4]) {
        for i in 7..M - 2 {
            let t2 = sub(diff[i], mean);
            let t3 = sub(diff[i - lag], mean);
            let (hi, lo) = l_extract(l_mult(t2, t3));
            *c = l_add(*c, mpy_32(hi, lo, hi, lo));
        }
    }
    let mut max_corr = if corr[0] > corr[1] { 0 } else { 1 };
    if corr[2] > corr[max_corr] {
        max_corr = 2;
    }
    let max_corr = max_corr + 1;
    for i in M - 1..M16K - 1 {
        let tmp = sub(hf[i - 1 - max_corr], hf[i - 2 - max_corr]);
        hf[i] = add(hf[i - 1], tmp);
    }
    let mut tmp = add(hf[4], hf[3]);
    tmp = sub(hf[2], tmp);
    tmp = mult(tmp, 5461);
    tmp = add(tmp, 20390);
    if tmp > 19456 {
        tmp = 19456;
    }
    let tmp = sub(tmp, hf[M - 2]);
    let tmp2 = sub(hf[M16K - 2], hf[M - 2]);
    let exp2 = norm_s(tmp2);
    let exp = sub(norm_s(tmp), 1);
    let tmp = shl(tmp, exp);
    let tmp2 = shl(tmp2, exp2);
    let coeff = div_s(tmp, tmp2);
    let exp = sub(exp2, exp);
    let mut d2 = [0i16; M16K - M];
    for i in M - 1..M16K - 1 {
        let t = mult(sub(hf[i], hf[i - 1]), coeff);
        d2[i - (M - 1)] = shl(t, exp);
    }
    for i in M..M16K - 1 {
        let t = sub(add(d2[i - (M - 1)], d2[i - M]), 1280);
        if t < 0 {
            if d2[i - (M - 1)] > d2[i - M] {
                d2[i - M] = sub(1280, d2[i - (M - 1)]);
            } else {
                d2[i - (M - 1)] = sub(1280, d2[i - M]);
            }
        }
    }
    for i in M - 1..M16K - 1 {
        hf[i] = add(hf[i - 1], d2[i - (M - 1)]);
    }
    for v in hf.iter_mut().take(M16K - 1) {
        *v = mult(*v, 26214);
    }
    let src = *hf;
    isf_isp(&src, hf, M16K);
}
