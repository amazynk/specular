//! Spectral band replication (HE-AAC v1, ISO/IEC 14496-3 §4.6.18): the SBR header and data
//! of one channel element, the frequency band tables, and the decoding of a frame: QMF
//! analysis of the core output, HF generation by patching with inverse filtering, HF
//! adjustment (envelope gains, limiter, noise floor, added sinusoids, gain smoothing) and QMF
//! synthesis at twice the core rate (or at the core rate for downsampled SBR).
//!
//! Time runs in QMF slots of 32 core samples (32 per 1024-sample frame). The low band
//! buffer holds 8 slots of the previous frame before the current 32; SBR slot `s` of a frame
//! is buffer slot `s + 2`, and an SBR frame may reach 6 slots into the next core frame, whose
//! output then takes those slots from this frame.

// Band, envelope and slot indices address several parallel arrays at once here.
#![allow(clippy::needless_range_loop)]

pub(crate) mod qmf;

use std::sync::OnceLock;

use crate::bits::Bits;
use crate::data::sbr as tab;
use crate::fft::C;
use crate::ps::Ps;
use crate::vlc::Vlc;
use crate::{Error, Result};

/// QMF slots per frame.
pub(crate) const SLOTS: usize = 32;
/// SBR slot `s` is low-band buffer slot `s + HF_ADJ` (t_HFAdj).
const HF_ADJ: usize = 2;
/// Slots of the previous frame kept in front of the low-band buffer (t_HFGen).
const HF_GEN: usize = 8;
/// Envelope time borders are in units of two QMF slots, 16 per frame.
const TIME_SLOTS: u8 = 16;
/// Most envelopes and noise floors per frame.
const MAX_ENV: usize = 5;
const MAX_NOISE: usize = 2;

fn malformed(s: &str) -> Error {
    Error::Malformed(format!("SBR: {s}"))
}

/// The Huffman codebooks, by the order of `data::sbr`.
pub(crate) struct Books {
    pub env_1_5_t: Vlc,
    pub env_1_5_f: Vlc,
    pub bal_1_5_t: Vlc,
    pub bal_1_5_f: Vlc,
    pub env_3_0_t: Vlc,
    pub env_3_0_f: Vlc,
    pub bal_3_0_t: Vlc,
    pub bal_3_0_f: Vlc,
    pub noise_t: Vlc,
    pub noise_bal_t: Vlc,
}

/// Leaf values of the trees are signed deltas; the codebook symbol is the delta plus this.
pub(crate) const TREE_BIAS: i32 = 128;

/// Builds a prefix-code decoder from a PacketVideo-style tree.
pub(crate) fn tree_vlc(tree: tab::Tree) -> Option<Vlc> {
    let mut codes = Vec::new();
    let mut stack = vec![(0usize, 0u32, 0u8)];
    while let Some((node, code, len)) = stack.pop() {
        if len >= 24 {
            return None;
        }
        for bit in 0..2u32 {
            let child = i32::from(tree.get(node)?[bit as usize]);
            let (c, l) = ((code << 1) | bit, len + 1);
            if child < 0 {
                codes.push((c, l, (child + 64 + TREE_BIAS) as u32));
            } else if (child as usize) > node {
                stack.push((child as usize, c, l));
            } else {
                return None;
            }
        }
    }
    Vlc::new(&codes)
}

pub(crate) fn books() -> Option<&'static Books> {
    static B: OnceLock<Option<Books>> = OnceLock::new();
    B.get_or_init(|| {
        Some(Books {
            env_1_5_t: tree_vlc(tab::ENV_1_5DB_T)?,
            env_1_5_f: tree_vlc(tab::ENV_1_5DB_F)?,
            bal_1_5_t: tree_vlc(tab::BAL_1_5DB_T)?,
            bal_1_5_f: tree_vlc(tab::BAL_1_5DB_F)?,
            env_3_0_t: tree_vlc(tab::ENV_3_0DB_T)?,
            env_3_0_f: tree_vlc(tab::ENV_3_0DB_F)?,
            bal_3_0_t: tree_vlc(tab::BAL_3_0DB_T)?,
            bal_3_0_f: tree_vlc(tab::BAL_3_0DB_F)?,
            noise_t: tree_vlc(tab::NOISE_3_0DB_T)?,
            noise_bal_t: tree_vlc(tab::NOISE_BAL_3_0DB_T)?,
        })
    })
    .as_ref()
}

/// Reads one Huffman-coded delta.
fn delta(b: &mut Bits, v: &Vlc) -> Result<i32> {
    v.read(b)
        .map(|s| s as i32 - TREE_BIAS)
        .ok_or_else(|| malformed("invalid Huffman code"))
}

/// sbr_header().
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    amp_res: bool,
    start_freq: u8,
    stop_freq: u8,
    xover_band: u8,
    freq_scale: u8,
    alter_scale: bool,
    noise_bands: u8,
    limiter_bands: u8,
    limiter_gains: u8,
    interpol_freq: bool,
    smoothing_mode: bool,
}

impl Header {
    fn read(b: &mut Bits) -> Header {
        let amp_res = b.bit();
        let start_freq = b.read(4) as u8;
        let stop_freq = b.read(4) as u8;
        let xover_band = b.read(3) as u8;
        b.read(2); // bs_reserved
        let extra_1 = b.bit();
        let extra_2 = b.bit();
        let (freq_scale, alter_scale, noise_bands) = if extra_1 {
            (b.read(2) as u8, b.bit(), b.read(2) as u8)
        } else {
            (2, true, 2)
        };
        let (limiter_bands, limiter_gains, interpol_freq, smoothing_mode) = if extra_2 {
            (b.read(2) as u8, b.read(2) as u8, b.bit(), b.bit())
        } else {
            (2, 2, true, true)
        };
        Header {
            amp_res,
            start_freq,
            stop_freq,
            xover_band,
            freq_scale,
            alter_scale,
            noise_bands,
            limiter_bands,
            limiter_gains,
            interpol_freq,
            smoothing_mode,
        }
    }

    /// Whether `o` gives the same frequency band tables.
    fn same_bands(&self, o: &Header) -> bool {
        self.start_freq == o.start_freq
            && self.stop_freq == o.stop_freq
            && self.xover_band == o.xover_band
            && self.freq_scale == o.freq_scale
            && self.alter_scale == o.alter_scale
            && self.noise_bands == o.noise_bands
            && self.limiter_bands == o.limiter_bands
    }
}

/// Nearest integer, halves up (the standard's NINT).
fn nint(x: f64) -> i32 {
    (x + 0.5).floor() as i32
}

/// `n` band widths dividing `start..stop` logarithmically (the standard's vDk), ascending.
fn log_bands(start: i32, stop: i32, n: i32) -> Vec<i32> {
    let ratio = f64::from(stop) / f64::from(start);
    let edge = |k: i32| nint(f64::from(start) * ratio.powf(f64::from(k) / f64::from(n)));
    let mut v: Vec<i32> = (0..n).map(|k| edge(k + 1) - edge(k)).collect();
    v.sort_unstable();
    v
}

