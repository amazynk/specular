//! individual_channel_stream (§4.4.2.7): ics_info (with AAC Main prediction and long-term
//! prediction side information), section data, scalefactors, pulse data, TNS data and
//! spectral data, then inverse quantisation and noise substitution, plus the joint-stereo
//! tools, TNS filtering and AAC Main's backward-adaptive prediction applied on top.

use crate::bits::Bits;
use crate::filterbank::Sequence;
use crate::tables::{self, Books};
use crate::{Error, Result};

pub(crate) const ZERO_HCB: u8 = 0;
pub(crate) const NOISE_HCB: u8 = 13;
pub(crate) const INTENSITY_HCB2: u8 = 14;
pub(crate) const INTENSITY_HCB: u8 = 15;

/// Object types of the core coders decoded.
pub(crate) const AOT_MAIN: u8 = 1;
pub(crate) const AOT_LTP: u8 = 4;

fn malformed(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// What the channel streams of a stream are coded with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Core {
    /// Sampling frequency index of the band tables.
    pub rate_index: usize,
    /// Samples per frame: 1024, or 960.
    pub frame_len: usize,
    /// Audio object type: 1 Main, 2 LC, 4 LTP.
    pub object_type: u8,
}

/// AAC Main prediction side information of a long-window frame.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MainPred {
    /// `predictor_reset_group_number` (1–30), 0 for no reset.
    pub reset_group: u8,
    /// `prediction_used`, bit per scalefactor band.
    pub used: u64,
}

/// Long-term prediction side information (`ltp_data`, long windows).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Ltp {
    pub lag: usize,
    pub coef: f32,
    /// `ltp_long_used`, bit per scalefactor band.
    pub used: u64,
}

impl Ltp {
    fn read(b: &mut Bits, max_sfb: usize) -> Ltp {
        let lag = b.read(11) as usize;
        let coef = tables::LTP_COEF[b.read(3) as usize];
        let mut used = 0u64;
        for s in 0..max_sfb.min(tables::LTP_LONG_SFB) {
            if b.bit() {
                used |= 1 << s;
            }
        }
        Ltp { lag, coef, used }
    }
}

/// ics_info.
#[derive(Debug, Clone, Default)]
pub(crate) struct IcsInfo {
    pub seq: Sequence,
    pub shape: usize,
    pub max_sfb: usize,
    pub num_windows: usize,
    /// Windows in each group.
    pub group_len: Vec<usize>,
    /// Band offsets (in coefficients of one window), `num_swb + 1` entries.
    pub offsets: &'static [u16],
    /// Coefficients of one window (frame length, or an eighth of it for short windows).
    pub win_len: usize,
    /// `predictor_data_present`.
    pub predictor: bool,
    /// AAC Main prediction data.
    pub pred: Option<MainPred>,
    /// Long-term prediction data of this channel.
    pub ltp: Option<Ltp>,
}

impl IcsInfo {
    pub fn read(b: &mut Bits, core: &Core) -> Result<IcsInfo> {
        if b.bit() {
            return Err(malformed("reserved ics_info bit set"));
        }
        let seq = Sequence::from_bits(b.read(2));
        let shape = b.read(1) as usize;
        let mut info = IcsInfo {
            seq,
            shape,
            ..IcsInfo::default()
        };
        let ri = core.rate_index;
        if seq == Sequence::EightShort {
            info.max_sfb = b.read(4) as usize;
            let grouping = b.read(7);
            info.num_windows = 8;
            info.group_len = vec![1];
            for w in 0..7 {
                if grouping & (1 << (6 - w)) != 0 {
                    *info.group_len.last_mut().unwrap_or(&mut 0) += 1;
                } else {
                    info.group_len.push(1);
                }
            }
            info.offsets = tables::swb_short(ri, core.frame_len);
            info.win_len = core.frame_len / 8;
        } else {
            info.max_sfb = b.read(6) as usize;
            info.num_windows = 1;
            info.group_len = vec![1];
            info.offsets = tables::swb_long(ri, core.frame_len);
            info.win_len = core.frame_len;
            info.predictor = b.bit();
            if info.predictor {
                match core.object_type {
                    AOT_MAIN => {
                        let mut p = MainPred::default();
                        if b.bit() {
                            p.reset_group = b.read(5) as u8;
                            if !(1..=30).contains(&p.reset_group) {
                                return Err(malformed("invalid predictor reset group"));
                            }
                        }
                        let n = info.max_sfb.min(usize::from(tables::PRED_SFB_MAX[ri]));
                        for s in 0..n {
                            if b.bit() {
                                p.used |= 1 << s;
                            }
                        }
                        info.pred = Some(p);
                    }
                    AOT_LTP => {
                        if b.bit() {
                            info.ltp = Some(Ltp::read(b, info.max_sfb));
                        }
                    }
                    _ => {
                        return Err(Error::Unsupported(
                            "AAC Main prediction in an AAC-LC stream".into(),
                        ));
                    }
                }
            }
        }
        if info.offsets.is_empty() {
            return Err(Error::Unsupported(
                "scalefactor band tables are not available".into(),
            ));
        }
        if info.max_sfb + 1 > info.offsets.len() {
            return Err(malformed("max_sfb beyond the scalefactor bands"));
        }
        Ok(info)
    }

