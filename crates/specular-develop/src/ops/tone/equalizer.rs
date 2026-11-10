//! The tone equalizer: a gain in EV by the luminance of a smooth mask,
//! so large areas move while local contrast survives.
//!
//! - **Mask.** The `log2` luminance `I`, smoothed by the guided filter guided by itself
//!   (He, Sun and Tang 2013) in its fast form (He and Sun 2015): the coefficients `a`, `b`
//!   are taken on the whole-image statistics grid, further subsampled by s = r/4 for
//!   large radii as the fast form does, and each pixel's mask is `Ā·I + B̄` with Ā, B̄
//!   sampled bilinearly and `I` the pixel's own. Edges stronger than `mask_edges`
//!   (a variance in EV²) stay sharp in the mask, so the gain does not halo across them.
//!   `smoothing` is the filter's radius as a fraction of the image's long side, so the
//!   mask is the same at every view size.
//! - **Mask exposure and contrast** move and stretch the mask about mid grey;
//!   `mask_auto` first fits the mask's 5th–95th percentiles to −7.5…−0.5 EV.
//! - **Gain.** The Basic panel's `shadows_ev` and `highlights_ev` (their fields and
//!   weights: full at 4 EV under mid grey, and from grey to full at 3 EV over), plus nine
//!   zones `zones` at −8…0 EV of mask luminance, joined by a Catmull–Rom spline and flat
//!   beyond the ends. All zero is the identity.

use super::kernels::{self, Alloc, BINS, fw, threads};
use crate::graph::{NodeKind, Op, Program};
use crate::model::{Fields, Params, Value};

/// Mask luminance of the zones, in EV (log2 of linear luminance).
pub const ZONES: [f32; 9] = [-8.0, -7.0, -6.0, -5.0, -4.0, -3.0, -2.0, -1.0, 0.0];

/// The longest side [`ToneEq::mask_percentiles`] measures at.
pub const PERCENTILE_LONG: u32 = 512;

/// `mask_auto`'s fit of mask value `m` (EV): percentiles `lo`, `hi` to −7.5 … −0.5 EV (the
/// kernel's arithmetic).
pub fn auto_fit(m: f32, lo: f32, hi: f32) -> f32 {
    (m - lo) * 7.0 / (hi - lo).max(0.5) - 7.5
}

/// −log2(0.18): mask exposure and contrast move and stretch the mask about mid grey.
const MID_GREY_EV: f32 = 2.473_931_2;

/// The tone equalizer's params (fields of [`crate::model::ModuleKind::TONE_EQUALIZER`],
/// version 1; missing fields take these defaults).
#[derive(Clone, Debug, PartialEq)]
pub struct ToneEq {
    pub shadows_ev: f32,
    pub highlights_ev: f32,
    pub zones: [f32; 9],
    /// Guided filter radius, fraction of the long side.
    pub smoothing: f32,
    /// Guided filter ε: edges with more variance than this (EV²) stay in the mask.
    pub mask_edges: f32,
    pub mask_exposure: f32,
    pub mask_contrast: f32,
    pub mask_auto: bool,
}

impl Default for ToneEq {
    fn default() -> Self {
        Self {
            shadows_ev: 0.0,
            highlights_ev: 0.0,
            zones: [0.0; 9],
            smoothing: 0.025,
            mask_edges: 0.1,
            mask_exposure: 0.0,
            mask_contrast: 1.0,
            mask_auto: false,
        }
    }
}

