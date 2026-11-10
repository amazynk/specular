//! AMR-WB speech decoder: 16 kHz (core at 12.8 kHz plus a generated 6.4–7 kHz band),
//! 20 ms frames, the nine modes 6.60 to 23.85 kbit/s, comfort noise from SID frames
//! (TS 26.192), DTX (TS 26.193) and the concealment of lost frames (TS 26.191), following
//! the bit-exact fixed-point description of TS 26.173 / TS 26.190.

mod dsp;
mod dtx;
pub(crate) mod tables;

use crate::basic::*;
use crate::{Frame, RxType};
use dsp::*;
use tables as t;

pub(crate) const L_FRAME16K: usize = 320;
const L_FRAME: usize = 256;
pub(crate) const L_SUBFR: usize = 64;
const L_SUBFR16K: usize = 80;
pub(crate) const M: usize = 16;
pub(crate) const M16K: usize = 20;
const PIT_MIN: i16 = 34;
const PIT_FR2: i16 = 128;
const PIT_FR1_9B: i16 = 160;
const PIT_FR1_8B: i16 = 92;
const PIT_MAX: i16 = 231;
const L_INTERPOL: usize = 17;
const EXC_OFF: usize = PIT_MAX as usize + L_INTERPOL;
const PIT_SHARP: i16 = 27853;
const PREEMPH_FAC: i16 = 22282;
const Q_MAX: i16 = 8;
const ISF_GAP: i16 = 128;
const MRDTX: u8 = 9;

/// Speech bits per mode (0..8) and of a SID frame (9).
pub const BITS: [usize; 10] = [132, 177, 253, 285, 317, 365, 397, 461, 477, 35];
const NBBITS_7K: usize = 132;
const NBBITS_9K: usize = 177;
const NBBITS_12K: usize = 253;
const NBBITS_14K: usize = 285;
const NBBITS_16K: usize = 317;
const NBBITS_18K: usize = 365;
const NBBITS_20K: usize = 397;
const NBBITS_24K: usize = 477;

const INTERPOL_FRAC: [i16; 4] = [14746, 26214, 31457, 32767];

pub(crate) fn sort(mode: u8) -> &'static [i16] {
    match mode {
        0 => &t::SORT_660,
        1 => &t::SORT_885,
        2 => &t::SORT_1265,
        3 => &t::SORT_1425,
        4 => &t::SORT_1585,
        5 => &t::SORT_1825,
        6 => &t::SORT_1985,
        7 => &t::SORT_2305,
        8 => &t::SORT_2385,
        _ => &t::SORT_SID,
    }
}

fn dhf(mode: u8) -> &'static [i16] {
    match mode {
        0 => &t::DHF_7K,
        1 => &t::DHF_9K,
        2 => &t::DHF_12K,
        3 => &t::DHF_14K,
        4 => &t::DHF_16K,
        5 => &t::DHF_18K,
        6 => &t::DHF_20K,
        7 => &t::DHF_23K,
        _ => &t::DHF_24K,
    }
}

/// Reads parameters MSB first from the serial bits.
pub(crate) struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Self {
        Bits { b, pos: 0 }
    }
    pub(crate) fn get(&mut self, n: usize) -> i16 {
        let mut v: i32 = 0;
        for _ in 0..n {
            v = (v << 1) | (self.b.get(self.pos).copied().unwrap_or(0) & 1) as i32;
            self.pos += 1;
        }
        v as i16
    }
}

// --- algebraic codebook ---

const NB_POS: i16 = 16;

fn dec_1p_n1(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let mask = (1i32 << n) - 1;
    let mut p = add(extract_l(index & mask), offset);
    if (l_shr(index, n) & 1) == 1 {
        p = add(p, NB_POS);
    }
    pos[0] = p;
}

fn dec_2p_2n1(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let mask = (1i32 << n) - 1;
    let mut p1 = extract_l(l_add(l_shr(index, n) & mask, offset as i32));
    let i = l_shr(index, shl(n, 1)) & 1;
    let mut p2 = add(extract_l(index & mask), offset);
    if p2 < p1 {
        if i == 1 {
            p1 = add(p1, NB_POS);
        } else {
            p2 = add(p2, NB_POS);
        }
    } else if i == 1 {
        p1 = add(p1, NB_POS);
        p2 = add(p2, NB_POS);
    }
    pos[0] = p1;
    pos[1] = p2;
}

fn dec_3p_3n1(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let tmp = sub(shl(n, 1), 1);
    let mask = l_sub(l_shl(1, tmp), 1);
    let idx = index & mask;
    let mut j = offset;
    if (l_shr(index, tmp) & 1) != 0 {
        j = add(j, shl(1, sub(n, 1)));
    }
    dec_2p_2n1(idx, n - 1, j, pos);
    let mask = (sub(shl(1, add(n, 1)), 1)) as i32;
    let idx = l_shr(index, shl(n, 1)) & mask;
    dec_1p_n1(idx, n, offset, &mut pos[2..]);
}

fn dec_4p_4n1(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let tmp = sub(shl(n, 1), 1);
    let mask = l_sub(l_shl(1, tmp), 1);
    let idx = index & mask;
    let mut j = offset;
    if (l_shr(index, tmp) & 1) != 0 {
        j = add(j, shl(1, sub(n, 1)));
    }
    dec_2p_2n1(idx, n - 1, j, pos);
    let tmp = add(shl(n, 1), 1);
    let mask = l_sub(l_shl(1, tmp), 1);
    let idx = l_shr(index, shl(n, 1)) & mask;
    dec_2p_2n1(idx, n, offset, &mut pos[2..]);
}

fn dec_4p_4n(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let n_1 = n - 1;
    let j = add(offset, shl(1, n_1));
    let tmp = sub(shl(n, 2), 2);
    match l_shr(index, tmp) & 3 {
        0 => {
            let tmp = add(shl(n_1, 2), 1);
            if (l_shr(index, tmp) & 1) == 0 {
                dec_4p_4n1(index, n_1, offset, pos);
            } else {
                dec_4p_4n1(index, n_1, j, pos);
            }
        }
        1 => {
            let tmp = add(extract_l(l_shr(l_mult(3, n_1), 1)), 1);
            dec_1p_n1(l_shr(index, tmp), n_1, offset, pos);
            dec_3p_3n1(index, n_1, j, &mut pos[1..]);
        }
        2 => {
            let tmp = add(shl(n_1, 1), 1);
            dec_2p_2n1(l_shr(index, tmp), n_1, offset, pos);
            dec_2p_2n1(index, n_1, j, &mut pos[2..]);
        }
        _ => {
            let tmp = add(n_1, 1);
            dec_3p_3n1(l_shr(index, tmp), n_1, offset, pos);
            dec_1p_n1(index, n_1, j, &mut pos[3..]);
        }
    }
}

