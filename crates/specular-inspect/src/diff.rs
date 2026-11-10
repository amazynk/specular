//! Difference view of compare (DESIGN.md §6.3): the per-pixel error, its colour maps and the
//! error metrics of the visible region (MAE, MSE, PSNR; SSIM and ΔE2000 in [`crate::quality`]).
//!
//! `specular-render` computes all of it on the GPU (`compare.wgsl`); this is the definition
//! the shader follows and the CPU reference its tests check it against.
//!
//! The pixels are the ones the viewer draws: linear light (BT.709 primaries, 1.0 = SDR
//! reference white), after any gain map and tone mapping for the display, premultiplied by
//! alpha, after channel isolation (a channel view compares the grey it shows). The errors are
//! taken in a [`MetricSpace`]: in linear light values above 1.0 (HDR highlights) count as
//! they are; encoded sRGB clips them to 1.0 first, as an 8-bit file would. PSNR takes 1.0 as
//! its peak either way.
//!
//! A pixel's error is the mean over the compared channels: red, green and blue, and alpha too
//! when either image has alpha. Premultiplied, colour hidden under full transparency does not
//! count; a difference in coverage does.

/// Which error the difference view maps to colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DiffKind {
    /// |a − b|.
    #[default]
    Absolute,
    /// (a − b)².
    Squared,
    /// |a − b| / (|a| + [`REL_EPS`]): the first image is the reference.
    Relative,
    /// a − b (the channel mean) through a diverging map: blue where the first image is
    /// darker, red where it is lighter ([`signed_error`]).
    Signed,
    /// |a − b| in false colour laid over the first image, dimmed to [`ON_IMAGE_DIM`].
    OnImage,
}

/// The space the errors and metrics are taken in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MetricSpace {
    /// Linear light as displayed (values above 1.0 count as they are).
    #[default]
    Linear,
    /// sRGB-encoded, clipped to 0‥1: what other tools measure on 8-bit files.
    Srgb,
}

impl MetricSpace {
    /// The shader's flag (`compare_error.wgsl`'s `to_space`).
    pub fn shader_flag(self) -> u32 {
        match self {
            MetricSpace::Linear => 0,
            MetricSpace::Srgb => 1,
        }
    }

    /// A premultiplied linear pixel in this space: sRGB encodes the colour channels (of the
    /// premultiplied value, so over black) and keeps alpha.
    pub fn convert(self, p: [f32; 4]) -> [f32; 4] {
        match self {
            MetricSpace::Linear => p,
            MetricSpace::Srgb => [
                crate::srgb_encode(p[0]),
                crate::srgb_encode(p[1]),
                crate::srgb_encode(p[2]),
                p[3],
            ],
        }
    }
}

impl DiffKind {
    /// The shader's mode number (`compare.wgsl`).
    pub fn shader_mode(self) -> u32 {
        match self {
            DiffKind::Absolute => 1,
            DiffKind::Squared => 2,
            DiffKind::Relative => 3,
            DiffKind::Signed => 4,
            DiffKind::OnImage => 5,
        }
    }

    /// Every kind, in the `d` cycle's order.
    pub const ALL: [DiffKind; 5] = [
        DiffKind::Absolute,
        DiffKind::Squared,
        DiffKind::Relative,
        DiffKind::Signed,
        DiffKind::OnImage,
    ];

    /// The next kind in the `d` cycle (`None` after the last: the difference view closes).
    pub fn next(self) -> Option<DiffKind> {
        match self {
            DiffKind::Absolute => Some(DiffKind::Squared),
            DiffKind::Squared => Some(DiffKind::Relative),
            DiffKind::Relative => Some(DiffKind::Signed),
            DiffKind::Signed => Some(DiffKind::OnImage),
            DiffKind::OnImage => None,
        }
    }

    /// The error that maps to the top of the colour ramp at scale exponent `k`
    /// ([`DEFAULT_SCALE`]): 2⁻ᵏ for the absolute error, its square for the squared error, and
    /// 2³⁻ᵏ for the relative error (relative errors run larger). The signed error and the
    /// error over the image use the absolute error's.
    pub fn full_scale(self, k: i32) -> f32 {
        let a = (-(k as f32)).exp2();
        match self {
            DiffKind::Absolute | DiffKind::Signed | DiffKind::OnImage => a,
            DiffKind::Squared => a * a,
            DiffKind::Relative => a * 8.0,
        }
    }

