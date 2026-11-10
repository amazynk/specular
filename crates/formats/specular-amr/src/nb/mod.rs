//! AMR (narrowband) speech decoder: 8 kHz, 20 ms frames, the eight modes 4.75 to
//! 12.2 kbit/s, comfort noise from SID frames (TS 26.092), DTX (TS 26.093) and the
//! example concealment of lost frames (TS 26.091), following the bit-exact fixed-point
//! description of TS 26.073 / TS 26.090.

mod dtx;
mod post;
pub(crate) mod tables;

use crate::basic::*;
use crate::{Frame, RxType};
use tables as t;

pub(crate) const L_FRAME: usize = 160;
pub(crate) const L_SUBFR: usize = 40;
pub(crate) const M: usize = 10;
pub(crate) const MP1: usize = M + 1;
const PIT_MAX: i16 = 143;
const PIT_MIN: i16 = 20;
const PIT_MIN_MR122: i16 = 18;
const L_INTERPOL: usize = 11;
const SHARPMAX: i16 = 13017;
const SHARPMIN: i16 = 0;
pub(crate) const LSF_GAP: i16 = 205;
const LSP_PRED_FAC_MR122: i16 = 21299;
const EXC_OFF: usize = PIT_MAX as usize + L_INTERPOL;
const OLD_EXC_LEN: usize = L_SUBFR + EXC_OFF;

/// Codec modes, in the order of their frame type numbers.
pub(crate) const MR475: u8 = 0;
pub(crate) const MR515: u8 = 1;
pub(crate) const MR59: u8 = 2;
pub(crate) const MR67: u8 = 3;
pub(crate) const MR74: u8 = 4;
pub(crate) const MR795: u8 = 5;
pub(crate) const MR102: u8 = 6;
pub(crate) const MR122: u8 = 7;
pub(crate) const MRDTX: u8 = 8;

/// Speech bits per mode (frame types 0..7) and of a SID frame (8).
pub const BITS: [usize; 9] = [95, 103, 118, 134, 148, 159, 204, 244, 35];

pub(crate) fn bitno(mode: u8) -> &'static [i16] {
    match mode {
        MR475 => &t::BITNO_MR475,
        MR515 => &t::BITNO_MR515,
        MR59 => &t::BITNO_MR59,
        MR67 => &t::BITNO_MR67,
        MR74 => &t::BITNO_MR74,
        MR795 => &t::BITNO_MR795,
        MR102 => &t::BITNO_MR102,
        MR122 => &t::BITNO_MR122,
        _ => &t::BITNO_MRDTX,
    }
}

/// The order in which a mode's bits are stored in files and RTP (TS 26.101 Annex B):
/// `sort[i]` is the parameter-order index of the i-th stored bit.
pub(crate) fn sort(mode: u8) -> &'static [i16] {
    match mode {
        MR475 => &t::SORT_475,
        MR515 => &t::SORT_515,
        MR59 => &t::SORT_59,
        MR67 => &t::SORT_67,
        MR74 => &t::SORT_74,
        MR795 => &t::SORT_795,
        MR102 => &t::SORT_102,
        MR122 => &t::SORT_122,
        _ => &t::SORT_SID,
    }
}

fn dhf(mode: u8) -> &'static [i16] {
    match mode {
        MR475 => &t::DHF_MR475,
        MR515 => &t::DHF_MR515,
        MR59 => &t::DHF_MR59,
        MR67 => &t::DHF_MR67,
        MR74 => &t::DHF_MR74,
        MR795 => &t::DHF_MR795,
        MR102 => &t::DHF_MR102,
        _ => &t::DHF_MR122,
    }
}

/// Serial bits (one 0/1 per entry, parameter order) to codec parameters.
pub(crate) fn bits2prm(mode: u8, bits: &[u8], prm: &mut [i16]) {
    let mut k = 0;
    for (i, &n) in bitno(mode).iter().enumerate() {
        let mut v: i16 = 0;
        for _ in 0..n {
            v = (v << 1) | (bits.get(k).copied().unwrap_or(0) & 1) as i16;
            k += 1;
        }
        prm[i] = v;
    }
}

// ---------------------------------------------------------------------------------------
// Shared signal-processing pieces
// ---------------------------------------------------------------------------------------

pub(crate) fn log2_norm(l_x: i32, exp: i16) -> (i16, i16) {
    if l_x <= 0 {
        return (0, 0);
    }
    let e = sub(30, exp);
    let l = l_shr(l_x, 9);
    let i = extract_h(l) as usize;
    let l = l_shr(l, 1);
    let a = extract_l(l) & 0x7fff;
    let i = i - 32;
    let tb = &t::LOG2_TABLE;
    let y = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    (e, extract_h(l_msu(y, tmp, a)))
}

pub(crate) fn log2(l_x: i32) -> (i16, i16) {
    let e = norm_l(l_x);
    log2_norm(l_shl(l_x, e), e)
}

pub(crate) fn pow2(exponent: i16, fraction: i16) -> i32 {
    let l = l_mult(fraction, 32);
    let i = extract_h(l) as usize;
    let l = l_shr(l, 1);
    let a = extract_l(l) & 0x7fff;
    let tb = &t::POW2_TABLE;
    let x = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    let x = l_msu(x, tmp, a);
    l_shr_r(x, sub(30, exponent))
}

pub(crate) fn inv_sqrt(l_x: i32) -> i32 {
    if l_x <= 0 {
        return 0x3fff_ffff;
    }
    let e = norm_l(l_x);
    let mut l = l_shl(l_x, e);
    let mut exp = sub(30, e);
    if exp & 1 == 0 {
        l = l_shr(l, 1);
    }
    exp = add(shr(exp, 1), 1);
    let l = l_shr(l, 9);
    let i = extract_h(l) as usize;
    let l = l_shr(l, 1);
    let a = extract_l(l) & 0x7fff;
    let i = i - 16;
    let tb = &t::INV_SQRT_TABLE;
    let y = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    l_shr(l_msu(y, tmp, a), exp)
}

fn sqrt_l_exp(l_x: i32) -> (i32, i16) {
    if l_x <= 0 {
        return (0, 0);
    }
    let e = norm_l(l_x) & !1;
    let l = l_shl(l_x, e);
    let l = l_shr(l, 9);
    let i = extract_h(l) as usize;
    let l = l_shr(l, 1);
    let a = extract_l(l) & 0x7fff;
    let i = i - 16;
    let tb = &t::SQRT_L_TABLE;
    let y = l_deposit_h(tb[i]);
    let tmp = sub(tb[i], tb[i + 1]);
    (l_msu(y, tmp, a), e)
}

pub(crate) fn lsf_lsp(lsf: &[i16], lsp: &mut [i16]) {
    let tb = &t::LSP_LSF_TABLE;
    for i in 0..M {
        let ind = shr(lsf[i], 8) as usize;
        let offset = lsf[i] & 0xff;
        let l = l_mult(sub(tb[ind + 1], tb[ind]), offset);
        lsp[i] = add(tb[ind], extract_l(l_shr(l, 9)));
    }
}

pub(crate) fn lsp_lsf(lsp: &[i16], lsf: &mut [i16]) {
    let (tb, sl) = (&t::LSP_LSF_TABLE, &t::LSP_LSF_SLOPE);
    let mut ind: usize = 63;
    for i in (0..M).rev() {
        while ind > 0 && sub(tb[ind], lsp[i]) < 0 {
            ind -= 1;
        }
        let l = l_mult(sub(lsp[i], tb[ind]), sl[ind]);
        lsf[i] = round(l_shl(l, 3));
        lsf[i] = add(lsf[i], shl(ind as i16, 8));
    }
}

pub(crate) fn reorder_lsf(lsf: &mut [i16], min_dist: i16) {
    let mut lsf_min = min_dist;
    for v in lsf.iter_mut().take(M) {
        if sub(*v, lsf_min) < 0 {
            *v = lsf_min;
        }
        lsf_min = add(*v, min_dist);
    }
}

fn get_lsp_pol(lsp: &[i16], first: usize) -> [i32; 6] {
    let mut f = [0i32; 6];
    f[0] = l_mult(4096, 2048);
    f[1] = l_msu(0, lsp[first], 512);
    let mut li = first + 2;
    for i in 2..=5usize {
        f[i] = f[i - 2];
        let mut k = i;
        for _ in 1..i {
            let (hi, lo) = l_extract(f[k - 1]);
            let t0 = l_shl(mpy_32_16(hi, lo, lsp[li]), 1);
            f[k] = l_add(f[k], f[k - 2]);
            f[k] = l_sub(f[k], t0);
            k -= 1;
        }
        f[k] = l_msu(f[k], lsp[li], 512);
        li += 2;
    }
    f
}

