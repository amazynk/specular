//! The correction models and their interpolation, from Lensfun's published documentation:
//!
//! - the models: "Lens" group, `lfDistortionModel`, `lfTCAModel`, `lfVignettingModel`
//!   (<https://lensfun.github.io/manual/latest/group__Lens.html>);
//! - the calibration elements: "Lens calibration data format"
//!   (<https://lensfun.github.io/manual/latest/elem_calibration.html>);
//! - the order of the corrections: "How the corrections work"
//!   (<https://lensfun.github.io/manual/latest/corrections.html>);
//! - how the coefficients were measured (Hugin's optimiser and `tca_correct`): the
//!   calibration tutorial (<https://lensfun.github.io/calibration-tutorial/>).
//!
//! # Models (r_u: corrected radius, r_d: radius in the recorded image)
//!
//! - distortion `ptlens`: `r_d = r_u (a r_u³ + b r_u² + c r_u + 1 − a − b − c)`;
//!   `poly3`: `r_d = r_u (1 − k1 + k1 r_u²)` (ptlens with a = c = 0, b = k1);
//!   `poly5`: `r_d = r_u (1 + k1 r_u² + k2 r_u⁴)`.
//! - TCA, applied after distortion to the distorted radius, red and blue planes:
//!   `linear`: `r = r_d k`; `poly3`: `r = r_d (b r_d² + c r_d + v)` (linear is poly3 with
//!   b = c = 0, v = k).
//! - vignetting `pa`: `C_d = C_s / (1 + k1 r² + k2 r⁴ + k3 r⁶)`.
//! - The Adobe models (`acm`) are not in the database and are not read.
//!
//! # Units of the radius
//!
//! Distortion and TCA: the coefficients come from Hugin (panotools), where 1 is half the
//! shorter side of the calibration frame; the `<center>` element's documentation uses the
//! same scale ("the smaller dimension has the value 2"). Vignetting: Hugin's photometric
//! model, where 1 is the corner of the calibration frame (the tutorial's example values only
//! make a falloff, not a sign change, on that scale). The calibration frame is the lens's
//! `<cropfactor>` and `<aspect-ratio>` (default 1.5). On a camera with crop factor `c`, a
//! point at a fraction `ρ` of that camera's half diagonal sits at
//!
//! - `r = ρ · (c_lens / c) · √(1 + aspect²)` for distortion and TCA,
//! - `r = ρ · (c_lens / c)` for vignetting,
//!
//! since the crop factor is the ratio of the 35 mm frame's diagonal to the sensor's.
//!
//! # Interpolation (ours; the documentation only says the parameters are interpolated)
//!
//! Between the calibrated focal lengths each coefficient is linear in the focal length;
//! outside them the nearest calibration holds. Vignetting is also interpolated between
//! apertures (linear in stops, `log2 N`) and focus distances (linear in `1 / distance`),
//! nearest-holding at the ends. A missing focus distance means far away (the database's
//! own "1000" m).

/// A distortion calibration at one focal length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Distortion {
    PtLens { a: f64, b: f64, c: f64 },
    Poly3 { k1: f64 },
    Poly5 { k1: f64, k2: f64 },
}

impl Distortion {
    /// `r_d / r_u` at `ru`.
    pub fn ratio(&self, ru: f64) -> f64 {
        match *self {
            Distortion::PtLens { a, b, c } => {
                a * ru * ru * ru + b * ru * ru + c * ru + 1.0 - a - b - c
            }
            Distortion::Poly3 { k1 } => 1.0 - k1 + k1 * ru * ru,
            Distortion::Poly5 { k1, k2 } => {
                let r2 = ru * ru;
                1.0 + k1 * r2 + k2 * r2 * r2
            }
        }
    }

    /// As ptlens, when it is one (poly3 is).
    fn as_ptlens(&self) -> Option<[f64; 3]> {
        match *self {
            Distortion::PtLens { a, b, c } => Some([a, b, c]),
            Distortion::Poly3 { k1 } => Some([0.0, k1, 0.0]),
            Distortion::Poly5 { .. } => None,
        }
    }

