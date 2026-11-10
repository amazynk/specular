//! Colour deconvolution of brightfield stains (Ruifrok and Johnston, "Quantification of
//! histochemical staining by color deconvolution", Anal. Quant. Cytol. Histol. 23, 2001).
//!
//! Light through a stained section follows Beer–Lambert: each channel's optical density
//! `OD = -log10(I / I0)` is a sum over stains of concentration × the stain's OD vector. With
//! the stains' unit OD vectors as the rows of `M`, `OD = C · M` (row vectors), so
//! `C = OD · M⁻¹`. Two-stain sets get a third, residual row: the cross product of the two,
//! normalised (scikit-image's convention), which keeps `M` invertible and collects what the
//! two stains do not explain.
//!
//! Values are sRGB codes scaled to 0..=1 with `I0 = 1` (white), floored at [`MIN_CODE`] as
//! scikit-image's `separate_stains` does, so this module and `skimage.color.separate_stains`
//! agree to floating-point rounding (OD here is log10; scikit-image divides the natural log
//! by `ln 1e-6`, a factor 6 apart: `ours = 6 × theirs`).
//!
//! [`macenko`] estimates two stain vectors from a region's pixels (Macenko et al., "A method
//! for normalizing histology slides for quantitative analysis", ISBI 2009); [`display`]
//! turns stains (and, optionally, a Macenko normalisation to a [`Reference`]) into the
//! three matrices the GPU applies per pixel, and [`render`] is the same arithmetic on the
//! CPU, for tests and oracles.

/// A row vector (R, G, B).
pub type V3 = [f64; 3];
/// Three row vectors.
pub type M3 = [[f64; 3]; 3];

/// The smallest code (0..=1) logs are taken of; black pixels have this OD.
pub const MIN_CODE: f64 = 1e-6;

/// Haematoxylin's unit OD vector (Ruifrok and Johnston 2001, normalised).
pub const HEMATOXYLIN: V3 = [0.650, 0.704, 0.286];
/// Eosin's.
pub const EOSIN: V3 = [0.072, 0.990, 0.105];
/// DAB's (3,3'-diaminobenzidine).
pub const DAB: V3 = [0.268, 0.570, 0.776];

/// A stain combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StainSet {
    /// Haematoxylin and eosin (residual third).
    He,
    /// Haematoxylin and DAB (residual third).
    Hdab,
    /// Haematoxylin, eosin and DAB.
    HeDab,
}

/// A stain, for names and colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stain {
    Hematoxylin,
    Eosin,
    Dab,
    Residual,
}

impl Stain {
    /// A stable lower-case id (settings, scripts, message ids).
    pub fn id(self) -> &'static str {
        match self {
            Stain::Hematoxylin => "hematoxylin",
            Stain::Eosin => "eosin",
            Stain::Dab => "dab",
            Stain::Residual => "residual",
        }
    }
}

impl StainSet {
    pub const ALL: [StainSet; 3] = [StainSet::He, StainSet::Hdab, StainSet::HeDab];

    /// A stable lower-case id.
    pub fn id(self) -> &'static str {
        match self {
            StainSet::He => "he",
            StainSet::Hdab => "hdab",
            StainSet::HeDab => "hedab",
        }
    }

    pub fn from_id(s: &str) -> Option<StainSet> {
        StainSet::ALL.into_iter().find(|v| v.id() == s)
    }

    /// The channels, in order (the third is the residual for two-stain sets).
    pub fn stains(self) -> [Stain; 3] {
        match self {
            StainSet::He => [Stain::Hematoxylin, Stain::Eosin, Stain::Residual],
            StainSet::Hdab => [Stain::Hematoxylin, Stain::Dab, Stain::Residual],
            StainSet::HeDab => [Stain::Hematoxylin, Stain::Eosin, Stain::Dab],
        }
    }

    /// Real stains (2 or 3); the rest is residual.
    pub fn count(self) -> usize {
        match self {
            StainSet::HeDab => 3,
            _ => 2,
        }
    }
}

/// A stain set's OD vectors (rows, unit length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stains {
    pub set: StainSet,
    pub vectors: M3,
    /// The first two rows were estimated from a region (else the published vectors).
    pub estimated: bool,
}

impl Stains {
    /// The published vectors.
    pub fn published(set: StainSet) -> Stains {
        let vectors = match set {
            StainSet::He => with_residual(HEMATOXYLIN, EOSIN),
            StainSet::Hdab => with_residual(HEMATOXYLIN, DAB),
            StainSet::HeDab => [unit(HEMATOXYLIN), unit(EOSIN), unit(DAB)],
        };
        Stains {
            set,
            vectors,
            estimated: false,
        }
    }

