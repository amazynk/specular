//! Plot axes for cytometry data and the density plots and histograms drawn through them.
//!
//! An [`Axis`] maps a parameter's scale values onto the unit interval of a plot (0 the
//! low edge, 1 the high edge) and back, and names its ticks:
//!
//! * **Linear** from `min(0, low data)` to the parameter's range.
//! * **Log** over the decades of a log-amplified parameter, else five decades below its
//!   range; values at or below zero sit on the low edge.
//! * **Logicle** (Parks, Roederer and Moore 2006; the parameterisation of Moore and Parks
//!   2012): linear around zero, logarithmic above, with top `T` the range, `M` = 4.5
//!   decades, `A` = 0 and the linear width `W` from the data's negative values
//!   (`(M − log10(T/|r|)) / 2` for `r` the 5th percentile of them), else 0.5.
//! * **Arcsinh** with cofactor 150: `asinh(x / 150)`.
//!
//! Values past an edge are drawn on it, as cytometry software does.

/// How an axis spaces values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scale {
    Linear,
    Log,
    Logicle,
    Arcsinh,
}

impl Scale {
    pub const ALL: [Scale; 4] = [Scale::Linear, Scale::Log, Scale::Logicle, Scale::Arcsinh];

    pub fn name(self) -> &'static str {
        match self {
            Scale::Linear => "Linear",
            Scale::Log => "Log",
            Scale::Logicle => "Logicle",
            Scale::Arcsinh => "Arcsinh",
        }
    }
}

/// The cofactor of the arcsinh scale.
pub const ASINH_COFACTOR: f64 = 150.0;
/// Logicle decades.
pub const LOGICLE_M: f64 = 4.5;

/// The logicle (biexponential) function for top `t`, linear width `w`, decades `m` and
/// extra negative decades `a`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Logicle {
    pub t: f64,
    pub w: f64,
    pub m: f64,
    pub a: f64,
    ca: f64,
    b: f64,
    c: f64,
    d: f64,
    f: f64,
    x1: f64,
}

impl Logicle {
    pub fn new(t: f64, w: f64, m: f64, a: f64) -> Option<Logicle> {
        if !(t > 0.0 && m > 0.0 && w >= 0.0 && a >= 0.0 && 2.0 * w <= m + a) {
            return None;
        }
        let wn = w / (m + a);
        let x2 = a / (m + a);
        let x1 = x2 + wn;
        let x0 = x2 + 2.0 * wn;
        let b = (m + a) * std::f64::consts::LN_10;
        // d solves 2 (ln d − ln b) + wn (b + d) = 0 on (0, b]; increasing in d.
        let d = if wn == 0.0 {
            b
        } else {
            let g = |d: f64| 2.0 * (d.ln() - b.ln()) + wn * (b + d);
            let (mut lo, mut hi) = (f64::MIN_POSITIVE, b);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if g(mid) > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            0.5 * (lo + hi)
        };
        let c_a = (x0 * (b + d)).exp();
        let mf_a = (b * x1).exp() - c_a / (d * x1).exp();
        let ca = t / ((b.exp() - mf_a) - c_a / d.exp());
        Some(Logicle {
            t,
            w,
            m,
            a,
            ca,
            b,
            c: c_a * ca,
            d,
            f: -mf_a * ca,
            x1,
        })
    }

    /// The data value at scale position `y` (1 at `t`).
    pub fn inverse(&self, y: f64) -> f64 {
        let (yy, neg) = if y < self.x1 {
            (2.0 * self.x1 - y, true)
        } else {
            (y, false)
        };
        let v = self.ca * (self.b * yy).exp() - self.c * (-self.d * yy).exp() + self.f;
        if neg { -v } else { v }
    }

    /// The scale position of data value `x`.
    pub fn scale(&self, x: f64) -> f64 {
        if !x.is_finite() {
            return if x > 0.0 { 2.0 } else { -1.0 };
        }
        if x == 0.0 {
            return self.x1;
        }
        // Monotone: bracket, then bisection polished by Newton steps.
        let (mut lo, mut hi) = (self.x1 - 1.0, 1.0f64);
        while self.inverse(hi) < x && hi < 64.0 {
            hi = hi * 2.0 + 1.0;
        }
        while self.inverse(lo) > x && lo > -64.0 {
            lo = lo * 2.0 - 1.0;
        }
        let mut y = 0.5 * (lo + hi);
        for _ in 0..200 {
            let v = self.inverse(y) - x;
            if v.abs() <= 1e-12 * x.abs().max(1e-300) {
                break;
            }
            if v > 0.0 {
                hi = y;
            } else {
                lo = y;
            }
            // Newton from the derivative, kept inside the bracket.
            let yy = if y < self.x1 { 2.0 * self.x1 - y } else { y };
            let dv =
                self.ca * self.b * (self.b * yy).exp() + self.c * self.d * (-self.d * yy).exp();
            let n = y - v / dv;
            y = if n > lo && n < hi { n } else { 0.5 * (lo + hi) };
            if hi - lo < 1e-15 {
                break;
            }
        }
        y
    }
}