    /// The second channel's `ltp_data` of a channel pair sharing this ics_info.
    pub fn read_second_ltp(&self, b: &mut Bits, core: &Core) -> Option<Ltp> {
        (self.predictor && core.object_type == AOT_LTP && b.bit())
            .then(|| Ltp::read(b, self.max_sfb))
    }

    pub fn window_len(&self) -> usize {
        self.win_len
    }
}

/// The state of one AAC Main predictor (one spectral line): a second-order backward-
/// adaptive lattice LMS predictor (§4.6.7), its values held to a 16-bit mantissa.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Predictor {
    r0: f32,
    r1: f32,
    cor0: f32,
    cor1: f32,
    var0: f32,
    var1: f32,
}

impl Default for Predictor {
    fn default() -> Self {
        Predictor {
            r0: 0.0,
            r1: 0.0,
            cor0: 0.0,
            cor1: 0.0,
            var0: 1.0,
            var1: 1.0,
        }
    }
}

/// Rounds to the nearest 16-bit-mantissa float (ties away from zero).
fn round16(x: f32) -> f32 {
    f32::from_bits(x.to_bits().wrapping_add(0x8000) & 0xffff_0000)
}

/// Rounds to the nearest 16-bit-mantissa float (ties to even).
fn even16(x: f32) -> f32 {
    let b = x.to_bits();
    f32::from_bits(b.wrapping_add(0x7fff + ((b >> 16) & 1)) & 0xffff_0000)
}

/// Truncates to a 16-bit-mantissa float.
fn trunc16(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0xffff_0000)
}

impl Predictor {
    /// Predicts line `x` from the past (adding the estimate when `apply`), then adapts to the
    /// reconstructed value.
    fn run(&mut self, x: &mut f32, apply: bool) {
        const A: f32 = 0.953_125; // 61/64
        const ALPHA: f32 = 0.906_25; // 29/32
        let (r0, r1) = (self.r0, self.r1);
        let k1 = if self.var0 > 1.0 {
            self.cor0 * even16(A / self.var0)
        } else {
            0.0
        };
        let k2 = if self.var1 > 1.0 {
            self.cor1 * even16(A / self.var1)
        } else {
            0.0
        };
        if apply {
            *x += round16(k1 * r0 + k2 * r1);
        }
        let e0 = *x;
        let e1 = e0 - k1 * r0;
        self.cor1 = trunc16(ALPHA * self.cor1 + r1 * e1);
        self.var1 = trunc16(ALPHA * self.var1 + 0.5 * (r1 * r1 + e1 * e1));
        self.cor0 = trunc16(ALPHA * self.cor0 + r0 * e0);
        self.var0 = trunc16(ALPHA * self.var0 + 0.5 * (r0 * r0 + e0 * e0));
        self.r1 = trunc16(A * (r0 - k1 * e0));
        self.r0 = trunc16(A * e0);
    }
}

/// One TNS filter.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TnsFilter {
    pub length: usize,
    pub order: usize,
    pub downward: bool,
    pub coef: [f32; 20],
}

/// TNS data of one channel: the filters of each window.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tns {
    pub present: bool,
    pub filters: [Vec<TnsFilter>; 8],
}

/// The decoded state of one channel of one element.
#[derive(Clone)]
pub(crate) struct Channel {
    pub info: IcsInfo,
    /// Codebook per (group, band), groups × 64.
    pub cb: [u8; 8 * 64],
    /// Scalefactor, noise energy or intensity position per (group, band).
    pub sf: [i32; 8 * 64],
    /// Dequantised coefficients, windows de-interleaved (window w at w·128).
    pub spec: Vec<f32>,
    pub tns: Tns,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            info: IcsInfo::default(),
            cb: [0; 512],
            sf: [0; 512],
            spec: vec![0.0; 1024],
            tns: Tns::default(),
        }
    }
}

