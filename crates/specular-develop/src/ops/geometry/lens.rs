//! Lens models as radial functions (the file's own data,
//! Lensfun profiles and manual controls).
//!
//! Everything here works about the centre of the recorded image, with radii `ρ` as fractions
//! of its half diagonal (1 = a corner). A model gives, per colour plane, the **back-map
//! factor** `F(ρ)`: a corrected point at radius `ρ` reads the source at `ρ · F(ρ)` along the
//! same ray. Vignetting is a gain `G(ρ)` at the source radius.
//!
//! - **Sony** (ARW raw IFD 0x7037 / 0x7035 / 0x7032): knots evenly from the centre to the
//!   corner, linear between them. Distortion `g(ρ) = 1 + d(ρ)/2^14`, red and blue
//!   `g(ρ) · (1 + c(ρ)/2^21)`, all at the corrected radius (published by Y. Galka,
//!   stannum.io, and in the pixls.us thread "Sony RAW chromatic aberration correction
//!   model"). The camera divides by the largest distortion factor on the frame's edge so
//!   the corrected frame fills the picture; that is part of the factor here. Checked against
//!   the cameras' JPEGs (`tests/lens_oracle.rs`). Vignetting: [`MakerModel::vignette_gain`]
//!   evaluates the proposed reading (per knot `s = k·v/2^13`, gain `2^(1 − 2^s)`, linear in
//!   ρ), but it is **not applied** ([`MakerModel::has_vignetting`] is false for Sony): on
//!   the corpus files the tables are positive and this gain darkens the corners by 0.6 to
//!   1.1 EV, while the cameras' JPEGs match the raw's own falloff within 0.06 EV
//!   (`sony_vignetting_against_the_jpeg`), so neither the sign nor the scale is confirmed.
//!   Both corpus files were shot with the camera's vignetting correction on (maker note
//!   0x2011 "Auto") and their JPEGs still match the raw's falloff (within 0.01–0.06 EV in
//!   every ring), which fits public reports that the camera corrects the raw data itself:
//!   applying the table (either sign) would move the corners 0.5–1 EV away from the
//!   camera's own picture. Left unapplied until a file with the correction off says more.
//! - **Fujifilm** (RAF FujiIFD 0xf00b / 0xf00f / 0xf010): knots evenly spaced in `ρ²` (a knot
//!   at 0 added), linear in `ρ²`. Distortion in percent at the source radius: a point the
//!   lens put at `ρs` belongs at `ρs / (1 + D(ρs)/100)`. Vignetting in percent of the
//!   centre's illumination, corrected by `100 / V(ρs)`. Red and blue: relative radial shifts
//!   `ρs · (1 + c(ρs))`.
//! - **Manual**: distortion `1 − k ρ²` (k = slider × 0.002, positive straightens barrel
//!   distortion), vignetting `2^(EV · ρ²)` at the source radius, and red/blue scales
//!   (percent of the radius).
//! - **Lensfun** (raws without data of their own): a profile from Lensfun's database
//!   ([`specular_lensfun::model::Profile`]): back-map factor `r_d / r_u` of its distortion,
//!   then its TCA on the distorted radius; vignetting `1 / (1 + k1 r² + k2 r⁴ + k3 r⁶)` at
//!   the source radius (<https://lensfun.github.io/manual/latest/group__Lens.html>).
//! - **Fill**: an automatic zoom so the corrected frame shows no empty edges (Sony's own
//!   normalisation by the largest edge factor is the same thing).

use std::sync::Arc;

use specular_lensfun::model::Profile;
use specular_raw::sensor::{LensCorrection, LensMaker};

/// Samples of a radial table over `0..=RHO_MAX`.
pub const TABLE_N: usize = 1024;
/// Largest radius a table covers (corners of a zoomed-out or rotated frame stay inside).
pub const RHO_MAX: f64 = 2.0;

/// Interpolates `vals` at the knots `xs` (ascending) linearly at `x`, holding the end values.
fn interp(xs: &[f64], vals: &[f64], x: f64) -> f64 {
    let n = xs.len().min(vals.len());
    if n == 0 {
        return 0.0;
    }
    if x <= xs[0] {
        return vals[0];
    }
    for i in 1..n {
        if x <= xs[i] {
            let t = (x - xs[i - 1]) / (xs[i] - xs[i - 1]).max(1e-12);
            return vals[i - 1] + t * (vals[i] - vals[i - 1]);
        }
    }
    // Past the last knot: continue the last segment's slope (corners of rotated frames).
    if n >= 2 {
        let t = (x - xs[n - 1]) / (xs[n - 1] - xs[n - 2]).max(1e-12);
        return vals[n - 1] + t * (vals[n - 1] - vals[n - 2]);
    }
    vals[n - 1]
}