fn finite(v: f32, d: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

impl ToneEq {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let zones = match f.get("zones") {
            Some(Value::Floats(v)) if v.len() == 9 => {
                std::array::from_fn(|i| finite(v[i], 0.0, -8.0, 8.0))
            }
            _ => d.zones,
        };
        Self {
            shadows_ev: finite(f.f32_or("shadows_ev", 0.0), 0.0, -8.0, 8.0),
            highlights_ev: finite(f.f32_or("highlights_ev", 0.0), 0.0, -8.0, 8.0),
            zones,
            smoothing: finite(f.f32_or("smoothing", d.smoothing), d.smoothing, 0.002, 0.25),
            mask_edges: finite(
                f.f32_or("mask_edges", d.mask_edges),
                d.mask_edges,
                1e-4,
                16.0,
            ),
            mask_exposure: finite(f.f32_or("mask_exposure", 0.0), 0.0, -8.0, 8.0),
            mask_contrast: finite(f.f32_or("mask_contrast", 1.0), 1.0, 0.1, 10.0),
            mask_auto: f.bool_or("mask_auto", false),
        }
    }

    /// The kernel's uniforms: shadows, highlights, nine zones, mask exposure, mask contrast,
    /// mask auto.
    pub fn uniforms(&self) -> [f32; 14] {
        let mut u = [0f32; 14];
        u[0] = self.shadows_ev;
        u[1] = self.highlights_ev;
        u[2..11].copy_from_slice(&self.zones);
        u[11] = self.mask_exposure;
        u[12] = self.mask_contrast;
        u[13] = if self.mask_auto { 1.0 } else { 0.0 };
        u
    }

    /// The zone (index into [`ZONES`]) a pixel of linear working-space colour `rgb` falls
    /// in, through `mask_auto`'s fit (`percentiles`: the mask's 5th and 95th, EV, from
    /// [`ToneEq::mask_percentiles`]; ignored when `mask_auto` is off) and mask exposure and
    /// contrast, as the kernel maps the mask. The mask's own smoothing is left out (a pixel
    /// stands for its neighbourhood).
    pub fn zone_of(&self, rgb: [f32; 3], percentiles: Option<[f32; 2]>) -> usize {
        let y = rgb[0] * super::LUMA[0] + rgb[1] * super::LUMA[1] + rgb[2] * super::LUMA[2];
        let mut l = if y.is_finite() {
            y.max(1.525_878_9e-5).log2()
        } else {
            0.0
        };
        if self.mask_auto
            && let Some([lo, hi]) = percentiles
        {
            l = auto_fit(l, lo, hi);
        }
        let m = (l + MID_GREY_EV) * self.mask_contrast - MID_GREY_EV + self.mask_exposure;
        (m - ZONES[0]).round().clamp(0.0, 8.0) as usize
    }

    /// The 5th and 95th percentiles (EV) of the smoothed mask of a `w`×`h` linear RGBA
    /// picture (the input of the module), as `mask_auto` takes them: the statistics program
    /// on the CPU, the picture first reduced to at most [`PERCENTILE_LONG`] px (the
    /// smoothing is a fraction of the long side, so the mask keeps its shape).
    pub fn mask_percentiles(&self, w: u32, h: u32, rgba: &[f32]) -> Option<[f32; 2]> {
        if w == 0 || h == 0 || rgba.len() < (w as usize) * (h as usize) * 4 {
            return None;
        }
        let f = w.max(h).div_ceil(PERCENTILE_LONG).max(1);
        let (sw, sh) = (w.div_ceil(f), h.div_ceil(f));
        let mut small = vec![0f32; (sw * sh * 4) as usize];
        for y in 0..sh {
            for x in 0..sw {
                let mut acc = [0f32; 4];
                let mut n = 0.0;
                for yy in y * f..((y + 1) * f).min(h) {
                    for xx in x * f..((x + 1) * f).min(w) {
                        let i = ((yy * w + xx) * 4) as usize;
                        for k in 0..4 {
                            acc[k] += rgba[i + k];
                        }
                        n += 1.0;
                    }
                }
                let o = ((y * sw + x) * 4) as usize;
                for k in 0..4 {
                    small[o + k] = acc[k] / n;
                }
            }
        }
        let prog = stats_program(self, sw, sh);
        let st = super::cpu::run_program(
            &prog,
            super::cpu::Img {
                w: sw,
                h: sh,
                px: &small,
            },
            &[],
            crate::graph::Roi::full(sw, sh),
        );
        let (lo, hi) = (*st.get(4)?, *st.get(5)?);
        (lo.is_finite() && hi.is_finite()).then_some([lo, hi])
    }

    /// The box radius on a `w`×`h` grid.
    pub fn radius(&self, w: u32, h: u32) -> u32 {
        ((self.smoothing * w.max(h) as f32).round() as u32).clamp(1, 256)
    }

    /// The fast guided filter's subsampling of the grid and its radius there: none up to a
    /// radius of 8, then s = r/4 (He and Sun 2015), at most 8.
    pub fn subsample(&self, w: u32, h: u32) -> (u32, u32) {
        let r = self.radius(w, h);
        if r <= 8 {
            return (1, r);
        }
        let s = (r / 4).clamp(1, 8);
        (s, ((r as f32 / s as f32).round() as u32).max(1))
    }
}