    /// `self` and `o` mixed at `t` (0: self), coefficient by coefficient; the nearest of the
    /// two when the models cannot be mixed.
    pub fn mix(&self, o: &Distortion, t: f64) -> Distortion {
        let l = |x: f64, y: f64| x + (y - x) * t;
        match (self, o) {
            (Distortion::Poly3 { k1: a }, Distortion::Poly3 { k1: b }) => {
                Distortion::Poly3 { k1: l(*a, *b) }
            }
            (Distortion::Poly5 { k1: a1, k2: a2 }, Distortion::Poly5 { k1: b1, k2: b2 }) => {
                Distortion::Poly5 {
                    k1: l(*a1, *b1),
                    k2: l(*a2, *b2),
                }
            }
            _ => match (self.as_ptlens(), o.as_ptlens()) {
                (Some(p), Some(q)) => Distortion::PtLens {
                    a: l(p[0], q[0]),
                    b: l(p[1], q[1]),
                    c: l(p[2], q[2]),
                },
                _ => {
                    if t < 0.5 {
                        *self
                    } else {
                        *o
                    }
                }
            },
        }
    }
}

/// A TCA calibration at one focal length, as poly3 (`linear` is `b = c = 0`, `v = k`):
/// `[b, c, v]` of red, then of blue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tca {
    pub red: [f64; 3],
    pub blue: [f64; 3],
}

impl Tca {
    /// The linear model's `kr`, `kb`.
    pub fn linear(kr: f64, kb: f64) -> Tca {
        Tca {
            red: [0.0, 0.0, kr],
            blue: [0.0, 0.0, kb],
        }
    }

    /// The plane's radius over the distorted radius `rd` (plane 0 red, 2 blue, else 1).
    pub fn ratio(&self, plane: usize, rd: f64) -> f64 {
        let [b, c, v] = match plane {
            0 => self.red,
            2 => self.blue,
            _ => return 1.0,
        };
        b * rd * rd + c * rd + v
    }

    pub fn mix(&self, o: &Tca, t: f64) -> Tca {
        let l = |x: [f64; 3], y: [f64; 3]| std::array::from_fn(|i| x[i] + (y[i] - x[i]) * t);
        Tca {
            red: l(self.red, o.red),
            blue: l(self.blue, o.blue),
        }
    }
}

/// The `pa` model's correction gain `C_d / C_s` at `r`. A denominator at or below 0.05 (a
/// curve that left the frame it was fitted on) is held at 0.05, at most 20× brighter.
pub fn pa_gain(k: [f64; 3], r: f64) -> f64 {
    let r2 = r * r;
    let d = 1.0 + k[0] * r2 + k[1] * r2 * r2 + k[2] * r2 * r2 * r2;
    1.0 / d.max(0.05)
}

/// A distortion calibration and its focal length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistortionCal {
    pub focal: f64,
    pub model: Distortion,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TcaCal {
    pub focal: f64,
    pub model: Tca,
}

/// A `pa` vignetting calibration at a focal length, aperture (f-number) and focus distance
/// (metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VignettingCal {
    pub focal: f64,
    pub aperture: f64,
    pub distance: f64,
    pub k: [f64; 3],
}

/// The two calibrated values around `x` (ascending `xs`, not empty) and the weight of the
/// second: equal indices outside the range.
fn bracket(xs: &[f64], x: f64) -> (usize, usize, f64) {
    let n = xs.len();
    if n == 1 || x <= xs[0] {
        return (0, 0, 0.0);
    }
    if x >= xs[n - 1] {
        return (n - 1, n - 1, 0.0);
    }
    let i = xs.partition_point(|&v| v <= x).max(1);
    let (a, b) = (xs[i - 1], xs[i]);
    let t = if b > a { (x - a) / (b - a) } else { 0.0 };
    (i - 1, i, t)
}

/// Distinct values, ascending.
fn distinct(mut v: Vec<f64>) -> Vec<f64> {
    v.retain(|x| x.is_finite());
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    v
}

/// The distortion at `focal` (see the module documentation), `None` without calibrations.
pub fn distortion_at(cals: &[DistortionCal], focal: f64) -> Option<Distortion> {
    let mut c: Vec<&DistortionCal> = cals.iter().collect();
    c.sort_by(|a, b| a.focal.total_cmp(&b.focal));
    let xs: Vec<f64> = c.iter().map(|d| d.focal).collect();
    if xs.is_empty() {
        return None;
    }
    let (i, j, t) = bracket(&xs, focal);
    Some(c[i].model.mix(&c[j].model, t))
}

