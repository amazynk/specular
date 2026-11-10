//! The filmic-style and AgX-style tone curves.
//!
//! Both work on a **log encoding** of the scene, `u = log2(x / grey)` (EV from mid grey),
//! inside a scene range `lo..hi` (moved by skew; the top also rises with the display
//! white, so an HDR rendition shows more of the highlights). Both put scene mid grey on
//! display mid grey whatever the settings.
//!
//! - **Filmic-style** (log + spline with a latitude): a straight line of slope `c` in
//!   log–log over the latitude (`y = grey · 2^(c·u)`, a pure contrast around grey), then
//!   a power toe down to the display black at `lo` and a power shoulder up to the display
//!   white at `hi`, each meeting the line with the same slope:
//!   `y = black + (y_t − black) · ((u − lo) / (u_t − lo))^k0` and
//!   `y = white − (white − y_s) · ((hi − u) / (hi − u_s))^k1`. The toe and the latitude do
//!   not depend on the white, so an HDR rendition keeps the shadows and midtones of the
//!   SDR one and only the shoulder moves. It reaches white at `hi` (a hard end, as
//!   film-like curves do).
//! - **AgX-style** (after Sobotka's public description: inset → log2 → sigmoid → outset):
//!   a display value `d` in a power-1/2.4 domain, `y = black + (1 − black) · d^2.4`
//!   (`d = 1` is SDR white); on each side of the grey point a rational sigmoid in EV,
//!   `f(u) = s·u / (1 + (s·u / L)^p)^(1/p)`, with `L` solved so the toe ends exactly at
//!   black at `lo` and the shoulder exactly at the display white at `hi`. The toe does not
//!   depend on the white, so HDR only stretches the shoulder. It runs per channel on inset
//!   primaries, so bright saturated colours move smoothly towards white.
//!
//! Every piece is a power, exponential or rational function with a positive derivative, so
//! the curves are monotone; the parameters are clamped so they stay so.

use super::GREY;

/// The AgX-style display power.
pub const POWER: f64 = 2.4;

/// Filmic-style scene range in EV around grey (moved by skew; the top also by the log2 of
/// the display white).
pub const FILMIC_RANGE: (f64, f64) = (-9.0, 5.0);
/// AgX-style scene range in EV around grey (as the filmic one).
pub const AGX_RANGE: (f64, f64) = (-10.0, 6.5);
/// AgX-style toe and shoulder powers.
const AGX_POWER: (f64, f64) = (1.5, 1.5);
/// Contrast 1.5 (the default) gives these slopes: log–log for filmic, `d` per `t` for AgX.
const FILMIC_SLOPE: f64 = 0.8;
const AGX_SLOPE: f64 = 1.6;
/// Filmic-style: the toe starts at a quarter of grey, the shoulder this far from grey to
/// white.
const FILMIC_TOE: f64 = 0.25;
const FILMIC_SHOULDER: f64 = 0.45;

/// The scene range for a method's base range, skew and display white.
fn range(base: (f64, f64), skew: f64, white: f64) -> (f64, f64) {
    (base.0 + skew, base.1 + skew + white.max(1.0).log2())
}

/// Constants of the filmic-style curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filmic {
    pub lo: f64,
    pub hi: f64,
    /// Log–log slope of the latitude.
    pub c: f64,
    pub u_t: f64,
    pub y_t: f64,
    pub k0: f64,
    pub u_s: f64,
    pub y_s: f64,
    pub k1: f64,
    pub white: f64,
    pub black: f64,
}

impl Filmic {
    /// `contrast` and `skew` as the sigmoid's; `white`/`black` are display targets.
    pub fn new(contrast: f64, skew: f64, white: f64, black: f64) -> Filmic {
        let white = white.max(2.0 * GREY);
        let (lo, hi) = range(FILMIC_RANGE, skew, white);
        // Steep enough to reach white within the range.
        let c = (contrast * FILMIC_SLOPE).max((white / GREY).log2() / (hi - 0.5) * 1.05);
        let u_t = ((FILMIC_TOE.log2()) / c).max(lo + 1.0);
        let y_t = GREY * (c * u_t).exp2();
        let black = black.min(0.5 * y_t);
        let k0 = y_t * std::f64::consts::LN_2 * c * (u_t - lo) / (y_t - black);
        let y_s = GREY + (white - GREY) * FILMIC_SHOULDER;
        let u_s = ((y_s / GREY).log2() / c).min(hi - 0.5);
        let y_s = GREY * (c * u_s).exp2();
        let k1 = y_s * std::f64::consts::LN_2 * c * (hi - u_s) / (white - y_s).max(1e-9);
        Filmic {
            lo,
            hi,
            c,
            u_t,
            y_t,
            k0,
            u_s,
            y_s,
            k1,
            white,
            black,
        }
    }