/// The frequency band tables of a header (§4.6.18.3.2), in QMF band indices.
#[derive(Debug, Clone, Default)]
pub(crate) struct Bands {
    pub kx: usize,
    pub m: usize,
    f_high: Vec<usize>,
    f_low: Vec<usize>,
    f_noise: Vec<usize>,
    f_lim: Vec<usize>,
    /// (first source band, bands) of each patch.
    patches: Vec<(usize, usize)>,
    /// For a time delta from low to high resolution: the low band holding each high band.
    high_from_low: Vec<usize>,
    /// From high to low resolution: the high band starting where each low band starts.
    low_from_high: Vec<usize>,
}

impl Bands {
    /// `fs` is the SBR rate (twice the core rate).
    fn new(h: &Header, fs: u32) -> Option<Bands> {
        // start and stop bands
        let class = match fs {
            0..=16000 => 0,
            16001..=22050 => 1,
            22051..=24000 => 2,
            24001..=32000 => 3,
            32001..=64000 => 4,
            _ => 5,
        };
        let fs64 = f64::from(fs);
        let start_min = if fs < 32000 {
            3000.0
        } else if fs < 64000 {
            4000.0
        } else {
            5000.0
        };
        let start_min = nint(start_min * 128.0 / fs64);
        let k0 = start_min + i32::from(tab::START_OFFSET[class][usize::from(h.start_freq)]);
        let stop_min = if fs < 32000 {
            6000.0
        } else if fs < 64000 {
            8000.0
        } else {
            10000.0
        };
        let stop_min = nint(stop_min * 128.0 / fs64);
        let k2 = match h.stop_freq {
            14 => 2 * k0,
            15 => 3 * k0,
            s => stop_band(stop_min, usize::from(s)),
        }
        .min(64);
        if k0 <= 0 || k2 <= k0 || k0 >= 64 {
            return None;
        }
        // the limits of §4.6.18.3.6
        let max_span = if fs <= 32000 {
            48
        } else if fs <= 44100 {
            35
        } else {
            32
        };
        if k2 - k0 > max_span {
            return None;
        }
        // master table
        let mut master = vec![k0];
        if h.freq_scale == 0 {
            let dk = if h.alter_scale { 2 } else { 1 };
            let n = if h.alter_scale {
                2 * nint(f64::from(k2 - k0) / 4.0)
            } else {
                2 * ((k2 - k0) / 2)
            };
            if n <= 0 {
                return None;
            }
            let mut vdk = vec![dk; n as usize];
            let mut diff = k2 - (k0 + n * dk);
            let (mut k, incr) = if diff < 0 { (0i32, 1) } else { (n - 1, -1) };
            while diff != 0 {
                if !(0..n).contains(&k) {
                    return None;
                }
                vdk[k as usize] -= incr;
                k += incr;
                diff += incr;
            }
            for d in vdk {
                master.push(master.last()? + d);
            }
        } else {
            let bands = [12.0, 10.0, 8.0][usize::from(h.freq_scale - 1)];
            let warp = if h.alter_scale { 1.3 } else { 1.0 };
            let two = f64::from(k2) / f64::from(k0) > 2.2449;
            let k1 = if two { 2 * k0 } else { k2 };
            let n0 = 2 * nint(bands * (f64::from(k1) / f64::from(k0)).log2() / 2.0);
            if n0 <= 0 {
                return None;
            }
            let vdk0 = log_bands(k0, k1, n0);
            if vdk0[0] <= 0 {
                return None;
            }
            for &d in &vdk0 {
                master.push(master.last()? + d);
            }
            if two {
                let n1 = 2 * nint(bands * (f64::from(k2) / f64::from(k1)).log2() / (2.0 * warp));
                if n1 > 0 {
                    let mut vdk1 = log_bands(k1, k2, n1);
                    let max0 = *vdk0.last()?;
                    if max0 > vdk1[0] {
                        let change = max0 - vdk1[0];
                        vdk1[0] += change;
                        let last = vdk1.len() - 1;
                        vdk1[last] -= change;
                        vdk1.sort_unstable();
                    }
                    if vdk1[0] <= 0 {
                        return None;
                    }
                    for d in vdk1 {
                        master.push(master.last()? + d);
                    }
                }
            }
        }
        let n_master = master.len() - 1;
        let xover = usize::from(h.xover_band);
        if xover >= n_master {
            return None;
        }
        let f_high: Vec<usize> = master[xover..].iter().map(|&v| v as usize).collect();
        let n_high = f_high.len() - 1;
        let kx = f_high[0];
        let m = f_high[n_high] - kx;
        if kx > 32 || kx + m > 64 || m == 0 {
            return None;
        }
        let n_low = n_high - n_high / 2;
        let odd = n_high % 2;
        let f_low: Vec<usize> = (0..=n_low)
            .map(|k| f_high[if k == 0 { 0 } else { 2 * k - odd }])
            .collect();
        let n_q = if h.noise_bands == 0 {
            1
        } else {
            nint(f64::from(h.noise_bands) * (k2 as f64 / kx as f64).log2()).max(1) as usize
        };
        if n_q > 5 {
            return None;
        }
        let mut f_noise = vec![f_low[0]];
        let mut i = 0usize;
        for k in 1..=n_q {
            i += (n_low - i) / (n_q + 1 - k);
            f_noise.push(f_low[i]);
        }
        let patches = patches(&master, k0 as usize, kx, m, fs)?;
        let f_lim = limiter_bands(h.limiter_bands, &f_low, kx, &patches);
        let high_from_low = (0..n_high)
            .map(|k| (0..n_low).rfind(|&i| f_low[i] <= f_high[k]).unwrap_or(0))
            .collect();
        let low_from_high = (0..n_low)
            .map(|k| f_high.iter().position(|&v| v == f_low[k]).unwrap_or(0))
            .collect();
        Some(Bands {
            kx,
            m,
            f_high,
            f_low,
            f_noise,
            f_lim,
            patches,
            high_from_low,
            low_from_high,
        })
    }

    fn n_high(&self) -> usize {
        self.f_high.len() - 1
    }

    fn n_low(&self) -> usize {
        self.f_low.len() - 1
    }

    fn n_q(&self) -> usize {
        self.f_noise.len() - 1
    }

    fn table(&self, high: bool) -> &[usize] {
        if high { &self.f_high } else { &self.f_low }
    }
}

/// The stop band for `bs_stop_freq` below 14: `stop_min` plus the first widths of the
/// 13 logarithmic steps up to band 64, sorted narrowest first (§4.6.18.3.2.1).
fn stop_band(stop_min: i32, stop_freq: usize) -> i32 {
    stop_min
        + log_bands(stop_min, 64, 13)[..stop_freq.min(13)]
            .iter()
            .sum::<i32>()
}

