//! Line segments: LSD, written from the paper (R. Grompone von Gioi, J. Jakubowicz,
//! J.-M. Morel, G. Randall, "LSD: a Fast Line Segment Detector with a False Detection
//! Control", IEEE TPAMI 32(4), 2010; the IPOL 2012 article's description). No reference
//! code was used.
//!
//! 1. The picture is scaled by 0.8 with a Gaussian (σ = 0.6 / 0.8), which takes out the
//!    staircase of aliased edges.
//! 2. Gradient by a 2 × 2 difference; the level-line angle is perpendicular to it. Pixels
//!    whose gradient is under `q / sin τ` (q = 2 grey levels of quantisation, τ = 22.5°) are
//!    not used: their angle is noise.
//! 3. Pixels are taken in falling gradient order (1024 bins). Each unused one seeds a
//!    **line-support region**: 8-connected neighbours whose angle is within τ of the
//!    region's mean angle join it.
//! 4. The region is approximated by a **rectangle**: centre of mass by gradient, the
//!    principal axis of its inertia, the extent of its pixels along and across it.
//! 5. A region with too few of the rectangle's pixels (density under 0.7) is refined: a
//!    tighter angle tolerance from the seed's neighbourhood, then a shrinking radius.
//! 6. **Validation (a contrario):** with `n` pixels in the rectangle and `k` of them aligned
//!    (angle within `p·π`), `NFA = (N·M)^{5/2} · γ · B(n, k, p)`, `B` the binomial tail and
//!    γ = 11 the precisions tried. The rectangle is improved (finer precisions, narrower
//!    widths) and kept when `NFA ≤ 1`: on noise, fewer than one false detection is
//!    expected per picture.
//!
//! Lengths and positions come back in the input's pixels (pixel centres at `i + 0.5`).

use std::f64::consts::PI;

/// One detected segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: (f64, f64),
    pub b: (f64, f64),
    pub width: f64,
    /// −log10 NFA (positive: meaningful).
    pub log_nfa: f64,
}

impl Segment {
    pub fn len(&self) -> f64 {
        (self.b.0 - self.a.0).hypot(self.b.1 - self.a.1)
    }
}

const SCALE: f64 = 0.8;
const SIGMA_SCALE: f64 = 0.6;
const QUANT: f64 = 2.0;
const ANGLE_TH_DEG: f64 = 22.5;
const DENSITY: f64 = 0.7;
const BINS: usize = 1024;
const NOTDEF: f64 = -1024.0;

/// The detector's grey levels are 0..255: `lum` (any range) is scaled by `levels`
/// (255 for 0..1 data).
pub fn detect(lum: &[f32], w: usize, h: usize, levels: f64) -> Vec<Segment> {
    if w < 4 || h < 4 || lum.len() < w * h {
        return Vec::new();
    }
    let img: Vec<f64> = lum[..w * h]
        .iter()
        .map(|v| f64::from(*v) * levels)
        .collect();
    let (s, sw, sh) = downscale(&img, w, h);
    let mut lsd = Lsd::new(&s, sw, sh);
    lsd.run()
        .into_iter()
        .map(|g| Segment {
            a: ((g.x1 + 0.5) / SCALE, (g.y1 + 0.5) / SCALE),
            b: ((g.x2 + 0.5) / SCALE, (g.y2 + 0.5) / SCALE),
            width: g.width / SCALE,
            log_nfa: g.log_nfa,
        })
        .collect()
}

