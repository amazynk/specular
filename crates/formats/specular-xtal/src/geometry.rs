//! One detector geometry for every format, and what follows from it: the scattering angle,
//! q and d under a pixel, and resolution rings.
//!
//! Model: a flat detector at `distance` from the sample, perpendicular to the beam before
//! it swings by `two_theta` about the detector's fast (x) axis; the beam centre is where the
//! perpendicular from the sample meets the detector. Pixel coordinates are continuous, from
//! the top-left corner of the first pixel (pixel `i` covers `i..i + 1`), x along the fast
//! axis and y along the slow axis, down. A pixel's position relative to the sample is
//! `((x - bx)·sx, (y - by)·sy, D)` in the detector frame; 2θ is the angle between it and
//! the beam, `q = 4π sin θ / λ` and `d = λ / (2 sin θ)`.

/// X-ray photon energy times wavelength, keV·Å (CODATA 2018: hc).
pub const HC_KEV_ANGSTROM: f64 = 12.398_419_843_320_026;

/// Experiment and detector parameters from a header. Every field is optional: files often
/// leave some out (or write zero for unknown).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Geometry {
    /// X-ray wavelength, Å.
    pub wavelength: Option<f64>,
    /// Sample to detector distance, mm.
    pub distance: Option<f64>,
    /// Beam centre in pixels (x fast, y slow), from the top-left corner of the first pixel.
    pub beam: Option<(f64, f64)>,
    /// Pixel size (x, y), mm.
    pub pixel: Option<(f64, f64)>,
    /// Detector swing (2θ arm), degrees.
    pub two_theta: Option<f64>,
    /// Goniometer angles at the start of the exposure, degrees.
    pub phi: Option<f64>,
    pub omega: Option<f64>,
    pub chi: Option<f64>,
    pub kappa: Option<f64>,
    /// Rotation (oscillation) start and width, degrees, and the axis it is about.
    pub osc_start: Option<f64>,
    pub osc_width: Option<f64>,
    pub osc_axis: Option<String>,
    /// Exposure time, seconds.
    pub exposure: Option<f64>,
    /// The count at which a pixel is saturated (overloaded).
    pub overload: Option<f64>,
    /// A curved detector (R-AXIS RAPID cylindrical image plate): the flat model does not
    /// apply, so no q, d or rings.
    pub curved: bool,
    /// Detector pixels summed into one (x, y), when the header says.
    pub binning: Option<(u32, u32)>,
}

/// What lies under a point of the detector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scatter {
    /// Scattering angle 2θ, degrees.
    pub two_theta: f64,
    /// Momentum transfer, Å⁻¹.
    pub q: f64,
    /// Lattice spacing, Å (infinite at the beam centre).
    pub d: f64,
}

/// A resolution ring: the points at one d-spacing, as polylines in pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    /// d-spacing, Å.
    pub d: f64,
    /// Scattering angle, degrees.
    pub two_theta: f64,
    pub segments: Vec<Vec<(f64, f64)>>,
}

/// The parameters the model needs, all present and sane.
#[derive(Debug, Clone, Copy)]
struct Model {
    lambda: f64,
    dist: f64,
    bx: f64,
    by: f64,
    sx: f64,
    sy: f64,
    sin_t: f64,
    cos_t: f64,
}

fn pos(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite() && *v > 0.0)
}

/// d-spacings rings are drawn at, Å: round numbers, largest first.
const NICE_D: &[f64] = &[
    100.0, 50.0, 30.0, 20.0, 15.0, 10.0, 8.0, 6.0, 5.0, 4.0, 3.5, 3.0, 2.5, 2.2, 2.0, 1.8, 1.6,
    1.5, 1.4, 1.3, 1.2, 1.1, 1.0, 0.9, 0.8, 0.7, 0.6, 0.5,
];
/// Rings drawn at most.
const MAX_RINGS: usize = 6;
/// Points per full ring.
const RING_STEPS: usize = 720;

impl Geometry {
    fn model(&self) -> Option<Model> {
        if self.curved {
            return None;
        }
        let (bx, by) = self.beam.filter(|b| b.0.is_finite() && b.1.is_finite())?;
        let (sx, sy) = self.pixel?;
        let t = self.two_theta.unwrap_or(0.0).to_radians();
        Some(Model {
            lambda: pos(self.wavelength)?,
            dist: pos(self.distance)?,
            bx,
            by,
            sx: pos(Some(sx))?,
            sy: pos(Some(sy))?,
            sin_t: t.sin(),
            cos_t: t.cos(),
        })
    }

    /// Whether the header gives everything q, d and 2θ need (wavelength, distance, beam
    /// centre, pixel size) and the detector is flat.
    pub fn complete(&self) -> bool {
        self.model().is_some()
    }

    /// Photon energy, keV.
    pub fn energy_kev(&self) -> Option<f64> {
        pos(self.wavelength).map(|l| HC_KEV_ANGSTROM / l)
    }