/// One axis of a plot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub scale: Scale,
    /// The edges in the scale's own units (values for linear and log, logicle positions,
    /// arcsinh values).
    lo: f64,
    hi: f64,
    logicle: Option<Logicle>,
}

/// The axes fit the data when its largest value is under this share of `$PnR`.
pub const FIT_BELOW: f64 = 0.25;

/// What an axis needs of a parameter's values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// The top of the scale (the range in scale values), 0 when unknown.
    pub top: f64,
    /// Log-amplified decades and offset (`$PnE`), `(0, 0)` linear.
    pub log: (f64, f64),
    pub min: f64,
    pub max: f64,
    /// 0.1st percentile.
    pub low: f64,
    /// 5th percentile of the negative values (0 when none).
    pub neg5: f64,
}

impl Stats {
    /// From a parameter's top of scale, its `$PnE` and a sample of its values.
    pub fn of(top: f64, log: (f64, f64), values: impl Iterator<Item = f32>) -> Stats {
        // A strided sample of at most 200 000 values for the percentiles.
        let all: Vec<f64> = values.map(f64::from).filter(|v| v.is_finite()).collect();
        let step = (all.len() / 200_000).max(1);
        let mut s: Vec<f64> = all.iter().step_by(step).copied().collect();
        s.sort_by(f64::total_cmp);
        let min = all.iter().copied().fold(f64::INFINITY, f64::min);
        let max = all.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let pct = |v: &[f64], p: f64| {
            if v.is_empty() {
                0.0
            } else {
                v[((v.len() - 1) as f64 * p).round() as usize]
            }
        };
        let neg: Vec<f64> = s.iter().copied().filter(|v| *v < 0.0).collect();
        Stats {
            top,
            log,
            min: if min.is_finite() { min } else { 0.0 },
            max: if max.is_finite() { max } else { 0.0 },
            low: pct(&s, 0.001),
            neg5: pct(&neg, 0.05),
        }
    }

    /// The top used: the stated one, else the data's maximum, else 1.
    pub fn top(&self) -> f64 {
        if self.top > 0.0 && self.top.is_finite() {
            // Events using a small part of the declared range (a Fortessa's or Miltenyi's
            // 5 % of `$PnR`) left the plot nearly empty: the axis fits the data then, with a
            // little room above the largest value.
            if self.max > 0.0 && self.max.is_finite() && self.max < FIT_BELOW * self.top {
                return self.max * 1.05;
            }
            self.top
        } else if self.max > 0.0 {
            self.max
        } else {
            1.0
        }
    }
}

/// The logicle linear width for data with negative values down to `neg5` under top `t`.
pub fn logicle_width(t: f64, neg5: f64) -> f64 {
    if neg5 >= 0.0 || t <= 0.0 {
        return 0.5;
    }
    let w = (LOGICLE_M - (t / neg5.abs()).log10()) / 2.0;
    w.clamp(0.0, LOGICLE_M / 2.0)
}

impl Axis {
    pub fn new(scale: Scale, st: &Stats) -> Axis {
        let top = st.top();
        let (lo, hi, logicle) = match scale {
            Scale::Linear => (st.low.min(0.0), top, None),
            Scale::Log => {
                if st.log.0 > 0.0 {
                    let f2 = if st.log.1 > 0.0 { st.log.1 } else { 1.0 };
                    (f2.log10(), (f2 * 10f64.powf(st.log.0)).log10(), None)
                } else {
                    let h = top.log10();
                    (h - 5.0, h, None)
                }
            }
            Scale::Logicle => {
                let w = logicle_width(top, st.neg5);
                let l = Logicle::new(top, w, LOGICLE_M, 0.0)
                    .or_else(|| Logicle::new(top, 0.5, LOGICLE_M, 0.0));
                (0.0, 1.0, l)
            }
            Scale::Arcsinh => (
                (st.low.min(0.0) / ASINH_COFACTOR).asinh(),
                (top / ASINH_COFACTOR).asinh(),
                None,
            ),
        };
        let hi = if hi > lo { hi } else { lo + 1.0 };
        Axis {
            scale,
            lo,
            hi,
            logicle,
        }
    }

    /// The scale's own units of a value.
    fn own(&self, x: f64) -> f64 {
        match self.scale {
            Scale::Linear => x,
            Scale::Log => {
                if x > 0.0 {
                    x.log10()
                } else {
                    f64::NEG_INFINITY
                }
            }
            Scale::Logicle => self.logicle.map_or(0.0, |l| l.scale(x)),
            Scale::Arcsinh => (x / ASINH_COFACTOR).asinh(),
        }
    }

    fn value_of_own(&self, v: f64) -> f64 {
        match self.scale {
            Scale::Linear => v,
            Scale::Log => 10f64.powf(v),
            Scale::Logicle => self.logicle.map_or(0.0, |l| l.inverse(v)),
            Scale::Arcsinh => v.sinh() * ASINH_COFACTOR,
        }
    }

    /// The plot position (0 low edge, 1 high edge) of a value; may lie outside.
    pub fn unit(&self, x: f64) -> f64 {
        (self.own(x) - self.lo) / (self.hi - self.lo)
    }

