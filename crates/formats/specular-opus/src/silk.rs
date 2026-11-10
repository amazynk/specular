//! The SILK decoder (RFC 6716 section 4.2). Parameter decoding is bit-exact. The
//! reconstruction and stereo unmixing run in fixed point, matched sample for sample to the
//! reference decoder's output at the internal rate (`silk_study.rs`; the RFC describes them in
//! floating point and leaves them non-normative). Lost packets are concealed, or rebuilt
//! from the next packet's LBRR (in-band FEC) frames.

use crate::range::RangeDecoder;
use crate::resample::Resampler;
use crate::rfc_tables::*;
use crate::silk_lpc::{nlsf_decode, nlsf2a};

const TYPE_VOICED: usize = 2;
const MAX_FRAME: usize = 320;
const LTP_SCALES_Q14: [i32; 3] = [15565, 12288, 8192];
/// Quantization offsets in Q23: [signal type >> 1][offset type].
const QUANT_OFFSET_Q23: [[i32; 2]; 2] = [[25, 60], [8, 25]];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Coding {
    Independent,
    IndependentNoLtpScaling,
    Conditional,
}

/// Decoded side information of one SILK frame.
#[derive(Default, Clone)]
struct Indices {
    signal_type: usize,
    offset_type: usize,
    gains: [i32; 4],
    nlsf: [i32; 17],
    interp: i32,
    lag_index: i32,
    contour: usize,
    per_index: usize,
    ltp_index: [usize; 4],
    ltp_scale: usize,
    seed: u32,
}

/// One SILK channel (mid or side).
struct Channel {
    fs_khz: usize,
    nb_subfr: usize,
    frame_length: usize,
    subfr_length: usize,
    lpc_order: usize,
    first_frame_after_reset: bool,
    last_gain_index: i32,
    ec_prev_signal_type: usize,
    ec_prev_lag_index: i32,
    prev_nlsf: [i32; 16],
    vad: [bool; 3],
    lbrr_flag: bool,
    lbrr: [bool; 3],
    /// Last 20 ms of clamped output, then the current frame.
    out16: Vec<i16>,
    /// Last 16 LPC synthesis states (Q14, excitation scale).
    slpc_q14: [i32; 16],
    /// The gain those states are scaled for.
    prev_gain_q16: i32,
    resampler: Resampler,
    plc: Plc,
}

impl Channel {
    fn new() -> Channel {
        Channel {
            fs_khz: 0,
            nb_subfr: 4,
            frame_length: 0,
            subfr_length: 0,
            lpc_order: 10,
            first_frame_after_reset: true,
            last_gain_index: 0,
            ec_prev_signal_type: 0,
            ec_prev_lag_index: 0,
            prev_nlsf: [0; 16],
            vad: [false; 3],
            lbrr_flag: false,
            lbrr: [false; 3],
            out16: vec![0; 2 * MAX_FRAME],
            slpc_q14: [0; 16],
            prev_gain_q16: 65536,
            resampler: Resampler::new(16),
            plc: Plc::default(),
        }
    }

    fn ltp_mem(&self) -> usize {
        20 * self.fs_khz
    }

    fn set_fs(&mut self, fs_khz: usize, nb_subfr: usize) {
        self.nb_subfr = nb_subfr;
        self.subfr_length = 5 * fs_khz;
        let frame_length = nb_subfr * self.subfr_length;
        if self.fs_khz != fs_khz {
            self.resampler = Resampler::new(fs_khz);
            self.lpc_order = if fs_khz == 16 { 16 } else { 10 };
            self.first_frame_after_reset = true;
            self.last_gain_index = 10;
            self.out16.fill(0);
            self.slpc_q14 = [0; 16];
            self.prev_gain_q16 = 65536;
            self.plc = Plc::default();
            self.fs_khz = fs_khz;
        }
        self.frame_length = frame_length;
    }

    /// Clears the prediction state (an uncoded side frame was followed by a coded one).
    fn clear_history(&mut self) {
        self.out16.fill(0);
        self.slpc_q14 = [0; 16];
        self.prev_gain_q16 = 65536;
        self.last_gain_index = 10;
        self.first_frame_after_reset = true;
    }

