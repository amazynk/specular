//! The CELT decoder (RFC 6716 section 4.3): one frame of 2.5 to 20 ms at 48 kHz.

use crate::celt_alloc::{BITRES, MAX_FINE_BITS, NB_EBANDS, compute_allocation, eb, init_caps};
use crate::celt_bands::{SPREAD_NONE, anti_collapse, decode_all_bands, lcg_rand};
use crate::celt_data::{E_MEANS_Q6, E_PROB_MODEL};
use crate::mdct::{Mdct, OVERLAP};
use crate::range::RangeDecoder;
use crate::rfc_tables::CELT_TRIM;

const DECODE_BUFFER: usize = 2048;
const MEM: usize = DECODE_BUFFER + OVERLAP;
const SHORT_MDCT: usize = 120;
const PREEMPH: f32 = 0.850_006_1;
const COMBFILTER_MINPERIOD: i32 = 15;

const SPREAD_ICDF: [u8; 4] = [25, 23, 2, 0];
const TAPSET_ICDF: [u8; 3] = [2, 1, 0];
const SMALL_ENERGY_ICDF: [u8; 3] = [2, 1, 0];
const PRED_COEF: [f32; 4] = [
    29440.0 / 32768.0,
    26112.0 / 32768.0,
    21248.0 / 32768.0,
    16384.0 / 32768.0,
];
const BETA_COEF: [f32; 4] = [
    30147.0 / 32768.0,
    22282.0 / 32768.0,
    12124.0 / 32768.0,
    6554.0 / 32768.0,
];
const BETA_INTRA: f32 = 4915.0 / 32768.0;

/// TF resolution changes (RFC 6716 tables 60 to 63): [LM][4 * transient + 2 * tf_select + tf].
const TF_SELECT: [[i8; 8]; 4] = [
    [0, -1, 0, -1, 0, -1, 0, -1],
    [0, -1, 0, -2, 1, 0, 1, -1],
    [0, -2, 0, -3, 2, 0, 1, -1],
    [0, -2, 0, -3, 3, 0, 1, -1],
];

const COMB_GAINS: [[f32; 3]; 3] = [
    [0.306_640_63, 0.217_041_02, 0.129_638_67],
    [0.463_867_2, 0.268_066_4, 0.0],
    [0.799_804_7, 0.100_097_656, 0.0],
];

/// Decoder state of one CELT stream.
pub(crate) struct CeltDecoder {
    /// Output channels.
    channels: usize,
    /// Coded channels of the current frame.
    pub(crate) stream_channels: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) disable_inv: bool,
    mem: [Vec<f32>; 2],
    old_band_e: [f32; 2 * NB_EBANDS],
    old_log_e: [f32; 2 * NB_EBANDS],
    old_log_e2: [f32; 2 * NB_EBANDS],
    background_log_e: [f32; 2 * NB_EBANDS],
    preemph_mem: [f32; 2],
    pf_period: i32,
    pf_period_old: i32,
    pf_gain: f32,
    pf_gain_old: f32,
    pf_tapset: usize,
    pf_tapset_old: usize,
    rng: u32,
    loss_count: u32,
    mdct: Mdct,
    // Scratch.
    x: Vec<f32>,
    freq: Vec<f32>,
}

/// Laplace-distributed integer (ec_laplace_decode) with P(0) = fs/32768 and decay `decay`/16384.
fn laplace_decode(ec: &mut RangeDecoder, fs0: u32, decay: u32) -> i32 {
    let mut val = 0i32;
    let (fm, ext) = ec.decode_bin(15);
    let mut fl = 0u32;
    let mut fs = fs0;
    if fm >= fs {
        val += 1;
        fl = fs;
        fs = (((32768 - 32 - fs0) * (16384 - decay)) >> 15) + 1;
        while fs > 1 && fm >= fl + 2 * fs {
            fs *= 2;
            fl += fs;
            fs = ((fs - 2) * decay) >> 15;
            fs += 1;
            val += 1;
        }
        if fs <= 1 {
            let di = (fm - fl) >> 1;
            val += di as i32;
            fl += 2 * di;
        }
        if fm < fl + fs {
            val = -val;
        } else {
            fl += fs;
        }
    }
    ec.update(ext, fl, (fl + fs).min(32768), 32768);
    val
}

