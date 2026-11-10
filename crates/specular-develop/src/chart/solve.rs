//! The profile fit (CPU, f64).
//!
//! 1. **Normalise.** The camera neutral from the grey patches: per channel, the least-squares
//!    ratio to green over the neutrals (white too unless it is near clipping). Then one
//!    exposure scale so the anchor neutral (Neutral 6.5 on a ColorChecker: the grey nearest
//!    L* 66) matches its reference Y.
//! 2. **Matrix.** A ForwardMatrix FM, white-balanced camera → XYZ D50, kept white-preserving
//!    (FM·(1, 1, 1) = D50; six free values). Start: the closed-form constrained least
//!    squares in XYZ (Finlayson and Drew, "Constrained least-squares regression in color
//!    spaces", JEI 1997). Then Levenberg–Marquardt on Σ ΔE2000² (Sharma, Wu, Dalal 2005),
//!    with the neutral's red and blue refined too (a chart's greys are not exactly neutral).
//! 3. **Table.** A `ProfileHueSatMap` (DNG 1.7.1 chapter 6, in linear ProPhoto) for what the
//!    matrix leaves: per patch the hue shift, saturation and value scales between the
//!    matrix result and the reference, spread over the grid by a smoothing thin-plate spline
//!    on the hue circle (saturation as the radius, so saturation 0 is one point). The
//!    smoothing comes from leave-one-out cross-validation. Identity at saturation 0, fading
//!    to identity past the most saturated patch; hue shifts within ±15°, scales 0.6–1.5.
//! 4. **Root polynomial** (Finlayson, Mackiewicz and Hurlbert, "Color correction using
//!    root-polynomial regression", IEEE TIP 2015; degree 2): for the internal path only, a
//!    DCP cannot carry it.
//!
//! The report gives ΔE2000 before (the current matrix, white-balanced on the chart's greys),
//! with the matrix, with matrix and table, and cross-validated: each patch predicted with
//! the matrix refitted without it and the table's leave-one-out value there.

use super::colour::{self, Lab, M3, V3};
use super::layout::Layout;
use super::sample::{Flag, Sampled};

/// Smoothing values tried (λ, per data point).
const LAMBDAS: [f64; 13] = [
    1e-7, 1e-6, 3e-6, 1e-5, 3e-5, 1e-4, 3e-4, 1e-3, 3e-3, 1e-2, 3e-2, 1e-1, 1.0,
];
/// Table limits.
const MAX_HUE_SHIFT: f64 = 15.0;
const SCALE_RANGE: (f64, f64) = (0.6, 1.5);
/// Patches needed for a fit (six matrix values, with room to check them).
pub const MIN_PATCHES: usize = 12;

/// What the solver works on.
#[derive(Clone, Debug)]
pub struct Input<'a> {
    pub layout: &'a Layout,
    /// One per layout patch: camera RGB as sampled (as-shot white-balanced camera RGB for
    /// raws, linear RGB for other images).
    pub samples: &'a [Sampled],
    /// The current colours: neutral-normalised camera RGB → XYZ D50.
    pub before: M3,
    /// Fit a HueSatMap.
    pub table: bool,
}

/// A HueSatMap (one value division): `data[h * sat + s]` = (hue shift °, sat scale,
/// value scale).
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub hue: u32,
    pub sat: u32,
    pub data: Vec<[f64; 3]>,
}

/// Mean, median and largest ΔE2000.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub mean: f64,
    pub median: f64,
    pub max: f64,
}

impl Stats {
    fn of(v: &[f64]) -> Stats {
        if v.is_empty() {
            return Stats::default();
        }
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        let n = s.len();
        Stats {
            mean: s.iter().sum::<f64>() / n as f64,
            median: if n % 2 == 1 {
                s[n / 2]
            } else {
                (s[n / 2 - 1] + s[n / 2]) / 2.0
            },
            max: s[n - 1],
        }
    }
}

/// One patch in the report.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchResult {
    pub reference: Lab,
    /// The profile's L*a*b* (matrix and table), for the drawing.
    pub measured: Lab,
    /// The current colours'.
    pub before: Lab,
    pub de_before: f64,
    pub de_matrix: f64,
    pub de_after: f64,
    /// Cross-validated (only for fitted patches).
    pub de_cv: Option<f64>,
    /// Why it was left out of the fit.
    pub left_out: Option<Flag>,
}

