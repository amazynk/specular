//! Specular's own QCELP-13 speech decoder (Qualcomm PureVoice 13K, TIA/EIA IS-733,
//! 3GPP2 C.S0020 "High Rate Speech Service Option 17").
//!
//! 8 kHz mono, one 20 ms frame (160 samples) per packet, at Rate 1 (266 bits), Rate 1/2
//! (124), Rate 1/4 (54) or Rate 1/8 (20), plus blank and erasure packets. The decoder follows
//! the floating-point description of C.S0020-A §2.4.3.3, §2.4.5.2, §2.4.6.2 and §2.4.8:
//! LSP decoding with the Rate 1/8 predictor, interpolation and bandwidth expansion, the
//! pitch synthesis filter with half-sample lags, the pitch pre-filter, the formant
//! synthesis filter, the adaptive postfilter with its gain control, and the erasure
//! handling of §2.4.8.7 (gains and LSP predictors decayed, never plain silence). Damaged
//! packets (set reserved bits, all-ones Rate 1/8, all-zero packets, implausible LSPs or
//! gains, invalid lags) are treated as erasures, as the specification says.
//!
//! There is no bit-exact oracle at hand (the 3GPP2 reference C is not used), so the decoder
//! is checked against ffmpeg's floating-point `qcelp` decoder. Where the printed text is
//! ambiguous or its formulas were garbled, the decoder takes the reading that the decoders
//! in use agree on: the anti-tilt filter as 1 − 0.3 z^-1, the postfilter gain smoothed per
//! sample, the Rate 1/4 and 1/8 noise √2 below Eq. 2.4.8.1.2 and the Rate 1/8 gain
//! predictor's mean truncated. Real files then match ffmpeg to 22–29 dB SNR.
//!
//! The tables are data transcribed from the specification by `tools/corpus/qcelp-tables.py`
//! (`tables.rs`). The 1994 patents behind IS-733 have expired.
//!
//! Packets come in the storage form of RFC 3625 (QCP) and 3GPP2 C.S0050 (3G2 `sqcp`): a
//! rate octet followed by the frame's bits, most significant first. [`Decoder::decode`] also
//! takes a bare frame whose length gives its rate.
//!
//! QCELP-8 (IS-96A, the 8K PureVoice codec) has a different frame layout and is not
//! decoded; [`is_qcelp8`] lets a container name it.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

mod tables;

use std::f32::consts::PI;

use tables::*;

/// Samples per frame.
pub const FRAME_SAMPLES: usize = 160;
/// The output sample rate.
pub const SAMPLE_RATE: u32 = 8000;

/// The packet type (§2.3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    Blank,
    Eighth,
    Quarter,
    Half,
    Full,
    /// Insufficient frame quality.
    Erasure,
}

impl Rate {
    /// The rate of an RFC 3625 / C.S0050 rate octet.
    pub fn from_byte(b: u8) -> Option<Rate> {
        Some(match b {
            0 => Rate::Blank,
            1 => Rate::Eighth,
            2 => Rate::Quarter,
            3 => Rate::Half,
            4 => Rate::Full,
            14 => Rate::Erasure,
            _ => return None,
        })
    }

    /// The rate of a bare frame (no rate octet) by its length.
    pub fn from_len(n: usize) -> Option<Rate> {
        Some(match n {
            34 => Rate::Full,
            16 => Rate::Half,
            7 => Rate::Quarter,
            3 => Rate::Eighth,
            0 => Rate::Blank,
            _ => return None,
        })
    }

    /// Bytes of the frame after the rate octet.
    pub fn payload_bytes(self) -> usize {
        match self {
            Rate::Full => 34,
            Rate::Half => 16,
            Rate::Quarter => 7,
            Rate::Eighth => 3,
            Rate::Blank | Rate::Erasure => 0,
        }
    }
}

/// Bytes of a packet including its rate octet, from that octet.
pub fn packet_len(rate_byte: u8) -> Option<usize> {
    Rate::from_byte(rate_byte).map(|r| r.payload_bytes() + 1)
}

/// The two QCP codec GUIDs of QCELP-13K (RFC 3625 §4, `5E7F6D41`/`5E7F6D42`), in the
/// little-endian byte order of the file.
pub const QCP_GUIDS: [[u8; 16]; 2] = [
    guid_le(
        0x5E7F_6D41,
        0xB115,
        0x11D0,
        [0xBA, 0x91, 0x00, 0x80, 0x5F, 0xB4, 0xB9, 0x7E],
    ),
    guid_le(
        0x5E7F_6D42,
        0xB115,
        0x11D0,
        [0xBA, 0x91, 0x00, 0x80, 0x5F, 0xB4, 0xB9, 0x7E],
    ),
];