impl CeltDecoder {
    pub(crate) fn new(channels: usize) -> CeltDecoder {
        let mut d = CeltDecoder {
            channels,
            stream_channels: channels,
            start: 0,
            end: NB_EBANDS,
            disable_inv: false,
            mem: [vec![0.0; MEM], vec![0.0; MEM]],
            old_band_e: [0.0; 2 * NB_EBANDS],
            old_log_e: [0.0; 2 * NB_EBANDS],
            old_log_e2: [0.0; 2 * NB_EBANDS],
            background_log_e: [0.0; 2 * NB_EBANDS],
            preemph_mem: [0.0; 2],
            pf_period: 0,
            pf_period_old: 0,
            pf_gain: 0.0,
            pf_gain_old: 0.0,
            pf_tapset: 0,
            pf_tapset_old: 0,
            rng: 0,
            loss_count: 0,
            mdct: Mdct::new(),
            x: vec![0.0; 2 * 960],
            freq: vec![0.0; 960],
        };
        d.reset();
        d
    }

    /// OPUS_RESET_STATE.
    pub(crate) fn reset(&mut self) {
        for m in self.mem.iter_mut() {
            m.fill(0.0);
        }
        self.old_band_e = [0.0; 2 * NB_EBANDS];
        self.old_log_e = [-28.0; 2 * NB_EBANDS];
        self.old_log_e2 = [-28.0; 2 * NB_EBANDS];
        self.background_log_e = [0.0; 2 * NB_EBANDS];
        self.preemph_mem = [0.0; 2];
        self.pf_period = 0;
        self.pf_period_old = 0;
        self.pf_gain = 0.0;
        self.pf_gain_old = 0.0;
        self.pf_tapset = 0;
        self.pf_tapset_old = 0;
        self.rng = 0;
        self.loss_count = 0;
    }

    /// Final range of the last decoded frame.
    pub(crate) fn rng(&self) -> u32 {
        self.rng
    }

