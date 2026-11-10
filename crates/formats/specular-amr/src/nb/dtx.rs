//! AMR comfort noise and discontinuous transmission on the receive side (TS 26.092,
//! TS 26.093): the DTX state machine, SID decoding and the comfort-noise synthesis.

use super::tables as t;
use super::{
    CbGainAverage, DPlsf, GcPred, L_FRAME, L_SUBFR, LSF_GAP, M, MP1, MRDTX, log2, lsf_lsp, lsp_az,
    lsp_lsf, pow2, reorder_lsf, syn_filt,
};
use crate::RxType;
use crate::basic::*;

const DTX_HIST_SIZE: usize = 8;
const DTX_ELAPSED_FRAMES_THRESH: i16 = 24 + 7 - 1;
const DTX_HANG_CONST: i16 = 7;
const DTX_MAX_EMPTY_THRESH: i16 = 50;
const PN_INITIAL_SEED: i32 = 0x7081_6958;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum State {
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
    l_pn_seed_rx: i32,
    lsp: [i16; M],
    lsp_old: [i16; M],
    lsf_hist: [i16; M * DTX_HIST_SIZE],
    lsf_hist_ptr: usize,
    lsf_hist_mean: [i16; M * DTX_HIST_SIZE],
    log_pg_mean: i16,
    log_en_hist: [i16; DTX_HIST_SIZE],
    log_en_hist_ptr: usize,
    log_en_adjust: i16,
    dtx_hangover_count: i16,
    dec_ana_elapsed_count: i16,
    sid_frame: bool,
    valid_data: bool,
    dtx_hangover_added: bool,
    pub(crate) global_state: State,
    data_updated: bool,
}

fn pseudonoise(reg: &mut i32, no_bits: u32) -> i16 {
    let mut noise_bits: i16 = 0;
    for _ in 0..no_bits {
        let mut sn = *reg & 1;
        if *reg & 0x1000_0000 != 0 {
            sn ^= 1;
        }
        noise_bits = shl(noise_bits, 1) | (extract_l(*reg) & 1);
        *reg = l_shr(*reg, 1);
        if sn & 1 != 0 {
            *reg |= 0x4000_0000;
        }
    }
    noise_bits
}

fn build_cn_code(seed: &mut i32, cod: &mut [i16; L_SUBFR]) {
    cod.fill(0);
    for k in 0..10 {
        let i = pseudonoise(seed, 2);
        let i = add(shr(extract_l(l_mult(i, 10)), 1), k as i16) as usize;
        let j = pseudonoise(seed, 1);
        cod[i] = if j > 0 { 4096 } else { -4096 };
    }
}

/// Reflection coefficients from LP coefficients (all zero if the filter is unstable).
fn a_refl(a: &[i16], refl: &mut [i16; M]) {
    let mut a_state = [0i16; M];
    let mut b_state = [0i16; M];
    a_state.copy_from_slice(&a[..M]);
    for i in (0..M).rev() {
        if abs_s(a_state[i]) >= 4096 {
            refl.fill(0);
            return;
        }
        refl[i] = shl(a_state[i], 3);
        let l = l_sub(MAX_32, l_mult(refl[i], refl[i]));
        let norm_shift = norm_l(l);
        let scale = sub(15, norm_shift);
        let norm_prod = round(l_shl(l, norm_shift));
        let mul = div_s(16384, norm_prod);
        for j in 0..i {
            let l = l_msu(l_deposit_h(a_state[j]), refl[i], a_state[i - j - 1]);
            let temp = round(l);
            let l = l_shr_r(l_mult(mul, temp), scale);
            if l_abs(l) > 32767 {
                refl.fill(0);
                return;
            }
            b_state[j] = extract_l(l);
        }
        a_state[..i].copy_from_slice(&b_state[..i]);
    }
}

