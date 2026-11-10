//! DNG camera profile look tables: `ProfileHueSatMap` and `ProfileLookTable` (DNG 1.7.1 chapter
//! 6, "Applying the Hue/Saturation/Value Mapping Table"), applied in linear ProPhoto RGB.
//!
//! Profiles made by Adobe ("Adobe Standard") pair their ForwardMatrix with these tables; the
//! matrix alone gives flat, under-saturated colour. Order per the specification: camera → XYZ
//! (D50) → ProPhoto → HueSatMap → exposure → LookTable → ProfileToneCurve → output.

/// Largest accepted hue / saturation / value division count of a table.
const MAX_DIV: u32 = 1024;
/// Largest accepted table size in entries (Adobe Standard uses 2 700 and 4 608).
const MAX_ENTRIES: u32 = 1 << 16;

/// One hue/saturation/value mapping table (value divisions outer, hue middle, saturation inner;
/// each entry is hue shift in degrees, saturation scale, value scale).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HsvTable {
    hue: u32,
    sat: u32,
    val: u32,
    /// Index a 3D table with sRGB-encoded value (`Profile*Encoding` = 1).
    srgb: bool,
    data: Vec<[f32; 3]>,
}

impl HsvTable {
    /// A table of `dims` (hue, saturation, value divisions) from its tag values. `None` when the
    /// dimensions are out of range, the data count does not match, or an entry is not finite or
    /// has a negative scale.
    pub(crate) fn new(dims: &[u32], data: &[f64], encoding: u32) -> Option<HsvTable> {
        let &[hue, sat, val] = dims else {
            return None;
        };
        if !(1..=MAX_DIV).contains(&hue)
            || !(2..=MAX_DIV).contains(&sat)
            || !(1..=MAX_DIV).contains(&val)
            || encoding > 1
        {
            return None;
        }
        let n = hue.checked_mul(sat)?.checked_mul(val)?;
        if n > MAX_ENTRIES || data.len() != n as usize * 3 {
            return None;
        }
        let mut out = Vec::with_capacity(n as usize);
        for e in data.chunks_exact(3) {
            if !e.iter().all(|v| v.is_finite() && v.abs() < 1e6) || e[1] < 0.0 || e[2] < 0.0 {
                return None;
            }
            out.push([e[0] as f32, e[1] as f32, e[2] as f32]);
        }
        Some(HsvTable {
            hue,
            sat,
            val,
            srgb: encoding == 1 && val > 1,
            data: out,
        })
    }

    /// `a * w + b * (1 - w)`, entry by entry (tables of one profile share their dimensions).
    pub(crate) fn lerp(a: &HsvTable, b: &HsvTable, w: f64) -> Option<HsvTable> {
        if (a.hue, a.sat, a.val) != (b.hue, b.sat, b.val) {
            return None;
        }
        let w = w as f32;
        Some(HsvTable {
            data: a
                .data
                .iter()
                .zip(&b.data)
                .map(|(x, y)| std::array::from_fn(|i| x[i] * w + y[i] * (1.0 - w)))
                .collect(),
            ..a.clone()
        })
    }

    fn at(&self, v: usize, h: usize, s: usize) -> [f32; 3] {
        self.data[(v * self.hue as usize + h) * self.sat as usize + s]
    }