pub(crate) fn lsp_az(lsp: &[i16], a: &mut [i16]) {
    let mut f1 = get_lsp_pol(lsp, 0);
    let mut f2 = get_lsp_pol(lsp, 1);
    for i in (1..=5).rev() {
        f1[i] = l_add(f1[i], f1[i - 1]);
        f2[i] = l_sub(f2[i], f2[i - 1]);
    }
    a[0] = 4096;
    let mut j = 10;
    for i in 1..=5 {
        a[i] = extract_l(l_shr_r(l_add(f1[i], f2[i]), 13));
        a[j] = extract_l(l_shr_r(l_sub(f1[i], f2[i]), 13));
        j -= 1;
    }
}

fn int_lpc_1and3(lsp_old: &[i16], lsp_mid: &[i16], lsp_new: &[i16], az: &mut [i16]) {
    let mut lsp = [0i16; M];
    for i in 0..M {
        lsp[i] = add(shr(lsp_mid[i], 1), shr(lsp_old[i], 1));
    }
    lsp_az(&lsp, &mut az[0..MP1]);
    lsp_az(lsp_mid, &mut az[MP1..2 * MP1]);
    for i in 0..M {
        lsp[i] = add(shr(lsp_mid[i], 1), shr(lsp_new[i], 1));
    }
    lsp_az(&lsp, &mut az[2 * MP1..3 * MP1]);
    lsp_az(lsp_new, &mut az[3 * MP1..4 * MP1]);
}

fn int_lpc_1to3(lsp_old: &[i16], lsp_new: &[i16], az: &mut [i16]) {
    let mut lsp = [0i16; M];
    for i in 0..M {
        lsp[i] = add(shr(lsp_new[i], 2), sub(lsp_old[i], shr(lsp_old[i], 2)));
    }
    lsp_az(&lsp, &mut az[0..MP1]);
    for i in 0..M {
        lsp[i] = add(shr(lsp_old[i], 1), shr(lsp_new[i], 1));
    }
    lsp_az(&lsp, &mut az[MP1..2 * MP1]);
    for i in 0..M {
        lsp[i] = add(shr(lsp_old[i], 2), sub(lsp_new[i], shr(lsp_new[i], 2)));
    }
    lsp_az(&lsp, &mut az[2 * MP1..3 * MP1]);
    lsp_az(lsp_new, &mut az[3 * MP1..4 * MP1]);
}

/// The LP synthesis filter 1/A(z), 10th order, output scaled by 8 (Q12 coefficients).
pub(crate) fn syn_filt(
    a: &[i16],
    x: &[i16],
    y: &mut [i16],
    lg: usize,
    mem: &mut [i16],
    update: bool,
) {
    let mut tmp = [0i16; 80 + M];
    tmp[..M].copy_from_slice(&mem[..M]);
    for i in 0..lg {
        let mut s = l_mult(x[i], a[0]);
        for j in 1..=M {
            s = l_msu(s, a[j], tmp[M + i - j]);
        }
        tmp[M + i] = round(l_shl(s, 3));
    }
    y[..lg].copy_from_slice(&tmp[M..M + lg]);
    if update {
        mem[..M].copy_from_slice(&y[lg - M..lg]);
    }
}

/// [`syn_filt`] that also reports whether any of its operations saturated (the reference's
/// global overflow flag, which the decoder reads after its synthesis filter).
pub(crate) fn syn_filt_overflow(a: &[i16], x: &[i16], y: &mut [i16], mem: &[i16]) -> bool {
    let mut ov = false;
    let mut tmp = [0i16; L_SUBFR + M];
    tmp[..M].copy_from_slice(&mem[..M]);
    for i in 0..L_SUBFR {
        let p = x[i] as i32 * a[0] as i32;
        let mut s = if p == 0x4000_0000 {
            ov = true;
            MAX_32
        } else {
            p * 2
        };
        for j in 1..=M {
            let p = a[j] as i32 * tmp[M + i - j] as i32;
            let m = if p == 0x4000_0000 {
                ov = true;
                MAX_32
            } else {
                p * 2
            };
            let (r, o) = s.overflowing_sub(m);
            if o {
                ov = true;
                s = if s < 0 { MIN_32 } else { MAX_32 };
            } else {
                s = r;
            }
        }
        if !(-0x1000_0000..0x1000_0000).contains(&s) {
            ov = true;
        }
        let s = l_shl(s, 3);
        if s > MAX_32 - 0x8000 {
            ov = true;
        }
        tmp[M + i] = round(s);
    }
    y[..L_SUBFR].copy_from_slice(&tmp[M..]);
    ov
}

fn residu(a: &[i16], x: &[i16], x_off: usize, y: &mut [i16], lg: usize) {
    for i in 0..lg {
        let mut s = l_mult(x[x_off + i], a[0]);
        for j in 1..=M {
            s = l_mac(s, a[j], x[x_off + i - j]);
        }
        y[i] = round(l_shl(s, 3));
    }
}

fn weight_ai(a: &[i16], fac: &[i16], out: &mut [i16]) {
    out[0] = a[0];
    for i in 1..=M {
        out[i] = round(l_mult(a[i], fac[i - 1]));
    }
}

/// Median of `ind[..n]`, as the reference picks it.
pub(crate) fn gmed_n(ind: &[i16], n: usize) -> i16 {
    let mut tmp = [0usize; 9];
    let mut tmp2 = [0i16; 9];
    tmp2[..n].copy_from_slice(&ind[..n]);
    let mut ix = 0;
    for slot in tmp.iter_mut().take(n) {
        let mut max: i16 = -32767;
        for (j, &v) in tmp2.iter().enumerate().take(n) {
            if v >= max {
                max = v;
                ix = j;
            }
        }
        tmp2[ix] = -32768;
        *slot = ix;
    }
    ind[tmp[n >> 1]]
}

fn pred_lt_3or6(exc: &mut [i16], base: usize, t0: i16, frac: i16, flag3: bool) {
    let mut x0 = base as isize - t0 as isize;
    let mut frac = negate(frac);
    if flag3 {
        frac = shl(frac, 1);
    }
    if frac < 0 {
        frac = add(frac, 6);
        x0 -= 1;
    }
    let c1 = frac as usize;
    let c2 = (6 - frac) as usize;
    let tb = &t::INTER_6;
    for j in 0..L_SUBFR {
        let x1 = (x0 + j as isize) as usize;
        let x2 = x1 + 1;
        let mut s = 0i32;
        for i in 0..10 {
            s = l_mac(s, exc[x1 - i], tb[c1 + 6 * i]);
            s = l_mac(s, exc[x2 + i], tb[c2 + 6 * i]);
        }
        exc[base + j] = round(s);
    }
}

// ---------------------------------------------------------------------------------------
// Parameter decoding
// ---------------------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct DPlsf {
    pub past_r_q: [i16; M],
    pub past_lsf_q: [i16; M],
}

impl DPlsf {
    fn new() -> Self {
        DPlsf {
            past_r_q: [0; M],
            past_lsf_q: t::MEAN_LSF_5,
        }
    }

    pub(crate) fn d_plsf_3(&mut self, mode: u8, bfi: bool, ind: &[i16], lsp1_q: &mut [i16]) {
        const ALPHA: i16 = 29491;
        const ONE_ALPHA: i16 = 3277;
        let mean = &t::MEAN_LSF_3;
        let pred = &t::PRED_FAC_3;
        let mut lsf1_q = [0i16; M];
        if bfi {
            for i in 0..M {
                lsf1_q[i] = add(mult(self.past_lsf_q[i], ALPHA), mult(mean[i], ONE_ALPHA));
            }
            for i in 0..M {
                let temp = if mode != MRDTX {
                    add(mean[i], mult(self.past_r_q[i], pred[i]))
                } else {
                    add(mean[i], self.past_r_q[i])
                };
                self.past_r_q[i] = sub(lsf1_q[i], temp);
            }
        } else {
            let (cb1, cb2, cb3): (&[i16], &[i16], &[i16]) = if mode == MR475 || mode == MR515 {
                (&t::DICO1_LSF_3, &t::DICO2_LSF_3, &t::MR515_3_LSF)
            } else if mode == MR795 {
                (&t::MR795_1_LSF, &t::DICO2_LSF_3, &t::DICO3_LSF_3)
            } else {
                (&t::DICO1_LSF_3, &t::DICO2_LSF_3, &t::DICO3_LSF_3)
            };
            let mut r = [0i16; M];
            let i0 = ind[0] as usize * 3;
            r[0..3].copy_from_slice(&cb1[i0..i0 + 3]);
            let mut i1 = ind[1] as usize;
            if mode == MR475 || mode == MR515 {
                i1 <<= 1;
            }
            let i1 = i1 * 3;
            r[3..6].copy_from_slice(&cb2[i1..i1 + 3]);
            let i2 = ind[2] as usize * 4;
            r[6..10].copy_from_slice(&cb3[i2..i2 + 4]);
            for i in 0..M {
                let temp = if mode != MRDTX {
                    add(mean[i], mult(self.past_r_q[i], pred[i]))
                } else {
                    add(mean[i], self.past_r_q[i])
                };
                lsf1_q[i] = add(r[i], temp);
                self.past_r_q[i] = r[i];
            }
        }
        reorder_lsf(&mut lsf1_q, LSF_GAP);
        self.past_lsf_q = lsf1_q;
        lsf_lsp(&lsf1_q, lsp1_q);
    }