    /// Decodes one frame of `n` samples per channel (120, 240, 480 or 960) from `ec` into
    /// `pcm` (interleaved, `channels` channels); `None` conceals a lost frame.
    pub(crate) fn decode(&mut self, ec: Option<&mut RangeDecoder>, n: usize, pcm: &mut [f32]) {
        let lm = match n {
            120 => 0,
            240 => 1,
            480 => 2,
            _ => 3,
        };
        let ec = match ec {
            Some(ec) if ec.storage() > 1 => ec,
            _ => {
                self.decode_lost(n, lm);
                self.deemphasis(pcm, n);
                return;
            }
        };
        let c = self.stream_channels;
        let cc = self.channels;
        let start = self.start;
        let end = self.end;
        let m = 1usize << lm;
        let eff_end = end.min(NB_EBANDS);
        let len = ec.storage() as i32;

        if c == 1 {
            for i in 0..NB_EBANDS {
                self.old_band_e[i] = self.old_band_e[i].max(self.old_band_e[NB_EBANDS + i]);
            }
        }
        let mut total_bits = len * 8;
        let mut tell = ec.tell();
        let silence = if tell >= total_bits {
            true
        } else if tell == 1 {
            ec.bit_logp(15)
        } else {
            false
        };
        if silence {
            ec.skip_to_end();
            tell = len * 8;
        }
        let mut pf_gain = 0f32;
        let mut pf_pitch = 0i32;
        let mut pf_tapset = 0usize;
        if start == 0 && tell + 16 <= total_bits {
            if ec.bit_logp(1) {
                let octave = ec.uint(6);
                pf_pitch = ((16 << octave) + ec.bits(4 + octave) - 1) as i32;
                let qg = ec.bits(3);
                if ec.tell() + 2 <= total_bits {
                    pf_tapset = ec.icdf(&TAPSET_ICDF, 2);
                }
                pf_gain = 0.09375 * (qg + 1) as f32;
            }
            tell = ec.tell();
        }
        let is_transient = if lm > 0 && tell + 3 <= total_bits {
            let t = ec.bit_logp(3);
            tell = ec.tell();
            t
        } else {
            false
        };
        let intra = if tell + 3 <= total_bits {
            ec.bit_logp(3)
        } else {
            false
        };
        self.unquant_coarse(ec, start, end, intra, c, lm);

        let tf_res = tf_decode(ec, start, end, is_transient, lm);

        tell = ec.tell();
        let spread = if tell + 4 <= total_bits {
            ec.icdf(&SPREAD_ICDF, 5) as u32
        } else {
            2
        };

        let mut cap = [0i32; NB_EBANDS];
        init_caps(&mut cap, lm, c);
        let mut offsets = [0i32; NB_EBANDS];
        let mut dynalloc_logp = 6;
        total_bits <<= BITRES;
        let mut tellf = ec.tell_frac();
        for i in start..end {
            let width = (c as i32 * (eb(i + 1) - eb(i))) << lm;
            let quanta = (width << BITRES).min((6 << BITRES).max(width));
            let mut loop_logp = dynalloc_logp;
            let mut boost = 0;
            while tellf + (loop_logp << BITRES) < total_bits && boost < cap[i] {
                let flag = ec.bit_logp(loop_logp as u32);
                tellf = ec.tell_frac();
                if !flag {
                    break;
                }
                boost += quanta;
                total_bits -= quanta;
                loop_logp = 1;
            }
            offsets[i] = boost;
            if boost > 0 {
                dynalloc_logp = (dynalloc_logp - 1).max(2);
            }
        }
        let alloc_trim = if tellf + (6 << BITRES) <= total_bits {
            ec.icdf(&CELT_TRIM, 7) as i32
        } else {
            5
        };
        let mut bits = ((len * 8) << BITRES) - ec.tell_frac() - 1;
        let anti_collapse_rsv = if is_transient && lm >= 2 && bits >= (lm as i32 + 2) << BITRES {
            1 << BITRES
        } else {
            0
        };
        bits -= anti_collapse_rsv;
        let alloc = compute_allocation(start, end, &offsets, &cap, alloc_trim, bits, c, lm, ec);

        // Fine energy.
        for i in start..end {
            let fq = alloc.fine_quant[i];
            if fq <= 0 {
                continue;
            }
            for ch in 0..c {
                let q2 = ec.bits(fq as u32);
                let offset = (q2 as f32 + 0.5) * (1 << (14 - fq)) as f32 * (1.0 / 16384.0) - 0.5;
                self.old_band_e[i + ch * NB_EBANDS] += offset;
            }
        }

        for ch in 0..cc {
            self.mem[ch].copy_within(n..DECODE_BUFFER + OVERLAP / 2, 0);
        }

        // Shapes.
        let mut collapse = [0u8; 2 * NB_EBANDS];
        let mut x = std::mem::take(&mut self.x);
        x.clear();
        x.resize(c * n, 0.0);
        {
            let (xa, ya) = x.split_at_mut(n);
            decode_all_bands(
                ec,
                start,
                end,
                xa,
                if c == 2 { Some(ya) } else { None },
                &mut collapse,
                &alloc.pulses,
                is_transient,
                spread,
                alloc.dual_stereo,
                alloc.intensity,
                &tf_res,
                (len * (8 << BITRES)) - anti_collapse_rsv,
                alloc.balance,
                lm,
                alloc.coded_bands,
                &mut self.rng,
                self.disable_inv,
            );
        }
        let anti_collapse_on = anti_collapse_rsv > 0 && ec.bits(1) != 0;

        // Final fine energy bits.
        let mut bits_left = len * 8 - ec.tell();
        for prio in 0..2 {
            let mut i = start;
            while i < end && bits_left >= c as i32 {
                if alloc.fine_quant[i] < MAX_FINE_BITS && alloc.fine_priority[i] == prio {
                    for ch in 0..c {
                        let q2 = ec.bits(1);
                        let offset = (q2 as f32 - 0.5)
                            * (1 << (14 - alloc.fine_quant[i] - 1)) as f32
                            * (1.0 / 16384.0);
                        self.old_band_e[i + ch * NB_EBANDS] += offset;
                        bits_left -= 1;
                    }
                }
                i += 1;
            }
        }

        if anti_collapse_on {
            anti_collapse(
                &mut x,
                &collapse,
                lm,
                c,
                n,
                start,
                end,
                &self.old_band_e,
                &self.old_log_e,
                &self.old_log_e2,
                &alloc.pulses,
                self.rng,
            );
        }
        if silence {
            self.old_band_e = [-28.0; 2 * NB_EBANDS];
        }
        self.synthesis(&x, n, lm, start, eff_end, c, is_transient, silence);
        self.x = x;

        // Post-filter.
        for ch in 0..cc {
            self.pf_period = self.pf_period.max(COMBFILTER_MINPERIOD);
            self.pf_period_old = self.pf_period_old.max(COMBFILTER_MINPERIOD);
            let off = DECODE_BUFFER - n;
            let window = &self.mdct.window;
            comb_filter(
                &mut self.mem[ch],
                off,
                self.pf_period_old,
                self.pf_period,
                SHORT_MDCT,
                self.pf_gain_old,
                self.pf_gain,
                self.pf_tapset_old,
                self.pf_tapset,
                window,
            );
            if lm != 0 {
                comb_filter(
                    &mut self.mem[ch],
                    off + SHORT_MDCT,
                    self.pf_period,
                    pf_pitch,
                    n - SHORT_MDCT,
                    self.pf_gain,
                    pf_gain,
                    self.pf_tapset,
                    pf_tapset,
                    window,
                );
            }
        }
        self.pf_period_old = self.pf_period;
        self.pf_gain_old = self.pf_gain;
        self.pf_tapset_old = self.pf_tapset;
        self.pf_period = pf_pitch;
        self.pf_gain = pf_gain;
        self.pf_tapset = pf_tapset;
        if lm != 0 {
            self.pf_period_old = self.pf_period;
            self.pf_gain_old = self.pf_gain;
            self.pf_tapset_old = self.pf_tapset;
        }

        if c == 1 {
            let (a, b) = self.old_band_e.split_at_mut(NB_EBANDS);
            b.copy_from_slice(a);
        }
        if !is_transient {
            self.old_log_e2 = self.old_log_e;
            self.old_log_e = self.old_band_e;
            for i in 0..2 * NB_EBANDS {
                self.background_log_e[i] =
                    (self.background_log_e[i] + m as f32 * 0.001).min(self.old_band_e[i]);
            }
        } else {
            for i in 0..2 * NB_EBANDS {
                self.old_log_e[i] = self.old_log_e[i].min(self.old_band_e[i]);
            }
        }
        for ch in 0..2 {
            for i in (0..start).chain(end..NB_EBANDS) {
                self.old_band_e[ch * NB_EBANDS + i] = 0.0;
                self.old_log_e[ch * NB_EBANDS + i] = -28.0;
                self.old_log_e2[ch * NB_EBANDS + i] = -28.0;
            }
        }
        self.rng = ec.rng();
        self.deemphasis(pcm, n);
        self.loss_count = 0;
    }

