//! Pairwise geometry from feature matches.
//!
//! - **Homography**: the normalised direct linear transform (Hartley, "In defense of the
//!   eight-point algorithm", TPAMI 1997; Hartley & Zisserman, *Multiple View Geometry*,
//!   ch. 4): each point set moved to its centroid and scaled to a mean distance of √2, the
//!   2n×9 system solved as the eigenvector of AᵀA with the smallest eigenvalue, then
//!   denormalised.
//! - **Rotation only**: when the focal lengths are known, a camera turning about its centre
//!   maps rays by a rotation, fixed by two matches (Brown, Hartley & Nistér, "Minimal
//!   solutions for panoramic stitching", CVPR 2007). The rotation is the weighted absolute
//!   orientation of the rays (Horn's unit quaternion, JOSA 1987).
//! - **RANSAC** (Fischler & Bolles, CACM 1981) with the adaptive sample count for 99.5 %
//!   confidence, then a least-squares refit on the inliers.
//! - **Acceptance** (Brown & Lowe, "Automatic panoramic image stitching using invariant
//!   features", IJCV 2007, §3.2): a pair overlaps when its inliers exceed 8 + 0.3 × the
//!   matches that fall inside the overlap.

use super::homography::{Homography, inv3, mul3, symmetric_eigen};

/// A correspondence: a point in frame A and where it shows in frame B.
pub type Pair = ((f64, f64), (f64, f64));

/// The similarity that moves `pts` to centroid 0 and mean distance √2.
fn normaliser(pts: impl Iterator<Item = (f64, f64)> + Clone) -> Homography {
    let n = pts.clone().count().max(1) as f64;
    let (sx, sy) = pts.clone().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    let (cx, cy) = (sx / n, sy / n);
    let d = pts.map(|p| (p.0 - cx).hypot(p.1 - cy)).sum::<f64>() / n;
    let s = if d > 1e-12 {
        std::f64::consts::SQRT_2 / d
    } else {
        1.0
    };
    Homography([s, 0.0, -s * cx, 0.0, s, -s * cy, 0.0, 0.0, 1.0])
}

/// The homography `b ≈ H a` of four or more pairs (least squares beyond four).
pub fn dlt(pairs: &[Pair]) -> Option<Homography> {
    if pairs.len() < 4 {
        return None;
    }
    let ta = normaliser(pairs.iter().map(|p| p.0));
    let tb = normaliser(pairs.iter().map(|p| p.1));
    let mut ata = [0.0f64; 81];
    for &(a, b) in pairs {
        let (x, y) = ta.apply(a.0, a.1);
        let (u, v) = tb.apply(b.0, b.1);
        let rows = [
            [-x, -y, -1.0, 0.0, 0.0, 0.0, u * x, u * y, u],
            [0.0, 0.0, 0.0, -x, -y, -1.0, v * x, v * y, v],
        ];
        for r in &rows {
            for i in 0..9 {
                for j in 0..9 {
                    ata[i * 9 + j] += r[i] * r[j];
                }
            }
        }
    }
    let (_, vecs) = symmetric_eigen(&ata, 9);
    let h: [f64; 9] = vecs[0].clone().try_into().ok()?;
    let tbi = inv3(&tb.0)?;
    let m = mul3(&mul3(&tbi, &h), &ta.0);
    let out = Homography(m).normalised();
    out.is_finite().then_some(out)
}

/// Whether three points are nearly on a line (a degenerate sample).
fn collinear(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let area = ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs();
    let scale = (b.0 - a.0).hypot(b.1 - a.1) * (c.0 - a.0).hypot(c.1 - a.1);
    area <= 1e-3 * scale.max(1e-12)
}

/// A small deterministic generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % n.max(1)
    }

    fn distinct<const N: usize>(&mut self, n: usize) -> [usize; N] {
        let mut out = [0usize; N];
        let mut k = 0;
        while k < N {
            let c = self.below(n);
            if !out[..k].contains(&c) {
                out[k] = c;
                k += 1;
            }
        }
        out
    }
}