/// The patches copying low bands up (§4.6.18.6.3).
fn patches(master: &[i32], k0: usize, kx: usize, m: usize, fs: u32) -> Option<Vec<(usize, usize)>> {
    let n_master = master.len() - 1;
    let goal = ((2_048_000u64 + u64::from(fs) / 2) / u64::from(fs)) as usize;
    let mut k = if goal < kx + m {
        master
            .iter()
            .position(|&v| v as usize >= goal)
            .unwrap_or(n_master)
    } else {
        n_master
    };
    let (mut msb, mut usb) = (k0, kx);
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 64 {
            return None;
        }
        let mut j = k + 1;
        let (mut sb, mut odd);
        loop {
            j -= 1;
            sb = master[j] as usize;
            odd = (sb + k0) % 2;
            if j == 0 || sb + odd < k0 + msb {
                break;
            }
        }
        if sb + odd > k0 + msb - 1 {
            return None;
        }
        let len = sb.saturating_sub(usb);
        let start = (k0 - odd).checked_sub(len)?;
        if len > 0 {
            out.push((start, len));
            usb = sb;
            msb = sb;
        } else {
            msb = kx;
        }
        if out.len() > 5 {
            return None;
        }
        if (master[k] as usize) < sb + 3 {
            k = n_master;
        }
        if sb == kx + m {
            break;
        }
    }
    if out.len() > 1 && out.last()?.1 < 3 {
        out.pop();
    }
    if out.is_empty() || out.iter().any(|&(s, l)| s + l > kx) {
        return None;
    }
    Some(out)
}

/// The limiter band table (§4.6.18.3.2.3).
fn limiter_bands(
    limiter: u8,
    f_low: &[usize],
    kx: usize,
    patches: &[(usize, usize)],
) -> Vec<usize> {
    let n_low = f_low.len() - 1;
    if limiter == 0 {
        return vec![f_low[0], f_low[n_low]];
    }
    let per_octave = [1.2f64, 2.0, 3.0][usize::from(limiter - 1)];
    let mut borders = vec![kx];
    for &(_, l) in patches {
        borders.push(borders.last().copied().unwrap_or(kx) + l);
    }
    let mut t: Vec<usize> = f_low.to_vec();
    t.extend_from_slice(&borders[1..borders.len() - 1]);
    t.sort_unstable();
    let is_border = |v: usize| borders.contains(&v);
    let mut k = 1;
    while k < t.len() {
        let octaves = (t[k] as f64 / t[k - 1] as f64).log2();
        if octaves * per_octave < 0.49 {
            if t[k] == t[k - 1] {
                t.remove(k);
                continue;
            }
            if is_border(t[k]) {
                if is_border(t[k - 1]) {
                    k += 1;
                } else {
                    t.remove(k - 1);
                }
            } else {
                t.remove(k);
            }
        } else {
            k += 1;
        }
    }
    t
}

/// sbr_grid() of one channel.
#[derive(Debug, Clone, Copy, Default)]
struct Grid {
    num_env: usize,
    /// Envelope borders in units of two QMF slots.
    t_e: [u8; MAX_ENV + 1],
    freq_res: [bool; MAX_ENV],
    num_noise: usize,
    t_q: [u8; MAX_NOISE + 1],
    /// The transient envelope, or −1.
    l_a: i32,
    /// `bs_amp_res` for this frame (forced to 1.5 dB for a single FIXFIX envelope).
    amp_res: bool,
}

/// Bits of `bs_pointer` for `n` envelopes: ceil(log2(n + 1)).
fn pointer_bits(n: usize) -> u32 {
    usize::BITS - n.leading_zeros()
}

impl Grid {
    fn read(b: &mut Bits, amp_res: bool) -> Result<Grid> {
        let mut g = Grid {
            amp_res,
            ..Grid::default()
        };
        let class = b.read(2);
        let pointer;
        match class {
            0 => {
                g.num_env = 1 << b.read(2);
                if g.num_env > 4 {
                    return Err(malformed("more than 4 FIXFIX envelopes"));
                }
                let r = b.bit();
                g.freq_res = [r; MAX_ENV];
                if g.num_env == 1 {
                    g.amp_res = false;
                }
                for l in 0..=g.num_env {
                    g.t_e[l] = (l * usize::from(TIME_SLOTS) / g.num_env) as u8;
                }
                pointer = 0;
            }
            1 => {
                let var1 = b.read(2) as u8;
                let n_rel = b.read(2) as usize;
                g.num_env = n_rel + 1;
                let mut rel = [0u8; 3];
                for r in rel.iter_mut().take(n_rel) {
                    *r = 2 * b.read(2) as u8 + 2;
                }
                pointer = b.read(pointer_bits(g.num_env)) as usize;
                for l in 0..g.num_env {
                    g.freq_res[g.num_env - 1 - l] = b.bit();
                }
                g.t_e[g.num_env] = TIME_SLOTS + var1;
                for i in 0..n_rel {
                    let e = g.num_env - i;
                    g.t_e[e - 1] = g.t_e[e]
                        .checked_sub(rel[i])
                        .ok_or_else(|| malformed("envelope borders"))?;
                }
            }
            2 => {
                g.t_e[0] = b.read(2) as u8;
                let n_rel = b.read(2) as usize;
                g.num_env = n_rel + 1;
                for i in 0..n_rel {
                    g.t_e[i + 1] = g.t_e[i] + 2 * b.read(2) as u8 + 2;
                }
                pointer = b.read(pointer_bits(g.num_env)) as usize;
                for l in 0..g.num_env {
                    g.freq_res[l] = b.bit();
                }
                g.t_e[g.num_env] = TIME_SLOTS;
            }
            _ => {
                g.t_e[0] = b.read(2) as u8;
                let var1 = b.read(2) as u8;
                let n0 = b.read(2) as usize;
                let n1 = b.read(2) as usize;
                g.num_env = n0 + n1 + 1;
                if g.num_env > MAX_ENV {
                    return Err(malformed("more than 5 envelopes"));
                }
                for i in 0..n0 {
                    g.t_e[i + 1] = g.t_e[i] + 2 * b.read(2) as u8 + 2;
                }
                g.t_e[g.num_env] = TIME_SLOTS + var1;
                let mut rel = [0u8; 3];
                for r in rel.iter_mut().take(n1) {
                    *r = 2 * b.read(2) as u8 + 2;
                }
                for (i, &r) in rel.iter().take(n1).enumerate() {
                    let e = g.num_env - i;
                    g.t_e[e - 1] = g.t_e[e]
                        .checked_sub(r)
                        .ok_or_else(|| malformed("envelope borders"))?;
                }
                pointer = b.read(pointer_bits(g.num_env)) as usize;
                for l in 0..g.num_env {
                    g.freq_res[l] = b.bit();
                }
            }
        }
        let n = g.num_env;
        if (0..n).any(|l| g.t_e[l] >= g.t_e[l + 1]) || g.t_e[n] > TIME_SLOTS + 3 {
            return Err(malformed("envelope borders out of order"));
        }
        if pointer > n + 1 {
            return Err(malformed("bs_pointer beyond the envelopes"));
        }
        g.num_noise = if n > 1 { 2 } else { 1 };
        g.t_q[0] = g.t_e[0];
        if n > 1 {
            let mid = match class {
                0 => n / 2,
                2 => match pointer {
                    0 => 1,
                    1 => n - 1,
                    p => p - 1,
                },
                _ => {
                    if pointer > 1 {
                        n + 1 - pointer
                    } else {
                        n - 1
                    }
                }
            };
            g.t_q[1] = g.t_e[mid];
            g.t_q[2] = g.t_e[n];
        } else {
            g.t_q[1] = g.t_e[1];
        }
        g.l_a = match class {
            0 => -1,
            2 => {
                if pointer > 1 {
                    pointer as i32 - 1
                } else {
                    -1
                }
            }
            _ => {
                if pointer > 0 {
                    (n + 1 - pointer) as i32
                } else {
                    -1
                }
            }
        };
        Ok(g)
    }
}