/// The parameters of spectral codebook `cb` (1–11): (dimension, signed, modulus, offset).
fn book_params(cb: u8) -> (usize, bool, i32, i32) {
    match cb {
        1 | 2 => (4, true, 3, 1),
        3 | 4 => (4, false, 3, 0),
        5 | 6 => (2, true, 9, 4),
        7 | 8 => (2, false, 8, 0),
        9 | 10 => (2, false, 13, 0),
        _ => (2, false, 17, 0),
    }
}

/// |q|^(4/3), from a table for the values codebook 11's escapes reach (up to 8191).
fn pow43(q: u32) -> f32 {
    static T: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let t = T.get_or_init(|| {
        (0..8192u32)
            .map(|v| (f64::from(v)).powf(4.0 / 3.0) as f32)
            .collect()
    });
    t.get(q as usize)
        .copied()
        .unwrap_or_else(|| (f64::from(q)).powf(4.0 / 3.0) as f32)
}

/// A deterministic noise source for PNS (a 32-bit linear congruential generator).
pub(crate) struct Lcg(pub u32);

impl Lcg {
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 as i32) as f32
    }
}

impl Channel {
    /// Reads an individual_channel_stream. `common` carries the ics_info shared by a CPE.
    pub fn read(
        &mut self,
        b: &mut Bits,
        books: &Books,
        core: &Core,
        common: Option<&IcsInfo>,
        noise: &mut Lcg,
    ) -> Result<()> {
        let global_gain = b.read(8) as i32;
        self.info = match common {
            Some(i) => i.clone(),
            None => IcsInfo::read(b, core)?,
        };
        self.read_sections(b)?;
        self.read_scalefactors(b, books, global_gain)?;
        let pulse = if b.bit() {
            if self.info.seq == Sequence::EightShort {
                return Err(malformed("pulse data in a short-window frame"));
            }
            Some(self.read_pulse(b, core.frame_len)?)
        } else {
            None
        };
        self.tns = Tns::default();
        if b.bit() {
            self.read_tns(b, core.object_type)?;
        }
        if b.bit() {
            return Err(Error::Unsupported("AAC-SSR gain control".into()));
        }
        self.read_spectral(b, books, pulse.as_ref(), noise)?;
        if b.overrun() {
            return Err(malformed("channel stream runs past the end of the frame"));
        }
        Ok(())
    }

    fn read_sections(&mut self, b: &mut Bits) -> Result<()> {
        let short = self.info.seq == Sequence::EightShort;
        let (bits, esc) = if short { (3, 7) } else { (5, 31) };
        let max_sfb = self.info.max_sfb;
        self.cb = [0; 512];
        for g in 0..self.info.group_len.len() {
            let mut k = 0;
            while k < max_sfb {
                let cb = b.read(4) as u8;
                if cb == 12 {
                    return Err(malformed("reserved codebook 12"));
                }
                let mut len = 0;
                loop {
                    let inc = b.read(bits) as usize;
                    len += inc;
                    if inc != esc {
                        break;
                    }
                    if b.overrun() {
                        return Err(malformed("section data truncated"));
                    }
                }
                if len == 0 || k + len > max_sfb {
                    return Err(malformed("empty section or one past max_sfb"));
                }
                for s in k..k + len {
                    self.cb[g * 64 + s] = cb;
                }
                k += len;
            }
        }
        Ok(())
    }

    fn read_scalefactors(&mut self, b: &mut Bits, books: &Books, global_gain: i32) -> Result<()> {
        let mut gain = global_gain;
        let mut noise = global_gain - 90;
        let mut is_pos = 0;
        let mut first_noise = true;
        self.sf = [0; 512];
        for g in 0..self.info.group_len.len() {
            for s in 0..self.info.max_sfb {
                let i = g * 64 + s;
                let delta = |b: &mut Bits| -> Result<i32> {
                    books
                        .scalefactor
                        .read(b)
                        .map(|v| v as i32 - 60)
                        .ok_or_else(|| malformed("invalid scalefactor code"))
                };
                match self.cb[i] {
                    ZERO_HCB => {}
                    INTENSITY_HCB | INTENSITY_HCB2 => {
                        is_pos += delta(b)?;
                        self.sf[i] = is_pos;
                    }
                    NOISE_HCB => {
                        if first_noise {
                            noise += b.read(9) as i32 - 256;
                            first_noise = false;
                        } else {
                            noise += delta(b)?;
                        }
                        self.sf[i] = noise;
                    }
                    _ => {
                        gain += delta(b)?;
                        if !(0..256).contains(&gain) {
                            return Err(malformed("scalefactor out of range"));
                        }
                        self.sf[i] = gain;
                    }
                }
            }
        }
        Ok(())
    }

