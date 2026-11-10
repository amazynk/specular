//! A LandXML file as a 3D model for the mesh view: each TIN surface's triangles coloured by
//! elevation (the plan's ramp, per vertex), its contours as thin tubes lying on it (index
//! contours thicker and darker), and each alignment as a tube draped over the surfaces
//! (else at its design profile's elevation), parcels' boundaries draped the same way (else
//! at the model's lowest elevation) and survey points as small posts (at their own
//! elevation, else the surface's), in the file's units with the model centred on the origin
//! (survey coordinates are too large for the GPU's 32-bit floats).

use crate::align::profile_elevation;
use crate::{LandXml, Profile, Surface, tin};

/// Contour segments drawn at most over all surfaces.
const MAX_CONTOUR_SEGMENTS: usize = 400_000;
/// Tube radius as a share of the model's plan diagonal: contours, index contours,
/// alignments.
const R_MINOR: f64 = 1.0 / 2500.0;
const R_MAJOR: f64 = 1.0 / 1500.0;
const R_ALIGN: f64 = 1.0 / 600.0;
const MINOR: [u8; 4] = [56, 40, 28, 255];
const MAJOR: [u8; 4] = [28, 18, 10, 255];
const ALIGN: [u8; 4] = [220, 40, 160, 255];
/// Parcels and survey points in the plan's colours; a post's radius and height as shares
/// of the plan diagonal.
const PARCEL: [u8; 4] = [230, 140, 30, 255];
const SURVEY: [u8; 4] = [40, 160, 220, 255];
const R_POST: f64 = 1.0 / 500.0;
const H_POST: f64 = 1.0 / 50.0;
/// Survey points drawn at most.
const MAX_POSTS: usize = 100_000;

/// Triangles with a colour per vertex, centred: a file point `p` is at `p - offset`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub positions: Vec<[f64; 3]>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
    pub offset: [f64; 3],
    /// Surface triangles, contour segments and alignment segments drawn.
    pub triangles: usize,
    pub contour_segments: usize,
    pub alignment_segments: usize,
    /// Parcel boundary segments and survey points drawn.
    pub parcel_segments: usize,
    pub survey_points: usize,
    /// Contours were left out past [`MAX_CONTOUR_SEGMENTS`].
    pub contours_cut: bool,
}

/// Finds the surface triangle under a plan point: a grid of triangle boxes.
struct Locator<'a> {
    s: &'a Surface,
    x0: f64,
    y0: f64,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl<'a> Locator<'a> {
    fn new(s: &'a Surface) -> Option<Locator<'a>> {
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &s.points {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        if !(x1 > x0 && y1 > y0) || s.faces.is_empty() {
            return None;
        }
        // About two triangles a cell.
        let n = ((s.faces.len() as f64 / 2.0).sqrt().ceil() as usize).clamp(1, 2048);
        let cell = ((x1 - x0).max(y1 - y0) / n as f64).max(1e-9);
        let nx = (((x1 - x0) / cell) as usize + 1).min(4096);
        let ny = (((y1 - y0) / cell) as usize + 1).min(4096);
        let mut cells = vec![Vec::new(); nx * ny];
        for (i, f) in s.faces.iter().enumerate() {
            let Some(t) = corners(s, f) else { continue };
            let cx = |x: f64| (((x - x0) / cell) as usize).min(nx - 1);
            let cy = |y: f64| (((y - y0) / cell) as usize).min(ny - 1);
            let (a, b) = (
                cx(t.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min)),
                cx(t.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max)),
            );
            let (c, d) = (
                cy(t.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min)),
                cy(t.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max)),
            );
            for y in c..=d {
                for x in a..=b {
                    cells[y * nx + x].push(i as u32);
                }
            }
        }
        Some(Locator {
            s,
            x0,
            y0,
            cell,
            nx,
            ny,
            cells,
        })
    }

    /// The surface's elevation at a plan point, `None` outside its triangles.
    fn z(&self, p: [f64; 2]) -> Option<f64> {
        let fx = (p[0] - self.x0) / self.cell;
        let fy = (p[1] - self.y0) / self.cell;
        if fx < 0.0 || fy < 0.0 {
            return None;
        }
        let (x, y) = (fx as usize, fy as usize);
        if x >= self.nx || y >= self.ny {
            return None;
        }
        for &i in &self.cells[y * self.nx + x] {
            let Some(t) = corners(self.s, &self.s.faces[i as usize]) else {
                continue;
            };
            let d = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1])
                - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]);
            if d.abs() < 1e-18 {
                continue;
            }
            let u = ((p[0] - t[0][0]) * (t[2][1] - t[0][1])
                - (t[2][0] - t[0][0]) * (p[1] - t[0][1]))
                / d;
            let v = ((t[1][0] - t[0][0]) * (p[1] - t[0][1])
                - (p[0] - t[0][0]) * (t[1][1] - t[0][1]))
                / d;
            let e = 1e-9;
            if u >= -e && v >= -e && u + v <= 1.0 + e {
                return Some(t[0][2] + u * (t[1][2] - t[0][2]) + v * (t[2][2] - t[0][2]));
            }
        }
        None
    }
}