    /// The gain the bar shows ("Gain ×16"): how much the error is amplified before it is
    /// mapped (1 / [`DiffKind::full_scale`]).
    pub fn gain(self, k: i32) -> f32 {
        1.0 / self.full_scale(k)
    }

    /// The error this kind maps, of per-pixel error `e` ([`pixel_error`]).
    pub fn value(self, e: PixelError) -> f32 {
        match self {
            DiffKind::Absolute | DiffKind::OnImage => e.abs,
            DiffKind::Squared => e.sq,
            DiffKind::Relative => e.rel,
            // Not in `PixelError` (it has no sign): see `signed_error`.
            DiffKind::Signed => e.abs,
        }
    }
}

/// How bright the first image stays under [`DiffKind::OnImage`] (linear light).
pub const ON_IMAGE_DIM: f32 = 0.25;

/// The relative error's guard against division by (near) zero, as in tev's.
pub const REL_EPS: f32 = 0.01;
/// The default scale exponent: an absolute error of 1/16 maps to the top of the ramp.
pub const DEFAULT_SCALE: i32 = 4;
/// The scale exponent's range (`[` and `]` step it).
pub const SCALE_RANGE: (i32, i32) = (0, 12);

/// One pixel's error, each the mean over the compared channels, and the largest absolute
/// channel difference.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PixelError {
    pub abs: f32,
    pub sq: f32,
    pub rel: f32,
    pub max: f32,
}

/// The error between premultiplied linear pixels `a` (the reference) and `b`; `alpha`:
/// alpha is a compared channel (either image has alpha). The shader's `pixel_error`.
pub fn pixel_error(a: [f32; 4], b: [f32; 4], alpha: bool) -> PixelError {
    let n = if alpha { 4 } else { 3 };
    let mut e = PixelError::default();
    for c in 0..n {
        let d = (a[c] - b[c]).abs();
        e.abs += d;
        e.sq += d * d;
        e.rel += d / (a[c].abs() + REL_EPS);
        e.max = e.max.max(d);
    }
    let k = 1.0 / n as f32;
    e.abs *= k;
    e.sq *= k;
    e.rel *= k;
    e
}

/// [`pixel_error`] of `a` and `b` taken in `space`.
pub fn pixel_error_in(space: MetricSpace, a: [f32; 4], b: [f32; 4], alpha: bool) -> PixelError {
    pixel_error(space.convert(a), space.convert(b), alpha)
}

/// The signed error a − b, the mean over the compared channels (the shader's
/// `signed_error`); positive where the first image is lighter.
pub fn signed_error(a: [f32; 4], b: [f32; 4], alpha: bool) -> f32 {
    let n = if alpha { 4 } else { 3 };
    (0..n).map(|c| a[c] - b[c]).sum::<f32>() / n as f32
}

/// The colour the difference view draws for pixels `a` and `b` (premultiplied linear, as
/// rendered) under `kind`, in linear light with alpha 1: `fs_diff` in `compare.wgsl`.
/// `gain` is [`DiffKind::gain`].
pub fn diff_colour(
    kind: DiffKind,
    space: MetricSpace,
    a: [f32; 4],
    b: [f32; 4],
    alpha: bool,
    gain: f32,
) -> [f32; 4] {
    let (sa, sb) = (space.convert(a), space.convert(b));
    let e = pixel_error(sa, sb, alpha);
    let lin = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
    let ramp = |v: f32| false_colour(v).map(crate::srgb_decode);
    match kind {
        DiffKind::Signed => {
            let t = 0.5 + 0.5 * (signed_error(sa, sb, alpha) * gain).clamp(-1.0, 1.0);
            // Vik runs blue (low) to red (high): A darker than B reads blue.
            lin(crate::ColourMap::Vik.sample_linear(t))
        }
        DiffKind::OnImage => {
            let v = (e.abs * gain).clamp(0.0, 1.0);
            let r = ramp(v);
            let base = [a[0], a[1], a[2]].map(|c| c.max(0.0) * ON_IMAGE_DIM * (1.0 - v));
            lin([0, 1, 2].map(|c| base[c] + r[c] * v))
        }
        k => lin(ramp(k.value(e) * gain)),
    }
}

