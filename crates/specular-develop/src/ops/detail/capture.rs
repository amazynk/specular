//! Capture sharpening: Richardson–Lucy deconvolution
//! (Richardson 1972, Lucy 1974) of the luminance with a Gaussian point-spread function,
//! after denoise in sensor geometry.
//!
//! Each iteration `u ← u · (P ⊛ (d / (P ⊛ u)))` (the Gaussian is symmetric, so `P* = P`)
//! runs as four separable passes. Two guards keep it from ringing:
//!
//! - the ratio `d / (P ⊛ u)` is held within [1/(1+c), 1+c] (damping), so no pixel moves
//!   fast;
//! - the estimate is held within the local range of the observed luminance over the PSF's
//!   support, widened by a margin of that range ([`MARGIN`]): a step edge cannot overshoot
//!   by more than the margin, however many iterations run.
//!
//! The colour channels are scaled by the luminance's change, so hue and saturation stay.
//!
//! **Radius.** The PSF's σ in source pixels: the slider's, or with the slider at 0 the
//! lens's ([`auto_sigma`]: diffraction at the aperture combined with a sensor baseline),
//! set per image by the caller ([`super::with_image`]). At view scale `s` the PSF is
//! `σ·s`; below a quarter pixel there is nothing to undo and the passes do no work.
//!
//! Amount 0 is the identity (the node is left out).

use super::kernels::{BLUR, LUM, MINMAX, fw, gauss_taps};
use crate::graph::{NodeKind, Op, Program, Roi};
use crate::model::{Fields, Params, Value};

/// Iterations at full size.
pub const ITERATIONS: u32 = 10;
/// How far past the local range the estimate may go, as a share of that range.
pub const MARGIN: f32 = 0.05;
/// Ratio damping c: each iteration changes a pixel by at most a factor 1 + c.
const DAMPING: f32 = 0.5;
/// σ used when neither the slider nor the lens gives one (source pixels).
pub const DEFAULT_SIGMA: f32 = 0.7;
/// The smallest PSF worth deconvolving, in view pixels.
const MIN_VIEW_SIGMA: f32 = 0.25;
/// The luminance floor (RL needs positive values).
const FLOOR: f32 = 1e-6;

/// Capture sharpening params (fields of
/// [`crate::model::ModuleKind::CAPTURE_SHARPENING`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    /// Blend of the deconvolved luminance, 0 … 1.
    pub amount: f32,
    /// PSF σ in source pixels; 0 = from the lens.
    pub radius: f32,
    /// The lens's σ (set per image by the caller).
    pub auto_sigma: f32,
}

impl Default for Capture {
    fn default() -> Self {
        Self {
            amount: 0.0,
            radius: 0.0,
            auto_sigma: DEFAULT_SIGMA,
        }
    }
}

impl Capture {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let get = |n: &str, dv: f32, lo: f32, hi: f32| {
            let v = f.f32_or(n, dv);
            if v.is_finite() { v.clamp(lo, hi) } else { dv }
        };
        Self {
            amount: get("amount", d.amount, 0.0, 1.0),
            radius: get("radius", d.radius, 0.0, 3.0),
            auto_sigma: get("auto_sigma", d.auto_sigma, 0.3, 3.0),
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("radius", Value::Float(d.radius))
    }

    fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }

    /// The PSF σ in source pixels.
    pub fn sigma(&self) -> f32 {
        if self.radius > 0.0 {
            self.radius.max(0.3)
        } else {
            self.auto_sigma
        }
    }
}

/// The PSF σ in source pixels for aperture `f_number` on a sensor with `pitch_um` pixels:
/// the Airy disc's Gaussian approximation (σ ≈ 0.42·λ·N, λ = 0.55 µm) combined with a
/// 0.5-pixel baseline (anti-aliasing filter, demosaic). [`DEFAULT_SIGMA`] without both.
pub fn auto_sigma(f_number: Option<f32>, pitch_um: Option<f32>) -> f32 {
    match (f_number, pitch_um) {
        (Some(n), Some(p)) if n.is_finite() && p.is_finite() && n > 0.0 && p > 0.5 => {
            let diff = 0.42 * 0.55 * n / p;
            (diff * diff + 0.25).sqrt().clamp(0.3, 3.0)
        }
        _ => DEFAULT_SIGMA,
    }
}

/// The lens facts [`auto_sigma`] needs, from a file's Exif: the aperture, and the pixel
/// pitch (µm) of a source whose long side is `long_px` pixels: from the focal plane
/// resolution (scaled from the Exif image width to `long_px`), else from the 35 mm
/// equivalent focal length (a 36 mm long side). Unknown facts are `None`.
pub fn lens_of_file(data: &[u8], long_px: u32) -> (Option<f32>, Option<f32>) {
    let Some(exif) = specular_convert::source_meta(data).exif else {
        return (None, None);
    };
    let entry = |tag: u16| {
        exif.exif
            .iter()
            .chain(&exif.ifd0)
            .find(|e| e.tag == tag)
            .map(|e| e.data.as_slice())
    };
    let u32_at = |d: &[u8], i: usize| -> Option<u32> {
        Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
    };
    let rational = |tag: u16| -> Option<f64> {
        let d = entry(tag)?;
        let (n, dn) = (u32_at(d, 0)?, u32_at(d, 4)?);
        (dn != 0 && n != 0).then(|| f64::from(n) / f64::from(dn))
    };
    let number = |tag: u16| -> Option<f64> {
        let d = entry(tag)?;
        match d.len() {
            2 | 3 => Some(f64::from(u16::from_le_bytes([d[0], d[1]]))),
            _ => u32_at(d, 0).map(f64::from),
        }
        .filter(|v| *v > 0.0)
    };
    let f_number = rational(0x829D).map(|v| v as f32);
    let long = f64::from(long_px.max(1));
    let from_plane = rational(0xA20E).map(|res| {
        let unit_um = match number(0xA210).unwrap_or(2.0) as u32 {
            3 => 10_000.0,
            4 => 1_000.0,
            5 => 1.0,
            _ => 25_400.0,
        };
        // The resolution counts pixels of the Exif image (its width), not the source's.
        let exif_long = number(0xA002)
            .into_iter()
            .chain(number(0xA003))
            .fold(0.0f64, f64::max);
        let k = if exif_long > 0.0 {
            exif_long / long
        } else {
            1.0
        };
        unit_um / res * k
    });
    let from_35mm = || {
        let (f, f35) = (rational(0x920A)?, number(0xA405)?);
        Some(36.0 * f / f35 * 1000.0 / long)
    };
    let pitch = from_plane
        .or_else(from_35mm)
        .filter(|p| (0.5..30.0).contains(p))
        .map(|p| p as f32);
    (f_number, pitch)
}