    fn decode_indices(
        &mut self,
        ec: &mut RangeDecoder,
        frame: usize,
        lbrr: bool,
        coding: Coding,
    ) -> Indices {
        let mut ix = Indices::default();
        let t = if lbrr || self.vad[frame] {
            ec.icdf(&FRAME_TYPE_ACTIVE, 8) + 2
        } else {
            ec.icdf(&FRAME_TYPE_INACTIVE, 8)
        };
        ix.signal_type = t >> 1;
        ix.offset_type = t & 1;
        if coding == Coding::Conditional {
            ix.gains[0] = ec.icdf(&GAIN_DELTA, 8) as i32;
        } else {
            ix.gains[0] = (ec.icdf(&GAIN_MSB[ix.signal_type], 8) as i32) << 3;
            ix.gains[0] += ec.icdf(&GAIN_LSB, 8) as i32;
        }
        for i in 1..self.nb_subfr {
            ix.gains[i] = ec.icdf(&GAIN_DELTA, 8) as i32;
        }
        let wb = self.fs_khz == 16;
        let s1 = usize::from(wb) * 2 + (ix.signal_type >> 1);
        let i1 = ec.icdf(&LSF_STAGE1[s1], 8);
        ix.nlsf[0] = i1 as i32;
        for k in 0..self.lpc_order {
            let icdf: &[u8] = if wb {
                &LSF_STAGE2_WB[usize::from(LSF_SEL_WB[i1][k])]
            } else {
                &LSF_STAGE2_NBMB[usize::from(LSF_SEL_NBMB[i1][k])]
            };
            let mut v = ec.icdf(icdf, 8) as i32;
            if v == 0 {
                v -= ec.icdf(&LSF_EXT, 8) as i32;
            } else if v == 8 {
                v += ec.icdf(&LSF_EXT, 8) as i32;
            }
            ix.nlsf[k + 1] = v - 4;
        }
        ix.interp = if self.nb_subfr == 4 {
            ec.icdf(&LSF_INTERP, 8) as i32
        } else {
            4
        };
        if ix.signal_type == TYPE_VOICED {
            let mut absolute = true;
            if coding == Coding::Conditional && self.ec_prev_signal_type == TYPE_VOICED {
                let d = ec.icdf(&PITCH_DELTA, 8) as i32;
                if d > 0 {
                    ix.lag_index = self.ec_prev_lag_index + d - 9;
                    absolute = false;
                }
            }
            if absolute {
                let low: &[u8] = match self.fs_khz {
                    8 => &PITCH_LOW_NB,
                    12 => &PITCH_LOW_MB,
                    _ => &PITCH_LOW_WB,
                };
                ix.lag_index = ec.icdf(&PITCH_HIGH, 8) as i32 * (self.fs_khz as i32 / 2);
                ix.lag_index += ec.icdf(low, 8) as i32;
            }
            self.ec_prev_lag_index = ix.lag_index;
            let contour: &[u8] = match (self.fs_khz == 8, self.nb_subfr == 4) {
                (true, false) => &PITCH_CONTOUR_NB10,
                (true, true) => &PITCH_CONTOUR_NB20,
                (false, false) => &PITCH_CONTOUR_WB10,
                (false, true) => &PITCH_CONTOUR_WB20,
            };
            ix.contour = ec.icdf(contour, 8);
            ix.per_index = ec.icdf(&LTP_PERIODICITY, 8);
            for k in 0..self.nb_subfr {
                ix.ltp_index[k] = ec.icdf(LTP_FILTER[ix.per_index], 8);
            }
            ix.ltp_scale = if coding == Coding::Independent {
                ec.icdf(&LTP_SCALE, 8)
            } else {
                0
            };
        }
        self.ec_prev_signal_type = ix.signal_type;
        ix.seed = ec.icdf(&LCG_SEED, 8) as u32;
        ix
    }
}

/// Pulses of one frame (silk_decode_pulses): fills `pulses` (rounded up to 16).
fn decode_pulses(
    ec: &mut RangeDecoder,
    pulses: &mut [i32],
    signal_type: usize,
    offset_type: usize,
    frame_length: usize,
) {
    let rate = ec.icdf(&RATE_LEVEL[signal_type >> 1], 8);
    let blocks = frame_length.div_ceil(16);
    let mut sum = [0usize; 20];
    let mut lsbs = [0usize; 20];
    for i in 0..blocks {
        let mut s = ec.icdf(&PULSE_COUNT[rate], 8);
        while s == 17 {
            lsbs[i] += 1;
            s = ec.icdf(&PULSE_COUNT[if lsbs[i] == 10 { 10 } else { 9 }], 8);
        }
        sum[i] = s;
    }
    for i in 0..blocks {
        let p = &mut pulses[i * 16..i * 16 + 16];
        p.fill(0);
        if sum[i] > 0 {
            shell_decode(ec, p, sum[i]);
        }
    }
    for i in 0..blocks {
        if lsbs[i] > 0 {
            for v in pulses[i * 16..i * 16 + 16].iter_mut() {
                let mut a = *v;
                for _ in 0..lsbs[i] {
                    a = (a << 1) + ec.icdf(&EXC_LSB, 8) as i32;
                }
                *v = a;
            }
        }
    }
    let row = &SIGN[signal_type * 2 + offset_type];
    for i in 0..blocks {
        if sum[i] > 0 || lsbs[i] > 0 {
            let icdf = [row[sum[i].min(6)], 0];
            for v in pulses[i * 16..i * 16 + 16].iter_mut() {
                if *v > 0 && ec.icdf(&icdf, 8) == 0 {
                    *v = -*v;
                }
            }
        }
    }
}

fn shell_split(ec: &mut RangeDecoder, table: &[&[u8]; 16], p: usize) -> (usize, usize) {
    if p == 0 {
        return (0, 0);
    }
    let l = ec.icdf(table[p - 1], 8);
    (l, p - l)
}