    fn unquant_coarse(
        &mut self,
        ec: &mut RangeDecoder,
        start: usize,
        end: usize,
        intra: bool,
        c: usize,
        lm: usize,
    ) {
        let prob = &E_PROB_MODEL[(lm * 2 + usize::from(intra)) * 42..][..42];
        let mut prev = [0f32; 2];
        let (coef, beta) = if intra {
            (0.0, BETA_INTRA)
        } else {
            (PRED_COEF[lm], BETA_COEF[lm])
        };
        let budget = (ec.storage() * 8) as i32;
        for i in start..end {
            for (ch, pv) in prev.iter_mut().enumerate().take(c) {
                let tell = ec.tell();
                let qi = if budget - tell >= 15 {
                    let pi = 2 * i.min(20);
                    laplace_decode(ec, u32::from(prob[pi]) << 7, u32::from(prob[pi + 1]) << 6)
                } else if budget - tell >= 2 {
                    let q = ec.icdf(&SMALL_ENERGY_ICDF, 2) as i32;
                    (q >> 1) ^ -(q & 1)
                } else if budget - tell >= 1 {
                    -i32::from(ec.bit_logp(1))
                } else {
                    -1
                };
                let q = qi as f32;
                let e = &mut self.old_band_e[i + ch * NB_EBANDS];
                *e = e.max(-9.0);
                let tmp = coef * *e + *pv + q;
                *e = tmp;
                *pv = *pv + q - beta * q;
            }
        }
    }