    fn d_plsf_5(&mut self, bfi: bool, ind: &[i16], lsp1_q: &mut [i16], lsp2_q: &mut [i16]) {
        const ALPHA: i16 = 31128;
        const ONE_ALPHA: i16 = 1639;
        let mean = &t::MEAN_LSF_5;
        let mut lsf1_q = [0i16; M];
        let mut lsf2_q = [0i16; M];
        if bfi {
            for i in 0..M {
                lsf1_q[i] = add(mult(self.past_lsf_q[i], ALPHA), mult(mean[i], ONE_ALPHA));
                lsf2_q[i] = lsf1_q[i];
            }
            for i in 0..M {
                let temp = add(mean[i], mult(self.past_r_q[i], LSP_PRED_FAC_MR122));
                self.past_r_q[i] = sub(lsf2_q[i], temp);
            }
        } else {
            let mut r1 = [0i16; M];
            let mut r2 = [0i16; M];
            let books: [&[i16]; 5] = [
                &t::DICO1_LSF_5,
                &t::DICO2_LSF_5,
                &t::DICO3_LSF_5,
                &t::DICO4_LSF_5,
                &t::DICO5_LSF_5,
            ];
            for k in 0..5 {
                let (idx, neg) = if k == 2 {
                    ((ind[2] >> 1) as usize, ind[2] & 1 != 0)
                } else {
                    (ind[k] as usize, false)
                };
                let p = &books[k][idx * 4..idx * 4 + 4];
                let v = |x: i16| if neg { negate(x) } else { x };
                r1[2 * k] = v(p[0]);
                r1[2 * k + 1] = v(p[1]);
                r2[2 * k] = v(p[2]);
                r2[2 * k + 1] = v(p[3]);
            }
            for i in 0..M {
                let temp = add(mean[i], mult(self.past_r_q[i], LSP_PRED_FAC_MR122));
                lsf1_q[i] = add(r1[i], temp);
                lsf2_q[i] = add(r2[i], temp);
                self.past_r_q[i] = r2[i];
            }
        }
        reorder_lsf(&mut lsf1_q, LSF_GAP);
        reorder_lsf(&mut lsf2_q, LSF_GAP);
        self.past_lsf_q = lsf2_q;
        lsf_lsp(&lsf1_q, lsp1_q);
        lsf_lsp(&lsf2_q, lsp2_q);
    }
}

#[allow(clippy::too_many_arguments)]
fn dec_lag3(
    index: i16,
    t0_min: i16,
    t0_max: i16,
    i_subfr: usize,
    t0_prev: i16,
    flag4: bool,
) -> (i16, i16) {
    if i_subfr == 0 {
        if index < 197 {
            let t0 = add(mult(add(index, 2), 10923), 19);
            let i = add(add(t0, t0), t0);
            (t0, add(sub(index, i), 58))
        } else {
            (sub(index, 112), 0)
        }
    } else if !flag4 {
        let i = sub(mult(add(index, 2), 10923), 1);
        let t0 = add(i, t0_min);
        let i = add(add(i, i), i);
        (t0, sub(sub(index, 2), i))
    } else {
        let mut tmp_lag = t0_prev;
        if sub(sub(tmp_lag, t0_min), 5) > 0 {
            tmp_lag = add(t0_min, 5);
        }
        if sub(sub(t0_max, tmp_lag), 4) > 0 {
            tmp_lag = sub(t0_max, 4);
        }
        if index < 4 {
            (add(sub(tmp_lag, 5), index), 0)
        } else if index < 12 {
            let i = sub(mult(sub(index, 5), 10923), 1);
            let t0 = add(i, tmp_lag);
            let i = add(add(i, i), i);
            (t0, sub(sub(index, 9), i))
        } else {
            (add(add(sub(index, 12), tmp_lag), 1), 0)
        }
    }
}

fn dec_lag6(
    index: i16,
    pit_min: i16,
    pit_max: i16,
    i_subfr: usize,
    t0: &mut i16,
    t0_frac: &mut i16,
) {
    if i_subfr == 0 {
        if index < 463 {
            *t0 = add(mult(add(index, 5), 5462), 17);
            let i = add(add(*t0, *t0), *t0);
            *t0_frac = add(sub(index, add(i, i)), 105);
        } else {
            *t0 = sub(index, 368);
            *t0_frac = 0;
        }
    } else {
        let mut t0_min = sub(*t0, 5);
        if t0_min < pit_min {
            t0_min = pit_min;
        }
        let mut t0_max = add(t0_min, 9);
        if t0_max > pit_max {
            t0_max = pit_max;
            t0_min = sub(t0_max, 9);
        }
        let _ = t0_max;
        let i = sub(mult(add(index, 5), 5462), 1);
        *t0 = add(i, t0_min);
        let i = add(add(i, i), i);
        *t0_frac = sub(sub(index, 3), add(i, i));
    }
}

fn set_pulses(cod: &mut [i16; L_SUBFR], pos: &[i16], mut sign: i16) {
    cod.fill(0);
    for &p in pos {
        cod[p as usize] = if sign & 1 != 0 { 8191 } else { -8192 };
        sign >>= 1;
    }
}

fn decode_2i40_9bits(sub_nr: usize, sign: i16, index: i16, cod: &mut [i16; L_SUBFR]) {
    let j = ((index & 64) >> 6) as usize;
    let sp = &t::START_POS;
    let i = index & 7;
    let p0 = add(i * 5, sp[j * 8 + sub_nr * 2]);
    let i = (index >> 3) & 7;
    let p1 = add(i * 5, sp[j * 8 + sub_nr * 2 + 1]);
    set_pulses(cod, &[p0, p1], sign);
}

fn decode_2i40_11bits(sign: i16, index: i16, cod: &mut [i16; L_SUBFR]) {
    let j = index & 1;
    let index = index >> 1;
    let i = index & 7;
    let p0 = i * 5 + 1 + j * 2;
    let index = index >> 3;
    let j = index & 3;
    let index = index >> 2;
    let i = index & 7;
    let p1 = if j == 3 { i * 5 + 4 } else { i * 5 + j };
    set_pulses(cod, &[p0, p1], sign);
}

fn decode_3i40_14bits(sign: i16, index: i16, cod: &mut [i16; L_SUBFR]) {
    let i = index & 7;
    let p0 = i * 5;
    let index = index >> 3;
    let j = index & 1;
    let index = index >> 1;
    let i = index & 7;
    let p1 = i * 5 + 1 + j * 2;
    let index = index >> 3;
    let j = index & 1;
    let index = index >> 1;
    let i = index & 7;
    let p2 = i * 5 + 2 + j * 2;
    set_pulses(cod, &[p0, p1, p2], sign);
}

fn decode_4i40_17bits(sign: i16, index: i16, cod: &mut [i16; L_SUBFR]) {
    let g = &t::DGRAY;
    let i = g[(index & 7) as usize];
    let p0 = i * 5;
    let index = index >> 3;
    let i = g[(index & 7) as usize];
    let p1 = i * 5 + 1;
    let index = index >> 3;
    let i = g[(index & 7) as usize];
    let p2 = i * 5 + 2;
    let index = index >> 3;
    let j = index & 1;
    let index = index >> 1;
    let i = g[(index & 7) as usize];
    let p3 = i * 5 + 3 + j;
    set_pulses(cod, &[p0, p1, p2, p3], sign);
}