/// Gaussian-filtered resampling by [`SCALE`] (separable).
fn downscale(img: &[f64], w: usize, h: usize) -> (Vec<f64>, usize, usize) {
    let sigma = SIGMA_SCALE / SCALE;
    let r = (3.0 * sigma).ceil() as i64;
    let nw = ((w as f64) * SCALE).floor().max(1.0) as usize;
    let nh = ((h as f64) * SCALE).floor().max(1.0) as usize;
    let taps = |n_out: usize, n_in: usize| -> Vec<(usize, Vec<f64>)> {
        (0..n_out)
            .map(|o| {
                let c = (o as f64 + 0.5) / SCALE - 0.5;
                let c0 = c.round() as i64;
                let lo = (c0 - r).max(0);
                let hi = (c0 + r).min(n_in as i64 - 1);
                let ws: Vec<f64> = (lo..=hi)
                    .map(|i| (-((i as f64 - c).powi(2)) / (2.0 * sigma * sigma)).exp())
                    .collect();
                let sum: f64 = ws.iter().sum();
                (lo as usize, ws.into_iter().map(|v| v / sum).collect())
            })
            .collect()
    };
    let tx = taps(nw, w);
    let ty = taps(nh, h);
    let mut mid = vec![0.0; nw * h];
    for y in 0..h {
        let row = &img[y * w..(y + 1) * w];
        for (x, (lo, ws)) in tx.iter().enumerate() {
            mid[y * nw + x] = ws.iter().enumerate().map(|(i, k)| k * row[lo + i]).sum();
        }
    }
    let mut out = vec![0.0; nw * nh];
    for (y, (lo, ws)) in ty.iter().enumerate() {
        for x in 0..nw {
            out[y * nw + x] = ws
                .iter()
                .enumerate()
                .map(|(i, k)| k * mid[(lo + i) * nw + x])
                .sum();
        }
    }
    (out, nw, nh)
}

/// A rectangle approximating a region.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: f64,
    cx: f64,
    cy: f64,
    theta: f64,
    dx: f64,
    dy: f64,
    /// Angle tolerance (radians) and the matching probability `prec / π`.
    prec: f64,
    p: f64,
}

#[derive(Clone, Copy, Debug)]
struct Found {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    width: f64,
    log_nfa: f64,
}

struct Lsd {
    w: usize,
    h: usize,
    angle: Vec<f64>,
    mag: Vec<f64>,
    used: Vec<bool>,
    log_nt: f64,
    prec: f64,
    p: f64,
}

/// `|a − b|` on the circle (0..π).
fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(2.0 * PI);
    if d > PI { 2.0 * PI - d } else { d }
}