/// The samples RANSAC needs for `confidence` with `inlier_share` inliers and `m`-point
/// samples.
fn needed(inlier_share: f64, m: i32, confidence: f64) -> usize {
    let p = inlier_share.powi(m).clamp(1e-12, 1.0 - 1e-12);
    ((1.0 - confidence).ln() / (1.0 - p).ln()).ceil() as usize
}

/// A model and the pairs that agree with it.
#[derive(Clone, Debug)]
pub struct Fit<M> {
    pub model: M,
    pub inliers: Vec<usize>,
}

fn residual(h: &Homography, p: &Pair) -> f64 {
    let (u, v) = h.apply(p.0.0, p.0.1);
    let d = (u - p.1.0).hypot(v - p.1.1);
    if d.is_finite() { d } else { f64::INFINITY }
}

fn inliers_of(h: &Homography, pairs: &[Pair], threshold: f64) -> Vec<usize> {
    (0..pairs.len())
        .filter(|&i| residual(h, &pairs[i]) <= threshold)
        .collect()
}

/// The homography with the most pairs within `threshold` pixels, refitted to them.
pub fn ransac_homography(pairs: &[Pair], threshold: f64, seed: u64) -> Option<Fit<Homography>> {
    let n = pairs.len();
    if n < 4 {
        return None;
    }
    let mut rng = Rng(seed | 1);
    let mut best: Option<Fit<Homography>> = None;
    let (mut i, mut limit) = (0usize, 2000usize);
    while i < limit {
        i += 1;
        let s: [usize; 4] = rng.distinct(n);
        let p = s.map(|k| pairs[k]);
        if (0..4).any(|a| {
            let q: Vec<(f64, f64)> = (0..4).filter(|&b| b != a).map(|b| p[b].0).collect();
            collinear(q[0], q[1], q[2])
        }) {
            continue;
        }
        let Some(h) = dlt(&p) else { continue };
        let inl = inliers_of(&h, pairs, threshold);
        if best.as_ref().is_none_or(|b| inl.len() > b.inliers.len()) {
            limit = limit.min(needed(inl.len() as f64 / n as f64, 4, 0.995).max(50));
            best = Some(Fit {
                model: h,
                inliers: inl,
            });
        }
    }
    let mut best = best?;
    // Refit to the inliers, twice (the set can grow).
    for _ in 0..2 {
        let sel: Vec<Pair> = best.inliers.iter().map(|&k| pairs[k]).collect();
        let Some(h) = dlt(&sel) else { break };
        let inl = inliers_of(&h, pairs, threshold);
        if inl.len() < best.inliers.len() {
            break;
        }
        best = Fit {
            model: h,
            inliers: inl,
        };
    }
    (best.inliers.len() >= 4).then_some(best)
}

/// A pinhole camera's intrinsics: focal length in pixels and principal point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intrinsics {
    pub f: f64,
    pub cx: f64,
    pub cy: f64,
}

impl Intrinsics {
    pub fn k(&self) -> [f64; 9] {
        [self.f, 0.0, self.cx, 0.0, self.f, self.cy, 0.0, 0.0, 1.0]
    }

    pub fn k_inv(&self) -> [f64; 9] {
        let g = 1.0 / self.f;
        [g, 0.0, -self.cx * g, 0.0, g, -self.cy * g, 0.0, 0.0, 1.0]
    }

    /// The unit ray of pixel (x, y).
    pub fn ray(&self, x: f64, y: f64) -> [f64; 3] {
        let v = [(x - self.cx) / self.f, (y - self.cy) / self.f, 1.0];
        let n = (v[0] * v[0] + v[1] * v[1] + 1.0).sqrt();
        v.map(|c| c / n)
    }
}