/// Statistics layout: words 0–1 the size of the coefficient grid, 4–5 the mask percentiles
/// (auto), Ā from word 16, B̄ after it.
pub const STATS_A: u32 = 16;

/// The guided tone equalizer, param version 1.
pub struct ToneEqOp;

/// `log2(0.18)`: mid grey.
pub const GREY_EV: f32 = -2.473_931_2;

const TONE_WGSL: &str = "fn dv_smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}
fn dv_zones(p: u32, m: f32) -> f32 {
    let u = clamp(m + 8.0, 0.0, 8.0);
    let i = min(u32(floor(u)), 7u);
    let f = u - f32(i);
    let g0 = pf(p + select(i - 1u, 0u, i == 0u));
    let g1 = pf(p + i);
    let g2 = pf(p + i + 1u);
    let g3 = pf(p + min(i + 2u, 8u));
    return 0.5 * (2.0 * g1 + (g2 - g0) * f + (2.0 * g0 - 5.0 * g1 + 4.0 * g2 - g3) * f * f
        + (3.0 * g1 - g0 - 3.0 * g2 + g3) * f * f * f);
}
";

impl Op for ToneEqOp {
    fn name(&self) -> &'static str {
        "tone_eq_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Global
    }
    fn param_words(&self) -> u32 {
        14
    }
    fn write_params(&self, p: &Params, _: f32, out: &mut [u32]) {
        crate::ops::put(out, &ToneEq::from_fields(&p.to_fields()).uniforms());
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[kernels::COMMON_WGSL, kernels::GRID_WGSL, TONE_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        let te = ToneEq::from_fields(&p.to_fields());
        te.shadows_ev == 0.0 && te.highlights_ev == 0.0 && te.zones.iter().all(|z| *z == 0.0)
    }
    fn has_stats(&self, _: &Params) -> bool {
        true
    }
    fn stats(&self, p: &Params, w: u32, h: u32, _: f32) -> Option<Program> {
        let te = ToneEq::from_fields(&p.to_fields());
        Some(stats_program(&te, w, h))
    }
    fn wgsl(&self) -> &'static str {
        "    let gw = u32(st(0u));
    let n = gw * u32(st(1u));
    let y = dot(c.rgb, DV_LUMA);
    let l = log2(max(y, 1.52587890625e-5));
    var m = dv_grid(16u, px, py) * l + dv_grid(16u + n, px, py);
    if (pf(p + 13u) != 0.0) {
        let lo = st(4u);
        m = (m - lo) * 7.0 / max(st(5u) - lo, 0.5) - 7.5;
    }
    m = (m + 2.4739312) * pf(p + 12u) - 2.4739312 + pf(p + 11u);
    let d = m + 2.4739312;
    let ws = 1.0 - dv_smooth(-4.0, 0.0, d);
    let wh = dv_smooth(0.0, 3.0, d);
    let g = pf(p) * ws + pf(p + 1u) * wh + dv_zones(p + 2u, m);
    return vec4<f32>(c.rgb * exp2(g), c.a);"
    }
}

/// The statistics program on a `w`×`h` grid.
pub fn stats_program(te: &ToneEq, w: u32, h: u32) -> Program {
    let (s, r) = te.subsample(w, h);
    let (cw, ch) = (w.div_ceil(s), h.div_ceil(s));
    let n = u64::from(cw) * u64::from(ch);
    let mut a = Alloc(u64::from(STATS_A));
    let ab = a.take(2 * n);
    let full = a.take(u64::from(w) * u64::from(h));
    let i = if s > 1 { a.take(n) } else { full };
    let hist = a.take(u64::from(BINS));
    let mut prog = Program::default();
    prog.step(&kernels::PUT, [2, 1], vec![0, fw(cw as f32), fw(ch as f32)]);
    prog.step(&kernels::LUM, [w, h], vec![full, w, 0, 0]);
    if s > 1 {
        prog.step(&kernels::DOWN, [cw, ch], vec![full, w, h, i, cw, s]);
    }
    let (w, h) = (cw, ch);
    kernels::guided(
        &mut prog,
        &mut a,
        i,
        i,
        false,
        ab,
        (w, h),
        r,
        te.mask_edges,
        true,
    );
    let auto = te.mask_auto;
    prog.step(&kernels::HIST_CLEAR, threads(auto, BINS, 1), vec![hist]);
    kernels::hist_add(
        &mut prog,
        auto,
        (w, h),
        [2, ab, ab + n as u32, i, w, hist, fw(-20.0), fw(12.0), 0],
    );
    kernels::hist_pct(
        &mut prog,
        auto,
        vec![hist, fw(-20.0), fw(12.0), 4, 2, fw(0.05), fw(0.95)],
    );
    prog.words = a.0;
    prog
}

