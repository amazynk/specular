//! Display stretches for scientific (N-D, real-valued) data: the limits a plane is shown
//! between and the function mapping values inside them to grey levels.
//!
//! The functions follow astropy.visualization, so a FITS user sees what they know:
//!
//! * Limits ([`Limits`]): `MinMax` over finite values, `Percentile` (symmetric, linear
//!   interpolation as `np.percentile`), and `ZScale` (IRAF's zscale as astropy's
//!   `ZScaleInterval`: 1000 stride samples, an iterative k-σ clipped line fit to the sorted
//!   samples, contrast 0.25).
//! * Stretches ([`Stretch`]) of the normalised value `x ∈ [0, 1]`: linear, `sqrt`,
//!   `log(a x + 1) / log(a + 1)` with a = 1000, `asinh(x / a) / asinh(1 / a)` with a = 0.1,
//!   and histogram equalisation through a CDF lookup table ([`histeq_lut`]).
//! * The automatic stretch ([`Stretch::Auto`], [`stf`]): PixInsight's screen transfer
//!   function, per channel (unlinked): shadows clipped at the median less 2.8 normalised
//!   MADs, highlights at the maximum, and the midtones transfer function ([`mtf`]) that puts
//!   the median at 0.25.
//! * Colour filter arrays ([`Cfa`]): the 2×2 Bayer pattern of a one-shot colour camera's raw
//!   frame and its bilinear demosaic ([`Cfa::debayer_at`], the reference of the GPU pass).
//!
//! The GPU applies the same stretch per pixel (the renderer's science mode); [`apply`] is the
//! CPU reference it is tested against and what thumbnails use.

/// The function applied to normalised values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Stretch {
    #[default]
    Linear,
    Sqrt,
    Log,
    Asinh,
    /// Histogram equalisation: the CDF of the plane's values between the limits.
    HistEq,
    /// The screen transfer function ([`stf`]): limits and a midtones balance per channel,
    /// in place of the [`Limits`].
    Auto,
}

impl Stretch {
    pub const ALL: [Stretch; 6] = [
        Stretch::Linear,
        Stretch::Sqrt,
        Stretch::Log,
        Stretch::Asinh,
        Stretch::HistEq,
        Stretch::Auto,
    ];

    pub fn next(self) -> Stretch {
        let i = Stretch::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Stretch::ALL[(i + 1) % Stretch::ALL.len()]
    }

    /// Stable code (settings, shader constant, string ids).
    pub fn code(self) -> &'static str {
        match self {
            Stretch::Linear => "linear",
            Stretch::Sqrt => "sqrt",
            Stretch::Log => "log",
            Stretch::Asinh => "asinh",
            Stretch::HistEq => "histeq",
            Stretch::Auto => "auto",
        }
    }

    pub fn from_code(s: &str) -> Option<Stretch> {
        Stretch::ALL.into_iter().find(|v| v.code() == s)
    }

    /// The shader's function number.
    pub fn index(self) -> u32 {
        Stretch::ALL.iter().position(|s| *s == self).unwrap_or(0) as u32
    }
}

/// `a` of the log stretch.
pub const LOG_A: f32 = 1000.0;
/// `a` of the asinh stretch.
pub const ASINH_A: f32 = 0.1;
/// Bins of the histogram equalisation table.
pub const HISTEQ_BINS: usize = 4096;

/// How the display limits are found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Limits {
    MinMax,
    /// Keep the central `p` per cent (e.g. 99.5: 0.25 % clipped at each end).
    Percentile(f32),
    #[default]
    ZScale,
    /// Symmetric about 0: ±the `p`th percentile of the magnitudes (seismic amplitudes; a
    /// lower `p` clips more, a higher gain).
    Symmetric(f32),
}

impl Limits {
    pub const ALL: [Limits; 5] = [
        Limits::ZScale,
        Limits::MinMax,
        Limits::Percentile(99.5),
        Limits::Percentile(99.0),
        Limits::Percentile(95.0),
    ];

    /// The gains of seismic sections, least clipped first.
    pub const SYMMETRIC: [Limits; 6] = [
        Limits::Symmetric(100.0),
        Limits::Symmetric(99.5),
        Limits::Symmetric(99.0),
        Limits::Symmetric(98.0),
        Limits::Symmetric(95.0),
        Limits::Symmetric(90.0),
    ];

    /// The default gain of seismic sections.
    pub const SEISMIC: Limits = Limits::Symmetric(99.0);

    pub fn next(self) -> Limits {
        let i = Limits::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Limits::ALL[(i + 1) % Limits::ALL.len()]
    }

