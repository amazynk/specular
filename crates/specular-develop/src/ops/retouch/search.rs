//! Picks a spot's source automatically: the best match on rings around the spot.
//!
//! Candidates sit on three rings (2.5, 3.5 and 5 spot sizes out, 24 directions each) and
//! must lie on the picture. Each is scored on log luminance:
//! - **edge match**: the ring just outside the spot against the ring just outside the
//!   candidate, means removed (heal fixes the level, not the texture);
//! - **clean inside**: the candidate's inside should vary like the spot's surroundings, so a
//!   blemish or an edge is not copied in;
//! - a small preference for near candidates.
//!
//! Candidates over other spots' areas are skipped.

/// Picture pixels around a spot ([`best_source`]).
pub struct View<'a> {
    pub w: u32,
    pub h: u32,
    /// RGBA, linear.
    pub px: &'a [f32],
}

impl View<'_> {
    fn lum(&self, x: f64, y: f64) -> f64 {
        let xi = (x.round() as i64).clamp(0, i64::from(self.w) - 1) as usize;
        let yi = (y.round() as i64).clamp(0, i64::from(self.h) - 1) as usize;
        let i = 4 * (yi * self.w as usize + xi);
        let p = &self.px[i..i + 3];
        let l = 0.2627 * f64::from(p[0]) + 0.678 * f64::from(p[1]) + 0.0593 * f64::from(p[2]);
        (l.max(0.0) + 1e-3).log2()
    }
}

/// The spot to find a source for, in picture pixels.
pub struct Want<'a> {
    pub centre: [f64; 2],
    pub radius: f64,
    /// A brushed area's path (empty for a round spot).
    pub path: &'a [[f64; 2]],
    /// Other spots' areas: centre and radius.
    pub avoid: &'a [([f64; 2], f64)],
}

/// Sample points of the area's outline ring (`k` radii out) and of its inside.
fn samples(w: &Want<'_>) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let pts: Vec<[f64; 2]> = if w.path.is_empty() {
        vec![w.centre]
    } else {
        let step = (w.path.len() / 12).max(1);
        w.path.iter().step_by(step).copied().collect()
    };
    let mut ring = Vec::new();
    let mut inside = Vec::new();
    let per = (32 / pts.len()).max(6);
    for p in &pts {
        for i in 0..per {
            let a = std::f64::consts::TAU * i as f64 / per as f64;
            let (s, c) = a.sin_cos();
            for k in [1.25, 1.5] {
                ring.push([p[0] + c * w.radius * k, p[1] + s * w.radius * k]);
            }
            for k in [0.3, 0.7] {
                inside.push([p[0] + c * w.radius * k, p[1] + s * w.radius * k]);
            }
        }
        inside.push(*p);
    }
    // Ring points that fall inside the area itself (a brushed path) are not outside it.
    let near = |q: &[f64; 2]| {
        pts.iter()
            .any(|p| ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt() < w.radius * 1.1)
    };
    ring.retain(|q| !near(q));
    (ring, inside)
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len().max(1) as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n;
    (m, var.sqrt())
}

/// The best source centre (picture pixels) for `want`, or `None` when no candidate is on
/// the picture.
pub fn best_source(view: &View<'_>, want: &Want<'_>) -> Option<[f64; 2]> {
    let (ring, inside) = samples(want);
    // The area's extent from its reference point.
    let reach = want
        .path
        .iter()
        .map(|p| ((p[0] - want.centre[0]).powi(2) + (p[1] - want.centre[1]).powi(2)).sqrt())
        .fold(0.0f64, f64::max)
        + want.radius;
    let all: Vec<[f64; 2]> = ring.iter().chain(&inside).copied().collect();
    let t_ring: Vec<f64> = ring.iter().map(|q| view.lum(q[0], q[1])).collect();
    let (t_mean, t_sd) = mean_sd(&t_ring);
    let fits = |d: [f64; 2]| {
        all.iter().all(|q| {
            let (x, y) = (q[0] + d[0], q[1] + d[1]);
            x >= 0.0 && y >= 0.0 && x <= f64::from(view.w) - 1.0 && y <= f64::from(view.h) - 1.0
        })
    };
    let clear = |d: [f64; 2]| {
        want.avoid.iter().all(|(c, r)| {
            let (x, y) = (want.centre[0] + d[0], want.centre[1] + d[1]);
            ((x - c[0]).powi(2) + (y - c[1]).powi(2)).sqrt() > r + reach
        })
    };
    let mut best: Option<(f64, [f64; 2])> = None;
    for k in [2.5, 3.5, 5.0] {
        let dist = reach * k;
        for i in 0..24 {
            let a = std::f64::consts::TAU * (i as f64 + 0.5 * (k - 2.5)) / 24.0;
            let d = [a.cos() * dist, a.sin() * dist];
            if !fits(d) || !clear(d) {
                continue;
            }
            let c_ring: Vec<f64> = ring
                .iter()
                .map(|q| view.lum(q[0] + d[0], q[1] + d[1]))
                .collect();
            let c_in: Vec<f64> = inside
                .iter()
                .map(|q| view.lum(q[0] + d[0], q[1] + d[1]))
                .collect();
            let (c_mean, _) = mean_sd(&c_ring);
            let edge = t_ring
                .iter()
                .zip(&c_ring)
                .map(|(a, b)| ((a - t_mean) - (b - c_mean)).powi(2))
                .sum::<f64>()
                / t_ring.len().max(1) as f64;
            let (in_mean, in_sd) = mean_sd(&c_in);
            let clean = (in_sd - t_sd).powi(2) + 0.25 * (in_mean - c_mean).powi(2);
            let score = edge + clean + 0.002 * k;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, d));
            }
        }
    }
    best.map(|(_, d)| [want.centre[0] + d[0], want.centre[1] + d[1]])
}