    /// 2θ, q and d at pixel coordinates `(x, y)` (continuous; a pixel's centre is at
    /// `i + 0.5`).
    pub fn at(&self, x: f64, y: f64) -> Option<Scatter> {
        let m = self.model()?;
        let (lx, ly, lz) = m.lab(x, y);
        let r = (lx * lx + ly * ly + lz * lz).sqrt();
        if r <= 0.0 {
            return None;
        }
        let tth = (lz / r).clamp(-1.0, 1.0).acos();
        let s = (tth / 2.0).sin();
        Some(Scatter {
            two_theta: tth.to_degrees(),
            q: 4.0 * std::f64::consts::PI * s / m.lambda,
            d: if s > 0.0 {
                m.lambda / (2.0 * s)
            } else {
                f64::INFINITY
            },
        })
    }

    /// The smallest and largest 2θ (degrees) over a `w` × `h` detector.
    pub fn two_theta_range(&self, w: u32, h: u32) -> Option<(f64, f64)> {
        let m = self.model()?;
        let (w, h) = (f64::from(w), f64::from(h));
        let mut lo = f64::INFINITY;
        let mut hi: f64 = 0.0;
        // The edges (the extremes of a smooth function on a rectangle lie on its boundary
        // or at the beam centre).
        const N: usize = 64;
        for k in 0..=N {
            let f = k as f64 / N as f64;
            for (x, y) in [(f * w, 0.0), (f * w, h), (0.0, f * h), (w, f * h)] {
                if let Some(s) = self.at(x, y) {
                    lo = lo.min(s.two_theta);
                    hi = hi.max(s.two_theta);
                }
            }
        }
        // The point nearest the beam: the beam centre itself when it is on the detector
        // (2θ there is the detector swing).
        let (cx, cy) = (m.bx.clamp(0.0, w), m.by.clamp(0.0, h));
        if let Some(s) = self.at(cx, cy) {
            lo = lo.min(s.two_theta);
            hi = hi.max(s.two_theta);
        }
        (lo.is_finite() && hi > 0.0).then_some((lo, hi))
    }

    /// d (Å) at the edge of a `w` × `h` detector nearest the beam (the resolution recorded
    /// all round, for a centred beam) and at its farthest corner.
    pub fn edge_resolution(&self, w: u32, h: u32) -> Option<(f64, f64)> {
        let m = self.model()?;
        let (wf, hf) = (f64::from(w), f64::from(h));
        let by = m.by.clamp(0.0, hf);
        let bx = m.bx.clamp(0.0, wf);
        let edge = [(0.0, by), (wf, by), (bx, 0.0), (bx, hf)]
            .iter()
            .filter_map(|&(x, y)| self.at(x, y))
            .map(|s| s.d)
            .fold(0.0f64, f64::max);
        let corner = [(0.0, 0.0), (wf, 0.0), (0.0, hf), (wf, hf)]
            .iter()
            .filter_map(|&(x, y)| self.at(x, y))
            .map(|s| s.d)
            .fold(f64::INFINITY, f64::min);
        (edge.is_finite() && edge > 0.0 && corner.is_finite()).then_some((edge, corner))
    }