/// The camera's model or a Lensfun profile, ready to evaluate (see the module
/// documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct MakerModel {
    /// A Lensfun profile instead of the maker's tables (`lens` is then empty).
    lensfun: Option<Arc<Profile>>,
    lens: LensCorrection,
    /// Fujifilm distortion: (corrected ρ, source ρ) pairs, ascending, for inverting.
    fuji_inverse: Vec<(f64, f64)>,
    /// Sony: the largest distortion factor on the frame's edge.
    sony_edge: f64,
}

impl MakerModel {
    /// The model of `lens` on a `w`×`h` frame.
    pub fn new(lens: &LensCorrection, w: f64, h: f64) -> MakerModel {
        let mut m = MakerModel {
            lensfun: None,
            lens: lens.clone(),
            fuji_inverse: Vec::new(),
            sony_edge: 1.0,
        };
        if lens.maker == LensMaker::Sony && lens.distortion.is_some() {
            let r = (w * w + h * h).sqrt() / 2.0;
            let n = 256;
            m.sony_edge = (0..=n)
                .flat_map(|i| {
                    let t = i as f64 / n as f64 - 0.5;
                    [(t * w, h / 2.0), (w / 2.0, t * h)]
                })
                .map(|(x, y)| m.sony_distortion((x * x + y * y).sqrt() / r))
                .fold(f64::MIN, f64::max)
                .max(1e-6);
        }
        if lens.maker == LensMaker::Fujifilm && lens.distortion.is_some() {
            let n = 4096;
            let mut last = -1.0;
            for i in 0..=n {
                let s = RHO_MAX * 1.2 * i as f64 / n as f64;
                let u = s / (1.0 + m.fuji_table(lens.distortion.as_deref(), s, 0.0) / 100.0);
                if u > last {
                    m.fuji_inverse.push((u, s));
                    last = u;
                }
            }
        }
        m
    }

    /// A Lensfun profile's model.
    pub fn lensfun(profile: Arc<Profile>) -> MakerModel {
        MakerModel {
            lensfun: Some(profile),
            lens: LensCorrection {
                maker: LensMaker::Sony,
                knots: Vec::new(),
                distortion: None,
                vignetting: None,
                ca_red: None,
                ca_blue: None,
                half_diagonal_px: None,
            },
            fuji_inverse: Vec::new(),
            sony_edge: 1.0,
        }
    }

    /// The maker whose tables these are (`None`: a Lensfun profile).
    pub fn maker(&self) -> Option<LensMaker> {
        self.lensfun.is_none().then_some(self.lens.maker)
    }

    pub fn has_distortion(&self) -> bool {
        match &self.lensfun {
            Some(p) => p.distortion.is_some(),
            None => self.lens.distortion.is_some(),
        }
    }

    pub fn has_ca(&self) -> bool {
        match &self.lensfun {
            Some(p) => p.tca.is_some(),
            None => self.lens.ca_red.is_some() && self.lens.ca_blue.is_some(),
        }
    }

    /// Whether vignetting data is present and applied (not Sony's, see the module docs).
    pub fn has_vignetting(&self) -> bool {
        match &self.lensfun {
            Some(p) => p.vignetting.is_some(),
            None => self.lens.vignetting.is_some() && self.lens.maker != LensMaker::Sony,
        }
    }

    /// Fujifilm's distortion percent at source radius `rho` (tests).
    #[cfg(test)]
    pub fn factor_table_distortion(&self, rho: f64) -> f64 {
        self.fuji_table(self.lens.distortion.as_deref(), rho, 0.0)
    }

    fn sony_distortion(&self, rho: f64) -> f64 {
        1.0 + self.sony_table(self.lens.distortion.as_deref(), rho) / 16384.0
    }

    /// A Fujifilm table at radius `rho` (a knot at 0 with value `at0`, linear in ρ²).
    fn fuji_table(&self, vals: Option<&[f64]>, rho: f64, at0: f64) -> f64 {
        let Some(v) = vals else { return at0 };
        let mut xs = vec![0.0];
        xs.extend(self.lens.knots.iter().map(|k| k * k));
        let mut ys = vec![at0];
        ys.extend_from_slice(v);
        interp(&xs, &ys, rho * rho)
    }

