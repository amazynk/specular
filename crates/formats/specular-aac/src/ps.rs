//! Parametric stereo (HE-AAC v2, ISO/IEC 14496-3 §8.6.4): the hybrid filterbank in its 20-
//! and 34-band configurations (parameters coded at another resolution are mapped), all-pass
//! decorrelation with transient attenuation, both mixing procedures, and the phase
//! parameters (IPD/OPD) of the PS extension with their smoothing.
//!
//! The mono QMF matrix of an SBR frame (64 bands × 32 slots) becomes a left and a right one.
//! The hybrid filters of the lowest QMF bands look 6 slots ahead; those slots come from the
//! next frame's low band, which the SBR decoder already has.

// Band, parameter and slot indices address several parallel arrays at once here.
#![allow(clippy::needless_range_loop)]

use std::f32::consts::{FRAC_1_SQRT_2, SQRT_2};
use std::f64::consts::PI;
use std::sync::OnceLock;

use crate::bits::Bits;
use crate::data::ps as tab;
use crate::fft::C;
use crate::sbr::{SLOTS, TREE_BIAS, tree_vlc};
use crate::vlc::Vlc;
use crate::{Error, Result};

fn malformed(s: &str) -> Error {
    Error::Malformed(format!("parametric stereo: {s}"))
}

/// The most hybrid bands (34-band configuration: 32 from QMF bands 0-4, then QMF 5-63).
const MAX_BANDS: usize = 91;
/// The most parameter bands.
const MAX_PAR: usize = 34;
const MAX_ALLPASS: usize = 50;
const MAX_PHASE: usize = 17;
/// Longest decorrelation delay in slots.
const MAX_DELAY: usize = 14;
const LINK_DELAY: [usize; 3] = [3, 4, 5];
/// a(m) = exp(−d(m)/7) for the link delays at 48 kHz.
const LINK_COEF: [f64; 3] = [
    0.651_439_057_531_06,
    0.564_718_122_007_76,
    0.489_541_659_556_95,
];
const FRACT_LINK: [f64; 3] = [0.43, 0.75, 0.347];
const FRACT_GAIN: f64 = 0.39;
const MAX_ENV: usize = 5;

/// IID quantisation grids in dB (coarse and fine) and the ICC values.
const IID_COARSE_DB: [f32; 15] = [
    -25.0, -18.0, -14.0, -10.0, -7.0, -4.0, -2.0, 0.0, 2.0, 4.0, 7.0, 10.0, 14.0, 18.0, 25.0,
];
const IID_FINE_DB: [f32; 31] = [
    -50.0, -45.0, -40.0, -35.0, -30.0, -25.0, -22.0, -19.0, -16.0, -13.0, -10.0, -8.0, -6.0, -4.0,
    -2.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 13.0, 16.0, 19.0, 22.0, 25.0, 30.0, 35.0, 40.0, 45.0,
    50.0,
];
const ICC: [f32; 8] = [1.0, 0.937, 0.841_18, 0.600_92, 0.367_64, 0.0, -0.589, -1.0];

/// The parameter band of each sub-QMF hybrid band of the 34-band configuration (the
/// standard's group-to-bin map; JAADec `map_group2bk34`, see data/PROVENANCE.md).
const SUB_TO_PAR_34: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 6, 7, 2, 1, 0, 10, 10, 4, 5, 6, 7, 8, 9, 10, 11, 12, 9, 14, 11, 12, 13,
    14, 15, 16, 13,
];
/// QMF band borders of the parameter bands above the sub-QMF bands: 20-band configuration
/// (bins 8..=19) and 34-band configuration (bins 16..=33).
const QMF_BORDERS_20: [usize; 13] = [3, 4, 5, 6, 7, 8, 9, 11, 14, 18, 23, 35, 64];
const QMF_BORDERS_34: [usize; 19] = [
    5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 21, 24, 27, 30, 33, 37, 41, 64,
];

/// Mixing matrices [h11, h12, h21, h22] by (procedure, fine, iid index + offset, icc).
struct Mixing {
    /// [procedure B][coarse 15 + fine 31][icc 8]
    h: [Vec<[[f32; 4]; 8]>; 2],
}

fn mixing() -> &'static Mixing {
    static M: OnceLock<Mixing> = OnceLock::new();
    M.get_or_init(|| {
        let mut h = [Vec::new(), Vec::new()];
        for (proc_b, out) in h.iter_mut().enumerate() {
            for &db in IID_COARSE_DB.iter().chain(&IID_FINE_DB) {
                let c = 10f32.powf(db / 20.0);
                let mut row = [[0f32; 4]; 8];
                for (icc, r) in row.iter_mut().enumerate() {
                    let rho = ICC[icc];
                    if proc_b == 0 {
                        let c1 = SQRT_2 / (1.0 + c * c).sqrt();
                        let c2 = c * c1;
                        let alpha = 0.5 * rho.acos();
                        let beta = alpha * (c1 - c2) * FRAC_1_SQRT_2;
                        *r = [
                            c2 * (beta + alpha).cos(),
                            c1 * (beta - alpha).cos(),
                            c2 * (beta + alpha).sin(),
                            c1 * (beta - alpha).sin(),
                        ];
                    } else {
                        let rho = rho.max(0.05);
                        let mut alpha = 0.5 * (2.0 * c * rho).atan2(c * c - 1.0);
                        let mu = c + 1.0 / c;
                        let mu = (1.0 + (4.0 * rho * rho - 4.0) / (mu * mu)).sqrt();
                        let gamma = ((1.0 - mu) / (1.0 + mu)).sqrt().atan();
                        if alpha < 0.0 {
                            alpha += std::f32::consts::FRAC_PI_2;
                        }
                        *r = [
                            SQRT_2 * alpha.cos() * gamma.cos(),
                            SQRT_2 * alpha.sin() * gamma.cos(),
                            -SQRT_2 * alpha.sin() * gamma.sin(),
                            SQRT_2 * alpha.cos() * gamma.sin(),
                        ];
                    }
                }
                out.push(row);
            }
        }
        Mixing { h }
    })
}