fn decompress10(msbs: i16, lsbs: i16, i1: usize, i2: usize, i3: usize, pos: &mut [i16; 8]) {
    let msbs = msbs.min(124);
    let ia = mult(msbs, 1311);
    let ia = sub(msbs, extract_l(l_shr(l_mult(ia, 25), 1)));
    let ib = shl(sub(ia, extract_l(l_shr(l_mult(mult(ia, 6554), 5), 1))), 1);
    let ic = sub(lsbs, shl(shr(lsbs, 2), 2));
    pos[i1] = add(ib, ic & 1);
    let ib = shl(mult(ia, 6554), 1);
    pos[i2] = add(ib, shr(ic, 1));
    pos[i3] = add(shl(mult(msbs, 1311), 1), shr(lsbs, 2));
}

fn dec_8i40_31bits(index: &[i16], cod: &mut [i16; L_SUBFR]) {
    cod.fill(0);
    let mut signs = [0i16; 4];
    let mut pos = [0i16; 8];
    signs.copy_from_slice(&index[..4]);
    decompress10(shr(index[4], 3), index[4] & 7, 0, 4, 1, &mut pos);
    decompress10(shr(index[5], 3), index[5] & 7, 2, 6, 5, &mut pos);
    let msbs = shr(index[6], 2);
    let lsbs = index[6] & 3;
    let m024 = shr(add(extract_l(l_shr(l_mult(msbs, 25), 1)), 12), 5);
    let ia = mult(m024, 6554) & 1;
    let mut ib = sub(m024, extract_l(l_shr(l_mult(mult(m024, 6554), 5), 1)));
    if ia == 1 {
        ib = sub(4, ib);
    }
    pos[3] = add(shl(ib, 1), lsbs & 1);
    let ia = shl(mult(m024, 6554), 1);
    pos[7] = add(ia, shr(lsbs, 1));
    for j in 0..4 {
        let p1 = (pos[j] as usize) * 4 + j;
        let mut sign: i16 = if signs[j] == 0 { 8191 } else { -8191 };
        cod[p1] = sign;
        let p2 = (pos[j + 4] as usize) * 4 + j;
        if p2 < p1 {
            sign = negate(sign);
        }
        cod[p2] = add(cod[p2], sign);
    }
}

fn dec_10i40_35bits(index: &[i16], cod: &mut [i16; L_SUBFR]) {
    cod.fill(0);
    let g = &t::DGRAY;
    for j in 0..5 {
        let tmp = index[j];
        let p1 = g[(tmp & 7) as usize] as usize * 5 + j;
        let mut sign: i16 = if (tmp >> 3) & 1 == 0 { 4096 } else { -4096 };
        cod[p1] = sign;
        let p2 = g[(index[j + 5] & 7) as usize] as usize * 5 + j;
        if p2 < p1 {
            sign = negate(sign);
        }
        cod[p2] = add(cod[p2], sign);
    }
}

// --- gains ---

const NPRED: usize = 4;
const MIN_ENERGY: i16 = -14336;
const MIN_ENERGY_MR122: i16 = -2381;

#[derive(Clone)]
pub(crate) struct GcPred {
    pub past_qua_en: [i16; NPRED],
    pub past_qua_en_mr122: [i16; NPRED],
}

impl GcPred {
    fn new() -> Self {
        GcPred {
            past_qua_en: [MIN_ENERGY; NPRED],
            past_qua_en_mr122: [MIN_ENERGY_MR122; NPRED],
        }
    }

    /// Predicted codebook gain (exponent, fraction); for MR795 also the innovation energy.
    fn pred(&self, mode: u8, code: &[i16]) -> (i16, i16, i16, i16) {
        const PRED: [i16; 4] = [5571, 4751, 2785, 1556];
        const PRED_MR122: [i16; 4] = [44, 37, 22, 12];
        let mut ener_code = l_mac(0, code[0], code[0]);
        for &c in &code[1..L_SUBFR] {
            ener_code = l_mac(ener_code, c, c);
        }
        if mode == MR122 {
            let ener_code = l_mult(round(ener_code), 26214);
            let (e, f) = log2(ener_code);
            let ener_code = l_comp(sub(e, 30), f);
            let mut ener: i32 = 783_741;
            for i in 0..NPRED {
                ener = l_mac(ener, self.past_qua_en_mr122[i], PRED_MR122[i]);
            }
            let ener = l_shr(l_sub(ener, ener_code), 1);
            let (e, f) = l_extract(ener);
            (e, f, 0, 0)
        } else {
            let exp_code = norm_l(ener_code);
            let ener_code = l_shl(ener_code, exp_code);
            let (e, f) = log2_norm(ener_code, exp_code);
            let mut l = mpy_32_16(e, f, -24660);
            let (mut exp_en, mut frac_en) = (0, 0);
            l = match mode {
                MR102 => l_mac(l, 16678, 64),
                MR795 => {
                    frac_en = extract_h(ener_code);
                    exp_en = sub(-11, exp_code);
                    l_mac(l, 17062, 64)
                }
                MR74 => l_mac(l, 32588, 32),
                MR67 => l_mac(l, 32268, 32),
                _ => l_mac(l, 16678, 64),
            };
            l = l_shl(l, 10);
            for i in 0..4 {
                l = l_mac(l, PRED[i], self.past_qua_en[i]);
            }
            let gcode0 = extract_h(l);
            let l = if mode == MR74 {
                l_mult(gcode0, 5439)
            } else {
                l_mult(gcode0, 5443)
            };
            let (e, f) = l_extract(l_shr(l, 8));
            (e, f, exp_en, frac_en)
        }
    }

    fn update(&mut self, qua_ener_mr122: i16, qua_ener: i16) {
        for i in (1..4).rev() {
            self.past_qua_en[i] = self.past_qua_en[i - 1];
            self.past_qua_en_mr122[i] = self.past_qua_en_mr122[i - 1];
        }
        self.past_qua_en_mr122[0] = qua_ener_mr122;
        self.past_qua_en[0] = qua_ener;
    }

    fn average_limited(&self) -> (i16, i16) {
        let mut av: i16 = 0;
        for &v in &self.past_qua_en_mr122 {
            av = add(av, v);
        }
        let a1 = mult(av, 8192).max(MIN_ENERGY_MR122);
        let mut av: i16 = 0;
        for &v in &self.past_qua_en {
            av = add(av, v);
        }
        let a2 = mult(av, 8192).max(MIN_ENERGY);
        (a1, a2)
    }
}

fn d_gain_pitch(mode: u8, index: i16) -> i16 {
    let g = t::QUA_GAIN_PITCH[index as usize];
    if mode == MR122 { shl(shr(g, 2), 2) } else { g }
}

fn d_gain_code(pred: &mut GcPred, mode: u8, index: i16, code: &[i16]) -> i16 {
    let (exp, frac, _, _) = pred.pred(mode, code);
    let p = &t::QUA_GAIN_CODE[index as usize * 3..index as usize * 3 + 3];
    let gain = if mode == MR122 {
        let gcode0 = shl(extract_l(pow2(exp, frac)), 4);
        shl(mult(gcode0, p[0]), 1)
    } else {
        let gcode0 = extract_l(pow2(14, frac));
        let l = l_shr(l_mult(p[0], gcode0), sub(9, exp));
        extract_h(l)
    };
    pred.update(p[1], p[2]);
    gain
}

fn dec_gain(pred: &mut GcPred, mode: u8, index: i16, code: &[i16], even_subfr: bool) -> (i16, i16) {
    let index = (index as usize) << 2;
    let (gain_pit, g_code, qua_ener_mr122, qua_ener);
    if mode == MR102 || mode == MR74 || mode == MR67 {
        let p = &t::TABLE_GAIN_HIGHRATES[index..index + 4];
        (gain_pit, g_code, qua_ener_mr122, qua_ener) = (p[0], p[1], p[2], p[3]);
    } else if mode == MR475 {
        let index = index + if even_subfr { 0 } else { 2 };
        let p = &t::TABLE_GAIN_MR475[index..index + 2];
        gain_pit = p[0];
        g_code = p[1];
        let (exp, frac) = log2(l_deposit_l(g_code));
        let exp = sub(exp, 12);
        qua_ener_mr122 = add(shr_r(frac, 5), shl(exp, 10));
        let l = mpy_32_16(exp, frac, 24660);
        qua_ener = round(l_shl(l, 13));
    } else {
        let p = &t::TABLE_GAIN_LOWRATES[index..index + 4];
        (gain_pit, g_code, qua_ener_mr122, qua_ener) = (p[0], p[1], p[2], p[3]);
    }
    let (exp, frac, _, _) = pred.pred(mode, code);
    let gcode0 = extract_l(pow2(14, frac));
    let l = l_shr(l_mult(g_code, gcode0), sub(10, exp));
    pred.update(qua_ener_mr122, qua_ener);
    (gain_pit, extract_h(l))
}