    fn sony_table(&self, vals: Option<&[f64]>, rho: f64) -> f64 {
        vals.map_or(0.0, |v| interp(&self.lens.knots, v, rho))
    }

    /// The back-map factor of `plane` (0 red, 1 green, 2 blue) at corrected radius `rho`,
    /// with distortion and/or colour as asked.
    pub fn factor(&self, plane: usize, rho: f64, distortion: bool, ca: bool) -> f64 {
        if let Some(p) = &self.lensfun {
            return p.factor(plane, rho, distortion, ca);
        }
        let l = &self.lens;
        let ca_vals = match plane {
            0 => l.ca_red.as_deref(),
            2 => l.ca_blue.as_deref(),
            _ => None,
        }
        .filter(|_| ca);
        match l.maker {
            LensMaker::Sony => {
                let g = if distortion && l.distortion.is_some() {
                    self.sony_distortion(rho) / self.sony_edge
                } else {
                    1.0
                };
                g * (1.0 + self.sony_table(ca_vals, rho) / 2_097_152.0)
            }
            LensMaker::Fujifilm => {
                let s = if distortion && l.distortion.is_some() {
                    interp_pairs(&self.fuji_inverse, rho)
                } else {
                    rho
                };
                let c = 1.0 + self.fuji_table(ca_vals, s, 0.0);
                if rho <= 1e-9 {
                    if distortion && l.distortion.is_some() {
                        1.0 / (1.0 + self.fuji_table(l.distortion.as_deref(), 0.0, 0.0) / 100.0) * c
                    } else {
                        c
                    }
                } else {
                    s * c / rho
                }
            }
        }
    }

    /// The vignetting correction gain at source radius `rho` (Sony: the proposed reading,
    /// for the oracle; not applied).
    pub fn vignette_gain(&self, rho: f64) -> f64 {
        if let Some(p) = &self.lensfun {
            return p.vignette_gain(rho);
        }
        let l = &self.lens;
        match l.maker {
            LensMaker::Sony => match l.vignetting.as_deref() {
                Some(v) => {
                    let g: Vec<f64> = v.iter().map(|&v| sony_vignette_gain(v)).collect();
                    interp(&l.knots, &g, rho)
                }
                None => 1.0,
            },
            LensMaker::Fujifilm => {
                let v = self.fuji_table(l.vignetting.as_deref(), rho, 100.0);
                if v > 1.0 { 100.0 / v } else { 100.0 }
            }
        }
    }
}

/// Sony's vignetting fine-tune factor `k` (1: the table as recorded).
pub const SONY_VIGNETTE_K: f64 = 1.0;

/// The correction gain of one Sony vignetting table value: `2^(1 − 2^(k·v/2^13))`.
pub fn sony_vignette_gain(v: f64) -> f64 {
    (1.0 - (SONY_VIGNETTE_K * v / 8192.0).exp2()).exp2()
}

/// Interpolates ascending (x, y) pairs at `x`.
fn interp_pairs(p: &[(f64, f64)], x: f64) -> f64 {
    if p.is_empty() {
        return x;
    }
    let i = p.partition_point(|q| q.0 < x);
    if i == 0 {
        return p[0].1 * x / p[0].0.max(1e-12);
    }
    if i >= p.len() {
        let (a, b) = (p[p.len() - 2], p[p.len() - 1]);
        return b.1 + (x - b.0) * (b.1 - a.1) / (b.0 - a.0).max(1e-12);
    }
    let (a, b) = (p[i - 1], p[i]);
    a.1 + (x - a.0) * (b.1 - a.1) / (b.0 - a.0).max(1e-12)
}

/// The manual lens controls (see the module documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Manual {
    /// −100..100; positive straightens barrel distortion.
    pub distortion: f32,
    /// Stops of brightening at the corner, −3..3.
    pub vignette_ev: f32,
    /// Red and blue radial scales in percent of the radius.
    pub tca_red: f32,
    pub tca_blue: f32,
}

impl Manual {
    pub fn distortion_factor(&self, rho: f64) -> f64 {
        1.0 - f64::from(self.distortion) * 0.002 * rho * rho
    }

    pub fn tca_factor(&self, plane: usize) -> f64 {
        1.0 + match plane {
            0 => f64::from(self.tca_red),
            2 => f64::from(self.tca_blue),
            _ => 0.0,
        } / 100.0
    }

    pub fn vignette_gain(&self, rho: f64) -> f64 {
        (f64::from(self.vignette_ev) * rho * rho).exp2()
    }