struct Books {
    iid_dt: Vlc,
    iid_df: Vlc,
    icc_dt: Vlc,
    icc_df: Vlc,
    iid_fine_dt: Vlc,
    iid_fine_df: Vlc,
    ipd_dt: Vlc,
    ipd_df: Vlc,
    opd_dt: Vlc,
    opd_df: Vlc,
}

fn books() -> Option<&'static Books> {
    static B: OnceLock<Option<Books>> = OnceLock::new();
    B.get_or_init(|| {
        Some(Books {
            iid_dt: tree_vlc(tab::IID_DT)?,
            iid_df: tree_vlc(tab::IID_DF)?,
            icc_dt: tree_vlc(tab::ICC_DT)?,
            icc_df: tree_vlc(tab::ICC_DF)?,
            iid_fine_dt: tree_vlc(tab::IID_FINE_DT)?,
            iid_fine_df: tree_vlc(tab::IID_FINE_DF)?,
            ipd_dt: tree_vlc(tab::IPD_DT)?,
            ipd_df: tree_vlc(tab::IPD_DF)?,
            opd_dt: tree_vlc(tab::OPD_DT)?,
            opd_df: tree_vlc(tab::OPD_DF)?,
        })
    })
    .as_ref()
}

/// A complex `bands`-band hybrid filter (13 taps per band) from a symmetric prototype:
/// g(n)·e^{−i2π(q+½)(n−6)/bands}.
fn hybrid_filter(half: &[f32; 7], bands: usize) -> Vec<[C; 13]> {
    (0..bands)
        .map(|q| {
            std::array::from_fn(|n| {
                let g = half[if n <= 6 { n } else { 12 - n }];
                let w = -2.0 * PI * (q as f64 + 0.5) * (n as f64 - 6.0) / bands as f64;
                C::new(g * w.cos() as f32, g * w.sin() as f32)
            })
        })
        .collect()
}

/// Centre frequency (in QMF bands) of sub-band `q` of `bands` split from QMF band `p`: the
/// band's own frequency (q + ½)·2/bands, taken into the span (p − ½, p + 3/2] it covers.
fn sub_centre(p: usize, q: usize, bands: usize) -> f64 {
    let mut c = (q as f64 + 0.5) * 2.0 / bands as f64;
    while c > p as f64 + 1.5 {
        c -= 2.0;
    }
    while c <= p as f64 - 0.5 {
        c += 2.0;
    }
    c
}

/// One of the two hybrid configurations.
struct Layout {
    /// Hybrid bands; the first `sub` come from the QMF bands below `split`.
    bands: usize,
    sub: usize,
    split: usize,
    par: usize,
    /// All-pass decorrelation below this band, a 14-slot delay below `short_delay`.
    allpass: usize,
    short_delay: usize,
    /// The all-pass decay slope starts here.
    decay_cutoff: usize,
    /// Parameter bands carrying phases.
    phase: usize,
    /// Hybrid bands whose phase parameters start mirrored (negative frequencies).
    negate: std::ops::RangeInclusive<usize>,
    band_to_par: Vec<u8>,
    /// Centre frequency of each all-pass band, in QMF bands.
    #[cfg(test)]
    centre: Vec<f64>,
    phi: Vec<C>,
    q: Vec<[C; 3]>,
    /// Hybrid filters: per split QMF band.
    filters: Vec<Vec<[C; 13]>>,
    /// 20-band configuration: the real 2-band filter of QMF bands 1 and 2.
    h2: [f32; 13],
}

fn layout(is34: bool) -> &'static Layout {
    static L: OnceLock<[Layout; 2]> = OnceLock::new();
    &L.get_or_init(|| {
        let make = |is34: bool| {
            let (sub, split, borders, first_bin): (usize, usize, &[usize], u8) = if is34 {
                (32, 5, &QMF_BORDERS_34, 16)
            } else {
                (10, 3, &QMF_BORDERS_20, 8)
            };
            let bands = sub + 64 - split;
            let mut band_to_par = if is34 {
                SUB_TO_PAR_34.to_vec()
            } else {
                vec![1, 0, 0, 1, 2, 3, 4, 5, 6, 7]
            };
            for (i, w) in borders.windows(2).enumerate() {
                for _ in w[0]..w[1] {
                    band_to_par.push(first_bin + i as u8);
                }
            }
            // centre frequencies of the sub-QMF bands, then QMF band k + ½
            let centres: Vec<f64> = if is34 {
                let mut c = Vec::new();
                for (p, b) in [(0usize, 12usize), (1, 8), (2, 4), (3, 4), (4, 4)] {
                    for q in 0..b {
                        c.push(sub_centre(p, q, b));
                    }
                }
                c
            } else {
                // QMF 0: 8 bands combined to 6 (−3/8, −1/8, 1/8, 3/8, 5/8, 7/8), then the
                // halves of QMF 1 and 2
                vec![
                    -0.375, -0.125, 0.125, 0.375, 0.625, 0.875, 1.25, 1.75, 2.25, 2.75,
                ]
            };
            let allpass = if is34 { 50 } else { 30 };
            let centre = |k: usize| {
                if k < sub {
                    centres[k]
                } else {
                    (k - sub + split) as f64 + 0.5
                }
            };
            let ex = |t: f64| C::new(t.cos() as f32, t.sin() as f32);
            let centre: Vec<f64> = (0..allpass).map(centre).collect();
            let phi = centre.iter().map(|&f| ex(-PI * FRACT_GAIN * f)).collect();
            let q = centre
                .iter()
                .map(|&f| std::array::from_fn(|m| ex(-PI * FRACT_LINK[m] * f)))
                .collect();
            let filters = if is34 {
                vec![
                    hybrid_filter(&tab::HYBRID12_34, 12),
                    hybrid_filter(&tab::HYBRID8_34, 8),
                    hybrid_filter(&tab::HYBRID4_34, 4),
                    hybrid_filter(&tab::HYBRID4_34, 4),
                    hybrid_filter(&tab::HYBRID4_34, 4),
                ]
            } else {
                vec![hybrid_filter(&tab::HYBRID8, 8)]
            };
            let h2 = std::array::from_fn(|n| tab::HYBRID2[if n <= 6 { n } else { 12 - n }]);
            Layout {
                bands,
                sub,
                split,
                par: if is34 { 34 } else { 20 },
                allpass,
                short_delay: if is34 { 62 } else { 42 },
                decay_cutoff: if is34 { 32 } else { 10 },
                phase: if is34 { 17 } else { 11 },
                negate: if is34 { 9..=13 } else { 0..=1 },
                band_to_par,
                #[cfg(test)]
                centre,
                phi,
                q,
                filters,
                h2,
            }
        };
        [make(false), make(true)]
    })[usize::from(is34)]
}