/// The TCA at `focal`, `None` without calibrations.
pub fn tca_at(cals: &[TcaCal], focal: f64) -> Option<Tca> {
    let mut c: Vec<&TcaCal> = cals.iter().collect();
    c.sort_by(|a, b| a.focal.total_cmp(&b.focal));
    let xs: Vec<f64> = c.iter().map(|d| d.focal).collect();
    if xs.is_empty() {
        return None;
    }
    let (i, j, t) = bracket(&xs, focal);
    Some(c[i].model.mix(&c[j].model, t))
}

/// The vignetting coefficients at `focal`, `aperture` and `distance` (see the module
/// documentation), `None` without calibrations.
pub fn vignetting_at(
    cals: &[VignettingCal],
    focal: f64,
    aperture: f64,
    distance: f64,
) -> Option<[f64; 3]> {
    let mix = |a: [f64; 3], b: [f64; 3], t: f64| -> [f64; 3] {
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
    };
    // At one focal length and distance: across apertures, linear in stops.
    let at_ad = |set: &[&VignettingCal]| -> [f64; 3] {
        let mut s: Vec<&VignettingCal> = set.to_vec();
        s.sort_by(|a, b| a.aperture.total_cmp(&b.aperture));
        let xs: Vec<f64> = s.iter().map(|v| v.aperture.max(1e-6).log2()).collect();
        let (i, j, t) = bracket(&xs, aperture.max(1e-6).log2());
        mix(s[i].k, s[j].k, t)
    };
    // At one focal length: across distances, linear in 1/distance.
    let at_f = |set: &[&VignettingCal]| -> [f64; 3] {
        let inv = |d: f64| 1.0 / d.max(1e-3);
        // Ascending in 1/distance: far first.
        let mut ds = distinct(set.iter().map(|v| inv(v.distance)).collect());
        ds.sort_by(f64::total_cmp);
        let (i, j, t) = bracket(&ds, inv(distance));
        let of = |x: f64| -> [f64; 3] {
            let s: Vec<&VignettingCal> = set
                .iter()
                .copied()
                .filter(|v| (inv(v.distance) - x).abs() < 1e-9)
                .collect();
            at_ad(&s)
        };
        let a = of(ds[i]);
        if i == j { a } else { mix(a, of(ds[j]), t) }
    };
    let fs = distinct(cals.iter().map(|v| v.focal).collect());
    if fs.is_empty() {
        return None;
    }
    let (i, j, t) = bracket(&fs, focal);
    let of = |f: f64| -> [f64; 3] {
        let s: Vec<&VignettingCal> = cals.iter().filter(|v| (v.focal - f).abs() < 1e-9).collect();
        at_f(&s)
    };
    let a = of(fs[i]);
    Some(if i == j { a } else { mix(a, of(fs[j]), t) })
}

/// A lens's corrections for one shot, ready to evaluate in the camera's frame: radii `rho`
/// as fractions of its half diagonal (see the module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub distortion: Option<Distortion>,
    pub tca: Option<Tca>,
    pub vignetting: Option<[f64; 3]>,
    /// `r` of distortion and TCA per unit `rho`.
    pub geometry_scale: f64,
    /// `r` of vignetting per unit `rho`.
    pub vignetting_scale: f64,
}

impl Profile {
    /// The profile of a lens calibrated at crop factor `lens_crop` and aspect ratio
    /// `lens_aspect` (long over short side) on a camera of crop factor `camera_crop`.
    pub fn new(
        distortion: Option<Distortion>,
        tca: Option<Tca>,
        vignetting: Option<[f64; 3]>,
        lens_crop: f64,
        lens_aspect: f64,
        camera_crop: f64,
    ) -> Profile {
        let k = if lens_crop > 0.0 && camera_crop > 0.0 {
            lens_crop / camera_crop
        } else {
            1.0
        };
        let aspect = if lens_aspect >= 1.0 { lens_aspect } else { 1.5 };
        Profile {
            distortion,
            tca,
            vignetting,
            geometry_scale: k * (1.0 + aspect * aspect).sqrt(),
            vignetting_scale: k,
        }
    }