    /// The ring at d-spacing `d` (Å) on a `w` × `h` detector: polylines in pixel
    /// coordinates, broken where the cone misses the detector plane or runs far off the
    /// image. Empty when no part of it is on the image.
    pub fn ring(&self, d: f64, w: u32, h: u32) -> Option<Ring> {
        let m = self.model()?;
        let s = m.lambda / (2.0 * d);
        if d.is_nan() || d <= 0.0 || !(0.0..1.0).contains(&s) {
            return None;
        }
        let tth = 2.0 * s.asin();
        let (st, ct) = (tth.sin(), tth.cos());
        let (wf, hf) = (f64::from(w), f64::from(h));
        let far = 4.0 * (wf + hf);
        let mut segments = Vec::new();
        let mut cur: Vec<(f64, f64)> = Vec::new();
        let mut on_image = false;
        for k in 0..=RING_STEPS {
            let a = k as f64 / RING_STEPS as f64 * std::f64::consts::TAU;
            // Lab direction, then into the detector frame (inverse swing about x).
            let (ux, uy, uz) = (st * a.cos(), st * a.sin(), ct);
            let dy = uy * m.cos_t + uz * m.sin_t;
            let dz = -uy * m.sin_t + uz * m.cos_t;
            let p = (dz > 1e-9).then(|| {
                let t = m.dist / dz;
                (m.bx + t * ux / m.sx, m.by + t * dy / m.sy)
            });
            match p.filter(|p| p.0.abs() < far && p.1.abs() < far) {
                Some(p) => {
                    on_image |= p.0 >= 0.0 && p.1 >= 0.0 && p.0 <= wf && p.1 <= hf;
                    cur.push(p);
                }
                None => {
                    if cur.len() >= 2 {
                        segments.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                }
            }
        }
        if cur.len() >= 2 {
            segments.push(cur);
        }
        // A closed ring that was broken at the start: join its two ends.
        if segments.len() >= 2 {
            let first = &segments[0];
            let last = &segments[segments.len() - 1];
            if first.first() == last.last() {
                let first = segments.remove(0);
                if let Some(l) = segments.last_mut() {
                    l.extend(first.into_iter().skip(1));
                }
            }
        }
        (on_image && !segments.is_empty()).then(|| Ring {
            d,
            two_theta: tth.to_degrees(),
            segments,
        })
    }

    /// A few resolution rings at round d-spacings, spread over the detector's 2θ range,
    /// low resolution (large d) first.
    pub fn rings(&self, w: u32, h: u32) -> Vec<Ring> {
        let Some(m) = self.model() else {
            return Vec::new();
        };
        let Some((lo, hi)) = self.two_theta_range(w, h) else {
            return Vec::new();
        };
        let tth = |d: f64| {
            let s = m.lambda / (2.0 * d);
            (s < 1.0).then(|| 2.0 * s.asin().to_degrees())
        };
        let candidates: Vec<(f64, f64)> = NICE_D
            .iter()
            .filter_map(|&d| tth(d).map(|t| (d, t)))
            .filter(|&(_, t)| t > lo && t <= hi)
            .collect();
        let mut chosen: Vec<f64> = Vec::new();
        for k in 1..=MAX_RINGS {
            let target = lo + (hi - lo) * k as f64 / (MAX_RINGS as f64 + 0.5);
            if let Some(&(d, _)) = candidates
                .iter()
                .min_by(|a, b| (a.1 - target).abs().total_cmp(&(b.1 - target).abs()))
                && !chosen.contains(&d)
            {
                chosen.push(d);
            }
        }
        chosen.sort_by(|a, b| b.total_cmp(a));
        chosen
            .into_iter()
            .filter_map(|d| self.ring(d, w, h))
            .collect()
    }
}

impl Model {
    /// Lab-frame position (mm) of pixel coordinates `(x, y)`, beam along +z.
    fn lab(&self, x: f64, y: f64) -> (f64, f64, f64) {
        let px = (x - self.bx) * self.sx;
        let py = (y - self.by) * self.sy;
        let ly = py * self.cos_t - self.dist * self.sin_t;
        let lz = py * self.sin_t + self.dist * self.cos_t;
        (px, ly, lz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pilatus() -> Geometry {
        Geometry {
            wavelength: Some(1.0332),
            distance: Some(300.0),
            beam: Some((1230.1, 1310.5)),
            pixel: Some((0.172, 0.172)),
            ..Default::default()
        }
    }

    #[test]
    fn scatter_at_pixel() {
        let g = pilatus();
        assert!(g.complete());
        let s = g.at(1230.1, 1310.5).unwrap();
        assert!(s.two_theta.abs() < 1e-9 && s.d.is_infinite() && s.q == 0.0);
        // 100 px right of the beam: tan 2θ = 17.2 / 300.
        let s = g.at(1330.1, 1310.5).unwrap();
        let tth = (17.2f64 / 300.0).atan();
        assert!((s.two_theta - tth.to_degrees()).abs() < 1e-9);
        let d = 1.0332 / (2.0 * (tth / 2.0).sin());
        assert!((s.d - d).abs() < 1e-9);
        assert!((s.q - 2.0 * std::f64::consts::PI / d).abs() < 1e-12);
        assert!((g.energy_kev().unwrap() - 12.0).abs() < 0.01);
    }

    #[test]
    fn swung_detector() {
        let g = Geometry {
            two_theta: Some(30.0),
            ..pilatus()
        };
        // The beam centre (the foot of the perpendicular) is at 2θ = swing.
        let s = g.at(1230.1, 1310.5).unwrap();
        assert!((s.two_theta - 30.0).abs() < 1e-9);
        // Rings still pass through pixels at their 2θ.
        let r = g.ring(3.0, 2463, 2527).unwrap();
        for seg in &r.segments {
            for &(x, y) in seg.iter().step_by(37) {
                let s = g.at(x, y).unwrap();
                assert!((s.d - 3.0).abs() < 1e-6, "{x} {y} {}", s.d);
            }
        }
    }

    #[test]
    fn rings_spread() {
        let g = pilatus();
        let rings = g.rings(2463, 2527);
        assert!((3..=MAX_RINGS).contains(&rings.len()), "{}", rings.len());
        assert!(rings.windows(2).all(|w| w[0].d > w[1].d));
        let (edge, corner) = g.edge_resolution(2463, 2527).unwrap();
        assert!(edge > corner);
        assert!(rings.iter().all(|r| r.d >= corner * 0.999));
        // Centred beam: whole circles, one segment each, closed.
        let r = &rings[0];
        assert_eq!(r.segments.len(), 1);
        let s = &r.segments[0];
        let (a, b) = (s[0], s[s.len() - 1]);
        assert!((a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
    }

    #[test]
    fn incomplete() {
        let g = Geometry {
            distance: None,
            ..pilatus()
        };
        assert!(!g.complete());
        assert!(g.at(0.0, 0.0).is_none());
        assert!(g.rings(10, 10).is_empty());
        let g = Geometry {
            curved: true,
            ..pilatus()
        };
        assert!(g.at(0.0, 0.0).is_none());
    }
}