/// The decoder state of parametric stereo for one channel element.
pub(crate) struct Ps {
    header_seen: bool,
    enable_iid: bool,
    iid_mode: u8,
    enable_icc: bool,
    icc_mode: u8,
    enable_ext: bool,
    /// The 34-band configuration is in use, and the one the mixing state is in.
    is34: bool,
    mixed34: bool,
    /// Parameters of this frame, in their coded resolution.
    num_env: usize,
    border: [i32; MAX_ENV + 2],
    iid: [[i8; 34]; MAX_ENV + 1],
    icc: [[i8; 34]; MAX_ENV + 1],
    iid_prev: [i8; 34],
    icc_prev: [i8; 34],
    /// Phase parameters (indices of π/4) of this frame, when the extension sent them.
    enable_ipdopd: bool,
    ipd: [[u8; 17]; MAX_ENV + 1],
    opd: [[u8; 17]; MAX_ENV + 1],
    ipd_prev: [u8; 17],
    opd_prev: [u8; 17],
    /// The two previous phase indices of each band (3 bits each), for smoothing.
    ipd_hist: [u8; MAX_PHASE],
    opd_hist: [u8; MAX_PHASE],
    /// QMF bands 0-4 of the last 6 slots, for the hybrid filters.
    hyb_hist: [[C; 6]; 5],
    /// Hybrid band history (last `MAX_DELAY` slots) and the current frame.
    delay: Vec<[C; MAX_DELAY + SLOTS]>,
    /// All-pass link states (last 5 slots, then the frame) per band and link.
    ap: Vec<[[C; 5 + SLOTS]; 3]>,
    peak_decay: [f32; MAX_PAR],
    power_smooth: [f32; MAX_PAR],
    peak_diff_smooth: [f32; MAX_PAR],
    /// Mixing matrix reached at the end of the last frame, per parameter band: real parts
    /// of h11, h12, h21, h22, then their imaginary parts.
    h_prev: [[f32; 8]; MAX_PAR],
    /// Scratch: hybrid signals.
    s: Vec<[C; SLOTS]>,
    d: Vec<[C; SLOTS]>,
}

impl Ps {
    pub fn new() -> Ps {
        Ps {
            header_seen: false,
            enable_iid: false,
            iid_mode: 0,
            enable_icc: false,
            icc_mode: 0,
            enable_ext: false,
            is34: false,
            mixed34: false,
            num_env: 0,
            border: [0; MAX_ENV + 2],
            iid: [[0; 34]; MAX_ENV + 1],
            icc: [[0; 34]; MAX_ENV + 1],
            iid_prev: [0; 34],
            icc_prev: [0; 34],
            enable_ipdopd: false,
            ipd: [[0; 17]; MAX_ENV + 1],
            opd: [[0; 17]; MAX_ENV + 1],
            ipd_prev: [0; 17],
            opd_prev: [0; 17],
            ipd_hist: [0; MAX_PHASE],
            opd_hist: [0; MAX_PHASE],
            hyb_hist: [[C::default(); 6]; 5],
            delay: vec![[C::default(); MAX_DELAY + SLOTS]; MAX_BANDS],
            ap: vec![[[C::default(); 5 + SLOTS]; 3]; MAX_ALLPASS],
            peak_decay: [0.0; MAX_PAR],
            power_smooth: [0.0; MAX_PAR],
            peak_diff_smooth: [0.0; MAX_PAR],
            h_prev: [[0.0; 8]; MAX_PAR],
            s: vec![[C::default(); SLOTS]; MAX_BANDS],
            d: vec![[C::default(); SLOTS]; MAX_BANDS],
        }
    }

    /// A PS header has been read.
    pub fn seen(&self) -> bool {
        self.header_seen
    }