#[cfg(test)]
mod zone_tests {
    use super::*;

    #[test]
    fn a_pixel_falls_in_the_zone_of_its_luminance() {
        let t = ToneEq::default();
        for (i, z) in ZONES.iter().enumerate() {
            let y = z.exp2();
            assert_eq!(t.zone_of([y, y, y], None), i, "zone {z} EV");
        }
        // Beyond the ends: the end zones.
        assert_eq!(t.zone_of([0.0; 3], None), 0);
        assert_eq!(t.zone_of([16.0; 3], None), 8);
        assert_eq!(t.zone_of([f32::NAN; 3], None), 8);
        // Mask exposure moves the mask: +1 EV puts a −3 EV pixel in the −2 EV zone.
        let t = ToneEq {
            mask_exposure: 1.0,
            ..ToneEq::default()
        };
        let y = (-3f32).exp2();
        assert_eq!(t.zone_of([y, y, y], None), 6);
    }

    /// A picture whose luminance runs evenly over −12 … 0 EV (left to right).
    fn ramp(w: u32, h: u32) -> Vec<f32> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..h {
            for x in 0..w {
                let y = (-12.0 + 12.0 * (x as f32 + 0.5) / w as f32).exp2();
                v.extend_from_slice(&[y, y, y, 1.0]);
            }
        }
        v
    }

    #[test]
    fn mask_auto_moves_the_picked_zone_as_the_kernel_does() {
        let t = ToneEq {
            mask_auto: true,
            smoothing: 0.01,
            ..ToneEq::default()
        };
        let (w, h) = (1024, 256);
        let px = ramp(w, h);
        // Reduced to 512 px wide for the percentiles; the kernel's own at full size.
        let pct = t.mask_percentiles(w, h, &px).expect("percentiles");
        let full = super::super::cpu::run_program(
            &stats_program(&t, w, h),
            super::super::cpu::Img { w, h, px: &px },
            &[],
            crate::graph::Roi::full(w, h),
        );
        eprintln!(
            "mask percentiles reduced {:.3} / {:.3} EV, full size {:.3} / {:.3} EV",
            pct[0], pct[1], full[4], full[5]
        );
        assert!((pct[0] - full[4]).abs() < 0.1 && (pct[1] - full[5]).abs() < 0.1);
        // Evenly spread: the 5th and 95th percentiles near −11.4 and −0.6 EV.
        assert!(
            (pct[0] + 11.4).abs() < 0.3 && (pct[1] + 0.6).abs() < 0.3,
            "{pct:?}"
        );
        // −6 EV: zone −6 EV without the fit, about −4 EV with it.
        let y = (-6f32).exp2();
        let plain = ToneEq {
            mask_auto: false,
            ..t.clone()
        };
        assert_eq!(plain.zone_of([y, y, y], Some(pct)), 2);
        assert_eq!(t.zone_of([y, y, y], None), 2);
        assert_eq!(t.zone_of([y, y, y], Some(pct)), 4);
        // Every zone the fit gives is the kernel's mapping rounded.
        for i in 0..w {
            let k = (i * 4) as usize;
            let rgb = [px[k], px[k + 1], px[k + 2]];
            let l = rgb[1].log2();
            let m = auto_fit(l, pct[0], pct[1]);
            let want = (m + 8.0).round().clamp(0.0, 8.0) as usize;
            assert_eq!(t.zone_of(rgb, Some(pct)), want);
        }
    }
}