fn shell_decode(ec: &mut RangeDecoder, out: &mut [i32], total: usize) {
    fn rec(ec: &mut RangeDecoder, out: &mut [i32], p: usize) {
        let n = out.len();
        if n == 1 {
            out[0] = p as i32;
            return;
        }
        let table = match n {
            16 => &SHELL16,
            8 => &SHELL8,
            4 => &SHELL4,
            _ => &SHELL2,
        };
        let (l, r) = shell_split(ec, table, p);
        let (a, b) = out.split_at_mut(n / 2);
        if l > 0 {
            rec(ec, a, l);
        }
        if r > 0 {
            rec(ec, b, r);
        }
    }
    rec(ec, out, total);
}

#[inline]
fn log2lin(in_log_q7: i32) -> i32 {
    if in_log_q7 < 0 {
        return 0;
    }
    if in_log_q7 >= 3967 {
        return i32::MAX;
    }
    let i = in_log_q7 >> 7;
    let f = in_log_q7 & 127;
    let out = 1i32 << i;
    if in_log_q7 < 2048 {
        out + ((out * (((-174 * f * (128 - f)) >> 16) + f)) >> 7)
    } else {
        out + (out >> 7) * (((-174 * f * (128 - f)) >> 16) + f)
    }
}

/// The stereo unmixing state.
struct Stereo {
    pred_prev: [i32; 2],
    mid: [f32; 2],
    side: f32,
}

/// The SILK decoder of one Opus stream.
pub(crate) struct SilkDecoder {
    ch: [Channel; 2],
    stereo: Stereo,
    prev_decode_only_middle: bool,
    n_internal: usize,
    /// Output channels (1 or 2).
    n_api: usize,
    buf: [Vec<f32>; 2],
    pulses: Vec<i32>,
    /// Study aid: every frame's output at the internal rate, before upsampling.
    #[cfg(test)]
    pub(crate) tap: Option<Vec<TapFrame>>,
    #[cfg(test)]
    trace_voiced: bool,
}

/// One frame of SILK output at the internal rate (tests only).
#[cfg(test)]
pub(crate) struct TapFrame {
    pub(crate) fs_khz: usize,
    pub(crate) voiced: bool,
    pub(crate) lost: bool,
    /// `n_api` channels, interleaved.
    pub(crate) pcm: Vec<f32>,
}

impl SilkDecoder {
    pub(crate) fn new(n_api: usize) -> SilkDecoder {
        SilkDecoder {
            ch: [Channel::new(), Channel::new()],
            stereo: Stereo {
                pred_prev: [0; 2],
                mid: [0.0; 2],
                side: 0.0,
            },
            prev_decode_only_middle: false,
            n_internal: 1,
            n_api,
            buf: [vec![0.0; MAX_FRAME + 2], vec![0.0; MAX_FRAME + 2]],
            pulses: vec![0; MAX_FRAME],
            #[cfg(test)]
            tap: None,
            #[cfg(test)]
            trace_voiced: false,
        }
    }

    /// silk_InitDecoder (also clears the stereo state, RFC 8251 section 3).
    pub(crate) fn reset(&mut self) {
        let n_api = self.n_api;
        #[cfg(test)]
        let tap = self.tap.take();
        *self = SilkDecoder::new(n_api);
        #[cfg(test)]
        {
            self.tap = tap;
        }
    }