    /// Reads ps_data() (at most `bits` bits).
    pub fn read(&mut self, b: &mut Bits, bits: usize) -> Result<()> {
        let start = b.pos();
        let books = books().ok_or_else(|| Error::Unsupported("PS tables".into()))?;
        if b.bit() {
            self.header_seen = true;
            self.enable_iid = b.bit();
            if self.enable_iid {
                self.iid_mode = b.read(3) as u8;
            }
            self.enable_icc = b.bit();
            if self.enable_icc {
                self.icc_mode = b.read(3) as u8;
            }
            self.enable_ext = b.bit();
            if self.iid_mode > 5 || self.icc_mode > 5 {
                self.header_seen = false;
                return Err(malformed("reserved IID or ICC mode"));
            }
            // without IID and ICC the configuration stays as it was
            let is34 = (self.enable_iid && self.iid_mode % 3 == 2)
                || (self.enable_icc && self.icc_mode % 3 == 2);
            if (self.enable_iid || self.enable_icc) && is34 != self.is34 {
                self.is34 = is34;
                self.reset_filters();
            }
        }
        let class = b.bit();
        let idx = b.read(2) as usize;
        let num_env = if class {
            [1, 2, 3, 4][idx]
        } else {
            [0, 1, 2, 4][idx]
        };
        if class {
            for e in 1..=num_env {
                self.border[e] = b.read(5) as i32;
            }
        }
        if !self.header_seen {
            return Err(malformed("data before the first header"));
        }
        let n_iid = [10, 20, 34][usize::from(self.iid_mode % 3)];
        let n_icc = [10, 20, 34][usize::from(self.icc_mode % 3)];
        let fine = self.iid_mode >= 3;
        if self.enable_iid {
            let lim = if fine { 15 } else { 7 };
            for e in 0..num_env {
                let dt = b.bit();
                let book = match (fine, dt) {
                    (false, true) => &books.iid_dt,
                    (false, false) => &books.iid_df,
                    (true, true) => &books.iid_fine_dt,
                    (true, false) => &books.iid_fine_df,
                };
                let prev = if e == 0 {
                    self.iid_prev
                } else {
                    self.iid[e - 1]
                };
                read_par(b, book, dt, &prev, &mut self.iid[e][..n_iid], -lim, lim)?;
            }
        } else {
            self.iid = [[0; 34]; MAX_ENV + 1];
        }
        if self.enable_icc {
            for e in 0..num_env {
                let dt = b.bit();
                let book = if dt { &books.icc_dt } else { &books.icc_df };
                let prev = if e == 0 {
                    self.icc_prev
                } else {
                    self.icc[e - 1]
                };
                read_par(b, book, dt, &prev, &mut self.icc[e][..n_icc], 0, 7)?;
            }
        } else {
            self.icc = [[0; 34]; MAX_ENV + 1];
        }
        self.enable_ipdopd = false;
        if self.enable_ext {
            let mut cnt = b.read(4) as usize;
            if cnt == 15 {
                cnt += b.read(8) as usize;
            }
            let mut left = 8 * cnt;
            while left > 7 {
                let id = b.read(2);
                left -= 2;
                if id != 0 {
                    break;
                }
                let at = b.pos();
                self.read_phases(b, books, num_env)?;
                left = left
                    .checked_sub(b.pos() - at)
                    .ok_or_else(|| malformed("phase data runs past the extension"))?;
            }
            b.skip(left);
        }
        if b.pos() - start > bits {
            return Err(malformed("data runs past its extension"));
        }
        // envelope borders in QMF slots
        self.border[0] = -1;
        let mut n = num_env;
        if class {
            for e in 1..=n {
                if self.border[e] <= self.border[e - 1] || self.border[e] > SLOTS as i32 - 1 {
                    return Err(malformed("envelope borders out of order"));
                }
            }
        } else {
            for e in 1..=n {
                self.border[e] = (e * SLOTS / n) as i32 - 1;
            }
        }
        if n == 0 || self.border[n] < SLOTS as i32 - 1 {
            // one more envelope holding the last parameters to the end of the frame
            let (iid, icc, ipd, opd) = if n == 0 {
                (self.iid_prev, self.icc_prev, self.ipd_prev, self.opd_prev)
            } else {
                (
                    self.iid[n - 1],
                    self.icc[n - 1],
                    self.ipd[n - 1],
                    self.opd[n - 1],
                )
            };
            self.iid[n] = iid;
            self.icc[n] = icc;
            self.ipd[n] = ipd;
            self.opd[n] = opd;
            n += 1;
            self.border[n] = SLOTS as i32 - 1;
        }
        self.num_env = n;
        self.iid_prev = self.iid[n - 1];
        self.icc_prev = self.icc[n - 1];
        self.ipd_prev = self.ipd[n - 1];
        self.opd_prev = self.opd[n - 1];
        Ok(())
    }

    /// The decorrelator starts afresh when the hybrid configuration changes.
    fn reset_filters(&mut self) {
        for d in &mut self.delay {
            *d = [C::default(); MAX_DELAY + SLOTS];
        }
        for a in &mut self.ap {
            *a = [[C::default(); 5 + SLOTS]; 3];
        }
        self.peak_decay = [0.0; MAX_PAR];
        self.power_smooth = [0.0; MAX_PAR];
        self.peak_diff_smooth = [0.0; MAX_PAR];
    }

    /// ps_extension() with id 0: `enable_ipdopd` and the IPD/OPD data of each envelope.
    fn read_phases(&mut self, b: &mut Bits, books: &Books, num_env: usize) -> Result<()> {
        self.enable_ipdopd = b.bit();
        if self.enable_ipdopd {
            let n = [5, 11, 17][usize::from(self.iid_mode % 3)];
            for e in 0..num_env {
                for opd in [false, true] {
                    let dt = b.bit();
                    let book = match (opd, dt) {
                        (false, true) => &books.ipd_dt,
                        (false, false) => &books.ipd_df,
                        (true, true) => &books.opd_dt,
                        (true, false) => &books.opd_df,
                    };
                    let (prev, cur) = match (opd, e) {
                        (false, 0) => (self.ipd_prev, &mut self.ipd[0]),
                        (true, 0) => (self.opd_prev, &mut self.opd[0]),
                        (false, _) => (self.ipd[e - 1], &mut self.ipd[e]),
                        (true, _) => (self.opd[e - 1], &mut self.opd[e]),
                    };
                    let mut acc = 0i32;
                    for (i, c) in cur.iter_mut().take(n).enumerate() {
                        let d = book
                            .read(b)
                            .ok_or_else(|| malformed("invalid Huffman code"))?
                            as i32
                            - TREE_BIAS;
                        acc = if dt { i32::from(prev[i]) + d } else { acc + d };
                        acc &= 7;
                        *c = acc as u8;
                    }
                }
            }
        }
        b.read(1); // reserved_ps
        Ok(())
    }