/// The elevation of surface `s` at a plan point, `None` outside its triangles.
pub fn elevation_at(s: &Surface, p: [f64; 2]) -> Option<f64> {
    Locator::new(s)?.z(p)
}

fn corners(s: &Surface, f: &[u32; 3]) -> Option<[[f64; 3]; 3]> {
    Some([
        *s.points.get(f[0] as usize)?,
        *s.points.get(f[1] as usize)?,
        *s.points.get(f[2] as usize)?,
    ])
}

impl Model {
    /// A tube (a triangular prism without caps) of radius `r` from `a` to `b`.
    fn tube(&mut self, a: [f64; 3], b: [f64; 3], r: f64, color: [u8; 4]) {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-12 {
            return;
        }
        let t = [d[0] / len, d[1] / len, d[2] / len];
        // Two directions across the tube: one level (or along x for a vertical tube), one
        // across both.
        let mut u = [-t[1], t[0], 0.0];
        let ul = (u[0] * u[0] + u[1] * u[1]).sqrt();
        if ul < 1e-9 {
            u = [1.0, 0.0, 0.0];
        } else {
            u = [u[0] / ul, u[1] / ul, 0.0];
        }
        let w = [
            t[1] * u[2] - t[2] * u[1],
            t[2] * u[0] - t[0] * u[2],
            t[0] * u[1] - t[1] * u[0],
        ];
        let base = self.positions.len() as u32;
        for k in 0..3 {
            let ang = std::f64::consts::TAU * f64::from(k) / 3.0 + std::f64::consts::FRAC_PI_2;
            let (c, s) = (ang.cos() * r, ang.sin() * r);
            let o = [
                u[0] * c + w[0] * s,
                u[1] * c + w[1] * s,
                u[2] * c + w[2] * s,
            ];
            for p in [a, b] {
                self.positions.push([
                    p[0] + o[0] - self.offset[0],
                    p[1] + o[1] - self.offset[1],
                    p[2] + o[2] - self.offset[2],
                ]);
                self.colors.push(color);
            }
        }
        // Vertices 2k (at a) and 2k+1 (at b) for each of the three edges.
        for k in 0..3u32 {
            let (a0, b0) = (base + 2 * k, base + 2 * k + 1);
            let (a1, b1) = (base + 2 * ((k + 1) % 3), base + 2 * ((k + 1) % 3) + 1);
            self.indices.extend_from_slice(&[a0, a1, b1, a0, b1, b0]);
        }
    }
}