    /// `set` with its first two stains replaced by `two` (a [`macenko`] estimate); the
    /// third, for H&E-DAB, stays DAB's published vector. `None` when the vectors are
    /// degenerate.
    pub fn with_estimate(set: StainSet, two: [V3; 2]) -> Option<Stains> {
        let vectors = match set {
            StainSet::HeDab => [unit(two[0]), unit(two[1]), unit(DAB)],
            _ => with_residual(two[0], two[1]),
        };
        let s = Stains {
            set,
            vectors,
            estimated: true,
        };
        s.unmix().map(|_| s)
    }

    /// `M⁻¹`: concentrations are `OD · unmix` (row vectors). `None` when `M` is singular.
    pub fn unmix(&self) -> Option<M3> {
        invert(&self.vectors)
    }
}

/// `v` scaled to unit length (unchanged when zero).
pub fn unit(v: V3) -> V3 {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 0.0 { v.map(|x| x / n) } else { v }
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Two stains and their residual: the cross product (scikit-image's choice).
fn with_residual(a: V3, b: V3) -> M3 {
    let (a, b) = (unit(a), unit(b));
    [a, b, unit(cross(a, b))]
}

/// `v · m` (row vector times matrix).
pub fn vec_mat(v: V3, m: &M3) -> V3 {
    [0, 1, 2].map(|c| v[0] * m[0][c] + v[1] * m[1][c] + v[2] * m[2][c])
}

/// `a · b`.
pub fn mat_mul(a: &M3, b: &M3) -> M3 {
    [vec_mat(a[0], b), vec_mat(a[1], b), vec_mat(a[2], b)]
}

/// The inverse of `m`, `None` when (nearly) singular.
pub fn invert(m: &M3) -> Option<M3> {
    let [a, b, c] = *m;
    let det = dot(a, cross(b, c));
    if !det.is_finite() || det.abs() < 1e-9 {
        return None;
    }
    // Columns of the inverse are the cross products of the rows, over the determinant.
    let (x, y, z) = (cross(b, c), cross(c, a), cross(a, b));
    Some([
        [x[0] / det, y[0] / det, z[0] / det],
        [x[1] / det, y[1] / det, z[1] / det],
        [x[2] / det, y[2] / det, z[2] / det],
    ])
}

/// Optical density of a code (0..=1).
pub fn od(code: f64) -> f64 {
    -code.max(MIN_CODE).log10()
}

/// A pixel's optical densities.
pub fn od3(rgb: V3) -> V3 {
    rgb.map(od)
}

/// The concentrations (OD units) of a pixel's codes (0..=1), unclamped.
pub fn concentrations(rgb: V3, unmix: &M3) -> V3 {
    vec_mat(od3(rgb), unmix)
}

/// Why stain vectors could not be estimated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EstimateError {
    /// Fewer stained pixels than [`MIN_PIXELS`] (the region is mostly background).
    TooFewPixels(usize),
    /// The stained pixels do not span two colours.
    Degenerate,
}

impl std::fmt::Display for EstimateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EstimateError::TooFewPixels(n) => write!(f, "{n} stained pixels, too few"),
            EstimateError::Degenerate => f.write_str("the stained pixels hold one colour"),
        }
    }
}

/// Stained pixels a region needs for an estimate.
pub const MIN_PIXELS: usize = 100;
/// Macenko's OD threshold: pixels with any channel's OD below it are background.
pub const BETA: f64 = 0.15;
/// Macenko's robust extreme: the angles' `ALPHA` and `100 − ALPHA` percentiles.
pub const ALPHA: f64 = 1.0;
/// Concentrations' percentile taken as their maximum.
pub const MAX_PERCENTILE: f64 = 99.0;

/// Two stains estimated from a region, haematoxylin (the bluer, larger red OD) first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub vectors: [V3; 2],
    /// Each stain's [`MAX_PERCENTILE`] concentration in the region.
    pub max_c: [f64; 2],
    /// Stained pixels used.
    pub pixels: usize,
}

/// The `p`th percentile of sorted `v` (linear interpolation, numpy's default).
fn percentile(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let x = (p / 100.0).clamp(0.0, 1.0) * (v.len() - 1) as f64;
    let i = x.floor() as usize;
    let f = x - i as f64;
    match v.get(i + 1) {
        Some(&n) => v[i] + (n - v[i]) * f,
        None => v[i],
    }
}