fn dec_5p_5n(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let n_1 = n - 1;
    let j = add(offset, shl(1, n_1));
    let idx = l_shr(index, add(shl(n, 1), 1));
    let tmp = sub(extract_l(l_shr(l_mult(5, n), 1)), 1);
    if (l_shr(index, tmp) & 1) == 0 {
        dec_3p_3n1(idx, n_1, offset, pos);
    } else {
        dec_3p_3n1(idx, n_1, j, pos);
    }
    dec_2p_2n1(index, n, offset, &mut pos[3..]);
}

fn dec_6p_6n_2(index: i32, n: i16, offset: i16, pos: &mut [i16]) {
    let n_1 = n - 1;
    let j = add(offset, shl(1, n_1));
    let (mut off_a, mut off_b) = (j, j);
    if (l_shr(index, 6 * n - 5) & 1) == 0 {
        off_a = offset;
    } else {
        off_b = offset;
    }
    match l_shr(index, 6 * n - 4) & 3 {
        0 => {
            dec_5p_5n(l_shr(index, n), n_1, off_a, pos);
            dec_1p_n1(index, n_1, off_a, &mut pos[5..]);
        }
        1 => {
            dec_5p_5n(l_shr(index, n), n_1, off_a, pos);
            dec_1p_n1(index, n_1, off_b, &mut pos[5..]);
        }
        2 => {
            dec_4p_4n(l_shr(index, 2 * n_1 + 1), n_1, off_a, pos);
            dec_2p_2n1(index, n_1, off_b, &mut pos[4..]);
        }
        _ => {
            dec_3p_3n1(l_shr(index, 3 * n_1 + 1), n_1, offset, pos);
            dec_3p_3n1(index, n_1, j, &mut pos[3..]);
        }
    }
}

fn add_pulses(pos: &[i16], track: usize, code: &mut [i16; L_SUBFR]) {
    for &p in pos {
        let i = (((p & (NB_POS - 1)) as usize) << 2) + track;
        code[i] = if p & NB_POS == 0 {
            add(code[i], 512)
        } else {
            sub(code[i], 512)
        };
    }
}

fn dec_acelp_2t64(index: i16, code: &mut [i16; L_SUBFR]) {
    code.fill(0);
    let i0 = (shr(index, 5) & 0x3e) as usize;
    let i1 = add(shl(index & 0x1f, 1), 1) as usize;
    code[i0] = if shr(index, 6) & 32 == 0 { 512 } else { -512 };
    code[i1] = if index & 32 == 0 { 512 } else { -512 };
}

fn dec_acelp_4t64(ind: &[i16; 8], nbbits: usize, code: &mut [i16; L_SUBFR]) {
    code.fill(0);
    let mut pos = [0i16; 6];
    let wide = |hi: i16, lo: i16, s: i16| l_add(l_shl(hi as i32, s), lo as i32);
    for k in 0..4 {
        let (n, idx) = match nbbits {
            20 => (1, ind[k] as i32),
            36 => (2, ind[k] as i32),
            44 => (if k < 2 { 3 } else { 2 }, ind[k] as i32),
            52 => (3, ind[k] as i32),
            64 => (4, wide(ind[k], ind[k + 4], 14)),
            72 => {
                if k < 2 {
                    (5, wide(ind[k], ind[k + 4], 10))
                } else {
                    (4, wide(ind[k], ind[k + 4], 14))
                }
            }
            _ => (6, wide(ind[k], ind[k + 4], 11)),
        };
        match n {
            1 => dec_1p_n1(idx, 4, 0, &mut pos),
            2 => dec_2p_2n1(idx, 4, 0, &mut pos),
            3 => dec_3p_3n1(idx, 4, 0, &mut pos),
            4 => dec_4p_4n(idx, 4, 0, &mut pos),
            5 => dec_5p_5n(idx, 4, 0, &mut pos),
            _ => dec_6p_6n_2(idx, 4, 0, &mut pos),
        }
        add_pulses(&pos[..n], k, code);
    }
}

// --- gains ---

/// Gain decoding and its concealment memory (`dec_gain[23]` of the reference).
#[derive(Clone)]
struct GainMem {
    past_qua_en: [i16; 4],
    past_gain_pit: i16,
    past_gain_code: i16,
    prev_gc: i16,
    pbuf: [i16; 5],
    gbuf: [i16; 5],
    pbuf2: [i16; 5],
    #[allow(dead_code)]
    seed: i16,
}