const fn guid_le(a: u32, b: u16, c: u16, d: [u8; 8]) -> [u8; 16] {
    let a = a.to_le_bytes();
    let b = b.to_le_bytes();
    let c = c.to_le_bytes();
    [
        a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d[0], d[1], d[2], d[3], d[4], d[5], d[6],
        d[7],
    ]
}

/// Whether a QCP codec name or header says QCELP-8 (IS-96A), which this crate does not
/// decode: its packets are 1+20 bytes at full rate.
pub fn is_qcelp8(full_rate_packet_bytes: usize) -> bool {
    full_rate_packet_bytes == 21 || full_rate_packet_bytes == 20
}

const ORDER: usize = 10;
/// Pitch memory kept between frames: the longest lag (143) plus the interpolation taps.
const HIST: usize = 152;
/// Bandwidth expansion factor (§2.4.3.3.6).
const BETA: f32 = 0.9883;
/// Postfilter zero and pole factors (§2.4.8.6).
const PF_S: f32 = 0.625;
const PF_P: f32 = 0.775;
/// Output scale: the codec works in 14-bit units (±8031 for the input, §2.4.2.1.1).
const OUT_SCALE: f32 = 1.0 / 8192.0;

/// Decoded parameters of one frame.
#[derive(Default)]
struct Fields {
    lspv: [u32; 6],
    lsp: [u32; 11],
    plag: [u32; 5],
    pfrac: [u32; 5],
    pgain: [u32; 5],
    cbindex: [u32; 17],
    cbgain: [u32; 17],
    cbsign: [u32; 17],
    cbseed: u32,
    reserved: u32,
}

fn unpack(rate: Rate, payload: &[u8]) -> Fields {
    let table: &[(u8, u8, u8)] = match rate {
        Rate::Full => &PACK_FULL,
        Rate::Half => &PACK_HALF,
        Rate::Quarter => &PACK_QUARTER,
        Rate::Eighth => &PACK_EIGHTH,
        _ => &[],
    };
    let mut f = Fields::default();
    for (i, &(field, n, pos)) in table.iter().enumerate() {
        let bit = payload
            .get(i / 8)
            .map_or(0, |b| u32::from((b >> (7 - i % 8)) & 1));
        let v = bit << pos;
        let n = usize::from(n);
        match field {
            0 => f.lspv[n] |= v,
            1 => f.lsp[n] |= v,
            2 => f.plag[n] |= v,
            3 => f.pfrac[n] |= v,
            4 => f.pgain[n] |= v,
            5 => f.cbindex[n] |= v,
            6 => f.cbgain[n] |= v,
            7 => f.cbsign[n] |= v,
            8 => f.cbseed |= v,
            _ => f.reserved |= bit,
        }
    }
    f
}

/// The first `n` frame bits as an integer, most significant first.
fn leading_bits(payload: &[u8], n: usize) -> u32 {
    (0..n).fold(0, |acc, i| {
        let bit = payload
            .get(i / 8)
            .map_or(0, |b| u32::from((b >> (7 - i % 8)) & 1));
        (acc << 1) | bit
    })
}

fn all_bits(payload: &[u8], rate: Rate, value: u8) -> bool {
    let table: &[(u8, u8, u8)] = match rate {
        Rate::Full => &PACK_FULL,
        Rate::Half => &PACK_HALF,
        Rate::Quarter => &PACK_QUARTER,
        Rate::Eighth => &PACK_EIGHTH,
        _ => return false,
    };
    table.iter().enumerate().all(|(i, &(field, _, _))| {
        field == 9 || payload.get(i / 8).map_or(0, |b| (b >> (7 - i % 8)) & 1) == value
    })
}

fn bias(i: usize) -> f32 {
    (i + 1) as f32 / (ORDER + 1) as f32
}

/// hammsinc(x) of Eq. 2.4.5.2-3.
fn hammsinc(x: f32) -> f32 {
    let px = PI * x;
    (px.sin() / px) * (0.5 + 0.46 * (px / 4.0).cos())
}

/// The 16-bit pseudorandom sequence of §2.4.8.1.2, scaled to ±1.887/√2.
fn random_vector(seed: u32, out: &mut [f32; FRAME_SAMPLES]) {
    let mut r = seed & 0xffff;
    for v in out.iter_mut() {
        r = (521 * r + 259) & 0xffff;
        let t = ((r + 0x8000) & 0xffff) as i32 - 0x8000;
        // Eq. 2.4.8.1.2's 1.887 · tmprnd / 32768, taken √2 lower: the noise level of the
        // decoders in use (it puts Rate 1/4 and 1/8 frames 3 dB from ffmpeg's otherwise).
        *v = 1.887 * t as f32 / 32768.0 / std::f32::consts::SQRT_2;
    }
}