/// The rotation `R` that best maps rays `a` to rays `b` (`b ≈ R a`), weighted (Horn).
pub fn absolute_orientation(a: &[[f64; 3]], b: &[[f64; 3]]) -> [f64; 9] {
    let mut s = [[0.0f64; 3]; 3];
    for (p, q) in a.iter().zip(b) {
        for i in 0..3 {
            for j in 0..3 {
                s[i][j] += p[i] * q[j];
            }
        }
    }
    let [[sxx, sxy, sxz], [syx, syy, syz], [szx, szy, szz]] = s;
    let n = [
        sxx + syy + szz,
        syz - szy,
        szx - sxz,
        sxy - syx,
        syz - szy,
        sxx - syy - szz,
        sxy + syx,
        szx + sxz,
        szx - sxz,
        sxy + syx,
        -sxx + syy - szz,
        syz + szy,
        sxy - syx,
        szx + sxz,
        syz + szy,
        -sxx - syy + szz,
    ];
    let (_, vecs) = symmetric_eigen(&n, 4);
    let q = &vecs[3];
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    [
        w * w + x * x - y * y - z * z,
        2.0 * (x * y - w * z),
        2.0 * (x * z + w * y),
        2.0 * (x * y + w * z),
        w * w - x * x + y * y - z * z,
        2.0 * (y * z - w * x),
        2.0 * (x * z - w * y),
        2.0 * (y * z + w * x),
        w * w - x * x - y * y + z * z,
    ]
}

/// The pixel homography of a rotation between cameras `a` and `b`: `K_b R K_a⁻¹`.
pub fn rotation_homography(r: &[f64; 9], a: &Intrinsics, b: &Intrinsics) -> Homography {
    Homography(mul3(&mul3(&b.k(), r), &a.k_inv())).normalised()
}

/// The rotation (A's rays → B's) with the most pairs within `threshold` pixels, from
/// two-point samples, refitted to its inliers.
pub fn ransac_rotation(
    pairs: &[Pair],
    a: &Intrinsics,
    b: &Intrinsics,
    threshold: f64,
    seed: u64,
) -> Option<Fit<[f64; 9]>> {
    let n = pairs.len();
    if n < 2 {
        return None;
    }
    let ra: Vec<[f64; 3]> = pairs.iter().map(|p| a.ray(p.0.0, p.0.1)).collect();
    let rb: Vec<[f64; 3]> = pairs.iter().map(|p| b.ray(p.1.0, p.1.1)).collect();
    let mut rng = Rng(seed | 1);
    let mut best: Option<Fit<[f64; 9]>> = None;
    let (mut i, mut limit) = (0usize, 500usize);
    let count = |r: &[f64; 9]| {
        let h = rotation_homography(r, a, b);
        // A rotation never maps a ray behind the other camera.
        inliers_of(&h, pairs, threshold)
            .into_iter()
            .filter(|&k| {
                let v = super::homography::mul3v(r, ra[k]);
                v[2] > 0.0
            })
            .collect::<Vec<_>>()
    };
    while i < limit {
        i += 1;
        let [p, q] = rng.distinct::<2>(n);
        // Two rays almost the same fix no rotation.
        let d: f64 = ra[p].iter().zip(&ra[q]).map(|(x, y)| x * y).sum();
        if d > 1.0 - 1e-6 {
            continue;
        }
        let r = absolute_orientation(&[ra[p], ra[q]], &[rb[p], rb[q]]);
        let inl = count(&r);
        if best.as_ref().is_none_or(|b| inl.len() > b.inliers.len()) {
            limit = limit.min(needed(inl.len() as f64 / n as f64, 2, 0.995).max(30));
            best = Some(Fit {
                model: r,
                inliers: inl,
            });
        }
    }
    let mut best = best?;
    for _ in 0..2 {
        let sa: Vec<[f64; 3]> = best.inliers.iter().map(|&k| ra[k]).collect();
        let sb: Vec<[f64; 3]> = best.inliers.iter().map(|&k| rb[k]).collect();
        if sa.len() < 2 {
            break;
        }
        let r = absolute_orientation(&sa, &sb);
        let inl = count(&r);
        if inl.len() < best.inliers.len() {
            break;
        }
        best = Fit {
            model: r,
            inliers: inl,
        };
    }
    (best.inliers.len() >= 2).then_some(best)
}