/// Sums of [`PixelError`] over a region, as the GPU reduction produces them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DiffSums {
    pub abs: f64,
    pub sq: f64,
    pub rel: f64,
    pub max: f32,
    pub pixels: u64,
}

impl DiffSums {
    pub fn add(&mut self, e: PixelError) {
        self.abs += e.abs as f64;
        self.sq += e.sq as f64;
        self.rel += e.rel as f64;
        self.max = self.max.max(e.max);
        self.pixels += 1;
    }

    /// The sums the GPU reads back (absolute, squared, relative, max) over `pixels` pixels.
    pub fn from_gpu(v: [f32; 4], pixels: u64) -> DiffSums {
        DiffSums {
            abs: v[0] as f64,
            sq: v[1] as f64,
            rel: v[2] as f64,
            max: v[3],
            pixels,
        }
    }

    pub fn stats(&self) -> DiffStats {
        let n = self.pixels.max(1) as f64;
        DiffStats {
            pixels: self.pixels,
            mae: self.abs / n,
            mse: self.sq / n,
            mre: self.rel / n,
            max: self.max,
        }
    }
}

/// Error metrics of a region.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DiffStats {
    /// Pixels compared (0: nothing overlapped in view).
    pub pixels: u64,
    /// Mean absolute error.
    pub mae: f64,
    /// Mean squared error.
    pub mse: f64,
    /// Mean relative error ([`DiffKind::Relative`]).
    pub mre: f64,
    /// Largest absolute channel difference.
    pub max: f32,
}

impl DiffStats {
    /// Peak signal-to-noise ratio in dB with a peak of 1.0 (SDR reference white); infinite
    /// for identical images.
    pub fn psnr(&self) -> f64 {
        if self.mse <= 0.0 {
            f64::INFINITY
        } else {
            -10.0 * self.mse.log10()
        }
    }

    /// PSNR for display: "∞" for identical images, else one decimal.
    pub fn psnr_text(&self) -> String {
        let p = self.psnr();
        if p.is_infinite() {
            "∞".into()
        } else {
            format!("{p:.1}")
        }
    }
}

/// The CPU reference: metrics of premultiplied linear pixels `a` against `b` (same length).
pub fn reference(a: &[[f32; 4]], b: &[[f32; 4]], alpha: bool) -> DiffStats {
    reference_in(MetricSpace::Linear, a, b, alpha)
}

/// [`reference`] in `space`.
pub fn reference_in(space: MetricSpace, a: &[[f32; 4]], b: &[[f32; 4]], alpha: bool) -> DiffStats {
    let mut s = DiffSums::default();
    for (pa, pb) in a.iter().zip(b) {
        s.add(pixel_error_in(space, *pa, *pb, alpha));
    }
    s.stats()
}

/// A metric value for display: three significant digits, scientific below 0.001.
pub fn metric_text(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else if v.abs() < 1e-3 {
        format!("{v:.2e}")
    } else {
        let digits = (2 - v.abs().log10().floor() as i32).clamp(0, 6) as usize;
        format!("{v:.digits$}")
    }
}

/// The false-colour ramp's stops (sRGB-encoded), evenly spaced from 0 to 1: black, deep blue,
/// magenta, orange, pale yellow. Rising in lightness, so it reads in greyscale too.
pub const RAMP: [[f32; 3]; 5] = [
    [0.0, 0.0, 0.0],
    [0.10, 0.12, 0.62],
    [0.75, 0.15, 0.55],
    [1.0, 0.55, 0.10],
    [1.0, 1.0, 0.70],
];