    /// Denormalisation and inverse MDCT into the decode buffers (celt_synthesis).
    #[allow(clippy::too_many_arguments)]
    fn synthesis(
        &mut self,
        x: &[f32],
        n: usize,
        lm: usize,
        start: usize,
        eff_end: usize,
        c: usize,
        transient: bool,
        silence: bool,
    ) {
        let cc = self.channels;
        let m = 1usize << lm;
        let (b, nb, shift) = if transient {
            (m, SHORT_MDCT, 3)
        } else {
            (1, n, 3 - lm)
        };
        let out_off = DECODE_BUFFER - n;
        let mut freq = std::mem::take(&mut self.freq);
        freq.resize(n, 0.0);
        if cc == 2 && c == 1 {
            denormalise(
                &x[..n],
                &mut freq,
                &self.old_band_e[..NB_EBANDS],
                start,
                eff_end,
                m,
                silence,
            );
            for ch in 0..2 {
                for bi in 0..b {
                    let out = &mut self.mem[ch][out_off + nb * bi..];
                    self.mdct.backward(&freq[bi..], b, out, shift);
                }
            }
        } else if cc == 1 && c == 2 {
            let mut f2 = vec![0f32; n];
            denormalise(
                &x[..n],
                &mut freq,
                &self.old_band_e[..NB_EBANDS],
                start,
                eff_end,
                m,
                silence,
            );
            denormalise(
                &x[n..2 * n],
                &mut f2,
                &self.old_band_e[NB_EBANDS..],
                start,
                eff_end,
                m,
                silence,
            );
            for (a, b) in freq.iter_mut().zip(f2.iter()) {
                *a = 0.5 * (*a + *b);
            }
            for bi in 0..b {
                let out = &mut self.mem[0][out_off + nb * bi..];
                self.mdct.backward(&freq[bi..], b, out, shift);
            }
        } else {
            for ch in 0..cc {
                denormalise(
                    &x[ch * n..(ch + 1) * n],
                    &mut freq,
                    &self.old_band_e[ch * NB_EBANDS..(ch + 1) * NB_EBANDS],
                    start,
                    eff_end,
                    m,
                    silence,
                );
                for bi in 0..b {
                    let out = &mut self.mem[ch][out_off + nb * bi..];
                    self.mdct.backward(&freq[bi..], b, out, shift);
                }
            }
        }
        self.freq = freq;
    }

    fn deemphasis(&mut self, pcm: &mut [f32], n: usize) {
        let cc = self.channels;
        for ch in 0..cc {
            let mut mm = self.preemph_mem[ch];
            let src = &self.mem[ch][DECODE_BUFFER - n..DECODE_BUFFER];
            for (j, &v) in src.iter().enumerate() {
                let tmp = v + 1e-30 + mm;
                mm = PREEMPH * tmp;
                pcm[j * cc + ch] = tmp * (1.0 / 32768.0);
            }
            self.preemph_mem[ch] = mm;
        }
    }

    /// Simple concealment of a lost frame: noise shaped by the last band energies, decaying
    /// (the reference decoder's noise-based concealment).
    fn decode_lost(&mut self, n: usize, lm: usize) {
        let c = self.channels;
        let start = self.start;
        let end = self.end;
        let eff_end = end.min(NB_EBANDS).max(start);
        for ch in 0..c {
            self.mem[ch].copy_within(n..DECODE_BUFFER + OVERLAP / 2, 0);
        }
        let decay = if self.loss_count == 0 { 1.5 } else { 0.5 };
        for ch in 0..c {
            for i in start..end {
                let k = ch * NB_EBANDS + i;
                self.old_band_e[k] = self.background_log_e[k].max(self.old_band_e[k] - decay);
            }
        }
        let mut seed = self.rng;
        let mut x = std::mem::take(&mut self.x);
        x.clear();
        x.resize(c * n, 0.0);
        for ch in 0..c {
            for i in start..eff_end {
                let off = n * ch + ((eb(i) as usize) << lm);
                let blen = ((eb(i + 1) - eb(i)) as usize) << lm;
                for v in x[off..off + blen].iter_mut() {
                    seed = lcg_rand(seed);
                    *v = ((seed as i32) >> 20) as f32;
                }
                let e: f32 = 1e-15 + x[off..off + blen].iter().map(|v| v * v).sum::<f32>();
                let g = 1.0 / e.sqrt();
                for v in x[off..off + blen].iter_mut() {
                    *v *= g;
                }
            }
        }
        self.rng = seed;
        let sc = self.stream_channels;
        self.stream_channels = c;
        self.synthesis(&x, n, lm, start, eff_end, c, false, false);
        self.stream_channels = sc;
        self.x = x;
        self.loss_count += 1;
    }
}