/// Brown & Lowe's test that a pair truly overlaps: `inliers > 8 + 0.3·in_overlap`, where
/// `in_overlap` counts the matches that fall inside the overlap.
pub fn overlaps(inliers: usize, in_overlap: usize) -> bool {
    inliers as f64 > 8.0 + 0.3 * in_overlap as f64
}

/// Matches whose point in A maps inside a `w × h` frame B under `h` (B's frame) and whose
/// point in B maps back inside A.
pub fn in_overlap(pairs: &[Pair], h: &Homography, a: (u32, u32), b: (u32, u32)) -> usize {
    let Some(hi) = h.inverse() else { return 0 };
    let inside = |p: (f64, f64), s: (u32, u32)| {
        p.0 >= 0.0 && p.1 >= 0.0 && p.0 < f64::from(s.0) && p.1 < f64::from(s.1)
    };
    pairs
        .iter()
        .filter(|p| inside(h.apply(p.0.0, p.0.1), b) && inside(hi.apply(p.1.0, p.1.1), a))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn truth() -> Homography {
        Homography([1.05, 0.03, 12.0, -0.02, 0.97, -7.5, 2e-4, -1e-4, 1.0])
    }

    fn points(n: usize, seed: u64) -> Vec<(f64, f64)> {
        let mut r = Rng(seed);
        (0..n)
            .map(|_| (r.below(1000) as f64 * 0.64, r.below(1000) as f64 * 0.48))
            .collect()
    }

    #[test]
    fn dlt_is_exact_on_exact_correspondences() {
        let h = truth();
        for n in [4usize, 10, 100] {
            let pairs: Vec<Pair> = points(n, 7 + n as u64)
                .into_iter()
                .map(|p| (p, h.apply(p.0, p.1)))
                .collect();
            let got = dlt(&pairs).expect("fitted");
            let err = pairs.iter().map(|p| residual(&got, p)).fold(0.0, f64::max);
            assert!(err < 1e-9, "{n} points: {err:e}");
        }
    }

    #[test]
    fn ransac_finds_the_homography_under_half_outliers() {
        let h = truth();
        let mut r = Rng(99);
        let mut pairs: Vec<Pair> = Vec::new();
        for (k, p) in points(400, 3).into_iter().enumerate() {
            let q = h.apply(p.0, p.1);
            if k % 2 == 0 {
                // Noise of about 0.3 px.
                let e = (r.below(1000) as f64 / 1000.0 - 0.5) * 0.6;
                pairs.push((p, (q.0 + e, q.1 - e)));
            } else {
                pairs.push((p, (r.below(640) as f64, r.below(480) as f64)));
            }
        }
        let fit = ransac_homography(&pairs, 2.0, 1).expect("found");
        assert!(fit.inliers.len() >= 195, "{} inliers", fit.inliers.len());
        let d = fit.model.distance(&h, 640, 480);
        assert!(d < 0.3, "off by {d:.3} px");
        let inside = in_overlap(&pairs, &fit.model, (640, 480), (640, 480));
        assert!(overlaps(fit.inliers.len(), inside));
    }

    #[test]
    fn two_point_rotation_recovers_a_turn() {
        let a = Intrinsics {
            f: 800.0,
            cx: 320.0,
            cy: 240.0,
        };
        let r = super::super::homography::rotation([0.02, 0.35, -0.01]);
        let h = rotation_homography(&r, &a, &a);
        let pairs: Vec<Pair> = points(60, 5)
            .into_iter()
            .filter_map(|p| {
                let q = h.apply(p.0, p.1);
                (q.0.is_finite() && q.0 > -100.0).then_some((p, q))
            })
            .collect();
        let fit = ransac_rotation(&pairs, &a, &a, 1.0, 3).expect("found");
        let d = super::super::homography::rotation_distance(&fit.model, &r);
        assert!(d < 1e-9, "off by {d:e} rad");
        assert_eq!(fit.inliers.len(), pairs.len());
    }
}