    /// IID and ICC indices of envelope `e` at the configuration's resolution.
    fn mapped(&self, e: usize) -> ([i8; MAX_PAR], [i8; MAX_PAR]) {
        let n_iid = [10, 20, 34][usize::from(self.iid_mode % 3)];
        let n_icc = [10, 20, 34][usize::from(self.icc_mode % 3)];
        (
            to_bands(&self.iid[e], n_iid, self.is34),
            to_bands(&self.icc[e], n_icc, self.is34),
        )
    }

    /// IPD and OPD indices of envelope `e` at the configuration's resolution.
    fn mapped_phases(&self, e: usize) -> ([u8; MAX_PAR], [u8; MAX_PAR]) {
        // coded in 5, 11 or 17 bands: the first bands of 10, 20 or 34
        let n = [10, 20, 34][usize::from(self.iid_mode % 3)];
        let map = |p: &[u8; 17]| -> [u8; MAX_PAR] {
            let mut wide = [0i8; 34];
            for (w, &v) in wide.iter_mut().zip(p.iter()) {
                *w = v as i8;
            }
            let m = to_bands(&wide, n, self.is34);
            std::array::from_fn(|i| m[i] as u8)
        };
        (map(&self.ipd[e]), map(&self.opd[e]))
    }

    /// A frame without new data repeats the last frame's envelopes.
    pub fn process_without_data(
        &mut self,
        l: &mut [[C; 64]],
        r: &mut [[C; 64]],
        ahead: &[[C; 32]],
    ) {
        if !self.header_seen || self.num_env == 0 {
            r.copy_from_slice(l);
            return;
        }
        self.process(l, r, ahead);
    }

    /// Turns the mono matrix `l` (32 slots × 64 bands) into `l` and `r`. `ahead` holds the
    /// next 6 slots of the lowest QMF bands.
    pub fn process(&mut self, l: &mut [[C; 64]], r: &mut [[C; 64]], ahead: &[[C; 32]]) {
        let lay = layout(self.is34);
        self.hybrid_analysis(l, ahead, lay);
        self.decorrelate(lay);
        self.mix(lay);
        // hybrid synthesis of both channels
        let groups: &[usize] = if self.is34 {
            &[12, 8, 4, 4, 4]
        } else {
            &[6, 2, 2]
        };
        for (out, sig) in [(&mut *l, &self.s), (&mut *r, &self.d)] {
            for n in 0..SLOTS {
                let row = &mut out[n];
                let mut k = 0;
                for (q, &g) in groups.iter().enumerate() {
                    let mut acc = C::default();
                    for band in &sig[k..k + g] {
                        acc = acc.add(band[n]);
                    }
                    row[q] = acc;
                    k += g;
                }
                for q in lay.split..64 {
                    row[q] = sig[q - lay.split + lay.sub][n];
                }
            }
        }
    }

    fn hybrid_analysis(&mut self, x: &[[C; 64]], ahead: &[[C; 32]], lay: &Layout) {
        // the split QMF bands with 6 slots before and after the frame
        let mut buf = [[C::default(); 6 + SLOTS + 6]; 5];
        for q in 0..lay.split {
            buf[q][..6].copy_from_slice(&self.hyb_hist[q]);
            for n in 0..SLOTS {
                buf[q][6 + n] = x[n][q];
            }
            for n in 0..6 {
                buf[q][6 + SLOTS + n] = ahead.get(n).map_or(C::default(), |a| a[q]);
            }
        }
        for q in 0..5 {
            let src = if q < lay.split {
                buf[q]
            } else {
                // keep the history of every band either configuration splits
                let mut t = [C::default(); 6 + SLOTS + 6];
                for n in 0..SLOTS {
                    t[6 + n] = x[n][q];
                }
                t
            };
            self.hyb_hist[q].copy_from_slice(&src[SLOTS..SLOTS + 6]);
        }
        let apply = |f: &[C; 13], x: &[C], n: usize| -> C {
            let mut acc = C::default();
            for j in 0..13 {
                acc = acc.add(f[j].mul(x[n + j]));
            }
            acc
        };
        for n in 0..SLOTS {
            if self.is34 {
                let mut k = 0;
                for (q, f) in lay.filters.iter().enumerate() {
                    for band in f {
                        self.s[k][n] = apply(band, &buf[q], n);
                        k += 1;
                    }
                }
            } else {
                // 8 complex bands from QMF 0, combined to 6
                let t: [C; 8] = std::array::from_fn(|k| apply(&lay.filters[0][k], &buf[0], n));
                self.s[0][n] = t[6];
                self.s[1][n] = t[7];
                self.s[2][n] = t[0];
                self.s[3][n] = t[1];
                self.s[4][n] = t[2].add(t[5]);
                self.s[5][n] = t[3].add(t[4]);
                // 2 real bands from each of QMF 1 and 2 (their spectra run in opposite
                // directions)
                for (q, base, flip) in [(1usize, 6usize, true), (2, 8, false)] {
                    let x = &buf[q];
                    let mid = C::new(lay.h2[6] * x[n + 6].re, lay.h2[6] * x[n + 6].im);
                    let mut side = C::default();
                    for j in [1usize, 3, 5] {
                        let s = x[n + j].add(x[n + 12 - j]);
                        side = side.add(C::new(lay.h2[j] * s.re, lay.h2[j] * s.im));
                    }
                    let (lo, hi) = (mid.add(side), mid.sub(side));
                    if flip {
                        self.s[base][n] = hi;
                        self.s[base + 1][n] = lo;
                    } else {
                        self.s[base][n] = lo;
                        self.s[base + 1][n] = hi;
                    }
                }
            }
            for q in lay.split..64 {
                self.s[q - lay.split + lay.sub][n] = x[n][q];
            }
        }
    }

