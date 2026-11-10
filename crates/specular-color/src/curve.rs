//! One-dimensional tone curves (ICC `curv` / `para`) and their inverses.

/// A tone curve on [0, 1].
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    /// `y = x`, not clamped (keeps extended-range values intact).
    Identity,
    /// `y = x^g`.
    Gamma(f32),
    /// Uniformly sampled, linearly interpolated (at least two samples).
    Table(Vec<f32>),
    /// ICC parametric curve: function type 0–4 and up to seven parameters `g a b c d e f`.
    Param(u16, [f32; 7]),
    /// Analytic inverse of a [`Curve::Param`].
    InvParam(u16, [f32; 7]),
    /// Exact inverse of a monotonic [`Curve::Table`] (stores the forward table).
    InvTable(Vec<f32>),
}

/// Clamps to [0, 1], mapping NaN to 0.
#[inline]
pub(crate) fn unit(x: f32) -> f32 {
    if x >= 0.0 {
        if x > 1.0 { 1.0 } else { x }
    } else {
        0.0
    }
}

#[inline]
fn pow(x: f32, g: f32) -> f32 {
    if x > 0.0 { x.powf(g) } else { 0.0 }
}

impl Curve {
    /// The sRGB transfer function (encoded → linear) as an ICC type-3 parametric curve.
    pub fn srgb() -> Curve {
        Curve::Param(
            3,
            [
                2.4,
                1.0 / 1.055,
                0.055 / 1.055,
                1.0 / 12.92,
                0.04045,
                0.0,
                0.0,
            ],
        )
    }

    /// Evaluates the curve.
    #[inline]
    pub fn eval(&self, x: f32) -> f32 {
        match self {
            Curve::Identity => x,
            Curve::Gamma(g) => pow(unit(x), *g),
            Curve::Table(t) => {
                if t.len() < 2 {
                    return t.first().copied().unwrap_or(x);
                }
                let n = t.len() - 1;
                let p = unit(x) * n as f32;
                let i = (p as usize).min(n - 1);
                let f = p - i as f32;
                t[i] + (t[i + 1] - t[i]) * f
            }
            Curve::Param(k, p) => param(*k, p, unit(x)),
            Curve::InvParam(k, p) => inv_param(*k, p, x),
            Curve::InvTable(t) => inv_table(t, x),
        }
    }

    /// True when the curve is `y = x` (within 1e-4 everywhere).
    pub fn is_identity(&self) -> bool {
        match self {
            Curve::Identity => true,
            Curve::Gamma(g) => (g - 1.0).abs() < 1e-4,
            _ => (0..=64).all(|i| {
                let x = i as f32 / 64.0;
                (self.eval(x) - x).abs() < 1e-4
            }),
        }
    }

    /// Largest difference from another curve over 1024 samples.
    pub fn max_diff(&self, o: &Curve) -> f32 {
        (0..=1024)
            .map(|i| {
                let x = i as f32 / 1024.0;
                (self.eval(x) - o.eval(x)).abs()
            })
            .fold(0.0, f32::max)
    }

    /// The inverse curve. Gamma and parametric curves invert analytically; tables (assumed
    /// monotonic, either direction) are inverted numerically into a 4096-entry table.
    pub fn inverse(&self) -> Curve {
        match self {
            Curve::Identity => Curve::Identity,
            Curve::Gamma(g) if *g > 1e-4 => Curve::Gamma(1.0 / g),
            Curve::Param(k, p) if param_invertible(*k, p) => Curve::InvParam(*k, *p),
            Curve::InvParam(k, p) => Curve::Param(*k, *p),
            Curve::Table(t) => Curve::InvTable(t.clone()),
            Curve::InvTable(t) => Curve::Table(t.clone()),
            _ => Curve::InvTable(self.sample(4096)),
        }
    }

    /// Samples the curve at `n` evenly spaced points of [0, 1].
    pub fn sample(&self, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| self.eval(i as f32 / (n - 1) as f32))
            .collect()
    }
}