    fn read_pulse(&self, b: &mut Bits, frame_len: usize) -> Result<Vec<(usize, i32)>> {
        let n = b.read(2) as usize + 1;
        let start = b.read(6) as usize;
        if start >= self.info.offsets.len().saturating_sub(1) {
            return Err(malformed("pulse start band out of range"));
        }
        let mut k = usize::from(self.info.offsets[start]);
        let mut pulses = Vec::with_capacity(n);
        for _ in 0..n {
            k += b.read(5) as usize;
            let amp = b.read(4) as i32;
            if k >= frame_len {
                return Err(malformed("pulse beyond the spectrum"));
            }
            pulses.push((k, amp));
        }
        Ok(pulses)
    }

    fn read_tns(&mut self, b: &mut Bits, object_type: u8) -> Result<()> {
        let short = self.info.seq == Sequence::EightShort;
        // TNS_MAX_ORDER: 20 for AAC Main long windows, else 12 (7 for short windows)
        let long_order = if object_type == AOT_MAIN { 20 } else { 12 };
        let (filt_bits, len_bits, order_bits, max_order) = if short {
            (1, 4, 3, 7)
        } else {
            (2, 6, 5, long_order)
        };
        self.tns.present = true;
        for w in 0..self.info.num_windows {
            let n_filt = b.read(filt_bits);
            if n_filt == 0 {
                continue;
            }
            let coef_res = b.read(1);
            for _ in 0..n_filt {
                let mut f = TnsFilter {
                    length: b.read(len_bits) as usize,
                    order: b.read(order_bits) as usize,
                    ..TnsFilter::default()
                };
                if f.order > max_order {
                    return Err(malformed("TNS order above the profile's limit"));
                }
                if f.order > 0 {
                    f.downward = b.bit();
                    let compress = b.read(1);
                    let res = coef_res + 3;
                    let width = res - compress;
                    let iq_pos = ((1u32 << (res - 1)) as f32 - 0.5) / std::f32::consts::FRAC_PI_2;
                    let iq_neg = ((1u32 << (res - 1)) as f32 + 0.5) / std::f32::consts::FRAC_PI_2;
                    let mut refl = [0f32; 20];
                    for r in refl.iter_mut().take(f.order) {
                        let raw = b.read(width) as i32;
                        let v = (raw << (32 - width)) >> (32 - width);
                        *r = if v >= 0 {
                            (v as f32 / iq_pos).sin()
                        } else {
                            (v as f32 / iq_neg).sin()
                        };
                    }
                    // reflection coefficients to a direct-form LPC (a[0] = 1 implied)
                    let mut a = [0f32; 20];
                    for m in 0..f.order {
                        let mut next = a;
                        for i in 0..m {
                            next[i] = a[i] + refl[m] * a[m - 1 - i];
                        }
                        next[m] = refl[m];
                        a = next;
                    }
                    f.coef = a;
                }
                self.tns.filters[w].push(f);
            }
        }
        Ok(())
    }