/// `ln Γ(x)` (Lanczos, g = 7, nine terms).
fn ln_gamma(x: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (PI / (PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + 7.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// −log10 of the number of false alarms of a rectangle with `n` pixels, `k` aligned at
/// probability `p`, among `10^log_nt` tests.
fn log_nfa(n: u64, k: u64, p: f64, log_nt: f64) -> f64 {
    if n == 0 || k == 0 {
        return -log_nt;
    }
    if n == k {
        return -log_nt - (n as f64) * p.log10();
    }
    let (nf, kf) = (n as f64, k as f64);
    let ln10 = std::f64::consts::LN_10;
    // The first term of the tail, then each next one by the ratio of binomial terms.
    let ln_term = ln_gamma(nf + 1.0) - ln_gamma(kf + 1.0) - ln_gamma(nf - kf + 1.0)
        + kf * p.ln()
        + (nf - kf) * (1.0 - p).ln();
    let first = ln_term.exp();
    if first == 0.0 {
        // Under f64's range: the tail is about its first term.
        return -ln_term / ln10 - log_nt;
    }
    let mut term = first;
    let mut sum = first;
    let q = p / (1.0 - p);
    let mut i = kf;
    while i < nf {
        term *= (nf - i) / (i + 1.0) * q;
        sum += term;
        // The rest is bounded by a geometric series of the current ratio.
        let ratio = (nf - i - 1.0) / (i + 2.0) * q;
        if ratio < 1.0 && term * ratio / (1.0 - ratio) < sum * 1e-3 {
            break;
        }
        i += 1.0;
    }
    -(sum.ln() / ln10) - log_nt
}

impl Lsd {
    fn new(img: &[f64], w: usize, h: usize) -> Lsd {
        let prec = ANGLE_TH_DEG.to_radians();
        let rho = QUANT / prec.sin();
        let mut angle = vec![NOTDEF; w * h];
        let mut mag = vec![0.0; w * h];
        for y in 0..h - 1 {
            for x in 0..w - 1 {
                let i = y * w + x;
                let (a, b, c, d) = (img[i], img[i + 1], img[i + w], img[i + w + 1]);
                let gx = (b + d - a - c) / 2.0;
                let gy = (c + d - a - b) / 2.0;
                let m = gx.hypot(gy);
                mag[i] = m;
                if m > rho {
                    // The level line is perpendicular to the gradient.
                    angle[i] = gx.atan2(-gy);
                }
            }
        }
        let log_nt = 5.0 * ((w as f64).log10() + (h as f64).log10()) / 2.0 + 11f64.log10();
        Lsd {
            w,
            h,
            angle,
            mag,
            used: vec![false; w * h],
            log_nt,
            prec,
            p: ANGLE_TH_DEG / 180.0,
        }
    }

    fn run(&mut self) -> Vec<Found> {
        let w = self.w;
        // Pseudo-ordering by gradient.
        let max = self.mag.iter().copied().fold(0.0, f64::max);
        if max <= 0.0 {
            return Vec::new();
        }
        let mut bins: Vec<Vec<u32>> = vec![Vec::new(); BINS];
        for (i, &a) in self.angle.iter().enumerate() {
            if a != NOTDEF {
                let b = ((self.mag[i] / max) * (BINS as f64 - 1.0)) as usize;
                bins[b.min(BINS - 1)].push(i as u32);
            }
        }
        // Least region size that could be meaningful if all its pixels were aligned.
        let min_reg = (-self.log_nt / self.p.log10()).max(1.0) as usize;
        let mut out = Vec::new();
        let mut region = Vec::new();
        for bin in bins.iter().rev() {
            for &i in bin {
                let i = i as usize;
                if self.used[i] || self.angle[i] == NOTDEF {
                    continue;
                }
                let (x, y) = (i % w, i / w);
                let mut reg_angle = self.grow(x, y, self.prec, &mut region);
                if region.len() < min_reg {
                    continue;
                }
                let mut rect = self.rect(&region, reg_angle, self.prec, self.p);
                if !self.refine(&mut region, &mut reg_angle, &mut rect, x, y, min_reg) {
                    continue;
                }
                let (nfa, rect) = self.improve(rect);
                if nfa <= 0.0 {
                    continue;
                }
                out.push(Found {
                    x1: rect.x1,
                    y1: rect.y1,
                    x2: rect.x2,
                    y2: rect.y2,
                    width: rect.width,
                    log_nfa: nfa,
                });
            }
        }
        out
    }

    fn aligned(&self, i: usize, theta: f64, prec: f64) -> bool {
        let a = self.angle[i];
        a != NOTDEF && angle_diff(a, theta) <= prec
    }

    /// Grows a region from (x, y) at tolerance `prec`; returns its angle.
    fn grow(&mut self, x: usize, y: usize, prec: f64, region: &mut Vec<(usize, usize)>) -> f64 {
        let w = self.w;
        region.clear();
        region.push((x, y));
        let i0 = y * w + x;
        let mut ang = self.angle[i0];
        let (mut sx, mut sy) = (ang.cos(), ang.sin());
        self.used[i0] = true;
        let mut k = 0;
        while k < region.len() {
            let (px, py) = region[k];
            k += 1;
            for yy in py.saturating_sub(1)..=(py + 1).min(self.h - 1) {
                for xx in px.saturating_sub(1)..=(px + 1).min(w - 1) {
                    let j = yy * w + xx;
                    if !self.used[j] && self.aligned(j, ang, prec) {
                        self.used[j] = true;
                        region.push((xx, yy));
                        sx += self.angle[j].cos();
                        sy += self.angle[j].sin();
                        ang = sy.atan2(sx);
                    }
                }
            }
        }
        ang
    }

    /// The rectangle of a region (see the module documentation).
    fn rect(&self, region: &[(usize, usize)], reg_angle: f64, prec: f64, p: f64) -> Rect {
        let w = self.w;
        let (mut sum, mut cx, mut cy) = (0.0, 0.0, 0.0);
        for &(x, y) in region {
            let m = self.mag[y * w + x];
            sum += m;
            cx += m * x as f64;
            cy += m * y as f64;
        }
        let sum = sum.max(1e-12);
        cx /= sum;
        cy /= sum;
        let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
        for &(x, y) in region {
            let m = self.mag[y * w + x];
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            cxx += m * dx * dx;
            cyy += m * dy * dy;
            cxy += m * dx * dy;
        }
        // The major axis of the inertia; pointed as the level lines run.
        let mut theta = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
        if angle_diff(theta, reg_angle) > PI / 2.0 {
            theta += PI;
        }
        let (dx, dy) = (theta.cos(), theta.sin());
        let (mut l0, mut l1, mut w0, mut w1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for &(x, y) in region {
            let (ex, ey) = (x as f64 - cx, y as f64 - cy);
            let l = ex * dx + ey * dy;
            let ww = -ex * dy + ey * dx;
            l0 = l0.min(l);
            l1 = l1.max(l);
            w0 = w0.min(ww);
            w1 = w1.max(ww);
        }
        let mut width = w1 - w0;
        // The rectangle is at least one pixel wide.
        let wc = (w0 + w1) / 2.0;
        if width < 1.0 {
            width = 1.0;
        }
        let (ox, oy) = (cx - wc * dy, cy + wc * dx);
        Rect {
            x1: ox + l0 * dx,
            y1: oy + l0 * dy,
            x2: ox + l1 * dx,
            y2: oy + l1 * dy,
            width,
            cx: ox + (l0 + l1) / 2.0 * dx,
            cy: oy + (l0 + l1) / 2.0 * dy,
            theta,
            dx,
            dy,
            prec,
            p,
        }
    }

    /// Pixels (inside the picture) whose centres lie in `r`, and those aligned with it.
    fn count(&self, r: &Rect) -> (u64, u64) {
        let half = r.width / 2.0;
        let (nx, ny) = (-r.dy, r.dx);
        let c = [
            (r.x1 - nx * half, r.y1 - ny * half),
            (r.x2 - nx * half, r.y2 - ny * half),
            (r.x2 + nx * half, r.y2 + ny * half),
            (r.x1 + nx * half, r.y1 + ny * half),
        ];
        let xmin = c
            .iter()
            .map(|p| p.0)
            .fold(f64::MAX, f64::min)
            .ceil()
            .max(0.0) as i64;
        let xmax = c
            .iter()
            .map(|p| p.0)
            .fold(f64::MIN, f64::max)
            .floor()
            .min(self.w as f64 - 1.0) as i64;
        let (mut n, mut k) = (0u64, 0u64);
        for x in xmin..=xmax {
            let xf = x as f64;
            // The quadrilateral's span at this column.
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for e in 0..4 {
                let (a, b) = (c[e], c[(e + 1) % 4]);
                if (a.0 - xf) * (b.0 - xf) <= 0.0 {
                    if (b.0 - a.0).abs() < 1e-12 {
                        lo = lo.min(a.1.min(b.1));
                        hi = hi.max(a.1.max(b.1));
                    } else {
                        let t = (xf - a.0) / (b.0 - a.0);
                        let y = a.1 + t * (b.1 - a.1);
                        lo = lo.min(y);
                        hi = hi.max(y);
                    }
                }
            }
            if lo > hi {
                continue;
            }
            let y0 = lo.ceil().max(0.0) as i64;
            let y1 = hi.floor().min(self.h as f64 - 1.0) as i64;
            for y in y0..=y1 {
                n += 1;
                if self.aligned(y as usize * self.w + x as usize, r.theta, r.prec) {
                    k += 1;
                }
            }
        }
        (n, k)
    }

    fn nfa(&self, r: &Rect) -> f64 {
        let (n, k) = self.count(r);
        log_nfa(n, k, r.p, self.log_nt)
    }

    /// Finer precisions and narrower widths, keeping the best NFA.
    fn improve(&self, r: Rect) -> (f64, Rect) {
        let mut best = r;
        let mut nfa = self.nfa(&r);
        let try_set = |c: Rect, best: &mut Rect, nfa: &mut f64| {
            let v = self.nfa(&c);
            if v > *nfa {
                *nfa = v;
                *best = c;
            }
        };
        let mut c = best;
        for _ in 0..5 {
            c.p /= 2.0;
            c.prec = c.p * PI;
            try_set(c, &mut best, &mut nfa);
        }
        let mut c = best;
        for _ in 0..5 {
            if c.width - 0.5 < 1.0 {
                break;
            }
            c.width -= 0.5;
            try_set(c, &mut best, &mut nfa);
        }
        for side in [1.0, -1.0] {
            let mut c = best;
            for _ in 0..5 {
                if c.width - 0.5 < 1.0 {
                    break;
                }
                c.width -= 0.5;
                // Move the centre line a quarter pixel towards the other side.
                let (nx, ny) = (-c.dy * 0.25 * side, c.dx * 0.25 * side);
                c.x1 += nx;
                c.y1 += ny;
                c.x2 += nx;
                c.y2 += ny;
                c.cx += nx;
                c.cy += ny;
                try_set(c, &mut best, &mut nfa);
            }
        }
        let mut c = best;
        for _ in 0..5 {
            c.p /= 2.0;
            c.prec = c.p * PI;
            try_set(c, &mut best, &mut nfa);
        }
        (nfa, best)
    }

    fn density(region: &[(usize, usize)], r: &Rect) -> f64 {
        let len = (r.x2 - r.x1).hypot(r.y2 - r.y1);
        region.len() as f64 / (len * r.width).max(1e-12)
    }

    /// Makes the region dense enough (see the module documentation); false drops it.
    fn refine(
        &mut self,
        region: &mut Vec<(usize, usize)>,
        reg_angle: &mut f64,
        rect: &mut Rect,
        sx: usize,
        sy: usize,
        min_reg: usize,
    ) -> bool {
        if Self::density(region, rect) >= DENSITY {
            return true;
        }
        // A tolerance from the angles near the seed.
        let seed = self.angle[sy * self.w + sx];
        let (xc, yc) = (sx as f64, sy as f64);
        let (mut s, mut s2, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for &(x, y) in region.iter() {
            if (x as f64 - xc).hypot(y as f64 - yc) < rect.width {
                let d = angle_diff(self.angle[y * self.w + x], seed);
                s += d;
                s2 += d * d;
                n += 1.0;
            }
        }
        let mean = s / n.max(1.0);
        let tau = 2.0 * (s2 / n.max(1.0) - mean * mean).max(0.0).sqrt();
        for &(x, y) in region.iter() {
            self.used[y * self.w + x] = false;
        }
        *reg_angle = self.grow(sx, sy, tau.max(1e-3), region);
        if region.len() < 2 {
            return false;
        }
        *rect = self.rect(region, *reg_angle, rect.prec, rect.p);
        if Self::density(region, rect) >= DENSITY {
            return true;
        }
        // Then shrink the region about the seed.
        let mut rad = (rect.x2 - rect.x1)
            .hypot(rect.y2 - rect.y1)
            .max((rect.x1 - xc).hypot(rect.y1 - yc));
        while Self::density(region, rect) < DENSITY {
            rad *= 0.75;
            let w = self.w;
            let used = &mut self.used;
            region.retain(|&(x, y)| {
                let keep = (x as f64 - xc).hypot(y as f64 - yc) <= rad;
                if !keep {
                    used[y * w + x] = false;
                }
                keep
            });
            if region.len() < 2 || region.len() < min_reg.min(2) {
                return false;
            }
            *rect = self.rect(region, *reg_angle, rect.prec, rect.p);
            if rad < 1.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ln_gamma_matches_factorials() {
        let mut f = 1.0f64;
        for n in 1..30 {
            f *= f64::from(n);
            assert!((ln_gamma(f64::from(n) + 1.0) - f.ln()).abs() < 1e-9, "{n}");
        }
    }

    #[test]
    fn nfa_of_a_perfect_line_is_meaningful() {
        // 100 of 100 pixels aligned at p = 1/8.
        assert!(log_nfa(100, 100, 0.125, 15.0) > 50.0);
        // Half aligned is what noise gives at p = 1/2: not meaningful.
        assert!(log_nfa(100, 50, 0.5, 15.0) < 0.0);
        // The tail sums to about the first terms' total.
        let v = log_nfa(1000, 300, 0.125, 0.0);
        assert!(v > 30.0 && v.is_finite(), "{v}");
    }

    #[test]
    fn a_step_edge_is_one_segment() {
        let (w, h) = (200usize, 150usize);
        let mut img = vec![0.2f32; w * h];
        for y in 0..h {
            for x in 0..w {
                // An edge from (30, 20) to (170, 130).
                let s = (x as f32 + 0.5 - 30.0) * 110.0 - (y as f32 + 0.5 - 20.0) * 140.0;
                if s > 0.0 {
                    img[y * w + x] = 0.8;
                }
            }
        }
        let segs = detect(&img, w, h, 255.0);
        let long: Vec<_> = segs.iter().filter(|s| s.len() > 50.0).collect();
        assert!(!long.is_empty(), "{segs:?}");
        let s = long[0];
        let ang = (s.b.1 - s.a.1)
            .atan2(s.b.0 - s.a.0)
            .to_degrees()
            .rem_euclid(180.0);
        let want = 110f64.atan2(140.0).to_degrees();
        assert!((ang - want).abs() < 1.0, "{ang} vs {want}");
    }
}