    /// The next seismic gain (more clipping), back to the least after the most.
    pub fn next_symmetric(self) -> Limits {
        match Limits::SYMMETRIC.iter().position(|s| *s == self) {
            Some(i) => Limits::SYMMETRIC[(i + 1) % Limits::SYMMETRIC.len()],
            None => Limits::SEISMIC,
        }
    }

    pub fn code(self) -> String {
        match self {
            Limits::MinMax => "minmax".into(),
            Limits::ZScale => "zscale".into(),
            Limits::Percentile(p) => format!("p{p}"),
            Limits::Symmetric(p) => format!("s{p}"),
        }
    }

    pub fn from_code(s: &str) -> Option<Limits> {
        match s {
            "minmax" => Some(Limits::MinMax),
            "zscale" => Some(Limits::ZScale),
            _ if s.starts_with('s') => s[1..]
                .parse()
                .ok()
                .filter(|p: &f32| *p > 0.0 && *p <= 100.0)
                .map(Limits::Symmetric),
            _ => s
                .strip_prefix('p')?
                .parse()
                .ok()
                .filter(|p: &f32| *p > 0.0 && *p <= 100.0)
                .map(Limits::Percentile),
        }
    }

    /// Display limits of a plane (`width` values per row, rows top-down). `None` when it has
    /// no finite value. `lo == hi` for a constant plane. Limits that collapse to one value on
    /// a plane holding others widen, so the plane does not show as one flat grey: zscale to
    /// the 0.5th–99.5th percentiles (a planetary frame whose zscale sample is mostly a fill
    /// value), then either to the minimum and maximum (a sparse counts image: an X-ray event
    /// list binned, nearly every bin 0).
    pub fn compute(self, values: &[f32], width: usize) -> Option<(f32, f32)> {
        let collapsed = |r: Option<(f32, f32)>| matches!(r, Some((lo, hi)) if hi <= lo);
        let mut r = match self {
            Limits::MinMax => return minmax(values),
            Limits::Percentile(p) => {
                let c = (100.0 - f64::from(p)) / 2.0;
                percentiles(values, c, 100.0 - c)
            }
            Limits::ZScale => zscale(values, width),
            Limits::Symmetric(p) => return symmetric(values, p),
        };
        if collapsed(r) && self == Limits::ZScale {
            r = percentiles(values, 0.5, 99.5).or(r);
        }
        if collapsed(r) {
            r = minmax(values).or(r);
        }
        r
    }
}

/// ±the `p`th percentile of the finite values' magnitudes. Data with no negative value
/// (unsigned or offset samples: SEG-Y format 16, a DC offset) is centred on its median
/// instead of 0; about 0 it showed as one colour, all red on the diverging palette (the
/// visual review).
pub fn symmetric(values: &[f32], p: f32) -> Option<(f32, f32)> {
    let finite = || values.iter().copied().filter(|v| v.is_finite());
    let centre = if finite().all(|v| v >= 0.0) && finite().any(|v| v > 0.0) {
        let all: Vec<f32> = finite().collect();
        percentiles(&all, 50.0, 50.0)?.0
    } else {
        0.0
    };
    let mags: Vec<f32> = finite().map(|v| (v - centre).abs()).collect();
    let (_, a) = percentiles(&mags, 0.0, f64::from(p))?;
    Some((centre - a, centre + a))
}

