//! AMR-WB comfort noise and DTX on the receive side (TS 26.192, TS 26.193).

use super::dsp::{dot_product12, isqrt_n, log2, pow2, random, reorder_isf};
use super::tables as t;
use super::{Bits, ISF_GAP, L_FRAME, M};
use crate::RxType;
use crate::basic::*;

const DTX_HIST_SIZE: usize = 8;
const DTX_ELAPSED_FRAMES_THRESH: i16 = 24 + 7 - 1;
const DTX_HANG_CONST: i16 = 7;
const DTX_MAX_EMPTY_THRESH: i16 = 50;

/// The receiver's DTX state, named as TS 26.193 names them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Dtx {
    Speech,
    Dtx,
    DtxMute,
}

#[derive(Clone)]
pub(crate) struct DtxDec {
    since_last_sid: i16,
    true_sid_period_inv: i16,
    log_en: i16,
    old_log_en: i16,
    isf: [i16; M],
    isf_old: [i16; M],
    cng_seed: i16,
    isf_hist: [i16; M * DTX_HIST_SIZE],
    log_en_hist: [i16; DTX_HIST_SIZE],
    hist_ptr: usize,
    dtx_hangover_count: i16,
    dec_ana_elapsed_count: i16,
    sid_frame: bool,
    valid_data: bool,
    dtx_hangover_added: bool,
    pub(crate) global_state: Dtx,
    data_updated: bool,
    dither_seed: i16,
    cn_dith: bool,
    pub(crate) vad_hist: i16,
}

fn disf_ns(ind: &[i16; 5], isf_q: &mut [i16; M]) {
    let parts: [(&[i16], usize, usize); 5] = [
        (&t::DICO1_ISF_NOISE, 0, 2),
        (&t::DICO2_ISF_NOISE, 2, 3),
        (&t::DICO3_ISF_NOISE, 5, 3),
        (&t::DICO4_ISF_NOISE, 8, 4),
        (&t::DICO5_ISF_NOISE, 12, 4),
    ];
    for (k, (book, at, dim)) in parts.into_iter().enumerate() {
        let idx = ind[k] as usize * dim;
        isf_q[at..at + dim].copy_from_slice(&book[idx..idx + dim]);
    }
    for i in 0..M {
        isf_q[i] = add(isf_q[i], t::MEAN_ISF_NOISE[i]);
    }
    reorder_isf(isf_q, ISF_GAP, M);
}

fn cn_dithering(isf: &mut [i16; M], l_log_en_int: &mut i32, seed: &mut i16) {
    const ISF_DITH_GAP: i16 = 448;
    const GAIN_FACTOR: i16 = 75;
    let rnd = |seed: &mut i16| {
        let a = shr(random(seed), 1);
        let b = shr(random(seed), 1);
        add(a, b)
    };
    let r = rnd(seed);
    *l_log_en_int = l_add(*l_log_en_int, l_mult(r, GAIN_FACTOR));
    if *l_log_en_int < 0 {
        *l_log_en_int = 0;
    }
    let mut dither_fac: i16 = 256;
    let r = rnd(seed);
    let temp = add(isf[0], mult_r(r, dither_fac));
    isf[0] = if temp < ISF_GAP { ISF_GAP } else { temp };
    for i in 1..M - 1 {
        dither_fac = add(dither_fac, 2);
        let r = rnd(seed);
        let temp = add(isf[i], mult_r(r, dither_fac));
        let temp1 = sub(temp, isf[i - 1]);
        isf[i] = if temp1 < ISF_DITH_GAP {
            add(isf[i - 1], ISF_DITH_GAP)
        } else {
            temp
        };
    }
    if isf[M - 2] > 16384 {
        isf[M - 2] = 16384;
    }
}