/// Eigenvalues and unit eigenvectors (columns of the result) of symmetric `a`, by Jacobi
/// rotations, sorted by decreasing eigenvalue.
fn eigen(mut a: M3) -> ([f64; 3], M3) {
    let mut v: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..64 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off < 1e-15 {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in &mut a {
                let (akp, akq) = (row[p], row[q]);
                row[p] = c * akp - s * akq;
                row[q] = s * akp + c * akq;
            }
            let (rp, rq) = (a[p], a[q]);
            a[p] = [0, 1, 2].map(|k| c * rp[k] - s * rq[k]);
            a[q] = [0, 1, 2].map(|k| s * rp[k] + c * rq[k]);
            for row in &mut v {
                let (vp, vq) = (row[p], row[q]);
                row[p] = c * vp - s * vq;
                row[q] = s * vp + c * vq;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&x, &y| a[y][y].total_cmp(&a[x][x]));
    let vals = order.map(|i| a[i][i]);
    let vecs = [0, 1, 2].map(|r| order.map(|i| v[r][i]));
    (vals, vecs)
}

/// Estimates two stain vectors from `pixels` (8-bit sRGB codes) by Macenko's method: the
/// stained pixels' ODs projected onto the plane of their two main directions, and the
/// directions at the [`ALPHA`] and `100 − ALPHA` percentile angles in that plane.
pub fn macenko(pixels: &[[u8; 3]]) -> Result<Estimate, EstimateError> {
    let ods: Vec<V3> = pixels
        .iter()
        .map(|p| od3(p.map(|v| f64::from(v) / 255.0)))
        .filter(|o| o.iter().all(|&x| x >= BETA))
        .collect();
    if ods.len() < MIN_PIXELS {
        return Err(EstimateError::TooFewPixels(ods.len()));
    }
    // Covariance (numpy.cov: mean removed, n − 1).
    let n = ods.len() as f64;
    let mut mean = [0.0; 3];
    for o in &ods {
        for c in 0..3 {
            mean[c] += o[c] / n;
        }
    }
    let mut cov = [[0.0; 3]; 3];
    for o in &ods {
        let d = [o[0] - mean[0], o[1] - mean[1], o[2] - mean[2]];
        for r in 0..3 {
            for c in 0..3 {
                cov[r][c] += d[r] * d[c] / (n - 1.0);
            }
        }
    }
    let (vals, vecs) = eigen(cov);
    if vals[1].is_nan() || vals[1] <= 1e-12 {
        return Err(EstimateError::Degenerate);
    }
    let col = |i: usize| -> V3 { [vecs[0][i], vecs[1][i], vecs[2][i]] };
    // The plane's axes, pointing into the positive octant.
    let orient = |v: V3| {
        if v[0] + v[1] + v[2] < 0.0 {
            v.map(|x| -x)
        } else {
            v
        }
    };
    let (e1, e2) = (orient(col(0)), orient(col(1)));
    let mut phi: Vec<f64> = ods.iter().map(|o| dot(*o, e2).atan2(dot(*o, e1))).collect();
    phi.sort_by(f64::total_cmp);
    let (lo, hi) = (percentile(&phi, ALPHA), percentile(&phi, 100.0 - ALPHA));
    let dir = |a: f64| unit(orient([0, 1, 2].map(|c| e1[c] * a.cos() + e2[c] * a.sin())));
    let (v_lo, v_hi) = (dir(lo), dir(hi));
    let vectors = if v_lo[0] > v_hi[0] {
        [v_lo, v_hi]
    } else {
        [v_hi, v_lo]
    };
    let Some(max_c) = max_concentrations(&ods, vectors) else {
        return Err(EstimateError::Degenerate);
    };
    Ok(Estimate {
        vectors,
        max_c,
        pixels: ods.len(),
    })
}

/// The [`MAX_PERCENTILE`] concentrations of `ods` under two stains (least squares).
fn max_concentrations(ods: &[V3], two: [V3; 2]) -> Option<[f64; 2]> {
    let p = pinv2(two)?;
    let mut c0 = Vec::with_capacity(ods.len());
    let mut c1 = Vec::with_capacity(ods.len());
    for o in ods {
        c0.push(dot(*o, p[0]));
        c1.push(dot(*o, p[1]));
    }
    c0.sort_by(f64::total_cmp);
    c1.sort_by(f64::total_cmp);
    Some([
        percentile(&c0, MAX_PERCENTILE),
        percentile(&c1, MAX_PERCENTILE),
    ])
}

/// Least-squares concentrations of two stains: `c_k = OD · p[k]`, from the 2 × 3 rows
/// `two` (`p = (V Vᵀ)⁻¹ V`).
fn pinv2(two: [V3; 2]) -> Option<[V3; 2]> {
    let (a, b) = (two[0], two[1]);
    let (aa, ab, bb) = (dot(a, a), dot(a, b), dot(b, b));
    let det = aa * bb - ab * ab;
    if det.abs() < 1e-12 {
        return None;
    }
    let p0 = [0, 1, 2].map(|c| (bb * a[c] - ab * b[c]) / det);
    let p1 = [0, 1, 2].map(|c| (aa * b[c] - ab * a[c]) / det);
    Some([p0, p1])
}

/// What a normalisation maps to: two stain vectors and, when known, their maximum
/// concentrations (log10 OD units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub vectors: [V3; 2],
    pub max_c: Option<[f64; 2]>,
}