    /// The value at plot position `u`.
    pub fn value(&self, u: f64) -> f64 {
        self.value_of_own(self.lo + u * (self.hi - self.lo))
    }

    /// The logicle parameters, for a logicle axis.
    pub fn logicle(&self) -> Option<&Logicle> {
        self.logicle.as_ref()
    }

    /// Ticks: plot position and label, low to high.
    pub fn ticks(&self) -> Vec<(f64, String)> {
        let mut out = Vec::new();
        match self.scale {
            Scale::Linear => {
                let span = self.hi - self.lo;
                let raw = span / 5.0;
                let mag = 10f64.powf(raw.log10().floor());
                let step = [1.0, 2.0, 2.5, 5.0, 10.0]
                    .iter()
                    .map(|m| m * mag)
                    .find(|s| *s >= raw)
                    .unwrap_or(10.0 * mag);
                let mut v = (self.lo / step).ceil() * step;
                let mut n = 0;
                while v <= self.hi + step * 1e-9 && n < 64 {
                    out.push((self.unit(v), short(v)));
                    v += step;
                    n += 1;
                }
            }
            _ => {
                // Candidates by priority: zero, the positive decades, the negative ones
                // (logicle, arcsinh); one is kept when clear of those kept before.
                let mut cand: Vec<(f64, String)> = Vec::new();
                if self.scale != Scale::Log {
                    cand.push((0.0, "0".into()));
                }
                let k0 = if self.scale == Scale::Log {
                    self.lo.floor() as i32
                } else {
                    1
                };
                for k in k0..=k0.max(1) + 12 {
                    cand.push((10f64.powi(k), decade(k)));
                }
                if self.scale != Scale::Log {
                    for k in 1..=9 {
                        cand.push((-(10f64.powi(k)), format!("−{}", decade(k))));
                    }
                }
                for (x, label) in cand {
                    let u = self.unit(x);
                    if (-1e-9..=1.0 + 1e-9).contains(&u)
                        && out.iter().all(|(v, _)| (u - v).abs() >= 0.06)
                    {
                        out.push((u.clamp(0.0, 1.0), label));
                    }
                }
                out.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
        }
        out
    }
}

/// 10 to the `k` as `10³`.
fn decade(k: i32) -> String {
    const SUP: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let mut s = String::from("10");
    if k < 0 {
        s.push('⁻');
    }
    for c in k.unsigned_abs().to_string().chars() {
        s.push(SUP[c.to_digit(10).unwrap_or(0) as usize]);
    }
    s
}

/// A linear tick label: `250`, `50K`, `1.5M`.
pub fn short(v: f64) -> String {
    let a = v.abs();
    let (x, suffix) = if a >= 1e6 {
        (v / 1e6, "M")
    } else if a >= 1e3 {
        (v / 1e3, "K")
    } else {
        (v, "")
    };
    let s = format!("{x:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    let s = if s == "-0" { "0" } else { s };
    format!("{}{suffix}", s.replace('-', "−"))
}

/// The bin of plot position `u` among `n` (edges clamp into the end bins).
#[inline]
fn bin(u: f64, n: usize) -> usize {
    if u.is_nan() {
        return 0;
    }
    ((u * n as f64).floor().max(0.0) as usize).min(n - 1)
}

/// A density plot: `n`×`n` event counts of parameters `px` (columns, left to right) and `py`
/// (rows, the highest values at the top).
pub fn density(
    ev: &crate::Events,
    px: usize,
    xa: &Axis,
    py: usize,
    ya: &Axis,
    n: usize,
) -> Vec<f32> {
    let mut out = vec![0f32; n * n];
    if px >= ev.params || py >= ev.params || n == 0 {
        return out;
    }
    for row in ev.values.chunks_exact(ev.params).take(ev.count) {
        let bx = bin(xa.unit(f64::from(row[px])), n);
        let by = bin(ya.unit(f64::from(row[py])), n);
        out[(n - 1 - by) * n + bx] += 1.0;
    }
    out
}

/// A histogram of parameter `p` in `n` bins of the axis.
pub fn histogram(ev: &crate::Events, p: usize, a: &Axis, n: usize) -> Vec<f64> {
    let mut out = vec![0f64; n];
    if p >= ev.params || n == 0 {
        return out;
    }
    for v in ev.column(p).take(ev.count) {
        out[bin(a.unit(f64::from(v)), n)] += 1.0;
    }
    out
}

/// A histogram drawn as an `n`×`n` image: each column's bar (its count as the value) up to
/// its height, the tallest reaching 90 % of the rows; zero above.
pub fn histogram_image(h: &[f64], n: usize) -> Vec<f32> {
    let mut out = vec![0f32; n * n];
    let peak = h.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 || h.len() != n {
        return out;
    }
    for (x, &c) in h.iter().enumerate() {
        let rows = ((c / peak) * 0.9 * n as f64).round() as usize;
        let rows = if c > 0.0 { rows.max(1) } else { 0 };
        for r in 0..rows.min(n) {
            out[(n - 1 - r) * n + x] = c as f32;
        }
    }
    out
}