/// Smallest and largest finite values.
pub fn minmax(values: &[f32]) -> Option<(f32, f32)> {
    let (lo, hi) = values
        .iter()
        .filter(|v| v.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    (lo <= hi).then_some((lo, hi))
}

/// Above this many finite values, percentiles are taken from an even stride sample.
const PERCENTILE_EXACT: usize = 1 << 22;

/// The `lo` and `hi` percentiles (0–100) of the finite values, interpolated linearly as
/// `np.percentile`. Exact up to 4 Mi values, from a 4 Mi stride sample above.
pub fn percentiles(values: &[f32], lo: f64, hi: f64) -> Option<(f32, f32)> {
    let finite = values.iter().filter(|v| v.is_finite()).count();
    if finite == 0 {
        return None;
    }
    let stride = finite.div_ceil(PERCENTILE_EXACT).max(1);
    let mut s: Vec<f32> = values
        .iter()
        .filter(|v| v.is_finite())
        .step_by(stride)
        .copied()
        .collect();
    let n = s.len();
    let mut at = |p: f64| -> f32 {
        let pos = (p.clamp(0.0, 100.0) / 100.0) * (n - 1) as f64;
        let i = pos.floor() as usize;
        let f = pos - i as f64;
        let (_, a, rest) = s.select_nth_unstable_by(i, f32::total_cmp);
        let a = *a;
        if f == 0.0 || i + 1 >= n {
            return a;
        }
        let b = rest.iter().copied().fold(f32::INFINITY, f32::min);
        (f64::from(a) + (f64::from(b) - f64::from(a)) * f) as f32
    };
    let l = at(lo);
    let h = at(hi);
    Some((l, h))
}

/// IRAF zscale, as astropy's `ZScaleInterval` with its defaults. Rows are sampled bottom-up
/// (the FITS storage order of a top-down plane), so the result equals astropy's on the same
/// FITS image.
pub fn zscale(values: &[f32], width: usize) -> Option<(f32, f32)> {
    const N_SAMPLES: usize = 1000;
    const CONTRAST: f64 = 0.25;
    const MAX_REJECT: f64 = 0.5;
    const MIN_NPIXELS: usize = 5;
    const KREJ: f64 = 2.5;
    const MAX_ITERATIONS: usize = 5;
    let width = width.max(1);
    let rows = || values.chunks(width).rev().flatten().copied();
    let finite = values.iter().filter(|v| v.is_finite()).count();
    if finite == 0 {
        return None;
    }
    let stride = ((finite as f64 / N_SAMPLES as f64).max(1.0)) as usize;
    let mut samples: Vec<f32> = rows()
        .filter(|v| v.is_finite())
        .step_by(stride)
        .take(N_SAMPLES)
        .collect();
    samples.sort_unstable_by(f32::total_cmp);
    let npix = samples.len();
    let (mut vmin, mut vmax) = (f64::from(samples[0]), f64::from(samples[npix - 1]));
    let y: Vec<f64> = samples.iter().map(|&v| f64::from(v)).collect();
    let minpix = MIN_NPIXELS.max((npix as f64 * MAX_REJECT) as usize);
    let mut ngood = npix;
    let mut last_ngood = npix + 1;
    let mut bad = vec![false; npix];
    let ngrow = 1.max((npix as f64 * 0.01) as usize);
    let mut slope = 0.0;
    let mut fitted_any = false;
    for _ in 0..MAX_ITERATIONS {
        if ngood >= last_ngood || ngood < minpix {
            break;
        }
        // Least-squares line over the good samples.
        let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (i, &v) in y.iter().enumerate() {
            if !bad[i] {
                let x = i as f64;
                sw += 1.0;
                sx += x;
                sy += v;
                sxx += x * x;
                sxy += x * v;
            }
        }
        let den = sw * sxx - sx * sx;
        if den == 0.0 {
            break;
        }
        let b = (sw * sxy - sx * sy) / den;
        let a = (sy - b * sx) / sw;
        slope = b;
        fitted_any = true;
        // k-σ rejection (population σ of the good residuals).
        let flat: Vec<f64> = y
            .iter()
            .enumerate()
            .map(|(i, v)| v - (a + b * i as f64))
            .collect();
        let mean = flat
            .iter()
            .zip(&bad)
            .filter(|(_, b)| !**b)
            .map(|(f, _)| f)
            .sum::<f64>()
            / sw;
        let var = flat
            .iter()
            .zip(&bad)
            .filter(|(_, b)| !**b)
            .map(|(f, _)| (f - mean) * (f - mean))
            .sum::<f64>()
            / sw;
        let threshold = KREJ * var.sqrt();
        for (i, f) in flat.iter().enumerate() {
            if *f < -threshold || *f > threshold {
                bad[i] = true;
            }
        }
        // Dilate as np.convolve(bad, ones(ngrow), "same").
        let shift = (ngrow - 1) / 2;
        let mut grown = vec![false; npix];
        // Prefix counts make each window O(1).
        let mut prefix = vec![0usize; npix + 1];
        for i in 0..npix {
            prefix[i + 1] = prefix[i] + usize::from(bad[i]);
        }
        for (i, g) in grown.iter_mut().enumerate() {
            let hi = (i + shift).min(npix - 1);
            let lo = (i + shift + 1).saturating_sub(ngrow);
            *g = prefix[hi + 1] > prefix[lo];
        }
        bad = grown;
        last_ngood = ngood;
        ngood = bad.iter().filter(|b| !**b).count();
    }
    if ngood >= minpix && fitted_any {
        let slope = slope / CONTRAST;
        let center = (npix - 1) / 2;
        let median = if npix % 2 == 1 {
            y[npix / 2]
        } else {
            // np.median of float32 data averages in float32.
            f64::from(((y[npix / 2 - 1] + y[npix / 2]) / 2.0) as f32)
        };
        vmin = vmin.max(median - (center as f64 - 1.0) * slope);
        vmax = vmax.min(median + (npix - center) as f64 * slope);
    }
    Some((vmin as f32, vmax as f32))
}

/// Histogram equalisation table: `HISTEQ_BINS` entries, entry `i` the fraction of finite
/// values in `[lo, hi]` at or below bin `i`'s upper edge (values outside clamp to the ends).
pub fn histeq_lut(values: &[f32], lo: f32, hi: f32) -> Vec<f32> {
    let mut hist = vec![0u64; HISTEQ_BINS];
    let span = hi - lo;
    let scale = if span > 0.0 {
        HISTEQ_BINS as f32 / span
    } else {
        0.0
    };
    let stride = values.len().div_ceil(1 << 24).max(1);
    for &v in values.iter().step_by(stride) {
        if v.is_finite() {
            let b = ((v - lo) * scale).clamp(0.0, (HISTEQ_BINS - 1) as f32) as usize;
            hist[b] += 1;
        }
    }
    let total: u64 = hist.iter().sum();
    let mut acc = 0u64;
    hist.iter()
        .map(|&c| {
            acc += c;
            if total == 0 {
                0.0
            } else {
                acc as f32 / total as f32
            }
        })
        .collect()
}

/// The stretch of a normalised value (clamped to [0, 1]); `lut` is the histogram table
/// (only read for [`Stretch::HistEq`]). [`Stretch::Auto`] here is its midtones transfer at
/// the neutral balance 0.5 (the identity); see [`apply_mid`].
pub fn apply(s: Stretch, x: f32, lut: &[f32]) -> f32 {
    apply_mid(s, x, lut, 0.5)
}

/// [`apply`] with the midtones balance `mid` of [`Stretch::Auto`].
pub fn apply_mid(s: Stretch, x: f32, lut: &[f32], mid: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    match s {
        Stretch::Auto => mtf(mid, x),
        Stretch::Linear => x,
        Stretch::Sqrt => x.sqrt(),
        Stretch::Log => (LOG_A * x).ln_1p() / LOG_A.ln_1p(),
        Stretch::Asinh => (x / ASINH_A).asinh() / (1.0 / ASINH_A).asinh(),
        Stretch::HistEq => {
            if lut.is_empty() {
                return x;
            }
            let i = ((x * lut.len() as f32) as usize).min(lut.len() - 1);
            lut[i]
        }
    }
}

/// The midtones transfer function with balance `m` (0 < m < 1) of `x` in [0, 1]: 0 and 1
/// stay, `m` goes to 0.5. `mtf(m, x)` is also the balance taking `x` to `m`.
pub fn mtf(m: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let d = (2.0 * m - 1.0) * x - m;
    if d == 0.0 {
        return 0.5;
    }
    ((m - 1.0) * x / d).clamp(0.0, 1.0)
}

/// Shadows clipping of the automatic stretch, in normalised MADs below the median.
pub const STF_SHADOWS: f64 = -2.8;
/// Where the automatic stretch puts the median (the background).
pub const STF_TARGET: f64 = 0.25;
/// Values the automatic stretch's statistics are taken from, at most (an even stride).
const STF_SAMPLES: usize = 1 << 20;

/// The automatic stretch of values (PixInsight's AutoSTF, one channel): `(lo, hi, mid)`
/// with `lo` the median less 2.8 normalised MADs (not below the minimum), `hi` the maximum
/// and `mid` the midtones balance putting the median at [`STF_TARGET`]. Statistics come
/// from at most 1 Mi values in an even stride. `None` without a finite value. With the
/// median at the clipping point (a black, noiseless sky behind a planet: nothing to lift
/// out of the noise) the stretch is linear, where AutoSTF would turn everything lit white.
pub fn stf(values: &[f32]) -> Option<(f32, f32, f32)> {
    let finite = values.iter().filter(|v| v.is_finite()).count();
    if finite == 0 {
        return None;
    }
    let stride = finite.div_ceil(STF_SAMPLES).max(1);
    let mut s: Vec<f32> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .step_by(stride)
        .collect();
    let (lo_v, hi_v) = minmax(values)?;
    // The upper median of an even count (the statistics are of a sample anyway).
    let median = |s: &mut [f32]| -> f64 {
        let n = s.len();
        f64::from(*s.select_nth_unstable_by(n / 2, f32::total_cmp).1)
    };
    let med = median(&mut s);
    let mut dev: Vec<f32> = s
        .iter()
        .map(|&v| (f64::from(v) - med).abs() as f32)
        .collect();
    let mad = median(&mut dev) * 1.4826;
    let lo = (med + STF_SHADOWS * mad).max(f64::from(lo_v));
    let hi = f64::from(hi_v);
    if hi <= lo {
        return Some((lo as f32, hi as f32, 0.5));
    }
    let x = ((med - lo) / (hi - lo)) as f32;
    let mid = if x <= 0.0 {
        0.5
    } else {
        mtf(STF_TARGET as f32, x).clamp(1e-4, 0.9999)
    };
    Some((lo as f32, hi as f32, mid))
}

/// A 2×2 colour filter array (Bayer) pattern: the colour (0 red, 1 green, 2 blue) of the
/// pixel at column `x`, row `y` (top-down as displayed) is `self.0[(x & 1) + 2 (y & 1)]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cfa(pub [u8; 4]);

impl Cfa {
    pub const RGGB: Cfa = Cfa([0, 1, 1, 2]);
    pub const GRBG: Cfa = Cfa([1, 0, 2, 1]);
    pub const GBRG: Cfa = Cfa([1, 2, 0, 1]);
    pub const BGGR: Cfa = Cfa([2, 1, 1, 0]);
    /// The four Bayer layouts, in the order an override steps through them.
    pub const ALL: [Cfa; 4] = [Cfa::RGGB, Cfa::GRBG, Cfa::GBRG, Cfa::BGGR];

    /// A pattern named by its four letters, top row first ("RGGB", any case); `None` for
    /// anything else (CMY patterns, X-Trans).
    pub fn parse(s: &str) -> Option<Cfa> {
        let s = s.trim().to_ascii_uppercase();
        Cfa::ALL.into_iter().find(|c| c.name() == s)
    }

    /// The four letters, top row first.
    pub fn name(self) -> String {
        self.0
            .iter()
            .map(|&c| ['R', 'G', 'B'][usize::from(c.min(2))])
            .collect()
    }

    /// The colour at column `x`, row `y`.
    pub fn at(self, x: usize, y: usize) -> usize {
        usize::from(self.0[(x & 1) + 2 * (y & 1)])
    }

    /// The pattern seen from `dx` columns and `dy` rows further in (an image cropped by
    /// that much, or a pattern whose origin sits at that offset).
    pub fn shifted(self, dx: usize, dy: usize) -> Cfa {
        let mut p = [0u8; 4];
        for (i, o) in p.iter_mut().enumerate() {
            *o = self.0[((i & 1) + dx) % 2 + 2 * ((i / 2 + dy) % 2)];
        }
        Cfa(p)
    }

    /// The pattern of the image turned upside down when it has `height` rows.
    pub fn flipped(self, height: usize) -> Cfa {
        if height.is_multiple_of(2) {
            Cfa([self.0[2], self.0[3], self.0[0], self.0[1]])
        } else {
            self
        }
    }

    /// The next pattern of [`Cfa::ALL`] (for an override).
    pub fn next(self) -> Cfa {
        let i = Cfa::ALL.iter().position(|c| *c == self).unwrap_or(0);
        Cfa::ALL[(i + 1) % 4]
    }

    /// Bilinear demosaic of the pixel at `x`, `y` of a `width`×`height` raw frame: its own
    /// colour as is, the others the mean of the nearest pixels of that colour (the two or
    /// four neighbours across, or the four diagonal ones), the frame mirrored at its edges
    /// without repeating the edge (as `scipy.ndimage.convolve(mode="mirror")`, so the
    /// edge pixels' neighbours keep the pattern). Frames under 2×2 give their value in all
    /// three channels.
    pub fn debayer_at(
        self,
        values: &[f32],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
    ) -> [f32; 3] {
        let v = values[y * width + x];
        if width < 2 || height < 2 {
            return [v; 3];
        }
        let m = |i: isize, n: usize| -> usize {
            let n = n as isize;
            let i = if i < 0 {
                -i
            } else if i >= n {
                2 * n - 2 - i
            } else {
                i
            };
            i.clamp(0, n - 1) as usize
        };
        let at = |dx: isize, dy: isize| {
            values[m(y as isize + dy, height) * width + m(x as isize + dx, width)]
        };
        let own = self.at(x, y);
        let mut out = [0.0f32; 3];
        out[own] = v;
        let cross = (at(-1, 0) + at(1, 0) + at(0, -1) + at(0, 1)) * 0.25;
        let diag = (at(-1, -1) + at(1, -1) + at(-1, 1) + at(1, 1)) * 0.25;
        let horiz = (at(-1, 0) + at(1, 0)) * 0.5;
        let vert = (at(0, -1) + at(0, 1)) * 0.5;
        if own == 1 {
            // Green: red and blue lie across, one pair along the row, one along the column.
            let row = self.at(x + 1, y);
            out[row] = horiz;
            out[2 - row] = vert;
        } else {
            out[1] = cross;
            out[2 - own] = diag;
        }
        out
    }

    /// [`Cfa::debayer_at`] of every pixel of row `y`, handed to `put(x, rgb)` left to right
    /// (the fast path: neighbours are read straight from the three rows).
    pub fn debayer_row(
        self,
        values: &[f32],
        width: usize,
        height: usize,
        y: usize,
        mut put: impl FnMut(usize, [f32; 3]),
    ) {
        let r = &values[y * width..][..width];
        if width < 2 || height < 2 {
            for (x, &v) in r.iter().enumerate() {
                put(x, [v; 3]);
            }
            return;
        }
        let ym = if y == 0 { 1 } else { y - 1 };
        let yp = if y + 1 == height { height - 2 } else { y + 1 };
        let (u, d) = (
            &values[ym * width..][..width],
            &values[yp * width..][..width],
        );
        // Row y's colours at even and odd columns, and the other colour of each green.
        let (c0, c1) = (self.at(0, y), self.at(1, y));
        for x in 0..width {
            let xm = if x == 0 { 1 } else { x - 1 };
            let xp = if x + 1 == width { width - 2 } else { x + 1 };
            let (own, row) = if x & 1 == 0 { (c0, c1) } else { (c1, c0) };
            let mut out = [0.0f32; 3];
            out[own] = r[x];
            if own == 1 {
                out[row] = (r[xm] + r[xp]) * 0.5;
                out[2 - row] = (u[x] + d[x]) * 0.5;
            } else {
                out[1] = (r[xm] + r[xp] + u[x] + d[x]) * 0.25;
                out[2 - own] = (u[xm] + u[xp] + d[xm] + d[xp]) * 0.25;
            }
            put(x, out);
        }
    }

    /// The samples of colour `c` (0 red, 1 green, 2 blue) of a raw frame, from pairs of
    /// rows in an even stride so that about `max` are taken (for statistics).
    pub fn samples(self, values: &[f32], width: usize, c: usize, max: usize) -> Vec<f32> {
        let height = values.len() / width.max(1);
        let pairs = height.div_ceil(2);
        let step = (values.len() / max.max(1)).max(1).min(pairs.max(1));
        let mut out = Vec::new();
        for p in (0..pairs).step_by(step) {
            for y in [2 * p, 2 * p + 1] {
                if y >= height {
                    continue;
                }
                let row = &values[y * width..][..width];
                for (x, &v) in row.iter().enumerate() {
                    if self.at(x, y) == c {
                        out.push(v);
                    }
                }
            }
        }
        out
    }

    /// The unlinked automatic stretch ([`stf`]) of each colour of a raw frame, from its own
    /// samples; `None` for a colour without a finite sample.
    pub fn stf(self, values: &[f32], width: usize) -> [Option<(f32, f32, f32)>; 3] {
        [0, 1, 2].map(|c| stf(&self.samples(values, width, c, 4 * STF_SAMPLES)))
    }
}

/// Grey levels (0–255, gamma-encoded as display values) of a plane under `limits` and
/// `stretch`; NaN shows black. For thumbnails and the CPU fallback. [`Stretch::Auto`] takes
/// its own limits ([`stf`]).
pub fn to_grey(values: &[f32], width: usize, limits: Limits, stretch: Stretch) -> Vec<u8> {
    let (lo, hi, mid) = if stretch == Stretch::Auto {
        stf(values).unwrap_or((0.0, 1.0, 0.5))
    } else {
        let (lo, hi) = limits.compute(values, width).unwrap_or((0.0, 1.0));
        (lo, hi, 0.5)
    };
    let lut = if stretch == Stretch::HistEq {
        histeq_lut(values, lo, hi)
    } else {
        Vec::new()
    };
    let inv = if hi > lo { 1.0 / (hi - lo) } else { 0.0 };
    values
        .iter()
        .map(|&v| {
            if v.is_finite() {
                (apply_mid(stretch, (v - lo) * inv, &lut, mid) * 255.0 + 0.5) as u8
            } else {
                0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_ends() {
        for s in Stretch::ALL {
            if s == Stretch::HistEq {
                continue;
            }
            assert!(apply(s, 0.0, &[]).abs() < 1e-6, "{s:?}");
            assert!((apply(s, 1.0, &[]) - 1.0).abs() < 1e-6, "{s:?}");
            assert!(apply(s, 0.3, &[]) >= 0.3 - 1e-6, "{s:?} brightens");
        }
        assert_eq!(Stretch::from_code("asinh"), Some(Stretch::Asinh));
        assert_eq!(Limits::from_code("p99.5"), Some(Limits::Percentile(99.5)));
    }

    #[test]
    fn limits() {
        let v: Vec<f32> = (0..101).map(|i| i as f32).chain([f32::NAN]).collect();
        assert_eq!(minmax(&v), Some((0.0, 100.0)));
        assert_eq!(percentiles(&v, 10.0, 90.0), Some((10.0, 90.0)));
        assert_eq!(percentiles(&[1.0, 2.0], 50.0, 50.0), Some((1.5, 1.5)));
        assert_eq!(minmax(&[f32::NAN]), None);
        // A linear ramp: zscale keeps the whole range.
        let (lo, hi) = zscale(&v, 101).unwrap();
        assert_eq!((lo, hi), (0.0, 100.0));
        // A sparse counts image (nearly every bin 0): the sampled limits collapse to 0, the
        // plane's range keeps the counts visible (the visual review sheets).
        let mut sparse = vec![0f32; 300 * 300];
        for i in (0..sparse.len()).step_by(997) {
            sparse[i] = 1.0 + (i % 3) as f32;
        }
        assert_eq!(zscale(&sparse, 300), Some((0.0, 0.0)));
        assert_eq!(Limits::ZScale.compute(&sparse, 300), Some((0.0, 3.0)));
        assert_eq!(
            Limits::Percentile(99.0).compute(&sparse, 300),
            Some((0.0, 3.0))
        );
        let g = to_grey(&sparse, 300, Limits::ZScale, Stretch::Linear);
        assert_eq!(g[0], 85);
        assert_eq!(g[1], 0);
        // A frame that is mostly fill (0) with a few wild values (Voyager's GEOMED frames): the
        // zscale sample collapses on the fill; the percentiles keep the wild values out, as
        // the plane's range would not.
        let mut fill = vec![0f32; 100 * 100];
        for (i, v) in fill.iter_mut().enumerate().skip(9_000) {
            *v = (i % 100) as f32;
        }
        fill[9_500] = -32_147.0;
        fill[9_501] = 8_576.0;
        assert_eq!(zscale(&fill, 100), Some((0.0, 0.0)));
        let (lo, hi) = Limits::ZScale.compute(&fill, 100).unwrap();
        assert!(lo == 0.0 && hi > 50.0 && hi < 100.0, "{lo}..{hi}");
        // A constant plane stays constant.
        assert_eq!(Limits::ZScale.compute(&[2.0; 16], 4), Some((2.0, 2.0)));
        // Seismic gain: symmetric about 0 from the magnitudes.
        let w: Vec<f32> = (-50..=100).map(|i| i as f32).collect();
        assert_eq!(
            Limits::Symmetric(100.0).compute(&w, 151),
            Some((-100.0, 100.0))
        );
        assert_eq!(symmetric(&[f32::NAN], 99.0), None);
        // Unsigned samples (offset binary, 0..4000 around 2000): centred on their median, so
        // the diverging palette shows both polarities rather than one colour.
        let u: Vec<f32> = (0..=4000).map(|i| i as f32).collect();
        assert_eq!(symmetric(&u, 100.0), Some((0.0, 4000.0)));
        let (lo, hi) = Limits::SEISMIC.compute(&u, 4001).unwrap();
        assert!(lo > 0.0 && hi < 4000.0 && ((lo + hi) / 2.0 - 2000.0).abs() < 1.0);
        assert_eq!(Limits::from_code("s98"), Some(Limits::Symmetric(98.0)));
        assert_eq!(
            Limits::from_code(&Limits::SEISMIC.code()),
            Some(Limits::SEISMIC)
        );
        assert_eq!(
            Limits::Symmetric(90.0).next_symmetric(),
            Limits::Symmetric(100.0)
        );
    }

    #[test]
    fn auto_stretch() {
        // Background around 100 (noise ±2), a few stars to 1000.
        let mut v: Vec<f32> = (0..10_000)
            .map(|i| 100.0 + ((i * 7919) % 5) as f32 - 2.0)
            .collect();
        v[17] = 1000.0;
        v[999] = 700.0;
        let (lo, hi, mid) = stf(&v).unwrap();
        assert_eq!(hi, 1000.0);
        assert!((98.0..100.0).contains(&lo), "{lo}");
        // The median lands at the target.
        let x = (100.0 - lo) / (hi - lo);
        assert!(
            (mtf(mid, x) - STF_TARGET as f32).abs() < 1e-4,
            "{}",
            mtf(mid, x)
        );
        assert_eq!(mtf(0.3, 0.0), 0.0);
        assert_eq!(mtf(0.3, 1.0), 1.0);
        assert!((mtf(0.3, 0.3) - 0.5).abs() < 1e-6);
        assert_eq!(Stretch::from_code("auto"), Some(Stretch::Auto));
        assert!(stf(&[f32::NAN]).is_none());
        // A planet on a black sky: linear, not blown out.
        let mut p = vec![0.0f32; 1000];
        p[..100]
            .iter_mut()
            .enumerate()
            .for_each(|(i, v)| *v = 50.0 + i as f32);
        assert_eq!(stf(&p), Some((0.0, 149.0, 0.5)));
    }

    #[test]
    fn cfa_patterns() {
        assert_eq!(Cfa::parse("rggb"), Some(Cfa::RGGB));
        assert_eq!(Cfa::parse("CYYM"), None);
        assert_eq!(Cfa::BGGR.name(), "BGGR");
        assert_eq!(Cfa::RGGB.shifted(1, 0), Cfa::GRBG);
        assert_eq!(Cfa::RGGB.shifted(0, 1), Cfa::GBRG);
        assert_eq!(Cfa::RGGB.shifted(1, 1), Cfa::BGGR);
        assert_eq!(Cfa::RGGB.flipped(4), Cfa::GBRG);
        assert_eq!(Cfa::RGGB.flipped(5), Cfa::RGGB);
        assert_eq!(Cfa::BGGR.next(), Cfa::RGGB);
        // A flat colour field demosaics to itself, edges included.
        let (w, h) = (5, 4);
        let rgb = [10.0f32, 20.0, 30.0];
        let raw: Vec<f32> = (0..w * h)
            .map(|i| rgb[Cfa::GRBG.at(i % w, i / w)])
            .collect();
        for y in 0..h {
            for x in 0..w {
                assert_eq!(Cfa::GRBG.debayer_at(&raw, w, h, x, y), rgb, "{x},{y}");
            }
        }
        // Red at a blue site is the mean of the four diagonal reds.
        let raw: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let c = Cfa::RGGB.debayer_at(&raw, 4, 4, 1, 1);
        assert_eq!(
            c,
            [
                (0.0 + 2.0 + 8.0 + 10.0) / 4.0,
                (1.0 + 4.0 + 6.0 + 9.0) / 4.0,
                5.0
            ]
        );
        // The row fast path is the reference, edges included.
        for (w, h) in [(2, 2), (3, 5), (8, 7), (9, 6), (1, 4)] {
            let raw: Vec<f32> = (0..w * h).map(|i| ((i * 7919) % 101) as f32).collect();
            for cfa in Cfa::ALL {
                for y in 0..h {
                    cfa.debayer_row(&raw, w, h, y, |x, rgb| {
                        assert_eq!(rgb, cfa.debayer_at(&raw, w, h, x, y), "{w}×{h} {x},{y}");
                    });
                }
            }
        }
        // Per-colour statistics take only that colour's samples.
        let raw: Vec<f32> = (0..64)
            .map(|i| [10.0, 20.0, 20.0, 30.0][i % 2 + 2 * (i / 8 % 2)])
            .collect();
        let s = Cfa::RGGB.samples(&raw, 8, 0, 1 << 20);
        assert_eq!(s.len(), 16);
        assert!(s.iter().all(|&v| v == 10.0));
        assert!(
            Cfa::RGGB
                .samples(&raw, 8, 2, 1 << 20)
                .iter()
                .all(|&v| v == 30.0)
        );
    }

    #[test]
    fn histeq() {
        let v: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).powi(4)).collect();
        let lut = histeq_lut(&v, 0.0, 1.0);
        assert_eq!(lut.len(), HISTEQ_BINS);
        assert!((lut[HISTEQ_BINS - 1] - 1.0).abs() < 1e-6);
        // Half the values lie below 0.0625.
        let i = (0.0625 * HISTEQ_BINS as f32) as usize;
        assert!((lut[i] - 0.5).abs() < 0.01, "{}", lut[i]);
    }
}