    fn decorrelate(&mut self, lay: &Layout) {
        // transient attenuation per parameter band
        let mut gain = [[1f32; SLOTS]; MAX_PAR];
        let mut power = [[0f32; SLOTS]; MAX_PAR];
        for k in 0..lay.bands {
            let i = usize::from(lay.band_to_par[k]);
            for n in 0..SLOTS {
                let v = self.s[k][n];
                power[i][n] += v.re * v.re + v.im * v.im;
            }
        }
        for i in 0..lay.par {
            for n in 0..SLOTS {
                let p = power[i][n];
                self.peak_decay[i] = (self.peak_decay[i] * 0.765_928_3).max(p);
                self.power_smooth[i] += 0.25 * (p - self.power_smooth[i]);
                self.peak_diff_smooth[i] +=
                    0.25 * (self.peak_decay[i] - p - self.peak_diff_smooth[i]);
                let den = 1.5 * self.peak_diff_smooth[i];
                gain[i][n] = if den > self.power_smooth[i] {
                    self.power_smooth[i] / den
                } else {
                    1.0
                };
            }
        }
        for k in 0..lay.bands {
            let line = &mut self.delay[k];
            line.copy_within(SLOTS..SLOTS + MAX_DELAY, 0);
            line[MAX_DELAY..].copy_from_slice(&self.s[k]);
            let gi = &gain[usize::from(lay.band_to_par[k])];
            if k < lay.allpass {
                let decay = if k < lay.decay_cutoff {
                    1.0
                } else {
                    (1.0 - 0.05 * (k - lay.decay_cutoff) as f32).clamp(0.0, 1.0)
                };
                let ap = &mut self.ap[k];
                for link in ap.iter_mut() {
                    link.copy_within(SLOTS..SLOTS + 5, 0);
                }
                for n in 0..SLOTS {
                    let mut v = line[MAX_DELAY + n - 2].mul(lay.phi[k]);
                    for m in 0..3 {
                        let a = decay * LINK_COEF[m] as f32;
                        let w_old = ap[m][5 + n - LINK_DELAY[m]];
                        let y = lay.q[k][m].mul(w_old).sub(C::new(a * v.re, a * v.im));
                        ap[m][5 + n] = v.add(C::new(a * y.re, a * y.im));
                        v = y;
                    }
                    self.d[k][n] = C::new(v.re * gi[n], v.im * gi[n]);
                }
            } else {
                let dl = if k < lay.short_delay { MAX_DELAY } else { 1 };
                for n in 0..SLOTS {
                    let v = line[MAX_DELAY + n - dl];
                    self.d[k][n] = C::new(v.re * gi[n], v.im * gi[n]);
                }
            }
        }
    }

    fn mix(&mut self, lay: &Layout) {
        if self.mixed34 != self.is34 {
            // carry the mixing state over to the other band layout
            for j in 0..8 {
                let mut v: [f32; MAX_PAR] = std::array::from_fn(|b| self.h_prev[b][j]);
                v = if self.is34 {
                    values_to34(&v)
                } else {
                    values_to20(&v)
                };
                for b in 0..MAX_PAR {
                    self.h_prev[b][j] = v[b];
                }
            }
            self.ipd_hist = [0; MAX_PHASE];
            self.opd_hist = [0; MAX_PHASE];
            self.mixed34 = self.is34;
        }
        let mix = mixing();
        let table = &mix.h[usize::from(self.icc_mode >= 3)];
        let fine = self.iid_mode >= 3;
        let phases = self.enable_ipdopd;
        for e in 0..self.num_env {
            let (iid, icc) = self.mapped(e);
            let (ipd, opd) = self.mapped_phases(e);
            let (start, stop) = (self.border[e], self.border[e + 1]);
            let width = (stop - start).max(1) as f32;
            let mut h_new = [[0f32; 8]; MAX_PAR];
            for b in 0..lay.par {
                let row = if fine {
                    15 + (i32::from(iid[b]) + 15) as usize
                } else {
                    (i32::from(iid[b]) + 7) as usize
                };
                let h = table[row.min(45)][usize::from(icc[b] as u8).min(7)];
                h_new[b][..4].copy_from_slice(&h);
                if phases && b < lay.phase {
                    // smoothed phases from this and the two previous envelopes
                    let o_idx = usize::from(self.opd_hist[b]) * 8 + usize::from(opd[b]);
                    let i_idx = usize::from(self.ipd_hist[b]) * 8 + usize::from(ipd[b]);
                    self.opd_hist[b] = (o_idx & 0x3f) as u8;
                    self.ipd_hist[b] = (i_idx & 0x3f) as u8;
                    let (o, ip) = (smoothed_phase(o_idx), smoothed_phase(i_idx));
                    // h11 and h21 turn by the OPD, h12 and h22 by OPD − IPD
                    let adj = C::new(o.re * ip.re + o.im * ip.im, o.im * ip.re - o.re * ip.im);
                    h_new[b] = [
                        h[0] * o.re,
                        h[1] * adj.re,
                        h[2] * o.re,
                        h[3] * adj.re,
                        h[0] * o.im,
                        h[1] * adj.im,
                        h[2] * o.im,
                        h[3] * adj.im,
                    ];
                }
            }
            let slots = (start + 1).max(0) as usize..=(stop as usize).min(SLOTS - 1);
            for k in 0..lay.bands {
                let b = usize::from(lay.band_to_par[k]);
                let mut h0 = self.h_prev[b];
                if phases && lay.negate.contains(&k) {
                    // negative-frequency hybrid bands take the phases mirrored
                    for v in &mut h0[4..] {
                        *v = -*v;
                    }
                }
                let mut step = [0f32; 8];
                for j in 0..8 {
                    step[j] = (h_new[b][j] - h0[j]) / width;
                }
                for n in slots.clone() {
                    let t = (n as i32 - start) as f32;
                    let h: [f32; 8] = std::array::from_fn(|j| h0[j] + step[j] * t);
                    let (s, d) = (self.s[k][n], self.d[k][n]);
                    if phases {
                        self.s[k][n] = C::new(
                            h[0] * s.re + h[2] * d.re - h[4] * s.im - h[6] * d.im,
                            h[0] * s.im + h[2] * d.im + h[4] * s.re + h[6] * d.re,
                        );
                        self.d[k][n] = C::new(
                            h[1] * s.re + h[3] * d.re - h[5] * s.im - h[7] * d.im,
                            h[1] * s.im + h[3] * d.im + h[5] * s.re + h[7] * d.re,
                        );
                    } else {
                        self.s[k][n] = C::new(h[0] * s.re + h[2] * d.re, h[0] * s.im + h[2] * d.im);
                        self.d[k][n] = C::new(h[1] * s.re + h[3] * d.re, h[1] * s.im + h[3] * d.im);
                    }
                }
            }
            self.h_prev = h_new;
        }
    }
}