impl DtxDec {
    pub(crate) fn new() -> Self {
        let mut isf_hist = [0i16; M * DTX_HIST_SIZE];
        for c in isf_hist.chunks_mut(M) {
            c.copy_from_slice(&t::ISF_INIT);
        }
        DtxDec {
            since_last_sid: 0,
            true_sid_period_inv: 1 << 13,
            log_en: 3500,
            old_log_en: 3500,
            isf: t::ISF_INIT,
            isf_old: t::ISF_INIT,
            cng_seed: 21845,
            isf_hist,
            log_en_hist: [3500 >> 3; DTX_HIST_SIZE],
            hist_ptr: 0,
            dtx_hangover_count: DTX_HANG_CONST,
            dec_ana_elapsed_count: 32767,
            sid_frame: false,
            valid_data: false,
            dtx_hangover_added: false,
            global_state: Dtx::Speech,
            data_updated: false,
            dither_seed: 21845,
            cn_dith: false,
            vad_hist: 0,
        }
    }

    pub(crate) fn decode(
        &mut self,
        exc2: &mut [i16; L_FRAME],
        new_state: Dtx,
        isf: &mut [i16; M],
        bits: &mut Bits,
    ) {
        if self.dtx_hangover_added && self.sid_frame {
            let mut ptr = self.hist_ptr + 1;
            if ptr == DTX_HIST_SIZE {
                ptr = 0;
            }
            self.isf_hist
                .copy_within(self.hist_ptr * M..self.hist_ptr * M + M, ptr * M);
            self.log_en_hist[ptr] = self.log_en_hist[self.hist_ptr];
            self.log_en = 0;
            let mut l_isf = [0i32; M];
            for i in 0..DTX_HIST_SIZE {
                self.log_en = add(self.log_en, self.log_en_hist[i]);
                for j in 0..M {
                    l_isf[j] = l_add(l_isf[j], l_deposit_l(self.isf_hist[i * M + j]));
                }
            }
            self.log_en = shr(self.log_en, 1);
            self.log_en = add(self.log_en, 1024);
            if self.log_en < 0 {
                self.log_en = 0;
            }
            for j in 0..M {
                self.isf[j] = extract_l(l_shr(l_isf[j], 3));
            }
        }
        if self.sid_frame {
            self.isf_old = self.isf;
            self.old_log_en = self.log_en;
            if self.valid_data {
                let tmp_int_length = self.since_last_sid.min(32);
                self.true_sid_period_inv = if tmp_int_length >= 2 {
                    div_s(1 << 10, shl(tmp_int_length, 10))
                } else {
                    1 << 14
                };
                let mut ind = [0i16; 5];
                for (k, n) in [6, 6, 6, 5, 5].into_iter().enumerate() {
                    ind[k] = bits.get(n);
                }
                disf_ns(&ind, &mut self.isf);
                let log_en_index = bits.get(6);
                self.cn_dith = bits.get(1) != 0;
                self.log_en = shl(log_en_index, 15 - 6);
                self.log_en = mult(self.log_en, 12483);
                if !self.data_updated || self.global_state == Dtx::Speech {
                    self.isf_old = self.isf;
                    self.old_log_en = self.log_en;
                }
            }
        }
        if self.sid_frame && self.valid_data {
            self.since_last_sid = 0;
        }
        let mut int_fac = shl(self.since_last_sid, 10);
        int_fac = mult(int_fac, self.true_sid_period_inv);
        if int_fac > 1024 {
            int_fac = 1024;
        }
        int_fac = shl(int_fac, 4);
        let mut l_log_en_int = l_mult(int_fac, self.log_en);
        for i in 0..M {
            isf[i] = mult(int_fac, self.isf[i]);
        }
        let int_fac = sub(16384, int_fac);
        l_log_en_int = l_mac(l_log_en_int, int_fac, self.old_log_en);
        for i in 0..M {
            isf[i] = add(isf[i], mult(int_fac, self.isf_old[i]));
            isf[i] = shl(isf[i], 1);
        }
        if self.cn_dith {
            cn_dithering(isf, &mut l_log_en_int, &mut self.dither_seed);
        }
        let l = l_shr(l_log_en_int, 9);
        let e = extract_h(l);
        let m = extract_l(l_shr(l_sub(l, l_deposit_h(e)), 1));
        let e = add(e, 16 - 1);
        let level32 = pow2(e, m);
        let exp0 = norm_l(level32);
        let level32 = l_shl(level32, exp0);
        let exp0 = sub(15, exp0);
        let level = extract_h(level32);
        for v in exc2.iter_mut() {
            *v = shr(random(&mut self.cng_seed), 4);
        }
        let (mut ener32, mut exp) = dot_product12(&exc2[..], &exc2[..]);
        isqrt_n(&mut ener32, &mut exp);
        let gain = mult(level, extract_h(ener32));
        let exp = add(add(exp0, exp), 4);
        for v in exc2.iter_mut() {
            *v = shl(mult(*v, gain), exp);
        }
        if new_state == Dtx::DtxMute {
            let mut tmp_int_length = self.since_last_sid.min(32);
            if tmp_int_length <= 0 {
                tmp_int_length = 8;
            }
            self.true_sid_period_inv = div_s(1 << 10, shl(tmp_int_length, 10));
            self.since_last_sid = 0;
            self.old_log_en = self.log_en;
            self.log_en = sub(self.log_en, 64);
        }
        if self.sid_frame && (self.valid_data || self.dtx_hangover_added) {
            self.since_last_sid = 0;
            self.data_updated = true;
        }
    }