/// LSP frequencies (0..1) to bandwidth-expanded direct-form coefficients â1..â10
/// (§2.4.3.3.5, §2.4.3.3.6): A(z) = 1 − Σ âi z^-i.
fn lsp_to_lpc(w: &[f32; ORDER]) -> [f32; ORDER] {
    // P_A(z) = (1 + z^-1) Π (1 − 2cos(π w_odd) z^-1 + z^-2), Q_A with (1 − z^-1) and w_even.
    let mut p = [0f32; ORDER + 2];
    let mut q = [0f32; ORDER + 2];
    p[0] = 1.0;
    q[0] = 1.0;
    let mut deg = 0;
    for j in 0..5 {
        let cp = -2.0 * (PI * w[2 * j]).cos();
        let cq = -2.0 * (PI * w[2 * j + 1]).cos();
        // multiply by (1 + c z^-1 + z^-2)
        for k in (0..=deg + 2).rev() {
            let a = |v: &[f32; ORDER + 2], c: f32| {
                v[k] + if k >= 1 { c * v[k - 1] } else { 0.0 } + if k >= 2 { v[k - 2] } else { 0.0 }
            };
            let np = a(&p, cp);
            let nq = a(&q, cq);
            p[k] = np;
            q[k] = nq;
        }
        deg += 2;
    }
    // times (1 + z^-1) and (1 − z^-1)
    for k in (1..=ORDER + 1).rev() {
        p[k] += p[k - 1];
        q[k] -= q[k - 1];
    }
    let mut a = [0f32; ORDER];
    let mut g = BETA;
    for i in 0..ORDER {
        a[i] = -(p[i + 1] + q[i + 1]) / 2.0 * g;
        g *= BETA;
    }
    a
}

/// The low-pass filtered LSPs of one frame and how the decoder should use them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LspUse {
    /// Interpolated per 40-sample subframe (Rates 1, 1/2, 1/4).
    Four,
    /// One set, 0.375 previous + 0.625 current (Rate 1/8).
    Eighth,
    /// The previous frame's set for the whole frame (erasure, blank).
    Previous,
}