/// The phase of `idx` = (oldest·64 + previous·8 + current) indices of π/4, smoothed with
/// weights ¼, ½, 1 and normalised.
fn smoothed_phase(idx: usize) -> C {
    let ph = |i: usize| std::f32::consts::FRAC_PI_4 * i as f32;
    let (a, b, c) = (ph((idx >> 6) & 7), ph((idx >> 3) & 7), ph(idx & 7));
    let re = 0.25 * a.cos() + 0.5 * b.cos() + c.cos();
    let im = 0.25 * a.sin() + 0.5 * b.sin() + c.sin();
    let mag = (re * re + im * im).sqrt();
    if mag > 0.0 {
        C::new(re / mag, im / mag)
    } else {
        C::new(1.0, 0.0)
    }
}

/// Mixing values of the 34 bands averaged to 20 (the first 20 entries).
fn values_to20(p: &[f32; MAX_PAR]) -> [f32; MAX_PAR] {
    let mut o = [0f32; MAX_PAR];
    let pairs: [(usize, usize, f32, f32); 20] = [
        (0, 1, 2.0, 1.0),
        (1, 2, 1.0, 2.0),
        (3, 4, 2.0, 1.0),
        (4, 5, 1.0, 2.0),
        (6, 7, 1.0, 1.0),
        (8, 9, 1.0, 1.0),
        (10, 10, 1.0, 0.0),
        (11, 11, 1.0, 0.0),
        (12, 13, 1.0, 1.0),
        (14, 15, 1.0, 1.0),
        (16, 16, 1.0, 0.0),
        (17, 17, 1.0, 0.0),
        (18, 18, 1.0, 0.0),
        (19, 19, 1.0, 0.0),
        (20, 21, 1.0, 1.0),
        (22, 23, 1.0, 1.0),
        (24, 25, 1.0, 1.0),
        (26, 27, 1.0, 1.0),
        (28, 29, 1.0, 1.0),
        (32, 33, 1.0, 1.0),
    ];
    for (i, &(a, b, wa, wb)) in pairs.iter().enumerate() {
        o[i] = (wa * p[a] + wb * p[b]) / (wa + wb);
    }
    o[18] = (p[28] + p[29] + p[30] + p[31]) * 0.25;
    o
}

/// Mixing values of the 20 bands spread to 34.
fn values_to34(p: &[f32; MAX_PAR]) -> [f32; MAX_PAR] {
    const SRC: [usize; 34] = [
        0, 0, 1, 2, 2, 3, 4, 4, 5, 5, 6, 7, 8, 8, 9, 9, 10, 11, 12, 13, 14, 14, 15, 15, 16, 16, 17,
        17, 18, 18, 18, 18, 19, 19,
    ];
    let mut o: [f32; MAX_PAR] = std::array::from_fn(|i| p[SRC[i]]);
    o[1] = (p[0] + p[1]) * 0.5;
    o[4] = (p[2] + p[3]) * 0.5;
    o
}

/// Reads `out.len()` parameters, time- or frequency-differential, checking the range.
fn read_par(
    b: &mut Bits,
    book: &Vlc,
    dt: bool,
    prev: &[i8; 34],
    out: &mut [i8],
    lo: i32,
    hi: i32,
) -> Result<()> {
    let mut acc = 0i32;
    for (i, o) in out.iter_mut().enumerate() {
        let d = book
            .read(b)
            .ok_or_else(|| malformed("invalid Huffman code"))? as i32
            - TREE_BIAS;
        let v = if dt {
            i32::from(prev[i]) + d
        } else {
            acc += d;
            acc
        };
        if !(lo..=hi).contains(&v) {
            return Err(malformed("parameter out of range"));
        }
        *o = v as i8;
    }
    Ok(())
}

/// Parameters of `n` bands (10, 20 or 34) at 34 bands or at 20 (`is34` false).
fn to_bands(p: &[i8; 34], n: usize, is34: bool) -> [i8; MAX_PAR] {
    let mut o = [0i8; MAX_PAR];
    if is34 {
        let p20 = if n == 34 {
            o.copy_from_slice(p);
            return o;
        } else {
            to20(p, n)
        };
        o.copy_from_slice(&to34(&p20));
    } else {
        o[..20].copy_from_slice(&to20(p, n));
    }
    o
}

/// Maps parameters of `n` bands (10, 20 or 34) to the 20 bands (34 by averaging, as the
/// standard's mapping for the baseline decoder; integer division truncates).
fn to20(p: &[i8; 34], n: usize) -> [i8; 20] {
    let mut o = [0i8; 20];
    match n {
        10 => {
            for b in 0..10 {
                o[2 * b] = p[b];
                o[2 * b + 1] = p[b];
            }
        }
        34 => {
            let v = |i: usize| i32::from(p[i]);
            let m = [
                (2 * v(0) + v(1)) / 3,
                (v(1) + 2 * v(2)) / 3,
                (2 * v(3) + v(4)) / 3,
                (v(4) + 2 * v(5)) / 3,
                (v(6) + v(7)) / 2,
                (v(8) + v(9)) / 2,
                v(10),
                v(11),
                (v(12) + v(13)) / 2,
                (v(14) + v(15)) / 2,
                v(16),
                v(17),
                v(18),
                v(19),
                (v(20) + v(21)) / 2,
                (v(22) + v(23)) / 2,
                (v(24) + v(25)) / 2,
                (v(26) + v(27)) / 2,
                (v(28) + v(29) + v(30) + v(31)) / 4,
                (v(32) + v(33)) / 2,
            ];
            for (d, s) in o.iter_mut().zip(m) {
                *d = s as i8;
            }
        }
        _ => o.copy_from_slice(&p[..20]),
    }
    o
}