#[derive(Clone)]
struct EcGainPitch {
    pbuf: [i16; 5],
    past_gain_pit: i16,
    prev_gp: i16,
}

impl EcGainPitch {
    fn new() -> Self {
        EcGainPitch {
            pbuf: [1640; 5],
            past_gain_pit: 0,
            prev_gp: 16384,
        }
    }
    fn conceal(&self, state: usize) -> i16 {
        const PDOWN: [i16; 7] = [32767, 32112, 32112, 26214, 9830, 6553, 6553];
        let tmp = gmed_n(&self.pbuf, 5).min(self.past_gain_pit);
        mult(tmp, PDOWN[state])
    }
    fn update(&mut self, bfi: bool, prev_bf: bool, gain: &mut i16) {
        if !bfi {
            if prev_bf && *gain > self.prev_gp {
                *gain = self.prev_gp;
            }
            self.prev_gp = *gain;
        }
        self.past_gain_pit = (*gain).min(16384);
        self.pbuf.copy_within(1..5, 0);
        self.pbuf[4] = self.past_gain_pit;
    }
}

#[derive(Clone)]
struct EcGainCode {
    gbuf: [i16; 5],
    past_gain_code: i16,
    prev_gc: i16,
}

impl EcGainCode {
    fn new() -> Self {
        EcGainCode {
            gbuf: [1; 5],
            past_gain_code: 0,
            prev_gc: 1,
        }
    }
    fn conceal(&self, pred: &mut GcPred, state: usize) -> i16 {
        const CDOWN: [i16; 7] = [32767, 32112, 32112, 32112, 32112, 32112, 22937];
        let tmp = gmed_n(&self.gbuf, 5).min(self.past_gain_code);
        let g = mult(tmp, CDOWN[state]);
        let (a, b) = pred.average_limited();
        pred.update(a, b);
        g
    }
    fn update(&mut self, bfi: bool, prev_bf: bool, gain: &mut i16) {
        if !bfi {
            if prev_bf && *gain > self.prev_gc {
                *gain = self.prev_gc;
            }
            self.prev_gc = *gain;
        }
        self.past_gain_code = *gain;
        self.gbuf.copy_within(1..5, 0);
        self.gbuf[4] = *gain;
    }
}

// --- background noise and smoothing ---

const L_CBGAINHIST: usize = 7;

#[derive(Clone)]
pub(crate) struct CbGainAverage {
    cb_gain_history: [i16; L_CBGAINHIST],
    pub hang_var: i16,
    pub hang_count: i16,
}