    pub fn eval(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return self.black;
        }
        let u = (x / GREY).log2();
        if u <= self.lo {
            self.black
        } else if u >= self.hi {
            self.white
        } else if u < self.u_t {
            self.black
                + (self.y_t - self.black) * ((u - self.lo) / (self.u_t - self.lo)).powf(self.k0)
        } else if u > self.u_s {
            self.white
                - (self.white - self.y_s) * ((self.hi - u) / (self.hi - self.u_s)).powf(self.k1)
        } else {
            GREY * (self.c * u).exp2()
        }
    }

    /// The 12 curve uniforms.
    pub fn uniforms(&self) -> [f32; 12] {
        [
            self.lo, self.hi, self.c, self.u_t, self.y_t, self.k0, self.u_s, self.y_s, self.k1,
            self.white, self.black, 0.0,
        ]
        .map(|v| v as f32)
    }
}

/// Constants of the AgX-style curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Agx {
    pub lo: f64,
    pub hi: f64,
    /// Slope at grey, in `d` per EV.
    pub slope: f64,
    pub d_g: f64,
    pub d_max: f64,
    /// `L` of the toe and of the shoulder.
    pub l_toe: f64,
    pub l_sh: f64,
    pub white: f64,
    pub black: f64,
}

/// `L` so that `s·X / (1 + (s·X / L)^p)^(1/p) = Y`.
fn limit(s: f64, x: f64, y: f64, p: f64) -> f64 {
    let a = s * x;
    a / ((a / y).powf(p) - 1.0).max(1e-9).powf(1.0 / p)
}

fn rational(s: f64, x: f64, l: f64, p: f64) -> f64 {
    let a = s * x;
    if a <= 0.0 {
        return 0.0;
    }
    a / (1.0 + (a / l).powf(p)).powf(1.0 / p)
}

impl Agx {
    pub fn new(contrast: f64, skew: f64, white: f64, black: f64) -> Agx {
        let black = black.clamp(-0.5, 0.05);
        let white = white.max(GREY * 2.0);
        let enc = |y: f64| ((y - black) / (1.0 - black)).max(0.0).powf(1.0 / POWER);
        let d_g = enc(GREY);
        let d_max = enc(white).max(d_g * 1.01);
        let lo = (AGX_RANGE.0 + skew).min(-1.0);
        let span = AGX_RANGE.1 - AGX_RANGE.0;
        let slope = (contrast * AGX_SLOPE / span).max(1.05 * d_g / -lo);
        let hi = (AGX_RANGE.1 + skew + white.max(1.0).log2()).max(1.1 * (d_max - d_g) / slope);
        Agx {
            lo,
            hi,
            slope,
            d_g,
            d_max,
            l_toe: limit(slope, -lo, d_g, AGX_POWER.0),
            l_sh: limit(slope, hi, d_max - d_g, AGX_POWER.1),
            white,
            black,
        }
    }

    /// The display-domain value `u` EV from grey.
    pub fn d(&self, u: f64) -> f64 {
        let d = if u <= self.lo {
            0.0
        } else if u >= self.hi {
            self.d_max
        } else if u >= 0.0 {
            self.d_g + rational(self.slope, u, self.l_sh, AGX_POWER.1)
        } else {
            self.d_g - rational(self.slope, -u, self.l_toe, AGX_POWER.0)
        };
        d.clamp(0.0, self.d_max)
    }

    pub fn eval(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return self.black;
        }
        let d = self.d((x / GREY).log2());
        self.black + (1.0 - self.black) * d.powf(POWER)
    }

    pub fn uniforms(&self) -> [f32; 12] {
        [
            self.lo,
            self.hi,
            self.slope,
            self.d_g,
            self.d_max,
            self.l_toe,
            self.l_sh,
            AGX_POWER.0,
            AGX_POWER.1,
            self.white,
            self.black,
            POWER,
        ]
        .map(|v| v as f32)
    }
}

/// The AgX-style primaries: each primary pulled towards white by its own amount and turned
/// a little towards a neighbour (inset), and the plain luminance pull undone after the
/// curve (outset, without the turn). Rows sum to 1, so white stays white.
pub fn agx_matrices(inset: f64, outset: f64) -> (crate::colour::M3, crate::colour::M3) {
    let a = (inset + 0.1).clamp(0.0, 0.6);
    let pull = [a, a * 0.85, a * 1.15];
    // Red towards yellow, green towards cyan, blue towards magenta (columns: where each
    // primary goes).
    let turn = [(1usize, 0.08 * a), (2, 0.05 * a), (0, 0.10 * a)];
    let w = super::super::tone::LUMA.map(f64::from);
    let mut m: crate::colour::M3 = std::array::from_fn(|i| {
        std::array::from_fn(|j| (1.0 - pull[j]) * f64::from(u8::from(i == j)) + pull[j] * w[j])
    });
    for (j, &(k, r)) in turn.iter().enumerate() {
        m[k][j] += r;
    }
    for row in &mut m {
        let s: f64 = row.iter().sum();
        row.iter_mut().for_each(|v| *v /= s);
    }
    let (_, out) = super::inset_outset(0.0, outset);
    (m, out)
}