    pub fn is_identity(&self) -> bool {
        *self == Manual::default()
    }
}

/// The radial part of a warp: per-plane back-map factors and the vignetting gain, sampled
/// into tables for the GPU (see [`Radial::tables`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Radial {
    pub maker: Option<MakerModel>,
    pub use_distortion: bool,
    pub use_ca: bool,
    pub use_vignetting: bool,
    pub manual: Manual,
    /// The fill zoom (1: none); output radius ρ reads corrected radius `zoom · ρ`.
    pub zoom: f64,
}

impl Radial {
    /// Whether the factor is 1 everywhere and the gain is 1.
    pub fn is_identity(&self) -> bool {
        self.zoom == 1.0
            && self.manual.is_identity()
            && self.maker.as_ref().is_none_or(|m| {
                !(self.use_distortion && m.has_distortion())
                    && !(self.use_ca && m.has_ca())
                    && !(self.use_vignetting && m.has_vignetting())
            })
    }

    /// Whether the planes move differently.
    pub fn per_plane(&self) -> bool {
        self.manual.tca_red != 0.0
            || self.manual.tca_blue != 0.0
            || (self.use_ca && self.maker.as_ref().is_some_and(MakerModel::has_ca))
    }

    /// The back-map factor of `plane` at output radius `rho`.
    pub fn factor(&self, plane: usize, rho: f64) -> f64 {
        let r1 = rho * self.zoom;
        let m = self.manual.distortion_factor(r1) * self.manual.tca_factor(plane);
        let r2 = r1 * m;
        let k = match &self.maker {
            Some(mm) if self.use_distortion || self.use_ca => {
                mm.factor(plane, r2, self.use_distortion, self.use_ca)
            }
            _ => 1.0,
        };
        self.zoom * m * k
    }

    /// The vignetting gain at source radius `rho`.
    pub fn gain(&self, rho: f64) -> f64 {
        let mut g = self.manual.vignette_gain(rho);
        if self.use_vignetting
            && let Some(m) = self.maker.as_ref().filter(|m| m.has_vignetting())
        {
            g *= m.vignette_gain(rho);
        }
        g
    }

    /// The largest zoom that leaves no empty edge: every edge point of a `w`×`h` frame
    /// (radii in units of its half diagonal) reads inside the frame, for every plane.
    pub fn fill_zoom(&self, w: f64, h: f64) -> f64 {
        let r = (w * w + h * h).sqrt() / 2.0;
        let inside = |z: f64| {
            let t = Radial {
                zoom: z,
                ..self.clone()
            };
            let n = 64;
            (0..4 * n).all(|i| {
                let f = (i % n) as f64 / n as f64;
                let (x, y) = match i / n {
                    0 => (-w / 2.0 + f * w, -h / 2.0),
                    1 => (w / 2.0, -h / 2.0 + f * h),
                    2 => (w / 2.0 - f * w, h / 2.0),
                    _ => (-w / 2.0, h / 2.0 - f * h),
                };
                let rho = (x * x + y * y).sqrt() / r;
                (0..3).all(|p| {
                    let k = t.factor(p, rho);
                    (x * k).abs() <= w / 2.0 + 1e-9 && (y * k).abs() <= h / 2.0 + 1e-9
                })
            })
        };
        let (mut lo, mut hi) = (0.25, 4.0);
        if !inside(lo) {
            return 1.0;
        }
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if inside(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Tables for the GPU: factors of red, green, blue, then the gain, each [`TABLE_N`]
    /// samples over `0..=RHO_MAX`.
    pub fn tables(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(4 * TABLE_N);
        let at = |i: usize| RHO_MAX * i as f64 / (TABLE_N - 1) as f64;
        for p in 0..3 {
            out.extend((0..TABLE_N).map(|i| self.factor(p, at(i)) as f32));
        }
        out.extend((0..TABLE_N).map(|i| self.gain(at(i)) as f32));
        out
    }
}

/// Linear interpolation in a table of [`TABLE_N`] samples over `0..=RHO_MAX` (the GPU's).
pub fn table_at(t: &[f32], rho: f64) -> f64 {
    let x = (rho / RHO_MAX * (TABLE_N - 1) as f64).clamp(0.0, (TABLE_N - 1) as f64);
    let i = (x.floor() as usize).min(TABLE_N - 2);
    let f = x - i as f64;
    f64::from(t[i]) * (1.0 - f) + f64::from(t[i + 1]) * f
}