/// The 3D model of a file, `None` when it has neither a surface with triangles nor an
/// alignment with an elevation anywhere.
pub fn model(x: &LandXml) -> Option<Model> {
    // Plan box and elevation range of everything drawn.
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    fn grow(lo: &mut [f64; 3], hi: &mut [f64; 3], p: [f64; 3]) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let surfaces: Vec<&Surface> = x.surfaces.iter().filter(|s| !s.faces.is_empty()).collect();
    for s in &surfaces {
        for f in &s.faces {
            for &i in f {
                if let Some(p) = s.points.get(i as usize) {
                    grow(&mut lo, &mut hi, *p);
                }
            }
        }
    }
    let locators: Vec<Locator<'_>> = surfaces.iter().filter_map(|s| Locator::new(s)).collect();
    // Alignments as 3D polylines: on a surface where there is one, else on the first design
    // profile; breaks where neither gives an elevation.
    let mut plan_diag = (hi[0] - lo[0]).hypot(hi[1] - lo[1]);
    let mut paths: Vec<Vec<Vec<[f64; 3]>>> = Vec::new();
    for a in &x.alignments {
        let pvis = a.profiles.iter().find_map(|p| match p {
            Profile::Design { pvis, .. } => Some(pvis.as_slice()),
            _ => None,
        });
        let len = a.computed_length();
        let step = if plan_diag.is_finite() && plan_diag > 0.0 {
            plan_diag / 2000.0
        } else {
            (len / 2000.0).max(1e-6)
        };
        let mut runs: Vec<Vec<[f64; 3]>> = vec![Vec::new()];
        // Straight elements come as their two ends: points at most `step` apart along them
        // too, so the line follows the ground.
        let samples = a.sample(step);
        let mut dense: Vec<(f64, [f64; 2])> = Vec::with_capacity(samples.len());
        for (i, &(sta, p)) in samples.iter().enumerate() {
            if let Some(&(s0, q)) = i.checked_sub(1).and_then(|j| samples.get(j)) {
                let d = (p[0] - q[0]).hypot(p[1] - q[1]);
                let n = ((d / step).ceil() as usize).clamp(1, 100_000);
                for k in 1..n {
                    let u = k as f64 / n as f64;
                    dense.push((
                        s0 + (sta - s0) * u,
                        [q[0] + (p[0] - q[0]) * u, q[1] + (p[1] - q[1]) * u],
                    ));
                }
            }
            dense.push((sta, p));
        }
        for (sta, p) in dense {
            let z = locators
                .iter()
                .find_map(|l| l.z(p))
                .or_else(|| pvis.and_then(|v| profile_elevation(v, sta)));
            match z {
                Some(z) => runs.last_mut().unwrap().push([p[0], p[1], z]),
                None => {
                    if !runs.last().unwrap().is_empty() {
                        runs.push(Vec::new());
                    }
                }
            }
        }
        runs.retain(|r| r.len() >= 2);
        for r in &runs {
            for p in r {
                grow(&mut lo, &mut hi, *p);
            }
        }
        paths.push(runs);
    }
    // Survey points with an elevation: their own, else the surface's under them.
    let posts: Vec<[f64; 3]> = x
        .points
        .iter()
        .filter_map(|p| {
            let z = p.z.or_else(|| locators.iter().find_map(|l| l.z(p.at)))?;
            Some([p.at[0], p.at[1], z])
        })
        .take(MAX_POSTS)
        .collect();
    for p in &posts {
        grow(&mut lo, &mut hi, *p);
    }
    // Nothing with an elevation: no model (parcels alone stay in the plan).
    if !(lo[0].is_finite() && hi[0].is_finite()) {
        return None;
    }
    // Parcel boundaries: plan points (straight sides cut to `step` like alignments), on a
    // surface where there is one (coarser than alignments: many parcels, straight sides).
    let step = (hi[0] - lo[0]).hypot(hi[1] - lo[1]).max(1e-6) / 500.0;
    let mut parcels: Vec<Vec<([f64; 2], Option<f64>)>> = Vec::new();
    for p in &x.parcels {
        let el = crate::Alignment {
            elements: p.boundary.clone(),
            ..crate::Alignment::default()
        };
        let mut ring: Vec<([f64; 2], Option<f64>)> = Vec::new();
        let mut prev: Option<[f64; 2]> = None;
        for (_, q) in el.sample(step.max(1e-6)) {
            if let Some(a) = prev {
                let d = (q[0] - a[0]).hypot(q[1] - a[1]);
                let n = ((d / step.max(1e-6)).ceil() as usize).clamp(1, 10_000);
                for k in 1..n {
                    let u = k as f64 / n as f64;
                    let r = [a[0] + (q[0] - a[0]) * u, a[1] + (q[1] - a[1]) * u];
                    ring.push((r, locators.iter().find_map(|l| l.z(r))));
                }
            }
            ring.push((q, locators.iter().find_map(|l| l.z(q))));
            prev = Some(q);
        }
        for (q, z) in &ring {
            lo[0] = lo[0].min(q[0]);
            lo[1] = lo[1].min(q[1]);
            hi[0] = hi[0].max(q[0]);
            hi[1] = hi[1].max(q[1]);
            if let Some(z) = z {
                lo[2] = lo[2].min(*z);
                hi[2] = hi[2].max(*z);
            }
        }
        parcels.push(ring);
    }
    plan_diag = (hi[0] - lo[0]).hypot(hi[1] - lo[1]).max(1e-6);
    let mut m = Model {
        offset: [
            (lo[0] + hi[0]) / 2.0,
            (lo[1] + hi[1]) / 2.0,
            (lo[2] + hi[2]) / 2.0,
        ],
        ..Model::default()
    };
    let (z_min, z_max) = (lo[2], hi[2]);
    let range = z_max - z_min;
    let mut segments_left = MAX_CONTOUR_SEGMENTS;
    for s in &surfaces {
        let base = m.positions.len() as u32;
        for p in &s.points {
            let t = if range > 0.0 {
                (p[2] - z_min) / range
            } else {
                0.5
            };
            m.positions
                .push([p[0] - m.offset[0], p[1] - m.offset[1], p[2] - m.offset[2]]);
            m.colors.push(crate::scene::ramp_rgb(t));
        }
        for f in &s.faces {
            if f.iter().all(|&i| (i as usize) < s.points.len()) {
                m.indices.extend(f.iter().map(|&i| base + i));
                m.triangles += 1;
            }
        }
        // The plan's contour interval for this surface.
        let Some(me) = tin::measures(s) else { continue };
        let step = tin::interval(me.z_max - me.z_min, 15.0);
        let segs = tin::contours(s, step, segments_left.max(1));
        if segs.len() >= segments_left {
            m.contours_cut = true;
        }
        segments_left = segments_left.saturating_sub(segs.len());
        for (z, a, b) in segs {
            let major = ((z / step).round() as i64).rem_euclid(5) == 0;
            let (r, c) = if major {
                (R_MAJOR, MAJOR)
            } else {
                (R_MINOR, MINOR)
            };
            m.tube([a[0], a[1], z], [b[0], b[1], z], r * plan_diag, c);
            m.contour_segments += 1;
        }
    }
    for runs in &paths {
        for r in runs {
            for w in r.windows(2) {
                m.tube(w[0], w[1], R_ALIGN * plan_diag, ALIGN);
                m.alignment_segments += 1;
            }
        }
    }
    for ring in &parcels {
        for w in ring.windows(2) {
            let (a, b) = (w[0], w[1]);
            if (a.0[0] - b.0[0]).hypot(a.0[1] - b.0[1]) < 1e-12 {
                continue;
            }
            let za = a.1.unwrap_or(z_min);
            let zb = b.1.unwrap_or(z_min);
            m.tube(
                [a.0[0], a.0[1], za],
                [b.0[0], b.0[1], zb],
                R_MAJOR * plan_diag,
                PARCEL,
            );
            m.parcel_segments += 1;
        }
    }
    for p in &posts {
        let top = [p[0], p[1], p[2] + H_POST * plan_diag];
        m.tube(*p, top, R_POST * plan_diag, SURVEY);
        m.survey_points += 1;
    }
    (!m.indices.is_empty()).then_some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Alignment, Geom};

    fn pyramid() -> Surface {
        Surface {
            name: "p".into(),
            points: vec![
                [100.0, 200.0, 10.0],
                [102.0, 200.0, 10.0],
                [102.0, 202.0, 10.0],
                [100.0, 202.0, 10.0],
                [101.0, 201.0, 11.0],
            ],
            faces: vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
            ..Surface::default()
        }
    }

    #[test]
    fn surface_contours_and_a_draped_alignment() {
        let a = Alignment {
            name: "A".into(),
            elements: vec![Geom::Line {
                start: [100.0, 201.0],
                end: [102.0, 201.0],
            }],
            ..Alignment::default()
        };
        let x = LandXml {
            surfaces: vec![pyramid()],
            alignments: vec![a],
            ..LandXml::default()
        };
        let m = model(&x).unwrap();
        assert_eq!(m.offset, [101.0, 201.0, 10.5]);
        assert_eq!(m.triangles, 4);
        // The apex, centred.
        assert_eq!(m.positions[4], [0.0, 0.0, 0.5]);
        // Low corners at the ramp's low end, the apex at its high end.
        assert_eq!(m.colors[0], crate::scene::ramp_rgb(0.0));
        assert_eq!(m.colors[4], crate::scene::ramp_rgb(1.0));
        assert!(m.contour_segments > 0);
        // The alignment rises over the apex: its highest tube point is near 0.5 (+ radius).
        assert!(m.alignment_segments >= 2);
        let first_align = 5 + m.contour_segments * 6;
        let top = m.positions[first_align..]
            .iter()
            .map(|p| p[2])
            .fold(f64::NEG_INFINITY, f64::max);
        let r = R_ALIGN * 8f64.sqrt();
        assert!((top - (0.5 + r)).abs() < 0.01, "{top}");
        assert_eq!(m.indices.len() % 3, 0);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
    }

    #[test]
    fn parcels_and_survey_points_in_3d() {
        let lot = crate::Parcel {
            name: "Lot".into(),
            boundary: vec![
                Geom::Line {
                    start: [100.5, 200.5],
                    end: [101.5, 200.5],
                },
                Geom::Line {
                    start: [101.5, 200.5],
                    end: [101.5, 201.5],
                },
            ],
            ..crate::Parcel::default()
        };
        let pts = vec![
            // Its own elevation; the surface's; none (off the surface): left out.
            crate::CgPoint {
                at: [100.0, 200.0],
                z: Some(12.0),
                ..crate::CgPoint::default()
            },
            crate::CgPoint {
                at: [101.0, 201.0],
                ..crate::CgPoint::default()
            },
            crate::CgPoint {
                at: [90.0, 190.0],
                ..crate::CgPoint::default()
            },
        ];
        let x = LandXml {
            surfaces: vec![pyramid()],
            parcels: vec![lot.clone()],
            points: pts.clone(),
            ..LandXml::default()
        };
        let m = model(&x).unwrap();
        assert_eq!(m.survey_points, 2);
        assert!(m.parcel_segments >= 2);
        // The box reaches the survey point at 12 m: centred on (10 + 12) / 2.
        assert_eq!(m.offset[2], 11.0);
        // The second post stands on the apex (11 m, centred 0), the first at 1 m.
        let posts = &m.positions[m.positions.len() - 12..];
        let low = |k: usize| {
            posts[k * 6..k * 6 + 6]
                .iter()
                .map(|p| p[2])
                .fold(f64::INFINITY, f64::min)
        };
        assert!((low(0) - 1.0).abs() < 1e-9, "{}", low(0));
        assert!(low(1).abs() < 1e-9, "{}", low(1));
        // Both kinds in their plan colours.
        assert!(m.colors.contains(&PARCEL) && m.colors.contains(&SURVEY));
        assert_eq!(m.indices.len() % 3, 0);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
        // Parcels alone have no elevation: no model.
        let flat = LandXml {
            parcels: vec![lot],
            ..LandXml::default()
        };
        assert!(model(&flat).is_none());
        // Survey points alone with elevations make one.
        let only = LandXml {
            points: pts,
            ..LandXml::default()
        };
        assert_eq!(model(&only).unwrap().survey_points, 1);
    }

    #[test]
    fn locator_interpolates() {
        let s = pyramid();
        let l = Locator::new(&s).unwrap();
        assert!((l.z([101.0, 201.0]).unwrap() - 11.0).abs() < 1e-12);
        assert!((l.z([100.5, 200.5]).unwrap() - 10.5).abs() < 1e-12);
        assert!(l.z([99.0, 201.0]).is_none());
    }
}