impl Reference {
    /// The H&E reference most Macenko implementations ship (vectors as published with the
    /// method's reference code; maximum concentrations converted from natural-log to log10
    /// OD).
    pub fn standard_he() -> Reference {
        Reference {
            vectors: [
                unit([0.5626, 0.7201, 0.4062]),
                unit([0.2159, 0.8012, 0.5581]),
            ],
            max_c: Some([
                1.9705 / std::f64::consts::LN_10,
                1.0308 / std::f64::consts::LN_10,
            ]),
        }
    }

    /// A region's estimate as a reference.
    pub fn of(e: &Estimate) -> Reference {
        Reference {
            vectors: e.vectors,
            max_c: Some(e.max_c),
        }
    }
}

/// How the stains are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Each shown stain as the light it alone lets through (white where it is absent).
    Intensity,
    /// Each shown stain's concentration as brightness on black (grey for one stain, the
    /// stains' colours added for several).
    Od,
}

/// Concentrations shown at full brightness in [`Mode::Od`].
pub const OD_MAX: f64 = 1.0;

/// The per-pixel arithmetic, as matrices (row-vector convention):
/// `c = max(OD · unmix, 0)`, masked to the shown stains; [`Mode::Intensity`] shows
/// `10^-(c · recon)` and [`Mode::Od`] `Σ min(c_s / OD_MAX, 1) · tint_s`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub unmix: M3,
    pub recon: M3,
    /// Display colours (sRGB codes, 0..=1) of the stains in [`Mode::Od`].
    pub tint: M3,
}

/// A stain's colour when it is shown alone: the light one unit of it passes, scaled so its
/// brightest channel is 1.
fn tint(v: V3) -> V3 {
    let t = v.map(|x| 10f64.powf(-x));
    let m = t[0].max(t[1]).max(t[2]).max(1e-6);
    t.map(|x| x / m)
}

/// The matrices for `stains`, normalised from `source` (the slide's own estimate) to
/// `reference` when both are given. `None` when the vectors are singular.
pub fn display(stains: &Stains, norm: Option<(&Estimate, &Reference)>) -> Option<Display> {
    let mut unmix = stains.unmix()?;
    let mut recon = stains.vectors;
    if let Some((src, reference)) = norm
        && stains.set != StainSet::HeDab
    {
        // Concentrations under the slide's own vectors, scaled to the reference's range,
        // then recombined with the reference's vectors.
        let own = Stains::with_estimate(stains.set, src.vectors)?;
        unmix = own.unmix()?;
        if let Some(rm) = reference.max_c {
            for (k, (&r, &s)) in rm.iter().zip(&src.max_c).enumerate() {
                let f = if s > 1e-6 { r / s } else { 1.0 };
                for row in &mut unmix {
                    row[k] *= f;
                }
            }
        }
        recon = with_residual(reference.vectors[0], reference.vectors[1]);
    }
    let names = stains.set.stains();
    let mut tints = [[1.0; 3]; 3];
    for (k, s) in names.iter().enumerate() {
        if *s != Stain::Residual {
            tints[k] = tint(stains.vectors[k]);
        }
    }
    Some(Display {
        unmix,
        recon,
        tint: tints,
    })
}

/// The tints [`Mode::Od`] uses with stains `visible`: grey when one stain is shown, the
/// stains' colours when several are.
pub fn tints(d: &Display, visible: [bool; 3]) -> M3 {
    if visible.iter().filter(|v| **v).count() == 1 {
        [[1.0; 3]; 3]
    } else {
        d.tint
    }
}

/// One pixel (sRGB codes, 0..=1) as [`Display`] shows it with stains `visible` in `mode`:
/// the GPU's arithmetic (`image.wgsl`), in f64.
pub fn render(rgb: V3, d: &Display, visible: [bool; 3], mode: Mode) -> V3 {
    let c = vec_mat(od3(rgb), &d.unmix);
    let c = [0, 1, 2].map(|k| if visible[k] { c[k].max(0.0) } else { 0.0 });
    match mode {
        Mode::Intensity => {
            let o = vec_mat(c, &d.recon);
            o.map(|x| 10f64.powf(-x).clamp(0.0, 1.0))
        }
        Mode::Od => {
            let t = tints(d, visible);
            let mut out = [0.0; 3];
            for k in 0..3 {
                let b = (c[k] / OD_MAX).min(1.0);
                for ch in 0..3 {
                    out[ch] += b * t[k][ch];
                }
            }
            out.map(|x| x.clamp(0.0, 1.0))
        }
    }
}

#[cfg(test)]
mod tests;