/// How good the profile is.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub patches: Vec<PatchResult>,
    pub before: Stats,
    pub matrix: Stats,
    pub after: Stats,
    pub cross_validated: Stats,
    pub root_polynomial: Stats,
    /// The neutrals' exposure or colour ratios vary by this fraction (> 3 %: uneven light).
    pub uneven_light: Option<f64>,
    /// The table's smoothing.
    pub lambda: f64,
}

/// A solved profile for one shot.
#[derive(Clone, Debug)]
pub struct Fit {
    /// Camera neutral in the sampled units (G = 1).
    pub neutral: V3,
    /// Exposure scale applied to the neutral-normalised values for the fit.
    pub exposure: f64,
    /// ForwardMatrix: neutral-normalised camera → XYZ D50.
    pub fm: M3,
    pub table: Option<Table>,
    /// Root-polynomial (degree 2) neutral-normalised camera → XYZ D50, 3 × 6.
    pub root_polynomial: [[f64; 6]; 3],
    pub report: Report,
}

/// Why no profile could be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveError {
    /// Fewer usable patches than [`MIN_PATCHES`].
    TooFewPatches { usable: usize },
    /// No usable grey patch for the white balance.
    NoNeutrals,
    /// The values do not determine a matrix (all patches alike).
    Degenerate,
}

impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolveError::TooFewPatches { usable } => {
                write!(f, "only {usable} usable patches, {MIN_PATCHES} needed")
            }
            SolveError::NoNeutrals => write!(f, "no usable grey patch"),
            SolveError::Degenerate => write!(f, "the patches are too alike"),
        }
    }
}

impl std::error::Error for SolveError {}

/// Neutral-normalised camera RGB → linear ProPhoto as the DNG reader builds it.
pub fn prophoto_matrix(fm: &M3) -> M3 {
    let to_pro = colour::inv(&colour::prophoto_to_xyz()).unwrap_or(colour::IDENTITY);
    let mut m = colour::mul(&to_pro, fm);
    colour::normalize_rows(&mut m);
    m
}

impl Table {
    /// Trilinear lookup as DNG readers do (hue wraps, saturation clamps).
    pub fn lookup(&self, h6: f64, s: f64) -> [f64; 3] {
        let hs = h6 * f64::from(self.hue) / 6.0;
        let h0 = (hs.max(0.0) as usize).min(self.hue as usize - 1);
        let hf = (hs - h0 as f64).clamp(0.0, 1.0);
        let h1 = if h0 + 1 >= self.hue as usize {
            0
        } else {
            h0 + 1
        };
        let ss = s.clamp(0.0, 1.0) * f64::from(self.sat - 1);
        let s0 = (ss as usize).min(self.sat as usize - 2);
        let sf = ss - s0 as f64;
        let at = |h: usize, s: usize| self.data[h * self.sat as usize + s];
        let mix = |a: [f64; 3], b: [f64; 3], f: f64| -> [f64; 3] {
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f)
        };
        mix(
            mix(at(h0, s0), at(h1, s0), hf),
            mix(at(h0, s0 + 1), at(h1, s0 + 1), hf),
            sf,
        )
    }

    /// The table on HSV `hsv` (saturation and value capped at 1, as DNG readers do).
    pub fn map(&self, hsv: V3) -> V3 {
        let [dh, ss, vs] = self.lookup(hsv[0], hsv[1]);
        let mut h = (hsv[0] + dh / 60.0).rem_euclid(6.0);
        if !h.is_finite() || h >= 6.0 {
            h = 0.0;
        }
        [h, (hsv[1] * ss).min(1.0), (hsv[2] * vs).clamp(0.0, 1.0)]
    }
}

/// The profile's forward map: neutral-normalised camera RGB → linear ProPhoto, as a DNG
/// reader applies a profile without look table or tone curve (input clamped to 0–1).
pub fn forward_prophoto(fm: &M3, table: Option<&Table>, c: V3) -> V3 {
    let p = colour::mulv(&prophoto_matrix(fm), c).map(|x| x.clamp(0.0, 1.0));
    match table {
        Some(t) => colour::hsv_to_rgb(t.map(colour::rgb_to_hsv(p))),
        None => p,
    }
}

fn to_lab_pro(p: V3) -> Lab {
    colour::xyz_to_lab(colour::mulv(&colour::prophoto_to_xyz(), p))
}