fn denormalise(
    x: &[f32],
    freq: &mut [f32],
    band_log_e: &[f32],
    start: usize,
    end: usize,
    m: usize,
    silence: bool,
) {
    let n = freq.len();
    let (start, end, bound) = if silence {
        (0, 0, 0)
    } else {
        (start, end, m * eb(end) as usize)
    };
    let s0 = m * eb(start) as usize;
    freq[..s0].fill(0.0);
    for i in start..end {
        let lg = band_log_e[i] + f32::from(E_MEANS_Q6[i]) / 16.0;
        let g = lg.min(32.0).exp2();
        let a = m * eb(i) as usize;
        let b = m * eb(i + 1) as usize;
        for j in a..b {
            freq[j] = x[j] * g;
        }
    }
    freq[bound.max(s0)..n].fill(0.0);
}

fn tf_decode(
    ec: &mut RangeDecoder,
    start: usize,
    end: usize,
    transient: bool,
    lm: usize,
) -> [i32; NB_EBANDS] {
    let mut tf_res = [0i32; NB_EBANDS];
    let mut budget = (ec.storage() * 8) as i32;
    let mut tell = ec.tell();
    let mut logp = if transient { 2 } else { 4 };
    let tf_select_rsv = lm > 0 && tell + logp < budget;
    budget -= i32::from(tf_select_rsv);
    let mut tf_changed = 0usize;
    let mut curr = 0usize;
    for r in tf_res.iter_mut().take(end).skip(start) {
        if tell + logp <= budget {
            curr ^= usize::from(ec.bit_logp(logp as u32));
            tell = ec.tell();
            tf_changed |= curr;
        }
        *r = curr as i32;
        logp = if transient { 4 } else { 5 };
    }
    let t = 4 * usize::from(transient);
    let mut tf_select = 0usize;
    if tf_select_rsv && TF_SELECT[lm][t + tf_changed] != TF_SELECT[lm][t + 2 + tf_changed] {
        tf_select = usize::from(ec.bit_logp(1));
    }
    for r in tf_res.iter_mut().take(end).skip(start) {
        *r = i32::from(TF_SELECT[lm][t + 2 * tf_select + *r as usize]);
    }
    tf_res
}

/// The pitch post-filter over buf[off .. off + n] in place, cross-fading from (t0, g0, tapset0)
/// to (t1, g1, tapset1) over the first `OVERLAP` samples.
#[allow(clippy::too_many_arguments)]
fn comb_filter(
    buf: &mut [f32],
    off: usize,
    t0: i32,
    t1: i32,
    n: usize,
    g0: f32,
    g1: f32,
    tapset0: usize,
    tapset1: usize,
    window: &[f32],
) {
    if g0 == 0.0 && g1 == 0.0 {
        return;
    }
    let t0 = t0.max(COMBFILTER_MINPERIOD) as usize;
    let t1 = t1.max(COMBFILTER_MINPERIOD) as usize;
    let g00 = g0 * COMB_GAINS[tapset0][0];
    let g01 = g0 * COMB_GAINS[tapset0][1];
    let g02 = g0 * COMB_GAINS[tapset0][2];
    let g10 = g1 * COMB_GAINS[tapset1][0];
    let g11 = g1 * COMB_GAINS[tapset1][1];
    let g12 = g1 * COMB_GAINS[tapset1][2];
    let mut x1 = buf[off - t1 + 1];
    let mut x2 = buf[off - t1];
    let mut x3 = buf[off - t1 - 1];
    let mut x4 = buf[off - t1 - 2];
    let overlap = if g0 == g1 && t0 == t1 && tapset0 == tapset1 {
        0
    } else {
        OVERLAP.min(n)
    };
    let mut i = 0;
    while i < overlap {
        let p = off + i;
        let x0 = buf[p - t1 + 2];
        let f = window[i] * window[i];
        let v = buf[p]
            + (1.0 - f) * g00 * buf[p - t0]
            + (1.0 - f) * g01 * (buf[p - t0 + 1] + buf[p - t0 - 1])
            + (1.0 - f) * g02 * (buf[p - t0 + 2] + buf[p - t0 - 2])
            + f * g10 * x2
            + f * g11 * (x1 + x3)
            + f * g12 * (x0 + x4);
        buf[p] = v;
        x4 = x3;
        x3 = x2;
        x2 = x1;
        x1 = x0;
        i += 1;
    }
    if g1 == 0.0 {
        return;
    }
    while i < n {
        let p = off + i;
        let x0 = buf[p - t1 + 2];
        buf[p] += g10 * x2 + g11 * (x1 + x3) + g12 * (x0 + x4);
        x4 = x3;
        x3 = x2;
        x2 = x1;
        x1 = x0;
        i += 1;
    }
}

#[allow(dead_code)]
const _: u32 = SPREAD_NONE;