    fn read_spectral(
        &mut self,
        b: &mut Bits,
        books: &Books,
        pulse: Option<&Vec<(usize, i32)>>,
        noise: &mut Lcg,
    ) -> Result<()> {
        let mut quant = [0i32; 1024];
        let offsets = self.info.offsets;
        let win_len = self.info.window_len();
        // read quantised values, window-major
        let mut win = 0;
        for (g, &glen) in self.info.group_len.iter().enumerate() {
            for s in 0..self.info.max_sfb {
                let cb = self.cb[g * 64 + s];
                if cb == ZERO_HCB || cb >= NOISE_HCB {
                    continue;
                }
                let (dim, signed, modulus, off) = book_params(cb);
                let book = &books.spectral[usize::from(cb) - 1];
                let lo = usize::from(offsets[s]);
                let hi = usize::from(offsets[s + 1]);
                for w in win..win + glen {
                    let base = w * win_len;
                    let mut k = lo;
                    while k < hi {
                        let mut idx = book
                            .read(b)
                            .ok_or_else(|| malformed("invalid spectral code"))?
                            as i32;
                        let mut vals = [0i32; 4];
                        for d in (0..dim).rev() {
                            vals[d] = idx % modulus - off;
                            idx /= modulus;
                        }
                        if !signed {
                            for v in vals.iter_mut().take(dim) {
                                if *v != 0 && b.bit() {
                                    *v = -*v;
                                }
                            }
                        }
                        if cb == 11 {
                            for v in vals.iter_mut().take(dim) {
                                if v.abs() == 16 {
                                    let mut n = 0;
                                    while b.bit() {
                                        n += 1;
                                        if n > 8 {
                                            return Err(malformed("escape sequence too long"));
                                        }
                                    }
                                    let e = (1 << (n + 4)) + b.read(n + 4) as i32;
                                    *v = if *v < 0 { -e } else { e };
                                }
                            }
                        }
                        for d in 0..dim.min(hi - k) {
                            quant[base + k + d] = vals[d];
                        }
                        k += dim;
                    }
                    if b.overrun() {
                        return Err(malformed("spectral data truncated"));
                    }
                }
            }
            win += glen;
        }
        if let Some(pulses) = pulse {
            for &(k, amp) in pulses {
                if quant[k] > 0 {
                    quant[k] += amp;
                } else {
                    quant[k] -= amp;
                }
            }
        }
        // inverse quantisation and noise substitution
        self.spec.fill(0.0);
        let mut win = 0;
        for (g, &glen) in self.info.group_len.iter().enumerate() {
            for s in 0..self.info.max_sfb {
                let i = g * 64 + s;
                let cb = self.cb[i];
                let lo = usize::from(offsets[s]);
                let hi = usize::from(offsets[s + 1]);
                for w in win..win + glen {
                    let band = w * win_len + lo..w * win_len + hi;
                    match cb {
                        ZERO_HCB | INTENSITY_HCB | INTENSITY_HCB2 => {}
                        NOISE_HCB => {
                            let mut energy = 0.0f32;
                            for c in &mut self.spec[band.clone()] {
                                *c = noise.next();
                                energy += *c * *c;
                            }
                            let scale =
                                2f32.powf(0.25 * self.sf[i] as f32) / energy.max(1e-30).sqrt();
                            for c in &mut self.spec[band] {
                                *c *= scale;
                            }
                        }
                        _ => {
                            let scale = 2f32.powf(0.25 * (self.sf[i] - 100) as f32);
                            for k in band {
                                let q = quant[k];
                                let mag = pow43(q.unsigned_abs()) * scale;
                                self.spec[k] = if q < 0 { -mag } else { mag };
                            }
                        }
                    }
                }
            }
            win += glen;
        }
        Ok(())
    }

    /// Applies the TNS filters in place.
    pub fn apply_tns(&mut self, rate_index: usize) {
        let mut spec = std::mem::take(&mut self.spec);
        self.tns_filter(&mut spec, rate_index, false);
        self.spec = spec;
    }

    /// Runs this channel's TNS filters over `spec`: the decoder's all-pole filters, or with
    /// `analysis` the encoder's all-zero ones (long-term prediction filters its estimate so).
    pub fn tns_filter(&self, spec: &mut [f32], rate_index: usize, analysis: bool) {
        if !self.tns.present {
            return;
        }
        let short = self.info.seq == Sequence::EightShort;
        let max_bands = usize::from(if short {
            tables::TNS_MAX_BANDS_SHORT[rate_index]
        } else {
            tables::TNS_MAX_BANDS_LONG[rate_index]
        });
        let offsets = self.info.offsets;
        let num_swb = offsets.len() - 1;
        let win_len = self.info.window_len();
        let limit = max_bands.min(self.info.max_sfb).min(num_swb);
        for w in 0..self.info.num_windows {
            let mut top = num_swb;
            for f in &self.tns.filters[w] {
                let bottom = top.saturating_sub(f.length);
                let start = usize::from(offsets[bottom.min(limit)]);
                let end = usize::from(offsets[top.min(limit)]);
                top = bottom;
                if f.order == 0 || end <= start {
                    continue;
                }
                let spec = &mut spec[w * win_len..(w + 1) * win_len];
                // the last `order` filter inputs (analysis) or outputs (synthesis)
                let mut state = [0f32; 20];
                let len = end - start;
                for n in 0..len {
                    let k = if f.downward { end - 1 - n } else { start + n };
                    let x = spec[k];
                    let mut y = x;
                    for (c, st) in f.coef[..f.order].iter().zip(&state) {
                        if analysis {
                            y += c * st;
                        } else {
                            y -= c * st;
                        }
                    }
                    state.copy_within(0..f.order - 1, 1);
                    state[0] = if analysis { x } else { y };
                    spec[k] = y;
                }
            }
        }
    }