/// The view-scale PSF σ, its tap radius and the iterations at `scale` (0 when nothing to do).
pub fn plan(c: &Capture, scale: f32) -> (f32, u32, u32) {
    let s = c.sigma() * scale;
    if !(s.is_finite() && s >= MIN_VIEW_SIGMA) || c.amount == 0.0 {
        return (s, 0, 0);
    }
    let r = (3.0 * s).ceil().max(1.0) as u32;
    (s, r, ITERATIONS)
}

/// Input pixels read around each output pixel.
pub fn halo(c: &Capture, scale: f32) -> u32 {
    let (_, r, k) = plan(c, scale);
    if k == 0 { 0 } else { 2 * k * r + r }
}

/// The deconvolution over `input`: planes observed `D` (0), estimate `U` (n), temporary
/// `T` (2n), ratio `R` (3n), local minimum (4n) and maximum (5n).
pub fn program(c: &Capture, input: Roi, scale: f32) -> Program {
    program_iterations(c, input, scale, plan(c, scale).2)
}

/// [`program`] with `k` iterations (when the PSF is large enough to run at all).
pub fn program_iterations(c: &Capture, input: Roi, scale: f32, k: u32) -> Program {
    let (w, h) = (input.w, input.h);
    let n = w * h;
    let (s, r, planned) = plan(c, scale);
    let k = if planned == 0 { 0 } else { k };
    let on = k > 0;
    let th = if on { [w, h] } else { [0, 0] };
    let (d, u, t, q, mn, mx) = (0, n, 2 * n, 3 * n, 4 * n, 5 * n);
    let taps = if on { gauss_taps(s, r) } else { vec![1.0] };
    let mut prog = Program::default();
    prog.step(&LUM, th, vec![d, u, w, input.x, input.y, fw(FLOOR)]);
    // The local range: rows into T (min) and R (max), then columns.
    prog.step(&MINMAX, th, vec![d, t, q, w, h, 0, r, 0]);
    prog.step(&MINMAX, th, vec![t, mn, mx, w, h, 1, r, q]);
    let blur = |src: u32, dst: u32, vert: bool, mode: u32| {
        let mut words = vec![
            src,
            dst,
            w,
            h,
            u32::from(vert),
            r,
            mode,
            d,
            u,
            mn,
            mx,
            fw(MARGIN),
            fw(DAMPING),
        ];
        words.extend(taps.iter().map(|v| fw(*v)));
        words
    };
    for _ in 0..k.max(1) {
        prog.step(&BLUR, th, blur(u, t, false, 0));
        prog.step(&BLUR, th, blur(t, q, true, 1));
        prog.step(&BLUR, th, blur(q, t, false, 0));
        prog.step(&BLUR, th, blur(t, u, true, 2));
    }
    prog.words = 6 * u64::from(n);
    prog
}

/// Capture sharpening, param version 1.
pub struct CaptureOp;

impl Op for CaptureOp {
    fn name(&self) -> &'static str {
        "capture_sharpening_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, p: &Params, scale: f32) -> u32 {
        halo(&Capture::of(p), scale)
    }
    fn heavy(&self) -> bool {
        true
    }
    fn param_words(&self) -> u32 {
        2
    }
    fn write_params(&self, p: &Params, scale: f32, out: &mut [u32]) {
        let c = Capture::of(p);
        out[0] = fw(c.amount);
        out[1] = u32::from(plan(&c, scale).2 > 0);
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[DETAIL_LUMA_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        Capture::of(p).amount == 0.0
    }
    fn local(&self, p: &Params, input: Roi, scale: f32) -> Option<Program> {
        Some(program(&Capture::of(p), input, scale))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    if (pu(p + 1u) == 0u) { return c; }
    let yl = dot(c.rgb, DV_DY);
    if (yl <= 1e-6) { return c; }
    let rw = pu(18u);
    let n = rw * pu(19u);
    let i = u32(y - i32(pu(17u))) * rw + u32(x - i32(pu(16u)));
    let d = lc(i);
    let u = lc(n + i);
    let nl = d + pf(p) * (u - d);
    return vec4<f32>(c.rgb * (nl / d), c.a);"
    }
}

/// Rec.2020 luminance weights for the node kernels.
pub const DETAIL_LUMA_WGSL: &str = "const DV_DY = vec3<f32>(0.2627, 0.6780, 0.0593);\n";