/// The six free values of a white-preserving matrix: the first two of each row.
fn pack(m: &M3) -> [f64; 6] {
    [m[0][0], m[0][1], m[1][0], m[1][1], m[2][0], m[2][1]]
}

fn unpack(p: &[f64]) -> M3 {
    let w = colour::d50();
    std::array::from_fn(|r| {
        let (a, b) = (p[2 * r], p[2 * r + 1]);
        [a, b, w[r] - a - b]
    })
}

/// Least squares of `targets` (3 per point) on `terms` (k per point): k values per output.
fn least_squares(terms: &[Vec<f64>], targets: &[V3]) -> Option<Vec<[f64; 3]>> {
    let k = terms.first()?.len();
    let mut ata = vec![vec![0f64; k]; k];
    let mut atb = vec![[0f64; 3]; k];
    for (t, y) in terms.iter().zip(targets) {
        for i in 0..k {
            for j in 0..k {
                ata[i][j] += t[i] * t[j];
            }
            for c in 0..3 {
                atb[i][c] += t[i] * y[c];
            }
        }
    }
    let lu = Lu::new(ata)?;
    let cols: Vec<Vec<f64>> = (0..3)
        .map(|c| lu.solve(&atb.iter().map(|v| v[c]).collect::<Vec<_>>()))
        .collect();
    Some(
        (0..k)
            .map(|i| [cols[0][i], cols[1][i], cols[2][i]])
            .collect(),
    )
}

/// Unconstrained least-squares 3 × 3 matrix (Cheung et al. 2004, three terms).
pub fn matrix_least_squares(c: &[V3], xyz: &[V3]) -> Option<M3> {
    let terms: Vec<Vec<f64>> = c.iter().map(|v| v.to_vec()).collect();
    let m = least_squares(&terms, xyz)?;
    Some(std::array::from_fn(|r| [m[0][r], m[1][r], m[2][r]]))
}

/// Degree-2 root-polynomial terms: R, G, B, √(RG), √(GB), √(RB).
pub fn root_terms(c: V3) -> [f64; 6] {
    let [r, g, b] = c.map(|v| v.max(0.0));
    [r, g, b, (r * g).sqrt(), (g * b).sqrt(), (r * b).sqrt()]
}

/// Root-polynomial least squares (degree 2), 3 × 6.
pub fn root_polynomial(c: &[V3], xyz: &[V3]) -> Option<[[f64; 6]; 3]> {
    let terms: Vec<Vec<f64>> = c.iter().map(|v| root_terms(*v).to_vec()).collect();
    let m = least_squares(&terms, xyz)?;
    Some(std::array::from_fn(|r| std::array::from_fn(|k| m[k][r])))
}

pub fn apply_root(m: &[[f64; 6]; 3], c: V3) -> V3 {
    let t = root_terms(c);
    std::array::from_fn(|r| (0..6).map(|k| m[r][k] * t[k]).sum())
}

/// The white-preserving least-squares matrix (closed form, per row with a Lagrange
/// multiplier): minimises Σ |FM c − X|² with FM·(1, 1, 1) = `white`.
pub fn white_preserving(c: &[V3], xyz: &[V3], white: V3) -> Option<M3> {
    let mut a = [[0f64; 3]; 3];
    for v in c {
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += v[i] * v[j];
            }
        }
    }
    let ai = colour::inv(&a)?;
    let one = [1.0; 3];
    let ai1 = colour::mulv(&ai, one);
    let d: f64 = ai1.iter().sum();
    if d.abs() < 1e-15 {
        return None;
    }
    let mut m = [[0f64; 3]; 3];
    for r in 0..3 {
        let mut b = [0f64; 3];
        for (v, x) in c.iter().zip(xyz) {
            for i in 0..3 {
                b[i] += v[i] * x[r];
            }
        }
        let mu = colour::mulv(&ai, b);
        let k = (mu.iter().sum::<f64>() - white[r]) / d;
        m[r] = std::array::from_fn(|i| mu[i] - k * ai1[i]);
    }
    Some(m)
}