/// A QCELP-13 decoder: one per stream, frames in order.
pub struct Decoder {
    /// ŵ(previous): the filtered LSPs of the last frame.
    prev_lsp: [f32; ORDER],
    /// The LSP predictor states (bias removed).
    pred: [f32; ORDER],
    /// Consecutive Rate 1/8 packets.
    eighth_count: u32,
    /// Consecutive erasures.
    erasures: u32,
    /// The rate of the last good frame.
    last_good: Rate,
    /// Pitch synthesis filter output history (oldest first).
    pitch_mem: [f32; HIST],
    /// Pitch pre-filter output history.
    pre_mem: [f32; HIST],
    /// Formant synthesis filter memory, yd(n−1) first.
    syn_mem: [f32; ORDER],
    /// Postfilter memories: the FIR input (yd), the IIR output and the anti-tilt output.
    pf_fir: [f32; ORDER],
    pf_iir: [f32; ORDER],
    pf_tilt: f32,
    /// SCALE of the postfilter gain control.
    scale: f32,
    /// G1 (dB) of the last codebook subframes, most recent first.
    g1_hist: [i32; 3],
    /// Ĝ'(previous): the last (filtered) linear codebook gain magnitude.
    g_prev: f32,
    /// The last pitch subframe's lag and gain.
    last_lag: f32,
    last_b: f32,
    /// The Rate 1/4 band-pass filter's input history (last 20 samples, newest last).
    bpf_mem: [f32; 20],
    /// The erasure codebook index generator.
    erase_seed: u32,
    /// Interpolation taps hammsinc(i + 0.5), i = −4..3.
    taps: [f32; 8],
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        let mut taps = [0f32; 8];
        for (k, t) in taps.iter_mut().enumerate() {
            *t = hammsinc(k as f32 - 4.0 + 0.5);
        }
        Decoder {
            prev_lsp: std::array::from_fn(bias),
            pred: [0.0; ORDER],
            eighth_count: 0,
            erasures: 0,
            last_good: Rate::Eighth,
            pitch_mem: [0.0; HIST],
            pre_mem: [0.0; HIST],
            syn_mem: [0.0; ORDER],
            pf_fir: [0.0; ORDER],
            pf_iir: [0.0; ORDER],
            pf_tilt: 0.0,
            scale: 1.0,
            g1_hist: [0; 3],
            g_prev: 0.0,
            last_lag: 0.0,
            last_b: 0.0,
            bpf_mem: [0.0; 20],
            erase_seed: 0,
            taps,
        }
    }

    /// Back to the initial state (§2.4.9), as after a seek.
    pub fn reset(&mut self) {
        *self = Decoder::new();
    }

    /// Decodes one packet (rate octet + frame, or a bare frame) and appends 160 samples in
    /// [−1, 1]. Unknown or truncated packets are decoded as erasures.
    pub fn decode(&mut self, packet: &[u8], out: &mut Vec<f32>) {
        let (rate, payload) = split_packet(packet);
        self.decode_frame(rate, payload, out);
    }

    /// Decodes one frame of a known rate (`payload` without the rate octet).
    pub fn decode_frame(&mut self, rate: Rate, payload: &[u8], out: &mut Vec<f32>) {
        let mut buf = [0f32; FRAME_SAMPLES];
        let rate = if payload.len() < rate.payload_bytes() {
            Rate::Erasure
        } else {
            rate
        };
        let ok = match rate {
            Rate::Full | Rate::Half | Rate::Quarter | Rate::Eighth => {
                self.good_frame(rate, payload, &mut buf)
            }
            Rate::Blank => {
                self.blank_frame(&mut buf);
                true
            }
            Rate::Erasure => false,
        };
        if !ok {
            self.erasure_frame(&mut buf);
        }
        out.extend(buf.iter().map(|&v| (v * OUT_SCALE).clamp(-1.0, 1.0)));
    }

    /// Rates 1 to 1/8; false when the packet must be erased.
    fn good_frame(&mut self, rate: Rate, payload: &[u8], out: &mut [f32; FRAME_SAMPLES]) -> bool {
        let f = unpack(rate, payload);
        if f.reserved != 0 {
            return false;
        }
        match rate {
            Rate::Eighth if all_bits(payload, rate, 1) => return false,
            Rate::Full | Rate::Half | Rate::Quarter if all_bits(payload, rate, 0) => return false,
            _ => {}
        }
        // LSPs (§2.4.3.3.1)
        let mut wq = [0f32; ORDER];
        if rate == Rate::Eighth {
            let mut d = [0f32; ORDER];
            for i in 0..ORDER {
                let q = if f.lsp[i + 1] != 0 { 0.02 } else { -0.02 };
                d[i] = q + 0.90625 * self.pred[i];
                wq[i] = d[i] + bias(i);
            }
            stabilise(&mut wq);
            self.pred = d;
        } else {
            let books: [&[[f32; 2]]; 5] = [&LSPVQ1, &LSPVQ2, &LSPVQ3, &LSPVQ4, &LSPVQ5];
            let mut acc = 0.0;
            for (k, book) in books.iter().enumerate() {
                let v = book[f.lspv[k + 1] as usize % book.len()];
                acc += v[0];
                wq[2 * k] = acc;
                acc += v[1];
                wq[2 * k + 1] = acc;
            }
            // §2.4.8.7.3: implausible LSPs mean a misdetected packet
            let bad = match rate {
                Rate::Full | Rate::Half => {
                    wq[9] <= 0.66
                        || wq[9] >= 0.985
                        || (4..10).any(|n| (wq[n] - wq[n - 4]).abs() < 0.0931)
                }
                _ => {
                    wq[9] <= 0.70
                        || wq[9] >= 0.97
                        || (3..10).any(|n| (wq[n] - wq[n - 3]).abs() < 0.08)
                }
            };
            if bad {
                return false;
            }
        }
        // pitch and codebook parameters, checked before any state changes
        let mut lags = [0f32; 4];
        let mut gains = [0f32; 4];
        if matches!(rate, Rate::Full | Rate::Half) {
            for i in 0..4 {
                let plag = f.plag[i + 1];
                if plag == 0 {
                    continue;
                }
                let frac = f.pfrac[i + 1];
                let lag = plag as f32 + 16.0 + 0.5 * frac as f32;
                if frac == 1 && plag + 16 >= 140 {
                    return false; // 140.5 .. 143.5 are invalid (§2.4.5.1.3)
                }
                lags[i] = lag;
                gains[i] = (f.pgain[i + 1] as f32 + 1.0) / 4.0;
            }
        }
        if rate == Rate::Quarter {
            let g0: [i32; 5] = std::array::from_fn(|i| 4 * f.cbgain[i + 1] as i32);
            if (0..4).any(|i| (g0[i + 1] - g0[i]).abs() > 40)
                || (0..3).any(|i| (g0[i + 2] - 2 * g0[i + 1] + g0[i]).abs() > 48)
            {
                return false;
            }
        }

        // LSP predictor and low-pass filter (§2.4.3.3.3)
        let sm = if rate == Rate::Eighth {
            self.eighth_count += 1;
            if self.eighth_count < 10 { 0.125 } else { 0.9 }
        } else {
            self.eighth_count = 0;
            for i in 0..ORDER {
                self.pred[i] = wq[i] - bias(i);
            }
            0.0
        };
        let mut cur = [0f32; ORDER];
        for i in 0..ORDER {
            cur[i] = sm * self.prev_lsp[i] + (1.0 - sm) * wq[i];
        }

        // the scaled codebook vector (§2.4.8.1)
        let mut cd = [0f32; FRAME_SAMPLES];
        match rate {
            Rate::Full => {
                for k in 0..16 {
                    let g1 = if k % 4 == 3 {
                        let avg =
                            (self.g1_hist[0] + self.g1_hist[1] + self.g1_hist[2]).div_euclid(3);
                        4 * f.cbgain[k + 1] as i32 - 6 + avg.clamp(6, 38)
                    } else {
                        4 * f.cbgain[k + 1] as i32
                    };
                    let g = self.push_gain(g1);
                    let (gs, idx) = sign_index(f.cbsign[k + 1], f.cbindex[k + 1]);
                    for n in 0..10 {
                        cd[k * 10 + n] = gs * g * CODEBOOK_FULL[(n + 128 - idx) % 128];
                    }
                    self.g_prev = g;
                }
            }
            Rate::Half => {
                for k in 0..4 {
                    let g = self.push_gain(4 * f.cbgain[k + 1] as i32);
                    let (gs, idx) = sign_index(f.cbsign[k + 1], f.cbindex[k + 1]);
                    for n in 0..40 {
                        cd[k * 40 + n] = gs * g * CODEBOOK_HALF[(n + 128 - idx) % 128];
                    }
                    self.g_prev = g;
                }
            }
            Rate::Quarter => {
                let mut g = [0f32; 5];
                for k in 0..5 {
                    g[k] = self.push_gain(4 * f.cbgain[k + 1] as i32);
                }
                // DECSD: Table 2.4.6.1.5-1 (bits 33..24 and 46..41 of the packet)
                let bit = |b: usize| {
                    let i = 53 - b;
                    payload
                        .get(i / 8)
                        .map_or(0, |v| u32::from((v >> (7 - i % 8)) & 1))
                };
                let seed = [
                    33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 46, 45, 44, 43, 42, 41,
                ]
                .iter()
                .fold(0, |acc, &b| (acc << 1) | bit(b));
                let mut rnd = [0f32; FRAME_SAMPLES];
                random_vector(seed, &mut rnd);
                for n in 0..FRAME_SAMPLES {
                    // FIR band-pass over the running excitation (states kept between frames)
                    let mut acc = BPF[0] * rnd[n];
                    for k in 1..21 {
                        let x = if n >= k {
                            rnd[n - k]
                        } else {
                            self.bpf_mem[20 + n - k]
                        };
                        acc += BPF[k] * x;
                    }
                    let seg = n / 20;
                    let gain = match seg {
                        0 => g[0],
                        1 => 0.6 * g[0] + 0.4 * g[1],
                        2 => g[1],
                        3 => 0.2 * g[1] + 0.8 * g[2],
                        4 => 0.8 * g[2] + 0.2 * g[3],
                        5 => g[3],
                        6 => 0.4 * g[3] + 0.6 * g[4],
                        _ => g[4],
                    };
                    cd[n] = gain * acc;
                }
                self.bpf_mem.copy_from_slice(&rnd[FRAME_SAMPLES - 20..]);
                self.g_prev = g[4];
            }
            Rate::Eighth => {
                // Eq. 2.4.6.1.6-2's round() of the mean of the last two G1 taken as the
                // integer part: rounding halves up makes the decoded noise floor creep
                // 0.5 dB a frame above the encoder's through long pauses.
                let pg = (self.g1_hist[0] + self.g1_hist[1]).div_euclid(2);
                let pg = if pg <= 4 {
                    4
                } else if pg >= 59 {
                    58
                } else {
                    pg - 1
                };
                let g1 = 2 * f.cbgain[1] as i32 - 4 + pg;
                let ga = self.push_gain(g1);
                let gc = 0.5 * self.g_prev + 0.5 * ga;
                let seed = leading_bits(payload, 16);
                let mut rnd = [0f32; FRAME_SAMPLES];
                random_vector(seed, &mut rnd);
                for n in 0..FRAME_SAMPLES {
                    let w = (n / 20 + 1) as f32 / 8.0;
                    cd[n] = ((1.0 - w) * self.g_prev + w * gc) * rnd[n];
                }
                self.g_prev = gc;
            }
            _ => {}
        }
        if matches!(rate, Rate::Full | Rate::Half) {
            self.last_lag = lags[3];
            self.last_b = gains[3];
        } else {
            self.last_lag = 0.0;
            self.last_b = 0.0;
        }
        let use_ = if rate == Rate::Eighth {
            LspUse::Eighth
        } else {
            LspUse::Four
        };
        let pre = matches!(rate, Rate::Full | Rate::Half);
        self.synthesise(&cd, &lags, &gains, pre, &cur, use_, out);
        self.prev_lsp = cur;
        self.last_good = rate;
        self.erasures = 0;
        true
    }

    /// Clamps G1 to the table, records it and returns the linear gain Ĝa.
    fn push_gain(&mut self, g1: i32) -> f32 {
        let g1 = g1.clamp(0, 60);
        self.g1_hist = [g1, self.g1_hist[0], self.g1_hist[1]];
        GAIN_LINEAR[g1 as usize]
    }

    /// §2.4.8.7.1.
    fn erasure_frame(&mut self, out: &mut [f32; FRAME_SAMPLES]) {
        self.erasures = self.erasures.saturating_add(1);
        let e = self.erasures;
        let sub = match e {
            1 => 0,
            2 => 1,
            3 => 2,
            _ => 6,
        };
        let ga = self.push_gain(self.g1_hist[0] - sub);
        let gc = 0.5 * self.g_prev + 0.5 * ga;
        self.erase_seed = (521 * self.erase_seed + 259) & 0xffff;
        let idx = (self.erase_seed >> 9) as usize % 128;
        let mut cd = [0f32; FRAME_SAMPLES];
        for n in 0..FRAME_SAMPLES {
            let g = match n / 40 {
                0 => 0.75 * self.g_prev + 0.25 * gc,
                1 => 0.5 * self.g_prev + 0.5 * gc,
                2 => 0.25 * self.g_prev + 0.75 * gc,
                _ => gc,
            };
            cd[n] = g * CODEBOOK_FULL[(n + 128 * 2 - idx) % 128];
        }
        self.g_prev = gc;
        let (lag, b) = if matches!(self.last_good, Rate::Full | Rate::Half) {
            let be = match e {
                1 => 0.9,
                2 => 0.6,
                3 => 0.3,
                _ => 0.0,
            };
            (self.last_lag, self.last_b.min(be))
        } else {
            (0.0, 0.0)
        };
        self.last_b = b;
        let coef = match e {
            1 => 1.0,
            2 | 3 => 0.9,
            _ => 0.7,
        };
        let mut cur = [0f32; ORDER];
        for i in 0..ORDER {
            self.pred[i] *= coef;
            let wq = self.pred[i] + bias(i);
            cur[i] = 0.875 * self.prev_lsp[i] + 0.125 * wq;
        }
        let lags = [lag; 4];
        let gains = [b; 4];
        let prev = self.prev_lsp;
        self.synthesise(&cd, &lags, &gains, true, &prev, LspUse::Previous, out);
        self.prev_lsp = cur;
    }

    /// §2.4.8.7.2.
    fn blank_frame(&mut self, out: &mut [f32; FRAME_SAMPLES]) {
        let b = self.last_b.min(1.0);
        let lags = [self.last_lag; 4];
        let gains = [b; 4];
        let cd = [0f32; FRAME_SAMPLES];
        let prev = self.prev_lsp;
        self.synthesise(&cd, &lags, &gains, true, &prev, LspUse::Previous, out);
    }

    /// The pitch filter, pre-filter, formant synthesis and postfilter over one frame.
    #[allow(clippy::too_many_arguments)]
    fn synthesise(
        &mut self,
        cd: &[f32; FRAME_SAMPLES],
        lags: &[f32; 4],
        gains: &[f32; 4],
        prefilter: bool,
        cur: &[f32; ORDER],
        use_: LspUse,
        out: &mut [f32; FRAME_SAMPLES],
    ) {
        // pitch synthesis (§2.4.8.2) and pre-filter (§2.4.8.3)
        let mut pbuf = [0f32; HIST + FRAME_SAMPLES];
        pbuf[..HIST].copy_from_slice(&self.pitch_mem);
        let mut qbuf = [0f32; HIST + FRAME_SAMPLES];
        qbuf[..HIST].copy_from_slice(&self.pre_mem);
        let mut excite = [0f32; FRAME_SAMPLES];
        for sf in 0..4 {
            let (lag, b) = (lags[sf], gains[sf]);
            let b_pre = if prefilter { 0.5 * b.min(1.0) } else { 0.0 };
            let (mut e_in, mut e_out) = (0f32, 0f32);
            for n in sf * 40..sf * 40 + 40 {
                let at = HIST + n;
                let p = cd[n]
                    + if b != 0.0 {
                        b * self.delayed(&pbuf, at, lag)
                    } else {
                        0.0
                    };
                pbuf[at] = p;
                let q = p + if b_pre != 0.0 {
                    b_pre * self.delayed(&qbuf, at, lag)
                } else {
                    0.0
                };
                qbuf[at] = q;
                e_in += p * p;
                e_out += q * q;
            }
            let gain = if b_pre != 0.0 && e_out > 0.0 {
                (e_in / e_out).sqrt()
            } else {
                1.0
            };
            for n in sf * 40..sf * 40 + 40 {
                excite[n] = gain * qbuf[HIST + n];
            }
        }
        self.pitch_mem.copy_from_slice(&pbuf[FRAME_SAMPLES..]);
        self.pre_mem.copy_from_slice(&qbuf[FRAME_SAMPLES..]);

        // formant synthesis (§2.4.8.4) and postfilter (§2.4.8.6), per 40 samples
        let mut lpc = lsp_to_lpc(&interp(&self.prev_lsp, cur, use_, 0));
        let mut yd = [0f32; FRAME_SAMPLES];
        let mut pf = [0f32; FRAME_SAMPLES];
        for sf in 0..4 {
            if use_ == LspUse::Four && sf > 0 {
                lpc = lsp_to_lpc(&interp(&self.prev_lsp, cur, use_, sf));
            }
            let mut num = [0f32; ORDER];
            let mut den = [0f32; ORDER];
            let (mut gs, mut gp) = (PF_S, PF_P);
            for i in 0..ORDER {
                num[i] = lpc[i] * gs;
                den[i] = lpc[i] * gp;
                gs *= PF_S;
                gp *= PF_P;
            }
            for k in 0..40 {
                let n = sf * 40 + k;
                let mut y = excite[n];
                for i in 0..ORDER {
                    y += lpc[i] * self.syn_mem[i];
                }
                self.syn_mem.copy_within(0..ORDER - 1, 1);
                self.syn_mem[0] = y;
                yd[n] = y;
                // A(z/s) zeros over yd, then 1/A(z/p), then the anti-tilt filter
                let mut u = y;
                for i in 0..ORDER {
                    u -= num[i] * self.pf_fir[i];
                }
                self.pf_fir.copy_within(0..ORDER - 1, 1);
                self.pf_fir[0] = y;
                let mut v = u;
                for i in 0..ORDER {
                    v += den[i] * self.pf_iir[i];
                }
                self.pf_iir.copy_within(0..ORDER - 1, 1);
                self.pf_iir[0] = v;
                // The anti-tilt B(z) as the FIR 1 − 0.3 z^-1: the same tilt as the
                // 1/(1 + 0.3 z^-1) printed in Eq. 2.4.8.6-2, and the form the decoders in use
                // apply (7 dB closer to ffmpeg's output on real files).
                pf[n] = v - 0.3 * self.pf_tilt;
                self.pf_tilt = v;
            }
        }
        // gain control (Eq. 2.4.8.6-3 .. -7), the first-order smoothing of SCALE run per
        // sample so the gain has no steps at subframe edges
        for s0 in (0..FRAME_SAMPLES).step_by(40) {
            let e_in: f32 = yd[s0..s0 + 40].iter().map(|v| v * v).sum();
            let e_out: f32 = pf[s0..s0 + 40].iter().map(|v| v * v).sum();
            let tmp = if e_out > 0.0 {
                (e_in / e_out).sqrt()
            } else {
                1.0
            };
            for n in s0..s0 + 40 {
                self.scale = 0.9375 * self.scale + 0.0625 * tmp;
                out[n] = self.scale * pf[n];
            }
        }
        // guard against runaway states from hostile streams
        for v in self.syn_mem.iter_mut().chain(self.pf_iir.iter_mut()) {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        for v in self.pitch_mem.iter_mut().chain(self.pre_mem.iter_mut()) {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        if !self.pf_tilt.is_finite() {
            self.pf_tilt = 0.0;
        }
        if !self.scale.is_finite() {
            self.scale = 1.0;
        }
        for v in out.iter_mut() {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
    }

    /// p(n − L) from a history buffer, with the 8-tap interpolation of Eq. 2.4.5.2-2 for
    /// half-sample lags.
    fn delayed(&self, buf: &[f32], at: usize, lag: f32) -> f32 {
        let li = lag.floor() as usize;
        if lag.fract() == 0.0 {
            return buf[at - li];
        }
        // p(n − L) = Σ_{i=−4..3} hammsinc(i + 0.5) p(n + i − (L − 0.5))
        let base = at - li;
        let mut acc = 0.0;
        for (k, t) in self.taps.iter().enumerate() {
            acc += t * buf[base + k - 4];
        }
        acc
    }
}

/// §2.4.6.2.1: the sign and the codebook index it shifts.
fn sign_index(sign: u32, index: u32) -> (f32, usize) {
    if sign != 0 {
        (-1.0, (index as usize + 128 - 89) % 128)
    } else {
        (1.0, index as usize % 128)
    }
}

/// The LSP set for 40-sample subframe `sf` (Table 2.4.3.3.4-1).
fn interp(prev: &[f32; ORDER], cur: &[f32; ORDER], use_: LspUse, sf: usize) -> [f32; ORDER] {
    let w = match use_ {
        LspUse::Four => [0.25, 0.5, 0.75, 1.0][sf],
        LspUse::Eighth => 0.625,
        LspUse::Previous => 0.0,
    };
    std::array::from_fn(|i| (1.0 - w) * prev[i] + w * cur[i])
}

/// §2.4.3.3.2: ordered LSPs at least 0.02 apart.
fn stabilise(w: &mut [f32; ORDER]) {
    const MIN: f32 = 0.02;
    let mut ext = [0f32; ORDER + 2];
    ext[1..=ORDER].copy_from_slice(w);
    for i in 0..ORDER {
        if ext[i + 1] - ext[i] < MIN {
            ext[i + 1] = ext[i] + MIN;
        }
    }
    ext[ORDER + 1] = 1.0;
    for i in (1..=ORDER).rev() {
        if ext[i + 1] - ext[i] < MIN {
            ext[i] = ext[i + 1] - MIN;
        }
    }
    w.copy_from_slice(&ext[1..=ORDER]);
}

/// The rate and frame of a packet: a rate octet when the length agrees with it, else a
/// bare frame by its length, else an erasure.
pub fn split_packet(packet: &[u8]) -> (Rate, &[u8]) {
    let octet = packet.first().and_then(|&b| Rate::from_byte(b));
    if let Some(r) = octet
        && packet.len() == r.payload_bytes() + 1
    {
        return (r, &packet[1..]);
    }
    if let Some(r) = Rate::from_len(packet.len()) {
        return (r, packet);
    }
    match octet {
        // trailing padding after the frame
        Some(r) if packet.len() > r.payload_bytes() => (r, &packet[1..=r.payload_bytes()]),
        _ => (Rate::Erasure, &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_bits(table: &[(u8, u8, u8)]) -> [[u32; 17]; 10] {
        let mut n = [[0u32; 17]; 10];
        for &(f, i, _) in table {
            n[f as usize][i as usize] += 1;
        }
        n
    }

    #[test]
    fn packing_matches_bit_allocation() {
        // Table 2.4.1-2
        let full = field_bits(&PACK_FULL);
        assert_eq!(&full[0][1..6], &[6, 7, 7, 6, 6]);
        for i in 1..=4 {
            assert_eq!((full[2][i], full[3][i], full[4][i]), (7, 1, 3));
        }
        for i in 1..=16 {
            assert_eq!(full[5][i], 7, "CBINDEX{i}");
            assert_eq!(full[6][i], if i % 4 == 0 { 3 } else { 4 }, "CBGAIN{i}");
            assert_eq!(full[7][i], 1);
        }
        assert_eq!(full[9][0], 2);
        let half = field_bits(&PACK_HALF);
        assert_eq!(&half[0][1..6], &[6, 7, 7, 6, 6]);
        for i in 1..=4 {
            assert_eq!((half[5][i], half[6][i], half[7][i]), (7, 4, 1));
        }
        let q = field_bits(&PACK_QUARTER);
        assert_eq!(&q[6][1..6], &[4, 4, 4, 4, 4]);
        assert_eq!(q[9][0], 2);
        let e = field_bits(&PACK_EIGHTH);
        assert_eq!(&e[1][1..11], &[1; 10]);
        assert_eq!((e[8][0], e[6][1], e[9][0]), (4, 2, 4));
    }

    #[test]
    fn tables_transcribed() {
        // spot checks against the printed tables
        assert_eq!(LSPVQ3[23], [0.2393, 0.0646]);
        assert_eq!(LSPVQ1[43], [0.1122, 0.0751]);
        assert_eq!(GAIN_LINEAR[60], 1000.0);
        for (g, &v) in GAIN_LINEAR.iter().enumerate() {
            let want = (10f32.powf(g as f32 / 20.0) * 8.0).round() / 8.0;
            assert!((v - want).abs() <= 0.125, "{g}: {v} vs {want}");
        }
        assert_eq!(BPF[10], 0.8985137);
        assert_eq!(CODEBOOK_HALF[106], 3.0);
        assert_eq!(CODEBOOK_FULL[127], -0.65);
    }

    #[test]
    fn rnd_example() {
        // §2.4.6.1.6: SD_old = 40481 gives SD_new = 53804
        assert_eq!((521 * 40481 + 259) & 0xffff, 53804);
    }

    #[test]
    fn flat_lsps_give_flat_filter() {
        let w: [f32; ORDER] = std::array::from_fn(bias);
        let a = lsp_to_lpc(&w);
        assert!(a.iter().all(|v| v.abs() < 1e-4), "{a:?}");
    }

    #[test]
    fn packet_splitting() {
        let mut p = vec![4u8];
        p.extend([0u8; 34]);
        assert_eq!(split_packet(&p).0, Rate::Full);
        assert_eq!(split_packet(&p[1..]).0, Rate::Full);
        assert_eq!(split_packet(&[14]).0, Rate::Erasure);
        assert_eq!(split_packet(&[0]).0, Rate::Blank);
        assert_eq!(split_packet(&[9, 9]).0, Rate::Erasure);
        assert_eq!(packet_len(3), Some(17));
    }

    #[test]
    fn garbage_never_panics() {
        let mut d = Decoder::new();
        let mut out = Vec::new();
        let mut s = 1u32;
        for i in 0..4000 {
            let rate = [1u8, 2, 3, 4, 0, 14][i % 6];
            let n = packet_len(rate).unwrap();
            let mut p = vec![rate];
            for _ in 1..n {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                p.push((s >> 16) as u8);
            }
            d.decode(&p, &mut out);
        }
        assert_eq!(out.len(), 4000 * FRAME_SAMPLES);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
    }
}