impl CbGainAverage {
    fn new() -> Self {
        CbGainAverage {
            cb_gain_history: [0; L_CBGAINHIST],
            hang_var: 0,
            hang_count: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn average(
        &mut self,
        mode: u8,
        gain_code: i16,
        lsp: &[i16],
        lsp_aver: &[i16],
        bfi: bool,
        prev_bf: bool,
        pdfi: bool,
        prev_pdf: bool,
        in_bg_noise: bool,
        voiced_hangover: i16,
    ) -> i16 {
        let mut cb_gain_mix = gain_code;
        self.cb_gain_history.copy_within(1.., 0);
        self.cb_gain_history[L_CBGAINHIST - 1] = gain_code;
        let mut diff: i16 = 0;
        for i in 0..M {
            let tmp1 = abs_s(sub(lsp_aver[i], lsp[i]));
            let shift1 = sub(norm_s(tmp1), 1);
            let tmp1 = shl(tmp1, shift1);
            let shift2 = norm_s(lsp_aver[i]);
            let tmp2 = shl(lsp_aver[i], shift2);
            let mut v = div_s(tmp1, tmp2);
            let shift = sub(add(2, shift1), shift2);
            v = if shift >= 0 {
                shr(v, shift)
            } else {
                shl(v, negate(shift))
            };
            diff = if i == 0 { v } else { add(diff, v) };
        }
        if diff > 5325 {
            self.hang_var = add(self.hang_var, 1);
        } else {
            self.hang_var = 0;
        }
        if self.hang_var > 10 {
            self.hang_count = 0;
        }
        let low3 = mode == MR475 || mode == MR515 || mode == MR59;
        if mode <= MR67 || mode == MR102 {
            let thr = if ((pdfi && prev_pdf) || bfi || prev_bf)
                && voiced_hangover > 1
                && in_bg_noise
                && low3
            {
                4506
            } else {
                3277
            };
            let tmp1 = sub(diff, thr).max(0);
            let mut bg_mix = if tmp1 > 2048 { 8192 } else { shl(tmp1, 2) };
            if self.hang_count < 40 || diff > 5325 {
                bg_mix = 8192;
            }
            let h = &self.cb_gain_history;
            let mut l = l_mult(6554, h[2]);
            for &v in &h[3..] {
                l = l_mac(l, 6554, v);
            }
            let mut mean = round(l);
            if (bfi || prev_bf) && in_bg_noise && low3 {
                let mut l = l_mult(4681, h[0]);
                for &v in &h[1..] {
                    l = l_mac(l, 4681, v);
                }
                mean = round(l);
            }
            let mut l = l_mult(bg_mix, cb_gain_mix);
            l = l_mac(l, 8192, mean);
            l = l_msu(l, bg_mix, mean);
            cb_gain_mix = round(l_shl(l, 2));
        }
        self.hang_count = add(self.hang_count, 1);
        cb_gain_mix
    }
}

const L_ENERGYHIST: usize = 60;

#[derive(Clone)]
struct BgnScd {
    frame_energy_hist: [i16; L_ENERGYHIST],
    bg_hangover: i16,
}

impl BgnScd {
    fn new() -> Self {
        BgnScd {
            frame_energy_hist: [0; L_ENERGYHIST],
            bg_hangover: 0,
        }
    }

    fn detect(
        &mut self,
        ltp_gain_hist: &[i16; 9],
        speech: &[i16],
        voiced_hangover: &mut i16,
    ) -> bool {
        let mut s = 0i32;
        for &v in &speech[..L_FRAME] {
            s = l_mac(s, v, v);
        }
        let curr = extract_h(l_shl(s, 2));
        let h = &self.frame_energy_hist;
        let mut emin: i16 = 32767;
        for &v in h.iter() {
            if v < emin {
                emin = v;
            }
        }
        let noise_floor = shl(emin, 4);
        let mut emax = h[0];
        for &v in &h[1..L_ENERGYHIST - 4] {
            if emax < v {
                emax = v;
            }
        }
        let mut emax_last = h[2 * L_ENERGYHIST / 3];
        for &v in &h[2 * L_ENERGYHIST / 3 + 1..] {
            if emax_last < v {
                emax_last = v;
            }
        }
        if emax > 20 && curr < 17578 && curr > 20 && (curr < noise_floor || emax_last < 1953) {
            self.bg_hangover = if add(self.bg_hangover, 1) > 30 {
                30
            } else {
                add(self.bg_hangover, 1)
            };
        } else {
            self.bg_hangover = 0;
        }
        let in_bg = self.bg_hangover > 1;
        self.frame_energy_hist.copy_within(1.., 0);
        self.frame_energy_hist[L_ENERGYHIST - 1] = curr;
        let mut ltp_limit = 13926;
        if self.bg_hangover > 8 {
            ltp_limit = 15565;
        }
        if self.bg_hangover > 15 {
            ltp_limit = 16383;
        }
        let mut prev_voiced = gmed_n(&ltp_gain_hist[4..], 5) > ltp_limit;
        if self.bg_hangover > 20 {
            prev_voiced = gmed_n(ltp_gain_hist, 9) > ltp_limit;
        }
        if prev_voiced {
            *voiced_hangover = 0;
        } else {
            *voiced_hangover = add(*voiced_hangover, 1).min(10);
        }
        in_bg
    }
}

const PHDGAINMEMSIZE: usize = 5;

#[derive(Clone)]
struct PhDisp {
    gain_mem: [i16; PHDGAINMEMSIZE],
    prev_state: i16,
    prev_cb_gain: i16,
    lock_full: bool,
    onset: i16,
}

impl PhDisp {
    fn new() -> Self {
        PhDisp {
            gain_mem: [0; PHDGAINMEMSIZE],
            prev_state: 0,
            prev_cb_gain: 0,
            lock_full: false,
            onset: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        mode: u8,
        x: &mut [i16],
        cb_gain: i16,
        ltp_gain: i16,
        inno: &mut [i16; L_SUBFR],
        pitch_fac: i16,
        tmp_shift: i16,
    ) {
        const THR1: i16 = 9830;
        const THR2: i16 = 14746;
        self.gain_mem.copy_within(0..PHDGAINMEMSIZE - 1, 1);
        self.gain_mem[0] = ltp_gain;
        let mut imp_nr: i16 = if ltp_gain < THR2 {
            if ltp_gain > THR1 { 1 } else { 0 }
        } else {
            2
        };
        let tmp1 = round(l_shl(l_mult(self.prev_cb_gain, 16384), 2));
        if cb_gain > tmp1 {
            self.onset = 2;
        } else if self.onset > 0 {
            self.onset -= 1;
        }
        if self.onset == 0 {
            let i1 = self.gain_mem.iter().filter(|&&g| g < THR1).count();
            if i1 > 2 {
                imp_nr = 0;
            }
        }
        if imp_nr > add(self.prev_state, 1) && self.onset == 0 {
            imp_nr -= 1;
        }
        if imp_nr < 2 && self.onset > 0 {
            imp_nr += 1;
        }
        if cb_gain < 10 {
            imp_nr = 2;
        }
        if self.lock_full {
            imp_nr = 0;
        }
        self.prev_state = imp_nr;
        self.prev_cb_gain = cb_gain;
        if mode != MR122 && mode != MR102 && mode != MR74 && imp_nr < 2 {
            let mut ps_poss = [0usize; L_SUBFR];
            let mut nze = 0;
            let inno_sav = *inno;
            for i in 0..L_SUBFR {
                if inno[i] != 0 {
                    ps_poss[nze] = i;
                    nze += 1;
                }
                inno[i] = 0;
            }
            let ph_imp: &[i16; 40] = match (mode == MR795, imp_nr == 0) {
                (true, true) => &t::PH_IMP_LOW_MR795,
                (true, false) => &t::PH_IMP_MID_MR795,
                (false, true) => &t::PH_IMP_LOW,
                (false, false) => &t::PH_IMP_MID,
            };
            for &ppos in &ps_poss[..nze] {
                for (j, i) in (ppos..L_SUBFR).chain(0..ppos).enumerate() {
                    inno[i] = add(inno[i], mult(inno_sav[ppos], ph_imp[j]));
                }
            }
        }
        for i in 0..L_SUBFR {
            let l = l_mult(x[i], pitch_fac);
            let l = l_mac(l, inno[i], cb_gain);
            x[i] = round(l_shl(l, tmp_shift));
        }
    }
}

fn ex_ctrl(
    exc: &mut [i16],
    exc_energy: i16,
    hist: &[i16; 9],
    voiced_hangover: i16,
    prev_bfi: bool,
    careful: bool,
) {
    let mut avg = gmed_n(hist, 9);
    let mut prev = shr(add(hist[7], hist[8]), 1);
    if hist[8] < prev {
        prev = hist[8];
    }
    if exc_energy < avg && exc_energy > 5 {
        let mut test = shl(prev, 2);
        if voiced_hangover < 7 || prev_bfi {
            test = sub(test, prev);
        }
        if avg > test {
            avg = test;
        }
        let exp = norm_s(exc_energy);
        let e = div_s(16383, shl(exc_energy, exp));
        let t0 = l_shr(l_mult(avg, e), sub(20, exp)).min(32767);
        let mut scale = extract_l(t0);
        if careful && scale > 3072 {
            scale = 3072;
        }
        for v in exc.iter_mut().take(L_SUBFR) {
            *v = extract_l(l_shr(l_mult(scale, *v), 11));
        }
    }
}

fn energy_new(x: &[i16]) -> i32 {
    let mut s = l_mult(x[0], x[0]);
    for &v in &x[1..] {
        s = l_mac(s, v, v);
    }
    if s == MAX_32 {
        let tmp = shr(x[0], 2);
        let mut s = l_mult(tmp, tmp);
        for &v in &x[1..] {
            let tmp = shr(v, 2);
            s = l_mac(s, tmp, tmp);
        }
        s
    } else {
        l_shr(s, 4)
    }
}

/// Gain factor of the automatic gain control: sqrt(energy(out) / energy(in)) in Q?.
fn agc_g0(sig_in: &[i16], sig_out: &[i16]) -> Option<i16> {
    let s = energy_new(sig_out);
    if s == 0 {
        return None;
    }
    let exp = sub(norm_l(s), 1);
    let gain_out = round(l_shl(s, exp));
    let s = energy_new(sig_in);
    if s == 0 {
        return Some(0);
    }
    let i = norm_l(s);
    let gain_in = round(l_shl(s, i));
    let exp = sub(exp, i);
    let s = l_deposit_l(div_s(gain_out, gain_in));
    let s = l_shr(l_shl(s, 7), exp);
    let s = inv_sqrt(s);
    Some(round(l_shl(s, 9)))
}

fn agc2(sig_in: &[i16], sig_out: &mut [i16]) {
    let Some(g0) = agc_g0(sig_in, sig_out) else {
        return;
    };
    for v in sig_out.iter_mut() {
        *v = extract_h(l_shl(l_mult(*v, g0), 3));
    }
}

#[derive(Clone)]
struct LspAvg {
    lsp_mean_save: [i16; M],
}

impl LspAvg {
    fn new() -> Self {
        LspAvg {
            lsp_mean_save: t::MEAN_LSF_5,
        }
    }
    fn update(&mut self, lsp: &[i16]) {
        const EXPCONST: i16 = 5243;
        for i in 0..M {
            let l = l_deposit_h(self.lsp_mean_save[i]);
            let l = l_msu(l, EXPCONST, self.lsp_mean_save[i]);
            let l = l_mac(l, EXPCONST, lsp[i]);
            self.lsp_mean_save[i] = round(l);
        }
    }
}

fn int_lsf(old: &[i16], new: &[i16], i_subfr: usize, out: &mut [i16; M]) {
    for i in 0..M {
        out[i] = match i_subfr {
            0 => add(sub(old[i], shr(old[i], 2)), shr(new[i], 2)),
            40 => add(shr(old[i], 1), shr(new[i], 1)),
            80 => add(shr(old[i], 2), sub(new[i], shr(new[i], 2))),
            _ => new[i],
        };
    }
}

/// Comfort-noise parameters for a lost or empty frame (pseudo-random, from a window).
fn build_cn_param(seed: &mut i16, mode: u8, parm: &mut [i16]) {
    *seed = extract_l(l_add(l_shr(l_mult(*seed, 31821), 1), 13849));
    let start = (*seed & 0x7f) as usize;
    let bits = bitno(mode);
    for (i, &n) in bits.iter().enumerate() {
        parm[i] = t::WINDOW_200_40[start + i] & !((0xffffu16 << n) as i16);
    }
}

// ---------------------------------------------------------------------------------------
// The decoder
// ---------------------------------------------------------------------------------------

#[derive(Clone)]
struct DecoderAmr {
    old_exc: [i16; OLD_EXC_LEN],
    lsp_old: [i16; M],
    mem_syn: [i16; M],
    sharp: i16,
    old_t0: i16,
    prev_bf: bool,
    prev_pdf: bool,
    state: i16,
    exc_energy_hist: [i16; 9],
    t0_lag_buff: i16,
    in_bg_noise: bool,
    voiced_hangover: i16,
    ltp_gain_history: [i16; 9],
    background: BgnScd,
    nodata_seed: i16,
    cb_gain_aver: CbGainAverage,
    lsp_avg: LspAvg,
    lsf: DPlsf,
    ec_p: EcGainPitch,
    ec_c: EcGainCode,
    pred: GcPred,
    ph_disp: PhDisp,
    dtx: dtx::DtxDec,
}

impl DecoderAmr {
    fn new() -> Self {
        DecoderAmr {
            old_exc: [0; OLD_EXC_LEN],
            lsp_old: t::LSP_INIT_DATA,
            mem_syn: [0; M],
            sharp: SHARPMIN,
            old_t0: 40,
            prev_bf: false,
            prev_pdf: false,
            state: 0,
            exc_energy_hist: [0; 9],
            t0_lag_buff: 40,
            in_bg_noise: false,
            voiced_hangover: 0,
            ltp_gain_history: [0; 9],
            background: BgnScd::new(),
            nodata_seed: 21845,
            cb_gain_aver: CbGainAverage::new(),
            lsp_avg: LspAvg::new(),
            lsf: DPlsf::new(),
            ec_p: EcGainPitch::new(),
            ec_c: EcGainCode::new(),
            pred: GcPred::new(),
            ph_disp: PhDisp::new(),
            dtx: dtx::DtxDec::new(),
        }
    }

    /// The reset done on entering comfort noise: everything but the synthesis memory,
    /// LSPs, excitation energies, LSP average, gain predictor and DTX state.
    fn reset_for_dtx(&mut self) {
        self.old_exc = [0; OLD_EXC_LEN];
        self.sharp = SHARPMIN;
        self.old_t0 = 40;
        self.prev_bf = false;
        self.prev_pdf = false;
        self.state = 0;
        self.t0_lag_buff = 40;
        self.in_bg_noise = false;
        self.voiced_hangover = 0;
        self.ltp_gain_history = [0; 9];
        self.cb_gain_aver = CbGainAverage::new();
        self.lsf = DPlsf::new();
        self.ec_p = EcGainPitch::new();
        self.ec_c = EcGainCode::new();
        self.background = BgnScd::new();
        self.nodata_seed = 21845;
        self.ph_disp = PhDisp::new();
    }

    fn decode(
        &mut self,
        mode: u8,
        parm_in: &[i16],
        frame_type: RxType,
        synth: &mut [i16; L_FRAME],
        a_t: &mut [i16; 4 * MP1],
    ) {
        let mut parm = [0i16; 58];
        parm[..parm_in.len().min(58)].copy_from_slice(&parm_in[..parm_in.len().min(58)]);
        let new_state = self.dtx.rx_handler(frame_type);
        if new_state != dtx::State::Speech {
            self.reset_for_dtx();
            self.dtx.decode(
                &mut self.mem_syn,
                &mut self.lsf,
                &mut self.pred,
                &mut self.cb_gain_aver,
                new_state,
                mode,
                &parm,
                synth,
                a_t,
            );
            lsf_lsp(&self.lsf.past_lsf_q, &mut self.lsp_old);
            self.lsp_avg.update(&self.lsf.past_lsf_q);
            self.dtx.global_state = new_state;
            return;
        }

        let mut bfi = false;
        let mut pdfi = false;
        match frame_type {
            RxType::SpeechBad | RxType::NoData | RxType::Onset => {
                bfi = true;
                if matches!(frame_type, RxType::NoData | RxType::Onset) {
                    build_cn_param(&mut self.nodata_seed, mode, &mut parm);
                }
            }
            RxType::SpeechDegraded => pdfi = true,
            _ => {}
        }
        if bfi {
            self.state = add(self.state, 1);
        } else if self.state == 6 {
            self.state = 5;
        } else {
            self.state = 0;
        }
        if self.state > 6 {
            self.state = 6;
        }
        match self.dtx.global_state {
            dtx::State::Dtx => {
                self.state = 5;
                self.prev_bf = false;
            }
            dtx::State::DtxMute => {
                self.state = 5;
                self.prev_bf = true;
            }
            dtx::State::Speech => {}
        }
        let st_idx = self.state as usize;

        let prev_lsf = self.lsf.past_lsf_q;
        let mut lsp_new = [0i16; M];
        let mut lsp_mid = [0i16; M];
        let mut p = 0usize;
        if mode != MR122 {
            self.lsf.d_plsf_3(mode, bfi, &parm[0..3], &mut lsp_new);
            p += 3;
            int_lpc_1to3(&self.lsp_old, &lsp_new, a_t);
        } else {
            self.lsf
                .d_plsf_5(bfi, &parm[0..5], &mut lsp_mid, &mut lsp_new);
            p += 5;
            int_lpc_1and3(&self.lsp_old, &lsp_mid, &lsp_new, a_t);
        }
        self.lsp_old = lsp_new;

        let low3 = mode == MR475 || mode == MR515 || mode == MR59;
        let mut even_subfr = false;
        let mut index_mr475: i16 = 0;
        let mut t0: i16 = 0;
        let mut t0_frac: i16 = 0;
        let mut code = [0i16; L_SUBFR];
        let mut excp = [0i16; L_SUBFR];
        for (subfr_nr, i_subfr) in (0..L_FRAME).step_by(L_SUBFR).enumerate() {
            let az = &a_t[subfr_nr * MP1..subfr_nr * MP1 + MP1];
            even_subfr = !even_subfr;
            let mut pit_flag = i_subfr;
            if i_subfr == L_FRAME / 2 && mode != MR475 && mode != MR515 {
                pit_flag = 0;
            }
            let index = parm[p];
            p += 1;
            if mode != MR122 {
                let flag4 = mode == MR475 || mode == MR515 || mode == MR59 || mode == MR67;
                let (dlow, drange) = if mode == MR795 { (10, 19) } else { (5, 9) };
                let mut t0_min = sub(self.old_t0, dlow);
                if t0_min < PIT_MIN {
                    t0_min = PIT_MIN;
                }
                let mut t0_max = add(t0_min, drange);
                if t0_max > PIT_MAX {
                    t0_max = PIT_MAX;
                    t0_min = sub(t0_max, drange);
                }
                (t0, t0_frac) = dec_lag3(index, t0_min, t0_max, pit_flag, self.old_t0, flag4);
                self.t0_lag_buff = t0;
                if bfi {
                    if self.old_t0 < PIT_MAX {
                        self.old_t0 = add(self.old_t0, 1);
                    }
                    t0 = self.old_t0;
                    t0_frac = 0;
                    if self.in_bg_noise && self.voiced_hangover > 4 && low3 {
                        t0 = self.t0_lag_buff;
                    }
                }
                pred_lt_3or6(&mut self.old_exc, EXC_OFF, t0, t0_frac, true);
            } else {
                dec_lag6(
                    index,
                    PIT_MIN_MR122,
                    PIT_MAX,
                    pit_flag,
                    &mut t0,
                    &mut t0_frac,
                );
                if !(!bfi && (pit_flag == 0 || index < 61)) {
                    self.t0_lag_buff = t0;
                    t0 = self.old_t0;
                    t0_frac = 0;
                }
                pred_lt_3or6(&mut self.old_exc, EXC_OFF, t0, t0_frac, false);
            }

            let mut gain_pit: i16 = 0;
            let pit_sharp: i16;
            match mode {
                MR475 | MR515 => {
                    let (index, i) = (parm[p], parm[p + 1]);
                    p += 2;
                    decode_2i40_9bits(subfr_nr, i, index, &mut code);
                    pit_sharp = shl(self.sharp, 1);
                }
                MR59 => {
                    let (index, i) = (parm[p], parm[p + 1]);
                    p += 2;
                    decode_2i40_11bits(i, index, &mut code);
                    pit_sharp = shl(self.sharp, 1);
                }
                MR67 => {
                    let (index, i) = (parm[p], parm[p + 1]);
                    p += 2;
                    decode_3i40_14bits(i, index, &mut code);
                    pit_sharp = shl(self.sharp, 1);
                }
                MR74 | MR795 => {
                    let (index, i) = (parm[p], parm[p + 1]);
                    p += 2;
                    decode_4i40_17bits(i, index, &mut code);
                    pit_sharp = shl(self.sharp, 1);
                }
                MR102 => {
                    dec_8i40_31bits(&parm[p..p + 7], &mut code);
                    p += 7;
                    pit_sharp = shl(self.sharp, 1);
                }
                _ => {
                    let index = parm[p];
                    p += 1;
                    gain_pit = if bfi {
                        self.ec_p.conceal(st_idx)
                    } else {
                        d_gain_pitch(mode, index)
                    };
                    self.ec_p.update(bfi, self.prev_bf, &mut gain_pit);
                    dec_10i40_35bits(&parm[p..p + 10], &mut code);
                    p += 10;
                    pit_sharp = shl(gain_pit, 1);
                }
            }
            if t0 >= 0 {
                for i in t0 as usize..L_SUBFR {
                    let temp = mult(code[i - t0 as usize], pit_sharp);
                    code[i] = add(code[i], temp);
                }
            }

            let mut gain_code: i16;
            let mut pit_sharp: i16;
            if mode == MR475 {
                if even_subfr {
                    index_mr475 = parm[p];
                    p += 1;
                }
                if !bfi {
                    (gain_pit, gain_code) =
                        dec_gain(&mut self.pred, mode, index_mr475, &code, even_subfr);
                } else {
                    gain_pit = self.ec_p.conceal(st_idx);
                    gain_code = self.ec_c.conceal(&mut self.pred, st_idx);
                }
                self.ec_p.update(bfi, self.prev_bf, &mut gain_pit);
                self.ec_c.update(bfi, self.prev_bf, &mut gain_code);
                pit_sharp = gain_pit.min(SHARPMAX);
            } else if mode <= MR74 || mode == MR102 {
                let index = parm[p];
                p += 1;
                if !bfi {
                    (gain_pit, gain_code) =
                        dec_gain(&mut self.pred, mode, index, &code, even_subfr);
                } else {
                    gain_pit = self.ec_p.conceal(st_idx);
                    gain_code = self.ec_c.conceal(&mut self.pred, st_idx);
                }
                self.ec_p.update(bfi, self.prev_bf, &mut gain_pit);
                self.ec_c.update(bfi, self.prev_bf, &mut gain_code);
                pit_sharp = gain_pit.min(SHARPMAX);
                if mode == MR102 && self.old_t0 > add(L_SUBFR as i16, 5) {
                    pit_sharp = shr(pit_sharp, 2);
                }
            } else {
                let index = parm[p];
                p += 1;
                if mode == MR795 {
                    gain_pit = if bfi {
                        self.ec_p.conceal(st_idx)
                    } else {
                        d_gain_pitch(mode, index)
                    };
                    self.ec_p.update(bfi, self.prev_bf, &mut gain_pit);
                    let index = parm[p];
                    p += 1;
                    gain_code = if !bfi {
                        d_gain_code(&mut self.pred, mode, index, &code)
                    } else {
                        self.ec_c.conceal(&mut self.pred, st_idx)
                    };
                    self.ec_c.update(bfi, self.prev_bf, &mut gain_code);
                    pit_sharp = gain_pit.min(SHARPMAX);
                } else {
                    gain_code = if !bfi {
                        d_gain_code(&mut self.pred, mode, index, &code)
                    } else {
                        self.ec_c.conceal(&mut self.pred, st_idx)
                    };
                    self.ec_c.update(bfi, self.prev_bf, &mut gain_code);
                    pit_sharp = gain_pit;
                }
            }
            if mode != MR475 || !even_subfr {
                self.sharp = gain_pit.min(SHARPMAX);
            }
            pit_sharp = shl(pit_sharp, 1);
            if pit_sharp > 16384 {
                for i in 0..L_SUBFR {
                    let temp = mult(self.old_exc[EXC_OFF + i], pit_sharp);
                    let mut l = l_mult(temp, gain_pit);
                    if mode == MR122 {
                        l = l_shr(l, 1);
                    }
                    excp[i] = round(l);
                }
            }
            if !bfi {
                self.ltp_gain_history.copy_within(1.., 0);
                self.ltp_gain_history[8] = gain_pit;
            }
            if (self.prev_bf || bfi) && self.in_bg_noise && low3 {
                if gain_pit > 12288 {
                    gain_pit = add(shr(sub(gain_pit, 12288), 1), 12288);
                }
                if gain_pit > 14745 {
                    gain_pit = 14745;
                }
            }
            let mut lsf_i = [0i16; M];
            int_lsf(&prev_lsf, &self.lsf.past_lsf_q, i_subfr, &mut lsf_i);
            let mut gain_code_mix = self.cb_gain_aver.average(
                mode,
                gain_code,
                &lsf_i,
                &self.lsp_avg.lsp_mean_save,
                bfi,
                self.prev_bf,
                pdfi,
                self.prev_pdf,
                self.in_bg_noise,
                self.voiced_hangover,
            );
            if mode > MR67 && mode != MR102 {
                gain_code_mix = gain_code;
            }
            let (pitch_fac, tmp_shift) = if mode <= MR102 {
                (gain_pit, 1)
            } else {
                (shr(gain_pit, 1), 2)
            };
            let mut exc_enhanced = [0i16; L_SUBFR];
            for i in 0..L_SUBFR {
                let e = self.old_exc[EXC_OFF + i];
                exc_enhanced[i] = e;
                let l = l_mult(e, pitch_fac);
                let l = l_mac(l, code[i], gain_code);
                self.old_exc[EXC_OFF + i] = round(l_shl(l, tmp_shift));
            }
            self.ph_disp.lock_full = false;
            if low3 && self.voiced_hangover > 3 && self.in_bg_noise && bfi {
                self.ph_disp.lock_full = true;
            }
            self.ph_disp.apply(
                mode,
                &mut exc_enhanced,
                gain_code_mix,
                gain_pit,
                &mut code,
                pitch_fac,
                tmp_shift,
            );

            let mut l = 0i32;
            for &v in &exc_enhanced {
                l = l_mac(l, v, v);
            }
            let l = l_shr(l, 1);
            let (l, temp) = sqrt_l_exp(l);
            let l = l_shr(l, add(shr(temp, 1), 15));
            let exc_energy = extract_l(l_shr(l, 2));
            if low3
                && self.voiced_hangover > 5
                && self.in_bg_noise
                && self.state < 4
                && ((pdfi && self.prev_pdf) || bfi || self.prev_bf)
            {
                let careful = pdfi && !bfi;
                ex_ctrl(
                    &mut exc_enhanced,
                    exc_energy,
                    &self.exc_energy_hist,
                    self.voiced_hangover,
                    self.prev_bf,
                    careful,
                );
            }
            if !(self.in_bg_noise && (bfi || self.prev_bf) && self.state < 4) {
                self.exc_energy_hist.copy_within(1.., 0);
                self.exc_energy_hist[8] = exc_energy;
            }

            let out = &mut synth[i_subfr..i_subfr + L_SUBFR];
            let overflow = if pit_sharp > 16384 {
                for i in 0..L_SUBFR {
                    excp[i] = add(excp[i], exc_enhanced[i]);
                }
                agc2(&exc_enhanced, &mut excp);
                syn_filt_overflow(az, &excp, out, &self.mem_syn)
            } else {
                syn_filt_overflow(az, &exc_enhanced, out, &self.mem_syn)
            };
            if overflow {
                for v in self.old_exc.iter_mut() {
                    *v = shr(*v, 2);
                }
                for v in exc_enhanced.iter_mut() {
                    *v = shr(*v, 2);
                }
                syn_filt(az, &exc_enhanced, out, L_SUBFR, &mut self.mem_syn, true);
            } else {
                self.mem_syn.copy_from_slice(&out[L_SUBFR - M..]);
            }
            self.old_exc.copy_within(L_SUBFR.., 0);
            self.old_t0 = t0;
        }
        let _ = even_subfr;
        self.in_bg_noise = self.background.detect(
            &self.ltp_gain_history,
            &synth[..],
            &mut self.voiced_hangover,
        );
        self.dtx.activity_update(&self.lsf.past_lsf_q, &synth[..]);
        self.prev_bf = bfi;
        self.prev_pdf = pdfi;
        self.lsp_avg.update(&self.lsf.past_lsf_q);
        self.dtx.global_state = new_state;
    }
}

/// A complete AMR decoder: parameter decoding, post-filter, high-pass and homing.
#[derive(Clone)]
pub struct Decoder {
    dec: DecoderAmr,
    post: post::PostFilter,
    hp: post::PostProcess,
    prev_mode: u8,
    reset_flag_old: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Self {
        Decoder {
            dec: DecoderAmr::new(),
            post: post::PostFilter::new(),
            hp: post::PostProcess::new(),
            prev_mode: 0,
            reset_flag_old: true,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn homing_test(&self, bits: &[u8], mode: u8, first: bool) -> bool {
        let mut prm = [0i16; 58];
        bits2prm(mode, bits, &mut prm);
        let n = if first {
            t::PRMNOFSF[mode as usize]
        } else {
            t::PRMNO[mode as usize]
        } as usize;
        prm[..n] == dhf(mode)[..n]
    }

    /// Decodes one 20 ms frame to 160 samples (13-bit resolution in 16 bits).
    pub fn decode(&mut self, frame: &Frame, out: &mut [i16; L_FRAME]) {
        let mut mode = frame.mode.min(MR122);
        if frame.rx == RxType::NoData {
            mode = self.prev_mode;
        } else {
            self.prev_mode = mode;
        }
        let bits = &frame.bits[..];
        let mut reset_flag = false;
        if self.reset_flag_old {
            reset_flag = self.homing_test(bits, mode, true);
        }
        if reset_flag && self.reset_flag_old {
            out.fill(0x0008);
        } else {
            let mut prm = [0i16; 58];
            let pm = if matches!(frame.rx, RxType::SidBad | RxType::SidUpdate) {
                MRDTX
            } else {
                mode
            };
            bits2prm(pm, bits, &mut prm);
            let mut a_t = [0i16; 4 * MP1];
            self.dec.decode(mode, &prm, frame.rx, out, &mut a_t);
            self.post.filter(mode, out, &a_t);
            self.hp.process(out);
            for v in out.iter_mut() {
                *v &= !7;
            }
        }
        if !self.reset_flag_old {
            reset_flag = self.homing_test(bits, mode, false);
        }
        if reset_flag {
            let keep = self.prev_mode;
            *self = Self::new();
            self.prev_mode = keep;
            self.reset_flag_old = true;
            // the reference resets the decoder state but keeps its homing memory
            self.prev_mode = 0;
        }
        self.reset_flag_old = reset_flag;
    }
}