/// The parsed SBR data of one channel for one frame.
#[derive(Clone)]
struct Frame {
    grid: Grid,
    df_env: [bool; MAX_ENV],
    df_noise: [bool; MAX_NOISE],
    invf: [u8; 5],
    /// Quantised envelope scalefactors (balance values for the second coupled channel).
    env: [[i32; 64]; MAX_ENV],
    noise: [[i32; 5]; MAX_NOISE],
    add_harmonic: Option<[bool; 64]>,
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            grid: Grid::default(),
            df_env: [false; MAX_ENV],
            df_noise: [false; MAX_NOISE],
            invf: [0; 5],
            env: [[0; 64]; MAX_ENV],
            noise: [[0; 5]; MAX_NOISE],
            add_harmonic: None,
        }
    }
}

/// The state of one channel kept from frame to frame.
struct Channel {
    analysis: qmf::Analysis,
    /// Low band subband samples, slot-major: 8 slots of the previous frame, then 32.
    x_low: Vec<[C; 32]>,
    x_high: Vec<[C; 64]>,
    /// HF-adjusted output for SBR slots 0..38.
    y: Vec<[C; 64]>,
    /// The part of the previous frame's output that falls in this frame: slots, kx, M.
    spill: Vec<[C; 64]>,
    spill_slots: usize,
    spill_kx: usize,
    spill_m: usize,
    env_prev: [i32; 64],
    env_prev_high: bool,
    noise_prev: [i32; 5],
    invf_prev: [u8; 5],
    bw_prev: [f32; 5],
    /// Gains and noise levels of the last four slots, for smoothing.
    g_hist: [[f32; 64]; 4],
    q_hist: [[f32; 64]; 4],
    hist_valid: bool,
    /// Sinusoid positions of the last envelope.
    sine_prev: [bool; 64],
    /// The previous frame's transient was at its end.
    trans_at_end: bool,
    noise_index: usize,
    sine_index: usize,
}

impl Channel {
    fn new() -> Self {
        Channel {
            analysis: qmf::Analysis::new(),
            x_low: vec![[C::default(); 32]; HF_GEN + SLOTS],
            x_high: vec![[C::default(); 64]; HF_GEN + SLOTS],
            y: vec![[C::default(); 64]; 2 * TIME_SLOTS as usize + 6],
            spill: vec![[C::default(); 64]; 6],
            spill_slots: 0,
            spill_kx: 32,
            spill_m: 0,
            env_prev: [0; 64],
            env_prev_high: false,
            noise_prev: [0; 5],
            invf_prev: [0; 5],
            bw_prev: [0.0; 5],
            g_hist: [[0.0; 64]; 4],
            q_hist: [[0.0; 64]; 4],
            hist_valid: false,
            sine_prev: [false; 64],
            trans_at_end: false,
            noise_index: 0,
            sine_index: 0,
        }
    }
}

/// Smoothing filter (§4.6.18.7.5).
const H_SMOOTH: [f32; 5] = [
    0.333_333_33,
    0.301_502_83,
    0.218_169_5,
    0.115_163_83,
    0.031_830_5,
];
const LIMITER_GAINS: [f32; 4] = [0.707_945_8, 1.0, 1.412_537_5, 1e10];

/// SBR of one channel element (SCE: one channel, CPE: two), with parametric stereo for an
/// SCE.
pub(crate) struct Sbr {
    /// The SBR sampling rate (twice the core rate).
    fs: u32,
    /// Synthesis at the core rate (32 bands).
    downsampled: bool,
    header: Option<Header>,
    bands: Option<Bands>,
    /// The tables changed with this frame's header.
    reset: bool,
    channels: Vec<Channel>,
    synth: Vec<qmf::Synthesis>,
    frames: [Frame; 2],
    coupling: bool,
    /// Data for this frame was read.
    have_data: bool,
    pub ps: Option<Box<Ps>>,
    /// PS data came with this frame.
    ps_now: bool,
    /// SBR data has come at least once.
    pub seen: bool,
    /// Output channels: those of the element, or 2 for an SCE with PS.
    out_channels: usize,
    /// Scratch: the QMF matrix of each output channel for one frame.
    x: Vec<Vec<[C; 64]>>,
}

impl Sbr {
    /// `core_rate` is the AAC core's rate; `channels` 1 or 2; `stereo_out` asks an SCE for
    /// two output channels (parametric stereo, or the mono signal twice).
    pub fn new(core_rate: u32, channels: usize, downsampled: bool, stereo_out: bool) -> Sbr {
        let out_channels = if channels == 1 && stereo_out {
            2
        } else {
            channels
        };
        let bands = if downsampled { 32 } else { 64 };
        Sbr {
            fs: 2 * core_rate,
            downsampled,
            header: None,
            bands: None,
            reset: false,
            channels: (0..channels).map(|_| Channel::new()).collect(),
            synth: (0..out_channels)
                .map(|_| qmf::Synthesis::new(bands))
                .collect(),
            frames: [Frame::default(), Frame::default()],
            coupling: false,
            have_data: false,
            ps: if out_channels == 2 && channels == 1 {
                Some(Box::new(Ps::new()))
            } else {
                None
            },
            ps_now: false,
            seen: false,
            out_channels,
            x: (0..out_channels)
                .map(|_| vec![[C::default(); 64]; SLOTS])
                .collect(),
        }
    }

    /// The last SBR header with its band tables, to carry over a seek.
    pub fn header(&self) -> Option<(Header, Bands)> {
        Some((self.header?, self.bands.clone()?))
    }

    /// Takes a header kept from before a seek.
    pub fn set_header(&mut self, h: Option<(Header, Bands)>) {
        if let Some((h, b)) = h {
            self.header = Some(h);
            self.bands = Some(b);
            self.reset = true;
        }
    }

    /// An SBR header has been read (so the SBR data, and any PS data in it, can be).
    pub fn header_seen(&self) -> bool {
        self.header.is_some()
    }

    /// Parametric stereo data has been read at least once.
    pub fn ps_seen(&self) -> bool {
        self.ps.as_ref().is_some_and(|p| p.seen())
    }

    pub fn out_channels(&self) -> usize {
        self.out_channels
    }

    /// Reads sbr_extension_data() of `bits` bits (the fill element's payload after its
    /// extension type). Errors leave the frame without SBR data.
    pub fn read(&mut self, b: &mut Bits, bits: usize, crc: bool) -> Result<()> {
        self.seen = true;
        let end = b.pos() + bits;
        let r = self.read_inner(b, crc);
        if b.pos() > end {
            self.have_data = false;
            b.skip(0);
            return Err(malformed("data runs past its fill element"));
        }
        b.skip(end - b.pos());
        if r.is_err() {
            self.have_data = false;
        }
        r
    }