    /// The back-map factor of `plane` (0 red, 1 green, 2 blue) at corrected radius `rho`:
    /// the recorded image is read at `rho · factor` (distortion, then TCA on the distorted
    /// radius, as "How the corrections work" orders the back-map).
    pub fn factor(&self, plane: usize, rho: f64, distortion: bool, tca: bool) -> f64 {
        let ru = rho * self.geometry_scale;
        let mut k = 1.0;
        if distortion && let Some(d) = &self.distortion {
            k = d.ratio(ru);
        }
        if tca && let Some(t) = &self.tca {
            k *= t.ratio(plane, ru * k);
        }
        k
    }

    /// The vignetting correction gain at radius `rho` of the recorded image.
    pub fn vignette_gain(&self, rho: f64) -> f64 {
        self.vignetting
            .map_or(1.0, |k| pa_gain(k, rho * self.vignetting_scale))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn models_by_hand() {
        // ptlens a=0.02964 b=-0.07853 c=0.02943 at r_u = 0.5:
        // 0.02964·0.125 − 0.07853·0.25 + 0.02943·0.5 + 1 − 0.02964 + 0.07853 − 0.02943
        // = 0.003705 − 0.0196325 + 0.014715 + 1.01946 = 1.0182475
        let d = Distortion::PtLens {
            a: 0.02964,
            b: -0.07853,
            c: 0.02943,
        };
        assert!(close(d.ratio(0.5), 1.0182475, 1e-12), "{}", d.ratio(0.5));
        // Every ptlens model leaves r = 1 where it is.
        assert!(close(d.ratio(1.0), 1.0, 1e-12));
        // poly3 k1 = −0.01919 at r_u = 1.5: 1 + 0.01919 − 0.01919·2.25 = 0.9760125.
        let p = Distortion::Poly3 { k1: -0.01919 };
        assert!(close(p.ratio(1.5), 0.9760125, 1e-12));
        // poly5 k1 = 0.1, k2 = −0.02 at 2: 1 + 0.4 − 0.32 = 1.08.
        let q = Distortion::Poly5 { k1: 0.1, k2: -0.02 };
        assert!(close(q.ratio(2.0), 1.08, 1e-12));
        // TCA poly3 br = 0.0000916, vr = 0.9999904 at 1.2: 0.0000916·1.44 + 0.9999904.
        let t = Tca {
            red: [0.0000916, 0.0, 0.9999904],
            blue: [-0.0002397, 0.0, 1.000649],
        };
        assert!(close(t.ratio(0, 1.2), 1.000122304, 1e-12));
        assert!(close(t.ratio(2, 1.2), 1.000303832, 1e-12));
        assert_eq!(t.ratio(1, 1.2), 1.0);
        assert_eq!(Tca::linear(1.001, 0.999).ratio(0, 3.0), 1.001);
        // pa k = (−0.44227, 0.2599, −0.09436) at the corner: 1/(1 − 0.44227 + 0.2599 − 0.09436).
        let g = pa_gain([-0.44227, 0.2599, -0.09436], 1.0);
        assert!(close(g, 1.0 / 0.72327, 1e-12));
        assert_eq!(pa_gain([0.0; 3], 0.7), 1.0);
        // A runaway curve is held at 20×.
        assert_eq!(pa_gain([-2.0, 0.0, 0.0], 1.0), 20.0);
    }

    #[test]
    fn focal_interpolation() {
        let cals = [
            DistortionCal {
                focal: 24.0,
                model: Distortion::PtLens {
                    a: 0.02,
                    b: -0.08,
                    c: 0.03,
                },
            },
            DistortionCal {
                focal: 35.0,
                model: Distortion::PtLens {
                    a: 0.0,
                    b: -0.02,
                    c: 0.01,
                },
            },
        ];
        // 29.5 mm is half way.
        match distortion_at(&cals, 29.5) {
            Some(Distortion::PtLens { a, b, c }) => {
                assert!(close(a, 0.01, 1e-12) && close(b, -0.05, 1e-12) && close(c, 0.02, 1e-12));
            }
            o => panic!("{o:?}"),
        }
        // Outside the range: the nearest.
        assert_eq!(distortion_at(&cals, 10.0), Some(cals[0].model));
        assert_eq!(distortion_at(&cals, 200.0), Some(cals[1].model));
        assert_eq!(distortion_at(&[], 50.0), None);
        // poly3 mixes with ptlens as ptlens.
        let mixed = Distortion::Poly3 { k1: -0.04 }.mix(
            &Distortion::PtLens {
                a: 0.02,
                b: 0.0,
                c: 0.0,
            },
            0.25,
        );
        assert_eq!(
            mixed,
            Distortion::PtLens {
                a: 0.005,
                b: -0.03,
                c: 0.0
            }
        );
        let tc = [
            TcaCal {
                focal: 10.0,
                model: Tca::linear(1.0, 1.0),
            },
            TcaCal {
                focal: 20.0,
                model: Tca::linear(1.002, 0.998),
            },
        ];
        let t = tca_at(&tc, 15.0).expect("tca");
        assert!(close(t.red[2], 1.001, 1e-12) && close(t.blue[2], 0.999, 1e-12));
    }

    #[test]
    fn vignetting_interpolation() {
        let v = |focal, aperture, distance, k1| VignettingCal {
            focal,
            aperture,
            distance,
            k: [k1, 0.0, 0.0],
        };
        let cals = [
            v(24.0, 2.8, 1000.0, -0.6),
            v(24.0, 5.6, 1000.0, -0.2),
            v(24.0, 2.8, 1.0, -0.8),
            v(24.0, 5.6, 1.0, -0.4),
            v(70.0, 2.8, 1000.0, -0.3),
            v(70.0, 5.6, 1000.0, -0.1),
        ];
        // f/4 is half a stop pair: log2(4) is half way between log2(2.8) and log2(5.6).
        let t = (4f64.log2() - 2.8f64.log2()) / (5.6f64.log2() - 2.8f64.log2());
        let k = vignetting_at(&cals, 24.0, 4.0, 1000.0).expect("k")[0];
        assert!(close(k, -0.6 + 0.4 * t, 1e-12), "{k}");
        // 2 m is half way between 1 m and 1000 m in 1/d? 1/2 between 1 and 0.001.
        let s = (1.0 - 0.5) / (1.0 - 0.001);
        let k = vignetting_at(&cals, 24.0, 2.8, 2.0).expect("k")[0];
        assert!(close(k, -0.8 + (-0.6 + 0.8) * s, 1e-12), "{k}");
        // 47 mm is half way between 24 and 70, at f/2.8 far: −0.45 (70 mm has no near data).
        let k = vignetting_at(&cals, 47.0, 2.8, 1000.0).expect("k")[0];
        assert!(close(k, -0.45, 1e-12), "{k}");
        // Outside every range: the nearest corner.
        let k = vignetting_at(&cals, 10.0, 1.4, 0.1).expect("k")[0];
        assert!(close(k, -0.8, 1e-12), "{k}");
        assert_eq!(vignetting_at(&[], 1.0, 1.0, 1.0), None);
    }

    #[test]
    fn profile_scales_for_the_crop_factor() {
        // A lens calibrated on full frame (3:2) used on a 1.6 crop body: ρ = 1 on the small
        // sensor is r = (1/1.6) · √(1 + 2.25) in the calibration's units.
        let p = Profile::new(
            Some(Distortion::Poly3 { k1: 0.01 }),
            None,
            Some([-0.5, 0.0, 0.0]),
            1.0,
            1.5,
            1.6,
        );
        let r = 3.25f64.sqrt() / 1.6;
        assert!(close(p.geometry_scale, r, 1e-12));
        assert!(close(
            p.factor(1, 1.0, true, true),
            1.0 - 0.01 + 0.01 * r * r,
            1e-12
        ));
        assert_eq!(p.factor(1, 1.0, false, true), 1.0);
        // Vignetting: the corner of the small sensor is 1/1.6 of the calibration's corner.
        let rv = 1.0 / 1.6;
        assert!(close(
            p.vignette_gain(1.0),
            1.0 / (1.0 - 0.5 * rv * rv),
            1e-12
        ));
        // TCA after distortion: on the distorted radius.
        let q = Profile {
            tca: Some(Tca {
                red: [0.001, 0.0, 1.0],
                blue: [0.0, 0.0, 1.0],
            }),
            ..p.clone()
        };
        let k = 1.0 - 0.01 + 0.01 * r * r;
        let rd = r * k;
        assert!(close(
            q.factor(0, 1.0, true, true),
            k * (0.001 * rd * rd + 1.0),
            1e-12
        ));
        assert!(close(q.factor(2, 1.0, true, true), k, 1e-12));
    }
}