    /// Decodes one Opus frame's SILK layer (10 to 60 ms) at `fs_khz` with `channels` coded
    /// channels; appends 48 kHz interleaved samples (`n_api` channels) to `out`. With `fec`,
    /// decodes its LBRR (in-band FEC) frames instead, standing in for the lost frame before.
    pub(crate) fn decode(
        &mut self,
        ec: &mut RangeDecoder,
        fs_khz: usize,
        channels: usize,
        ms: usize,
        fec: bool,
        out: &mut Vec<f32>,
    ) {
        let (frames, nb_subfr) = match ms {
            10 => (1, 2),
            20 => (1, 4),
            40 => (2, 4),
            _ => (3, 4),
        };
        if channels > self.n_internal {
            self.ch[1] = Channel::new();
        }
        let stereo_to_mono = channels == 1 && self.n_internal == 2 && fs_khz == self.ch[0].fs_khz;
        for c in 0..channels {
            self.ch[c].set_fs(fs_khz, nb_subfr);
        }
        if self.n_api == 2 && channels == 2 && self.n_internal == 1 {
            self.stereo.pred_prev = [0; 2];
            self.stereo.side = 0.0;
            self.ch[1].resampler = self.ch[0].resampler.clone();
        }
        self.n_internal = channels;

        // Header: VAD and LBRR flags.
        for c in 0..channels {
            for i in 0..frames {
                self.ch[c].vad[i] = ec.bit_logp(1);
            }
            self.ch[c].lbrr_flag = ec.bit_logp(1);
        }
        for c in 0..channels {
            self.ch[c].lbrr = [false; 3];
            if self.ch[c].lbrr_flag {
                if frames == 1 {
                    self.ch[c].lbrr[0] = true;
                } else {
                    let sym = if frames == 2 {
                        ec.icdf(&LBRR_FLAGS_40MS, 8)
                    } else {
                        ec.icdf(&LBRR_FLAGS_60MS, 8)
                    } + 1;
                    for i in 0..frames {
                        self.ch[c].lbrr[i] = (sym >> i) & 1 != 0;
                    }
                }
            }
        }
        // The LBRR frames: skipped, or with `fec` decoded in place of the regular ones (the
        // packet before this one was lost); a frame without them is concealed.
        for i in 0..frames {
            let mut pred = self.stereo.pred_prev;
            let mut mid_only = false;
            let mut coded = [false; 2];
            for c in 0..channels {
                if self.ch[c].lbrr[i] {
                    if channels == 2 && c == 0 {
                        pred = decode_stereo_pred(ec);
                        if !self.ch[1].lbrr[i] {
                            mid_only = ec.icdf(&MID_ONLY, 8) == 1;
                        }
                    }
                    let coding = if i > 0 && self.ch[c].lbrr[i - 1] {
                        Coding::Conditional
                    } else {
                        Coding::Independent
                    };
                    let ix = self.ch[c].decode_indices(ec, i, true, coding);
                    let fl = self.ch[c].frame_length;
                    decode_pulses(ec, &mut self.pulses, ix.signal_type, ix.offset_type, fl);
                    if fec && (c == 0 || coded[0]) {
                        if c == 1 && self.prev_decode_only_middle {
                            self.ch[1].clear_history();
                        }
                        #[cfg(test)]
                        if c == 0 {
                            self.trace_voiced = ix.signal_type == TYPE_VOICED;
                        }
                        let pulses = std::mem::take(&mut self.pulses);
                        let mut frame = std::mem::take(&mut self.buf[c]);
                        self.ch[c].synthesize(&ix, &pulses, coding, &mut frame[2..2 + fl]);
                        self.buf[c] = frame;
                        self.pulses = pulses;
                        coded[c] = true;
                    }
                }
            }
            if !fec {
                continue;
            }
            let fl = self.ch[0].frame_length;
            if !coded[0] {
                for c in 0..channels {
                    let mut frame = std::mem::take(&mut self.buf[c]);
                    self.ch[c].conceal(&mut frame[2..2 + fl]);
                    self.buf[c] = frame;
                }
                #[cfg(test)]
                {
                    self.trace_voiced = self.ch[0].plc.voiced;
                }
            } else if channels == 2 && !coded[1] {
                self.buf[1][2..2 + fl].fill(0.0);
            }
            self.emit(pred, channels, fl, stereo_to_mono, !coded[0], out);
            self.prev_decode_only_middle = channels == 2 && (mid_only || !coded[1]);
        }
        if fec {
            return;
        }

        for i in 0..frames {
            let mut pred = [0i32; 2];
            let mut decode_only_middle = false;
            if channels == 2 {
                pred = decode_stereo_pred(ec);
                if !self.ch[1].vad[i] {
                    decode_only_middle = ec.icdf(&MID_ONLY, 8) == 1;
                }
            }
            if channels == 2 && !decode_only_middle && self.prev_decode_only_middle {
                self.ch[1].clear_history();
            }
            let fl = self.ch[0].frame_length;
            #[cfg(test)]
            let mut voiced0 = false;
            for c in 0..channels {
                if c == 0 || !decode_only_middle {
                    let coding = if i == 0 {
                        Coding::Independent
                    } else if c > 0 && self.prev_decode_only_middle {
                        Coding::IndependentNoLtpScaling
                    } else {
                        Coding::Conditional
                    };
                    let ix = self.ch[c].decode_indices(ec, i, false, coding);
                    #[cfg(test)]
                    if c == 0 {
                        voiced0 = ix.signal_type == TYPE_VOICED;
                    }
                    let flc = self.ch[c].frame_length;
                    decode_pulses(ec, &mut self.pulses, ix.signal_type, ix.offset_type, flc);
                    let pulses = std::mem::take(&mut self.pulses);
                    let mut frame = std::mem::take(&mut self.buf[c]);
                    self.ch[c].synthesize(&ix, &pulses, coding, &mut frame[2..2 + flc]);
                    self.buf[c] = frame;
                    self.pulses = pulses;
                } else {
                    self.buf[c][2..2 + fl].fill(0.0);
                }
            }
            #[cfg(test)]
            {
                self.trace_voiced = voiced0;
            }
            self.emit(pred, channels, fl, stereo_to_mono, false, out);
            self.prev_decode_only_middle = decode_only_middle;
        }
    }