/// Maps 20-band parameters to the 34 bands.
fn to34(p: &[i8; 20]) -> [i8; 34] {
    // the 20-band source of each of the 34 bands; bands 1 and 4 average two
    const SRC: [u8; 34] = [
        0, 0, 1, 2, 2, 3, 4, 4, 5, 5, 6, 7, 8, 8, 9, 9, 10, 11, 12, 13, 14, 14, 15, 15, 16, 16, 17,
        17, 18, 18, 18, 18, 19, 19,
    ];
    let mut o: [i8; 34] = std::array::from_fn(|i| p[usize::from(SRC[i])]);
    o[1] = ((i32::from(p[0]) + i32::from(p[1])) / 2) as i8;
    o[4] = ((i32::from(p[2]) + i32::from(p[3])) / 2) as i8;
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_maps_cover_the_qmf_bands() {
        let l = layout(false);
        assert_eq!(l.band_to_par.len(), 71);
        assert_eq!(l.band_to_par[10], 8); // QMF 3
        assert_eq!(l.band_to_par[16], 14); // QMF 9
        assert_eq!(l.band_to_par[17], 14); // QMF 10
        assert_eq!(l.band_to_par[29], 17); // QMF 22
        assert_eq!(l.band_to_par[30], 18); // QMF 23
        assert_eq!(l.band_to_par[70], 19); // QMF 63
        let l = layout(true);
        assert_eq!(l.band_to_par.len(), 91);
        assert_eq!(l.band_to_par[32], 16); // QMF 5
        assert_eq!(l.band_to_par[90], 33); // QMF 63
    }

    /// Parameter bands follow the centre frequencies of the hybrid bands: never lower for a
    /// higher frequency (negative frequencies count by their magnitude).
    #[test]
    fn band_maps_follow_frequency() {
        for is34 in [false, true] {
            let l = layout(is34);
            let centre = |k: usize| l.centre[k].abs();
            let mut order: Vec<usize> = (0..l.allpass).collect();
            order.sort_by(|&a, &b| centre(a).total_cmp(&centre(b)));
            for w in order.windows(2) {
                assert!(
                    l.band_to_par[w[0]] <= l.band_to_par[w[1]],
                    "is34 {is34}: bands {} {}",
                    w[0],
                    w[1]
                );
            }
        }
    }

    /// The hybrid filters split a band without loss: their sum is the input delayed.
    #[test]
    fn hybrid_filters_sum_to_a_delay() {
        for (half, bands) in [
            (&tab::HYBRID8, 8),
            (&tab::HYBRID12_34, 12),
            (&tab::HYBRID8_34, 8),
            (&tab::HYBRID4_34, 4),
        ] {
            let f = hybrid_filter(half, bands);
            for n in 0..13 {
                let s = f.iter().fold(C::default(), |a, b| a.add(b[n]));
                let want = if n == 6 { 1.0 } else { 0.0 };
                assert!(
                    (s.re - want).abs() < 1e-6 && s.im.abs() < 1e-6,
                    "{bands} {n}: {s:?}"
                );
            }
        }
    }

    /// The IID grids give PacketVideo's c1 = √2/√(1+c²) values (scaleFactors).
    #[test]
    fn iid_grid_matches_reference_scale_factors() {
        let pv = [
            1.411983, 1.403138, 1.386877, 1.348400, 1.291249, 1.196037, 1.107372, 1.000000,
            0.879617, 0.754649, 0.576780, 0.426401, 0.276718, 0.176645, 0.079402,
        ];
        for (db, want) in IID_COARSE_DB.iter().zip(pv) {
            let c = 10f64.powf(f64::from(*db) / 20.0);
            let c1 = 2f64.sqrt() / (1.0 + c * c).sqrt();
            assert!((c1 - want).abs() < 2e-6, "{db}: {c1} {want}");
        }
        let fine_first = [1.414207, 1.414191, 1.414143, 1.413990, 1.413507];
        for (db, want) in IID_FINE_DB.iter().zip(fine_first) {
            let c = 10f64.powf(f64::from(*db) / 20.0);
            let c1 = 2f64.sqrt() / (1.0 + c * c).sqrt();
            assert!((c1 - want).abs() < 2e-6, "{db}: {c1} {want}");
        }
    }

    #[test]
    fn trees_build() {
        assert!(books().is_some());
    }

    /// With IID 0 and ICC 1 (fully correlated), both outputs equal the input.
    #[test]
    fn identity_mixing() {
        let h = mixing().h[0][7][0];
        assert!(
            (h[0] - 1.0).abs() < 1e-6 && (h[1] - 1.0).abs() < 1e-6,
            "{h:?}"
        );
        assert!(h[2].abs() < 1e-6 && h[3].abs() < 1e-6);
    }

    #[test]
    fn resolution_maps() {
        let p: [i8; 34] = std::array::from_fn(|i| i as i8);
        let o = to_bands(&p, 34, true);
        assert_eq!(&o[..], &p[..]);
        let mut p10 = [0i8; 34];
        for (i, v) in p10.iter_mut().take(10).enumerate() {
            *v = i as i8;
        }
        // 10 bands to 34: each 10-band value fills a run, in order
        let o = to_bands(&p10, 10, true);
        assert!(o.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!((o[0], o[33]), (0, 9));
    }
}