    /// Trilinear lookup: `h6` in [0, 6) (hue / 60 degrees), `s` and `v` in [0, 1]. Hue wraps,
    /// saturation and value clamp at the table edges.
    fn lookup(&self, h6: f32, s: f32, v: f32) -> [f32; 3] {
        let hs = h6 * self.hue as f32 / 6.0;
        let mut h0 = (hs.max(0.0) as usize).min(self.hue as usize - 1);
        let hf = (hs - h0 as f32).clamp(0.0, 1.0);
        let h1 = if h0 + 1 >= self.hue as usize {
            0
        } else {
            h0 + 1
        };
        if self.hue == 1 {
            h0 = 0;
        }
        let ss = s.clamp(0.0, 1.0) * (self.sat - 1) as f32;
        let s0 = (ss as usize).min(self.sat as usize - 2);
        let sf = ss - s0 as f32;
        let (v0, v1, vf) = if self.val > 1 {
            let vs = v.clamp(0.0, 1.0) * (self.val - 1) as f32;
            let v0 = (vs as usize).min(self.val as usize - 2);
            (v0, v0 + 1, vs - v0 as f32)
        } else {
            (0, 0, 0.0)
        };
        let mix = |a: [f32; 3], b: [f32; 3], f: f32| -> [f32; 3] {
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f)
        };
        let plane = |v: usize| {
            mix(
                mix(self.at(v, h0, s0), self.at(v, h1, s0), hf),
                mix(self.at(v, h0, s0 + 1), self.at(v, h1, s0 + 1), hf),
                sf,
            )
        };
        if self.val > 1 {
            mix(plane(v0), plane(v1), vf)
        } else {
            plane(0)
        }
    }

    /// Map HSV (`h6` in [0, 6)) through the table. `clip`: standard dynamic range (value clipped
    /// to 1).
    fn map(&self, (h6, s, v): (f32, f32, f32), clip: bool) -> (f32, f32, f32) {
        let ve = if self.srgb { srgb_encode(v) } else { v };
        let [dh, ss, vs] = self.lookup(h6, s, ve);
        let mut h = h6 + dh / 60.0;
        h = h.rem_euclid(6.0);
        if !h.is_finite() || h >= 6.0 {
            h = 0.0;
        }
        let s = (s * ss).min(1.0);
        let mut v = (ve * vs).max(0.0);
        if clip {
            v = v.min(1.0);
        }
        if self.srgb {
            v = srgb_decode(v);
        }
        (h, s, v)
    }
}