    pub(crate) fn activity_update(&mut self, isf: &[i16; M], exc: &[i16]) {
        self.hist_ptr += 1;
        if self.hist_ptr == DTX_HIST_SIZE {
            self.hist_ptr = 0;
        }
        self.isf_hist[self.hist_ptr * M..self.hist_ptr * M + M].copy_from_slice(isf);
        let mut l = 0i32;
        for &v in &exc[..L_FRAME] {
            l = l_mac(l, v, v);
        }
        let l = l_shr(l, 1);
        let (e, m) = log2(l);
        let mut log_en = shl(e, 7);
        log_en = add(log_en, shr(m, 15 - 7));
        log_en = sub(log_en, 1024);
        self.log_en_hist[self.hist_ptr] = log_en;
    }

    pub(crate) fn rx_handler(&mut self, ft: RxType) -> Dtx {
        use RxType::*;
        let in_dtx = matches!(self.global_state, Dtx::Dtx | Dtx::DtxMute);
        let mut new_state;
        if matches!(ft, SidFirst | SidUpdate | SidBad)
            || (in_dtx && matches!(ft, NoData | SpeechBad | SpeechLost))
        {
            new_state = Dtx::Dtx;
            if self.global_state == Dtx::DtxMute
                && matches!(ft, SidBad | SidFirst | SpeechLost | NoData)
            {
                new_state = Dtx::DtxMute;
            }
            self.since_last_sid = add(self.since_last_sid, 1);
            if ft != SidUpdate && self.since_last_sid > DTX_MAX_EMPTY_THRESH {
                new_state = Dtx::DtxMute;
            }
        } else {
            new_state = Dtx::Speech;
            self.since_last_sid = 0;
        }
        if !self.data_updated && ft == SidUpdate {
            self.dec_ana_elapsed_count = 0;
        }
        self.dec_ana_elapsed_count = add(self.dec_ana_elapsed_count, 1);
        self.dtx_hangover_added = false;
        let enc_dtx = matches!(ft, SidFirst | SidUpdate | SidBad)
            || (ft == NoData
                && (self.global_state != Dtx::Speech || self.vad_hist >= DTX_HANG_CONST));
        if !enc_dtx {
            self.dtx_hangover_count = DTX_HANG_CONST;
        } else if self.dec_ana_elapsed_count > DTX_ELAPSED_FRAMES_THRESH {
            self.dtx_hangover_added = true;
            self.dec_ana_elapsed_count = 0;
            self.dtx_hangover_count = 0;
        } else if self.dtx_hangover_count == 0 {
            self.dec_ana_elapsed_count = 0;
        } else {
            self.dtx_hangover_count = sub(self.dtx_hangover_count, 1);
        }
        if new_state != Dtx::Speech {
            self.sid_frame = false;
            self.valid_data = false;
            match ft {
                SidFirst => self.sid_frame = true,
                SidUpdate => {
                    self.sid_frame = true;
                    self.valid_data = true;
                }
                SidBad => {
                    self.sid_frame = true;
                    self.dtx_hangover_added = false;
                }
                _ => {}
            }
        }
        new_state
    }
}