/// The false colour of `v` (0‥1, clamped), sRGB-encoded; the shader's `ramp` (which
/// interpolates the same stops, then decodes to linear).
pub fn false_colour(v: f32) -> [f32; 3] {
    let v = v.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32;
    let i = (v.floor() as usize).min(RAMP.len() - 2);
    let t = v - i as f32;
    let (a, b) = (RAMP[i], RAMP[i + 1]);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{luma, srgb_decode};

    #[test]
    fn pixel_error_is_the_channel_mean() {
        let a = [0.5, 0.25, 1.0, 1.0];
        let b = [0.25, 0.25, 0.5, 0.5];
        // Colour only: (0.25 + 0 + 0.5) / 3.
        let e = pixel_error(a, b, false);
        assert!((e.abs - 0.25).abs() < 1e-6);
        assert!((e.sq - (0.0625 + 0.25) / 3.0).abs() < 1e-6);
        let rel = (0.25 / 0.51 + 0.0 + 0.5 / 1.01) / 3.0;
        assert!((e.rel - rel).abs() < 1e-6);
        assert_eq!(e.max, 0.5);
        // With alpha, the coverage difference counts as a fourth channel.
        let e = pixel_error(a, b, true);
        assert!((e.abs - 1.25 / 4.0).abs() < 1e-6);
        // Identical pixels: nothing.
        assert_eq!(pixel_error(a, a, true), PixelError::default());
    }

    #[test]
    fn metrics_follow_their_definitions() {
        // Identical: zero error, infinite PSNR.
        let a: Vec<[f32; 4]> = (0..64).map(|i| [i as f32 / 64.0, 0.5, 0.25, 1.0]).collect();
        let s = reference(&a, &a, false);
        assert_eq!((s.pixels, s.mae, s.mse), (64, 0.0, 0.0));
        assert!(s.psnr().is_infinite());
        assert_eq!(s.psnr_text(), "∞");
        // Every channel off by 0.1: MAE 0.1, MSE 0.01, PSNR 20 dB.
        let b: Vec<[f32; 4]> = a
            .iter()
            .map(|p| [p[0] + 0.1, p[1] - 0.1, p[2] + 0.1, 1.0])
            .collect();
        let s = reference(&a, &b, false);
        assert!((s.mae - 0.1).abs() < 1e-6, "{}", s.mae);
        assert!((s.mse - 0.01).abs() < 1e-6, "{}", s.mse);
        assert!((s.psnr() - 20.0).abs() < 1e-3, "{}", s.psnr());
        assert!((s.max - 0.1).abs() < 1e-6);
        // Sums from the GPU give the same statistics as adding up here.
        let mut sums = DiffSums::default();
        for (pa, pb) in a.iter().zip(&b) {
            sums.add(pixel_error(*pa, *pb, false));
        }
        let g = DiffSums::from_gpu(
            [sums.abs as f32, sums.sq as f32, sums.rel as f32, sums.max],
            64,
        );
        assert!((g.stats().mse - s.mse).abs() < 1e-7);
        // An HDR highlight counts as it is (no clamp at 1.0).
        let s = reference(&[[4.0, 4.0, 4.0, 1.0]], &[[2.0, 2.0, 2.0, 1.0]], false);
        assert_eq!((s.mae, s.mse), (2.0, 4.0));
        assert!(s.psnr() < 0.0);
    }

    #[test]
    fn diff_scale_and_ramp() {
        assert_eq!(DiffKind::Absolute.full_scale(DEFAULT_SCALE), 1.0 / 16.0);
        assert_eq!(DiffKind::Squared.full_scale(DEFAULT_SCALE), 1.0 / 256.0);
        assert_eq!(DiffKind::Relative.full_scale(DEFAULT_SCALE), 0.5);
        assert_eq!(DiffKind::Absolute.next(), Some(DiffKind::Squared));
        assert_eq!(DiffKind::Relative.next(), Some(DiffKind::Signed));
        assert_eq!(DiffKind::OnImage.next(), None);
        assert_eq!(DiffKind::Absolute.gain(DEFAULT_SCALE), 16.0);
        assert_eq!(
            DiffKind::Signed.full_scale(3),
            DiffKind::Absolute.full_scale(3)
        );
        // The cycle visits every kind once, each with its own shader mode.
        let mut k = Some(DiffKind::Absolute);
        let mut seen = Vec::new();
        while let Some(x) = k {
            seen.push(x);
            k = x.next();
        }
        assert_eq!(seen, DiffKind::ALL);
        let modes: std::collections::HashSet<u32> = seen.iter().map(|k| k.shader_mode()).collect();
        assert_eq!(modes.len(), 5);
        // The ramp starts black, ends at its last stop, and rises in lightness.
        assert_eq!(false_colour(-1.0), [0.0; 3]);
        assert_eq!(false_colour(2.0), RAMP[4]);
        let mut last = -1.0;
        for i in 0..=100 {
            let c = false_colour(i as f32 / 100.0).map(srgb_decode);
            let l = luma(c);
            assert!(l >= last - 1e-6, "{i}: {l} < {last}");
            last = l;
        }
        assert_eq!(metric_text(0.0), "0");
        assert_eq!(metric_text(0.01234), "0.0123");
        assert_eq!(metric_text(12.345), "12.3");
        assert_eq!(metric_text(0.000_012), "1.20e-5");
    }

    #[test]
    fn metric_space_encodes_colour_and_keeps_alpha() {
        let p = [0.214_041, 1.0, 4.0, 0.5];
        assert_eq!(MetricSpace::Linear.convert(p), p);
        let q = MetricSpace::Srgb.convert(p);
        assert!((q[0] - 0.5).abs() < 1e-4, "{}", q[0]);
        // HDR is clipped to white; alpha stays linear coverage.
        assert!(
            (q[1] - 1.0).abs() < 1e-6 && q[2] == q[1] && q[3] == 0.5,
            "{q:?}"
        );
        // An 8-bit step is 1/255 in sRGB, whatever the level.
        let a = [srgb_decode(100.0 / 255.0); 4];
        let b = [srgb_decode(101.0 / 255.0); 4];
        let e = pixel_error_in(MetricSpace::Srgb, a, b, false);
        assert!((e.abs - 1.0 / 255.0).abs() < 1e-5, "{}", e.abs);
        assert!(pixel_error_in(MetricSpace::Linear, a, b, false).abs < e.abs);
        let s = reference_in(MetricSpace::Srgb, &[a], &[b], false);
        assert!(
            (s.psnr() - 20.0 * 255f64.log10()).abs() < 0.01,
            "{}",
            s.psnr()
        );
    }

    #[test]
    fn diff_colours_follow_their_kinds() {
        let a = [0.5, 0.5, 0.5, 1.0];
        let b = [0.25, 0.25, 0.25, 1.0];
        let gain = 4.0;
        // The ramp kinds: the error times the gain on the false-colour ramp.
        let c = diff_colour(DiffKind::Absolute, MetricSpace::Linear, a, b, false, gain);
        assert_eq!(c, {
            let r = false_colour(1.0).map(srgb_decode);
            [r[0], r[1], r[2], 1.0]
        });
        // Signed: no difference is the map's middle; A lighter is its red end, darker blue.
        let mid = diff_colour(DiffKind::Signed, MetricSpace::Linear, a, a, false, gain);
        let m = crate::ColourMap::Vik.sample_linear(0.5);
        assert_eq!(mid, [m[0], m[1], m[2], 1.0]);
        let hi = diff_colour(DiffKind::Signed, MetricSpace::Linear, a, b, false, gain);
        let lo = diff_colour(DiffKind::Signed, MetricSpace::Linear, b, a, false, gain);
        assert!(hi[0] > hi[2] && lo[2] > lo[0], "{hi:?} {lo:?}");
        assert!((signed_error(a, b, false) - 0.25).abs() < 1e-6);
        // On the image: the first image dimmed where nothing differs, the ramp where it does.
        let same = diff_colour(DiffKind::OnImage, MetricSpace::Linear, a, a, false, gain);
        assert_eq!(
            same,
            [
                0.5 * ON_IMAGE_DIM,
                0.5 * ON_IMAGE_DIM,
                0.5 * ON_IMAGE_DIM,
                1.0
            ]
        );
        let full = diff_colour(DiffKind::OnImage, MetricSpace::Linear, a, b, false, gain);
        assert_eq!(full, c);
    }
}