/// Levenberg–Marquardt on residuals `f(p)` (forward-difference Jacobian).
fn levenberg_marquardt(p0: &[f64], f: impl Fn(&[f64]) -> Vec<f64>, iters: usize) -> Vec<f64> {
    let n = p0.len();
    let mut p = p0.to_vec();
    let mut r = f(&p);
    let cost = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
    let mut c = cost(&r);
    let mut lambda = 1e-3;
    for _ in 0..iters {
        let mut jac = vec![vec![0f64; r.len()]; n];
        for k in 0..n {
            let h = 1e-7 * p[k].abs().max(1e-3);
            let mut q = p.clone();
            q[k] += h;
            let rq = f(&q);
            for (i, v) in rq.iter().enumerate() {
                jac[k][i] = (v - r[i]) / h;
            }
        }
        let mut jtj = vec![vec![0f64; n]; n];
        let mut jtr = vec![0f64; n];
        for a in 0..n {
            for b in 0..n {
                jtj[a][b] = jac[a].iter().zip(&jac[b]).map(|(x, y)| x * y).sum();
            }
            jtr[a] = jac[a].iter().zip(&r).map(|(x, y)| x * y).sum();
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut m = jtj.clone();
            for (a, row) in m.iter_mut().enumerate() {
                row[a] += lambda * jtj[a][a].max(1e-12);
            }
            let Some(lu) = Lu::new(m) else {
                lambda *= 10.0;
                continue;
            };
            let step = lu.solve(&jtr);
            let q: Vec<f64> = p.iter().zip(&step).map(|(a, s)| a - s).collect();
            let rq = f(&q);
            let cq = cost(&rq);
            if cq.is_finite() && cq < c {
                let done = (c - cq) < 1e-12 * c.max(1e-12);
                p = q;
                r = rq;
                c = cq;
                lambda = (lambda * 0.3).max(1e-12);
                improved = !done;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    p
}

/// Dense LU with partial pivoting.
struct Lu {
    a: Vec<Vec<f64>>,
    piv: Vec<usize>,
}

impl Lu {
    fn new(mut a: Vec<Vec<f64>>) -> Option<Lu> {
        let n = a.len();
        let mut piv: Vec<usize> = (0..n).collect();
        let scale = a
            .iter()
            .flatten()
            .fold(0f64, |m, v| m.max(v.abs()))
            .max(1e-300);
        for c in 0..n {
            let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
            if a[p][c].abs() < 1e-14 * scale {
                return None;
            }
            a.swap(c, p);
            piv.swap(c, p);
            let pivot = a[c].clone();
            for row in a.iter_mut().skip(c + 1) {
                let f = row[c] / pivot[c];
                row[c] = f;
                for (x, y) in row[c + 1..].iter_mut().zip(&pivot[c + 1..]) {
                    *x -= f * y;
                }
            }
        }
        Some(Lu { a, piv })
    }

    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.a.len();
        let mut x: Vec<f64> = self.piv.iter().map(|&i| b[i]).collect();
        for r in 0..n {
            for k in 0..r {
                x[r] -= self.a[r][k] * x[k];
            }
        }
        for r in (0..n).rev() {
            for k in r + 1..n {
                x[r] -= self.a[r][k] * x[k];
            }
            x[r] /= self.a[r][r];
        }
        x
    }
}

/// A smoothing thin-plate spline in the plane, three outputs.
struct Tps {
    pts: Vec<(f64, f64)>,
    w: Vec<[f64; 3]>,
    a: [[f64; 3]; 3],
}

fn kernel(r2: f64) -> f64 {
    if r2 <= 0.0 { 0.0 } else { 0.5 * r2 * r2.ln() }
}

impl Tps {
    /// The system matrix for points `pts` and smoothing `lambda`.
    fn system(pts: &[(f64, f64)], lambda: f64) -> Vec<Vec<f64>> {
        let n = pts.len();
        let mut m = vec![vec![0f64; n + 3]; n + 3];
        for i in 0..n {
            for j in 0..n {
                let d2 = (pts[i].0 - pts[j].0).powi(2) + (pts[i].1 - pts[j].1).powi(2);
                m[i][j] = kernel(d2);
            }
            m[i][i] += n as f64 * lambda;
            let p = [1.0, pts[i].0, pts[i].1];
            for k in 0..3 {
                m[i][n + k] = p[k];
                m[n + k][i] = p[k];
            }
        }
        m
    }

    fn fit(pts: &[(f64, f64)], y: &[[f64; 3]], lambda: f64) -> Option<Tps> {
        let n = pts.len();
        let lu = Lu::new(Tps::system(pts, lambda))?;
        let mut w = vec![[0f64; 3]; n];
        let mut a = [[0f64; 3]; 3];
        for c in 0..3 {
            let mut rhs: Vec<f64> = y.iter().map(|v| v[c]).collect();
            rhs.extend([0.0; 3]);
            let x = lu.solve(&rhs);
            for i in 0..n {
                w[i][c] = x[i];
            }
            for k in 0..3 {
                a[k][c] = x[n + k];
            }
        }
        Some(Tps {
            pts: pts.to_vec(),
            w,
            a,
        })
    }

    fn eval(&self, x: f64, y: f64) -> [f64; 3] {
        let mut out: [f64; 3] =
            std::array::from_fn(|c| self.a[0][c] + self.a[1][c] * x + self.a[2][c] * y);
        for (p, w) in self.pts.iter().zip(&self.w) {
            let k = kernel((x - p.0).powi(2) + (y - p.1).powi(2));
            for c in 0..3 {
                out[c] += k * w[c];
            }
        }
        out
    }

    /// Leave-one-out predictions at the data points for smoothing `lambda`:
    /// `ŷ₋ᵢ = yᵢ − (yᵢ − ŷᵢ) / (1 − Hᵢᵢ)`, from the smoother matrix's diagonal.
    fn loo(pts: &[(f64, f64)], y: &[[f64; 3]], lambda: f64) -> Option<Vec<[f64; 3]>> {
        let n = pts.len();
        let sys = Tps::system(pts, lambda);
        let lu = Lu::new(sys.clone())?;
        let fit = Tps::fit(pts, y, lambda)?;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let mut e = vec![0f64; n + 3];
            e[i] = 1.0;
            let x = lu.solve(&e);
            // Hᵢᵢ: the fitted value at i of the unit response (without the smoothing term).
            let hii: f64 = (0..n)
                .map(|j| (sys[i][j] - if i == j { n as f64 * lambda } else { 0.0 }) * x[j])
                .sum::<f64>()
                + (0..3).map(|k| sys[i][n + k] * x[n + k]).sum::<f64>();
            let fi = fit.eval(pts[i].0, pts[i].1);
            let d = (1.0 - hii).max(1e-6);
            out.push(std::array::from_fn(|c| y[i][c] - (y[i][c] - fi[c]) / d));
        }
        Some(out)
    }
}

/// Table targets as deviations from identity, scaled for the cross-validation score. The
/// hue shift and the saturation change are fitted times the saturation (a shift on the
/// chroma plane, which vanishes at grey and varies smoothly), the value scale as it is.
const TARGET_SCALE: [f64; 3] = [1.5, 0.015, 0.05];
/// Below this saturation the chroma-plane targets are divided by it instead.
const S_FLOOR: f64 = 0.05;

fn clamp_entry(e: [f64; 3]) -> [f64; 3] {
    [
        e[0].clamp(-MAX_HUE_SHIFT, MAX_HUE_SHIFT),
        e[1].clamp(SCALE_RANGE.0, SCALE_RANGE.1),
        e[2].clamp(SCALE_RANGE.0, SCALE_RANGE.1),
    ]
}

/// Position of HSV `hsv` on the hue circle (saturation as radius).
fn polar(hsv: V3) -> (f64, f64) {
    let a = hsv[0] * std::f64::consts::PI / 3.0;
    (hsv[1] * a.cos(), hsv[1] * a.sin())
}

/// Per-patch table targets (deviation from identity) between the matrix result `fit` and
/// the reference `want`, both ProPhoto HSV.
fn target(fit: V3, want: V3) -> [f64; 3] {
    let vs = if fit[2] > 1e-6 {
        want[2] / fit[2] - 1.0
    } else {
        0.0
    };
    if fit[1] < 0.03 {
        return [0.0, 0.0, vs];
    }
    let mut dh = (want[0] - fit[0]) * 60.0;
    if dh > 180.0 {
        dh -= 360.0;
    }
    if dh < -180.0 {
        dh += 360.0;
    }
    let s = fit[1];
    [dh * s, (want[1] / fit[1] - 1.0) * s, vs]
}

/// A table entry (absolute) from fitted chroma-plane values `v` at saturation `s`.
fn from_plane(v: [f64; 3], s: f64) -> [f64; 3] {
    let s = s.max(S_FLOOR);
    [v[0] / s, 1.0 + v[1] / s, 1.0 + v[2]]
}

/// The fitted table and its smoothing, from per-patch HSV pairs (matrix result, reference).
struct TableFit {
    tps: Tps,
    lambda: f64,
    /// Leave-one-out entry (absolute, clamped) per data point.
    loo: Vec<[f64; 3]>,
    s_max: f64,
}

fn fit_table(pairs: &[(V3, V3)]) -> Option<TableFit> {
    let mut pts: Vec<(f64, f64)> = pairs.iter().map(|(f, _)| polar(*f)).collect();
    let mut y: Vec<[f64; 3]> = pairs
        .iter()
        .map(|(f, w)| {
            let t = target(*f, *w);
            std::array::from_fn(|c| t[c] / TARGET_SCALE[c])
        })
        .collect();
    // Identity at saturation 0.
    pts.push((0.0, 0.0));
    y.push([0.0; 3]);
    let s_max = pairs
        .iter()
        .map(|(f, _)| f[1])
        .fold(0f64, f64::max)
        .clamp(0.05, 0.95);
    let mut best: Option<(f64, f64, Vec<[f64; 3]>)> = None;
    for &l in &LAMBDAS {
        let Some(loo) = Tps::loo(&pts, &y, l) else {
            continue;
        };
        let score: f64 = loo
            .iter()
            .zip(&y)
            .take(pairs.len())
            .map(|(a, b)| (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f64>())
            .sum();
        if best.as_ref().is_none_or(|b| score < b.0) {
            best = Some((score, l, loo));
        }
    }
    let (_, lambda, loo) = best?;
    let tps = Tps::fit(&pts, &y, lambda)?;
    let loo = loo
        .iter()
        .zip(pairs)
        .map(|(v, (f, _))| {
            let d: [f64; 3] = std::array::from_fn(|c| v[c] * TARGET_SCALE[c]);
            clamp_entry(from_plane(d, f[1]))
        })
        .collect();
    Some(TableFit {
        tps,
        lambda,
        loo,
        s_max,
    })
}

impl TableFit {
    /// The table entry (absolute) at HSV position (hue `h6`, saturation `s`).
    fn entry(&self, h6: f64, s: f64) -> [f64; 3] {
        if s <= 0.0 {
            return [0.0, 1.0, 1.0];
        }
        let (x, y) = polar([h6, s, 1.0]);
        let v = self.tps.eval(x, y);
        let t = if s <= self.s_max {
            0.0
        } else {
            ((s - self.s_max) / (1.0 - self.s_max)).clamp(0.0, 1.0)
        };
        let keep = 1.0 - t * t * (3.0 - 2.0 * t);
        let d: [f64; 3] = std::array::from_fn(|c| v[c] * TARGET_SCALE[c] * keep);
        clamp_entry(from_plane(d, s))
    }

    fn grid(&self, hue: u32, sat: u32) -> Table {
        let mut data = Vec::with_capacity((hue * sat) as usize);
        for h in 0..hue {
            for s in 0..sat {
                let h6 = 6.0 * f64::from(h) / f64::from(hue);
                data.push(self.entry(h6, f64::from(s) / f64::from(sat - 1)));
            }
        }
        Table { hue, sat, data }
    }
}

/// Fits a profile.
pub fn solve(input: &Input) -> Result<Fit, SolveError> {
    let layout = input.layout;
    let n = layout.patches.len().min(input.samples.len());
    let usable: Vec<usize> = (0..n)
        .filter(|&i| input.samples[i].flag.is_none())
        .collect();
    if usable.len() < MIN_PATCHES {
        return Err(SolveError::TooFewPatches {
            usable: usable.len(),
        });
    }
    let ref_xyz: Vec<V3> = layout
        .patches
        .iter()
        .map(|p| colour::lab_to_xyz(p.reference))
        .collect();
    // 1. White balance from the greys (L* 20–95; white only when well below clipping, which
    //    the sampler has flagged already).
    let neutrals: Vec<usize> = usable
        .iter()
        .copied()
        .filter(|&i| layout.is_neutral(i) && (20.0..97.0).contains(&layout.patches[i].reference[0]))
        .collect();
    if neutrals.is_empty() {
        return Err(SolveError::NoNeutrals);
    }
    let cam = |i: usize| input.samples[i].rgb;
    let ratio = |c: usize| {
        let num: f64 = neutrals.iter().map(|&i| cam(i)[c] * cam(i)[1]).sum();
        let den: f64 = neutrals.iter().map(|&i| cam(i)[1] * cam(i)[1]).sum();
        num / den
    };
    let neutral = [ratio(0), 1.0, ratio(2)];
    if !neutral.iter().all(|v| v.is_finite() && *v > 0.0) {
        return Err(SolveError::NoNeutrals);
    }
    // Exposure: the grey nearest L* 66 (Neutral 6.5).
    let anchor = *neutrals
        .iter()
        .min_by(|&&a, &&b| {
            let d = |i: usize| (layout.patches[i].reference[0] - 66.0).abs();
            d(a).total_cmp(&d(b))
        })
        .ok_or(SolveError::NoNeutrals)?;
    // Neutral-normalised values for neutral `n`, with the exposure that puts the anchor
    // grey at its reference Y.
    let y_anchor = ref_xyz[anchor][1];
    let anchored = |n: V3| -> f64 {
        let a: V3 = std::array::from_fn(|k| cam(anchor)[k] / n[k]);
        y_anchor / ((a[0] + a[1] + a[2]) / 3.0).max(1e-12)
    };
    let scaled = |n: V3, k: f64, v: V3| -> V3 { std::array::from_fn(|j| v[j] / n[j] * k) };
    let first: Vec<V3> = (0..n)
        .map(|i| scaled(neutral, anchored(neutral), cam(i)))
        .collect();
    // Uneven light: the greys' exposure and colour ratios.
    let uneven_light = {
        let c = &first;
        let lit: Vec<usize> = neutrals
            .iter()
            .copied()
            .filter(|&i| layout.patches[i].reference[0] > 30.0)
            .collect();
        let spread = |v: Vec<f64>| {
            let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = v.iter().copied().fold(0f64, f64::max);
            if lo > 0.0 { hi / lo - 1.0 } else { 0.0 }
        };
        (lit.len() >= 2).then(|| {
            let y = spread(lit.iter().map(|&i| c[i][1] / ref_xyz[i][1]).collect());
            let rg = spread(lit.iter().map(|&i| c[i][0] / c[i][1]).collect());
            let bg = spread(lit.iter().map(|&i| c[i][2] / c[i][1]).collect());
            y.max(rg).max(bg)
        })
    };
    // 2. Matrix. The greys' white balance starts it; the matrix and the neutral (red and
    //    blue over green) are then refined together on ΔE2000, since a chart's greys are
    //    not exactly neutral.
    let ucam: Vec<V3> = usable.iter().map(|&i| cam(i)).collect();
    let ux: Vec<V3> = usable.iter().map(|&i| ref_xyz[i]).collect();
    let ulab: Vec<Lab> = usable
        .iter()
        .map(|&i| layout.patches[i].reference)
        .collect();
    let k0 = anchored(neutral);
    let uc0: Vec<V3> = ucam.iter().map(|v| scaled(neutral, k0, *v)).collect();
    let start = white_preserving(&uc0, &ux, colour::d50()).ok_or(SolveError::Degenerate)?;
    // The exposure is refined too: a grey that is not exactly neutral cannot fix it alone.
    let refine = |fm0: &M3, n0: V3, k0: f64, cams: &[V3], labs: &[Lab], iters: usize| {
        let mut p0 = pack(fm0).to_vec();
        p0.extend([n0[0].ln(), n0[2].ln(), k0.ln()]);
        let p = levenberg_marquardt(
            &p0,
            |p| {
                let m = unpack(&p[..6]);
                let nn = [p[6].exp(), 1.0, p[7].exp()];
                cams.iter()
                    .zip(labs)
                    .flat_map(|(v, l)| {
                        let c = scaled(nn, p[8].exp(), *v);
                        colour::delta_e2000_parts(colour::xyz_to_lab(colour::mulv(&m, c)), *l)
                    })
                    .collect()
            },
            iters,
        );
        (unpack(&p[..6]), [p[6].exp(), 1.0, p[7].exp()], p[8].exp())
    };
    let (fm, neutral, exposure) = refine(&start, neutral, k0, &ucam, &ulab, 150);
    if !fm.iter().flatten().all(|v| v.is_finite())
        || colour::inv(&fm).is_none()
        || !neutral.iter().all(|v| v.is_finite() && *v > 0.0)
        || !(exposure.is_finite() && exposure > 0.0)
    {
        return Err(SolveError::Degenerate);
    }
    let c: Vec<V3> = (0..n).map(|i| scaled(neutral, exposure, cam(i))).collect();
    let uc: Vec<V3> = usable.iter().map(|&i| c[i]).collect();
    // 3. Table.
    let to_pro = colour::inv(&colour::prophoto_to_xyz()).ok_or(SolveError::Degenerate)?;
    let pm = prophoto_matrix(&fm);
    let hsv_of = |m: &M3, v: V3| colour::rgb_to_hsv(colour::mulv(m, v).map(|x| x.clamp(0.0, 1.0)));
    let want_hsv =
        |i: usize| colour::rgb_to_hsv(colour::mulv(&to_pro, ref_xyz[i]).map(|x| x.clamp(0.0, 1.0)));
    let pairs: Vec<(V3, V3)> = usable
        .iter()
        .map(|&i| (hsv_of(&pm, c[i]), want_hsv(i)))
        .collect();
    let table_fit = if input.table { fit_table(&pairs) } else { None };
    let (hd, sd) = layout.kind.table_divisions();
    let table = table_fit.as_ref().map(|t| t.grid(hd, sd));
    // 4. Root polynomial.
    let root = root_polynomial(&uc, &ux).ok_or(SolveError::Degenerate)?;
    // Report.
    let mut patches = Vec::with_capacity(n);
    let (mut d_before, mut d_matrix, mut d_after, mut d_cv, mut d_root) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (i, &ci) in c.iter().enumerate() {
        let reference = layout.patches[i].reference;
        let before = colour::xyz_to_lab(colour::mulv(&input.before, ci));
        let matrix = colour::xyz_to_lab(colour::mulv(&fm, ci));
        let measured = to_lab_pro(forward_prophoto(&fm, table.as_ref(), ci));
        let left_out = input.samples[i].flag;
        let de_before = colour::delta_e2000(before, reference);
        let de_matrix = colour::delta_e2000(matrix, reference);
        let de_after = colour::delta_e2000(measured, reference);
        let de_cv = usable.iter().position(|&u| u == i).map(|k| {
            // Matrix without the patch.
            let cs: Vec<V3> = ucam
                .iter()
                .enumerate()
                .filter(|e| e.0 != k)
                .map(|e| *e.1)
                .collect();
            let ls: Vec<Lab> = ulab
                .iter()
                .enumerate()
                .filter(|e| e.0 != k)
                .map(|e| *e.1)
                .collect();
            let (m, nn, kk) = refine(&fm, neutral, exposure, &cs, &ls, 25);
            let ci = scaled(nn, kk, cam(i));
            let p = colour::mulv(&prophoto_matrix(&m), ci).map(|x| x.clamp(0.0, 1.0));
            let p = match &table_fit {
                Some(t) => {
                    let e = t.loo[k];
                    let hsv = colour::rgb_to_hsv(p);
                    let h = (hsv[0] + e[0] / 60.0).rem_euclid(6.0);
                    colour::hsv_to_rgb([h, (hsv[1] * e[1]).min(1.0), hsv[2] * e[2]])
                }
                None => p,
            };
            colour::delta_e2000(to_lab_pro(p), reference)
        });
        if left_out.is_none() {
            d_before.push(de_before);
            d_matrix.push(de_matrix);
            d_after.push(de_after);
            d_cv.extend(de_cv);
            d_root.push(colour::delta_e2000(
                colour::xyz_to_lab(apply_root(&root, ci)),
                reference,
            ));
        }
        patches.push(PatchResult {
            reference,
            measured,
            before,
            de_before,
            de_matrix,
            de_after,
            de_cv,
            left_out,
        });
    }
    let report = Report {
        patches,
        before: Stats::of(&d_before),
        matrix: Stats::of(&d_matrix),
        after: Stats::of(&d_after),
        cross_validated: Stats::of(&d_cv),
        root_polynomial: Stats::of(&d_root),
        uneven_light: uneven_light.filter(|s| *s > 0.03),
        lambda: table_fit.as_ref().map_or(0.0, |t| t.lambda),
    };
    Ok(Fit {
        neutral,
        exposure,
        fm,
        table,
        root_polynomial: root,
        report,
    })
}