fn srgb_encode_exact(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_decode_exact(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Steps of the transfer-curve tables; linear interpolation between them stays within 2e-5 of
/// the curve, below 16-bit precision.
const CURVE_STEPS: usize = 4096;

fn curve_table(f: fn(f32) -> f32) -> Vec<f32> {
    (0..=CURVE_STEPS)
        .map(|i| f(i as f32 / CURVE_STEPS as f32))
        .collect()
}

static SRGB_ENCODE: std::sync::LazyLock<Vec<f32>> =
    std::sync::LazyLock::new(|| curve_table(srgb_encode_exact));
static SRGB_DECODE: std::sync::LazyLock<Vec<f32>> =
    std::sync::LazyLock::new(|| curve_table(srgb_decode_exact));

/// `f` on [0, 1] by table, exactly outside it (and on the linear toe of the encoding, where the
/// curve is steepest).
#[inline]
fn curve(t: &[f32], exact: fn(f32) -> f32, v: f32) -> f32 {
    if !(v > 0.0031308 && v < 1.0) {
        return exact(v);
    }
    let x = v * CURVE_STEPS as f32;
    let i = (x as usize).min(CURVE_STEPS - 1);
    let f = x - i as f32;
    t[i] + (t[i + 1] - t[i]) * f
}

fn srgb_encode(v: f32) -> f32 {
    curve(&SRGB_ENCODE, srgb_encode_exact, v)
}

fn srgb_decode(v: f32) -> f32 {
    curve(&SRGB_DECODE, srgb_decode_exact, v)
}

/// High dynamic range encoding before indexing a 3D table (DNG 1.7 `ProfileDynamicRange`).
fn hdr_encode(x: f32) -> f32 {
    x * (256.0 + x) / (256.0 * (1.0 + x))
}

fn hdr_decode(y: f32) -> f32 {
    16.0 * (8.0 * y - 8.0 + (64.0 * y * y - 127.0 * y + 64.0).max(0.0).sqrt())
}

/// RGB (components >= 0) → HSV with hue in [0, 6).
fn rgb_to_hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let v = r.max(g).max(b);
    let gap = v - r.min(g).min(b);
    if gap <= 0.0 {
        return (0.0, 0.0, v);
    }
    let h = if r == v {
        let h = (g - b) / gap;
        if h < 0.0 { h + 6.0 } else { h }
    } else if g == v {
        2.0 + (b - r) / gap
    } else {
        4.0 + (r - g) / gap
    };
    (h, gap / v, v)
}

fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> [f32; 3] {
    if s <= 0.0 {
        return [v; 3];
    }
    let h = if h >= 6.0 || h < 0.0 { 0.0 } else { h };
    let i = (h as u32).min(5);
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Largest accepted `ProfileToneCurve` point count (Adobe's default curve has 1 025).
const MAX_CURVE_POINTS: usize = 8192;
/// Resolution of the baked tone curve.
const CURVE_LUT: usize = 4096;

/// A DNG `ProfileToneCurve`: output = curve(input) on linear ProPhoto values in 0..=1, through
/// its points by a natural cubic spline, baked into a table. Applied the way the DNG reference
/// renders RGB tone: the largest and smallest component follow the curve and the middle one keeps
/// its relative place between them, so hue does not shift.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToneCurve {
    lut: Vec<f32>,
}

impl ToneCurve {
    /// The curve from the tag's values (x0, y0, x1, y1, ...). `None` when malformed (fewer than
    /// two points, x not strictly increasing, values outside 0..=1) or the identity.
    pub(crate) fn new(v: &[f64]) -> Option<ToneCurve> {
        if !v.len().is_multiple_of(2) || v.len() < 4 || v.len() > 2 * MAX_CURVE_POINTS {
            return None;
        }
        let xs: Vec<f64> = v.iter().step_by(2).copied().collect();
        let ys: Vec<f64> = v.iter().skip(1).step_by(2).copied().collect();
        if v.iter().any(|x| !(0.0..=1.0).contains(x)) || xs.windows(2).any(|w| w[1] <= w[0]) {
            return None;
        }
        let identity = xs.iter().zip(&ys).all(|(x, y)| (x - y).abs() < 1e-6)
            && xs[0] == 0.0
            && xs[xs.len() - 1] == 1.0;
        if identity {
            return None;
        }
        let m = natural_spline(&xs, &ys);
        let lut = (0..=CURVE_LUT)
            .map(|i| spline_eval(&xs, &ys, &m, i as f64 / CURVE_LUT as f64).clamp(0.0, 1.0) as f32)
            .collect();
        Some(ToneCurve { lut })
    }

    fn eval(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0) * CURVE_LUT as f32;
        let i = (x as usize).min(CURVE_LUT - 1);
        let f = x - i as f32;
        self.lut[i] + (self.lut[i + 1] - self.lut[i]) * f
    }

    /// Hue-preserving RGB tone: components clamped to 0..=1.
    pub(crate) fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        let p = p.map(|x| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) });
        let (mut lo, mut hi) = (0, 2);
        for i in 0..3 {
            if p[i] < p[lo] {
                lo = i;
            }
            if p[i] > p[hi] {
                hi = i;
            }
        }
        if p[hi] <= p[lo] {
            return [self.eval(p[0]); 3];
        }
        let mid = 3 - lo - hi;
        let (a, b) = (self.eval(p[lo]), self.eval(p[hi]));
        let mut out = [0.0; 3];
        out[lo] = a;
        out[hi] = b;
        out[mid] = a + (b - a) * (p[mid] - p[lo]) / (p[hi] - p[lo]);
        out
    }
}

/// Second derivatives of the natural cubic spline through (xs, ys) (zero at both ends).
fn natural_spline(xs: &[f64], ys: &[f64]) -> Vec<f64> {
    let n = xs.len();
    let mut m = vec![0.0; n];
    if n < 3 {
        return m;
    }
    // Tridiagonal system for the interior points (Thomas algorithm).
    let mut c = vec![0.0; n];
    let mut r = vec![0.0; n];
    for i in 1..n - 1 {
        let h0 = xs[i] - xs[i - 1];
        let h1 = xs[i + 1] - xs[i];
        let a = h0 / 6.0;
        let b = (h0 + h1) / 3.0;
        let d = (ys[i + 1] - ys[i]) / h1 - (ys[i] - ys[i - 1]) / h0;
        let den = b - a * c[i - 1];
        c[i] = h1 / 6.0 / den;
        r[i] = (d - a * r[i - 1]) / den;
    }
    for i in (1..n - 1).rev() {
        m[i] = r[i] - c[i] * m[i + 1];
    }
    m
}