    /// AAC Main prediction (§4.6.7) of a long-window frame, on the reconstructed spectrum
    /// after M/S: every line below `PRED_SFB_MAX` is predicted and its predictor adapted, the
    /// estimate added in bands that say so; noise-substituted bands and a signalled reset
    /// group reset their predictors. Short windows reset every predictor.
    pub fn predict(&mut self, state: &mut [Predictor], rate_index: usize) {
        if self.info.seq == Sequence::EightShort {
            state.fill(Predictor::default());
            return;
        }
        let offsets = self.info.offsets;
        let bands = usize::from(tables::PRED_SFB_MAX[rate_index]).min(offsets.len() - 1);
        let pred = self.info.pred.unwrap_or_default();
        let on = self.info.predictor;
        for s in 0..bands {
            let used = on && pred.used & (1 << s) != 0;
            let band = usize::from(offsets[s])..usize::from(offsets[s + 1]);
            for (st, x) in state[band.clone()].iter_mut().zip(&mut self.spec[band]) {
                st.run(x, used);
            }
        }
        for s in 0..self.info.max_sfb.min(bands) {
            if self.cb[s] == NOISE_HCB {
                let band = usize::from(offsets[s])..usize::from(offsets[s + 1]);
                state[band].fill(Predictor::default());
            }
        }
        if on && pred.reset_group > 0 {
            for st in state
                .iter_mut()
                .skip(usize::from(pred.reset_group) - 1)
                .step_by(30)
            {
                *st = Predictor::default();
            }
        }
    }
}

/// Applies M/S stereo to a channel pair. `ms` is `ms_mask_present`, `ms_used` the
/// per-(group, band) flags.
pub(crate) fn ms_stereo(l: &mut Channel, r: &mut Channel, ms: u32, ms_used: &[bool; 512]) {
    if ms == 0 {
        return;
    }
    let info = r.info.clone();
    let win_len = info.window_len();
    let mut win = 0;
    for (g, &glen) in info.group_len.iter().enumerate() {
        for s in 0..info.max_sfb {
            let i = g * 64 + s;
            let used = ms == 2 || ms_used[i];
            let (rcb, lcb) = (r.cb[i], l.cb[i]);
            if !used
                || matches!(rcb, NOISE_HCB | INTENSITY_HCB | INTENSITY_HCB2)
                || lcb == NOISE_HCB
            {
                continue;
            }
            let lo = usize::from(info.offsets[s]);
            let hi = usize::from(info.offsets[s + 1]);
            for w in win..win + glen {
                for k in w * win_len + lo..w * win_len + hi {
                    let (m, sd) = (l.spec[k], r.spec[k]);
                    l.spec[k] = m + sd;
                    r.spec[k] = m - sd;
                }
            }
        }
        win += glen;
    }
}

/// Applies intensity stereo to a channel pair: the right channel's intensity bands from the
/// left, the sign flipped where M/S is signalled.
pub(crate) fn intensity_stereo(l: &mut Channel, r: &mut Channel, ms: u32, ms_used: &[bool; 512]) {
    let info = r.info.clone();
    let win_len = info.window_len();
    let mut win = 0;
    for (g, &glen) in info.group_len.iter().enumerate() {
        for s in 0..info.max_sfb {
            let i = g * 64 + s;
            let rcb = r.cb[i];
            if rcb != INTENSITY_HCB && rcb != INTENSITY_HCB2 {
                continue;
            }
            let used = ms == 2 || (ms == 1 && ms_used[i]);
            let mut scale = 0.5f32.powf(0.25 * r.sf[i] as f32);
            if (rcb == INTENSITY_HCB2) != used {
                scale = -scale;
            }
            let lo = usize::from(info.offsets[s]);
            let hi = usize::from(info.offsets[s + 1]);
            for w in win..win + glen {
                for k in w * win_len + lo..w * win_len + hi {
                    r.spec[k] = l.spec[k] * scale;
                }
            }
        }
        win += glen;
    }
}