    /// Unmixes (or just delays mono by one sample) the frame in `buf` and appends it to `out`
    /// at 48 kHz.
    fn emit(
        &mut self,
        pred: [i32; 2],
        channels: usize,
        fl: usize,
        stereo_to_mono: bool,
        lost: bool,
        out: &mut Vec<f32>,
    ) {
        #[cfg(not(test))]
        let _ = lost;
        // Unmix (or just delay mono by one sample) and resample.
        let start = out.len();
        let nout = fl * 48 / self.ch[0].fs_khz;
        out.resize(start + nout * self.n_api, 0.0);
        let o = &mut out[start..];
        if self.n_api == 2 && channels == 2 {
            self.ms_to_lr(pred, fl);
            let mut tmp = vec![0f32; nout];
            for c in 0..2 {
                self.ch[c]
                    .resampler
                    .process(&self.buf[c][1..1 + fl], &mut tmp);
                for (k, v) in tmp.iter().enumerate() {
                    o[2 * k + c] = *v;
                }
            }
        } else {
            let b = &mut self.buf[0];
            b[0] = self.stereo.mid[0];
            b[1] = self.stereo.mid[1];
            self.stereo.mid = [b[fl], b[fl + 1]];
            let mut tmp = vec![0f32; nout];
            self.ch[0]
                .resampler
                .process(&self.buf[0][1..1 + fl], &mut tmp);
            if self.n_api == 2 {
                let mut tmp2 = tmp.clone();
                if stereo_to_mono {
                    self.ch[1]
                        .resampler
                        .process(&self.buf[0][1..1 + fl], &mut tmp2);
                }
                for k in 0..nout {
                    o[2 * k] = tmp[k];
                    o[2 * k + 1] = tmp2[k];
                }
            } else {
                o[..nout].copy_from_slice(&tmp);
            }
        }
        #[cfg(test)]
        if let Some(tap) = self.tap.as_mut() {
            let src = if self.n_api == 2 && channels == 2 {
                2
            } else {
                1
            };
            let mut pcm = vec![0f32; fl * self.n_api];
            for k in 0..fl {
                for c in 0..self.n_api {
                    pcm[k * self.n_api + c] = self.buf[c.min(src - 1)][1 + k];
                }
            }
            tap.push(TapFrame {
                fs_khz: self.ch[0].fs_khz,
                voiced: self.trace_voiced,
                lost,
                pcm,
            });
        }
    }

    /// Conceals a lost packet of `ms` milliseconds: continues the last frame's pitch or
    /// noise through its LPC filter, fading (RFC 6716 leaves concealment open, section 4.4);
    /// appends 48 kHz samples to `out`.
    pub(crate) fn conceal(&mut self, ms: usize, out: &mut Vec<f32>) {
        let fs = self.ch[0].fs_khz;
        if fs == 0 {
            out.resize(out.len() + ms * 48 * self.n_api, 0.0);
            return;
        }
        let (frames, nb_subfr) = match ms {
            10 => (1, 2),
            20 => (1, 4),
            40 => (2, 4),
            _ => (3, 4),
        };
        let channels = self.n_internal;
        for _ in 0..frames {
            for c in 0..channels {
                self.ch[c].set_fs(fs, nb_subfr);
            }
            let fl = self.ch[0].frame_length;
            for c in 0..channels {
                let mut frame = std::mem::take(&mut self.buf[c]);
                self.ch[c].conceal(&mut frame[2..2 + fl]);
                self.buf[c] = frame;
            }
            #[cfg(test)]
            {
                self.trace_voiced = self.ch[0].plc.voiced;
            }
            self.emit(self.stereo.pred_prev, channels, fl, false, true, out);
        }
    }

    fn ms_to_lr(&mut self, pred: [i32; 2], fl: usize) {
        let fs = self.ch[0].fs_khz;
        let (m, s) = self.buf.split_at_mut(1);
        let x1 = &mut m[0];
        let x2 = &mut s[0];
        x1[0] = self.stereo.mid[0];
        x1[1] = self.stereo.mid[1];
        x2[1] = self.stereo.side;
        self.stereo.mid = [x1[fl], x1[fl + 1]];
        self.stereo.side = x2[fl + 1];
        // In fixed point, on the 16-bit scale (measured against the reference decoder).
        let n1 = 8 * fs;
        let q = |v: f32| (v * 32768.0).round() as i32;
        let denom_q16 = (1i32 << 16) / n1 as i32;
        let step = |p: i32, prev: i32| {
            rshift_round64(
                i64::from((p - prev) as i16) * i64::from(denom_q16 as i16),
                16,
            )
        };
        let d0 = step(pred[0], self.stereo.pred_prev[0]);
        let d1 = step(pred[1], self.stereo.pred_prev[1]);
        let (mut w0, mut w1) = (self.stereo.pred_prev[0], self.stereo.pred_prev[1]);
        for n in 0..fl {
            if n < n1 {
                w0 += d0;
                w1 += d1;
            } else {
                w0 = pred[0];
                w1 = pred[1];
            }
            let (m0, m1, m2) = (q(x1[n]), q(x1[n + 1]), q(x1[n + 2]));
            let sum_q11 = (m0 + m2 + 2 * m1) << 9;
            let mut side_q8 = (q(x2[n + 1]) << 8).wrapping_add(smulwb(sum_q11, w0));
            side_q8 = side_q8.wrapping_add(smulwb(m1 << 11, w1));
            x2[n + 1] = f32::from(sat16(rshift_round64(i64::from(side_q8), 8))) / 32768.0;
        }
        // Then left and right (a second pass: the low-pass above needs the mid samples).
        for n in 1..=fl {
            let (m, s) = (q(x1[n]), q(x2[n]));
            x1[n] = f32::from(sat16(m + s)) / 32768.0;
            x2[n] = f32::from(sat16(m - s)) / 32768.0;
        }
        self.stereo.pred_prev = pred;
    }
}