/// Inverse of a piecewise-linear monotonic table (either direction); flat runs resolve to
/// their first point, values beyond the range clamp to the ends.
fn inv_table(f: &[f32], y: f32) -> f32 {
    let n = f.len();
    if n < 2 {
        return unit(y);
    }
    let ascending = f[n - 1] >= f[0];
    let (lo, hi) = if ascending {
        (f[0], f[n - 1])
    } else {
        (f[n - 1], f[0])
    };
    let y = if y >= lo {
        if y > hi { hi } else { y }
    } else {
        lo
    };
    let idx = if ascending {
        f.partition_point(|&v| v < y)
    } else {
        f.partition_point(|&v| v > y)
    };
    if idx == 0 {
        return 0.0;
    }
    if idx >= n {
        return 1.0;
    }
    let (a, b) = (f[idx - 1], f[idx]);
    let t = if (b - a).abs() > 1e-12 {
        (y - a) / (b - a)
    } else {
        0.0
    };
    ((idx - 1) as f32 + t.clamp(0.0, 1.0)) / (n - 1) as f32
}

#[inline]
fn param(k: u16, p: &[f32; 7], x: f32) -> f32 {
    let [g, a, b, c, d, e, f] = *p;
    match k {
        0 => pow(x, g),
        1 => {
            if a != 0.0 && x >= -b / a {
                pow(a * x + b, g)
            } else {
                0.0
            }
        }
        2 => {
            if a != 0.0 && x >= -b / a {
                pow(a * x + b, g) + c
            } else {
                c
            }
        }
        3 => {
            if x >= d {
                pow(a * x + b, g)
            } else {
                c * x
            }
        }
        4 => {
            if x >= d {
                pow(a * x + b, g) + e
            } else {
                c * x + f
            }
        }
        _ => x,
    }
}

fn param_invertible(k: u16, p: &[f32; 7]) -> bool {
    let [g, a, _, c, ..] = *p;
    let ok = g.is_finite() && g.abs() > 1e-4 && a.is_finite() && a.abs() > 1e-6;
    match k {
        0 => g.is_finite() && g.abs() > 1e-4,
        1 | 2 => ok,
        3 | 4 => ok && c.is_finite() && c > 1e-6,
        _ => false,
    }
}

#[inline]
fn inv_param(k: u16, p: &[f32; 7], y: f32) -> f32 {
    let [g, a, b, c, d, e, f] = *p;
    let r = match k {
        0 => pow(y, 1.0 / g),
        1 => (pow(y, 1.0 / g) - b) / a,
        2 => (pow(y - c, 1.0 / g) - b) / a,
        3 => {
            if y >= c * d {
                (pow(y, 1.0 / g) - b) / a
            } else {
                y / c
            }
        }
        4 => {
            if y >= c * d + f {
                (pow(y - e, 1.0 / g) - b) / a
            } else {
                (y - f) / c
            }
        }
        _ => y,
    };
    unit(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_inverse_round_trips() {
        let c = Curve::srgb();
        let inv = c.inverse();
        for i in 0..=255 {
            let x = i as f32 / 255.0;
            assert!((inv.eval(c.eval(x)) - x).abs() < 1e-4, "{x}");
        }
    }

    #[test]
    fn table_inverse() {
        let t = Curve::Table((0..256).map(|i| (i as f32 / 255.0).powf(2.2)).collect());
        let inv = t.inverse();
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            assert!((inv.eval(t.eval(x)) - x).abs() < 2e-3, "{x}");
        }
        let desc = Curve::Table(vec![1.0, 0.5, 0.0]);
        assert!((desc.inverse().eval(0.25) - 0.75).abs() < 1e-3);
    }

    #[test]
    fn nan_and_out_of_range_are_safe() {
        for c in [
            Curve::srgb(),
            Curve::Gamma(2.2),
            Curve::Table(vec![0.0, 1.0]),
        ] {
            assert!(c.eval(f32::NAN).is_finite());
            assert!(c.eval(-5.0).is_finite());
            assert!(c.eval(5.0).is_finite());
            assert!(c.inverse().eval(f32::NAN).is_finite());
        }
    }
}