    fn read_inner(&mut self, b: &mut Bits, crc: bool) -> Result<()> {
        if crc {
            b.read(10);
        }
        if b.bit() {
            let h = Header::read(b);
            let changed = self.header.is_none_or(|o| !o.same_bands(&h));
            self.header = Some(h);
            if changed {
                self.bands = Bands::new(&h, self.fs);
                self.reset = true;
                if self.bands.is_none() {
                    return Err(Error::Unsupported(
                        "SBR frequency tables out of range".into(),
                    ));
                }
            }
        }
        let (Some(h), Some(bands)) = (self.header, self.bands.clone()) else {
            return Ok(());
        };
        let books = books().ok_or_else(|| Error::Unsupported("SBR tables".into()))?;
        let stereo = self.channels.len() == 2;
        if b.bit() {
            b.read(if stereo { 8 } else { 4 });
        }
        let nq = bands.n_q();
        if stereo {
            self.coupling = b.bit();
            if self.coupling {
                self.frames[0].grid = Grid::read(b, h.amp_res)?;
                self.frames[1].grid = self.frames[0].grid;
                read_dtdf(b, &mut self.frames[0]);
                read_dtdf(b, &mut self.frames[1]);
                read_invf(b, &mut self.frames[0], nq);
                self.frames[1].invf = self.frames[0].invf;
                for ch in 0..2 {
                    let (f, c) = (&mut self.frames[ch], &self.channels[ch]);
                    read_envelope(b, f, c, &bands, books, ch == 1)?;
                    read_noise(b, f, c, &bands, books, ch == 1)?;
                }
            } else {
                for ch in 0..2 {
                    self.frames[ch].grid = Grid::read(b, h.amp_res)?;
                }
                for f in &mut self.frames {
                    read_dtdf(b, f);
                }
                for f in &mut self.frames {
                    read_invf(b, f, nq);
                }
                for ch in 0..2 {
                    read_envelope(
                        b,
                        &mut self.frames[ch],
                        &self.channels[ch],
                        &bands,
                        books,
                        false,
                    )?;
                }
                for ch in 0..2 {
                    read_noise(
                        b,
                        &mut self.frames[ch],
                        &self.channels[ch],
                        &bands,
                        books,
                        false,
                    )?;
                }
            }
            for f in &mut self.frames {
                read_harmonic(b, f, bands.n_high());
            }
        } else {
            self.coupling = false;
            let f = &mut self.frames[0];
            f.grid = Grid::read(b, h.amp_res)?;
            read_dtdf(b, f);
            read_invf(b, f, nq);
            read_envelope(b, f, &self.channels[0], &bands, books, false)?;
            read_noise(b, f, &self.channels[0], &bands, books, false)?;
            read_harmonic(b, f, bands.n_high());
        }
        self.ps_now = false;
        if b.bit() {
            let mut cnt = b.read(4) as usize;
            if cnt == 15 {
                cnt += b.read(8) as usize;
            }
            let mut left = 8 * cnt;
            while left > 7 {
                let id = b.read(2);
                left -= 2;
                let start = b.pos();
                if id == 2
                    && let Some(ps) = &mut self.ps
                {
                    let r = ps.read(b, left);
                    self.ps_now = r.is_ok();
                    let used = b.pos() - start;
                    if used > left {
                        return Err(malformed("parametric stereo data runs past its end"));
                    }
                    left -= used;
                } else {
                    b.skip(left);
                    left = 0;
                }
            }
            b.skip(left);
        }
        if b.overrun() {
            return Err(malformed("data runs past the frame"));
        }
        self.have_data = true;
        Ok(())
    }

    /// Decodes one frame: `pcm` holds the core output of each element channel (1024
    /// samples, 16-bit scale); `out` receives `frame_len()` samples per output channel.
    pub fn process(&mut self, pcm: &[&[f32]], out: &mut [&mut [f32]]) {
        let n_ch = self.channels.len();
        let data = self.have_data && self.bands.is_some() && self.header.is_some();
        self.have_data = false;
        let bands = if data { self.bands.clone() } else { None };
        let header = self.header;
        // QMF analysis into each channel's low-band buffer
        for (c, input) in self.channels.iter_mut().zip(pcm) {
            c.x_low.copy_within(SLOTS..SLOTS + HF_GEN, 0);
            let mut slot = [C::default(); 32];
            for l in 0..SLOTS {
                c.analysis.slot(&input[32 * l..32 * l + 32], &mut slot);
                c.x_low[HF_GEN + l] = slot;
            }
        }
        if let (Some(bands), Some(h)) = (&bands, header) {
            let (e_orig, q_orig) = dequantise(&self.frames, n_ch, self.coupling, bands);
            let reset = self.reset;
            for (ch, c) in self.channels.iter_mut().enumerate() {
                hf_generate(c, &self.frames[ch], bands);
                hf_adjust(
                    c,
                    &self.frames[ch],
                    &e_orig[ch],
                    &q_orig[ch],
                    bands,
                    &h,
                    reset,
                );
            }
            self.reset = false;
        }
        // assemble the QMF matrix of each channel
        for ch in 0..n_ch {
            let c = &mut self.channels[ch];
            let x = &mut self.x[ch];
            let (kx, m, end) = match (&bands, data) {
                (Some(b), true) => {
                    let g = &self.frames[ch].grid;
                    (b.kx, b.m, 2 * usize::from(g.t_e[g.num_env]))
                }
                _ => (32, 0, SLOTS),
            };
            for (s, row) in x.iter_mut().enumerate() {
                *row = [C::default(); 64];
                let (low_kx, hi) = if s < c.spill_slots {
                    (c.spill_kx, Some((&c.spill[s], c.spill_kx, c.spill_m)))
                } else {
                    (kx, Some((&c.y[s], kx, m)))
                };
                row[..low_kx].copy_from_slice(&c.x_low[s + HF_ADJ][..low_kx]);
                if let Some((src, k0, mm)) = hi {
                    row[k0..k0 + mm].copy_from_slice(&src[k0..k0 + mm]);
                }
            }
            // keep what falls in the next frame
            c.spill_slots = end.saturating_sub(SLOTS).min(6);
            for s in 0..c.spill_slots {
                c.spill[s] = c.y[SLOTS + s];
            }
            c.spill_kx = kx;
            c.spill_m = m;
        }
        // parametric stereo makes the second channel from the first
        if self.out_channels == 2 && n_ch == 1 {
            let (a, b) = self.x.split_at_mut(1);
            let ahead = &self.channels[0].x_low[SLOTS + HF_ADJ..];
            match (&mut self.ps, self.ps_now) {
                (Some(ps), true) => ps.process(&mut a[0], &mut b[0], ahead),
                (Some(ps), false) => ps.process_without_data(&mut a[0], &mut b[0], ahead),
                _ => b[0].copy_from_slice(&a[0]),
            }
        }
        self.ps_now = false;
        // synthesis
        let bands_out = if self.downsampled { 32 } else { 64 };
        for (ch, o) in out.iter_mut().enumerate().take(self.out_channels) {
            for s in 0..SLOTS {
                self.synth[ch].slot(&self.x[ch][s], &mut o[bands_out * s..bands_out * (s + 1)]);
            }
        }
    }
}