impl GainMem {
    fn new() -> Self {
        GainMem {
            past_qua_en: [-14336; 4],
            past_gain_pit: 0,
            past_gain_code: 0,
            prev_gc: 0,
            pbuf: [0; 5],
            gbuf: [0; 5],
            pbuf2: [0; 5],
            seed: 21845,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn decode(
        &mut self,
        index: i16,
        nbits: u32,
        code: &[i16],
        bfi: bool,
        prev_bfi: bool,
        state: usize,
        unusable: bool,
        vad_hist: i16,
    ) -> (i16, i32) {
        const PDOWN_UNUSABLE: [i16; 7] = [32767, 31130, 29491, 24576, 7537, 1638, 328];
        const CDOWN_UNUSABLE: [i16; 7] = [32767, 16384, 8192, 8192, 8192, 4915, 3277];
        const PDOWN_USABLE: [i16; 7] = [32767, 32113, 31457, 24576, 7537, 1638, 328];
        const CDOWN_USABLE: [i16; 7] = [32767, 32113, 32113, 32113, 32113, 32113, 22938];
        const PRED: [i16; 4] = [4096, 3277, 2458, 1638];
        let (mut l, exp) = dot_product12(code, code);
        let mut exp = sub(exp, 18 + 6);
        isqrt_n(&mut l, &mut exp);
        let gcode_inov = extract_h(l_shl(l, sub(exp, 3)));
        if bfi {
            let mut tmp = median5(&self.pbuf);
            if tmp > 15565 {
                tmp = 15565;
            }
            self.past_gain_pit = tmp;
            let gain_pit = mult(
                if unusable {
                    PDOWN_UNUSABLE[state]
                } else {
                    PDOWN_USABLE[state]
                },
                tmp,
            );
            let tmp = median5(&self.gbuf);
            self.past_gain_code = if vad_hist > 2 {
                tmp
            } else {
                mult(
                    if unusable {
                        CDOWN_UNUSABLE[state]
                    } else {
                        CDOWN_USABLE[state]
                    },
                    tmp,
                )
            };
            let mut l = l_mult(self.past_qua_en[0], 8192);
            for k in 1..4 {
                l = l_mac(l, self.past_qua_en[k], 8192);
            }
            let mut qua_ener = sub(extract_h(l), 3072);
            if qua_ener < -14336 {
                qua_ener = -14336;
            }
            self.past_qua_en.copy_within(0..3, 1);
            self.past_qua_en[0] = qua_ener;
            self.gbuf.copy_within(1.., 0);
            self.gbuf[4] = self.past_gain_code;
            self.pbuf.copy_within(1.., 0);
            self.pbuf[4] = self.past_gain_pit;
            return (gain_pit, l_mult(self.past_gain_code, gcode_inov));
        }
        let mut l = l_deposit_h(30);
        l = l_shl(l, 8);
        for k in 0..4 {
            l = l_mac(l, PRED[k], self.past_qua_en[k]);
        }
        let gcode0 = extract_h(l);
        let l = l_shr(l_mult(gcode0, 5443), 8);
        let (exp_gcode0, frac) = l_extract(l);
        let gcode0 = extract_l(pow2(14, frac));
        let exp_gcode0 = sub(exp_gcode0, 14);
        let p = if nbits == 6 {
            &t::T_QUA_GAIN6B[index as usize * 2..index as usize * 2 + 2]
        } else {
            &t::T_QUA_GAIN7B[index as usize * 2..index as usize * 2 + 2]
        };
        let gain_pit = p[0];
        let g_code = p[1];
        let mut gain_cod = l_shl(l_mult(g_code, gcode0), add(exp_gcode0, 4));
        if prev_bfi {
            let l = l_mult(self.prev_gc, 5120);
            if gain_cod > l && gain_cod > 6_553_600 {
                gain_cod = l;
            }
        }
        self.past_gain_code = round(l_shl(gain_cod, 3));
        self.past_gain_pit = gain_pit;
        self.prev_gc = self.past_gain_code;
        self.gbuf.copy_within(1.., 0);
        self.gbuf[4] = self.past_gain_code;
        self.pbuf.copy_within(1.., 0);
        self.pbuf[4] = self.past_gain_pit;
        self.pbuf2.copy_within(1.., 0);
        self.pbuf2[4] = self.past_gain_pit;
        let (e, f) = l_extract(gain_cod);
        let gain_cod = l_shl(mpy_32_16(e, f, gcode_inov), 3);
        let (e, f) = log2(l_deposit_l(g_code));
        let e = sub(e, 11);
        let l = mpy_32_16(e, f, 24660);
        let qua_ener = extract_l(l_shr(l, 3));
        self.past_qua_en.copy_within(0..3, 1);
        self.past_qua_en[0] = qua_ener;
        (gain_pit, gain_cod)
    }
}

fn insertion_sort(a: &mut [i16; 5]) {
    for n in 0..5 {
        let x = a[n];
        let mut i = n as isize - 1;
        while i >= 0 && x < a[i as usize] {
            a[i as usize + 1] = a[i as usize];
            i -= 1;
        }
        a[(i + 1) as usize] = x;
    }
}

/// Pitch lag of a lost or damaged subframe from the lag and gain history.
fn lagconc(
    gain_hist: &[i16; 5],
    lag_hist: &[i16; 5],
    t0: &mut i16,
    old_t0: i16,
    seed: &mut i16,
    unusable: bool,
) {
    const ONE_PER_3: i16 = 10923;
    let last_gain = gain_hist[4];
    let sec_last_gain = gain_hist[3];
    let last_lag = lag_hist[0];
    let min_lag = *lag_hist.iter().min().unwrap_or(&0);
    let max_lag = *lag_hist.iter().max().unwrap_or(&0);
    let min_gain = *gain_hist.iter().min().unwrap_or(&0);
    let lag_dif = sub(max_lag, min_lag);
    let random_lag = |seed: &mut i16| {
        let mut h = *lag_hist;
        insertion_sort(&mut h);
        let lag_dif = sub(h[4], h[2]).min(40);
        let d = random(seed);
        let d2 = mult(shr(lag_dif, 1), d);
        add(mult(add(add(h[2], h[3]), h[4]), ONE_PER_3), d2)
    };
    if unusable {
        if min_gain > 8192 && lag_dif < 10 {
            *t0 = old_t0;
        } else if last_gain > 8192 && sec_last_gain > 8192 {
            *t0 = lag_hist[0];
        } else {
            *t0 = random_lag(seed);
        }
        *t0 = (*t0).min(max_lag).max(min_lag);
    } else {
        let mut mean_lag: i16 = 0;
        for &l in lag_hist {
            mean_lag = add(mean_lag, l);
        }
        let mean_lag = mult(mean_lag, 6554);
        let tmp = sub(*t0, max_lag);
        let tmp2 = sub(*t0, last_lag);
        let keep = (lag_dif < 10 && *t0 > sub(min_lag, 5) && tmp < 5)
            || (last_gain > 8192 && sec_last_gain > 8192 && add(tmp2, 10) > 0 && tmp2 < 10)
            || (min_gain < 6554 && last_gain == min_gain && *t0 > min_lag && *t0 < max_lag)
            || (lag_dif < 70 && *t0 > min_lag && *t0 < max_lag)
            || (*t0 > mean_lag && *t0 < max_lag);
        if !keep {
            if (min_gain > 8192 && lag_dif < 10) || (last_gain > 8192 && sec_last_gain > 8192) {
                *t0 = lag_hist[0];
            } else {
                *t0 = random_lag(seed);
            }
            *t0 = (*t0).min(max_lag).max(min_lag);
        }
    }
}

fn phase_dispersion(
    gain_code: i16,
    gain_pit: i16,
    code: &mut [i16; L_SUBFR],
    mode: i16,
    mem: &mut [i16; 8],
) {
    const P06: i16 = 9830;
    const P09: i16 = 14746;
    let mut code2 = [0i16; 2 * L_SUBFR];
    let mut state: i16 = if gain_pit < P06 {
        0
    } else if gain_pit < P09 {
        1
    } else {
        2
    };
    mem.copy_within(2..7, 3);
    mem[2] = gain_pit;
    let prev_state = mem[0];
    let prev_gain_code = mem[1];
    if sub(sub(gain_code, prev_gain_code), shl(prev_gain_code, 1)) > 0 {
        if state < 2 {
            state += 1;
        }
    } else {
        let j = mem[2..8].iter().filter(|&&g| g < P06).count();
        if j > 2 {
            state = 0;
        }
        if sub(state, prev_state) > 1 {
            state -= 1;
        }
    }
    mem[1] = gain_code;
    mem[0] = state;
    let state = add(state, mode);
    let imp = match state {
        0 => Some(&t::PH_IMP_LOW),
        1 => Some(&t::PH_IMP_MID),
        _ => None,
    };
    if let Some(imp) = imp {
        for i in 0..L_SUBFR {
            if code[i] != 0 {
                for j in 0..L_SUBFR {
                    code2[i + j] = add(code2[i + j], mult_r(code[i], imp[j]));
                }
            }
        }
        for i in 0..L_SUBFR {
            code[i] = add(code2[i], code2[i + L_SUBFR]);
        }
    }
}

// --- the decoder ---

#[derive(Clone)]
struct State {
    old_exc: [i16; EXC_OFF],
    ispold: [i16; M],
    isfold: [i16; M],
    isf_buf: [i16; 3 * M],
    past_isfq: [i16; M],
    tilt_code: i16,
    q_old: i16,
    q_subfr: [i16; 4],
    l_gc_thres: i32,
    mem_syn_hi: [i16; M],
    mem_syn_lo: [i16; M],
    mem_deemph: i16,
    mem_sig_out: [i16; 6],
    mem_oversamp: [i16; 24],
    mem_syn_hf: [i16; M16K],
    mem_hf: [i16; 30],
    mem_hf3: [i16; 30],
    seed: i16,
    seed2: i16,
    old_t0: i16,
    lag_hist: [i16; 5],
    gain: GainMem,
    seed3: i16,
    disp_mem: [i16; 8],
    mem_hp400: [i16; 6],
    prev_bfi: bool,
    state: i16,
    first_frame: bool,
    dtx: dtx::DtxDec,
    vad_hist: i16,
}

impl State {
    fn new() -> Self {
        let mut isf_buf = [0i16; 3 * M];
        for c in isf_buf.chunks_mut(M) {
            c.copy_from_slice(&t::ISF_INIT);
        }
        State {
            old_exc: [0; EXC_OFF],
            ispold: t::ISP_INIT,
            isfold: t::ISF_INIT,
            isf_buf,
            past_isfq: [0; M],
            tilt_code: 0,
            q_old: Q_MAX,
            q_subfr: [Q_MAX; 4],
            l_gc_thres: 0,
            mem_syn_hi: [0; M],
            mem_syn_lo: [0; M],
            mem_deemph: 0,
            mem_sig_out: [0; 6],
            mem_oversamp: [0; 24],
            mem_syn_hf: [0; M16K],
            mem_hf: [0; 30],
            mem_hf3: [0; 30],
            seed: 21845,
            seed2: 21845,
            old_t0: 64,
            lag_hist: [64; 5],
            gain: GainMem::new(),
            seed3: 21845,
            disp_mem: [0; 8],
            mem_hp400: [0; 6],
            prev_bfi: false,
            state: 0,
            first_frame: true,
            dtx: dtx::DtxDec::new(),
            vad_hist: 0,
        }
    }

    /// The partial reset after a comfort-noise frame (`Reset_decoder(st, 0)`).
    fn reset_partial(&mut self) {
        self.old_exc = [0; EXC_OFF];
        self.past_isfq = [0; M];
        self.old_t0 = 64;
        self.first_frame = true;
        self.l_gc_thres = 0;
        self.tilt_code = 0;
        self.disp_mem = [0; 8];
        self.q_old = Q_MAX;
        self.q_subfr = [Q_MAX; 4];
    }

    #[allow(clippy::too_many_arguments)]
    fn synthesis(
        &mut self,
        aq: &[i16],
        exc: &mut [i16],
        q_new: i16,
        synth16k: &mut [i16],
        corr_gain: i16,
        hf_isf: &mut [i16; M16K],
        nb_bits: usize,
        dtx_state: dtx::Dtx,
        bfi: bool,
    ) {
        let mut hi = [0i16; M + L_SUBFR];
        let mut lo = [0i16; M + L_SUBFR];
        hi[..M].copy_from_slice(&self.mem_syn_hi);
        lo[..M].copy_from_slice(&self.mem_syn_lo);
        syn_filt_32(aq, exc, q_new, &mut hi, &mut lo);
        self.mem_syn_hi.copy_from_slice(&hi[L_SUBFR..]);
        self.mem_syn_lo.copy_from_slice(&lo[L_SUBFR..]);
        let mut synth = [0i16; L_SUBFR];
        deemph_32(
            &hi[M..],
            &lo[M..],
            &mut synth,
            PREEMPH_FAC,
            &mut self.mem_deemph,
        );
        hp_iir(
            &mut synth,
            &mut self.mem_sig_out,
            [4053, -8106, 4053],
            [8192, 16211, -8021],
            true,
        );
        oversamp_16k(&synth, synth16k, &mut self.mem_oversamp);

        let mut hf = [0i16; L_SUBFR16K];
        for v in hf.iter_mut() {
            *v = shr(random(&mut self.seed2), 3);
        }
        scale_sig(exc, -3);
        let q_new = sub(q_new, 3);
        let (e, exp_ener) = dot_product12(exc, exc);
        let ener = extract_h(e);
        let exp_ener = sub(exp_ener, add(q_new, q_new));
        let (e, mut exp) = dot_product12(&hf, &hf);
        let mut tmp = extract_h(e);
        if tmp > ener {
            tmp = shr(tmp, 1);
            exp = add(exp, 1);
        }
        let mut l = l_deposit_h(div_s(tmp, ener));
        let mut exp = sub(exp, exp_ener);
        isqrt_n(&mut l, &mut exp);
        let l = l_shl(l, add(exp, 1));
        let tmp = extract_h(l);
        for v in hf.iter_mut() {
            *v = mult(*v, tmp);
        }
        hp_iir(
            &mut synth,
            &mut self.mem_hp400,
            [915, -1830, 915],
            [16384, 29280, -14160],
            false,
        );
        let mut l = 1i32;
        for &v in &synth {
            l = l_mac(l, v, v);
        }
        let exp = norm_l(l);
        let ener = extract_h(l_shl(l, exp));
        let mut l = 1i32;
        for i in 1..L_SUBFR {
            l = l_mac(l, synth[i], synth[i - 1]);
        }
        let tmp = extract_h(l_shl(l, exp));
        let fac = if tmp > 0 { div_s(tmp, ener) } else { 0 };
        let gain1 = sub(32767, fac);
        let gain2 = shl(mult(sub(32767, fac), 20480), 1);
        let (w1, w2) = if self.vad_hist > 0 {
            (0, 32767)
        } else {
            (32767, 0)
        };
        let mut tmp = add(mult(w1, gain1), mult(w2, gain2));
        if tmp != 0 {
            tmp = add(tmp, 1);
        }
        if tmp < 3277 {
            tmp = 3277;
        }
        if nb_bits >= NBBITS_24K && !bfi {
            let g = t::HP_GAIN[(corr_gain & 15) as usize];
            for v in hf.iter_mut() {
                *v = shl(mult(*v, g), 1);
            }
        } else {
            for v in hf.iter_mut() {
                *v = mult(*v, tmp);
            }
        }
        if nb_bits <= NBBITS_7K && dtx_state == dtx::Dtx::Speech {
            isf_extrapolation(hf_isf);
            let mut hf_a = [0i16; M16K + 1];
            isp_az(&hf_isf[..], &mut hf_a, M16K, false);
            let mut ap = [0i16; M16K + 1];
            weight_a(&hf_a, &mut ap, 29491, M16K);
            syn_filt(&ap, M16K, &mut hf, &mut self.mem_syn_hf, true);
        } else {
            let mut ap = [0i16; M + 1];
            weight_a(aq, &mut ap, 19661, M);
            syn_filt(&ap, M, &mut hf, &mut self.mem_syn_hf[M16K - M..], true);
        }
        fir31(&mut hf, &mut self.mem_hf, &t::FIR_6K_7K, 2);
        if nb_bits >= NBBITS_24K {
            fir31(&mut hf, &mut self.mem_hf3, &t::FIR_7K, 0);
        }
        for i in 0..L_SUBFR16K {
            synth16k[i] = add(synth16k[i], hf[i]);
        }
    }

    fn decode(
        &mut self,
        mode: u8,
        prms: &[u8],
        synth16k: &mut [i16; L_FRAME16K],
        frame_type: RxType,
    ) {
        let nb_bits = BITS[(mode as usize).min(9)];
        let mut bits = Bits::new(prms);
        let new_state = self.dtx.rx_handler(frame_type);
        let mut exc2 = [0i16; L_FRAME];
        let mut isf = [0i16; M];
        if new_state != dtx::Dtx::Speech {
            self.dtx.decode(&mut exc2, new_state, &mut isf, &mut bits);
        }
        let (bfi, unusable) = match frame_type {
            RxType::SpeechBad | RxType::SpeechDegraded => (true, false),
            RxType::NoData | RxType::SpeechLost => (true, true),
            _ => (false, false),
        };
        if bfi {
            self.state = add(self.state, 1).min(6);
        } else {
            self.state = shr(self.state, 1);
        }
        match self.dtx.global_state {
            dtx::Dtx::Dtx => {
                self.state = 5;
                self.prev_bfi = false;
            }
            dtx::Dtx::DtxMute => {
                self.state = 5;
                self.prev_bfi = true;
            }
            dtx::Dtx::Speech => {}
        }
        if new_state == dtx::Dtx::Speech {
            let vad_flag = bits.get(1);
            if !bfi {
                if vad_flag == 0 {
                    self.vad_hist = add(self.vad_hist, 1);
                    self.dtx.vad_hist = add(self.dtx.vad_hist, 1);
                } else {
                    self.vad_hist = 0;
                    self.dtx.vad_hist = 0;
                }
            } else if self.dtx.vad_hist > 0 {
                self.dtx.vad_hist = add(self.dtx.vad_hist, 1);
            }
        }
        if new_state != dtx::Dtx::Speech {
            let mut ispnew = [0i16; M];
            isf_isp(&isf, &mut ispnew, M);
            let mut aq = [0i16; M + 1];
            isp_az(&ispnew, &mut aq, M, true);
            let isf_tmp = self.isfold;
            for (j, i_subfr) in (0..L_FRAME).step_by(L_SUBFR).enumerate() {
                let mut hf_isf = [0i16; M16K];
                for i in 0..M {
                    let l = l_mult(isf_tmp[i], sub(32767, INTERPOL_FRAC[j]));
                    hf_isf[i] = round(l_mac(l, isf[i], INTERPOL_FRAC[j]));
                }
                let o = i_subfr * 5 / 4;
                self.synthesis(
                    &aq,
                    &mut exc2[i_subfr..i_subfr + L_SUBFR],
                    0,
                    &mut synth16k[o..o + L_SUBFR16K],
                    1,
                    &mut hf_isf,
                    nb_bits,
                    new_state,
                    bfi,
                );
            }
            self.reset_partial();
            self.isfold = isf;
            self.prev_bfi = bfi;
            self.dtx.global_state = new_state;
            return;
        }

        let mut old_exc = [0i16; L_FRAME + 1 + EXC_OFF];
        old_exc[..EXC_OFF].copy_from_slice(&self.old_exc);
        let mut ind = [0i16; 8];
        if nb_bits <= NBBITS_7K {
            for (k, n) in [8, 8, 7, 7, 6].into_iter().enumerate() {
                ind[k] = bits.get(n);
            }
            self.dpisf_2s(&ind, &mut isf, bfi, true);
        } else {
            for (k, n) in [8, 8, 6, 7, 7, 5, 5].into_iter().enumerate() {
                ind[k] = bits.get(n);
            }
            self.dpisf_2s(&ind, &mut isf, bfi, false);
        }
        let mut ispnew = [0i16; M];
        isf_isp(&isf, &mut ispnew, M);
        if self.first_frame {
            self.first_frame = false;
            self.ispold = ispnew;
        }
        let mut aq = [0i16; 4 * (M + 1)];
        int_isp(&self.ispold, &ispnew, &INTERPOL_FRAC, &mut aq);
        self.ispold = ispnew;
        let mut l = 0i32;
        for i in 0..M - 1 {
            let d = sub(isf[i], self.isfold[i]);
            l = l_mac(l, d, d);
        }
        let tmp = mult(extract_h(l_shl(l, 8)), 26214);
        let stab_fac = shl(sub(20480, tmp), 1).max(0);
        let isf_tmp = self.isfold;
        self.isfold = isf;

        let mut t0: i16;
        let mut t0_frac: i16;
        let mut t0_min: i16 = 0;
        let mut q_new: i16 = 0;
        let mut code = [0i16; L_SUBFR];
        for (sf, i_subfr) in (0..L_FRAME).step_by(L_SUBFR).enumerate() {
            let e = EXC_OFF + i_subfr;
            let mut pit_flag = i_subfr;
            if i_subfr == 2 * L_SUBFR && nb_bits > NBBITS_7K {
                pit_flag = 0;
            }
            if pit_flag == 0 {
                if nb_bits <= NBBITS_9K {
                    let index = bits.get(8);
                    if index < (PIT_FR1_8B - PIT_MIN) * 2 {
                        t0 = add(PIT_MIN, shr(index, 1));
                        t0_frac = shl(sub(index, shl(sub(t0, PIT_MIN), 1)), 1);
                    } else {
                        t0 = add(index, PIT_FR1_8B - (PIT_FR1_8B - PIT_MIN) * 2);
                        t0_frac = 0;
                    }
                } else {
                    let mut index = bits.get(9);
                    if index < (PIT_FR2 - PIT_MIN) * 4 {
                        t0 = add(PIT_MIN, shr(index, 2));
                        t0_frac = sub(index, shl(sub(t0, PIT_MIN), 2));
                    } else if index < (PIT_FR2 - PIT_MIN) * 4 + (PIT_FR1_9B - PIT_FR2) * 2 {
                        index = sub(index, (PIT_FR2 - PIT_MIN) * 4);
                        t0 = add(PIT_FR2, shr(index, 1));
                        t0_frac = shl(sub(index, shl(sub(t0, PIT_FR2), 1)), 1);
                    } else {
                        t0 = add(
                            index,
                            PIT_FR1_9B - (PIT_FR2 - PIT_MIN) * 4 - (PIT_FR1_9B - PIT_FR2) * 2,
                        );
                        t0_frac = 0;
                    }
                }
                t0_min = sub(t0, 8);
                if t0_min < PIT_MIN {
                    t0_min = PIT_MIN;
                }
                let t0_max = add(t0_min, 15);
                if t0_max > PIT_MAX {
                    t0_min = sub(PIT_MAX, 15);
                }
            } else if nb_bits <= NBBITS_9K {
                let index = bits.get(5);
                t0 = add(t0_min, shr(index, 1));
                t0_frac = shl(sub(index, shl(sub(t0, t0_min), 1)), 1);
            } else {
                let index = bits.get(6);
                t0 = add(t0_min, shr(index, 2));
                t0_frac = sub(index, shl(sub(t0, t0_min), 2));
            }
            if bfi {
                let gh = self.gain.pbuf2;
                lagconc(
                    &gh,
                    &self.lag_hist,
                    &mut t0,
                    self.old_t0,
                    &mut self.seed3,
                    unusable,
                );
                t0_frac = 0;
            }
            pred_lt4(&mut old_exc, e, t0, t0_frac, L_SUBFR + 1);
            let select = if unusable {
                1
            } else if nb_bits <= NBBITS_9K {
                0
            } else {
                bits.get(1)
            };
            if select == 0 {
                let mut tmp = [0i16; L_SUBFR];
                for i in 0..L_SUBFR {
                    let l = l_mult(5898, old_exc[e + i - 1]);
                    let l = l_mac(l, 20972, old_exc[e + i]);
                    let l = l_mac(l, 5898, old_exc[e + i + 1]);
                    tmp[i] = round(l);
                }
                old_exc[e..e + L_SUBFR].copy_from_slice(&tmp);
            }
            if unusable {
                for v in code.iter_mut() {
                    *v = shr(random(&mut self.seed), 3);
                }
            } else if nb_bits <= NBBITS_7K {
                let i0 = bits.get(12);
                dec_acelp_2t64(i0, &mut code);
            } else {
                let spec: &[usize] = match nb_bits {
                    NBBITS_9K => &[5, 5, 5, 5],
                    NBBITS_12K => &[9, 9, 9, 9],
                    NBBITS_14K => &[13, 13, 9, 9],
                    NBBITS_16K => &[13, 13, 13, 13],
                    NBBITS_18K => &[2, 2, 2, 2, 14, 14, 14, 14],
                    NBBITS_20K => &[10, 10, 2, 2, 10, 10, 14, 14],
                    _ => &[11, 11, 11, 11, 11, 11, 11, 11],
                };
                let mut ind = [0i16; 8];
                for (k, &n) in spec.iter().enumerate() {
                    ind[k] = bits.get(n);
                }
                let nbbits = match nb_bits {
                    NBBITS_9K => 20,
                    NBBITS_12K => 36,
                    NBBITS_14K => 44,
                    NBBITS_16K => 52,
                    NBBITS_18K => 64,
                    NBBITS_20K => 72,
                    _ => 88,
                };
                dec_acelp_4t64(&ind, nbbits, &mut code);
            }
            let mut mem0 = 0i16;
            preemph(&mut code, self.tilt_code, &mut mem0);
            let mut tmp = t0;
            if t0_frac > 2 {
                tmp = add(tmp, 1);
            }
            pit_shrp(&mut code, tmp, PIT_SHARP);
            let (index, nbits) = if nb_bits <= NBBITS_9K {
                (bits.get(6), 6)
            } else {
                (bits.get(7), 7)
            };
            let (gain_pit, mut l_gain_code) = self.gain.decode(
                index,
                nbits,
                &code,
                bfi,
                self.prev_bfi,
                self.state as usize,
                unusable,
                self.vad_hist,
            );
            let mut tmp = self.q_subfr[0];
            for &q in &self.q_subfr[1..] {
                if q < tmp {
                    tmp = q;
                }
            }
            if tmp > Q_MAX {
                tmp = Q_MAX;
            }
            q_new = 0;
            let mut l = l_gain_code;
            while l < 0x0800_0000 && q_new < tmp {
                l = l_shl(l, 1);
                q_new = add(q_new, 1);
            }
            let gain_code = round(l);
            scale_sig(
                &mut old_exc[i_subfr..i_subfr + EXC_OFF + L_SUBFR],
                sub(q_new, self.q_old),
            );
            self.q_old = q_new;
            if !bfi {
                self.lag_hist.copy_within(0..4, 1);
                self.lag_hist[0] = t0;
                self.old_t0 = t0;
            }
            let mut exc2s = [0i16; L_SUBFR];
            exc2s.copy_from_slice(&old_exc[e..e + L_SUBFR]);
            scale_sig(&mut exc2s, -3);
            let mut excp = [0i16; L_SUBFR];
            let pit_sharp = if nb_bits <= NBBITS_9K {
                let ps = shl(gain_pit, 1);
                if ps > 16384 {
                    for i in 0..L_SUBFR {
                        let tmp = mult(exc2s[i], ps);
                        excp[i] = round(l_shr(l_mult(tmp, gain_pit), 1));
                    }
                }
                ps
            } else {
                0
            };
            let voice_fac = voice_factor(&exc2s, -3, gain_pit, &code, gain_code);
            self.tilt_code = add(shr(voice_fac, 2), 8192);
            exc2s.copy_from_slice(&old_exc[e..e + L_SUBFR]);
            for i in 0..L_SUBFR {
                let l = l_shl(l_mult(code[i], gain_code), 5);
                let l = l_mac(l, old_exc[e + i], gain_pit);
                old_exc[e + i] = round(l_shl(l, 1));
            }
            let mut max: i16 = 1;
            for &v in &old_exc[e..e + L_SUBFR] {
                let a = abs_s(v);
                if a > max {
                    max = a;
                }
            }
            let tmp = sub(add(norm_s(max), q_new), 1);
            self.q_subfr.copy_within(0..3, 1);
            self.q_subfr[0] = tmp;

            let (gc_hi, gc_lo) = l_extract(l_gain_code);
            let j = if nb_bits <= NBBITS_7K {
                0
            } else if nb_bits <= NBBITS_9K {
                1
            } else {
                2
            };
            phase_dispersion(gc_hi, gain_pit, &mut code, j, &mut self.disp_mem);
            let tmp = sub(16384, shr(voice_fac, 1));
            let fac = mult(stab_fac, tmp);
            let mut l = l_gain_code;
            if l < self.l_gc_thres {
                l = l_add(l, mpy_32_16(gc_hi, gc_lo, 6226));
                if l > self.l_gc_thres {
                    l = self.l_gc_thres;
                }
            } else {
                l = mpy_32_16(gc_hi, gc_lo, 27536);
                if l < self.l_gc_thres {
                    l = self.l_gc_thres;
                }
            }
            self.l_gc_thres = l;
            l_gain_code = mpy_32_16(gc_hi, gc_lo, sub(32767, fac));
            let (h2, l2) = l_extract(l);
            l_gain_code = l_add(l_gain_code, mpy_32_16(h2, l2, fac));

            let tmp = add(shr(voice_fac, 3), 4096);
            let mut code2 = [0i16; L_SUBFR];
            code2[0] = round(l_msu(l_deposit_h(code[0]), code[1], tmp));
            for i in 1..L_SUBFR - 1 {
                let l = l_msu(l_deposit_h(code[i]), code[i + 1], tmp);
                code2[i] = round(l_msu(l, code[i - 1], tmp));
            }
            code2[L_SUBFR - 1] = round(l_msu(
                l_deposit_h(code[L_SUBFR - 1]),
                code[L_SUBFR - 2],
                tmp,
            ));
            let gain_code = round(l_shl(l_gain_code, q_new));
            for i in 0..L_SUBFR {
                let l = l_shl(l_mult(code2[i], gain_code), 5);
                let l = l_mac(l, exc2s[i], gain_pit);
                exc2s[i] = round(l_shl(l, 1));
            }
            if nb_bits <= NBBITS_9K && pit_sharp > 16384 {
                for i in 0..L_SUBFR {
                    excp[i] = add(excp[i], exc2s[i]);
                }
                agc2(&exc2s, &mut excp);
                exc2s = excp;
            }
            let mut hf_isf = [0i16; M16K];
            if nb_bits <= NBBITS_7K {
                for i in 0..M {
                    let l = l_mult(isf_tmp[i], sub(32767, INTERPOL_FRAC[sf]));
                    hf_isf[i] = round(l_mac(l, isf[i], INTERPOL_FRAC[sf]));
                }
            } else {
                self.mem_syn_hf[..M16K - M].fill(0);
            }
            let corr_gain = if nb_bits >= NBBITS_24K {
                bits.get(4)
            } else {
                0
            };
            let o = i_subfr * 5 / 4;
            let aq_s: [i16; M + 1] = aq[sf * (M + 1)..(sf + 1) * (M + 1)]
                .try_into()
                .unwrap_or([0; M + 1]);
            self.synthesis(
                &aq_s,
                &mut exc2s,
                q_new,
                &mut synth16k[o..o + L_SUBFR16K],
                corr_gain,
                &mut hf_isf,
                nb_bits,
                new_state,
                bfi,
            );
        }
        self.old_exc
            .copy_from_slice(&old_exc[L_FRAME..L_FRAME + EXC_OFF]);
        let mut exc = [0i16; L_FRAME];
        exc.copy_from_slice(&old_exc[EXC_OFF..EXC_OFF + L_FRAME]);
        scale_sig(&mut exc, sub(0, q_new));
        self.dtx.activity_update(&isf, &exc);
        self.dtx.global_state = new_state;
        self.prev_bfi = bfi;
    }

    fn dpisf_2s(&mut self, ind: &[i16; 8], isf_q: &mut [i16; M], bfi: bool, b36: bool) {
        const MU: i16 = 10923;
        const ALPHA: i16 = 29491;
        const ONE_ALPHA: i16 = 3277;
        let mean = &t::MEAN_ISF;
        if !bfi {
            let i0 = ind[0] as usize * 9;
            isf_q[..9].copy_from_slice(&t::DICO1_ISF[i0..i0 + 9]);
            let i1 = ind[1] as usize * 7;
            isf_q[9..16].copy_from_slice(&t::DICO2_ISF[i1..i1 + 7]);
            let parts: &[(&[i16], usize, usize)] = if b36 {
                &[
                    (&t::DICO21_ISF_36B, 0, 5),
                    (&t::DICO22_ISF_36B, 5, 4),
                    (&t::DICO23_ISF_36B, 9, 7),
                ]
            } else {
                &[
                    (&t::DICO21_ISF, 0, 3),
                    (&t::DICO22_ISF, 3, 3),
                    (&t::DICO23_ISF, 6, 3),
                    (&t::DICO24_ISF, 9, 3),
                    (&t::DICO25_ISF, 12, 4),
                ]
            };
            for (k, &(book, at, dim)) in parts.iter().enumerate() {
                let idx = ind[k + 2] as usize * dim;
                for i in 0..dim {
                    isf_q[at + i] = add(isf_q[at + i], book[idx + i]);
                }
            }
            for i in 0..M {
                let tmp = isf_q[i];
                isf_q[i] = add(tmp, mean[i]);
                isf_q[i] = add(isf_q[i], mult(MU, self.past_isfq[i]));
                self.past_isfq[i] = tmp;
            }
            self.isf_buf.copy_within(0..2 * M, M);
            self.isf_buf[..M].copy_from_slice(isf_q);
        } else {
            let mut ref_isf = [0i16; M];
            for i in 0..M {
                let mut l = l_mult(mean[i], 8192);
                for j in 0..3 {
                    l = l_mac(l, self.isf_buf[j * M + i], 8192);
                }
                ref_isf[i] = round(l);
            }
            for i in 0..M {
                isf_q[i] = add(mult(ALPHA, self.isfold[i]), mult(ONE_ALPHA, ref_isf[i]));
            }
            for i in 0..M {
                let tmp = add(ref_isf[i], mult(self.past_isfq[i], MU));
                self.past_isfq[i] = shr(sub(isf_q[i], tmp), 1);
            }
        }
        reorder_isf(isf_q, ISF_GAP, M);
    }
}

/// Parameters of the decoder homing frame test, packed in 15-bit words as the reference.
fn homing_test(bits: &[u8], mode: u8, nparms: usize) -> bool {
    if mode >= MRDTX {
        return false;
    }
    let mut b = Bits::new(bits);
    let mut param = [0i16; 32];
    let mut i = 0usize;
    let shift;
    if mode != 8 {
        let mut j = 0usize;
        while nparms > j + 15 {
            param[i] = b.get(15);
            j += 15;
            i += 1;
        }
        let rest = nparms - j;
        param[i] = b.get(rest);
        shift = 15 - rest as i16;
        param[i] = shl(param[i], shift);
    } else {
        for p in param.iter_mut().take(10) {
            *p = b.get(15);
        }
        param[10] = b.get(15) & 0x61ff;
        for p in param.iter_mut().take(17).skip(11) {
            *p = b.get(15);
        }
        param[17] = b.get(15) & (0xe0ffu16 as i16);
        for p in param.iter_mut().take(24).skip(18) {
            *p = b.get(15);
        }
        param[24] = b.get(15) & 0x7f0f;
        for p in param.iter_mut().take(31).skip(25) {
            *p = b.get(15);
        }
        param[31] = shl(b.get(8), 7);
        i = 31;
        shift = 0;
    }
    let d = dhf(mode);
    let tmpn = i;
    let mut k = 0;
    let mut j: i16 = 0;
    while k < tmpn {
        j = param[k] ^ d[k];
        if j != 0 {
            break;
        }
        k += 1;
    }
    let mask = shl(shr(0x7fff, shift), shift);
    j |= param[k] ^ (d[k] & mask);
    j == 0
}

/// A complete AMR-WB decoder with homing.
#[derive(Clone)]
pub struct Decoder {
    st: Box<State>,
    mode_old: u8,
    reset_flag_old: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder {
            st: Box::new(State::new()),
            mode_old: 0,
            reset_flag_old: true,
        }
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Decodes one 20 ms frame to 320 samples (14-bit resolution in 16 bits). A frame
    /// whose `mode` is 0xFF (SID frames from files) takes the previous speech mode.
    pub fn decode(&mut self, frame: &Frame, out: &mut [i16; L_FRAME16K]) {
        let mut mode = frame.mode;
        let mut reset_flag = false;
        if matches!(frame.rx, RxType::NoData | RxType::SpeechLost) || mode == 0xff {
            mode = self.mode_old;
        }
        if !matches!(frame.rx, RxType::NoData | RxType::SpeechLost) {
            self.mode_old = mode;
            if self.reset_flag_old {
                reset_flag = homing_test(
                    &frame.bits,
                    mode,
                    t::PRMNOFSF[(mode as usize).min(8)] as usize,
                );
            }
        }
        if reset_flag && self.reset_flag_old {
            out.fill(0x0008);
        } else {
            self.st.decode(mode.min(9), &frame.bits, out, frame.rx);
        }
        for v in out.iter_mut() {
            *v &= !3;
        }
        if !self.reset_flag_old {
            reset_flag = homing_test(&frame.bits, mode, BITS[(mode as usize).min(9)]);
        }
        if reset_flag {
            *self.st = State::new();
        }
        self.reset_flag_old = reset_flag;
    }
}
