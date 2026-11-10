//! The AMR adaptive post-filter (formant and tilt filters with gain control) and the
//! 60 Hz high-pass output filter.

use super::{L_FRAME, L_SUBFR, M, MP1, MR102, MR122, agc_g0, residu, syn_filt, weight_ai};
use crate::basic::*;

const L_H: usize = 22;
const MU: i16 = 26214;
const AGC_FAC: i16 = 29491;

const GAMMA3_MR122: [i16; M] = [22938, 16057, 11240, 7868, 5508, 3856, 2699, 1889, 1322, 925];
const GAMMA3: [i16; M] = [18022, 9912, 5451, 2998, 1649, 907, 499, 274, 151, 83];
const GAMMA4_MR122: [i16; M] = [
    24576, 18432, 13824, 10368, 7776, 5832, 4374, 3281, 2461, 1846,
];
const GAMMA4: [i16; M] = [22938, 16057, 11240, 7868, 5508, 3856, 2699, 1889, 1322, 925];

#[derive(Clone)]
pub(crate) struct PostFilter {
    mem_syn_pst: [i16; M],
    res2: [i16; L_SUBFR],
    synth_buf: [i16; L_FRAME + M],
    mem_pre: i16,
    past_gain: i16,
}

impl PostFilter {
    pub(crate) fn new() -> Self {
        PostFilter {
            mem_syn_pst: [0; M],
            res2: [0; L_SUBFR],
            synth_buf: [0; L_FRAME + M],
            mem_pre: 0,
            past_gain: 4096,
        }
    }

    fn preemphasis(&mut self, g: i16) {
        let s = &mut self.res2;
        let temp = s[L_SUBFR - 1];
        for i in (1..L_SUBFR).rev() {
            s[i] = sub(s[i], mult(g, s[i - 1]));
        }
        s[0] = sub(s[0], mult(g, self.mem_pre));
        self.mem_pre = temp;
    }

    fn agc(&mut self, sig_in: &[i16], sig_out: &mut [i16]) {
        let g0 = match agc_g0(sig_in, sig_out) {
            None => {
                self.past_gain = 0;
                return;
            }
            Some(0) => 0,
            Some(i) => mult(i, sub(32767, AGC_FAC)),
        };
        let mut gain = self.past_gain;
        for v in sig_out.iter_mut() {
            gain = mult(gain, AGC_FAC);
            gain = add(gain, g0);
            *v = extract_h(l_shl(l_mult(*v, gain), 3));
        }
        self.past_gain = gain;
    }

    pub(crate) fn filter(&mut self, mode: u8, syn: &mut [i16; L_FRAME], az_4: &[i16]) {
        self.synth_buf[M..].copy_from_slice(&syn[..]);
        let (g3, g4) = if mode == MR122 || mode == MR102 {
            (&GAMMA3_MR122, &GAMMA4_MR122)
        } else {
            (&GAMMA3, &GAMMA4)
        };
        for (k, i_subfr) in (0..L_FRAME).step_by(L_SUBFR).enumerate() {
            let az = &az_4[k * MP1..k * MP1 + MP1];
            let mut ap3 = [0i16; MP1];
            let mut ap4 = [0i16; MP1];
            weight_ai(az, g3, &mut ap3);
            weight_ai(az, g4, &mut ap4);
            residu(&ap3, &self.synth_buf, M + i_subfr, &mut self.res2, L_SUBFR);
            let mut h = [0i16; L_H];
            h[..MP1].copy_from_slice(&ap3);
            let x = h;
            let zero = [0i16; M];
            let mut mem = zero;
            syn_filt(&ap4, &x, &mut h, L_H, &mut mem, false);
            let mut l = l_mult(h[0], h[0]);
            for &v in &h[1..] {
                l = l_mac(l, v, v);
            }
            let temp1 = extract_h(l);
            let mut l = l_mult(h[0], h[1]);
            for i in 1..L_H - 1 {
                l = l_mac(l, h[i], h[i + 1]);
            }
            let mut temp2 = extract_h(l);
            if temp2 <= 0 {
                temp2 = 0;
            } else {
                temp2 = mult(temp2, MU);
                temp2 = div_s(temp2, temp1);
            }
            self.preemphasis(temp2);
            let res2 = self.res2;
            syn_filt(
                &ap4,
                &res2,
                &mut syn[i_subfr..],
                L_SUBFR,
                &mut self.mem_syn_pst,
                true,
            );
            let work: [i16; L_SUBFR] = self.synth_buf[M + i_subfr..M + i_subfr + L_SUBFR]
                .try_into()
                .unwrap_or([0; L_SUBFR]);
            self.agc(&work, &mut syn[i_subfr..i_subfr + L_SUBFR]);
        }
        self.synth_buf.copy_within(L_FRAME..L_FRAME + M, 0);
    }
}

/// Second-order high-pass at 60 Hz with a factor-2 gain (TS 26.090 §6.2.2).
#[derive(Clone)]
pub(crate) struct PostProcess {
    y2_hi: i16,
    y2_lo: i16,
    y1_hi: i16,
    y1_lo: i16,
    x0: i16,
    x1: i16,
}

impl PostProcess {
    pub(crate) fn new() -> Self {
        PostProcess {
            y2_hi: 0,
            y2_lo: 0,
            y1_hi: 0,
            y1_lo: 0,
            x0: 0,
            x1: 0,
        }
    }

    pub(crate) fn process(&mut self, signal: &mut [i16]) {
        const B: [i16; 3] = [7699, -15398, 7699];
        const A: [i16; 3] = [8192, 15836, -7667];
        for v in signal.iter_mut() {
            let x2 = self.x1;
            self.x1 = self.x0;
            self.x0 = *v;
            let mut l = mpy_32_16(self.y1_hi, self.y1_lo, A[1]);
            l = l_add(l, mpy_32_16(self.y2_hi, self.y2_lo, A[2]));
            l = l_mac(l, self.x0, B[0]);
            l = l_mac(l, self.x1, B[1]);
            l = l_mac(l, x2, B[2]);
            l = l_shl(l, 2);
            *v = round(l_shl(l, 1));
            self.y2_hi = self.y1_hi;
            self.y2_lo = self.y1_lo;
            (self.y1_hi, self.y1_lo) = l_extract(l);
        }
    }
}