fn read_dtdf(b: &mut Bits, f: &mut Frame) {
    for l in 0..f.grid.num_env {
        f.df_env[l] = b.bit();
    }
    for n in 0..f.grid.num_noise {
        f.df_noise[n] = b.bit();
    }
}

fn read_invf(b: &mut Bits, f: &mut Frame, nq: usize) {
    for v in f.invf.iter_mut().take(nq) {
        *v = b.read(2) as u8;
    }
}

fn read_harmonic(b: &mut Bits, f: &mut Frame, n_high: usize) {
    f.add_harmonic = if b.bit() {
        let mut h = [false; 64];
        for v in h.iter_mut().take(n_high) {
            *v = b.bit();
        }
        Some(h)
    } else {
        None
    };
}

/// sbr_envelope(): `balance` for the second channel of a coupled pair.
fn read_envelope(
    b: &mut Bits,
    f: &mut Frame,
    c: &Channel,
    bands: &Bands,
    books: &Books,
    balance: bool,
) -> Result<()> {
    let amp = f.grid.amp_res;
    let (tb, fb, start_bits) = match (balance, amp) {
        (false, false) => (&books.env_1_5_t, &books.env_1_5_f, 7),
        (false, true) => (&books.env_3_0_t, &books.env_3_0_f, 6),
        (true, false) => (&books.bal_1_5_t, &books.bal_1_5_f, 6),
        (true, true) => (&books.bal_3_0_t, &books.bal_3_0_f, 5),
    };
    let step = if balance { 2 } else { 1 };
    for l in 0..f.grid.num_env {
        let high = f.grid.freq_res[l];
        let n = if high { bands.n_high() } else { bands.n_low() };
        if !f.df_env[l] {
            f.env[l][0] = step * b.read(start_bits) as i32;
            for k in 1..n {
                f.env[l][k] = f.env[l][k - 1] + step * delta(b, fb)?;
            }
        } else {
            let (prev, prev_high) = if l == 0 {
                (c.env_prev, c.env_prev_high)
            } else {
                (f.env[l - 1], f.grid.freq_res[l - 1])
            };
            for k in 0..n {
                let i = match (high, prev_high) {
                    (a, p) if a == p => k,
                    (true, false) => bands.high_from_low[k],
                    _ => bands.low_from_high[k],
                };
                f.env[l][k] = prev[i] + step * delta(b, tb)?;
            }
        }
    }
    Ok(())
}

/// sbr_noise().
fn read_noise(
    b: &mut Bits,
    f: &mut Frame,
    c: &Channel,
    bands: &Bands,
    books: &Books,
    balance: bool,
) -> Result<()> {
    let (tb, fb) = if balance {
        (&books.noise_bal_t, &books.bal_3_0_f)
    } else {
        (&books.noise_t, &books.env_3_0_f)
    };
    let step = if balance { 2 } else { 1 };
    let nq = bands.n_q();
    for n in 0..f.grid.num_noise {
        if !f.df_noise[n] {
            f.noise[n][0] = step * b.read(5) as i32;
            for k in 1..nq {
                f.noise[n][k] = f.noise[n][k - 1] + step * delta(b, fb)?;
            }
        } else {
            let prev = if n == 0 { c.noise_prev } else { f.noise[n - 1] };
            for k in 0..nq {
                f.noise[n][k] = prev[k] + step * delta(b, tb)?;
            }
        }
    }
    Ok(())
}

type EnvLevels = [[f32; 64]; MAX_ENV];
type NoiseLevels = [[f32; 5]; MAX_NOISE];

/// Envelope and noise floor levels of each channel (§4.6.18.3.5).
fn dequantise(
    frames: &[Frame; 2],
    n_ch: usize,
    coupling: bool,
    bands: &Bands,
) -> ([EnvLevels; 2], [NoiseLevels; 2]) {
    let mut e = [[[0f32; 64]; MAX_ENV]; 2];
    let mut q = [[[0f32; 5]; MAX_NOISE]; 2];
    let nq = bands.n_q();
    if coupling && n_ch == 2 {
        let g = &frames[0].grid;
        let alpha = if g.amp_res { 1.0 } else { 0.5 };
        let pan = if g.amp_res { 12.0 } else { 24.0 };
        for l in 0..g.num_env {
            let n = if g.freq_res[l] {
                bands.n_high()
            } else {
                bands.n_low()
            };
            for k in 0..n {
                let t1 = (frames[0].env[l][k] as f32 * alpha + 7.0).exp2();
                let t2 = ((pan - frames[1].env[l][k] as f32) * alpha).exp2();
                let left = t1 / (1.0 + t2);
                e[0][l][k] = left;
                e[1][l][k] = left * t2;
            }
        }
        for n in 0..g.num_noise {
            for k in 0..nq {
                let t1 = (7.0 - frames[0].noise[n][k] as f32).exp2();
                let t2 = (12.0 - frames[1].noise[n][k] as f32).exp2();
                let left = t1 / (1.0 + t2);
                q[0][n][k] = left;
                q[1][n][k] = left * t2;
            }
        }
    } else {
        for ch in 0..n_ch {
            let g = &frames[ch].grid;
            let alpha = if g.amp_res { 1.0 } else { 0.5 };
            for l in 0..g.num_env {
                let n = if g.freq_res[l] {
                    bands.n_high()
                } else {
                    bands.n_low()
                };
                for k in 0..n {
                    e[ch][l][k] = (frames[ch].env[l][k] as f32 * alpha + 6.0).exp2();
                }
            }
            for n in 0..g.num_noise {
                for k in 0..nq {
                    q[ch][n][k] = (6.0 - frames[ch].noise[n][k] as f32).exp2();
                }
            }
        }
    }
    (e, q)
}

/// Linear prediction coefficients of low band `p` from its covariance (§4.6.18.6.2).
fn lpc(x: &[[C; 32]], p: usize) -> (C, C) {
    // φ(i, j) = Σ_n X(n + 2 − i)·X*(n + 2 − j), n = 0..37
    let (mut r01, mut r02, mut r11, mut r12, mut r22) =
        (C::default(), C::default(), 0f32, C::default(), 0f32);
    let mul_conj = |a: C, b: C| C::new(a.re * b.re + a.im * b.im, a.im * b.re - a.re * b.im);
    for n in 0..38 {
        let (x0, x1, x2) = (x[n + 2][p], x[n + 1][p], x[n][p]);
        r01 = r01.add(mul_conj(x0, x1));
        r02 = r02.add(mul_conj(x0, x2));
        r11 += x1.re * x1.re + x1.im * x1.im;
        r12 = r12.add(mul_conj(x1, x2));
        r22 += x2.re * x2.re + x2.im * x2.im;
    }
    let d = r22 * r11 - (r12.re * r12.re + r12.im * r12.im) / (1.0 + 1e-6);
    let a1 = if d != 0.0 {
        let n = r01.mul(r12).sub(C::new(r02.re * r11, r02.im * r11));
        C::new(n.re / d, n.im / d)
    } else {
        C::default()
    };
    let a0 = if r11 != 0.0 {
        let t = r01.add(mul_conj(a1, r12));
        C::new(-t.re / r11, -t.im / r11)
    } else {
        C::default()
    };
    let big = |c: C| c.re * c.re + c.im * c.im >= 16.0;
    if big(a0) || big(a1) {
        (C::default(), C::default())
    } else {
        (a0, a1)
    }
}