fn spline_eval(xs: &[f64], ys: &[f64], m: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let j = xs.partition_point(|&v| v <= x).clamp(1, n - 1);
    let (x0, x1) = (xs[j - 1], xs[j]);
    let h = x1 - x0;
    let a = (x1 - x) / h;
    let b = (x - x0) / h;
    a * ys[j - 1] + b * ys[j] + ((a * a * a - a) * m[j - 1] + (b * b * b - b) * m[j]) * h * h / 6.0
}

fn mat(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// A DNG camera profile with look tables, evaluated per pixel on the CPU by [`crate::develop`]:
/// white-balanced camera RGB (white = 1) → linear Rec.709 (white = 1).
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileTransform {
    /// White-balanced camera RGB → linear ProPhoto RGB (rows sum to 1).
    pub(crate) pro_cam: [[f32; 3]; 3],
    /// Linear ProPhoto RGB (D50) → linear Rec.709 (D65), Bradford (rows sum to 1).
    pub(crate) rgb_pro: [[f32; 3]; 3],
    pub(crate) hue_sat_map: Option<HsvTable>,
    pub(crate) look_table: Option<HsvTable>,
    /// ProfileToneCurve, applied after the LookTable at the exposed level (standard dynamic
    /// range profiles).
    pub(crate) tone_curve: Option<ToneCurve>,
    /// Exposure gain the viewer applies after developing (`2^BaselineExposure`, see
    /// [`crate::RawInfo::baseline_exposure`]): the LookTable is defined after exposure.
    pub(crate) gain: f32,
    /// High dynamic range profile (`ProfileDynamicRange`): no clipping at 1, HDR encoding of
    /// 3D tables.
    pub(crate) hdr: bool,
}

impl ProfileTransform {
    /// White-balanced camera RGB (white = 1) → linear Rec.709 (white = 1), before the viewer's
    /// exposure gain.
    pub fn apply(&self, cam: [f32; 3]) -> [f32; 3] {
        let p = mat(&self.pro_cam, cam);
        let g = self.gain;
        let out = if self.hdr {
            let mut p = p.map(|x| x.max(0.0));
            if let Some(t) = &self.hue_sat_map {
                p = hdr_stage(t, p);
            }
            if let Some(t) = &self.look_table {
                let g = g.max(1.0);
                p = hdr_stage(t, p.map(|x| x * g)).map(|x| x / g);
            }
            p
        } else {
            // Standard dynamic range. Positive exposure is a linear ramp that clips each
            // ProPhoto component at 1 before the LookTable; negative exposure is left to the
            // later tone stage, so the LookTable sees the unexposed value. Exposure only
            // scales value, so unclipped pixels need a single HSV round trip.
            let g = g.max(1.0);
            let (mut h, mut s, mut v) = rgb_to_hsv(p.map(|x| x.clamp(0.0, 1.0)));
            if let Some(t) = &self.hue_sat_map {
                (h, s, v) = t.map((h, s, v), true);
            }
            if v * g > 1.0 {
                let c = hsv_to_rgb((h, s, v)).map(|x| (x * g).min(1.0));
                (h, s, v) = rgb_to_hsv(c);
            } else {
                v *= g;
            }
            if let Some(t) = &self.look_table {
                (h, s, v) = t.map((h, s, v), true);
            }
            match &self.tone_curve {
                // The curve sees the exposed value; the viewer's gain follows, so divide it out.
                Some(c) => c.apply(hsv_to_rgb((h, s, v))).map(|x| x / g),
                None => hsv_to_rgb((h, s, v / g)),
            }
        };
        mat(&self.rgb_pro, out)
    }
}

/// One hue/saturation/value table as plain data (see [`ProfileTransform::hue_sat_map`]).
#[derive(Clone, Debug, PartialEq)]
pub struct HsvTableData {
    /// Hue, saturation and value divisions.
    pub hue: u32,
    pub sat: u32,
    pub val: u32,
    /// The value index is sRGB-encoded (`Profile*Encoding` = 1).
    pub srgb: bool,
    /// `data[(v * hue + h) * sat + s]`: hue shift in degrees, saturation scale, value scale.
    pub data: Vec<[f32; 3]>,
}

impl HsvTable {
    pub(crate) fn plain(&self) -> HsvTableData {
        HsvTableData {
            hue: self.hue,
            sat: self.sat,
            val: self.val,
            srgb: self.srgb,
            data: self.data.clone(),
        }
    }
}

impl ProfileTransform {
    /// White-balanced camera RGB → linear ProPhoto RGB (D50), rows summing to 1.
    pub fn camera_to_prophoto(&self) -> [[f32; 3]; 3] {
        self.pro_cam
    }

    /// Linear ProPhoto RGB (D50) → linear Rec.709 (D65), Bradford.
    pub fn prophoto_to_rec709(&self) -> [[f32; 3]; 3] {
        self.rgb_pro
    }

    /// `ProfileHueSatMap` (interpolated for the as-shot illuminant), applied in ProPhoto RGB.
    pub fn hue_sat_map(&self) -> Option<HsvTableData> {
        self.hue_sat_map.as_ref().map(HsvTable::plain)
    }

    /// `ProfileLookTable`, after the HueSatMap.
    pub fn look_table(&self) -> Option<HsvTableData> {
        self.look_table.as_ref().map(HsvTable::plain)
    }

    /// A high dynamic range profile (`ProfileDynamicRange`): 3D tables are indexed by
    /// HDR-encoded values.
    pub fn is_hdr(&self) -> bool {
        self.hdr
    }
}

fn hdr_stage(t: &HsvTable, p: [f32; 3]) -> [f32; 3] {
    let enc = t.val > 1;
    let p = if enc { p.map(hdr_encode) } else { p };
    let (h, s, v) = rgb_to_hsv(p);
    // The lookup clamps the value index to [0, 1]; the value itself is not clipped.
    let (h, s, v) = t.map((h, s, v), false);
    let p = hsv_to_rgb((h, s, v));
    if enc { p.map(hdr_decode) } else { p }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_tables_match_the_formulas() {
        for i in 0..=100_000 {
            let v = i as f32 / 100_000.0;
            assert!(
                (srgb_encode(v) - srgb_encode_exact(v)).abs() < 2e-5,
                "encode {v}"
            );
            assert!(
                (srgb_decode(v) - srgb_decode_exact(v)).abs() < 2e-5,
                "decode {v}"
            );
        }
        for v in [-1.0, 1.0, 2.5, f32::INFINITY] {
            assert_eq!(srgb_encode(v), srgb_encode_exact(v));
        }
        assert!(srgb_encode(f32::NAN).is_nan());
    }

    const ID3: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    fn table(hue: u32, sat: u32, val: u32, f: impl Fn(u32, u32, u32) -> [f64; 3]) -> HsvTable {
        let mut d = Vec::new();
        for v in 0..val {
            for h in 0..hue {
                for s in 0..sat {
                    d.extend(f(h, s, v));
                }
            }
        }
        HsvTable::new(&[hue, sat, val], &d, 0).unwrap()
    }

    fn close(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
    }

    fn transform(hsm: Option<HsvTable>, look: Option<HsvTable>, gain: f32) -> ProfileTransform {
        ProfileTransform {
            pro_cam: ID3,
            rgb_pro: ID3,
            hue_sat_map: hsm,
            look_table: look,
            tone_curve: None,
            gain,
            hdr: false,
        }
    }

    #[test]
    fn hsv_roundtrip() {
        for rgb in [
            [0.2, 0.5, 0.9],
            [0.9, 0.1, 0.3],
            [0.4, 0.4, 0.1],
            [0.0, 0.0, 0.0],
            [0.7, 0.7, 0.7],
            [1.0, 0.0, 0.5],
        ] {
            let back = hsv_to_rgb(rgb_to_hsv(rgb));
            assert!(close(rgb, back, 1e-6), "{rgb:?} {back:?}");
        }
    }

    #[test]
    fn identity_tables_change_nothing() {
        let id = table(6, 3, 1, |_, _, _| [0.0, 1.0, 1.0]);
        let look = table(4, 2, 3, |_, _, _| [0.0, 1.0, 1.0]);
        let t = transform(Some(id), Some(look), 1.1);
        for c in [[0.2, 0.5, 0.9], [0.5, 0.1, 0.05], [0.3, 0.3, 0.3]] {
            assert!(close(t.apply(c), c, 1e-5), "{c:?} {:?}", t.apply(c));
        }
    }

    #[test]
    fn hue_wraps_between_last_and_first_division() {
        // 4 hue divisions at 0, 90, 180, 270 degrees; shift 0 everywhere except 10 degrees at
        // 270. Between 270 and 360 the lookup interpolates towards the 0-degree column.
        let t = table(4, 2, 1, |h, _, _| {
            [if h == 3 { 10.0 } else { 0.0 }, 1.0, 1.0]
        });
        let at = |deg: f32| t.lookup(deg / 60.0, 0.5, 0.5)[0];
        assert!((at(270.0) - 10.0).abs() < 1e-5);
        assert!((at(315.0) - 5.0).abs() < 1e-4);
        assert!(at(359.9).abs() < 0.02);
        assert!(at(0.0).abs() < 1e-6);
        // A single hue division is constant around the circle.
        let one = table(1, 2, 1, |_, _, _| [7.0, 1.0, 1.0]);
        for deg in [0.0, 100.0, 359.0] {
            assert!((one.lookup(deg / 60.0, 0.3, 0.5)[0] - 7.0).abs() < 1e-6);
        }
    }

    #[test]
    fn hue_shift_wraps_angle() {
        let t = table(6, 2, 1, |_, _, _| [-30.0, 1.0, 1.0]);
        let (h, s, v) = t.map((0.25, 0.5, 0.5), true);
        assert!((h - (6.0 - 0.25)).abs() < 1e-5, "{h}");
        assert_eq!((s, v), (0.5, 0.5));
    }

    #[test]
    fn saturation_and_value_clamp_at_edges() {
        // Saturation scale 1 at s = 0, 3 at s = 1; value scale 2.
        let t = table(3, 2, 1, |_, s, _| {
            [0.0, if s == 0 { 1.0 } else { 3.0 }, 2.0]
        });
        assert!((t.lookup(1.0, 2.0, 0.5)[1] - 3.0).abs() < 1e-6);
        assert!((t.lookup(1.0, -1.0, 0.5)[1] - 1.0).abs() < 1e-6);
        let (_, s, v) = t.map((1.0, 0.9, 0.8), true);
        assert_eq!((s, v), (1.0, 1.0));
        let (_, _, v) = t.map((1.0, 0.9, 0.8), false);
        assert!((v - 1.6).abs() < 1e-6);
        // Value axis: 3 divisions, scales 1, 2, 3 at v = 0, 0.5, 1; out of range clamps.
        let t = table(2, 2, 3, |_, _, v| [0.0, 1.0, 1.0 + v as f64]);
        assert!((t.lookup(0.0, 0.0, 0.25)[2] - 1.5).abs() < 1e-6);
        assert!((t.lookup(0.0, 0.0, 7.0)[2] - 3.0).abs() < 1e-6);
        assert!((t.lookup(0.0, 0.0, -1.0)[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn srgb_encoding_indexes_by_encoded_value() {
        // Identity table: sRGB encoding must round-trip value.
        let t = HsvTable::new(&[2, 2, 2], &[0.0, 1.0, 1.0].repeat(8), 1).unwrap();
        assert!(t.srgb);
        let (_, _, v) = t.map((0.0, 0.0, 0.18), true);
        assert!((v - 0.18).abs() < 1e-5, "{v}");
        // Value scale 0.5 on encoded value: linear 0.18 -> encoded e -> 0.5e -> decoded.
        let half = table(2, 2, 2, |_, _, _| [0.0, 1.0, 0.5]);
        let mut srgb = half.clone();
        srgb.srgb = true;
        let (_, _, lin) = half.map((0.0, 0.0, 0.18), true);
        let (_, _, enc) = srgb.map((0.0, 0.0, 0.18), true);
        assert!((lin - 0.09).abs() < 1e-6);
        assert!((enc - srgb_decode(0.5 * srgb_encode(0.18))).abs() < 1e-6);
        // Encoding is ignored for 2.5D tables.
        let flat = HsvTable::new(&[2, 2, 1], &[0.0, 1.0, 1.0].repeat(4), 1).unwrap();
        assert!(!flat.srgb);
    }

    #[test]
    fn hdr_encoding_roundtrips() {
        // f maps [0, 16] onto [0, 1].
        assert!((hdr_encode(16.0) - 1.0).abs() < 1e-6);
        for x in [0.0f32, 0.01, 0.5, 1.0, 4.0, 15.0, 60.0] {
            let y = hdr_encode(x);
            assert!(y >= 0.0 && (y < 1.0 || x > 16.0));
            assert!((hdr_decode(y) - x).abs() <= 1e-3 * x.max(1.0), "{x} {y}");
        }
        // HDR profile with an identity 3D table keeps over-range values.
        let id = table(4, 2, 4, |_, _, _| [0.0, 1.0, 1.0]);
        let mut t = transform(Some(id), None, 1.0);
        t.hdr = true;
        let c = [3.0, 1.5, 0.25];
        assert!(close(t.apply(c), c, 1e-2), "{:?}", t.apply(c));
    }

    #[test]
    fn look_table_sees_exposed_value() {
        // LookTable value scale 1 below v = 0.5, 0 above (3 value divisions: 0, 0.5, 1).
        let look = table(2, 2, 3, |_, _, v| {
            [0.0, 1.0, if v == 2 { 0.0 } else { 1.0 }]
        });
        let grey = [0.25f32; 3];
        // Without gain the grey sits at v = 0.25: unchanged.
        let t = transform(None, Some(look.clone()), 1.0);
        assert!(close(t.apply(grey), grey, 1e-6));
        // With gain 4 it is indexed at v = 1 (scale 0), result divided back by the gain.
        let t = transform(None, Some(look), 4.0);
        assert!(close(t.apply(grey), [0.0; 3], 1e-6), "{:?}", t.apply(grey));
    }

    #[test]
    fn interpolates_tables_by_weight() {
        let a = table(2, 2, 1, |_, _, _| [10.0, 1.0, 1.0]);
        let b = table(2, 2, 1, |_, _, _| [20.0, 2.0, 1.0]);
        let m = HsvTable::lerp(&a, &b, 0.25).unwrap();
        assert_eq!(m.data[0], [17.5, 1.75, 1.0]);
        let c = table(3, 2, 1, |_, _, _| [0.0, 1.0, 1.0]);
        assert!(HsvTable::lerp(&a, &c, 0.5).is_none());
    }

    #[test]
    fn rejects_malformed_tables() {
        let ok = [0.0, 1.0, 1.0].repeat(4);
        assert!(HsvTable::new(&[2, 2, 1], &ok, 0).is_some());
        assert!(HsvTable::new(&[2, 2], &ok, 0).is_none());
        assert!(HsvTable::new(&[2, 1, 2], &ok, 0).is_none(), "sat < 2");
        assert!(HsvTable::new(&[0, 2, 2], &ok, 0).is_none());
        assert!(HsvTable::new(&[4, 2, 1], &ok, 0).is_none(), "count");
        assert!(HsvTable::new(&[2, 2, 1], &ok[1..], 0).is_none());
        assert!(HsvTable::new(&[2, 2, 1], &ok, 2).is_none(), "encoding");
        assert!(HsvTable::new(&[1 << 20, 2, 1], &ok, 0).is_none());
        assert!(HsvTable::new(&[u32::MAX, u32::MAX, u32::MAX], &ok, 0).is_none());
        let mut bad = ok.clone();
        bad[1] = -1.0;
        assert!(HsvTable::new(&[2, 2, 1], &bad, 0).is_none());
        bad[1] = f64::NAN;
        assert!(HsvTable::new(&[2, 2, 1], &bad, 0).is_none());
        bad[1] = f64::INFINITY;
        assert!(HsvTable::new(&[2, 2, 1], &bad, 0).is_none());
    }

    #[test]
    fn extreme_inputs_stay_finite() {
        let t = table(5, 3, 2, |h, s, _| {
            [h as f64 * 50.0 - 100.0, s as f64 * 2.0, 1.5]
        });
        let p = transform(Some(t.clone()), Some(t), 0.0625);
        for c in [
            [f32::MAX, 0.0, -1.0],
            [-5.0, -5.0, -5.0],
            [f32::NAN, 0.5, 0.5],
            [1e-30, 0.0, 0.0],
            [16.0, 16.0, 16.0],
        ] {
            let o = p.apply(c);
            assert!(
                o.iter().all(|v| v.is_finite() || c[0].is_nan()),
                "{c:?} {o:?}"
            );
        }
    }

    #[test]
    fn tone_curve_spline_and_validation() {
        let c = ToneCurve::new(&[0.0, 0.0, 0.5, 0.75, 1.0, 1.0]).unwrap();
        assert!((c.eval(0.5) - 0.75).abs() < 1e-3);
        assert!(c.eval(0.0).abs() < 1e-6 && (c.eval(1.0) - 1.0).abs() < 1e-6);
        let mut last = 0.0;
        for i in 0..=100 {
            let v = c.eval(i as f32 / 100.0);
            assert!(v >= last - 1e-6);
            last = v;
        }
        // Two points: linear.
        let l = ToneCurve::new(&[0.0, 0.2, 1.0, 1.0]).unwrap();
        assert!((l.eval(0.5) - 0.6).abs() < 1e-3);
        // Identity, odd counts, unsorted, out of range: none.
        assert!(ToneCurve::new(&[0.0, 0.0, 1.0, 1.0]).is_none());
        assert!(ToneCurve::new(&[0.0, 0.0, 1.0]).is_none());
        assert!(ToneCurve::new(&[0.0, 0.0, 0.6, 0.5, 0.5, 0.7, 1.0, 1.0]).is_none());
        assert!(ToneCurve::new(&[0.0, 0.0, 0.5, 1.5, 1.0, 1.0]).is_none());
        assert!(ToneCurve::new(&[0.0, 0.0, 0.5, f64::NAN, 1.0, 1.0]).is_none());
        assert!(ToneCurve::new(&[]).is_none());
    }

    #[test]
    fn tone_curve_preserves_hue() {
        let c = ToneCurve::new(&[0.0, 0.0, 0.25, 0.5, 1.0, 1.0]).unwrap();
        let p = [0.1, 0.4, 0.2];
        let o = c.apply(p);
        assert!((o[0] - c.eval(0.1)).abs() < 1e-6 && (o[1] - c.eval(0.4)).abs() < 1e-6);
        // The middle component keeps its relative place: (0.2 - 0.1) / (0.4 - 0.1) = 1/3.
        assert!(((o[2] - o[0]) / (o[1] - o[0]) - 1.0 / 3.0).abs() < 1e-5);
        let (h0, ..) = rgb_to_hsv(p);
        let (h1, ..) = rgb_to_hsv(o);
        assert!((h0 - h1).abs() < 1e-3, "{h0} {h1}");
        // Greys stay grey; NaN and out of range clamp.
        let g = c.apply([0.25; 3]);
        assert!(g.iter().all(|&v| (v - 0.5).abs() < 1e-3));
        assert_eq!(c.apply([f32::NAN, -1.0, 2.0]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn tone_curve_sees_exposed_value() {
        // Gain 2: grey 0.125 is exposed to 0.25, the curve lifts it to 0.5, and the gain is
        // divided back out (the viewer applies it after developing).
        let mut t = transform(None, None, 2.0);
        t.tone_curve = ToneCurve::new(&[0.0, 0.0, 0.25, 0.5, 1.0, 1.0]);
        let o = t.apply([0.125; 3]);
        assert!(o.iter().all(|&v| (v - 0.25).abs() < 2e-3), "{o:?}");
    }
}