fn decode_stereo_pred(ec: &mut RangeDecoder) -> [i32; 2] {
    let n = ec.icdf(&STEREO_STAGE1, 8);
    let mut ix = [[0usize; 3]; 2];
    ix[0][2] = n / 5;
    ix[1][2] = n % 5;
    for row in ix.iter_mut() {
        row[0] = ec.icdf(&STEREO_STAGE2, 8);
        row[1] = ec.icdf(&STEREO_STAGE3, 8);
    }
    let mut pred = [0i32; 2];
    for k in 0..2 {
        let i0 = ix[k][0] + 3 * ix[k][2];
        let low = STEREO_WEIGHTS_Q13[i0];
        let step = ((STEREO_WEIGHTS_Q13[i0 + 1] - low) * 6554) >> 16;
        pred[k] = low + step * (2 * ix[k][1] as i32 + 1);
    }
    pred[0] -= pred[1];
    pred
}

impl Channel {
    /// Conceals one lost frame into `out` from the last frame's parameters: voiced frames
    /// repeat their pitch through a weakened LTP filter, others get noise of the last
    /// excitation's level; both fade out over about 100 ms.
    fn conceal(&mut self, out: &mut [f32]) {
        let fl = self.frame_length;
        let fs = self.fs_khz as i32;
        let mut plc = self.plc;
        plc.lost += 1;
        // Per lost frame: the level kept (1, 0.8, 0.6, ... to 0 after five frames).
        let fade = (1.0 - 0.2 * (plc.lost - 1) as f32).max(0.0);
        let voiced = plc.voiced && plc.lag >= 2 * fs;
        let mut b_q7 = [0i32; 5];
        if voiced {
            // The LTP gain capped below one so the repetition decays on its own too.
            let sum: i32 = plc.b_q7.iter().sum();
            let cap = if sum > 0 {
                (0.9 * 128.0 / sum as f32).min(1.0)
            } else {
                1.0
            };
            for k in 0..5 {
                b_q7[k] = (plc.b_q7[k] as f32 * cap * fade).round() as i32;
            }
        }
        let mut exc_q14 = [0i32; MAX_FRAME];
        // Unvoiced levels by place in the run, matched on average to the reference decoder's
        // concealment (silk_study.rs).
        const UNVOICED: [f32; 6] = [0.75, 0.45, 0.4, 0.2, 0.1, 0.1];
        let level = plc.exc_rms_q14 as f32
            * if voiced {
                0.1 * fade
            } else {
                UNVOICED[(plc.lost as usize - 1).min(5)]
            };
        for v in exc_q14[..fl].iter_mut() {
            plc.seed = plc.seed.wrapping_mul(196_314_165).wrapping_add(907_633_515);
            // Uniform in [-sqrt(3), sqrt(3)) times the level: the same RMS.
            let u = (plc.seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
            *v = (u * 1.732 * level) as i32;
        }
        let lag = plc.lag.clamp(2 * fs, 18 * fs);
        let f = FrameParams {
            a: [plc.a; 2],
            gains: [plc.gain_q16.max(1); 4],
            interp: 4,
            voiced,
            lags: [lag; 4],
            b_q7: [b_q7; 4],
            ltp_scale: 16384,
        };
        self.reconstruct(&f, &exc_q14[..fl], out);
        self.plc = plc;
    }

    /// Decodes the parameters and reconstructs one frame into `out` (RFC 4.2.7.9).
    fn synthesize(&mut self, ix: &Indices, pulses: &[i32], coding: Coding, out: &mut [f32]) {
        let n = self.subfr_length;
        let order = self.lpc_order;
        let fl = self.frame_length;
        let fs = self.fs_khz as i32;
        // Gains.
        let mut gain_q16 = [0i32; 4];
        for k in 0..self.nb_subfr {
            if k == 0 && coding != Coding::Conditional {
                self.last_gain_index = ix.gains[0].max(self.last_gain_index - 16);
            } else {
                let d = ix.gains[k] - 4;
                let thr = 8 + self.last_gain_index;
                if d > thr {
                    self.last_gain_index += 2 * d - thr;
                } else {
                    self.last_gain_index += d;
                }
            }
            self.last_gain_index = self.last_gain_index.clamp(0, 63);
            gain_q16[k] = log2lin(
                (((0x1D1C71i64 * i64::from(self.last_gain_index)) >> 16) as i32 + 2090).min(3967),
            );
        }
        // LPC.
        let mut nlsf = [0i32; 16];
        nlsf_decode(
            order == 16,
            ix.nlsf[0] as usize,
            &ix.nlsf[1..=order],
            &mut nlsf,
        );
        let mut a = [[0i32; 16]; 2];
        nlsf2a(&nlsf[..order], &mut a[1]);
        let mut interp = ix.interp;
        if self.first_frame_after_reset {
            interp = 4;
        }
        if interp < 4 {
            let mut n0 = [0i32; 16];
            for k in 0..order {
                n0[k] = self.prev_nlsf[k] + ((interp * (nlsf[k] - self.prev_nlsf[k])) >> 2);
            }
            nlsf2a(&n0[..order], &mut a[0]);
        } else {
            a[0] = a[1];
        }
        self.prev_nlsf[..order].copy_from_slice(&nlsf[..order]);
        // Pitch and LTP.
        let voiced = ix.signal_type == TYPE_VOICED;
        let mut lags = [0i32; 4];
        let mut b_q7 = [[0i32; 5]; 4];
        let mut ltp_scale = 0;
        if voiced {
            let (min_lag, max_lag) = (2 * fs, 18 * fs);
            let lag = min_lag + ix.lag_index;
            for k in 0..self.nb_subfr {
                let off = match (fs == 8, self.nb_subfr == 4) {
                    (true, false) => PITCH_CB_NB10[ix.contour][k],
                    (true, true) => PITCH_CB_NB20[ix.contour][k],
                    (false, false) => PITCH_CB_WB10[ix.contour][k],
                    (false, true) => PITCH_CB_WB20[ix.contour][k],
                };
                lags[k] = (lag + i32::from(off)).clamp(min_lag, max_lag);
                let row: &[i8; 5] = match ix.per_index {
                    0 => &LTP_CB0[ix.ltp_index[k]],
                    1 => &LTP_CB1[ix.ltp_index[k]],
                    _ => &LTP_CB2[ix.ltp_index[k]],
                };
                for t in 0..5 {
                    b_q7[k][t] = i32::from(row[t]);
                }
            }
            ltp_scale = LTP_SCALES_Q14[ix.ltp_scale];
        }
        // Excitation, in Q14 of the 16-bit output scale (e_Q23 << 6).
        let off_q23 = QUANT_OFFSET_Q23[ix.signal_type >> 1][ix.offset_type];
        let mut exc_q14 = [0i32; MAX_FRAME];
        let mut seed = ix.seed;
        for i in 0..fl {
            let e = pulses[i];
            let mut v = (e << 8) - e.signum() * 20 + off_q23;
            seed = seed.wrapping_mul(196_314_165).wrapping_add(907_633_515);
            if seed & 0x8000_0000 != 0 {
                v = -v;
            }
            seed = seed.wrapping_add(e as u32);
            exc_q14[i] = v << 6;
        }
        let f = FrameParams {
            a,
            gains: gain_q16,
            interp,
            voiced,
            lags,
            b_q7,
            ltp_scale,
        };
        // What concealment continues from if the next packet is lost.
        let last = self.nb_subfr - 1;
        self.plc = Plc {
            a: a[1],
            gain_q16: gain_q16[last],
            voiced,
            lag: lags[last],
            b_q7: b_q7[last],
            exc_rms_q14: rms(&exc_q14[fl - n..fl]),
            lost: 0,
            seed: self.plc.seed,
        };
        self.reconstruct(&f, &exc_q14[..fl], out);
    }

    /// Runs the LTP and LPC synthesis of one frame (RFC 6716 4.2.7.9) from its parameters
    /// and excitation (Q14 of the 16-bit output scale) into `out`.
    fn reconstruct(&mut self, f: &FrameParams, exc_q14: &[i32], out: &mut [f32]) {
        let n = self.subfr_length;
        let order = self.lpc_order;
        let fl = self.frame_length;
        let FrameParams {
            a,
            gains: gain_q16,
            interp,
            voiced,
            lags,
            b_q7,
            ltp_scale,
        } = *f;
        // Reconstruction in fixed point (measured against the reference decoder's output;
        // RFC 6716 4.2.7.9 gives the floating-point equivalent). The LTP and LPC filters run
        // on the excitation scale (output / gain); their histories are rescaled when the
        // gain changes. `out16` = [ltp_mem history | current frame].
        let mem = self.ltp_mem();
        let mut sltp_q15 = [0i32; 2 * MAX_FRAME];
        let mut sltp16 = [0i16; 2 * MAX_FRAME];
        let mut slpc = [0i32; 16 + MAX_FRAME];
        slpc[..16].copy_from_slice(&self.slpc_q14);
        let mut pres_q14 = [0i32; MAX_FRAME];
        for s in 0..self.nb_subfr {
            let j = s * n;
            let ak = &a[s >> 1];
            let gain = gain_q16[s];
            let gain_q10 = gain >> 6;
            let mut inv_gain_q31 = inverse_varq(gain, 47);
            if gain != self.prev_gain_q16 {
                let adj_q16 = div_varq(self.prev_gain_q16, gain, 16);
                for v in slpc[..16 + j].iter_mut().skip(j) {
                    *v = smulww(adj_q16, *v);
                }
                if voiced && !(s == 0 || (s == 2 && interp < 4)) {
                    let lag = lags[s] as usize;
                    for v in sltp_q15[mem + j - lag - 2..mem + j].iter_mut() {
                        *v = smulww(adj_q16, *v);
                    }
                }
            }
            self.prev_gain_q16 = gain;
            if voiced {
                let lag = lags[s] as usize;
                if s == 0 || (s == 2 && interp < 4) {
                    // Rewhiten the clamped output with this subframe's LPC filter.
                    let start = mem + j - lag - order - 2;
                    for p in start + order..mem + j {
                        let mut acc = i64::from(self.out16[p]) << 12;
                        for k in 0..order {
                            acc -= i64::from(self.out16[p - k - 1]) * i64::from(ak[k]);
                        }
                        sltp16[p] = sat16(rshift_round64(acc, 12));
                    }
                    if s == 0 {
                        inv_gain_q31 = smulwb(inv_gain_q31, ltp_scale) << 2;
                    }
                    for p in mem + j - lag - 2..mem + j {
                        sltp_q15[p] = smulwb(inv_gain_q31, i32::from(sltp16[p]));
                    }
                }
                for i in j..j + n {
                    let p = mem + i;
                    let mut pred_q13 = 2i32;
                    for k in 0..5 {
                        pred_q13 += smulwb(sltp_q15[p + 2 - k - lag], b_q7[s][k] << 7);
                    }
                    pres_q14[i] = exc_q14[i] + (pred_q13 << 1);
                    sltp_q15[p] = pres_q14[i] << 1;
                }
            } else {
                pres_q14[j..j + n].copy_from_slice(&exc_q14[j..j + n]);
            }
            for i in j..j + n {
                let li = i + 16;
                let mut pred_q10 = (order >> 1) as i32;
                for k in 0..order {
                    pred_q10 += smulwb(slpc[li - k - 1], ak[k]);
                }
                slpc[li] = pres_q14[i].saturating_add(pred_q10.saturating_mul(16));
                let o = sat16(rshift_round64(i64::from(smulww(slpc[li], gain_q10)), 8));
                self.out16[mem + i] = o;
                out[i] = f32::from(o) / 32768.0;
            }
        }
        self.slpc_q14.copy_from_slice(&slpc[fl..fl + 16]);
        // Keep the last `mem` outputs as history.
        self.out16.copy_within(fl..fl + mem, 0);
        self.first_frame_after_reset = false;
    }
}

/// The parameters [`Channel::reconstruct`] needs for one frame.
#[derive(Clone, Copy)]
struct FrameParams {
    /// LPC coefficients (Q12) of the first and second half of the frame.
    a: [[i32; 16]; 2],
    gains: [i32; 4],
    interp: i32,
    voiced: bool,
    lags: [i32; 4],
    b_q7: [[i32; 5]; 4],
    ltp_scale: i32,
}

/// What packet loss concealment continues from: the last frame's final subframe.
#[derive(Clone, Copy, Default)]
struct Plc {
    a: [i32; 16],
    gain_q16: i32,
    voiced: bool,
    lag: i32,
    b_q7: [i32; 5],
    /// RMS of the last subframe's excitation (Q14).
    exc_rms_q14: i32,
    /// Frames concealed in a row.
    lost: u32,
    seed: u32,
}

fn rms(v: &[i32]) -> i32 {
    let e: f64 = v.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
    (e / v.len().max(1) as f64).sqrt() as i32
}

/// (a * b) >> 16 with a 32-bit `a` and the low 16 bits of `b`, signed.
#[inline]
fn smulwb(a: i32, b: i32) -> i32 {
    ((i64::from(a) * i64::from(b as i16)) >> 16) as i32
}

/// (a * b) >> 16 with both 32-bit.
#[inline]
fn smulww(a: i32, b: i32) -> i32 {
    ((i64::from(a) * i64::from(b)) >> 16) as i32
}

#[inline]
fn rshift_round64(a: i64, s: u32) -> i32 {
    (((a >> (s - 1)) + 1) >> 1) as i32
}

#[inline]
fn sat16(v: i32) -> i16 {
    v.clamp(-32768, 32767) as i16
}

/// Leading sign-bit headroom of `v` (bits it can be shifted left without overflow).
#[inline]
fn headroom(v: i32) -> u32 {
    v.unsigned_abs().leading_zeros().saturating_sub(1)
}

/// `v` shifted by `shift` (left when negative, saturating; right otherwise; 0 past 31).
#[inline]
fn shift_q(v: i32, shift: i32) -> i32 {
    if shift < 0 {
        let l = (-shift) as u32;
        let lim = i32::MAX >> l;
        v.clamp(-lim - 1, lim) << l
    } else if shift < 32 {
        v >> shift
    } else {
        0
    }
}

/// a / b in Q`q`, the way the reference decoder computes it: a 16-bit reciprocal of the
/// normalised divisor and one refinement step (matched to its output, not exact).
fn div_varq(a: i32, b: i32, q: i32) -> i32 {
    let a_hr = headroom(a);
    let a_nrm = a << a_hr;
    let b_hr = headroom(b);
    let b_nrm = b << b_hr;
    let b_inv = (i32::MAX >> 2) / (b_nrm >> 16);
    let mut r = smulwb(a_nrm, b_inv);
    let a2 = a_nrm.wrapping_sub((((i64::from(b_nrm) * i64::from(r)) >> 32) as i32).wrapping_shl(3));
    r = r.wrapping_add(smulwb(a2, b_inv));
    shift_q(r, 29 + a_hr as i32 - b_hr as i32 - q)
}

/// 1 / b in Q`q`, likewise approximated.
fn inverse_varq(b: i32, q: i32) -> i32 {
    let b_hr = headroom(b);
    let b_nrm = b << b_hr;
    let b_inv = (i32::MAX >> 2) / (b_nrm >> 16);
    let r = b_inv << 16;
    let err = ((1i32 << 29) - smulwb(b_nrm, b_inv)).wrapping_shl(3);
    let r = r.wrapping_add(smulww(err, b_inv));
    shift_q(r, 61 - b_hr as i32 - q)
}