/// The chirp factor of inverse filtering level `mode` after `prev` (§4.6.18.6.2).
fn new_bw(mode: u8, prev: u8) -> f32 {
    match (mode, prev) {
        (0, 1) => 0.6,
        (0, _) => 0.0,
        (1, 0) => 0.6,
        (1, _) => 0.75,
        (2, _) => 0.9,
        _ => 0.98,
    }
}

/// HF generation: the patched, inverse-filtered high band (§4.6.18.6).
fn hf_generate(c: &mut Channel, f: &Frame, bands: &Bands) {
    let g = &f.grid;
    let (start, end) = (2 * usize::from(g.t_e[0]), 2 * usize::from(g.t_e[g.num_env]));
    let nq = bands.n_q();
    let mut bw = [0f32; 5];
    for i in 0..nq {
        let nb = new_bw(f.invf[i], c.invf_prev[i]);
        let prev = c.bw_prev[i];
        let t = if nb < prev {
            0.75 * nb + 0.25 * prev
        } else {
            0.906_25 * nb + 0.093_75 * prev
        };
        bw[i] = if t < 0.015_625 {
            0.0
        } else {
            t.min(0.996_093_75)
        };
        c.bw_prev[i] = bw[i];
        c.invf_prev[i] = f.invf[i];
    }
    let mut alpha = [(C::default(), C::default()); 32];
    let lowest = bands.patches.iter().map(|p| p.0).min().unwrap_or(0);
    for (p, a) in alpha.iter_mut().enumerate().take(bands.kx).skip(lowest) {
        *a = lpc(&c.x_low, p);
    }
    for row in &mut c.x_high[start + HF_ADJ..end + HF_ADJ] {
        *row = [C::default(); 64];
    }
    let mut k = bands.kx;
    let mut gi = 0;
    for &(first, len) in &bands.patches {
        for x in 0..len {
            let p = first + x;
            while gi + 1 < nq && k >= bands.f_noise[gi + 1] {
                gi += 1;
            }
            let b = bw[gi];
            let (a0, a1) = alpha[p];
            let c0 = C::new(a0.re * b, a0.im * b);
            let c1 = C::new(a1.re * b * b, a1.im * b * b);
            for s in start..end {
                let i = s + HF_ADJ;
                let v = c.x_low[i][p]
                    .add(c0.mul(c.x_low[i - 1][p]))
                    .add(c1.mul(c.x_low[i - 2][p]));
                c.x_high[i][k] = v;
            }
            k += 1;
        }
    }
}