impl DtxDec {
    pub(crate) fn new() -> Self {
        let mut lsf_hist = [0i16; M * DTX_HIST_SIZE];
        for c in lsf_hist.chunks_mut(M) {
            c.copy_from_slice(&t::MEAN_LSF_5);
        }
        DtxDec {
            since_last_sid: 0,
            true_sid_period_inv: 1 << 13,
            log_en: 3500,
            old_log_en: 3500,
            l_pn_seed_rx: PN_INITIAL_SEED,
            lsp: t::LSP_INIT_DATA,
            lsp_old: t::LSP_INIT_DATA,
            lsf_hist,
            lsf_hist_ptr: 0,
            lsf_hist_mean: [0; M * DTX_HIST_SIZE],
            log_pg_mean: 0,
            log_en_hist: [3500; DTX_HIST_SIZE],
            log_en_hist_ptr: 0,
            log_en_adjust: 0,
            dtx_hangover_count: DTX_HANG_CONST,
            dec_ana_elapsed_count: 32767,
            sid_frame: false,
            valid_data: false,
            dtx_hangover_added: false,
            global_state: State::Dtx,
            data_updated: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decode(
        &mut self,
        mem_syn: &mut [i16; M],
        lsf_state: &mut DPlsf,
        pred: &mut GcPred,
        aver: &mut CbGainAverage,
        new_state: State,
        mode: u8,
        parm: &[i16],
        synth: &mut [i16; L_FRAME],
        a_t: &mut [i16; 4 * MP1],
    ) {
        const LSF_HIST_MEAN_SCALE: [i16; M] =
            [20000, 20000, 20000, 20000, 20000, 18000, 16384, 8192, 0, 0];
        const DTX_LOG_EN_ADJUST: [i16; 9] = [-1023, -878, -732, -586, -440, -294, -148, 0, 0];
        let adj = DTX_LOG_EN_ADJUST[(mode as usize).min(8)];

        if self.dtx_hangover_added && self.sid_frame {
            self.log_en_adjust = adj;
            let mut ptr = self.lsf_hist_ptr + M;
            if ptr == 80 {
                ptr = 0;
            }
            self.lsf_hist
                .copy_within(self.lsf_hist_ptr..self.lsf_hist_ptr + M, ptr);
            let mut ptr = self.log_en_hist_ptr + 1;
            if ptr == DTX_HIST_SIZE {
                ptr = 0;
            }
            self.log_en_hist[ptr] = self.log_en_hist[self.log_en_hist_ptr];
            self.log_en = 0;
            let mut l_lsf = [0i32; M];
            for i in 0..DTX_HIST_SIZE {
                self.log_en = add(self.log_en, shr(self.log_en_hist[i], 3));
                for j in 0..M {
                    l_lsf[j] = l_add(l_lsf[j], l_deposit_l(self.lsf_hist[i * M + j]));
                }
            }
            let mut lsf = [0i16; M];
            for j in 0..M {
                lsf[j] = extract_l(l_shr(l_lsf[j], 3));
            }
            lsf_lsp(&lsf, &mut self.lsp);
            self.log_en = sub(self.log_en, self.log_en_adjust);
            self.lsf_hist_mean = self.lsf_hist;
            for i in 0..M {
                let mut l_mean = 0i32;
                for j in 0..8 {
                    l_mean = l_add(l_mean, l_deposit_l(self.lsf_hist_mean[i + j * M]));
                }
                let mean = extract_l(l_shr(l_mean, 3));
                for j in 0..8 {
                    let k = i + j * M;
                    let mut v = sub(self.lsf_hist_mean[k], mean);
                    v = mult(v, LSF_HIST_MEAN_SCALE[i]);
                    let negative = v < 0;
                    v = abs_s(v);
                    if v > 655 {
                        v = add(655, shr(sub(v, 655), 2));
                    }
                    if v > 1310 {
                        v = 1310;
                    }
                    if negative {
                        v = -v;
                    }
                    self.lsf_hist_mean[k] = v;
                }
            }
        }

        if self.sid_frame {
            self.lsp_old = self.lsp;
            self.old_log_en = self.log_en;
            if self.valid_data {
                let mut tmp_int_length = self.since_last_sid;
                self.since_last_sid = 0;
                if tmp_int_length > 32 {
                    tmp_int_length = 32;
                }
                self.true_sid_period_inv = if tmp_int_length >= 2 {
                    div_s(1 << 10, shl(tmp_int_length, 10))
                } else {
                    1 << 14
                };
                lsf_state.past_r_q.copy_from_slice(
                    &t::PAST_RQ_INIT[parm[0] as usize * M..parm[0] as usize * M + M],
                );
                lsf_state.d_plsf_3(MRDTX, false, &parm[1..4], &mut self.lsp);
                lsf_state.past_r_q = [0; M];
                let log_en_index = parm[4];
                self.log_en = shl(log_en_index, 11 - 2);
                self.log_en = sub(self.log_en, 2560 * 2);
                if log_en_index == 0 {
                    self.log_en = MIN_16;
                }
                if !self.data_updated || self.global_state == State::Speech {
                    self.lsp_old = self.lsp;
                    self.old_log_en = self.log_en;
                }
            }
            let ma_pred_init = sub(shr(self.log_en, 1), 9000).clamp(-14436, 0);
            pred.past_qua_en = [ma_pred_init; 4];
            pred.past_qua_en_mr122 = [mult(5443, ma_pred_init); 4];
        }

        self.log_en_adjust = add(
            mult(self.log_en_adjust, 29491),
            shr(mult(shl(adj, 5), 3277), 5),
        );
        let mut int_fac = shl(add(1, self.since_last_sid), 10);
        int_fac = mult(int_fac, self.true_sid_period_inv);
        if int_fac > 1024 {
            int_fac = 1024;
        }
        int_fac = shl(int_fac, 4);
        let mut l_log_en_int = l_mult(int_fac, self.log_en);
        let mut lsp_int = [0i16; M];
        for i in 0..M {
            lsp_int[i] = mult(int_fac, self.lsp[i]);
        }
        let int_fac = sub(16384, int_fac);
        l_log_en_int = l_mac(l_log_en_int, int_fac, self.old_log_en);
        for i in 0..M {
            lsp_int[i] = add(lsp_int[i], mult(int_fac, self.lsp_old[i]));
            lsp_int[i] = shl(lsp_int[i], 1);
        }

        let mut lsf_variab_factor = sub(self.log_pg_mean, 2457);
        lsf_variab_factor = sub(4096, mult(lsf_variab_factor, 9830));
        lsf_variab_factor = lsf_variab_factor.clamp(0, 4096);
        lsf_variab_factor = shl(lsf_variab_factor, 3);
        let lsf_variab_index = pseudonoise(&mut self.l_pn_seed_rx, 3) as usize;
        let mut lsf_int = [0i16; M];
        lsp_lsf(&lsp_int, &mut lsf_int);
        let mut lsf_int_variab = lsf_int;
        for i in 0..M {
            lsf_int_variab[i] = add(
                lsf_int_variab[i],
                mult(
                    lsf_variab_factor,
                    self.lsf_hist_mean[i + lsf_variab_index * M],
                ),
            );
        }
        reorder_lsf(&mut lsf_int, LSF_GAP);
        reorder_lsf(&mut lsf_int_variab, LSF_GAP);
        lsf_state.past_lsf_q = lsf_int;
        let mut lsp_int_variab = [0i16; M];
        lsf_lsp(&lsf_int, &mut lsp_int);
        lsf_lsp(&lsf_int_variab, &mut lsp_int_variab);
        let mut acoeff = [0i16; MP1];
        let mut acoeff_variab = [0i16; MP1];
        lsp_az(&lsp_int, &mut acoeff);
        lsp_az(&lsp_int_variab, &mut acoeff_variab);
        for k in 0..4 {
            a_t[k * MP1..k * MP1 + MP1].copy_from_slice(&acoeff);
        }
        let mut refl = [0i16; M];
        a_refl(&acoeff[1..], &mut refl);
        let mut pred_err = MAX_16;
        for &r in &refl {
            pred_err = mult(pred_err, sub(MAX_16, mult(r, r)));
        }
        let (log_pg_e, log_pg_m) = log2(l_deposit_l(pred_err));
        let log_pg = shl(sub(log_pg_e, 15), 12);
        let log_pg = shr(sub(0, add(log_pg, shr(log_pg_m, 15 - 12))), 1);
        self.log_pg_mean = add(mult(29491, self.log_pg_mean), mult(3277, log_pg));

        l_log_en_int = l_shr(l_log_en_int, 10);
        l_log_en_int = l_add(l_log_en_int, 4 * 65536);
        l_log_en_int = l_sub(l_log_en_int, l_shl(l_deposit_l(log_pg), 4));
        l_log_en_int = l_add(l_log_en_int, l_shl(l_deposit_l(self.log_en_adjust), 5));
        let e = extract_h(l_log_en_int);
        let m = extract_l(l_shr(l_sub(l_log_en_int, l_deposit_h(e)), 1));
        let level = extract_l(pow2(e, m));
        let mut ex = [0i16; L_SUBFR];
        for i in 0..4 {
            build_cn_code(&mut self.l_pn_seed_rx, &mut ex);
            for v in ex.iter_mut() {
                *v = mult(level, *v);
            }
            syn_filt(
                &acoeff_variab,
                &ex,
                &mut synth[i * L_SUBFR..],
                L_SUBFR,
                mem_syn,
                true,
            );
        }
        aver.hang_var = 20;
        aver.hang_count = 0;

        if new_state == State::DtxMute {
            let mut tmp_int_length = self.since_last_sid.min(32);
            if tmp_int_length <= 0 {
                tmp_int_length = 8;
            }
            self.true_sid_period_inv = div_s(1 << 10, shl(tmp_int_length, 10));
            self.since_last_sid = 0;
            self.lsp_old = self.lsp;
            self.old_log_en = self.log_en;
            self.log_en = sub(self.log_en, 256);
        }
        if self.sid_frame && (self.valid_data || self.dtx_hangover_added) {
            self.since_last_sid = 0;
            self.data_updated = true;
        }
    }

    pub(crate) fn activity_update(&mut self, lsf: &[i16; M], frame: &[i16]) {
        self.lsf_hist_ptr += M;
        if self.lsf_hist_ptr == 80 {
            self.lsf_hist_ptr = 0;
        }
        self.lsf_hist[self.lsf_hist_ptr..self.lsf_hist_ptr + M].copy_from_slice(lsf);
        let mut l = 0i32;
        for &v in &frame[..L_FRAME] {
            l = l_mac(l, v, v);
        }
        let (e, m) = log2(l);
        let mut log_en = shl(e, 10);
        log_en = add(log_en, shr(m, 15 - 10));
        log_en = sub(log_en, 7497 + 1024);
        self.log_en_hist_ptr += 1;
        if self.log_en_hist_ptr == DTX_HIST_SIZE {
            self.log_en_hist_ptr = 0;
        }
        self.log_en_hist[self.log_en_hist_ptr] = log_en;
    }

    pub(crate) fn rx_handler(&mut self, ft: RxType) -> State {
        use RxType::*;
        let in_dtx = matches!(self.global_state, State::Dtx | State::DtxMute);
        let mut new_state;
        if matches!(ft, SidFirst | SidUpdate | SidBad)
            || (in_dtx && matches!(ft, NoData | SpeechBad | Onset))
        {
            new_state = State::Dtx;
            if self.global_state == State::DtxMute
                && matches!(ft, SidBad | SidFirst | Onset | NoData)
            {
                new_state = State::DtxMute;
            }
            self.since_last_sid = add(self.since_last_sid, 1);
            if ft != SidUpdate && self.since_last_sid > DTX_MAX_EMPTY_THRESH {
                new_state = State::DtxMute;
            }
        } else {
            new_state = State::Speech;
            self.since_last_sid = 0;
        }
        if !self.data_updated && ft == SidUpdate {
            self.dec_ana_elapsed_count = 0;
        }
        self.dec_ana_elapsed_count = add(self.dec_ana_elapsed_count, 1);
        self.dtx_hangover_added = false;
        let enc_speech = if matches!(ft, SidFirst | SidUpdate | SidBad | Onset | NoData) {
            ft == NoData && new_state == State::Speech
        } else {
            true
        };
        if enc_speech {
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
        if new_state != State::Speech {
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