/// HF adjustment: maps the envelope and noise data to QMF bands, computes the limited gains
/// and writes the adjusted high band with noise and sinusoids into `c.y` (§4.6.18.7).
fn hf_adjust(
    c: &mut Channel,
    f: &Frame,
    e_orig: &EnvLevels,
    q_orig: &NoiseLevels,
    bands: &Bands,
    h: &Header,
    reset: bool,
) {
    let g = &f.grid;
    let (kx, m) = (bands.kx, bands.m);
    let n_env = g.num_env;
    let mut e_map = [[0f32; 64]; MAX_ENV];
    let mut q_map = [[0f32; 64]; MAX_ENV];
    let mut s_idx = [[false; 64]; MAX_ENV];
    let mut s_band = [[false; 64]; MAX_ENV];
    let mut e_curr = [[0f32; 64]; MAX_ENV];
    for l in 0..n_env {
        let table = bands.table(g.freq_res[l]);
        for i in 0..table.len() - 1 {
            for k in table[i]..table[i + 1] {
                e_map[l][k - kx] = e_orig[l][i];
            }
        }
        let qn = usize::from(g.num_noise > 1 && g.t_e[l] >= g.t_q[1]);
        for i in 0..bands.n_q() {
            for k in bands.f_noise[i]..bands.f_noise[i + 1] {
                q_map[l][k - kx] = q_orig[qn][i];
            }
        }
        if let Some(harm) = &f.add_harmonic {
            for i in 0..bands.n_high() {
                let mid = (bands.f_high[i] + bands.f_high[i + 1]) / 2 - kx;
                s_idx[l][mid] = harm[i] && (l as i32 >= g.l_a || c.sine_prev[mid]);
            }
        }
        for i in 0..table.len() - 1 {
            let any = (table[i]..table[i + 1]).any(|k| s_idx[l][k - kx]);
            for k in table[i]..table[i + 1] {
                s_band[l][k - kx] = any;
            }
        }
        // current energy of the patched band
        let (lo, hi) = (2 * usize::from(g.t_e[l]), 2 * usize::from(g.t_e[l + 1]));
        let width = (hi - lo) as f32;
        let energy = |k: usize| -> f32 {
            c.x_high[lo + HF_ADJ..hi + HF_ADJ]
                .iter()
                .map(|r| r[k].re * r[k].re + r[k].im * r[k].im)
                .sum()
        };
        if h.interpol_freq {
            for (mm, e) in e_curr[l].iter_mut().enumerate().take(m) {
                *e = energy(kx + mm) / width;
            }
        } else {
            for i in 0..table.len() - 1 {
                let sum: f32 = (table[i]..table[i + 1]).map(energy).sum();
                let v = sum / (width * (table[i + 1] - table[i]) as f32);
                for k in table[i]..table[i + 1] {
                    e_curr[l][k - kx] = v;
                }
            }
        }
    }
    if n_env > 0 {
        c.sine_prev = [false; 64];
        c.sine_prev[..m].copy_from_slice(&s_idx[n_env - 1][..m]);
    }
    // gains with the limiter and its compensation
    let mut gain = [[0f32; 64]; MAX_ENV];
    let mut q_m = [[0f32; 64]; MAX_ENV];
    let mut s_m = [[0f32; 64]; MAX_ENV];
    let lim_gain = LIMITER_GAINS[usize::from(h.limiter_gains)];
    let trans_at_end = c.trans_at_end;
    let transient = |l: usize| l as i32 == g.l_a || (l == 0 && trans_at_end);
    const EPS: f32 = 1e-12;
    for l in 0..n_env {
        let delta = if transient(l) { 0.0 } else { 1.0 };
        for mm in 0..m {
            let (e, q, ec) = (e_map[l][mm], q_map[l][mm], e_curr[l][mm]);
            q_m[l][mm] = (e * q / (1.0 + q)).sqrt();
            s_m[l][mm] = if s_idx[l][mm] {
                (e / (1.0 + q)).sqrt()
            } else {
                0.0
            };
            gain[l][mm] = if s_band[l][mm] {
                (e * q / ((1.0 + ec) * (1.0 + q))).sqrt()
            } else {
                (e / ((1.0 + ec) * (1.0 + q * delta))).sqrt()
            };
        }
        for j in 0..bands.f_lim.len() - 1 {
            let (a, b) = (bands.f_lim[j] - kx, bands.f_lim[j + 1] - kx);
            let so: f32 = e_map[l][a..b].iter().sum();
            let sc: f32 = e_curr[l][a..b].iter().sum();
            let g_max = (lim_gain * ((EPS + so) / (EPS + sc)).sqrt()).min(1e5);
            for mm in a..b {
                if gain[l][mm] > g_max {
                    q_m[l][mm] *= g_max / gain[l][mm];
                    gain[l][mm] = g_max;
                }
            }
            let mut den = 0f32;
            for mm in a..b {
                den += e_curr[l][mm] * gain[l][mm] * gain[l][mm] + s_m[l][mm] * s_m[l][mm];
                if delta != 0.0 && s_m[l][mm] == 0.0 {
                    den += q_m[l][mm] * q_m[l][mm];
                }
            }
            let boost = ((EPS + so) / (EPS + den)).sqrt().min(1.584_893_2);
            for mm in a..b {
                gain[l][mm] *= boost;
                q_m[l][mm] *= boost;
                s_m[l][mm] *= boost;
            }
        }
    }
    // assembly, slot by slot
    let start = 2 * usize::from(g.t_e[0]);
    let end = 2 * usize::from(g.t_e[n_env]);
    let smooth = !h.smoothing_mode;
    let mut g_temp = vec![[0f32; 64]; 4 + end - start];
    let mut q_temp = vec![[0f32; 64]; 4 + end - start];
    if reset || !c.hist_valid {
        for i in 0..4 {
            g_temp[i] = gain[0];
            q_temp[i] = q_m[0];
        }
    } else {
        g_temp[..4].copy_from_slice(&c.g_hist);
        q_temp[..4].copy_from_slice(&c.q_hist);
    }
    for l in 0..n_env {
        for s in 2 * usize::from(g.t_e[l])..2 * usize::from(g.t_e[l + 1]) {
            g_temp[4 + s - start] = gain[l];
            q_temp[4 + s - start] = q_m[l];
        }
    }
    for row in &mut c.y[..end.max(SLOTS)] {
        *row = [C::default(); 64];
    }
    for l in 0..n_env {
        let tr = transient(l);
        for s in 2 * usize::from(g.t_e[l])..2 * usize::from(g.t_e[l + 1]) {
            let t = 4 + s - start;
            let xh = &c.x_high[s + HF_ADJ];
            let y = &mut c.y[s];
            let (re_s, im_s) = match c.sine_index {
                0 => (1.0, 0.0),
                1 => (0.0, 1.0),
                2 => (-1.0, 0.0),
                _ => (0.0, -1.0),
            };
            let mut im_sign: f32 = if kx % 2 == 0 { 1.0 } else { -1.0 };
            for mm in 0..m {
                let (gf, qf) = if smooth && !tr {
                    let (mut a, mut b) = (0f32, 0f32);
                    for (j, hj) in H_SMOOTH.iter().enumerate() {
                        a += g_temp[t - j][mm] * hj;
                        b += q_temp[t - j][mm] * hj;
                    }
                    (a, b)
                } else {
                    (g_temp[t][mm], q_temp[t][mm])
                };
                let k = kx + mm;
                let mut v = C::new(xh[k].re * gf, xh[k].im * gf);
                let sm = s_m[l][mm];
                if sm != 0.0 {
                    v.re += sm * re_s;
                    v.im += sm * im_s * im_sign;
                } else if !tr {
                    let nz = tab::NOISE[(c.noise_index + mm + 1) & 511];
                    v.re += qf * nz[0];
                    v.im += qf * nz[1];
                }
                y[k] = v;
                im_sign = -im_sign;
            }
            c.noise_index = (c.noise_index + m) & 511;
            c.sine_index = (c.sine_index + 1) & 3;
        }
    }
    let n = g_temp.len();
    c.g_hist.copy_from_slice(&g_temp[n - 4..]);
    c.q_hist.copy_from_slice(&q_temp[n - 4..]);
    c.hist_valid = true;
    c.trans_at_end = g.l_a == n_env as i32;
    if n_env > 0 {
        c.env_prev = f.env[n_env - 1];
        c.env_prev_high = g.freq_res[n_env - 1];
        c.noise_prev = f.noise[g.num_noise - 1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_build() {
        assert!(books().is_some());
    }

    /// The stop frequencies match PacketVideo's precomputed table (`sbr_stopfreq_tbl`, by
    /// SBR rate, `bs_stop_freq` 0..=12).
    #[test]
    fn stop_frequencies() {
        let want: [(u32, [i32; 13]); 6] = [
            (48000, [21, 23, 25, 27, 29, 32, 35, 38, 41, 45, 49, 54, 59]),
            (44100, [23, 25, 27, 29, 31, 34, 37, 40, 43, 47, 51, 55, 59]),
            (32000, [32, 34, 36, 38, 40, 42, 44, 46, 49, 52, 55, 58, 61]),
            (24000, [32, 34, 36, 38, 40, 42, 44, 46, 49, 52, 55, 58, 61]),
            (22050, [35, 36, 38, 40, 42, 44, 46, 48, 50, 52, 55, 58, 61]),
            (16000, [48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 60, 62]),
        ];
        for (fs, row) in want {
            let hz = if fs < 32000 { 6000.0 } else { 8000.0 };
            let stop_min = nint(hz * 128.0 / f64::from(fs));
            for (s, &k2) in row.iter().enumerate() {
                assert_eq!(stop_band(stop_min, s), k2, "{fs} {s}");
            }
        }
    }

    #[test]
    fn typical_tables() {
        // a common 44.1 kHz HE-AAC header: start 5, stop 9, xover 0, default extras
        let h = Header {
            amp_res: true,
            start_freq: 5,
            stop_freq: 9,
            xover_band: 0,
            freq_scale: 2,
            alter_scale: true,
            noise_bands: 2,
            limiter_bands: 2,
            limiter_gains: 2,
            interpol_freq: true,
            smoothing_mode: true,
        };
        let b = Bands::new(&h, 44100).unwrap();
        assert!(b.kx > 0 && b.kx <= 32 && b.kx + b.m <= 64);
        assert_eq!(b.f_low[0], b.kx);
        assert_eq!(*b.f_low.last().unwrap(), b.kx + b.m);
        assert!(b.n_q() >= 1 && b.n_q() <= 5);
        let total: usize = b.patches.iter().map(|p| p.1).sum();
        assert_eq!(total, b.m);
        assert!(b.f_lim.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn pointer_widths() {
        assert_eq!(
            (1..=5).map(pointer_bits).collect::<Vec<_>>(),
            vec![1, 2, 2, 3, 3]
        );
    }
}
